//! A menu's command: an icon, a label and a one-line description, drawn as the old stylesheet's `.menu-item` inside a
//! session row's menu.
//!
//! Ported from the old app's `list/row.rs` (each command's `button` with `MenuActionIcon` and `MenuItemCopy`) and the
//! stylesheet's `.session-row-menu-item`, `.menu-item`, `.menu-icon`, `.menu-copy`, `.menu-description` and
//! `.menu-danger` rules at the reference commit. Here's the TLDR of how it behaves:
//!
//! - **The highlight tracks focus.** Inside a menu, focus paints the accent fill, and the pointer moves focus onto the
//!   item it moves over ([`focus::highlight`]), so the pointer and the arrow keys share one highlight (SPEC.md,
//!   "Things changing underneath": "A menu has one highlight"). There is no separate hover fill: a resting pointer
//!   would otherwise leave a second fill behind after an arrow-key move, which the old sheet suppressed for the same
//!   reason. The one exception is an expanded submenu trigger, which keeps the accent fill while its submenu is open
//!   so the user sees which item the submenu belongs to. Only actual focus draws the keyboard ring or receives a key.
//! - **A danger item stays a command.** Its label and icon rest in `--danger`, and once highlighted it takes the same
//!   accent fill and inverse text as every other item: the wording, the icon and its group say it is destructive.
//! - **Disabled is still focusable.** The old menus marked a busy item `aria-disabled` rather than disabling it,
//!   because a browser cannot focus a disabled control and a menu whose only item went busy under the user's focus
//!   would have nowhere for its keys to go. This item does the same: drawn at half opacity with no fill, still a tab
//!   stop, and activation does nothing.
//! - **Help.** Its tooltip sits beside the menu panel when there is room, keeping the other choices visible. The visible description
//!   line is its accessible description, as the old item's `aria-describedby` made it. SPEC.md lets an item that shows
//!   a description go without hover text; the component layer has no form of [`HoverHelp`] that stands for a visible
//!   description yet, so every item carries a tooltip too.
//! - **Shortcut hints.** A shortcut for the same command appears beside the label, at the far edge where a submenu
//!   trigger would put its arrow. The item also exposes the chord to assistive technology.
//!
//! Activation follows [`super::Button`]: a click, or Enter or Space while the item has focus (never a held key), and
//! the focus coordinator's click rule for whether a press moves focus.
//!
//! A choice in a select's list is the same item made with [`MenuItem::choice`]: a tick in the icon's place on the
//! chosen one and an empty slot of the same size on the others, as the platform's pop-up menu marked the chosen
//! option, and its label kept to one line and cut with an ellipsis, since the list is exactly as wide as its control
//! ([`crate::layers::Anchor::matching_width`]) and a native menu never wrapped an option. An unavailable choice says
//! why after its label ([`MenuItem::suffix`]), as the old app's disabled `<option>`s did (" · " and the reason), and
//! may carry a mark of its own after the tick's slot ([`MenuItem::mark`]), as a select's marked option does.
//!
//! A command that is one of a set of choices in force (the session list's orders, a `menuitemradio` in the old app)
//! says whether it is the one in force with [`MenuItem::checked`]: it keeps its icon and its description, reports
//! itself checked or not, and the checked one draws " ✓" after its label, as the old sheet's
//! `.sort-menu-item[aria-checked="true"] .menu-label::after` did. That differs from a choice on purpose: a select's
//! list marks the chosen option in the icon's place and reports it selected, while a menu's radio items keep their
//! icons and are reported checked.

use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Toggled, Transformation,
    Window, div, prelude::FluentBuilder as _, px, radians, svg,
};

use super::{Handler, HoverHelp, key_chip};
use crate::assets::Icon;
use crate::focus;
use crate::host_text::{HostSentence, Line};
use crate::layers;
use crate::theme::{self, metrics};

/// `.menu-item`'s padding, vertical then horizontal.
const PAD: (f32, f32) = (6., 7.);
/// `.menu-item`'s horizontal margin inside its panel: the fill is inset rather than full bleed.
const INSET: f32 = 4.;
/// `.menu-item`'s `line-height: 1.4` at the 12-pixel control size.
const LINE_HEIGHT: f32 = 16.8;
/// `.menu-description`'s size.
const DESCRIPTION_SIZE: f32 = 11.;

