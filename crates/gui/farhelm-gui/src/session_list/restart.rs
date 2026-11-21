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
//! - **A host change comes first.** Restart waits for one ([`super::menu::launch_wait`]).
//! - **A long wait says so, as Replace's does.** After two seconds the open session's header shows "Restarting on
//!   <host>… <n> s" ([`super::outcome`]'s clock, shared with Replace's).
//! - **A refusal stays on the session** as "Restart: <the helm's words>", in the operation store's slot for the session,
//!   for as long as [`super::outcome`] says. A success says nothing: the pushed list shows the session running again,
//!   once the host's next listing reaches the helm (the interrupted card bridges that gap).
//!
//! The decisions live in [`begin`] and [`confirm`], functions of a session's row, so another surface (the interrupted
//! card, `session_pane/card.rs`) can ask the same question and send the same request.

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::session_requests::RestartSessionBody;
use farhelm_proto::ui_channel::SessionRow;
use farhelm_proto::{AgentKind, LaunchKind, ReadinessWording, RestartOffer, SessionStatus};
use gpui::{App, Window};
use serde_json::json;

use super::{menu, outcome};
use crate::components::{HoverHelp, Tier};
use crate::operations::{Command, OpKey, Outcome, Work};
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
        offer_continuation(row)
    )
}

/// What a restart of this session does, or why it cannot, as the clause that follows the
/// accessible name in the hover help: a lead for the two states where the terminal is gone for a reason the user should
/// hear first (interrupted, never started), the offer's own clause, and for an uncaptured conversation the invitation to
/// send feedback. Built from authored clauses, so the agent's name and "Restart" keep their case. The header's Restart
/// menu shows it, with its first letter raised and no closing period, as its plain Restart's description, as the old
/// menu showed the same explanation there.
pub(crate) fn offer_continuation(row: &SessionRow) -> String {
    let status = &row.info.status;
    let ended = status.has_ended();
    let offered = offer_clause(row, ended);
    let note = feedback_note(row.info.restart_offer, ended);
    match status {
        SessionStatus::Interrupted => {
            format!("interrupted by a host reboot or a Farhelm update — {offered}.{note}")
        }
        SessionStatus::Error { .. } => {
            let program = super::stop::program_word(row);
            format!("the {program} never started — {offered}.{note}")
        }
        SessionStatus::Running
        | SessionStatus::Waiting
        | SessionStatus::Idle
        | SessionStatus::Unknown
        | SessionStatus::Exited { .. } => format!("{offered}.{note}"),
    }
}

/// Whether a restart resumes the conversation, and why not when it cannot, as one clause without its period. Every
/// unavailable clause names Replace, since SPEC.md makes Replace the way such a session starts over. No "Restart is
/// unavailable" here: the accessible name says so, and the hover help joins the two. The interrupted card's text
/// (`session_pane/card.rs`) is built from the same clause, so the card and the hover help cannot drift apart.
pub(crate) fn offer_clause(row: &SessionRow, ended: bool) -> String {
    match row.info.restart_offer {
        RestartOffer::Resume => "restarting resumes this session's own conversation".to_owned(),
        RestartOffer::NotCaptured => not_captured_clause(row.info.agent_kind, ended, REPLACE_STARTS_OVER),
        RestartOffer::NoConversationReporting if row.info.launch.is_shell() => {
            "this shell has no conversation to resume, so Replace starts a fresh shell".to_owned()
        }
        RestartOffer::NoConversationReporting => {
            "Farhelm has no conversation reporting for this session's agent, so Replace starts it over"
                .to_owned()
        }
        RestartOffer::NoResumeCommand if row.info.launch.launch_kind() == LaunchKind::Agent => {
            "this agent launch has no stored resume command, so Replace starts it over".to_owned()
        }
        RestartOffer::NoResumeCommand => {
            "this command launch did not opt into Resume, so it has no resume command and Replace starts it over"
                .to_owned()
        }
    }
}

/// How an uncaptured conversation's explanation says the session can start over, on every surface that offers plain
/// Replace.
pub(crate) const REPLACE_STARTS_OVER: &str = "Replace starts the session over";

