//! The select: a closed control showing the chosen option, which opens a list of the options under itself. The old app's
//! native `<select>`.
//!
//! Ported from the old app's selects at the reference commit (`24bc0580`): the launcher's Effort select
//! (`launch_controls.rs`, shown when a harness offers more efforts than the segments hold) and Agent type select
//! (`list/create_form.rs`), and the environment selects (Recipe and Container host, `environment_ui.rs`), whose
//! unavailable and retained options this carries. Here's the TLDR of how it behaves:
//!
//! - **The closed control** is the old stylesheet's select box (`.launch-composer-effort-select`, which the templates'
//!   selects match but for two pixels of height and two of side padding): the recessed `--well`, a one-pixel
//!   `--border`, the small radius, 13-pixel text 6 by 10 pixels in, at least 36 pixels high, and the chevron at its
//!   right end. It fills the width it is given, as the old ones were `width: 100%`, and cuts a long option with an
//!   ellipsis.
//! - **The list** is a menu of the layer manager's, below the control and exactly as wide ([`Anchor::matching_width`]),
//!   drawn with the shared menu items ([`MenuItem::choice`]); the chosen option carries a tick, as the platform's
//!   pop-up menu marked it. Opening it moves focus to the chosen option (to the first available one when the chosen one
//!   cannot be chosen); Up and Down walk the options that can be chosen, as in every menu, and the pointer moves the
//!   same highlight; Enter, Space or a click chooses and closes; Escape, a press outside or a press on the closed
//!   control closes without choosing. Closing returns focus to the closed control. GPUI Kit's `Select` is banned (it
//!   draws its own overlay), so this is the component layer's own.
//! - **Keys on the closed control** are WebKit's on macOS, where the old app shipped: Space, Up and Down open the list
//!   rather than stepping the value, and Alt+Down or Alt+Up (the convention of the other desktops) open it too
//!   ([`closed_key`]). Enter does not open it: in WebKit a Return on a closed select submitted its form. So Enter goes
//!   on to whatever holds the control, unless the owner gave the dialog's main action ([`Select::on_enter`]), which
//!   Enter then runs with the value shown, under the choice controls' shared Enter rule ([`super::enter_choice`]). A
//!   closed select has no focused choice to apply first, and the old launcher applied none: its selects' Enter called
//!   `enter_choice` with nothing to apply, since each pick had already reached the owner on its own, and SPEC.md's
//!   launcher has Enter on "the closed host picker, agent-type or effort dropdown" launch "with the values shown".
//! - **Focus** follows the pop-up rule ([`focus::OnPress::Focus`]): a press on it focuses it on every platform, and Tab
//!   reaches it even on macOS with Keyboard navigation off, where Tab reaches text fields and pop-ups only and skips
//!   buttons. With the button rule it would drop out of Tab's order there.
//! - **No state of its own** but whether its list is open: its owner says which value is chosen and hears each choice
//!   ([`Select::on_choose`]), as with the checkbox, so a refused change can put the old value back by drawing it. A
//!   choice of the option already chosen closes the list and tells the owner nothing, as a native select fired no
//!   change for it.
//!
//! An option can be unavailable ([`SelectOption::unavailable`]): drawn dimmed with its reason after its label (" · " and
//! the reason, the old options' wording), which is also its accessible description, and impossible to choose; the arrow
//! keys pass over it. A select whose stored value is no longer among its options can keep showing it, chosen, until the
//! user picks another ([`Select::retained`]), as the old environment selects did with "Unavailable recipe (retained)".
//! And a select with nothing chosen shows a placeholder in the closed control ([`Select::placeholder`]), as the old
//! "Choose a host" and "Choose a recipe" options did.
//!
//! NOTE: Deviations from the old native control, all decided or accepted as part of the GPUI migration. The pop-up was
//! the platform's own menu, which on macOS opened over the control with the chosen option on top of it and was at
//! least as wide as the control; this list is an anchored menu below the control (above it when there is no room
//! below), exactly as wide as the control, so an option longer than that is cut with an ellipsis (the migration's
//! orchestrator chose "exactly" for the launcher's floating lists on 2026-11-14). The arrow keys wrap from the last
//! option to the first, as in every menu here; the native menu stopped at the ends. The old "Choose a recipe"
//! placeholder was also a disabled first entry of the list; here a placeholder shows in the closed control only.
//! Typing a letter to jump to an option is not built. An option's label is the app's own words: a value a host wrote
//! (a recipe or host name) needs a host-text label, which comes with the first select that shows one.

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, ClickEvent, Context, ElementId, Entity, FocusHandle, InteractiveElement,
    IntoElement, Keystroke, Modifiers, MouseButton, ParentElement, Render, RenderOnce, Role,
    SharedString, StatefulInteractiveElement, Styled, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px, svg,
};

