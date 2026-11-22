//! A session's Delete as the session list runs it: whether to ask, what the question says, the precondition the
//! request carries, and what the row shows from the moment the request is sent until the helm answers.
//!
//! SPEC.md, "Lifecycle operations". Ported from the old app's `status.rs` (`confirm_consequence`,
//! `shows_nothing_alive`, `delete_guard`, `delete_progress_label`), the row's Delete question in `list/row.rs` and the
//! stylesheet's `.delete-progress` and `.delete-spinner` rules at the reference commit. Here's the TLDR of the rules:
//!
//! - **Ask only when something is alive.** A session whose agent has ended and that lists no terminal tab is deleted at
//!   once, with [`DeleteGuard::NothingAlive`], so the supervisor refuses if the row was stale and something runs after
//!   all. Anything alive asks, unless the helm-wide **Skip session confirmations** preference is on, and then the click
//!   goes ahead with the guard of what the row showed.
//! - **The answer authorizes only what the question said.** The guard is chosen from the same row the question was
//!   drawn from and frozen into the question with it ([`Frozen`]); confirming sends that guard, never one recomputed at
//!   the click from a row that may have moved on (SPEC.md, "Confirmed 2026-10-01").
//! - **The operation store carries the request.** Delete is refused while the session has any request in flight, a
//!   Rename included (SPEC.md: "a pending request refuses another lifecycle action"). The store keeps the guard the
//!   request was sent with ([`Work::Delete`]) and its refusal, and the row reads both back from it while drawing
//!   ([`in_flight`], and the row's line in [`super::outcome`], which says how long a refusal stays).
//! - **A pending Delete shows as an expected wait.** The row shows a spinner and what is happening in place of its
//!   title, in the accent rather than in danger red (the user just confirmed this, so the wait is the outcome they asked
//!   for), and in the fill's ink on the selected row. The title stays the row's accessible name. The spinner is the
//!   component layer's stepped one ([`Spinner`](crate::components::Spinner)), turned by one handle the list holds
//!   while a Delete shows and its window is active.
//!
//! While Delete is in flight, the header and terminal scrim use the same label from [`in_flight`] as the row.
//! The pane holds back the terminal's own notices (`session_pane.rs`) and turns its own
//! [`Spinner`](crate::components::Spinner).
//!
//! A success's own report outlives the row it was about: a notice the helm's reply carries (a checkout that could not
//! be archived, which SPEC.md says is never silent) goes to the list's Delete notice band ([`super::delete_notice`]),
//! from [`start`]'s completion, which every ordinary Delete of this window passes through (an environment Delete's
//! completion is its own, `super::environment_delete`'s).
//!
//! A session in a disposable environment gets its own Delete in place of this one ([`super::environment_delete`]):
//! [`begin`] always asks it, in the lane the hosts read picks, [`question`] words it, and the answer
//! ([`confirming`]) sends it, with a way to ask the warned whole-environment question again at the same row or header
//! when the helm could not ask the environment's own Farhelm.
//!
//! The open session's header asks its own Delete question (`session_pane/header.rs`), and the Delete shortcut asks it
//! through the header, from these same functions ([`begin`]): beside the header's Delete button, and even for a session
//! with nothing alive, as SPEC.md's header Delete does. A bulk Delete of the marked sessions (`super::bulk`) sends each
//! of them the request its row would, through [`start`], with its guard and consequence from the same functions, and
//! an environment session's through `super::environment_delete`'s own start.
//!
//! **This window's own Delete is evidence.** The selection rules drop a selected session once a complete, unfiltered
//! listing proves it gone (`session::listing_proves_gone`), or once an available listing without it answers a query
//! that named it, with no cap having cut it away (`selection::absent_while_named`), which is how the open session a
//! filter hides is found deleted, here or elsewhere. A Delete this window sent that the helm did ([`deleted_here`]),
//! with the session gone from the store, counts as proof too, whatever query the helm serves. It is what proves a
//! filtered listing's loss of a session that the query in force does not name: a later query is in flight (the user
//! chose a filter while the Delete was on its way), or the helm refused the query naming it (`filters`'
//! `Choice::named`). Without it the pane would stay on the deleted session until the next reply, or the next query. A
//! Rename editor or a question about a hidden session deleted by an agent or the command line still stays open, since a
//! layer keeps the complete-listing rule, and the helm refuses what it then sends.

