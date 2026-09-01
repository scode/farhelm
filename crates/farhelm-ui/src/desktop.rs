//! Desktop ownership of the embedded helm, local supervisor, and two client
//! credentials.
//!
//! The native app is not a remote-helm client. It owns one loopback helm for
//! its lifetime, discovers or starts the local supervisor against the same
//! state directory, and authenticates both client stacks through the helm's normal
//! token exchange. Native reqwest and the webview deliberately retain
//! different device credentials: they are separate clients with separate
//! persistence and WebSocket behavior, and sharing one would leave half of
//! the bootstrap contract unexercised.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc, LazyLock, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

const APP_STATE_FILE: &str = "desktop-client.json";
const DEFAULT_DESKTOP_PORT: u16 = 7433;
const DESKTOP_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

/// First path segment the desktop asset handler claims.
///
/// dioxus-desktop dispatches on exactly this: `desktop_handler` in
/// dioxus-desktop 0.7.10's `protocol.rs` takes `uri.path().split('/').nth(1)`
/// and, if a handler is registered under that name, calls it INSTEAD of
/// `dioxus_asset_resolver::native::serve_asset`. So the name is not a label —
/// it is the route, and it has to equal the first segment manganis puts in
/// front of every bundled asset path (`/assets/<file>`; see
/// `manganis_core::Asset::resolve`).
const ASSET_ROUTE: &str = "assets";

/// The hidden argv-1 flag that prints resolved asset paths instead of
/// launching (see [`print_assets`]).
const PRINT_ASSETS_FLAG: &str = "--print-assets";

/// The desktop app's entire entry point, shared by both binaries that have
/// one.
///
/// `crates/farhelm-desktop` (the bare binary a release ships, D6) and this
/// crate's own `farhelm-ui` bin (what `dx build --platform desktop` and
/// `scripts/desktop-smoke.sh` run) both call exactly this. That is the point:
/// the shell exercised under Xvfb on every change is the same shell that
/// ships, not a near-copy of it that can drift.
///
/// Blocks for the app's whole lifetime. The `Ok(())` is reached only after
/// the event loop returns, i.e. once the last window closed; a bootstrap
/// failure panics rather than returning, because there is no window yet in
/// which to show an error and a silently-exiting GUI process is worse than a
/// crash report.
pub fn run() -> anyhow::Result<()> {
    // Ahead of the tracing subscriber and the bootstrap both: this mode
    // must start nothing, bind no port, and touch no state directory. It is
    // a build-time question asked of a built binary (see `print_assets`).
    if std::env::args().nth(1).as_deref() == Some(PRINT_ASSETS_FLAG) {
        print_assets();
        return Ok(());
    }

    // Before anything else: bootstrap itself can log, and the embedded helm
    // it starts begins emitting the moment it does.
    init_tracing();

    // A panic here prints a Rust backtrace hint ahead of what is, for every
    // failure this can actually produce, an operator-facing startup refusal
    // rather than a programming bug — someone staring at a GUI app that
    // just quit gets "thread 'main' panicked ... run with RUST_BACKTRACE=1"
    // when what they need is one plain sentence. ONLY the two specialized
    // tmux refusals (missing, below floor) print their own exact message
    // and exit inside `start`'s preflight (see `run_tmux_preflight_or_exit`)
    // without ever returning an `Err`; every other tmux-probe failure this
    // preflight cannot make sense of (permission denied, a nonzero `-V`, an
    // unparseable version) returns `Err` same as any other bootstrap
    // failure — state directory unreadable, port already bound, the
    // sibling `farhelm` missing — and lands right here. None of those
    // deserve a crash report either, so they get the same plain treatment.
    let desktop = match DesktopBootstrap::start() {
        Ok(desktop) => desktop,
        Err(error) => {
            eprintln!("farhelm-desktop: {error:#}");
            std::process::exit(1);
        }
    };

    let builder = dioxus::LaunchBuilder::new()
        .with_context(crate::ApiBase(desktop.api_base().to_string()))
        .with_context(desktop.webview_bootstrap());

    // Desktop windows need an explicit WindowBuilder, and not only for the
    // title: dioxus-desktop's `Config::new()` marks debug-build windows
    // always-on-top whenever the app is NOT launched through `dx`
    // (`dioxus_cli_config::always_on_top().unwrap_or(true)` — a
    // convenience for `dx serve` development that misfires for a real app
    // started via `cargo run`, leaving the window permanently above
    // everything). `Config::with_window` replaces the default builder
    // wholesale, which discards that always-on-top default along with the
    // "Dioxus App" placeholder title.
    // `with_disable_drag_drop_handler(true)` is the attachments feature's
    // half of this (PLAN_M4.md item 7, SPEC_impl.md's "one concrete thing
    // to check early rather than debug late: wry's own file-drop handling
    // swallows DOM drop events unless configured not to"). Dropping a file
    // into a terminal is intercepted in the PAGE — assets/terminal.js —
    // so the DOM `drop` event has to reach it, and anything that consumes
    // the drag first breaks the headline feature on the desktop build
    // alone, where nothing in CI would notice.
    //
    // The audit trail behind this call, against dioxus-desktop 0.7.9 and
    // wry 0.53.5, since "configured not to" means different things per
    // platform:
    //
    // - Without this, dioxus installs its own `wry` drag-drop handler
    //   (`webview.rs`, gated on `cfg.disable_file_drop_handler`) to feed
    //   its native file-drop support. That handler returns `false`, which
    //   wry reads as "not handled" and answers by invoking the OS default
    //   — so on macOS (`wkwebview/drag_drop.rs` calling `super`) and on
    //   GTK the DOM events do still fire. On Windows they do not: dioxus's
    //   own comment says the WebView2 host blocks HTML-native drag events
    //   whenever a drop handler is present, and its config doc says the
    //   handler must be disabled for the HTML drag and drop APIs to work.
    // - So the setting is not load-bearing on the two platforms Farhelm
    //   targets today, and it is set anyway: it is the difference between
    //   "the DOM path works because a handler we do not want happens to
    //   decline every event" and "nothing is competing for the drag". The
    //   cost is dioxus's native file-drop support, which this UI does not
    //   use — no `ondrop` handler exists anywhere in the component tree,
    //   and the attachment path deliberately reads `File` objects in JS
    //   rather than paths in Rust (see src/attachments.rs).
    //
    // Verifying the CAPABILITY rather than the configuration is the
    // manual desktop pass PLAN_M4.md acceptance 9 records; this call is
    // what that pass is checking the effect of. The checklist that pass
    // has to work through — including the one risk it is most likely to
    // trip over — is written out in `attachments`' module header.
    builder
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_window(dioxus::desktop::WindowBuilder::new().with_title("farhelm"))
                .with_disable_drag_drop_handler(true),
        )
        .launch(crate::App);
    Ok(())
}

/// Print every `asset!()` path this build will ask the webview for, one per
/// line, and start nothing.
///
/// `scripts/check-desktop-assets.sh` is the only caller. It compares this set
/// against the files `dx build --platform web` actually emitted, in both
/// directions, and fails CI on any difference — the Linux stand-in for the
/// plan's macOS asset-name gate. Hidden rather than a real CLI surface
/// (no `clap`, no `--help` entry) because it exists for that script, and D6's
/// binary otherwise takes no arguments at all.
///
/// What comes out depends on how the binary was built and how it is run,
/// which is the whole reason the script is fussy about both:
/// `manganis_core::Asset::resolve` returns the ABSOLUTE SOURCE path when
/// `dioxus_core_types::is_bundled_app()` is false — and that is a runtime
/// check of `CARGO_MANIFEST_DIR`, so `cargo run -- --print-assets` prints
/// source paths while the same binary invoked directly prints `/assets/...`.
/// The hashed names themselves are written into the binary by `dx` AFTER the
/// link, by rewriting the `__ASSETS__` symbols rustc emitted; a plain
/// `cargo build` leaves `BundledAsset::PLACEHOLDER_HASH` in their place. See
/// that script's header for what it does about both.
fn print_assets() {
    for asset in crate::all_assets() {
        println!("{asset}");
    }
}

/// Install this process's `tracing` subscriber.
///
/// Load-bearing for `docs/desktop-web-triage.md`'s whole premise: without a
/// subscriber, every `tracing::error!`/`warn!` this binary emits — including
/// the embedded helm's forwarded webview console events (`farhelm-helm`'s
/// `client_log.rs`), the eval-bridge watchdog's own health line
/// (`webview_watchdog.rs`), and the asset handler's per-request debug lines
/// below — reaches the default no-op dispatcher and simply vanishes.
/// `crates/farhelm`'s CLI installs its own subscriber (`init_tracing` there)
/// for `farhelm helm run` and `farhelm supervisor run`, but those are that
/// OTHER binary's subcommands, running in a spawned subprocess; this desktop
/// app embeds a helm directly in ITS OWN process ([`DesktopBootstrap::start`])
/// and never goes through that code path, so it needs the same setup
/// independently. Mirrors that function's filter default (`RUST_LOG`, else
/// `info`) and its choice of stderr, which the smoke and the maintainer's dev
/// loop both redirect into `desktop.log` alongside stdout either way.
fn init_tracing() {
    // Build-sensitive default, matching what dioxus's own launcher would
    // have installed had this subscriber not claimed the global slot first:
    // debug builds (the laptop dev flow's default) keep debug-level events
    // in desktop.log, release stays at info. `RUST_LOG` overrides both.
    let default_filter = if cfg!(debug_assertions) {
        "debug"
    } else {
        "info"
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter)),
        )
        .with_writer(std::io::stderr)
        .init();
}

/// Claim `/assets/*` in the desktop window and serve it from the UI tree
/// this build embedded (D6).
///
/// ## Why a handler at all
///
/// A bare binary has no bundle around it. Left to itself, dioxus resolves
/// `asset!()` paths off the FILESYSTEM relative to the executable
/// (`dioxus_asset_resolver::native::get_asset_root`: `<exe>/../Resources` on
/// macOS, `<exe>/../lib/<product>` on Linux), so `farhelm-desktop` sitting in
/// `~/.local/bin` would look for `~/.local/Resources/assets/...` and find
/// nothing — a window that renders chrome and loads no terminal, no fonts, no
/// scripts.
/// The embedded tree (`farhelm_helm::embedded_ui()`, D12) is already in the
/// binary for the helm's own sake; this hands the same bytes to the webview.
///
/// ## Why registration wins
///
/// dioxus-desktop 0.7.10 consults the handler registry BEFORE the filesystem
/// resolver — `protocol.rs`'s `desktop_handler` matches the first path
/// segment against registered names and returns early on a hit. Registering
/// [`ASSET_ROUTE`] therefore takes `/assets/*` away from the resolver
/// entirely, which is what makes the embedded tree authoritative rather than
/// merely a fallback. Note what that costs: with a handler registered there
/// is no filesystem fallback for `/assets/*` at all, so a file missing from
/// the embedded tree is a hard 404 even when a copy happens to sit next to
/// the binary.
///
/// (0.7.10 has no `Config::with_asset_handler`; `use_asset_handler` is the
/// only public way in, which is why this is a hook and must run from a
/// component rather than from [`run`].)
pub(crate) fn use_embedded_asset_handler() {
    dioxus::desktop::use_asset_handler(ASSET_ROUTE, |request, responder| {
        responder.respond(serve_asset(request.method(), request.uri().path()));
    });
}

/// What [`serve_asset_from`] is allowed to know about the embedded tree: a
/// path in, the bytes out, nothing else.
///
/// The narrow shape is the point. `farhelm_helm::embedded_ui()` answers from
/// a `static` that only a release-shaped build populates, so a test running
/// against the real thing would be testing whichever build it happened to be
/// compiled into. One function pointer's worth of indirection lets the
/// response rules be tested against a fixture instead, with no environment
/// mutation and no `include_dir!` fixture tree to maintain.
type AssetLookup<'a> = dyn Fn(&str) -> Option<&'a [u8]> + 'a;

