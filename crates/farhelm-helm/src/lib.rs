//! The helm: Farhelm's single control-plane process.
//!
//! Per SPEC.md, exactly one helm runs at a time. It connects to
//! supervisors (locally over their unix socket, remotely over the user's
//! own ssh running `farhelm internal stdio`), aggregates their sessions,
//! and serves the UI and API over loopback HTTP/WS. It holds no
//! authoritative session state — supervisors are the authority.
//!
//! M1 scope: exactly one supervisor connection (local or one ssh host),
//! chosen by CLI flags; the host registry and multi-host aggregation are
//! M6 (PLAN.md). The loopback-only bind is enforced here — SPEC.md's
//! security posture says the helm refuses non-loopback addresses in v1,
//! and this code simply never binds anything else.

use anyhow::Context;
use axum::{
    Router,
    extract::{Path as AxPath, Query, State, WebSocketUpgrade, ws},
    response::IntoResponse,
    routing::get,
};
use clap::Args;
use farhelm_proto::ErrorKind;
use serde::Deserialize;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tracing::{error, info};

mod client;
pub use client::{SupervisorClient, SupervisorError, TermEvent};

/// CLI arguments for `farhelm helm run`. Lives here (not in the bin
/// crate) so the helm's surface and its implementation evolve together.
#[derive(Args, Debug, Clone)]
pub struct HelmArgs {
    /// Loopback port for the web UI and API.
    #[arg(long, default_value_t = 7433)]
    pub port: u16,

    /// State directory (default: ~/.local/state/farhelm). Holds ssh
    /// ControlMaster sockets; also locates the local supervisor's socket
    /// when --ssh is not given.
    #[arg(long)]
    pub state_dir: Option<PathBuf>,

    /// SSH destination of the host to drive (e.g. user@host). Omit to
    /// use a supervisor on this machine.
    #[arg(long)]
    pub ssh: Option<String>,

    /// Command for the farhelm binary on the remote host.
    #[arg(long, default_value = "farhelm")]
    pub remote_farhelm: String,

    /// State directory on the remote host (defaults to the remote's own
    /// default). A `String`, not a `PathBuf`: this path is handed to the
    /// remote shell as ssh argv text and never touches the local
    /// filesystem, so there is no reason to parse it as a local path — and
    /// `String` lets clap reject a non-UTF-8 value right here, before it
    /// can survive to a lossy conversion deep in the ssh argv builder.
    /// This is ssh's own textual boundary, not farhelm-proto's wire
    /// contract: this string never crosses the protocol (contrast
    /// `ControlMsg::CreateSession::cwd`, which does and has its UTF-8-only
    /// contract documented in farhelm-proto's crate docs).
    #[arg(long)]
    pub remote_state_dir: Option<String>,

    /// Directory with the built web UI (index.html + assets). Without
    /// it the API still serves; the UI returns 404.
    #[arg(long)]
    pub ui_dist: Option<PathBuf>,

    /// Create a session at startup in this working directory (on the
    /// target host). PLAN_M1.md's argv-driven creation: these flags feed
    /// the same creation API the UI will use, never a side door.
    #[arg(long, requires = "agent")]
    pub cwd: Option<String>,

    /// Agent invocation for the startup session (e.g. "claude").
    #[arg(long, requires = "cwd")]
    pub agent: Option<String>,

    /// Title for the startup session.
    #[arg(long)]
    pub title: Option<String>,
}

/// What the axum handlers share. One supervisor client, because M1 drives
/// exactly one host; M6's registry turns this into a map keyed by host id
/// without the handlers changing shape.
struct AppState {
    client: Arc<SupervisorClient>,
}

