//! The merged, multi-host session list the REST edge serves, and the
//! helm-level cursor that pages it (PLAN_M6.md item 5).
//!
//! ## Where the rows come from
//!
//! Not from the hosts. Every connected host's actor drains its supervisor's
//! paginated list to exhaustion into helm.db's session cache
//! ([`crate::manager`]), and this module merges what is in that cache —
//! live hosts' latest refresh and down hosts' last-known entries alike —
//! into one order. A host being connected changes only the `stale` flag on
//! its rows, never where they are read from.
//!
//! One host does not fit that rule and cannot be made to: a supervisor that
//! reports NO identity, against a registry row that has none on record
//! either, has nothing for the cache's identity-bound write to bind to, so
//! its refreshes are never persisted. Its sessions come from the list its
//! actor holds in memory ([`HostSnapshot::live_sessions`]) and are merged in
//! here. They serve exactly while it is connected and vanish when it is not
//! — there is no durable copy, so there is nothing honest to show once the
//! connection is gone. Such a host is excluded from the PERSISTED scope in
//! the same snapshot the merge is built from, so one that reconnects with
//! an identity mid-request cannot appear on both sides at once.
//!
//! (A row that HAS a recorded identity meeting an identity-less hello is a
//! different situation entirely: it freezes rather than serving — see
//! [`crate::manager::HostState::IdentityUnverified`].)
//!
//! That is the decoupling PLAN_M6.md item 5 asks for, and it is worth
//! stating why rather than leaving it to look like an implementation
//! shortcut. Composing the per-host WIRE cursors into the REST cursor
//! would tie one browser page fetch to N live host round trips, so a
//! single flapping host would break a page walk that has nothing to do
//! with it, and a slow host would set the latency of every page. Draining
//! into the cache first makes the REST cursor a plain resume point over
//! local data, and leaves the two cursor layers free to disagree about
//! page sizes, timing, and failure entirely.
//!
//! ## The order
//!
//! Creation time descending, session id ascending, host id ascending — the
//! same total order the wire uses (`farhelm-supervisor`'s `list_order_key`)
//! with one extra component, so a merged list reads as one list rather than
//! as concatenated per-host ones. helm.db's `session_cache_order` index is
//! exactly that key, so a page is an index range scan rather than a sort.
//!
//! The host id is in the key even though session ids are unique: it makes
//! the order TOTAL unconditionally, and a cursor over a non-total order can
//! skip or repeat rows. See [`store::CacheKey`].

use crate::manager::{ConnectionManager, HostSnapshot};
use crate::store::{self, HelmStore, HostId, HostKind};
use farhelm_proto::SessionInfo;
use serde::Serialize;

/// Default page size for `GET /api/sessions` when the caller names none.
///
/// Deliberately equal to the supervisor's own `LIST_SESSION_CAP` default
/// (500): the two cursor layers are independent, but a helm page smaller
/// than a wire page would make the REST layer look like it had introduced a
/// limit of its own, and a larger one would only ever be a partial page
/// anyway for a single-host fleet.
///
/// A page is real work — an indexed scan, a JSON decode per row, a
/// serialize per row — so what a caller may ask for IS capped
/// ([`MAX_PAGE_LIMIT`]) and what a reply may carry is capped again
/// ([`PAGE_BYTE_BUDGET`]). An earlier version of this comment said the
/// opposite, from when the whole cache was read on every poll and a bigger
/// page genuinely cost nothing extra; that has not been true since the page
/// became a page.
pub(crate) const DEFAULT_PAGE_LIMIT: usize = 500;

/// The largest page a caller may ask for.
///
/// A page is real work on this side — an indexed scan, a JSON decode per
/// row, and a serialize per row — so an uncapped `?limit=` is a request to
/// do all of it at once, from an endpoint that is unauthenticated on
/// loopback until M7's web token lands. Ten times the default leaves ample
/// room for a client that genuinely wants fewer round trips while keeping
/// one request bounded.
///
/// A cap on what may be ASKED, not on what may be returned: the byte budget
/// ([`PAGE_BYTE_BUDGET`]) is the independent second cut, and an over-large
/// request is refused outright rather than silently clamped, because a
/// caller that asked for 50,000 and received 5,000 with no `next_cursor`
/// difference has no way to know it did not get what it asked for.
pub(crate) const MAX_PAGE_LIMIT: usize = 5_000;

