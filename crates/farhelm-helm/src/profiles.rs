//! `/api/hosts/{id}/profiles` — agent profiles, proxied to the host that
//! owns them, plus the one thing the helm owns about them (PLAN_M6_75.md
//! item 5).
//!
//! ## The split: catalogs are the supervisor's, defaults are the helm's
//!
//! A profile is per-supervisor state. The catalog, the ids, the validation
//! rules and the snapshot semantics all live over there, and everything in
//! this module except one field is a pass-through to the owning host's live
//! connection — the same `host_client` routing every other host-scoped
//! operation uses, so a profile request against a host that is down refuses
//! exactly like a session operation against it, naming the state.
//!
//! What the helm owns is the REMEMBERED DEFAULT: which profile a session was
//! last created from, per host, in helm.db. That is deliberately not on the
//! wire (PLAN_M6_75.md item 3) — it is a preference of whoever is driving
//! this helm, not a fact about the supervisor, and a supervisor serving two
//! helms has no business holding either one's last choice.
//!
//! ## A default belongs to an INSTALL, not to a registry row
//!
//! The single fact that shapes every rule about the remembered default:
//! profile ids are minted per supervisor AND every fresh supervisor seeds the
//! same starter profiles, so an id recorded against one install does not
//! merely go stale when the row moves — it RESOLVES on the successor, to a
//! profile the user never chose, offered back as their own last choice. That
//! is precisely the guess SPEC.md's ask-don't-guess rule exists to prevent.
//!
//! So the binding is enforced at every point a default can cross an install
//! boundary, and no single one of them is trusted alone: the write is bound
//! to the connection claim it was made on, adoption and a genuine retarget
//! delete the row inside their own transactions, the stored row carries the
//! identity it was recorded against and is revalidated on every read
//! (`crate::store::HelmStore::remembered_profile`), and [`list_profiles`]
//! rechecks the claim after both of its reads so one install's catalog cannot
//! be paired with another's default.
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
//! ## What a client may ASSERT about the moment it prepared a request in
//!
//! Every route here names a host by registry id, and an id outlives the
//! install it points at: a retarget or an adoption elsewhere replaces what
//! answers on it, silently, and colliding starter profile ids make a
//! misdirected profile request RESOLVE on the successor rather than fail. So
//! every route here — the catalog READ included, which files its answer under
//! a connection the client cannot otherwise name — accepts an OPTIONAL
//! precondition naming the connection it was written against, and an update
//! accepts a second naming the definition it means to replace. Absent means
//! "no claim", which is what keeps `curl`, the CLI, and older clients working
//! exactly as before. See [`crate::precondition`] for the whole story,
//! including how these compose with the connection-claim revalidation every
//! request here already does, and what remains the client's own job.
//!
//! The other half of that story is what a client is given to guard WITH. The
//! catalog read serves a fingerprint per definition
//! ([`ProfilesView::definitions`]) and every successful create and update
//! answers with the fingerprint of what was committed ([`ProfileReply`]), so a
//! client that has just saved can guard its next edit from the reply it is
//! already holding rather than by re-reading — which it would otherwise have
//! to do before the row could safely be reopened.
//!
//! ## Every mutation that changed something invalidates
//!
//! Profiles are one of the surfaces the goal promises arrives without
//! polling: an edit in one client must reach another client's open profile
//! surface and its create dialog. Each mutation that actually changed the
//! catalog therefore bumps the fleet's revision
//! (`crate::manager::FleetEvents`). A plain read does not, and neither does
//! an edit that submitted exactly what was already stored — see
//! [`update_profile`], which recognizes that case here rather than waking the
//! fleet for it, and which serializes each host's edits so that recognition
//! is a decision rather than a guess about a catalog that may already have
//! moved.
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
//! A CANCELLED request is the case with no such cover, and it is the ordinary
//! one rather than the exotic one: an axum handler's future is dropped the
//! moment its client disconnects, so a browser tab closed a heartbeat after
//! Save abandons a mutation whose frame is already with the supervisor. The
//! supervisor commits, this side never sees the reply, and nothing bumps —
//! while every other client goes on showing a catalog that no longer exists,
//! because catalogs are read on demand and have no refresh loop to heal them.
//! So a mutation's completion is deliberately not the handler's to lose: see
//! [`committed`], which carries the reply, the invalidation and the per-host
//! lock's release into a task the handler only awaits.

use crate::sessions::host_client;
use crate::store::HostId;
use crate::{AppState, http_error};
use anyhow::Context;
use axum::extract::{Path as AxPath, State};
use axum::response::IntoResponse;
use farhelm_proto::{AgentKind, ErrorKind, Profile};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// What `GET /api/hosts/{id}/profiles` answers with: the host's catalog, this
/// helm's remembered default for it, and a fingerprint per definition.
///
/// See the module docs for why the first two travel in one shape. The field
/// names are frozen by PLAN_M6_75.md item 6's consumer; `definitions` is
/// additive, and a client that does not guard its updates can ignore it
/// entirely.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ProfilesView {
    /// The catalog exactly as the supervisor ordered it (by profile id
    /// ascending — stable across renames). Not re-sorted here: a client
    /// wanting the user's alphabet sorts locally, where it knows the locale.
    pub(crate) profiles: Vec<Profile>,
    /// The id of the profile a session was last created from on this host,
    /// or `None` if none ever was.
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
    /// Each profile's DEFINITION fingerprint, keyed by profile id — the value
    /// an editor hands back as `expected_definition` when it saves
    /// ([`crate::precondition::definition_fingerprint`]).
    ///
    /// Served rather than left to the client to compute, and that is the point
    /// of it existing at all: the fingerprint is compared for exact equality,
    /// so a client that builds its own has to reproduce this encoding
    /// byte-for-byte in another language, and any drift shows up as guarded
    /// updates that refuse forever with nothing wrong. Echoing an opaque value
    /// back has nothing to get wrong.
    ///
    /// A separate map rather than a field on each profile because `Profile` is
    /// the WIRE type (`farhelm_proto`), shared with the supervisor, and a
    /// fingerprint is this API's convenience rather than a fact about the
    /// catalog. Sorted (a `BTreeMap`) so the JSON is stable between reads.
    pub(crate) definitions: std::collections::BTreeMap<String, String>,
}

/// The body of a profile create or update — everything but the id, which the
/// supervisor mints on a create (a client has no way to know one in advance,
/// and letting it propose one would invite collisions the catalog would then
/// have to arbitrate) and the URL carries on an update.
///
/// The two `expected_*` fields are PRECONDITIONS rather than content: they say
/// what the caller believed when it composed this request, and nothing about
/// them reaches the supervisor. Both are optional, and absent means the
/// request behaves exactly as it did before they existed — see
/// [`crate::precondition`] for why that compatibility is permanent rather than
/// transitional.
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
    /// Which CONNECTION the caller prepared this request against — a
    /// `HostView::incarnation` it read from `GET /api/hosts`.
    ///
    /// Present, and this mutation is refused unless the host is still on that
    /// connection ([`crate::precondition::incarnation_holds`]). Absent, and no
    /// claim is made.
    expected_incarnation: Option<u64>,
    /// Which DEFINITION this update means to replace — the fingerprint the
    /// editor was seeded from ([`ProfilesView::definitions`]).
    ///
    /// UPDATE ONLY. A create has no prior definition to expect, so a create
    /// carrying this is REFUSED rather than served with the field ignored: the
    /// caller believes it sent a precondition, and silently dropping one is
    /// how a client comes to trust a guarantee nothing is providing.
    expected_definition: Option<String>,
}

/// The query string of the two profile routes that carry no body: the catalog
/// READ and the DELETE.
///
/// A struct for one optional field, because that is what an axum `Query`
/// extractor needs, and shared by both routes because the field means exactly
/// one thing — see [`ProfileSpec::expected_incarnation`], which is the same
/// precondition spelled in a body.
///
/// Unknown parameters are IGNORED, matching `serde`'s default everywhere else
/// in this API. A typo therefore proceeds without the guard the caller thought
/// it sent — the same exposure `expected_incarnation` has when it is simply
/// omitted, which is the compatibility posture the whole feature rests on.
#[derive(Deserialize)]
pub(crate) struct ConnectionQuery {
    /// See [`ProfileSpec::expected_incarnation`] — identical meaning, spelled
    /// as a query parameter.
    expected_incarnation: Option<u64>,
}

