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
//!   option, wrapping at the ends as WebKit's radios do, and Space chooses the focused option. Home and End move
//!   neither focus nor the choice, as in the old app: its radio groups were native radios, which in WebKit and Chromium
//!   handle only the arrows, or `role="radio"` buttons with no keys of their own (below), so Home and End did nothing
//!   in any of them. With nothing chosen, Tab enters at the first available option and Shift+Tab at the last, as in
//!   WebKit. Options report the radio button role and whether they are checked. The old Credentials editor's
//!   `role="radio"` buttons (Claude Code or Codex) were plain buttons that only announced themselves as radios, with
//!   each one a tab stop and no arrows; under this role they would get the native keyboard their role promised when the
//!   Credentials editor is ported, and that port confirms it.
//! - **[`Role::Group`] behaves as the old buttons with `aria-pressed`.** Each available option is a tab stop of its
//!   own, Enter or Space chooses it, the arrows do nothing, and options report the button role with their pressed state
//!   (toggled, to assistive technology).
//! - **[`Role::GenericContainer`] is the same pressed buttons with no group around them.** The launcher's harness row
//!   was a plain `div` of `aria-pressed` buttons: no `role="group"`, no accessible name, only a visible "Harness" word
//!   beside it that nothing tied to the buttons. Its keys were the button group's (each a tab stop, Enter applies the
//!   choice and launches, Space and a click choose), so this role keeps that keyboard and differs only in what
//!   assistive technology sees: nothing at all for the container, which takes no role, name or description, as the
//!   old `div` had none (GPUI's accessibility tree drops generic containers, so the role is never set). The label then
//!   names the control only in code; the help still shows on hover between the options.
//!
//! Enter follows the choice controls' shared Enter rule ([`super::enter_choice`], the old launcher's `enter_choice`),
//! through [`RadioGroup::on_enter`]. Without it, Enter on a button option chooses it, and Enter on a radio goes on to
//! whatever holds the group, as a native radio's did. With it (the owner's main action, a launch), Enter on a button
//! option applies that option and then runs the action, and Enter on a radio runs the action with the values shown,
//! unchanged (SPEC.md's launcher: "Enter on a choice button ... applies that choice and then launches"; "Enter on ...
//! YOLO radios ... launches with the values shown"). A held Enter does nothing there, so holding it never launches
//! twice. A modified Enter (Shift+Enter) is not an activation and goes on to the owner, as the shared rule says.
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
//! Five looks, one per old stylesheet rule, picked by the owner and independent of the role's keyboard (except that
//! plain radio circles promise the radio keyboard, so [`RadioGroup::radios`] demands it):
//!
//! - **Segments** (the default): `.segmented-control`, the options side by side in one rounded well, the chosen one in
//!   the accent fill.
//! - **Small segments** ([`RadioGroup::small_segments`]): the session list's status filter
//!   (`.session-status-filter .segmented-control`), the same well with shorter, narrower segments of 12-pixel words,
//!   all of one width, and the well growing to fill its row.
//! - **Plain radios** ([`RadioGroup::radios`]): the launcher's YOLO fieldset, a circle before each option's words.
//! - **Chips** ([`RadioGroup::chips`]): the launcher's harness row (`.launch-composer-options`), separate bordered
//!   buttons on the well color, wrapping with an 8-pixel gap, and no well around them.
//! - **Cards** ([`RadioGroup::cards`]): the recipe editor's Software choice (`.recipe-source-cards`), bordered cards
//!   on the well color in a three-column grid, each with its title over its description, the chosen one edged in the
//!   accent.
//!
//! An option can carry more than its words, each ported from one old use:
//!
//! - **A glyph** ([`RadioOption::icon`], [`RadioOption::icon_only`]): the launcher's destination type drew a folder or
//!   a branch before "Folder" and "Managed checkout". The glyph says nothing to assistive technology; the option's
//!   label stays its accessible name, with or without words beside the glyph.
//! - **Danger ink** ([`RadioOption::danger`]): the launcher's YOLO permission segment kept its words red while not
//!   chosen (`.launch-composer-segment-danger`), and took the chosen segment's inverse ink once chosen, because red on
//!   the accent fill cannot carry the warning. Segments and chips draw it; plain radios ignore it, because the only old
//!   plain radios (the launcher's "Runs in YOLO mode" fieldset) drew "Yes (YOLO)" in the form's ordinary label ink, so
//!   the YOLO fieldset's port can mark the option without turning words red that the old app never did.
//! - **A description line** ([`RadioOption::description`]): Settings' terminal link choices showed what each choice
//!   does under its label (`.terminal-link-description`). The visible line is the option's accessible description,
//!   as the old input's `aria-describedby` pointed at it, so a screen reader reads what a sighted user sees under the
//!   label; the option's hover help is then the tooltip alone, so the two are not both announced when they repeat each
//!   other, as the old choices' did. A group with any described option stacks its options, one per line, as the old
//!   `.terminal-link-choices` did, since a description beside another option's would not fit.

