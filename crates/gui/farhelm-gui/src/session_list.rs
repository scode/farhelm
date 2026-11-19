//! The session list draws the helm's sessions as global pins, named groups and an ungrouped tail. The open session
//! stays selected even when its group is collapsed.
//!
//! SPEC.md, "Session list". The store supplies the ordered sessions and the group roster. The sidebar partitions them
//! before applying the pointer's order hold ([`groups`], [`pins`], [`hold`]). Collapse comes after the hold, so an
//! expand reveals the same held slots. Above the rows, a
//! heading band carries the order's title, which opens the menu that changes the order ([`order`]), the count
//! ("Sessions: N" to assistive technology and on hover, as the old count said) and, at its right, the Trash button (a
//! view of the trash's own, [`crate::trash`]), the Templates button and New; a listing the helm's cap cut says so in an
//! amber band in place of the count, and a failed read says so in red, while the rows it had stay (a failed read is no
//! evidence that anything was deleted).
//!
//! The list also owns the selection rules ([`selection`]), and applies them from observers, never from drawing: it
//! settles the selection, the open session's seen mark and the open session its query names ([`SessionList::settle`],
//! [`filters`]' "The query names the open session") whenever the connection changes (a new UI session after a reset,
//! or none while the window is inert), the selection changes, the current UI session's store applies a pushed change,
//! its operation store starts or settles a request, or the helm answers the list's query (whose naming of the open
//! session is evidence the selection rules read, [`filters::Choice::serves_named`]). GPUI draws nothing for a window
//! that is hidden, minimized or behind other windows, and SPEC.md, "Status", wants a session open in a window behind
//! other apps recorded as seen the same way as one in a focused window; work driven by frames would wait until the
//! window showed again. Each settle recomputes from the state as it is then. A settle that changes the selection
//! notifies the selection's observers, the list among them, and that second settle finds nothing to do.
//!
//! The list follows the connection and the current store through [`session::observe_current`], which moves its store
//! subscription to each new UI session's store, so the list never observes a store a reset retired and never misses
//! the first change of the one that replaced it. What the list keeps for one UI session (the seen mark's memory, the
//! row menus and the layers they opened, the list's query, whether the Hosts panel shows, and the subscription to
//! the operation store, which `observe_current` does not follow) lives in a [`PerSession`], which starts it over when
//! the session changes.
//!
//! The rows are virtualized: they are drawn by GPUI's variable-height `list`, which builds only the rows in view and a
//! few beyond its edges, so a redraw costs the rows on screen, not the fleet. The list is drawn uncached (the
//! foundations decision of 2026-11-14), and GPUI redraws every uncached view of a window whenever anything in it
//! animates (the running pulse, the terminal's cursor blink). Measured on the test Mac on 2026-11-14 with every row
//! built on every frame, 200 sessions idled at about 12% of a core and 500 at about 20%, against about 2% for five, and
//! typing latency grew from about 31 ms to 50 and 64; the migration's orchestrator then made virtualizing the list the
//! first sidebar change. Rows differ in height (an ended row's status line wraps, a refusal adds a line), so this is
//! `list` with a [`ListState`], not `uniform_list`, which measures only its first row.
//!
//! NOTE: SPEC.md says the list is "served and rendered WHOLE". That rules out paging, cursors and incremental
//! listing: the helm still serves the whole capped fleet, the store holds all of it, and every row is in the list,
//! reachable by scrolling. Drawing only the rows in view is not a partial list, so it is no breach. What it does change:
//! the accessibility tree and the test bridge see only the rows drawn, so a test reading rows from a frame scrolls them
//! into view or uses a fleet that fits.
//!
//! The list state is kept in step with the store at the top of every frame ([`SessionList::sync_rows`]): rebuilt
//! whenever the listed sessions or their order change, keeping the scroll position where it was (as the old page kept
//! its scroll offset), with every row not yet drawn counted at an estimated height so the list knows how far it can
//! scroll. Rows in view are laid out afresh on every frame, so a pushed change that changes a row's height shows at
//! once.
//!
//! Keyboard focus needs two things the virtualization would otherwise take away:
//!
//! - **A focused row stays drawn.** Each row's item carries a focus handle the list owns, one that never takes focus
//!   itself but contains whichever of the row's controls holds it, and every row's handle is registered with the list
//!   state, which keeps drawing the row that contains focus when it scrolls out of view (the cookbook's trap: a
//!   control's handle kept in its element state ends with the row's last frame, and focus with it).
//! - **Tab walks the whole list.** GPUI's Tab order holds only what was painted, so when the keyboard moves focus into
//!   a row the list scrolls its neighbors into view ([`SessionList::reveal_focused`]); the next Tab or Shift+Tab then
//!   finds the next row painted. SPEC.md, "Reaching everything by keyboard". Only the keyboard: a click that focuses a
//!   row's control must not scroll the list under the pointer, or the release lands elsewhere and the click is lost.
//!   A Space or Enter on a row scrolled out of view (its menu button still focused after a wheel scroll) scrolls the
//!   row back before it acts; a Tab from there leaves the list (the parity file records it).
//!
//! The running pulse is the one thing that redraws the list on its own, and its cost is bounded by the list, not by its
//! rows. The list holds one [`ticker::Pulsing`] handle while the window is active and any listed row pulses (running,
//! on a connected host), and none otherwise. Every running mark reads the same shared step from it, so the window
//! redraws at most once per step (four times a second) whatever the number of running rows, and not at all while the
//! window is inactive, under reduced motion, or with nothing running. Without a handle a running mark draws at full
//! opacity, which is how an inactive window shows a still dot. The dot is deliberately not a view of its own: GPUI
//! redraws a notified view's whole window, re-rendering every uncached view in it and dirtying the notified view's
//! ancestors, so per-row views would not have limited what a step redraws, only multiplied the notifications.
//!
//! The rows' ages ([`age`]) are the other thing that changes with time alone, and they wake the list twice a minute,
//! not once per row: the list holds the ticker's one [`ticker::Clock`] while any listed row has an age to show, reads
//! the wall clock once per frame, and every row's age is counted from that one reading. The clock notifies in an
//! inactive window and under reduced motion too (an age is information, not motion). The list is drawn uncached, which
//! a view that draws an age must be, since the cache wrapper cannot see the time.
//!
//! Each row has an actions menu ([`menu`]) whose commands so far are Rename ([`rename`]), Mark read or Mark unread
//! ([`seen`], also on a click on the row's status dot), Pin to top or Unpin and Move up and Move down ([`pins`]; a
//! movable pin's grip drags it to the same effect, [`pin_drag`]), Replace ([`replace`]), Stop ([`stop`]) and Delete
//! ([`delete`]),
//! and the open session's header (`session_pane/header.rs`) asks the same Stop, Replace and Delete through the same
//! functions, plus Restart ([`restart`]), which the old row menu never had; the Rename shortcut reaches Rename for the
//! open session ([`rename::open_for_selected`]), and the Delete shortcut the header's Delete. A Replace that the
//! source's host refuses for YOLO asks the launcher's question where it was asked ([`replace_yolo`]). What a row says
//! about its operations beyond "in flight" (a refusal, a success's notice, how long a Replace has waited) is
//! [`outcome`]'s. While any Delete is in flight, the selection rules hold back their automatic choice, so a list that
//! loses its open session to that Delete opens the next one only once every Delete has settled; a Stop or a Replace
//! holds nothing back, with one exception for the open session's own Replace ([`selection`], "A Replace keeps its own
//! source open until it answers"). Every pending Delete's, Stop's and long Replace's spinner turns from one handle the
//! list holds, the way the pulse does ([`Spinner`]), and so does an environment session's Suspending… or Resuming…
//! ([`environment`], whose Suspend, Resume and Stop and suspend the row's menu and the header offer). A row's menu
//! button and its anchor exist only while the row is drawn, so a menu whose row scrolls well out of view closes, and a
//! question about a row out of view is asked centered.
//!
//! A row's hover help is its summary card ([`summary`]), a view the row hands the layer manager, which draws it beside
//! the sidebar while it shows; every mark in the row places its own help there too ([`row_marks::HelpPlace`]).
//!
//! The Rename editor the list owns is a dialog rather than part of a row, so a listing that changes under it never
//! moves its field.
//!
//! The quick switcher ([`quick_switcher`]) belongs to the list too: a dialog, opened with its shortcut, that searches
//! a listing of its own and opens the session the user picks through the same selection rules as a click on a row.
//!
//! The host rail ([`host_rail`]) is drawn by the list, at the sidebar's left edge with the list's own column beside
//! it, because choosing a host is the list's query, and so is choosing a status in the segments under the heading
//! ([`filters`]): both set the UI channel's session query, the list's state keeps the choice per UI session, and the
//! list's column shows the segments with the chosen host's chip beside them and, for a filtered answer with no
//! sessions, a sentence that names its filters. The heading and the segments are drawn from the first frame on, also
//! while the first listing has not arrived, as the old app kept them usable before its first read. The rail's foot,
//! which the list draws after its column, holds Hosts, which swaps that column for the Hosts panel ([`crate::hosts`]),
//! the Settings gear, and the Expand control, which widens the whole rail to show its hosts' names and the foot's
//! labels; the sidebar widens by as much, so the list's column keeps its width ([`host_rail`], "Expansion").
//!
//! Above the heading, while Hosts does not show, the list's column draws the Hosts attention list ([`attention`]): one
//! row per host with a problem, work going on or an update, whose Details opens Hosts at that host. Its rows scroll on
//! their own, so the sessions stay reachable however many hosts need attention, and a change of what it draws closes
//! the rows' menus and the marked menu, since it moves them.
//!
//! Under the segments, the list's column draws the Delete notice band ([`delete_notice`]) while a Delete or a Replace
//! this window sent has left a notice: the helm's words about a checkout it could not archive, say, or the count of
//! sessions an environment kept after an environment session's Delete ([`environment_delete`]), with Dismiss. It
//! belongs to the UI session, so a reset takes it away, and it is drawn with the rest of the column, so the Hosts panel
//! hides it.
//!
//! The Templates button opens the Templates dialog ([`crate::templates::open`]), which returns focus to what held it
//! when it opened, as SPEC.md's "Opening and closing" asks. New, after it, opens the launcher through its entry point
//! ([`open_new`], which the quick switcher's "New session named X" row and the New shortcut share): the sidebar's one
//! filled control, drawn disabled while New is unavailable (a launch pending, a host change in flight).
//!
//! The order menu also holds Compact, the helm-wide density preference: with it on, every row is one line
//! ([`row`]'s "Compact rows"), read once per frame like the steps of the animations. The order, Compact and the
//! rail's expansion are written the same way ([`queued_preference`]): a choice made while another preference save is
//! in flight is shown at once and sent once that save settles, never dropped.
//!
//! Shift-click and Cmd-click (Ctrl-click on Linux) on a row mark sessions without opening them ([`marking`]). The marks
//! belong to the UI session, and every frame projects them onto the rows it shows; while any is shown, the heading's
//! band is the marked bar ("N marked", Stop, Delete, the marked menu's ⋯ and Clear) in place of the title, the count
//! and the heading's actions, and Escape anywhere in the list's column clears them. Bulk Stop and Delete, their
//! question and their per-host pacing are [`bulk`]'s; a bulk Delete's queued rows hold the automatic selection back as
//! the Deletes in flight do.
//!
//! A retired containerized session supports Delete only, so its row, its menu and its open view withdraw everything
//! else, and the marked bar's Stop leaves it out ([`retired`]).
//!
//! NOTE: Not ported yet, with the area that brings it: the row menu's other commands. New is not yet where focus rests
//! when there are no sessions (SPEC.md, "Where focus rests"): nothing names it home; that comes with the session pane's
//! status-card work (M2.12), which decides home when no terminal is on screen.