use super::{Handler, HoverHelp, MainAction, MenuItem};
use crate::assets::Icon;
use crate::focus::{self, Mark};
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::theme::{self, metrics};

/// `.launch-composer-effort-select`'s padding, vertical then horizontal.
const PAD: (f32, f32) = (6., 10.);
/// `.launch-composer-effort-select`'s `min-height`.
const MIN_HEIGHT: f32 = 36.;

/// What a choice does with the chosen option's value.
type OnChoose = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// One option of a [`Select`]: the value its owner stores, the words shown for it, and its hover help.
#[derive(Clone)]
pub struct SelectOption {
    value: SharedString,
    label: SharedString,
    help: HoverHelp,
    /// Why it cannot be chosen, if it cannot.
    unavailable: Option<SharedString>,
}

impl SelectOption {
    /// An option worth `value` to the select's owner, shown as `label` (the app's own words) with hover help `help`.
    /// `value` must be unique among the select's options: it keys the option's element and its focus.
    pub fn new(
        value: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
    ) -> Self {
        SelectOption {
            value: value.into(),
            label: label.into(),
            help,
            unavailable: None,
        }
    }

    /// An option that cannot be chosen now, for `reason` ("Needs setup"): drawn dimmed with the reason after its label,
    /// reported with the reason as its description, and skipped by the arrow keys. It still shows in the closed
    /// control, with its reason, when it is the chosen one.
    pub fn unavailable(mut self, reason: impl Into<SharedString>) -> Self {
        self.unavailable = Some(reason.into());
        self
    }

    /// The words the closed control shows for this option: its label, and its reason when it is unavailable.
    fn shown(&self) -> SharedString {
        match &self.unavailable {
            Some(reason) => format!("{} · {reason}", self.label).into(),
            None => self.label.clone(),
        }
    }
}

/// A select. See the module docs.
#[derive(IntoElement)]
pub struct Select {
    id: SharedString,
    label: SharedString,
    help: HoverHelp,
    role: Role,
    options: Vec<SelectOption>,
    chosen: Option<SharedString>,
    retained: Option<(SharedString, HoverHelp)>,
    placeholder: Option<SharedString>,
    disabled: bool,
    on_choose: Option<OnChoose>,
    on_enter: Option<MainAction>,
    focus: Option<FocusHandle>,
}

