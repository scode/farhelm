//! What a session runs, as one resolved value carried end to end.
//!
//! SPEC.md (Concepts, "Launch") gives every launch one of two launch kinds,
//! and this module is that rule as a type. The helm resolves the user's
//! choices into a [`SessionLaunch`] before it contacts a supervisor; the
//! supervisor validates it once, stores it with the session, and every later
//! operation (restart, Restart with, clone, replace, inherit, the YOLO guard)
//! reads that stored value rather than re-deriving anything from a command
//! line. Farhelm never parses a command to decide what it runs, whether it is
//! YOLO, or how to resume it.
//!
//! The third variant, [`SessionLaunch::Legacy`], exists only for sessions
//! created before launch kinds: the upgrade migrates them in, and nothing
//! creates one afterwards ([`SessionLaunch::validate_new`] refuses it).
//!
//! Placeholders are whole argv elements, filled at spawn by the supervisor:
//! [`CWD_PLACEHOLDER`](crate::session_launch::CWD_PLACEHOLDER), [`CONVERSATION_PLACEHOLDER`](crate::session_launch::CONVERSATION_PLACEHOLDER) and
//! [`FARHELM_ARGS_PLACEHOLDER`](crate::session_launch::FARHELM_ARGS_PLACEHOLDER). They live here because both sides check
//! them: the helm composes them into agent launches and validates command
//! launches before sending, and the supervisor fills them.

use serde::{Deserialize, Serialize};

use crate::{AgentKind, LaunchHarness, LaunchSelection};

/// Stands for the session's working directory, as a whole argument, in a
/// start or resume command.
pub const CWD_PLACEHOLDER: &str = "{cwd}";

/// Stands for the captured conversation a resume command continues, as a
/// whole argument. Refused in a start command: there is nothing to fill it
/// with before a conversation exists.
pub const CONVERSATION_PLACEHOLDER: &str = "{conversation}";

/// Marks where Farhelm places a declared agent type's own launch arguments
/// (its conversation-reporting hook and the instructions pointer), as a
/// whole argument exactly once. Replaced by nothing for an agent type that
/// takes no such arguments, so the rule never depends on which type it is.
pub const FARHELM_ARGS_PLACEHOLDER: &str = "{farhelm_args}";

/// One session's launch, resolved: what it runs and how it resumes.
///
/// Serialized with an internal `kind` tag because the value is stored in the
/// supervisor's database and carried on the wire and in the helm's session
/// cache; the tag makes each stored row say which kind it is without a
/// reader having to guess from which fields are present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionLaunch {
    /// An agent type plus its own choices, with the commands Farhelm composed
    /// from them. Replace and agent clone copy `start` and `resume` as
    /// stored; Clone, Replace with and Restart with compose anew from
    /// `selection`.
    Agent {
        selection: LaunchSelection,
        /// The start argv, with [`FARHELM_ARGS_PLACEHOLDER`] exactly once.
        start: Vec<String>,
        /// The resume argv, with [`CONVERSATION_PLACEHOLDER`] and
        /// [`FARHELM_ARGS_PLACEHOLDER`] exactly once each, or `None` for an
        /// agent type without conversation reporting.
        resume: Option<Vec<String>>,
    },
    /// A command line the user wrote, with their own YOLO assertion.
    Command(CommandLaunch),
    /// A session created before launch kinds existed (SPEC.md, the
    /// launch-kinds upgrade paragraph): its stored command, integration and
    /// resume command, untouched. Restart runs the stored resume command;
    /// plain Replace, agent clone, inheritance and Restart with refuse it.
    Legacy {
        invocation: String,
        agent_kind: AgentKind,
        resume_template: Option<Vec<String>>,
    },
}

/// A command launch's fields: what the user wrote and asserted, carried
/// verbatim from the launcher or the agent CLI to the supervisor.
///
/// Its own struct, rather than fields on the variant, because two values
/// carry it: a resolved [`SessionLaunch`] and an agent's [`LaunchRequest`],
/// which differ only in how an agent launch is expressed. One struct keeps
/// SPEC.md's command-launch rules in one [`CommandLaunch::validate`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandLaunch {
    pub command: String,
    /// The user's statement of whether the command (and its resume
    /// command) runs without approval prompts. Farhelm believes it.
    pub yolo: bool,
    /// The agent type the command runs, when declared. Buys that type's
    /// status reading, conversation reporting and, with `resume`, Restart.
    pub agent: Option<LaunchHarness>,
    /// The user's own resume command; only with a declared agent type.
    pub resume: Option<String>,
}