pub(crate) mod age;
mod attention;
pub(crate) mod bulk;
pub(crate) mod delete;
pub(crate) mod delete_notice;
pub(crate) mod environment;
pub(crate) mod environment_delete;
pub(crate) mod filters;
mod groups;
pub(crate) mod hold;
pub(crate) mod host_rail;
pub(crate) mod marking;
pub(crate) mod menu;
mod mute;
pub(crate) mod order;
pub(crate) mod outcome;
pub(crate) mod pin_drag;
pub(crate) mod pins;
pub(crate) mod queued_preference;
pub(crate) mod quick_switcher;
pub(crate) mod rename;
pub(crate) mod replace;
pub(crate) mod replace_yolo;
pub(crate) mod restart;
pub(crate) mod retired;
mod row;
pub(crate) mod row_marks;
pub(crate) mod seen;
pub(crate) mod selection;
pub(crate) mod status;
pub(crate) mod stop;
pub(crate) mod summary;

/// The home-folding rule the rows' last line uses, shared with the details bar's directory field so the two fold a path
/// alike.
pub(crate) use row::abbreviate_home;

#[cfg(test)]
mod bulk_tests;

#[cfg(test)]
mod compact_tests;

#[cfg(test)]
mod delete_notice_tests;

#[cfg(test)]
mod groups_tests;

#[cfg(test)]
mod environment_delete_tests;

#[cfg(test)]
pub(crate) mod environment_tests;

#[cfg(test)]
pub(crate) mod lifecycle_tests;

#[cfg(test)]
mod list_tests;

#[cfg(test)]
mod marking_tests;

#[cfg(test)]
pub(crate) mod menu_tests;

#[cfg(test)]
mod pin_drag_tests;

#[cfg(test)]
mod pins_tests;

#[cfg(test)]
pub(crate) mod row_tests;

#[cfg(test)]
mod row_marks_tests;

#[cfg(test)]
pub(crate) mod retired_tests;

#[cfg(test)]
mod seen_tests;

#[cfg(test)]
mod summary_tests;

#[cfg(test)]
pub(crate) mod tests;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::{Availability, Collection, CollectionMeta, SessionRow};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, ListAlignment, ListState, ParentElement, Pixels, Render, Role, ScrollHandle,
    StatefulInteractiveElement, Styled, Subscription, WeakEntity, WeakFocusHandle, Window, div, px,
};

use crate::actions::Dismiss;
use crate::assets::Icon;
use crate::components::{HoverHelp, IconButton, Spinner, StatusMark, Tier};
use crate::focus::{self, Mark};
use crate::help_menu::HelpButton;
use crate::host_text::{HostSentence, HostText};
use crate::hosts::HostsPanel;
use crate::layers::Anchor;
use crate::local_state::{self, LocalState};
use crate::local_time;
use crate::main_view::MainView;
use crate::operations;
use crate::rail_notices::NoticesButton;
use crate::render_count;
use crate::session::{self, PerSession};
use crate::theme::{self, metrics};
use crate::trash::TrashButton;
use crate::{launcher, templates, ticker};
use selection::{Change, Listed, Open, Stamped};

/// How far past each edge of its view the list draws rows: about two noncompact rows, so a row scrolling in is already
/// laid out and a short scroll draws nothing new. Every pixel of it is rows drawn on every frame, which is the cost the
/// virtualization exists to bound.
const ROW_OVERDRAW: f32 = 100.;

/// The height a row the list has not drawn yet is taken to have: about a noncompact row with its status and folder
/// lines. The list needs an estimate for every row to know how far it can scroll; a row's real height replaces it once
/// the row is drawn, so the estimate only has to be close. Compact rows are about two thirds of it, and keep it all the
/// same: a toggle of the density changes no row's estimate, and the rows drawn after it measure themselves.
const ROW_HEIGHT_HINT: f32 = 50.;

