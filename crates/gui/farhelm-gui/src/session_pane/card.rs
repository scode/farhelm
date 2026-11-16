//! The interrupted card: what the session pane shows in place of the open session's terminals once a host reboot or a
//! Farhelm update ended its agent, with the way forward.
//!
//! SPEC.md, "Durability and resume": "Opening an interrupted session shows a centered neutral card in the empty terminal
//! area explaining that a host reboot or a Farhelm update ended the agent and that it needs an intentional restart. The
//! card offers Restart when the session can resume its conversation (see Lifecycle operations), and Replace, which
//! starts a fresh session under the same settings and keeps its inline destructive confirmation; a session that cannot
//! resume offers only Replace and says why." Ported from the old app's `.interrupted-card` (`session_view.rs` and
//! `app.css` at the reference commit). Here's the TLDR of the rules:
//!
//! - **When.** While the store holds the open session as Interrupted on a host the helm reaches ([`shows`]), unless a
//!   Restart has relaunched it since (below). A session on a host the helm cannot reach is the unreachable-host card's,
//!   whatever its last status: a host that cannot be asked anything cannot be restarted either, and a reboot is known
//!   only once the host answers again. The pane mounts no terminal and no tab strip while the card shows (the reboot
//!   took the terminal, so an attach would only meet its absence), drops the ones it had when a session it shows is
//!   interrupted, and mounts fresh ones only once the session is on a reachable host and no longer reads interrupted
//!   ([`reachable`]; `session_pane.rs`), so a card whose host drops away mounts nothing in between. The top row and the
//!   details bar stay, so the session's metadata is what an interrupted session shows, never an empty pane (SPEC.md,
//!   "Session view").
//! - **Restarted, until the list says so.** The helm's reply to a Restart comes before its list stops saying
//!   Interrupted: the supervisor's reply carries an Unknown status, which the helm's session cache never lets overwrite
//!   a definite one, and the real status arrives with the host's next listing, after the reply. So a Restart that
//!   succeeds marks its session restarted ([`restarted`]), and the card treats a marked session as recovered: it goes
//!   on the reply and the pane mounts the new terminal. The card's Restart goes with it, so a second click there cannot
//!   send a second Restart into the gap; the header's Restart stays available meanwhile, as the old header's did. The
//!   mark goes once the store lists the session as anything but Interrupted, or its host as unreachable: a second
//!   reboot inside that window would leave every later push saying Interrupted, and the mark would otherwise keep the
//!   pane attaching to a terminal that reboot took. The old card's `relaunched` (`session_view.rs` at the reference
//!   commit), with the same two ends.
//! - **The text** says what happened and what the card offers ([`text`]), from the same clauses as Restart's hover help
//!   (`session_list::restart`), so the two cannot drift apart.
//! - **Restart** is one click: nothing runs, so nothing is asked (`restart::begin` asks only while the agent reads
//!   working), and it is drawn only when the conversation can be resumed. **Replace** asks its inline question in the
//!   card, unless Skip session confirmations is on, through `replace::begin` and `confirm` as the row and the header do.
//!   A legacy launch (a session from before launch kinds) cannot be replaced as it is, so its card offers the
//!   launcher's Replace with in Replace's place, as the old card did. Restart and Replace start a session, so they wait
//!   for a host change as the header's and the row's do (`session_list::menu::launch_wait`). Replace with opens the
//!   launcher, so it waits whenever the launcher cannot open (`session_list::menu::launcher_wait`: a pending launch as
//!   well as a host change), as the row's and the header's Replace with do, saying why in its hover help.
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
    AnyElement, App, Context, EntityId, FocusHandle, Global, InteractiveElement as _, IntoElement,
    ParentElement, Role, Styled, Subscription, WeakEntity, Window, div, px, relative,
};

use super::SessionPane;
use crate::components::{self, Button, HoverHelp, Ring, Tier};
use crate::focus::Mark;
use crate::operations::Work;
use crate::prompt::{self, Inline};
use crate::session::{self, ObservingCurrent};
use crate::session_list::replace_yolo::{self, Surface};
use crate::session_list::{menu, outcome, replace, restart};
use crate::theme::{self, metrics};
use crate::{launcher, local_state};

/// `.interrupted-card`'s `max-width: 42rem`.
const MAX_WIDTH: f32 = 672.;
/// `.interrupted-card`'s `padding: 2rem`.
const PAD: f32 = 32.;
/// `.interrupted-card`'s `gap`.
const GAP: f32 = 12.;

// ---- When the card shows, and what it says ---------------------------------------------------------------------------

