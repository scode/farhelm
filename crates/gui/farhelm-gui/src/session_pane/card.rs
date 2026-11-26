//! The recovery card stands in for the open session's terminals when its agent was interrupted or its terminal
//! is missing. Both cases use the same lifecycle controls, with an explanation specific to the row.
//!
//! SPEC.md, "Durability and resume": "Opening an interrupted session shows a centered neutral card in the empty terminal
//! area explaining that a host reboot or a Farhelm update ended the agent and that it needs an intentional restart. The
//! card offers Restart when the session can resume its conversation (see Lifecycle operations), and Replace, which
//! starts a fresh session under the same settings and keeps its inline destructive confirmation; a session that cannot
//! resume offers only Replace and says why." Ported from the old app's `.interrupted-card` (`session_view.rs` and
//! `app.css` at the reference commit). Here's the TLDR of the rules:
//!
//! - **When.** The card takes a reachable session whose row says Interrupted or whose supervisor reports no agent
//!   terminal ([`shows`]), unless a Restart has just brought it back (below). An unreachable host has its own card,
//!   and retired sessions have their own rules. While recovery shows, the pane drops its terminal set, including
//!   the tab strip. The top row and details bar stay, so the session's metadata remains visible.
//! - **Restarted, until the list says so.** The helm's reply to a Restart comes before its list stops saying
//!   Interrupted: the supervisor's reply carries an Unknown status, which the helm's session cache never lets overwrite
//!   a definite one, and the real status arrives with the host's next listing, after the reply. So a Restart that
//!   publishes a terminal marks its session restarted ([`restart_finished`]), and the card treats it as recovered: it goes
//!   on the reply and the pane mounts the new terminal. The card's Restart goes with it, so a second click there cannot
//!   send a second Restart into the gap; the header's Restart stays available meanwhile, as the old header's did. The
//!   mark goes once the store no longer calls for a recovery card. In particular, an exited row whose terminal is
//!   still listed as missing retains the mark until terminal presence is refreshed. An unreachable host clears it,
//!   so a later report of absence can show the recovery card again.
//! - **The text** says what happened and what the card offers ([`text`]), from the same clauses as Restart's hover help
//!   (`session_list::restart`), so the two cannot drift apart.
//! - **Restart** is one click: the card shows only when no agent process is left, so there is nothing to stop and
//!   nothing is asked, even when the last status the helm kept still reads working (a terminal that went missing
//!   leaves the last definite status in place). It is drawn only when the conversation can be resumed. **Replace**
//!   asks its inline question in the card, unless Skip session confirmations is on, through `replace::begin` and `confirm` as the row and the header do.
//!   A legacy launch (a session from before launch kinds) cannot be replaced as it is, so its card offers the
//!   launcher's Replace with in Replace's place, as the old card did; once the launcher opens, the sidebar shows
//!   Sessions in place of the Hosts panel, a New handoff like the header's Replace with… (`super::header`). Restart
//!   and Replace start a session, so they wait for a host change as the header's and the row's do
//!   (`session_list::menu::launch_wait`). Replace with opens the launcher, so it waits whenever the launcher cannot
//!   open (`session_list::menu::launcher_wait`: a pending launch as well as a host change), as the row's and the
//!   header's Replace with do, saying why in its hover help.
//! - **While it asks.** The card's open Replace question is a lifecycle confirmation like any other, so it refuses the
//!   session's other lifecycle actions (SPEC.md: "an open lifecycle confirmation or a pending request refuses another
//!   lifecycle action"): the header's, the row menu's and the card's own Restart. It is inline, so a press elsewhere
//!   does not close it as it closes the row's and the header's anchored questions; they have to refuse on their own.
//!   So the question is kept here, under the UI session and the session it was asked for, rather than by the pane
//!   alone, and [`asks`] is the fact `session_list::menu::refused` reads with only `&App`. The fact goes when the
//!   question closes by any path: an answer (Escape is Cancel) closes it, which [`asks`] reads from the prompt itself,
//!   and the pane drops it unanswered ([`drop_question`]) whenever the selection moves (the session deleted elsewhere
//!   included) and whenever the card does not show: the session recovered, or its host dropped away, which hides the
//!   card and with it the question. A card that shows again starts with no question, as the old card's did. A reset
//!   makes a new UI session, under which nothing kept earlier is read.
//! - **The lines move in.** While the card shows, a long Restart's or Replace's wait, the session's refusal or notice,
//!   and the YOLO question its Replace met are drawn in the card and not below the top row, so nothing shows twice; the
//!   wait line is the one copy announced, as the header's is otherwise.
//! - **Focus.** The card is home while it shows (SPEC.md, "Where focus rests": focus rests "on a stopped session's card
//!   when the selected session has stopped"; the migration's orchestrator decided on 2026-11-14 that the stopped
//!   session's card is this one): a tab stop with nothing bound to Enter, so typing already under way cannot restart
//!   anything, with Restart and Replace the next stops after it. The old card was never a stop; SPEC.md's rule was
//!   written after the old app. It is the component layer's focus stop (`components::focus_stop`) on a handle the pane
//!   keeps, so the pane can make it home before its first frame. It has no hover help: a tooltip over the whole card
//!   would cover the explanation it is there to show.

