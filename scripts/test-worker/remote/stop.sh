#!/bin/bash
# End the run holding this tree's lock: its whole process group, with SIGTERM and then SIGKILL after 30 seconds (the
# recorder stops its child and keeps its evidence on SIGTERM). Only the group with-lock.sh recorded is touched.
#
# Usage: stop.sh <base>
set -u
base=$1
pgid=$(cat "$base/lock.pgid" 2>/dev/null) || { echo "nothing is running in $base"; exit 0; }
case "$pgid" in '' | *[!0-9]*) echo "unreadable process group in $base/lock.pgid"; exit 1 ;; esac
kill -TERM -- "-$pgid" 2>/dev/null || { echo "process group $pgid is already gone"; exit 0; }
for _ in $(seq 30); do
  kill -0 -- "-$pgid" 2>/dev/null || { echo "stopped process group $pgid"; exit 0; }
  sleep 1
done
kill -KILL -- "-$pgid" 2>/dev/null
echo "killed process group $pgid"
