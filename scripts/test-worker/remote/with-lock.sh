#!/bin/bash
# The controller owns both locks until the detached run finishes. The SSH-side process only waits and reads the
# report. Losing the connection cannot end the controller or leave the workload holding a shared lock descriptor.
#
# Usage: with-lock.sh <tree> <id> <bootstrap|rust|gui|run> <source> [arg...], from the worker's home directory.
set -uo pipefail

if [ "$1" != --detached ]; then
  stage=$1/stage/$2
  # Open the report before starting. The controller can remove the stage even when SSH disconnects. This reader
  # can still print the report after the last path to it disappears.
  : >"$stage/controller.log" && exec 3<"$stage/controller.log" || exit 1
  setsid --wait bash "$0" --detached "$@" </dev/null >"$stage/controller.log" 2>&1 3<&- &
  controller=$!
  wait "$controller"
  code=$?
  cat <&3
  exit "$code"
fi
shift

base=$(cd "$1" && pwd) || exit 1
id=$2 cmd=$3 source_commit=$4
shift 4
worker=$(dirname "$base")
stage=$base/stage/$id
tools=$stage/tools
holder=$worker/holders/$id
tree_held=
worker_shared=

# The detached shell, rather than the workload or its descendants, owns the stage (7), bootstrap (8),
# checkout (9) and worker (6) leases. Descendants may deliberately outlive a test.
run_without_locks() {
  "$@" 6>&- 7>&- 8>&- 9>&-
}

# shellcheck disable=SC2317 # Invoked by the EXIT trap.
finish() {
  if [ -n "$worker_shared" ] && flock 8; then
    rm -f -- "$holder"
    flock -u 6
    flock -u 8
  fi
  if [ -n "$tree_held" ]; then
    rm -f -- "$base/lock.holder" "$base/lock.id"
  fi
  rm -rf -- "$stage"
}

exec 7>"$stage/.lock" && flock 7 || exit 1
trap finish EXIT

name_holder() {
  local receipt tree running
  for receipt in "$worker/holders/"*; do
    [ -f "$receipt" ] || continue
    IFS= read -r tree <"$receipt" || continue
    running=$(cat "$tree/lock.id" 2>/dev/null) || continue
    if [ "$running" = "${receipt##*/}" ] && ! flock -n "$tree/lock" true; then
      cat "$tree/lock.holder"
    else
      rm -f -- "$receipt"
    fi
  done
}

# Serialize bootstrap callers, including automatic setup on a new worker. While the bootstrap lock is held
# no run may acquire or release a shared worker lease, so a refused bootstrap can name active run receipts.
admit_worker() {
  local want have
  mkdir -p "$worker/holders" && exec 8>"$worker/bootstrap.lock" 6>"$worker/active.lock" || return 1
  if [ "$cmd" = bootstrap ]; then
    flock 8 || return 1
  elif ! flock -w 300 8; then
    echo "test-worker: timed out waiting for worker bootstrap; check status or try again" >&2
    return 75
  fi
  [ ! -e "$stage/stop" ] || return 143
  want=$(sha256sum "$tools/bootstrap.sh" | cut -d' ' -f1) || return 1
  have=$(cat "$worker/bootstrapped" 2>/dev/null)
  if [ "$cmd" = bootstrap ] || [ "$have" != "$want" ]; then
    if ! flock -n -x 6; then
      echo "test-worker: cannot bootstrap while the worker is running tests; use status or stop on the listed checkout" >&2
      name_holder >&2
      return 75
    fi
    run_without_locks bash "$tools/bootstrap.sh" || return "$?"
    [ "$cmd" != bootstrap ] || return 0
    flock -u 6 || return 1
  fi
  flock -s 6 || return 1
  worker_shared=1
  printf '%s\n' "$base" >"$holder" && flock -u 8
}

# A day-old stage can be left by an interrupted upload. Each transfer and the detached controller hold the
# stage's lock, so a slow transfer or a controller waiting for bootstrap is never removed mid-operation.
sweep_stale_stages() {
  local abandoned age=+1440
  while IFS= read -r -d '' abandoned; do
    [ "$abandoned" != "$stage" ] || continue
    # A transfer may refresh the age after find lists the directory and before flock acquires the stage.
    # shellcheck disable=SC2016 # The lease holder checks the same path and age inside its Bash process.
    flock -n "$abandoned/.lock" bash -c '
      [ -z "$(find "$1" -maxdepth 0 -mmin "$2" -print -quit)" ] || rm -rf -- "$1"
    ' bash "$abandoned" "$age" || continue
  done < <(find "$base/stage" -mindepth 1 -maxdepth 1 -type d -mmin "$age" -print0)
}

prune_evidence() {
  local old count=1
  while read -r _ old; do
    [[ "$old" =~ ^[0-9a-f]{8}-[0-9a-f-]{27}$ ]] || continue
    [ "$old" != "$id" ] || continue
    count=$((count + 1))
    [ "$count" -le 20 ] || rm -rf -- "$base/out/$old"
  done < <(find "$base/out" -mindepth 1 -maxdepth 1 -type d -printf '%T@ %f\n' | sort -nr)
}

if [ "$cmd" = bootstrap ]; then
  admit_worker
  exit "$?"
fi

exec 9>"$base/lock" || exit 1
if ! flock -n 9; then
  echo "test-worker: $base is in use by: $(cat "$base/lock.holder" 2>/dev/null || echo unknown)" >&2
  exit 75
fi
tree_held=1
printf '%s\n' "$id" >"$base/lock.id" \
  && printf '%s\n' "${base##*/} $cmd $id (since $(date -u +%Y-%m-%dT%H:%M:%SZ))" >"$base/lock.holder" || exit 1
admit_worker || exit "$?"
[ ! -e "$stage/stop" ] || exit 143
sweep_stale_stages
[ ! -e "$stage/stop" ] || exit 143

# Install the staged tracked files without deleting ignored state and build output. A same-size edit within the
# mtime resolution must still reach the recorder's snapshot. Unchanged files keep their original mtimes.
mkdir -p "$base/src" && run_without_locks rsync -a --checksum "$stage/src/" "$base/src/" || exit 1
[ ! -e "$stage/stop" ] || exit 143
run_without_locks python3 "$tools/prune.py" "$base" <"$stage/files" \
  && run_without_locks bash "$tools/snapshot.sh" "$base" "$source_commit" || exit 1
rm -rf -- "$stage/src"
[ ! -e "$stage/stop" ] || exit 143

out=$base/out/$id
mkdir -p "$base/out" && mkdir "$out" && printf '%s\n' "$source_commit" >"$out/source" || exit 1
# The workload watcher keeps its group's leader waitable through Stop, even if a shell exits early.
# The checkout and worker leases stay here, outside both the watcher and the group it can signal.
code=0
run_without_locks python3 "$tools/run-workload.py" "$stage" bash "$tools/run-$cmd.sh" "$base" "$out" "$@" || code=$?
if [ -e "$stage/running" ]; then
  prune_evidence
elif [ "$code" -eq 143 ]; then
  # Stop reached the watcher before spawn. Drop this invocation's empty reservation.
  rm -- "$out/source" && rmdir -- "$out" || exit 1
  exit "$code"
fi
# Publish completion atomically. Collect treats the marker as a promise that all evidence writes ended.
printf '%s\n' "$code" >"$out/.finished" && mv "$out/.finished" "$out/finished" || exit 1
exit "$code"
