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
//! ## The order, and the three of them
//!
//! Creation time descending, session id ascending, host id ascending — the
//! same total order the wire uses (`farhelm-supervisor`'s `list_order_key`)
//! with one extra component, so a merged list reads as one list rather than
//! as concatenated per-host ones. helm.db carries that key as an index, so a
//! page is an index range scan rather than a sort — one branch of the scan per
//! host, merged, for the reason [`HelmStore::scan_page`] gives.
//!
//! The host id is in the key even though session ids are unique: it makes
//! the order TOTAL unconditionally, and a cursor over a non-total order can
//! skip or repeat rows. See [`store::CacheKey`].
//!
//! That order is now one of three ([`store::ListSort`]): a caller may ask for
//! recent activity or for title instead, and each of those ends in this exact
//! creation-order tail so it is total for the same reason. What changes with
//! the sort is which index the persisted scan walks and how the in-memory
//! rows below are ordered before they are merged into it; what does not
//! change is anything about WHICH rows the view holds, so neither count moves.
//!
//! The sort is a property of THIS merged view and of nothing underneath it.
//! Hosts go on reporting their sessions in creation order, the drain goes on
//! validating that (`crate::manager::drain_sessions`), and the cache stores
//! what the drain brought — an in-memory list is therefore in creation order
//! whatever the request asked for, which is exactly why the merge below
//! re-orders it per request rather than assuming it arrives sorted.
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
//! how many matched, and how many the VIEW holds. The second is not
//! redundant — it is the number the list's own coherence check has always
//! described, and a filtered list showing fewer rows than the view holds is
//! a working filter rather than a missing page.
//!
//! "The view" rather than "the fleet" because one dimension is not a
//! narrowing at all: the archive switch decides which list is being served,
//! so `total` follows it and follows nothing else (see
//! [`SessionPageBody::total`]).
//!
//! An UNFILTERED reply carries only that total and makes no matching
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
///
/// The flattening is also why a new `SessionInfo` field needs nothing here to
/// reach a client: `last_activity_at` is on every row already, so a client can
/// show recent activity without a second request or a row shape of its own.
///
/// What travels is the RAW field, exactly as its supervisor reported it —
/// `0` and all. `?sort=activity` does NOT order by that value; it orders by
/// the EFFECTIVE one, `SessionInfo::effective_activity`, which reads `0` as
/// "this sender never told us" and falls back to `created_at`. Serving the raw
/// field is deliberate (a synthesized value written into it would be
/// indistinguishable from an observation at the next merge — see
/// `crate::manager::merge_cached_session`), and it puts one obligation on the
/// client: a row rendered from `last_activity_at` directly will show 1970 for
/// exactly the sessions the list sorted by their creation time. Apply
/// `effective_activity` when displaying the stamp, or the rendering and the
/// order disagree on the same row.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct SessionRow {
    #[serde(flatten)]
    pub(crate) info: SessionInfo,
    /// The registry id of the host this session lives on — the value a
    /// client hands back to `POST /api/sessions` to create alongside it,
    /// and the join key into `GET /api/hosts`.
    pub(crate) host: HostId,
    /// The install identity the REGISTRY held for that host when the reply
    /// was being built (`HostView::identity`'s value), denormalized onto
    /// the row so a client can bind "the session the user is looking at"
    /// to the INSTALL behind it rather than to the row id alone. "Was
    /// being built" is deliberate: the identity is sampled beside the
    /// session content, not atomically with it, and can be one write stale
    /// by the time the reply lands — the assembly orders its reads so that
    /// such skew mismatches (and the client falls back safely) rather than
    /// falsely matching; see the identity join in `session_page_staged`. A `HostId` outlives
    /// retargets and adoptions while the machine behind it changes; this
    /// field is what lets the create dialog's host default notice that the
    /// install it was derived from is no longer the one the row id now
    /// names (the #156-review residual).
    ///
    /// `None` means the registry records no identity for the host (never
    /// reached, or it reports none) — and the key is ALWAYS serialized, so
    /// a client can tell "this helm says no identity" (`null`) from "this
    /// helm predates the field" (key absent) and degrade to the old
    /// row-id-only behavior only in the latter case.
    pub(crate) host_identity: Option<String>,
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
    /// page's cut and before any of the caller's search dimensions — not one
    /// supervisor's count, and not a count of what this page holds. That is
    /// the number SPEC.md's "showing N of M" is about once a fleet has more
    /// than one host in it, and it is deliberately the one the list's
    /// coherence check has always been made against (PLAN_M6_75.md item 5):
    /// a filtered page showing fewer rows than the view holds is not an
    /// incoherent list, it is a working filter.
    ///
    /// WHICH VIEW is the request's archive switch, and that one dimension
    /// does move this number: with the switch off — the public default —
    /// archived rows are outside the view and outside this count; with it on
    /// this is the whole fleet. The switch is not a narrowing the user
    /// applied, it is which list they are looking at, and a denominator that
    /// ignored it left the ordinary list showing ten rows above "of 12
    /// sessions" (maintainer's verdict, 2026-08-22; `store::count_rows` has
    /// the full reasoning).
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
/// ## The leading key is now MUTABLE, and that is a real weakening
///
/// A keyset cursor is stable against inserts and deletes, but only because
/// the key it names does not move. That was unconditionally true while
/// `created_at` was the leading component: a session's creation time never
/// changes. It is NOT true of the other two orders. A session that produces
/// output advances its activity stamp, and a rename rewrites its folded
/// title, so a row can cross the cursor between two page fetches of one
/// walk — appearing on a later page it was already shown on, or moving to
/// the far side of the resume point and never being shown at all.
///
/// Nothing here prevents that, deliberately. The fix a database would reach
/// for is a snapshot or a generation the walk pins itself to, which means
/// holding server state per in-flight walk and deciding when to expire it,
/// for a list a user re-reads constantly anyway. What stands instead is two
/// things that were already there: the activity stamp moves on a coarse
/// quantum rather than per byte of output (the supervisor advances it at most
/// once per `ACTIVITY_STAMP_QUANTUM`, a whole minute), which bounds how often
/// a row can cross at all; and the UI re-reads the whole list when its own
/// coherence checks trip, saying so on screen — "the list changed while it was
/// being read; refreshing" (`crates/farhelm-ui/src/rows.rs`). A duplicated or
/// missed row under an activity or title walk is a stale read of a list that
/// is visibly moving, not a lost session.
///
/// ## What it carries, and what it deliberately does not
///
/// A position, the ORDER that position is a position in, and the filter it
/// was taken under. Nothing else — in particular no COUNT. A cursor is a
/// string the client holds, so a count carried in one is a count the client
/// picks, and every number the reply derives from it becomes a number the
/// caller dictated to the server about its own fleet. The matching count is
/// kept server-side instead ([`MatchingCounts`]), where the only thing a
/// client can influence is whether a cache entry is hit.
///
/// ## A position is a position IN AN ORDER
///
/// The sort travels with the position for the same reason the filter digest
/// does, and the failure it prevents is the same shape: a resume point from a
/// title-ordered walk applied to an activity-ordered one names a place in a
/// sequence it never described, so the walk resumes mid-list and drops
/// everything before it with no error a client could see. Hence [`Self::s`],
/// compared in [`session_page`].
///
/// Only the components the sort actually compares are carried
/// ([`Self::la`], [`Self::t`]): a creation-ordered cursor omits both, so it
/// stays FIXED-SIZE — bounded by the session id, like every version of this
/// token — even though version 3 grew it by the order word and the longer
/// domain tag. A title-ordered one pays for the title only where the title is
/// the thing being ordered by.
///
/// That last one is the only order whose cursor grows with peer-supplied text,
/// and what bounds it is [`store::title_sort_key`]'s own truncation
/// ([`store::TITLE_SORT_KEY_CHARS`] folded characters) rather than anything the
/// peer chose — the supervisor's create-field cap is 64 KiB, which is a
/// request-body ceiling and no bound at all for a value that has to fit in a
/// query string. Truncating is sound ONLY because it happens where the key is
/// minted, so the cut value is the row's actual position under the title order
/// rather than a shortened stand-in for it; truncating here instead would name
/// a position the order does not contain, and the walk would skip or repeat
/// rows.
#[derive(Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ListCursor {
    /// The domain tag; only [`CURSOR_DOMAIN`] is accepted.
    fh: String,
    /// The order this position was taken in, as [`store::ListSort::key`]
    /// spells it.
    s: String,
    /// `created_at`, descending. Present under every order — it leads the
    /// creation order and is the first tiebreak of the other two.
    at: i64,
    /// Session id, ascending.
    sid: String,
    /// Host id, ascending — the component that makes every order total
    /// even where the one-owner index is absent (see [`store::CacheKey`]).
    h: HostId,
    /// Effective activity, descending — present exactly under
    /// [`store::ListSort::Activity`], and REQUIRED there: a token that named
    /// that order without carrying its leading component would resume at an
    /// activity of zero, which is a real position in the order rather than an
    /// absence of one. Where that position falls differs per order — the
    /// bottom of a descending stamp, the top of an ascending title — so the
    /// damage is a silent skip or a silent repeat depending on which, and the
    /// refusal is what makes it neither.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    la: Option<i64>,
    /// The collated title, ascending — present exactly under
    /// [`store::ListSort::Title`], and required there for [`Self::la`]'s
    /// reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    t: Option<String>,
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
///
/// Version 3 is the shape that names its ORDER. A version-2 token predates
/// the choice, so every one of them was minted in the creation order — but
/// reading them as such would be a compatibility gesture with a real cost:
/// the field would have to be optional forever, and every later reader would
/// have to keep deciding what an absent order means. They are rejected
/// instead, which costs a walk in progress across a helm upgrade one
/// re-fetch. Cursors already do not survive a restart (the filter digest is
/// keyed per process), so this adds no failure mode that was not there.
const CURSOR_DOMAIN: &str = "farhelm/helm-sessions/3";

