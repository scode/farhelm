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

mod client;
pub use client::{
    CreateExtras, PeerHello, SessionListing, SessionPage, SupervisorClient, SupervisorError,
    TermDetachSignal, TermEvent, TermStream,
};

/// The merged multi-host session list and the helm-level cursor that pages
/// it — what `GET /api/sessions` is built out of.
mod aggregate;

/// `--ensure-hosts`: the JSON5 floor under the registry, applied once
/// before serving starts.
mod ensure;

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
/// scoped to the attachment route alone.
mod middleware;

/// The session REST surface — the list, the owner-lookup routing behind
/// every operation on one session, and the handlers themselves.
mod sessions;

/// The ssh argv the remote transport is built out of, and the handshake
/// failure only that transport can explain.
mod ssh;

pub mod store;

/// The terminal WebSocket: the browser's end of an attachment.
mod terminal;

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
    /// the ssh ControlMaster sockets, and — in the ordinary single-machine
    /// arrangement — the local supervisor's socket the reserved local host
    /// row is reached through.
    #[arg(long)]
    pub state_dir: Option<PathBuf>,

    /// Directory with the built web UI (index.html + assets). Without
    /// it the API still serves; the UI returns 404.
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
/// into it. The session, upload, terminal, and host handlers all reach for
/// one or both, and none of them holds a connection of its own — see this
/// crate's docs for why the single-client `AppState` this replaced could
/// not survive multi-host. (The stateless routes — the CORS preflight, the
/// middleware layers — take no state at all and are not part of this.)
struct AppState {
    manager: Arc<manager::ConnectionManager>,
    store: store::HelmStore,
}

/// Assemble the routes, optional static UI service, and loopback-origin
/// middleware that `run()` serves.
///
/// Pulled out of `run()` so tests can drive the real middleware stack
/// in-process (via `tower::ServiceExt::oneshot`) against a scripted fleet,
/// instead of only exercising handlers directly and silently skipping the
/// origin guard and its response headers.
fn build_router(state: Arc<AppState>, ui_dist: Option<&std::path::Path>, port: u16) -> Router {
    let mut app = Router::new()
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
        .route(
            "/api/sessions/{id}/attachments",
            // SPEC.md's "no size cap in v1" is a promise about MEMORY,
            // not about axum's own 2 MiB default request-body limit —
            // this route disables that default so a large screenshot or
            // recording is refused nowhere in the helm at all, while every
            // other route (small JSON bodies) keeps the default's
            // protection against a runaway control-message body.
            axum::routing::post(uploads::upload_attachment)
                .options(middleware::attachment_preflight)
                .layer(axum::extract::DefaultBodyLimit::disable())
                // Scoped to this ONE route rather than the router: it is
                // the only endpoint a cross-origin caller has any reason
                // to reach (see `attachment_cors`), and a CORS header on
                // the session list or the delete route would widen what a
                // custom-scheme page can read for no benefit.
                .layer(axum::middleware::from_fn(middleware::attachment_cors)),
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
        .route("/api/hosts/{id}", axum::routing::delete(hosts::remove_host))
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
        .with_state(state);

    if let Some(dist) = ui_dist {
        let serve = tower_http::services::ServeDir::new(dist).fallback(
            tower_http::services::ServeFile::new(dist.join("index.html")),
        );
        app = app.fallback_service(serve);
    }

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

/// Run the helm until the process is killed: open helm.db, apply any
/// `--ensure-hosts` floor, start a connection actor per registered host,
/// and serve the API and UI on loopback.
///
/// Startup order is deliberate at three points, each for a different
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
///
/// Returns only on a fatal error. There is no graceful-shutdown path, and
/// none is needed: SPEC.md's whole durability promise is that killing the
/// helm does nothing to any session.
pub async fn run(args: HelmArgs) -> anyhow::Result<()> {
    let state_dir = match args.state_dir.clone() {
        Some(dir) => dir,
        None => farhelm_supervisor::default_state_dir()?,
    };
    // 0700: this directory holds helm.db and ssh ControlMaster sockets.
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

    let app = build_router(
        Arc::new(AppState { manager, store }),
        args.ui_dist.as_deref(),
        addr.port(),
    );

    // Printed on stdout, not logged: the README tells the user to open
    // this URL, and tracing goes to stderr behind an env filter.
    println!("farhelm helm: http://{addr}/");
    // Loopback keeps other MACHINES out, not other local accounts: until
    // the web token lands (M7, SPEC.md's Security section), any process
    // on this machine can drive the API — which includes launching
    // arbitrary commands as this user. Said out loud rather than left
    // for a security audit to rediscover.
    tracing::warn!(
        "the API on {addr} is unauthenticated in M1: any local user on this machine can \
         create and drive sessions (the web token that closes this arrives in a later milestone)"
    );
    axum::serve(listener, app).await?;
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
/// intended reader is the user's own UI. Note the honest caveat: until the
/// web token lands (M7), the loopback port is reachable by every local
/// account, so "no untrusted caller" is not yet true — which is a reason
/// to keep credentials out of error text (see the invocation-parse context
/// in the supervisor), not to strip detail the user needs. The body is
/// displayed as text, never interpreted, which is what makes it safe to
/// pass a remote supervisor's message through verbatim.
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