impl Select {
    /// A select. `id` must be stable and unique among its siblings (it keys the control's element state, its list's
    /// among it, and the test bridge finds the control by it); `label` is its accessible name ("Effort"), which the
    /// form around it shows as the row's label; `role` is what assistive technology and the test bridge see,
    /// [`Role::ComboBox`] for a select.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
    ) -> Self {
        Select {
            id: id.into(),
            label: label.into(),
            help,
            role,
            options: Vec::new(),
            chosen: None,
            retained: None,
            placeholder: None,
            disabled: false,
            on_choose: None,
            on_enter: None,
            focus: None,
        }
    }

    /// The options, in the order the list shows them.
    pub fn options(mut self, options: impl IntoIterator<Item = SelectOption>) -> Self {
        self.options.extend(options);
        self
    }

    /// The value of the chosen option. Without one nothing is chosen, and the closed control shows its placeholder.
    pub fn chosen(mut self, value: impl Into<SharedString>) -> Self {
        self.chosen = Some(value.into());
        self
    }

    /// When the chosen value is none of the options (a stored recipe no longer offered), show it anyway: as the list's
    /// first option, chosen, labelled `label` ("Unavailable recipe (retained)") with hover help `help`, until the user
    /// picks another. Without this, such a value shows as nothing chosen.
    pub fn retained(mut self, label: impl Into<SharedString>, help: HoverHelp) -> Self {
        self.retained = Some((label.into(), help));
        self
    }

    /// What the closed control shows while nothing is chosen ("Choose a host").
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// An unavailable select: drawn at half opacity, ignoring presses and keys, not focusable, and its list, if open,
    /// closes.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// What a choice does: called with the value of the option the user chose, after the list has closed, and only
    /// when it differs from the chosen one.
    pub fn on_choose(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_choose = Some(Rc::new(handler));
        self
    }

    /// The owner's main action (the launcher's Launch), which Enter on the closed control runs with the value shown,
    /// under the choice controls' shared Enter rule (`components::enter_choice`, which says what the action must
    /// guard itself); see the module docs. Without it, Enter goes on to whatever holds the control. The open list
    /// keeps its own Enter, which chooses and closes.
    pub fn on_enter(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_enter = Some(Rc::new(handler));
        self
    }

    /// Use `handle` as the closed control's focus handle, so its owner can make it where focus starts. Without it the
    /// control keeps its own handle in its element state.
    pub fn focus(mut self, handle: &FocusHandle) -> Self {
        self.focus = Some(handle.clone());
        self
    }

    /// The options the list shows: the retained value first when there is one (see [`Select::retained`]), then the
    /// options.
    fn entries(&self) -> Vec<SelectOption> {
        let retained = match (&self.chosen, &self.retained) {
            (Some(chosen), Some((label, help)))
                if !self.options.iter().any(|option| &option.value == chosen) =>
            {
                Some(SelectOption::new(
                    chosen.clone(),
                    label.clone(),
                    help.clone(),
                ))
            }
            _ => None,
        };
        retained
            .into_iter()
            .chain(self.options.iter().cloned())
            .collect()
    }
}

/// What a key pressed on the closed control does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClosedKey {
    /// Open the list.
    Open,
    /// Not the select's: the key goes on to whatever holds the control.
    PassOn,
}

/// The closed control's own keys, in one place (see the module docs): Space, Up and Down open the list, as WebKit's
/// select did on macOS, and so do Alt+Down and Alt+Up. Everything else goes on, Enter included: WebKit submitted the
/// form on a closed select's Return. When the owner gave its main action, the choice controls' shared Enter rule
/// ([`super::enter_choice`]) takes Enter before these keys are asked. A held key opens nothing, so a repeat that
/// arrives after the list closed does not reopen it. These are key listeners, and GPUI dispatches key bindings first: a
/// binding for `up` or `down` in a context around the select (only the menu context binds them today) would take the
/// arrows before they reach it.
fn closed_key(keystroke: &Keystroke, held: bool) -> ClosedKey {
    if held {
        return ClosedKey::PassOn;
    }
    let plain = !keystroke.modifiers.modified();
    match keystroke.key.as_str() {
        "space" if plain => ClosedKey::Open,
        "up" | "down" if plain || keystroke.modifiers == Modifiers::alt() => ClosedKey::Open,
        _ => ClosedKey::PassOn,
    }
}

/// Whether a select's list is open, kept in the closed control's element state: the one thing the select remembers
/// between frames. Weak, so a list the layer manager has let go of is not kept alive here.
#[derive(Default)]
struct SelectState {
    open: Option<(LayerId, WeakEntity<SelectList>)>,
    /// Whether the latest press on the closed control closed its list ([`Opening::press`]), so the click that press
    /// ends in does not open it again. Set afresh by every press, so a press that was dragged off the control and
    /// never became a click leaves nothing behind for the next one.
    closed_by_press: bool,
}

/// What the list shows and does, as the closed control last drew it: refreshed on every frame while the list is open,
/// so an option that becomes unavailable, or a value chosen elsewhere, shows in the open list too.
#[derive(Clone)]
struct Choices {
    /// The select's id, which the options' element ids start with.
    id: SharedString,
    /// The select's accessible name, which the list carries too.
    label: SharedString,
    entries: Vec<SelectOption>,
    chosen: Option<SharedString>,
    on_choose: Option<OnChoose>,
}

