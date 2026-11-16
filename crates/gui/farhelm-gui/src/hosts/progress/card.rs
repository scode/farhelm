//! The host progress card: a running setup's, Update's or uninstall's whole run on one card, for the hover help of the
//! two places that show the run in a line too short for it, the Hosts row's inline status and the attention list's
//! working rows.
//!
//! SPEC.md, "Topology" ("Hovering that status opens the same card as a working host in the attention list: the host's
//! mark and name, operation and completed-step count with elapsed time, every step with its status and explanation,
//! the current step highlighted, and any degraded or failed step's reason. Connection detail follows the steps while
//! the host is not connected ... and the run's own message follows when present. In a short window the step list
//! shrinks, keeping the current step in view ... If connection detail or a run message cannot fit beside a readable
//! current step, only the oversized text becomes a preview of whole lines from its end and the card says `Full details
//! in Hosts`. Messages that fit stay whole. The card remains non-interactive") and "Session list" (the attention list's
//! working rows). Ported from the old app's `HostProgressCard` in `hosts.rs`, the fit in `tooltip.js`
//! (`fitHostProgress`, `diagnosticPreviewHeights`, `followCurrentStep`) and the `.host-progress-*` and
//! `.farhelm-tooltip[data-kind="card"]` rules in `app.css`, at reference commit `24bc0580`.
//!
//! ## Entry point
//!
//! [`progress_card`] makes the card of one host's running run, or says there is none. The card is a view of its own
//! rather than an element, because it counts the run's elapsed time and pulses its running step, and the ticker hands
//! its clock and pulse only to a view; the layer manager's card form of hover help takes the caller's view as content.
//! The card reads the store whenever it draws, so it follows the run and the host's phase as they are pushed, and it
//! counts from the same first sight of the run as the Hosts row ([`super::seen_now`]), so the two surfaces agree.
//!
//! Its owner makes it when the card is to show and drops it when it goes: from its first draw it holds the ticker's
//! one-second clock, and the shared pulse while its window is active, until it is dropped, since a view cannot tell
//! when it stops being drawn.
//!
//! The card draws only its content. The layer manager's card form of hover help draws the chrome around it (the
//! tooltip's ground, border, radius and shadow) at the width its caller passes, the old 310 pixels
//! (`.farhelm-tooltip[data-kind="card"]`), and the card fills that width.
//!
//! Both of its places offer the card as lazy card help placed beside the sidebar, which builds it with [`view`] when the
//! tooltip shows and drops it when it hides: the Hosts row's running status ([`super::status_mark`]) and the attention
//! list's working rows (`components::HoverHelp::card_with`).
//!
//! ## The fit
//!
//! The card is at most the window's height less 10 pixels, as the old card was, and in a window too short for it the
//! parts give up height in the old order. The step list goes first: the steps before and after the current one (or the
//! first one, while none runs) are clipped away around it, so the current step is always whole. The connection detail
//! and the run's message keep their whole text while both fit beside the header and that one step; otherwise each that
//! cannot keep it shows its last whole lines, sharing the room with the other as the old preview did (the shorter one
//! whole when it fits in its half), and the card ends with `Full details in Hosts`. The heights are the ones the card
//! was last drawn at ([`Measured`]), as the old fit measured the laid-out card: each part records its height as it is
//! drawn and asks for one more frame when it changed, and the next frame fits from them. So the first frame of a card
//! that does not fit, and the first after a resize or a longer message, is drawn unfitted, clipped at the card's height,
//! and the frame after it fitted; the fitted parts' heights do not depend on the caps, so that settles at once.
//!
//! NOTE: Deviations from the old card, to surface to the maintainer. The connection detail sets host values as the
//! Hosts row's details do (builds and identities monospace, an error in the interface face), where the old card set
//! every host value monospace. A shrunk step list shows its steps before and after the current one in proportion to how
//! much of each there is, where the old list centered the current step.

#[cfg(test)]
pub(crate) mod tests;

use farhelm_proto::provisioning::{ProvisioningView, RunStatus, StepStatus};
use farhelm_proto::ui_channel::HostView;
use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Pixels, Render, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, canvas, div, prelude::FluentBuilder as _,
    px,
};

