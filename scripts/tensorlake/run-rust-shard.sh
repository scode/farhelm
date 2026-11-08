#!/bin/bash
# Run one partition of a Rust nextest archive on a Tensorlake clone, inside one attached exec (whatever an exec leaves
# running is frozen when the sandbox idles). `rust-tests.ts` beside this file stages the archive and calls this on each
# clone; `.agents/tensorlake.md` says when to use it. The clones come from the template `sandbox-tests.ts make-template`
# builds, which `bootstrap.sh` gives rootless Podman and the offline docker.io/library/ubuntu:24.04 image, so the
# containerized e2e tests run here too.
#
# The checkout is unpacked at the development machine's own absolute path, so every path the test binaries had compiled
# in (CARGO_MANIFEST_DIR fixtures, CARGO_BIN_EXE_farhelm, the target directory) is valid here without remapping, and the
# archive is extracted over that same target directory. No cargo exists here, so nextest runs with a copy of the repo's
# config whose setup script (`cargo build -p farhelm-fixtures`) is removed; the fixtures binary is shipped prebuilt.
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
if podman image exists docker.io/library/ubuntu:24.04; then
  podman_image=present
else
  podman_image=MISSING
  setup_error=${setup_error:-"the offline Podman image is missing"}
fi
{
  echo "pid 1: ${pid1:-unreadable}"
  echo "linger: $linger"
  echo "systemd --user: $state"
  echo "podman: $(podman --version 2>&1); image $podman_image"
} >"$OUT/summary.txt"
if [ -n "$setup_error" ]; then
  echo "setup failed: $setup_error; no tests were run" >>"$OUT/summary.txt"
  cat "$OUT/summary.txt"
  exit 1
fi
# --- self-ssh on a port of its own, as the browser shards do (scripts/tensorlake/run-shard.sh explains why not 22).
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
# --- the partition
cd "$M" || exit 1
export PATH="$M/.ci-tmux:$PATH"
# The caller names the pinned nextest (scripts/install-pinned-nextest.py's directory): .ci-nextest can also hold older
# installs, which the repo's config refuses.
nextest="$M/$NEXTEST"
config=/home/tl-user/stage/nextest-no-setup.toml
python3 - "$M/.config/nextest.toml" "$config" <<'PY'
import re, sys
text = open(sys.argv[1]).read()
# Drop the setup script and its profile hook, and the experimental flag that only they need.
text = re.sub(r"\[scripts\.setup\.build-fixtures\]\n(?:[^\[\n].*\n|\n)*", "", text)
text = re.sub(r"\[\[profile\.default\.scripts\]\]\n(?:[^\[\n].*\n|\n)*", "", text)
text = re.sub(r'^experimental = \["setup-scripts"\]\n', "", text, flags=re.M)
open(sys.argv[2], "w").write(text)
PY
python3 scripts/record-test-run.py --kind development --tmux required \
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
