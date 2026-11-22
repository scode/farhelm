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
#   collect --out DIR ID       retrieve one finished run's evidence
#   status                     show the checkout's lock holder and recent run summaries
#   stop                       end the run holding the lock (interrupting this script does not end the remote run)
#
# Each run prints an invocation id. The driver stamps `<out>/invocation.txt` and clears the harness's old artifacts
# there, then copies that invocation's summary, logs and retained failure evidence. A refused run has no evidence
# to copy. The worker keeps its 20 newest runs per checkout for later collection.
#
# The worker keeps one tree per checkout name under `~/farhelm-test-worker/<checkout>/` with its own `target/`, so
# builds stay incremental. An invocation stages its own tools and source and only installs the source under the tree
# lock. Bootstrap has exclusive use of the worker while it changes prerequisites shared by every checkout.
set -uo pipefail

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

output_locked=
if [ "${1:-}" = __output_locked ]; then
  shift
  output_locked=1
fi
original_args=("$@")

here=$(cd "$(dirname "$0")" && pwd) || die "cannot find the script's directory"
checkout=$(cd "$here/../.." && pwd) || die "cannot find the checkout"
config="$HOME/.farhelm-test-worker"
out=
cmd=${1:-}
[ -n "$cmd" ] || die "usage: test-worker.sh bootstrap|rust|gui|run|collect|status|stop [options]"
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

# Keep the stamp and the files it describes together if two clients reuse an output directory. The wrapper
# execs this script with the lease on fd 9, before either client touches any of the harness's artifact names.
case "$cmd" in
rust | gui | run | collect)
  [ -n "$out" ] || die "$cmd needs --out DIR"
  if [ -z "$output_locked" ]; then
    exec python3 "$here/lock-output.py" "$out" "$0" "${original_args[@]}"
  fi
  ;;
esac

invocation=
source=unknown
case "$cmd" in
bootstrap | rust | gui | run)
  invocation=$(python3 -c 'import uuid; print(uuid.uuid4())') || die "could not generate an invocation id"
  ;;
collect)
  if [ -z "${args[0]:-}" ] || [ -n "${args[1]+extra}" ]; then
    die "collect needs an invocation id"
  fi
  invocation=${args[0]}
  [[ "$invocation" =~ ^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$ ]] \
    || die "invalid invocation id"
  ;;
esac
if [ -n "$invocation" ]; then
  printf 'test-worker: invocation %s\n' "$invocation" >&2
fi

stamp_out() {
  printf 'invocation: %s\nsource commit: %s\noutcome: %s\n' "$invocation" "$source" "$1" >"$out/invocation.txt"
}

# A reused directory can contain an archive from a previous failed run. Clear only names owned by the harness.
case "$cmd" in
rust | gui | run | collect)
  rm -f -- "$out/"{invocation.txt,summary.txt,console.log,smoke.log,lifecycle.log,runs.tar.gz} \
    || die "could not clear earlier evidence in $out"
  stamp_out preparing || die "could not stamp evidence directory $out"
  ;;
esac

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
stage="$base/stage/$invocation"
result="$base/out/$invocation"

# Run a command line on the worker. The worker's login shell gets it as one string, so every word is quoted here.
remote() {
  # A disconnected client's transport must not keep its local output directory locked.
  "${ssh_cmd[@]}" "$(printf '%q ' "$@")" 9>&-
}

# Compose the source stamp from the local checkout before staging. `jj file list` includes working-copy changes the
# snapshot of `@` sees. The Git path mirrors the tracked files on disk and labels its potentially dirty source.
source_list() {
  if [ -d "$checkout/.jj" ]; then
    jj -R "$checkout" file list -r @ >"$list" || return 1
    source=$(jj -R "$checkout" log -r @ --no-graph -T commit_id) || return 1
  else
    git -C "$checkout" ls-files >"$list" || return 1
    source=$(git -C "$checkout" rev-parse HEAD)-plus-working-tree || return 1
  fi
}

