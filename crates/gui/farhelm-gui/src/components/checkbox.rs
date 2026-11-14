//! The checkbox and the toggle: one on/off control, drawn either as the old app's native checkbox or as its switch.
//!
//! The old app had two looks for one control. Its plain checkboxes (the launcher's Resume, say) were the platform's
//! `<input type="checkbox">`, tinted with the accent (`accent-color: var(--accent-fill)` on the page), and its
//! Settings switches were the same input restyled (`.control-switch` in `app.css` at reference commit `24bc0580`:
//! "These remain native checkboxes: labels, Space activation, disabled writes ... all use the existing input and focus
//! lifecycle"). So this is one control with a [`Checkbox::switch`] look, not two controls that could drift apart.
//!
//! What it keeps from the native input:
//!
//! - **Space toggles; Enter does not.** Enter on a checkbox in a form submits the form rather than toggling, so here it
//!   goes on to whatever holds the control (a dialog's Enter). A held Space does not toggle again. An owner can give
//!   it the dialog's main action instead ([`Checkbox::on_enter`]): Enter then runs that action with the box as shown,
//!   unchanged, under the choice controls' shared Enter rule ([`super::enter_choice`]). That is what the old
//!   launcher's Resume checkbox did (its `enter_choice` applied nothing, then launched), and SPEC.md's launcher says
//!   so: "Enter on the ... Resume checkbox launches with the values shown, without changing them".
//! - **The label is part of the control.** The old markup wrapped the input in its `<label>`, so a click on the words
//!   toggles too, and the words are drawn beside the box (after the box for a checkbox, before the switch for a
//!   switch, as the old Settings rows put them, and after a [`Checkbox::inline`] switch, as the old Hosts heading
//!   put its Details control).
//! - **Focus follows the platform's control rule** ([`crate::focus::OnPress::Control`]): on macOS with Keyboard
//!   navigation off, Tab skips it and a click does not focus it, as WebKit treated a checkbox (SPEC.md, "Keyboard
//!   focus"); on Linux Tab reaches it.
//! - **A disabled one ignores everything** and is drawn at half opacity, as `.control-switch:disabled` was.
//!
//! It owns no state: its owner says whether it is checked and hears each toggle as the new value, so a refused save
//! can put it back by drawing the stored value again.

use std::rc::Rc;

use gpui::{
    App, ElementId, FocusHandle, InteractiveElement, IntoElement, MouseButton, ParentElement,
    RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Toggled, Window, div,
    prelude::FluentBuilder as _, px, svg,
};

use super::{HoverHelp, MainAction};
use crate::assets::Icon;
use crate::focus;
use crate::theme::{self, Palette, metrics};

/// The native checkbox's box, as WebKit draws it at the 13-pixel text size.
const BOX: f32 = 14.;

/// `.control-switch`: 34 by 20 pixels with a 12-pixel knob 3 pixels in from the edge, moved 14 pixels when on.
const SWITCH: (f32, f32) = (34., 20.);
const KNOB: f32 = 12.;
const KNOB_INSET: f32 = 3.;
const KNOB_TRAVEL: f32 = 14.;

/// What a toggle does with the new value.
type OnToggle = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// An on/off control. See the module docs.
#[derive(IntoElement)]
pub struct Checkbox {
    id: SharedString,
    label: SharedString,
    help: HoverHelp,
    role: Role,
    checked: bool,
    disabled: bool,
    switch: bool,
    inline: bool,
    on_toggle: Option<OnToggle>,
    on_enter: Option<MainAction>,
    focus: Option<FocusHandle>,
}

