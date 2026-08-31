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
# real Mac (docs/manual-mac-checklist.md).
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
# 0.7.10, the wasm32-unknown-unknown target (the run starts by building the
# web bundle, which is then embedded in the desktop build), and tmux.
# Usage: scripts/desktop-smoke.sh   (from the repo root; ~5 min)
# Honors CARGO_TARGET_DIR, relative or absolute.
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

# The tmux-preflight legs below assert the exact stderr farhelm-desktop
# renders for a below-floor tmux, and that message quotes `TMUX_FLOOR`.
# Sourced from the same pin file the release build and
# `scripts/build-pinned-tmux-ci.sh` read, rather than hardcoded here a
# second time, so a coordinated floor-and-pin bump can never leave this
# script's expectation stale — see `farhelm_supervisor::tmux::TMUX_FLOOR`'s
# own doc comment for why the two are bumped together. `unset` first
# because the file is SOURCED, not read structurally: a caller's own
# ambient `TMUX_VERSION` must never silently win over the pinned one.
unset TMUX_VERSION
# shellcheck source=../.github/release/source-pins.env
. "$REPO/.github/release/source-pins.env"
[ -n "${TMUX_VERSION:-}" ] || {
  echo "FAIL: source-pins.env did not define TMUX_VERSION" >&2
  exit 1
}
TMUX_FLOOR="$TMUX_VERSION"

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

# Honor CARGO_TARGET_DIR the way cargo does, resolved ONCE to an absolute
# path and exported. The repo's own dev loop runs out of jj workspaces that
# share one target directory, so this script has to follow it; CI leaves it
# unset, where this is exactly the old `$REPO/target`.
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
WEB_DIST="$TARGET_DIR/dx/farhelm-ui/release/web/public"

echo "== building (cargo + web bundle + dx desktop build)"
(cd "$REPO" && flock --close /tmp/fh-build.lock -c 'ulimit -c unlimited && cargo build --quiet') || fail "cargo build"
# `--package farhelm-ui` is not decoration. The workspace has `default-members`
# (it excludes `farhelm-desktop` so ordinary builds never compile WebKit), and
# dx 0.7.10 resolves those entries by canonicalizing each RELATIVE member path
# against its own working directory — from `crates/farhelm-ui` that is
# `crates/farhelm-ui/crates/farhelm`, which does not exist, and dx panics on
# the `unwrap` (`packages/cli/src/workspace.rs`). Naming the package takes an
# earlier return that never reads `default-members` at all.
(cd "$REPO/crates/farhelm-ui" && flock --close /tmp/fh-build.lock -c 'ulimit -c unlimited && dx build --package farhelm-ui --platform web --release' >"$X/dx-web.log" 2>&1) || fail "dx web build (see $X/dx-web.log)"
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
#
# There is no bundled tmux (SPEC_impl.md, "Terminal substrate: private tmux
# server" — it records the version floor, the `FARHELM_TMUX` override taking
# precedence over the fixed-prefix probe, and the decision that the Mac app
# ships none and requires Homebrew's);
# the sentinel below wraps the preflighted host binary and is named only
# through that variable, so the marker proves the override reached the managed
# supervisor and that it ran this exact program rather than whatever `tmux`
# the app's deliberately small PATH would have found.
BUILT_FARHELM="$TARGET_DIR/debug/farhelm"
[ -x "$BUILT_FARHELM" ] || fail "farhelm CLI missing at $BUILT_FARHELM"

# A sibling `farhelm` that logs every argv it is exec'd with before handing
# off to the real binary unchanged — used by the two tmux-preflight refusal
# legs below (F3) to prove that discovery ran (`internal stdio`, the only
# thing `discover_local_supervisor`'s local-transport probe ever invokes a
# sibling farhelm for — see `SystemBackend::spawn_probe` in
# crates/farhelm-helm/src/provisioning/backend.rs) but the managed
# supervisor never got as far as `supervisor run`. Exit status, stderr text,
# and an empty state directory (already asserted below) prove the refusal
# was clean; none of them alone rules out a spawn-then-kill implementation
# that started the supervisor and tore it down again before it wrote
# anything — this wrapper is what turns "no evidence of a spawn" into "an
# instrumented process that would have logged one did not see one at all".
# Reused by both refusal legs with a fresh log path per invocation, the same
# way `resolve_sibling_farhelm`'s own `FARHELM_DESKTOP_FARHELM` override is
# reused for the un-instrumented sibling elsewhere in this script.
INSTRUMENTED_FARHELM="$X/instrumented-farhelm/farhelm"
mkdir -p "$(dirname "$INSTRUMENTED_FARHELM")"
printf '%s\n' \
  '#!/bin/sh' \
  'printf "%s\n" "$*" >>"$FARHELM_SMOKE_SIBLING_LOG"' \
  "exec \"$BUILT_FARHELM\" \"\$@\"" >"$INSTRUMENTED_FARHELM"
