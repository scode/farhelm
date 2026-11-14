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
//!   request was sent with ([`Work::Delete`]) and a refusal until the session's next operation starts, and the row reads
//!   both back from it while drawing ([`in_flight`], [`failure_line`]).
//! - **A pending Delete shows as an expected wait.** The row shows a spinner and what is happening in place of its
//!   title, in the accent rather than in danger red (the user just confirmed this, so the wait is the outcome they asked
//!   for), and in the fill's ink on the selected row. The title stays the row's accessible name. The spinner turns one
//!   45-degree step per animation tick, eight steps per 0.8-second turn as the old `steps(8)` animation did, from one
//!   handle the list holds ([`Spinner`]) while a Delete shows and its window is active.
//!
//! While the open session's Delete is in flight, the main pane says the same over its terminal and holds back the
//! terminal's own notices (`session_pane.rs`), reading the same [`in_flight`] and [`progress_label`] the row does and
//! turning its own [`Spinner`].
//!
//! NOTE: The open session's header, whose Delete the shortcut stands for in SPEC.md, is not ported; until it is, the
//! shortcut asks the row's question.

use std::f32::consts::TAU;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::DeleteGuard;
use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    App, AppContext as _, Context, Div, ElementId, Entity, FontWeight, Hsla, InteractiveElement,
    IntoElement, ParentElement, Pixels, Role, SharedString, StatefulInteractiveElement as _,
    Styled, Subscription, Svg, Transformation, Window, div, px, radians, svg,
};

use super::SessionList;
use super::menu::Opened;
use crate::assets::Icon;
use crate::components::{HoverHelp, Tier};
use crate::focus::Mark;
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::Subject;
use crate::operations::{Command, OpKey, Work};
use crate::prompt::{self, Answer, Question};
use crate::theme::{self, metrics};
use crate::{local_state, session, ticker};

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

/// The question's consequence: what happens to the session's terminal tabs, when it has any, then what happens to its
/// agent. The three live statuses share one wording on purpose (the agent's whole process tree dies whatever it was
/// doing), an unknown status says only that the agent may be running, and an ended one never claims a kill. The ended
/// wordings are reached when the question is about open tabs of an ended agent.
pub(super) fn consequence(row: &SessionRow) -> String {
    use farhelm_proto::SessionStatus;
    let agent = match &row.info.status {
        SessionStatus::Running | SessionStatus::Waiting | SessionStatus::Idle => {
            "still running — deleting kills the agent:"
        }
        SessionStatus::Unknown => {
            "status unknown — the agent may still be running and will be killed:"
        }
        SessionStatus::Exited { .. } => "delete anyway:",
        SessionStatus::Interrupted => {
            "interrupted by a host reboot or a Farhelm update, which ended the agent; deleting discards the session:"
        }
        SessionStatus::Error { .. } => "the agent never started; deleting discards the session:",
    };
    match row.info.tabs.len() {
        0 => {
            let mut chars = agent.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        }
        1 => format!(
            "1 terminal tab is still open, and deleting closes it with everything running in it; {agent}"
        ),
        tabs => format!(
            "{tabs} terminal tabs are still open, and deleting closes them with everything running in them; {agent}"
        ),
    }
}

/// What a confirmed Delete acts on: the session, by the key the helm files it under, and the guard of the row the
/// question was drawn from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Frozen {
    pub(super) key: String,
    pub(super) guard: DeleteGuard,
}

/// The row's Delete question for `row`, filed under `key`, asked with `guard`: the consequence first, then the title
/// quoted on a line of its own (so a long title can never push the consequence out of view), then, for a session in a
/// managed checkout, what happens to the checkout. Ported from the old row's question with its wording; Cancel keeps
/// the prompt's own wording.
pub(super) fn question(key: &str, row: &SessionRow, guard: DeleteGuard) -> Question<Frozen> {
    let title = HostText::from_host(row.info.title.as_str());
    let mut question = Question::new(
        "Delete confirmation",
        Answer {
            label: "Confirm delete".into(),
            help: HoverHelp::tooltip("Delete this session and its state now"),
            tier: Tier::Danger,
        },
        Frozen {
            key: key.to_owned(),
            guard,
        },
    )
    .line(consequence(row))
    .line(
        HostSentence::new(SharedString::from(format!("session-delete-title-{key}")))
            .text("\"")
            .host(&title)
            .text("\""),
    )
    .keeps_focus_on_empty_click();
    // Whether the checkout moves is decided by the supervisor from every session that uses it, so this says what
    // happens in either case rather than counting sessions the client may have stale.
    if row.info.working_copy.is_some() {
        question = question.line(
            "The managed checkout stays while another session uses it. When its last session is deleted, its folder \
             moves to the trash; no files are deleted.",
        );
    }
    question
}

// ---- Starting a Delete -----------------------------------------------------------------------------------------------

