//! The helm: Farhelm's single control-plane process.
//!
//! Per SPEC.md, exactly one helm runs at a time. It connects to
//! supervisors (locally over their unix socket, remotely over the user's
//! own ssh running `farhelm internal stdio`), aggregates their sessions,
//! and serves the UI and API over loopback HTTP/WS. It holds no
//! authoritative session state — supervisors are the authority.
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
//! - **The session list is merged and paged.** [`aggregate::session_page`]
//!   merges one indexed page of helm.db's cross-host cache with the rows a
//!   connected host holds in the manager's memory when it has no identity
//!   to bind a cache write to, tags each row with its host, marks rows of
//!   non-connected hosts stale, and pages the result with a helm-level
//!   cursor that is deliberately independent of the wire cursor underneath
//!   it (see that module's docs).
//! - **Session operations route by owner.** [`sessions::route_session`]
//!   looks a session's host up in that same merged view and hands back the
//!   host's LIVE connection — or refuses, naming the state the host is
//!   actually in. Unreachable is not special-cased; it is one of several
//!   ways a host can fail to be connected, and all of them refuse
//!   identically.
//! - **Hosts are managed over REST.** [`hosts`] is the registry's own
//!   surface — add, retarget, remove, adopt, retry — and `--ensure-hosts`
//!   ([`ensure`]) is the same registration path run once at startup.
//!
//! ## What M6.75 added (PLAN_M6_75.md item 5)
//!
//! Two things, and they are deliberately independent of each other:
//!
//! - **Clients CAN stop polling.** [`events`] serves a WebSocket that
//!   carries a REVISION NUMBER and nothing else; every chokepoint that
//!   changes what a client could read bumps it, and only when something
//!   actually changed. A client re-reads through the same REST readers it
//!   already had, so there is exactly one serving path and one set of
//!   consistency rules — see that module for the changed-only rule and the
//!   subscription handshake the fallback handover depends on. This is the
//!   capability, not its consumption: retiring the UI's four periodic loops
//!   is PLAN_M6_75.md item 6's work, against the contract this module
//!   freezes.
//! - **Narrowing happens here, not in the browser.** The merged list takes
//!   SPEC.md's filter dimensions as query parameters and answers a
//!   filtered request with two counts (matching, and the fleet's own),
//!   because a client that filtered the page it was handed would hide
//!   matches beyond the page cut while reporting a count that included them.
//!   The CACHED half of that count is computed once per page WALK and kept on
//!   this side ([`aggregate::MatchingCounts`]) rather than carried in the
//!   client's cursor — a count a caller can edit is a count the server would
//!   be repeating back to whoever asked. The half that lives only in memory is
//!   recounted per page, from the same snapshot the rows come from, because
//!   nothing qualifies it that a page could check. [`profiles`] is the other half of that
//!   surface: profile CRUD proxied to the owning supervisor, with the one
//!   profile fact the helm owns — the remembered default per host — served
//!   beside the catalog, and identity-bound so one install's preference can
//!   never resolve on another's.
//!
//! M1's argv session flags (`--ssh`, `--cwd`, `--agent`, `--title`,
//! `--remote-farhelm`, `--remote-state-dir`) are gone in this same PR: the
//! registry and the create API are the mechanism now, and the last two live
//! on as per-host registry fields.
//!
//! ## What is left in this file
//!
//! The composition root, and nothing that answers a request. [`HelmArgs`]
//! and [`run`] are the process; [`AppState`] is the fleet the REST handlers
//! reach into; [`build_router`] is the one place the routes and the
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
use clap::Args;
use farhelm_proto::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::warn;

mod client;
pub use client::{
    CreateExtras, PeerHello, SessionListing, SessionPage, SupervisorClient, SupervisorError,
    TermDetachSignal, TermEvent, TermStream,
};

/// The merged multi-host session list and the helm-level cursor that pages
/// it — what `GET /api/sessions` is built out of.
mod aggregate;

/// Browser token exchange, explicit device-secret enforcement, and live socket
/// revocation.
mod auth;

/// `POST /api/client-log` — the desktop webview console shim's receiving
/// end: authenticated, capped, and forwarded into native `tracing` under
/// the `webview_console` target (PLAN_desktop_web_bug_triage.md).
mod client_log;

/// The web UI tree a release build compiles into this binary (D12/D13) —
/// `build.rs`'s counterpart. Public so `farhelm-desktop` (Step 4) can reach
/// the same compiled-in tree its embedded helm serves.
mod embedded_ui;
pub use embedded_ui::embedded_ui;

/// `--ensure-hosts`: the JSON5 floor under the registry, applied once
/// before serving starts.
mod ensure;

/// `/api/events` — the invalidation feed: a WebSocket carrying revision
/// numbers and nothing else, which is what lets every client drop its
/// polling loops (PLAN_M6_75.md item 5).
mod events;

/// The changed-only fleet invalidation feed shared by the manager and REST edge.
mod feed;

/// `/api/hosts` — the registry's REST surface, and the JSON shape a UI
/// renders a host chip from.
mod hosts;

/// The per-host connection actors, their reconnect state machine, and the
/// cache refresh that rides them (PLAN_M6.md item 4).
///
/// Public because the desktop embedder drives one directly, and because the
/// real-transport tests in `farhelm`'s e2e suite are written against it.
pub mod manager;

/// The layers wrapped around the routes: the loopback-origin guard and the
/// build stamp, which every response passes through, plus the CORS headers
/// scoped to the desktop webview's token-exchange and attachment routes.
mod middleware;

/// The optional preconditions a mutation may carry — which install it was
/// prepared against, and which definition it means to replace — so a request
/// written for one moment cannot execute in another.
mod precondition;

/// Discovery-first supervisor setup, explicit update, and host-scoped run
/// progress (PLAN_M7.md item 6).
mod provisioning;
pub use provisioning::{LocalSupervisorDiscovery, discover_local_supervisor};

/// `/api/hosts/{id}/profiles` — agent profile CRUD, proxied to the owning
/// supervisor, plus the helm-owned remembered default served beside it.
mod profiles;

/// The session REST surface — the list, the owner-lookup routing behind
/// every operation on one session, and the handlers themselves.
mod sessions;

/// The ssh argv the remote transport is built out of, and the handshake
/// failure only that transport can explain.
mod ssh;

pub mod store;

/// The terminal WebSocket: the browser's end of an attachment.
mod terminal;

/// Private local coordination between the token CLI and a serving helm.
mod token_control;
pub use token_control::{rotate as rotate_token, show as show_token};

/// Local and SSH transport implementations behind the connection manager's seam.
mod transport;

/// The systemd user units Farhelm writes, shared by `farhelm helm setup`
/// and remote provisioning so the two cannot render different policy.
pub mod units;

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

/// CLI arguments for `farhelm helm run`. Lives here (not in the bin
/// crate) so the helm's surface and its implementation evolve together.
///
/// M1's session and transport flags are deliberately absent (PLAN_M6.md
/// item 5, user decision 2026-08-04). `--ssh`/`--remote-farhelm`/
/// `--remote-state-dir` became per-host registry fields, and
/// `--cwd`/`--agent`/`--title` became `POST /api/sessions` — a helm now
/// drives every registered host at once, so an argv flag naming ONE of
/// them could only ever have meant the wrong thing.
#[derive(Args, Debug, Clone)]
pub struct HelmArgs {
    /// Loopback port for the web UI and API.
    #[arg(long, default_value_t = 7433)]
    pub port: u16,

    /// State directory (default: ~/.local/state/farhelm). Holds helm.db,
    /// the private token-control socket, the ssh ControlMaster sockets, and
    /// — in the ordinary single-machine arrangement — the local supervisor's
    /// socket the reserved local host row is reached through.
    #[arg(long)]
    pub state_dir: Option<PathBuf>,

    /// Directory with the built web UI (index.html + assets). Overrides
    /// whatever UI this binary compiled in (D12/D13), for pointing at a
    /// freshly rebuilt `dx` output without recompiling `farhelm-helm`. The
    /// browser UI is unavailable — the API still serves, and the UI's own
    /// paths 404 — only when neither this flag nor a compiled-in tree is
    /// present.
    #[arg(long)]
    pub ui_dist: Option<PathBuf>,