impl Checkbox {
    /// A checkbox labelled `label` (drawn beside it and its accessible name) with hover help `help`. `role` is what
    /// assistive technology and the test bridge see: [`Role::CheckBox`], or [`Role::Switch`] for a Settings switch.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
    ) -> Self {
        Checkbox {
            id: id.into(),
            label: label.into(),
            help,
            role,
            checked: false,
            disabled: false,
            switch: false,
            inline: false,
            on_toggle: None,
            on_enter: None,
            focus: None,
        }
    }

    /// Whether it is on.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// An unavailable control: drawn at half opacity, ignoring clicks and keys, and not focusable.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Draw it as the old Settings switch (`.control-switch`) instead of a checkbox, with its label before it.
    pub fn switch(mut self) -> Self {
        self.switch = true;
        self
    }

    /// Draw a switch at its own width with its words after it, in the quiet ink and 4 pixels away, instead of across a
    /// Settings row: the old Hosts heading's Details control (`.host-details-control`, a `label` holding the switch and
    /// then its text). Changes nothing on a plain checkbox, only on a [`Checkbox::switch`]; the words take the
    /// surrounding text size.
    pub fn inline(mut self) -> Self {
        self.inline = true;
        self
    }

    /// What a toggle does: called with the new value (the opposite of [`Checkbox::checked`]).
    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// The owner's main action (the launcher's Launch), which Enter on the focused control runs without toggling it,
    /// under the choice controls' shared Enter rule (`components::enter_choice`, which says what the action must
    /// guard itself); see the module docs. Without it, Enter goes on to whatever holds the control.
    pub fn on_enter(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_enter = Some(Rc::new(handler));
        self
    }

    /// Use `handle` as its focus handle, so its owner can make it where focus starts. Without it the control keeps its
    /// own handle in its element state.
    pub fn focus(mut self, handle: &FocusHandle) -> Self {
        self.focus = Some(handle.clone());
        self
    }

    /// The box of a checkbox: the well with the border when off, the accent fill with a check when on.
    fn draw_box(&self, palette: &Palette) -> gpui::Div {
        let (fill, edge) = if self.checked {
            (palette.accent_fill, palette.accent_edge)
        } else {
            (palette.well, palette.border)
        };
        div()
            .flex_none()
            .size(px(BOX))
            .flex()
            .items_center()
            .justify_center()
            .rounded(metrics::RADIUS_SM)
            .border_1()
            .border_color(theme::color(edge))
            .bg(theme::color(fill))
            .when(self.checked, |checkbox| {
                checkbox.child(
                    svg()
                        .path(Icon::Check.path())
                        .size(px(BOX - 4.))
                        .text_color(theme::color(palette.on_fill)),
                )
            })
    }

    /// The switch: its track (the well with the border when off, the accent fill with its edge when on) and its knob.
    fn draw_switch(&self, palette: &Palette) -> gpui::Div {
        let (track, edge, knob) = if self.checked {
            (palette.accent_fill, palette.accent_edge, palette.on_fill)
        } else {
            (palette.well, palette.border, palette.fg_1)
        };
        let offset = KNOB_INSET + if self.checked { KNOB_TRAVEL } else { 0. };
        div()
            .flex_none()
            .relative()
            .w(px(SWITCH.0))
            .h(px(SWITCH.1))
            .rounded(metrics::RADIUS_LG)
            .border_1()
            .border_color(theme::color(edge))
            .bg(theme::color(track))
            .child(
                div()
                    .absolute()
                    // Measured from inside the one-pixel border, as the old knob's margin was from the input's
                    // content box.
                    .top(px(KNOB_INSET))
                    .left(px(offset))
                    .size(px(KNOB))
                    .rounded_full()
                    .bg(theme::color(knob)),
            )
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let focus = self.focus.clone().unwrap_or_else(|| {
            focus::control_handle(
                ElementId::Name(format!("{}-focus", self.id).into()),
                window,
                cx,
            )
        });
        let ring = focus::shows_ring(&focus, window);
        let enabled = !self.disabled;
        let toggle = self
            .on_toggle
            .clone()
            .filter(|_| enabled)
            .map(|handler| (handler, !self.checked));
        let mark = if self.switch {
            self.draw_switch(palette)
        } else {
            self.draw_box(palette)
        };
        // The old focus outline, drawn as a ring element since GPUI has no outline: `2px solid var(--accent)` one pixel
        // outside the box or the switch, except on a checked switch, where the old stylesheet drew it in `--on-fill`
        // inside the accent track (`.control-switch:checked:focus-visible`: offset -3px, so its two pixels cover the
        // band one to three pixels in), so it shows against the fill.
        let inset_ring = self.switch && self.checked;
        let (ring_inset, ring_color, ring_radius) = if inset_ring {
            (px(1.), palette.on_fill, metrics::RADIUS_LG - px(1.))
        } else if self.switch {
            (px(-3.), palette.accent, metrics::RADIUS_LG + px(3.))
        } else {
            (px(-3.), palette.accent, metrics::RADIUS_SM + px(3.))
        };
        let mark = div()
            .relative()
            .flex_none()
            .child(mark)
            // A native disabled checkbox dimmed its box, not its words (`.control-switch:disabled` likewise).
            .when(!enabled, |mark| mark.opacity(0.5))
            .when(ring, |mark| {
                mark.child(
                    div()
                        .absolute()
                        .top(ring_inset)
                        .left(ring_inset)
                        .right(ring_inset)
                        .bottom(ring_inset)
                        .border_2()
                        .border_color(theme::color(ring_color))
                        .rounded(ring_radius),
                )
            });
        let words = div()
            .when(self.switch && self.inline, |words| {
                words.text_color(theme::color(palette.fg_1))
            })
            .child(self.label.clone());
        let mut control = div()
            .id(ElementId::Name(self.id.clone()))
            // Tests find a control by its id; this records nothing outside test builds.
            .debug_selector(|| self.id.to_string())
            .role(self.role)
            .aria_label(self.label.clone())
            .aria_toggled(if self.checked {
                Toggled::True
            } else {
                Toggled::False
            })
            .flex()
            .items_center()
            .gap(px(8.))
            // The words take the surrounding text size: the launcher's choices and Settings' rows each set their own.
            .text_color(theme::color(palette.fg_0));
        // A Settings row (`.setting-row`) put its words at the left and the switch at the far right, 16 pixels apart at
        // least, with the whole row a click target; a checkbox, and an inline switch, sit before their words.
        control = if self.switch && self.inline {
            control.gap(px(4.)).child(mark).child(words)
        } else if self.switch {
            control
                .w_full()
                .justify_between()
                .gap(px(16.))
                .child(words)
                .child(mark)
        } else {
            control.child(mark).child(words)
        };
        if enabled {
            control = control.cursor_pointer();
        }
        let control = super::with_hover_help(control, &self.id, &self.help, true, window, cx);
        let control = if enabled {
            focus::track(control, &focus, focus::OnPress::Control)
        } else {
            // Not tracked, so neither Tab nor a click can focus it; the press is still this control's, so it does not
            // reach the page's empty-space handler and move focus.
            control.on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        };
        let on_space = toggle.clone();
        // A disabled control is not focus-tracked, so its keys never arrive and the action needs no filter.
        let on_enter = self.on_enter.clone();
        // A click toggles; GPUI's keyboard "click" (the release of an Enter or Space pressed on the focused element) does
        // not, through the shared click rule: it would toggle a second time after Space, whose key handler below already
        // ran on the press, and toggle on Enter, which a checkbox must leave to the form around it.
        let on_click = toggle.map(|(handler, next)| -> super::Handler {
            Rc::new(move |_, window, cx| handler(next, window, cx))
        });
        super::activate_on_click(control, on_click)
            // Space only, as the native checkbox: Enter goes on to the form (here, whatever holds the control), unless
            // the owner gave its main action, which Enter then runs with the value shown. A held or modified Space does
            // nothing, as with every control's keys.
            .on_key_down(move |event, window, cx| {
                if super::enter_choice(event, on_enter.as_ref(), |_, _| {}, window, cx) {
                    return;
                }
                if event.is_held
                    || event.keystroke.modifiers.modified()
                    || event.keystroke.key != "space"
                {
                    return;
                }
                if let Some((handler, next)) = &on_space {
                    cx.stop_propagation();
                    handler(*next, window, cx);
                }
            })
    }
}
