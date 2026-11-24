//! The Welcome card, and the dialogs SPEC.md opens by themselves when a window starts or resets.
//!
//! SPEC.md, "Welcome card" and "Announcements". Ported from the old app's `welcome.rs` (`WelcomeDialog`), its
//! `PreferencesGate` (the decision after the preference read) and `app_bar.rs` (`finish_welcome`, the announcement
//! check that waited for Welcome and its feedback handoff), with the `.welcome-*` rules in `app.css`, at reference
//! commit `24bc0580`. Here's the TLDR of how it behaves:
//!
//! - **Once per helm, decided once per UI session.** [`StartDialogs`], which the main view keeps for the window's
//!   life, decides when it first sees a new UI session (a window start, or a reset's new connection): the card opens
//!   when the pushed preferences say `welcome_seen` is not true. With no preferences entry in the store the read
//!   failed, which is never evidence of unseen, so the card does not open. The decision lives in a [`PerSession`],
//!   so a reset decides again and nothing decided for an old connection acts for the new one.
//! - **Every way of closing the first-open card marks it seen.** Escape, the ×, Got it and Send feedback now each send
//!   one keyless `PUT /api/preferences` of `{"welcome_seen": true}`, a sparse patch that touches no other preference.
//!   A failed write is logged and the card stays closed; the next window start or reset may show it again. A card
//!   reopened from the `?` menu ([`open`]) writes nothing and returns focus to the `?` button.
//! - **Read Get started leaves the card open**; it opens the first-session guide through the app's one opener
//!   (`cx.open_url`). **Send feedback now** closes the card and opens the feedback dialog with the card's own return
//!   target, sending nothing.
//! - **The announcement check comes after Welcome.** SPEC.md has the initial Welcome, its Send feedback now handoff
//!   included, close first. When the card does not open, the batch of pending announcements opens at once
//!   ([`crate::announcements::open_pending`]). When it does, the check waits until the card closes, or, after the
//!   handoff, until the feedback dialog closes (its close continuation, [`crate::feedback::open_returning`]), and then
//!   reads the pushed queue as it is. The installed app checks for upgrade highlights once the batch is dismissed, or
//!   when the queue is empty. One flag per UI session says these checks are due after Welcome.
//!
//! An agent run (`scripts/gui-agent.py start`, whose helm is always fresh) turns this automatic decision off for the
//! whole run unless it asks for `--start-dialogs` ([`crate::agent_startup::start_dialogs_off`]): then nothing opens at a
//! window start or reset and nothing is written. A card or history the user opens from the `?` menu does not consult it.
//!
//! The marks: the card, or the batch when the card does not open, opens with [`Mark::at_ui_session_start`], taken
//! when [`StartDialogs`] first sees the UI session, before anything of it is drawn, so a key press or click that
//! lands before the open refuses it. Then neither the card nor the batch opens for that UI session, and both stay
//! unseen until the next window start or reset (the maintainer's decision, 2026-11-14). A batch that waited behind the
//! card opens with a mark taken when the last of them closes, which always follows a press of the user's (the
//! migration's orchestrator, 2026-11-14).
//!
//! NOTE: The first-open card returns focus home when it closes (`ReturnTo::Home`): it has no originating control, and
//! SPEC.md has it restore ordinary initial focus. The same goes for the feedback dialog it hands off to; the old app
//! sent focus to its Help toggle after that dialog in every case.

#[cfg(test)]
pub(crate) mod tests;

use std::rc::Rc;

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{
    App, AppContext as _, Context, FocusHandle, FontWeight, HighlightStyle, IntoElement,
    ParentElement as _, Render, Styled as _, StyledText, Subscription, Task, WeakEntity, Window,
    div, px, relative,
};

use crate::components::{Button, DialogFrame, HoverHelp, Role, Tier};
use crate::focus::{Mark, ReturnTo};
use crate::layers::{self, Dialog, DismissHook, Dismissal, LayerId, MainAction};
use crate::session::{self, PerSession};
use crate::theme::{self, Palette, metrics};

