//! The helm: Farhelm's single control-plane process.
//!
//! Per SPEC.md, exactly one helm runs at a time. It connects to
//! supervisors (locally over their unix socket, remotely over the user's
//! own ssh running `farhelm internal stdio`), aggregates their sessions,
//! and serves the API over loopback HTTP/WS. The desktop app embeds it,
//! private to its window, through [`run_embedded`], the only way to start a
//! helm. It holds no authoritative session state — supervisors are the
//! authority.
//!
//! The loopback-only bind is enforced here — SPEC.md's security posture
//! says the helm refuses non-loopback addresses in v1, and this code simply
//! never binds anything else.
//!
//! ## The shape of the serving path (PLAN_M6.md item 5)
//!
//! Every request is answered against a FLEET, never against one
//! connection. `AppState` holds two things: the [`manager::ConnectionManager`],
//! which owns one connection actor per registered host and publishes each
//! host's live state, and the [`store::HelmStore`], which holds the
//! registry and the last-known session cache those actors drain into.
//!
//! Three consequences run through everything below:
//!
//! - **The session list is merged and served WHOLE.**
//!   [`aggregate::session_list`] reads every host's cached rows from
//!   helm.db plus the rows a connected host holds in the manager's memory
//!   when it has no identity to bind a cache write to, tags each row with
//!   its host, marks rows of non-connected hosts stale, and filters, sorts
//!   and counts the union in memory, cut at one fixed cap (see that
//!   module's docs and SPEC.md's Session list section).
//! - **Session operations route by owner.** [`sessions::route_session`]
//!   looks a session's host up in that same merged view and hands back the
//!   host's LIVE connection — or refuses, naming the state the host is
//!   actually in. Unreachable is not special-cased; it is one of several
//!   ways a host can fail to be connected, and all of them refuse
//!   identically.
//! - **Hosts are managed over REST.** [`hosts`] is the registry's own
//!   surface — add, retarget, remove, adopt, retry — and an ensure-hosts
//!   file ([`ensure`]) is the same registration path run once at startup.
//!
//! ## What M6.75 added (PLAN_M6_75.md item 5)
//!
//! Two things, and they are deliberately independent of each other:
//!
//! - **Clients CAN stop polling.** [`feed`] is a REVISION NUMBER and nothing
//!   else; every chokepoint that changes what a client could read bumps it,
//!   and only when something actually changed. The UI channel, the one
//!   listener a client sees, re-reads through the same GET handlers, so
//!   there is exactly one serving path and
//!   one set of consistency rules — see that module for the changed-only
//!   rule. M6.75 relayed the number to browsers over a WebSocket,
//!   `/api/events`; today the only client-facing listener is the UI channel
//!   ([`ui_channel`]), which re-runs its pushed collections' reads on every
//!   bump and pushes what changed. The maintainer decided on 2026-11-15 to
//!   remove `/api/events` once nothing in Farhelm called it.
//! - **Narrowing happens here, not in the browser.** The merged list takes
//!   SPEC.md's filter dimensions as query parameters and answers a
//!   filtered request with two counts (matching, and the fleet's own),
//!   because a client that filtered a list the cap had cut would hide
//!   matches beyond the cut while reporting a count that included them.
//!   Both counts are taken from the same in-memory view the rows come
//!   from, in the same request.
//!
//! M1's argv session flags (`--ssh`, `--cwd`, `--agent`, `--title`,
//! `--remote-farhelm`, `--remote-state-dir`) are gone in this same PR: the
//! registry and the create API are the mechanism now, and the last two live
//! on as per-host registry fields.
//!
//! ## What is left in this file
//!
//! The composition root, and nothing that answers a request. [`HelmArgs`]
//! and [`run_embedded`] are the process; [`AppState`] is the fleet the REST
//! handlers reach into; [`build_router`] is the one place the routes and the
//! middleware order are written down; [`http_error`] is the shared mapping
//! from a typed REST or supervisor error to a status and a body — the
//! failure path of the request/response routes, not a universal one, since
//! a terminal socket has already been upgraded by the time most of its
//! failures happen and some validation and middleware paths build their
//! responses directly. The handlers themselves live in [`sessions`],
//! [`uploads`], [`terminal`], and [`hosts`], behind the layers
//! [`middleware`] defines — split out because the serving path outgrew one
//! file, not because any seam between them is load-bearing.

use anyhow::Context;
use axum::{Router, response::IntoResponse, routing::get};

#[cfg(all(target_os = "linux", feature = "test-seams"))]
#[doc(hidden)]
pub use environment::podman_probe::probe_local_podman_with_test_storage;
use farhelm_proto::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

/// Shared machine execution routes for transport, provisioning and credentials.
mod access;
mod client;
mod downloads;
mod environment;
pub use client::{
    CreateExtras, Detach, DownloadGuard, ErrorOrigin, PeerHello, SessionListing, SupervisorClient,
    SupervisorError, SupervisorTransportError, TcpTunnelAccepted, TermDetachSignal, TermEvent,
    TermStream,
};

/// The merged multi-host session list, served whole — what
/// `GET /api/sessions` is built out of.
mod aggregate;

/// Answers to questions an agent asks from inside its own session, which
/// reach this process as upcalls on the supervisor connections it opened.
pub mod agent_requests;

/// An agent's create request (templates, then flags) resolved into a launch.
mod agent_launch;

mod askpass;
/// Device-secret enforcement for the in-process credentials the helm mints.
mod auth;
pub use askpass::{ASKPASS_JUMP_PROGRAM_NAME, ASKPASS_PROGRAM_NAME, run_askpass, run_askpass_jump};

mod browser;
pub use browser::LinkOpener;
pub use downloads::{FileOpener, NativeDownloads, cleanup_native_downloads};

/// The ensure-hosts file ([`HelmArgs::ensure_hosts`]): the JSON5 floor
/// under the registry, applied once before serving starts.
mod ensure;

/// Shared condition notices, initially stuck archived-checkout removal.
mod notices;

/// `POST /api/feedback` — sends the UI's feedback submission to the project's
/// feedback endpoint; the helm's explicit feedback connection carrying user text.
mod feedback;
mod ntfy;

/// `/api/credentials` — the named credential catalog: saved secrets can be
/// replaced but are never returned.
mod credentials;
/// `/api/tensorlake` and the Tensorlake API client: the account side of
/// Tensorlake sandbox hosts (the API key, Farhelm's own ssh key for
/// sandboxes, and the helm tag their names carry), plus the recipes under
/// `/api/tensorlake/recipes`.
mod tensorlake;

/// The changed-only fleet invalidation feed shared by the manager and REST
/// edge, which the UI channel re-projects on.
mod feed;

/// `/api/ui` — the GUI's one ordered channel: commands, replies and pushed
/// state, and the seat that makes the helm serve one GUI at a time.
mod ui_channel;

/// `/api/hosts` — the registry's REST surface, and the JSON shape a UI
/// renders a host chip from.
mod hosts;

/// The one SemVer release order Farhelm uses to call a build newer, and the
/// test for a development build that has no place in it.
///
/// Public for the desktop app's updater (`farhelm-desktop`'s `updater.rs`),
/// which decides whether an installed or released version is newer than the
/// running one by the same rule the host list uses for a host's build, rather
/// than keeping a second comparison that could drift from this one.
pub use hosts::{build_is_newer, is_development_build};

/// Whether this build carries the release mark (D13), which `build.rs` turns into the `farhelm_release_build` cfg
/// and whose module doc says where it comes from.
///
/// The mark picks the helm's default host payload source (`provisioning/payloads.rs`): a marked build of a release
/// version downloads from its own release on get.farhelm.io, a marked build of a development version refuses
/// (`UnreleasedPayloads`), and an unmarked build has none (`NoPayloads`).
///
/// Public because a cfg a build script sets applies to this crate alone: the `farhelm` binary cannot test
/// `farhelm_release_build` itself, and reading it here reports exactly the value the helm linked into the same binary
/// acts on.
pub const RELEASE_BUILD: bool = cfg!(farhelm_release_build);

/// Release-owned structured launch catalog and argv compiler.
mod launches;

/// Compose an agent launch from an agent type and its choices with this
/// helm's catalog, exactly as the launcher's create does: for callers that
/// drive a supervisor directly rather than through the helm's REST API (the
/// Rust end-to-end tests). A choice the catalog refuses is the error.
pub fn compile_agent_launch(
    selection: farhelm_proto::LaunchSelection,
) -> Result<farhelm_proto::SessionLaunch, String> {
    launches::compile(selection)
}

/// The per-host connection actors, their reconnect state machine, and the
/// cache refresh that rides them (PLAN_M6.md item 4).
///
/// Public because the desktop embedder drives one directly, and because the
/// real-transport tests in `farhelm`'s e2e suite are written against it.
pub mod manager;

/// The layers wrapped around the routes: the loopback-origin guard and the
/// build stamp, which every response passes through.
mod middleware;

/// Discovery-first supervisor setup, explicit update, and host-scoped run
/// progress (PLAN_M7.md item 6).
mod provisioning;
pub use provisioning::{LocalSupervisorDiscovery, discover_local_supervisor};
/// The release trust boundary shared with the desktop updater: the compiled-in
/// key ring, release-checksum verification, and purpose-bound verification for
/// other signed metadata. Keeping these rules together prevents a second
/// consumer from accepting signatures the helm would refuse.
pub use provisioning::{RELEASE_KEY_RING, verify_signed_metadata, verify_signed_sums};

/// The optional precondition a session create may carry — which connection
/// it was prepared against — so a create written for one install cannot
/// launch on another. The module docs say why.
mod precondition;
/// `/api/preferences` — the one client preference (list order, last
/// selection) the helm remembers for every client at once.
mod preferences;

/// The helm's durable announcement queue and shared shown history.
mod announcements;
/// The compiled release table is the queue's first producer; release policy
/// belongs here rather than in its storage, routes or clients.
mod release_announcements;

/// The rules both session caches share: the reply merge, creation order,
/// cap eviction, and the session id bound.
pub mod session_cache;
mod session_mute;

/// Acting agent requests waiting for the user's approval (see the module's
/// own docs).
mod approvals;
/// The session REST surface — the list, the owner-lookup routing behind
/// every operation on one session, and the handlers themselves.
mod sessions;
/// The launch templates API: the helm-owned catalog of named, partial
/// launcher edits (SPEC.md, Launch templates).
mod templates;
/// Workspace-trust policies required by create and Restart with dispatch.
mod workspace_trust;
/// The refusal of a YOLO launch on a host that asks before YOLO launches (see its own docs).
mod yolo_guard;

/// The ssh argv the remote transport is built out of, and the handshake
/// failure only that transport can explain.
mod ssh;
mod ssh_sign_in;

pub mod store;

/// The terminal WebSocket: the browser's end of an attachment.
mod terminal;

/// Desktop loopback tunnels and the consent-ordered lifetime of their listeners.
mod tunnels;

/// `farhelm helm checkout-config`: the stored checkout configuration
/// (working-copy root, post-clone command), its inheritance resolution,
/// and the offline CLI that edits it — backed by the never-create,
/// never-migrate opening mode R1.5 requires.
pub mod checkout_config;

/// One serving helm per state directory, through a lifetime flock.
mod ownership;
pub use ownership::{LOCK_NAME as HELM_LOCK_FILE, SERVING_CLAIM_WAIT as HELM_LOCK_WAIT};

/// Local sockets and routed child processes behind the connection manager's seam.
mod transport;

/// The systemd user unit remote provisioning writes, and the reading of
/// the systemd user manager's environment that provisioning and podman share.
pub(crate) mod units;

/// `POST /api/sessions/{id}/attachments` — the streaming attachment
/// upload.
mod uploads;

