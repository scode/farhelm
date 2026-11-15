//! The list's query: which sessions the helm serves the list, narrowed by host on the rail and by status in the
//! segments under the list's heading, and the three places the list shows the choice (the segments, the removable host
//! chip beside them, and the sentence that names the filters of an empty answer).
//!
//! SPEC.md, "Session list" (the status segments, the host chip, the empty filtered list) and "Desktop window chrome"
//! (the quick switcher's reset of the filters). Ported from the old app's `list/view.rs` (`SessionFilter`,
//! `apply_status`, `choose_host`, `FilterHostLabel`, the empty result, `switcher_pick_hidden`), `rows.rs`
//! (`no_match_line`) and the stylesheet's `.session-status-filter`, `.host-filter-chip` and `.filter-empty` rules at
//! the reference commit.
//!
//! **One query, two dimensions.** Choosing is a query, not a filter the window applies: the rail's host, the segments'
//! status and the switcher's reset all send the UI channel's session query (`PUT /ui/session-query`), and the helm then
//! serves only the matching sessions with their counts. The helm replaces the whole query on every send, so the host
//! and the status are one [`Query`] and every send carries both: a host chosen while a status is in force keeps the
//! status, and the other way round. The query never names a sort, so the channel keeps following the stored order
//! (SPEC_impl.md, "The UI channel"). Nothing is persisted, and a choice equal to the current one sends nothing.
//!
//! **The query names the open session.** Every send also carries the open session's key as `selected`, and the list
//! sends the chosen query again whenever the open session changes ([`SessionList::name_open_session`]), so the helm
//! always knows which session to keep in the pushed collection when the filters or its listing cap leave it out
//! (SPEC_impl.md, "What is pushed, and how changes are found"). That carried session is outside the collection's order,
//! so the list never draws it and its counts stay the query's own, while the pane, its header, its details bar and
//! Rename find it by key. A re-send for a new open session carries the choice in force and leaves the line a refused
//! query left, since the user changed no filter; only a new choice of the filters takes the line away. A refused change
//! of the filters also puts back the open session the accepted query named, so the session open now is named again
//! under the filters the helm serves ([`Choice`]), while the refusal's words stay shown.
//!
//! **Two moments.** What the window shows follows two different moments, as in the old app ([`Choice`]): the rail's
//! pressed entry, the segments and the chip follow the user's choice at once, while the empty sentence names the query
//! the helm has answered, so a held reply leaves the sentence describing the listing still on screen. A refused query
//! puts the previous choice back, both dimensions of it, and shows the helm's words under the heading until the next
//! choice.
//!
//! **What filtering leaves alone.** Hiding the open session changes neither the selection nor its terminal: a filtered
//! listing proves nothing about the sessions it leaves out (`session::listing_proves_gone`), and the helm keeps pushing
//! the hidden open session's current state, so the pane goes on naming it. The one thing a filtered listing does prove
//! is that the open session itself is gone, once the query it answers named that session and the listing is not cut
//! ([`Choice::serves_named`]).
//! Ended sessions show only under All, which is the helm's doing: its status filter matches the status word exactly, so
//! Idle never lists an exited session.
//!
//! **Removing a filter.** The chip and the empty sentence's links ("Show all hosts", "Show all statuses") take away the
//! control the user just used, so focus moves to the rail's All, as in the old app (SPEC.md: "After a clear button or
//! the chip removes its focused control, focus returns to the rail's All button"), through the focus coordinator's move
//! for a user's action (`focus::focus_control`, made from the action itself), which does nothing once the user has
//! moved on or while a layer is open. Show all statuses keeps the host, and Show all hosts the status.
//!
//! **The quick switcher's pick.** A pick the filters hide resets both to All once the session is open, so the user sees
//! what they opened ([`SessionList::picked_from_switcher`]); one they show leaves them alone. Whether the filters hide
//! it is decided as the old `switcher_pick_hidden` decided it ([`pick_hidden`]).
//!
//! The choice belongs to the UI session (`super::Current`): a reset forgets it, as the helm's new channel starts
//! unfiltered (SPEC_impl.md, "One failure path: the reset").
//!
//! NOTE: The old count line's unsupported-filter wording (" — this helm does not support filtering, so the filter was
//! ignored") is not ported: it answered an old helm that ignored a filter, and the channel's helm always applies its
//! query or refuses it, so the listing carries nothing that could call for it.

