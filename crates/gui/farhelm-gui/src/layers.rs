//! The layer manager: the one owner of everything drawn above the window's ordinary content.
//!
//! SPEC_impl.md, "Overlays: the layer manager": one owner draws the dialog slot, the popover stack and tooltips, and
//! GPUI's `deferred`/`anchored` are banned everywhere else in the GUI crates (`crates/gui/clippy.toml`). This module
//! holds that one exemption. Its rules:
//!
//! - **Who opened what.** Opening a layer returns its [`LayerId`], and only that id closes it ([`close`]): nothing
//!   closes a dialog, menu or confirmation it did not open. A layer also remembers the layer it was opened from, and
//!   closing a layer closes the layers opened from it, not every layer above it: one opened from an approval card over
//!   a dialog belongs to no layer, and stays when the dialog goes ([`close`] has the rule). The user closes the
//!   topmost layer with Escape ([`dismiss_top`]) or, for a popover, by clicking outside it; a popover that lets the
//!   pointer through is not one the user answers, so both pass over it ([`Popover::passes_pointer`]). A dialog's own
//!   hook decides each dismissal ([`Dialog::on_dismiss`]): it may keep the dialog open, as an editor with a save in
//!   flight does, or keep it open and ask first, as an editor with an unsaved draft will. The one exception is a reset
//!   of the connection to the helm, which closes everything ([`close_all`]) without asking.
//! - **Things the user did not start never open a layer.** Every open takes the [`Mark`] the feature took when the user
//!   acted, and is refused once the user has moved on since (the focus coordinator's counter). Work that finishes later
//!   shows its result where it belongs instead. At window start, a pending SSH question may open before the first key
//!   or click. The same exception covers the Welcome card
//!   and the first batch of announcements when nothing is open ahead of it. Those dialogs take a mark when their
//!   owner first sees a new UI session. SSH takes the mark when the window is built ([`Mark::at_ui_session_start`]). The user
//!   has done nothing yet; if they type or click before the open, the open is refused, and the dialog stays unseen
//!   until a later window start or reset (the maintainer's decision, 2026-11-14). A batch that SPEC.md has wait behind
//!   the Welcome card (and, after Send feedback now, behind the feedback dialog) is not an exception: it opens with a
//!   mark taken when the last of them closes, which always follows a press of the user's.
//! - **An overlay closes when its subject is gone, not when it moves.** A popover is anchored to an element through an
//!   [`Anchor`], state the anchor element keeps in its element state: it follows the element wherever it is drawn,
//!   and closes when the element is no longer drawn (a deleted row's menu). A layer may also name a [`Subject`]: an
//!   entity whose release closes it, or a session the helm's listing proves gone (`Subject::session`, a
//!   confirmation about a session that was deleted elsewhere). Every overlay is anchored to an element: SPEC.md has no
//!   menu opened at a pointer position.
//! - **Three kinds of layer.** A dialog is centered over a veil (or near the window's top, for an opener that asks:
//!   [`Dialog::placement`]). A popover (a menu, opened from [`Popover::menu`]) is drawn below its anchor, or
//!   beside it when the anchor asks for that ([`Anchor::beside`], the session row's menu flying out beside the
//!   sidebar). An anchored dialog ([`open_anchored`]) is drawn like a popover but answers like a dialog: it has a
//!   subject, a main action for Enter and its own empty-space rule. It is how the prompt asks a question beside the
//!   control it is about (a row's Delete question, opened from the row's menu, as the old app turned the menu into the
//!   question). An anchored layer is as wide as its content, or exactly as wide as its anchor when the anchor asks for
//!   that ([`Anchor::matching_width`]): a select's list under its control, a floating list under its field.
//! - **Dialogs, popovers and tooltips sit in the window's content area.** A dialog is sized and centered, a popover
//!   is placed, flipped and kept, and a tooltip (at the pointer, beside the sidebar or against a focused control) is
//!   kept, inside the part of the window that holds the page ([`crate::desktop::linux_title_bar::content_area`]), as
//!   in the old app's page: on Linux, inside the frame GPUI Kit draws around the window and below the window's own
//!   title bar, both of which the window's reported size includes, so a tall card or a dialog at its height limit
//!   placed against the whole window covered the bar (settings-and-hosts and templates reported it on 2026-11-15), and
//!   a wide popover near the right edge ended in Kit's frame, outside the page, where SPEC.md has a click on a
//!   terminal link refused while its destination display is not wholly inside the page (popovers moved into the
//!   content area on 2026-11-16, [`popover_origin`] says more). The notices panels cap their height by the content
//!   area too, so a full one still fits. A dialog's veil still covers the bar, so the bar stays inert while a dialog
//!   is open. Under a compositor's decorations and on macOS the content area is the whole window. The pills and the
//!   approval region are still placed against the whole window.
//! - **A dialog is modal.** It draws a veil over the window that takes every click. A click on its empty space (label
//!   text included: labels are no longer selectable, so the GPUI migration settled on 2026-11-13 that a click on one
//!   counts as empty space) keeps focus on the control in use in the dialogs SPEC.md lists, and moves focus to the
//!   dialog itself in the others ([`Dialog::keeps_focus_on_empty_click`]); a click on a menu's empty space keeps focus
//!   on its item. Tab stays inside the topmost layer, dialog or menu alike (the focus coordinator walks GPUI's tab
//!   order within [`trap`]), unless it is the popover below that lets Tab through. Closing any layer returns focus to
//!   what held it when the layer opened, unless its opener named home or a control (the button that opened a dialog,
//!   the toggle that opened a menu) instead ([`Dialog::returns_to`], [`Popover::returns_to`]). A press reaches a
//!   dialog's empty-space rule only after the controls under it have handled it, and one that gave focus to a control
//!   (a click into a text field) is not a press on empty space (the focus coordinator's [`focus::OnPress::Focus`]).
//! - **A popover's card takes the pointer from what it covers.** It has no veil, so the page around it stays live, but
//!   a press, click, wheel, hover or move on the card itself never reaches the element under it: a click on a
//!   question's answer over a terminal must not also focus the terminal or click a control there. The one exception
//!   is a popover opened to let the pointer through ([`Popover::passes_pointer`]), which is display only: a press,
//!   hover or wheel turn on it reaches what it covers, except on a control inside it that occludes itself, as long as
//!   its view keeps to the rules the [`Pill`] NOTE lists. Escape and a press outside it never close it.
//! - **A popover may leave focus alone.** One opened through [`Popover::floating`] (no [`Popover::start`]) never takes
//!   focus: opening it moves none, Tab is not trapped in it, closing it returns none, and a press on it leaves focus
//!   where it was. It is for a list that floats beside the control the user keeps typing in or pointing at, as the old
//!   launcher's folder picker and composer search kept focus in their field (options at `tabindex: -1`, the list's
//!   mousedown prevented). Escape, a press outside it and its anchor going still close it, as any popover, but closing
//!   it closes only it: it owns nothing above it, so a dialog opened while it shows (a shortcut pressed in the field or
//!   the terminal beside it) stays open, with focus inside, when it goes ([`close`]). Every rule above that names "the
//!   topmost layer" for focus (Tab's trap, where focus recovers, the menu highlight) means the topmost layer that holds
//!   focus, which such a popover never is (and for Tab's trap, the topmost that also traps Tab: see the next rule).
//! - **Hover can open a submenu before it takes focus.** It belongs to the parent menu from the first frame, so Escape
//!   and outside presses see it immediately. The parent keeps the keyboard until the user enters the child. Hovering
//!   a sibling item can dismiss that child while either menu has focus. Other overlays keep their usual ownership.
//! - **A popover may let Tab through.** One opened with [`Popover::tab_through`] ([`Popover::traps_tab`] off) takes
//!   focus when it opens and returns it on Escape like a menu, but traps no Tab: Tab and Shift+Tab walk its controls as
//!   if they stood in the page right after its toggle (the control its [`Popover::returns_to`] names), and go on out of
//!   it to the controls around the toggle. Focus leaving it, by Tab or by a press elsewhere, closes it and returns no
//!   focus; focus on its toggle is not leaving, and neither is keyboard focus moving into a layer opened above it (a
//!   press inside such a layer is still a press outside the popover, which closes it and the layers opened from it).
//!   It is SPEC.md's tunnel list: "Escape closes the list and returns focus to the pill; Tab follows its ordinary
//!   controls and leaving the list closes it without reclaiming focus", and the old list closed on the pill's
//!   `focusout` once focus was outside both the pill and the list. It holds focus, so it is the topmost layer that
//!   holds focus for recovery and the menu highlight, and it counts for [`focus_layer_open`] (which says why).
//!   [`tab_through`] says how its place in Tab order is kept, since GPUI paints every overlay, and so orders its
//!   controls, after the whole page.
//! - **One tooltip at a time**, owned by the control that showed it (see [`TooltipOwner`]). It says the app's own
//!   words, or a host sentence when the help names a host value, drawn through host text's element ([`ShownTooltip`]).
//!   Help in paragraphs ([`Help::Paragraphs`]) is drawn wider and never cut short, as the old app drew it, and card
//!   help ([`Help::card`]) draws a view of the caller's at a fixed width. The [`tooltip`] submodule holds it, and says
//!   when it shows and goes. [`beside`] places the card after the control and its containing surface record their
//!   bounds. Menus can use either side of their panel, leaving any open submenu readable. A session row uses the
//!   sidebar edge and keeps its accent ring. The ordinary above/below rule is the fallback.
//! - **The innermost help wins** when a control with hover help sits inside another with its own (a session row's
//!   marks inside the row); [`hover_began`], "Nested hover help", explains the rule and how it is decided.
//! - **Display-only pill slots** ([`show_pill`], [`show_launch_pill`], [`show_terminal_refusal`]). They let a feature draw
//!   over the dialogs without opening one. The pill is SPEC.md's quick-launch sequence pill, which stays in view above
//!   the launcher's veil; it sits centered over an [`Anchor`] element or at its bottom edge ([`PillPlacement`]) and
//!   goes when that element is no longer drawn. The launch pill is SPEC.md's launch progress pill and its "Started"
//!   report, centered 20 pixels above the window's bottom edge, where the old `.launch-progress-pill` was fixed. The
//!   z-order ladder below gives each its place. A refused link's explanation uses an anchored slot above the approval
//!   cards. Each slot holds one pill, and the slots can show together. Each is hidden through the id its show returned
//!   ([`hide_pill`]); all go on a reset ([`close_all`]). They are display only, like
//!   the tooltip: they ask for no pointer input and hold no focus, so a press on one is a press on whatever it covers,
//!   and nothing that asks about layers ([`any_open`], Tab's trap, where focus recovers, Escape's [`dismiss_top`]) sees
//!   them.
//! - **One approval region that is not a layer either** ([`install_approval_region`]): where SPEC.md's independent
//!   agent approval cards stay usable while a dialog is open, so it is drawn over every dialog but a raised one (its
//!   rung on the z-order ladder below). Unlike the pills it takes presses and focus; like them, nothing that asks about
//!   layers sees it, and it is installed once per window, never shown or hidden. SPEC_impl.md, "Overlays: the layer
//!   manager", says why it has this shape and which of its behaviors follow from the rules above.
//! - **One layer may be raised above the approval region** ([`above_approval_region`]), and every layer opened over it
//!   goes with it: SPEC.md's tunnel warning, which the old app drew over the approval cards whatever opened it, so a
//!   card that asks for it cannot cover it. Its box is raised and its veil is not, so the cards beside it stay usable.
//!
//! State is kept per window: multi-window support is a watch-list item, and the one thing it asks of the foundations
//! now is that overlays are not shared between windows. Each window's [`LayerStack`] is an entity that notifies, and
//! only that window's root view observes it ([`attach`]), so opening a layer or showing a tooltip redraws the root,
//! which draws the overlay, while cached views below it replay. A whole-window refresh would instead re-render every
//! cached view, which on a 200-row session list is every row each time the pointer crosses a control.
//!
//! Terminal destination panels ask [`pointer_covers`] whether a later surface blocks input over any of their bounds.
//! Pass-through and display-only surfaces leave that hit test alone. The popover supplies the surface, while each
//! menu supplies its items. Session row menus use the shared menu item component. The tooltip's own deviations and
//! gaps are in the [`tooltip`] submodule's NOTE.
//!
//! # The z-order ladder
//!
//! The one order in which a window draws everything, bottom to top. Every other doc in this module, and SPEC_impl.md's
//! "Overlays: the layer manager", that says what an overlay is drawn over or under points here; a new slot gets a rung
//! here first. [`overlay`] draws rungs 2 to 9 as `deferred` elements whose priorities follow this order. Each rung
//! gives the old app's z-index for what it ports, from the old stylesheet's z-index ladder comment at `.header-confirm`
//! (`crates/farhelm-ui/assets/app.css` at reference commit `24bc0580`, lines 2187-2272, cited below by line), where
//! equal values were ordered by their place in the page.
//!
//! 1. **The page**: the window's ordinary content, everything the root view draws before [`overlay`], its last child.
//! 2. **The layers**, each over the one opened before it: dialogs over their veils, popovers (menus and lists) and
//!    anchored dialogs (questions). Old: the row menus at 20 (line 2220), the header questions at 30 (line 2227) and
//!    the modal dialogs' backdrops at 40 (line 2243). Only the layers under the lowest raised one are on this rung; it
//!    and every layer opened over it are on rung 7.
//! 3. **The launch pill** ([`show_launch_pill`]). Old: 40, the progress pill inside the launcher's backdrop and the
//!    "Started" report on its own (lines 6527 and 6564, past the ladder comment), level with the dialogs, so the page's
//!    order put it over some and under others. Here it is over every dialog on rung 2, a deviation chosen by the
//!    foundations owner on 2026-11-15; [`show_launch_pill`]'s NOTE has the old split and why.
//! 4. **The raised dialog's veil**, drawn apart from its box only for the lowest raised layer, when it is a dialog
//!    ([`above_approval_region`]). Old: the tunnel consent dialog's backdrop at 50 (line 2261), over the cards. Here it
//!    is under them, decided by the foundations owner on 2026-11-15: SPEC.md's "remain usable" wins over the old app's
//!    blocking backdrop ([`above_approval_region`] has the decision).
//! 5. **The approval region** ([`install_approval_region`]). Old: the approval cards at 45 (line 2247), above every
//!    modal backdrop.
//! 6. **The terminal link refusal** ([`show_terminal_refusal`]), at its ordinary line's bounds with pointer input
//!    still going to the approval card beneath it. While a layer holds the keyboard, draw the refusal under the
//!    layers so it cannot obscure the question there.
//! 7. **The raised layers**: the lowest raised layer's box, and every layer opened over it, whole, veils included, each
//!    over the one before. Old: the tunnel consent dialog at 50 (line 2261). A dialog opened over it would have sat at
//!    40, under it; here it goes over it, so that the drawn order stays the stack's ([`above_approval_region`]).
//! 8. **The sequence pill** ([`show_pill`]). Old: `.launch-sequence-indicator` at 50 (line 2256), later in the page
//!    than the tunnel consent dialog.
//! 9. **The tooltip** ([`tooltip`]). Old: `.farhelm-tooltip` at 50 (line 2258), "above every panel and modal backdrop,
//!    the approval cards included", its card appended to the page's body (`tooltip.js` line 388).

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, AnyView, AnyWindowHandle, App, AppContext as _, AvailableSpace, Bounds, BoxShadow,
    Context, Element, ElementId, Entity, EntityId, FocusHandle, Global, GlobalElementId, HitboxId,
    InspectorElementId, InteractiveElement, IntoElement, LayoutId, ParentElement, Pixels, Point,
    Size, Style, Styled, Subscription, WeakEntity, WeakFocusHandle, Window, WindowId, canvas, div,
    point, prelude::FluentBuilder as _, px,
};

use crate::actions;
use crate::bootstrap::{installed, installed_mut};
use crate::focus::{self, Mark};
use crate::theme::{self, metrics};
use crate::ticker;

mod beside;
#[cfg(test)]
mod beside_tests;
mod tooltip;
#[cfg(test)]
mod tooltip_tests;

pub use beside::beside_edge;
// The tooltip's public items, at the paths every caller has always used (`layers::hover_began` and the like).
pub use tooltip::{
    BesideCues, DrawnRing, HOVER_DELAY, Help, Hint, ShownTooltip, TooltipOwner, TooltipSide,
    focus_drawn, help_drawn, hover_began, hover_ended, hover_moved, pointer_pressed, shown_tooltip,
};
pub(crate) use tooltip::{HelpPlacement, TallSide};
use tooltip::{Hovered, tooltip_card};
#[cfg(test)]
pub(crate) use tooltip::{TRACKER, show_tooltip_at};