use std::collections::HashSet;

use farhelm_proto::ui_channel::SessionRow;
use farhelm_proto::{RestartOffer, SessionLaunch, SessionStatus};
use gpui::{
    AnyElement, App, BorrowAppContext as _, Context, FocusHandle, Global, InteractiveElement as _,
    IntoElement, ParentElement, Role, Styled, Subscription, WeakEntity, Window, div, px, relative,
};

use super::SessionPane;
use crate::components::{self, Button, HoverHelp, Ring, Tier};
use crate::focus::Mark;
use crate::host_text::HostSentence;
use crate::operations::Work;
use crate::prompt::{self, Inline, ShownAction};
use crate::session::{self, ObservingCurrent, PerSession};
use crate::session_list::replace_yolo::{self, Surface};
use crate::session_list::{self, menu, outcome, replace, restart};
use crate::theme::{self, metrics};
use crate::{launcher, local_state};

/// `.interrupted-card`'s `max-width: 42rem`. The retired card has the same look (`super::retired`).
pub(super) const MAX_WIDTH: f32 = 672.;
/// `.interrupted-card`'s `padding: 2rem`.
pub(super) const PAD: f32 = 32.;
/// `.interrupted-card`'s `gap`.
pub(super) const GAP: f32 = 12.;

// ---- When the card shows, and what it says ---------------------------------------------------------------------------

/// The two explanations the recovery card can show. The follower tracks the reason to refresh the explanation
/// even when the pane keeps the same card and focus handle.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reason {
    Interrupted,
    MissingTerminal,
}

/// A recovery card needs a reachable, operable session. Interruption carries a more specific explanation than
/// the missing-terminal fact, which can accompany any status.
fn reason(row: &SessionRow) -> Option<Reason> {
    if row.stale || session_list::retired::is(row) {
        return None;
    }
    if row.info.status == SessionStatus::Interrupted {
        Some(Reason::Interrupted)
    } else {
        row.info.terminal_missing.then_some(Reason::MissingTerminal)
    }
}

/// The row's recovery reason unless a successful Restart already supplied a new terminal.
fn shown_reason(key: &str, row: &SessionRow, cx: &App) -> Option<Reason> {
    reason(row).filter(|_| !is_restarted(key, cx))
}

/// Whether the open session filed under `key` shows its recovery card.
pub(crate) fn shows_row(key: &str, row: &SessionRow, cx: &App) -> bool {
    shown_reason(key, row, cx).is_some()
}

/// [`shows_row`] for the row the current UI session's store holds under `key`. `false` while the store holds none (a
/// selection the helm has not answered for yet), which gets a terminal as before.
pub(crate) fn shows(key: &str, cx: &App) -> bool {
    row(key, cx).is_some_and(|row| shows_row(key, &row, cx))
}

/// Whether the store holds a row for the session filed under `key` on a host the helm reaches. The pane mounts
/// terminals that the card no longer stands in for only then. A missing row is not reachable: it is the moment between
/// the helm removing a Replace's source and its reply, while the selection stays on the source, and terminals mounted
/// then would attach to a session that is gone.
pub(crate) fn reachable(key: &str, cx: &App) -> bool {
    row(key, cx).is_some_and(|row| !row.stale)
}

