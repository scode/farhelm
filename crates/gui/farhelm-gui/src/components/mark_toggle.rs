//! A mark toggle: a small mark that means something on its own (a session's status dot) and is also a pointer shortcut
//! for one action, inside a larger control that has its own click (the session row's open control).
//!
//! The old app's status dot was such a mark: a click on it marked the session read or unread without opening the row,
//! and its hover text named the status and then the action. It was a pointer shortcut only, hidden from assistive
//! technology and never focused, because the same action is a real, keyboard-operable item in the row's menu. This is
//! that dot as a control. Here's the TLDR of how it behaves:
//!
//! - **Its press is its own.** A press on it never reaches the control around it: the press stops at the mark, so the
//!   surrounding control neither arms its click (the row does not open) nor applies its focus rule (on Linux the row's
//!   open control would otherwise take focus). The press still counts as the user moving on, and still closes an open
//!   menu or anchored question, because those are watched before any element sees the press. This holds whether or not
//!   the action is available: an unavailable mark (its session busy) is not a dead spot that falls through to opening
//!   the row, it simply does nothing, as the old dot did.
//! - **No focus.** It tracks no focus handle, so it is never a tab stop and a press focuses nothing, like
//!   [`super::OptionRow`]. The keyboard path to the same action belongs to whoever offers it beside the mark (the row's
//!   menu); the mark is not keyboard reachable by design, and must not be used for an action that has no other path.
//! - **Help and name.** Its constructor takes the label, hover help and role every control takes. The help is the
//!   tooltip and accessible description; the label is the accessible name, so a screen reader that lists the row's
//!   contents hears the action rather than an unnamed shape. Where many marks share a list, the label should say which
//!   item each belongs to.
//!
//! Activation is a pointer click only, through the controls' shared click rule ([`super::activate_on_click`]); there is
//! no key rule, since nothing focuses it.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement, MouseButton,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder as _,
};

use super::{Handler, HoverHelp};

/// A mark that toggles something on a click. See the module docs.
#[derive(IntoElement)]
pub struct MarkToggle {
    id: SharedString,
    label: SharedString,
    help: HoverHelp,
    role: Role,
    mark: Option<AnyElement>,
    on_click: Option<Handler>,
}

impl MarkToggle {
    /// A mark toggle. `id` must be stable for the thing the mark is about (a session's key, not its position), so its
    /// tooltip follows it when a list reorders, and it must not share a prefix other code uses to find the surrounding
    /// controls; `label` is its accessible name (the action, "Mark read"); `role` is what assistive technology and the
    /// test bridge see.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
    ) -> Self {
        MarkToggle {
            id: id.into(),
            label: label.into(),
            help,
            role,
            mark: None,
            on_click: None,
        }
    }

    /// What the toggle draws: the mark itself. The toggle is exactly as large as the mark, which is its click target.
    pub fn mark(mut self, mark: impl IntoElement) -> Self {
        self.mark = Some(mark.into_any_element());
        self
    }

    /// What a click on the mark does. Without one (the action is unavailable just now), a click does nothing at all and
    /// still does not reach the control around the mark.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for MarkToggle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let enabled = self.on_click.is_some();
        let toggle = div()
            .id(ElementId::Name(self.id.clone()))
            // Tests find a control by its id; this records nothing outside test builds.
            .debug_selector(|| self.id.to_string())
            .role(self.role)
            .aria_label(self.label.clone())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            // The old `.status-dot-toggle`: the pointer cursor only while there is something to toggle.
            .when(enabled, |toggle| toggle.cursor_pointer())
            // GPUI runs a press's bubble listeners innermost first, and the surrounding control registers both its
            // click (which arms on the press) and its focus rule there, so stopping the press here keeps it from both.
            // The mark's own click armed on this same press before this listener runs (an element's click listeners
            // come after its own mouse-down listeners, and the bubble phase runs them in reverse), so it still fires.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(self.mark);
        let toggle = super::with_hover_help(toggle, &self.id, &self.help, true, window, cx);
        super::activate_on_click(toggle, self.on_click)
    }
}