#[cfg(test)]
mod tests;

use farhelm_helm_client::{ApiPath, Collections, Method};
use farhelm_proto::SessionStatus;
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionMeta, SessionQuery, SessionRow,
};
use gpui::{
    AnyElement, App, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, WeakEntity, Window, div, px,
};

use super::SessionList;
use super::host_rail::{CHIP_MARK, identity, registered, unregistered};
use super::row::short_name;
use super::selection;
use crate::components::{Button, HoverHelp, RadioGroup, RadioOption, Role, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::hosts::identity::host_mark;
use crate::theme::{self, Palette, metrics};
use crate::{local_state, operations, session};

/// `.host-filter-chip`'s gap between its mark, the host's name and the ✕.
const CHIP_GAP: f32 = 5.;

/// `.session-status-filter`'s padding (vertical then horizontal) and gap: the row under the heading that holds the
/// segments and the chip.
const ROW_PAD: (f32, f32) = (6., 8.);
const ROW_GAP: f32 = 6.;

/// The empty sentence's line height: fixed, so the mark can be centered on the first line of a wrapped name.
const EMPTY_LINE: f32 = 20.;

/// `.filter-empty-actions`'s gaps, vertical then horizontal, between the links that clear a filter.
const EMPTY_ACTIONS_GAP: (f32, f32) = (8., 12.);

/// The sentence an empty answer gets when the helm's cap cut the listing: it can claim only what the helm could read.
pub(crate) const CUT_SENTENCE: &str =
    "No matches among the sessions that could be read — the list could not be read to the end";

// ---- The query -----------------------------------------------------------------------------------------------------

/// The status segment chosen: All, or one of the three live statuses the old segments offered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Status {
    #[default]
    All,
    Running,
    Waiting,
    Idle,
}

/// What a status segment says and asks for, one row per [`Status`] in its order.
struct Segment {
    /// The word the helm's query takes (the `?status=` vocabulary), `None` for All, which asks for no status.
    word: Option<&'static str>,
    /// The segment's words, also its accessible name and, after the group's id, its element id.
    label: &'static str,
    /// The segment's hover help, the old app's words.
    help: &'static str,
    /// How the empty sentence starts under this status, before any host and the closing period.
    lead: &'static str,
}

/// The segments' rows, indexed by [`Status`] as declared.
const ROWS: [Segment; 4] = [
    Segment {
        word: None,
        label: "All",
        help: "Show every session, including ended sessions",
        lead: "No sessions",
    },
    Segment {
        word: Some("running"),
        label: "Running",
        help: "Show sessions whose agents are working",
        lead: "No running sessions",
    },
    Segment {
        word: Some("waiting"),
        label: "Waiting",
        help: "Show sessions that need your attention",
        lead: "No waiting sessions",
    },
    Segment {
        word: Some("idle"),
        label: "Idle",
        help: "Show live sessions whose agents are at rest",
        lead: "No idle sessions",
    },
];

impl Status {
    /// The segments, in the order they are drawn.
    pub(crate) const SEGMENTS: [Status; 4] =
        [Status::All, Status::Running, Status::Waiting, Status::Idle];

    /// This status's row of [`ROWS`].
    fn segment(self) -> &'static Segment {
        &ROWS[self as usize]
    }

    /// Whether the helm lists a session in `status` under this choice: every session under All, else only an exact
    /// match, so an ended or unclassified session shows under All alone.
    fn shows(self, status: &SessionStatus) -> bool {
        match self {
            Status::All => true,
            Status::Running => matches!(status, SessionStatus::Running),
            Status::Waiting => matches!(status, SessionStatus::Waiting),
            Status::Idle => matches!(status, SessionStatus::Idle),
        }
    }
}