    /// JSON5 file of hosts to guarantee are registered at startup:
    /// `{ hosts: [ { ssh, remote_farhelm?, remote_state_dir? } ] }`.
    ///
    /// An additive floor over helm.db, applied before serving begins and
    /// never consulted again — see `crate::ensure` for what it does and
    /// does not promise. Built for half-automated setups and agent-driven
    /// testing, where a fleet has to exist before anything can drive it.
    #[arg(long)]
    pub ensure_hosts: Option<PathBuf>,
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
    /// The browser security boundary: durable credentials plus the
    /// process-local channel that closes admitted sockets on rotation.
    auth: auth::AuthState,
    /// Matching counts already computed, so a filtered page WALK pays for one
    /// rather than one per page — see [`aggregate::MatchingCounts`], which
    /// also carries why this must not live in the client's cursor.
    ///
    /// Per-helm rather than a process-wide static, like every other bound
    /// here: an embedded second helm has its own store generations, and a
    /// shared cache would qualify one helm's counts with another's data.
    counts: aggregate::MatchingCounts,
    /// How many `/api/events` subscriptions this helm admits at once.
    ///
    /// Held here rather than read from a constant at the call site purely so
    /// a test can exhaust it through REAL sockets: a bound that could only be
    /// reached by opening sixty-four connections would be tested against the
    /// counter instead of against the endpoint, which is the half that can
    /// actually forget to admit.
    event_subscriber_cap: usize,
    /// Serializes each host's catalog MUTATIONS — the read-compare-forward
    /// edit ([`profiles::update_profile`]) and the delete that would otherwise
    /// land inside one ([`profiles::delete_profile`]).
    ///
    /// Keyed by host and created on demand, because the interesting case is
    /// two clients editing the SAME host's catalog; edits to different hosts
    /// have nothing to serialize against each other.
    ///
    /// Entries are never removed, including for a host that is later
    /// forgotten. That is only a bound because a lock is allocated exclusively
    /// for a host the registry currently holds — see [`Self::profile_edit_lock`],
    /// whose contract is what keeps a caller-supplied path id from minting
    /// entries. Each entry is an empty mutex, so reclaiming them would cost a
    /// lifetime rule (who may drop a lock another request is queued on?) to
    /// save nothing measurable.
    profile_edits:
        std::sync::Mutex<std::collections::HashMap<store::HostId, Arc<tokio::sync::Mutex<()>>>>,
    /// How many requests are currently BLOCKED waiting for some host's
    /// profile-mutation lock, across the whole fleet.
    ///
    /// Instrumentation, and the only reason it exists is that the property it
    /// exposes is otherwise untestable: "the second edit reached the queue
    /// before the first released it" is the entire content of the
    /// serialization contract, and without an observable for it a test can
    /// only sleep and hope — which passes just as happily against a helm that
    /// serializes nothing. A `tokio::sync::Mutex` publishes no waiter count of
    /// its own, so this is counted where the wait happens
    /// ([`Self::enter_profile_edit`]).
    ///
    /// The COUNTING is compiled in every build, for the reason
    /// `store::HelmStore::counting_passes` records: a hook that exists only in
    /// the test build lets the shape it guards drift in the build nobody
    /// tests. Two atomic operations per mutation cost nothing beside a round
    /// trip to a supervisor. Only the reader is `cfg(test)`, because
    /// `AppState` is private to this crate and production has no question this
    /// number answers.
    profile_edit_queue: std::sync::atomic::AtomicUsize,
    /// The one in-flight provisioning authority shared by every browser.
    provisioning: Arc<provisioning::ProvisioningService>,
    /// The shared fixed-window accept budget behind `POST /api/client-log`
    /// (see [`client_log::RateWindow`]).
    ///
    /// Per-helm rather than a process-wide static, for the same reason
    /// [`Self::counts`] is: an embedded second helm serving its own webview
    /// must not have its budget shared with — or starved by — another
    /// helm's.
    client_log_rate: std::sync::Mutex<client_log::RateWindow>,
}

/// Decrements [`AppState::profile_edit_queue`] however its waiter leaves —
/// acquired, cancelled, or unwound.
///
/// A drop guard rather than a matching `fetch_sub`, because the wait it
/// counts is exactly the thing an axum handler's cancellation interrupts: a
/// client disconnecting mid-queue would otherwise leave the counter high for
/// the life of the process, and an instrument that only ever climbs is worse
/// than none.
struct QueuedEdit<'a>(&'a std::sync::atomic::AtomicUsize);

impl Drop for QueuedEdit<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

impl AppState {
    /// The shared state as production builds it: the two halves of the
    /// fleet, provisioning authority, and every bound/cache at its default.
    ///
    /// A constructor rather than a literal at each call site so the defaults
    /// live in ONE place — the test harness stands this up too, and a
    /// harness that quietly used a different subscriber cap or its own count
    /// cache would be testing something the product does not do.
    fn new(
        manager: Arc<manager::ConnectionManager>,
        store: store::HelmStore,
        state_dir: PathBuf,
    ) -> anyhow::Result<AppState> {
        let provisioning = provisioning::ProvisioningService::production(
            store.clone(),
            Arc::clone(&manager),
            state_dir,
        )?;
        Ok(Self::with_provisioning(manager, store, provisioning))
    }

    /// Assemble state with an injected provisioning service. The ordinary
    /// constructor owns production wiring; this seam lets provisioning tests
    /// keep every path and external action isolated.
    fn with_provisioning(
        manager: Arc<manager::ConnectionManager>,
        store: store::HelmStore,
        provisioning: Arc<provisioning::ProvisioningService>,
    ) -> AppState {
        AppState {
            manager,
            store: store.clone(),
            auth: auth::AuthState::new(store),
            counts: aggregate::MatchingCounts::default(),
            event_subscriber_cap: events::MAX_SUBSCRIBERS,
            profile_edits: std::sync::Mutex::new(std::collections::HashMap::new()),
            profile_edit_queue: std::sync::atomic::AtomicUsize::new(0),
            provisioning,
            client_log_rate: std::sync::Mutex::new(client_log::RateWindow::new(
                std::time::Instant::now(),
            )),
        }
    }