// ---- Wording and sizes -----------------------------------------------------------------------------------------------

/// The card's title, which is also its accessible name.
pub(crate) const TITLE: &str = "Welcome to Farhelm";

/// The Get started guide's first-session page, which Read Get started opens (SPEC.md, "Welcome card").
pub(crate) const GET_STARTED_URL: &str = "https://farhelm.io/docs/get-started/first-session/";

/// The opening paragraph (`.welcome-introduction`), the old card's words.
const INTRODUCTION: &str = "One window for every coding agent you run, on your Mac and on any Linux box you can SSH \
                            to. Each agent keeps running on its machine when you close the window, and you pick it up \
                            where it was.";

/// The footer's note (`.dialog-footer-hint`), the old card's words: where to find the card again.
const FOOTER_NOTE: &str = "This card is in the ? menu as Welcome if you want it again.";

/// `.welcome-dialog`: `width: min(520px, 100%)`.
const WIDTH: f32 = 520.;
/// `.welcome-dialog`'s text size.
const TEXT: f32 = 12.;
/// `.welcome-dialog`'s `line-height: 1.55`, relative to the text's size as CSS's unitless value is.
const LINE: f32 = 1.55;
/// `.welcome-body`'s gap between the introduction and the two items.
const BODY_GAP: f32 = 8.;
/// `.welcome-introduction`'s bottom margin (`margin: 0 0 2px`), on top of the body's gap.
const INTRODUCTION_FOOT: f32 = 2.;
/// `.welcome-item`'s padding, vertical then horizontal.
const ITEM_PAD: (f32, f32) = (8., 10.);

// ---- Opening ---------------------------------------------------------------------------------------------------------

/// Open the card again from the `?` menu's Welcome item. It writes nothing on any close, and closing returns focus to
/// `opener`, the `?` button (SPEC.md: "Reopening does not reset or rewrite seen. Closing a reopened card returns focus
/// to the help toggle"); Send feedback now hands the feedback dialog the same return target. `mark` is the [`Mark`]
/// taken in the menu item's click handler. Returns the card's layer, or `None` when the user moved on since `mark` or
/// the window is inert.
pub fn open(
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    open_card(ReturnTo::Control(opener.clone()), None, mark, window, cx)
}