impl SessionList {
    /// The Delete shortcut (SPEC.md, "Desktop window chrome": Cmd+W on a Mac, Ctrl+Shift+Backspace on Linux): the open
    /// session's Delete, with the row's confirmation and busy rules. With no session open it does nothing, and while a
    /// dialog, menu or question is open it does nothing either, since that layer keeps the keyboard. It runs once per
    /// press (the action is in `actions::ONCE_PER_PRESS`), so its caller must take the key even when this does nothing.
    pub fn delete_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if crate::layers::any_open(window, cx) {
            return;
        }
        let Some(key) = local_state::selected(cx) else {
            return;
        };
        let mark = Mark::take(cx);
        self.delete(&key, &mark, window, cx);
    }

    /// The user asked to delete `key` (its row's Delete, or the shortcut), at `mark`: delete it at once when nothing
    /// shows alive or confirmations are skipped, and otherwise ask, beside the row's menu button while the row is in
    /// the list's view and centered when it is not (scrolled away, or not listed). Nothing happens for a session that
    /// is not listed, or that has any request in flight. Visible to the crate for the screenshot goldens, which ask the
    /// question without a click.
    pub(crate) fn delete(
        &mut self,
        key: &str,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = session::current_store(cx) else {
            return;
        };
        let collections = store.read(cx).collections();
        let Some(row) = collections.get::<SessionRow>(key).cloned() else {
            return;
        };
        if busy(key, cx) {
            return;
        }
        let guard = guard(&row);
        let skipped = super::selection::preferences(collections)
            .is_some_and(|preferences| preferences.skip_session_confirmations == Some(true));
        if shows_nothing_alive(&row) || skipped {
            send_delete(key, guard, cx);
            return;
        }
        let on_confirm = |frozen: Frozen, _: &mut Window, cx: &mut App| {
            send_delete(&frozen.key, frozen.guard, cx);
        };
        // Beside the row's menu button only while the row is in the list's view: a row scrolled away may still be
        // drawn just past the list's edge, and a question beside it would sit next to nothing the user can see.
        let anchor = if self.row_in_view(key) {
            self.current.current(cx).rows.borrow().anchor(key)
        } else {
            None
        };
        let asked = anchor.and_then(|anchor| {
            prompt::ask_anchored(
                question(key, &row, guard),
                anchor,
                Subject::session(key, cx),
                mark,
                on_confirm,
                window,
                cx,
            )
        });
        let asked = asked.or_else(|| {
            prompt::ask(
                question(key, &row, guard),
                Subject::session(key, cx),
                mark,
                on_confirm,
                window,
                cx,
            )
        });
        if let Some(layer) = asked {
            self.current
                .current(cx)
                .rows
                .borrow_mut()
                .opened(key, layer, Opened::Question);
        }
    }
}

/// The ordinary session Delete, `DELETE /api/sessions/{id}`, with the precondition the user's confirmation covered:
/// `guard` is what that confirmation showed, so a session that became more alive since is refused rather than killed.
/// Only a guard that asks for something adds its flag, as in the old app's `api::delete_url`.
///
/// NOTE: The helm refuses this for a session in a disposable environment, whose Delete states what happens to the
/// environment and asks for it with a flag of its own. That refusal is correct, not a bug; the area that ports
/// environment sessions adds their Delete.
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
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let Ok(command) = command(key, guard) else {
        tracing::warn!("a session key that cannot be a path segment was not deleted");
        return;
    };
    let started = operations.update(cx, |operations, cx| {
        operations.start(&OpKey::session(key), command, cx, |_, _| {})
    });
    if let Err(refused) = started {
        tracing::debug!(?refused, "a Delete was not sent");
    }
}

// ---- What the row shows ----------------------------------------------------------------------------------------------

/// Whether `key` has any request in flight, a Rename as much as a Delete: then its Delete is refused, and its menu
/// draws Delete unavailable.
pub(super) fn busy(key: &str, cx: &App) -> bool {
    session::current_operations(cx).is_some_and(|operations| {
        operations
            .read(cx)
            .in_flight(&OpKey::session(key))
            .is_some()
    })
}

/// The guard of `key`'s Delete while it is in flight, as the request carries it; `None` when no Delete is. For the
/// row's progress, and for the open session's pane once it draws the Delete over its terminal.
pub(crate) fn in_flight(key: &str, cx: &App) -> Option<DeleteGuard> {
    let operations = session::current_operations(cx)?;
    let Work::Delete(guard) = operations.read(cx).in_flight(&OpKey::session(key))? else {
        return None;
    };
    Some(guard)
}