/// The session list view. The main view holds one for the window's life.
///
/// It must be made after the connection ([`session::start`]): it observes the connection it finds when it is made, and
/// without one it never settles anything. The bootstrap starts the connection before it opens the window.
pub struct SessionList {
    /// The state that belongs to the current UI session alone, started over after a reset.
    current: PerSession<Current>,
    /// Settles on every change of the connection and of the current UI session's store. Following the store across
    /// resets is this handle's job, so no store subscription of the list can outlive the session it was made under.
    _following: session::ObservingCurrent,
    /// Settles on a change of the selection, and redraws the list, which draws the selected row.
    _selection: Subscription,
    /// Redraws the list when the host status the rail draws changes outside the store: a setup or Update warning raised
    /// or acknowledged ([`crate::hosts::status::observe_changes`]). The rest of that status is read from the store and
    /// the operation store while drawing, which GPUI already redraws the list for.
    _host_status: Subscription,
    /// Redraws the list when the Delete notice band changes ([`delete_notice::observe`]): Dismiss changes nothing else
    /// the list reads.
    _delete_notice: Subscription,
    /// The pulse's ticks (see the module docs), held exactly while the window is active and some listed row pulses.
    /// Taken and dropped at the top of each frame, before the frame reads its step, as the terminal cursor's blink
    /// is: everything that decides it (the store, the connection, the window's activation) redraws the list when it
    /// changes, so working it out there cannot miss a change.
    pulse: Option<ticker::Pulsing>,
    /// The pending operations' spinner, held exactly while the window is active and some listed row shows one (a
    /// Delete or a Stop in flight, or a Replace that has waited long enough to say so), and worked out at the top of
    /// each frame like `pulse`.
    spinner: Option<Spinner>,
    /// The ticker's 30-second clock (see the module docs), held exactly while some listed row has an age to show, and
    /// worked out at the top of each frame like `pulse`. Unlike the pulse it does not depend on the window being
    /// active: an inactive window still shows its ages.
    clock: Option<ticker::Clock>,
    /// The virtualized rows' state: their count, measured heights, scroll position and focus handles. Kept in step
    /// with the store by [`SessionList::sync_rows`] at the top of every frame; it holds no session data of its own.
    rows: ListState,
    /// The displayed session and group keys, each with its item's focus handle:
    /// how a frame tells whether the rows or their order changed, and what the item callback draws by index. That is
    /// the displayed order (pins first, as the order hold keeps it), which is also what the hold snapshots when the
    /// pointer takes it, not the helm's.
    entries: Rc<[(String, FocusHandle)]>,
    /// Each listed session's row focus handle (see the module docs), owned here so it outlives a frame in which the
    /// row is not drawn and stays the same across rebuilds; dropped once the session is no longer listed.
    row_focus: HashMap<String, FocusHandle>,
    /// The heading's Templates button's focus handle, owned here so it outlives a frame: the Templates dialog names it
    /// as where closing returns focus (on macOS a click leaves the button unfocused, so what held focus before the
    /// click is not where SPEC.md returns it).
    templates_focus: FocusHandle,
    /// The heading's New button's focus handle, owned here so the session pane can name New as where focus rests when
    /// there are no sessions ([`SessionList::new_focus`]).
    new_focus: FocusHandle,
    /// The list's width when its rows were last given their height estimate; see [`SessionList::sync_rows`].
    hinted_width: Option<Pixels>,
    /// What held the window's focus when [`SessionList::reveal_focused`] last looked, so it acts once per focus move.
    ///
    /// NOTE: Weak on purpose. A strong handle here would keep whatever held focus alive after it went away (a terminal
    /// dropped by a reset, say), and the window would go on reporting it as focused, so focus would never go home.
    focus_seen: Option<WeakFocusHandle>,
    /// The host rail's All button's focus handle, a tab stop: kept here rather than in the button's element state so
    /// that removing a filter from the chip or the empty sentence's links can move focus to All ([`filters`]).
    all_focus: FocusHandle,
    /// The Settings gear's focus handle, on the host rail's foot ([`host_rail::foot`]), a tab stop: kept here for the
    /// window's life so that Settings, which the gear opens with itself as the opener, returns focus to the same
    /// control when it closes.
    settings_focus: FocusHandle,
    /// The Hosts control's focus handle, on the rail's foot, a tab stop: kept here so that Hosts keeps focus when it
    /// toggles the panel ([`host_rail`]).
    hosts_focus: FocusHandle,
    /// The `?` Help button on the rail's foot, below the gear: a view of the help menu's own, which keeps its focus
    /// handle and its open menu, created here once for the window's life ([`crate::help_menu`]).
    help: Entity<HelpButton>,
    /// The heading's Trash button: a view of the trash's own, which keeps its focus handle, its reads of every host's
    /// trash and the dialog it opens, created here once for the window's life ([`crate::trash`]).
    trash: Entity<TrashButton>,
    /// The notices bell at the top of the rail's foot, above Hosts: a view of the rail notices' own, which keeps its
    /// focus handle and its open panel, created here once for the window's life ([`crate::rail_notices`]). Its notices'
    /// Retry now and Open Trash go to [`Self::trash`].
    notices: Entity<NoticesButton>,
    /// The order title's focus handle, a tab stop: kept here for the window's life so that the order menu names the
    /// title as where closing returns focus, whichever frame drew it ([`order`]).
    order_focus: FocusHandle,
    /// Where the order title was last drawn, for its menu to open below; recorded by every frame that draws the title.
    /// It refers to the title's element weakly, so a title no longer drawn (the Hosts panel shown) opens nothing.
    order_anchor: Option<Anchor>,
    /// What the host rail keeps across frames (Add host's and the Expand control's focus handles, its scroll position,
    /// its expansion, its avatars' menu anchors): kept here, since the rail is rebuilt on every draw
    /// ([`host_rail::Kept`]).
    rail: host_rail::Kept,
    /// The Hosts attention list's rows' scroll state ([`attention`]), kept for the window's life: a fresh handle per
    /// frame would put the rows back at their top on every draw.
    attention_scroll: ScrollHandle,
    /// The Hosts panel, drawn in place of the list's column while Hosts is pressed. One for the window's life, so the
    /// panel's Details choice survives closing and reopening it (SPEC.md: opening or closing Hosts keeps "the current
    /// ... details choice").
    hosts_panel: Entity<HostsPanel>,
    /// The move neighbors in each displayed pin band and among group headers. Menus read these from the same frame
    /// that draws the list so Move up and Move down name the neighbors the user sees.
    moves: HashMap<String, pins::Neighbors>,
    /// The marked bar's focus handles and anchor, and what the last frame projected of the marks ([`marking::Kept`]):
    /// kept here for the window's life, since the bar is rebuilt on every draw and its menu names its toggle.
    marking: marking::Kept,
    /// Whether the pointer is inside the window, which the order hold asks when a row's menu closes
    /// ([`SessionList::after_menus`]): GPUI's own answer, `Window::is_window_hovered`, in every build. A field rather
    /// than a call so that a test can hand in another reading (`SessionList::pointer_in_window_reads`), since GPUI's test
    /// window never reports the pointer inside it on Linux and offers no way to say it is.
    pointer_in_window: fn(&Window) -> bool,
}

/// What the list keeps for one UI session.
#[derive(Default)]
struct Current {
    /// What the open session's automatic seen mark last acted on: after a reset it starts over, so the open session is
    /// marked again if it is unseen.
    stamped: Stamped,
    /// Settles whenever the session's operation store changes, so the selection held back for a Delete moves on once
    /// the Delete settles. Made by the first settle under the session and dropped with this state when the session
    /// changes, so a retired operation store is never observed. It is here rather than behind
    /// [`session::observe_current`] because that follows the store only.
    operations: Option<Subscription>,
    /// Whether a launch went out while nothing was open and nothing has been selected since, which holds the automatic
    /// pick back ([`selection::hold_for_launch`]). Per UI session, so a reset picks as a freshly loaded window does.
    kept_empty: bool,
    /// The rows' menu buttons and the layers they opened, shared with the item callback that draws the rows.
    rows: menu::SharedRows,
    /// The list's query (the rail's host and the segments' status) and the helm's answers to it. Per UI session
    /// because the helm's channel keeps the query per channel: a new channel starts unfiltered, and so does this.
    filters: filters::Choice,
    /// Whether the Hosts panel shows in place of the list's column. Per UI session like the choice, so a reset brings
    /// the window back to Sessions with the rest of the UI.
    hosts_shown: bool,
    /// The order's write and its menu ([`order::OrderState`]), made the first time the title is drawn under the session.
    order: Option<Entity<order::OrderState>>,
    /// The order hold ([`hold`]): the order on screen while the pointer is over the rows. Per UI session, so a reset
    /// drops it with the listing it held.
    hold: hold::Hold,
    /// The frame's complete layout before collapse, for a hold taken by
    /// the pointer before the next frame.
    last_uncollapsed: Option<hold::Held>,
    /// A pinned row's drag by its grip and the movable rows the last frame drew ([`pin_drag`]). Per UI session, so a
    /// reset forgets a drag with the listing it was over.
    pin_drag: pin_drag::Shared,
    /// Collapse saves are independent for each group and retire with this
    /// UI session's operation store.
    group_collapse: groups::CollapseWrites,
    /// The marked sessions ([`marking`]), shown or hidden. Per UI session, so a reset forgets them, as the old app's
    /// reload did.
    marks: marking::Marks,
    /// The marked menu the bar's ⋯ opened, checked with the layer manager when read, as the order menu's is.
    marked_menu: Option<crate::layers::LayerId>,
    /// The bulk Stop and Delete batches and the open bulk question ([`bulk::Bulk`]). Per UI session, so a reset drops
    /// the queues with the operation store's waiting.
    bulk: bulk::Bulk,
    /// The Hosts attention list's dismissal and the layout it last drew ([`attention::Memory`]). Per UI session, so a
    /// reset shows dismissed reminders again (SPEC.md: "a reset after a lost connection shows optional reminders
    /// again").
    attention: attention::Memory,
}

