//! Tests of what an approval card shows and offers, for every request kind, without a window.
//!
//! These port the old app's card tests at the reference commit (re-expressed for the model, which the drawing only lays
//! out) and add one card per request kind with every fact and answer it must carry. They matter because a card is the
//! only thing the user sees before an agent acts: a fact left out, a value shown as the app's own words, or an Always
//! allow offered where SPEC.md forbids it would let an approval cover something the user never saw.

use farhelm_proto::approvals::{
    ApprovalAction, ApprovalAnswer, ApprovalLaunchKind, ApprovalSession, HostFieldChange,
    LaunchVerb, PendingApproval, QuickLaunchCard, TemplateKeyCard, TemplateShortcutCard,
};
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::launcher::{TemplateDestination, TemplateFields, TemplateNameKind};
use farhelm_proto::{
    AgentKind, CommandLaunch, DeleteGuard, LaunchHarness, LaunchPermission, LaunchSelection, Place,
    SessionLaunch, SessionRef,
};

use super::model::{
    ALLOW_HELP, Answer, AnswerHelp, AnswerLabel, COPY, COPY_HELP, COPY_ID, Card, ChipKind, Chips,
    DENY_HELP, FROM_COPY_NAME, FROM_HOST, FROM_ID_LABEL, Fact, LeadNote, ON_HOST_ONLY,
    OWN_HOST_HELP, Shape, TRUSTED_WORKSPACE, TUNNEL_HELP, TunnelOffer, count, facts, kind,
    selected, summary,
};
use crate::host_text::HostText;
use crate::permission::Shield;

// ---- Fixtures -------------------------------------------------------------------------------------------------------

/// A helm entry UUID for full session references.
const ENTRY: &str = "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d";

/// A full session reference built with the protocol's own formatter, so every fixture also stays inside the helm's ID
/// bounds.
fn reference(bare: &str) -> String {
    SessionRef::new(ENTRY, bare)
        .expect("a fixture bare ID within bounds")
        .to_string()
}

/// A session the helm knows nothing about but its reference.
fn unknown(id: &str) -> ApprovalSession {
    ApprovalSession {
        id: id.to_owned(),
        title: None,
        host_name: None,
        launch_kind: None,
    }
}

/// A waiting request from `host` asking for `action`, from a session titled "asker".
fn request(id: &str, host: &str, action: ApprovalAction) -> PendingApproval {
    PendingApproval {
        id: id.to_owned(),
        host_id: 7,
        host_name: host.to_owned(),
        session: ApprovalSession {
            id: reference("asker"),
            title: Some("asker".to_owned()),
            host_name: Some(host.to_owned()),
            launch_kind: None,
        },
        action,
        own_host_session_action: false,
        expires_at_ms: 1,
    }
}

/// A host value fact, written the way the tests expect it.
fn value(key: &'static str, raw: &str) -> Fact {
    Fact::Value(key, HostText::from_host(raw))
}

/// A boxed host value fact.
fn boxed(key: &'static str, raw: &str) -> Fact {
    Fact::Box(key, HostText::from_host(raw))
}

/// An agent launch selection with nothing chosen beyond the agent type.
fn selection(harness: LaunchHarness) -> LaunchSelection {
    LaunchSelection {
        custom_agent: None,
        harness,
        model: None,
        effort: None,
        permissions: None,
        permission_flags: None,
        wrapper_prefix: None,
        container: None,
        workspace_trust: None,
    }
}

/// An agent launch of `selection` whose start is `start`, without resume.
fn agent(selection: LaunchSelection, start: &[&str]) -> SessionLaunch {
    SessionLaunch::Agent {
        custom_reporting: None,
        selection,
        start: start.iter().map(|word| (*word).to_owned()).collect(),
        resume: None,
    }
}

/// A command launch with its assertion and optional resume.
fn command(command: &str, yolo: bool, resume: Option<&str>) -> SessionLaunch {
    SessionLaunch::Command(CommandLaunch {
        custom_agent: None,
        command: command.to_owned(),
        yolo,
        agent: None,
        resume: resume.map(str::to_owned),
    })
}

/// An ordinary create of `launch` on host "box" in `/w`.
fn create(launch: SessionLaunch) -> ApprovalAction {
    ApprovalAction::Launch {
        verb: LaunchVerb::Create,
        host_name: "box".to_owned(),
        cwd: "/w".to_owned(),
        repo: None,
        title: None,
        launch,
        source: None,
    }
}

/// The Permissions row of a list of facts.
fn chips(facts: &[Fact]) -> Vec<&Chips> {
    facts
        .iter()
        .filter_map(|fact| match fact {
            Fact::Permission(chips) => Some(chips),
            _ => None,
        })
        .collect()
}

/// The answers a card offers, in order.
fn offered(card: &Card) -> Vec<ApprovalAnswer> {
    card.answers.iter().map(|answer| answer.answer).collect()
}

// ---- The waiting list -----------------------------------------------------------------------------------------------

/// SPEC.md: the oldest request opens first; a chosen header stays expanded until it is answered or expires, after
/// which the oldest remaining request opens. Keeping the choice by identity is what stops an answer elsewhere in the
/// list from putting another request under the user's pointer in its place.
#[test]
fn the_oldest_request_opens_until_the_user_chooses_another_while_it_waits() {
    let stop = |id: &str| {
        request(
            id,
            "host",
            ApprovalAction::Stop {
                target: unknown("t"),
            },
        )
    };
    let waiting = vec![stop("old"), stop("middle"), stop("new")];
    assert_eq!(selected(&waiting, None).map(|a| a.id.as_str()), Some("old"));
    assert_eq!(
        selected(&waiting, Some("new")).map(|a| a.id.as_str()),
        Some("new")
    );
    let answered_elsewhere = vec![stop("middle"), stop("new")];
    assert_eq!(
        selected(&answered_elsewhere, Some("new")).map(|a| a.id.as_str()),
        Some("new")
    );
    assert_eq!(
        selected(&answered_elsewhere, Some("old")).map(|a| a.id.as_str()),
        Some("middle"),
        "a chosen request that left gives way to the oldest remaining one, never its old position"
    );
    assert!(selected(&[], Some("old")).is_none());
    assert_eq!(count(3), "1 of 3 waiting");
}

// ---- Facts and their layout -----------------------------------------------------------------------------------------

/// Security-relevant URL and flag tails wrap in full, and only technical destinations scroll sideways; long texts
/// leave the fact list for boxes of their own, so the layout can shrink them before the fact list scrolls (SPEC.md).
#[test]
fn facts_keep_their_layout_contracts() {
    for key in [
        "folder",
        "managed checkout of",
        "ssh destination",
        "destination",
    ] {
        assert_eq!(value(key, "v").shape(), Shape::OneLine, "{key}");
    }
    for key in ["URL", "permission flags"] {
        assert_eq!(value(key, "v").shape(), Shape::WrappedMono, "{key}");
    }
    for key in ["title", "host", "new session on host"] {
        assert_eq!(value(key, "v").shape(), Shape::Wrapped, "{key}");
    }
    assert_eq!(value("permissions", "Guarded").shape(), Shape::Chip);
    assert_eq!(Fact::Words("asserted YOLO", "yes").shape(), Shape::Chip);
    assert_eq!(
        Fact::CopyIdentity("session id", "full".into()).shape(),
        Shape::OneLine
    );

    let card = Card::of(
        &request("r", "host", create(command("agent --go", false, None))),
        TunnelOffer::NotOffered,
    );
    assert!(card.boxes.iter().all(|fact| matches!(fact, Fact::Box(..))));
    assert!(card.facts.iter().all(|fact| !matches!(fact, Fact::Box(..))));
    assert_eq!(card.boxes, vec![boxed("command", "agent --go")]);
    // A prompt is prose in the interface face; a command, a resume command and wrapper words are read character by
    // character in the monospace face (the card draws a box's face from its shape).
    for key in ["initial prompt", "prompt"] {
        assert_eq!(boxed(key, "v").shape(), Shape::Wrapped, "{key}");
    }
    for key in [
        "command",
        "resume command",
        "wrapper prefix",
        "agent supplied wrapper",
    ] {
        assert_eq!(boxed(key, "v").shape(), Shape::WrappedMono, "{key}");
    }
}

