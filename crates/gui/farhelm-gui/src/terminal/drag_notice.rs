//! The drag-copy notice: what the terminal says when a plain drag went to a program under mouse reporting and nothing
//! was copied (SPEC.md, "Terminal experience"; the old app's `.drag-copy-notice`, `copy-on-select.js` and its driver in
//! `terminal.js` at the reference commit `24bc0580`).
//!
//! Under mouse reporting a plain drag belongs to the program, so Farhelm makes no selection of its own. Programs that
//! copy by themselves answer with a clipboard write (OSC 52), which reaches the clipboard. Programs that only draw a
//! highlight (Codex's prompt box, vim with `mouse=a`) leave the user looking at highlighted text and an unchanged
//! clipboard, with no clue why. The notice is guidance about the program, never a report that a clipboard write
//! failed: SPEC.md keeps clipboard failures silent.
//!
//! This module holds what the notice says ([`text`], with the one agent that has its own instruction answered per kind
//! by [`copy_hint`]), where it sits ([`place`]) and what it draws ([`NoticeView`]). When it shows and goes is the
//! terminal's, in `attached.rs` ("The drag-copy notice"), because it follows the terminal's presses, releases and
//! clipboard writes.

use std::rc::Rc;
use std::time::Duration;

use farhelm_input_mapping::Platform;
use farhelm_proto::AgentKind;
use gpui::{
    App, Bounds, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels,
    Point, Render, SharedString, Size, StatefulInteractiveElement, Styled, Window, div, point, px,
};

use crate::components::{HoverHelp, Role, with_hover_help};
use crate::theme::{self, metrics};

/// How far, in logical pixels, the pointer must move between press and release for the gesture to count as a drag
/// (the old `DRAG_THRESHOLD_PX`). Below this it is a click, which the program treats as a click and nobody expects to
/// copy.
pub(super) const DRAG_THRESHOLD: f32 = 4.;

/// How long after the release the terminal waits for the program's own clipboard write before deciding it copied
/// nothing (the old `OSC52_GRACE_MS`). The old source's reasoning: Codex's copy-on-release over its conversation
/// arrives within a frame, and this leaves generous room for a slow host without making the notice feel unrelated to
/// the drag.
pub(super) const GRACE: Duration = Duration::from_millis(1500);

/// How long the notice stays (SPEC.md: 30 seconds; the old `DRAG_NOTICE_MS`): long enough to read without watching
/// for it. Every notice gets the whole of it, even one that replaces an identical one.
pub(super) const SHOWN_FOR: Duration = Duration::from_secs(30);

/// The notice's distance from the release, in terminal rows, above it or below it: it keeps the guidance off the row
/// the user just dragged over.
const GAP_ROWS: f32 = 3.;

/// The closest the notice comes to the edges of the area the window's content is laid out in (the old shared
/// tooltip's `EDGE`, from the window's edges).
const EDGE: f32 = 4.;

/// How far above the release the notice's bottom edge sits when neither three-row placement fits its pane (the old
/// tooltip's `GAP_ABOVE`, which the old fallback kept).
const FALLBACK_GAP: f32 = 6.;

/// The notice's widest: the old `max-width: min(36em, 100vw - 8px)`, at its 12-pixel text.
const MAX_WIDTH: f32 = 36. * 12.;

/// The notice's element id, and its debug selector.
pub(crate) const NOTICE_ID: &str = "terminal-drag-copy-notice";

/// Its × button's element id, and its debug selector.
pub(crate) const DISMISS_ID: &str = "terminal-drag-copy-notice-dismiss";

/// The × button's accessible name and its hover help, the old button's `aria-label` and `data-tooltip`.
const DISMISS_NAME: &str = "Dismiss drag-copy notice";
const DISMISS_HELP: &str = "Dismiss this notice";

/// The notice's first sentences when the agent has no instruction of its own ([`copy_hint`]), the old words.
const GENERIC_FIRST: &str = "This program handles mouse selection itself, so this drag did not copy anything. Use \
                             the program's own copy command.";

// ---- What it says ----------------------------------------------------------------------------------------------------

