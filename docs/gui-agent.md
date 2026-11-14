# Driving the GPUI app as an agent

NOTE: Debug builds only, and never against the live install. On Linux a run is headless; on a Mac it opens a real window
in the logged-in GUI session (see "On a Mac" below).

`scripts/gui-agent.py` launches the GPUI desktop app (headless on Linux) against a real embedded helm, a real local
supervisor and fake agents, and then lets you read the screen, click, type, press keys, wait and take screenshots, the
way a browser automation tool drives a web page. It is how an agent checks a GUI change it just made, and what
end-to-end GUI tests are built on.

## Quick start

- `scripts/gui-agent.py /tmp/farhelm-tmp/<id>/run1 start`: builds what it needs, starts a private headless Mutter,
  launches the app on it and seeds two fake-agent sessions. The run directory must be absolute and must not exist yet;
  put it under your scratch directory (AGENTS.md, "Agent scratch space").
- `scripts/gui-agent.py /tmp/farhelm-tmp/<id>/run1 snapshot -i`: the controls on screen, each with a ref (`@e3`), its
  role, its label and the source line that built it.
- `scripts/gui-agent.py /tmp/farhelm-tmp/<id>/run1 click @e3`, `... type @e6 "text"`, `... press enter`,
  `... wait --text "Saved"`, `... screenshot /abs/shot.png`.
- `scripts/gui-agent.py /tmp/farhelm-tmp/<id>/run1 stop`: quits the app and stops its supervisor, its terminal hosts and
  the compositor. Always run it, also after a failed `start`.

Every verb other than `start`, `stop` and `terminal-text` goes to the vendored `gpui-agent` command line unchanged, so
its README (`vendor/gpui-agent/README.md`, "Commands") is the full list. `gpui-agent skills get core`, run the same way,
prints its usage guide. `terminal-text` prints the focused terminal's text. While no terminal holds focus (after a click
on a button, say) it fails saying so rather than guessing which terminal was meant: click the terminal or Tab back into
it first.

`start` takes `--sessions N` (fake-agent sessions to seed, default 2, 0 for none), `--size WxH` (Linux only: Mutter's
virtual monitor, default 1600x1200; refused on a Mac, which uses its real screen), `--no-build` (reuse the binaries
already in the target directory), `--lock FILE` (hold an exclusive lock on FILE from before the build until `stop`; see
"On a Mac"), `--window-size WxH` (the window's first size, still fitted to the screen; on Linux it must fit Mutter's
monitor, and it includes the frame GPUI Kit draws inside the window there, a 20-pixel black shadow margin and a 1-pixel
border on every side, which `screenshot` captures too) and `--appearance light|dark` (start in that palette whatever the
system's appearance). The last two are test hooks of agent builds, passed to the app as environment variables that
`crates/gui/farhelm-gui/src/agent_startup.rs` reads, so a screenshot batch can capture each theme and size; they are not
the app's appearance setting, and a value the app cannot parse makes it refuse to start. It builds with whatever
`CARGO_TARGET_DIR` you set. If `start` fails or is interrupted, run `stop` anyway: it releases whatever `start` had
already taken, the lock included.

To compare the app with the old app's reference pictures, `scripts/gui-screenshot-batch.py` drives these same commands
over a fixed scene list and writes one browsable page (`docs/gpui/screenshot-batch.md`).

## What a run is made of

Everything lives under the run directory: on Linux the compositor's runtime directory and Wayland socket, the bridge's
state and socket (in a 0700 directory, because the socket is unauthenticated), the app's state directory, logs (`logs/`
for Mutter, `agent/app/app.log` for the app) and `run.json`, which records what `stop` needs. The Wayland display name
is random per run, so runs in different directories never meet.

The app is `farhelm-desktop` built with the `agent` cargo feature, which compiles the bridge in. It starts its helm and
supervisor exactly as an installed app does, except that its state directory and its `farhelm` program are the run's
(`FARHELM_DESKTOP_STATE_DIR`, `FARHELM_DESKTOP_FARHELM`). In an agent build it also writes its helm's address and a
credential for its own process to the run's `helm.json`, which is how `start` seeds sessions through the real helm API.
Seeded sessions are command launches of `farhelm-fixtures fake-agent --script basic`.