/// What an agent asks the helm to launch (`farhelm agent create`): an
/// agent type with its choices, which the helm composes into commands
/// exactly as it does for the launcher, or a command launch as written.
///
/// Not a [`SessionLaunch`], because an agent never supplies composed argv:
/// accepting one would let a request smuggle arbitrary start and resume
/// commands under an agent launch's YOLO verdict and kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LaunchRequest {
    Agent { selection: LaunchSelection },
    Command(CommandLaunch),
}

impl CommandLaunch {
    /// SPEC.md's command-launch rules, checked by shape alone: the command
    /// splits and names a program, `{conversation}` is refused in it,
    /// `{farhelm_args}` appears exactly once when an agent type is declared
    /// and not at all otherwise, and a resume command needs a declared type
    /// and then holds `{conversation}` and `{farhelm_args}` once each. The
    /// refusal names the field, so a client can show it beside the right
    /// control.
    pub fn validate(&self) -> Result<(), String> {
        let start = split("command", &self.command)?;
        check_argv("command", &start)?;
        refuse_present("command", &start, CONVERSATION_PLACEHOLDER)?;
        if self.agent.is_some() {
            require_exactly_once("command", &start, FARHELM_ARGS_PLACEHOLDER)?;
        } else if start
            .iter()
            .any(|element| element == FARHELM_ARGS_PLACEHOLDER)
        {
            return Err(format!(
                "command: {FARHELM_ARGS_PLACEHOLDER} needs a declared agent type; declare the \
                 agent type the command runs, or remove {FARHELM_ARGS_PLACEHOLDER}"
            ));
        }
        if let Some(resume) = &self.resume {
            if self.agent.is_none() {
                return Err(
                    "resume command: Resume needs a declared agent type; declare the agent type \
                     the command runs, or remove the resume command"
                        .to_string(),
                );
            }
            let resume = split("resume command", resume)?;
            check_argv("resume command", &resume)?;
            require_exactly_once("resume command", &resume, CONVERSATION_PLACEHOLDER)?;
            require_exactly_once("resume command", &resume, FARHELM_ARGS_PLACEHOLDER)?;
        }
        Ok(())
    }
}

/// Which of the launch kinds a [`SessionLaunch`] is, for messages and for
/// the "same kind" check Restart with makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchKind {
    Agent,
    Command,
    Legacy,
}

impl LaunchKind {
    /// The word a user reads for this kind.
    pub fn word(self) -> &'static str {
        match self {
            LaunchKind::Agent => "agent",
            LaunchKind::Command => "command",
            LaunchKind::Legacy => "legacy",
        }
    }
}

impl SessionLaunch {
    /// A command launch with no declared agent type and no resume command,
    /// asserted not YOLO: the plainest launch there is, and what a caller
    /// that only has a command line to run (a test fixture, a scripted
    /// create that never asks about YOLO) builds.
    pub fn plain_command(command: impl Into<String>) -> SessionLaunch {
        SessionLaunch::Command(CommandLaunch {
            command: command.into(),
            yolo: false,
            agent: None,
            resume: None,
        })
    }

