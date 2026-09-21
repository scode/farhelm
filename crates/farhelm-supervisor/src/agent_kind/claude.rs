//! Claude's foreground-ownership proof surface: the reported-transition
//! vocabulary, the launch-program classification the process corridor
//! dispatches on, the foreground CLI grammar the live runtime must still
//! describe, and the transcript-path validation the admission and refresh
//! paths share.
//!
//! The grammar tables live here (not in `service::core`) the way the OMP
//! and Goose halves do, so launch-time classification and admission-time
//! runtime verification read the same rules. `procs` owns the byte-level
//! chain matching (images, argv, trampolines); this module owns everything
//! stated over decoded argv and reported hook fields — the policy half of
//! the proof.
//!
//! All vendor facts below are grounded in the pinned `claude` binary
//! (`2.1.278`, native ELF install): its `--help` (session vs utility vs
//! background shapes), its `SessionStart` hook schema (the exact
//! five-word source enum, `agent_type` without `agent_id`), and live
//! hook-firing probes with an isolated vendor home (startup on `claude
//! -p`, `resume` on `claude --resume <id>` with re-passed `--settings`,
//! a forwarded-settings child emitting its own `startup`, and
//! `agent_type` without `agent_id` on top-level `--agent`). Whatever
//! those probes could not drive (ordinary subagent emission, team/swarm
//! shapes, background daemons, fork/clear/compact transitions) is a
//! documented residual in SPEC_impl, never a claim.

use std::path::Path;

/// Whether a reported hook event names the one event Farhelm subscribes
/// for Claude: the exact string `"SessionStart"`. Checked raw at the
/// doorway (before sanitation, beside the Codex/OMP/Goose checks) and
/// re-checked at admission. Absent, null, non-string, or any other event
/// (`SubagentStart`, `SubagentStop`, anything teams/background invent)
/// refuses.
///
/// The pinned binary's hook schemas carry `SubagentStart`/`SubagentStop`
/// as events whose payloads name a subagent `agent_id`; ordinary native
/// subagents emit those, not the subscribed `SessionStart`. If a future
/// version ever emits a legitimate foreground `SessionStart` without the
/// field shape this proof expects, that version is an unsupported capture
/// shape with a documented limitation, not a silent acceptance.
pub(crate) fn is_claude_foreground_event(event: &str) -> bool {
    event == "SessionStart"
}

/// Whether a reported Claude transition names a foreground conversation
/// move this integration subscribes to: Claude's own five-word source
/// vocabulary — the exact enum the pinned binary's `SessionStart` hook
/// schema carries — not Codex's four-word list. Checked raw at the
/// doorway and re-checked at admission. Empty/missing/anything else
/// refuses.
///
/// `fork` is a legitimate transition source here, never an auto-adoption
/// of a background copy: whether a fork report replaces the binding is
/// decided by process attribution (the fork's runtime must BE the owned
/// foreground), not by this word. A background copy's fork report fails
/// foreground attribution, so the parent binding is untouched.
pub(crate) fn is_claude_foreground_source(source: &str) -> bool {
    matches!(source, "startup" | "resume" | "clear" | "compact" | "fork")
}

/// The launch program behind one Claude invocation, classified from the
/// durable launch argv's effective program. This decides which
/// installation descriptor the corridor must find live — it never
/// accepts anything by itself. Fieldless and `Copy` so the blocking
/// attribution closure can carry it across threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClaudeLaunchProgram {
    /// A native `claude` launch: exactly one Claude runtime image (`C`) —
    /// a native `claude` by basename or the standard install's
    /// versioned single file.
    Claude,
    /// An npm/npx/bun package launch (`L`): classified so the
    /// provenance column names the shape honestly, but with no
    /// package/bin layout proved on the pinned install the corridor
    /// refuses it — unproven layouts never reach evidence.
    Package,
    /// A shell wrapper: a known transparent trampoline (`S`) above
    /// the native image, or an exec'd-away shell leaving no link.
    Shell,
    /// Anything else — direct `node`/`bun`/script execution, unknown
    /// wrappers: fail closed, never reaching the corridor.
    Unknown,
}