/// Encode one row's ordering key, bound to the order and the filter it was
/// taken under, into the opaque token [`SessionPageBody::next_cursor`]
/// carries.
///
/// Base64url-unpadded JSON: self-describing enough that [`decode_cursor`]
/// can reject a malformed value by construction (a JSON parse failure)
/// instead of by delimiter scanning, and free of characters that would need
/// escaping in the query string this token actually travels in.
///
/// Carries the components `sort` COMPARES and no others — see
/// [`ListCursor`]. The dropped components are not lost: nothing may ever
/// compare a position under an order other than its own, which is what the
/// sort binding enforces.
fn encode_cursor(key: &store::CacheKey, sort: store::ListSort, digest: &str) -> String {
    use base64::Engine;
    // ONE match over the order, so the two optional components are decided
    // together and by exhaustiveness: a fourth order added later cannot
    // silently mint a cursor carrying neither of them, which
    // [`decode_cursor`] would then refuse as missing its own leading key.
    let (la, t) = match sort {
        store::ListSort::Created => (None, None),
        store::ListSort::Activity => (Some(key.activity_at), None),
        store::ListSort::Title => (None, Some(key.title_sort.clone())),
    };
    let cursor = ListCursor {
        fh: CURSOR_DOMAIN.to_string(),
        s: sort.key().to_string(),
        at: key.created_at,
        sid: key.session_id.clone(),
        h: key.host,
        la,
        t,
        f: digest.to_string(),
    };
    // Unwrap is safe: `ListCursor` has no map keys and no non-UTF-8 bytes
    // for JSON serialization to fail on.
    let json = serde_json::to_vec(&cursor).expect("ListCursor is always serializable");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json)
}

