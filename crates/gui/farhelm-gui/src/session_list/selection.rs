//! The selection rules: which session is open, chosen by a click or automatically, and what choosing one tells the
//! helm.
//!
//! SPEC.md, "Session list" and "Status". Ported from the old app's `list/view.rs` (`automatic_selection`,
//! `newest_created_fallback`, `remember_selection`) and `session_view.rs` (the automatic seen mark). The rules are pure
//! functions of the pushed state and the current selection ([`settle`], [`seen_due`]), so they are tested without a
//! window; the session list applies them whenever the store, the selection or the connection changes.
//!
//! - **Something is always open while the list has rows.** With nothing selected, the list picks one: the most recent
//!   visit the helm recorded that is still listed, else (only while the helm has recorded no visits) the remembered
//!   last selection, else the newest-created session.
//! - **A selection stays until it is proven gone.** A selected session missing from the listing is dropped only when
//!   the listing proves it gone ([`session::listing_proves_gone`], the rule a confirmation about a session uses too); a
//!   truncated, filtered or unavailable listing keeps it, or a passing read failure would close what the user had open.
//!   When it is proven gone, the automatic choice picks the next one, never the one that just went.
//! - **Choosing tells the helm, once.** Every accepted choice, a click or automatic, is written as the `last_selected`
//!   preference, from which the helm also keeps its visit history. Choosing the session that is already open writes
//!   nothing.
//! - **Only the user's choice takes focus.** A click on a row ([`choose`]) hands the session and the click's mark to
//!   the pane, whose terminal for that session then takes focus, also when the session was already open (SPEC.md,
//!   "Where focus rests"). An automatic choice ([`open_automatically`]) does not: its terminal only becomes home, and
//!   takes focus only when nothing else holds it, so a session that went away under the user never pulls focus from a
//!   control they are using.
//! - **Opening marks the output seen.** While a session is open, its latest activity is recorded as seen whenever the
//!   session becomes open or its activity advances, unless it is stale (its host is not connected) or has ended (SPEC.md,
//!   "Status", as the old app did). The memory of what was stamped is keyed on the session, its activity and its
//!   eligibility, never on the stamp itself:
//!   that is what makes a manual mark-unread of the open session stick until new output arrives or the user opens
//!   another session. A listing that hides the open session leaves the memory alone ([`Open::Hidden`]). A failed mark is silent (SPEC.md, "Errors and diagnostics"): it costs a dot that is one visit
//!   behind, and the next mark corrects it.
//! - **A Delete holds the automatic choice back.** While any Delete the list sent is in flight, no session is opened
//!   automatically, and a selection proven gone is cleared rather than replaced ([`hold_for_deletes`]); the choice is
//!   made once every Delete has settled. SPEC.md: "Automatic selection after Delete waits until every admitted Delete
//!   has settled … and never opens a session that is still being deleted". A user's own click is not held back.
//! - **A Replace keeps its own source open until it answers.** The helm removes a Replace's source before it replies,
//!   and the reply is what opens the replacement in the source's place (SPEC.md: "A completed Replace opens its
//!   replacement only if the source is still selected"), so while the open session's own Replace is in flight its
//!   removal does not count as proof that it is gone (`SessionList::settle`). Any other Stop or Replace holds nothing
//!   back.
//!
//! NOTE: Not ported yet, because nothing in the app can cause it: the old app's refusal to stamp a retired
//! environment's session, which arrives with environment sessions.

use farhelm_helm_client::{ApiPath, Collections, Method};
use farhelm_proto::SessionSeenRequest;
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences, SessionRow};
use gpui::App;

use crate::focus::Mark;
use crate::local_state;
use crate::{session, session_pane};

/// One listed session: the key the helm files it under, and its row.
#[derive(Clone, Copy)]
pub(crate) struct Listed<'a> {
    pub(crate) key: &'a str,
    pub(crate) row: &'a SessionRow,
}

/// The sessions collection in the helm's order, with keys. A key the order names but no entry carries is skipped, as
/// [`Collections::ordered`] skips it.
pub(crate) fn listed(collections: &Collections) -> Vec<Listed<'_>> {
    let Some(meta) = collections.meta(farhelm_proto::ui_channel::Collection::Sessions) else {
        return Vec::new();
    };
    meta.order
        .iter()
        .filter_map(|key| {
            collections
                .get::<SessionRow>(key)
                .map(|row| Listed { key, row })
        })
        .collect()
}

/// What the selection should become.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    /// It is right as it is.
    Keep,
    /// Open this session (automatically).
    Select(String),
    /// Nothing is left to open.
    Clear,
}