/// One command in a menu. See the module docs.
#[derive(Clone, IntoElement)]
pub struct MenuItem {
    id: SharedString,
    /// `None` for a choice that is not the chosen one: an empty slot as wide as an icon, so labels line up.
    icon: Option<Icon>,
    /// A choice's own mark, drawn after the icon's slot ([`MenuItem::mark`]).
    mark: Option<Icon>,
    label: SharedString,
    description: Option<Line>,
    /// Words drawn after the label on its line ([`MenuItem::suffix`]).
    suffix: Option<SharedString>,
    shortcut: Option<&'static str>,
    /// Whether this is a choice ([`MenuItem::choice`]), and then whether it is the chosen one.
    chosen: Option<bool>,
    /// Whether this command is one of a set of choices in force ([`MenuItem::checked`]), and then whether it is the
    /// one in force.
    checked: Option<bool>,
    submenu: Option<SubmenuTrigger>,
    help: HoverHelp,
    role: Role,
    danger: bool,
    disabled: bool,
    focus: Option<FocusHandle>,
    on_click: Option<Handler>,
}

/// Pointer entry shows the child menu. Keyboard activation also enters it.
#[derive(Clone)]
struct SubmenuTrigger {
    expanded: bool,
    enter: Handler,
    hover: Handler,
}

impl MenuItem {
    /// A menu command. `id` must be stable and unique among its siblings (it keys the item's element state, and the
    /// test bridge finds the item by it); `label` is its visible text and accessible name; `role` is what assistive
    /// technology and the test bridge see, [`Role::MenuItem`] inside a menu.
    pub fn new(
        id: impl Into<SharedString>,
        icon: Icon,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
    ) -> Self {
        MenuItem {
            id: id.into(),
            icon: Some(icon),
            mark: None,
            label: label.into(),
            description: None,
            suffix: None,
            shortcut: None,
            chosen: None,
            checked: None,
            submenu: None,
            help: help.beside_menu(),
            role,
            danger: false,
            disabled: false,
            focus: None,
            on_click: None,
        }
    }

    /// One choice of a select's list (see the module docs): ticked when `chosen`, with an empty slot in the icon's
    /// place otherwise, its label on one line cut with an ellipsis, and reported selected or not. The arguments are
    /// [`MenuItem::new`]'s; `role` is [`Role::MenuListOption`] in a select's list.
    pub fn choice(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
        chosen: bool,
    ) -> Self {
        let mut item = MenuItem::new(id, Icon::Check, label, help, role);
        item.icon = chosen.then_some(Icon::Check);
        item.chosen = Some(chosen);
        item
    }

    /// The command is one of a set of choices in force (a menu's radio items, [`Role::MenuItemRadio`]), and `checked`
    /// says whether it is the one in force now: reported checked or not to assistive technology, and, when checked,
    /// " ✓" drawn after the label (see the module docs). The icon and the description stay; the accessible name stays
    /// the label alone, without the tick, as the old item's name did. Give an item a check or a [`MenuItem::suffix`],
    /// not both: a suffix takes the tick's place after the label.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Words drawn after the label on the same line, after a middle dot: why an unavailable choice cannot be chosen
    /// ("Podman · Needs setup"), as the old app's disabled options read. They are the item's accessible description
    /// too, so a screen reader hears the reason; give an item a suffix or a [`MenuItem::description`], not both.
    pub fn suffix(mut self, suffix: impl Into<SharedString>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    /// Draw `mark` between the icon's slot and the label, in the icon's ink: a select's option that carries a mark of
    /// its own ([`super::SelectOption::mark`]), whose tick keeps the icon's slot. Decoration only, named by nothing.
    pub fn mark(mut self, mark: Icon) -> Self {
        self.mark = Some(mark);
        self
    }

    /// The line under the label that says what the command does ("Removes the session and its state").
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(Line::App(description.into()));
        self
    }

