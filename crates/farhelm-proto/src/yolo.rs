//! What counts as a YOLO launch: a launch that lets the agent act without
//! asking for approval.
//!
//! Two shapes of launch reach a helm. A structured launch carries its
//! permission choice ([`LaunchSelection::permissions`]), so it is YOLO when
//! that choice is YOLO or when its harness has no other mode
//! ([`crate::LaunchHarness::sole_permission`], which makes every Pi launch
//! YOLO). A raw command line or profile invocation carries only argv, so it
//! is YOLO when its program is a recognized vendor CLI and a leading switch
//! is one of that vendor's permission-bypass flags, or when its program is
//! one whose only mode is YOLO.
//!
//! The raw recognition is shared with the browser's session row, which
//! badges the same flags; keeping one table here is what stops the badge and
//! the helm's refusal from disagreeing about the same command line. Codex's
//! `--full-auto` is recognized (the row badges it) but is not YOLO: it skips
//! prompts but keeps Codex's sandbox.

use crate::{LaunchPermission, LaunchSelection};

/// A permission-changing flag recognized on a vendor CLI's command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationMarker {
    /// A YOLO flag by that name (Codex's `--yolo`, Muse's, Cursor's).
    Yolo,
    /// A flag that bypasses approvals and the sandbox (Codex's long form).
    NoSandbox,
    /// A flag that skips every permission prompt (Claude Code's).
    SkipPerms,
    /// Codex's sandboxed auto-approval: prompts skipped, sandbox kept.
    FullAuto,
}

impl InvocationMarker {
    /// Whether this flag makes the launch a YOLO launch. Everything but the
    /// sandboxed `--full-auto` does.
    #[warn(clippy::wildcard_enum_match_arm)]
    pub fn is_yolo(self) -> bool {
        match self {
            InvocationMarker::Yolo | InvocationMarker::NoSandbox | InvocationMarker::SkipPerms => {
                true
            }
            InvocationMarker::FullAuto => false,
        }
    }
}

/// Vendor-recognized executables and the unattended-mode flags worth naming
/// for each, most consequential first within a program's own list.
///
/// Keyed by the program's BASENAME rather than a flat flag table: `--yolo`
/// belongs to Codex, and matching it against ANY program's argv would badge
/// `echo --yolo` or a future tool that happens to share a flag spelling with
/// a vendor it has nothing to do with. A basename absent from this table
/// earns no marker no matter what its arguments look like — nothing here
/// guesses at a command it does not recognize.
///
/// These flags share one property: they change what the agent is ALLOWED to
/// do without asking, which is the one fact about a command line worth four
/// characters of a sidebar row. Everything else — model pins, prompts,
/// working-directory overrides — is argument noise at this size and stays
/// in the `title` attribute with the rest of the line. Within one program's
/// list, order is precedence: an invocation carrying two of that vendor's
/// flags renders the first one listed.
const INVOCATION_MARKERS: &[(&str, &[(&str, InvocationMarker)])] = &[
    (
        "claude",
        &[
            // Claude Code: skips every permission prompt.
            (
                "--dangerously-skip-permissions",
                InvocationMarker::SkipPerms,
            ),
        ],
    ),
    (
        "codex",
        &[
            // The unabbreviated flag: bypasses approvals AND the sandbox.
            (
                "--dangerously-bypass-approvals-and-sandbox",
                InvocationMarker::NoSandbox,
            ),
            // `--yolo` is Codex's own alias for the flag directly above —
            // same bypass of approvals AND sandbox, shorter to type. It is
            // NOT the sandboxed auto mode; see `--full-auto` below for that
            // one, and do not conflate the two in future edits here.
            ("--yolo", InvocationMarker::Yolo),
            // Full auto-approval, but SANDBOXED: prompts are skipped, the
            // sandbox stays enforced. Strictly less permissive than the two
            // flags above, which is exactly why it earns a marker of its
            // own rather than collapsing into "yolo".
            ("--full-auto", InvocationMarker::FullAuto),
        ],
    ),
    ("muse", &[("--yolo", InvocationMarker::Yolo)]),
    // OpenCode calls its permission-bypass mode `--auto`; it is a YOLO
    // equivalent, unlike Codex's separately sandboxed `--full-auto`.
    ("opencode", &[("--auto", InvocationMarker::Yolo)]),
    (
        "agent",
        &[
            ("--force", InvocationMarker::Yolo),
            ("--yolo", InvocationMarker::Yolo),
        ],
    ),
    (
        "cursor-agent",
        &[
            ("--force", InvocationMarker::Yolo),
            ("--yolo", InvocationMarker::Yolo),
        ],
    ),
];

