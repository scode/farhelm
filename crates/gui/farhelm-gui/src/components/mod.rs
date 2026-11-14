//! The component layer: every control the GUI draws, built so that the old app's coverage rules hold by construction.
//!
//! SPEC_impl.md, "The component layer and theme": a control's constructor takes its label, its hover help and its
//! accessibility role, so a control without them does not compile. The old app checked "every control has hover help"
//! with a browser test that walked the page; here the type does it, and the test bridge reads the same role and label
//! to find controls. Colors come only from theme tokens ([`crate::theme`]), and hover help is drawn by the layer
//! manager ([`crate::layers`]), never by a component itself.
//!
//! Among the controls, GPUI Kit is used only for the text input (Kit's `Input`, with its right-click menu turned off as
//! in the old app's release builds). Buttons and the dialog frame are plain GPUI, which matches the old stylesheet's
//! control tiers exactly and keeps Kit's own focus and overlay machinery out of them. Outside this module Kit appears
//! only where SPEC_impl.md allows it: its `Root` around the window (the bootstrap and the screenshot goldens) and the
//! projection of Farhelm's tokens onto Kit's theme (the theme module), which Kit's input reads.

mod banner;
mod button;
mod dialog;
mod text_input;

#[cfg(test)]
mod tests;

pub use banner::Banner;
pub use button::{Button, IconButton, Tier};
pub use dialog::DialogFrame;
pub use gpui::Role;
pub use text_input::{TextInput, TextInputEvent, TextInputState};

use gpui::{App, ElementId, SharedString, StatefulInteractiveElement, Window};

use crate::layers::{self, TooltipOwner};

/// What a control says when the user hovers it or asks assistive technology about it.
///
/// Every control has one: the layer manager shows it as a tooltip on hover, and it doubles as the control's accessible
/// description. SPEC_impl.md lets a control that already shows its explanation beside itself (a menu item with a
/// description line) count that text as its hover help instead; that form arrives with the first component that draws
/// such a description, so that no control can claim help it never shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoverHelp(SharedString);

impl HoverHelp {
    /// Help shown as a tooltip.
    pub fn tooltip(text: impl Into<SharedString>) -> Self {
        HoverHelp(text.into())
    }
}

/// Give `element`, the control drawn under `id`, its hover help: the tooltip the layer manager shows while the pointer
/// is over it, owned by this control so it closes when the control is gone, and the help as the control's accessible
/// description where the element can carry one.
///
/// Every control wires its help through here, and nothing else can: the help's text is private to this module, so a
/// control that skipped this would never read its own help, and the unused field fails the build (`-D warnings`). That
/// is what keeps a future control from silently dropping its tooltip.
pub(crate) fn with_hover_help<E: StatefulInteractiveElement>(
    element: E,
    id: &SharedString,
    help: &HoverHelp,
    describe: bool,
    window: &mut Window,
    cx: &mut App,
) -> E {
    let owner = TooltipOwner::for_control(
        ElementId::Name(format!("{id}-tooltip-owner").into()),
        window,
        cx,
    );
    let text = help.0.clone();
    let element = if describe {
        element.aria_description(text.clone())
    } else {
        element
    };
    element.on_hover(move |hovered, window, cx| {
        if *hovered {
            layers::show_tooltip(&owner, text.clone(), window, cx);
        } else {
            layers::hide_tooltip(&owner, window, cx);
        }
    })
}