/// Answer one `dioxus://` asset request out of the embedded UI tree.
///
/// Thin on purpose: it resolves whichever lookup this build has — including
/// none — and hands the rest to [`serve_asset_from`], which is where the
/// rules live and where the tests point.
fn serve_asset(
    method: &dioxus::desktop::wry::http::Method,
    path: &str,
) -> dioxus::desktop::wry::http::Response<Vec<u8>> {
    match farhelm_helm::embedded_ui() {
        Some(dir) => {
            let lookup = move |relative: &str| dir.get_file(relative).map(|file| file.contents());
            serve_asset_from(Some(&lookup), method, path)
        }
        None => serve_asset_from(None, method, path),
    }
}

/// The response rules for one asset request, over any lookup.
///
/// Deliberately mirrors `farhelm-helm`'s `serve_embedded` for the rules that
/// matter — the `GET`/`HEAD` method gate, percent-decoding before lookup, and
/// `mime_guess` content types — so an asset behaves identically whether a
/// browser fetched it from the helm or the native window fetched it from
/// here. It does NOT mirror the SPA `index.html` fallback: this route only
/// ever serves concrete files, and answering a miss with markup would hand
/// the webview HTML where it asked for JavaScript. Nor does anything strip a
/// `HEAD` response's body the way axum's router does for the helm — wry hands
/// the response straight back — which costs nothing here, since the webview
/// only ever issues `GET` for an asset.
///
/// Every outcome is logged at DEBUG with a stable prefix, because that log is
/// the only evidence anyone has that the handler ran at all:
/// `scripts/desktop-smoke.sh` asserts at least one `served` line and zero
/// `missing` lines, which is what turns "the window looked fine" into a real
/// gate. Do not reword these lines without updating that script.
///
/// `lookup` is `None` for a build that embedded no UI at all. That is not an
/// error: D12 makes it a supported developer arrangement (`cargo build -p
/// farhelm-desktop` with no `FARHELM_UI_DIST`), and it answers every request
/// with the same empty 404 a miss gets — a window with no UI, whose one
/// explanation is the log line below. Taking it as a parameter rather than
/// reading `embedded_ui()` here is what lets that branch be tested at all:
/// whether a tree is embedded is fixed when the test binary is compiled.
fn serve_asset_from(
    lookup: Option<&AssetLookup<'static>>,
    method: &dioxus::desktop::wry::http::Method,
    path: &str,
) -> dioxus::desktop::wry::http::Response<Vec<u8>> {
    use dioxus::desktop::wry::http::{Method, Response, StatusCode, header};

    if *method != Method::GET && *method != Method::HEAD {
        tracing::debug!("desktop asset handler: refused {method} {path} (405)");
        return Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .header(header::ALLOW, "GET,HEAD")
            .body(Vec::new())
            .expect("a status-and-header response is always well-formed");
    }
    let Some(lookup) = lookup else {
        tracing::debug!("desktop asset handler: no UI is embedded in this build, {path} (404)");
        return not_found();
    };
    // `include_dir!` keys every entry by its path relative to the embedded
    // root with no leading slash; every path wry hands a handler has one.
    let relative = path.trim_start_matches('/');
    // Decode once, before the lookup: an asset whose real name needs
    // escaping in a URL must resolve to the file `include_dir!` compiled in
    // under its actual name. Invalid UTF-8 cannot match any entry (they are
    // all Rust string literals), so it is a miss rather than a panic.
    let Ok(relative) = percent_encoding::percent_decode_str(relative).decode_utf8() else {
        tracing::debug!("desktop asset handler: missing {path} (404)");
        return not_found();
    };
    match lookup(relative.as_ref()) {
        Some(bytes) => {
            tracing::debug!("desktop asset handler: served {path} (200)");
            Response::builder()
                .header(
                    header::CONTENT_TYPE,
                    mime_guess::from_path(relative.as_ref())
                        .first_or_octet_stream()
                        .essence_str(),
                )
                // Matches what dioxus's own resolver stamps on every asset
                // it serves. The page and its assets share the
                // `dioxus://index.html` origin, so nothing here NEEDS it
                // today; diverging from the responses the rest of the
                // ecosystem produces is the larger risk.
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(bytes.to_vec())
                .unwrap_or_else(|_| not_found())
        }
        None => {
            tracing::debug!("desktop asset handler: missing {path} (404)");
            not_found()
        }
    }
}

/// The one 404 shape [`serve_asset`] returns, built here so every miss looks
/// the same to the webview regardless of which check rejected it.
fn not_found() -> dioxus::desktop::wry::http::Response<Vec<u8>> {
    dioxus::desktop::wry::http::Response::builder()
        .status(dioxus::desktop::wry::http::StatusCode::NOT_FOUND)
        .body(Vec::new())
        .expect("a status-only response is always well-formed")
}

/// Values the webview needs to validate or mint its own device session.
///
/// The bootstrap token crosses the native/webview boundary through Dioxus
/// IPC after the document exists. It is never placed in a URL, page markup,
/// or command line.
#[derive(Clone, PartialEq)]
pub struct WebviewBootstrap {
    pub(crate) base: String,
    /// Durable browser credential candidate; `None` means the page must mint.
    pub(crate) persisted_secret: Option<String>,
    /// A smoke-test-only hook: `None` in every real run.
    ///
    /// `scripts/desktop-smoke.sh` sets `FARHELM_SMOKE_CLIENT_LOG_MARKER` so
    /// the console shim can `console.error` it once armed, proving the
    /// shim -> `/api/client-log` -> `tracing` pipeline end to end. Only a
    /// marker that flows through the REAL capture path is honest proof; a
    /// shortcut that wrote the marker straight into the log would validate
    /// nothing about the pipeline it exists to catch regressions in.
    pub(crate) smoke_client_log_marker: Option<String>,
}

struct RuntimeAuth {
    base: String,
    state_dir: PathBuf,
    state_path: PathBuf,
}

static RUNTIME_AUTH: std::sync::OnceLock<RuntimeAuth> = std::sync::OnceLock::new();

/// Serializes recovery after rotation so a burst of stale REST requests mints
/// one replacement native session, not one session per in-flight reader.
static DEVICE_REFRESH: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

/// The app-owned processes and authentication state kept alive by `main`.
pub struct DesktopBootstrap {
    webview: WebviewBootstrap,
    /// Present only when this process proved absence and started the local
    /// supervisor. A discovered supervisor belongs to its existing owner and
    /// must neither be tethered to nor stopped with the desktop app.
    supervisor: Option<Child>,
    /// Owned as an option because `Drop` must move the handle out to join it.
    /// The monitor itself owns the helm handle and reports any unexpected
    /// completion before this orderly-shutdown join returns.
    helm_monitor: Option<std::thread::JoinHandle<()>>,
    /// The embedded server's graceful lifetime edge. Dropping this sender is
    /// also shutdown, which covers partial construction failures.
    helm_shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    /// Separates expected teardown from a fatal post-readiness completion in
    /// the independent monitor thread.
    expected_helm_shutdown: Arc<AtomicBool>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct PersistedState {
    native_device_secret: Option<String>,
    webview_device_secret: Option<String>,
    /// Monotonic proof that the real webview JavaScript stack completed an
    /// authenticated WebSocket handshake. It survives restart so the smoke
    /// gate can distinguish new readiness from old persisted credentials.
    #[serde(default)]
    webview_auth_generation: u64,
}

static STATE_FILE_WRITE: Mutex<()> = Mutex::new(());

#[derive(Debug, Deserialize)]
struct DeviceExchange {
    device_secret: String,
}

impl DesktopBootstrap {
    /// Discover or start the local supervisor and own one embedded helm.
    ///
    /// An answering supervisor is reused unchanged. Only confirmed absence
    /// permits a bundled child, which this process then tethers and monitors
    /// before authenticating native REST or exposing the component tree.
    pub fn start() -> anyhow::Result<Self> {
        // Resolved here, unconditionally, because BOTH branches below need
        // to know it: the `Absent` branch to run the preflight and choose
        // what `FARHELM_TMUX` to hand its child, and neither branch needs
        // to spawn or probe anything to compute it — `resolve_supervisor_tmux`
        // only stats candidate paths (`is_executable_file`), which is silent
        // on stderr regardless of the tracing filter `init_tracing` already
        // installed in `run`.
        let ambient_tmux = std::env::var_os("FARHELM_TMUX");
        let tmux_prefixes = macos_tmux_prefixes();
        let supervisor_tmux =
            resolve_supervisor_tmux(ambient_tmux.clone(), tmux_prefixes, is_executable_file);

        let state_dir = desktop_state_dir()?;
        let runtime =
            tokio::runtime::Runtime::new().context("starting desktop bootstrap runtime")?;
        runtime.block_on(farhelm_supervisor::ensure_private_dir(&state_dir))?;

        let farhelm = bundled_farhelm()?;
        // Discovery MUST run before the tmux preflight, not after: an
        // answering supervisor is an ownership boundary (this type's own
        // doc comment) that has to be reused exactly as it stands, tmux
        // included — it may be driving a perfectly good tmux selected by
        // its own `--tmux`, its own `FARHELM_TMUX`, or a login-shell `PATH`
        // this Finder-launched process never sees. Running the preflight
        // first would refuse startup over a dependency this process is not
        // about to need, for a supervisor it does not own and must not
        // reconfigure. Only the `Absent` branch — the one case where THIS
        // process is about to spawn and hand down its own tmux choice —
        // runs the preflight, immediately before that spawn.
        //
        // Nothing between here and that preflight call reaches stderr,
        // which is what keeps "the refusal is the ONLY thing printed" true
        // on the missing/below-floor path even though the preflight itself
        // no longer runs first: `ensure_private_dir` and `bundled_farhelm`
        // do no logging at all, and `discover_local_supervisor`'s probe
        // pipes its own probe child's stderr and drains it internally
        // rather than inheriting this process's — verified by the exact
        // stderr assertion in `scripts/desktop-smoke.sh`'s tmux-preflight
        // legs, which is the oracle for this ordering claim.
        let mut supervisor = match runtime.block_on(farhelm_helm::discover_local_supervisor(
            &farhelm, &state_dir,
        ))? {
            farhelm_helm::LocalSupervisorDiscovery::Answering => None,
            farhelm_helm::LocalSupervisorDiscovery::Absent => Some({
                run_tmux_preflight_or_exit(
                    supervisor_tmux.as_deref(),
                    ambient_tmux.as_deref(),
                    tmux_prefixes,
                )?;
                let mut command = Command::new(&farhelm);
                command
                    .args(["supervisor", "run", "--exit-on-stdin-close", "--state-dir"])
                    .arg(&state_dir)
                    // The supervisor inherits this process's PATH unchanged.
                    // The bundle's own directory used to be prepended so a
                    // bundled tmux would win; there is no bundled tmux any
                    // more (SPEC_impl.md, "Terminal substrate: private tmux
                    // server") — the `FARHELM_TMUX` below is what names the
                    // substrate — and
                    // the supervisor is launched by absolute path while the
                    // launch shim prepends its own directory for the spawn
                    // CLI inside sessions, so nothing else needed it.
                    //
                    // Retaining the write end tethers only the child this app
                    // owns, including GUI exits that skip Rust destructors.
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit());
                if let Some(tmux) = &supervisor_tmux {
                    command.env("FARHELM_TMUX", tmux);
                }
                command.spawn().with_context(|| {
                    format!(
                        "starting the managed supervisor child through the sibling farhelm at {}",
                        farhelm.display()
                    )
                })?
            }),
        };
        ensure_managed_supervisor_running(&mut supervisor)?;

