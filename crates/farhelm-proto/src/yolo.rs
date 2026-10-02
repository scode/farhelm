//! What counts as a YOLO launch: a launch that lets the agent act without
//! asking for approval.
//!
//! Two shapes of launch reach a helm. A structured launch carries its
//! permission choice ([`LaunchSelection::permissions`]), so it is YOLO when
//! that choice is YOLO or when its harness has no other mode
//! ([`crate::LaunchHarness::sole_permission`], which makes every Pi launch
//! YOLO). A raw command line or profile invocation carries only argv, so it
//! is YOLO when its program is a recognized vendor CLI and a leading switch
//! is one of that vendor's permission-bypass flags or its options spell a
//! YOLO mode (`--permission-mode bypassPermissions`, Codex's `-a never` with
//! `-s danger-full-access`), or when its program is one whose only mode is
//! YOLO. Recognition of raw command lines is best
//! effort (SPEC.md, the YOLO-launch paragraph): it covers the documented
//! shapes, including a program behind a simple `env NAME=value` prefix
//! ([`effective_program_index`]), but no argv classifier can see through an
//! arbitrary wrapper such as a script or `sh -c`.
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
    // Cursor's CLI, by its own program name only. The generic name `agent`
    // is deliberately absent: other tools install a command by that name
    // too, so a flag after `agent` says nothing about which vendor's
    // meaning applies. Farhelm launches Cursor as
    // `cursor-agent` for the same reason.
    (
        "cursor-agent",
        &[
            ("--force", InvocationMarker::Yolo),
            ("-f", InvocationMarker::Yolo),
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
///
/// Guard-only, like [`GUARD_ONLY_YOLO_FLAGS`]: the badge does not consult
/// it. [`YOLO_OPTION_SETS`] could express these rows too (one setting each),
/// and folding them in would badge them; that change of what the sidebar
/// shows is left for its own decision.
const YOLO_OPTION_VALUES: &[(&str, &str, &str)] = &[
    ("omp", "--approval-mode", "yolo"),
    ("claude", "--permission-mode", "bypassPermissions"),
];

/// One setting in a [`YOLO_OPTION_SETS`] entry: the `(option, value)`
/// spellings, any one of which sets it.
type SettingSpellings = &'static [(&'static str, &'static str)];

/// YOLO modes a vendor spells as several valued settings that must ALL be
/// present: `(program, marker, settings)`, where each setting lists the
/// `(option, value)` spellings that set it, each in either the two-argument
/// or the `option=value` form, in any order.
///
/// Codex's never-ask approval policy together with its `danger-full-access`
/// sandbox is the same mode as `--dangerously-bypass-approvals-and-sandbox`
/// (its CLI help), so it carries that flag's marker. Either setting alone is
/// not YOLO: `-a never` keeps Codex's sandbox, like `--full-auto`, and
/// `-s danger-full-access` keeps its approval prompts. The `-c`/`--config`
/// spellings set the same two config keys; Codex reads a config value as
/// TOML, so a quoted value (`approval_policy="never"`) is the same setting
/// ([`setting_value_matches`]).
const YOLO_OPTION_SETS: &[(&str, InvocationMarker, &[SettingSpellings])] = &[(
    "codex",
    InvocationMarker::NoSandbox,
    &[
        &[
            ("-a", "never"),
            ("--ask-for-approval", "never"),
            ("-c", "approval_policy=never"),
            ("--config", "approval_policy=never"),
        ],
        &[
            ("-s", "danger-full-access"),
            ("--sandbox", "danger-full-access"),
            ("-c", "sandbox_mode=danger-full-access"),
            ("--config", "sandbox_mode=danger-full-access"),
        ],
    ],
)];

/// Whether an option's value as found on the command line sets the value a
/// [`YOLO_OPTION_SETS`] spelling expects. A plain value must match exactly.
/// An expected `key=value` (a `-c` config setting) matches the same key with
/// the same value, allowing spaces around the `=` and one pair of TOML
/// quotes around the value, since Codex parses it as TOML.
fn setting_value_matches(found: &str, expected: &str) -> bool {
    if found == expected {
        return true;
    }
    let (Some((expected_key, expected_value)), Some((key, value))) =
        (expected.split_once('='), found.split_once('='))
    else {
        return false;
    };
    let value = value.trim();
    let unquoted = ['"', '\'']
        .iter()
        .find_map(|quote| {
            value
                .strip_prefix(*quote)
                .and_then(|rest| rest.strip_suffix(*quote))
        })
        .unwrap_or(value);
    key.trim() == expected_key && unquoted == expected_value
}

/// The markers of `program`'s [`YOLO_OPTION_SETS`] entries whose every
/// setting appears among `values`, the `(option, value)` pairs read off a
/// command line.
fn satisfied_option_sets<'a>(
    program: &'a str,
    values: &'a [(&str, &str)],
) -> impl Iterator<Item = InvocationMarker> + 'a {
    YOLO_OPTION_SETS
        .iter()
        .filter(move |(vendor, _, _)| *vendor == program)
        .filter(move |(_, _, settings)| {
            settings.iter().all(|spellings| {
                spellings.iter().any(|(option, expected)| {
                    values.iter().any(|(found_option, found)| {
                        found_option == option && setting_value_matches(found, expected)
                    })
                })
            })
        })
        .map(|(_, marker, _)| *marker)
}

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
    argv.first().map(|program| word_basename(program))
}