use std::rc::Rc;

use gpui::{
    App, ElementId, FocusHandle, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Toggled,
    Window, div, prelude::FluentBuilder as _, px, svg,
};

use super::{HoverHelp, MainAction};
use crate::assets::Icon;
use crate::focus;
use crate::theme::{self, Palette, Token, metrics};

/// The plain radio's circle, as large as the checkbox's box so the launcher's native controls match, and the dot a
/// chosen one holds.
const CIRCLE: f32 = 14.;
const DOT: f32 = 6.;

/// `.segmented-control > button`: `min-height: 30px` and `padding: 4px 10px`, at `line-height: 1.4` of 13 pixels.
const SEGMENT_MIN_HEIGHT: f32 = 30.;
const SEGMENT_PAD: (f32, f32) = (4., 10.);
const SEGMENT_LINE_HEIGHT: f32 = 18.2;

/// `.session-status-filter .segmented-control > .segment-radio > span`: `min-height: 28px` and `padding-inline: 5px` at
/// 12 pixels, keeping the segment's 4-pixel `padding-block` and its `line-height: 1.4`. The well around them is
/// `flex: 1 1 220px`: it starts at 220 pixels and shares out whatever its row has left.
const SMALL_SEGMENT_MIN_HEIGHT: f32 = 28.;
const SMALL_SEGMENT_PAD_X: f32 = 5.;
const SMALL_SEGMENT_LINE_HEIGHT: f32 = 16.8;
const SMALL_SEGMENTS_BASIS: f32 = 220.;

/// The harness row's chip as the launcher drew it. The base `.launch-composer-options > button` rule says 38 pixels
/// high, 14-pixel side padding and 14-pixel semibold words, but the launcher form's own override
/// (`.create-session-form .launch-composer-harness-choice .launch-composer-options > button`) made every chip anyone
/// saw 36 pixels high with 12-pixel side padding and 13-pixel regular words; the override is what is ported. The
/// 1.5-pixel edge and the 8-pixel gap between chips are the base rule's.
const CHIP_MIN_HEIGHT: f32 = 36.;
const CHIP_PAD_X: f32 = 12.;
const CHIP_EDGE: f32 = 1.5;
const CHIP_GAP: f32 = 8.;

/// The destination type's glyphs (`.destination-folder-glyph`, `.managed-checkout-glyph`) and their gap to the words
/// (`.segmented-control > button`'s `gap: 6px`).
const GLYPH: f32 = 12.;
const GLYPH_GAP: f32 = 6.;

/// A plain radio's gap between its circle and its words (`.create-session-form .launch-command-yolo > label`'s
/// `gap: 6px`). Equal to [`GLYPH_GAP`] by coincidence; the two come from different old rules.
const RADIO_GAP: f32 = 6.;

/// `.recipe-source-cards`: three equal columns 8 pixels apart; `.recipe-source-card`: 10 pixels of padding and 4 between
/// its title and its description, inside the chip's 1.5-pixel edge.
const CARD_COLUMNS: u16 = 3;
const CARD_GAP: f32 = 8.;
const CARD_PAD: f32 = 10.;
const CARD_INNER_GAP: f32 = 4.;

/// `.terminal-link-description`: 12-pixel words at the segment's `line-height: 1.4`, 3 pixels under the label.
const DESCRIPTION_LINE_HEIGHT: f32 = 16.8;
const DESCRIPTION_GAP: f32 = 3.;

/// What a choice does with the chosen option's value.
type OnChange<V> = Rc<dyn Fn(&V, &mut Window, &mut App)>;

/// One option of a [`RadioGroup`]: its words (drawn, and its accessible name), its hover help, and the value its owner
/// hears when it is chosen.
pub struct RadioOption<V> {
    label: SharedString,
    help: HoverHelp,
    value: V,
    disabled: bool,
    glyph: Option<Glyph>,
    danger: bool,
    description: Option<SharedString>,
}