/// YOLO permission modes a vendor spells as an option with a value rather
/// than as a flag of its own: `(program, option, value)`, in either the
/// two-argument or the `option=value` spelling. OMP's `--approval-mode yolo`
/// (its other values, `always-ask` and `write`, are not YOLO), and Claude
/// Code's `--permission-mode bypassPermissions`, which its CLI reference
/// documents as the same mode `--dangerously-skip-permissions` selects.
const YOLO_OPTION_VALUES: &[(&str, &str, &str)] = &[
    ("omp", "--approval-mode", "yolo"),
    ("claude", "--permission-mode", "bypassPermissions"),
];

/// YOLO flags recognized only by the guard ([`argv_is_yolo`]), not by the
/// row badge's [`invocation_marker`]: Grok's `--always-approve` follows its
/// mandatory `--no-leader`, a switch the badge's leading-switch parser does
/// not know and stops at.
const GUARD_ONLY_YOLO_FLAGS: &[(&str, &str)] = &[("grok", "--always-approve")];

/// Programs whose only permission mode is YOLO, so any invocation of them is
/// a YOLO launch whatever its arguments. Pi has no tool-approval gate
/// ([`crate::LaunchHarness::sole_permission`]).
const SOLE_YOLO_PROGRAMS: &[&str] = &["pi"];

/// The basename of argv's program, the key both tables above use: the
/// token after its last `/`, or the whole token when that is empty (a
/// program spelled `/usr/bin/` has no basename to take).
pub fn program_basename(argv: &[String]) -> Option<&str> {
    let program = argv.first()?;
    Some(match program.rsplit('/').next() {
        Some(name) if !name.is_empty() => name,
        _ => program.as_str(),
    })
}

/// The first recognized permission flag among argv's leading switches, in
/// the program's own precedence order, or `None` for an unrecognized
/// program or no recognized flag.
pub fn invocation_marker(argv: &[String]) -> Option<InvocationMarker> {
    let basename = program_basename(argv)?;
    let (_, flags) = INVOCATION_MARKERS
        .iter()
        .find(|(vendor, _)| *vendor == basename)?;
    let leading_args = invocation_switches(basename, &argv[1..], flags);
    flags
        .iter()
        .find_map(|(flag, marker)| leading_args.contains(flag).then_some(*marker))
}

/// Whether a raw command line (already split into argv) is a YOLO launch,
/// for the helm's sensitive-host guard.
///
/// Deliberately broader than [`invocation_marker`]: any argument before a
/// `--` that exactly equals one of the program's YOLO-class flags counts,
/// wherever it sits, as does a YOLO option value such as OMP's
/// `--approval-mode yolo`. The badge parser stops at the first option it does
/// not know, which is right for a label and wrong for a guard: a user profile
/// like `claude --effort high --dangerously-skip-permissions` would slip past
/// it. The cost runs the other way here. A flag spelling that is really some
/// other option's value (`codex -c --yolo`) is counted too, which only asks
/// the user a question; a miss would start a YOLO session nobody confirmed.
pub fn argv_is_yolo(argv: &[String]) -> bool {
    let Some(basename) = program_basename(argv) else {
        return false;
    };
    if SOLE_YOLO_PROGRAMS.contains(&basename) {
        return true;
    }
    let flags = INVOCATION_MARKERS
        .iter()
        .filter(|(vendor, _)| *vendor == basename)
        .flat_map(|(_, flags)| flags.iter())
        .filter(|(_, marker)| marker.is_yolo())
        .map(|(flag, _)| *flag)
        .chain(
            GUARD_ONLY_YOLO_FLAGS
                .iter()
                .filter(|(vendor, _)| *vendor == basename)
                .map(|(_, flag)| *flag),
        )
        .collect::<Vec<_>>();
    let options = YOLO_OPTION_VALUES
        .iter()
        .filter(|(vendor, _, _)| *vendor == basename)
        .collect::<Vec<_>>();
    let args = argv[1..]
        .iter()
        .map(String::as_str)
        .take_while(|arg| *arg != "--")
        .collect::<Vec<_>>();
    args.iter().enumerate().any(|(i, arg)| {
        flags.contains(arg)
            || options.iter().any(|(_, option, value)| {
                (arg == option && args.get(i + 1) == Some(value))
                    || arg
                        .strip_prefix(option)
                        .and_then(|rest| rest.strip_prefix('='))
                        == Some(value)
            })
    })
}

/// Whether a structured launch is a YOLO launch: its harness's only mode is
/// YOLO, or its permission choice is YOLO.
pub fn selection_is_yolo(selection: &LaunchSelection) -> bool {
    selection.harness.sole_permission() == Some(LaunchPermission::Yolo)
        || selection.permissions == Some(LaunchPermission::Yolo)
}

