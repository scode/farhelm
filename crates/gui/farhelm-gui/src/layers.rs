//! The layer manager: the one owner of everything drawn above the window's ordinary content.
//!
//! SPEC_impl.md, "Overlays: the layer manager": one owner draws the dialog slot, the popover stack and tooltips, and
//! GPUI's `deferred`/`anchored` are banned everywhere else in the GUI crates (`crates/gui/clippy.toml`). This module
//! holds that one exemption.
//!
//! NOTE: Phase 1 of the foundations builds only the tooltip slot, because the component layer's hover help needs
//! somewhere to draw. The dialog slot, the popover stack, element anchoring and the covered-link hit test arrive with
//! the rest of the layer manager. Until then a tooltip is placed below and right of the pointer where the hover began,
//! without the old tooltip's pointer notch and the target ring on its control (both follow element anchoring), and it
//! appears at once: the hover delay is a one-shot deadline the ticker will serve.
//!
//! How the slot stays cheap and honest:
//!
//! - The slot is an entity that notifies, and only a root view observes it ([`observe`]). Showing or hiding a tooltip
//!   redraws that root, which draws the overlay; cached views below it replay. A whole-window refresh would instead
//!   re-render every cached view on every hover, which on a 200-row session list is every row each time the pointer
//!   crosses a control.
//! - A tooltip belongs to a [`TooltipOwner`], a token each tooltip-bearing control keeps in its element state
//!   ([`TooltipOwner::for_control`]). The token is unique per control instance in the window (it is keyed by the
//!   control's full element path, not its sibling-unique id), and GPUI drops it when the control is no longer drawn.
//!   The slot watches the token's release, so a tooltip closes when its control is gone (a dialog closed, a row was
//!   deleted) even though no hover-leave ever fires for it: an overlay closes when its subject is gone.

use gpui::{
    AnyElement, App, AppContext as _, BoxShadow, Context, ElementId, Entity, EntityId, Global,
    IntoElement, ParentElement, Pixels, Point, SharedString, Styled, Subscription, Window, div,
    point, px,
};

use crate::theme::{self, metrics};

/// The token that says which control a tooltip belongs to. It carries no data: its entity id is the owner's identity,
/// and its release is the signal that the control is gone.
pub struct TooltipOwner;

impl TooltipOwner {
    /// The owner token of the control being rendered under `key`, created on its first frame and kept for as long as
    /// the control is drawn in consecutive frames (GPUI's keyed element state, which cached views carry over when they
    /// replay). `key` must differ from the control's own element id: GPUI keys element state by path and type, and a
    /// second state type under the same path panics.
    pub fn for_control(
        key: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<TooltipOwner> {
        window.use_keyed_state(key, cx, |_, _| TooltipOwner)
    }
}

/// The hover help currently showing, if any. At most one tooltip shows at a time, which is what the old app did too.
#[derive(Clone, Debug, PartialEq)]
pub struct ShownTooltip {
    /// Which control asked for it, so only that control can take it down again.
    pub owner: EntityId,
    /// The help text, already final (hover help is Farhelm's own wording, never host-supplied text).
    pub text: SharedString,
    /// Where the pointer was when the hover began, in window coordinates.
    pub at: Point<Pixels>,
}

/// The tooltip slot. Changed only through [`show_tooltip`] and [`hide_tooltip`], which notify.
#[derive(Default)]
pub struct TooltipSlot {
    shown: Option<ShownTooltip>,
    /// Takes the tooltip down when its owner is released; replaced with each new tooltip.
    _owner_release: Option<Subscription>,
}

/// Where the app keeps its one tooltip slot.
struct Slot(Entity<TooltipSlot>);
impl Global for Slot {}

/// Create the tooltip slot. The bootstrap calls this with the rest of the app-wide state, before any window opens.
pub fn install(cx: &mut App) {
    let slot = cx.new(|_| TooltipSlot::default());
    cx.set_global(Slot(slot));
}

/// Redraw `root` whenever the overlay changes. A root view that draws [`overlay`] calls this once and keeps the
/// subscription; without it a tooltip would show only on the root's next unrelated redraw. Returns `None` before
/// [`install`] (a bare fixture), where there is nothing to observe.
pub fn observe<V: 'static>(cx: &mut Context<V>) -> Option<Subscription> {
    let slot = cx.try_global::<Slot>()?.0.clone();
    Some(cx.observe(&slot, |_, _, cx| cx.notify()))
}

