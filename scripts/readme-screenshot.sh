#!/usr/bin/env bash
# Produce the README hero screenshot (docs/readme-hero/SPEC.md).
#
# One command, so that "refresh the README screenshot" is a thing an agent
# can be told without it inventing a procedure. It stages the scenario's
# fleet in the desktop app's agent build through scripts/gui-agent.py
# (`start --scenario`), takes the picture with its `capture` verb, stops
# the run, and prints where the PNG landed. It never publishes: look at the
# picture, then run scripts/publish-readme-hero.sh on it.
#
# Usage:
#   scripts/readme-screenshot.sh --mac DEST [--output PATH] [--no-build]
#   scripts/readme-screenshot.sh --local [--output PATH] [--no-build] [--lock FILE]
#
#   --local        capture on this machine (the default unless
#                  FARHELM_MEDIA_MAC names a Mac): on Linux on a private
#                  headless Mutter at the scenario's scale, on a Mac on its
#                  real screen.
#   --mac DEST     capture on a Mac reached over ssh at DEST (the default
#                  when FARHELM_MEDIA_MAC names one). The checkout's files
#                  (those git would not ignore) are shipped to it, built
#                  there and run under its lock.
#   --output PATH  where to write the PNG (default
#                  target/readme-hero/readme-hero.png). Removed first, and
#                  written only when every check passed.
#   --no-build     reuse the binaries already built (on the Mac, in its
#                  build directory).
#   --lock FILE    (local) hold this lock from before the build until the
#                  run stops, for a machine whose one GUI session several
#                  runs share.
#
# The operator's settings for --mac, from the environment, so that this
# file names no machine:
#   FARHELM_MEDIA_MAC        the Mac's ssh destination, when --mac is not given
#   FARHELM_MEDIA_SSH        the command that reaches it (default: ssh), split
#                            on spaces; rsync runs it too, so it must take
#                            ssh's options (`tailscale ssh` does not)
#   FARHELM_MEDIA_MAC_DIR    where under the Mac's home the shipped sources
#                            live (default: farhelm-media); kept between runs,
#                            with their own target/, so a build there is
#                            incremental
#   FARHELM_MEDIA_MAC_LOCK   the Mac-wide lock file under its home that every
#                            build and window there takes (default:
#                            .farhelm-gpui-mac.lock)
#
# What the Mac needs: the user ssh logs in as is the one logged in at its
# console (the app opens a real window there), a Rust toolchain in
# ~/.cargo/bin, Python 3.9 or newer, OpenSSH (sshd, ssh, ssh-keygen) and
# rsync, at least 15 GB of free disk and 4 GB of free memory (checked first,
# under the lock), and a display of at least the scenario's viewport plus
# the menu bar and title bar: the app fits its first window to the screen,
# and the capture refuses a picture that came out smaller. This side needs
# rsync and the ssh command above.
#
# Nothing here needs passwordless ssh to the capturing machine itself: a
# staged run reaches its staged hosts through a private sshd of its own
# (docs/gui-agent.md, "The private sshd").
#
# NOTE: no reliance on `set -e` (inert under some harnesses); every step
# carries its own guard. Bash 3.2-safe, because on a Mac this same file runs
# the --local half under whatever bash is first on PATH.

repo="$(cd "$(dirname "$0")/.." && pwd)" || exit 1
output="$repo/target/readme-hero/readme-hero.png"
build=true
mode=""
mac=""
lock=""
while [ $# -gt 0 ]; do
  case "$1" in
    --output)
      shift
      test -n "${1:-}" || { echo "--output needs a path" >&2; exit 2; }
      output="$1"
      ;;
    --no-build) build=false ;;
    --local) mode=local ;;
    --mac)
      shift
      test -n "${1:-}" || { echo "--mac needs an ssh destination" >&2; exit 2; }
      mode=mac
      mac="$1"
      ;;
    --lock)
      shift
      test -n "${1:-}" || { echo "--lock needs a file" >&2; exit 2; }
      lock="$1"
      ;;
    -h | --help)
      sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done