/// The scripted-fleet harness the REST tests in this crate stand a real
/// serving path up on. Test-only, but its own module because three test
/// modules share it.
#[cfg(test)]
mod rest_harness;

#[cfg(test)]
mod test_capture;

/// Golden JSON for the helm's session and host HTTP views.
#[cfg(test)]
mod http_contract_tests;

/// What a helm is started with: the arguments [`run_embedded`] takes.
///
/// Lives here, not in the desktop crate, so the fields and what the helm
/// does with them change together.
#[derive(Debug, Clone)]
pub struct HelmArgs {
    /// Loopback port for the API. The desktop app and the tests pass zero,
    /// so the kernel chooses a private port for each launch; only the e2e
    /// test `helm_bind_failure_happens_before_any_durable_setup` asks for a
    /// fixed (taken) one.
    pub port: u16,

    /// State directory. Holds helm.db, the helm's ownership lock, the ssh
    /// ControlMaster sockets, and — in the ordinary single-machine
    /// arrangement — the local supervisor's socket the reserved local host
    /// row is reached through.
    pub state_dir: PathBuf,
    /// Devbuilds seed an unset remote-update choice off before any fleet work starts.
    /// The same remote hosts may be in use from a released app.
    pub devbuild: bool,
    /// The version-matched Farhelm CLI that OpenSSH runs as the local helper. Test helms can omit interactive SSH.
    pub ssh_askpass_program: Option<PathBuf>,

    /// JSON5 file of hosts to guarantee are registered at startup:
    /// `{ hosts: [ { ssh, remote_farhelm?, remote_state_dir? } ] }`.
    ///
    /// An additive floor over helm.db, applied before serving begins and
    /// never consulted again — see `crate::ensure` for what it does and
    /// does not promise. Built for agent-driven testing, where a fleet has
    /// to exist before anything can drive it: the desktop app's agent
    /// builds pass one when asked to, and every other launch passes `None`.
    pub ensure_hosts: Option<PathBuf>,

    /// Directory holding published release files (`farhelm-<target>.tar.gz`)
    /// to use as "add host" provisioning payloads instead of downloading
    /// them. The desktop app fills it from `FARHELM_HELM_PAYLOAD_DIR`
    /// (`helm_payload_dir` there). Only read, so it may be
    /// read-only or shared: the materialized binaries go to a cache in the
    /// helm's own state directory. Farhelm does not verify files placed
    /// here — this directory was explicitly supplied by the operator, so its
    /// contents are trusted as given.
    pub payload_dir: Option<PathBuf>,

    /// TEST ONLY: the seconds between a connected host's backstop session
    /// polls, in place of `manager::REFRESH_INTERVAL`. Lets an end-to-end
    /// test push the poll out of reach, so that anything the helm still
    /// learns about promptly can only have arrived as a supervisor's change
    /// hint.
    ///
    /// No production launch sets it: the poll is the backstop for every
    /// change a hint misses, and a helm that polled rarely would show such a
    /// change that much later.
    pub backstop_refresh_secs: Option<u64>,
}

impl HelmArgs {
    /// Resolve the payload directory into the selection
    /// `production_payloads` acts on: the directory when one is given, and
    /// otherwise this build's default (D13).
    ///
    /// `pub(crate)` (F24, review round 1): `PayloadSelection` lives in the
    /// private `provisioning` module, so no caller outside this crate could
    /// ever name the type this method returns — a `pub` method returning an
    /// unnameable type is not a usable external contract, just a wider
    /// surface than the crate needs to expose.
    pub(crate) fn payload_selection(&self) -> provisioning::PayloadSelection {
        match &self.payload_dir {
            Some(dir) => provisioning::PayloadSelection::Directory(dir.clone()),
            None => provisioning::PayloadSelection::Default,
        }
    }
}

/// What the axum handlers share: the fleet, in its two halves.
///
/// The manager is authority for what each host is DOING right now (and
/// holds the only live connections); the store is authority for what the
/// registry says and for the last-known sessions every host's actor drains
/// into it. Authentication joins those durable credentials to the
/// process-local revocation channel. The request handlers reach for the
/// pieces they need, and none of them holds a connection of its own — see this
/// crate's docs for why the single-client `AppState` this replaced could
/// not survive multi-host.
struct AppState {
    manager: Arc<manager::ConnectionManager>,
    store: store::HelmStore,
    /// The client security boundary: the credentials this process minted.
    auth: auth::AuthState,
    /// Order trash observations across clients without holding the host's
    /// session/cache or provisioning locks during remote work.
    trash_requests: Arc<notices::TrashRequests>,
    /// The one in-flight provisioning authority shared by every browser.
    provisioning: Arc<provisioning::ProvisioningService>,
    /// Own the event-driven updater independently of any connected interface.
    /// Dropping application state requests cancellation of its listener.
    _automatic_updates: provisioning::AutomaticUpdates,
    /// A desktop-only capability, supplied by the window that embeds this
    /// helm; tests may leave it `None`.
    link_opener: Option<LinkOpener>,
    /// Session-owned native TCP listeners; shutdown closes accepted relays too.
    tunnels: tunnels::TunnelManager,
    /// Native saves are an embedded-app capability, never a server-helm flag.
    /// The desktop resolves its account's Downloads folder at startup; tests
    /// inject a private directory without changing the process environment.
    downloads_dir: Option<PathBuf>,
    /// Receives only a newly published, allowlisted save. Separate from the
    /// hardened HTTP(S) opener so local files never widen that URL boundary.
    file_opener: Option<FileOpener>,
    /// Where `POST /api/feedback` sends the user's feedback, and with which
    /// client. Always the production endpoint outside tests; a field rather
    /// than a constant so a test helm can point it at a stand-in server it
    /// started itself (see `feedback.rs`).
    feedback: feedback::FeedbackForwarder,
    /// The manager's fleet sender, also used by explicit test-send requests.
    ntfy: ntfy::Alerts,
    /// The Tensorlake account: stored API key, API client and Farhelm's ssh
    /// key. A field rather than a global so each test helm has its own, and
    /// so a test can put a stub API behind it.
    tensorlake: tensorlake::Tensorlake,
    /// The manager's exact Podman service, cloned for provider-owned Containers
    /// HTTP and safe tunnel projection. Lifecycle/create/status always dispatch
    /// through manager.providers(); injected assemblies may omit this handle.
    podman: Option<Arc<environment::podman::Podman>>,
    /// Place probes are process-local observations and never gate reading Hosts.
    /// Hosts entries show them, so the constructor points them at the fleet feed.
    podman_places: Arc<environment::podman_probe::PodmanPlaces>,
    /// Acting agent requests waiting for the user's answer (see
    /// `approvals`). A field rather than a global so each helm in a test
    /// process has its own table, and so a test can shorten the wait.
    approvals: approvals::Approvals,
    /// Questions for this helm only, shared with the connection attempts that own their replies.
    ssh_questions: Arc<askpass::Questions>,
}

impl AppState {
    /// Both providers' artifact tasks and Settings projections use this one
    /// registry; its legacy Tensorlake owner does not create another engine.
    pub(crate) fn recipe_builds(&self) -> &Arc<tensorlake::build::RecipeBuilds> {
        self.tensorlake.builds()
    }
    /// The shared state as production builds it: the two halves of the
    /// fleet, provisioning authority, and every bound/cache at its default.
    ///
    /// A constructor rather than a literal at each call site so the defaults
    /// live in ONE place — the test harness stands this up too, and a
    /// harness that quietly used its own bounds or caches would be testing
    /// something the product does not do.
    ///
    /// `tensorlake` is built by the caller, before the manager: the manager's
    /// actors ask it about Tensorlake hosts' sandboxes from their first
    /// attempt, so both must hold the same service.
    fn new(
        manager: Arc<manager::ConnectionManager>,
        store: store::HelmStore,
        tensorlake: tensorlake::Tensorlake,
        podman: Option<Arc<environment::podman::Podman>>,
        state_dir: PathBuf,
        sources: provisioning::ProductionProvisioning,
    ) -> anyhow::Result<AppState> {
        let provisioning = provisioning::ProvisioningService::production(
            store.clone(),
            Arc::clone(&manager),
            state_dir,
            sources,
        )?;
        Ok(AppState {
            podman,
            tensorlake,
            ..Self::with_provisioning(manager, store, provisioning)
        })
    }

    /// Assemble state with an injected provisioning service. The ordinary
    /// constructor owns production wiring; this seam lets provisioning tests
    /// keep every path and external action isolated.
    fn with_provisioning(
        manager: Arc<manager::ConnectionManager>,
        store: store::HelmStore,
        provisioning: Arc<provisioning::ProvisioningService>,
    ) -> AppState {
        let tensorlake = tensorlake::Tensorlake::detached_with_delivery(
            store.clone(),
            provisioning.credential_delivery(),
        );
        // Hosts entries carry the Podman check (`container_check`), so its
        // changes must reach the feed the hosts collection is re-read on. The
        // cache predates the manager, so this is the first point both exist.
        let podman_places = provisioning.credential_delivery().places();
        podman_places.publish_to(Arc::clone(manager.events()));
        AppState {
            podman_places,
            _automatic_updates: provisioning::AutomaticUpdates::start(&provisioning),
            ntfy: manager.ntfy.clone(),
            // No state directory reaches this seam, so no ssh key and no
            // API; `new` replaces it with the production service.
            podman: None,
            tensorlake,
            manager,
            store,
            auth: auth::AuthState::new(),
            trash_requests: Arc::default(),
            provisioning,
            link_opener: None,
            tunnels: tunnels::TunnelManager::default(),
            downloads_dir: None,
            file_opener: None,
            // Test-built helms never reach the production endpoint; a
            // feedback test injects its own stand-in instead.
            #[cfg(not(test))]
            feedback: feedback::FeedbackForwarder::production(),
            #[cfg(test)]
            feedback: feedback::FeedbackForwarder::offline(),
            approvals: approvals::Approvals::new(farhelm_proto::approvals::APPROVAL_WAIT),
            ssh_questions: Arc::default(),
        }
    }
}

