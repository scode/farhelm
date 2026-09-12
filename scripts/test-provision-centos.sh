#!/usr/bin/env bash
# Provision a CentOS Stream 9 host from this repository's helm, over ssh.
#
# ## Why this exists
#
# Provisioning accepts any Linux host with a usable systemd user manager; it
# used to accept only `ID=ubuntu`. Nothing proved the wider promise, because
# every provisioning integration test targets `localhost` — which on a
# GitHub-hosted runner, and in this project's CI, is Ubuntu. So the one
# scenario the removed gate existed to forbid — a helm on one distribution
# installing a supervisor onto another — was the one scenario with no
# coverage at all.
#
# GitHub hosts no non-Ubuntu Linux runner, so the honest stand-in is a
# systemd-booted CentOS Stream 9 container ON the runner, reached over ssh and
# sftp exactly like any other remote host: the helm dials an ssh destination,
# the container answers with its own sshd, its own PAM stack, its own systemd
# user manager, and its own `/etc/os-release`. Nothing about the transport is
# simulated.
#
# ## Why the payloads are the release's static ones
#
# The workspace's ordinary debug `farhelm` is linked against this machine's
# glibc and would fail to exec on CentOS Stream 9, whose glibc is older. Both
# payloads pushed here are therefore the ones a real release ships: the
# musl-static `farhelm` for `x86_64-unknown-linux-musl`, and the pinned static
# tmux. That makes this leg a test of the artifacts users actually receive,
# not of a build only this machine can run. `scripts/check-static-elf.sh`
# refuses either payload if it is not in fact a static ELF for the right
# machine, because a dynamic one would fail on first exec inside the container
# with nothing nearby to explain it.
#
# ## Known limit
#
# The container's SELinux is not enforcing (containers run under the host's
# policy, and the host here is not RHEL-family). This leg therefore covers
# CentOS's PAM, systemd, filesystem layout, and libc — not SELinux confinement
# of the installed supervisor. RHEL-family hosts with SELinux enforcing remain
# untested.
#
# Requires: docker, a Rust toolchain with the musl target (installed here if
# absent) and `musl-tools` for rusqlite's bundled SQLite, plus the usual
# ssh client. Takes a few minutes; the container image is built once and
# cached by the docker daemon.

set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
record_args=(--kind development)
while [ "$#" -gt 0 ]; do
  case "$1" in
    --kind|--output-root)
      [ "$#" -ge 2 ] || { echo "missing value for $1" >&2; exit 2; }
      record_args+=("$1" "$2")
      shift 2
      ;;
    *) echo "usage: $0 [--kind development|repetition|release] [--output-root PATH]" >&2; exit 2 ;;
  esac
done
target_triple=x86_64-unknown-linux-musl
# Respect an external CARGO_TARGET_DIR: CI and local runs both override it,
# and guessing `$repo/target` would look for a payload that was built
# somewhere else.
cargo_target_dir=${CARGO_TARGET_DIR:-$repo/target}
farhelm_payload="$cargo_target_dir/$target_triple/debug/farhelm"

# The image is derived rather than used bare: `quay.io/centos/centos:stream9`
# ships no systemd at all (no `/sbin/init`), and PID 1 cannot be changed after
# a container starts. The tag is fixed so repeat runs hit the daemon's layer
# cache instead of re-running dnf.
image=farhelm-centos-ci:stream9
container_user=farhelm
# The one test that dials the destination. Deliberately narrow: a broader
# filter would pull in the localhost and direct-local cases, which correctly
# SKIP where no local user manager exists — and this script treats a skip as a
# failure, since a leg that skipped covered nothing.
test_filter=provisioning::tests::provisioning_and_update_over_ssh_preserve_an_operable_session

ssh_config="$HOME/.ssh/config"

run_dir=$(mktemp -d "${TMPDIR:-/tmp}/farhelm-centos-ci.XXXXXX")
container=""
# The alias the test dials is PER RUN, suffixed with the run directory's
# random tail, and so are the markers that fence its block in `~/.ssh/config`.
# `~/.ssh/config` is shared by every checkout this user runs this script from,
# and a fixed alias meant a second concurrent run (another agent on the same
# machine) silently repointed the first run's destination at its own container
# and then deleted the block on exit, so the first run's test dialed a host
# that no longer existed. The prefix stays the same so a human can still find
# and remove every block this script ever wrote.
ssh_alias_prefix=farhelm-centos-target
ssh_alias="$ssh_alias_prefix-${run_dir##*.}"
config_begin_prefix="# BEGIN farhelm-centos-ci"
config_begin="$config_begin_prefix $ssh_alias (scripts/test-provision-centos.sh)"
config_end="# END farhelm-centos-ci $ssh_alias"
# Whether this run had to create `~/.ssh/config`. If it did, and scrubbing the
# block leaves it empty, the file goes too — a machine that had no ssh config
# before this ran should have none after.
created_ssh_config=no