/// Labels are the app's words in sentence case; the requester's and the new session's redundant context is dropped,
/// and copy buttons name what they copy with the noun kept lowercase, so a card acting on another session never shows
/// two facts or two Copy buttons that say the same thing.
#[test]
fn labels_and_copy_names_follow_the_old_cards_words() {
    assert_eq!(
        Fact::CopyIdentity("session id", String::new()).label(),
        "Session ID"
    );
    assert_eq!(
        Fact::CopyIdentity("copied session id", String::new()).label(),
        "Copied session ID"
    );
    assert_eq!(value("ssh destination", "d").label(), "SSH destination");
    assert_eq!(value("new session on host", "h").label(), "On host");
    assert_eq!(value("URL", "u").label(), "URL");
    assert_eq!(
        Fact::Words("⚡ quick launch", "w").label(),
        "⚡ Quick launch"
    );
    assert_eq!(Fact::Words("resume command", "none").label(), "Resume");
    assert_eq!(
        boxed("resume command", "r").label(),
        "Resume command",
        "a box keeps every word"
    );
    assert_eq!(
        Fact::CopyIdentity("session id", String::new())
            .copy_name()
            .as_deref(),
        Some("Copy session ID")
    );
    assert_eq!(value("folder", "f").copy_name(), None);
}

/// Literal escape text, formatting characters and spaces used to share one visible spelling. Every target and source
/// reference stays a copyable identity carrying its complete raw reference, which the strict identity spelling then
/// draws distinctly; a target that came out as ordinary host text could pose as another session.
#[test]
fn session_references_stay_raw_copyable_identities() {
    for bare in [
        "peer<U+200B>",
        "peer\u{200b}",
        " peer ",
        "peer  id",
        reference("x").as_str(),
    ] {
        let id = reference(bare);
        let target = ApprovalSession {
            id: id.clone(),
            title: Some("a session title".into()),
            host_name: Some("a host".into()),
            launch_kind: None,
        };
        for action in [
            ApprovalAction::Stop {
                target: target.clone(),
            },
            ApprovalAction::Rename {
                target: target.clone(),
                title: "t".into(),
            },
            ApprovalAction::Delete {
                target: target.clone(),
                guard: DeleteGuard::Unconditional,
            },
            ApprovalAction::Restart {
                target: target.clone(),
                stop_if_running: false,
                launch: None,
            },
        ] {
            let identities: Vec<_> = facts(&action)
                .into_iter()
                .filter_map(|fact| match fact {
                    Fact::CopyIdentity(key, raw) => Some((key, raw)),
                    _ => None,
                })
                .collect();
            assert_eq!(
                identities,
                vec![("session id", id.clone())],
                "{}",
                kind(&action)
            );
        }
    }
}

/// The From line keeps host, title and ID apart, and keeps the complete raw reference for its hover, accessible name
/// and Copy ID; a session the helm does not know by title says so in the app's words.
#[test]
fn the_from_line_keeps_its_parts_apart_and_the_full_reference_raw() {
    let mut approval = request(
        "r",
        "build\u{202e}host",
        ApprovalAction::Stop {
            target: unknown("t"),
        },
    );
    approval.session = ApprovalSession {
        id: reference("1234567\u{200b}middle\u{202e}"),
        title: Some("“quoted” title".into()),
        host_name: None,
        launch_kind: None,
    };
    let from = Card::of(&approval, TunnelOffer::NotOffered).from;
    assert_eq!(from.host, HostText::from_host("build\u{202e}host"));
    assert_eq!(from.title, Some(HostText::from_host("“quoted” title")));
    assert_eq!(from.id, reference("1234567\u{200b}middle\u{202e}"));

    approval.session = unknown("not a reference");
    let from = Card::of(&approval, TunnelOffer::NotOffered).from;
    assert_eq!((from.title, from.id.as_str()), (None, "not a reference"));
}

// ---- Permission chips -----------------------------------------------------------------------------------------------

/// The chip's description says what Farhelm does not know: a command launch's YOLO claim is the launcher's unchecked
/// assertion, a saved agent mode can be treated as YOLO, and an unclassified launch predates launch kinds. Losing
/// these words would make a claim or an unknown read like Farhelm's verdict. The asserted chips' own words say
/// "Asserted" for the same reason.
#[test]
fn permission_chips_say_what_farhelm_did_not_check() {
    for asserted in [ChipKind::AssertedYolo, ChipKind::AssertedNotYolo] {
        assert!(asserted.description().contains("not checked by Farhelm"));
    }
    assert!(
        ChipKind::Unknown
            .description()
            .contains("before launch kinds")
    );
    assert_eq!(ChipKind::AssertedYolo.label(), "Asserted YOLO");
    assert_eq!(ChipKind::AssertedNotYolo.label(), "Asserted not YOLO");
}

/// A launch's chip comes from the session list's own shield, for every agent type and mode it offers: custom flags
/// stay YOLO, guarded carries the agent's own name for the mode, the flags stay an inspectable fact, and the chip warns
/// exactly when the session list draws a YOLO shield. Start and Resume cards agree.
#[test]
fn launch_chips_follow_the_session_lists_shield_for_every_mode() {
    for &harness in LaunchHarness::ALL {
        for permission in LaunchPermission::ALL {
            if !harness.offers_permission(permission) {
                continue;
            }
            let expected = match permission {
                LaunchPermission::Guarded => ChipKind::Guarded,
                LaunchPermission::Yolo | LaunchPermission::Custom => ChipKind::Yolo,
                LaunchPermission::Approve => ChipKind::Approve,
                LaunchPermission::SmartApprove => ChipKind::SmartApprove,
                LaunchPermission::Chat => ChipKind::Chat,
            };
            let launch = agent(
                LaunchSelection {
                    permissions: Some(permission),
                    // Only Custom carries flags: the helm refuses flags on any other mode.
                    permission_flags: (permission == LaunchPermission::Custom)
                        .then(|| vec!["--example-mode".into()]),
                    ..selection(harness)
                },
                &["agent"],
            );
            let restart = ApprovalAction::Restart {
                target: unknown("s"),
                stop_if_running: false,
                launch: Some(launch.clone()),
            };
            for facts in [facts(&create(launch.clone())), facts(&restart)] {
                let rows = chips(&facts);
                assert_eq!(rows.len(), 1, "{harness:?} {permission:?}");
                let row = rows[0];
                assert_eq!(row.kind, expected, "{harness:?} {permission:?}");
                assert_eq!(
                    Some(row.shield()),
                    crate::permission::permission(&launch, false)
                );
                assert_eq!(row.warning(), row.shield() == Shield::Yolo);
                assert_eq!(
                    row.guarded.is_some(),
                    permission == LaunchPermission::Guarded
                );
                assert_eq!(
                    facts.contains(&value("permission flags", "--example-mode")),
                    permission == LaunchPermission::Custom,
                    "custom flags stay an inspectable fact"
                );
            }
        }
    }
}

/// A saved launch with no recorded mode warns as YOLO. Guarded names the
/// agent's mode in one line.
#[test]
fn an_agent_launch_names_its_permission_in_one_line() {
    let row = |permissions| {
        chips(&facts(&create(agent(
            LaunchSelection {
                permissions,
                ..selection(LaunchHarness::Claude)
            },
            &["claude"],
        ))))
        .into_iter()
        .next()
        .cloned()
        .expect("a launch card has a Permissions row")
    };
    let guarded = row(Some(LaunchPermission::Guarded));
    assert_eq!(
        (
            guarded.kind,
            guarded.guarded,
            guarded.trust,
            guarded.warning()
        ),
        (
            ChipKind::Guarded,
            Some("Guarded · Claude auto mode"),
            None,
            false
        )
    );
    assert_eq!(guarded.description(), "Guarded · Claude auto mode");
    let unclassified = row(None);
    assert_eq!(
        (
            unclassified.kind,
            unclassified.guarded,
            unclassified.warning()
        ),
        (ChipKind::Yolo, None, true)
    );
    assert_eq!(unclassified.description(), ChipKind::Yolo.description());
}