    /// What a session stored before launch kinds becomes (SPEC.md, the
    /// launch-kinds upgrade): a structured row becomes an agent launch, and
    /// every other row stays legacy, untouched.
    ///
    /// The previous release appended Farhelm's own arguments after the
    /// stored start and resume commands at spawn, so the conversion puts
    /// [`FARHELM_ARGS_PLACEHOLDER`] at the end of each; the session keeps
    /// launching exactly as it did. (The environment-variable prefix some
    /// agent types used to receive moves into the launch's environment,
    /// which changes how, not what.) A structured row that would not make
    /// a valid agent launch (a selection naming a different kind than the
    /// row, a command that no longer splits, a resume command without its
    /// `{conversation}`) stays legacy rather than being repaired by a
    /// guess. Shared by the supervisor's row migration and the helm's
    /// session-cache migration, so a cached row and its host's own row
    /// convert alike.
    pub fn from_pre_launch_kinds(
        invocation: &str,
        agent_kind: AgentKind,
        resume_template: Option<Vec<String>>,
        selection: Option<LaunchSelection>,
    ) -> SessionLaunch {
        let legacy = |resume_template| SessionLaunch::Legacy {
            invocation: invocation.to_string(),
            agent_kind,
            resume_template,
        };
        let Some(selection) = selection else {
            return legacy(resume_template);
        };
        if selection.harness.agent_kind() != agent_kind {
            return legacy(resume_template);
        }
        let Ok(mut start) = shell_words::split(invocation) else {
            return legacy(resume_template);
        };
        start.push(FARHELM_ARGS_PLACEHOLDER.to_string());
        let candidate = SessionLaunch::Agent {
            selection,
            start,
            resume: resume_template.clone().map(|mut resume| {
                resume.push(FARHELM_ARGS_PLACEHOLDER.to_string());
                resume
            }),
        };
        match candidate.validate_new() {
            Ok(()) => candidate,
            Err(_) => legacy(resume_template),
        }
    }

    /// This launch's kind.
    pub fn launch_kind(&self) -> LaunchKind {
        match self {
            SessionLaunch::Agent { .. } => LaunchKind::Agent,
            SessionLaunch::Command(_) => LaunchKind::Command,
            SessionLaunch::Legacy { .. } => LaunchKind::Legacy,
        }
    }

    /// An agent launch's choices, or `None` for any other kind: what
    /// remembered defaults, recent setups, Clone and Restart with read for
    /// an agent launch, and what nothing may guess for a command.
    pub fn agent_selection(&self) -> Option<&LaunchSelection> {
        match self {
            SessionLaunch::Agent { selection, .. } => Some(selection),
            SessionLaunch::Command(_) | SessionLaunch::Legacy { .. } => None,
        }
    }

    /// The agent type this launch runs, when it has one: an agent launch's
    /// harness or a command launch's declared type. A legacy session has a
    /// stored integration kind but no agent type in this sense.
    pub fn agent_type(&self) -> Option<LaunchHarness> {
        match self {
            SessionLaunch::Agent { selection, .. } => Some(selection.harness),
            SessionLaunch::Command(command) => command.agent,
            SessionLaunch::Legacy { .. } => None,
        }
    }

    /// The supervisor integration this launch runs under: the agent type's,
    /// generic for a command launch with no declared type, and the stored
    /// kind for a legacy session.
    pub fn agent_kind(&self) -> AgentKind {
        match self {
            SessionLaunch::Agent { .. } | SessionLaunch::Command(_) => self
                .agent_type()
                .map_or(AgentKind::Generic, LaunchHarness::agent_kind),
            SessionLaunch::Legacy { agent_kind, .. } => *agent_kind,
        }
    }

    /// Whether this launch runs without approval prompts, as the launch
    /// itself says: an agent launch by its effective permission, a command
    /// launch by the user's assertion, and `None` for a legacy session,
    /// which carries no assertion and is shown as unclassified.
    pub fn yolo(&self) -> Option<bool> {
        match self {
            SessionLaunch::Agent { selection, .. } => {
                Some(crate::yolo::selection_is_yolo(selection))
            }
            SessionLaunch::Command(command) => Some(command.yolo),
            SessionLaunch::Legacy { .. } => None,
        }
    }

    /// The start command as text, for display: an agent launch's composed
    /// argv shell-joined with the [`FARHELM_ARGS_PLACEHOLDER`] left out, a
    /// command launch's command as written, a legacy session's invocation.
    pub fn display_command(&self) -> String {
        match self {
            SessionLaunch::Agent { start, .. } => shell_words::join(
                start
                    .iter()
                    .filter(|element| element.as_str() != FARHELM_ARGS_PLACEHOLDER),
            ),
            SessionLaunch::Command(command) => command.command.clone(),
            SessionLaunch::Legacy { invocation, .. } => invocation.clone(),
        }
    }