/// Assemble the API and WebSocket routes that share the helm's application
/// state and security boundary.
///
/// The control-plane routes are assembled and protected as one group, so no
/// future route has to remember whether it belongs inside authentication.
/// Ordinary state changes also acquire helm ownership here, before argument
/// extraction. Add a route to the normal group unless its cancellation is part
/// of the contract documented beside the explicit exceptions below.
fn api_router(state: Arc<AppState>) -> Router {
    let protected = Router::new()
        .route(
            "/api/sessions",
            get(sessions::list_sessions).post(sessions::create_session),
        )
        .route("/api/launch-catalog", get(sessions::launch_catalog))
        .route(
            "/api/environment-sessions",
            axum::routing::post(sessions::create_environment_session),
        )
        .route(
            "/api/sandbox-sessions",
            axum::routing::post(sessions::create_sandbox_session),
        )
        .route("/api/notices", get(notices::get))
        .route("/api/notices/marks", axum::routing::post(notices::marks))
        .route("/api/sessions/{id}/tunnels", get(tunnels::list_tunnels))
        .route(
            "/api/sessions/{id}/tunnels/{tunnel_id}",
            axum::routing::delete(tunnels::close_tunnel),
        )
        .route(
            "/api/checkout-trash/delete",
            axum::routing::post(sessions::delete_checkout_trash),
        )
        .route("/api/templates", get(templates::list_templates))
        .route(
            "/api/templates/{name}",
            axum::routing::put(templates::put_template).delete(templates::delete_template),
        )
        .route("/api/launch-history", get(sessions::launch_history))
        .route(
            "/api/environment-launch-history",
            get(sessions::environment_launch_history),
        )
        .route(
            "/api/sessions/{id}/restart",
            axum::routing::post(sessions::restart_session),
        )
        .route(
            "/api/sessions/{id}/rename",
            axum::routing::post(sessions::rename_session),
        )
        .route(
            "/api/sessions/{id}/replace",
            axum::routing::post(sessions::replace_session),
        )
        .route(
            "/api/sessions/{id}/stop-and-suspend",
            axum::routing::post(sessions::stop_and_suspend),
        )
        .route(
            "/api/sessions/{id}",
            get(sessions::get_session).delete(sessions::delete_session),
        )
        .route(
            "/api/sessions/{id}/seen",
            axum::routing::put(sessions::mark_seen),
        )
        .route(
            "/api/sessions/{id}/pin",
            axum::routing::put(sessions::set_pin),
        )
        .route(
            "/api/sessions/{id}/mute",
            axum::routing::put(sessions::set_mute),
        )
        .route(
            "/api/sessions/{id}/pin-order",
            axum::routing::put(sessions::move_pin),
        )
        .route(
            "/api/sessions/{id}/group",
            axum::routing::put(sessions::groups::set_group),
        )
        .route(
            "/api/session-groups",
            axum::routing::post(sessions::groups::create_group),
        )
        .route(
            "/api/session-groups/{id}",
            axum::routing::patch(sessions::groups::update_group).delete(sessions::groups::ungroup),
        )
        .route(
            "/api/session-groups/{id}/order",
            axum::routing::put(sessions::groups::move_group),
        )
        .route(
            "/api/sessions/{id}/notifications/read",
            axum::routing::put(sessions::mark_notifications_read),
        )
        .route(
            "/api/sessions/{id}/notifications/cleared",
            axum::routing::put(sessions::clear_notifications),
        )
        .route("/api/sessions/{id}/term", get(terminal::term_ws))
        // Host management (PLAN_M6.md item 5). The two verb routes are
        // shaped like the session verbs (`/stop`, `/rename`) rather than as
        // PATCHes, for the reason `rename_session` records: each changes
        // exactly one thing, and naming the thing is clearer than inventing
        // a partial-update shape this API has nowhere else.
        .route("/api/hosts", get(hosts::list_hosts).post(hosts::add_host))
        .route("/api/recipe-places", get(hosts::recipe_places))
        .route(
            "/api/recipes/{name}/rebuild",
            axum::routing::post(hosts::rebuild_recipe),
        )
        .route(
            "/api/container-orphans",
            get(environment::podman::list_leftovers).delete(environment::podman::delete_leftover),
        )
        // Provisioning is separate from registry management: probe is
        // discovery-first. It changes nothing when no supervisor answers, and
        // registers the host (on a helm-owned task) when one does; provision,
        // update and uninstall return run identities whose state is re-read
        // after feed bumps. Their in-flight exclusion lives in AppState, not in one
        // browser's operation lock.
        .route(
            "/api/hosts/probe",
            axum::routing::post(provisioning::probe_host),
        )
        .route("/api/hosts/{id}", axum::routing::delete(hosts::remove_host))
        .route(
            "/api/hosts/{id}/podman-probe",
            get(hosts::podman_probe).post(hosts::refresh_podman_probe),
        )
        .route(
            "/api/hosts/{id}/uninstall",
            axum::routing::post(provisioning::uninstall_host),
        )
        .route(
            "/api/hosts/{id}/provisioning",
            get(provisioning::provisioning_state),
        )
        .route("/api/provisioning", get(provisioning::provisioning_runs))
        .route(
            "/api/hosts/{id}/destination",
            axum::routing::post(hosts::set_destination),
        )
        .route(
            "/api/hosts/{id}/alias",
            axum::routing::post(hosts::set_alias),
        )
        .route(
            "/api/hosts/{id}/appearance",
            axum::routing::post(hosts::set_appearance),
        )
        // Configuration stays editable without a host connection. This reply
        // exposes the folder override only; host hook overrides stay private.
        .route(
            "/api/hosts/{id}/checkout-root",
            get(checkout_config::get_host_checkout_root)
                .put(checkout_config::put_host_checkout_root),
        )
        .route(
            "/api/hosts/{id}/yolo-without-asking",
            axum::routing::post(hosts::set_yolo_without_asking),
        )
        .route(
            "/api/hosts/{id}/own-sessions-without-asking",
            axum::routing::post(hosts::set_own_sessions_without_asking),
        )
        .route(
            "/api/hosts/{id}/commands-without-asking",
            axum::routing::post(hosts::set_commands_without_asking),
        )
        .route(
            "/api/hosts/{id}/links-without-asking",
            axum::routing::post(hosts::set_links_without_asking),
        )
        .route(
            "/api/hosts/{id}/local-links-without-asking",
            axum::routing::post(hosts::set_local_links_without_asking),
        )
        .route("/api/approvals", get(approvals::list_approvals))
        .route(
            "/api/approvals/{id}",
            axum::routing::post(approvals::answer_approval),
        )
        .route(
            "/api/hosts/{id}/adopt",
            axum::routing::post(hosts::adopt_host),
        )
        .route(
            "/api/hosts/{id}/take-over",
            axum::routing::post(hosts::take_over_host),
        )
        // The shared client preference (SPEC.md, Session list).
        .route(
            "/api/preferences",
            get(preferences::get_preferences).put(preferences::put_preferences),
        )
        .route("/api/announcements", get(announcements::get))
        .route(
            "/api/announcements/shown",
            axum::routing::post(announcements::shown),
        )
        // Separate explicit saves expose only global checkout configuration;
        // the host folder has its own route above, with no hook override.
        .route(
            "/api/checkout-root",
            get(checkout_config::get_checkout_root).put(checkout_config::put_checkout_root),
        )
        .route(
            "/api/checkout-post-clone",
            get(checkout_config::get_checkout_post_clone)
                .put(checkout_config::put_checkout_post_clone),
        )
        .route(
            "/api/checkout-claude-trust",
            get(checkout_config::get_claude_trust).put(checkout_config::put_claude_trust),
        )
        .route(
            "/api/checkout-spare",
            get(checkout_config::get_checkout_spare).put(checkout_config::put_checkout_spare),
        )
        .route(
            "/api/checkout-agent-trust/{harness}",
            get(checkout_config::get_agent_trust).put(checkout_config::put_agent_trust),
        )
        // The GUI's channel (see `ui_channel`). A WebSocket upgrade, so a GET
        // that `own_state_change` runs inline; the commands it carries go
        // back through this same router, owned or cancellable per route.
        .route(
            farhelm_proto::ui_channel::UI_CHANNEL_PATH,
            get(ui_channel::ui_ws),
        )
        .route(
            "/api/ntfy-alerts",
            axum::routing::get(ntfy::get)
                .put(ntfy::put)
                .layer(axum::extract::DefaultBodyLimit::max(16 * 1024)),
        )
        .route("/api/ntfy-alerts/test", axum::routing::post(ntfy::test))
        .route(
            "/api/tensorlake",
            axum::routing::get(tensorlake::get)
                .put(tensorlake::put)
                .layer(axum::extract::DefaultBodyLimit::max(16 * 1024)),
        )
        .route(
            "/api/tensorlake/test",
            axum::routing::post(tensorlake::test),
        )
        .route(
            "/api/credentials",
            get(credentials::list).post(credentials::create).layer(
                axum::extract::DefaultBodyLimit::max(credentials::BODY_LIMIT),
            ),
        )
        .route(
            "/api/credentials/{name}",
            axum::routing::put(credentials::save)
                .delete(credentials::delete)
                .layer(axum::extract::DefaultBodyLimit::max(
                    credentials::BODY_LIMIT,
                )),
        )
        .route("/api/tensorlake/orphans", get(tensorlake::orphans::list))
        .route(
            "/api/tensorlake/orphans/{id}",
            axum::routing::delete(tensorlake::orphans::delete),
        )
        // Creating Farhelm's ssh key is its own explicit request, so merely
        // reading the settings never writes a credential.
        .route(
            "/api/tensorlake/ssh-key",
            axum::routing::post(tensorlake::create_ssh_key),
        )
        // Tensorlake recipes. The write bodies can carry endpoint tokens and
        // up to 500 package names, hence the explicit limit.
        .route(
            "/api/tensorlake/recipes",
            get(tensorlake::recipes::list_recipes)
                .post(tensorlake::recipes::create_recipe)
                .layer(axum::extract::DefaultBodyLimit::max(
                    tensorlake::recipes::BODY_LIMIT,
                )),
        )
        .route(
            "/api/tensorlake/recipes/{name}",
            axum::routing::put(tensorlake::recipes::update_recipe)
                .delete(tensorlake::recipes::delete_recipe)
                .layer(axum::extract::DefaultBodyLimit::max(
                    tensorlake::recipes::BODY_LIMIT,
                )),
        )
        // Build, Rebuild and Retry of a recipe's snapshot. Starts (or joins)
        // a build that runs on its own task; the reply does not wait for it.
        .route(
            "/api/tensorlake/recipes/{name}/build",
            axum::routing::post(tensorlake::recipes::build_recipe),
        )
        // An unknown API path is an authenticated 404 for every method, so a
        // removed route never answers with some other layer's status.
        .route("/api/{*rest}", axum::routing::any(api_not_found))
        // Apply only to the normal group before merging the exceptions. Reads
        // and their implicit HEAD companions pass through this layer inline.
        .route_layer(axum::middleware::from_fn(middleware::own_state_change));

    // Cancellation is an explicit route contract, not the default for a new
    // mutation. Keep these outside ownership, then authenticate both groups
    // together so an exception never becomes an authentication exemption.
    let cancellable = Router::new()
        // Admission may wait on the consent gate. A caller leaving before
        // publication must not later create a listener or open a browser.
        .route(
            "/api/sessions/{id}/tunnels/open",
            axum::routing::post(tunnels::open_tunnel),
        )
        // Read-only POSTs retain cancellable lookup/remote work. A started
        // trash-notice commit owns its narrow guards through DB completion.
        // Browse may refine suggestion history atomically, but that is best effort and
        // repaired by the next browse/create, not an accepted mutation tail.
        .route(
            "/api/checkout-trash/list",
            axum::routing::post(sessions::list_checkout_trash),
        )
        .route(
            "/api/browse-directory",
            axum::routing::post(sessions::browse_directory),
        )
        .route(
            "/api/github-checkout-preview",
            axum::routing::post(sessions::github_checkout_preview),
        )
        .route(
            "/api/github-repositories",
            axum::routing::post(sessions::github_repositories),
        )
        .route("/api/ssh-questions", get(askpass::list))
        .route(
            "/api/ssh-sign-in-completions",
            get(provisioning::ssh_sign_in_completions),
        )
        // Cancellation dispatch is synchronous; the attempt owns cleanup after the signal.
        .route(
            "/api/ssh-sign-in-drafts/{attempt}/cancel",
            axum::routing::post(provisioning::cancel_ssh_sign_in),
        )
        // After the bounded body is read, validation and the one-shot reply run in one poll. The SSH attempt
        // owns forwarding or cancellation, so the route has no suspended mutation to hand to an owned task.
        .route(
            "/api/ssh-questions/{id}",
            axum::routing::post(askpass::reply)
                .layer(axum::extract::DefaultBodyLimit::max(8 * 1024)),
        )
        // Tabs issue one host request with no local follow-through; #1404
        // deliberately retained their cancellable request lifetime. Stop sits
        // here too, but its handler owns its work: it runs under the host
        // reply bound (`sessions::within_host_reply_bound`), whose task
        // finishes the Stop whether or not the caller still waits.
        .route(
            "/api/sessions/{id}/stop",
            axum::routing::post(sessions::stop_session),
        )
        .route(
            "/api/sessions/{id}/tabs",
            axum::routing::post(sessions::open_tab),
        )
        .route(
            "/api/sessions/{id}/tabs/{tab_id}",
            axum::routing::delete(sessions::close_tab),
        )
        // Retry's lookup is cancellable; once dispatch mutates the actor map,
        // its start gate and event bump complete without another await.
        .route(
            "/api/hosts/{id}/retry",
            axum::routing::post(hosts::retry_host),
        )
        // Retry-delete, Suspend and Resume sit here too, but their handlers own
        // their work: each runs it on a task of its own under the host reply
        // bound (`sessions::within_host_reply_bound_saying`), which finishes
        // it whether or not the caller still waits.
        .route(
            "/api/hosts/{id}/retry-delete",
            axum::routing::post(sessions::retry_environment_delete),
        )
        .route(
            "/api/hosts/{id}/suspend",
            axum::routing::post(tensorlake::lifecycle::suspend),
        )
        .route(
            "/api/hosts/{id}/resume",
            axum::routing::post(tensorlake::lifecycle::resume),
        )
        // ADD consumes its plan before its owned service operation; UPDATE's
        // bodyless planner first reads/refuses a host outside ownership. Keep
        // those admission boundaries and the service's distinct panic mapping.
        .route(
            "/api/hosts/provision",
            axum::routing::post(provisioning::provision_host),
        )
        .route(
            "/api/hosts/take-over",
            axum::routing::post(provisioning::take_over_discovered_host),
        )
        .route(
            "/api/hosts/{id}/update",
            axum::routing::post(provisioning::update_host),
        )
        // Only forwarding is owned. Its outer failure log is still skipped
        // when the caller disappears; owning the whole route would change it.
        .route(
            "/api/feedback",
            axum::routing::post(feedback::send_feedback).layer(
                axum::extract::DefaultBodyLimit::max(feedback::MAX_BODY_BYTES),
            ),
        );
    let protected = protected
        .merge(cancellable)
        // Applied to the ROUTES already assembled above.
        .route_layer(axum::middleware::from_fn_with_state(
            Arc::clone(&state),
            auth::require_device_session,
        ));
    // Uploads and file operations are cancellable exceptions with their own
    // body limits, so they sit outside the ordinary group: dropping an upload's
    // handler aborts the upstream transfer, stat is read-only, and dropping a
    // download must abort upstream and remove unfinished native staging.
    let transfers = Router::new()
        .route(
            "/api/sessions/{id}/attachments",
            // SPEC.md's "no size cap in v1" is a promise about memory, not
            // axum's 2 MiB default. Other small control messages retain it.
            axum::routing::post(uploads::upload_attachment)
                .layer(axum::extract::DefaultBodyLimit::disable()),
        )
        .route(
            "/api/sessions/{id}/files/stat",
            axum::routing::post(downloads::stat_file),
        )
        .route(
            "/api/sessions/{id}/files/download",
            axum::routing::post(downloads::download_file),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            Arc::clone(&state),
            auth::require_device_session,
        ));

    protected.merge(transfers).with_state(state)
}

