//! Buttons, drawn as the old stylesheet's `.btn` tiers: ghost (the default), neutral, primary and danger, the outlined
//! danger of a permanent answer, plus the compact control that sits on the red banner, the ⋯ that opens a list row's
//! menu, the host rail's square entries, the removable filter chip, and the button that reads as a link.
//!
//! A button shows its label's text, or other content in its place ([`Button::content`]: a host's mark, a mark beside a
//! host's name); the label is its accessible name either way. A button that is one of a set of choices says whether
//! it is the one in force ([`Button::pressed`]), as the rail's entries do.
//!
//! A button that opens something (a menu, a question) can say so with [`Button::expanded`]: assistive technology hears
//! whether what it opens is open, and while it is the button wears the old pressed look, the accent fill with the
//! inverse ink. A plain hover fill left behind after the pointer moves away would read as stale hover, not as "open";
//! the old header triggers and the row menu's ⋯ followed the same rule.
//!
//! A button activates on click, and on Enter or Space while it has keyboard focus (a held key does not repeat it:
//! SPEC.md, "Repeated or composing Enter cannot answer the new question"). Focus is the focus coordinator's: an enabled
//! button is a tab stop tracked through [`focus::track`], which applies the coordinator's click rule (on Linux a click
//! focuses it, on macOS it does not, as in WebKit there) in place of GPUI's own focus-on-press.
//! A disabled button is not a tab stop and takes no focus on a click (SPEC.md: Tab stays within a dialog's "visible,
//! enabled controls"). A busy button ([`Button::busy`]), whose action is in flight, is different: it keeps focus and
//! its place in Tab's order and does nothing when pressed, the old app's `aria-disabled` Save, so the control the user
//! chose stays focused while its work runs (disabling it would drop focus out of the dialog's controls). The
//! focus ring is the old `:focus-visible` ring (a 2-pixel accent outline one pixel outside the button, shown only after
//! keyboard input) together with the hover fill, which the old sheet also gave a focused button so a ghost button's box
//! is visible to a keyboard user.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, FocusHandle, InteractiveElement, IntoElement,
    MouseButton, ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled,
    Toggled, Window, div, prelude::FluentBuilder as _, px, svg,
};

use super::{Handler, HoverHelp};
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
    /// `.btn-danger.btn-outline`: red text and a red outline on no fill, with a subdued red hover. The permanent answer
    /// beside a one-off danger answer (the YOLO confirmation's "Start, and don't ask again on this host" beside "Start
    /// YOLO session anyway"): a filled button draws the reflex click, and the answer that turns a question off for good
    /// should not be the reflex target, so it keeps the danger tier's color but not its fill. Unavailable, it takes the
    /// retired fill as [`Tier::Danger`] does; busy, it keeps its outline and loses only its hover, as the old
    /// `aria-disabled` rule left it.
    DangerOutline,
    /// `.btn.banner-reclaim`: a compact ghost button for the red banner (the old terminal's "Take control"). It has a
    /// pale red outline and white text so it reads against the banner's fill, a darker red hover, and a white focus
    /// ring because the accent ring is not calibrated for a red background. Use it only on a red banner.
    OnBanner,
    /// `.session-row-menu`: the ghost ⋯ at the end of a list row, in the old toggle's box (`padding: 0 5px` around the
    /// glyph, at the row's 14-pixel text and `line-height: 1.4`, so 19.6 pixels high). `on_fill` is for the selected
    /// row's solid accent fill, where it draws in the inverse ink with an accent-family hover and an inverse focus ring
    /// (`.session-row.selected .session-row-menu` and `.session-row.selected :focus-visible`).
    RowMenu { on_fill: bool },
    /// `.host-rail .btn`: an entry of the host rail, a 40-pixel square with no padding around what it shows (a word
    /// or a host's mark, through [`Button::content`]), quiet ink, and the raised `--bg-2` under the pointer. Its chosen
    /// entry is drawn through [`Button::pressed`].
    Rail,
    /// `.btn.btn-neutral.host-filter-chip`: the neutral tier in a chip's tighter box (`padding: 4px 6px`) that may
    /// shrink to its row, so the host name inside it can be cut short. Its content lays out its own parts and does the
    /// cutting.
    Chip,
    /// `.inline-link`: a button that reads as a link in running text: the link color, underlined, with no fill, edge
    /// or padding. The focus ring is every control's.
    Link,
}