/// What an uncaptured conversation means for a session of `kind`, ending in `start_over`, the way the surface offers to
/// start the session over ([`REPLACE_STARTS_OVER`], or the interrupted card's Replace with for a legacy launch). While
/// the agent may still report, it says, naming the agent, when Restart normally becomes available (the agent kind's own
/// timing, the one source of those facts); once the agent has ended that would be false, so it says the conversation
/// was never captured. A kind that names no agent gets the same shape with no per-agent claim.
pub(crate) fn not_captured_clause(kind: AgentKind, ended: bool, start_over: &str) -> String {
    let readiness = kind
        .display_name()
        .zip(kind.restart_readiness())
        .map(|(name, readiness)| (name, readiness.clause(ReadinessWording::ToTheUser)));
    match (readiness, ended) {
        (Some((name, timing)), false) => format!(
            "for {name} sessions, Restart becomes available {timing}; until then, {start_over}"
        ),
        (None, false) => format!(
            "Restart becomes available once Farhelm captures this session's conversation; until then, {start_over}"
        ),
        (Some((name, _)), true) => format!(
            "Farhelm never captured this {name} session's conversation, so Restart can't resume it; {start_over}"
        ),
        (None, true) => format!(
            "Farhelm never captured this session's conversation, so Restart can't resume it; {start_over}"
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
        Answer::new(
            "Confirm restart",
            HoverHelp::tooltip(
                "Confirm restart: stop the running agent, then resume its conversation",
            ),
            Tier::Danger,
        ),
        Frozen {
            key: key.to_owned(),
        },
    )
    .line(CONSEQUENCE)
    .cancel_answer(Answer::new(
        "Cancel",
        HoverHelp::tooltip("Cancel: leave the agent running"),
        Tier::Neutral,
    ))
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
/// already has a request in flight sends nothing, and nothing is sent while a host change is in flight
/// ([`menu::refuse_for_host_change`]).
pub(crate) fn send(key: &str, stop_if_running: bool, cx: &mut App) {
    if menu::refuse_for_host_change(key, Work::Restart, cx) {
        return;
    }
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let Ok(command) = command(key, stop_if_running) else {
        tracing::warn!("a session key that cannot be a path segment was not restarted");
        return;
    };
    // A successful reply that published a terminal lets the recovery card give way before the listing catches up.
    // If the supervisor could not publish the new terminal, its reply keeps the card in place.
    let session = key.to_owned();
    let started = operations.update(cx, |operations, cx| {
        operations.start(&OpKey::session(key), command, cx, move |settled, cx| {
            if let Outcome::Done(body) = &settled.outcome {
                let terminal_missing = body
                    .as_ref()
                    .and_then(|body| body.get("terminal_missing"))
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or_default();
                crate::session_pane::card::restart_finished(&session, terminal_missing, cx);
            }
        })
    });
    match started {
        Ok(()) => outcome::start_clock(key, Work::Restart, cx),
        Err(refused) => tracing::debug!(?refused, "a Restart was not sent"),
    }
}

/// Whether `key`'s Restart is in flight in this window, from the moment its request is sent until the helm answers.
/// Restart with runs as the same work on the same key, so it counts too.
///
/// The open session's pane asks it while the terminal draws, to hold back the terminal's reconnect card: a Restart
/// relaunches the agent and the supervisor moves the viewer to the new terminal with a detach that reconnects, so the
/// card would announce an outage the user just asked for (`session_pane.rs`).
pub(crate) fn in_flight(key: &str, cx: &App) -> bool {
    session::current_operations(cx).is_some_and(|operations| {
        operations.read(cx).in_flight(&OpKey::session(key)) == Some(Work::Restart)
    })
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
    /// conversation reporting; and an interrupted session's lead. The header and interrupted-card tests exercise
    /// missing resume commands with fixtures for both launch kinds.
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
            hover_help(&row(running, "no_conversation_reporting", "generic")),
            "Restart unavailable — Farhelm has no conversation reporting for this session's agent, so Replace starts \
             it over."
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