/// Return the ordinary status for an authenticated but unknown API path.
///
/// This handler intentionally defines no response-body schema. Its job is to
/// keep API namespace misses inside the protected control-plane router.
async fn api_not_found() -> axum::http::StatusCode {
    axum::http::StatusCode::NOT_FOUND
}

#[cfg(test)]
mod request_ownership_tests {
    use axum::body::{Body, Bytes};
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;
    use tokio::sync::{mpsc, oneshot};
    use tower::ServiceExt;

    /// The real authenticated router owns parsing before the trust handler
    /// starts. A body parked at EOF separates that guarantee from the former
    /// per-handler wrapper, which could not protect extraction.
    #[farhelm_testtrace::test]
    async fn authenticated_mutation_finishes_parsing_after_its_caller_is_cancelled() {
        let harness = crate::rest_harness::idle_helm().await;
        assert!(
            !harness
                .store
                .checkout_config_snapshot(None)
                .await
                .unwrap()
                .claude_trust
        );
        let mut events = harness.manager.events().subscribe();
        let before = *events.borrow_and_update();
        let (send, receive) = mpsc::channel::<Result<Bytes, std::io::Error>>(2);
        send.send(Ok(Bytes::from_static(br#"{"enabled":true}"#)))
            .await
            .unwrap();
        let (at_eof, waiting) = oneshot::channel();
        // All valid JSON bytes are supplied first. The named gate means only
        // EOF remains; a cancelled/incomplete body cannot masquerade as parsing
        // that was retained by the helm. Sender drop releases it on test failure.
        let stream = futures_util::stream::unfold(
            (receive, Some(at_eof), false),
            |(mut receive, mut at_eof, delivered)| async move {
                if delivered && let Some(at_eof) = at_eof.take() {
                    let _ = at_eof.send(());
                }
                receive
                    .recv()
                    .await
                    .map(|bytes| (bytes, (receive, at_eof, true)))
            },
        );
        let request = Request::builder()
            .method("PUT")
            .uri("/api/checkout-claude-trust")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(Body::from_stream(stream))
            .unwrap();
        let caller = tokio::spawn(harness.router().oneshot(request));
        tokio::time::timeout(Duration::from_secs(5), waiting)
            .await
            .expect("the authenticated extractor must wait for EOF")
            .expect("the extractor must report its EOF boundary");
        assert!(
            !harness
                .store
                .checkout_config_snapshot(None)
                .await
                .unwrap()
                .claude_trust
        );
        caller.abort();
        let cancelled = tokio::time::timeout(Duration::from_secs(5), caller)
            .await
            .expect("the HTTP waiter must stop after abort")
            .expect_err("the gated caller must be cancelled");
        assert!(cancelled.is_cancelled());
        assert!(!send.is_closed(), "the helm must still own the extractor");
        drop(send);
        // A feed event is the operation's real completion oracle. Unrelated
        // fleet events cannot satisfy it without the persisted opt-in as well.
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                events
                    .changed()
                    .await
                    .expect("the fixture feed remains live");
                if harness
                    .store
                    .checkout_config_snapshot(None)
                    .await
                    .unwrap()
                    .claude_trust
                {
                    assert!(*events.borrow_and_update() > before);
                    break;
                }
            }
        })
        .await
        .expect("the disconnected save must persist and announce its opt-in");
    }

    /// Authentication must reject before ownership or parsing. A body that
    /// never becomes ready proves a 401 cannot depend on extractor completion.
    #[farhelm_testtrace::test]
    async fn unauthenticated_mutation_is_refused_without_polling_its_body() {
        let harness = crate::rest_harness::idle_helm().await;
        let polled = Arc::new(AtomicBool::new(false));
        let body_polled = Arc::clone(&polled);
        let body = Body::from_stream(futures_util::stream::poll_fn(move |_| {
            body_polled.store(true, Ordering::SeqCst);
            std::task::Poll::<Option<Result<Bytes, std::io::Error>>>::Pending
        }));
        let response = tokio::time::timeout(
            Duration::from_secs(5),
            harness.unauthenticated_router().oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/checkout-claude-trust")
                    .header("host", "127.0.0.1:7433")
                    .header("content-type", "application/json")
                    .body(body)
                    .unwrap(),
            ),
        )
        .await
        .expect("authentication must not wait for the body")
        .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!polled.load(Ordering::SeqCst));
        assert!(
            !harness
                .store
                .checkout_config_snapshot(None)
                .await
                .unwrap()
                .claude_trust
        );
    }
}

/// Compose the API and the shared middleware from one serving mode.
///
/// The helm serves no page: every path outside `/api` is a 404 from the
/// router itself, after the origin guard.
///
/// Tests drive this same composition in-process against a scripted fleet,
/// including the origin guard and headers, rather than exercising handlers
/// in isolation and silently skipping the security boundary.
fn build_router(state: Arc<AppState>, port: u16) -> Router {
    api_router(state)
        .layer(axum::middleware::from_fn(
            move |req: axum::extract::Request, next: axum::middleware::Next| {
                middleware::require_loopback_origin(port, req, next)
            },
        ))
        // OUTSIDE the origin guard, and that placement is the whole point:
        // layers apply outside-in, so this one decorates every response the
        // stack can produce — including the guard's own 403, which returns
        // before anything inner runs. "Every reply carries the stamp" has to
        // mean every reply, refusals included.
        .layer(axum::middleware::from_fn(middleware::stamp_build))
}

/// The router's outer boundary: what answers outside the API, and that the
/// build stamp reaches every reply.
#[cfg(test)]
mod router_boundary_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode, header};
    use tower::ServiceExt;

    /// A loopback API request carrying `secret`. Namespace-miss tests must
    /// pass authentication so they prove routing reaches the protected 404
    /// rather than merely observing a 401.
    fn authenticated_api_request(method: Method, path: &str, secret: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, "127.0.0.1:7433")
            .header(header::AUTHORIZATION, format!("Bearer {secret}"))
            .body(Body::empty())
            .unwrap()
    }

    /// The helm serves no page at `/`, and the browser sign-in route is gone.
    ///
    /// Why it matters: the browser UI was removed together with its sign-in;
    /// a page or a token exchange answering again would bring back a surface
    /// that nothing maintains.
    #[farhelm_testtrace::test]
    async fn the_helm_serves_no_page_and_no_sign_in() {
        let harness = rest_harness::idle_helm().await;
        let secret = harness.state.auth.mint_device().unwrap();
        let router = build_router(Arc::clone(&harness.state), 7433);
        let root = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(header::HOST, "127.0.0.1:7433")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(root.status(), StatusCode::NOT_FOUND);
        let sign_in = router
            .oneshot(authenticated_api_request(
                Method::POST,
                "/api/auth/token",
                &secret,
            ))
            .await
            .unwrap();
        assert_eq!(sign_in.status(), StatusCode::NOT_FOUND);
    }

    /// Spec: an unknown path under `/api` is an authenticated 404 for every
    /// method. This keeps a removed GET route from looking like something
    /// else, and keeps non-GET misses from inheriting another layer's 405.
    #[farhelm_testtrace::test]
    async fn api_misses_are_404_for_every_method() {
        let harness = rest_harness::idle_helm().await;
        let secret = harness.state.auth.mint_device().unwrap();
        let router = build_router(Arc::clone(&harness.state), 7433);
        for method in [
            Method::GET,
            Method::HEAD,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ] {
            let response = router
                .clone()
                .oneshot(authenticated_api_request(
                    method.clone(),
                    "/api/removed-route",
                    &secret,
                ))
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::NOT_FOUND,
                "wrong status for {method}"
            );
        }
    }

    /// Spec: [`middleware::stamp_build`] is layered OUTSIDE the whole router
    /// (see [`build_router`]'s own comment on why), so it reaches an ordinary
    /// successful response.
    #[farhelm_testtrace::test]
    async fn successful_response_carries_build_stamp_header() {
        let harness = rest_harness::idle_helm().await;
        let secret = harness.state.auth.mint_device().unwrap();
        let response = build_router(Arc::clone(&harness.state), 7433)
            .oneshot(authenticated_api_request(
                Method::GET,
                "/api/hosts",
                &secret,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        // Spelled out rather than taken from the constant: this pins the
        // header's wire name, so an edit to the shared constant fails here
        // instead of silently renaming what clients look for.
        assert_eq!(
            response.headers().get("x-farhelm-build").unwrap(),
            env!("CARGO_PKG_VERSION")
        );
    }

    /// Spec: [`middleware::require_loopback_origin`] rejects a request whose
    /// `Host` names a foreign origin — the DNS-rebinding defense
    /// `middleware`'s own tests pin exhaustively for `origin_is_allowed` — and
    /// [`middleware::stamp_build`]'s outer layering means even THAT rejection
    /// carries the build stamp, so a refusal names the build that refused.
    #[farhelm_testtrace::test]
    async fn foreign_host_is_rejected_but_still_carries_build_stamp_header() {
        let harness = rest_harness::idle_helm().await;
        let foreign_request = Request::builder()
            .method(Method::GET)
            .uri("/")
            .header(header::HOST, "evil.example:7433")
            .body(Body::empty())
            .unwrap();
        let response = build_router(Arc::clone(&harness.state), 7433)
            .oneshot(foreign_request)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response.headers().get(BUILD_STAMP_HEADER).unwrap(),
            env!("CARGO_PKG_VERSION")
        );
    }
}

