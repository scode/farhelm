#!/bin/bash
# Run through SSH stdin so status never stages or changes a tool the current run might use.
set -u
base=$1
if [ -f "$base/lock" ] && ! flock -n "$base/lock" true; then
  cat "$base/lock.holder" 2>/dev/null
fi
[ -d "$base/out" ] || exit 0
while IFS= read -r path; do
  printf '\n%s\n' "$(basename "$(dirname "$path")")"
  tail -n 5 "$path"
done < <(find "$base/out" -mindepth 2 -maxdepth 2 -name summary.txt -printf '%T@ %p\n' \
  | sort -nr | head -n 5 | cut -d' ' -f2-)