impl Choices {
    /// The entry whose value is `value`, if any.
    fn entry(&self, value: &SharedString) -> Option<&SelectOption> {
        self.entries.iter().find(|entry| &entry.value == value)
    }
}

impl RenderOnce for Select {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let focus = self.focus.clone().unwrap_or_else(|| {
            focus::control_handle(
                ElementId::Name(format!("{}-focus", self.id).into()),
                window,
                cx,
            )
        });
        let anchor = Anchor::for_element(
            ElementId::Name(format!("{}-anchor", self.id).into()),
            window,
            cx,
        )
        .matching_width();
        let state = window.use_keyed_state(
            ElementId::Name(format!("{}-list-state", self.id).into()),
            cx,
            |_, _| SelectState::default(),
        );
        let enabled = !self.disabled;
        let choices = Choices {
            id: self.id.clone(),
            label: self.label.clone(),
            entries: self.entries(),
            chosen: self.chosen.clone(),
            on_choose: self.on_choose.clone(),
        };

        // The list follows what its owner draws now. Written without a notify: the list is drawn later in this same
        // frame, in the overlay, so it reads the fresh copy without asking for another frame.
        let open = state.read(cx).open.clone();
        let open = open.filter(|(layer, _)| layers::is_open(*layer, window, cx));
        if let Some((layer, list)) = &open {
            let _ = list.update(cx, |list, _| list.choices = choices.clone());
            if !enabled {
                // Not closed from inside the frame being drawn: the close moves focus, which belongs to an event.
                let layer = *layer;
                window.defer(cx, move |window, cx| layers::close(layer, window, cx));
            }
        }

        let shown = self
            .chosen
            .as_ref()
            .and_then(|chosen| choices.entry(chosen))
            .map(SelectOption::shown)
            .or_else(|| self.placeholder.clone())
            .unwrap_or_default();
        let ring = enabled && focus::shows_ring(&focus, window);
        let mut control = div()
            .id(ElementId::Name(self.id.clone()))
            .relative()
            // Tests find a control by its id; this records nothing outside test builds.
            .debug_selector(|| self.id.to_string())
            .role(self.role)
            .aria_label(self.label.clone())
            .aria_value(shown.clone())
            .aria_expanded(open.is_some())
            .w_full()
            .min_w_0()
            .min_h(px(MIN_HEIGHT))
            .flex()
            .items_center()
            .gap(px(8.))
            .py(px(PAD.0))
            .px(px(PAD.1))
            .bg(theme::color(palette.well))
            .border_1()
            .border_color(theme::color(palette.border))
            .rounded(metrics::RADIUS_SM)
            .text_size(metrics::TEXT_MD)
            .text_color(theme::color(palette.fg_0))
            .child(div().flex_1().min_w_0().truncate().child(shown))
            .child(
                svg()
                    .path(Icon::ChevronDown.path())
                    .size(px(14.))
                    .flex_none()
                    .text_color(theme::color(palette.fg_0)),
            )
            // A native disabled select dimmed as a whole.
            .when(!enabled, |control| control.opacity(0.5));
        if ring {
            // The old global `:focus-visible` ring, as the buttons draw it: two pixels of accent, one pixel clear.
            control = control.child(
                div()
                    .absolute()
                    .top(px(-3.))
                    .left(px(-3.))
                    .right(px(-3.))
                    .bottom(px(-3.))
                    .border_2()
                    .border_color(theme::color(palette.accent))
                    .rounded(metrics::RADIUS_SM + px(3.)),
            );
        }
        let control = super::with_hover_help(control, &self.id, &self.help, true, window, cx);
        // The anchor's tracker sits on a borderless wrapper rather than on the control: an absolute child is placed
        // inside its parent's border, so on the control it would measure a box one pixel in on every side, and the
        // list would be two pixels narrower than the control and one pixel to the right.
        let tracked = anchor.track();
        let wrap = |control| {
            div()
                .relative()
                .w_full()
                .min_w_0()
                .child(control)
                .child(tracked)
        };
        if !enabled {
            // Not tracked, so neither Tab nor a click can focus it; the press is still this control's, so it does not
            // reach the page's empty-space handler and move focus.
            return wrap(
                control.on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default()),
            );
        }

        let opening = Rc::new(Opening {
            state,
            anchor: anchor.clone(),
            choices,
        });
        let (on_key, on_press) = (opening.clone(), opening.clone());
        let on_enter = self.on_enter.clone();
        let clicked: Handler = Rc::new(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            opening.click(window, cx)
        });
        let control = focus::track(control, &focus, focus::OnPress::Focus)
            .on_key_down(move |event, window, cx| {
                // Nothing to apply: a closed select has no focused choice, and its value is the one shown.
                if super::enter_choice(event, on_enter.as_ref(), |_, _| {}, window, cx) {
                    return;
                }
                if closed_key(&event.keystroke, event.is_held) == ClosedKey::Open {
                    cx.stop_propagation();
                    on_key.open(window, cx);
                }
            })
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                on_press.press(window, cx)
            });
        // A pointer click opens the list (a press already closed an open one). GPUI's keyboard "click" (the release
        // of an Enter or Space pressed on the closed control) is dropped by the shared click rule: Enter must open
        // nothing, and Space opened the list on its press already.
        wrap(super::activate_on_click(control, Some(clicked)))
    }
}

