//! Goose's foreground-ownership proof surface: the reported-transition
//! vocabulary, the launch-program classification the process corridor
//! dispatches on, the session grammar shared by injection and live
//! runtime verification, the exact store path resolver, and the
//! foreground session-type allowlist.
//!
//! The grammar moved here from `service::core` unchanged when the proof
//! landed, so the launch-time classifier and the admission-time runtime
//! check read the same rules rather than two copies that can drift.
//! `procs` owns the byte-level chain matching (images, argv, the MCP
//! trampoline); `goose_store` owns the read-only SQLite mechanics; this
//! module owns everything stated over decoded launch argv, the runtime
//! environment, and reported metadata — the policy half of the proof.

use std::path::Path;

/// Whether a reported Goose transition names the one source the helper
/// sends: the literal `"goose"` from `internal goose-hook`'s payload.
/// Checked raw at the doorway (before sanitation, beside the Codex/OMP
/// checks) and re-checked at admission. Unknown sources refuse; there
/// is no Goose transition vocabulary beyond this single string.
///
/// `transcript_path` and `hook_event_name` are IGNORED for Goose —
/// never consulted, never a rejection signal — because the helper
/// sends neither. `agent_id` needs no Goose code: the doorway already
/// rejects a non-empty one for every kind.
pub(crate) fn is_goose_foreground_source(source: &str) -> bool {
    source == "goose"
}

/// The launch program behind one Goose invocation, classified from the
/// durable launch argv's effective program. This decides which
/// installation descriptor the corridor must find live — it never
/// accepts anything by itself. Fieldless and `Copy` so the blocking
/// attribution closure can carry it across threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GooseLaunchProgram {
    /// A native `goose` launch: exactly one native image (`G`).
    Goose,
    /// A shell wrapper: a known transparent trampoline (`S`) above
    /// the native image, or an exec'd-away shell leaving no link.
    Shell,
    /// Anything else — npm/npx/bun/node, direct scripts, unknown
    /// wrappers: fail closed, never reaching the corridor.
    Unknown,
}

/// Classify the durable launch argv's program for corridor dispatch. An
/// `env` prefix is skipped the same way injection skips it; anything
/// without an effective program is unknown.
pub(crate) fn classify_goose_launch(argv: &[String]) -> GooseLaunchProgram {
    let Some(program) = super::effective_program_index(argv) else {
        return GooseLaunchProgram::Unknown;
    };
    let name = argv
        .get(program)
        .and_then(|element| Path::new(element).file_name())
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    match name {
        "goose" => GooseLaunchProgram::Goose,
        "sh" | "bash" | "dash" => GooseLaunchProgram::Shell,
        _ => GooseLaunchProgram::Unknown,
    }
}

impl GooseLaunchProgram {
    /// The durable spelling of one classification, for the launch
    /// provenance column. A new variant must extend both this and
    /// [`GooseLaunchProgram::from_column_value`], never reuse a spelling.
    pub(crate) fn column_value(self) -> &'static str {
        match self {
            GooseLaunchProgram::Goose => "goose",
            GooseLaunchProgram::Shell => "shell",
            GooseLaunchProgram::Unknown => "unknown",
        }
    }

    /// The inverse of [`GooseLaunchProgram::column_value`], lenient the
    /// way the OMP half of provenance is: a missing or unrecognized
    /// value is `Unknown` — fail closed at the corridor — rather than
    /// a row this supervisor can no longer load.
    pub(crate) fn from_column_value(value: Option<&str>) -> Self {
        match value {
            Some("goose") => GooseLaunchProgram::Goose,
            Some("shell") => GooseLaunchProgram::Shell,
            _ => GooseLaunchProgram::Unknown,
        }
    }
}

/// Facts needed to inject once on fresh Goose launches and reuse its saved
/// reporter on resume without overriding a user-supplied reporter of that name.
#[derive(Clone, Copy)]
pub(crate) struct GooseLaunchShape {
    pub(crate) resuming: bool,
    pub(crate) reporter_collision: bool,
    pub(crate) needs_session_subcommand: bool,
}