/// One session query: the host (`None` for All) and the status. The default is the unfiltered query a new channel
/// starts with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Query {
    pub(crate) host: Option<i64>,
    pub(crate) status: Status,
}

impl Query {
    /// Whether the query leaves any session out.
    fn narrows(self) -> bool {
        self.host.is_some() || self.status != Status::All
    }

    /// The body the channel's query command carries: both dimensions and `selected`, the key of the open session (none
    /// with nothing open), never a sort (see the module docs).
    fn body(self, selected: Option<&str>) -> SessionQuery {
        SessionQuery {
            host: self.host,
            status: self.status.segment().word.map(str::to_owned),
            selected: selected.map(str::to_owned),
            ..SessionQuery::default()
        }
    }
}

/// What the list's query is in one UI session: the query the user chose and the query the helm last accepted, which
/// differ while a query is in flight, the open session the last send named, and the helm's words for a refused one.
///
/// Replies arrive in the order the queries were sent (the channel admits one command at a time), and each query
/// replaces the whole previous one on the helm's side, so the helm serves the last query it accepted: that is
/// `answered`. A refusal of the latest query puts the choice back to `answered`; a refusal of a query a later one
/// already superseded changes nothing, since the user has moved on from it and the later one is still on its way.
///
/// The open session is part of what was sent but not of the choice: [`Query`] compares only the filters, so the
/// switcher's pick and the empty sentence never see a difference that only the open session makes.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Choice {
    /// The query the user chose: what the rail presses, the segments check and the chip names.
    chosen: Query,
    /// The open session's key as the last send named it (`None` before any send and for a send with nothing open).
    ///
    /// NOTE: A refusal of a send that changed the filters puts back the key the accepted query named
    /// (`answered_named`), since the helm still serves that query and so still carries that session. The refusal's
    /// handler then sends the accepted filters once more, naming the session open now (the quick switcher's refused
    /// widening of a hidden pick is the case). A refusal of a send that only named another session puts nothing back:
    /// its filters are already the accepted ones, so sending them again could only be refused again.
    named: Option<String>,
    /// The query the helm last accepted (unfiltered, as a new channel starts): what the listing on screen answers, and
    /// so what its empty sentence names.
    ///
    /// NOTE: It moves when the reply arrives, which the helm sends right after the listing it pushed for the query.
    /// Between the two, possibly for one frame, a new empty listing can be drawn naming the previous query. The
    /// listing's metadata does not say which query it answers, so nothing closer is known.
    answered: Query,
    /// The open session's key as the query the helm last accepted named it.
    answered_named: Option<String>,
    /// How many queries this UI session sent; the last one's number.
    sent: u64,
    /// The number of the last query the helm answered, accepted or refused: `sent` once no query is in flight, since
    /// replies arrive in the order the queries were sent.
    replied: u64,
    /// The number of the latest send that changed the filters (0 before any): the send whose answer brings the listing
    /// of the view the user chose ([`Choice::listing_view`]).
    view_sent: u64,
    /// The helm's words for the latest query, when it refused it.
    refusal: Option<HostText>,
}

impl Choice {
    /// The query the user chose.
    pub(crate) fn chosen(&self) -> Query {
        self.chosen
    }

    /// The query the helm last accepted.
    pub(crate) fn answered(&self) -> Query {
        self.answered
    }

    /// The helm's words for the latest query, if it refused it.
    pub(crate) fn refusal(&self) -> Option<&HostText> {
        self.refusal.as_ref()
    }

