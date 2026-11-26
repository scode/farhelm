#!/bin/bash
# Run the GPUI app's end-to-end suites on a bootstrapped boat sandbox, against binaries built on the development machine
# (`boat-tests.ts gui` builds them the way `scripts/gui-agent.py start` would and unpacks them into the tree at the
# development machine's own absolute path, so the scripts find them where `cargo metadata` says the target directory
# is). Each suite runs through the recorder with `--no-build`, one after the other: each starts its own headless Mutter
# and app, and both render with lavapipe, which is CPU-bound.
#
# Usage: run-gui.sh <checkout> <suite>... where a suite is `smoke` (scripts/gui-e2e-smoke.py) or `lifecycle`
# (scripts/gui-lifecycle.py). Writes /home/user/out/gui/summary.txt and one console log per suite.
set -u
# Boat's exec hands every command the ambient capability CAP_DAC_READ_SEARCH (observed 2026-11-13 and 2026-11-17), so
# the unprivileged `user` reads files whose mode forbids it. Re-exec once without inheritable or ambient capabilities,
# as an ordinary login would run, so nothing in a run can lean on that.
if [ -z "${BOAT_CAPS_DROPPED:-}" ] && ! grep -qE '^CapAmb:[[:space:]]*0+$' /proc/self/status; then
  export BOAT_CAPS_DROPPED=1
  exec setpriv --inh-caps=-all --ambient-caps=-all bash "$0" "$@"
fi
M=$1; shift
OUT=/home/user/out/gui
mkdir -p "$OUT"
cd "$M" || exit 1
{
  echo "caps: $(grep -E '^Cap(Eff|Amb)' /proc/self/status | tr -s '[:space:]' ' ')"
  echo "mutter: $(mutter --version 2>&1 | head -1)"
  echo "mesa: $(dpkg-query -W -f='${Version}' mesa-vulkan-drivers 2>&1)"
  for b in farhelm-desktop farhelm farhelm-fixtures gpui-agent; do
    missing=$(ldd "target/debug/$b" 2>&1 | grep -c 'not found')
    echo "$b: $(test -x "target/debug/$b" && echo present || echo MISSING), $missing unresolved libraries"
  done
} >"$OUT/summary.txt"
code=0
for suite in "$@"; do
  case "$suite" in
  smoke)
    script=scripts/gui-e2e-smoke.py selection='GPUI app end-to-end smoke'
    concurrency='one app run, steps in order'
    ;;
  lifecycle)
    script=scripts/gui-lifecycle.py selection='Linux app lifecycle'
    concurrency='one app run, launched again in order'
    ;;
  *)
    echo "unknown suite $suite" >>"$OUT/summary.txt"
    code=2
    continue
    ;;
  esac
  python3 scripts/record-test-run.py --kind development --selection "$selection (boat sandbox)" \
    --concurrency "$concurrency" --termination-grace 30 -- python3 -B "$script" --no-build \
    >"$OUT/$suite.log" 2>&1
  c=$?
  [ "$c" = 0 ] || code=$c
  {
    echo "$suite: exit $c"
    grep -E 'test-run evidence' "$OUT/$suite.log"
    # The smoke reports like unittest; the lifecycle prints its own closing line.
    grep -E '^(Ran |OK|FAILED)|^(FAIL|ERROR):|skipped|^gui-lifecycle: (every step passed|FAILED)' "$OUT/$suite.log" \
      | head -20
  } >>"$OUT/summary.txt"
done
cat "$OUT/summary.txt"
exit "$code"
