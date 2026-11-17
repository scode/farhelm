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
#   --mac DEST     capture on a Mac reached over ssh at DEST (the default
#                  when FARHELM_MEDIA_MAC names one): the picture comes from
#                  there, because only a Mac renders it at the scenario's
#                  scale. The checkout's files (those git would not ignore)
#                  are shipped to it, built there and run under its lock.
#   --local        capture on this machine: on a Mac, the picture itself;
#                  on Linux a development run, which renders at scale 1 and
#                  so always stops at the size check.
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
# The capture: run the --local half with the Mac's toolchain on PATH, its output to a log rather than to ssh, so that
# a dropped connection (Ctrl-C here, a tool timeout, the network) cannot kill it with SIGPIPE before it reaches its own
# `stop`, which is what releases the Mac-wide lock. Then show the log. Arguments: the source directory, the lock, the
# output and the log, all under the Mac's home, and whether to build.
mac_capture_script() {
  cat <<'EOF'
src=$1 lock=$2 out=$3 log=$4 build=$5
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

fetch_from_mac() {
  local out="$1"
  [ "$2" -eq 0 ] || return 1
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
  remote "/bin/rm -f $out" || echo "could not remove the fetched picture from the Mac" >&2
  echo "$output"
}

case "$mode" in
  local) capture_here || exit 1 ;;
  mac)
    # An old picture must not pass for this run if shipment or capture fails.
    rm -f "$output" || exit 1
    # shellcheck source=scripts/media-mac.sh
    source "$repo/scripts/media-mac.sh" || exit 1
    capture_on_mac "$repo" "$mac" "$build" hero.png
    ;;
esac