impl SessionList {
    /// The heading's New button's focus handle. The session pane makes it home when there are no sessions (SPEC.md,
    /// "Where focus rests": "focus rests on New when there are no sessions"), since with no session open there is no
    /// terminal or card for focus to rest on. Nothing else should move focus to it.
    pub(crate) fn new_focus(&self) -> &FocusHandle {
        &self.new_focus
    }

    /// The rail foot's `?` button, for the screenshot goldens, which open its menu without a click.
    #[cfg(test)]
    pub(crate) fn help_button(&self) -> &Entity<HelpButton> {
        &self.help
    }

    /// The rail foot's notices bell, for the screenshot goldens, which open its panel without a click.
    #[cfg(test)]
    pub(crate) fn notices_button(&self) -> &Entity<NoticesButton> {
        &self.notices
    }

    /// Which view the listing in the current UI session's store answers, for the status sounds: `None` while a change
    /// of the host or status filter is unanswered, otherwise a number that changes exactly when such a change is answered
    /// ([`filters::Choice::listing_view`]). The list notifies when an answer arrives, so an observer of the list sees
    /// the number move once the new view's listing is in the store.
    pub(crate) fn listing_view(&mut self, cx: &App) -> Option<u64> {
        self.current.current(cx).filters.listing_view()
    }