        // A stable origin makes the embedded web UI discoverable and keeps
        // its browser credential scoped to one origin across app restarts.
        // Binding is deliberately exclusive: if another process owns the
        // chosen port, helm startup fails visibly instead of silently moving
        // the user to a different URL.
        let port = std::env::var("FARHELM_DESKTOP_PORT")
            .ok()
            .map(|value| value.parse::<u16>().context("parsing FARHELM_DESKTOP_PORT"))
            .transpose()?
            .unwrap_or(DEFAULT_DESKTOP_PORT);
        let ui_dist = bundled_web_ui();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let helm_state = state_dir.clone();
        let helm = std::thread::Builder::new()
            .name("farhelm-embedded-helm".to_string())
            .spawn(move || {
                (|| {
                    let runtime =
                        tokio::runtime::Runtime::new().context("starting embedded helm runtime")?;
                    runtime.block_on(farhelm_helm::run_embedded(
                        farhelm_helm::HelmArgs {
                            port,
                            state_dir: Some(helm_state),
                            ui_dist,
                            ensure_hosts: None,
                            payload_dir: None,
                            release_base_url: None,
                        },
                        ready_tx,
                        shutdown_rx,
                    ))
                })()
            })
            .context("starting embedded helm thread")?;
        let helm_deadline = std::time::Instant::now() + DESKTOP_STARTUP_TIMEOUT;
        let addr = loop {
            ensure_managed_supervisor_running(&mut supervisor)?;
            let remaining = helm_deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                bail!("embedded helm did not become ready within 30 seconds");
            }
            match ready_rx.recv_timeout(std::cmp::min(remaining, Duration::from_millis(100))) {
                Ok(addr) => break addr,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return match helm.join() {
                        Ok(Err(error)) => {
                            Err(error).context("embedded helm failed before readiness")
                        }
                        Ok(Ok(())) => bail!("embedded helm stopped before readiness"),
                        Err(_) => bail!("embedded helm panicked during startup"),
                    };
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
            }
        };
        ensure_managed_supervisor_running(&mut supervisor)?;
        let expected_helm_shutdown = Arc::new(AtomicBool::new(false));
        let monitor_expected = Arc::clone(&expected_helm_shutdown);
        let helm_monitor = std::thread::Builder::new()
            .name("farhelm-embedded-helm-monitor".to_string())
            .spawn(move || {
                let completion = helm.join();
                if !monitor_expected.load(Ordering::Acquire) {
                    match completion {
                        Ok(Err(error)) => eprintln!("embedded helm stopped: {error:#}"),
                        Ok(Ok(())) => eprintln!("embedded helm stopped unexpectedly"),
                        Err(_) => eprintln!("embedded helm panicked after readiness"),
                    }
                    std::process::exit(1);
                }
            })
            .context("starting embedded helm monitor")?;
        let base = format!("http://{addr}");

        let state_path = state_dir.join(APP_STATE_FILE);
        let mut persisted = read_state(&state_path)?;
        let token = runtime.block_on(farhelm_helm::show_token(Some(state_dir.clone())))?;
        let credential_deadline = tokio::time::Instant::now() + DESKTOP_STARTUP_TIMEOUT;
        let native = runtime.block_on(native_credential(
            &base,
            &token,
            persisted.native_device_secret.as_deref(),
            credential_deadline,
        ))?;
        persisted = persist_then_publish_native(&state_path, native, |secret| {
            crate::auth::install_native_device_secret(secret);
        })?;
        runtime.block_on(await_local_supervisor(
            &base,
            &state_dir,
            &state_path,
            &mut persisted,
            &mut supervisor,
        ))?;

        RUNTIME_AUTH
            .set(RuntimeAuth {
                base: base.clone(),
                state_dir,
                state_path: state_path.clone(),
            })
            .map_err(|_| anyhow::anyhow!("desktop authentication runtime was initialized twice"))?;

        let webview = WebviewBootstrap {
            base: base.clone(),
            persisted_secret: persisted.webview_device_secret,
            smoke_client_log_marker: std::env::var("FARHELM_SMOKE_CLIENT_LOG_MARKER").ok(),
        };
        Ok(Self {
            webview,
            supervisor,
            helm_monitor: Some(helm_monitor),
            helm_shutdown: Some(shutdown_tx),
            expected_helm_shutdown,
        })
    }

    /// Loopback origin of this process's embedded helm.
    pub fn api_base(&self) -> &str {
        &self.webview.base
    }

    /// Clone the bootstrap values passed to the component tree.
    ///
    /// The credential enters JavaScript only through Dioxus IPC and never
    /// appears in the URL or rendered markup.
    pub fn webview_bootstrap(&self) -> WebviewBootstrap {
        self.webview.clone()
    }
}

impl Drop for DesktopBootstrap {
    fn drop(&mut self) {
        self.expected_helm_shutdown.store(true, Ordering::Release);
        if let Some(shutdown) = self.helm_shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(supervisor) = &mut self.supervisor {
            let _ = supervisor.kill();
            let _ = supervisor.wait();
        }
        if let Some(monitor) = self.helm_monitor.take() {
            let _ = monitor.join();
        }
    }
}

/// Persist a webview credential only after its JavaScript stack completed an
/// authenticated event-socket handshake, advancing the durable readiness
/// generation in the same atomic state-file replacement.
pub(crate) fn persist_webview_secret(secret: String) -> anyhow::Result<()> {
    let state_path = &RUNTIME_AUTH
        .get()
        .context("desktop authentication runtime is not initialized")?
        .state_path;
    update_state(state_path, |state| {
        state.webview_device_secret = Some(secret);
        state.webview_auth_generation = state.webview_auth_generation.saturating_add(1);
    })
    .map(|_| ())
}

/// Expose the desktop smoke run's real listing query without a pixel oracle.
///
/// The hook is inert outside the private smoke environment. Its line is
/// deliberately emitted where native reqwest is about to issue the walk, so
/// `sort=title` proves restored behavior rather than merely restored bytes.
///
/// The line goes straight to stderr as plain text rather than through
/// `tracing`, on purpose: the smoke script greps the redirected log for a
/// literal `query=sort=title`, and `tracing-subscriber`'s fmt layer styles
/// field names and `=` with ANSI escapes by default — the pinned 0.3.23
/// turns styling on whenever its `ansi` feature is compiled in and
/// `NO_COLOR` is unset, without asking whether stderr is a terminal. On CI
/// (run 32584494800, PR #210) the bytes were therefore
/// `\e[3mquery\e[0m\e[2m=\e[0msort=title`, the grep never matched, and the
/// feature under test had worked all along. A hook whose whole job is to be
/// grepped must own its own formatting. The format is a contract with
/// `scripts/desktop-smoke.sh`'s `wait_for_listing_request`; change both
/// together.
///
/// Armed-ness is decided once per process: the script sets the environment
/// before launch and nothing changes it afterwards, so re-reading the
/// variable on every listing walk bought nothing.
pub(crate) fn log_smoke_session_query(query: &str) {
    static ARMED: LazyLock<bool> =
        LazyLock::new(|| std::env::var_os("FARHELM_SMOKE_CLIENT_LOG_MARKER").is_some());
    if *ARMED {
        eprintln!("desktop_smoke: session listing requested query={query}");
    }
}

/// Read the current bootstrap token for a webview exchange. Rotation can
/// happen in a separate bundled-CLI process, so retaining the startup value
/// would retry a credential the helm has already invalidated.
pub(crate) async fn current_token() -> anyhow::Result<String> {
    let runtime = RUNTIME_AUTH
        .get()
        .context("desktop authentication runtime is not initialized")?;
    farhelm_helm::show_token(Some(runtime.state_dir.clone())).await
}

/// Bring the window to the foreground once dioxus first makes it visible.
///
/// Without this, launching the desktop app from a terminal (laptop-dev.sh
/// nohup's the bundle's inner binary; `cargo run` hits it too) puts the icon
/// in the macOS dock but leaves the window behind the terminal until the user
/// cmd-tabs to it. The cause is a lifecycle mismatch: tao activates the app
/// exactly once, in `applicationDidFinishLaunching` — its launch handler calls
/// `activateIgnoringOtherApps` plus a `window_activation_hack` that
/// `makeKeyAndOrderFront`s every *visible* window. dioxus-desktop, however,
/// creates its window later (at `StartCause::Init`) and deliberately keeps it
/// hidden until the first render's edits are applied, to avoid a white flash.
/// So at activation time there is nothing to bring forward, and when the
/// window finally shows, nothing re-activates the app.
///
/// tao's `Window::set_focus` is the precise remedy — on macOS it dispatches
/// `makeKeyAndOrderFront` + `activateIgnoringOtherApps` to the main thread —
/// but it silently no-ops while the window is invisible, which is exactly the
/// state a mount-time effect can observe. Hence the poll: wait for
/// dioxus-desktop's own `set_visible(true)`, then focus. The deadline covers
/// the window never becoming visible (it should within the first render);
/// past it the task exits without focusing rather than lingering forever.
///
/// On GTK `set_focus` presents the window, which at startup is a no-op or
/// harmless. NOTE: macOS 14+ treats `activateIgnoringOtherApps` as a
/// cooperative activation request, so the OS can in principle still decline;
/// this is the strongest lever tao's public API offers.
pub(crate) fn use_foreground_on_launch() {
    use dioxus::prelude::{spawn, use_hook};
    use_hook(|| {
        spawn(async {
            let window = dioxus::desktop::window();
            let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
            while !window.window.is_visible() {
                if tokio::time::Instant::now() > deadline {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            window.window.set_focus();
        });
    });
}

/// Replace the native credential after the helm explicitly returns 401.
///
/// The boolean reports whether this caller performed the exchange. Concurrent
/// callers validate and reuse that result so only the winner remounts the
/// webview authentication gate.
pub(crate) async fn refresh_native_device() -> anyhow::Result<(String, bool)> {
    let _guard = DEVICE_REFRESH
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let runtime = RUNTIME_AUTH
        .get()
        .context("desktop authentication runtime is not initialized")?;
    let token = farhelm_helm::show_token(Some(runtime.state_dir.clone())).await?;
    let previous = crate::auth::device_secret();
    let secret = native_credential(
        &runtime.base,
        &token,
        previous.as_deref(),
        tokio::time::Instant::now() + DESKTOP_STARTUP_TIMEOUT,
    )
    .await?;
    if previous.as_deref() == Some(&secret) {
        return Ok((secret, false));
    }
    persist_then_publish_native(&runtime.state_path, secret.clone(), |secret| {
        crate::auth::install_native_device_secret(secret);
    })?;
    Ok((secret, true))
}

/// Commit a replacement native credential before making it visible to REST.
///
/// `publish` is a seam rather than direct global access so persistence
/// failures can be tested without mutating process-wide credential state.
fn persist_then_publish_native(
    state_path: &Path,
    secret: String,
    publish: impl FnOnce(String),
) -> anyhow::Result<PersistedState> {
    let state = update_state(state_path, |state| {
        state.native_device_secret = Some(secret.clone());
    })?;
    publish(secret);
    Ok(state)
}

/// Reuse a credential only when the helm accepts it, otherwise exchange the
/// current file-backed token after an explicit authentication rejection.
///
/// Transport failures and non-authentication HTTP failures remain startup
/// errors: silently minting around them would hide a broken embedded helm and
/// consume another entry in its bounded device table.
async fn native_credential(
    base: &str,
    token: &str,
    persisted: Option<&str>,
    deadline: tokio::time::Instant,
) -> anyhow::Result<String> {
    let client = loopback_client()?;
    if let Some(secret) = persisted {
        let response = tokio::time::timeout_at(
            deadline,
            client
                .get(format!("{base}/api/hosts"))
                .bearer_auth(secret)
                .send(),
        )
        .await
        .map_err(|_| {
            anyhow::anyhow!("desktop credential bootstrap did not complete within 30 seconds")
        })?
        .context("validating persisted native device session")?;
        if response.status().is_success() {
            return Ok(secret.to_string());
        }
        if response.status() != reqwest::StatusCode::UNAUTHORIZED {
            bail!(
                "persisted native device check failed with {}",
                response.status()
            );
        }
    }
    let response = tokio::time::timeout_at(
        deadline,
        client
            .post(format!("{base}/api/auth/token"))
            .json(&serde_json::json!({ "token": token }))
            .send(),
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!("desktop credential bootstrap did not complete within 30 seconds")
    })?
    .context("exchanging the desktop token for native REST")?;
    if !response.status().is_success() {
        bail!("native device exchange failed with {}", response.status());
    }
    tokio::time::timeout_at(deadline, response.json::<DeviceExchange>())
        .await
        .map_err(|_| {
            anyhow::anyhow!("desktop credential bootstrap did not complete within 30 seconds")
        })?
        .context("decoding native device exchange")
        .map(|exchange| exchange.device_secret)
}

/// Fail startup if the supervisor child owned by this app has already exited.
///
/// A successful readiness probe must not accidentally bless a rival process
/// that won the local socket after our discovery-first spawn lost a race.
fn ensure_managed_supervisor_running(supervisor: &mut Option<Child>) -> anyhow::Result<()> {
    if let Some(child) = supervisor
        && let Some(status) = child
            .try_wait()
            .context("monitoring the managed supervisor child during desktop startup")?
    {
        bail!("the managed supervisor child exited during desktop startup with {status}");
    }
    Ok(())
}

/// Wait for the reserved local row to reach the supervisor started above.
/// The manager intentionally starts actors without waiting for connections;
/// desktop startup is the consumer that needs the stronger readiness point.
async fn await_local_supervisor(
    base: &str,
    state_dir: &Path,
    state_path: &Path,
    state: &mut PersistedState,
    supervisor: &mut Option<Child>,
) -> anyhow::Result<()> {
    await_local_supervisor_until(
        base,
        state_dir,
        state_path,
        state,
        supervisor,
        tokio::time::Instant::now() + Duration::from_secs(30),
    )
    .await
}

/// Poll the managed row under one deadline, refreshing only a structured
/// desktop-auth rejection and preserving every other HTTP failure.
async fn await_local_supervisor_until(
    base: &str,
    state_dir: &Path,
    state_path: &Path,
    state: &mut PersistedState,
    supervisor: &mut Option<Child>,
    deadline: tokio::time::Instant,
) -> anyhow::Result<()> {
    let client = loopback_client()?;
    loop {
        ensure_managed_supervisor_running(supervisor)?;
        let secret = state
            .native_device_secret
            .as_deref()
            .context("desktop native credential is missing during supervisor readiness")?;
        let response = tokio::time::timeout_at(
            deadline,
            client
                .get(format!("{base}/api/hosts"))
                .bearer_auth(secret)
                .send(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("managed local supervisor did not connect within 30 seconds"))?
        .context("checking the managed local supervisor")?;
        let status = response.status();
        if status.is_success() {
            let hosts = tokio::time::timeout_at(deadline, crate::api::decode_hosts(response))
                .await
                .map_err(|_| {
                    anyhow::anyhow!("managed local supervisor did not connect within 30 seconds")
                })?
                .map_err(anyhow::Error::msg)?;
            if hosts.iter().any(|host| {
                host.kind == crate::HostKind::Local
                    && matches!(host.state, crate::HostPhase::Connected { .. })
            }) {
                ensure_managed_supervisor_running(supervisor)?;
                return Ok(());
            }
        } else {
            let body = tokio::time::timeout_at(deadline, response.text())
                .await
                .map_err(|_| {
                    anyhow::anyhow!("managed local supervisor did not connect within 30 seconds")
                })?
                .context("reading the managed local supervisor refusal")?;
            if status == reqwest::StatusCode::UNAUTHORIZED
                && crate::api::device_auth_required(&body)
            {
                let token = tokio::time::timeout_at(
                    deadline,
                    farhelm_helm::show_token(Some(state_dir.to_path_buf())),
                )
                .await
                .map_err(|_| {
                    anyhow::anyhow!("managed local supervisor did not connect within 30 seconds")
                })??;
                let replacement = tokio::time::timeout_at(
                    deadline,
                    native_credential(base, &token, None, deadline),
                )
                .await
                .map_err(|_| {
                    anyhow::anyhow!("managed local supervisor did not connect within 30 seconds")
                })??;
                *state = persist_then_publish_native(state_path, replacement, |secret| {
                    crate::auth::install_native_device_secret(secret);
                })?;
                continue;
            }
            let detail = body.trim();
            bail!(
                "managed local supervisor check failed with {status}{}",
                if detail.is_empty() {
                    String::new()
                } else {
                    format!(": {detail}")
                }
            );
        }
        if tokio::time::Instant::now() >= deadline {
            bail!("managed local supervisor did not connect within 30 seconds");
        }
        tokio::time::sleep_until(std::cmp::min(
            deadline,
            tokio::time::Instant::now() + Duration::from_millis(100),
        ))
        .await;
    }
}

/// Build an HTTP client that cannot forward loopback bearer credentials to
/// an ambient proxy configured for the desktop process.
fn loopback_client() -> anyhow::Result<reqwest::Client> {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .context("building proxy-free desktop HTTP client")
}

/// The macOS Homebrew/MacPorts prefixes `resolve_supervisor_tmux` should
/// probe, or an empty list everywhere else.
///
/// Kept as a thin `cfg!` wrapper (a runtime branch), not a `#[cfg(target_os =
/// "macos")]` item, so the branch itself — and everything it feeds — stays
/// compiled and exercised on every CI host, matching `tmux_probe`'s own
/// platform-agnostic design (see that module's docs).
fn macos_tmux_prefixes() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        crate::tmux_probe::MACOS_TMUX_PREFIXES
    } else {
        &[]
    }
}

