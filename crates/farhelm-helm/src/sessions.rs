//! `/api/sessions` — the list, the routing every operation on one session
//! goes through, and the handlers themselves.
//!
//! The bulk of what a client does with a helm happens here, and the two
//! halves are worth separating in the reader's head: WHERE a request goes,
//! and what it does once it gets there.
//!
//! ## Routing is the interesting half
//!
//! A helm holds no authoritative session state, so every operation on a
//! session has to find the host that owns it and reach that host's LIVE
//! connection. `resolve_owner` answers the first question from the two
//! places a session can be known (helm.db's cache, and the manager's
//! in-memory list for a connected host with no identity to cache under);
//! `route_session` and `host_client` answer the second, and both take the
//! host's state and its client from ONE borrow of the actor's published
//! status. What that buys is COHERENCE, not freshness: the pair is
//! guaranteed to describe the same incarnation, so an operation is never
//! aimed at a client from one connection while a state from another said it
//! was fine. The connection can still be replaced the instant after the
//! borrow — nothing at this layer can prevent that, and nothing needs to:
//! the operation then fails against a dead client, which is an honest error,
//! rather than succeeding against the wrong machine.
//!
//! Every non-connected state refuses identically, through `refusal_text`.
//! That uniformity is deliberate: unreachable is only the most common of
//! the ways a host can fail to be connected — skew, identity mismatch, an
//! unverified identity, a duplicate, a retired row, and a first connection
//! still in flight are the others — and a caller that special-cases some of
//! them mis-handles the rest.
//!
//! ## The list never fans out
//!
//! `list_sessions` is served entirely from what the helm has already
//! recorded — helm.db plus the manager's memory — so a slow or flapping
//! host cannot slow a list poll down. The cost is that a session created
//! by ANOTHER client appears only after its host's next refresh. Sessions
//! created through this helm do not pay it: `record_session` seeds them at
//! create time, which is also what makes them routable immediately.
//!
//! ## Mutations write back what the host just said
//!
//! Create, restart, and rename all record their reply
//! (`record_session`), and delete forgets (`forget_session`). Without
//! that, the list — which is served from the recording, not from the host —
//! would show the user their own successful action as a no-op for up to a
//! refresh interval.

use crate::manager;
use crate::{
    AppState, CreateExtras, SupervisorClient, SupervisorError, aggregate, http_error, launches,
    store,
};
use anyhow::Context;
use axum::extract::{Path as AxPath, Query, State};
use axum::response::IntoResponse;
use farhelm_proto::{ErrorKind, ProfileSnapshot};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::warn;

/// Query parameters for `GET /api/sessions`: the filters and order. There is
/// no cursor and no page size, by contract (SPEC.md's
/// Session list section): the reply is the whole list.
///
/// The filter parameters are SPEC.md's session-list dimensions: host,
/// parent, directory, profile, status, and title. Their match semantics live on
/// [`store::SessionFilter`], which is also where both the persisted and the
/// in-memory sources read them from, so there is one definition rather than
/// one per source. A parameter present but EMPTY is treated as absent
/// (`?title=` is what a cleared search box sends, and refusing it would make
/// clearing the box an error).
///
/// Unknown parameters are ignored rather than refused — deliberately, so a
/// client one version behind that still sends the paged design's `limit=`
/// and `cursor=` gets the whole list instead of an error. The cost is that
/// such a client's paging is silently inert rather than loudly rejected.
#[derive(Deserialize)]
pub(crate) struct ListQuery {
    /// Only sessions on this registered host (a `HostView::id`).
    host: Option<store::HostId>,
    /// Only direct children of this session id.
    parent: Option<String>,
    /// Only sessions whose working directory CONTAINS this text, ignoring
    /// case.
    directory: Option<String>,
    /// Only sessions created from this profile, named either by its id or
    /// by the name they snapshotted at creation — which is what keeps a
    /// DELETED profile's sessions findable. See [`store::SessionFilter`].
    profile: Option<String>,
    /// Only sessions in this status, spelled exactly as the wire spells it
    /// (`running`, `waiting`, `idle`, `exited`, `error`, `interrupted`,
    /// `unknown`). An unrecognized word is a 400 rather than an empty list:
    /// a typo that answers "no sessions" is a lie the user will believe.
    status: Option<String>,
    /// Only sessions whose title CONTAINS this text, ignoring case.
    title: Option<String>,
    /// Which order to serve the list in: `created` (the default when the
    /// parameter is absent), `activity`, or `title`. See
    /// [`store::ListSort`] for what each one is and for the tie-break tail
    /// they share.
    ///
    /// Absent means `created`, which is what every client and test written
    /// before there was a choice keeps getting. An unrecognized word is a
    /// 400 for the same reason an unknown status is: a list silently served
    /// in a different order than the one asked for is one the user reads as
    /// authoritative and has no way to question.
    ///
    /// It is not a filter. It changes the sequence, never the membership, so
    /// neither count in the reply moves with it.
    sort: Option<String>,
}

/// The selected host for composer suggestions. History is never inferred
/// from a registry row alone: this handler reuses the live connection claim
/// so a disconnected or retargeted host cannot silently donate paths from a
/// previous installation.
#[derive(Deserialize)]
pub(crate) struct LaunchHistoryQuery {
    host: store::HostId,
}

/// Shared structured and folder suggestions, scoped to one verified host.
#[derive(Serialize)]
pub(crate) struct LaunchHistoryBody {
    launches: Vec<store::LaunchHistoryEntry>,
    folders: Vec<store::FolderHistoryEntry>,
    /// The composer re-reads this authenticated surface on feed changes.
    /// Carrying the current configuration epoch lets it invalidate an old
    /// checkout preview without exposing roots or hook text in the feed.
    checkout_config_revision: i64,
}

/// A browser request to inspect one directory on the selected host.
#[derive(Deserialize)]
pub(crate) struct BrowseDirectoryReq {
    host: store::HostId,
    cwd: String,
    expected_incarnation: Option<u64>,
}

/// The bounded host-side result returned to the composer.
#[derive(serde::Serialize)]
pub(crate) struct BrowseDirectoryBody {
    cwd: String,
    parent: Option<String>,
    children: Vec<String>,
    truncated: bool,
}

/// A browser request to preview a fresh GitHub checkout: the repo text and
/// title as typed, scoped to the selected host. The browser never supplies
/// the checkout root or sees the hook — the helm resolves its own
/// configuration and the supervisor does the host-side naming.
#[derive(Deserialize)]
pub(crate) struct GithubCheckoutPreviewReq {
    host: store::HostId,
    expected_incarnation: Option<u64>,
    repo: String,
    title: Option<String>,
}

/// The preview answer the composer renders before enabling Launch: the
/// exact proposed path and host. The hook/post-clone command is never part
/// of this payload — the browser does not receive it.
pub(crate) type GithubCheckoutPreviewBody = farhelm_proto::AcceptedGithubPreview;

/// Repository completion is scoped to the destination connection selected by
/// the composer. The query is the text after `gh:`, not a filesystem path.
#[derive(Deserialize)]
pub(crate) struct GithubRepositoriesReq {
    host: store::HostId,
    expected_incarnation: Option<u64>,
    query: String,
}

/// Discovery status travels with usable suggestions. An incomplete scan cannot
/// forbid a manually entered repository, and its error must not disable ordinary
/// folder or agent choices. The installation claim lets the composer discard
/// an answer after adoption, reconnect, or a destination change.
#[derive(Serialize)]
struct GithubRepositoriesBody {
    host: String,
    incarnation: u64,
    installation_identity: String,
    repos: Vec<farhelm_proto::GithubRepo>,
    truncated: bool,
    scan_error: Option<String>,
}

/// Ask the selected target to inspect local clone origins using helm-owned
/// configuration. Missing configuration and scan failure are explicit incomplete
/// observations, rather than a successful empty inventory or an error affecting
/// the composer's independent launch choices. Accepted repo intent ranks first
/// and remains available from the registry's installation history while offline.
pub(crate) async fn github_repositories(
    State(state): State<Arc<AppState>>,
    axum::Json(req): axum::Json<GithubRepositoriesReq>,
) -> impl IntoResponse {
    if req.query.len() > 4096 {
        return http_error(
            SupervisorError {
                kind: ErrorKind::InvalidRequest,
                message: "repository query exceeds the 4096-byte limit".into(),
            }
            .into(),
        );
    }
    let Some(status) = state.manager.status(req.host) else {
        return http_error(
            SupervisorError {
                kind: ErrorKind::NotFound,
                message: format!("no such host: {}", req.host),
            }
            .into(),
        );
    };
    // Offline suggestions belong to the registry's accepted installation,
    // never the identity of an unadopted replacement peer. Connected hosts
    // instead use the same status snapshot that supplies the scan client.
    let identity = match &status.state {
        manager::HostState::Connected { identity, .. } => identity.clone(),
        _ => match state.store.list_hosts().await {
            Ok(hosts) => hosts
                .into_iter()
                .find(|host| host.id == req.host)
                .and_then(|host| host.host_identity),
            Err(error) => return http_error(error),
        },
    };
    let claim = manager::SessionClaim {
        host: req.host,
        incarnation: status.incarnation,
        identity,
    };
    if let Err(error) = crate::precondition::incarnation_holds(&claim, req.expected_incarnation) {
        return http_error(error);
    }
    let Some(installation_identity) = claim
        .identity
        .clone()
        .filter(|identity| !identity.is_empty())
    else {
        return http_error(
            SupervisorError {
                kind: ErrorKind::Conflict,
                message: "repository discovery requires a stable host installation identity".into(),
            }
            .into(),
        );
    };
    let config = match state.store.resolve_checkout_config(Some(claim.host)).await {
        Ok(config) => config,
        Err(error) => return http_error(error),
    };
    let recent = match state
        .store
        .github_repository_history(claim.host, &installation_identity)
        .await
    {
        Ok(recent) => recent,
        Err(error) => return http_error(error),
    };
    let result = if let Some(client) = status.client.filter(|_| config.root.is_some()) {
        client
            .github_repo_search(
                farhelm_proto::ClaimContext {
                    host: claim.host.to_string(),
                    incarnation: claim.incarnation,
                },
                &req.query,
                config.root,
            )
            .await
            // A remote diagnostic may contain arbitrary config text or URLs.
            // Keep discovery's public response limited to identities and a
            // bounded actionable status, including when the scan itself fails.
            .map_err(|_| "repository discovery is unavailable on this host; verify Git and the configured checkout root".to_string())
    } else if config.root.is_none() {
        Err(
            "no checkout root is configured; set one with `farhelm helm checkout-config set-root`"
                .to_string(),
        )
    } else {
        Err("repository discovery is unavailable while this host is offline; recent repositories are still available".to_string())
    };
    let (repos, mut truncated, scan_error) = match result {
        Ok((repos, truncated)) => (repos, truncated, None),
        Err(error) => (Vec::new(), true, Some(error)),
    };
    // Remote data is not authority to manufacture a repository identity. Keep
    // only canonical parser-validated pairs and cap again at the REST boundary.
    let mut validated = std::collections::BTreeMap::new();
    for repo in repos {
        let key = format!("{}/{}", repo.owner, repo.name);
        if farhelm_proto::parse_github_repo(&key).ok().as_ref() != Some(&repo) {
            truncated = true;
            continue;
        }
        if validated.len() == farhelm_proto::GITHUB_REPO_RESULTS_CAP
            && !validated.contains_key(&key)
        {
            truncated = true;
            continue;
        }
        validated.insert(key, repo);
    }
    // Recent intent wins over alphabetically ordered discoveries. Filter it
    // locally because the supervisor sees only its own scanned candidates.
    let query = req.query.to_ascii_lowercase();
    let mut repos = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for repo in recent.into_iter().chain(validated.into_values()) {
        let key = format!("{}/{}", repo.owner, repo.name);
        if !key.contains(&query) || !seen.insert(key) {
            continue;
        }
        if repos.len() == farhelm_proto::GITHUB_REPO_RESULTS_CAP {
            truncated = true;
            break;
        }
        repos.push(repo);
    }
    axum::Json(GithubRepositoriesBody {
        host: claim.host.to_string(),
        incarnation: claim.incarnation,
        installation_identity,
        repos,
        truncated,
        scan_error,
    })
    .into_response()
}

/// POST /api/github-checkout-preview resolves the helm's checkout
/// configuration in ONE database snapshot and asks the selected supervisor
/// to expand `~`, canonicalize, and propose the deterministic name.
/// NOTHING is created — the create carries this binding and the supervisor
/// re-verdicts it under directory admission.
pub(crate) async fn github_checkout_preview(
    State(state): State<Arc<AppState>>,
    axum::Json(req): axum::Json<GithubCheckoutPreviewReq>,
) -> impl IntoResponse {
    let (claim, client) = match host_client(&state, req.host) {
        Ok(target) => target,
        Err(error) => return http_error(error),
    };
    if let Err(error) = crate::precondition::incarnation_holds(&claim, req.expected_incarnation) {
        return http_error(error);
    }
    let Some(installation_identity) = claim.identity.clone().filter(|id| !id.is_empty()) else {
        return http_error(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::Conflict,
            message: "fresh checkout preview requires a host with a stable installation identity"
                .into(),
        }));
    };
    // One database snapshot: root + revision are resolved together so the
    // binding the create later presents is internally consistent.
    let config = match state.store.resolve_checkout_config(Some(claim.host)).await {
        Ok(config) => config,
        Err(error) => return http_error(error),
    };
    let Some(root) = config.root.clone() else {
        return http_error(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message: "no checkout root is configured; set one with \
                      `farhelm helm checkout-config set-root`"
                .to_string(),
        }));
    };
    let request = farhelm_proto::GithubPreviewRequest {
        host: Some(claim.host.to_string()),
        expected_incarnation: req.expected_incarnation,
        repo: req.repo,
        title: req.title,
        root: Some(root),
        config_revision: Some(config.config_revision),
    };
    match client.github_checkout_preview(request).await {
        Ok(mut preview) => {
            // The supervisor cannot know the helm's per-host incarnation;
            // the authoritative claim is stamped here, and the create is
            // bound against exactly this pair.
            preview.claim_context.host = claim.host.to_string();
            preview.claim_context.incarnation = claim.incarnation;
            axum::Json(GithubCheckoutPreviewBody {
                binding: farhelm_proto::CheckoutPreviewBinding {
                    canonical_root: preview.canonical_root,
                    basename: preview.basename,
                    cwd: preview.cwd,
                    config_revision: preview.config_revision,
                },
                host: preview.claim_context.host,
                incarnation: preview.claim_context.incarnation,
                installation_identity,
            })
            .into_response()
        }
        Err(error) => http_error(error),
    }
}

/// POST /api/browse-directory asks the selected supervisor, never the helm,
/// to list an immediate directory level.
pub(crate) async fn browse_directory(
    State(state): State<Arc<AppState>>,
    axum::Json(req): axum::Json<BrowseDirectoryReq>,
) -> impl IntoResponse {
    let (claim, client) = match host_client(&state, req.host) {
        Ok(target) => target,
        Err(error) => return http_error(error),
    };
    if let Err(error) = crate::precondition::incarnation_holds(&claim, req.expected_incarnation) {
        return http_error(error);
    }
    match client.browse_directory(&req.cwd).await {
        Ok((cwd, parent, children, truncated)) => {
            if let Some(identity) = claim.identity.as_deref()
                && let Err(error) = state
                    .store
                    .refine_folder_history(claim.host, identity, &req.cwd, &cwd)
                    .await
            {
                // Browsing is still a correct supervisor answer when local
                // suggestion maintenance loses a race with host adoption.
                // The identity-scoped next create/browse will repair it.
                tracing::warn!(host = claim.host, %error, "could not refine browsed folder history");
            }
            axum::Json(BrowseDirectoryBody {
                cwd,
                parent,
                children,
                truncated,
            })
            .into_response()
        }
        Err(error) => http_error(error),
    }
}