    /// `query` is to be sent with `selected` as the open session: take both and return the number to send them under,
    /// or `None` when neither changed since the last send, which sends nothing. A send that changes the filters drops
    /// the last refusal's words; one that only names another open session keeps them, or the re-send that follows a
    /// refused widening (`named`'s NOTE) would erase the words of the refusal it follows.
    pub(crate) fn choose(&mut self, query: Query, selected: Option<&str>) -> Option<u64> {
        if self.chosen == query && self.named.as_deref() == selected {
            return None;
        }
        if self.chosen != query {
            self.refusal = None;
            self.view_sent = self.sent + 1;
        }
        self.chosen = query;
        self.named = selected.map(str::to_owned);
        self.sent += 1;
        Some(self.sent)
    }

    /// Whether the helm now serves a query that names `key` as the open session, with no later query on its way. Then
    /// every listing it pushes holds `key`, listed or carried, for as long as that session exists (SPEC_impl.md, "What
    /// is pushed, and how changes are found"), so an available listing without it shows the session gone, filtered or
    /// not (SPEC.md's exception to "Existing removal detection", decided by the maintainer on 2026-11-15). A cut
    /// listing still proves nothing (`selection::absent_while_named`'s NOTE).
    ///
    /// NOTE: Both halves are needed. The query last sent is not enough: until the helm accepts a query naming a session
    /// the user just opened (a quick-switcher pick the filters hide, say), its listings need not hold that session, and
    /// treating its absence as proof would undo the pick. The query last accepted is not enough either while a later one
    /// is in flight: the helm pushes a query's listing before replying to it, so a later query naming another session
    /// can leave `key` out although it exists. That takes a quick return: with `key` listed, the user opens another
    /// session and then `key` again before the helm answers the first of those, while `key` stops matching the
    /// filters (`session_pane/carried_tests.rs`, `a_quick_return_to_a_session_is_not_undone_by_the_query_it_overtook`).
    pub(crate) fn serves_named(&self, key: &str) -> bool {
        self.replied == self.sent && self.answered_named.as_deref() == Some(key)
    }

    /// Which view the listing in the store answers, for the status sounds, which record a changed view's statuses
    /// silently (SPEC.md, "Status": "changing that view silently records its existing statuses"): `None` while a query
    /// that changed the filters is unanswered, and otherwise a number that changes exactly when such a query is
    /// answered.
    ///
    /// The helm pushes a new query's listing before it replies to the query, so a listing that arrives while this is
    /// `None` may already be the new view's, and one change may hold both the old view's rows and the new one's; nothing
    /// in the listing itself says which query it answers ([`Choice::answered`]'s NOTE). Once the answer is here, the
    /// store holds the view the number names. A send that only names another open session leaves the number alone:
    /// it changes which session the helm carries outside the list, never which sessions the list shows.
    pub(crate) fn listing_view(&self) -> Option<u64> {
        (self.replied >= self.view_sent).then_some(self.view_sent)
    }

    /// The helm answered query number `number`, which asked for `query` naming `selected` as the open session: accepted
    /// (`Ok`), or refused in its words.
    pub(crate) fn settle(
        &mut self,
        number: u64,
        query: Query,
        selected: Option<&str>,
        answer: Result<(), HostText>,
    ) {
        self.replied = number;
        match answer {
            Ok(()) => {
                self.answered = query;
                self.answered_named = selected.map(str::to_owned);
            }
            Err(words) if number == self.sent => {
                // `named`'s NOTE: only a refused change of the filters puts the accepted naming back.
                if self.chosen != self.answered {
                    self.named = self.answered_named.clone();
                }
                self.chosen = self.answered;
                self.refusal = Some(words);
            }
            Err(_) => {}
        }
    }
}

impl SessionList {
    /// The user chose `host` (`None` for All) on the rail, the chip or "Show all hosts": press it at once and send the
    /// query for it with the status in force, unless it is the choice already. Any choice also returns from the Hosts
    /// panel to Sessions. Nothing happens while the window is inert.
    pub(crate) fn choose_host(&mut self, host: Option<i64>, cx: &mut Context<Self>) {
        if !connected(cx) {
            return;
        }
        // SPEC.md: "All or a host avatar returns to Sessions". The old app closed the Hosts panel on every choice, the
        // choice already in force included, so this comes before the check that a repeated choice sends nothing.
        self.hide_hosts(cx);
        let query = Query {
            host,
            ..self.current.current(cx).filters.chosen()
        };
        self.send_query(query, cx);
    }

