//! The helm's pending approvals: acting `farhelm` commands from inside a
//! session that wait for the user's answer in the GUI (SPEC.md, Agent-spawned
//! sessions).
//!
//! These types are the REST contract between the helm, which holds the
//! requests and decides, and the GUI, which shows one card per request and
//! sends the user's answer back. They live here rather than in either crate
//! so the two cannot drift: a card that misread what an agent is about to do
//! is the one failure this feature exists to prevent.
//!
//! Every string in an [`ApprovalAction`] other than the helm's own host names
//! is agent-controlled (titles, folders, command lines, template names). The
//! GUI renders them as labelled data and never as its own wording; nothing in
//! these types marks which is which beyond that rule, so a renderer that
//! interpolates a field into a sentence is a bug.

use serde::{Deserialize, Serialize};

use crate::SessionLaunch;
use crate::host_appearance::{HostColor, HostIcon};
use crate::launcher::TemplateFields;

/// How long a request waits for the user's answer before the helm refuses it
/// as not answered.
///
/// Nine minutes rather than ten so that an agent whose shell commands are
/// cut off at ten minutes (Claude Code's tool timeout) still sees the answer
/// rather than a killed command (SPEC.md). Both ends of the agent relay read
/// this one value: the helm expires the request at this bound, and the
/// supervisor's relay budget for a verb that may wait is this plus its
/// ordinary answer budget, so the helm's own expiry answer always arrives
/// before the relay gives up.
pub const APPROVAL_WAIT: std::time::Duration = std::time::Duration::from_secs(9 * 60);

/// `GET /api/approvals`: every request waiting for the user, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingApprovals {
    pub approvals: Vec<PendingApproval>,
}

/// One request waiting for the user's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingApproval {
    /// Opaque, unique for the helm process's life; what an answer names.
    pub id: String,
    /// The registry id of the host whose connection carried the request. This
    /// is the host "Always allow" turns the setting on for.
    pub host_id: i64,
    /// That host's display name, as the helm renders it.
    pub host_name: String,
    /// The session that asked.
    pub session: ApprovalSession,
    /// What it asked for.
    pub action: ApprovalAction,
    /// Unix milliseconds, on the HELM's clock, at which the helm refuses the
    /// request as not answered. A GUI on another machine may disagree about
    /// the time by its clock skew, so this is for display, never a deadline
    /// a client acts on.
    pub expires_at_ms: i64,
}

/// A session named on a card: the asking session, or the target of a
/// lifecycle action. `title` and `host_name` are what the helm's session
/// cache knows, `None` when it does not know the session (a session created
/// moments ago, or one on a host it has not listed yet).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalSession {
    pub id: String,
    pub title: Option<String>,
    pub host_name: Option<String>,
}

/// Shortcut effects shown by the card for a template write.
///
/// These are the differences from the bindings read before approval. The
/// helm retains the full snapshot separately and checks it in the write
/// transaction; the card carries only the changes a person must see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateShortcutCard {
    pub quick_launch: Option<QuickLaunchCard>,
    pub key: Option<TemplateKeyCard>,
}

/// How the ⚡ mark changes when a template write is approved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum QuickLaunchCard {
    /// Mark this template, possibly taking the mark from another template.
    Set { moved_from: Option<String> },
    /// Remove the mark that this template currently holds.
    Clear,
}

/// The full number-key effect an approval must display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum TemplateKeyCard {
    /// Replace this template's old key and possibly take the new key from
    /// another template.
    Set {
        key: String,
        replaced_key: Option<String>,
        moved_from: Option<String>,
    },
    /// Remove the key currently held by this template.
    Clear { key: String },
}