/// Recognize only Goose's interactive command, including the structured
/// launcher's leading `env NAME=value ...` prefix.
///
/// Moved here from `service::core` unchanged: injection reads the full
/// shape (fresh/resume/collision/subcommand), while admission uses
/// "recognized" only — the live argv decodes to UTF-8 or refuses, then
/// this returning `Some` is the whole runtime-grammar leg.
pub(crate) fn goose_launch_shape(argv: &[String]) -> Option<GooseLaunchShape> {
    let program = super::effective_program_index(argv)?;
    if Path::new(&argv[program]).file_name()?.to_str()? != "goose" {
        return None;
    }
    let args = &argv[program + 1..];
    if !args.is_empty() && args.first().map(String::as_str) != Some("session") {
        return None;
    }
    let mut resuming = false;
    let mut reporter_collision = false;
    let mut index = usize::from(args.first().map(String::as_str) == Some("session"));
    while index < args.len() {
        let argument = &args[index];
        if matches!(
            argument.as_str(),
            "--help" | "-h" | "--version" | "-V" | "--"
        ) {
            return None;
        }
        if matches!(argument.as_str(), "--resume" | "-r" | "--fork" | "--edit") {
            resuming = true;
            index += 1;
            continue;
        }
        let takes_value = matches!(
            argument.as_str(),
            "--name"
                | "-n"
                | "--session-id"
                | "--id"
                | "--path"
                | "--provider"
                | "--model"
                | "--system"
                | "--max-turns"
                | "--with-extension"
                | "--with-builtin"
                | "--with-streamable-http-extension"
                | "--mode"
        );
        if takes_value {
            let value = args.get(index + 1)?;
            if argument == "--with-extension" && value.starts_with("farhelm-reporter:") {
                reporter_collision = true;
            }
            index += 2;
            continue;
        }
        if argument.starts_with("--with-extension=")
            && argument["--with-extension=".len()..].starts_with("farhelm-reporter:")
        {
            reporter_collision = true;
        }
        if !argument.starts_with('-') {
            return None;
        }
        index += 1;
    }
    Some(GooseLaunchShape {
        resuming,
        reporter_collision,
        needs_session_subcommand: args.is_empty(),
    })
}

/// Look up `name` in NUL-delimited environ bytes, FIRST-match wins,
/// returning the raw value bytes.
///
/// First-match is `getenv` semantics: it is what Goose's own
/// startup read saw, so resolving any other same-named entry would
/// bind a store the runtime never used. Entries without `=` are
/// skipped; values may contain `=` (split once). Raw bytes rather
/// than `&str` because Goose reads its roots with `var_os` — no
/// UTF-8 requirement — so the resolver must see a non-UTF-8 value
/// to classify it, not have the lookup silently swallow it into
/// absence.
fn environ_lookup<'a>(environ: &'a [u8], name: &str) -> Option<&'a [u8]> {
    for entry in environ.split(|byte| *byte == b'\0') {
        let Some(position) = entry.iter().position(|byte| *byte == b'=') else {
            continue;
        };
        if entry[..position] != *name.as_bytes() {
            continue;
        }
        return Some(&entry[position + 1..]);
    }
    None
}

/// One root rule's verdict over the raw looked-up value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RootDecision<'a> {
    /// An absolute UTF-8 root: bind this store.
    Use(&'a str),
    /// Missing, empty, or relative (by byte shape, exactly as
    /// Goose's absolute filter reads it): fall through to the next
    /// rule. A non-UTF-8 relative root falls through too — Goose
    /// ignores it the same way, so refusing would deny a report
    /// Goose serves from the next rule down.
    FallThrough,
    /// Absolute but unrepresentable: Goose binds this store (its
    /// filter is byte-level), and the resolved path must persist
    /// into a UTF-8 `TEXT` column, so falling through would read a
    /// store the runtime never used. Refuse instead.
    Refuse,
}

/// Classify one looked-up root value: absolute UTF-8 wins,
/// relative/empty falls through, absolute-but-non-UTF-8 refuses.
fn root_decision(value: Option<&[u8]>) -> RootDecision<'_> {
    let Some(raw) = value else {
        return RootDecision::FallThrough;
    };
    if raw.is_empty() || !raw.starts_with(b"/") {
        return RootDecision::FallThrough;
    }
    match std::str::from_utf8(raw) {
        Ok(root) => RootDecision::Use(root),
        Err(_) => RootDecision::Refuse,
    }
}

