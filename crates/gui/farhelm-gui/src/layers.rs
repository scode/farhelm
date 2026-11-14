//! The layer manager: the one owner of everything drawn above the window's ordinary content.
//!
//! SPEC_impl.md, "Overlays: the layer manager": one owner draws the dialog slot, the popover stack and tooltips, and
//! GPUI's `deferred`/`anchored` are banned everywhere else in the GUI crates (`crates/gui/clippy.toml`). This module
//! holds that one exemption. Its rules:
//!
//! - **Who opened what.** Opening a layer returns its [`LayerId`], and only that id closes it ([`close`]): nothing
//!   closes a dialog, menu or confirmation it did not open. The user closes the topmost layer with Escape
//!   ([`dismiss_top`]) or, for a popover, by clicking outside it. A dialog's own hook decides each dismissal
//!   ([`Dialog::on_dismiss`]): it may keep the dialog open, as an editor with a save in flight does, or keep it open
//!   and ask first, as an editor with an unsaved draft will. The one exception is a reset of the connection to the
//!   helm, which closes everything ([`close_all`]) without asking.
//! - **Things the user did not start never open a layer.** Every open takes the [`Mark`] the feature took when the user
//!   acted, and is refused once the user has moved on since (the focus coordinator's counter). Work that finishes later
//!   shows its result where it belongs instead.
//! - **An overlay closes when its subject is gone, not when it moves.** A popover is anchored to an element through an
//!   [`Anchor`], state the anchor element keeps in its element state: it follows the element wherever it is drawn,
//!   and closes when the element is no longer drawn (a deleted row's menu). A layer may also name a [`Subject`]: an
//!   entity whose release closes it, or a session the helm's listing proves gone (`Subject::session`, a
//!   confirmation about a session that was deleted elsewhere). Every overlay is anchored to an element: SPEC.md has no
//!   menu opened at a pointer position.
//! - **Three kinds of layer.** A dialog is centered over a veil (or near the window's top, for an opener that asks:
//!   [`Dialog::placement`]). A popover (a menu) is drawn below its anchor, or
//!   beside it when the anchor asks for that ([`Anchor::beside`], the session row's menu flying out beside the
//!   sidebar). An anchored dialog ([`open_anchored`]) is drawn like a popover but answers like a dialog: it has a
//!   subject, a main action for Enter and its own empty-space rule. It is how the prompt asks a question beside the
//!   control it is about (a row's Delete question, opened from the row's menu, as the old app turned the menu into the
//!   question). An anchored layer is as wide as its content, or exactly as wide as its anchor when the anchor asks for
//!   that ([`Anchor::matching_width`]): a select's list under its control, a floating list under its field.
//! - **A dialog is modal.** It draws a veil over the window that takes every click. A click on its empty space (label
//!   text included: labels are no longer selectable, so the GPUI migration settled on 2026-11-13 that a click on one
//!   counts as empty space) keeps focus on the control in use in the dialogs SPEC.md lists, and moves focus to the
//!   dialog itself in the others ([`Dialog::keeps_focus_on_empty_click`]); a click on a menu's empty space keeps focus
//!   on its item. Tab stays inside the topmost layer, dialog or menu alike (the focus coordinator walks GPUI's tab
//!   order within [`trap`]). Closing any layer returns focus to what held it when the layer opened, unless a dialog's
//!   opener named home or a control (the button that opened it) instead ([`Dialog::returns_to`]). A press reaches a
//!   dialog's empty-space rule only after the controls under it have handled it, and one that gave focus to a control
//!   (a click into a text field) is not a press on empty space (the focus coordinator's [`focus::OnPress::Focus`]).
//! - **A popover's card takes the pointer from what it covers.** It has no veil, so the page around it stays live, but
//!   a press, click, wheel, hover or move on the card itself never reaches the element under it: a click on a
//!   question's answer over a terminal must not also focus the terminal or click a control there.
//! - **A popover may leave focus alone.** One opened without a start ([`Popover::start`]) never takes focus: opening
//!   it moves none, Tab is not trapped in it, closing it returns none, and a press on it leaves focus where it was. It
//!   is for a list that floats beside the control the user keeps typing in or pointing at, as the old launcher's
//!   folder picker and composer search kept focus in their field (options at `tabindex: -1`, the list's mousedown
//!   prevented). Escape, a press outside it and its anchor going still close it, as any popover, but closing it closes
//!   only it: it owns nothing above it, so a dialog opened while it shows (a shortcut pressed in the field or the
//!   terminal beside it) stays open, with focus inside, when it goes ([`close`]). Every rule above that names "the
//!   topmost layer" for focus (Tab's trap, where focus recovers, the menu highlight) means the topmost layer that holds
//!   focus, which such a popover never is.
//! - **One tooltip at a time**, owned by the control that showed it (see [`TooltipOwner`]). It says the app's own
//!   words, or a host sentence when the help names a host value, drawn through host text's element ([`ShownTooltip`]).
//! - **The innermost help wins** when a control with hover help sits inside another with its own (a session row's
//!   marks inside the row); [`hover_began`], "Nested hover help", explains the rule and how it is decided.
//! - **One pill, drawn above every layer and under the tooltip, that is not a layer** ([`show_pill`]). It is the one
//!   thing a feature draws over the dialogs without opening one: SPEC.md's quick-launch sequence pill, which stays in
//!   view above the launcher's veil, as the old one sat at z-index 50 over the launcher's backdrop at 40. It is display
//!   only, like the tooltip: it asks for no pointer input and holds no focus, so a press on it is a press on whatever it
//!   covers, and nothing that asks about layers ([`any_open`], Tab's trap, where focus recovers, Escape's
//!   [`dismiss_top`]) sees it. It sits centered over an [`Anchor`] element or at its bottom edge ([`PillPlacement`]) and
//!   goes when that element is no longer drawn, when its feature hides it ([`hide_pill`]), or on a reset
//!   ([`close_all`]).
//!
//! State is kept per window: multi-window support is a watch-list item, and the one thing it asks of the foundations
//! now is that overlays are not shared between windows. Each window's [`LayerStack`] is an entity that notifies, and
//! only that window's root view observes it ([`attach`]), so opening a layer or showing a tooltip redraws the root,
//! which draws the overlay, while cached views below it replay. A whole-window refresh would instead re-render every
//! cached view, which on a 200-row session list is every row each time the pointer crosses a control.
//!
//! A tooltip waits [`HOVER_DELAY`] (the old app's 300 ms) before it shows, through a ticker deadline, and shows at once
//! when another tooltip is showing or one went away less than that long ago, so a user scanning a row of icons does
//! not wait at each. Escape, a press of any mouse button on a control with help ([`pointer_pressed`]) or a layer
//! opening puts the tooltip away without that grace, as in the old app. After a press made without moving, the help
//! stays down while the pointer rests on that control, since nothing changes whose help the pointer calls for.
//!
//! NOTE: Known deviations from the old app's `tooltip.js`, which kept a dismissed control's help down until the pointer
//! left it: after Escape, the next pointer move brings the help back after the delay; a key other than Escape takes
//! help down (GPUI ends every hover on a key press, and `tooltip.js` had no key handler but Escape's); after a press, a
//! key press and then a pointer move bring the help back; a press on a control right after a key press, or a click
//! whose pointer moves while the button is held, brings the help back after the delay once the button is released (GPUI
//! ends the control's hover during either and begins it again after); and after a press on a row, a visit to one of its
//! marks and back shows the row's help again. Still to come here: the covered-terminal-link hit test, which needs the
//! terminal; and the tooltip's pointer notch and the target ring on its control (both follow element anchoring for
//! tooltips). Until then a tooltip sits below and right of where the pointer was, and a pointer moving inside a control
//! does not restart the wait. Menu items are plain buttons until the session row's menu brings the menu item component,
//! so the popover golden shows centered item labels, which the old menus did not have; it is captured again with that
//! component.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, AnyView, AnyWindowHandle, App, AppContext as _, AvailableSpace, Bounds, BoxShadow,
    Context, Element, ElementId, Entity, EntityId, FocusHandle, Global, GlobalElementId,
    InspectorElementId, InteractiveElement, IntoElement, LayoutId, ParentElement, Pixels, Point,
    SharedString, Style, Styled, Subscription, WeakEntity, WeakFocusHandle, Window, WindowId,
    canvas, div, point, prelude::FluentBuilder as _, px,
};

