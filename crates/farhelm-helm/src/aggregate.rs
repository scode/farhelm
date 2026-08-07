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
//!
//! ## Filtering, and why it happens HERE (PLAN_M6_75.md item 5)
//!
//! SPEC.md's session list filters by host, directory, profile, status and
//! title, and every one of those predicates is applied in this merged view
//! — not in the drains, and not in the browser.
//!
//! Not in the DRAINS: a filtered drain would cache a partial list, and the
//! cache's whole job is to be the complete last-known state of a host that
//! may be gone by the time anyone asks. Host filters, identity-less
//! sessions, and stale retained rows only exist merged anyway.
//!
//! Not in the BROWSER: predicates have to apply before the page cut, or "N
//! matching" is a claim about rows the client cannot see. A page filtered
//! after the fact hides matches beyond its own boundary while counting them.
//!
//! So both sources are narrowed by one predicate ([`store::SessionFilter`],
//! which lives in the store precisely so the SQL scan and the in-memory
//! merge cannot come to disagree), and a FILTERED reply carries TWO counts:
//! how many matched, and how many the fleet holds. The second is not
//! redundant — it is the number the list's own coherence check has always
//! described, and a filtered list showing fewer rows than the fleet holds is
//! a working filter rather than a missing page.
//!
//! An UNFILTERED reply carries only the fleet total and makes no matching
//! claim at all (see [`SessionPageBody::matching`]).
//!
//! ## Counting once per walk, honestly (PLAN_M6_75.md item 5)
//!
//! Counting matches is a decode of every row in scope, so doing it per page
//! makes a walk quadratic in the fleet — under the store's one mutex, which
//! makes it everyone's problem rather than only the walker's. It therefore
//! happens once per walk, and the machinery that achieves that is worth
//! naming because an earlier shape got it dangerously wrong.
//!
//! The count lives HERE, in a bounded in-process cache ([`MatchingCounts`]),
//! and never in the cursor. A cursor is a client-held string; a number
//! carried in one is a number the client chooses, and the server would be
//! reporting an arbitrary "N matching" back to whoever asked. What the cursor
//! carries is a POSITION and a binding to the filter it was taken under
//! ([`store::SessionFilter::digest`]) — neither of which is authority the
//! caller does not already have.
//!
//! Only the PERSISTED half is ever cached, and that asymmetry is the whole
//! safety argument. helm.db's rows are qualified by the store's mutation
//! generation, sampled inside the same lock hold that produced the page
//! ([`store::MergedRead::generation`]), so "the count and these rows describe
//! one moment" is true by construction rather than by a check that could
//! straddle a commit. The identity-less hosts' in-memory lists have no such
//! token — nothing about them touches the store — so their share of the count
//! is recomputed on every request, from the very snapshot the page's live rows
//! are merged out of.
//!
//! A cheaper-looking arrangement was live for a while and was wrong: key the
//! cache by the fleet REVISION as well, and reuse the live component while it
//! stands still. The revision is published AFTER the change it describes
//! becomes visible (`crate::manager`'s `HostActor::publish_refresh` swaps a
//! host's list and only then bumps), so a request can read the old
//! revision, snapshot the NEW list, and hit a cache entry filed under a count
//! taken over the OLD one — reporting a matching number that disagrees with
//! the rows it is returning beside it, with the store's generation unmoved
//! because none of it ever touched the store. Recomputing costs a few string
//! compares over a bounded in-memory list the request is already holding, and
//! it buys coherence that does not depend on scheduling.

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
/// row, and a serialize per row — so an uncapped `?limit=` lets one admitted
/// client demand all of it at once. Authentication identifies a device; it
/// does not make its requests cheap. Ten times the default leaves ample room
/// for a client that genuinely wants fewer round trips while keeping one
/// request bounded.
///
/// A cap on what may be ASKED, not on what may be returned: the byte budget
/// ([`PAGE_BYTE_BUDGET`]) is the independent second cut, and an over-large
/// request is refused outright rather than silently clamped, because a
/// caller that asked for 50,000 and received 5,000 with no `next_cursor`
/// difference has no way to know it did not get what it asked for.
pub(crate) const MAX_PAGE_LIMIT: usize = 5_000;