/// Where the selection should be, given what is selected, what is listed, whether the listing proves the selected
/// session gone (`gone`, [`session::listing_proves_gone`]), and the helm's preferences (its visit history and
/// remembered last selection).
pub(crate) fn settle(
    selected: Option<&str>,
    listed: &[Listed],
    gone: bool,
    preferences: Option<&Preferences>,
) -> Change {
    let excluded = match selected {
        Some(key) if listed.iter().any(|session| session.key == key) => return Change::Keep,
        // Not listed, but not proven gone either: a selection is never dropped on a partial listing.
        Some(_) if !gone => return Change::Keep,
        Some(key) => Some(key),
        None => None,
    };
    match automatic(listed, preferences, excluded) {
        Some(key) => Change::Select(key.to_owned()),
        None if selected.is_some() => Change::Clear,
        None => Change::Keep,
    }
}

/// `change`, held back while `deleting` (any Delete the list sent is still in flight): an automatic choice becomes
/// [`Change::Clear`], so a selection proven gone is dropped with nothing opened in its place (and with nothing
/// selected, clearing changes nothing). The list settles again when the operation store changes, so the choice is made
/// once the last Delete has settled. The old app likewise dropped the deleted session at once and deferred its pick
/// while any delete was pending. Keeping and clearing pass through unchanged.
pub(crate) fn hold_for_deletes(change: Change, deleting: bool) -> Change {
    match change {
        Change::Select(_) if deleting => Change::Clear,
        change => change,
    }
}

/// The automatic choice among `listed`: history outranks the remembered last selection, which outranks creation time.
///
/// The visit history is authoritative once it holds anything, even when the remembered last selection differs (an
/// upgraded helm whose history starts empty still reopens what was open). Visits that are not listed are skipped; ended
/// and stale sessions count like any other. `excluded` (the session that just went) is never chosen.
pub(crate) fn automatic<'a>(
    listed: &[Listed<'a>],
    preferences: Option<&Preferences>,
    excluded: Option<&str>,
) -> Option<&'a str> {
    let history = preferences.map_or(&[][..], |preferences| &preferences.visit_history[..]);
    let last = preferences
        .and_then(|preferences| preferences.last_selected.as_deref())
        .filter(|_| history.is_empty());
    history
        .iter()
        .map(String::as_str)
        .chain(last)
        .filter(|key| Some(*key) != excluded)
        .find_map(|key| listed.iter().find(|session| session.key == key))
        .map(|session| session.key)
        .or_else(|| newest_created(listed, excluded))
}

/// The newest-created session, by `created_at` rather than position (the list may be in any order). A zero stamp is
/// unknown, not 1970, so such rows are no candidates; if no row has a stamp, the first listed row wins. Equal stamps
/// (one-second granularity makes them ordinary) keep the listing's order: the first of the newest wins.
pub(crate) fn newest_created<'a>(listed: &[Listed<'a>], excluded: Option<&str>) -> Option<&'a str> {
    let mut eligible = listed
        .iter()
        .filter(|session| Some(session.key) != excluded);
    eligible
        .clone()
        .filter(|session| session.row.info.created_at > 0)
        // `min_by` over the reversed comparison returns the FIRST maximum; `max_by` would return the last.
        .min_by(|a, b| b.row.info.created_at.cmp(&a.row.info.created_at))
        .or_else(|| eligible.next())
        .map(|session| session.key)
}

/// What the automatic seen mark last acted on: the open session, its activity, and whether it was eligible. The mark
/// is due whenever this changes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Stamped(Option<(String, i64, bool)>);

