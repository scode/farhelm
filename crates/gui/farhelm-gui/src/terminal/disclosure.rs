//! A program's hyperlink's destination display: the panel next to the pointer that names where a terminal link
//! really goes before a click opens it (SPEC.md's terminal links paragraph; the old app's `.terminal-link-target`,
//! `showLinkTarget` in terminal-links.js at the reference commit).
//!
//! A hyperlink a program emits (OSC 8) can underline text that differs from where it goes, so hovering it shows the
//! exact target first, with its host emphasized; when the underlined text is itself a web address that names a
//! different place, the panel becomes a loud warning that shows that text beside the real target. The grid model
//! decides what to show ([`farhelm_grid_model::LinkDisclosure`]); this draws it, every piece of program text through
//! the host-text element so invisible and direction-changing characters are visible.
//!
//! A printed web address gets the panel too when, on a Mac, it points at a remote session's own loopback: the panel
//! then ends with the tunnel hint, which says that a plain click opens the address on this Mac and Cmd-click through a
//! tunnel ([`super::tunnels`]). A hyperlink's panel keeps its warning and adds the hint after it.
//!
//! The panel is the click's safeguard: a click opens the hyperlink only while the panel is fully visible. It records
//! where it was drawn and the window's size every frame ([`Drawn`]); its owner reads that at the click. It takes no
//! focus and holds nothing that does: it is a non-focusing popover the layer manager draws on top, anchored to a
//! zero-width element the terminal draws by the pointer.

use std::cell::Cell;
use std::rc::Rc;

use farhelm_grid_model::LinkDisclosure;
use gpui::{
    App, Bounds, Context, Div, FontWeight, InteractiveElement, IntoElement, ParentElement, Pixels,
    Render, Size, Stateful, StatefulInteractiveElement, Styled, Window, canvas, div,
    prelude::FluentBuilder as _, px,
};

use crate::host_text::{self, HostSentence, HostText};
use crate::theme::{self, metrics};

/// The warning's first line (the old app's words).
const MISMATCH_WARNING: &str = "Link text does not match where it goes";

/// Where the panel was last drawn and how large the window was then: what a click checks the panel against. `None`
/// until its first frame.
pub(crate) type Drawn = Rc<Cell<Option<(Bounds<Pixels>, Size<Pixels>)>>>;

/// Whether a panel drawn at `drawn` was fully visible: inside the window it was drawn in (the old app's
/// `disclosureFits`). Its other half, refusing when another surface is drawn over the panel, is not built: a dialog or
/// menu opened above the panel is not checked for.
pub(crate) fn fully_visible(drawn: &Drawn) -> bool {
    let Some((bounds, window)) = drawn.get() else {
        return false;
    };
    bounds.size.width > px(0.)
        && bounds.size.height > px(0.)
        && bounds.origin.x >= px(0.)
        && bounds.origin.y >= px(0.)
        && bounds.origin.x + bounds.size.width <= window.width
        && bounds.origin.y + bounds.size.height <= window.height
}

/// The floating surface every destination display is drawn on, under element id `id` (the old app's
/// `.terminal-link-target`): the interface face at 12 pixels on the floating ground, at most the window's width less 8
/// pixels. A web link's warning and a file link's stripe are drawn over it by their own views.
pub(super) fn surface(id: &'static str, window: &Window, cx: &App) -> Stateful<Div> {
    let palette = theme::palette(cx);
    div()
        .id(id)
        .debug_selector(move || id.into())
        .role(gpui::Role::Tooltip)
        .relative()
        .max_w(window.viewport_size().width - px(8.))
        .rounded(metrics::RADIUS_MD)
        .shadow(crate::layers::menu_shadow(palette))
        .px(px(6.))
        .py(px(4.))
        .bg(theme::color(palette.bg_2))
        .text_color(theme::color(palette.fg_1))
        .text_size(metrics::TEXT_SM)
}