/// What an embedded helm hands the desktop process that owns it once every
/// serving dependency is ready.
///
/// The device secret is the desktop app's own credential, the one its GUI
/// presents on every request and socket. It is minted in memory for this helm
/// process alone (see `auth::AuthState::mint_device`): no HTTP request can
/// obtain one, and it is worthless once the process exits. The owner must not
/// persist it; a fresh one comes with every launch.
pub struct EmbeddedReady {
    /// The address the helm actually bound, which a port-0 caller needs.
    pub addr: SocketAddr,
    /// The credential for the desktop app's GUI.
    pub native_device_secret: String,
}

/// Run an embedded helm until its owner asks it to stop or serving fails:
/// open helm.db, apply any ensure-hosts file, start a connection actor per
/// registered host, serve the API on loopback, and report the bound address
/// and the desktop's own credential once every serving dependency is ready.
///
/// The desktop shell lets the kernel choose a fresh loopback port, but it must
/// not launch the window until the HTTP listener and the local-host actor
/// exist. A synchronous channel keeps that startup boundary out of any UI
/// runtime and, unlike parsing stdout, cannot confuse another log line for
/// readiness. The explicit shutdown receiver gives `DesktopBootstrap`
/// a teardown path it can join; dropping its sender carries the same owner-
/// disappeared meaning as sending the signal. The helm installs no signal
/// handlers: the embedding process keeps its own signal policy.
///
/// Startup order is deliberate at these points, each for a different
/// reason:
///
/// - The listener is bound FIRST, so a bind failure happens before anything
///   durable is written (pinned by the e2e test
///   `helm_bind_failure_happens_before_any_durable_setup`). Every launch but
///   that test binds port 0, so what keeps two helms apart is the ownership
///   claim below, not the port.
/// - The state directory's single-owner lock is claimed next, BEFORE
///   `helm.db` is opened: a serving helm's open migrates the schema, and a
///   second helm on a different port must not upgrade the database, write
///   the host registry, or dial the fleet under a running incumbent.
/// - The ensure-hosts file is ingested BEFORE the manager starts, so the
///   guaranteed hosts have actors from the first moment rather than after a
///   reconcile — and so a bad file fails startup before anything is
///   serving. See [`ensure::ingest`] for its all-or-nothing contract.
/// - The manager returns as soon as its actors are SPAWNED, not once they
///   have connected. A down host must never delay the helm's startup: it is
///   simply a host in a non-connected state, which the API exposes for the
///   GUI to draw.
///
/// Stopping ends serving and drains connection actors, including their local
/// tunnel processes. Sessions themselves continue on their supervisors.
///
/// The native shell supplies the link opener and Downloads capabilities for
/// this window's account, including dispatch of an allowlisted completed save to
/// its default app. An absent Downloads capability leaves saves and opening
/// refused.
///
/// This synchronous entry point owns its runtime; the desktop thread returns
/// only after runtime teardown, with state ownership retained throughout it,
/// including during panic unwind. Call from outside a tokio runtime.
pub fn run_embedded(
    args: HelmArgs,
    link_opener: Option<LinkOpener>,
    downloads: Option<NativeDownloads>,
    ready: std::sync::mpsc::Sender<EmbeddedReady>,
    shutdown: tokio::sync::oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    // Reverse declaration order is the shutdown contract, including unwind:
    // runtime drop joins started blocking work before the claim can be released.
    // This entry point owns the runtime; callers cannot drop it later.
    let mut ownership = None;
    let runtime = tokio::runtime::Runtime::new().context("starting helm runtime")?;
    runtime.block_on(serve_with_ready(
        args,
        link_opener,
        downloads,
        ready,
        shutdown,
        &mut ownership,
    ))
}

#[cfg(test)]
thread_local! {
    /// One test's post-claim work, installed only on its dedicated helm thread.
    /// Thread locality prevents another helm fixture from consuming the seam.
    static HELM_RUNTIME_TEST_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}

/// Serve inside the owning runtime, publishing the claim to its outer scope.
/// Ownership outlives serving and also covers runtime drain.
async fn serve_with_ready(
    args: HelmArgs,
    link_opener: Option<LinkOpener>,
    downloads: Option<NativeDownloads>,
    ready: std::sync::mpsc::Sender<EmbeddedReady>,
    shutdown: tokio::sync::oneshot::Receiver<()>,
    ownership: &mut Option<ownership::OwnershipLock>,
) -> anyhow::Result<()> {
    let state_dir = args.state_dir.clone();
    // 0700: this directory holds helm.db, the ownership lock, and ssh
    // ControlMaster sockets. Each grants the user's authority when reached.
    farhelm_supervisor::ensure_private_dir(&state_dir).await?;

    // Bind before creating anything durable, so a bind failure leaves
    // nothing behind to strand on a retry.
    // Loopback only — deliberately not configurable in v1 (SPEC.md).
    let bind_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, args.port));
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("binding {bind_addr}"))?;
    // Use the bound address rather than the requested one. This keeps
    // programmatic callers that request port 0 coherent: Host validation
    // and the address handed to the owner must name the ephemeral port the
    // OS chose.
    let addr = listener
        .local_addr()
        .context("reading bound helm address")?;

    *ownership = Some(ownership::claim_serving_ownership(&state_dir).await?);
    #[cfg(test)]
    HELM_RUNTIME_TEST_HOOK.with_borrow_mut(|hook| {
        if let Some(hook) = hook.take() {
            hook();
        }
    });
    let store = store::HelmStore::open(&state_dir.join("helm.db")).await?;
    // Only the serving owner can write this choice, and serving starts after
    // this point. Respect a saved choice before the first automatic scan.
    if args.devbuild
        && store
            .preferences()
            .await?
            .auto_update_remote_hosts
            .is_none()
    {
        store
            .update_preferences(store::PreferencePatch {
                auto_update_remote_hosts: Some(Some(false)),
                ..Default::default()
            })
            .await
            .context("defaulting remote updates off for a devbuild")?;
    }
    // The only producer runs before readiness and serving, so every initial
    // client read sees the complete queue without needing an event-feed path.
    release_announcements::enqueue_builtin(&store).await?;
    if let Some(path) = args.ensure_hosts.as_deref() {
        ensure::ingest(&store, path).await?;
    }
    // Before the manager: its actors check a Tensorlake host's sandbox
    // through this service before every dial, from their first attempt on.
    let ssh_questions = Arc::new(askpass::Questions::default());
    let sign_in = args.ssh_askpass_program.as_ref().map(|cli| {
        Arc::new(ssh_sign_in::SignIn::new(
            state_dir.clone(),
            cli.clone(),
            Arc::clone(&ssh_questions),
        ))
    });
    let sources = provisioning::ProductionProvisioning::new(
        store.clone(),
        &state_dir,
        args.payload_selection(),
        RELEASE_BUILD,
        sign_in.clone(),
    )?;
    let tensorlake = tensorlake::Tensorlake::production_with_delivery(
        store.clone(),
        &state_dir,
        Arc::clone(&sources.delivery),
        sources.snapshot_preparer(&state_dir),
    )?;
    // Both providers share exactly one recipe registry and one place cache.
    // The concrete handle below is cloned for provider-owned Containers HTTP;
    // actors and lifecycle dispatch use this same service through the registry.
    let podman = Arc::new(environment::podman::Podman::new(
        store.clone(),
        sources.delivery.places(),
        Arc::clone(&sources.delivery),
        Arc::clone(tensorlake.builds()),
        sources.environment_preparer(&state_dir),
    ));
    let providers = environment::Providers::new(Arc::new(tensorlake.clone()), podman.clone());
    let manager = manager::ConnectionManager::start_with_providers(
        store.clone(),
        Arc::new(manager::SystemTransport::with_store(
            &state_dir,
            store.clone(),
            sources.delivery.places(),
            sign_in,
        )),
        manager::Cadence {
            refresh: args
                .backstop_refresh_secs
                .map_or(manager::REFRESH_INTERVAL, std::time::Duration::from_secs),
            ..manager::Cadence::default()
        },
        providers,
    )
    .await?;

    ssh_questions.publish_to(Arc::clone(manager.events()));
    let mut app = AppState::new(
        manager,
        store,
        tensorlake,
        Some(podman),
        state_dir.clone(),
        sources,
    )?;
    app.ssh_questions = ssh_questions;
    app.link_opener = link_opener;
    if let Some(downloads) = downloads {
        app.downloads_dir = Some(downloads.directory);
        app.file_opener = Some(downloads.opener);
    }
    let state = Arc::new(app);
    // The manager was started before this state existed — it is one of the
    // state's own fields — so the handler that answers an agent's questions
    // can only be published now. Every connection reads the slot per
    // request, so a host that connected during the gap answers correctly
    // from this moment on rather than staying permanently mute; see
    // `agent_requests::AgentRequestSlot`.
    state
        .manager
        .set_agent_requests(agent_requests::HelmAgentRequests::for_state(&state));
    // Started here for the same reason: it needs the state the manager is a
    // field of. A seal noted before this point is kept and acted on at once.
    let _seal_finisher = sessions::spawn_seal_finisher(Arc::clone(&state));
    let mute_expiry = session_mute::ExpiryTask::start(&state);
    let app = build_router(Arc::clone(&state), addr.port());

    // Establish the configuration baseline before announcing readiness or
    // accepting previews. A later CLI commit is then necessarily a change
    // the watcher can publish, even if it precedes the loop's first tick.
    let checkout_revision = state.store.checkout_config_snapshot(None).await?.revision;

    let _ = ready.send(EmbeddedReady {
        addr,
        native_device_secret: state.auth.mint_device()?,
    });

    let requested_shutdown = async move {
        // Sender cancellation means the desktop owner disappeared; it is the
        // same lifetime boundary as an explicit stop.
        let _ = shutdown.await;
    };
    let serving = tokio::select! {
        result = axum::serve(listener, app) => result.context("serving helm HTTP"),
        () = checkout_config::watch_revision(
            state.store.clone(), Arc::clone(state.manager.events()), checkout_revision,
        ) => unreachable!("checkout configuration watcher runs until serving ends"),
        () = requested_shutdown => Ok(())
    };
    drop(mute_expiry);
    state._automatic_updates.stop();
    state.tunnels.shutdown().await;
    state.manager.shutdown_and_wait().await;
    serving
}

/// The response header carrying this helm's build version to its client
/// (PLAN_M6.md item 6).
///
/// The name comes from [`farhelm_proto::http::BUILD_STAMP_HEADER`], whose
/// docs say who reads it.
const BUILD_STAMP_HEADER: &str = farhelm_proto::http::BUILD_STAMP_HEADER;