case "$output" in
  /*) ;;
  *) output="$PWD/$output" ;;
esac
if [ -z "$mode" ]; then
  mode=local
  [ -z "${FARHELM_MEDIA_MAC:-}" ] || mode=mac
fi
mac="${mac:-${FARHELM_MEDIA_MAC:-}}"

# --- this machine -------------------------------------------------------------------------------------------------
#
# One gui-agent run in a fresh directory of its own: `start --scenario` stages the fleet (and builds, unless told not
# to), `capture` checks and takes the picture, and `stop` runs whatever happened before it, also on a signal. A run that
# did not end cleanly keeps its directory for its logs and the shot `capture` refused; a clean one is removed. Any
# CARGO_TARGET_DIR works: gui-agent.py asks cargo where it builds and runs the binaries from there.

# The scenario's viewport scale, read through the same loader the capture uses, so this script never restates it.
scenario_scale() {
  PYTHONDONTWRITEBYTECODE=1 python3 -I - "$repo/scripts" "$repo/docs/readme-hero/scenario.json5" <<'PY'
import sys
sys.path.insert(0, sys.argv[1])
import scenario_json5
print(scenario_json5.load(sys.argv[2])["viewport"].get("scale", 1))
PY
}

capture_here() {
  local tmp run status started captured stopped
  tmp="$(mktemp -d /tmp/fh-hero.XXXXXX)" || exit 1
  run="$tmp/run"
  agent=(python3 -B "$repo/scripts/gui-agent.py" "$run")
  child=""

  # Each step runs in the background and is waited for, so a signal reaches the trap at once instead of after the step
  # (bash runs a trap only once its foreground command returns), and the trap can end the step before it stops the run.
  step() {
    "$@" &
    child=$!
    wait "$child"
    status=$?
    child=""
    return "$status"
  }
  interrupted() {
    if [ -n "$child" ]; then
      kill "$child" 2>/dev/null
      wait "$child" 2>/dev/null
    fi
    "${agent[@]}" stop >&2
    echo "interrupted; the run directory $run is kept for its logs" >&2
    exit 130
  }
  trap interrupted INT TERM HUP

  # A picture left from an earlier run must not pass for this one's.
  rm -f "$output"
  mkdir -p "$(dirname "$output")" || exit 1
  local start=(start --scenario "$repo/docs/readme-hero/scenario.json5" --appearance dark)
  # Linux renders on the run's private headless Mutter, which takes its output scale from the start command; without
  # the scenario's scale it renders at 1 and the capture's size check refuses the picture. A Mac uses its real screen
  # and refuses --scale.
  if [ "$(uname -s)" = Linux ]; then
    local scale
    scale="$(scenario_scale)" || { echo "cannot read the scale from the hero scenario" >&2; exit 1; }
    start+=(--scale "$scale")
  fi
  [ "$build" = true ] || start+=(--no-build)
  [ -z "$lock" ] || start+=(--lock "$lock")
  step "${agent[@]}" "${start[@]}"
  started=$?
  captured=1
  if [ "$started" -eq 0 ]; then
    step "${agent[@]}" capture "$output"
    captured=$?
  fi
  step "${agent[@]}" stop
  stopped=$?
  trap - INT TERM HUP
  if [ "$captured" -eq 0 ] && [ "$stopped" -eq 0 ]; then
    rm -rf "$tmp"
    echo "$output"
    return 0
  fi
  if [ "$stopped" -ne 0 ]; then
    echo "the run did not stop cleanly; run: python3 $repo/scripts/gui-agent.py $run stop" >&2
  fi
  echo "no picture; the run directory $run is kept for its logs" >&2
  return 1
}

# --- a Mac over ssh -------------------------------------------------------------------------------------------------
#
# Four steps. The first three run under the Mac-wide lock (`lockf -k`, the lock every build and window there takes): a
# preflight that checks the console user, disk and memory; an rsync of the checkout into a directory of its own there;
# the capture, which runs this script's --local half there (whose `start --lock` holds the lock from before the build
# until the run stops). The fourth, fetching the picture, needs no lock. The remote halves are bash scripts sent on
# stdin, and every value they take is checked to be a plain path first. A bash function exported to the locked child is
# the one portable way to put a block of commands under `lockf`.
#
# Each step's status comes back as a line of its output (`checked_script`), never as ssh's exit status: the Tailscale SSH
# server reported 0 for a remote `exit 3` (2026-11-16), and a step that failed there would otherwise read here as one
# that worked. rsync's own status is its protocol's, which holds.
#
# Everything this leaves on the Mac has a name fixed per checkout (`src-<id>`, `hero-<id>.png`, `hero-<id>.log` under
# FARHELM_MEDIA_MAC_DIR), and the capture step clears the picture and log before it starts. A failed run therefore
# leaves at most one picture and one log per checkout on the shared Mac, where the next run replaces them, instead of a
# pair per attempt.

# The preflight: refuse a Mac where the window could not open or the build would not fit, failing closed when a figure
# cannot be read. Argument: the lock, under the Mac's home.
mac_preflight_script() {
  cat <<'EOF'
preflight() {
  console=$(/usr/bin/stat -f %Su /dev/console) || return 1
  me=$(/usr/bin/id -un) || return 1
  free=$(/bin/df -g "$HOME" | /usr/bin/awk 'NR==2 {print $4}')
  mem=$(/usr/bin/vm_stat | /usr/bin/awk '/page size of/ {ps=$8} /Pages free/ {f=$3} /Pages inactive/ {i=$3} END {printf "%d", (f+i)*ps/1073741824}')
  for figure in "$free" "$mem"; do
    case "$figure" in
      "" | *[!0-9]*) echo "could not read the Mac's free disk ('$free') or memory ('$mem')" >&2; return 1 ;;
    esac
  done
  echo "the Mac: console user $console, $free GB of disk and $mem GB of memory free"
  if [ "$console" != "$me" ]; then
    echo "the Mac's console belongs to $console, not $me, so no window of this user can open there" >&2
    return 1
  fi
  if [ "$free" -lt 15 ]; then
    echo "refusing: the Mac has under 15 GB of free disk" >&2
    return 1
  fi
  if [ "$mem" -lt 4 ]; then
    echo "refusing: the Mac has under 4 GB of free memory" >&2
    return 1
  fi
}
export -f preflight
cd "$HOME" || exit 1
/usr/bin/lockf -k "$1" /bin/bash -c preflight
EOF
}

# The capture: clear the previous run's picture and log, which share this run's fixed names, then run the --local half
# with the Mac's toolchain on PATH, its output to a log rather than to ssh, so that a dropped connection (Ctrl-C here, a
# tool timeout, the network) cannot kill it with SIGPIPE before it reaches its own `stop`, which is what releases the
# Mac-wide lock. Then show the log. Arguments: the source directory, the lock, the output and the log, all under the
# Mac's home, and whether to build.
mac_capture_script() {
  cat <<'EOF'
src=$1 lock=$2 out=$3 log=$4 build=$5
/bin/rm -f "$HOME/$out" "$HOME/$log" || exit 1
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
export USER="${USER:-$(/usr/bin/id -un)}"
# A small Mac builds GPUI slowly enough at full width to starve everything else on it (docs/gui-agent.md, "On a Mac").
export CARGO_BUILD_JOBS=6
set -- --local --output "$HOME/$out" --lock "$HOME/$lock"
if [ "$build" != true ]; then
  set -- "$@" --no-build
fi
cd "$HOME/$src" || exit 1
bash scripts/readme-screenshot.sh "$@" </dev/null >"$HOME/$log" 2>&1
status=$?
cat "$HOME/$log"
test "$status" -eq 0
EOF
}

# Whether a value is safe to put on a remote command line unquoted: a relative path of letters, digits, `.`, `_`, `-`
# and `/`, with no `..` component and no leading `-` that a remote command could read as an option.
plain_path() {
  case "$1" in
    "" | /* | -* | *[!A-Za-z0-9._/-]* | .. | ../* | */.. | */../*) return 1 ;;
  esac
  return 0
}

capture_on_mac() {
  local ssh_cmd base lockfile id src out log tmp status=0
  # A picture left from an earlier run must not pass for this one's, whichever step fails.
  rm -f "$output" || exit 1
  case "$mac" in
    "" | -* | *[[:space:]]*) echo "--mac needs an ssh destination such as user@host, not '$mac'" >&2; exit 2 ;;
  esac
  read -r -a ssh_cmd <<<"${FARHELM_MEDIA_SSH:-ssh}"
  base="${FARHELM_MEDIA_MAC_DIR:-farhelm-media}"
  lockfile="${FARHELM_MEDIA_MAC_LOCK:-.farhelm-gpui-mac.lock}"
  plain_path "$base" || { echo "FARHELM_MEDIA_MAC_DIR must be a plain path under the Mac's home" >&2; exit 2; }
  plain_path "$lockfile" || { echo "FARHELM_MEDIA_MAC_LOCK must be a plain path under the Mac's home" >&2; exit 2; }
  command -v rsync >/dev/null || { echo "shipping the checkout needs rsync" >&2; exit 1; }
  # One source directory per checkout on the Mac, named from this checkout's path, so two checkouts never ship over
  # each other and one checkout's builds there stay incremental. The picture and log are per checkout for the same
  # reason.
  id="$(printf '%s' "$repo" | cksum | cut -d' ' -f1)" || exit 1
  src="$base/src-$id"
  out="$base/hero-$id.png"
  log="$base/hero-$id.log"
  tmp="$(mktemp -d)" || exit 1
  # shellcheck disable=SC2064 # expand now: the path is this call's own
  trap "rm -rf '$tmp'" EXIT
  remote() { "${ssh_cmd[@]}" "$mac" "$@"; }
  # Run a remote bash script from stdin with a status line added after its last command, show its output, and return
  # that status, or 125 when the line never came, so the caller can tell "failed" from "never said".
  checked_script() {
    local status
    { cat; printf '\necho "remote-status:$?"\n'; } | remote bash -s -- "$@" | tee "$tmp/remote.log"
    status="$(sed -n 's/^remote-status:\([0-9][0-9]*\)$/\1/p' "$tmp/remote.log" | tail -n 1)"
    return "${status:-125}"
  }

  mac_preflight_script | checked_script "$lockfile" || { echo "the Mac refused the capture (above)" >&2; exit 1; }

  # What git would not ignore, by every .gitignore in the checkout, which the sending rsync reads the way git does; not
  # git itself, so a jj workspace without a git working tree ships the same files. The Mac's rsync (openrsync) does not
  # apply those rules to what `--delete` may remove there, so the shipped tree's own build directory is named too, or
  # every run would delete it and build from nothing.
  echo "shipping the checkout to $mac:$src"
  rsync -a --delete --filter=':- .gitignore' --exclude=/.git --exclude=/.jj --exclude=/target \
    -e "${ssh_cmd[*]}" --rsync-path="/usr/bin/lockf -k $lockfile rsync" "$repo/" "$mac:$src/" || {
    echo "shipping the checkout to the Mac failed" >&2
    exit 1
  }

  mac_capture_script | checked_script "$src" "$lockfile" "$out" "$log" "$build" || status=$?
  if [ "$status" -eq 125 ]; then
    echo "the Mac did not report a capture status; its picture and log, if any, stay at $mac:$out and" \
      "$mac:$log until the next run" >&2
    exit 1
  fi
  if [ "$status" -ne 0 ]; then
    echo "no picture from the Mac (the reason is above); its run directory, if one is named, and its log at" \
      "$mac:$log are kept there" >&2
    exit 1
  fi
  mkdir -p "$(dirname "$output")" || exit 1
  remote "/bin/cat $out" >"$output.partial"
  # cat's status does not come back either, so the bytes have to show it worked: a PNG's signature at the front and its
  # IEND chunk at the end, so a transfer cut short is not taken for a picture.
  if [ "$(head -c 8 "$output.partial" | od -An -tx1 | tr -d ' \n')" != 89504e470d0a1a0a ] ||
    [ "$(tail -c 8 "$output.partial" | od -An -tx1 | tr -d ' \n')" != 49454e44ae426082 ]; then
    echo "fetching the picture from the Mac failed: what came back is not a whole PNG" >&2
    rm -f "$output.partial"
    exit 1
  fi
  mv "$output.partial" "$output" || exit 1
  # Cleanup has its own outcome once the checked picture has reached the checkout.
  remote "/bin/rm -f $out $log" || echo "could not remove the fetched picture and its log from the Mac" >&2
  echo "$output"
}

case "$mode" in
  local) capture_here || exit 1 ;;
  mac) capture_on_mac ;;
esac