    /// The lock that makes one host's profile mutations a queue rather than a
    /// race. See [`Self::profile_edits`].
    ///
    /// CALLERS MUST HAVE ESTABLISHED THAT `host` EXISTS. The map is documented
    /// as bounded by the registry, and nothing here can enforce that: the id
    /// arrives as a path segment, so calling this before routing turns any
    /// stream of made-up ids into permanent entries — a compromised
    /// authenticated device growing this process's memory one `i64` at a time. Every
    /// call site therefore routes first (`sessions::host_client`) and takes the
    /// lock afterwards, then re-routes under it, because a host can be
    /// forgotten while a request waits its turn.
    fn profile_edit_lock(&self, host: store::HostId) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self
            .profile_edits
            .lock()
            .expect("profile edit lock map poisoned");
        Arc::clone(locks.entry(host).or_default())
    }

    /// Wait this request's turn to mutate `host`'s profile catalog, counting
    /// the wait while it lasts (see [`Self::profile_edit_queue`]).
    ///
    /// The guard is OWNED so a caller can hand it to the task that finishes
    /// the mutation — the lock must outlive the handler that took it, or a
    /// cancelled request would release the queue while its edit is still in
    /// flight (see [`profiles::committed`]).
    ///
    /// Callers must have routed `host` first; see [`Self::profile_edit_lock`].
    async fn enter_profile_edit(&self, host: store::HostId) -> tokio::sync::OwnedMutexGuard<()> {
        let serialized = self.profile_edit_lock(host);
        self.profile_edit_queue
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let _queued = QueuedEdit(&self.profile_edit_queue);
        serialized.lock_owned().await
    }

    /// How many requests are queued on a profile-mutation lock right now.
    ///
    /// The observable the concurrency tests wait on instead of sleeping. The
    /// counting itself ships in every build; only this reader is test-only —
    /// see [`Self::profile_edit_queue`].
    #[cfg(test)]
    fn queued_profile_edits(&self) -> usize {
        self.profile_edit_queue
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Assemble the API and WebSocket routes that share the helm's application
/// state and security boundary.
///
/// The static UI deliberately does not live here: it must remain reachable
/// before a browser has authenticated. The control-plane routes are assembled
/// first and protected as one group; the one public exchange route is added
/// afterwards. Keeping both boundaries structural avoids relying on each
/// future route to remember whether it belongs inside authentication.
fn api_router(state: Arc<AppState>) -> Router {
    let protected = Router::new()
        .route(
            "/api/sessions",
            get(sessions::list_sessions).post(sessions::create_session),
        )
        .route(
            "/api/sessions/{id}/stop",
            axum::routing::post(sessions::stop_session),
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
            "/api/sessions/{id}/archive",
            axum::routing::post(sessions::archive_session),
        )
        .route(
            "/api/sessions/{id}",
            get(sessions::get_session).delete(sessions::delete_session),
        )
        .route(
            "/api/sessions/{id}/tabs",
            axum::routing::post(sessions::open_tab),
        )
        .route(
            "/api/sessions/{id}/tabs/{tab_id}",
            axum::routing::delete(sessions::close_tab),
        )
        .route("/api/sessions/{id}/term", get(terminal::term_ws))
        // The non-displacing attach lives at its own PATH rather than
        // behind a query flag, and the difference is the whole safety
        // property (PLAN_M6.md item 7). A flag is something an older helm
        // ignores while happily performing the displacing attach the
        // caller was trying to avoid — the browser has no handshake to
        // catch that with, and the build stamp it does have can be up to a
        // poll interval stale, which is exactly the window a rolled-back
        // helm lives in. A path an older helm does not serve cannot be
        // misread: the upgrade simply fails (a 404, or the UI's own
        // index.html from the static fallback — either way no WebSocket),
        // the attempt counts as a failure, the ladder carries on, and the
        // stamp check latches the mismatch moments later.
        .route(
            "/api/sessions/{id}/term/unowned",
            get(terminal::term_ws_if_unowned),
        )
        // Host management (PLAN_M6.md item 5). The two verb routes are
        // shaped like the session verbs (`/stop`, `/rename`) rather than as
        // PATCHes, for the reason `rename_session` records: each changes
        // exactly one thing, and naming the thing is clearer than inventing
        // a partial-update shape this API has nowhere else.
        .route("/api/hosts", get(hosts::list_hosts).post(hosts::add_host))
        // Provisioning is separate from registry management: probe is
        // discovery-first and non-mutating on absence, while provision and
        // update return run identities whose state is re-read after feed
        // bumps. Their in-flight exclusion lives in AppState, not in one
        // browser's operation lock.
        .route(
            "/api/hosts/probe",
            axum::routing::post(provisioning::probe_host),
        )
        .route(
            "/api/hosts/provision",
            axum::routing::post(provisioning::provision_host),
        )
        .route("/api/hosts/{id}", axum::routing::delete(hosts::remove_host))
        .route(
            "/api/hosts/{id}/update",
            axum::routing::post(provisioning::update_host),
        )
        .route(
            "/api/hosts/{id}/provisioning",
            get(provisioning::provisioning_state),
        )
        .route(
            "/api/hosts/{id}/destination",
            axum::routing::post(hosts::set_destination),
        )
        .route(
            "/api/hosts/{id}/adopt",
            axum::routing::post(hosts::adopt_host),
        )
        .route(
            "/api/hosts/{id}/retry",
            axum::routing::post(hosts::retry_host),
        )
        // Profiles hang off the HOST they belong to rather than sitting at
        // a top-level `/api/profiles` (PLAN_M6_75.md item 5): a profile id
        // only means anything on the supervisor that minted it, so a route
        // that did not name a host would be one whose ids collide across the
        // fleet with nothing to disambiguate them.
        .route(
            "/api/hosts/{id}/profiles",
            get(profiles::list_profiles).post(profiles::create_profile),
        )
        .route(
            "/api/hosts/{id}/profiles/{profile_id}",
            axum::routing::post(profiles::update_profile).delete(profiles::delete_profile),
        )
        // The invalidation feed (PLAN_M6_75.md item 5). A WebSocket like the
        // terminal routes, and served beside them for the same reason they
        // are here at all — one process, one port, one origin guard — but
        // with nothing else in common: it names no session, carries no data,
        // and holds no attachment.
        .route("/api/events", get(events::events_ws))
        // Applied to the ROUTES already assembled above. The exchange route
        // is added afterwards and is therefore structurally outside the
        // authenticated boundary rather than exempted inside middleware.
        .route_layer(axum::middleware::from_fn_with_state(
            Arc::clone(&state),
            auth::require_device_session,
        ));
    // Validation is protected like every other read, but its webview caller
    // must be able to distinguish the auth middleware's 401 from a transport
    // failure. Keeping CORS outside this small router makes that refusal
    // readable without widening any ordinary REST route.
    let desktop_device = Router::new()
        .route("/api/auth/device", get(auth::validate_device))
        .route_layer(axum::middleware::from_fn_with_state(
            Arc::clone(&state),
            auth::require_device_session,
        ))
        .route(
            "/api/auth/device",
            axum::routing::options(middleware::desktop_webview_preflight),
        )
        .layer(axum::middleware::from_fn(middleware::desktop_webview_cors));
    // Upload authentication remains mandatory, but CORS must wrap that
    // middleware so a custom-scheme webview can read its structured 401.
    let desktop_attachment = Router::new()
        .route(
            "/api/sessions/{id}/attachments",
            // SPEC.md's "no size cap in v1" is a promise about memory, not
            // axum's 2 MiB default. Other small control messages retain it.
            axum::routing::post(uploads::upload_attachment)
                .layer(axum::extract::DefaultBodyLimit::disable()),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            Arc::clone(&state),
            auth::require_device_session,
        ))
        // Preflight cannot carry the Authorization header it requests.
        .route(
            "/api/sessions/{id}/attachments",
            axum::routing::options(middleware::desktop_webview_preflight),
        )
        .layer(axum::middleware::from_fn(middleware::desktop_webview_cors));
    // Client-log authentication is layered identically to the attachment
    // upload above: mandatory device-session auth, wrapped in CORS so the
    // desktop webview can read the structured 401 rather than a generic
    // fetch failure — the one thing this route must never do is fail
    // silently for the caller reporting silent failures.
    let desktop_client_log = Router::new()
        .route(
            "/api/client-log",
            axum::routing::post(client_log::post_client_log),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            Arc::clone(&state),
            auth::require_device_session,
        ))
        .route(
            "/api/client-log",
            axum::routing::options(middleware::desktop_webview_preflight),
        )
        .layer(axum::middleware::from_fn(middleware::desktop_webview_cors))
        // Tighter than axum's 2 MiB default, sized from the endpoint's own
        // caps: the JSON is fully allocated before the handler's entry cap
        // can drop anything, so without this a spent budget still buys a
        // 2 MiB parse per request. The upload route makes the OPPOSITE
        // choice (no limit at all) for SPEC.md's no-size-cap promise, which
        // is why these two webview routes cannot share a router group.
        .layer(axum::extract::DefaultBodyLimit::max(
            client_log::MAX_BODY_BYTES,
        ));

    protected
        .merge(desktop_device)
        .merge(desktop_attachment)
        .merge(desktop_client_log)
        .route(
            "/api/auth/token",
            axum::routing::post(auth::exchange_token)
                .layer(axum::extract::DefaultBodyLimit::max(256))
                .options(middleware::desktop_webview_preflight)
                .layer(axum::middleware::from_fn(middleware::desktop_webview_cors)),
        )
        .with_state(state)
}

