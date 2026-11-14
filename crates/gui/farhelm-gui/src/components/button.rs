//! Buttons, drawn as the old stylesheet's `.btn` tiers: ghost (the default), neutral, primary and danger.
//!
//! A button activates on click. It is not focusable yet, deliberately: in GPUI an element that tracks a focus handle
//! takes focus on every mouse-down, a focus move the lint on GPUI's focus calls cannot see, and whether a click focuses
//! a button at all is a platform rule the focus coordinator owns (SPEC.md, "Keyboard focus"; on macOS Tab follows the
//! system's Keyboard navigation setting, on Linux it always stops: maintainer decision D-2). The coordinator adds the
//! focus handle, Enter and Space activation, the Tab stop and the focus ring (the old `.btn:focus-visible`, which also
//! took the hover fill, with an `--on-fill` outline inset on the filled tiers).

use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder as _, px,
    svg,
};

use super::HoverHelp;
use crate::assets::Icon;
use crate::layers::{self, TooltipOwner};
use crate::theme::{self, Palette, Token, metrics};

/// The old stylesheet's button tiers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tier {
    /// `.btn`: no fill, quiet text, a hover fill. Most toolbar and row controls.
    #[default]
    Ghost,
    /// `.btn-neutral`: a raised fill and outline, for secondary answers such as Cancel.
    Neutral,
    /// `.btn-primary`: the solid accent fill of an ordinary main action.
    Primary,
    /// `.btn-danger`: the solid red fill of a destructive confirmation.
    Danger,
}

/// A click handler.
type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A button with a visible text label (and optionally a leading icon).
#[derive(IntoElement)]
pub struct Button {
    id: SharedString,
    label: SharedString,
    help: HoverHelp,
    role: Role,
    tier: Tier,
    icon: Option<Icon>,
    show_label: bool,
    disabled: bool,
    on_click: Option<Handler>,
}

impl Button {
    /// A button. `id` must be stable and unique among its siblings (it keys the button's element state, the tooltip
    /// owner among it, and the test bridge finds the button by it);
    /// `label` is its visible text and accessible name; `role` is what assistive technology and the test bridge see
    /// (usually [`Role::Button`], [`Role::MenuItem`] inside a menu).
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
    ) -> Self {
        Button {
            id: id.into(),
            label: label.into(),
            help,
            role,
            tier: Tier::default(),
            icon: None,
            show_label: true,
            disabled: false,
            on_click: None,
        }
    }

    /// Draw it in one of the stylesheet's tiers.
    pub fn tier(mut self, tier: Tier) -> Self {
        self.tier = tier;
        self
    }

    /// A leading icon before the label.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// An unavailable button: drawn dimmed (or with the retired fill for primary and danger) and ignoring activation.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// What activating the button does.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Colors for this tier: (fill, hover fill, edge, text). Ghost buttons have no fill or edge at rest.
    fn colors(&self, palette: &Palette) -> (Option<Token>, Token, Option<Token>, Token) {
        let retired = (
            Some(palette.bg_2),
            palette.bg_2,
            Some(palette.border_dim),
            palette.fg_2,
        );
        match self.tier {
            Tier::Ghost => (None, palette.control_hover, None, palette.fg_1),
            Tier::Neutral => (
                Some(palette.bg_2),
                palette.control_hover,
                Some(palette.border),
                palette.fg_0,
            ),
            Tier::Primary if self.disabled => retired,
            Tier::Danger if self.disabled => retired,
            Tier::Primary => (
                Some(palette.accent_fill),
                palette.accent_fill_hover,
                Some(palette.accent_edge),
                palette.on_fill,
            ),
            Tier::Danger => (
                Some(palette.danger_bg),
                palette.danger_fill_hover,
                Some(palette.danger_soft),
                palette.on_fill,
            ),
        }
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let (fill, hover_fill, edge, text) = self.colors(palette);
        let owner = TooltipOwner::for_control(
            ElementId::Name(format!("{}-tooltip-owner", self.id).into()),
            window,
            cx,
        );
        let (pad_y, pad_x) = metrics::BUTTON_PAD;
        let enabled = !self.disabled;
        let handler = self.on_click.clone().filter(|_| enabled);
        let tooltip = self.help.text().clone();

        let mut button = div()
            .id(ElementId::Name(self.id.clone()))
            // Tests find a control by its id; this records nothing outside test builds.
            .debug_selector(|| self.id.to_string())
            .role(self.role)
            .aria_label(self.label.clone())
            .aria_description(self.help.text().clone())
            .flex()
            .items_center()
            .justify_center()
            .gap(px(6.))
            .py(pad_y)
            .px(if self.show_label { pad_x } else { pad_y })
            .rounded(metrics::RADIUS_SM)
            .border_1()
            .text_size(metrics::TEXT_SM)
            .line_height(px(17.))
            .text_color(theme::color(text))
            // A transparent edge where a tier has none keeps every tier the same size.
            .border_color(theme::color(edge.unwrap_or(theme::TRANSPARENT)));
        button = button.when_some(fill, |button, fill| button.bg(theme::color(fill)));
        if enabled {
            button = button
                .cursor_pointer()
                .hover(|style| style.bg(theme::color(hover_fill)));
        } else if matches!(self.tier, Tier::Ghost | Tier::Neutral) {
            button = button.opacity(0.5);
        }
        if let Some(icon) = self.icon {
            button = button.child(
                svg()
                    .path(icon.path())
                    .size(px(14.))
                    .text_color(theme::color(text)),
            );
        }
        if self.show_label {
            button = button.child(self.label);
        }
        button
            .on_hover(move |hovered, window, cx| {
                if *hovered {
                    layers::show_tooltip(&owner, tooltip.clone(), window, cx);
                } else {
                    layers::hide_tooltip(&owner, cx);
                }
            })
            .when_some(handler, |button, handler| {
                button.on_click(move |event, window, cx| handler(event, window, cx))
            })
    }
}

/// A button that shows only an icon. Its label is still required: it is the accessible name, and the hover help
/// carries the words a sighted user would otherwise lack.
#[derive(IntoElement)]
pub struct IconButton(Button);

impl IconButton {
    /// An icon-only button; see [`Button::new`] for the arguments.
    pub fn new(
        id: impl Into<SharedString>,
        icon: Icon,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
    ) -> Self {
        let mut button = Button::new(id, label, help, role).icon(icon);
        button.show_label = false;
        IconButton(button)
    }

    /// See [`Button::tier`].
    pub fn tier(self, tier: Tier) -> Self {
        IconButton(self.0.tier(tier))
    }

    /// See [`Button::disabled`].
    pub fn disabled(self, disabled: bool) -> Self {
        IconButton(self.0.disabled(disabled))
    }

    /// See [`Button::on_click`].
    pub fn on_click(self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        IconButton(self.0.on_click(handler))
    }
}

impl RenderOnce for IconButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.0.render(window, cx)
    }
}