/// How a host is NAMED on a session row.
///
/// M6 invents no host-naming surface (PLAN_M6.md's Out list), so this is a
/// rendering of what the registry already holds rather than a stored
/// display name: an ssh row is its destination, and the reserved local row
/// — which has no destination by construction — is described rather than
/// named. Kept in one function so every surface that shows a host (session
/// rows, the hosts list) says the same thing about it.
pub(crate) fn host_display_name(kind: HostKind, destination: Option<&str>) -> String {
    match kind {
        // Not the machine's hostname: the helm's own machine is wherever
        // the user is sitting, and SPEC.md's promise is that it is always
        // present as itself rather than as a registered address. A
        // hostname here would also be a second, quietly different identity
        // for the one host the registry deliberately keeps address-less.
        HostKind::Local => "this machine".to_string(),
        HostKind::Ssh => destination.unwrap_or_default().to_string(),
    }
}

/// One row of the merged list: a session exactly as its supervisor
/// reported it, plus the three facts that only exist once more than one
/// host is in play.
///
/// `SessionInfo` is FLATTENED rather than nested, which is the whole
/// compatibility story for the UI that predates this PR: every field it
/// already reads (`id`, `title`, `status`, `tabs`, …) stays at the top
/// level of each row, and the host fields are additive siblings. A nested
/// `{"session": {...}, "host": ...}` would have been tidier and would have
/// broken every existing reader.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct SessionRow {
    #[serde(flatten)]
    pub(crate) info: SessionInfo,
    /// The registry id of the host this session lives on — the value a
    /// client hands back to `POST /api/sessions` to create alongside it,
    /// and the join key into `GET /api/hosts`.
    pub(crate) host: HostId,
    /// [`host_display_name`]'s rendering of that host, denormalized onto
    /// the row so a list can be drawn without a second request.
    pub(crate) host_name: String,
    /// Whether this row is last-known knowledge rather than a live report:
    /// true for every row of a host that is not currently connected, in
    /// ANY of the non-connected states. SPEC.md requires such rows to stay
    /// listed and be clearly marked, and requires operations against them
    /// to be refused — this flag is the first half, and
    /// `crate::route_session` is the second.
    pub(crate) stale: bool,
}

/// One page of the merged list, in the JSON shape `GET /api/sessions`
/// answers with.
///
/// `sessions`/`total`/`truncated` are byte-for-byte the pre-M6 shape
/// (PLAN_M2.md step 6) because the UI in this same tree reads exactly
/// those three keys; `next_cursor` is additive. See each field for what
/// changed UNDERNEATH the unchanged names — this is the one place where
/// keeping a name honest required saying what it now counts.
#[derive(Debug, Serialize)]
pub(crate) struct SessionPageBody {
    pub(crate) sessions: Vec<SessionRow>,
    /// Every session in the MERGED view, across every host, before this
    /// page's cut — not one supervisor's count. That is the number
    /// SPEC.md's "showing N of M" is about once a fleet has more than one
    /// host in it.
    pub(crate) total: u64,
    /// Whether entries remain beyond this page. Retained under its M2 name
    /// for the existing UI, but it no longer means "the supervisor held
    /// entries back and you cannot get them" — the honest reading now is
    /// "there is a next page", and `next_cursor` is how to ask for it.
    pub(crate) truncated: bool,
    /// The opaque resume key for the next page, absent exactly when this
    /// page reached the end of the order. Opaque as a USAGE convention:
    /// replay it verbatim or start a fresh walk, and never construct or
    /// interpret one. It carries an ordering position and no authority (a
    /// caller may already read every session), which is why a well-formed
    /// value from nowhere resumes rather than being refused.
    pub(crate) next_cursor: Option<String>,
}