# --------------------------------------------------------------------------
# Teardown
#
# Both halves must run on EVERY exit path, including a failed assertion in the
# middle of the suite: a leaked container holds a published port and a few
# hundred MB, and a leaked `~/.ssh/config` block would point a real alias at a
# port that no longer answers.
# --------------------------------------------------------------------------

# Every write to `~/.ssh/config` is a read-modify-write through a scratch
# file, and every run on this machine shares that one file. Without a lock,
# run B can snapshot the config in the instant after run A truncated it and
# before A refilled it, and B's write-back then drops A's stanza (A's test
# dials a name that no longer resolves) or the user's own stanzas. The lock
# is an flock on a fixed file next to the config, held only across the
# sweep-and-write at setup and the removal at teardown, never across the
# test itself. Where flock(1) is missing the writes go ahead unlocked, as
# they always did; the loss is a narrow race, not a wrong result.
#
# The fd is unlocked explicitly before it is closed, because children
# spawned while it is open inherit the open file description and closing
# only this shell's fd would leave a still-running child holding the lock.
ssh_config_lock="$HOME/.ssh/.farhelm-centos-ci.lock"
ssh_config_locked=no
lock_ssh_config() {
  [ "$ssh_config_locked" = no ] || return 0
  command -v flock >/dev/null 2>&1 || return 0
  exec 8>>"$ssh_config_lock"
  flock -w 60 8 || {
    echo "another run has held $ssh_config_lock for over a minute; giving up" >&2
    return 1
  }
  ssh_config_locked=yes
}
unlock_ssh_config() {
  [ "$ssh_config_locked" = yes ] || return 0
  flock -u 8
  exec 8>&-
  ssh_config_locked=no
}

# Remove one alias's block: this run's own by default, or the alias named by
# `$1`. Only exact markers match, so a concurrent run's block (a different
# alias, hence different markers) is left alone. Callers hold the ssh config
# lock; this function does not take it, so that the setup sweep can remove
# several blocks under one acquisition.
remove_ssh_config_block() {
  local begin="$config_begin" end="$config_end"
  if [ -n "${1:-}" ]; then
    begin="$config_begin_prefix $1 (scripts/test-provision-centos.sh)"
    end="# END farhelm-centos-ci $1"
  fi
  test -f "$ssh_config" || return 0
  scrubbed="$run_dir/ssh-config.scrubbed"
  awk -v begin="$begin" -v end="$end" '
    $0 == begin { skip = 1; next }
    $0 == end   { skip = 0; next }
    !skip       { print }
  ' "$ssh_config" >"$scrubbed"
  # Write THROUGH the existing file rather than renaming over it: the config
  # keeps its inode and its 0600 mode, and a symlinked config keeps pointing
  # where the user aimed it.
  cat "$scrubbed" >"$ssh_config"
  if [ "$created_ssh_config" = yes ] && [ ! -s "$ssh_config" ]; then
    rm -f "$ssh_config"
  fi
}

cleanup() {
  status=$?
  if [ -n "$container" ]; then
    docker rm -f "$container" >/dev/null 2>&1 || true
  fi
  # A failure inside the locked setup section arrives here with the lock
  # still held; lock_ssh_config is a no-op in that case, and the unlock
  # afterwards releases either acquisition.
  if lock_ssh_config; then
    remove_ssh_config_block || true
    unlock_ssh_config || true
  else
    echo "skipping ssh config cleanup: could not take $ssh_config_lock" >&2
  fi
  rm -rf "$run_dir"
  exit "$status"
}
trap cleanup EXIT

# --------------------------------------------------------------------------
# Payloads
# --------------------------------------------------------------------------

echo "== building the musl-static farhelm payload"
if ! rustup target list --installed 2>/dev/null | grep -qx "$target_triple"; then
  rustup target add "$target_triple"