use crate::actions;
use crate::bootstrap::{installed, installed_mut};
use crate::focus::{self, Mark};
use crate::host_text::Line;
use crate::theme::{self, metrics};
use crate::ticker;

// ---- Per-window state ------------------------------------------------------------------------------------------------

/// Identifies one open layer. Returned by the open functions; the only way to close that layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerId(u64);

/// One window's overlays: its open layers, bottom to top, its pill and its tooltip.
pub struct LayerStack {
    window: AnyWindowHandle,
    layers: Vec<Layer>,
    next_id: u64,
    /// The pill showing in this window, if any ([`show_pill`]). Kept apart from `layers` so that nothing that walks the
    /// layers for focus, Escape or "is anything open" can count it.
    pill: Option<ShownPill>,
    /// The id the next shown pill gets.
    next_pill_id: u64,
    tooltip: Option<ShownTooltip>,
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
    /// The first of `hovered` when the hover news last settled. A pointer moving inside the same control changes
    /// nothing, so it neither shows a showing tooltip again nor (not yet: see the module NOTE) starts a waiting one's
    /// delay over, and a help a press put away stays away while the hover lasts ([`pointer_pressed`]; the module NOTE
    /// says when GPUI ends it). A tooltip shown or waiting through a hover is always the winner's; only the goldens'
    /// [`show_tooltip_at`] shows one that is not.
    winner: Option<EntityId>,
    /// The hover news of the event being handled, waiting for [`settle_hover`], which runs once after it: `Some` while
    /// a settle is scheduled, holding the owners whose move listener ran, in the order GPUI ran them.
    round: Option<Vec<EntityId>>,
}

/// One control the pointer is over, as [`LayerStack::hovered`] keeps it.
struct Hovered {
    owner: WeakEntity<TooltipOwner>,
    id: EntityId,
    /// The help it asked for when its hover began, refreshed each time the control is drawn ([`help_drawn`]). An outer
    /// control's help may show long after its hover began (the pointer returning from a mark onto its row), and by then
    /// the control may say something else: a session row's help follows the session's status, and the frame that takes
    /// a stopped session's live dot away also changes its row's help.
    help: Line,
    /// Drops the entry, and settles again, when the control is gone: no hover-leave fires for a control that simply
    /// stops being drawn, and the control around it, still under the pointer, should then have its help back.
    _release: Subscription,
}

/// How long the pointer must rest on a control before its hover help shows: the old app's tooltip delay.
pub const HOVER_DELAY: std::time::Duration = std::time::Duration::from_millis(300);

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
    /// Whether the layer took focus when it opened and holds it while open: every layer but a popover opened without
    /// a start ([`Popover::start`]), which Tab's trap, focus recovery and the focus return on close all pass over.
    takes_focus: bool,
    /// Where closing it returns focus: `opener`, unless its opener named home or a control ([`Dialog::returns_to`]).
    returns_to: focus::ReturnTo,
    /// What a dismissal of it does ([`Dialog::on_dismiss`]); `None` always closes.
    on_dismiss: Option<DismissHook>,
    /// What Enter does while the dialog itself (not one of its controls) has focus.
    main_action: Option<MainAction>,
    /// What a press on the layer's empty space does to focus.
    empty_press: focus::OnPress,
    /// Close the layer when its subject is gone or its anchor is no longer drawn.
    _watches: Vec<Subscription>,
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
        placement: Placement,
        /// Whether it is exactly as wide as its anchor ([`Anchor::matching_width`]).
        matches_width: bool,
    },
}