    /// The user chose `status` in the segments or with "Show all statuses": check it at once and send the query for it
    /// with the host in force, unless it is the choice already.
    pub(crate) fn choose_status(&mut self, status: Status, cx: &mut Context<Self>) {
        let query = Query {
            status,
            ..self.current.current(cx).filters.chosen()
        };
        self.send_query(query, cx);
    }

    /// The quick switcher opened `row`'s session (SPEC.md: "Opening a session hidden by the host choice or status
    /// filter resets both to All after the open is accepted"): return from the Hosts panel to Sessions, and widen the
    /// query to every session if the filters hide it ([`pick_hidden`]), so the open session is listed. A pick the
    /// filters show leaves them in place.
    ///
    /// NOTE: Called once the pick has opened the session. Opening a pick is never refused in this app, as at the
    /// reference commit, where the old `guarded_open` always accepted; SPEC.md's "a refused pick keeps both choices"
    /// therefore has no case here. A widening query the helm refuses keeps both choices the way any refused query
    /// does ([`Choice::settle`]).
    pub(crate) fn picked_from_switcher(&mut self, row: &SessionRow, cx: &mut Context<Self>) {
        self.hide_hosts(cx);
        let Some(store) = session::current_store(cx) else {
            return;
        };
        let hidden = pick_hidden(
            &self.current.current(cx).filters,
            store.read(cx).collections(),
            row,
        );
        if hidden {
            self.send_query(Query::default(), cx);
        }
    }

    /// The open session may have changed (the list's settle calls this after every selection change it applies or
    /// observes, and after a reset; a query's reply calls it after a refusal): send the query in force again if the
    /// last send named another open session, or none, so the helm carries the session now open. Nothing is sent when
    /// the last send already named it.
    ///
    /// A reset's new channel starts unfiltered and names no session, so with a session open the first settle under
    /// it sends the unfiltered query naming that session.
    pub(super) fn name_open_session(&mut self, cx: &mut Context<Self>) {
        let query = self.current.current(cx).filters.chosen();
        self.send_query(query, cx);
    }

    /// Take `query` as the choice and send it on the UI channel, naming the open session, unless neither the choice nor
    /// the open session changed since the last send; its reply settles the choice ([`Choice`]). Nothing happens while
    /// the window is inert.
    fn send_query(&mut self, query: Query, cx: &mut Context<Self>) {
        if !connected(cx) {
            return;
        }
        let selected = local_state::selected(cx);
        // Encoded before the choice is taken: a query that could not be sent must not leave a choice shown that no
        // reply will ever settle.
        let body = match serde_json::to_value(query.body(selected.as_deref())) {
            Ok(body) => body,
            Err(error) => {
                tracing::warn!(%error, "could not encode the session query");
                return;
            }
        };
        let Some(number) = self
            .current
            .current(cx)
            .filters
            .choose(query, selected.as_deref())
        else {
            return;
        };
        cx.notify();
        let list = cx.weak_entity();
        // No busy state and no refusal kept on a key: the choice itself is what the window shows meanwhile, and the
        // refusal line is the list's own. A command the channel lost would reach `then` as an error too and be shown
        // like a refusal, though that cannot arrive in practice: the reset that loses it, which forgets the choice with
        // its UI session, drops the wait first.
        operations::send(
            Method::Put,
            ApiPath::session_query(),
            Some(body),
            "PUT /ui/session-query",
            cx,
            move |answer, cx| {
                let _ = list.update(cx, |list, cx| {
                    list.current.current(cx).filters.settle(
                        number,
                        query,
                        selected.as_deref(),
                        answer.map(|_| ()),
                    );
                    // Settle the whole list, not only its naming: the query in force is evidence the selection
                    // rules read (`Choice::serves_named`), and a push that left the open session out may have
                    // arrived while this reply was on its way. The settle also corrects the naming at once when a
                    // refused change of the filters put the accepted one back (`Choice::named`'s NOTE), rather than
                    // at the next pushed change, which may be a long way off. Otherwise it sends nothing.
                    list.settle(cx);
                    cx.notify();
                });
            },
        );
    }