chmod 700 "$INSTRUMENTED_FARHELM"

HOST_TMUX=$(command -v tmux)
SENTINEL_TMUX="$X/sentinel-tmux/tmux"
mkdir -p "$(dirname "$SENTINEL_TMUX")"
# Logs the COMPLETE argv of every invocation, one per line, rather than
# writing a single fixed marker: the desktop's own tmux preflight now
# calls this wrapper with `-V` too (immediately before the managed
# supervisor it is about to spawn), and the managed supervisor's own
# `ensure_server` (crates/farhelm-supervisor/src/tmux.rs) issues a SECOND
# `-V` — this one prefixed with its private-server `-S <socket> -f
# <config>` — before it ever touches the server. A marker that only
# recorded "was this program run at all", or even "was it run with
# anything other than exactly `-V`", would already be satisfied by that
# second probe line and would prove nothing about whether the SUPERVISOR
# went on to actually use this tmux. The later assertion greps for a
# logged line naming `start-server` specifically — the operational
# command `ensure_server` issues right after its version check succeeds.
printf '%s\n' \
  '#!/bin/sh' \
  'printf "%s\n" "$*" >>"$FARHELM_SMOKE_TMUX_MARKER"' \
  "exec \"$HOST_TMUX\" \"\$@\"" >"$SENTINEL_TMUX"
chmod 700 "$SENTINEL_TMUX"

# One plain stderr message and exit status 1 — never a panic, never the
# supervisor's own "Error:"/`Caused by` chain — is this app's whole
# contract for a missing or below-floor tmux (SPEC_impl.md's "Terminal
# substrate: private tmux server"). These two legs are the only assertions
# in this whole script that need NEITHER Xvfb NOR the pixel-driven
# interaction phase: `DesktopBootstrap::start`'s preflight runs right
# before spawning the managed supervisor, once discovery has confirmed no
# supervisor already answers — well before any window, display, or web
# asset is ever requested, so exercising the entry point headless here
# proves the SAME refusal path a Finder launch would hit.
#
# The binary under test is the dx-built `farhelm-ui` desktop app the rest of
# this script drives (see `BUILT_FARHELM_DESKTOP` below for why not a
# separate build of the `farhelm-desktop` wrapper crate). Both binaries
# share one entry point (`farhelm_ui::desktop::run`), and refusal happens
# before any asset is ever requested, so which of the two runs it makes no
# difference to what these legs check.
#
# Both legs use the SAME kind of isolation the main app launch further
# down uses — a private state directory, the shared smoke port, and the
# built `farhelm` sibling — rather than leaving any of them unset: an
# unset state directory would let `desktop_state_dir()` fall back to
# `~/.local/state/farhelm`, the MAINTAINER's real, live Farhelm state, and
# an unset sibling would leave `bundled_farhelm()` resolving against
# whatever happens to sit next to `farhelm-desktop` on this machine.
# Discovery now runs before the preflight (see `DesktopBootstrap::start`'s
# own doc comment on that ordering), so a WORKING sibling `farhelm` is
# required for these legs to even reach the preflight at all — it is what
# proves "no supervisor answers here" quickly and silently. Each leg's
# guaranteed-absent or below-floor tmux path also lives inside this run's
# own private directory ($X) rather than a global path like
# `/nonexistent/tmux`: on a machine where that literal path happened to
# exist and answer as a supported tmux, the missing-tmux leg would prove
# nothing, and bootstrap would proceed into whatever state or sibling
# `desktop_state_dir()`/`bundled_farhelm()` fell back to.
echo "== tmux preflight: a missing tmux refuses with one plain message"
# The refusal legs run the dx-built `farhelm-ui` desktop app rather than a
# separately Cargo-built `farhelm-desktop`: the shipped `farhelm-desktop`
# crate is a one-line wrapper whose `main` calls the very same
# `farhelm_ui::desktop::run()` this app runs, so the preflight under test is
# byte-for-byte the same code, while a second WebKit build of the wrapper
# crate on top of the dx build is what pushed CI's runner out of disk
# (PR #276's first run: "No space left on device" inside `dx build`).
BUILT_FARHELM_DESKTOP="$APP"
[ -x "$BUILT_FARHELM_DESKTOP" ] || fail "desktop app missing at $BUILT_FARHELM_DESKTOP"