/// Where a popover sits against its anchor. Each is the old app's placement for the overlays that used it, and each is
/// kept inside the window by the same clamp.
#[derive(Clone, Copy)]
pub enum Placement {
    /// Below the anchor, left edges aligned, or above it when it would run off the bottom and there is more room above:
    /// the old menus and popup lists.
    Below,
    /// Beside the anchor, to its right: the left edge 10 pixels past the anchor's right edge and the top 4 pixels above
    /// the anchor's. The old session row menu's flyout (`session_menu_placement_style` in `menu_panel.rs` at reference
    /// commit `24bc0580`), whose toggle ends just inside the sidebar's edge, so the menu stands 8 pixels clear of the
    /// list. Like the old one it never changes to another side when the window is narrow: the clamp only shifts it back
    /// inside.
    ///
    /// NOTE: The clamp is the one every popover shares, which keeps the popover flush inside the window. The old
    /// flyout's own clamp differed, a deviation recorded when this was ported: it kept 8 pixels from the edges, it held
    /// a fixed 528 pixels of room below the menu's top rather than the menu's own height (so a short menu on a low row
    /// moved up further than it does here), and it capped the menu's height and scrolled it, where this one would run
    /// past the bottom of a window shorter than the menu (the full row menu fits in the minimum window).
    Beside,
}

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
        next_pill_id: 0,
        tooltip: None,
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
    /// anchor element must be positioned (`relative()`) so the tracker covers it.
    pub fn track(&self) -> impl IntoElement {
        let bounds = self.bounds.clone();
        // Pinned to the element's corner: an absolute child without insets would sit wherever it falls in the flow,
        // after the element's other children, and record the wrong place.
        canvas(move |at, _, _| bounds.set(Some(at)), |_, _, _, _| {})
            .absolute()
            .top_0()
            .left_0()
            .size_full()
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
    /// still takes the press, so the page behind never sees it.
    pub dismiss_on_outside_press: bool,
}

/// A popover to open: a menu or a popup list.
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
    pub start: Option<FocusHandle>,
    /// The element it is drawn below (or beside, [`Anchor::beside`]), following it as it moves; it closes when the
    /// element is no longer drawn.
    pub anchor: Anchor,
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
#[derive(Clone, Copy)]
struct DialogOptions {
    placement: DialogPlacement,
    dismiss_on_outside_press: bool,
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
        }),
        view,
        container,
        Some(start),
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
    /// The kind of an anchored layer opened on this anchor: placed and sized as the anchor asks.
    fn kind(&self) -> Kind {
        Kind::Popover {
            anchor: self.bounds.clone(),
            placement: self.placement,
            matches_width: self.matches_width,
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
        anchor.kind(),
        view,
        container,
        Some(start),
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
    } = popover;
    let drawn = drawn(&anchor)?;
    open(
        anchor.kind(),
        view,
        container,
        start,
        vec![drawn],
        None,
        // A menu's empty space (a separator, padding) keeps focus on its item.
        focus::OnPress::Keep,
        focus::ReturnTo::Opener,
        None,
        mark,
        window,
        cx,
    )
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
    let id = stack.update(cx, |stack, cx| {
        let id = LayerId(stack.next_id);
        stack.next_id += 1;
        let window = stack.window;
        let watches = subjects
            .into_iter()
            .map(|subject| (subject.0)(cx, window, id))
            .collect();
        stack.layers.push(Layer {
            id,
            kind,
            view,
            container,
            opener,
            takes_focus: start.is_some(),
            returns_to,
            on_dismiss,
            main_action,
            empty_press,
            _watches: watches,
        });
        // A layer covers the tooltip of whatever is under it, and the next tooltip waits the full delay again.
        stack.put_tooltip_away();
        cx.notify();
        id
    });
    // A popover that never takes focus leaves it with the control the user is in.
    if let Some(start) = &start {
        focus::enter_layer(start, window, cx);
    }
    Some(id)
}

/// Close `id` in `window` from inside the stack's own update (a subject going, an anchor's release), once that update
/// has returned: closing updates the stack, which cannot be updated from inside itself.
fn close_later(window: AnyWindowHandle, id: LayerId, cx: &mut App) {
    cx.defer(move |cx| {
        // The window may have closed meanwhile; then there is nothing left to close.
        let _ = window.update(cx, |_, window, cx| close(id, window, cx));
    });
}

/// Close the layer `id`, if it is still open, and anything opened on top of it, unless `id` is a popover that never
/// takes focus ([`Popover::start`] `None`): that one closes alone. If focus was inside what closed, it returns to what
/// held it when `id` opened, or where `id`'s opener said instead ([`Dialog::returns_to`]). A popover that never took
/// focus is passed over: closing one moves no focus.
///
/// A layer that takes focus owns what was opened over it: focus stays inside the topmost such layer ([`trap`]), so
/// whatever opened over it was opened from inside it (a menu from one of a dialog's controls), and goes with it. A
/// popover that never takes focus owns nothing above it. Focus was never in it, so a dialog over it was opened from the
/// page or the layer beneath (a shortcut pressed while a floating list or a link's disclosure showed), and the
/// popover's going (its owner closing it, a press outside it, its anchor no longer drawn) must leave that dialog open
/// with focus inside it. A layer opened by a click on the popover's own content and anchored there still goes with it,
/// since its anchor is no longer drawn; a centered dialog opened that way stays.
pub fn close(id: LayerId, window: &mut Window, cx: &mut App) {
    close_as(id, false, window, cx);
}

