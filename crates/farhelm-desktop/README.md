# farhelm-desktop

The desktop app's process: an internal helm running in the same process, the local supervisor it finds or starts, the
updater, and the app's own state files. It is a library the GPUI app is wired into, plus the `farhelm-desktop` binary,
which the installer puts in `~/Applications/Farhelm.app` as the app's main program (D6 in the distribution plan).

The binary starts the embedded helm and the local supervisor, then hands the main thread to the GPUI app
(`farhelm_gui::bootstrap::run`), whose window connects to the embedded helm over its UI channel. SIGINT and SIGTERM quit
the app the ordinary way. A startup failure, and a window that cannot open, take the pre-window refusal path (stderr,
exit status 1, and on macOS a native alert and a Console line).

The embedded helm serves the app over a fresh loopback port and serves no page. It mints the app's credential in memory
at startup and hands it over through its readiness channel; nothing is written to disk.

The local supervisor is not in this process. On startup the app probes for one that is already answering and reuses it
untouched; only if none answers does it start `farhelm supervisor run` from its own version of the CLI (its sibling, or
in the installed app `Contents/Versions/<version>/farhelm`), as a separate child whose lifetime it then owns. A
supervisor it found rather than started is somebody else's, and the app neither tethers to nor stops it.

## Building and running

`cargo build -p farhelm-desktop -p farhelm` builds both binaries. The app needs `farhelm` next to it, which is the shape
a release install has; `target/debug/` already has both, or set `FARHELM_DESKTOP_FARHELM` to the CLI to use.

The crate is outside the workspace's default members, so a bare `cargo build` does not compile it. On Linux it needs the
GLib development package for GIO, which opens saved downloads.

On a Mac, `scripts/devbuild-mac.sh` does the same thing in release mode, stages the pair in `target/devbuild/` with
Linux `farhelm` payloads built from the same working copy (so Update works on remote hosts), and runs the app on a state
directory of its own instead of the installed app's. Its header has the details.

Only the Mac app is released. A release builds it with cargo, through `scripts/build-desktop-binary.sh`, which is the
build command of the generic dist package `packaging/farhelm-desktop/dist.toml`: the script refuses a binary that must
not ship and stages the one it accepts with the app icon, which the archive needs and the cargo package alone would not
ship. The cargo package here is marked `dist = false` so the two descriptions cannot both produce an asset
(SPEC_impl.md, "Native app packaging", has why). On Linux the app is run from a build like the ones above (SPEC.md,
"Installation and updates").

## Environment overrides

- `FARHELM_DESKTOP_FARHELM` — path to the `farhelm` CLI, instead of the sibling next to this binary (or, in the Mac
  app's side-by-side version layout, this version's own `Contents/Versions/<version>/farhelm`).
- `FARHELM_DESKTOP_STATE_DIR` — state directory for the embedded helm and managed local supervisor.
- `FARHELM_HELM_PAYLOAD_DIR` — the embedded helm's provisioning payload directory, with the same meaning as the
  standalone helm's `--payload-dir`: setup and Update push the Linux binaries staged there, unverified. A build from a
  working copy has no release to download them from, so this is what lets it update remote hosts.
- `FARHELM_DESKTOP_UPDATE_LATEST` — `v<version>` of a stable release for the installed Mac app's updater to treat as
  latest, for the release test in `releasing/mac-vm-test/`. An invalid or empty value fails every check without asking
  `/latest`; the real download origin and verification remain unchanged. Restart to update removes it from the relaunch
  helper's environment (SPEC_impl.md, "The desktop app's updater").
- `FARHELM_RELEASE_BUILD` — read when the crates COMPILE, not when the app runs. Set and not empty, it marks a release
  build, whose embedded helm downloads host payloads from its own version's release by default
  (`crates/farhelm-helm/build.rs`); without it the helm has no payloads unless `FARHELM_HELM_PAYLOAD_DIR` names some.
  `scripts/build-desktop-binary.sh` refuses to build without it. A build of main carries no release version, so a
  release-marked one refuses that download instead.