/// GET /api/launch-catalog returns the release-owned choices the helm will
/// validate and compile for structured creates.
///
/// It is deliberately independent of host reachability: this is a property
/// of the helm build, while a later create remains guarded against the host
/// connection that the user selected.
pub(crate) async fn launch_catalog() -> impl IntoResponse {
    axum::Json(launches::catalog())
}

/// GET /api/launch-history returns reusable successful-create history for a
/// currently connected host.
pub(crate) async fn launch_history(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LaunchHistoryQuery>,
) -> impl IntoResponse {
    let (claim, _) = match host_client(&state, query.host) {
        Ok(target) => target,
        Err(error) => return http_error(error),
    };
    let checkout_config_revision = match state.store.checkout_config_snapshot(None).await {
        Ok(snapshot) => snapshot.revision,
        Err(error) => return http_error(error),
    };
    let Some(identity) = claim.identity else {
        return axum::Json(LaunchHistoryBody {
            launches: Vec::new(),
            folders: Vec::new(),
            checkout_config_revision,
        })
        .into_response();
    };
    match tokio::try_join!(
        state.store.launch_history(claim.host, &identity),
        state.store.folder_history(claim.host, &identity),
    ) {
        Ok((launches, folders)) => axum::Json(LaunchHistoryBody {
            launches,
            folders,
            checkout_config_revision,
        })
        .into_response(),
        Err(error) => http_error(error),
    }
}

/// Build the merged view's predicate from one request's query string, or
/// refuse it.
///
/// The one place the wire's spelling meets [`store::SessionFilter`].
///
/// The EXACTLY-EMPTY value is dropped rather than matched against, so a
/// cleared search box widens the list instead of narrowing it to sessions
/// whose title contains the empty string (which is all of them, but by
/// accident rather than by intent — and would count as "filtered" for the
/// two-totals reply).
///
/// Nothing else is dropped, and specifically not surrounding whitespace:
/// a directory or a title may legitimately contain it, and a session in
/// `/srv/my project/` or titled `fix  the  spacing` must stay findable by
/// typing what is actually there. Trimming would also make two different
/// searches — `" "` and `""` — into the same request, which is the one case
/// a user can see: typing a space would silently clear the filter. The cost
/// of not trimming is a search for `"drain "` that finds nothing, which the
/// user can see and fix.
fn list_filter(q: &ListQuery) -> anyhow::Result<store::SessionFilter> {
    let present = |value: &Option<String>| -> Option<String> {
        value
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    };
    let mut filter = store::SessionFilter::default();
    if let Some(host) = q.host {
        filter = filter.host(host);
    }
    if let Some(parent) = present(&q.parent) {
        filter = filter.parent(&parent);
    }
    if let Some(directory) = present(&q.directory) {
        filter = filter.directory(&directory);
    }
    if let Some(profile) = present(&q.profile) {
        filter = filter.profile(&profile);
    }
    if let Some(title) = present(&q.title) {
        filter = filter.title(&title);
    }
    if let Some(status) = present(&q.status) {
        let known = store::parse_status_key(&status).ok_or_else(|| {
            anyhow::Error::new(SupervisorError {
                kind: ErrorKind::InvalidRequest,
                message: format!(
                    "{status:?} is not a session status; this helm knows running, waiting, idle, \
                     exited, error, interrupted, and unknown"
                ),
            })
        })?;
        filter = filter.status(known);
    }
    Ok(filter)
}

/// Read one request's `?sort=`, or refuse it.
///
/// Absent — and, on the same "an empty value is a cleared control" rule
/// [`list_filter`] applies, exactly-empty — means the default order rather
/// than an error: a client that renders a sort control and clears it is
/// asking for the ordinary list, not making a mistake.
///
/// That an exactly-empty `?sort=` reads as absent is a CONSISTENCY decision,
/// not an accident of parsing. Every other listing parameter this handler
/// takes treats `?x=` as "not narrowing by x" — the convention
/// [`store::SessionFilter`] documents and [`list_filter`] applies — and a
/// query string that clears four controls one way and refuses the fifth would
/// be a rule nobody could hold in their head. The 400 is reserved for a word
/// that means something this build does not serve.
fn list_sort(q: &ListQuery) -> anyhow::Result<store::ListSort> {
    let Some(sort) = q.sort.as_deref().filter(|text| !text.is_empty()) else {
        return Ok(store::ListSort::default());
    };
    store::parse_sort_key(sort).ok_or_else(|| {
        anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message: format!(
                "{sort:?} is not a session list order; this helm serves created, activity, and \
                 title"
            ),
        })
    })
}

/// `GET /api/sessions` — the whole MERGED, multi-host session list, as one
/// array.
///
/// The rows are every registered host's sessions in ONE order across the
/// fleet — creation time by default, recent activity or title on request
/// (`?sort=`) — each tagged with the host it lives on and marked `stale` when
/// that host is not currently connected. SPEC.md's "sessions on an
/// unreachable host stay in the list, clearly marked" is this handler plus
/// the cache behind it, and nothing else.
///
/// The order is produced in memory from the helm's own cache; no host is
/// asked to sort anything (see [`aggregate`]'s module docs).
///
/// The body is `sessions`/`total`/`matching`/`truncated`
/// ([`aggregate::SessionListBody`]). `matching` is present whenever a
/// predicate is active. `total` counts the merged view before those filters
/// are applied. `truncated` means the
/// client is not looking at the whole view — some host's reply or the
/// merge hit `farhelm_proto::LIST_SESSIONS_CAP` — and is the only thing
/// behind SPEC.md's "could not read to the end" notice.
///
/// The filter parameters narrow the list server-side, which is what makes
/// "N matching of M" a claim about the whole view rather than about the
/// rows a client happens to hold.
///
/// Served from what the helm has already RECORDED, never by asking hosts:
/// helm.db for every host that caches, and the manager's in-memory list for
/// a connected host that has no identity to bind a cache write to. Either
/// way nothing here makes a network call, so a slow or flapping host cannot
/// slow a list poll down.
///
/// One consequence is worth stating rather than discovering: a session
/// created on ANOTHER client appears here only after its host's next
/// refresh, so this list trails such a create by up to one refresh
/// interval. A session created through this helm is recorded by the create
/// itself, and is routable immediately either way — routing does not go
/// through this handler.
pub(crate) async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let filter = match list_filter(&q) {
        Ok(filter) => filter,
        Err(e) => return http_error(e),
    };
    let sort = match list_sort(&q) {
        Ok(sort) => sort,
        Err(e) => return http_error(e),
    };
    match aggregate::session_list(&state.manager, &state.store, &filter, sort).await {
        Ok(list) => axum::Json(list).into_response(),
        Err(e) => http_error(e),
    }
}

/// Find the live connection for the host that owns `session_id`, or refuse
/// naming the state that host is actually in (PLAN_M6.md item 5).
///
/// The single owner-lookup path every session operation goes through. Two
/// properties are the whole point:
///
/// - **The state and the client are read TOGETHER**, from one borrow of the
///   actor's published status ([`manager::ConnectionManager::status`]). Two
///   separate reads can straddle a transition and hand back a fresh
///   `Connected` beside a `None` client, or a live-looking client beside a
///   dead state — which is exactly how an operation gets routed onto a
///   corpse.
/// - **Every non-connected state refuses identically**, with the state
///   named. Unreachable is not special; it is merely the common case. A
///   skewed, mismatched, unverified, duplicate, or retired host refuses the
///   same way, as does one whose first connection has not finished, because
///   the alternative is a caller that handles the states it thought of and
///   silently mis-handles the rest. Nothing queues — SPEC.md v1 refuses
///   rather than deferring.
///
/// A session nothing knows about is a 404. A session created HERE is
/// routable immediately — `create_session` seeds it into its host's cache
/// in the same handler — so that 404 means "no host has ever reported this
/// id", not "you were too quick". A session created by another client on
/// another host is the one case that waits, for up to one refresh interval,
/// which is the price of a list that never fans out to N hosts per request.
pub(crate) async fn route_session(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<(manager::SessionClaim, Arc<SupervisorClient>)> {
    let (host, status) = resolve_owner(state, session_id).await?;
    // The claim comes out of the SAME status this routed by, so an
    // operation whose reply is recorded afterwards (restart, rename) files
    // it against the connection it actually used — see
    // `manager::SessionClaim`.
    let identity = match &status.state {
        manager::HostState::Connected { identity, .. } => identity.clone(),
        _ => None,
    };
    let claim = manager::SessionClaim {
        host,
        incarnation: status.incarnation,
        identity,
    };
    let client = status.client.ok_or_else(|| {
        anyhow::Error::new(SupervisorError {
            kind: ErrorKind::Conflict,
            message: refusal_text(host, &status.state),
        })
    })?;
    Ok((claim, client))
}

/// Which host owns `session_id`, and that host's live status — read
/// together, from the two places a session can be known.
///
/// helm.db answers for every host that caches; the manager's in-memory
/// lists answer for a connected host that reports no identity and has none
/// on record, which therefore caches nothing (see
/// [`manager::HostSnapshot::live_sessions`]). Both are consulted because
/// either alone leaves a whole class of session unroutable: without the
/// first, nothing survives a helm restart; without the second, an
/// identity-less host reads as connected and empty while its sessions are
/// unreachable.
///
/// The in-memory lookup and the status it returns come from ONE hold of the
/// manager's actor map ([`manager::ConnectionManager::live_owner`]), not
/// from a snapshot followed by a second call. Split across two reads, a
/// reconnect landing in between pairs one install's session claim with the
/// next install's client — the same hazard the status accessor exists to
/// prevent for the cached case, and it deserves the same answer rather than
/// a second, weaker one.
///
/// The lookup is deliberately independent of whether the session's cached
/// METADATA still decodes: routing asks where to send an operation, not
/// what the session is, so a poisoned `info_json` must not make a live
/// session unreachable.
///
/// The cache consulted here holds at most `farhelm_proto::LIST_SESSIONS_CAP`
/// rows per host, so a session older than everything under a capped host's
/// cut resolves as not-found even though it still exists on the machine.
/// Accepted by decision: SPEC.md's Session list section places a fleet past
/// the cap outside what this product is built for, and the listing's
/// "could not read to the end" notice is the whole of the answer to one.
///
/// FAILS CLOSED where two hosts claim one id, with the ambiguity named —
/// including a collision a create discovered and recorded
/// ([`AppState::contested_sessions`]). helm.db makes that unconstructible
/// within itself, but a create can still mint an id another host already
/// holds, and picking one would mean a stop aimed at one machine landing on
/// another. A contested entry clears itself as soon as the fleet agrees
/// again, so a collision that resolved needs no intervention.
async fn resolve_owner(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<(store::HostId, manager::HostStatus)> {
    // Contested claims come first, from live refresh state rather than a
    // remembered incident: a host that STILL reports an id another host's
    // cache holds is a standing disagreement, and there is no honest owner
    // to route to while it stands. A claimant that stopped reporting the
    // id, was removed, or had its cache purged by an adoption is simply not
    // in this answer — the contest clears itself with the evidence that
    // made it.
    let contested = state.manager.contested_claimants(session_id);
    let cached = state.store.host_of_session(session_id).await?;
    // The list is sorted for stable reporting, not for authority: its first
    // claimant can be the cached owner while a later claimant still makes
    // the route unsafe. A sole self-claim remains harmless.
    if let Some(owner) = cached
        && let Some(claimant) = contested.into_iter().find(|claimant| *claimant != owner)
    {
        return Err(anyhow::Error::new(
            store::HostStoreError::SessionOwnerAmbiguous {
                session: session_id.to_string(),
                first: owner.min(claimant),
                second: owner.max(claimant),
            },
        ));
    }

    let live = state.manager.live_owner(session_id)?;
    match (cached, live) {
        (None, None) => Err(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::NotFound,
            message: format!("no such session: {session_id}"),
        })),
        // The in-memory answer carries its own status from the same lock
        // hold, so it is used as it stands rather than looked up again.
        (None, Some((host, status))) => Ok((host, status)),
        (Some(host), Some((live_host, _))) if host != live_host => Err(anyhow::Error::new(
            store::HostStoreError::SessionOwnerAmbiguous {
                session: session_id.to_string(),
                first: host.min(live_host),
                second: host.max(live_host),
            },
        )),
        (Some(host), _) => {
            let status = state.manager.status(host).ok_or_else(|| {
                anyhow::Error::new(SupervisorError {
                    kind: ErrorKind::Conflict,
                    message: format!(
                        "session {session_id} lives on host {host}, which is no longer registered"
                    ),
                })
            })?;
            // RE-READ the cached owner after capturing the status, and
            // refuse if it moved. An adoption landing between the two reads
            // purges one host's cache and connects another, so the pair
            // taken naively can be "host A owns it" beside "host B's live
            // connection" — an operation sent to the wrong machine, with
            // nothing about either read looking wrong. Refusing is the only
            // safe answer available at this layer: the caller retries and
            // gets a coherent pair.
            let still = state.store.host_of_session(session_id).await?;
            if still != Some(host) {
                return Err(anyhow::Error::new(SupervisorError {
                    kind: ErrorKind::Conflict,
                    message: format!(
                        "session {session_id} changed hosts while this request was being routed; \
                         retry it"
                    ),
                }));
            }
            Ok((host, status))
        }
    }
}

/// The refusal sentence a non-connected host produces, for a session
/// operation and for a create alike.
///
/// Written once because SPEC.md requires the host's state to be IN the
/// error and requires errors to be actionable: two hand-written versions
/// would drift, and the one that drifted would be the one a user actually
/// read. The phase label is the same vocabulary the hosts list chips and
/// the log lines use ([`manager::HostState::phase`]), so a user comparing
/// an error against the hosts panel sees the same word in both.
fn refusal_text(host: store::HostId, state: &manager::HostState) -> String {
    let detail = match state {
        manager::HostState::Connecting { last_error, .. } => last_error
            .clone()
            .unwrap_or_else(|| "the first connection attempt has not finished yet".to_string()),
        manager::HostState::Unreachable { last_error, .. } => last_error.clone(),
        manager::HostState::VersionSkew {
            peer_protocol,
            our_protocol,
            remediation,
            ..
        } => format!(
            "the host speaks protocol {peer_protocol} and this helm speaks {our_protocol}; \
             {remediation}"
        ),
        manager::HostState::IdentityMismatch { recorded, reported } => format!(
            "the host now reports identity {reported} where {recorded} was recorded; adopt the \
             new identity or fix the destination"
        ),
        manager::HostState::IdentityUnverified { recorded } => format!(
            "the host answered without an identity, so this helm cannot confirm it is still the \
             install recorded as {recorded}; fix the host so it reports its identity, or \
             retarget or remove this entry"
        ),
        manager::HostState::Duplicate { twin, .. } => {
            format!("this entry duplicates host {twin}; edit or remove it")
        }
        manager::HostState::Retired { reason } => reason.clone(),
        // Unreachable in practice — a connected host has a client and
        // never reaches this function — but stated rather than
        // `unreachable!()`: a panic on the refusal path would turn a
        // routing race into a dropped connection.
        manager::HostState::Connected { .. } => "the host connected while this was decided".into(),
    };
    format!(
        "host {host} is {phase}, so this operation is refused and nothing was queued: {detail}",
        phase = state.phase()
    )
}

