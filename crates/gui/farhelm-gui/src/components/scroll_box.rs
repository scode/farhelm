//! The scroll box, and the scroll stop it shares with the dialog frame: a region the keyboard reaches with Tab and
//! scrolls with the arrows, Page Up and Page Down, Home, End and Space, because nothing inside it takes focus.
//!
//! SPEC.md, "Reaching everything by keyboard": "A region whose content holds nothing focusable gets its own tab stop
//! with an accessible name." GPUI scrolls nothing from the keyboard on its own, and a scrolling container takes no
//! focus, so such a region needs a stop drawn for it. Two kinds of region need one, and both get it from
//! [`scroll_stop`](super::scroll_stop), so their keys, ring, role and Tab reach cannot drift apart. They differ only in
//! when the stop exists:
//!
//! - [`ScrollBox`], a bounded box of text inside a card (an agent approval's command or prompt), has its stop whenever
//!   it is drawn, short content included. It is the old app's `PeerBlock` given a `scroll_label` (`peer.rs` at
//!   reference commit `24bc0580`): a `pre` capped by CSS with `tabindex="0"`, `role="region"` and the label as its
//!   name, which the old box had whenever it was labelled. The old box without a label was plain text with no stop;
//!   here that is host text drawn without this control, so the control always has the label the component rules
//!   require.
//! - A dialog's scrolling body ([`DialogFrame::scroll_stop`](super::DialogFrame::scroll_stop)) has its stop whenever it
//!   holds no tab stop, overflowing or not, as the old `ScrollRegion` (`modal_dialog.rs` at the same commit) did: a
//!   button counts whatever Tab reaches on macOS, as there (SPEC.md: "button-only regions need no extra stop"). A body
//!   with a control is reached through that control, and the focus coordinator scrolls it into view. Whether the body
//!   holds one is known only after layout, so the focus coordinator's scroll region reports it
//!   ([`focus::ScrollRegion::report`](crate::focus::ScrollRegion::report)). A dialog's notices region gets the same
//!   stop under the same rule ([`DialogFrame::notices_stop`](super::DialogFrame::notices_stop)), as the old Trash
//!   dialog's global confirmation was that same `ScrollRegion`. Because that region holds a question's answers, it
//!   also takes Page Up, Page Down, Home and End from a control inside it
//!   ([`scroll_from_controls`](super::scroll_from_controls)), which the stop never does.
//!
//! What both share, through [`scroll_stop`](super::scroll_stop), which takes its Tab reach, ring and role from
//! [`focus_stop`](super::focus_stop), the stop without the keys that a place where focus rests (a session pane's
//! interrupted card) uses alone:
//!
//! - **The keys**, as the old app's browser scrolled a focused region: Up and Down by a line, Page Up and Page Down and
//!   Space by a page, Home and End to either end. A line is 40 pixels and a page seven eighths of the visible height,
//!   WebKit's steps (`Scrollbar::pixelsPerLineStep` and `minFractionToStepWhenPaging`), since the old app ran in
//!   WebKit. A held key keeps scrolling, as there. A key handled here stops at the stop, so no key listener around it
//!   sees it; bindings never do, since GPUI runs every matching binding on the focused path before any key listener.
//! - **Tab reach.** A stop counts as a text field for the focus coordinator's Tab reach (`focus::OnPress::Focus`), so
//!   plain Tab reaches it on macOS even with the system's Keyboard navigation setting off. In WebKit that setting
//!   decides only whether Tab stops on buttons and the like; an element made focusable with `tabindex="0"`, as both old
//!   regions were, stayed in plain Tab's order, and SPEC.md exempts only regions of buttons on macOS ("button-only
//!   regions need no extra stop"). A press of any mouse button on the region focuses it, as a press on such an element
//!   did in WebKit.
//! - **The ring**: the accent's two pixels inside the stop's edge, the old `outline-offset: -2px` both regions had, so
//!   whatever clips the stop (the dialog's card) cannot cut it off. A dialog's notices sit inside the footer's
//!   padding, so their ring reaches out to the card's sides, where the old full-width region's ring lay, and two
//!   pixels above and below the notices.
//! - **Its shape.** The stop is not the scrolling element but a box around it. The ring and the focus coordinator's
//!   position record are drawn as children of the stop, and a child of the scrolling element would move with its
//!   content, add to its scroll extent and, for the record, make a Tab onto the stop scroll the region back to its top.
//!   For the dialog's body the record must also sit outside the region for a second reason: inside, it would count as a
//!   control the body holds, and the stop would switch itself off on the next frame.
//!
//! NOTE: Browsers paged up on Shift+Space. Modified keys are left to whatever holds the stop here, the narrower port.