/// Show `text` as hover help for the control `owner`, replacing any other tooltip, at the pointer's position.
pub fn show_tooltip(
    owner: &Entity<TooltipOwner>,
    text: SharedString,
    window: &mut Window,
    cx: &mut App,
) {
    let at = window.mouse_position();
    show_tooltip_at(owner, text, at, cx);
}

/// Show `text` for `owner` with the hover beginning at `at`. [`show_tooltip`] reads `at` from the pointer; the
/// screenshot goldens pass a fixed point.
pub fn show_tooltip_at(
    owner: &Entity<TooltipOwner>,
    text: SharedString,
    at: Point<Pixels>,
    cx: &mut App,
) {
    let Some(slot) = cx.try_global::<Slot>().map(|slot| slot.0.clone()) else {
        return;
    };
    let owner_id = owner.entity_id();
    slot.update(cx, |slot, cx| {
        slot.shown = Some(ShownTooltip {
            owner: owner_id,
            text,
            at,
        });
        slot._owner_release = Some(cx.observe_release(owner, move |slot, _, cx| {
            if slot
                .shown
                .as_ref()
                .is_some_and(|shown| shown.owner == owner_id)
            {
                slot.shown = None;
                cx.notify();
            }
        }));
        cx.notify();
    });
}

/// Take down the tooltip if `owner` is the control showing it. A control leaving never hides another's tooltip, which
/// matters when the pointer moves straight from one control onto the next and the new control's hover arrives first.
pub fn hide_tooltip(owner: &Entity<TooltipOwner>, cx: &mut App) {
    let Some(slot) = cx.try_global::<Slot>().map(|slot| slot.0.clone()) else {
        return;
    };
    let owner_id = owner.entity_id();
    slot.update(cx, |slot, cx| {
        if slot
            .shown
            .as_ref()
            .is_some_and(|shown| shown.owner == owner_id)
        {
            slot.shown = None;
            slot._owner_release = None;
            cx.notify();
        }
    });
}

/// The tooltip showing now, for tests and the bridge's snapshot.
pub fn shown_tooltip(cx: &App) -> Option<ShownTooltip> {
    cx.try_global::<Slot>()
        .and_then(|slot| slot.0.read(cx).shown.clone())
}

/// The overlay layer the root view draws last: the tooltip, if one is showing, styled as the old `.farhelm-tooltip`.
#[allow(
    clippy::disallowed_methods,
    reason = "the layer manager is the one owner of deferred and anchored"
)]
pub fn overlay(window: &Window, cx: &App) -> Option<AnyElement> {
    let shown = shown_tooltip(cx)?;
    let palette = theme::palette(cx);
    let (pad_y, pad_x) = metrics::TOOLTIP_PAD;
    let viewport = window.viewport_size();
    // `max-width: min(280px, 100vw - 8px)` and `max-height: 100vh - 8px`, as the old tooltip.
    let max_width = metrics::TOOLTIP_MAX_WIDTH.min(viewport.width - px(8.));
    let card = div()
        .max_w(max_width)
        .max_h(viewport.height - px(8.))
        .overflow_hidden()
        .py(pad_y)
        .px(pad_x)
        .bg(theme::color(palette.bg_2))
        .text_color(theme::color(palette.fg_0))
        .border_1()
        .border_color(theme::color(palette.border))
        .rounded(metrics::RADIUS_MD)
        // `--shadow-menu`: `0 6px 18px` in the token's color.
        .shadow(vec![BoxShadow {
            color: theme::color(palette.shadow_menu),
            offset: point(px(0.), px(6.)),
            blur_radius: px(18.),
            spread_radius: px(0.),
            inset: false,
        }])
        .text_size(metrics::TEXT_SM)
        .line_height(metrics::TOOLTIP_LINE_HEIGHT)
        .line_clamp(metrics::TOOLTIP_MAX_LINES)
        .child(shown.text);
    // Offset like a native tooltip so the pointer never covers the text.
    let at = point(shown.at.x + px(8.), shown.at.y + px(16.));
    Some(
        gpui::deferred(gpui::anchored().position(at).snap_to_window().child(card))
            .with_priority(1)
            .into_any_element(),
    )
}