// ---- Per-window state ------------------------------------------------------------------------------------------------

/// Identifies one open layer. Returned by the open functions; the only way to close that layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerId(u64);

impl LayerId {
    /// Whether this layer opened after `other`, and so, while both are open, sits above it: ids grow with every open,
    /// are never reused, and a layer only ever joins the top of its window's stack, which nothing reorders.
    fn opened_after(self, other: LayerId) -> bool {
        self.0 > other.0
    }
}

/// One window's overlays: its open layers, display-only slots, approval region and tooltip.
pub struct LayerStack {
    window: AnyWindowHandle,
    layers: Vec<Layer>,
    next_id: u64,
    /// The pill showing in this window, if any ([`show_pill`]). Kept apart from `layers` so that nothing that walks the
    /// layers for focus, Escape or "is anything open" can count it.
    pill: Option<ShownPill>,
    /// The launch pill showing in this window, if any ([`show_launch_pill`]): the second slot, kept apart from `layers`
    /// for the same reason as `pill`, and apart from `pill` so that neither show replaces the other's pill.
    launch_pill: Option<ShownLaunchPill>,
    /// The terminal's refused-link line while the pointer still rests on the link. Its own slot lets the line
    /// explain the refusal over the card that covered the details without disturbing the other pills.
    terminal_refusal: Option<ShownPill>,
    /// One counter across the slots so a [`PillId`] always names a single pill.
    next_pill_id: u64,
    /// The approval region installed in this window, if any ([`install_approval_region`]). Kept apart from `layers`,
    /// as the pill is, so that nothing that walks the layers for focus, Escape or "is anything open" counts it.
    region: Option<Region>,
    tooltip: Option<ShownTooltip>,
    /// Keyboard focus owning the tooltip shown or waiting, if it does: the pointer leaving does not take it down, focus
    /// leaving does, and it is placed against its control ([`focus_drawn`]).
    by_focus: Option<tooltip::ByFocus>,
    /// The control with help that last rendered holding focus ([`focus_drawn`]).
    focused: Option<EntityId>,
    /// Focus news waiting for the settle after this event, like `round` for the pointer's.
    focus_news: Option<tooltip::FocusNews>,
    /// Drawn containers with a right edge their controls can use for help. Popovers supply their surface bounds
    /// separately because the layer manager already tracks them.
    help_edges: Vec<Anchor>,
    /// Takes the tooltip down when its owner is released; replaced with each new tooltip.
    tooltip_release: Option<Subscription>,
    /// The tooltip waiting out its hover delay: its owner, and the deadline that shows it. Dropping it cancels.
    pending_tooltip: Option<(EntityId, ticker::Deadline)>,
    /// A tooltip showed recently enough that the next one skips the hover delay. Cleared by `cool_down`.
    warm: bool,
    cool_down: Option<ticker::Deadline>,
    /// Every control with hover help the pointer is over, innermost first; the first one's help is the one the pointer
    /// calls for ([`hover_began`], "Nested hover help").
    hovered: Vec<Hovered>,
    /// The first of `hovered` when the hover news last settled. A pointer moving inside the same control leaves the
    /// winner as it is, so it never shows a showing tooltip again, and a help a press put away stays away while the
    /// hover lasts ([`pointer_pressed`]; the [`tooltip`] module's NOTE says when GPUI ends it). Starting a waiting
    /// tooltip's delay over on such a move is [`hover_moved`]'s, not the winner's. A tooltip shown or waiting through a
    /// hover is always the winner's; only the goldens' [`show_tooltip_at`] shows one that is not.
    winner: Option<EntityId>,
    /// The hover news of the event being handled, waiting for [`tooltip::settle_hover`], which runs once after it:
    /// `Some` while a settle is scheduled, holding the owners whose move listener ran, in the order GPUI ran them.
    round: Option<Vec<EntityId>>,
}

/// An open dialog or popover.
struct Layer {
    id: LayerId,
    kind: Kind,
    view: AnyView,
    /// The layer's own focus handle, tracked by the element that wraps its content: Tab stays inside it (dialogs), and
    /// focus recovers to it when its focused control disappears.
    container: FocusHandle,
    /// What held focus when the layer opened, so closing it can return focus there.
    opener: Option<WeakFocusHandle>,
    /// The layer it was opened from, decided as it opened ([`parent_for_new_layer`], or cleared by its opener through
    /// [`opened_from_approval_card`]): closing that layer closes this one too ([`close`]). Always a layer that holds
    /// focus and sits below this one, since layers only ever join the top of the stack; `None` for a layer opened from
    /// the page or from an approval card.
    parent: Option<LayerId>,
    /// Whether the layer took focus when it opened and holds it while open: every layer but a popover opened without
    /// a start ([`Popover::start`]), and a submenu that hover opened, until the user enters it ([`enter_submenu`]).
    /// Tab's trap, focus recovery and the focus return on close all pass over a layer without it.
    takes_focus: bool,
    /// Whether Tab stays inside it while it holds focus: every layer that takes focus but a popover opened with
    /// [`Popover::traps_tab`] off, whose Tab follows the page instead ([`tab_through`]).
    traps_tab: bool,
    /// Where closing it returns focus: `opener`, unless its opener named home or a control ([`Dialog::returns_to`],
    /// [`Popover::returns_to`]).
    returns_to: focus::ReturnTo,
    /// What a dismissal of it does ([`Dialog::on_dismiss`]); `None` always closes.
    on_dismiss: Option<DismissHook>,
    /// What Enter does while the dialog itself (not one of its controls) has focus.
    main_action: Option<MainAction>,
    /// What a press on the layer's empty space does to focus.
    empty_press: focus::OnPress,
    /// Close the layer when its subject is gone or its anchor is no longer drawn.
    _watches: Vec<Subscription>,
    /// Whether its box, and every layer opened over it, is drawn above the approval region ([`above_approval_region`]).
    /// It lives and goes with the layer: ids are never reused, so no later layer inherits it.
    above_region: bool,
}

/// A dialog's main action, which Enter presses while the dialog itself has focus.
pub type MainAction = Rc<dyn Fn(&mut Window, &mut App)>;

/// What a dialog does when the user dismisses it ([`Dialog::on_dismiss`]), asked at the moment of each dismissal. It
/// gets the window so it can open a question over the dialog before it answers [`Dismissal::Stay`].
pub type DismissHook = Rc<dyn Fn(&mut Window, &mut App) -> Dismissal>;

/// A dialog's answer to a dismissal ([`Dialog::on_dismiss`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dismissal {
    /// Close the dialog, as a dismissal does without a hook.
    Close,
    /// Keep it open: it is busy with work it must not lose, or it has just asked the user what to do (and closes itself
    /// later, through [`close`], if the answer says so).
    Stay,
}

enum Kind {
    /// A dialog over its veil, sitting and answering an outside press as its opener asked.
    Dialog(DialogOptions),
    /// A popover, placed against its anchor element as its [`Placement`] says and following it as it moves.
    Popover {
        /// Where the anchor was last drawn.
        anchor: AnchorBounds,
        /// Where the popover itself was drawn. An ancestor's outside
        /// press must leave a click inside this surface to the child.
        surface: AnchorBounds,
        placement: Placement,
        /// Whether it is exactly as wide as its anchor ([`Anchor::matching_width`]).
        matches_width: bool,
        /// Whether a press on it reaches what it covers ([`Popover::passes_pointer`]): `false` for every anchored
        /// layer but a popover opened that way, which never takes focus ([`open_popover`] makes sure).
        passes_pointer: bool,
    },
}

impl Kind {
    /// Submenu placement identifies the child menus that participate in hover switching.
    fn is_submenu(&self) -> bool {
        matches!(
            self,
            Self::Popover {
                placement: Placement::Submenu,
                ..
            }
        )
    }

    /// Whether this is a popover that lets the pointer through ([`Popover::passes_pointer`]): what drawing, Escape, a
    /// dismissible veil and the tooltip ask before treating the layer as one the user is pointing at or answering.
    fn passes_pointer(&self) -> bool {
        matches!(
            self,
            Kind::Popover {
                passes_pointer: true,
                ..
            }
        )
    }
}

/// Where a popover sits against its anchor. Every placement uses the same
/// content-area clamp ([`popover_origin`]).
#[derive(Clone)]
pub enum Placement {
    /// Below the anchor, left edges aligned, or above it when it would run off the content area's bottom and fits
    /// above: the old menus and popup lists.
    Below,
    /// Beside the anchor, to its right: the left edge 10 pixels past the anchor's right edge and the top 4 pixels above
    /// the anchor's. The old session row menu's flyout (`session_menu_placement_style` in `menu_panel.rs` at reference
    /// commit `24bc0580`), whose toggle ends just inside the sidebar's edge, so the menu stands 8 pixels clear of the
    /// list. Like the old one it never changes to another side when the window is narrow: the clamp only shifts it back
    /// inside.
    ///
    /// NOTE: The clamp is the one every popover shares, which keeps the popover flush inside the content area. The old
    /// flyout's own clamp differed, a deviation recorded when this was ported: it kept 8 pixels from the edges, it held
    /// a fixed 528 pixels of room below the menu's top rather than the menu's own height (so a short menu on a low row
    /// moved up further than it does here), and it capped the menu's height and scrolled it. This clamp caps no height:
    /// a popover taller than the content area would run past its bottom. The row menu caps its own height to the
    /// content area instead and scrolls inside it (`session_list/menu.rs`, through `notices_bell::window_cap`), so a long
    /// refusal in it stays in reach.
    Beside,
    /// Prefer the right of the parent item, with the left side available
    /// when it has more room and the right would cover part of the menu.
    Submenu,
    /// Wherever the anchor's owner says ([`Anchor::placed_by`]), asked as each frame is laid out ([`PlaceBy`] has the
    /// rule's inputs and what it may do). For an overlay whose rule needs its own measured size and facts only its
    /// owner knows. Its one caller is the terminal's drag-copy notice, which sits three terminal rows above or below
    /// the pointer, by the room its pane leaves, centred on the pointer (SPEC.md, "Terminal experience"). Every other
    /// placement here is a named rule from the old app, so a second caller is a decision for the layer manager's
    /// owner, not a use of this one.
    By(PlaceBy),
}

/// A popover's own placement rule ([`Placement::By`]): from the anchor's bounds in this frame, the popover's measured
/// size and `area`, the area the window's content is laid out in (all in window coordinates), the popover's top-left
/// corner.
///
/// The rule keeps the popover inside `area` itself. The shared clamp that follows it keeps every popover inside the
/// same area ([`popover_origin`]), so it leaves a rule's answer as it was, and it only catches a rule that does not
/// keep inside.
///
/// It is called while the frame is laid out, with no app access, once per frame for as long as the popover shows: it
/// must be cheap and pure, and read only cells or anchors ([`Anchor::drawn_bounds`] gives this frame's bounds there).
/// It lives as long as the layer, so it captures cells, weak handles or anchors, never a strong `Entity`: that entity
/// would be kept alive until the layer closes, and if it holds the anchor too, each keeps the other alive until then.
pub type PlaceBy = Rc<dyn Fn(Bounds<Pixels>, Size<Pixels>, Bounds<Pixels>) -> Point<Pixels>>;

/// Where an anchor element was last drawn, written by its tracker as frames are prepared.
type AnchorBounds = Rc<Cell<Option<Bounds<Pixels>>>>;

/// Where each window's layer stack lives, by window. Weak, so a closed window's stack goes with its root view.
#[derive(Default)]
struct Windows(HashMap<WindowId, WeakEntity<LayerStack>>);

impl Global for Windows {}

/// Install the app-wide registry. The bootstrap calls this before any window opens.
pub fn install(cx: &mut App) {
    cx.set_global(Windows::default());
}

/// What a root view keeps so its window has overlays: the window's layer stack and the root's subscription to it.
pub struct Attachment {
    _stack: Entity<LayerStack>,
    _observe: Subscription,
}

/// Give the root view's window a layer stack, and redraw the root whenever an overlay changes. Every root view does this
/// once, from its constructor, through [`crate::root::attach`], and draws through [`crate::root::wrap`], which draws
/// [`overlay`] as its last child.
pub fn attach<V: 'static>(window: &mut Window, cx: &mut Context<V>) -> Attachment {
    let handle = window.window_handle();
    let stack = cx.new(|_| LayerStack {
        window: handle,
        layers: Vec::new(),
        next_id: 0,
        pill: None,
        launch_pill: None,
        terminal_refusal: None,
        next_pill_id: 0,
        region: None,
        tooltip: None,
        by_focus: None,
        focused: None,
        focus_news: None,
        help_edges: Vec::new(),
        tooltip_release: None,
        pending_tooltip: None,
        warm: false,
        cool_down: None,
        hovered: Vec::new(),
        winner: None,
        round: None,
    });
    if let Some(windows) = installed_mut::<Windows>(cx) {
        windows.0.retain(|_, stack| stack.upgrade().is_some());
        windows.0.insert(handle.window_id(), stack.downgrade());
    }
    let observe = cx.observe(&stack, |_, _, cx| cx.notify());
    Attachment {
        _stack: stack,
        _observe: observe,
    }
}

fn stack(window: &Window, cx: &App) -> Option<Entity<LayerStack>> {
    installed::<Windows>(cx)?
        .0
        .get(&window.window_handle().window_id())?
        .upgrade()
}

// ---- Subjects and anchors --------------------------------------------------------------------------------------------

/// What a layer is about. When the subject is gone, the layer closes: SPEC.md, "Nothing closes a dialog … it did not
/// open. Each goes away when … the thing it is about is gone".
///
/// Two kinds of subject. An entity ([`Subject::of`]) is gone when it is released. Something an entity holds
/// ([`Subject::when`]) is gone when its owner says so: a session, an entry in the store the channel feeds with no entity
/// of its own, is gone when the helm's listing proves it (`Subject::session`, which [`crate::session`] supplies).
pub struct Subject(Watch);

/// Starts watching a subject for the layer with the given id: the subscription that closes it once the subject is gone.
type Watch = Box<dyn FnOnce(&mut Context<LayerStack>, AnyWindowHandle, LayerId) -> Subscription>;

impl Subject {
    /// The layer is about `entity`: it closes when `entity` is released.
    pub fn of<T: 'static>(entity: &Entity<T>) -> Self {
        let entity = entity.clone();
        Subject(Box::new(move |cx, window, id| {
            cx.observe_release(&entity, move |_, _, cx| close_later(window, id, cx))
        }))
    }

    /// The layer is about something `entity` holds: it closes once `gone` says it is gone, checked when the layer
    /// opens (a question about something already gone closes at once) and each time `entity` notifies. For things that
    /// have no entity of their own, such as a session, which is an entry in the store the channel feeds; the owner of
    /// that state supplies `gone` (`Subject::session` in [`crate::session`]), so the layer manager knows nothing of it.
    ///
    /// `gone` runs on every notification of `entity` for as long as the layer is open, so it should be cheap.
    pub fn when<T: 'static>(entity: Entity<T>, gone: impl Fn(&T, &App) -> bool + 'static) -> Self {
        Subject(Box::new(move |cx, window, id| {
            if gone(entity.read(cx), cx) {
                close_later(window, id, cx);
            }
            cx.observe(&entity, move |_, entity, cx| {
                if gone(entity.read(cx), cx) {
                    close_later(window, id, cx);
                }
            })
        }))
    }
}

/// An element a popover can be anchored to, or the pill placed over ([`Pill::anchor`]). Built while the element renders
/// ([`Anchor::for_element`]); the element draws [`Anchor::track`] as a child so the overlay knows where it is.
///
/// It refers to the element's state weakly: keeping an `Anchor` around (in a view, a test) does not keep the element
/// "drawn", so a popover anchored to it still closes when the element goes.
#[derive(Clone)]
pub struct Anchor {
    state: WeakEntity<AnchorState>,
    bounds: AnchorBounds,
    /// Where a layer opened on this anchor sits against it: [`Placement::Below`] unless the feature asked otherwise.
    placement: Placement,
    /// Whether a layer opened on this anchor is exactly as wide as it ([`Anchor::matching_width`]).
    matches_width: bool,
}

/// An anchor element's state between frames, kept in its keyed element state: GPUI releases it when the element is not
/// drawn in a frame, and that release is the signal that the anchor is gone. It holds the bounds its tracker records.
pub struct AnchorState {
    bounds: AnchorBounds,
}

