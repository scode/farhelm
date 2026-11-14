//! The layer manager: the one owner of everything drawn above the window's ordinary content.
//!
//! SPEC_impl.md, "Overlays: the layer manager": one owner draws the dialog slot, the popover stack and tooltips, and
//! GPUI's `deferred`/`anchored` are banned everywhere else in the GUI crates (`crates/gui/clippy.toml`). This module
//! holds that one exemption. Its rules:
//!
//! - **Who opened what.** Opening a layer returns its [`LayerId`], and only that id closes it ([`close`]): nothing
//!   closes a dialog, menu or confirmation it did not open. The user closes the topmost layer with Escape
//!   ([`dismiss_top`]) or, for a popover, by clicking outside it; Escape leaves a dialog open while it says it may not
//!   be dismissed now ([`Dialog::may_dismiss`], an editor with a save in flight). The one exception is a reset of the connection to
//!   the helm, which closes everything ([`close_all`]).
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
//!   [`DialogOptions`]). A popover (a menu) is drawn below its anchor, or
//!   beside it when the anchor asks for that ([`Anchor::beside`], the session row's menu flying out beside the
//!   sidebar). An anchored dialog ([`open_anchored`]) is drawn like a popover but answers like a dialog: it has a
//!   subject, a main action for Enter and its own empty-space rule. It is how the prompt asks a question beside the
//!   control it is about (a row's Delete question, opened from the row's menu, as the old app turned the menu into the
//!   question).
//! - **A dialog is modal.** It draws a veil over the window that takes every click. A click on its empty space (label
//!   text included: labels are no longer selectable, so the GPUI migration settled on 2026-11-13 that a click on one
//!   counts as empty space) keeps focus on the control in use in the dialogs SPEC.md lists, and moves focus to the
//!   dialog itself in the others ([`Dialog::keeps_focus_on_empty_click`]); a click on a menu's empty space keeps focus
//!   on its item. Tab stays inside the topmost layer, dialog or menu alike (the focus coordinator walks GPUI's tab
//!   order within [`trap`]). Closing any layer returns focus to what held it when the layer opened, unless a dialog's
//!   opener named home instead ([`Dialog::returns_to`]). A press reaches a dialog's empty-space rule only after the
//!   controls under it have handled it, and one that gave focus to a control (a click into a text field) is not a
//!   press on empty space (the focus coordinator's [`focus::OnPress::Focus`]).
//! - **One tooltip at a time**, owned by the control that showed it (see [`TooltipOwner`]).
//!
//! State is kept per window: multi-window support is a watch-list item, and the one thing it asks of the foundations
//! now is that overlays are not shared between windows. Each window's [`LayerStack`] is an entity that notifies, and
//! only that window's root view observes it ([`attach`]), so opening a layer or showing a tooltip redraws the root,
//! which draws the overlay, while cached views below it replay. A whole-window refresh would instead re-render every
//! cached view, which on a 200-row session list is every row each time the pointer crosses a control.
//!
//! A tooltip waits [`HOVER_DELAY`] (the old app's 300 ms) before it shows, through a ticker deadline, and shows at once
//! when another tooltip is showing or one went away less than that long ago, so a user scanning a row of icons does
//! not wait at each. Escape or a layer opening puts the tooltip away without that grace, as in the old app.
//!
//! NOTE: Still to come here: putting the tooltip away on any pointer press, as the old app's `tooltip.js` did (nothing
//! on a press path does yet); the covered-terminal-link hit test, which needs the terminal; and the tooltip's pointer
//! notch and the target ring on its control (both follow element anchoring for tooltips). Until then a tooltip sits
//! below and right of where the pointer was, and a pointer moving inside a control does not restart the wait. Menu
//! items are plain buttons until the session row's menu brings the menu item component, so the popover golden shows
//! centered item labels, which the old menus did not have; it is captured again with that component.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, AnyView, AnyWindowHandle, App, AppContext as _, Bounds, BoxShadow, Context,
    Element, ElementId, Entity, EntityId, FocusHandle, Global, GlobalElementId, InspectorElementId,
    InteractiveElement, IntoElement, LayoutId, ParentElement, Pixels, Point, SharedString, Style,
    Styled, Subscription, WeakEntity, WeakFocusHandle, Window, WindowId, canvas, div, point,
    prelude::FluentBuilder as _, px,
};

