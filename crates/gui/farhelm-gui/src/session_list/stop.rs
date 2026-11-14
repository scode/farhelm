//! A session's Stop as the session list runs it: whether to ask, what the question says, the preconditions the request
//! carries, and what the row shows while it runs and once it has settled.
//!
//! SPEC.md, "Lifecycle operations": Stop ends the agent and its whole process tree, then closes the session's terminal
//! tabs; the session stays, its agent terminal still viewable. Ported from the old app's `stop_consequence`
//! (`status.rs`), the row's Stop question and `.stop-progress` (`list/row.rs`), and `stop_session` (`api.rs`) at the
//! reference commit. Here's the TLDR of the rules:
//!
//! - **Ask only about tabs.** The row asks only when it lists terminal tabs, and never just because the agent is alive:
//!   a live agent with no tabs stops on the first click, as in the old app. Skip session confirmations skips the
//!   question too. "Agent ended" changes the wording and a precondition, not whether it asks.
//! - **The preconditions are what the row showed.** The request refuses a tab the click did not show
//!   (`refuse_if_tabs_open` when the row listed none) and an agent that is alive again after the row showed it ended
//!   (`refuse_if_agent_running`), both frozen from the row the question, or the skipped click, was drawn from (SPEC.md,
//!   "Confirmed 2026-10-01"). A question that named tabs authorizes closing every tab, one opened since included
//!   (accepted there).
//! - **A pending Stop is an expected wait, beside the title.** The row shows the spinner and "Stopping…" before its
//!   title, which stays, unlike Delete's, since the session stays too.
//! - **What the reply says stays on the row.** A refusal shows as "Stop: <the helm's words>", and a success whose reply
//!   names a tab the host could not close as "Stop: <that notice>", for as long as [`super::outcome`] says. A plain
//!   success says nothing: the pushed list already shows the session stopped.
//!
//! The question, the preconditions, the command and the progress are functions of a session's row, so the open
//! session's header (a later piece) can ask the same question and send the same request.

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::ui_channel::SessionRow;
use farhelm_proto::{SessionStopOutcome, SessionStopRequest};
use gpui::{
    App, Context, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement, Role,
    SharedString, StatefulInteractiveElement as _, Styled, Window, div, px,
};
use serde_json::{Value, json};

use super::{SessionList, delete, menu};
use crate::components::{HoverHelp, Tier};
use crate::focus::Mark;
use crate::host_text::{HostSentence, HostText};
use crate::operations::{Command, OpKey, Outcome, Settled, Work};
use crate::prompt::{Answer, Question};
use crate::session;
use crate::theme::{self, metrics};

/// The menu's Stop item's hover help (the item's description line says the same at more length).
pub(super) const MENU_HELP: &str = "Stop: end the agent and its processes, and close terminal tabs";

/// What a pending Stop says beside its spinner: on its row, and (in the old app) in place of the header's Stop label,
/// where the header will say it too once it is ported.
pub(crate) const PROGRESS_LABEL: &str = "Stopping…";

// ---- What a Stop of a row asks and sends -----------------------------------------------------------------------------

/// The preconditions a Stop of `row` as drawn carries, as the route's own request type, shared with the helm.
/// `refuse_if_tabs_open` is set when the row listed no terminal tab, so a tab open by the time the supervisor acts is
/// refused rather than closed. `refuse_if_agent_running` is set when the row showed the agent ended, so an agent alive
/// again (restarted by another client, say) is refused rather than stopped; the name reads backwards at first glance.
pub(crate) fn guard(row: &SessionRow) -> SessionStopRequest {
    SessionStopRequest {
        refuse_if_tabs_open: row.info.tabs.is_empty(),
        refuse_if_agent_running: row.info.status.has_ended(),
    }
}

/// Whether Stop of `row` asks first: only when the row lists terminal tabs, and not when confirmations are skipped
/// (`skipped`).
pub(crate) fn asks(row: &SessionRow, skipped: bool) -> bool {
    !row.info.tabs.is_empty() && !skipped
}

/// The question's one sentence: whether the agent is stopped (not when it has ended) and how many tabs close. The
/// tabless wordings are never asked from a row (a row without tabs does not ask), but a later surface whose tab count
/// moves while it asks can reach them.
pub(crate) fn consequence(row: &SessionRow) -> String {
    let ended = row.info.status.has_ended();
    match (row.info.tabs.len(), ended) {
        (0, false) => "Stop the agent?".to_owned(),
        (0, true) => "Stop this session?".to_owned(),
        (1, false) => "Stop the agent and close 1 terminal tab?".to_owned(),
        (1, true) => "Close 1 terminal tab?".to_owned(),
        (tabs, false) => format!("Stop the agent and close {tabs} terminal tabs?"),
        (tabs, true) => format!("Close {tabs} terminal tabs?"),
    }
}

