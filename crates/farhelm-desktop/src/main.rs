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
    // `--licenses` prints the notices this binary must carry and exits, before anything starts; the binary takes no
    // other arguments (a launcher may add its own, which are ignored as before).
    // `args_os`, not `args`: a launch argument that is not valid Unicode must not panic before the app starts.
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--licenses")
    {
        print!("{}", farhelm_desktop::licenses_text());
        return Ok(());
    }
    farhelm_desktop::run()
}