use std::collections::HashSet;
use std::rc::Rc;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::ui_channel::SessionRow;
use farhelm_proto::{DeleteGuard, SessionDeleteOutcome};
use gpui::{
    App, BorrowAppContext as _, Context, ElementId, FontWeight, Global, InteractiveElement,
    IntoElement, ParentElement, Role, SharedString, StatefulInteractiveElement as _, Styled,
    Window, div, px,
};
use serde_json::Value;

use super::environment_delete::{self, AskAgain};
use super::{SessionList, delete_notice, menu, outcome};
use crate::components::{HoverHelp, Tier, spinner_icon};
use crate::focus::Mark;
use crate::host_text::{HostSentence, HostText};
use crate::operations::{Command, NotStarted, OpKey, Outcome, Work};
use crate::prompt::{Answer, Question};
use crate::session::{self, PerSession};
use crate::theme::{self, metrics};

// ---- What a Delete of a row asks and sends ---------------------------------------------------------------------------

/// Whether `row` shows nothing of the session still running: its agent has ended and it lists no terminal tab. Only
/// then does Delete go ahead without asking.
pub(super) fn shows_nothing_alive(row: &SessionRow) -> bool {
    row.info.status.has_ended() && row.info.tabs.is_empty()
}

/// The precondition a Delete sends when it goes ahead on `row` as drawn: what the question drawn from that row (or the
/// absence of one) told the user would be stopped. Nothing alive is [`DeleteGuard::NothingAlive`]; an ended agent with
/// tabs, a question that warned only about the tabs, is [`DeleteGuard::AgentEnded`]; an agent that has not ended, or
/// whose status is unknown, is [`DeleteGuard::Unconditional`].
pub(super) fn guard(row: &SessionRow) -> DeleteGuard {
    if shows_nothing_alive(row) {
        DeleteGuard::NothingAlive
    } else if row.info.status.has_ended() {
        DeleteGuard::AgentEnded
    } else {
        DeleteGuard::Unconditional
    }
}

/// The question names what Delete will do to the session's terminal tabs and its agent or shell. The three live
/// statuses share one wording because Delete kills the whole process tree in each case. Unknown status says the
/// program may still be running. The ended wordings apply when the question is about remaining terminal tabs.
pub(super) fn consequence(row: &SessionRow) -> String {
    use farhelm_proto::SessionStatus;
    // A shell session runs a shell where others run an agent; Stop and Replace name it the same way.
    let program = super::stop::program_word(row);
    let agent = match &row.info.status {
        SessionStatus::Running | SessionStatus::Waiting | SessionStatus::Idle => {
            format!("still running — deleting kills the {program}:")
        }
        SessionStatus::Unknown => {
            format!("status unknown — the {program} may still be running and will be killed:")
        }
        SessionStatus::Exited { .. } => "delete anyway:".to_owned(),
        SessionStatus::Interrupted => format!(
            "interrupted by a host reboot or a Farhelm update, which ended the {program}; deleting discards the session:"
        ),
        SessionStatus::Error { .. } => {
            format!("the {program} never started; deleting discards the session:")
        }
    };
    with_tabs("deleting", row.info.tabs.len(), &agent)
}

/// A destructive question's consequence for a session with `tabs` terminal tabs: the tab clause first, saying that
/// `verb` ("deleting", "replacing") closes them with everything in them, then the `agent` clause after a semicolon; with
/// no tabs, the agent clause alone, capitalized. Delete and Replace share the shape, as the old app's `with_tabs` made
/// them, so the two questions cannot drift apart in how they warn about tabs. The agent clauses are the app's own
/// lowercase words, never host text, so capitalizing the first letter is safe.
pub(super) fn with_tabs(verb: &str, tabs: usize, agent: &str) -> String {
    match tabs {
        0 => {
            let mut chars = agent.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        }
        1 => format!(
            "1 terminal tab is still open, and {verb} closes it with everything running in it; {agent}"
        ),
        tabs => format!(
            "{tabs} terminal tabs are still open, and {verb} closes them with everything running in them; {agent}"
        ),
    }
}