/// What a profile CREATE or UPDATE answers with: the definition as committed,
/// plus its fingerprint.
///
/// The profile is FLATTENED, which is the whole compatibility story: every
/// field these replies have always carried (`id`, `name`, `invocation`,
/// `agent_kind`, `resume_template`) stays exactly where it was, and
/// `fingerprint` is an additive sibling. A nested `{"profile": {...}}` would
/// have been tidier and would have broken every existing reader.
///
/// ## Why a mutation reply carries it at all
///
/// Without it, a client that has just successfully edited a profile holds the
/// new definition beside the OLD fingerprint (or, after a create, none at
/// all) until it re-reads the catalog. Both halves of that are bugs the user
/// sees: reopening the row and saving again sends a fingerprint the helm no
/// longer recognizes, so an edit nobody raced is refused as stale; and a
/// freshly created profile cannot be guarded on its first edit because there
/// is nothing to send. Handing back the committed fingerprint lets a client
/// fold the reply into what it holds and guard the very next edit.
///
/// Computed from the definition the far side actually COMMITTED — the profile
/// carried by `ProfileCreated`/`ProfileUpdated`, not the one submitted — so a
/// supervisor that normalized anything is described by the fingerprint rather
/// than contradicted by it. It is the same
/// [`crate::precondition::definition_fingerprint`] the catalog read serves and
/// the precondition compares, which is what makes "reply, then guard" work
/// without a re-read.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ProfileReply {
    #[serde(flatten)]
    profile: Profile,
    /// The canonical encoding of the definition as committed — byte-for-byte
    /// what a later [`list_profiles`] serves for this profile in
    /// [`ProfilesView::definitions`].
    fingerprint: String,
}

impl ProfileReply {
    /// The reply for a profile the far side just committed.
    fn of(profile: Profile) -> ProfileReply {
        ProfileReply {
            fingerprint: crate::precondition::definition_fingerprint(&profile),
            profile,
        }
    }
}

/// `GET /api/hosts/{id}/profiles` — one host's profile catalog plus this
/// helm's remembered default for it.
///
/// A live read from the owning supervisor, never a cache: profiles are
/// small, hand-curated, and read when a user opens a picker, so there is
/// nothing a cache would buy that is worth a second copy of the catalog
/// going stale. The consequence is stated rather than hidden — a host that
/// is not connected cannot answer, and this refuses with the host's state
/// named, exactly as a session operation on it would.
///
/// ## The two halves must come from ONE install
///
/// The catalog and the default are read in separate awaits and cannot be
/// atomic — one comes over the wire and the other from helm.db — but they
/// must at least describe the same connection. Between them a host can be
/// retargeted, adopted, or simply reconnect to a different install, and the
/// reply would then pair one install's catalog with another install's
/// remembered id. That is not a stale number: starter profile ids collide
/// across installs by construction, so the id would RESOLVE against the
/// catalog beside it and be offered as the user's own last choice.
///
/// So the connection claim taken before the reads is checked again after
/// both, and a claim that moved is a [`ErrorKind::Conflict`] telling the
/// caller to ask again. Refusing rather than retrying here: a retry loop
/// against a flapping host is unbounded work on a read a user can simply
/// repeat, and the client already re-reads on the invalidation feed.
///
/// ## A READ takes a precondition too, and why one is worth taking
///
/// `?expected_incarnation=` says which connection the caller believes it is
/// reading. A read is not a mutation and cannot damage anything on the far
/// side, so the reason is entirely about what the ANSWER is filed under: a
/// client that stores catalogs per (host, connection) has only the host id in
/// the request, and a retarget or an adoption between its dispatch and this
/// routing hands it the successor's catalog to record under the predecessor's
/// key. Everything it then does with that cache — seeding an editor, guarding
/// an update, offering a remembered default — is about the wrong install.
///
/// Checked where the read BINDS to a connection: once when the claim is taken
/// (so a caller naming a connection this host has already left is refused
/// before a round trip), and once at the same post-read revalidation the two
/// halves are judged by — where a guarded caller gets the marked refusal
/// (`crate::precondition`) rather than the generic one, because the two ask
/// for different recoveries.
///
/// The RESIDUAL is real and belongs to the client. This can only refuse up to
/// the moment it answers; a retarget landing while the reply is in flight
/// produces a perfectly valid catalog for a connection that is already over,
/// and no server-side check can close that. What closes it is the client's own
/// activation gate — the same (host, connection) key it stores under, checked
/// again when it uses what it stored — and the invalidation feed, which tells
/// it to re-read as soon as the connection changes.
///
/// ## What an editor is seeded with
///
/// The reply also carries a FINGERPRINT per definition
/// ([`ProfilesView::definitions`]). An editor holds the one it loaded and
/// hands it back as `expected_definition` when it saves, which is how
/// [`update_profile`] can tell "this replaces what I read" from "this
/// silently reverts somebody else's change". Nothing requires a client to use
/// it, and a client that ignores it gets exactly the previous behavior.
pub(crate) async fn list_profiles(
    State(state): State<Arc<AppState>>,
    AxPath(host): AxPath<HostId>,
    axum::extract::Query(query): axum::extract::Query<ConnectionQuery>,
) -> impl IntoResponse {
    list_profiles_staged(
        state,
        host,
        query.expected_incarnation,
        std::future::ready(()),
    )
    .await
}