    /// Whether this launch has a resume command at all. Restart still needs
    /// a captured conversation; this answers only whether one could ever be
    /// resumed.
    pub fn has_resume(&self) -> bool {
        match self {
            SessionLaunch::Agent { resume, .. } => resume.is_some(),
            SessionLaunch::Command(command) => command.resume.is_some(),
            SessionLaunch::Legacy {
                resume_template, ..
            } => resume_template.is_some(),
        }
    }

    /// Check a launch a create, Restart with, or inheritance is about to
    /// use, by shape alone: the refusal names the field, so a client can
    /// show it beside the right control.
    ///
    /// Legacy is refused outright, because nothing creates one. An agent
    /// launch's argv is the helm's own composition, so its checks catch a
    /// compiler bug rather than a user mistake; a command launch's are
    /// SPEC.md's command-launch rules. Validity that needs more than shape
    /// (a model the catalog offers, a directory that exists) is checked
    /// where that knowledge lives.
    pub fn validate_new(&self) -> Result<(), String> {
        match self {
            SessionLaunch::Agent { start, resume, .. } => {
                check_argv("the agent launch's start command", start)?;
                require_exactly_once(
                    "the agent launch's start command",
                    start,
                    FARHELM_ARGS_PLACEHOLDER,
                )?;
                refuse_present(
                    "the agent launch's start command",
                    start,
                    CONVERSATION_PLACEHOLDER,
                )?;
                if let Some(resume) = resume {
                    check_argv("the agent launch's resume command", resume)?;
                    require_exactly_once(
                        "the agent launch's resume command",
                        resume,
                        FARHELM_ARGS_PLACEHOLDER,
                    )?;
                    require_exactly_once(
                        "the agent launch's resume command",
                        resume,
                        CONVERSATION_PLACEHOLDER,
                    )?;
                }
                Ok(())
            }
            SessionLaunch::Command(command) => command.validate(),
            SessionLaunch::Legacy { .. } => Err(
                "a launch from before launch kinds cannot be used to start a session; start a new \
                 agent or command launch instead"
                    .to_string(),
            ),
        }
    }

    /// The start argv to spawn, still holding its placeholders.
    ///
    /// An agent launch's composed argv as stored, a command launch's command
    /// shell-split, or a legacy invocation shell-split. A command that does
    /// not split is an error here only if it slipped past
    /// [`SessionLaunch::validate_new`], which refuses it.
    pub fn start_argv(&self) -> Result<Vec<String>, String> {
        match self {
            SessionLaunch::Agent { start, .. } => Ok(start.clone()),
            SessionLaunch::Command(command) => split("command", &command.command),
            SessionLaunch::Legacy { invocation, .. } => split("invocation", invocation),
        }
    }

    /// The resume argv to spawn, still holding its placeholders, or `None`
    /// when this launch has no resume command.
    pub fn resume_argv(&self) -> Result<Option<Vec<String>>, String> {
        match self {
            SessionLaunch::Agent { resume, .. } => Ok(resume.clone()),
            SessionLaunch::Command(command) => command
                .resume
                .as_deref()
                .map(|resume| split("resume command", resume))
                .transpose(),
            SessionLaunch::Legacy {
                resume_template, ..
            } => Ok(resume_template.clone()),
        }
    }

    /// The combined byte size of the user-chosen text in this launch: what
    /// counts toward SPEC.md's 64 KiB create bound beside the working
    /// directory and title. The JSON a launch serializes to is what the
    /// fingerprint and the wire carry, so that is what is counted.
    pub fn field_bytes(&self) -> usize {
        serde_json::to_string(self).map_or(usize::MAX, |json| json.len())
    }
}

/// Shell-split one command field, naming it in the refusal.
fn split(field: &str, text: &str) -> Result<Vec<String>, String> {
    shell_words::split(text).map_err(|error| format!("{field}: {error}"))
}

/// The argv rules every launch shares: a program that names something, and
/// no placeholder standing as the program.
fn check_argv(field: &str, argv: &[String]) -> Result<(), String> {
    let Some(program) = argv.first() else {
        return Err(format!("{field} is empty"));
    };
    if program.is_empty() {
        return Err(format!("{field} names no program"));
    }
    for placeholder in [
        CWD_PLACEHOLDER,
        CONVERSATION_PLACEHOLDER,
        FARHELM_ARGS_PLACEHOLDER,
    ] {
        if program == placeholder {
            return Err(format!(
                "{field}: {placeholder} cannot be the program; it belongs in an argument slot"
            ));
        }
    }
    Ok(())
}