/// What a confirmed Delete acts on: the session, by the key the helm files it under, the guard of the row the question
/// was drawn from, and, for a session in a disposable environment, the environment Delete the question asked for
/// ([`environment_delete::Asked`]), which sends no guard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Frozen {
    pub(crate) key: String,
    pub(crate) guard: DeleteGuard,
    pub(crate) environment: Option<environment_delete::Asked>,
}

/// The Delete question for `row`, freezing `frozen`: the consequence first, then the title quoted on a line of its own
/// (so a long title can never push the consequence out of view), then, for a session in a managed checkout, what
/// happens to the checkout. Ported from the old row's question with its wording; Cancel keeps the prompt's own wording.
/// An environment session's question is its lane's instead: the lane's consequence and the environment's name, the
/// quoted title, no checkout line, and the lane's answer ([`environment_delete`]). The whole lane starts on Cancel
/// because it can destroy other sessions and their work. That also gives the delayed re-ask its harmless first answer.
pub(crate) fn question(row: &SessionRow, frozen: Frozen) -> Question<Frozen> {
    let key = frozen.key.clone();
    let title = HostText::from_host(row.info.title.as_str());
    let environment = frozen.environment.clone();
    let answer = match &environment {
        Some(asked) => asked.answer(),
        None => Answer::new(
            "Confirm delete",
            HoverHelp::tooltip("Delete this session and its state now"),
            Tier::Danger,
        ),
    };
    let question = Question::new("Delete confirmation", answer, frozen);
    let question = match &environment {
        Some(asked) => {
            let question = question.line(asked.consequence(&key));
            if asked.lane == environment_delete::Lane::Whole {
                question.starts_on_cancel()
            } else {
                question
            }
        }
        None => question.line(consequence(row)),
    };
    let question = question
        .line(
            HostSentence::new(SharedString::from(format!("session-delete-title-{key}")))
                .text("\"")
                .host(&title)
                .text("\""),
        )
        .keeps_focus_on_empty_click();
    // Whether the checkout moves is decided by the supervisor from every session that uses it, so this says what
    // happens in either case rather than counting sessions the client may have stale.
    if environment.is_none() && row.info.working_copy.is_some() {
        question.line(MANAGED_CHECKOUT)
    } else {
        question
    }
}

/// What a Delete question says about a session in a managed checkout, the row's and the bulk question's alike
/// (`super::bulk`).
pub(super) const MANAGED_CHECKOUT: &str = "The managed checkout stays while another session uses it. When its last \
                                           session is deleted, its folder moves to the trash; no files are deleted.";

// ---- Starting a Delete -----------------------------------------------------------------------------------------------

impl SessionList {
    /// Delete `target`'s row at `mark`: delete it at once when nothing shows alive or
    /// confirmations are skipped, and otherwise ask, beside the row's menu button while the row is in the list's view
    /// and centered when it is not (scrolled away). A session in a disposable environment is always asked ([`begin`]).
    /// Called after checking that the live session accepts a new action. The screenshot goldens ask without a click.
    pub(crate) fn delete(
        &mut self,
        target: &menu::Actionable,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(ask) = begin(target, false, cx) {
            let again = self.ask_again_at_row(window, cx);
            self.ask_about_row(&target.key, ask, mark, confirming(Some(again)), window, cx);
        }
    }

    /// How an environment Delete asked at a row asks the whole-lane question at the same row once the helm answers
    /// "unasked" (`environment_delete`'s "Settling"): beside the row's menu button while the row is in view, centered
    /// otherwise, as the first question was, and only while the session's commands are not refused. Its answer sends
    /// the whole lane, which never asks again.
    fn ask_again_at_row(&self, window: &Window, cx: &Context<Self>) -> AskAgain {
        let (list, handle) = (cx.weak_entity(), window.window_handle());
        Rc::new(move |frozen: Frozen, mark: Mark, cx: &mut App| {
            let Some(target) = menu::actionable(&frozen.key, cx) else {
                return false;
            };
            let list = list.clone();
            handle
                .update(cx, move |_, window, cx| {
                    let key = frozen.key.clone();
                    list.update(cx, |list, cx| {
                        list.ask_about_row(
                            &key,
                            || question(&target.row, frozen.clone()),
                            &mark,
                            confirm,
                            window,
                            cx,
                        )
                    })
                    .unwrap_or(false)
                })
                .unwrap_or(false)
        })
    }
}

