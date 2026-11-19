#!/bin/bash
# Build the GPUI app in the worker's synced tree and run its end-to-end suites there, each through the recorder, one
# after the other: each starts its own headless Mutter and app, and both render with lavapipe, which is CPU-bound. The
# first suite builds the app the way `scripts/gui-agent.py start` does; the second reuses that build.
#
# Usage: run-gui.sh <base> [smoke] [lifecycle] (both when none is named). Writes <base>/out/gui/summary.txt, one console
# log per suite, and runs.tar.gz on failure.
set -u
base=$(cd "$1" && pwd) || exit 1
shift
# shellcheck source=SCRIPTDIR/env.sh
. "$base/tools/env.sh"
out=$base/out/gui
rm -rf "$out" && mkdir -p "$out" || exit 1
cd "$base/src" || exit 1
use_pinned_toolchain
[ $# -gt 0 ] || set -- smoke lifecycle
{
  substrate
  keep_disk_free
  echo "mutter: $(mutter --version 2>&1 | head -1)"
  echo "mesa: $(dpkg-query -W -f='${Version}' mesa-vulkan-drivers 2>&1)"
} >"$out/summary.txt"
code=0 built='' runs=()
for suite in "$@"; do
  case "$suite" in
  smoke)
    script=scripts/gui-e2e-smoke.py selection='GPUI app end-to-end smoke'
    concurrency='one app run, steps in order; one step starts and stops a second run'
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
if [ "${#runs[@]}" -gt 0 ]; then
  tar -czf "$out/runs.tar.gz" "${runs[@]}" 2>/dev/null
fi
cat "$out/summary.txt"
exit "$code"
