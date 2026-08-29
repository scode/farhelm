//! Floating row-actions menu: the geometry, focus, and keyboard mechanics
//! shared by every row that gets a "⋯" menu — the session row (`list::row`,
//! PR #239) and the host row (`hosts`).
//!
//! ## Why this lives outside both rows
//!
//! The two menus render completely different ITEMS (rename/stop/archive/
//! delete versus profiles/retry/adopt/edit/remove) with completely different
//! visibility rules, so nothing here decides what a menu contains or what
//! its items do. What the two rows share instead is everything about a menu
//! that has NOTHING to do with what it lists: a `position: fixed` panel has
//! to escape its sidebar ancestor's `overflow: hidden` the same way regardless
//! of contents (see `menu_panel_style`'s own doc for why), the panel's own
//! screen position is a three-state measurement race (`PanelPlacement`)
//! whether the toggle belongs to a session or a host, and a `role="menu"`'s
//! roving-tabindex keyboard contract (arrows, Home/End, Escape, Tab) is the
//! same ARIA pattern either way. Duplicating that mechanism per row was tried
//! first, informally, while sketching the host row's menu, and the second
//! copy was line-for-line the first with only the item type swapped — exactly
//! the shape that belongs in one place instead of two that can drift.
//!
//! ## Why the item-list mechanics are GENERIC rather than copied
//!
//! [`MenuOrder`], [`MenuWiring`], [`handle_menu_key`], [`remember_menu_item`]
//! and [`focus_menu_item`] all need to know, for the one render in progress,
//! "what are today's visible items, in what order, and which DOM handle does
//! each one currently have" — and every operation built on that (arrow
//! navigation, Home/End, filing a freshly-mounted item's handle, moving focus
//! to a computed position) is IDENTICAL once that question can be asked,
//! whether the answer is a session's four actions or a host's five. Rather
//! than write that logic twice against two concrete enums, it is written
//! once against a type parameter `A` (the row's own action enum — `Copy +
//! Eq + Hash`, since a position is filed and looked up by the action it
//! performs, never by index — see `list::row`'s own note on why position
//! is never a durable identity) and a second parameter `Id` (the value the
//! row's OWN identity is threaded through when closing the menu — a
//! `String` session id, an `i64` `HostId`). The array length `N` is a const
//! generic rather than a `Vec` because [`MenuOrder`] and [`MenuWiring`] both
//! have to stay `Copy`: every per-item event closure in a row's `rsx!`
//! captures its own copy of the wiring, and a heap-allocating `Vec` would
//! force each of those closures into a clone dance instead.
//!
//! What is deliberately NOT made generic, because genericizing it would
//! only move the row-specific knowledge one level up without removing it:
//! which items are visible for a given row state (each row's own
//! `*_menu_order` constructor, built on the shared [`MenuOrder::pack`]),
//! what each item's label and click handler are (each row's own `rsx!`),
//! and how many keyboard-driven sub-states the open panel can show (the
//! session row's confirm/rename swap the panel's CONTENTS in place; the
//! host row has no such sub-state at all — its confirm/edit surfaces
//! replace the whole row line instead, a design difference `hosts::HostRow`
//! records where it applies). A shared abstraction that tried to also cover
//! those would need to know things about both rows that neither needs to
//! know about the other.

use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

use dioxus::html::geometry::PixelsRect;
use dioxus::prelude::*;

/// Vertical gap between the toggle's bottom edge and the floating panel's
/// top edge, in pixels — the fixed-position replacement for the old
/// `top: 2.4em` (see `.session-row-menu-panel` in app.css for the switch
/// from an absolutely-positioned, row-anchored panel to one positioned
/// against the viewport). Small on purpose: the panel is meant to read as
/// hanging off the button that opened it, and a wider gap is exactly the
/// untethered float the menu redesign set out to end.
const MENU_PANEL_GAP_PX: f64 = 2.0;

/// Horizontal gap between the toggle's LEFT edge and the panel's RIGHT
/// edge, in pixels. Small for the same reason `MENU_PANEL_GAP_PX` is —
/// the panel has to read as hanging off the button that opened it — but
/// non-zero on purpose: at zero the panel's corner would touch the
/// toggle's, and a touching edge reads as one continuous surface rather
/// than as a popup tethered to a button. See `menu_panel_style`'s "## The
/// anchor" for why the panel hangs to the LEFT of the toggle at all rather
/// than flush under it.
const MENU_PANEL_TOGGLE_GAP_PX: f64 = 4.0;

/// How close to the viewport's own edges any clamp here is willing to put
/// the panel, in pixels — the numeric twin of the literal `8px` floors in
/// the emitted `top`/`right` expressions below, used where the same margin
/// has to be arithmetic rather than CSS.
const MENU_PANEL_VIEWPORT_MARGIN_PX: f64 = 8.0;

/// A conservative reserve, in pixels, subtracted from the viewport height
/// when clamping the panel's `top`. This is a CEILING on the clamp, not a
/// claim about the panel's real height — the panel's mode (menu / confirm
/// / rename) changes that height at runtime, and it is
/// `.session-row-menu-panel`'s own `max-height` + `overflow-y: auto` in
/// app.css that actually guarantees the panel stays on screen regardless
/// of how far off this estimate runs. This constant only keeps the clamp
/// from placing the panel's TOP so close to the viewport's bottom that
/// even its shortest mode would still be pushed off.
const MENU_PANEL_MIN_RESERVE_PX: f64 = 160.0;

/// The panel's own floor on how narrow `menu_panel_style`'s horizontal
/// clamp (below) may shrink it to. Below this, the panel could no longer
/// show its own button labels without wrapping every word — a worse
/// failure than the few pixels of edge overflow this floor accepts
/// instead in the single extreme case that forces it (see that clamp's
/// own doc).
const MENU_PANEL_MIN_WIDTH_PX: f64 = 96.0;