/// [`close`], saying whether the user dismissed the layer (`dismissed`), which decides where a layer that returns
/// focus home on Escape only ([`focus::ReturnTo::HomeOnEscape`]) sends it.
fn close_as(id: LayerId, dismissed: bool, window: &mut Window, cx: &mut App) {
    close_taking(dismissed, window, cx, |layers| {
        let index = layers.iter().position(|layer| layer.id == id)?;
        Some(if layers[index].takes_focus {
            layers.drain(index..).collect()
        } else {
            vec![layers.remove(index)]
        })
    });
}

/// Take the layers `take` picks out of `window`'s stack, then return focus as the lowest of them that took it says.
/// The one place layers leave the stack, so [`close`] and [`close_all`] share the focus return. `take` gets the stack's
/// layers, bottom to top, and answers `None` when there is nothing to close.
fn close_taking(
    dismissed: bool,
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
    let focus_was_inside = closed
        .iter()
        .any(|layer| layer.container.contains_focused(window, cx));
    // Focus goes back as the lowest closed layer that took it says: a popover that never took focus has none to give
    // back, and the control the user stayed in keeps it.
    if (focus_was_inside || window.focused(cx).is_none())
        && let Some(bottom) = closed.iter().find(|layer| layer.takes_focus)
    {
        let returns_to = match &bottom.returns_to {
            focus::ReturnTo::HomeOnEscape if dismissed => focus::ReturnTo::Home,
            focus::ReturnTo::HomeOnEscape => focus::ReturnTo::Opener,
            other => other.clone(),
        };
        focus::return_to(returns_to, bottom.opener.as_ref(), window, cx);
    }
}

/// Close every dialog and popover in every window, as a reset of the connection to the helm requires (SPEC.md,
/// "Losing the connection to the helm": open dialogs and menus start over). The UI-session owner calls it when the
/// window goes inert.
///
/// This is the one close that does not need the layer's id: a reset is not any opener's decision, and a dialog that
/// survived it would keep its veil over the inert window, where it takes the clicks meant for Take back or Retry and
/// traps Tab away from them. Focus follows the ordinary close rule, returning to what held it when the bottom layer
/// that took focus opened (and going home once that is no longer drawn). The pill goes too ([`show_pill`]): what it
/// showed belonged to the UI session the reset retired, and its feature, which lives in that session, is no longer
/// there to hide it. The tooltip is left alone: it goes with its control.
pub fn close_all(cx: &mut App) {
    let Some(windows) = installed::<Windows>(cx) else {
        return;
    };
    let stacks: Vec<Entity<LayerStack>> =
        windows.0.values().filter_map(WeakEntity::upgrade).collect();
    for stack in stacks {
        stack.update(cx, |stack, cx| {
            if stack.pill.take().is_some() {
                cx.notify();
            }
        });
        let window = stack.read(cx).window;
        // Every layer, not a close of the bottom one: when that is a popover that never takes focus, it closes alone.
        // A window that closed meanwhile has nothing left to close.
        let _ = window.update(cx, |_, window, cx| {
            close_taking(false, window, cx, |layers| {
                (!layers.is_empty()).then(|| std::mem::take(layers))
            })
        });
    }
}

/// The user closes the topmost layer (Escape), or the tooltip if no layer is open. Returns whether Escape was taken, so
/// the key can go on to whatever else wants it when nothing is open: a topmost dialog whose hook keeps it open
/// ([`Dialog::on_dismiss`]) takes it too.
pub fn dismiss_top(window: &mut Window, cx: &mut App) -> bool {
    let Some(stack) = stack(window, cx) else {
        return false;
    };
    let top = stack.read(cx).layers.last().map(|layer| layer.id);
    if let Some(top) = top {
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
        close_as(id, true, window, cx);
    }
}

/// The topmost layer in `window`, if any is open.
fn top_layer(window: &Window, cx: &App) -> Option<LayerId> {
    stack(window, cx)?
        .read(cx)
        .layers
        .last()
        .map(|layer| layer.id)
}

/// Whether the layer `id` is still open.
pub fn is_open(id: LayerId, window: &Window, cx: &App) -> bool {
    stack(window, cx).is_some_and(|stack| stack.read(cx).layers.iter().any(|layer| layer.id == id))
}

/// Whether any dialog or popover is open in `window`. The pill ([`show_pill`]) is not a layer and never counts.
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
pub fn focus_layer_open(window: &Window, cx: &App) -> bool {
    top_container(window, cx).is_some()
}

/// The container Tab must stay inside: the topmost layer's, dialog or menu alike (SPEC.md: "Inside a dialog or menu,
/// focus stays inside"). A menu opened over a dialog holds Tab until it closes.
///
/// NOTE: A popover that never takes focus ([`Popover::start`] `None`) traps nothing: Tab goes on from the control that
/// holds focus as if the popover were not open, inside whatever layer below it holds focus (the launcher dialog around
/// a floating list), or across the page. A list that keeps focus in its field (the launcher's folder picker, a
/// suggestion list) is that popover, not a menu with a flag.
pub fn trap(window: &Window, cx: &App) -> Option<FocusHandle> {
    top_container(window, cx)
}

/// Whether focus is inside the topmost dialog or a layer above it (a menu opened from inside the dialog): where SPEC.md
/// lets focus be while a dialog is open. `true` when no dialog is open.
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
    stack.layers[top_dialog..]
        .iter()
        .any(|layer| layer.container.contains_focused(window, cx))
}

/// The container of the topmost layer that holds focus, where focus recovers to when its holder disappears inside an
/// open layer. A popover that never takes focus is passed over, so focus never recovers into one.
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

// ---- Tooltips --------------------------------------------------------------------------------------------------------

/// The token that says which control a tooltip belongs to. It carries no data: its entity id is the owner's identity,
/// and its release is the signal that the control is gone.
pub struct TooltipOwner;

