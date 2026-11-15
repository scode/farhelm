//! The approval region: where waiting agent requests show, one expanded card at the top of the main area with the
//! others as headers below it, over everything but the pill and the tooltip.
//!
//! SPEC.md, "Agent-spawned sessions": the oldest waiting request opens first as one expanded card, centered at the top
//! of the main area below the session's tabs, at most 660 pixels wide; the other waiting requests are listed below it
//! as headers that open their card; the card's header and answer bar stay visible within the region's cap; cards stay
//! usable over open dialogs. "Keyboard focus": a request arriving, a list change or an expiry moves no focus and opens
//! nothing. Ported from the old app's `ApprovalCards` region (`approvals.rs`) and `approval-layout.js`, with the
//! `.approval-cards`, `.approval-waiting*` and `.approval-late` rules of `app.css` at the reference commit.
//!
//! The region is the layer manager's approval slot ([`layers::install_approval_region`]), installed once per window at
//! window start ([`install`]) and never shown or hidden: it draws from the store on every frame, and nothing with a
//! size or a listener while no request waits and no late-answer notice shows. Its own state is only what the user did
//! there: which request they chose to read, the answer bar's pause ([`Arming`]), which it starts again whenever what
//! it shows changes, and why the card's last action opened nothing.
//!
//! Placement follows the old layout script, from two elements other views draw ([`Anchors`]): the main pane, and the
//! open session's terminal area, below its tabs and top lines. The region is as wide as the card and centered in the
//! pane; it starts 12 pixels below the terminal area's top (the pane's top with no session open) and takes at most
//! three quarters of the pane and the space left below it ([`place`]). Inside that cap the card's body gives way first,
//! its boxes shrinking toward two lines and then its fact list scrolling, so the header, From line and answer bar stay.
//! If even those, with the late notice and the waiting list, do not fit, the region takes the whole window, and below
//! that it scrolls as a whole. What those fixed parts need depends on wrapping, so it is measured as they are drawn
//! ([`Pinned`]), and a change draws the region once more.

use std::cell::Cell;
use std::rc::Rc;

use farhelm_proto::approvals::{ApprovalAction, ApprovalAnswer, PendingApproval};
use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, Entity, FocusHandle, FontWeight, Global,
    InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, canvas, div, prelude::FluentBuilder as _,
    px,
};

use super::answers::{self, Arming};
use super::card::{self, CardMeasure, CardState, MAX_WIDTH};
use super::host_add::AgentRequest;
use super::model::{self, Card};
use super::tunnel;
use crate::components::{Button, HoverHelp, Role, Tier};
use crate::focus::Mark;
use crate::host_text::{self, HostSentence, HostText};
use crate::hosts::add::NotOpened;
use crate::layers::{self, Anchor};
use crate::notices::{self, Notice};
use crate::operations::OpKey;
use crate::theme::{self, metrics};
use crate::{local_state, session};

// ---- Geometry --------------------------------------------------------------------------------------------------------

/// The space kept between the region and the pane's sides, the terminal area's top and the pane's bottom (the old
/// script's 12, and its 24 for both sides).
const MARGIN: f32 = 12.;

/// The share of the pane's height the region may take (the old script's `0.75`).
const CAP_SHARE: f32 = 0.75;

/// The gap between the region's occupants (`.approval-cards`' `gap: 8px`).
const GAP: f32 = 8.;

/// The waiting list's height before it scrolls, about three headers (`.approval-waiting-list`'s `max-height: 108px`).
const WAITING_MAX: f32 = 108.;

/// The gap between waiting headers (`.approval-waiting-list`'s `gap: 6px`).
const WAITING_GAP: f32 = 6.;