/// What a waiting request would do once approved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalAction {
    /// A browser request, shown as exact peer-controlled URL data. A preapproved request
    /// still needs an Open click in a web client; desktop opens through the helm's native sink.
    OpenUrl {
        url: String,
        local_network: bool,
        preapproved: bool,
        /// Helm-computed offer, independent of consent. Older cards omit it.
        #[serde(default)]
        tunnel_available: bool,
    },
    /// A new session: `farhelm spawn`, `farhelm agent create`, or
    /// `farhelm agent clone`.
    Launch {
        verb: LaunchVerb,
        /// The target host's display name.
        host_name: String,
        cwd: String,
        /// GitHub repository spelling for a fresh managed checkout; the UI
        /// names that source explicitly. An older UI ignores this field and
        /// still shows the exact destination, while new UIs accept old cards.
        #[serde(default)]
        repo: Option<String>,
        /// The requested title; `None` lets the supervisor generate one.
        title: Option<String>,
        /// The whole launch the new session will run, as resolved. Whether
        /// it is YOLO is [`SessionLaunch::yolo`] of it, which a card reads
        /// itself rather than from a copy here that could drift, and which
        /// can be unknown (a launch from before launch kinds).
        launch: SessionLaunch,
        /// The session a clone copies.
        source: Option<ApprovalSession>,
    },
    /// A distinct tag makes older UIs refuse a prompt-bearing request rather
    /// than silently hide the first message while still offering Allow.
    LaunchWithPrompt {
        verb: LaunchVerb,
        host_name: String,
        cwd: String,
        repo: Option<String>,
        title: Option<String>,
        launch: SessionLaunch,
        source: Option<ApprovalSession>,
        prompt: String,
    },
    /// An agent-supplied wrapper needs this particular user's answer on a
    /// host that asks before YOLO. Its separate tag makes older clients fail
    /// closed instead of showing an ordinary launch card with Always allow.
    LaunchWithAgentWrapper {
        verb: LaunchVerb,
        host_name: String,
        cwd: String,
        repo: Option<String>,
        title: Option<String>,
        launch: SessionLaunch,
        source: Option<ApprovalSession>,
        prompt: Option<String>,
    },
    /// `farhelm agent rename`.
    Rename {
        target: ApprovalSession,
        title: String,
    },
    /// `farhelm agent stop`.
    Stop { target: ApprovalSession },
    /// A message typed into an existing agent. Its own tag keeps an older UI
    /// from approving text it cannot show. Launch and observed bypass are
    /// display evidence for the target's shield, not instructions to relaunch it.
    Prompt {
        target: ApprovalSession,
        launch: SessionLaunch,
        screen_yolo: bool,
        /// Informational snapshot: a client may attach or leave during the wait.
        attached: bool,
        prompt: String,
    },
    /// The guard describes the same snapshot shown to the user; it is never
    /// recomputed after approval, when more processes might be running.
    Delete {
        target: ApprovalSession,
        guard: crate::DeleteGuard,
    },
    /// `farhelm agent restart`: resumes the target's own conversation with its
    /// stored launch.
    Restart {
        target: ApprovalSession,
        /// Whether the request consents to stopping a working agent.
        stop_if_running: bool,
        /// The stored launch the restart will resume with, as the helm's
        /// cache knows it, so the card can show the resume command that will
        /// run; `None` when the cache does not know the session.
        launch: Option<SessionLaunch>,
    },
    /// `farhelm agent template create` or `edit`: the whole template as it
    /// will be stored, command text included.
    TemplateWrite {
        name: String,
        /// Whether a template of that name exists now (an edit).
        replaces_existing: bool,
        fields: TemplateFields,
        /// The display name of the host `fields.host` names, when one does.
        host_name: Option<String>,
    },
    /// A template write that also changes ⚡ or a number key. A separate kind
    /// keeps older UIs from approving a binding move they cannot display.
    TemplateWriteWithShortcuts {
        name: String,
        replaces_existing: bool,
        fields: TemplateFields,
        host_name: Option<String>,
        shortcuts: TemplateShortcutCard,
    },
    /// `farhelm agent template delete`, with the template as it is now.
    TemplateDelete {
        name: String,
        fields: TemplateFields,
        /// The display name of the host `fields.host` names, when one does.
        host_name: Option<String>,
    },
    /// `farhelm agent host edit`: each field the request changes, with its
    /// value now and the value approval writes. A field the request leaves
    /// alone, or sets to the value it already has, is `None` and is not
    /// written. The kind always needs a direct Allow
    /// ([`Self::requires_direct_allow`]), and its own tag makes a UI from
    /// before host edits fail to decode the card instead of offering Allow
    /// on one it cannot show.
    HostEdit {
        /// The host's display name now, before any alias change.
        host_name: String,
        /// The host's ssh destination; `None` for this machine.
        destination: Option<String>,
        /// `None` inside a change means no alias: the host is named by its
        /// destination (or as this machine).
        alias: Option<HostFieldChange<Option<String>>>,
        icon: Option<HostFieldChange<HostIcon>>,
        color: Option<HostFieldChange<HostColor>>,
    },
    /// `farhelm agent host remove`: the host approval forgets, as the GUI's
    /// remove does. Always needs a direct Allow, under its own tag for the
    /// reason [`Self::HostEdit`] gives. Only an ssh host is ever carded: the
    /// helm refuses this machine and the asking session's own host first.
    HostRemove {
        /// The host's display name.
        host_name: String,
        /// The ssh destination it is reached at, which the GUI's remove
        /// dialog also shows beside the name.
        destination: String,
    },
}