#[derive(Deserialize)]
pub(crate) struct CreateReq {
    cwd: String,
    /// A complete raw command, mutually exclusive with profile or structured
    /// selection. Exactly one of `invocation`, `profile_id`, `profile_name`
    /// or `launch` must be present; the helm never guesses which complete
    /// launch choice should win.
    invocation: Option<String>,
    /// The profile to create from, in PROFILE mode — a `Profile::id` from
    /// the helm catalog (`GET /api/profiles`). The id has the same meaning
    /// on every managed host because the helm resolves it before choosing a
    /// supervisor connection.
    ///
    /// A successful profile-backed create is also what UPDATES the helm's
    /// remembered default (see [`create_session`]): "last used" means a
    /// session was actually created from it, not that a picker was opened.
    profile_id: Option<String>,
    /// An exact, unambiguous helm catalog name instead of its stable id.
    /// Name resolution happens only for a new create; a known fresh intent
    /// keeps the profile snapshot that originally passed admission.
    profile_name: Option<String>,
    /// Explicit launch-composer intent. The helm compiles this into the
    /// existing resolved invocation before contacting a supervisor, so the
    /// supervisor never needs a vendor catalog or a command parser.
    launch: Option<farhelm_proto::LaunchSelection>,
    title: Option<String>,
    /// Which registered host to create on — a `HostView::id` from
    /// `GET /api/hosts` (PLAN_M6.md item 5).
    ///
    /// Optional, defaulting to the reserved LOCAL row. That is the tail of
    /// SPEC.md's own creation default ("the host of the currently open
    /// session, else the helm's own host"): the first half needs to know
    /// what the user is looking at and is therefore the client's to supply,
    /// while the fallback is a server-side fact the helm can state itself.
    /// Keeping it optional is also what leaves every hand-written caller —
    /// a curl, a script, a test — meaning the obvious thing on a
    /// single-machine setup.
    host: Option<store::HostId>,
    #[serde(default = "default_cols")]
    cols: u16,
    #[serde(default = "default_rows")]
    rows: u16,
    /// The caller's idempotency key for this create (PLAN_M3.md item 6),
    /// passed straight through to the supervisor. Optional — like `title`,
    /// an absent field decodes as `None` — so every pre-M3 caller (curl, an
    /// older UI build, the CLI's startup create) keeps working unchanged,
    /// with each request its own create.
    intent_key: Option<String>,
    /// Override of the integrated-agent kind (PLAN_M3.md item 7), forwarded
    /// verbatim to `ControlMsg::CreateSession::agent_kind` — see that
    /// field's doc comment (farhelm-proto's `lib.rs`) for the full
    /// three-state semantics. Absent, like `intent_key`, decodes as `None`
    /// and preserves pre-M3 behavior: the supervisor derives the kind from
    /// `invocation`'s basename. On the wire a present value is one of the
    /// snake_case strings `"claude"`, `"codex"`, `"generic"` — the same
    /// representation `AgentKind`'s `#[serde(rename_all = "snake_case")]`
    /// produces on the supervisor protocol, so a JSON body needs no
    /// translation between the two.
    agent_kind: Option<farhelm_proto::AgentKind>,
    /// Override of the resume invocation template (PLAN_M3.md item 7),
    /// forwarded verbatim to `ControlMsg::CreateSession::resume_template` —
    /// see that field's doc comment for the placeholder-placement rule and
    /// the integrated/non-integrated distinction it enforces. Absent
    /// decodes as `None`, same posture as `intent_key`: for a session
    /// whose EFFECTIVE kind (after any `agent_kind` override) is
    /// integrated (claude/codex), the supervisor derives the template
    /// from `invocation`'s first token instead; a generic-kind session
    /// derives none — only this explicit override can give one a
    /// (verbatim, placeholder-free) resume invocation.
    resume_template: Option<Vec<String>>,
    /// Which CONNECTION the caller prepared this create against — a
    /// `HostView::incarnation` read from `GET /api/hosts`.
    ///
    /// Optional, and absent means no claim is made (see
    /// [`crate::precondition`], which carries the whole reasoning). Present,
    /// and the create is refused with a 409 unless the host is still on that
    /// connection when routing resolves it.
    ///
    /// Profile ids are helm-wide now, but the guard still matters because
    /// "run this on THAT machine" is a claim independent of how the launch
    /// bundle was selected. A retargeted row must not silently send either a
    /// profile-backed or raw create to a successor installation.
    expected_incarnation: Option<u64>,
    /// A fresh GitHub checkout the caller asks this create to perform.
    ///
    /// Absent decodes as `None` — serde's built-in handling for `Option` —
    /// so every pre-existing caller and older UI build sends and means
    /// exactly what it always did. When PRESENT, the helm parses the
    /// repository text first (an invalid `owner/repo` is reported as the
    /// parse error it is), resolves the checkout configuration for the
    /// claimed host in one database snapshot, and sends the resolved
    /// payload to the supervisor, whose admission allocates the checkout
    /// under directory admission. A checkout root that is not configured
    /// is refused with the exact CLI command that fixes it. The root and
    /// post-clone hook come from helm configuration. Preview exposes the
    /// target-resolved root and destination for acceptance; hook text stays
    /// private to the helm and supervisor.
    ///
    /// Restricted session-authenticated callers can never supply this:
    /// they reach the supervisor directly, never this body, and the
    /// supervisor's restricted dispatcher refuses the create outright.
    github_checkout: Option<farhelm_proto::GithubCheckoutRequest>,
}

/// Freeze client-controlled launch fields before mode compilation consumes
/// selectors or normalizes titles. Dimensions and the lookup key do not shape
/// the durable create. Replacement wraps this encoding with its source id so
/// the same key cannot accidentally reconcile another replacement operation.
fn fresh_create_request_identity(req: &CreateReq) -> String {
    let identity = serde_json::to_string(&(
        "github_create_request_v1",
        &req.cwd,
        &req.invocation,
        &req.profile_id,
        &req.launch,
        &req.title,
        &req.host,
        &req.agent_kind,
        &req.resume_template,
        &req.expected_incarnation,
        &req.github_checkout,
    ))
    .expect("create request identity contains only serializable fields");
    // Preserve the existing encoding when the new selector is absent.
    // A name is client intent, not the mutable id it happens to resolve to.
    match &req.profile_name {
        Some(name) => serde_json::to_string(&("github_named_profile_v1", name, identity))
            .expect("named profile identity contains only strings"),
        None => identity,
    }
}

/// Verify the original preview's installation before any intent lookup.
/// An incarnation alone cannot establish this across helm restarts, and using
/// today's identity as the request's expected identity would silently retarget
/// a request prepared for another installation.
fn accepted_checkout_preview<'a>(
    checkout: &'a farhelm_proto::GithubCheckoutRequest,
    claim: &manager::SessionClaim,
) -> anyhow::Result<&'a farhelm_proto::AcceptedGithubPreview> {
    let preview = checkout.preview.as_ref().ok_or_else(|| SupervisorError {
        kind: ErrorKind::InvalidRequest,
        message: "fresh checkout requires an accepted preview; request a preview before launching"
            .into(),
    })?;
    if preview.host != claim.host.to_string()
        || preview.installation_identity.is_empty()
        || claim.identity.as_deref() != Some(preview.installation_identity.as_str())
    {
        return Err(SupervisorError {
            kind: ErrorKind::Conflict,
            message: "the accepted checkout preview belongs to a different host installation"
                .into(),
        }
        .into());
    }
    Ok(preview)
}

/// The helm's fresh-checkout resolution for a create body carrying
/// `github_checkout`: `None` when the field is absent and the create
/// proceeds unchanged; `Some(Ok(resolved))` when everything resolved; and
/// `Some(Err(error))` for a refusal before a new create frame or history write.
/// A keyed request may already have performed lookup-only reconciliation;
/// only an unknown result reaches this current-settings resolution.
///
/// The failure order answers different questions: a repository string that
/// does not parse is the CALLER's mistake and is reported as the parse
/// error (naming the failure class against what the user actually typed,
/// per the request type's own contract); a checkout root that is not
/// configured is the OPERATOR's gap and is reported with the exact CLI
/// command that fixes it. The resolution itself is one database snapshot —
/// root and revision together — so the binding the create presents to the
/// supervisor is internally consistent. The helm refuses a stale revision;
/// the supervisor independently verifies the recorded root and exact path.
///
/// Restricted session-authenticated callers can never supply this field:
/// they reach the supervisor directly, never this body, and the
/// supervisor's restricted dispatcher refuses the create outright. The
/// root and post-clone hook come from helm configuration. The accepted
/// preview carries the target-resolved paths, but hook text is never sent
/// to the browser.
async fn github_checkout_resolution(
    state: &AppState,
    claim: &manager::SessionClaim,
    req: &CreateReq,
    client_identity: String,
) -> Option<anyhow::Result<Option<farhelm_proto::ResolvedGithubCheckout>>> {
    let checkout = req.github_checkout.as_ref()?;
    if let Err(error) = farhelm_proto::parse_github_repo(&checkout.repo) {
        return Some(Err(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message: format!(
                "invalid GitHub repository {:?}: {error}",
                truncate_repo_text(&checkout.repo)
            ),
        })));
    }
    // One database snapshot: root and revision resolved together, the same
    // pair the preview endpoint presents.
    let config = match state.store.resolve_checkout_config(Some(claim.host)).await {
        Ok(config) => config,
        Err(error) => return Some(Err(error)),
    };
    let Some(root) = config.root else {
        return Some(Err(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message: "no checkout root is configured; set one with \
                      `farhelm helm checkout-config set-root`"
                .to_string(),
        })));
    };
    let repo = farhelm_proto::parse_github_repo(&checkout.repo)
        .expect("the parse above already accepted this identifier");
    let Some(preview) = &checkout.preview else {
        return Some(Err(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message:
                "fresh checkout requires an accepted preview; request a preview before launching"
                    .into(),
        })));
    };
    if preview.binding.config_revision != config.config_revision {
        return Some(Err(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::Conflict,
            message: "checkout settings changed since the accepted preview; request a new preview"
                .into(),
        })));
    }
    // REST callers may retain the displayed preview path in their request,
    // but it is not a second destination. Reject contradictory folder state
    // before converting this request to the supervisor's fresh-only shape.
    if !req.cwd.is_empty() && req.cwd != preview.binding.cwd {
        return Some(Err(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message: "fresh checkout cwd must be empty or match the accepted preview".into(),
        })));
    }
    Some(Ok(Some(farhelm_proto::ResolvedGithubCheckout {
        client_identity,
        repo,
        root,
        post_clone: config.post_clone,
        preview: preview.binding.clone(),
    })))
}

/// The repository text as typed is user input that an error message will
/// quote; bound it the way other echoed free text on this surface is
/// bounded rather than quoting an arbitrarily long string verbatim.
fn truncate_repo_text(repo: &str) -> &str {
    match repo.char_indices().nth(120) {
        Some((idx, _)) => &repo[..idx],
        None => repo,
    }
}

// Dimensions for a caller that has no terminal yet — the CLI, a script,
// a UI dialog that has not laid out a pane. 80x24 is a guess and it does
// not have to be a good one: the first attach resizes the window to the
// real client size, so these only decide how the agent's first few lines
// wrap before anyone is looking.
pub(crate) fn default_cols() -> u16 {
    80
}
pub(crate) fn default_rows() -> u16 {
    24
}

/// The live connection for one NAMED host, plus the claim that pins WHICH
/// connection it was — or a refusal naming the state that host is in.
///
/// The host-scoped twin of [`route_session`], which answers the same
/// question for a host it first has to derive from a session id. Both exist
/// so that the state and the client are read TOGETHER, from one borrow of
/// the actor's published status ([`manager::ConnectionManager::status`]):
/// two separate reads can straddle a transition and hand back a fresh
/// `Connected` beside a `None` client, or a live-looking client beside a
/// dead state, which is exactly how an operation gets routed onto a corpse.
///
/// An UNREGISTERED host is a 404 and a registered-but-not-connected host is
/// a conflict, because they are different things to the caller: the first
/// is a name that was never valid, the second is a condition that clears on
/// its own once the host comes back.
///
/// Synchronous, unlike [`route_session`]: naming a host outright skips the
/// helm.db lookup the owner search needs, and the manager's published status
/// is behind a plain lock. Nothing here awaits, so nothing here should
/// pretend it might.
///
/// Visible to the crate because REST and agent creates both route through
/// it. Profile CRUD no longer needs a host connection: the catalog belongs
/// to the helm.
pub(crate) fn host_client(
    state: &AppState,
    host: store::HostId,
) -> anyhow::Result<(manager::SessionClaim, Arc<SupervisorClient>)> {
    let status = state.manager.status(host).ok_or_else(|| {
        anyhow::Error::new(SupervisorError {
            kind: ErrorKind::NotFound,
            message: format!("no such host: {host}"),
        })
    })?;
    // The claim is taken from the SAME read that produced the client, so
    // the seed that follows can prove it is still talking about this
    // connection — see `manager::SessionClaim`.
    let identity = match &status.state {
        manager::HostState::Connected { identity, .. } => identity.clone(),
        // Unreachable in practice: a client is published exactly while the
        // state is `Connected`, and the `ok_or_else` below is what turns
        // every other state into a refusal. Written as a value rather than
        // `unreachable!()` because a panic on the create path would be a
        // far worse answer than a seed that later declines itself.
        _ => None,
    };
    let claim = manager::SessionClaim {
        host,
        incarnation: status.incarnation,
        identity,
    };
    let client = status.client.ok_or_else(|| {
        anyhow::Error::new(SupervisorError {
            kind: ErrorKind::Conflict,
            message: refusal_text(host, &status.state),
        })
    })?;
    Ok((claim, client))
}

/// The connection to create on: the body's `host`, or the reserved local
/// row when the body named none (see [`CreateReq::host`]).
///
/// A create against a host in ANY non-connected state is a PRECONDITION
/// FAILURE, exactly as SPEC.md's creation section demands — a visible error
/// naming the host's state, and no session anywhere. That refusal is
/// [`host_client`]'s, shared with the lifecycle routes on purpose:
/// "unreachable host" is listed in SPEC.md beside "nonexistent directory"
/// as one of the preconditions that fail a create, and every other
/// non-connected state is the same failure with a different cause.
///
/// The default is resolved by KIND rather than by a remembered id: the
/// reserved local row is the one host every helm has, and a body that names
/// no host means "here" — see [`CreateReq::host`].
fn create_target(
    state: &AppState,
    host: Option<store::HostId>,
) -> anyhow::Result<(manager::SessionClaim, Arc<SupervisorClient>)> {
    let snapshots = state.manager.snapshots();
    let host =
        match host {
            Some(host) => host,
            None => snapshots
                .iter()
                .find(|snapshot| snapshot.kind == store::HostKind::Local)
                .context(
                    "this helm has no local host row, so a create naming no host has no default \
                     target",
                )?
                .id,
        };
    host_client(state, host)
}

/// Record what a host just told us about one session, where the serving
/// path will find it.
///
/// Called by every mutation whose reply carries a fresh `SessionInfo`, and
/// each has its own reason:
///
/// - **Create.** Without this a create is followed by a window — up to one
///   refresh interval — in which every operation on the session it just
///   returned 404s, because routing resolves owners from what the helm has
///   recorded and the helm has recorded nothing yet. Not a theoretical gap:
///   the create dialog's own flow is "create, then open the terminal",
///   which lands in exactly that window.
/// - **Restart and rename.** The list is served from what the helm has
///   recorded, so a mutation whose result was not recorded leaves the row
///   showing the PREVIOUS state for a poll interval. A user who restarts an
///   exited session and watches the list keep saying `exited` has been shown
///   their own successful action as a failure (observed in the browser
///   suite, which is this behavior's regression test). Recording the reply
///   the host just sent costs nothing and closes it.
///
/// Goes through the MANAGER rather than straight to the store, and that is
/// not indirection for its own sake. The manager is what knows the two
/// things this write depends on: which storage a host uses (a host with no
/// identity caches nothing and serves from memory, and its created sessions
/// have to land there or they are invisible too), and whether the
/// connection the create used is still the current one. It is also what
/// serializes this write against the host's own refresh, so a drain that
/// predates the create cannot commit its wholesale replacement afterwards
/// and erase it.
///
/// BEST EFFORT for a stale claim, and deliberately not fatal: the session
/// exists and the caller must be told about it, since reporting a create
/// that actually succeeded as a failure is the one outcome SPEC.md's
/// creation contract rules out. Every such failure is self-healing within
/// one refresh — the host has the session and will report it.
///
/// AMBIGUITY IS THE EXCEPTION, and it is reported rather than swallowed:
/// if the session id is already cached under a DIFFERENT host there is no
/// honest owner, and routing would silently pick the other one. The
/// standing collision itself is not remembered HERE — it is refresh state
/// on the hosts that report it (`manager::ActorStatus::contested`), so it
/// clears itself when they stop.
async fn record_session(
    state: &AppState,
    claim: &manager::SessionClaim,
    session: &farhelm_proto::SessionInfo,
) {
    // Every caller resolves the reply once and passes that same row both to
    // the cache and to its consumer. Refuse to persist a supervisor marker
    // if a future caller bypasses that boundary.
    if session
        .source_profile
        .as_ref()
        .is_some_and(|source| source.existence == farhelm_proto::ProfileExistence::Unresolved)
    {
        warn!(session = %manager::peer_text(&session.id), "refusing to cache a session whose profile existence is unresolved");
        return;
    }
    let Err(error) = state.manager.remember_session(claim, session).await else {
        return;
    };
    // The id is the PEER's text — escaped and bounded before it reaches a
    // log line, like every other peer-supplied value this process writes.
    let session_id = manager::peer_text(&session.id);
    if let Some(store::HostStoreError::SessionOwnerAmbiguous { first, second, .. }) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<store::HostStoreError>())
    {
        warn!(
            host = claim.host,
            session_id = session_id.as_str(),
            first,
            second,
            "the host reported a session id another host already claims; it will not be routed \
             while both keep claiming it"
        );
        return;
    }
    warn!(
        host = claim.host,
        session_id = session_id.as_str(),
        error = %error,
        "could not record the session for routing; it will be picked up at the next refresh"
    );
}