impl Anchor {
    /// The anchor of the element being rendered under `key`, kept for as long as the element is drawn in consecutive
    /// frames (GPUI's keyed element state). `key` must differ from the element's own id: GPUI keys element state by
    /// path and type.
    pub fn for_element(key: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> Anchor {
        let state = window.use_keyed_state(key, cx, |_, _| AnchorState {
            bounds: Rc::new(Cell::new(None)),
        });
        Anchor {
            bounds: state.read(cx).bounds.clone(),
            state: state.downgrade(),
            placement: Placement::Below,
            matches_width: false,
        }
    }

    /// This anchor, with the layer opened on it placed beside it ([`Placement::Beside`]) instead of below. The
    /// placement travels with the anchor, so a feature asks for it where it builds the anchor and opens its popover
    /// as any other.
    pub fn beside(self) -> Anchor {
        Anchor {
            placement: Placement::Beside,
            ..self
        }
    }

    /// A child menu should stay beside its parent at a narrow window.
    pub(crate) fn submenu(self) -> Anchor {
        Anchor {
            placement: Placement::Submenu,
            ..self
        }
    }

    /// This anchor, with the layer opened on it placed by `place` ([`Placement::By`]) instead of below. Like
    /// [`Anchor::beside`], the placement travels with the anchor, and it is asked again in every frame, so the layer
    /// follows the anchor, its own size and the window as each changes. [`PlaceBy`] says what `place` gets and what it
    /// must not do.
    ///
    /// Only for the terminal's drag-copy notice: a second caller is a decision for the layer manager's owner, since
    /// every other placement here is a named rule from the old app.
    pub fn placed_by(
        self,
        place: impl Fn(Bounds<Pixels>, Size<Pixels>, Bounds<Pixels>) -> Point<Pixels> + 'static,
    ) -> Anchor {
        Anchor {
            placement: Placement::By(Rc::new(place)),
            ..self
        }
    }

    /// This anchor, with the layer opened on it exactly as wide as the anchor element, whatever its content would take:
    /// a select's list matching its control, a floating list matching its field. Like the placement, the width follows
    /// the anchor in the very frame the anchor changes size. The content fills that width and must fit it (clip or
    /// wrap): it is never widened past the anchor.
    ///
    /// Exactly, not at least, because that is what the old app's floating lists did (decided by the GPUI migration's
    /// foundations owner when this was ported, on 2026-11-14): the launcher's folder picker and composer search were
    /// `position: absolute; left: 0; right: 0` under their field, with `min-width: 0` and a path that clips its leading
    /// edge, so a long entry never widened the list (`.launch-composer-folder-options` and
    /// `.launch-composer-search-results` in `app.css` at reference commit `24bc0580`). The old selects differ: a native
    /// `<select>`'s pop-up is at least its control's width on macOS, and the launcher's host list was `min-width: 100%`
    /// up to `min(30em, 80vw)`. A select on this anchor therefore keeps its options inside its control's width, a
    /// deviation from those for the select to record. "At least" would size the list by its content's widest line, so
    /// a list that clips or wraps would grow to that line's length instead of staying under its field.
    pub fn matching_width(self) -> Anchor {
        Anchor {
            matches_width: true,
            ..self
        }
    }

    /// A zero-cost child that records where the anchor element is drawn, every frame, before any overlay is placed. The
    /// anchor element must be positioned (`relative()`) so the tracker covers it. It borrows nothing from the anchor,
    /// so a function that made the anchor may return its tracker ([`beside_edge`] does).
    pub fn track(&self) -> impl IntoElement + use<> {
        self.tracker(None)
    }

    /// Also report whether the anchor intersects its clip in this frame. A caller opening from a deferred action
    /// can then reveal a scrolled-out control before using its bounds for placement.
    pub(crate) fn track_visibility(&self, visible: Rc<Cell<bool>>) -> impl IntoElement + use<> {
        self.tracker(Some(visible))
    }

    fn tracker(&self, visible: Option<Rc<Cell<bool>>>) -> impl IntoElement + use<> {
        let bounds = self.bounds.clone();
        // Pinned to the element's corner: an absolute child without insets would sit wherever it falls in the flow,
        // after the element's other children, and record the wrong place.
        canvas(
            move |at, window, _| {
                bounds.set(Some(at));
                if let Some(visible) = visible {
                    visible.set(window.content_mask().bounds.intersects(&at));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }

    /// Where the anchor element is drawn, in window coordinates, or `None` when it has never been drawn or is no longer
    /// drawn. For a view that places itself against an element another view draws, as the approval region's view
    /// places its card below the session's tabs ([`install_approval_region`]).
    ///
    /// Read from the approval region's render, it is this frame's bounds: that view is laid out after every tracker has
    /// recorded its element. Read anywhere else (a view laid out with the rest of the window, an event handler), it is
    /// the last frame's, since trackers record as a frame is prepared, after the window's layout.
    ///
    /// NOTE: In the one frame that no longer draws the element it still returns the element's last bounds, and it turns
    /// `None` only once that frame has ended, which redraws nothing. So a view must choose which anchor to read from
    /// its own state (the store saying no session is open), not by falling back when one reads `None`: such a fallback
    /// would leave its card where the gone element was until something else redrew.
    pub fn drawn_bounds(&self) -> Option<Bounds<Pixels>> {
        self.state.upgrade()?;
        self.bounds.get()
    }
}

// ---- Opening and closing ---------------------------------------------------------------------------------------------

/// A dialog to open: its view, the focus handle of its container, and where focus starts.
pub struct Dialog {
    /// What the dialog draws, usually a [`crate::components::DialogFrame`] inside a view the feature owns.
    pub view: AnyView,
    /// The dialog's own focus handle. The layer manager tracks it on the element around `view`; the dialog may use it
    /// to tell when its container (rather than one of its controls) has focus.
    pub container: FocusHandle,
    /// Where focus starts. SPEC.md, "Opening and closing", says which control that is; the prompt decides it for
    /// questions.
    pub start: FocusHandle,
    /// What the dialog is about, if anything: it closes when that is gone.
    pub subject: Option<Subject>,
    /// What Enter does while the dialog itself has focus: its enabled main action, if it has exactly one (SPEC.md:
    /// "If a dialog container itself has focus, Enter presses its enabled main action"). `None` does nothing.
    pub main_action: Option<MainAction>,
    /// Whether a primary click on the dialog's empty space (or its label text, which counts as empty space now that
    /// labels are not selectable) keeps focus on the control in use. SPEC.md, "Opening and closing", lists the dialogs
    /// that do: the session launcher, Restart with, the dialogs that share the host and Settings installer, and the
    /// session row and header Stop, Restart, Replace and Delete questions. Every other dialog passes `false`, and such
    /// a click focuses the dialog itself, as a click on a web dialog's background did; Enter then presses its main
    /// action. Per dialog, never a global rule: SPEC.md says which dialogs "do not have this rule yet".
    pub keeps_focus_on_empty_click: bool,
    /// Where focus goes when the dialog closes with focus inside it: [`focus::ReturnTo::Opener`] (what held focus when
    /// it opened) for every dialog SPEC.md does not send elsewhere, and [`focus::ReturnTo::Control`] with the button's
    /// handle for a dialog a button opens, so closing returns to that button also where a click leaves it unfocused
    /// (macOS). An opener names the button here rather than focusing it before opening.
    pub returns_to: focus::ReturnTo,
    /// What the dialog does when the user dismisses it, asked on every dismissal: Escape, wherever focus is inside the
    /// dialog, the dialog itself included; a press outside a dialog opened with [`Dialog::dismiss_on_outside_press`];
    /// and a control the dialog routes through [`dismiss`] (its header Close). [`Dismissal::Stay`] keeps it open and
    /// the dismissal does nothing else (Escape closes no tooltip or lower layer either): for a dialog busy with work it
    /// must not lose, such as an editor whose save is in flight, or one that asks first, such as an editor leaving an
    /// unsaved draft (SPEC.md for the template editor: "Leaving an unsaved draft asks Keep editing, Discard, or Save"),
    /// which opens its question from the hook and closes itself once the answer says so. The hook lives with the layer,
    /// not in the dialog's view, because a key handler in the view would miss the case where the dialog itself holds
    /// focus (after a click on its empty space): the layer manager's element sits around that view.
    ///
    /// Only dismissals ask. A close the opener makes itself ([`close`], on a save that succeeded or on an answer that
    /// says to leave), a subject going away, and a reset of the connection ([`close_all`]) close the dialog whatever
    /// this would say. `None` always closes.
    pub on_dismiss: Option<DismissHook>,
    /// Where the dialog sits: [`DialogPlacement::Centered`] for every dialog but the quick switcher.
    pub placement: DialogPlacement,
    /// Whether a press on the veil, outside the dialog, dismisses it: the old quick switcher closed that way; every
    /// other dialog passes `false`, and its veil only takes the press. It goes through the same path as Escape
    /// ([`dismiss_top`]), so [`Dialog::on_dismiss`] still decides and focus returns as Escape returns it; the veil
    /// still takes the press, so the page behind never sees it. A press on an approval card, drawn above the veil
    /// ([`install_approval_region`]), is the card's and leaves the dialog open, as the old switcher stayed open.
    pub dismiss_on_outside_press: bool,
}

/// A popover to open: a menu or a popup list.
///
/// Build one through its presets, [`Popover::menu`] for a popover that takes focus and [`Popover::floating`] for one
/// that never does, and change a default through a builder ([`Popover::returns_to`], [`Popover::tab_through`],
/// [`Popover::passes_pointer`]). The fields are still public, but a new field gets its default in the presets rather
/// than in every opener's file, so a struct literal is not the way to open one.
pub struct Popover {
    /// What the popover draws.
    pub view: AnyView,
    /// The popover's own focus handle, tracked on the element around `view`.
    pub container: FocusHandle,
    /// Where focus starts (a menu's first item, usually), or `None` for a popover that never takes focus (the module
    /// docs, "A popover may leave focus alone"): opening it moves no focus, Tab is not trapped in it, closing it
    /// returns none, and a press on it leaves focus where it was. Its content must then hold nothing that takes focus,
    /// neither a tab stop nor a focus-tracked control, as the old floating lists' options were `tabindex: -1`: its rows
    /// are plain elements whose owner acts on a press, and the keys stay with the control that holds focus. Its uses
    /// are the launcher's floating lists, and the terminal's overlays on a zero-size anchor element at the pointer or
    /// at a link's cells (the GPUI migration's orchestrator decided on 2026-11-13 that those get no layer kind of their
    /// own, only this popover). Escape closes it from a control that lets Escape go on (a text field), not from a
    /// focused terminal, whose program gets Escape. Nothing here closes it when focus leaves the control it floats
    /// beside (the old launcher closed its lists on Tab and on blur): that is its owner's to do.
    /// A menu opened through [`open_submenu`] can defer its start until [`enter_submenu`] gives it focus ownership.
    pub start: Option<FocusHandle>,
    /// The element it is drawn below (or beside, [`Anchor::beside`]), following it as it moves; it closes when the
    /// element is no longer drawn.
    pub anchor: Anchor,
    /// Where focus goes when it closes with focus inside it, under the same rules as [`Dialog::returns_to`]:
    /// [`focus::ReturnTo::Opener`] (what held focus when it opened) for a popover opened from the control that keeps
    /// focus (a select's list, a field's floating list), and [`focus::ReturnTo::Control`] with the toggle's handle for
    /// a menu a toggle button opens (a session row's ⋯), so Escape or a chosen item returns focus to the toggle also
    /// where the click that opened the menu left the toggle unfocused (macOS). A named control that is no longer drawn,
    /// or that sits outside another layer that still holds focus, falls back to the opener, as for a dialog, and focus
    /// recovers (home, on the page) when the opener was that control itself and it is gone. A popover that never takes
    /// focus ([`Popover::start`] `None`) returns none, so this is never used for one.
    pub returns_to: focus::ReturnTo,
    /// Whether Tab and Shift+Tab stay inside it while it holds focus: `true` for a menu or a select's list (SPEC.md:
    /// "Inside a dialog or menu, focus stays inside"), and every popover but the one below.
    ///
    /// `false` for a list of ordinary controls opened from a toggle that sits in the page's Tab order like the toggle
    /// itself, as SPEC.md has the terminal's tunnel pill and its list: "Escape closes the list and returns focus to
    /// the pill; Tab follows its ordinary controls and leaving the list closes it without reclaiming focus". Such a
    /// popover still takes focus when it opens (its start), and Escape still returns focus where `returns_to` says,
    /// but Tab and Shift+Tab walk its controls in place of a trap and go on out of it, and focus leaving it closes it
    /// with no focus returned (the module docs, "A popover may let Tab through"). It needs `returns_to` naming its
    /// toggle ([`focus::ReturnTo::Control`]): that control is its place in the page's Tab order, its controls coming
    /// right after it, and focus on it does not count as leaving. Without one it has no place: Tab out of it goes
    /// wherever GPUI's window-wide order goes, and only its own controls count as inside it.
    ///
    /// Ignored for a popover that never takes focus ([`Popover::start`] `None`), which traps nothing anyway.
    pub traps_tab: bool,
    /// Whether a press, hover or wheel turn on the popover reaches what it covers, as if it were not there
    /// ([`Popover::passes_pointer`]); `false` for every popover but one opened that way.
    ///
    /// Ignored, and a debug build panics, for a popover with a start ([`Popover::start`] `Some`): one that takes focus
    /// needs the focus tracking and the outside-press close that letting the pointer through leaves out, so it opens
    /// as an ordinary popover instead.
    pub passes_pointer: bool,
}

impl Popover {
    /// A menu or a select's list: it takes focus on `start` when it opens, keeps Tab inside it, and returns focus to
    /// what held it when it opened ([`focus::ReturnTo::Opener`]).
    ///
    /// Those defaults are the shape every focus-taking popover starts from, and the one SPEC.md asks of a menu ("Inside
    /// a dialog or menu, focus stays inside"). A menu a toggle button opens names the toggle with
    /// [`Popover::returns_to`], since on macOS the click that opened it left the toggle unfocused; a list that sits in
    /// the page's Tab order after its toggle adds [`Popover::tab_through`].
    pub fn menu(
        view: impl Into<AnyView>,
        container: FocusHandle,
        anchor: Anchor,
        start: FocusHandle,
    ) -> Self {
        Self {
            view: view.into(),
            container,
            start: Some(start),
            anchor,
            returns_to: focus::ReturnTo::Opener,
            traps_tab: true,
            passes_pointer: false,
        }
    }

    /// A list that floats beside the control the user keeps typing in or pointing at, and never takes focus
    /// ([`Popover::start`] `None`; the module docs, "A popover may leave focus alone"): the launcher's floating lists
    /// and the terminal's displays on a zero-size anchor.
    ///
    /// It moves no focus, traps no Tab and returns none, so neither builder means anything for it: its `returns_to`
    /// and `traps_tab` hold the menu's values only so the struct is complete, and an opener has no reason to change
    /// them. Its content must hold nothing that takes focus, as [`Popover::start`] says.
    pub fn floating(view: impl Into<AnyView>, container: FocusHandle, anchor: Anchor) -> Self {
        Self {
            view: view.into(),
            container,
            start: None,
            anchor,
            returns_to: focus::ReturnTo::Opener,
            traps_tab: true,
            passes_pointer: false,
        }
    }

    /// Return focus to `returns_to` instead of the opener when the popover closes with focus inside it, under the rules
    /// the field of the same name documents: usually [`focus::ReturnTo::Control`] naming the toggle that opened it.
    /// Unused for a popover that never takes focus.
    pub fn returns_to(mut self, returns_to: focus::ReturnTo) -> Self {
        self.returns_to = returns_to;
        self
    }

    /// Let Tab through instead of trapping it ([`Popover::traps_tab`] off): SPEC.md's tunnel list, whose Tab walks its
    /// controls as if they stood in the page right after its toggle and goes on out, and which closes once focus
    /// leaves it. It needs [`Popover::returns_to`] naming that toggle, which gives the list its place in Tab order
    /// (the module docs, "A popover may let Tab through"). Unused for a popover that never takes focus.
    pub fn tab_through(mut self) -> Self {
        self.traps_tab = false;
        self
    }

    /// Let the pointer through to what the popover covers: the exception to "A popover's card takes the pointer from
    /// what it covers" (the module docs). A press, a hover or a wheel turn on it reaches the element under it as if the
    /// popover were not drawn, except on a control inside it that occludes itself (`occlude`), which takes the pointer
    /// alone. Only for a popover that never takes focus ([`Popover::floating`]); on one with a start it is ignored, and
    /// a debug build panics (the field of the same name says why).
    ///
    /// The layer manager registers no hit area for the popover's own wrapper, but it cannot see inside the view. What
    /// else breaks pass-through is what the [`Pill`] NOTE lists, among them a listener, a hover style, a cursor style,
    /// a scrolling container and interactive text. The view keeps to every rule there everywhere but on its occluding
    /// controls.
    ///
    /// What closes it: its owner ([`close`]), its anchor no longer drawn, a reset ([`close_all`]), and a layer below it
    /// that takes focus closing, which takes every layer above it. What never does: a press outside it, and Escape,
    /// which go to the topmost layer the user answers, passing over this one ([`dismiss_top`]). Opening it leaves the
    /// tooltip up, as the pill does, since it covers nothing the user is pointing at.
    ///
    /// It is an ordinary layer on rung 2 of the module docs' z-order ladder (rung 7 over a raised one), in stack order,
    /// not a slot of its own like the pill: a dialog opened while it shows covers it, its veil taking even the presses
    /// on its controls, and one opened over a dialog passes the pointer to that dialog or its veil, never to the page
    /// beneath.
    ///
    /// Its one caller is the terminal's drag-copy notice: SPEC.md has the notice pointer-transparent apart from its ×,
    /// so a press on its words is the terminal's next press, and has it go only on its ×, that press, or its time
    /// running out. A second caller is a decision for the layer manager's owner, as a second display-only popover is a
    /// z-order question too.
    pub fn passes_pointer(mut self) -> Self {
        self.passes_pointer = true;
        self
    }
}

/// A dialog drawn beside an element instead of centered over a veil: placed and followed like a [`Popover`], closed
/// like one when its anchor is no longer drawn or the user presses outside it, and answered like a [`Dialog`] (a
/// subject, Enter on its container, its own empty-space rule). Tab stays inside it, as in every layer. It has no veil:
/// the page behind stays visible, and a press on it closes the dialog unanswered, which for a question is Cancel.
pub struct AnchoredDialog {
    /// What it draws.
    pub view: AnyView,
    /// Its own focus handle, tracked on the element around `view`.
    pub container: FocusHandle,
    /// Where focus starts.
    pub start: FocusHandle,
    /// The element it is drawn below (or beside, [`Anchor::beside`]).
    pub anchor: Anchor,
    /// What it is about, if anything, besides its anchor: it closes when that is gone too.
    pub subject: Option<Subject>,
    /// As [`Dialog::main_action`].
    pub main_action: Option<MainAction>,
    /// As [`Dialog::keeps_focus_on_empty_click`].
    pub keeps_focus_on_empty_click: bool,
    /// As [`Dialog::returns_to`]: [`focus::ReturnTo::Opener`] for a row's questions, and
    /// [`focus::ReturnTo::HomeOnEscape`] for the session header's (SPEC.md: Escape on the header's Stop, Restart,
    /// Replace or Delete question returns focus to the terminal).
    pub returns_to: focus::ReturnTo,
}

/// Where an open dialog sits and whether an outside press dismisses it, as its [`Dialog`] asked.
struct DialogOptions {
    placement: DialogPlacement,
    dismiss_on_outside_press: bool,
    shortcut: Option<DialogShortcut>,
}

/// A dialog-local action also has to work when the layer container holds fallback focus. Put its key context and
/// handler there so the same binding covers the dialog and every control inside it.
struct DialogShortcut {
    context: &'static str,
    action: Box<dyn gpui::Action>,
    run: MainAction,
}

/// Where a dialog sits over its veil ([`Dialog::placement`]).
#[derive(Clone, Copy)]
pub enum DialogPlacement {
    /// Centered in the window, as every dialog of the old app but the quick switcher.
    Centered,
    /// Centered across, its top `min(12% of the window's height, 80 px)` from the window's top and its height at most
    /// 75% of the window's: the old quick switcher (`.quick-switcher-backdrop` and `.quick-switcher-dialog` in
    /// `app.css` at reference commit `24bc0580`), which a dialog that grows with its results keeps from moving. The cap
    /// holds a view that can shrink to it (`flex_1` and `min_h_0` on its root, with its own scrolling list), as the old
    /// dialog's list scrolled inside its `max-height`; a view that cannot shrink draws past the cap, and that part
    /// counts as outside the dialog for presses and focus, so a top-placed view must shrink. A view on
    /// [`crate::components::DialogFrame`] fits only if its root and the frame both shrink, since the frame's own cap is
    /// taller. The old backdrop's 16-pixel side padding is the view's to keep, by capping its own width.
    Top,
}

/// Open `dialog` on top of everything else, unless the user has moved on since `mark` (then nothing opens, and the
/// result belongs where the feature shows it otherwise).
pub fn open_dialog(
    dialog: Dialog,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    open_dialog_inner(dialog, None, mark, window, cx)
}

/// Open a dialog with an action in its own key context, available also from the layer container's fallback focus.
pub(crate) fn open_dialog_with_shortcut(
    dialog: Dialog,
    context: &'static str,
    action: impl gpui::Action,
    run: MainAction,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let shortcut = DialogShortcut {
        context,
        action: Box::new(action),
        run,
    };
    open_dialog_inner(dialog, Some(shortcut), mark, window, cx)
}

fn open_dialog_inner(
    dialog: Dialog,
    shortcut: Option<DialogShortcut>,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let Dialog {
        view,
        container,
        start,
        subject,
        main_action,
        keeps_focus_on_empty_click,
        returns_to,
        on_dismiss,
        placement,
        dismiss_on_outside_press,
    } = dialog;
    open(
        Kind::Dialog(DialogOptions {
            placement,
            dismiss_on_outside_press,
            shortcut,
        }),
        view,
        container,
        Some(start),
        true,
        subject.into_iter().collect(),
        main_action,
        empty_press(keeps_focus_on_empty_click),
        returns_to,
        on_dismiss,
        mark,
        window,
        cx,
    )
}

/// What a press on a dialog's empty space does to focus, by the dialog's rule.
fn empty_press(keeps_focus_on_empty_click: bool) -> focus::OnPress {
    if keeps_focus_on_empty_click {
        focus::OnPress::Keep
    } else {
        focus::OnPress::Focus
    }
}

/// The subject every anchored layer has: its anchor element, which closes it when no longer drawn. `None` when the
/// anchor is already gone, and then there is nowhere to show the layer.
fn drawn(anchor: &Anchor) -> Option<Subject> {
    let state = anchor.state.upgrade()?;
    Some(Subject(Box::new(move |cx, window, id| {
        cx.observe_release(&state, move |_, _, cx| close_later(window, id, cx))
    })))
}

impl Anchor {
    /// The kind of an anchored layer opened on this anchor: placed and sized as the anchor asks, and letting the
    /// pointer through when `passes_pointer` says so (only ever for a popover that never takes focus).
    fn kind(&self, passes_pointer: bool) -> Kind {
        Kind::Popover {
            anchor: self.bounds.clone(),
            surface: Rc::new(Cell::new(None)),
            placement: self.placement.clone(),
            matches_width: self.matches_width,
            passes_pointer,
        }
    }
}

/// Open `dialog` beside its anchor, on top of everything else, unless the user has moved on since `mark` or the anchor
/// is no longer drawn.
pub fn open_anchored(
    dialog: AnchoredDialog,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let AnchoredDialog {
        view,
        container,
        start,
        anchor,
        subject,
        main_action,
        keeps_focus_on_empty_click,
        returns_to,
    } = dialog;
    let drawn = drawn(&anchor)?;
    open(
        anchor.kind(false),
        view,
        container,
        Some(start),
        true,
        std::iter::once(drawn).chain(subject).collect(),
        main_action,
        empty_press(keeps_focus_on_empty_click),
        returns_to,
        None,
        mark,
        window,
        cx,
    )
}

/// Open `popover` on top of everything else, unless the user has moved on since `mark`.
///
/// A popover that takes focus ([`Popover::start`]) and asks to let the pointer through ([`Popover::passes_pointer`]) is
/// a mistake the builders allow: letting the pointer through drops the focus tracking its container needs (Tab's trap,
/// focus recovery and the focus return on close all look for focus inside it) and the outside-press close a menu
/// depends on. A debug build stops at the open that asks for it (the assertion below). A release build fails closed
/// through the one-line normalization after the assertion, which drops the pass-through request whenever there is a
/// start, so the popover opens as an ordinary one. No test runs a release build, so that line is held by review, not by
/// a test.
pub fn open_popover(
    popover: Popover,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let Popover {
        view,
        container,
        start,
        anchor,
        returns_to,
        traps_tab,
        passes_pointer,
    } = popover;
    debug_assert!(
        !passes_pointer || start.is_none(),
        "a popover that lets the pointer through never takes focus (Popover::passes_pointer)"
    );
    let passes_pointer = passes_pointer && start.is_none();
    let drawn = drawn(&anchor)?;
    open(
        anchor.kind(passes_pointer),
        view,
        container,
        start,
        traps_tab,
        vec![drawn],
        None,
        // A menu's empty space (a separator, padding) keeps focus on its item.
        focus::OnPress::Keep,
        returns_to,
        None,
        mark,
        window,
        cx,
    )
}

/// Open a child menu beside its trigger. A hover leaves the keyboard on the trigger until the user enters the child.
/// Its parent is still recorded at open time, so a click in either menu follows the existing layer ancestry rules.
pub(crate) fn open_submenu(
    mut menu: Popover,
    take_focus: bool,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    debug_assert!(matches!(menu.anchor.placement, Placement::Submenu));
    if !take_focus {
        menu.start = None;
    }
    open_popover(menu, mark, window, cx)
}

/// Give the top submenu keyboard ownership. The caller chooses the focused item, which can be the one under the
/// pointer or the first keyboard choice. A stale event cannot enter a closed submenu or reach through another layer.
pub(crate) fn enter_submenu(id: LayerId, window: &Window, cx: &mut App) -> bool {
    let Some(stack) = stack(window, cx) else {
        return false;
    };
    stack.update(cx, |stack, cx| {
        let Some(top) = stack
            .layers
            .iter_mut()
            .rev()
            .find(|layer| !layer.passes_pointer())
        else {
            return false;
        };
        if top.id != id || !top.kind.is_submenu() || top.takes_focus {
            return false;
        }
        top.takes_focus = true;
        cx.notify();
        true
    })
}

/// Make the hovered item eligible for the menu's shared focus move. Crossing into a child grants it the keyboard.
/// A sibling in the parent can close the child and take focus in the same pointer event. That exception applies
/// only to the top submenu and its own parent, so a hover can never dismiss a question over the menu.
pub(crate) fn prepare_menu_hover(item: &FocusHandle, window: &mut Window, cx: &mut App) -> bool {
    let Some(stack) = stack(window, cx) else {
        return false;
    };
    let layers = &stack.read(cx).layers;
    let Some(top) = layers.iter().rev().find(|layer| !layer.passes_pointer()) else {
        return false;
    };
    let top_id = top.id;
    if top.container.contains(item, window) {
        return top.takes_focus || enter_submenu(top_id, window, cx);
    }
    if !top.kind.is_submenu()
        || matches!(&top.returns_to, focus::ReturnTo::Control(trigger) if trigger == item)
        || !layers
            .iter()
            .any(|layer| Some(layer.id) == top.parent && layer.container.contains(item, window))
    {
        return false;
    }
    // The hover will focus the sibling. Returning to the old trigger first would register a second move.
    close_as(top_id, CloseCause::Left, window, cx);
    true
}

/// Keyboard navigation can leave the trigger while its child is still showing. At that point the new item is the
/// parent menu's sole highlight, and the old child's choices no longer belong to it.
pub(crate) fn close_unentered_submenu(window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let layers = &stack.read(cx).layers;
    let Some(top) = layers.iter().rev().find(|layer| !layer.passes_pointer()) else {
        return;
    };
    if !top.kind.is_submenu() || top.takes_focus {
        return;
    }
    if let focus::ReturnTo::Control(trigger) = &top.returns_to
        && !trigger.is_focused(window)
        && layers.iter().any(|layer| {
            Some(layer.id) == top.parent && layer.container.contains_focused(window, cx)
        })
    {
        close(top.id, window, cx);
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the private join of the public open functions"
)]
fn open(
    kind: Kind,
    view: AnyView,
    container: FocusHandle,
    start: Option<FocusHandle>,
    traps_tab: bool,
    subjects: Vec<Subject>,
    main_action: Option<MainAction>,
    empty_press: focus::OnPress,
    returns_to: focus::ReturnTo,
    on_dismiss: Option<DismissHook>,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    if focus::has_moved_on(mark, cx) {
        tracing::debug!(
            "a layer was not opened: the user moved on since the action that asked for it"
        );
        return None;
    }
    let stack = stack(window, cx)?;
    // A layer's container holds focus only as a fallback; Tab moves between its controls, never onto the container.
    let container = container.tab_stop(false);
    let opener = window.focused(cx).map(|focused| focused.downgrade());
    let parent = parent_for_new_layer(stack.read(cx), window, cx);
    let id = stack.update(cx, |stack, cx| {
        let id = LayerId(stack.next_id);
        stack.next_id += 1;
        let window = stack.window;
        let watches = subjects
            .into_iter()
            .map(|subject| (subject.0)(cx, window, id))
            .collect();
        // A layer covers the tooltip of whatever is under it, and the next tooltip waits the full delay again. A
        // popover that lets the pointer through covers nothing the user points at (the drag-copy notice appears well
        // after the release), so, like the pill, it leaves the tooltip and its warm-up alone.
        if !kind.passes_pointer() {
            stack.put_tooltip_away();
        }
        stack.layers.push(Layer {
            id,
            kind,
            view,
            container,
            opener,
            parent,
            takes_focus: start.is_some(),
            traps_tab,
            returns_to,
            on_dismiss,
            main_action,
            empty_press,
            _watches: watches,
            above_region: false,
        });
        cx.notify();
        id
    });
    if start.is_some() && !traps_tab {
        watch_for_leaving(&stack, id, window, cx);
    }
    // A popover that never takes focus leaves it with the control the user is in.
    if let Some(start) = &start {
        focus::enter_layer(start, window, cx);
    }
    Some(id)
}

/// The layer a layer opening now is opened from, its parent, read from where focus is before the open moves it; the
/// rules, in order, and why each is so, are [`close`]'s. Only a layer that holds focus is ever a parent: a popover that
/// never takes focus never held the focus anything was opened from. This sees only focus; a card's opener that leaves
/// focus where it was (macOS) clears the result afterwards ([`opened_from_approval_card`]).
fn parent_for_new_layer(stack: &LayerStack, window: &Window, cx: &App) -> Option<LayerId> {
    let holders = || stack.layers.iter().rev().filter(|layer| layer.takes_focus);
    let holding = holders().find(|layer| {
        layer.container.contains_focused(window, cx)
            || layer
                .place()
                .is_some_and(|place| place.contains_focused(window, cx))
    });
    if let Some(layer) = holding {
        return Some(layer.id);
    }
    if stack.focus_in_region(window, cx) {
        return None;
    }
    holders().next().map(|layer| layer.id)
}

/// Close the popover `id`, which takes focus but lets Tab through ([`Popover::traps_tab`]), once focus has left it:
/// SPEC.md's "leaving the list closes it without reclaiming focus". A focus listener sees every way out (Tab, a press
/// that focuses something else, GPUI Kit's own focus moves), which a check in each of those paths would not.
///
/// Focus on its place (the toggle named in [`Popover::returns_to`]) is not outside it, so focus can leave the popover
/// in two steps (Shift+Tab to the toggle, then on to the control before it), and the toggle is watched as well as the
/// popover. Focus moving into a layer opened above it (a question one of its controls asked) does not close it either;
/// this listener sees only focus, and a press inside such a layer still closes the popover, and the layer with it, as
/// a press outside it ([`overlay`]).
///
/// GPUI tells focus listeners while it finishes the frame that shows the move, so the listener only queues the check,
/// which runs once that frame is done and reads where focus is then rather than what the event said: the event also
/// fires when the window stops being active (switching apps), which moves no focus, and when the focused control
/// vanished and the focus coordinator's recovery, which GPUI runs just before, has already put focus on the closest
/// control left inside the popover (a closed tunnel's row).
fn watch_for_leaving(stack: &Entity<LayerStack>, id: LayerId, window: &mut Window, cx: &mut App) {
    let Some((container, place)) = stack
        .read(cx)
        .layers
        .iter()
        .find(|layer| layer.id == id)
        .map(|layer| (layer.container.clone(), layer.place().cloned()))
    else {
        return;
    };
    let watches: Vec<Subscription> = std::iter::once(container)
        .chain(place)
        .map(|watched| {
            window.on_focus_out(&watched, cx, move |_, window, cx| {
                window.defer(cx, move |window, cx| close_if_left(id, window, cx));
            })
        })
        .collect();
    stack.update(cx, |stack, _| {
        if let Some(layer) = stack.layers.iter_mut().find(|layer| layer.id == id) {
            layer._watches.extend(watches);
        }
    });
}

/// [`watch_for_leaving`]'s check: close `id` if it is still open and focus is neither inside it, inside a layer above
/// it, nor on its place.
fn close_if_left(id: LayerId, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let left = {
        let stack = stack.read(cx);
        let Some(index) = stack.layers.iter().position(|layer| layer.id == id) else {
            return;
        };
        let on_place = stack.layers[index]
            .place()
            .is_some_and(|place| place.contains_focused(window, cx));
        !on_place
            && !stack.layers[index..]
                .iter()
                .any(|layer| layer.container.contains_focused(window, cx))
    };
    if left {
        close_as(id, CloseCause::Left, window, cx);
    }
}

impl Layer {
    /// Whether this layer is a popover that lets the pointer through ([`Popover::passes_pointer`]).
    fn passes_pointer(&self) -> bool {
        self.kind.passes_pointer()
    }

    /// Where a popover that lets Tab through sits in the page's Tab order: the control its opener named for focus to
    /// return to, its toggle ([`Popover::traps_tab`]). `None` for any other layer.
    fn place(&self) -> Option<&FocusHandle> {
        match &self.returns_to {
            focus::ReturnTo::Control(control) if self.takes_focus && !self.traps_tab => {
                Some(control)
            }
            _ => None,
        }
    }
}

/// Close `id` in `window` from inside the stack's own update (a subject going, an anchor's release), once that update
/// has returned: closing updates the stack, which cannot be updated from inside itself.
fn close_later(window: AnyWindowHandle, id: LayerId, cx: &mut App) {
    cx.defer(move |cx| {
        // The window may have closed meanwhile; then there is nothing left to close.
        let _ = window.update(cx, |_, window, cx| close(id, window, cx));
    });
}

/// Close the layer `id`, if it is still open, and every layer opened from it, directly or from one of those: its
/// descendants. A layer another owner opened above it stays open, in its place in the stack, and so does that layer's
/// raise above the approval region ([`above_approval_region`]), which is worked out from stack positions every frame.
/// If focus was inside what closed, it returns to what held it when `id` opened, or where `id`'s opener said instead
/// ([`Dialog::returns_to`], [`Popover::returns_to`]), unless a layer that holds focus is left open above it: then focus
/// goes into that layer, as when its own focused control goes ([`focus::return_to`]'s recovery), since what `id`'s
/// opener named sits under that layer now. A popover that never took focus is passed over: closing one moves no focus.
///
/// Which layer a layer was opened from, its parent, is decided as it opens, from where focus is then, by the first of
/// the focus rules below that applies; an opener's own word that it opened from an approval card (rule 2) then holds
/// whatever the focus rules decided:
///
/// 1. The topmost layer that holds focus and contains the focused control, or whose toggle holds it (a popover that
///    lets Tab through, [`Popover::traps_tab`]): focus stays inside the topmost such layer ([`trap`]), so a menu opened
///    from one of a dialog's controls, or a question from a menu's item, was opened from there and goes with it.
/// 2. No layer, when the layer was opened from an approval card ([`install_approval_region`]): its opener says so
///    ([`opened_from_approval_card`]), or focus is on the card. The cards stay usable over a dialog, and what a card
///    opens (its tunnel warning, the agent's Add host) is about that card's request, not about the dialog under it.
///    SPEC.md: "Nothing closes a dialog, menu, popup list or confirmation that it did not open", so the dialog's owner
///    closing it (a bulk question whose targets are gone) must leave the card's layer open. Focus alone cannot tell:
///    on macOS a press on a card's button moves no focus, which stays in the dialog under the card, and rule 1 would
///    make that dialog the parent.
/// 3. The topmost layer that holds focus, when focus is anywhere else: nowhere, or on a control no frame has drawn yet
///    (the start of a layer opened earlier in the same update), and the topmost such layer is the one the user is in.
/// 4. No layer, when none holds focus: a layer opened from the page.
///
/// So only a layer that holds focus is ever a parent. A popover that never takes focus owns nothing: focus was never in
/// it, so a dialog over it was opened from the page or the layer beneath (a shortcut pressed while a floating list or a
/// link's disclosure showed), and the popover's going (its owner closing it, a press outside it, its anchor no longer
/// drawn) must leave that dialog open with focus inside it. A layer opened by a click on the popover's own content and
/// anchored there still goes with it, since its anchor is no longer drawn; a centered dialog opened that way stays.
///
/// Every close but [`close_all`] comes here, whatever its cause: the opener's own call, a subject or anchor going
/// ([`Subject`]), Escape and a dialog's Close ([`dismiss`]), Tab out of a caret menu ([`close_on_tab`]), and a press
/// outside a popover.
pub fn close(id: LayerId, window: &mut Window, cx: &mut App) {
    close_as(id, CloseCause::Closed, window, cx);
}

/// Close the menu `id` because Tab or Shift+Tab left it, for a menu whose own rule sends Tab back to its toggle rather
/// than keeping it inside (SPEC.md for the session header's caret menus: "Escape or Tab closes the menu and returns
/// focus to its caret"). A held Tab stops at the caret. The next fresh Tab moves to the next control.
pub fn close_on_tab(id: LayerId, window: &mut Window, cx: &mut App) {
    close_as(id, CloseCause::TabbedOut, window, cx);
}

/// Why a layer is closing, which decides where focus goes after it and which repeats of the closing key are swallowed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CloseCause {
    /// Its opener closed it, its subject or anchor went, a reset, or a press outside a popover that traps Tab: focus
    /// returns as the layer's `returns_to` says.
    Closed,
    /// The user dismissed it (Escape, a dialog's Close): as `Closed`, except that a layer returning focus home on
    /// Escape only ([`focus::ReturnTo::HomeOnEscape`]) sends it home.
    Dismissed,
    /// Tab or Shift+Tab left a menu whose rule returns focus to its toggle ([`close_on_tab`]): as `Closed`, except that
    /// the key is navigation that happens to close the menu, so only its held repeats are swallowed.
    TabbedOut,
    /// Focus moves independently of the close. A hover on a submenu sibling takes focus immediately afterward.
    /// A popover that lets Tab through also closes this way when focus leaves or a press lands outside it. If that
    /// press took no focus, recovery moves it on when the old control disappears.
    Left,
}

/// [`close`], saying why the layer closes ([`CloseCause`]). When focus is returned, the cause also decides which repeats
/// of the key press that closed it the focus coordinator swallows (see [`focus::return_to`]): for `Closed` and
/// `Dismissed`, the press's held repeats (except Enter) and a fresh Escape within the repeat interval if no layer
/// remains open or opens next. For `TabbedOut`, only held repeats can be swallowed. A close that
/// returns no focus swallows nothing, which is always so for `Left`.
fn close_as(id: LayerId, why: CloseCause, window: &mut Window, cx: &mut App) {
    close_taking(why, window, cx, |layers| {
        let closing = layer_branch(layers, id)?;
        // `extract_if` keeps stack order on both sides: the layers left keep theirs, and the closed ones come out bottom
        // to top, as `close_taking` reads them.
        Some(
            layers
                .extract_if(.., |layer| closing.contains(&layer.id))
                .collect(),
        )
    });
}

/// The layer and its descendants in stack order. A parent always sits
/// below the layers it opened, so one pass finds the whole branch.
fn layer_branch(layers: &[Layer], id: LayerId) -> Option<Vec<LayerId>> {
    let index = layers.iter().position(|layer| layer.id == id)?;
    let mut branch = vec![id];
    for layer in &layers[index + 1..] {
        if layer.parent.is_some_and(|parent| branch.contains(&parent)) {
            branch.push(layer.id);
        }
    }
    Some(branch)
}

/// GPUI sends outside-press capture to a parent even for a press on a
/// submenu beside it. Only a drawn descendant that takes the pointer
/// makes that position part of the parent's interactive menu tree.
fn on_descendant_popover(id: LayerId, position: Point<Pixels>, window: &Window, cx: &App) -> bool {
    let Some(stack) = stack(window, cx) else {
        return false;
    };
    let layers = &stack.read(cx).layers;
    let Some(branch) = layer_branch(layers, id) else {
        return false;
    };
    layers.iter().any(|layer| {
        layer.id != id
            && branch.contains(&layer.id)
            && matches!(&layer.kind, Kind::Popover { surface, passes_pointer: false, .. }
                if surface.get().is_some_and(|bounds| bounds.contains(&position)))
    })
}

/// Take the layers `take` picks out of `window`'s stack, then return focus as the lowest of them that took it says, or
/// into the layer that holds focus left open above them, if one is ([`close`]). The one place layers leave the stack,
/// so [`close`] and [`close_all`] share the focus return. `take` gets the stack's layers, bottom to top, leaves the
/// ones that stay in their order, and answers the ones it took, bottom to top, or `None` when there is nothing to
/// close.
fn close_taking(
    why: CloseCause,
    window: &mut Window,
    cx: &mut App,
    take: impl FnOnce(&mut Vec<Layer>) -> Option<Vec<Layer>>,
) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let closed = stack.update(cx, |stack, cx| {
        let closed = take(&mut stack.layers)?;
        cx.notify();
        Some(closed)
    });
    let Some(closed) = closed else {
        return;
    };
    // A hover preview leaves focus on its trigger, so there is no focus return to remember the dismissal key.
    // Record it here to keep a held Escape from going on to dismiss the parent.
    if why == CloseCause::Dismissed
        && closed.iter().any(|layer| {
            !layer.takes_focus
                && layer.kind.is_submenu()
                && matches!(&layer.returns_to, focus::ReturnTo::Control(trigger)
                    if trigger.is_focused(window))
        })
    {
        focus::note_closing(true, window, cx);
    }
    // A dismissal from a popover's place (Escape on the tunnel pill while its list is open, [`Popover::traps_tab`])
    // counts as from inside it: focus "returns" to the place it is already on, and the coordinator notes the Escape
    // as the closing press, so its repeats are swallowed rather than dismissing each layer below in turn. Only for a
    // dismissal: the toggle closing its own list (Enter or Space on the pill) must not swallow a quick second press
    // that reopens it.
    let focus_was_inside = closed.iter().any(|layer| {
        layer.container.contains_focused(window, cx)
            || (why == CloseCause::Dismissed
                && layer
                    .place()
                    .is_some_and(|place| place.contains_focused(window, cx)))
    });
    // Focus goes back as the lowest closed layer that took it says: a popover that never took focus has none to give
    // back, and the control the user stayed in keeps it.
    if why != CloseCause::Left
        && (focus_was_inside || window.focused(cx).is_none())
        && let Some(bottom) = closed.iter().find(|layer| layer.takes_focus)
    {
        // A layer that holds focus left open above the closed one was opened by another owner (an approval card's
        // warning over a dialog whose owner closed it). Its veil covers whatever the closed layer would return focus
        // to, so focus goes into it instead, through the recovery a return with nowhere to go makes: the closest
        // control inside the topmost layer that holds focus.
        let covered = stack
            .read(cx)
            .layers
            .iter()
            .any(|layer| layer.takes_focus && layer.id.opened_after(bottom.id));
        if covered {
            focus::return_to(
                focus::ReturnTo::Opener,
                None,
                why == CloseCause::TabbedOut,
                window,
                cx,
            );
            return;
        }
        let returns_to = match &bottom.returns_to {
            focus::ReturnTo::HomeOnEscape if why == CloseCause::Dismissed => focus::ReturnTo::Home,
            focus::ReturnTo::HomeOnEscape => focus::ReturnTo::Opener,
            other => other.clone(),
        };
        focus::return_to(
            returns_to,
            bottom.opener.as_ref(),
            why == CloseCause::TabbedOut,
            window,
            cx,
        );
    }
}

/// Close every dialog and popover in every window, as a reset of the connection to the helm requires (SPEC.md,
/// "Losing the connection to the helm": open dialogs and menus start over). The UI-session owner calls it when the
/// window goes inert.
///
/// This is the one close that does not need the layer's id: a reset is not any opener's decision, and a dialog that
/// survived it would keep its veil over the inert window, where it takes the clicks meant for Take back or Retry and
/// traps Tab away from them. Focus follows the ordinary close rule, returning to what held it when the bottom layer
/// that took focus opened (and going home once that is no longer drawn). Pills go too: their views belonged to
/// the UI session the reset retired, and their features, which live
/// in that session, are no longer there to hide them. The tooltip is left alone: it goes with its control. So is the
/// approval region ([`install_approval_region`]): its feature installs it once per window, and its view draws from the
/// current UI session, which after a reset has no cards until the new session brings them.
pub fn close_all(cx: &mut App) {
    let Some(windows) = installed::<Windows>(cx) else {
        return;
    };
    let stacks: Vec<Entity<LayerStack>> =
        windows.0.values().filter_map(WeakEntity::upgrade).collect();
    for stack in stacks {
        stack.update(cx, |stack, cx| {
            // Eager evaluation empties every slot even if the first also held a pill.
            if stack.pill.take().is_some()
                | stack.launch_pill.take().is_some()
                | stack.terminal_refusal.take().is_some()
            {
                cx.notify();
            }
        });
        let window = stack.read(cx).window;
        // Every layer, not a close of the bottom one: when that is a popover that never takes focus, it closes alone.
        // A window that closed meanwhile has nothing left to close.
        let _ = window.update(cx, |_, window, cx| {
            close_taking(CloseCause::Closed, window, cx, |layers| {
                (!layers.is_empty()).then(|| std::mem::take(layers))
            })
        });
    }
}

/// The user closes the topmost layer they answer (Escape, [`top_layer`]), or the tooltip if no such layer is open.
/// Returns whether Escape was taken, so the key can go on to whatever else wants it when nothing is open: a topmost
/// dialog whose hook keeps it open ([`Dialog::on_dismiss`]) takes it too. A popover that lets the pointer through is
/// passed over, so with only such a popover open Escape goes on as if it were not there.
pub fn dismiss_top(window: &mut Window, cx: &mut App) -> bool {
    let Some(stack) = stack(window, cx) else {
        return false;
    };
    if let Some(top) = top_layer(window, cx) {
        dismiss(top, window, cx);
        return true;
    }
    stack.update(cx, |stack, cx| {
        let shown = stack.tooltip.is_some();
        stack.put_tooltip_away();
        if shown {
            cx.notify();
        }
        shown
    })
}

/// The user dismisses the layer `id` (a dialog's header Close, or Escape through [`dismiss_top`]): its hook decides
/// ([`Dialog::on_dismiss`]), and it closes unless the hook keeps it open. Nothing happens if `id` is no longer open.
///
/// The hook runs outside the stack's own update, so it may open a question over the dialog (or close the dialog
/// itself); its answer is acted on only if the layer is still open after it.
///
/// NOTE: Call it from a plain event handler, not from inside an update of the dialog's own view: a hook that reads the
/// view (Rename's reads its draft) would find it already being updated, which GPUI refuses with a panic.
pub fn dismiss(id: LayerId, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let Some(hook) = stack
        .read(cx)
        .layers
        .iter()
        .find(|layer| layer.id == id)
        .map(|layer| layer.on_dismiss.clone())
    else {
        return;
    };
    let dismissal = hook.map_or(Dismissal::Close, |hook| hook(window, cx));
    if dismissal == Dismissal::Close && is_open(id, window, cx) {
        close_as(id, CloseCause::Dismissed, window, cx);
    }
}

/// The topmost layer the user answers in `window` (Escape, a dismissible veil's outside press), if any is open: any but
/// a popover that lets the pointer through, which its owner alone closes ([`Popover::passes_pointer`]). Both callers
/// ask this one question, so a display-only popover above the quick switcher neither takes Escape from it nor keeps a
/// press on its veil from dismissing it.
fn top_layer(window: &Window, cx: &App) -> Option<LayerId> {
    stack(window, cx)?
        .read(cx)
        .layers
        .iter()
        .rev()
        .find(|layer| !layer.passes_pointer())
        .map(|layer| layer.id)
}

/// Whether the layer `id` is still open.
pub fn is_open(id: LayerId, window: &Window, cx: &App) -> bool {
    stack(window, cx).is_some_and(|stack| stack.read(cx).layers.iter().any(|layer| layer.id == id))
}

/// Whether later surfaces block pointer input over any of a rendered panel's hitbox. GPUI supplies the order and
/// clipping, so every occluding surface counts without having to register a second set of layer bounds. `None`
/// means the panel's hitbox no longer belongs to the last rendered frame.
pub(crate) fn pointer_covers(panel: HitboxId, window: &Window) -> Option<bool> {
    panel.is_occluded(window)
}

/// Whether any dialog or popover is open in `window`. The display-only slots never count as open layers.
pub fn any_open(window: &Window, cx: &App) -> bool {
    stack(window, cx).is_some_and(|stack| !stack.read(cx).layers.is_empty())
}

/// Whether a layer that holds focus is open in `window`: any but a popover that never takes focus. Where the focus
/// coordinator keeps focus from leaving the topmost layer for the page behind it, this is the test, since with only
/// such a popover open focus is on the page already.
///
/// NOTE: A shortcut that refuses while something modal is open (Rename, Delete, the quick switcher) must ask this, not
/// [`any_open`]. A popover that never takes focus (a floating list, the terminal's link destination display) is not
/// modal: [`any_open`] counts it, so a shortcut asking that would stop working while one shows. The dialog such a
/// shortcut opens then sits above the popover, which is why closing the popover closes only itself ([`close`]).
///
/// A popover that lets Tab through ([`Popover::traps_tab`]) counts, though it is not modal either: what this answers is
/// whether a layer holds the keyboard, which is how its callers already treat a menu, and it does. Counting it keeps
/// the shortcuts and the page's late focus moves ([`focus::focus_control`], [`focus::choose_terminal`]) from acting
/// while the user is in it, as they keep from acting in a menu, rather than opening a dialog over a list the user is
/// tabbing through or pulling focus out of it to the page. That holds while focus is on the list's toggle too (the
/// pill, which does not count as leaving the list): with the list open, Cmd+W, Rename and the quick switcher refuse
/// there, and the page's late focus moves wait, until the list closes. It is the same test the focus coordinator's
/// recovery uses (`top_container`), so a focused control going away inside it (a closed tunnel's row) leaves focus on
/// the list's closest control rather than sending it home.
pub fn focus_layer_open(window: &Window, cx: &App) -> bool {
    top_container(window, cx).is_some()
}

/// The container Tab must stay inside: the topmost layer's, dialog or menu alike (SPEC.md: "Inside a dialog or menu,
/// focus stays inside"). A menu opened over a dialog holds Tab until it closes.
///
/// NOTE: A popover that never takes focus ([`Popover::start`] `None`) traps nothing: Tab goes on from the control that
/// holds focus as if the popover were not open, inside whatever layer below it holds focus (the launcher dialog around
/// a floating list), or across the page. A list that keeps focus in its field (the launcher's folder picker, a
/// suggestion list) is that popover, not a menu with a flag. A popover that lets Tab through ([`Popover::traps_tab`]
/// off) traps nothing either: the trap is the topmost layer below it that does, if any, and Tab walks the popover
/// itself as [`tab_through`] says.
pub fn trap(window: &Window, cx: &App) -> Option<FocusHandle> {
    let stack = stack(window, cx)?;
    stack
        .read(cx)
        .layers
        .iter()
        .rev()
        .find(|layer| layer.takes_focus && layer.traps_tab)
        .map(|layer| layer.container.clone())
}

/// A popover that lets Tab through, as the focus coordinator walks it: its container and its place in the page's Tab
/// order ([`Popover::traps_tab`]).
pub(crate) struct TabThrough {
    /// The popover's container, around every control of it.
    pub(crate) container: FocusHandle,
    /// The toggle it was opened from, which its controls follow in Tab order; `None` when its opener named none.
    pub(crate) place: Option<FocusHandle>,
}

/// The topmost layer that holds focus, when it lets Tab through ([`Popover::traps_tab`] off): the one popover whose
/// controls Tab walks in the page's order rather than inside a trap.
///
/// GPUI's Tab order is window-wide, by tab index and then by the order elements are painted, and every overlay is
/// painted after the window's content, so in GPUI's own order a popover's controls come after every control on the
/// page, whatever its anchor is. SPEC.md's tunnel list follows its pill instead, as the old app's list did, which was
/// drawn inside the pill's own element: Shift+Tab from its first control reaches the pill, and Tab from its last goes
/// on to the control after the pill. The coordinator splices the popover's controls in after its place to give that
/// order; this hands it what it needs.
pub(crate) fn tab_through(window: &Window, cx: &App) -> Option<TabThrough> {
    let stack = stack(window, cx)?;
    let stack = stack.read(cx);
    let top = stack.layers.iter().rev().find(|layer| layer.takes_focus)?;
    (!top.traps_tab).then(|| TabThrough {
        container: top.container.clone(),
        place: top.place().cloned(),
    })
}

/// Whether focus is inside the topmost dialog, a layer above it (a menu opened from inside the dialog) or the approval
/// region: where SPEC.md lets focus be while a dialog is open. `true` when no dialog is open.
pub fn focus_is_where_dialogs_allow(window: &Window, cx: &App) -> bool {
    let Some(stack) = stack(window, cx) else {
        return true;
    };
    let stack = stack.read(cx);
    let Some(top_dialog) = stack
        .layers
        .iter()
        .rposition(|layer| matches!(layer.kind, Kind::Dialog(_)))
    else {
        return true;
    };
    // SPEC.md, "Keyboard focus": approval cards "remain usable" while a dialog is open, so focus on one of their
    // controls is allowed too ([`install_approval_region`]).
    stack.layers[top_dialog..]
        .iter()
        .any(|layer| layer.container.contains_focused(window, cx))
        || stack.focus_in_region(window, cx)
}

/// The container of the topmost layer that holds focus, where focus recovers to when its holder disappears inside an
/// open layer. A popover that never takes focus is passed over, so focus never recovers into one; a popover that lets
/// Tab through ([`Popover::traps_tab`]) holds focus and is not passed over.
pub(crate) fn top_container(window: &Window, cx: &App) -> Option<FocusHandle> {
    let stack = stack(window, cx)?;
    stack
        .read(cx)
        .layers
        .iter()
        .rev()
        .find(|layer| layer.takes_focus)
        .map(|layer| layer.container.clone())
}

/// What breaks the overlay rules in `window` right now, if anything: a popover anchored to an element that was never
/// drawn has nowhere to be, and a popover that never takes focus must not hold it. The test harness runs this after
/// every step.
pub fn invariant_violation(window: &Window, cx: &App) -> Option<String> {
    let stack = stack(window, cx)?;
    stack
        .read(cx)
        .layers
        .iter()
        .find_map(|layer| match &layer.kind {
            Kind::Popover { anchor, .. } if anchor.get().is_none() => Some(format!(
                "popover {:?} is open but its anchor was never drawn",
                layer.id
            )),
            _ if !layer.takes_focus && layer.container.contains_focused(window, cx) => Some(
                format!("popover {:?} never takes focus but holds it", layer.id),
            ),
            _ => None,
        })
}

// ---- The dialog opened last, for tests -------------------------------------------------------------------------------

/// The last view of type `T` that an open function recorded ([`record_opened`]): one global per type, so each dialog
/// has its own slot and recording one never touches another's.
///
/// Weak, so the record never keeps a closed dialog alive: a test that checks that a closed dialog is gone (its view
/// dropped with its layer) would otherwise always fail.
#[cfg(test)]
struct Opened<T>(WeakEntity<T>);

#[cfg(test)]
impl<T: 'static> Global for Opened<T> {}

/// Record `view` as the last view of its type opened, so tests and golden scenes can reach the dialog (or other
/// layer's view) an open function made inside itself, and read or drive its fields. Test builds only: an open
/// function calls it under `#[cfg(test)]` right after its open succeeds, so a refused open records nothing.
///
/// Each open records over the previous one of the same type. A new dialog records itself here, under `#[cfg(test)]`
/// in its open function, rather than declaring a global of its own.
#[cfg(test)]
pub(crate) fn record_opened<T: 'static>(view: &Entity<T>, cx: &mut App) {
    cx.set_global(Opened(view.downgrade()));
}

/// The last view of type `T` recorded by [`record_opened`], if one was recorded and is still alive. `None` cannot tell
/// "never opened" from "opened and since dropped"; [`last_opened_weak`] can.
#[cfg(test)]
pub(crate) fn last_opened<T: 'static>(cx: &App) -> Option<Entity<T>> {
    last_opened_weak(cx).and_then(|view| view.upgrade())
}

