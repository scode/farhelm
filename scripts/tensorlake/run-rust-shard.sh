#!/bin/bash
# Run one partition of a Rust nextest archive on a Tensorlake clone, inside one attached exec (whatever an exec leaves
# running is frozen when the sandbox idles). `rust-tests.ts` beside this file stages the archive and calls this on each
# clone; `.agents/tensorlake.md` says when to use it. The clones come from the template `sandbox-tests.ts make-template`
# builds, which `bootstrap.sh` gives rootless Podman and the exact production Ubuntu base offline, so the real
# environment e2e test can build its recipe here through the production provider.
#
# The checkout is unpacked at the development machine's own absolute path, so every path the test binaries had compiled
# in (CARGO_MANIFEST_DIR fixtures, CARGO_BIN_EXE_farhelm, the target directory) is valid here without remapping, and the
# archive is extracted over that same target directory. No cargo exists here, so nextest runs with a copy of the repo's
# config with only the local fixture-build setup removed. Static installer payloads arrive prebuilt. The retained
# Podman setup exposes their staged path only to its matching test.
#
# Usage: run-rust-shard.sh <checkout> <archive> <nextest binary, relative to the checkout> <k> <n> [nextest filter args...]
# Writes /home/tl-user/out/shard<k>/summary.txt and console.log; the recorder's run directory stays under
# /tmp/farhelm-tmp/test-runs on the sandbox.
set -u
M=$1 ARCHIVE=$2 NEXTEST=$3 K=$4 N=$5; shift 5
OUT=/home/tl-user/out/shard$K
mkdir -p "$OUT"
uid=$(id -u)
export XDG_RUNTIME_DIR=/run/user/$uid
export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
setup_error=
pid1=$(cat /proc/1/comm 2>/dev/null)
# --- systemd user manager: clones of the template boot systemd; lingering keeps tl-user's manager up with no session.
if [ "$pid1" = systemd ] || [ -S /run/systemd/private ]; then
  for _ in $(seq 1 40); do systemctl is-active --quiet systemd-logind && break; sleep 0.5; done
  if sudo -n loginctl enable-linger "$uid"; then
    linger=ok
  else
    linger=FAILED
    setup_error="loginctl enable-linger failed"
  fi
else
  linger="n/a"; setup_error="PID 1 is not systemd; run only on clones of the template"
fi
for _ in $(seq 1 40); do
  state=$(systemctl --user is-system-running 2>&1)
  case "$state" in running|degraded) break ;; esac
  sleep 0.5
done
case "$state" in running|degraded) ;; *) setup_error=${setup_error:-"the systemd user manager is not running"} ;; esac
# --- rootless Podman needs /dev/net/tun open to the user (no udevd runs on clones to apply a rule) and working DNS.
sudo -n chmod 0666 /dev/net/tun || setup_error=${setup_error:-"cannot open /dev/net/tun"}
sudo -n systemctl restart systemd-resolved 2>/dev/null
base=docker.io/library/ubuntu@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852
if [ "${FARHELM_TEST_SKIP_PODMAN:-}" = 1 ]; then
  podman_image=SKIPPED
elif podman image exists "$base"; then
  podman_image=present
else
  podman_image=MISSING
  setup_error=${setup_error:-"the offline Podman image is missing"}
fi
{
  echo "pid 1: ${pid1:-unreadable}"
  echo "linger: $linger"
  echo "systemd --user: $state"
  echo "podman: $(podman --version 2>&1); image $base $podman_image"
} >"$OUT/summary.txt"
if [ -n "$setup_error" ]; then
  echo "setup failed: $setup_error; no tests were run" >>"$OUT/summary.txt"
  cat "$OUT/summary.txt"
  exit 1