/// Whether `row` takes the interrupted card: its status is Interrupted and the helm reaches its host (see the module
/// docs, "When").
fn interrupted(row: &SessionRow) -> bool {
    !row.stale && row.info.status == SessionStatus::Interrupted
}

/// Whether the open session filed under `key`, whose row the store holds as `row`, shows the interrupted card now: its
/// row takes the card and no Restart has relaunched it since ([`restarted`]).
pub(crate) fn shows_row(key: &str, row: &SessionRow, cx: &App) -> bool {
    interrupted(row) && !is_restarted(key, cx)
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

/// The row the current UI session's store holds under `key`.
fn row(key: &str, cx: &App) -> Option<SessionRow> {
    session::current_store(cx)?
        .read(cx)
        .collections()
        .get::<SessionRow>(key)
        .cloned()
}

// ---- Restarted sessions ----------------------------------------------------------------------------------------------

/// The sessions a Restart relaunched while the store may still list them as Interrupted, under the UI session they were
/// marked in (see the module docs, "Restarted, until the list says so"). A global, because the Restart's completion
/// marks it from the operation store, with no view at hand, and the follower and the pane read it.
#[derive(Default)]
struct Restarted {
    session: Option<EntityId>,
    keys: HashSet<String>,
}

impl Global for Restarted {}

/// The current UI session's id, `None` while the window is inert.
fn ui_session(cx: &App) -> Option<EntityId> {
    session::connection(cx).and_then(|connection| {
        connection
            .read(cx)
            .session()
            .map(|session| session.entity_id())
    })
}

/// A Restart of the session filed under `key` succeeded: its card treats it as recovered until the list catches up.
/// Called from every Restart's completion (`session_list::restart`), the header's included, since a Restart from there
/// meets the same gap. Every successful Restart marks its session, interrupted or not, on purpose: the completion knows
/// nothing of the row, and a mark on a session the store lists as anything but Interrupted is dropped at once
/// ([`forget_settled`] runs as the mark notifies), so it changes nothing.
pub(crate) fn restarted(key: &str, cx: &mut App) {
    let now = ui_session(cx);
    let marks = cx.default_global::<Restarted>();
    if marks.session != now {
        marks.session = now;
        marks.keys.clear();
    }
    marks.keys.insert(key.to_owned());
}

/// Whether the session filed under `key` is marked restarted under the current UI session.
fn is_restarted(key: &str, cx: &App) -> bool {
    cx.try_global::<Restarted>()
        .is_some_and(|marks| marks.session == ui_session(cx) && marks.keys.contains(key))
}

/// Drop the marks the store has caught up with: a session it lists as anything but Interrupted, on a host the helm
/// cannot reach, or not at all, and every mark of an earlier UI session. A mark matters only while the open session's
/// card would show, and the open session always has a row unless it is gone; a row that left the listing (a filter,
/// the cap, another session opened) could otherwise keep its mark until a later reboot made it Interrupted again, and
/// then hide that card. Changes the global (which notifies its observers) only when a mark goes.
fn forget_settled(cx: &mut App) {
    let Some(marks) = cx.try_global::<Restarted>() else {
        return;
    };
    let now = ui_session(cx);
    let gone: Vec<String> = marks
        .keys
        .iter()
        .filter(|key| {
            marks.session != now
                || row(key, cx)
                    .is_none_or(|row| row.stale || row.info.status != SessionStatus::Interrupted)
        })
        .cloned()
        .collect();
    if gone.is_empty() {
        return;
    }
    let marks = cx.default_global::<Restarted>();
    for key in gone {
        marks.keys.remove(&key);
    }
}

// ---- The card's question ---------------------------------------------------------------------------------------------

/// The Replace question the open session's card asked, with the key of the session it is about, under the UI session it
/// was asked in (see the module docs, "While it asks"). A global, because the row menu and the header read it through
/// `session_list::menu::refused`, with only `&App` and no pane at hand; the pane is its one writer ([`keep_question`],
/// [`drop_question`]). One question at most, since only the open session shows a card.
#[derive(Default)]
struct Asked {
    session: Option<EntityId>,
    question: Option<(String, Inline<replace::Frozen>)>,
}

impl Global for Asked {}

/// Keep `question`, which the card of the session filed under `key` just asked, in place of any earlier one.
pub(super) fn keep_question(key: &str, question: Inline<replace::Frozen>, cx: &mut App) {
    let session = ui_session(cx);
    cx.set_global(Asked {
        session,
        question: Some((key.to_owned(), question)),
    });
}

/// Drop the card's question, unanswered if it is still open: the pane calls this whenever it stops showing the card
/// the question belongs to. Dropping is what ends the fact then, since a question dropped unanswered still reads open
/// for any clone of it (`prompt::Inline::is_open`), this one included.
pub(super) fn drop_question(cx: &mut App) {
    if cx
        .try_global::<Asked>()
        .is_some_and(|asked| asked.question.is_some())
    {
        cx.set_global(Asked::default());
    }
}

/// The question the card of the session filed under `key` asked, if one is kept for it under the current UI session,
/// open or answered: what the card draws while it is open.
pub(super) fn question(key: &str, cx: &App) -> Option<Inline<replace::Frozen>> {
    let asked = cx.try_global::<Asked>()?;
    if asked.session != ui_session(cx) {
        return None;
    }
    asked
        .question
        .as_ref()
        .filter(|(asked_for, _)| asked_for == key)
        .map(|(_, question)| question.clone())
}

/// Whether the interrupted card of the session filed under `key` asks its Replace question now: one is kept for it
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

/// The card's sentence for `row`: the old `interrupted_surface_text`. It names the reboot or update and the intentional
/// handoff before what the choices do, so the empty terminal area explains both the missing terminal and the safe next
/// step without implying that anything recovers by itself; a session that cannot resume names only the way it can
/// start over. An interrupted session's agent has ended, so every clause is the ended one, as Restart's hover help says
/// it for the same session.
pub(crate) fn text(row: &SessionRow) -> String {
    const ENDED: &str = "A host reboot or a Farhelm update ended this session's agent";
    let (offer, kind) = (row.info.restart_offer, row.info.agent_kind);
    let clause = restart::offer_clause(offer, kind, true);
    let note = restart::feedback_note(offer, true);
    // A legacy session's card offers Replace with, never plain Replace, so its text must not promise that "Replace
    // starts it over".
    if offers_replace_with(row) {
        return if offer.can_restart() {
            format!(
                "{ENDED}; it needs an intentional restart. Farhelm will wait for you to choose Restart or Replace with \
                 — {clause}."
            )
        } else if offer == RestartOffer::NotCaptured {
            let start_over = "Replace with opens the launcher with its command to start it over";
            let clause = restart::not_captured_clause(kind, true, start_over);
            format!("{ENDED}. Farhelm will wait for you to choose Replace with — {clause}.{note}")
        } else {
            format!(
                "{ENDED}. Farhelm will wait for you to choose Replace with — this session was created before launch \
                 kinds, so it cannot resume, and Replace with opens the launcher with its command to start it over."
            )
        };
    }
    if offer.can_restart() {
        format!(
            "{ENDED}; it needs an intentional restart. Farhelm will wait for you to choose Restart or Replace — \
             {clause}."
        )
    } else {
        format!("{ENDED}. Farhelm will wait for you to choose Replace — {clause}.{note}")
    }
}

// ---- Following the store ---------------------------------------------------------------------------------------------

/// What tells the pane that the open session went into or out of its interrupted card, or its host into or out of
/// reach, so it can drop or mount its terminals then: the store pushes the change (or a Restart marks the session), and
/// nothing else the pane observes (the selection, the user's choices, the connection) moves. It also drops the restarted
/// marks the store has caught up with ([`forget_settled`]).
///
/// It is built on [`session::observe_current`], which cannot serve the pane by itself: its callback gets no window,
/// which every focus move the pane makes needs, and it calls on every pushed change, where the pane wants only these
/// transitions. So the pane observes this entity with its window, as it observes the terminal set that follows the
/// session's tabs, and this entity notifies only when the open session, whether it shows the card, or whether its host
/// is reachable changes.
pub(super) struct Follow {
    last: Option<(String, bool, bool)>,
    _following: ObservingCurrent,
    _marks: Subscription,
}

impl Follow {
    /// The follower, which reads the current UI session's store and the restarted marks from the next update on.
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        Follow {
            last: None,
            _following: session::observe_current(cx, |follow: &mut Self, cx| follow.check(cx)),
            _marks: cx.observe_global::<Restarted>(|follow, cx| follow.check(cx)),
        }
    }

    /// Drop the settled marks, then notify if the open session, whether it shows the card, or whether its host is
    /// reachable changed since the last check.
    fn check(&mut self, cx: &mut Context<Self>) {
        forget_settled(cx);
        let now = local_state::selected(cx).map(|key| {
            // One read of the row for both answers, so they describe the same push.
            let row = row(&key, cx);
            let card = row.as_ref().is_some_and(|row| shows_row(&key, row, cx));
            let reachable = row.is_some_and(|row| !row.stale);
            (key, card, reachable)
        });
        if now != self.last {
            self.last = now;
            cx.notify();
        }
    }
}

