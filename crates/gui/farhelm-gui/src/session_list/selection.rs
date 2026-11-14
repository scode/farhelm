//! The selection rules: which session is open, chosen by a click or automatically, and what choosing one tells the helm.
//!
//! SPEC.md, "Session list" and "Status". Ported from the old app's `list/view.rs` (`automatic_selection`,
//! `newest_created_fallback`, `remember_selection`) and `session_view.rs` (the automatic seen mark). The rules are pure
//! functions of the pushed state and the current selection ([`settle`], [`seen_due`]), so they are tested without a
//! window; the session list applies them whenever the store, the selection or the connection changes.
//!
//! - **Something is always open while the list has rows.** With nothing selected, the list picks one: the most recent
//!   visit the helm recorded that is still listed, else (only while the helm has recorded no visits) the remembered last
//!   selection, else the newest-created session.
//! - **A selection stays until it is proven gone.** A selected session missing from the listing is dropped only when the
//!   listing proves it gone ([`session::listing_proves_gone`], the rule a confirmation about a session uses too); a
//!   truncated, filtered or unavailable listing keeps it, or a passing read failure would close what the user had
//!   open. When it is proven gone, the automatic choice picks the next one, never the one that just went.
//! - **Choosing tells the helm, once.** Every accepted choice, a click or automatic, is written as the `last_selected`
//!   preference, from which the helm also keeps its visit history. Choosing the session that is already open writes
//!   nothing.
//! - **Only the user's choice takes focus.** A click on a row ([`choose`]) hands the session and the click's mark to the
//!   pane, whose terminal for that session then takes focus, also when the session was already open (SPEC.md, "Where
//!   focus rests"). An automatic choice ([`open_automatically`]) does not: its terminal only becomes home, and takes
//!   focus only when nothing else holds it, so a session that went away under the user never pulls focus from a
//!   control they are using.
//! - **Opening marks the output seen.** While a session is open, its latest activity is recorded as seen whenever the
//!   session becomes open or its activity advances, unless it is stale (its host is not connected) or has ended (SPEC.md,
//!   "Status", as the old app did). The memory of what was stamped is keyed on the session, its activity and its
//!   eligibility, never on the stamp itself:
//!   that is what makes a manual mark-unread of the open session stick until new output arrives or the user opens
//!   another session. A failed mark is silent (SPEC.md, "Errors and diagnostics"): it costs a dot that is one visit
//!   behind, and the next mark corrects it.
//!
//! NOTE: Not ported yet, because nothing in the app can cause them: the selection after a Delete waits until every
//! admitted Delete has settled and never picks a session being deleted (the row Delete brings the operation store's
//! in-flight deletes into the exclusion); and the old app's refusal to stamp a retired environment's session, which
//! arrives with environment sessions.

use farhelm_helm_client::{ApiPath, Collections, Method};
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

/// The seen mark the open session `open` is due, if any: its new memory, and the session and activity stamp to send
/// when one is due (`None` when the memory changed but nothing needs marking, which still has to be remembered).
pub(crate) fn seen_due(
    open: Option<Listed>,
    stamped: &Stamped,
) -> Option<(Stamped, Option<(String, i64)>)> {
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

/// Record `activity` as the activity the open session `key` was seen at.
pub(crate) fn mark_seen(key: &str, activity: i64, cx: &mut App) {
    send(
        ApiPath::api(&["sessions", key, "seen"]),
        serde_json::json!({ "seen_activity_at": activity }),
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
        current.command(
            Method::Put,
            path,
            Some(body),
            cx,
            move |_, result, _| match result {
                Ok(reply) if (200..300).contains(&reply.status) => {}
                Ok(reply) => tracing::warn!(status = reply.status, "could not {what}"),
                Err(error) => tracing::debug!(?error, "could not {what}"),
            },
        );
    });
}
