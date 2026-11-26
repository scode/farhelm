#!/bin/bash
# Build the GPUI app in the worker's synced tree and run its end-to-end suites there, each through the recorder, one
# after the other: each starts its own headless Mutter and app, and both render with lavapipe, which is CPU-bound. The
# first suite builds the app the way `scripts/gui-agent.py start` does; the second reuses that build.
#
# Usage: run-gui.sh <base> <out> [smoke] [lifecycle] (both when none is named). Writes summary.txt, one console log
# per suite, and runs.tar.gz on failure.
set -u
# Finish the foreground recorder's cleanup on TERM and skip the next suite.
cancelled=
trap 'cancelled=1' TERM
base=$(cd "$1" && pwd) || exit 1
out=$2
shift 2
# shellcheck source=SCRIPTDIR/env.sh
. "$(dirname "$0")/env.sh"
cd "$base/src" || exit 1
use_pinned_toolchain
[ $# -gt 0 ] || set -- smoke lifecycle
disk_full=
{
  substrate
  keep_disk_free || disk_full=1
  echo "mutter: $(mutter --version 2>&1 | head -1)"
  echo "mesa: $(dpkg-query -W -f='${Version}' mesa-vulkan-drivers 2>&1)"
} >"$out/summary.txt"
[ -z "$disk_full" ] || { cat "$out/summary.txt"; exit "$DISK_FULL_STATUS"; }
code=0 built='' runs=()
for suite in "$@"; do
  [ -z "$cancelled" ] || break
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
    echo "unknown suite $suite" >>"$out/summary.txt"
    code=2
    continue
    ;;
  esac
  build=()
  [ -n "$built" ] && build=(--no-build)
  python3 scripts/record-test-run.py --kind development --selection "$selection (test worker, $(uname -m))" \
    --concurrency "$concurrency" --termination-grace 30 -- python3 -B "$script" "${build[@]}" \
    >"$out/$suite.log" 2>&1
  c=$?
  built=1
  [ "$c" = 0 ] || code=$c
  run_dir=$(grep -oE 'test-run evidence: [^ ]+' "$out/$suite.log" | tail -1 | cut -d' ' -f3)
  [ "$c" != 0 ] && [ -n "$run_dir" ] && runs+=("$run_dir")
  {
    echo "$suite: exit $c"
    echo "run: ${run_dir:-unknown}"
    # The smoke reports like unittest; the lifecycle prints its own closing line.
    grep -E '^(Ran |OK|FAILED)|^(FAIL|ERROR):|skipped|^gui-lifecycle: (every step passed|FAILED)' "$out/$suite.log" \
      | head -20
  } >>"$out/summary.txt"
done
[ -z "$cancelled" ] || code=143
if [ "${#runs[@]}" -gt 0 ]; then
  tar -czf "$out/runs.tar.gz" "${runs[@]}" 2>/dev/null
fi
cat "$out/summary.txt"
exit "$code"
