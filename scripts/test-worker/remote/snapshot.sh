#!/bin/bash
# Make the synced tree a one-commit-per-sync git checkout, after each sync. The recorder runs only from a checkout root
# (`git rev-parse --show-toplevel`) and records the tested revision from it, as the boat harness's one-commit clone
# allows there. The commit names the development machine's commit it mirrors, so the evidence can be traced back; its
# own id means nothing outside the worker. Ignored files (the build output) stay out of it through the tree's own
# `.gitignore`.
#
# Usage: snapshot.sh <base> <source commit id>
set -u
src=$1/src
source_commit=$2
git -C "$src" rev-parse --git-dir >/dev/null 2>&1 || git -C "$src" init -q || exit 1
git -C "$src" add -A || exit 1
git -C "$src" -c user.name=test-worker -c user.email=test-worker@localhost commit -q --allow-empty \
  -m "test-worker sync of $source_commit" || exit 1
echo "$source_commit" >"$1/source.txt"