impl TooltipOwner {
    /// The owner token of the control being rendered under `key`, created on its first frame and kept for as long as
    /// the control is drawn in consecutive frames (GPUI's keyed element state, which cached views carry over when they
    /// replay). The token is unique per control instance in the window: it is keyed by the control's full element path,
    /// not its sibling-unique id. `key` must differ from the control's own element id.
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
    /// What the tooltip says, as one string, for tests and the bridge's snapshot: the app's wording, or for help that
    /// names a host value the sentence's accessible text, each host value escaped ([`Line::accessible_text`]). The
    /// card never draws it; it draws `help`.
    pub text: SharedString,
    /// Where the pointer was when the hover began, in window coordinates.
    pub at: Point<Pixels>,
    /// The help the card draws. Host values reach a tooltip only through this form, a [`Line::Sentence`], whose host
    /// parts are drawn by host text's own element, escaped and each in a text element of its own (SPEC_impl.md,
    /// "Host-supplied text"); a [`Line::App`] is Farhelm's own wording.
    help: Line,
}

/// The pointer came onto the control `owner`, whose hover help is `help`. This is the hover listener's half of the
/// tooltip rule, which [`crate::components::with_hover_help`] wires on every control, beside [`hover_moved`] and
/// [`hover_ended`]. Which hovered control's help shows is decided once, after GPUI has finished telling every
/// listener about the event ([`settle_hover`]). When it is this control's, it shows after [`HOVER_DELAY`] at the
/// pointer's position, or at once while another tooltip is showing or has only just gone.
///
/// A host value reaches a tooltip only as part of a [`Line::Sentence`]. The app's own wording is a [`Line::App`], which
/// a caller makes from a plain string with `.into()`.
///
/// # Nested hover help
///
/// A control with help can sit inside another that has its own (a session row's marks inside the row), so the pointer
/// can be over several at once. The innermost one's help shows, as in the old app, whose tooltip took the help nearest
/// the element under the pointer. Moving from it back onto the control around it shows that control's help again, and
/// moving from the outer control onto the inner shows the inner one's. GPUI draws a child after its parent, so on a
/// pointer move "innermost" means drawn last: of two hovered controls that overlap without nesting, the one drawn over
/// the other wins. Under a still pointer the newest hover wins until the pointer next moves, which for a control and
/// the ones inside it is the same answer, since they appear together or the inner one later.
///
/// The hover listeners alone cannot say which control is innermost, because the order they arrive in depends on why
/// the hover changed. On a pointer move GPUI calls them from the last drawn to the first, so a pointer that lands on a
/// mark in one move tells the mark first and the row last. When a frame moves controls under a still pointer (a
/// scroll, a row appearing) GPUI calls them in drawing order, row first. And a pointer going from the mark back onto
/// the row tells only the mark, because the row was hovered all along. So the layer manager keeps every hovered control
/// ([`LayerStack::hovered`]), newest hover first, which is right for a frame's news and for a pointer that crossed
/// into a control from the one around it. On a pointer move it takes GPUI's order instead: each hovered control's move
/// listener ([`hover_moved`]) runs on every move, last drawn first, so the order they ran in is exact.
pub fn hover_began(owner: &Entity<TooltipOwner>, help: Line, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let id = owner.entity_id();
    stack.update(cx, |stack, cx| {
        let release = cx.observe_release(owner, move |stack, _, cx| {
            stack.hovered.retain(|hovered| hovered.id != id);
            stack.settle_after_this_event(cx);
        });
        stack.hovered.retain(|hovered| hovered.id != id);
        stack.hovered.insert(
            0,
            Hovered {
                owner: owner.downgrade(),
                id,
                help,
                _release: release,
            },
        );
        stack.settle_after_this_event(cx);
    });
}

/// The pointer moved while over the control `owner`: the move listener's half of the tooltip rule. It says nothing
/// about the help itself. GPUI runs these on every pointer move, for every hovered control, from the last drawn to the
/// first, and that order is how [`settle_hover`] tells the innermost control from the ones around it (see "Nested
/// hover help" at [`hover_began`]).
pub fn hover_moved(owner: &Entity<TooltipOwner>, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let id = owner.entity_id();
    stack.update(cx, |stack, cx| {
        stack.settle_after_this_event(cx);
        if let Some(moved) = &mut stack.round
            && !moved.contains(&id)
        {
            moved.push(id);
        }
    });
}

/// The control `owner` is being drawn with `help` as its hover help: refresh the copy kept while the pointer is over
/// it, so a tooltip of it that begins later says what the control says then (see [`Hovered::help`]). It redraws
/// nothing: a tooltip already waiting or showing keeps the words it began with. Called while the control renders, so a
/// cached view's controls refresh exactly when they re-render, which is when their help can change.
pub fn help_drawn(owner: &Entity<TooltipOwner>, help: &Line, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let id = owner.entity_id();
    stack.update(cx, |stack, _| {
        if let Some(hovered) = stack.hovered.iter_mut().find(|hovered| hovered.id == id)
            && hovered.help != *help
        {
            hovered.help = help.clone();
        }
    });
}

/// The pointer left the control `owner`. Its tooltip goes only if no other hovered control's help takes its place, so
/// a control leaving never hides another's tooltip: that matters when the pointer moves straight from one control
/// onto the next and the new control's hover arrives first, and when it moves from a mark back onto the row around
/// it, whose help then shows.
pub fn hover_ended(owner: &Entity<TooltipOwner>, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let id = owner.entity_id();
    stack.update(cx, |stack, cx| {
        stack.hovered.retain(|hovered| hovered.id != id);
        stack.settle_after_this_event(cx);
    });
}

/// A mouse button went down on a control with help: put the tooltip away, as the old `tooltip.js` did on
/// `pointerdown` whatever the button, and make the control under the press (the innermost hovered one) the winner, so
/// that its help stays down while the pointer rests there and its hover lasts (the module NOTE lists when GPUI ends
/// it): [`settle_hover`] acts only when the winner changes. Every hovered control's press listener calls this for the
/// same press; the later calls change nothing.
pub fn pointer_pressed(window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    stack.update(cx, |stack, cx| {
        let shown = stack.tooltip.is_some();
        // The press may come in the same update as the pointer move that brought the pointer here, before that move's
        // settle (the test bridge sends a click's move, press and release together): order the hovers by that move.
        if let Some(moved) = &stack.round {
            innermost_first(&mut stack.hovered, moved);
        }
        stack.winner = stack.hovered.first().map(|hovered| hovered.id);
        stack.put_tooltip_away();
        if shown {
            cx.notify();
        }
    });
}

