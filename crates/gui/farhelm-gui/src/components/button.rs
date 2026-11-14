//! Buttons, drawn as the old stylesheet's `.btn` tiers: ghost (the default), neutral, primary and danger, the outlined
//! danger of a permanent answer, plus the compact control that sits on the red banner, the ⋯ that opens a list row's
//! menu, the host rail's square entries, the removable filter chip, the button that reads as a link, and an entry of a
//! navigation list such as Settings' sections.
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
//! enabled controls"). A busy button ([`Button::busy`]) is the old app's `aria-disabled` control, different in that it
//! keeps focus, its place in Tab's order and its hover help, and does nothing when pressed. The old app used it for
//! two cases: an action in flight (a Save while the save runs, so the control the user chose stays focused while its
//! work runs, where disabling it would drop focus out of the dialog's controls), and an action that cannot run for a
//! reason its hover help explains (the session header's Restart for a session that cannot resume, whose reason must
//! stay reachable by pointer and keyboard alike).
//!
//! The focus ring is the old `:focus-visible` ring (a 2-pixel accent outline one pixel outside the button, shown only
//! after keyboard input) together with the hover fill, which the old sheet also gave a focused button so a ghost
//! button's box is visible to a keyboard user. The Settings section list's entries (`Tier::Nav`) are the exception:
//! their old rules drew the ring alone, one pixel inside the edge, with no focus fill.

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
    /// `.btn-primary.btn-outline`: accent text and an accent outline on no fill, with the control hover fill. The
    /// permanent answer beside a one-off primary answer, for the same reason as [`Tier::DangerOutline`]: the host setup
    /// confirmation's "Set up, don't ask again" beside Set up. Unavailable, it takes the retired fill as
    /// [`Tier::Primary`] does; busy, it keeps its outline and loses only its hover.
    PrimaryOutline,
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
    /// `.app-settings-navigation button`: an entry of a navigation list (Settings' sections), as wide as its list, its
    /// icon and label at the left 8 pixels apart, 13-pixel text on a 16-pixel line (the old `line-height: normal`, which
    /// made it 34 pixels high), the primary ink at rest, and the label cut short with an ellipsis rather than wrapped.
    /// The entry whose pane is shown is drawn through [`Button::pressed`], in this tier as the old sheet drew it: the
    /// accent fill with the accent edge, which the pointer does not change, and the inverse ring one pixel inside the
    /// edge (`outline-offset: -3px`).
    Nav,
    /// `.templates-quick-mark`: the bolt beside a template in the Templates list, a ghost with no fill at rest, its glyph
    /// in the quiet `--fg-2` until it holds ⚡, then in the warning amber (`lit`). The old sheet drew no fill under the
    /// pointer, only an edge; here the pointer gets the ghost tier's hover fill, the nearest look the button draws. A
    /// lit bolt is also reported pressed ([`Button::pressed`]), without the pressed fill: the amber glyph is its look.
    QuickMark { lit: bool },
    /// `.templates-number-key`: a template's number-key button, a ghost in a tight 4-pixel box whose text (a chord, in
    /// the monospace face, given as [`Button::content`]) is the quiet `--fg-2` with no key and the accent once one is
    /// bound (`bound`). The same hover note as [`Tier::QuickMark`].
    NumberKey { bound: bool },
}

/// The [`Tier::QuickMark`] box's padding, vertical then horizontal (`.templates-quick-mark`: `10px 5px`).
const QUICK_MARK_PAD: (f32, f32) = (10., 5.);
/// The [`Tier::NumberKey`] box's padding on every side.
const NUMBER_KEY_PAD: f32 = 4.;

/// The [`Tier::RowMenu`] box's height: 14-pixel text at `line-height: 1.4`.
const ROW_MENU_HEIGHT: f32 = 19.6;
/// The [`Tier::RowMenu`] box's horizontal padding.
const ROW_MENU_PAD_X: f32 = 5.;
/// The [`Tier::Rail`] box's side (`.host-rail .btn`'s `width` and `height`).
const RAIL_SIZE: f32 = 40.;
/// The [`Tier::Chip`] box's padding, vertical then horizontal.
const CHIP_PAD: (f32, f32) = (4., 6.);
/// The [`Tier::Nav`] box's padding, vertical then horizontal.
const NAV_PAD: (f32, f32) = (8., 10.);
/// The [`Tier::Nav`] label's line height.
const NAV_LINE_HEIGHT: f32 = 16.;