/// [`program_basename`]'s rule for a single word.
fn word_basename(word: &str) -> &str {
    match word.rsplit('/').next() {
        Some(name) if !name.is_empty() => name,
        _ => word,
    }
}

/// Locate the program behind a leading simple `env NAME=value …` prefix:
/// index 0 when argv does not start with `env`, the first word after the
/// `NAME=value` assignments when it does, and `None` when there is no such
/// word or `env` is given an option (`env -i`, `env -u NAME`).
///
/// This is the one copy of the rule, shared by the supervisor (which looks
/// past the prefix to inject hooks and build resume commands, and wraps
/// Goose, Pi and OMP launches in exactly this shape to pass their reporter
/// controls) and the YOLO classifier below, so the program the supervisor
/// integrates as an agent is the program the sensitive-host guard checks.
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
/// The one test for `env` that the supervisor (which wraps launches in an
/// `env` prefix and looks past one) and the classifier share. It differs
/// from [`program_basename`] only for a token ending in `/`, where `Path`
/// takes the last directory name; the supervisor's behavior depends on
/// `Path`'s reading, so this keeps it.
pub fn is_env_program(program: &str) -> bool {
    std::path::Path::new(program)
        .file_name()
        .is_some_and(|name| name == "env")
}

/// Whether `name` is a program some table in this module classifies, the
/// set the guard falls back on for an `env` command it does not interpret.
/// Derived from the tables themselves so a program added to one of them is
/// covered here without a second list to keep in step.
fn is_classified_program(name: &str) -> bool {
    INVOCATION_MARKERS
        .iter()
        .any(|(program, _)| *program == name)
        || YOLO_OPTION_VALUES
            .iter()
            .any(|(program, _, _)| *program == name)
        || YOLO_OPTION_SETS
            .iter()
            .any(|(program, _, _)| *program == name)
        || GUARD_ONLY_YOLO_FLAGS
            .iter()
            .any(|(program, _)| *program == name)
        || SOLE_YOLO_PROGRAMS.contains(&name)
}