/// Recognize only switches whose argv role is unambiguous.
///
/// Shell quoting does not distinguish a switch from an option value after
/// splitting. Skip the values of known options, and stop at unknown syntax
/// (including subcommands), rather than claiming that a prompt/config value
/// changes permissions. This deliberately recognizes only a prefix of legacy
/// commands; structured launch provenance does not need this approximation.
fn invocation_switches<'a>(
    vendor: &str,
    args: &'a [String],
    markers: &[(&str, InvocationMarker)],
) -> Vec<&'a str> {
    let valued: &[&str] = match vendor {
        "codex" => &[
            "-c",
            "--config",
            "-m",
            "--model",
            "-p",
            "--profile",
            "-C",
            "--cd",
            "-s",
            "--sandbox",
            "-a",
            "--ask-for-approval",
            "--add-dir",
            "--enable",
            "--disable",
            "-i",
            "--image",
        ],
        "claude" => &[
            "--model",
            "--permission-mode",
            "--output-format",
            "--input-format",
            "--system-prompt",
            "--append-system-prompt",
            "--settings",
        ],
        "muse" | "opencode" => &["--model", "-m"],
        "agent" | "cursor-agent" => &["--model"],
        _ => &[],
    };
    let mut switches = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if markers.iter().any(|(flag, _)| arg == flag) {
            switches.push(arg.as_str());
        } else if valued.contains(&arg.as_str()) {
            // A marker-looking value remains data, even when quoted.
            if args.next().is_none_or(|value| value == "--") {
                break;
            }
        } else if arg
            .split_once('=')
            .is_some_and(|(key, _)| valued.contains(&key))
        {
            continue;
        } else {
            break;
        }
    }
    switches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LaunchHarness;

    fn argv(line: &str) -> Vec<String> {
        shell_words::split(line).expect("test argv")
    }

    /// Spec: a raw command line is YOLO when a recognized vendor program
    /// carries one of its YOLO-class flags anywhere before `--` (Claude's
    /// skip-permissions, Codex's `--yolo` and its long form, Muse's,
    /// OpenCode's `--auto`, Cursor's `--force`/`--yolo`, Grok's
    /// `--always-approve`), OMP's `--approval-mode yolo` or Claude's
    /// `--permission-mode bypassPermissions` in either spelling, or when its
    /// program is Pi. Codex's sandboxed `--full-auto`,
    /// OMP's other approval modes, an unrecognized program carrying the same
    /// spelling, and anything after `--` are not. A flag spelling used as
    /// another option's value counts, by design.
    ///
    /// Why: this decides whether the helm refuses a launch on a sensitive
    /// host. A miss starts a YOLO session the user never confirmed; a false
    /// hit only asks. The command lines Farhelm's own launch compiler writes
    /// (every harness's YOLO spelling, after its other options) are the ones
    /// this has to get right first.
    #[test]
    fn raw_command_lines_are_classified_by_program_and_leading_flag() {
        for yolo in [
            "claude --dangerously-skip-permissions",
            "/opt/bin/codex --yolo",
            "codex --dangerously-bypass-approvals-and-sandbox",
            "codex -m gpt --yolo",
            "muse --yolo",
            "opencode --auto",
            "agent --force",
            "cursor-agent --yolo",
            "pi",
            "/usr/local/bin/pi --model x",
            "grok --no-leader --always-approve",
            "omp --provider openrouter --model x --thinking high --approval-mode yolo",
            "omp --approval-mode=yolo",
            "claude --permission-mode bypassPermissions",
            "claude --model m --permission-mode=bypassPermissions",
            "claude --effort high --dangerously-skip-permissions",
            "muse --model m --reasoning-effort high --yolo",
            "codex -c --yolo",
            "claude --append-system-prompt --dangerously-skip-permissions",
        ] {
            assert!(argv_is_yolo(&argv(yolo)), "{yolo}");
        }
        for not_yolo in [
            "claude",
            "codex --full-auto",
            "codex",
            "echo --yolo",
            "codex -- --yolo",
            "grok --no-leader",
            "omp --approval-mode always-ask",
            "claude --permission-mode acceptEdits",
            "omp --approval-mode",
            "omp yolo",
            "",
        ] {
            assert!(!argv_is_yolo(&argv(not_yolo)), "{not_yolo}");
        }
        assert_eq!(
            invocation_marker(&argv("codex --full-auto")),
            Some(InvocationMarker::FullAuto)
        );
    }

    /// Spec: a structured launch is YOLO when its permission choice is YOLO,
    /// or when its harness's only mode is YOLO (Pi, even with the permission
    /// omitted); any other choice, including none, is not.
    ///
    /// Why: structured launches never pass through the argv classifier on
    /// their way to the guard, so this is their whole classification.
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
        assert!(selection_is_yolo(&selection(LaunchHarness::Pi, None)));
        assert!(!selection_is_yolo(&selection(LaunchHarness::Codex, None)));
        assert!(!selection_is_yolo(&selection(
            LaunchHarness::Omp,
            Some(LaunchPermission::Approve)
        )));
    }
}