use crate::actions;
use crate::bootstrap::{installed, installed_mut};
use crate::focus::{self, Mark};
use crate::theme::{self, metrics};
use crate::ticker;

// ---- Per-window state ------------------------------------------------------------------------------------------------

/// Identifies one open layer. Returned by the open functions; the only way to close that layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerId(u64);

/// One window's overlays: its open layers, bottom to top, and its tooltip.
pub struct LayerStack {
    window: AnyWindowHandle,
    layers: Vec<Layer>,
    next_id: u64,
    tooltip: Option<ShownTooltip>,
    /// Takes the tooltip down when its owner is released; replaced with each new tooltip.
    tooltip_release: Option<Subscription>,
    /// The tooltip waiting out its hover delay: its owner, and the deadline that shows it. Dropping it cancels.
    pending_tooltip: Option<(EntityId, ticker::Deadline)>,
    /// A tooltip showed recently enough that the next one skips the hover delay. Cleared by `cool_down`.
    warm: bool,
    cool_down: Option<ticker::Deadline>,
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
    /// Where closing it returns focus: `opener`, unless its opener named home ([`Dialog::returns_to`]).
    returns_to: focus::ReturnTo,
    /// Whether the user may dismiss it right now ([`Dialog::may_dismiss`]); `None` always may.
    may_dismiss: Option<DismissGuard>,
    /// What Enter does while the dialog itself (not one of its controls) has focus.
    main_action: Option<MainAction>,
    /// What a press on the layer's empty space does to focus.
    empty_press: focus::OnPress,
    /// Close the layer when its subject is gone or its anchor is no longer drawn.
    _watches: Vec<Subscription>,
}

/// A dialog's main action, which Enter presses while the dialog itself has focus.
pub type MainAction = Rc<dyn Fn(&mut Window, &mut App)>;

/// Whether the user may dismiss a dialog right now ([`Dialog::may_dismiss`]). Asked at the moment of each dismissal.
pub type DismissGuard = Rc<dyn Fn(&App) -> bool>;