/// Real executability check behind the macOS tmux probe: a regular file with
/// at least one execute bit set.
///
/// `tmux_probe::find_tmux_in_prefixes` takes this as an injected predicate so
/// its search-ORDER logic can be unit-tested without touching a real
/// filesystem; this function is the one real implementation, used only at
/// this call site.
fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Decide the `FARHELM_TMUX` value (if any) the managed supervisor should be
/// launched with.
///
/// Precedence, per SPEC_impl.md's "Terminal substrate: private tmux server"
/// ("the desktop app takes an ambient `FARHELM_TMUX` as given, otherwise
/// probes ..."): an `ambient_override` already present in the app's own
/// environment is passed through UNTOUCHED and the probe is
/// skipped entirely — the user's choice always wins, and probing anyway
/// would risk silently overruling it with a different Homebrew install found
/// first. Only absent an override does `prefixes` get searched (macOS only in
/// practice; every other platform passes an empty list here and leaves
/// resolution to the supervisor's own PATH lookup, unchanged from before this
/// probe existed).
///
/// `prefixes` and `is_executable` are parameters, not globals, purely so this
/// precedence can be unit-tested by passing in fake values directly —
/// without mutating `std::env` (which races every other test reading the
/// same process-wide environment) or touching a real filesystem.
fn resolve_supervisor_tmux(
    ambient_override: Option<std::ffi::OsString>,
    prefixes: &[&str],
    is_executable: impl FnMut(&Path) -> bool,
) -> Option<std::ffi::OsString> {
    // An EMPTY value means "no override", exactly as the supervisor reads
    // it (a profile or unit that writes `FARHELM_TMUX=` means that to
    // whoever wrote it); passing it through would skip the probe and then
    // fall back to the bare `tmux` on Finder's PATH, which is the case the
    // probe exists for.
    if let Some(value) = ambient_override.filter(|value| !value.is_empty()) {
        return Some(value);
    }
    crate::tmux_probe::find_tmux_in_prefixes(prefixes, is_executable).map(PathBuf::into_os_string)
}

/// Which OS's install guidance a tmux refusal should give.
///
/// A parameter to [`tmux_refusal_message`] rather than a `cfg!` branch
/// baked into that function, for the same reason `macos_tmux_prefixes`
/// wraps its `cfg!` in a runtime branch: parameterizing the platform lets
/// CI exercise BOTH pure message variants on whatever host it runs on,
/// rather than compiling the macOS wording out wherever CI happens not to
/// be a Mac. The one real call site ([`run_tmux_preflight_or_exit`]) still
/// picks with `cfg!(target_os = "macos")`, once, at the moment it actually
/// matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TmuxRefusalPlatform {
    Mac,
    Linux,
}

/// The two ways this app's tmux preflight refuses startup outright before
/// spawning its managed supervisor, each rendered by
/// [`tmux_refusal_message`] as one plain stderr message followed by exit
/// status 1 — see `SPEC_impl.md`'s "Terminal substrate: private tmux
/// server" section for why the desktop owns this check at all.
///
/// Deliberately narrower than [`farhelm_supervisor::tmux::TmuxSupport`]:
/// `AtFloor` and `AbovePin` both mean "let it proceed" here (an above-pin
/// tmux still earns its own "unaudited" warning, but that is the
/// supervisor's concern once it actually starts a server, not this
/// preflight's), so [`classify_tmux_preflight`] folds both into `Ok` rather
/// than growing a matching third variant of this enum that no caller would
/// ever act on differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TmuxRefusal {
    /// The probe could not run `program` at all: nothing answered to that
    /// name or path (`io::ErrorKind::NotFound`).
    NotFound,
    /// `program` answered `-V`, but with a version below
    /// [`farhelm_supervisor::tmux::TMUX_FLOOR`].
    BelowFloor {
        found: farhelm_supervisor::tmux::TmuxVersion,
    },
}

/// Whether the tmux this app is about to hand its managed supervisor clears
/// Farhelm's version floor — the classification half of the preflight,
/// deliberately separated from running the probe itself.
///
/// Takes the ALREADY-COMPLETED [`farhelm_supervisor::tmux::probe_tmux`]
/// result rather than a program to probe or a callback that would run one:
/// classification is then pure data-in-data-out, so tests construct a
/// canned `TmuxProbe` or `TmuxProbeError` directly and never spawn a real
/// process — this preflight is exercised the same way regardless of
/// whether the host running the tests has a below-floor or missing tmux
/// anywhere on it. The real call site, [`run_tmux_preflight_or_exit`], is
/// the only thing that actually calls `probe_tmux`, and it does so through
/// the supervisor's own BOUNDED probe (a time limit, a captured-output
/// cap, and its own process group) rather than a raw `Command::output()` —
/// this preflight runs before any of Farhelm's other startup machinery
/// exists, so an unbounded probe against a hostile or merely broken
/// program named by `--tmux`/`FARHELM_TMUX` could otherwise hang or
/// exhaust memory in an already-invisible GUI process.
///
/// Any failure this cannot make sense of (a spawn failure that is not
/// `ENOENT`, a nonzero `-V` exit, unparseable `-V` output) is reported
/// through the ordinary `anyhow::Error` path instead of inventing a third
/// `TmuxRefusal` variant: those are genuinely unexpected, and this
/// preflight's specialized wording is specifically for the two predictable
/// ways a real Mac without an acceptable tmux fails, not for surprises it
/// has no tailored message for.
fn classify_tmux_preflight(
    program: &Path,
    probed: Result<farhelm_supervisor::tmux::TmuxProbe, farhelm_supervisor::tmux::TmuxProbeError>,
) -> anyhow::Result<Option<TmuxRefusal>> {
    use farhelm_supervisor::tmux::{TmuxProbeError, TmuxSupport};

    match probed {
        Ok(probe) => Ok(match probe.support {
            TmuxSupport::BelowFloor => Some(TmuxRefusal::BelowFloor {
                found: probe.version,
            }),
            TmuxSupport::AtFloor | TmuxSupport::AbovePin => None,
        }),
        // `ENOENT`: the same shape `std::process::Command` produces when
        // the OS could not find `program` at all, whether that means
        // nothing answered to that name/path or something answered but
        // could not be run (a missing shebang interpreter or dynamic
        // loader looks identical to a genuinely absent target on Unix —
        // see `tmux_refusal_message`'s `NotFound` wording).
        Err(TmuxProbeError::NotRunnable(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(Some(TmuxRefusal::NotFound))
        }
        Err(TmuxProbeError::NotRunnable(error)) => Err(error).with_context(|| {
            format!(
                "running {} -V during the desktop tmux preflight",
                program.display()
            )
        }),
        Err(error @ TmuxProbeError::Unparseable(_)) => {
            bail!(
                "{} -V did not produce a usable version: {error}",
                program.display()
            )
        }
        Err(error @ TmuxProbeError::Overran(_)) => {
            bail!("{} -V {error}", program.display())
        }
    }
}

/// Human labels for the places `resolve_supervisor_tmux` actually looked
/// this run, in the order it looked, for the "looked at: ..." clause of a
/// [`TmuxRefusal::NotFound`] message.
///
/// Mirrors `resolve_supervisor_tmux`'s own precedence exactly — an ambient
/// override skips every other probe there, so it must be the ONLY entry
/// here too, or the message would claim to have searched somewhere the
/// real resolution never looked. `MacPorts` is a friendlier label for
/// `/opt/local/bin` than the bare path would be; the other two prefixes are
/// self-explanatory Homebrew locations and are shown verbatim.
fn tmux_probe_targets(ambient: Option<&std::ffi::OsStr>, prefixes: &[&str]) -> Vec<String> {
    if let Some(value) = ambient.filter(|value| !value.is_empty()) {
        return vec![value.to_string_lossy().into_owned()];
    }
    let mut targets = vec!["FARHELM_TMUX".to_string()];
    targets.extend(prefixes.iter().map(|&prefix| {
        if prefix == "/opt/local/bin" {
            "MacPorts".to_string()
        } else {
            prefix.to_string()
        }
    }));
    targets.push("PATH".to_string());
    targets
}

/// Render `items` as an English list ending in "..., then LAST" — the shape
/// both refusal-message probe clauses use ("FARHELM_TMUX, /opt/homebrew/bin,
/// ..., then PATH"), because these are steps tried IN ORDER, and "then"
/// says that where a plain comma list would not.
fn join_with_then(items: &[String]) -> String {
    match items.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{}, then {last}", rest.join(", ")),
    }
}