/// Assemble the routes, optional static UI service, and loopback-origin
/// middleware that `run()` serves.
///
/// Pulled out of `run()` so tests can drive the real middleware stack
/// in-process (via `tower::ServiceExt::oneshot`) against a scripted
/// `SupervisorClient`, instead of only exercising handlers directly and
/// silently skipping the origin guard and its response headers.
fn build_router(
    client: Arc<SupervisorClient>,
    ui_dist: Option<&std::path::Path>,
    port: u16,
) -> Router {
    let mut app = Router::new()
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/{id}/term", get(term_ws))
        .with_state(Arc::new(AppState { client }));

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

/// Run the helm until the process is killed: connect to the one
/// supervisor, optionally create the argv-specified startup session, and
/// serve the API and UI on loopback.
///
/// Startup order is deliberate and worth preserving. The listener is bound
/// before anything is created on a host, so the likely failure (port busy
/// because a helm is already running) happens before a session exists; the
/// reverse order would strand a live agent on every failed retry.
///
/// Returns only on a fatal error. There is no graceful-shutdown path, and
/// none is needed: SPEC.md's whole durability promise is that killing the
/// helm does nothing to any session.
pub async fn run(args: HelmArgs) -> anyhow::Result<()> {
    let state_dir = match args.state_dir.clone() {
        Some(dir) => dir,
        None => farhelm_supervisor::default_state_dir()?,
    };
    // 0700: this directory holds ssh ControlMaster sockets.
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

    let client = connect_supervisor(&args, &state_dir).await?;

    if let (Some(cwd), Some(agent)) = (&args.cwd, &args.agent) {
        let session = client
            .create_session(cwd, agent, args.title.clone(), 80, 24)
            .await
            .context("creating startup session")?;
        info!(id = %session.id, title = %session.title, "startup session created");
    }

    let app = build_router(client, args.ui_dist.as_deref(), addr.port());

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
        v.to_str().is_ok_and(|o| {
            is_loopback_authority(o) || o.starts_with("dioxus://") || o.starts_with("wry://")
        })
    });

    host_ok && origin_ok
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
/// The UTF-8 requirement enforced below is specific to this ssh path;
/// local-only mode (no `--ssh`) keeps native `OsString` state paths and
/// still tolerates non-UTF-8 homes (see `farhelm_supervisor::default_state_dir`).
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
        dest.to_string(),
        "--".to_string(),
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

/// Establish the supervisor transport per flags: ssh exec channel when
/// --ssh is given, the local unix socket otherwise. Both produce the
/// same reader/writer pair — transport-blind from here on.
async fn connect_supervisor(
    args: &HelmArgs,
    state_dir: &std::path::Path,
) -> anyhow::Result<Arc<SupervisorClient>> {
    match &args.ssh {
        None => {
            let stream = farhelm_supervisor::service::connect(state_dir).await?;
            let (r, w) = tokio::io::split(stream);
            SupervisorClient::start(r, w).await
        }
        Some(dest) => {
            let control_path = state_dir.join("ssh-cm-%C");
            let mut cmd = tokio::process::Command::new("ssh");
            cmd.args(ssh_args(
                dest,
                &control_path,
                &args.remote_farhelm,
                args.remote_state_dir.as_deref(),
            )?);
            // stderr inherits: ssh's own diagnostics (auth failures,
            // unreachable host) go to the user's terminal untouched —
            // they are the actionable error SPEC.md wants surfaced.
            let mut child = cmd
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .context("spawning ssh")?;
            let stdout = child.stdout.take().expect("piped stdout");
            let stdin = child.stdin.take().expect("piped stdin");
            // The child handle is parked in a reaper task: if ssh dies,
            // the frame reader sees EOF and the client surfaces it.
            tokio::spawn(async move {
                let _ = child.wait().await;
            });
            SupervisorClient::start(stdout, stdin).await
        }
    }
}

/// `GET /api/sessions` — the supervisor's list, passed through unchanged.
/// The helm caches nothing before M6; supervisors are the authority
/// (SPEC.md). The last-known-session cache that survives helm restarts
/// arrives with M6's registry and stale-cache semantics (PLAN.md) — with
/// one always-connected supervisor there is nothing for a cache to add.
async fn list_sessions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.client.list_sessions().await {
        Ok(sessions) => axum::Json(sessions).into_response(),
        Err(e) => http_error(e),
    }
}