/// The anchors the region is placed from: the main pane and the open session's terminal area. The views that draw
/// those elements hand their anchors over ([`set_pane`], [`track_terminal_area`]) every frame, so the region reads this
/// frame's bounds as it lays itself out, after them.
///
/// NOTE: One set for the app, as there is one main window (multi-window support is a watch-list item); the session
/// pane documents the same assumption.
#[derive(Default)]
pub struct Anchors {
    pane: Option<Anchor>,
    terminal_area: Option<Anchor>,
}

impl Global for Anchors {}

/// The main pane's anchor, which the main view draws and hands over each frame it draws the pane, so the region knows
/// the pane.
pub fn set_pane(anchor: Anchor, cx: &mut App) {
    cx.default_global::<Anchors>().pane = Some(anchor);
}

/// The tracker the session pane draws inside the open session's terminal area (which must be `relative()`), so the
/// region starts below the session's tabs and top lines, as the old script measured `.terminal-panes`' top.
pub fn track_terminal_area(window: &mut Window, cx: &mut App) -> AnyElement {
    let anchor = Anchor::for_element("approval-region-terminal-area", window, cx);
    let tracker = anchor.track().into_any_element();
    cx.default_global::<Anchors>().terminal_area = Some(anchor);
    tracker
}

/// Where the region goes this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    /// The region's left edge, in window coordinates.
    pub left: Pixels,
    /// The region's width, which is the card's.
    pub width: Pixels,
    /// The region's top, in window coordinates.
    pub top: Pixels,
    /// The most height the region may take.
    pub cap: Pixels,
    /// Even the whole window is shorter than the fixed parts: the region scrolls as a whole.
    pub scrolls: bool,
}

/// The old layout script's placement, for a pane at `pane`, a terminal area whose top is `area_top` (`None` with no
/// session open), a window `viewport` pixels high, and fixed parts needing `pinned` pixels.
pub fn place(
    pane: Bounds<Pixels>,
    area_top: Option<Pixels>,
    viewport: Pixels,
    pinned: Pixels,
) -> Place {
    let width = MAX_WIDTH.min((pane.size.width - px(2. * MARGIN)).max(px(0.)));
    let left = pane.origin.x + (pane.size.width - width) / 2.;
    let top = area_top.unwrap_or(pane.origin.y) + px(MARGIN);
    let cap = (pane.size.height * CAP_SHARE)
        .min(pane.origin.y + pane.size.height - top - px(MARGIN))
        .max(px(0.));
    // Whole pixels, as the old script rounded its minimum up: a fraction must not clip the last answer row.
    let pinned = pinned.ceil();
    if cap >= pinned {
        return Place {
            left,
            width,
            top,
            cap,
            scrolls: false,
        };
    }
    Place {
        left,
        width,
        top: px(0.),
        cap: viewport,
        scrolls: viewport < pinned,
    }
}

/// The heights the region's fixed parts took as they were last drawn: the late notice, the waiting list, and the
/// card's own fixed parts ([`CardMeasure`]). Everything the region holds but the card's body, which gives way first.
#[derive(Clone, Default)]
struct Pinned {
    notice: Rc<Cell<Pixels>>,
    list: Rc<Cell<Pixels>>,
    card: CardMeasure,
}

impl Pinned {
    /// What the fixed parts need together, with the gaps between the occupants drawn now.
    fn total(&self, occupants: usize) -> Pixels {
        self.notice.get()
            + self.list.get()
            + self.card.pinned()
            + px(GAP) * occupants.saturating_sub(1) as f32
    }
}