/// Resolve the exact Goose store the attributed runtime uses, from that
/// runtime's exec-time environ bytes (`procs::read_environ`).
///
/// The rules mirror Goose's own `paths.rs` plus the etcetera 0.11
/// `choose_app_strategy` the pinned Goose tree's `Cargo.lock` names
/// (XDG on BOTH Linux and macOS — verified against the published
/// crate source, not inferred):
/// - an absolute `GOOSE_PATH_ROOT=R` wins: `R/data/sessions/sessions.db`;
/// - else an absolute `XDG_DATA_HOME=D`: `D/goose/sessions/sessions.db`;
/// - else `H/.local/share/goose/sessions/sessions.db` with `H` from
///   this same environ's `HOME`.
///
/// Relative or empty roots are IGNORED (not errors), falling through
/// to the next rule — exactly as Goose's absolute-only filter reads
/// them. An absolute-but-non-UTF-8 root REFUSES rather than falling
/// through: Goose binds that store (its filter is byte-level), and
/// the resolved path must persist into a UTF-8 `TEXT` column, so
/// falling through would read a store the runtime never used. A
/// missing, relative, empty, or non-UTF-8 `HOME` refuses: Goose
/// itself crashes without a home (`expect("goose requires a home
/// dir")`), so a running Goose was exec'd with a usable one, and
/// guessing another home (including the supervisor's own, or a
/// `passwd` lookup) would bind the wrong store.
///
/// The output is absolute, UTF-8, NUL-free by construction, and bounded
/// by [`MAX_SESSION_PATH_BYTES`](super::MAX_SESSION_PATH_BYTES) — it
/// persists into a `TEXT` column, so anything else refuses. It is never
/// canonicalized: the open itself resolves symlinks and `..`, and
/// resolving first would fail on absent components.
pub(crate) fn resolve_goose_store(environ: &[u8]) -> Option<String> {
    const SUFFIX: &str = "sessions/sessions.db";
    let data_dir;
    match root_decision(environ_lookup(environ, "GOOSE_PATH_ROOT")) {
        RootDecision::Use(root) => {
            data_dir = Path::new(root).join("data");
        }
        RootDecision::Refuse => return None,
        RootDecision::FallThrough => {
            match root_decision(environ_lookup(environ, "XDG_DATA_HOME")) {
                RootDecision::Use(root) => {
                    data_dir = Path::new(root).join("goose");
                }
                RootDecision::Refuse => return None,
                RootDecision::FallThrough => {
                    let home = environ_lookup(environ, "HOME")
                        .and_then(|raw| std::str::from_utf8(raw).ok())
                        .filter(|home| !home.is_empty() && home.starts_with('/'))?;
                    data_dir = Path::new(home).join(".local/share/goose");
                }
            }
        }
    }
    let store = data_dir.join(SUFFIX);
    let store = store.to_str()?;
    if !store.starts_with('/') || store.len() > super::MAX_SESSION_PATH_BYTES {
        return None;
    }
    Some(store.to_string())
}

