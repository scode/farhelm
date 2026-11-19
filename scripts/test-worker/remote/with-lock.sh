#!/bin/bash
# Run one of the worker scripts while holding this checkout tree's lock, so two runs never build or test in the same
# tree at once. A second run fails at once, naming the holder, rather than queueing behind a run that may take an hour.
#
# The run gets a process group of its own, recorded in `<base>/lock.pgid`, because ending the ssh session that started
# it does not end it: `stop.sh` ends that group (the `stop` command), and nothing else may.
#
# Usage: with-lock.sh <base> <script> [arg...], from the worker's home directory; <script> is a file in <base>/tools.
set -u
base=$1 script=$2
shift 2
exec 9>"$base/lock" || exit 1
if ! flock -n 9; then
  echo "test-worker: $base is in use by: $(cat "$base/lock.holder" 2>/dev/null || echo unknown)" >&2
  exit 75
fi
echo "$script $* (since $(date -u +%Y-%m-%dT%H:%M:%SZ))" >"$base/lock.holder"
setsid bash "$base/tools/$script" "$base" "$@" &
child=$!
echo "$child" >"$base/lock.pgid"
wait "$child"
code=$?
rm -f "$base/lock.holder" "$base/lock.pgid"
exit "$code"