/// The merged view's cursor payload: the ordering key of the last row a
/// page actually returned.
///
/// ## Why this is not the wire cursor's shape
///
/// The two cursor layers are decoupled by design (see the module docs), and
/// for a while they were decoupled in intent only: both encoded
/// `{"created_at":N,"id":"..."}` as base64url JSON, so each side's decoder
/// accepted the other's tokens verbatim. A wire cursor pasted into
/// `?cursor=` resumed the merged walk at a position it named by accident,
/// and a helm cursor forwarded to a supervisor did the same in reverse —
/// silently, with no error anywhere, producing a page that is not wrong in
/// any way a caller can detect.
///
/// The field NAMES are what enforce the split. `fh` is a required
/// domain-and-version tag with no counterpart on the wire, and the key's
/// components are spelled differently (`at`/`sid`/`h`), so the wire
/// decoder's required `created_at` and `id` are simply absent from a helm
/// token and it fails to parse; `deny_unknown_fields` plus the required tag
/// makes the reverse fail here. A discriminator that lived only in a value
/// both sides ignore would not have done it.
///
/// Resuming means "strictly after this key", never "starting at this row",
/// which is what lets a cursor naming a session that has since been deleted
/// (or whose host was removed) resume cleanly instead of erroring.
#[derive(Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ListCursor {
    /// The domain tag; only [`CURSOR_DOMAIN`] is accepted.
    fh: String,
    /// `created_at`, descending.
    at: i64,
    /// Session id, ascending.
    sid: String,
    /// Host id, ascending — the third component that makes the order total
    /// even where the one-owner index is absent (see [`store::CacheKey`]).
    h: HostId,
}

/// The value [`ListCursor::fh`] must carry: the domain this cursor belongs
/// to, and the version of its own shape.
///
/// Versioned from the start so a future key change is a clean rejection of
/// old tokens (400, "start a fresh walk") rather than a silent
/// misinterpretation of them.
const CURSOR_DOMAIN: &str = "farhelm/helm-sessions/1";

/// Encode one row's ordering key into the opaque token
/// [`SessionPageBody::next_cursor`] carries.
///
/// Base64url-unpadded JSON: self-describing enough that [`decode_cursor`]
/// can reject a malformed value by construction (a JSON parse failure)
/// instead of by delimiter scanning, and free of characters that would need
/// escaping in the query string this token actually travels in.
fn encode_cursor(key: &store::CacheKey) -> String {
    use base64::Engine;
    let cursor = ListCursor {
        fh: CURSOR_DOMAIN.to_string(),
        at: key.created_at,
        sid: key.session_id.clone(),
        h: key.host,
    };
    // Unwrap is safe: `ListCursor` has no map keys and no non-UTF-8 bytes
    // for JSON serialization to fail on.
    let json = serde_json::to_vec(&cursor).expect("ListCursor is always serializable");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json)
}

/// Decode a caller-supplied cursor, or `None` for anything malformed —
/// invalid base64, unparseable JSON, JSON of the wrong shape, or a token
/// from a different domain (a supervisor's wire cursor, a future version of
/// this one).
///
/// Every failure mode collapses to the same `None` on purpose: a
/// bit-flipped byte, a truncated value, and a string from nowhere are
/// indistinguishable to an honest server, and there is no differently
/// actionable answer to give for any of them. Never panics on caller
/// input; `?` short-circuits through `Option` at every fallible step.
fn decode_cursor(cursor: &str) -> Option<store::CacheKey> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .ok()?;
    let decoded: ListCursor = serde_json::from_slice(&bytes).ok()?;
    (decoded.fh == CURSOR_DOMAIN).then_some(store::CacheKey {
        created_at: decoded.at,
        session_id: decoded.sid,
        host: decoded.h,
    })
}

/// The total order pages walk: creation time DESCENDING, then session id
/// ascending, then host id ascending.
///
/// `Reverse` on `created_at` turns "descending time, ascending id" into one
/// ordinary ascending comparison, so a resume point is a plain `>` rather
/// than a hand-written three-field comparison repeated at every call site.
/// This must agree EXACTLY with the `ORDER BY` in
/// [`HelmStore::cached_page`]: the page comes back sorted by SQL and the
/// in-memory rows below are merged into it by this comparison, so a
/// disagreement between the two would interleave them wrongly.
fn order_key(info: &SessionInfo, host: HostId) -> (std::cmp::Reverse<i64>, &str, HostId) {
    (std::cmp::Reverse(info.created_at), info.id.as_str(), host)
}

/// Tag one host's session with that host, for the merged list.
fn row_of(host: &HostSnapshot, info: SessionInfo) -> SessionRow {
    SessionRow {
        info,
        host: host.id,
        host_name: host_display_name(host.kind, host.destination.as_deref()),
        stale: !host.state.is_connected(),
    }
}