/// The highest-precedence permission marker among argv's leading switches,
/// or `None` for an unrecognized program or none recognized: a recognized
/// flag, or a [`YOLO_OPTION_SETS`] mode whose settings all appear there,
/// ranked where its marker's flag sits in the program's own order.
///
/// The program is the one behind a simple `env NAME=value` prefix, if any
/// ([`effective_program_index`]). An `env` given an option earns no marker:
/// the badge labels only a flag it actually recognizes, unlike the guard's
/// [`argv_is_yolo`], which errs toward asking.
pub fn invocation_marker(argv: &[String]) -> Option<InvocationMarker> {
    let argv = &argv[effective_program_index(argv)?..];
    let basename = program_basename(argv)?;
    let (_, flags) = INVOCATION_MARKERS
        .iter()
        .find(|(vendor, _)| *vendor == basename)?;
    let (leading_args, values) = invocation_switches(basename, &argv[1..], flags);
    // A satisfied option set ranks where its marker's flag sits in the
    // program's own precedence order, so it competes with the flags fairly.
    let rank_of = |marker: InvocationMarker| {
        flags
            .iter()
            .position(|(_, listed)| *listed == marker)
            .unwrap_or(flags.len())
    };
    flags
        .iter()
        .enumerate()
        .filter(|(_, (flag, _))| leading_args.contains(flag))
        .map(|(rank, (_, marker))| (rank, *marker))
        .chain(satisfied_option_sets(basename, &values).map(|marker| (rank_of(marker), marker)))
        .min_by_key(|(rank, _)| *rank)
        .map(|(_, marker)| marker)
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
/// A mode spelled as several settings (Codex's `-a never` with
/// `-s danger-full-access`, [`YOLO_OPTION_SETS`]) counts when every setting
/// appears anywhere before the `--`.
///
/// A simple `env NAME=value` prefix is looked past to the program behind it
/// ([`effective_program_index`]), and the program found there is classified
/// the same way again, so a second `env` behind the first gets the same
/// treatment. An `env` given an option is not interpreted; the launch counts
/// as YOLO when any later word, or any piece of one split at whitespace or
/// `=` (`env -S 'claude …'` and `--split-string=…` hand `env` the whole
/// command as one word), has a basename that is a program these tables
/// classify. That asks about anything that might start a known agent while
/// leaving `env -u FOO ./build.sh` alone.
pub fn argv_is_yolo(argv: &[String]) -> bool {
    match effective_program_index(argv) {
        Some(0) => program_argv_is_yolo(argv),
        // The slice is strictly shorter, so this recursion ends.
        Some(program) => argv_is_yolo(&argv[program..]),
        None if argv.first().is_some_and(|program| is_env_program(program)) => argv[1..]
            .iter()
            .flat_map(|word| word.split(|c: char| c.is_whitespace() || c == '='))
            .any(|piece| is_classified_program(word_basename(piece))),
        None => false,
    }
}

/// [`argv_is_yolo`] for an argv whose first word is the program itself.
fn program_argv_is_yolo(argv: &[String]) -> bool {
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
    // Every `(option, value)` reading of the arguments: each word with the
    // one after it, each `option=value` word split at its first `=`, and
    // each single-dash word split after its short option (`-anever`, a
    // value attached the way clap-based CLIs such as Codex accept).
    // Readings that are not really option and value never match a setting.
    let values = args
        .iter()
        .zip(args.iter().skip(1))
        .map(|(option, value)| (*option, *value))
        .chain(args.iter().filter_map(|arg| arg.split_once('=')))
        .chain(args.iter().filter_map(|arg| attached_short_value(arg)))
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
    }) || satisfied_option_sets(basename, &values).any(InvocationMarker::is_yolo)
}

/// Whether a structured launch is a YOLO launch: its harness's only mode is
/// YOLO, or its permission choice is YOLO.
pub fn selection_is_yolo(selection: &LaunchSelection) -> bool {
    selection.harness.sole_permission() == Some(LaunchPermission::Yolo)
        || selection.permissions == Some(LaunchPermission::Yolo)
}

/// Split a single-dash word into its short option and an attached value
/// (`-anever` into `-a` and `never`), or `None` for anything else: a long
/// option, a bare short option, or a word that is not an option at all.
fn attached_short_value(arg: &str) -> Option<(&str, &str)> {
    if arg.starts_with("--") || !arg.starts_with('-') {
        return None;
    }
    let (option, value) = (arg.get(..2)?, arg.get(2..)?);
    (!value.is_empty()).then_some((option, value))
}