/// What a confirmed Stop acts on: the session, by the key the helm files it under, and the preconditions of the row the
/// question was drawn from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Frozen {
    pub(crate) key: String,
    pub(crate) guard: SessionStopRequest,
}

/// The row's Stop question for `row`, filed under `key`, asked with `guard`: the consequence, then the title quoted on
/// a line of its own, as the old row's question had them. Its answer is "Confirm stop" in the danger tier, and its
/// Cancel says in its help what Cancel keeps, as the old row's did.
pub(crate) fn question(key: &str, row: &SessionRow, guard: SessionStopRequest) -> Question<Frozen> {
    let title = HostText::from_host(row.info.title.as_str());
    Question::new(
        "Stop confirmation",
        Answer {
            label: "Confirm stop".into(),
            help: HoverHelp::tooltip("Stop this session's agent and close its terminal tabs"),
            tier: Tier::Danger,
        },
        Frozen {
            key: key.to_owned(),
            guard,
        },
    )
    .line(consequence(row))
    .line(
        HostSentence::new(SharedString::from(format!("session-stop-title-{key}")))
            .text("\"")
            .host(&title)
            .text("\""),
    )
    .cancel_answer(Answer {
        label: "Cancel".into(),
        help: HoverHelp::tooltip("Cancel: keep the agent and terminal tabs running"),
        tier: Tier::Neutral,
    })
    .keeps_focus_on_empty_click()
}

/// The session's Stop, `POST /api/sessions/{id}/stop`, with both preconditions always spelled out: an omitted body
/// means "refuse if any tab is open" to the helm, which is right only for a click that showed no tabs.
pub(crate) fn command(session: &str, guard: SessionStopRequest) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Stop,
        Method::Post,
        ApiPath::api(&["sessions", session, "stop"])?,
        // Two booleans always encode, so `json!` (which panics on a failed encode) cannot fail here.
        Some(json!(guard)),
        "POST /api/sessions/[session]/stop",
    ))
}

// ---- Starting a Stop -------------------------------------------------------------------------------------------------

impl SessionList {
    /// The user asked to stop `key` (its row's Stop) at `mark`: stop it at once when the row lists no tabs or
    /// confirmations are skipped, and otherwise ask, beside the row's menu button while the row is drawn and centered
    /// when it is not. Nothing happens for a session that is not listed, or that has any request in flight.
    pub(crate) fn stop(
        &mut self,
        key: &str,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = menu::actionable(key, cx) else {
            return;
        };
        let row = &target.row;
        let guard = guard(row);
        if !asks(row, target.skips_questions) {
            send(key, guard, cx);
            return;
        }
        self.ask_about_row(
            key,
            || question(key, row, guard),
            mark,
            |frozen: Frozen, _: &mut Window, cx: &mut App| send(&frozen.key, frozen.guard, cx),
            window,
            cx,
        );
    }
}

/// Send `key`'s Stop with `guard` through the operation store, and keep what its reply says on the row. A session that
/// already has a request in flight sends nothing.
pub(crate) fn send(key: &str, guard: SessionStopRequest, cx: &mut App) {
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let Ok(command) = command(key, guard) else {
        tracing::warn!("a session key that cannot be a path segment was not stopped");
        return;
    };
    let stopped = key.to_owned();
    let started = operations.update(cx, |operations, cx| {
        operations.start(&OpKey::session(key), command, cx, move |settled, cx| {
            settle(&stopped, settled, cx)
        })
    });
    if let Err(refused) = started {
        tracing::debug!(?refused, "a Stop was not sent");
    }
}

/// `key`'s Stop settled. A refusal is the operation store's to keep; a success whose reply names cleanup the host could
/// not finish leaves that notice on the row. A success moves no focus and opens nothing: the user stays where they are.
fn settle(key: &str, settled: &Settled, cx: &mut App) {
    let Outcome::Done(body) = &settled.outcome else {
        return;
    };
    if let (Some(words), Some(operations)) =
        (notice(body.as_ref()), session::current_operations(cx))
    {
        operations.update(cx, |operations, cx| {
            operations.note(
                &OpKey::session(key),
                Work::Stop,
                HostText::from_host(words),
                cx,
            )
        });
    }
}