    /// The user removed a filter from a control that goes with it (the chip, a link under the empty sentence): choose
    /// All for that dimension, keeping the other, and move focus to the rail's All in place of the control that is
    /// going away. `mark` is the one taken in the control's handler.
    fn clear(
        &mut self,
        dimension: Dimension,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match dimension {
            Dimension::Host => self.choose_host(None, cx),
            Dimension::Status => self.choose_status(Status::All, cx),
        }
        focus::focus_control(&self.all_focus, mark, window, cx);
    }
}

/// The two dimensions of a [`Query`], for the controls that clear one.
#[derive(Clone, Copy)]
enum Dimension {
    Host,
    Status,
}

/// Whether the window has a UI session to send a query on; without one it is inert.
fn connected(cx: &App) -> bool {
    session::connection(cx).is_some_and(|connection| connection.read(cx).session().is_some())
}

/// A handler for a control that removes the `dimension` filter and goes away with it ([`SessionList::clear`]).
fn clear_from(
    list: &WeakEntity<SessionList>,
    dimension: Dimension,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
    let list = list.clone();
    move |_, window, cx| {
        // Taken in the handler, after the coordinator counted this press or key: the move is this action's own.
        let mark = Mark::take(cx);
        let _ = list.update(cx, |list, cx| list.clear(dimension, &mark, window, cx));
    }
}

/// Whether the filters `choice` holds hide the session the quick switcher picked, `row` as the switcher's own read
/// lists it: the old `switcher_pick_hidden`, a navigation decision rather than a second filter.
///
/// The list's listing decides when it answers the query in force (the helm accepted the chosen query) and its read
/// succeeded: the session in it is shown, and absent from a complete one it is hidden. Otherwise (a query in flight, a
/// failed read, or a cut listing without it) the row decides: a chosen host other than the row's hides it, and so does
/// a chosen status the row's status is not, which every ended or unclassified status is.
pub(crate) fn pick_hidden(choice: &Choice, collections: &Collections, row: &SessionRow) -> bool {
    let chosen = choice.chosen();
    if !chosen.narrows() {
        return false;
    }
    if choice.answered() == chosen
        && let Some(meta) = collections.meta(Collection::Sessions)
        && meta.availability == Availability::Available
    {
        if selection::listed(collections)
            .iter()
            .any(|session| session.key == row.info.id)
        {
            return false;
        }
        if meta.counts.is_none_or(|counts| !counts.truncated) {
            return true;
        }
    }
    chosen.host.is_some_and(|host| row.host != host) || !chosen.status.shows(&row.info.status)
}

/// The sentence for a refused query: the words [`operations::send`] hands over (the helm's, which may quote a host, in
/// the store's shared refusal wording, or why the command was lost), after the app's own. One lead for every refused
/// query, whichever dimension it changed (decided by the migration's orchestrator on 2026-11-14): the refusal puts back
/// the last answered query, which can undo more than the latest change, and neither SPEC.md nor the old app worded a
/// refused query.
pub(crate) fn refusal_sentence(words: &HostText) -> HostSentence {
    HostSentence::new("filter-refusal")
        .text("Could not show those sessions: ")
        .host(words)
}

// ---- The segments row ----------------------------------------------------------------------------------------------