/// Classify an error chain into the coarse [`ErrorKind`] vocabulary this
/// crate's two error-facing callers both need: [`http_error`] turns it into
/// an HTTP status for the REST surface, and
/// `agent_requests::HelmAgentRequests::handle` turns the SAME classification
/// into an [`farhelm_proto::AgentOutcome::Err`] for a session-authenticated
/// agent — one rulebook rather than two that could disagree about what a
/// lifecycle mutation's refusal means.
///
/// Three families are consulted in order of specificity, because a
/// `SupervisorError` can sit UNDER either of the other two once
/// `anyhow::Context` has wrapped it on the way out. Each family is looked
/// for through [`find_cause`], which covers both ways an error can be
/// carried — as a `source()` link and as anyhow context — so the ordering
/// below is a statement about SPECIFICITY alone and not about how a caller
/// happened to attach the thing:
///
/// - [`store::HostStoreError`] (PLAN_M6.md item 5's host-management and
///   routing refusals): an unknown host id is `NotFound`, an unusable
///   destination is `InvalidRequest` (the caller sent something `ssh`
///   cannot use), and everything else the registry refuses — a duplicate
///   destination, the immovable local row, a lost identity
///   compare-and-swap, an identity another row claimed, a row reconfigured
///   mid-decision, a session two hosts both claim — is `Conflict`, because
///   each is the same shape of failure: the request was well formed and
///   conflicts with the fleet as it stands.
/// - [`manager::ManagerError`] carries the same split for the decisions the
///   manager owns rather than the store.
/// - Otherwise, whatever [`SupervisorError`] is nearest the surface is
///   classified by ITS OWN `kind` verbatim — a non-connected host's refusal
///   reaches this function as an ordinary `SupervisorError` carrying
///   `Conflict` (see `sessions::refusal_text`), for the same reading.
///
/// Nothing classified anywhere in the chain, or a `SupervisorError` that
/// already said `Internal` itself, both fall through to `Internal`: the
/// honest default for a failure the caller could not have avoided by
/// sending a different request.
///
/// One family is deliberately absent: a [`SupervisorTransportError`], the
/// shape of a target supervisor failing to give a usable answer — its
/// connection dying, a correlated reply of the wrong variant, or the right
/// variant carrying a payload the ingress rules refuse.
/// Classifying it needs a fact this function is not given — whether the
/// failed REQUEST changed anything — because the same lost answer means
/// "retry freely" for a listing and "the outcome is unknown" for a
/// mutation, be it a rename/stop/restart or a create/clone. The agent
/// caller asks that question first
/// (`agent_requests::transport_outcome`) and only falls back here; the
/// REST caller keeps the `Internal` it has always produced, since a
/// browser's retry decision is the user's own.
fn error_kind(e: &anyhow::Error) -> ErrorKind {
    if find_cause::<yolo_guard::YoloNeedsConfirmation>(e).is_some() {
        return ErrorKind::Conflict;
    }
    if let Some(refusal) = find_cause::<store::HostStoreError>(e) {
        return match refusal {
            store::HostStoreError::HostNotFound(_) => ErrorKind::NotFound,
            store::HostStoreError::InvalidDestination(_)
            | store::HostStoreError::InvalidRemoteFarhelm(_)
            | store::HostStoreError::InvalidRemoteStateDir(_)
            | store::HostStoreError::InvalidAlias(_) => ErrorKind::InvalidRequest,
            store::HostStoreError::DuplicateDestination(_)
            | store::HostStoreError::AliasTaken(_)
            | store::HostStoreError::AlreadyAdded(_)
            | store::HostStoreError::LocalHostImmutable
            | store::HostStoreError::DestinationManaged
            | store::HostStoreError::TensorlakeSetupRequired(_)
            | store::HostStoreError::SandboxNotDeleted(_)
            | store::HostStoreError::IdentityMismatch { .. }
            | store::HostStoreError::IdentityClaimed { .. }
            | store::HostStoreError::IdentityClaimedBeforeRegistration { .. }
            | store::HostStoreError::StaleAttempt { .. } => ErrorKind::Conflict,
        };
    }
    if let Some(refusal) = find_cause::<manager::ManagerError>(e) {
        return match refusal {
            manager::ManagerError::NoSuchHost(_) => ErrorKind::NotFound,
            // Re-render the host when its current state cannot accept the
            // user decision the client was displaying.
            manager::ManagerError::NotAwaitingAdoption { .. }
            | manager::ManagerError::AdoptionSuperseded { .. }
            | manager::ManagerError::TakeoverSuperseded(_) => ErrorKind::Conflict,
        };
    }
    if find_cause::<precondition::IncarnationStale>(e).is_some()
        || find_cause::<environment::lifecycle::HostBusy>(e).is_some()
        || find_cause::<sessions::EnvironmentUnasked>(e).is_some()
    {
        return ErrorKind::Conflict;
    }
    find_cause::<SupervisorError>(e)
        .map(|s| s.kind())
        .unwrap_or(ErrorKind::Internal)
}

/// Find a `T` anywhere in `e`, whether it was attached as a SOURCE or as
/// anyhow CONTEXT.
///
/// Two lookups because anyhow keeps the two in different places and neither
/// mechanism sees the other. `chain()` walks `std::error::Error::source()`
/// links, and a value attached with `.context(value)` is not one: the chain
/// element is an opaque `ContextError` wrapper whose `downcast_ref::<T>()`
/// answers `None`. `anyhow::Error::downcast_ref` is the converse — it
/// unwraps its own context layers and the head error, but does not follow
/// `source()` links belonging to foreign error types.
///
/// [`error_kind`] used to consult only the first, which made its documented
/// order of specificity quietly conditional on HOW a caller had attached
/// the more specific error: a `HostStoreError` wrapped as context around a
/// `SupervisorError` was invisible, and the chain's less specific answer
/// won. Consulting both, per family, in the family's own turn, is what
/// makes that order hold regardless of the attachment style — which matters
/// because nothing at a call site marks which style it used.
pub(crate) fn find_cause<T: std::error::Error + Send + Sync + 'static>(
    e: &anyhow::Error,
) -> Option<&T> {
    e.downcast_ref::<T>()
        .or_else(|| e.chain().find_map(|c| c.downcast_ref::<T>()))
}

/// Run a request's state-changing work on a task the helm owns, and wait for
/// its result.
///
/// axum drops a handler's future when the client goes away, stopping it at
/// whatever step it had reached; blocking work already handed off (a store
/// transaction) still finishes, so a handler that commits and then
/// reconciles running state is left half done, in a state nothing later
/// repairs (SPEC_impl.md "Who owns an accepted action"). Spawned work runs to
/// completion whether or not anyone is still waiting; the client only loses
/// the reply. A panic in the work is re-raised here, as it would have been
/// inline.
pub(crate) async fn run_owned<T: Send + 'static>(
    work: impl std::future::Future<Output = T> + Send + 'static,
) -> T {
    match tokio::spawn(work).await {
        Ok(value) => value,
        Err(join) if join.is_panic() => std::panic::resume_unwind(join.into_panic()),
        Err(join) => panic!("a helm-owned request task was cancelled: {join}"),
    }
}

/// Render an error as an HTTP response whose body is the error chain in
/// full and whose status is [`error_kind`]'s classification, mapped onto
/// the closest HTTP status for each kind (`Unavailable`→503,
/// `Timeout`→504, both gateway-shaped rather than lumped into `Internal`).
/// Both were born for the agent relay's own request/reply pair; a REST call
/// meets `Unavailable` when a supervisor refuses a management request
/// because every one of its management slots is taken, and the UI shows
/// that refusal's text on the action like any other error.
///
/// The body itself is deliberately unsanitized regardless of status:
/// SPEC.md requires concrete, actionable errors in the client, and the
/// intended reader is the user's authenticated UI. That does not make error
/// text a safe place for credentials: other same-user processes can read the
/// browser traffic, and logs may preserve the body. The invocation parser
/// therefore still keeps credentials out of its context. The UI displays the
/// body as text rather than interpreting it, which is what makes a remote
/// supervisor's message safe to pass through verbatim.
fn http_error(e: anyhow::Error) -> axum::response::Response {
    let kind = error_kind(&e);
    let yolo_refused = find_cause::<yolo_guard::YoloNeedsConfirmation>(&e).is_some();
    let unaccepted = e.downcast_ref::<FreshCreateUnaccepted>().is_some()
        || e.downcast_ref::<SettledCreateUnaccepted>().is_some()
        || kind == ErrorKind::CheckoutConflict
        || yolo_refused;
    let status = if error_kind_is_a_supervisor_reply(&e) {
        supervisor_reply_status(kind)
    } else {
        helm_status(kind)
    };
    let stale = precondition::is_stale(&e);
    // The UI shows this body verbatim.
    let mut response = (status, format!("{e:#}")).into_response();
    // Set only from the helm's own typed refusal, never from anything a
    // supervisor wrote: this is what tells a client to discard its intent and
    // re-read (see `precondition`'s module docs).
    if stale {
        response.headers_mut().insert(
            farhelm_proto::http::PRECONDITION_HEADER,
            axum::http::HeaderValue::from_static(farhelm_proto::http::PRECONDITION_INCARNATION),
        );
    }
    // Set only from the helm's own typed refusal, like the precondition
    // header above: this is what makes the browser ask for an explicit YOLO
    // confirmation, so a supervisor must not be able to set it.
    if yolo_refused {
        response.headers_mut().insert(
            farhelm_proto::http::YOLO_CONFIRMATION_HEADER,
            axum::http::HeaderValue::from_static(farhelm_proto::http::YOLO_CONFIRMATION_REQUIRED),
        );
    }
    // Helm-only like the two above: it tells a launch waking this host that
    // waiting can clear the refusal (`environment::lifecycle::HostBusy`).
    if find_cause::<environment::lifecycle::HostBusy>(&e).is_some() {
        response.headers_mut().insert(
            farhelm_proto::http::HOST_BUSY_HEADER,
            axum::http::HeaderValue::from_static(farhelm_proto::http::HOST_BUSY_OPERATION),
        );
    }
    // Helm-only too: it asks a client to offer the warned
    // whole-environment Delete, so nothing a supervisor says may set it.
    if find_cause::<sessions::EnvironmentUnasked>(&e).is_some() {
        response.headers_mut().insert(
            farhelm_proto::http::ENVIRONMENT_UNASKED_HEADER,
            axum::http::HeaderValue::from_static(farhelm_proto::http::ENVIRONMENT_UNASKED),
        );
    }
    if unaccepted {
        response.headers_mut().insert(
            farhelm_proto::http::CREATE_OUTCOME_HEADER,
            axum::http::HeaderValue::from_static(
                farhelm_proto::http::CREATE_OUTCOME_DEFINITELY_UNACCEPTED,
            ),
        );
    }
    response
}

/// The HTTP status for a kind the helm decided itself.
///
/// Direct, because the helm is choosing what to tell its own client. The one
/// arm with client-visible weight is `Unauthorized`: a 401 is what sends the
/// browser to the token prompt, and nothing in the helm raises this kind
/// today (device authentication answers in the auth middleware before any
/// route runs), so it stays mapped only for a future helm-side refusal that
/// really does mean "sign in again".
fn helm_status(kind: ErrorKind) -> axum::http::StatusCode {
    match kind {
        ErrorKind::NotFound => axum::http::StatusCode::NOT_FOUND,
        ErrorKind::InvalidRequest => axum::http::StatusCode::BAD_REQUEST,
        ErrorKind::Internal => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        // PLAN_M3.md item 6: an intent key reused with a different
        // fingerprint. 409 is the standard HTTP reading of "this identifier
        // already means something else"; `error_kind`'s own docs are where
        // the full classification table lives.
        ErrorKind::Conflict
        | ErrorKind::CheckoutConflict
        | ErrorKind::TakenOver
        | ErrorKind::Sealed => axum::http::StatusCode::CONFLICT,
        ErrorKind::Unauthorized => axum::http::StatusCode::UNAUTHORIZED,
        ErrorKind::Unavailable => axum::http::StatusCode::SERVICE_UNAVAILABLE,
        ErrorKind::Timeout => axum::http::StatusCode::GATEWAY_TIMEOUT,
    }
}