/// Whether the current UI session's store shows that there are no sessions at all ([`session_list::empty_fleet`]: an
/// available, complete, unfiltered listing with no rows). That is also the proof the pane needs to make Start a
/// session home (SPEC.md, "Where focus rests"). Keep it beside [`Follow`] so the pane and its follower use the same
/// evidence.
pub(super) fn empty_fleet(cx: &App) -> bool {
    session::current_store(cx)
        .is_some_and(|store| session_list::empty_fleet(store.read(cx).collections()))
}

/// The row the current UI session's store holds under `key`.
fn row(key: &str, cx: &App) -> Option<SessionRow> {
    session::current_store(cx)?
        .read(cx)
        .collections()
        .get::<SessionRow>(key)
        .cloned()
}

// ---- Restarted sessions ----------------------------------------------------------------------------------------------

/// The sessions a Restart relaunched while the store may still give a reason to show the recovery card. The marks
/// belong to their UI session (see "Restarted, until the list says so"). A global, because the Restart's completion
/// marks it from the operation store, with no view at hand, and the follower and the pane read it. The marked keys are
/// read through [`PerSession::read`], since [`is_restarted`] answers while drawing, with only `&App`; they are written
/// through `current` inside a global update, which takes the global off the app for the closure, so `current` can
/// borrow the app to read the connection.
#[derive(Default)]
struct Restarted(PerSession<HashSet<String>>);

impl Global for Restarted {}

/// A Restart reply can bridge a stale listing as soon as it reports a terminal. A later reply reporting absence
/// revokes that evidence, including when Restart came from the header during the gap.
pub(crate) fn restart_finished(key: &str, terminal_missing: bool, cx: &mut App) {
    if terminal_missing {
        if is_restarted(key, cx) {
            cx.update_global::<Restarted, _>(|marks, cx| {
                marks.0.current(cx).remove(key);
            });
        }
    } else {
        cx.update_default_global::<Restarted, _>(|marks, cx| {
            marks.0.current(cx).insert(key.to_owned());
        });
    }
}

/// Whether the session filed under `key` is marked restarted under the current UI session.
fn is_restarted(key: &str, cx: &App) -> bool {
    cx.try_global::<Restarted>()
        .and_then(|marks| marks.0.read(cx))
        .is_some_and(|keys| keys.contains(key))
}

/// Drop marks as soon as the row has no recovery reason or leaves the listing. A mark matters only while the card
/// would show. The open session always has a row unless it is gone. A row that left the listing (a filter, the cap,
/// another session opened) could otherwise keep its mark until a later terminal loss and then hide that card. Changes
/// the global (which notifies its observers) only when a mark goes, since the follower that calls this observes the global.
///
/// Marks of an earlier UI session are left alone: no read finds them ([`is_restarted`] reads through
/// [`PerSession::read`]), and the next mark starts them over. Dropping them here would notify the follower of a change
/// nothing can see.
fn forget_settled(cx: &mut App) {
    let Some(keys) = cx
        .try_global::<Restarted>()
        .and_then(|marks| marks.0.read(cx))
    else {
        return;
    };
    let gone: Vec<String> = keys
        .iter()
        .filter(|key| row(key, cx).is_none_or(|row| reason(&row).is_none()))
        .cloned()
        .collect();
    if gone.is_empty() {
        return;
    }
    cx.update_global::<Restarted, _>(|marks, cx| {
        // The marks were just read under the current UI session, so this starts nothing over.
        let keys = marks.0.current(cx);
        for key in gone {
            keys.remove(&key);
        }
    });
}

// ---- The card's question ---------------------------------------------------------------------------------------------

/// The Replace question the open session's card asked, with the key of the session it is about, under the UI session it
/// was asked in (see the module docs, "While it asks"). A global, because the row menu and the header read it through
/// `session_list::menu::refused`, with only `&App` and no pane at hand; the pane is its one writer ([`keep_question`],
/// [`drop_question`]). One question at most, since only the open session shows a card.
///
/// The global exists only while it holds a question: [`keep_question`] makes it and [`drop_question`] removes it. It is
/// read through [`PerSession::read`], so a question a reset left behind is never drawn or counted under the new UI
/// session, even before the pane drops it.
#[derive(Default)]
struct Asked(PerSession<Option<(String, Inline<replace::Frozen>)>>);