/// How to copy in an agent of `kind` when a plain mouse drag over its terminal copied nothing, or `None` when Farhelm
/// knows nothing more specific than the generic advice. The per-kind answer the map in
/// `crates/farhelm-supervisor/src/agent_kind/mod.rs` ("How the GUI shows it") names for this notice, ported from the
/// old `SessionAgentKind::drag_copy_hint` (`crates/farhelm-ui/src/lib.rs` at the reference commit).
///
/// Only Codex has one. Its full-screen interface takes mouse drags itself: over its conversation it copies on release
/// (which the grace window sees as a clipboard write, so no notice shows), but in its prompt box it only highlights,
/// and copies only on Ctrl+C while the highlight is up. The old source verified that against Codex 0.160.0: Ctrl+C
/// with no highlight clears the draft instead, and Ctrl+Insert does not copy, so neither is offered. Telling another
/// agent's user to press Ctrl+C could interrupt it, so every other kind gets the generic advice.
///
/// By the integration kind the supervisor recorded for the session (`SessionInfo::agent_kind`), as the old app keyed
/// it, so a command launch declared as Codex and a legacy Codex session get the instruction too.
#[warn(clippy::wildcard_enum_match_arm)]
pub(crate) const fn copy_hint(kind: AgentKind) -> Option<&'static str> {
    match kind {
        AgentKind::Codex => Some(
            "Codex handles mouse selection itself: to copy what you highlighted, press Ctrl+C while it is still \
             highlighted (without a highlight, Ctrl+C clears your draft).",
        ),
        AgentKind::Claude
        | AgentKind::Goose
        | AgentKind::Pi
        | AgentKind::Omp
        | AgentKind::Grok
        | AgentKind::Muse
        | AgentKind::Generic => None,
    }
}

/// The key that forces Farhelm's own selection under mouse reporting on `platform`, as the notice names it: the one
/// the terminal's forcing rule reads (`forces_selection` in `attached.rs`), so the notice never names a key that does
/// nothing.
fn forcing_key(platform: Platform) -> &'static str {
    match platform {
        Platform::Mac => "Option",
        Platform::Linux => "Shift",
    }
}

/// The notice's words (the old `dragCopyNoticeText`): the agent's own instruction when it has one (`hint`, from
/// [`copy_hint`]) or the generic advice, then the way to copy with Farhelm instead, which is the same for every
/// program.
pub(crate) fn text(hint: Option<&str>, platform: Platform) -> String {
    let first = hint.unwrap_or(GENERIC_FIRST);
    format!(
        "{first} Or hold {} while dragging to select and copy here.",
        forcing_key(platform)
    )
}

// ---- Where it sits ---------------------------------------------------------------------------------------------------

/// Where a notice of `size` goes for a release at `release` (SPEC.md, "Terminal experience"; the old
/// `createDragCopyNotice`'s `reposition`), all in window coordinates. `pane` is where the terminal's whole pane is
/// drawn in this frame and `row_height` the height a row of it is drawn at; `area` is the area the window's content is
/// laid out in, which the layer manager hands its placement rules (`layers::PlaceBy`) and which SPEC.md's "the window"
/// means here.
///
/// - Its bottom edge three terminal rows above the release when that fits inside the pane and `area` (4 pixels in
///   from the area's edges), else its top edge three rows below when that fits.
/// - When neither fits (a pane too short, or no `pane` drawn), its bottom edge 6 pixels above the release, clamped
///   inside `area`.
/// - Horizontally centred on the release, clamped inside `area`; one wider than the area is pinned to its left edge,
///   where its text starts.
///
/// Asked again in every frame the notice is drawn, so a resized window or pane, or a new text size, moves it.
pub(crate) fn place(
    release: Point<Pixels>,
    pane: Option<Bounds<Pixels>>,
    row_height: f32,
    size: Size<Pixels>,
    area: Bounds<Pixels>,
) -> Point<Pixels> {
    let (x, y) = (f32::from(release.x), f32::from(release.y));
    let (width, height) = (f32::from(size.width), f32::from(size.height));
    let (area_left, area_top) = (f32::from(area.left()) + EDGE, f32::from(area.top()) + EDGE);
    let (area_right, area_bottom) = (
        f32::from(area.right()) - EDGE,
        f32::from(area.bottom()) - EDGE,
    );
    let left = (x - width / 2.).min(area_right - width).max(area_left);
    let fallback = (y - FALLBACK_GAP - height)
        .min(area_bottom - height)
        .max(area_top);
    let top = pane.map_or(fallback, |pane| {
        let gap = GAP_ROWS * row_height;
        let (above, below) = (y - gap - height, y + gap);
        // The pane decides where there is room; the area still bounds a pane partly outside it.
        let first = area_top.max(f32::from(pane.top()));
        let last = area_bottom.min(f32::from(pane.bottom()));
        let fits = |top: f32| top >= first && top + height <= last;
        if fits(above) {
            above
        } else if fits(below) {
            below
        } else {
            fallback
        }
    });
    point(px(left), px(top))
}