/// Launch and Restart cards explain stored workspace trust on its own chip, and only when the launch records one, so
/// the user never has to read trust out of command flags and an unset choice makes no trust claim.
#[test]
fn launch_and_restart_cards_explain_stored_workspace_trust() {
    for &harness in LaunchHarness::ALL {
        let choices: &[Option<bool>] = if harness.offers_workspace_trust() {
            &[Some(true), Some(false), None]
        } else {
            &[None]
        };
        for &workspace_trust in choices {
            let launch = agent(
                LaunchSelection {
                    workspace_trust,
                    ..selection(harness)
                },
                &["agent"],
            );
            for action in [
                create(launch.clone()),
                ApprovalAction::Restart {
                    target: unknown("s"),
                    stop_if_running: false,
                    launch: Some(launch.clone()),
                },
            ] {
                let trust: Vec<_> = chips(&facts(&action))
                    .into_iter()
                    .map(|row| row.trust)
                    .collect();
                if harness.is_shell() {
                    assert!(trust.is_empty(), "shells have no permissions row");
                } else {
                    assert_eq!(trust, vec![workspace_trust], "{harness:?}");
                }
            }
        }
    }
    assert!(TRUSTED_WORKSPACE.starts_with("Enabled on the agent's Settings page."));
}

// ---- Launch cards ---------------------------------------------------------------------------------------------------

/// An approval for Bash or Zsh tells the user which shell would start and shows its full wrapped command.
/// A shell lacks an agent permission mode, so the same card must omit its permissions row.
#[test]
fn a_shell_launch_card_names_the_shell_and_its_full_command() {
    for (harness, program, label) in [
        (LaunchHarness::Bash, "bash", "Bash"),
        (LaunchHarness::Zsh, "zsh", "Zsh"),
    ] {
        let selection = LaunchSelection {
            wrapper_prefix: Some(vec!["env".into(), "PROFILE=work".into()]),
            ..selection(harness)
        };
        let placeholder = farhelm_proto::session_launch::FARHELM_ARGS_PLACEHOLDER;
        let launch = agent(selection, &["env", "PROFILE=work", program, placeholder]);
        let facts = facts(&create(launch));
        assert!(facts.contains(&value("shell", label)));
        assert!(facts.contains(&boxed("command", &format!("env PROFILE=work {program}"))));
        assert!(chips(&facts).is_empty());
    }
}

/// A command launch's card shows the command and the resume command in full and calls the YOLO assertion the agent's
/// unchecked claim, never the app's verdict. Why: a later plain Restart runs the resume command without another YOLO
/// check, on the grounds that the user approved it here.
#[test]
fn a_command_launch_card_shows_both_commands_and_its_claim() {
    let facts = facts(&create(command(
        "agent --go",
        false,
        Some("agent --resume {conversation}"),
    )));
    assert!(facts.contains(&boxed("command", "agent --go")));
    assert!(facts.contains(&boxed("resume command", "agent --resume {conversation}")));
    let rows = chips(&facts);
    assert_eq!(
        (rows[0].kind, rows[0].guarded, rows[0].trust),
        (ChipKind::AssertedNotYolo, None, None)
    );
    assert!(facts.contains(&Fact::Words("title", "generated")));
    let yolo = create(command("agent --yolo", true, None));
    let yolo_facts = super::model::facts(&yolo);
    let rows = chips(&yolo_facts);
    assert_eq!(
        (rows[0].kind, rows[0].warning()),
        (ChipKind::AssertedYolo, true)
    );
    assert!(yolo_facts.contains(&Fact::Words("resume command", "none")));
}

/// A wrapped launch shows the agent's exact prefix, labelled as the agent's, beside the full start and resume commands;
/// only the raw prefix keeps its shell spelling, and the card needs the user's own Allow with no Always allow.
#[test]
fn an_agent_wrapper_card_names_the_agents_prefix() {
    let words = ["with-endpoint", "\"$PROFILE\"", "--"];
    let placeholder = farhelm_proto::session_launch::FARHELM_ARGS_PLACEHOLDER;
    let launch = SessionLaunch::Agent {
        custom_reporting: None,
        selection: LaunchSelection {
            wrapper_prefix: Some(words.iter().map(|word| (*word).to_owned()).collect()),
            ..selection(LaunchHarness::Claude)
        },
        start: [&words[..], &["claude", placeholder]]
            .concat()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        resume: Some(
            [
                &words[..],
                &["claude", "--resume", "two words", placeholder],
            ]
            .concat()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        ),
    };
    let action = ApprovalAction::LaunchWithAgentWrapper {
        verb: LaunchVerb::Create,
        host_name: "host".into(),
        cwd: "/work".into(),
        repo: None,
        title: None,
        launch,
        source: None,
        prompt: Some("first".into()),
    };
    let card = Card::of(&request("r", "host", action), TunnelOffer::NotOffered);
    assert_eq!(card.kind, "launch_with_agent_wrapper");
    assert_eq!(
        card.boxes,
        vec![
            boxed("agent supplied wrapper", "with-endpoint \"$PROFILE\" --"),
            boxed("command", "with-endpoint \"$PROFILE\" -- claude"),
            boxed(
                "resume command",
                "with-endpoint \"$PROFILE\" -- claude --resume 'two words'"
            ),
            boxed("initial prompt", "first"),
        ]
    );
    assert_eq!(
        offered(&card),
        vec![ApprovalAnswer::Allow, ApprovalAnswer::Deny]
    );
    assert!(card.notes[0].starts_with("The agent supplied this wrapper."));
}

/// An agent's environment-launch card names the recipe, where the new machine goes (a Tensorlake sandbox, or a
/// container on a named host), the consequence the launcher's own footer states (billing for Tensorlake), the
/// destination and the launch; a clone also names the copied session. Why: this card is the creation confirmation,
/// and the user's Allow is only informed if it says a machine will be created and what it costs.
#[test]
fn an_environment_launch_card_names_the_new_machine_and_its_cost() {
    let launch = command("agent --go", false, None);
    let sandbox = ApprovalAction::LaunchEnvironment {
        verb: LaunchVerb::Create,
        recipe: "dev".into(),
        place: Place::TensorlakeAccount,
        place_name: None,
        cwd: "~".into(),
        repo: None,
        title: None,
        launch: launch.clone(),
        source: None,
        prompt: Some("start here".into()),
    };
    let rows = facts(&sandbox);
    assert!(rows.contains(&value("recipe", "dev")));
    assert!(rows.contains(&Fact::Words("creates", "a new Tensorlake sandbox")));
    assert!(
        rows.iter()
            .any(|fact| matches!(fact, Fact::Words("consequence", text) if text.contains("bills")))
    );
    assert!(rows.contains(&value("folder", "~")));
    assert!(rows.contains(&boxed("command", "agent --go")));
    assert!(rows.contains(&boxed("initial prompt", "start here")));
    assert_eq!(
        summary(&sandbox),
        "Start a session in a new sandbox or container"
    );
    let card = Card::of(&request("r", "host", sandbox), TunnelOffer::NotOffered);
    assert_eq!(
        offered(&card),
        vec![ApprovalAnswer::Allow, ApprovalAnswer::Deny],
        "a new machine always asks"
    );

    let container = ApprovalAction::LaunchEnvironment {
        verb: LaunchVerb::Clone,
        recipe: "dev".into(),
        place: Place::PodmanHost(3),
        place_name: Some("build-box".into()),
        cwd: String::new(),
        repo: Some("acme/web".into()),
        title: Some("copy".into()),
        launch,
        source: Some(ApprovalSession {
            id: "entry:source".into(),
            title: Some("original".into()),
            host_name: Some("dev container".into()),
            launch_kind: None,
        }),
        prompt: None,
    };
    let rows = facts(&container);
    assert!(rows.contains(&value("new container on host", "build-box")));
    assert!(
        rows.iter().any(
            |fact| matches!(fact, Fact::Words("consequence", text) if !text.contains("bills"))
        )
    );
    assert!(rows.contains(&value("managed checkout of", "GitHub: acme/web")));
    assert!(
        !rows
            .iter()
            .any(|fact| matches!(fact, Fact::Value("folder", _))),
        "a checkout's folder does not exist yet"
    );
    assert!(rows.contains(&Fact::CopyIdentity(
        "copied session id",
        "entry:source".into()
    )));
    assert_eq!(
        summary(&container),
        "Copy a session into a new sandbox or container"
    );
    assert_eq!(kind(&container), "launch_environment");
}

// ---- Lifecycle cards ------------------------------------------------------------------------------------------------

