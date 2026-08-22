#!/usr/bin/env bash
# The default transport/state phase is the CI desktop integration gate. Its
# optional pixel-driven interaction phase remains experimental because
# webkit2gtk under Xvfb paints
# unreliably in this repo's testing so far: the window intermittently
# stays black (tried: a window manager, resize nudges,
# WEBKIT_DISABLE_COMPOSITING_MODE=1 — none fully cured it), and when it
# does not paint, the UI-driving phase cannot see or click anything. Item 8's
# bootstrap assertions are transport/state based and form the default smoke;
# set DESKTOP_SMOKE_LEGACY_INTERACTION=1 to run the pixel-driven create and
# terminal round-trip too. The interaction recipe is kept because it has driven the
# real UI end to end on this host (created sessions through the form,
# typed into the terminal) and is the starting point for making this a
# broader interaction path on this host.
#
# Boots the dx-bundled Linux desktop app under Xvfb. The app itself owns the
# embedded helm and managed local supervisor; starting either one here would
# preserve the old thin-client shape this harness exists to retire.
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
# while macOS-specific quirks still need the documented manual pass.
#
# Prereqs (apt): xvfb xdotool openbox imagemagick curl python3, plus the
# webkit2gtk dev stack the desktop feature already needs, dioxus-cli
# 0.7.10, and tmux.
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
# the phases below shell out to. Local optional runs skip cleanly; CI must
# fail because a green integration gate that exercised nothing is false.
for tool in Xvfb xdotool openbox import convert curl python3 tmux dx; do
  if ! command -v "$tool" >/dev/null; then
    if [ "${CI:-}" = true ]; then
      echo "FAIL: desktop-smoke requires $tool in CI" >&2
      exit 1
    fi
    echo "SKIPPED desktop-smoke: $tool not installed" >&2
    exit 0
  fi
done

PORT="${DESKTOP_SMOKE_PORT:-7493}"
API="http://127.0.0.1:$PORT"
DISP="" # assigned once Xvfb reports its allocated display number, below

# A fixed correlation value, not a uniqueness scheme: every run greps a log
# inside its own freshly created private directory (the mktemp/refuse-existing
# logic above), so a prior run's marker can never be in the file searched.
# The app reads this into WebviewBootstrap::smoke_client_log_marker and the
# shim console.errors it once armed; the poll loops below grep for it landing
# on a webview_console tracing line, proving shim -> /api/client-log ->
# tracing end to end (PLAN_desktop_web_bug_triage.md).
CLIENT_LOG_MARKER="farhelm-smoke-clientlog-pipeline-proof"

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

# Curl reads the bearer header from this private file so the secret never
# appears in a process listing. Every loopback request bypasses ambient proxy
# configuration as the desktop clients do.
CURL_AUTH_CONFIG="$X/curl-auth.conf"
write_curl_auth() {
  printf 'header = "Authorization: Bearer %s"\n' "$NATIVE_SECRET" >"$CURL_AUTH_CONFIG"
  chmod 600 "$CURL_AUTH_CONFIG"
}
curl_local() {
  curl --noproxy '*' "$@"
}
curl_auth() {
  curl --noproxy '*' --config "$CURL_AUTH_CONFIG" "$@"
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
SID_NEWEST="" # second restart fixture; empty during the interaction-only create
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
  [ -n "$SID" ] && [ -s "$CURL_AUTH_CONFIG" ] && curl_auth -s --max-time 5 -X DELETE "$API/api/sessions/$SID" >/dev/null 2>&1
  [ -n "$SID_NEWEST" ] && [ -s "$CURL_AUTH_CONFIG" ] && curl_auth -s --max-time 5 -X DELETE "$API/api/sessions/$SID_NEWEST" >/dev/null 2>&1

  for p in desktop openbox xvfb; do
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
    rm -r -- "$X"
  else
    echo "state kept at $X" >&2
  fi
}
trap teardown EXIT
trap 'exit 143' INT TERM

echo "== building (cargo + web bundle + dx desktop bundle)"
(cd "$REPO" && flock --close /tmp/fh-build.lock -c 'ulimit -c unlimited && cargo build --quiet') || fail "cargo build"
(cd "$REPO/crates/farhelm-ui" && flock --close /tmp/fh-build.lock -c 'ulimit -c unlimited && dx build --platform web --release' >"$X/dx-web.log" 2>&1) || fail "dx web build (see $X/dx-web.log)"
(cd "$REPO/crates/farhelm-ui" && flock --close /tmp/fh-build.lock -c 'ulimit -c unlimited && dx build --platform desktop' >"$X/dx.log" 2>&1) || fail "dx desktop build (see $X/dx.log)"
BUILT_APP="$REPO/target/dx/farhelm-ui/debug/linux/app/farhelm-ui"
[ -x "$BUILT_APP" ] || fail "bundled app missing at $BUILT_APP"