impl Global for Asked {}

/// Keep `question`, which the card of the session filed under `key` just asked, in place of any earlier one.
fn keep_question(key: &str, question: Inline<replace::Frozen>, cx: &mut App) {
    cx.update_default_global::<Asked, _>(|asked, cx| {
        *asked.0.current(cx) = Some((key.to_owned(), question));
    });
}

/// Drop the card's question, unanswered if it is still open: the pane calls this whenever it stops showing the card
/// the question belongs to. Dropping is what ends the fact then, since a question dropped unanswered still reads open
/// for any clone of it (`prompt::Inline::is_open`), this one included. A question kept under an earlier UI session goes
/// too, as it always has: removing the whole global needs no comparison with the session, and since the global exists
/// only while it holds a question, the pane's calls while none is kept change nothing.
pub(super) fn drop_question(cx: &mut App) {
    if cx.has_global::<Asked>() {
        cx.remove_global::<Asked>();
    }
}

/// The question the card of the session filed under `key` asked, if one is kept for it under the current UI session,
/// open or answered: what the card draws while it is open.
pub(super) fn question(key: &str, cx: &App) -> Option<Inline<replace::Frozen>> {
    let (asked_for, question) = cx.try_global::<Asked>()?.0.read(cx)?.as_ref()?;
    (asked_for == key).then(|| question.clone())
}

/// Whether the recovery card of the session filed under `key` asks its Replace question now: one is kept for it
/// under the current UI session and has had no answer. While it does, the session's lifecycle commands are refused
/// everywhere (`session_list::menu::refused`).
pub(crate) fn asks(key: &str, cx: &App) -> bool {
    question(key, cx).is_some_and(|question| question.is_open(cx))
}

/// Whether `row`'s card offers the launcher's Replace with in place of plain Replace: only for a row that says its
/// launch is legacy, which plain Replace cannot copy as it is (the old card's `card_offers_replace_with`).
fn offers_replace_with(row: &SessionRow) -> bool {
    matches!(row.info.launch, SessionLaunch::Legacy { .. })
}

/// Name the reason the pane has no terminal and what each recovery action does. An interrupted session keeps the
/// explanation about a reboot or update. The missing-terminal report does not identify a cause.
/// A hook setup error stays separate from the card's recovery instructions when it is drawn.
fn text(row: &SessionRow) -> HostSentence {
    let offer = row.info.restart_offer;
    let lead = match row.info.status {
        SessionStatus::Interrupted if row.info.launch.is_shell() => {
            "A host reboot or a Farhelm update ended this session's shell"
        }
        SessionStatus::Interrupted if offer.can_restart() => {
            "A host reboot or a Farhelm update ended this session's agent; it needs an intentional restart"
        }
        SessionStatus::Interrupted => {
            "A host reboot or a Farhelm update ended this session's agent"
        }
        _ => {
            "This session's terminal is gone: it ended outside Farhelm (for example when its container was stopped), \
             never started, or could not be found on its host"
        }
    };
    // The source's launch kind determines which Replace action the card can actually offer.
    let legacy = offers_replace_with(row);
    let replace = if legacy { "Replace with" } else { "Replace" };
    let restart = if offer.can_restart() {
        "Restart or "
    } else {
        ""
    };
    let sentence = HostSentence::new("session-recovery-reason").text(format!(
        "{lead}. Farhelm will wait for you to choose {restart}{replace} — "
    ));
    let sentence = if !legacy || offer.can_restart() {
        restart::offer_clause(sentence, row, true, false)
    } else if offer == RestartOffer::NotCaptured {
        let start_over = "Replace with opens the launcher with its command to start it over";
        restart::not_captured_clause(sentence, &row.info, true, start_over, false)
    } else {
        sentence.text(
            "this session was created before launch kinds, so it cannot resume, and Replace with opens the launcher with its command to start it over",
        )
    };
    sentence
        .text(".")
        .text(restart::feedback_note(&row.info, true))
}

// ---- Following the store ---------------------------------------------------------------------------------------------

