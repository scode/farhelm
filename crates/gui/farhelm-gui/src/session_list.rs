//! The session list: every session the helm lists, in the helm's order, with the open one selected.
//!
//! SPEC.md, "Session list". The list is drawn from the store while it draws and keeps nothing the helm pushed: its rows
//! are the sessions collection in the order the collection's metadata gives (the helm sorts; the channel starts in the
//! user's stored order), so a pushed change or a reset redraws it with nothing to go stale. Above the rows, a heading
//! band names the order and carries the count; a listing the helm's cap cut says so in an amber band, and a failed read
//! says so in red, while the rows it had stay (a failed read is no evidence that anything was deleted).
//!
//! The list also owns the selection rules ([`selection`]), and applies them from observers, never from drawing: it
//! settles the selection and the open session's seen mark ([`SessionList::settle`]) whenever the connection changes (a
//! new UI session after a reset, or none while the window is inert), the selection changes, or the current UI session's
//! store applies a pushed change. GPUI draws nothing for a window that is hidden, minimized or behind other windows,
//! and SPEC.md, "Status", wants a session open in a window behind other apps recorded as seen the same way as one in
//! a focused window; work driven by frames would wait until the window showed again. Each settle recomputes from the
//! state as it is then. A settle that changes the selection notifies the selection's observers, the list among them,
//! and that second settle finds nothing to do.
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
//! Each row has an actions menu ([`menu`]) whose commands so far are Rename ([`rename`]), Mark read or Mark unread
//! ([`seen`], also on a click on the row's status dot), Replace ([`replace`]), Stop ([`stop`]) and Delete ([`delete`]);
//! the Rename and Delete shortcuts reach them for the open session ([`rename::open_for_selected`],
//! [`SessionList::delete_selected`]). What a row says about its operations beyond
//! "in flight" (a refusal, a success's notice, how long a Replace has waited) is [`outcome`]'s. While any Delete is in
//! flight, the selection rules hold back their automatic choice, so a list that loses its open session to that Delete
//! opens the next one only once every Delete has settled; a Stop or a Replace holds nothing back, with one exception
//! for the open session's own Replace ([`selection`], "A Replace keeps its own source open until it answers"). Every
//! pending Delete's, Stop's and long Replace's spinner turns from one handle the list holds, the way the pulse does
//! ([`delete::Spinner`]). A row's menu button and its anchor exist only while the row is drawn, so a menu whose row
//! scrolls well out of view closes, and a question about a row out of view is asked centered.
//!
//! The Rename editor the list owns is a dialog rather than part of a row, so a listing that changes under it never
//! moves its field.
//!
//! The quick switcher ([`quick_switcher`]) belongs to the list too: a dialog, opened with its shortcut, that searches
//! a listing of its own and opens the session the user picks through the same selection rules as a click on a row.
//!
//! The host rail ([`host_rail`]) is drawn by the list, at the sidebar's left edge with the list's own column beside
//! it, because choosing a host is the list's query: the rail sets the UI channel's session query, the list's state
//! keeps the choice per UI session, and the list's column shows the chosen host's chip under its heading and, for a
//! host with no sessions, an empty state that names it.
//!
//! NOTE: Not ported yet, each with the area that brings it: the order menu (the title is the stored order's name, not a
//! control), the status segments, compact rows, marking, pins, the row menu's other commands, and the rail's host
//! status, foot, Add host and expansion.

pub(crate) mod delete;
pub(crate) mod host_rail;
mod menu;
pub(crate) mod outcome;
pub(crate) mod quick_switcher;
pub(crate) mod rename;
pub(crate) mod replace;
mod row;
pub(crate) mod seen;
mod selection;
mod status;
pub(crate) mod stop;

#[cfg(test)]
mod lifecycle_tests;

#[cfg(test)]
mod list_tests;

#[cfg(test)]
mod menu_tests;

#[cfg(test)]
mod seen_tests;