/// Put `hovered` in the order a pointer move's move listeners ran in (`moved`), which is innermost first and beats the
/// order the hovers began in. A control whose move listener did not run keeps its place after them (the sort is
/// stable), and an empty `moved` (no pointer move: a frame's news) changes nothing.
fn innermost_first(hovered: &mut [Hovered], moved: &[EntityId]) {
    if moved.is_empty() {
        return;
    }
    hovered.sort_by_key(|hovered| {
        moved
            .iter()
            .position(|id| *id == hovered.id)
            .unwrap_or(usize::MAX)
    });
}

/// Decide whose help the pointer calls for now that GPUI has told every listener about the event, and show it, or
/// take the tooltip down when the pointer is over no control with help. Nothing changes while the answer stays the
/// same control ([`LayerStack::winner`]).
fn settle_hover(window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let change = stack.update(cx, |stack, _| {
        let moved = stack.round.take().unwrap_or_default();
        stack
            .hovered
            .retain(|hovered| hovered.owner.upgrade().is_some());
        innermost_first(&mut stack.hovered, &moved);
        // This runs on every pointer move over a control with help, so the unchanged case clones nothing.
        let winner = stack.hovered.first().map(|hovered| hovered.id);
        if winner == stack.winner {
            return None;
        }
        let previous = std::mem::replace(&mut stack.winner, winner);
        let next = stack
            .hovered
            .first()
            .and_then(|hovered| Some((hovered.owner.upgrade()?, hovered.help.clone())));
        Some((previous, next))
    });
    match change {
        None => {}
        Some((_, Some((owner, help)))) => begin_tooltip(&owner, help, window, cx),
        Some((previous, None)) => end_tooltip(&stack, previous, cx),
    }
}

/// Start `owner`'s tooltip, now that its help is the one the pointer calls for: after [`HOVER_DELAY`], or at once
/// while another tooltip is showing or has only just gone. Either way it replaces any other tooltip, shown or waiting.
fn begin_tooltip(owner: &Entity<TooltipOwner>, help: Line, window: &mut Window, cx: &mut App) {
    let at = window.mouse_position();
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let at_once = {
        let stack = stack.read(cx);
        stack.tooltip.is_some() || stack.warm
    };
    if at_once {
        show_tooltip_from(owner, help, at, window, cx);
        return;
    }
    let owner_id = owner.entity_id();
    let weak_owner = owner.downgrade();
    let handle = window.window_handle();
    let deadline = ticker::after(HOVER_DELAY, cx, move |cx| {
        let Some(owner) = weak_owner.upgrade() else {
            return;
        };
        let _ = handle.update(cx, |_, window, cx| {
            show_tooltip_from(&owner, help, at, window, cx)
        });
    });
    stack.update(cx, |stack, _| {
        stack.pending_tooltip = Some((owner_id, deadline));
    });
}

/// Take down `previous`'s tooltip, shown or waiting: the pointer has left it and is over no other control whose help
/// shows. A tooltip some other control showed stays, so a control leaving never hides another's. A tooltip that was
/// showing leaves the grace behind it, so the next one within the delay shows at once.
fn end_tooltip(stack: &Entity<LayerStack>, previous: Option<EntityId>, cx: &mut App) {
    let Some(previous) = previous else {
        return;
    };
    let hidden = stack.update(cx, |stack, cx| {
        if stack
            .pending_tooltip
            .as_ref()
            .is_some_and(|(pending, _)| *pending == previous)
        {
            stack.pending_tooltip = None;
        }
        if !stack
            .tooltip
            .as_ref()
            .is_some_and(|shown| shown.owner == previous)
        {
            return false;
        }
        stack.tooltip = None;
        stack.tooltip_release = None;
        cx.notify();
        true
    });
    if hidden {
        let weak = stack.downgrade();
        let cool_down = ticker::after(HOVER_DELAY, cx, move |cx| {
            let _ = weak.update(cx, |stack, _| stack.warm = false);
        });
        stack.update(cx, |stack, _| {
            stack.warm = true;
            stack.cool_down = Some(cool_down);
        });
    }
}

/// [`hover_began`]'s tooltip shown at once, with the hover beginning at a fixed point, for the screenshot goldens.
#[cfg(test)]
pub(crate) fn show_tooltip_at(
    owner: &Entity<TooltipOwner>,
    help: Line,
    at: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    show_tooltip_from(owner, help, at, window, cx);
}

/// Show `help` for `owner` with the hover beginning at `at`. The one place a [`ShownTooltip`] is made, so its `text`
/// is always its `help`'s accessible text.
fn show_tooltip_from(
    owner: &Entity<TooltipOwner>,
    help: Line,
    at: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let owner_id = owner.entity_id();
    stack.update(cx, |stack, cx| {
        stack.pending_tooltip = None;
        stack.tooltip = Some(ShownTooltip {
            owner: owner_id,
            text: help.accessible_text(),
            at,
            help,
        });
        // The tooltip closes when its control is gone, though no hover-leave ever fires for a control that simply
        // stops being drawn (a dialog closing on Enter, a deleted row).
        stack.tooltip_release = Some(cx.observe_release(owner, move |stack, _, cx| {
            if stack
                .tooltip
                .as_ref()
                .is_some_and(|shown| shown.owner == owner_id)
            {
                stack.tooltip = None;
                cx.notify();
            }
        }));
        cx.notify();
    });
}

