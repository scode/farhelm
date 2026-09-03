//! Agent profile APIs: the helm-owned catalog and the still-proxied host
//! routes.
//!
//! ## The helm owns the catalog
//!
//! The helm now stores one catalog shared by every host and every client.
//! `GET` and CRUD under `/api/profiles` read and mutate that catalog, while
//! `/api/hosts/{id}/profiles` remains a proxy to the owning supervisor for
//! this part of the migration. The remembered default is one raw id per helm,
//! including a dangling id after deletion, so the client can ask instead of
//! guessing. The next change makes session creates resolve against this helm
//! catalog rather than the supervisor's catalog.
//!
//! ## The split during this migration
//!
//! The new top-level routes use the helm-owned catalog. The host-scoped
//! routes remain a pass-through to the owning supervisor for this part of the
//! migration, so existing clients can continue to use them while session
//! creation is moved in the next change. The helm-wide remembered default is
//! separate from both catalogs and is not sent over the supervisor wire.
//! Selectorless creates therefore still send that raw id to the target
//! supervisor, where it may be absent or name that host's different profile
//! definition until the next migration part moves resolution to the helm.
//!
//! ## The default is one bare profile id
//!
//! The remembered default is one plain profile id in helm.db. It is not
//! reconciled against either catalog on read: a deleted profile remains a
//! useful signal to the client that it must ask instead of guessing.
//!
//! ## One read, both halves
//!
//! [`list_profiles`] answers with the catalog AND the remembered default id
//! in one shape, and that pairing is the point rather than a convenience.
//! SPEC.md's creation rule is that the dialog defaults to the last-used
//! profile and ASKS when that profile is gone — which is a question about
//! two facts at once. Served separately, a client would have to reconcile a
//! catalog and a default read at different moments, and the moment that
//! matters is exactly the one where a profile was just deleted. The
//! remembered id is served RAW, never filtered against the catalog beside
//! it: a default naming a profile that no longer exists is precisely the
//! state the ask-don't-guess fallback exists for, and quietly dropping it
//! would turn "your last profile is gone, pick another" into a silent
//! nothing.
//!
//! ## Every mutation that changed something invalidates
//!
//! Profiles are one of the surfaces the goal promises arrives without
//! polling: an edit in one client must reach another client's open profile
//! surface and its create dialog. Each mutation that actually changed the
//! catalog therefore bumps the fleet's revision
//! (`crate::manager::FleetEvents`). A plain read does not. An edit that
//! submits exactly what is already stored is NOT recognized here: it
//! forwards, commits on the supervisor, and wakes the fleet like any other
//! accepted edit — last-write-wins with no helm-side comparison (see
//! [`update_profile`]).
//!
//! A FAILED mutation does not bump either, and that rule needs its boundary
//! stated rather than assumed: a failure this side observes is not proof
//! that nothing happened on the far side. A transport that dies after the
//! supervisor committed an edit reaches this module as an error, and no
//! invalidation is published for a change that is nonetheless real. What
//! covers it is the connection itself: losing the connection is a host state
//! transition, which the manager publishes (`HostActor::publish_refresh`),
//! so every client re-reads anyway — and re-reads again when the host comes
//! back. The bump this module skips is one the connection's own state change
//! has already made redundant.
//!
//! A CANCELLED request — an axum handler's future is dropped the moment its
//! client disconnects — can likewise abandon a mutation whose frame is
//! already with the supervisor, so the supervisor commits and nothing bumps.
//! That is accepted, and it is worth being exact about what it costs, because
//! catalogs have no refresh loop of their own: a client re-reads a catalog
//! when it opens or re-points the surface, when a feed notice arrives, after
//! its own mutations, and on the fallback poll ONLY while the feed is down.
//! So a connected client with the profiles section already open sees a
//! cancelled save's effect only when something else bumps the fleet revision
//! or the user re-opens the section — not within seconds, and not by any
//! periodic tick. A detached task that carried the reply and the invalidation
//! past the handler's lifetime used to close that window, and was removed as
//! more machinery than the case (one user, one browser, a save cut off
//! mid-request) is worth.

use crate::sessions::host_client;
use crate::store::HostId;
use crate::{AppState, http_error};
use axum::extract::{Path as AxPath, State};
use axum::response::IntoResponse;
use farhelm_proto::{AgentKind, Profile};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The common response shape for host-scoped and helm-owned catalog reads.
///
/// See the module docs for why the two travel in one shape. The field names
/// are frozen by PLAN_M6_75.md item 6's consumer.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ProfilesView {
    /// The requested catalog in stable id order: supervisor-owned for a host
    /// route and helm-owned for a top-level route.
    pub(crate) profiles: Vec<Profile>,
    /// The helm-wide id last used by a profile-backed session, or `None`.
    ///
    /// May name a profile ABSENT from `profiles` above — a deleted one — and
    /// that combination is meaningful rather than a bug: it is what a client
    /// keys SPEC.md's ask-don't-guess fallback off.
    ///
    /// The two halves are NOT read atomically and make no claim to be: the
    /// catalog comes from the supervisor over the wire, the default from
    /// helm.db, and no snapshot spans both. Nothing needs one, because the
    /// client's response to any mismatch is the same single behavior — ask
    /// which profile to use instead of guessing — and that response is
    /// correct whether the profile was deleted an hour ago or between these
    /// two reads.
    pub(crate) default_profile: Option<String>,
}

/// The body of a profile create or update — everything but the id.
///
/// A host-scoped create asks that host's supervisor to mint the id; a
/// top-level create asks the helm store. A client has no id to know in
/// advance, and letting it propose one would invite collisions. On update,
/// the URL is the sole resource authority.
#[derive(Deserialize)]
pub(crate) struct ProfileSpec {
    name: String,
    invocation: String,
    /// Which integrated agent this profile IS. Required, unlike a create's
    /// `agent_kind` override: `Generic` is the explicit spelling of "no
    /// kind", and an absent field would be a second way to say the same
    /// thing about a value that decides whether capture and status
    /// sharpening run at all.
    agent_kind: AgentKind,
    /// The resume invocation as an argv vector, or absent. See
    /// `farhelm_proto::Profile::resume_template` for what absence means per
    /// kind — it is not uniformly "no resume".
    resume_template: Option<Vec<String>>,
}

/// Render a catalog field refusal as the same 400-shaped error used by the
/// supervisor profile API.
fn catalog_validation_error(message: String) -> axum::response::Response {
    http_error(anyhow::Error::new(crate::SupervisorError {
        kind: farhelm_proto::ErrorKind::InvalidRequest,
        message,
    }))
}