# Stage the release shape and make its tmux a sentinel wrapper around the
# preflighted host binary. The app starts with a deliberately small PATH; the
# marker proves DesktopBootstrap prepended the CLI's sibling directory and
# the supervisor resolved this exact `tmux` entry rather than the host copy.
APP_CONTENTS="$X/Farhelm.app/Contents"
mkdir -p "$APP_CONTENTS/MacOS" "$APP_CONTENTS/Resources/web"
cp "$BUILT_APP" "$APP_CONTENTS/MacOS/farhelm-ui"
cp "$REPO/target/debug/farhelm" "$APP_CONTENTS/MacOS/farhelm"
cp -R "$REPO/target/dx/farhelm-ui/release/web/public/." "$APP_CONTENTS/Resources/web/"
# `dx build --platform desktop`'s Linux output puts every page script
# (client-log-shim.js, terminal.js, xterm.js, ...) in an `assets/` directory
# that is a SIBLING of the executable, not inside it — wry's `dioxus://`
# asset scheme resolves them relative to the running binary's own path.
# Relocating only the binary above and leaving `assets/` behind breaks that
# resolution silently: every `<script src="dioxus://...">` tag still
# appears in `document.scripts` and the page still reports
# `readyState: "complete"`, but NONE of their content ever loads or runs, so
# every `window.__farhelm*` global they install stays `undefined` — this is
# exactly the class of failure the client-log leg below exists to catch, so
# the harness must not itself reintroduce it by omission.
cp -R "$(dirname "$BUILT_APP")/assets" "$APP_CONTENTS/MacOS/assets" || fail "staging desktop assets"
APP="$APP_CONTENTS/MacOS/farhelm-ui"
HOST_TMUX=$(command -v tmux)
printf '%s\n' \
  '#!/bin/sh' \
  'printf used >"$FARHELM_SMOKE_TMUX_MARKER"' \
  "exec \"$HOST_TMUX\" \"\$@\"" >"$APP_CONTENTS/MacOS/tmux"
chmod 700 "$APP_CONTENTS/MacOS/farhelm" "$APP_CONTENTS/MacOS/tmux"

echo "== booting Xvfb, openbox, and the self-contained app"

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
DISPLAY=$DISP \
  PATH="$APP_CONTENTS/MacOS:/usr/bin:/bin" \
  FARHELM_SMOKE_TMUX_MARKER="$X/bundled-tmux-used" \
  FARHELM_SMOKE_CLIENT_LOG_MARKER="$CLIENT_LOG_MARKER" \
  RUST_LOG=info \
  FARHELM_DESKTOP_PORT="$PORT" \
  FARHELM_DESKTOP_STATE_DIR="$X/state" \
  "$APP" >"$X/desktop.log" 2>&1 &
echo $! >"$X/desktop.pid"

for _ in $(seq 1 30); do
  curl_local -sf --max-time 2 "$API/" >/dev/null 2>&1 && break
  sleep 1
done
curl_local -sf --max-time 5 "$API/" | grep -q '<!DOCTYPE html>' || fail "embedded helm did not serve the bundled UI"

for _ in $(seq 1 30); do
  [ -f "$X/state/desktop-client.json" ] && break
  sleep 1
done
[ -f "$X/state/desktop-client.json" ] || fail "desktop credentials were not persisted"
NATIVE_SECRET=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["native_device_secret"])' "$X/state/desktop-client.json")
write_curl_auth
LOCAL_READY=""
for _ in $(seq 1 30); do
  if curl_auth -sf --max-time 5 "$API/api/hosts" | python3 -c '
import json,sys
hosts=json.load(sys.stdin)["hosts"]
assert any(h["kind"] == "local" and h["state"]["phase"] == "connected" for h in hosts)
' 2>/dev/null; then
    LOCAL_READY=1
    break
  fi
  sleep 1
done
[ -n "$LOCAL_READY" ] || fail "authenticated native API or managed local supervisor was not reachable"
[ -s "$X/bundled-tmux-used" ] || fail "managed supervisor did not resolve the bundle-shaped tmux sentinel"
for _ in $(seq 1 30); do
  WEBVIEW_SECRET=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("webview_device_secret") or "")' "$X/state/desktop-client.json")
  WEBVIEW_GENERATION=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("webview_auth_generation") or 0)' "$X/state/desktop-client.json")
  [ -n "$WEBVIEW_SECRET" ] && [ "$WEBVIEW_GENERATION" -ge 1 ] && break
  sleep 1