impl LayerStack {
    /// Run [`settle_hover`] once, after the event being handled: GPUI tells each hovered control's listeners about one
    /// pointer move (or one frame's hover changes) in turn, and the help to show depends on all of them. A settle runs
    /// as a deferred effect, which GPUI runs only once the event's listeners have all returned, and the first piece of
    /// news schedules it.
    fn settle_after_this_event(&mut self, cx: &mut App) {
        if self.round.is_some() {
            return;
        }
        self.round = Some(Vec::new());
        let window = self.window;
        // The update fails only when the window is gone, and its stack with it, so the round it leaves open is never
        // needed again.
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| settle_hover(window, cx));
        });
    }

    /// Take the tooltip down, and any waiting one, without the grace that lets the next one skip the delay: after
    /// Escape or a layer opening, the user has just put one away.
    fn put_tooltip_away(&mut self) {
        self.tooltip = None;
        self.tooltip_release = None;
        self.pending_tooltip = None;
        self.warm = false;
        self.cool_down = None;
    }
}

/// The tooltip showing now in `window`, for tests and the bridge's snapshot.
pub fn shown_tooltip(window: &Window, cx: &App) -> Option<ShownTooltip> {
    stack(window, cx)?.read(cx).tooltip.clone()
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
/// its keys come from the feature's own capture, not from the view.
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

/// Identifies one shown pill. Returned by [`show_pill`]; the only way to hide that pill, as a [`LayerId`] is the only
/// way to close its layer.
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

/// Show `pill` in `window`, above every layer and under the tooltip, replacing any pill showing there. Returns the id
/// that hides it, or `None` when its anchor element is already gone (there is nowhere to place it) or the window has
/// no layer stack.
///
/// One slot per window: showing replaces whatever shows, and [`hide_pill`] hides only the pill its id names, so a
/// feature that hides a pill another show has replaced does nothing. SPEC.md has one such pill and arms one sequence at
/// a time, so one slot is all it needs. A second display-only pill that can show at the same time (the launch progress
/// pill, say) needs a slot of its own here, or each show would replace the other's pill.
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
        let drawn = cx.observe_release(&state, |stack, _, cx| {
            stack.pill = None;
            cx.notify();
        });
        stack.pill = Some(ShownPill {
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

/// Take down the pill `id` in `window`, if it still shows. Nothing happens once another show has replaced it.
pub fn hide_pill(id: PillId, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    stack.update(cx, |stack, cx| {
        if stack.pill.as_ref().is_some_and(|pill| pill.id == id) {
            stack.pill = None;
            cx.notify();
        }
    });
}

/// Whether the pill `id` still shows in `window`: not hidden, replaced, taken down by a reset, or gone with its anchor.
pub fn pill_shown(id: PillId, window: &Window, cx: &App) -> bool {
    stack(window, cx).is_some_and(|stack| {
        stack
            .read(cx)
            .pill
            .as_ref()
            .is_some_and(|pill| pill.id == id)
    })
}

// ---- Drawing ---------------------------------------------------------------------------------------------------------

/// Everything above the window's content, which the root view draws as its last child: each open layer, bottom to top,
/// then the pill, then the tooltip.
#[allow(
    clippy::disallowed_methods,
    reason = "the layer manager is the one owner of deferred and anchored"
)]
pub fn overlay(window: &Window, cx: &App) -> Option<AnyElement> {
    let stack = stack(window, cx)?;
    let stack = stack.read(cx);
    if stack.layers.is_empty() && stack.pill.is_none() && stack.tooltip.is_none() {
        return None;
    }
    let palette = theme::palette(cx);
    let mut overlay = div().absolute().top_0().left_0().size_full();
    for (depth, layer) in stack.layers.iter().enumerate() {
        let id = layer.id;
        let main_action = layer.main_action.clone();
        let container = layer.container.clone();
        let content = div()
            .id(ElementId::Name(format!("layer-{}", id.0).into()))
            .debug_selector(move || format!("layer-{}", id.0));
        // A press that no control inside the layer took (empty space, a label) keeps focus on the control in use, or
        // moves it to the layer itself, as the layer was opened to do (SPEC.md, "Opening and closing").
        let content = focus::track(content, &layer.container, layer.empty_press)
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
                let content = content.when(options.dismiss_on_outside_press, |content| {
                    content.on_mouse_down_out(move |_, window, cx| {
                        // Only while this dialog is the top layer: a press inside a menu (or another dialog) opened over
                        // it is outside this dialog too, and belongs to that layer. GPUI runs these handlers from the
                        // bottom layer up, so this one sees such a layer still open and leaves the press to it.
                        if top_layer(window, cx) == Some(id) {
                            dismiss_top(window, cx);
                        }
                    })
                });
                let veil = div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .occlude()
                    .bg(theme::color(palette.scrim_dialog))
                    .flex()
                    .justify_center();
                match options.placement {
                    DialogPlacement::Centered => veil.items_center().child(content),
                    DialogPlacement::Top => {
                        let viewport = window.viewport_size();
                        // A column, so a view that can shrink (`flex_1` and `min_h_0`, a list that scrolls) fits the
                        // cap. A view that cannot spills past it: what it draws below the cap is outside the layer
                        // for presses and focus (an outside press there dismisses), so a top-placed view must shrink.
                        // It is not clipped at the cap: a clip there would also cut off the card's own drop shadow.
                        veil.items_start()
                            .pt((viewport.height * 0.12).min(px(80.)))
                            .child(content.max_h(viewport.height * 0.75).flex().flex_col())
                    }
                }
                .into_any_element()
            }
            Kind::Popover {
                anchor,
                placement,
                matches_width,
            } => {
                let pressed = anchor.clone();
                // The card takes the pointer from whatever it covers, as the old app's cards did (a web page hands a press
                // to the topmost element only). A popover has no veil, and GPUI hands a press, a click and a wheel to
                // every element under the pointer up to the first that blocks them, so without this a click on a menu
                // item or a question's answer also reached the page below: it clicked a control there, and a terminal
                // there took focus before the answer ran, so the question closed with focus outside it. The card's own
                // scrolling content still takes the wheel; nothing behind it does.
                let content = content.occlude();
                let content = content.on_mouse_down_out(move |event, window, cx| {
                    // A press on the anchor itself is left to the anchor, which toggles its popover: closing here first
                    // would let the same click reopen it.
                    let on_anchor = pressed
                        .get()
                        .is_some_and(|bounds| bounds.contains(&event.position));
                    if !on_anchor {
                        close(id, window, cx);
                    }
                });
                PlacedPopover {
                    anchor: anchor.clone(),
                    placement: *placement,
                    matches_width: *matches_width,
                    child: content.into_any_element(),
                }
                .into_any_element()
            }
        };
        overlay = overlay.child(gpui::deferred(element).with_priority(depth + 1));
    }
    if let Some(pill) = &stack.pill {
        // Above every layer, as the old pill sat above the launcher's backdrop, and under the tooltip, which the old
        // z-index ladder put above every panel. Nothing here asks for the pointer (no listener, no `occlude`), so GPUI
        // registers no hit area for the placing wrapper and a press on the pill reaches what it covers.
        let placed = PlacedPill {
            anchor: pill.anchor.clone(),
            placement: pill.placement,
            child: pill.view.clone().into_any_element(),
        };
        overlay = overlay.child(gpui::deferred(placed).with_priority(stack.layers.len() + 1));
    }
    if let Some(shown) = &stack.tooltip {
        // Offset like a native tooltip so the pointer never covers the text.
        let at = point(shown.at.x + px(8.), shown.at.y + px(16.));
        let placed = gpui::anchored()
            .position(at)
            .snap_to_window()
            .child(tooltip_card(shown, window, cx));
        overlay = overlay.child(gpui::deferred(placed).with_priority(stack.layers.len() + 2));
    }
    Some(overlay.into_any_element())
}

