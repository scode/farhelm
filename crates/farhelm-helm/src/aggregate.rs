//! The merged, multi-host session list the REST edge serves: every host's
//! rows, held and sorted in memory, handed to a client as one array.
//!
//! ## Where the rows come from
//!
//! Not from the hosts. Every connected host's actor drains its supervisor's
//! whole list into helm.db's session cache ([`crate::manager`]), and this
//! module merges what is in that cache — live hosts' latest refresh and down
//! hosts' last-known entries alike — into one order. A host being connected
//! changes only the `stale` flag on its rows, never where they are read
//! from. That is what keeps a flapping or slow host off the request path:
//! a list read is a local read, and its latency is the store's.
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
//! ## Whole, in memory, by contract
//!
//! SPEC.md's Session list section fixes the scale this list is built for
//! (tens of sessions on a few hosts) and says outright that no layer
//! paginates, cursors, streams, or indexes it: the whole fleet is read,
//! sorted, counted and served per request, and the one bound is the cap
//! ([`farhelm_proto::LIST_SESSIONS_CAP`]) every supervisor already applies
//! to its own reply. A read here therefore decodes every cached row in
//! scope — up to the cap PER HOST, so the input work grows with the number
//! of cache-serving hosts while only the merged OUTPUT is cut back to one
//! cap — and sorts them in Rust. At the scale SPEC.md fixes that input is
//! a few hundred small JSON payloads. An earlier shape paged this list with keyset cursors over
//! per-order indexes; the cursor-drift edge cases that came with sorting
//! by mutable keys under pagination (a row crossing the cursor between two
//! pages) do not exist for a reply that is a snapshot, which is the whole
//! reason the simpler shape is the wanted one.
//!
//! ## The order, and the three of them
//!
//! Creation time descending, session id ascending, host id ascending — the
//! wire's own order (`farhelm-supervisor`'s `list_order_key`) with one extra
//! component, so a merged list reads as one list rather than as
//! concatenated per-host ones. A caller may ask for recent activity or for
//! title instead ([`store::ListSort`]); each of those leads with its own
//! component and ends in that creation-order tail, so every order is total
//! and two reads of an unchanged fleet come back identical. What changes
//! with the sort is the sequence; nothing about WHICH rows the view holds,
//! so neither count moves. Hosts go on reporting their sessions in
//! whatever order they like: the sort is a property of this merged view
//! and of nothing underneath it.
//!
//! The title order compares `str::to_lowercase` of the whole title:
//! Unicode's locale-independent full lowercase mapping, case-insensitive
//! and otherwise ordinal, deliberately not locale-aware (an ICU dependency
//! and a per-user setting this product does not have) and deliberately not
//! SQLite's ASCII-only `NOCASE`.
//!
//! ## Filtering, and why it happens HERE
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
//! Not in the BROWSER: the count has to be a claim about the whole view,
//! and the browser only ever holds what it was sent. Applied here, "N
//! matching of M" is computed from the same array the rows come out of.
//!
//! So both sources are narrowed by one predicate ([`store::SessionFilter`]),
//! and a FILTERED reply carries TWO counts: how many matched, and how many
//! the VIEW holds. "The view" rather than "the fleet" because one dimension
//! is not a narrowing at all: the archive switch decides which list is
//! being served, so `total` follows it and follows nothing else (see
//! [`SessionListBody::total`]). An UNFILTERED reply carries only that total
//! and makes no matching claim at all (see [`SessionListBody::matching`]).
//!
//! ## One snapshot, one answer
//!
//! The rows, both counts and the `truncated` flag are all computed from
//! one in-memory array built from one store read and one manager
//! snapshot, so "the count and these rows describe one moment" is true by
//! construction. There is no count cache: a count is a pass over an array
//! the request is already holding, and the arrangement this replaced — a
//! cache keyed by a store generation, with a long argument about which
//! keys were unsafe — existed only because counting per PAGE made a walk
//! quadratic. With no pages there is nothing to amortize.

use crate::manager::{ConnectionManager, HostSnapshot};
use crate::store::{self, HelmStore, HostId, HostKind};
use farhelm_proto::{LIST_SESSIONS_CAP, SessionInfo};
use serde::Serialize;

