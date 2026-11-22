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
//!   carries no `intent_key`. It sets `confirm_yolo` only when it is sent again after the user confirmed the YOLO
//!   question ([`super::replace_yolo`]): a host that asks before YOLO launches refuses a YOLO launch's Replace without
//!   it, and that refusal asks the question. Every other refusal shows on the row as described below.
//! - **A host change comes first.** Replace waits for one ([`menu::launch_wait`]).
//! - **A long wait says so.** After two seconds the row shows "Replacing on <host>… <n> s", the host as the helm names
//!   it and the whole seconds since the request went out, moved on once a second ([`super::outcome`]'s clock); a
//!   quick reply adds nothing.
//! - **The replacement opens only where the source was open.** On success the replacement is selected only if the
//!   source is still the selected session when the reply lands, and its terminal takes focus only if the user has not
//!   moved on since the click (SPEC.md, "Who may move focus"); otherwise it just appears in the list. That the source is
//!   still selected when the reply lands, although the helm removed it first, is the selection rules' doing
//!   ([`super::selection`], "A Replace keeps its own source open until it answers"). A refusal shows as "Replace: <the
//!   helm's words>" on the row and in its menu, for as long as [`super::outcome`] says.
//! - **The source's Delete notice goes to the Delete notice band.** The reply's `delete_notice` (the source's Delete
//!   reporting, say, a checkout it could not archive) shows in the list's band ([`super::delete_notice`]), unprefixed,
//!   as the old app showed it and as an ordinary Delete's notice shows, until it is dismissed or a newer notice takes
//!   its place. The source's row is gone by then, and the band is what outlives it.
//!
//! The question, the precondition, the command and the progress line are functions of a session's row, so the open
//! session's header (`session_pane/header.rs`) asks the same question and sends the same request, and so does the
//! interrupted card (`session_pane/card.rs`), inline and without the quoted title. Each names its [`Surface`], which is
//! where a YOLO question for its Replace shows.

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::session_requests::ReplaceSessionBody;
use farhelm_proto::ui_channel::SessionRow;
use farhelm_proto::{DeleteGuard, SessionReplaceReply, SessionStatus};
use gpui::{AnyWindowHandle, App, Context, IntoElement, SharedString, Styled, Window, px};
use serde_json::json;

use super::replace_yolo::{self, Surface};
use super::{SessionList, delete, menu, outcome, selection};
use crate::components::{HoverHelp, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText};
use crate::operations::{Command, HelmMarker, NotStarted, OpKey, Outcome, Settled, Work};
use crate::prompt::{Answer, Question};
use crate::theme;
use crate::{local_state, session};

/// The menu's Replace item's hover help: the old header Replace button's tooltip, which says what the item's short
/// description line ("Fresh conversation, same settings") means.
pub(crate) fn menu_help(row: &SessionRow) -> &'static str {
    if row.info.launch.is_shell() {
        "Replace: start a fresh shell with the same settings"
    } else {
        "Replace: swap this session for a fresh conversation with the same settings"
    }
}

/// How Replace's consequence ends: what takes the source's place, the one clause that tells this question from
/// Delete's.
const SUCCESSOR: &str = "a fresh session with the same settings takes its place:";

// ---- What a Replace of a row asks and sends --------------------------------------------------------------------------

/// What happens to the source's agent, by its status, under the old app's no-guessing rule: a live status says the agent
/// is killed, an unknown one only that it may be, and an ended one claims no kill. Every arm also says the conversation
/// is discarded, because that is what Replace costs even when nothing runs; an agent that never started may still have
/// an earlier run's conversation to resume.
fn replaced_program(row: &SessionRow) -> &'static str {
    if row.info.launch.is_shell() {
        return match row.info.status {
            SessionStatus::Running | SessionStatus::Waiting | SessionStatus::Idle => {
                "still running — replacing stops the shell"
            }
            SessionStatus::Unknown => {
                "status unknown — replacing stops the shell if it is still running"
            }
            SessionStatus::Exited { .. } => "the shell exited",
            SessionStatus::Interrupted => "a host reboot or a Farhelm update ended the shell",
            SessionStatus::Error { .. } => "the shell never started",
        };
    }
    match row.info.status {
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
        &format!("{}; {SUCCESSOR}", replaced_program(row)),
    )
}