/// One host setting an approved [`ApprovalAction::HostEdit`] changes: what
/// the card showed as the value now, and what approval writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostFieldChange<T> {
    pub old: T,
    pub new: T,
}

impl ApprovalAction {
    /// Whether this request needs one visible Allow on its own card, which
    /// neither the requesting host's "run farhelm commands from this host
    /// without asking" setting nor an Always allow answer can stand in for.
    ///
    /// Two reasons put a kind here. An agent-supplied wrapper prefix: the
    /// host's ordinary command shortcut cannot vouch for a wrapper the agent
    /// chose, and a wrapper-bearing template becomes trusted by later
    /// launches only after this card. And a change to a host itself (edit or
    /// remove): the maintainer decided that an agent's changes to hosts are
    /// never approved automatically (SPEC.md, Agent-spawned sessions). A UI
    /// must offer only Allow and Deny for such a card, and the helm refuses
    /// an Always allow answer to one.
    pub fn requires_direct_allow(&self) -> bool {
        match self {
            Self::LaunchWithAgentWrapper { .. } => true,
            Self::HostEdit { .. } | Self::HostRemove { .. } => true,
            Self::TemplateWrite { fields, .. }
            | Self::TemplateWriteWithShortcuts { fields, .. } => {
                matches!(&fields.wrapper_prefix, Some(Some(_)))
            }
            // Listed rather than caught by a wildcard: a new card kind that
            // can carry a wrapper prefix must decide here whether it forces
            // the visible Allow, instead of silently inheriting `false`.
            Self::OpenUrl { .. }
            | Self::Launch { .. }
            | Self::LaunchWithPrompt { .. }
            | Self::Rename { .. }
            | Self::Stop { .. }
            | Self::Prompt { .. }
            | Self::Delete { .. }
            | Self::Restart { .. }
            | Self::TemplateDelete { .. } => false,
        }
    }
}

/// Which command a [`ApprovalAction::Launch`] came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchVerb {
    /// `farhelm spawn` with launch flags.
    Spawn,
    /// `farhelm spawn --inherit-agent`: the asking session's own launch.
    SpawnInherited,
    Create,
    Clone,
}

/// The body of `POST /api/approvals/{id}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalAnswerRequest {
    pub answer: ApprovalAnswer,
}

/// The user's answer to one card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalAnswer {
    Allow,
    /// Open this remote loopback URL through a consented native tunnel.
    /// This grants no persistent host permission and never opens the raw URL.
    TunnelAndOpen,
    /// Turn on the requesting host's "run farhelm commands from this host
    /// without asking" setting, then allow this request.
    AlwaysAllow,
    Deny,
}