/// The largest FILTERED page a caller may ask for.
///
/// Lower than [`MAX_PAGE_LIMIT`] because a filtered page is a different kind
/// of work, not merely more of it. An unfiltered page reads exactly the rows
/// it returns — its `limit` is a slice. A filtered one walks the merged order
/// until it has filled itself, decoding every row it steps over, so a large
/// limit paired with a selective filter asks this side to scan and decode the
/// whole cache while holding the store's one mutex.
///
/// Set to the default page size deliberately: a filtering client is a person
/// looking at a list, and nobody reads five thousand filtered rows at once.
/// A caller that genuinely wants more pages the walk, which is what the
/// cursor is for.
///
/// ## What this caps, and what it does NOT
///
/// It caps ROWS RETURNED, not work done. A filtered request decodes rows
/// until it has filled its page or run out of scope, and the first page of
/// any walk decodes the whole scope once regardless (that is the counting
/// pass — see [`store::MatchingCount`]). So `limit=1` against a filter that
/// matches nothing costs a full scan of the cache, and this constant does not
/// change that by a single row. What the cache behind the count buys is that
/// the REST of the walk is bounded; what this constant buys is that no single
/// reply is enormous.
///
/// Two things would genuinely bound the work and are deliberately NOT here:
/// a global scan or time budget across concurrent requests, and pushing the
/// predicates into SQL so SQLite decides what to touch. Both are real
/// options and both are being declined this milestone for the same reason:
/// the helm listens on loopback and serves authenticated devices for one user,
/// the cache is bounded per host by `crate::manager::REFRESH_SESSION_CAP`, and
/// the cost of the wrong
/// abstraction here (a budget that silently truncates a count, or a filter
/// vocabulary split between SQL and Rust that can come to disagree) is worse
/// than the scan. Revisit when the helm serves more than one trust domain.
pub(crate) const MAX_FILTERED_PAGE_LIMIT: usize = DEFAULT_PAGE_LIMIT;

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
    /// page's cut and BEFORE any filter — not one supervisor's count, and
    /// not a count of what the caller asked to see. That is the number
    /// SPEC.md's "showing N of M" is about once a fleet has more than one
    /// host in it, and it is deliberately the one the list's coherence check
    /// has always been made against (PLAN_M6_75.md item 5): a filtered page
    /// showing fewer rows than the fleet holds is not an incoherent list, it
    /// is a working filter.
    pub(crate) total: u64,
    /// How many sessions match the request's filter, across the whole
    /// merged view — the other half of "N matching of M sessions"
    /// (PLAN_M6_75.md item 5).
    ///
    /// Counted before the page cut, so it describes the whole fleet rather
    /// than this page: a client showing twenty rows of two hundred matches
    /// needs the two hundred, and can derive nothing about it from the rows
    /// it holds.
    ///
    /// ABSENT for an unfiltered listing, which makes no matching claim at
    /// all. The obvious alternative — "no filter, so everything matches,
    /// report `total`" — is not true of this list: `total` counts every
    /// cached row including ones whose payload can no longer be trusted as
    /// that row, while a matching count deliberately excludes exactly those
    /// (`store::usable_cached_session`). Reporting `total` here would make an
    /// unshowable row count as a match in precisely the case nobody filtered,
    /// which is the one place the invariant was stated most loudly. A client
    /// with no matching count has `total` in hand and substitutes it, which
    /// is what it would have been handed anyway.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) matching: Option<u64>,
    /// Whether entries remain beyond this page — of the MATCHING rows, once
    /// a filter is set: the page walk is over what matched, so "there is a
    /// next page" is a statement about that walk. Retained under its M2 name
    /// for the existing UI, but it no longer means "the supervisor held
    /// entries back and you cannot get them" — the honest reading now is
    /// "there is a next page", and `next_cursor` is how to ask for it.
    pub(crate) truncated: bool,
    /// The opaque resume key for the next page, absent exactly when this
    /// page reached the end of the order. Opaque as a USAGE convention:
    /// replay it verbatim or start a fresh walk, and never construct or
    /// interpret one.
    ///
    /// It carries an ordering position and a binding to the filter it was
    /// taken under, and no authority of any kind — a caller may already read
    /// every session, and nothing the server reports is derived from a number
    /// the cursor supplies. That is why a well-formed value naming a position
    /// nobody has seen resumes rather than being refused, while one replayed
    /// under a DIFFERENT filter is refused outright (see [`session_page`]).
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
///
/// ## What it carries, and what it deliberately does not
///
/// A position and the filter it was taken under. Nothing else — in
/// particular no COUNT. A cursor is a string the client holds, so a count
/// carried in one is a count the client picks, and every number the reply
/// derives from it becomes a number the caller dictated to the server about
/// its own fleet. The matching count is kept server-side instead
/// ([`MatchingCounts`]), where the only thing a client can influence is
/// whether a cache entry is hit.
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
    /// The digest of the FILTER this position was taken under
    /// ([`store::SessionFilter::digest`]) — required on every cursor, an
    /// unfiltered walk's included.
    ///
    /// A position only means something within the result set it came from.
    /// Replayed under a different filter it resumes mid-order through a
    /// sequence it never described, so every match before that point is
    /// silently skipped — which contradicts the cursor-and-filter-travel-
    /// together contract in a way no client can detect. The digest is what
    /// turns that into a 400.
    ///
    /// Fixed-size, so a cursor cannot grow with the search box: a first page
    /// whose query string already sits near an HTTP head limit would
    /// otherwise mint a follow-up cursor nobody could replay.
    f: String,
}

