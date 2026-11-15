//! A session's plain Restart: whether it is offered and why not, whether it asks, what the question says, the consent
//! the request carries, and how its wait and its refusal show.
//!
//! SPEC.md, "Lifecycle operations": Restart relaunches the agent in the same directory and resumes the session's own
//! conversation; it is offered only when Farhelm can resume it, and is unavailable rather than starting fresh
//! otherwise. Ported from the old app's `restart_consequence`, `restart_prompt_stops_agent` (`status.rs`), the header's
//! Restart button, `direct_restart_stops_agent`, `offer_clause`, `feedback_note`, `not_captured_clause` and
//! `restart_button_label` (`session_view.rs`), and `restart_session` (`api.rs`) at the reference commit. The old row
//! menu had no Restart, so the open session's header is the one place that offers it here. Here's the TLDR of the
//! rules:
//!
//! - **Offered only when the conversation can be resumed.** The session's `restart_offer` (the supervisor's, never
//!   re-derived here) decides. When it is not Resume, the header still draws Restart, greyed but focusable, and its
//!   hover help and accessible name say why ([`offer_label`], [`hover_help`]), so the reason stays reachable.
//! - **Ask only while the agent reads working.** A working agent (status Running) asks first, unless Skip session
//!   confirmations is on; an idle, waiting or unknown agent is stopped and relaunched on the click, because a prompt
//!   shown on every live agent gets clicked through unread.
//! - **The consent is what the user was shown.** The question opens only on a working agent and says it is stopped
//!   first, so a confirmed question authorizes that; a click that asked nothing authorizes it only when Skip session
//!   confirmations let a working agent go ahead unasked. The supervisor refuses a restart of a working agent without
//!   that consent, so an agent that turned working after the click is refused rather than stopped unconfirmed (SPEC.md,
//!   "Confirmed 2026-10-01").
//! - **A long wait says so, as Replace's does.** After two seconds the open session's header shows "Restarting on
//!   <host>… <n> s" ([`super::outcome`]'s clock, shared with Replace's).
//! - **A refusal stays on the session** as "Restart: <the helm's words>", in the operation store's slot for the session,
//!   for as long as [`super::outcome`] says. A success says nothing: the pushed list shows the session running again.
//!
//! The decisions live in [`begin`] and [`confirm`], functions of a session's row, so another surface (the interrupted
//! card, a later piece) can ask the same question and send the same request.

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::session_requests::RestartSessionBody;
use farhelm_proto::ui_channel::SessionRow;
use farhelm_proto::{AgentKind, ReadinessWording, RestartOffer, SessionStatus};
use gpui::{App, Window};
use serde_json::json;

use super::outcome;
use crate::components::{HoverHelp, Tier};
use crate::operations::{Command, OpKey, Work};
use crate::prompt::{Answer, Question};
use crate::session;

// ---- Whether Restart is offered, and the words that say so -----------------------------------------------------------

/// Whether `row`'s Restart can run at all: only when the supervisor says its own conversation can be resumed.
pub(crate) fn offered(row: &SessionRow) -> bool {
    row.info.restart_offer.can_restart()
}

/// Restart's accessible name for `offer`: the offer rather than the action, because "Restart" alone would leave the
/// user guessing whether the conversation survives, which SPEC.md requires an honest answer to before the click.
pub(crate) fn offer_label(offer: RestartOffer) -> &'static str {
    if offer.can_restart() {
        "Resume conversation"
    } else {
        "Restart unavailable"
    }
}

/// Restart's hover help for `row`: its accessible name, an em dash, then [`offer_continuation`], as the old header's
/// tooltip joined them.
pub(crate) fn hover_help(row: &SessionRow) -> String {
    format!(
        "{} — {}",
        offer_label(row.info.restart_offer),
        offer_continuation(
            &row.info.status,
            row.info.restart_offer,
            row.info.agent_kind
        )
    )
}

