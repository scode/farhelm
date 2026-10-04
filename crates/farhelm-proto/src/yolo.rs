//! What counts as a YOLO launch: a launch that lets the agent act without
//! asking for approval.
//!
//! The launch says so itself ([`crate::SessionLaunch::yolo`]): an agent
//! launch by its effective permission ([`selection_is_yolo`](crate::yolo::selection_is_yolo)), resolved
//! through [`crate::LaunchHarness::effective_permission`] (an omitted Pi,
//! OpenCode, OMP or Goose permission means YOLO; supported approval choices
//! still win), and a command launch by the user's assertion. Farhelm never
//! reads a command line to decide or second-guess it (SPEC.md, Creation).
//!
//! The `env`-prefix helpers below are not classification: they serve the
//! supervisor's injection for legacy sessions, the one place a command line
//! is still read.

use crate::{LaunchPermission, LaunchSelection};

/// Locate the program behind a leading simple `env NAME=value …` prefix:
/// index 0 when argv does not start with `env`, the first word after the
/// `NAME=value` assignments when it does, and `None` when there is no such
/// word or `env` is given an option (`env -i`, `env -u NAME`).
///
/// Only a legacy session's launch is ever read this way: the supervisor's
/// injection for sessions from before launch kinds looks past the prefix,
/// and wraps Goose, Pi and OMP launches in exactly this shape to pass their
/// reporter controls (SPEC.md keeps that path for legacy sessions alone).
/// Option-bearing `env` commands have parsing rules of their own and are
/// deliberately not interpreted: callers decide what an unknown shape means
/// for them. `env` is recognized by [`is_env_program`].
pub fn effective_program_index(argv: &[String]) -> Option<usize> {
    std::path::Path::new(argv.first()?).file_name()?;
    if !is_env_program(&argv[0]) {
        return Some(0);
    }
    for (index, argument) in argv.iter().enumerate().skip(1) {
        if argument.starts_with('-') {
            return None;
        }
        if argument.contains('=') {
            continue;
        }
        return Some(index);
    }
    None
}

/// Whether `program` is `env`, judged by its file name as `Path` reads it.
///
/// The legacy injection's one test for `env` (it wraps launches in an `env`
/// prefix and looks past one), by `Path`'s reading of the token.
pub fn is_env_program(program: &str) -> bool {
    std::path::Path::new(program)
        .file_name()
        .is_some_and(|name| name == "env")
}

/// Whether a structured launch has an effective YOLO permission, including
/// older snapshots that omitted a now-explicit default.
pub fn selection_is_yolo(selection: &LaunchSelection) -> bool {
    selection
        .harness
        .effective_permission(selection.permissions)
        == Some(LaunchPermission::Yolo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LaunchHarness;

    fn argv(line: &str) -> Vec<String> {
        shell_words::split(line).expect("test argv")
    }

    /// Spec: the shared program locator returns 0 for a command without an
    /// `env` prefix, the first non-assignment word after a simple prefix, and
    /// `None` for an option-bearing `env` or a prefix with no program.
    ///
    /// Why: a legacy session's hook injection wraps and reads launches in
    /// exactly this shape, so a drift here would hook the wrong program.
    #[test]
    fn effective_program_index_finds_the_program_behind_a_simple_env_prefix() {
        assert_eq!(effective_program_index(&argv("claude --x")), Some(0));
        assert_eq!(
            effective_program_index(&argv("env A=1 B=2 goose run")),
            Some(3)
        );
        assert_eq!(
            effective_program_index(&argv("/usr/bin/env A=1 pi")),
            Some(2)
        );
        assert_eq!(effective_program_index(&argv("env -i pi")), None);
        assert_eq!(effective_program_index(&argv("env A=1")), None);
        assert_eq!(effective_program_index(&[]), None);
    }

    /// Spec: a structured launch is YOLO when its permission choice is YOLO,
    /// or when omission implies YOLO. Explicit supported approval modes must
    /// remain non-YOLO even on a harness with a YOLO default.
    ///
    /// Why: an agent launch's YOLO verdict is exactly this, and the helm's
    /// guard, the sidebar mark and the confirmation's reason all read it.
    #[test]
    fn structured_selections_are_classified_by_permission_and_sole_mode() {
        let selection = |harness, permissions| LaunchSelection {
            harness,
            model: None,
            effort: None,
            permissions,
            workspace_trust: None,
        };
        assert!(selection_is_yolo(&selection(
            LaunchHarness::Codex,
            Some(LaunchPermission::Yolo)
        )));
        for harness in [
            LaunchHarness::Pi,
            LaunchHarness::OpenCode,
            LaunchHarness::Omp,
            LaunchHarness::Goose,
        ] {
            assert!(selection_is_yolo(&selection(harness, None)), "{harness:?}");
        }
        for permission in [
            LaunchPermission::Approve,
            LaunchPermission::SmartApprove,
            LaunchPermission::Chat,
        ] {
            assert!(!selection_is_yolo(&selection(
                LaunchHarness::Goose,
                Some(permission)
            )));
        }

        assert!(!selection_is_yolo(&selection(LaunchHarness::Codex, None)));
        assert!(!selection_is_yolo(&selection(
            LaunchHarness::Omp,
            Some(LaunchPermission::Approve)
        )));
    }
}