/// Computes the fixed-position panel's `top`/`right`/`max-width` inline
/// style from the toggle button's own viewport rect
/// (`MountedData::get_client_rect`, measured on open — see the row's own
/// toggle `onclick`). Callers reach this only through
/// `menu_panel_placement_style`, which adds the `opacity`/`pointer-events`
/// every state must restate — see that function's own doc for why this
/// one deliberately does not.
///
/// ## The anchor
///
/// The panel hangs BELOW-LEFT of the toggle: its top-right corner meets
/// the toggle's bottom-left corner, `MENU_PANEL_GAP_PX` down and
/// `MENU_PANEL_TOGGLE_GAP_PX` left. The toggle COLUMN — the narrow strip
/// every row's "⋯" occupies at the sidebar's trailing edge — is therefore
/// never covered by an open panel, no matter how far the panel extends
/// down over the rows below.
///
/// That is the whole reason for the offset, and it is a safety property
/// rather than a taste one. The rows this panel covers are session rows
/// whose own "⋯" sits at a fixed x, and the panel's items at that point
/// in its list are Stop and Delete: Stop kills a running process tree and
/// Delete removes an ended session outright. A user reaching for a
/// neighboring row's menu aims at a toggle they can see; if the panel
/// covered that spot, the click would land on this row's destructive item
/// instead of the other row's toggle — the wrong session, and a click the
/// user never intended to be an action at all. The host row's menu keeps
/// the same anchor for the same reason once `remove` moved into it: the
/// hosts panel stacks rows exactly as densely as the session list does.
///
/// The flush anchor (panel's right edge aligned with the toggle's, the
/// ordinary menu-button placement) was written first and REJECTED for
/// exactly that: it is the tidier tether, but it is the one placement
/// that puts destructive items under the pixels other toggles occupy.
/// This is the record that the trade was decided rather than overlooked.
/// The left offset costs the panel its flush edge and spends most of the
/// 340px sidebar's width on a 288px surface, so three other cues carry
/// "which row owns this menu" instead: the opening toggle holds a pressed
/// state and its row a highlight (see
/// `.session-row-menu[aria-expanded="true"]` and `.session-row.menu-open`
/// in app.css), and the panel is a raised surface with a shadow rather
/// than a flat box, so it reads as being in front of the rows it covers.
/// With the toggle column clear, switching menus stays ONE click on the
/// other row's "⋯" — no close-then-open — which is also what the browser
/// suite's existing multi-row helpers assume.
///
/// `position: fixed` resolves these axes against the VIEWPORT, which is
/// exactly what lets the panel escape `.session-list`'s and
/// `.app-sidebar`'s `overflow` clipping that an absolutely-positioned,
/// row-anchored panel could never escape (opening the menu on a row near
/// the bottom of a full list used to cut the panel off at the scroll
/// container's edge — see `.session-row-menu-panel` in app.css). Each
/// axis is clamped for a DIFFERENT reason:
///
/// - `top` is clamped on BOTH ends: `min()` keeps a toggle near the
///   bottom of a short viewport from pushing the panel's START past a
///   point from which even its shortest mode would run off screen — see
///   `MENU_PANEL_MIN_RESERVE_PX` for why this is a ceiling, not a
///   promise — while the outer `max(8px, ...)` is the floor: on a
///   viewport shorter than `MENU_PANEL_MIN_RESERVE_PX` the `min()` alone
///   would go negative, pushing the panel's top edge ABOVE the viewport
///   (off screen the other direction, and in the opposite sense
///   `max-height` below assumes). `8px` keeps it a hair inside the top
///   edge instead, matching the fallback's own resting position in
///   app.css.
/// - `right` is expressed as `calc(100vw - Xpx)` rather than a raw pixel
///   offset: `right` measures from the viewport's RIGHT edge, but the
///   toggle's rect only gives an offset from the viewport's LEFT edge.
///   `100vw` lets the browser/webview do that unit conversion at paint
///   time (and re-do it for free on resize) instead of this function
///   needing to ask the renderer for the viewport's width itself, which
///   would mean a `document::eval` round trip this fix deliberately
///   avoids. The outer `max(8px, ...)` floors `right` itself: if
///   `right_edge` ever reached or exceeded the viewport's own width (a
///   toggle scrolled to the far right of an unusually narrow shell), the
///   unclamped expression would go negative and push the panel's right
///   edge PAST the viewport's own right side.
/// - `max-width` shrinks from the CSS class's static 288px when there
///   isn't `288px + MENU_PANEL_VIEWPORT_MARGIN_PX` of room between the
///   panel's right edge and the viewport's LEFT edge — the horizontal
///   twin of the `top` clamp. Without it, a toggle sitting close to the
///   viewport's left edge (the `.app-shell`-scrolled extreme: a narrow
///   window scrolled far enough that the sidebar is partway off screen)
///   would still try to grow the panel a full 288px leftward from
///   `right_edge`, running its LEFT edge off screen. Floored at
///   `MENU_PANEL_MIN_WIDTH_PX` rather than left to go negative — CSS
///   treats a negative `max-width` as an INVALID declaration, which drops
///   the whole property and silently reverts to the class's unclamped
///   288px, the opposite of what this clamp exists for. The floor is a
///   documented, accepted residual, not a full fix: in that one extreme
///   (`right_edge` closer to the viewport's left edge than
///   `MENU_PANEL_MIN_WIDTH_PX + MENU_PANEL_VIEWPORT_MARGIN_PX`), the
///   panel may still hang a few pixels past the screen's left edge — a
///   narrower panel than that would stop being legible, which is the
///   worse failure between the two.
///
///   The `calc(100vw - 16px)` term beside it closes the mirror-image
///   hole, and it has to be a THIRD term rather than a different way of
///   writing the second: the arithmetic above works from the RAW
///   `right_edge`, while `right` itself is floored at `8px`, so the two
///   disagree exactly when `right_edge` sits at or past the viewport's
///   own right edge (a narrow, horizontally scrolled shell). There the
///   panel is painted with its right edge at `100vw - 8px` while
///   `right_edge - 8` claims a width measured from somewhere off screen,
///   and long content would take the panel's LEFT edge off the other
///   side. `100vw - 16px` is the two `8px` margins the clamped placement
///   actually leaves, so `min()` hands the win to whichever bound is
///   really binding at paint time — computed by the engine, which is the
///   only party that knows the viewport's width (see the `right` bullet
///   for why this function deliberately never asks for it).
fn menu_panel_style(toggle_rect: PixelsRect) -> String {
    let top = toggle_rect.max_y() + MENU_PANEL_GAP_PX;
    // Left of the toggle, not flush with it — see "## The anchor" above
    // for the covered-toggle hazard that placement exists to avoid.
    let right_edge = toggle_rect.min_x() - MENU_PANEL_TOGGLE_GAP_PX;
    let max_width = (right_edge - MENU_PANEL_VIEWPORT_MARGIN_PX).max(MENU_PANEL_MIN_WIDTH_PX);
    // The clamped top is emitted ONCE, as a custom property, so the
    // `max-height` can be derived from the same value: `calc(100vh -
    // top - 8px)` is an EXACT ceiling on the space below the panel's
    // start, where the class's static `max-height` (a fallback for the
    // never-measured case) can only bound the panel against the whole
    // viewport. Without this, a mode taller than
    // `MENU_PANEL_MIN_RESERVE_PX` opened near the viewport's bottom
    // would still overrun the screen edge by the difference — the same
    // clipping this fix exists to end, just smaller.
    format!(
        "--menu-top: max(8px, min({top}px, calc(100vh - {MENU_PANEL_MIN_RESERVE_PX}px))); \
         top: var(--menu-top); \
         right: max(8px, calc(100vw - {right_edge}px)); \
         max-width: min(288px, {max_width}px, calc(100vw - 16px)); \
         max-height: calc(100vh - var(--menu-top) - 8px);"
    )
}

/// Where the actions panel currently believes its own screen position is,
/// while an async `get_client_rect()` measurement races the render that
/// opened it.
///
/// The toggle's `onclick` opens the menu SYNCHRONOUSLY (see its own doc):
/// `menu_open` flips before any `await`, so the panel is already mounted
/// by the time a measurement can even start. `Unmeasured` is what that
/// first render is — the panel exists in the DOM (so its own height is
/// ready and tab order includes it, and any `autofocus` element inside it
/// — the delete/archive confirm's cancel button, the rename field — can
/// actually RECEIVE that focus; see `menu_panel_placement_style`'s own
/// doc for why hiding via `visibility` would have silently broken that)
/// but paints nothing, which is what keeps a still-pending measurement
/// from ever flashing the panel at the CSS fallback position for one
/// frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PanelPlacement {
    /// No measurement has resolved yet for the CURRENT open — freshly
    /// reset every time the toggle opens (see its `onclick`), because a
    /// rect measured for a PREVIOUS open can be stale (the toggle can
    /// move between opens: a row above it changing height, the window
    /// resizing) and is not safe to reuse as a stand-in.
    Unmeasured,
    /// `get_client_rect()` resolved; the panel renders at these exact
    /// viewport coordinates via `menu_panel_style`.
    Measured(PixelsRect),
    /// The measurement itself failed — no mounted handle yet, or the
    /// renderer's `get_client_rect()` returned `Err` — as opposed to
    /// `Unmeasured`'s "hasn't resolved YET". This is not transient: it
    /// persists for as long as the panel stays open in this state (a
    /// renderer with no `get_client_rect()` support will fail on every
    /// future measurement too), so the panel pins itself top-left over
    /// the sidebar (coordinates emitted inline by
    /// `menu_panel_placement_style` — the class deliberately carries NO
    /// positional fallback, see its app.css comment) rather than staying
    /// invisible — a toggle whose renderer cannot answer this query must
    /// still open to SOMETHING, never become a dead button.
    Fallback,
}

/// Maps a row's current `PanelPlacement` to the actions panel's inline
/// `style` attribute — the presentation half of the state machine
/// `PanelPlacement` itself documents.
///
/// `Unmeasured` hides the panel with `opacity: 0; pointer-events: none;`,
/// NOT `visibility: hidden`. This is load-bearing, not a style
/// preference: the confirm/rename sub-states use the plain HTML
/// `autofocus` attribute (see the session row's own doc) to land keyboard
/// focus the instant they mount, and confirm/rename state SURVIVES
/// closing and reopening the panel (`list::view::ListView` tracks them
/// independently of `menu_open`) — so a row mid-confirmation that gets
/// closed and reopened remounts its `autofocus` cancel button while
/// `placement` is freshly `Unmeasured` again. The HTML focusable-area
/// algorithm requires a computed `visibility` of `visible` for an element
/// to be focusable at all; `visibility: hidden` would make that
/// remounted button UNFOCUSABLE at the exact moment `autofocus` tries to
/// focus it, and — per spec — a browser does not retry `autofocus` later
/// once the candidate has been processed, so the safety default (focus
/// lands on the SAFE action before a stray Enter/Space can reach
/// anything) would silently fail for the rest of that open. `opacity` is
/// not part of that algorithm at all: an `opacity: 0` element is exactly
/// as focusable as a fully opaque one, so `autofocus` still succeeds
/// while the panel is invisible, and nothing needs to re-focus anything
/// once a measurement resolves and paints it. `pointer-events: none` is
/// what still keeps it from intercepting stray clicks while invisible.
///
/// EVERY non-`Unmeasured` branch explicitly restates `opacity: 1;
/// pointer-events: auto;` — never omits them to "fall through" to
/// whatever the CSS class or an earlier render already set. This is not
/// defensive redundancy: Dioxus's own JS interpreter does NOT treat a
/// `style` attribute update as a plain replace (see
/// `.session-row-menu-panel`'s own app.css doc for the exact mechanism,
/// confirmed by reading `dioxus-interpreter-js` directly) — it RESTORES
/// any inline style property the new string omits from whatever the
/// PREVIOUS update on that same DOM node left behind. Omitting these two
/// properties once `Unmeasured` has already set them to `0`/`none` would
/// not fall back to the stylesheet at all; it would silently carry
/// `Unmeasured`'s hidden, inert styling forward into the state that is
/// supposed to make the panel visible and usable again.
pub(crate) fn menu_panel_placement_style(placement: PanelPlacement) -> String {
    // Every positional property ANY state sets is restated by every state
    // that must not inherit it: the interpreter's style-restore quirk (see
    // the type doc) carries an omitted property's previous value forward
    // across `style` updates, so `Fallback`'s `left: 8px` would otherwise
    // resurface under a later `Measured` render — and a fixed box with
    // both `left` and `right` set stretches between them, ballooning the
    // panel across the sidebar (the exact regression a class-level
    // fallback `left` caused before coordinates moved fully in here).
    match placement {
        PanelPlacement::Unmeasured => "opacity: 0; pointer-events: none;".to_string(),
        PanelPlacement::Measured(rect) => {
            format!(
                "opacity: 1; pointer-events: auto; left: auto; {}",
                menu_panel_style(rect)
            )
        }
        PanelPlacement::Fallback => "opacity: 1; pointer-events: auto; \
                                     top: 8px; left: 8px; right: auto; \
                                     max-width: 288px; max-height: calc(100vh - 16px);"
            .to_string(),
    }
}

