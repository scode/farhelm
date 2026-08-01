#!/usr/bin/env bash
# EXPERIMENTAL desktop smoke test — run on demand, wired to no gate, and
# expected to be flaky past the boot phases. webkit2gtk under Xvfb paints
# unreliably in this repo's testing so far: the window intermittently
# stays black (tried: a window manager, resize nudges,
# WEBKIT_DISABLE_COMPOSITING_MODE=1 — none fully cured it), and when it
# does not paint, the UI-driving phases below cannot see or click
# anything. The boot phases (supervisor + helm + bundled app + window
# appears) are reliable and are the part worth keeping today; treat a
# failure past them as "inspect the failure screenshot", not "the product
# regressed". The interaction recipe is kept because it HAS driven the
# real UI end to end on this host (created sessions through the form,
# typed into the terminal) and is the starting point for making this a
# real gate later.
#
# Boots a real supervisor + helm + the dx-bundled Linux desktop app
# (webkit2gtk) under Xvfb, drives the UI with xdotool, and asserts
# pass/fail through side effects — the HTTP API and tmux capture-pane.
# The one pixel-based check (ImageMagick, region-brightness) is an
# interaction GATE, not an assertion: it only answers "has the create
# form visibly opened yet, so it's safe to click into it" and retries
# until it has. It never decides pass or fail on its own — every real
# assertion still goes through the API or tmux. Screenshots are taken
# only as debugging artifacts on failure.
#
# Why this exists: the desktop renderer is otherwise a compile-check-only
# surface (CI builds it, nobody runs it), which is exactly how a bug that
# bricked every desktop create (MT-5, an eval channel dying under wry)
# survived to manual testing on macOS. This script runs the same wry +
# dioxus-desktop code paths on the same engine family (WebKitGTK), so that
# class of regression fails here first. It is NOT the final word on real
# WKWebView behavior — macOS-specific quirks still need a manual pass —
# and it is run on demand, not in CI (GUI-under-Xvfb brings flake risk the
# CI gate does not want; same pattern as the cgroup tests' documented run).
#
# Prereqs (apt): xvfb xdotool openbox imagemagick curl python3, plus the
# webkit2gtk dev stack the desktop feature already needs, dioxus-cli
# 0.7.9, and tmux.
# Usage: scripts/desktop-smoke.sh   (from the repo root; ~3 min)
#
# Known handling quirks, learned the hard way:
# - The window sometimes maps at 10x10 and stays black until a resize
#   nudge; the retry loop below resizes until xdotool reports a sane
#   geometry. A window manager (openbox) must be running or map/size
#   behavior is timing-dependent.
# - xdotool typing faster than ~10 chars/s drops keystrokes into the
#   controlled inputs (dioxus re-renders between keystrokes), so all
#   typing uses --delay 120.
# - State dirs must be SHORT: unix socket paths cap at ~108 bytes.
# - The private Xvfb display has no X authentication set up (no
#   Xauthority cookie): `-nolisten tcp` keeps it off the network, but any
#   other local account on this box can still open the display's Unix
#   socket in /tmp/.X11-unix. Fine for a single-user dev box; a shared
#   box would need `-auth` plus a per-run Xauthority file — left as
#   future work, not done here.

set -u

REPO="$(cd "$(dirname "$0")/.." && pwd)"

# Preflight, before anything is created or spawned: every external binary
# the phases below shell out to, so a missing tool is a clean SKIP instead
# of a confusing failure partway through.
for tool in Xvfb xdotool openbox import convert curl python3 tmux dx; do
  command -v "$tool" >/dev/null || { echo "SKIPPED desktop-smoke: $tool not installed" >&2; exit 0; }
done

PORT="${DESKTOP_SMOKE_PORT:-7493}"
API="http://127.0.0.1:$PORT"
DISP="" # assigned once Xvfb reports its allocated display number, below

# Private run dir. Default: a fresh mktemp'd directory, mode 0700 so no
# other local account can read the logs, sockets, or screenshots this run
# produces. If the caller points DESKTOP_SMOKE_DIR at an existing path we
# refuse it outright rather than reuse it — a pre-existing directory could
# be stale state from a killed run (its *.pid files would then point at
# unrelated, possibly-reused PIDs) or, worse, planted by another local
# user; either way trusting it is how you end up killing the wrong
# process or writing through a symlink into somewhere you didn't intend.
if [ -n "${DESKTOP_SMOKE_DIR:-}" ]; then
  X="$DESKTOP_SMOKE_DIR"
  if [ -e "$X" ] || [ -L "$X" ]; then
    echo "FAIL: DESKTOP_SMOKE_DIR ($X) already exists — refusing to reuse a possibly stale or attacker-planted path" >&2
    exit 1
  fi
  mkdir -m 0700 "$X" || { echo "FAIL: could not create DESKTOP_SMOKE_DIR ($X)" >&2; exit 1; }