/// The value [`ListCursor::fh`] must carry: the domain this cursor belongs
/// to, and the version of its own shape.
///
/// Versioned so a key change is a clean rejection of old tokens (400, "start
/// a fresh walk") rather than a silent misinterpretation of them. Version 2
/// is the shape that stopped carrying a matching count and started requiring
/// a filter digest; a version-1 token names neither correctly, and there is
/// no reading of one that is safe to attempt.
const CURSOR_DOMAIN: &str = "farhelm/helm-sessions/2";

/// Encode one row's ordering key, bound to the filter it was taken under,
/// into the opaque token [`SessionPageBody::next_cursor`] carries.
///
/// Base64url-unpadded JSON: self-describing enough that [`decode_cursor`]
/// can reject a malformed value by construction (a JSON parse failure)
/// instead of by delimiter scanning, and free of characters that would need
/// escaping in the query string this token actually travels in.
fn encode_cursor(key: &store::CacheKey, digest: &str) -> String {
    use base64::Engine;
    let cursor = ListCursor {
        fh: CURSOR_DOMAIN.to_string(),
        at: key.created_at,
        sid: key.session_id.clone(),
        h: key.host,
        f: digest.to_string(),
    };
    // Unwrap is safe: `ListCursor` has no map keys and no non-UTF-8 bytes
    // for JSON serialization to fail on.
    let json = serde_json::to_vec(&cursor).expect("ListCursor is always serializable");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json)
}

/// Decode a caller-supplied cursor into its position and the filter digest
/// it is bound to, or `None` for anything malformed — invalid base64,
/// unparseable JSON, JSON of the wrong shape, or a token from a different
/// domain (a supervisor's wire cursor, an older or newer version of this
/// one).
///
/// Every failure mode collapses to the same `None` on purpose: a
/// bit-flipped byte, a truncated value, and a string from nowhere are
/// indistinguishable to an honest server, and there is no differently
/// actionable answer to give for any of them. Never panics on caller
/// input; `?` short-circuits through `Option` at every fallible step.
///
/// Note what this does NOT do: it does not judge the digest. Deciding
/// whether the bound filter is the request's filter belongs to
/// [`session_page`], which is the only place that knows what was asked.
fn decode_cursor(cursor: &str) -> Option<(store::CacheKey, String)> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .ok()?;
    let decoded: ListCursor = serde_json::from_slice(&bytes).ok()?;
    if decoded.fh != CURSOR_DOMAIN {
        return None;
    }
    Some((
        store::CacheKey {
            created_at: decoded.at,
            session_id: decoded.sid,
            host: decoded.h,
        },
        decoded.f,
    ))
}

/// How many distinct filters the matching-count cache remembers at once.
///
/// Sized for the shape it exists to serve rather than for a hit rate: a page
/// WALK, which is one filter held still across a handful of requests, plus
/// however many other surfaces happen to be open. A user typing in a search
/// box mints a new entry per keystroke and evicts them just as fast, which is
/// correct — those counts describe filters nobody is looking at any more.
const MATCHING_COUNT_CACHE: usize = 64;

/// The PERSISTED matching counts this helm has already computed, so a page
/// walk pays for one scan of helm.db rather than one per page.
///
/// ## Why the count is HERE and not in the cursor
///
/// The obvious cheap answer is to carry the number in the opaque cursor and
/// believe it on the way back. It is also wrong in a way that is easy to miss
/// and hard to see once shipped: a cursor is a string the client holds, base64
/// of JSON with nothing authenticating it, so a caller that edits the number
/// makes the server report whatever total it likes — larger than the fleet,
/// or large enough to overflow the addition of the live-host component.
/// Nothing in a reply would look wrong. Keeping the count on this side means
/// the only thing a caller can influence is whether it gets a cached answer
/// or a fresh one, and both are honest.
///
/// ## What is cached, and what deliberately is NOT
///
/// Only helm.db's share of the count, keyed by the filter's digest and
/// qualified by the STORE GENERATION it was taken at. The identity-less hosts'
/// share is never cached at all: it is recounted on every request from the
/// same snapshot the page's live rows are taken from, so the number reported
/// and the rows returned cannot describe different moments.
///
/// The generation is not checked here. It is handed back to the store and
/// compared inside the lock hold that produces the page
/// ([`store::MatchingCount::ComputeUnless`]), because any comparison made out
/// here can have a commit land between it and the read.
///
/// An earlier shape keyed entries by the FLEET REVISION too and cached the two
/// components summed. That is unsound and worth remembering rather than
/// rediscovering: the revision is bumped after the change it describes is
/// already visible, so a request can sample the old revision, merge the new
/// in-memory rows, and hit an entry whose live component predates them —
/// while the store's generation, which never saw any of it, happily confirms.
/// See this module's docs.
///
/// ## In memory, and empty after a restart
///
/// Deliberately: the filter digests it is keyed by are minted with a
/// process-random key (see [`store::SessionFilter::digest`]) and the store
/// generation restarts at zero, so nothing here could be carried across a
/// restart without inventing a way for stale numbers to look fresh. An empty
/// cache costs one count.
#[derive(Default)]
pub(crate) struct MatchingCounts {
    /// Most-recently-used first, evicted from the back. A `Vec` scan rather
    /// than a map plus a recency list: at [`MATCHING_COUNT_CACHE`] entries the
    /// scan is a handful of string compares against work measured in whole
    /// table decodes, and the ordering IS the eviction policy.
    entries: std::sync::Mutex<Vec<CountEntry>>,
}