/// Records, as the display holding it paints, the display's bounds and the window's size into `drawn`: what a click
/// checks the display against ([`fully_visible`]). It fills the display and draws nothing.
pub(super) fn recorder(drawn: Drawn) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| drawn.set(Some((bounds, window.viewport_size()))),
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// The look the terminal's one message line has, whatever it says (the old `.terminal-file-message`): a status line
/// over the top of the grid's area, clear of the newest output at the bottom, at most three lines tall, under element
/// id `id` and named `label`. The owner's refused-click line and the download status line both draw through it, which
/// is why it lives here, with what web links and file links share, rather than with either.
pub(crate) fn message_line(
    id: &'static str,
    label: gpui::SharedString,
    content: impl IntoElement,
    cx: &App,
) -> gpui::AnyElement {
    let palette = theme::palette(cx);
    div()
        .id(id)
        .debug_selector(move || id.into())
        .role(crate::components::Role::Status)
        .aria_label(label)
        .px(px(6.))
        .py(px(4.))
        .max_h(px(54.))
        .overflow_hidden()
        .bg(theme::color(palette.bg_2))
        .text_color(theme::color(palette.fg_1))
        .text_size(metrics::TEXT_SM)
        .child(content)
        .into_any_element()
}

/// The panel's view: one disclosure, as the grid model judged it when the hover started (SPEC.md: the warning is
/// judged then and need not follow later redraws while the pointer rests).
pub(crate) struct DisclosureView {
    host: HostText,
    target: HostText,
    /// The link's own text, when it names somewhere else.
    claimed: Option<HostText>,
    /// The tunnel offer for a remote host's loopback link, drawn last, after the warning if there is one, which it
    /// never replaces (SPEC.md: the hover "preserves any OSC hyperlink mismatch warning").
    hint: Option<HostSentence>,
    drawn: Drawn,
}

impl DisclosureView {
    pub(crate) fn new(
        disclosure: &LinkDisclosure,
        hint: Option<HostSentence>,
        drawn: Drawn,
    ) -> Self {
        DisclosureView {
            host: HostText::from_host(disclosure.host.as_str()),
            target: HostText::from_host(disclosure.target.as_str()),
            claimed: disclosure.claimed_text.as_deref().map(HostText::from_host),
            hint,
            drawn,
        }
    }
}

impl Render for DisclosureView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let mismatch = self.claimed.is_some();
        // The old panel's look (`.terminal-link-target` in app.css, see [`surface`]); the loud form danger-filled at
        // 14 pixels with a 2-pixel edge.
        let panel = surface("terminal-link-disclosure", window, cx).when(mismatch, |panel| {
            panel
                .px(px(10.))
                .py(px(8.))
                .bg(theme::color(palette.danger_bg))
                .text_color(theme::color(palette.on_fill))
                .border_2()
                .border_color(theme::color(palette.danger_strong))
                .text_size(px(14.))
        });
        let strong = if mismatch {
            palette.on_fill
        } else {
            palette.fg_bright
        };
        let claimed = self.claimed.as_ref().map(|claimed| {
            div()
                .flex()
                .flex_wrap()
                .gap(px(4.))
                .child("Text says:")
                .child(host_text::element(claimed))
        });
        panel
            .when(mismatch, |panel| {
                panel.child(
                    div()
                        .debug_selector(|| "terminal-link-disclosure-warning".into())
                        .mb(px(4.))
                        .font_weight(FontWeight::BOLD)
                        .child(MISMATCH_WARNING),
                )
            })
            .children(claimed)
            .when(mismatch, |panel| panel.child("Actually goes to:"))
            .child(
                host_text::element(&self.host)
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::color(strong)),
            )
            .child(host_text::element(&self.target))
            .children(self.hint.clone().map(|hint| {
                div()
                    .debug_selector(|| super::tunnels::HINT_ID.into())
                    .mt(px(4.))
                    .child(hint)
            }))
            .child(recorder(self.drawn.clone()))
    }
}