/// The [`Tier::RowMenu`] box's height: 14-pixel text at `line-height: 1.4`.
const ROW_MENU_HEIGHT: f32 = 19.6;
/// The [`Tier::RowMenu`] box's horizontal padding.
const ROW_MENU_PAD_X: f32 = 5.;
/// The [`Tier::Rail`] box's side (`.host-rail .btn`'s `width` and `height`).
const RAIL_SIZE: f32 = 40.;
/// The [`Tier::Chip`] box's padding, vertical then horizontal.
const CHIP_PAD: (f32, f32) = (4., 6.);

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
    /// Whether the button's action is in flight: drawn as unavailable, ignoring activation, but still focusable.
    busy: bool,
    /// Whether what the button opens is open; `None` for a button that opens nothing.
    expanded: Option<bool>,
    /// Whether the button's choice is the one in force; `None` for a button that is not a choice.
    pressed: Option<bool>,
    /// What the button shows in place of its label's text, when it shows more than words.
    content: Option<AnyElement>,
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
            busy: false,
            expanded: None,
            pressed: None,
            content: None,
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

    /// A button whose action is in flight (a Save while the save runs): drawn as unavailable (the retired fill for
    /// primary and danger, the old `aria-disabled` look; the other tiers keep their look and lose only their hover
    /// fill) and ignoring activation, but, unlike [`Button::disabled`], it stays a tab stop and keeps focus, so the
    /// button the user pressed is still where focus is when the work settles. A press while busy does nothing at all:
    /// Enter and Space stop at the button, as they do at an enabled one, instead of going on to whatever holds it. The
    /// look is applied on the next redraw, so a handler must still keep its own guard for a press that arrives before
    /// it (Rename's Save checks its draft).
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// The button opens something, and `expanded` says whether it is open now: `aria-expanded`, and the pressed look
    /// while it is (see the module docs).
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// The button is one of a set of choices (the host rail's entries), and `pressed` says whether it is the one in
    /// force now: `aria-pressed`, which assistive technology hears as a toggle's state, and while pressed the old
    /// `[aria-pressed="true"]` look, the accent fill with the inverse ink and an inverse focus ring drawn inside the
    /// edge. A pressed button keeps that fill under the pointer and the keyboard ring (one step up the accent family),
    /// where the old sheet's later `.btn` hover and focus fills replaced it and left white ink on a light fill.
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = Some(pressed);
        self
    }

    /// Show `content` in place of the label's text (a host's mark, or a mark beside a host's name), centered in the
    /// button's box. The label stays the button's accessible name, so content that a screen reader should not read
    /// twice needs no hiding; content holding host text draws it through the host-text element, as everywhere.
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
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

    /// Colors for this tier and state: (fill, hover fill, edge, text). Ghost buttons have no fill or edge at rest.
    fn colors(&self, palette: &Palette) -> (Option<Token>, Token, Option<Token>, Token) {
        if self.expanded == Some(true) {
            return (
                Some(palette.accent_fill_hover),
                palette.accent_fill_hover,
                None,
                palette.on_fill,
            );
        }
        if self.pressed == Some(true) {
            return (
                Some(palette.accent_fill),
                palette.accent_fill_hover,
                None,
                palette.on_fill,
            );
        }
        let retired = (
            Some(palette.bg_2),
            palette.bg_2,
            Some(palette.border_dim),
            palette.fg_2,
        );
        match self.tier {
            Tier::Ghost | Tier::RowMenu { on_fill: false } => {
                (None, palette.control_hover, None, palette.fg_1)
            }
            Tier::RowMenu { on_fill: true } => {
                (None, palette.accent_fill_hover, None, palette.on_fill)
            }
            Tier::OnBanner => (
                None,
                palette.danger_fill_hover,
                Some(palette.danger_soft),
                palette.on_fill,
            ),
            Tier::Neutral | Tier::Chip => (
                Some(palette.bg_2),
                palette.control_hover,
                Some(palette.border),
                palette.fg_0,
            ),
            Tier::Rail => (None, palette.bg_2, None, palette.fg_1),
            Tier::Link => (None, theme::TRANSPARENT, None, palette.link),
            Tier::Primary if self.disabled || self.busy => retired,
            Tier::Danger if self.disabled || self.busy => retired,
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
            Tier::DangerOutline if self.disabled => retired,
            Tier::DangerOutline => (
                None,
                palette.danger_bg_dim,
                Some(palette.danger),
                palette.danger,
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
            Tier::RowMenu { .. } => (px(0.), px(ROW_MENU_PAD_X)),
            Tier::Rail | Tier::Link => (px(0.), px(0.)),
            Tier::Chip => (px(CHIP_PAD.0), px(CHIP_PAD.1)),
            Tier::Ghost | Tier::Neutral | Tier::Primary | Tier::Danger | Tier::DangerOutline => {
                metrics::BUTTON_PAD
            }
        };
        let pressed = self.pressed == Some(true);
        let ring_color = match self.tier {
            _ if pressed => palette.on_fill,
            Tier::OnBanner | Tier::RowMenu { on_fill: true } => palette.on_fill,
            Tier::Ghost
            | Tier::Neutral
            | Tier::Primary
            | Tier::Danger
            | Tier::DangerOutline
            | Tier::Rail
            | Tier::Chip
            | Tier::Link
            | Tier::RowMenu { on_fill: false } => palette.accent,
        };
        // An icon-only button is otherwise as high as its icon plus its padding and square; the row menu's box is the
        // old toggle's, set by its text line and its own side padding; a rail entry is a fixed square.
        let (pad_x, height) = match self.tier {
            Tier::RowMenu { .. } => (pad_x, Some(px(ROW_MENU_HEIGHT))),
            Tier::Rail => (pad_x, Some(px(RAIL_SIZE))),
            _ if self.show_label || self.content.is_some() => (pad_x, None),
            _ => (pad_y, None),
        };
        let enabled = !self.disabled;
        // A busy button stays focusable (`enabled`), but nothing it does acts while busy (`active`).
        let active = enabled && !self.busy;
        let handler = self.on_click.clone().filter(|_| active);
        // A busy button still takes Enter and Space, doing nothing with them, so a press while busy does nothing at
        // all, rather than going on up to whatever holds the button, as it would from a button that does not take
        // them.
        let key_handler: Option<Handler> = if enabled && self.busy {
            Some(Rc::new(|_: &ClickEvent, _: &mut Window, _: &mut App| {}))
        } else {
            handler.clone()
        };

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
            .px(pad_x)
            .when_some(height, |button, height| button.h(height))
            .when(self.tier == Tier::Rail, |button| {
                button.w(px(RAIL_SIZE)).flex_none()
            })
            // A chip yields its width to the row it sits in (`min-width: 0; max-width: 100%`), and its content cuts
            // the host's name short.
            .when(self.tier == Tier::Chip, |button| {
                button.min_w_0().max_w_full()
            })
            .when_some(self.expanded, |button, expanded| {
                button.aria_expanded(expanded)
            })
            .when_some(self.pressed, |button, pressed| {
                button.aria_toggled(if pressed {
                    Toggled::True
                } else {
                    Toggled::False
                })
            })
            .rounded(metrics::RADIUS_SM)
            // `.inline-link` has no edge at all (`border: 0`), so it sits on the text line like the words around it;
            // every other tier keeps the 1-pixel edge.
            .when(self.tier != Tier::Link, |button| button.border_1())
            .when(self.tier == Tier::Link, |button| button.underline())
            .text_size(metrics::TEXT_SM)
            .line_height(px(17.))
            .text_color(theme::color(text))
            // A transparent edge where a tier has none keeps every tier the same size.
            .border_color(theme::color(edge.unwrap_or(theme::TRANSPARENT)));
        button = button.when_some(fill, |button, fill| button.bg(theme::color(fill)));
        if ring && enabled {
            button = button.bg(theme::color(hover_fill));
        }
        if ring && pressed {
            // A pressed button's ring is the inverse ink inside its edge (`outline-color: var(--on-fill);
            // outline-offset: -4px`), as the old rail drew it: the accent ring would not read against the accent fill.
            button = button.child(
                div()
                    .absolute()
                    .top(px(2.))
                    .left(px(2.))
                    .right(px(2.))
                    .bottom(px(2.))
                    .border_2()
                    .border_color(theme::color(ring_color))
                    .rounded(px(1.)),
            );
        } else if ring {
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
        if active {
            button = button
                .cursor_pointer()
                .hover(|style| style.bg(theme::color(hover_fill)));
        } else if !enabled
            && matches!(
                self.tier,
                Tier::Ghost
                    | Tier::Neutral
                    | Tier::OnBanner
                    | Tier::RowMenu { .. }
                    | Tier::Rail
                    | Tier::Chip
                    | Tier::Link
            )
        {
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
        if let Some(content) = self.content {
            button = button.child(content);
        } else if self.show_label {
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
        super::activate_on_click(
            super::activate_on_enter_or_space(button, key_handler),
            handler,
        )
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

    /// See [`Button::expanded`].
    pub fn expanded(self, expanded: bool) -> Self {
        IconButton(self.0.expanded(expanded))
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