/// A restart card shows the resume command the restart runs, for a legacy launch too, and never the start command it
/// does not run; a restart whose launch the helm does not know says so. Why: a plain restart skips the YOLO rule
/// because it re-runs what the user approved, so the card is where the user sees what that is.
#[test]
fn a_restart_card_shows_the_resume_command_only() {
    let action = ApprovalAction::Restart {
        target: unknown("s"),
        stop_if_running: true,
        launch: Some(SessionLaunch::Legacy {
            invocation: "agent --start".into(),
            agent_kind: AgentKind::Generic,
            resume_template: Some(vec!["agent".into(), "--resume".into()]),
        }),
    };
    let rows = facts(&action);
    assert!(rows.contains(&boxed("resume command", "agent --resume")));
    assert!(
        !rows
            .iter()
            .any(|fact| matches!(fact, Fact::Box("command", _)))
    );
    assert!(rows.contains(&Fact::Words("if it is working", "stop it first")));
    let unknown_launch = ApprovalAction::Restart {
        target: unknown("s"),
        stop_if_running: false,
        launch: None,
    };
    let rows = facts(&unknown_launch);
    assert!(rows.contains(&Fact::Words("resume command", "unknown to this helm")));
    assert!(rows.contains(&Fact::Words("if it is working", "refuse")));
    assert!(chips(&rows).is_empty());
}

/// Stop names the program in the heading alone. Delete names the scope of
/// the guard, so a tabs-only warning never covers a restarted main program.
#[test]
fn stop_and_delete_cards_describe_the_target_program() {
    use ApprovalLaunchKind::{Agent, Command, Legacy, Shell, Unknown};

    for (launch_kind, program) in [
        (Some(Agent), "agent"),
        (Some(Shell), "shell"),
        (Some(Command), "main program"),
        (Some(Legacy), "main program"),
        (Some(Unknown), "main program"),
        (None, "main program"),
    ] {
        let target = ApprovalSession {
            launch_kind,
            ..unknown("t")
        };
        let stop = ApprovalAction::Stop {
            target: target.clone(),
        };
        assert_eq!(
            summary(&stop),
            format!("Stop a session's {program} and close its terminal tabs")
        );
        assert_eq!(
            facts(&stop),
            [
                Fact::Words("session", "title unknown"),
                Fact::CopyIdentity("session id", "t".into()),
                Fact::Words("session's host", "unknown to this helm"),
            ]
        );

        for (guard, expected) in [
            (
                DeleteGuard::NothingAlive,
                format!("nothing. Refuse if the {program} or any terminal tab is alive"),
            ),
            (
                DeleteGuard::AgentEnded,
                format!("terminal tabs. Refuse if the {program} is running again"),
            ),
            (
                DeleteGuard::Unconditional,
                format!("the {program} and all terminal tabs"),
            ),
        ] {
            let delete = ApprovalAction::Delete {
                target: target.clone(),
                guard,
            };
            assert_eq!(kind(&delete), "delete");
            let shown = facts(&delete).into_iter().find_map(|fact| match fact {
                Fact::Words("what deletion stops", words) => Some(words),
                _ => None,
            });
            assert_eq!(shown, Some(expected.as_str()));
        }
    }
}

/// The dormant running-agent prompt card shows the whole message as one boxed host value, the target's shield by the
/// session list's rules (observed bypass included), whether the window has it open, and how delivery can mix with
/// typing; a target's permission is evidence, never a relaunch command.
#[test]
fn a_prompt_card_shows_the_message_the_targets_shield_and_its_caveats() {
    const PROMPT: &str = "<script>first</script>\nsecond\t\u{202e}third";
    let action = |screen_yolo| ApprovalAction::Prompt {
        target: ApprovalSession {
            id: "target".into(),
            title: Some("title".into()),
            host_name: Some("host".into()),
            launch_kind: None,
        },
        launch: SessionLaunch::Command(CommandLaunch {
            custom_agent: None,
            command: "secret command".into(),
            yolo: false,
            agent: Some(LaunchHarness::Claude),
            resume: None,
        }),
        screen_yolo,
        attached: true,
        prompt: PROMPT.into(),
    };
    let rows = facts(&action(false));
    assert_eq!(
        summary(&action(false)),
        "Send a prompt to a session's agent"
    );
    assert!(rows.contains(&boxed("prompt", PROMPT)));
    assert_eq!(chips(&rows)[0].kind, ChipKind::AssertedNotYolo);
    assert_eq!(
        (chips(&rows)[0].guarded, chips(&rows)[0].trust),
        (None, None)
    );
    assert!(rows.contains(&Fact::Words("client", "this session is open in the window")));
    assert!(rows.iter().any(
        |fact| matches!(fact, Fact::Words("delivery", words) if words.contains("interleave"))
    ));
    assert!(
        !rows
            .iter()
            .any(|fact| matches!(fact, Fact::Box("command", _)))
    );
    // Claude has a status line that shows a mode without checks; seeing it makes the shield YOLO.
    assert_eq!(chips(&facts(&action(true)))[0].kind, ChipKind::Yolo);
}

// ---- Template cards -------------------------------------------------------------------------------------------------

/// Approving a stored shell template covers its actual fields, including permissions the shell cannot use.
#[test]
fn a_shell_template_card_names_its_incompatible_stored_permissions() {
    let action = ApprovalAction::TemplateWrite {
        name: "shell".into(),
        replaces_existing: false,
        fields: TemplateFields {
            agent: Some(LaunchHarness::Bash),
            permissions: Some(Some(LaunchPermission::Custom)),
            permission_flags: Some(vec!["--profile".into(), "test only".into()]),
            ..TemplateFields::default()
        },
        host_name: None,
    };
    let rows = facts(&action);
    assert!(rows.contains(&value("permissions", "Custom")));
    assert!(rows.contains(&value("permission flags", "--profile 'test only'")));
}

/// A template write shows the whole resulting template, custom flags as words and command text the agent templates
/// listing withholds included. Why: an agent can store flags or a command that later runs on any host, possibly by
/// quick launch with no further question, and the approval covers only what the card showed.
#[test]
fn a_template_card_shows_every_field_it_sets() {
    let fields = TemplateFields {
        agent: Some(LaunchHarness::Claude),
        permissions: Some(Some(LaunchPermission::Custom)),
        permission_flags: Some(vec!["--settings".into(), "{\"hooks\": {}}".into()]),
        command: Some("run-me".into()),
        resume_command: Some(None),
        yolo: Some(true),
        wrapper_prefix: Some(None),
        ..TemplateFields::default()
    };
    let action = ApprovalAction::TemplateWrite {
        name: "t".into(),
        replaces_existing: false,
        fields,
        host_name: None,
    };
    let rows = facts(&action);
    assert!(rows.contains(&value("permission flags", "--settings '{\"hooks\": {}}'")));
    assert!(rows.contains(&value(
        "permissions",
        &crate::templates::summary::permission_label(
            Some(LaunchHarness::Claude),
            Some(LaunchPermission::Custom)
        )
    )));
    assert!(rows.contains(&boxed("command", "run-me")));
    assert!(rows.contains(&Fact::Words("resume command", "none")));
    assert!(rows.contains(&Fact::Words("wrapper prefix", "none")));
    assert!(rows.contains(&Fact::Words("asserted YOLO", "yes")));
    assert_eq!(summary(&action), "Create a launch template");
}

/// The approval distinguishes which registered agent a template will run and an edit that clears that choice.
#[test]
fn a_template_card_names_the_custom_agent_registration() {
    let card = |custom_agent| ApprovalAction::TemplateWrite {
        name: "review".into(),
        replaces_existing: true,
        fields: TemplateFields {
            agent: Some(LaunchHarness::Custom),
            custom_agent,
            ..TemplateFields::default()
        },
        host_name: None,
    };
    assert!(
        facts(&card(Some(Some("acme-code".into())))).contains(&value("custom agent", "acme-code"))
    );
    assert!(facts(&card(Some(None))).contains(&Fact::Words("custom agent", "none")));
}

