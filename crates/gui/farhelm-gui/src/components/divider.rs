//! The divider: a thin, focusable separator between two panes that the user drags, double-clicks or steps with the
//! arrow keys to move. The sidebar's resize edge is the first (the old `.sidebar-resize-handle`).
//!
//! It draws nothing of its own at rest: the visible edge belongs to the pane beside it (the sidebar's hairline), and
//! the divider is only the strip that takes the pointer there, with the column-resize cursor. Its one mark is the old
//! `:focus-visible` ring, drawn inside the strip (`outline: 2px solid var(--fg-1); outline-offset: -2px`), shown only
//! after keyboard input as every control's ring is.
//!
//! It owns no position. Its owner says what value it stands for and in what range ([`Divider::new`]), which is what
//! assistive technology hears (a separator's value, minimum and maximum, as the old `aria-valuenow` and its bounds),
//! and hears what the user did:
//!
//! - **A drag** ([`Divider::on_drag`]) with the primary button reports where the pointer went in window coordinates:
//!   [`DividerDrag::Start`] at the press, [`DividerDrag::Move`] on every move until the release, and
//!   [`DividerDrag::End`] once. The move and release are watched across the whole window, not only over the strip, as
//!   the old handle's pointer capture did, so a drag keeps going when the pointer outruns the strip, and nothing under
//!   the pointer (the terminal, a row) sees the drag's moves or its release. A drag also ends when a move arrives with
//!   the button no longer held, which is how a release the window never saw (one outside it, or lost to another app)
//!   shows: the old handle's `pointercancel` and `lostpointercapture`. Either way the owner hears one `End` and keeps
//!   whatever the last move gave it. While a drag is on, the whole window shows the column-resize cursor, as the old
//!   stylesheet forced it on every element during a resize.
//! - **A double-click** ([`Divider::on_double_click`]) reports itself, and the second press starts no drag.
//! - **Left and Right** ([`Divider::on_step`]) on the focused divider report a step back or forward. Held keys repeat,
//!   as the old handle's arrows did (a slider's keys repeat everywhere), and a modified arrow is left alone. The key
//!   stops at the divider, so it reaches nothing behind it.
//!
//! ## Two parts: the strip and the grip
//!
//! A divider can need to reach over what lies beside it, and the sidebar's does. Each session row's actions-menu button
//! sits 2 pixels inside the sidebar's inner edge (`session_list/menu.rs`), so a 5-pixel strip kept inside the sidebar
//! would cover 2 pixels of every row's button, and one narrow enough to miss them (3 pixels) would be below the old
//! handle's 5-pixel hit area, which the old acceptance case checked. So, like the old handle, it covers the sidebar's
//! 1-pixel edge and reaches 4 pixels into the pane, where they land in the terminal's own 8-pixel padding
//! (`PANE_PADDING` in `terminal/attached.rs`) and cover no cell. GPUI gives the pointer to whatever is drawn last.
//! Drawn where it sits between the panes, the divider would lose those 4 pixels to the pane drawn after it, and the
//! terminal would take the press too; drawn after the pane, it would also come after the pane in Tab's order and in
//! what assistive technology reads, where the old handle sat between the panes. So it is drawn in two parts, which is
//! why it must not be folded back into the sidebar or into one element drawn after the pane. The divider itself
//! ([`Divider`], drawn where it sits between the panes) is the strip: the tab stop with its name and value, which takes
//! the arrows. Its grip ([`Divider::grip`], drawn by the owner after everything it reaches over, over the strip's own
//! box) takes the pointer: the press, the double-click, the drag, the hover help and the cursor, and draws the focus
//! ring, so the ring is not hidden under the pane either. The grip blocks the pointer from what lies under it,
//! scrolling aside, so a press on it never also reaches the terminal.
//!
//! Focus is the coordinator's. The divider is a separator, not a button: the old handle was a `tabindex=0` element that
//! Tab reached on every platform and that focused itself on every press. So the strip is tracked as an element that
//! takes focus when pressed ([`focus::OnPress::Focus`]), which Tab reaches on macOS with Keyboard navigation off as
//! well, where it skips buttons; and a press on the grip, which lies over the strip and so takes its presses, moves
//! focus to the strip on every platform, in one move through the coordinator ([`focus::focus_control`]).

use std::ops::RangeInclusive;
use std::rc::Rc;

use gpui::{
    App, CursorStyle, ElementId, Entity, FocusHandle, InteractiveElement, IntoElement, MouseButton,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, RenderOnce, Role, SharedString,
    StatefulInteractiveElement, Styled, Window, canvas, div, prelude::FluentBuilder as _,
};