/// Decode a caller-supplied cursor into its position, the order it names, and
/// the filter digest it is bound to, or `None` for anything malformed —
/// invalid base64, unparseable JSON, JSON of the wrong shape, a token from a
/// different domain (a supervisor's wire cursor, an older or newer version of
/// this one), an order word this build does not know, or an order whose own
/// leading component the token does not carry.
///
/// Every failure mode collapses to the same `None` on purpose: a
/// bit-flipped byte, a truncated value, and a string from nowhere are
/// indistinguishable to an honest server, and there is no differently
/// actionable answer to give for any of them. Never panics on caller
/// input; `?` short-circuits through `Option` at every fallible step.
///
/// The key it returns is filled out ONLY where the named order compares: the
/// components that order ignores come back as placeholders (`0`, `""`)
/// because the token never carried them. That is safe precisely because
/// [`session_page`] refuses a cursor whose order is not the request's, so a
/// placeholder can never reach a comparison that would read it.
///
/// Note what this does NOT do: it does not judge the digest or the sort.
/// Deciding whether either is the request's belongs to [`session_page`],
/// which is the only place that knows what was asked.
fn decode_cursor(cursor: &str) -> Option<(store::CacheKey, store::ListSort, String)> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .ok()?;
    let decoded: ListCursor = serde_json::from_slice(&bytes).ok()?;
    if decoded.fh != CURSOR_DOMAIN {
        return None;
    }
    let sort = store::parse_sort_key(&decoded.s)?;
    // Each order's leading component is REQUIRED under that order. A default
    // is not "no position": zero activity and the empty title are REAL
    // positions in their orders, so a token missing its own leading key would
    // resume at one of them and the walk would silently continue from a place
    // the caller never reached. Which rows that skips or repeats depends on
    // where the default falls relative to the walk, and the answer is not
    // fixed — the point is that it is arbitrary, not that it is late.
    let activity_at = match sort {
        store::ListSort::Activity => decoded.la?,
        store::ListSort::Created | store::ListSort::Title => 0,
    };
    let title_sort = match sort {
        store::ListSort::Title => decoded.t?,
        store::ListSort::Created | store::ListSort::Activity => String::new(),
    };
    Some((
        store::CacheKey {
            created_at: decoded.at,
            session_id: decoded.sid,
            host: decoded.h,
            activity_at,
            title_sort,
        },
        sort,
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

/// Tag one host's session with that host, for the merged list.
///
/// `identity` is the registry's recorded install identity for the host —
/// passed alongside the snapshot rather than read from it because the
/// snapshot cannot carry a fresh one: first contact writes the identity to
/// helm.db without the actor being reconciled, so a snapshot-side copy
/// would sit at `NULL` for most hosts most of the time (the same reasoning
/// `hosts`' module doc gives for joining the two reads in `host_views`).
fn row_of(host: &HostSnapshot, identity: Option<&str>, info: SessionInfo) -> SessionRow {
    SessionRow {
        info,
        host: host.id,
        host_identity: identity.map(str::to_string),
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
/// much work to a `limit=1` request as a host holding one. Cloning of the
/// SESSION happens per item TAKEN, never per item available.
///
/// "Already-ordered" is a precondition the two sources meet differently. The
/// persisted page is ordered by SQL, in the order the request asked for. A
/// live list arrives in the order its host reported it (creation time — see
/// this module's docs), so it is put into the requested order by the caller
/// before it becomes a source, which is also where each entry's ordering key
/// is computed once instead of at every comparison.
struct MergeSource<'a> {
    snapshot: Option<&'a HostSnapshot>,
    /// Remaining items, in order. For the persisted source these are
    /// already-decoded scan results; for a live source they are borrowed
    /// `SessionInfo`s, beside the key they occupy, cloned only when taken.
    persisted: std::collections::VecDeque<store::ScannedRow>,
    live: std::collections::VecDeque<(store::CacheKey, &'a SessionInfo)>,
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
        while self
            .live
            .front()
            .is_some_and(|(_, info)| !filter.matches(snapshot.id, info))
        {
            self.live.pop_front();
        }
    }

    /// This source's next position under `sort`, without consuming it.
    fn peek(&self, sort: store::ListSort) -> Option<store::OrderPosition<'_>> {
        if let Some(scanned) = self.persisted.front() {
            return Some(sort.position(&scanned.key));
        }
        self.live.front().map(|(key, _)| sort.position(key))
    }

    /// Take this source's next item, cloning only what is taken.
    ///
    /// `hosts` is consulted for the persisted source, whose rows can name
    /// any host in scope; a live source already knows its own.
    /// `identities` is the registry-identity join for [`row_of`] — keyed
    /// the same way, consulted for both source kinds.
    fn take(
        &mut self,
        hosts: &std::collections::HashMap<HostId, &HostSnapshot>,
        identities: &std::collections::HashMap<HostId, Option<String>>,
    ) -> Option<MergeItem> {
        let identity_of = |host: HostId| {
            identities
                .get(&host)
                .and_then(|identity| identity.as_deref())
        };
        if let Some(scanned) = self.persisted.pop_front() {
            let row = scanned.info.and_then(|info| {
                Some(row_of(
                    hosts.get(&scanned.key.host)?,
                    identity_of(scanned.key.host),
                    info,
                ))
            });
            return Some(MergeItem {
                key: scanned.key,
                row,
            });
        }
        let snapshot = self.snapshot?;
        let (key, info) = self.live.pop_front()?;
        Some(MergeItem {
            key,
            row: Some(row_of(snapshot, identity_of(snapshot.id), info.clone())),
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
/// That guarantee is WEAKER under the two new orders, and the difference is
/// worth stating rather than inheriting silently. It rested on the leading
/// key being immutable: `created_at` never changes, so a row cannot move
/// across a creation-ordered cursor no matter what happens to it. An activity
/// stamp advances and a title is renamed, so under those orders a row CAN
/// cross the resume point between two fetches of one walk and be shown twice
/// or not at all. See [`ListCursor`] for why no snapshot machinery stands
/// against that and what does instead.
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
/// ## A cursor and its ORDER travel together too
///
/// The same rule, for the same failure, one axis over: a resume point names a
/// place in a sequence, and re-sorting the list makes it a different sequence.
/// A cursor minted under one `sort` and replayed under another is refused
/// rather than applied, because applying it would resume in the middle of an
/// order it never described and drop every row before that point.
///
/// ## Ordering, and what it does not touch
///
/// `sort` selects which of [`store::ListSort`]'s orders this page is a page of.
/// It reaches the persisted scan as an `ORDER BY` and reaches the live side as
/// the order its in-memory rows are put into before the merge; it touches
/// neither count, because both count a SET and re-ordering a set does not
/// resize it. The matching-count cache is keyed by the FILTER alone for the
/// same reason — a walk that changes sort keeps a valid count and pays only
/// for a fresh page.
///
/// ## Filtering, and the counts
///
/// `filter` narrows BOTH sources by the same predicate ([`store::SessionFilter`],
/// which lives in the store precisely so the two cannot come to disagree),
/// and it applies before the page cut on each of them: the persisted side
/// filters inside its own scan, the live side by skipping non-matching
/// entries as the merge walks. A filtered reply therefore carries two
/// counts — `matching` for the filter, `total` for the view it narrowed —
/// because "N matching of M sessions" needs both, and a client cannot derive
/// either from a page it was handed. Which view `total` counts is the
/// archive switch's to say and no other dimension's (see
/// [`SessionPageBody::total`]). An unfiltered reply carries only `total` and
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
    sort: store::ListSort,
) -> anyhow::Result<SessionPageBody> {
    session_page_staged(
        manager,
        store,
        counts,
        cursor,
        limit,
        filter,
        sort,
        std::future::ready(()),
        std::future::ready(()),
    )
    .await
}

/// [`session_page`], with seams where a test can stage a concurrent fleet
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
///
/// `staged_between_reads` is awaited between the registry-identity read and
/// the fleet snapshot — the window where a retarget skews the identity join.
/// The read order makes that skew fail SAFE (see the comment at the identity
/// join), and the test standing on this seam is what keeps a refactor from
/// quietly swapping the reads back into the dangerous order.
#[allow(clippy::too_many_arguments)]
async fn session_page_staged(
    manager: &ConnectionManager,
    store: &HelmStore,
    counts: &MatchingCounts,
    cursor: Option<&str>,
    limit: usize,
    filter: &store::SessionFilter,
    sort: store::ListSort,
    staged: impl std::future::Future<Output = ()>,
    staged_between_reads: impl std::future::Future<Output = ()>,
) -> anyhow::Result<SessionPageBody> {
    let digest = filter.digest();
    let after = match cursor {
        None => None,
        Some(raw) => {
            let (key, bound_sort, bound) = decode_cursor(raw).ok_or_else(malformed_cursor)?;
            if bound != digest {
                return Err(cursor_filter_changed());
            }
            // Checked separately from the digest, and reported separately,
            // because the caller's mistake is a different one: the filter is
            // right and the ORDER moved under it. A merged answer would tell
            // a client that changed its sort control to go and check its
            // filters.
            if bound_sort != sort {
                return Err(cursor_sort_changed());
            }
            Some(key)
        }
    };
    staged.await;
    // The registry-identity join for `SessionRow::host_identity` — a second
    // read beside the snapshot, on the same two-reads-joined terms as
    // `hosts::host_views` and for the same reason (see `row_of`): helm.db is
    // the authority for recorded identity, and the snapshot cannot carry a
    // fresh copy. Deliberately OUTSIDE the one-snapshot coherence claim
    // below, which is about rows/scope/counts agreeing with each other.
    //
    // ORDER MATTERS: identities are read BEFORE the snapshot, and that is a
    // correctness invariant, not style. The two reads can straddle a
    // retarget, and the direction of the skew decides who gets hurt. Read
    // this way, a retarget landing between them yields the PREDECESSOR's
    // identity on the SUCCESSOR's sessions — the create default's install
    // check sees a mismatch and falls back to the local row, the safe
    // direction. Read the other way (snapshot first), the same window
    // labels the predecessor's sessions with the successor's identity — a
    // false MATCH that defaults a create onto the wrong machine, which is
    // precisely the bug `host_identity` exists to close.
    // `a_retarget_between_the_identity_read_and_the_snapshot_fails_safe`
    // stands on the seam below to keep the order honest.
    let identities: std::collections::HashMap<HostId, Option<String>> = store
        .list_hosts()
        .await?
        .into_iter()
        .map(|row| (row.id, row.host_identity))
        .collect();
    staged_between_reads.await;
    // ONE snapshot, for the rows, the scope, and the live matching count
    // alike. Nothing fleet-wide is sampled before it except the identity
    // join above (ordered first on purpose — see its comment), and nothing
    // this reply says about the live hosts comes from anywhere else — which
    // is what makes the count and the rows one answer.
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
            sort,
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
        live: std::collections::VecDeque::new(),
    }];

    // The in-memory side, put into the REQUESTED order and then positioned at
    // the resume point by binary search rather than by scanning.
    //
    // A live list arrives in the order its host reported it — creation time
    // descending with the id as tiebreak, VALIDATED rather than assumed
    // (`manager::drain_sessions` refuses a list that is not in it) — and one
    // list is one host, so `host_id` is constant across it and that wire order
    // IS this helm's creation order restricted to that host. The default sort
    // therefore skips the re-sort entirely, which is not a micro-optimization:
    // it is the difference between an ordinary listing paying nothing for the
    // in-memory side and paying a comparison sort per request.
    //
    // Under the other two orders the wire order is not the requested one, and
    // merging as though it were would interleave the list into the persisted
    // page arbitrarily: a k-way merge is only correct over sources that are
    // each already ordered. So those two establish the order here, once per
    // request, over a list bounded by `crate::manager::REFRESH_SESSION_CAP`
    // and held only as pointers plus ordering keys — the sessions themselves
    // are still cloned only when taken.
    //
    // Either way the resume point is found by BINARY SEARCH, which is sound
    // for both paths for the same reason: the slice is in the requested order
    // by the time it is searched. Hosts serving from memory are the exception
    // rather than the rule (a supervisor reporting no identity at all), so the
    // common request builds none of this.
    let mut live_total: u64 = 0;
    let mut live_matching: u64 = 0;
    for snapshot in &snapshots {
        let Some(live) = snapshot.live_sessions.as_ref() else {
            continue;
        };
        // Every identity-less host counts toward the view's total, host
        // filter or not: `total` is what the view holds, and narrowing it
        // to the filter's own scope would make "N matching of M" compare a
        // number against itself.
        //
        // The ARCHIVE switch is the exception, exactly as it is for the
        // persisted side (`store::count_rows`): it says which view is being
        // counted, so an archived in-memory row is outside the default view's
        // denominator too. Counted by walking the list rather than by a
        // column, because this side has no storage to extract one into — the
        // cost is one flag test per in-memory row per page, over a list
        // bounded by `crate::manager::REFRESH_SESSION_CAP`.
        live_total = live_total.saturating_add(if filter.includes_archived() {
            live.len() as u64
        } else {
            live.iter().filter(|info| !info.archived).count() as u64
        });
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
        let mut ordered: Vec<(store::CacheKey, &SessionInfo)> = live
            .iter()
            .map(|info| (store::CacheKey::of(info, snapshot.id), info))
            .collect();
        if sort != store::ListSort::Created {
            ordered.sort_by(|(left, _), (right, _)| sort.position(left).cmp(&sort.position(right)));
        }
        let start = match &after {
            None => 0,
            Some(after) => {
                ordered.partition_point(|(key, _)| sort.position(key) <= sort.position(after))
            }
        };
        sources.push(MergeSource {
            snapshot: Some(snapshot),
            persisted: std::collections::VecDeque::new(),
            live: ordered.drain(start..).collect(),
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
            .filter_map(|(index, source)| source.peek(sort).map(|key| (key, index)))
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
                .peek(sort)
                .is_some_and(|key| key >= sort.position(fence))
        {
            more = true;
            break;
        }
        let Some(item) = sources[index].take(&by_id, &identities) else {
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
    // It carries the order it was taken in and the filter's digest, and
    // nothing else. Everything a later page needs to know about the COUNT
    // lives on this side.
    let next_cursor = match (more, taken.last()) {
        (true, Some(item)) => Some(encode_cursor(&item.key, sort, &digest)),
        _ => None,
    };
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

/// The refusal a cursor replayed under a DIFFERENT order produces.
///
/// Its own message rather than [`cursor_filter_changed`]'s, because the thing
/// the caller has to change is different: nothing is wrong with the filter,
/// and a client told to check its filters after moving a sort control would
/// look in the wrong place. The damage it prevents is the same — a position
/// in one sequence applied to another resumes mid-list and drops everything
/// before that point, with nothing in the reply to say so.
fn cursor_sort_changed() -> anyhow::Error {
    anyhow::Error::new(crate::SupervisorError {
        kind: farhelm_proto::ErrorKind::InvalidRequest,
        message: "this session list cursor was issued for a different sort order; a resume \
                  point names a place in one order and means nothing in another, so changing \
                  ?sort= requires a fresh walk with no cursor"
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
                parent: None,
                archived: false,
                id: id.to_string(),
                title: id.to_string(),
                created_at,
                last_activity_at: created_at,
                creation_seq: None,
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::Running,
                annotation: None,
                restart_offer: RestartOffer::default(),
                tabs: Vec::new(),
                source_profile: None,
            },
            host,
            host_identity: None,
            host_name: "this machine".to_string(),
            stale: false,
        }
    }

    /// A cursor must survive its own round trip exactly, in EVERY order — a
    /// resume point that decoded to a different position would skip or repeat
    /// rows with no error anywhere, which is the one failure mode a page walk
    /// cannot detect for itself.
    ///
    /// Asserted as POSITIONS rather than as whole keys, because a cursor
    /// deliberately carries only the components its own order compares (see
    /// [`ListCursor`]): what has to survive is where the token sits in the
    /// order it names, and the components that order ignores are not in the
    /// token at all.
    #[test]
    fn a_cursor_round_trips_every_component_of_the_key() {
        for sort in [
            store::ListSort::Created,
            store::ListSort::Activity,
            store::ListSort::Title,
        ] {
            for key in [
                store::CacheKey {
                    created_at: 1_700_000_000,
                    session_id: "sess-1".to_string(),
                    host: 7,
                    activity_at: 1_700_000_900,
                    title_sort: "refactor the drain".to_string(),
                },
                // The extremes, because the key is signed and the id is
                // arbitrary text a supervisor chose.
                store::CacheKey {
                    created_at: i64::MIN,
                    session_id: String::new(),
                    host: i64::MAX,
                    activity_at: i64::MIN,
                    title_sort: String::new(),
                },
                store::CacheKey {
                    created_at: i64::MAX,
                    session_id: "a \"quoted\" / slashed \u{1f600} id".to_string(),
                    host: 0,
                    activity_at: i64::MAX,
                    title_sort: "a \"quoted\" / slashed \u{1f600} title".to_string(),
                },
            ] {
                let digest = store::SessionFilter::default().title("drain").digest();
                let (decoded, bound_sort, bound) =
                    decode_cursor(&encode_cursor(&key, sort, &digest))
                        .expect("our own cursor decodes");
                assert_eq!(
                    sort.position(&decoded),
                    sort.position(&key),
                    "the position must survive the round trip under {sort:?}"
                );
                assert_eq!(
                    bound_sort, sort,
                    "and so must the order the position belongs to"
                );
                assert_eq!(
                    bound, digest,
                    "the filter binding must survive the round trip beside the position"
                );
            }
        }
    }

    /// A cursor is bound to its ORDER, not merely to its filter: the token
    /// names the sort it was minted under, and the position it carries is the
    /// one that order compares.
    ///
    /// Two properties, and both are load-bearing. A decoded cursor reports
    /// the order it came from, which is what lets [`session_page`] refuse a
    /// replay under a different one — applying it instead would resume
    /// mid-list through a sequence the position never described. And each
    /// order's own leading component is REQUIRED: a token naming the activity
    /// order without an activity stamp is refused rather than defaulted,
    /// because a default is a real position in that order and the walk would
    /// silently resume there — skipping or repeating rows depending on where
    /// in the order the default happens to fall.
    #[test]
    fn a_cursor_is_bound_to_the_order_it_was_minted_under() {
        use base64::Engine;

        let key = store::CacheKey {
            created_at: 100,
            session_id: "sess-1".to_string(),
            host: 1,
            activity_at: 900,
            title_sort: "nightly sweep".to_string(),
        };
        let digest = store::SessionFilter::default().digest();

        // That a decoded cursor REPORTS its order is
        // `a_cursor_round_trips_every_component_of_the_key`'s, asserted there
        // for every order beside the position itself; what is left to this
        // test is which components the token carries and which forgeries the
        // decoder refuses.

        // The leading component is carried exactly where it is compared, so a
        // creation-ordered cursor carries neither `la` nor `t` and stays
        // fixed-size — version 3 grew it by the order word, not by anything
        // that scales with a peer's text.
        let fields = |token: &str| -> Vec<String> {
            let json: serde_json::Value = serde_json::from_slice(
                &base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(token)
                    .expect("our cursor is base64url"),
            )
            .expect("our cursor is JSON");
            json.as_object()
                .expect("a cursor is a JSON object")
                .keys()
                .cloned()
                .collect()
        };
        assert_eq!(
            fields(&encode_cursor(&key, store::ListSort::Created, &digest)),
            vec!["at", "f", "fh", "h", "s", "sid"]
        );
        assert_eq!(
            fields(&encode_cursor(&key, store::ListSort::Activity, &digest)),
            vec!["at", "f", "fh", "h", "la", "s", "sid"]
        );
        assert_eq!(
            fields(&encode_cursor(&key, store::ListSort::Title, &digest)),
            vec!["at", "f", "fh", "h", "s", "sid", "t"]
        );

        // A token naming an order it does not carry the key for is refused
        // outright, as is one naming an order this build has never served.
        let decodes = |value: serde_json::Value| -> bool {
            let token = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(serde_json::to_vec(&value).expect("test JSON"));
            decode_cursor(&token).is_some()
        };
        assert!(
            !decodes(serde_json::json!({
                "fh": CURSOR_DOMAIN, "s": "activity", "at": 100, "sid": "s", "h": 1, "f": digest
            })),
            "the activity order's own stamp is required, never defaulted"
        );
        assert!(
            !decodes(serde_json::json!({
                "fh": CURSOR_DOMAIN, "s": "title", "at": 100, "sid": "s", "h": 1, "f": digest
            })),
            "and so is the title order's collated title"
        );
        assert!(
            !decodes(serde_json::json!({
                "fh": CURSOR_DOMAIN, "s": "cwd", "at": 100, "sid": "s", "h": 1, "f": digest
            })),
            "an order word this build does not know is not an order"
        );
        // The requiredness above is about the LEADING component and nothing
        // else: the same token with every other required field present still
        // decodes once its own order's component is there, so what these
        // refusals pin is the missing key rather than an incidentally
        // malformed token.
        assert!(
            decodes(serde_json::json!({
                "fh": CURSOR_DOMAIN, "s": "activity", "at": 100, "sid": "s", "h": 1,
                "la": 900, "f": digest
            })),
            "an activity cursor carrying its stamp is well formed"
        );
        assert!(
            decodes(serde_json::json!({
                "fh": CURSOR_DOMAIN, "s": "title", "at": 100, "sid": "s", "h": 1,
                "t": "nightly sweep", "f": digest
            })),
            "and so is a title cursor carrying its collated title"
        );
    }

    /// A cursor minted by the version that predates the sort binding must
    /// fail to decode, rather than being read as a creation-ordered one.
    ///
    /// Every version-2 token WAS creation-ordered, so reading it that way
    /// would even be correct — and it would cost the decoder a permanently
    /// optional field plus a rule about what its absence means. The domain
    /// tag exists to make that a clean refusal instead, and this pins that
    /// the tag is what does it.
    ///
    /// Two tokens, and the second is what makes the claim honest. The first is
    /// a version-2 cursor as one was actually minted: no `s`, because the
    /// order had no name yet. That alone proves nothing about the TAG — the
    /// decoder would refuse it for the missing order too. The second is a
    /// fully current token with only the domain wound back, so the tag is the
    /// single thing standing between it and a successful decode.
    #[test]
    fn a_cursor_from_the_previous_domain_is_refused() {
        use base64::Engine;

        let digest = store::SessionFilter::default().digest();
        let encode = |value: serde_json::Value| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(serde_json::to_vec(&value).expect("test JSON"))
        };
        let as_minted = encode(serde_json::json!({
            "fh": "farhelm/helm-sessions/2",
            "at": 100,
            "sid": "sess-1",
            "h": 1,
            "f": digest,
        }));
        let only_the_domain_is_old = encode(serde_json::json!({
            "fh": "farhelm/helm-sessions/2",
            "s": "created",
            "at": 100,
            "sid": "sess-1",
            "h": 1,
            "f": digest,
        }));
        assert!(
            decode_cursor(&as_minted).is_none(),
            "a token from the previous cursor version names no order and must not be guessed at"
        );
        assert!(
            decode_cursor(&only_the_domain_is_old).is_none(),
            "and the tag alone is enough to refuse one that is otherwise current"
        );
        // The control: the same token under the current tag decodes, so the
        // two refusals above are about the tag rather than about anything else
        // in the payload.
        assert!(
            decode_cursor(&encode(serde_json::json!({
                "fh": CURSOR_DOMAIN,
                "s": "created",
                "at": 100,
                "sid": "sess-1",
                "h": 1,
                "f": digest,
            })))
            .is_some(),
            "the fixture must be a well-formed cursor apart from its domain"
        );
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
                activity_at: 1_700_000_900,
                title_sort: "refactor the drain".to_string(),
            },
            store::ListSort::Created,
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
            vec!["at", "f", "fh", "h", "s", "sid"],
            "a cursor carries the domain tag, the order, the ordering key and the filter \
             binding — a count among these would be a number the caller gets to choose"
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
            activity_at: 1,
            title_sort: "s".to_string(),
        };
        let (_, _, bound) = decode_cursor(&encode_cursor(
            &key,
            store::ListSort::Created,
            &filtered.digest(),
        ))
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
                activity_at: 1_700_000_000,
                title_sort: "sess-1".to_string(),
            },
            store::ListSort::Created,
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
    ///
    /// Every hand-built fixture below carries EVERY field a current cursor
    /// needs except the one it is about, and the control at the end asserts
    /// that the shared shape does decode. Without that discipline a fixture
    /// missing two things proves only that the decoder refused it, never
    /// which of the two did the refusing — and a later change that dropped one
    /// of the checks would leave the table green.
    #[test]
    fn a_malformed_or_foreign_cursor_decodes_to_nothing() {
        use base64::Engine;

        let encode = |value: serde_json::Value| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value.to_string())
        };
        let digest = "0000000000000000";
        let foreign_domain = encode(
            serde_json::json!({"fh": "farhelm/other-sessions/3", "s": "created", "at": 1,
                               "sid": "s", "h": 1, "f": digest}),
        );
        let superseded_version = encode(
            serde_json::json!({"fh": "farhelm/helm-sessions/1", "at": 1, "sid": "s",
                                      "h": 1, "m": 99, "r": 1, "f": "h-;d-;t-;p-;s-;"}),
        );
        let extra_field = encode(
            serde_json::json!({"fh": CURSOR_DOMAIN, "s": "created", "at": 1, "sid": "s",
                               "h": 1, "f": digest, "x": 1}),
        );
        // The binding is REQUIRED, not defaulted: a token without one names
        // no result set, and inventing "the unfiltered walk" for it would
        // resume exactly the way this refusal exists to prevent. `f` is the
        // only thing missing here, which is what makes it a test OF `f`.
        let no_binding = encode(
            serde_json::json!({"fh": CURSOR_DOMAIN, "s": "created", "at": 1, "sid": "s", "h": 1}),
        );
        // And the order is required for the same reason, on the same terms.
        let no_order = encode(
            serde_json::json!({"fh": CURSOR_DOMAIN, "at": 1, "sid": "s", "h": 1,
                                      "f": digest}),
        );
        for malformed in [
            "not-base64!!",
            "",
            "YWJj",
            "e30",
            foreign_domain.as_str(),
            superseded_version.as_str(),
            extra_field.as_str(),
            no_binding.as_str(),
            no_order.as_str(),
        ] {
            assert!(
                decode_cursor(malformed).is_none(),
                "cursor {malformed:?} must not decode"
            );
        }

        // The control: the shape every fixture above is one field away from.
        assert!(
            decode_cursor(&encode(
                serde_json::json!({"fh": CURSOR_DOMAIN, "s": "created", "at": 1, "sid": "s",
                                   "h": 1, "f": digest}),
            ))
            .is_some(),
            "each refusal above must be about its own missing or wrong field, not about a \
             fixture that was never a cursor"
        );
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
            store::ListSort::Created,
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
            store::ListSort::Created,
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
            std::future::ready(()),
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

    /// Changing the SORT does not recount an unchanged filter.
    ///
    /// Spec: three pages of one filter under three different orders cost the
    /// store exactly one counting pass.
    ///
    /// This is the operational half of "ordering is not filtering". A sort
    /// changes the sequence and never the membership, so the matching count
    /// cache is keyed by the filter alone ([`MatchingCounts`]) — and the whole
    /// point of that key is that a user moving a sort control pays for a fresh
    /// page and nothing else. A count is a decode of every row in scope under
    /// the store's one mutex, so recounting per sort change would make an idle
    /// UI control expensive for every other reader too.
    ///
    /// Instrumented rather than inferred, for
    /// `a_filtered_walk_counts_once_and_recounts_only_after_a_change`'s reason:
    /// an implementation that recounted on every sort would report the same
    /// number every time, so no reply can tell the two apart. The counter can.
    #[tokio::test]
    async fn changing_the_sort_does_not_recount_an_unchanged_filter() {
        use crate::rest_harness;

        let builder = rest_harness::FleetBuilder::new()
            .await
            .local(rest_harness::HostScript {
                identity: Some("identity-local".to_string()),
                sessions: vec![
                    rest_harness::session("keep-1", 300),
                    rest_harness::session("keep-2", 200),
                    rest_harness::session("other", 100),
                ],
                ..rest_harness::HostScript::default()
            })
            .await;
        let harness = builder.start().await;
        let local = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(local).await;

        let filter = store::SessionFilter::default().title("keep");
        let baseline = harness.store.counting_passes();
        let mut counts = Vec::new();
        for sort in [
            store::ListSort::Created,
            store::ListSort::Activity,
            store::ListSort::Title,
        ] {
            let page = session_page(
                &harness.manager,
                &harness.store,
                &harness.state.counts,
                None,
                50,
                &filter,
                sort,
            )
            .await
            .expect("the page reads");
            assert_eq!(
                page.sessions.len(),
                2,
                "{sort:?} must serve the same membership"
            );
            counts.push(page.matching);
        }
        assert_eq!(
            counts,
            vec![Some(2), Some(2), Some(2)],
            "the matching count is a property of the filter, so it cannot move with the order"
        );
        assert_eq!(
            harness.store.counting_passes() - baseline,
            1,
            "and the count is computed once: a sort change reuses the cached entry rather than \
             re-decoding the scope"
        );
    }

    /// A retarget landing between the registry-identity read and the fleet
    /// snapshot must skew toward MISMATCH, never toward a false match.
    ///
    /// Why this matters: the two reads cannot be atomic (helm.db vs the
    /// manager's memory), so the read ORDER is the entire correctness
    /// argument — identities first means a mid-request identity change
    /// labels rows with the OLD identity, which the create default treats
    /// as "fall back to the local row". Swapped reads would label the
    /// predecessor's sessions with the successor's identity: a false match
    /// that re-opens the wrong-machine create this field exists to close.
    ///
    /// What is tested: an identity-less host's registry row gains an
    /// identity inside the between-reads seam; the page built around that
    /// write must still carry the pre-write `host_identity` (null) on the
    /// host's rows. An implementation that samples identities after the
    /// seam returns the new identity instead and fails here.
    #[tokio::test]
    async fn a_retarget_between_the_identity_read_and_the_snapshot_fails_safe() {
        use crate::rest_harness;

        let builder = rest_harness::FleetBuilder::new()
            .await
            .local(rest_harness::HostScript {
                // Identity-less on purpose: its rows serve from memory and
                // its registry row is free to gain an identity mid-request.
                identity: None,
                sessions: vec![rest_harness::session("live-1", 300)],
                ..rest_harness::HostScript::default()
            })
            .await;
        let harness = builder.start().await;
        let local = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(local).await;

        let page = session_page_staged(
            &harness.manager,
            &harness.store,
            &harness.state.counts,
            None,
            50,
            &store::SessionFilter::default(),
            store::ListSort::Created,
            std::future::ready(()),
            async {
                let rows = harness.store.list_hosts().await.expect("hosts list");
                let row = rows
                    .iter()
                    .find(|row| row.id == local)
                    .expect("the local row exists");
                let outcome = harness
                    .store
                    .record_first_contact(local, &store::DialedAs::of(row), "install-late")
                    .await
                    .expect("the identity write runs");
                assert!(
                    matches!(outcome, store::FirstContactOutcome::Recorded),
                    "the staged identity write must actually land for the \
                     race to be exercised"
                );
            },
        )
        .await
        .expect("the page reads");

        assert_eq!(page.sessions.len(), 1);
        assert_eq!(
            page.sessions[0].host_identity, None,
            "a mid-request identity write must not be attributed to rows \
             sampled beside it — stale identity mismatches safely, a fresh \
             one would falsely match"
        );
    }

    /// The ordering key must agree with the SQL `ORDER BY` it is merged
    /// against, down to the third component — an in-memory row sorted by a
    /// different rule would interleave wrongly with the paged rows and the
    /// resulting page would be neither order.
    #[test]
    fn the_order_is_time_descending_then_id_then_host() {
        let sort = store::ListSort::Created;
        let position = |row: &SessionRow| store::CacheKey::of(&row.info, row.host);

        let newest = row("b", 200, 1);
        let older = row("a", 100, 1);
        assert!(sort.position(&position(&newest)) < sort.position(&position(&older)));

        let tie_low_id = row("a", 100, 9);
        let tie_high_id = row("b", 100, 1);
        assert!(
            sort.position(&position(&tie_low_id)) < sort.position(&position(&tie_high_id)),
            "equal creation times order by session id, not by host"
        );

        let same_id_low_host = row("a", 100, 1);
        let same_id_high_host = row("a", 100, 2);
        assert!(
            sort.position(&position(&same_id_low_host))
                < sort.position(&position(&same_id_high_host)),
            "the host id is the final tiebreak, so the order is total even where two hosts \
             claim one session id"
        );
    }

    /// Each of the other two orders leads with its own component and then
    /// falls into the SAME creation-order tail.
    ///
    /// The tail is the property worth pinning: a sort whose leading component
    /// ties has to break that tie identically to every other sort, or two
    /// equally-ranked rows can swap places between two page fetches and a
    /// cursor over them will skip one and repeat the other. Asserted here on
    /// in-memory keys because that is the side a merge builds by hand; the
    /// store's own tests pin that SQLite agrees.
    #[test]
    fn the_other_orders_lead_with_their_own_component_and_share_the_tail() {
        let keyed = |id: &str, created_at: i64, activity: i64, title: &str, host: HostId| {
            let mut row = row(id, created_at, host);
            row.info.last_activity_at = activity;
            row.info.title = title.to_string();
            store::CacheKey::of(&row.info, host)
        };

        let activity = store::ListSort::Activity;
        let busy = keyed("a", 100, 900, "zzz", 1);
        let quiet = keyed("b", 800, 200, "aaa", 1);
        assert!(
            activity.position(&busy) < activity.position(&quiet),
            "recent activity outranks a later creation time under the activity order"
        );
        let tie_new = keyed("z", 300, 500, "zzz", 1);
        let tie_old = keyed("a", 200, 500, "aaa", 1);
        assert!(
            activity.position(&tie_new) < activity.position(&tie_old),
            "equal activity falls through to creation time descending, not to the id"
        );

        let title = store::ListSort::Title;
        let apple = keyed("z", 100, 100, "Apple", 1);
        let banana = keyed("a", 900, 900, "banana", 1);
        assert!(
            title.position(&apple) < title.position(&banana),
            "the title order is case-insensitive, so Apple precedes banana"
        );
        let same_new = keyed("z", 300, 100, "Same", 1);
        let same_old = keyed("a", 200, 900, "same", 1);
        assert!(
            title.position(&same_new) < title.position(&same_old),
            "titles that differ only by case tie, and the tie breaks by creation time"
        );

        // The final tiebreak is the host id under every order, which is what
        // makes each of them total even where two hosts claim one session id.
        for sort in [
            store::ListSort::Created,
            store::ListSort::Activity,
            store::ListSort::Title,
        ] {
            let low = keyed("a", 100, 100, "same", 1);
            let high = keyed("a", 100, 100, "same", 2);
            assert!(
                sort.position(&low) < sort.position(&high),
                "{sort:?} must be a total order"
            );
        }
    }
}
