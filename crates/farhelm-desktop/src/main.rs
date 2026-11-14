//! The `farhelm-desktop` binary: the desktop app's process, which owns an embedded helm and the local supervisor.
//!
//! D6 ships the Mac app as two bare binaries, `farhelm` (CLI, helm, supervisor) and `farhelm-desktop` (this one),
//! installed side by side in `~/.local/bin`. `install.sh` also wraps copies of the pair in an assembled
//! `~/Applications/Farhelm.app` for launcher identity (SPEC_impl.md, "Native app packaging"); the sibling contract is
//! the same in both places.
//!
//! Everything this binary does lives in the library half ([`farhelm_desktop::run`]): it starts the embedded helm and the
//! local supervisor, then runs the GPUI app.

fn main() -> anyhow::Result<()> {
    farhelm_desktop::run()
}