/// Whether the toggle's `onmounted` should start its OWN measurement
/// rather than leaving one to the `onclick` path that ordinarily starts
/// every measurement.
///
/// `onmounted` fires once per DOM node, on every mount — including a
/// remount that lands with the menu already open (see the session row's
/// `onmounted` rationale for the failed-then-recovered listing read: a
/// fresh row-local `placement` signal starts back at `Unmeasured` with no
/// click of its own to trigger a measurement). Extracted as a pure
/// predicate — rather than left as the inline `if` at the one call site —
/// so this decision can be pinned by a unit test independent of the
/// `onmounted` event, `MountedData`, and the `spawn`ed task around it,
/// none of which a headless `VirtualDom` test (no real renderer attached)
/// can exercise at all.
pub(crate) fn should_measure_on_mount(menu_open: bool, placement: PanelPlacement) -> bool {
    menu_open && placement == PanelPlacement::Unmeasured
}

/// Whether a resolved measurement is still current, and what `placement`
/// should become if so — the apply-if-current half of a row's own
/// `spawn_measurement` async body, split out for the same reason
/// `should_measure_on_mount` is: a plain unit test can pin it, where the
/// `spawn`ed task, the `Signal` captures, and the real `await` around it
/// cannot run at all under a headless `VirtualDom` with no renderer attached.
///
/// `None` means "discard this measurement": `generation_now` has moved past
/// `generation_at_capture`, so a newer open has already superseded it (see
/// the `open_generation` rationale each row keeps for the exact
/// interleaving this guards against — two opens of the same toggle close
/// enough together that their measurements can resolve out of order).
/// Applying a stale result here
/// would silently overwrite whatever the newer open's own measurement
/// already wrote, or will still write, with coordinates for a toggle
/// position this open no longer speaks for.
///
/// `Some(_)` is the ordinary case: still current, so the measurement's own
/// outcome decides the placement — `Measured` on a resolved rect, `Fallback`
/// on a renderer that could not answer `get_client_rect()` at all (no
/// mounted handle yet, or the query itself failed).
pub(crate) fn measurement_outcome(
    generation_at_capture: u64,
    generation_now: u64,
    measured: Option<PixelsRect>,
) -> Option<PanelPlacement> {
    if generation_at_capture != generation_now {
        return None;
    }
    Some(measured.map_or(PanelPlacement::Fallback, PanelPlacement::Measured))
}

// ===== Item-list mechanics, generic over the row's own action enum ====
//
// Everything below identifies a menu item by WHAT IT DOES (the type
// parameter `A`) rather than by where it currently sits — the fix for a
// real bug the session row hit first: its item set is not fixed for the
// life of an open menu (archiving a session withdraws Stop and Archive
// while Delete's DOM node survives in place), so a scheme that filed
// handles under "index 3" left a surviving item's handle at an index the
// shorter list no longer reaches. Positions are derived from `MenuOrder`
// at the moment a key is pressed; nothing durable is ever keyed by one.
// The host row's menu has the identical hazard the moment a poll changes
// which items `adoptable` and `manageable` allow while the menu is open,
// which is why this is written once, generically, rather than copied.

/// Handles for the menu items currently mounted, keyed by the action each
/// one performs.
///
/// Row-local and reset with the row, exactly like the toggle's own
/// measurement handle — the row that owns a menu's DOM handles is never
/// the parent that merely decides whether it is open. Cleared on every
/// fresh open AND on every close, so a handle never outlives the panel
/// that mounted it: the entries are strong `Rc<MountedData>`s, and on the
/// web renderer a retained handle retains a detached DOM node with it.
pub(crate) type MenuItemHandles<A> = Signal<HashMap<A, Rc<MountedData>>>;

// ===== Moving focus, one request at a time ===========================

/// One row's serialized pipeline of `MountedData::set_focus` requests.
///
/// Focus is asynchronous here — on desktop it crosses the WebView bridge
/// — and a user holding an arrow key down produces requests far faster
/// than the bridge answers them. Spawning an independent task per
/// keystroke let those tasks interleave: each computed its target from
/// whatever was focused when it STARTED (still the same item, since none
/// had landed yet), mixed directions could complete out of order, and a
/// slow bridge accumulated tasks each retaining a DOM handle. This queue
/// replaces that with the two properties that make the behavior
/// predictable: at most ONE request is ever in flight, and a request made
/// while one is in flight simply overwrites the pending target instead of
/// queueing behind it. Coalescing is the right semantic for focus — only
/// the last destination asked for can be observed anyway — and it is what
/// bounds the outstanding work no matter how long the bridge takes.
#[derive(Clone, Copy)]
pub(crate) struct MenuFocusQueue {
    /// Where focus should end up, overwritten by each new request and
    /// taken by the drainer. `None` means nothing is waiting. `pub(crate)`
    /// rather than a constructor: each row builds this struct from a pair
    /// of its OWN `use_signal` hooks (see e.g. `SessionRow`'s
    /// `focus_queue`), and a `MenuFocusQueue::new()` that called
    /// `Signal::new` internally would silently stop being a hook — a
    /// fresh, un-persisted signal minted on every render — instead of the
    /// row's own per-render-stable hook state.
    pub(crate) target: Signal<Option<Rc<MountedData>>>,
    /// Whether a task is currently draining `target`. Guards against a
    /// second drainer, which is what would reintroduce the interleaving.
    pub(crate) draining: Signal<bool>,
}

/// Ask for focus on `handle`, superseding any request not yet delivered.
///
/// The `Result` from `set_focus` is discarded deliberately — see this
/// module's "Moving focus inside the open menu" note (mirrored on each
/// row) for why a renderer that cannot move focus is an unimproved
/// keyboard experience rather than a lost safety default.
pub(crate) fn request_menu_focus(queue: MenuFocusQueue, handle: Rc<MountedData>) {
    let mut queue = queue;
    queue.target.set(Some(handle));
    // A drainer already running will pick the new target up on its next
    // turn; starting a second one is exactly the concurrency this exists
    // to prevent.
    if *queue.draining.peek() {
        return;
    }
    queue.draining.set(true);
    spawn(async move {
        let mut queue = queue;
        loop {
            // The write guard is scoped to this statement on purpose: it
            // must be released before the `await` below, or the next
            // `request_menu_focus` (which runs while this task is
            // suspended) would panic against a borrow still outstanding.
            let next = queue.target.write().take();
            let Some(next) = next else {
                break;
            };
            let _ = next.set_focus(true).await;
        }
        queue.draining.set(false);
    });
}

/// Drop any request that has not been delivered yet.
///
/// Called when the menu closes: a target captured for a panel that no
/// longer exists would focus a detached node. The one request already
/// awaiting the renderer cannot be recalled, but focusing a detached node
/// is a browser no-op, so the residual is nothing a user can observe.
pub(crate) fn cancel_menu_focus(mut queue: MenuFocusQueue) {
    queue.target.set(None);
}

/// One render's item list: the visible subset of a row's fixed action set,
/// packed to the front in the order the row declares.
///
/// Fixed-capacity (`[Option<A>; N]`) rather than a `Vec` so the whole thing
/// stays `Copy`: every per-item event closure in a row's `rsx!` captures
/// it, and a `Copy` value needs no clone dance to hand the same list to
/// several closures. Only the leading `Some` entries are meaningful —
/// [`MenuOrder::pack`] packs the visible actions to the front, so
/// iteration stops at the first gap by construction.
///
/// `N` is the row's OWN total action count (4 for the session row, 5 for
/// the host row) — a compile-time fact each row bakes into its own
/// `MenuOrder` type alias, never inferred or shared across rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MenuOrder<A, const N: usize>([Option<A>; N]);

impl<A: Copy + Eq, const N: usize> MenuOrder<A, N> {
    /// Packs the actions `visible` accepts, in `all`'s own declared order,
    /// to the front of a fresh list.
    ///
    /// `all` is expected to be the row's complete, fixed action set (its
    /// `MENU_ACTIONS`/`HOST_MENU_ACTIONS` constant) — the canonical order
    /// every row keeps in one place so the rendered list and the
    /// navigable list cannot disagree about what "the first item" or "the
    /// last item" means. `visible` is the row's own state-dependent
    /// answer for each one (an archived session withdraws Stop and
    /// Archive; a non-ssh host withdraws Edit and Remove).
    pub(crate) fn pack(all: [A; N], mut visible: impl FnMut(A) -> bool) -> Self {
        let mut packed = [None; N];
        let mut next = 0;
        for action in all {
            if visible(action) {
                packed[next] = Some(action);
                next += 1;
            }
        }
        Self(packed)
    }

    /// How many items arrow navigation has to wrap around.
    pub(crate) fn len(self) -> usize {
        self.actions().count()
    }

    /// The action at a focus position, or `None` for a position this
    /// render's list does not have — the shape `next_menu_focus` already
    /// treats as "not on an item".
    pub(crate) fn get(self, position: usize) -> Option<A> {
        self.actions().nth(position)
    }

    /// The final item in this render's list — what ArrowUp and End resolve
    /// to when focus is not currently on any item (see [`next_menu_focus`]).
    pub(crate) fn last(self) -> Option<A> {
        self.actions().last()
    }

    /// Where an action sits in THIS render's list, which is the only
    /// place a position number is ever produced.
    pub(crate) fn position(self, action: A) -> Option<usize> {
        self.actions().position(|candidate| candidate == action)
    }

    /// The visible actions in order, skipping the trailing `None` gaps
    /// [`pack`](Self::pack) leaves behind — the one iterator every other
    /// method here is built from.
    fn actions(self) -> impl Iterator<Item = A> {
        self.0.into_iter().flatten()
    }
}