fi
# --- self-ssh
# The ssh-to-localhost tests reach a second supervisor with plain `ssh localhost`, so the OpenSSH client must reach an
# sshd that takes tl-user's key. Port 22 cannot be relied on for that: on clones that boot systemd (observed November
# 2026) something other than the image's sshd already listens on 0.0.0.0:22, so ssh.socket fails to start, and that
# listener presents its own host key and refuses tl-user's key ("Permission denied (publickey)"). The shard therefore
# starts the image's sshd on a port of its own and points the client's `localhost` and `127.0.0.1` at it through
# ~/.ssh/config, which `ssh localhost` honors. Earlier entries for those names are dropped from known_hosts first: a
# template's file can hold keys from an earlier boot, and ssh refuses on a stale entry rather than trying an appended
# one. The scan uses `-4` because `ssh-keyscan localhost` would otherwise try ::1 alone.
SELF_SSH_PORT=2222
sudo -n mkdir -p /run/sshd
ss -ltn "sport = :$SELF_SSH_PORT" | grep -q LISTEN || sudo -n /usr/sbin/sshd -p "$SELF_SSH_PORT"
if ! grep -q '^# farhelm self-ssh$' /home/tl-user/.ssh/config 2>/dev/null; then
  printf '# farhelm self-ssh\nHost localhost 127.0.0.1\n  Port %s\n\n' "$SELF_SSH_PORT" >>/home/tl-user/.ssh/config
  chmod 600 /home/tl-user/.ssh/config
fi
for host in localhost 127.0.0.1 "[localhost]:$SELF_SSH_PORT" "[127.0.0.1]:$SELF_SSH_PORT"; do
  ssh-keygen -q -f /home/tl-user/.ssh/known_hosts -R "$host" >/dev/null 2>&1
done
ssh-keyscan -4 -H -p "$SELF_SSH_PORT" localhost 127.0.0.1 >>/home/tl-user/.ssh/known_hosts 2>/dev/null
ssh -o BatchMode=yes localhost true && echo "self-ssh: ok" >>"$OUT/summary.txt" || echo "self-ssh: FAILED" >>"$OUT/summary.txt"
# A template without tmux runs the tests that start a real tmux server as SKIPPED passes; say so where the summary is read.
{ tmux -V 2>/dev/null || echo "tmux: MISSING"; } >>"$OUT/summary.txt"
# --- the partition
cd "$M" || exit 1
# The caller names the pinned nextest (scripts/install-pinned-nextest.py's directory): .ci-nextest can also hold older
# installs, which the repo's config refuses.
nextest="$M/$NEXTEST"
config=/home/tl-user/stage/nextest-no-setup.toml
python3 - "$M/.config/nextest.toml" "$config" <<'PY' || exit 1
import re, sys
text = open(sys.argv[1]).read()
# Drop only the Cargo fixture build. The retained Podman setup consumes staged
# static payloads and scopes the helm payload variable to that test.
text = re.sub(r"\[scripts\.setup\.build-fixtures\]\n(?:[^\[\n].*\n|\n)*", "", text)
text = re.sub(r"\[\[profile\.default\.scripts\]\]\n(?:[^\[\n].*\n|\n)*",
              lambda block: "" if re.search(r'^setup = "build-fixtures"$', block[0], re.M) else block[0], text)
if "[scripts.setup." not in text:
    text = re.sub(r'^experimental = \["setup-scripts"\]\n', "", text, flags=re.M)
open(sys.argv[2], "w").write(text)
PY
# This private nextest setup input selects the staged files. The retained setup
# exports FARHELM_HELM_PAYLOAD_DIR only for podman_environments, after the
# recorder has scrubbed unrelated ambient Farhelm variables.
export FARHELM_TEST_PODMAN_PAYLOAD_DIR="$M/target/podman-test-payloads"
python3 scripts/record-test-run.py --kind development \
  --keep-farhelm-env FARHELM_TEST_PODMAN_PAYLOAD_DIR --keep-farhelm-env FARHELM_TEST_SKIP_PODMAN \
  --selection "sandbox rust shard $K/$N (hash partition): $* " \
  --concurrency '4 nextest slots; retries 0' \
  -- "$nextest" nextest run --archive-file "$ARCHIVE" --extract-to "$M" --extract-overwrite --workspace-remap "$M" \
     --config-file "$config" --partition "hash:$K/$N" --test-threads 4 --retries 0 --no-tests warn "$@" \
  >"$OUT/console.log" 2>&1
code=$?
{
  echo "exit: $code"
  grep -E 'test-run evidence' "$OUT/console.log"
  grep -E '^\s+Summary |^\s+FAIL |SKIPPED' "$OUT/console.log" | sort -u | head -60
} >>"$OUT/summary.txt"
cat "$OUT/summary.txt"
exit "$code"
