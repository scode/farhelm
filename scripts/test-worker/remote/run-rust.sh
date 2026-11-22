#!/bin/bash
# Run the Rust workspace suite in the worker's synced tree, built there, through the recorder's nextest mode (four
# slots, no retries), exactly as `.agents/validation.md` runs it locally. Extra arguments go to `cargo nextest run`
# after the workspace selection (a filter expression, `-E ...`, or a narrower package selection's extra flags).
#
# Usage: run-rust.sh <base> <out> [nextest arg...]. Writes summary.txt, console.log, and runs.tar.gz on failure.
set -u
# The recorder owns a separate command session. A TERM must leave this shell waiting until it has stopped.
cancelled=
trap 'cancelled=1' TERM
base=$(cd "$1" && pwd) || exit 1
out=$2
shift 2
# shellcheck source=SCRIPTDIR/env.sh
. "$(dirname "$0")/env.sh"
cd "$base/src" || exit 1
use_pinned_toolchain
substrate >"$out/summary.txt"
keep_disk_free >>"$out/summary.txt"
nextest_dir=$(python3 scripts/install-pinned-nextest.py 2>>"$out/console.log") || {
  echo "setup failed: could not install the pinned nextest" >>"$out/summary.txt"
  cat "$out/summary.txt"
  exit 1
}
export PATH="$nextest_dir:$PATH"
[ -z "$cancelled" ] || exit 143
selection="workspace Rust targets on the test worker ($(uname -m))"
[ $# -gt 0 ] && selection="$selection, narrowed: $*"
python3 scripts/record-test-run.py --runner nextest --kind development --selection "$selection" \
  --concurrency '4 nextest slots; retries 0' -- cargo nextest run --workspace --exclude farhelm-desktop "$@" \
  >>"$out/console.log" 2>&1
code=$?
[ -z "$cancelled" ] || code=143
run_dir=$(grep -oE 'test-run evidence: [^ ]+' "$out/console.log" | tail -1 | cut -d' ' -f3)
{
  echo "exit: $code"
  echo "run: ${run_dir:-unknown}"
  grep -E '^\s+Summary |^\s+FAIL ' "$out/console.log" | sort -u | head -60
  # A test that returns early without its substrate prints SKIPPED and passes, so the retained output is the evidence.
  if [ -n "${run_dir:-}" ] && [ -d "$run_dir" ]; then
    echo "runtime SKIPPED lines in the retained output: $(grep -rh 'SKIPPED' "$run_dir" 2>/dev/null | wc -l)"
    grep -rhoa 'SKIPPED.\{0,140\}' "$run_dir" 2>/dev/null | sort | uniq -c | sort -rn | head -20
  else
    echo "runtime SKIPPED lines: UNKNOWN (no retained run directory)"
  fi
} >>"$out/summary.txt"
if [ "$code" != 0 ] && [ -n "${run_dir:-}" ] && [ -d "$run_dir" ]; then
  tar -czf "$out/runs.tar.gz" -C "$(dirname "$run_dir")" "$(basename "$run_dir")" 2>/dev/null
fi
cat "$out/summary.txt"
exit "$code"