/// Whether the drawn Delete requires confirmation, before anything is sent. The header and its shortcut ask even
/// when the row shows nothing alive, unless the preference skips questions. The row menu can also skip on that
/// premise. Both always ask for an environment Delete, in either lane.
pub(super) fn asks(target: &menu::Actionable, ask_when_nothing_alive: bool) -> bool {
    target.environment_delete.is_some()
        || (!target.skips_questions
            && (ask_when_nothing_alive || !shows_nothing_alive(&target.row)))
}

/// Start a Delete from the activated control's snapshot. If it [`asks`], defer the request and return the question
/// builder for [`confirm`] or [`confirming`]. Otherwise send at once with the guard of the row as drawn.
pub(crate) fn begin<'a>(
    target: &'a menu::Actionable,
    ask_when_nothing_alive: bool,
    cx: &mut App,
) -> Option<impl Fn() -> Question<Frozen> + 'a> {
    let (key, row) = (&target.key, &target.row);
    let guard = guard(row);
    let environment = &target.environment_delete;
    if !asks(target, ask_when_nothing_alive) {
        send_delete(key, guard, cx);
        return None;
    }
    Some(move || {
        question(
            row,
            Frozen {
                key: key.to_owned(),
                guard,
                environment: environment.clone(),
            },
        )
    })
}

/// A Delete question was confirmed: send the Delete it froze, an environment Delete without a way to ask its whole
/// lane again (for a question that is already the whole lane's, or a surface that cannot ask).
pub(crate) fn confirm(frozen: Frozen, window: &mut Window, cx: &mut App) {
    confirming(None)(frozen, window, cx);
}

/// The answer of a Delete question asked where `again` can ask the whole-lane question once more: it sends the Delete
/// the question froze, the ordinary one or the environment one, and an environment Delete the helm answers "unasked"
/// asks through `again` ([`environment_delete::AskAgain`]).
pub(crate) fn confirming(
    again: Option<AskAgain>,
) -> impl FnOnce(Frozen, &mut Window, &mut App) + Clone + 'static {
    move |frozen: Frozen, _: &mut Window, cx: &mut App| {
        if frozen.environment.is_some() {
            environment_delete::send(frozen, again, cx);
        } else {
            send_delete(&frozen.key, frozen.guard, cx);
        }
    }
}

/// The ordinary session Delete, `DELETE /api/sessions/{id}`, with the precondition the user's confirmation covered:
/// `guard` is what that confirmation showed, so a session that became more alive since is refused rather than killed.
/// Only a guard that asks for something adds its flag, as in the old app's `api::delete_url`.
///
/// The helm refuses this for a session in a disposable environment, whose Delete states what happens to the environment
/// and asks for it with a flag of its own ([`environment_delete::command`]). The row's, the header's, the shortcut's
/// and the bulk Delete send that one instead.
pub(crate) fn command(session: &str, guard: DeleteGuard) -> Result<Command, PathError> {
    let path = ApiPath::api(&["sessions", session])?;
    let flag = match guard {
        DeleteGuard::NothingAlive => Some("only_if_nothing_alive"),
        DeleteGuard::AgentEnded => Some("only_if_agent_ended"),
        DeleteGuard::Unconditional => None,
    };
    let (path, shown) = match flag {
        Some(flag) => (
            path.with_query(&[(flag, "true")])?,
            format!("DELETE /api/sessions/[session]?{flag}=true"),
        ),
        None => (path, "DELETE /api/sessions/[session]".to_owned()),
    };
    Ok(Command::new(
        Work::Delete(guard),
        Method::Delete,
        path,
        None,
        shown,
    ))
}

/// Send `key`'s Delete with `guard` through the operation store. A session that already has a request in flight sends
/// nothing: SPEC.md lets each session exclude its own competing actions.
fn send_delete(key: &str, guard: DeleteGuard, cx: &mut App) {
    if let Err(refused) = start(key, guard, |_| {}, cx) {
        tracing::debug!(?refused, "a Delete was not sent");
    }
}

