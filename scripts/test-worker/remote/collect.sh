#!/bin/bash
# Run on the worker through SSH stdin. A finished marker belongs to exactly one invocation and only appears
# after its runner has stopped writing. Return source and workload status on stdout, separately from transport errors.
set -u
base=$1 id=$2
result=$base/out/$id
running=
if [ "$(cat "$base/lock.id" 2>/dev/null)" = "$id" ] && ! flock -n "$base/lock" true; then
  running=1
fi
# Check completion after sampling the lock. It can finish and release the lock between the two reads.
if [ -f "$result/finished" ]; then
  source_commit=$(cat "$result/source") && code=$(cat "$result/finished") || exit 1
  printf '%s\t%s\n' "$source_commit" "$code"
elif [ -n "$running" ]; then
  echo "test-worker: invocation $id is still running" >&2
  exit 75
else
  echo "test-worker: no finished evidence for $id (unknown or pruned invocation)" >&2
  exit 1
fi