/// Everything a key press or a mount inside one row's open menu needs to
/// reach.
///
/// Bundled — and kept `Copy` — so the several rsx closures that need it can
/// each capture it without cloning, and so adding a piece of menu state
/// does not mean threading another argument through every call site.
/// Generic over the row's own action enum `A` and its own identity type
/// `Id` (a session's `String` id, a host's `HostId`) — see the module doc
/// for why both vary per row while everything else here does not.
pub(crate) struct MenuWiring<A: 'static, Id: 'static, const N: usize> {
    /// This render's item list, the only source of position numbers.
    pub(crate) order: MenuOrder<A, N>,
    /// The DOM handles of whichever items have mounted so far this open —
    /// see [`MenuItemHandles`]'s own doc for the lifecycle rule.
    pub(crate) handles: MenuItemHandles<A>,
    /// This row's serialized focus-request pipeline — see
    /// [`MenuFocusQueue`]'s own doc for the interleaving it prevents.
    pub(crate) focus: MenuFocusQueue,
    /// Which item focus is on, or `None` when it is not on one — see each
    /// row's own `menu_focus` for what maintains it and what reads it.
    pub(crate) focused: Signal<Option<usize>>,
    /// Where focus should land as the panel mounts, honoured once and
    /// then cleared (see [`MenuOpenIntent`]).
    pub(crate) open_intent: Signal<Option<MenuOpenIntent>>,
    /// The row's own close path — the parent's toggle callback, which is
    /// the same one a click on the "⋯" uses.
    pub(crate) close_menu: EventHandler<Id>,
}

// Written by hand rather than `#[derive(Clone, Copy)]`: the derive macro
// adds a `T: Copy` bound for EVERY generic parameter a struct carries,
// with no way to tell it that `close_menu: EventHandler<Id>` is Copy for
// ANY `Id: 'static` — `dioxus_core::Callback` (`EventHandler`'s alias)
// implements `Copy` unconditionally, precisely so a plain `String` id
// works here the way the session row's original, non-generic `MenuWiring`
// already relied on. A derived bound of `Id: Copy` would compile (`HostId`
// is `i64`) but reject a session `String` id outright, silently narrowing
// this type to only the host row's use. The manual impls below assert
// only what soundness actually requires: `A: Copy` (needed for
// `MenuOrder<A, N>` itself to be `Copy`) and nothing about `Id`.
impl<A: Copy, Id: 'static, const N: usize> Clone for MenuWiring<A, Id, N> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Copy, Id: 'static, const N: usize> Copy for MenuWiring<A, Id, N> {}

// ===== Moving focus inside the open menu =============================
//
// The helpers below are the impure half of the keyboard support whose
// decisions live in the pure functions further down (`menu_key_action`,
// `next_menu_focus`, `closed_toggle_key_intent`): they touch real events
// and real DOM handles, so they cannot be exercised by a headless
// `VirtualDom` and are kept as small as the job allows, with every branch
// that could go wrong pushed into the pure functions instead.
//
// Every focus move INSIDE the menu routes through `MenuFocusQueue` rather
// than spawning its own task; the one that leaves it — each row's own
// `focus_menu_toggle` call — goes through JS instead, for a reason its own
// doc gives. All of them are fire-and-forget, discarding the `Result` that
// `MountedData::set_focus` returns. That is the opposite of what each row
// does for its own confirm prompt's cancel button (declarative `autofocus`
// precisely so a dropped `Result` cannot silently lose a SAFETY default).
// The difference is what failure costs. A renderer that cannot move focus
// leaves the arrow keys inert and the menu closing without restoring
// focus — an unimproved keyboard experience, identical to what existed
// before any of this — and there is no fallback to reach for anyway, since
// the whole point of these calls is to focus an element chosen at
// runtime, which no static attribute can express.

/// Apply one key press arriving on a menu's toggle or on one of its
/// items — the single place the pure decisions above meet a real event
/// and a real DOM handle.
///
/// `current` is the position of the item the key arrived on, or `None`
/// when it arrived on the toggle.
///
/// `prevent_default` fires only for a key the menu CLAIMS, which is what
/// keeps it out of the way of everything it does not implement: Enter and
/// Space still activate the focused `<button>` natively rather than being
/// swallowed here. The arrows and Home/End are claimed because their
/// default action is to scroll, which would move the sidebar out from
/// under the very menu they are navigating; Tab is claimed on an item for
/// the timing reason [`MenuKeyAction::Exit`] records, and deliberately
/// NOT claimed on the toggle, where it is how the browser walks into the
/// open menu.
pub(crate) fn handle_menu_key<A: Copy + Eq + Hash, Id: Clone, const N: usize>(
    evt: &Event<KeyboardData>,
    current: Option<usize>,
    wiring: MenuWiring<A, Id, N>,
    row_id: &Id,
) {
    let Some(action) = menu_key_action(&evt.key()) else {
        return;
    };
    // Tab arriving on the TOGGLE is not an exit from anything: with the
    // roving `tabindex` each row sets up, the menu's one tabbable item is
    // the next stop in document order, so the browser's own default is
    // what walks INTO the open menu. Closing here would take that
    // destination away mid-keystroke.
    if action == MenuKeyAction::Exit && current.is_none() {
        return;
    }
    evt.prevent_default();
    let count = wiring.order.len();
    let target = match action {
        // Both closes hand the focus decision to the row's own dismissal
        // effect rather than making it here, so that Escape, Tab, and an
        // automatic dismissal all resolve focus through one path — which
        // is also what keeps the destination consistent no matter which
        // of them fired.
        MenuKeyAction::Close | MenuKeyAction::Exit => {
            wiring.close_menu.call(row_id.clone());
            return;
        }
        MenuKeyAction::Step(direction) => next_menu_focus(count, current, direction),
        MenuKeyAction::First => next_menu_focus(count, None, MenuFocusMove::Next),
        MenuKeyAction::Last => next_menu_focus(count, None, MenuFocusMove::Previous),
    };
    if let Some(position) = target {
        focus_menu_item(wiring, position);
    }
}

/// Record a freshly mounted menu item's handle under the action it
/// performs, and honour a pending open-intent if this is the item that
/// intent named.
///
/// Doing the open-focus here rather than in an effect is what makes it
/// independent of mount ORDER: `onmounted` fires per item with no
/// guaranteed sequence, so instead of waiting for "all items mounted"
/// (which nothing reports) each item asks whether IT is the one wanted.
/// Exactly one can answer yes, and it does so holding its own handle.
///
/// Takes the signals by value because `Signal` is a `Copy` handle into
/// shared storage: each item's closure captures its own copy, and they
/// all write to the one map.
pub(crate) fn remember_menu_item<A: Copy + Eq + Hash, Id: 'static, const N: usize>(
    wiring: MenuWiring<A, Id, N>,
    action: A,
    data: Rc<MountedData>,
) {
    let mut handles = wiring.handles;
    handles.write().insert(action, data.clone());
    let wanted = match *wiring.open_intent.peek() {
        Some(MenuOpenIntent::First) => wiring.order.get(0),
        Some(MenuOpenIntent::Last) => wiring.order.last(),
        None => None,
    };
    if wanted != Some(action) {
        return;
    }
    let mut open_intent = wiring.open_intent;
    open_intent.set(None);
    let mut focused = wiring.focused;
    focused.set(wiring.order.position(action));
    request_menu_focus(wiring.focus, data);
}

/// Move keyboard focus onto the menu item at `position`, if that item is
/// in this render's list and has actually mounted.
///
/// `focused` is updated SYNCHRONOUSLY, before the asynchronous focus
/// request goes out, so the roving `tabindex` and the next key press both
/// see the intended destination rather than whatever the renderer has
/// caught up to. Silently does nothing for a position or an action no
/// item has claimed: not an expected state — every position handed here
/// comes from `next_menu_focus`, bounded by this render's own item count
/// — but a menu that quietly declines to move focus is a far better
/// failure than one that panics while the user holds a key down.
pub(crate) fn focus_menu_item<A: Copy + Eq + Hash, Id: 'static, const N: usize>(
    wiring: MenuWiring<A, Id, N>,
    position: usize,
) {
    let Some(action) = wiring.order.get(position) else {
        return;
    };
    let Some(handle) = wiring.handles.peek().get(&action).cloned() else {
        return;
    };
    let mut focused = wiring.focused;
    focused.set(Some(position));
    request_menu_focus(wiring.focus, handle);
}