/// The catalog fields needed to classify a session's immutable profile
/// snapshot.
///
/// Launch settings stay out of this index deliberately. Existence depends
/// only on stable identity and the current display name; keeping the smaller
/// view also makes it clear that resolving a reply cannot change what the
/// session will run.
pub(crate) type ProfileNameIndex = HashMap<String, String>;

/// Load one catalog snapshot for use on both sides of a supervisor mutation.
///
/// Mutation handlers call this before sending the operation. A failed read
/// must therefore fail before the supervisor changes anything, while a
/// successful read leaves reply enrichment infallible after the side effect.
pub(crate) async fn load_profile_name_index(
    store: &store::HelmStore,
) -> anyhow::Result<ProfileNameIndex> {
    Ok(profile_name_index(&store.profiles().await?))
}

/// Reduce a decoded catalog to the identity fields session replies need.
///
/// Profile-backed creates already need the full catalog to resolve their
/// launch bundle. Building the index from that same read preserves the
/// one-read contract instead of reopening the store after creation.
fn profile_name_index(profiles: &[farhelm_proto::Profile]) -> ProfileNameIndex {
    profiles
        .iter()
        .map(|profile| (profile.id.clone(), profile.name.clone()))
        .collect()
}

/// Resolve every source-profile marker against an already-loaded catalog.
///
/// This is deliberately infallible: callers that mutate a supervisor load
/// the index before the side effect, then use this function afterwards.
/// Centralizing the three-way rule keeps live replies, cached rows, and
/// merged listings from exposing the supervisor-only `Unresolved` marker.
pub(crate) fn resolve_session_profiles<'a>(
    profiles: &ProfileNameIndex,
    sessions: impl IntoIterator<Item = &'a mut farhelm_proto::SessionInfo>,
) {
    for session in sessions {
        if let Some(source) = &mut session.source_profile {
            source.existence = match profiles.get(&source.id) {
                None => farhelm_proto::ProfileExistence::Deleted,
                Some(name) if name == &source.name => farhelm_proto::ProfileExistence::Present,
                Some(_) => farhelm_proto::ProfileExistence::Renamed,
            };
        }
    }
}

/// Resolve a read-only reply, avoiding the catalog entirely for raw rows.
///
/// A malformed profile row must not hide sessions that carry no provenance.
/// The early return happens before the store is touched; profile-backed rows
/// still fail loudly if the catalog cannot provide a trustworthy snapshot.
pub(crate) async fn resolve_session_profiles_from_store(
    store: &store::HelmStore,
    sessions: &mut [farhelm_proto::SessionInfo],
) -> anyhow::Result<()> {
    if sessions
        .iter()
        .all(|session| session.source_profile.is_none())
    {
        return Ok(());
    }
    let profiles = load_profile_name_index(store).await?;
    resolve_session_profiles(&profiles, sessions.iter_mut());
    Ok(())
}

/// Reject a session row at the HTTP edge if it still carries the
/// supervisor-only existence marker.
///
/// This release-build check complements the cache guard. A debug assertion
/// alone would let a production browser observe a fourth existence word the
/// public JSON contract does not contain.
fn browser_session_ready(session: &farhelm_proto::SessionInfo) -> anyhow::Result<()> {
    if session
        .source_profile
        .as_ref()
        .is_some_and(|source| source.existence == farhelm_proto::ProfileExistence::Unresolved)
    {
        anyhow::bail!("refusing to serialize unresolved profile existence");
    }
    Ok(())
}

/// Forget a deleted session everywhere the serving path looks for it.
///
/// The delete's half of [`record_session`]'s principle, and the quadrant
/// that was missing: a reply with no `SessionInfo` still carries the fact
/// that a session is gone, and the merged list is served from what the helm
/// has recorded. Leaving the row behind means the list shows a deleted
/// session until the owning host's next refresh — and a client that deletes
/// and immediately re-creates then sees BOTH, which is indistinguishable
/// from a duplicate. That is precisely how the browser suite found it, in
/// its own shared-session reset.
///
/// Best effort on the same terms as a seed: the delete SUCCEEDED and the
/// caller must be told so. Everything here is self-healing within one
/// refresh.
async fn forget_session(state: &AppState, claim: &manager::SessionClaim, session_id: &str) {
    if let Err(error) = state.manager.forget_session(claim, session_id).await {
        warn!(
            host = claim.host,
            session_id = manager::peer_text(session_id).as_str(),
            error = %error,
            "could not forget the deleted session; it will disappear at the next refresh"
        );
    }
}

/// `POST /api/sessions` — the creation API SPEC_impl.md calls the one true
/// path. The UI's create dialog and any script land on the same supervisor
/// call this reaches; there is no side door, and as of PLAN_M6.md item 5
/// there is no argv path either.
///
/// The body's `host` selects which registered host to create on, defaulting
/// to the local row; a host that is not connected fails the create as a
/// precondition (see [`create_target`]).
///
/// A body carrying `intent_key` gets server-enforced idempotency
/// (PLAN_M3.md item 6): a retry of the same request under the same key
/// yields the same session rather than a second one, and a key reused for
/// a DIFFERENT request comes back 409 through `http_error`. A body carrying
/// `agent_kind` and/or `resume_template` (PLAN_M3.md item 7) reaches the
/// supervisor's create validation unchanged, including its refusal of an
/// integrated kind paired with a placeholder-free template — that refusal
/// surfaces as `ErrorKind::InvalidRequest` and comes back 400 through the
/// same `http_error` mapping every other create precondition failure uses.
///
/// The reply is the created `SessionInfo`, unchanged. It carries no host
/// fields (contrast the list's rows): the caller already knows which host
/// it asked for, and inventing a second place where a session's host is
/// reported would be a second thing to keep true.
///
/// The new session is seeded into its host's cache before this answers
/// ([`seed_created_session`]), so it is routable — stop, rename, terminal —
/// the moment the caller has its id, rather than after the owning host's
/// next refresh. It joins the LIST on that next refresh like any other
/// session; the two are separate promises and only the first one is
/// something a client can be surprised by.
///
/// ## Profile mode, and the remembered default
///
/// A body naming `profile_id` instead of `invocation` resolves from the
/// helm-wide catalog before the supervisor call. Two consequences live here:
///
/// - The helm REMEMBERS the profile as its fleet-wide last-used id in
///   helm.db, but only after the create SUCCEEDS. A create that failed its
///   preconditions did not establish a preference — remembering an
///   attempted profile would make a typo the default the next dialog
///   suggests.
/// - The write is best-effort and never turns a successful create into a
///   failure. The session exists; reporting otherwise is the one outcome
///   SPEC.md's creation contract rules out, and a lost preference costs the
///   user one extra click.
///
/// A profile that no longer exists fails the create visibly, with no session
/// anywhere, and this handler does nothing to soften that: SPEC.md's rule is
/// to ask rather than guess, and a fallback to some other profile here would
/// be exactly the guess it forbids.
///
/// ## Naming the install this create was written for
///
/// An optional `expected_incarnation` says which connection the caller
/// prepared this body against, and the create is refused (409, with
/// `crate::precondition`'s marker) unless the host is still on it. Absent
/// means no claim, which is every pre-existing caller.
///
/// The profile selection itself is helm-wide, but the connection guard still
/// protects the chosen TARGET. A retarget or adoption between rendering and
/// submit would otherwise launch the right bundle on the wrong installation.
/// See [`crate::precondition`].
pub(crate) async fn create_session(
    State(state): State<Arc<AppState>>,
    axum::Json(mut req): axum::Json<CreateReq>,
) -> impl IntoResponse {
    // FIRST, before mode resolution or target routing: a fresh-checkout
    // request's repository-text parse error must win over any
    // selector-shape error a body might also carry (the user should hear
    // about the repo they typed, not an unrelated field). The parse is
    // sync; the config resolution needs the claim, so it re-runs below
    // after routing — a fresh create resolves against the host it will
    // actually land on.
    if let Some(checkout) = req.github_checkout.as_ref()
        && let Err(error) = farhelm_proto::parse_github_repo(&checkout.repo)
    {
        return http_error(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message: format!(
                "invalid GitHub repository {:?}: {error}",
                truncate_repo_text(&checkout.repo)
            ),
        }));
    }
    // Mode compilation consumes selector fields. Retain the original request
    // first so later reconciliation does not depend on mutable catalogs.
    let client_identity = fresh_create_request_identity(&req);
    let ordinary_mode = if req.github_checkout.is_none() {
        match resolve_create_mode(&state, &mut req).await {
            Ok(mode) => Some(mode),
            Err(e) => return http_error(e),
        }
    } else {
        None
    };
    let (claim, client) = match create_target(&state, req.host) {
        Ok(target) => target,
        Err(e) => return http_error(e),
    };
    if req.github_checkout.is_some() {
        let intent_key = req.intent_key.clone();
        return match create_fresh_session(
            &state,
            &claim,
            &client,
            req,
            intent_key,
            client_identity,
            None,
        )
        .await
        {
            Ok(session) => match browser_session_ready(&session) {
                Ok(()) => axum::Json(session).into_response(),
                Err(error) => http_error(error),
            },
            Err(error) => http_error(error),
        };
    }
    // Checked HERE, and once, because routing and claim-taking are one read
    // for a create: `create_target` resolves the host, takes the connection,
    // and mints the claim from the same borrow of the actor's status, so there
    // is no interval between "which install" and "which connection" for
    // anything to land in. The cache seed this create goes on to make
    // revalidates the same claim under the host's write lock; the remembered
    // default does not, by design: it is a helm-wide suggestion rather than
    // a claim about the install currently behind this registry row.
    if let Err(e) = crate::precondition::incarnation_holds(&claim, req.expected_incarnation) {
        return http_error(e);
    }
    let mode = ordinary_mode.expect("fresh requests returned through their shared admission path");
    match do_create_session(
        &state,
        &claim,
        &client,
        CreateSpec {
            cwd: req.cwd,
            mode,
            title: req.title,
            cols: req.cols,
            rows: req.rows,
            intent_key: req.intent_key,
            agent_kind: req.agent_kind,
            resume_template: req.resume_template,
            origin: CreateOrigin::User,
            // A REST create takes whatever session the target answers with,
            // replays included: the client asked for a session on that host
            // and the reply names one. Only the relay's clone has a result
            // it must refuse.
            accept_result: None,
            github_checkout: None,
        },
    )
    .await
    {
        Ok(session) => match browser_session_ready(&session) {
            Ok(()) => axum::Json(session).into_response(),
            Err(error) => http_error(error),
        },
        Err(e) => http_error(e),
    }
}

/// One create, with its host and its mode already resolved — everything
/// [`create_session`] does after routing, and nothing it does before.
///
/// Shared VERBATIM with the agent relay's `Create`/`Clone` verbs
/// (`agent_requests::HelmAgentRequests::handle`), which is the whole reason
/// it exists as a function. Both callers need the same three things to
/// happen in the same order — the supervisor call and cache seed, followed
/// by a remembered-default write only for a user profile create — and a
/// create is exactly
/// the operation where a second implementation would be most expensive to
/// get subtly wrong: an agent-initiated create that skipped
/// [`record_session`] would leave a real session running that the UI could
/// not route to for a refresh interval. The deliberate difference is that
/// an agent's profile-backed create must not move the user's dialog default.
///
/// What is deliberately NOT here is routing. Naming the target host is where
/// the two callers genuinely differ — the REST edge takes a registry id from
/// a client that read `GET /api/hosts`, the agent takes a display NAME — and
/// folding that in would mean one function with two mutually exclusive
/// halves. The `claim` a caller passes must come from the SAME
/// [`host_client`] read that produced `client`, which is what lets the cache
/// seed below revalidate against the connection the create was actually sent
/// on.
///
/// ## Two phases, and why the seam is where it is
///
/// The supervisor call comes first, then [`CreateSpec::accept_result`]'s
/// veto, and only then the bookkeeping. A caller that rejects the session
/// the target answered with is saying the create it asked for did not
/// happen — so the row must not be seeded into the cache and must not
/// rewrite the helm-wide remembered default on the way out. That is not
/// hypothetical tidiness: the clone verb's veto fires on a legitimate
/// idempotency REPLAY, where the target answers with a session that already
/// existed, and letting the bookkeeping run first can move a
/// provenance-less remembered default to the replayed session's profile and
/// wake every client with a fleet revision — durable effects of a create
/// the caller is simultaneously being told did not occur.
///
/// A hook rather than a split into two public halves because the ORDER is
/// the contract this function exists to enforce; a caller holding two
/// functions is a caller that can call one of them.
///
/// Profile-backed modes load their catalog snapshot before the supervisor
/// call and reuse it to enrich the reply. That ordering makes the only
/// catalog failure happen before creation; a successful create can no longer
/// be reported as failed because a second read broke afterwards. Raw mode
/// deliberately has no catalog dependency at either phase.
pub(crate) async fn do_create_session(
    state: &AppState,
    claim: &manager::SessionClaim,
    client: &SupervisorClient,
    spec: CreateSpec,
) -> anyhow::Result<farhelm_proto::SessionInfo> {
    let CreateSpec {
        cwd,
        mode,
        title,
        cols,
        rows,
        intent_key,
        agent_kind,
        resume_template,
        origin,
        accept_result,
        github_checkout,
    } = spec;
    // REST carries the displayed preview cwd as part of the retained client
    // request identity. The supervisor's fresh-create destination instead
    // comes exclusively from the resolved binding; its ordinary cwd input
    // must remain empty. Keep the original spelling for history bookkeeping.
    let supervisor_cwd = if github_checkout.is_some() { "" } else { &cwd };
    let (mut session, profile_names) = match &mode {
        CreateMode::Raw(invocation) => {
            let session = client
                .create_session_with_extras(
                    supervisor_cwd,
                    invocation,
                    title,
                    cols,
                    rows,
                    CreateExtras {
                        intent_key,
                        agent_kind,
                        resume_template,
                        source_profile: None,
                        launch: None,
                        github_checkout: github_checkout.clone(),
                    },
                )
                .await?;
            (session, None)
        }
        CreateMode::Structured(compiled) => {
            let session = client
                .create_session_with_extras(
                    supervisor_cwd,
                    &compiled.invocation,
                    title,
                    cols,
                    rows,
                    CreateExtras {
                        intent_key,
                        agent_kind: Some(compiled.agent_kind),
                        resume_template: compiled.resume_template.clone(),
                        source_profile: None,
                        launch: Some(compiled.selection.clone()),
                        github_checkout: github_checkout.clone(),
                    },
                )
                .await?;
            (session, None)
        }
        CreateMode::Profile(profile_id) => {
            let profiles = state.store.profiles().await?;
            let profile_names = profile_name_index(&profiles);
            let profile = profiles
                .into_iter()
                .find(|profile| profile.id == *profile_id)
                .ok_or_else(|| {
                    anyhow::Error::new(SupervisorError {
                        kind: ErrorKind::NotFound,
                        message: format!("profile not found: {profile_id}"),
                    })
                })?;
            let session = client
                .create_session_with_extras(
                    supervisor_cwd,
                    &profile.invocation,
                    title,
                    cols,
                    rows,
                    CreateExtras {
                        intent_key,
                        agent_kind: Some(profile.agent_kind),
                        resume_template: profile.resume_template,
                        source_profile: Some(ProfileSnapshot {
                            id: profile.id,
                            name: profile.name,
                        }),
                        launch: None,
                        github_checkout: github_checkout.clone(),
                    },
                )
                .await?;
            (session, Some(profile_names))
        }
        CreateMode::ResolvedProfile {
            profile,
            profile_names,
        } => {
            let session = client
                .create_session_with_extras(
                    supervisor_cwd,
                    &profile.invocation,
                    title,
                    cols,
                    rows,
                    CreateExtras {
                        intent_key,
                        agent_kind: Some(profile.agent_kind),
                        resume_template: profile.resume_template.clone(),
                        source_profile: Some(ProfileSnapshot {
                            id: profile.id.clone(),
                            name: profile.name.clone(),
                        }),
                        launch: None,
                        github_checkout: github_checkout.clone(),
                    },
                )
                .await?;
            (session, Some(profile_names.clone()))
        }
    };
    if let Some(profile_names) = &profile_names {
        resolve_session_profiles(profile_names, std::iter::once(&mut session));
    }
    let remembered_profile = match &mode {
        CreateMode::Profile(profile_id) => Some(profile_id.clone()),
        CreateMode::ResolvedProfile { profile, .. } => Some(profile.id.clone()),
        CreateMode::Raw(_) | CreateMode::Structured(_) => None,
    };
    accept_created_session(
        state,
        claim,
        session,
        CreateAcceptance {
            github_repo: github_checkout.map(|checkout| checkout.repo),
            requested_cwd: cwd,
            origin,
            accept_result,
            remembered_profile,
        },
    )
    .await
}