done
[ -n "$WEBVIEW_SECRET" ] && [ "$WEBVIEW_GENERATION" -ge 1 ] || fail "the webview JavaScript stack did not authenticate its event socket"

DEVICE_ROWS=$(python3 -c 'import sqlite3,sys; print(sqlite3.connect(sys.argv[1]).execute("select count(*) from device_sessions").fetchone()[0])' "$X/state/helm.db")
[ "$DEVICE_ROWS" = 2 ] || fail "desktop bootstrap minted $DEVICE_ROWS device rows instead of two"

# Wait for the client-log marker to land in $1 (a log file), proving the
# shim -> /api/client-log -> tracing pipeline for the process writing it.
# The shim paces flushes against MIN_FLUSH_INTERVAL_MS (30s) measured on
# the page's monotonic clock from load, so the FIRST flush after arming can
# legitimately wait out most of one cycle (measured ~29s on this harness),
# and the batch still owes a round trip through fetch, the helm's
# auth/parse/rate-cap path, and tracing before it appears in the log. 60s
# covers a full pacing cycle plus that margin; a HEALTHY marker arrives
# before the bound, while a genuinely missing one is only reported after
# the full timeout — the checked-then-sleep loop ends with one final check
# so a marker landing during the last sleep still counts.
wait_for_client_log_marker() {
  local log="$1" start seen=""
  start=$(date +%s)
  for _ in $(seq 1 60); do
    if grep -q "webview_console.*${CLIENT_LOG_MARKER}" "$log" 2>/dev/null; then
      seen=1
      break
    fi
    sleep 1
  done
  [ -n "$seen" ] || grep -q "webview_console.*${CLIENT_LOG_MARKER}" "$log" 2>/dev/null || fail \
    "the client-log marker never reached a webview_console line in $(basename "$log") (shim -> /api/client-log -> tracing pipeline broken)"
  echo "   client-log marker landed in $(basename "$log") after $(($(date +%s) - start))s"
}

echo "== waiting for the client-log marker: shim -> /api/client-log -> tracing"
wait_for_client_log_marker "$X/desktop.log"

echo "== creating two bundle-substrate sessions before the hard restart"
CREATE_BODY=$(python3 -c 'import json,sys; print(json.dumps({"cwd": sys.argv[1], "invocation": "bash", "title": "zzz-remembered"}))' "$X/work") || fail "encoding remembered smoke session"
SID=$(curl_auth -sf --max-time 10 -H 'content-type: application/json' -d "$CREATE_BODY" "$API/api/sessions" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])') || fail "creating the pre-restart smoke session"
[ -n "$SID" ] || fail "the pre-restart create returned no session id"
tmux -S "$X/state/tmux.sock" has-session -t "fh-$SID" 2>/dev/null || fail "the created session did not reach bundle-local tmux"
sleep 1
NEWEST_BODY=$(python3 -c 'import json,sys; print(json.dumps({"cwd": sys.argv[1], "invocation": "bash", "title": "aaa-newest"}))' "$X/work") || fail "encoding newest smoke session"
SID_NEWEST=$(curl_auth -sf --max-time 10 -H 'content-type: application/json' -d "$NEWEST_BODY" "$API/api/sessions" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])') || fail "creating the newest pre-restart smoke session"
[ -n "$SID_NEWEST" ] || fail "the newest pre-restart create returned no session id"
tmux -S "$X/state/tmux.sock" has-session -t "fh-$SID_NEWEST" 2>/dev/null || fail "the newest session did not reach bundle-local tmux"