// ---- Drawing ---------------------------------------------------------------------------------------------------------

/// What the card draws for the open session filed under `key`: its row as the store holds it, the focus handle the
/// card tracks (home while it shows), the Replace question it asked if one is kept, why the launcher cannot open now
/// if it cannot, and the spinner's step.
pub(crate) struct Drawn<'a> {
    pub(crate) key: &'a str,
    pub(crate) row: &'a SessionRow,
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
        let owner = key.to_owned();
        buttons.push(
            Button::new(
                "session-card-restart",
                "Restart",
                HoverHelp::tooltip(wait.map_or_else(|| restart::hover_help(row), str::to_owned)),
                Role::Button,
            )
            .tier(Tier::Primary)
            // The visible word matches the text's "Restart or Replace"; the name keeps the conversation's promise.
            .accessible_name(restart::offer_label(row.info.restart_offer))
            .busy(wait.is_some())
            .disabled(busy || asking)
            .on_click(move |_, _, cx| restart_from_card(&owner, cx))
            .into_any_element(),
        );
    }
    let replace_button = if offers_replace_with(row) {
        let owner = key.to_owned();
        Button::new(
            "session-card-replace-with",
            "Replace with",
            HoverHelp::tooltip(launcher_wait.unwrap_or(
                "Replace with: edit the settings, then swap this session for a new one",
            )),
            Role::Button,
        )
        // The launcher holds one launch at a time, so a pending launch keeps it from opening as a host change does; the
        // entry point refuses then anyway (`launcher::entry::new_available`), and this says why rather than look ready.
        .busy(launcher_wait.is_some())
        .on_click(move |_, window, cx| {
            launcher::entry::open_replace_with(&owner, window, cx);
        })
    } else {
        Button::new(
            "session-card-replace",
            "Replace",
            HoverHelp::tooltip(wait.unwrap_or(replace::MENU_HELP)),
            Role::Button,
        )
        .busy(wait.is_some())
        .expanded(asking)
        .on_click(move |_, window, cx| {
            let mark = Mark::take(cx);
            // The pane may be gone with its window; then there is nothing to act on.
            let _ = pane.update(cx, |pane, cx| pane.replace_from_card(&mark, window, cx));
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
                .child(text.clone()),
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
        text.into(),
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

/// The card's Restart for the session filed under `key`, through the same `begin` the header's Restart calls. Nothing
/// runs in an interrupted session, so `begin` sends it at once without asking; a row that reads working by the click
/// is no longer the card's, and the question `begin` would hand back for it is dropped unasked.
fn restart_from_card(key: &str, cx: &mut App) {
    let Some(target) = menu::actionable(key, cx) else {
        return;
    };
    let _ = restart::begin(key, &target.row, target.skips_questions, cx);
}

impl SessionPane {
    /// The card's Replace at `mark`: ask Replace's question inside the card and keep it ([`keep_question`]), or send it
    /// at once when confirmations are skipped (`replace::begin` decides). Nothing happens while the card's question is
    /// open, for a session the store holds no row for, or for one with a request in flight: `menu::actionable` refuses
    /// all three, the first through [`asks`].
    fn replace_from_card(&mut self, mark: &Mark, window: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.open_key(cx) else {
            return;
        };
        let Some(target) = menu::actionable(&key, cx) else {
            return;
        };
        let asked = replace::begin(
            &key,
            &target.row,
            target.skips_questions,
            Surface::Card,
            window.window_handle(),
            cx,
        )
        .map(|ask| prompt::ask_inline(ask(), mark, replace::confirm, window, cx));
        match asked {
            Some(question) => keep_question(&key, question, cx),
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
    /// agent without conversation reporting and a command launch without Resume name only Replace and why; a legacy
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
        assert_eq!(
            text(&row("no_resume_command", "claude", None)),
            format!(
                "{ended}. Farhelm will wait for you to choose Replace — this command launch did not opt into Resume, so \
                 it has no resume command and Replace starts it over."
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

    /// Stale beats interrupted: a host the helm cannot reach cannot be restarted, so its session is the unreachable
    /// card's whatever its last status said, and only a reachable interrupted session takes this card. Specified: an
    /// interrupted row takes it, the same row stale does not, and an exited row does not (an exited agent keeps its
    /// terminal and scrollback).
    #[test]
    fn only_a_reachable_interrupted_session_takes_the_card() {
        let mut row = row("resume", "claude", None);
        assert!(interrupted(&row));
        row.stale = true;
        assert!(!interrupted(&row));
        row.stale = false;
        row.info.status = SessionStatus::Exited { exit_code: Some(0) };
        assert!(!interrupted(&row));
    }
}