/// Bookkeeping intent for an accepted create, independent of launch compilation.
/// Reconciliation uses the original request's provenance without resolving a
/// profile or structured selection that may have changed since acceptance.
struct CreateAcceptance {
    /// Trusted request intent, retained across lookup-only reconciliation.
    /// SessionInfo's descriptive repo field cannot establish this authority.
    github_repo: Option<farhelm_proto::GithubRepo>,
    requested_cwd: String,
    origin: CreateOrigin,
    accept_result: Option<CreatedSessionCheck>,
    remembered_profile: Option<String>,
}

/// Apply the same source veto, cache, history and default effects to first
/// replies and reconciled replies. The veto precedes every durable side effect;
/// best-effort suggestion writes never turn an accepted create into a failure.
async fn accept_created_session(
    state: &AppState,
    claim: &manager::SessionClaim,
    session: farhelm_proto::SessionInfo,
    acceptance: CreateAcceptance,
) -> anyhow::Result<farhelm_proto::SessionInfo> {
    let CreateAcceptance {
        github_repo,
        requested_cwd,
        origin,
        accept_result,
        remembered_profile,
    } = acceptance;
    // The caller's veto, BEFORE anything durable is written for this row —
    // see this function's own "Two phases" note for why the seam is here and
    // not after the seed.
    if let Some(accept_result) = &accept_result {
        accept_result(&session)?;
    }
    record_session(state, claim, &session).await;
    // A fresh request's cwd is not its accepted destination: the target
    // allocated that path. Keep the actual path for diagnostics while repo
    // intent, independently captured from the request, controls reuse.
    // Suggestions are a convenience written only after the session exists.
    // A database failure here must not turn a successful supervisor create
    // into an HTTP error that tempts the caller to submit it again.
    if let Some(identity) = claim.identity.as_deref() {
        match state
            .store
            .record_create_history_with_destination(
                claim.host,
                identity,
                &session,
                crate::store::HistoryPaths {
                    canonical_cwd: session.canonical_cwd.as_deref().unwrap_or(&session.cwd),
                    display_cwd: if github_repo.is_some() {
                        &session.cwd
                    } else {
                        &requested_cwd
                    },
                },
                github_repo.as_ref(),
                // Only a USER-initiated create may move the helm-wide
                // remembered permissions default — the same authority
                // boundary already drawn around the remembered legacy
                // profile default a few lines below, and for the same
                // reason: an agent acting on its own (a relay clone, say)
                // must not silently change what the next human "New" open
                // preselects.
                if origin == CreateOrigin::User {
                    crate::store::LaunchChoiceMemory::Remember
                } else {
                    crate::store::LaunchChoiceMemory::Leave
                },
            )
            .await
        {
            Ok(true) => state.manager.events().bump(),
            Ok(false) => {}
            Err(error) => warn!(
                host = claim.host,
                session = %manager::peer_text(&session.id),
                error = %error,
                "could not record post-create launch history"
            ),
        }
    }
    // The remembered default is written only after the resolved create
    // succeeds. A reply that unexpectedly names no source profile writes
    // nothing: inventing an id would make the next dialog preselect a profile
    // nobody used.
    if origin == CreateOrigin::User
        && let Some(profile_id) = remembered_profile
    {
        remember_default_profile(state, claim.host, &profile_id, &session).await;
    }
    Ok(session)
}

/// Everything one create carries once its host is chosen and its two
/// mutually exclusive selectors have collapsed into a [`CreateMode`].
///
/// A struct rather than nine positional parameters because two of the
/// fields are `Option<String>` and two more are `u16`: a call site that
/// transposed `title` and `intent_key`, or `cols` and `rows`, would compile
/// and be wrong in a way no type could catch.
///
/// The integration overrides (`agent_kind`, `resume_template`) and dimensions
/// are carried even though the agent relay normally passes the defaults for
/// them. Giving the agent path a narrower struct of its own would be a second
/// shape to keep in step with the supervisor's create message, which is the
/// drift this function exists to prevent.
pub(crate) struct CreateSpec {
    pub(crate) cwd: String,
    pub(crate) mode: CreateMode,
    pub(crate) title: Option<String>,
    pub(crate) cols: u16,
    pub(crate) rows: u16,
    pub(crate) intent_key: Option<String>,
    pub(crate) agent_kind: Option<farhelm_proto::AgentKind>,
    pub(crate) resume_template: Option<Vec<String>>,
    /// Identifies whether this create expresses the user's dialog choice or
    /// an agent's request. Only the former may update the helm-wide default:
    /// an agent creating work must not silently move the user's next-dialog
    /// suggestion.
    pub(crate) origin: CreateOrigin,
    /// The helm-resolved fresh-checkout payload for a create whose body
    /// carried `github_checkout`: `Some` reaches the supervisor's
    /// allocation path; `None` is every existing create. Resolved against
    /// the claimed host AFTER routing, one database snapshot.
    pub(crate) github_checkout: Option<farhelm_proto::ResolvedGithubCheckout>,
    /// A veto on the session the target answered with, run before any of
    /// [`do_create_session`]'s bookkeeping. `None` accepts whatever the
    /// target says it created, which is the REST edge's position: it asked
    /// for a session and any session is the answer.
    ///
    /// The one producer is the agent relay's `Clone`, whose result must not
    /// be the ASKING session; see [`do_create_session`]'s "Two phases" note
    /// for why the check cannot simply run at the call site afterwards.
    pub(crate) accept_result: Option<CreatedSessionCheck>,
}

/// Whose successful create may affect the helm-wide profile suggestion.
///
/// The shared creation pipeline serves both browser REST requests and relay
/// requests from agents. They produce the same session side effects except
/// for the remembered default, which belongs to the user rather than an
/// agent that happens to create a profile-backed session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CreateOrigin {
    User,
    Agent,
}

/// [`CreateSpec::accept_result`]'s hook: judge the session a create came
/// back with, and fail the create by returning an error.
///
/// The error is the caller's whole answer — it travels out of
/// [`do_create_session`] unchanged, so it must carry its own
/// [`SupervisorError`] kind if the caller wants anything but `Internal`.
///
/// Boxed and `Send` rather than borrowed, because a [`CreateSpec`] is held
/// across the await that sends the create; a lifetime here would put one on
/// the struct and on every future that carries it.
pub(crate) type CreatedSessionCheck =
    Box<dyn Fn(&farhelm_proto::SessionInfo) -> anyhow::Result<()> + Send>;

/// Which creation mode a caller selected — the choice PLAN_M6_75.md item 3
/// made mutually exclusive on the wire, resolved once before
/// [`do_create_session`] runs.
///
/// Owned rather than borrowed from the body, because the mode outlives the
/// request that produced it: it decides which call to make, and is consulted
/// AGAIN after the reply lands (only a user-originated profile-backed create
/// writes a remembered default), by which point the body's other fields have been
/// moved into the call. Taken out of the body rather than cloned — nothing
/// else reads them afterwards.
///
/// Profile names and ids resolve against the helm catalog before any
/// supervisor call. Ordinary creates fingerprint the resolved bundle, so a
/// profile edit between keyed retries remains a changed request. Fresh
/// checkout callers reconcile the original request identity before reaching
/// mode resolution; their accepted profile snapshot survives later edits.
pub(crate) enum CreateMode {
    Raw(String),
    /// A release-catalog-validated launch composer selection. This stays
    /// distinct from raw mode until the resolved bundle and user intent have
    /// both crossed the supervisor boundary.
    Structured(crate::launches::CompiledLaunch),
    Profile(String),
    /// A profile and identity index produced by one caller-owned catalog
    /// read before target routing.
    ///
    /// Agent defaults and clones need to refuse a dangling id without
    /// contacting the destination. Carrying the catalog snapshot forward
    /// lets creation enrich the reply without reopening the store after the
    /// supervisor mutation.
    ResolvedProfile {
        profile: farhelm_proto::Profile,
        profile_names: ProfileNameIndex,
    },
}

impl CreateMode {
    /// Build a resolved mode from the catalog snapshot that selected it.
    ///
    /// The profile and index must come from the same read. That pairing is
    /// what keeps the bundle sent to the supervisor and the existence verdict
    /// applied to its reply from observing different catalog moments.
    pub(crate) fn resolved_profile(
        profile: farhelm_proto::Profile,
        catalog: &[farhelm_proto::Profile],
    ) -> CreateMode {
        CreateMode::ResolvedProfile {
            profile,
            profile_names: profile_name_index(catalog),
        }
    }
}

/// What [`mode_from_source`] does when a source session's snapshotted
/// profile is no longer in the helm's catalog.
///
/// The two current callers want opposite answers to the exact same
/// question, which is why the choice is a parameter rather than a second
/// copy of the derivation: the agent-CLI clone (`agent_requests::clone_for_agent`)
/// refuses, because a command line written for one machine may not run on
/// another (SPEC.md's agent-verbs section states this explicitly) — while
/// `replace` falls back, because it never changes machine, so the fallback
/// clone refuses is safe there and matches what the UI's own clone already
/// shows the user for the same dangling-profile case.
pub(crate) enum DanglingProfilePolicy {
    /// Refuse outright, naming the vanished profile.
    Refuse,
    /// Silently use the source's raw invocation instead.
    FallBackToRaw,
}

/// Derive a create's [`CreateMode`] from an existing session's row, the way
/// both `clone_for_agent` and `replace` need to: a profiled source follows
/// its snapshot by helm-wide id (so a rename does not move it), and a
/// source with no profile uses its raw invocation. What happens when the
/// snapshot's id is no longer in the catalog is `policy`'s call — see
/// [`DanglingProfilePolicy`] for why the two callers disagree about it.
///
/// `source` must come from a LIVE read of the owning host's session list
/// (`manager::drain_sessions`), never from the helm's cache: a cached row
/// can describe a title, directory, or profile snapshot the session no
/// longer has, and a derived mode built from stale data would carry that
/// staleness into a brand-new session.
pub(crate) async fn mode_from_source(
    state: &AppState,
    source: &farhelm_proto::SessionInfo,
    policy: DanglingProfilePolicy,
) -> anyhow::Result<CreateMode> {
    if let Some(selection) = source.launch.clone() {
        // Clone/replace retain the source's frozen bundle. Recompiling it
        // through today's catalog could change an older selection before a
        // person has reviewed and submitted it again.
        return Ok(CreateMode::Structured(crate::launches::CompiledLaunch {
            invocation: source.invocation.clone(),
            agent_kind: selection.harness.agent_kind(),
            resume_template: source.resume_template.clone(),
            selection,
        }));
    }
    let Some(snapshot) = &source.source_profile else {
        return Ok(CreateMode::Raw(source.invocation.clone()));
    };
    let profiles = state.store.profiles().await?;
    match profiles
        .iter()
        .find(|profile| profile.id == snapshot.id)
        .cloned()
    {
        Some(profile) => Ok(CreateMode::resolved_profile(profile, &profiles)),
        None => match policy {
            DanglingProfilePolicy::Refuse => Err(anyhow::Error::new(SupervisorError {
                kind: ErrorKind::InvalidRequest,
                message: format!(
                    "cannot clone profile {:?}: its snapshotted id {} is no longer in the helm \
                     catalog",
                    snapshot.name, snapshot.id
                ),
            })),
            DanglingProfilePolicy::FallBackToRaw => Ok(CreateMode::Raw(source.invocation.clone())),
        },
    }
}