use super::{Run, Status, Surface, clock, seen_now, shown, status, step_row};
use crate::assets;
use crate::components::pulse_opacity;
use crate::host_text::{HostSentence, HostText};
use crate::hosts::identity::{self, host_mark, phase};
use crate::local_time;
use crate::render_count;
use crate::session;
use crate::theme::{self, Palette, metrics};
use crate::ticker;

/// The width the card's hover help gives it, which the card fills: the old `.farhelm-tooltip[data-kind="card"]`'s
/// `width: min(310px, 100vw - 8px)` (the layer manager keeps it inside a narrower window).
pub(crate) const WIDTH: Pixels = px(310.);

/// `.host-progress-card`'s `max-height: calc(100vh - 10px)`: room kept below the window's height. The chrome around
/// the card clips it at the window's height less 8 pixels, inside its one-pixel border on each side, so this is the
/// most the card's own content may take. The card applies it itself rather than leaving it to that clip: the cap is
/// what makes its step list shrink around the current step, and what the fit's room is counted from, where a clip
/// would cut off the card's end, notice included.
const WINDOW_ROOM: f32 = 10.;

/// The sections' side padding (`padding: ... 10px`).
const PAD_X: f32 = 10.;

/// `.host-progress-header`'s `padding: 9px 10px 8px`, `.host-progress-title`'s gap, `.host-progress-meta`'s
/// `margin-top: 5px`, and the host mark's size (`.host-kind-icon`).
const HEADER_PAD_TOP: f32 = 9.;
const HEADER_PAD_BOTTOM: f32 = 8.;
const TITLE_GAP: f32 = 6.;
const META_GAP: f32 = 5.;
const MARK: f32 = 12.;

/// `.host-progress-elapsed`'s smaller size.
const ELAPSED_SIZE: f32 = 11.;

/// The words after the elapsed time, with the space before them kept in a flex row.
const ELAPSED: &str = "\u{a0}elapsed";

/// `.host-progress-steps`' `padding: 7px 10px`.
const LIST_PAD_Y: f32 = 7.;

/// `.host-progress-connection` and `.host-progress-run-message`: `padding: 8px 10px` under a one-pixel rule, and the
/// "Connection" label's `margin-bottom: 3px`.
const SECTION_PAD_Y: f32 = 8.;
const RULE: f32 = 1.;
const LABEL_GAP: f32 = 3.;

/// `.host-progress-connection-detail, .host-progress-run-detail`'s `line-height: 1.5` at the 12-pixel size: the
/// definite line a preview keeps whole lines of.
const DIAGNOSTIC_LINE: f32 = 18.;

/// `.host-progress-preview-notice`'s `padding: 6px 10px`.
const NOTICE_PAD_Y: f32 = 6.;

/// What the preview notice says: where the whole text is.
const FULL_DETAILS: &str = "Full details in Hosts";

/// The notice's height, which the fit sets aside before it is drawn: its padding and rule around one line at the
/// tooltip's `line-height: 1.35`, which the card inherited.
fn notice_height() -> f32 {
    RULE + 2. * NOTICE_PAD_Y + f32::from(metrics::TOOLTIP_LINE_HEIGHT)
}

// ---- The entry point ------------------------------------------------------------------------------------------------

/// The progress card of host `host`'s running setup, Update or uninstall, or `None` when the host has none: no run the
/// helm retains, a run that finished or failed, or only requests out (a pending "Setting up…" or "Updating…" has no
/// steps to show, and the old app showed no card for it either). A new view each call: its owner keeps it for as long
/// as the card shows and drops it then (the module docs say why). Nothing while the window is inert.
pub fn progress_card(host: i64, cx: &mut App) -> Option<Entity<ProgressCard>> {
    running(host, cx)?;
    Some(view(host, cx))
}