list=
owns_stage=
cleanup() {
  [ -z "$list" ] || rm -f -- "$list"
  # Ownership passes to the detached controller before starting SSH. A lost transport at that point leaves a stage
  # for the worker to reap. The driver's exit must never delete a stage a run may already be using.
  [ -z "$owns_stage" ] || remote rm -rf -- "$stage" >/dev/null 2>&1
}

stage_tools() {
  # Create the lease from the start, so opening it for a later sweep cannot refresh a stale stage's mtime.
  # shellcheck disable=SC2016 # The worker's Bash expands its own arguments.
  remote bash -c 'mkdir -p -- "$1" && mkdir -- "$2" && touch -- "$2/.lock"' bash "$base/stage" "$stage" || return 1
  owns_stage=1
  # Refresh the stage age before unlocking. A long transfer must leave time to start the next one.
  # shellcheck disable=SC2016 # The worker's Bash expands its own arguments.
  stage_rsync=$(printf '%q ' flock "$stage/.lock" bash -c \
    'stage=$1; shift; trap '\''touch -- "$stage"'\'' EXIT; rsync "$@"' rsync "$stage")
  rsync -a -e "$rsh" --rsync-path="$stage_rsync" "$here/remote/" "$destination:$stage/tools/" 9>&-
}

# Upload only tracked bytes into the private stage. Reading a shared checkout as rsync's alternate source here would
# let an overlapping install change the bytes used for this request. The locked install still preserves mtimes.
stage_source() {
  rsync -a --files-from="$list" -e "$rsh" --rsync-path="$stage_rsync" \
    "$checkout/" "$destination:$stage/src/" >/dev/null 9>&- \
    && rsync -a -e "$rsh" --rsync-path="$stage_rsync" "$list" "$destination:$stage/files" 9>&-
}

# The worker publishes this marker after the runner has stopped writing. The relative path carries the invocation id
# even if a different run wins the lock before this copy starts.
fetch_out() {
  rsync -a -e "$rsh" --exclude '/source' --exclude '/finished' "$destination:$result/" "$out/" >/dev/null 9>&-
}

copy_failed() {
  stamp_out "copy failed (command exit $1)" || exit 1
  printf 'test-worker: could not copy evidence to %s (command exit %s)\n' "$out" "$1" >&2
  [ "$1" != 0 ] || exit 1
  exit "$1"
}

# ---- Commands -------------------------------------------------------------------------------------------------------

case "$cmd" in
bootstrap | rust | gui | run)
  trap cleanup EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  if [ "$cmd" != bootstrap ]; then
    list=$(mktemp) || die "could not create a source list"
    source_list || die "could not identify the source checkout"
    stamp_out staging || die "could not stamp evidence directory $out"
  fi
  stage_tools || die "could not reach $destination or stage the worker scripts"
  if [ "$cmd" != bootstrap ]; then
    stage_source || die "could not stage the working copy on $destination"
  fi
  owns_stage=
  remote bash "$stage/tools/with-lock.sh" "$base" "$invocation" "$cmd" "$source" ${args[@]+"${args[@]}"}
  code=$?
  [ "$cmd" != bootstrap ] || exit "$code"
  if remote test -f "$result/finished"; then
    fetch_out || copy_failed "$code"
  elif [ "$?" != 1 ] || [ "$code" = 0 ]; then
    copy_failed "$code"
  fi
  stamp_out "exit $code" || exit 1
  exit "$code"
  ;;
collect)
  source=$(remote bash -s -- "$base" "$invocation" <"$here/remote/collect.sh")
  code=$?
  if [ "$code" != 0 ]; then
    source=unknown
    stamp_out "collection refused (exit $code)" || exit 1
    exit "$code"
  fi
  if ! fetch_out; then
    stamp_out "collection copy failed" || exit 1
    echo "test-worker: could not copy evidence for $invocation to $out" >&2
    exit 1
  fi
  stamp_out collected || die "could not stamp evidence directory $out"
  ;;
stop)
  remote bash -s -- "$base" <"$here/remote/stop.sh"
  ;;
status)
  remote bash -s -- "$base" <"$here/remote/status.sh"
  ;;
*)
  die "unknown command $cmd"
  ;;
esac