/// [`send_delete`] for a caller that has to know what happened: the bulk Delete (`super::bulk`), which shows a refused
/// start on the row and moves its host's queue on when each Delete settles. Says why nothing was sent, and otherwise
/// runs `then` once the Delete has settled, after a success was recorded as this window's own ([`deleted_here`]), its
/// trash cue played and its notice, if the reply carried one, put in the list's Delete notice band
/// ([`delete_notice`]); a reset drops `then` with the Delete's own waiting.
///
/// Every ordinary Delete this window sends goes through here, so this completion is the one place its result reaches
/// the band: the row's, the header's, the shortcut's and the bulk Delete's alike. An environment Delete settles in
/// `super::environment_delete`, which reaches the band its own way.
pub(super) fn start(
    key: &str,
    guard: DeleteGuard,
    then: impl FnOnce(&mut App) + 'static,
    cx: &mut App,
) -> Result<(), NotStarted> {
    let deleted = key.to_owned();
    super::start_request(
        &OpKey::session(key),
        command(key, guard),
        cx,
        move |settled, cx| {
            if let Outcome::Done(reply) = &settled.outcome {
                record_deleted_here(&deleted, cx);
                let outcome = decode(reply.as_ref());
                crate::trash::cue::archived(outcome.as_ref(), cx);
                // A checkout the host could not archive is never silent (SPEC.md, "Managed checkouts").
                delete_notice::publish(outcome.and_then(|outcome| outcome.notice), cx);
            }
            then(cx);
        },
    )
    .inspect_err(|refused| {
        if let NotStarted::Path(_) = refused {
            tracing::warn!("a session key that cannot be a path segment was not deleted");
        }
    })
}

/// A Delete's success `reply`, the route's own answer, read as its outcome ([`SessionDeleteOutcome`]): `None` for no
/// reply or a body that does not read as one. Decoded once per completion, for both the trash cue and the band, and
/// for an environment Delete's count (`environment_delete`).
pub(super) fn decode(reply: Option<&Value>) -> Option<SessionDeleteOutcome> {
    reply.and_then(|reply| serde_json::from_value::<SessionDeleteOutcome>(reply.clone()).ok())
}

/// Record that this window's Delete of `key` succeeded under the current UI session ([`deleted_here`]): from the
/// ordinary Delete's completion, and from an environment Delete's, a 404 included (`environment_delete`).
pub(super) fn record_deleted_here(key: &str, cx: &mut App) {
    cx.update_default_global::<DeletedHere, _>(|here, cx| {
        here.0.current(cx).insert(key.to_owned());
    });
}

/// The sessions this window deleted under the current UI session: every Delete it sent that the helm did (a 2xx other
/// than 202). Per UI session, since a reset's new store is read afresh and a key from an earlier one proves nothing about
/// it. Kept app-wide like the selection itself ([`crate::local_state`]), which assumes one window.
#[derive(Default)]
struct DeletedHere(PerSession<HashSet<String>>);

impl Global for DeletedHere {}

/// Whether this window's own Delete of `key` succeeded under the current UI session (the module docs' "This window's
/// own Delete is evidence"). It says nothing about whether the session is still in the store: the selection rules pair
/// it with the store holding no row for `key`.
///
/// NOTE: The list's settle on the Delete's own settling sees the record only because GPUI runs the operation store's
/// observers when the update that settled the Delete returns, after its completion recorded the key here.
/// `session_pane/carried_tests.rs`'s `this_windows_delete_proves_its_session_gone_while_a_later_query_is_in_flight`
/// fails if that order changes, or if this record stops counting.
pub(crate) fn deleted_here(key: &str, cx: &mut App) -> bool {
    cx.update_default_global::<DeletedHere, _>(|here, cx| here.0.current(cx).contains(key))
}

// ---- What the row shows ----------------------------------------------------------------------------------------------

/// Whether `key` has any request in flight, whatever its kind, or is held for a bulk Stop or Delete not sent yet
/// ([`crate::operations::Operations::hold`]): then every lifecycle command of the session (Rename, Replace, Stop and
/// Delete) is refused ([`super::menu::actionable`]) and its menu draws them unavailable, as SPEC.md's "a pending
/// request refuses another lifecycle action" asks.
pub(crate) fn busy(key: &str, cx: &App) -> bool {
    outcome::in_flight(key, cx).is_some()
}

