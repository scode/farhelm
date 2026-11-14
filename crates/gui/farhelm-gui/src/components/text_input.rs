//! The single-line text input, over GPUI Kit's `Input`.
//!
//! Kit supplies the hard part (editing, selection, undo, input methods), which is why SPEC_impl.md keeps Kit for text
//! inputs. Two things are Farhelm's: the label and hover help every control must have, and Kit's built-in right-click
//! menu, which is turned off because the old app's release builds had no right-click menu on text (browser-behavior
//! inventory, entry B1). The only way to get an input's state is [`TextInputState::new`], so no input can skip that.

use gpui::{
    App, AppContext as _, ElementId, Entity, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
};
use gpui_component::input::{Input, InputState};

use super::HoverHelp;
use crate::layers::{self, TooltipOwner};
use crate::theme::{self, metrics};

/// The editable state behind a [`TextInput`]: the text, the selection and the undo history, owned by the view that
/// shows the input for as long as it shows it.
#[derive(Clone)]
pub struct TextInputState(Entity<InputState>);

impl TextInputState {
    /// A new, empty input state with Kit's right-click menu turned off.
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        TextInputState(cx.new(|cx| InputState::new(window, cx).context_menu(false)))
    }

    /// The current text.
    pub fn text(&self, cx: &App) -> SharedString {
        self.0.read(cx).value()
    }

    /// Replace the whole text, as when a dialog opens prefilled with the current value. Not an edit: it joins no undo
    /// history and raises no change event.
    pub fn set_text(&self, text: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
        let text: SharedString = text.into();
        self.0
            .update(cx, |state, cx| state.set_value(text, window, cx));
    }
}

/// A labeled single-line text input. Its accessible role is Kit's text-input role, the one role an editable line has,
/// so the constructor takes no role.
///
/// NOTE: Kit's `Input` takes an accessible name but no accessible description, so the hover help reaches assistive
/// technology only as the tooltip, unlike a button's. That is a gap in Kit, recorded rather than worked around.
#[derive(IntoElement)]
pub struct TextInput {
    id: SharedString,
    state: TextInputState,
    label: SharedString,
    help: HoverHelp,
}

impl TextInput {
    /// An input showing `state`. `label` is its accessible name; `help` its hover help.
    pub fn new(
        id: impl Into<SharedString>,
        state: &TextInputState,
        label: impl Into<SharedString>,
        help: HoverHelp,
    ) -> Self {
        TextInput {
            id: id.into(),
            state: state.clone(),
            label: label.into(),
            help,
        }
    }
}

impl RenderOnce for TextInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let owner = TooltipOwner::for_control(
            ElementId::Name(format!("{}-tooltip-owner", self.id).into()),
            window,
            cx,
        );
        let tooltip = self.help.text().clone();
        div()
            .id(ElementId::Name(self.id.clone()))
            .w_full()
            .on_hover(move |hovered, window, cx| {
                if *hovered {
                    layers::show_tooltip(&owner, tooltip.clone(), window, cx);
                } else {
                    layers::hide_tooltip(&owner, cx);
                }
            })
            .child(
                // Kit fills a dark-mode input with its border color at 30%, not the old stylesheet's recessed `--well`,
                // and rounds it at its general radius; these override that with the old input's fill, corner and type
                // size. Kit's padding and height stay (a few pixels taller than the old 4-by-8 padding).
                Input::new(&self.state.0)
                    .aria_label(self.label)
                    .bg(theme::color(palette.well))
                    .rounded(metrics::RADIUS_SM)
                    .text_size(metrics::TEXT_MD),
            )
    }
}