// ---- What it draws ---------------------------------------------------------------------------------------------------

/// What the × does: put the notice away. Handed in by the terminal, which owns when the notice shows.
pub(super) type Dismiss = Rc<dyn Fn(&mut Window, &mut App)>;

/// The notice's card: its words and its ×, the old `.drag-copy-notice` (the interface face at 12 pixels in the dim
/// foreground, on the floating ground with a 1-pixel edge).
///
/// It is drawn in a popover that never takes focus and lets the pointer through (`layers::Popover::passes_pointer`):
/// SPEC.md has the notice pointer-transparent apart from its ×, so a press on its words is the terminal's next press,
/// which reaches the program and puts the notice away. The × alone takes the pointer, and it takes no focus: it is no
/// tab stop and its press is not the terminal's, so dismissing it leaves focus where it was and sends the program
/// nothing (the old button's `tabindex: -1` and prevented `mousedown`).
///
/// That is why the × is built from a plain element rather than taken from the component layer, which every other
/// control comes from: the layer's `Button` is a tab stop tracked for focus, which a click on Linux focuses. The ×
/// still carries the label, hover help and role the component layer requires of a control, through
/// [`with_hover_help`], as the session list's pin grip (`session_list/pin_drag.rs`) does.
pub(crate) struct NoticeView {
    text: SharedString,
    dismiss: Dismiss,
}

impl NoticeView {
    pub(super) fn new(text: SharedString, dismiss: Dismiss) -> Self {
        NoticeView { text, dismiss }
    }
}

impl Render for NoticeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let area = crate::desktop::linux_title_bar::content_area(window).size;
        let dismiss = self.dismiss.clone();
        let id: SharedString = DISMISS_ID.into();
        let button = div()
            .id(DISMISS_ID)
            .debug_selector(|| DISMISS_ID.into())
            .role(Role::Button)
            .aria_label(DISMISS_NAME)
            // The one part of the notice that takes the pointer: a press on it is not also a press on the terminal.
            .occlude()
            .flex_none()
            .px(px(4.))
            .rounded(metrics::RADIUS_MD)
            .cursor_pointer()
            .hover(move |style| style.bg(theme::color(palette.control_hover)))
            // Nothing in the notice takes focus; GPUI's own focus-on-press has nothing to do here either.
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_click(move |_, window, cx| dismiss(window, cx))
            .child("×");
        let button = with_hover_help(
            button,
            &id,
            &HoverHelp::tooltip(DISMISS_HELP),
            false,
            None,
            window,
            cx,
        );
        div()
            .id(NOTICE_ID)
            .debug_selector(|| NOTICE_ID.into())
            .role(Role::Status)
            .aria_label(self.text.clone())
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.))
            // Capped by the window's content area, the same area the layer manager hands `place`, so a notice that
            // fits here also fits where it is placed.
            .max_w(px(MAX_WIDTH).min(area.width - px(2. * EDGE)))
            .max_h(area.height - px(2. * EDGE))
            .overflow_hidden()
            .px(px(10.))
            .py(px(8.))
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(palette.fg_dim))
            .bg(theme::color(palette.bg_2))
            .border_1()
            .border_color(theme::color(palette.border))
            .rounded(metrics::RADIUS_MD)
            .child(div().min_w_0().child(self.text.clone()))
            .child(button)
    }
}