# The default gate deliberately does not click rows: WebKitGTK's unreliable
# Xvfb paint is why its CI leg is non-pixel. Seed a real identity-keyed record,
# then observe restoration through the native request trace (`sort=title`) and
# tmux's client ownership (the remembered, non-newest session gains the page's
# output client). The state-file assertion remains as a check of the durable
# input, not as a substitute for those behavioral observations.
LOCAL_IDENTITY=$(curl_auth -sf --max-time 5 "$API/api/hosts" | python3 -c '
import json,sys
hosts=json.load(sys.stdin)["hosts"]
print(next(h["identity"] for h in hosts if h["kind"] == "local"))
') || fail "reading the local install identity for the preference seed"
[ -n "$LOCAL_IDENTITY" ] || fail "the local host row carried no install identity"
python3 -c '
import json,os,sys,tempfile
path, helm, session = sys.argv[1:]
with open(path) as source:
    state = json.load(source)
state["remembered_selection"] = {"helm": helm, "id": session}
state["list_sort"] = "title"
fd, temporary = tempfile.mkstemp(prefix="desktop-client.", suffix=".tmp", dir=os.path.dirname(path))
try:
    os.fchmod(fd, 0o600)
    with os.fdopen(fd, "w") as target:
        json.dump(state, target, separators=(",", ":"))
        target.flush()
        os.fsync(target.fileno())
    os.replace(temporary, path)
finally:
    if os.path.exists(temporary):
        os.unlink(temporary)
' "$X/state/desktop-client.json" "$LOCAL_IDENTITY" "$SID" || fail "seeding desktop restart preferences"

echo "== waiting for the window and nudging it to render"
WID=""
for _ in $(seq 1 20); do
  WID=$(DISPLAY=$DISP xdotool search --name farhelm 2>/dev/null | tail -1)
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

echo "== killing the app without Rust cleanup and reusing both device sessions"
OLD_DESKTOP_PID=$(cat "$X/desktop.pid")
SUPERVISOR_PID=$(ps -eo pid=,ppid=,args= | awk -v parent="$OLD_DESKTOP_PID" '$2 == parent && /supervisor run/ { print $1; exit }')
[ -n "$SUPERVISOR_PID" ] || fail "could not identify the managed supervisor child"
kill -KILL "$OLD_DESKTOP_PID" || fail "terminating desktop app without graceful cleanup"
for _ in $(seq 1 20); do
  DESKTOP_STAT=$(ps -o stat= -p "$OLD_DESKTOP_PID" 2>/dev/null | tr -d ' ')
  [ -z "$DESKTOP_STAT" ] || [ "${DESKTOP_STAT#Z}" != "$DESKTOP_STAT" ] && break
  sleep 0.5
done
DESKTOP_STAT=$(ps -o stat= -p "$OLD_DESKTOP_PID" 2>/dev/null | tr -d ' ')
[ -z "$DESKTOP_STAT" ] || [ "${DESKTOP_STAT#Z}" != "$DESKTOP_STAT" ] || fail "desktop app did not exit after its window closed"
wait "$OLD_DESKTOP_PID" 2>/dev/null
KILLED_STATUS=$?
[ "$KILLED_STATUS" -ne 0 ] || fail "desktop app unexpectedly reported graceful success after SIGKILL"
SUPERVISOR_GONE=""
for _ in $(seq 1 20); do
  if ! kill -0 "$SUPERVISOR_PID" 2>/dev/null; then
    SUPERVISOR_GONE=1
    break
  fi
  sleep 0.25
done
[ -n "$SUPERVISOR_GONE" ] || fail "managed supervisor outlived the desktop app"
for _ in $(seq 1 20); do
  curl_local -sf --max-time 1 "$API/" >/dev/null 2>&1 || break
  sleep 0.25
done

DISPLAY=$DISP \
  PATH="$APP_CONTENTS/MacOS:/usr/bin:/bin" \
  FARHELM_SMOKE_TMUX_MARKER="$X/bundled-tmux-used" \
  FARHELM_SMOKE_CLIENT_LOG_MARKER="$CLIENT_LOG_MARKER" \
  RUST_LOG=info \
  FARHELM_DESKTOP_PORT="$PORT" \
  FARHELM_DESKTOP_STATE_DIR="$X/state" \
  "$APP" >"$X/desktop-restart.log" 2>&1 &
echo $! >"$X/desktop.pid"
for _ in $(seq 1 30); do
  curl_local -sf --max-time 2 "$API/" >/dev/null 2>&1 && break
  sleep 1
done
for _ in $(seq 1 30); do
  RESTART_WEBVIEW_GENERATION=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("webview_auth_generation") or 0)' "$X/state/desktop-client.json")
  [ "$RESTART_WEBVIEW_GENERATION" -gt "$WEBVIEW_GENERATION" ] && break
  sleep 1