/// What the launcher's Replace with shows beside its main action about `row`, its source, and the precondition a press
/// then sends: Replace's consequence ending in "the session launched here takes its place." (the launcher's settings are
/// an edited copy, so "the same settings" would be untrue there), and the Delete guard of the same row. A source showing
/// nothing alive gets no warning and `NothingAlive`, so one restarted behind the user's back is kept rather than killed
/// unannounced (SPEC.md, "Replace with": the launch button "is the confirmation of that delete"). The old launcher's
/// `replace_with_warning` and `replace_with_consequence` at the reference commit; kept here beside plain Replace's
/// wording so the two cannot drift apart.
pub(crate) fn launcher_warning(row: &SessionRow) -> (Option<String>, DeleteGuard) {
    if delete::shows_nothing_alive(row) {
        return (None, DeleteGuard::NothingAlive);
    }
    let warning = delete::with_tabs(
        "replacing",
        row.info.tabs.len(),
        &format!(
            "{}; the session launched here takes its place.",
            replaced_program(row)
        ),
    );
    (Some(warning), delete::guard(row))
}

/// What a confirmed Replace acts on: the source, by the key the helm files it under, the guard of the row the question
/// was drawn from, and the surface it was asked on, where a YOLO question for it would show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Frozen {
    pub(crate) key: String,
    pub(crate) guard: DeleteGuard,
    pub(crate) surface: Surface,
}

/// The row's Replace question for `row`, filed under `key`, asked on `surface` with `guard`: the consequence, then the
/// title quoted on a line of its own, as the old row's question had them. Its answer is "Confirm replace" in the danger
/// tier, and its Cancel says in its help what Cancel keeps, as the old row's did.
///
/// On the interrupted card ([`Surface::Card`]) it is the old card's inline question instead: the consequence alone,
/// since the card stands in the open session's own pane, and no rule for a click on empty space, which SPEC.md says the
/// card's question does not have yet ("Opening and closing") and which an inline question may not carry.
pub(crate) fn question(
    key: &str,
    row: &SessionRow,
    guard: DeleteGuard,
    surface: Surface,
) -> Question<Frozen> {
    let question = Question::new(
        "Replace confirmation",
        Answer::new(
            "Confirm replace",
            HoverHelp::tooltip("Replace this session now, as described"),
            Tier::Danger,
        ),
        Frozen {
            key: key.to_owned(),
            guard,
            surface,
        },
    )
    .line(consequence(row))
    .cancel_answer(Answer::new(
        "Cancel",
        HoverHelp::tooltip("Cancel: keep this session as it is"),
        Tier::Neutral,
    ));
    if surface == Surface::Card {
        return question;
    }
    let title = HostText::from_host(row.info.title.as_str());
    question
        .line(
            HostSentence::new(SharedString::from(format!("session-replace-title-{key}")))
                .text("\"")
                .host(&title)
                .text("\""),
        )
        .keeps_focus_on_empty_click()
}

/// The session's plain Replace, `POST /api/sessions/{id}/replace`, as the route's shared client body
/// ([`ReplaceSessionBody`], which the helm's tests decode into its own request type) with the source's Delete guard as
/// its two flags, and `confirm_yolo` as `consent` says: set only for the Replace sent again after the user confirmed
/// the YOLO question (see the module docs). No `with` (that is Replace with), and no retry key at all.
pub(crate) fn command(
    session: &str,
    guard: DeleteGuard,
    consent: bool,
) -> Result<Command, PathError> {
    let body = ReplaceSessionBody {
        confirm_yolo: consent,
        with: None,
        only_if_nothing_alive: guard.only_if_nothing_alive(),
        only_if_agent_ended: guard.only_if_agent_ended(),
    };
    Ok(Command::new(
        Work::Replace,
        Method::Post,
        ApiPath::api(&["sessions", session, "replace"])?,
        // Two booleans and unset options always encode, so `json!` (which panics on a failed encode) cannot fail here.
        Some(json!(body)),
        "POST /api/sessions/[session]/replace",
    ))
}