/// The record [`record_opened`] keeps for type `T`, whether or not the view is still alive: `None` only when no view
/// of that type was recorded in this app. For the tests that check that nothing opened at all, that a closed view is
/// gone, or that a second opening made a new view.
#[cfg(test)]
pub(crate) fn last_opened_weak<T: 'static>(cx: &App) -> Option<WeakEntity<T>> {
    cx.try_global::<Opened<T>>().map(|opened| opened.0.clone())
}

// ---- The pill --------------------------------------------------------------------------------------------------------

/// A display-only element to draw above every layer ([`show_pill`]): SPEC.md's quick-launch sequence pill, with its
/// candidate list, which must stay in view over the launcher's veil yet take neither focus nor pointer input.
///
/// NOTE: The view must ask for no pointer input. GPUI hands a press, a click and a wheel to every element under the
/// pointer up to the first that blocks them, so a press on a view that asks for nothing passes through to whatever it
/// covers, as the old pill's `pointer-events: none` did. Anything in the view that does ask breaks that: `occlude` (or
/// `block_mouse_except_scroll`) blocks the press from what is below; a listener of any kind, a hover or group-hover
/// style, `.group(..)`, hover help or a tooltip makes the pill react to the pointer as well; a cursor style replaces
/// the cursor of what is below, since the topmost one wins; a scrolling container (`overflow_*_scroll`,
/// `track_scroll`) takes the wheel; `list` and `uniform_list` always ask, and so does interactive text; and a drop
/// target or drag-over style reacts to a drag passing over it. The layer manager cannot see inside the view to check
/// any of this. Nothing in it can hold focus either: it opens no layer, and no control in it may be focus-tracked, so
/// its keys come from the feature's own capture, not from the view. All of this holds for a launch pill's view too
/// ([`show_launch_pill`]).
pub struct Pill {
    /// What the pill draws. The feature updates it in place (a notify on its view redraws it, as any view) rather than
    /// showing a new pill for every change.
    pub view: AnyView,
    /// The element it is placed over, which must stay drawn for as long as the pill should show (see [`show_pill`]).
    /// The anchor's own placement and width options are for layers and do not apply here.
    pub anchor: Anchor,
    /// Where over the anchor it sits.
    pub placement: PillPlacement,
}