/// The row under the list's heading (`.session-status-filter`): the status segments, named "Session status" and drawn
/// as the status filter's small segments, then, while a host is chosen, the chip that removes the host. The row wraps,
/// so the chip goes below the segments when the column is too narrow for both. It is drawn whatever the listing holds,
/// also before the first one arrives, as the old app kept it usable while a read was pending or failed.
pub(crate) fn row(
    collections: &Collections,
    chosen: Query,
    list: &WeakEntity<SessionList>,
    palette: &Palette,
) -> impl IntoElement {
    let choose = list.clone();
    let segments = Status::SEGMENTS.iter().fold(
        // The old group had no hover text of its own (only its segments did); every control's constructor takes one
        // here, and it shows only over the well's gaps and edge.
        RadioGroup::new(
            "session-status",
            "Session status",
            HoverHelp::tooltip("Session status: show only the sessions in one status"),
            Role::RadioGroup,
        )
        .small_segments()
        .chosen(Some(chosen.status))
        .on_change(move |status: &Status, _, cx| {
            let status = *status;
            let _ = choose.update(cx, |list, cx| list.choose_status(status, cx));
        }),
        |group, status| {
            group.option(RadioOption::new(
                status.segment().label,
                HoverHelp::tooltip(status.segment().help),
                *status,
            ))
        },
    );
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(ROW_GAP))
        .py(px(ROW_PAD.0))
        .px(px(ROW_PAD.1))
        .min_w_0()
        .child(segments)
        .children(
            chosen
                .host
                .map(|host| chip(collections, host, list, palette)),
        )
}

/// The chip that removes the host choice `chosen`: the host's mark, its short name (cut short when the row is narrow)
/// and a ✕, named "Filtered to host <name>; remove filter" with the host's identity as its help; for a host the
/// registry no longer lists, "?" and "Host <id> (no longer registered)". Pressing it chooses All for the host, keeping
/// the status.
fn chip(
    collections: &Collections,
    chosen: i64,
    list: &WeakEntity<SessionList>,
    palette: &Palette,
) -> AnyElement {
    let (label, help, mark, name): (SharedString, SharedString, AnyElement, HostText) =
        match registered(collections, chosen) {
            Some(host) => {
                let name = HostText::from_host(short_name(host));
                let label = HostSentence::new("host-filter-chip-label")
                    .text("Filtered to host ")
                    .host(&name)
                    .text("; remove filter")
                    .accessible_name();
                let help = identity(host);
                let mark =
                    host_mark(host.icon, host.color, px(CHIP_MARK), palette).into_any_element();
                (label, help, mark, name)
            }
            None => {
                let name = unregistered(chosen);
                (
                    format!("Filtered to host {name}; remove filter").into(),
                    name.clone().into(),
                    div().flex_none().child("?").into_any_element(),
                    // The app's own words, drawn through the host-text element only to share the name's layout.
                    HostText::from_host(name),
                )
            }
        };
    Button::new(
        "host-filter-chip",
        label,
        HoverHelp::tooltip(help),
        Role::Button,
    )
    .tier(Tier::Chip)
    .content(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(CHIP_GAP))
            .min_w_0()
            .child(mark)
            .child(host_text::element(&name).min_w_0().truncate())
            .child(div().flex_none().child("✕")),
    )
    .on_click(clear_from(list, Dimension::Host))
    .into_any_element()
}

// ---- The empty answer ----------------------------------------------------------------------------------------------

/// What the list says in place of rows when a filtered query matched nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Empty {
    /// The complete answer to this query matched nothing: the sentence names its filters, with a link per filter.
    Filtered(Query),
    /// The helm's cap cut the answer, so it can say only that nothing matched among what was read ([`CUT_SENTENCE`]).
    Cut,
}

/// What the list should say instead of drawing no rows, if anything: an available listing of `shown` rows (none) that
/// answers `answered`, a query that narrows, and matched nothing. Under All an empty listing is the empty fleet's
/// ("No sessions", the list's own), and a failed read keeps its failure line.
pub(crate) fn empty(meta: &CollectionMeta, shown: usize, answered: Query) -> Option<Empty> {
    let counts = meta.counts?;
    let matched_nothing = meta.availability == Availability::Available
        && shown == 0
        && answered.narrows()
        && counts.matching == Some(0);
    matched_nothing.then_some(if counts.truncated {
        Empty::Cut
    } else {
        Empty::Filtered(answered)
    })
}

