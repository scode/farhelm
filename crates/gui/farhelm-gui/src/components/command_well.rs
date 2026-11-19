//! The command well: a command the helm composed, in a monospace well that wraps it, with Copy beside it.
//!
//! It keeps the old `.tensorlake-command` layout (`app.css` at reference commit `24bc0580`). Settings owns the
//! line confirming a copy, since it also shows the current state of the key. The well tells it through `on_copy`.

use gpui::{
    App, FocusHandle, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _,
    Window, div, prelude::FluentBuilder as _, px,
};

use super::{Button, HoverHelp, copy_button_then};
use crate::host_text::{HostSentence, HostText};
use crate::theme::{self, metrics};

/// The command's text size (`pre { font-size: 11px }`); its line height is 1.5 times this, as the old dialogs' was.
pub const COMMAND_TEXT: f32 = 11.;
/// The well's padding on every side (0.75rem).
const PAD: f32 = 12.;
/// The gap between the well and Copy.
const GAP: f32 = 6.;

/// `command` in its well, drawn as host text under `id`, with Copy under `{id}-copy`. Copy writes the command's raw
/// bytes ([`HostText::raw`]), so a pasted command addresses the same destination and path as the helm composed even
/// where the screen shows an escape. Copy's label, hover help and role (a push button) are the control's.
#[derive(IntoElement)]
pub struct CommandWell {
    id: SharedString,
    command: HostText,
    copy_label: SharedString,
    help: HoverHelp,
    accessible_name: Option<SharedString>,
    copied: Box<dyn Fn(&mut App)>,
    focus: Option<FocusHandle>,
}

impl CommandWell {
    /// `command` in its well under `id`, with a Copy labelled `copy_label` whose hover help is `help`.
    pub fn new(
        id: impl Into<SharedString>,
        command: HostText,
        copy_label: impl Into<SharedString>,
        help: HoverHelp,
    ) -> Self {
        CommandWell {
            id: id.into(),
            command,
            copy_label: copy_label.into(),
            help,
            accessible_name: None,
            copied: Box::new(|_| {}),
            focus: None,
        }
    }

    /// Copy's accessible name, where its label alone does not say what it copies ("Copy sessions connection").
    pub fn accessible_name(mut self, name: impl Into<SharedString>) -> Self {
        self.accessible_name = Some(name.into());
        self
    }

    /// Run `copied` after each copy, once the command is on the clipboard ([`super::copy_button_then`]).
    pub fn on_copy(mut self, copied: impl Fn(&mut App) + 'static) -> Self {
        self.copied = Box::new(copied);
        self
    }

    /// Copy's focus handle, for an owner that moves focus to Copy.
    pub fn focus(mut self, handle: &FocusHandle) -> Self {
        self.focus = Some(handle.clone());
        self
    }
}

impl RenderOnce for CommandWell {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        // `flex_1` with `min_w_0` is the old grid's `minmax(0, 1fr)`: the well takes what Copy leaves and may shrink
        // below the command's unwrapped width, which makes the command wrap instead of pushing Copy out.
        let well = div()
            .flex_1()
            .min_w_0()
            .p(px(PAD))
            .bg(theme::color(palette.bg_1))
            .border_1()
            .border_color(theme::color(palette.border))
            .rounded(metrics::RADIUS_SM)
            .text_size(px(COMMAND_TEXT))
            .line_height(px(COMMAND_TEXT * 1.5))
            .child(HostSentence::new(self.id.clone()).mono(&self.command));
        let copy = copy_button_then(
            format!("{}-copy", self.id),
            self.copy_label,
            self.help,
            self.command.raw().to_owned(),
            self.copied,
        )
        .when_some(self.accessible_name, Button::accessible_name)
        .when_some(self.focus.as_ref(), Button::focus);
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(GAP))
            .child(well)
            .child(copy)
    }
}