/// Classify the durable launch argv's program for corridor dispatch. An
/// `env` prefix is skipped the same way injection skips it; anything
/// without an effective program is unknown.
///
/// There is deliberately NO subcommand grammar here (no Goose-`session`
/// or OMP-utility equivalent is verified for Claude): classification is
/// launcher shape, and the corridor's `C` evidence carries the CLI-argv
/// check. Do NOT copy another kind's classifier to fill the gap.
pub(crate) fn classify_claude_launch(argv: &[String]) -> ClaudeLaunchProgram {
    let Some(program) = super::effective_program_index(argv) else {
        return ClaudeLaunchProgram::Unknown;
    };
    let name = argv
        .get(program)
        .and_then(|element| Path::new(element).file_name())
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    match name {
        "claude" => ClaudeLaunchProgram::Claude,
        "npm" | "npx" | "bun" | "bunx" => ClaudeLaunchProgram::Package,
        "sh" | "bash" | "dash" => ClaudeLaunchProgram::Shell,
        _ => ClaudeLaunchProgram::Unknown,
    }
}

impl ClaudeLaunchProgram {
    /// The durable spelling of one classification, for the launch
    /// provenance column. A new variant must extend both this and
    /// [`ClaudeLaunchProgram::from_column_value`], never reuse a spelling.
    pub(crate) fn column_value(self) -> &'static str {
        match self {
            ClaudeLaunchProgram::Claude => "claude",
            ClaudeLaunchProgram::Package => "package",
            ClaudeLaunchProgram::Shell => "shell",
            ClaudeLaunchProgram::Unknown => "unknown",
        }
    }

    /// The inverse of [`ClaudeLaunchProgram::column_value`], lenient the
    /// way the OMP/Goose halves of provenance are: a missing or
    /// unrecognized value is `Unknown` — fail closed at the corridor —
    /// rather than a row this supervisor can no longer load.
    pub(crate) fn from_column_value(value: Option<&str>) -> Self {
        match value {
            Some("claude") => ClaudeLaunchProgram::Claude,
            Some("package") => ClaudeLaunchProgram::Package,
            Some("shell") => ClaudeLaunchProgram::Shell,
            _ => ClaudeLaunchProgram::Unknown,
        }
    }
}

/// Claude's session-hosting subcommands on the pinned binary: management,
/// background-session, server, and installer verbs that never host the
/// owned foreground conversation. Grounded in `claude --help` on 2.1.278
/// (the `Commands:` section); anything else in command position is prompt
/// text, which is exactly what an unknown word is — `claude frobnicate
/// the wobbler` prompts, it does not dispatch. A future UTILITY refuses
/// harmlessly once listed (utilities emit no `SessionStart`, so the
/// corridor never sees them); a future FOREGROUND verb not on this list
/// would be accepted as prompt-shaped, so the list is pinned to the
/// surveyed version and grows only with evidence (see the residuals in
/// SPEC_impl).
const CLAUDE_UTILITY_COMMANDS: &[&str] = &[
    "agents",
    "attach",
    "auth",
    "auto-mode",
    "doctor",
    "gateway",
    "import",
    "install",
    "logs",
    "mcp",
    "plugin",
    "plugins",
    "project",
    "respawn",
    "rm",
    "setup-token",
    "stop",
    "kill",
    "update",
    "upgrade",
    "ultrareview",
];