use super::HoverHelp;
use crate::focus::{self, Mark};
use crate::theme;

/// What a drag on the divider did, in window coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DividerDrag {
    /// The primary button went down on the divider, with the pointer at this horizontal position.
    Start(Pixels),
    /// The pointer moved to this horizontal position with the button still held.
    Move(Pixels),
    /// The drag is over: the button came up, or a move showed it was no longer held.
    End,
}

/// Which way an arrow key on the focused divider steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DividerStep {
    /// Left.
    Back,
    /// Right.
    Forward,
}

/// What the divider reports a drag to.
type OnDrag = Rc<dyn Fn(DividerDrag, &mut Window, &mut App)>;
/// What the divider reports a double-click to.
type OnDoubleClick = Rc<dyn Fn(&mut Window, &mut App)>;
/// What the divider reports an arrow key to.
type OnStep = Rc<dyn Fn(DividerStep, &mut Window, &mut App)>;

/// Whether a drag that started on this grip is still on, kept in the grip's element state so the window-wide
/// listeners of a frame drawn before the press still see it.
#[derive(Default)]
struct Dragging(bool);

/// A focusable separator the user drags or steps: the strip part, drawn where it sits. See the module docs.
#[derive(IntoElement)]
pub struct Divider {
    id: SharedString,
    label: SharedString,
    help: HoverHelp,
    role: Role,
    value: u32,
    range: RangeInclusive<u32>,
    focus: FocusHandle,
    on_drag: Option<OnDrag>,
    on_double_click: Option<OnDoubleClick>,
    on_step: Option<OnStep>,
}

impl Divider {
    /// A divider. `id` must be stable (it keys the divider's drag and tooltip state, and tests and the bridge find it
    /// by it); `label` is its accessible name; `role` is what assistive technology and the test bridge see, which for
    /// a focusable separator is [`Role::Splitter`] (AccessKit's role for ARIA's focusable `separator`). `focus` is the
    /// strip's focus handle, the owner's, so the strip and its grip, drawn apart, share it (the grip shows its ring and
    /// moves focus to it). `value` is what the divider stands for now and `range` what it moves in, as assistive
    /// technology hears them; the divider clamps nothing, its owner does and reports the result here.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
        focus: &FocusHandle,
        value: u32,
        range: RangeInclusive<u32>,
    ) -> Self {
        Divider {
            id: id.into(),
            label: label.into(),
            help,
            role,
            value,
            range,
            focus: focus.clone(),
            on_drag: None,
            on_double_click: None,
            on_step: None,
        }
    }

    /// What a drag reports to: one [`DividerDrag::Start`], a [`DividerDrag::Move`] per move, one [`DividerDrag::End`].
    pub fn on_drag(
        mut self,
        handler: impl Fn(DividerDrag, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_drag = Some(Rc::new(handler));
        self
    }

    /// What a double-click reports to.
    pub fn on_double_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_double_click = Some(Rc::new(handler));
        self
    }

    /// What Left and Right on the focused divider report to.
    pub fn on_step(
        mut self,
        handler: impl Fn(DividerStep, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_step = Some(Rc::new(handler));
        self
    }

    /// The grip of this divider: the part that takes the pointer, for the owner to draw after everything the divider
    /// reaches over, over the strip's own box (see the module docs). Build it from a divider configured the same way as
    /// the one drawn as the strip.
    pub fn grip(self) -> DividerGrip {
        DividerGrip(self)
    }
}

impl RenderOnce for Divider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let strip = div()
            .id(ElementId::Name(self.id.clone()))
            // Tests find a control by its id; this records nothing outside test builds.
            .debug_selector(|| self.id.to_string())
            .role(self.role)
            .aria_label(self.label.clone())
            .aria_orientation(gpui::Orientation::Vertical)
            .size_full();
        let strip = strip
            .aria_numeric_value(f64::from(self.value))
            .aria_min_numeric_value(f64::from(*self.range.start()))
            .aria_max_numeric_value(f64::from(*self.range.end()))
            // The number as text too: AccessKit's numeric value is what a screen reader reads, and the text value is
            // what the frame record (and so the test bridge) reports.
            .aria_value(self.value.to_string());
        // The help as the strip's description; its tooltip is the grip's, which lies over it.
        let strip = super::with_hover_help(strip, &self.id, &self.help, true, window, cx);
        let strip = focus::track(strip, &self.focus, focus::OnPress::Focus);
        // The arrows, on the focused strip only (a key listener runs only on the focus path).
        let on_step = self.on_step.clone();
        strip.on_key_down(move |event, window, cx| {
            if event.keystroke.modifiers.modified() {
                return;
            }
            let step = match event.keystroke.key.as_str() {
                "left" => DividerStep::Back,
                "right" => DividerStep::Forward,
                _ => return,
            };
            cx.stop_propagation();
            if let Some(handler) = &on_step {
                handler(step, window, cx);
            }
        })
    }
}