else
  OLD_UMASK="$(umask)"
  umask 077
  X="$(mktemp -d /tmp/fhm-smoke.XXXXXX)" || { echo "FAIL: mktemp failed" >&2; exit 1; }
  umask "$OLD_UMASK"
fi
mkdir -p "$X/state" "$X/work"

fail() {
  echo "FAIL: $*" >&2
  DISPLAY=$DISP import -window root "$X/failure.png" 2>/dev/null &&
    echo "screenshot: $X/failure.png" >&2
  exit 1
}

# Teardown is a trap, not a function called at each exit site: registering
# it before anything is spawned means even a SIGTERM mid-boot (or a `fail`
# three phases in) still reaps every daemon this run started. Idempotent
# via TEARDOWN_DONE because the trap can fire once from the `exit` below
# *and* once more from bash's own EXIT handling of that same exit — INT
# and TERM are converted to a plain `exit` so the EXIT trap is the only
# path that ever runs teardown logic.
TEARDOWN_DONE=""
SID="" # set once the create-form phase produces a session; read by teardown
PASS=""
teardown() {
  [ -n "$TEARDOWN_DONE" ] && return
  TEARDOWN_DONE=1

  # Delete THIS run's session through its own helm while the helm is
  # still answering, so the supervisor's normal delete path (process-tree
  # kill, tmux teardown, systemd scope stop) runs first and the daemon
  # kills below and the unit-stop fallback further down are usually
  # no-ops. Best-effort: teardown must proceed even if the helm is
  # already gone or wedged.
  [ -n "$SID" ] && curl -s --max-time 5 -X DELETE "$API/api/sessions/$SID" >/dev/null 2>&1

  for p in desktop helm supervisor openbox xvfb; do
    [ -f "$X/$p.pid" ] && kill "$(cat "$X/$p.pid")" 2>/dev/null
  done
  tmux -S "$X/state/tmux.sock" kill-server 2>/dev/null

  # Last-resort scope cleanup, scoped to THIS run's session id only. A
  # bare `farhelm-*` glob here would stop every farhelm session on the
  # box, including ones this run never touched (the review swarm's worst
  # finding on an earlier version of this script). Session ids are full
  # UUIDv4s, so one can never be a prefix of another's — the glob cannot
  # catch a stranger's unit. Harmless no-op if the API delete above
  # already tore the scope down, or no systemd user manager exists.
  if [ -n "$SID" ]; then
    systemctl --user list-units "farhelm-$SID-*" --no-legend --plain 2>/dev/null |
      awk '{print $1}' | xargs -r -n1 systemctl --user stop 2>/dev/null
  fi

  if [ -n "$PASS" ]; then
    rm -rf "$X"
  else
    echo "state kept at $X" >&2
  fi
}
trap teardown EXIT
trap 'exit 143' INT TERM

echo "== building (cargo + dx desktop bundle)"
(cd "$REPO" && cargo build --quiet) || fail "cargo build"
(cd "$REPO/crates/farhelm-ui" && dx build --platform desktop >"$X/dx.log" 2>&1) || fail "dx desktop build (see $X/dx.log)"
APP="$REPO/target/dx/farhelm-ui/debug/linux/app/farhelm-ui"
[ -x "$APP" ] || fail "bundled app missing at $APP"

echo "== booting supervisor, helm, Xvfb, openbox, app"
"$REPO/target/debug/farhelm" supervisor run --state-dir "$X/state" >"$X/supervisor.log" 2>&1 &
echo $! >"$X/supervisor.pid"
sleep 2
"$REPO/target/debug/farhelm" helm run --port "$PORT" --state-dir "$X/state" >"$X/helm.log" 2>&1 &
HELM_PID=$!
echo "$HELM_PID" >"$X/helm.pid"
sleep 2
curl -sf --max-time 5 "$API/api/sessions" >/dev/null || fail "helm API not answering"
# The port is configurable (DESKTOP_SMOKE_PORT) but a squatter already
# bound to it is only DETECTED here, not avoided: curl above would happily
# succeed against someone else's server on the same port. Confirming our
# own child is still alive turns that into a loud failure instead of the
# rest of the script silently driving a stack we didn't start.
kill -0 "$HELM_PID" 2>/dev/null || fail "helm API answered but our helm process is gone (port collision with another process?)"

# Allocate the display dynamically instead of a fixed :97: a fixed number
# is a predictable resource another concurrent run (or another local
# user) can already be squatting, which would point xdotool/openbox/the
# app at someone else's X server. `-displayfd` has Xvfb pick the first
# free number itself and report it back over a fifo; opening the fifo for
# writing (fd 3, via the redirect below) happens in the forked child
# before Xvfb execs, so the blocking `read` after it can never wait on a
# writer that never shows up — if Xvfb dies before writing, the fifo's
# write end still closes and `read` unblocks with an empty result.
mkfifo "$X/xvfb-displayfd"
Xvfb -displayfd 3 -screen 0 1400x1000x24 -nolisten tcp \
  >"$X/xvfb.log" 2>&1 3>"$X/xvfb-displayfd" &