PREFLIGHT_MISSING_STATE="$X/pf-missing"
MISSING_TMUX="$X/pf-missing-tmux/tmux"
TMUX_PREFLIGHT_STDERR="$X/tmux-preflight-missing.stderr"
SIBLING_LOG_MISSING="$X/farhelm-sibling-missing.log"
: >"$SIBLING_LOG_MISSING"
FARHELM_TMUX="$MISSING_TMUX" \
  FARHELM_DESKTOP_STATE_DIR="$PREFLIGHT_MISSING_STATE" \
  FARHELM_DESKTOP_PORT="$PORT" \
  FARHELM_DESKTOP_FARHELM="$INSTRUMENTED_FARHELM" \
  FARHELM_DESKTOP_UI_DIST="$WEB_DIST" \
  FARHELM_SMOKE_SIBLING_LOG="$SIBLING_LOG_MISSING" \
  "$BUILT_FARHELM_DESKTOP" >/dev/null 2>"$TMUX_PREFLIGHT_STDERR"
TMUX_PREFLIGHT_STATUS=$?
[ "$TMUX_PREFLIGHT_STATUS" -eq 1 ] ||
  fail "farhelm-desktop exited $TMUX_PREFLIGHT_STATUS with no tmux on FARHELM_TMUX (expected exactly 1)"
# The exact text `tmux_refusal_message` (desktop.rs) renders for this case:
# an ambient `FARHELM_TMUX` override is probed ALONE (resolve_supervisor_tmux
# skips every other probe once one is set) and is what selected this missing
# program, so the override-remedy clause applies; the program never existed
# so the outcome is NotFound; and this box is Linux so the Linux
# install-guidance line applies. `$TMUX_FLOOR` was sourced from
# `.github/release/source-pins.env` above — the same pin
# `farhelm_supervisor`'s own `floor_and_release_pin_cannot_drift` test
# checks the floor constant against — so a coordinated bump of both cannot
# leave this expectation stale.
EXPECTED_TMUX_PREFLIGHT_STDERR="$X/tmux-preflight-missing.expected"
printf '%s\n' \
  "farhelm-desktop needs tmux $TMUX_FLOOR or newer, and none could be run (looked at: $MISSING_TMUX). Each one was either not found, or is missing its interpreter or loader." \
  "Install tmux $TMUX_FLOOR or newer with your package manager or Linuxbrew (\`brew install tmux\`). FARHELM_TMUX is set and overrides where farhelm-desktop looks, so update it to point at a supported tmux, or unset it, before starting farhelm-desktop again." \
  >"$EXPECTED_TMUX_PREFLIGHT_STDERR"
# `cmp` compares raw bytes, including the final newline `printf` above put
# on the expected side — a `$(...)`-based comparison would strip every
# trailing newline from BOTH sides and could not tell an exact match from
# one with extra blank diagnostic lines appended.
cmp -s "$TMUX_PREFLIGHT_STDERR" "$EXPECTED_TMUX_PREFLIGHT_STDERR" ||
  fail "tmux preflight stderr did not match exactly (see $TMUX_PREFLIGHT_STDERR)"
# `ensure_private_dir` runs before discovery, so the directory itself is
# expected to exist — but the preflight refuses immediately after
# discovery and before anything is ever written into it, so it must still
# be EMPTY: no desktop-client.json, no supervisor socket, nothing.
[ -d "$PREFLIGHT_MISSING_STATE" ] ||
  fail "expected the preflight's own private state directory to exist ($PREFLIGHT_MISSING_STATE)"
[ -z "$(ls -A "$PREFLIGHT_MISSING_STATE")" ] ||
  fail "the missing-tmux preflight must refuse before writing anything into its state directory"
