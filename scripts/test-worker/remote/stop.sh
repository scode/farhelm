#!/bin/bash
# Record a Stop against the current invocation. Its workload watcher owns all process-group signals.
# Usage: stop.sh <base>, via SSH stdin.
set -u
base=$1

idle() {
  echo "nothing is running in $base"
  exit 0
}
if [ ! -f "$base/lock" ] || flock -n "$base/lock" true; then
  idle
fi
id=$(cat "$base/lock.id" 2>/dev/null) || idle
[[ "$id" =~ ^[0-9a-f]{8}-[0-9a-f-]{27}$ ]] || idle
stage=$base/stage/$id
# The stage is unique to the observed invocation, even if a new one takes the tree lock in the meantime.
touch -- "$stage/stop" 2>/dev/null || idle
if [ ! -e "$stage/running" ]; then
  echo "stop requested for $id during preparation"
  exit 0
fi
# The workload watcher allows 35 seconds for graceful shutdown. Leave it time to finish ownership cleanup.
for _ in $(seq 40); do
  if [ "$(cat "$base/lock.id" 2>/dev/null)" != "$id" ]; then
    echo "invocation $id ended"
    exit 0
  fi
  sleep 1
done
echo "test-worker: invocation $id is still stopping; use status to check its progress" >&2
exit 1