/// How a host is NAMED on a session row.
///
/// The alias, when present, IS the display name outright — a stored label
/// that wins over the derivation below without qualification, including for
/// the local row, and this is also the name `farhelm agent` resolution
/// matches against (agent_requests.rs's `resolve_host`), which is why an
/// aliased host's raw destination stops resolving.
///
/// Absent an alias, M6 invented no separate host-naming surface (PLAN_M6.md's
/// Out list), so this falls back to a rendering of what the registry already
/// holds: an ssh row is its destination, and the reserved local row — which
/// has no destination by construction — is described rather than named.
/// Kept in one function so every surface that shows a host (session rows,
/// the hosts list, the agent relay) says the same thing about it.
pub(crate) fn host_display_name(
    kind: HostKind,
    destination: Option<&str>,
    alias: Option<&str>,
) -> String {
    if let Some(alias) = alias {
        return alias.to_string();
    }
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
    /// falsely matching; see the identity join in `session_list_staged`. A
    /// `HostId` outlives retargets and adoptions while the machine behind
    /// it changes; this field is what lets the create dialog's host default
    /// notice that the install it was derived from is no longer the one
    /// the row id now names (the #156-review residual).
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
    /// The activity stamp that was current the last time some client had
    /// this session open (`store::HelmStore::seen_activity`'s per-session
    /// answer), or `None` when nothing is currently recorded — either
    /// because no client has ever opened the session, or because a client
    /// manually marked it unread (`store::HelmStore::clear_seen` DELETES
    /// the row rather than writing a sentinel, so the two cases are
    /// indistinguishable here on purpose: SPEC.md, Status defines "unseen"
    /// from the absence of a stamp either way). Compared against
    /// `SessionInfo::effective_activity` on the CLIENT to decide whether
    /// the idle dot draws grey (seen) or blue (unseen output).
    ///
    /// ALWAYS serialized, `host_identity`'s shape and reason exactly: a
    /// client must be able to tell "this helm says never seen" (`null`) from
    /// "this helm predates the field" (key absent), because only the former
    /// may draw the unseen-blue dot and offer the read/unread toggle — an
    /// old helm's idle rows draw the SAME grey every other idle row does;
    /// what they lose is only the toggle and the possibility of the blue
    /// variant, never a distinct legacy colour.
    pub(crate) seen_activity_at: Option<i64>,
}

/// The whole merged list, in the JSON shape `GET /api/sessions` answers
/// with: one array, its two counts, and whether a cap cut it.
///
/// `sessions`/`total`/`truncated` are the keys the UI has read since the
/// list first existed; `matching` arrived with filtering. There is no
/// cursor and no page size, by contract (SPEC.md's Session list section).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct SessionListBody {
    /// Every row of the view that satisfies the filter, in the requested
    /// order — cut at [`LIST_SESSIONS_CAP`] only when `truncated` says so.
    pub(crate) sessions: Vec<SessionRow>,
    /// How many sessions the VIEW holds, filter or no filter — the
    /// denominator the UI's "N matching of M sessions" prints.
    ///
    /// Deliberately does not move when the user types: a denominator that
    /// tracked the filter would compare a number against itself. The one
    /// thing that does move it is the archive-inclusion switch, which
    /// selects which list is being served rather than narrowing one — the
    /// default view's rows and its total are both about the non-archived
    /// fleet, and `include_archived=true` widens both. A cached row whose
    /// payload no longer decodes is in neither the rows nor this count: it
    /// is dropped at the read with a warning (`store::CachedRow`), so the
    /// counts always describe rows a client can see.
    pub(crate) total: u64,
    /// How many rows of the view satisfy the caller's filter, present
    /// exactly when a predicate is active (`!filter.is_empty()`, which
    /// includes the default view's implicit "not archived").
    ///
    /// Equal to `sessions.len()` unless the cap cut the reply — the count
    /// is over the whole view, the array is what fits — so a client that
    /// prints "showing N of M matching" has both numbers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) matching: Option<u64>,
    /// Whether the client is NOT looking at the whole view: some host's
    /// list was cut at the wire's cap (a fact kept with that host's cache,
    /// so it holds for a host that has since gone down and across a helm
    /// restart), or the merged, filtered, sorted array hit the cap. This is
    /// the one signal behind SPEC.md's "could not read to the end" notice;
    /// nothing else produces it.
    pub(crate) truncated: bool,
}