On Linux, rendering is Mesa's software Vulkan driver (lavapipe), as for the screenshot goldens, so you need
`mesa-vulkan-drivers` and `libegl-mesa0`, plus `mutter` and `dbus-run-session`. The compositor runs on its own session
bus, where the supervisor cannot reach your systemd user manager, so terminal hosts start without a transient scope (the
supervisor's ordinary fallback).

## Why the bridge never ships

The bridge's socket is unauthenticated, and agent sessions run as the same user as the app, so with the bridge in a
build any agent could click Allow on its own approval card. `farhelm-gui` therefore refuses to compile the `agent`
feature into any build with debug assertions off, which covers releases (`--release` and the `dist` profile) and
devbuilds (`scripts/devbuild-mac.sh` builds with `--release`). Nothing else turns the feature on.

## Things to know

- The app connects to its embedded helm and shows the connection's states (connecting, another window took over with
  Take back, the connection-lost notice) and the session list, where each row is a list item holding a button, both
  named by its session's title (`click text:"Fixture 2"` opens that session). Opening a session shows its terminal,
  which takes focus; `terminal-text` reads the focused one.
- Headless Mutter reports the window as inactive forever, so agent builds pin the animation ticker's notion of an active
  window on (SPEC_impl.md, "GPUI cookbook: traps"); otherwise nothing would animate in these runs.
- `stop` signals only processes it can tie to the run, and only while each is still the process that was recorded (its
  pid and its start time both match): the app as recorded right after launch, the supervisor through the app's record of
  it plus the state directory on its command line, the compositor as recorded when it started. A `start` whose
  compositor never comes up stops that compositor itself. If the script is killed partway, run `stop`; if `run.json` was
  never written, look for leftovers by the run directory in their command lines.

## On a Mac

The same commands work on a Mac, run on the Mac itself (over SSH is fine) as the user logged in at its console: the app
opens a real window in that GUI session, rendered with Metal, and there is no compositor to start. `start` refuses when
the console belongs to someone else or to no one, because a window could not open. Everything else is the same: the run
directory, the seeded fake-agent sessions, the bridge verbs, `terminal-text` and `stop`; the Linux-only parts (Mutter,
its bus and runtime directory, lavapipe, `--size`) do not apply. macOS limits a socket path to 103 bytes, so the run
directory may be a little shallower than on Linux.

Things that differ from a headless run:

- The window shares the screen with whatever else the session shows, and a process started over SSH is not brought to
  the front, so the window can open behind another app's window. Bridge screenshots are unaffected (the bridge renders
  the window itself), but a capture of the screen is not. A window that is not frontmost also draws at most about 30
  frames a second on macOS 26 (the display link of an inactive app is paced down), so measure frame pacing only with the
  app in front.
- A Mac's one GUI session is shared by every run on it. On a Mac several agents use, pass `--lock FILE` to `start` with
  the file every run there agrees on; on the migration's shared test Mac that is `~/.farhelm-gpui-mac.lock`, the same
  file its builds and measurements lock with `lockf -k`. `start` takes an exclusive `flock` on it before the build
  (waiting up to 30 minutes for another run) and holds it until `stop`, which releases it last, and only once nothing
  else of the run is left: if `stop` reports that something did not stop, the lock stays held until a second `stop`
  succeeds. A `start` killed while it waits leaves nothing holding the lock.
- Building GPUI on a small Mac is heavy: with `--lock`, the build already runs under the lock; cap its job count (for
  example `CARGO_BUILD_JOBS=6`). Do not also wrap `start` in `lockf -k` on the same file: the run would wait on itself.
- macOS may show a privacy or permission dialog (screen recording, accessibility, automation) the first time something
  captures the screen or posts input. The bridge itself needs none of these. If one appears, it is the machine owner's
  to answer; do not click it from a script.