/// One item a merge source yields: its ordering key, and the row to serve
/// if there is one.
///
/// A key with NO row is a cache entry whose stored metadata no longer
/// decodes. It occupies its place in the order and must be passed over by
/// the cursor even though nothing can be shown for it — see
/// [`store::ScannedRow`], which exists for exactly this and whose absence
/// made poisoned rows a permanent wall in front of everything after them.
struct MergeItem {
    key: store::CacheKey,
    row: Option<SessionRow>,
}

/// One already-ordered sequence to merge: the persisted page, or one
/// identity-less host's in-memory list.
///
/// The sources are pulled from lazily and in lockstep, which is what keeps
/// this bounded: the merge takes `limit + 1` items in total, so an
/// identity-less host holding thousands of sessions contributes exactly as
/// much work to a `limit=1` request as a host holding one. Cloning happens
/// per item TAKEN, never per item available.
struct MergeSource<'a> {
    snapshot: Option<&'a HostSnapshot>,
    /// Remaining items, in order. For the persisted source these are
    /// already-decoded scan results; for a live source they are borrowed
    /// `SessionInfo`s cloned only when taken.
    persisted: std::collections::VecDeque<store::ScannedRow>,
    live: &'a [SessionInfo],
}

impl<'a> MergeSource<'a> {
    /// This source's next ordering key, without consuming it.
    fn peek(&self) -> Option<(std::cmp::Reverse<i64>, &str, HostId)> {
        if let Some(scanned) = self.persisted.front() {
            return Some((
                std::cmp::Reverse(scanned.key.created_at),
                scanned.key.session_id.as_str(),
                scanned.key.host,
            ));
        }
        let host = self.snapshot?.id;
        self.live.first().map(|info| order_key(info, host))
    }

    /// Take this source's next item, cloning only what is taken.
    ///
    /// `hosts` is consulted for the persisted source, whose rows can name
    /// any host in scope; a live source already knows its own.
    fn take(
        &mut self,
        hosts: &std::collections::HashMap<HostId, &HostSnapshot>,
    ) -> Option<MergeItem> {
        if let Some(scanned) = self.persisted.pop_front() {
            let row = scanned
                .info
                .and_then(|info| Some(row_of(hosts.get(&scanned.key.host)?, info)));
            return Some(MergeItem {
                key: scanned.key,
                row,
            });
        }
        let snapshot = self.snapshot?;
        let (info, rest) = self.live.split_first()?;
        self.live = rest;
        Some(MergeItem {
            key: store::CacheKey {
                created_at: info.created_at,
                session_id: info.id.clone(),
                host: snapshot.id,
            },
            row: Some(row_of(snapshot, info.clone())),
        })
    }
}