/// What tells the pane that the open session's recovery card or its reason changed, or its host came into or out of reach
/// (and so into or out of the unavailable card, `unavailable.rs`), or that its row turned out to be a retired
/// containerized session's (`retired.rs`), so it can drop, mount or make inert its terminals then, and that the fleet
/// became empty or stopped being empty, so it can make Welcome's Start a session home or stop: the store pushes the change (or a Restart
/// marks the session), and nothing else the pane observes (the selection, the user's choices, the connection) moves.
/// It also drops the restarted marks the store has caught up with ([`forget_settled`]).
///
/// It is built on [`session::observe_current`], which cannot serve the pane by itself: its callback gets no window,
/// which every focus move the pane makes needs, and it calls on every pushed change, where the pane wants only these
/// transitions. So the pane observes this entity with its window, as it observes the terminal set that follows the
/// session's tabs, and this entity notifies only when the open session, its shown recovery reason, the
/// state of its row's host (reached, not reached, or no row at all), whether it is retired, or whether the fleet is
/// empty changes. The two unreached states are told apart because a stale row shows the unavailable card and a missing
/// one does not, so a row that goes from one to the other changes what the pane does with its terminals although
/// neither is reachable. The fleet's emptiness is followed on its own, apart from the open session, because it can
/// change with nothing open and the selection staying as it was: a failed or cut listing, which opens nothing, coming
/// back complete and empty.
pub(super) struct Follow {
    /// The open session, its card's reason if it shows, whether its row is stale (`None` with no row), and
    /// whether it is a retired containerized session, which never gets terminals (`super::retired`).
    last: Option<(String, Option<Reason>, Option<bool>, bool)>,
    /// Whether the store showed an empty fleet at the last check ([`empty_fleet`]).
    empty: bool,
    _following: ObservingCurrent,
    _marks: Subscription,
}

impl Follow {
    /// The follower, which reads the current UI session's store and the restarted marks from the next update on.
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        Follow {
            last: None,
            empty: false,
            _following: session::observe_current(cx, |follow: &mut Self, cx| follow.check(cx)),
            _marks: cx.observe_global::<Restarted>(|follow, cx| follow.check(cx)),
        }
    }

    /// Drop the settled marks, then notify when the open session, its shown recovery reason, its host state,
    /// retirement or the fleet's emptiness changes.
    fn check(&mut self, cx: &mut Context<Self>) {
        forget_settled(cx);
        let now = local_state::selected(cx).map(|key| {
            // One read of the row for both answers, so they describe the same push.
            let row = row(&key, cx);
            let card = row.as_ref().and_then(|row| shown_reason(&key, row, cx));
            let retired = row.as_ref().is_some_and(session_list::retired::is);
            let stale = row.map(|row| row.stale);
            (key, card, stale, retired)
        });
        let empty = empty_fleet(cx);
        if now != self.last || empty != self.empty {
            self.last = now;
            self.empty = empty;
            cx.notify();
        }
    }
}

// ---- Drawing ---------------------------------------------------------------------------------------------------------

/// What the card draws for the open session filed under `key`: its row as the store holds it, the focus handle the
/// card tracks (home while it shows), the question it asked if one is kept, why the launcher cannot open now
/// if it cannot, and the spinner's step.
pub(crate) struct Drawn<'a> {
    pub(crate) key: &'a str,
    pub(crate) row: &'a SessionRow,
    /// Shared with the header. A held card can still draw after its session is missing from the store, but has no
    /// live action snapshot until the row returns.
    pub(crate) action: Option<ShownAction<menu::Actionable>>,
    pub(crate) focus: &'a FocusHandle,
    pub(crate) question: Option<Inline<replace::Frozen>>,
    /// [`menu::launcher_wait`], read by the caller, which has the `&mut App` it needs (the pane's render, as for the
    /// header's `Shown::launcher_wait`), so that drawing keeps to `&App`.
    pub(crate) launcher_wait: Option<&'static str>,
    pub(crate) step: u8,
}

