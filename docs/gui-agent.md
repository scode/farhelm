# Driving the GPUI app as an agent

NOTE: Linux only for now, debug builds only, and never against the live install. The Mac side of the bridge works with
the prototype but has no command here yet (see "On a Mac" below).

`scripts/gui-agent.py` launches the GPUI desktop app headless against a real embedded helm, a real local supervisor and
fake agents, and then lets you read the screen, click, type, press keys, wait and take screenshots, the way a browser
automation tool drives a web page. It is how an agent checks a GUI change it just made, and what end-to-end GUI tests
are built on.

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
prints its usage guide. `terminal-text` prints the focused terminal's text once the GPUI app has a terminal; until then
it says there is none.

`start` takes `--sessions N` (fake-agent sessions to seed, default 2, 0 for none), `--size WxH` (the virtual monitor,
default 1600x1200) and `--no-build` (reuse the binaries already in the target directory). It builds with whatever
`CARGO_TARGET_DIR` you set.

## What a run is made of

Everything lives under the run directory: the compositor's runtime directory and Wayland socket, the bridge's state and
socket (in a 0700 directory, because the socket is unauthenticated), the app's state directory, logs (`logs/` for
Mutter, `agent/app/app.log` for the app) and `run.json`, which records what `stop` needs. The Wayland display name is
random per run, so runs in different directories never meet.

The app is `farhelm-desktop` built with the `agent` cargo feature, which compiles the bridge in. It starts its helm and
supervisor exactly as an installed app does, except that its state directory and its `farhelm` program are the run's
(`FARHELM_DESKTOP_STATE_DIR`, `FARHELM_DESKTOP_FARHELM`). In an agent build it also writes its helm's address and a
credential for its own process to the run's `helm.json`, which is how `start` seeds sessions through the real helm API.
Seeded sessions are command launches of `farhelm-fixtures fake-agent --script basic`.

Rendering is Mesa's software Vulkan driver (lavapipe), as for the screenshot goldens, so you need `mesa-vulkan-drivers`
and `libegl-mesa0`, plus `mutter` and `dbus-run-session`. The compositor runs on its own session bus, where the
supervisor cannot reach your systemd user manager, so terminal hosts start without a transient scope (the supervisor's
ordinary fallback).

## Why the bridge never ships

The bridge's socket is unauthenticated, and agent sessions run as the same user as the app, so with the bridge in a
build any agent could click Allow on its own approval card. `farhelm-gui` therefore refuses to compile the `agent`
feature into any build with debug assertions off, which covers releases (`--release` and the `dist` profile) and
devbuilds (`scripts/devbuild-mac.sh` builds with `--release`). Nothing else turns the feature on.

## Things to know

- The app's first screen is still empty: the foundations it will draw with are being built (GPUI migration, M1). A
  snapshot shows the window and little else until the session list lands.
- Headless Mutter reports the window as inactive forever, so anything that pauses while the window is inactive (the
  animation ticker, once it exists) needs a test override in these runs (SPEC_impl.md, "GPUI cookbook: traps").
- `stop` signals only processes it can tie to the run, and only while each is still the process that was recorded (its
  pid and its start time both match): the app as recorded right after launch, the supervisor through the app's record of
  it plus the state directory on its command line, the compositor as recorded when it started. A `start` whose
  compositor never comes up stops that compositor itself. If the script is killed partway, look for leftovers by the run
  directory in their command lines.

## On a Mac

The bridge itself works on macOS (the prototype drove a real window on the test Mac over SSH), but this script does not
yet: it needs a logged-in GUI session and a different launch. That is still to come.