/// Historical container fields describe a retired choice, not a runnable option, and an explicit reset stays distinct
/// from a field the template leaves alone.
#[test]
fn a_template_card_explains_retired_container_fields_and_their_removal() {
    let mut fields: TemplateFields = serde_json::from_value(serde_json::json!({
        "container": {"engine": "podman", "image_file": "legacy.tar", "api_proxy_port": 42000}
    }))
    .expect("a historical template decodes");
    let card = |fields| {
        facts(&ApprovalAction::TemplateWrite {
            name: "historical".into(),
            replaces_existing: true,
            fields,
            host_name: None,
        })
    };
    assert!(card(fields.clone()).contains(&Fact::Words(
        "retired container setting",
        farhelm_proto::RETIRED_CONTAINER_REASON
    )));
    fields.container = Some(None);
    assert!(card(fields.clone()).contains(&Fact::Words(
        "retired container setting",
        "remove the retired choice"
    )));
    fields.container = None;
    assert!(
        !card(fields)
            .iter()
            .any(|fact| matches!(fact, Fact::Words("retired container setting", _)))
    );
}

/// A template card names its host by name when the helm knows it and by install identity otherwise, its session name
/// with its base or fixed policy (base when older templates leave it out), and its folder or managed checkout.
#[test]
fn a_template_card_names_its_host_name_policy_and_destination() {
    let fields = TemplateFields {
        host: Some("install-1".into()),
        ..TemplateFields::default()
    };
    let named = facts(&ApprovalAction::TemplateDelete {
        name: "t".into(),
        fields: fields.clone(),
        host_name: Some("box".into()),
    });
    assert!(named.contains(&value("host", "box")));
    let unnamed = facts(&ApprovalAction::TemplateDelete {
        name: "t".into(),
        fields,
        host_name: None,
    });
    assert!(unnamed.contains(&Fact::Identity("host install", "install-1".into())));

    for (name_kind, words) in [
        (None, "base name"),
        (Some(TemplateNameKind::Base), "base name"),
        (Some(TemplateNameKind::Fixed), "fixed name"),
    ] {
        let fields = TemplateFields {
            name: Some("review".into()),
            name_kind,
            destination: Some(TemplateDestination::Folder("/src".into())),
            ..TemplateFields::default()
        };
        let rows = facts(&ApprovalAction::TemplateWrite {
            name: "t".into(),
            replaces_existing: true,
            fields,
            host_name: None,
        });
        assert!(rows.contains(&value("session title", "review")));
        assert!(rows.contains(&Fact::Words("session name kind", words)));
        assert!(rows.contains(&value("folder", "/src")));
    }
}

/// A shortcut card shows every assignment and removal, the old key the edited template loses and the template a moved
/// ⚡ or key leaves, beside the ordinary template facts.
#[test]
fn a_shortcut_card_shows_every_move() {
    let action = ApprovalAction::TemplateWriteWithShortcuts {
        name: "new".into(),
        replaces_existing: true,
        fields: TemplateFields {
            destination: Some(TemplateDestination::Github("o/r".into())),
            ..TemplateFields::default()
        },
        host_name: None,
        shortcuts: TemplateShortcutCard {
            quick_launch: Some(QuickLaunchCard::Set {
                moved_from: Some("old".into()),
            }),
            key: Some(TemplateKeyCard::Set {
                key: "1".into(),
                replaced_key: Some("2".into()),
                moved_from: Some("old".into()),
            }),
        },
    };
    let rows = facts(&action);
    assert_eq!(kind(&action), "template_write_with_shortcuts");
    assert_eq!(
        summary(&action),
        "Change a launch template and its shortcuts"
    );
    assert_eq!(
        rows,
        vec![
            value("template", "new"),
            value("managed checkout of", "GitHub: o/r"),
            Fact::Words("⚡ quick launch", "mark this template"),
            value("⚡ moves from", "old"),
            value("number key removed", "2"),
            value("number key assigned", "1"),
            value("number key moves from", "old"),
        ]
    );
    let clear = ApprovalAction::TemplateWriteWithShortcuts {
        name: "new".into(),
        replaces_existing: false,
        fields: TemplateFields::default(),
        host_name: None,
        shortcuts: TemplateShortcutCard {
            quick_launch: Some(QuickLaunchCard::Clear),
            key: Some(TemplateKeyCard::Clear { key: "1".into() }),
        },
    };
    let rows = facts(&clear);
    assert!(rows.contains(&Fact::Words("⚡ quick launch", "clear this template")));
    assert!(rows.contains(&value("number key removed", "1")));
    assert_eq!(
        summary(&clear),
        "Create a launch template and assign its shortcuts"
    );
}

/// A template write that leaves a wrapper prefix always needs the user's own Allow: a later launch counts that prefix
/// as user-configured, so no host setting or Always allow may stand in for this card (SPEC.md).
#[test]
fn a_template_write_leaving_a_prefix_offers_allow_or_deny_only() {
    let fields = TemplateFields {
        wrapper_prefix: Some(Some(vec!["wrap".into()])),
        ..TemplateFields::default()
    };
    let action = ApprovalAction::TemplateWrite {
        name: "t".into(),
        replaces_existing: false,
        fields,
        host_name: None,
    };
    let card = Card::of(&request("r", "host", action), TunnelOffer::NotOffered);
    assert!(card.boxes.contains(&boxed("wrapper prefix", "wrap")));
    assert_eq!(
        offered(&card),
        vec![ApprovalAnswer::Allow, ApprovalAnswer::Deny]
    );
}

// ---- Host cards -----------------------------------------------------------------------------------------------------

/// A host edit is approval for the changes shown, not for every setting on the host: old and new values stay apart,
/// names stay host text, and icon and color words come from the app's own vocabulary.
#[test]
fn a_host_edit_card_shows_each_changed_field() {
    let action = ApprovalAction::HostEdit {
        host_name: "<b>builder</b>\u{202e}".into(),
        destination: Some("operator@builder\t".into()),
        alias: Some(HostFieldChange {
            old: None,
            new: Some("<b>new alias</b>\u{202e}".into()),
        }),
        icon: Some(HostFieldChange {
            old: HostIcon::Cloud,
            new: HostIcon::Rocket,
        }),
        color: Some(HostFieldChange {
            old: HostColor::Default,
            new: HostColor::Teal,
        }),
    };
    assert_eq!(kind(&action), "host_edit");
    assert_eq!(
        facts(&action),
        vec![
            value("host", "<b>builder</b>\u{202e}"),
            value("destination", "operator@builder\t"),
            Fact::Words("alias now", "none"),
            value("new alias", "<b>new alias</b>\u{202e}"),
            Fact::Words("icon now", "cloud"),
            Fact::Words("new icon", "rocket"),
            Fact::Words("color now", "default"),
            Fact::Words("new color", "teal"),
        ]
    );
}

/// Clearing this machine's alias reads as a removal, and fields the request leaves alone stay absent; otherwise a card
/// could imply broader approval or disguise a clear as an empty agent-supplied name.
#[test]
fn a_host_edit_card_shows_an_alias_removal_and_omits_untouched_fields() {
    let action = ApprovalAction::HostEdit {
        host_name: "local alias".into(),
        destination: None,
        alias: Some(HostFieldChange {
            old: Some("local alias".into()),
            new: None,
        }),
        icon: None,
        color: None,
    };
    assert_eq!(
        facts(&action),
        vec![
            value("host", "local alias"),
            Fact::Words("destination", "this machine"),
            value("alias now", "local alias"),
            Fact::Words("new alias", "none"),
        ]
    );
}

/// Host add, edit and remove always need the user's own Allow (the requesting host's command setting never covers
/// them), say so on the card, and a remove repeats the GUI remove dialog's words; a host add's Allow opens the add
/// dialog.
#[test]
fn host_cards_offer_allow_or_deny_only_and_say_why() {
    let add = Card::of(
        &request(
            "r",
            "host",
            ApprovalAction::HostAdd {
                destination: "ops@new.example".into(),
                alias: Some("new".into()),
            },
        ),
        TunnelOffer::NotOffered,
    );
    assert_eq!(
        add.facts,
        vec![
            value("ssh destination", "ops@new.example"),
            value("alias", "new")
        ]
    );
    assert_eq!(add.answers[0].label, AnswerLabel::Words("Open add dialog"));
    assert_eq!(
        add.answers[0].help,
        AnswerHelp::Words("Open add dialog: review the host before adding it")
    );
    assert_eq!(
        offered(&add),
        vec![ApprovalAnswer::Allow, ApprovalAnswer::Deny]
    );
    assert_eq!(add.notes.len(), 1);
    assert!(add.notes[0].starts_with("Each host change needs your approval"));

    let remove = Card::of(
        &request(
            "r",
            "host",
            ApprovalAction::HostRemove {
                host_name: "old".into(),
                destination: "ops@old.example".into(),
            },
        ),
        TunnelOffer::NotOffered,
    );
    assert_eq!(
        remove.facts,
        vec![
            value("host", "old"),
            value("destination", "ops@old.example")
        ]
    );
    assert_eq!(
        offered(&remove),
        vec![ApprovalAnswer::Allow, ApprovalAnswer::Deny]
    );
    assert_eq!(remove.notes.len(), 2);
    assert!(remove.notes[0].starts_with("Farhelm will forget this host."));
}