/// One page of the merged list, resuming strictly after `cursor`.
///
/// ## What this reads, and what it deliberately does not
///
/// The persisted majority of the list comes from ONE indexed scan of ONE
/// page ([`HelmStore::cached_page`]): the resume predicate, the row limit,
/// and a work-bounding byte cap all apply inside that scan, so a poll
/// decodes only the rows it is about to return. The shape this replaced
/// loaded and deserialized every session on every host on every poll, which
/// made a full page walk quadratic in the fleet's size.
///
/// The exception is a connected host that reports NO identity and has none
/// on record. Such a host caches nothing (the cache's write is
/// identity-bound) and instead serves from the list its actor holds in
/// memory, so those rows are merged in here rather than paged from SQL. The
/// merge is lazy and lockstep: it takes `limit + 1` items in total across
/// every source, so such a host contributes the same work to a `limit=1`
/// request whether it holds one session or five thousand.
///
/// ## The cursor advances past what cannot be shown
///
/// A cache row whose stored metadata no longer decodes is skipped for
/// display but still consumes its place in the order, and the page's
/// `next_cursor` is the key of the last item TAKEN — decoded or not. Any
/// other rule makes a poisoned row a permanent wall: the next page resumes
/// before it, skips it again, and every later row in the fleet becomes
/// unreachable. A page can therefore legitimately come back EMPTY with a
/// `next_cursor` set, and a walking caller must follow it rather than stop.
///
/// ## Stability
///
/// The cursor encodes a KEY, never an offset, so a session created between
/// two page fetches appears at the front of a later refresh and never tears
/// the page being walked, and a session deleted between them simply is not
/// there — the resume point does not need it to still exist. That mirrors
/// the supervisor's own page-walk contract exactly, one layer up.
///
/// ## Two independent cuts
///
/// The caller's `limit` and [`PAGE_BYTE_BUDGET`] both cut, exactly as the
/// supervisor's own list reply has always applied its count cap and its byte
/// budget independently. A page of fat records shrinks rather than
/// oversizing the reply, and the `next_cursor` a cut leaves behind is what
/// keeps the walk complete either way.
///
/// An undecodable cursor is an error rather than a silent restart from the
/// front: restarting would hand a caller a page it had already seen while
/// looking exactly like progress.
pub(crate) async fn session_page(
    manager: &ConnectionManager,
    store: &HelmStore,
    cursor: Option<&str>,
    limit: usize,
) -> anyhow::Result<SessionPageBody> {
    let after = match cursor {
        None => None,
        Some(raw) => Some(decode_cursor(raw).ok_or_else(malformed_cursor)?),
    };
    let snapshots = manager.snapshots();
    let by_id: std::collections::HashMap<HostId, &HostSnapshot> = snapshots
        .iter()
        .map(|snapshot| (snapshot.id, snapshot))
        .collect();

    // A host serving from MEMORY is excluded from the persisted scope, in
    // this same snapshot. Both halves of that matter: an identity-less host
    // writes no cache, so including it costs nothing — until it reconnects
    // WITH an identity mid-request, at which point the snapshot still shows
    // its in-memory list while the cache already holds the same sessions,
    // and every one of them appears twice in the page and twice in the
    // total. Scoping from the one snapshot the merge is built on makes the
    // two views disjoint by construction rather than by timing.
    let scope: Vec<HostId> = snapshots
        .iter()
        .filter(|snapshot| snapshot.live_sessions.is_none())
        .map(|snapshot| snapshot.id)
        .collect();

    let persisted = store
        .cached_page(scope.clone(), after.clone(), limit, PAGE_BYTE_BUDGET)
        .await?;
    let persisted_more = persisted.more;
    // The fence: with the persisted source incomplete, nothing at or past
    // this key has been seen, so the merge must not step over it on the
    // strength of an in-memory row that sorts later. Without it a
    // byte-bounded scan (which can return FEWER than `limit` rows) let the
    // merge fill the rest of the page from live hosts and issue a cursor
    // past persisted rows nobody had been shown — skipped for good.
    let fence = persisted.frontier.clone();
    let mut sources: Vec<MergeSource<'_>> = vec![MergeSource {
        snapshot: None,
        persisted: persisted.rows.into(),
        live: &[],
    }];

    // The in-memory side, positioned at the resume point by binary search
    // rather than by scanning: each list is already in the merged order (a
    // drain follows the wire order — validated, not assumed, see
    // `manager::drain_sessions` — and one list is one host, so the third
    // key component is constant), so the resume point is a partition.
    let mut live_total: u64 = 0;
    for snapshot in &snapshots {
        let Some(live) = snapshot.live_sessions.as_ref() else {
            continue;
        };
        live_total += live.len() as u64;
        let start = match &after {
            None => 0,
            Some(after) => {
                live.partition_point(|info| order_key(info, snapshot.id) <= key_order(after))
            }
        };
        sources.push(MergeSource {
            snapshot: Some(snapshot),
            persisted: std::collections::VecDeque::new(),
            live: &live[start..],
        });
    }

    // The k-way merge: one item at a time, up to `limit + 1`, so nothing
    // beyond the page is ever cloned — and the byte budget is applied AS
    // items are taken, so a page of fat records stops costing at the budget
    // rather than after cloning every candidate.
    let mut taken: Vec<MergeItem> = Vec::new();
    let mut bytes = 0usize;
    let mut more = false;
    loop {
        let next = sources
            .iter()
            .enumerate()
            .filter_map(|(index, source)| source.peek().map(|key| (key, index)))
            .min()
            .map(|(_, index)| index);
        let Some(index) = next else { break };
        if taken.len() == limit {
            // The extra item exists only to answer "is there more"; it is
            // not part of the page and must not advance the cursor.
            more = true;
            break;
        }
        // The fence, checked BEFORE taking: an item at or past the first
        // unseen persisted row would carry the cursor over rows nobody has
        // been shown.
        if sources[index].snapshot.is_some()
            && let Some(fence) = &fence
            && sources[index]
                .peek()
                .is_some_and(|key| key >= key_order(fence))
        {
            more = true;
            break;
        }
        let Some(item) = sources[index].take(&by_id) else {
            break;
        };
        // Measured on the row that will actually be serialized, and applied
        // here rather than after the merge: the post-hoc version cloned
        // every candidate first, which is the cost the budget exists to
        // avoid. At least one row is always taken — a single record larger
        // than the budget must still make progress, or the walk stalls on
        // it forever.
        if let Some(row) = &item.row {
            let size = serde_json::to_vec(row)
                .map(|bytes| bytes.len())
                .unwrap_or(0);
            if !taken.is_empty() && bytes.saturating_add(size) > PAGE_BYTE_BUDGET {
                more = true;
                break;
            }
            bytes = bytes.saturating_add(size);
        }
        taken.push(item);
    }
    // A persisted scan that stopped at its OWN bound means more remains even
    // where the merge above ran dry — its rows are all consumed, but the
    // table is not.
    more |= persisted_more && sources[0].persisted.is_empty();

    // The cursor comes from the last taken item's ORIGINAL key — the one the
    // store filed the row under — never from the payload it decoded to. The
    // two can disagree (a poisoned row has no payload at all, and a
    // column/payload mismatch is treated as poison for exactly this reason),
    // and a cursor built from the payload would resume somewhere the order
    // does not contain.
    let next_cursor = more
        .then(|| taken.last().map(|item| encode_cursor(&item.key)))
        .flatten();
    let sessions: Vec<SessionRow> = taken.into_iter().filter_map(|item| item.row).collect();
    let total = store.cached_count(scope).await? + live_total;
    Ok(SessionPageBody {
        sessions,
        total,
        truncated: next_cursor.is_some(),
        next_cursor,
    })
}

