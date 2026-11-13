#!/usr/bin/env bash
# The default transport/state phase is the Linux release-artifact desktop
# integration gate. Its optional pixel-driven interaction phase remains
# experimental because
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
# Boots the dx-built Linux desktop app under Xvfb, in place, with its
# `farhelm` sibling named through FARHELM_DESKTOP_FARHELM. The app itself owns
# the embedded helm and managed local supervisor; starting either one here
# would preserve the old thin-client shape this harness exists to retire.
#
# The subject is the `farhelm-ui` bin rather than `farhelm-desktop`, and that
# costs nothing: both are one call to `farhelm_ui::desktop::run`, and this one
# is what `dx build --platform desktop` produces with the asset names filled
# in. `farhelm-desktop` itself is exercised by
# `scripts/check-desktop-assets.sh` at build time and by the maintainer on a
# real Mac.
# The legacy leg's pixel-based check (ImageMagick, region-brightness) is an
# interaction GATE, not an assertion: it only answers "has the create
# form visibly opened yet, so it's safe to click into it" and retries
# until it has. It never decides pass or fail on its own — every real
# assertion still goes through the API, the supervisor's database and the
# kernel's socket table, the embedded helm's attachment trace, or a command
# typed into a terminal (see `session_terminal_alive`, `expect_host_terminal`,
# `open_agent_attachments` and `drive_terminal`). Default/legacy screenshots
# are debugging artifacts on failure; the appearance leg retains its own
# successful startup evidence too.
#
# Why this exists: ordinary CI compiles the desktop renderer, while this
# release gate runs it on native WebKitGTK. A compile-only surface is exactly
# how a bug that
# bricked every desktop create (MT-5, an eval channel dying under wry)
# survived to manual testing on macOS. This script runs the same wry +
# dioxus-desktop code paths on the same engine family (WebKitGTK), so that
# class of regression fails here first. It is NOT the final word on real
# WKWebView behavior — macOS-specific quirks still need a manual pass —
# while macOS-specific quirks still need the documented manual pass.
#
# Prereqs (apt): xvfb xdotool openbox imagemagick curl python3, plus the
# webkit2gtk dev stack the desktop feature already needs, dioxus-cli
# 0.7.10, the wasm32-unknown-unknown target (the run starts by building the
# web bundle, which is then embedded in the desktop build).
# Usage: scripts/desktop-smoke.sh   (from the repo root; ~5 min)
# Honors CARGO_TARGET_DIR, relative or absolute.
# DESKTOP_SMOKE_WEB_DIST=/abs/path/to/public skips the web build and embeds
# that already-built bundle instead (the release gate's use).
# DESKTOP_SMOKE_APPEARANCE=1 selects only the finite native appearance leg
# after the shared build/display setup. It retains bounded JSON and one final
# client screenshot per case. DOM state is not proof of presentation; normal
# appshots supply visual review. Linux observations are not macOS evidence.
# DESKTOP_SMOKE_APPEARANCE=no-initialize runs only the fixed rootless-document
# fault: the hidden native window must refuse within its creation deadline and
# the supervisor it started must keep running, detached, after the refusal.
#
# Known handling quirks, learned the hard way:
# - The window sometimes maps at 10x10 and stays black until a resize
#   nudge; the retry loop below resizes until xdotool reports a sane
#   geometry. A window manager (openbox) must be running or map/size
#   behavior is timing-dependent.
# - xdotool typing faster than ~10 chars/s drops keystrokes into the
#   controlled inputs (dioxus re-renders between keystrokes), so all
#   typing uses --delay 120.
# - An installed xdg-desktop-portal that cannot start (a headless box, where
#   its GTK backend has no display) stalls the app's GTK startup for D-Bus's
#   25-second activation timeout before the window exists, which lands the
#   first page load right at the webview-authentication wait below. The
#   release gate purges the portal packages for this reason (see
#   .github/dist-build-setup.yml); a desktop session's working portal, or no
#   portal at all, answers at once.
# - State dirs must be SHORT: unix socket paths cap at ~108 bytes.
# - The private Xvfb display has no X authentication set up (no
#   Xauthority cookie): `-nolisten tcp` keeps it off the network, but any
#   other local account on this box can still open the display's Unix
#   socket in /tmp/.X11-unix. Fine for a single-user dev box; a shared
#   box would need `-auth` plus a per-run Xauthority file — left as
#   future work, not done here.

set -u

REPO="$(cd "$(dirname "$0")/.." && pwd)"
APPEARANCE_MODE="${DESKTOP_SMOKE_APPEARANCE:-}"
case "$APPEARANCE_MODE" in
  ""|1|no-initialize) ;;
  *) echo "FAIL: invalid DESKTOP_SMOKE_APPEARANCE selection" >&2; exit 1 ;;
esac

# Preflight, before anything is created or spawned: every external binary
# the phases below shell out to. Local optional runs skip cleanly; CI must
# fail because a green integration gate that exercised nothing is false.
for tool in Xvfb xdotool openbox import convert curl python3 dx; do
  if ! command -v "$tool" >/dev/null; then
    if [ "${CI:-}" = true ]; then
      echo "FAIL: desktop-smoke requires $tool in CI" >&2
      exit 1
    fi
    echo "SKIPPED desktop-smoke: $tool not installed" >&2
    exit 0
  fi
done
API=""
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
  # The name is in farhelm-teststate's sweep family (`fh-smoke.` plus six
  # random characters, directly under /tmp because the supervisors' sockets
  # live inside it), so the sweep every Rust test process and browser stack
  # runs reclaims a run that was killed, and a kept failure once its keep
  # marker (written by the teardown below) is two days old. The old
  # `fhm-smoke.` name was outside the family, and its kept failures, each
  # carrying a copy of the debug binary, piled up with nothing to remove
  # them.
  X="$(mktemp -d /tmp/fh-smoke.XXXXXX)" || { echo "FAIL: mktemp failed" >&2; exit 1; }
  umask "$OLD_UMASK"
fi
# The run's liveness lock (farhelm-teststate's protocol), held while this
# script runs so the sweep never reaps a live run however long it takes.
# A separate holder process takes it, not a descriptor of this shell: every
# child this script starts would inherit such a descriptor, the supervisor
# deliberately outlives the app, and a supervisor holding the lock would make
# a dead run look alive forever. `flock -o` closes the lock descriptor before
# running `tail`, so only flock itself holds it, and `tail --pid` ends when
# this shell does, even by SIGKILL. The sweep's five-minute grace on a fresh
# lock file covers the moment before the holder has the lock. Its output goes
# to /dev/null so it never holds a pipe the test recorder waits on.
flock -o -w 10 "$X/fh-run.lock" tail --pid=$$ -f /dev/null </dev/null >/dev/null 2>&1 &
mkdir -p "$X/state" "$X/work"
DESKTOP_READY="$X/desktop-ready.json"

fail() {
  echo "FAIL: $*" >&2
  if [ -n "$APPEARANCE_MODE" ] && [ -n "${window:-}" ] && [ -n "${evidence:-}" ]; then
    # Keep a failure image inside the fixture's lifetime. If the window already
    # disappeared, retained logs and bounded observations remain available.
    DISPLAY=$DISP timeout 10 import -window "$window" "$evidence/failure-client.png" 2>/dev/null
  fi
  DISPLAY=$DISP import -window root "$X/failure.png" 2>/dev/null &&
    echo "screenshot: $X/failure.png" >&2
  exit 1
}