/// Recognize only switches whose argv role is unambiguous, and read the
/// values of the known valued options among them.
///
/// Shell quoting does not distinguish a switch from an option value after
/// splitting. Treat the values of known options as values, never as
/// switches, and stop at unknown syntax (including subcommands), rather than
/// claiming that a prompt/config value changes permissions. This
/// deliberately recognizes only a prefix of legacy commands; structured
/// launch provenance does not need this approximation. Returns the
/// recognized marker switches and the `(option, value)` pairs read, the
/// latter for [`YOLO_OPTION_SETS`].
fn invocation_switches<'a>(
    vendor: &str,
    args: &'a [String],
    markers: &[(&str, InvocationMarker)],
) -> (Vec<&'a str>, Vec<(&'a str, &'a str)>) {
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
        "cursor-agent" => &["--model"],
        _ => &[],
    };
    let mut switches = Vec::new();
    let mut values = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if markers.iter().any(|(flag, _)| arg == flag) {
            switches.push(arg.as_str());
        } else if valued.contains(&arg.as_str()) {
            // A marker-looking value remains data, even when quoted.
            match args.next() {
                Some(value) if value != "--" => values.push((arg.as_str(), value.as_str())),
                _ => break,
            }
        } else if let Some((key, value)) =
            arg.split_once('=').filter(|(key, _)| valued.contains(key))
        {
            values.push((key, value));
        } else if let Some((option, value)) =
            attached_short_value(arg).filter(|(option, _)| valued.contains(option))
        {
            values.push((option, value));
        } else {
            break;
        }
    }
    (switches, values)
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
    /// OpenCode's `--auto`, Cursor's `--force`/`-f`/`--yolo` under the name
    /// `cursor-agent`, Grok's `--always-approve`), OMP's
    /// `--approval-mode yolo` or Claude's `--permission-mode
    /// bypassPermissions` in either spelling, or when its program is Pi.
    /// Codex's sandboxed `--full-auto`, OMP's other approval modes, an
    /// unrecognized program carrying the same spelling, and anything after
    /// `--` are not. Neither is the generic program name `agent`, whatever
    /// flags follow it: other tools install a command by that name, so it is
    /// deliberately not interpreted (Farhelm launches Cursor as
    /// `cursor-agent`). The badge recognizes Cursor's `-f` too and ignores
    /// `agent`. A flag spelling used as
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
            "cursor-agent --force",
            "cursor-agent -f",
            "cursor-agent --model m -f",
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
            "agent --force",
            "agent -f",
            "agent --no-leader --always-approve",
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
        assert_eq!(
            invocation_marker(&argv("cursor-agent --model m -f")),
            Some(InvocationMarker::Yolo)
        );
        assert_eq!(invocation_marker(&argv("agent --force")), None);
    }

    /// Spec: the classifier looks past a leading simple `env NAME=value`
    /// prefix to the program behind it, by full path or bare name, for both
    /// the guard and the badge, and through a second `env` behind the first.
    /// An `env` given an option is not interpreted: the guard counts it as
    /// YOLO when a later word, or a whitespace- or `=`-separated piece of one
    /// (`env -S`), names a program the classifier knows, and as not YOLO
    /// otherwise, and the badge shows nothing for it.
    ///
    /// Why: an env prefix is an ordinary way to set an API key or config
    /// directory, and the supervisor already integrates the program behind it
    /// as that agent. Before this, the first word `env` matched no vendor, so
    /// the sensitive-host guard let `env A=1 claude
    /// --dangerously-skip-permissions` start without asking and the sidebar
    /// showed no badge.
    #[test]
    fn an_env_prefix_does_not_hide_the_program_behind_it() {
        for yolo in [
            "env A=1 claude --dangerously-skip-permissions",
            "/usr/bin/env A=1 codex --yolo",
            "env A=b pi",
            "env A=1 B=2 omp --approval-mode yolo",
            "env -i HOME=/h claude --dangerously-skip-permissions",
            "env -u FOO codex",
            "env -S 'claude --dangerously-skip-permissions'",
            "env '--split-string=/usr/bin/claude --dangerously-skip-permissions'",
            "env A=1 env B=2 claude --dangerously-skip-permissions",
            "env A=1 env -i claude --dangerously-skip-permissions",
        ] {
            assert!(argv_is_yolo(&argv(yolo)), "{yolo}");
        }
        for not_yolo in [
            "env A=1 claude",
            "env A=1 codex --full-auto",
            "env -u FOO ./build.sh",
            "env -i ls -l",
            "env A=1",
            "env",
            "env A=1 echo --yolo",
            "env A=1 env B=2 claude",
            "env -S './build.sh --fast'",
        ] {
            assert!(!argv_is_yolo(&argv(not_yolo)), "{not_yolo}");
        }
        assert_eq!(
            invocation_marker(&argv("env A=1 claude --dangerously-skip-permissions")),
            Some(InvocationMarker::SkipPerms)
        );
        assert_eq!(
            invocation_marker(&argv("/usr/bin/env A=1 codex --yolo")),
            Some(InvocationMarker::Yolo)
        );
        assert_eq!(
            invocation_marker(&argv("env -i claude --dangerously-skip-permissions")),
            None,
            "the badge labels only what it recognizes, not an uninterpreted env"
        );
    }

    /// Spec: Codex's never-ask approval policy together with its
    /// `danger-full-access` sandbox is a YOLO launch in every spelling: short
    /// and long options, two-argument, `=` and attached forms, either order,
    /// and the `-c`/`--config` settings with or without TOML quotes. Either
    /// setting alone is not. The badge shows Codex's no-sandbox marker for the
    /// pair among its leading switches and nothing for either one alone.
    ///
    /// Why: Codex documents this pair as the same mode as
    /// `--dangerously-bypass-approvals-and-sandbox`. Before, it matched no
    /// table, so a host that asks before YOLO launches started it without
    /// asking and the sidebar showed no badge; `-a never` alone keeps the
    /// sandbox and must not start asking.
    #[test]
    fn codex_option_spelled_yolo_needs_both_settings_in_any_spelling() {
        for yolo in [
            "codex -a never -s danger-full-access",
            "codex -s danger-full-access -a never",
            "codex --ask-for-approval never --sandbox danger-full-access",
            "codex --ask-for-approval=never --sandbox=danger-full-access",
            "codex -a=never -s=danger-full-access",
            "codex -m gpt -a never --model x -s danger-full-access",
            "codex -c approval_policy=never -c sandbox_mode=danger-full-access",
            r#"codex -c 'approval_policy="never"' --config='sandbox_mode="danger-full-access"'"#,
            "codex -a never -c sandbox_mode=danger-full-access",
            "codex -anever -sdanger-full-access",
            "env A=1 codex -a never -s danger-full-access",
        ] {
            assert!(argv_is_yolo(&argv(yolo)), "{yolo}");
            assert_eq!(
                invocation_marker(&argv(yolo)),
                Some(InvocationMarker::NoSandbox),
                "{yolo}"
            );
        }
        for not_yolo in [
            "codex -a never",
            "codex --ask-for-approval=never",
            "codex -s danger-full-access",
            "codex --sandbox danger-full-access -a on-request",
            "codex -c approval_policy=never",
            "codex -a never -- -s danger-full-access",
            "claude -a never -s danger-full-access",
        ] {
            assert!(!argv_is_yolo(&argv(not_yolo)), "{not_yolo}");
            assert_eq!(invocation_marker(&argv(not_yolo)), None, "{not_yolo}");
        }
    }

    /// Spec: the shared program locator returns 0 for a command without an
    /// `env` prefix, the first non-assignment word after a simple prefix, and
    /// `None` for an option-bearing `env` or a prefix with no program.
    ///
    /// Why: the supervisor's hook injection and resume building use this same
    /// function, so its contract is what keeps them and the YOLO guard
    /// looking at the same program.
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
