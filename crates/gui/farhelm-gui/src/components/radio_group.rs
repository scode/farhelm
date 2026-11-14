//! The radio group and the segmented control: one choice among a few options, drawn as the old app's segmented well or
//! as its plain native radios, with the keyboard of whichever control the old app used for the choice.
//!
//! The old app had one look and two kinds of control behind its few-choice pickers. `.segmented-control` in `app.css`
//! at reference commit `24bc0580` says so: "Two to five choices share this presentation while retaining their native
//! button or radio semantics". Some of its groups were real radios styled as segments (Appearance, the session status
//! filter, Settings' terminal link click), some a `role="group"` of buttons with `aria-pressed` (the launcher's
//! destination type, effort and permission mode, a host's kind, template choices, ntfy priority), and one fieldset was
//! plain, unstyled radios (the launcher's "Runs in YOLO mode"). So this is one control, and the role its owner gives it
//! picks the keyboard:
//!
//! - **[`Role::RadioGroup`] behaves as native radios.** The group is one tab stop: Tab enters at the chosen option and
//!   the next Tab leaves the group. The arrows move the choice and focus together to the next or previous available
//!   option, wrapping at the ends as WebKit's radios do, and Space chooses the focused option. With nothing chosen, Tab
//!   enters at the first available option and Shift+Tab at the last, as in WebKit. Options report the radio button role
//!   and whether they are checked. The old Credentials editor's `role="radio"` buttons (Claude Code or Codex) were
//!   plain buttons that only announced themselves as radios, with each one a tab stop and no arrows; under this role
//!   they would get the native keyboard their role promised when the Credentials editor is ported, and that port
//!   confirms it.
//! - **[`Role::Group`] behaves as the old buttons with `aria-pressed`.** Each available option is a tab stop of its
//!   own, Enter or Space chooses it, the arrows do nothing, and options report the button role with their pressed state
//!   (toggled, to assistive technology).
//!
//! Enter follows the old launcher's `enter_choice`, through [`RadioGroup::on_enter`]. Without it, Enter on a button
//! option chooses it, and Enter on a radio goes on to whatever holds the group, as a native radio's did. With it (the
//! owner's main action, a launch), Enter on a button option applies that option and then runs the action, and Enter on
//! a radio runs the action with the values shown, unchanged (SPEC.md's launcher: "Enter on a choice button ... applies
//! that choice and then launches"; "Enter on ... YOLO radios ... launches with the values shown"). A held Enter does
//! nothing there, so holding it never launches twice. A modified Enter (Shift+Enter) is not an activation and goes on to
//! the owner, following the controls' shared key rule, where the old `enter_choice` ignored modifiers and launched.
//!
//! A click chooses the option it lands on. Focus follows the platform's control rule
//! ([`crate::focus::OnPress::Control`]): on Linux the click focuses the option, on macOS it does not, and with
//! Keyboard navigation off a Mac's plain Tab skips the group, as WebKit skipped its radios and buttons there (SPEC.md,
//! "Keyboard focus").
//!
//! It holds no state: its owner says which value is chosen and hears each choice, so a refused change is undone by
//! drawing the stored value again. A radio group reports a choice only when the option was not already chosen, as a
//! native radio's change event did; a button group reports every press, as the old buttons' click handlers did.
//!
//! Not built yet, for when a consumer needs them: the terminal link choice's second line of description, the
//! launcher's glyphs in segments and its red unchosen YOLO segment, and the status filter's smaller segments.

use std::rc::Rc;

use gpui::{
    App, ElementId, FocusHandle, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled,
    Toggled, Window, div, prelude::FluentBuilder as _, px,
};

use super::HoverHelp;
use crate::focus;
use crate::theme::{self, Palette, metrics};

/// The plain radio's circle, as large as the checkbox's box so the launcher's native controls match, and the dot a
/// chosen one holds.
const CIRCLE: f32 = 14.;
const DOT: f32 = 6.;

/// `.segmented-control > button`: `min-height: 30px` and `padding: 4px 10px`, at `line-height: 1.4` of 13 pixels.
const SEGMENT_MIN_HEIGHT: f32 = 30.;
const SEGMENT_PAD: (f32, f32) = (4., 10.);
const SEGMENT_LINE_HEIGHT: f32 = 18.2;