    /// The list, settled once when it is made and again on every change it observes.
    ///
    /// The settle at the end runs while the list is being made. [`session::observe_current`]'s own first call comes
    /// only after the update that made the list, and settles again from the state as it is then, which unless
    /// something changed in between finds nothing to do.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let selection = cx.observe_global::<LocalState>(|list, cx| {
            cx.notify();
            list.settle(cx);
        });
        let following = session::observe_current(cx, |list: &mut Self, cx| list.settle(cx));
        let trash = cx.new(TrashButton::new);
        let weak_trash = trash.downgrade();
        // First, so the foot's bell and Help button can read the rail's expansion from the entity it holds.
        let rail = host_rail::Kept::new(cx);
        let expansion = rail.expansion().clone();
        let mut list = SessionList {
            current: PerSession::default(),
            _following: following,
            _selection: selection,
            _host_status: crate::hosts::status::observe_changes(cx),
            _delete_notice: delete_notice::observe(cx),
            pulse: None,
            spinner: None,
            clock: None,
            rows: ListState::new(0, ListAlignment::Top, px(ROW_OVERDRAW)),
            entries: Rc::from([]),
            row_focus: HashMap::new(),
            templates_focus: cx.focus_handle().tab_stop(true),
            new_focus: cx.focus_handle().tab_stop(true),
            hinted_width: None,
            focus_seen: None,
            all_focus: cx.focus_handle().tab_stop(true),
            settings_focus: cx.focus_handle().tab_stop(true),
            hosts_focus: cx.focus_handle().tab_stop(true),
            help: cx.new(|cx| HelpButton::new(expansion.clone(), cx)),
            trash,
            notices: cx.new(|cx| NoticesButton::new(weak_trash, expansion, cx)),
            order_focus: cx.focus_handle().tab_stop(true),
            order_anchor: None,
            rail,
            attention_scroll: ScrollHandle::new(),
            hosts_panel: cx.new(HostsPanel::new),
            moves: HashMap::new(),
            marking: marking::Kept::new(cx),
            pointer_in_window: Window::is_window_hovered,
        };
        list.settle(cx);
        list
    }

    /// Apply the selection rules to the state as it is now: select (and remember) or clear, send the list's query again
    /// when it does not yet name the session now open ([`Self::name_open_session`]), and send the open session's seen
    /// mark when one is due. Nothing happens while the window is inert.
    fn settle(&mut self, cx: &mut Context<Self>) {
        // A "stays" count in the Delete notice band goes once the hosts read shows its environment gone or going.
        delete_notice::follow_hosts(cx);
        // A bulk Delete's rows not sent yet are held in the store as Deletes (`bulk`'s module docs), so they count here
        // as the Deletes in flight do, and the selection never opens one of them before its request goes out. An
        // environment session's Delete is a Delete here too.
        let deleting = session::current_operations(cx).is_some_and(|operations| {
            operations.read(cx).any_in_flight(|work| {
                matches!(
                    work,
                    operations::Work::Delete(_) | operations::Work::EnvironmentDelete
                )
            })
        });
        let launching = session::current_operations(cx).is_some_and(|operations| {
            operations
                .read(cx)
                .any_in_flight(|work| matches!(work, operations::Work::Launch))
        });
        let selected = local_state::selected(cx);
        // Read before the store is borrowed below: `delete`'s "This window's own Delete is evidence".
        let deleted_here = selected
            .as_deref()
            .is_some_and(|key| delete::deleted_here(key, cx));
        let current = self.current.current(cx);
        let Some(store) = session::current_store(cx) else {
            return;
        };
        if current.operations.is_none()
            && let Some(operations) = session::current_operations(cx)
        {
            current.operations = Some(cx.observe(&operations, |list, _, cx| list.settle(cx)));
        }
        let serves_named = selected
            .as_deref()
            .is_some_and(|key| current.filters.serves_named(key));
        let collections = store.read(cx).collections();
        // A mark goes on the same evidence as a session: a listing that proves it gone, never a filtered, cut or failed
        // one (`marking`'s "Stored and effective marks").
        current
            .marks
            .prune(|key| session::listing_proves_gone(collections, key));
        let listed = selection::listed(collections);
        // Gone on a complete listing without it, on any available listing without it once the query the helm serves
        // names it and no cap cut it away, or once this window's own Delete of it succeeded and the store no longer
        // holds it (`selection`'s "A selection stays until it is proven gone"); and never while its own Replace is in
        // flight (`selection`'s "A Replace keeps its own source open until it answers").
        let gone = selected.as_deref().is_some_and(|key| {
            let proven = session::listing_proves_gone(collections, key)
                || selection::absent_while_named(collections, key, serves_named)
                || (deleted_here && collections.get::<SessionRow>(key).is_none());
            proven && !replace::in_flight(key, cx)
        });
        // `selection`'s "A launch keeps an empty selection empty": set by a launch seen in flight over an empty list
        // with nothing open, spent by any selection.
        current.kept_empty =
            selected.is_none() && (current.kept_empty || (launching && listed.is_empty()));
        let change = selection::hold_for_launch(
            selection::hold_for_deletes(
                selection::settle(
                    selected.as_deref(),
                    &listed,
                    gone,
                    selection::preferences(collections),
                ),
                deleting,
            ),
            current.kept_empty,
        );
        // The seen mark is for whichever session is open once the change is made, read by key: the helm carries the
        // open session in the store also while the list leaves it out (`filters`, "The query names the open session").
        let open = match &change {
            Change::Keep => selected,
            Change::Select(key) => Some(key.clone()),
            Change::Clear => None,
        };
        let seen = selection::seen_due(
            match open.as_deref() {
                None => Open::Nothing,
                Some(key) => collections
                    .get::<SessionRow>(key)
                    .map_or(Open::Hidden(key), |row| Open::Shown(Listed { key, row })),
            },
            &current.stamped,
        );

        match change {
            Change::Keep => {}
            Change::Select(key) => selection::open_automatically(&key, cx),
            Change::Clear => {
                local_state::clear(cx);
            }
        }
        // Read after the change above, which took effect at once: the query names the session open from here on.
        self.name_open_session(cx);
        self.reconcile_bulk_question(cx);
        if let Some((stamped, send)) = seen {
            self.current.current(cx).stamped = stamped;
            if let Some((key, activity)) = send {
                selection::mark_seen(&key, activity, cx);
            }
        }
    }

    /// Bring the list state into step with `keys`, the listed sessions in order, at the top of a frame.
    ///
    /// When the sessions or their order changed, the state is rebuilt with every row's focus handle registered and the
    /// scroll position kept (the same row index and offset, as the old page kept its scroll offset; the order hold is
    /// what keeps rows still under the pointer). Rows not yet drawn are then given an estimated height,
    /// [`ROW_HEIGHT_HINT`]; a row's real height replaces its estimate once it is drawn. On a frame that did not give
    /// estimates, focus moving into a row then scrolls its neighbors into view ([`Self::reveal_focused`]).
    ///
    /// NOTE: The list drops every row's estimate itself the first time it lays out and whenever its width changes (its
    /// cached heights are for the old width), and a row without an estimate counts as zero pixels, which would leave the
    /// scroll range covering only the rows drawn so far. So a width other than the one the estimates were last given at
    /// gives them again: the frame that changed the width is laid out without them, and the next restores them.
    ///
    /// `keys` is `None` when the listing has the same sessions in the same order as `entries` (the caller compares, so
    /// an unchanged frame allocates nothing). Returns whether it gave the estimates, which the caller follows with one
    /// more frame: the first layout drops them right after this frame's sync, and nothing else may draw again soon.
    fn sync_rows(&mut self, keys: Option<Vec<String>>, window: &Window, cx: &App) -> bool {
        if let Some(keys) = keys {
            let listed: HashSet<&str> = keys.iter().map(String::as_str).collect();
            self.row_focus
                .retain(|key, _| listed.contains(key.as_str()));
            let row_focus = &mut self.row_focus;
            // Not a tab stop: the row's handle only contains its controls' focus, and never takes focus itself.
            self.entries = keys
                .into_iter()
                .map(|key| {
                    let handle = row_focus
                        .entry(key.clone())
                        .or_insert_with(|| cx.focus_handle())
                        .clone();
                    (key, handle)
                })
                .collect();
            let top = self.rows.logical_scroll_top();
            self.rows.splice_focusable(
                0..self.rows.item_count(),
                self.entries.iter().map(|(_, handle)| Some(handle.clone())),
            );
            self.rows.scroll_to(top);
            self.hinted_width = None;
        }
        let width = self.rows.viewport_bounds().size.width;
        if self.hinted_width != Some(width) {
            // The list state is shared, so the clone's builder sets the estimate on this list's rows. It fills in only
            // rows without a measured height or an estimate, keeps every row's focus handle, and turns measured rows
            // into estimated ones at their measured height, which the next layout measures again where it draws.
            let _ = self
                .rows
                .clone()
                .with_uniform_item_height(px(ROW_HEIGHT_HINT));
            self.hinted_width = Some(width);
            // No reveal on this frame: no row has a measured height yet, so every row would count as out of view and a
            // focus move would scroll rows that are fully visible. The frame the caller asks for judges it instead.
            return true;
        }
        self.reveal_focused(window, cx);
        false
    }

    /// Once per focus move the keyboard made, if focus is now inside a row: scroll a neighbor that is not fully in view
    /// into view (the one below first, else the one above), or failing both the row itself if it is not, so the next Tab or
    /// Shift+Tab finds a painted neighbor to move to (GPUI's Tab order holds only what was painted). Rows already fully
    /// in view never move: GPUI's reveal would scroll even those.
    ///
    /// Only for the keyboard ([`focus::last_input_was_keyboard`]): a click that focuses a row's control must not scroll
    /// the list before the click's release, which would then land on another element and be lost. The once-per-move
    /// check also keeps a wheel scroll after a Tab from being fought, since a wheel does not change the flag. Accepted
    /// edge case: Escape out of a menu opened at an edge row may scroll the list by up to a row.
    fn reveal_focused(&mut self, window: &Window, cx: &App) {
        let focused = window.focused(cx);
        let seen = self.focus_seen.as_ref().and_then(WeakFocusHandle::upgrade);
        if focused == seen {
            return;
        }
        self.focus_seen = focused.as_ref().map(FocusHandle::downgrade);
        if !focus::last_input_was_keyboard(window, cx) {
            return;
        }
        let Some(ix) = self
            .entries
            .iter()
            .position(|(_, handle)| handle.contains_focused(window, cx))
        else {
            return;
        };
        // Judged from the last layout, before any reveal in this pass moves the scroll position.
        let below = ix + 1 < self.entries.len() && !self.row_fully_in_view(ix + 1);
        let above = ix > 0 && !self.row_fully_in_view(ix - 1);
        let itself = !self.row_fully_in_view(ix);
        if below {
            self.rows.scroll_to_reveal_item(ix + 1);
        } else if above {
            self.rows.scroll_to_reveal_item(ix - 1);
        } else if itself {
            // Only when no neighbor moved: revealing the row after its neighbor can scroll back and undo the neighbor's
            // reveal when the rows differ in height.
            self.rows.scroll_to_reveal_item(ix);
        }
    }

    /// Whether row `ix` lies wholly inside the list's view as last laid out. A row the last layout did not measure (far
    /// from the view) is not.
    fn row_fully_in_view(&self, ix: usize) -> bool {
        let view = self.rows.viewport_bounds();
        self.rows
            .bounds_for_item(ix)
            .is_some_and(|row| row.top() >= view.top() && row.bottom() <= view.bottom())
    }

    /// Scroll the row of a plain Replace's YOLO question that took focus wholly into view, once per frame while
    /// [`replace_yolo::take_reveal`] names one, and ask for the next frame: SPEC.md has that question "scrolled into
    /// view", and its focus is lost if the row is not drawn in the next frame. Run before the rows are laid out, so the
    /// frame that runs it draws the row already. The first frame judges the row by its height without the question;
    /// the next measures it with the question and scrolls again if its foot is out of view.
    fn reveal_yolo_question(&mut self, cx: &mut Context<Self>) {
        let Some(key) = replace_yolo::take_reveal(cx) else {
            return;
        };
        if let Some(ix) = self.entries.iter().position(|(listed, _)| *listed == key)
            && !self.row_fully_in_view(ix)
        {
            self.rows.scroll_to_reveal_item(ix);
        }
        cx.notify();
    }

    /// Scroll `key`'s row into view if it is listed and out of view: a press on a control of a row scrolled away
    /// (its menu button kept focus through a wheel scroll) acts on a row the user can see, and an overlay it opens has
    /// a drawn anchor to sit beside.
    pub(super) fn reveal_row(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.row_in_view(key) {
            return;
        }
        if let Some(ix) = self.entries.iter().position(|(listed, _)| listed == key) {
            self.rows.scroll_to_reveal_item(ix);
            cx.notify();
        }
    }

    /// Whether `key`'s row is at least partly inside the list's view as last drawn. A row past the view's edges, or one
    /// the list does not show at all, is not.
    pub(super) fn row_in_view(&self, key: &str) -> bool {
        let Some(ix) = self.entries.iter().position(|(listed, _)| listed == key) else {
            return false;
        };
        self.rows.item_is_above_viewport(ix) == Some(false)
            && self.rows.item_is_below_viewport(ix) == Some(false)
    }
}