#[cfg(test)]
mod tests {
    /// A direct wrapper answer must survive as a distinct card kind. Older
    /// clients cannot render its trust warning, so they must fail decoding.
    #[test]
    fn agent_wrapper_cards_fail_an_older_decoder_and_force_allow() {
        #[derive(serde::Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum OlderAction {
            Launch {},
        }
        let card = super::ApprovalAction::LaunchWithAgentWrapper {
            verb: super::LaunchVerb::Create,
            host_name: "host".into(),
            cwd: "/work".into(),
            repo: None,
            title: None,
            source: None,
            prompt: Some("go".into()),
            launch: crate::SessionLaunch::Command(crate::CommandLaunch {
                command: "with-endpoint codex".into(),
                agent: Some(crate::LaunchHarness::Codex),
                yolo: false,
                resume: None,
            }),
        };
        let value = serde_json::to_value(&card).unwrap();
        assert_eq!(value["kind"], "launch_with_agent_wrapper");
        assert_eq!(
            serde_json::from_value::<super::ApprovalAction>(value.clone()).unwrap(),
            card
        );
        assert!(serde_json::from_value::<OlderAction>(value).is_err());
        assert!(card.requires_direct_allow());

        let fields = crate::launcher::TemplateFields {
            wrapper_prefix: Some(Some(vec!["with-endpoint".into()])),
            ..Default::default()
        };
        assert!(
            super::ApprovalAction::TemplateWrite {
                name: "wrapped".into(),
                replaces_existing: false,
                fields,
                host_name: None,
            }
            .requires_direct_allow()
        );
    }

    /// Spec: host edit and host remove cards travel under their own kind tags,
    /// which a UI from before them cannot decode, and both always need a
    /// direct Allow.
    ///
    /// Why: an older UI that decoded one as some card it knows would offer
    /// Allow (and Always allow) on a card that does not show the host change,
    /// and a host change must never be approved by the requesting host's
    /// "without asking" setting.
    #[test]
    fn host_cards_fail_an_older_decoder_and_force_allow() {
        use crate::host_appearance::{HostColor, HostIcon};
        #[derive(serde::Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum OlderAction {
            TemplateDelete {},
        }
        let edit = super::ApprovalAction::HostEdit {
            host_name: "builder".into(),
            destination: Some("user@builder".into()),
            alias: Some(super::HostFieldChange {
                old: None,
                new: Some("baz".into()),
            }),
            icon: Some(super::HostFieldChange {
                old: HostIcon::Cloud,
                new: HostIcon::Rocket,
            }),
            color: Some(super::HostFieldChange {
                old: HostColor::Default,
                new: HostColor::Teal,
            }),
        };
        let remove = super::ApprovalAction::HostRemove {
            host_name: "builder".into(),
            destination: "user@builder".into(),
        };
        for (card, kind) in [(edit, "host_edit"), (remove, "host_remove")] {
            let value = serde_json::to_value(&card).unwrap();
            assert_eq!(value["kind"], kind);
            assert_eq!(
                serde_json::from_value::<super::ApprovalAction>(value.clone()).unwrap(),
                card
            );
            assert!(serde_json::from_value::<OlderAction>(value).is_err());
            assert!(card.requires_direct_allow(), "{kind}");
        }
    }

    /// Existing-agent messages need a distinct card tag: an older UI must
    /// fail closed instead of offering approval while omitting the message.
    #[test]
    fn running_prompt_cards_fail_an_older_action_decoder() {
        #[derive(serde::Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum OlderAction {
            Stop {},
        }
        let card = super::ApprovalAction::Prompt {
            target: super::ApprovalSession {
                id: "target".into(),
                title: Some("title".into()),
                host_name: Some("host".into()),
            },
            launch: crate::SessionLaunch::Command(crate::CommandLaunch {
                command: "codex".into(),
                agent: Some(crate::LaunchHarness::Codex),
                yolo: false,
                resume: None,
            }),
            screen_yolo: true,
            attached: true,
            prompt: "first\nsecond\t<script>".into(),
        };
        let value = serde_json::to_value(&card).unwrap();
        assert_eq!(value["kind"], "prompt");
        assert_eq!(
            serde_json::from_value::<super::ApprovalAction>(value.clone()).unwrap(),
            card
        );
        assert!(serde_json::from_value::<OlderAction>(value).is_err());
    }

    /// A separate kind prevents an older UI from offering Allow on a card
    /// that omits a shortcut move. Wrapper-bearing writes still need a
    /// visible Allow even on a host that normally skips cards.
    #[test]
    fn shortcut_template_cards_fail_an_older_action_decoder() {
        #[derive(serde::Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum OlderAction {
            TemplateWrite {},
        }
        let card = super::ApprovalAction::TemplateWriteWithShortcuts {
            name: "fast".into(),
            replaces_existing: true,
            fields: crate::launcher::TemplateFields {
                wrapper_prefix: Some(Some(vec!["with-endpoint".into()])),
                ..Default::default()
            },
            host_name: None,
            shortcuts: super::TemplateShortcutCard {
                quick_launch: Some(super::QuickLaunchCard::Set {
                    moved_from: Some("old".into()),
                }),
                key: Some(super::TemplateKeyCard::Set {
                    key: "1".into(),
                    replaced_key: None,
                    moved_from: None,
                }),
            },
        };
        let value = serde_json::to_value(&card).unwrap();
        assert_eq!(value["kind"], "template_write_with_shortcuts");
        assert_eq!(
            serde_json::from_value::<super::ApprovalAction>(value.clone()).unwrap(),
            card
        );
        assert!(serde_json::from_value::<OlderAction>(value).is_err());
        assert!(card.requires_direct_allow());
    }

    /// Old card payloads must not advertise a new native capability, while the
    /// new answer needs its own wire tag so it cannot be mistaken for raw Allow.
    #[test]
    fn tunnel_offer_defaults_off_and_answer_has_a_distinct_wire_tag() {
        let action = serde_json::from_value::<super::ApprovalAction>(serde_json::json!({
            "kind": "open_url", "url": "http://localhost/", "local_network": true, "preapproved": false
        })).unwrap();
        assert!(matches!(
            action,
            super::ApprovalAction::OpenUrl {
                tunnel_available: false,
                ..
            }
        ));
        assert_eq!(
            serde_json::to_value(super::ApprovalAnswer::TunnelAndOpen).unwrap(),
            "tunnel_and_open"
        );
    }

    /// A page loaded before prompt support must fail to decode this card;
    /// a silently ignored optional field would let it approve unseen text.
    #[test]
    fn initial_prompt_cards_fail_an_older_action_decoder() {
        #[derive(serde::Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum OlderAction {
            Launch {},
        }
        let card = super::ApprovalAction::LaunchWithPrompt {
            verb: super::LaunchVerb::Create,
            host_name: "host".into(),
            cwd: "/work".into(),
            repo: None,
            title: None,
            source: None,
            launch: crate::SessionLaunch::Agent {
                selection: crate::LaunchSelection {
                    harness: crate::LaunchHarness::Codex,
                    model: None,
                    effort: None,
                    permissions: None,
                    permission_flags: None,
                    wrapper_prefix: None,
                    workspace_trust: None,
                },
                start: vec![
                    "codex".into(),
                    crate::session_launch::FARHELM_ARGS_PLACEHOLDER.into(),
                ],
                resume: None,
            },
            prompt: "first\nsecond".into(),
        };
        let value = serde_json::to_value(&card).unwrap();
        assert_eq!(
            serde_json::from_value::<super::ApprovalAction>(value.clone()).unwrap(),
            card
        );
        assert!(serde_json::from_value::<OlderAction>(value).is_err());
    }
}