/// One remembered PERSISTED matching count, with what qualifies it.
#[derive(Clone)]
struct CountEntry {
    /// The filter this count is about ([`store::SessionFilter::digest`]).
    digest: String,
    /// The store generation it was computed at — passed back to the store to
    /// be validated under its own lock, never compared here.
    generation: u64,
    /// How many CACHED rows matched. Never a merged total: the live component
    /// is not cacheable (see [`MatchingCounts`]).
    persisted: u64,
}

impl MatchingCounts {
    /// The persisted count remembered for this filter, if any.
    ///
    /// A hit is a CANDIDATE, not an answer: the store still has to confirm
    /// that its own generation has not moved. Promoted to the front on the
    /// way out, so a walk in progress cannot be evicted by unrelated traffic.
    fn get(&self, digest: &str) -> Option<CountEntry> {
        let mut entries = self.entries.lock().expect("matching count cache poisoned");
        let found = entries.iter().position(|entry| entry.digest == digest)?;
        let entry = entries.remove(found);
        entries.insert(0, entry.clone());
        Some(entry)
    }

    /// Remember a freshly computed persisted count, evicting the least
    /// recently used entry once the cache is full.
    ///
    /// Replaces any entry for the same filter rather than adding a second:
    /// two counts for one question would differ only in how stale they are,
    /// and a lookup has no way to prefer the fresher one.
    fn remember(&self, digest: &str, generation: u64, persisted: u64) {
        let mut entries = self.entries.lock().expect("matching count cache poisoned");
        entries.retain(|entry| entry.digest != digest);
        entries.insert(
            0,
            CountEntry {
                digest: digest.to_string(),
                generation,
                persisted,
            },
        );
        entries.truncate(MATCHING_COUNT_CACHE);
    }
}