// ---- Link cards -----------------------------------------------------------------------------------------------------

/// A link card names the site the browser will contact, in ASCII, above the full URL. Why: allowing a link is a trust
/// decision about a site, and credentials before an `@`, a backslash ending the host or a look-alike Unicode host make
/// the raw URL read as a different site than the one it opens.
#[test]
fn a_link_card_names_the_site_it_opens() {
    let site_of = |url: &str| {
        facts(&ApprovalAction::OpenUrl {
            url: url.into(),
            local_network: false,
            preapproved: false,
            tunnel_available: false,
        })
        .into_iter()
        .find_map(|fact| match fact {
            Fact::Identity("site", site) => Some(site),
            _ => None,
        })
    };
    assert_eq!(
        site_of("https://accounts.google.com@evil.example/").as_deref(),
        Some("evil.example")
    );
    assert_eq!(
        site_of("https://evil.example\\@google.com/").as_deref(),
        Some("evil.example")
    );
    assert_eq!(
        site_of("https://\u{0430}pple.com/").as_deref(),
        Some("xn--pple-43d.com")
    );
    assert_eq!(
        site_of("https://example.com/a").as_deref(),
        Some("example.com")
    );
    assert_eq!(
        site_of("not a url"),
        None,
        "an invalid URL names no site, and still shows whole"
    );
    assert!(
        facts(&ApprovalAction::OpenUrl {
            url: "not a url".into(),
            local_network: false,
            preapproved: false,
            tunnel_available: false
        })
        .contains(&value("URL", "not a url"))
    );
}

/// A link card's answers follow SPEC.md: Allow, Always allow (for that category only) and Deny; Open and Dismiss for a
/// preapproved link; and on an eligible loopback card in the Mac app, Tunnel and open first, with its note naming the
/// port and host, and no Always allow. A local-network link explains its separate permission unless the tunnel is
/// offered. Where the app cannot offer the tunnel, the same card falls back to its ordinary answers.
#[test]
fn link_cards_offer_the_answers_their_kind_and_platform_allow() {
    let link = |url: &str, local_network, preapproved, tunnel_available| {
        request(
            "r",
            "remote\u{202e}",
            ApprovalAction::OpenUrl {
                url: url.into(),
                local_network,
                preapproved,
                tunnel_available,
            },
        )
    };

    let public = Card::of(
        &link("https://example.com/", false, false, false),
        TunnelOffer::Offered,
    );
    assert_eq!(
        offered(&public),
        vec![
            ApprovalAnswer::Allow,
            ApprovalAnswer::AlwaysAllow,
            ApprovalAnswer::Deny
        ]
    );
    assert_eq!(
        public.answers[1].help,
        AnswerHelp::Words("Always allow: stop asking about public links from this host")
    );
    assert_eq!(public.lead_note, None);

    let local = Card::of(
        &link("http://printer.local/", true, false, false),
        TunnelOffer::NotOffered,
    );
    assert_eq!(local.lead_note, Some(LeadNote::LocalNetwork));
    assert!(LeadNote::LOCAL_NETWORK.contains("needs its own host permission"));
    assert_eq!(
        local.answers[1].help,
        AnswerHelp::Words("Always allow: stop asking about local-network links from this host")
    );

    let preapproved = Card::of(
        &link("https://example.com/", false, true, false),
        TunnelOffer::NotOffered,
    );
    assert_eq!(
        offered(&preapproved),
        vec![ApprovalAnswer::Allow, ApprovalAnswer::Deny]
    );
    assert_eq!(preapproved.answers[0].label, AnswerLabel::Words("Open"));
    assert_eq!(preapproved.answers[1].label, AnswerLabel::Words("Dismiss"));
    assert_eq!(preapproved.answers[1].help, AnswerHelp::Words(DENY_HELP));

    let loopback = link("http://localhost:8123/app", true, false, true);
    let tunnel = Card::of(&loopback, TunnelOffer::Offered);
    assert_eq!(
        offered(&tunnel),
        vec![
            ApprovalAnswer::TunnelAndOpen,
            ApprovalAnswer::Allow,
            ApprovalAnswer::Deny
        ]
    );
    assert_eq!(
        tunnel.answers[0],
        Answer {
            answer: ApprovalAnswer::TunnelAndOpen,
            label: AnswerLabel::Words("Tunnel and open"),
            help: AnswerHelp::Words(TUNNEL_HELP),
            primary: true,
        }
    );
    assert_eq!(
        (tunnel.answers[1].label.clone(), tunnel.answers[1].primary),
        (AnswerLabel::Words("Open"), false)
    );
    assert_eq!(
        tunnel.lead_note,
        Some(LeadNote::Tunnel {
            port: 8123,
            host: HostText::from_host("remote\u{202e}")
        })
    );
    let default_port = Card::of(
        &link("http://localhost/", true, false, true),
        TunnelOffer::Offered,
    );
    assert_eq!(
        default_port.lead_note,
        Some(LeadNote::Tunnel {
            port: 80,
            host: HostText::from_host("remote\u{202e}")
        })
    );

    let no_tunnel = Card::of(&loopback, TunnelOffer::NotOffered);
    assert_eq!(
        offered(&no_tunnel),
        vec![
            ApprovalAnswer::Allow,
            ApprovalAnswer::AlwaysAllow,
            ApprovalAnswer::Deny
        ]
    );
    assert_eq!(no_tunnel.lead_note, Some(LeadNote::LocalNetwork));
}

// ---- Answers on session cards ---------------------------------------------------------------------------------------

/// The narrow "Always allow on <host> only" appears only when the helm marks the request eligible, before the
/// fleet-wide choice, which stays; never on a card that needs a direct Allow, even with an erroneous offer. Both name
/// the requesting host as host text, and the narrow one's help names it twice and says other hosts still ask.
#[test]
fn session_cards_offer_the_own_host_grant_only_when_eligible() {
    let host = "<builder>\u{202e}";
    let target = unknown(&reference("target"));
    let launch = SessionLaunch::plain_command("matrix command");
    for (action, eligible, expected) in [
        (
            ApprovalAction::Stop {
                target: target.clone(),
            },
            true,
            true,
        ),
        (
            ApprovalAction::Stop {
                target: target.clone(),
            },
            false,
            false,
        ),
        (
            ApprovalAction::LaunchWithPrompt {
                verb: LaunchVerb::Create,
                host_name: "target".into(),
                cwd: "/work".into(),
                repo: None,
                title: None,
                launch: launch.clone(),
                source: None,
                prompt: "first message".into(),
            },
            true,
            true,
        ),
        (
            ApprovalAction::LaunchWithAgentWrapper {
                verb: LaunchVerb::Clone,
                host_name: "target".into(),
                cwd: "/work".into(),
                repo: None,
                title: None,
                launch,
                source: Some(target.clone()),
                prompt: None,
            },
            true,
            false,
        ),
        (
            ApprovalAction::HostRemove {
                host_name: "target".into(),
                destination: "user@target".into(),
            },
            true,
            false,
        ),
        (
            ApprovalAction::TemplateWrite {
                name: "t".into(),
                replaces_existing: false,
                fields: TemplateFields::default(),
                host_name: None,
            },
            false,
            false,
        ),
    ] {
        let kind = kind(&action);
        let mut approval = request("card", host, action);
        approval.own_host_session_action = eligible;
        let card = Card::of(&approval, TunnelOffer::Offered);
        let answers = offered(&card);
        let own = answers
            .iter()
            .position(|answer| *answer == ApprovalAnswer::AlwaysAllowOwnHost);
        assert_eq!(own.is_some(), expected, "{kind}: {answers:?}");
        if let Some(own) = own {
            let fleet = answers
                .iter()
                .position(|answer| *answer == ApprovalAnswer::AlwaysAllow);
            assert_eq!(
                fleet,
                Some(own + 1),
                "the narrow choice comes just before the fleet-wide one"
            );
            assert_eq!(
                card.answers[own].label,
                AnswerLabel::OnHostOnly(HostText::from_host(host))
            );
            assert_eq!(
                card.answers[own].help,
                AnswerHelp::OwnHost(HostText::from_host(host))
            );
            assert_eq!(
                card.answers[own + 1].label,
                AnswerLabel::FromHost(HostText::from_host(host))
            );
            assert_eq!(
                card.answers[own + 1].help,
                AnswerHelp::Words(
                    "Always allow: stop asking about farhelm commands from this host"
                )
            );
        }
        assert_eq!(
            answers.last(),
            Some(&ApprovalAnswer::Deny),
            "{kind}: Deny is always last"
        );
    }
}