fi
(cd "$repo" && cargo build -p farhelm --target "$target_triple" --locked)
"$repo/scripts/check-static-elf.sh" "$farhelm_payload" "$target_triple"

# `build-tmux-assets.sh` is the release's producer and builds BOTH published
# architectures; half of that is aarch64 this leg would never push. Its
# single-architecture sibling below builds the same binary from the same
# checksummed pins for the host's own architecture, asserts the pinned tmux
# version, caches under `.ci-tmux/`, and is already a CI dependency — so it is
# what this uses. It prints the containing directory and nothing else.
echo "== building the static tmux payload"
tmux_payload="$("$repo/scripts/build-pinned-tmux-ci.sh")/tmux"
"$repo/scripts/check-static-elf.sh" "$tmux_payload" "$target_triple"

# --------------------------------------------------------------------------
# The target host
# --------------------------------------------------------------------------

echo "== building the CentOS Stream 9 image"
cat >"$run_dir/Dockerfile" <<EOF
FROM quay.io/centos/centos:stream9
# systemd is the point of the image: provisioning writes a user unit and
# expects a user manager to start it. openssh-server is the transport, and it
# brings the sftp subsystem the payload push needs.
RUN dnf -y install --setopt=install_weak_deps=False systemd openssh-server \\
  && dnf clean all
RUN ssh-keygen -A && systemctl enable sshd
# A normal, unprivileged account: provisioning's whole no-root promise is that
# it installs under \$HOME.
RUN useradd --create-home $container_user
# Then remove that account's /etc/shadow entry and leave a '*' password in
# /etc/passwd instead. This is NOT about weakening authentication — '*' is
# still "no password can ever match", and only the injected key logs in. It is
# about pam_unix's account phase, which delegates shadow reads to the
# setcap'd unix_chkpwd helper. On an AppArmor host (Ubuntu, including
# GitHub-hosted runners) the host's unix-chkpwd profile confines that helper
# INSIDE the container too and denies it dac_read_search, so it cannot read
# /etc/shadow, and sshd refuses every login with "Access denied for user by
# PAM account configuration" — before pam_systemd ever runs, which is exactly
# the user session this leg needs. With no shadow entry, pam_unix answers from
# /etc/passwd itself and never invokes the helper.
RUN sed -i "/^$container_user:/d" /etc/shadow \\
  && sed -i "s|^$container_user:x:|$container_user:*:|" /etc/passwd
CMD ["/usr/sbin/init"]
EOF
docker build -t "$image" "$run_dir"

echo "== booting the container"
# Flag by flag, because each one is load-bearing and none is cargo-culted:
#   --privileged        systemd needs to mount its own API filesystems and
#                       manage cgroups; an unprivileged container cannot.
#   --cgroupns=host     with the host's cgroup tree bind-mounted in, the
#                       container's systemd must be in the namespace that tree
#                       belongs to. A private cgroup namespace plus a host
#                       mount is contradictory, and PID 1 exits immediately
#                       (status 255, no log line) when given it.
#   -v /sys/fs/cgroup   systemd writes there for every unit it starts; read
#                       only would let it boot and then fail to start anything.
#   --tmpfs /run …lock  systemd wants a fresh, writable runtime tree, and this
#                       is where the user manager's /run/user/<uid> appears.
#   /usr/sbin/init      systemd as PID 1 — the whole reason for the image.
#   -p 127.0.0.1::22    published to loopback only, and to a port the DAEMON
#                       picks. Choosing one here would be a race between the
#                       check and the bind; asking docker afterwards cannot be.
container=$(docker run -d \
  --privileged \
  --cgroupns=host \
  -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
  --tmpfs /run \
  --tmpfs /run/lock \
  -p 127.0.0.1::22 \
  "$image" /usr/sbin/init)
port=$(docker port "$container" 22 | head -n 1 | sed 's/.*://')
test -n "$port" || {
  echo "the container published no port for sshd" >&2
  exit 1
}
echo "   container ${container:0:12} sshd on 127.0.0.1:$port"

echo "== authorizing this run's key on the target"
ssh-keygen -q -t ed25519 -N '' -C "$ssh_alias" -f "$run_dir/id_ed25519"
# `docker exec` rather than a build argument: the key is generated per run and
# must never end up in a cached image layer.
docker exec -i "$container" bash -s <<EOF
set -euo pipefail
install -d -m 700 -o $container_user -g $container_user /home/$container_user/.ssh
install -m 600 -o $container_user -g $container_user /dev/stdin \
  /home/$container_user/.ssh/authorized_keys <<'KEY'