/// The card for `drawn`, centered in the area the terminals would take, its buttons acting through `pane`. Visible to
/// the crate for the screenshot goldens.
///
/// The old `.interrupted-card`: a box on `--bg-1` with the ordinary border, as tall as the area between the top row
/// and the details bar so the bar stays in the window, at most 42rem wide, its contents centered and wrapped. While its
/// Replace question is open, Replace reads as pressed and does nothing, and Restart is unavailable, as the old card's
/// claim held them; while the session has a request in flight (or its YOLO question is open), both are unavailable.
pub(crate) fn draw(
    drawn: Drawn,
    pane: WeakEntity<SessionPane>,
    window: &Window,
    cx: &App,
) -> AnyElement {
    let Drawn {
        key,
        row,
        action,
        focus,
        question,
        launcher_wait,
        step,
    } = drawn;
    let palette = theme::palette(cx);
    let question = question.filter(|question| question.is_open(cx));
    let asking = question.is_some();
    // The card's own open question is among what `refused` counts ([`asks`]); it refuses the card's Restart, but its
    // Replace stays drawn pressed rather than unavailable, so focus can rest on it and come back to it on Escape.
    let busy = menu::refused(key, cx) && !asking;
    let text = text(row);
    let name = text.accessible_name();
    let ink = theme::color(palette.fg_0);
    let mut lines: Vec<AnyElement> = Vec::new();
    for work in [Work::Restart, Work::Replace] {
        // Announced: the header leaves its copy out while the card shows (see the module docs, "The lines move in").
        if let Some(line) = outcome::wait_line("session-card", work, true, key, row, step, ink, cx)
        {
            lines.push(
                div()
                    .flex()
                    .justify_center()
                    .w_full()
                    .child(line)
                    .into_any_element(),
            );
        }
    }
    if let Some(line) = outcome::pane_line(key, cx) {
        lines.push(line.w_full().into_any_element());
    }
    // Restart and Replace start a session, so they wait for a host change as the header's and the row's do
    // (`menu::launch_wait`), saying why in place of what they do; busy rather than disabled, so that reason stays
    // reachable. Replace with waits for the launcher instead (`Drawn::launcher_wait`), the same way.
    let wait = menu::launch_wait(cx);
    let mut buttons: Vec<AnyElement> = Vec::new();
    if restart::offered(row) {
        let captured = action.clone();
        buttons.push(
            Button::new(
                "session-card-restart",
                "Restart",
                wait.map_or_else(
                    || HoverHelp::sentence(restart::hover_help(row, "recovery-restart-help")),
                    HoverHelp::tooltip,
                ),
                Role::Button,
            )
            .help_below()
            .tier(Tier::Primary)
            // The visible word matches the text's "Restart or Replace"; the name keeps the conversation's promise.
            .accessible_name(restart::offer_label(row.info.restart_offer))
            .busy(wait.is_some())
            .disabled(busy || asking)
            .on_click(move |_, _, cx| restart_from_card(captured.as_ref(), cx))
            .into_any_element(),
        );
    }
    let replace_button = if offers_replace_with(row) {
        let captured = action.clone();
        Button::new(
            "session-card-replace-with",
            "Replace with",
            HoverHelp::tooltip(launcher_wait.unwrap_or(
                "Replace with: edit the settings, then swap this session for a new one",
            )),
            Role::Button,
        )
        .help_below()
        // The launcher holds one launch at a time, so a pending launch keeps it from opening as a host change does; the
        // entry point refuses then anyway (`launcher::entry::new_available`), and this says why rather than look ready.
        .busy(launcher_wait.is_some())
        // A New handoff, as the header's Replace with… is (`super::header`, "Into the launcher from Hosts"): once
        // the launcher opens, Sessions shows in place of the Hosts panel, as the old card's did through the header's
        // path.
        .on_click(move |_, window, cx| {
            if let Some(target) = captured.as_ref().and_then(ShownAction::shown)
                && target.allows(cx, offers_replace_with)
                && launcher::entry::open_replace_with(&target.key, window, cx)
            {
                session_list::show_sessions(window, cx);
            }
        })
    } else {
        Button::new(
            "session-card-replace",
            "Replace",
            HoverHelp::tooltip(wait.unwrap_or_else(|| replace::menu_help(row))),
            Role::Button,
        )
        .help_below()
        .busy(wait.is_some())
        .expanded(asking)
        .on_click(move |_, window, cx| {
            if let Some(target) = action.as_ref().and_then(ShownAction::shown) {
                let mark = Mark::take(cx);
                let _ = pane.update(cx, |pane, cx| {
                    pane.replace_from_card(target, &mark, window, cx)
                });
            }
        })
    };
    buttons.push(
        replace_button
            .tier(Tier::Primary)
            .disabled(busy)
            .into_any_element(),
    );
    let yolo = replace_yolo::shown(key, Surface::Card, cx);
    let card = div()
        .id("session-interrupted-card")
        .debug_selector(|| "session-interrupted-card".into())
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_center()
        .content_center()
        .gap(px(GAP))
        .w_full()
        .max_w(px(MAX_WIDTH))
        .h_full()
        .p(px(PAD))
        .bg(theme::color(palette.bg_1))
        .border_1()
        .border_color(theme::color(palette.border))
        .rounded(metrics::RADIUS_SM)
        .text_center()
        .text_color(ink)
        .child(
            div()
                .w_full()
                .text_size(metrics::TEXT_MD)
                .line_height(relative(1.5))
                .child(text.named_by_parent()),
        )
        .children(lines)
        .children(buttons)
        .children(question.map(|question| div().w_full().child(question)))
        .children(yolo.map(|question| div().w_full().child(question)));
    // The focus stop: named by the card's words, so assistive technology reads the explanation where focus rests, with
    // the keyboard's ring over the card's own edge. A press on the card's own space takes focus there, as home would
    // anyway; its buttons take their presses first.
    let ring = Ring {
        inset_x: px(0.),
        inset_y: px(0.),
        radius: metrics::RADIUS_SM,
    };
    let card = components::focus_stop(
        card,
        "session-interrupted-card",
        focus,
        Role::Group,
        name,
        ring,
        window,
        cx,
    );
    div()
        .size_full()
        .flex()
        .justify_center()
        .child(card)
        .into_any_element()
}