/// What a restart of a session with `status`, `offer` and `kind` does, or why it cannot, as the clause that follows the
/// accessible name in the hover help: a lead for the two states where the terminal is gone for a reason the user should
/// hear first (interrupted, never started), the offer's own clause, and for an uncaptured conversation the invitation to
/// send feedback. Built from authored clauses, so the agent's name and "Restart" keep their case.
fn offer_continuation(status: &SessionStatus, offer: RestartOffer, kind: AgentKind) -> String {
    let ended = status.has_ended();
    let offered = offer_clause(offer, kind, ended);
    let note = feedback_note(offer, ended);
    match status {
        SessionStatus::Interrupted => {
            format!("interrupted by a host reboot or a Farhelm update — {offered}.{note}")
        }
        SessionStatus::Error { .. } => format!("the agent never started — {offered}.{note}"),
        SessionStatus::Running
        | SessionStatus::Waiting
        | SessionStatus::Idle
        | SessionStatus::Unknown
        | SessionStatus::Exited { .. } => format!("{offered}.{note}"),
    }
}

/// Whether a restart resumes the conversation, and why not when it cannot, as one clause without its period. Every
/// unavailable clause names Replace, since SPEC.md makes Replace the way such a session starts over. No "Restart is
/// unavailable" here: the accessible name says so, and the hover help joins the two.
fn offer_clause(offer: RestartOffer, kind: AgentKind, ended: bool) -> String {
    match offer {
        RestartOffer::Resume => "restarting resumes this session's own conversation".to_owned(),
        RestartOffer::NotCaptured => not_captured_clause(kind, ended),
        RestartOffer::NoConversationReporting => {
            "Farhelm has no conversation reporting for this session's agent, so Replace starts it over"
                .to_owned()
        }
        RestartOffer::NoResumeCommand => {
            "this command launch did not opt into Resume, so it has no resume command and Replace starts it over"
                .to_owned()
        }
    }
}

/// What an uncaptured conversation means for a session of `kind`. While the agent may still report, it says, naming
/// the agent, when Restart normally becomes available (the agent kind's own timing, the one source of those facts);
/// once the agent has ended that would be false, so it says the conversation was never captured. A kind that names no
/// agent gets the same shape with no per-agent claim.
fn not_captured_clause(kind: AgentKind, ended: bool) -> String {
    const START_OVER: &str = "Replace starts the session over";
    let readiness = kind
        .display_name()
        .zip(kind.restart_readiness())
        .map(|(name, readiness)| (name, readiness.clause(ReadinessWording::ToTheUser)));
    match (readiness, ended) {
        (Some((name, timing)), false) => format!(
            "for {name} sessions, Restart becomes available {timing}; until then, {START_OVER}"
        ),
        (None, false) => format!(
            "Restart becomes available once Farhelm captures this session's conversation; until then, {START_OVER}"
        ),
        (Some((name, _)), true) => format!(
            "Farhelm never captured this {name} session's conversation, so Restart can't resume it; {START_OVER}"
        ),
        (None, true) => format!(
            "Farhelm never captured this session's conversation, so Restart can't resume it; {START_OVER}"
        ),
    }
}

/// The invitation to send feedback when an uncaptured conversation's Restart never arrives, after the explanation's own
/// period; empty for every other offer. The app cannot tell a report still coming from one that never will, so it says
/// what normally happens and asks to hear about it when that is wrong (SPEC.md, "Terminal experience").
pub(crate) fn feedback_note(offer: RestartOffer, ended: bool) -> &'static str {
    match (offer, ended) {
        (RestartOffer::NotCaptured, false) => {
            " (If Restart doesn't become available, please send feedback from the Help (?) menu.)"
        }
        (RestartOffer::NotCaptured, true) => {
            " (If you expected Restart here, please send feedback from the Help (?) menu.)"
        }
        (
            RestartOffer::Resume
            | RestartOffer::NoConversationReporting
            | RestartOffer::NoResumeCommand,
            _,
        ) => "",
    }
}

// ---- What a Restart asks and sends -----------------------------------------------------------------------------------

/// Whether Restart of `row` asks first: only while its agent reads working, and not when confirmations are skipped
/// (`skipped`).
fn asks(row: &SessionRow, skipped: bool) -> bool {
    row.info.status == SessionStatus::Running && !skipped
}

/// The consent a Restart that asked nothing carries: it stops a working agent only when the question was skipped by the
/// preference and the click showed the agent working, as the old `direct_restart_stops_agent` decided. With
/// confirmations on, a click that asked nothing showed an idle, waiting or unknown agent, which the supervisor stops
/// without consent anyway.
fn direct_consent(row: &SessionRow, skipped: bool) -> bool {
    skipped && row.info.status == SessionStatus::Running
}