/// What a choice does with the chosen option's value.
type OnChange<V> = Rc<dyn Fn(&V, &mut Window, &mut App)>;

/// The owner's main action, which Enter on an option runs.
type OnEnter = Rc<dyn Fn(&mut Window, &mut App)>;

/// One option of a [`RadioGroup`]: its words (drawn, and its accessible name), its hover help, and the value its owner
/// hears when it is chosen.
pub struct RadioOption<V> {
    label: SharedString,
    help: HoverHelp,
    value: V,
    disabled: bool,
}

impl<V> RadioOption<V> {
    /// An option labelled `label` with hover help `help`, standing for `value`. The label must be unique within its
    /// group: it keys the option's focus and tooltip, so they follow the option when the options around it change.
    pub fn new(label: impl Into<SharedString>, help: HoverHelp, value: V) -> Self {
        RadioOption {
            label: label.into(),
            help,
            value,
            disabled: false,
        }
    }

    /// An unavailable option: drawn dimmed (a segment whole, a plain radio's circle only, as a native radio's words
    /// stayed as they were), not focusable, and skipped by Tab and the arrows; a click does nothing.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// One choice among a few options. See the module docs.
#[derive(IntoElement)]
pub struct RadioGroup<V: Clone + PartialEq + 'static> {
    id: SharedString,
    label: SharedString,
    help: HoverHelp,
    role: Role,
    chosen: Option<V>,
    options: Vec<RadioOption<V>>,
    disabled: bool,
    radios: bool,
    on_change: Option<OnChange<V>>,
    on_enter: Option<OnEnter>,
}