# An empty state directory proves nothing spawned SURVIVED, but a
# spawn-then-kill implementation (start the managed supervisor, run the
# preflight right after, tear the child down on refusal before it wrote a
# socket) would satisfy every assertion above too. The instrumented sibling
# is what tells those apart: discovery must have run (`internal stdio`,
# logged by the wrapper before it execs the real binary), and no
# `supervisor run` invocation may appear anywhere in that same log.
grep -q '^internal stdio' "$SIBLING_LOG_MISSING" ||
  fail "expected discovery's 'internal stdio' invocation in the sibling log (see $SIBLING_LOG_MISSING)"
if grep -q 'supervisor run' "$SIBLING_LOG_MISSING"; then
  fail "the missing-tmux preflight must refuse before spawning 'supervisor run' (see $SIBLING_LOG_MISSING)"
fi

echo "== tmux preflight: a below-floor tmux refuses with one plain message"
PREFLIGHT_BELOWFLOOR_STATE="$X/pf-belowfloor"
BELOWFLOOR_TMUX="$X/pf-belowfloor-tmux/tmux"
mkdir -p "$(dirname "$BELOWFLOOR_TMUX")"
# A tiny stand-in that answers `-V` with a version below the floor and
# refuses anything else — real enough to prove this is a PROCESS-level
# refusal (stderr ownership, exact wording, exit status), not merely
# something the pure formatter tests already cover.
printf '%s\n' \
  '#!/bin/sh' \
  'if [ "$1" = -V ]; then echo "tmux 3.4"; exit 0; fi' \
  'echo "unexpected invocation: $*" >&2' \
  'exit 2' >"$BELOWFLOOR_TMUX"
chmod 700 "$BELOWFLOOR_TMUX"
TMUX_PREFLIGHT_BELOWFLOOR_STDERR="$X/tmux-preflight-belowfloor.stderr"
SIBLING_LOG_BELOWFLOOR="$X/farhelm-sibling-belowfloor.log"
: >"$SIBLING_LOG_BELOWFLOOR"
FARHELM_TMUX="$BELOWFLOOR_TMUX" \
  FARHELM_DESKTOP_STATE_DIR="$PREFLIGHT_BELOWFLOOR_STATE" \
  FARHELM_DESKTOP_PORT="$PORT" \
  FARHELM_DESKTOP_FARHELM="$INSTRUMENTED_FARHELM" \
  FARHELM_DESKTOP_UI_DIST="$WEB_DIST" \
  FARHELM_SMOKE_SIBLING_LOG="$SIBLING_LOG_BELOWFLOOR" \
  "$BUILT_FARHELM_DESKTOP" >/dev/null 2>"$TMUX_PREFLIGHT_BELOWFLOOR_STDERR"
TMUX_PREFLIGHT_BELOWFLOOR_STATUS=$?
[ "$TMUX_PREFLIGHT_BELOWFLOOR_STATUS" -eq 1 ] ||
  fail "farhelm-desktop exited $TMUX_PREFLIGHT_BELOWFLOOR_STATUS against a 3.4 tmux (expected exactly 1)"
EXPECTED_TMUX_PREFLIGHT_BELOWFLOOR_STDERR="$X/tmux-preflight-belowfloor.expected"
printf '%s\n' \
  "found tmux 3.4 at $BELOWFLOOR_TMUX, which is below the $TMUX_FLOOR farhelm needs." \
  "Install tmux $TMUX_FLOOR or newer with your package manager or Linuxbrew (\`brew install tmux\`). FARHELM_TMUX is set and overrides where farhelm-desktop looks, so update it to point at a supported tmux, or unset it, before starting farhelm-desktop again." \
  >"$EXPECTED_TMUX_PREFLIGHT_BELOWFLOOR_STDERR"
cmp -s "$TMUX_PREFLIGHT_BELOWFLOOR_STDERR" "$EXPECTED_TMUX_PREFLIGHT_BELOWFLOOR_STDERR" ||
  fail "below-floor tmux preflight stderr did not match exactly (see $TMUX_PREFLIGHT_BELOWFLOOR_STDERR)"
[ -d "$PREFLIGHT_BELOWFLOOR_STATE" ] ||
  fail "expected the preflight's own private state directory to exist ($PREFLIGHT_BELOWFLOOR_STATE)"
[ -z "$(ls -A "$PREFLIGHT_BELOWFLOOR_STATE")" ] ||
  fail "the below-floor preflight must refuse before writing anything into its state directory"