/// Render the exact stderr text [`run_tmux_preflight_or_exit`] prints
/// before exiting — the whole required message: one plain sentence naming
/// what went wrong and where farhelm looked, one plain sentence saying how
/// to fix it, and nothing else — no panic, no backtrace hint, no
/// supervisor-shaped `WARN`/`Caused by` chain ahead of it.
///
/// Pure and parameterized over [`TmuxRefusalPlatform`] specifically so both
/// platforms' wording is exercised by plain `cargo test` on whatever host
/// runs it — the actual macOS wording would otherwise be unverified by
/// anything this project's CI runs. `probed` is threaded through rather
/// than recomputed so the message can never disagree with what
/// [`tmux_probe_targets`] (called once, at the real call site) says was
/// actually tried. `program` is the DISPLAY spelling the caller resolved
/// (see [`farhelm_supervisor::tmux::program_display_path`]) — a below-floor
/// refusal has to name the concrete binary that answered, not the bare
/// spelling `--tmux`/`FARHELM_TMUX` happened to carry, or two installs
/// sharing a bare name would be indistinguishable in the message.
///
/// `override_in_force` says whether an ambient `FARHELM_TMUX` — rather
/// than the fixed-prefix probe or a bare `PATH` lookup — is what selected
/// `program`. It changes the REMEDY, not the diagnosis: telling a user
/// whose override names a bad binary to "restart the app" is useless
/// advice, because restarting with the same unchanged override reproduces
/// the identical refusal without ever considering a freshly installed
/// tmux.
///
/// The floor version is read from [`farhelm_supervisor::tmux::TMUX_FLOOR`]
/// — the ONLY source of the "3.7c" text — so a future floor bump cannot
/// leave this message quoting a stale one.
fn tmux_refusal_message(
    refusal: TmuxRefusal,
    program: &Path,
    platform: TmuxRefusalPlatform,
    probed: &[String],
    override_in_force: bool,
) -> String {
    use farhelm_supervisor::tmux::TMUX_FLOOR;

    let subject = match refusal {
        // Deliberately does not claim outright that nothing EXISTS at any
        // of these locations: on Unix, the same `ENOENT` this preflight
        // classifies as `NotFound` also covers a script whose shebang
        // interpreter disappeared or a binary whose loader is missing, and
        // telling those two apart would need an existence check this
        // preflight does not perform (see `classify_tmux_preflight`).
        TmuxRefusal::NotFound => format!(
            "farhelm-desktop needs tmux {TMUX_FLOOR} or newer, and none could be run (looked \
             at: {}). Each one was either not found, or is missing its interpreter or loader.",
            join_with_then(probed)
        ),
        TmuxRefusal::BelowFloor { found } => format!(
            "found tmux {found} at {}, which is below the {TMUX_FLOOR} farhelm needs.",
            program.display()
        ),
    };
    let body = match (platform, override_in_force) {
        // The indented command block matches how the README and
        // provisioning already point Mac users at Homebrew — a bare
        // sentence mentioning `brew install tmux` reads like trivia next to
        // an exact command a person can paste.
        (TmuxRefusalPlatform::Mac, false) => "On macOS, tmux has to be installed by hand; \
             Homebrew is the recommended way:\n\n    brew install tmux\n\nThen start \
             farhelm-desktop again."
            .to_string(),
        (TmuxRefusalPlatform::Mac, true) => "On macOS, tmux has to be installed by hand; \
             Homebrew is the recommended way:\n\n    brew install tmux\n\nFARHELM_TMUX is set \
             and overrides that search, so update it to point at the new install (or unset it) \
             before starting farhelm-desktop again."
            .to_string(),
        (TmuxRefusalPlatform::Linux, false) => format!(
            "Install tmux {TMUX_FLOOR} or newer with your package manager or Linuxbrew (`brew \
             install tmux`), or point FARHELM_TMUX at one.\nThen start farhelm-desktop again."
        ),
        (TmuxRefusalPlatform::Linux, true) => format!(
            "Install tmux {TMUX_FLOOR} or newer with your package manager or Linuxbrew (`brew \
             install tmux`). FARHELM_TMUX is set and overrides where farhelm-desktop looks, so \
             update it to point at a supported tmux, or unset it, before starting \
             farhelm-desktop again."
        ),
    };
    format!("{subject}\n{body}")
}

/// Check the tmux this app is about to hand its managed supervisor against
/// Farhelm's floor, and if it refuses, print one plain message and exit —
/// called from [`DesktopBootstrap::start`]'s `Absent` branch, immediately
/// before that branch spawns the managed supervisor child, which is the
/// only situation where this process is about to choose and configure a
/// tmux at all (an answering supervisor is reused with whatever tmux it
/// already has, untouched — see `start`'s own doc comment on that
/// ordering).
///
/// `supervisor_tmux` is the SAME value [`DesktopBootstrap::start`] hands
/// its managed supervisor through `FARHELM_TMUX` (`None` meaning "let the
/// supervisor's own bare `tmux` / PATH lookup apply", exactly as it does
/// there) — resolved via [`farhelm_supervisor::tmux::resolve_tmux_program`]
/// so this preflight probes the IDENTICAL program the child would have
/// resolved to. Probing a different one would leave preflight and child
/// free to disagree, which is worse than no preflight: a green preflight
/// followed by the child's own refusal is the exact confusing, panicky
/// experience this whole feature exists to remove.
///
/// The probe itself goes through [`farhelm_supervisor::tmux::probe_tmux`],
/// not a raw `Command::output()`: this call happens before ANY of
/// Farhelm's other startup machinery exists, so the candidate named by
/// `--tmux`/`FARHELM_TMUX` gets the same bounded treatment (a time limit,
/// a captured-output cap, its own process group) the supervisor already
/// gives every operator-supplied tmux candidate, rather than a synchronous
/// call with no deadline that a wedged or chatty wrapper could hang or
/// exhaust memory in.
///
/// Any OTHER bootstrap failure (state directory unreadable, port already
/// bound, ...) is deliberately NOT this function's problem: it returns
/// `Ok(())` and lets `DesktopBootstrap::start` continue, so those failures
/// still surface through the ordinary `anyhow::Error` path that
/// `desktop::run` now prints plainly rather than panics on.
fn run_tmux_preflight_or_exit(
    supervisor_tmux: Option<&std::ffi::OsStr>,
    ambient_tmux: Option<&std::ffi::OsStr>,
    prefixes: &[&str],
) -> anyhow::Result<()> {
    let program = farhelm_supervisor::tmux::resolve_tmux_program(None, supervisor_tmux);
    let refusal =
        classify_tmux_preflight(&program, farhelm_supervisor::tmux::probe_tmux(&program))?;
    if let Some(refusal) = refusal {
        let probed = tmux_probe_targets(ambient_tmux, prefixes);
        let platform = if cfg!(target_os = "macos") {
            TmuxRefusalPlatform::Mac
        } else {
            TmuxRefusalPlatform::Linux
        };
        // ADVISORY resolution purely for the message (see
        // `program_display_path`'s own doc comment): a below-floor refusal
        // that printed the bare spelling ("found tmux 3.6 at tmux") would
        // not tell a reader with several tmux installs which one answered.
        // The probe above already ran the ORIGINAL spelling, so this
        // read-only lookup adds no time-of-check/time-of-use gap in what
        // actually executed.
        let display_program = farhelm_supervisor::tmux::program_display_path(
            &program,
            std::env::var_os("PATH").as_deref(),
        );
        let override_in_force = ambient_tmux.filter(|value| !value.is_empty()).is_some();
        eprintln!(
            "{}",
            tmux_refusal_message(
                refusal,
                &display_program,
                platform,
                &probed,
                override_in_force
            )
        );
        std::process::exit(1);
    }
    Ok(())
}

fn desktop_state_dir() -> anyhow::Result<PathBuf> {
    match std::env::var_os("FARHELM_DESKTOP_STATE_DIR") {
        Some(path) => Ok(PathBuf::from(path)),
        None => farhelm_supervisor::default_state_dir(),
    }
}

/// Locate the `farhelm` CLI this app spawns its supervisor from.
///
/// D6 ships two bare binaries that install side by side (`~/.local/bin`), so
/// "next to me" is the whole contract — there is no bundle to look INSIDE.
/// The installer-assembled `Farhelm.app` (SPEC_impl.md, "Native app
/// packaging") satisfies the same contract from the other direction: it
/// places a `farhelm` copy next to the executable in `Contents/MacOS/`,
/// which is why this code needs no bundle awareness to run from either
/// location. `FARHELM_DESKTOP_FARHELM` overrides it for developers and for
/// `scripts/desktop-smoke.sh`, which runs a `dx` build tree where the sibling
/// does not exist.
///
/// The failure text names the exact path that was tried and both ways out,
/// because the person hitting it is looking at a GUI app that refused to
/// start with no other diagnostic.
fn bundled_farhelm() -> anyhow::Result<PathBuf> {
    let current = std::env::current_exe().context("locating desktop executable")?;
    resolve_sibling_farhelm(
        &current,
        std::env::var_os("FARHELM_DESKTOP_FARHELM").as_deref(),
    )
}

/// Decide where `farhelm` is, given this executable's path and the override.
///
/// Split from [`bundled_farhelm`] so the decision can be tested: the release
/// contract is "the two binaries are installed side by side", and nothing
/// else in this repository's automation exercises it — the smoke always sets
/// `FARHELM_DESKTOP_FARHELM`, and the asset check exits through
/// `--print-assets` before bootstrap runs. Reading `current_exe` and the
/// environment stays in the caller so the rule itself needs neither.
///
/// The override wins unconditionally, INCLUDING over a sibling that exists
/// and including when it names something that does not: a developer pointing
/// at a specific build wants that build or a clear failure from it, not a
/// silent fall back to whatever happens to be next to the running binary.
/// The only filesystem question asked here is whether the sibling is a file.
fn resolve_sibling_farhelm(
    current_exe: &Path,
    override_path: Option<&std::ffi::OsStr>,
) -> anyhow::Result<PathBuf> {
    if let Some(path) = override_path {
        return Ok(PathBuf::from(path));
    }
    let sibling = current_exe.with_file_name("farhelm");
    if sibling.is_file() {
        return Ok(sibling);
    }
    bail!(
        "farhelm-desktop needs the farhelm binary next to it ({}) and did not find one; \
         run the install script or set FARHELM_DESKTOP_FARHELM",
        sibling.display()
    )
}

