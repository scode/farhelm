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
//! The list is drawn uncached (the maintainer's foundations decision of 2026-11-14): SPEC's fleet is tens of sessions,
//! so a whole-list redraw per change is cheap, and caching is settled when profiling at the listing cap asks for it.
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
//! NOTE: Not ported yet, each with the area that brings it: the order menu (the title is the stored order's name, not a
//! control), filters and their counts, compact rows, marking, pins, the row menu, and the host rail.

mod row;
mod selection;
mod status;

#[cfg(test)]
pub(crate) mod tests;

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::{Availability, Collection, CollectionMeta, Preferences};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, Role,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};

use crate::host_text::{HostSentence, HostText};
use crate::local_state::{self, LocalState};
use crate::render_count;
use crate::session::{self, PerSession};
use crate::theme::{self, metrics};
use crate::ticker;
use selection::{Change, Stamped};

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
        };
        list.settle(cx);
        list
    }

    /// Apply the selection rules to the state as it is now: select (and remember) or clear, and send the open
    /// session's seen mark when one is due. Nothing happens while the window is inert.
    fn settle(&mut self, cx: &mut Context<Self>) {
        let current = self.current.current(cx);
        let Some(store) = session::current_store(cx) else {
            return;
        };
        if current.store.is_none() {
            current.store = Some(cx.observe(&store, |list, _, cx| list.settle(cx)));
        }
        let collections = store.read(cx).collections();
        let listed = selection::listed(collections);
        let selected = local_state::selected(cx);
        let gone = selected
            .as_deref()
            .is_some_and(|key| session::listing_proves_gone(collections, key));
        let change = selection::settle(
            selected.as_deref(),
            &listed,
            gone,
            selection::preferences(collections),
        );
        // The seen mark is for whichever session is open once the change is made.
        let open = match &change {
            Change::Keep => selected,
            Change::Select(key) => Some(key.clone()),
            Change::Clear => None,
        };
        let open = open.and_then(|key| listed.iter().find(|session| session.key == key).copied());
        let seen = selection::seen_due(open, &current.stamped);

        match change {
            Change::Keep => {}
            Change::Select(key) => selection::choose(&key, cx),
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
}

impl Render for SessionList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("session-list", cx);

        let list = div()
            .id("session-list")
            .role(Role::List)
            .aria_label("Sessions")
            .size_full()
            .flex()
            .flex_col()
            .min_h_0();
        let Some(store) = session::current_store(cx) else {
            self.pulse = None;
            return list;
        };
        let pulses = ticker::is_active(window, cx)
            && selection::listed(store.read(cx).collections())
                .iter()
                .any(|session| status::pulses(session.row));
        if pulses != self.pulse.is_some() {
            self.pulse = pulses.then(|| ticker::pulse(window, cx));
        }
        let pulse_step = self.pulse.as_ref().map_or(0, |pulse| pulse.step(cx));
        let collections = store.read(cx).collections();
        let listed = selection::listed(collections);
        let Some(meta) = collections.meta(Collection::Sessions) else {
            return list.child(status_line("Loading sessions…"));
        };
        let selected = local_state::selected(cx);
        let palette = theme::palette(cx);

        let count = count_line(meta, listed.len());
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
            Availability::Unavailable { error } => Some(
                div()
                    .py(px(4.))
                    .px(px(10.))
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.danger))
                    .child(
                        HostSentence::new("session-list-error")
                            .text("Failed to load sessions: ")
                            .host(&HostText::from_host(error.as_str())),
                    ),
            ),
        };

        let body: AnyElement = if empty_fleet(collections) {
            status_line("No sessions").into_any_element()
        } else {
            div()
                .id("session-rows")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .children(listed.iter().map(|session| {
                    let key = session.key.to_owned();
                    row::row(
                        *session,
                        collections,
                        selected.as_deref() == Some(session.key),
                        pulse_step,
                        move |_, cx| selection::choose(&key, cx),
                        cx,
                    )
                }))
                .into_any_element()
        };
        list.child(heading)
            .children(truncated)
            .children(unavailable)
            .child(body)
    }
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