/// Create and replacement share the same fresh-intent admission protocol.
/// Authority and reply enrichment precede any recovery that could launch a
/// pending request. Original identity/provenance is captured before selector
/// consumption or title normalization. A local resolution failure may race
/// another create, so only a supervisor-settled refusal permits resubmission;
/// a concurrent winner instead receives ordinary acceptance bookkeeping.
///
/// The refusal branch ends before dispatch. Transport errors, rejected
/// returned sessions, history writes and replacement deletion cannot enter it.
async fn create_fresh_session(
    state: &AppState,
    claim: &manager::SessionClaim,
    client: &SupervisorClient,
    mut req: CreateReq,
    intent_key: Option<String>,
    client_identity: String,
    accept_result: Option<CreatedSessionCheck>,
) -> anyhow::Result<farhelm_proto::SessionInfo> {
    let checkout = req
        .github_checkout
        .as_ref()
        .expect("fresh request has checkout intent");
    let preview_incarnation = accepted_checkout_preview(checkout, claim)?.incarnation;
    let acceptance = CreateAcceptance {
        github_repo: Some(farhelm_proto::parse_github_repo(&checkout.repo)?),
        requested_cwd: req.cwd.clone(),
        origin: CreateOrigin::User,
        accept_result,
        // Remote profile provenance cannot select a helm-wide default. Named
        // replay has no trusted retained name-to-id mapping, so leaves it alone.
        remembered_profile: req.profile_id.clone(),
    };
    let profile_names = if req.profile_id.is_some() || req.profile_name.is_some() {
        Some(load_profile_name_index(&state.store).await?)
    } else {
        None
    };
    if let Some(key) = &intent_key
        && let Some(session) = client
            .reconcile_github_checkout(key.clone(), client_identity.clone(), req.cols, req.rows)
            .await?
    {
        return accept_reconciled_fresh(state, claim, session, profile_names.as_ref(), acceptance)
            .await;
    }

    // The complete locally fallible preparation phase sits before dispatch,
    // including profile-ID lookup that ordinary create performs farther down.
    let prepared = async {
        crate::precondition::incarnation_holds(claim, Some(preview_incarnation))?;
        crate::precondition::incarnation_holds(claim, req.expected_incarnation)?;
        let checkout = req.github_checkout.as_ref().expect("fresh intent retained");
        if let (Some(outer), Some(inner)) = (&req.title, &checkout.title)
            && outer != inner
        {
            return Err(anyhow::Error::new(SupervisorError {
                kind: ErrorKind::InvalidRequest,
                message: "fresh checkout and session titles must agree".into(),
            }));
        }
        if req.title.is_none() {
            req.title = checkout.title.clone();
        }
        let github_checkout =
            github_checkout_resolution(state, claim, &req, client_identity.clone())
                .await
                .expect("fresh request requires resolution")?;
        let mode = resolve_create_mode(state, &mut req).await?;
        let mode = if let CreateMode::Profile(id) = mode {
            let profiles = state.store.profiles().await?;
            let profile = profiles
                .iter()
                .find(|profile| profile.id == id)
                .cloned()
                .ok_or_else(|| {
                    anyhow::Error::new(SupervisorError {
                        kind: ErrorKind::NotFound,
                        message: format!("profile not found: {id}"),
                    })
                })?;
            CreateMode::resolved_profile(profile, &profiles)
        } else {
            mode
        };
        Ok((github_checkout, mode))
    }
    .await;
    let (github_checkout, mode) = match prepared {
        Ok(prepared) => prepared,
        Err(local_error) => {
            let Some(key) = intent_key else {
                return Err(local_error.context(crate::FreshCreateUnaccepted));
            };
            return match client.reconcile_github_checkout_with_refusal(
                key, client_identity, req.cols, req.rows, true,
            ).await {
                Ok(Some(session)) => accept_reconciled_fresh(
                    state, claim, session, profile_names.as_ref(), acceptance,
                ).await,
                // Older peers may ignore the flag. Unknown is still only an
                // observation and must never authorize a fresh-key allocation.
                Ok(None) => Err(local_error.context("the supervisor did not establish a durable refusal; retain this request for retry")),
                // Preserve the recorded supervisor outcome, including its
                // kind, rather than replacing it with today's local failure.
                // Only CheckoutConflict carries durable non-acceptance proof.
                Err(error) => Err(error.context(format!("current request validation also failed: {local_error:#}"))),
            };
        }
    };
    do_create_session(
        state,
        claim,
        client,
        CreateSpec {
            cwd: req.cwd,
            mode,
            title: req.title,
            cols: req.cols,
            rows: req.rows,
            intent_key,
            agent_kind: req.agent_kind,
            resume_template: req.resume_template,
            github_checkout,
            origin: CreateOrigin::User,
            accept_result: acceptance.accept_result,
        },
    )
    .await
}

/// Initial lookup and refusal settlement can both return the accepted winner.
/// Enrich from the pre-mutation catalog snapshot, then apply the same source
/// veto and trusted request-derived bookkeeping to either reply.
async fn accept_reconciled_fresh(
    state: &AppState,
    claim: &manager::SessionClaim,
    mut session: farhelm_proto::SessionInfo,
    profile_names: Option<&ProfileNameIndex>,
    acceptance: CreateAcceptance,
) -> anyhow::Result<farhelm_proto::SessionInfo> {
    if let Some(index) = profile_names {
        resolve_session_profiles(index, std::iter::once(&mut session));
    }
    accept_created_session(state, claim, session, acceptance).await
}

/// Resolve an explicit name using the same exact/ambiguity rules as other
/// helm catalog callers. Fresh callers invoke this only after reconciliation
/// found no recorded intent, so later renames cannot invalidate a retry.
async fn resolve_create_mode(state: &AppState, req: &mut CreateReq) -> anyhow::Result<CreateMode> {
    let Some(name) = req.profile_name.as_ref() else {
        return create_mode(req);
    };
    if req.invocation.is_some() || req.profile_id.is_some() || req.launch.is_some() {
        return Err(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message:
                "a create names exactly one of invocation, profile_id, profile_name, or launch"
                    .into(),
        }));
    }
    if req.agent_kind.is_some() || req.resume_template.is_some() {
        return Err(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message: "a profile-backed create cannot also send agent_kind or resume_template"
                .into(),
        }));
    }
    let profiles = state.store.profiles().await?;
    let profile = crate::profiles::resolve_profile_name(&profiles, name)?;
    Ok(CreateMode::resolved_profile(profile, &profiles))
}

/// Resolve an id/raw/structured body after the name-taking boundary.
///
/// Both refusals are `InvalidRequest` — a 400 — and both are worth making
/// loudly rather than picking a winner. A body naming BOTH has no honest
/// reading (does the profile's invocation win, or the caller's?), which is
/// the same reasoning that made the two mutually exclusive on the wire; a
/// body naming NEITHER says nothing about what to run at all. Silently
/// preferring one, or defaulting to some shell, would launch something the
/// caller never asked for.
/// The snapshot overrides are RAW-MODE ONLY, and a profile-mode body
/// carrying either is refused rather than quietly served: a profile already
/// states its kind and its resume template, the wire refuses a request that
/// names both, and this API's shape makes it easy to send both by accident.
/// Discarding them silently — which is what forwarding a profile create and
/// dropping the fields amounts to — would launch a session under settings
/// the caller believes it chose. The refusal names the fields so the caller
/// knows which half to remove.
fn create_mode(req: &mut CreateReq) -> anyhow::Result<CreateMode> {
    match (
        req.invocation.take(),
        req.profile_id.take(),
        req.launch.take(),
    ) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) | (_, Some(_), Some(_)) => {
            Err(anyhow::Error::new(SupervisorError {
                kind: ErrorKind::InvalidRequest,
                message: "a create names exactly one of invocation, profile, or launch: each is a \
                      complete selector and there is no honest way to merge two of them"
                    .to_string(),
            }))
        }
        (None, None, None) => Err(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message: "a create must name an invocation, profile, or launch; this body names \
                      neither, so there is nothing to launch"
                .to_string(),
        })),
        (Some(invocation), None, None) => Ok(CreateMode::Raw(invocation)),
        (None, Some(_), None) if req.agent_kind.is_some() || req.resume_template.is_some() => {
            Err(anyhow::Error::new(SupervisorError {
                kind: ErrorKind::InvalidRequest,
                message: "a profile-backed create cannot also send agent_kind or \
                          resume_template: the profile states both, and the wire refuses a \
                          request that names a profile alongside either override — edit the \
                          profile, or create from a raw invocation instead"
                    .to_string(),
            }))
        }
        (None, Some(profile_id), None) => Ok(CreateMode::Profile(profile_id)),
        (None, None, Some(selection)) => {
            if req.agent_kind.is_some() || req.resume_template.is_some() {
                return Err(anyhow::Error::new(SupervisorError {
                    kind: ErrorKind::InvalidRequest,
                    message: "a structured launch cannot also send agent_kind or resume_template: \
                              the composer owns its resolved bundle"
                        .to_string(),
                }));
            }
            crate::launches::compile(selection)
                .map(CreateMode::Structured)
                .map_err(|message| {
                    anyhow::Error::new(SupervisorError {
                        kind: ErrorKind::InvalidRequest,
                        message,
                    })
                })
        }
    }
}

/// Record a successful user's `profile_id` as the helm-wide last-used profile,
/// and invalidate.
///
/// The remembered id belongs to the helm rather than a host registry row.
/// `host` is diagnostic context for the supervisor-issued creation sequence;
/// it does not order choices, make the default host-owned, or bind it to an
/// installation.
/// [`do_create_session`] calls this only for [`CreateOrigin::User`]: an
/// agent-relay create may seed its session into the cache but must not change
/// the profile the user's next dialog suggests.
///
/// Best effort, on the same terms as [`record_session`]: the session has
/// been created and the caller is about to be told so, and a preference that
/// failed to persist costs one extra click at the next create dialog — where
/// reporting a successful create as a failure would cost a session the user
/// then has to find and clean up by hand.
///
/// Bumps the fleet's revision when the stored id actually CHANGED, which is
/// what makes a create-dialog default arrive in a second client without
/// polling. Creating from the same profile twice in a row changes nothing and
/// wakes nobody.
async fn remember_default_profile(
    state: &AppState,
    host: store::HostId,
    profile_id: &str,
    session: &farhelm_proto::SessionInfo,
) {
    match state
        .store
        .remember_profile_default_from_host_session(
            profile_id,
            host,
            session.creation_seq,
            session.created_at,
            &session.id,
        )
        .await
    {
        Ok(true) => state.manager.events().bump(),
        Ok(false) => {}
        Err(error) => warn!(
            host,
            profile_id = manager::peer_text(profile_id).as_str(),
            error = %error,
            "the session was created but its profile could not be remembered as the helm-wide \
             default; the next create dialog will suggest the previous one"
        ),
    }
}

/// Route to `id`'s owning host and kill its agent's process tree, leaving
/// the session listed and its terminal viewable (SPEC.md's "stop").
///
/// Shared verbatim between [`stop_session`] below and the agent relay's
/// `Stop` verb (`agent_requests::HelmAgentRequests::handle`): both need
/// exactly "route, then ask the owning supervisor to stop it", and nothing
/// else — a stop's reply carries no fresh state to record, unlike rename,
/// which is what keeps this helper simpler than [`do_rename_session`].
pub(crate) async fn do_stop_session(state: &AppState, id: &str) -> anyhow::Result<()> {
    let (_claim, client) = route_session(state, id).await?;
    client.stop_session(id).await
}

/// `POST /api/sessions/{id}/stop` — the recoverable operation the UI does
/// not confirm. The body carries no information beyond success — an empty
/// JSON object, so the response shape stays uniform with `delete_session`
/// below and callers do not need to special-case "no content" bodies. An
/// `id` the merged view does not know is a 404 from [`route_session`]
/// before any host is contacted, and a session whose host is not connected
/// is a 409 naming that state.
pub(crate) async fn stop_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
    match do_stop_session(&state, &id).await {
        Ok(()) => axum::Json(serde_json::json!({})).into_response(),
        Err(e) => http_error(e),
    }
}

/// `GET /api/sessions/{id}` — one session's current state, as a merged-list
/// row (`SessionInfo` fields plus `host`, `host_name`, `stale`).
///
/// Exists for the recovery paths rather than for browsing: after a restart
/// (or after a restart whose reply was lost) a client needs THIS session's
/// current status and offer, and finding it must not depend on where it
/// happens to sit in its host's list.
///
/// ## Read, not operation — which is why it is not refused
///
/// This is the ONE `/api/sessions/{id}` route that a non-connected host
/// does not refuse, and the exception is SPEC.md's own: "opening such a
/// session shows its metadata — title, directory, last-known status —
/// behind a clear host-unreachable notice". Refusing here would leave the
/// UI nothing to put behind that notice. Every route that CHANGES
/// something still refuses (see [`route_session`]).
///
/// The two answers are deliberately different data, from one status read so
/// they cannot disagree:
///
/// - **Connected host: live, and the WHOLE list.** The owner's session list
///   is read in one reply (the same `drain_sessions` the cache refresh
///   uses), which is the only list the wire serves. PLAN_M6.md is explicit that the cache is for the stale
///   list and is not a general serving layer, so a reachable host's detail
///   must never come from it: a detail poll lagging the refresh cadence
///   would show a restart offer that no longer exists.
/// - **Non-connected host: last-known, `stale: true`.** The cached row,
///   which is exactly what the notice is drawn around.
///
/// ## Owner lookup does not depend on the cached row decoding
///
/// The owner is resolved from the cache's COLUMNS (and the manager's
/// in-memory lists), never from the stored metadata — so a row whose
/// `info_json` no longer decodes still routes, and a live session is served
/// from its host regardless of what its cached copy looks like. The
/// undecodable case only costs something for a host that is DOWN, where
/// there is genuinely nothing left to show and 404 is the honest answer.
///
/// Honest limitation, stated because it is not fixed here: the supervisor's
/// protocol has no per-session query, so the live path walks a list. What
/// this route buys is ONE place for every client's recovery lookup to live,
/// so the fix — a `GetSession` message — lands behind it rather than in each
/// caller.
pub(crate) async fn get_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
    // The registry-identity join, same terms as the merged listing's (see
    // `aggregate::row_of`): helm.db is the identity authority, and the
    // snapshot cannot carry a fresh copy. Read BEFORE the owner/status
    // capture below for the same ordering reason the listing documents at
    // its identity join: a retarget straddling the two reads must produce
    // a stale identity on fresh session content (mismatch → the create
    // default falls back locally) rather than a fresh identity on stale
    // content (a false match onto the wrong machine). The whole registry
    // is read because the owner is not known yet — one indexed lookup
    // against a list this small is cheaper than being wrong.
    let identities: Vec<store::HostRow> = match state.store.list_hosts().await {
        Ok(rows) => rows,
        Err(e) => return http_error(e),
    };
    let (host, status) = match resolve_owner(&state, &id).await {
        Ok(owner) => owner,
        Err(e) => return http_error(e),
    };
    let Some(snapshot) = state
        .manager
        .snapshots()
        .into_iter()
        .find(|snapshot| snapshot.id == host)
    else {
        return http_error(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::NotFound,
            message: format!("no such session: {id}"),
        }));
    };
    let host_name = aggregate::host_display_name(
        snapshot.kind,
        snapshot.destination.as_deref(),
        snapshot.alias.as_deref(),
    );
    let host_identity = identities
        .into_iter()
        .find(|row| row.id == host)
        .and_then(|row| row.host_identity);
    // One-id seen-state read, same table and same reason as the listing's
    // own join (`aggregate::session_list_staged`): this route answers with
    // exactly the shape a listing row has, so it must carry the same field.
    let seen_activity_at = match state.store.seen_activity(std::slice::from_ref(&id)).await {
        Ok(map) => map.get(&id).copied(),
        Err(e) => return http_error(e),
    };

    // The client comes from the SAME status read that resolved the owner,
    // so "ask the host" and "say this row is live" cannot disagree.
    let Some(client) = status.client else {
        let cached = match state.store.cached_session(host, &id).await {
            Ok(cached) => cached,
            Err(e) => return http_error(e),
        };
        return match cached {
            Some(mut info) => {
                if let Err(error) = resolve_session_profiles_from_store(
                    &state.store,
                    std::slice::from_mut(&mut info),
                )
                .await
                {
                    return http_error(error);
                }
                match browser_session_ready(&info) {
                    Ok(()) => axum::Json(aggregate::SessionRow {
                        info,
                        host,
                        host_identity,
                        host_name,
                        seen_activity_at,
                        stale: true,
                    })
                    .into_response(),
                    Err(error) => http_error(error),
                }
            }
            // The host is down and its cached copy is unreadable (or gone).
            // There is nothing to put behind the notice, and inventing a
            // placeholder would be worse than saying so.
            None => http_error(anyhow::Error::new(SupervisorError {
                kind: ErrorKind::NotFound,
                message: format!("no such session: {id}"),
            })),
        };
    };
    match manager::drain_sessions(&client).await {
        Ok(mut drained) => {
            if let Err(error) =
                resolve_session_profiles_from_store(&state.store, &mut drained.sessions).await
            {
                return http_error(error);
            }
            match drained.sessions.into_iter().find(|s| s.id == id) {
                Some(info) => match browser_session_ready(&info) {
                    Ok(()) => axum::Json(aggregate::SessionRow {
                        info,
                        host,
                        host_identity,
                        host_name,
                        seen_activity_at,
                        stale: false,
                    })
                    .into_response(),
                    Err(error) => http_error(error),
                },
                // The host is up and says this session is gone: it was deleted
                // between the last cache refresh and now, so 404 is the truth
                // rather than the stale row.
                None => http_error(anyhow::Error::new(SupervisorError {
                    kind: ErrorKind::NotFound,
                    message: format!("no such session: {id}"),
                })),
            }
        }
        Err(e) => http_error(e),
    }
}

