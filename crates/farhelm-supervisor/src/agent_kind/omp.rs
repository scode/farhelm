//! OMP's foreground-ownership proof surface: the reported-transition
//! vocabulary, the interactive-shape grammar shared by injection and live
//! runtime verification, and the launch-program classification the process
//! corridor dispatches on.
//!
//! The grammar tables and the injection decision moved here from
//! `service::core` unchanged when the proof landed, so the launch-time
//! classifier and the admission-time runtime check read the same rules
//! rather than two copies that can drift. `procs` owns the byte-level
//! chain matching (images, entry points, launcher argv); this module owns
//! everything stated over decoded launch argv.

use std::path::Path;

/// Whether a reported OMP transition names a foreground conversation move
/// this integration subscribes to: the asset's four event tags, with
/// `session_switch` carrying its upstream reason after a colon. The reason
/// is opaque passthrough — upstream-defined and version-varying — and the
/// proofs never depend on it. A bare `session_switch` without a reason is
/// not something the asset sends and is refused, like any unknown tag.
pub(crate) fn is_omp_foreground_source(source: &str) -> bool {
    source == "session_start"
        || source == "session_branch"
        || source == "agent_end"
        || source.starts_with("session_switch:")
}

/// The launch program behind one OMP invocation, classified from the
/// durable launch argv's effective program. This decides which installation
/// descriptor the corridor must find live — it never accepts anything by
/// itself. Fieldless and `Copy` so the blocking attribution closure can
/// carry it across threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OmpLaunchProgram {
    /// The installed `omp` command: a Bun-executed bundle (`Oj`) or a
    /// compiled target (`Ob`).
    Omp,
    /// A Bun package launch (`bun x` / `bunx`): launcher above the runtime.
    Bun,
    /// An npm/npx launch: launcher above the runtime, Bun-resulting only.
    Npm,
    /// A directly Node-executed entry point: never a supported runtime.
    Node,
    /// A shell wrapper: a known transparent trampoline or nothing live.
    Shell,
    /// Anything else: fail closed.
    Unknown,
}

/// Classify the durable launch argv's program for corridor dispatch. An
/// `env` prefix is skipped the same way injection skips it; anything
/// without an effective program is unknown.
pub(crate) fn classify_omp_launch(argv: &[String]) -> OmpLaunchProgram {
    let Some(program) = super::effective_program_index(argv) else {
        return OmpLaunchProgram::Unknown;
    };
    let name = argv
        .get(program)
        .and_then(|element| Path::new(element).file_name())
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    match name {
        "omp" => OmpLaunchProgram::Omp,
        "bun" | "bunx" => OmpLaunchProgram::Bun,
        "npm" | "npx" => OmpLaunchProgram::Npm,
        "node" | "nodejs" => OmpLaunchProgram::Node,
        "sh" | "bash" | "dash" => OmpLaunchProgram::Shell,
        _ => OmpLaunchProgram::Unknown,
    }
}

impl OmpLaunchProgram {
    /// The durable spelling of one classification, for the launch
    /// provenance column. A new variant must extend both this and
    /// [`OmpLaunchProgram::from_column_value`], never reuse a spelling.
    pub(crate) fn column_value(self) -> &'static str {
        match self {
            OmpLaunchProgram::Omp => "omp",
            OmpLaunchProgram::Bun => "bun",
            OmpLaunchProgram::Npm => "npm",
            OmpLaunchProgram::Node => "node",
            OmpLaunchProgram::Shell => "shell",
            OmpLaunchProgram::Unknown => "unknown",
        }
    }

    /// The inverse of [`OmpLaunchProgram::column_value`], lenient the
    /// way the asset half of provenance is: a missing or unrecognized
    /// value is `Unknown` — fail closed at the corridor — rather than
    /// a row this supervisor can no longer load.
    pub(crate) fn from_column_value(value: Option<&str>) -> Self {
        match value {
            Some("omp") => OmpLaunchProgram::Omp,
            Some("bun") => OmpLaunchProgram::Bun,
            Some("npm") => OmpLaunchProgram::Npm,
            Some("node") => OmpLaunchProgram::Node,
            Some("shell") => OmpLaunchProgram::Shell,
            _ => OmpLaunchProgram::Unknown,
        }
    }
}