impl Render for SessionList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("session-list", cx);

        // The sidebar's body: the host rail, then the list's own column in the width the rail leaves, then the rail's
        // foot, which this row positions absolutely over the room the rail leaves at its bottom. Escape ends a pin drag
        // wherever focus is in the sidebar's body: the host rail, the list's column and the rail's foot
        // ([`pin_drag::catch_escape`]).
        let body_row = pin_drag::catch_escape(div().relative().flex().flex_row().size_full(), cx);
        let list = div()
            .id("session-list")
            .role(Role::List)
            .aria_label("Sessions")
            // Escape inside the column clears the marks without claiming the key ([`marking`]).
            .on_action(cx.listener(|list, _: &Dismiss, window, cx| list.escape_in_list(window, cx)))
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .min_h_0();
        let Some(store) = session::current_store(cx) else {
            self.pulse = None;
            self.spinner = None;
            self.clock = None;
            if !self.entries.is_empty() {
                self.sync_rows(Some(Vec::new()), window, cx);
            }
            return body_row.child(list);
        };
        // Whether a row's menu or its question is open, which keeps the order hold on whatever the pointer did.
        let menu_open = {
            let current = self.current.current(cx);
            current.rows.borrow_mut().any_open(window, cx)
                || current
                    .marked_menu
                    .is_some_and(|layer| crate::layers::is_open(layer, window, cx))
        };
        self.after_menus(menu_open, window, cx);
        // One pass over the listing for what the frame needs from all of it: the keys in the order drawn (pins first,
        // then as the order hold keeps them), the pins' Move neighbors and the separator, the marks shown and the
        // marked menu's offers, whether any row pulses or shows a spinner (a pending Delete or Stop, or a Replace that
        // has waited long enough to say so), and whether any has an activity stamp to show an age for. Everything else
        // is read row by row, for the rows drawn.
        let (
            keys,
            headers,
            member_groups,
            separators,
            projection,
            any_pulse,
            any_spinner,
            any_stamp,
            drag_bands,
            served_count,
        ) = {
            let collections = store.read(cx).collections();
            let list_id = self.list_id(collections, cx);
            let current = self.current.current(cx);
            current.hold.release_if_due(menu_open);
            let roster = groups::roster(collections);
            let served = selection::listed(collections);
            let served_count = served.len();
            let layout =
                current
                    .hold
                    .apply(groups::Layout::served(served, roster), roster, list_id);
            // Marks count only while the rows are shown and the helm could read the listing (`marking`'s "Stored and
            // effective marks"); otherwise the frame projects nothing and keeps the stored marks.
            let meta = collections.meta(Collection::Sessions);
            let exposed = !current.hosts_shown
                && meta.is_some_and(|meta| meta.availability == Availability::Available);
            current.last_uncollapsed = if exposed {
                current.hold.snapshot(&layout, list_id)
            } else {
                None
            };
            if meta.is_some_and(|meta| meta.availability == Availability::Available) {
                current
                    .group_collapse
                    .retain(|id, _| roster.iter().any(|group| group.id == *id));
            }
            let groups::Frame {
                keys,
                headers,
                member_groups,
                separators,
                moves,
                rows: listed,
                drag,
            } = groups::Frame::new(
                layout,
                roster,
                current.hold.original_pins(),
                &current.group_collapse,
                meta.and_then(|meta| meta.counts),
                cx,
            );
            let projection = exposed.then(|| marking::project(&listed, &current.marks));
            self.moves = if exposed { moves } else { HashMap::new() };
            // The rows a drag may land beside, and whose grips may drag them: the movable pins, in the order drawn, and
            // none while the rows are not shown or the listing is unavailable (`pin_drag`'s "The list cancels too"). A
            // retired session's pin is one of them as a target; `pin_drag` refuses only its own drag.
            let drag_bands = if exposed { drag } else { Vec::new() };
            let unchanged = keys.len() == self.entries.len()
                && keys
                    .iter()
                    .zip(self.entries.iter())
                    .all(|(next, (key, _))| next == key);
            let keys = (!unchanged).then_some(keys);
            // The sidebar bell takes the place of the running dot, so a
            // muted row alone needs no tick for the running animation.
            let any_pulse = listed
                .iter()
                .any(|session| !status::muted(session.row) && status::pulses(session.row))
                || headers.values().any(|header| {
                    header
                        .attention
                        .as_ref()
                        .is_some_and(groups::Attention::pulses)
                });
            let any_spinner = listed.iter().any(|session| {
                delete::in_flight(session.key, cx).is_some()
                    || stop::in_flight(session.key, cx)
                    || outcome::waited_seconds(session.key, operations::Work::Replace, cx).is_some()
                    || environment::pending_of(session.key, cx).is_some()
            });
            let any_stamp = listed
                .iter()
                .any(|session| session.row.info.effective_activity() > 0);
            (
                keys,
                headers,
                member_groups,
                separators,
                projection,
                any_pulse,
                any_spinner,
                any_stamp,
                drag_bands,
                served_count,
            )
        };
        let pin_drag = self.pin_drag_frame(drag_bands, window.is_window_active(), cx);
        self.marks_projected(projection, window, cx);
        let pulses = ticker::is_active(window, cx) && any_pulse;
        if pulses != self.pulse.is_some() {
            self.pulse = pulses.then(|| ticker::pulse(window, cx));
        }
        let spins = ticker::is_active(window, cx) && any_spinner;
        if spins != self.spinner.is_some() {
            self.spinner = spins.then(|| Spinner::start(window, cx));
        }
        if any_stamp != self.clock.is_some() {
            self.clock = any_stamp.then(|| ticker::clock(cx));
        }
        let stored_compact = order::compact_stored(store.read(cx).collections());
        let steps = row::Steps {
            pulse: self.pulse.as_ref().map_or(0, |pulse| pulse.step(cx)),
            spinner: self.spinner.as_ref().map_or(0, |spinner| spinner.step(cx)),
            now: age::seconds(ticker::wall_now(cx)),
            zone: local_time::zone(cx),
            compact: self.order_state(cx).read(cx).compact_shown(stored_compact),
        };
        if self.sync_rows(keys, window, cx) {
            cx.notify();
        }
        self.reveal_yolo_question(cx);
        let shared = self.current.current(cx).rows.clone();
        shared.borrow_mut().start_frame();
        let choice = self.current.current(cx).filters.clone();
        let hosts_shown = self.current.current(cx).hosts_shown;
        let weak = cx.weak_entity();
        // The heading's start before the store is borrowed for the rest of the frame: drawing the title or the marked
        // bar records an anchor, which needs the window's element state. While marks are shown the bar takes the
        // title's place (and the heading's), and neither is drawn while the Hosts panel shows.
        let stored = order::Order::stored_in(store.read(cx).collections());
        let bar = if hosts_shown {
            None
        } else {
            self.marked_bar(window, cx)
        };
        let title = (!hosts_shown && bar.is_none()).then(|| self.order_title(stored, window, cx));
        // The Hosts attention list above the heading ([`attention`]), absent while the panel shows. Read on every frame
        // all the same, since a change of what it draws closes the menus below it.
        let attention = self.attention_frame(hosts_shown, window, cx);
        // Read before the store is borrowed too: answering it needs the app mutably.
        let new_available = launcher::entry::new_available(cx);
        // The rail's avatars' anchors and focus handles are element state, made with the window, so before the store is
        // borrowed as well.
        let menu = self.prepare_rail(window, cx);
        let expanded = self.rail_expanded(cx);
        let collections = store.read(cx).collections();
        let palette = theme::palette(cx);
        let body_row = body_row.child(host_rail::rail(
            collections,
            &weak,
            &self.all_focus,
            &self.rail,
            host_rail::Look {
                chosen: choice.chosen().host,
                quiet: hosts_shown,
                expanded,
                menu,
            },
            cx,
        ));
        // Drawn after the list's column, so Tab reaches the list's controls before the foot (`host_rail`'s docs).
        let foot = host_rail::foot(
            &weak,
            host_rail::FootParts {
                hosts_focus: &self.hosts_focus,
                settings_focus: &self.settings_focus,
                notices: &self.notices,
                help: &self.help,
            },
            &self.rail,
            hosts_shown,
            expanded,
            cx,
        );
        if hosts_shown {
            // The Hosts panel, in the column's box, which gives it the definite height it scrolls its rows in. The
            // list's column is not drawn meanwhile, so neither are its rows and controls.
            return body_row
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .child(self.hosts_panel.clone()),
                )
                .child(foot);
        }
        // The heading and the segments row are drawn before the first listing too (the module docs).
        let filter_row = filters::row(collections, choice.chosen(), &weak, expanded, palette);
        // Under the segments, where the old `.delete-notice` sat between the status filter and the listing.
        let notice = delete_notice::band(cx);
        let refused = choice
            .refusal()
            .map(|refusal| failure_line(filters::refusal_sentence(refusal), palette));
        let Some(meta) = collections.meta(Collection::Sessions) else {
            return body_row
                .child(
                    list.children(attention)
                        .child(heading(bar, title, None, self, new_available, &weak, cx))
                        .child(filter_row)
                        .children(notice)
                        .children(refused)
                        .child(status_line("Loading sessions…")),
                )
                .child(foot);
        };
        let selected = local_state::selected(cx);

        let count = count_line(meta, served_count);
        let heading = heading(
            bar,
            title,
            match &count {
                CountLine::Count(count) => Some(count.clone()),
                CountLine::Cut(_) => None,
            },
            self,
            new_available,
            &weak,
            cx,
        );
        let truncated = match count {
            CountLine::Count(_) => None,
            CountLine::Cut(warning) => Some(cut_band(warning, palette)),
        };
        let unavailable = match &meta.availability {
            Availability::Available => None,
            // The helm's own description, which may quote what a host answered: host text.
            Availability::Unavailable { error } => Some(failure_line(
                HostSentence::new("session-list-error")
                    .text("Failed to load sessions: ")
                    .host(&HostText::from_host(error.as_str())),
                palette,
            )),
        };

        let empty = filters::empty(meta, served_count, choice.answered());
        let body: AnyElement = if let Some(empty) = empty {
            filters::empty_state(collections, empty, &weak, palette).into_any_element()
        } else if empty_fleet(collections) {
            status_line("No sessions").into_any_element()
        } else {
            // What the item callback needs, by value: it runs while the list lays out, after this render returns, and
            // builds only the rows it draws.
            let entries = self.entries.clone();
            let marked = self.marking.shown();
            let weak = weak.clone();
            let watch = pin_drag::watch(&pin_drag, &weak);
            let rows = div()
                .id("session-rows")
                // Tests scroll the list over this; it records nothing outside test builds.
                .debug_selector(|| "session-rows".into())
                .flex_1()
                .min_h_0();
            hold::area(rows, &weak)
                .child(watch)
                .child(
                    gpui::list(self.rows.clone(), move |ix, window, cx| {
                        let Some((key, focus)) = entries.get(ix) else {
                            return div().into_any_element();
                        };
                        let row = if let Some(header) = headers.get(key) {
                            groups::draw_header(
                                header,
                                focus,
                                &shared,
                                &pin_drag,
                                &weak,
                                steps.pulse,
                                window,
                                cx,
                            )
                        } else {
                            let selected = selected.as_deref() == Some(key.as_str());
                            let marked = marked.iter().any(|shown| shown == key);
                            draw_row(
                                &shared,
                                &pin_drag,
                                &weak,
                                key,
                                focus,
                                selected,
                                marked,
                                member_groups.contains_key(key),
                                steps,
                                window,
                                cx,
                            )
                        };
                        if let Some(separator) = separators.get(key) {
                            // After the row's item, so the item's own border stays its bottom edge.
                            div()
                                .child(row)
                                .child(pins::separator(*separator, theme::palette(cx)))
                                .into_any_element()
                        } else {
                            row
                        }
                    })
                    .size_full(),
                )
                .into_any_element()
        };
        body_row
            .child(
                list.children(attention)
                    .child(heading)
                    .child(filter_row)
                    .children(notice)
                    .children(refused)
                    .children(truncated)
                    .children(unavailable)
                    .child(body),
            )
            .child(foot)
    }
}

