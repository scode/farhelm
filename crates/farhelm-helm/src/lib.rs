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
use farhelm_proto::io::ClosedBeforeHello;
use serde::Deserialize;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tracing::{error, info};

mod client;
pub use client::{
    CreateExtras, SessionListing, SupervisorClient, SupervisorError, TermDetachSignal, TermEvent,
    TermStream,
};

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
        .route("/api/sessions/{id}/stop", axum::routing::post(stop_session))
        .route(
            "/api/sessions/{id}/restart",
            axum::routing::post(restart_session),
        )
        .route(
            "/api/sessions/{id}",
            get(get_session).delete(delete_session),
        )
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
            SupervisorClient::start(stdout, stdin)
                .await
                .map_err(|e| annotate_ssh_handshake_eof(e, dest, args.remote_state_dir.as_deref()))
        }
    }
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
/// reaches the operator only as inherited ssh stderr, disconnected from
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
/// `remote_state_dir` mirrors `--remote-state-dir` so the suggested
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
         failed — ssh reports its own errors on stderr above"
    ))
}

/// `GET /api/sessions` — the full `SessionListing` as JSON:
/// `{"sessions": [...], "total": N, "truncated": bool}` (PLAN_M2.md step
/// 6). This is a breaking shape change from M1's bare array, so this PR
/// also updates the UI's `fetch_sessions` (farhelm-ui/src/lib.rs) in the
/// same change — the object shape itself, and the `sessions` key
/// specifically, are load-bearing for that caller today. `total` and
/// `truncated` are threaded through the wire here but not yet consumed
/// by the production UI (the tests here and in Playwright do read them):
/// they are the contract the next PR's list UI ("showing N of M") builds
/// against. The helm caches
/// nothing before M6; supervisors are the authority (SPEC.md). The
/// last-known-session cache that survives helm restarts arrives with
/// M6's registry and stale-cache semantics (PLAN.md) — with one
/// always-connected supervisor there is nothing for a cache to add.
async fn list_sessions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.client.list_sessions().await {
        Ok(listing) => axum::Json(listing).into_response(),
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

/// `POST /api/sessions` — the creation API SPEC_impl.md calls the one true
/// path. The CLI's `--cwd/--agent` flags and any future UI dialog both
/// land on the same supervisor call this reaches.
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
/// Everything else about this handler is unchanged, including for bodies
/// that omit all three fields entirely.
async fn create_session(
    State(state): State<Arc<AppState>>,
    axum::Json(req): axum::Json<CreateReq>,
) -> impl IntoResponse {
    match state
        .client
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
        Ok(session) => axum::Json(session).into_response(),
        Err(e) => http_error(e),
    }
}

/// `POST /api/sessions/{id}/stop` — kill the agent's process tree, leaving
/// the session listed and its terminal viewable (SPEC.md's "stop", the
/// recoverable operation the UI does not confirm). The body carries no
/// information beyond success — an empty JSON object, so the response
/// shape stays uniform with `delete_session` below and callers do not
/// need to special-case "no content" bodies — and an unknown `id` reaches
/// the browser as a 404 through `http_error`'s `SupervisorError` downcast.
async fn stop_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
    match state.client.stop_session(&id).await {
        Ok(()) => axum::Json(serde_json::json!({})).into_response(),
        Err(e) => http_error(e),
    }
}