/// Where [`build_router`] gets the web UI it serves, decided once at
/// startup by [`select_ui_source`] and threaded through unchanged for the
/// life of the process (D12/D13).
///
/// `Dir` and `Embedded` answer a MISSING asset differently, and that split
/// is deliberate rather than something to reconcile away:
///
/// - `Dir` keeps `ServeDir`'s own behaviour — ANY miss, including a typo in
///   an asset path that would be a real bug, falls back to `index.html`.
/// - `Embedded` draws the asset/SPA-route line itself: an extension means a
///   concrete asset the browser expected to exist (404 on a miss), no
///   extension means a path reserved for a client-side route (`index.html`
///   on a miss) — the fallback a single-page-application router would need,
///   whether that is the UI as it exists today or one added later. The
///   current UI has no router of its own to claim such a path yet; this
///   rule exists so adding one is a client-side change with nothing to
///   update here.
///
/// In EITHER mode the shipped UI never actually asks for a missing asset —
/// its own build pins every asset URL it emits to a file that exists — so
/// this divergence is observable only to a client hand-crafting requests,
/// never to the UI as built.
#[derive(Debug)]
pub enum UiSource {
    /// A filesystem directory of a built UI, read at request time so
    /// editing it takes effect with no restart. Where `--ui-dist` (or a
    /// developer override) points.
    Dir(PathBuf),
    /// The UI tree this binary compiled in via `FARHELM_UI_DIST` (see
    /// [`embedded_ui`]). `'static` because `include_dir!` places it in the
    /// binary's own read-only data, not behind a value anyone constructs at
    /// runtime.
    Embedded(&'static include_dir::Dir<'static>),
    /// No UI is available — an ordinary developer build with no
    /// `--ui-dist`. The API still serves; only the static routes are
    /// absent.
    None,
}

/// Decide which [`UiSource`] a helm serves: an explicit `--ui-dist` wins
/// over whatever this build embedded, which in turn wins over serving no UI
/// at all.
///
/// `--ui-dist` outranks the embedded tree so a developer holding a
/// release-shaped build (`FARHELM_UI_DIST` was set, D13) can still point at
/// a freshly rebuilt `dx` output without recompiling `farhelm-helm` — the
/// build-time embedding and the runtime flag are independent inputs, not
/// one substituting for the other.
pub fn select_ui_source(
    flag: Option<PathBuf>,
    embedded: Option<&'static include_dir::Dir<'static>>,
) -> UiSource {
    match (flag, embedded) {
        (Some(dir), _) => UiSource::Dir(dir),
        (None, Some(dir)) => UiSource::Embedded(dir),
        (None, None) => UiSource::None,
    }
}

/// Warn once, at startup, that this process has no web UI to serve at all.
///
/// Split out of [`run_with_ready`]'s call site rather than inlined there, so
/// a test can drive it directly against all three [`UiSource`] variants —
/// asserting both that `None` logs and that `Dir`/`Embedded` stay silent —
/// without standing up a whole helm just to observe one log line.
fn warn_if_no_ui(ui: &UiSource) {
    if matches!(ui, UiSource::None) {
        // Not fatal — SPEC_impl.md's developer-build contract (D12: a build
        // without an embedded UI and without `--ui-dist` serves the API
        // alone) makes this a real, supported arrangement — but silent here
        // would leave "why is the browser getting nothing" to be
        // rediscovered from scratch every time a developer forgets
        // `--ui-dist` on a build with nothing embedded.
        warn!(
            "no web UI: this build embeds none and --ui-dist was not given; the API still serves"
        );
    }
}

/// Answer one request against the compiled-in UI tree, gating by method the
/// way `ServeDir` does for [`UiSource::Dir`] (the embedded fallback has to
/// replicate that gate itself, since it is a bare `axum` handler rather than
/// a `tower_http` service).
///
/// Only `GET` and `HEAD` are static-content methods; any other method is
/// `405 Method Not Allowed` with `Allow: GET,HEAD`, before any path lookup
/// happens at all — matching what `ServeDir` returns for `Dir`, asserted
/// directly against both sources below so the parity is checked rather than
/// assumed. `HEAD` returns the exact `GET` response, body included: status
/// and every header, content-type included, must match what `GET` would
/// have returned, so a `HEAD` probe learns the truth about a resource
/// without ever fetching it. The body is not dropped here — see the comment
/// below on why that has to happen one layer up.
fn serve_embedded(
    dir: &'static include_dir::Dir<'static>,
    method: &axum::http::Method,
    path: &str,
) -> axum::response::Response {
    if *method != axum::http::Method::GET && *method != axum::http::Method::HEAD {
        return (
            axum::http::StatusCode::METHOD_NOT_ALLOWED,
            [(axum::http::header::ALLOW, "GET,HEAD")],
        )
            .into_response();
    }
    // Deliberately the SAME response object for GET and HEAD, body and all.
    // axum's router computes `Content-Length` from the body's exact size
    // hint before it strips a HEAD response's body (see
    // `axum::routing::route::RouteFuture::poll`, which calls
    // `set_content_length` first and only then empties the body for
    // `Method::HEAD`) — that happens one layer up, at the fallback
    // dispatch this function's caller is wrapped in. Emptying the body
    // ourselves here would run BEFORE that router step sees it, so the
    // size hint it measures would already be zero and every HEAD response
    // would falsely claim `Content-Length: 0` instead of the real asset
    // size. Verified empirically for this axum version (0.8.9) by the HEAD
    // parity tests below asserting `Content-Length` against the fixture
    // file's actual length.
    serve_embedded_get(dir, path)
}

/// The `GET` half of [`serve_embedded`]: look one path up in the compiled-in
/// tree and apply [`UiSource::Embedded`]'s asset/SPA-route split (see that
/// variant's docstring for why it differs from `UiSource::Dir`'s fallback).
fn serve_embedded_get(
    dir: &'static include_dir::Dir<'static>,
    path: &str,
) -> axum::response::Response {
    // Every `include_dir!` entry is keyed by its path relative to the
    // embedded root, with no leading slash — but every path axum hands a
    // fallback handler starts with one, so it has to come off before a
    // lookup can match anything.
    let relative = path.trim_start_matches('/');
    // Percent-decoding happens once, here, before the lookup below, the
    // extension check that follows it, and the `mime_guess` call inside
    // `serve_embedded_bytes` all run — every one of those steps must agree
    // on the same literal characters the browser meant, or an asset whose
    // real name needs escaping in a URL (a literal space, say) would 404
    // even though `include_dir!` compiled it in under its actual name.
    let relative = match percent_encoding::percent_decode_str(relative).decode_utf8() {
        Ok(decoded) => decoded,
        // Not valid UTF-8 once decoded: no `include_dir!` entry — every one
        // is a Rust string literal — could ever match this, so it is a
        // miss, not a case worth panicking over or resolving lossily
        // against a path it might coincidentally collide with.
        Err(_) => return axum::http::StatusCode::NOT_FOUND.into_response(),
    };
    let relative = relative.as_ref();
    if relative.is_empty() {
        return serve_embedded_file(dir, "index.html");
    }
    if let Some(file) = dir.get_file(relative) {
        return serve_embedded_bytes(relative, file.contents());
    }
    // No exact file at this path: an extension means the browser asked for
    // a concrete asset that simply is not there (404, matching `Embedded`'s
    // contract); no extension means a path reserved for a client-side
    // route, which only `index.html` can take over and render — the
    // fallback a single-page-application router needs, for the current UI
    // or a future one. Nothing in the UI as it exists today actually owns
    // such a route; this rule just makes sure adding one later needs no
    // change here.
    match std::path::Path::new(relative).extension() {
        Some(_) => axum::http::StatusCode::NOT_FOUND.into_response(),
        None => serve_embedded_file(dir, "index.html"),
    }
}