/// The card of host `host`, made whether or not a run is going on: what lazy card help ([`HoverHelp::card_with`],
/// which takes a view, never none) builds when the tooltip shows. When the run ends while its card shows, which happens
/// whenever the user watches a run finish, the card draws nothing (an empty card, for about a frame) until the hover
/// ends: the phase words that replace the status are drawn under the same id, so the control's hover carries over to
/// them, and GPUI's check at paint time, finding the pointer not over the words, ends it, which takes the tooltip away.
/// A run that ends during the hover delay instead has its waiting tooltip cancelled the same way, before anything is
/// built.
///
/// [`HoverHelp::card_with`]: crate::components::HoverHelp::card_with
pub(crate) fn view(host: i64, cx: &mut App) -> Entity<ProgressCard> {
    cx.new(|_| ProgressCard {
        host,
        clock: None,
        pulse: None,
        measured: Measured::default(),
    })
}

/// Host `host`'s retained run, while it is a running run the card draws.
fn running(host: i64, cx: &App) -> Option<ProvisioningView> {
    let store = session::current_store(cx)?;
    let collections = store.read(cx).collections();
    shown(collections.get::<ProvisioningView>(&HostView::key(host)))
        .filter(|run| run.status == RunStatus::Running)
        .cloned()
}

// ---- The view -------------------------------------------------------------------------------------------------------

/// The card of one host's running run ([`progress_card`]).
pub struct ProgressCard {
    host: i64,
    /// The one-second clock, held from the first draw that shows a run, for the elapsed time.
    clock: Option<ticker::Clock>,
    /// The shared pulse, held while the window is active and the run has a running step, for its ring.
    pulse: Option<ticker::Pulsing>,
    /// The heights its parts were last drawn at, for the fit.
    measured: Measured,
}

/// The heights the card's parts were last drawn at, which the fit works from ([`tracker`] records each); zero before
/// the part was first drawn.
#[derive(Default)]
struct Measured {
    header: Rc<Cell<Pixels>>,
    /// The step the list keeps whole; zero while there is none.
    pivot: Rc<Cell<Pixels>>,
    connection: Diagnostic,
    message: Diagnostic,
}

/// One diagnostic section's heights as last drawn: the whole section, its text box as shown (cut or whole), and its
/// text whole. The section less the box is what the section takes besides its text.
#[derive(Clone, Default)]
struct Diagnostic {
    section: Rc<Cell<Pixels>>,
    shown: Rc<Cell<Pixels>>,
    whole: Rc<Cell<Pixels>>,
}

impl Diagnostic {
    /// What the section takes besides its text, and its text's whole height.
    fn overhead_and_whole(&self) -> (f32, f32) {
        (
            f32::from(self.section.get() - self.shown.get()),
            f32::from(self.whole.get()),
        )
    }
}

impl ProgressCard {
    /// Take the clock while a run shows, and the pulse while one of its steps runs in an active window; let go of
    /// both otherwise. The ticker's rule: no pulse for a window nobody is looking at.
    fn keep_time(&mut self, shows: bool, pulsing: bool, window: &Window, cx: &mut Context<Self>) {
        let pulses = shows && pulsing && ticker::is_active(window, cx);
        if pulses != self.pulse.is_some() {
            self.pulse = pulses.then(|| ticker::pulse(window, cx));
        }
        if shows != self.clock.is_some() {
            self.clock = shows.then(|| ticker::every_second(cx));
        }
    }
}

impl Render for ProgressCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("host-progress-card", cx);
        let palette = theme::palette(cx);
        let drawn = Card::read(self.host, cx);
        let pulsing = drawn.as_ref().is_some_and(|card| {
            card.run
                .steps
                .iter()
                .any(|step| step.status == StepStatus::Running)
        });
        self.keep_time(drawn.is_some(), pulsing, window, cx);
        let pulse = pulse_opacity(
            self.pulse.as_ref().map_or(0, |pulse| pulse.step(cx)),
            palette.status_pulse_min,
        );
        match drawn {
            Some(card) => card.element(&self.measured, pulse, palette, window),
            // The run ended under the card: it shows nothing until its owner drops it.
            None => div().into_any_element(),
        }
    }
}

// ---- What the card draws --------------------------------------------------------------------------------------------

/// Everything one frame of the card is drawn from, read from the store and what the UI keeps about the run.
struct Card {
    host: HostView,
    run: ProvisioningView,
    kind: Run,
    done: usize,
    total: usize,
    elapsed: std::time::Duration,
    /// The connection detail, while the host is not connected.
    connection: Option<HostSentence>,
    /// The run's own message.
    message: Option<HostSentence>,
}

