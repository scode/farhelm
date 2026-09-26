//! The rules the helm's two session caches share.
//!
//! The helm remembers each host's sessions in one of two places: durably in
//! the store's `session_cache` table for a host with an identity, and in the
//! host actor's published status for one without. Both answer the same
//! questions: how a mutation's reply combines with what was already known,
//! what order a host's sessions come in, which row makes room when a seed
//! would push a host past the listing cap, and which session ids may enter at
//! all. Those answers live here, as plain functions over plain values, so
//! the SQL backend (`store.rs`) and the in-memory one (`manager.rs`) call the
//! same code instead of each carrying a copy that can drift.
//!
//! This module depends on neither backend. The store used to reach into the
//! manager for the merge rule and the id bound, which tied the storage layer
//! to the connection layer above it.

use farhelm_proto::{SessionInfo, SessionStatus};
use std::cmp::Reverse;

/// Longest session id this helm will accept from a peer.
///
/// Not a guess at what a supervisor mints (a UUID, 36 bytes) but a bound on
/// what this side can still WORK with. A session id is embedded verbatim in
/// REST paths and query strings (`/api/sessions/{id}`, the `parent` filter),
/// so an id anywhere near the frame limit names a session no client could
/// ever address, because the request head is refused before any handler
/// sees it.
///
/// A kibibyte is two orders of magnitude above every id any farhelm
/// supervisor has ever minted and still leaves a URL comfortably inside
/// any HTTP head limit. The value comes from `farhelm-proto` so this
/// ingestion check cannot drift from the handshake's auth-session check.
pub const MAX_SESSION_ID_BYTES: usize = farhelm_proto::MAX_SESSION_ID_BYTES;

/// Refuse a mutation's reply whose session id is past
/// [`MAX_SESSION_ID_BYTES`], before either cache records it.
pub fn ensure_recordable_id(id: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        id.len() <= MAX_SESSION_ID_BYTES,
        "session id of {} bytes exceeds the {} this helm can build resumable cursors over",
        id.len(),
        MAX_SESSION_ID_BYTES
    );
    Ok(())
}

/// The sort key for a host's sessions: newest first by creation time, ties
/// broken by id ascending so the order is total and repeatable.
///
/// The same key decides eviction ([`eviction_victim`] takes the LARGEST key,
/// the oldest row) and the order a cached slice is served in.
pub fn creation_order(created_at: i64, id: &str) -> (Reverse<i64>, &str) {
    (Reverse(created_at), id)
}

/// The row to evict when recording one more session would leave a host
/// with more than `farhelm_proto::LIST_SESSIONS_CAP` cached rows: the oldest
/// row other than `keep`, the one just recorded.
///
/// Both caches hold the cap by eviction rather than by refusing the new
/// row: the create already succeeded on the supervisor, and a cache without
/// its row would leave a session the caller was just told exists
/// unroutable until the next refresh. The victim is chosen by
/// [`creation_order`] rather than by position, because the in-memory list
/// keeps the peer's own order and the table has none. `None` only when
/// `keep` is the sole candidate.
pub fn eviction_victim<'a>(
    rows: impl IntoIterator<Item = (i64, &'a str)>,
    keep: &str,
) -> Option<&'a str> {
    rows.into_iter()
        .filter(|(_, id)| *id != keep)
        .max_by(|a, b| creation_order(a.0, a.1).cmp(&creation_order(b.0, b.1)))
        .map(|(_, id)| id)
}

/// The status a recorded session should end up with, given what the helm
/// already knew and what a mutation's reply just said.
///
/// `Unknown` NEVER overwrites a definite status, and that one rule is the
/// whole function.
///
/// The asymmetry is the protocol's, not this helm's invention.
/// `SessionStatus::Unknown`'s own contract is explicit that `ListSessions`
/// is the only reply computing a REAL answer, and that everywhere else the
/// value means "not yet known" rather than "known not to be running" — so a
/// restart's or a create's reply carries `Unknown` deliberately, because at
/// the instant it is built the pane exists but the agent's own `exec`
/// inside it has not been observed. Claiming liveness there would be a
/// fabrication; the supervisor is right to refuse it.
///
/// What follows for THIS side is that such a reply is not evidence about
/// liveness at all, and must not be recorded as if it were. Letting it
/// through cost a real, user-visible regression: a restart of a live
/// session replaced a cached `alive` with `unknown`, so the list answered a
/// successful restart with a badge that says the helm has no idea — for a
/// session it had definite knowledge about a moment earlier. Every other
/// field of the reply is fresh and authoritative and is taken as given; the
/// status alone is knowledge the reply does not have.
///
/// Keeping the previous value can leave it briefly STALE — a restart of an
/// `exited` session goes on reading `exited` until the owning host's next
/// refresh computes the truth. That is the same one-interval lag the list
/// had before mutations were recorded at all, and it is strictly better
/// than the alternative: stale-but-definite is a claim the helm can defend,
/// and `unknown` is the absence of one.
pub fn merged_status(previous: &SessionStatus, incoming: SessionStatus) -> SessionStatus {
    match incoming {
        SessionStatus::Unknown => previous.clone(),
        definite => definite,
    }
}