/// The tooltip's card, styled as the old `.farhelm-tooltip`.
fn tooltip_card(shown: &ShownTooltip, window: &Window, cx: &App) -> gpui::Div {
    let palette = theme::palette(cx);
    let (pad_y, pad_x) = metrics::TOOLTIP_PAD;
    let viewport = window.viewport_size();
    // `max-width: min(280px, 100vw - 8px)` and `max-height: 100vh - 8px`, as the old tooltip.
    let max_width = metrics::TOOLTIP_MAX_WIDTH.min(viewport.width - px(8.));
    div()
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
        .shadow(menu_shadow(palette))
        .text_size(metrics::TEXT_SM)
        .line_height(metrics::TOOLTIP_LINE_HEIGHT)
        .line_clamp(metrics::TOOLTIP_MAX_LINES)
        // The help itself, not its accessible text: a sentence draws each host value through host text's element.
        .child(shown.help.clone())
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
        let viewport = window.viewport_size();
        let mut origin = match (self.anchor.get(), self.placement) {
            (Some(anchor), Placement::Beside) => {
                point(anchor.right() + BESIDE_GAP, anchor.top() - BESIDE_RISE)
            }
            (Some(anchor), Placement::Below) => {
                let below = anchor.bottom() + POPOVER_GAP;
                let above = anchor.top() - POPOVER_GAP - size.height;
                // Below unless it would run off the bottom and there is more room above.
                let y = if below + size.height > viewport.height && above >= px(0.) {
                    above
                } else {
                    below
                };
                point(anchor.left(), y)
            }
            (None, _) => point(px(0.), px(0.)),
        };
        // Keep it inside the window: shift left when it overflows the right edge, down from the top as a last resort.
        origin.x = origin.x.min(viewport.width - size.width).max(px(0.));
        origin.y = origin.y.min(viewport.height - size.height).max(px(0.));
        if child.is_some() {
            // Laid out at this element's own place: move it by the difference.
            let offset = origin - bounds.origin;
            window.with_element_offset(point(offset.x.round(), offset.y.round()), |window| {
                self.child.prepaint(window, cx)
            });
        } else {
            // Laid out as a root of its own, at no place: put it at its window position.
            self.child
                .prepaint_at(point(origin.x.round(), origin.y.round()), window, cx);
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

/// Draws the pill over its anchor as its [`PillPlacement`] says, deciding the place while the frame is laid out, for
/// the reason [`PlacedPopover`] does: the anchor records this frame's bounds before overlays are prepared, so the pill
/// follows a moved or resized anchor in the same frame. Nothing is drawn while the anchor has never been drawn. In the
/// one frame that no longer draws the anchor, the pill is drawn once more at the anchor's last place, as a popover is,
/// before the anchor's release takes it down.
struct PlacedPill {
    anchor: AnchorBounds,
    placement: PillPlacement,
    child: AnyElement,
}

impl IntoElement for PlacedPill {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for PlacedPill {
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
        let Some(anchor) = self.anchor.get() else {
            return false;
        };
        // A box exactly over the anchor, which centers the view across and centers it or sets it on the bottom edge
        // down. Flex centering lets a view wider or taller than the anchor overflow it evenly, as the old pill's
        // `translate(-50%, …)` did. The box takes no pointer input, like the view.
        let content = std::mem::replace(&mut self.child, gpui::Empty.into_any_element());
        let frame = div()
            .w(anchor.size.width)
            .h(anchor.size.height)
            .flex()
            .justify_center();
        let frame = match self.placement {
            PillPlacement::Centered => frame.items_center(),
            PillPlacement::Bottom => frame.items_end(),
        };
        self.child = frame.child(content).into_any_element();
        let space = gpui::size(
            AvailableSpace::Definite(anchor.size.width),
            AvailableSpace::Definite(anchor.size.height),
        );
        self.child.layout_as_root(space, window, cx);
        self.child.prepaint_at(
            point(anchor.origin.x.round(), anchor.origin.y.round()),
            window,
            cx,
        );
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