    /// Advertise the app shortcut for this direct command. The caller supplies the platform label and checks that
    /// the shortcut operates on the same target with the same effect. Submenu triggers reserve this space for an arrow.
    pub(crate) fn shortcut(mut self, shortcut: &'static str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    /// [`MenuItem::description`] for a line that names a host value: the Templates dialog's number-key menu says which
    /// template holds a key ("my-codex (moves here)"), and a template's name is host-supplied text. The sentence is
    /// drawn through the host-text element, and the item's accessible description is its text with each host value
    /// escaped, as for hover help that names one.
    pub fn description_sentence(mut self, description: HostSentence) -> Self {
        self.description = Some(Line::Sentence(description));
        self
    }

    /// Draw it as a destructive command: label and icon in `--danger` while it is not highlighted.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    /// An unavailable command: dimmed, never filled, and activation does nothing. It stays focusable (see the module
    /// docs).
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// What activating the command does.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Use `handle` as the item's focus handle, so the menu that owns it can start focus there. Without it the item
    /// keeps its own handle in its element state.
    pub fn focus(mut self, handle: &FocusHandle) -> Self {
        self.focus = Some(handle.clone());
        self
    }

    /// Use independent entry points for hover and keys. The click handler still follows the usual press rules.
    pub(super) fn submenu_trigger(
        mut self,
        expanded: bool,
        enter: Handler,
        hover: Handler,
    ) -> Self {
        self.submenu = Some(SubmenuTrigger {
            expanded,
            enter,
            hover,
        });
        self
    }
}

impl RenderOnce for MenuItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let focus = self.focus.clone().unwrap_or_else(|| {
            focus::control_handle(
                ElementId::Name(format!("{}-focus", self.id).into()),
                window,
                cx,
            )
        });
        let enabled = !self.disabled;
        let expanded = self.submenu.as_ref().map(|trigger| trigger.expanded);
        let focused = enabled && focus.is_focused(window);
        let lit = focused || (enabled && expanded == Some(true));
        let ring = focused && focus::shows_ring(&focus, window, cx);
        let (ink, icon_ink, description_ink) = if lit {
            (palette.on_fill, palette.on_fill, palette.on_accent_dim)
        } else if self.danger {
            (palette.danger, palette.danger, palette.fg_1)
        } else {
            (palette.fg_0, palette.fg_1, palette.fg_1)
        };
        let handler = self.on_click.clone().filter(|_| enabled);
        let submenu = self.submenu.filter(|_| enabled);
        let submenu_handler = submenu.as_ref().map(|trigger| trigger.enter.clone());
        let hover_handler = submenu.map(|trigger| trigger.hover);
        let key_handler = submenu_handler.clone().or_else(|| {
            let command = handler.clone()?;
            let activate: Handler = Rc::new(move |event, window, cx| {
                if focus::menu_highlight_shown(window, cx) {
                    command(event, window, cx);
                }
            });
            Some(activate)
        });
        // The trigger moves focus on activation. A press on an open one must leave focus in the child.
        let on_press = if enabled && expanded.is_some() {
            focus::OnPress::KeepControl
        } else {
            focus::OnPress::Control
        };
        let highlight = focus.clone();