/// Whether the kind [`error_kind`] would report came from a remote
/// supervisor's own reply rather than from the helm.
///
/// Mirrors `error_kind`'s precedence: a helm store or manager refusal
/// anywhere in the chain is the helm's classification, whatever supervisor
/// error sits beneath it.
pub(crate) fn error_kind_is_a_supervisor_reply(e: &anyhow::Error) -> bool {
    find_cause::<store::HostStoreError>(e).is_none()
        && find_cause::<manager::ManagerError>(e).is_none()
        && find_cause::<SupervisorError>(e)
            .is_some_and(|s| s.origin() == ErrorOrigin::SupervisorReply)
}

/// The HTTP status for a kind a remote supervisor chose.
///
/// SPEC_impl.md's rule: a code at the supervisor's level is never taken as
/// the same-named code at the helm's level by default, and every arm here is
/// a case-by-case decision. The supervisor is untrusted, so the test for
/// each arm is what a client does with the status and whether a lying
/// supervisor could use that to reach beyond its own sessions.
fn supervisor_reply_status(kind: ErrorKind) -> axum::http::StatusCode {
    match kind {
        // A supervisor's `Unauthorized` means a session-scoped peer asked
        // for something outside its slice; the helm's full-authority
        // connection never legitimately gets one. A 401 would tell the
        // browser it is signed out and replace the whole UI, every host's
        // sessions included, with the token prompt. From the client's side
        // this is the helm's upstream refusing it, which is 502.
        ErrorKind::Unauthorized => axum::http::StatusCode::BAD_GATEWAY,
        // The supervisor is the authority on its own sessions and tabs. The
        // UI reads a 404 on a session fetch as "this session is gone", and a
        // supervisor that lies here can only hide its own objects, which it
        // controls anyway.
        ErrorKind::NotFound => axum::http::StatusCode::NOT_FOUND,
        // The supervisor judged the request's content (a cwd, an
        // invocation) against its own host. Clients only display the text.
        ErrorKind::InvalidRequest => axum::http::StatusCode::BAD_REQUEST,
        // Conflicts with the supervisor's own state. Clients display the
        // text; the one conflict a client acts on, the stale-create
        // refusal, is signalled by a helm-only header, never by status or
        // body, so a supervisor's 409 cannot trigger it.
        ErrorKind::Conflict | ErrorKind::TakenOver => axum::http::StatusCode::CONFLICT,
        // A sealed environment refusing a new session or run: a conflict
        // with the supervisor's own state like the arm above. Clients that
        // act on it (Replace, the environment Delete) read the kind from the
        // helm's own handling, never from this status.
        ErrorKind::Sealed => axum::http::StatusCode::CONFLICT,
        // The supervisor performs the checkout mkdir, so it is the
        // authority on whether that create was accepted. `http_error` adds
        // the definitely-unaccepted header for this kind; a supervisor
        // lying about it can only cause a duplicate attempt on itself.
        ErrorKind::CheckoutConflict => axum::http::StatusCode::CONFLICT,
        // Clients display these without acting on the status, and each
        // already describes a failure upstream of the helm. `Unavailable`
        // is how a busy supervisor refuses a management request ("try
        // again"); a supervisor lying with it can only make its own host's
        // actions show an error.
        ErrorKind::Internal => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        ErrorKind::Unavailable => axum::http::StatusCode::SERVICE_UNAVAILABLE,
        ErrorKind::Timeout => axum::http::StatusCode::GATEWAY_TIMEOUT,
    }
}

/// An unkeyed fresh request failed before any create frame was dispatched.
/// Keyed local refusals cannot establish this proof: a concurrent request may
/// accept the same key after an unknown lookup. Spent or unresolved keys also
/// conflict and must never acquire this marker from their status alone.
#[derive(Debug, thiserror::Error)]
#[error("managed checkout was not accepted")]
pub(crate) struct FreshCreateUnaccepted;

/// The create's supervisor confirmed a durable refusal with no session left.
/// Kept distinct from the helm's pre-dispatch proof: a transport error, a
/// retained failed session, or a later failed Delete must never acquire it.
#[derive(Debug, thiserror::Error)]
#[error("the create was durably refused without retaining a session")]
pub(crate) struct SettledCreateUnaccepted;

#[cfg(test)]
mod tests {
    use super::SupervisorError;

    /// A `HelmArgs` with every optional field unset, port 0 and a placeholder
    /// state directory no test serves from — the base every test below
    /// customizes exactly the fields its case is about.
    fn base_args() -> crate::HelmArgs {
        crate::HelmArgs {
            port: 0,
            state_dir: std::path::PathBuf::from("/nonexistent/helm-state"),
            devbuild: false,
            ssh_askpass_program: None,
            ensure_hosts: None,
            payload_dir: None,
            backstop_refresh_secs: None,
        }
    }