#[derive(Deserialize)]
struct CreateReq {
    cwd: String,
    invocation: String,
    title: Option<String>,
    #[serde(default = "default_cols")]
    cols: u16,
    #[serde(default = "default_rows")]
    rows: u16,
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

/// `POST /api/sessions` — the creation API SPEC_impl.md calls the one true
/// path. The CLI's `--cwd/--agent` flags and any future UI dialog both
/// land on the same supervisor call this reaches.
async fn create_session(
    State(state): State<Arc<AppState>>,
    axum::Json(req): axum::Json<CreateReq>,
) -> impl IntoResponse {
    match state
        .client
        .create_session(&req.cwd, &req.invocation, req.title, req.cols, req.rows)
        .await
    {
        Ok(session) => axum::Json(session).into_response(),
        Err(e) => http_error(e),
    }
}

/// Render an error as an HTTP response whose body is the error chain in
/// full and whose status reflects what the supervisor actually classified.
///
/// The status mapping is only as honest as the M1 supervisor's own
/// classification: `NotFound` and `InvalidRequest` map to 404/400
/// respectively when a `SupervisorClient` request surfaces a
/// [`SupervisorError`] with that kind anywhere in its chain (the walk
/// mirrors the supervisor's own `error_kind` helper, for the same
/// reason — a `SupervisorError` can sit under context layers this client
/// or an intermediate caller added). Anything else — no `SupervisorError`
/// in the chain at all, or one explicitly carrying `Internal` — is a 500:
/// the honest default for a failure the caller could not have avoided by
/// sending a different request.
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
    let status = match e
        .chain()
        .find_map(|c| c.downcast_ref::<SupervisorError>())
        .map(|s| s.kind)
    {
        Some(ErrorKind::NotFound) => axum::http::StatusCode::NOT_FOUND,
        Some(ErrorKind::InvalidRequest) => axum::http::StatusCode::BAD_REQUEST,
        Some(ErrorKind::Internal) | None => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
    };
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
#[derive(Deserialize)]
struct TermQuery {
    #[serde(default = "default_cols")]
    cols: u16,
    #[serde(default = "default_rows")]
    rows: u16,
}

/// Terminal WebSocket: binary frames are terminal bytes in both
/// directions; text frames are small JSON control messages (client →
/// resize; server → detached notice). This is the browser-facing twin of
/// the proto data channel, kept equally dumb.
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

/// Client-to-helm control messages on a terminal socket. Text frames only
/// — binary is always terminal input — and an unparseable one is ignored
/// rather than fatal, so adding a message type does not break older
/// clients.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WsClientMsg {
    Resize { cols: u16, rows: u16 },
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
async fn serve_term(
    state: Arc<AppState>,
    session_id: String,
    q: TermQuery,
    socket: ws::WebSocket,
) -> anyhow::Result<()> {
    use futures_util::{SinkExt, StreamExt};

    let (mut ws_tx, mut ws_rx) = socket.split();
    // Attach failures (unknown session, tmux trouble) must reach the
    // user, not just the helm's log: without this the browser sees a
    // bare socket close and shows a generic "connection closed".
    let (channel, mut events) = match state.client.attach(&session_id, q.cols, q.rows).await {
        Ok(parts) => parts,
        Err(e) => {
            let notice = serde_json::json!({"type": "detached", "reason": format!("{e:#}")});
            let _ = ws_tx
                .send(ws::Message::Text(notice.to_string().into()))
                .await;
            return Err(e);
        }
    };

    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(TermEvent::Data(bytes)) => {
                    if ws_tx.send(ws::Message::Binary(bytes.into())).await.is_err() {
                        break;
                    }
                }
                Some(TermEvent::Detached(reason)) => {
                    let notice = serde_json::json!({"type": "detached", "reason": reason});
                    let _ = ws_tx.send(ws::Message::Text(notice.to_string().into())).await;
                    break;
                }
                None => break,
            },
            msg = ws_rx.next() => match msg {
                Some(Ok(ws::Message::Binary(bytes))) => {
                    state.client.send_input(channel, bytes.to_vec());
                }
                Some(Ok(ws::Message::Text(text))) => {
                    if let Ok(WsClientMsg::Resize { cols, rows }) =
                        serde_json::from_str::<WsClientMsg>(&text)
                    {
                        state.client.resize(&session_id, channel, cols, rows);
                    }
                }
                Some(Ok(ws::Message::Close(_))) | None => break,
                Some(Ok(_)) => {} // ping/pong handled by axum
                // Surfaced, not swallowed: an oversized message or a
                // protocol error here is otherwise invisible to both the
                // user (generic "connection closed") and the log.
                Some(Err(e)) => {
                    state.client.detach(channel).await;
                    return Err(anyhow::Error::new(e).context("terminal websocket receive failed"));
                }
            },
        }
    }
    state.client.detach(channel).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{HelmArgs, SupervisorError, build_router, origin_is_allowed};
    use axum::http::HeaderMap;

    const PORT: u16 = 7433;

    /// A `#[derive(Parser)]` shim purely so this test can call
    /// `try_parse_from` — `HelmArgs` itself is `#[derive(Args)]`, meant to
    /// be `#[command(flatten)]`ed into the real `farhelm` CLI, and only a
    /// top-level `Parser` exposes clap's parsing entry point.
    #[derive(clap::Parser, Debug)]
    struct Wrapper {
        #[command(flatten)]
        args: HelmArgs,
    }

    /// Pins `--remote-state-dir`'s CLI-level UTF-8 rejection: the field is
    /// `Option<String>`, not `Option<PathBuf>`, specifically so clap itself
    /// refuses a non-UTF-8 OS argument before it can reach the ssh argv
    /// builder. Reverting the field to `PathBuf` (with a lossy conversion
    /// added downstream to compensate) would leave every other test in
    /// this crate green — clap's own argument-parsing behavior is the only
    /// thing that would catch that regression, so it must be pinned here
    /// directly rather than inferred from ssh-argv-level tests.
    #[test]
    fn remote_state_dir_rejects_non_utf8_os_argument() {
        use clap::Parser;
        use std::os::unix::ffi::OsStringExt;

        let non_utf8 = std::ffi::OsString::from_vec(vec![0xff, 0xfe]);
        let mut argv: Vec<std::ffi::OsString> = vec!["farhelm".into(), "--remote-state-dir".into()];
        argv.push(non_utf8);
        let err = Wrapper::try_parse_from(argv).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidUtf8);

        // A valid unicode value must keep working — this is a rejection
        // test for non-UTF-8 specifically, not a ban on the flag.
        let parsed = Wrapper::try_parse_from([
            "farhelm".into(),
            "--remote-state-dir".into(),
            std::ffi::OsString::from("/remote/state"),
        ])
        .unwrap();
        assert_eq!(
            parsed.args.remote_state_dir.as_deref(),
            Some("/remote/state")
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
        let dashdash = args.iter().position(|a| a == "--").expect("-- separator");
        let remote = args[dashdash + 1..].join(" ");
        let parsed = shell_words::split(&remote).expect("remote command must be shell-parseable");
        assert_eq!(parsed, vec!["/opt/far helm's/bin", "internal", "stdio"]);
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
        let dashdash = args.iter().position(|a| a == "--").expect("-- separator");
        let remote = args[dashdash + 1..].join(" ");
        let parsed = shell_words::split(&remote).expect("remote command must be shell-parseable");
        assert_eq!(
            parsed,
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
            writer
                .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                    req_id,
                    session: SessionInfo {
                        id: "sess-1".into(),
                        title: "some-agent".into(),
                        cwd: "/some/dir".into(),
                        invocation: "some-agent".into(),
                    },
                }))
                .await
                .unwrap();
        });

        let (r, w) = tokio::io::split(client_side);
        let client = super::SupervisorClient::start(r, w).await.unwrap();
        let app = build_router(client, None, 7433);

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

        let (r, w) = tokio::io::split(client_side);
        let client = super::SupervisorClient::start(r, w).await.unwrap();
        let app = build_router(client, None, 7433);

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