/// Identifies one shown pill, in any slot. Returned by the slot's show function; the only way to hide
/// that pill, as a [`LayerId`] is the only way to close its layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PillId(u64);

/// Where a pill sits over its anchor element. The view is laid out in a box the anchor's size, so it is as wide as its
/// content up to the anchor's width, and a view that must stay clear of the anchor's edges pads itself (padding asks
/// for no pointer input); one larger than the anchor overflows it evenly on both sides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PillPlacement {
    /// Centered over the anchor, both ways: the sequence pill outside the launcher (the old
    /// `.launch-sequence-indicator`, `left: 50%; top: 50%; transform: translate(-50%, -50%)`).
    Centered,
    /// Centered across the anchor, its bottom edge on the anchor's: the sequence pill while the launcher is open, at
    /// the bottom of the main area so it does not cover the launcher. The old one sat 14 pixels above that edge (44
    /// with the details bar's reservation); the view keeps such a gap as its own bottom padding.
    Bottom,
}

/// The pill a window shows: what [`show_pill`] was given, with the anchor reduced to where it was last drawn.
struct ShownPill {
    id: PillId,
    view: AnyView,
    anchor: AnchorBounds,
    placement: PillPlacement,
    /// Takes the pill down when its anchor element is no longer drawn. Dropped with the pill, so a pill shown on a new
    /// anchor is not taken down by the old one going.
    _drawn: Subscription,
}