/// Everything the closed control needs to open its list: its state, its anchor and what the list shows.
struct Opening {
    state: Entity<SelectState>,
    anchor: Anchor,
    choices: Choices,
}

impl Opening {
    /// The list that is open now, if any.
    fn open_layer(&self, window: &Window, cx: &App) -> Option<LayerId> {
        let (layer, _) = self.state.read(cx).open.clone()?;
        layers::is_open(layer, window, cx).then_some(layer)
    }

    /// A press on the closed control: it closes the list if the list is open. On the press rather than the click,
    /// because the press has already focused the control (the pop-up press rule), taking focus out of the open list;
    /// a press dragged off the control before its release makes no click, and would leave the list open with focus
    /// outside it, its keys gone. The layer manager leaves a press on the anchor to the anchor, so nothing else closes
    /// the list on this press.
    fn press(&self, window: &mut Window, cx: &mut App) {
        let open = self.open_layer(window, cx);
        self.state
            .update(cx, |state, _| state.closed_by_press = open.is_some());
        if let Some(layer) = open {
            layers::close(layer, window, cx);
        }
    }

    /// A pointer click on the closed control: it opens the list, unless the click's own press just closed it.
    fn click(&self, window: &mut Window, cx: &mut App) {
        let closed_by_press = self
            .state
            .update(cx, |state, _| std::mem::take(&mut state.closed_by_press));
        if !closed_by_press {
            self.open(window, cx);
        }
    }

    /// Open the list below the control, focus on the chosen option, unless it is open already or there is nothing to
    /// list.
    fn open(&self, window: &mut Window, cx: &mut App) {
        if self.open_layer(window, cx).is_some() {
            return;
        }
        let mark = Mark::take(cx);
        let list = cx.new(|cx| SelectList::new(self.choices.clone(), cx));
        // A select with no options opens nothing, as there is nowhere for focus to go.
        let Some(start) = list.read(cx).start() else {
            return;
        };
        let container = cx.focus_handle();
        let opened = layers::open_popover(
            Popover {
                view: list.clone().into(),
                container,
                start: Some(start),
                anchor: self.anchor.clone(),
            },
            &mark,
            window,
            cx,
        );
        if let Some(layer) = opened {
            list.update(cx, |list, _| list.layer = Some(layer));
            self.state
                .update(cx, |state, _| state.open = Some((layer, list.downgrade())));
        }
    }
}

/// The open list: a menu panel of the options, one [`MenuItem::choice`] each.
struct SelectList {
    choices: Choices,
    /// Each option's focus handle, by value: the arrow keys and the pointer move focus among them.
    handles: HashMap<SharedString, FocusHandle>,
    /// The layer it is drawn in, so a choice can close it.
    layer: Option<LayerId>,
}

impl SelectList {
    fn new(choices: Choices, cx: &mut Context<Self>) -> Self {
        let handles = choices
            .entries
            .iter()
            .map(|entry| (entry.value.clone(), cx.focus_handle()))
            .collect();
        SelectList {
            choices,
            handles,
            layer: None,
        }
    }

