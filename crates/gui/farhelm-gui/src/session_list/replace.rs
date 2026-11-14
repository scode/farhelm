//! A session's plain Replace as the session list runs it: the question, the precondition the request carries, what the
//! row shows while it waits, and where the user ends up once the replacement exists.
//!
//! SPEC.md, "Lifecycle operations": Replace creates a new session (new id, fresh conversation, same host, directory,
//! title and launch) and then deletes the source. Ported from the old app's `replace_consequence` and `delete_guard`
//! (`status.rs`), the row's Replace question (`list/row.rs`), `do_replace` (`list/view.rs`) and `LaunchProgress`
//! (`launch_progress.rs`) at the reference commit. Here's the TLDR of the rules:
//!
//! - **Always ask.** Replace asks whatever the agent is doing, unless Skip session confirmations is on ("Replace keeps
//!   its confirmation whatever the agent is doing"); then the click goes ahead with the guard of what the row showed.
//! - **The precondition is Delete's.** The source's removal is a Delete, so the request carries the guard of the row the
//!   question (or the skipped click) was drawn from ([`delete::guard`]): a question that said nothing was alive is
//!   refused if something runs by then, and one that warned only about tabs is refused if the agent runs again. A
//!   refused removal still leaves the replacement, which the helm's words name.
//! - **No retry key.** The GUI writes no retry keys (SPEC_impl.md, "One failure path: the reset"), so the request
//!   carries no `intent_key`; nor `confirm_yolo`, since the YOLO question belongs to the launcher area. A host that asks
//!   before YOLO launches refuses a YOLO launch's Replace, and that refusal shows like any other.
//! - **A long wait says so.** After two seconds the row shows "Replacing on <host>… <n> s", the host as the helm names
//!   it and the whole seconds since the request went out, moved on once a second ([`super::outcome`]'s clock); a
//!   quick reply adds nothing.
//! - **The replacement opens only where the source was open.** On success the replacement is selected only if the
//!   source is still the selected session when the reply lands, and its terminal takes focus only if the user has not
//!   moved on since the click (SPEC.md, "Who may move focus"); otherwise it just appears in the list. That the source is
//!   still selected when the reply lands, although the helm removed it first, is the selection rules' doing
//!   ([`super::selection`], "A Replace keeps its own source open until it answers"). A refusal shows as "Replace: <the
//!   helm's words>" on the row and in its menu, for as long as [`super::outcome`] says.
//! - **The source's Delete notice moves to the replacement.** The old app showed the reply's `delete_notice` in its
//!   list-wide delete-notice band, which the port does not have yet. The source's row is gone by then, so the notice
//!   shows on the replacement's row as "Replace: <notice>", for as long as [`super::outcome`] says. It is a notice of
//!   this Replace like Stop's of its Stop, named by its operation the same way.
//!
//! The question, the precondition, the command and the progress line are functions of a session's row, so the open
//! session's header and the interrupted card (later pieces) can ask the same question and send the same request.

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::ui_channel::SessionRow;
use farhelm_proto::{DeleteGuard, SessionReplaceReply, SessionStatus};
use gpui::{
    App, Context, ElementId, InteractiveElement, IntoElement, ParentElement, Role, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div, px,
};
use serde_json::json;

use super::outcome;
use super::{SessionList, delete, menu, selection};
use crate::components::{HoverHelp, Tier};
use crate::focus::Mark;
use crate::host_text::{HostSentence, HostText};
use crate::operations::{Command, OpKey, Outcome, Settled, Work};
use crate::prompt::{Answer, Question};
use crate::theme::{self, metrics};
use crate::{local_state, session};

/// The menu's Replace item's hover help: the old header Replace button's tooltip, which says what the item's short
/// description line ("Fresh conversation, same settings") means.
pub(super) const MENU_HELP: &str =
    "Replace: swap this session for a fresh conversation with the same settings";

/// How Replace's consequence ends: what takes the source's place, the one clause that tells this question from
/// Delete's.
const SUCCESSOR: &str = "a fresh session with the same settings takes its place:";

// ---- What a Replace of a row asks and sends --------------------------------------------------------------------------

/// What happens to the source's agent, by its status, under the old app's no-guessing rule: a live status says the agent
/// is killed, an unknown one only that it may be, and an ended one claims no kill. Every arm also says the conversation
/// is discarded, because that is what Replace costs even when nothing runs; an agent that never started may still have
/// an earlier run's conversation to resume.
fn agent_clause(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Running | SessionStatus::Waiting | SessionStatus::Idle => {
            "still running — replacing kills the agent and discards the conversation"
        }
        SessionStatus::Unknown => {
            "status unknown — the agent may still be running and will be killed, and the conversation is discarded \
             either way"
        }
        SessionStatus::Exited { .. } => "replacing discards the conversation",
        SessionStatus::Interrupted => {
            "interrupted by a host reboot or a Farhelm update, which ended the agent, but replacing still discards the \
             conversation"
        }
        SessionStatus::Error { .. } => {
            "the agent never started, but replacing still discards any conversation the session could resume"
        }
    }
}