$(cat "$run_dir/id_ed25519.pub")
KEY
EOF

# Linger, not because the test asserts it, but because the supervisor must
# outlive the ssh session that installs it: without linger, systemd tears the
# user manager down when the last session ends, taking the just-started
# supervisor with it. logind may still be starting, so this retries.
for _ in $(seq 1 30); do
  if docker exec "$container" loginctl enable-linger "$container_user" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

echo "== teaching this user's ssh about the target"
mkdir -p "$HOME/.ssh"
chmod 700 "$HOME/.ssh"
# Held from the stale sweep through this run's own stanza landing in the file:
# the two are one logical edit, and the keyscan between them is fast.
lock_ssh_config
# Scrub stale blocks first, create second — and in that order, because the
# scrub can delete a config this script created, which the creation below has
# to happen after rather than before. A block is stale when the identity file
# it names is gone: a run killed before its trap fired leaves a block naming a
# port that no longer answers, and its run directory (where the key lived) is
# what a reboot or a /tmp cleaner removes. A block whose key still exists may
# belong to a run in another checkout that is live right now, and is left
# alone; this run's own alias cannot be in the file yet, so nothing here ever
# touches a live block.
if [ -f "$ssh_config" ]; then
  while read -r stale_alias; do
    [ -n "$stale_alias" ] || continue
    identity=$(awk -v begin="$config_begin_prefix $stale_alias (scripts/test-provision-centos.sh)" \
      -v end="# END farhelm-centos-ci $stale_alias" '
      $0 == begin { inside = 1; next }
      $0 == end   { inside = 0; next }
      inside && $1 == "IdentityFile" { print $2; exit }
    ' "$ssh_config")
    if [ -z "$identity" ] || [ ! -e "$identity" ]; then
      echo "   removing stale ssh config block for $stale_alias"
      remove_ssh_config_block "$stale_alias"
    fi
  done <<EOF
$(awk -v prefix="$config_begin_prefix " -v alias_prefix="$ssh_alias_prefix-" \
  'index($0, prefix) == 1 && index($4, alias_prefix) == 1 { print $4 }' "$ssh_config")
EOF
  # Before aliases were per run, the block carried no alias in its markers. A
  # machine that ran that version and was then killed mid-run still has one of
  # those; nothing dials its alias any more, so it is always stale.
  if grep -qxF "$config_begin_prefix (scripts/test-provision-centos.sh)" "$ssh_config"; then
    echo "   removing the pre-per-run-alias ssh config block"
    scrubbed="$run_dir/ssh-config.scrubbed"
    awk -v begin="$config_begin_prefix (scripts/test-provision-centos.sh)" -v end="# END farhelm-centos-ci" '
      $0 == begin { skip = 1; next }
      $0 == end   { skip = 0; next }
      !skip       { print }
    ' "$ssh_config" >"$scrubbed"
    cat "$scrubbed" >"$ssh_config"
  fi
fi
if [ ! -e "$ssh_config" ]; then
  created_ssh_config=yes
  # ssh refuses a group- or world-writable config, and the default umask on
  # some systems produces exactly that.
  install -m 600 /dev/null "$ssh_config"
fi
# BatchMode makes an unknown host key a hard failure rather than a prompt, so
# known_hosts is seeded before anything dials — the same `-H` keyscan
# `ci.yml`'s localhost step uses. It is seeded into a RUN-SCOPED file rather
# than `~/.ssh/known_hosts` because every run gets a fresh container with a
# fresh host key on a daemon-chosen port: a shared file would accumulate dead
# entries and, whenever the daemon reused a port, would report a host-key
# mismatch for what is simply the next container.
ssh-keyscan -H -p "$port" 127.0.0.1 >"$run_dir/known_hosts" 2>/dev/null
test -s "$run_dir/known_hosts" || {
  echo "sshd on 127.0.0.1:$port offered no host key to keyscan" >&2
  docker logs "$container" 2>&1 | tail -n 40 >&2
  exit 1
}
# The destination the test uses is a plain NAME, resolved by this user's own
# ssh configuration — which is how provisioning reaches every real host, and
# what keeps the port, the key, and the account out of the test's own code.
{
  printf '%s\n' "$config_begin"
  printf 'Host %s\n' "$ssh_alias"
  printf '    HostName 127.0.0.1\n'
  printf '    Port %s\n' "$port"
  printf '    User %s\n' "$container_user"
  printf '    IdentityFile %s\n' "$run_dir/id_ed25519"
  printf '    IdentitiesOnly yes\n'
  printf '    UserKnownHostsFile %s\n' "$run_dir/known_hosts"
  printf '    StrictHostKeyChecking yes\n'
  printf '    BatchMode yes\n'
  printf '\n'
  printf '%s\n' "$config_end"
  cat "$ssh_config"
} >"$run_dir/ssh-config.new"
cat "$run_dir/ssh-config.new" >"$ssh_config"
unlock_ssh_config

# Readiness is TWO conditions, and the second is the one that matters. ssh
# answering only proves sshd is up; provisioning additionally needs pam_systemd
# to have created a user session, because without it there is no
# XDG_RUNTIME_DIR, `systemctl --user` fails, and the reach check reports "no
# usable systemd user manager" — which the helm answers by declining to
# provision at all. Waiting on the real condition here turns a slow boot into a
# wait and a broken PAM stack into a loud failure, instead of both looking like
# an unprovisionable host later.
echo "== waiting for ssh and the target's systemd user manager"
ready=no
for _ in $(seq 1 60); do
  if ssh -o BatchMode=yes "$ssh_alias" 'systemctl --user show-environment' >/dev/null 2>&1; then
    ready=yes
    break
  fi
  sleep 2
done
if [ "$ready" != yes ]; then
  echo "the target never presented a usable systemd user session over ssh" >&2
  echo "--- ssh attempt ---" >&2
  ssh -v -o BatchMode=yes "$ssh_alias" 'id; systemctl --user show-environment' >&2 2>&1 || true
  echo "--- container journal ---" >&2
  docker exec "$container" journalctl --no-pager -n 200 >&2 2>&1 || true
  exit 1
fi

# --------------------------------------------------------------------------
# The test
# --------------------------------------------------------------------------

echo "== provisioning $ssh_alias from this workspace's helm"
output="$run_dir/nextest.log"
set +e
(
  cd "$repo" &&
    FARHELM_TEST_SSH_DESTINATION="$ssh_alias" \
      FARHELM_TEST_BINARY="$farhelm_payload" \
      FARHELM_TEST_TMUX="$tmux_payload" \
      python3 scripts/record-test-run.py --runner nextest --require-complete-console "${record_args[@]}" \
        --selection 'CentOS provisioning over ssh' --concurrency '4 nextest slots; one selected test' \
        --tmux none --keep-farhelm-env FARHELM_TEST_SSH_DESTINATION \
        --keep-farhelm-env FARHELM_TEST_BINARY --keep-farhelm-env FARHELM_TEST_TMUX \
        -- cargo nextest run -p farhelm-helm --lib -E "test(=$test_filter)"
) 2>&1 | tee "$output"
test_status=${PIPESTATUS[0]}
set -e

if [ "$test_status" -ne 0 ]; then
  echo "the provisioning suite failed against $ssh_alias" >&2
  exit "$test_status"
fi
# A skip is a failure here. The whole point of this leg is coverage that RAN,
# and every reason the test skips itself (no reachable destination, no
# payload) is something this script was supposed to have provided.
if grep -q '^[[:space:]]*SKIPPED ' "$output"; then
  echo "the provisioning test SKIPPED instead of running:" >&2
  grep '^[[:space:]]*SKIPPED ' "$output" >&2
  exit 1
fi
# Complete console forwarding is required above: a missing SKIPPED line
# cannot be treated as evidence that provisioning ran. Nextest indents its
# captured output, hence the whitespace allowance in the skip witness.
# Nextest refuses an empty exact selection. Its immediate success output
# also retains the underlying test process's one-pass witness and SKIPPED
# explanation, so a substrate skip cannot quietly turn this gate green.
if ! grep -q 'test result: ok\. 1 passed' "$output"; then
  echo "expected exactly one test to run and pass; the filter matched something else:" >&2
  grep 'test result:' "$output" >&2
  exit 1
fi

echo "== ok: provisioned a CentOS Stream 9 host over ssh"