/// The question's one sentence. The question opens only on an agent that reads working ([`asks`]), and a question is
/// frozen when it opens, so this is the only sentence it can show; the old app's other wordings were for an open
/// question it redrew as the status moved, which the prompt never does.
const CONSEQUENCE: &str =
    "Still running — restarting stops the agent and its whole process tree first:";

/// What a confirmed Restart acts on: the session, by the key the helm files it under. Its consent is always to stop
/// the working agent first, since that is what the only sentence it asks with says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Frozen {
    pub(crate) key: String,
}

/// The Restart question for the session filed under `key`: [`CONSEQUENCE`] alone (the old header question quoted no
/// title, since it hung under the open session's own button), "Confirm restart" in the danger tier, and a Cancel whose
/// help says what it keeps.
fn question(key: &str) -> Question<Frozen> {
    Question::new(
        "Restart confirmation",
        Answer {
            label: "Confirm restart".into(),
            help: HoverHelp::tooltip(
                "Confirm restart: stop the running agent, then resume its conversation",
            ),
            tier: Tier::Danger,
        },
        Frozen {
            key: key.to_owned(),
        },
    )
    .line(CONSEQUENCE)
    .cancel_answer(Answer {
        label: "Cancel".into(),
        help: HoverHelp::tooltip("Cancel: leave the agent running"),
        tier: Tier::Neutral,
    })
    .keeps_focus_on_empty_click()
}

/// Start a Restart of `row`, filed under `key`, with Skip session confirmations as `skipped`: the one statement of
/// Restart's rules. Nothing happens for a session that cannot resume its conversation. A Restart that asks nothing is
/// sent at once with [`direct_consent`]'s consent, and there is nothing to ask. Otherwise nothing is sent, and what
/// comes back builds the question to ask, whose answer goes to [`confirm`].
pub(crate) fn begin<'a>(
    key: &'a str,
    row: &'a SessionRow,
    skipped: bool,
    cx: &mut App,
) -> Option<impl Fn() -> Question<Frozen> + 'a> {
    if !offered(row) {
        return None;
    }
    if !asks(row, skipped) {
        send(key, direct_consent(row, skipped), cx);
        return None;
    }
    Some(move || question(key))
}

/// A Restart question was confirmed: send the Restart, consenting to stop the working agent its question named.
pub(crate) fn confirm(frozen: Frozen, _: &mut Window, cx: &mut App) {
    send(&frozen.key, true, cx);
}

/// The session's plain Restart, `POST /api/sessions/{id}/restart`, whose body is the route's shared client body
/// ([`RestartSessionBody`], which the helm's tests decode into its own request type) with only the consent set: no
/// changed launch (that is Restart with), no named conversation and no YOLO consent, which a plain Restart never needs
/// (it relaunches a choice already made).
pub(crate) fn command(session: &str, stop_if_running: bool) -> Result<Command, PathError> {
    let request = RestartSessionBody {
        conversation: None,
        stop_if_running,
        with: None,
        with_command: None,
        confirm_yolo: false,
    };
    Ok(Command::new(
        Work::Restart,
        Method::Post,
        ApiPath::api(&["sessions", session, "restart"])?,
        // Two booleans and unset options always encode, so `json!` (which panics on a failed encode) cannot fail here.
        Some(json!(request)),
        "POST /api/sessions/[session]/restart",
    ))
}