impl Card {
    /// Host `host`'s card as things stand, or `None` when the host is not listed or has no running run.
    fn read(host: i64, cx: &App) -> Option<Card> {
        let run = running(host, cx)?;
        let store = session::current_store(cx)?;
        let collections = store.read(cx).collections();
        let entry = collections.get::<HostView>(&HostView::key(host))?.clone();
        let seen = seen_now(cx);
        let Some(Status::Running {
            run: kind,
            done,
            total,
            elapsed,
            ..
        }) = status(Some(&run), seen.get(&host), None, ticker::wall_now(cx))
        else {
            return None;
        };
        let twin = |id: i64| {
            collections
                .get::<HostView>(&HostView::key(id))
                .map(identity::display_name)
        };
        let connection = phase::connection_line(
            &entry,
            format!("host-card-connection-{host}"),
            twin,
            local_time::zone(cx),
        );
        let message = run.message.as_deref().map(|message| {
            HostSentence::new(SharedString::from(format!("host-card-run-message-{host}")))
                .host(&HostText::from_host(message))
        });
        Some(Card {
            host: entry,
            run,
            kind,
            done,
            total,
            elapsed,
            connection,
            message,
        })
    }

    /// The card's step the list keeps whole: the running one, or the first while none runs (`None` with no steps).
    fn pivot(&self) -> Option<usize> {
        let steps = &self.run.steps;
        steps
            .iter()
            .position(|step| step.status == StepStatus::Running)
            .or((!steps.is_empty()).then_some(0))
    }