/// Option spellings the CLI grammar consumes a value for. Only flags the
/// pinned `--help` documents, with the arity it documents: a required
/// value consumes the next token, an optional one consumes it only when
/// it does not start with `-` (so `claude --resume --json` is the picker
/// form, not a resume of a session named `--json`). Inline
/// `--flag=value` never consumes. Anything unlisted passes through
/// unconsumed — explicitly parsed flags only, never a guessed arity.
fn claude_flag_takes_value(flag: &str, inline_value: bool, next: Option<&str>) -> bool {
    if inline_value {
        return false;
    }
    // Required values: the next token belongs to the flag, whatever it
    // is — a launch this malformed errors out of the vendor before any
    // session exists, so the corridor never sees it either way.
    const REQUIRED: &[&str] = &[
        "--model",
        "--agent",
        "--session-id",
        "--add-dir",
        "--append-system-prompt",
        "--system-prompt",
        "--mcp-config",
        "--output-format",
        "--input-format",
        "--permission-mode",
        "--permission-prompts",
        "--permission-prompt-tool",
        "--betas",
        "--max-budget-usd",
        "--effort",
        "--tools",
        "--allowedTools",
        "--allowed-tools",
        "--disallowedTools",
        "--disallowed-tools",
        "--plugin-dir",
        "--plugin-url",
        "--file",
        "--fallback-model",
        "--json-schema",
        "--name",
        "-n",
        "--settings",
        "--setting-sources",
        "--autocompact",
        "--system-prompt-snapshot",
        "--remote-control-session-name-prefix",
        "--ide",
    ];
    if REQUIRED.contains(&flag) {
        return next.is_some();
    }
    // Optional values: consumed only when present and not flag-shaped.
    const OPTIONAL: &[&str] = &[
        "-r",
        "--resume",
        "-c",
        "--continue",
        "--teleport",
        "--cloud",
        "--from-pr",
        "--remote-control",
        "-w",
        "--worktree",
        "--tmux",
        "-d",
        "--debug",
        "--prompt-suggestions",
    ];
    if OPTIONAL.contains(&flag) {
        return next.is_some_and(|value| !value.starts_with('-'));
    }
    false
}

/// Whether one CLI token is an excluded launch shape: background sessions
/// (a daemon outside the owned pane must never prove foreground),
/// `--bare` (which skips hooks — a report chained through a hookless
/// runtime is necessarily foreign), and the exiting introspection flags.
/// Exact elements only: `--verbose` is not `-v`, and a prompt word that
/// merely contains these bytes is one argv element the vendor parses as
/// prompt text, not a flag it acts on.
fn claude_excluded_flag(token: &str) -> bool {
    matches!(
        token,
        "--bg" | "--background" | "--bare" | "-h" | "--help" | "-v" | "--version" | "--"
    )
}