/// An option's glyph, and whether its words are drawn beside it or only stand behind it as its accessible name.
#[derive(Clone, Copy)]
struct Glyph {
    icon: Icon,
    words: bool,
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
            glyph: None,
            danger: false,
            description: None,
        }
    }

    /// An unavailable option: drawn dimmed (a segment or chip whole, a plain radio's circle only, as a native radio's
    /// words stayed as they were), not focusable, and skipped by Tab and the arrows; a click does nothing.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Draw `icon` before the option's words, in the words' ink (the destination type's folder and branch). The label
    /// stays the accessible name.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.glyph = Some(Glyph { icon, words: true });
        self
    }

    /// Draw `icon` alone, without the words. The label is still the accessible name, since a glyph says nothing to a
    /// screen reader, and the hover help is what a sighted user has to tell such options apart, so it should name the
    /// choice.
    pub fn icon_only(mut self, icon: Icon) -> Self {
        self.glyph = Some(Glyph { icon, words: false });
        self
    }

    /// A choice that warrants a warning (the launcher's YOLO permission mode): in segments and chips, its words and
    /// glyph in the danger ink wherever they are not on the chosen option's accent fill. Plain radios ignore it, as the
    /// old YOLO fieldset's radio was not red. See the module docs.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    /// A line under the label saying what the choice does (Settings' terminal link choices). It is the option's
    /// accessible description, and its presence stacks the group's options; see the module docs for both.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
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
    look: Look,
    on_change: Option<OnChange<V>>,
    on_enter: Option<MainAction>,
}

/// How a group draws its options; one at a time, so a single field rather than a flag per look. See the module docs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Segments,
    SmallSegments,
    Radios,
    Chips,
    Cards,
}