done
[ "$RESTART_WEBVIEW_GENERATION" -gt "$WEBVIEW_GENERATION" ] || fail "restarted webview never completed authenticated readiness"
curl_auth -sf --max-time 5 "$API/api/hosts" >/dev/null || fail "restarted app did not reuse native authentication"
RESTART_ROWS=$(python3 -c 'import sqlite3,sys; print(sqlite3.connect(sys.argv[1]).execute("select count(*) from device_sessions").fetchone()[0])' "$X/state/helm.db")
[ "$RESTART_ROWS" = "$DEVICE_ROWS" ] || fail "restart minted device rows ($DEVICE_ROWS before, $RESTART_ROWS after)"
RESTART_NATIVE=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["native_device_secret"])' "$X/state/desktop-client.json")
RESTART_WEBVIEW=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["webview_device_secret"])' "$X/state/desktop-client.json")
[ "$RESTART_NATIVE" = "$NATIVE_SECRET" ] || fail "restart replaced the persisted native device session"
[ "$RESTART_WEBVIEW" = "$WEBVIEW_SECRET" ] || fail "restart replaced the persisted webview device session"
RESTART_SELECTION=$(python3 -c 'import json,sys; print(json.dumps(json.load(open(sys.argv[1])).get("remembered_selection"),sort_keys=True,separators=(",",":")))' "$X/state/desktop-client.json")
EXPECTED_SELECTION=$(python3 -c 'import json,sys; print(json.dumps({"helm":sys.argv[1],"id":sys.argv[2]},sort_keys=True,separators=(",",":")))' "$LOCAL_IDENTITY" "$SID")
RESTART_SORT=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("list_sort") or "")' "$X/state/desktop-client.json")
[ "$RESTART_SELECTION" = "$EXPECTED_SELECTION" ] || fail "restart changed the persisted desktop selection record"
[ "$RESTART_SORT" = "title" ] || fail "restart dropped the persisted desktop sort"
tmux -S "$X/state/tmux.sock" has-session -t "fh-$SID" 2>/dev/null || fail "the tmux-held session did not survive the app restart"
RESTORED_SORT_REQUEST=""
for _ in $(seq 1 30); do
  if grep -q 'desktop_smoke.*query=sort=title' "$X/desktop-restart.log"; then
    RESTORED_SORT_REQUEST=1
    break
  fi
  sleep 1
done
[ -n "$RESTORED_SORT_REQUEST" ] || fail "the relaunched page did not request sort=title"
RESTORED_ATTACHMENT=""
for _ in $(seq 1 30); do
  CLIENT_SESSIONS=$(tmux -S "$X/state/tmux.sock" list-clients -F '#{session_name}' 2>/dev/null)
  REMEMBERED_CLIENTS=$(printf '%s\n' "$CLIENT_SESSIONS" | grep -cx "fh-$SID")
  NEWEST_CLIENTS=$(printf '%s\n' "$CLIENT_SESSIONS" | grep -cx "fh-$SID_NEWEST")
  if [ "$REMEMBERED_CLIENTS" -gt "$NEWEST_CLIENTS" ]; then
    RESTORED_ATTACHMENT=1
    break
  fi
  sleep 1
done
[ -n "$RESTORED_ATTACHMENT" ] || fail "the relaunched page did not attach the remembered non-newest session"
SESSION_REDISCOVERED=""
for _ in $(seq 1 30); do
  if curl_auth -sf --max-time 5 "$API/api/sessions/$SID" >/dev/null; then
    SESSION_REDISCOVERED=1
    break
  fi
  sleep 1
done
[ -n "$SESSION_REDISCOVERED" ] || fail "the restarted app did not rediscover the surviving session"

# The RESTARTED process must prove the client-log pipeline too, before the
# rotation below muddies the auth waters: restart arms the shim through the
# persisted-credential path — a different flow from first launch — and a
# regression there would leave logging silently dead after every ordinary
# app restart while all the other restart assertions stayed green.
echo "== waiting for the restarted app's client-log marker"
wait_for_client_log_marker "$X/desktop-restart.log"

echo "== rotating the token and refreshing both client stacks on 401"
"$APP_CONTENTS/MacOS/farhelm" helm token rotate --state-dir "$X/state" >/dev/null || fail "rotating desktop helm token"
ROTATED=""
for _ in $(seq 1 30); do
  NEXT_NATIVE=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("native_device_secret") or "")' "$X/state/desktop-client.json")
  NEXT_WEBVIEW=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("webview_device_secret") or "")' "$X/state/desktop-client.json")
  NEXT_WEBVIEW_GENERATION=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("webview_auth_generation") or 0)' "$X/state/desktop-client.json")
  if [ -n "$NEXT_NATIVE" ] && [ -n "$NEXT_WEBVIEW" ] && [ "$NEXT_NATIVE" != "$NATIVE_SECRET" ] && [ "$NEXT_WEBVIEW" != "$WEBVIEW_SECRET" ] && [ "$NEXT_WEBVIEW_GENERATION" -gt "$RESTART_WEBVIEW_GENERATION" ]; then
    ROTATED=1
    break
  fi
  sleep 1
done
[ -n "$ROTATED" ] || fail "both desktop client stacks did not exchange after rotation"
NATIVE_SECRET="$NEXT_NATIVE"
WEBVIEW_SECRET="$NEXT_WEBVIEW"
write_curl_auth
ROTATED_ROWS=$(python3 -c 'import sqlite3,sys; print(sqlite3.connect(sys.argv[1]).execute("select count(*) from device_sessions").fetchone()[0])' "$X/state/helm.db")
[ "$ROTATED_ROWS" = 2 ] || fail "rotation recovery left $ROTATED_ROWS device rows instead of two"
curl_auth -sf --max-time 5 "$API/api/hosts" >/dev/null || fail "refreshed native credential was not accepted"