echo $! >"$X/xvfb.pid"
DISPNUM="$(timeout 15 head -n1 "$X/xvfb-displayfd" 2>/dev/null)"
[ -n "$DISPNUM" ] || fail "Xvfb never reported a display number (see $X/xvfb.log)"
DISP=":$DISPNUM"
DISPLAY=$DISP openbox >"$X/openbox.log" 2>&1 &
echo $! >"$X/openbox.pid"
sleep 1
DISPLAY=$DISP FARHELM_URL="$API" "$APP" >"$X/desktop.log" 2>&1 &
echo $! >"$X/desktop.pid"

echo "== waiting for the window and nudging it to render"
WID=""
for _ in $(seq 1 20); do
  WID=$(DISPLAY=$DISP xdotool search --name farhelm 2>/dev/null | head -1)
  [ -n "$WID" ] && break
  sleep 1
done
[ -n "$WID" ] || fail "app window never appeared"
for _ in $(seq 1 10); do
  DISPLAY=$DISP xdotool windowsize "$WID" 1200 900
  sleep 2
  GEOM=$(DISPLAY=$DISP xdotool getwindowgeometry "$WID" | awk '/Geometry/{print $2}')
  [ "$GEOM" = "1200x900" ] && break
done
[ "$GEOM" = "1200x900" ] || fail "window never took a sane size (got: ${GEOM:-none})"
sleep 3

echo "== creating a session through the real create form"
# Layout constants for the styled 1200x900 window: the new-session button,
# then the form's three inputs and the create button. If the layout shifts,
# take a screenshot (import -window root) and update these.
#
# The webview accepts clicks some unpredictable time after it first
# paints, so opening the form needs an oracle: the form panel lightens
# the region under the button, and ImageMagick can report that region's
# mean brightness without any extra tooling. Retry the click until the
# form is demonstrably open. This is the interaction GATE described in
# the header, not a pass/fail assertion.
form_region_mean() {
  DISPLAY=$DISP import -window root png:- 2>/dev/null |
    convert - -crop 700x180+30+90 -format "%[fx:mean]" info: 2>/dev/null
}
BASE=$(form_region_mean)
FORM_OPEN=""
for _ in $(seq 1 15); do
  DISPLAY=$DISP xdotool mousemove 65 67 click 1
  sleep 1.5
  NOW=$(form_region_mean)
  if python3 -c "import sys; sys.exit(0 if abs(float('$NOW')-float('$BASE'))>0.01 else 1)" 2>/dev/null; then
    FORM_OPEN=1
    break
  fi
done
[ -n "$FORM_OPEN" ] || fail "create form never opened (webview unresponsive to clicks?)"
DISPLAY=$DISP xdotool mousemove 400 128 click 1 sleep 0.3 type --delay 120 "$X/work"
DISPLAY=$DISP xdotool mousemove 400 177 click 1 sleep 0.3 type --delay 120 "bash"
DISPLAY=$DISP xdotool mousemove 400 226 click 1 sleep 0.3 type --delay 120 "smoke"

# Identify the created session by set difference, not by title: matching
# "any title starting with smo" would happily grab an unrelated pre-
# existing session (e.g. a real one the user is running, titled
# "smoke-test-notes") and report success on typed input reaching THAT
# pane. Snapshotting ids before the submit click and requiring exactly
# one new id afterward is unambiguous regardless of what titles already
# exist.
BEFORE_IDS=$(curl -s --max-time 5 "$API/api/sessions" | python3 -c "
import json, sys
d = json.load(sys.stdin)
print(' '.join(sorted(s['id'] for s in d['sessions'])))" 2>/dev/null)
DISPLAY=$DISP xdotool mousemove 60 259 click 1
SID=""
for _ in $(seq 1 15); do
  SID=$(curl -s --max-time 5 "$API/api/sessions" | BEFORE_IDS="$BEFORE_IDS" python3 -c "
import json, os, sys
before = set(os.environ.get('BEFORE_IDS', '').split())
d = json.load(sys.stdin)
after = {s['id'] for s in d['sessions']}
new = after - before
print(next(iter(new)) if len(new) == 1 else '')" 2>/dev/null)
  [ -n "$SID" ] && break
  sleep 1
done
[ -n "$SID" ] || fail "create through the desktop form did not yield exactly one new session (MT-5 class regression?)"
echo "   created $SID"

echo "== typing into the terminal and asserting through tmux"
# The create lands in the session view with the terminal mounted.
sleep 3
DISPLAY=$DISP xdotool mousemove 400 400 click 1 sleep 0.5 type --delay 120 "echo smoke-ok"
DISPLAY=$DISP xdotool key Return
OK=""
for _ in $(seq 1 10); do
  if tmux -S "$X/state/tmux.sock" capture-pane -p -t "fh-$SID" 2>/dev/null | grep -q "^smoke-ok"; then
    OK=1
    break
  fi
  sleep 1
done
[ -n "$OK" ] || fail "typed input never reached the pane"

echo "== PASS: desktop create + terminal round-trip work"
PASS=1
exit 0