/// The UI tree the EMBEDDED HELM serves over loopback, if a developer named
/// one.
///
/// `None` is the normal answer, and it is not a failure: a release build
/// carries the tree compiled in (D12) and the helm falls back to that, while
/// a plain `cargo build -p farhelm-desktop` genuinely has no UI to serve and
/// says so in its own log (`farhelm-helm`'s `warn_if_no_ui`).
///
/// Note the scope: this only decides what the loopback HELM answers with. The
/// native window never loads that page — it renders the component tree in the
/// webview and pulls its `/assets/*` from [`serve_asset`] instead — so an
/// override here does NOT change what the window shows. The
/// `Contents/Resources/web` lookup this used to perform is gone with the
/// dx-produced `.app` bundle it belonged to (D6); the installer-assembled
/// `Farhelm.app` carries no web tree either, so nothing brings it back.
fn bundled_web_ui() -> Option<PathBuf> {
    std::env::var_os("FARHELM_DESKTOP_UI_DIST").map(PathBuf::from)
}

fn read_state(path: &Path) -> anyhow::Result<PersistedState> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .with_context(|| format!("reading desktop state from {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PersistedState::default()),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

/// Serialize every read-modify-replace of the desktop state file.
///
/// The native credential refresh and the webview credential commit can
/// arrive concurrently. Locking the whole merge is what prevents either
/// writer from reinstalling a snapshot that silently drops the field the
/// other just committed. The file holds credentials and nothing else now:
/// the list preference that once shared it lives in the helm (SPEC.md,
/// Session list), so no non-credential writer ever contends here.
fn update_state(
    path: &Path,
    mutate: impl FnOnce(&mut PersistedState),
) -> anyhow::Result<PersistedState> {
    let _guard = STATE_FILE_WRITE
        .lock()
        .expect("desktop state-file lock poisoned");
    let mut state = read_state(path)?;
    mutate(&mut state);
    write_state(path, &state)?;
    Ok(state)
}

fn write_state(path: &Path, state: &PersistedState) -> anyhow::Result<()> {
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec(state).context("encoding desktop state")?;
    let mut file =
        File::create(&temporary).with_context(|| format!("opening {}", temporary.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(&bytes)?;
    file.sync_all()?;
    std::fs::rename(&temporary, path)
        .with_context(|| format!("installing desktop state at {}", path.display()))?;
    // The file fsync protects its bytes; syncing the containing directory
    // makes the rename itself survive a crash or sudden power loss.
    File::open(path.parent().context("desktop state path has no parent")?)?
        .sync_all()
        .context("syncing the desktop state directory")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    const PROXY_CHILD_ENV: &str = "FARHELM_DESKTOP_PROXY_TEST_CHILD";
    const PROXY_TARGET_ENV: &str = "FARHELM_DESKTOP_PROXY_TEST_TARGET";

    // ---- the desktop asset handler (D6) ----
    //
    // These drive `serve_asset_from` against a fixture lookup rather than the
    // real embedded tree, which only a release-shaped build populates. What
    // is under test is the RESPONSE CONTRACT: with the handler registered,
    // dioxus stops consulting its own filesystem resolver for `/assets/*`
    // (see `use_embedded_asset_handler`), so whatever this function returns
    // is the entire answer the webview gets — there is nothing behind it to
    // paper over a wrong status, a wrong content type, or a body that is
    // subtly not the file.

    /// A two-entry embedded tree: one JavaScript asset and one whose name
    /// needs percent-escaping in a URL.
    fn fixture_lookup(relative: &str) -> Option<&'static [u8]> {
        match relative {
            "assets/terminal-dxhabc.js" => Some(b"console.log('hi')\n"),
            "assets/a name with spaces.css" => Some(b"body{}"),
            _ => None,
        }
    }

    fn method(name: &str) -> dioxus::desktop::wry::http::Method {
        dioxus::desktop::wry::http::Method::from_bytes(name.as_bytes())
            .expect("test method is valid")
    }

    /// Serve against the fixture tree — the release-shaped configuration.
    fn serve(method_name: &str, path: &str) -> dioxus::desktop::wry::http::Response<Vec<u8>> {
        serve_asset_from(Some(&fixture_lookup), &method(method_name), path)
    }

    /// Serve against no embedded tree at all — the plain-`cargo build`
    /// configuration.
    fn serve_unembedded(
        method_name: &str,
        path: &str,
    ) -> dioxus::desktop::wry::http::Response<Vec<u8>> {
        serve_asset_from(None, &method(method_name), path)
    }

    /// A hit returns the file's exact bytes, a guessed content type, and the
    /// permissive CORS header dioxus's own resolver stamps on assets.
    ///
    /// The content type is the load-bearing part: a webview that receives
    /// `application/octet-stream` for a `<script src>` refuses to execute it,
    /// which looks exactly like an asset that never loaded.
    #[test]
    fn a_hit_returns_the_bytes_with_a_guessed_content_type() {
        let response = serve("GET", "/assets/terminal-dxhabc.js");
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/javascript"
        );
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .unwrap(),
            "*"
        );
        assert_eq!(response.body().as_slice(), b"console.log('hi')\n");
    }

    /// A percent-escaped path resolves to the file whose real name contains
    /// those characters.
    ///
    /// Decoding has to happen before BOTH the lookup and the `mime_guess`
    /// call: an undecoded `%20` would miss the entry, and a path decoded
    /// after the type guess would classify by the wrong extension.
    #[test]
    fn a_percent_escaped_path_is_decoded_once_before_lookup_and_typing() {
        let response = serve("GET", "/assets/a%20name%20with%20spaces.css");
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers().get("content-type").unwrap(), "text/css");
        assert_eq!(response.body().as_slice(), b"body{}");
    }

    /// `HEAD` returns exactly what `GET` would, body included.
    ///
    /// wry hands the response back untouched — there is no router layer here
    /// to strip the body and recompute `Content-Length` the way axum does for
    /// the helm — so returning the same response is both the simplest and the
    /// only truthful option.
    #[test]
    fn head_is_answered_identically_to_get() {
        let head = serve("HEAD", "/assets/terminal-dxhabc.js");
        let get = serve("GET", "/assets/terminal-dxhabc.js");
        assert_eq!(head.status(), get.status());
        assert_eq!(head.headers(), get.headers());
        assert_eq!(head.body(), get.body());
    }

    /// Anything other than `GET`/`HEAD` is refused before any lookup, with
    /// the `Allow` header that makes the refusal actionable.
    ///
    /// Parity with `farhelm-helm`'s `serve_embedded`, so the same request
    /// gets the same answer from the native window and from the browser.
    #[test]
    fn a_write_method_is_refused_with_the_allowed_set() {
        let response = serve("POST", "/assets/terminal-dxhabc.js");
        assert_eq!(response.status(), 405);
        assert_eq!(response.headers().get("allow").unwrap(), "GET,HEAD");
        assert!(response.body().is_empty());
    }

    /// A path with no entry is a 404, not an `index.html` fallback.
    ///
    /// The helm's embedded source answers an extension-less miss with
    /// `index.html` so a single-page route can render. This route must NOT:
    /// its only clients are `<script>`, `<link>` and font requests, and
    /// handing one of those a page of HTML produces a parse error instead of
    /// a legible failure.
    #[test]
    fn a_miss_is_a_plain_404_with_no_spa_fallback() {
        for path in ["/assets/never-bundled.js", "/assets/looks-like-a-route"] {
            let response = serve("GET", path);
            assert_eq!(response.status(), 404, "{path}");
            assert!(response.body().is_empty(), "{path}");
        }
    }

    /// A percent escape that decodes to invalid UTF-8 is a miss, not a panic.
    ///
    /// Every embedded entry is keyed by a Rust string literal, so no such
    /// path could ever match one. The webview is not a trusted input source
    /// in the sense that matters here: this runs in the app's own process,
    /// and a panic in the handler takes the window with it.
    #[test]
    fn an_undecodable_path_is_a_miss_rather_than_a_panic() {
        let response = serve("GET", "/assets/%ff%fe.js");
        assert_eq!(response.status(), 404);
    }

    /// A build with no embedded UI answers every asset with an empty 404.
    ///
    /// D12 makes that a supported arrangement, not a broken build: `cargo
    /// build -p farhelm-desktop` without `FARHELM_UI_DIST` opens a window
    /// with no UI in it. What must NOT happen is a panic, a partial
    /// response, or a revived filesystem fallback — registering the handler
    /// took `/assets/*` away from dioxus's resolver, so anything this
    /// returns is the whole answer.
    ///
    /// Reachable as a test only because the lookup is a parameter: whether
    /// this binary embedded a tree was decided when it was compiled.
    #[test]
    fn a_build_with_no_embedded_ui_answers_every_asset_with_an_empty_404() {
        for path in ["/assets/terminal-dxhabc.js", "/assets/anything-at-all"] {
            let response = serve_unembedded("GET", path);
            assert_eq!(response.status(), 404, "{path}");
            assert!(response.body().is_empty(), "{path}");
        }
    }

    /// The method gate runs before the embedded-tree question.
    ///
    /// Both orderings answer honestly, but this one keeps the refusal
    /// identical across build configurations: a client asking the wrong way
    /// gets `405` and `Allow` whether or not this build has a UI, rather
    /// than a 404 that suggests the path was the problem.
    #[test]
    fn a_write_method_is_refused_even_with_no_embedded_ui() {
        let response = serve_unembedded("POST", "/assets/terminal-dxhabc.js");
        assert_eq!(response.status(), 405);
        assert_eq!(response.headers().get("allow").unwrap(), "GET,HEAD");
    }

    // ---- the release contract: two binaries, side by side (D6) ----

    /// The override names the CLI outright, even when a sibling exists.
    ///
    /// `scripts/desktop-smoke.sh` depends on exactly this: it runs the dx
    /// build tree, where no `farhelm` sibling exists at all, and names the
    /// one it built.
    #[test]
    fn the_farhelm_override_wins_over_a_present_sibling() {
        let dir = tempfile::tempdir().expect("temp dir");
        let sibling = dir.path().join("farhelm");
        std::fs::write(&sibling, b"#!/bin/sh\n").expect("writing the sibling");
        let chosen = resolve_sibling_farhelm(
            &dir.path().join("farhelm-desktop"),
            Some(std::ffi::OsStr::new("/elsewhere/farhelm")),
        )
        .expect("an override is taken as given");
        assert_eq!(chosen, PathBuf::from("/elsewhere/farhelm"));
    }

    /// With no override, the CLI is the file named `farhelm` in this
    /// executable's own directory.
    ///
    /// This IS the installed shape (`~/.local/bin/farhelm` beside
    /// `~/.local/bin/farhelm-desktop`) and nothing else in the repository's
    /// automation exercises it — the smoke and the asset check both bypass
    /// it — so a regression here would first be noticed by a user.
    #[test]
    fn the_sibling_beside_this_executable_is_found_without_an_override() {
        let dir = tempfile::tempdir().expect("temp dir");
        let sibling = dir.path().join("farhelm");
        std::fs::write(&sibling, b"#!/bin/sh\n").expect("writing the sibling");
        let chosen = resolve_sibling_farhelm(&dir.path().join("farhelm-desktop"), None)
            .expect("a sibling next to the executable is found");
        assert_eq!(chosen, sibling);
    }

    /// A missing sibling fails with the exact text the distribution plan
    /// specifies, naming the path that was tried.
    ///
    /// Asserted verbatim because this string is the entire diagnostic a user
    /// gets: a GUI binary that refuses to start has no window to explain
    /// itself in, and the two ways out (the install script, the override)
    /// have to be in the message or they are nowhere.
    #[test]
    fn a_missing_sibling_names_the_path_and_both_ways_out() {
        let dir = tempfile::tempdir().expect("temp dir");
        let exe = dir.path().join("farhelm-desktop");
        let error = resolve_sibling_farhelm(&exe, None)
            .expect_err("no sibling was created, so this must fail");
        assert_eq!(
            format!("{error}"),
            format!(
                "farhelm-desktop needs the farhelm binary next to it ({}) and did not find one; \
                 run the install script or set FARHELM_DESKTOP_FARHELM",
                dir.path().join("farhelm").display()
            )
        );
    }

    /// A DIRECTORY named `farhelm` next to the executable is not the CLI.
    ///
    /// The check is `is_file` rather than "exists" for this case. Spawning a
    /// directory fails later and much less clearly than refusing here does.
    #[test]
    fn a_directory_named_farhelm_does_not_count_as_the_sibling() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("farhelm")).expect("creating the decoy directory");
        resolve_sibling_farhelm(&dir.path().join("farhelm-desktop"), None)
            .expect_err("a directory is not the CLI");
    }

    /// A `FARHELM_TMUX` already present in the app's own environment is the
    /// user's explicit override and must win outright — probing must not
    /// even run. Asserted here by making the
    /// injected predicate return `true` for everything: if the override were
    /// ignored, the probe would "find" a match and this test could not tell
    /// the two cases apart, so a passing predicate is what makes this a real
    /// test of precedence rather than of the override merely being present.
    #[test]
    fn ambient_tmux_override_wins_without_probing() {
        let resolved = resolve_supervisor_tmux(
            Some(std::ffi::OsString::from("/wherever/the/user/said/tmux")),
            &["/opt/homebrew/bin"],
            |candidate| {
                panic!("the probe must not run under an override; asked about {candidate:?}")
            },
        );
        assert_eq!(
            resolved,
            Some(std::ffi::OsString::from("/wherever/the/user/said/tmux"))
        );
    }

    /// Absent an override, the first prefix the predicate accepts is what the
    /// supervisor is launched with.
    #[test]
    fn probe_result_is_used_when_no_override_is_set() {
        let resolved = resolve_supervisor_tmux(
            None,
            &["/opt/homebrew/bin", "/usr/local/bin"],
            |candidate| candidate == Path::new("/usr/local/bin/tmux"),
        );
        assert_eq!(
            resolved,
            Some(std::ffi::OsString::from("/usr/local/bin/tmux"))
        );
    }

    /// No override and no prefix match (or, equivalently, an empty prefix
    /// list — the shape the non-macOS call site always passes) must yield
    /// `None`, leaving PATH resolution to the supervisor exactly as before
    /// this probe existed.
    #[test]
    fn no_override_and_no_probe_hit_falls_back_to_nothing() {
        assert_eq!(
            resolve_supervisor_tmux(None, &["/opt/homebrew/bin"], |_| false),
            None
        );
        assert_eq!(resolve_supervisor_tmux(None, &[], |_| true), None);
    }

    /// `FARHELM_TMUX=` (present but empty) is how a unit file or profile
    /// spells "no override", and the supervisor reads it that way too; the
    /// app must then probe rather than pass the empty value through, which
    /// would disable the discovery Finder launches depend on.
    #[test]
    fn an_empty_ambient_override_counts_as_unset_and_probes() {
        let resolved = resolve_supervisor_tmux(
            Some(std::ffi::OsString::new()),
            &["/opt/homebrew/bin"],
            |candidate| candidate == Path::new("/opt/homebrew/bin/tmux"),
        );
        assert_eq!(
            resolved,
            Some(std::ffi::OsString::from("/opt/homebrew/bin/tmux"))
        );
    }

    // ---- the tmux preflight (missing/below-floor startup refusal) ----
    //
    // `classify_tmux_preflight` takes an already-completed `probe_tmux`
    // result rather than a program to run, so these construct a canned
    // `TmuxProbe`/`TmuxProbeError` directly instead of spawning a real
    // tmux — the fixtures deliberately avoid depending on the host's own
    // tmux installation, or lack of one. `tmux_refusal_message` is pure on
    // top of that classification, so both platforms' wording get exercised
    // by plain `cargo test` here even though only one of them is ever seen
    // on a real Mac.

    /// Build a passing [`farhelm_supervisor::tmux::TmuxProbe`] for `stdout`,
    /// classifying it the same way `probe_tmux` itself would.
    fn probe_ok(program: &str, stdout: &str) -> farhelm_supervisor::tmux::TmuxProbe {
        let version = farhelm_supervisor::tmux::parse_tmux_version(stdout)
            .expect("fixture version must parse");
        farhelm_supervisor::tmux::TmuxProbe {
            program: PathBuf::from(program),
            version,
            support: farhelm_supervisor::tmux::classify_tmux_version(version),
        }
    }

    /// `TmuxProbeError::NotRunnable` carrying `ENOENT` — the shape
    /// `std::process::Command` produces when the program cannot be found on
    /// disk or on `PATH` — must classify as [`TmuxRefusal::NotFound`], not
    /// surface as a generic error.
    #[test]
    fn a_not_found_probe_classifies_as_not_found() {
        let probed = Err(farhelm_supervisor::tmux::TmuxProbeError::NotRunnable(
            std::io::Error::from(std::io::ErrorKind::NotFound),
        ));
        let refusal = classify_tmux_preflight(Path::new("/nonexistent/tmux"), probed)
            .expect("ENOENT is a classified refusal, not an error");
        assert_eq!(refusal, Some(TmuxRefusal::NotFound));
    }

    /// A `-V` answer below `TMUX_FLOOR` must classify as `BelowFloor`,
    /// carrying the exact version found so the message can name it.
    #[test]
    fn a_below_floor_version_classifies_with_the_found_version() {
        let refusal =
            classify_tmux_preflight(Path::new("tmux"), Ok(probe_ok("tmux", "tmux 3.4\n")))
                .expect("a well-formed below-floor version classifies cleanly");
        assert_eq!(
            refusal,
            Some(TmuxRefusal::BelowFloor {
                found: farhelm_supervisor::tmux::parse_tmux_version("tmux 3.4\n").unwrap()
            })
        );
    }

    /// At or above the floor must let startup proceed — `None`, not a
    /// refusal — which is what lets an above-pin tmux through silently here
    /// (its "unaudited" warning is the supervisor's job once it actually
    /// starts, not this preflight's).
    #[test]
    fn at_or_above_the_floor_is_not_a_refusal() {
        assert_eq!(
            classify_tmux_preflight(Path::new("tmux"), Ok(probe_ok("tmux", "tmux 3.7c\n")))
                .unwrap(),
            None
        );
        assert_eq!(
            classify_tmux_preflight(Path::new("tmux"), Ok(probe_ok("tmux", "tmux 3.9\n"))).unwrap(),
            None
        );
    }

    /// A spawn failure that is NOT `ENOENT` (a permission-denied `--tmux`,
    /// say) must reach the ordinary `anyhow::Error` path rather than being
    /// folded into [`TmuxRefusal::NotFound`] — a bare `EACCES` describes a
    /// wrong permission bit, not an absent tmux, and the two need different
    /// operator repairs. The rendered chain must still name the program so
    /// the failure is actionable.
    #[test]
    fn a_permission_denied_probe_is_an_ordinary_error_naming_the_program() {
        let probed = Err(farhelm_supervisor::tmux::TmuxProbeError::NotRunnable(
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        ));
        let error = classify_tmux_preflight(Path::new("/opt/locked/tmux"), probed)
            .expect_err("permission-denied is not a classified refusal");
        let message = format!("{error:#}");
        assert!(message.contains("/opt/locked/tmux"), "{message}");
    }

    /// A nonzero `-V` exit surfaces as [`TmuxProbeError::Unparseable`]
    /// carrying tmux's own stderr (see `probe_tmux`'s doc comment) — the
    /// rendered chain must keep both the program and that stderr text,
    /// since stderr is the only actionable detail a caller ever gets for
    /// "something answered to this name and refused".
    #[test]
    fn a_nonzero_probe_exit_names_the_program_and_keeps_its_stderr() {
        let probed = Err(farhelm_supervisor::tmux::TmuxProbeError::Unparseable(
            "libevent.so.2: cannot open shared object file".to_string(),
        ));
        let error = classify_tmux_preflight(Path::new("/opt/broken/tmux"), probed)
            .expect_err("a nonzero exit cannot be treated as a version");
        let message = format!("{error:#}");
        assert!(message.contains("/opt/broken/tmux"), "{message}");
        assert!(
            message.contains("libevent.so.2: cannot open shared object file"),
            "{message}"
        );
    }

    /// Successful but unparseable `-V` output is the SAME `Unparseable`
    /// variant as a nonzero exit (both come from `probe_tmux` unable to
    /// produce a usable version), so the rendered chain must still carry
    /// the malformed text even though nothing failed at the process level.
    #[test]
    fn malformed_successful_output_names_the_program_and_keeps_the_malformed_text() {
        let probed = Err(farhelm_supervisor::tmux::TmuxProbeError::Unparseable(
            "tmux 9.9zzz-vendor-mangled".to_string(),
        ));
        let error = classify_tmux_preflight(Path::new("/opt/weird/tmux"), probed)
            .expect_err("an unreadable version must refuse");
        let message = format!("{error:#}");
        assert!(message.contains("/opt/weird/tmux"), "{message}");
        assert!(message.contains("tmux 9.9zzz-vendor-mangled"), "{message}");
    }

    /// `TmuxProbeError::Overran` (the bounded probe's time/output-budget
    /// refusal — see `probe_tmux`'s own doc comment) must reach the
    /// ordinary `anyhow::Error` path, exactly like `Unparseable`, rather
    /// than being folded into `Ok(None)` or misread as the specialized
    /// `TmuxRefusal::NotFound`. Those two outcomes look similar only in
    /// that both let a caller move on, but they mean opposite things here:
    /// `Ok(None)` says "this tmux is fine, proceed", while an overrun
    /// candidate answered nothing conclusive at all. Confusing them would
    /// let the desktop hand its managed supervisor a candidate the
    /// preflight never actually cleared, so the supervisor's own
    /// unbounded `-V` call could then hang or flood output with no
    /// preflight left to have caught it first.
    #[test]
    fn an_overrun_probe_is_an_ordinary_error_naming_the_program_and_detail() {
        let probed = Err(farhelm_supervisor::tmux::TmuxProbeError::Overran(
            "it did not answer -V within 5 seconds".to_string(),
        ));
        let error = classify_tmux_preflight(Path::new("/opt/wedged/tmux"), probed)
            .expect_err("an overrun probe cannot be treated as a version");
        let message = format!("{error:#}");
        assert!(message.contains("/opt/wedged/tmux"), "{message}");
        assert!(
            message.contains("it did not answer -V within 5 seconds"),
            "{message}"
        );
    }

    /// The four refusal×platform combinations, pinned as COMPLETE strings
    /// rather than substring checks — a message that silently dropped the
    /// floor version, reworded the install command, added a clause, or let
    /// an `Error:`/backtrace-shaped line back in would be exactly the
    /// regression this preflight exists to prevent, and a looser assertion
    /// could miss any of those.
    #[test]
    fn refusal_messages_are_pinned_exactly_per_platform() {
        use farhelm_supervisor::tmux::TMUX_FLOOR;

        let probed = vec!["FARHELM_TMUX".to_string(), "PATH".to_string()];
        let below = TmuxRefusal::BelowFloor {
            found: farhelm_supervisor::tmux::parse_tmux_version("tmux 3.4\n").unwrap(),
        };
        let program = Path::new("/usr/bin/tmux");

        let not_found_subject = format!(
            "farhelm-desktop needs tmux {TMUX_FLOOR} or newer, and none could be run (looked \
             at: FARHELM_TMUX, then PATH). Each one was either not found, or is missing its \
             interpreter or loader."
        );
        let below_floor_subject = format!(
            "found tmux 3.4 at /usr/bin/tmux, which is below the {TMUX_FLOOR} farhelm needs."
        );
        let mac_body = "On macOS, tmux has to be installed by hand; Homebrew is the recommended \
             way:\n\n    brew install tmux\n\nThen start farhelm-desktop again.";
        let linux_body = format!(
            "Install tmux {TMUX_FLOOR} or newer with your package manager or Linuxbrew (`brew \
             install tmux`), or point FARHELM_TMUX at one.\nThen start farhelm-desktop again."
        );

        let cases = [
            (
                TmuxRefusal::NotFound,
                TmuxRefusalPlatform::Mac,
                format!("{not_found_subject}\n{mac_body}"),
            ),
            (
                TmuxRefusal::NotFound,
                TmuxRefusalPlatform::Linux,
                format!("{not_found_subject}\n{linux_body}"),
            ),
            (
                below,
                TmuxRefusalPlatform::Mac,
                format!("{below_floor_subject}\n{mac_body}"),
            ),
            (
                below,
                TmuxRefusalPlatform::Linux,
                format!("{below_floor_subject}\n{linux_body}"),
            ),
        ];
        for (refusal, platform, expected) in cases {
            let message = tmux_refusal_message(refusal, program, platform, &probed, false);
            assert_eq!(message, expected);
        }
    }

    /// An ambient `FARHELM_TMUX` in force changes the REMEDY, not the
    /// diagnosis: restarting with the same bad override reproduces the
    /// identical refusal without ever considering a freshly installed
    /// tmux, so both platforms' bodies must name `FARHELM_TMUX` as the
    /// thing to fix. Pinned exactly, and for both platforms, since this is
    /// the one clause that only appears when an override is in force.
    #[test]
    fn an_override_in_force_is_named_as_the_remedy_on_both_platforms() {
        use farhelm_supervisor::tmux::TMUX_FLOOR;

        let probed = vec!["/custom/tmux".to_string()];
        let mac = tmux_refusal_message(
            TmuxRefusal::NotFound,
            Path::new("/custom/tmux"),
            TmuxRefusalPlatform::Mac,
            &probed,
            true,
        );
        assert_eq!(
            mac,
            format!(
                "farhelm-desktop needs tmux {TMUX_FLOOR} or newer, and none could be run \
                 (looked at: /custom/tmux). Each one was either not found, or is missing its \
                 interpreter or loader.\nOn macOS, tmux has to be installed by hand; Homebrew is \
                 the recommended way:\n\n    brew install tmux\n\nFARHELM_TMUX is set and \
                 overrides that search, so update it to point at the new install (or unset it) \
                 before starting farhelm-desktop again."
            )
        );

        let linux = tmux_refusal_message(
            TmuxRefusal::NotFound,
            Path::new("/custom/tmux"),
            TmuxRefusalPlatform::Linux,
            &probed,
            true,
        );
        assert_eq!(
            linux,
            format!(
                "farhelm-desktop needs tmux {TMUX_FLOOR} or newer, and none could be run \
                 (looked at: /custom/tmux). Each one was either not found, or is missing its \
                 interpreter or loader.\nInstall tmux {TMUX_FLOOR} or newer with your package \
                 manager or Linuxbrew (`brew install tmux`). FARHELM_TMUX is set and overrides \
                 where farhelm-desktop looks, so update it to point at a supported tmux, or \
                 unset it, before starting farhelm-desktop again."
            )
        );
    }

    /// `NotFound`'s "looked at: ..." clause must actually name what
    /// `tmux_probe_targets` reports, in the SAME order — a message that
    /// invented its own list, or reordered the real one, would mislead
    /// exactly the operator trying to act on it.
    #[test]
    fn the_not_found_message_names_every_probed_location_in_order() {
        let probed = tmux_probe_targets(
            None,
            &["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"],
        );
        let message = tmux_refusal_message(
            TmuxRefusal::NotFound,
            Path::new("tmux"),
            TmuxRefusalPlatform::Mac,
            &probed,
            false,
        );
        assert!(message.contains(
            "looked at: FARHELM_TMUX, /opt/homebrew/bin, /usr/local/bin, MacPorts, then PATH"
        ));
    }

    /// An ambient `FARHELM_TMUX` that failed must be named ALONE — probing
    /// never ran (see `resolve_supervisor_tmux`'s own precedence), so a
    /// message claiming to have also checked Homebrew or PATH would be
    /// describing a search this run never performed.
    ///
    /// The fake prefix list is FIXED and non-empty rather than
    /// `macos_tmux_prefixes()`, which is an empty slice outside macOS: an
    /// implementation that wrongly appended the real prefixes after an
    /// explicit override would still pass against an empty list, because
    /// there would be nothing for it to wrongly append. A fixed, populated
    /// list exercises the precedence regardless of which host runs the
    /// test.
    #[test]
    fn an_ambient_override_is_probed_alone() {
        assert_eq!(
            tmux_probe_targets(
                Some(std::ffi::OsStr::new("/nonexistent/tmux")),
                &["/fake/homebrew/bin", "/fake/local/bin", "/opt/local/bin"]
            ),
            vec!["/nonexistent/tmux".to_string()]
        );
    }

    /// `FARHELM_TMUX=` (present but empty) must report the SAME probe
    /// targets as no override at all — `resolve_supervisor_tmux` already
    /// treats the two as equivalent (see
    /// `an_empty_ambient_override_counts_as_unset_and_probes`), and this
    /// pins that `tmux_probe_targets` cannot drift from that decision by
    /// treating an empty value as a real override with nothing to report.
    #[test]
    fn an_empty_override_reports_the_same_targets_as_no_override() {
        let prefixes = ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"];
        assert_eq!(
            tmux_probe_targets(Some(std::ffi::OsStr::new("")), &prefixes),
            tmux_probe_targets(None, &prefixes)
        );
    }

    /// A state file written by a build that still kept the list preference
    /// beside the credentials (`remembered_selection`, `list_sort`) must
    /// decode as the credentials alone, and a rewrite must drop the stale
    /// fields rather than carry them forward.
    ///
    /// The preference moved into the helm (SPEC.md, Session list); a
    /// relaunch after the upgrade reads exactly such a file, and refusing it
    /// would log the operator out of the desktop app for no reason. The
    /// dropped-on-rewrite half pins that the fields really are gone from
    /// the type and not merely tolerated, so nothing can quietly revive a
    /// per-client copy by reading them back.
    #[test]
    fn a_state_file_with_the_retired_preference_fields_decodes_as_credentials_only() {
        let root = tempfile::tempdir().unwrap();
        let state_path = root.path().join(APP_STATE_FILE);
        std::fs::write(
            &state_path,
            r#"{
                "native_device_secret":"native-old",
                "webview_device_secret":"webview-old",
                "webview_auth_generation":7,
                "remembered_selection":{"helm":"helm-a","id":"session-1"},
                "list_sort":"title"
            }"#,
        )
        .unwrap();

        // `update_state` reads, mutates, and returns the decoded state, so
        // one call both proves the credentials decoded and performs the
        // rewrite whose output the raw-JSON assertions below inspect.
        let state = update_state(&state_path, |state| {
            state.webview_auth_generation += 1;
        })
        .unwrap();
        assert_eq!(state.native_device_secret.as_deref(), Some("native-old"));
        assert_eq!(state.webview_device_secret.as_deref(), Some("webview-old"));
        assert_eq!(state.webview_auth_generation, 8);

        let rewritten: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
        assert_eq!(rewritten["webview_auth_generation"], 8);
        assert!(
            rewritten.get("remembered_selection").is_none() && rewritten.get("list_sort").is_none(),
            "the retired preference fields must not survive a rewrite: {rewritten}"
        );
    }

    /// A failed atomic replacement must leave the credential visible to REST
    /// unchanged, or later requests can no longer drive webview recovery.
    #[test]
    fn persistence_failure_never_publishes_the_replacement_native_secret() {
        let root = tempfile::tempdir().unwrap();
        let missing_parent = root.path().join("missing");
        let state_path = missing_parent.join(APP_STATE_FILE);
        let mut published = "old-secret".to_string();

        let result =
            persist_then_publish_native(&state_path, "replacement-secret".to_string(), |secret| {
                published = secret
            });

        assert!(result.is_err());
        assert_eq!(published, "old-secret");
    }

    /// One stalled response must consume the readiness deadline itself rather
    /// than parking desktop startup forever inside reqwest body handling.
    #[tokio::test]
    async fn supervisor_readiness_deadline_bounds_a_server_that_never_answers() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(5)).await;
        });
        let root = tempfile::tempdir().unwrap();
        let state_path = root.path().join(APP_STATE_FILE);
        let mut state = PersistedState {
            native_device_secret: Some("device-secret".to_string()),
            ..PersistedState::default()
        };
        let started = Instant::now();
        let mut supervisor = None;

        let error = await_local_supervisor_until(
            &format!("http://{addr}"),
            root.path(),
            &state_path,
            &mut state,
            &mut supervisor,
            tokio::time::Instant::now() + Duration::from_millis(100),
        )
        .await
        .unwrap_err();

        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(
            error
                .to_string()
                .contains("did not connect within 30 seconds")
        );
        server.abort();
    }

    /// The first credential exchange shares one absolute startup deadline
    /// across connection, headers, and body decoding. A loopback listener that
    /// accepts but never answers must therefore fail instead of freezing launch.
    #[tokio::test]
    async fn initial_native_credential_exchange_has_an_absolute_deadline() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(5)).await;
        });
        let started = Instant::now();

        let error = native_credential(
            &format!("http://{addr}"),
            "bootstrap-token",
            None,
            tokio::time::Instant::now() + Duration::from_millis(100),
        )
        .await
        .unwrap_err();

        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(
            error
                .to_string()
                .contains("desktop credential bootstrap did not complete")
        );
        server.abort();
    }

    /// Ambient proxy variables belong only to the child. The parent owns both
    /// listeners so it can prove the loopback request reached its destination
    /// and never opened a connection to the proxy.
    #[test]
    fn desktop_loopback_client_ignores_an_ambient_proxy() {
        if std::env::var_os(PROXY_CHILD_ENV).is_some() {
            let target = std::env::var(PROXY_TARGET_ENV).unwrap();
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let body = runtime.block_on(async {
                loopback_client()
                    .unwrap()
                    .get(target)
                    .timeout(Duration::from_secs(2))
                    .send()
                    .await
                    .unwrap()
                    .text()
                    .await
                    .unwrap()
            });
            assert_eq!(body, "direct");
            return;
        }

        // Both fake servers poll a non-blocking LISTENER so the deadline loop
        // can give up, but each accepted stream is put back into blocking
        // mode explicitly. Linux hands out blocking sockets from a
        // non-blocking listener; macOS (BSD semantics) makes the accepted
        // socket inherit O_NONBLOCK, so a read on it returns WouldBlock
        // straight away, the 200 goes out before the request has arrived,
        // and hyper refuses a response that precedes its request
        // (`UnexpectedMessage`). That is exactly how this test failed the
        // first Mac release gate while every Linux run stayed green — the
        // release workflow is the only place this test runs on macOS.
        let target = TcpListener::bind("127.0.0.1:0").unwrap();
        let target_addr = target.local_addr().unwrap();
        target.set_nonblocking(true).unwrap();
        let target_thread = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                match target.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(3)))
                            .unwrap();
                        // Read the whole request head before answering, so
                        // the response can never overtake the request even
                        // on a platform that delivers it in several reads.
                        let mut request = Vec::new();
                        let mut chunk = [0_u8; 1024];
                        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                            match stream.read(&mut chunk) {
                                Ok(0) => break,
                                Ok(n) => request.extend_from_slice(&chunk[..n]),
                                Err(error) => panic!("target read failed: {error}"),
                            }
                        }
                        stream
                            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\ndirect")
                            .unwrap();
                        return;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("target accept failed: {error}"),
                }
            }
            panic!("proxy child never reached the loopback target");
        });
        let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy_addr = proxy.local_addr().unwrap();
        proxy.set_nonblocking(true).unwrap();
        let proxy_reached = Arc::new(AtomicBool::new(false));
        let proxy_witness = Arc::clone(&proxy_reached);
        let proxy_thread = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                match proxy.accept() {
                    Ok((_stream, _)) => {
                        proxy_witness.store(true, Ordering::Release);
                        return;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("proxy accept failed: {error}"),
                }
            }
        });

        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "desktop::tests::desktop_loopback_client_ignores_an_ambient_proxy",
                "--nocapture",
            ])
            .env(PROXY_CHILD_ENV, "1")
            .env(PROXY_TARGET_ENV, format!("http://{target_addr}/"))
            .env("HTTP_PROXY", format!("http://{proxy_addr}"))
            .env("HTTPS_PROXY", format!("http://{proxy_addr}"))
            .env("ALL_PROXY", format!("http://{proxy_addr}"))
            .env("NO_PROXY", "")
            .output()
            .unwrap();

        target_thread.join().unwrap();
        proxy_thread.join().unwrap();
        assert!(
            output.status.success(),
            "proxy child failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!proxy_reached.load(Ordering::Acquire));
    }
}