// ---- Starting a Replace ----------------------------------------------------------------------------------------------

impl SessionList {
    /// Replace `target`'s row at `mark`: place the question beside its menu button if the row is drawn, or center it.
    /// Skipped confirmation sends the request at once. The menu checks whether the live session accepts a new action.
    pub(crate) fn replace(
        &mut self,
        target: &menu::Actionable,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = &target.key;
        if let Some(ask) = begin(
            key,
            &target.row,
            target.skips_questions,
            Surface::Row,
            window.window_handle(),
            cx,
        ) {
            self.ask_about_row(key, ask, mark, confirm, window, cx);
        }
    }
}

/// Start a Replace of `row`, filed under `key`, on `surface` in `window`, with Skip session confirmations as
/// `skipped`: the one statement of Replace's ask-or-send rule, for the row's menu and the open session's header alike.
/// Replace always asks unless the question is skipped; then it is sent at once with the Delete guard of `row` as drawn,
/// and there is nothing to ask. Otherwise nothing is sent, and what comes back builds the question to ask, whose answer
/// goes to [`confirm`]. Its callers reach it only through [`menu::actionable`], which refuses while the session has a
/// request in flight or its YOLO question is open, so a Replace never races the one that question's answer sends.
pub(crate) fn begin<'a>(
    key: &'a str,
    row: &'a SessionRow,
    skipped: bool,
    surface: Surface,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Option<impl Fn() -> Question<Frozen> + 'a> {
    let guard = delete::guard(row);
    if skipped {
        send(key, guard, surface, window, cx);
        return None;
    }
    Some(move || question(key, row, guard, surface))
}

/// A Replace question was confirmed: send the Replace it froze.
pub(crate) fn confirm(frozen: Frozen, window: &mut Window, cx: &mut App) {
    send(
        &frozen.key,
        frozen.guard,
        frozen.surface,
        window.window_handle(),
        cx,
    );
}

/// One Replace as it was sent, for its completion: the guard it carried, the surface it was asked on, the mark of the
/// user's answer it counts "moved on" from, and whether it carried YOLO consent.
#[derive(Clone, Copy)]
struct Sent {
    guard: DeleteGuard,
    surface: Surface,
    mark: Mark,
    consent: bool,
}

/// Send `key`'s plain Replace with `guard`, asked on `surface` in `window`, for the user's answer now. A YOLO question an
/// earlier Replace of the session met is closed by now (an open one refuses the Replace before its question opens), and
/// the next one asked replaces it.
/// Visible to the crate for the launcher's tests, which put a plain Replace in flight on the session's key.
pub(crate) fn send(
    key: &str,
    guard: DeleteGuard,
    surface: Surface,
    window: AnyWindowHandle,
    cx: &mut App,
) {
    let sent = Sent {
        guard,
        surface,
        mark: Mark::take(cx),
        consent: false,
    };
    start(key, sent, window, cx);
}

/// The user confirmed the YOLO question of `key`'s Replace, asked on `surface` with `guard`, at `answer`: send the same
/// Replace again with consent, its completion settling in `window` and counting "moved on" from that answer's press.
pub(super) fn send_with_consent(
    key: &str,
    guard: DeleteGuard,
    surface: Surface,
    answer: Mark,
    window: AnyWindowHandle,
    cx: &mut App,
) {
    let sent = Sent {
        guard,
        surface,
        mark: answer,
        consent: true,
    };
    start(key, sent, window, cx);
}