enum Kind {
    /// A dialog over its veil, sitting and answering an outside press as its opener asked.
    Dialog(DialogOptions),
    /// A popover, placed against its anchor element as its [`Placement`] says and following it as it moves; the bounds
    /// are where the anchor was last drawn.
    Popover(AnchorBounds, Placement),
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
        tooltip: None,
        tooltip_release: None,
        pending_tooltip: None,
        warm: false,
        cool_down: None,
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

/// An element a popover can be anchored to. Built while the element renders ([`Anchor::for_element`]); the element
/// draws [`Anchor::track`] as a child so the popover knows where it is.
///
/// It refers to the element's state weakly: keeping an `Anchor` around (in a view, a test) does not keep the element
/// "drawn", so a popover anchored to it still closes when the element goes.
#[derive(Clone)]
pub struct Anchor {
    state: WeakEntity<AnchorState>,
    bounds: AnchorBounds,
    /// Where a layer opened on this anchor sits against it: [`Placement::Below`] unless the feature asked otherwise.
    placement: Placement,
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
    /// it opened) for every dialog SPEC.md does not send elsewhere.
    pub returns_to: focus::ReturnTo,
    /// Whether the user may dismiss the dialog right now, asked on every dismissal the layer manager performs: Escape,
    /// wherever focus is inside the dialog, the dialog itself included, and a press outside a dialog opened with
    /// [`DialogOptions::dismiss_on_outside_press`]. While it says no, a dismissal does nothing at all (Escape closes no
    /// tooltip or lower layer either). For a dialog busy with work it must not lose, such as an editor
    /// whose save is in flight: a guard in the dialog's own view would miss the case where the dialog itself holds
    /// focus (after a click on its empty space), because the layer manager's element sits around that view.
    ///
    /// Only dismissals are guarded. A close the opener makes itself ([`close`], on a save that succeeded or on its own
    /// Cancel), a subject going away, and a reset of the connection ([`close_all`]) close the dialog whatever this says.
    /// `None` is always dismissible.
    pub may_dismiss: Option<DismissGuard>,
}

/// A popover to open: a menu or a popup list.
pub struct Popover {
    /// What the popover draws.
    pub view: AnyView,
    /// The popover's own focus handle, tracked on the element around `view`.
    pub container: FocusHandle,
    /// Where focus starts (a menu's first item, usually).
    pub start: FocusHandle,
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
}

/// Where a dialog sits over its veil, and whether a press outside it dismisses it: choices its opener makes with
/// [`open_dialog_with`]. The default is every dialog's ordinary form: centered, and an outside press does nothing but
/// what a press on the veil always does (nothing reaches the page).
#[derive(Clone, Copy, Default)]
pub struct DialogOptions {
    pub placement: DialogPlacement,
    /// Whether a press on the veil, outside the dialog, dismisses it: the old quick switcher closed that way. It goes
    /// through the same path as Escape ([`dismiss_top`]), so the dialog's [`Dialog::may_dismiss`] guard still decides
    /// and focus returns as Escape returns it; the veil still takes the press, so the page behind never sees it.
    pub dismiss_on_outside_press: bool,
}

/// Where a dialog sits over its veil.
#[derive(Clone, Copy, Default)]
pub enum DialogPlacement {
    /// Centered in the window, as every dialog of the old app but the quick switcher.
    #[default]
    Centered,
    /// Centered across, its top `min(12% of the window's height, 80 px)` from the window's top and its height at most
    /// 75% of the window's: the old quick switcher (`.quick-switcher-backdrop` and `.quick-switcher-dialog` in
    /// `app.css` at reference commit `24bc0580`), which a dialog that grows with its results keeps from moving. The cap
    /// holds a view that can shrink to it (`flex_1` and `min_h_0` on its root, with its own scrolling list), as the old
    /// dialog's list scrolled inside its `max-height`; a view that cannot shrink draws past the cap, and that part counts
    /// as outside the dialog for presses and focus. A view on [`crate::components::DialogFrame`] fits only if its root
    /// and the frame both shrink, since the frame's own cap is taller. The old backdrop's 16-pixel side padding is the
    /// view's to keep, by capping its own width.
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
    open_dialog_with(dialog, DialogOptions::default(), mark, window, cx)
}

/// [`open_dialog`], placed and dismissed as `options` say.
pub fn open_dialog_with(
    dialog: Dialog,
    options: DialogOptions,
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
        may_dismiss,
    } = dialog;
    open(
        Kind::Dialog(options),
        view,
        container,
        start,
        subject.into_iter().collect(),
        main_action,
        empty_press(keeps_focus_on_empty_click),
        returns_to,
        may_dismiss,
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
    } = dialog;
    let drawn = drawn(&anchor)?;
    open(
        Kind::Popover(anchor.bounds, anchor.placement),
        view,
        container,
        start,
        std::iter::once(drawn).chain(subject).collect(),
        main_action,
        empty_press(keeps_focus_on_empty_click),
        focus::ReturnTo::Opener,
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
        Kind::Popover(anchor.bounds, anchor.placement),
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
    start: FocusHandle,
    subjects: Vec<Subject>,
    main_action: Option<MainAction>,
    empty_press: focus::OnPress,
    returns_to: focus::ReturnTo,
    may_dismiss: Option<DismissGuard>,
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
            returns_to,
            may_dismiss,
            main_action,
            empty_press,
            _watches: watches,
        });
        // A layer covers the tooltip of whatever is under it, and the next tooltip waits the full delay again.
        stack.put_tooltip_away();
        cx.notify();
        id
    });
    focus::enter_layer(&start, window, cx);
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

/// Close the layer `id`, if it is still open, and anything opened on top of it. If focus was inside what closed, it
/// returns to what held it when `id` opened, or where `id`'s opener said instead ([`Dialog::returns_to`]).
pub fn close(id: LayerId, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let closed = stack.update(cx, |stack, cx| {
        let index = stack.layers.iter().position(|layer| layer.id == id)?;
        let closed: Vec<Layer> = stack.layers.drain(index..).collect();
        cx.notify();
        Some(closed)
    });
    let Some(closed) = closed else {
        return;
    };
    let focus_was_inside = closed
        .iter()
        .any(|layer| layer.container.contains_focused(window, cx));
    if (focus_was_inside || window.focused(cx).is_none())
        && let Some(bottom) = closed.first()
    {
        focus::return_to(bottom.returns_to, bottom.opener.as_ref(), window, cx);
    }
}