/// OMP's registered top-level commands and aliases EXCEPT `launch` — the
/// command table (`OMP/cli-commands.ts`) whose dispatch makes the process a
/// utility rather than an interactive conversation. `launch` is the one
/// command token that IS an ordinary launch, and any unregistered word is a
/// prompt, so both stay out of this list.
pub(crate) const OMP_UTILITY_COMMANDS: &[&str] = &[
    "acp",
    "auth-broker",
    "auth-gateway",
    "agents",
    "bench",
    "browser-relay",
    "cleanse",
    "collab",
    "commit",
    "completions",
    "__complete",
    "compress",
    "config",
    "dry-balance",
    "gc",
    "grep",
    "gallery",
    "git",
    "grievances",
    "images",
    "img",
    "if-bench",
    "install",
    "join",
    "models",
    "plugin",
    "plugins",
    "ps",
    "say",
    "share",
    "setup",
    "shell",
    "read",
    "render",
    "ssh",
    "stats",
    "update",
    "usage",
    "tiny-models",
    "token",
    "ttsr",
    "worktree",
    "wt",
    "search",
    "q",
];

/// Options whose OCCURRENCE anywhere in the argv makes the launch
/// non-interactive or exiting: print and explicit modes, export, the alias
/// installer, help, version, license, and the obsolete list-models. Inline
/// `--flag=value` spellings count.
pub(crate) const OMP_EXCLUDED_OPTIONS: &[&str] = &[
    "-p",
    "--print",
    "--export",
    "--alias",
    "--mode",
    "-h",
    "--help",
    "-v",
    "--version",
    "--license",
    "--list-models",
];

/// OMP's reserved plugin/marketplace words, rejected ONLY in the forms
/// `reservedTopLevelWordMessage` rejects: a bare verb, a `marketplace`
/// sub-action, or any later argument carrying a `name@marketplace` plugin id.
/// A prompt such as `omp list all my files` still launches, so the word
/// alone decides nothing.
pub(crate) const OMP_RESERVED_WORDS: &[&str] = &[
    "extensions",
    "list",
    "remove",
    "uninstall",
    "marketplace",
    "discover",
    "upgrade",
    "enable",
    "disable",
];

/// The sub-actions that make `omp marketplace <sub>` unambiguously a
/// management command (`OMP/cli-commands.ts` MARKETPLACE_SUBCOMMANDS).
pub(crate) const OMP_MARKETPLACE_SUBCOMMANDS: &[&str] = &["add", "remove", "rm", "update", "list"];

/// Whether a reserved word in OMP's command position is one of the REJECTING
/// forms OMP itself turns into an error message (and exits) rather than a
/// launch — mirroring `reservedTopLevelWordMessage`. Only the FIRST CLI token
/// is ever this word: OMP's guard runs on argv[0] before flag-hoisted
/// subcommand resolution, so `omp --model x list` is a prompt "list", not a
/// rejected management command.
pub(crate) fn omp_reserved_word_rejects(command: &str, rest: &[String]) -> bool {
    if !OMP_RESERVED_WORDS.contains(&command) {
        return false;
    }
    let Some(second) = rest.first() else {
        return true;
    };
    if command == "marketplace" && OMP_MARKETPLACE_SUBCOMMANDS.contains(&second.as_str()) {
        return true;
    }
    rest.iter()
        .any(|argument| !argument.starts_with('-') && argument.contains('@'))
}

/// What injection should do with one OMP invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OmpInjection {
    /// Leave the invocation untouched, runnable as the user wrote it. The
    /// reason is the skip-log line, and it is the only evidence for why
    /// identity capture did not ride along.
    Leave(&'static str),
    /// Inject the reporter. `pointer` says the instructions pointer may also
    /// ride: false when the user's own argv already carries an
    /// `--append-system-prompt` occurrence, whose value OMP's last-wins
    /// assignment would otherwise silently replace with Farhelm's pointer.
    Inject { pointer: bool },
}