    /// The card (`.host-progress-card`) at this frame's `pulse` for its running step's ring, fitted from the heights
    /// `measured` holds and recording them again.
    fn element(
        self,
        measured: &Measured,
        pulse: f32,
        palette: &Palette,
        window: &Window,
    ) -> AnyElement {
        let id = self.host.id;
        let pivot = self.pivot();
        if pivot.is_none() {
            measured.pivot.set(px(0.));
        }
        // The room the diagnostics may take, before the notice: the card's limit less the header, the list kept down
        // to its one whole step, and what each diagnostic section takes besides its text.
        let mut room = f32::from(
            window.viewport_size().height
                - px(WINDOW_ROOM)
                - measured.header.get()
                - measured.pivot.get(),
        ) - 2. * LIST_PAD_Y;
        let mut heights = Vec::new();
        for (drawn, diagnostic) in [
            (self.connection.is_some(), &measured.connection),
            (self.message.is_some(), &measured.message),
        ] {
            if drawn {
                let (overhead, whole) = diagnostic.overhead_and_whole();
                room -= overhead;
                heights.push(whole);
            }
        }
        let fit = fit(&heights, room, notice_height(), DIAGNOSTIC_LINE);
        let mut caps = fit.caps.iter().copied();
        let connection = self.connection.clone().map(|text| {
            section(&measured.connection, palette)
                .child(
                    div()
                        .mb(px(LABEL_GAP))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::color(palette.fg_0))
                        .child("Connection"),
                )
                .child(diagnostic(
                    text,
                    caps.next().flatten(),
                    &measured.connection,
                ))
        });
        let message = self.message.clone().map(|text| {
            section(&measured.message, palette).child(diagnostic(
                text,
                caps.next().flatten(),
                &measured.message,
            ))
        });
        let notice = fit.notice.then(|| {
            div()
                .id(SharedString::from(format!("host-card-notice-{id}")))
                .role(Role::Label)
                .aria_label(FULL_DETAILS)
                .flex_none()
                .py(px(NOTICE_PAD_Y))
                .px(px(PAD_X))
                .border_t_1()
                .border_color(theme::color(palette.border_dim))
                .text_color(theme::color(palette.fg_2))
                .child(FULL_DETAILS)
        });
        div()
            .id(SharedString::from(format!("host-card-{id}")))
            .w_full()
            .max_h(window.viewport_size().height - px(WINDOW_ROOM))
            .flex()
            .flex_col()
            .overflow_hidden()
            .text_size(metrics::TEXT_SM)
            .line_height(metrics::TOOLTIP_LINE_HEIGHT)
            .text_color(theme::color(palette.fg_1))
            .child(self.header(&measured.header, palette))
            .child(self.steps(pivot, &measured.pivot, pulse, palette))
            .children(connection)
            .children(message)
            .children(notice)
            .into_any_element()
    }

    /// `.host-progress-header`: the host's mark and short name, then "<verb>: N of M steps done" and the elapsed time.
    /// The count and the time are labels of their own, which is how tests and the agent bridge read them. Its height
    /// goes to `height` as it is drawn.
    fn header(&self, height: &Rc<Cell<Pixels>>, palette: &Palette) -> AnyElement {
        let id = self.host.id;
        let count = self.count();
        let elapsed = clock(self.elapsed);
        div()
            .relative()
            .flex_none()
            .pt(px(HEADER_PAD_TOP))
            .pb(px(HEADER_PAD_BOTTOM))
            .px(px(PAD_X))
            .border_b_1()
            .border_color(theme::color(palette.border_dim))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(TITLE_GAP))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_0))
                    .child(host_mark(
                        self.host.icon,
                        self.host.color,
                        px(MARK),
                        palette,
                    ))
                    .child(
                        HostSentence::new(SharedString::from(format!("host-card-title-{id}")))
                            .host(&identity::short_name(&self.host)),
                    ),
            )
            .child(
                div()
                    .mt(px(META_GAP))
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .justify_between()
                    .gap(px(TITLE_GAP))
                    .child(
                        div()
                            .id(SharedString::from(format!("host-card-count-{id}")))
                            .role(Role::Label)
                            .aria_label(count.clone())
                            .min_w_0()
                            .child(count),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("host-card-elapsed-{id}")))
                            .role(Role::Label)
                            .aria_label(format!("{elapsed} elapsed"))
                            .flex_none()
                            .flex()
                            .flex_row()
                            .whitespace_nowrap()
                            .text_size(px(ELAPSED_SIZE))
                            .text_color(theme::color(palette.fg_2))
                            .child(div().font_family(assets::TERMINAL_FONT).child(elapsed))
                            .child(ELAPSED),
                    ),
            )
            .child(tracker(height.clone()))
            .into_any_element()
    }

    /// `.host-progress-steps`: every step, drawn by the step list's own rows ([`step_row`] on [`Surface::Card`]), kept
    /// at least as tall as the `pivot` step as last drawn (`pivot_height`, which records it again) and its padding. The
    /// rows before the pivot and after it are clipped away first, those before from their top and those after from
    /// their bottom, so the pivot stays whole and the steps nearest it stay in view.
    fn steps(
        &self,
        pivot: Option<usize>,
        pivot_height: &Rc<Cell<Pixels>>,
        pulse: f32,
        palette: &Palette,
    ) -> AnyElement {
        let id = self.host.id;
        let count = self.run.steps.len();
        let mut rows: Vec<AnyElement> = self
            .run
            .steps
            .iter()
            .enumerate()
            .map(|(n, step)| step_row(Surface::Card, id, n, step, n + 1 == count, pulse, palette))
            .collect();
        let list = div()
            .id(SharedString::from(format!("host-card-steps-{id}")))
            .role(Role::List)
            .aria_label(self.kind.steps())
            .min_h(pivot_height.get() + px(2. * LIST_PAD_Y))
            .py(px(LIST_PAD_Y))
            .px(px(PAD_X))
            .flex()
            .flex_col()
            .overflow_hidden();
        let Some(pivot) = pivot else {
            return list.into_any_element();
        };
        let after: Vec<AnyElement> = rows.split_off(pivot + 1);
        let current = rows.pop();
        list.child(
            div()
                .flex()
                .flex_col()
                .justify_end()
                .min_h_0()
                .overflow_hidden()
                .children(rows.into_iter().map(unshrunk)),
        )
        .children(current.map(|row| {
            div()
                .relative()
                .flex_none()
                .child(row)
                .child(tracker(pivot_height.clone()))
        }))
        .child(
            div()
                .flex()
                .flex_col()
                .min_h_0()
                .overflow_hidden()
                .children(after.into_iter().map(unshrunk)),
        )
        .into_any_element()
    }

    /// The header's count: "<verb>: N of M steps done".
    fn count(&self) -> String {
        format!(
            "{}: {} of {} steps done",
            self.kind.verb(),
            self.done,
            self.total
        )
    }
}