/// Fold what the helm already cached for a session into the `SessionInfo`
/// a MUTATION's reply just produced, before that reply is recorded.
///
/// One helper for both storage shapes on purpose. The helm caches sessions
/// two ways — durably in `session_cache` for a host with an identity, in
/// memory for one without — and each has its own "what did we know"
/// lookup. The rules for reconciling a mutation's reply against it are the
/// same rules either way, and two copies of them is how the two shapes
/// come to disagree about what a reply is evidence of.
///
/// A mutation's reply is authoritative about everything it describes
/// EXCEPT the sampled status and timestamps handled here: each is computed
/// by machinery the reply did not run. `status` is
/// [`merged_status`]'s subject. Activity age and work-start ordering are the supervisor's
/// sampler's, and a create/rename/restart reply merely copies
/// whatever the entry happened to hold when it was built — which can be
/// OLDER than what a `ListSessions` drain already committed here, because
/// replies and drains race and nothing orders them.
///
/// So the value is carried forward monotonically: a reply may push it
/// forward, never back. `0` is the field's "unknown" (an old sender omits
/// it entirely — see `SessionInfo::last_activity_at` and
/// `SessionInfo::last_work_started_at`), and it is also the
/// smallest value the field takes, so a plain maximum is exactly the rule
/// "never let an absent or stale answer erase a real one". Moving it
/// backwards could undo activity/unseen evidence or drop a session down the
/// recent-work list just because somebody renamed or restarted it.
///
/// The `created_at` fallback a 0 implies is deliberately NOT applied here.
/// It belongs at read and sort time, where the reader has both fields in
/// hand; writing a synthesized value into the cache would make a guess
/// indistinguishable from an observation for every later merge.
pub fn merge_cached_session(previous: &SessionInfo, incoming: &mut SessionInfo) {
    incoming.status = merged_status(&previous.status, incoming.status.clone());
    incoming.last_activity_at = previous.last_activity_at.max(incoming.last_activity_at);
    incoming.last_work_started_at = previous
        .last_work_started_at
        .max(incoming.last_work_started_at);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: the eviction victim is the oldest row by creation time, with a
    /// tie going to the later id, and never the row just recorded.
    ///
    /// Both caches evict through this one function, so the choice it makes
    /// is the choice both make. The tie case matters because creation
    /// times are whole seconds and a burst of creates shares one; the
    /// exclusion matters because the new row is the one the caller was just
    /// told exists.
    #[farhelm_testtrace::test]
    fn eviction_takes_the_oldest_other_row_with_ties_to_the_later_id() {
        let rows = [(30, "new"), (10, "a"), (10, "b"), (20, "c")];
        assert_eq!(eviction_victim(rows, "new"), Some("b"));
        let oldest_is_new = [(5, "new"), (10, "a"), (20, "c")];
        assert_eq!(eviction_victim(oldest_is_new, "new"), Some("a"));
        assert_eq!(eviction_victim([(5, "new")], "new"), None);
    }

    /// Spec: a host's sessions sort newest first, ties by id ascending.
    ///
    /// The durable cache's read path serves rows in this order, and the
    /// eviction rule above is defined as its last element; the two must be
    /// the same key or the evicted row would not be the one listed last.
    #[farhelm_testtrace::test]
    fn creation_order_is_newest_first_with_ties_by_id() {
        let mut rows = [(10, "b"), (20, "c"), (10, "a")];
        rows.sort_by(|x, y| creation_order(x.0, x.1).cmp(&creation_order(y.0, y.1)));
        assert_eq!(rows, [(20, "c"), (10, "a"), (10, "b")]);
    }

    /// Spec: an `Unknown` status never replaces a definite one, and any
    /// definite status replaces whatever was there.
    ///
    /// A mutation's reply can carry `Unknown` as a placeholder rather than an
    /// observation (see `SessionStatus`); recording it erased a known status
    /// in the list, which is the regression [`merged_status`] exists to
    /// prevent.
    #[farhelm_testtrace::test]
    fn unknown_keeps_the_previous_status_and_definite_replaces_it() {
        let running = SessionStatus::Running;
        assert_eq!(merged_status(&running, SessionStatus::Unknown), running);
        let exited = SessionStatus::Exited { exit_code: Some(0) };
        assert_eq!(merged_status(&running, exited.clone()), exited);
    }

    /// Spec: the id bound admits exactly [`MAX_SESSION_ID_BYTES`] bytes.
    #[farhelm_testtrace::test]
    fn the_id_bound_is_inclusive() {
        assert!(ensure_recordable_id(&"x".repeat(MAX_SESSION_ID_BYTES)).is_ok());
        assert!(ensure_recordable_id(&"x".repeat(MAX_SESSION_ID_BYTES + 1)).is_err());
    }
}
