#!/bin/bash
# Run one hash partition of a locally built Rust nextest archive on a bootstrapped boat sandbox. `boat-tests.ts rust`
# has already unpacked the checkout at the development machine's own absolute path (test binaries have it compiled in),
# so the archive is extracted over that tree and nothing is remapped. Nextest runs with a copy of the repo's config
# without the Cargo fixture-build setup (the binaries arrive prebuilt and boat workers build nothing); the Podman setup
# stays and consumes the staged static payloads.
#
# Usage: run-rust-shard.sh <checkout> <archive> <nextest binary, relative to the checkout> <k> <n> [nextest filter...]
# Writes /home/user/out/shard<k>/summary.txt and console.log.
set -u
# Boat's exec hands every command the ambient capability CAP_DAC_READ_SEARCH (CapAmb 0x4, observed 2026-11-13 and
# 2026-11-17), so even the unprivileged `user` reads files whose mode forbids it, and tests that expect a permission
# refusal fail (the old browser suite's unreadable-file test did). Re-exec once without inheritable or ambient
# capabilities, as an ordinary login would run.
if [ -z "${BOAT_CAPS_DROPPED:-}" ] && ! grep -qE '^CapAmb:[[:space:]]*0+$' /proc/self/status; then
  export BOAT_CAPS_DROPPED=1
  exec setpriv --inh-caps=-all --ambient-caps=-all bash "$0" "$@"
fi
M=$1 ARCHIVE=$2 NEXTEST=$3 K=$4 N=$5; shift 5
OUT=/home/user/out/shard$K
mkdir -p "$OUT"
XDG_RUNTIME_DIR=/run/user/$(id -u)
export XDG_RUNTIME_DIR
export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
setup_error=
state=$(systemctl --user is-system-running 2>&1)
case "$state" in running|degraded) ;; *) setup_error="the systemd user manager is not running" ;; esac
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
  echo "pid 1: $(cat /proc/1/comm)"
  echo "caps: $(grep -E '^Cap(Eff|Amb)' /proc/self/status | tr -s '[:space:]' ' ')"
  echo "systemd --user: $state"
  echo "podman: $(podman --version 2>&1); image $base $podman_image"
  # The distribution's tmux (bootstrap.sh), which the real-tmux tests use; missing means they would only print SKIPPED.
  echo "tmux: $(command -v tmux 2>&1) $(tmux -V 2>&1)"
} >"$OUT/summary.txt"
command -v tmux >/dev/null || setup_error=${setup_error:-"tmux is missing"}
if [ -n "$setup_error" ]; then
  echo "setup failed: $setup_error; no tests were run" >>"$OUT/summary.txt"
  cat "$OUT/summary.txt"
  exit 1
fi
ssh -o BatchMode=yes localhost true && echo "self-ssh: ok" >>"$OUT/summary.txt" || echo "self-ssh: FAILED" >>"$OUT/summary.txt"
cd "$M" || exit 1
nextest="$M/$NEXTEST"
config=/home/user/stage/nextest-no-setup.toml
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
# Keep passing tests' output in the JUnit report. A test that returns early without its substrate prints SKIPPED and
# passes; the recorder's generic mode keeps no per-test output, so this report is the only place a shard can find those
# lines (summarized below).
text = text.replace("store-success-output = false", "store-success-output = true")
open(sys.argv[2], "w").write(text)
PY
export FARHELM_TEST_PODMAN_PAYLOAD_DIR="$M/target/podman-test-payloads"
# Four slots, as the repo's recorder nextest mode enforces locally. With eight (nproc on a large boat), the real Podman
# test (threads-required = 4, in the e2e group capped at 4) and 57 other e2e tests were never scheduled and nextest
# ended with "58/1910 tests were not run" and no failure (round 240, run ec108b53).
threads=4
python3 scripts/record-test-run.py --kind development \
  --keep-farhelm-env FARHELM_TEST_PODMAN_PAYLOAD_DIR --keep-farhelm-env FARHELM_TEST_SKIP_PODMAN \
  --selection "boat sandbox rust shard $K/$N (hash partition): $* " \
  --concurrency "$threads nextest slots; retries 0" \
  -- "$nextest" nextest run --archive-file "$ARCHIVE" --extract-to "$M" --extract-overwrite --workspace-remap "$M" \
     --config-file "$config" --partition "hash:$K/$N" --test-threads "$threads" --retries 0 --no-tests warn "$@" \
  >"$OUT/console.log" 2>&1
code=$?
{
  echo "exit: $code"
  grep -E 'test-run evidence' "$OUT/console.log"
  grep -E '^\s+Summary |^\s+FAIL |SKIPPED' "$OUT/console.log" | sort -u | head -60
  junit="$M/target/nextest/default/junit.xml"
  if [ -f "$junit" ] && [ "$junit" -nt "$config" ]; then
    echo "runtime SKIPPED lines in passing tests' output: $(grep -c 'SKIPPED' "$junit")"
    grep -o 'SKIPPED[^<&]\{0,140\}' "$junit" | sort | uniq -c | sort -rn | head -40
  else
    echo "runtime SKIPPED lines: UNKNOWN (no JUnit report from this run)"
  fi
} >>"$OUT/summary.txt"
cat "$OUT/summary.txt"
exit "$code"
