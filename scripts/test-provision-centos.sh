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
ssh_alias=farhelm-centos-target
# The one test that dials the destination. Deliberately narrow: a broader
# filter would pull in the localhost and direct-local cases, which correctly
# SKIP where no local user manager exists — and this script treats a skip as a
# failure, since a leg that skipped covered nothing.
test_filter=provisioning_and_update_over_ssh_preserve_an_operable_session

ssh_config="$HOME/.ssh/config"
config_begin="# BEGIN farhelm-centos-ci (scripts/test-provision-centos.sh)"
config_end="# END farhelm-centos-ci"

run_dir=$(mktemp -d "${TMPDIR:-/tmp}/farhelm-centos-ci.XXXXXX")
container=""
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

remove_ssh_config_block() {
  test -f "$ssh_config" || return 0
  scrubbed="$run_dir/ssh-config.scrubbed"
  awk -v begin="$config_begin" -v end="$config_end" '
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
  remove_ssh_config_block || true
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
# Scrub first, create second — and in that order, because the scrub is what
# makes this idempotent (a run killed before its trap fired leaves a block
# naming a port that is now gone) and it also deletes a config this script
# created, which the creation below has to happen after rather than before.
remove_ssh_config_block
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
output="$run_dir/cargo-test.log"
set +e
(
  cd "$repo" &&
    FARHELM_TEST_SSH_DESTINATION="$ssh_alias" \
      FARHELM_TEST_BINARY="$farhelm_payload" \
      FARHELM_TEST_TMUX="$tmux_payload" \
      cargo test -p farhelm-helm "$test_filter" -- --show-output
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
if grep -q '^SKIPPED ' "$output"; then
  echo "the provisioning test SKIPPED instead of running:" >&2
  grep '^SKIPPED ' "$output" >&2
  exit 1
fi
# And a filter that matches nothing also "passes". Require the test to have
# been selected at all, so a rename cannot quietly empty this gate.
if ! grep -q 'test result: ok\. 1 passed' "$output"; then
  echo "expected exactly one test to run and pass; the filter matched something else:" >&2
  grep 'test result:' "$output" >&2
  exit 1
fi

echo "== ok: provisioned a CentOS Stream 9 host over ssh"