# Curl reads the bearer header from this private file so the secret never
# appears in a process listing. Every loopback request bypasses ambient proxy
# configuration as the desktop clients do.
#
# The secret is the desktop app's native in-memory credential, published only
# through the debug-only readiness seam. The embedded helm has no browser
# token exchange, so this is the only credential the smoke can use.
CURL_AUTH_CONFIG="$X/curl-auth.conf"
write_curl_auth() {
  printf 'header = "Authorization: Bearer %s"\n' "$SMOKE_SECRET" >"$CURL_AUTH_CONFIG"
  chmod 600 "$CURL_AUTH_CONFIG"
}
read_desktop_ready() {
  local ready="$1"
  API=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["base"])' "$ready") ||
    fail "reading the embedded helm address"
  SMOKE_SECRET=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["native_device_secret"])' "$ready") ||
    fail "reading the embedded helm credential"
  write_curl_auth
}
# The desktop app must never write a credential to its state file.
assert_no_persisted_credentials() {
  python3 -c '
import json,sys
state=json.load(open(sys.argv[1]))
leaked=[k for k in ("native_device_secret","webview_device_secret") if k in state]
sys.exit("desktop state file holds credentials: %s" % leaked if leaked else 0)
' "$X/state/desktop-client.json" || fail "the desktop app persisted a credential ($1)"
}
device_rows() {
  python3 -c 'import sqlite3,sys; print(sqlite3.connect(sys.argv[1]).execute("select count(*) from device_sessions").fetchone()[0])' "$X/state/helm.db"
}
curl_local() {
  curl --noproxy '*' "$@"
}
curl_auth() {
  curl --noproxy '*' --config "$CURL_AUTH_CONFIG" "$@"
}
# Whether session $1 (its local id) of the main state directory still has a
# live agent terminal; exit status only.
#
# Every session this script creates runs in a terminal host, whose socket the
# session's row names (`terminal_address`). The terminal is alive when a
# process still listens on that socket. The socket table is read rather than
# the socket connected to, because a host serves exactly one client, its
# supervisor, and a socket file outlives the host that bound it. The database
# is opened read-only beside the running supervisor.
session_terminal_alive() {
  python3 - "$X/state" "$1" <<'PY'
import os, sqlite3, sys
state, sid = sys.argv[1], sys.argv[2]
db = sqlite3.connect("file:" + os.path.join(state, "supervisor.db") + "?mode=ro", uri=True, timeout=5)
row = db.execute("select terminal_kind, terminal_address from sessions where id = ?", (sid,)).fetchone()
if row is None:
    sys.exit(1)
kind, address = row
if kind != "host" or not address:
    sys.exit(1)
socket = os.path.join(state, "hosts", address)
wanted = {socket, os.path.realpath(socket)}
LISTENING = 0x10000  # __SO_ACCEPTCON in /proc/net/unix's flags column
with open("/proc/net/unix") as table:
    next(table)
    for line in table:
        fields = line.split()
        if len(fields) >= 8 and int(fields[3], 16) & LISTENING and fields[7] in wanted:
            sys.exit(0)
sys.exit(1)
PY
}
# Fail unless session $1 (its local id) is a terminal-host row, the only kind
# a supervisor of this release creates. `session_terminal_alive` would also
# report another kind as not alive, but this names what went wrong.
expect_host_terminal() {
  local kind
  kind=$(python3 - "$X/state" "$1" <<'PY'
import os, sqlite3, sys
db = sqlite3.connect("file:" + os.path.join(sys.argv[1], "supervisor.db") + "?mode=ro", uri=True, timeout=5)
row = db.execute("select terminal_kind from sessions where id = ?", (sys.argv[2],)).fetchone()
print(row[0] if row else "missing")
PY
) || fail "reading session $1's terminal kind"
  [ "$kind" = host ] || fail "session $1's terminal is '$kind', not a terminal host: the app's supervisor did not create it in a host"
}
# How many attachments to session $2's agent terminal the app writing log $1
# has open, by its embedded helm's attachment trace: `terminal attached`
# events minus `terminal ended` ones on the `farhelm_helm::terminal_attachments`
# target (crates/farhelm-helm/src/terminal.rs, `ATTACHMENT_TRACE`; change both
# together), which the launches below turn on through RUST_LOG. $2 is the API
# id, as the page names the session in its terminal socket's URL. The fmt
# layer may color field names, so escapes are stripped before matching.
#
# These events are the only outside view of the page's terminal attachments
# for a terminal in a terminal host: such a host serves only its supervisor,
# so nothing outside it can list a terminal's clients. Open
# attachments rather than a running count, so that an attachment that came
# and went (the page moving on to another session, a socket closed by a
# rotation) does not count as one the page still holds.
open_agent_attachments() {
  python3 - "$1" "$2" <<'PY'
import re, sys
log, session = sys.argv[1], sys.argv[2]
open_count = 0
pattern = re.compile(r"farhelm_helm::terminal_attachments: terminal (attached|ended) session=(\S+) terminal=agent\b")
with open(log, errors="replace") as lines:
    for line in lines:
        match = pattern.search(re.sub(r"\x1b\[[0-9;]*m", "", line))
        if match and match.group(2) == session:
            open_count += 1 if match.group(1) == "attached" else -1
print(open_count)
PY
}
# How many `terminal attached` events for session $2's agent terminal log $1
# holds, open or not: what tells a socket that closed and came back from one
# that stayed, alongside `open_agent_attachments`. `terminal=agent` may end the
# line (the event carries no field after it), and a tab's value never starts
# with `agent`, so no trailing delimiter is needed.
agent_attachment_starts() {
  sed 's/\x1b\[[0-9;]*m//g' "$1" |
    grep -cF "farhelm_helm::terminal_attachments: terminal attached session=$2 terminal=agent"
}
# Type `printf smoke-typed-ok >$3` and Enter into session $1's agent terminal
# through the embedded helm at $2 (http://host:port) as a viewer that can draw
# grids, after the stream's replay, and wait for the file: proof that the
# supervisor serving the session can drive its terminal, which a listening
# socket and a listed row do not show (an unreadable host is listed and still
# listens). Speaks the browser's WebSocket by hand, as the Rust process tests
# do: the native credential travels in Sec-WebSocket-Protocol. Requires the
# stream to open with a grid `hello`, the first message of every grid
# attachment, so it also proves the grid stream reached the page.
drive_terminal() {
  python3 - "$1" "$2" "$3" "$SMOKE_SECRET" <<'PY'
import base64, json, os, socket, struct, sys, time
session, base, proof, secret = sys.argv[1:5]
host, port = base.split("//", 1)[1].rstrip("/").rsplit(":", 1)
deadline = time.monotonic() + 60
sock = socket.create_connection((host, int(port)), timeout=30)
key = base64.b64encode(os.urandom(16)).decode()
sock.sendall((f"GET /api/sessions/{session}/term HTTP/1.1\r\nHost: {host}:{port}\r\n"
              f"Connection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\n"
              f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Protocol: farhelm, farhelm-device-{secret}\r\n\r\n").encode())
head = b""
while not head.endswith(b"\r\n\r\n"):
    byte = sock.recv(1)
    if not byte:
        sys.exit("the helm closed during the upgrade")
    head += byte
if not head.startswith(b"HTTP/1.1 101"):
    sys.exit("upgrade refused: " + head.decode(errors="replace").splitlines()[0])
def recv_exact(n):
    data = b""
    while len(data) < n:
        part = sock.recv(n - len(data))
        if not part:
            sys.exit("the helm closed the terminal socket")
        data += part
    return data
def message():
    payload, opcode = b"", None
    while True:
        b0, b1 = recv_exact(2)
        length = b1 & 0x7F
        if length == 126:
            length = struct.unpack(">H", recv_exact(2))[0]
        elif length == 127:
            length = struct.unpack(">Q", recv_exact(8))[0]
        part = recv_exact(length)
        code = b0 & 0x0F
        if code == 8:
            sys.exit("the helm closed the terminal socket")
        if code in (9, 10):
            continue
        if code:
            opcode = code
        payload += part
        if b0 & 0x80:
            return opcode, payload
def send(payload):
    mask = os.urandom(4)
    frame = bytes([0x82])
    n = len(payload)
    frame += bytes([0x80 | n]) if n < 126 else bytes([0x80 | 126]) + struct.pack(">H", n)
    sock.sendall(frame + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(payload)))
hello = None
while time.monotonic() < deadline:
    opcode, payload = message()
    if opcode != 1:
        # A grid message: binary rows start with a zero byte, the rest is JSON.
        if hello is None:
            hello = payload[:1] != b"\0" and json.loads(payload).get("t") == "hello"
        continue
    text = json.loads(payload)
    if text.get("type") == "detached":
        sys.exit(f"detached: {text}")
    elif text.get("type") == "replay_complete":
        break
else:
    sys.exit("no replay_complete within the bound")
if not hello:
    sys.exit("the terminal stream did not open with a grid hello")
send(json.dumps({"t": "text", "data": f"printf smoke-typed-ok >{proof}"}).encode())
send(json.dumps({"t": "key", "key": "Enter", "code": "Enter", "kc": 13, "mods": 0}).encode())
while time.monotonic() < deadline:
    try:
        if open(proof).read() == "smoke-typed-ok":
            sys.exit(0)
    except FileNotFoundError:
        pass
    time.sleep(0.2)  # sleep-ok: polling interval for the typed command's file, bounded by the deadline above
sys.exit("the typed command never wrote its file")
PY
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
SID_ROTATED="" # post-rotation fixture: proves native REST still works; read by teardown
SID_KEPT="" # held across the supervisor replacement to prove sessions survive it; read by teardown
# The helm's API IDs include their host entry; systemd scopes belong to this
# run's local supervisor and contain only the session component.
# Keep both spellings so API cleanup never loses the routing identity.
LOCAL_SID=""
LOCAL_SID_NEWEST=""
# Set by the answering-supervisor leg, which starts its own supervisor and
# desktop outside the main launch's state directory. Each is
# cleared once stop_answering has stopped what it names, so a pid is never
# signalled after it has been reaped (and possibly reused).
ANSWERING_STATE=""
ANSWERING_SUPERVISOR_PID=""
ANSWERING_DESKTOP_PID=""
# Stops everything the answering-supervisor leg started; the leg calls it on
# success and teardown calls it on any exit, and a second call does nothing.
#
# The supervisor runs in its own process group (the leg starts it under
# setsid) and the whole group is stopped and waited for, not just the
# supervisor's pid. A supervisor killed during startup, before it installs its
# SIGTERM handler, dies at once and leaves any in-flight child behind. Its
# terminal hosts daemonize out of the group and outlive the supervisor by
# design (sessions survive a supervisor restart), so they are stopped through
# their state directory afterwards, before teardown's `rm -r` deletes it.
stop_answering() {
  if [ -n "$ANSWERING_DESKTOP_PID" ]; then
    kill "$ANSWERING_DESKTOP_PID" 2>/dev/null
    wait "$ANSWERING_DESKTOP_PID" 2>/dev/null
    ANSWERING_DESKTOP_PID=""
  fi
  if [ -n "$ANSWERING_SUPERVISOR_PID" ]; then
    kill -TERM -- "-$ANSWERING_SUPERVISOR_PID" 2>/dev/null
    for _ in $(seq 1 50); do
      kill -0 -- "-$ANSWERING_SUPERVISOR_PID" 2>/dev/null || break
      sleep 0.2
    done
    kill -KILL -- "-$ANSWERING_SUPERVISOR_PID" 2>/dev/null
    wait "$ANSWERING_SUPERVISOR_PID" 2>/dev/null
    ANSWERING_SUPERVISOR_PID=""
  fi
  if [ -n "$ANSWERING_STATE" ]; then
    stop_terminal_hosts "$ANSWERING_STATE"
    ANSWERING_STATE=""
  fi
}
PASS=""
APPEARANCE_STATES=()
APPEARANCE_PROFILES=()
# State directories whose terminal hosts could not be confirmed stopped.
# Teardown keeps the whole run directory when any is listed, pass or fail.
HOSTS_NOT_STOPPED=()

# Ends every terminal host whose socket is in state directory $1, after the
# supervisor that may have started them is gone: hosts outlive their
# supervisor by design (its stop leaves them running), so nothing else ends
# them. `farhelm internal stop-terminal-hosts` is the command an uninstall
# runs (SPEC_impl.md, "Terminal host"); it exits 0 only when every host is
# confirmed gone, and needs an absolute state directory. A directory with no
# `hosts` entry never had a host and costs nothing. Anything else is listed
# in HOSTS_NOT_STOPPED: removing that directory would delete the sockets
# that are the only way left to reach and end those hosts, so teardown keeps
# it, and the rerun command is printed. The command's per-host lines are
# appended to a log in the run directory. Bounded twice: by the command's
# own 30 s budget, and by `timeout` against a wedge.
stop_terminal_hosts() {
  local dir="$1"
  [ -e "$dir/hosts" ] || [ -L "$dir/hosts" ] || return 0
  case "$dir" in
    /*) ;;
    *) dir="$PWD/$dir" ;;
  esac
  if [ -n "${BUILT_FARHELM:-}" ] &&
    timeout 60 "$BUILT_FARHELM" internal stop-terminal-hosts --state-dir "$dir" \
      </dev/null >/dev/null 2>>"$X/stop-terminal-hosts.log"; then
    return 0
  fi
  HOSTS_NOT_STOPPED+=("$dir")
  echo "terminal hosts in $dir could not be confirmed stopped (see $X/stop-terminal-hosts.log);" \
    "retry with: ${BUILT_FARHELM:-farhelm} internal stop-terminal-hosts --state-dir $dir" >&2
  return 1
}

# Records state directory $1 as not scanned for terminal hosts, because the
# supervisor that could still start one there was not confirmed gone. Like
# a failed stop, it keeps the run directory, with or without a `hosts`
# entry yet: that supervisor may still make one.
hosts_left_unscanned() {
  HOSTS_NOT_STOPPED+=("$1")
  echo "the supervisor in $1 was not confirmed stopped, so its terminal hosts were not stopped either" >&2
}

# Stops the supervisor a desktop app started in state directory $1, by the
# record the app keeps there (`desktop-supervisor.json`), which is the only
# handle on it: the app starts it detached and it outlives the app. The pid is
# signalled only while its kernel start time and executable still match the
# record, so a reused pid is never hit; SIGTERM first (the orderly stop), and
# SIGKILL only if it is still running 20 s later. Nothing to do without a
# record, or when its process is already gone.
stop_recorded_supervisor() {
  [ -f "$1/desktop-supervisor.json" ] || return 0
  python3 - "$1/desktop-supervisor.json" <<'PY'
import json, os, signal, sys, time
from pathlib import Path

record = json.loads(Path(sys.argv[1]).read_text())

def same_live_process():
    """Only the recorded birth, still running the recorded program."""
    try:
        fields = Path(f'/proc/{record["pid"]}/stat').read_text().rsplit(') ', 1)[1].split()
        if fields[0] == 'Z' or int(fields[19]) != record['start']:
            return False
        image = os.readlink(f'/proc/{record["pid"]}/exe').removesuffix(' (deleted)')
        return image == record['program']
    except (FileNotFoundError, ProcessLookupError):
        return False

for sig, wait in ((signal.SIGTERM, 20), (signal.SIGKILL, 5)):
    if not same_live_process():
        sys.exit(0)
    try:
        os.kill(record['pid'], sig)
    except ProcessLookupError:
        sys.exit(0)
    deadline = time.monotonic() + wait
    while same_live_process() and time.monotonic() < deadline:
        # sleep-ok: bounded wait for the recorded supervisor to exit.
        time.sleep(0.1)
sys.exit(1 if same_live_process() else 0)
PY
}

teardown() {
  [ -n "$TEARDOWN_DONE" ] && return
  TEARDOWN_DONE=1

  # Delete THIS run's session through its own helm while the helm is
  # still answering, so the supervisor's normal delete path (process-tree
  # kill, ending the terminal host, systemd scope stop) runs first and the daemon
  # kills below and the unit-stop fallback further down are usually
  # no-ops. Best-effort: teardown must proceed even if the helm is
  # already gone or wedged.
  # 30 s, not 5: a delete of a session whose shell ignores SIGTERM (an
  # interactive `bash`, which these smoke sessions run) waits the supervisor's
  # full SIGTERM grace (`KILL_GRACE` in sweep.rs, 5 s) before the SIGKILL, so
  # the request itself takes a little over 5 s; see the cleanup below.
  [ -n "$SID" ] && [ -s "$CURL_AUTH_CONFIG" ] && curl_auth -s --max-time 30 -X DELETE "$API/api/sessions/$SID" >/dev/null 2>&1
  [ -n "$SID_NEWEST" ] && [ -s "$CURL_AUTH_CONFIG" ] && curl_auth -s --max-time 30 -X DELETE "$API/api/sessions/$SID_NEWEST" >/dev/null 2>&1
  [ -n "$SID_ROTATED" ] && [ -s "$CURL_AUTH_CONFIG" ] && curl_auth -s --max-time 30 -X DELETE "$API/api/sessions/$SID_ROTATED" >/dev/null 2>&1
  [ -n "$SID_KEPT" ] && [ -s "$CURL_AUTH_CONFIG" ] && curl_auth -s --max-time 30 -X DELETE "$API/api/sessions/$SID_KEPT" >/dev/null 2>&1

  for p in desktop openbox xvfb; do
    [ -f "$X/$p.pid" ] && kill "$(cat "$X/$p.pid")" 2>/dev/null
  done
  # The app's supervisor outlives the app, so it is stopped here by its
  # record. Then its terminal hosts, which a
  # supervisor's stop leaves running, but only once that supervisor is
  # confirmed gone: one still running could start a host after the scan.
  for state in "$X/state" "${APPEARANCE_STATES[@]}"; do
    if stop_recorded_supervisor "$state"; then
      stop_terminal_hosts "$state"
    else
      hosts_left_unscanned "$state"
    fi
  done
  stop_answering

  # Last-resort scope cleanup, scoped to THIS run's session id only. A
  # bare `farhelm-*` glob here would stop every farhelm session on the
  # box, including ones this run never touched (the review swarm's worst
  # finding on an earlier version of this script). Session ids are full
  # UUIDv4s, so one can never be a prefix of another's — the glob cannot
  # catch a stranger's unit. Harmless no-op if the API delete above
  # already tore the scope down, or no systemd user manager exists.
  if [ -n "$LOCAL_SID" ]; then
    systemctl --user list-units "farhelm-$LOCAL_SID-*" --no-legend --plain 2>/dev/null |
      awk '{print $1}' | xargs -r -n1 systemctl --user stop 2>/dev/null
  fi

  # Every branch that keeps the directory writes farhelm-teststate's keep
  # marker first. The lock above is released when this script exits, and
  # without the marker the next sweep would reap the evidence minutes later;
  # with it, the sweep leaves the directory for two days and then reclaims it
  # (ending any terminal hosts still running in it).
  if [ "${#HOSTS_NOT_STOPPED[@]}" -gt 0 ]; then
    : >"$X/fh-keep"
    echo "state kept at $X: terminal hosts may still run in ${HOSTS_NOT_STOPPED[*]}" >&2
  elif [ -n "$PASS" ] && [ -n "$APPEARANCE_MODE" ]; then
    # The profile is disposable fixture state; the retained images and JSON are
    # the evidence. A failure keeps everything for diagnosis instead.
    for state in "${APPEARANCE_STATES[@]}" "${APPEARANCE_PROFILES[@]}"; do
      rm -r -- "$state"
    done
    rm -f -- "$CURL_AUTH_CONFIG" "$DESKTOP_READY"
    : >"$X/fh-keep"
    echo "appearance evidence kept at $X for two days" >&2
  elif [ -n "$PASS" ]; then
    rm -r -- "$X"
  else
    : >"$X/fh-keep"
    echo "state kept at $X for two days" >&2
  fi
}
trap teardown EXIT
trap 'exit 143' INT TERM

# Honor CARGO_TARGET_DIR the way cargo does, resolved ONCE to an absolute
# path and exported. A checkout or the release gate's job may point it at a
# target directory outside the tree, so this script has to follow it; CI
# leaves it unset, where this is exactly the old `$REPO/target`.
#
# The normalization is not tidiness. A relative value is legal, and cargo
# resolves it per process working directory — while this script runs cargo
# from the repository root and both dx builds from `crates/farhelm-ui`, then
# checks the resulting paths from wherever the caller stood. Left relative,
# one setting would name three different trees. `FARHELM_UI_DIST` has a
# harder requirement still: `farhelm-helm`'s build.rs rejects a relative
# value outright, because build scripts run from their own crate directory.
if [ -n "${CARGO_TARGET_DIR:-}" ]; then
  mkdir -p "$CARGO_TARGET_DIR" || fail "creating CARGO_TARGET_DIR ($CARGO_TARGET_DIR)"
  CARGO_TARGET_DIR="$(cd "$CARGO_TARGET_DIR" && pwd)" || fail "resolving CARGO_TARGET_DIR"
else
  CARGO_TARGET_DIR="$REPO/target"
fi
export CARGO_TARGET_DIR
TARGET_DIR="$CARGO_TARGET_DIR"

# DESKTOP_SMOKE_WEB_DIST names a web bundle that is already built, which the
# smoke then embeds and serves instead of building its own. The release gate
# sets it to the bundle the release ships, so the smoke tests those exact
# bytes and the job builds the bundle once (.github/dist-build-setup.yml,
# "Run the native desktop smoke gate"). Unset, the smoke builds the bundle
# itself, as a local run needs.
if [ -n "${DESKTOP_SMOKE_WEB_DIST:-}" ]; then
  case "$DESKTOP_SMOKE_WEB_DIST" in
    /*) ;;
    # farhelm-helm's build.rs rejects a relative FARHELM_UI_DIST.
    *) fail "DESKTOP_SMOKE_WEB_DIST must be an absolute path ($DESKTOP_SMOKE_WEB_DIST)" ;;
  esac
  [ -f "$DESKTOP_SMOKE_WEB_DIST/index.html" ] || fail "DESKTOP_SMOKE_WEB_DIST has no index.html ($DESKTOP_SMOKE_WEB_DIST)"
  WEB_DIST="$DESKTOP_SMOKE_WEB_DIST"
else
  WEB_DIST="$TARGET_DIR/dx/farhelm-ui/release/web/public"
fi

echo "== building (cargo + web bundle + dx desktop build)"
(cd "$REPO" && flock --close /tmp/fh-build.lock -c 'ulimit -c unlimited && cargo build --quiet') || fail "cargo build"
# `--package farhelm-ui` is not decoration. The workspace has `default-members`
# (it excludes `farhelm-desktop` so ordinary builds never compile WebKit), and
# dx 0.7.10 resolves those entries by canonicalizing each RELATIVE member path
# against its own working directory — from `crates/farhelm-ui` that is
# `crates/farhelm-ui/crates/farhelm`, which does not exist, and dx panics on
# the `unwrap` (`packages/cli/src/workspace.rs`). Naming the package takes an
# earlier return that never reads `default-members` at all.
if [ -z "${DESKTOP_SMOKE_WEB_DIST:-}" ]; then
  (cd "$REPO/crates/farhelm-ui" && flock --close /tmp/fh-build.lock -c 'ulimit -c unlimited && dx build --package farhelm-ui --platform web --release' >"$X/dx-web.log" 2>&1) || fail "dx web build (see $X/dx-web.log)"
fi
# FARHELM_UI_DIST makes this a release-SHAPED desktop build (D12/D13): the web
# bundle built a moment ago is compiled into `farhelm-helm`, and it is that
# embedded tree — not the `assets/` directory dx leaves beside the binary —
# that the desktop asset handler serves to the window (D6). Building without
# it would leave the handler with nothing to serve and every asset request
# 404ing, which is precisely what the assertion further down checks for.
(cd "$REPO/crates/farhelm-ui" && FARHELM_UI_DIST="$WEB_DIST" flock --close /tmp/fh-build.lock -c 'ulimit -c unlimited && dx build --package farhelm-ui --platform desktop' >"$X/dx.log" 2>&1) || fail "dx desktop build (see $X/dx.log)"
APP="$TARGET_DIR/dx/farhelm-ui/debug/linux/app/farhelm-ui"
[ -x "$APP" ] || fail "desktop app missing at $APP"

# The app runs where dx built it, with both siblings named explicitly. D6
# retired the `Farhelm.app/Contents` staging this used to do: there is no
# bundle any more, just two bare binaries that a release installs side by side
# — and relocating the dx output would only reintroduce the asset-resolution
# trap that staging existed to work around.
BUILT_FARHELM="$TARGET_DIR/debug/farhelm"
[ -x "$BUILT_FARHELM" ] || fail "farhelm CLI missing at $BUILT_FARHELM"

if [ -z "$APPEARANCE_MODE" ]; then
# One plain stderr line and exit status 1, never a panic, is the desktop
# app's whole contract for a bootstrap failure. This leg needs neither Xvfb
# nor the interaction phase: `ensure_private_dir` is the very first fallible
# step `DesktopBootstrap::start` takes, well before any window, display or web
# asset is requested, so exercising the entry point headless proves the SAME
# refusal path a Finder launch would hit. A state "directory" that is actually
# a plain file is the cheapest way to reach it: `DirBuilder::create` fails at
# once with the OS's own error text, which is why this leg checks the shape of
# the message rather than its exact wording.
#
# The binary under test is the dx-built `farhelm-ui` desktop app the rest of
# this script drives rather than a separately Cargo-built `farhelm-desktop`:
# the shipped `farhelm-desktop` crate is a one-line wrapper whose `main` calls
# the very same `farhelm_ui::desktop::run()`, while a second WebKit build of
# the wrapper crate on top of the dx build is what pushed CI's runner out of
# disk (PR #276's first run: "No space left on device" inside `dx build`).
# The sibling `farhelm` is named explicitly, and the state directory is this
# run's own, so nothing falls back to the maintainer's live Farhelm state.
BUILT_FARHELM_DESKTOP="$APP"
[ -x "$BUILT_FARHELM_DESKTOP" ] || fail "desktop app missing at $BUILT_FARHELM_DESKTOP"

echo "== a bootstrap failure exits 1 with a plain message, no panic"
BOGUS_STATE="$X/pf-bogus-state"
: >"$BOGUS_STATE"
BOGUS_STATE_STDERR="$X/pf-bogus-state.stderr"
FARHELM_DESKTOP_STATE_DIR="$BOGUS_STATE" \
  FARHELM_DESKTOP_FARHELM="$BUILT_FARHELM" \
  "$BUILT_FARHELM_DESKTOP" >/dev/null 2>"$BOGUS_STATE_STDERR"
BOGUS_STATE_STATUS=$?
[ "$BOGUS_STATE_STATUS" -eq 1 ] ||
  fail "farhelm-desktop exited $BOGUS_STATE_STATUS against an unusable state directory (expected exactly 1)"
grep -q "^farhelm-desktop: " "$BOGUS_STATE_STDERR" ||
  fail "expected a plain 'farhelm-desktop: ...' line (see $BOGUS_STATE_STDERR)"
for banned in panicked RUST_BACKTRACE; do
  if grep -qF "$banned" "$BOGUS_STATE_STDERR"; then
    fail "a bootstrap failure must never contain $banned (see $BOGUS_STATE_STDERR)"
  fi
done
fi

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

# Unlike the four appearance cases, this intentionally never initializes App.
# Observe real hidden native creation and the child before the independent
# deadline; an early bootstrap failure with the same exit status cannot pass.
if [ "$APPEARANCE_MODE" = no-initialize ]; then
  evidence="$X/no-initialize"
  profile="$X/no-initialize-profile"
  appearance_state="$X/no-initialize-state"
  mkdir -m 0700 "$evidence" "$profile" "$appearance_state" || fail "creating missing-initialize fixture"
  APPEARANCE_PROFILES+=("$profile")
  APPEARANCE_STATES+=("$appearance_state")
  # Failure cleanup requires PID-bound signalling; no name-based process search
  # or signal can substitute for the actual child identity retained below.
  python3 -c 'import os, signal; fd=os.pidfd_open(os.getpid()); os.close(fd); assert hasattr(signal, "pidfd_send_signal")' || fail "missing-initialize smoke requires Linux pidfd support"
  rm -f "$DESKTOP_READY"
  DISPLAY="$DISP" GTK_THEME="Adwaita:dark" NO_COLOR=1 PATH="/usr/bin:/bin" \
    FARHELM_DESKTOP_FARHELM="$BUILT_FARHELM" \
    FARHELM_DESKTOP_STATE_DIR="$appearance_state" FARHELM_DESKTOP_SMOKE_READY="$DESKTOP_READY" \
    FARHELM_SMOKE_APPEARANCE=no-initialize FARHELM_SMOKE_WEBVIEW_DATA_DIR="$profile" \
    RUST_LOG="info,farhelm_ui::desktop=debug,desktop_startup=debug" \
    "$APP" >"$evidence/desktop.log" 2>&1 &
  desktop_pid=$!
  echo "$desktop_pid" >"$X/desktop.pid"
  # Python is already the harness's process/JSON tool. Keep its bounded Linux
  # identity observation here with the owned launch, rather than adding another
  # application-control service. It reads no credentials or process environment.
  DISPLAY="$DISP" python3 - "$desktop_pid" "$BUILT_FARHELM" "$appearance_state" "$evidence" <<'PY'
import json, os, re, subprocess, sys, time
from pathlib import Path

desktop = int(sys.argv[1])
program, state, evidence = Path(sys.argv[2]).resolve(), Path(sys.argv[3]), Path(sys.argv[4])
result = {"ok": False, "minimumPollDelayMs": 100, "visibleSamples": 0,
          "coverage": "sampled no observed mapped client; no continuous X-event assertion",
          "xid": None, "xidObservation": "not discovered; realization unknown"}

def process(pid):
    """Read birth identity before trusting a PID, including nonrunning zombies."""
    root = Path(f'/proc/{pid}')
    try:
        fields = (root / 'stat').read_text().rsplit(') ', 1)[1].split()
        record = {"pid": pid, "state": fields[0], "ppid": int(fields[1]), "startTicks": int(fields[19])}
        if record["state"] != 'Z':
            record["exe"] = os.readlink(root / 'exe')
            raw = (root / 'cmdline').read_bytes()
            assert len(raw) < 65536, 'unexpectedly large owned process argv'
            record["argv"] = [part.decode() for part in raw.split(b'\0') if part]
        return record
    except FileNotFoundError:
        return None

def live(record):
    """A zombie has exited even if its new parent has not reaped it yet."""
    return record is not None and record['state'] != 'Z'

def startup_log():
    """Read only the fixed observation cap, even when a failing child floods stderr."""
    with (evidence / 'desktop.log').open() as stream:
        log = stream.read(1048577)
    assert len(log) <= 1048576, 'fault log exceeded retained observation cap'
    return log

def windows(visible=False):
    """Query only this owned desktop; timeout and output caps bound each sample."""
    args = ['xdotool', 'search'] + (['--onlyvisible'] if visible else [])
    observed = subprocess.run(args + ['--pid', str(desktop), '--name', '^farhelm$'],
                              capture_output=True, text=True, timeout=2, check=False)
    assert observed.returncode in (0, 1), 'xdotool could not inspect owned windows'
    assert len(observed.stdout) < 4096, 'unexpectedly many owned windows'
    return observed.stdout.split()

def identify_supervisor():
    """Prove direct ownership, exact private state, and the detached setup."""
    children = Path(f'/proc/{desktop}/task/{desktop}/children').read_text().split()
    assert len(children) <= 128, 'unexpected desktop child count'
    matches = []
    for child in children:
        candidate = process(int(child))
        if not live(candidate):
            continue
        if candidate.get('argv', [])[1:] == ['supervisor', 'run', '--state-dir', str(state)]:
            assert candidate['ppid'] == desktop and Path(candidate['exe']).resolve() == program, 'managed child executable/parent mismatch'
            matches.append(candidate)
    assert len(matches) == 1, 'expected exactly one live managed supervisor child'
    owned = matches[0]
    # Detached: a session of its own, no terminal-facing stdio, and its
    # diagnostics in its own log file in the state directory.
    session = int(Path(f'/proc/{owned["pid"]}/stat').read_text().rsplit(') ', 1)[1].split()[3])
    assert session == owned['pid'], 'managed supervisor does not lead a session of its own'
    assert os.readlink(f'/proc/{owned["pid"]}/fd/0') == '/dev/null', 'managed supervisor stdin is not /dev/null'
    assert os.readlink(f'/proc/{owned["pid"]}/fd/2') == str((state / 'supervisor.log').resolve()), 'managed supervisor stderr is not its log file'
    record = json.loads((state / 'desktop-supervisor.json').read_text())
    assert (record['pid'], record['start'], record['program']) == (owned['pid'], owned['startTicks'], str(program)), 'the app did not record the supervisor it started'
    owned.update(session=session)
    (evidence / 'supervisor.json').write_text(json.dumps(owned, indent=2) + '\n')
    return owned

try:
    original = process(desktop)
    if not live(original):
        (evidence.parent / 'desktop.pid').unlink(missing_ok=True)
    assert live(original), 'desktop exited before fault observation began'
    started, created_at = time.monotonic(), None
    owned = None
    log = ''
    while True:
        current = process(desktop)
        if not live(current):
            # Bash may reap its background child while this observer runs.
            # Retire the raw cleanup handle before any post-exit assertion.
            (evidence.parent / 'desktop.pid').unlink(missing_ok=True)
            break
        if current['startTicks'] != original['startTicks']:
            (evidence.parent / 'desktop.pid').unlink(missing_ok=True)
        assert current['startTicks'] == original['startTicks'], 'desktop PID identity changed'
        log = startup_log()
        # Native creation is a Tao/GTK object boundary. It need not have an
        # XID until realization, so X11 discovery cannot be that premise.
        creation = re.search(r'native creation state.*\bwindow=(.*?)\s+visible=false(?:\s|$)', log)
        if creation and 'hidden native window created' in log and created_at is None:
            created_at = time.monotonic()
            result['createdObservedAt'] = created_at
            result['nativeCreation'] = {"taoWindow": creation.group(1), "visible": False}
            owned = identify_supervisor()
            result['supervisorBeforeRefusal'] = owned
        now = time.monotonic()
        assert now < (created_at + 35 if created_at is not None else started + 60), 'desktop survived the native startup observation deadline'
        assert not windows(visible=True), 'missing-initialize client became visible'
        result['visibleSamples'] += 1
        if result['xid'] is None:
            # An absent result is retained as unknown realization state. Never
            # realize/show a native window merely to obtain an evidence handle.
            hidden = windows()
            if hidden:
                assert len(hidden) == 1, 'ambiguous hidden native window identity'
                result.update(xid=hidden[0], xidObservation='owned hidden XID discovered')
                geometry = subprocess.run(['xdotool', 'getwindowgeometry', '--shell', hidden[0]],
                                          capture_output=True, text=True, timeout=2, check=True)
                assert len(geometry.stdout) < 4096, 'unexpected native geometry output'
                (evidence / 'client-geometry.txt').write_text(geometry.stdout)
        if owned is not None:
            child = process(owned['pid'])
            if not live(child) or child['startTicks'] != owned['startTicks']:
                # The desktop can exit between samples; recheck it before
                # calling the supervisor's exit premature.
                current = process(desktop)
                if not live(current):
                    (evidence.parent / 'desktop.pid').unlink(missing_ok=True)
                    break
                if current['startTicks'] != original['startTicks']:
                    (evidence.parent / 'desktop.pid').unlink(missing_ok=True)
                    raise AssertionError('desktop PID identity changed')
                raise AssertionError('managed supervisor died before desktop refusal')
        # sleep-ok: bounded sampling of owned process identity and X11 visibility; never a paint guarantee.
        time.sleep(0.1)
    exited_at = time.monotonic()
    assert created_at is not None and 'nativeCreation' in result and owned is not None, 'missing native hidden-creation/owned-child premise'
    elapsed = exited_at - created_at
    result.update(exitedObservedAt=exited_at, secondsAfterObservedCreation=elapsed)
    assert 28.5 <= elapsed <= 35, 'refusal did not match the 30-second creation deadline within observation tolerance'
    log = startup_log()
    refusal = 'farhelm-desktop: native window startup did not finish within 30 seconds'
    assert log.count(refusal) == 1, 'expected exactly one native startup deadline refusal'
    assert 'desktop_no_initialize: configured rootless index' in log, 'rootless fixture was not configured'
    for forbidden in ['desktop_no_initialize: unexpected App root', 'root content acknowledged',
                      'requesting reveal', 'focusing once', 'panicked', 'RUST_BACKTRACE']:
        assert forbidden not in log, f'unexpected fault-path marker: {forbidden}'
    stored = json.loads((state / 'desktop-client.json').read_text())
    assert stored['webview_auth_generation'] == 0, 'webview authenticated despite missing initialize'
    # The refusal is a quit like any other: the detached supervisor keeps
    # running after it. Teardown stops it by its record.
    outlived_until = time.monotonic() + 3
    while time.monotonic() < outlived_until:
        child = process(owned['pid'])
        assert live(child) and child['startTicks'] == owned['startTicks'], 'managed supervisor did not outlive the refused desktop'
        # sleep-ok: bounded observation that the detached supervisor survives the refusal.
        time.sleep(0.1)
    result['supervisorAfterRefusal'] = process(owned['pid'])
    result['supervisorOutlivedRefusal'] = True
    result['ok'] = True
except Exception as error:
    result['error'] = str(error)
    raise
finally:
    (evidence / 'observation.json').write_text(json.dumps(result, indent=2) + '\n')
PY
  observation_status=$?
  [ "$observation_status" -eq 0 ] || fail "missing-initialize lifetime observation failed (see $evidence)"
  wait "$desktop_pid"
  desktop_status=$?
  printf '%s\n' "$desktop_status" >"$evidence/desktop-exit-status"
  # Only this shell can reap its direct child. Bind that actual exit status to
  # the process/pipe observations before presenting the JSON as a passing leg.
  python3 - "$evidence/observation.json" "$desktop_status" <<'PY'
import json, sys
from pathlib import Path
path = Path(sys.argv[1])
result = json.loads(path.read_text())
result['desktopExitStatus'] = int(sys.argv[2])
result['ok'] = result['ok'] and result['desktopExitStatus'] == 1
path.write_text(json.dumps(result, indent=2) + '\n')
PY
  [ "$?" -eq 0 ] || fail "recording actual missing-initialize exit status"
  rm -f "$X/desktop.pid"
  [ "$desktop_status" -eq 1 ] || fail "missing-initialize app did not naturally refuse with exit status 1"
  printf '%s\n' 'Native creation deadline refused before App initialization; its detached supervisor kept running.' \
    'No mapped client observed by bounded X11 polling; no continuous event assertion.' \
    'Linux-only fault evidence, not ordinary appearance or macOS paint coverage.' >"$evidence/assessment.txt"
  echo "missing-initialize native deadline/detached-supervisor assertions passed; evidence kept at $evidence"
  PASS=1
  exit 0
fi

# This is an opt-in finite integration experiment, not a second native command
# service. The debug app contains the only driver, with fixed Settings/radio
# actions. Child-only GTK inputs request two startup signals; the app must
# establish the actual matchMedia premise before any palette assertion passes.
if [ "$APPEARANCE_MODE" = 1 ]; then
  # Keep a closed selection rather than accepting arbitrary scenario strings.
  # The ordinary mode still proves each fresh profile and its stored relaunch.
  appearance_schemes=(dark light)
  appearance_lifetimes=(fresh stored)
  for scheme in "${appearance_schemes[@]}"; do
    profile="$X/appearance-$scheme-profile"
    appearance_state="$X/appearance-$scheme-state"
    mkdir -m 0700 "$profile" "$appearance_state" || fail "creating private appearance profile/state"
    APPEARANCE_PROFILES+=("$profile")
    APPEARANCE_STATES+=("$appearance_state")
    for lifetime in "${appearance_lifetimes[@]}"; do
      window=""
      selection="$scheme-$lifetime"
      evidence="$X/appearance-$selection"
      mkdir -m 0700 "$evidence" || fail "creating appearance evidence directory"
      if [ "$scheme" = dark ]; then gtk_theme="Adwaita:dark"; else gtk_theme="Adwaita"; fi

      rm -f "$DESKTOP_READY"
      DISPLAY="$DISP" GTK_THEME="$gtk_theme" PATH="/usr/bin:/bin" \
        FARHELM_DESKTOP_FARHELM="$BUILT_FARHELM" \
        FARHELM_DESKTOP_STATE_DIR="$appearance_state" FARHELM_DESKTOP_SMOKE_READY="$DESKTOP_READY" \
        FARHELM_SMOKE_APPEARANCE="$selection" FARHELM_SMOKE_WEBVIEW_DATA_DIR="$profile" \
        RUST_LOG="info,farhelm_ui::desktop=debug,desktop_startup=debug" \
        "$APP" >"$evidence/desktop.log" 2>&1 &
      desktop_pid=$!
      echo "$desktop_pid" >"$X/desktop.pid"
      window=""
      for _ in $(seq 1 300); do
        kill -0 "$desktop_pid" 2>/dev/null || fail "appearance app exited before mapping"
        window=$(DISPLAY="$DISP" xdotool search --onlyvisible --pid "$desktop_pid" --name '^farhelm$' 2>/dev/null | head -n1)
        [ -n "$window" ] && break
        sleep 0.1
      done
      [ -n "$window" ] || fail "appearance app never mapped a visible window"
      printf 'window=%s\npid=%s\ndisplay=%s\nrequested_gtk_theme=%s\n' \
        "$window" "$desktop_pid" "$DISP" "$gtk_theme" >"$evidence/window.txt"
      DISPLAY="$DISP" xdotool getwindowgeometry --shell "$window" >"$evidence/client-geometry.txt" || fail "reading owned client geometry"
      for _ in $(seq 1 300); do
        [ -s "$DESKTOP_READY" ] && break
        kill -0 "$desktop_pid" 2>/dev/null || fail "appearance app exited before helm readiness"
        sleep 0.1
      done
      [ -s "$DESKTOP_READY" ] || fail "appearance app did not publish readiness"
      read_desktop_ready "$DESKTOP_READY"
      connected=""
      for _ in $(seq 1 100); do
        if curl_auth -sf --max-time 2 "$API/api/hosts" | python3 -c \
          'import json,sys; assert any(h["kind"] == "local" and h["state"]["phase"] == "connected" for h in json.load(sys.stdin)["hosts"])' 2>/dev/null; then
          connected=1
          break
        fi
        sleep 0.1
      done
      [ -n "$connected" ] || fail "appearance fixture local host never connected"
      body=$(python3 -c 'import json,sys; print(json.dumps({"cwd":sys.argv[1],"command":{"command":"bash","yolo":False},"title":"appearance fixture"}))' "$X/work") || fail "encoding appearance session"
      SID=$(curl_auth -sf --max-time 10 -H 'content-type: application/json' -d "$body" "$API/api/sessions" |
        python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])') || fail "creating appearance session"
      LOCAL_SID="${SID#*:}"
      observed=""
      for _ in $(seq 1 750); do
        # A long JSON record takes several writes. The prefix can be visible
        # while its final string is still incomplete; this marker follows the
        # completed write on the same stderr stream.
        if grep -q '^desktop_appearance_smoke_done$' "$evidence/desktop.log"; then observed=1; break; fi
        kill -0 "$desktop_pid" 2>/dev/null || fail "appearance app exited before observation"
        sleep 0.1
      done
      [ -n "$observed" ] || fail "native appearance observation timed out"
      sed -n 's/^desktop_appearance_smoke: //p' "$evidence/desktop.log" >"$evidence/observation.json"
      # The probe fails a terminal that is not a grid; the burst count is
      # asserted here too, so a probe that stopped checking the terminal
      # cannot pass the case.
      python3 -c 'import json,sys; result=json.load(open(sys.argv[1])); assert result.get("ok"), result; assert result.get("terminalRoundtrip", {}).get("burstLines") == 512, result.get("terminalRoundtrip")' \
        "$evidence/observation.json" || fail "native appearance observation failed (see $evidence)"
      kill -0 "$desktop_pid" 2>/dev/null || fail "appearance app died before final frame"
      DISPLAY="$DISP" timeout 10 import -window "$window" "$evidence/final-client.png" || fail "capturing final appearance client"
      # A uniform/nonpainting client cannot establish visible native behavior.
      # This is only a rejection oracle; varied pixels still need inspection.
      variation=$(convert "$evidence/final-client.png" -format '%[fx:standard_deviation]' info:) || fail "measuring final client"
      python3 -c 'import sys; assert float(sys.argv[1]) > 0.01, "uniform/nonpainting native client"' "$variation" || fail "native client did not visibly paint"
      grep -q 'desktop asset handler: served /assets/' "$evidence/desktop.log" || fail "native appearance loaded no embedded assets"
      if grep -n 'desktop asset handler: missing ' "$evidence/desktop.log" >"$evidence/asset-misses.txt"; then
        fail "native appearance has embedded asset misses"
      fi
      curl_auth -sf --max-time 30 -X DELETE "$API/api/sessions/$SID" >/dev/null || fail "deleting appearance fixture session"
      SID=""; LOCAL_SID=""

      # Openbox handles Alt-F4 as the ordinary WM close request. Wait for a
      # successful process exit before reusing the profile: SIGKILL is the
      # transport smoke's separate contract and cannot prove profile flushing.
      DISPLAY="$DISP" timeout 10 xdotool windowactivate --sync "$window" key --clearmodifiers alt+F4 || fail "requesting graceful appearance close"
      exited=""
      for _ in $(seq 1 100); do
        stat=$(ps -o stat= -p "$desktop_pid" 2>/dev/null | tr -d ' ')
        if [ -z "$stat" ] || [ "${stat#Z}" != "$stat" ]; then exited=1; break; fi
        sleep 0.1
      done
      [ -n "$exited" ] || fail "appearance app did not acknowledge graceful close"
      wait "$desktop_pid"
      desktop_status=$?
      rm -f "$X/desktop.pid"
      [ "$desktop_status" -eq 0 ] || fail "appearance app failed during graceful close"
      # Retain where WebKit actually wrote. The two fresh profiles plus each
      # control-selected relaunch distinguish isolated persistence from a
      # silently shared default profile; directory creation alone would not.
      find "$profile" -type f -printf '%P %s bytes\n' >"$evidence/profile-files.txt" || fail "recording private profile writes"
      [ -s "$evidence/profile-files.txt" ] || fail "WebKit wrote nothing to the explicit private profile"
      printf '%s\n' 'Native startup/control/persistence assertions passed; final client image retained.' \
        'DOM observations do not establish transient paint correctness.' \
        'No live native system-appearance event or macOS coverage.' >"$evidence/assessment.txt"
    done
  done
  echo "appearance integration assertions passed; final client images retained"
  PASS=1
  exit 0
fi
rm -f "$DESKTOP_READY"
DISPLAY=$DISP \
  PATH="/usr/bin:/bin" \
  FARHELM_SMOKE_CLIENT_LOG_MARKER="$CLIENT_LOG_MARKER" \
  RUST_LOG="info,farhelm_ui::desktop=debug,farhelm_helm::terminal_attachments=debug" \
  FARHELM_DESKTOP_SMOKE_READY="$DESKTOP_READY" \
  FARHELM_DESKTOP_STATE_DIR="$X/state" \
  FARHELM_DESKTOP_FARHELM="$BUILT_FARHELM" \
  "$APP" >"$X/desktop.log" 2>&1 &
echo $! >"$X/desktop.pid"

for _ in $(seq 1 30); do
  [ -s "$DESKTOP_READY" ] && break
  sleep 1
done
[ -s "$DESKTOP_READY" ] || fail "desktop did not publish embedded helm readiness"
read_desktop_ready "$DESKTOP_READY"
FIRST_SECRET="$SMOKE_SECRET"
ROOT_STATUS=$(curl_local -s -o "$X/root-response" -w '%{http_code}' --max-time 5 "$API/")
[ "$ROOT_STATUS" = 404 ] || fail "embedded helm served browser UI at / (status $ROOT_STATUS)"
TOKEN_STATUS=$(curl_auth -s -o "$X/token-response" -w '%{http_code}' --max-time 5 \
  -X POST -H 'content-type: application/json' -d '{}' "$API/api/auth/token")
[ "$TOKEN_STATUS" = 404 ] || fail "embedded helm exposed /api/auth/token (status $TOKEN_STATUS)"

# The state file appears when the webview records its first authenticated
# readiness; until then there is nothing to read.
WEBVIEW_GENERATION=0
for _ in $(seq 1 30); do
  if [ -f "$X/state/desktop-client.json" ]; then
    WEBVIEW_GENERATION=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("webview_auth_generation") or 0)' "$X/state/desktop-client.json")
    [ "$WEBVIEW_GENERATION" -ge 1 ] && break
  fi
  sleep 1
done
[ "$WEBVIEW_GENERATION" -ge 1 ] || fail "the webview JavaScript stack did not authenticate its event socket"
assert_no_persisted_credentials "first launch"
# Both desktop credentials are in-memory ones the embedded helm minted; no
# stored row should exist for either one.
DEVICE_ROWS=$(device_rows)
[ "$DEVICE_ROWS" = 0 ] || fail "desktop bootstrap stored $DEVICE_ROWS device rows instead of none"
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

# Random ports no longer make two app launches collide at bind. The state
# directory's ownership lock must still refuse the newcomer, and its startup
# error must reach stderr before the GUI starts. The incumbent must keep serving.
echo "== a second desktop launch refuses the owned state directory"
python3 - "$BUILT_FARHELM_DESKTOP" "$BUILT_FARHELM" "$X/state" "$X/second-desktop.stderr" <<'PY_SECOND' || fail "second desktop refusal"
import os
import subprocess
import sys

binary, sibling, state, stderr_path = sys.argv[1:]
child_env = dict(os.environ, FARHELM_DESKTOP_STATE_DIR=state, FARHELM_DESKTOP_FARHELM=sibling)
with open(stderr_path, "wb") as stderr:
    result = subprocess.run([binary], env=child_env, stdout=subprocess.DEVNULL,
                            stderr=stderr, timeout=20)
assert result.returncode == 1, f"second desktop exited {result.returncode}"
with open(stderr_path) as stderr:
    message = stderr.read()
assert "another process owns token control" in message, message
PY_SECOND
curl_auth -sf --max-time 5 "$API/api/hosts" >/dev/null || fail "second launch disturbed the incumbent helm"
# A second launch whose program differs would plan to replace the open app's
# supervisor; it must refuse at the open app's lock before stopping anything.
echo "== a second launch with another program refuses without stopping the open app's supervisor"
OPEN_SUPERVISOR_PID=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["pid"])' "$X/state/desktop-supervisor.json") ||
  fail "the app did not record the supervisor it started"
cp "$BUILT_FARHELM" "$X/other-farhelm" || fail "copying the program for the other-program launch"
python3 - "$BUILT_FARHELM_DESKTOP" "$X/other-farhelm" "$X/state" "$X/second-desktop-other.stderr" <<'PY_SECOND' || fail "second desktop refusal with another program"
import os
import subprocess
import sys

binary, sibling, state, stderr_path = sys.argv[1:]
child_env = dict(os.environ, FARHELM_DESKTOP_STATE_DIR=state, FARHELM_DESKTOP_FARHELM=sibling)
with open(stderr_path, "wb") as stderr:
    result = subprocess.run([binary], env=child_env, stdout=subprocess.DEVNULL,
                            stderr=stderr, timeout=60)
assert result.returncode == 1, f"second desktop exited {result.returncode}"
with open(stderr_path) as stderr:
    message = stderr.read()
assert "another process owns token control" in message, message
PY_SECOND
# The copy is a whole debug binary (about 460 MB), needed only for the
# launch above. Removing it now keeps it out of the directory a failure
# further down leaves behind.
rm -f -- "$X/other-farhelm"
kill -0 "$OPEN_SUPERVISOR_PID" 2>/dev/null || fail "the other-program launch stopped the open app's supervisor"
curl_auth -sf --max-time 5 "$API/api/hosts" >/dev/null || fail "the other-program launch disturbed the incumbent helm"

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

# Wait for the app writing $1 to issue a session listing whose query is
# exactly $2, as reported by the native `log_smoke_session_query` hook. The
# hook prints a plain `desktop_smoke: session listing requested query=...`
# line straight to stderr — NOT a tracing event — because tracing's ANSI
# styling of field names in CI's log defeated a literal grep for
# `query=sort=title` while the feature worked (CI run 32584494800 on PR
# #210). The whole-line fixed-string match below is the other half of that
# contract: whole-line, so `sort=title&title=x` cannot pass for `sort=title`.
# Both launch legs use this one oracle so a broken hook fails the first-boot
# leg by name instead of masquerading as a restore regression after relaunch.
wait_for_listing_request() {
  local log="$1" query="$2" line="desktop_smoke: session listing requested query=$2"
  for _ in $(seq 1 30); do
    grep -qxF "$line" "$log" 2>/dev/null && return 0
    sleep 1
  done
  grep -qxF "$line" "$log" 2>/dev/null || fail \
    "$(basename "$log") never recorded a session listing with query=$query (hook or oracle broken?)"
}

echo "== waiting for the first-boot listing request through the smoke hook"
# Fresh state carries no sort preference, so the first launch must list with
# the default order. The matching negative check — that nothing asked for
# sort=title before the preference is seeded — runs right before the seed,
# so it covers the whole unseeded launch rather than this first instant.
wait_for_listing_request "$X/desktop.log" "sort=activity"

echo "== waiting for the client-log marker: shim -> /api/client-log -> tracing"
wait_for_client_log_marker "$X/desktop.log"

# The asset handler is the whole of D6: a bare binary has no bundle to read
# assets from, so every script, stylesheet and font the window loads has to
# come out of the UI tree compiled into it (`desktop::serve_asset`). Two
# assertions, and the second is the load-bearing one — "something was served"
# is satisfied by a single lucky file, while "nothing was missing" is what
# catches an asset present in the desktop build and absent from the embedded
# web bundle, the exact divergence `scripts/check-desktop-assets.sh` guards at
# build time. The marker above already proves the pipeline runs; this proves
# it ran through the handler.
#
# The grep patterns are the log lines `serve_asset` emits verbatim. Keep them
# in step with that function.
grep -q 'desktop asset handler: served /assets/' "$X/desktop.log" ||
  fail "no asset reached the desktop asset handler (see $X/desktop.log)"
if grep -n 'desktop asset handler: missing ' "$X/desktop.log" >"$X/asset-misses"; then
  cat "$X/asset-misses" >&2
  fail "the desktop asset handler 404'd requests the embedded UI tree should have answered"
fi
echo "== asset handler served $(grep -c 'desktop asset handler: served ' "$X/desktop.log") requests, 0 missing"

echo "== creating two bundle-substrate sessions before the hard restart"
CREATE_BODY=$(python3 -c 'import json,sys; print(json.dumps({"cwd": sys.argv[1], "command": {"command": "bash", "yolo": False}, "title": "zzz-remembered"}))' "$X/work") || fail "encoding remembered smoke session"
SID=$(curl_auth -sf --max-time 10 -H 'content-type: application/json' -d "$CREATE_BODY" "$API/api/sessions" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])') || fail "creating the pre-restart smoke session"
[ -n "$SID" ] || fail "the pre-restart create returned no session id"
LOCAL_SID="${SID#*:}"
session_terminal_alive "$LOCAL_SID" || fail "the created session has no live agent terminal"
expect_host_terminal "$LOCAL_SID"
sleep 1
NEWEST_BODY=$(python3 -c 'import json,sys; print(json.dumps({"cwd": sys.argv[1], "command": {"command": "bash", "yolo": False}, "title": "aaa-newest"}))' "$X/work") || fail "encoding newest smoke session"
SID_NEWEST=$(curl_auth -sf --max-time 10 -H 'content-type: application/json' -d "$NEWEST_BODY" "$API/api/sessions" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])') || fail "creating the newest pre-restart smoke session"
[ -n "$SID_NEWEST" ] || fail "the newest pre-restart create returned no session id"
LOCAL_SID_NEWEST="${SID_NEWEST#*:}"
session_terminal_alive "$LOCAL_SID_NEWEST" || fail "the newest session has no live agent terminal"
expect_host_terminal "$LOCAL_SID_NEWEST"

# The default gate deliberately does not click rows: WebKitGTK's unreliable
# Xvfb paint is why its CI leg is non-pixel. Seed the helm's shared preference
# (SPEC.md, Session list: one row for every client, `PUT /api/preferences`)
# with a non-default order and the non-newest session, then observe
# restoration through the native request trace (`sort=title`) and the
# app's terminal attachment trace (the remembered, non-newest session gains
# the page's open attachment; `open_agent_attachments`). A `GET` read-back after the restart checks the durable
# input survived the relaunch, as a check of the input rather than a
# substitute for those behavioral observations.
# The post-restart `sort=title` oracle proves nothing if the app already
# asked for that order on its own, so the entire unseeded first launch — up
# to this point, sessions created and all — must be free of such a request.
grep -qxF 'desktop_smoke: session listing requested query=sort=title' "$X/desktop.log" \
  && fail "first boot requested sort=title before any preference was seeded"
# The optional click leg uses this restarted window too; keep the welcome out
# of its way through the same shared preference API ordinary clients use.
SEED_BODY=$(python3 -c 'import json,sys; print(json.dumps({"list_sort": "title", "last_selected": sys.argv[1], "welcome_seen": True}))' "$SID") || fail "encoding the preference seed"
curl_auth -sf --max-time 5 -X PUT -H 'content-type: application/json' -d "$SEED_BODY" "$API/api/preferences" \
  || fail "seeding the helm's shared preference for the restart"

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

echo "== killing the app without Rust cleanup and relaunching it"
OLD_DESKTOP_PID=$(cat "$X/desktop.pid")
SUPERVISOR_PID=$(ps -eo pid=,ppid=,args= | awk -v parent="$OLD_DESKTOP_PID" '$2 == parent && /supervisor run/ { print $1; exit }')
[ -n "$SUPERVISOR_PID" ] || fail "could not identify the managed supervisor child"
RECORDED_PID=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["pid"])' "$X/state/desktop-supervisor.json") ||
  fail "the app did not record the supervisor it started"
[ "$RECORDED_PID" = "$SUPERVISOR_PID" ] || fail "the app recorded process $RECORDED_PID, not its supervisor $SUPERVISOR_PID"
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
# The supervisor is detached and outlives the app: still running after a
# hard kill of the app, and reused (same process) by the relaunch below.
for _ in $(seq 1 20); do
  kill -0 "$SUPERVISOR_PID" 2>/dev/null || fail "the managed supervisor did not outlive the desktop app"
  sleep 0.25
done

rm -f "$DESKTOP_READY"
DISPLAY=$DISP \
  PATH="/usr/bin:/bin" \
  FARHELM_SMOKE_CLIENT_LOG_MARKER="$CLIENT_LOG_MARKER" \
  RUST_LOG="info,farhelm_ui::desktop=debug,farhelm_helm::terminal_attachments=debug" \
  FARHELM_DESKTOP_SMOKE_READY="$DESKTOP_READY" \
  FARHELM_DESKTOP_STATE_DIR="$X/state" \
  FARHELM_DESKTOP_FARHELM="$BUILT_FARHELM" \
  "$APP" >"$X/desktop-restart.log" 2>&1 &
echo $! >"$X/desktop.pid"
for _ in $(seq 1 30); do
  [ -s "$DESKTOP_READY" ] && break
  sleep 1
done
[ -s "$DESKTOP_READY" ] || fail "restarted desktop did not publish embedded helm readiness"
read_desktop_ready "$DESKTOP_READY"
[ "$SMOKE_SECRET" != "$FIRST_SECRET" ] || fail "desktop restart reused its native credential"
# Fresh issuance alone does not prove expiry. Present the old credential to
# the new listener, keeping it out of argv just like the current credential.
printf 'header = "Authorization: Bearer %s"\n' "$FIRST_SECRET" >"$X/old-curl-auth.conf"
chmod 600 "$X/old-curl-auth.conf"
OLD_STATUS=$(curl_local --config "$X/old-curl-auth.conf" -s -o /dev/null -w '%{http_code}' --max-time 5 "$API/api/hosts")
[ "$OLD_STATUS" = 401 ] || fail "the previous launch's native credential still authenticated (status $OLD_STATUS)"

for _ in $(seq 1 30); do
  RESTART_WEBVIEW_GENERATION=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("webview_auth_generation") or 0)' "$X/state/desktop-client.json")
  [ "$RESTART_WEBVIEW_GENERATION" -gt "$WEBVIEW_GENERATION" ] && break
  sleep 1
done
[ "$RESTART_WEBVIEW_GENERATION" -gt "$WEBVIEW_GENERATION" ] || fail "restarted webview never completed authenticated readiness"
curl_auth -sf --max-time 5 "$API/api/hosts" >/dev/null || fail "the native credential did not authenticate after the app restart"
assert_no_persisted_credentials "restart"
# The relaunch minted fresh in-memory desktop credentials and stored nothing,
# so no device rows accumulate across launches.
RESTART_ROWS=$(device_rows)
[ "$RESTART_ROWS" = 0 ] || fail "restart left $RESTART_ROWS device rows instead of none"
RESTART_PREFERENCES=$(curl_auth -sf --max-time 5 "$API/api/preferences") || fail "reading the shared preference after the restart"
RESTART_SELECTION=$(printf '%s' "$RESTART_PREFERENCES" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("last_selected") or "")')
RESTART_SORT=$(printf '%s' "$RESTART_PREFERENCES" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("list_sort") or "")')
[ "$RESTART_SELECTION" = "$SID" ] || fail "restart changed the helm's remembered selection"
[ "$RESTART_SORT" = "title" ] || fail "restart dropped the helm's remembered sort"
session_terminal_alive "$LOCAL_SID" || fail "the session's terminal did not survive the app restart"
kill -0 "$SUPERVISOR_PID" 2>/dev/null || fail "the relaunch stopped the supervisor it should have reused"
REUSED_PID=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["pid"])' "$X/state/desktop-supervisor.json")
[ "$REUSED_PID" = "$SUPERVISOR_PID" ] || fail "the relaunch replaced its own current supervisor (now $REUSED_PID) instead of reusing it"
wait_for_listing_request "$X/desktop-restart.log" "sort=title"
RESTORED_ATTACHMENT=""
for _ in $(seq 1 30); do
  REMEMBERED_CLIENTS=$(open_agent_attachments "$X/desktop-restart.log" "$SID")
  NEWEST_CLIENTS=$(open_agent_attachments "$X/desktop-restart.log" "$SID_NEWEST")
  if [ "$REMEMBERED_CLIENTS" -ge 1 ] && [ "$NEWEST_CLIENTS" -eq 0 ]; then
    RESTORED_ATTACHMENT=1
    break
  fi
  sleep 1
done
[ -n "$RESTORED_ATTACHMENT" ] ||
  fail "the relaunched page does not hold the remembered non-newest session (open attachments: remembered ${REMEMBERED_CLIENTS:-?}, newest ${NEWEST_CLIENTS:-?})"
# The terminal's link asset reached the same relaunched page whose
# terminal attachment the oracle above just proved: parity
# (`scripts/check-desktop-assets.sh`) holds the requested and bundled
# sets equal but cannot prove the window ever asked for this file, and
# the first-boot "something was served" check is satisfied by any
# single lucky file. The hashed name keeps its `terminal-links-` stem
# (`terminal-links-dxh<hash>.js`), so a stem prefix match names this
# asset without pinning the content hash. The restart log is the one
# that pairs with a proven terminal mount.
grep -q 'desktop asset handler: served /assets/terminal-links-' "$X/desktop-restart.log" ||
  fail "the relaunched page never requested the terminal link asset (see $X/desktop-restart.log)"
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
# rotation below: a regression that left logging dead after every ordinary
# app restart would otherwise slip past all the other restart assertions.
echo "== waiting for the restarted app's client-log marker"
wait_for_client_log_marker "$X/desktop-restart.log"

echo "== rotating the token: the internal desktop credential carries on"
# Rotation still changes the durable token used by a standalone helm, but the
# desktop's per-launch credential is in memory and deliberately outside that
# authority. The webview must not re-authenticate, and its terminal socket must
# remain the same socket after rotation: the attachment trace shows no end and
# no new attachment for the remembered session across the window below (a
# closed socket reports its end, and the page's reconnect a new attachment).
PAGE_OPEN_BEFORE=$(open_agent_attachments "$X/desktop-restart.log" "$SID")
PAGE_STARTS_BEFORE=$(agent_attachment_starts "$X/desktop-restart.log" "$SID")
[ "$PAGE_OPEN_BEFORE" -ge 1 ] ||
  fail "test premise: the page has no open attachment to the remembered session before rotation"
"$BUILT_FARHELM" helm token rotate --state-dir "$X/state" >/dev/null || fail "rotating desktop helm token"
ROTATED_STATUS=$(curl_auth -s -o /dev/null -w '%{http_code}' --max-time 5 "$API/api/hosts")
[ "$ROTATED_STATUS" = 200 ] || fail "rotation revoked the desktop's native credential (status $ROTATED_STATUS)"
# An observation window rather than a readiness wait: the claim is that
# nothing happens, so the script has to give the old behavior (a webview
# re-authentication within a second or two of rotation) time to show up.
sleep 5
AFTER_ROTATION_GENERATION=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("webview_auth_generation") or 0)' "$X/state/desktop-client.json")
[ "$AFTER_ROTATION_GENERATION" = "$RESTART_WEBVIEW_GENERATION" ] ||
  fail "rotation made the webview authenticate again (generation $RESTART_WEBVIEW_GENERATION -> $AFTER_ROTATION_GENERATION)"
PAGE_OPEN_AFTER=$(open_agent_attachments "$X/desktop-restart.log" "$SID")
PAGE_STARTS_AFTER=$(agent_attachment_starts "$X/desktop-restart.log" "$SID")
[ "$PAGE_OPEN_AFTER/$PAGE_STARTS_AFTER" = "$PAGE_OPEN_BEFORE/$PAGE_STARTS_BEFORE" ] ||
  fail "rotation closed or replaced the page's terminal socket (open/started before: $PAGE_OPEN_BEFORE/$PAGE_STARTS_BEFORE, after: $PAGE_OPEN_AFTER/$PAGE_STARTS_AFTER)"
ROTATED_ROWS=$(device_rows)
[ "$ROTATED_ROWS" = 0 ] || fail "rotation left $ROTATED_ROWS device rows instead of none"
assert_no_persisted_credentials "rotation"
# And the app's own native client is still authenticated: a new session makes
# the page list sessions again over native REST, and the smoke hook logs a
# listing only once the helm has answered it with success. Counting answered
# lines (rather than waiting for a fixed one) is what pins the answer to
# after the rotation.
ANSWERED_BEFORE=$(grep -c '^desktop_smoke: session listing answered ' "$X/desktop-restart.log")
ROTATED_BODY=$(python3 -c 'import json,sys; print(json.dumps({"cwd": sys.argv[1], "command": {"command": "bash", "yolo": False}, "title": "zzz-after-rotation"}))' "$X/work") || fail "encoding the post-rotation session"
SID_ROTATED=$(curl_auth -sf --max-time 10 -H 'content-type: application/json' -d "$ROTATED_BODY" "$API/api/sessions" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])') || fail "creating the post-rotation session"
expect_host_terminal "${SID_ROTATED#*:}"
NATIVE_ANSWERED=""
for _ in $(seq 1 30); do
  if [ "$(grep -c '^desktop_smoke: session listing answered ' "$X/desktop-restart.log")" -gt "$ANSWERED_BEFORE" ]; then
    NATIVE_ANSWERED=1
    break
  fi
  sleep 1
done
[ -n "$NATIVE_ANSWERED" ] || fail "no native session listing succeeded after rotation (see $X/desktop-restart.log)"
curl_auth -sf --max-time 30 -X DELETE "$API/api/sessions/$SID_ROTATED" >/dev/null || fail "cleaning up the post-rotation session"
SID_ROTATED=""

WID=""
for _ in $(seq 1 20); do
  WID=$(DISPLAY=$DISP xdotool search --name farhelm 2>/dev/null | tail -1)
  [ -n "$WID" ] && break
  sleep 1
done
[ -n "$WID" ] || fail "restarted app window never appeared"
DISPLAY=$DISP xdotool windowsize "$WID" 1200 900
sleep 3

# These deletes are the slowest requests in the smoke, and deliberately so:
# each session runs an interactive `bash`, which ignores SIGTERM, so the
# supervisor spends its whole SIGTERM grace (`KILL_GRACE` in
# crates/farhelm-supervisor/src/service/sweep.rs, 5 s) before the SIGKILL
# that actually ends it, and the DELETE reply follows that. A 5 s curl cap
# raced the 5 s grace and lost by a few hundred milliseconds on the
# v0.8.0-rc.1 gate; 30 s leaves the grace, the kill confirmation, and a slow
# runner comfortable room while still failing a delete that hangs.
curl_auth -sf --max-time 30 -X DELETE "$API/api/sessions/$SID" >/dev/null || fail "cleaning up the persisted smoke session"
curl_auth -sf --max-time 30 -X DELETE "$API/api/sessions/$SID_NEWEST" >/dev/null || fail "cleaning up the newest persisted smoke session"
SID=""
SID_NEWEST=""
LOCAL_SID=""
LOCAL_SID_NEWEST=""

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
  KEPT_BODY=$(python3 -c 'import json,sys; print(json.dumps({"cwd": sys.argv[1], "command": {"command": "bash", "yolo": False}, "title": "kept across replacement"}))' "$X/work") || fail "encoding the kept session"
  SID_KEPT=$(curl_auth -sf --max-time 10 -H 'content-type: application/json' -d "$KEPT_BODY" "$API/api/sessions" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])') || fail "creating the session kept across the replacement"
  LOCAL_SID_KEPT="${SID_KEPT#*:}"
  for _ in $(seq 1 30); do
    session_terminal_alive "$LOCAL_SID_KEPT" && break
    sleep 1
  done
  session_terminal_alive "$LOCAL_SID_KEPT" || fail "the kept session never got a live agent terminal"
  expect_host_terminal "$LOCAL_SID_KEPT"
  close_app_and_wait "$WID" "$(cat "$X/desktop.pid")"

  # An ordinary quit leaves the supervisor running too. A launch whose
  # program file differs from the one the running supervisor was started
  # from (an update, or a development build replacing the file at the same
  # path) replaces it: the old one is stopped and a new one started from the
  # current file. The file is replaced the way an install does it, a new
  # inode renamed over the path.
  echo "== a launch with a replaced program replaces the app's own supervisor"
  kill -0 "$SUPERVISOR_PID" 2>/dev/null || fail "the managed supervisor did not outlive an ordinary quit"
  cp "$BUILT_FARHELM" "$BUILT_FARHELM.new" && mv -f "$BUILT_FARHELM.new" "$BUILT_FARHELM" ||
    fail "replacing the program file"
  # A replacement checks the new supervisor's prerequisites before it stops
  # anything. A program reached through a directory whose name holds ':' is
  # one the supervisor cannot put on a session's PATH, so the launch must
  # refuse while the old supervisor keeps serving, its record unchanged. The
  # program itself is the real one (a symlink to it), so discovery runs as
  # usual and only the prerequisite check can refuse.
  echo "== a replacement that would fail its prerequisites stops nothing"
  mkdir -p "$X/replace:refused" || fail "creating the refused replacement's directory"
  ln -s "$BUILT_FARHELM" "$X/replace:refused/farhelm" || fail "linking the refused replacement's program"
  DISPLAY=$DISP \
    PATH="/usr/bin:/bin" \
    FARHELM_DESKTOP_STATE_DIR="$X/state" \
    FARHELM_DESKTOP_FARHELM="$X/replace:refused/farhelm" \
    timeout 120 "$APP" >"$X/desktop-replace-refused.log" 2>&1
  REFUSED_STATUS=$?
  [ "$REFUSED_STATUS" -eq 1 ] || fail "a replacement that cannot run sessions exited $REFUSED_STATUS, not 1 (see $X/desktop-replace-refused.log)"
  grep -q 'cannot be used as one PATH entry' "$X/desktop-replace-refused.log" ||
    fail "the refused replacement did not report its prerequisite check (see $X/desktop-replace-refused.log)"
  kill -0 "$SUPERVISOR_PID" 2>/dev/null || fail "a replacement that refused stopped the old supervisor"
  [ "$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["pid"])' "$X/state/desktop-supervisor.json")" = "$SUPERVISOR_PID" ] ||
    fail "a replacement that refused changed the record"
  rm -f "$DESKTOP_READY"
  DISPLAY=$DISP \
    PATH="/usr/bin:/bin" \
    RUST_LOG="info,farhelm_ui::desktop=debug" \
    FARHELM_DESKTOP_SMOKE_READY="$DESKTOP_READY" \
    FARHELM_DESKTOP_STATE_DIR="$X/state" \
    FARHELM_DESKTOP_FARHELM="$BUILT_FARHELM" \
    "$APP" >"$X/desktop-replaced.log" 2>&1 &
  echo $! >"$X/desktop.pid"
  for _ in $(seq 1 60); do
    [ -s "$DESKTOP_READY" ] && break
    sleep 1
  done
  [ -s "$DESKTOP_READY" ] || fail "the relaunch with a replaced program did not publish readiness (see $X/desktop-replaced.log)"
  read_desktop_ready "$DESKTOP_READY"
  REPLACED_READY=""
  for _ in $(seq 1 30); do
    if curl_auth -sf --max-time 5 "$API/api/hosts" | python3 -c '
import json,sys
assert any(h["kind"] == "local" and h["state"]["phase"] == "connected" for h in json.load(sys.stdin)["hosts"])
' 2>/dev/null; then
      REPLACED_READY=1
      break
    fi
    sleep 1
  done
  [ -n "$REPLACED_READY" ] || fail "the replacement supervisor never connected (see $X/desktop-replaced.log)"
  ! kill -0 "$SUPERVISOR_PID" 2>/dev/null || fail "the previous supervisor was not stopped"
  grep -q 'replacing the supervisor an earlier launch started' "$X/desktop-replaced.log" ||
    fail "the relaunch did not report replacing its own supervisor (see $X/desktop-replaced.log)"
  if grep -q 'ignored the orderly stop and was killed' "$X/desktop-replaced.log"; then
    fail "the previous supervisor had to be killed instead of stopping in order (see $X/desktop-replaced.log)"
  fi
  REPLACEMENT_PID=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["pid"])' "$X/state/desktop-supervisor.json")
  [ "$REPLACEMENT_PID" != "$SUPERVISOR_PID" ] && kill -0 "$REPLACEMENT_PID" 2>/dev/null ||
    fail "the record does not name a running replacement supervisor"
  [ "$(stat -L -c %i "/proc/$REPLACEMENT_PID/exe")" = "$(stat -c %i "$BUILT_FARHELM")" ] ||
    fail "the replacement supervisor does not run the replaced program file"
  # Sessions survive the replacement: the same terminal still alive, and
  # still listed by the new supervisor through the relaunched app.
  session_terminal_alive "$LOCAL_SID_KEPT" ||
    fail "the kept session did not survive the supervisor replacement"
  KEPT_LISTED=""
  for _ in $(seq 1 30); do
    if curl_auth -sf --max-time 5 "$API/api/sessions/$SID_KEPT" >/dev/null; then
      KEPT_LISTED=1
      break
    fi
    sleep 1
  done
  [ -n "$KEPT_LISTED" ] || fail "the replacement supervisor does not list the kept session"
  # Listed and listening is not usable: an unreadable host is listed and still
  # listens. Drive the kept terminal through the replacement: attach as a grid
  # viewer, wait for its replay, type a command and wait for what it writes.
  drive_terminal "$SID_KEPT" "$API" "$X/kept-typed" ||
    fail "the kept session's terminal cannot be driven after the supervisor replacement"
  curl_auth -sf --max-time 30 -X DELETE "$API/api/sessions/$SID_KEPT" >/dev/null || fail "deleting the kept session"
  SID_KEPT=""
  kill -KILL "$(cat "$X/desktop.pid")" 2>/dev/null
  wait "$(cat "$X/desktop.pid")" 2>/dev/null
  rm -f "$X/desktop.pid"

  # A supervisor of the app's own that holds the state directory but never
  # answers (here: frozen with SIGSTOP, so its socket still accepts while the
  # hello never comes back) is replaced too, rather than leaving every launch
  # stuck behind it. A stopped process cannot run its SIGTERM handler, so the
  # app's stop falls through to the kill after its grace; that is the one case
  # the kill is for.
  echo "== a launch replaces the app's own supervisor when it never answers"
  kill -STOP "$REPLACEMENT_PID" || fail "freezing the replacement supervisor"
  rm -f "$DESKTOP_READY"
  DISPLAY=$DISP \
    PATH="/usr/bin:/bin" \
    RUST_LOG="info,farhelm_ui::desktop=debug" \
    FARHELM_DESKTOP_SMOKE_READY="$DESKTOP_READY" \
    FARHELM_DESKTOP_STATE_DIR="$X/state" \
    FARHELM_DESKTOP_FARHELM="$BUILT_FARHELM" \
    "$APP" >"$X/desktop-wedged.log" 2>&1 &
  echo $! >"$X/desktop.pid"
  # A frozen supervisor still holds its socket, so the probe times out; the
  # app then gives it the startup wait (it could be a supervisor still in its
  # post-bind startup sweeps), and only then stops it, waits the orderly
  # grace and kills it. The probe, the 35 s wait, the 15 s grace and the new
  # supervisor's start add up to over a minute; the bound leaves room for a
  # loaded machine.
  for _ in $(seq 1 180); do
    [ -s "$DESKTOP_READY" ] && break
    sleep 1
  done
  [ -s "$DESKTOP_READY" ] || fail "the relaunch beside a frozen supervisor did not publish readiness (see $X/desktop-wedged.log)"
  read_desktop_ready "$DESKTOP_READY"
  grep -q 'the supervisor an earlier launch started does not answer' "$X/desktop-wedged.log" ||
    fail "the relaunch did not treat its frozen supervisor as wedged (see $X/desktop-wedged.log)"
  grep -q 'ignored the orderly stop and was killed' "$X/desktop-wedged.log" ||
    fail "the frozen supervisor was not killed after the grace, as a stopped process must be (see $X/desktop-wedged.log)"
  ! kill -0 "$REPLACEMENT_PID" 2>/dev/null || fail "the frozen supervisor was not stopped"
  UNWEDGED_PID=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["pid"])' "$X/state/desktop-supervisor.json")
  [ "$UNWEDGED_PID" != "$REPLACEMENT_PID" ] && kill -0 "$UNWEDGED_PID" 2>/dev/null ||
    fail "the record does not name a running supervisor after the wedged one was replaced"
  kill -KILL "$(cat "$X/desktop.pid")" 2>/dev/null
  wait "$(cat "$X/desktop.pid")" 2>/dev/null
  rm -f "$X/desktop.pid"

  # An ANSWERING supervisor the app did not start is an ownership boundary:
  # `DesktopBootstrap::start` reuses it exactly as it stands. Proving that
  # needs a real existing supervisor, which is what this leg spins up by
  # hand: a bare `farhelm supervisor run`, then a fresh desktop process
  # pointed at that SAME state directory, which must come up beside it
  # without starting or recording a supervisor of its own.
  #
  # Reuses the Xvfb/openbox display already up for the main launch above —
  # dioxus-desktop still needs one to reach this supervisor at all, even
  # though this leg drives no window and clicks nothing. The readiness file
  # and authenticated API request prove the embedded helm started.
  echo "== an answering supervisor the app did not start is reused"
  ANSWERING_STATE="$X/answering-state"
  mkdir -m 0700 "$ANSWERING_STATE"
  ANSWERING_SUPERVISOR_LOG="$X/answering-supervisor.log"
  # setsid gives the supervisor a process group of its own; see stop_answering.
  setsid "$BUILT_FARHELM" supervisor run --state-dir "$ANSWERING_STATE" \
    >"$ANSWERING_SUPERVISOR_LOG" 2>&1 &
  ANSWERING_SUPERVISOR_PID=$!
  for _ in $(seq 1 30); do
    [ -S "$ANSWERING_STATE/supervisor.sock" ] && break
    kill -0 "$ANSWERING_SUPERVISOR_PID" 2>/dev/null ||
      fail "the hand-started answering supervisor exited before listening (see $ANSWERING_SUPERVISOR_LOG)"
    sleep 1
  done
  [ -S "$ANSWERING_STATE/supervisor.sock" ] ||
    fail "the hand-started answering supervisor never created its socket (see $ANSWERING_SUPERVISOR_LOG)"

  # A real terminal host in this state directory, started the way a
  # supervisor starts one, standing in for a terminal the supervisor would
  # have started: stop_answering below must end it through the teardown's
  # own stop_terminal_hosts. This is the leg's own assertion of what that stop
  # does: the main state's hosts (the app's sessions run in terminal hosts) are
  # stopped by the trap's teardown too, but that only keeps the directory when
  # it cannot confirm them, where this leg fails. The fixture prints
  # "<pid> <socket>".
  BUILT_FIXTURES="$TARGET_DIR/debug/farhelm-fixtures"
  [ -x "$BUILT_FIXTURES" ] || fail "farhelm-fixtures missing at $BUILT_FIXTURES"
  ANSWERING_HOST=$("$BUILT_FIXTURES" spawn-terminal-host --farhelm "$BUILT_FARHELM" \
    --state-dir "$ANSWERING_STATE") || fail "starting a terminal host in the answering state directory"
  read -r ANSWERING_HOST_PID ANSWERING_HOST_SOCKET <<<"$ANSWERING_HOST"
  case "$(ps -ww -p "$ANSWERING_HOST_PID" -o args= 2>/dev/null)" in
    *"internal terminal-host --socket $ANSWERING_HOST_SOCKET") ;;
    *) fail "the planted terminal host is not running as one: $ANSWERING_HOST" ;;
  esac

  # The embedded helm chooses its own fresh loopback port, published through
  # the debug-only readiness seam.
  ANSWERING_READY_PATH="$X/answering-ready.json"
  rm -f "$ANSWERING_READY_PATH"
  ANSWERING_DESKTOP_LOG="$X/answering-desktop.log"
  DISPLAY=$DISP \
    FARHELM_DESKTOP_SMOKE_READY="$ANSWERING_READY_PATH" \
    FARHELM_DESKTOP_STATE_DIR="$ANSWERING_STATE" \
    FARHELM_DESKTOP_FARHELM="$BUILT_FARHELM" \
    "$APP" >"$ANSWERING_DESKTOP_LOG" 2>&1 &
  ANSWERING_DESKTOP_PID=$!

  ANSWERING_READY=""
  for _ in $(seq 1 30); do
    if [ -s "$ANSWERING_READY_PATH" ]; then
      ANSWERING_READY=1
      break
    fi
    kill -0 "$ANSWERING_DESKTOP_PID" 2>/dev/null || break
    sleep 1
  done
  [ -n "$ANSWERING_READY" ] ||
    fail "desktop against an answering supervisor never published readiness (see $ANSWERING_DESKTOP_LOG)"
  read_desktop_ready "$ANSWERING_READY_PATH"
  curl_auth -sf --max-time 5 "$API/api/hosts" >/dev/null ||
    fail "desktop against an answering supervisor could not authenticate its native API"
  # Reused, not replaced: the hand-started supervisor still runs, and the app
  # wrote no record of a supervisor of its own, which it does for every one
  # it starts.
  kill -0 "$ANSWERING_SUPERVISOR_PID" 2>/dev/null ||
    fail "desktop against an answering supervisor stopped it (see $ANSWERING_DESKTOP_LOG)"
  [ ! -e "$ANSWERING_STATE/desktop-supervisor.json" ] ||
    fail "desktop against an answering supervisor started one of its own (see $ANSWERING_DESKTOP_LOG)"

  stop_answering
  [ "${#HOSTS_NOT_STOPPED[@]}" -eq 0 ] ||
    fail "teardown could not confirm the answering state's terminal host stopped: ${HOSTS_NOT_STOPPED[*]}"
  # Bounded: the stop already confirmed the host gone; this only lets the
  # kernel finish the exit.
  for _ in $(seq 1 50); do
    case "$(ps -o stat= -p "$ANSWERING_HOST_PID" 2>/dev/null)" in
      '' | Z*) break ;;
    esac
    sleep 0.1
  done
  case "$(ps -o stat= -p "$ANSWERING_HOST_PID" 2>/dev/null)" in
    '' | Z*) ;;
    *) fail "the answering state's terminal host $ANSWERING_HOST_PID still runs after teardown" ;;
  esac

  echo "== PASS: embedded helm, dual auth, local supervisor, and a session's terminal survive restart"
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
# ctrl+a first: the working-directory field is prefilled with "~" (the
# create form's default), and xdotool types at the caret rather than
# replacing — without the select-all this would submit "~$X/work".
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
LOCAL_SID="${SID#*:}"
echo "   created $SID"
expect_host_terminal "$LOCAL_SID"

# Creation is a user-initiated selection, so it exercises the same
# preference write (`PUT /api/preferences` through native reqwest) as a row
# click without adding another fragile coordinate. Poll the helm's row rather
# than sleeping for an assumed duration.
for _ in $(seq 1 20); do
  WRITTEN_ID=$(curl_auth -sf --max-time 5 "$API/api/preferences" 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin).get("last_selected") or "")' 2>/dev/null)
  [ "$WRITTEN_ID" = "$SID" ] && break
  sleep 0.25
done
[ "$WRITTEN_ID" = "$SID" ] || fail "the desktop page selection did not reach the helm's preference row"

echo "== typing into the terminal and asserting through a file the typed command writes"
# The create lands in the session view with the terminal mounted. The typed
# command leaves its evidence in a file rather than on the screen, because a
# terminal host's screen is something nothing outside the supervisor can read.
sleep 3
TYPED_PROOF="$X/legacy-typed"
DISPLAY=$DISP xdotool mousemove 700 400 click 1 sleep 0.5 type --delay 120 "echo smoke-ok >$TYPED_PROOF"
DISPLAY=$DISP xdotool key Return
OK=""
for _ in $(seq 1 10); do
  if grep -qx "smoke-ok" "$TYPED_PROOF" 2>/dev/null; then
    OK=1
    break
  fi
  sleep 1
done
[ -n "$OK" ] || fail "typed input never reached the pane"

# Bash can consume the full five-second graceful stop; allow the same
# escalation-and-response budget used by the default headless leg.
curl_auth -sf --max-time 30 -X DELETE "$API/api/sessions/$SID" >/dev/null || fail "cleaning up smoke session"
SID=""
LOCAL_SID=""
close_app_and_wait "$WID" "$(cat "$X/desktop.pid")"

# The legacy path's ONE watchdog check, after every interaction that could
# have provoked a false positive — see assert_watchdog_silent's comment.
assert_watchdog_silent
echo "== PASS: embedded helm, dual auth, local supervisor, restart reuse, and terminal round-trip work"
PASS=1
exit 0