use gpui::{
    AnyElement, App, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce,
    Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use super::{HoverHelp, Ring, scroll_handle, scroll_stop_on};
use crate::focus;
use crate::theme::{self, metrics};

/// `.approval-card-block`: the old box's padding, vertical then horizontal.
const PADDING: (Pixels, Pixels) = (px(6.), px(8.));

/// `.approval-card-block`: the old box's line height, as a multiple of its text size.
const LINE_HEIGHT: f32 = 1.6;

/// A bounded, read-only box of content that is a tab stop of its own. See the module docs.
///
/// It looks like the old approval card's well (`.approval-card-block`): the well fill, the row border, the small
/// radius, six and eight pixels of padding and a line height of 1.6. How tall it may grow and how short it may shrink
/// are its owner's ([`ScrollBox::min_height`], [`ScrollBox::max_height`]); the old card set them in lines of its own
/// text. So is the face ([`ScrollBox::monospace`]).
#[derive(IntoElement)]
pub struct ScrollBox {
    id: SharedString,
    label: SharedString,
    help: HoverHelp,
    role: Role,
    content: AnyElement,
    min_height: Option<Pixels>,
    max_height: Option<Pixels>,
    monospace: bool,
}

impl ScrollBox {
    /// A box around `content`, labelled `label` (its accessible name, such as "Initial prompt") with hover help `help`.
    /// `role` is what assistive technology and the test bridge see: [`Role::Region`], as the old box's `role="region"`.
    /// Content holding host values draws them through the host-text element, as everywhere.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
        content: impl IntoElement,
    ) -> Self {
        ScrollBox {
            id: id.into(),
            label: label.into(),
            help,
            role,
            content: content.into_any_element(),
            min_height: None,
            max_height: None,
            monospace: false,
        }
    }

    /// The least height the box keeps, border included, however short its content.
    pub fn min_height(mut self, height: Pixels) -> Self {
        self.min_height = Some(height);
        self
    }

    /// The most height the box takes, border included; taller content scrolls inside it.
    pub fn max_height(mut self, height: Pixels) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Draw the content in the monospace face, as the old box (a `pre`) drew a command, its resume command and its
    /// wrapper words. The face is the caller's because the old prompt box alone kept the interface face
    /// (`.approval-card-prompt`: "Preformatted prompt prose ... keep the interface face"), as [`TextInput::monospace`]
    /// is the field's.
    ///
    /// [`TextInput::monospace`]: super::TextInput::monospace
    pub fn monospace(mut self) -> Self {
        self.monospace = true;
        self
    }
}

impl RenderOnce for ScrollBox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let scroll = scroll_handle(format!("{}-scroll", self.id), window, cx);
        let scroller = div()
            .id(ElementId::Name(format!("{}-scroller", self.id).into()))
            .debug_selector(|| format!("{}-scroller", self.id))
            // Fills the box between its bounds, and gives up height to the box's cap rather than overflowing it.
            .flex_grow(1.)
            .flex_shrink_1()
            .min_h_0()
            .overflow_y_scroll()
            .py(PADDING.0)
            .px(PADDING.1)
            .child(self.content);
        let stop = div()
            .id(ElementId::Name(self.id.clone()))
            .debug_selector(|| self.id.to_string())
            .flex()
            .flex_col()
            .min_w_0()
            .when_some(self.min_height, |stop, height| stop.min_h(height))
            .when_some(self.max_height, |stop, height| stop.max_h(height))
            .line_height(relative(LINE_HEIGHT))
            .when(self.monospace, |stop| {
                stop.font_family(crate::assets::TERMINAL_FONT)
            })
            .text_color(theme::color(palette.fg_0))
            .bg(theme::color(palette.well))
            .border_1()
            .border_color(theme::color(palette.border_row))
            .rounded(metrics::RADIUS_SM)
            .child(focus::scroll_region(scroller, &scroll));
        // The stop's handle, made here so its help shows on keyboard focus too, and handed to the stop.
        let handle = focus::control_handle(
            ElementId::Name(format!("{}-focus", self.id).into()),
            window,
            cx,
        );
        let stop =
            super::with_hover_help(stop, &self.id, &self.help, true, Some(&handle), window, cx);
        // The ring covers the one-pixel border and one pixel inside it, as the old outline two pixels in did.
        let ring = Ring {
            inset_x: px(-1.),
            inset_y: px(-1.),
            radius: metrics::RADIUS_SM,
        };
        scroll_stop_on(
            stop, &self.id, &handle, &scroll, self.role, self.label, ring, window, cx,
        )
    }
}