/// A step row that keeps its height inside a clipping part of the list, which would otherwise shrink it.
fn unshrunk(row: AnyElement) -> AnyElement {
    div().flex_none().child(row).into_any_element()
}

/// `.host-progress-connection` and `.host-progress-run-message`: a section under a rule, in the amber prose ink, whose
/// height goes to `measured` as it is drawn.
fn section(measured: &Diagnostic, palette: &Palette) -> gpui::Div {
    div()
        .relative()
        .child(tracker(measured.section.clone()))
        .flex_none()
        .py(px(SECTION_PAD_Y))
        .px(px(PAD_X))
        .border_t_1()
        .border_color(theme::color(palette.border_dim))
        .text_color(theme::color(palette.warn_bright))
}

/// A diagnostic's text at its definite line, whole, or cut to its last whole lines within `cap` pixels: the box ends
/// at the text's end, so what it clips is the text's start. The box's height and the text's whole height go to
/// `measured` as they are drawn.
fn diagnostic(text: HostSentence, cap: Option<f32>, measured: &Diagnostic) -> AnyElement {
    div()
        .relative()
        .line_height(px(DIAGNOSTIC_LINE))
        .flex()
        .flex_col()
        .justify_end()
        .overflow_hidden()
        .when_some(cap, |text, cap| text.max_h(px(cap)))
        .child(
            div()
                .relative()
                .flex_none()
                .child(text)
                .child(tracker(measured.whole.clone())),
        )
        .child(tracker(measured.shown.clone()))
        .into_any_element()
}

/// An element filling its `relative()` parent that records the parent's height in `cell` as it is drawn, and draws the
/// window again when the height changed, so the fit catches up with what layout made of the card. The redraw is
/// deferred to after this frame: the tracker runs while the frame is prepared, where a refresh is dropped. Copied from
/// `height_tracker` in `approvals/region.rs` (that area's own), for one shared helper to replace both later.
fn tracker(cell: Rc<Cell<Pixels>>) -> impl IntoElement {
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

// ---- The fit --------------------------------------------------------------------------------------------------------

/// How the diagnostics fit: per diagnostic, in order, the height its preview is cut to, or `None` to keep it whole; and
/// whether the card ends with the notice.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Fit {
    pub(crate) caps: Vec<Option<f32>>,
    pub(crate) notice: bool,
}

/// Fit diagnostics `heights` high (each whole, in pixels) into `room`, with `notice` the notice's height and `line` the
/// diagnostics' line. Everything whole and no notice when the whole texts fit (with half a pixel to spare for
/// rounding); otherwise the notice takes its room, the rest is shared ([`shares`]), and each diagnostic that does not
/// get its whole height is cut to the whole lines its share holds, which may be none.
pub(crate) fn fit(heights: &[f32], room: f32, notice: f32, line: f32) -> Fit {
    if heights.is_empty() || heights.iter().sum::<f32>() <= room + 0.5 {
        return Fit {
            caps: vec![None; heights.len()],
            notice: false,
        };
    }
    let caps = heights
        .iter()
        .zip(shares(heights, room - notice))
        .map(|(&height, share)| (share < height).then(|| (share / line).floor() * line))
        .collect();
    Fit { caps, notice: true }
}

/// Share `room` between texts `heights` high, without cutting a short text to spare a long one: once the shorter
/// texts fit whole, the longer ones share what is left equally (the old `diagnosticPreviewHeights`). Each share is at
/// most its text's height.
pub(crate) fn shares(heights: &[f32], room: f32) -> Vec<f32> {
    let mut remaining = room.max(0.);
    let mut count = heights.len() as f32;
    let mut cap = 0.;
    let mut sorted = heights.to_vec();
    sorted.sort_by(f32::total_cmp);
    for height in sorted {
        cap = remaining / count;
        if height > cap {
            break;
        }
        remaining -= height;
        count -= 1.;
    }
    heights.iter().map(|height| height.min(cap)).collect()
}