/// Require `placeholder` as a whole argument exactly once.
fn require_exactly_once(field: &str, argv: &[String], placeholder: &str) -> Result<(), String> {
    match argv
        .iter()
        .filter(|element| element.as_str() == placeholder)
        .count()
    {
        1 => Ok(()),
        0 => Err(format!(
            "{field} must contain {placeholder} as a whole argument"
        )),
        _ => Err(format!("{field} must contain {placeholder} only once")),
    }
}

/// Refuse `placeholder` anywhere as a whole argument.
fn refuse_present(field: &str, argv: &[String], placeholder: &str) -> Result<(), String> {
    if argv.iter().any(|element| element == placeholder) {
        return Err(format!("{field} must not contain {placeholder}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LaunchPermission, LaunchSelection};

    fn command(command: &str, agent: Option<LaunchHarness>, resume: Option<&str>) -> SessionLaunch {
        SessionLaunch::Command(CommandLaunch {
            command: command.to_string(),
            yolo: false,
            agent,
            resume: resume.map(str::to_string),
        })
    }

    /// Spec: SPEC.md's command-launch rules, as one table — `{farhelm_args}`
    /// exactly once iff an agent type is declared, `{conversation}` refused
    /// in the start command, a resume command only with a declared type and
    /// then with `{conversation}` and `{farhelm_args}` exactly once each.
    ///
    /// Why: these are the rules a user meets in the launcher and the agent
    /// CLI, and both sides (helm before sending, supervisor before storing)
    /// call this one function; a rule that drifted here would be refused in
    /// one place and accepted in the other.
    #[test]
    fn command_launch_rules_hold() {
        let ok = [
            command("sh -c 'sleep 1'", None, None),
            command("claude {farhelm_args}", Some(LaunchHarness::Claude), None),
            command(
                "wrapper run {cwd} claude {farhelm_args}",
                Some(LaunchHarness::Claude),
                Some("claude {farhelm_args} --resume {conversation}"),
            ),
        ];
        for launch in ok {
            assert_eq!(launch.validate_new(), Ok(()), "{launch:?}");
        }
        let refused = [
            (command("", None, None), "command is empty"),
            (command("'unterminated", None, None), "command:"),
            (command("{cwd} x", None, None), "cannot be the program"),
            (
                command("claude {farhelm_args}", None, None),
                "needs a declared agent type",
            ),
            (
                command("claude", Some(LaunchHarness::Claude), None),
                "{farhelm_args} as a whole",
            ),
            (
                command(
                    "claude {farhelm_args} {farhelm_args}",
                    Some(LaunchHarness::Claude),
                    None,
                ),
                "only once",
            ),
            (
                command(
                    "claude {farhelm_args} {conversation}",
                    Some(LaunchHarness::Claude),
                    None,
                ),
                "must not contain {conversation}",
            ),
            (
                command("sh", None, Some("sh {conversation}")),
                "Resume needs a declared",
            ),
            (
                command(
                    "claude {farhelm_args}",
                    Some(LaunchHarness::Claude),
                    Some("claude --resume {conversation}"),
                ),
                "resume command must contain {farhelm_args}",
            ),
            (
                command(
                    "claude {farhelm_args}",
                    Some(LaunchHarness::Claude),
                    Some("claude {farhelm_args}"),
                ),
                "resume command must contain {conversation}",
            ),
        ];
        for (launch, expected) in refused {
            let error = launch.validate_new().expect_err("refused");
            assert!(error.contains(expected), "{launch:?}: {error}");
        }
    }

    /// Spec: a legacy launch is never accepted for a new session, and an
    /// agent launch's composed commands are held to the same placeholder
    /// counts.
    ///
    /// Why: legacy sessions only arrive by migration (SPEC.md), so a create
    /// carrying one is a bug or a forged request; an agent launch missing
    /// its `{farhelm_args}` would silently launch without its hook.
    #[test]
    fn legacy_is_refused_and_agent_commands_are_checked() {
        let legacy = SessionLaunch::Legacy {
            invocation: "claude".into(),
            agent_kind: AgentKind::Claude,
            resume_template: None,
        };
        assert!(
            legacy
                .validate_new()
                .unwrap_err()
                .contains("before launch kinds")
        );
        let selection = LaunchSelection {
            harness: LaunchHarness::Claude,
            model: None,
            effort: None,
            permissions: None,
            workspace_trust: None,
        };
        let agent = |start: &[&str], resume: Option<&[&str]>| SessionLaunch::Agent {
            selection: selection.clone(),
            start: start.iter().map(|s| s.to_string()).collect(),
            resume: resume.map(|r| r.iter().map(|s| s.to_string()).collect()),
        };
        assert_eq!(
            agent(
                &["claude", "{farhelm_args}"],
                Some(&["claude", "--resume", "{conversation}", "{farhelm_args}"])
            )
            .validate_new(),
            Ok(())
        );
        assert_eq!(
            agent(&["claude"], None).validate_new(),
            Err(
                "the agent launch's start command must contain {farhelm_args} as a whole argument"
                    .to_string()
            )
        );
        assert_eq!(
            agent(
                &["claude", "{farhelm_args}"],
                Some(&["claude", "{farhelm_args}"])
            )
            .validate_new(),
            Err(
                "the agent launch's resume command must contain {conversation} as a whole argument"
                    .to_string()
            )
        );
    }

    /// Spec: the YOLO verdict comes from the launch itself — an agent
    /// launch's effective permission, a command launch's assertion, none for
    /// a legacy session — and the integration kind from the agent type.
    ///
    /// Why: SPEC.md forbids reading a command line to second-guess the
    /// assertion, and the guard, the sidebar mark and the confirmation
    /// reason must all agree; they all read these two methods.
    #[test]
    fn yolo_and_kind_come_from_the_launch() {
        let yolo_agent = SessionLaunch::Agent {
            selection: LaunchSelection {
                harness: LaunchHarness::Claude,
                model: None,
                effort: None,
                permissions: Some(LaunchPermission::Yolo),
                workspace_trust: None,
            },
            start: vec!["claude".into(), "{farhelm_args}".into()],
            resume: None,
        };
        assert_eq!(yolo_agent.yolo(), Some(true));
        assert_eq!(yolo_agent.agent_kind(), AgentKind::Claude);
        let asserted = SessionLaunch::plain_command("claude --dangerously-skip-permissions");
        assert_eq!(asserted.yolo(), Some(false), "the assertion is believed");
        assert_eq!(asserted.agent_kind(), AgentKind::Generic);
        let legacy = SessionLaunch::Legacy {
            invocation: "codex".into(),
            agent_kind: AgentKind::Codex,
            resume_template: None,
        };
        assert_eq!(legacy.yolo(), None);
        assert_eq!(legacy.agent_kind(), AgentKind::Codex);
    }

    /// Spec: a structured row from before launch kinds becomes an agent
    /// launch with `{farhelm_args}` appended to its stored start and resume
    /// commands; an unstructured row, or a structured one that would not
    /// make a valid agent launch, stays legacy with its fields untouched.
    ///
    /// Why: SPEC.md promises upgraded structured sessions keep resuming
    /// and launching as before. The previous release appended Farhelm's
    /// arguments after the stored commands, so appending the placeholder
    /// reproduces the old argv; a guess at "repairing" an odd row would
    /// launch something the session never ran.
    #[test]
    fn pre_launch_kind_rows_convert_by_shape() {
        let selection = LaunchSelection {
            harness: LaunchHarness::Claude,
            model: None,
            effort: None,
            permissions: None,
            workspace_trust: None,
        };
        let template = vec![
            "claude".to_string(),
            "--resume".to_string(),
            CONVERSATION_PLACEHOLDER.to_string(),
        ];
        assert_eq!(
            SessionLaunch::from_pre_launch_kinds(
                "claude --model 'a b'",
                AgentKind::Claude,
                Some(template.clone()),
                Some(selection.clone()),
            ),
            SessionLaunch::Agent {
                selection: selection.clone(),
                start: vec![
                    "claude".into(),
                    "--model".into(),
                    "a b".into(),
                    FARHELM_ARGS_PLACEHOLDER.into()
                ],
                resume: Some(vec![
                    "claude".into(),
                    "--resume".into(),
                    CONVERSATION_PLACEHOLDER.into(),
                    FARHELM_ARGS_PLACEHOLDER.into()
                ]),
            }
        );
        let legacy =
            |invocation: &str, kind, template: Option<Vec<String>>| SessionLaunch::Legacy {
                invocation: invocation.to_string(),
                agent_kind: kind,
                resume_template: template,
            };
        // No selection: a typed or profile command stays legacy.
        assert_eq!(
            SessionLaunch::from_pre_launch_kinds(
                "claude",
                AgentKind::Claude,
                Some(template.clone()),
                None
            ),
            legacy("claude", AgentKind::Claude, Some(template.clone()))
        );
        // A selection disagreeing with the row's kind.
        assert_eq!(
            SessionLaunch::from_pre_launch_kinds(
                "claude",
                AgentKind::Codex,
                None,
                Some(selection.clone())
            ),
            legacy("claude", AgentKind::Codex, None)
        );
        // A stored resume command without its conversation slot.
        let broken = vec!["claude".to_string()];
        assert_eq!(
            SessionLaunch::from_pre_launch_kinds(
                "claude",
                AgentKind::Claude,
                Some(broken.clone()),
                Some(selection)
            ),
            legacy("claude", AgentKind::Claude, Some(broken))
        );
        // A structured row of a type with no conversation reporting stored
        // no resume command, and converts to an agent launch without one.
        let muse = selection_for_muse();
        assert_eq!(
            SessionLaunch::from_pre_launch_kinds(
                "muse --yolo",
                AgentKind::Generic,
                None,
                Some(muse.clone())
            ),
            SessionLaunch::Agent {
                selection: muse,
                start: vec![
                    "muse".into(),
                    "--yolo".into(),
                    FARHELM_ARGS_PLACEHOLDER.into()
                ],
                resume: None,
            }
        );
    }

    /// A Muse selection with every choice at its default.
    fn selection_for_muse() -> LaunchSelection {
        LaunchSelection {
            harness: LaunchHarness::Muse,
            model: None,
            effort: None,
            permissions: None,
            workspace_trust: None,
        }
    }

    /// Spec: a stored or transmitted launch names its kind in a `kind` tag,
    /// a command launch's fields sit beside the tag, and an unknown field is
    /// refused in every kind, including inside a command launch and an
    /// agent's launch request.
    ///
    /// Why: the value is stored in the supervisor's database and the helm's
    /// cache, so its JSON is a format; a misspelled field silently dropped
    /// (say `yolo` sent as `YOLO`) would otherwise decode as a refusal-free
    /// launch missing the user's assertion.
    #[test]
    fn launch_json_is_tagged_and_strict() {
        let command = SessionLaunch::plain_command("sh");
        let json = serde_json::to_value(&command).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "command",
                "command": "sh",
                "yolo": false,
                "agent": null,
                "resume": null,
            })
        );
        assert_eq!(
            serde_json::from_value::<SessionLaunch>(json).unwrap(),
            command
        );
        // The assertion has no default: a command without `yolo` must fail
        // to decode rather than read as "not YOLO", the unsafe direction.
        let mut unasserted = serde_json::to_value(&command).unwrap();
        unasserted.as_object_mut().unwrap().remove("yolo");
        assert!(serde_json::from_value::<SessionLaunch>(unasserted.clone()).is_err());
        assert!(serde_json::from_value::<LaunchRequest>(unasserted).is_err());
        let mut extra = serde_json::to_value(&command).unwrap();
        extra["YOLO"] = serde_json::json!(true);
        assert!(serde_json::from_value::<SessionLaunch>(extra.clone()).is_err());
        assert!(serde_json::from_value::<LaunchRequest>(extra).is_err());
        let request = serde_json::json!({
            "kind": "agent",
            "selection": {"harness": "codex", "model": null, "effort": null, "permissions": null},
        });
        assert!(matches!(
            serde_json::from_value::<LaunchRequest>(request.clone()).unwrap(),
            LaunchRequest::Agent { .. }
        ));
        let mut smuggled = request;
        smuggled["start"] = serde_json::json!(["sh"]);
        assert!(
            serde_json::from_value::<LaunchRequest>(smuggled).is_err(),
            "an agent's request never carries composed commands"
        );
    }
}