/// Draw the row of `key` for the virtualized list's item callback: its menu button, its notification bell and its part
/// of a pin drag first (their anchors are element state, and the drag records the row's bounds as it is laid out, so
/// they are made while the row's elements are), then the row from the store as it is now. A session missing from the
/// store draws nothing; the callback runs in the frame whose listing named it, so that does not happen.
#[allow(
    clippy::too_many_arguments,
    reason = "the item callback's captures, passed through once"
)]
fn draw_row(
    shared: &menu::SharedRows,
    pin_drag: &pin_drag::Shared,
    weak: &WeakEntity<SessionList>,
    key: &str,
    focus: &FocusHandle,
    selected: bool,
    marked: bool,
    grouped: bool,
    steps: row::Steps,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let Some(store) = session::current_store(cx) else {
        return div().into_any_element();
    };
    let Some((title, retired)) = store
        .read(cx)
        .collections()
        .get::<SessionRow>(key)
        .map(|row| (row.info.title.clone(), retired::is(row)))
    else {
        return div().into_any_element();
    };
    let hover = row::RowHover::of(key, window, cx);
    // Two of the menu button's reveal reasons, which only the row knows: the pointer over it, focus inside it.
    let row_active = hover.hovered(cx) || focus.contains_focused(window, cx);
    let menu = menu::menu_button(shared, weak, key, &title, selected, row_active, window, cx);
    // A retired session draws no bell: closing one writes a read mark, and such a session supports Delete only.
    let bell = (!retired)
        .then(|| crate::notices_bell::bell(key, selected, window, cx))
        .flatten();
    let drag = pin_drag::paint(
        pin_drag, shared, weak, key, selected, row_active, window, cx,
    );
    let card = summary::SummaryCard::of(key, steps, window, cx);
    let collections = store.read(cx).collections();
    let Some(row) = collections.get::<SessionRow>(key) else {
        return div().into_any_element();
    };
    let (open, list) = (key.to_owned(), weak.clone());
    row::row(
        Listed { key, row },
        collections,
        selected,
        marked,
        grouped,
        steps,
        menu,
        bell,
        drag,
        card,
        focus,
        hover,
        // A plain click opens the session and clears the marks; Shift or Cmd or Ctrl marks instead ([`marking`]).
        move |gesture, _, cx| {
            if gesture == marking::Gesture::Open {
                selection::choose(&open, cx);
            }
            let _ = list.update(cx, |list, cx| list.row_clicked(&open, gesture, cx));
        },
        cx,
    )
}

/// The list's heading band (`.session-heading`): the order's title ([`order`]), the count beside it when the list has
/// one to show (none before the first listing, and none for a cut listing, whose warning has a line of its own), and
/// the heading's actions at its right edge (`.session-heading-actions`), in SPEC.md's order: Trash (`owner.trash`, a
/// view of its own), Templates, then New, drawn disabled unless `new_available` ([`launcher::entry::new_available`],
/// read by the caller). The band wraps, so in a narrow column the actions take a line of their own under the title.
/// Templates and New track the focus handles `owner` keeps for them. While marks are shown,
/// `bar`, the marked bar the caller drew ([`marking`]), is the band instead, and the caller draws no `title`: the bar
/// replaces the title, the count and the actions, and Trash's view stays alive undrawn.
fn heading(
    bar: Option<AnyElement>,
    title: Option<AnyElement>,
    count: Option<String>,
    owner: &SessionList,
    new_available: bool,
    list: &WeakEntity<SessionList>,
    cx: &App,
) -> AnyElement {
    if let Some(bar) = bar {
        return bar;
    }
    let palette = theme::palette(cx);
    let trash = &owner.trash;
    let templates_focus = &owner.templates_focus;
    let templates = IconButton::new(
        "templates-button",
        Icon::Templates,
        "Templates",
        templates::button_help(cx),
        Role::Button,
    )
    .tier(Tier::Neutral)
    .square(px(HEADING_ICON_SIDE))
    .focus(templates_focus)
    .on_click({
        let opener = templates_focus.clone();
        move |_, window, cx| {
            let mark = Mark::take(cx);
            let _ = templates::open(&opener, &mark, window, cx);
        }
    });
    // The old `.btn-primary.new-session-button`: the Add glyph in the band's 24-pixel square, named "New session", and
    // the one filled control in the sidebar (SPEC.md: "New remains the sidebar's one filled control"). Disabled rather
    // than busy, as the old button was, so it leaves the Tab order while New is unavailable; `open_new` refuses a press
    // that lands before the redraw all the same.
    let list = list.clone();
    let new = IconButton::new(
        NEW_ID,
        Icon::Add,
        "New session",
        HoverHelp::tooltip(launcher::entry::NEW_SESSION_HELP),
        Role::Button,
    )
    .tier(Tier::Primary)
    .square(px(HEADING_ICON_SIDE))
    .disabled(!new_available)
    .focus(&owner.new_focus)
    .on_click(move |_, window, cx| open_new(&list, None, window, cx));
    div()
        .flex()
        .flex_row()
        // The old `.session-heading { flex-wrap: wrap }`: in a column too narrow for the title, the count and the
        // actions on one line (beside the expanded rail in a minimum window), the actions wrap below the title rather
        // than covering it.
        .flex_wrap()
        .items_center()
        .gap(px(6.))
        .py(metrics::LIST_HEADING_PAD.0)
        .px(metrics::LIST_HEADING_PAD.1)
        .bg(theme::color(palette.bg_2))
        .children(title)
        .children(count.map(|count| {
            let name = format!("Sessions: {count}");
            // The old count's `aria-label` and `data-tooltip`; the name and the help are the same words, so the help is
            // not read a second time as its description.
            StatusMark::new(
                COUNT_ID,
                name.clone(),
                HoverHelp::tooltip(name),
                Role::Label,
                count,
            )
            .described(false)
            .into_any_element()
        }))
        .child(
            // `margin-left: auto; flex-shrink: 0`: the actions keep their place at the right edge as one group.
            div()
                .ml_auto()
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(HEADING_ACTIONS_GAP))
                .child(trash.clone())
                .child(templates)
                .child(new),
        )
        .into_any_element()
}