/// Put focus back on the "⋯" that owned a just-closed menu, WITHOUT
/// scrolling to reach it.
///
/// Called from a row's dismissal teardown, and it is the half of "the
/// menu closed" that matters to a keyboard user: closing alone destroys
/// the element holding focus and drops them at the top of the document,
/// several dozen Tab presses from the row they were working on.
///
/// `id_attr`/`id_value` name the row: `("data-session-id", &session.id)`
/// for the session row, `("data-host-id", &host.id.to_string())` for the
/// host row — the DOM marker every row already carries for the browser
/// suite, reused here instead of adding a second one. `toggle_selector` is
/// the row's own toggle class (`.session-row-menu` / `.host-row-menu`).
///
/// ## Why this one does not use `MountedData::set_focus`
///
/// `preventScroll` is the whole reason, and it is not a nicety. One of
/// the things that CLOSES a menu is the sidebar scrolling (`ListView`
/// watches for it, because a `position: fixed` panel does not travel with
/// its row). If the teardown then focused the toggle through
/// `set_focus` — which the web renderer implements as a plain
/// `HTMLElement.focus()`, with the default "scroll it into view" — the
/// browser would scroll the row the user just scrolled AWAY from back
/// into view, undoing the very gesture that dismissed the menu. Focus
/// belongs on the toggle; the viewport belongs to the user.
///
/// Dioxus's `set_focus` takes no options, so the option has to come from
/// JS. The script compares the row's identity attribute as a STRING
/// rather than interpolating it into a selector, so a value carrying a
/// quote could not reshape the query: `id_value` is a session id — reported
/// by the far end's supervisor, and under `--ssh` a genuinely untrusted one
/// — for the session row, or a `HostId` converted to text for the host row.
/// The host case needs no defending in practice (`HostId` is an integer
/// this helm's own registry assigns, never a value the remote end
/// supplies), but this helper is shared, and treating both row kinds'
/// identity with the one discipline the untrusted case actually requires is
/// simpler than asking each future caller to judge whether its own id needs
/// it. `serde_json` produces every literal here, and nothing about the
/// row's identity is trusted beyond that.
///
/// Fire-and-forget, like every other focus call here: a renderer that
/// cannot run this leaves the keyboard experience unimproved rather than
/// losing a safety default (see this section's own note).
pub(crate) fn focus_menu_toggle(id_attr: &str, id_value: &str, toggle_selector: &str) {
    let id_js = serde_json::to_string(id_value).expect("a string is serializable");
    let attr_js = serde_json::to_string(id_attr).expect("a string is serializable");
    let toggle_js = serde_json::to_string(toggle_selector).expect("a string is serializable");
    document::eval(&format!(
        r#"(() => {{
            const wanted = {id_js};
            const attrName = {attr_js};
            const toggleSelector = {toggle_js};
            for (const row of document.querySelectorAll(`[${{attrName}}]`)) {{
                if (row.getAttribute(attrName) === wanted) {{
                    row.querySelector(toggleSelector)?.focus({{ preventScroll: true }});
                    return;
                }}
            }}
        }})();"#
    ));
}

/// A row identity title as it may appear inside an accessible name,
/// clamped to a length worth reading aloud.
///
/// Shared by both rows' menu labels and prompt names, so every accessible
/// name either produces clamps the same way. A title/destination has no
/// length bound the UI enforces (tens of KB is legal on the wire) and an
/// accessible name is read aloud in full; 64 characters is plenty to tell
/// rows apart, and the ellipsis says something was cut. Char-based, not
/// byte-based, so a multi-byte value can never split a codepoint.
///
/// Callers that build a label from PEER-supplied text (`hosts::host_menu_label`)
/// are expected to run it through [`display_peer`](crate::peer::display_peer)
/// FIRST, so this clamp routinely sees `<U+XXXX>` escape tokens rather than
/// the live control characters they stand for. The naive char-count cut
/// point can land inside one of those eight-character tokens — the
/// character-boundary safety above says nothing about TOKEN boundaries — so
/// [`back_off_from_split_escape`] nudges the cut point left, out of any
/// token it would otherwise bisect, before truncating. A clamped title that
/// happens to contain that literal shape without having gone through
/// `display_peer` at all only ever loses a few extra characters as a
/// harmless side effect; it never gains any.
pub(crate) fn clamp_title(title: &str) -> String {
    const MAX_CHARS: usize = 64;
    let mut clamped: String = title.chars().take(MAX_CHARS + 1).collect();
    if clamped.chars().count() > MAX_CHARS {
        let cut = clamped
            .char_indices()
            .nth(MAX_CHARS)
            .map_or(clamped.len(), |(i, _)| i);
        clamped.truncate(back_off_from_split_escape(&clamped, cut));
        clamped.push('…');
    }
    clamped
}

/// Moves a candidate truncation point left, out of a `display_peer`
/// `<U+XXXX>` escape token, if it would otherwise fall strictly inside one.
///
/// A token cut in half — `<U+20`, say, with no closing `>` — is not a
/// shorter escape, it is meaningless literal text: the reader can no longer
/// tell it apart from four random characters the peer actually sent, which
/// is the opposite of what the token exists to communicate. An escape token
/// is always exactly eight ASCII bytes (`<`, `U`, `+`, four hex digits,
/// `>`), so the search window this needs is bounded regardless of how long
/// `s` is, and — being pure ASCII — every byte in it is also a valid char
/// boundary, so slicing on the offsets found here can never panic.
///
/// Walks CHARACTERS rather than raw bytes for that search: a multi-byte
/// character elsewhere in `s` (an ordinary non-ASCII name, say) has no byte
/// that equals the ASCII `<`, but arithmetic on raw byte offsets could still
/// land `window_start` inside one, which `str` indexing then panics on.
/// `char_indices` never produces a non-boundary offset in the first place.
fn back_off_from_split_escape(s: &str, cut: usize) -> usize {
    /// `<U+XXXX>`: three literal characters, four hex digits, one literal
    /// character.
    const TOKEN_LEN: usize = 8;
    let mut before: Vec<(usize, char)> =
        s[..cut].char_indices().rev().take(TOKEN_LEN - 1).collect();
    before.reverse();
    let Some(&(open, _)) = before.iter().find(|(_, ch)| *ch == '<') else {
        // No `<` in the trailing window at all, so `cut` cannot be inside a
        // token — either there is no token nearby, or a complete one
        // already ended before `cut`.
        return cut;
    };
    if s[open..cut].contains('>') {
        // The token starting at `open` already closed before `cut`; the cut
        // point falls after it, not inside it.
        cut
    } else {
        // `cut` is between this token's `<` and its (not yet seen) `>`:
        // back off to exclude the whole partial token instead of keeping a
        // broken prefix of it.
        open
    }
}

// ===== Keyboard navigation ===========================================
//
// The row menu is a real `role="menu"`, and a menu that can only be
// driven by mouse is a menu in name only. The decisions below are pure so
// they can be pinned headlessly: a `VirtualDom` with no renderer attached
// dispatches no key events and cannot move focus, so everything testable
// about this is the MAPPING — key to intent, intent to the item that
// should receive focus, and action to whether the browser's own default
// survives. The rest — reading the pressed key off a real event, the
// roving `tabindex` that makes the whole menu one tab stop, and calling
// `MountedData::set_focus` on the chosen item — stays with each row and
// is covered by the browser suite.

/// Which way along the item list a key moves focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuFocusMove {
    /// Toward the end of the list (ArrowDown), wrapping back to the first.
    Next,
    /// Toward the start of the list (ArrowUp), wrapping back to the last.
    Previous,
}

/// What a key pressed on a row menu's toggle or on one of its items
/// means, for the keys the menu claims and only those.
///
/// Deliberately NOT a total mapping: anything not listed returns `None`
/// from [`menu_key_action`] and is left entirely alone — no
/// `prevent_default`, no handling — because this menu shares its keyboard
/// with the browser's own (Enter/Space activation on the native buttons
/// underneath) and with whatever the user's assistive technology sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuKeyAction {
    /// Escape: close the menu. Focus returns to the toggle through the
    /// row's own close teardown (its dismissal effect), which is the same
    /// path an automatic dismissal takes, so the keyboard user is
    /// returned where they started rather than dropped at the top of the
    /// document.
    Close,
    /// Tab or Shift+Tab pressed ON AN ITEM: dismiss the menu and hand
    /// focus back to the toggle, which is the tab stop the whole menu
    /// stands in for (each row's own roving `tabindex` is what makes it
    /// one). From there the user's next Tab continues out of the row
    /// natively, because a closed toggle claims neither key.
    ///
    /// A separate variant from [`MenuKeyAction::Close`] because it means
    /// something different on the TOGGLE, where it is not the menu's key
    /// to take at all: Tab there is how the browser walks INTO the open
    /// menu, and `handle_menu_key` leaves it alone.
    ///
    /// Both of them SUPPRESS the browser's default, which is not the
    /// obvious choice for Tab and is worth the sentence. Letting the
    /// native focus move stand would be ideal — one keystroke out instead
    /// of two — but it does not survive this renderer: the handler's
    /// signal write reaches a microtask checkpoint (and therefore a
    /// re-render, and therefore the panel's unmount) BEFORE the browser
    /// performs the keystroke's default action, so that action would run
    /// against an item that no longer exists and drop focus on the
    /// document body. Suppressing it and choosing the destination
    /// ourselves is what keeps the promise the roles make.
    Exit,
    /// An arrow: step one item from wherever focus is now.
    Step(MenuFocusMove),
    /// Home: the first item, regardless of where focus is now.
    First,
    /// End: the last item, regardless of where focus is now.
    Last,
}

/// Maps a pressed key to the menu's own meaning for it, or `None` for
/// every key the menu does not claim.
pub(crate) fn menu_key_action(key: &Key) -> Option<MenuKeyAction> {
    match key {
        Key::Escape => Some(MenuKeyAction::Close),
        Key::Tab => Some(MenuKeyAction::Exit),
        Key::ArrowDown => Some(MenuKeyAction::Step(MenuFocusMove::Next)),
        Key::ArrowUp => Some(MenuKeyAction::Step(MenuFocusMove::Previous)),
        Key::Home => Some(MenuKeyAction::First),
        Key::End => Some(MenuKeyAction::Last),
        _ => None,
    }
}

/// Where focus should land when the menu is opened, recorded at the
/// moment of opening so the item list can honour it as it mounts.
///
/// A menu button that opens without moving focus into its menu makes
/// every keyboard user press one extra arrow to reach the list they just
/// asked for, which is the behavior the `menu`/`menuitem` roles promise
/// against. The two directions are the ARIA menu-button convention:
/// anything that means "open it" lands on the FIRST command, and ArrowUp
/// — the one gesture that says "the end of the list" before the list
/// exists — lands on the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuOpenIntent {
    /// Focus the first item once the panel mounts.
    First,
    /// Focus the last item once the panel mounts (ArrowUp on a closed
    /// toggle).
    Last,
}