/// Whether one store row's metadata proves a foreground root: EXACTLY
/// `user` with a NULL parent.
///
/// NULL parentage is necessary but never sufficient — native children
/// are created as `sub_agent` and linked to their parent in a second,
/// separate transaction, so a `sub_agent` row with a NULL parent is
/// the creation race made visible, not a root. `scheduled`
/// (runnable, not supported capture), `hidden`/`terminal`/`gateway`/
/// `acp`, unknown strings, and a NULL `session_type` all refuse, as
/// does any non-NULL parent on ANY type (delegation evidence,
/// regardless of type). Genuine foreground fork/copy/resume needs no
/// parent-link acceptance at the pinned revision (`copy_session` drops
/// parentage; CLI `--fork` calls it): a foreground fork is a NEW
/// `user` row admitted as a legitimate transition, not lineage to
/// accept. Unknown roles fail closed.
pub(crate) fn is_foreground_session_type(
    session_type: Option<&str>,
    parent_session_id: Option<&str>,
) -> bool {
    session_type == Some("user") && parent_session_id.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The doorway and admission share one word: the helper's literal
    /// `"goose"`. Anything else — including near-misses a sanitizer
    /// might normalize — refuses in both places.
    #[farhelm_testtrace::test]
    fn the_goose_source_vocabulary_is_the_single_helper_word() {
        assert!(is_goose_foreground_source("goose"));
        assert!(!is_goose_foreground_source(""));
        assert!(!is_goose_foreground_source("Goose"));
        assert!(!is_goose_foreground_source("goose-hook"));
        assert!(!is_goose_foreground_source("session_start"));
    }

    /// The launch classification dispatches on the effective program: an
    /// `env` prefix is skipped, shells are wrappers, and anything else —
    /// including every package-manager and interpreter spelling — fails
    /// closed rather than inheriting another shape's rules.
    #[farhelm_testtrace::test]
    fn the_launch_classification_follows_the_effective_program() {
        let program = |argv: &[&str]| {
            classify_goose_launch(
                &argv
                    .iter()
                    .map(|word| (*word).to_string())
                    .collect::<Vec<_>>(),
            )
        };
        assert!(matches!(program(&["goose"]), GooseLaunchProgram::Goose));
        assert!(matches!(
            program(&["/usr/local/bin/goose", "session"]),
            GooseLaunchProgram::Goose
        ));
        assert!(matches!(
            program(&["env", "GOOSE_MODE=auto", "goose", "session"]),
            GooseLaunchProgram::Goose
        ));
        assert!(matches!(
            program(&["sh", "-c", "exec goose session"]),
            GooseLaunchProgram::Shell
        ));
        assert!(matches!(
            program(&["bash", "-c", "exec goose"]),
            GooseLaunchProgram::Shell
        ));
        assert!(matches!(program(&[]), GooseLaunchProgram::Unknown));
        assert!(matches!(
            program(&["my-wrapper"]),
            GooseLaunchProgram::Unknown
        ));
        for spelling in ["npm", "npx", "bun", "bunx", "node", "nodejs"] {
            assert!(
                matches!(program(&[spelling]), GooseLaunchProgram::Unknown),
                "{spelling} has no verified Goose mapping"
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
            GooseLaunchProgram::Goose,
            GooseLaunchProgram::Shell,
            GooseLaunchProgram::Unknown,
        ] {
            assert_eq!(
                GooseLaunchProgram::from_column_value(Some(program.column_value())),
                program,
                "the {program:?} spelling must survive the column"
            );
        }
        assert_eq!(
            GooseLaunchProgram::from_column_value(None),
            GooseLaunchProgram::Unknown
        );
        assert_eq!(
            GooseLaunchProgram::from_column_value(Some("omp")),
            GooseLaunchProgram::Unknown
        );
        assert_eq!(
            GooseLaunchProgram::from_column_value(Some("")),
            GooseLaunchProgram::Unknown
        );
    }

    /// The store resolver honors an absolute custom root, falls through
    /// relative and empty roots exactly as Goose's absolute-only filter
    /// does, honors an absolute `XDG_DATA_HOME`, and defaults to the
    /// runtime's own `HOME` — never the supervisor's.
    #[farhelm_testtrace::test]
    fn the_store_resolver_follows_gooses_own_precedence() {
        let resolve = |entries: &[&[u8]]| {
            let environ = entries.join(&b'\0');
            resolve_goose_store(&environ)
        };
        assert_eq!(
            resolve(&[b"GOOSE_PATH_ROOT=/custom", b"HOME=/home/u"]),
            Some("/custom/data/sessions/sessions.db".to_string()),
            "an absolute custom root wins outright"
        );
        assert_eq!(
            resolve(&[
                b"GOOSE_PATH_ROOT=relative/root",
                b"XDG_DATA_HOME=/xdg",
                b"HOME=/home/u",
            ]),
            Some("/xdg/goose/sessions/sessions.db".to_string()),
            "a relative custom root is ignored, not an error"
        );
        assert_eq!(
            resolve(&[b"GOOSE_PATH_ROOT=", b"HOME=/home/u"]),
            Some("/home/u/.local/share/goose/sessions/sessions.db".to_string()),
            "an empty custom root falls through to HOME"
        );
        assert_eq!(
            resolve(&[b"XDG_DATA_HOME=relative", b"HOME=/home/u"]),
            Some("/home/u/.local/share/goose/sessions/sessions.db".to_string()),
            "a relative XDG root falls through to HOME"
        );
        assert_eq!(
            resolve(&[b"HOME=/home/u"]),
            Some("/home/u/.local/share/goose/sessions/sessions.db".to_string()),
            "HOME alone resolves the default layout"
        );
        // A malformed entry (no `=`) is skipped, and a value carrying
        // `=` splits once — the lookup is over names, not substrings.
        assert_eq!(
            resolve(&[b"NOEQUALS", b"HOME=/home/u"]),
            Some("/home/u/.local/share/goose/sessions/sessions.db".to_string()),
        );
        assert_eq!(
            resolve(&[b"HOME=/home/u", b"OTHER=A=B"]),
            Some("/home/u/.local/share/goose/sessions/sessions.db".to_string()),
        );
    }

    /// Name lookup is FIRST-match, matching `getenv` semantics: what
    /// Goose's own startup read saw is what the resolver binds.
    #[farhelm_testtrace::test]
    fn the_store_resolver_reads_the_first_same_named_entry() {
        let resolve = |entries: &[&[u8]]| {
            let environ = entries.join(&b'\0');
            resolve_goose_store(&environ)
        };
        assert_eq!(
            resolve(&[b"HOME=/first", b"HOME=/second"]),
            Some("/first/.local/share/goose/sessions/sessions.db".to_string()),
        );
        assert_eq!(
            resolve(&[b"GOOSE_PATH_ROOT=/first", b"GOOSE_PATH_ROOT=/second"]),
            Some("/first/data/sessions/sessions.db".to_string()),
        );
    }

    /// No home, no store: a missing, relative, or empty `HOME` refuses
    /// rather than guessing — Goose itself cannot run without one, so
    /// any guess would bind the wrong store. Over-long results refuse
    /// rather than truncating into a different path.
    #[farhelm_testtrace::test]
    fn the_store_resolver_refuses_without_a_usable_home() {
        let resolve = |entries: &[&[u8]]| {
            let environ = entries.join(&b'\0');
            resolve_goose_store(&environ)
        };
        assert_eq!(resolve(&[]), None, "no environ, no store");
        assert_eq!(resolve(&[b"PATH=/usr/bin"]), None, "no HOME, no store");
        assert_eq!(
            resolve(&[b"HOME=relative/home"]),
            None,
            "a relative HOME refuses"
        );
        assert_eq!(resolve(&[b"HOME="]), None, "an empty HOME refuses");
        let long_home = format!("/{}", "h".repeat(super::super::MAX_SESSION_PATH_BYTES));
        assert_eq!(
            resolve(&[format!("HOME={long_home}").as_bytes()]),
            None,
            "an over-long result refuses rather than truncating"
        );
    }

    /// Non-UTF-8 roots classify by byte shape, the way Goose's own
    /// byte-level absolute filter reads them: an absolute one
    /// refuses — Goose binds that store and the locator cannot name
    /// it, so falling through would read the wrong store — while a
    /// relative one is ignored exactly as Goose ignores it. A
    /// non-UTF-8 `HOME` refuses outright.
    #[farhelm_testtrace::test]
    fn the_store_resolver_classifies_non_utf8_roots_by_byte_shape() {
        let resolve = |entries: &[&[u8]]| {
            let environ = entries.join(&b'\0');
            resolve_goose_store(&environ)
        };
        assert_eq!(
            resolve(&[b"GOOSE_PATH_ROOT=/\xff/nonutf8", b"HOME=/home/u"]),
            None,
            "an absolute non-UTF-8 custom root refuses rather than reading the HOME store"
        );
        assert_eq!(
            resolve(&[b"XDG_DATA_HOME=/\xff/nonutf8", b"HOME=/home/u"]),
            None,
            "an absolute non-UTF-8 XDG root refuses rather than reading the HOME store"
        );
        assert_eq!(
            resolve(&[b"GOOSE_PATH_ROOT=relative/\xff", b"HOME=/home/u"]),
            Some("/home/u/.local/share/goose/sessions/sessions.db".to_string()),
            "a relative non-UTF-8 root is ignored like any relative root"
        );
        assert_eq!(
            resolve(&[b"HOME=/\xff/nonutf8"]),
            None,
            "a non-UTF-8 HOME refuses"
        );
    }

    /// The allowlist is positive and exact: `user` plus a NULL parent,
    /// and nothing else. The `sub_agent`-with-NULL-parent row is the
    /// creation race made visible — type alone rejects regardless of
    /// parent — and any non-NULL parent is delegation evidence on any
    /// type. Unknown roles and a NULL type fail closed.
    #[farhelm_testtrace::test]
    fn the_foreground_allowlist_is_exactly_user_with_null_parent() {
        assert!(is_foreground_session_type(Some("user"), None));
        assert!(!is_foreground_session_type(Some("sub_agent"), None));
        assert!(!is_foreground_session_type(
            Some("sub_agent"),
            Some("parent")
        ));
        assert!(!is_foreground_session_type(Some("user"), Some("parent")));
        for role in [
            "scheduled",
            "hidden",
            "terminal",
            "gateway",
            "acp",
            "User",
            "USER",
            "",
            "user ",
        ] {
            assert!(
                !is_foreground_session_type(Some(role), None),
                "{role:?} is not a foreground root"
            );
        }
        assert!(!is_foreground_session_type(None, None));
        assert!(!is_foreground_session_type(None, Some("parent")));
    }
}