/// The empty answer `empty` in the old `.status.filter-empty` line's size and quiet ink, as one labelled node for
/// assistive technology. A complete answer's sentence is its status's lead, " on " and the answered host's mark and
/// short name when a host was asked for, and a period; under it, a link for each filter it names, Show all hosts first
/// and Show all statuses second. A cut answer's sentence has no links, since it does not claim what the filters left
/// out.
///
/// The host's name is its own text element, kept together with the period that ends the sentence. A long name wraps
/// within its own element: the words before it and its mark stay on its first line, and the period sits on its last,
/// as the old inline sentence had them.
pub(crate) fn empty_state(
    collections: &Collections,
    empty: Empty,
    list: &WeakEntity<SessionList>,
    palette: &Palette,
) -> impl IntoElement {
    let sentence = div()
        .id("filter-empty")
        .role(Role::Label)
        .p(metrics::LIST_STATUS_PAD)
        .pb(px(8.))
        .text_size(metrics::TEXT_ROW_TITLE)
        .line_height(px(EMPTY_LINE))
        .text_color(theme::color(palette.fg_1));
    let query = match empty {
        Empty::Cut => {
            return div()
                .flex()
                .flex_col()
                .child(sentence.aria_label(CUT_SENTENCE).child(CUT_SENTENCE));
        }
        Empty::Filtered(query) => query,
    };
    let lead = query.status.segment().lead;
    let sentence = match query.host {
        None => {
            let words = format!("{lead}.");
            sentence.aria_label(words.clone()).child(words)
        }
        Some(host) => {
            let (mark, name) = match registered(collections, host) {
                Some(view) => (
                    host_mark(view.icon, view.color, px(CHIP_MARK), palette).into_any_element(),
                    HostText::from_host(short_name(view)),
                ),
                None => (
                    div().flex_none().child("?").into_any_element(),
                    HostText::from_host(unregistered(host)),
                ),
            };
            let label = HostSentence::new("filter-empty-label")
                .text(format!("{lead} on "))
                .host(&name)
                .text(".")
                .accessible_name();
            sentence
                .aria_label(label)
                .flex()
                .flex_row()
                .flex_wrap()
                .items_start()
                .gap_x(px(4.))
                .child(div().child(format!("{lead} on")))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .min_w_0()
                        // The mark sits centered on the name's first line, whatever the name's height.
                        .child(
                            div()
                                .flex_none()
                                .h(px(EMPTY_LINE))
                                .flex()
                                .items_center()
                                .child(mark),
                        )
                        .child(host_text::element(&name).min_w_0().ml(px(4.)))
                        .child(div().flex_none().self_end().child(".")),
                )
        }
    };
    let show_all_hosts = query.host.map(|_| {
        Button::new(
            "filter-show-all-hosts",
            "Show all hosts",
            HoverHelp::tooltip("Show all hosts: remove the host filter"),
            Role::Button,
        )
        .tier(Tier::Link)
        .on_click(clear_from(list, Dimension::Host))
    });
    let show_all_statuses = (query.status != Status::All).then(|| {
        Button::new(
            "filter-show-all-statuses",
            "Show all statuses",
            HoverHelp::tooltip("Show all statuses: remove the status filter"),
            Role::Button,
        )
        .tier(Tier::Link)
        .on_click(clear_from(list, Dimension::Status))
    });
    div().flex().flex_col().child(sentence).child(
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap_y(px(EMPTY_ACTIONS_GAP.0))
            .gap_x(px(EMPTY_ACTIONS_GAP.1))
            .px(metrics::LIST_STATUS_PAD)
            .pb(metrics::LIST_STATUS_PAD)
            .children(show_all_hosts)
            .children(show_all_statuses),
    )
}