/// One [`store::CacheKey`] as the comparable ordering tuple [`order_key`]
/// produces, so a key and a row are compared by the same rule.
fn key_order(key: &store::CacheKey) -> (std::cmp::Reverse<i64>, &str, HostId) {
    (
        std::cmp::Reverse(key.created_at),
        key.session_id.as_str(),
        key.host,
    )
}

/// The refusal an undecodable `?cursor=` produces.
fn malformed_cursor() -> anyhow::Error {
    anyhow::Error::new(crate::SupervisorError {
        kind: farhelm_proto::ErrorKind::InvalidRequest,
        message: "session list cursor could not be decoded; cursors are opaque — replay one \
                  exactly as a reply carried it, or start a fresh walk with no cursor at all"
            .to_string(),
    })
}

/// How many bytes of encoded session rows one page may carry.
///
/// The helm's counterpart to the supervisor's own `LIST_BYTE_BUDGET`, and
/// derived the same way (half the protocol's maximum frame) for the same
/// reason: a session's fields are user-supplied text with no individual cap
/// worth trusting, so a page of a few hundred fat records can be far larger
/// than its row count suggests. This is the ceiling that turns "500 rows"
/// into "500 rows or as many as fit", with the cut expressed as a
/// `next_cursor` so the walk still completes.
///
/// A caller-set `limit` does not raise it. That is the point of having two
/// cuts: the count is what the caller wants, the budget is what the reply
/// can carry, and neither is allowed to overrule the other.
const PAGE_BYTE_BUDGET: usize = (farhelm_proto::MAX_FRAME_LEN / 2) as usize;

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::{RestartOffer, SessionStatus};

    /// A row with only the fields this module's ordering and cursors care
    /// about; everything else is filler.
    fn row(id: &str, created_at: i64, host: HostId) -> SessionRow {
        SessionRow {
            info: SessionInfo {
                id: id.to_string(),
                title: id.to_string(),
                created_at,
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::Alive,
                annotation: None,
                restart_offer: RestartOffer::default(),
                tabs: Vec::new(),
            },
            host,
            host_name: "this machine".to_string(),
            stale: false,
        }
    }

    /// A cursor must survive its own round trip exactly — a resume point
    /// that decoded to a DIFFERENT key would skip or repeat rows with no
    /// error anywhere, which is the one failure mode a page walk cannot
    /// detect for itself.
    #[test]
    fn a_cursor_round_trips_every_component_of_the_key() {
        for key in [
            store::CacheKey {
                created_at: 1_700_000_000,
                session_id: "sess-1".to_string(),
                host: 7,
            },
            // The extremes, because the key is signed and the id is
            // arbitrary text a supervisor chose.
            store::CacheKey {
                created_at: i64::MIN,
                session_id: String::new(),
                host: i64::MAX,
            },
            store::CacheKey {
                created_at: i64::MAX,
                session_id: "a \"quoted\" / slashed \u{1f600} id".to_string(),
                host: 0,
            },
        ] {
            let decoded = decode_cursor(&encode_cursor(&key)).expect("our own cursor decodes");
            assert_eq!(decoded, key);
        }
    }

    /// The helm's cursor and the supervisor's wire cursor must not be
    /// interchangeable, in EITHER direction.
    ///
    /// SPEC_impl.md records the two layers as decoupled, and for a while
    /// they were decoupled in intent only: both encoded the same
    /// `{"created_at","id"}` JSON, so each decoder silently accepted the
    /// other's tokens and resumed at a position it named by accident. There
    /// is no error such a caller could observe, which is exactly why the
    /// property is pinned here rather than left to the shapes looking
    /// different.
    ///
    /// The wire side is asserted against a faithful LOCAL mirror of
    /// `farhelm-supervisor`'s `ListCursor` rather than the real type: that
    /// one is `pub(crate)` to its own crate, and reaching across for it
    /// would couple this test to the supervisor's internals to prove the
    /// two are uncoupled. The mirror carries what matters — the same two
    /// required fields — so a helm token that cannot satisfy it cannot
    /// satisfy the original either.
    #[test]
    fn the_helm_cursor_and_the_wire_cursor_reject_each_other() {
        use base64::Engine;

        #[derive(serde::Deserialize, serde::Serialize)]
        struct WireCursorMirror {
            created_at: i64,
            id: String,
        }

        let helm = encode_cursor(&store::CacheKey {
            created_at: 1_700_000_000,
            session_id: "sess-1".to_string(),
            host: 1,
        });
        let helm_json = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&helm)
            .expect("our cursor is base64url");
        assert!(
            serde_json::from_slice::<WireCursorMirror>(&helm_json).is_err(),
            "a supervisor must not be able to read a helm cursor as a wire cursor"
        );

        let wire_json = serde_json::to_vec(&WireCursorMirror {
            created_at: 1_700_000_000,
            id: "sess-1".to_string(),
        })
        .expect("serializable");
        let wire = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&wire_json);
        assert!(
            decode_cursor(&wire).is_none(),
            "the helm must not accept a supervisor's wire cursor as a resume point"
        );
    }

    /// A tampered, hand-built, or foreign-domain cursor must be a clean
    /// refusal, never a panic and never a silent restart from the front — a
    /// restart would re-serve a page the caller already had while looking
    /// exactly like progress.
    #[test]
    fn a_malformed_or_foreign_cursor_decodes_to_nothing() {
        use base64::Engine;

        let foreign_domain = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            serde_json::json!({"fh": "farhelm/helm-sessions/2", "at": 1, "sid": "s", "h": 1})
                .to_string(),
        );
        let extra_field = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            serde_json::json!({"fh": CURSOR_DOMAIN, "at": 1, "sid": "s", "h": 1, "x": 1})
                .to_string(),
        );
        for malformed in [
            "not-base64!!",
            "",
            "YWJj",
            "e30",
            foreign_domain.as_str(),
            extra_field.as_str(),
        ] {
            assert!(
                decode_cursor(malformed).is_none(),
                "cursor {malformed:?} must not decode"
            );
        }
    }

    /// The ordering key must agree with the SQL `ORDER BY` it is merged
    /// against, down to the third component — an in-memory row sorted by a
    /// different rule would interleave wrongly with the paged rows and the
    /// resulting page would be neither order.
    #[test]
    fn the_order_is_time_descending_then_id_then_host() {
        let newest = row("b", 200, 1);
        let older = row("a", 100, 1);
        assert!(order_key(&newest.info, newest.host) < order_key(&older.info, older.host));

        let tie_low_id = row("a", 100, 9);
        let tie_high_id = row("b", 100, 1);
        assert!(
            order_key(&tie_low_id.info, tie_low_id.host)
                < order_key(&tie_high_id.info, tie_high_id.host),
            "equal creation times order by session id, not by host"
        );

        let same_id_low_host = row("a", 100, 1);
        let same_id_high_host = row("a", 100, 2);
        assert!(
            order_key(&same_id_low_host.info, same_id_low_host.host)
                < order_key(&same_id_high_host.info, same_id_high_host.host),
            "the host id is the final tiebreak, so the order is total even where two hosts \
             claim one session id"
        );
    }
}