/// `GET /api/hosts/{id}/profiles` — one host's profile catalog plus this
/// helm-wide remembered default.
///
/// A live read from the owning supervisor, never a cache: profiles are
/// small, hand-curated, and read when a user opens a picker, so there is
/// nothing a cache would buy that is worth a second copy of the catalog
/// going stale. The consequence is stated rather than hidden — a host that
/// is not connected cannot answer, and this refuses with the host's state
/// named, exactly as a session operation on it would.
///
/// The catalog and the default are read in separate awaits and are not
/// atomic — one comes over the wire and the other from helm.db. Nothing here
/// checks that the host stayed on one connection across the two reads: the
/// default is a helm-wide bare id (see the module docs), so pairing it
/// with whatever catalog the row reaches now is the intended reading.
pub(crate) async fn list_profiles(
    State(state): State<Arc<AppState>>,
    AxPath(host): AxPath<HostId>,
) -> impl IntoResponse {
    let (_, client) = match host_client(&state, host) {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    let profiles = match client.list_profiles().await {
        Ok(profiles) => profiles,
        Err(e) => return http_error(e),
    };
    // Read after the catalog rather than before it. Neither order makes the
    // pair atomic — see `ProfilesView::default_profile` — but this one skews
    // the possible mismatch toward the case the client already handles: a
    // default whose profile has just been deleted (ask which to use
    // instead), rather than a default written moments ago for a profile the
    // catalog read predates.
    let default_profile = match state.store.remembered_profile().await {
        Ok(default_profile) => default_profile,
        Err(e) => return http_error(e),
    };
    axum::Json(ProfilesView {
        profiles,
        default_profile,
    })
    .into_response()
}

/// `POST /api/hosts/{id}/profiles` — define a new profile on that host.
///
/// Every field travels to the supervisor verbatim, and every refusal comes
/// back from it: the name's control-character rule, the per-field cap, the
/// catalog bound, and the `{conversation}` placeholder rule for an
/// integrated kind's resume template. None of them is re-implemented here —
/// a second copy would be a second thing to drift, and the supervisor is the
/// only side that can check the catalog bound anyway.
///
/// The one mutation that does NOT take the per-host serialization lock, and
/// the exception is reasoned rather than an oversight: a create mints an id
/// nobody holds yet, so no queued edit or delete can be aimed at it and
/// there is no order for the per-host queue to preserve. Queueing creates
/// behind edits would buy nothing and would make a slow edit block an
/// unrelated create.
///
/// Answers with the profile as the supervisor committed it — the one carried
/// by `ProfileCreated`, not the one submitted — so a supervisor that
/// normalized anything is described rather than contradicted.
pub(crate) async fn create_profile(
    State(state): State<Arc<AppState>>,
    AxPath(host): AxPath<HostId>,
    axum::Json(spec): axum::Json<ProfileSpec>,
) -> impl IntoResponse {
    let (_, client) = match host_client(&state, host) {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client
        .create_profile(
            &spec.name,
            &spec.invocation,
            spec.agent_kind,
            spec.resume_template,
        )
        .await
    {
        Ok(profile) => {
            state.manager.events().bump();
            axum::Json(profile).into_response()
        }
        Err(e) => http_error(e),
    }
}

/// `POST /api/hosts/{id}/profiles/{profile_id}` — replace a profile's
/// definition wholesale.
///
/// A POST to the resource rather than a PUT or a PATCH, matching the rest of
/// this API's verb vocabulary (`/stop`, `/rename`, `/destination`): there is
/// no partial-update shape anywhere in it, and this is emphatically not one
/// — the whole definition is replaced, because per-field optionality would
/// make "clear the resume template" and "leave it alone" the same request.
///
/// The path's `profile_id` WINS over any id in the body, and the body's is
/// ignored rather than compared. The URL is what the caller aimed at, and a
/// mismatch between the two is a client bug whose only safe resolution is to
/// edit the profile the request was addressed to.
///
/// Nothing here touches sessions already created from this profile: their
/// launch and resume snapshots are their own (SPEC.md's snapshot rule), and
/// a rename simply starts showing up as `Renamed` on their source-profile
/// reference. The merged list's profile FILTER is what keeps them findable
/// through the change — it matches the snapshotted name as well as the id
/// (see `crate::store::SessionFilter`).
///
/// An edit applies to whichever install the row reaches when it is routed.
/// There is no precondition naming the connection the editor was opened on:
/// on any realistic timescale that install is where the catalog on screen
/// came from, and an edit landing after a retarget just applies to the
/// install that is there now.
///
/// ## Last write wins, and every accepted edit is forwarded
///
/// Two clients editing one profile resolve by last-write-wins (SPEC.md,
/// Concepts / Agent profile): there is no optimistic-concurrency check, no
/// fingerprint to echo, and no helm-side comparison against what is stored.
/// An edit that submits exactly what the catalog already holds forwards
/// like any other — the supervisor commits it and the fleet is woken for it.
/// A helm-side suppression used to recognize that case, and it went with the
/// last-write-wins simplification: keeping it truthful required a catalog
/// pre-read on every edit plus a serialization burden on the queue below,
/// which outweighed the no-op wakes it saved.
///
/// ## One host's mutations are a queue
///
/// Edits and deletes on one host share a per-host lock
/// (`AppState::profile_edits`), so they reach the supervisor in the order
/// this helm accepted them. What that buys under last-write-wins is modest
/// but real: "the later write wins" is only a meaningful sentence if there
/// IS an order, and a delete must not overtake an edit whose success a
/// client has already been shown (see [`delete_profile`]).
///
/// ## Routing is checked twice
///
/// The host is routed BEFORE the lock is taken, because the lock map is keyed
/// by a caller-supplied path id and must only ever hold real hosts
/// (`AppState::profile_edit_lock`). It is routed AGAIN under the lock, because
/// a request can queue behind another for as long as that one takes and the
/// host may be forgotten, retargeted, or dropped in the meantime — the client
/// resolved before the wait would then be a connection to somewhere the id no
/// longer names.
pub(crate) async fn update_profile(
    State(state): State<Arc<AppState>>,
    AxPath((host, profile_id)): AxPath<(HostId, String)>,
    axum::Json(spec): axum::Json<ProfileSpec>,
) -> impl IntoResponse {
    // Routed before the lock is allocated, and again after it is held: see
    // this function's docs for why neither check covers the other.
    if let Err(e) = host_client(&state, host) {
        return http_error(e);
    }
    let _editing = state.enter_profile_edit(host).await;
    let (_, client) = match host_client(&state, host) {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    let profile = Profile {
        id: profile_id,
        name: spec.name,
        invocation: spec.invocation,
        agent_kind: spec.agent_kind,
        resume_template: spec.resume_template,
    };
    match client.update_profile(profile).await {
        Ok(profile) => {
            state.manager.events().bump();
            axum::Json(profile).into_response()
        }
        Err(e) => http_error(e),
    }
}

/// `DELETE /api/hosts/{id}/profiles/{profile_id}` — remove a profile from
/// that host's catalog.
///
/// The empty-object body matches every other verb that reports only success
/// (`stop`, `delete`), so callers need no special case for a bodyless reply.
///
/// The helm's remembered default is deliberately NOT cleared when it names
/// the profile being deleted. That looks like tidying and would actually
/// destroy information: the remembered id outliving its profile is exactly
/// what lets the next create dialog say "the profile you last used is gone,
/// pick another" instead of silently offering nothing (SPEC.md's
/// ask-don't-guess). It is replaced by the next successful profile-backed
/// create, and it costs one row until then.
///
/// ## Why a delete queues behind edits
///
/// It takes the same per-host lock [`update_profile`] does: one host's
/// catalog mutations reach the supervisor in the order this helm accepted
/// them, so a delete cannot overtake an edit whose success a client has
/// already been shown. (The lock's original, stronger reason — keeping the
/// identical-edit suppression's read-compare-forward span truthful — went
/// with the suppression itself.)
///
/// The routing discipline is [`update_profile`]'s, for [`update_profile`]'s
/// reasons: routed before the lock so a made-up path id cannot mint one, and
/// routed again under it.
pub(crate) async fn delete_profile(
    State(state): State<Arc<AppState>>,
    AxPath((host, profile_id)): AxPath<(HostId, String)>,
) -> impl IntoResponse {
    if let Err(e) = host_client(&state, host) {
        return http_error(e);
    }
    let _editing = state.enter_profile_edit(host).await;
    let (_, client) = match host_client(&state, host) {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client.delete_profile(&profile_id).await {
        Ok(()) => {
            state.manager.events().bump();
            axum::Json(serde_json::json!({})).into_response()
        }
        Err(e) => http_error(e),
    }
}

/// `GET /api/profiles` — read the helm-owned catalog and its raw remembered id.
///
/// The shared response deliberately exposes a dangling default: deletion
/// changes future choices, not the historical suggestion clients need to
/// recognize and ask the user to replace.
pub(crate) async fn list_catalog_profiles(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let profiles = match state.store.profiles().await {
        Ok(profiles) => profiles,
        Err(error) => return http_error(error),
    };
    let default_profile = match state.store.remembered_profile().await {
        Ok(default_profile) => default_profile,
        Err(error) => return http_error(error),
    };
    axum::Json(ProfilesView {
        profiles,
        default_profile,
    })
    .into_response()
}

/// `POST /api/profiles` — create a profile in the helm-wide catalog.
///
/// SQLite serializes catalog writes and enforces the bound in the same
/// transaction as insertion, so an application mutex would only impose an
/// unneeded request-arrival order. The detached task couples a successful
/// durable mutation to its revision bump even after axum drops this request.
pub(crate) async fn create_catalog_profile(
    State(state): State<Arc<AppState>>,
    axum::Json(spec): axum::Json<ProfileSpec>,
) -> impl IntoResponse {
    if let Err(message) = farhelm_proto::validate_profile_fields(
        &spec.name,
        &spec.invocation,
        spec.agent_kind,
        spec.resume_template.as_deref(),
    ) {
        return catalog_validation_error(message);
    }
    let task_state = Arc::clone(&state);
    let mutation = tokio::spawn(async move {
        let outcome = task_state
            .store
            .create_profile(
                spec.name,
                spec.invocation,
                spec.agent_kind,
                spec.resume_template,
            )
            .await?;
        if matches!(outcome, crate::store::ProfileCreation::Created(_)) {
            task_state.manager.events().bump();
        }
        Ok::<_, anyhow::Error>(outcome)
    });
    match mutation.await {
        Err(error) => http_error(anyhow::Error::new(error).context("catalog create task panicked")),
        Ok(Err(error)) => http_error(error),
        Ok(Ok(crate::store::ProfileCreation::Created(profile))) => {
            (axum::http::StatusCode::CREATED, axum::Json(profile)).into_response()
        }
        Ok(Ok(crate::store::ProfileCreation::CatalogFull)) => {
            http_error(anyhow::Error::new(crate::SupervisorError {
                kind: farhelm_proto::ErrorKind::InvalidRequest,
                message: format!(
                    "this helm already holds the maximum of {} profiles; delete one before creating another",
                    farhelm_proto::MAX_PROFILES
                ),
            }))
        }
    }
}

/// `POST /api/profiles/{id}` — replace a helm-owned profile wholesale.
///
/// The URL is the resource authority; the id-free body cannot accidentally
/// redirect an update. Concurrent accepted updates are last-write-wins in
/// SQLite commit order. As with create, a detached task makes the revision
/// bump cancellation-safe and refusals leave that revision unchanged.
pub(crate) async fn update_catalog_profile(
    State(state): State<Arc<AppState>>,
    AxPath(profile_id): AxPath<String>,
    axum::Json(spec): axum::Json<ProfileSpec>,
) -> impl IntoResponse {
    if let Err(message) = farhelm_proto::validate_profile_fields(
        &spec.name,
        &spec.invocation,
        spec.agent_kind,
        spec.resume_template.as_deref(),
    ) {
        return catalog_validation_error(message);
    }
    let profile = Profile {
        id: profile_id,
        name: spec.name,
        invocation: spec.invocation,
        agent_kind: spec.agent_kind,
        resume_template: spec.resume_template,
    };
    let task_state = Arc::clone(&state);
    let mutation = tokio::spawn(async move {
        let outcome = task_state.store.update_profile(profile).await?;
        if outcome.is_some() {
            task_state.manager.events().bump();
        }
        Ok::<_, anyhow::Error>(outcome)
    });
    match mutation.await {
        Err(error) => http_error(anyhow::Error::new(error).context("catalog update task panicked")),
        Ok(Err(error)) => http_error(error),
        Ok(Ok(Some(profile))) => axum::Json(profile).into_response(),
        Ok(Ok(None)) => http_error(anyhow::Error::new(crate::SupervisorError {
            kind: farhelm_proto::ErrorKind::NotFound,
            message: "profile not found".to_string(),
        })),
    }
}

/// `DELETE /api/profiles/{id}` — remove a helm-owned profile.
///
/// Deletion intentionally leaves a dangling remembered id, which tells a
/// later picker to ask rather than silently substitute another profile. Its
/// detached mutation task follows the same commit-and-invalidate contract as
/// create and update.
pub(crate) async fn delete_catalog_profile(
    State(state): State<Arc<AppState>>,
    AxPath(profile_id): AxPath<String>,
) -> impl IntoResponse {
    let task_state = Arc::clone(&state);
    let mutation = tokio::spawn(async move {
        let deleted = task_state.store.delete_profile(&profile_id).await?;
        if deleted {
            task_state.manager.events().bump();
        }
        Ok::<_, anyhow::Error>(deleted)
    });
    match mutation.await {
        Err(error) => http_error(anyhow::Error::new(error).context("catalog delete task panicked")),
        Ok(Err(error)) => http_error(error),
        Ok(Ok(true)) => axum::Json(serde_json::json!({})).into_response(),
        Ok(Ok(false)) => http_error(anyhow::Error::new(crate::SupervisorError {
            kind: farhelm_proto::ErrorKind::NotFound,
            message: "profile not found".to_string(),
        })),
    }
}

#[cfg(test)]
mod tests {
    use crate::rest_harness;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{AgentKind, ControlMsg, Frame, Profile};
    use std::time::Duration;
    use tokio::io::{DuplexStream, ReadHalf, WriteHalf};
    use tower::ServiceExt;

    /// One scripted supervisor's two halves, already past the handshake.
    ///
    /// A type alias purely to keep the helpers below readable: every test
    /// here scripts an exchange, and the reader/writer pair is the same in
    /// all of them.
    type Peer = (
        FrameReader<ReadHalf<DuplexStream>>,
        FrameWriter<WriteHalf<DuplexStream>>,
    );

    /// Complete the handshake on a test peer and hand back its halves.
    ///
    /// The spliced harness answers `ListSessions` on this peer's behalf
    /// (see `rest_harness`'s module docs), so everything that arrives here
    /// is what the test's own request produced — which is exactly what lets
    /// these tests assert on the NEXT frame rather than filtering the
    /// manager's housekeeping out of a stream.
    async fn peer_up(peer_side: DuplexStream) -> Peer {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        (reader, writer)
    }

    /// Consume EXACTLY the next frame this peer is sent, as a control
    /// message.
    ///
    /// One frame per call, in order, with nothing skipped: these tests
    /// assert on WHICH request the helm made and in what sequence — an
    /// idempotent edit's whole contract is that a `ListProfiles` arrives and
    /// an `UpdateProfile` does not — so a helper that filtered or looked
    /// ahead would hide the very thing under test. The spliced harness
    /// answers `ListSessions` on this peer's behalf, so the manager's
    /// housekeeping never reaches this stream and there is nothing to skip.
    ///
    /// Panics on EOF or on an unparseable frame, deliberately: every caller
    /// is asserting that a specific request arrived, so "the connection
    /// closed instead" IS the failure and unwrapping reports it at the line
    /// that expected it.
    async fn asked(reader: &mut FrameReader<ReadHalf<DuplexStream>>) -> ControlMsg {
        parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap()
    }

    /// The state a [`catalog_peer`] and the test driving it share.
    ///
    /// One struct rather than five `Arc`s threaded through a call, because
    /// every concurrency test below needs all of it and the interesting
    /// assertions are about the RELATIONSHIP between the fields — which
    /// requests arrived, in what order, and what the catalog holds as a
    /// result.
    struct CatalogPeer {
        /// Awaited before the FIRST reply and never again: how a test holds
        /// one request open inside the helm's serialized span while it starts
        /// another.
        gate: tokio::sync::Notify,
        /// Fired once the first request has reached the far side, so a test
        /// knows the helm is inside that span rather than assuming it.
        arrived: tokio::sync::Notify,
        /// Every request this peer served, in arrival order, as a bare verb.
        ///
        /// The ORDER is the assertion in the delete-versus-edit test: a
        /// serialized helm cannot have forwarded a delete before the edit
        /// ahead of it decided, and a count alone could not tell the two
        /// interleavings apart.
        seen: std::sync::Mutex<Vec<&'static str>>,
        /// The catalog itself, mutated by the writes this peer applies.
        held: std::sync::Mutex<Vec<Profile>>,
    }

    impl CatalogPeer {
        fn new(initial: Vec<Profile>) -> std::sync::Arc<CatalogPeer> {
            std::sync::Arc::new(CatalogPeer {
                gate: tokio::sync::Notify::new(),
                arrived: tokio::sync::Notify::new(),
                seen: std::sync::Mutex::new(Vec::new()),
                held: std::sync::Mutex::new(initial),
            })
        }

        /// How many `UpdateProfile` frames arrived — the count the
        /// serialization tests assert on.
        fn forwards(&self) -> usize {
            self.seen
                .lock()
                .expect("request log mutex")
                .iter()
                .filter(|verb| **verb == "update")
                .count()
        }

        fn requests(&self) -> Vec<&'static str> {
            self.seen.lock().expect("request log mutex").clone()
        }

        fn catalog(&self) -> Vec<Profile> {
            self.held.lock().expect("catalog mutex").clone()
        }
    }

    /// A supervisor that actually HOLDS a catalog: it answers `ListProfiles`
    /// with what it currently has, and applies the writes it is sent.
    ///
    /// Every other peer in this module scripts a fixed exchange, which is the
    /// right shape for asserting which request the helm made. It is the wrong
    /// shape for the concurrency tests, whose whole subject is what a SECOND
    /// reader sees after a first writer landed — a scripted reply cannot
    /// change between two reads, so a scripted peer would answer the stale
    /// catalog to both and prove nothing.
    async fn catalog_peer(peer_side: DuplexStream, shared: std::sync::Arc<CatalogPeer>) {
        let (mut reader, mut writer) = peer_up(peer_side).await;
        let mut gated = false;
        while let Ok(Some(frame)) = reader.read_frame().await {
            let Ok(message) = parse_control(&frame) else {
                continue;
            };
            if !gated {
                shared.arrived.notify_one();
                shared.gate.notified().await;
                gated = true;
            }
            match message {
                ControlMsg::ListProfiles { req_id } => {
                    shared.seen.lock().expect("request log mutex").push("list");
                    let profiles = shared.catalog();
                    writer
                        .write_frame(&Frame::control(&ControlMsg::ProfileList {
                            req_id,
                            profiles,
                        }))
                        .await
                        .unwrap();
                }
                ControlMsg::UpdateProfile { req_id, profile } => {
                    shared
                        .seen
                        .lock()
                        .expect("request log mutex")
                        .push("update");
                    {
                        let mut catalog = shared.held.lock().expect("catalog mutex");
                        catalog.retain(|held| held.id != profile.id);
                        catalog.push(profile.clone());
                    }
                    writer
                        .write_frame(&Frame::control(&ControlMsg::ProfileUpdated {
                            req_id,
                            profile,
                        }))
                        .await
                        .unwrap();
                }
                ControlMsg::CreateProfile {
                    req_id,
                    name,
                    invocation,
                    agent_kind,
                    resume_template,
                } => {
                    shared
                        .seen
                        .lock()
                        .expect("request log mutex")
                        .push("create");
                    // The minted id is the supervisor's to choose, and a test
                    // that could predict it would not be exercising the reason
                    // a create's reply has to carry one back.
                    let profile = Profile {
                        id: format!("p-minted-{}", shared.catalog().len() + 1),
                        name,
                        invocation,
                        agent_kind,
                        resume_template,
                    };
                    shared
                        .held
                        .lock()
                        .expect("catalog mutex")
                        .push(profile.clone());
                    writer
                        .write_frame(&Frame::control(&ControlMsg::ProfileCreated {
                            req_id,
                            profile,
                        }))
                        .await
                        .unwrap();
                }
                ControlMsg::DeleteProfile { req_id, profile_id } => {
                    shared
                        .seen
                        .lock()
                        .expect("request log mutex")
                        .push("delete");
                    shared
                        .held
                        .lock()
                        .expect("catalog mutex")
                        .retain(|held| held.id != profile_id);
                    writer
                        .write_frame(&Frame::control(&ControlMsg::ProfileDeleted { req_id }))
                        .await
                        .unwrap();
                }
                other => panic!("this peer only serves the profile catalog; got {other:?}"),
            }
        }
    }

    /// Block until `waiting` requests are queued on a profile-mutation lock,
    /// or fail the test.
    ///
    /// The completion-driven replacement for a sleep, and the difference is
    /// not stylistic. "Wait 50ms and assume the second request got as far as
    /// the lock" passes identically against a helm that serializes NOTHING —
    /// the sleep proves only that time passed. This waits for the queue itself
    /// (`AppState::profile_edit_queue`), so a helm that never queued the
    /// second request fails here rather than sailing on to assert the
    /// serialized outcome it reached by luck.
    async fn await_queued(state: &crate::AppState, waiting: usize) {
        let queued = tokio::time::timeout(Duration::from_secs(10), async {
            while state.queued_profile_edits() != waiting {
                tokio::task::yield_now().await;
            }
        })
        .await;
        assert!(
            queued.is_ok(),
            "{waiting} request(s) should be queued on the host's profile lock; \
             {} are — an unserialized mutation never queues at all",
            state.queued_profile_edits()
        );
    }

    /// The body of an edit, as the two concurrency tests submit it.
    fn edit_body(name: &str) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "invocation": "claude",
            "agent_kind": "claude",
            "resume_template": ["claude", "--resume", "{conversation}"],
        })
    }

    /// A profile with the fields a round trip is asserted on.
    fn profile(id: &str, name: &str) -> Profile {
        Profile {
            id: id.to_string(),
            name: name.to_string(),
            invocation: "claude".to_string(),
            agent_kind: AgentKind::Claude,
            resume_template: Some(vec![
                "claude".into(),
                "--resume".into(),
                "{conversation}".into(),
            ]),
        }
    }

    /// Issue one request against the real router and return its status and
    /// body.
    ///
    /// `method`/`body` rather than separate helpers because these tests
    /// exercise all three verbs against the same two paths, and the
    /// difference between them is the only thing worth seeing at the call
    /// site.
    async fn request(
        harness: &rest_harness::Harness,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (axum::http::StatusCode, serde_json::Value) {
        let builder = axum::http::Request::builder()
            .method(method)
            .uri(uri)
            .header("host", "127.0.0.1:7433");
        let request = match &body {
            Some(json) => builder
                .header("content-type", "application/json")
                .body(axum::body::Body::from(json.to_string()))
                .unwrap(),
            None => builder.body(axum::body::Body::empty()).unwrap(),
        };
        let response = harness.router().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, value)
    }

    /// The helm-owned routes expose the seeded catalog, validate writes, and
    /// keep the catalog bound observable at the HTTP boundary. This test
    /// matters because store-only coverage cannot catch a wrong route, status
    /// code, request shape, or fleet-revision invalidation.
    #[tokio::test]
    async fn helm_profile_catalog_routes_cover_crud_errors_and_bound() {
        let harness = rest_harness::idle_helm().await;
        let before = harness.manager.events().revision();

        let (status, value) = request(&harness, "GET", "/api/profiles", None).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value["profiles"].as_array().unwrap().len(), 4);
        assert_eq!(value["default_profile"], serde_json::Value::Null);

        let (status, value) = request(
            &harness,
            "POST",
            "/api/profiles",
            Some(serde_json::json!({
                "name": "wrapper",
                "invocation": "wrapper --agent",
                "agent_kind": "generic",
                "resume_template": null,
            })),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::CREATED);
        let id = value["id"].as_str().unwrap().to_string();
        assert!(harness.manager.events().revision() > before);
        let after_create = harness.manager.events().revision();

        let (status, value) = request(
            &harness,
            "POST",
            &format!("/api/profiles/{id}"),
            Some(serde_json::json!({
                "name": "renamed",
                "invocation": "wrapper --renamed",
                "agent_kind": "generic",
                "resume_template": null,
            })),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value["id"], id);
        assert_eq!(value["name"], "renamed");
        assert!(harness.manager.events().revision() > after_create);
        let after_update = harness.manager.events().revision();

        let (status, value) = request(
            &harness,
            "POST",
            &format!("/api/profiles/{id}"),
            Some(serde_json::json!({
                "name": " ",
                "invocation": "wrapper",
                "agent_kind": "generic",
                "resume_template": null,
            })),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
        assert!(value.as_str().unwrap().contains("must not be empty"));
        assert_eq!(harness.manager.events().revision(), after_update);

        let (status, value) = request(
            &harness,
            "POST",
            "/api/profiles/unknown",
            Some(serde_json::json!({
                "name": "missing",
                "invocation": "wrapper",
                "agent_kind": "generic",
                "resume_template": null,
            })),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
        assert!(value.as_str().unwrap().contains("profile not found"));
        assert_eq!(harness.manager.events().revision(), after_update);

        let (status, value) =
            request(&harness, "DELETE", &format!("/api/profiles/{id}"), None).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value, serde_json::json!({}));
        assert!(harness.manager.events().revision() > after_update);
        let after_delete = harness.manager.events().revision();

        let (status, value) = request(&harness, "DELETE", "/api/profiles/unknown", None).await;
        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
        assert!(value.as_str().unwrap().contains("profile not found"));
        assert_eq!(harness.manager.events().revision(), after_delete);

        let starting_len = harness.store.profiles().await.unwrap().len();
        for _ in starting_len..farhelm_proto::MAX_PROFILES {
            assert!(matches!(
                harness
                    .store
                    .create_profile(
                        "wrapper".to_string(),
                        "wrapper".to_string(),
                        AgentKind::Generic,
                        None,
                    )
                    .await
                    .unwrap(),
                crate::store::ProfileCreation::Created(_)
            ));
        }
        let (status, value) = request(
            &harness,
            "POST",
            "/api/profiles",
            Some(serde_json::json!({
                "name": "too-many",
                "invocation": "wrapper",
                "agent_kind": "generic",
                "resume_template": null,
            })),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
        assert!(value.as_str().unwrap().contains("maximum"));
        assert_eq!(harness.manager.events().revision(), after_delete);
        assert_eq!(
            harness.store.profiles().await.unwrap().len(),
            farhelm_proto::MAX_PROFILES
        );

        harness
            .store
            .remember_profile_default("deleted-profile")
            .await
            .unwrap();
        let (status, value) = request(&harness, "GET", "/api/profiles", None).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value["default_profile"], "deleted-profile");
        assert_eq!(
            value["profiles"].as_array().unwrap().len(),
            farhelm_proto::MAX_PROFILES
        );
    }

    /// [`request`]'s POST half against an OWNED router, for the tests that
    /// drive two requests concurrently.
    ///
    /// Separate because `request` borrows the harness, and a borrowed harness
    /// cannot be moved into two spawned tasks. A router is a cheap clone of
    /// the same shared state, so both tasks drive the real serving path.
    async fn post(
        router: axum::Router,
        uri: &str,
        body: serde_json::Value,
    ) -> (axum::http::StatusCode, serde_json::Value) {
        let request = axum::http::Request::builder()
            .method("POST")
            .uri(uri)
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap();
        let response = router.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, value)
    }

    /// The read a picker makes: the owning host's catalog and this helm's
    /// remembered default, in ONE reply.
    ///
    /// Spec: `GET /api/hosts/{id}/profiles` proxies `ListProfiles` to that
    /// host and answers `{profiles, default_profile}`, with the catalog's
    /// own order preserved and a `null` default for a host nothing has ever
    /// been created on. The single shape is what SPEC.md's ask-don't-guess
    /// fallback needs — see this module's docs on why the two facts must be
    /// reconcilable against each other.
    #[tokio::test]
    async fn the_catalog_and_the_remembered_default_arrive_in_one_reply() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (mut reader, mut writer) = peer_up(peer_side).await;
            let ControlMsg::ListProfiles { req_id } = asked(&mut reader).await else {
                panic!("the helm must proxy a catalog read as ListProfiles");
            };
            writer
                .write_frame(&Frame::control(&ControlMsg::ProfileList {
                    req_id,
                    profiles: vec![profile("p-1", "Claude Code"), profile("p-2", "Codex")],
                }))
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        let (status, value) = request(
            &harness,
            "GET",
            &format!("/api/hosts/{local}/profiles"),
            None,
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            value["profiles"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["p-1", "p-2"],
            "the supervisor's order is served as-is"
        );
        assert_eq!(value["profiles"][0]["name"], "Claude Code");
        assert_eq!(value["profiles"][0]["agent_kind"], "claude");
        assert_eq!(
            value["default_profile"],
            serde_json::Value::Null,
            "a host nothing has been created on has no remembered default"
        );
        peer.await.unwrap();
    }

    /// A create round-trips every field and invalidates.
    ///
    /// Two properties in one exchange because they are one code path: the
    /// helm is a pass-through for the fields (a dropped one would be
    /// invisible in this crate otherwise — nothing else reads them), and a
    /// successful mutation must reach OTHER clients without polling, which
    /// is what the revision assertion pins.
    #[tokio::test]
    async fn creating_a_profile_forwards_every_field_and_invalidates() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (mut reader, mut writer) = peer_up(peer_side).await;
            let ControlMsg::CreateProfile {
                req_id,
                name,
                invocation,
                agent_kind,
                resume_template,
            } = asked(&mut reader).await
            else {
                panic!("the helm must proxy a create as CreateProfile");
            };
            assert_eq!(name, "Nightly");
            assert_eq!(invocation, "codex --sandbox");
            assert_eq!(agent_kind, AgentKind::Codex);
            assert_eq!(
                resume_template,
                Some(vec!["codex".to_string(), "{conversation}".to_string()])
            );
            writer
                .write_frame(&Frame::control(&ControlMsg::ProfileCreated {
                    req_id,
                    // The minted id is the one thing the caller could not
                    // have known, so the reply carries it back.
                    profile: profile("p-minted", &name),
                }))
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        let before = harness.manager.events().revision();

        let (status, value) = request(
            &harness,
            "POST",
            &format!("/api/hosts/{local}/profiles"),
            Some(serde_json::json!({
                "name": "Nightly",
                "invocation": "codex --sandbox",
                "agent_kind": "codex",
                "resume_template": ["codex", "{conversation}"],
            })),
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value["id"], "p-minted");
        assert!(
            harness.manager.events().revision() > before,
            "a profile create must invalidate every other client's profile surface"
        );
        peer.await.unwrap();
    }

    /// An edit is keyed by the PATH's profile id, and the whole definition
    /// is replaced.
    ///
    /// The path-wins rule matters because a body carrying a different id is
    /// a client bug with two possible readings, and only one of them is
    /// safe: editing the profile the request was addressed to. The
    /// replacement half is pinned by sending a body with NO resume template
    /// and asserting the supervisor is asked to store none — under a patch
    /// reading, "clear it" and "leave it alone" would be the same request.
    #[tokio::test]
    async fn an_edit_is_keyed_by_the_path_id_and_replaces_the_whole_definition() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (mut reader, mut writer) = peer_up(peer_side).await;
            // The FIRST thing the peer sees is the forwarded edit itself:
            // the helm makes no catalog pre-read on this path (there is no
            // identical-edit suppression to inform), so anything arriving
            // ahead of the UpdateProfile is a regression.
            let ControlMsg::UpdateProfile { req_id, profile } = asked(&mut reader).await else {
                panic!("the helm must proxy an edit as UpdateProfile, with no prior read");
            };
            assert_eq!(
                profile.id, "p-path",
                "the URL is what the caller aimed at; the body's id is ignored"
            );
            assert_eq!(profile.name, "Renamed");
            assert_eq!(
                profile.resume_template, None,
                "an absent template is a cleared template, not an unchanged one"
            );
            writer
                .write_frame(&Frame::control(&ControlMsg::ProfileUpdated {
                    req_id,
                    profile,
                }))
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        let before = harness.manager.events().revision();

        let (status, value) = request(
            &harness,
            "POST",
            &format!("/api/hosts/{local}/profiles/p-path"),
            Some(serde_json::json!({
                // Deliberately contradicts the path, to pin which one wins.
                "id": "p-body",
                "name": "Renamed",
                "invocation": "claude",
                "agent_kind": "generic",
            })),
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value["id"], "p-path");
        assert!(harness.manager.events().revision() > before);
        peer.await.unwrap();
    }

    /// A delete proxies, answers the uniform empty-object body, and
    /// invalidates.
    #[tokio::test]
    async fn deleting_a_profile_proxies_and_invalidates() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (mut reader, mut writer) = peer_up(peer_side).await;
            let ControlMsg::DeleteProfile { req_id, profile_id } = asked(&mut reader).await else {
                panic!("the helm must proxy a delete as DeleteProfile");
            };
            assert_eq!(profile_id, "p-doomed");
            writer
                .write_frame(&Frame::control(&ControlMsg::ProfileDeleted { req_id }))
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        let before = harness.manager.events().revision();

        let (status, value) = request(
            &harness,
            "DELETE",
            &format!("/api/hosts/{local}/profiles/p-doomed"),
            None,
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value, serde_json::json!({}));
        assert!(harness.manager.events().revision() > before);
        peer.await.unwrap();
    }

    /// A supervisor's refusal reaches the caller with its status and its
    /// words, rather than becoming a generic failure.
    ///
    /// The helm re-implements none of the profile validation (see
    /// `create_profile`'s docs), which is only safe if the supervisor's
    /// answer survives the proxy intact — otherwise a user editing a
    /// profile would be told "something went wrong" about a rule they could
    /// have satisfied.
    #[tokio::test]
    async fn a_supervisors_profile_refusal_reaches_the_caller_verbatim() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (mut reader, mut writer) = peer_up(peer_side).await;
            let ControlMsg::CreateProfile { req_id, .. } = asked(&mut reader).await else {
                panic!("expected CreateProfile");
            };
            writer
                .write_frame(&Frame::control(&ControlMsg::Error {
                    req_id,
                    kind: farhelm_proto::ErrorKind::InvalidRequest,
                    message: "a claude profile's resume template must contain {conversation}"
                        .to_string(),
                }))
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        let before = harness.manager.events().revision();

        let (status, body) = request(
            &harness,
            "POST",
            &format!("/api/hosts/{local}/profiles"),
            Some(serde_json::json!({
                "name": "Broken",
                "invocation": "claude",
                "agent_kind": "claude",
                "resume_template": ["claude", "--resume"],
            })),
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
        assert!(
            body.as_str().unwrap_or_default().contains("{conversation}"),
            "the supervisor's own words must survive the proxy: {body:?}"
        );
        assert_eq!(
            harness.manager.events().revision(),
            before,
            "a refused mutation changed nothing and must wake nobody"
        );
        peer.await.unwrap();
    }

    /// A profile-backed create makes a profile the helm-wide remembered
    /// default, which both catalog surfaces then return raw.
    ///
    /// This is the whole helm-owned half of profiles (PLAN_M6_75.md item 5)
    /// in one exchange: the create names a profile and no invocation (the
    /// wire's exclusive mode), the helm records the choice in helm.db, and
    /// the next catalog read carries it. SPEC.md's "creation defaults to
    /// last-used" has nowhere else to come from — the supervisor neither
    /// knows nor should.
    ///
    /// The invalidation is ISOLATED, which takes a deliberate fixture: a
    /// create ordinarily bumps twice over (the session it recorded, then the
    /// default), so "the revision moved" would prove nothing about the
    /// default at all. So the session this create returns is byte-identical
    /// to one the host already listed — its cache write is then a genuine
    /// no-op that publishes nothing — leaving the remembered default as the
    /// only thing in the request that can move the revision, and by exactly
    /// one.
    #[tokio::test]
    async fn a_profile_backed_create_becomes_the_helm_wide_remembered_default() {
        // The session the create will return, ALREADY in the host's list, so
        // recording it changes nothing (see this test's docs).
        let existing = farhelm_proto::SessionInfo {
            cwd: "/work".to_string(),
            source_profile: Some(farhelm_proto::SourceProfile {
                id: "p-favorite".to_string(),
                name: "Claude Code".to_string(),
                existence: farhelm_proto::ProfileExistence::Present,
            }),
            ..rest_harness::session("sess-new", 1_700_000_500)
        };
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn({
            let existing = existing.clone();
            async move {
                let (mut reader, mut writer) = peer_up(peer_side).await;
                let ControlMsg::CreateSession {
                    req_id,
                    parent: None,
                    profile_name: None,
                    invocation,
                    profile_id,
                    cwd,
                    ..
                } = asked(&mut reader).await
                else {
                    panic!("expected CreateSession");
                };
                assert_eq!(cwd, "/work");
                assert_eq!(
                    invocation, None,
                    "profile mode names no invocation — a request naming both is refused"
                );
                assert_eq!(profile_id, Some("p-favorite".to_string()));
                writer
                    .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                        req_id,
                        session: existing,
                    }))
                    .await
                    .unwrap();
                // The catalog read that follows, so the test can assert the
                // remembered default arrives beside real profiles.
                let ControlMsg::ListProfiles { req_id } = asked(&mut reader).await else {
                    panic!("expected ListProfiles");
                };
                writer
                    .write_frame(&Frame::control(&ControlMsg::ProfileList {
                        req_id,
                        profiles: vec![profile("p-favorite", "Claude Code")],
                    }))
                    .await
                    .unwrap();
            }
        });

        let harness = rest_harness::spliced_helm_listing(client_side, vec![existing]).await;
        let local = rest_harness::local_id(&harness.store).await;
        let before = harness.manager.events().revision();

        let (status, _) = request(
            &harness,
            "POST",
            "/api/sessions",
            Some(serde_json::json!({"cwd": "/work", "profile_id": "p-favorite"})),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);

        let (status, value) = request(
            &harness,
            "GET",
            &format!("/api/hosts/{local}/profiles"),
            None,
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            value["default_profile"], "p-favorite",
            "a successful profile-backed create is what 'last used' means"
        );
        assert_eq!(
            harness.manager.events().revision(),
            before,
            "the preceding drain already converged this profile-backed session into the \
             remembered default, so replaying the same create changes neither cache nor default"
        );
        peer.await.unwrap();
    }

    /// The two creation modes are exclusive at the REST edge too, and a
    /// body that gets it wrong reaches no supervisor at all.
    ///
    /// Refused HERE rather than forwarded because the refusal is about the
    /// request's shape, and a helm that passed an ambiguous create along
    /// would turn a client bug into a round trip whose failure mode depends
    /// on which supervisor answered it. `silent_supervisor` is what proves
    /// nothing was forwarded.
    #[tokio::test]
    async fn a_create_naming_both_modes_or_neither_is_refused_locally() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(rest_harness::silent_supervisor(peer_side));
        let harness = rest_harness::spliced_helm(client_side).await;

        for (body, expected) in [
            (
                serde_json::json!({"cwd": "/work", "invocation": "claude", "profile_id": "p-1"}),
                "never both",
            ),
            (serde_json::json!({"cwd": "/work"}), "names neither"),
        ] {
            let (status, text) = request(&harness, "POST", "/api/sessions", Some(body)).await;
            assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
            assert!(
                text.as_str().unwrap_or_default().contains(expected),
                "the refusal must say which shape was wrong: {text:?}"
            );
        }
        peer.await.unwrap();
    }

    /// A profile-mode create carrying a snapshot override is REFUSED, not
    /// quietly stripped.
    ///
    /// The overrides are raw-mode only — a profile already states its kind
    /// and its resume template, and the wire refuses a request naming both —
    /// so forwarding a profile create while dropping the fields would launch
    /// a session under settings the caller believes it chose. Both fields
    /// are staged, because either alone is enough to make the request
    /// ambiguous, and neither reaches a supervisor.
    #[tokio::test]
    async fn a_profile_create_carrying_a_snapshot_override_is_refused_locally() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(rest_harness::silent_supervisor(peer_side));
        let harness = rest_harness::spliced_helm(client_side).await;

        for body in [
            serde_json::json!({
                "cwd": "/work",
                "profile_id": "p-1",
                "agent_kind": "codex",
            }),
            serde_json::json!({
                "cwd": "/work",
                "profile_id": "p-1",
                "resume_template": ["claude", "--resume", "{conversation}"],
            }),
        ] {
            let (status, text) = request(&harness, "POST", "/api/sessions", Some(body)).await;
            assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
            let text = text.as_str().unwrap_or_default().to_string();
            assert!(
                text.contains("agent_kind") && text.contains("resume_template"),
                "the refusal must name the fields to remove: {text:?}"
            );
        }
        peer.await.unwrap();
    }

    /// An edit that submits exactly what is stored is FORWARDED and
    /// invalidates, like any other accepted edit.
    ///
    /// This pins the removal of the helm-side identical-edit suppression:
    /// under last-write-wins there is no helm-side comparison deciding
    /// whether an edit "changed anything", so a no-op submission commits on
    /// the supervisor and wakes the fleet, and the redundant wake is the
    /// accepted cost. A reintroduced suppression — with its catalog pre-read
    /// and its truthfulness burden on the edit queue — would fail here by
    /// forwarding nothing.
    #[tokio::test]
    async fn an_identical_profile_edit_is_forwarded_like_any_other() {
        let stored = profile("p-1", "Claude Code");
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn({
            let stored = stored.clone();
            async move {
                let (mut reader, mut writer) = peer_up(peer_side).await;
                // The FIRST request is the forwarded edit itself: no catalog
                // pre-read precedes it any more.
                let ControlMsg::UpdateProfile { req_id, profile } = asked(&mut reader).await else {
                    panic!("an edit forwards as UpdateProfile, with no prior catalog read");
                };
                assert_eq!(profile, stored, "the submission travels verbatim");
                writer
                    .write_frame(&Frame::control(&ControlMsg::ProfileUpdated {
                        req_id,
                        profile,
                    }))
                    .await
                    .unwrap();
            }
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        let before = harness.manager.events().revision();

        let (status, value) = request(
            &harness,
            "POST",
            &format!("/api/hosts/{local}/profiles/p-1"),
            Some(serde_json::json!({
                "name": "Claude Code",
                "invocation": "claude",
                "agent_kind": "claude",
                "resume_template": ["claude", "--resume", "{conversation}"],
            })),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value["id"], "p-1");
        assert!(
            harness.manager.events().revision() > before,
            "an accepted edit invalidates even when it changed nothing"
        );
        peer.await.unwrap();
    }

    /// Two DIFFERING concurrent edits both land, and the one that QUEUED
    /// SECOND is what the catalog ends up holding.
    ///
    /// Spec: with Beta deterministically queued behind Alpha, both reach the
    /// supervisor and the durable definition is Beta.
    ///
    /// The other half of the serialization contract, and the half that says
    /// what it is NOT: serializing the read-compare-forward span makes the
    /// suppression honest, it does not turn edits into a compare-and-swap. Two
    /// clients that genuinely disagree still resolve by last-write-wins,
    /// exactly as this API has always said they do — and this pins that the
    /// queue did not quietly start rejecting the second writer.
    ///
    /// The winner is asserted by NAME rather than as "one of the two". A test
    /// that accepts either outcome cannot fail on a queue that runs backwards,
    /// which is precisely the bug last-write-wins would have; the queue order
    /// is forced with [`await_queued`], so there is a right answer here.
    #[tokio::test]
    async fn two_differing_concurrent_edits_both_land_and_the_later_one_wins() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let shared = CatalogPeer::new(vec![profile("p-1", "Before")]);
        let peer = tokio::spawn(catalog_peer(peer_side, std::sync::Arc::clone(&shared)));

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        let uri = format!("/api/hosts/{local}/profiles/p-1");

        let first = tokio::spawn({
            let router = harness.router();
            let uri = uri.clone();
            async move { post(router, &uri, edit_body("Alpha")).await }
        });
        shared.arrived.notified().await;
        let second = tokio::spawn({
            let router = harness.router();
            let uri = uri.clone();
            async move { post(router, &uri, edit_body("Beta")).await }
        });
        // Beta is behind Alpha in the queue, established rather than hoped
        // for — which is what makes "the later one" a name and not a coin.
        await_queued(&harness.state, 1).await;
        shared.gate.notify_one();

        let (first_status, first_body) = first.await.unwrap();
        let (second_status, second_body) = second.await.unwrap();
        assert_eq!(first_status, axum::http::StatusCode::OK);
        assert_eq!(second_status, axum::http::StatusCode::OK);
        assert_eq!(
            shared.forwards(),
            2,
            "two clients that disagree both get to write"
        );

        // Each client is answered with its OWN edit — a queue that handed one
        // writer the other's result would be reporting a write that never
        // happened.
        assert_eq!(first_body["name"], "Alpha");
        assert_eq!(second_body["name"], "Beta");
        assert_eq!(
            shared.catalog()[0].name,
            "Beta",
            "the durable definition is the one the queue let through last"
        );
        drop(harness);
        let _ = peer.await;
    }

    /// A DELETE queues behind an in-flight edit on the same host.
    ///
    /// Spec: a delete issued while an edit is still waiting on the
    /// supervisor queues on the per-host lock and reaches the supervisor
    /// only after that edit has been answered.
    ///
    /// The ORDER is the point. Under last-write-wins the queue is the lock's
    /// whole remaining job: a client that was just shown its edit's success
    /// must not have that edit overtaken by a delete this helm accepted
    /// after it — the edit would then appear to have resurrected a profile
    /// the user watched themselves remove. This is the test that notices if
    /// the lock quietly stops serializing.
    #[tokio::test]
    async fn a_delete_queues_behind_an_in_flight_edit() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let shared = CatalogPeer::new(vec![profile("p-1", "Claude Code")]);
        let peer = tokio::spawn(catalog_peer(peer_side, std::sync::Arc::clone(&shared)));

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        let uri = format!("/api/hosts/{local}/profiles/p-1");

        // An edit the peer holds open, so the delete demonstrably queues.
        let edit = tokio::spawn({
            let router = harness.router();
            let uri = uri.clone();
            async move { post(router, &uri, edit_body("Renamed")).await }
        });
        shared.arrived.notified().await;

        let delete = tokio::spawn({
            let router = harness.router();
            let uri = uri.clone();
            async move {
                let request = axum::http::Request::builder()
                    .method("DELETE")
                    .uri(&uri)
                    .header("host", "127.0.0.1:7433")
                    .body(axum::body::Body::empty())
                    .unwrap();
                router.oneshot(request).await.unwrap().status()
            }
        });
        await_queued(&harness.state, 1).await;

        shared.gate.notify_one();
        let (edit_status, edit_body) = edit.await.unwrap();
        let delete_status = delete.await.unwrap();
        assert_eq!(edit_status, axum::http::StatusCode::OK);
        assert_eq!(edit_body["name"], "Renamed");
        assert_eq!(delete_status, axum::http::StatusCode::OK);

        assert_eq!(
            shared.requests(),
            vec!["update", "delete"],
            "the delete reached the supervisor only after the edit ahead of it was answered"
        );
        assert!(
            shared.catalog().is_empty(),
            "and the delete is what the catalog ends up reflecting"
        );
        drop(harness);
        let _ = peer.await;
    }

    /// A made-up host id mints no per-host lock.
    ///
    /// Spec: mutations against ids the registry does not hold are refused
    /// without allocating anything, and a real host's mutation allocates
    /// exactly one entry.
    ///
    /// The lock map is documented as bounded by how many hosts a person has
    /// registered, and that bound is only real if the id is validated BEFORE
    /// the entry is created. Taking the lock first — the obvious ordering,
    /// since the lock is what makes the routing decision stable — turns a
    /// path segment into a permanent allocation, so an unauthenticated
    /// loopback caller grows this process one `i64` at a time for as long as
    /// it likes.
    #[tokio::test]
    async fn a_made_up_host_id_mints_no_profile_edit_lock() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let shared = CatalogPeer::new(vec![profile("p-1", "Before")]);
        let peer = tokio::spawn(catalog_peer(peer_side, std::sync::Arc::clone(&shared)));
        shared.gate.notify_one();

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        let locks = || {
            harness
                .state
                .profile_edits
                .lock()
                .expect("profile edit lock map poisoned")
                .len()
        };
        assert_eq!(locks(), 0);

        for id in [local + 1_000, local + 2_000, i64::MAX, i64::MIN, -1] {
            let (status, _) = request(
                &harness,
                "POST",
                &format!("/api/hosts/{id}/profiles/p-1"),
                Some(edit_body("Renamed")),
            )
            .await;
            assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
            let (status, _) = request(
                &harness,
                "DELETE",
                &format!("/api/hosts/{id}/profiles/p-1"),
                None,
            )
            .await;
            assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
        }
        assert_eq!(
            locks(),
            0,
            "a host that does not exist has no edits to serialize, and no entry to remember"
        );

        // The control: a real host's edit does allocate, so this is a bound
        // rather than a lock map that never fills.
        let (status, _) = request(
            &harness,
            "POST",
            &format!("/api/hosts/{local}/profiles/p-1"),
            Some(edit_body("Renamed")),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(locks(), 1);
        drop(harness);
        let _ = peer.await;
    }

    /// A profile request against a host that is not connected refuses the
    /// way every other host-scoped operation does, naming the state.
    ///
    /// Worth pinning because profiles are the newest surface to route
    /// through `host_client`, and the alternative — a bespoke lookup that
    /// answered 500, or worse queued the edit — is exactly what the shared
    /// routing exists to prevent (SPEC.md: nothing queues in v1).
    #[tokio::test]
    async fn profiles_on_a_down_host_refuse_with_the_hosts_state() {
        let (builder, host) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@down",
                rest_harness::HostScript {
                    identity: Some("identity-down".to_string()),
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(host).await;
        harness.fleet.take_down(host);
        harness
            .await_state(host, |state| state.phase() == "unreachable-reprobing")
            .await;

        let (status, body) = request(
            &harness,
            "GET",
            &format!("/api/hosts/{host}/profiles"),
            None,
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::CONFLICT);
        assert!(
            body.as_str()
                .unwrap_or_default()
                .contains("unreachable-reprobing"),
            "the refusal must name the state the host is in: {body:?}"
        );
    }
}