/// A tracker that records its element's height in `cell` as it is drawn, and draws the window again when the height
/// changed, so the region's placement catches up with what wrapping made of its fixed parts. The element must be
/// `relative()`.
///
/// The redraw is deferred to after this frame: the tracker runs while the frame is prepared, where a refresh is
/// dropped. It settles in one more frame, since the region's width does not depend on the measurements and the fixed
/// parts' heights do not depend on the cap.
pub(super) fn height_tracker(cell: Rc<Cell<Pixels>>) -> impl IntoElement {
    canvas(
        move |bounds, window, cx| {
            if cell.replace(bounds.size.height) != bounds.size.height {
                window.defer(cx, |window, _| window.refresh());
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

// ---- The region ------------------------------------------------------------------------------------------------------

/// Install the approval region in `window`, once, at window start: the main view does, as it attaches the layer stack.
pub fn install(window: &mut Window, cx: &mut App) {
    let region = cx.new(Region::new);
    layers::install_approval_region(region.into(), window, cx);
}

/// The region's view: what the user did there; everything else is read from the store as it draws.
pub struct Region {
    /// The request the user chose to read, kept by id while it waits ([`model::selected`]).
    chosen: Option<String>,
    /// The answer bar's pause, shared with the Add host dialog an agent's host add opens.
    arming: Entity<Arming>,
    /// Why the card's last action opened nothing (another dialog is open, the tunnel warning's consent was not saved),
    /// shown on that request's card until the next press.
    refused: Option<(String, HostText)>,
    /// What a dialog opened from a card names as where focus returns when no control held focus at the press (a click
    /// does not focus a button on macOS). Never drawn, so closing falls back to what held focus when the dialog opened,
    /// or to the focus coordinator's recovery.
    home: FocusHandle,
    /// The waiting list's scroll state, so Tab onto a header below its fold scrolls it into view.
    list_scroll: gpui::ScrollHandle,
    /// The whole region's scroll state in a window too short even for the fixed parts, where the region scrolls as a
    /// whole and Tab onto an answer below the fold scrolls it into view.
    region_scroll: gpui::ScrollHandle,
    /// What the fixed parts took as they were last drawn.
    pinned: Pinned,
    /// The expanded card's body scroll state, and the request it belongs to: a different card starts at its top.
    body_scroll: (Option<String>, gpui::ScrollHandle),
    /// Restarts the pause whenever what the region shows changes, and redraws.
    _store: session::ObservingCurrent,
    /// Redraws when the late-answer notice comes or goes, and when the pause ends.
    _observed: Vec<gpui::Subscription>,
}

impl Region {
    fn new(cx: &mut Context<Self>) -> Self {
        let arming = cx.new(|_| Arming::default());
        let mut observed = vec![cx.observe(&arming, |_, _, cx| cx.notify())];
        if let Some(notices) = notices::entity(cx) {
            observed.push(cx.observe(&notices, |_, _, cx| cx.notify()));
        }
        let store = session::observe_current(cx, |region: &mut Self, cx| region.observe(cx));
        Region {
            chosen: None,
            arming,
            refused: None,
            home: cx.focus_handle(),
            list_scroll: gpui::ScrollHandle::new(),
            region_scroll: gpui::ScrollHandle::new(),
            pinned: Pinned::default(),
            body_scroll: (None, gpui::ScrollHandle::new()),
            _store: store,
            _observed: observed,
        }
    }

    /// The waiting requests, oldest first, as the helm pushed them.
    fn waiting(cx: &App) -> Vec<PendingApproval> {
        session::current_store(cx)
            .map(|store| {
                store
                    .read(cx)
                    .collections()
                    .ordered::<PendingApproval>()
                    .into_iter()
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The store changed: forget a choice whose request is gone (the oldest opens), tell the pause what is shown now (a
    /// change disarms the answers for 700 ms), and redraw.
    fn observe(&mut self, cx: &mut Context<Self>) {
        let waiting = Self::waiting(cx);
        if self
            .chosen
            .as_ref()
            .is_some_and(|chosen| !waiting.iter().any(|approval| approval.id == *chosen))
        {
            self.chosen = None;
        }
        if self
            .refused
            .as_ref()
            .is_some_and(|(id, _)| !waiting.iter().any(|approval| approval.id == *id))
        {
            self.refused = None;
        }
        let expanded =
            model::selected(&waiting, self.chosen.as_deref()).map(|approval| approval.id.clone());
        let ids = waiting.into_iter().map(|approval| approval.id).collect();
        self.arming
            .update(cx, |arming, cx| arming.observe(ids, expanded, cx));
        cx.notify();
    }

    /// A header was pressed: its request opens, and the answers pause as for any change of the expanded request.
    fn choose(&mut self, id: String, cx: &mut Context<Self>) {
        self.chosen = Some(id);
        self.refused = None;
        self.observe(cx);
    }

    /// The expanded card for `approval`, one of `waiting` requests, `width` wide.
    fn card(
        &mut self,
        approval: &PendingApproval,
        waiting: usize,
        width: Pixels,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let model = Card::of(approval, tunnel::offer(crate::actions::THIS_PLATFORM));
        let key = OpKey::approval(&approval.id);
        let operations = session::current_operations(cx);
        let answering = operations
            .as_ref()
            .is_some_and(|operations| operations.read(cx).in_flight(&key).is_some());
        // The card's own refusal first (it is the newer: it was set by the last press), then the helm's refusal of the
        // last answer, which stays on the request's key.
        let refusal = self
            .refused
            .as_ref()
            .filter(|(id, _)| *id == approval.id)
            .map(|(_, words)| words.clone())
            .or_else(|| {
                operations.and_then(|operations| {
                    operations
                        .read(cx)
                        .failure(&key)
                        .map(|failure| failure.message.clone())
                })
            });
        let state = CardState {
            width,
            waiting,
            busy: answering || !self.arming.read(cx).armed(),
            refusal,
            measure: Some(self.pinned.card.clone()),
            body_scroll: Some(self.body_scroll(&approval.id)),
        };
        let region = cx.weak_entity();
        let pressed = approval.clone();
        let on_answer: card::OnAnswer = Rc::new(move |answer, window, cx| {
            let _ = region.update(cx, |region, cx| region.answer(&pressed, answer, window, cx));
        });
        card::card(&model, &state, on_answer, cx)
    }

    /// The body scroll state for the card of request `id`: kept while that card stays expanded, new for another.
    fn body_scroll(&mut self, id: &str) -> gpui::ScrollHandle {
        if self.body_scroll.0.as_deref() != Some(id) {
            self.body_scroll = (Some(id.to_owned()), gpui::ScrollHandle::new());
        }
        self.body_scroll.1.clone()
    }

    /// An answer button was pressed on `approval`'s card. A host add's Allow opens the Add host dialog instead
    /// ([`super::host_add`]), and Tunnel and open asks the tunnel warning first when it must ([`tunnel`]); every other
    /// answer is sent as it is. Each checks the answer bar's pause when the press arrives.
    fn answer(
        &mut self,
        approval: &PendingApproval,
        answer: ApprovalAnswer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.refused = None;
        let mark = Mark::take(cx);
        let id = approval.id.clone();
        match (&approval.action, answer) {
            (ApprovalAction::HostAdd { .. }, ApprovalAnswer::Allow) => {
                let Some(request) = AgentRequest::of(approval, self.arming.clone()) else {
                    return;
                };
                let opener = window.focused(cx).unwrap_or_else(|| self.home.clone());
                let refused =
                    crate::hosts::add::open_for_agent(request, &opener, &mark, window, cx)
                        .err()
                        .and_then(NotOpened::reason);
                // The app's own words, drawn as text on the card's refusal line like the helm's.
                self.refused = refused.map(|words| (id, HostText::from_host(words)));
            }
            (_, ApprovalAnswer::TunnelAndOpen) => {
                let region = cx.weak_entity();
                let refused_id = id.clone();
                let pressed = tunnel::tunnel_and_open(
                    approval,
                    &self.arming,
                    &mark,
                    window,
                    cx,
                    move |reason, cx| {
                        let _ = region.update(cx, |region, cx| {
                            region.refused = Some((refused_id, reason));
                            cx.notify();
                        });
                    },
                );
                match pressed {
                    // The app's own words, drawn as text on the card's refusal line like the helm's.
                    Ok(pressed) => {
                        if let Some(words) = pressed.reason() {
                            self.refused = Some((id, HostText::from_host(words)));
                        }
                    }
                    Err(not_started) => {
                        tracing::debug!(?not_started, "Tunnel and open not started")
                    }
                }
            }
            (_, answer) => {
                if let Err(not_started) =
                    answers::send(&approval.id, answer, None, &self.arming, cx)
                {
                    tracing::debug!(?not_started, "approval answer not sent");
                }
            }
        }
        cx.notify();
    }

    /// The other waiting requests, as headers that open their card (`.approval-waiting-list`): about three show, and
    /// the list scrolls within itself beyond that.
    fn waiting_list(&mut self, others: &[&PendingApproval], cx: &mut Context<Self>) -> AnyElement {
        let region = cx.weak_entity();
        let list = div()
            .id("approval-waiting-list")
            .role(Role::Group)
            .aria_label("Other waiting requests")
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(WAITING_GAP))
            .max_h(px(WAITING_MAX))
            .overflow_y_scroll()
            .children(others.iter().map(|approval| {
                let region = region.clone();
                let id = approval.id.clone();
                waiting_header(approval, cx).on_click(move |_, _, cx| {
                    let id = id.clone();
                    let _ = region.update(cx, |region, cx| region.choose(id, cx));
                })
            }))
            .child(height_tracker(self.pinned.list.clone()));
        crate::focus::scroll_region(list, &self.list_scroll).into_any_element()
    }
}

impl Render for Region {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let waiting = Self::waiting(cx);
        let late = notices::posted(cx)
            .into_iter()
            .find(|notice| matches!(notice, Notice::LateAnswer { .. }));
        let pane = cx
            .try_global::<Anchors>()
            .and_then(|anchors| anchors.pane.as_ref())
            .and_then(Anchor::drawn_bounds);
        // Drawn only over a live UI session: a late-answer notice outlives a disconnect, and the pane's anchor still
        // answers with its last bounds on the first inert frame ([`Anchor::drawn_bounds`]'s note).
        let live = session::current_store(cx).is_some();
        let (Some(pane), false, true) = (pane, waiting.is_empty() && late.is_none(), live) else {
            // Nothing with a size or a listener: an empty region must cover nothing (the slot's contract). The
            // measurements of parts no longer drawn are forgotten, so they do not hold the next card's placement.
            self.pinned = Pinned::default();
            return div().into_any_element();
        };
        // The terminal area's top only while a session is open, by the store's word, never by falling back on a stale
        // anchor ([`Anchor::drawn_bounds`]'s note).
        let area_top = session_open(cx)
            .then(|| {
                cx.try_global::<Anchors>()
                    .and_then(|anchors| anchors.terminal_area.as_ref())
                    .and_then(Anchor::drawn_bounds)
                    .map(|area| area.origin.y)
            })
            .flatten();
        let expanded = model::selected(&waiting, self.chosen.as_deref()).cloned();
        let others: Vec<&PendingApproval> = waiting
            .iter()
            .filter(|approval| Some(&approval.id) != expanded.as_ref().map(|expanded| &expanded.id))
            .collect();
        if late.is_none() {
            self.pinned.notice.set(px(0.));
        }
        if others.is_empty() {
            self.pinned.list.set(px(0.));
        }
        if expanded.is_none() {
            self.pinned.card = CardMeasure::default();
        }
        let occupants = usize::from(late.is_some())
            + usize::from(expanded.is_some())
            + usize::from(!others.is_empty());
        let place = place(
            pane,
            area_top,
            window.viewport_size().height,
            self.pinned.total(occupants),
        );
        let notice = late.map(|notice| late_notice(notice, self.pinned.notice.clone(), cx));
        let card = expanded.map(|approval| self.card(&approval, waiting.len(), place.width, cx));
        let list = (!others.is_empty()).then(|| self.waiting_list(&others, cx));
        let region = div()
            .id("approval-region")
            .debug_selector(|| "approval-region".into())
            .role(Role::Region)
            .aria_label("Requests waiting for your approval")
            .occlude()
            .absolute()
            .left(place.left)
            .top(place.top)
            .w(place.width)
            .max_h(place.cap)
            .flex()
            .flex_col()
            .gap(px(GAP))
            // In a window shorter than even the fixed parts, the region scrolls as a whole, every part at its own
            // height (the old `[data-tiny-pane]` rules); otherwise the card's body gives way inside the cap.
            .when(place.scrolls, |region| region.overflow_y_scroll())
            .when(!place.scrolls, |region| region.overflow_hidden())
            .children(notice)
            .children(card.map(|card| {
                div()
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .when(place.scrolls, |slot| slot.flex_none())
                    .when(!place.scrolls, |slot| slot.flex_shrink_1())
                    .child(card)
            }))
            .children(list);
        if place.scrolls {
            crate::focus::scroll_region(region, &self.region_scroll).into_any_element()
        } else {
            region.into_any_element()
        }
    }
}

// ---- Parts -----------------------------------------------------------------------------------------------------------

/// One waiting request's header (`.approval-waiting`): what it asks, bold, then "From host" and the host as host text.
/// Its accessible name says both, the host escaped as everywhere.
fn waiting_header(approval: &PendingApproval, cx: &App) -> Button {
    let palette = theme::palette(cx);
    let summary = model::sentence_case(model::summary(&approval.action));
    let host = HostText::from_host(approval.host_name.as_str());
    let name = HostSentence::new(SharedString::from(format!(
        "approval-{}-waiting-name",
        approval.id
    )))
    .text(format!("{summary}, from host "))
    .host(&host)
    .accessible_name();
    Button::new(
        SharedString::from(format!("approval-{}-waiting", approval.id)),
        name,
        HoverHelp::tooltip("Show this request: open its card in place of the one shown"),
        Role::Button,
    )
    .tier(Tier::Waiting)
    .content(
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_x(px(12.))
            .gap_y(px(4.))
            .min_w_0()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(summary))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .min_w_0()
                    .child(
                        div()
                            .text_color(theme::color(palette.fg_dim))
                            .child("From host"),
                    )
                    .child(host_text::element(&host)),
            ),
    )
}

/// The late-answer notice (`.approval-late`), above the card: its words and Dismiss, which withdraws it. Its height is
/// recorded in `height` for the region's placement.
fn late_notice(notice: Notice, height: Rc<Cell<Pixels>>, cx: &App) -> AnyElement {
    let palette = theme::palette(cx);
    div()
        .id("approval-late")
        .role(Role::Status)
        .aria_label(notice.text())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .py(px(8.))
        .px(px(12.))
        .bg(theme::color(palette.bg_2))
        .border_1()
        .border_color(theme::color(palette.border))
        .rounded(metrics::RADIUS_MD)
        .shadow(layers::menu_shadow(palette))
        .child(div().flex_1().min_w_0().child(notice.text()))
        .child(
            Button::new(
                "approval-late-dismiss",
                "Dismiss",
                HoverHelp::tooltip("Dismiss: hide this notice"),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .on_click(move |_, _, cx| notices::withdraw(notice, cx)),
        )
        .child(height_tracker(height))
        .into_any_element()
}

/// Whether a session is open: one is selected and the store still has it. The terminal area's anchor is read only
/// then ([`Anchor::drawn_bounds`]'s note).
fn session_open(cx: &App) -> bool {
    local_state::selected(cx).is_some_and(|key| {
        session::current_store(cx).is_some_and(|store| {
            store
                .read(cx)
                .collections()
                .get::<SessionRow>(&key)
                .is_some()
        })
    })
}