impl<V: Clone + PartialEq + 'static> RadioGroup<V> {
    /// A group labelled `label` (its accessible name; any visible legend is its owner's to draw) with hover help
    /// `help` (its accessible description, and shown when the pointer reaches the group's gaps or edge from outside
    /// it). `id` must be stable and unique among its siblings; each option is drawn under `<id>-<its label>`, which is
    /// how tests and the test bridge find it. `role` picks the keyboard: [`Role::RadioGroup`] for native radios,
    /// [`Role::Group`] for the old segmented buttons, and [`Role::GenericContainer`] for those buttons with no group
    /// around them (the module docs). Any other role is a mistake, caught in debug builds. Drawn as the segmented well
    /// unless [`RadioGroup::radios`] or [`RadioGroup::chips`] asks for another look.
    ///
    /// A generic container has no accessibility node, so its `label` reaches no reader and its `help` is no accessible
    /// description; the help still shows on hover between the options. It takes both anyway because every control's
    /// constructor requires a label and hover help (SPEC_impl.md, "The component layer and theme"), and a
    /// [`Role::Group`] port of the same row, should one be wanted, needs them.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
    ) -> Self {
        debug_assert!(
            matches!(
                role,
                Role::RadioGroup | Role::Group | Role::GenericContainer
            ),
            "a radio group's role picks its keyboard, so it must be a radio group, a group or a generic container, \
             not {role:?}"
        );
        RadioGroup {
            id: id.into(),
            label: label.into(),
            help,
            role,
            chosen: None,
            options: Vec::new(),
            disabled: false,
            look: Look::Segments,
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
        self.look = Look::Radios;
        self
    }

    /// Draw the options as the launcher's harness row drew its buttons: separate bordered chips that hug their words
    /// and wrap, with no well around them (module docs). The look says nothing about the keyboard; the harness row
    /// pairs it with [`Role::GenericContainer`], as its old buttons sat in no group.
    pub fn chips(mut self) -> Self {
        self.look = Look::Chips;
        self
    }

    /// Draw the options as the session list's status filter drew its segments: the segmented well, with each segment
    /// 28 pixels high, 5 pixels in from its sides and in 12-pixel words, every segment the same width however long its
    /// words (`flex: 1 1 0`), and the well itself growing from 220 pixels to fill the row it sits in, or shrinking
    /// below that when the row is narrower (`flex: 1 1 220px`). The old status filter fitted its four words into the
    /// sidebar's narrowest list column this way. The look says nothing about the keyboard; the status filter pairs it
    /// with [`Role::RadioGroup`], as its old inputs were native radios.
    pub fn small_segments(mut self) -> Self {
        self.look = Look::SmallSegments;
        self
    }

    /// Draw the options as the recipe editor's Software choice drew them (`.recipe-source-cards`): a card per option in a
    /// grid of three equal columns, its label as a semibold title over its description line, on the well color inside
    /// a 1.5-pixel edge that turns the accent on the chosen card (module docs). The old cards were buttons with
    /// `aria-pressed` in a `role="group"`, so the recipe editor pairs this look with [`Role::Group`]; the look itself
    /// says nothing about the keyboard. A described option's line is its accessible description, as in every look.
    pub fn cards(mut self) -> Self {
        self.look = Look::Cards;
        self
    }

    /// What a choice does: called with the chosen option's value.
    pub fn on_change(mut self, handler: impl Fn(&V, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// The owner's main action, which Enter on a focused option runs (after choosing it, in a button group), under the
    /// choice controls' shared Enter rule (`components::enter_choice`, which says what the action must guard itself);
    /// see the module docs.
    ///
    /// In a button group the action runs right after [`RadioGroup::on_change`] and reads what it wrote, so
    /// `on_change` must change the owner's state directly: a choice sent through `window.dispatch_action` runs only
    /// after the key handler, and the action would see the previous choice.
    ///
    /// It covers the whole group, every option alike. An option whose Enter must only choose (SPEC.md's Custom
    /// permission mode, which selects the mode and focuses its flags editor instead of launching) is the owner's to
    /// handle: its action checks the choice it runs with. A per-option opt-out comes with the first consumer that
    /// needs it.
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
    on_enter: Option<MainAction>,
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
        // Enter with the owner's main action: a button applies its option first, while a radio's Enter chooses nothing,
        // so the action runs with the values shown.
        let apply = |window: &mut Window, cx: &mut App| {
            if !self.radio {
                self.choose(index, window, cx);
            }
        };
        if super::enter_choice(event, self.on_enter.as_ref(), apply, window, cx) {
            return;
        }
        // A modified key is not an activation, the controls' shared key rule.
        if event.keystroke.modifiers.modified() {
            return;
        }
        match event.keystroke.key.as_str() {
            "space" if !event.is_held => {
                cx.stop_propagation();
                self.choose(index, window, cx);
            }
            // Without the owner's action, Enter chooses a button, as a native button's did, and a radio's goes on.
            "enter" if !self.radio && !event.is_held => {
                cx.stop_propagation();
                self.choose(index, window, cx);
            }
            "down" | "right" if self.radio => {
                cx.stop_propagation();
                self.step(index, true, window, cx);
            }
            "up" | "left" if self.radio => {
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
        // Described options stack, one per line across the group's width (`.terminal-link-choices`); otherwise the
        // options sit in a row that wraps.
        let stacked = self
            .options
            .iter()
            .any(|option| option.description.is_some());
        let mut options = Vec::with_capacity(self.options.len());
        for (index, option) in self.options.iter().enumerate() {
            let state = OptionState {
                chosen: chosen == Some(index),
                enabled: enabled[index],
                ring: focus::shows_ring(&handles[index], window),
                stacked,
            };
            let element = match self.look {
                Look::Segments | Look::SmallSegments => {
                    state.segment(&ids[index], option, self.look, palette)
                }
                Look::Radios => state.radio(&ids[index], option, palette),
                Look::Chips => state.chip(&ids[index], option, palette),
                Look::Cards => state.card(&ids[index], option, palette),
            };
            let element = element
                .role(option_role)
                .aria_label(option.label.clone())
                .aria_toggled(if state.chosen {
                    Toggled::True
                } else {
                    Toggled::False
                })
                .when_some(option.description.clone(), |element, description| {
                    element.aria_description(description)
                });
            // A described option's visible line is its accessible description (module docs), so its help is the
            // tooltip alone; any other option's help is its description too, as every control's is.
            let describe = option.description.is_none();
            let element = super::with_hover_help(
                element,
                &ids[index],
                &option.help,
                describe,
                Some(&handles[index]),
                window,
                cx,
            );
            if !state.enabled {
                // Not tracked, so neither Tab nor a click can focus it; a press of any mouse button is still this
                // control's, so it does not reach the page's empty-space handler and move focus.
                options.push(
                    element
                        .on_any_mouse_down(|_, window, _| window.prevent_default())
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
        // A generic container is the old harness row's plain `div`: no role, no name, no description (module docs).
        // GPUI refuses the generic role itself, since the accessibility tree filters it out, so it is left unset.
        let named = self.role != Role::GenericContainer;
        let group = div()
            .id(ElementId::Name(self.id.clone()))
            // Tests find a control by its id; this records nothing outside test builds.
            .debug_selector(|| self.id.to_string())
            .when(named, |group| {
                group.role(self.role).aria_label(self.label.clone())
            })
            .flex()
            .map(|group| {
                if stacked {
                    group.flex_col().items_stretch()
                } else {
                    group.flex_wrap().items_center()
                }
            });
        let group = match self.look {
            Look::Radios => group.gap_x(px(16.)).gap_y(px(8.)),
            Look::Chips => group.gap(px(CHIP_GAP)),
            // A grid in place of the row or the stack: every card a third of the width, and a row of cards as tall as
            // its tallest, as the old grid's stretched items were.
            Look::Cards => group
                .grid()
                .grid_cols(CARD_COLUMNS)
                .items_stretch()
                .gap(px(CARD_GAP)),
            // `.segmented-control`: the options in a well with a 2-pixel gap and padding inside a dim edge.
            Look::Segments | Look::SmallSegments => group
                .gap(px(2.))
                .p(px(2.))
                .border_1()
                .border_color(theme::color(palette.border_dim))
                .rounded(metrics::RADIUS_MD)
                .bg(theme::color(palette.well))
                // The status filter's well fills its row from a 220-pixel start (`flex: 1 1 220px; min-width: 0`).
                .when(self.look == Look::SmallSegments, |group| {
                    group
                        .flex_grow_1()
                        .flex_shrink_1()
                        .flex_basis(px(SMALL_SEGMENTS_BASIS))
                        .min_w_0()
                }),
        };
        // The group's help shows over its gaps and edge, and an option's over the option: the layer manager shows the
        // innermost control's help (`with_hover_help`). It is also the group's accessible description, as every
        // control's is, except for the generic container, which takes no description (module docs).
        let group = super::with_hover_help(group, &self.id, &self.help, named, None, window, cx);
        group.children(options)
    }
}

/// How one option is drawn: whether it is chosen, available, showing the keyboard focus ring, and in a group that
/// stacks its options (so its contents start at the left edge rather than centered).
#[derive(Clone, Copy)]
struct OptionState {
    chosen: bool,
    enabled: bool,
    ring: bool,
    stacked: bool,
}

impl OptionState {
    /// The ink of an option's words and glyph in a look that fills the chosen option with the accent: the inverse ink
    /// on the fill, the danger ink for an unchosen danger option, and `ink` otherwise.
    fn filled_ink<V>(self, option: &RadioOption<V>, ink: Token, palette: &Palette) -> Token {
        if self.chosen {
            palette.on_fill
        } else if option.danger {
            palette.danger
        } else {
            ink
        }
    }

    /// The description line's ink in a look that fills the chosen option: `--on-accent-dim` on the fill (the old
    /// `.segment-radio > input:checked + span .terminal-link-description`), the secondary ink elsewhere.
    fn filled_description_ink(self, palette: &Palette) -> Token {
        if self.chosen {
            palette.on_accent_dim
        } else {
            palette.fg_1
        }
    }

    /// One segment of `.segmented-control`: transparent with the control hover fill, or the accent fill with the
    /// inverse ink and semibold words when chosen, one step up the accent family under the pointer so hovering the
    /// choice does not hide it. Its focus ring is inset (the old `outline-offset: -3px`), in the inverse ink on the
    /// chosen segment, so it stays visible inside the rounded well. Under [`Look::SmallSegments`] it is the status
    /// filter's ([`RadioGroup::small_segments`]): shorter and narrower, in smaller words, and as wide as every other
    /// segment.
    fn segment<V>(
        self,
        id: &SharedString,
        option: &RadioOption<V>,
        look: Look,
        palette: &Palette,
    ) -> gpui::Stateful<gpui::Div> {
        let small = look == Look::SmallSegments;
        let (fill, hover, ring) = if self.chosen {
            (
                palette.accent_fill,
                palette.accent_fill_hover,
                palette.on_fill,
            )
        } else {
            (theme::TRANSPARENT, palette.control_hover, palette.accent)
        };
        let ink = self.filled_ink(option, palette.fg_0, palette);
        let (min_height, pad_x, text_size, line_height) = if small {
            (
                SMALL_SEGMENT_MIN_HEIGHT,
                SMALL_SEGMENT_PAD_X,
                metrics::TEXT_SM,
                SMALL_SEGMENT_LINE_HEIGHT,
            )
        } else {
            (
                SEGMENT_MIN_HEIGHT,
                SEGMENT_PAD.1,
                metrics::TEXT_MD,
                SEGMENT_LINE_HEIGHT,
            )
        };
        div()
            .id(ElementId::Name(id.clone()))
            .debug_selector(|| id.to_string())
            .relative()
            .flex()
            // Small segments share the well equally whatever their words (`flex: 1 1 0; min-width: 0`); the others
            // grow from their words' width and never shrink below it.
            .map(|segment| {
                if small {
                    segment.flex_1().min_w_0()
                } else {
                    segment.flex_grow(1.).flex_shrink_0()
                }
            })
            .items_center()
            // `.terminal-link-choices > .segment-radio > span` lines a stacked option's words up at its left edge.
            .map(|segment| {
                if self.stacked {
                    segment.justify_start()
                } else {
                    segment.justify_center()
                }
            })
            .gap(px(GLYPH_GAP))
            .min_h(px(min_height))
            .py(px(SEGMENT_PAD.0))
            .px(px(pad_x))
            .rounded(metrics::RADIUS_SM)
            .bg(theme::color(fill))
            .text_size(text_size)
            .line_height(px(line_height))
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
            .children(contents(option, ink, self.filled_description_ink(palette)))
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

    /// One chip of the launcher's harness row (`.launch-composer-options > button`, at the launcher form's size; see
    /// [`CHIP_MIN_HEIGHT`]): the well color inside a 1.5-pixel edge, or the accent fill with the inverse ink when
    /// chosen, the words regular weight either way. The old chips had no hover fill, no pointer cursor (no rule set
    /// one, unlike the segments' `cursor: pointer`) and no disabled look of their own (the stylesheet's colors overrode
    /// the browser's disabled ink); this one keeps the first two and dims an unavailable chip as every other look does,
    /// so an unavailable choice never looks available. Its focus ring is the page's
    /// (2 pixels, 1 pixel clear of the edge, in the accent) and, on the chosen chip, the inverse ink inset 3 pixels
    /// (the old `.launch-composer-options > button.selected:focus-visible`).
    fn chip<V>(
        self,
        id: &SharedString,
        option: &RadioOption<V>,
        palette: &Palette,
    ) -> gpui::Stateful<gpui::Div> {
        let fill = if self.chosen {
            palette.accent_fill
        } else {
            palette.well
        };
        let ink = self.filled_ink(option, palette.fg_0, palette);
        // Absolute children are placed inside the edge, so the old ring's offsets, which are measured from the edge's
        // outer side, are moved out by its width.
        let (ring, inset, radius) = if self.chosen {
            (palette.on_fill, 1. - CHIP_EDGE, metrics::RADIUS_SM - px(1.))
        } else {
            (
                palette.accent,
                -(CHIP_EDGE + 3.),
                metrics::RADIUS_SM + px(3.),
            )
        };
        div()
            .id(ElementId::Name(id.clone()))
            .debug_selector(|| id.to_string())
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(GLYPH_GAP))
            .min_h(px(CHIP_MIN_HEIGHT))
            .px(px(CHIP_PAD_X))
            .border(px(CHIP_EDGE))
            .border_color(theme::color(palette.border))
            .rounded(metrics::RADIUS_SM)
            .bg(theme::color(fill))
            .text_size(metrics::TEXT_MD)
            .line_height(px(SEGMENT_LINE_HEIGHT))
            .text_color(theme::color(ink))
            .when(!self.enabled, |chip| chip.opacity(0.5))
            .children(contents(option, ink, self.filled_description_ink(palette)))
            .when(self.ring, |chip| {
                chip.child(
                    div()
                        .absolute()
                        .top(px(inset))
                        .left(px(inset))
                        .right(px(inset))
                        .bottom(px(inset))
                        .border_2()
                        .border_color(theme::color(ring))
                        .rounded(radius),
                )
            })
    }

    /// One card of the recipe editor's Software choice (`.recipe-source-card`): the label as a 13-pixel semibold title
    /// over the description in the secondary ink at 12 pixels, left-aligned, on the well color inside the chip's
    /// 1.5-pixel edge, which is the accent on the chosen card. The old card had no hover fill, and its disabled look was
    /// only the default cursor; this one dims an unavailable card as every other look does, so an unavailable choice
    /// never looks available. Its focus ring is the page's, outside the edge, as the unchosen chip's.
    fn card<V>(
        self,
        id: &SharedString,
        option: &RadioOption<V>,
        palette: &Palette,
    ) -> gpui::Stateful<gpui::Div> {
        let edge = if self.chosen {
            palette.accent
        } else {
            palette.border
        };
        let inset = -(CHIP_EDGE + 3.);
        div()
            .id(ElementId::Name(id.clone()))
            .debug_selector(|| id.to_string())
            .relative()
            .flex()
            .flex_col()
            .gap(px(CARD_INNER_GAP))
            .min_w_0()
            .p(px(CARD_PAD))
            .border(px(CHIP_EDGE))
            .border_color(theme::color(edge))
            .rounded(metrics::RADIUS_SM)
            .bg(theme::color(palette.well))
            .text_color(theme::color(palette.fg_0))
            .when(self.enabled, |card| card.cursor_pointer())
            .when(!self.enabled, |card| card.opacity(0.5))
            .child(
                div()
                    .text_size(metrics::TEXT_MD)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(option.label.clone()),
            )
            .children(option.description.clone().map(|description| {
                div()
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.fg_1))
                    .child(description)
            }))
            .when(self.ring, |card| {
                card.child(
                    div()
                        .absolute()
                        .top(px(inset))
                        .left(px(inset))
                        .right(px(inset))
                        .bottom(px(inset))
                        .border_2()
                        .border_color(theme::color(palette.accent))
                        .rounded(metrics::RADIUS_SM + px(3.)),
                )
            })
    }

    /// One plain radio, as the launcher's YOLO fieldset drew it: the platform's circle tinted with the accent (the old
    /// page's `accent-color`), filled with a dot in the inverse ink when chosen, then the words in the form's 12-pixel
    /// secondary ink (`.create-session-form label`), a danger option's too, as the old YOLO radio's were. The words are
    /// part of the control, as the old `<label>` around the input made them, so a click on them chooses too. Its focus
    /// ring sits around the circle, as the old outline did.
    fn radio<V>(
        self,
        id: &SharedString,
        option: &RadioOption<V>,
        palette: &Palette,
    ) -> gpui::Stateful<gpui::Div> {
        let (fill, edge) = if self.chosen {
            (palette.accent_fill, palette.accent_edge)
        } else {
            (palette.well, palette.border)
        };
        let ink = palette.fg_1;
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
            .gap(px(RADIO_GAP))
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(ink))
            .when(self.enabled, |radio| radio.cursor_pointer())
            .child(circle)
            .children(contents(option, ink, palette.fg_1))
    }
}

/// What an option shows after any circle: its glyph, then its words, then its description line under them. A plain
/// option's label is a direct text child, as it was before glyphs and descriptions existed, so those options draw
/// exactly as they did. The glyph takes `ink`, the words' ink; the description takes `description_ink` at its own
/// regular weight, so a chosen segment's semibold stays on the label.
fn contents<V>(
    option: &RadioOption<V>,
    ink: Token,
    description_ink: Token,
) -> Vec<gpui::AnyElement> {
    let mut children = Vec::with_capacity(2);
    let words = option.glyph.is_none_or(|glyph| glyph.words);
    if let Some(glyph) = option.glyph {
        children.push(
            svg()
                .path(glyph.icon.path())
                .size(px(GLYPH))
                .flex_none()
                .text_color(theme::color(ink))
                .into_any_element(),
        );
    }
    match &option.description {
        None if words => children.push(option.label.clone().into_any_element()),
        None => {}
        Some(description) => children.push(
            div()
                .flex()
                .flex_col()
                .items_start()
                .flex_1()
                .min_w_0()
                .gap(px(DESCRIPTION_GAP))
                .when(words, |column| column.child(option.label.clone()))
                .child(
                    div()
                        .text_size(metrics::TEXT_SM)
                        .line_height(px(DESCRIPTION_LINE_HEIGHT))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(theme::color(description_ink))
                        .child(description.clone()),
                )
                .into_any_element(),
        ),
    }
    children
}