#[cfg(test)]
pub(crate) mod tests;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionMeta, Preferences, SessionRow,
};
use gpui::{
    AnyElement, App, Context, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    ListAlignment, ListState, ParentElement, Pixels, Render, Role, SharedString,
    StatefulInteractiveElement, Styled, Subscription, WeakEntity, WeakFocusHandle, Window, div, px,
};

use crate::host_text::{HostSentence, HostText};
use crate::local_state::{self, LocalState};
use crate::operations;
use crate::render_count;
use crate::session::{self, PerSession};
use crate::theme::{self, metrics};
use crate::ticker;
use selection::{Change, Listed, Open, Stamped};

/// How far past each edge of its view the list draws rows: about two noncompact rows, so a row scrolling in is already
/// laid out and a short scroll draws nothing new. Every pixel of it is rows drawn on every frame, which is the cost the
/// virtualization exists to bound.
const ROW_OVERDRAW: f32 = 100.;

/// The height a row the list has not drawn yet is taken to have: about a noncompact row with its status and folder
/// lines. The list needs an estimate for every row to know how far it can scroll; a row's real height replaces it once
/// the row is drawn, so the estimate only has to be close.
const ROW_HEIGHT_HINT: f32 = 50.;

/// The session list view. The main view holds one for the window's life.
///
/// It must be made after the connection ([`session::start`]): it observes the connection it finds when it is made, and
/// without one it never settles anything. The bootstrap starts the connection before it opens the window.
pub struct SessionList {
    /// The state that belongs to the current UI session alone, started over after a reset.
    current: PerSession<Current>,
    /// Settle on a change of the connection and of the selection (which also redraws the list).
    _observing: Vec<Subscription>,
    /// The pulse's ticks (see the module docs), held exactly while the window is active and some listed row pulses.
    /// Taken and dropped at the top of each frame, before the frame reads its step, as the terminal cursor's blink
    /// is: everything that decides it (the store, the connection, the window's activation) redraws the list when it
    /// changes, so working it out there cannot miss a change.
    pulse: Option<ticker::Pulsing>,
    /// The pending operations' spinner, held exactly while the window is active and some listed row shows one (a
    /// Delete or a Stop in flight, or a Replace that has waited long enough to say so), and worked out at the top of
    /// each frame like `pulse`.
    spinner: Option<delete::Spinner>,
    /// The virtualized rows' state: their count, measured heights, scroll position and focus handles. Kept in step
    /// with the store by [`SessionList::sync_rows`] at the top of every frame; it holds no session data of its own.
    rows: ListState,
    /// The listed sessions' keys in the order the list state was last rebuilt to, each with its row's focus handle:
    /// how a frame tells whether the rows or their order changed, and what the item callback draws by index.
    entries: Rc<[(String, FocusHandle)]>,
    /// Each listed session's row focus handle (see the module docs), owned here so it outlives a frame in which the
    /// row is not drawn and stays the same across rebuilds; dropped once the session is no longer listed.
    row_focus: HashMap<String, FocusHandle>,
    /// The list's width when its rows were last given their height estimate; see [`SessionList::sync_rows`].
    hinted_width: Option<Pixels>,
    /// What held the window's focus when [`SessionList::reveal_focused`] last looked, so it acts once per focus move.
    ///
    /// NOTE: Weak on purpose. A strong handle here would keep whatever held focus alive after it went away (a terminal
    /// dropped by a reset, say), and the window would go on reporting it as focused, so focus would never go home.
    focus_seen: Option<WeakFocusHandle>,
    /// The host rail's All button's focus handle, a tab stop: kept here rather than in the button's element state so
    /// that removing the host filter from the chip or the empty state can move focus to All ([`host_rail`]).
    all_focus: FocusHandle,
}