/// The body of `POST /api/sessions/{id}/restart`.
///
/// `mode` is required, and deliberately has no default: a restart that
/// guessed a mode could resume a conversation the caller never asked to
/// resume, or launch a fresh agent where the caller expected a resume.
/// The supervisor validates it against the session's CURRENT offer anyway
/// (PLAN_M3.md item 9), so a wrong value is refused rather than obeyed —
/// but an ABSENT one should not be silently turned into a choice at all.
///
/// `stop_if_running` defaults to false, the safe direction: an old-shaped
/// or hand-written body never kills a live agent by omission.
#[derive(Deserialize)]
pub(crate) struct RestartReq {
    mode: farhelm_proto::RestartMode,
    #[serde(default)]
    stop_if_running: bool,
    #[serde(default)]
    with: Option<farhelm_proto::LaunchSelection>,
}

/// `POST /api/sessions/{id}/restart` — relaunch the session's agent
/// (SPEC.md's restart; the resume offered when opening an interrupted
/// session is this same operation, not a separate one).
///
/// The restart fields pass through unchanged, including the refusals that
/// carry this endpoint's real contract: a `mode` that no longer matches the
/// session's offer and a live agent without `stop_if_running` both come back
/// as 409s through `http_error`, and a vanished working directory as a 400
/// naming the directory. Before that call the helm snapshots its profile
/// identity index, so enriching a successful reply is infallible after the
/// agent has been relaunched. The resulting `SessionInfo` is the same shape
/// `POST /api/sessions` answers with, allowing a caller to re-render the row
/// without listing again. Routed by owner like every other lifecycle
/// operation, so a session on a non-connected host is refused with that
/// host's state named rather than reaching a supervisor at all.
pub(crate) async fn restart_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    axum::Json(req): axum::Json<RestartReq>,
) -> impl IntoResponse {
    match do_restart_session(&state, &id, req.mode, req.stop_if_running, req.with).await {
        Ok((_claim, session)) => match browser_session_ready(&session) {
            Ok(()) => axum::Json(session).into_response(),
            Err(error) => http_error(error),
        },
        Err(e) => http_error(e),
    }
}

/// Route, relaunch, enrich, and publish one session restart.
///
/// This is shared by the REST surface and the attached-session relay so a
/// restart has one owner-routing and post-mutation publication contract.
/// `mode` and `stop_if_running` deliberately reach the supervisor unchanged:
/// it alone can revalidate the current offer and liveness immediately before
/// destructive work. The profile index is read before that work, because a
/// catalog read that fails afterward must not make a completed relaunch look
/// unsuccessful to a caller that might otherwise retry it.
pub(crate) async fn do_restart_session(
    state: &AppState,
    id: &str,
    mode: farhelm_proto::RestartMode,
    stop_if_running: bool,
    with: Option<farhelm_proto::LaunchSelection>,
) -> anyhow::Result<(manager::SessionClaim, farhelm_proto::SessionInfo)> {
    let (claim, client) = route_session(state, id).await?;
    let profile_names = load_profile_name_index(&state.store).await?;
    let mut session = client
        .restart_session_with(id, mode, stop_if_running, with)
        .await?;
    resolve_session_profiles(&profile_names, std::iter::once(&mut session));
    record_session(state, &claim, &session).await;
    Ok((claim, session))
}

/// The body of `POST /api/sessions/{id}/rename`: the verb-POST convention
/// `/stop` and `/restart` already use (PLAN_M5.md item 4), rather than a
/// PATCH with a partial `SessionInfo` — there is exactly one field to
/// change, and a verb route says so without inventing a partial-update
/// shape this API has nowhere else.
///
/// `title` has no default and no client-side shape check: an absent field
/// is a 422 from axum's `Json` extractor (a body that parses as JSON but
/// fails to deserialize into this struct — axum 0.8's
/// `JsonRejection::JsonDataError` status, distinct from the 400 a body
/// that is not even valid JSON gets) before this handler ever runs, and
/// every value that DOES parse — including control characters and the
/// empty string — is forwarded as-is (see `rename_session`'s docs for why
/// this handler does not pre-filter what only the supervisor is
/// authoritative over).
#[derive(Deserialize)]
pub(crate) struct RenameReq {
    title: String,
}

/// Route to `id`'s owning host, ask it to change the title, and record the
/// fresh reply — the sequence [`rename_session`] below and the agent relay's
/// `Rename` verb (`agent_requests::HelmAgentRequests::handle`) both need.
/// The profile identity index is loaded before the supervisor call, making
/// reply enrichment infallible after the title changes. `title` still
/// reaches `SupervisorClient::rename_session` VERBATIM, with no trimming or
/// validation on this side (see [`rename_session`]'s own docs for why).
///
/// Returns the [`manager::SessionClaim`] alongside the fresh
/// `SessionInfo` so a caller that must name the OWNING HOST — the agent
/// relay's reply needs it for `AgentSession::host` — is not forced to
/// re-route just to learn what this call already knew; the REST handler
/// below ignores it.
pub(crate) async fn do_rename_session(
    state: &AppState,
    id: &str,
    title: &str,
    expected_title: Option<&str>,
) -> anyhow::Result<(manager::SessionClaim, farhelm_proto::SessionInfo)> {
    let (claim, client) = route_session(state, id).await?;
    // Catalog failure is still safe here: the title has not changed yet.
    let profile_names = load_profile_name_index(&state.store).await?;
    let mut session = client.rename_session(id, title, expected_title).await?;
    resolve_session_profiles(&profile_names, std::iter::once(&mut session));
    record_session(state, &claim, &session).await;
    Ok((claim, session))
}

/// `POST /api/sessions/{id}/rename` — SPEC.md's rename verb (PLAN_M5.md
/// item 4).
///
/// The title is passed through deliberately: the supervisor is the sole
/// authority on what title is acceptable — control characters are refused,
/// and a title over the 64 KiB field cap is refused, but every value that
/// clears both (including an explicit empty title) is accepted. A helm-side
/// title check would only be a second copy of that rule with its own chance
/// to drift. The helm does preload its profile identity index before the
/// mutation, then enriches the successful `SessionInfo` without another
/// fallible read. Refusals retain the ordinary `ErrorKind`→status mapping,
/// and the fresh reply matches `get_session`'s and `restart_session`'s shape
/// so a caller can re-render the row without listing again.
pub(crate) async fn rename_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    axum::Json(req): axum::Json<RenameReq>,
) -> impl IntoResponse {
    match do_rename_session(&state, &id, &req.title, None).await {
        Ok((_claim, session)) => match browser_session_ready(&session) {
            Ok(()) => axum::Json(session).into_response(),
            Err(error) => http_error(error),
        },
        Err(e) => http_error(e),
    }
}

/// `DELETE /api/sessions/{id}` — remove a session and all its stored state
/// (SPEC.md's "delete"). This handler enforces nothing about liveness: it
/// deletes unconditionally, in any state. SPEC.md's confirm-when-alive
/// rule is normatively a CLIENT responsibility — no UI calls this route
/// yet, and when the UI PR adds the delete action, confirming before it
/// sends this request is that PR's job, not something to retrofit here.
/// Same empty-object success body as `stop_session`; an unknown `id` maps
/// to 404.
///
/// A successful delete FORGETS the session from the helm's own records
/// before it answers ([`forget_session`]), so the merged list stops showing
/// it at once rather than at the owning host's next refresh. Without that,
/// a delete followed immediately by a create shows both rows — which is
/// what the browser suite's own shared-session reset does on every test.
///
/// It also clears the session's `session_seen` row (SPEC_impl.md's
/// `session_seen` paragraph).
/// That table carries no foreign key to the session it names — it survives a
/// retarget or an adoption on purpose (`store::SESSION_SEEN_SCHEMA`'s own
/// comment) — so it does not disappear on its own the way the cache row
/// `forget_session` clears does; a genuine delete has to say so explicitly,
/// or the row becomes exactly the harmless garbage that comment already
/// accepts for every OTHER path a session can vanish by. Best-effort like
/// the manager-side forget below: the session is already gone on the host by
/// this point, and refusing the whole delete over a local bookkeeping table
/// would contradict SPEC.md's "changes ... appear in all other connected
/// clients automatically" for the one fact — deletion — that matters most
/// here.
pub(crate) async fn delete_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
    let (claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client.delete_session(&id).await {
        Ok(()) => {
            if let Err(error) = state.store.clear_seen(&id).await {
                warn!(
                    session_id = manager::peer_text(&id).as_str(),
                    error = %error,
                    "could not clear the deleted session's seen state; a stray row may remain"
                );
            }
            forget_session(&state, &claim, &id).await;
            axum::Json(serde_json::json!({})).into_response()
        }
        Err(e) => http_error(e),
    }
}

/// The body of `PUT /api/sessions/{id}/seen`: `Some(stamp)` marks the
/// session seen as of that activity stamp (a manual "mark read", or the
/// automatic mark the session view issues on open and on every activity
/// advance), `None` clears it (a manual "mark unread" — SPEC.md, Status).
/// One field rather than two verbs
/// because the two directions share every other part of the handler:
/// existence check, the changed-or-not bump decision, and the reply shape.
#[derive(Deserialize)]
pub(crate) struct MarkSeenReq {
    #[serde(deserialize_with = "require_present_seen_activity_at")]
    seen_activity_at: Option<i64>,
}

/// Delegates to `Option<i64>`'s ordinary `Deserialize` impl — an explicit
/// JSON `null` still decodes to `None`, an integer to `Some` — but merely
/// attaching a `deserialize_with` is what disables serde derive's syntactic
/// special case that would otherwise default an ABSENT `seen_activity_at`
/// key to `None` too (the same trick `Session::seen_activity_at`'s
/// `double_option` uses the other way, to tell absent apart from an
/// explicit null). Without this, a malformed or truncated PUT body (`{}`)
/// would silently be interpreted as a "mark unread" instead of being
/// rejected — axum 0.8's `JsonRejection::JsonDataError`, a 422, the same
/// status `RenameReq`'s own missing-field doc explains — see the
/// seen-route tests' `{}`-body coverage.
fn require_present_seen_activity_at<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<i64>::deserialize(deserializer)
}

/// `PUT /api/sessions/{id}/seen` — record or clear this session's "last
/// seen" activity stamp (SPEC.md, Status), the state behind the idle dot's
/// grey/blue split and the row's read/unread toggle.
///
/// Deliberately NOT routed through [`route_session`], unlike every
/// lifecycle verb above: marking a session seen is a helm-local write to
/// `store::HelmStore::mark_seen`/`clear_seen`, with no host to ask and
/// nothing for a down host to refuse. A session on an unreachable host is
/// still listed and clearly marked stale (SPEC.md), and its dot still has a
/// colour — so it must still be markable. The existence check is the SAME
/// "no host has ever reported this id" lookup [`get_session`] uses
/// ([`resolve_owner`]) rather than a bespoke one, but only for the 404: the
/// resolved host and connection status are irrelevant to a write that never
/// reaches a supervisor, and are discarded.
///
/// Bumps the fleet-events revision only when the store reports the write
/// actually changed something. Without that check, the same stamp being
/// resent — a manual "mark read" repeated on an already-seen session, the
/// session view's auto-mark effect re-firing after a staleness or
/// capability transition with no new activity behind it
/// (`session_view.rs`'s `mark_key`), or a client retrying a PUT whose
/// response it missed — would each re-bump the revision for nothing,
/// pinging every other connected client to re-read a list that has not
/// moved. The store's `mark_seen`/`clear_seen` own docs carry the SQL half
/// of that guarantee; `sessions_tests.rs`'s seen-route tests pin the
/// bump/no-bump split from this end.
///
/// Same bare `{}` success body as `stop_session`/`delete_session`: nothing
/// about the write needs echoing back, since the events bump this handler
/// issues on a real change is what brings every client's next list read
/// (including the caller's own) up to date — no optimistic client-side
/// state is needed here.
pub(crate) async fn mark_seen(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    axum::Json(req): axum::Json<MarkSeenReq>,
) -> impl IntoResponse {
    if let Err(e) = resolve_owner(&state, &id).await {
        return http_error(e);
    }
    let changed = match req.seen_activity_at {
        Some(activity_at) => state.store.mark_seen(&id, activity_at).await,
        None => state.store.clear_seen(&id).await,
    };
    match changed {
        Ok(changed) => {
            if changed {
                state.manager.events().bump();
            }
            axum::Json(serde_json::json!({})).into_response()
        }
        Err(e) => http_error(e),
    }
}

/// The body of `POST /api/sessions/{id}/replace`: an optional idempotency
/// key, forwarded to the CREATE half of the operation (see
/// [`do_replace_session`]), and an optional override of what that create
/// launches — SPEC.md's "replace with".
///
/// The delete half deliberately gets no key of its own: a browser fires one
/// request per confirmation and never retries a replace on its own, so the
/// one place a retry can land is the create — see `do_replace_session`'s own
/// doc for what a retry after a fully successful replace answers instead.
#[derive(Deserialize)]
pub(crate) struct ReplaceReq {
    intent_key: Option<String>,
    /// "Replace with"'s editable form, reusing [`CreateReq`] verbatim rather
    /// than inventing a parallel override type: a replace-with body IS an
    /// ordinary create body in every field that matters to a create, and a
    /// second type would only be one more place for the two to drift apart.
    /// `None` means today's plain replace — the source's own cwd, title, and
    /// agent, read live and carried forward unchanged (see
    /// [`do_replace_session`]'s doc for that path).
    ///
    /// Present, this field's `cwd`/`title`/`cols`/`rows`/`agent_kind`/
    /// `resume_template`/mode selector (`invocation`/`profile_id`/`launch`)
    /// are resolved exactly as an ordinary `POST /api/sessions` body's are —
    /// including its own mutual-exclusivity and compatibility refusals
    /// (`create_mode`) — and used in place of the source's live row. Two
    /// fields on it mean something DIFFERENT here than on an ordinary
    /// create, because replace-with is deliberately not an ordinary create:
    ///
    /// - `host`: replace never changes machine (SPEC.md's contrast with
    ///   clone, which is the one way to start a session elsewhere). A
    ///   `with.host` naming anything other than the SOURCE's own host (the
    ///   `claim` [`route_session`] resolved for `id`, not a live re-read of
    ///   any registry state) is refused with `Conflict` before anything is
    ///   created; `None` means "the source's host", which is what makes an
    ///   ordinary REST caller's replace-with body able to omit it entirely,
    ///   exactly as a plain create can.
    /// - `intent_key`: this create's real idempotency key is always the
    ///   TOP-LEVEL `intent_key` on this struct, never this field — the wire
    ///   body would otherwise carry two candidate keys for one create, and
    ///   callers that build `with` from the same body-building
    ///   code path an ordinary create uses (see the browser client's
    ///   `create_body`) naturally send the same value in both places. A
    ///   `with.intent_key` that is `Some` and DIFFERS from the top-level key
    ///   is refused with `InvalidRequest` (400): a caller sending two
    ///   different keys for what is supposed to be one intended create is
    ///   describing an ambiguity this route will not silently resolve by
    ///   picking one.
    with: Option<CreateReq>,
}

