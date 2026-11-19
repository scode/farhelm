#!/bin/bash
# Run Farhelm's slow test suites on one SSH test worker: a Linux machine of the maintainer's own (any architecture
# Farhelm builds for), reached over plain ssh or `tailscale ssh`, which builds and tests the checkout's working copy
# itself. `.agents/test-worker.md` says when to use it and what it does; this header is the command reference.
#
# The worker is named in a local file, never in the repository: `~/.farhelm-test-worker` (or `--config FILE`), with
# `key=value` lines:
#
#   destination=user@host      required; an ordinary ssh destination
#   transport=tailscale        optional; `tailscale` (tailscale ssh) or `ssh` (the default)
#
# Commands (each takes --checkout DIR, default the checkout holding this script, and --config FILE):
#
#   bootstrap                  install what the suites need on the worker (idempotent). rust, gui and run do this
#                              themselves first when the worker is fresh or remote/bootstrap.sh changed
#   rust --out DIR [-- ARG...] sync the working copy, then run the Rust workspace suite there
#                              (`cargo nextest run --workspace --exclude farhelm-desktop ARG...`) through the recorder
#   gui --out DIR [SUITE...]   sync, build the GPUI app there, and run `smoke` and/or `lifecycle` (both by default)
#   run --out DIR -- CMD...    sync, then run CMD from the synced tree (a clippy, a narrow nextest selection)
#   status                     show the worker's lock holder and the last runs' summaries
#   stop                       end the run holding the lock (interrupting this script does not end the remote run)
#
# Each run writes a summary and console logs under `<out>`, and, when it fails, the recorder's retained run
# directories as `<out>/runs.tar.gz`. Exit status is the remote command's.
#
# The worker keeps one tree per checkout name under `~/farhelm-test-worker/<checkout>/` with its own `target/`, so
# builds stay incremental across runs. A lock there lets only one run use that tree at a time; a second run fails at
# once rather than waiting.
set -u

die() {
  echo "test-worker: $*" >&2
  exit 2
}

# rsync's remote shell. rsync splits `user@host:` and calls its shell as `<shell> -l user host command...`, which
# `tailscale ssh` refuses (it takes no `-l`), so for that transport rsync calls this script back in this mode, which
# puts the user back in front of the host.
if [ "${1:-}" = __tailscale_rsh ]; then
  shift
  user=
  if [ "${1:-}" = -l ]; then
    user="$2@"
    shift 2
  fi
  host=$1
  shift
  exec tailscale ssh "$user$host" -o BatchMode=yes -o ConnectTimeout=15 "$@"
fi

here=$(cd "$(dirname "$0")" && pwd) || die "cannot find the script's directory"
checkout=$(cd "$here/../.." && pwd) || die "cannot find the checkout"
config="$HOME/.farhelm-test-worker"
out=
cmd=${1:-}
[ -n "$cmd" ] || die "usage: test-worker.sh bootstrap|rust|gui|run|status [options]"
shift
args=()
while [ $# -gt 0 ]; do
  case "$1" in
  --checkout) checkout=$2; shift 2 ;;
  --config) config=$2; shift 2 ;;
  --out) out=$2; shift 2 ;;
  --) shift; args+=("$@"); break ;;
  *) args+=("$1"); shift ;;
  esac
done

# ---- Configuration and transport ------------------------------------------------------------------------------------

[ -f "$config" ] || die "no worker configured: create $config with a destination=user@host line"
destination=$(sed -n 's/^destination=//p' "$config" | tail -1)
transport=$(sed -n 's/^transport=//p' "$config" | tail -1)
[ -n "$destination" ] || die "$config has no destination= line"
case "$destination" in -*) die "the destination must not start with '-'" ;; esac
case "${transport:-ssh}" in
tailscale)
  ssh_cmd=(tailscale ssh "$destination" -o BatchMode=yes -o ConnectTimeout=15)
  rsh="$(printf '%q' "$here/test-worker.sh") __tailscale_rsh"
  ;;
ssh) ssh_cmd=(ssh -o BatchMode=yes -o ConnectTimeout=15 "$destination") rsh='ssh -o BatchMode=yes' ;;
*) die "transport must be tailscale or ssh, not '$transport'" ;;
esac