/// The open session as the automatic seen mark sees it.
#[derive(Clone, Copy)]
pub(crate) enum Open<'a> {
    /// No session is open.
    Nothing,
    /// The session filed under this key is open but the listing does not show it: the helm's cap cut it, or a filter
    /// (a chosen host, say) leaves it out, and the store then holds no row for it. Its activity is unknown, not gone.
    Hidden(&'a str),
    /// The open session, listed.
    Shown(Listed<'a>),
}

/// The seen mark the open session `open` is due, if any: its new memory, and the session and activity stamp to send
/// when one is due (`None` when the memory changed but nothing needs marking, which still has to be remembered).
/// Nothing is ever due for an [`Open::Hidden`] session, since its row is unknown. The memory is kept when it is that
/// session's own, so its reappearance with the same activity marks nothing; any other memory is forgotten, so a session
/// the user opened while it was hidden is marked once it shows.
///
/// NOTE: Treating every hidden session as nothing open, or every one as no change, would be wrong though either looks
/// simpler. Forgetting the open session's own memory makes its reappearance (the filter cleared, the cap no longer
/// cutting it) mark it seen again, undoing a manual Mark unread of a session the user never left. Keeping a memory of
/// another session means that leaving for a session the listing also hides (the quick switcher reaches any session)
/// and coming back is never noticed, so the return does not mark the session seen, as SPEC.md, "Status", asks.
pub(crate) fn seen_due(open: Open, stamped: &Stamped) -> Option<(Stamped, Option<(String, i64)>)> {
    let open = match open {
        Open::Hidden(key) => {
            let own = stamped
                .0
                .as_ref()
                .is_some_and(|(stamped, _, _)| stamped == key);
            let forgotten = Stamped::default();
            return (!own && *stamped != forgotten).then_some((forgotten, None));
        }
        Open::Nothing => None,
        Open::Shown(open) => Some(open),
    };
    let next = Stamped(open.map(|open| {
        let row = open.row;
        let eligible = !row.stale && !row.info.status.has_ended();
        (open.key.to_owned(), row.info.effective_activity(), eligible)
    }));
    if &next == stamped {
        return None;
    }
    let eligible = next.0.as_ref().is_some_and(|(_, _, eligible)| *eligible);
    let send = open
        .filter(|open| eligible && super::status::unseen(open.row))
        .map(|open| (open.key.to_owned(), open.row.info.effective_activity()));
    Some((next, send))
}

/// The helm's preferences row, if the snapshot carried it.
pub(crate) fn preferences(collections: &Collections) -> Option<&Preferences> {
    collections.get::<Preferences>(PREFERENCES_KEY)
}

/// Open `key` because the user chose it (a click): select it, remember it with the helm, and have its terminal take
/// focus. Choosing the open session again sends nothing, but still focuses its terminal.
///
/// Called from the click's handler, so the mark taken here is the click's own: the focus coordinator counts a press as
/// it goes down, before any handler runs.
pub(crate) fn choose(key: &str, cx: &mut App) {
    // Recorded before the selection changes, so the pane finds the choice whichever of the two it hears about first.
    session_pane::chosen(key, Mark::take(cx), cx);
    select(key, cx);
}

/// Open `key` because the selection rules chose it: select it and remember it with the helm, as for a click, but move
/// no focus; its terminal becomes home. Opening the open session again does nothing at all.
pub(crate) fn open_automatically(key: &str, cx: &mut App) {
    select(key, cx);
}

/// Select `key` and, if that changed the selection, remember it with the helm.
fn select(key: &str, cx: &mut App) {
    if local_state::select(key, cx) {
        remember(key, cx);
    }
}

/// Write `key` as the helm's `last_selected` preference, which also moves it to the front of the visit history.
pub(crate) fn remember(key: &str, cx: &mut App) {
    send(
        ApiPath::api(&["preferences"]),
        serde_json::json!({ "last_selected": key }),
        "remember the selected session",
        cx,
    );
}

/// Record `activity` as the activity the open session `key` was seen at, with the route's own request type, which the
/// manual toggle sends too ([`super::seen`]).
pub(crate) fn mark_seen(key: &str, activity: i64, cx: &mut App) {
    let request = SessionSeenRequest {
        seen_activity_at: Some(activity),
    };
    send(
        ApiPath::api(&["sessions", key, "seen"]),
        // An optional integer always encodes, so `json!` (which panics on a failed encode) cannot fail here.
        serde_json::json!(request),
        "mark the open session seen",
        cx,
    );
}

/// Send a `PUT` through the current UI session, whose reply nobody waits for. A failure is logged, never shown: both
/// writes here are best effort, and the next one corrects a lost one. A path that cannot be built (a key holding a
/// slash, which no helm serves) is logged the same way.
fn send(
    path: Result<ApiPath, farhelm_helm_client::PathError>,
    body: serde_json::Value,
    what: &'static str,
    cx: &mut App,
) {
    send_then(path, body, what, cx, |_| {});
}

/// [`send`], then `then` once the helm has replied, whatever it replied (success, refusal, or a command the channel
/// lost). Nothing is sent, and `then` never runs, when the path cannot be built or the window is inert. `then` does not
/// run either when a reset retires the UI session first. The sidebar's width writes go through here, keeping their
/// width on screen until `then` (`crate::sidebar`).
pub(crate) fn send_then(
    path: Result<ApiPath, farhelm_helm_client::PathError>,
    body: serde_json::Value,
    what: &'static str,
    cx: &mut App,
    then: impl FnOnce(&mut App) + 'static,
) {
    let path = match path {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(%error, "could not {what}");
            return;
        }
    };
    let Some(current) = session::connection(cx).and_then(|c| c.read(cx).session().cloned()) else {
        return;
    };
    current.update(cx, |current, cx| {
        current.command(Method::Put, path, Some(body), cx, move |_, result, cx| {
            match result {
                Ok(reply) if (200..300).contains(&reply.status) => {}
                Ok(reply) => tracing::warn!(status = reply.status, "could not {what}"),
                Err(error) => tracing::debug!(?error, "could not {what}"),
            }
            then(cx);
        });
    });
}