/// Every card's answers name its approval id, never either session's reference, and its first Allow-like answer is the
/// primary one with the Allow help; Deny carries its own help. Ported from the old app's check that answering sent the
/// random approval id rather than a session reference.
#[test]
fn a_card_answers_under_its_approval_id_with_the_old_help() {
    let approval = request(
        "a1b2-random",
        "host",
        ApprovalAction::Stop {
            target: unknown(&reference("peer")),
        },
    );
    let card = Card::of(&approval, TunnelOffer::NotOffered);
    assert_eq!(card.id, "a1b2-random");
    assert_ne!(card.id, card.from.id);
    assert_eq!(
        card.heading,
        "Stop a session's main program and close its terminal tabs"
    );
    assert_eq!(
        card.answers[0],
        Answer {
            answer: ApprovalAnswer::Allow,
            label: AnswerLabel::Words("Allow"),
            help: AnswerHelp::Words(ALLOW_HELP),
            primary: true
        }
    );
    let deny = card.answers.last().expect("a card always offers Deny");
    assert_eq!(
        (deny.label.clone(), deny.help.clone(), deny.primary),
        (
            AnswerLabel::Words("Deny"),
            AnswerHelp::Words(DENY_HELP),
            false
        )
    );
}

// ---- Whole cards ----------------------------------------------------------------------------------------------------

/// The answers a session card offers when the helm marks nothing eligible: Allow, the fleet-wide Always allow, Deny.
const ORDINARY: [ApprovalAnswer; 3] = [
    ApprovalAnswer::Allow,
    ApprovalAnswer::AlwaysAllow,
    ApprovalAnswer::Deny,
];

/// The answers a card needing the user's own Allow offers.
const DIRECT: [ApprovalAnswer; 2] = [ApprovalAnswer::Allow, ApprovalAnswer::Deny];

/// One expected card: the request, then its heading, facts, boxes and answers.
type WholeCard = (
    ApprovalAction,
    &'static str,
    Vec<Fact>,
    Vec<Fact>,
    &'static [ApprovalAnswer],
);