/// The row, header and terminal scrim all describe the same pending Delete. The guard says which processes the
/// request is allowed to stop, even while a bulk Delete is held before sending. The session's launch names the
/// program the unconditional Delete stops. A filtered or truncated listing can omit an open session, so the
/// pending label uses "Stopping…" if the program is unavailable.
///
/// Environment Delete has no request guard, so its label follows the current session row, or says "Deleting…"
/// when that row is gone.
pub(crate) fn in_flight(key: &str, cx: &App) -> Option<&'static str> {
    let work = outcome::in_flight(key, cx)?;
    let store = session::current_store(cx);
    let row = store
        .as_ref()
        .and_then(|store| store.read(cx).collections().get::<SessionRow>(key));
    let guard = match work {
        Work::Delete(guard) => guard,
        Work::EnvironmentDelete => row.map(guard).unwrap_or(DeleteGuard::NothingAlive),
        _ => return None,
    };
    Some(match guard {
        DeleteGuard::Unconditional => match row {
            Some(row) if row.info.launch.is_shell() => "Stopping shell…",
            Some(_) => "Stopping agent…",
            None => "Stopping…",
        },
        DeleteGuard::AgentEnded => "Stopping tabs…",
        DeleteGuard::NothingAlive => "Deleting…",
    })
}

/// The row's in-progress mark for `key`'s pending Delete: the spinner at `step` and the label from [`in_flight`],
/// drawn where the title would be. In the accent, or in the fill's ink on the selected row
/// (`.session-row.selected .delete-progress`), where the accent would vanish into the accent fill.
pub(super) fn progress(
    key: &str,
    label: &'static str,
    step: u8,
    selected: bool,
    cx: &App,
) -> impl IntoElement {
    let palette = theme::palette(cx);
    let ink = theme::color(if selected {
        palette.on_fill
    } else {
        palette.accent
    });
    div()
        .id(ElementId::Name(
            format!("session-delete-progress-{key}").into(),
        ))
        .debug_selector(|| "session-delete-progress".into())
        .role(Role::Status)
        .aria_label(label)
        .flex()
        .flex_row()
        .flex_1()
        .min_w_0()
        .items_center()
        .gap(px(6.))
        .whitespace_nowrap()
        .text_size(metrics::TEXT_ROW_TITLE)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
        .child(spinner_icon(step, px(12.), ink))
        .child(label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_list::tests::session;
    use serde_json::json;

    /// Delete's question must agree with the session row about what is running. A running, unknown or never-started
    /// Bash session names the shell in the consequence. A running agent session still names the agent.
    #[test]
    fn a_shell_sessions_delete_question_names_the_shell() {
        let bash = json!({
            "kind": "agent",
            "selection": {"harness": "bash", "permissions": null},
            "start": ["bash", "{farhelm_args}"],
            "resume": null,
        });
        let shell = session("a").launch(bash.clone()).row();
        assert_eq!(
            consequence(&shell),
            "Still running — deleting kills the shell:"
        );
        let unknown = session("a")
            .launch(bash.clone())
            .status(json!({"state": "unknown"}))
            .row();
        assert_eq!(
            consequence(&unknown),
            "Status unknown — the shell may still be running and will be killed:"
        );
        let failed = session("a")
            .launch(bash)
            .status(json!({"state": "error", "detail": "no such program"}))
            .row();
        assert_eq!(
            consequence(&failed),
            "The shell never started; deleting discards the session:"
        );
        assert_eq!(
            consequence(&session("a").row()),
            "Still running — deleting kills the agent:"
        );
    }

    /// A Delete must ask for exactly the precondition its confirmation showed, or a session that became more alive
    /// since would be killed instead of refused (the old app's `api::delete_url`). Specified: an unconditional Delete
    /// carries no flag, and each guard that asks for something carries its own flag, on the session's own path.
    #[test]
    fn each_delete_guard_asks_for_its_own_precondition() {
        for (guard, path) in [
            (DeleteGuard::Unconditional, "/api/sessions/a"),
            (
                DeleteGuard::AgentEnded,
                "/api/sessions/a?only_if_agent_ended=true",
            ),
            (
                DeleteGuard::NothingAlive,
                "/api/sessions/a?only_if_nothing_alive=true",
            ),
        ] {
            let built = command("a", guard).unwrap();
            assert_eq!(built.work(), Work::Delete(guard));
            assert_eq!(built.path().as_str(), path, "{guard:?}");
        }
    }
}