/// Classify one OMP invocation for reporter injection.
///
/// The scan identifies the ONE command position and then walks the ENTIRE
/// option stream, because OMP keeps parsing options after its first
/// positional: `omp launch --print`, a prompt followed by `--mode rpc`, and
/// `omp @input.txt --export saved.jsonl` are all excluded shapes, and a
/// trailing string option left without its value would swallow an injected
/// flag as its own. Fail-closed choices, and why:
///
/// - a GENUINE end-of-options delimiter (an unconsumed `--`, per the shared
///   grammar) declines: everything after it is prompt text, so an appended
///   hook flag has no defined meaning — while a `--` consumed as an option
///   value is just a value, and the walk never reinterprets one;
/// - `--trusted-extension` declines the WHOLE injection: OMP refuses to
///   combine it with `-e`/`--extension`/`--hook`, so appending Farhelm's
///   extension would turn a valid launch into an error before the session
///   starts. Never inferred from an opaque value;
/// - an excluded option occurrence (`--print`, `--mode`, help, version,
///   license, export, alias, inline forms included) declines, because each
///   either exits before an interactive session exists or selects a
///   non-interactive output mode;
/// - an unknown long flag followed by a value-like token declines ONLY
///   before the command position is settled: the successor might be the
///   command (`omp --extflag acp` with a boolean extension flag runs the
///   `acp` command). Once a prompt or `launch` is established, later words
///   are not subcommands and the pair is consumed opaquely;
/// - a string flag left without its successor declines: the injected `-e`
///   would become that option's value.
///
/// Values of KNOWN flags stay opaque: `omp --system-prompt --help` is a
/// prompt that says `--help`, not a help request, exactly as OMP's own
/// parser reads it.
pub(crate) fn omp_injection_decision(argv: &[String]) -> OmpInjection {
    let Some(program) = super::effective_program_index(argv) else {
        return OmpInjection::Leave("invocation has no effective program to classify");
    };
    if Path::new(&argv[program])
        .file_name()
        .and_then(|name| name.to_str())
        != Some("omp")
    {
        return OmpInjection::Leave("invocation is not an omp launch");
    }
    omp_tui_args_decision(&argv[program + 1..])
}

