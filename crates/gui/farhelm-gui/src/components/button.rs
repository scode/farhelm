//! Buttons, drawn as the old stylesheet's `.btn` tiers: ghost (the default), neutral, primary and danger, plus the
//! compact control that sits on the red banner.
//!
//! A button activates on click, and on Enter or Space while it has keyboard focus (a held key does not repeat it:
//! SPEC.md, "Repeated or composing Enter cannot answer the new question"). Focus is the focus coordinator's: an enabled
//! button is a tab stop tracked through [`focus::track`], which applies the coordinator's click rule (on Linux a click
//! focuses it, on macOS it does not, as in WebKit there) in place of GPUI's own focus-on-press.
//! A disabled button is not a tab stop and takes no focus on a click (SPEC.md: Tab stays within a dialog's "visible,
//! enabled controls"). The
//! focus ring is the old `:focus-visible` ring (a 2-pixel accent outline one pixel outside the button, shown only after
//! keyboard input) together with the hover fill, which the old sheet also gave a focused button so a ghost button's box
//! is visible to a keyboard user.

use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, FocusHandle, InteractiveElement, IntoElement, MouseButton,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder as _, px, svg,
};

use super::HoverHelp;
use crate::assets::Icon;
use crate::focus;
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
    /// `.btn.banner-reclaim`: a compact ghost button for the red banner (the old terminal's "Take control"). It has a
    /// pale red outline and white text so it reads against the banner's fill, a darker red hover, and a white focus
    /// ring because the accent ring is not calibrated for a red background. Use it only on a red banner.
    OnBanner,
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
    focus: Option<FocusHandle>,
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
            focus: None,
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

    /// Use `handle` as the button's focus handle, so whoever owns it can make the button where focus starts (the prompt
    /// does, for its answers). Without it the button keeps its own handle in its element state.
    pub fn focus(mut self, handle: &FocusHandle) -> Self {
        self.focus = Some(handle.clone());
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
            Tier::OnBanner => (
                None,
                palette.danger_fill_hover,
                Some(palette.danger_soft),
                palette.on_fill,
            ),
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
        let focus = self.focus.clone().unwrap_or_else(|| {
            focus::control_handle(
                ElementId::Name(format!("{}-focus", self.id).into()),
                window,
                cx,
            )
        });
        let ring = focus::shows_ring(&focus, window);
        // `.banner-reclaim` overrides `.btn`'s padding to fit inside the banner's 4-pixel band.
        let (pad_y, pad_x) = match self.tier {
            Tier::OnBanner => metrics::BANNER_BUTTON_PAD,
            Tier::Ghost | Tier::Neutral | Tier::Primary | Tier::Danger => metrics::BUTTON_PAD,
        };
        let ring_color = match self.tier {
            Tier::OnBanner => palette.on_fill,
            Tier::Ghost | Tier::Neutral | Tier::Primary | Tier::Danger => palette.accent,
        };
        let enabled = !self.disabled;
        let handler = self.on_click.clone().filter(|_| enabled);
        let key_handler = handler.clone();

        let mut button = div()
            .id(ElementId::Name(self.id.clone()))
            .relative()
            // Tests find a control by its id; this records nothing outside test builds.
            .debug_selector(|| self.id.to_string())
            .role(self.role)
            .aria_label(self.label.clone())
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
        if ring && enabled {
            button = button.bg(theme::color(hover_fill));
        }
        if ring {
            // The old `outline: 2px solid var(--accent); outline-offset: 1px`, drawn as a ring element because GPUI has
            // no outline: two pixels wide, one pixel clear of the button's edge.
            button = button.child(
                div()
                    .absolute()
                    .top(px(-3.))
                    .left(px(-3.))
                    .right(px(-3.))
                    .bottom(px(-3.))
                    .border_2()
                    .border_color(theme::color(ring_color))
                    .rounded(metrics::RADIUS_SM + px(3.)),
            );
        }
        if enabled {
            button = button
                .cursor_pointer()
                .hover(|style| style.bg(theme::color(hover_fill)));
        } else if matches!(self.tier, Tier::Ghost | Tier::Neutral | Tier::OnBanner) {
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
        let button = super::with_hover_help(button, &self.id, &self.help, true, window, cx);
        let button = if enabled {
            focus::track(button, &focus, focus::OnPress::Control)
        } else {
            // Not tracked, so neither Tab nor a click can focus it; the press is still this control's, so it does not
            // reach the page's empty-space handler and move focus.
            button.on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        };
        button
            .on_key_down(move |event, window, cx| {
                let key = event.keystroke.key.as_str();
                if event.is_held || event.keystroke.modifiers.modified() {
                    return;
                }
                if (key == "enter" || key == "space")
                    && let Some(handler) = &key_handler
                {
                    cx.stop_propagation();
                    handler(&ClickEvent::default(), window, cx);
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

    /// See [`Button::focus`].
    pub fn focus(self, handle: &FocusHandle) -> Self {
        IconButton(self.0.focus(handle))
    }
}

impl RenderOnce for IconButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.0.render(window, cx)
    }
}