/// Show `pill` in `window`, above every layer (rung 8 of the module docs' z-order ladder), replacing any pill showing
/// there. Returns the id that hides it, or `None` when its anchor element is already gone (there is nowhere to place
/// it) or the window has no layer stack.
///
/// One slot per window for this pill: showing replaces whatever pill shows in it, and [`hide_pill`] hides only the pill
/// its id names, so a feature that hides a pill another show has replaced does nothing. SPEC.md arms one sequence at a
/// time, so one slot is all it needs. The launch pill has a slot of its own ([`show_launch_pill`]), and neither show
/// replaces the other's pill.
///
/// It is not a layer and changes nothing that layers decide: showing it moves no focus, takes no tooltip down, and is
/// not counted by [`any_open`], [`focus_layer_open`], [`trap`] or Escape's [`dismiss_top`]. A layer opening over it
/// does not hide it either; the sequence's own rules say when a dialog opening cancels it. It takes no [`Mark`]: it is
/// not a layer and takes nothing from the user, and its feature shows it from the keystroke that arms the sequence.
///
/// It goes when its feature hides it, on a reset ([`close_all`]), and when its anchor element is no longer drawn, with
/// no word to the feature. So the feature anchors it to the element SPEC.md names for the current state: centered over
/// the terminal area, or the main area with no session open. Best is one element drawn under the same key in both
/// states (the terminal area's box while a session is open, the main area's otherwise), whose anchor then survives a
/// session opening or closing and re-centers the pill in that frame. If they are different elements, the feature shows
/// the pill again on the new anchor when a session opens or closes (from an observer of the selected session, which
/// [`crate::local_state`] notifies), or the pill is gone until then. [`pill_shown`] is there for a feature that must
/// check.
pub fn show_pill(pill: Pill, window: &mut Window, cx: &mut App) -> Option<PillId> {
    show_anchored_pill(pill, |stack| &mut stack.pill, window, cx)
}

/// Show a terminal refusal over the approval cards when no layer holds the keyboard. The caller anchors it to the
/// line's normal place and supplies a view without input, so the approval cards' controls keep working underneath.
pub(crate) fn show_terminal_refusal(
    view: AnyView,
    anchor: Anchor,
    window: &mut Window,
    cx: &mut App,
) -> Option<PillId> {
    show_anchored_pill(
        Pill {
            view,
            anchor,
            placement: PillPlacement::Centered,
        },
        |stack| &mut stack.terminal_refusal,
        window,
        cx,
    )
}

/// Anchored slots share one lifetime rule: the slot's own subscription retires it when its anchor leaves the frame.
fn show_anchored_pill(
    pill: Pill,
    slot: fn(&mut LayerStack) -> &mut Option<ShownPill>,
    window: &mut Window,
    cx: &mut App,
) -> Option<PillId> {
    let Pill {
        view,
        anchor,
        placement,
    } = pill;
    let state = anchor.state.upgrade()?;
    let stack = stack(window, cx)?;
    let id = stack.update(cx, |stack, cx| {
        let id = PillId(stack.next_pill_id);
        stack.next_pill_id += 1;
        let drawn = cx.observe_release(&state, move |stack, _, cx| {
            *slot(stack) = None;
            cx.notify();
        });
        *slot(stack) = Some(ShownPill {
            id,
            view,
            anchor: anchor.bounds,
            placement,
            _drawn: drawn,
        });
        cx.notify();
        id
    });
    Some(id)
}

/// The launch pill a window shows: what [`show_launch_pill`] was given. It has no anchor: the window places it.
struct ShownLaunchPill {
    id: PillId,
    view: AnyView,
}

/// How far the launch pill keeps from the window's bottom and side edges: the old `.launch-progress-pill`'s
/// `bottom: 20px` and `max-width: calc(100vw - 40px)`, centered by `left: 50%; transform: translateX(-50%)`.
const LAUNCH_PILL_INSET: Pixels = px(20.);

/// Show `view` as `window`'s launch pill, replacing any launch pill showing there: SPEC.md's launch progress pill
/// ("Launching", "Restarting" or "Replacing", the host, the session name and the elapsed seconds) and the "Started"
/// report that follows it when nothing navigates. Returns the id that hides it ([`hide_pill`]), or `None` when the
/// window has no layer stack.
///
/// The second pill slot. It shows beside the sequence pill ([`show_pill`]) without replacing it or being replaced by
/// it, so a sequence armed while a launch pill shows leaves both up, and each feature hides only its own. The progress
/// pill and the "Started" report share this slot. The old app drew them as two elements with the same class at the same
/// fixed place (`list/create_form.rs` and `list/view.rs` at reference commit `24bc0580`), so a launch whose progress
/// pill appeared while a report still showed had the report, later in the page, covering it until the report's four
/// seconds ran out. Here each show replaces the other, so the launcher decides which shows. The [`Pill`] rules hold for
/// `view`: it must ask for no pointer input and hold no focus.
///
/// It is placed against the window, not an element, as the old pill was `position: fixed`: centered across the window,
/// its bottom edge 20 pixels above the window's, and at most the window's width less 20 pixels a side. The view is as
/// wide as its content up to that cap and keeps its text inside it (clip or wrap); a view wider than the cap overflows
/// it. Having no anchor, it goes only when its feature hides it or on a reset ([`close_all`]).
///
/// It is drawn above every layer but a raised one ([`above_approval_region`]), on rung 3 of the module docs' z-order
/// ladder, which gives the old z-indices around it. Like the pill it is not a layer, takes no [`Mark`], and changes
/// nothing that layers decide.
///
/// NOTE: The old dialogs sat at z-index 40 beside the old pills, except the tunnel consent dialog at 50, which was
/// always above them. Among the 40s, element order decided: the launcher, the quick switcher, the templates dialog, the
/// checkout trash dialog and the Hosts panel's dialogs came before the pills and sat under them, and Delete, Stop,
/// Rename and the app bar's dialogs (Settings and the help menu's) came after and covered them. Here the launch pill is
/// above every dialog but the raised tunnel warning (which covered it in the old app too), which matches the launcher
/// and the other dialogs that start a launch, a deviation chosen by the foundations owner on 2026-11-15 when the slot
/// was ported. Below every dialog would put the "Started" report under a launcher opened again while it shows, which
/// the old report covered; above, it only hides part of a later dialog from view, never from a click, as it takes no
/// pointer input; and below some dialogs only would need a per-layer choice, where the old app's split fell out of
/// element order. The old launcher drew no veil and took no input while its own progress pill showed
/// (`.launch-request-pending`). One overlap is kept as the old app had it: the launch pill at the window's bottom and
/// the sequence pill on the main area's bottom edge (while the launcher is open) can overlap, and the sequence pill
/// then covers it.
pub fn show_launch_pill(view: AnyView, window: &mut Window, cx: &mut App) -> Option<PillId> {
    let stack = stack(window, cx)?;
    let id = stack.update(cx, |stack, cx| {
        let id = PillId(stack.next_pill_id);
        stack.next_pill_id += 1;
        stack.launch_pill = Some(ShownLaunchPill { id, view });
        cx.notify();
        id
    });
    Some(id)
}