/// The args-walking half of [`omp_injection_decision`]: whether OMP-level
/// arguments (everything after the program) describe an interactive TUI
/// conversation. Injection runs it over the launch argv; admission runs the
/// SAME function over the live runtime's argv (interpreter and entry point
/// already stripped by the corridor), so a process that exec'd from an
/// interactive launch into a utility or print shape refuses even though its
/// launch passed. The injected `-e` tail and a resume `--resume <file>` are
/// ordinary known-flag occurrences and pass; nothing here knows about the
/// program position, so callers must not feed it one.
pub(crate) fn omp_tui_args_decision(args: &[String]) -> OmpInjection {
    let mut command_classified = false;
    let mut user_appends_prompt = false;
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        let Some(flag) = super::omp_flag_occurrence(argument) else {
            if argument == "--" {
                // A `--` the walk reached is, by construction, unconsumed:
                // every value a flag claimed was skipped past wholesale.
                return OmpInjection::Leave(
                    "invocation contains an end-of-options boundary; appended hook flags \
                     would be prompt text",
                );
            }
            if !command_classified {
                // The command position: the first token OMP could dispatch
                // as a subcommand, or the start of the prompt. `help`,
                // worker selectors, and the reserved words are rejected
                // only from the literal first CLI position, mirroring where
                // OMP checks them. Later positionals are prompt words.
                if index == 0 && argument.starts_with('@') {
                    // `@file` tokens are prompts, never subcommands.
                } else if index == 0
                    && (argument == "help" || argument.starts_with("__omp_worker_"))
                {
                    return OmpInjection::Leave("invocation is a help or worker-selector form");
                } else if OMP_UTILITY_COMMANDS.contains(&argument) {
                    return OmpInjection::Leave("invocation runs a utility command");
                } else if index == 0 && omp_reserved_word_rejects(argument, &args[index + 1..]) {
                    return OmpInjection::Leave(
                        "invocation is a reserved management form OMP itself rejects",
                    );
                }
                command_classified = true;
            }
            index += 1;
            continue;
        };
        // Option occurrences are checked ANYWHERE in the stream, before
        // their values are consumed.
        if OMP_EXCLUDED_OPTIONS.contains(&flag.name) {
            return OmpInjection::Leave("invocation carries a non-interactive or exiting option");
        }
        if flag.name == "--trusted-extension" {
            return OmpInjection::Leave(
                "invocation declares --trusted-extension, which OMP refuses to combine with \
                 an injected -e",
            );
        }
        if flag.name == "--append-system-prompt" {
            user_appends_prompt = true;
        }
        let next = args.get(index + 1).map(String::as_str);
        // Both successor-based refusals apply ONLY to non-inline
        // occurrences: an inline `=value` belongs to the token itself, so
        // `--model=<model>` as the FINAL token needs no successor, and an
        // inline unknown option cannot be eating the command-position word.
        if flag.arity == super::OmpFlagArity::UnknownLong
            && !flag.inline_value
            && !command_classified
            && next.is_some_and(|value| !value.starts_with('-'))
        {
            // Before the command position is settled, an unknown long flag
            // might be an extension string flag eating the very token that
            // would have named the command — or a boolean flag leaving it in
            // command position. No guess.
            return OmpInjection::Leave(
                "an unknown long flag makes the command position ambiguous",
            );
        }
        if flag.arity == super::OmpFlagArity::String && !flag.inline_value && next.is_none() {
            return OmpInjection::Leave(
                "invocation ends in a string option with no value; an injected flag would \
                 become its value",
            );
        }
        if super::omp_flag_consumes_next(&flag, next) {
            index += 2;
        } else {
            index += 1;
        }
    }
    OmpInjection::Inject {
        pointer: !user_appends_prompt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The doorway and admission share one vocabulary: the asset's four
    /// event tags, with `session_switch` carrying its opaque upstream
    /// reason. Anything else — including a reasonless `session_switch`
    /// the asset never sends — refuses in both places.
    #[farhelm_testtrace::test]
    fn the_omp_transition_vocabulary_is_the_four_subscribed_tags() {
        assert!(is_omp_foreground_source("session_start"));
        assert!(is_omp_foreground_source("session_branch"));
        assert!(is_omp_foreground_source("agent_end"));
        assert!(is_omp_foreground_source("session_switch:new"));
        assert!(is_omp_foreground_source("session_switch:resume"));
        assert!(is_omp_foreground_source("session_switch:"));
        assert!(!is_omp_foreground_source("session_switch"));
        assert!(!is_omp_foreground_source("session_end"));
        assert!(!is_omp_foreground_source("message"));
        assert!(!is_omp_foreground_source(""));
    }

    /// The launch classification dispatches on the effective program: an
    /// `env` prefix is skipped, and anything without a classifiable
    /// program fails closed rather than inheriting another shape's rules.
    #[farhelm_testtrace::test]
    fn the_launch_classification_follows_the_effective_program() {
        let program = |argv: &[&str]| {
            classify_omp_launch(
                &argv
                    .iter()
                    .map(|word| (*word).to_string())
                    .collect::<Vec<_>>(),
            )
        };
        assert!(matches!(program(&["omp"]), OmpLaunchProgram::Omp));
        assert!(matches!(
            program(&["/usr/local/bin/omp", "--resume", "f"]),
            OmpLaunchProgram::Omp
        ));
        assert!(matches!(
            program(&["env", "FARHELM_X=1", "omp"]),
            OmpLaunchProgram::Omp
        ));
        assert!(matches!(
            program(&["bun", "x", "@oh-my-pi/pi-coding-agent"]),
            OmpLaunchProgram::Bun
        ));
        assert!(matches!(
            program(&["bunx", "@oh-my-pi/pi-coding-agent"]),
            OmpLaunchProgram::Bun
        ));
        assert!(matches!(
            program(&["npm", "exec", "--package=X"]),
            OmpLaunchProgram::Npm
        ));
        assert!(matches!(
            program(&["node", "dist/cli.js"]),
            OmpLaunchProgram::Node
        ));
        assert!(matches!(
            program(&["sh", "-c", "exec omp"]),
            OmpLaunchProgram::Shell
        ));
        assert!(matches!(program(&[]), OmpLaunchProgram::Unknown));
        assert!(matches!(
            program(&["my-wrapper"]),
            OmpLaunchProgram::Unknown
        ));
    }

    /// The provenance spellings round-trip, and anything unrecognized —
    /// including a missing value — decodes to `Unknown` rather than
    /// breaking the row: a corrupt program fails closed at the
    /// corridor, never as a load error.
    #[farhelm_testtrace::test]
    fn the_launch_program_spellings_round_trip_leniently() {
        for program in [
            OmpLaunchProgram::Omp,
            OmpLaunchProgram::Bun,
            OmpLaunchProgram::Npm,
            OmpLaunchProgram::Node,
            OmpLaunchProgram::Shell,
            OmpLaunchProgram::Unknown,
        ] {
            assert_eq!(
                OmpLaunchProgram::from_column_value(Some(program.column_value())),
                program,
                "the {program:?} spelling must survive the column"
            );
        }
        assert_eq!(
            OmpLaunchProgram::from_column_value(None),
            OmpLaunchProgram::Unknown
        );
        assert_eq!(
            OmpLaunchProgram::from_column_value(Some("bunx")),
            OmpLaunchProgram::Unknown
        );
        assert_eq!(
            OmpLaunchProgram::from_column_value(Some("")),
            OmpLaunchProgram::Unknown
        );
    }
}
