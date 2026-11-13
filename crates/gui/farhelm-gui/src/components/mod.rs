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

mod button;
mod dialog;
mod text_input;

#[cfg(test)]
mod tests;

pub use button::{Button, IconButton, Tier};
pub use dialog::DialogFrame;
pub use gpui::Role;
pub use text_input::{TextInput, TextInputState};

use gpui::SharedString;

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

    /// The text, for the tooltip and the accessible description.
    pub fn text(&self) -> &SharedString {
        &self.0
    }
}