/// [`list_profiles`], with a seam between its two reads and the claim check
/// that judges them.
///
/// `staged` is awaited exactly where a reconnection would have to land for the
/// revalidation to matter: after the catalog and the remembered default are
/// both in hand, before either is reported. Production passes a ready future.
///
/// The seam exists because the alternative is a test that proves nothing. This
/// window is two awaits wide inside one handler, and the only way to put a
/// real reconnection in it from outside is to break the connection the catalog
/// read is waiting on — which fails the request for a different reason
/// entirely and would pass just as well against a handler that had dropped the
/// check. Staging the rotation here keeps the assertion on the REAL handler,
/// its real refusal, and its real status code.
async fn list_profiles_staged(
    state: Arc<AppState>,
    host: HostId,
    expected_incarnation: Option<u64>,
    staged: impl std::future::Future<Output = ()>,
) -> axum::response::Response {
    let (claim, client) = match host_client(&state, host) {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    if let Err(e) = crate::precondition::incarnation_holds(&claim, expected_incarnation) {
        return http_error(e);
    }
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
    let default_profile = match state.store.remembered_profile(host).await {
        Ok(default_profile) => default_profile,
        Err(e) => return http_error(e),
    };
    staged.await;
    // The claim these reads were taken under, judged against the world as it
    // is now. A caller that ASSERTED a connection gets the marked refusal
    // rather than the generic one: its recovery is "re-read under the
    // connection you are on now", which is a different instruction from the
    // unguarded caller's "ask again".
    if let Err(e) = still_the_same_connection(&state, &claim) {
        return http_error(match expected_incarnation {
            Some(expected) => crate::precondition::connection_moved(
                host,
                expected,
                state.manager.status(host).map(|status| status.incarnation),
            ),
            None => e,
        });
    }
    let definitions = profiles
        .iter()
        .map(|profile| {
            (
                profile.id.clone(),
                crate::precondition::definition_fingerprint(profile),
            )
        })
        .collect();
    axum::Json(ProfilesView {
        profiles,
        default_profile,
        definitions,
    })
    .into_response()
}

/// Refuse unless `claim` still names the connection this host currently has.
///
/// The incarnation token is minted afresh whenever a host's client changes —
/// including when it goes away — so equality here means every read taken
/// under `claim` describes one uninterrupted connection to one install. The
/// identity is compared too, because a host can learn or change one without
/// the client object changing.
fn still_the_same_connection(
    state: &AppState,
    claim: &crate::manager::SessionClaim,
) -> anyhow::Result<()> {
    let current = state.manager.status(claim.host);
    let matches = current.as_ref().is_some_and(|status| {
        status.incarnation == claim.incarnation
            && match &status.state {
                crate::manager::HostState::Connected { identity, .. } => {
                    *identity == claim.identity
                }
                _ => false,
            }
    });
    if matches {
        return Ok(());
    }
    Err(anyhow::Error::new(crate::SupervisorError {
        kind: ErrorKind::Conflict,
        message: format!(
            "host {} changed connection while this request was being answered; its catalog and \
             this helm's remembered default could describe different installs, so nothing is \
             being reported — ask again",
            claim.host
        ),
    }))
}

/// Carry one catalog mutation to its END, whatever happens to the request
/// that asked for it.
///
/// The mutation frame is already on its way to the supervisor by the time
/// `mutation` is awaited, so a caller that goes away here does not undo
/// anything — it only abandons the reply, and with it the invalidation this
/// helm owes every OTHER client. Catalogs have no periodic refresh to heal
/// that: a create dialog somewhere else would keep offering a profile that no
/// longer exists until something unrelated moved the revision. A browser tab
/// closing mid-save is enough to produce it.
///
/// So the whole span — awaiting the reply, revalidating the claim, bumping,
/// and releasing `serialized` — runs in a task of its own that this function
/// merely awaits. Cancelling the handler drops the await, never the task.
///
/// `serialized` is moved in for the same reason: the per-host lock must be
/// held until the mutation is genuinely finished, and a guard dropped by
/// cancellation would let the next queued edit read a catalog the supervisor
/// is still in the middle of changing.
///
/// The claim is revalidated on the way out because a connection that changed
/// mid-flight means the mutation landed on an install that is no longer this
/// host: reporting it as this host's current state — and waking the fleet to
/// re-read on the strength of it — would describe a catalog nobody is looking
/// at. No bump goes with that refusal (the connection change published its
/// own).
///
/// `expected` is the caller's own assertion about that connection, and it is
/// carried here only to decide HOW the refusal reads: a request that named a
/// connection gets `crate::precondition`'s marked 409, an unguarded one gets
/// [`still_the_same_connection`]'s. Same condition, same status, different
/// recovery — and a client that went to the trouble of guarding should not have
/// to parse prose to learn that its guard is what fired.
async fn committed<T: Send + 'static>(
    state: &Arc<AppState>,
    claim: crate::manager::SessionClaim,
    expected: Option<u64>,
    serialized: Option<tokio::sync::OwnedMutexGuard<()>>,
    mutation: impl std::future::Future<Output = anyhow::Result<T>> + Send + 'static,
) -> anyhow::Result<T> {
    let state = Arc::clone(state);
    tokio::spawn(async move {
        let _editing = serialized;
        let outcome = mutation.await?;
        if let Err(e) = still_the_same_connection(&state, &claim) {
            let host = claim.host;
            return Err(match expected {
                Some(expected) => crate::precondition::connection_moved(
                    host,
                    expected,
                    state.manager.status(host).map(|status| status.incarnation),
                ),
                None => e,
            });
        }
        state.manager.events().bump();
        Ok(outcome)
    })
    .await
    .context("profile mutation task panicked")?
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
/// the exception is reasoned rather than an oversight: an edit's suppression
/// decision is about ONE profile id, and a create mints an id nobody holds
/// yet, so it cannot change the answer an in-flight [`update_profile`] is
/// computing. Queueing creates behind edits would buy nothing and would make
/// a slow catalog read block an unrelated create.
///
/// An optional `expected_incarnation` says which install the caller meant. It
/// is checked against the claim routing produced, and the claim is revalidated
/// before this reports success ([`committed`]), so a create that lands on a
/// replaced install is refused rather than reported — see
/// [`crate::precondition`] for why an id alone cannot say this, and why a
/// profile create in particular RESOLVES rather than failing over there.
///
/// `expected_definition` is refused outright: there is no prior definition a
/// create could be replacing.
pub(crate) async fn create_profile(
    State(state): State<Arc<AppState>>,
    AxPath(host): AxPath<HostId>,
    axum::Json(spec): axum::Json<ProfileSpec>,
) -> impl IntoResponse {
    if spec.expected_definition.is_some() {
        return http_error(anyhow::Error::new(crate::SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message: "expected_definition is an update precondition: a create has no stored \
                      definition to compare against, so a create carrying one is refused rather \
                      than served as if the precondition had been checked"
                .to_string(),
        }));
    }
    let (claim, client) = match host_client(&state, host) {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    if let Err(e) = crate::precondition::incarnation_holds(&claim, spec.expected_incarnation) {
        return http_error(e);
    }
    let expected = spec.expected_incarnation;
    let created = committed(&state, claim, expected, None, async move {
        client
            .create_profile(
                &spec.name,
                &spec.invocation,
                spec.agent_kind,
                spec.resume_template,
            )
            .await
    })
    .await;
    match created {
        // With the fingerprint of what was committed, so the caller can guard
        // its first edit of this profile without a re-read (see
        // [`ProfileReply`]).
        Ok(profile) => axum::Json(ProfileReply::of(profile)).into_response(),
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
/// ## An identical edit is a no-op, decided HERE
///
/// A profile editor that submits on blur, a client that re-saves a form it
/// did not change, a retry — all of them send an edit whose fields are
/// exactly what is already stored. Forwarded, each one is a successful
/// mutation, and each one would wake every open client in the fleet to
/// re-read a catalog that did not move. So this reads the catalog first and,
/// when the submitted definition is byte-identical to the stored one, skips
/// the supervisor call and the invalidation entirely and answers with what
/// is already there. The reply is indistinguishable from a real edit's,
/// which is the point: an idempotent update is a successful update.
///
/// The check belongs to the HELM rather than the supervisor, and not by
/// preference. A supervisor-side "did this change anything" answer would
/// have to travel back over the wire, which means new reply vocabulary in a
/// protocol version this milestone deliberately does not reopen
/// (PLAN_M6_75.md item 3: the wire shapes for M6.75 all landed in v10, and
/// `ProfileUpdated` carries the profile, not a changed bit). Doing it here
/// costs one extra `ListProfiles` on the edit path — a bounded, unpaginated
/// read of a hand-curated catalog — and needs nothing from the far side it
/// does not already offer.
///
/// ## What the suppression actually guarantees
///
/// Read-compare-forward is three steps, and it is only a decision if nothing
/// slips between them. Two clients of THIS helm editing one host would
/// otherwise interleave into outcomes that are wrong rather than merely
/// racy: A reads X, B commits Y, A submits X and finds it "identical" to the
/// catalog it read — so A reports success while the durable state is Y, and
/// last-write-wins has been broken by a suppression rather than by a write.
/// Two identical concurrent edits could likewise both find the old value and
/// both forward, waking the fleet twice for one change.
///
/// So all three steps run under one per-host lock (`AppState::profile_edits`),
/// making them a queue: among this helm's own clients, an edit either wins
/// outright or sees the winner's result and suppresses honestly. A DELETE
/// takes the same lock for the same reason — see [`delete_profile`], where a
/// delete landing inside this span would make a suppression report 200 for a
/// profile that no longer exists.
///
/// The guarantee STOPS at this helm. Another helm, or anything else speaking
/// to the same supervisor, can still commit between this lock's read and its
/// forward, and this side has no way to notice. Closing that needs the
/// supervisor to answer "did this change anything" — a changed bit on
/// `ProfileUpdated` — which is protocol vocabulary this milestone
/// deliberately does not reopen. That is the eventual fix, and it is what
/// makes the current guarantee worth stating precisely rather than implying
/// it is total.
///
/// Within the lock the read is still NOT a precondition check and this is
/// still not a compare-and-swap against the supervisor: it decides only
/// whether there is anything to forward.
///
/// ## Routing is checked twice, and the claim once more on the way out
///
/// The host is routed BEFORE the lock is taken, because the lock map is keyed
/// by a caller-supplied path id and must only ever hold real hosts
/// (`AppState::profile_edit_lock`). It is routed AGAIN under the lock, because
/// a request can queue behind another for as long as that one takes and the
/// host may be forgotten, retargeted, or dropped in the meantime — the client
/// resolved before the wait would then be a connection to somewhere the id no
/// longer names.
///
/// And the connection claim is revalidated before this handler reports
/// anything at all, on BOTH exits. A suppression is a claim about what is
/// stored on this host right now; forwarded or not, an edit whose connection
/// moved underneath it describes an install the caller was not addressing.
///
/// ## The two optional preconditions
///
/// `expected_incarnation` names the install this edit was written for, and is
/// checked at BOTH routing points — before the lock and again under it —
/// because the wait for the lock is unbounded by anything this request
/// controls, and a retarget landing during it is exactly the case the
/// precondition exists for. `expected_definition` names the definition the
/// editor was seeded from, and is checked under the lock against the same
/// catalog read the suppression compares, which is what makes "unchanged since
/// I loaded it" a decision rather than a hope.
///
/// The definition precondition is the helm's answer to a stale editor
/// silently reverting somebody else's change — an update replaces the whole
/// definition, so last-write-wins here loses a field the loser never saw. It
/// is deliberately NOT a compare-and-swap against the supervisor: another helm
/// can still commit between this lock's read and its forward, and closing that
/// needs the wire vocabulary this milestone does not reopen (see above). What
/// this closes is the case a single helm can see, which is the case a person
/// with two tabs actually produces.
pub(crate) async fn update_profile(
    State(state): State<Arc<AppState>>,
    AxPath((host, profile_id)): AxPath<(HostId, String)>,
    axum::Json(spec): axum::Json<ProfileSpec>,
) -> impl IntoResponse {
    update_profile_staged(state, host, profile_id, spec, std::future::ready(())).await
}

/// [`update_profile`], with a seam between its pre-lock validation and the
/// lock it then waits for.
///
/// `staged` is awaited exactly where a request queues: past the first routing
/// and precondition check, before `enter_profile_edit`. That is the interval
/// the SECOND check exists for — a retarget or an adoption landing while this
/// request waits its turn behind another client's edit — and it is otherwise
/// unreachable from outside, because a test cannot schedule anything into a
/// lock acquisition. Production passes a ready future.
///
/// The seam is at the queue rather than deeper on purpose: a future edit that
/// dropped the post-lock re-check would still pass every test that only ever
/// arrives at an uncontended lock.
async fn update_profile_staged(
    state: Arc<AppState>,
    host: HostId,
    profile_id: String,
    spec: ProfileSpec,
    staged: impl std::future::Future<Output = ()>,
) -> axum::response::Response {
    // Routed before the lock is allocated, and again after it is held: see
    // this function's docs for why neither check covers the other. The
    // precondition rides along with each, for the same reason.
    match host_client(&state, host) {
        Ok((claim, _)) => {
            if let Err(e) =
                crate::precondition::incarnation_holds(&claim, spec.expected_incarnation)
            {
                return http_error(e);
            }
        }
        Err(e) => return http_error(e),
    }
    staged.await;
    let editing = state.enter_profile_edit(host).await;
    let (claim, client) = match host_client(&state, host) {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    if let Err(e) = crate::precondition::incarnation_holds(&claim, spec.expected_incarnation) {
        return http_error(e);
    }
    // Taken before `spec` is dismantled into the profile below, because both
    // exits past this point still need to know whether the caller guarded.
    let spec_expected = spec.expected_incarnation;
    let expected_definition = spec.expected_definition;
    let profile = Profile {
        id: profile_id,
        name: spec.name,
        invocation: spec.invocation,
        agent_kind: spec.agent_kind,
        resume_template: spec.resume_template,
    };
    // An unreadable catalog is not a reason to refuse the edit ITSELF: the
    // read is an optimization, and the honest fallback is to forward, which is
    // what this endpoint did before the check existed.
    let catalog = client.list_profiles().await;
    let stored = match &catalog {
        Ok(catalog) => catalog.iter().find(|held| held.id == profile.id).cloned(),
        Err(error) => {
            tracing::debug!(
                host,
                error = %error,
                "could not read the catalog to check for an identical edit; forwarding it"
            );
            None
        }
    };
    // The precondition is judged on the SAME read the suppression below
    // compares against, under the same lock — so "unchanged since the editor
    // loaded it" and "identical to what is stored" are two questions about one
    // catalog rather than two.
    //
    // A catalog that could not be READ refuses instead of falling back. The
    // fallback is honest for a suppression and dishonest for a precondition:
    // forwarding would report a guarantee as checked when nothing was
    // compared, which is the one outcome a caller asking for a precondition
    // cannot tolerate.
    if let Some(expected) = expected_definition.as_deref() {
        let checked = match &catalog {
            Err(error) => Err(crate::precondition::definition_unverifiable(
                host,
                &profile.id,
                error,
            )),
            Ok(_) => crate::precondition::definition_holds(
                host,
                &profile.id,
                stored.as_ref(),
                Some(expected),
            ),
        };
        if let Err(e) = checked {
            return http_error(e);
        }
    }
    if stored.as_ref() == Some(&profile) {
        // Suppressing means answering "this is already what is stored", which
        // is only true of the install the catalog was read from.
        //
        // The reply carries the fingerprint like any other successful update:
        // an idempotent update is a successful update, and a client that could
        // not tell the two apart by their bodies must not be able to tell them
        // apart by what it can guard with afterwards either.
        return match still_the_same_connection(&state, &claim) {
            Ok(()) => axum::Json(ProfileReply::of(profile)).into_response(),
            Err(e) => http_error(match spec_expected {
                Some(expected) => crate::precondition::connection_moved(
                    host,
                    expected,
                    state.manager.status(host).map(|status| status.incarnation),
                ),
                None => e,
            }),
        };
    }
    let updated = committed(&state, claim, spec_expected, Some(editing), async move {
        client.update_profile(profile).await
    })
    .await;
    match updated {
        Ok(profile) => axum::Json(ProfileReply::of(profile)).into_response(),
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
/// It takes the same per-host lock [`update_profile`] does, and the reason is
/// not symmetry. An edit's suppression is a read-compare-forward span, and a
/// delete landing inside one turns it into a lie: the edit reads a catalog
/// still holding P, the delete removes P, and the edit then finds its
/// submission "identical to what is stored" and answers 200 for a profile that
/// does not exist anywhere. The client is told its edit is durable when
/// nothing of the sort is true, and no invalidation says otherwise because the
/// suppression published none.
///
/// The routing and claim discipline is [`update_profile`]'s, for
/// [`update_profile`]'s reasons: routed before the lock so a made-up path id
/// cannot mint one, routed again under it, and the claim revalidated before
/// this reports success.
///
/// ## The precondition rides in the QUERY STRING here
///
/// `?expected_incarnation=` rather than a body field, because a delete has no
/// body at all in this API and inventing one for a precondition would make
/// every existing caller send `{}` to keep working. Same meaning, same refusal,
/// same marker as the body field on the other two verbs — see
/// [`crate::precondition`]. Deleting the wrong install's profile is not
/// recoverable by re-reading, which is why the option exists on this verb too.
pub(crate) async fn delete_profile(
    State(state): State<Arc<AppState>>,
    AxPath((host, profile_id)): AxPath<(HostId, String)>,
    axum::extract::Query(query): axum::extract::Query<ConnectionQuery>,
) -> impl IntoResponse {
    match host_client(&state, host) {
        Ok((claim, _)) => {
            if let Err(e) =
                crate::precondition::incarnation_holds(&claim, query.expected_incarnation)
            {
                return http_error(e);
            }
        }
        Err(e) => return http_error(e),
    }
    let editing = state.enter_profile_edit(host).await;
    let (claim, client) = match host_client(&state, host) {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    if let Err(e) = crate::precondition::incarnation_holds(&claim, query.expected_incarnation) {
        return http_error(e);
    }
    let deleted = committed(
        &state,
        claim,
        query.expected_incarnation,
        Some(editing),
        async move { client.delete_profile(&profile_id).await },
    )
    .await;
    match deleted {
        Ok(()) => axum::Json(serde_json::json!({})).into_response(),
        Err(e) => http_error(e),
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

        /// How many `UpdateProfile` frames arrived — the number the
        /// suppression is about.
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
            // The catalog read every edit makes first, to recognize an
            // identical submission (see `update_profile`). What is stored
            // here differs from what the test submits, so the edit is
            // forwarded — which is the case this test is about.
            let ControlMsg::ListProfiles { req_id } = asked(&mut reader).await else {
                panic!("an edit reads the catalog it is compared against first");
            };
            writer
                .write_frame(&Frame::control(&ControlMsg::ProfileList {
                    req_id,
                    profiles: vec![profile("p-path", "Before")],
                }))
                .await
                .unwrap();
            let ControlMsg::UpdateProfile { req_id, profile } = asked(&mut reader).await else {
                panic!("the helm must proxy an edit as UpdateProfile");
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

    /// A profile-backed create is what makes a profile the host's
    /// remembered default — and the default is then served back beside the
    /// catalog.
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
    async fn a_profile_backed_create_becomes_the_hosts_remembered_default() {
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
            before + 1,
            "exactly one invalidation, and the recorded session cannot be the one that made it \
             (its cache write was a no-op) — so it is the remembered default reaching the other \
             clients' create dialogs"
        );
        peer.await.unwrap();
    }

    /// REPAIRING a remembered default's identity binding invalidates, even
    /// though the profile id did not change.
    ///
    /// Spec: after a host learns an identity — which makes the row recorded
    /// against no identity unreadable, so the default disappears — the next
    /// create from that same profile restores it AND moves the fleet's
    /// revision by one.
    ///
    /// The path is ordinary: a supervisor with no identity, upgraded or
    /// reconfigured into one that has one. What makes it worth a test is that
    /// the two obvious observables both LOOK right without the fix — the row
    /// is repaired by the upsert either way, and the default reads back — so
    /// the only thing that fails is the invalidation, and its absence is
    /// invisible until some other client's create dialog is still offering
    /// nothing an hour later.
    ///
    /// The bump is ISOLATED the same way its sibling test above isolates one:
    /// the session this create returns is byte-identical to one the host
    /// already listed, so recording it publishes nothing and the remembered
    /// default is the only thing left that can move the revision.
    #[tokio::test]
    async fn repairing_a_remembered_defaults_identity_binding_invalidates() {
        let existing = farhelm_proto::SessionInfo {
            cwd: "/work".to_string(),
            source_profile: Some(farhelm_proto::SourceProfile {
                id: "p-favorite".to_string(),
                name: "Claude Code".to_string(),
                existence: farhelm_proto::ProfileExistence::Present,
            }),
            ..rest_harness::session("sess-new", 1_700_000_500)
        };
        // The host reports NO identity to begin with, so the default recorded
        // now is bound to nothing.
        let harness = rest_harness::FleetBuilder::new()
            .await
            .local(rest_harness::HostScript {
                identity: None,
                sessions: vec![existing.clone()],
                ..rest_harness::HostScript::default()
            })
            .await
            .start()
            .await;
        let local = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(local).await;
        let status = harness
            .manager
            .status(local)
            .expect("the local host has an actor");
        assert!(
            harness
                .manager
                .remember_profile_default(
                    &crate::manager::SessionClaim {
                        host: local,
                        incarnation: status.incarnation,
                        identity: None,
                    },
                    "p-favorite",
                )
                .await
                .expect("an identity-less host may record a default")
        );

        // The supervisor comes back reporting an identity. The stored row is
        // bound to none, so it stops being this install's preference.
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn({
            let existing = existing.clone();
            async move {
                let (mut reader, mut writer) = peer_up(peer_side).await;
                let ControlMsg::CreateSession { req_id, .. } = asked(&mut reader).await else {
                    panic!("expected CreateSession");
                };
                writer
                    .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                        req_id,
                        session: existing,
                    }))
                    .await
                    .unwrap();
            }
        });
        let harness = harness
            .restart_with(|fleet| {
                fleet.edit(local, |script| {
                    script.identity = Some("identity-learned".to_string());
                    script.peer = Some(client_side);
                });
            })
            .await;
        harness
            .await_refreshed_as(local, "identity-learned", 1)
            .await;
        assert_eq!(
            harness.store.remembered_profile(local).await.unwrap(),
            None,
            "a default bound to no identity is not the identified install's preference"
        );

        let before = harness.manager.events().revision();
        let (status, _) = request(
            &harness,
            "POST",
            "/api/sessions",
            Some(serde_json::json!({"cwd": "/work", "profile_id": "p-favorite"})),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            harness.store.remembered_profile(local).await.unwrap(),
            Some("p-favorite".to_string()),
            "the create rebinds the default to the identity the host now reports"
        );
        assert_eq!(
            harness.manager.events().revision(),
            before + 1,
            "and the default going from absent back to present is a change other clients must be \
             told about — the profile id standing still does not make it one"
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

    /// An edit that submits exactly what is stored reaches NO supervisor and
    /// wakes NO client.
    ///
    /// A profile editor that saves on blur, a form re-submitted unchanged, a
    /// retry — each is a successful mutation that changed nothing, and each
    /// would otherwise wake every open client in the fleet to re-read a
    /// catalog that did not move.
    ///
    /// "No round trip" is proven by ORDER rather than by a timeout: the peer
    /// answers the catalog read, and the very next request it is asked for
    /// is the DELETE this test sends afterwards. An `UpdateProfile` slipping
    /// through would arrive in that slot and fail the assertion by name.
    #[tokio::test]
    async fn an_identical_profile_edit_is_a_no_op_with_no_round_trip() {
        let stored = profile("p-1", "Claude Code");
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn({
            let stored = stored.clone();
            async move {
                let (mut reader, mut writer) = peer_up(peer_side).await;
                // The catalog read the helm makes to decide whether there is
                // anything to forward.
                let ControlMsg::ListProfiles { req_id } = asked(&mut reader).await else {
                    panic!("an edit must first read the catalog it is compared against");
                };
                writer
                    .write_frame(&Frame::control(&ControlMsg::ProfileList {
                        req_id,
                        profiles: vec![stored],
                    }))
                    .await
                    .unwrap();
                // The NEXT request must be the delete below. Anything else —
                // an UpdateProfile the helm should have skipped — fails here.
                match asked(&mut reader).await {
                    ControlMsg::DeleteProfile { req_id, profile_id } => {
                        assert_eq!(profile_id, "p-1");
                        writer
                            .write_frame(&Frame::control(&ControlMsg::ProfileDeleted { req_id }))
                            .await
                            .unwrap();
                    }
                    other => panic!("an identical edit must not be forwarded; got {other:?}"),
                }
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
        assert_eq!(
            status,
            axum::http::StatusCode::OK,
            "an idempotent update is a successful update"
        );
        assert_eq!(
            value["id"], "p-1",
            "and answers with the profile, indistinguishably from a real edit"
        );
        assert_eq!(
            harness.manager.events().revision(),
            before,
            "an edit that changed nothing must wake nobody"
        );

        // The ordering probe: this is the next thing the peer is asked.
        let (status, _) = request(
            &harness,
            "DELETE",
            &format!("/api/hosts/{local}/profiles/p-1"),
            None,
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        peer.await.unwrap();
    }

    /// Two clients editing one host's catalog at the same time are a QUEUE,
    /// not a race — and two IDENTICAL edits cost one forward and one
    /// invalidation between them.
    ///
    /// Spec: with a second edit issued while the first is still inside its
    /// read-compare-forward span, exactly one `UpdateProfile` reaches the
    /// supervisor and the fleet revision moves by exactly one.
    ///
    /// Read-compare-forward is three steps and only means anything if nothing
    /// slips between them. Unserialized, both clients read the OLD catalog,
    /// both find their submission different from it, and both forward — so
    /// one change wakes every client in the fleet twice. The worse
    /// interleaving is the mirror of it, and it is why this is a correctness
    /// test rather than a chattiness one: A reads X, B commits Y, A submits X
    /// and finds it "identical" to the catalog it read, so A answers success
    /// while the durable state is Y. Last-write-wins is then broken by a
    /// suppression rather than by a write.
    ///
    /// The overlap is FORCED rather than hoped for, at both ends: the peer
    /// holds the first request's catalog read open, and the test does not
    /// release it until the second request has demonstrably QUEUED on the
    /// host's lock (see [`await_queued`]). An unserialized helm never queues,
    /// so it fails at the barrier rather than passing on a lucky schedule.
    #[tokio::test]
    async fn two_identical_concurrent_edits_forward_once_and_invalidate_once() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let shared = CatalogPeer::new(vec![profile("p-1", "Before")]);
        let peer = tokio::spawn(catalog_peer(peer_side, std::sync::Arc::clone(&shared)));

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        let before = harness.manager.events().revision();
        let uri = format!("/api/hosts/{local}/profiles/p-1");

        let first = tokio::spawn({
            let router = harness.router();
            let uri = uri.clone();
            async move { post(router, &uri, edit_body("Renamed")).await }
        });
        // The first request is now inside the helm's serialized span, blocked
        // on the peer's answer.
        shared.arrived.notified().await;

        let second = tokio::spawn({
            let router = harness.router();
            let uri = uri.clone();
            async move { post(router, &uri, edit_body("Renamed")).await }
        });
        // The second request has reached the lock and is waiting on the first
        // — which is the interleaving this test is about, established rather
        // than assumed.
        await_queued(&harness.state, 1).await;
        assert_eq!(
            shared.requests(),
            Vec::<&str>::new(),
            "and it is waiting on the LOCK: nothing of the second edit has reached the supervisor"
        );

        shared.gate.notify_one();
        let (first_status, _) = first.await.unwrap();
        let (second_status, second_body) = second.await.unwrap();
        assert_eq!(first_status, axum::http::StatusCode::OK);
        assert_eq!(
            second_status,
            axum::http::StatusCode::OK,
            "an idempotent update is a successful update, whichever client got there second"
        );
        assert_eq!(
            second_body["name"], "Renamed",
            "and it answers with the definition that is actually stored"
        );

        assert_eq!(
            shared.forwards(),
            1,
            "the second edit must see the first one's result and suppress itself"
        );
        assert_eq!(
            harness.manager.events().revision(),
            before + 1,
            "one change is one invalidation, not one per client that submitted it"
        );
        drop(harness);
        let _ = peer.await;
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

    /// A DELETE cannot land inside an identical edit's suppression window.
    ///
    /// Spec: a delete issued while an edit is between its catalog read and its
    /// decision queues on the same per-host lock, reaches the supervisor only
    /// after that edit has answered, and leaves the catalog empty.
    ///
    /// Unserialized, the delete forwards immediately: the edit then finds its
    /// submission "identical to what is stored" — true of the catalog it read,
    /// false of the world — and answers 200 for a profile that no longer
    /// exists anywhere, publishing no invalidation because it suppressed. The
    /// client is told its edit is durable, and nothing anywhere says otherwise.
    /// A delete is the one mutation that can do this, because it is the only
    /// one that can make a profile the edit already compared against vanish.
    #[tokio::test]
    async fn a_delete_cannot_land_inside_an_identical_edits_suppression_window() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let shared = CatalogPeer::new(vec![profile("p-1", "Claude Code")]);
        let peer = tokio::spawn(catalog_peer(peer_side, std::sync::Arc::clone(&shared)));

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        let uri = format!("/api/hosts/{local}/profiles/p-1");

        // An edit submitting exactly what is stored: the suppression path.
        let edit = tokio::spawn({
            let router = harness.router();
            let uri = uri.clone();
            async move { post(router, &uri, edit_body("Claude Code")).await }
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
        assert_eq!(edit_body["name"], "Claude Code");
        assert_eq!(delete_status, axum::http::StatusCode::OK);

        assert_eq!(
            shared.requests(),
            vec!["list", "delete"],
            "the delete reached the supervisor only after the edit ahead of it had decided — an \
             edit that suppressed AFTER the delete would have reported success for a profile that \
             was already gone"
        );
        assert!(
            shared.catalog().is_empty(),
            "and the delete is what the catalog ends up reflecting"
        );
        drop(harness);
        let _ = peer.await;
    }

    /// A mutation whose HTTP handler is cancelled still finishes: the
    /// supervisor's reply is consumed and the fleet is invalidated.
    ///
    /// Spec: with the create frame already at the peer, dropping the request
    /// task and only then letting the peer answer still moves the revision.
    ///
    /// This is the ordinary case, not an exotic one. An axum handler's future
    /// is dropped the instant its client disconnects, and a browser tab closed
    /// a heartbeat after Save does exactly that — with the mutation frame
    /// already gone. The supervisor commits either way; if the bump lived in
    /// the handler it would die with it, and every OTHER client would go on
    /// showing a catalog that no longer exists, because catalogs are read on
    /// demand and nothing re-reads them on a timer.
    #[tokio::test]
    async fn a_cancelled_profile_mutation_still_invalidates() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let gate = std::sync::Arc::new(tokio::sync::Notify::new());
        let arrived = std::sync::Arc::new(tokio::sync::Notify::new());
        let peer = tokio::spawn({
            let gate = std::sync::Arc::clone(&gate);
            let arrived = std::sync::Arc::clone(&arrived);
            async move {
                let (mut reader, mut writer) = peer_up(peer_side).await;
                let ControlMsg::CreateProfile { req_id, name, .. } = asked(&mut reader).await
                else {
                    panic!("the helm must proxy a create as CreateProfile");
                };
                // The frame is HERE: whatever happens to the requester now,
                // this supervisor is about to commit.
                arrived.notify_one();
                gate.notified().await;
                writer
                    .write_frame(&Frame::control(&ControlMsg::ProfileCreated {
                        req_id,
                        profile: profile("p-minted", &name),
                    }))
                    .await
                    .unwrap();
            }
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        let before = harness.manager.events().revision();

        let requester = tokio::spawn({
            let router = harness.router();
            let uri = format!("/api/hosts/{local}/profiles");
            async move {
                post(
                    router,
                    &uri,
                    serde_json::json!({
                        "name": "Nightly",
                        "invocation": "claude",
                        "agent_kind": "generic",
                    }),
                )
                .await
            }
        });
        arrived.notified().await;
        // The client goes away with the mutation already on the wire.
        requester.abort();
        assert!(
            requester.await.unwrap_err().is_cancelled(),
            "the handler must actually have been dropped, or this proves nothing"
        );

        gate.notify_one();
        let invalidated = tokio::time::timeout(Duration::from_secs(10), async {
            while harness.manager.events().revision() == before {
                tokio::task::yield_now().await;
            }
        })
        .await;
        assert!(
            invalidated.is_ok(),
            "the supervisor committed this create; the clients that are still here must be told"
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

    /// A remembered-default write made against a connection that has since
    /// been replaced is REFUSED, and nothing is stored.
    ///
    /// Profile ids are per-supervisor and every fresh supervisor seeds the
    /// same STARTER profiles, so an id recorded across a retarget can
    /// genuinely resolve on the new install — to a profile the user never
    /// picked, offered back to them as their own last choice. The claim is
    /// what makes that unconstructible; this stages the stale claim
    /// directly, because the window it closes is otherwise only reachable by
    /// timing.
    #[tokio::test]
    async fn a_remembered_default_from_a_superseded_connection_is_refused() {
        let harness = rest_harness::helm_listing(Vec::new()).await;
        let local = rest_harness::local_id(&harness.store).await;
        let status = harness
            .manager
            .status(local)
            .expect("the local host has an actor");
        let current = crate::manager::SessionClaim {
            host: local,
            incarnation: status.incarnation,
            identity: Some("local-identity".to_string()),
        };
        let superseded = crate::manager::SessionClaim {
            incarnation: status.incarnation - 1,
            ..current.clone()
        };

        harness
            .manager
            .remember_profile_default(&superseded, "starter-claude")
            .await
            .expect_err("a claim from a previous connection must not write");
        assert_eq!(
            harness.store.remembered_profile(local).await.unwrap(),
            None,
            "and nothing was stored"
        );

        // The same call on the CURRENT connection is the one that works, so
        // this is a guard rather than a broken path.
        assert!(
            harness
                .manager
                .remember_profile_default(&current, "starter-claude")
                .await
                .expect("the live connection may record a default")
        );
        assert_eq!(
            harness.store.remembered_profile(local).await.unwrap(),
            Some("starter-claude".to_string())
        );
    }

    /// The HANDLER refuses a profiles read whose connection changed under it,
    /// with the status a client acts on.
    ///
    /// Spec: `GET /api/hosts/{id}/profiles` answers 409 when the host
    /// reconnects between its two reads and the reply that was being
    /// assembled, rather than serving the catalog and default it holds.
    ///
    /// The two halves cannot be atomic (one comes over the wire, the other
    /// from helm.db) but they must at least describe one install, and this is
    /// the case where "stale" is not the honest word: starter profile ids
    /// collide across installs by construction, so a default recorded on one
    /// install RESOLVES against another's catalog and would be offered as the
    /// user's own last choice.
    ///
    /// Driven through the real handler, with the reconnection staged at
    /// [`list_profiles_staged`]'s seam. The shape this replaced asserted on
    /// [`still_the_same_connection`] alone, which is a test of the guard and
    /// not of the endpoint: deleting the handler's call to it left that test
    /// passing and the endpoint happily serving two installs' halves.
    #[tokio::test]
    async fn a_profiles_read_whose_connection_changed_underneath_is_refused() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        // Answers catalog reads for as long as this connection lasts, so the
        // test can take one that SUCCEEDS before staging the one that must
        // not. It ends when the staged reconnection tears the socket down.
        let peer = tokio::spawn(async move {
            let (mut reader, mut writer) = peer_up(peer_side).await;
            while let Ok(Some(frame)) = reader.read_frame().await {
                let Ok(ControlMsg::ListProfiles { req_id }) = parse_control(&frame) else {
                    panic!("this peer serves catalog reads and nothing else");
                };
                writer
                    .write_frame(&Frame::control(&ControlMsg::ProfileList {
                        req_id,
                        profiles: vec![profile("starter-claude", "Claude Code")],
                    }))
                    .await
                    .unwrap();
            }
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        // A default is on record, so the refusal is not standing in for an
        // empty answer: this is exactly the pairing that must not be served.
        let status = harness
            .manager
            .status(local)
            .expect("the local host has an actor");
        harness
            .manager
            .remember_profile_default(
                &crate::manager::SessionClaim {
                    host: local,
                    incarnation: status.incarnation,
                    identity: Some("local-identity".to_string()),
                },
                "starter-claude",
            )
            .await
            .expect("the live connection may record a default");

        // The control, taken first and through the ordinary route: nothing
        // about this fixture makes the read fail on its own, so the refusal
        // below is the revalidation rather than the setup.
        let (status, value) = request(
            &harness,
            "GET",
            &format!("/api/hosts/{local}/profiles"),
            None,
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value["default_profile"], "starter-claude");

        let response = super::list_profiles_staged(
            std::sync::Arc::clone(&harness.state),
            local,
            // Unguarded: this is the pre-existing claim revalidation, which
            // must refuse on its own without a caller asserting anything.
            None,
            {
                let harness = &harness;
                async move {
                    // A real reconnection, in the window between the reads and
                    // the reply: the connection drops and comes back, so the
                    // catalog in hand belongs to a connection that is over.
                    let was = harness
                        .manager
                        .status(local)
                        .expect("the local host has an actor")
                        .incarnation;
                    harness.fleet.kill_connection(local);
                    await_reconnected(harness, local, was).await;
                }
            },
        )
        .await;
        assert_eq!(
            response.status(),
            axum::http::StatusCode::CONFLICT,
            "a catalog and a default that may describe different installs are not reported at all"
        );
        drop(harness);
        let _ = peer.await;
    }

    /// The connection guard itself refuses every claim that no longer
    /// describes this host — including the two an incarnation check alone
    /// would wave through.
    ///
    /// Spec: [`still_the_same_connection`] accepts only a claim matching both
    /// the current incarnation and the current identity; a superseded
    /// incarnation, a different identity, and an identity-less claim against
    /// an identified host are each a [`ErrorKind::Conflict`].
    ///
    /// Asserted directly on the guard because these are its dimensions rather
    /// than the endpoint's: an identity changing WITHOUT the client changing
    /// is not reachable through the harness at all, and it is the case that
    /// matters most — a claim reused across an adoption looks perfectly
    /// current if only the incarnation is compared.
    #[tokio::test]
    async fn the_connection_guard_refuses_a_claim_that_no_longer_describes_this_host() {
        let harness = rest_harness::helm_listing(Vec::new()).await;
        let local = rest_harness::local_id(&harness.store).await;
        let status = harness
            .manager
            .status(local)
            .expect("the local host has an actor");
        let current = crate::manager::SessionClaim {
            host: local,
            incarnation: status.incarnation,
            identity: Some("local-identity".to_string()),
        };

        super::still_the_same_connection(&harness.state, &current)
            .expect("the live connection is the one the reads were taken on");

        for stale in [
            // A reconnection: the client changed, so the token did too.
            crate::manager::SessionClaim {
                incarnation: current.incarnation - 1,
                ..current.clone()
            },
            // The same connection token against a different install — the
            // shape a reused or hand-built claim would have.
            crate::manager::SessionClaim {
                identity: Some("some-other-install".to_string()),
                ..current.clone()
            },
            // And an identity-less claim is not "do not care" either.
            crate::manager::SessionClaim {
                identity: None,
                ..current.clone()
            },
        ] {
            let error = super::still_the_same_connection(&harness.state, &stale)
                .expect_err("a claim that no longer describes this host must be refused");
            let refusal = error
                .downcast_ref::<crate::SupervisorError>()
                .expect("the refusal is typed, so the REST edge can map it");
            assert_eq!(
                refusal.kind,
                farhelm_proto::ErrorKind::Conflict,
                "a moved connection is a conflict the caller retries, not a 500"
            );
        }
    }

    /// An edit body as JSON, optionally carrying the two preconditions.
    ///
    /// Built as JSON and deserialized rather than constructed as a struct, in
    /// the tests that exercise the preconditions: the FIELD NAMES are the
    /// contract a browser is written against, and a struct literal would keep
    /// passing after a rename that broke every client.
    fn guarded_body(
        name: &str,
        expected_incarnation: Option<u64>,
        expected_definition: Option<&str>,
    ) -> serde_json::Value {
        let mut body = edit_body(name);
        if let Some(incarnation) = expected_incarnation {
            body["expected_incarnation"] = serde_json::json!(incarnation);
        }
        if let Some(definition) = expected_definition {
            body["expected_definition"] = serde_json::json!(definition);
        }
        body
    }

    /// The body text of a response, whatever shape it came back as.
    fn text(value: &serde_json::Value) -> String {
        value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string())
    }

    /// Drop `host`'s connection and return once it is CONNECTED AGAIN on a
    /// different one, answering with the new connection token.
    ///
    /// Waiting on the incarnation rather than on the host being seen DOWN, and
    /// the difference is a real flake rather than a style preference: the
    /// actor's status is a `watch` channel, which coalesces, so "it went down"
    /// is an EDGE a fast reconnect can hide entirely — a waiter that asked for
    /// a non-connected state can find `Connected` both times it looks and wait
    /// forever for a transition that already happened. The incarnation is a
    /// LEVEL: once it differs it stays differing, so this cannot miss it.
    ///
    /// Both halves are needed by the callers. The token must have MOVED (or
    /// there is no stale expectation to refuse) and the host must be CONNECTED
    /// (or the refusal under test would be the ordinary host-is-down one,
    /// which carries no marker).
    async fn await_reconnected(
        harness: &rest_harness::Harness,
        host: crate::store::HostId,
        previous: u64,
    ) -> u64 {
        let reconnected = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(status) = harness.manager.status(host)
                    && status.incarnation != previous
                    && status.state.is_connected()
                {
                    return status.incarnation;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        reconnected.expect("the host must come back on a new connection")
    }

    /// One handler response's status and body text — for the tests that call a
    /// staged handler directly, where there is no [`request`] to unpack it.
    async fn read_response(response: axum::response::Response) -> (axum::http::StatusCode, String) {
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("a handler response body");
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    /// An edit prepared against a connection that has been REPLACED while the
    /// request waited its turn is refused, and reaches no supervisor.
    ///
    /// Spec: `expected_incarnation` is checked again under the per-host lock,
    /// so a retarget, adoption, or reconnection landing between the first
    /// check and the lock produces a 409 carrying
    /// [`crate::precondition::INCARNATION_MARKER`].
    ///
    /// The second check is the one that is easy to talk yourself out of — the
    /// request already validated on the way in — and it is the one that
    /// matters: an edit can sit in that queue for as long as the edit ahead of
    /// it takes, which is a supervisor round trip, and a host is exactly as
    /// replaceable during it. Without it, the id resolves on the successor
    /// install and colliding starter profile ids make the edit SUCCEED there.
    ///
    /// Staged at [`update_profile_staged`]'s seam, because a lock acquisition
    /// is not something a test can schedule into. The whole call is wrapped in
    /// a timeout: a handler that dropped the second check would carry on to
    /// read a catalog from a reconnected host that answers no profile
    /// requests, and the honest report for that is a failed assertion rather
    /// than a hung suite.
    #[tokio::test]
    async fn an_edit_prepared_against_a_replaced_connection_is_refused_at_the_lock() {
        let harness = rest_harness::helm_listing(Vec::new()).await;
        let local = rest_harness::local_id(&harness.store).await;
        let prepared = harness
            .manager
            .status(local)
            .expect("the local host has an actor")
            .incarnation;

        let spec: super::ProfileSpec =
            serde_json::from_value(guarded_body("Renamed", Some(prepared), None))
                .expect("the body a browser sends");
        let response = tokio::time::timeout(
            Duration::from_secs(10),
            super::update_profile_staged(
                std::sync::Arc::clone(&harness.state),
                local,
                "p-1".to_string(),
                spec,
                {
                    let harness = &harness;
                    async move {
                        // The host is replaced while this request is queued.
                        harness.fleet.kill_connection(local);
                        await_reconnected(harness, local, prepared).await;
                    }
                },
            ),
        )
        .await
        .expect("the edit must be refused rather than carried on to the successor install");

        let (status, body) = read_response(response).await;
        assert_eq!(status, axum::http::StatusCode::CONFLICT);
        assert!(
            body.contains(crate::precondition::INCARNATION_MARKER),
            "the refusal must be branchable: {body}"
        );
        assert!(
            harness
                .manager
                .status(local)
                .expect("still registered")
                .incarnation
                > prepared,
            "the fixture must actually have replaced the connection, or this proves nothing"
        );
    }

    /// A stale editor's update is REFUSED rather than silently reverting the
    /// change it never saw, and nothing is forwarded.
    ///
    /// Spec: `expected_definition` is compared under the per-host lock against
    /// the same catalog read the identical-edit suppression uses; a mismatch is
    /// a 409 carrying [`crate::precondition::DEFINITION_MARKER`] and no
    /// `UpdateProfile` frame, while the fingerprint the catalog actually served
    /// lets the same edit through.
    ///
    /// An update replaces the WHOLE definition, so two editors open on one
    /// profile otherwise resolve by last-write-wins — and the loser's change
    /// disappears with nothing anywhere reporting that it did. That is the
    /// silent half of the problem: a rename and an invocation change made in
    /// two tabs leave one of them undone, and both users are told they saved.
    ///
    /// The accepted value comes from `GET /api/hosts/{id}/profiles`'s own
    /// `definitions` map rather than from a fingerprint the test computes,
    /// because that round trip IS the contract a client uses: read a value,
    /// hand it back unchanged.
    #[tokio::test]
    async fn a_stale_expected_definition_refuses_the_update_and_forwards_nothing() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let shared = CatalogPeer::new(vec![profile("p-1", "Before")]);
        shared.gate.notify_one();
        let peer = tokio::spawn(catalog_peer(peer_side, std::sync::Arc::clone(&shared)));

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        let uri = format!("/api/hosts/{local}/profiles/p-1");

        // What an editor is seeded with: the catalog, and the fingerprint of
        // each definition in it.
        let (status, view) = request(
            &harness,
            "GET",
            &format!("/api/hosts/{local}/profiles"),
            None,
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        let seeded = view["definitions"]["p-1"]
            .as_str()
            .expect("the catalog serves a fingerprint per profile")
            .to_string();

        // Somebody else's edit lands first. The editor above still holds the
        // fingerprint of what it loaded.
        let (status, _) = request(
            &harness,
            "POST",
            &uri,
            Some(guarded_body("Somebody Elses Change", None, None)),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(shared.forwards(), 1);

        let (status, body) = request(
            &harness,
            "POST",
            &uri,
            Some(guarded_body("Stale Editors Change", None, Some(&seeded))),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::CONFLICT);
        let body = text(&body);
        assert!(
            body.contains(crate::precondition::DEFINITION_MARKER),
            "a stale definition is its own kind of conflict, distinguishable from a moved \
             connection: {body}"
        );
        assert_eq!(
            shared.forwards(),
            1,
            "the refused update must not have reached the supervisor"
        );
        assert_eq!(
            shared.catalog()[0].name,
            "Somebody Elses Change",
            "and the durable definition is untouched"
        );

        // Re-read, reapply: the fingerprint the catalog serves NOW lets the
        // same edit through, which is what makes this a precondition rather
        // than a permanent refusal.
        let (_, view) = request(
            &harness,
            "GET",
            &format!("/api/hosts/{local}/profiles"),
            None,
        )
        .await;
        let current = view["definitions"]["p-1"]
            .as_str()
            .expect("a fingerprint per profile")
            .to_string();
        assert_ne!(
            current, seeded,
            "the definition moved, so its fingerprint did"
        );
        let (status, _) = request(
            &harness,
            "POST",
            &uri,
            Some(guarded_body("Stale Editors Change", None, Some(&current))),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(shared.forwards(), 2);
        assert_eq!(shared.catalog()[0].name, "Stale Editors Change");
        drop(harness);
        let _ = peer.await;
    }

    /// A DELETE names the connection it was prepared against in the QUERY
    /// STRING, and a stale one reaches no supervisor.
    ///
    /// Spec: `?expected_incarnation=` on the delete route behaves exactly as
    /// the body field does on the other two verbs — mismatch is a 409 with
    /// [`crate::precondition::INCARNATION_MARKER`] and nothing forwarded, a
    /// match deletes.
    ///
    /// The query string is where it rides because this verb has no body at
    /// all, and inventing one would make every existing caller send `{}` to
    /// keep working. Worth guarding despite being a delete — or rather because
    /// of it: deleting the wrong install's profile is the one outcome here
    /// that re-reading cannot undo.
    #[tokio::test]
    async fn a_delete_prepared_against_a_replaced_connection_is_refused() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let shared = CatalogPeer::new(vec![profile("p-1", "Doomed")]);
        shared.gate.notify_one();
        let peer = tokio::spawn(catalog_peer(peer_side, std::sync::Arc::clone(&shared)));

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        let current = harness
            .manager
            .status(local)
            .expect("the local host has an actor")
            .incarnation;

        let (status, body) = request(
            &harness,
            "DELETE",
            &format!(
                "/api/hosts/{local}/profiles/p-1?expected_incarnation={}",
                current - 1
            ),
            None,
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::CONFLICT);
        assert!(
            text(&body).contains(crate::precondition::INCARNATION_MARKER),
            "{body:?}"
        );
        assert_eq!(
            shared.requests(),
            Vec::<&str>::new(),
            "a delete for another install must not reach this supervisor at all"
        );

        let (status, _) = request(
            &harness,
            "DELETE",
            &format!("/api/hosts/{local}/profiles/p-1?expected_incarnation={current}"),
            None,
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(shared.requests(), vec!["delete"]);
        assert!(shared.catalog().is_empty());
        drop(harness);
        let _ = peer.await;
    }

    /// A create's and an update's replies carry the fingerprint a LATER
    /// catalog read serves for the same profile, byte for byte.
    ///
    /// Spec: the `fingerprint` field of a successful create, a forwarded
    /// update, and a suppressed (identical) update each equal that profile's
    /// entry in a subsequent `GET`'s `definitions` map.
    ///
    /// Without it a client is stuck between two bad states after every
    /// successful save, and both reach the user. It holds the NEW definition
    /// beside the OLD fingerprint, so reopening the row and saving again sends
    /// a value the helm no longer recognizes and the edit is refused as stale
    /// with nobody having raced it. And a freshly CREATED profile has no
    /// fingerprint at all, so its first edit cannot be guarded — the one edit
    /// most likely to follow immediately.
    ///
    /// Equality with the read is what the test is really about: two encodings
    /// that merely both existed would let the reply be self-consistent and
    /// useless. The suppressed update is included because an idempotent update
    /// is a successful update, and a client cannot be expected to discover
    /// that this one kind of success left it unable to guard the next one.
    #[tokio::test]
    async fn a_mutation_reply_carries_the_fingerprint_a_later_read_serves() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let shared = CatalogPeer::new(Vec::new());
        shared.gate.notify_one();
        let peer = tokio::spawn(catalog_peer(peer_side, std::sync::Arc::clone(&shared)));

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;

        /// This profile's fingerprint as the catalog READ serves it.
        async fn served(
            harness: &rest_harness::Harness,
            host: crate::store::HostId,
            id: &str,
        ) -> String {
            let (status, view) =
                request(harness, "GET", &format!("/api/hosts/{host}/profiles"), None).await;
            assert_eq!(status, axum::http::StatusCode::OK);
            view["definitions"][id]
                .as_str()
                .unwrap_or_else(|| panic!("the catalog serves a fingerprint for {id}: {view}"))
                .to_string()
        }

        let (status, created) = request(
            &harness,
            "POST",
            &format!("/api/hosts/{local}/profiles"),
            Some(edit_body("Nightly")),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        let minted = created["id"]
            .as_str()
            .expect("a create answers with the minted id")
            .to_string();
        let from_create = created["fingerprint"]
            .as_str()
            .expect("a create's reply carries the committed definition's fingerprint")
            .to_string();
        assert_eq!(
            from_create,
            served(&harness, local, &minted).await,
            "the fingerprint a create hands back must be the one the catalog will serve, or the \
             first edit of a new profile cannot be guarded"
        );

        // A forwarded update: the definition changed, so the fingerprint must
        // have too — and must still match what the catalog now serves.
        let uri = format!("/api/hosts/{local}/profiles/{minted}");
        let (status, updated) = request(&harness, "POST", &uri, Some(edit_body("Renamed"))).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        let from_update = updated["fingerprint"]
            .as_str()
            .expect("an update's reply carries one too")
            .to_string();
        assert_ne!(
            from_update, from_create,
            "a definition that changed has a different fingerprint, or the guard means nothing"
        );
        assert_eq!(from_update, served(&harness, local, &minted).await);

        // And the suppressed path — the same edit again, which reaches no
        // supervisor at all.
        let forwards = shared.forwards();
        let (status, suppressed) =
            request(&harness, "POST", &uri, Some(edit_body("Renamed"))).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            shared.forwards(),
            forwards,
            "the identical edit must have been suppressed, or this asserts nothing about that path"
        );
        assert_eq!(
            suppressed["fingerprint"].as_str(),
            Some(from_update.as_str()),
            "an idempotent update is a successful update, and leaves the caller able to guard the \
             next one"
        );

        // The reply's other fields are exactly where they always were: the
        // fingerprint is an additive sibling, not a reshaping.
        assert_eq!(suppressed["id"].as_str(), Some(minted.as_str()));
        assert_eq!(suppressed["name"], "Renamed");
        assert_eq!(suppressed["invocation"], "claude");
        assert_eq!(suppressed["agent_kind"], "claude");
        drop(harness);
        let _ = peer.await;
    }

    /// A guarded catalog READ whose connection has been replaced is refused
    /// with the same marker a guarded mutation gets.
    ///
    /// Spec: `GET /api/hosts/{id}/profiles?expected_incarnation=` refuses with
    /// [`crate::precondition::INCARNATION_MARKER`] both when the named
    /// connection is already gone at routing (no round trip) and when it is
    /// replaced while the read is in flight; without the parameter the same
    /// read is served.
    ///
    /// A read damages nothing on the far side, so the point is what the ANSWER
    /// gets filed under. A client that stores catalogs per (host, connection)
    /// sends only the host id, so a retarget between its dispatch and this
    /// routing hands it the successor's catalog to record under the
    /// predecessor's key — and every later use of that cache, including the
    /// fingerprint it guards its next edit with, is about the wrong install.
    ///
    /// The in-flight half is staged at [`list_profiles_staged`]'s seam and is
    /// where the marker matters most: the generic revalidation would refuse
    /// this too, but unmarked, dropping a guarded client into the branch it
    /// wrote for "the host is busy" rather than "re-read, you are on a
    /// different connection now".
    #[tokio::test]
    async fn a_guarded_catalog_read_prepared_against_a_replaced_connection_is_refused() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let shared = CatalogPeer::new(vec![profile("p-1", "Claude Code")]);
        shared.gate.notify_one();
        let peer = tokio::spawn(catalog_peer(peer_side, std::sync::Arc::clone(&shared)));

        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;
        let current = harness
            .manager
            .status(local)
            .expect("the local host has an actor")
            .incarnation;

        // The control first, while this connection is still the live one: an
        // unguarded read is served exactly as before. Taken BEFORE the staged
        // reconnection below, because the reconnected host is served by the
        // standalone scripted peer, which answers session lists and nothing
        // else — a catalog read against it would hang rather than assert.
        let (status, view) = request(
            &harness,
            "GET",
            &format!("/api/hosts/{local}/profiles"),
            None,
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(view["profiles"][0]["id"], "p-1");
        assert_eq!(shared.requests(), vec!["list"]);

        // Already stale when it arrives: refused before a round trip.
        let (status, body) = request(
            &harness,
            "GET",
            &format!(
                "/api/hosts/{local}/profiles?expected_incarnation={}",
                current - 1
            ),
            None,
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::CONFLICT);
        assert!(
            text(&body).contains(crate::precondition::INCARNATION_MARKER),
            "{body:?}"
        );
        assert_eq!(
            shared.requests(),
            vec!["list"],
            "a read for a connection this host has already left need not be asked for"
        );

        // Still current when it arrives, replaced before it can answer.
        let response = tokio::time::timeout(
            Duration::from_secs(10),
            super::list_profiles_staged(
                std::sync::Arc::clone(&harness.state),
                local,
                Some(current),
                {
                    let harness = &harness;
                    async move {
                        harness.fleet.kill_connection(local);
                        await_reconnected(harness, local, current).await;
                    }
                },
            ),
        )
        .await
        .expect("the read must be refused rather than hanging");
        let (status, body) = read_response(response).await;
        assert_eq!(status, axum::http::StatusCode::CONFLICT);
        assert!(
            body.contains(crate::precondition::INCARNATION_MARKER),
            "a guarded reader must be told its guard is what fired, not that the host is busy: \
             {body}"
        );
        drop(harness);
        let _ = peer.await;
    }

    /// A create carrying `expected_definition` is REFUSED rather than served
    /// with the precondition ignored.
    ///
    /// Spec: a 400 naming the field, and no supervisor round trip.
    ///
    /// There is no prior definition a create could be replacing, so the field
    /// can only be a client mistake — and the dangerous way to treat a
    /// precondition nobody checked is to accept it. A caller that believes it
    /// sent a guard behaves as if it has one. Refused locally, like every
    /// other ambiguous create body this API sees.
    #[tokio::test]
    async fn a_create_carrying_a_definition_precondition_is_refused_locally() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(rest_harness::silent_supervisor(peer_side));
        let harness = rest_harness::spliced_helm(client_side).await;
        let local = rest_harness::local_id(&harness.store).await;

        let (status, body) = request(
            &harness,
            "POST",
            &format!("/api/hosts/{local}/profiles"),
            Some(guarded_body(
                "Nightly",
                None,
                Some("n1:x;i1:y;k6:claude;r-;"),
            )),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
        assert!(
            text(&body).contains("expected_definition"),
            "the refusal must name the field to remove: {body:?}"
        );
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