# See the missing-tmux leg's comment above: the empty state directory does
# not distinguish "never spawned" from "spawned and killed before it wrote
# anything", so this checks the instrumented sibling's log directly.
grep -q '^internal stdio' "$SIBLING_LOG_BELOWFLOOR" ||
  fail "expected discovery's 'internal stdio' invocation in the sibling log (see $SIBLING_LOG_BELOWFLOOR)"
if grep -q 'supervisor run' "$SIBLING_LOG_BELOWFLOOR"; then
  fail "the below-floor preflight must refuse before spawning 'supervisor run' (see $SIBLING_LOG_BELOWFLOOR)"
fi

# The two legs above exercise the SPECIALIZED tmux refusal; this one proves
# `desktop::run`'s FALLBACK path — every bootstrap failure the preflight has
# no tailored wording for — is equally plain: one `farhelm-desktop: ...`
# line and exit status 1, never a panic. `ensure_private_dir` is the very
# first fallible step `DesktopBootstrap::start` takes, well before the tmux
# preflight or the sibling `farhelm` are ever consulted, so a state
# "directory" that is actually a plain file is the cheapest way to reach
# this path: `DirBuilder::create` fails immediately with the OS's own
# error text, which is why this leg cannot pin an exact message the way the
# two tmux legs above do — the text is `ENOENT`/`EEXIST`-shaped OS prose,
# not one this project renders itself.
echo "== a non-tmux bootstrap failure exits 1 with a plain message, no panic"
BOGUS_STATE="$X/pf-bogus-state"
: >"$BOGUS_STATE"
BOGUS_STATE_STDERR="$X/pf-bogus-state.stderr"
FARHELM_DESKTOP_STATE_DIR="$BOGUS_STATE" \
  FARHELM_DESKTOP_PORT="$PORT" \
  FARHELM_DESKTOP_FARHELM="$BUILT_FARHELM" \
  FARHELM_DESKTOP_UI_DIST="$WEB_DIST" \
  "$BUILT_FARHELM_DESKTOP" >/dev/null 2>"$BOGUS_STATE_STDERR"
BOGUS_STATE_STATUS=$?
[ "$BOGUS_STATE_STATUS" -eq 1 ] ||
  fail "farhelm-desktop exited $BOGUS_STATE_STATUS against an unusable state directory (expected exactly 1)"
grep -q "^farhelm-desktop: " "$BOGUS_STATE_STDERR" ||
  fail "expected a plain 'farhelm-desktop: ...' line (see $BOGUS_STATE_STDERR)"
for banned in panicked RUST_BACKTRACE; do
  if grep -qF "$banned" "$BOGUS_STATE_STDERR"; then
    fail "non-tmux bootstrap failure must never contain $banned (see $BOGUS_STATE_STDERR)"
  fi
done

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
  PATH="/usr/bin:/bin" \
  FARHELM_TMUX="$SENTINEL_TMUX" \
  FARHELM_SMOKE_TMUX_MARKER="$X/tmux-override-used" \
  FARHELM_SMOKE_CLIENT_LOG_MARKER="$CLIENT_LOG_MARKER" \
  RUST_LOG="info,farhelm_ui::desktop=debug" \
  FARHELM_DESKTOP_PORT="$PORT" \
  FARHELM_DESKTOP_STATE_DIR="$X/state" \
  FARHELM_DESKTOP_FARHELM="$BUILT_FARHELM" \
  FARHELM_DESKTOP_UI_DIST="$WEB_DIST" \
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
[ -s "$X/tmux-override-used" ] || fail "managed supervisor did not run the tmux named by FARHELM_TMUX"
# A nonempty marker alone is not enough: the desktop's own tmux preflight
# calls this same wrapper with plain `-V` before the managed supervisor
# spawns, AND the supervisor's own `ensure_server` (crates/farhelm-supervisor/
# src/tmux.rs) issues its own private-server version check — `-S <socket> -f
# <config> -V` — before it ever touches the server. That second probe line
# still ENDS in `-V`, so a check for "any logged line that is not exactly
# `-V`" is satisfied by it alone and proves nothing about whether the
# supervisor went on to actually use this tmux. Require a line naming a
# known OPERATIONAL command instead — `start-server`, the very next thing
# `ensure_server` runs after its version check succeeds — which only a real
# session-owning invocation can produce.
grep -qw 'start-server' "$X/tmux-override-used" ||
  fail "the sentinel tmux never ran an operational command (start-server); only version probes were logged"