/// The heading's New button's element id: tests and the end-to-end smoke test find it by this.
pub(crate) const NEW_ID: &str = "new-session-button";

/// Open New in `window` through the launcher's entry point, named `name` for the quick switcher's "New session named X"
/// row and unnamed for the heading's New and the New shortcut, and when it opens, show Sessions in place of the Hosts
/// panel (SPEC.md: "All or a host avatar returns to Sessions, as does an accepted quick-switcher pick or New handoff; a
/// refused handoff leaves the panel as it was"). `list` is the window's session list, held weakly as the switcher holds
/// it. New refuses whenever the entry point does ([`launcher::entry::open_new`]: New unavailable, a dialog or menu open,
/// no UI session), so a trigger inside a layer closes that layer first; nothing changes then.
pub(crate) fn open_new(
    list: &WeakEntity<SessionList>,
    name: Option<&str>,
    window: &mut Window,
    cx: &mut App,
) {
    let opened = match name {
        Some(name) => launcher::entry::open_new_named(name, window, cx),
        None => launcher::entry::open_new(window, cx),
    };
    if opened {
        let _ = list.update(cx, |list, cx| list.hide_hosts(cx));
    }
}

/// Show Sessions in place of the Hosts panel in `window`, for a hand-off that starts outside the list: a quick launch
/// from the page (the ⚡ chord, a number key, a sequence or the template list), once it has started a launch or opened
/// New, and the open session's header Clone, Restart with… and Replace with… and its interrupted card's Replace with…,
/// once the launcher has opened (`session_pane::header`, `session_pane::card`). The old app's quick launch went through
/// the list's own New, which hid the panel, and its header and card revealed Sessions for those too (SPEC.md: "All or a
/// host avatar returns to Sessions, as does an accepted quick-switcher pick or New handoff"), so a session launched from
/// the panel shows up where the user can see it. The list is the one the window's main view holds; a window without one
/// (a golden's scene) has no panel, and nothing changes. Call it only once the hand-off was accepted, and never from
/// inside an update of the main view or the session list: it reads the one and updates the other, which GPUI refuses
/// while either is already being updated.
pub(crate) fn show_sessions(window: &mut Window, cx: &mut App) {
    let Some(Some(root)) = window.root::<gpui_component::Root>() else {
        return;
    };
    let Ok(main) = root.read(cx).view().clone().downcast::<MainView>() else {
        return;
    };
    let list = main.read(cx).session_list().clone();
    list.update(cx, |list, cx| list.hide_hosts(cx));
}

/// The count's element id, for the count beside the title and the cut listing's warning alike (only one is drawn).
pub(crate) const COUNT_ID: &str = "session-count";

/// The heading's icon buttons' side (`.session-heading .templates-button`: `height: 24px`, the box including its edge),
/// with the 14-pixel icon centered where the old `padding: 3px` put it.
const HEADING_ICON_SIDE: f32 = 24.;

/// `.session-heading-actions`'s gap between its buttons.
const HEADING_ACTIONS_GAP: f32 = 4.;

/// What the list says about its count: the count beside the title, or, for a listing the helm's cap cut, a warning on
/// a line of its own instead, because the numbers alone would read as the whole list. Ported from the old app's
/// `count_banner`, without its unsupported-filter wording, which nothing the channel's helm sends can call for
/// ([`filters`]' NOTE). A collection without counts (no helm sends one, but the type allows it) counts its rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CountLine {
    /// "N" for a complete list, "N of M" for a narrowed one (N matched, M is the whole fleet).
    Count(String),
    /// "Showing N of M sessions — could not read the list to the end".
    Cut(String),
}

/// The count line for `meta`, whose listing has `shown` rows.
pub(crate) fn count_line(meta: &CollectionMeta, shown: usize) -> CountLine {
    let Some(counts) = meta.counts else {
        return CountLine::Count(shown.to_string());
    };
    match (counts.truncated, counts.matching) {
        (false, None) => CountLine::Count(complete_count(counts.total, None)),
        (false, Some(matching)) => CountLine::Count(complete_count(matching, Some(counts.total))),
        (true, None) => CountLine::Cut(format!(
            "Showing {shown} of {} sessions — could not read the list to the end",
            counts.total
        )),
        (true, Some(matching)) => CountLine::Cut(format!(
            "Showing {shown} of {matching} matching sessions ({} in all) — could not read the list to the end",
            counts.total
        )),
    }
}

/// Both group and list counts name a denominator when a filter is active.
fn complete_count(shown: u64, total: Option<u64>) -> String {
    total.map_or_else(|| shown.to_string(), |total| format!("{shown} of {total}"))
}

/// Whether the helm has shown that there are no sessions at all: an available, complete, unfiltered listing with no
/// rows. Only then may the window say "No sessions"; an unread, failed or cut listing with no rows claims nothing.
pub(crate) fn empty_fleet(collections: &Collections) -> bool {
    let Some(meta) = collections.meta(Collection::Sessions) else {
        return false;
    };
    meta.availability == Availability::Available
        && meta.counts.is_none_or(|counts| {
            !counts.truncated && counts.total == 0 && counts.matching.is_none_or(|m| m == 0)
        })
        && selection::listed(collections).is_empty()
}

/// One of the list's own one-line states ("Loading sessions…", "No sessions"): the old `.status` line.
fn status_line(text: &'static str) -> impl IntoElement {
    div()
        .p(metrics::LIST_STATUS_PAD)
        .text_size(metrics::TEXT_ROW_TITLE)
        .child(text)
}

/// A failure line across the list's column: 12-pixel red words (a read that failed, a query the helm refused), which
/// may quote a host and so arrive as a host sentence.
fn failure_line(sentence: HostSentence, palette: &theme::Palette) -> impl IntoElement {
    div()
        .py(px(4.))
        .px(px(10.))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.danger))
        .child(sentence)
}

/// A cut listing's warning in place of the count: a full-width band of 12-pixel amber text on the warning fill, as the
/// old `.banner.truncation-banner` line, named and helped as "Sessions: <warning>" as the old count was.
fn cut_band(warning: String, palette: &theme::Palette) -> impl IntoElement {
    let name = format!("Sessions: {warning}");
    div()
        .py(px(4.))
        .px(px(10.))
        .bg(theme::color(palette.warn_bg))
        .child(
            StatusMark::new(
                COUNT_ID,
                name.clone(),
                HoverHelp::tooltip(name),
                Role::Label,
                warning,
            )
            .ink(palette.warn_bright)
            .described(false),
        )
}