/// Whether a decoded `claude` argv still describes a foreground session —
/// the `C` evidence beside the native image.
///
/// The full launch argv including the program (the caller decodes UTF-8;
/// undecodable argv never reaches here): the effective program must name
/// `claude`, no utility subcommand may sit in command position, and no
/// excluded flag may appear anywhere. Print mode (`-p`/`--print`) and
/// top-level `--agent` are foreground shapes, not refusals — the probe
/// watched both emit attributed `SessionStart` — as are resume/continue,
/// fork-session, session-id selection, and every other flag the grammar
/// does not explicitly exclude or consume. A bare `--` refuses: injection
/// skips such launches, so no legitimate reporter descends from one.
pub(crate) fn claude_foreground_cli_shape(argv: &[String]) -> bool {
    let Some(program) = super::effective_program_index(argv) else {
        return false;
    };
    if Path::new(&argv[program])
        .file_name()
        .and_then(|name| name.to_str())
        != Some("claude")
    {
        return false;
    }
    let mut command_seen = false;
    let mut index = program + 1;
    while index < argv.len() {
        let token = argv[index].as_str();
        if claude_excluded_flag(token) {
            return false;
        }
        if let Some((flag, _)) = token.split_once('=') {
            // An inline value belongs to its token; only the flag half
            // is examined, and only for exclusion (an inline unknown
            // flag cannot be eating the command position).
            if flag.starts_with('-') && claude_excluded_flag(flag) {
                return false;
            }
            index += 1;
            continue;
        }
        if token.starts_with('-') && token.len() > 1 {
            if claude_flag_takes_value(token, false, argv.get(index + 1).map(String::as_str)) {
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        // The command position: the first token the vendor could
        // dispatch as a subcommand. A utility verb refuses; anything
        // else is prompt text, and later bare words are prompt words —
        // only the FIRST bare token is ever the command.
        if !command_seen {
            command_seen = true;
            if CLAUDE_UTILITY_COMMANDS.contains(&token) {
                return false;
            }
        }
        index += 1;
    }
    true
}

/// Extract the transcript locator from a typed attributed event's
/// `transcript_path`: a present string, absolute, UTF-8, and bounded by
/// [`MAX_SESSION_PATH_BYTES`](super::MAX_SESSION_PATH_BYTES) — enforced
/// before the write — and never derived from cwd/time search. Anything
/// else (absent, null, non-string, relative, oversized) refuses: Claude
/// has no fileless report shape, so a report without a locator is not a
/// shape this proof knows.
pub(crate) fn transcript_path_hint(value: &serde_json::Value) -> Option<&str> {
    let path = value.as_str()?;
    if !path.starts_with('/') || path.len() > super::MAX_SESSION_PATH_BYTES {
        return None;
    }
    Some(path)
}

/// Whether one bounded transcript prefix verifies the reported
/// conversation: the FIRST line carrying a string `sessionId` must name
/// exactly the reported id.
///
/// First-carrying, not any-carrying: a transcript is one session's
/// record, and a file whose opening session line names someone else is
/// someone else's transcript however many matching lines follow. Lines
/// that are not JSON objects, or carry no string `sessionId`, are
/// skipped the way the old correlation parser skipped field-less
/// lines — that is the ordinary shape (queue operations, summaries),
/// not corruption. Only the audited [`RECORD_PREFIX_LINES`](super::RECORD_PREFIX_LINES)
/// leading lines are examined, the same bound the removed correlation
/// parser enforced: a real transcript names its session in its first
/// lines, and anything deeper is not this check's to bless. No
/// `sessionId` line at all, or a first one naming another id, fails
/// closed: at admission the report refuses, at refresh/pre-resume the
/// offer is suppressed or withdrawn.
pub(crate) fn transcript_session_matches(prefix: &str, expected: &str) -> bool {
    for line in prefix.lines().take(super::RECORD_PREFIX_LINES) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(session) = value
            .as_object()
            .and_then(|object| object.get("sessionId"))
            .and_then(|id| id.as_str())
        else {
            continue;
        };
        return session == expected;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The doorway and admission share one event: the exact string the
    /// vendor's hook schema subscribes. Anything else — including the
    /// subagent lifecycle the same binary emits — refuses in both
    /// places.
    #[farhelm_testtrace::test]
    fn the_claude_event_vocabulary_is_session_start_only() {
        assert!(is_claude_foreground_event("SessionStart"));
        assert!(!is_claude_foreground_event(""));
        assert!(!is_claude_foreground_event("session_start"));
        assert!(!is_claude_foreground_event("Sessionstart"));
        assert!(!is_claude_foreground_event("SubagentStart"));
        assert!(!is_claude_foreground_event("SubagentStop"));
        assert!(!is_claude_foreground_event("SessionStart "));
    }

    /// The doorway and admission share one vocabulary: Claude's own
    /// five-word source enum from the pinned `SessionStart` schema —
    /// never Codex's four-word list, which would reject legitimate
    /// `fork` transitions.
    #[farhelm_testtrace::test]
    fn the_claude_source_vocabulary_is_the_five_subscribed_words() {
        for word in ["startup", "resume", "clear", "compact", "fork"] {
            assert!(is_claude_foreground_source(word), "{word} must be accepted");
        }
        for word in [
            "",
            "Startup",
            "STARTUP",
            "startup ",
            " session_start",
            "session_start",
            "goose",
            "subagent",
            "branch",
        ] {
            assert!(!is_claude_foreground_source(word), "{word} must be refused");
        }
    }

    /// The launch classification dispatches on the effective program: an
    /// `env` prefix is skipped, package managers classify as package,
    /// shells as wrappers, and anything else — including every direct
    /// interpreter spelling — fails closed rather than inheriting
    /// another shape's rules.
    #[farhelm_testtrace::test]
    fn the_launch_classification_follows_the_effective_program() {
        let program = |argv: &[&str]| {
            classify_claude_launch(
                &argv
                    .iter()
                    .map(|word| (*word).to_string())
                    .collect::<Vec<_>>(),
            )
        };
        assert!(matches!(program(&["claude"]), ClaudeLaunchProgram::Claude));
        assert!(matches!(
            program(&["/usr/local/bin/claude", "-p", "hi"]),
            ClaudeLaunchProgram::Claude
        ));
        assert!(matches!(
            program(&["env", "FOO=1", "claude"]),
            ClaudeLaunchProgram::Claude
        ));
        for spelling in ["npm", "npx", "bun", "bunx"] {
            assert!(
                matches!(program(&[spelling]), ClaudeLaunchProgram::Package),
                "{spelling} is a package launcher"
            );
        }
        assert!(matches!(
            program(&["sh", "-c", "exec claude"]),
            ClaudeLaunchProgram::Shell
        ));
        assert!(matches!(
            program(&["bash", "-c", "exec claude"]),
            ClaudeLaunchProgram::Shell
        ));
        assert!(matches!(program(&[]), ClaudeLaunchProgram::Unknown));
        assert!(matches!(
            program(&["my-wrapper"]),
            ClaudeLaunchProgram::Unknown
        ));
        // Direct interpreter execution has no verified Claude mapping —
        // and a bare `bun`/`npx` classifies as a package launcher (the
        // corridor refuses those too, for lack of a proved layout), so
        // neither shape can prove.
        for spelling in ["node", "nodejs", "deno"] {
            assert!(
                matches!(program(&[spelling]), ClaudeLaunchProgram::Unknown),
                "{spelling} has no verified Claude mapping"
            );
        }
    }

    /// The provenance spellings round-trip, and anything unrecognized —
    /// including a missing value — decodes to `Unknown` rather than
    /// breaking the row: a corrupt program fails closed at the
    /// corridor, never as a load error.
    #[farhelm_testtrace::test]
    fn the_launch_program_spellings_round_trip_leniently() {
        for program in [
            ClaudeLaunchProgram::Claude,
            ClaudeLaunchProgram::Package,
            ClaudeLaunchProgram::Shell,
            ClaudeLaunchProgram::Unknown,
        ] {
            assert_eq!(
                ClaudeLaunchProgram::from_column_value(Some(program.column_value())),
                program,
                "the {program:?} spelling must survive the column"
            );
        }
        assert_eq!(
            ClaudeLaunchProgram::from_column_value(None),
            ClaudeLaunchProgram::Unknown
        );
        assert_eq!(
            ClaudeLaunchProgram::from_column_value(Some("goose")),
            ClaudeLaunchProgram::Unknown
        );
        assert_eq!(
            ClaudeLaunchProgram::from_column_value(Some("")),
            ClaudeLaunchProgram::Unknown
        );
    }

    /// The CLI grammar admits the foreground shapes the probe watched
    /// emit — bare sessions, print mode, top-level `--agent`, resume and
    /// continue — while refusing background, hookless, exiting, and
    /// utility shapes. Only explicitly parsed flags consume or refuse;
    /// prompt words never do.
    #[farhelm_testtrace::test]
    fn the_cli_grammar_admits_foreground_shapes_only() {
        let shape = |argv: &[&str]| {
            claude_foreground_cli_shape(
                &argv
                    .iter()
                    .map(|word| (*word).to_string())
                    .collect::<Vec<_>>(),
            )
        };
        // Foreground: bare, print, agent, resume/continue, session-id,
        // fork-session, and ordinary flags with values.
        assert!(shape(&["claude"]));
        assert!(shape(&["/usr/local/bin/claude"]));
        assert!(shape(&["env", "FOO=1", "claude", "-p", "hi"]));
        assert!(shape(&["claude", "-p", "hi"]));
        assert!(shape(&["claude", "--print", "--output-format", "json"]));
        assert!(shape(&["claude", "--agent", "general-purpose", "-p", "hi"]));
        assert!(shape(&["claude", "--resume", "some-id"]));
        assert!(shape(&["claude", "-r"]));
        assert!(shape(&["claude", "--continue"]));
        assert!(shape(&["claude", "--session-id", "some-id"]));
        assert!(shape(&["claude", "--fork-session", "--resume", "some-id"]));
        assert!(shape(&["claude", "--model", "opus", "do the thing"]));
        assert!(shape(&["claude", "--model=opus", "do the thing"]));
        assert!(shape(&["claude", "do the thing"]));
        assert!(shape(&["claude", "--mcp-config", "cfg.json"]));
        // A value that looks like a utility is consumed, not dispatched.
        assert!(shape(&["claude", "--model", "agents"]));
        assert!(shape(&["claude", "-r", "attach"]));
        // Background, hookless, exiting, and option-terminated shapes.
        assert!(!shape(&["claude", "--bg"]));
        assert!(!shape(&["claude", "--background"]));
        assert!(!shape(&["claude", "prompt", "--bg"]));
        assert!(!shape(&["claude", "--bare", "-p", "hi"]));
        assert!(!shape(&["claude", "--help"]));
        assert!(!shape(&["claude", "-h"]));
        assert!(!shape(&["claude", "--version"]));
        assert!(!shape(&["claude", "-v"]));
        assert!(!shape(&["claude", "--", "prompt"]));
        assert!(!shape(&["claude", "--background=true"]));
        // Utility subcommands refuse, wherever the vendor would
        // dispatch them; unknown words are prompt text, not commands.
        assert!(!shape(&["claude", "agents", "--json"]));
        assert!(!shape(&["claude", "attach", "abc123"]));
        assert!(!shape(&["claude", "mcp", "serve"]));
        assert!(!shape(&["claude", "--model", "opus", "logs"]));
        assert!(shape(&["claude", "frobnicate", "the", "wobbler"]));
        // No program, or a program that is not claude.
        assert!(!shape(&[]));
        assert!(!shape(&["node", "cli.js"]));
    }

    /// The locator gate admits a present absolute bounded string and
    /// nothing else: Claude has no fileless report shape, and a locator
    /// that is not absolute or not bounded cannot be persisted or
    /// opened safely.
    #[farhelm_testtrace::test]
    fn the_transcript_locator_gate_needs_a_bounded_absolute_string() {
        use serde_json::json;
        assert_eq!(
            transcript_path_hint(&json!("/home/u/.claude/projects/x/y.jsonl")),
            Some("/home/u/.claude/projects/x/y.jsonl")
        );
        assert_eq!(transcript_path_hint(&json!(null)), None);
        assert_eq!(transcript_path_hint(&json!(7)), None);
        assert_eq!(transcript_path_hint(&json!("relative/path.jsonl")), None);
        assert_eq!(transcript_path_hint(&json!("")), None);
        let long = format!("/{}", "p".repeat(super::super::MAX_SESSION_PATH_BYTES));
        assert_eq!(transcript_path_hint(&json!(long)), None);
    }

    /// The transcript check reads the first session line, not any line:
    /// a file opening on another session refuses even when a later line
    /// matches, and field-less leading lines are skipped rather than
    /// failing the file. Only the audited leading-line bound is
    /// examined: a session line at the last examined line still counts,
    /// one line deeper does not.
    #[farhelm_testtrace::test]
    fn the_transcript_check_reads_the_first_session_line() {
        let expected = "11111111-2222-4333-8444-555555555555";
        let matching = format!(
            "{{\"sessionId\":\"{expected}\",\"cwd\":\"/w\",\"timestamp\":\"2026-09-21T00:00:00Z\"}}"
        );
        assert!(transcript_session_matches(&matching, expected));
        // Field-less leading lines are skipped to the first session line.
        let padded = format!("{{\"type\":\"summary\"}}\nnot json\n{matching}\n");
        assert!(transcript_session_matches(&padded, expected));
        // A first session line naming another session refuses, however
        // many matching lines follow.
        let other = format!("{{\"sessionId\":\"other\"}}\n{matching}\n");
        assert!(!transcript_session_matches(&other, expected));
        assert!(!transcript_session_matches("", expected));
        assert!(!transcript_session_matches("{}\n[]\n", expected));
        // The line bound is exact: the last examined line still counts.
        let bound = super::super::RECORD_PREFIX_LINES;
        let inside = "{\"type\":\"filler\"}\n".repeat(bound - 1) + &matching;
        assert!(transcript_session_matches(&inside, expected));
        let outside = "{\"type\":\"filler\"}\n".repeat(bound) + &matching;
        assert!(
            !transcript_session_matches(&outside, expected),
            "a session line past the examined bound verifies nothing"
        );
    }
}