WID=""
for _ in $(seq 1 20); do
  WID=$(DISPLAY=$DISP xdotool search --name farhelm 2>/dev/null | tail -1)
  [ -n "$WID" ] && break
  sleep 1
done
[ -n "$WID" ] || fail "restarted app window never appeared"
DISPLAY=$DISP xdotool windowsize "$WID" 1200 900
sleep 3

curl_auth -sf --max-time 5 -X DELETE "$API/api/sessions/$SID" >/dev/null || fail "cleaning up the persisted smoke session"
curl_auth -sf --max-time 5 -X DELETE "$API/api/sessions/$SID_NEWEST" >/dev/null || fail "cleaning up the newest persisted smoke session"
SID=""
SID_NEWEST=""

# Guards the watchdog's other failure mode: a false positive. Every phase
# above ran with a healthy eval bridge, so its death message
# ("webview_watchdog.rs") must never have fired across either boot this run
# captured (the pre-restart desktop.log and the post-restart
# desktop-restart.log). Checked once, here, ahead of every PASS this script
# can print — the smoke script's fragility budget spends this leg as one
# assertion helper, invoked exactly once per exit path (default or legacy
# interaction) immediately before PASS, so a watchdog false positive during
# the optional interaction phase cannot slip in after an early check. The
# grep statuses are handled explicitly because this is a NEGATIVE assertion:
# "message absent" (status 1) is the only pass; an unreadable log (status 2)
# must fail loudly rather than impersonating silence.
assert_watchdog_silent() {
  grep -q "webview eval bridge is not answering" "$X/desktop.log" "$X/desktop-restart.log"
  case $? in
    0) fail "watchdog false positive: the eval-bridge death message fired during a healthy run" ;;
    1) : ;;
    *) fail "could not read the desktop logs to verify watchdog silence" ;;
  esac
}

# Focus the window and verify focus actually landed before returning.
# `xdotool windowactivate` is a REQUEST that returns before openbox acts on
# it, and a bare `key` chord goes to whichever window holds focus at that
# instant — so a close chord sent without this verification can fire before
# focus lands and ask nothing at all to exit. That race is the leading
# suspect for this gate's "did not exit cleanly" CI flakes (2026-08-16/17),
# which never reproduced locally. A bounded verify loop rather than
# `windowactivate --sync`, deliberately: --sync can block forever against a
# stalled window manager, and a smoke leg must always reach its own failure
# path.
activate_and_verify() {
  for _ in $(seq 1 10); do
    DISPLAY=$DISP xdotool windowactivate "$1" 2>/dev/null
    sleep 0.5
    [ "$(DISPLAY=$DISP xdotool getactivewindow 2>/dev/null)" = "$1" ] && return 0
  done
  return 1
}

# One observation: gone from the process table, or a zombie (which counts
# as exited — the caller reaps it with `wait`).
pid_gone() {
  STAT=$(ps -o stat= -p "$1" 2>/dev/null | tr -d ' ')
  [ -z "$STAT" ] || [ "${STAT#Z}" != "$STAT" ]
}

# Wait up to 10 seconds for a pid to leave the process table.
wait_for_pid_exit() {
  for _ in $(seq 1 20); do
    pid_gone "$1" && return 0
    sleep 0.5
  done
  # One observation AFTER the final sleep. Without it, an app exiting in
  # the last half-second reads as still alive — the pre-helper code made
  # this final check, and losing it would mint a fresh clean-exit flake
  # inside the change meant to remove one.
  pid_gone "$1"
}

# Deliver the close chord to the currently focused window. The one seam —
# DESKTOP_SMOKE_FAULT_FIRST_CHORD=1 turns the FIRST delivery into a
# harmless F24 — exists so the recovery branch below can be exercised on
# demand instead of trusting a one-off manual validation; nothing in CI
# sets it.
send_close_chord() { # $1 = first | retry
  if [ "$1" = first ] && [ "${DESKTOP_SMOKE_FAULT_FIRST_CHORD:-}" = 1 ]; then
    DISPLAY=$DISP xdotool key F24
    return $?
  fi
  DISPLAY=$DISP xdotool key alt+F4
}