/// Tag one host's session with that host, for the merged list.
///
/// `identity` is the registry's recorded install identity for the host —
/// passed alongside the snapshot rather than read from it because the
/// snapshot cannot carry a fresh one: first contact writes the identity to
/// helm.db without the actor being reconciled, so a snapshot-side copy
/// would sit at `NULL` for most hosts most of the time (the same reasoning
/// `hosts`' module doc gives for joining the two reads in `host_views`).
fn row_of(
    host: &HostSnapshot,
    identity: Option<&str>,
    info: SessionInfo,
    seen_activity_at: Option<i64>,
) -> SessionRow {
    SessionRow {
        info,
        host: host.id,
        host_identity: identity.map(str::to_string),
        host_name: host_display_name(
            host.kind,
            host.destination.as_deref(),
            host.alias.as_deref(),
        ),
        seen_activity_at,
        stale: !host.state.is_connected(),
    }
}

// ---- Ordering ---------------------------------------------------------

/// The creation-order tail every sort ends in: `created_at` descending,
/// session id ascending, host id ascending. Total on its own, which is what
/// makes every order total.
fn creation_tail(row: &SessionRow) -> (std::cmp::Reverse<i64>, &str, HostId) {
    (
        std::cmp::Reverse(row.info.created_at),
        row.info.id.as_str(),
        row.host,
    )
}

/// Sort `rows` into `sort`'s order, in place.
///
/// One function for all three orders so the tail is shared by
/// construction rather than by three copies agreeing. The title key is
/// computed once per row (`sort_by_cached_key`) because lowercasing a title
/// per comparison would allocate `n log n` strings for a sort that runs on
/// every refresh.
pub(crate) fn sort_rows(rows: &mut [SessionRow], sort: store::ListSort) {
    match sort {
        store::ListSort::Created => rows.sort_by(|a, b| creation_tail(a).cmp(&creation_tail(b))),
        store::ListSort::Activity => rows.sort_by(|a, b| {
            std::cmp::Reverse(a.info.effective_activity())
                .cmp(&std::cmp::Reverse(b.info.effective_activity()))
                .then_with(|| creation_tail(a).cmp(&creation_tail(b)))
        }),
        store::ListSort::Title => rows.sort_by_cached_key(|row| {
            (
                row.info.title.to_lowercase(),
                std::cmp::Reverse(row.info.created_at),
                row.info.id.clone(),
                row.host,
            )
        }),
    }
}

// ---- Assembly ---------------------------------------------------------

/// Build the reply from the merged view: filter, count, sort, cap.
///
/// The pure core of [`session_list`], separated so the merge/sort/cap/count
/// rules can be pinned without a fleet. `view` is every row of the view
/// (archive switch already applied, since that is a scope rather than a
/// predicate; a cached row that no longer decodes is not in it and not
/// counted — see `store::CachedRow`); `hosts_truncated` says whether any
/// host's own list was cut at the wire's cap.
///
/// Order of operations is a contract, not an accident: the filter is
/// applied before the cap, so `matching` describes the whole view and the
/// cap cuts a SORTED array — a fleet past the cap loses the rows that sort
/// last in the requested order, which under `created` is the oldest and
/// under `title` is the end of the alphabet. That is what "could not read
/// to the end" means to the client that sees the flag.
fn assemble(
    view: Vec<SessionRow>,
    filter: &store::SessionFilter,
    sort: store::ListSort,
    hosts_truncated: bool,
) -> SessionListBody {
    let total = view.len() as u64;
    let mut sessions: Vec<SessionRow> = view
        .into_iter()
        .filter(|row| filter.matches(row.host, &row.info))
        .collect();
    let matching = (!filter.is_empty()).then_some(sessions.len() as u64);
    sort_rows(&mut sessions, sort);
    let capped = sessions.len() > LIST_SESSIONS_CAP;
    sessions.truncate(LIST_SESSIONS_CAP);
    SessionListBody {
        sessions,
        total,
        matching,
        truncated: hosts_truncated || capped,
    }
}

/// The whole merged list, filtered and ordered as asked.
///
/// Reads the persisted cache for every host that serves from it, the
/// in-memory list of every host that does not, and hands the union to
/// [`assemble`]. See the module docs for why the whole fleet is read per
/// request and why that is the wanted shape.
pub(crate) async fn session_list(
    manager: &ConnectionManager,
    store: &HelmStore,
    filter: &store::SessionFilter,
    sort: store::ListSort,
) -> anyhow::Result<SessionListBody> {
    session_list_staged(
        manager,
        store,
        filter,
        sort,
        std::future::ready(()),
        std::future::ready(()),
    )
    .await
}