/// What the list keeps for one UI session.
#[derive(Default)]
struct Current {
    /// Settles on every change the session's store applies. Made by the first settle under the session; dropped with
    /// this state when the session changes, so a retired store is never observed.
    store: Option<Subscription>,
    /// What the open session's automatic seen mark last acted on: after a reset it starts over, so the open session is
    /// marked again if it is unseen.
    stamped: Stamped,
    /// Settles whenever the session's operation store changes, so the selection held back for a Delete moves on once
    /// the Delete settles. Made and dropped like `store`.
    operations: Option<Subscription>,
    /// The rows' menu buttons and the layers they opened, shared with the item callback that draws the rows.
    rows: menu::SharedRows,
    /// The host rail's choice and the helm's answers to it. Per UI session because the helm's channel keeps the query
    /// per channel: a new channel starts unfiltered, and so does this.
    hosts: host_rail::Choice,
}

impl SessionList {
    /// The list, settled once when it is made and again on every change it observes.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut observing = vec![cx.observe_global::<LocalState>(|list, cx| {
            cx.notify();
            list.settle(cx);
        })];
        if let Some(connection) = session::connection(cx) {
            observing.push(cx.observe(&connection, |list, _, cx| list.settle(cx)));
        }
        let mut list = SessionList {
            current: PerSession::default(),
            _observing: observing,
            pulse: None,
            spinner: None,
            rows: ListState::new(0, ListAlignment::Top, px(ROW_OVERDRAW)),
            entries: Rc::from([]),
            row_focus: HashMap::new(),
            hinted_width: None,
            focus_seen: None,
            all_focus: cx.focus_handle().tab_stop(true),
        };
        list.settle(cx);
        list
    }

    /// Apply the selection rules to the state as it is now: select (and remember) or clear, and send the open
    /// session's seen mark when one is due. Nothing happens while the window is inert.
    fn settle(&mut self, cx: &mut Context<Self>) {
        let deleting = session::current_operations(cx).is_some_and(|operations| {
            operations
                .read(cx)
                .any_in_flight(|work| matches!(work, operations::Work::Delete(_)))
        });
        let current = self.current.current(cx);
        let Some(store) = session::current_store(cx) else {
            return;
        };
        if current.store.is_none() {
            current.store = Some(cx.observe(&store, |list, _, cx| list.settle(cx)));
        }
        if current.operations.is_none()
            && let Some(operations) = session::current_operations(cx)
        {
            current.operations = Some(cx.observe(&operations, |list, _, cx| list.settle(cx)));
        }
        let collections = store.read(cx).collections();
        let listed = selection::listed(collections);
        let selected = local_state::selected(cx);
        // `selection`'s "A Replace keeps its own source open until it answers".
        let gone = selected.as_deref().is_some_and(|key| {
            session::listing_proves_gone(collections, key) && !replace::in_flight(key, cx)
        });
        let change = selection::hold_for_deletes(
            selection::settle(
                selected.as_deref(),
                &listed,
                gone,
                selection::preferences(collections),
            ),
            deleting,
        );
        // The seen mark is for whichever session is open once the change is made.
        let open = match &change {
            Change::Keep => selected,
            Change::Select(key) => Some(key.clone()),
            Change::Clear => None,
        };
        let seen = selection::seen_due(
            match open.as_deref() {
                None => Open::Nothing,
                Some(key) => listed
                    .iter()
                    .find(|session| session.key == key)
                    .map_or(Open::Hidden(key), |session| Open::Shown(*session)),
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
    /// Only for the keyboard (`last_input_was_keyboard`, which GPUI sets to false on a mouse press before dispatching
    /// it): a click that focuses a row's control must not scroll the list before the click's release, which would then
    /// land on another element and be lost. The once-per-move check also keeps a wheel scroll after a Tab from being
    /// fought, since a wheel does not change the input mode. Accepted edge cases: a pointer move between a key and the
    /// next frame can skip a reveal, and Escape out of a menu opened at an edge row may scroll the list by up to a row.
    fn reveal_focused(&mut self, window: &Window, cx: &App) {
        let focused = window.focused(cx);
        let seen = self.focus_seen.as_ref().and_then(WeakFocusHandle::upgrade);
        if focused == seen {
            return;
        }
        self.focus_seen = focused.as_ref().map(FocusHandle::downgrade);
        if !window.last_input_was_keyboard() {
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

        // The sidebar's body: the host rail, then the list's own column in the width the rail leaves.
        let body_row = div().flex().flex_row().size_full();
        let list = div()
            .id("session-list")
            .role(Role::List)
            .aria_label("Sessions")
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .min_h_0();
        let Some(store) = session::current_store(cx) else {
            self.pulse = None;
            self.spinner = None;
            if !self.entries.is_empty() {
                self.sync_rows(Some(Vec::new()), window, cx);
            }
            return body_row.child(list);
        };
        // One pass over the listing for what the frame needs from all of it: the keys, and whether any row pulses or
        // shows a spinner (a pending Delete or Stop, or a Replace that has waited long enough to say so). Everything
        // else is read row by row, for the rows drawn.
        let (keys, any_pulse, any_spinner) = {
            let listed = selection::listed(store.read(cx).collections());
            let unchanged = listed.len() == self.entries.len()
                && listed
                    .iter()
                    .zip(self.entries.iter())
                    .all(|(session, (key, _))| session.key == key);
            let keys = (!unchanged).then(|| {
                listed
                    .iter()
                    .map(|session| session.key.to_owned())
                    .collect::<Vec<String>>()
            });
            let any_pulse = listed.iter().any(|session| status::pulses(session.row));
            let any_spinner = listed.iter().any(|session| {
                delete::in_flight(session.key, cx).is_some()
                    || stop::in_flight(session.key, cx)
                    || outcome::replace_seconds(session.key, cx).is_some()
            });
            (keys, any_pulse, any_spinner)
        };
        let pulses = ticker::is_active(window, cx) && any_pulse;
        if pulses != self.pulse.is_some() {
            self.pulse = pulses.then(|| ticker::pulse(window, cx));
        }
        let spins = ticker::is_active(window, cx) && any_spinner;
        if spins != self.spinner.is_some() {
            self.spinner = spins.then(|| delete::Spinner::start(window, cx));
        }
        let steps = row::Steps {
            pulse: self.pulse.as_ref().map_or(0, |pulse| pulse.step(cx)),
            spinner: self.spinner.as_ref().map_or(0, |spinner| spinner.step(cx)),
        };
        if self.sync_rows(keys, window, cx) {
            cx.notify();
        }
        let shared = self.current.current(cx).rows.clone();
        shared.borrow_mut().start_frame();
        let choice = self.current.current(cx).hosts.clone();
        let weak = cx.weak_entity();
        let collections = store.read(cx).collections();
        let palette = theme::palette(cx);
        let body_row = body_row.child(host_rail::rail(
            collections,
            choice.chosen(),
            &weak,
            &self.all_focus,
            palette,
        ));
        let Some(meta) = collections.meta(Collection::Sessions) else {
            return body_row.child(list.child(status_line("Loading sessions…")));
        };
        let selected = local_state::selected(cx);

        let count = count_line(meta, self.entries.len());
        let heading = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .py(metrics::LIST_HEADING_PAD.0)
            .px(metrics::LIST_HEADING_PAD.1)
            .bg(theme::color(palette.bg_2))
            .child(
                div()
                    .text_size(metrics::TEXT_MD)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_0))
                    .child(order_title(selection::preferences(collections))),
            )
            .children(match &count {
                CountLine::Count(count) => Some(
                    div()
                        .text_size(metrics::TEXT_SM)
                        .text_color(theme::color(palette.fg_1))
                        .child(count.clone()),
                ),
                CountLine::Cut(_) => None,
            });
        let truncated = match count {
            CountLine::Count(_) => None,
            CountLine::Cut(warning) => Some(band(warning, palette.warn_bg, palette.warn_bright)),
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
        let chip = choice
            .chosen()
            .map(|host| host_rail::chip_row(collections, host, &weak, palette));
        let refused = choice
            .refusal()
            .map(|words| failure_line(host_rail::refusal_sentence(words), palette));

        let empty_host = host_rail::empty_host(meta, self.entries.len(), choice.answered());
        let body: AnyElement = if let Some(host) = empty_host {
            host_rail::empty_state(collections, host, &weak, palette).into_any_element()
        } else if empty_fleet(collections) {
            status_line("No sessions").into_any_element()
        } else {
            // What the item callback needs, by value: it runs while the list lays out, after this render returns, and
            // builds only the rows it draws.
            let entries = self.entries.clone();
            let weak = weak.clone();
            div()
                .id("session-rows")
                // Tests scroll the list over this; it records nothing outside test builds.
                .debug_selector(|| "session-rows".into())
                .flex_1()
                .min_h_0()
                .child(
                    gpui::list(self.rows.clone(), move |ix, window, cx| {
                        let Some((key, focus)) = entries.get(ix) else {
                            return div().into_any_element();
                        };
                        let selected = selected.as_deref() == Some(key.as_str());
                        draw_row(&shared, &weak, key, focus, selected, steps, window, cx)
                    })
                    .size_full(),
                )
                .into_any_element()
        };
        body_row.child(
            list.child(heading)
                .children(chip)
                .children(refused)
                .children(truncated)
                .children(unavailable)
                .child(body),
        )
    }
}

/// Draw the row of `key` for the virtualized list's item callback: its menu button first (its anchor is element state,
/// so it is made while the row's elements are), then the row from the store as it is now. A session missing from the
/// store draws nothing; the callback runs in the frame whose listing named it, so that does not happen.
#[allow(
    clippy::too_many_arguments,
    reason = "the item callback's captures, passed through once"
)]
fn draw_row(
    shared: &menu::SharedRows,
    weak: &WeakEntity<SessionList>,
    key: &str,
    focus: &FocusHandle,
    selected: bool,
    steps: row::Steps,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let Some(store) = session::current_store(cx) else {
        return div().into_any_element();
    };
    let Some(title) = store
        .read(cx)
        .collections()
        .get::<SessionRow>(key)
        .map(|row| row.info.title.clone())
    else {
        return div().into_any_element();
    };
    let menu = menu::menu_button(shared, weak, key, &title, selected, window, cx);
    let collections = store.read(cx).collections();
    let Some(row) = collections.get::<SessionRow>(key) else {
        return div().into_any_element();
    };
    let open = key.to_owned();
    row::row(
        Listed { key, row },
        collections,
        selected,
        steps,
        menu,
        focus,
        move |_, cx| selection::choose(&open, cx),
        cx,
    )
}

