#!/bin/bash
# Run one command from the worker's synced tree with the worker's environment (rustup's cargo on PATH, the pinned
# nextest too, the user manager's bus): a clippy, a narrow test selection through the recorder, a script's self-test.
#
# Usage: run-run.sh <base> <out> <command> [arg...]. Writes summary.txt and console.log in the invocation's output.
set -u
# Stay alive while a foreground recorder handles TERM and captures its child's last evidence.
cancelled=
trap 'cancelled=1' TERM
base=$(cd "$1" && pwd) || exit 1
out=$2
shift 2
[ $# -gt 0 ] || { echo "run-run.sh: no command given" >&2; exit 2; }
# shellcheck source=SCRIPTDIR/env.sh
. "$(dirname "$0")/env.sh"
cd "$base/src" || exit 1
use_pinned_toolchain
nextest_dir=$(python3 scripts/install-pinned-nextest.py 2>/dev/null) && export PATH="$nextest_dir:$PATH"
[ -z "$cancelled" ] || exit 143
"$@" >"$out/console.log" 2>&1
code=$?
[ -z "$cancelled" ] || code=143
{
  echo "command: $*"
  echo "arch: $(uname -m)"
  echo "exit: $code"
  grep -oE 'test-run evidence: [^ ]+' "$out/console.log" | tail -1
  tail -n 30 "$out/console.log"
} >"$out/summary.txt"
cat "$out/summary.txt"
exit "$code"