/// One replace: a fresh session with the source's cwd, title, and agent
/// (SPEC.md's "replace", contrasted there with restart), then the source's
/// removal.
///
/// ## Why create comes before delete, and the two failures are asymmetric
///
/// If the create fails, the source is untouched — nothing was lost, and the
/// error is the create's own (a vanished directory, an unreachable host, a
/// profile that resolves to nothing).
///
/// If the create SUCCEEDS and the delete then fails, the reply names both
/// ids either way, but what it CLAIMS about the source depends on whether
/// the delete's own failure is a definite answer or not. An explicit
/// supervisor refusal, or a delete that never left this process at all
/// (`SupervisorTransportError::NotSent`), is definite: the source was not
/// removed, and the reply says both sessions still exist. A delete that
/// reached the wire and then lost its answer (`SentUnanswered`, or any
/// other post-send ending this client cannot interpret) is NOT definite —
/// the supervisor may have completed it — so the reply says only that the
/// replacement exists and that the source's fate is unknown and must be
/// checked before anyone deletes it again or retries. Neither shape ever
/// rolls the create back (that would kill an agent the user just asked
/// for) or claims success (that would hide a session, or an uncertainty,
/// the user needs to see).
///
/// ## One connection for the whole operation
///
/// The `(claim, client)` pair [`route_session`] resolves for the source id
/// is reused for BOTH the create and the delete — no second owner lookup
/// before the delete. That keeps the operation's two mutations coherent
/// with each other (same connection, not "whichever install answers by the
/// time the delete goes out"), the same coherence-over-freshness argument
/// `route_session`'s own doc makes for a single call, extended across the
/// two calls this route composes.
///
/// ## Idempotency, and why only the create half needs it
///
/// `intent_key` reaches [`do_create_session`] exactly as an ordinary create
/// would, so a retried request cannot double-create. The delete half has no
/// such protection and needs none in practice — a browser fires one request
/// per confirmation and does not retry on its own. A retry sent AFTER a
/// fully successful replace answers 404 for the (now-deleted) source,
/// through the ordinary `route_session` refusal that precedes everything
/// else here; this route makes no attempt to look idempotent past that
/// point, and none is needed.
///
/// ## "Replace with", and what stays true either way
///
/// `with` (see [`ReplaceReq::with`]) overrides WHAT the create half
/// launches — cwd, title, agent, dimensions — never WHERE: the source's own
/// host and connection (`claim` below) are used for both the create and the
/// delete regardless of `with`, and a `with.host` naming a different host is
/// refused before either mutation runs. Everything from the create call
/// onward in this function's doc above — the idempotency-replay veto, the
/// delete, its two failure shapes, `forget_session` — is unchanged by
/// `with`'s presence; only the `CreateSpec` fields feeding that create
/// differ.
///
/// A fresh-checkout override has one additional phase: lookup of the original
/// source-bound request before mutable launch or checkout configuration is
/// resolved. A recorded result uses its durable snapshot; an unknown key must
/// still satisfy the accepted preview's current incarnation and revision.
pub(crate) async fn do_replace_session(
    state: &AppState,
    id: &str,
    intent_key: Option<String>,
    with: Option<CreateReq>,
) -> anyhow::Result<farhelm_proto::SessionInfo> {
    // Ordinary body-shape refusals, checked before anything touches the network —
    // the same precedence an ordinary create's own mutual-exclusivity
    // refusals (`create_mode`) get ahead of routing in `create_session`.
    // Resolving the override's MODE here, and not down where the source's
    // live row is read, is what makes that true for `with`: a body naming
    // both or neither of invocation/profile/launch is a 400 whether or not
    // the source's host is reachable or the source id even exists, exactly
    // as an ordinary create answers, and it never pays for a supervisor
    // round trip first. Two keys naming one intended create is the other
    // shape refused here: an ambiguity this route resolves by refusing
    // rather than silently picking one; see `ReplaceReq::with`'s own doc
    // for why the wire even has two fields that could disagree. Fresh mode
    // compilation waits for lookup below; repository syntax and key ambiguity
    // remain local refusals even for fresh requests.
    let mut with = with;
    let with_mode = match with.as_mut() {
        Some(with) => {
            // The same fresh-checkout resolution the create route applies,
            // for the same reason: "replace with" reuses the create body
            // verbatim, and a github_checkout field on it must be resolved
            // (or refused loudly), never silently dropped. The replacement
            // lands on the SOURCE's host — the claim both halves share.
            if let Some(checkout) = with.github_checkout.as_ref()
                && let Err(error) = farhelm_proto::parse_github_repo(&checkout.repo)
            {
                return Err(anyhow::Error::new(SupervisorError {
                    kind: ErrorKind::InvalidRequest,
                    message: format!(
                        "invalid GitHub repository {:?}: {error}",
                        truncate_repo_text(&checkout.repo)
                    ),
                }));
            }
            if with.github_checkout.is_some() {
                // A recorded fresh intent must reconcile before today's
                // catalog is allowed to compile or refuse its launch.
                None
            } else {
                Some(resolve_create_mode(state, with).await?)
            }
        }
        None => None,
    };
    if let Some(with) = &with
        && let Some(with_key) = &with.intent_key
        && Some(with_key) != intent_key.as_ref()
    {
        return Err(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message: "the replace's own idempotency key and its \"with\" body's key disagree; \
                      send the same key in both places, or omit it from \"with\" entirely"
                .to_string(),
        }));
    }
    let (claim, client) = route_session(state, id).await?;
    // "Replace with" keeps the source's own host — SPEC.md draws this line
    // explicitly, since clone already exists for "start this elsewhere".
    // Checked against the CLAIM (the connection this whole operation is
    // pinned to), not any live-read row, and BEFORE the live read below:
    // this refusal needs nothing from the source's own fields, so there is
    // no reason to pay for that read before a refusal that does not depend
    // on it.
    if let Some(with) = &with
        && let Some(wanted_host) = with.host
        && wanted_host != claim.host
    {
        return Err(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::Conflict,
            message: format!(
                "replace with keeps the source session's own host ({}); it cannot move a \
                 session to another host ({wanted_host}) — clone is the way to start one \
                 elsewhere",
                claim.host
            ),
        }));
    }
    if with
        .as_ref()
        .is_some_and(|req| req.github_checkout.is_some())
    {
        return replace_with_fresh_checkout(
            state,
            &claim,
            &client,
            id,
            intent_key,
            with.expect("fresh body checked above"),
        )
        .await;
    }
    crate::precondition::incarnation_holds(
        &claim,
        with.as_ref().and_then(|with| with.expected_incarnation),
    )?;
    // Read LIVE from the owning host, exactly as `clone_for_agent` does and
    // for the same reason: the helm's cache is for the stale list, not a
    // serving layer, and a replace built from a cached row could copy a
    // title or working directory the session no longer has. This read (and
    // its existence check) runs unconditionally — `with` or not — so a
    // source that has vanished from its own host's list is refused the same
    // way regardless of which form of replace was asked for; only the
    // FIELDS the create half launches with, resolved right below, differ.
    let source = manager::drain_sessions(&client)
        .await?
        .sessions
        .into_iter()
        .find(|session| session.id == id)
        .ok_or_else(|| {
            anyhow::Error::new(SupervisorError {
                kind: ErrorKind::NotFound,
                message: "this session's own host no longer lists it, so there is nothing to \
                          replace"
                    .to_string(),
            })
        })?;
    // The create half's fields: from the override body when "replace with"
    // supplied one (its mode already resolved at the top of this function,
    // ahead of routing), otherwise derived from the source's own live row
    // exactly as plain replace has always done. Building `with`'s mode
    // through the SAME `create_mode` an ordinary create uses is what gives
    // "replace with" every one of a create's own body-shape refusals (naming both or
    // neither of invocation/profile/launch, a profile body also naming
    // agent_kind/resume_template) for free, rather than a second copy of
    // them to keep in sync.
    let (mode, cwd, title, cols, rows, agent_kind, resume_template) = match (with, with_mode) {
        (Some(with), Some(mode)) => (
            mode,
            with.cwd,
            with.title,
            with.cols,
            with.rows,
            with.agent_kind,
            with.resume_template,
        ),
        // The two halves travel together: `with_mode` is `Some` exactly
        // when `with` is, since both come from the same `Option` above.
        (_, _) => {
            let mode =
                mode_from_source(state, &source, DanglingProfilePolicy::FallBackToRaw).await?;
            (
                mode,
                source.cwd,
                // Copied verbatim, empty string included — the same rule
                // `clone_for_agent` follows and for the same reason: deriving
                // a title from the directory instead would silently rename
                // the replacement, the one difference between the two rows a
                // user reading them side by side would notice first.
                Some(source.title),
                default_cols(),
                default_rows(),
                // Raw/profile compatibility overrides still have no durable
                // projection on `SessionInfo`. A structured source is
                // different: `mode_from_source` carries its recorded
                // template inside the structured mode, where
                // `do_create_session` forwards it.
                None,
                None,
            )
        }
    };
    let created = do_create_session(
        state,
        &claim,
        &client,
        CreateSpec {
            cwd,
            mode,
            title,
            cols,
            rows,
            intent_key,
            agent_kind,
            resume_template,
            github_checkout: None,
            origin: CreateOrigin::User,
            // Unlike an ordinary REST create, replace DOES have a session an
            // idempotency replay can collide with: the SOURCE itself. A
            // same-host replace with no field overrides reconstructs the
            // exact fingerprint that created the source in the first place
            // (same cwd, title, profile id or invocation, default
            // dimensions, no parent), so a caller that reuses the source's
            // own creation key hits a legitimate reservation REPLAY at the
            // target, which answers with the SOURCE row rather than a new
            // one. Accepting that reply would go on to send `DeleteSession`
            // for the very id just "created", forget it, and report success
            // with a `SessionInfo` describing the session it just deleted —
            // violating every promise replace makes (new id, fresh
            // conversation, a replacement left alive). See
            // `clone_for_agent`'s identical veto, which this mirrors for the
            // identical reason.
            accept_result: Some(replacement_result_check(id)),
        },
    )
    .await?;
    finish_replacement(state, &claim, &client, id, created).await
}

/// A replacement must leave a different session alive. Run this check through
/// shared create acceptance, before cache/history/default writes as well as
/// before deletion; a replay of the source is not a successful replacement.
fn replacement_result_check(id: &str) -> CreatedSessionCheck {
    let source_id = id.to_string();
    Box::new(move |created| {
        if created.id != source_id {
            return Ok(());
        }
        Err(SupervisorError {
            kind: ErrorKind::Conflict,
            message: "the idempotency key replayed the create that made the source \
                      session, so no replacement was made; retry the replace with a \
                      key that has not been used on this host, or with none at all"
                .into(),
        }
        .into())
    })
}

/// Reconcile fresh replacements against their original request and source.
/// Current incarnation/configuration/catalog checks apply only to unknown
/// intents. The accepted installation is checked even for known keys, and a
/// live source is still required, preserving replace's existing 404 contract
/// after a fully completed operation. Neither lookup nor creation may bypass
/// the source-id veto or change the connection used by the delete half.
async fn replace_with_fresh_checkout(
    state: &AppState,
    claim: &manager::SessionClaim,
    client: &SupervisorClient,
    id: &str,
    intent_key: Option<String>,
    req: CreateReq,
) -> anyhow::Result<farhelm_proto::SessionInfo> {
    let client_identity = serde_json::to_string(&(
        "github_replace_request_v1",
        id,
        fresh_create_request_identity(&req),
    ))
    .expect("replacement identity contains only serializable fields");
    accepted_checkout_preview(
        req.github_checkout
            .as_ref()
            .expect("fresh replacement has checkout intent"),
        claim,
    )?;
    // Lookup may resume an accepted pending create, so establish the source's
    // continued existence first. This read does not resolve launch settings.
    if !manager::drain_sessions(client)
        .await?
        .sessions
        .iter()
        .any(|session| session.id == id)
    {
        return Err(SupervisorError {
            kind: ErrorKind::NotFound,
            message: "this session's own host no longer lists it, so there is nothing to replace"
                .into(),
        }
        .into());
    }
    let created = create_fresh_session(
        state,
        claim,
        client,
        req,
        intent_key,
        client_identity,
        Some(replacement_result_check(id)),
    )
    .await?;
    finish_replacement(state, claim, client, id, created).await
}

/// Delete the source only after a replacement has passed acceptance. Both a
/// newly created session and a reconciled one use this tail so lost delete
/// replies retain the same definite-versus-ambiguous failure reporting.
async fn finish_replacement(
    state: &AppState,
    claim: &manager::SessionClaim,
    client: &SupervisorClient,
    id: &str,
    created: farhelm_proto::SessionInfo,
) -> anyhow::Result<farhelm_proto::SessionInfo> {
    if let Err(delete_error) = client.delete_session(id).await {
        // Two shapes of failure here, and they earn different words because
        // they answer a different question: did the delete happen?
        //
        // An EXPLICIT refusal — the supervisor received `DeleteSession` and
        // answered with its own `ControlMsg::Error` (`SupervisorError`), or
        // the request never reached the wire at all
        // (`SupervisorTransportError::NotSent`) — is a DEFINITE answer: the
        // original was not removed, full stop, and "both sessions still
        // exist" is simply true.
        //
        // Anything else this client can produce from a delete — the
        // connection dying after the frame was enqueued
        // (`SentUnanswered`), or an unexpected reply this client's own
        // wrapper does not recognize — is NOT a definite answer: the
        // supervisor may have completed the deletion and only the
        // confirmation was lost. Reporting "both sessions still exist" in
        // that case would be inventing a fact this side cannot have; the
        // honest reply says the replacement is real and that the source's
        // fate must be CHECKED rather than assumed either way.
        let refusal_is_definite = crate::find_cause::<SupervisorError>(&delete_error).is_some()
            || matches!(
                crate::find_cause::<crate::SupervisorTransportError>(&delete_error),
                Some(crate::SupervisorTransportError::NotSent)
            );
        let message = if refusal_is_definite {
            format!(
                "created replacement session {} but could not remove the original {id}: \
                 {delete_error:#}; both sessions still exist",
                created.id
            )
        } else {
            format!(
                "created replacement session {} but whether the original {id} was removed is \
                 unknown after {delete_error:#}; the replacement exists — check {id} before \
                 deleting it or retrying",
                created.id
            )
        };
        return Err(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::Internal,
            message,
        }));
    }
    forget_session(state, claim, id).await;
    Ok(created)
}

/// `POST /api/sessions/{id}/replace` — recreate `id` under a brand-new id on
/// the same host, then remove `id` (SPEC.md's "replace"). Absent `with`,
/// the replacement carries the same directory, title, and agent as `id`; a
/// present `with` overrides any of those instead (SPEC.md's "replace with").
/// See [`do_replace_session`] for the operation and its failure rule.
pub(crate) async fn replace_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    axum::Json(req): axum::Json<ReplaceReq>,
) -> impl IntoResponse {
    match do_replace_session(&state, &id, req.intent_key, req.with).await {
        Ok(session) => match browser_session_ready(&session) {
            Ok(()) => axum::Json(session).into_response(),
            Err(error) => http_error(error),
        },
        Err(e) => http_error(e),
    }
}

/// `POST /api/sessions/{id}/tabs` — open a terminal tab: a plain shell in
/// the session's working directory (PLAN_M4.md item 2, plumbed through by
/// item 5). No request body: unlike `create_session`, a tab has nothing
/// for a caller to specify.
///
/// The success body is `{"tab": TabInfo}` rather than the bare object
/// `stop`/`delete` use, because there is something to hand back — the
/// minted tab id a client needs before it can attach
/// (`?tab=<id>` on `term_ws`). Every refusal the supervisor can give
/// (vanished working directory, no tmux session to open a window on, a
/// shell dead by reply time) reaches the browser through the same
/// `http_error` mapping every other endpoint uses, verbatim.
pub(crate) async fn open_tab(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
    let (_claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client.open_tab(&id).await {
        Ok(tab) => axum::Json(serde_json::json!({ "tab": tab })).into_response(),
        Err(e) => http_error(e),
    }
}

/// `DELETE /api/sessions/{id}/tabs/{tab_id}` — close a terminal tab: kill
/// its shell and everything it left behind, then drop the window
/// (PLAN_M4.md item 2). Same empty-object success body as `stop_session`/
/// `delete_session`; an unknown `tab_id` maps to 404 like any other
/// unknown identifier, and a tab whose shell had already exited still
/// closes successfully — `close_tab`'s own idempotency, passed straight
/// through.
pub(crate) async fn close_tab(
    State(state): State<Arc<AppState>>,
    AxPath((id, tab_id)): AxPath<(String, String)>,
) -> impl IntoResponse {
    let (_claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client.close_tab(&id, &tab_id).await {
        Ok(()) => axum::Json(serde_json::json!({})).into_response(),
        Err(e) => http_error(e),
    }
}

// Tests occupy three quarters of this module, so they live in a sibling file. `#[path]`
// keeps them under `sessions` with private-item access and no visibility changes.
#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