/// [`session_list`], with seams where a test can stage a concurrent fleet
/// mutation.
///
/// `staged` is awaited BEFORE anything about the fleet is sampled, which
/// makes it the barrier the coherence property is stated against: whatever
/// happens inside it is entirely in this request's past, so every number
/// the reply carries must describe the world after it. Production passes a
/// ready future, so the seam costs a poll.
///
/// `staged_between_reads` is awaited between the registry-identity read and
/// the fleet snapshot — the window where a retarget skews the identity join.
/// The read order makes that skew fail SAFE (see the comment at the identity
/// join), and the test standing on this seam is what keeps a refactor from
/// quietly swapping the reads back into the dangerous order.
async fn session_list_staged(
    manager: &ConnectionManager,
    store: &HelmStore,
    filter: &store::SessionFilter,
    sort: store::ListSort,
    staged: impl std::future::Future<Output = ()>,
    staged_between_reads: impl std::future::Future<Output = ()>,
) -> anyhow::Result<SessionListBody> {
    staged.await;
    // The registry-identity join for `SessionRow::host_identity` — a second
    // read beside the snapshot, on the same two-reads-joined terms as
    // `hosts::host_views` and for the same reason (see `row_of`): helm.db is
    // the authority for recorded identity, and the snapshot cannot carry a
    // fresh copy.
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
    let registry = store.list_hosts().await?;
    let identities: std::collections::HashMap<HostId, Option<String>> = registry
        .iter()
        .map(|row| (row.id, row.host_identity.clone()))
        .collect();
    staged_between_reads.await;
    // ONE snapshot, for the rows, the scope, the live rows and the per-host
    // cap flags alike. Nothing fleet-wide is sampled before it except the
    // identity join above (ordered first on purpose — see its comment),
    // and nothing this reply says about the live hosts comes from anywhere
    // else — which is what makes the counts and the rows one answer.
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
    // and every one of them appears twice in the list and twice in the
    // total. Scoping from the one snapshot the merge is built on makes the
    // two views disjoint by construction rather than by timing.
    let scope: Vec<HostId> = snapshots
        .iter()
        .filter(|snapshot| snapshot.live_sessions.is_none())
        .map(|snapshot| snapshot.id)
        .collect();
    // Deliberately NOT narrowed to the host filter's host: `total` is the
    // size of the whole view whatever the filter says (the "M" the UI
    // prints "N matching of M" against), and a host filter is a filter
    // like any other. Reading only the named host's rows would make the
    // denominator follow the filter — exactly the comparison of a number
    // against itself that `SessionListBody::total` rules out.

    let mut view: Vec<SessionRow> = Vec::new();
    // Rows and per-host cap flags in ONE store read (`cached_slice`), so
    // the flag always describes exactly these rows: a refresh replaces both
    // in one transaction, and two separate reads could pair a newly capped
    // cache's rows with its pre-cap "complete" flag.
    let slice = store.cached_slice(&scope).await?;
    // The seen-state join for `SessionRow::seen_activity_at`: one read of
    // every id this reply could possibly carry — cache-served rows and
    // identity-less hosts' in-memory ones alike — gathered before either
    // build loop below rather than looked up per row, so a fleet-sized
    // listing costs one `session_seen` query instead of one per session.
    // Unlike the identity join above, there is no read-ordering hazard here
    // to document: a `mark_seen`/`clear_seen` racing this read merely shows
    // up on the next poll instead of this one, the same staleness every
    // other field in this reply already tolerates.
    let mut seen_ids: Vec<String> = slice
        .rows
        .iter()
        .map(|cached| cached.info.id.clone())
        .collect();
    for snapshot in &snapshots {
        if let Some(live) = &snapshot.live_sessions {
            seen_ids.extend(live.iter().map(|info| info.id.clone()));
        }
    }
    let seen_activity = store.seen_activity(&seen_ids).await?;
    for cached in slice.rows {
        // The archive switch is applied as a SCOPE, on the stored flag,
        // before the payload is looked at.
        if cached.archived && !filter.includes_archived() {
            continue;
        }
        let Some(host) = by_id.get(&cached.host) else {
            // A host the registry knows but the manager has no actor for
            // is a window during removal; its rows have nowhere to hang.
            continue;
        };
        let seen = seen_activity.get(&cached.info.id).copied();
        view.push(row_of(
            host,
            identities.get(&cached.host).and_then(Option::as_deref),
            cached.info,
            seen,
        ));
    }
    // A cache-serving host's cap flag comes from the CACHE (the same
    // `cached_slice` read as its rows): the flag describes the rows being
    // served, and those rows are served in every non-connected state and
    // across a helm restart. Gating it on the connection would present a
    // capped host's stale list as the whole one — the case SPEC.md's
    // Session list section forbids. The one host without a cache
    // (identity-less, serving from the actor's memory) carries its flag on
    // the snapshot beside its rows.
    //
    // A HOST filter scopes the notice, on both paths: a request for one
    // host's sessions cannot be missing rows another host's cap cut, so an
    // unrelated capped host must not mark it incomplete. Only the host
    // dimension gets this — for every other filter (title, status,
    // profile, parent, directory) an omitted row COULD have matched, so
    // any capped host keeps the conservative any-host rule.
    let counts_toward_notice =
        |host: HostId| filter.host_scope().is_none_or(|scoped| scoped == host);
    let mut hosts_truncated = slice
        .truncated_hosts
        .iter()
        .any(|host| counts_toward_notice(*host));
    for snapshot in &snapshots {
        let Some(live) = &snapshot.live_sessions else {
            continue;
        };
        if snapshot.list_truncated && counts_toward_notice(snapshot.id) {
            hosts_truncated = true;
        }
        let identity = identities.get(&snapshot.id).and_then(Option::as_deref);
        for info in live.iter() {
            if info.archived && !filter.includes_archived() {
                continue;
            }
            let seen = seen_activity.get(&info.id).copied();
            view.push(row_of(snapshot, identity, info.clone(), seen));
        }
    }
    // When provenance exists, one catalog read resolves every row before
    // filtering or serialization. A raw-only view skips that dependency.
    // Mutate the nested records in place: cloning whole rows here makes the
    // fleet path duplicate every title, cwd, invocation, and tab merely to
    // replace one small provenance enum.
    if view.iter().any(|row| row.info.source_profile.is_some()) {
        let profiles = crate::sessions::load_profile_name_index(store).await?;
        crate::sessions::resolve_session_profiles(
            &profiles,
            view.iter_mut().map(|row| &mut row.info),
        );
    }
    Ok(assemble(view, filter, sort, hosts_truncated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::{RestartOffer, SessionStatus};

    /// A row with only the fields this module's ordering and counting care
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
            seen_activity_at: None,
            stale: false,
        }
    }

    fn ids(rows: &[SessionRow]) -> Vec<&str> {
        rows.iter().map(|row| row.info.id.as_str()).collect()
    }

    /// The creation order is time descending, then session id, then host —
    /// total down to the third component, so two reads of an unchanged
    /// fleet come back identical and the cap always cuts the same rows.
    #[farhelm_testtrace::test]
    fn the_order_is_time_descending_then_id_then_host() {
        let mut rows = vec![
            row("a", 100, 2),
            row("b", 100, 1),
            row("a", 100, 1),
            row("b", 200, 1),
        ];
        sort_rows(&mut rows, store::ListSort::Created);
        let keyed: Vec<(&str, HostId)> = rows
            .iter()
            .map(|row| (row.info.id.as_str(), row.host))
            .collect();
        assert_eq!(
            keyed,
            [("b", 1), ("a", 1), ("a", 2), ("b", 1)],
            "newest first; equal times by session id, not by host; the host id is the final \
             tiebreak so the order is total even where two hosts claim one session id"
        );
    }

    /// Each of the other two orders leads with its own component and then
    /// falls into the SAME creation-order tail, and the title order is
    /// case-insensitive across Unicode. The tail is the property worth
    /// pinning: a sort whose leading component ties has to break that tie
    /// identically to every other sort, or the same fleet could come back
    /// in two different sequences on two reads.
    #[farhelm_testtrace::test]
    fn the_other_orders_lead_with_their_own_component_and_share_the_tail() {
        let keyed = |id: &str, created_at: i64, activity: i64, title: &str| {
            let mut row = row(id, created_at, 1);
            row.info.last_activity_at = activity;
            row.info.title = title.to_string();
            row
        };

        let mut rows = vec![
            keyed("quiet", 800, 200, "aaa"),
            keyed("busy", 100, 900, "zzz"),
            keyed("tie-old", 200, 500, "aaa"),
            keyed("tie-new", 300, 500, "zzz"),
            keyed("unknown", 250, 0, "mmm"),
        ];
        sort_rows(&mut rows, store::ListSort::Activity);
        assert_eq!(
            ids(&rows),
            ["busy", "tie-new", "tie-old", "unknown", "quiet"],
            "recent activity outranks a later creation time; equal activity falls through to \
             creation time descending; an unknown stamp sorts by its creation time rather than \
             at the epoch"
        );

        let mut rows = vec![
            keyed("banana", 900, 900, "banana"),
            keyed("apple", 100, 100, "Apple"),
            keyed("same-old", 200, 900, "same"),
            keyed("same-new", 300, 100, "Same"),
            keyed("istanbul", 400, 400, "İstanbul"),
        ];
        sort_rows(&mut rows, store::ListSort::Title);
        assert_eq!(
            ids(&rows),
            ["apple", "banana", "istanbul", "same-new", "same-old"],
            "the title order is case-insensitive (Apple precedes banana, İ folds), and titles \
             that differ only by case tie and break by creation time"
        );
    }

    /// `total` is the view and `matching` is the filter, and the array
    /// holds exactly the matches. Pinned on the pure core because this is
    /// the arithmetic the UI's "N matching of M sessions" banner prints,
    /// and the one place a count could disagree with the rows beside it.
    #[farhelm_testtrace::test]
    fn counts_describe_the_view_and_the_filter_from_one_array() {
        let mut keep = row("keep", 300, 1);
        keep.info.title = "keep me".to_string();
        let mut drop = row("drop", 200, 1);
        drop.info.title = "other".to_string();
        let view = vec![keep, drop];

        let unfiltered = assemble(
            view.clone(),
            &store::SessionFilter::default().include_archived(true),
            store::ListSort::Created,
            false,
        );
        assert_eq!(unfiltered.total, 2);
        assert_eq!(
            unfiltered.matching, None,
            "an unfiltered listing makes no matching claim"
        );
        assert_eq!(ids(&unfiltered.sessions), ["keep", "drop"]);
        assert!(!unfiltered.truncated);

        let filtered = assemble(
            view,
            &store::SessionFilter::default()
                .include_archived(true)
                .title("KEEP"),
            store::ListSort::Created,
            false,
        );
        assert_eq!(
            filtered.total, 2,
            "the denominator does not move with the filter"
        );
        assert_eq!(filtered.matching, Some(1));
        assert_eq!(ids(&filtered.sessions), ["keep"]);
    }

    /// The default view's implicit "not archived" is a predicate, so it
    /// reports `matching` too — the UI relies on the count being present to
    /// print "N matching" wording only for filters a person applied, and
    /// decides that on its own side.
    #[farhelm_testtrace::test]
    fn the_default_view_reports_a_matching_count() {
        let body = assemble(
            vec![row("a", 1, 1)],
            &store::SessionFilter::default(),
            store::ListSort::Created,
            false,
        );
        assert_eq!(body.matching, Some(1));
    }

    /// The cap cuts a SORTED, FILTERED array and says so; a host's own cap
    /// flag carries through; landing exactly on the cap is not a cut.
    /// These are the whole of what SPEC.md's "could not read to the end"
    /// notice means, so each half is pinned: the filter applies before the
    /// cap (or `matching` would describe rows the client cannot see), the
    /// cut keeps what sorts FIRST in the requested order, and the flag is
    /// never raised over a list the client did read to the end.
    #[farhelm_testtrace::test]
    fn the_cap_cuts_the_sorted_filtered_array_and_flags_it() {
        let view: Vec<SessionRow> = (0..=LIST_SESSIONS_CAP)
            .map(|i| row(&format!("s{i:04}"), i as i64, 1))
            .collect();
        let body = assemble(
            view.clone(),
            &store::SessionFilter::default().include_archived(true),
            store::ListSort::Created,
            false,
        );
        assert!(body.truncated);
        assert_eq!(body.sessions.len(), LIST_SESSIONS_CAP);
        assert_eq!(body.total, LIST_SESSIONS_CAP as u64 + 1);
        assert_eq!(
            body.sessions.first().map(|row| row.info.created_at),
            Some(LIST_SESSIONS_CAP as i64),
            "the newest row is kept under the creation order"
        );
        assert!(
            body.sessions.iter().all(|row| row.info.created_at > 0),
            "the oldest row is the one cut"
        );

        let filtered = assemble(
            view.clone(),
            &store::SessionFilter::default()
                .include_archived(true)
                .title("s000"),
            store::ListSort::Created,
            false,
        );
        assert!(
            !filtered.truncated,
            "the filter applies before the cap: ten matches do not hit it"
        );
        assert_eq!(filtered.matching, Some(10));
        assert_eq!(filtered.sessions.len(), 10);

        let at_cap = assemble(
            view.into_iter().skip(1).collect(),
            &store::SessionFilter::default().include_archived(true),
            store::ListSort::Created,
            false,
        );
        assert!(!at_cap.truncated, "landing exactly on the cap is not a cut");
        assert_eq!(at_cap.sessions.len(), LIST_SESSIONS_CAP);

        let host_capped = assemble(
            vec![row("a", 1, 1)],
            &store::SessionFilter::default().include_archived(true),
            store::ListSort::Created,
            true,
        );
        assert!(
            host_capped.truncated,
            "a host whose own reply hit the wire's cap makes the merged list incomplete"
        );
    }

    /// The cap cuts MEMBERSHIP in the requested order, not in creation
    /// order: under `title` and `activity` the row that survives is the one
    /// those orders put first, however old its creation time.
    ///
    /// The fixture makes every wrong rule visible: the survivor is the
    /// OLDEST-created row in the view, so an implementation that first kept
    /// the newest-created cap's worth and only then re-sorted the survivors
    /// would drop it and pass every creation-order test.
    #[farhelm_testtrace::test]
    fn the_cap_selects_membership_in_the_requested_order() {
        // `LIST_SESSIONS_CAP` filler rows created after the special row,
        // with titles and activity stamps that sort AFTER it.
        let mut view: Vec<SessionRow> = (0..LIST_SESSIONS_CAP)
            .map(|i| {
                let mut row = row(&format!("m{i:04}"), 1_000 + i as i64, 1);
                row.info.title = format!("mmm-{i:04}");
                row.info.last_activity_at = 1_000 + i as i64;
                row
            })
            .collect();
        let mut special = row("survivor", 1, 1);
        special.info.title = "aaa-first".to_string();
        special.info.last_activity_at = 9_999_999;
        view.push(special);

        for sort in [store::ListSort::Title, store::ListSort::Activity] {
            let body = assemble(
                view.clone(),
                &store::SessionFilter::default().include_archived(true),
                sort,
                false,
            );
            assert!(body.truncated);
            assert_eq!(body.sessions.len(), LIST_SESSIONS_CAP);
            assert_eq!(
                body.sessions.first().map(|row| row.info.id.as_str()),
                Some("survivor"),
                "{sort:?} must keep the row it sorts first, not the newest-created one"
            );
        }
    }

    /// A supervisor deliberately reports profile existence as unresolved;
    /// the fleet listing must replace that marker from one helm catalog read
    /// before the row can reach either the browser or agent-facing listing.
    /// This test pins the ownership boundary rather than merely exercising
    /// the pure three-way comparison.
    #[farhelm_testtrace::test]
    async fn a_supervisor_unresolved_profile_is_resolved_in_the_fleet_listing() {
        use crate::rest_harness;

        let mut source = rest_harness::session("profiled", 1);
        source.source_profile = Some(farhelm_proto::SourceProfile {
            id: "starter-claude".to_string(),
            name: "claude".to_string(),
            existence: farhelm_proto::ProfileExistence::Unresolved,
        });
        let harness = rest_harness::FleetBuilder::new()
            .await
            .local(rest_harness::HostScript {
                identity: Some("identity-local".to_string()),
                sessions: vec![source],
                ..rest_harness::HostScript::default()
            })
            .await
            .start()
            .await;
        let local = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(local).await;

        let list = session_list(
            &harness.manager,
            &harness.store,
            &store::SessionFilter::default(),
            store::ListSort::Created,
        )
        .await
        .expect("the helm resolves the supervisor reply");
        let source = list.sessions[0]
            .info
            .source_profile
            .as_ref()
            .expect("profile snapshot survives");
        assert_eq!(source.existence, farhelm_proto::ProfileExistence::Present);
    }

    /// A change to an identity-less host's in-memory sessions, landing in a
    /// request's PAST, is counted by the very reply that returns those rows.
    ///
    /// Spec: a reply whose live rows changed before the request sampled
    /// anything reports a `matching` and a `total` that agree with the rows
    /// it carries.
    ///
    /// The shape this replaced cached a matching count keyed by the fleet
    /// revision, and the revision is bumped only after the new list is
    /// published (`manager`'s `HostActor::publish_refresh`), so a request
    /// could carry three rows beside "2 matching" with nothing able to
    /// detect it. The cache is gone; this test is what keeps a future one
    /// from coming back with the same hole, by staging the mutation through
    /// [`session_list_staged`]'s seam so the barrier stands over the code.
    #[farhelm_testtrace::test]
    async fn a_live_change_before_the_snapshot_is_counted_by_the_reply_that_returns_it() {
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
        let first = session_list(
            &harness.manager,
            &harness.store,
            &filter,
            store::ListSort::Created,
        )
        .await
        .expect("the first list reads");
        assert_eq!(first.sessions.len(), 2);
        assert_eq!(first.matching, Some(2));

        let second = session_list_staged(
            &harness.manager,
            &harness.store,
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
        .expect("the second list reads");

        assert_eq!(
            second.sessions.len(),
            3,
            "the new in-memory session is in the list"
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
    /// identity inside the between-reads seam; the list built around that
    /// write must still carry the pre-write `host_identity` (null) on the
    /// host's rows. An implementation that samples identities after the
    /// seam returns the new identity instead and fails here.
    #[farhelm_testtrace::test]
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

        let list = session_list_staged(
            &harness.manager,
            &harness.store,
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
        .expect("the list reads");

        assert_eq!(list.sessions.len(), 1);
        assert_eq!(
            list.sessions[0].host_identity, None,
            "a mid-request identity write must not be attributed to rows \
             sampled beside it — stale identity mismatches safely, a fresh \
             one would falsely match"
        );
    }

    /// `SessionRow::seen_activity_at`'s listing join (SPEC.md, Status): a
    /// marked session's row carries exactly the stamp `HelmStore::mark_seen`
    /// recorded, an unmarked (or manually marked-unread) one carries `None`,
    /// and both are true JSON `null`/value pairs at the wire — not merely
    /// equal `Option`s in Rust — because the field's whole contract is that
    /// a client can tell "never seen" from "this helm predates the field"
    /// by whether the KEY is present at all.
    #[farhelm_testtrace::test]
    async fn the_listing_carries_each_sessions_seen_stamp() {
        use crate::rest_harness;

        let harness = rest_harness::helm_listing(vec![
            rest_harness::session("seen-1", 1_700_000_000),
            rest_harness::session("unseen-1", 1_700_000_100),
        ])
        .await;
        harness
            .store
            .mark_seen("seen-1", 1_700_000_000)
            .await
            .expect("mark seen-1 seen");

        let list = session_list(
            &harness.manager,
            &harness.store,
            &store::SessionFilter::default(),
            store::ListSort::Created,
        )
        .await
        .expect("the list reads");

        let seen = list
            .sessions
            .iter()
            .find(|row| row.info.id == "seen-1")
            .expect("the marked session is in the reply");
        assert_eq!(seen.seen_activity_at, Some(1_700_000_000));
        let unseen = list
            .sessions
            .iter()
            .find(|row| row.info.id == "unseen-1")
            .expect("the unmarked session is in the reply");
        assert_eq!(unseen.seen_activity_at, None);

        // The wire-shape half of the contract: a never-seen row's key must
        // be PRESENT and `null`, not simply absent the way an ordinary
        // `Option` field would serialize by default with `skip_serializing_if`
        // — `SessionRow` carries none for this field, which is what this
        // pins.
        let json = serde_json::to_value(unseen).expect("serialize the row");
        assert_eq!(
            json.get("seen_activity_at"),
            Some(&serde_json::Value::Null),
            "an unseen row's key must be present and null, never absent — a \
             client tells \"never seen\" from \"this helm predates the \
             field\" by exactly that distinction"
        );
    }
}