/// `GET /api/sessions/{id}` — one session's current `SessionInfo`.
///
/// Exists for the recovery paths rather than for browsing: after a restart
/// (or after a restart whose reply was lost) a client needs THIS session's
/// current status and offer, and asking through the full listing makes
/// that lookup depend on a reply the supervisor caps at
/// `LIST_SESSION_CAP` sessions — so on a busy host the one session a
/// client is acting on can simply be absent from the answer.
///
/// Honest limitation, stated because it is not fixed here: the supervisor's
/// protocol has no per-session query, so this handler still filters a
/// listing and therefore still inherits that cap. What it buys today is
/// ONE place for every client's recovery lookup to live, so the fix — a
/// `GetSession` message — lands behind this route rather than in each
/// caller. An id the listing does not contain is a 404, which is also the
/// honest answer for a session that was genuinely deleted.
async fn get_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
    match state.client.list_sessions().await {
        Ok(listing) => match listing.sessions.into_iter().find(|s| s.id == id) {
            Some(session) => axum::Json(session).into_response(),
            None => (
                axum::http::StatusCode::NOT_FOUND,
                format!("no such session: {id}\n"),
            )
                .into_response(),
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
/// again.
async fn restart_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    axum::Json(req): axum::Json<RestartReq>,
) -> impl IntoResponse {
    match state
        .client
        .restart_session(&id, req.mode, req.stop_if_running)
        .await
    {
        Ok(session) => axum::Json(session).into_response(),
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
async fn delete_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
    match state.client.delete_session(&id).await {
        Ok(()) => axum::Json(serde_json::json!({})).into_response(),
        Err(e) => http_error(e),
    }
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
        // PLAN_M3.md item 6: an intent key reused with a different
        // fingerprint. 409 is the standard HTTP reading of "this
        // identifier already means something else"; this function's own
        // docstring above is where the full status-mapping table lives.
        Some(ErrorKind::Conflict) => axum::http::StatusCode::CONFLICT,
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
/// resize/pause/resume; server → detached notice). This is the
/// browser-facing twin of the proto data channel, kept equally dumb.
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
/// Server → client stays one message: `{"type":"detached","reason":...}`.
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
                    state.client.send_input(channel, bytes.to_vec()).await;
                }
                Ok(ws::Message::Text(text)) => match serde_json::from_str::<WsClientMsg>(&text) {
                    Ok(WsClientMsg::Resize { cols, rows }) => {
                        state.client.resize(&session_id, channel, cols, rows).await;
                    }
                    Ok(WsClientMsg::Pause) => state.client.pause_output(channel).await,
                    Ok(WsClientMsg::Resume) => state.client.resume_output(channel).await,
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
    state.client.detach(channel).await;
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
    use super::{HelmArgs, SupervisorError, build_router, origin_is_allowed};
    use axum::http::HeaderMap;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake};
    use farhelm_proto::{ControlMsg, Frame};
    use std::sync::Arc;
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

    /// Serve the helm's real router on a loopback port, returning its
    /// address.
    ///
    /// The WebSocket tests cannot use `oneshot` like the HTTP ones do: an
    /// upgrade needs a real connection with a real byte stream on both
    /// sides, which is also exactly what makes a "browser that stops
    /// reading" expressible at all.
    async fn serve_helm(client: Arc<super::SupervisorClient>) -> std::net::SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");
        let app = build_router(client, None, addr.port());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        addr
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
        let (r, w) = tokio::io::split(client_side);
        let client = super::SupervisorClient::start(r, w).await.unwrap();
        let addr = serve_helm(client).await;
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
        let (r, w) = tokio::io::split(client_side);
        let client = super::SupervisorClient::start(r, w).await.unwrap();
        let addr = serve_helm(client).await;
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

        let (r, w) = tokio::io::split(client_side);
        let client = super::SupervisorClient::start(r, w).await.unwrap();
        let app = build_router(client, None, 7433);
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

        let (r, w) = tokio::io::split(client_side);
        let client = super::SupervisorClient::start(r, w).await.unwrap();
        let app = build_router(client, None, 7433);

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
    /// supervisor's own message verbatim — the same `SupervisorError`
    /// downcast `http_error` uses everywhere else, exercised here through
    /// the stop route specifically rather than assumed from the create/
    /// attach coverage above.
    ///
    /// The scripted message is a sentinel unlikely to appear by accident
    /// (not the generic "no such session" prose a supervisor might
    /// plausibly emit for unrelated reasons), and the assertion checks the
    /// COMPLETE body against it — a substring check would still pass if
    /// `http_error` silently truncated, reworded, or wrapped the message in
    /// extra context, none of which "verbatim" allows.
    #[tokio::test]
    async fn stop_session_unknown_id_returns_404_with_supervisor_message() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-stop-9f3ac2: no such session";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::StopSession { req_id, .. } = request else {
                panic!("expected StopSession, got {request:?}");
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

        let (r, w) = tokio::io::split(client_side);
        let client = super::SupervisorClient::start(r, w).await.unwrap();
        let app = build_router(client, None, 7433);

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
            SENTINEL,
            "body must carry the supervisor's own message verbatim, not a substring of it"
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

        let (r, w) = tokio::io::split(client_side);
        let client = super::SupervisorClient::start(r, w).await.unwrap();
        let app = build_router(client, None, 7433);

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

    /// Deleting an unknown id must 404 with the supervisor's message
    /// verbatim, the delete-side twin of
    /// `stop_session_unknown_id_returns_404_with_supervisor_message` — see
    /// that test's docs for why the assertion checks the complete body
    /// against a sentinel rather than a generic substring.
    #[tokio::test]
    async fn delete_session_unknown_id_returns_404_with_supervisor_message() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-delete-7b1e04: no such session";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::DeleteSession { req_id, .. } = request else {
                panic!("expected DeleteSession, got {request:?}");
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

        let (r, w) = tokio::io::split(client_side);
        let client = super::SupervisorClient::start(r, w).await.unwrap();
        let app = build_router(client, None, 7433);

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
            SENTINEL,
            "body must carry the supervisor's own message verbatim, not a substring of it"
        );

        peer.await.unwrap();
    }

    /// `GET /api/sessions` now serializes the WHOLE `SessionListing`, not
    /// just the bare `sessions` array (PLAN_M2.md step 6) — this pins the
    /// object shape end to end, with sentinel `total`/`truncated` values
    /// deliberately far from `sessions.len()` so a regression that drops
    /// either field, or recomputes `total` from the list length instead of
    /// forwarding the supervisor's own count, shows up immediately.
    #[tokio::test]
    async fn list_sessions_returns_full_listing_object_shape() {
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
            let ControlMsg::ListSessions { req_id } = request else {
                panic!("expected ListSessions, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::SessionList {
                    req_id,
                    sessions: vec![farhelm_proto::SessionInfo {
                        id: "sess-1".into(),
                        title: "sess-1".into(),
                        cwd: "/sess-1".into(),
                        invocation: "agent".into(),
                        status: farhelm_proto::SessionStatus::Alive,
                        annotation: None,
                        restart_offer: farhelm_proto::RestartOffer::default(),
                        tabs: Vec::new(),
                    }],
                    total: 42,
                    truncated: true,
                })
                .await
                .unwrap();
        });

        let (r, w) = tokio::io::split(client_side);
        let client = super::SupervisorClient::start(r, w).await.unwrap();
        let app = build_router(client, None, 7433);

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
        assert_eq!(value["total"], 42);
        assert_eq!(value["truncated"], true);
        assert_eq!(value["sessions"][0]["id"], "sess-1");

        peer.await.unwrap();
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
    #[tokio::test]
    async fn list_sessions_passes_interrupted_status_and_stop_annotation_through() {
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
            let ControlMsg::ListSessions { req_id } = request else {
                panic!("expected ListSessions, got {request:?}");
            };
            let session = |id: &str, status, annotation: Option<&str>| farhelm_proto::SessionInfo {
                id: id.into(),
                title: id.into(),
                cwd: "/tmp".into(),
                invocation: "agent".into(),
                status,
                annotation: annotation.map(str::to_string),
                restart_offer: farhelm_proto::RestartOffer::default(),
                tabs: Vec::new(),
            };
            writer
                .write_control(&ControlMsg::SessionList {
                    req_id,
                    sessions: vec![
                        session("lost", farhelm_proto::SessionStatus::Interrupted, None),
                        session(
                            "stopped",
                            farhelm_proto::SessionStatus::Exited { exit_code: Some(0) },
                            Some(farhelm_proto::STOP_ANNOTATION),
                        ),
                    ],
                    total: 2,
                    truncated: false,
                })
                .await
                .unwrap();
        });

        let (r, w) = tokio::io::split(client_side);
        let client = super::SupervisorClient::start(r, w).await.unwrap();
        let app = build_router(client, None, 7433);

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

        peer.await.unwrap();
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
            assert!(
                reader.read_frame().await.unwrap().is_none(),
                "stop request must never reach the supervisor for a foreign origin"
            );
        });

        let (r, w) = tokio::io::split(client_side);
        let client = super::SupervisorClient::start(r, w).await.unwrap();
        let app = build_router(client, None, 7433);

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

        let (r, w) = tokio::io::split(client_side);
        let client = super::SupervisorClient::start(r, w).await.unwrap();
        let app = build_router(client, None, 7433);
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

        let (r, w) = tokio::io::split(client_side);
        let client = super::SupervisorClient::start(r, w).await.unwrap();
        let app = build_router(client, None, 7433);
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
}