[ ! -e "$(dirname "$APP")/tmux" ] || fail "the app directory must not carry a tmux of its own"
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
CREATE_BODY=$(python3 -c 'import json,sys; print(json.dumps({"cwd": sys.argv[1], "invocation": "bash", "title": "zzz-remembered"}))' "$X/work") || fail "encoding remembered smoke session"
SID=$(curl_auth -sf --max-time 10 -H 'content-type: application/json' -d "$CREATE_BODY" "$API/api/sessions" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])') || fail "creating the pre-restart smoke session"
[ -n "$SID" ] || fail "the pre-restart create returned no session id"
tmux -S "$X/state/tmux.sock" has-session -t "fh-$SID" 2>/dev/null || fail "the created session did not reach the supervisor's tmux"
sleep 1
NEWEST_BODY=$(python3 -c 'import json,sys; print(json.dumps({"cwd": sys.argv[1], "invocation": "bash", "title": "aaa-newest"}))' "$X/work") || fail "encoding newest smoke session"
SID_NEWEST=$(curl_auth -sf --max-time 10 -H 'content-type: application/json' -d "$NEWEST_BODY" "$API/api/sessions" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])') || fail "creating the newest pre-restart smoke session"
[ -n "$SID_NEWEST" ] || fail "the newest pre-restart create returned no session id"
tmux -S "$X/state/tmux.sock" has-session -t "fh-$SID_NEWEST" 2>/dev/null || fail "the newest session did not reach the supervisor's tmux"

# The default gate deliberately does not click rows: WebKitGTK's unreliable
# Xvfb paint is why its CI leg is non-pixel. Seed the helm's shared preference
# (SPEC.md, Session list: one row for every client, `PUT /api/preferences`)
# with a non-default order and the non-newest session, then observe
# restoration through the native request trace (`sort=title`) and tmux's
# client ownership (the remembered, non-newest session gains the page's
# output client). A `GET` read-back after the restart checks the durable
# input survived the relaunch, as a check of the input rather than a
# substitute for those behavioral observations.
# The post-restart `sort=title` oracle proves nothing if the app already
# asked for that order on its own, so the entire unseeded first launch — up
# to this point, sessions created and all — must be free of such a request.
grep -qxF 'desktop_smoke: session listing requested query=sort=title' "$X/desktop.log" \
  && fail "first boot requested sort=title before any preference was seeded"