/// Maps a key pressed on a CLOSED toggle to the open it requests, or
/// `None` for every key that must leave a closed toggle alone.
///
/// Only the two arrows are here. Enter and Space are deliberately absent:
/// they already activate a native `<button>`, so the toggle's own
/// `onclick` opens the menu and records [`MenuOpenIntent::First`] for
/// them — claiming them here as well would either double-toggle or
/// require suppressing the activation every native control depends on.
/// Escape is absent for a different reason: on a closed toggle it must
/// not become a second way to OPEN the menu.
pub(crate) fn closed_toggle_key_intent(key: &Key) -> Option<MenuOpenIntent> {
    match key {
        Key::ArrowDown => Some(MenuOpenIntent::First),
        Key::ArrowUp => Some(MenuOpenIntent::Last),
        _ => None,
    }
}

/// Which item index an arrow key moves focus to, or `None` when there is
/// nothing to focus.
///
/// `current` is `None` when focus is not on an item at all — the toggle
/// itself, reachable again by Shift+Tab or by a pointer, where ArrowDown
/// enters the menu at its first item and ArrowUp at its last. It is also
/// what Home and End pass, since "first" and "last" are exactly `Next`
/// and `Previous` measured from outside the list. (An OPEN menu normally
/// starts with focus already on an item — see [`MenuOpenIntent`] — so
/// `None` is the re-entry case rather than the opening one.)
///
/// Wrapping rather than stopping at the ends: both rows' menus are short
/// enough to see whole (four items at most for the session row, five for
/// the host row), so running off the bottom and reappearing at the top
/// costs nothing and saves a direction change. An out-of-range `current`
/// (an index from a render whose item set has since changed) is treated
/// as "not on an item" rather than clamped, so the next arrow re-enters
/// the list at a defined end instead of landing somewhere derived from a
/// stale index.
pub(crate) fn next_menu_focus(
    count: usize,
    current: Option<usize>,
    direction: MenuFocusMove,
) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let current = current.filter(|index| *index < count);
    Some(match (current, direction) {
        (None, MenuFocusMove::Next) => 0,
        (None, MenuFocusMove::Previous) => count - 1,
        (Some(index), MenuFocusMove::Next) => (index + 1) % count,
        (Some(index), MenuFocusMove::Previous) => (index + count - 1) % count,
    })
}

// ===== Reconciling focus when the item set changes without a key press ===
//
// A poll can withdraw a menu item while the menu stays open and nothing the
// user did touched focus at all — the host row's `adoptable` flipping off
// mid-open is the concrete case, and the session row's `archived` toggle can
// do the identical thing. `menu_focus` only ever stores a POSITION, and a
// withdrawal from the middle of the list shifts every later item's index
// down, so the numeric slot a withdrawn action vacates can be, and often is,
// immediately re-occupied by a SURVIVING one. Comparing the stored position
// against the new list's length (as this reconciliation used to) therefore
// answers the wrong question: it only notices a withdrawal that shortens the
// list PAST the focused slot, not one that merely reassigns it. What has to
// survive the comparison is the ACTION focus was on, not the slot number it
// happened to sit in at the time.

/// What a row's menu-item-set effect should do about keyboard focus, given
/// the order the FOCUSED position was recorded against and the order the
/// menu now offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuFocusReconciliation {
    /// Nothing was focused, so there is nothing to reconcile.
    Unchanged,
    /// The focused action is still offered, at this (possibly different)
    /// position — update the stored index; the item's own DOM node and
    /// handle are untouched; only its slot in the list moved.
    Moved(usize),
    /// The focused action is no longer offered at all. The caller has no
    /// item left to aim keyboard focus at and must close the menu, which
    /// hands focus back to the toggle through the row's own dismissal path
    /// — an open panel with no accurate notion of what is focused leaves
    /// arrow keys and Escape unable to reach it (the real bug this
    /// reconciliation replaces: `hosts.rs`'s Adopt disappearing while
    /// Edit slides into its old slot, leaving the row believing Edit was
    /// focused when the browser had already dropped focus off the removed
    /// Adopt button).
    Withdrawn,
}

