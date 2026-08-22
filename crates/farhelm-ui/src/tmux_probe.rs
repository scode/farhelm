//! Locating a usable tmux binary without trusting this process's PATH.
//!
//! macOS GUI apps are launched by launchd (via Finder/Dock/Spotlight), not by
//! a login shell, so they do not inherit the user's shell PATH — a Homebrew
//! `tmux` on `/opt/homebrew/bin` (or an Intel/MacPorts prefix) is invisible to
//! `Command::new("tmux")` even though it is right there on disk. This is the
//! macOS half of TODO.md's 2026-08-22 tmux floor decision (part 2: "User
//! instructions route through Homebrew ... and probes the known prefixes
//! itself"): the desktop app no longer bundles a private tmux at all, so it
//! must find the user's Homebrew install by checking the well-known
//! installation prefixes directly, in order, before ever falling back to
//! PATH.
//!
//! The search itself lives here as pure, dependency-free logic — no
//! `cfg(target_os = "macos")`, no filesystem access, no desktop feature gate —
//! specifically so its behavior (search order, no-match fallback, an empty
//! prefix list on non-macOS) is exercised by plain `cargo test` on Linux CI,
//! which is where this workspace's tests actually run. `desktop.rs` is the
//! only caller that knows about the actual filesystem and the actual
//! platform: it decides which prefix list applies (`cfg!(target_os =
//! "macos")`, not a `cfg` attribute, again so this stays testable everywhere)
//! and supplies the real executability check.

use std::path::{Path, PathBuf};

/// Homebrew and MacPorts tmux locations to check, in the order TODO.md's
/// floor decision specifies: Apple Silicon Homebrew, Intel Homebrew, then
/// MacPorts. Only consulted on macOS (see `desktop.rs`'s call site); kept as
/// a plain constant here, rather than behind a `cfg`, so a test can assert
/// against its exact contents and order.
pub(crate) const MACOS_TMUX_PREFIXES: &[&str] =
    &["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"];

/// Return the first `<prefix>/tmux` the predicate accepts, or `None` if no
/// prefix has one (including when `prefixes` is empty, which is how the
/// non-macOS call site opts out of probing entirely and defers to PATH).
///
/// `is_executable` is injected rather than hard-coded to a real filesystem
/// check so this search order — the actual behavior worth pinning — can be
/// tested with a fake predicate, independent of any real binary existing on
/// the test machine. Real callers pass a predicate that checks both
/// existence and the execute bit; see `desktop.rs`'s `is_executable_file`.
pub(crate) fn find_tmux_in_prefixes(
    prefixes: &[&str],
    mut is_executable: impl FnMut(&Path) -> bool,
) -> Option<PathBuf> {
    prefixes.iter().find_map(|prefix| {
        let candidate = Path::new(prefix).join("tmux");
        is_executable(&candidate).then_some(candidate)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The search must return the FIRST prefix that matches, not merely any
    /// match — a later, also-matching prefix must never shadow an earlier
    /// one. This is what makes the documented order (Apple Silicon Homebrew
    /// before Intel Homebrew before MacPorts) an actual guarantee rather than
    /// an accident of iteration.
    /// The production list IS the documented order — Apple Silicon
    /// Homebrew, Intel Homebrew, MacPorts — and its first entry is examined
    /// first. Pinned against the constant itself rather than a copy so a
    /// reordering or a dropped prefix fails here and not on a user's Mac.
    #[test]
    fn the_production_prefixes_are_the_documented_ones_and_the_first_is_tried() {
        assert_eq!(
            MACOS_TMUX_PREFIXES,
            &["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"]
        );
        let found = find_tmux_in_prefixes(MACOS_TMUX_PREFIXES, |candidate| {
            candidate == Path::new("/opt/homebrew/bin/tmux")
        });
        assert_eq!(found, Some(PathBuf::from("/opt/homebrew/bin/tmux")));
    }

    #[test]
    fn first_matching_prefix_wins_even_when_a_later_one_also_matches() {
        let prefixes = ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"];
        let found = find_tmux_in_prefixes(&prefixes, |candidate| {
            candidate == Path::new("/usr/local/bin/tmux")
                || candidate == Path::new("/opt/local/bin/tmux")
        });
        assert_eq!(found, Some(PathBuf::from("/usr/local/bin/tmux")));
    }

    /// A prefix earlier in the list that does not have tmux must not stop the
    /// search — later prefixes still get checked.
    #[test]
    fn a_non_matching_prefix_does_not_stop_the_search() {
        let prefixes = ["/opt/homebrew/bin", "/usr/local/bin"];
        let found = find_tmux_in_prefixes(&prefixes, |candidate| {
            candidate == Path::new("/usr/local/bin/tmux")
        });
        assert_eq!(found, Some(PathBuf::from("/usr/local/bin/tmux")));
    }

    /// No prefix has tmux: the caller must get a plain `None` so it can fall
    /// back to PATH, not a wrong guess.
    #[test]
    fn no_hit_returns_none() {
        let prefixes = ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"];
        let found = find_tmux_in_prefixes(&prefixes, |_| false);
        assert_eq!(found, None);
    }

    /// An empty prefix list is exactly how the non-macOS call site opts out
    /// of probing: it must return `None` unconditionally, even if the
    /// predicate would say yes to everything (proving the empty list itself
    /// is what short-circuits the search, not a predicate side effect).
    #[test]
    fn empty_prefix_list_never_probes_anything() {
        let found = find_tmux_in_prefixes(&[], |candidate| {
            panic!("nothing may be probed with no prefixes; asked about {candidate:?}")
        });
        assert_eq!(found, None);
    }
}