        // A choice keeps its label (and any suffix) to one line, cut with an ellipsis: the list is as wide as its
        // control, and a pop-up menu never wrapped an option.
        let words: SharedString = match (&self.suffix, self.checked) {
            (Some(suffix), _) => format!("{} · {suffix}", self.label).into(),
            (None, Some(true)) => format!("{} ✓", self.label).into(),
            (None, _) => self.label.clone(),
        };
        let line = div()
            .min_w_0()
            .when(self.chosen.is_some(), |line| line.truncate());
        let line = if let Some(keys) = self.shortcut {
            line.flex()
                .items_center()
                .gap(px(6.))
                .child(div().flex_1().min_w_0().child(words))
                .child(key_chip(keys, description_ink, palette))
        } else {
            line.child(words)
        };
        let copy = div()
            .flex()
            .flex_col()
            .gap(px(1.))
            .min_w_0()
            .when(self.chosen.is_some() || self.shortcut.is_some(), |copy| {
                copy.flex_1()
            })
            .child(line)
            .children(self.description.clone().map(|description| {
                div()
                    .text_size(px(DESCRIPTION_SIZE))
                    .text_color(theme::color(description_ink))
                    .child(description)
            }));
        let mut item = div()
            .id(ElementId::Name(self.id.clone()))
            .relative()
            // Tests find a control by its id; this records nothing outside test builds.
            .debug_selector(|| self.id.to_string())
            .role(self.role)
            .aria_label(self.label.clone())
            .when_some(self.shortcut, |item, keys| item.aria_keyshortcuts(keys))
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.))
            .mx(px(INSET))
            .py(px(PAD.0))
            .px(px(PAD.1))
            .rounded(metrics::RADIUS_SM)
            .text_size(metrics::TEXT_SM)
            .line_height(px(LINE_HEIGHT))
            .text_color(theme::color(ink))
            .when(lit, |item| item.bg(theme::color(palette.accent_fill)))
            .when(enabled, |item| item.cursor_pointer())
            .when(!enabled, |item| item.opacity(0.5))
            .child(match self.icon {
                Some(icon) => svg()
                    .path(icon.path())
                    .size(px(14.))
                    .flex_none()
                    .mt(px(1.))
                    .text_color(theme::color(icon_ink))
                    .into_any_element(),
                None => div().size(px(14.)).flex_none().into_any_element(),
            })
            .when_some(self.mark, |item, mark| {
                let id = format!("{}-mark", self.id);
                item.child(
                    svg()
                        .path(mark.path())
                        .size(px(14.))
                        .flex_none()
                        .mt(px(1.))
                        .debug_selector(move || id)
                        .text_color(theme::color(icon_ink)),
                )
            })
            .child(copy)
            .when_some(expanded, |item, expanded| {
                item.aria_expanded(expanded).child(div().flex_1()).child(
                    svg()
                        .path(Icon::ChevronDown.path())
                        .size(px(12.))
                        .flex_none()
                        .mt(px(2.))
                        .text_color(theme::color(icon_ink))
                        .with_transformation(Transformation::rotate(radians(
                            -std::f32::consts::FRAC_PI_2,
                        ))),
                )
            })
            .when_some(self.chosen, |item, chosen| item.aria_selected(chosen))
            .when_some(self.checked, |item, checked| {
                item.aria_toggled(if checked {
                    Toggled::True
                } else {
                    Toggled::False
                })
            })
            .when_some(
                self.description
                    .as_ref()
                    .map(Line::accessible_text)
                    .or(self.suffix),
                |item, description| item.aria_description(description),
            );
        if ring {
            // `outline: 2px solid var(--on-fill); outline-offset: -3px`: inside the fill, since an accent ring would
            // vanish against the accent fill.
            item = item.child(
                div()
                    .absolute()
                    .top(px(1.))
                    .left(px(1.))
                    .right(px(1.))
                    .bottom(px(1.))
                    .border_2()
                    .border_color(theme::color(palette.on_fill))
                    .rounded(metrics::RADIUS_SM),
            );
        }
        // The visible description is the accessible description, so the help is the tooltip alone.
        let item =
            super::with_hover_help(item, &self.id, &self.help, false, Some(&focus), window, cx);
        let item = super::activate_on_click(
            super::activate_on_enter_or_space(focus::track(item, &focus, on_press), key_handler),
            handler,
        );
        // Act on a pointer move. GPUI can also report hover on a redraw after a mouse-down switches input modality.
        // That would reopen a submenu dismissed by Escape before the click meant to enter it reaches mouse-up.
        div()
            .id(ElementId::Name(format!("{}-highlight", self.id).into()))
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key == "right"
                    && !event.keystroke.modifiers.modified()
                    && !event.is_held
                    && let Some(open) = &submenu_handler
                {
                    cx.stop_propagation();
                    open(&ClickEvent::default(), window, cx);
                }
            })
            .on_mouse_move(move |event, window, cx| {
                if event.pressed_button.is_none()
                    && !cx.has_active_drag()
                    && layers::prepare_menu_hover(&highlight, window, cx)
                {
                    focus::highlight(&highlight, window, cx);
                    if let Some(open) = &hover_handler {
                        open(&ClickEvent::default(), window, cx);
                    }
                }
            })
            .child(item)
    }
}