/// A compact button's padding, vertical then horizontal: `.titlebar-actions .btn`'s `padding: 2px 8px`, the session
/// header's actions.
const COMPACT_PAD: (f32, f32) = (2., 8.);
/// A compact button's gap between its icon and its label: the header's main buttons' `gap: 4px`.
const COMPACT_GAP: f32 = 4.;
/// A compact button's icon: `.action-glyph`'s 13 pixels.
const COMPACT_ICON: f32 = 13.;

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
    /// The accessible name, when it says more than the visible label ([`Button::accessible_name`]).
    accessible_name: Option<SharedString>,
    disabled: bool,
    /// Whether the button's action is in flight or cannot run: drawn as unavailable, ignoring activation, but still
    /// focusable ([`Button::busy`]).
    busy: bool,
    /// Whether the button's action would be refused now: drawn as unavailable, but still pressed ([`Button::refused`]).
    refused: bool,
    /// The session header's tighter geometry ([`Button::compact`]).
    compact: bool,
    /// Whether what the button opens is open; `None` for a button that opens nothing.
    expanded: Option<bool>,
    /// Whether the button's choice is the one in force; `None` for a button that is not a choice.
    pressed: Option<bool>,
    /// Whether a pressed button wears the quieter look ([`Button::quiet`]).
    quiet: bool,
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
            accessible_name: None,
            disabled: false,
            busy: false,
            refused: false,
            compact: false,
            expanded: None,
            pressed: None,
            quiet: false,
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

    /// The old `aria-disabled` button: one whose action is in flight (a Save while the save runs) or cannot run for a
    /// reason its hover help explains (the session header's Restart for a session that cannot resume). Unlike
    /// [`Button::disabled`], it stays a tab stop, keeps focus and keeps its hover help, so the button the user pressed
    /// is still where focus is when the work settles, and a reason stays reachable. It ignores activation: Enter and
    /// Space stop at the button, as they do at an enabled one, instead of going on to whatever holds it. A busy neutral
    /// button shows `cursor: not-allowed`, as the old sheet gave the `aria-disabled` header Restart; no other tier
    /// does, since the old busy Save kept the button's cursor.
    ///
    /// Its look by tier: the filled tiers (neutral, chip, primary, danger) take the retired look, the old sheet's
    /// `--bg-2` fill, `--border-dim` edge and `--fg-2` text; the unfilled ones (ghost, the danger outline, the row's ⋯,
    /// the banner's, the rail's, the Settings section list's and the link) keep their look and lose only their hover
    /// fill. GPUI cannot report the state to assistive technology, so a button whose action cannot run should say so in
    /// its name ([`Button::accessible_name`]).
    ///
    /// The look is applied on the next redraw, so a handler must still keep its own guard for a press that arrives
    /// before it (Rename's Save checks its draft).
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// A main action that would be refused now (the launcher's Launch while its draft is incomplete): drawn as
    /// unavailable, with the retired fill for primary and danger, but still focusable and still pressed, so a press
    /// runs the handler, which says why nothing happened. The old launcher's submit button was `aria-disabled` rather
    /// than `disabled` for exactly this (`create_form.rs` at reference commit `24bc0580`; SPEC.md, "Creation": "an
    /// incomplete attempt says why beside Launch, as does a refused Launch press").
    pub fn refused(mut self, refused: bool) -> Self {
        self.refused = refused;
        self
    }

    /// The accessible name, when the visible label alone would leave something out: the session header's Restart shows
    /// "Restart" but is named for its offer ("Resume conversation", "Restart unavailable"), as the old button's
    /// `aria-label` was. Without it the label is the name.
    pub fn accessible_name(mut self, name: impl Into<SharedString>) -> Self {
        self.accessible_name = Some(name.into());
        self
    }

    /// The session header's geometry (`.titlebar-actions .btn`): 2 by 8 pixels of padding, 4 between the icon and the
    /// label, and a 13-pixel icon, so the header's actions keep their compact height and fit a 650-pixel pane.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// The button opens something, and `expanded` says whether it is open now: `aria-expanded`, and the pressed look
    /// while it is (see the module docs). On a button that is also [`Button::pressed`], the pressed look wins.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// The button is one of a set of choices (the host rail's entries), and `pressed` says whether it is the one in
    /// force now: `aria-pressed`, which assistive technology hears as a toggle's state, and while pressed the old
    /// `[aria-pressed="true"]` look, the accent fill with the inverse ink and an inverse focus ring drawn inside the
    /// edge. A pressed button keeps that fill under the pointer and the keyboard ring (one step up the accent family),
    /// where the old sheet's later `.btn` hover and focus fills replaced it and left white ink on a light fill; a
    /// pressed `Tier::Nav` entry keeps its fill unchanged under the pointer, as the old section list did.
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = Some(pressed);
        self
    }

    /// While `quiet`, a pressed button wears a quieter look than the accent fill: the control hover fill, the primary
    /// ink and an accent edge, with its keyboard ring in the accent inside the edge, and the same fill under the
    /// pointer. The host rail's chosen entry wears it while Hosts management is shown, so that the filled Hosts control
    /// stays the one that marks the view on screen, as the old sheet drew it (the
    /// `.host-rail .btn[aria-pressed="true"]` rule under `.app-sidebar:has(.hosts-panel-toggle[aria-pressed="true"])`).
    /// It changes only the look: the button still reports itself pressed. It does nothing on a button that is not
    /// pressed.
    pub fn quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
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
    pub(super) fn colors(&self, palette: &Palette) -> (Option<Token>, Token, Option<Token>, Token) {
        // Before the pressed looks: a lit bolt is reported pressed but keeps its own look.
        match self.tier {
            Tier::QuickMark { lit } => {
                let ink = if lit { palette.warn } else { palette.fg_2 };
                return (None, palette.control_hover, None, ink);
            }
            Tier::NumberKey { bound } => {
                let ink = if bound { palette.accent } else { palette.fg_2 };
                return (None, palette.control_hover, None, ink);
            }
            _ => {}
        }
        if self.pressed == Some(true) && self.quiet {
            return (
                Some(palette.control_hover),
                palette.control_hover,
                Some(palette.accent),
                palette.fg_0,
            );
        }
        if self.pressed == Some(true) && self.tier == Tier::Nav {
            return (
                Some(palette.accent_fill),
                palette.accent_fill,
                Some(palette.accent_edge),
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
        // After the pressed looks: a button that is both a choice in force and open (the host rail's Hosts while host
        // management shows) keeps its pressed look, as the old sheet drew it.
        if self.expanded == Some(true) {
            return (
                Some(palette.accent_fill_hover),
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
            // Returned above, before the pressed looks.
            Tier::QuickMark { .. } | Tier::NumberKey { .. } => {
                (None, palette.control_hover, None, palette.fg_2)
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
            Tier::Neutral | Tier::Chip if self.busy => retired,
            Tier::Neutral | Tier::Chip => (
                Some(palette.bg_2),
                palette.control_hover,
                Some(palette.border),
                palette.fg_0,
            ),
            Tier::Rail => (None, palette.bg_2, None, palette.fg_1),
            Tier::Nav => (None, palette.control_hover, None, palette.fg_0),
            Tier::Link => (None, theme::TRANSPARENT, None, palette.link),
            Tier::Primary if self.disabled || self.busy || self.refused => retired,
            Tier::Danger if self.disabled || self.busy || self.refused => retired,
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
            Tier::PrimaryOutline if self.disabled => retired,
            Tier::PrimaryOutline => (
                None,
                palette.control_hover,
                Some(palette.accent),
                palette.accent,
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
            Tier::Ghost | Tier::Neutral | Tier::Primary | Tier::Danger if self.compact => {
                (px(COMPACT_PAD.0), px(COMPACT_PAD.1))
            }
            Tier::Rail | Tier::Link => (px(0.), px(0.)),
            Tier::Chip => (px(CHIP_PAD.0), px(CHIP_PAD.1)),
            Tier::Nav => (px(NAV_PAD.0), px(NAV_PAD.1)),
            Tier::QuickMark { .. } => (px(QUICK_MARK_PAD.0), px(QUICK_MARK_PAD.1)),
            Tier::NumberKey { .. } => (px(NUMBER_KEY_PAD), px(NUMBER_KEY_PAD)),
            Tier::Ghost
            | Tier::Neutral
            | Tier::Primary
            | Tier::Danger
            | Tier::DangerOutline
            | Tier::PrimaryOutline => metrics::BUTTON_PAD,
        };
        let pressed = self.pressed == Some(true);
        let ring_color = match self.tier {
            // The quiet look's fill is light in the light palette, where the inverse ring would not show.
            _ if pressed && self.quiet => palette.accent,
            _ if pressed => palette.on_fill,
            Tier::OnBanner | Tier::RowMenu { on_fill: true } => palette.on_fill,
            Tier::Ghost
            | Tier::Neutral
            | Tier::Primary
            | Tier::Danger
            | Tier::DangerOutline
            | Tier::PrimaryOutline
            | Tier::Rail
            | Tier::Chip
            | Tier::Link
            | Tier::Nav
            | Tier::QuickMark { .. }
            | Tier::NumberKey { .. }
            | Tier::RowMenu { on_fill: false } => palette.accent,
        };
        // An icon-only button is otherwise as high as its icon plus its padding and square; the row menu's box is the
        // old toggle's, set by its text line and its own side padding; a rail entry is a fixed square.
        let (pad_x, height) = match self.tier {
            Tier::RowMenu { .. } => (pad_x, Some(px(ROW_MENU_HEIGHT))),
            Tier::Rail => (pad_x, Some(px(RAIL_SIZE))),
            // The bolt keeps its tall, narrow box (`padding: 10px 5px`), not the square an icon-only button gets.
            Tier::QuickMark { .. } => (pad_x, None),
            _ if self.show_label || self.content.is_some() => (pad_x, None),
            _ => (pad_y, None),
        };
        let enabled = !self.disabled;
        // A busy button stays focusable (`enabled`), but nothing it does acts then (`active`).
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
            .aria_label(
                self.accessible_name
                    .clone()
                    .unwrap_or_else(|| self.label.clone()),
            )
            .flex()
            .items_center()
            .justify_center()
            .gap(px(if self.compact { COMPACT_GAP } else { 6. }))
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
            .border_color(theme::color(edge.unwrap_or(theme::TRANSPARENT)))
            .when(self.tier == Tier::Nav, |button| {
                button
                    .justify_start()
                    .gap(px(8.))
                    .min_w_0()
                    .text_size(metrics::TEXT_MD)
                    .line_height(px(NAV_LINE_HEIGHT))
            });
        button = button.when_some(fill, |button, fill| button.bg(theme::color(fill)));
        // The old navigation entries showed keyboard focus by their ring alone, without `.btn`'s focus fill.
        if ring && enabled && self.tier != Tier::Nav {
            button = button.bg(theme::color(hover_fill));
        }
        if ring && pressed {
            // A pressed button's ring is the inverse ink inside its edge (`outline-color: var(--on-fill);
            // outline-offset: -4px`), as the old rail drew it: the accent ring would not read against the accent fill.
            // The old navigation entries drew it a pixel further out (`outline-offset: -3px`).
            let (inset, radius) = if self.tier == Tier::Nav {
                (px(1.), metrics::RADIUS_SM - px(1.))
            } else {
                (px(2.), px(1.))
            };
            button = button.child(
                div()
                    .absolute()
                    .top(inset)
                    .left(inset)
                    .right(inset)
                    .bottom(inset)
                    .border_2()
                    .border_color(theme::color(ring_color))
                    .rounded(radius),
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
        } else if enabled && self.busy && self.tier == Tier::Neutral {
            // The old sheet's `cursor: not-allowed` on the `aria-disabled` header Restart, which still answers the
            // hover. Only the neutral tier: the old busy Save (a primary) kept the button's cursor.
            button = button.cursor_not_allowed();
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
                    | Tier::Nav
                    | Tier::QuickMark { .. }
                    | Tier::NumberKey { .. }
            )
        {
            button = button.opacity(0.5);
        }
        if let Some(icon) = self.icon {
            button = button.child(
                svg()
                    .path(icon.path())
                    .size(px(if self.compact { COMPACT_ICON } else { 14. }))
                    .text_color(theme::color(text)),
            );
        }
        if let Some(content) = self.content {
            button = button.child(content);
        } else if self.show_label && self.tier == Tier::Nav {
            button = button.child(div().min_w_0().truncate().child(self.label));
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

    /// See [`Button::pressed`].
    pub fn pressed(self, pressed: bool) -> Self {
        IconButton(self.0.pressed(pressed))
    }

    /// See [`Button::busy`]: the Templates list's bolts keep focus while the mark's write is in flight.
    pub fn busy(self, busy: bool) -> Self {
        IconButton(self.0.busy(busy))
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
