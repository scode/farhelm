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
    /// `farhelm agent rename`.
    Rename {
        target: ApprovalSession,
        title: String,
    },
    /// `farhelm agent stop`.
    Stop { target: ApprovalSession },
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
    /// `farhelm agent template delete`, with the template as it is now.
    TemplateDelete {
        name: String,
        fields: TemplateFields,
        /// The display name of the host `fields.host` names, when one does.
        host_name: Option<String>,
    },
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