impl<V: Clone + PartialEq + 'static> RadioGroup<V> {
    /// A group labelled `label` (its accessible name; any visible legend is its owner's to draw) with hover help
    /// `help` (its accessible description, and shown when the pointer reaches the group's gaps or edge from outside
    /// it). `id` must be stable and unique among its siblings; each option
    /// is drawn under `<id>-<its label>`, which is how tests and the test bridge find it. `role` picks the keyboard:
    /// [`Role::RadioGroup`] for native radios, [`Role::Group`] for the old segmented buttons (the module docs). Any
    /// other role is a mistake, caught in debug builds. Drawn as the segmented well unless [`RadioGroup::radios`] asks
    /// for plain radios.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
    ) -> Self {
        debug_assert!(
            matches!(role, Role::RadioGroup | Role::Group),
            "a radio group's role picks its keyboard, so it must be a radio group or a group, not {role:?}"
        );
        RadioGroup {
            id: id.into(),
            label: label.into(),
            help,
            role,
            chosen: None,
            options: Vec::new(),
            disabled: false,
            radios: false,
            on_change: None,
            on_enter: None,
        }
    }

    /// Add `option` after the options already added.
    pub fn option(mut self, option: RadioOption<V>) -> Self {
        self.options.push(option);
        self
    }

    /// The value chosen now, or `None` when nothing is (a required choice with no default, such as the launcher's
    /// YOLO question).
    pub fn chosen(mut self, chosen: Option<V>) -> Self {
        self.chosen = chosen;
        self
    }

    /// An unavailable group: every option drawn dimmed as an unavailable option is, ignoring clicks and keys, and not
    /// focusable.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Draw the options as plain radios, a circle before each option's words (the launcher's YOLO fieldset), instead
    /// of the segmented well. Plain radios are radios: only a [`Role::RadioGroup`] may use this look, since buttons
    /// drawn as radio circles would take Tab and the arrows differently from what they show. Debug builds enforce it.
    pub fn radios(mut self) -> Self {
        debug_assert_eq!(
            self.role,
            Role::RadioGroup,
            "plain radios behave as radios: give the group Role::RadioGroup"
        );
        self.radios = true;
        self
    }

    /// What a choice does: called with the chosen option's value.
    pub fn on_change(mut self, handler: impl Fn(&V, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// The owner's main action, which Enter on a focused option runs (after choosing it, in a button group). The old
    /// launcher's `enter_choice`; see the module docs.
    pub fn on_enter(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_enter = Some(Rc::new(handler));
        self
    }
}

/// What every option's handlers share: the options' values, focus handles and availability, which one is chosen, and
/// the owner's handlers.
struct Choice<V> {
    radio: bool,
    values: Vec<V>,
    handles: Vec<FocusHandle>,
    enabled: Vec<bool>,
    chosen: Option<usize>,
    on_change: Option<OnChange<V>>,
    on_enter: Option<OnEnter>,
}

impl<V> Choice<V> {
    /// Choose option `index`: the owner hears it, except a radio already chosen (no change, as a native radio).
    fn choose(&self, index: usize, window: &mut Window, cx: &mut App) {
        if self.radio && self.chosen == Some(index) {
            return;
        }
        if let Some(handler) = &self.on_change {
            handler(&self.values[index], window, cx);
        }
    }

    /// The arrows on radio `from`: the next available option forward or back, wrapping around the ends, takes focus
    /// and is chosen. With no other option available, it is `from` itself, which is chosen if it was not.
    fn step(&self, from: usize, forward: bool, window: &mut Window, cx: &mut App) {
        let count = self.values.len();
        let next = (1..=count)
            .map(|offset| {
                if forward {
                    (from + offset) % count
                } else {
                    (from + count - offset) % count
                }
            })
            .find(|&index| self.enabled[index]);
        if let Some(next) = next {
            focus::move_within_control(&self.handles[from], &self.handles[next], window, cx);
            self.choose(next, window, cx);
        }
    }

    /// A key pressed on the focused option `index`. See the module docs for which keys do what under each role.
    fn key(&self, index: usize, event: &KeyDownEvent, window: &mut Window, cx: &mut App) {
        // A modified key is not an activation, the controls' shared key rule: a Shift+Enter goes on to the owner rather
        // than running `on_enter`, where the old launcher's `enter_choice` ignored modifiers and launched on it.
        if event.keystroke.modifiers.modified() {
            return;
        }
        match (event.keystroke.key.as_str(), &self.on_enter) {
            ("space", _) if !event.is_held => {
                cx.stop_propagation();
                self.choose(index, window, cx);
            }
            ("enter", Some(on_enter)) => {
                // Taken even when held, so a held Enter neither runs the action again nor reaches what holds the group.
                cx.stop_propagation();
                if !event.is_held {
                    if !self.radio {
                        self.choose(index, window, cx);
                    }
                    on_enter(window, cx);
                }
            }
            ("enter", None) if !self.radio && !event.is_held => {
                cx.stop_propagation();
                self.choose(index, window, cx);
            }
            ("down" | "right", _) if self.radio => {
                cx.stop_propagation();
                self.step(index, true, window, cx);
            }
            ("up" | "left", _) if self.radio => {
                cx.stop_propagation();
                self.step(index, false, window, cx);
            }
            _ => {}
        }
    }
}

/// Which options are tab stops. A button group's every available option is one. A radio group is one stop: the
/// focused option while focus is inside it, so the next Tab leaves the group; otherwise the chosen option, so Tab
/// enters there; and with nothing chosen (or the chosen option unavailable), every available option, so Tab enters at
/// the first and Shift+Tab at the last, which is WebKit's rule for a group with nothing checked.
fn tab_stops(
    radio: bool,
    enabled: &[bool],
    chosen: Option<usize>,
    focused: Option<usize>,
) -> Vec<bool> {
    let only =
        |stop: usize| -> Vec<bool> { (0..enabled.len()).map(|index| index == stop).collect() };
    match (radio, focused, chosen) {
        (true, Some(focused), _) => only(focused),
        (true, None, Some(chosen)) if enabled[chosen] => only(chosen),
        _ => enabled.to_vec(),
    }
}

impl<V: Clone + PartialEq + 'static> RenderOnce for RadioGroup<V> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let radio = self.role == Role::RadioGroup;
        let ids: Vec<SharedString> = self
            .options
            .iter()
            .map(|option| format!("{}-{}", self.id, option.label).into())
            .collect();
        let handles: Vec<FocusHandle> = ids
            .iter()
            .map(|id| {
                focus::control_handle(ElementId::Name(format!("{id}-focus").into()), window, cx)
            })
            .collect();
        let enabled: Vec<bool> = self
            .options
            .iter()
            .map(|option| !self.disabled && !option.disabled)
            .collect();
        let chosen = self
            .options
            .iter()
            .position(|option| self.chosen.as_ref() == Some(&option.value));
        let focused = handles.iter().position(|handle| handle.is_focused(window));
        // Set on the handles themselves: GPUI builds Tab's order from each tracked handle's flag as the frame is drawn.
        let handles: Vec<FocusHandle> = handles
            .into_iter()
            .zip(tab_stops(radio, &enabled, chosen, focused))
            .map(|(handle, stop)| handle.tab_stop(stop))
            .collect();
        let choice = Rc::new(Choice {
            radio,
            values: self
                .options
                .iter()
                .map(|option| option.value.clone())
                .collect(),
            handles: handles.clone(),
            enabled: enabled.clone(),
            chosen,
            on_change: self.on_change.clone(),
            on_enter: self.on_enter.clone(),
        });
        let option_role = if radio {
            Role::RadioButton
        } else {
            Role::Button
        };
        let mut options = Vec::with_capacity(self.options.len());
        for (index, option) in self.options.iter().enumerate() {
            let look = Look {
                chosen: chosen == Some(index),
                enabled: enabled[index],
                ring: focus::shows_ring(&handles[index], window),
            };
            let element = if self.radios {
                look.radio(&ids[index], &option.label, palette)
            } else {
                look.segment(&ids[index], &option.label, palette)
            };
            let element = element
                .role(option_role)
                .aria_label(option.label.clone())
                .aria_toggled(if look.chosen {
                    Toggled::True
                } else {
                    Toggled::False
                });
            let element =
                super::with_hover_help(element, &ids[index], &option.help, true, window, cx);
            if !look.enabled {
                // Not tracked, so neither Tab nor a click can focus it; the press is still this control's, so it does
                // not reach the page's empty-space handler and move focus.
                options.push(
                    element
                        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                        .into_any_element(),
                );
                continue;
            }
            let (on_key, on_click) = (choice.clone(), choice.clone());
            // Keys are handled here; the shared click rule ignores the click GPUI makes from a released Enter or
            // Space on a focused element, which would choose a radio on Enter and press a segment twice.
            let element = focus::track(element, &handles[index], focus::OnPress::Control)
                .on_key_down(move |event, window, cx| on_key.key(index, event, window, cx));
            let on_click: super::Handler =
                Rc::new(move |_, window, cx| on_click.choose(index, window, cx));
            options.push(super::activate_on_click(element, Some(on_click)).into_any_element());
        }
        let group = div()
            .id(ElementId::Name(self.id.clone()))
            // Tests find a control by its id; this records nothing outside test builds.
            .debug_selector(|| self.id.to_string())
            .role(self.role)
            .aria_label(self.label.clone())
            // The group's help is its accessible description, as every control's is (`HoverHelp`). Its hover listener
            // cannot go on this element (see the surface below), so the text is read here directly rather than through
            // `with_hover_help`, which would add that listener; this module sits inside the one that keeps the text
            // private, and the surface still wires the help through the shared rule.
            .aria_description(self.help.0.clone())
            .relative()
            .flex()
            .flex_wrap()
            .items_center();
        let group = if self.radios {
            group.gap_x(px(16.)).gap_y(px(8.))
        } else {
            // `.segmented-control`: the options in a well with a 2-pixel gap and padding inside a dim edge.
            group
                .gap(px(2.))
                .p(px(2.))
                .border_1()
                .border_color(theme::color(palette.border_dim))
                .rounded(metrics::RADIUS_MD)
                .bg(theme::color(palette.well))
        };
        // The group's own hover help is wired to a surface laid over the whole group, after the options, because GPUI
        // tells hover listeners about a pointer move from the last drawn to the first: drawn last, this surface hears
        // first, so an option's help, heard after it, wins whenever the pointer is over an option. Wired on the group
        // itself, it would be heard after the options and replace their help. The group's help shows when the pointer
        // reaches a gap or the edge from outside the group; moving from an option onto a gap does not bring it back,
        // since the surface was hovered all along and hears no new hover. The surface takes no presses, so clicks reach
        // the options under it. Its id is an integer, which no option's `Name` id (`<id>-<label>`) can equal.
        let surface = div()
            .id(ElementId::Integer(0))
            .absolute()
            .top_0()
            .left_0()
            .size_full();
        let surface = super::with_hover_help(surface, &self.id, &self.help, false, window, cx);
        group.children(options).child(surface)
    }
}