name=$(basename "$checkout")
base="farhelm-test-worker/$name"

# Run a command line on the worker. The worker's login shell gets it as one string, so every word is quoted here.
remote() {
  "${ssh_cmd[@]}" "$(printf '%q ' "$@")"
}

# Send the harness's own worker scripts. They live outside the synced tree so that a run never executes a script the
# working copy under test has changed.
push_tools() {
  remote mkdir -p "$base/tools" "$base/src" "$base/out" || return 1
  rsync -a --delete -e "$rsh" "$here/remote/" "$destination:$base/tools/"
}

# Mirror the working copy's tracked files (jj's view of `@`, so a jj checkout's uncommitted edits are included) into
# the worker's tree. Only tracked files are sent: build output, package caches and ignored files never cross. rsync's
# `--files-from` cannot delete, so the worker keeps the list it last received and `prune.py` removes the files that
# list named and this one does not, which leaves the worker's own build output (`target/`) alone.
sync_tree() {
  local list source
  list=$(mktemp) || return 1
  if [ -d "$checkout/.jj" ]; then
    jj -R "$checkout" file list -r @ >"$list" || { rm -f "$list"; return 1; }
    source=$(jj -R "$checkout" log -r @ --no-graph -T commit_id) || { rm -f "$list"; return 1; }
  else
    git -C "$checkout" ls-files >"$list" || { rm -f "$list"; return 1; }
    source=$(git -C "$checkout" rev-parse HEAD)-plus-working-tree || { rm -f "$list"; return 1; }
  fi
  local code=0
  rsync -a --files-from="$list" -e "$rsh" "$checkout/" "$destination:$base/src/" >/dev/null || code=1
  if [ "$code" = 0 ]; then
    remote python3 "$base/tools/prune.py" "$base" <"$list" || code=1
  fi
  # The recorder needs a git checkout to record from; snapshot.sh says why the worker's tree gets its own commits.
  if [ "$code" = 0 ]; then
    remote bash "$base/tools/snapshot.sh" "$base" "$source" || code=1
  fi
  rm -f "$list"
  return "$code"
}

# Copy a run's outputs back: its summary and logs, and the retained runs when it failed.
fetch_out() {
  local run=$1
  [ -n "$out" ] || return 0
  mkdir -p "$out" || return 1
  rsync -a -e "$rsh" "$destination:$base/out/$run/" "$out/" >/dev/null
}

# Bootstrap first when the worker has never been bootstrapped with this version of the bootstrap script: a fresh or
# reset worker has no marker, and a changed script needs its new steps. A finished bootstrap is only a hash compare.
ensure_bootstrapped() {
  local want have
  want=$(sha256sum "$here/remote/bootstrap.sh" | cut -d' ' -f1) || return 1
  have=$(remote cat farhelm-test-worker/bootstrapped 2>/dev/null)
  [ "$have" = "$want" ] && return 0
  echo "test-worker: bootstrapping $destination first (fresh worker or changed bootstrap)" >&2
  remote bash "$base/tools/bootstrap.sh" >&2
}

need_out() {
  [ -n "$out" ] || die "$cmd needs --out DIR"
}

# ---- Commands -------------------------------------------------------------------------------------------------------

case "$cmd" in
bootstrap)
  push_tools || die "could not reach $destination or copy the worker scripts"
  remote bash "$base/tools/bootstrap.sh"
  ;;
rust | gui | run)
  need_out
  push_tools || die "could not reach $destination or copy the worker scripts"
  ensure_bootstrapped || die "could not bootstrap $destination (run the bootstrap command to see why)"
  sync_tree || die "could not sync the working copy to $destination"
  remote bash "$base/tools/with-lock.sh" "$base" "run-$cmd.sh" "${args[@]}"
  code=$?
  fetch_out "$cmd"
  cat "$out/summary.txt" 2>/dev/null
  exit "$code"
  ;;
stop)
  remote bash "$base/tools/stop.sh" "$base"
  ;;
status)
  remote bash -c "cat $(printf '%q' "$base")/lock.holder 2>/dev/null; tail -n 5 $(printf '%q' "$base")/out/*/summary.txt 2>/dev/null"
  ;;
*)
  die "unknown command $cmd"
  ;;
esac
