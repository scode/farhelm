//! A program's hyperlink's destination display: the panel next to the pointer that names where a terminal link
//! really goes before a click opens it (SPEC.md's terminal links paragraph; the old app's `.terminal-link-target`,
//! `showLinkTarget` in terminal-links.js at the reference commit).
//!
//! A hyperlink a program emits (OSC 8) can underline text that differs from where it goes, so hovering it shows the
//! exact target first, with its host emphasized. When the underlined text gives a reason to doubt the link (it mentions
//! a web address that names a different place, spells one with look-alike characters or login details, couldn't be
//! read whole, or hides part of itself), the panel becomes a loud warning: one line per reason, then the address or
//! text that prompted it, then the real target. The grid model decides what to show
//! ([`farhelm_grid_model::LinkDisclosure`]); this draws it, every piece of program text through the host-text element
//! so invisible and direction-changing characters are visible.
//!
//! A printed web address gets the panel too when, on a Mac, it points at a remote session's own loopback: the panel
//! then ends with the tunnel hint, which says that a plain click opens the address on this Mac and Cmd-click through a
//! tunnel ([`super::tunnels`]). A hyperlink's panel keeps its warning and adds the hint after it.
//!
//! The panel is the click's safeguard: a click opens the hyperlink only while the panel is fully visible. It records
//! its hitbox and the window's content area every frame ([`Drawn`]); its owner reads that at the click. It
//! takes no focus and holds nothing that does: it is a non-focusing popover the layer manager draws on top, anchored to
//! a zero-width element the terminal draws by the pointer.

use std::cell::Cell;
use std::rc::Rc;

use farhelm_grid_model::{LinkDisclosure, WarningReason};
use gpui::{
    App, Bounds, Context, Div, FontWeight, HitboxBehavior, HitboxId, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, Stateful, StatefulInteractiveElement, Styled,
    Window, canvas, div, prelude::FluentBuilder as _, px,
};

use crate::host_text::{self, HostSentence, HostText};
use crate::theme::{self, metrics};

/// The line a warning shows for one reason. The wording is fixed by SPEC.md's terminal links paragraph; each says what
/// is wrong with the text in terms a user can check against it, not how the check works.
const fn reason_line(reason: WarningReason) -> &'static str {
    match reason {
        WarningReason::Mismatch => {
            "The link text mentions an address that differs from where it goes"
        }
        WarningReason::Disguised => {
            "The address in the link text uses look-alike or invisible characters"
        }
        WarningReason::Credentials => {
            "The address in the link text puts login details before the real site"
        }
        WarningReason::Incomplete => "The whole link text couldn't be checked",
        WarningReason::Hidden => "Part of the link text is hard to see",
    }
}

/// What the panel recorded at paint. An unanswered file check can invalidate this record before the answer draws.
pub(crate) type Drawn = Rc<Cell<Option<DrawnPanel>>>;

/// The panel's geometry and its place in the frame's hitbox order. The hitbox lets the layer manager check that
/// the panel still belongs to the rendered frame before it relies on any overlap result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DrawnPanel {
    pub(super) bounds: Bounds<Pixels>,
    pub(super) area: Bounds<Pixels>,
    hitbox: HitboxId,
}

/// What blocked the click. An absent or clipped panel has the old "not visible" explanation, even when a cover
/// also overlaps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hidden {
    NotVisible,
    Covered,
}

impl Hidden {
    /// The old app's `admitDisclosure` wording for a web or tunnel open.
    pub(super) const fn link_words(self) -> &'static str {
        match self {
            Self::NotVisible => {
                "cannot open: full link details are not visible; enlarge the window and hover again"
            }
            Self::Covered => {
                "cannot open: full link details are covered by another Farhelm surface; move the pointer and hover again"
            }
        }
    }

    /// The same refusal with the old app's download wording.
    pub(super) const fn file_words(self) -> &'static str {
        match self {
            Self::NotVisible => {
                "cannot download: full file details are not visible; enlarge the window and hover again"
            }
            Self::Covered => {
                "cannot download: full file details are covered by another Farhelm surface; move the pointer and hover again"
            }
        }
    }
}

/// The disclosure must fit inside the window's content area and remain uncovered in the last rendered frame.
/// Checking the hitbox itself keeps a retained record from an earlier frame from admitting a click.
pub(super) fn visibility(drawn: &Drawn, window: &Window) -> Result<(), Hidden> {
    let panel = drawn.get().ok_or(Hidden::NotVisible)?;
    if !fits(panel.bounds, panel.area) {
        return Err(Hidden::NotVisible);
    }
    match crate::layers::pointer_covers(panel.hitbox, window) {
        Some(false) => Ok(()),
        Some(true) => Err(Hidden::Covered),
        None => Err(Hidden::NotVisible),
    }
}

/// The content area omits Linux's client-side frame and title bar. Touching its edges is fine.
fn fits(bounds: Bounds<Pixels>, area: Bounds<Pixels>) -> bool {
    bounds.size.width > px(0.)
        && bounds.size.height > px(0.)
        && bounds.left() >= area.left()
        && bounds.top() >= area.top()
        && bounds.right() <= area.right()
        && bounds.bottom() <= area.bottom()
}