/// Take down the pill `id` in `window`, in whichever slot it shows. Only the exact id in its slot can match.
pub fn hide_pill(id: PillId, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    stack.update(cx, |stack, cx| {
        if stack.pill.as_ref().is_some_and(|pill| pill.id == id) {
            stack.pill = None;
            cx.notify();
        }
        if stack.launch_pill.as_ref().is_some_and(|pill| pill.id == id) {
            stack.launch_pill = None;
            cx.notify();
        }
        if stack
            .terminal_refusal
            .as_ref()
            .is_some_and(|pill| pill.id == id)
        {
            stack.terminal_refusal = None;
            cx.notify();
        }
    });
}

/// Whether the pill `id` still shows in `window`: not hidden, replaced, taken down by a reset, or gone
/// with its anchor.
pub fn pill_shown(id: PillId, window: &Window, cx: &App) -> bool {
    stack(window, cx).is_some_and(|stack| {
        let stack = stack.read(cx);
        stack.pill.as_ref().is_some_and(|pill| pill.id == id)
            || stack.launch_pill.as_ref().is_some_and(|pill| pill.id == id)
            || stack
                .terminal_refusal
                .as_ref()
                .is_some_and(|pill| pill.id == id)
    })
}

// ---- The approval region ---------------------------------------------------------------------------------------------

/// The approval region a window has: its feature's view, and the focus handle of the wrapper around it, which tells
/// whether focus is on one of its controls ([`focus_is_where_dialogs_allow`]).
struct Region {
    view: AnyView,
    container: FocusHandle,
}

/// Install `view` as `window`'s approval region: SPEC.md's independent agent approval cards, which stay usable while a
/// dialog is open (every dialog but one raised above it, [`above_approval_region`]). The approvals feature installs it
/// once per window, at window start, and never shows or hides it: the view draws its cards from the store, and nothing
/// while no card waits. Installing again replaces the view and keeps the region's focus handle. A reset
/// ([`close_all`]) leaves it installed. SPEC_impl.md, "Overlays: the layer manager", has its rules and why they are so,
/// the decision about Escape, and what is not built yet.
///
/// The view is laid out as the frame is prepared, inside a zero-size box at the window's origin, so it places its card
/// itself with absolute insets and an explicit width, from an element the session view draws ([`Anchor::drawn_bounds`],
/// this frame's there) and `window.viewport_size()`. Percent sizes resolve against the zero-size box.
///
/// NOTE: Whatever the view draws must occlude (`occlude()`), and while it has nothing to show it draws nothing with a
/// size or a listener. A card that does not occlude passes a press on to what it covers as well as taking it, and
/// something drawn while empty covers the page. The layer manager cannot see inside the view; review holds it. The
/// slot is named for the one thing it is for: a second feature that wants to draw over the dialogs needs a decision of
/// the layer manager's owner, not this slot.
pub fn install_approval_region(view: AnyView, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    stack.update(cx, |stack, cx| {
        // Not a tab stop, as a layer's container: Tab moves between the card's controls, never onto the wrapper. A
        // second install keeps the handle, so focus already on a card stays inside the region as the frame on screen
        // draws it.
        let container = stack
            .region
            .take()
            .map(|region| region.container)
            .unwrap_or_else(|| cx.focus_handle().tab_stop(false));
        stack.region = Some(Region { view, container });
        cx.notify();
    });
}

/// Draw the open layer `id`'s own box, and every layer opened over it, above the approval region until it closes (rung
/// 7 of the module docs' z-order ladder); its veil stays under the cards (rung 4). Its opener calls this right after
/// the open, before a frame shows it. Only SPEC.md's tunnel warning does, so a card that asks for the warning cannot
/// cover it: the old app drew that dialog at z-index 50, over the approval cards' 45 (`.tunnel-consent-backdrop` and
/// `.approval-cards` in `app.css` at reference commit `24bc0580`, lines 9790 and 8842, and the z-index ladder at
/// `.header-confirm`, line 2261), whatever opened it (a Cmd-click in the terminal, Settings' switch or a card's Tunnel
/// and open). Every other old dialog, menu and question sat at 40 or lower, under the cards (the hosts dialog a card
/// opens included), so no other opener raises its layer.
///
/// The old warning's backdrop was at 50 too, and blocked every card while it was up. Here the veil stays under the
/// cards, so a press on a card outside the warning's box is the card's and focus may rest there, as SPEC.md's "Opening
/// and closing" has it ("independent agent approval cards remain usable" while a modal dialog is open). Decided by the
/// foundations owner on 2026-11-15: SPEC.md's "remain usable" wins over the old app's blocking backdrop. The box
/// occludes, so a press on it where it overlaps a card is the warning's.
///
/// A layer opened over a raised one is drawn above the region with it, veil and all, where the old app would have
/// drawn a later dialog at z-index 40 under the warning, so that the drawn order stays the stack's and "the topmost
/// layer" means the same layer for drawing, presses, Tab's trap and Escape. While one is up, its veil covers the cards
/// too, the one case where they do not remain usable over a dialog: under them, its veil would sit under the raised
/// box, which would take presses meant for the dialog above. Nothing happens if `id` is no longer open.
pub fn above_approval_region(id: LayerId, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    stack.update(cx, |stack, cx| {
        if let Some(layer) = stack.layers.iter_mut().find(|layer| layer.id == id) {
            layer.above_region = true;
            cx.notify();
        }
    });
}

/// Say that the open layer `id` was opened from an approval card, so that it belongs to no layer and no other layer's
/// close takes it ([`close`], rule 2 of the parent rules). Every opener that a card's control calls makes this call
/// in the same update as a successful open, before anything can close the layer under it: Tunnel and open's warning
/// and the agent's Add host, together with every layer that add opens later in Add host's place (the setup
/// confirmation, and Add host reopened after a refused setup), which `setup::confirm` and `add::open_with` mark from
/// the add's `from_agent`. Those open after Add host has returned focus to its opener, which on macOS is the covered
/// dialog's control, so the focus rules alone would make them that dialog's too. A debug build fails on an `id` that
/// is no longer open: the call came too late, and in a release build it does nothing.
///
/// Focus cannot say where an open came from: on macOS a press on a card's button moves no focus
/// (`focus::CLICK_FOCUSES_CONTROLS`), so a card pressed while a dialog is open leaves focus in that dialog, which would
/// otherwise become the card layer's parent and take it along when its owner closes it. It is a call of its own rather
/// than part of [`above_approval_region`]: the tunnel warning is raised however it opens (from Settings, where it does
/// belong to Settings, as well as from a card), and the agent's Add host is not raised at all.
pub fn opened_from_approval_card(id: LayerId, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    stack.update(cx, |stack, _| {
        let layer = stack.layers.iter_mut().find(|layer| layer.id == id);
        debug_assert!(
            layer.is_some(),
            "opened_from_approval_card for a layer no longer open: call it in the same update as the open"
        );
        if let Some(layer) = layer {
            layer.parent = None;
        }
    });
}

/// Whether an open layer in `window` was raised above the approval region by its opener ([`above_approval_region`]),
/// wherever it sits in the stack. Only the raised layer itself counts, not the layers opened over it, which are drawn
/// above the region with it but were not raised by their own openers. Today that is only SPEC.md's tunnel warning, which
/// an approval card's Tunnel and open reads this as (`approvals::tunnel`): raising a second kind of layer would hold
/// that press back too.
pub fn above_approval_region_open(window: &Window, cx: &App) -> bool {
    stack(window, cx)
        .is_some_and(|stack| stack.read(cx).layers.iter().any(|layer| layer.above_region))
}

/// Whether the topmost open layer is raised above the approval region ([`above_approval_region`]): for the tests of
/// the tunnel warning's openers, which have no other way to see that their call was made.
#[cfg(test)]
pub(crate) fn top_is_above_approval_region(window: &Window, cx: &App) -> bool {
    stack(window, cx).is_some_and(|stack| {
        stack
            .read(cx)
            .layers
            .last()
            .is_some_and(|layer| layer.above_region)
    })
}

impl LayerStack {
    /// The index of the lowest layer drawn above the approval region ([`above_approval_region`]), or the number of
    /// layers when none is: the layers below it are drawn under the region, the rest above it.
    fn raised_from(&self) -> usize {
        self.layers
            .iter()
            .position(|layer| layer.above_region)
            .unwrap_or(self.layers.len())
    }

    /// Whether focus is on one of the approval region's controls (an agent approval card's).
    fn focus_in_region(&self, window: &Window, cx: &App) -> bool {
        self.region
            .as_ref()
            .is_some_and(|region| region.container.contains_focused(window, cx))
    }
}

// ---- Drawing ---------------------------------------------------------------------------------------------------------

/// Everything above the window's content, which the root view draws as its last child: rungs 2 to 9 of the module
/// docs' z-order ladder, from the layers under the lowest raised one ([`above_approval_region`]) up to the tooltip.
/// A staged recording can add paint above the ladder without creating a layer or taking focus or pointer input.
#[allow(
    clippy::disallowed_methods,
    reason = "the layer manager is the one owner of deferred (and of anchored, should it need it again)"
)]
pub fn overlay(window: &Window, cx: &App) -> Option<AnyElement> {
    let stack = stack(window, cx)?;
    let stack = stack.read(cx);
    #[cfg(feature = "agent")]
    let recording = crate::demo_video::overlay(window, cx);
    #[cfg(not(feature = "agent"))]
    let recording = None::<AnyElement>;
    if stack.layers.is_empty()
        && stack.launch_pill.is_none()
        && stack.region.is_none()
        && stack.pill.is_none()
        && stack.terminal_refusal.is_none()
        && stack.tooltip.is_none()
        && recording.is_none()
    {
        return None;
    }
    // The launch pill, raised veil, approval region and terminal refusal go between ordinary layers and raised
    // layers. Keep their priorities consecutive so the same rung always wins if they overlap.
    let (raised, above) = (stack.raised_from(), stack.layers.len());
    let (launch_depth, veil_depth, region_depth) = (raised + 1, raised + 2, raised + 3);
    // An open question must be readable even if it and the refusal overlap. Determine priority here, so opening
    // or closing a focus-taking layer takes effect in the same frame without a terminal redraw.
    let refusal_depth = if stack.layers.iter().any(|layer| layer.takes_focus) {
        0
    } else {
        raised + 4
    };
    let (pill_depth, tooltip_depth) = (above + 5, above + 6);
    let palette = theme::palette(cx);
    let mut overlay = div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .children(recording.map(|content| gpui::deferred(content).with_priority(usize::MAX)));
    for (index, layer) in stack.layers.iter().enumerate() {
        let depth = if index < raised { index + 1 } else { index + 5 };
        let id = layer.id;
        let main_action = layer.main_action.clone();
        let shortcut = match &layer.kind {
            Kind::Dialog(options) => options.shortcut.as_ref(),
            Kind::Popover { .. } => None,
        };
        let container = layer.container.clone();
        let content = div()
            .id(ElementId::Name(format!("layer-{}", id.0).into()))
            .debug_selector(move || format!("layer-{}", id.0));
        let passes = layer.passes_pointer();
        let content = content
            // A press that no control inside the layer took (empty space, a label) keeps focus on the control in use,
            // or moves it to the layer itself, as the layer was opened to do (SPEC.md, "Opening and closing"). A layer
            // that lets the pointer through is not tracked: it never takes focus, and the tracker's press handler calls
            // `window.prevent_default()`. Drawn over the page, the wrapper's handler runs before every handler under
            // it, so that would suppress GPUI's focus-on-press for an element under it tracked with plain
            // `track_focus` (Kit's text input) and the root's recovery of focus after a press no control takes: a
            // press through the popover would leave focus where the same press without it would not.
            .when(!passes, |content| {
                focus::track(content, &layer.container, layer.empty_press)
            })
            .when_some(shortcut, |content, shortcut| {
                let run = shortcut.run.clone();
                content
                    .key_context(shortcut.context)
                    .on_boxed_action(shortcut.action.as_ref(), move |_, window, cx| {
                        run(window, cx)
                    })
            })
            // Enter on the dialog itself presses its main action; a held or modified Enter does not.
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key == "enter"
                    && !event.is_held
                    && !event.keystroke.modifiers.modified()
                    && container.is_focused(window)
                    && let Some(action) = &main_action
                {
                    cx.stop_propagation();
                    action(window, cx);
                }
            })
            .child(layer.view.clone());
        let element = match &layer.kind {
            Kind::Dialog(options) => {
                let dismissible = options.dismiss_on_outside_press;
                // The lowest raised dialog's veil stays under the approval region while its box goes above it
                // ([`above_approval_region`]), so it is drawn on its own.
                let split = index == raised;
                // An outside press is a press the veil itself takes, not one outside the dialog's bounds: a press on an
                // occluding surface drawn above the veil (an approval card, a menu) never reaches the veil. The content
                // occludes so a press on it does not reach the veil either, nor a card under a split dialog's box.
                let content = content.when(dismissible || split, |content| content.occlude());
                let veil = div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .occlude()
                    .bg(theme::color(palette.scrim_dialog));
                let veil = veil.when(dismissible, |veil| {
                    // Only while this dialog is the top layer the user answers ([`top_layer`]): a press beside a menu
                    // opened over it is outside that menu, which closes, and the press is the menu's alone. Read before
                    // any handler acts (capture runs bottom layer up, before every bubble handler), since the menu's
                    // own close runs in capture.
                    let was_top = Rc::new(Cell::new(false));
                    let noted = was_top.clone();
                    veil.capture_any_mouse_down(move |_, window, cx| {
                        noted.set(top_layer(window, cx) == Some(id));
                    })
                    .on_any_mouse_down(move |_, window, cx| {
                        if was_top.take() {
                            dismiss_top(window, cx);
                        }
                    })
                });
                // What places the dialog: its veil, or for a split one a window-sized box with no hit area, so a press
                // beside the dialog goes on to the card or the veil under it.
                let frame = if split {
                    overlay = overlay.child(gpui::deferred(veil).with_priority(veil_depth));
                    div().absolute().top_0().left_0().size_full()
                } else {
                    veil
                };
                // The veil covers the whole root view, title bar included, so the bar stays inert; the dialog is
                // placed below the bar, in the content area (`desktop::linux_title_bar::content_area`). The root view
                // already sits inside Kit's frame, so the bar's height is the only padding it needs.
                let bar = crate::desktop::linux_title_bar::bar_height(window);
                let frame = frame.flex().justify_center();
                match options.placement {
                    DialogPlacement::Centered => frame.items_center().pt(bar).child(content),
                    DialogPlacement::Top => {
                        let height = crate::desktop::linux_title_bar::content_area(window)
                            .size
                            .height;
                        // A column, so a view that can shrink (`flex_1` and `min_h_0`, a list that scrolls) fits the
                        // cap. A view that cannot spills past it: what it draws below the cap is outside the layer
                        // for presses and focus (an outside press there dismisses), so a top-placed view must shrink.
                        // It is not clipped at the cap: a clip there would also cut off the card's own drop shadow.
                        frame
                            .items_start()
                            .pt(bar + (height * 0.12).min(px(80.)))
                            .child(content.max_h(height * 0.75).flex().flex_col())
                    }
                }
                .into_any_element()
            }
            Kind::Popover {
                anchor,
                surface,
                placement,
                matches_width,
                ..
            } => {
                // A popover that lets the pointer through gets neither part: its wrapper registers no hit area, so
                // every press, hover and wheel turn reaches what it covers, except on a control inside that occludes
                // itself, and a press outside it is not the user answering it ([`Popover::passes_pointer`]).
                let content = content.when(!passes, |content| {
                    let pressed = anchor.clone();
                    // A popover that lets Tab through closes on an outside press as when focus leaves it, returning
                    // none: the press puts focus where it lands ([`CloseCause::Left`]).
                    let why = if layer.takes_focus && !layer.traps_tab {
                        CloseCause::Left
                    } else {
                        CloseCause::Closed
                    };
                    // The card takes the pointer from whatever it covers, as the old app's cards did (a web page hands
                    // a press to the topmost element only). A popover has no veil, and GPUI hands a press, a click and
                    // a wheel to every element under the pointer up to the first that blocks them, so without this a
                    // click on a menu item or a question's answer also reached the page below: it clicked a control
                    // there, and a terminal there took focus before the answer ran, so the question closed with focus
                    // outside it. The card's own scrolling content still takes the wheel; nothing behind it does.
                    content
                        .occlude()
                        .on_mouse_down_out(move |event, window, cx| {
                            // A press on the anchor itself is left to the anchor, which toggles its popover: closing
                            // here first would let the same click reopen it.
                            let on_anchor = pressed
                                .get()
                                .is_some_and(|bounds| bounds.contains(&event.position));
                            if !on_anchor && !on_descendant_popover(id, event.position, window, cx)
                            {
                                close_as(id, why, window, cx);
                            }
                        })
                });
                PlacedPopover {
                    anchor: anchor.clone(),
                    surface: surface.clone(),
                    placement: placement.clone(),
                    matches_width: *matches_width,
                    child: content.into_any_element(),
                }
                .into_any_element()
            }
        };
        overlay = overlay.child(gpui::deferred(element).with_priority(depth));
    }
    if let Some(launch) = &stack.launch_pill {
        // Rung 3 of the ladder: above every layer but a raised one. Display only, as the pill below is.
        let placed = PlacedLate {
            place: LateBox::WindowBottom,
            child: launch.view.clone().into_any_element(),
        };
        overlay = overlay.child(gpui::deferred(placed).with_priority(launch_depth));
    }
    if let Some(region) = &stack.region {
        let placed = PlacedLate {
            place: LateBox::Window,
            child: region_wrapper(region).into_any_element(),
        };
        overlay = overlay.child(gpui::deferred(placed).with_priority(region_depth));
    }
    // These wrappers take no input, so a press on either pill reaches what it covers.
    if let Some(refusal) = &stack.terminal_refusal {
        let placed = PlacedLate {
            place: LateBox::AtAnchor(refusal.anchor.clone()),
            child: refusal.view.clone().into_any_element(),
        };
        overlay = overlay.child(gpui::deferred(placed).with_priority(refusal_depth));
    }
    if let Some(pill) = &stack.pill {
        let placed = PlacedLate {
            place: LateBox::Pill {
                anchor: pill.anchor.clone(),
                placement: pill.placement,
            },
            child: pill.view.clone().into_any_element(),
        };
        overlay = overlay.child(gpui::deferred(placed).with_priority(pill_depth));
    }
    if let Some(shown) = &stack.tooltip {
        let card = tooltip_card(shown, window, cx);
        let focus = stack
            .by_focus
            .as_ref()
            .map(|by_focus| by_focus.focus.clone());
        // Prepaint reads the bounds after the popovers have placed themselves for this frame.
        let panels = || {
            stack.layers.iter().filter_map(|layer| match &layer.kind {
                Kind::Popover {
                    anchor,
                    surface,
                    passes_pointer: false,
                    ..
                } => Some((anchor, surface)),
                _ => None,
            })
        };
        let vertical_panels = || {
            if shown.placement == HelpPlacement::Below {
                panels()
                    .map(|(anchor, surface)| (anchor.clone(), surface.clone()))
                    .collect()
            } else {
                Vec::new()
            }
        };
        let placed = match (shown.placement, focus) {
            (HelpPlacement::Ordinary | HelpPlacement::Below, Some(focus)) => {
                beside::PlacedBeside::around(shown, focus, card, vertical_panels())
            }
            (HelpPlacement::Ordinary | HelpPlacement::Below, None) => {
                beside::PlacedBeside::ordinary(shown, card, vertical_panels())
            }
            (placement, focus) => {
                let edges = beside::HelpEdges {
                    inline: stack.help_edges.clone(),
                    panels: panels().map(|(_, surface)| surface.clone()).collect(),
                };
                beside::PlacedBeside::new(shown, placement, edges, focus, card, palette)
            }
        };
        overlay = overlay.child(gpui::deferred(placed).with_priority(tooltip_depth));
    }
    Some(overlay.into_any_element())
}