# Evidence dump for a failed exit leg, written into the run dir so the CI
# failure artifact carries it: which window held focus, what farhelm
# windows existed, and what state the app process was in. Focus here is
# sampled after the fact rather than atomically with the chord, so this
# helps distinguish "the keystroke never landed" from "the app is
# genuinely wedged on shutdown" — the two candidate mechanisms behind the
# CI flake — without on its own proving either.
report_exit_leg_state() { # $1 = expected window id, $2 = app pid
  {
    echo "active window: $(DISPLAY=$DISP xdotool getactivewindow 2>/dev/null)"
    echo "expected window: $1"
    echo "farhelm windows: $(DISPLAY=$DISP xdotool search --name farhelm 2>/dev/null | tr '\n' ' ')"
    echo "app process: $(ps -o pid=,stat=,wchan=,cmd= -p "$2" 2>/dev/null)"
  } | tee "$X/exit-leg-state.txt" >&2
}

# Close the app the way a user would (alt+F4 through the window manager),
# wait for the process to exit, and reap it, tolerating exactly one lost
# chord: if the app is still alive after the first 10-second wait,
# re-verify focus and re-deliver once before concluding it is wedged. A
# second silent loss in a row with focus verified both times would be
# evidence of something worse than the known activation race, and should
# fail. Each failure path names a DISTINCT verdict, because the whole
# point of this leg's diagnostics is telling input-delivery failures
# apart from application-shutdown failures.
close_app_and_wait() { # $1 = window id, $2 = app pid
  if ! activate_and_verify "$1"; then
    report_exit_leg_state "$1" "$2"
    # A process that already exited has no window to focus; blaming the
    # focus race would send the investigation at input delivery when the
    # app crashed on its own.
    if pid_gone "$2"; then
      fail "desktop app exited on its own before the close chord was ever delivered"
    fi
    fail "the app window never took focus for the close chord"
  fi
  send_close_chord first || {
    report_exit_leg_state "$1" "$2"
    fail "xdotool could not deliver the close chord"
  }
  if ! wait_for_pid_exit "$2"; then
    echo "== app alive 10s after close chord; re-verifying focus and re-delivering once" >&2
    # A vanished window over a live process is its own verdict: the chord
    # landed, the UI died, and the process is stuck in shutdown.
    # Re-activating the dead window id would misreport that as an
    # input-delivery failure.
    if ! DISPLAY=$DISP xdotool search --name farhelm 2>/dev/null | grep -qx "$1"; then
      report_exit_leg_state "$1" "$2"
      fail "window closed but the process is still alive: wedged in shutdown, not missing input"
    fi
    activate_and_verify "$1" || {
      report_exit_leg_state "$1" "$2"
      fail "the app window lost focus after the first close chord"
    }
    send_close_chord retry || {
      report_exit_leg_state "$1" "$2"
      fail "xdotool could not deliver the retry close chord"
    }
    wait_for_pid_exit "$2" || {
      report_exit_leg_state "$1" "$2"
      fail "desktop app did not exit after a focus-verified, re-delivered close"
    }
  fi
  # Reaping lives here rather than at the call sites: both legs need the
  # same wait-and-status check, and a caller that forgot it would
  # silently accept an unsuccessful exit.
  wait "$2" || fail "desktop app exited unsuccessfully"
}

if [ "${DESKTOP_SMOKE_LEGACY_INTERACTION:-}" != 1 ]; then
  assert_watchdog_silent
  close_app_and_wait "$WID" "$(cat "$X/desktop.pid")"
  echo "== PASS: embedded helm, dual auth, local supervisor, and a tmux-held session survive restart"
  PASS=1
  exit 0
fi

echo "== creating a session through the real create form"
# The clicks below are ROOT coordinates, so the window they target must sit
# at a known origin first. The window being clicked is the RESTARTED app's —
# a different window from the one sized before the kill — and openbox places
# a fresh window wherever its policy likes (observed at (199,84) on this
# screen), which silently invalidates every constant here. Re-find, re-size,
# and pin it to 0,0 before the first click.
LEG_WID=""
for _ in $(seq 1 20); do
  LEG_WID=$(DISPLAY=$DISP xdotool search --name farhelm 2>/dev/null | tail -1)
  [ -n "$LEG_WID" ] && break
  sleep 1
done
[ -n "$LEG_WID" ] || fail "restarted app window never appeared for the interaction leg"
DISPLAY=$DISP xdotool windowsize "$LEG_WID" 1200 900
DISPLAY=$DISP xdotool windowmove "$LEG_WID" 0 0
sleep 2