/// Serve one exact path from the embedded tree, or 404 if it is not there.
///
/// Split out of [`serve_embedded`] because it has two callers: the ordinary
/// lookup, and both of that function's `index.html` fallbacks (the empty
/// path and the extension-less miss).
fn serve_embedded_file(
    dir: &'static include_dir::Dir<'static>,
    path: &str,
) -> axum::response::Response {
    match dir.get_file(path) {
        Some(file) => serve_embedded_bytes(path, file.contents()),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

/// Wrap one embedded file's bytes in a response carrying its guessed
/// content type — `mime_guess` maps `.js` to `text/javascript`, matching
/// what `ServeDir` returns for [`UiSource::Dir`] so a browser sees the same
/// type regardless of which source served it.
fn serve_embedded_bytes(path: &str, bytes: &'static [u8]) -> axum::response::Response {
    let content_type = mime_guess::from_path(path).first_or_octet_stream();
    (
        [(
            axum::http::header::CONTENT_TYPE,
            content_type.essence_str().to_string(),
        )],
        bytes,
    )
        .into_response()
}

/// Compose the protected API with the public static UI and the middleware
/// that must stamp every response.
///
/// Pulled out of `run()` so tests can drive the real middleware stack
/// in-process (via `tower::ServiceExt::oneshot`) against a scripted fleet,
/// instead of only exercising handlers directly and silently skipping the
/// origin guard and its response headers.
fn build_router(state: Arc<AppState>, ui: UiSource, port: u16) -> Router {
    let mut app = api_router(state);

    app = match ui {
        UiSource::Dir(dist) => {
            let serve = tower_http::services::ServeDir::new(&dist).fallback(
                tower_http::services::ServeFile::new(dist.join("index.html")),
            );
            app.fallback_service(serve)
        }
        UiSource::Embedded(dir) => app.fallback(move |req: axum::extract::Request| async move {
            serve_embedded(dir, req.method(), req.uri().path())
        }),
        UiSource::None => app,
    };

    app.layer(axum::middleware::from_fn(
        move |req: axum::extract::Request, next: axum::middleware::Next| {
            middleware::require_loopback_origin(port, req, next)
        },
    ))
    // OUTSIDE the origin guard, and that placement is the whole point:
    // layers apply outside-in, so this one decorates every response the
    // stack can produce — including the guard's own 403, which returns
    // before anything inner runs. "Every reply carries the stamp" has to
    // mean every reply, or the one path that skips it is the one a
    // skewed client hits first.
    .layer(axum::middleware::from_fn(middleware::stamp_build))
}

/// [`UiSource`]'s decision function and its `Embedded` variant's actual
/// serving behaviour — both against the tiny fixture tree committed at
/// `tests/fixtures/ui` rather than a real `dx` build output, since nothing
/// under test cares what a real UI looks like.
#[cfg(test)]
mod embedded_ui_tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode, header};
    use tower::ServiceExt;

    /// The committed fixture tree. A `static` rather than built per test:
    /// `include_dir!` output is `const` data, and every test here wants the
    /// same `&'static Dir<'static>` [`UiSource::Embedded`] itself requires.
    static FIXTURE_UI: include_dir::Dir<'static> =
        include_dir::include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/ui");

    /// The fixture `index.html`'s exact bytes, for exact-body assertions
    /// rather than a mere substring check — committed alongside
    /// [`FIXTURE_UI`] so a change to one always shows up as a diff to the
    /// other.
    const FIXTURE_INDEX_HTML: &[u8] = include_bytes!("../tests/fixtures/ui/index.html");
    /// The fixture `assets/app.js`'s exact bytes — see [`FIXTURE_INDEX_HTML`].
    const FIXTURE_APP_JS: &[u8] = include_bytes!("../tests/fixtures/ui/assets/app.js");

    /// The real router, serving [`FIXTURE_UI`] the way a release build's
    /// `UiSource::Embedded` actually would — built over `rest_harness`'s
    /// idle scripted fleet since nothing here is about sessions or hosts,
    /// only about what answers a static-asset path.
    async fn embedded_router() -> Router {
        let harness = rest_harness::idle_helm().await;
        // 7433 rather than reading it off the harness: `Harness::port` is
        // private to `rest_harness`, and every other test in this crate
        // that builds a request by hand already hardcodes the same
        // default (see e.g. `auth::tests`) rather than plumbing it out.
        build_router(
            Arc::clone(&harness.state),
            UiSource::Embedded(&FIXTURE_UI),
            7433,
        )
    }

    /// A `UiSource::Dir` router over a freshly written temp directory
    /// holding the same two fixture files [`FIXTURE_UI`] compiles in —
    /// built the way `auth.rs`'s `static_bundle_is_public` does — so a test
    /// can assert `ServeDir`'s ACTUAL behaviour side by side with
    /// `serve_embedded`'s, instead of assuming the two agree. Returns the
    /// backing `TempDir` too: it must outlive every request the caller
    /// makes against the router, or `ServeDir` reads a directory that no
    /// longer exists.
    async fn dir_router_over_fixtures() -> (Router, tempfile::TempDir) {
        let harness = rest_harness::idle_helm().await;
        let dist = tempfile::tempdir().unwrap();
        std::fs::write(dist.path().join("index.html"), FIXTURE_INDEX_HTML).unwrap();
        std::fs::create_dir(dist.path().join("assets")).unwrap();
        std::fs::write(dist.path().join("assets").join("app.js"), FIXTURE_APP_JS).unwrap();
        let router = build_router(
            Arc::clone(&harness.state),
            UiSource::Dir(dist.path().to_path_buf()),
            7433,
        );
        (router, dist)
    }

    /// A loopback-origin-valid request for `method` and `path` — the Host
    /// header `require_loopback_origin` demands of every request this
    /// router answers, static UI included.
    fn request(method: Method, path: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, "127.0.0.1:7433")
            .body(Body::empty())
            .unwrap()
    }

    /// A loopback-origin-valid GET for `path` — the common case of
    /// [`request`].
    fn get(path: &str) -> Request<Body> {
        request(Method::GET, path)
    }

    /// Spec: `/` serves the embedded tree's `index.html` verbatim, byte for
    /// byte, as `text/html` — the content type a browser needs to actually
    /// render it rather than offer it as a download.
    #[tokio::test]
    async fn root_serves_embedded_index() {
        let response = embedded_router().await.oneshot(get("/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html"
        );
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(body.as_ref(), FIXTURE_INDEX_HTML);
    }

    /// Spec: a path matching a real embedded file serves it verbatim with
    /// the `mime_guess`-derived content type — `.js` as `text/javascript`,
    /// the same type `ServeDir` would return for `UiSource::Dir`, so a
    /// browser sees identical types regardless of which source answered.
    #[tokio::test]
    async fn asset_path_serves_the_file_with_its_guessed_content_type() {
        let response = embedded_router()
            .await
            .oneshot(get("/assets/app.js"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/javascript"
        );
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(body.as_ref(), FIXTURE_APP_JS);
    }

    /// Spec: a percent-encoded path segment is decoded before the embedded
    /// tree is consulted — `include_dir!` entries are keyed by their real,
    /// unescaped names, so a request for the URL-escaped form of an asset
    /// whose actual name needs escaping (a literal space, here) must still
    /// resolve to it rather than 404.
    #[tokio::test]
    async fn percent_encoded_path_resolves_to_the_real_asset() {
        let response = embedded_router()
            .await
            .oneshot(get("/assets/space%20name.js"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/javascript"
        );
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(
            body.as_ref(),
            include_bytes!("../tests/fixtures/ui/assets/space name.js").as_slice()
        );
    }

    /// Spec: percent-decoding runs before the asset/SPA-route extension
    /// check too, not just before the `include_dir!` lookup. `%2E` decodes
    /// to a literal `.`, so `app%2Ejs` names a file that DOES have an
    /// extension once decoded — if the extension check ran on the raw,
    /// still-encoded path instead, `app%2Ejs` would look extensionless and
    /// wrongly fall back to `index.html` instead of resolving to the real
    /// `app.js`.
    #[tokio::test]
    async fn percent_encoded_dot_resolves_to_the_real_asset() {
        let response = embedded_router()
            .await
            .oneshot(get("/assets/app%2Ejs"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/javascript"
        );
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(body.as_ref(), FIXTURE_APP_JS);
    }

    /// The missing-file counterpart to
    /// [`percent_encoded_dot_resolves_to_the_real_asset`]: once `%2E` decodes
    /// to `.`, `missing%2Ejs` names a path WITH an extension that matches no
    /// compiled file, so it must take the extensioned-miss branch (a real
    /// 404) rather than the extensionless SPA fallback to `index.html`.
    #[tokio::test]
    async fn percent_encoded_dot_missing_asset_is_404() {
        let response = embedded_router()
            .await
            .oneshot(get("/assets/missing%2Ejs"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    /// Spec: `%FF` is not a valid UTF-8 byte on its own, so decoding it must
    /// answer a plain 404 — not panic, and not resolve against whatever byte
    /// sequence it happens to produce. `percent_decode_str(...).decode_utf8()`
    /// is exactly the fallible step `serve_embedded_get` guards for this; this
    /// test exercises it through the real router rather than only trusting
    /// that guard exists.
    #[tokio::test]
    async fn invalid_percent_encoding_is_404_not_a_panic() {
        let response = embedded_router().await.oneshot(get("/%FF")).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    /// Spec: a path with no extension that matches no compiled file answers
    /// with `index.html` — byte for byte, as `text/html` — rather than 404.
    /// Locks down Step 1's agreed fallback rule for single-page-application
    /// routes in general, not a claim that the UI as it exists today has
    /// deep links or history-dependent routing of its own to depend on it.
    #[tokio::test]
    async fn extensionless_miss_falls_back_to_index_for_spa_routing() {
        let response = embedded_router()
            .await
            .oneshot(get("/sessions/abc"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html"
        );
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(body.as_ref(), FIXTURE_INDEX_HTML);
    }

    /// Spec: a path WITH an extension that matches no compiled file is a
    /// genuine 404 — the divergence from `UiSource::Dir`'s ServeDir
    /// fallback that `UiSource`'s own docstring calls out. A typo in an
    /// asset URL must surface as a failed request, not silently render a
    /// page that looks like it loaded.
    #[tokio::test]
    async fn extensioned_miss_is_a_404() {
        let response = embedded_router()
            .await
            .oneshot(get("/assets/missing.js"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    /// Spec: a non-`GET`/`HEAD` method against the embedded fallback is
    /// `405 Method Not Allowed` naming the two methods that ARE allowed —
    /// the same contract `ServeDir` enforces for `UiSource::Dir`, asserted
    /// here for `Embedded` and below for `Dir` so the parity is checked
    /// rather than assumed.
    #[tokio::test]
    async fn post_to_embedded_ui_is_405_with_allow_header() {
        let response = embedded_router()
            .await
            .oneshot(request(Method::POST, "/"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers().get(header::ALLOW).unwrap(), "GET,HEAD");
    }

    /// Spec: `HEAD` against a real embedded asset returns exactly what
    /// `GET` would have — status, content-type, AND `Content-Length`
    /// included — with the body dropped. A `HEAD` probe exists precisely so
    /// a caller can learn that much without paying for the body; getting the
    /// headers wrong, `Content-Length` especially, would defeat the point of
    /// asking.
    ///
    /// `Content-Length` is asserted two ways: against the fixture file's own
    /// length (so the number is checked against ground truth, not just
    /// self-consistency) and against the sibling `GET`'s `Content-Length`
    /// (so a regression that moves both numbers together, but away from the
    /// truth, still fails). This pins the round-2 review fix: `serve_embedded`
    /// used to empty the body itself before returning, which ran ahead of
    /// axum's own router-level `Content-Length` computation (see that
    /// function's doc comment) and produced `Content-Length: 0` on every
    /// `HEAD` response instead of the real size.
    #[tokio::test]
    async fn head_asset_returns_get_headers_with_empty_body() {
        let router = embedded_router().await;
        let get_response = router.clone().oneshot(get("/assets/app.js")).await.unwrap();
        assert_eq!(get_response.status(), StatusCode::OK);
        let get_content_length = get_response
            .headers()
            .get(header::CONTENT_LENGTH)
            .expect("GET response must carry Content-Length")
            .clone();
        assert_eq!(get_content_length, FIXTURE_APP_JS.len().to_string());

        let response = router
            .oneshot(request(Method::HEAD, "/assets/app.js"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/javascript"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_LENGTH).unwrap(),
            &get_content_length,
            "HEAD's Content-Length must match GET's, not the emptied HEAD body"
        );
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert!(body.is_empty(), "HEAD must not return a body, got {body:?}");
    }

    /// Spec: `UiSource::Dir`'s `ServeDir` gates by method exactly the way
    /// `serve_embedded` now does for `Embedded` — the `Dir` half of the
    /// method-parity pair started above (`post_to_embedded_ui_is_405_with_allow_header`,
    /// `head_asset_returns_get_headers_with_empty_body`).
    #[tokio::test]
    async fn post_to_dir_ui_is_405_with_allow_header() {
        let (router, _dist) = dir_router_over_fixtures().await;
        let response = router.oneshot(request(Method::POST, "/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers().get(header::ALLOW).unwrap(), "GET,HEAD");
    }

    /// The `Dir` half of `head_asset_returns_get_headers_with_empty_body` —
    /// see that test's docstring, `Content-Length` assertions included.
    /// `ServeDir` sets `Content-Length` itself (it has to, for range
    /// requests), so this side was never at risk of the round-2 bug —
    /// asserted anyway so the parity between the two `UiSource` variants is
    /// checked, not assumed.
    #[tokio::test]
    async fn head_to_dir_ui_returns_get_headers_with_empty_body() {
        let (router, _dist) = dir_router_over_fixtures().await;
        let get_response = router.clone().oneshot(get("/assets/app.js")).await.unwrap();
        assert_eq!(get_response.status(), StatusCode::OK);
        let get_content_length = get_response
            .headers()
            .get(header::CONTENT_LENGTH)
            .expect("GET response must carry Content-Length")
            .clone();
        assert_eq!(get_content_length, FIXTURE_APP_JS.len().to_string());

        let response = router
            .oneshot(request(Method::HEAD, "/assets/app.js"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/javascript"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_LENGTH).unwrap(),
            &get_content_length,
            "HEAD's Content-Length must match GET's"
        );
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert!(body.is_empty(), "HEAD must not return a body, got {body:?}");
    }

    /// Spec: `--ui-dist` outranks whatever this build embedded, which in
    /// turn outranks serving nothing — `select_ui_source`'s whole contract,
    /// and pure enough to assert without a router at all.
    #[test]
    fn select_ui_source_prefers_flag_over_embedded_and_falls_back_to_none() {
        let flag = std::path::PathBuf::from("/some/dev/ui-dist");

        assert!(matches!(
            select_ui_source(Some(flag.clone()), Some(&FIXTURE_UI)),
            UiSource::Dir(dir) if dir == flag
        ));
        assert!(matches!(
            select_ui_source(None, Some(&FIXTURE_UI)),
            UiSource::Embedded(_)
        ));
        assert!(matches!(select_ui_source(None, None), UiSource::None));
    }

    /// Spec: [`warn_if_no_ui`] logs the exact missing-UI warning for
    /// [`UiSource::None`], and stays silent for `Dir`/`Embedded` — the
    /// assertion `run_with_ready`'s previously-inlined `if` made impossible
    /// to drive directly. The message is a unique sentence found nowhere
    /// else in this crate, so filtering the process-global capture buffer
    /// (see `crate::test_capture`'s own docs on why it must be global) by
    /// exact content is as good as filtering by this test's own identity.
    #[test]
    fn warn_if_no_ui_warns_only_for_none() {
        const MESSAGE: &str =
            "no web UI: this build embeds none and --ui-dist was not given; the API still serves";
        let events = crate::test_capture::install();

        warn_if_no_ui(&UiSource::Dir(std::path::PathBuf::from(
            "/some/dev/ui-dist",
        )));
        warn_if_no_ui(&UiSource::Embedded(&FIXTURE_UI));
        assert!(
            crate::test_capture::matching(&events, MESSAGE).is_empty(),
            "Dir and Embedded must not log the missing-UI warning"
        );

        warn_if_no_ui(&UiSource::None);
        let hits = crate::test_capture::matching(&events, MESSAGE);
        assert_eq!(
            hits.len(),
            1,
            "None must log the missing-UI warning exactly once"
        );
        assert_eq!(hits[0].field("message"), Some(MESSAGE));
        assert_eq!(hits[0].level, "WARN");
    }

    /// Spec: [`middleware::stamp_build`] is layered OUTSIDE the whole router
    /// (see [`build_router`]'s own comment on why), so it must reach a
    /// perfectly ordinary successful static-asset response too, not just the
    /// API routes it was originally added for.
    #[tokio::test]
    async fn successful_embedded_response_carries_build_stamp_header() {
        let response = embedded_router().await.oneshot(get("/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(BUILD_STAMP_HEADER).unwrap(),
            env!("CARGO_PKG_VERSION")
        );
    }

    /// Spec: [`middleware::require_loopback_origin`] rejects a request whose
    /// `Host` names a foreign origin — the DNS-rebinding defense
    /// `middleware`'s own tests pin exhaustively for `origin_is_allowed` — and
    /// [`middleware::stamp_build`]'s outer layering means even THAT rejection
    /// carries the build stamp: a confused, DNS-rebound client asking why its
    /// requests are failing needs the same skew signal a working one gets.
    #[tokio::test]
    async fn foreign_host_is_rejected_but_still_carries_build_stamp_header() {
        let foreign_request = Request::builder()
            .method(Method::GET)
            .uri("/")
            .header(header::HOST, "evil.example:7433")
            .body(Body::empty())
            .unwrap();
        let response = embedded_router()
            .await
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

/// Run the helm until the process is killed: open helm.db, apply any
/// `--ensure-hosts` floor, start a connection actor per registered host,
/// open the private token-control endpoint, and serve the API and UI on
/// loopback.
///
/// Startup order is deliberate at four points, each for a different
/// reason:
///
/// - The listener is bound FIRST, so the likely failure (port busy because
///   a helm is already running) happens before anything else has been set
///   up or written.
/// - `--ensure-hosts` is ingested BEFORE the manager starts, so the
///   guaranteed hosts have actors from the first moment rather than after a
///   reconcile — and so a bad file fails startup before anything is
///   serving. See [`ensure::ingest`] for its all-or-nothing contract.
/// - The manager returns as soon as its actors are SPAWNED, not once they
///   have connected. A down host must never delay the helm's startup: it is
///   simply a host in a non-connected state, which the API exposes for the
///   forthcoming UI to draw.
/// - The token-control socket is bound before HTTP serving begins, so a
///   successful browser request cannot race a separate `token rotate` into
///   the offline fallback and miss live WebSocket revocation.
///
/// Returns only on a fatal error. There is no graceful-shutdown path, and
/// none is needed: SPEC.md's whole durability promise is that killing the
/// helm does nothing to any session.
pub async fn run(args: HelmArgs) -> anyhow::Result<()> {
    run_with_ready(args, None, None).await
}

/// Run an embedded helm and report its bound address once every serving
/// dependency is ready.
///
/// The desktop shell chooses its documented stable port, but it must not launch the UI
/// until the HTTP listener, durable token, local-host actor, and token-control
/// socket all exist. A synchronous channel keeps that startup boundary out of
/// the Dioxus runtime and, unlike parsing stdout, cannot confuse another log
/// line for readiness. The explicit shutdown receiver gives `DesktopBootstrap`
/// a teardown path it can join; dropping its sender carries the same owner-
/// disappeared meaning as sending the signal.
pub async fn run_embedded(
    args: HelmArgs,
    ready: std::sync::mpsc::Sender<SocketAddr>,
    shutdown: tokio::sync::oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    run_with_ready(args, Some(ready), Some(shutdown)).await
}

/// Shared process and embedded-app serving path. There is deliberately one
/// startup sequence so the desktop cannot acquire a weaker auth or transport
/// boundary than `farhelm helm run`.
async fn run_with_ready(
    args: HelmArgs,
    ready: Option<std::sync::mpsc::Sender<SocketAddr>>,
    shutdown: Option<tokio::sync::oneshot::Receiver<()>>,
) -> anyhow::Result<()> {
    let state_dir = match args.state_dir.clone() {
        Some(dir) => dir,
        None => farhelm_supervisor::default_state_dir()?,
    };
    // 0700: this directory holds helm.db, the token-control socket, and ssh
    // ControlMaster sockets. Each grants the user's authority when reached.
    farhelm_supervisor::ensure_private_dir(&state_dir).await?;

    // Bind before creating anything on a host. A busy port is the likely
    // failure here (a helm is already running), and failing afterwards
    // would strand a live agent session on every retry.
    // Loopback only — deliberately not configurable in v1 (SPEC.md).
    let bind_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, args.port));
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("binding {bind_addr}"))?;
    // Use the bound address rather than the requested one. This keeps
    // programmatic callers that request port 0 coherent: Host validation
    // and the printed URL must name the ephemeral port the OS chose.
    let addr = listener
        .local_addr()
        .context("reading bound helm address")?;

    let store = store::HelmStore::open(&state_dir.join("helm.db")).await?;
    if let Some(path) = args.ensure_hosts.as_deref() {
        ensure::ingest(&store, path).await?;
    }
    let manager = manager::ConnectionManager::start(
        store.clone(),
        Arc::new(manager::SystemTransport::new(&state_dir)),
        manager::Cadence::default(),
    )
    .await?;

    let state = Arc::new(AppState::new(manager, store, state_dir.clone())?);
    // First run owns token creation. Browser serving must never begin with a
    // database whose bootstrap secret exists only after somebody invokes the
    // separate `token show` command.
    state.auth.token().await?;
    let mut token_control = token_control::serve(&state_dir, state.auth.clone()).await?;
    let ui = select_ui_source(args.ui_dist.clone(), embedded_ui());
    warn_if_no_ui(&ui);
    let app = build_router(Arc::clone(&state), ui, addr.port());

    if let Some(ready) = ready {
        let _ = ready.send(addr);
    }

    // Printed on stdout, not logged: the README tells the user to open
    // this URL, and tracing goes to stderr behind an env filter.
    println!("farhelm helm: http://{addr}/");
    let embedded_shutdown = async move {
        match shutdown {
            Some(receiver) => {
                // Sender cancellation means the desktop owner disappeared;
                // it is the same lifetime boundary as an explicit signal.
                let _ = receiver.await;
            }
            None => std::future::pending::<()>().await,
        }
    };
    tokio::select! {
        result = axum::serve(listener, app) => result.context("serving helm HTTP")?,
        result = token_control.failed() => result?,
        () = embedded_shutdown => {}
    }
    state.manager.shutdown();
    Ok(())
}

/// The response header carrying this helm's build version to the browser
/// (PLAN_M6.md item 6).
///
/// Lowercase because `HeaderName::from_static` accepts nothing else, and
/// spelled out in one place because the UI matches on the same literal
/// (farhelm-ui's `skew::BUILD_HEADER`) — a cross-language coupling with
/// nothing but this pairing to hold it together, which is why the browser
/// suite asserts the header by name.
const BUILD_STAMP_HEADER: &str = "x-farhelm-build";

/// Render an error as an HTTP response whose body is the error chain in
/// full and whose status reflects what the supervisor actually classified.
///
/// The status mapping is only as honest as the supervisor's own
/// classification: `NotFound` maps to 404, `InvalidRequest` to 400, and
/// `Conflict` (PLAN_M3.md item 6 — an intent key reused with a different
/// fingerprint) to 409, each when a `SupervisorClient` request surfaces a
/// [`SupervisorError`] with that kind anywhere in its chain (the walk
/// mirrors the supervisor's own `error_kind` helper, for the same
/// reason — a `SupervisorError` can sit under context layers this client
/// or an intermediate caller added). Anything else — no `SupervisorError`
/// in the chain at all, or one explicitly carrying `Internal` — is a 500:
/// the honest default for a failure the caller could not have avoided by
/// sending a different request.
///
/// PLAN_M6.md item 5's host-management and routing refusals join the same
/// table through two further downcasts. [`store::HostStoreError`]: an
/// unknown host id is a 404, an unusable destination is a 400 (the caller
/// sent something `ssh` cannot use), and everything else the registry
/// refuses — a duplicate destination, the immovable local row, a lost
/// identity compare-and-swap, an identity another row claimed, a row
/// reconfigured mid-decision, a session two hosts both claim — is a 409,
/// because each is the same shape of failure: the request was well formed
/// and conflicts with the fleet as it stands. [`manager::ManagerError`]
/// carries the same split for the decisions the manager owns rather than
/// the store. A non-connected host's refusal reaches this function as an
/// ordinary `SupervisorError` carrying `Conflict` (see `refusal_text`), for
/// the same reading.
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
    // ONE status decision and ONE response construction. The three families
    // are consulted in order of specificity — a registry refusal, then a
    // manager refusal, then whatever the supervisor said — and each yields
    // only a CODE, because three copies of "format the chain and return it"
    // is three places for the body to drift apart.
    let registry = e
        .chain()
        .find_map(|c| c.downcast_ref::<store::HostStoreError>())
        .map(|refusal| match refusal {
            store::HostStoreError::HostNotFound(_) => axum::http::StatusCode::NOT_FOUND,
            store::HostStoreError::InvalidDestination(_) => axum::http::StatusCode::BAD_REQUEST,
            store::HostStoreError::DuplicateDestination(_)
            | store::HostStoreError::LocalHostImmutable
            | store::HostStoreError::IdentityMismatch { .. }
            | store::HostStoreError::IdentityClaimed { .. }
            | store::HostStoreError::StaleAttempt { .. }
            // Two hosts claiming one session id: well-formed request,
            // incoherent fleet. 409 rather than 500 because the user CAN
            // act on it (remove whichever entry does not belong) and the
            // error names both candidates so they know which.
            | store::HostStoreError::SessionOwnerAmbiguous { .. } => {
                axum::http::StatusCode::CONFLICT
            }
        });
    let managed = || {
        e.chain()
            .find_map(|c| c.downcast_ref::<manager::ManagerError>())
            .map(|refusal| match refusal {
                manager::ManagerError::NoSuchHost(_) => axum::http::StatusCode::NOT_FOUND,
                // Both are "the host is not in the state this verb needs",
                // which a client answers by re-rendering the host and
                // offering whatever it is actually asking for now.
                manager::ManagerError::NotAwaitingAdoption { .. }
                | manager::ManagerError::AdoptionSuperseded { .. } => {
                    axum::http::StatusCode::CONFLICT
                }
            })
    };
    let supervised = || match e
        .chain()
        .find_map(|c| c.downcast_ref::<SupervisorError>())
        .map(|s| s.kind)
    {
        Some(ErrorKind::NotFound) => axum::http::StatusCode::NOT_FOUND,
        Some(ErrorKind::InvalidRequest) => axum::http::StatusCode::BAD_REQUEST,
        Some(ErrorKind::Internal) | None => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        // PLAN_M3.md item 6: an intent key reused with a different
        // fingerprint. 409 is the standard HTTP reading of "this identifier
        // already means something else"; this function's own docstring
        // above is where the full status-mapping table lives.
        Some(ErrorKind::Conflict) => axum::http::StatusCode::CONFLICT,
        Some(ErrorKind::Unauthorized) => axum::http::StatusCode::UNAUTHORIZED,
    };
    let status = registry.or_else(managed).unwrap_or_else(supervised);
    // The UI shows this body verbatim.
    (status, format!("{e:#}")).into_response()
}

#[cfg(test)]
mod tests {
    use super::SupervisorError;

    /// A `#[derive(Parser)]` shim purely so this test can call
    /// `try_parse_from` — `HelmArgs` itself is `#[derive(Args)]`, meant to
    /// be `#[command(flatten)]`ed into the real `farhelm` CLI, and only a
    /// top-level `Parser` exposes clap's parsing entry point.
    #[derive(clap::Parser, Debug)]
    struct Wrapper {
        #[command(flatten)]
        args: crate::HelmArgs,
    }

    /// The argv session and transport flags are GONE (PLAN_M6.md item 5),
    /// and `--ensure-hosts` is what replaced the transport half.
    ///
    /// Pinned as a test rather than left to the type, because a flag that
    /// quietly came back would be worse than one that never left: a helm
    /// now drives every registered host at once, so `--ssh` could only ever
    /// name one of them, and `--cwd`/`--agent` would be a creation path
    /// that bypasses the host selection the create API exists to make
    /// explicit. clap's own grammar is the only thing that catches a
    /// reintroduction, so it is asserted here directly.
    #[test]
    fn the_dropped_argv_session_flags_are_refused_and_ensure_hosts_replaces_them() {
        use clap::Parser;

        for dropped in [
            "--ssh",
            "--cwd",
            "--agent",
            "--title",
            "--remote-farhelm",
            "--remote-state-dir",
        ] {
            let err = Wrapper::try_parse_from(["farhelm", dropped, "value"])
                .expect_err("{dropped} must no longer be a helm flag");
            assert_eq!(
                err.kind(),
                clap::error::ErrorKind::UnknownArgument,
                "{dropped} must be refused as unknown, not merely ignored"
            );
        }

        let parsed = Wrapper::try_parse_from(["farhelm", "--ensure-hosts", "/etc/farhelm.json5"])
            .expect("--ensure-hosts is the flag that replaced them");
        assert_eq!(
            parsed.args.ensure_hosts,
            Some(std::path::PathBuf::from("/etc/farhelm.json5"))
        );
    }

    /// `http_error`'s downcast must find a [`SupervisorError`] under a
    /// `.context(...)` layer, not just at the root — an intermediate
    /// caller (`connect_supervisor`, say) is free to add its own context
    /// on the way out, and the status mapping must survive that. Spec:
    /// `NotFound` → 404 regardless of how many context layers sit on top.
    #[test]
    fn http_error_maps_context_wrapped_not_found_to_404() {
        let err = anyhow::Error::new(SupervisorError {
            kind: farhelm_proto::ErrorKind::NotFound,
            message: "no such session: s1".to_string(),
        })
        .context("attaching to terminal");
        let response = super::http_error(err);
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    }

    /// Spec: a `SupervisorError` explicitly carrying `Internal` maps to
    /// 500 — the honest default for a failure the caller could not have
    /// avoided by sending a different request, distinct from `NotFound`/
    /// `InvalidRequest` mapping to their own codes above.
    #[test]
    fn http_error_maps_internal_supervisor_error_to_500() {
        let err = anyhow::Error::new(SupervisorError {
            kind: farhelm_proto::ErrorKind::Internal,
            message: "tmux hiccup".to_string(),
        });
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
    #[test]
    fn http_error_maps_conflict_supervisor_error_to_409() {
        let err = anyhow::Error::new(SupervisorError {
            kind: farhelm_proto::ErrorKind::Conflict,
            message: "intent key already used with a different request".to_string(),
        });
        let response = super::http_error(err);
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
    }

    /// PLAN_M7.md item 2's authorization refusal maps to 401 so a client
    /// can distinguish rejected credentials from a malformed request or a
    /// supervisor fault.
    #[test]
    fn http_error_maps_unauthorized_supervisor_error_to_401() {
        let err = anyhow::Error::new(SupervisorError {
            kind: farhelm_proto::ErrorKind::Unauthorized,
            message: "session credential rejected".to_string(),
        });
        let response = super::http_error(err);
        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    /// Spec: an error chain with no `SupervisorError` anywhere in it — a
    /// bare `anyhow` error from a layer that never classified anything —
    /// must default to 500 rather than panicking or silently guessing a
    /// more specific status.
    #[test]
    fn http_error_maps_unclassified_error_to_500() {
        let err = anyhow::anyhow!("supervisor connection closed");
        let response = super::http_error(err);
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