/// Reconcile one row's recorded focus position against a freshly rebuilt
/// item list, by the ACTION it names rather than by the slot it occupies.
///
/// `previous_order` is the list the caller's `focused_position` was recorded
/// against (before whatever state change just altered which items are
/// visible); `current_order` is the freshly rebuilt list for the same menu.
/// A `focused_position` that does not even resolve to an action under
/// `previous_order` (should not happen in practice, since every write to a
/// row's focus signal goes through a real item) reconciles to `Unchanged`
/// rather than panicking or guessing — a reconciliation this defensive costs
/// nothing and a menu that merely fails to move focus is a far better
/// failure than one that panics.
pub(crate) fn reconcile_menu_focus<A: Copy + Eq, const N: usize>(
    previous_order: MenuOrder<A, N>,
    current_order: MenuOrder<A, N>,
    focused_position: Option<usize>,
) -> MenuFocusReconciliation {
    let Some(position) = focused_position else {
        return MenuFocusReconciliation::Unchanged;
    };
    let Some(action) = previous_order.get(position) else {
        return MenuFocusReconciliation::Unchanged;
    };
    match current_order.position(action) {
        Some(new_position) if new_position == position => MenuFocusReconciliation::Unchanged,
        Some(new_position) => MenuFocusReconciliation::Moved(new_position),
        None => MenuFocusReconciliation::Withdrawn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the geometry contract behind the actions-menu panel's
    /// viewport-fixed positioning (see `menu_panel_style`'s own doc): the
    /// panel's top sits just below the toggle EXCEPT where the top-clamp
    /// (`min()`, floored by the outer `max(8px, ...)`) says otherwise; its
    /// RIGHT edge sits `MENU_PANEL_TOGGLE_GAP_PX` LEFT of the toggle's own
    /// LEFT edge (floored so it cannot cross the viewport's right edge);
    /// and its `max-width` narrows once there is not
    /// `288px + MENU_PANEL_VIEWPORT_MARGIN_PX` of room to the viewport's
    /// left edge, floored at `MENU_PANEL_MIN_WIDTH_PX` rather than allowed
    /// to go negative (an invalid `max-width` CSS silently drops
    /// entirely) and capped by `calc(100vw - 16px)` so a toggle scrolled
    /// past the viewport's right edge cannot claim width that is not
    /// there.
    ///
    /// The right edge is the half worth stating twice, and the fixture
    /// separates the three coordinates a regression could confuse: the
    /// emitted `246` is the toggle's LEFT edge minus the gap, which is
    /// neither its left edge (`250`) nor its right one (`274`). Anchoring
    /// the panel flush under the toggle instead — the tidier, more
    /// conventional placement — puts its destructive items exactly where
    /// the rows below draw their own "⋯", so a click aimed at another
    /// row's menu lands on this row's own destructive item. That is a
    /// wrong-row hazard, not a cosmetic preference: a future change
    /// moving this edge back under the toggle is reintroducing it.
    ///
    /// The exact `max()`/`min()`/`calc()` shape matters, not just the
    /// numbers: a regression that dropped a clamp, swapped `100vw`/`100vh`,
    /// or let `max-width` go unfloored or uncapped would still pass a test
    /// that merely checked the offsets some other way.
    #[test]
    fn menu_panel_style_hangs_below_and_left_of_the_toggle() {
        let toggle_rect = PixelsRect::new(
            dioxus::html::geometry::euclid::point2(250.0, 50.0),
            dioxus::html::geometry::euclid::size2(24.0, 20.0),
        );
        assert_eq!(
            menu_panel_style(toggle_rect),
            "--menu-top: max(8px, min(72px, calc(100vh - 160px))); top: var(--menu-top); \
             right: max(8px, calc(100vw - 246px)); \
             max-width: min(288px, 238px, calc(100vw - 16px)); \
             max-height: calc(100vh - var(--menu-top) - 8px);"
        );
    }

    /// The horizontal `max-width` floor's OWN degenerate case: a toggle
    /// close enough to the viewport's left edge (the `.app-shell`-scrolled
    /// extreme `menu_panel_style`'s doc names) drives the natural
    /// `right_edge - MENU_PANEL_VIEWPORT_MARGIN_PX` computation negative,
    /// which is exactly what `MENU_PANEL_MIN_WIDTH_PX` exists to floor
    /// instead of emitting — an unfloored negative `max-width` is invalid
    /// CSS that silently drops the whole declaration, reverting to the
    /// class's unclamped `288px` and defeating the very clamp meant to
    /// prevent left-edge overflow in this exact scenario.
    #[test]
    fn menu_panel_style_floors_max_width_instead_of_going_negative() {
        // right_edge = min_x - 4 = -3; (right_edge - 8) = -11, which the
        // floor must replace with `MENU_PANEL_MIN_WIDTH_PX` (96) rather
        // than emit as-is.
        let toggle_rect = PixelsRect::new(
            dioxus::html::geometry::euclid::point2(1.0, 50.0),
            dioxus::html::geometry::euclid::size2(4.0, 20.0),
        );
        assert!(
            menu_panel_style(toggle_rect)
                .contains("max-width: min(288px, 96px, calc(100vw - 16px));")
        );
    }

    /// The placement→style mapping is the whole reason `PanelPlacement`
    /// exists as three states rather than a plain `Option<PixelsRect>`
    /// (see its own doc): each variant must render as its OWN distinct
    /// CSS, not just "measured or not" — `Unmeasured` hides the panel
    /// without a wrong-position flash, `Fallback` pins its own top-left
    /// sidebar coordinates inline instead of hiding forever, and
    /// `Measured` carries the real computed geometry
    /// (already pinned above). A regression that collapsed any two of
    /// these into the same style would reintroduce either the flash or
    /// the dead-toggle failure mode the state machine exists to prevent.
    ///
    /// Also pins the fixes for two real bugs this exact scheme hit:
    /// `Unmeasured` hides via `opacity`/`pointer-events`, NOT `visibility`
    /// (see `menu_panel_placement_style`'s own doc for why
    /// `visibility: hidden` would silently break `autofocus` on a reopened
    /// confirm/rename panel); and every positional property one state sets
    /// is explicitly restated (or `auto`-reset) by the others — Dioxus's
    /// JS interpreter restores an omitted property from the PREVIOUS
    /// inline style rather than treating a `style` update as a plain
    /// replace, so `Fallback`'s `left: 8px` surviving into a later
    /// `Measured` render would pin BOTH insets and stretch the panel
    /// across the sidebar (the class-level-fallback regression this
    /// mapping's per-state coordinates exist to prevent).
    #[test]
    fn menu_panel_placement_style_renders_each_variant_distinctly() {
        assert_eq!(
            menu_panel_placement_style(PanelPlacement::Unmeasured),
            "opacity: 0; pointer-events: none;",
        );
        assert_eq!(
            menu_panel_placement_style(PanelPlacement::Fallback),
            "opacity: 1; pointer-events: auto; top: 8px; left: 8px; right: auto; \
             max-width: 288px; max-height: calc(100vh - 16px);",
        );
        let toggle_rect = PixelsRect::new(
            dioxus::html::geometry::euclid::point2(100.0, 50.0),
            dioxus::html::geometry::euclid::size2(30.0, 20.0),
        );
        assert_eq!(
            menu_panel_placement_style(PanelPlacement::Measured(toggle_rect)),
            format!(
                "opacity: 1; pointer-events: auto; left: auto; {}",
                menu_panel_style(toggle_rect)
            ),
        );
    }

    /// `max-width`'s midband and cap, complementing the floor test above:
    /// with ordinary room to the toggle's left the clamp must pass the
    /// NATURAL available width through (not collapse everything to the
    /// 96px floor), and with abundant room the 288px cap must win. An
    /// implementation that always emitted the floor — or always the cap —
    /// would pass the anchor and floor tests alone.
    ///
    /// Both cases keep the `calc(100vw - 16px)` term, which is the
    /// mirror-image guard the arithmetic here CANNOT express: this
    /// function never learns the viewport's width, so the one case where
    /// the raw `right_edge` overstates the room actually available — a
    /// toggle at or past the viewport's right edge in a narrow,
    /// horizontally scrolled shell, where the emitted `right` is floored
    /// to `8px` and the panel is painted somewhere the arithmetic did not
    /// predict — is left to the engine to resolve at paint time.
    #[test]
    fn menu_panel_style_max_width_tracks_available_room_up_to_the_cap() {
        // right_edge = min_x - 4 = 230; available = 230 - 8 = 222,
        // between floor and cap, so it must appear verbatim.
        let midband = PixelsRect::new(
            dioxus::html::geometry::euclid::point2(234.0, 50.0),
            dioxus::html::geometry::euclid::size2(30.0, 20.0),
        );
        assert!(
            menu_panel_style(midband).contains("max-width: min(288px, 222px, calc(100vw - 16px));")
        );

        // right_edge = min_x - 4 = 530; available = 522, above the cap —
        // `min()` hands the win to 288px at paint time.
        let roomy = PixelsRect::new(
            dioxus::html::geometry::euclid::point2(534.0, 50.0),
            dioxus::html::geometry::euclid::size2(30.0, 20.0),
        );
        assert!(
            menu_panel_style(roomy).contains("max-width: min(288px, 522px, calc(100vw - 16px));")
        );
    }

    /// Pins the remount-heal GATE (see `should_measure_on_mount`'s own doc
    /// for the failed-then-recovered-listing scenario this exists for): a
    /// measurement starts on mount ONLY when the menu is already open AND
    /// nothing has measured it yet. This is deliberately narrower than
    /// "the menu is open" alone — a row that remounts already `Measured`
    /// or `Fallback` (not the scenario this heals, but worth pinning
    /// regardless) must not restart a measurement it does not need.
    ///
    /// This only pins the DECISION, not the remount-and-heal BEHAVIOR
    /// end to end: `onmounted` firing at all requires a real renderer to
    /// dispatch a mount event and supply a real `MountedData` handle for
    /// `get_client_rect()` to call — a headless `VirtualDom` test (no
    /// renderer attached, `NoOpMutations`) never fires `onmounted` in the
    /// first place, so the ASYNC "measure and heal" half of this fix is
    /// not something this test harness can exercise; that is what the
    /// browser suite is for.
    #[test]
    fn should_measure_on_mount_only_when_open_and_unmeasured() {
        assert!(should_measure_on_mount(true, PanelPlacement::Unmeasured));
        assert!(!should_measure_on_mount(false, PanelPlacement::Unmeasured));
        let toggle_rect = PixelsRect::new(
            dioxus::html::geometry::euclid::point2(0.0, 0.0),
            dioxus::html::geometry::euclid::size2(10.0, 10.0),
        );
        assert!(!should_measure_on_mount(
            true,
            PanelPlacement::Measured(toggle_rect)
        ));
        assert!(!should_measure_on_mount(true, PanelPlacement::Fallback));
    }

    /// Pins `spawn_measurement`'s own race directly: a measurement that
    /// resolves for a SUPERSEDED open (`generation_now` has moved past
    /// `generation_at_capture` — a fast close-then-reopen of the same
    /// toggle, or a reopen after the row itself moved) must be discarded
    /// rather than overwriting whatever the newer open's own measurement
    /// already wrote or will still write. Every current-generation outcome
    /// is pinned too — a resolved rect becomes `Measured`, and a renderer
    /// that could not answer `get_client_rect()` at all (no mounted handle
    /// yet, or the query itself failing) becomes `Fallback` rather than
    /// leaving the panel invisible forever.
    ///
    /// This is the ONE piece of `spawn_measurement`'s async body that CAN
    /// be pinned outside a real renderer: the `await` on
    /// `MountedData::get_client_rect()` itself has no meaning without one
    /// (see `should_measure_on_mount`'s own doc for why that half stays
    /// with the browser suite). An injectable geometry-reader seam would
    /// let a test drive that `await` too, but the decision actually worth
    /// pinning — apply-if-current — is exactly what this function already
    /// isolates, at no extra production cost: no new trait, no new type
    /// threaded through either row's state, just this closure's own two
    /// lines given a name and a return value instead of a direct write.
    #[test]
    fn measurement_outcome_discards_a_superseded_measurement() {
        let toggle_rect = PixelsRect::new(
            dioxus::html::geometry::euclid::point2(0.0, 0.0),
            dioxus::html::geometry::euclid::size2(10.0, 10.0),
        );
        // Still current: both outcomes of the measurement itself apply.
        assert_eq!(
            measurement_outcome(1, 1, Some(toggle_rect)),
            Some(PanelPlacement::Measured(toggle_rect)),
        );
        assert_eq!(
            measurement_outcome(1, 1, None),
            Some(PanelPlacement::Fallback),
        );
        // Superseded: a newer open has since bumped the generation, so
        // this result — whichever it was — must be discarded outright
        // rather than painted over the newer open's own placement.
        assert_eq!(measurement_outcome(1, 2, Some(toggle_rect)), None);
        assert_eq!(measurement_outcome(1, 2, None), None);
    }

    /// The menu's key map, and — for a SAMPLE of native keys it must not
    /// disturb — that they stay unclaimed.
    ///
    /// The negative half is the load-bearing one, and its claim is
    /// deliberately narrow: this pins the sampled keys below, not "no
    /// other key anywhere is ever claimed". A closed, assertable mapping
    /// would need `Key` itself to be enumerable, which it is not (it
    /// carries an open-ended `Character(String)` arm), so the honest
    /// contract is a sample chosen for consequence rather than for
    /// coverage. Enter and Space are the two that matter most: they are
    /// how the native `<button>`s underneath these items get activated at
    /// all, so a mapping that grew an entry for either would make the
    /// menu's own commands unreachable from the keyboard.
    #[test]
    fn the_menu_claims_its_navigation_keys_and_leaves_activation_native() {
        assert_eq!(menu_key_action(&Key::Escape), Some(MenuKeyAction::Close));
        assert_eq!(menu_key_action(&Key::Tab), Some(MenuKeyAction::Exit));
        assert_eq!(
            menu_key_action(&Key::ArrowDown),
            Some(MenuKeyAction::Step(MenuFocusMove::Next))
        );
        assert_eq!(
            menu_key_action(&Key::ArrowUp),
            Some(MenuKeyAction::Step(MenuFocusMove::Previous))
        );
        assert_eq!(menu_key_action(&Key::Home), Some(MenuKeyAction::First));
        assert_eq!(menu_key_action(&Key::End), Some(MenuKeyAction::Last));

        for unclaimed in [
            Key::Enter,
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::PageUp,
            Key::PageDown,
            Key::Character(" ".to_string()),
            Key::Character("a".to_string()),
        ] {
            assert_eq!(
                menu_key_action(&unclaimed),
                None,
                "{unclaimed:?} belongs to the browser, not to this menu"
            );
        }
    }

    /// A CLOSED toggle answers the two arrows by opening at the matching
    /// END of the list, and answers nothing else.
    ///
    /// This is the half of the menu-button pattern that only exists on
    /// the closed state: ArrowDown opens onto the first command and
    /// ArrowUp onto the last, so a keyboard user never has to open and
    /// then aim separately. Escape's absence is the deliberate one — a
    /// closed toggle must not treat it as a second way to open — and
    /// Enter/Space are absent because the native button activation
    /// already routes them through the toggle's `onclick`, which records
    /// the same `First` intent.
    #[test]
    fn a_closed_toggle_opens_at_the_end_the_arrow_names() {
        assert_eq!(
            closed_toggle_key_intent(&Key::ArrowDown),
            Some(MenuOpenIntent::First)
        );
        assert_eq!(
            closed_toggle_key_intent(&Key::ArrowUp),
            Some(MenuOpenIntent::Last)
        );
        for ignored in [
            Key::Escape,
            Key::Enter,
            Key::Tab,
            Key::Home,
            Key::End,
            Key::Character(" ".to_string()),
        ] {
            assert_eq!(
                closed_toggle_key_intent(&ignored),
                None,
                "{ignored:?} must not open a closed actions menu"
            );
        }
    }

    /// Arrow navigation wraps at both ends and enters the list from
    /// outside it, which is what makes the menu drivable without a mouse.
    ///
    /// `None` for `current` is focus sitting on the toggle rather than on
    /// an item — where Shift+Tab and a pointer click both leave it — so
    /// the two `None` cases are the RE-ENTRY behavior: ArrowDown must
    /// land on the first item and ArrowUp on the last, or a keyboard user
    /// who stepped back out to the toggle has no way in again. The wrap cases pin the
    /// deliberate choice of wrapping over stopping (see
    /// `next_menu_focus`), and the out-of-range case pins that a stale
    /// index re-enters at a defined end rather than resolving to whatever
    /// modular arithmetic on it would produce.
    #[test]
    fn arrow_navigation_wraps_and_enters_from_the_toggle() {
        use MenuFocusMove::{Next, Previous};

        // Entering from the toggle, where focus sits after Shift+Tab.
        assert_eq!(next_menu_focus(4, None, Next), Some(0));
        assert_eq!(next_menu_focus(4, None, Previous), Some(3));

        // Stepping within the list.
        assert_eq!(next_menu_focus(4, Some(0), Next), Some(1));
        assert_eq!(next_menu_focus(4, Some(2), Previous), Some(1));

        // Wrapping at both ends.
        assert_eq!(next_menu_focus(4, Some(3), Next), Some(0));
        assert_eq!(next_menu_focus(4, Some(0), Previous), Some(3));

        // An archived row's shorter list (rename + delete only) wraps on
        // its own length, not on some assumed four.
        assert_eq!(next_menu_focus(2, Some(1), Next), Some(0));

        // A stale index from a render whose item set has since shrunk is
        // treated as "not on an item", so the next arrow re-enters at an
        // end rather than at `(9 + 1) % 4`.
        assert_eq!(next_menu_focus(4, Some(9), Next), Some(0));
        assert_eq!(next_menu_focus(4, Some(9), Previous), Some(3));

        // Nothing to focus: no panel item exists, so no key can move
        // focus into one. Neither row's `MenuOrder` produces this today —
        // each caller currently keeps at least one item unconditional in
        // every state (`profiles`/`retry` for the host row, `rename`/
        // `delete` for the session row) — which is exactly why it is
        // pinned — a future control set that could empty a menu must not
        // make this arithmetic underflow.
        assert_eq!(next_menu_focus(0, None, Next), None);
        assert_eq!(next_menu_focus(0, Some(0), Previous), None);
    }

    /// [`MenuOrder::pack`] packs the visible subset of a fixed action set
    /// to the front, in the set's own declared order, with no gaps and no
    /// memory of the positions withdrawn actions used to hold — the
    /// generic mechanics both rows' `*_menu_order` constructors are built
    /// on. `list::row`'s own
    /// `menu_order_follows_the_retention_state_rather_than_a_fixed_numbering`
    /// and `hosts`'s host-menu-order test each pin their OWN action enum
    /// and visibility rule against this; this test pins the packing
    /// itself with a small enum local to the test, independent of either
    /// row's real one.
    #[test]
    fn menu_order_pack_keeps_declared_order_with_no_gaps() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        enum Toy {
            A,
            B,
            C,
        }

        let all = MenuOrder::pack([Toy::A, Toy::B, Toy::C], |_| true);
        assert_eq!(all.len(), 3);
        assert_eq!(all.get(0), Some(Toy::A));
        assert_eq!(all.get(1), Some(Toy::B));
        assert_eq!(all.get(2), Some(Toy::C));
        assert_eq!(all.last(), Some(Toy::C));

        // B withdrawn: C shifts up to fill the gap rather than leaving one
        // at position 1, and B's position is gone entirely rather than
        // some sentinel.
        let without_b = MenuOrder::pack([Toy::A, Toy::B, Toy::C], |action| action != Toy::B);
        assert_eq!(without_b.len(), 2);
        assert_eq!(without_b.get(0), Some(Toy::A));
        assert_eq!(without_b.get(1), Some(Toy::C));
        assert_eq!(without_b.position(Toy::C), Some(1));
        assert_eq!(without_b.position(Toy::B), None);

        // Everything withdrawn: an empty list, not a panic.
        let none = MenuOrder::pack([Toy::A, Toy::B, Toy::C], |_| false);
        assert_eq!(none.len(), 0);
        assert_eq!(none.get(0), None);
        assert_eq!(none.last(), None);
    }

    /// Accessible names built from a row's own title/destination clamp it,
    /// and say so with an ellipsis — shared by both rows' menu labels
    /// (`list::row::menu_label`, `hosts::host_menu_label`) and pinned here
    /// once rather than per caller.
    #[test]
    fn clamp_title_cuts_long_values_with_an_ellipsis() {
        assert_eq!(clamp_title("short"), "short");
        let long = "x".repeat(200);
        let clamped = clamp_title(&long);
        assert_eq!(
            clamped.chars().count(),
            65,
            "64 characters plus the ellipsis"
        );
        assert!(clamped.ends_with('…'));
        // Char-based, not byte-based: a multi-byte title must never be
        // cut mid-codepoint (which would not even be a `String`).
        let multibyte = "é".repeat(200);
        assert_eq!(clamp_title(&multibyte).chars().count(), 65);
    }

    /// A clamp applied to `display_peer`'s output must never bisect one of
    /// its `<U+XXXX>` escape tokens — a truncated token such as `<U+206`
    /// with no closing `>` is not a shorter escape, it is eight characters
    /// of meaningless literal text indistinguishable from something the
    /// peer actually sent (`hosts::host_menu_label` is the real caller this
    /// protects: an ssh destination escaped by `display_peer` before being
    /// clamped for a menu's `aria-label`).
    ///
    /// The fixture is built so the naive char-count cut point (64) lands
    /// inside the eighth token's `<U+2066` prefix, one character before its
    /// closing `>` — deliberately not aligned to any token boundary, so a
    /// regression back to plain `char_indices().nth(64)` truncation would
    /// fail this exact case while still passing the plain-ASCII and
    /// multibyte cases above.
    #[test]
    fn clamp_title_never_bisects_an_escape_token() {
        let token = "<U+2066>";
        let long = format!("AB{}", token.repeat(9));
        let clamped = clamp_title(&long);
        assert_eq!(
            clamped,
            format!("AB{}…", token.repeat(7)),
            "the eighth (partial) token is dropped whole rather than cut in half: {clamped:?}"
        );
    }

    /// The real bug this reconciliation replaces: an action withdrawn from
    /// the MIDDLE of a menu (host.rs's Adopt, once an identity mismatch
    /// resolves) shifts every later action's slot down, so the numeric
    /// position focus was recorded at gets reoccupied by a SURVIVING
    /// action rather than becoming out of range. A length-only check
    /// (this reconciliation's predecessor: is the stored position still
    /// less than the new list's length?) sees nothing wrong in that case
    /// and leaves the row believing the wrong action is focused; comparing
    /// by the action itself is what catches it.
    #[test]
    fn reconcile_menu_focus_tracks_the_action_not_the_slot() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Toy {
            Adopt,
            Edit,
            Remove,
        }

        // Before: [Adopt, Edit, Remove], focus on Adopt at position 0.
        let before = MenuOrder::pack([Toy::Adopt, Toy::Edit, Toy::Remove], |_| true);
        // After: Adopt withdrawn: Edit slides into position 0, the exact
        // slot the stale focus still names — a length check (0 < 2) would
        // wrongly call this unchanged.
        let after = MenuOrder::pack([Toy::Adopt, Toy::Edit, Toy::Remove], |a| a != Toy::Adopt);
        assert_eq!(
            reconcile_menu_focus(before, after, Some(0)),
            MenuFocusReconciliation::Withdrawn,
            "position 0 named Adopt, and Adopt is gone — a surviving Edit now sitting at the \
             same slot must not be mistaken for it"
        );

        // A surviving action that merely MOVED (Remove: position 2 -> 1)
        // is reported as `Moved`, not `Withdrawn` or `Unchanged`.
        assert_eq!(
            reconcile_menu_focus(before, after, Some(2)),
            MenuFocusReconciliation::Moved(1)
        );

        // A surviving action that did not move at all is `Unchanged`.
        let stable = MenuOrder::pack([Toy::Adopt, Toy::Edit, Toy::Remove], |a| a != Toy::Remove);
        assert_eq!(
            reconcile_menu_focus(before, stable, Some(0)),
            MenuFocusReconciliation::Unchanged
        );

        // Nothing focused: nothing to reconcile.
        assert_eq!(
            reconcile_menu_focus(before, after, None),
            MenuFocusReconciliation::Unchanged
        );
    }
}