# Layout constants for the styled 1200x900 window (frame pinned at 0,0;
# openbox's titlebar puts the client area roughly 18px down) under the
# two-pane shell (BUGS_BURNDOWN.md issue 5): the create form lives in the
# 340px left sidebar, and with the hosts panel and filter bar collapsed
# behind toggles only the toggle row and the compact host strip sit above
# the new-session button, so its y is near the top. The terminal occupies
# the pane to the right. If the layout shifts, take a screenshot
# (import -window root) and update these.
#
# The webview accepts clicks some unpredictable time after it first
# paints, so opening the form needs an oracle: the form panel lightens
# the region under the button, and ImageMagick can report that region's
# mean brightness without any extra tooling. Retry the click until the
# form is demonstrably open. This is the interaction GATE described in
# the header, not a pass/fail assertion.
form_region_mean() {
  DISPLAY=$DISP import -window root png:- 2>/dev/null |
    convert - -crop 300x280+20+133 -format "%[fx:mean]" info: 2>/dev/null
}
BASE=$(form_region_mean)
FORM_OPEN=""
for _ in $(seq 1 15); do
  DISPLAY=$DISP xdotool mousemove 65 123 click 1
  sleep 1.5
  NOW=$(form_region_mean)
  if python3 -c "import sys; sys.exit(0 if abs(float('$NOW')-float('$BASE'))>0.01 else 1)" 2>/dev/null; then
    FORM_OPEN=1
    break
  fi
done
[ -n "$FORM_OPEN" ] || fail "create form never opened (webview unresponsive to clicks?)"
# The agent selector preselects a PROFILE, under which the typed command
# below would be inert ("the selected profile supplies it") and the create
# would launch that profile's agent instead of bash. "custom command" is
# the selector's FIRST option by construction (see the create form's rsx),
# so Home+Return in the opened popup reaches it without depending on how
# many starter profiles exist.
DISPLAY=$DISP xdotool mousemove 170 248 click 1 sleep 0.5 key Home sleep 0.3 key Return
sleep 1
# ctrl+a first: the working-directory field is prefilled with "~" (the
# create form's default), and xdotool types at the caret rather than
# replacing — without the select-all this would submit "~$X/work".
#
# The command/title/create ys are midpoints tolerant of the ~14px upward
# shift the selector change causes (its explanatory label collapses from
# two lines to one): each lands inside the target field in both layouts.
DISPLAY=$DISP xdotool mousemove 170 302 click 1 sleep 0.3 key ctrl+a type --delay 120 "$X/work"
DISPLAY=$DISP xdotool mousemove 170 358 click 1 sleep 0.3 type --delay 120 "bash"
DISPLAY=$DISP xdotool mousemove 170 407 click 1 sleep 0.3 type --delay 120 "smoke"

# Identify the created session by set difference, not by title: matching
# "any title starting with smo" would happily grab an unrelated pre-
# existing session (e.g. a real one the user is running, titled
# "smoke-test-notes") and report success on typed input reaching THAT
# pane. Snapshotting ids before the submit click and requiring exactly
# one new id afterward is unambiguous regardless of what titles already
# exist.
BEFORE_IDS=$(curl_auth -s --max-time 5 "$API/api/sessions" | python3 -c "
import json, sys
d = json.load(sys.stdin)
print(' '.join(sorted(s['id'] for s in d['sessions'])))" 2>/dev/null)
DISPLAY=$DISP xdotool mousemove 60 440 click 1
SID=""
for _ in $(seq 1 15); do
  SID=$(curl_auth -s --max-time 5 "$API/api/sessions" | BEFORE_IDS="$BEFORE_IDS" python3 -c "
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

# Creation is a user-initiated selection, so it exercises the same eval and
# debounced native write-back as a row click without adding another fragile
# coordinate. Wait beyond the debounce through the observable file rather
# than sleeping for an assumed duration.
for _ in $(seq 1 20); do
  WRITTEN_ID=$(python3 -c 'import json,sys; print((json.load(open(sys.argv[1])).get("remembered_selection") or {}).get("id") or "")' "$X/state/desktop-client.json" 2>/dev/null)
  [ "$WRITTEN_ID" = "$SID" ] && break
  sleep 0.25
done
[ "$WRITTEN_ID" = "$SID" ] || fail "the desktop page selection did not reach desktop-client.json"

echo "== typing into the terminal and asserting through tmux"
# The create lands in the session view with the terminal mounted.
sleep 3
DISPLAY=$DISP xdotool mousemove 700 400 click 1 sleep 0.5 type --delay 120 "echo smoke-ok"
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

curl_auth -sf --max-time 5 -X DELETE "$API/api/sessions/$SID" >/dev/null || fail "cleaning up smoke session"
SID=""
close_app_and_wait "$WID" "$(cat "$X/desktop.pid")"

# The legacy path's ONE watchdog check, after every interaction that could
# have provoked a false positive — see assert_watchdog_silent's comment.
assert_watchdog_silent
echo "== PASS: embedded helm, dual auth, local supervisor, restart reuse, and terminal round-trip work"
PASS=1
exit 0