/// Close every dialog and popover in every window, as a reset of the connection to the helm requires (SPEC.md,
/// "Losing the connection to the helm": open dialogs and menus start over). The UI-session owner calls it when the
/// window goes inert.
///
/// This is the one close that does not need the layer's id: a reset is not any opener's decision, and a dialog that
/// survived it would keep its veil over the inert window, where it takes the clicks meant for Take back or Retry and
/// traps Tab away from them. Focus follows the ordinary close rule, returning to what held it when the bottom layer
/// opened (and going home once that is no longer drawn). The tooltip is left alone: it goes with its control.
pub fn close_all(cx: &mut App) {
    let Some(windows) = installed::<Windows>(cx) else {
        return;
    };
    let stacks: Vec<Entity<LayerStack>> =
        windows.0.values().filter_map(WeakEntity::upgrade).collect();
    for stack in stacks {
        let (window, bottom) = {
            let stack = stack.read(cx);
            (stack.window, stack.layers.first().map(|layer| layer.id))
        };
        if let Some(bottom) = bottom {
            // A window that closed meanwhile has nothing left to close.
            let _ = window.update(cx, |_, window, cx| close(bottom, window, cx));
        }
    }
}

/// The user closes the topmost layer (Escape), or the tooltip if no layer is open. Returns whether Escape was taken, so
/// the key can go on to whatever else wants it when nothing is open: a topmost layer that may not be dismissed now
/// ([`Dialog::may_dismiss`]) takes it and stays open.
pub fn dismiss_top(window: &mut Window, cx: &mut App) -> bool {
    let Some(stack) = stack(window, cx) else {
        return false;
    };
    let top = stack
        .read(cx)
        .layers
        .last()
        .map(|layer| (layer.id, layer.dismissible(cx)));
    if let Some((top, dismissible)) = top {
        if dismissible {
            close(top, window, cx);
        }
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

/// Whether any dialog or popover is open in `window`.
pub fn any_open(window: &Window, cx: &App) -> bool {
    stack(window, cx).is_some_and(|stack| !stack.read(cx).layers.is_empty())
}

/// The container Tab must stay inside: the topmost layer's, dialog or menu alike (SPEC.md: "Inside a dialog or menu,
/// focus stays inside"). A menu opened over a dialog holds Tab until it closes.
///
/// NOTE: Every popover traps Tab, which suits a menu. A pop-up suggestion list keeps focus in its field, outside the
/// popover (SPEC.md, the repository suggestions), so Tab must not be trapped in it: whoever builds one gives it its own
/// layer kind, or a no-trap flag, rather than relying on this.
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

/// The topmost layer's container, where focus recovers to when its holder disappears inside an open layer.
pub(crate) fn top_container(window: &Window, cx: &App) -> Option<FocusHandle> {
    let stack = stack(window, cx)?;
    stack
        .read(cx)
        .layers
        .last()
        .map(|layer| layer.container.clone())
}

/// What breaks the overlay rules in `window` right now, if anything: a popover anchored to an element that was never
/// drawn has nowhere to be. The test harness runs this after every step.
pub fn invariant_violation(window: &Window, cx: &App) -> Option<String> {
    let stack = stack(window, cx)?;
    stack
        .read(cx)
        .layers
        .iter()
        .find_map(|layer| match &layer.kind {
            Kind::Popover(bounds, _) if bounds.get().is_none() => Some(format!(
                "popover {:?} is open but its anchor was never drawn",
                layer.id
            )),
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
    /// The help text, already final (hover help is Farhelm's own wording, never host-supplied text).
    pub text: SharedString,
    /// Where the pointer was when the hover began, in window coordinates.
    pub at: Point<Pixels>,
}

/// The hover on the control `owner` began: show `text` as its hover help after [`HOVER_DELAY`], at the pointer's
/// position, replacing any other tooltip. Shows at once while another tooltip is showing or has only just gone.
pub fn show_tooltip(
    owner: &Entity<TooltipOwner>,
    text: SharedString,
    window: &mut Window,
    cx: &mut App,
) {
    let at = window.mouse_position();
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let at_once = {
        let stack = stack.read(cx);
        stack.tooltip.is_some() || stack.warm
    };
    if at_once {
        show_tooltip_from(owner, text, at, window, cx);
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
            show_tooltip_from(&owner, text, at, window, cx)
        });
    });
    stack.update(cx, |stack, _| {
        stack.pending_tooltip = Some((owner_id, deadline));
    });
}

/// [`show_tooltip`] with the hover beginning at a fixed point, for the screenshot goldens.
#[cfg(test)]
pub(crate) fn show_tooltip_at(
    owner: &Entity<TooltipOwner>,
    text: SharedString,
    at: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    show_tooltip_from(owner, text, at, window, cx);
}

/// Show `text` for `owner` with the hover beginning at `at`.
fn show_tooltip_from(
    owner: &Entity<TooltipOwner>,
    text: SharedString,
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
            text,
            at,
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

/// Take down the tooltip if `owner` is the control showing it. A control leaving never hides another's tooltip, which
/// matters when the pointer moves straight from one control onto the next and the new control's hover arrives first.
pub fn hide_tooltip(owner: &Entity<TooltipOwner>, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let owner_id = owner.entity_id();
    let hidden = stack.update(cx, |stack, cx| {
        if stack
            .pending_tooltip
            .as_ref()
            .is_some_and(|(pending, _)| *pending == owner_id)
        {
            stack.pending_tooltip = None;
        }
        if stack
            .tooltip
            .as_ref()
            .is_some_and(|shown| shown.owner == owner_id)
        {
            stack.tooltip = None;
            stack.tooltip_release = None;
            cx.notify();
            return true;
        }
        false
    });
    // The pointer moved off a showing tooltip's control: the next one within the delay shows at once.
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

impl Layer {
    /// Whether the user may dismiss this layer now: what its guard says, and yes without one.
    fn dismissible(&self, cx: &App) -> bool {
        self.may_dismiss.as_ref().is_none_or(|guard| guard(cx))
    }
}

impl LayerStack {
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

// ---- Drawing ---------------------------------------------------------------------------------------------------------

/// Everything above the window's content, which the root view draws as its last child: each open layer, bottom to top,
/// then the tooltip.
#[allow(
    clippy::disallowed_methods,
    reason = "the layer manager is the one owner of deferred and anchored"
)]
pub fn overlay(window: &Window, cx: &App) -> Option<AnyElement> {
    let stack = stack(window, cx)?;
    let stack = stack.read(cx);
    if stack.layers.is_empty() && stack.tooltip.is_none() {
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
                        veil.items_start()
                            .pt((viewport.height * 0.12).min(px(80.)))
                            .child(content.max_h(viewport.height * 0.75).flex().flex_col())
                    }
                }
                .into_any_element()
            }
            Kind::Popover(anchor, placement) => {
                let pressed = anchor.clone();
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
                    child: content.into_any_element(),
                }
                .into_any_element()
            }
        };
        overlay = overlay.child(gpui::deferred(element).with_priority(depth + 1));
    }
    if let Some(shown) = &stack.tooltip {
        // Offset like a native tooltip so the pointer never covers the text.
        let at = point(shown.at.x + px(8.), shown.at.y + px(16.));
        let placed = gpui::anchored()
            .position(at)
            .snap_to_window()
            .child(tooltip_card(shown, window, cx));
        overlay = overlay.child(gpui::deferred(placed).with_priority(stack.layers.len() + 1));
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
        .child(shown.text.clone())
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
struct PlacedPopover {
    anchor: AnchorBounds,
    placement: Placement,
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
    type RequestLayoutState = LayoutId;
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
    ) -> (LayoutId, LayoutId) {
        let child = self.child.request_layout(window, cx);
        let style = Style {
            position: gpui::Position::Absolute,
            ..Style::default()
        };
        (window.request_layout(style, [child], cx), child)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let size = window.layout_bounds(*child).size;
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
        let offset = origin - bounds.origin;
        window.with_element_offset(point(offset.x.round(), offset.y.round()), |window| {
            self.child.prepaint(window, cx)
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
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