/// Open the card, closing to `returns_to`. `owner` is the [`StartDialogs`] that opened it at a window start, and is
/// what makes this the first-open card: only that card marks itself seen when dismissed and tells its owner when it is
/// gone, so the announcement check can follow. A reopened card has none.
fn open_card(
    returns_to: ReturnTo,
    owner: Option<WeakEntity<StartDialogs>>,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let dialog = cx.new(|cx| WelcomeDialog {
        got_it: cx.focus_handle().tab_stop(true),
        returns_to: returns_to.clone(),
        owner: owner.clone(),
        layer: None,
        handed_off: false,
        marked_seen: false,
        _gone: None,
    });
    let weak = dialog.downgrade();
    // Enter on the card itself (after a click on its text) presses Got it, its one main action, as the announcements
    // dialog does.
    let main_action: MainAction = {
        let weak = weak.clone();
        Rc::new(move |window, cx| dismiss_card(&weak, window, cx))
    };
    // Every dismissal (Escape, the ×, Got it, Send feedback now, Enter on the card) goes through here, so the
    // first-open card is marked seen whichever way the user closes it. A reset closes the card without asking, and so
    // marks nothing: the card was not dismissed, and the next UI session decides again.
    let on_dismiss: DismissHook = Rc::new(move |_, cx| {
        let first_dismissal = weak
            .update(cx, |dialog, _| {
                dialog.owner.is_some() && !std::mem::replace(&mut dialog.marked_seen, true)
            })
            .unwrap_or(false);
        if first_dismissal {
            mark_seen(cx);
        }
        Dismissal::Close
    });
    let layer = layers::open_dialog(
        Dialog {
            view: dialog.clone().into(),
            container: cx.focus_handle(),
            // Got it, as the old card started there.
            start: dialog.read(cx).got_it.clone(),
            // None: the card is about nothing the helm pushes, so only the user (or a reset) closes it.
            subject: None,
            main_action: Some(main_action),
            // SPEC.md, "Opening and closing", does not list the Welcome card among the dialogs that keep focus on the
            // control in use when their empty space is clicked.
            keeps_focus_on_empty_click: false,
            returns_to,
            on_dismiss: Some(on_dismiss),
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    dialog.update(cx, |dialog, cx| {
        dialog.layer = Some(layer);
        // Registered only once the card is open, so a refused open (the entity dropped at once) tells the owner
        // nothing. The layer manager holds the card's only strong reference, so the card is released when its layer
        // closes, by whatever route: a dismissal, or a reset's `layers::close_all`, which the owner tells apart by its
        // UI session.
        if owner.is_some() {
            dialog._gone =
                Some(cx.on_release_in(window, |dialog, window, cx| dialog.gone(window, cx)));
        }
    });
    Some(layer)
}

/// Mark the card seen in the helm's shared preferences: a keyless, best-effort `PUT /api/preferences` that names only
/// `welcome_seen` (SPEC_impl.md: a write is a sparse patch, so no other field is touched). A failure is logged and
/// nothing else: the card is already closed, and SPEC.md lets it appear again at a later window start or reset.
fn mark_seen(cx: &mut App) {
    let path = match ApiPath::api(&["preferences"]) {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(%error, "could not mark the welcome card seen");
            return;
        }
    };
    crate::operations::send(
        Method::Put,
        path,
        Some(serde_json::json!({ "welcome_seen": true })),
        "PUT /api/preferences",
        cx,
        |answer, _| {
            if let Err(words) = answer {
                tracing::warn!(reason = %words.raw(), "could not mark the welcome card seen");
            }
        },
    );
}

// ---- The card --------------------------------------------------------------------------------------------------------

/// The open card: where focus starts, where closing returns it, and what its closing still has to do.
struct WelcomeDialog {
    /// Got it's focus handle, where the card starts.
    got_it: FocusHandle,
    /// Where closing returns focus, which Send feedback now hands on to the feedback dialog.
    returns_to: ReturnTo,
    /// The owner of the window-start dialogs, for the first-open card only (see [`open_card`]).
    owner: Option<WeakEntity<StartDialogs>>,
    /// Set right after the layer opens.
    layer: Option<LayerId>,
    /// Whether Send feedback now closed the card, so the feedback dialog, not the card, is what the announcement check
    /// waits for.
    handed_off: bool,
    /// Whether this card's dismissal already sent its seen write, so a second dismissal racing the close cannot send
    /// another.
    marked_seen: bool,
    /// Tells the owner the card is gone; first-open card only.
    _gone: Option<Subscription>,
}

impl WelcomeDialog {
    /// The card was released, its layer closed: tell the owner, which decides whether the announcement check is due.
    fn gone(&mut self, window: &mut Window, cx: &mut App) {
        let (Some(owner), Some(layer)) = (self.owner.as_ref(), self.layer) else {
            return;
        };
        let handed_off = self.handed_off;
        let _ = owner.update(cx, |owner, cx| {
            owner.card_gone(layer, handed_off, window, cx)
        });
    }
}

/// Dismiss the card `weak` through the layer manager, as Escape does, so its dismissal hook marks it seen; nothing once
/// it is gone. Called from click handlers and the main action, never from inside an update of the card, since the
/// hook updates it.
fn dismiss_card(weak: &WeakEntity<WelcomeDialog>, window: &mut Window, cx: &mut App) {
    let layer = weak.upgrade().and_then(|dialog| dialog.read(cx).layer);
    if let Some(layer) = layer {
        layers::dismiss(layer, window, cx);
    }
}

/// Send feedback now: close the card (a dismissal, so the first-open card is marked seen) and open the feedback
/// dialog for this press, with the card's own return target. The first-open card's owner hears when that dialog
/// closes, since the announcement check waits for it too.
///
/// Should the feedback dialog not open, the card's going runs the announcement check itself, as after Got it: nothing
/// would ever close the feedback dialog, so a check left waiting for it would be stranded until the next window start.
/// Today that cannot happen while the window is live (the mark is this press's own); it is kept so a later refusal in
/// the feedback dialog's open cannot strand the batch.
fn hand_off(weak: &WeakEntity<WelcomeDialog>, window: &mut Window, cx: &mut App) {
    let mark = Mark::take(cx);
    // Held to the end, so the card is released (and tells its owner it is gone) only after the feedback dialog's open
    // has answered, and a refused open can still take the handoff back.
    let Some(dialog) = weak.upgrade() else {
        return;
    };
    let (returns_to, owner) = dialog.update(cx, |dialog, _| {
        dialog.handed_off = true;
        (dialog.returns_to.clone(), dialog.owner.clone())
    });
    dismiss_card(weak, window, cx);
    let on_closed = owner.map(|owner| -> crate::feedback::OnClosed {
        Box::new(move |window, cx| {
            let _ = owner.update(cx, |owner, cx| owner.handoff_closed(window, cx));
        })
    });
    if crate::feedback::open_returning(returns_to, on_closed, &mark, window, cx).is_none() {
        dialog.update(cx, |dialog, _| dialog.handed_off = false);
    }
}

/// One bordered pointer of the card (`.welcome-item`): its sentence, starting with a bold lead-in, and the control it
/// points at. `bold` lists the byte ranges of `sentence` drawn bold in the brightest ink (`.welcome-item strong`).
///
/// The control sits on a line of its own under the sentence, at the start: GPUI has no inline flow for a control inside a
/// paragraph, where the old card ran its link on at the end of the sentence's last line.
fn item(
    sentence: &'static str,
    bold: &[std::ops::Range<usize>],
    control: Button,
    palette: &Palette,
) -> impl IntoElement {
    let strong = HighlightStyle {
        font_weight: Some(FontWeight::BOLD),
        color: Some(theme::color(palette.fg_0)),
        ..HighlightStyle::default()
    };
    div()
        .flex()
        .flex_col()
        .items_start()
        .py(px(ITEM_PAD.0))
        .px(px(ITEM_PAD.1))
        .border_1()
        .border_color(theme::color(palette.border_dim))
        .rounded(metrics::RADIUS_SM)
        // In a block of its own, which wraps the sentence at the item's width; the text element alone would be laid
        // out at its full length.
        .child(
            div().w_full().child(
                StyledText::new(sentence)
                    .with_highlights(bold.iter().cloned().map(|range| (range, strong))),
            ),
        )
        .child(control)
}

/// The byte range of `part` within `sentence`: for the bold parts of an item's sentence, so a change of wording cannot
/// leave a range pointing at the wrong words. Both are this module's own fixed wording, so a `part` the sentence does
/// not hold is a bug here: it fails a debug build, and otherwise bolds nothing (an empty range).
fn range_of(sentence: &str, part: &str) -> std::ops::Range<usize> {
    let found = sentence.find(part);
    debug_assert!(
        found.is_some(),
        "{part:?} is not in the welcome card's sentence {sentence:?}"
    );
    found.map_or(0..0, |start| start..start + part.len())
}

impl Render for WelcomeDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let get_started = Button::new(
            "welcome-get-started",
            "Read Get started ↗",
            HoverHelp::tooltip("Get started: open the first-session guide in your browser"),
            Role::Link,
        )
        .tier(Tier::Link)
        .on_click(|_, _, cx| cx.open_url(GET_STARTED_URL));
        let feedback = Button::new(
            "welcome-feedback",
            "Send feedback now",
            HoverHelp::tooltip(
                "Send feedback now: close the welcome card and write a private message",
            ),
            Role::Button,
        )
        .tier(Tier::Link)
        .on_click({
            let weak = weak.clone();
            move |_, window, cx| hand_off(&weak, window, cx)
        });
        let new_to_it = "New to it? The Get started guide walks through your first session and adding a remote \
                         host.";
        // The `?` button sits at the host rail's foot, at the window's bottom left; the old card's "at the top of the
        // sidebar" was already stale at the reference commit.
        let tell_us = "Tell us what you think. Send feedback, in the ? menu at the bottom left of the window, sends a \
                       private message straight to Farhelm's maintainer: bugs, rough edges, ideas.";
        let body = div()
            .flex()
            .flex_col()
            .gap(px(BODY_GAP))
            .text_size(px(TEXT))
            .line_height(relative(LINE))
            .text_color(theme::color(palette.fg_dim))
            .child(div().mb(px(INTRODUCTION_FOOT)).child(INTRODUCTION))
            .child(item(
                new_to_it,
                &[range_of(new_to_it, "New to it?")],
                get_started,
                palette,
            ))
            .child(item(
                tell_us,
                &[
                    range_of(tell_us, "Tell us what you think."),
                    range_of(tell_us, "?"),
                ],
                feedback,
                palette,
            ));
        let got_it = Button::new(
            "welcome-got-it",
            "Got it",
            HoverHelp::tooltip("Got it: close the welcome card"),
            Role::Button,
        )
        .tier(Tier::Primary)
        .focus(&self.got_it)
        .on_click({
            let weak = weak.clone();
            move |_, window, cx| dismiss_card(&weak, window, cx)
        });
        DialogFrame::new("welcome", TITLE, body, vec![got_it])
            .width(px(WIDTH))
            .compact()
            .note(FOOTER_NOTE)
            .close(
                "Close welcome",
                HoverHelp::tooltip("Close: done with the welcome card"),
                false,
                move |_, window, cx| dismiss_card(&weak, window, cx),
            )
    }
}