    /// A devbuild can share remote hosts with a release. Seed the safe default during
    /// serving startup, preserving either explicit choice and the regular launch default.
    #[farhelm_testtrace::test]
    async fn devbuild_startup_only_seeds_an_unset_remote_update_choice() {
        for (devbuild, saved, expected) in [
            (true, None, Some(false)),
            (true, Some(true), Some(true)),
            (true, Some(false), Some(false)),
            (false, None, None),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("helm.db");
            let store = crate::store::HelmStore::open(&path).await.unwrap();
            if let Some(on) = saved {
                store
                    .update_preferences(crate::store::PreferencePatch {
                        auto_update_remote_hosts: Some(Some(on)),
                        ..Default::default()
                    })
                    .await
                    .unwrap();
            }
            assert_eq!(
                store.preferences().await.unwrap().auto_update_remote_hosts,
                saved
            );
            drop(store);

            let (ready_tx, ready_rx) = std::sync::mpsc::channel();
            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
            // Request shutdown at the first serving boundary, after all startup work.
            shutdown_tx.send(()).unwrap();
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                crate::serve_with_ready(
                    crate::HelmArgs {
                        state_dir: dir.path().to_path_buf(),
                        devbuild,
                        ..base_args()
                    },
                    None,
                    None,
                    ready_tx,
                    shutdown_rx,
                    &mut None,
                ),
            )
            .await
            .expect("helm must reach and leave its serving boundary")
            .expect("helm startup and shutdown succeeded");
            ready_rx.try_recv().expect("the helm reported readiness");
            let reopened = crate::store::HelmStore::open(&path).await.unwrap();
            assert_eq!(
                reopened
                    .preferences()
                    .await
                    .unwrap()
                    .auto_update_remote_hosts,
                expected
            );
        }
    }

    /// A second helm on another port refuses before it opens (and so
    /// migrates) a state directory another helm already owns.
    ///
    /// Why it matters: the serving open migrates `helm.db`, and startup then
    /// writes the host registry and dials the fleet. A newer helm started
    /// beside an older running one used to do all of that before discovering
    /// the owner lock was taken, upgrading the schema under the incumbent.
    /// Spec: the ownership claim precedes the database open; the refused
    /// helm leaves the schema version untouched.
    #[farhelm_testtrace::test]
    async fn a_second_helm_refuses_before_migrating_an_owned_state_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        // A real schema-29 database: the current schema minus what the 29→30
        // step adds. Only a database the open CAN upgrade shows whether the
        // refused helm upgraded it.
        drop(crate::store::HelmStore::open(&path).await.unwrap());
        let conn = rusqlite::Connection::open(&path).unwrap();
        // Schema 36 drops the profile tables, so a file claiming version 29
        // has to hold them.
        conn.execute_batch(crate::store::PROFILE_TABLES_V15)
            .unwrap();
        conn.execute_batch(
            "ALTER TABLE preferences DROP COLUMN remembered_workspace_trust;
            ALTER TABLE preferences DROP COLUMN skip_host_remove_confirmation;
            ALTER TABLE preferences DROP COLUMN skip_host_setup_confirmation;
                 ALTER TABLE preferences DROP COLUMN terminal_link_click;
                 ALTER TABLE preferences DROP COLUMN feedback_contact;
            ALTER TABLE hosts DROP COLUMN commands_without_asking;
            ALTER TABLE hosts DROP COLUMN yolo_without_asking;
             PRAGMA user_version = 29;",
        )
        .unwrap();
        // Closing the last connection checkpoints the rewound schema into the
        // main file. Copying it while those writes remain in WAL would test an
        // already-current database instead of the intended upgrade fixture.
        drop(conn);
        let premise = tempfile::tempdir().unwrap();
        let unowned_copy = premise.path().join("helm.db");
        std::fs::copy(&path, &unowned_copy).unwrap();
        let copied_version: i64 = rusqlite::Connection::open(&unowned_copy)
            .unwrap()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            copied_version, 29,
            "fixture premise: the copy needs migration"
        );
        drop(crate::store::HelmStore::open(&unowned_copy).await.unwrap());
        let upgraded: i64 = rusqlite::Connection::open(&unowned_copy)
            .unwrap()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert!(
            upgraded > 29,
            "fixture premise: an unowned open upgrades this database"
        );
        let _incumbent = crate::ownership::claim_serving_ownership(dir.path())
            .await
            .expect("fixture premise: the incumbent owns the directory");

        // Both ends held for the whole call, so a regression that served
        // keeps serving into the timeout below rather than reading a dropped
        // sender as its owner asking it to stop.
        let (ready_tx, _ready_rx) = std::sync::mpsc::channel();
        let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            crate::serve_with_ready(
                crate::HelmArgs {
                    state_dir: dir.path().to_path_buf(),
                    ..base_args()
                },
                None,
                None,
                ready_tx,
                shutdown_rx,
                &mut None,
            ),
        )
        .await
        .expect("a refused helm must return rather than serve");
        let error = result.expect_err("the second helm must refuse an owned state directory");
        assert!(
            format!("{error:#}").contains("another helm owns this state directory"),
            "the refusal names the owner lock: {error:#}"
        );
        let after: i64 = rusqlite::Connection::open(&path)
            .unwrap()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(after, 29, "the refused helm must not migrate helm.db");
    }

    /// A successor must stay excluded after serving ends while the old runtime
    /// joins blocking work. A refused connection to the helm's address proves
    /// serving ended; the worker's entered/release channels prove that runtime
    /// drain still has work to join.
    /// Releasing it then permits a new claim, so a leaked lock cannot pass.
    #[farhelm_testtrace::test]
    async fn helm_ownership_outlives_runtime_drain() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        };
        use std::time::Duration;
        let dir = Arc::new(tempfile::tempdir().unwrap());
        let helm_dir = dir.clone();
        let state_dir = dir.path().to_path_buf();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let parked = Arc::new(AtomicBool::new(false));
        let worker_parked = parked.clone();
        let helm = std::thread::spawn(move || {
            // Keep fixture state alive through runtime drop even if the test
            // unwinds before its explicit release and join.
            let _helm_dir = helm_dir;
            crate::HELM_RUNTIME_TEST_HOOK.with_borrow_mut(|hook| {
                *hook = Some(Box::new(move || {
                    tokio::task::spawn_blocking(move || {
                        worker_parked.store(true, Ordering::SeqCst);
                        entered_tx.send(()).unwrap();
                        // Timeout and sender drop both release cleanup on assertion failure.
                        let result = release_rx.recv_timeout(Duration::from_secs(30));
                        worker_parked.store(false, Ordering::SeqCst);
                        assert!(
                            !matches!(result, Err(mpsc::RecvTimeoutError::Timeout)),
                            "test never released worker"
                        );
                    });
                }))
            });
            crate::run_embedded(
                crate::HelmArgs {
                    state_dir,
                    ..base_args()
                },
                None,
                None,
                ready_tx,
                shutdown_rx,
            )
        });
        entered_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("worker started on owned runtime");
        let ready = ready_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("real helm serving dependencies ready");
        assert!(parked.load(Ordering::SeqCst));
        // Premise for the oracle below: a refused connection only shows that
        // serving ended if the listener accepted connections before it.
        tokio::net::TcpStream::connect(ready.addr)
            .await
            .expect("fixture premise: the serving helm accepts connections");
        shutdown_tx.send(()).unwrap();
        wait_for_serving_to_end(ready.addr).await;
        assert!(
            parked.load(Ordering::SeqCst),
            "fixture worker must still be parked after serving ends"
        );
        let busy = crate::ownership::acquire_ownership(dir.path().to_path_buf()).await;
        let refused = busy.is_err();
        let message = busy
            .err()
            .map(|error| format!("{error:#}"))
            .unwrap_or_default();
        // Always release and join before reporting a failed exclusion assertion.
        release_tx.send(()).unwrap();
        helm.join().expect("helm thread did not panic").unwrap();
        assert!(
            refused,
            "a successor acquired ownership while old work was parked"
        );
        assert!(
            message.contains("another helm owns this state directory"),
            "{message}"
        );
        assert!(!parked.load(Ordering::SeqCst));
        let _successor = crate::ownership::acquire_ownership(dir.path().to_path_buf())
            .await
            .expect("ownership is released after runtime drain finishes");
    }

    /// Observe the HTTP listener closing rather than assuming a shutdown send
    /// ran. This bounds only the fixture's readiness wait, while its owned
    /// worker remains parked and gives timeout diagnostics the premise they
    /// need.
    async fn wait_for_serving_to_end(addr: std::net::SocketAddr) {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while tokio::net::TcpStream::connect(addr).await.is_ok() {
                // sleep-ok: poll the owned listener's teardown before probing the still-draining runtime's lock.
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("serving must close its listener before the drain assertion");
    }

    /// Spec (D13): a payload directory selects `Directory` with exactly that
    /// path, and no directory selects `Default`, which leaves the choice to
    /// the build.
    ///
    /// Why it matters: this is the one place the desktop app's
    /// `FARHELM_HELM_PAYLOAD_DIR` turns into what provisioning reads, so a
    /// slip here would make a working-copy build ignore the payloads staged
    /// for it, or an installed app stop downloading.
    #[farhelm_testtrace::test]
    fn payload_selection_is_the_directory_when_given_else_the_default() {
        let args = crate::HelmArgs {
            payload_dir: Some(std::path::PathBuf::from("/opt/farhelm-payloads")),
            ..base_args()
        };
        assert!(matches!(
            args.payload_selection(),
            crate::provisioning::PayloadSelection::Directory(dir)
                if dir == std::path::Path::new("/opt/farhelm-payloads")
        ));
        assert!(matches!(
            base_args().payload_selection(),
            crate::provisioning::PayloadSelection::Default
        ));
    }

    /// Work handed to `run_owned` finishes even when the request that
    /// started it is dropped.
    ///
    /// Why it matters: axum drops a handler's future when its client goes
    /// away, and host edits commit a change and then reconcile the running
    /// connections; stopped in between, the registry and the actors disagree
    /// until something unrelated repairs them (SPEC_impl.md "Who owns an
    /// accepted action"). Specified: a waiter dropped while its work is
    /// blocked does not stop the work, which completes once unblocked.
    #[farhelm_testtrace::test]
    async fn owned_work_completes_after_its_waiter_is_dropped() {
        let release = std::sync::Arc::new(tokio::sync::Notify::new());
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let work_release = std::sync::Arc::clone(&release);
        let waiter = tokio::spawn(super::run_owned(async move {
            let _ = started_tx.send(());
            work_release.notified().await;
            let _ = done_tx.send(());
        }));
        // The work must already belong to its own task before the waiter
        // goes away; a scheduler yield does not promise that.
        tokio::time::timeout(std::time::Duration::from_secs(5), started_rx)
            .await
            .expect("test premise: the owned work must start")
            .expect("test premise: the owned work must start");
        waiter.abort();
        let _ = waiter.await;
        release.notify_one();
        tokio::time::timeout(std::time::Duration::from_secs(5), done_rx)
            .await
            .expect("the owned work must finish after its waiter is gone")
            .expect("the owned work must run to completion");
    }

    /// `http_error`'s downcast must find a [`SupervisorError`] under a
    /// `.context(...)` layer, not just at the root — an intermediate
    /// caller (`connect_supervisor`, say) is free to add its own context
    /// on the way out, and the status mapping must survive that. Spec:
    /// `NotFound` → 404 regardless of how many context layers sit on top.
    #[farhelm_testtrace::test]
    fn http_error_maps_context_wrapped_not_found_to_404() {
        let err = anyhow::Error::new(SupervisorError::helm(
            farhelm_proto::ErrorKind::NotFound,
            "no such session: s1".to_string(),
        ))
        .context("attaching to terminal");
        let response = super::http_error(err);
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    }

    /// Spec: a `SupervisorError` explicitly carrying `Internal` maps to
    /// 500 — the honest default for a failure the caller could not have
    /// avoided by sending a different request, distinct from `NotFound`/
    /// `InvalidRequest` mapping to their own codes above.
    #[farhelm_testtrace::test]
    fn http_error_maps_internal_supervisor_error_to_500() {
        let err = anyhow::Error::new(SupervisorError::helm(
            farhelm_proto::ErrorKind::Internal,
            "terminal hiccup".to_string(),
        ));
        let response = super::http_error(err);
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    /// PLAN_M3.md item 6: an intent key reused with a different
    /// fingerprint maps to 409, the standard HTTP reading of "this
    /// identifier already means something else" (see `ErrorKind::Conflict`'s
    /// own doc comment in farhelm-proto for the full rationale, and this
    /// function's own docstring for the mapping table).
    #[farhelm_testtrace::test]
    fn http_error_maps_conflict_supervisor_error_to_409() {
        let err = anyhow::Error::new(SupervisorError::helm(
            farhelm_proto::ErrorKind::Conflict,
            "intent key already used with a different request".to_string(),
        ));
        let response = super::http_error(err);
        assert!(
            !response.headers().contains_key("x-farhelm-create-outcome"),
            "Conflict alone cannot prove an intent was never accepted"
        );
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
    }

    /// Only explicit pre-dispatch proof permits the browser to distinguish an
    /// unaccepted fresh request from an identically coded spent-key conflict.
    #[farhelm_testtrace::test]
    fn fresh_unaccepted_marker_preserves_status_and_exposes_outcome() {
        let error = anyhow::Error::new(SupervisorError::helm(
            farhelm_proto::ErrorKind::Conflict,
            "checkout settings changed".into(),
        ))
        .context(super::FreshCreateUnaccepted);
        let response = super::http_error(error);
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
        assert_eq!(
            response.headers()["x-farhelm-create-outcome"],
            "definitely-unaccepted"
        );
    }

    /// A durably settled supervisor refusal carries the same proof on first
    /// reply and replay. The HTTP marker must survive that wire-to-REST seam
    /// without relying on user-facing message text.
    #[farhelm_testtrace::test]
    fn supervisor_checkout_conflict_exposes_unaccepted_outcome() {
        let response = super::http_error(anyhow::Error::new(SupervisorError::helm(
            farhelm_proto::ErrorKind::CheckoutConflict,
            "checkout path occupied".into(),
        )));
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
        assert_eq!(
            response.headers()["x-farhelm-create-outcome"],
            "definitely-unaccepted"
        );
    }

    /// A discovery claim made before its destination is registered is still
    /// a fleet conflict, so the HTTP layer must expose the same 409 response
    /// as the existing-row identity-claim path.
    #[farhelm_testtrace::test]
    fn error_kind_maps_pre_registration_identity_claim_to_conflict() {
        let err = anyhow::Error::new(
            crate::store::HostStoreError::IdentityClaimedBeforeRegistration {
                identity: "identity-owned".to_string(),
                owner: 42,
            },
        );
        let response = super::http_error(err);
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
    }

    /// A remote supervisor's `Unauthorized` never becomes the helm's 401.
    ///
    /// The browser UI treats a 401 carrying `device_auth_required` as "you
    /// are signed out" and swaps the whole app, every host's sessions
    /// included, for the token prompt. A supervisor's refusal reaches the
    /// client with its message verbatim, so a compromised host that spelled
    /// that JSON used to force the prompt on every request touching its
    /// sessions. The translation must hold whatever the message says.
    #[farhelm_testtrace::test]
    fn http_error_never_maps_a_supervisor_unauthorized_to_401() {
        let err = anyhow::Error::new(crate::client::supervisor_reply_error_for_test(
            farhelm_proto::ErrorKind::Unauthorized,
            format!(
                r#"{{"code":"{}"}}"#,
                farhelm_proto::http::AUTH_REQUIRED_CODE
            ),
        ));
        let response = super::http_error(err);
        assert_eq!(response.status(), axum::http::StatusCode::BAD_GATEWAY);
    }

    /// A supervisor's text can never produce the stale-create signal.
    ///
    /// The one conflict a client answers by discarding its intent and
    /// re-reading is marked by a helm-only header. It used to be a marker at
    /// the end of the body, which a supervisor's own refusal could end with
    /// too; a conflict from a supervisor must come back as a plain 409.
    #[farhelm_testtrace::test]
    fn a_supervisor_conflict_never_carries_the_stale_create_header() {
        let err = anyhow::Error::new(crate::client::supervisor_reply_error_for_test(
            farhelm_proto::ErrorKind::Conflict,
            "refused [farhelm:precondition/incarnation]".to_string(),
        ));
        let response = super::http_error(err);
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
        assert!(
            response
                .headers()
                .get(farhelm_proto::http::PRECONDITION_HEADER)
                .is_none()
        );
    }

    /// Spec: an error chain with no `SupervisorError` anywhere in it — a
    /// bare `anyhow` error from a layer that never classified anything —
    /// must default to 500 rather than panicking or silently guessing a
    /// more specific status.
    #[farhelm_testtrace::test]
    fn http_error_maps_unclassified_error_to_500() {
        let err = anyhow::anyhow!("supervisor connection closed");
        let response = super::http_error(err);
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    /// A malformed registered remote executable is caller input, so its
    /// typed store error must reach the same 400 response as other invalid
    /// host-registration fields instead of being reported as a server fault.
    #[farhelm_testtrace::test]
    fn http_error_maps_invalid_remote_farhelm_to_400() {
        let error = anyhow::Error::new(crate::store::HostStoreError::InvalidRemoteFarhelm(
            ".".to_string(),
        ));
        let response = super::http_error(error);
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    }

    /// Pins `error_kind`'s documented order of specificity — `HostStoreError`
    /// before `SupervisorError` — against a chain that carries BOTH, rather
    /// than leaving that order asserted only in a doc comment.
    ///
    /// `error_kind` now serves two callers (`http_error` here and
    /// `agent_requests::HelmAgentRequests::handle`, added alongside the
    /// lifecycle verbs) instead of one, which is exactly the situation in
    /// which an undocumented-by-test ordering rule gets silently reordered
    /// by someone touching only one of the two call sites: nothing would
    /// fail until a chain shaped like this one reached whichever caller
    /// nobody re-checked. The shape itself is realistic, not contrived — a
    /// `SupervisorError` from a low-level call wrapped in a `.context(...)`
    /// that happens to be (or itself wrap) a `HostStoreError` is exactly
    /// what an intermediate caller composing both layers could produce
    /// without meaning to.
    #[farhelm_testtrace::test]
    fn error_kind_prefers_the_host_store_family_over_a_supervisor_error_in_the_same_chain() {
        let inner = anyhow::Error::new(SupervisorError::helm(
            farhelm_proto::ErrorKind::Internal,
            "an inner, less specific classification".to_string(),
        ));
        let chain = inner.context(crate::store::HostStoreError::HostNotFound(42));
        let response = super::http_error(chain);
        assert_eq!(
            response.status(),
            axum::http::StatusCode::NOT_FOUND,
            "the HostStoreError family must win over a SupervisorError deeper in the same chain"
        );
    }
}