/// Send `key`'s Replace as `sent` says through the operation store, its completion settling in `window`, and start its
/// row's clock. A session that already has a request in flight sends nothing, and nothing is sent while a host change
/// is in flight ([`menu::refuse_for_host_change`]). A YOLO question's permanent answer confirmed then has already
/// written its host's setting, which stays written while the Replace waits for the user to ask again. Returns whether
/// it was sent.
fn start(key: &str, sent: Sent, window: AnyWindowHandle, cx: &mut App) -> bool {
    if menu::refuse_for_host_change(key, Work::Replace, cx) {
        return false;
    }
    let source = key.to_owned();
    let started = super::start_request(
        &OpKey::session(key),
        command(key, sent.guard, sent.consent),
        cx,
        move |settled, cx| settle_sent(&source, sent, window, settled, cx),
    );
    match started {
        Ok(()) => {
            outcome::start_clock(key, Work::Replace, cx);
            true
        }
        Err(NotStarted::Retired) => false,
        Err(NotStarted::Path(_)) => {
            tracing::warn!("a session key that cannot be a path segment was not replaced");
            false
        }
        Err(refused) => {
            tracing::debug!(?refused, "a Replace was not sent");
            false
        }
    }
}

/// `source`'s plain Replace, sent as `sent`, settled. A refusal that carries the helm's YOLO marker asks the YOLO
/// question where the Replace was asked ([`replace_yolo`]), which then says it in place of the store's record; any other
/// refusal is the operation store's to keep. A success goes on as [`settle`], counting "moved on" from the user's answer
/// in `sent` as well as from the store's own mark: a Replace sent again with consent counts from the YOLO answer's
/// press, before the store sent it, and the permanent answer's host write ran in between.
///
/// The launcher's Replace with does not come here: its YOLO refusal asks the launcher's own question in the launcher
/// (with its guarded answer), and only its success goes through [`settle`].
fn settle_sent(source: &str, sent: Sent, window: AnyWindowHandle, settled: &Settled, cx: &mut App) {
    if let Outcome::Refused(refusal) = &settled.outcome {
        if refusal.has(HelmMarker::YoloConfirmation)
            && replace_yolo::ask(source, sent.guard, sent.surface, sent.mark, window, cx)
            && let Some(operations) = session::current_operations(cx)
        {
            // Called from the refusal's own completion, so the record is still this refusal's.
            operations.update(cx, |operations, cx| {
                operations.forget(&OpKey::session(source), cx)
            });
        }
        return;
    }
    let settled = Settled {
        outcome: settled.outcome.clone(),
        user_moved_on: settled.user_moved_on || focus::has_moved_on(&sent.mark, cx),
    };
    settle(source, &settled, cx);
}

/// `source`'s Replace settled, from the row's question, the header's or the launcher's Replace with alike. A refusal is
/// not handled here: plain Replace's is [`settle_sent`]'s, and Replace with's the launcher's. A success's reply
/// ([`SessionReplaceReply`]) is the replacement's own session (its `id` the key the helm files it under) and, when the
/// source's Delete carried one, its `delete_notice`, which goes to the Delete notice band whether or not the
/// replacement opens; see the module docs. A reply that does not read so selects nothing and shows no notice.
/// `settled.user_moved_on` is the caller's to combine with the mark of the answer it counts from.
pub(crate) fn settle(source: &str, settled: &Settled, cx: &mut App) {
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
    super::delete_notice::publish(delete_notice, cx);
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
    outcome::in_flight(key, cx) == Some(Work::Replace)
}

/// The line under `row`'s lines while its Replace has waited two seconds or more: [`outcome::wait_line`]'s "Replacing
/// on <host>… <n> s" with the spinner at `step`, in `--fg-0`, or in the fill's ink on the selected row, inset as the
/// row's other lines are. `None` while there is nothing to show.
pub(super) fn progress_line(
    key: &str,
    row: &SessionRow,
    step: u8,
    selected: bool,
    cx: &App,
) -> Option<impl IntoElement> {
    let palette = theme::palette(cx);
    let ink = theme::color(if selected {
        palette.on_fill
    } else {
        palette.fg_0
    });
    // The open session's header announces the wait; its row's copy stays a plain line (see `outcome::wait_line`).
    let line = outcome::wait_line("session", Work::Replace, !selected, key, row, step, ink, cx)?;
    Some(line.pt(px(2.)).pb(px(8.)).px(px(14.)))
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
            let built = command("a", guard, false).unwrap();
            assert_eq!(built.work(), Work::Replace);
            assert_eq!(
                built.path().as_str(),
                "/api/sessions/a/replace",
                "{guard:?}"
            );
        }
    }
}