/// The notice a successful Stop's reply carries, if any: its `notice` (a tab the host could not close, in the host's
/// words), or, for a reply that cannot be read, the old app's words saying such a notice may have been lost. An empty
/// reply is an ordinary success.
fn notice(body: Option<&Value>) -> Option<String> {
    let body = body?;
    match serde_json::from_value::<SessionStopOutcome>(body.clone()) {
        Ok(outcome) => outcome.notice,
        Err(error) => Some(format!(
            "Stop completed, but its reply could not be read ({error}), so a notice about terminal tabs left open may \
             have been lost."
        )),
    }
}

// ---- What the row shows ----------------------------------------------------------------------------------------------

/// Whether `key` has a Stop in flight: its row then shows [`progress`] before its title.
pub(crate) fn in_flight(key: &str, cx: &App) -> bool {
    session::current_operations(cx).is_some_and(|operations| {
        operations.read(cx).in_flight(&OpKey::session(key)) == Some(Work::Stop)
    })
}

/// The row's in-progress mark for `key`'s pending Stop: the spinner at `step` and "Stopping…", before the title, in the
/// accent, or in the fill's ink on the selected row (the old `.stop-progress`, which shared `.delete-progress`'s look).
pub(super) fn progress(key: &str, step: u8, selected: bool, cx: &App) -> impl IntoElement {
    let palette = theme::palette(cx);
    let ink = theme::color(if selected {
        palette.on_fill
    } else {
        palette.accent
    });
    div()
        .id(ElementId::Name(
            format!("session-stop-progress-{key}").into(),
        ))
        .debug_selector(|| "session-stop-progress".into())
        .role(Role::Status)
        .aria_label(PROGRESS_LABEL)
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .whitespace_nowrap()
        .text_size(metrics::TEXT_ROW_TITLE)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
        .child(delete::spinner_icon(step, px(12.), ink))
        .child(PROGRESS_LABEL)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wording is the old `stop_consequence` table: an ended agent is never claimed to be stopped, and a tab count
    /// is spelled with its noun's number. Specified: every cell of the table, for no, one and three tabs, live and
    /// ended.
    #[test]
    fn the_consequence_names_the_agent_only_while_it_lives_and_counts_the_tabs() {
        let row = |tabs: usize, ended: bool| -> SessionRow {
            let mut entry = crate::test_support::session_entry("a").value;
            entry["tabs"] = Value::Array(
                (0..tabs)
                    .map(|tab| json!({ "id": format!("t{tab}") }))
                    .collect(),
            );
            entry["status"] = if ended {
                json!({"state": "exited", "exit_code": 0})
            } else {
                json!({"state": "running"})
            };
            serde_json::from_value(entry).unwrap()
        };
        for (tabs, ended, words) in [
            (0, false, "Stop the agent?"),
            (0, true, "Stop this session?"),
            (1, false, "Stop the agent and close 1 terminal tab?"),
            (1, true, "Close 1 terminal tab?"),
            (3, false, "Stop the agent and close 3 terminal tabs?"),
            (3, true, "Close 3 terminal tabs?"),
        ] {
            assert_eq!(
                consequence(&row(tabs, ended)),
                words,
                "{tabs} tabs, ended {ended}"
            );
        }
    }

    /// A Stop's reply is the helm's `SessionStopOutcome`: an empty object for a Stop that closed every tab, and a
    /// notice naming a tab it could not close. The old client also refused to lose that notice silently when the reply
    /// could not be read. Specified: an empty object and no body give no notice; a notice is passed on verbatim; a
    /// reply of the wrong shape gives the old app's words about a possibly lost notice.
    #[test]
    fn only_a_reply_that_names_leftover_cleanup_leaves_a_notice() {
        assert_eq!(notice(Some(&json!({}))), None);
        assert_eq!(notice(None), None);
        assert_eq!(
            notice(Some(
                &json!({"notice": "Terminal 2 could not be closed: host unavailable"})
            ))
            .as_deref(),
            Some("Terminal 2 could not be closed: host unavailable")
        );
        let unread = notice(Some(&json!({"notice": 7}))).unwrap();
        assert!(
            unread.starts_with("Stop completed, but its reply could not be read ("),
            "{unread}"
        );
        assert!(unread.ends_with("may have been lost."), "{unread}");
    }
}