/// The question's consequence for `row`: its open tabs, its agent, then what takes its place, in the shape Delete's
/// consequence has.
pub(crate) fn consequence(row: &SessionRow) -> String {
    delete::with_tabs(
        "replacing",
        row.info.tabs.len(),
        &format!("{}; {SUCCESSOR}", agent_clause(&row.info.status)),
    )
}

/// What a confirmed Replace acts on: the source, by the key the helm files it under, and the guard of the row the
/// question was drawn from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Frozen {
    pub(crate) key: String,
    pub(crate) guard: DeleteGuard,
}

/// The row's Replace question for `row`, filed under `key`, asked with `guard`: the consequence, then the title quoted
/// on a line of its own, as the old row's question had them. Its answer is "Confirm replace" in the danger tier, and its
/// Cancel says in its help what Cancel keeps, as the old row's did.
pub(crate) fn question(key: &str, row: &SessionRow, guard: DeleteGuard) -> Question<Frozen> {
    let title = HostText::from_host(row.info.title.as_str());
    Question::new(
        "Replace confirmation",
        Answer {
            label: "Confirm replace".into(),
            help: HoverHelp::tooltip("Replace this session now, as described"),
            tier: Tier::Danger,
        },
        Frozen {
            key: key.to_owned(),
            guard,
        },
    )
    .line(consequence(row))
    .line(
        HostSentence::new(SharedString::from(format!("session-replace-title-{key}")))
            .text("\"")
            .host(&title)
            .text("\""),
    )
    .cancel_answer(Answer {
        label: "Cancel".into(),
        help: HoverHelp::tooltip("Cancel: keep this session as it is"),
        tier: Tier::Neutral,
    })
    .keeps_focus_on_empty_click()
}

/// The session's plain Replace, `POST /api/sessions/{id}/replace`, with the source's Delete guard as its two flags and
/// nothing else: no `with` (that is Replace with), no `intent_key` and no `confirm_yolo` (see the module docs).
pub(crate) fn command(session: &str, guard: DeleteGuard) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Replace,
        Method::Post,
        ApiPath::api(&["sessions", session, "replace"])?,
        // The route's request type is the helm's `ReplaceReq`, which also carries Replace with's create request; it
        // moves into the protocol crate with the launcher's create request, and until then the two flags go by hand.
        Some(json!({
            "only_if_nothing_alive": guard.only_if_nothing_alive(),
            "only_if_agent_ended": guard.only_if_agent_ended(),
        })),
        "POST /api/sessions/[session]/replace",
    ))
}

// ---- Starting a Replace ----------------------------------------------------------------------------------------------