/// The floating surface every destination display is drawn on, under element id `id` (the old app's
/// `.terminal-link-target`): the interface face at 12 pixels on the floating ground, at most the width of the window's
/// content area ([`crate::desktop::linux_title_bar::content_area`], the old page) less 8 pixels, since the layer
/// manager keeps every popover inside that area. A web link's warning and a file link's stripe are drawn over it by
/// their own views.
pub(super) fn surface(id: &'static str, window: &Window, cx: &App) -> Stateful<Div> {
    let palette = theme::palette(cx);
    div()
        .id(id)
        .debug_selector(move || id.into())
        .role(gpui::Role::Tooltip)
        .relative()
        .max_w(
            crate::desktop::linux_title_bar::content_area(window)
                .size
                .width
                - px(8.),
        )
        .rounded(metrics::RADIUS_MD)
        .shadow(crate::layers::menu_shadow(palette))
        .px(px(6.))
        .py(px(4.))
        .bg(theme::color(palette.bg_2))
        .text_color(theme::color(palette.fg_1))
        .text_size(metrics::TEXT_SM)
}

/// Witness the whole card, borders included. An absolute child of the card itself would start inside the border.
/// The parent layer prepaints its blocker first, so the transparent probe only sees later blockers.
pub(super) fn witnessed(panel: impl IntoElement, drawn: Drawn) -> impl IntoElement {
    let recorder = canvas(
        |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal).id,
        move |bounds, hitbox, window, _| {
            drawn.set(Some(DrawnPanel {
                bounds,
                area: crate::desktop::linux_title_bar::content_area(window),
                hitbox,
            }))
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    div().relative().child(panel).child(recorder)
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
    message_frame(cx)
        .id(id)
        .debug_selector(move || id.into())
        .role(crate::components::Role::Status)
        .aria_label(label)
        .child(content)
        .into_any_element()
}

/// Shared by the visible status and the layout placeholder for a status drawn by the layer manager. The placeholder
/// omits the status role and label so assistive technology sees only the live line.
pub(super) fn message_frame(cx: &App) -> Div {
    let palette = theme::palette(cx);
    div()
        .px(px(6.))
        .py(px(4.))
        .max_h(px(54.))
        .overflow_hidden()
        .bg(theme::color(palette.bg_2))
        .text_color(theme::color(palette.fg_1))
        .text_size(metrics::TEXT_SM)
}

/// The panel's view: one disclosure, as the grid model judged it when the hover started (SPEC.md: the warning is
/// judged then and need not follow later redraws while the pointer rests).
pub(crate) struct DisclosureView {
    host: HostText,
    target: HostText,
    /// The loud form, when the grid model found a reason to doubt the link.
    warning: Option<Warning>,
    /// The tunnel offer for a remote host's loopback link, drawn last, after the warning if there is one, which it
    /// never replaces (SPEC.md: the hover "preserves any OSC hyperlink mismatch warning").
    hint: Option<HostSentence>,
    drawn: Drawn,
}

/// What a warning panel shows above the destination.
struct Warning {
    /// One line per reason, in the grid model's order.
    reasons: Vec<&'static str>,
    /// The program text the user can check the reasons against, under its heading: the first address that doesn't
    /// match ("Text mentions:"), or else the visible label ("Link text:", marked partial when it couldn't be read
    /// whole). `None` for a label too long to read, which has no text to show.
    text: Option<(&'static str, HostText)>,
}

impl DisclosureView {
    pub(crate) fn new(
        disclosure: &LinkDisclosure,
        hint: Option<HostSentence>,
        drawn: Drawn,
    ) -> Self {
        let warning = disclosure.warning.as_ref().map(|warning| {
            let text = match &warning.address {
                Some(address) => Some(("Text mentions:", address.as_str())),
                None if warning.label.is_empty() => None,
                None if warning.reasons.contains(&WarningReason::Incomplete) => {
                    Some(("Link text (partial):", warning.label.as_str()))
                }
                None => Some(("Link text:", warning.label.as_str())),
            };
            Warning {
                reasons: warning
                    .reasons
                    .iter()
                    .map(|&reason| reason_line(reason))
                    .collect(),
                text: text.map(|(heading, text)| (heading, HostText::from_host(text))),
            }
        });
        DisclosureView {
            host: HostText::from_host(disclosure.host.as_str()),
            target: HostText::from_host(disclosure.target.as_str()),
            warning,
            hint,
            drawn,
        }
    }
}

impl Render for DisclosureView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let mismatch = self.warning.is_some();
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
        // The reasons, each on its own line, bold.
        let reasons = self.warning.as_ref().map(|warning| {
            div()
                .debug_selector(|| "terminal-link-disclosure-warning".into())
                .mb(px(4.))
                .font_weight(FontWeight::BOLD)
                .children(warning.reasons.iter().map(|&reason| {
                    div()
                        .debug_selector(|| "terminal-link-disclosure-reason".into())
                        .child(reason)
                }))
        });
        // The program's text on its own line under its heading, as a block: in a wrapping flex row it kept its whole
        // width, so a long one ran past the panel and the window's edge instead of wrapping inside the panel.
        let claimed = self
            .warning
            .as_ref()
            .and_then(|warning| warning.text.as_ref())
            .map(|(heading, text)| {
                div().child(*heading).child(
                    host_text::element(text)
                        .debug_selector(|| "terminal-link-disclosure-claimed".into()),
                )
            });
        let panel = panel
            .children(reasons)
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
            }));
        witnessed(panel, self.drawn.clone())
    }
}

#[cfg(test)]
mod tests;