/// How one option is drawn: whether it is chosen, available, and showing the keyboard focus ring.
#[derive(Clone, Copy)]
struct Look {
    chosen: bool,
    enabled: bool,
    ring: bool,
}

impl Look {
    /// One segment of `.segmented-control`: transparent with the control hover fill, or the accent fill with the
    /// inverse ink and semibold words when chosen, one step up the accent family under the pointer so hovering the
    /// choice does not hide it. Its focus ring is inset (the old `outline-offset: -3px`), in the inverse ink on the
    /// chosen segment, so it stays visible inside the rounded well.
    fn segment(
        self,
        id: &SharedString,
        label: &SharedString,
        palette: &Palette,
    ) -> gpui::Stateful<gpui::Div> {
        let (fill, hover, ink, ring) = if self.chosen {
            (
                palette.accent_fill,
                palette.accent_fill_hover,
                palette.on_fill,
                palette.on_fill,
            )
        } else {
            (
                theme::TRANSPARENT,
                palette.control_hover,
                palette.fg_0,
                palette.accent,
            )
        };
        div()
            .id(ElementId::Name(id.clone()))
            .debug_selector(|| id.to_string())
            .relative()
            .flex()
            .flex_grow(1.)
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .min_h(px(SEGMENT_MIN_HEIGHT))
            .py(px(SEGMENT_PAD.0))
            .px(px(SEGMENT_PAD.1))
            .rounded(metrics::RADIUS_SM)
            .bg(theme::color(fill))
            .text_size(metrics::TEXT_MD)
            .line_height(px(SEGMENT_LINE_HEIGHT))
            .text_color(theme::color(ink))
            .when(self.chosen, |segment| {
                segment.font_weight(FontWeight::SEMIBOLD)
            })
            .when(self.enabled, |segment| {
                segment
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme::color(hover)))
            })
            .when(!self.enabled, |segment| segment.opacity(0.5))
            .child(label.clone())
            .when(self.ring, |segment| {
                segment.child(
                    div()
                        .absolute()
                        .top(px(1.))
                        .left(px(1.))
                        .right(px(1.))
                        .bottom(px(1.))
                        .border_2()
                        .border_color(theme::color(ring))
                        .rounded(metrics::RADIUS_SM - px(1.)),
                )
            })
    }

    /// One plain radio, as the launcher's YOLO fieldset drew it: the platform's circle tinted with the accent (the old
    /// page's `accent-color`), filled with a dot in the inverse ink when chosen, then the words in the form's 12-pixel
    /// secondary ink (`.create-session-form label`). The words are part of the control, as the old `<label>` around the
    /// input made them, so a click on them chooses too. Its focus ring sits around the circle, as the old outline did.
    fn radio(
        self,
        id: &SharedString,
        label: &SharedString,
        palette: &Palette,
    ) -> gpui::Stateful<gpui::Div> {
        let (fill, edge) = if self.chosen {
            (palette.accent_fill, palette.accent_edge)
        } else {
            (palette.well, palette.border)
        };
        let circle = div()
            .relative()
            .flex_none()
            .size(px(CIRCLE))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .border_1()
            .border_color(theme::color(edge))
            .bg(theme::color(fill))
            // Only the circle dims when unavailable, as a native disabled radio did; its words keep their ink.
            .when(!self.enabled, |circle| circle.opacity(0.5))
            .when(self.chosen, |circle| {
                circle.child(
                    div()
                        .size(px(DOT))
                        .rounded_full()
                        .bg(theme::color(palette.on_fill)),
                )
            })
            .when(self.ring, |circle| {
                circle.child(
                    div()
                        .absolute()
                        .top(px(-3.))
                        .left(px(-3.))
                        .right(px(-3.))
                        .bottom(px(-3.))
                        .border_2()
                        .border_color(theme::color(palette.accent))
                        .rounded_full(),
                )
            });
        div()
            .id(ElementId::Name(id.clone()))
            .debug_selector(|| id.to_string())
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(palette.fg_1))
            .when(self.enabled, |radio| radio.cursor_pointer())
            .child(circle)
            .child(label.clone())
    }
}