/// What the list says about its count: the count beside the title, or, for a listing the helm's cap cut, a warning on
/// a line of its own instead, because the numbers alone would read as the whole list. Ported from the old app's
/// `count_banner` (its filter cases arrive with filters). A collection without counts (no helm sends one, but the type
/// allows it) counts its rows.
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
        (false, None) => CountLine::Count(counts.total.to_string()),
        (false, Some(matching)) => CountLine::Count(format!("{matching} of {}", counts.total)),
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

/// The list's title: the name of the order the helm stored, which is the order the channel started in. An order this
/// build does not know, or none, is most recent activity, as for the channel.
fn order_title(preferences: Option<&Preferences>) -> &'static str {
    match preferences.and_then(|preferences| preferences.list_sort.as_deref()) {
        Some("created") => "Newest created",
        Some("title") => "Title A–Z",
        _ => "Recently active",
    }
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

/// A full-width band of 12-pixel text, as the old `.banner` lines in the sidebar.
fn band(text: impl Into<SharedString>, fill: theme::Token, ink: theme::Token) -> impl IntoElement {
    div()
        .py(px(4.))
        .px(px(10.))
        .bg(theme::color(fill))
        .text_color(theme::color(ink))
        .text_size(metrics::TEXT_SM)
        .child(text.into())
}