SEED_BODY=$(python3 -c 'import json,sys; print(json.dumps({"list_sort": "title", "last_selected": sys.argv[1]}))' "$SID") || fail "encoding the preference seed"
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
  PATH="/usr/bin:/bin" \
  FARHELM_TMUX="$SENTINEL_TMUX" \
  FARHELM_SMOKE_TMUX_MARKER="$X/tmux-override-used" \
  FARHELM_SMOKE_CLIENT_LOG_MARKER="$CLIENT_LOG_MARKER" \
  RUST_LOG="info,farhelm_ui::desktop=debug" \
  FARHELM_DESKTOP_PORT="$PORT" \
  FARHELM_DESKTOP_STATE_DIR="$X/state" \
  FARHELM_DESKTOP_FARHELM="$BUILT_FARHELM" \
  FARHELM_DESKTOP_UI_DIST="$WEB_DIST" \
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
RESTART_PREFERENCES=$(curl_auth -sf --max-time 5 "$API/api/preferences") || fail "reading the shared preference after the restart"
RESTART_SELECTION=$(printf '%s' "$RESTART_PREFERENCES" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("last_selected") or "")')
RESTART_SORT=$(printf '%s' "$RESTART_PREFERENCES" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("list_sort") or "")')
[ "$RESTART_SELECTION" = "$SID" ] || fail "restart changed the helm's remembered selection"
[ "$RESTART_SORT" = "title" ] || fail "restart dropped the helm's remembered sort"
tmux -S "$X/state/tmux.sock" has-session -t "fh-$SID" 2>/dev/null || fail "the tmux-held session did not survive the app restart"
wait_for_listing_request "$X/desktop-restart.log" "sort=title"
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
"$BUILT_FARHELM" helm token rotate --state-dir "$X/state" >/dev/null || fail "rotating desktop helm token"
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

  # Every tmux-preflight leg so far starts from an EMPTY state directory and
  # so only ever exercises `DesktopBootstrap::start`'s `Absent` branch — a
  # missing or below-floor `FARHELM_TMUX` refuses because there was no
  # supervisor around to make that irrelevant. `start`'s own doc comment
  # claims a stronger rule: an ANSWERING supervisor is an ownership boundary
  # reused exactly as it stands, so its tmux is authoritative regardless of
  # what this process's own `FARHELM_TMUX` says — the preflight must not
  # even run. Proving that needs a real existing supervisor, which is what
  # this leg spins up by hand: a `farhelm supervisor run` with a KNOWN-GOOD
  # tmux, then a fresh desktop process pointed at that SAME state directory
  # with a `FARHELM_TMUX` that names a path nothing occupies. If the
  # preflight ever moved ahead of discovery (or discovery stopped being
  # tried first), this candidate would be exactly what makes it refuse.
  #
  # Reuses the Xvfb/openbox display already up for the main launch above —
  # dioxus-desktop still needs one to reach this supervisor at all, even
  # though this leg drives no window and clicks nothing — and the exact
  # DOCTYPE readiness check the main launch used earlier, so a broken
  # bypass fails the same oracle a broken embedded helm would.
  echo "== an answering supervisor bypasses the desktop's own tmux preflight"
  ANSWERING_STATE="$X/answering-state"
  mkdir -m 0700 "$ANSWERING_STATE"
  ANSWERING_SUPERVISOR_LOG="$X/answering-supervisor.log"
  "$BUILT_FARHELM" supervisor run --state-dir "$ANSWERING_STATE" --tmux "$HOST_TMUX" \
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

  # A path inside this run's own private directory, deliberately never
  # created — the same reasoning as the missing-tmux preflight leg above
  # applies here: a global path like `/nonexistent/tmux` could exist on
  # some machine and defeat the point. A different port than the main
  # launch's, since that app has only just been asked to close and its
  # listener may still be draining.
  ANSWERING_BAD_TMUX="$X/answering-bad-tmux/tmux"
  ANSWERING_PORT=$((PORT + 1))
  ANSWERING_DESKTOP_LOG="$X/answering-desktop.log"
  DISPLAY=$DISP \
    FARHELM_TMUX="$ANSWERING_BAD_TMUX" \
    FARHELM_DESKTOP_PORT="$ANSWERING_PORT" \
    FARHELM_DESKTOP_STATE_DIR="$ANSWERING_STATE" \
    FARHELM_DESKTOP_FARHELM="$BUILT_FARHELM" \
    FARHELM_DESKTOP_UI_DIST="$WEB_DIST" \
    "$APP" >"$ANSWERING_DESKTOP_LOG" 2>&1 &
  ANSWERING_DESKTOP_PID=$!

  ANSWERING_READY=""
  for _ in $(seq 1 30); do
    if curl_local -sf --max-time 2 "http://127.0.0.1:$ANSWERING_PORT/" | grep -q '<!DOCTYPE html>'; then
      ANSWERING_READY=1
      break
    fi
    kill -0 "$ANSWERING_DESKTOP_PID" 2>/dev/null || break
    sleep 1
  done
  [ -n "$ANSWERING_READY" ] ||
    fail "desktop against an answering supervisor never served the bundled UI (see $ANSWERING_DESKTOP_LOG)"
  # The preflight's refusal text ("needs tmux ...") naming the bad
  # candidate would be definitive proof it ran; its absence plus the
  # candidate never existing (checked next) is what proves it did not.
  if grep -q "needs tmux" "$ANSWERING_DESKTOP_LOG"; then
    fail "desktop against an answering supervisor ran its own tmux preflight and refused (see $ANSWERING_DESKTOP_LOG)"
  fi
  [ ! -e "$ANSWERING_BAD_TMUX" ] ||
    fail "the bad FARHELM_TMUX candidate must never be invoked when a supervisor already answers"

  kill "$ANSWERING_DESKTOP_PID" 2>/dev/null
  wait "$ANSWERING_DESKTOP_PID" 2>/dev/null
  kill "$ANSWERING_SUPERVISOR_PID" 2>/dev/null
  wait "$ANSWERING_SUPERVISOR_PID" 2>/dev/null

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