// ---- The window-start dialogs ----------------------------------------------------------------------------------------

/// Own the window-start decisions for Welcome, Announcements and upgrade highlights. The main view keeps one
/// for the window's life and starts the decisions over when the UI session resets.
pub struct StartDialogs {
    start: PerSession<Start>,
    /// Follows the connection, whose change is how a new UI session arrives.
    _connection: Option<Subscription>,
}

/// The signed-feed brick launch reserves the batch for the bell even if the first snapshot already has it.
/// Welcome and upgrade highlights keep their own startup decisions.
pub(crate) struct AnnouncementsFromBell;

impl gpui::Global for AnnouncementsFromBell {}

/// What one UI session's start has decided and still owes.
#[derive(Default)]
struct Start {
    /// The mark taken when the owner first saw this UI session ([`Mark::at_ui_session_start`]); `None` before.
    mark: Option<Mark>,
    /// Whether the decision ran: once per UI session, whatever it came to.
    decided: bool,
    /// The first-open card's layer while it is open.
    card: Option<LayerId>,
    /// Whether the announcement check still waits for the card, or for the feedback dialog it handed off to.
    check_due: bool,
    /// The current session owns the window side of the lookup. Resetting drops it before a later session can use
    /// its answer.
    _highlights: Option<Task<()>>,
}