// ---- Acting ----------------------------------------------------------------------------------------------------------

/// The card sends Restart without stop consent. Its missing terminal gives the card no live process to ask about.
/// The supervisor still checks for a live run, including a terminal published since the row was drawn. Failed cleanup
/// of a legacy terminal can leave an untracked agent alive, the exception in SPEC.md under Terminal experience.
/// The shown row supplies the guard while the current row still offers Restart on this card.
fn restart_from_card(snapshot: Option<&ShownAction<menu::Actionable>>, cx: &mut App) {
    use session_list::environment::{self, Environment};

    if let Some(target) = snapshot.and_then(ShownAction::shown)
        && local_state::selected(cx).as_deref() == Some(target.key.as_str())
        && target.allows(cx, |row| {
            shows_row(&target.key, row, cx)
                && restart::offered(row)
                && !environment::of_key(&target.key, cx).is_some_and(Environment::frozen)
        })
    {
        restart::send(&target.key, false, cx);
    }
}

impl SessionPane {
    /// The card's Replace at `mark`: ask Replace's question inside the card and keep it ([`keep_question`]), or send it
    /// at once when confirmations are skipped (`replace::begin` decides). Nothing happens while the card's question is
    /// open, when the card no longer owns this target, or when the session refuses competing commands.
    fn replace_from_card(
        &mut self,
        target: &menu::Actionable,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = &target.key;
        if self.open_key(cx).as_ref() != Some(key)
            || !target.allows(cx, |row| shows_row(key, row, cx))
        {
            return;
        }
        let asked = replace::begin(
            key,
            &target.row,
            target.skips_questions,
            Surface::Card,
            window.window_handle(),
            cx,
        )
        .map(|ask| prompt::ask_inline(ask(), mark, replace::confirm, window, cx));
        match asked {
            Some(question) => keep_question(key, question, cx),
            // Sent at once: an earlier question, answered by now, has nothing left to draw.
            None => drop_question(cx),
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    /// Compare the card's accessible words. The runtime tests separately exercise direction isolation when drawn.
    fn text(row: &SessionRow) -> String {
        super::text(row).accessible_name().to_string()
    }

    /// A row interrupted with `offer` for an agent of `kind`, launched as `launch` when one is given.
    fn row(offer: &str, kind: &str, launch: Option<Value>) -> SessionRow {
        let mut entry = crate::test_support::session_entry("a").value;
        entry["status"] = json!({"state": "interrupted"});
        // The helm's fixture row is on a host it cannot reach; these are on one it reaches.
        entry["stale"] = json!(false);
        entry["restart_offer"] = json!(offer);
        entry["agent_kind"] = json!(kind);
        if let Some(launch) = launch {
            entry["launch"] = launch;
        }
        serde_json::from_value(entry).unwrap()
    }

    /// A legacy launch, as the helm writes one for a session from before launch kinds.
    fn legacy() -> Value {
        json!({"kind": "legacy", "invocation": "claude", "agent_kind": "claude", "resume_template": null})
    }

    /// The card's words are the only place an interrupted session says why it has no terminal and what each choice
    /// does, so they are the old card's, pinned per offer. Specified: a resumable session names Restart or Replace and
    /// the resume; an uncaptured Claude session names only Replace, never-captured, and the feedback invitation; an
    /// agent without conversation reporting and an older Muse launch without a stored resume command name only Replace
    /// and why; a legacy
    /// launch names Replace with instead in each of its three forms and never promises that Replace starts it over.
    #[test]
    fn the_cards_words_say_what_ended_and_what_each_choice_does() {
        let ended = "A host reboot or a Farhelm update ended this session's agent";
        assert_eq!(
            text(&row("resume", "claude", None)),
            format!(
                "{ended}; it needs an intentional restart. Farhelm will wait for you to choose Restart or Replace — \
                 restarting resumes this session's own conversation."
            )
        );
        assert_eq!(
            text(&row("not_captured", "claude", None)),
            format!(
                "{ended}. Farhelm will wait for you to choose Replace — Farhelm never captured this Claude session's \
                 conversation, so Restart can't resume it; Replace starts the session over. (If you expected Restart \
                 here, please send feedback from the Help (?) menu.)"
            )
        );
        assert_eq!(
            text(&row("no_conversation_reporting", "generic", None)),
            format!(
                "{ended}. Farhelm will wait for you to choose Replace — Farhelm has no conversation reporting for this \
                 session's agent, so Replace starts it over."
            )
        );
        let older_muse = json!({"kind": "agent", "selection": {"harness": "muse"},
            "start": ["muse", "{farhelm_args}"], "resume": null});
        assert_eq!(
            text(&row("no_resume_command", "muse", Some(older_muse))),
            format!(
                "{ended}. Farhelm will wait for you to choose Replace — this agent launch has no stored resume \
                 command, so Replace starts it over."
            )
        );
        assert_eq!(
            text(&row("resume", "claude", Some(legacy()))),
            format!(
                "{ended}; it needs an intentional restart. Farhelm will wait for you to choose Restart or Replace with \
                 — restarting resumes this session's own conversation."
            )
        );
        assert_eq!(
            text(&row("not_captured", "claude", Some(legacy()))),
            format!(
                "{ended}. Farhelm will wait for you to choose Replace with — Farhelm never captured this Claude \
                 session's conversation, so Restart can't resume it; Replace with opens the launcher with its command \
                 to start it over. (If you expected Restart here, please send feedback from the Help (?) menu.)"
            )
        );
        let other = text(&row("no_resume_command", "claude", Some(legacy())));
        assert_eq!(
            other,
            format!(
                "{ended}. Farhelm will wait for you to choose Replace with — this session was created before launch \
                 kinds, so it cannot resume, and Replace with opens the launcher with its command to start it over."
            )
        );
        assert!(!other.contains("Replace starts it over"));
    }

    /// The card distinguishes an exited agent with a retained screen from a missing terminal. Interruption gets its
    /// own explanation even if both conditions hold, and an unreachable host owns the surface in either case.
    #[test]
    fn a_reachable_session_takes_the_card_when_interrupted_or_missing_its_terminal() {
        let mut row = row("resume", "claude", None);
        assert!(matches!(reason(&row), Some(Reason::Interrupted)));
        row.info.terminal_missing = true;
        assert!(matches!(reason(&row), Some(Reason::Interrupted)));
        row.stale = true;
        assert!(reason(&row).is_none());
        row.stale = false;
        row.info.status = SessionStatus::Exited { exit_code: Some(0) };
        assert!(matches!(reason(&row), Some(Reason::MissingTerminal)));
        row.info.terminal_missing = false;
        assert!(reason(&row).is_none());
    }
}
