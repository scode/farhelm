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
//! The panel is the click's safeguard: a click opens the hyperlink only while the panel is fully visible. It records
//! where it was drawn and the window's size every frame ([`Drawn`]); its owner reads that at the click. It takes no
//! focus and holds nothing that does: it is a non-focusing popover the layer manager draws on top, anchored to a
//! zero-width element the terminal draws by the pointer.

use std::cell::Cell;
use std::rc::Rc;

use farhelm_grid_model::LinkDisclosure;
use gpui::{
    Bounds, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Pixels, Render,
    Size, StatefulInteractiveElement, Styled, Window, canvas, div, prelude::FluentBuilder as _, px,
};

use crate::host_text::{self, HostText};
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

/// The panel's view: one disclosure, as the grid model judged it when the hover started (SPEC.md: the warning is
/// judged then and need not follow later redraws while the pointer rests).
pub(crate) struct DisclosureView {
    host: HostText,
    target: HostText,
    /// The link's own text, when it names somewhere else.
    claimed: Option<HostText>,
    drawn: Drawn,
}

impl DisclosureView {
    pub(crate) fn new(disclosure: &LinkDisclosure, drawn: Drawn) -> Self {
        DisclosureView {
            host: HostText::from_host(disclosure.host.as_str()),
            target: HostText::from_host(disclosure.target.as_str()),
            claimed: disclosure.claimed_text.as_deref().map(HostText::from_host),
            drawn,
        }
    }
}

impl Render for DisclosureView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let mismatch = self.claimed.is_some();
        let drawn = self.drawn.clone();
        // Records the panel's own bounds as it paints, with the window's size, for the click's check.
        let recorder = canvas(
            |_, _, _| (),
            move |bounds, (), window, _| drawn.set(Some((bounds, window.viewport_size()))),
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        // The old panel's look (`.terminal-link-target` in app.css): a floating surface in the interface face at 12
        // pixels, at most the window's width less 8 pixels, wrapping anywhere; the loud form danger-filled at 14
        // pixels with a 2-pixel edge.
        let panel = div()
            .id("terminal-link-disclosure")
            .debug_selector(|| "terminal-link-disclosure".into())
            .role(gpui::Role::Tooltip)
            .relative()
            .max_w(window.viewport_size().width - px(8.))
            .rounded(metrics::RADIUS_MD)
            .shadow(crate::layers::menu_shadow(palette))
            .when(!mismatch, |panel| {
                panel
                    .px(px(6.))
                    .py(px(4.))
                    .bg(theme::color(palette.bg_2))
                    .text_color(theme::color(palette.fg_1))
                    .text_size(metrics::TEXT_SM)
            })
            .when(mismatch, |panel| {
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
            .child(recorder)
    }
}