impl StartDialogs {
    /// The owner for the window being built. A UI session that exists already (the snapshot came before the window)
    /// is seen here, before the window's first frame draws it; the decision itself runs once the window exists, since
    /// nothing may open inside the window's construction (SPEC_impl.md, "GPUI cookbook: traps").
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let connection = session::connection(cx).map(|connection| {
            cx.observe_in(&connection, window, |owner: &mut Self, _, window, cx| {
                owner.see(cx);
                cx.defer_in(window, Self::decide);
            })
        });
        let mut owner = StartDialogs {
            start: PerSession::default(),
            _connection: connection,
        };
        owner.see(cx);
        cx.defer_in(window, Self::decide);
        owner
    }

    /// Take the window-start mark if this is the first sight of the current UI session. A connection change that brings
    /// no new session (a state change, the window going inert) takes nothing: the mark belongs to the session, told
    /// apart by its identity through [`PerSession`], not to whichever callback came first.
    fn see(&mut self, cx: &App) {
        let live = session::connection(cx)
            .is_some_and(|connection| connection.read(cx).session().is_some());
        let start = self.start.current(cx);
        if live && start.mark.is_none() {
            start.mark = Some(Mark::at_ui_session_start(cx));
        }
    }

    /// Decide once for this UI session. An unseen Welcome card holds the other checks until it closes.
    ///
    /// Deferred from wherever the session was seen to the end of that update, after every other view has heard of the
    /// new session (the session pane has made its terminal home by then). The decision runs with the mark taken when
    /// the session was seen, so a press that lands in between refuses the open.
    fn decide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let start = self.start.current(cx);
        let Some(mark) = start.mark.filter(|_| !start.decided) else {
            return;
        };
        start.decided = true;
        // An agent run that turned the start dialogs off has made this decision for the whole run: nothing opens and
        // nothing is written, so the helm keeps the card unseen and the announcements pending. Only this automatic
        // decision reads it; Help's Welcome and Announcements open as asked.
        if crate::agent_startup::start_dialogs_off(cx) {
            return;
        }
        let preferences = session::current_store(cx).map(|store| {
            store
                .read(cx)
                .collections()
                .get::<Preferences>(PREFERENCES_KEY)
                .map(|preferences| preferences.welcome_seen)
        });
        match preferences {
            Some(Some(seen)) if seen != Some(true) => {
                let owner = cx.weak_entity();
                let card = open_card(ReturnTo::Home, Some(owner), &mark, window, cx);
                let start = self.start.current(cx);
                // A refused card (the user acted first) leaves no check due: neither the card nor the batch opens for
                // this UI session.
                start.card = card;
                start.check_due = card.is_some();
                return;
            }
            Some(None) => {
                tracing::info!(
                    "no shared preferences were read, so the welcome card does not open"
                );
            }
            _ => {}
        }
        // SPEC.md: "A failed preference read does not prevent this separate check."
        self.remaining_dialogs(mark, window, cx);
    }

    /// The first-open card `layer` is gone. Unless Send feedback now handed off to the feedback dialog, which the check
    /// waits for instead, run the check that waited for the card. Nothing for a card of a UI session a reset retired:
    /// [`PerSession`] has started over, and the new session decides for itself.
    fn card_gone(
        &mut self,
        layer: LayerId,
        handed_off: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let start = self.start.current(cx);
        if start.card != Some(layer) {
            return;
        }
        start.card = None;
        if !handed_off {
            self.run_due_check(window, cx);
        }
    }

    /// The feedback dialog the first-open card handed off to has closed: run the check that waited for it.
    fn handoff_closed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.start.current(cx).card.is_none() {
            self.run_due_check(window, cx);
        }
    }

    /// The startup checks that waited for the card (and its handoff), at most once per UI session, with a mark
    /// taken now: the close that got here was the user's press.
    fn run_due_check(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.start.current(cx).check_due) {
            return;
        }
        let mark = Mark::take(cx);
        self.remaining_dialogs(mark, window, cx);
    }

    /// Upgrade highlights follow the automatic announcements. A queued dismissal continues only within
    /// the connection session that supplied the batch.
    fn remaining_dialogs(&mut self, mark: Mark, window: &mut Window, cx: &mut Context<Self>) {
        let store = session::current_store(cx)
            .as_ref()
            .map(|store| store.entity_id());
        let owner = cx.weak_entity();
        let after: MainAction = Rc::new(move |window, cx| {
            if session::current_store(cx)
                .as_ref()
                .map(|store| store.entity_id())
                == store
            {
                let _ = owner.update(cx, |owner, cx| {
                    let task = crate::whats_new::check(Mark::take(cx), window, cx);
                    owner.start.current(cx)._highlights = task;
                });
            }
        });
        if cx.try_global::<AnnouncementsFromBell>().is_some()
            || crate::announcements::open_pending(&mark, after, window, cx).is_none()
        {
            let task = crate::whats_new::check(mark, window, cx);
            self.start.current(cx)._highlights = task;
        }
    }
}