impl SessionList {
    /// The user asked to replace `key` (its row's Replace) at `mark`: ask, beside the row's menu button while the row is
    /// drawn and centered when it is not, or replace at once when confirmations are skipped. Nothing happens for a
    /// session that is not listed, or that has any request in flight.
    pub(crate) fn replace(
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
        let guard = delete::guard(row);
        if target.skips_questions {
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

/// Send `key`'s Replace with `guard` through the operation store and start its row's clock. A session that already has
/// a request in flight sends nothing.
pub(crate) fn send(key: &str, guard: DeleteGuard, cx: &mut App) {
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let Ok(command) = command(key, guard) else {
        tracing::warn!("a session key that cannot be a path segment was not replaced");
        return;
    };
    let source = key.to_owned();
    let started = operations.update(cx, |operations, cx| {
        operations.start(&OpKey::session(key), command, cx, move |settled, cx| {
            settle(&source, settled, cx)
        })
    });
    match started {
        Ok(()) => outcome::start_clock(key, cx),
        Err(refused) => tracing::debug!(?refused, "a Replace was not sent"),
    }
}

/// `source`'s Replace settled. A refusal is the operation store's to keep. A success's reply ([`SessionReplaceReply`])
/// is the replacement's own session (its `id` the key the helm files it under) and, when the source's Delete carried
/// one, its `delete_notice`; see the module docs for where each goes. A reply that does not read so selects nothing.
fn settle(source: &str, settled: &Settled, cx: &mut App) {
    let Outcome::Done(body) = &settled.outcome else {
        return;
    };
    // A copy of the body, because decoding from a borrowed `Value` needs serde's `Deserialize` trait, which this crate
    // does not depend on directly.
    let reply = body
        .clone()
        .map(serde_json::from_value::<SessionReplaceReply>);
    let Some(Ok(SessionReplaceReply {
        session: replacement,
        delete_notice,
    })) = reply
    else {
        tracing::warn!("a Replace succeeded with a reply that does not read as its replacement");
        return;
    };
    let replacement = replacement.id.as_str();
    if let (Some(words), Some(operations)) = (delete_notice, session::current_operations(cx)) {
        let key = OpKey::session(replacement);
        operations.update(cx, |operations, cx| {
            // The note lands on the replacement's key, not on the key of the work it reports, so newer work may run
            // there already (the helm lists the replacement before it deletes the source, and a Stop or Rename of it
            // can start meanwhile). A note then would outlive that work, which the store's `note` forbids and which
            // would break `outcome`'s "until the next operation starts"; the notice is dropped instead.
            if operations.in_flight(&key).is_none() {
                operations.note(&key, Work::Replace, HostText::from_host(words), cx);
            }
        });
    }
    if local_state::selected(cx).as_deref() != Some(source) {
        return;
    }
    if settled.user_moved_on {
        selection::open_automatically(replacement, cx);
    } else {
        selection::choose(replacement, cx);
    }
}

// ---- What the row shows ----------------------------------------------------------------------------------------------

/// Whether `key` has a Replace in flight.
pub(crate) fn in_flight(key: &str, cx: &App) -> bool {
    session::current_operations(cx).is_some_and(|operations| {
        operations.read(cx).in_flight(&OpKey::session(key)) == Some(Work::Replace)
    })
}

/// The line under `row`'s lines while its Replace has waited two seconds or more: the spinner at `step`, "Replacing on
/// <host>…" with the host as the helm names it (escaped, as its own element; "this host" when the helm names none, as
/// in the old app), then the whole seconds. The old `.launch-progress`: 12 pixels in `--fg-0`, or in the fill's ink on
/// the selected row, the spinner in the text's color. `None` while there is nothing to show.
pub(super) fn progress_line(
    key: &str,
    row: &SessionRow,
    step: u8,
    selected: bool,
    cx: &App,
) -> Option<impl IntoElement> {
    let seconds = outcome::replace_seconds(key, cx)?;
    let palette = theme::palette(cx);
    let ink = theme::color(if selected {
        palette.on_fill
    } else {
        palette.fg_0
    });
    let mut label = HostSentence::new(SharedString::from(format!(
        "session-replace-progress-label-{key}"
    )))
    .text("Replacing on ");
    label = if row.host_name.is_empty() {
        label.text("this host")
    } else {
        label.host(&HostText::from_host(row.host_name.as_str()))
    };
    let label = label.text("…");
    let name = label.accessible_name();
    Some(
        div()
            .id(ElementId::Name(
                format!("session-replace-progress-{key}").into(),
            ))
            .debug_selector(|| "session-replace-progress".into())
            .role(Role::Status)
            .aria_label(name)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .min_w_0()
            .pt(px(2.))
            .pb(px(8.))
            .px(px(14.))
            .text_size(metrics::TEXT_SM)
            .text_color(ink)
            .child(delete::spinner_icon(step, px(12.), ink))
            .child(div().flex_1().min_w_0().child(label))
            .child(div().flex_none().child(format!("{seconds} s"))),
    )
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    /// The consequence is the old `replace_consequence`: the tab clause in Delete's shape, the agent clause by status,
    /// and the clause saying what takes the source's place, which is what tells this question from Delete's.
    /// Specified: a running session with no tabs, an exited one with one tab, and an unknown one with two.
    #[test]
    fn the_consequence_warns_about_tabs_and_the_agent_then_names_the_replacement() {
        let row = |tabs: usize, status: Value| -> SessionRow {
            let mut entry = crate::test_support::session_entry("a").value;
            entry["tabs"] = Value::Array(
                (0..tabs)
                    .map(|tab| json!({ "id": format!("t{tab}") }))
                    .collect(),
            );
            entry["status"] = status;
            serde_json::from_value(entry).unwrap()
        };
        assert_eq!(
            consequence(&row(0, json!({"state": "running"}))),
            "Still running — replacing kills the agent and discards the conversation; a fresh session with the same \
             settings takes its place:"
        );
        assert_eq!(
            consequence(&row(1, json!({"state": "exited", "exit_code": 0}))),
            "1 terminal tab is still open, and replacing closes it with everything running in it; replacing discards \
             the conversation; a fresh session with the same settings takes its place:"
        );
        assert_eq!(
            consequence(&row(2, json!({"state": "unknown"}))),
            "2 terminal tabs are still open, and replacing closes them with everything running in them; status \
             unknown — the agent may still be running and will be killed, and the conversation is discarded either \
             way; a fresh session with the same settings takes its place:"
        );
    }

    /// A Replace goes to its source's own replace route as the Replace work, whatever its guard; what the body holds is
    /// checked where the window sends it (`lifecycle_tests.rs`), since a command's body is the operation store's.
    /// Specified: for each guard, `Work::Replace` on `/api/sessions/<key>/replace`.
    #[test]
    fn a_replace_goes_to_its_sources_replace_route() {
        for guard in [
            DeleteGuard::Unconditional,
            DeleteGuard::AgentEnded,
            DeleteGuard::NothingAlive,
        ] {
            let built = command("a", guard).unwrap();
            assert_eq!(built.work(), Work::Replace);
            assert_eq!(
                built.path().as_str(),
                "/api/sessions/a/replace",
                "{guard:?}"
            );
        }
    }
}