/// The total order pages walk: creation time DESCENDING, then session id
/// ascending, then host id ascending.
///
/// `Reverse` on `created_at` turns "descending time, ascending id" into one
/// ordinary ascending comparison, so a resume point is a plain `>` rather
/// than a hand-written three-field comparison repeated at every call site.
/// This must agree EXACTLY with the `ORDER BY` in
/// [`HelmStore::scan_page`]: the page comes back sorted by SQL and the
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
    /// Drop this source's leading LIVE entries that the filter excludes, so
    /// everything a merge peeks at or takes is a row the caller asked for.
    ///
    /// Only the live side needs this: the persisted side was already
    /// filtered inside its own scan ([`HelmStore::scan_page`]), which is
    /// where a filter belongs for rows SQLite is walking anyway. Doing it
    /// here rather than inside `peek`/`take` keeps both of those honest —
    /// `peek` stays a pure look at the front — and it is what makes the
    /// merge's ordering argument survive filtering: a source whose front
    /// item is excluded would otherwise win the `min` comparison and then
    /// yield nothing.
    ///
    /// The bound the merge otherwise enjoys (work proportional to the page,
    /// not to the fleet) is genuinely weakened here: skipping is
    /// proportional to how many non-matching entries a filter steps over. No
    /// arrangement avoids that — the rows are in memory, unindexed, and
    /// "which ones match" is the question being asked.
    fn advance(&mut self, filter: &store::SessionFilter) {
        let Some(snapshot) = self.snapshot else {
            return;
        };
        let skip = self
            .live
            .iter()
            .take_while(|info| !filter.matches(snapshot.id, info))
            .count();
        self.live = &self.live[skip..];
    }

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
/// page ([`HelmStore::merged_page`]): the resume predicate, the row limit,
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
///
/// ## A cursor and its filter travel together, or not at all
///
/// Every cursor is bound to the filter it was minted under, and a request
/// that replays one under a different filter — a changed parameter, a cleared
/// one, an added one — is REFUSED with a 400 telling the caller to start a
/// fresh walk. The position is meaningless outside its own result set:
/// applied to a different one it resumes somewhere in the middle, and every
/// match that sorts before that point is silently dropped from the walk. That
/// failure is invisible to the client, which is exactly why it is worth a
/// refusal rather than a best effort.
///
/// The binding is on unfiltered cursors too. "No filter" is a filter's worth
/// of meaning here — it is the widest possible result set — and a walk
/// resumed after a filter was cleared skips just as much as one resumed after
/// it was tightened.
///
/// ## Filtering, and the counts
///
/// `filter` narrows BOTH sources by the same predicate ([`store::SessionFilter`],
/// which lives in the store precisely so the two cannot come to disagree),
/// and it applies before the page cut on each of them: the persisted side
/// filters inside its own scan, the live side by skipping non-matching
/// entries as the merge walks. A filtered reply therefore carries two
/// counts — `matching` for the filter, `total` for the fleet — because "N
/// matching of M sessions" needs both, and a client cannot derive either
/// from a page it was handed. An unfiltered reply carries only `total` and
/// claims nothing about matching (see [`SessionPageBody::matching`]).
///
/// The page and its totals come from ONE store read
/// ([`HelmStore::merged_page`]), so they describe one moment: taken
/// separately, a refresh landing between them can report more matches than
/// the fleet holds. The live side is read from ONE snapshot for the same
/// reason.
///
/// ## helm.db's share of the matching count is computed once per WALK
///
/// See [`MatchingCounts`] for the whole arrangement, and this module's docs
/// for why the count is held here rather than carried in the cursor. What
/// happens per request is short: the cached count for this filter names the
/// store generation it was taken at, and the store decides under its own lock
/// whether that generation still stands. A store that confirms costs nothing;
/// a store that recounts replaces the entry.
///
/// The LIVE share is recounted every time, and that is not an oversight. Those
/// rows exist only in the manager's memory, so no store generation can qualify
/// a count of them, and the fleet revision cannot either — it is published
/// after the rows it describes are already visible. Recounting from the very
/// snapshot this page's live rows are merged from is what makes "N matching"
/// and the rows beside it one answer rather than two. The price is a filter
/// evaluation per in-memory row per page, over a list bounded by
/// `crate::manager::REFRESH_SESSION_CAP` and already held by this request.
///
/// Every addition of two counts saturates. They are `u64` totals over a
/// bounded cache and cannot realistically approach the ceiling, but "cannot
/// realistically" is not a reason for a reply to be able to wrap.
pub(crate) async fn session_page(
    manager: &ConnectionManager,
    store: &HelmStore,
    counts: &MatchingCounts,
    cursor: Option<&str>,
    limit: usize,
    filter: &store::SessionFilter,
) -> anyhow::Result<SessionPageBody> {
    session_page_staged(
        manager,
        store,
        counts,
        cursor,
        limit,
        filter,
        std::future::ready(()),
    )
    .await
}

