# farhelm-desktop

The native window: a webview showing the Farhelm UI, with a helm running in the same process. Released as a bare binary
that installs next to `farhelm` in `~/.local/bin` (D6 in the distribution plan).

The local supervisor is not in this process. On startup the app probes for one that is already answering and reuses it
untouched; only if none answers does it start `farhelm supervisor run` from the sibling binary, as a separate child
whose lifetime it then owns. A supervisor it found rather than started is somebody else's, and the app neither tethers
to nor stops it.

NOTE: this crate is a shell and holds no logic of its own. `main` calls `farhelm_ui::desktop::run()` and that is the
whole file. Read `crates/farhelm-ui/src/desktop.rs` for what actually happens.

## Plain `cargo build` here is a compile check, not a way to get a working app

Two separate things are missing from a plain-Cargo binary, and setting `FARHELM_UI_DIST` only fixes the first.

The UI the window renders comes from the web bundle compiled into `farhelm-helm` at build time (`FARHELM_UI_DIST`, D12)
and is served to the webview by the desktop asset handler. Build without that variable and there is nothing to serve:
the helm starts, the window opens, and every asset request 404s.

Underneath that: the hashed asset names (`/assets/terminal-dxh<hash>.js`) are written into the binary by `dx` after the
link, not by rustc. It scans the built file for manganis's `__ASSETS__` symbols and overwrites each one's placeholder
with the real content-hashed path. Cargo alone never does this, so a plain-Cargo binary asks for the placeholder string
instead of a filename — and would 404 every asset even with the bundle embedded. `scripts/check-desktop-assets.sh`
enforces this and explains it at length.

So `cargo check -p farhelm-desktop` (what CI runs) tells you the crate still compiles. It does not produce something you
can run.

## Building a runnable one locally

Run from the repository root:

```
TARGET="${CARGO_TARGET_DIR:-target}"
mkdir -p "$TARGET" && TARGET=$(cd "$TARGET" && pwd)
rm -rf "$TARGET/dx"
(cd crates/farhelm-ui && dx build --package farhelm-ui --platform web --release)
FARHELM_UI_DIST="$TARGET/dx/farhelm-ui/release/web/public" \
  dx build --package farhelm-desktop --platform desktop --release
cargo build --release -p farhelm
APP="$TARGET/dx/farhelm-desktop/release/linux/app"
cp "$TARGET/release/farhelm" "$APP/farhelm"
"$APP/farhelm-desktop"
```

Three things there are not decoration. `CARGO_TARGET_DIR` is resolved to an absolute path ONCE and every path derived
from it, because both `dx` commands honour that variable and they run from different directories — left relative, one
setting names two trees and the desktop build embeds the wrong web bundle or none. `FARHELM_UI_DIST` must be absolute
for the same family of reasons, and `farhelm-helm`'s build script rejects a relative value outright. And `farhelm` has
to exist next to the desktop binary or the app refuses to start: copying it there is the shape a release install has,
though `FARHELM_DESKTOP_FARHELM=$TARGET/release/farhelm "$APP/farhelm-desktop"` works just as well without the copy.

The `dx` output path is `<target>/dx/farhelm-desktop/<profile>/<platform>/app/`, so `debug/` replaces `release/` without
the flag, and a Mac writes `macos/` where Linux writes `linux/`.

The `rm -rf "$TARGET/dx"` is not superstition: `dx` accumulates every generation it has ever built in that directory,
and a stale one is indistinguishable from a current one once it is embedded. It also wipes any concurrent `dx` build's
output in a shared target directory, which is why `scripts/check-desktop-assets.sh` uses a tree of its own instead.

For an automated Linux run, use `scripts/desktop-smoke.sh` instead — it drives the `dx`-built `farhelm-ui` desktop
binary under Xvfb, which is the same `desktop::run` shell with the asset names filled in.

## Environment overrides

- `FARHELM_DESKTOP_FARHELM` — path to the `farhelm` CLI, instead of the sibling next to this binary.
- `FARHELM_DESKTOP_UI_DIST` — a directory the embedded helm serves over loopback. It does not change what the window
  renders; the window's own assets always come from the embedded tree.
- `FARHELM_DESKTOP_STATE_DIR`, `FARHELM_DESKTOP_PORT` — state directory and loopback port.