/// The part of a [`Divider`] that takes the pointer, drawn over the strip after everything the divider reaches over.
/// See the module docs.
#[derive(IntoElement)]
pub struct DividerGrip(Divider);

impl RenderOnce for DividerGrip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let divider = self.0;
        let palette = theme::palette(cx);
        let id: SharedString = format!("{}-grip", divider.id).into();
        let strip = divider.focus.clone();
        let dragging =
            window.use_keyed_state(ElementId::Name(format!("{id}-drag").into()), cx, |_, _| {
                Dragging::default()
            });
        let ring = focus::shows_ring(&strip, window);
        let grip = div()
            .id(ElementId::Name(id.clone()))
            .size_full()
            .cursor_col_resize()
            // Only the pointer: a wheel over the grip still scrolls whatever lies under it.
            .block_mouse_except_scroll()
            .when(ring, |grip| {
                grip.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .border_2()
                        .border_color(theme::color(palette.fg_1)),
                )
            });
        let grip = super::with_hover_help(grip, &id, &divider.help, false, window, cx);

        // The press: focus moves to the strip on every press, in one move through the coordinator (see the module
        // docs). The grip blocks the pointer from everything behind it, so the page's empty-space rule and GPUI's own
        // focus-on-press never see this press; the prevented default is only a guard should that blocking change, the
        // mark every control's press carries in the coordinator's rule. A double-click reports itself and starts
        // nothing; any other primary press starts a drag.
        let (on_drag, on_double_click) = (divider.on_drag.clone(), divider.on_double_click.clone());
        let pressed = dragging.clone();
        let grip = grip.on_mouse_down(MouseButton::Left, move |event, window, cx| {
            window.prevent_default();
            focus::focus_control(&strip, &Mark::take(cx), window, cx);
            if event.click_count >= 2 {
                // The first press of the pair started a drag that may not have seen its release yet.
                end(&pressed, &on_drag, window, cx);
                if let Some(handler) = &on_double_click {
                    handler(window, cx);
                }
                return;
            }
            pressed.update(cx, |dragging, cx| {
                dragging.0 = true;
                cx.notify();
            });
            if let Some(handler) = &on_drag {
                handler(DividerDrag::Start(event.position.x), window, cx);
            }
        });

        // The drag's moves and its release, watched across the whole window. Registered on every frame, not only while
        // a drag is on, because a frame drawn before the press may still be the one in force when the first move
        // arrives; each listener asks the element state whether a drag is on. During a drag they take the moves and
        // the release in the capture phase and stop them there, as the old handle's pointer capture did, so the
        // terminal or a row under the pointer never sees a drag that is not theirs (a program reporting the mouse
        // would otherwise hear it).
        let on_drag = divider.on_drag.clone();
        let watch = canvas(
            |_, _, _| (),
            move |_, (), window, cx| {
                if dragging.read(cx).0 {
                    window.set_window_cursor_style(CursorStyle::ResizeColumn);
                }
                let (moved, released) = (dragging.clone(), dragging.clone());
                let (on_move, on_release) = (on_drag.clone(), on_drag.clone());
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase != gpui::DispatchPhase::Capture || !moved.read(cx).0 {
                        return;
                    }
                    cx.stop_propagation();
                    if event.pressed_button == Some(MouseButton::Left) {
                        if let Some(handler) = &on_move {
                            handler(DividerDrag::Move(event.position.x), window, cx);
                        }
                    } else {
                        end(&moved, &on_move, window, cx);
                    }
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase == gpui::DispatchPhase::Capture
                        && event.button == MouseButton::Left
                        && released.read(cx).0
                    {
                        cx.stop_propagation();
                        end(&released, &on_release, window, cx);
                    }
                });
            },
        )
        .absolute()
        .size_0();
        grip.child(watch)
    }
}

/// End the drag `dragging` says is on, if one is, and report it once.
fn end(dragging: &Entity<Dragging>, on_drag: &Option<OnDrag>, window: &mut Window, cx: &mut App) {
    if !dragging.read(cx).0 {
        return;
    }
    dragging.update(cx, |dragging, cx| {
        dragging.0 = false;
        cx.notify();
    });
    if let Some(handler) = on_drag {
        handler(DividerDrag::End, window, cx);
    }
}