/// [`session_page`], with a seam where a test can stage a concurrent fleet
/// mutation.
///
/// `staged` is awaited after the cursor is decoded and BEFORE anything about
/// the fleet is sampled, which makes it the barrier the live-count coherence
/// property is stated against: whatever happens inside it is entirely in this
/// request's past, so every number the reply carries must describe the world
/// after it. Production passes a ready future, so the seam costs a poll.
///
/// It is placed here rather than deeper on purpose. The shape this replaced
/// sampled the fleet revision above this point and keyed a cached count by it,
/// which is exactly the arrangement a mutation landing at this barrier
/// defeated (see [`MatchingCounts`]) — so a future edit that reintroduces a
/// pre-snapshot sample of anything fleet-wide fails the test that stands here.
async fn session_page_staged(
    manager: &ConnectionManager,
    store: &HelmStore,
    counts: &MatchingCounts,
    cursor: Option<&str>,
    limit: usize,
    filter: &store::SessionFilter,
    staged: impl std::future::Future<Output = ()>,
) -> anyhow::Result<SessionPageBody> {
    let digest = filter.digest();
    let after = match cursor {
        None => None,
        Some(raw) => {
            let (key, bound) = decode_cursor(raw).ok_or_else(malformed_cursor)?;
            if bound != digest {
                return Err(cursor_filter_changed());
            }
            Some(key)
        }
    };
    staged.await;
    // ONE snapshot, for the rows, the scope, and the live matching count
    // alike. Nothing fleet-wide is sampled before it, and nothing this reply
    // says about the live hosts comes from anywhere else — which is what makes
    // the count and the rows one answer.
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

    // What this helm already knows about how many CACHED rows match, if
    // anything. A hit is only a candidate — it names the store generation it
    // was taken at, and the store is what decides whether that still holds.
    let held = (!filter.is_empty()).then(|| counts.get(&digest)).flatten();
    let read = store
        .merged_page(
            scope,
            after.clone(),
            limit,
            PAGE_BYTE_BUDGET,
            filter.clone(),
            match (filter.is_empty(), &held) {
                // An unfiltered listing makes no matching claim, so there is
                // nothing to count.
                (true, _) => store::MatchingCount::Skip,
                (false, Some(held)) => store::MatchingCount::ComputeUnless(held.generation),
                (false, None) => store::MatchingCount::Compute,
            },
        )
        .await?;
    let persisted = read.page;
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
    let mut live_matching: u64 = 0;
    for snapshot in &snapshots {
        let Some(live) = snapshot.live_sessions.as_ref() else {
            continue;
        };
        // Every identity-less host counts toward the FLEET total, host
        // filter or not: `total` is what the fleet holds, and narrowing it
        // to the filter's own scope would make "N matching of M" compare a
        // number against itself.
        live_total = live_total.saturating_add(live.len() as u64);
        // A host the filter excludes contributes no matches and no rows, so
        // it is skipped whole rather than walked and rejected item by item.
        if filter
            .host_scope()
            .is_some_and(|wanted| wanted != snapshot.id)
        {
            continue;
        }
        // Counted over the WHOLE list rather than from the resume point:
        // `matching` describes the fleet, not this page, so a walk already
        // three pages in must still report the same number.
        //
        // And counted UNCONDITIONALLY, out of the same snapshot the rows
        // below are taken from. Reusing a remembered live count is what an
        // earlier shape did, keyed by the fleet revision, and the revision
        // moves only after the list it describes is already visible — so the
        // reuse could pair a stale count with fresh rows in the same reply.
        // The cost is one filter evaluation per in-memory row per page,
        // bounded by `REFRESH_SESSION_CAP` per host, and it is what makes the
        // count true of the rows rather than true of a moment nearby.
        if !filter.is_empty() {
            live_matching = live_matching.saturating_add(
                live.iter()
                    .filter(|info| filter.matches(snapshot.id, info))
                    .count() as u64,
            );
        }
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
        // Non-matching live entries are dropped before anything is compared:
        // a source whose front item is excluded would otherwise win the
        // comparison below and hand back nothing.
        for source in &mut sources {
            source.advance(filter);
        }
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
    //
    // It carries the filter's digest and nothing else. Everything a later
    // page needs to know about the COUNT lives on this side.
    let next_cursor = more
        .then(|| taken.last().map(|item| encode_cursor(&item.key, &digest)))
        .flatten();
    let sessions: Vec<SessionRow> = taken.into_iter().filter_map(|item| item.row).collect();
    // Only the PERSISTED component is remembered; the live one is added on
    // the way out, freshly counted above, on both the hit and the miss path.
    let matching = match (filter.is_empty(), read.matching) {
        (true, _) => None,
        (false, Some(counted)) => {
            counts.remember(&digest, read.generation, counted);
            Some(counted.saturating_add(live_matching))
        }
        // The store confirmed its generation had not moved, so the cached
        // count of CACHED rows still stands. `held` is `Some` whenever the
        // store was handed a generation to check, which is the only way it
        // answers `None` for a filtered read.
        (false, None) => held.map(|held| held.persisted.saturating_add(live_matching)),
    };
    Ok(SessionPageBody {
        sessions,
        total: read.total.saturating_add(live_total),
        matching,
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

/// The refusal a cursor replayed under a DIFFERENT filter produces.
///
/// Named separately from [`malformed_cursor`] because the caller's mistake is
/// different and so is the fix: the token is perfectly good, it just belongs
/// to another walk. Answering it would resume mid-order through a result set
/// it never described and silently drop every earlier match.
fn cursor_filter_changed() -> anyhow::Error {
    anyhow::Error::new(crate::SupervisorError {
        kind: farhelm_proto::ErrorKind::InvalidRequest,
        message: "this session list cursor was issued for a different filter; a resume point \
                  only means something within the result set it came from, so changing, \
                  clearing or adding a filter parameter requires a fresh walk with no cursor"
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
                creation_seq: None,
                parent: None,
                archived: false,
                id: id.to_string(),
                title: id.to_string(),
                created_at,
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::Running,
                annotation: None,
                restart_offer: RestartOffer::default(),
                tabs: Vec::new(),
                source_profile: None,
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
            let digest = store::SessionFilter::default().title("drain").digest();
            let (decoded, bound) =
                decode_cursor(&encode_cursor(&key, &digest)).expect("our own cursor decodes");
            assert_eq!(decoded, key);
            assert_eq!(
                bound, digest,
                "the filter binding must survive the round trip beside the position"
            );
        }
    }

    /// A cursor carries a POSITION and a filter binding, and no count of any
    /// kind.
    ///
    /// This is the property that makes the token unable to lie. A matching
    /// count carried in a cursor is a number the client supplies — the token
    /// is base64 of JSON with nothing authenticating it — so a server that
    /// believed one would report whatever total the caller wrote into it.
    /// Pinned on the encoded bytes rather than on the decoder, because the
    /// decoder cannot show what was never put in.
    #[test]
    fn a_cursor_carries_no_count_a_caller_could_edit() {
        use base64::Engine;

        let token = encode_cursor(
            &store::CacheKey {
                created_at: 1_700_000_000,
                session_id: "sess-1".to_string(),
                host: 3,
            },
            &store::SessionFilter::default().title("drain").digest(),
        );
        let json: serde_json::Value = serde_json::from_slice(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(&token)
                .expect("our cursor is base64url"),
        )
        .expect("our cursor is JSON");
        // Sorted, because `serde_json`'s object is a map rather than a
        // sequence here — the SET of fields is what this is about, not the
        // order they were written in.
        let fields: Vec<&str> = json
            .as_object()
            .expect("a cursor is a JSON object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            fields,
            vec!["at", "f", "fh", "h", "sid"],
            "a cursor carries the domain tag, the ordering key and the filter binding — a count \
             among these would be a number the caller gets to choose"
        );
    }

    /// A cursor whose filter binding does not match the request's filter is
    /// REFUSED by the decoder's caller, and the refusal is distinguishable
    /// from a malformed token.
    ///
    /// The digest is a keyed hash of the filter's canonical encoding, so this
    /// also pins that two different filters cannot share a binding — which is
    /// what the refusal rests on.
    #[test]
    fn a_cursor_is_bound_to_the_filter_it_was_minted_under() {
        let unfiltered = store::SessionFilter::default();
        let filtered = store::SessionFilter::default().title("drain");
        let narrower = store::SessionFilter::default()
            .title("drain")
            .status("idle");

        assert_ne!(
            unfiltered.digest(),
            filtered.digest(),
            "an unfiltered walk is its own result set, and a cursor from one must not replay in \
             the other"
        );
        assert_ne!(filtered.digest(), narrower.digest());
        assert_eq!(
            filtered.digest(),
            store::SessionFilter::default().title("drain").digest(),
            "the same filter must recognize its own cursors, or no walk could ever continue"
        );

        let key = store::CacheKey {
            created_at: 1,
            session_id: "s".to_string(),
            host: 1,
        };
        let (_, bound) = decode_cursor(&encode_cursor(&key, &filtered.digest()))
            .expect("our own cursor decodes");
        assert_ne!(
            bound,
            unfiltered.digest(),
            "and the binding a decoded cursor reports is the one it was minted with"
        );
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

        let helm = encode_cursor(
            &store::CacheKey {
                created_at: 1_700_000_000,
                session_id: "sess-1".to_string(),
                host: 1,
            },
            &store::SessionFilter::default().digest(),
        );
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
    ///
    /// A version-1 token is in the table because the domain tag is the ONLY
    /// thing standing between the old shape and this one: v1 cursors carried
    /// a matching count and no filter binding, and a decoder that shrugged at
    /// the extra fields would resume a walk with neither guarantee in place.
    #[test]
    fn a_malformed_or_foreign_cursor_decodes_to_nothing() {
        use base64::Engine;

        let encode = |value: serde_json::Value| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value.to_string())
        };
        let foreign_domain = encode(
            serde_json::json!({"fh": "farhelm/helm-sessions/3", "at": 1, "sid": "s",
                                      "h": 1, "f": "0000000000000000"}),
        );
        let superseded_version = encode(
            serde_json::json!({"fh": "farhelm/helm-sessions/1", "at": 1, "sid": "s",
                                      "h": 1, "m": 99, "r": 1, "f": "h-;d-;t-;p-;s-;"}),
        );
        let extra_field = encode(serde_json::json!({"fh": CURSOR_DOMAIN, "at": 1, "sid": "s",
                                                    "h": 1, "f": "0000000000000000", "x": 1}));
        // The binding is REQUIRED, not defaulted: a token without one names
        // no result set, and inventing "the unfiltered walk" for it would
        // resume exactly the way this refusal exists to prevent.
        let no_binding = encode(serde_json::json!({"fh": CURSOR_DOMAIN, "at": 1, "sid": "s",
                                                   "h": 1}));
        for malformed in [
            "not-base64!!",
            "",
            "YWJj",
            "e30",
            foreign_domain.as_str(),
            superseded_version.as_str(),
            extra_field.as_str(),
            no_binding.as_str(),
        ] {
            assert!(
                decode_cursor(malformed).is_none(),
                "cursor {malformed:?} must not decode"
            );
        }
    }

    /// The matching-count cache is BOUNDED and evicts by recency, so a walk
    /// in progress cannot be pushed out by unrelated traffic.
    ///
    /// Spec: at capacity, one more entry evicts the least recently USED one —
    /// not the least recently written — and a `get` is what counts as use.
    ///
    /// Both halves matter and neither is visible from the outside. Without the
    /// bound, a search box mints an entry per keystroke and this map grows for
    /// the life of the process. Without recency, a walk's own entry is evicted
    /// by whatever a second browser tab happened to type, and every later page
    /// of that walk rescans the whole cache — the exact cost the entry exists
    /// to avoid.
    #[test]
    fn the_count_cache_is_bounded_and_evicts_the_least_recently_used_entry() {
        let counts = MatchingCounts::default();
        for n in 0..MATCHING_COUNT_CACHE {
            counts.remember(&format!("digest-{n}"), 1, n as u64);
        }
        assert_eq!(
            counts.entries.lock().expect("cache mutex").len(),
            MATCHING_COUNT_CACHE,
            "a full cache holds exactly its capacity"
        );

        // Reading the OLDEST entry makes it the newest, which is the whole
        // difference between insertion order and recency order.
        assert_eq!(
            counts.get("digest-0").map(|entry| entry.persisted),
            Some(0),
            "every entry is still there before anything is evicted"
        );
        counts.remember("digest-fresh", 1, 999);

        assert_eq!(
            counts.entries.lock().expect("cache mutex").len(),
            MATCHING_COUNT_CACHE,
            "one insertion past capacity evicts exactly one entry"
        );
        assert!(
            counts.get("digest-1").is_none(),
            "the entry nobody has touched since it was written is the one that goes"
        );
        assert_eq!(
            counts.get("digest-0").map(|entry| entry.persisted),
            Some(0),
            "and the entry a walk read moments ago survives the insertion that displaced it"
        );
        assert_eq!(
            counts.get("digest-fresh").map(|entry| entry.persisted),
            Some(999)
        );
    }

    /// A change to an identity-less host's in-memory sessions, landing in a
    /// request's PAST, is counted by the very page that returns those rows.
    ///
    /// Spec: with a cached persisted count already standing for this filter,
    /// a page whose live rows changed before the request sampled anything
    /// reports a `matching` that agrees with the rows it carries.
    ///
    /// This is the regression the count cache's shape was rebuilt around, and
    /// it is invisible in any single number taken alone. The cache used to be
    /// keyed by the FLEET REVISION and to hold both components summed; the
    /// revision is bumped only after the new list is already published
    /// (`manager`'s `HostActor::publish_refresh`), so a request could sample
    /// the old revision, merge the NEW rows, and hit an entry whose live
    /// component described the old ones — the store's generation confirming
    /// happily, because none of it ever touched the store. The reply then
    /// carried three rows beside "2 matching", which no client can detect.
    ///
    /// The mutation is staged through [`session_page_staged`]'s seam rather
    /// than raced against a real request: the window it exercises is a few
    /// instructions wide and would otherwise reproduce only under a
    /// multi-threaded scheduler on a bad day. Staging it makes the assertion
    /// deterministic AND keeps the barrier standing over the code — anything
    /// that starts sampling the fleet before that point fails here.
    #[tokio::test]
    async fn a_live_change_before_the_snapshot_is_counted_by_the_page_that_returns_it() {
        use crate::rest_harness;

        let (builder, _cached) = rest_harness::FleetBuilder::new()
            .await
            .local(rest_harness::HostScript {
                // No identity: this host caches nothing, so its rows and its
                // share of the count come from the manager's memory.
                identity: None,
                sessions: vec![rest_harness::session("live-keep-1", 300)],
                ..rest_harness::HostScript::default()
            })
            .await
            .ssh(
                "user@cached",
                rest_harness::HostScript {
                    identity: Some("identity-cached".to_string()),
                    sessions: vec![rest_harness::session("cached-keep-1", 400)],
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        let local = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(local).await;
        harness.await_refreshed(_cached).await;

        let filter = store::SessionFilter::default().title("keep");
        // The first walk is what puts a persisted count in the cache; without
        // one, the second would recount and could not be wrong.
        let first = session_page(
            &harness.manager,
            &harness.store,
            &harness.state.counts,
            None,
            50,
            &filter,
        )
        .await
        .expect("the first page reads");
        assert_eq!(first.sessions.len(), 2);
        assert_eq!(first.matching, Some(2));

        let second = session_page_staged(
            &harness.manager,
            &harness.store,
            &harness.state.counts,
            None,
            50,
            &filter,
            async {
                harness.fleet.edit(local, |script| {
                    script
                        .sessions
                        .push(rest_harness::session("live-keep-2", 200));
                });
                harness.manager.refresh_now(local);
                harness
                    .await_state(local, |state| {
                        matches!(
                            state,
                            crate::manager::HostState::Connected {
                                last_refresh: crate::manager::RefreshHealth::Ok { sessions: 2 },
                                ..
                            }
                        )
                    })
                    .await;
            },
        )
        .await
        .expect("the second page reads");

        assert_eq!(
            second.sessions.len(),
            3,
            "the new in-memory session is on the page"
        );
        assert_eq!(
            second.matching,
            Some(second.sessions.len() as u64),
            "and the matching count describes the rows it is reported beside"
        );
        assert_eq!(
            second.total, 3,
            "the fleet total counts the same three sessions"
        );
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