/// Send `key`'s Restart with `stop_if_running` through the operation store and start its wait's clock. A session that
/// already has a request in flight sends nothing.
pub(crate) fn send(key: &str, stop_if_running: bool, cx: &mut App) {
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let Ok(command) = command(key, stop_if_running) else {
        tracing::warn!("a session key that cannot be a path segment was not restarted");
        return;
    };
    // Nothing to do once it settles: a refusal is the operation store's to keep, and a success moves no focus and
    // opens nothing, since the pushed list shows the session running again and the terminal follows the restarted
    // agent through its own stream.
    let started = operations.update(cx, |operations, cx| {
        operations.start(&OpKey::session(key), command, cx, |_, _| {})
    });
    match started {
        Ok(()) => outcome::start_clock(key, Work::Restart, cx),
        Err(refused) => tracing::debug!(?refused, "a Restart was not sent"),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    /// A row with `status`, `offer` and `agent`, from the helm's pinned fixture.
    fn row(status: Value, offer: &str, agent: &str) -> SessionRow {
        let mut entry = crate::test_support::session_entry("a").value;
        entry["status"] = status;
        entry["restart_offer"] = json!(offer);
        entry["agent_kind"] = json!(agent);
        serde_json::from_value(entry).unwrap()
    }

    /// Restart's hover help is the old header tooltip, the only place a sighted user learns why Restart is greyed, so
    /// its wording per offer is pinned. Specified: a resumable session; an uncaptured running Codex session (the
    /// timing, and the feedback invitation for a report still to come); an uncaptured exited Claude session (never
    /// captured, and the other invitation); an uncaptured running session of no known agent; an agent without
    /// conversation reporting; a command launch without Resume; and an interrupted session's lead.
    #[test]
    fn the_hover_help_names_the_offer_then_says_why() {
        let running = json!({"state": "running"});
        assert_eq!(
            hover_help(&row(running.clone(), "resume", "claude")),
            "Resume conversation — restarting resumes this session's own conversation."
        );
        assert_eq!(
            hover_help(&row(running.clone(), "not_captured", "codex")),
            "Restart unavailable — for Codex sessions, Restart becomes available once you submit your first prompt; \
             until then, Replace starts the session over. (If Restart doesn't become available, please send feedback \
             from the Help (?) menu.)"
        );
        assert_eq!(
            hover_help(&row(
                json!({"state": "exited", "exit_code": 0}),
                "not_captured",
                "claude"
            )),
            "Restart unavailable — Farhelm never captured this Claude session's conversation, so Restart can't \
             resume it; Replace starts the session over. (If you expected Restart here, please send feedback from the \
             Help (?) menu.)"
        );
        assert_eq!(
            hover_help(&row(running.clone(), "not_captured", "generic")),
            "Restart unavailable — Restart becomes available once Farhelm captures this session's conversation; until \
             then, Replace starts the session over. (If Restart doesn't become available, please send feedback from \
             the Help (?) menu.)"
        );
        assert_eq!(
            hover_help(&row(
                running.clone(),
                "no_conversation_reporting",
                "generic"
            )),
            "Restart unavailable — Farhelm has no conversation reporting for this session's agent, so Replace starts \
             it over."
        );
        assert_eq!(
            hover_help(&row(running, "no_resume_command", "claude")),
            "Restart unavailable — this command launch did not opt into Resume, so it has no resume command and \
             Replace starts it over."
        );
        assert_eq!(
            hover_help(&row(json!({"state": "interrupted"}), "resume", "claude")),
            "Resume conversation — interrupted by a host reboot or a Farhelm update — restarting resumes this \
             session's own conversation."
        );
    }

    /// SPEC.md: a working agent's Restart confirms, and an idle, waiting or unknown one does not, "because a prompt
    /// shown on every live agent gets clicked through unread"; and a skipped question carries consent only for what
    /// the click showed working. Specified: only Running asks with confirmations on; nothing asks with them skipped;
    /// a click that asked nothing consents only when skipped on a Running agent.
    #[test]
    fn only_a_working_agent_asks_and_a_skipped_question_consents_only_for_it() {
        for (status, working) in [
            (json!({"state": "running"}), true),
            (json!({"state": "idle"}), false),
            (json!({"state": "waiting"}), false),
            (json!({"state": "unknown"}), false),
            (json!({"state": "exited", "exit_code": 0}), false),
        ] {
            let row = row(status.clone(), "resume", "claude");
            assert_eq!(asks(&row, false), working, "{status}");
            assert!(!asks(&row, true), "{status}: skipped");
            assert!(!direct_consent(&row, false), "{status}: asked nothing");
            assert_eq!(direct_consent(&row, true), working, "{status}: skipped");
        }
    }

    /// A Restart goes to its session's own restart route as the Restart work. Specified: `Work::Restart` on
    /// `/api/sessions/<key>/restart` for both consents; the body is checked where the window sends it.
    #[test]
    fn a_restart_goes_to_its_sessions_restart_route() {
        for consent in [false, true] {
            let built = command("a", consent).unwrap();
            assert_eq!(built.work(), Work::Restart);
            assert_eq!(built.path().as_str(), "/api/sessions/a/restart");
        }
    }
}