/// Every kind, card by card: the heading in the old app's words, every fact and box in order, the notes, and the
/// answers. Why: the port keeps the old wording exactly, and a card that dropped or reordered a fact, a note, or a
/// heading reworded by accident, would still pass a test that only checks that facts exist. The shortcut card's
/// moves are pinned whole by their own test, so its case here is the plain template write.
#[test]
fn every_kind_shows_exactly_the_old_cards_content() {
    let target = ApprovalSession {
        id: reference("target"),
        title: Some("target title".into()),
        host_name: Some("box".into()),
        launch_kind: None,
    };
    let anonymous = unknown(&reference("anon"));
    let codex = agent(
        LaunchSelection {
            model: Some("gpt-test".into()),
            permissions: Some(LaunchPermission::Guarded),
            ..selection(LaunchHarness::Codex)
        },
        &["codex"],
    );
    let declared = SessionLaunch::Command(CommandLaunch {
        custom_agent: None,
        command: "agent".into(),
        yolo: false,
        agent: Some(LaunchHarness::Claude),
        resume: None,
    });
    let target_facts = |extra: Vec<Fact>| {
        let mut facts = vec![
            value("session", "target title"),
            Fact::CopyIdentity("session id", reference("target")),
            value("session's host", "box"),
        ];
        facts.extend(extra);
        facts
    };
    let chip_of = |launch: &SessionLaunch| {
        Fact::Permission(chips(&facts(&create(launch.clone())))[0].clone())
    };
    let guarded_chips = || chip_of(&codex);
    let placeholder = farhelm_proto::session_launch::FARHELM_ARGS_PLACEHOLDER;
    let wrapped = SessionLaunch::Agent {
        custom_reporting: None,
        selection: LaunchSelection {
            wrapper_prefix: Some(vec!["wrap".into(), "--".into()]),
            ..selection(LaunchHarness::Claude)
        },
        start: vec![
            "wrap".into(),
            "--".into(),
            "claude".into(),
            placeholder.into(),
        ],
        resume: None,
    };
    let plain = command("agent --go", false, None);
    let cases: Vec<WholeCard> = vec![
        (
            ApprovalAction::Launch {
                verb: LaunchVerb::Spawn,
                host_name: "box".into(),
                cwd: "/w".into(),
                repo: None,
                title: Some("child".into()),
                launch: codex.clone(),
                source: None,
            },
            "Start a new session",
            vec![
                value("new session on host", "box"),
                value("folder", "/w"),
                value("title", "child"),
                guarded_chips(),
                value("agent", "codex"),
                value("model", "gpt-test"),
                Fact::Words("effort", "agent's default"),
                Fact::Words("resume command", "none"),
            ],
            vec![boxed("command", "codex")],
            &ORDINARY,
        ),
        (
            ApprovalAction::LaunchWithPrompt {
                verb: LaunchVerb::SpawnInherited,
                host_name: "box".into(),
                cwd: "/w/web-2".into(),
                repo: Some("acme/web".into()),
                title: None,
                launch: declared.clone(),
                source: None,
                prompt: "first".into(),
            },
            "Start a new session running the asking session's own launch",
            vec![
                value("new session on host", "box"),
                value("managed checkout of", "GitHub: acme/web"),
                value("folder", "/w/web-2"),
                Fact::Words("title", "generated"),
                Fact::Permission(chips(&facts(&create(declared.clone())))[0].clone()),
                Fact::Words("resume command", "none"),
                value("declared agent", "claude"),
            ],
            vec![boxed("command", "agent"), boxed("initial prompt", "first")],
            &ORDINARY,
        ),
        (
            ApprovalAction::Launch {
                verb: LaunchVerb::Clone,
                host_name: "other".into(),
                cwd: "/w".into(),
                repo: None,
                title: None,
                launch: codex.clone(),
                source: Some(anonymous.clone()),
            },
            "Start a copy of a session",
            vec![
                Fact::Words("copying session", "title unknown"),
                Fact::CopyIdentity("copied session id", reference("anon")),
                Fact::Words("copied session's host", "unknown to this helm"),
                value("new session on host", "other"),
                value("folder", "/w"),
                Fact::Words("title", "generated"),
                guarded_chips(),
                value("agent", "codex"),
                value("model", "gpt-test"),
                Fact::Words("effort", "agent's default"),
                Fact::Words("resume command", "none"),
            ],
            vec![boxed("command", "codex")],
            &ORDINARY,
        ),
        (
            ApprovalAction::Rename {
                target: target.clone(),
                title: "renamed".into(),
            },
            "Rename a session",
            target_facts(vec![value("new title", "renamed")]),
            vec![],
            &ORDINARY,
        ),
        (
            ApprovalAction::Stop {
                target: target.clone(),
            },
            "Stop a session's main program and close its terminal tabs",
            target_facts(vec![]),
            vec![],
            &ORDINARY,
        ),
        (
            ApprovalAction::Delete {
                target: target.clone(),
                guard: DeleteGuard::Unconditional,
            },
            "Delete a session and its stored state",
            target_facts(vec![Fact::Words(
                "what deletion stops",
                "the main program and all terminal tabs",
            )]),
            vec![],
            &ORDINARY,
        ),
        (
            ApprovalAction::Restart {
                target: target.clone(),
                stop_if_running: false,
                launch: Some(codex.clone()),
            },
            "Restart a session, resuming its conversation",
            target_facts(vec![
                Fact::Words("if it is working", "refuse"),
                guarded_chips(),
                value("agent", "codex"),
                value("model", "gpt-test"),
                Fact::Words("effort", "agent's default"),
                Fact::Words("resume command", "none"),
            ]),
            vec![],
            &ORDINARY,
        ),
        (
            ApprovalAction::TemplateWrite {
                name: "t".into(),
                replaces_existing: true,
                fields: TemplateFields {
                    kind: Some(farhelm_proto::launcher::LauncherKind::Agent),
                    agent: Some(LaunchHarness::Codex),
                    model: Some(None),
                    effort: Some(Some(farhelm_proto::LaunchEffort::High.into())),
                    ..TemplateFields::default()
                },
                host_name: None,
            },
            "Change a launch template",
            vec![
                value("template", "t"),
                value("launch kind", "agent"),
                value("agent", "codex"),
                Fact::Words("model", "agent's default"),
                value("effort", "high"),
            ],
            vec![],
            &ORDINARY,
        ),
        (
            ApprovalAction::TemplateDelete {
                name: "t".into(),
                fields: TemplateFields::default(),
                host_name: None,
            },
            "Delete a launch template",
            vec![value("template", "t")],
            vec![],
            &ORDINARY,
        ),
        (
            ApprovalAction::HostAdd {
                destination: "ops@new.example".into(),
                alias: None,
            },
            "Add a host",
            vec![value("ssh destination", "ops@new.example")],
            vec![],
            &DIRECT,
        ),
        (
            ApprovalAction::HostEdit {
                host_name: "box".into(),
                destination: None,
                alias: None,
                icon: None,
                color: None,
            },
            "Change a host's alias, icon or color",
            vec![
                value("host", "box"),
                Fact::Words("destination", "this machine"),
            ],
            vec![],
            &DIRECT,
        ),
        (
            ApprovalAction::HostRemove {
                host_name: "box".into(),
                destination: "ops@box.example".into(),
            },
            "Remove a host",
            vec![
                value("host", "box"),
                value("destination", "ops@box.example"),
            ],
            vec![],
            &DIRECT,
        ),
        (
            ApprovalAction::OpenUrl {
                url: "https://example.com/".into(),
                local_network: false,
                preapproved: false,
                tunnel_available: false,
            },
            "Open a link in your browser",
            vec![
                Fact::Identity("site", "example.com".into()),
                value("URL", "https://example.com/"),
            ],
            vec![],
            &ORDINARY,
        ),
        (
            ApprovalAction::LaunchWithAgentWrapper {
                verb: LaunchVerb::Create,
                host_name: "box".into(),
                cwd: "/w".into(),
                repo: None,
                title: None,
                launch: wrapped.clone(),
                source: None,
                prompt: None,
            },
            "Start a new session",
            vec![
                value("new session on host", "box"),
                value("folder", "/w"),
                Fact::Words("title", "generated"),
                chip_of(&wrapped),
                value("agent", "claude"),
                Fact::Words("model", "agent's default"),
                Fact::Words("effort", "agent's default"),
                Fact::Words("resume command", "none"),
            ],
            vec![
                boxed("agent supplied wrapper", "wrap --"),
                boxed("command", "wrap -- claude"),
            ],
            &DIRECT,
        ),
        (
            ApprovalAction::LaunchEnvironment {
                verb: LaunchVerb::Create,
                recipe: "dev".into(),
                place: Place::TensorlakeAccount,
                place_name: None,
                cwd: "~".into(),
                repo: None,
                title: None,
                launch: plain.clone(),
                source: None,
                prompt: Some("start".into()),
            },
            "Start a session in a new sandbox or container",
            vec![
                value("recipe", "dev"),
                Fact::Words("creates", "a new Tensorlake sandbox"),
                Fact::Words(
                    "consequence",
                    "Allow creates a new Tensorlake sandbox for this session. It bills while running; Suspend stops \
                     CPU and memory billing, and deleting its last session removes the sandbox and its files.",
                ),
                value("folder", "~"),
                Fact::Words("title", "generated"),
                chip_of(&plain),
                Fact::Words("resume command", "none"),
            ],
            vec![
                boxed("command", "agent --go"),
                boxed("initial prompt", "start"),
            ],
            &DIRECT,
        ),
        (
            ApprovalAction::LaunchEnvironment {
                verb: LaunchVerb::Clone,
                recipe: "dev".into(),
                place: Place::PodmanHost(3),
                place_name: Some("build-box".into()),
                cwd: String::new(),
                repo: Some("acme/web".into()),
                title: Some("copy".into()),
                launch: plain.clone(),
                source: Some(target.clone()),
                prompt: None,
            },
            "Copy a session into a new sandbox or container",
            vec![
                value("copying session", "target title"),
                Fact::CopyIdentity("copied session id", reference("target")),
                value("copied session's host", "box"),
                value("recipe", "dev"),
                value("new container on host", "build-box"),
                Fact::Words(
                    "consequence",
                    "Allow creates a new container for this session on the container host. Deleting its last \
                     session removes the container and its files.",
                ),
                value("managed checkout of", "GitHub: acme/web"),
                value("title", "copy"),
                chip_of(&plain),
                Fact::Words("resume command", "none"),
            ],
            vec![boxed("command", "agent --go")],
            &DIRECT,
        ),
        (
            ApprovalAction::Prompt {
                target: target.clone(),
                launch: declared.clone(),
                screen_yolo: false,
                attached: false,
                prompt: "p".into(),
            },
            "Send a prompt to a session's agent",
            target_facts(vec![
                chip_of(&declared),
                Fact::Words(
                    "delivery",
                    "the prompt may interleave with user typing or follow text already in the composer",
                ),
            ]),
            vec![boxed("prompt", "p")],
            &ORDINARY,
        ),
    ];
    let mut kinds = std::collections::BTreeSet::new();
    for (action, heading, facts, boxes, answers) in cases {
        let card = Card::of(&request("r", "host", action), TunnelOffer::Offered);
        assert_eq!(card.heading, heading, "{}", card.kind);
        assert_eq!(card.facts, facts, "{}", card.kind);
        assert_eq!(card.boxes, boxes, "{}", card.kind);
        assert_eq!(card.notes, old_notes(card.kind), "{}", card.kind);
        assert_eq!(offered(&card), answers, "{}", card.kind);
        kinds.insert(card.kind);
    }
    assert_eq!(
        kinds.len(),
        15,
        "every kind but the shortcut card's has a case: {kinds:?}"
    );
}

/// The notes the old app drew after any refusal on a card of `kind`, in order.
fn old_notes(kind: &str) -> Vec<&'static str> {
    const WRAPPER: &str = "The agent supplied this wrapper. Farhelm cannot check what it runs; this request needs your Allow.";
    const FORGET: &str = "Farhelm will forget this host. Its supervisor and sessions keep running, and adding the \
                          destination again finds them.";
    const HOST_CHANGE: &str = "Each host change needs your approval, even when the requesting host can run Farhelm \
                               commands without asking.";
    match kind {
        "launch_with_agent_wrapper" => vec![WRAPPER],
        "host_add" | "host_edit" => vec![HOST_CHANGE],
        "host_remove" => vec![FORGET, HOST_CHANGE],
        _ => vec![],
    }
}

/// The wording the drawing puts around host values is the old app's, kept here so the drawing cannot retype it: the
/// narrow grant's label and help (the host named twice in the help, so it is clear the grant covers that host alone),
/// the fleet-wide label, the tunnel note, the From line's ID label and Copy ID, and every copy button's help, which
/// says the raw reference is what gets copied.
#[test]
fn the_wording_around_host_values_is_the_old_apps() {
    assert_eq!(ON_HOST_ONLY, ("Always allow on ", " only"));
    assert_eq!(FROM_HOST, "Always allow from ");
    assert_eq!(
        OWN_HOST_HELP.join("<host>"),
        "Always allow: let agents in <host>'s sessions start, stop, restart, rename and delete sessions on <host> \
         without asking. Requests for other hosts still ask."
    );
    let (before, after) = LeadNote::tunnel_words(8123);
    assert_eq!(
        format!("{before}<host>{after}"),
        "This is port 8123 on <host>. Opening it directly would reach this Mac instead."
    );
    assert_eq!(
        (FROM_ID_LABEL, FROM_COPY_NAME),
        ("From session ID", "Copy from session ID")
    );
    assert_eq!((COPY_ID, COPY), ("Copy ID", "Copy"));
    assert_eq!(
        COPY_HELP,
        "Copy: the complete session ID, including characters shown as escapes"
    );
}