/// What a pending Delete says it is doing, from the guard it was sent with: stopping the agent, stopping only the
/// terminal tabs of an agent that has ended, or, with nothing alive, removing the session's state. It never claims to
/// stop what the question did not show running. The row and the main pane's scrim say the same words.
pub(crate) fn progress_label(guard: DeleteGuard) -> &'static str {
    match guard {
        DeleteGuard::Unconditional => "Stopping agent…",
        DeleteGuard::AgentEnded => "Stopping tabs…",
        DeleteGuard::NothingAlive => "Deleting…",
    }
}

/// How many steps the spinner's turn has: `steps(8)` in the old animation.
const SPINNER_STEPS: u8 = 8;

/// The spinner's turn, which the list holds while any row shows a pending Delete and its window is active, as it holds
/// the running pulse, and drops otherwise: a live animation handle keeps the ticker's timer firing ten times a second,
/// which an inactive window has no use for. Every pending row draws the same step, so one handle serves them all. The
/// main pane holds one of its own, on the same terms, for its scrim.
///
/// The step is counted from the ticker's notifications of the turn's own entity: the ticker notifies it once per tick
/// and nothing else does, so the count is the number of ticks it has seen. The view that holds the spinner observes
/// the entity to redraw.
pub(crate) struct Spinner {
    turn: Entity<Turn>,
    _redraw: Subscription,
}

/// See [`Spinner`].
pub(super) struct Turn {
    step: u8,
    _animating: ticker::Animating,
    _ticks: Subscription,
}

impl Spinner {
    /// A spinner starting at its first step, redrawing the view `cx` belongs to on every step.
    pub(crate) fn start<V: 'static>(window: &Window, cx: &mut Context<V>) -> Spinner {
        let turn = cx.new(|cx| Turn {
            step: 0,
            _animating: ticker::animate(window, cx),
            _ticks: cx
                .observe_self(|turn: &mut Turn, _| turn.step = (turn.step + 1) % SPINNER_STEPS),
        });
        Spinner {
            _redraw: cx.observe(&turn, |_, _, cx| cx.notify()),
            turn,
        }
    }

    /// The step to draw, `0..8`.
    pub(crate) fn step(&self, cx: &App) -> u8 {
        self.turn.read(cx).step
    }
}

/// The row's in-progress mark for `key`'s pending Delete, sent with `guard`: the spinner at `step` and
/// [`progress_label`], drawn where the title would be. In the accent, or in the fill's ink on the selected row
/// (`.session-row.selected .delete-progress`), where the accent would vanish into the accent fill.
pub(super) fn progress(
    key: &str,
    guard: DeleteGuard,
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
        .aria_label(progress_label(guard))
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
        .child(progress_label(guard))
}

/// The spinner's icon at `step`, `size` square, in `ink`: the row draws it at the old `.delete-spinner`'s 12 pixels,
/// and the pane's scrim at its card's 16.
pub(crate) fn spinner_icon(step: u8, size: Pixels, ink: Hsla) -> Svg {
    svg()
        .path(Icon::Spinner.path())
        .size(size)
        .flex_none()
        .text_color(ink)
        .with_transformation(Transformation::rotate(radians(
            f32::from(step) * TAU / f32::from(SPINNER_STEPS),
        )))
}

/// The last refusal of `key`'s work, as the row shows it under its lines until the session's next operation starts:
/// the helm's words, which may quote a host, in `.action-error`'s 12-pixel red, or in the fill's ink on the selected
/// row (`.session-row.selected .action-error`).
pub(super) fn failure_line(key: &str, selected: bool, cx: &App) -> Option<Div> {
    let operations = session::current_operations(cx)?;
    let message = operations
        .read(cx)
        .failure(&OpKey::session(key))?
        .message
        .clone();
    Some(failure_text(
        &message,
        refusal_ink(selected, theme::palette(cx)),
    ))
}

/// The ink of the row's refusal line: `.action-error`'s red, or the fill's ink on the selected row, where red on the
/// accent fill is hard to read.
fn refusal_ink(selected: bool, palette: &theme::Palette) -> theme::Token {
    if selected {
        palette.on_fill
    } else {
        palette.danger
    }
}

/// A refusal's words in `.action-error`'s look and `ink`, for the row and its menu alike.
pub(super) fn failure_text(message: &HostText, ink: theme::Token) -> Div {
    host_text::element(message)
        .pt(px(2.))
        .pb(px(8.))
        .px(px(14.))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(ink))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// A refusal on the selected row was once drawn red on the accent fill, near unreadable; the old sheet switched it
    /// to the fill's ink (`.session-row.selected .action-error`). Specified, in both palettes: the fill's ink when
    /// selected, `--danger` otherwise.
    #[test]
    fn a_refusal_on_the_selected_row_takes_the_fills_ink() {
        for palette in [&theme::DARK, &theme::LIGHT] {
            assert_eq!(refusal_ink(true, palette), palette.on_fill);
            assert_eq!(refusal_ink(false, palette), palette.danger);
        }
    }
}