/// `--shadow-menu`: `0 6px 18px` in the token's color, shared by menus, popovers and tooltips.
pub(crate) fn menu_shadow(palette: &theme::Palette) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: theme::color(palette.shadow_menu),
        offset: point(px(0.), px(6.)),
        blur_radius: px(18.),
        spread_radius: px(0.),
        inset: false,
    }]
}

/// Draws a popover where its placement says, deciding the position while the frame is laid out rather than when it is
/// built. An anchor element records its bounds as the frame is prepared, and overlays are prepared after the rest of
/// the window, so a popover lands under its anchor in the very frame the anchor moved: no frame shows it at the old
/// place, and nothing has to ask for a second frame.
///
/// A popover as wide as its anchor ([`Anchor::matching_width`]) is laid out at that point too, not with the rest of the
/// window: the window's layout is computed before any element records where it is drawn, so the anchor's width known
/// then is the previous frame's, and a popover sized from it would lag a resize by a frame (and stay wrong if no
/// other frame followed).
struct PlacedPopover {
    anchor: AnchorBounds,
    surface: AnchorBounds,
    placement: Placement,
    matches_width: bool,
    child: AnyElement,
}

/// The gap between an anchor and the popover below it, as the old menus had.
const POPOVER_GAP: Pixels = px(4.);

/// How far a popover placed [`Placement::Beside`] its anchor starts past the anchor's right edge: the old session
/// flyout's `left: rect.max_x() + 10`.
const BESIDE_GAP: Pixels = px(10.);

/// How far above its anchor's top a popover placed [`Placement::Beside`] it starts: the old flyout's
/// `top: rect.min_y() - 4`.
const BESIDE_RISE: Pixels = px(4.);

/// The top-left corner, in window coordinates, of a popover `size` big placed by `placement` against `anchor` (this
/// frame's bounds, `None` while the anchor has never been drawn) and kept inside `area`, the window's content area
/// ([`crate::desktop::linux_title_bar::content_area`]).
///
/// Every placement answers inside `area`: flush against its right and bottom edges when it would run past them, and
/// against its left and top edges as a last resort, which a popover wider or taller than the area also gets. Below
/// flips above its anchor when it would run past the area's bottom and fits above it within the area's top. The area,
/// not the window, because on Linux the window holds GPUI Kit's frame and the window's own title bar, and a popover
/// clamped to the window could end in the frame, outside the page. SPEC.md refuses a click on a terminal link whose
/// destination display is not wholly inside the page, and a display ending in Kit's frame is not; the click check
/// (`terminal::disclosure`'s `visibility`) measures against the same area. The layer manager's owner moved popovers
/// into the area on 2026-11-16. Under a compositor's decorations and on macOS the area is the whole window.
///
/// Crate-visible, and a pure function of its inputs, for `layers_tests.rs`: no test window reports client decorations,
/// so the tests pass a Linux content area apart from the window themselves.
pub(crate) fn popover_origin(
    anchor: Option<Bounds<Pixels>>,
    placement: &Placement,
    size: Size<Pixels>,
    area: Bounds<Pixels>,
) -> Point<Pixels> {
    let mut origin = match (anchor, placement) {
        (Some(anchor), Placement::Beside) => {
            point(anchor.right() + BESIDE_GAP, anchor.top() - BESIDE_RISE)
        }
        (Some(anchor), Placement::Submenu) => {
            let right = anchor.right() + BESIDE_GAP;
            let left = anchor.left() - BESIDE_GAP;
            let right_room = area.right() - right;
            let left_room = left - area.left();
            let x = if right_room < size.width && left_room > right_room {
                left - size.width
            } else {
                right
            };
            point(x, anchor.top() - BESIDE_RISE)
        }
        (Some(anchor), Placement::Below) => {
            let below = anchor.bottom() + POPOVER_GAP;
            let above = anchor.top() - POPOVER_GAP - size.height;
            // Below unless it would run off the area's bottom and it fits above, under the area's top.
            let y = if below + size.height > area.bottom() && above >= area.top() {
                above
            } else {
                below
            };
            point(anchor.left(), y)
        }
        // The rule keeps inside the area itself ([`PlaceBy`]); the clamp below only catches one that does not.
        (Some(anchor), Placement::By(place)) => place(anchor, size, area),
        (None, _) => area.origin,
    };
    // Shift left when it overflows the right edge, up when it overflows the bottom, and back to the left and top edges
    // as a last resort: `max` comes last, so a popover larger than the area starts at its top-left corner.
    origin.x = origin.x.min(area.right() - size.width).max(area.left());
    origin.y = origin.y.min(area.bottom() - size.height).max(area.top());
    origin
}

impl IntoElement for PlacedPopover {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for PlacedPopover {
    /// The child's layout when it is laid out with the window; `None` when it waits for prepaint (anchor width).
    type RequestLayoutState = Option<LayoutId>;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Option<LayoutId>) {
        let style = Style {
            position: gpui::Position::Absolute,
            ..Style::default()
        };
        if self.matches_width {
            return (window.request_layout(style, [], cx), None);
        }
        let child = self.child.request_layout(window, cx);
        (window.request_layout(style, [child], cx), Some(child))
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut Option<LayoutId>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let size = match *child {
            Some(child) => window.layout_bounds(child).size,
            None => {
                // The anchor's tracker has recorded this frame's bounds by now. The content goes in a wrapper of
                // exactly that width, which it fills as any block does; its height is its own.
                let width = self.anchor.get().map_or(px(0.), |anchor| anchor.size.width);
                let content = std::mem::replace(&mut self.child, gpui::Empty.into_any_element());
                self.child = div().w(width).child(content).into_any_element();
                let space = gpui::size(AvailableSpace::Definite(width), AvailableSpace::MinContent);
                self.child.layout_as_root(space, window, cx)
            }
        };
        let origin = popover_origin(
            self.anchor.get(),
            &self.placement,
            size,
            crate::desktop::linux_title_bar::content_area(window),
        );
        if child.is_some() {
            // Laid out at this element's own place: move it by the difference.
            let offset = origin - bounds.origin;
            let offset = point(offset.x.round(), offset.y.round());
            self.surface.set(Some(Bounds {
                origin: bounds.origin + offset,
                size,
            }));
            window.with_element_offset(offset, |window| self.child.prepaint(window, cx));
        } else {
            // Laid out as a root of its own, at no place: put it at its window position.
            let origin = point(origin.x.round(), origin.y.round());
            self.surface.set(Some(Bounds { origin, size }));
            self.child.prepaint_at(origin, window, cx);
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Option<LayoutId>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

/// Draws a pill or the approval region, deciding the place while the frame is laid out, for the reason
/// [`PlacedPopover`] does: anchors record this frame's bounds before overlays are prepared, so the pill follows a moved
/// or resized anchor in the same frame, and the approval region's view, which renders as it is laid out here, reads
/// this frame's bounds ([`Anchor::drawn_bounds`]). Nothing is drawn while the pill's anchor has never been drawn. In
/// the one frame that no longer draws the anchor, the pill is drawn once more at the anchor's last place, as a popover
/// is, before the anchor's release takes it down. The launch pill, which has no anchor, takes the window's size of
/// this frame.
struct PlacedLate {
    place: LateBox,
    child: AnyElement,
}

/// The box a [`PlacedLate`] lays its child out in.
enum LateBox {
    /// The refusal takes the exact bounds of its ordinary line, which are already snapped to physical pixels.
    AtAnchor(AnchorBounds),
    /// Over the pill's anchor, as its [`PillPlacement`] says.
    Pill {
        anchor: AnchorBounds,
        placement: PillPlacement,
    },
    /// The whole window from its origin: the approval region, whose view places its card itself.
    Window,
    /// The whole window, with the child centered across it on its bottom inset: the launch pill
    /// ([`show_launch_pill`]).
    WindowBottom,
}

impl IntoElement for PlacedLate {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for PlacedLate {
    type RequestLayoutState = ();
    /// Whether the child was laid out and prepared, and so may be painted.
    type PrepaintState = bool;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let style = Style {
            position: gpui::Position::Absolute,
            ..Style::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let (origin, size) = match &self.place {
            LateBox::AtAnchor(anchor) => {
                let Some(anchor) = anchor.get() else {
                    return false;
                };
                (anchor.origin, anchor.size)
            }
            LateBox::Window => (point(px(0.), px(0.)), window.viewport_size()),
            LateBox::WindowBottom => {
                // A window-sized column that sets the view on its bottom inset, centered across: the old pill's
                // `bottom: 20px; left: 50%; translateX(-50%)`. The wrapper's cap is the old `max-width: calc(100vw -
                // 40px)`, the column's width less its side insets. Neither box asks for the pointer.
                let viewport = window.viewport_size();
                let content = std::mem::replace(&mut self.child, gpui::Empty.into_any_element());
                self.child = div()
                    .w(viewport.width)
                    .h(viewport.height)
                    .flex()
                    .flex_col()
                    .justify_end()
                    .items_center()
                    .px(LAUNCH_PILL_INSET)
                    .pb(LAUNCH_PILL_INSET)
                    .child(div().max_w_full().child(content))
                    .into_any_element();
                (point(px(0.), px(0.)), viewport)
            }
            LateBox::Pill { anchor, placement } => {
                let Some(anchor) = anchor.get() else {
                    return false;
                };
                // A box exactly over the anchor, which centers the view across and centers it or sets it on the bottom
                // edge down. Flex centering lets a view wider or taller than the anchor overflow it evenly, as the old
                // pill's `translate(-50%, …)` did. The box takes no pointer input, like the view.
                let content = std::mem::replace(&mut self.child, gpui::Empty.into_any_element());
                let frame = div()
                    .w(anchor.size.width)
                    .h(anchor.size.height)
                    .flex()
                    .justify_center();
                let frame = match placement {
                    PillPlacement::Centered => frame.items_center(),
                    PillPlacement::Bottom => frame.items_end(),
                };
                self.child = frame.child(content).into_any_element();
                (
                    point(anchor.origin.x.round(), anchor.origin.y.round()),
                    anchor.size,
                )
            }
        };
        let space = gpui::size(
            AvailableSpace::Definite(size.width),
            AvailableSpace::Definite(size.height),
        );
        self.child.layout_as_root(space, window, cx);
        self.child.prepaint_at(origin, window, cx);
        true
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        placed: &mut bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        if *placed {
            self.child.paint(window, cx);
        }
    }
}

/// The element around the approval region's view: a zero-size box (see [`install_approval_region`] for why) that
/// tracks the region's focus handle, so containment says when focus is on one of its controls, and that takes Escape
/// there, so it never reaches the root's dismissal of the topmost layer.
fn region_wrapper(region: &Region) -> gpui::Div {
    focus::track(div().size_0(), &region.container, focus::OnPress::Keep)
        .on_action(|_: &actions::Dismiss, _, _| {})
        .child(region.view.clone())
}

/// The popover panel the old menus drew: the floating surface with its 4-pixel padding at top and bottom, and the menu
/// key context ([`actions::MENU_CONTEXT`]), in which Up and Down walk its items. Menu views wrap their content in it.
/// The menu role goes on the menu component that the session row's menu brings, which has the element id a role needs.
pub fn panel(cx: &App) -> gpui::Div {
    surface(cx)
        .key_context(actions::MENU_CONTEXT)
        .flex()
        .flex_col()
        .py(px(4.))
}

/// The floating surface every anchored layer is drawn on: the old `--bg-2` fill, its outline and radius, and the menu
/// shadow (`.session-row-menu-panel` in the old stylesheet, which both a row's menu and the question it turned into
/// used). No layout and no key context: a menu adds those through [`panel`], an anchored question its own.
pub fn surface(cx: &App) -> gpui::Div {
    let palette = theme::palette(cx);
    div()
        .bg(theme::color(palette.bg_2))
        .border_1()
        .border_color(theme::color(palette.border))
        .rounded(metrics::RADIUS_MD)
        .shadow(menu_shadow(palette))
}