    /// Where focus starts: the chosen option, or the first that can be chosen when the chosen one cannot (or nothing
    /// is chosen), or the first of all when none can. `None` only for a list of no options.
    fn start(&self) -> Option<FocusHandle> {
        let available = |entry: &&SelectOption| entry.unavailable.is_none();
        let chosen = self
            .choices
            .chosen
            .as_ref()
            .and_then(|chosen| self.choices.entry(chosen))
            .filter(available);
        let entry = chosen
            .or_else(|| self.choices.entries.iter().find(available))
            .or_else(|| self.choices.entries.first());
        entry
            .and_then(|entry| self.handles.get(&entry.value))
            .map(|handle| handle.clone().tab_stop(true))
    }
}

impl Render for SelectList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let list = cx.weak_entity();
        let Choices {
            id,
            label,
            entries,
            chosen,
            ..
        } = self.choices.clone();
        let items: Vec<MenuItem> = entries
            .into_iter()
            .map(|entry| {
                let handle = self
                    .handles
                    .entry(entry.value.clone())
                    .or_insert_with(|| cx.focus_handle())
                    .clone()
                    // An option that cannot be chosen is no stop for the arrow keys, as a native menu passed over it.
                    .tab_stop(entry.unavailable.is_none());
                let is_chosen = chosen.as_ref() == Some(&entry.value);
                let (list, value) = (list.clone(), entry.value.clone());
                MenuItem::choice(
                    format!("{id}-option-{}", entry.value),
                    entry.label,
                    entry.help,
                    Role::MenuListOption,
                    is_chosen,
                )
                .disabled(entry.unavailable.is_some())
                .when_some(entry.unavailable, |item, reason| item.suffix(reason))
                .focus(&handle)
                .on_click(move |_, window, cx| choose(&list, &value, window, cx))
            })
            .collect();
        layers::panel(cx)
            .id(ElementId::Name(format!("{id}-list").into()))
            .debug_selector(move || format!("{id}-list"))
            .role(Role::MenuListPopup)
            .aria_label(label)
            .children(items)
    }
}

/// The user chose `value` in `list`: close the list, which returns focus to the closed control, then tell the owner,
/// unless `value` is the chosen one already or cannot be chosen.
fn choose(list: &WeakEntity<SelectList>, value: &SharedString, window: &mut Window, cx: &mut App) {
    let Some(list) = list.upgrade() else {
        return;
    };
    let (layer, choices) = {
        let list = list.read(cx);
        (list.layer, list.choices.clone())
    };
    if choices
        .entry(value)
        .is_none_or(|entry| entry.unavailable.is_some())
    {
        return;
    }
    if let Some(layer) = layer {
        layers::close(layer, window, cx);
    }
    if choices.chosen.as_ref() != Some(value)
        && let Some(on_choose) = &choices.on_choose
    {
        on_choose(value, window, cx);
    }
}

#[cfg(test)]
mod key_tests {
    use super::*;

    /// Why this matters: the closed control's keys are the native pop-up's, and Enter in particular must reach the
    /// dialog around the select rather than open it, or the launcher's Enter could never run Launch from a select.
    /// Specified: Space, Up, Down, Alt+Down and Alt+Up open the list; Enter, Escape, a letter, a modified Space or Down,
    /// and any held key go on.
    #[test]
    fn the_closed_control_opens_on_space_and_the_arrows_and_passes_enter_on() {
        let key =
            |chord: &'static str, held: bool| closed_key(&Keystroke::parse(chord).unwrap(), held);
        for chord in ["space", "up", "down", "alt-down", "alt-up"] {
            assert_eq!(key(chord, false), ClosedKey::Open, "{chord} opens");
        }
        for chord in [
            "enter",
            "escape",
            "a",
            "shift-space",
            "ctrl-down",
            "shift-down",
            "tab",
        ] {
            assert_eq!(key(chord, false), ClosedKey::PassOn, "{chord} goes on");
        }
        assert_eq!(key("down", true), ClosedKey::PassOn, "a held key goes on");
    }
}
