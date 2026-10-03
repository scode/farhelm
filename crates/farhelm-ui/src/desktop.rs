//! Desktop ownership of the embedded helm, local supervisor, and two client
//! credentials.
//!
//! The native app is not a remote-helm client. It owns one loopback helm for
//! its lifetime and discovers or starts the local supervisor against the same
//! state directory. Its two client stacks authenticate with credentials the
//! embedded helm mints in memory and hands over at startup
//! ([`farhelm_helm::EmbeddedReady`]), not through the token exchange a browser
//! uses: token rotation and the helm's cap on remembered browser credentials
//! never revoke them, so the window never has to sign in again (SPEC.md
//! "Signing in again"). They are not persisted; every launch gets a fresh
//! pair. Native reqwest and the webview deliberately hold different
//! credentials: they are separate clients with separate WebSocket behavior.
//!
//! This file holds the entry point, the bootstrap of the helm, supervisor,
//! and credentials, and the pre-window refusal path. The rest is split by
//! concern: `assets` serves the embedded UI to the window,
//! `window_state` saves and restores the window frame, `state` owns the
//! credential file and the atomic write both state files share,
//! `tmux_preflight` picks the supervisor's tmux and refuses an unusable
//! one, and `bundle` finds the pieces installed beside the app.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc, LazyLock, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

mod assets;
mod bundle;
mod state;
mod tmux_preflight;
mod window_state;

pub(crate) use assets::use_embedded_asset_handler;
use bundle::{bundled_farhelm, bundled_web_ui, desktop_state_dir};
use state::{APP_STATE_FILE, update_state};
use tmux_preflight::{
    is_executable_file, macos_tmux_prefixes, resolve_supervisor_tmux, run_tmux_preflight_or_exit,
};
use window_state::{WINDOW_STATE_FILE, WindowTracker};

const DEFAULT_DESKTOP_PORT: u16 = 7433;
const DESKTOP_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

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
/// failure uses [`refuse_and_exit`] rather than returning, because there
/// is no window yet in which to show an error and the refusal needs both its
/// stable stderr line and its launcher-visible macOS surface.
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

    // A panic here would print a Rust backtrace hint ahead of what is, for
    // every failure this can actually produce, an operator-facing startup
    // refusal rather than a programming bug. Someone staring at a GUI app
    // that just quit needs one plain sentence and, on macOS, a native
    // surface (`refuse_and_exit`). ONLY the two specialized
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
            refuse_and_exit(&format!("farhelm-desktop: {error:#}"));
        }
    };

    // Bootstrap has already resolved and created this directory. Reusing its
    // path keeps a failed second lookup from redirecting persistence into cwd.
    let window_state_path = desktop.state_dir.join(WINDOW_STATE_FILE);
    let window_tracker = Arc::new(Mutex::new(WindowTracker::load(&window_state_path)));
    let restore_tracker = Arc::clone(&window_tracker);
    let event_tracker = Arc::clone(&window_tracker);
    let state_path = window_state_path;
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
                .with_window(desktop_window())
                .with_on_window(move |window, _| {
                    restore_tracker
                        .lock()
                        .expect("window tracker poisoned")
                        .attach(window);
                })
                .with_custom_event_handler(move |event, _| {
                    event_tracker
                        .lock()
                        .expect("window tracker poisoned")
                        .observe(event, &state_path);
                })
                .with_disable_drag_drop_handler(true),
        )
        .launch(crate::App);
    Ok(())
}

/// Retain native window controls while letting macOS content reach the top edge.
///
/// The visible title is hidden only on macOS; the window still has a title
/// for system window menus. Native decorations stay enabled so AppKit owns
/// resizing, fullscreen, shadows, and the traffic-light buttons. Their inset
/// is paired with the macOS-only header reservation in app.css.
fn desktop_window() -> dioxus::desktop::WindowBuilder {
    let window = dioxus::desktop::WindowBuilder::new().with_title("farhelm");
    #[cfg(target_os = "macos")]
    {
        use dioxus::desktop::tao::platform::macos::WindowBuilderExtMacOS;

        window
            .with_titlebar_transparent(true)
            .with_title_hidden(true)
            .with_fullsize_content_view(true)
            .with_traffic_light_inset(dioxus::desktop::LogicalPosition::new(12.0, 16.0))
            .with_movable_by_window_background(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        window
    }
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

/// Values the webview needs to authenticate.
///
/// The device secret crosses the native/webview boundary through Dioxus IPC
/// after the document exists. It is never placed in a URL, page markup, or
/// command line, and the web token never crosses at all.
#[derive(Clone, PartialEq)]
pub struct WebviewBootstrap {
    pub(crate) base: String,
    /// The webview's own in-memory credential for this launch, minted by the
    /// embedded helm (see [`farhelm_helm::EmbeddedReady`]).
    pub(crate) device_secret: String,
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

/// Where [`record_webview_ready`] writes, set once startup has resolved the
/// state directory.
static RUNTIME_STATE_PATH: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// The app-owned processes and authentication state kept alive by `main`.
pub struct DesktopBootstrap {
    webview: WebviewBootstrap,
    /// The state root was resolved and made private during bootstrap.
    state_dir: PathBuf,
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
        let ambient_tmux = std::env::var_os(farhelm_supervisor::tmux::TMUX_PROGRAM_ENV);
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
                    command.env(farhelm_supervisor::tmux::TMUX_PROGRAM_ENV, tmux);
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
        let (ready_tx, ready_rx) = mpsc::channel::<farhelm_helm::EmbeddedReady>();
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
                            backstop_refresh_secs: None,
                        },
                        Some(native_clipboard_sink()),
                        ready_tx,
                        shutdown_rx,
                    ))
                })()
            })
            .context("starting embedded helm thread")?;
        let helm_deadline = std::time::Instant::now() + DESKTOP_STARTUP_TIMEOUT;
        let ready = loop {
            ensure_managed_supervisor_running(&mut supervisor)?;
            let remaining = helm_deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                bail!("embedded helm did not become ready within 30 seconds");
            }
            match ready_rx.recv_timeout(std::cmp::min(remaining, Duration::from_millis(100))) {
                Ok(ready) => break ready,
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
                    let message = match completion {
                        Ok(Err(error)) => format!("embedded helm stopped: {error:#}"),
                        Ok(Ok(())) => "embedded helm stopped unexpectedly".to_string(),
                        Err(_) => "embedded helm panicked after readiness".to_string(),
                    };
                    // Readiness comes well before the window (the rest of
                    // bootstrap still has to run), so this can fire while
                    // nothing is on screen yet; the common refusal path gives
                    // a launcher launch its native surface either way, and an
                    // alert after the window has vanished is still the right
                    // outcome for a helm that died under it.
                    refuse_and_exit(&message);
                }
            })
            .context("starting embedded helm monitor")?;
        // Every `?` below drops `shutdown_tx`, which stops the embedded helm
        // cleanly, and the monitor above would then refuse with "embedded
        // helm stopped unexpectedly", racing the real startup error through
        // the same refusal path (on macOS, two native alerts, or the wrong
        // one). Until startup succeeds and `Drop for DesktopBootstrap` takes
        // over, this guard marks that shutdown as expected. It is declared
        // after `shutdown_tx`, so it drops first and the flag is set before
        // the helm can observe the dropped sender.
        let mut startup_failure =
            ExpectHelmShutdownUnlessDisarmed(Some(Arc::clone(&expected_helm_shutdown)));
        let base = format!("http://{}", ready.addr);
        let native_secret = ready.native_device_secret;
        crate::auth::install_native_device_secret(native_secret.clone());
        runtime.block_on(await_local_supervisor(
            &base,
            &state_dir,
            &native_secret,
            &mut supervisor,
        ))?;

        // Rewrite the state file once, before any window exists. A file from
        // an older build still holds that build's device secrets, and this
        // drops them at launch rather than whenever the webview first
        // records readiness; a file that cannot be read or written fails
        // here, on the pre-window refusal path, instead of later inside the
        // window.
        let state_path = state_dir.join(APP_STATE_FILE);
        update_state(&state_path, |_| {})?;
        RUNTIME_STATE_PATH
            .set(state_path)
            .map_err(|_| anyhow::anyhow!("desktop authentication runtime was initialized twice"))?;

        let webview = WebviewBootstrap {
            base: base.clone(),
            device_secret: ready.webview_device_secret,
            smoke_client_log_marker: std::env::var("FARHELM_SMOKE_CLIENT_LOG_MARKER").ok(),
        };
        startup_failure.disarm();
        Ok(Self {
            webview,
            state_dir,
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

/// Marks the embedded helm's shutdown as expected when dropped, unless
/// disarmed: armed across the fallible tail of [`DesktopBootstrap::start`],
/// where an early return stops the helm on purpose and its monitor must not
/// report that as the helm dying.
struct ExpectHelmShutdownUnlessDisarmed(Option<Arc<AtomicBool>>);

impl ExpectHelmShutdownUnlessDisarmed {
    /// Startup succeeded; the returned bootstrap's own `Drop` owns the flag.
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for ExpectHelmShutdownUnlessDisarmed {
    fn drop(&mut self) {
        if let Some(expected) = self.0.take() {
            expected.store(true, Ordering::Release);
        }
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

/// Record that the webview's JavaScript stack completed an authenticated
/// event-socket handshake by advancing the durable readiness generation.
///
/// Only `scripts/desktop-smoke.sh` reads the generation, as its proof that a
/// launch's real JavaScript stack authenticated, so the caller treats a
/// failure here as worth a warning, not as a reason to keep the window shut.
/// The credential itself is not written: it is valid only for this helm
/// process, so persisting it would buy nothing and leave a secret on disk.
pub(crate) fn record_webview_ready() -> anyhow::Result<()> {
    let state_path = RUNTIME_STATE_PATH
        .get()
        .context("desktop authentication runtime is not initialized")?;
    update_state(state_path, |state| {
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
    if *SMOKE_HOOKS_ARMED {
        eprintln!("desktop_smoke: session listing requested query={query}");
    }
}

/// The companion to [`log_smoke_session_query`], emitted only once the helm
/// has ANSWERED that listing with success, so the smoke can prove the native
/// client was still authenticated at a given moment (after a token rotation,
/// in particular), which a request-time line cannot show. Same plain-stderr
/// format rule and the same contract with `scripts/desktop-smoke.sh`.
pub(crate) fn log_smoke_session_answered(query: &str) {
    if *SMOKE_HOOKS_ARMED {
        eprintln!("desktop_smoke: session listing answered query={query}");
    }
}

/// Whether the smoke hooks above print, decided once per process.
static SMOKE_HOOKS_ARMED: LazyLock<bool> =
    LazyLock::new(|| std::env::var_os("FARHELM_SMOKE_CLIENT_LOG_MARKER").is_some());

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

/// What startup messages call the local supervisor: "managed" only when this
/// app spawned it. When a supervisor was already answering in the state
/// directory the app spawned nothing, and calling that one managed sent the
/// user looking for a process the app never started.
fn local_supervisor_label(spawned: bool) -> &'static str {
    if spawned {
        "managed local supervisor"
    } else {
        "local supervisor"
    }
}

/// The startup deadline ran out before the local host connected.
fn not_connected_in_time(spawned: bool) -> anyhow::Error {
    anyhow::anyhow!(
        "{} did not connect within 30 seconds",
        local_supervisor_label(spawned)
    )
}

/// Why the local host can never connect as things stand, if the helm says
/// so: the supervisor answering in `state_dir` speaks another protocol
/// version, or reports an identity other than the one recorded, or none.
///
/// Each of those is a settled refusal, not a slow start, so waiting out the
/// startup deadline only delayed the same failure by 30 seconds and then
/// reported it as a timeout that named neither the cause nor the remedy.
/// `None` while the local host is connected, still connecting, or in any
/// other state.
///
/// The remedy differs by state. A version mismatch with a supervisor the
/// app did not spawn is the usual trigger: one the user started by hand,
/// still running across an upgrade (SPEC.md, Supported host setup: such a
/// supervisor may be refused, with a clear message), so the message tells
/// the user to stop it. The helm's own remediation ("update the farhelm
/// binary") is left out there, because the app's binary is already the
/// current one. An identity refusal is NOT fixed by stopping a hand-started
/// supervisor: the identity lives in the shared state directory, so the
/// app's own supervisor would report the same one on the next launch. The
/// message says so instead of sending the user round that loop.
///
/// On this path a mismatch has essentially one cause: the helm and the
/// supervisor share `state_dir`, so the identity the helm recorded and the
/// one the supervisor minted can only part ways when the two databases in
/// it stop being a pair — one restored from a backup, replaced, or deleted
/// (a deleted supervisor database mints a fresh identity) without the
/// other. The message names that cause and the files rather than the
/// abstract state. Adopting the reported identity is deliberately not
/// offered: it is in a window this startup path never opens, and the
/// maintainer chose a clear message over more machinery for a state that
/// takes manual file surgery to reach. The remedy says to stop every
/// supervisor for the directory first because one already running keeps the
/// database open and the identity it read at startup, so even after the
/// files are swapped it would keep reporting the old identity and the next
/// launch would be refused again. A supervisor that reports
/// no identity gets no remedy here: fixing it happens in a window this
/// startup path never opens.
fn local_supervisor_refusal(
    hosts: &[crate::Host],
    state_dir: &Path,
    spawned: bool,
) -> Option<String> {
    let local = hosts.iter().find(|host| host.kind.is_this_machine())?;
    let label = local_supervisor_label(spawned);
    let dir = state_dir.display();
    Some(match &local.state {
        crate::HostPhase::VersionSkew {
            peer_protocol,
            peer_build,
            our_protocol,
            our_build,
            remediation,
        } => {
            let mismatch = format!(
                "the {label} in {dir} speaks protocol {peer_protocol} (build {peer_build}), but \
                 this app speaks protocol {our_protocol} (build {our_build})"
            );
            if spawned {
                format!("{mismatch}: {remediation}")
            } else {
                format!(
                    "{mismatch}. Stop the supervisor you started for {dir} (for example a \
                     `farhelm supervisor run` still running in a terminal), then start Farhelm \
                     again."
                )
            }
        }
        crate::HostPhase::IdentityMismatch { recorded, reported } => format!(
            "the {label} in {dir} reports identity {reported}, but this machine's host entry \
             recorded {recorded}. The supervisor's database (supervisor.db) and this app's own \
             (helm.db) in {dir} no longer describe the same install, which happens when one of \
             them is restored from a backup or replaced without the other, or when \
             supervisor.db is deleted. Restarting this app alone will not change it. To fix \
             it, quit Farhelm, make sure no supervisor is still running for {dir}, and restore \
             both files from the same backup."
        ),
        crate::HostPhase::IdentityUnverified { recorded } => format!(
            "the {label} in {dir} reports no identity, but this machine's host entry recorded \
             {recorded}, so this app cannot tell whether it is the same supervisor."
        ),
        _ => return None,
    })
}

/// Wait for the reserved local row to reach the supervisor started above.
/// The manager intentionally starts actors without waiting for connections;
/// desktop startup is the consumer that needs the stronger readiness point.
async fn await_local_supervisor(
    base: &str,
    state_dir: &Path,
    secret: &str,
    supervisor: &mut Option<Child>,
) -> anyhow::Result<()> {
    await_local_supervisor_until(
        base,
        state_dir,
        secret,
        supervisor,
        tokio::time::Instant::now() + Duration::from_secs(30),
    )
    .await
}

/// Poll the managed row under one deadline, reporting every HTTP failure.
///
/// A 401 is one of those failures, not something to recover from: `secret`
/// is an in-memory credential the embedded helm minted for this process, so
/// the helm refusing it means something is broken, not that it expired.
async fn await_local_supervisor_until(
    base: &str,
    state_dir: &Path,
    secret: &str,
    supervisor: &mut Option<Child>,
    deadline: tokio::time::Instant,
) -> anyhow::Result<()> {
    let client = loopback_client()?;
    // Whether this app started the supervisor it is waiting for, which only
    // changes the wording of what startup reports.
    let spawned = supervisor.is_some();
    loop {
        ensure_managed_supervisor_running(supervisor)?;
        let response = tokio::time::timeout_at(
            deadline,
            client
                .get(format!("{base}/api/hosts"))
                .bearer_auth(secret)
                .send(),
        )
        .await
        .map_err(|_| not_connected_in_time(spawned))?
        .context("checking the local supervisor")?;
        let status = response.status();
        if status.is_success() {
            let hosts = tokio::time::timeout_at(deadline, crate::api::decode_hosts(response))
                .await
                .map_err(|_| not_connected_in_time(spawned))?
                .map_err(anyhow::Error::msg)?;
            if let Some(refusal) = local_supervisor_refusal(&hosts, state_dir, spawned) {
                bail!(refusal);
            }
            if hosts.iter().any(|host| {
                host.kind.is_this_machine()
                    && matches!(host.state, crate::HostPhase::Connected { .. })
            }) {
                ensure_managed_supervisor_running(supervisor)?;
                return Ok(());
            }
        } else {
            let body = tokio::time::timeout_at(deadline, response.text())
                .await
                .map_err(|_| not_connected_in_time(spawned))?
                .context("reading the local supervisor check's refusal")?;
            let detail = body.trim();
            bail!(
                "{} check failed with {status}{}",
                local_supervisor_label(spawned),
                if detail.is_empty() {
                    String::new()
                } else {
                    format!(": {detail}")
                }
            );
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(not_connected_in_time(spawned));
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

/// Report a fatal refusal — a startup failure before any window exists, or the
/// embedded helm dying out from under one — then terminate with the historical
/// status and stderr contract.
///
/// Finder and Spotlight launches have no terminal, so stderr alone makes a
/// failed GUI launch look like nothing happened. On macOS this adds the two
/// native facilities already provided by the operating system — `osascript`
/// for a critical alert and `logger` for Console — without adding a crate or
/// changing the app's startup dependencies. Both children are deliberately
/// fire-and-forget: the alert must remain visible after this process exits,
/// and neither helper's completion can affect the refusal. Their standard
/// streams are null so helper diagnostics can never alter this process's
/// stderr. The exact stderr output (several lines, for the tmux refusals)
/// remains the primary contract because `scripts/desktop-smoke.sh` compares
/// it byte-for-byte.
fn refuse_and_exit(message: &str) -> ! {
    eprintln!("{message}");

    if cfg!(target_os = "macos") {
        for argv in native_refusal_commands(message) {
            let mut command = Command::new(&argv[0]);
            command
                .args(&argv[1..])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let _ = command.spawn();
        }
    }

    std::process::exit(1)
}

/// Build the detached macOS helper invocations for a pre-window refusal.
///
/// The logger gets one single-line representation because Console's message
/// field is line-oriented. The alert keeps literal newlines, but escapes only
/// backslashes and double quotes so the refusal remains the text the operator
/// would have seen in a terminal. Keeping this pure lets every host test the
/// exact macOS argv without requiring either macOS helper to be installed.
///
/// Both helpers are named by absolute path: a GUI launch's `PATH` is not the
/// shell's, and an ambient lookup could either miss `/usr/bin` entirely or
/// resolve to whatever a writable directory ahead of it happens to hold.
fn native_refusal_commands(message: &str) -> Vec<Vec<String>> {
    let logger_message = message.replace('\n', " ");
    let alert_message = message.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        "display alert \"farhelm-desktop cannot start\" message \"{alert_message}\" as critical"
    );

    vec![
        vec![
            "/usr/bin/logger".to_string(),
            "-t".to_string(),
            "farhelm-desktop".to_string(),
            "--".to_string(),
            logger_message,
        ],
        vec!["/usr/bin/osascript".to_string(), "-e".to_string(), script],
    ]
}

/// The native system-clipboard writer registered with the embedded helm —
/// the receiving end of `POST /api/clipboard`.
///
/// This is what makes copying in the desktop app WORK at all: WKWebView
/// gives the `dioxus://` page no `navigator.clipboard` (not a secure
/// context), so the webview's copy paths POST the text to the embedded helm
/// and this closure performs the real write (farhelm-helm's clipboard.rs
/// carries the full diagnosis and endpoint contract).
///
/// One lazily created `arboard::Clipboard` behind a mutex rather than one
/// per write: on X11 — the Linux substrate the Xvfb smoke drives this code
/// on — the paste side of a selection is SERVED by the connection that set
/// it, so dropping the handle after each write could drop the offer with
/// it; macOS's NSPasteboard has no such lifetime, and the shared shape
/// costs it nothing. A handle that fails a write is discarded so the next
/// write reinitializes rather than failing forever, and a failed
/// INITIALIZATION (headless test environments have no display server) is
/// reported as the per-write `Err` the endpoint's best-effort contract
/// logs and swallows.
fn native_clipboard_sink() -> farhelm_helm::ClipboardSink {
    let held: std::sync::Mutex<Option<arboard::Clipboard>> = std::sync::Mutex::new(None);
    Arc::new(move |text: &str| {
        let mut guard = held
            .lock()
            .map_err(|_| "clipboard handle mutex poisoned".to_string())?;
        if guard.is_none() {
            *guard = Some(
                arboard::Clipboard::new().map_err(|error| format!("opening clipboard: {error}"))?,
            );
        }
        let clipboard = guard.as_mut().expect("initialized just above");
        match clipboard.set_text(text.to_string()) {
            Ok(()) => Ok(()),
            Err(error) => {
                *guard = None;
                Err(format!("writing clipboard: {error}"))
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;
    use std::net::TcpListener;
    use std::time::Instant;

    const PROXY_CHILD_ENV: &str = "FARHELM_DESKTOP_PROXY_TEST_CHILD";
    const PROXY_TARGET_ENV: &str = "FARHELM_DESKTOP_PROXY_TEST_TARGET";

    /// Constructing the clipboard sink must be side-effect free: the
    /// `arboard` handle is created lazily on the first WRITE, so building
    /// the closure at embedded-helm startup can never fail and never touches
    /// a clipboard. Deliberately construct-only — a test that actually wrote
    /// would clobber the developer's real clipboard on any machine with a
    /// display, and on a headless CI box would only ever prove that arboard
    /// errors without one; the real write is the manual Mac checklist's job.
    #[farhelm_testtrace::test]
    fn native_clipboard_sink_constructs_without_touching_a_clipboard() {
        let _sink = native_clipboard_sink();
    }

    /// The macOS refusal surface must preserve the multiline diagnostic in
    /// its alert while making the Console record one line, and must quote
    /// shell-looking text without changing either command's fixed argv.
    #[farhelm_testtrace::test]
    fn native_refusal_commands_escape_each_surface_exactly() {
        let message = "quote \"here\"\\path\nnext line";
        assert_eq!(
            native_refusal_commands(message),
            vec![
                vec![
                    "/usr/bin/logger".to_string(),
                    "-t".to_string(),
                    "farhelm-desktop".to_string(),
                    "--".to_string(),
                    "quote \"here\"\\path next line".to_string(),
                ],
                vec![
                    "/usr/bin/osascript".to_string(),
                    "-e".to_string(),
                    "display alert \"farhelm-desktop cannot start\" message \"quote \\\"here\\\"\\\\path\nnext line\" as critical".to_string(),
                ],
            ]
        );
    }

    /// An unanswered request must consume the readiness deadline rather than
    /// parking desktop startup indefinitely. The listener never accepts or
    /// replies; the paused clock checks the budget independently of scheduler
    /// load, without claiming which socket phase timed out.
    #[farhelm_testtrace::test(start_paused = true)]
    async fn supervisor_readiness_deadline_bounds_a_server_that_never_answers() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let root = tempfile::tempdir().unwrap();
        let started = tokio::time::Instant::now();
        let mut supervisor = None;

        let error = await_local_supervisor_until(
            &format!("http://{addr}"),
            root.path(),
            "device-secret",
            &mut supervisor,
            tokio::time::Instant::now() + Duration::from_millis(100),
        )
        .await
        .unwrap_err();

        assert!(started.elapsed() >= Duration::from_millis(100));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(
            error
                .to_string()
                .contains("did not connect within 30 seconds")
        );
        assert!(
            !error.to_string().contains("managed"),
            "the app spawned nothing here, so nothing it reports is managed: {error}"
        );
        drop(listener);
    }

    /// The startup wait stops at the first host list that shows the local
    /// supervisor refused, well before its deadline.
    ///
    /// Why it matters: the refusal check is what ends the wait early; without
    /// it, a refused supervisor waited out the whole 30 s. Specified: a fake
    /// helm that answers every host-list read with the local host in version
    /// skew ends the wait with the refusal message, long before a deadline set
    /// an hour out.
    #[farhelm_testtrace::test]
    async fn the_startup_wait_ends_on_a_refused_local_supervisor() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let body = serde_json::json!({ "hosts": [{
            "id": 1,
            "kind": "local",
            "destination": null,
            "name": "this machine",
            "identity": null,
            "remote_farhelm": null,
            "remote_state_dir": null,
            "state": {
                "phase": "version-skew",
                "peer_protocol": 9,
                "peer_build": "old-build",
                "our_protocol": 10,
                "our_build": "new-build",
                "remediation": "update the supervisor"
            }
        }]})
        .to_string();
        // A minimal helm: every request, whatever it asks, gets the host list.
        let helm = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let body = body.clone();
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut request = Vec::new();
                    let mut buf = [0u8; 1024];
                    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                        let n = socket.read(&mut buf).await.unwrap();
                        if n == 0 {
                            return;
                        }
                        request.extend_from_slice(&buf[..n]);
                    }
                    let reply = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: \
                         {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    socket.write_all(reply.as_bytes()).await.unwrap();
                });
            }
        });
        let root = tempfile::tempdir().unwrap();
        let mut supervisor = None;

        let error = tokio::time::timeout(
            Duration::from_secs(10),
            await_local_supervisor_until(
                &format!("http://{addr}"),
                root.path(),
                "device-secret",
                &mut supervisor,
                tokio::time::Instant::now() + Duration::from_secs(3600),
            ),
        )
        .await
        .expect("a refused local supervisor must end the wait without waiting out its deadline")
        .unwrap_err();
        assert!(
            error.to_string().contains("speaks protocol 9"),
            "the wait must end with the refusal, not a timeout: {error}"
        );
        helm.abort();
    }

    /// Startup ends at once, with the state and the remedy, when the helm
    /// reports that the local supervisor can never connect: another protocol
    /// version, another identity, or none.
    ///
    /// Why it matters: a supervisor the user started by hand and left running
    /// across an upgrade answers in the app's state directory, so the app
    /// spawns nothing and used to wait out its 30 s deadline, then fail with
    /// "managed local supervisor did not connect", naming neither the cause
    /// nor what to do, on every launch. Specified: each refusing state yields
    /// a message carrying its details; when the app spawned nothing a version
    /// skew's message is not "managed" and tells the user to stop the
    /// supervisor they started; an identity mismatch names the two databases
    /// that disagree and says to restore both from the same backup with no
    /// supervisor running; any other state of the local host, and a refusing
    /// state on another host, leave startup waiting.
    #[farhelm_testtrace::test]
    fn a_refusing_local_supervisor_ends_startup_with_its_state_and_remedy() {
        let dir = Path::new("/home/someone/.local/state/farhelm");
        let host = |kind, state| crate::Host {
            id: 1,
            kind,
            destination: None,
            alias: None,
            name: "this machine".to_string(),
            identity: None,
            remote_farhelm: None,
            remote_state_dir: None,
            state,
            incarnation: 1,
            yolo_safe: false,
        };
        let skew = crate::HostPhase::VersionSkew {
            peer_protocol: 9,
            peer_build: "old-build".to_string(),
            our_protocol: 10,
            our_build: "new-build".to_string(),
            remediation: "update the supervisor".to_string(),
        };

        let refusal =
            local_supervisor_refusal(&[host(crate::HostKind::Local, skew.clone())], dir, false)
                .expect("a version skew is a refusal");
        for expected in [
            "protocol 9",
            "old-build",
            "protocol 10",
            "new-build",
            "Stop the supervisor you started",
            "/home/someone/.local/state/farhelm",
        ] {
            assert!(
                refusal.contains(expected),
                "{expected:?} missing from {refusal:?}"
            );
        }
        assert!(!refusal.contains("managed"), "{refusal:?}");
        assert!(
            !refusal.contains("update the supervisor"),
            "the helm's update advice points at the wrong binary when the app spawned nothing: \
             {refusal:?}"
        );

        let mismatch = local_supervisor_refusal(
            &[host(
                crate::HostKind::Local,
                crate::HostPhase::IdentityMismatch {
                    recorded: "id-recorded".to_string(),
                    reported: "id-reported".to_string(),
                },
            )],
            dir,
            false,
        )
        .expect("an identity mismatch is a refusal");
        assert!(mismatch.contains("id-recorded") && mismatch.contains("id-reported"));
        for expected in [
            "supervisor.db",
            "helm.db",
            "no supervisor is still running",
            "restore both files from the same backup",
        ] {
            assert!(
                mismatch.contains(expected),
                "the mismatch must name its likely cause ({expected:?}): {mismatch:?}"
            );
        }
        assert!(
            !mismatch.contains("Stop the supervisor") && !mismatch.contains("managed"),
            "the mismatch remedy is restoring the pair, not the version-skew advice: {mismatch:?}"
        );
        let unverified = local_supervisor_refusal(
            &[host(
                crate::HostKind::Local,
                crate::HostPhase::IdentityUnverified {
                    recorded: "id-recorded".to_string(),
                },
            )],
            dir,
            false,
        )
        .expect("an unverifiable identity is a refusal");
        assert!(unverified.contains("no identity") && unverified.contains("id-recorded"));
        assert!(!unverified.contains("Stop the supervisor") && !unverified.contains("managed"));

        let own =
            local_supervisor_refusal(&[host(crate::HostKind::Local, skew.clone())], dir, true)
                .expect("a refusal of the app's own supervisor still ends startup");
        assert!(own.contains("managed local supervisor"), "{own:?}");
        assert!(!own.contains("Stop the supervisor you started"), "{own:?}");
        assert!(
            own.contains("update the supervisor"),
            "with the app's own supervisor refused, the helm's remediation is the remedy: {own:?}"
        );

        assert_eq!(
            local_supervisor_refusal(&[host(crate::HostKind::Ssh, skew)], dir, false),
            None,
            "another host's refusal is not the local supervisor's"
        );
        assert_eq!(
            local_supervisor_refusal(
                &[host(
                    crate::HostKind::Local,
                    crate::HostPhase::Connecting {
                        attempt: 1,
                        last_error: None
                    }
                )],
                dir,
                false
            ),
            None,
            "a local host still connecting keeps startup waiting"
        );
    }

    /// Own the server thread until its protocol result and actual exit have been observed.
    ///
    /// Dropping this fixture requests cancellation even when an assertion unwinds. The
    /// result distinguishes cancellation from either a served request or an unused proxy.
    struct ProxyTestServer {
        owner: farhelm_teststate::thread::FixtureThread,
        result: std::sync::mpsc::Receiver<Result<bool, &'static str>>,
        /// First read evidence lets cleanup tests keep a partially read request alive.
        request_started: std::sync::mpsc::Receiver<()>,
    }

    /// Check cancellation and the shared transaction deadline between nonblocking operations.
    ///
    /// Waiting is reserved for WouldBlock: successful reads and writes must also revisit
    /// the deadline, so a continuously active peer cannot extend the fixture lifetime.
    fn proxy_fixture_checkpoint(
        stop: &std::sync::mpsc::Receiver<()>,
        deadline: Instant,
        wait: bool,
    ) -> Result<(), &'static str> {
        use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
        match stop.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => return Err("fixture cancelled"),
            Err(TryRecvError::Empty) => {}
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("fixture deadline");
        }
        if wait {
            match stop.recv_timeout(remaining.min(Duration::from_millis(10))) {
                Ok(()) | Err(RecvTimeoutError::Disconnected) => return Err("fixture cancelled"),
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
        Ok(())
    }

    /// Serve one bounded request, or observe whether a supposed proxy receives a connection.
    ///
    /// Both roles retain their listener until completion. The proxy's ordinary path observes
    /// its whole three-second window; cancelling it as soon as the child exits could miss a
    /// connection already queued by that child. Cancellation is only the unwind fallback.
    fn spawn_proxy_test_server(listener: TcpListener, serve: bool) -> ProxyTestServer {
        listener.set_nonblocking(true).unwrap();
        let context = farhelm_testtrace::current_thread_context().expect("test trace context");
        let (stop_tx, stop_rx) = std::sync::mpsc::channel();
        let (result_tx, result) = std::sync::mpsc::channel();
        let (request_tx, request_started) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            context.enter(|| {
                let role = if serve { "target" } else { "proxy" };
                // Kind and errno preserve the underlying failure without retaining arbitrary
                // error strings. Record here: a child assertion can discard the result channel.
                let io_failure = |stage: &'static str, error: std::io::Error| {
                    tracing::error!(role, stage, kind = ?error.kind(), errno = ?error.raw_os_error(),
                        "desktop proxy fixture I/O failed");
                    stage
                };
                let deadline = Instant::now() + Duration::from_secs(3);
                let transaction = || -> Result<bool, &'static str> {
                    let mut stream = loop {
                        proxy_fixture_checkpoint(&stop_rx, deadline, false)?;
                        match listener.accept() {
                            Ok((stream, _)) => break stream,
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                proxy_fixture_checkpoint(&stop_rx, deadline, true)?;
                            }
                            Err(error) => return Err(io_failure("fixture accept failed", error)),
                        }
                    };
                    if !serve {
                        return Ok(true);
                    }
                    // Linux and BSD differ in whether accepted sockets inherit O_NONBLOCK.
                    // Set it explicitly and handle WouldBlock throughout, including writes.
                    stream
                        .set_nonblocking(true)
                        .map_err(|error| io_failure("fixture nonblocking failed", error))?;
                    let mut request = Vec::new();
                    let mut chunk = [0_u8; 1024];
                    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                        proxy_fixture_checkpoint(&stop_rx, deadline, false)?;
                        if request.len() == 16 * 1024 {
                            return Err("fixture request head too large");
                        }
                        let available = chunk.len().min(16 * 1024 - request.len());
                        match stream.read(&mut chunk[..available]) {
                            Ok(0) => return Err("fixture request ended before headers"),
                            Ok(n) => {
                                if request.is_empty() {
                                    let _ = request_tx.send(());
                                }
                                request.extend_from_slice(&chunk[..n]);
                            }
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                proxy_fixture_checkpoint(&stop_rx, deadline, true)?;
                            }
                            Err(error) => return Err(io_failure("fixture read failed", error)),
                        }
                    }
                    // A response before the complete request head can make hyper reject it
                    // as UnexpectedMessage. EOF and oversized headers never earn a 200.
                    let response =
                        b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\ndirect";
                    let mut written = 0;
                    while written < response.len() {
                        proxy_fixture_checkpoint(&stop_rx, deadline, false)?;
                        match stream.write(&response[written..]) {
                            Ok(0) => return Err("fixture write made no progress"),
                            Ok(n) => written += n,
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                proxy_fixture_checkpoint(&stop_rx, deadline, true)?;
                            }
                            Err(error) => return Err(io_failure("fixture write failed", error)),
                        }
                    }
                    Ok(true)
                };
                let outcome = match transaction() {
                    // The observation window ending without an accepted connection is
                    // the proxy's normal result, including expiry during an accept poll.
                    Err("fixture deadline") if !serve => Ok(false),
                    outcome => outcome,
                };
                // Release the listening socket inside the captured context, before publishing
                // the result. Actual thread completion is still observed by the owner.
                drop(listener);
                tracing::info!(role, outcome = ?outcome, "desktop proxy fixture finished");
                let _ = result_tx.send(outcome);
            });
        });
        let owner = farhelm_teststate::thread::FixtureThread::new(
            "desktop-proxy-fixture",
            worker,
            move || {
                let _ = stop_tx.send(());
            },
        )
        .expect("start fixture join observer");
        ProxyTestServer {
            owner,
            result,
            request_started,
        }
    }

    /// A child failure can discard the result receiver; the fixture error must survive in capture.
    #[farhelm_testtrace::test]
    fn proxy_fixture_records_an_error_without_a_result_receiver() {
        let capture = farhelm_testtrace::current_capture().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let ProxyTestServer { owner, result, .. } = spawn_proxy_test_server(listener, true);
        drop(result);
        let peer = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(1)).unwrap();
        peer.shutdown(std::net::Shutdown::Write).unwrap();
        owner.finish(Duration::from_secs(4)).unwrap();
        let events = capture.matching("desktop proxy fixture finished").unwrap();
        assert!(
            events.iter().any(|event| event
                .fields
                .get("role")
                .is_some_and(|role| role == "target")
                && event.fields.get("outcome").is_some_and(
                    |outcome| outcome.contains("fixture request ended before headers")
                )),
            "fixture failure missing from capture: {events:?}"
        );
    }

    /// Neither server role may retain its listener after an assertion unwinds before accept.
    #[farhelm_testtrace::test]
    fn proxy_fixture_cancels_before_accept_on_unwind() {
        for serve in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let ProxyTestServer { owner, result, .. } = spawn_proxy_test_server(listener, serve);
            let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _owner = owner;
                panic!("exercise pre-accept cleanup");
            }));
            assert!(unwind.is_err());
            assert_eq!(
                result.recv_timeout(Duration::from_secs(1)).unwrap(),
                Err("fixture cancelled")
            );
            assert!(std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(1)).is_err());
        }
    }

    /// A peer that sends only part of its headers must not trap unwind cleanup in a read.
    #[farhelm_testtrace::test]
    fn proxy_fixture_cancels_a_partial_request_on_unwind() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let ProxyTestServer {
            owner,
            result,
            request_started,
        } = spawn_proxy_test_server(listener, true);
        let mut peer = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(1)).unwrap();
        peer.set_write_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        peer.write_all(b"GET / HTTP/1.1\r\n").unwrap();
        request_started
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _owner = owner;
            panic!("exercise incomplete request cleanup");
        }));
        assert!(unwind.is_err());
        assert_eq!(
            result.recv_timeout(Duration::from_secs(1)).unwrap(),
            Err("fixture cancelled")
        );
        assert_eq!(peer.read(&mut [0; 1]).unwrap(), 0);
    }

    /// The startup guard marks the embedded helm's shutdown as expected when
    /// startup fails, and leaves it alone once startup succeeds.
    ///
    /// Why: a startup error after the helm is ready stops the helm on purpose,
    /// and without the flag the helm's monitor refused with "embedded helm
    /// stopped unexpectedly", racing the real error (on macOS, a second or
    /// wrong native alert). Spec: dropping the armed guard sets the flag;
    /// dropping a disarmed one does not, so the bootstrap's own `Drop` stays
    /// the only thing that expects a shutdown after success.
    #[farhelm_testtrace::test]
    fn the_startup_guard_expects_the_helm_shutdown_only_on_failure() {
        let expected = Arc::new(AtomicBool::new(false));
        drop(ExpectHelmShutdownUnlessDisarmed(Some(Arc::clone(
            &expected,
        ))));
        assert!(
            expected.load(Ordering::Acquire),
            "a failed startup expects the shutdown"
        );

        let expected = Arc::new(AtomicBool::new(false));
        let mut guard = ExpectHelmShutdownUnlessDisarmed(Some(Arc::clone(&expected)));
        guard.disarm();
        drop(guard);
        assert!(
            !expected.load(Ordering::Acquire),
            "a successful startup leaves it alone"
        );
    }

    /// EOF and an overlarge header cannot earn the success response used by the proxy test.
    #[farhelm_testtrace::test]
    fn proxy_fixture_rejects_incomplete_and_oversized_headers() {
        for (request, expected) in [
            (
                b"GET / HTTP/1.1\r\n".to_vec(),
                "fixture request ended before headers",
            ),
            (vec![b'x'; 16 * 1024], "fixture request head too large"),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let server = spawn_proxy_test_server(listener, true);
            let mut peer =
                std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(1)).unwrap();
            peer.set_write_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
            peer.write_all(&request).unwrap();
            peer.shutdown(std::net::Shutdown::Write).unwrap();
            assert_eq!(
                server.result.recv_timeout(Duration::from_secs(4)).unwrap(),
                Err(expected)
            );
            server.owner.finish(Duration::from_secs(1)).unwrap();
            assert_eq!(peer.read(&mut [0; 1]).unwrap(), 0);
        }
    }

    /// A completed request receives the full response, and a contacted proxy reports that contact.
    #[farhelm_testtrace::test]
    fn proxy_fixture_reports_served_requests_and_proxy_connections() {
        for serve in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let server = spawn_proxy_test_server(listener, serve);
            let mut peer =
                std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(1)).unwrap();
            if serve {
                peer.set_write_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
                peer.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
                    .unwrap();
                let mut response = String::new();
                peer.read_to_string(&mut response).unwrap();
                assert_eq!(
                    response,
                    "HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\ndirect"
                );
            }
            assert_eq!(
                server.result.recv_timeout(Duration::from_secs(4)).unwrap(),
                Ok(true)
            );
            server.owner.finish(Duration::from_secs(1)).unwrap();
        }
    }

    /// Ambient proxy variables belong only to the child. The parent owns both
    /// listeners so it can prove the loopback request reached its destination
    /// and never opened a connection to the proxy.
    #[farhelm_testtrace::test]
    fn desktop_loopback_client_ignores_an_ambient_proxy() {
        if std::env::var_os(PROXY_CHILD_ENV).is_some() {
            let target = std::env::var(PROXY_TARGET_ENV).unwrap();
            // The child is a fresh test-process invocation, so it needs this
            // test's capture to own the fixture runtime through the assertion.
            let context = farhelm_testtrace::current_thread_context()
                .expect("the test wrapper installs a thread context");
            context
                .with_runtime(
                    farhelm_testtrace::RuntimeConfig {
                        flavor: farhelm_testtrace::RuntimeFlavor::MultiThread,
                        worker_threads: None,
                        start_paused: false,
                    },
                    |runtime| {
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
                    },
                )
                .unwrap();
            return;
        }

        let target = TcpListener::bind("127.0.0.1:0").unwrap();
        let target_addr = target.local_addr().unwrap();
        let target_server = spawn_proxy_test_server(target, true);
        let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy_addr = proxy.local_addr().unwrap();
        let proxy_server = spawn_proxy_test_server(proxy, false);

        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
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
            .env("NO_PROXY", "");
        let limits = farhelm_teststate::process::CommandRunLimits::new(
            Duration::from_secs(10),
            Duration::from_secs(1),
            16 * 1024,
            16 * 1024,
            32 * 1024,
        )
        .unwrap();
        let output = farhelm_teststate::process::run_bounded(&mut child, &limits).unwrap();
        assert!(
            output.direct_child_reaped
                && !output.timed_out
                && output.status.is_some_and(|status| status.success())
                && output.errors.is_empty(),
            "proxy child failed: {output:?}"
        );
        assert!(
            String::from_utf8_lossy(&output.stdout.prefix).contains("1 passed"),
            "the selected child test must actually run: {output:?}"
        );
        assert_eq!(
            target_server
                .result
                .recv_timeout(Duration::from_secs(4))
                .unwrap(),
            Ok(true)
        );
        target_server.owner.finish(Duration::from_secs(1)).unwrap();
        assert_eq!(
            proxy_server
                .result
                .recv_timeout(Duration::from_secs(4))
                .unwrap(),
            Ok(false)
        );
        proxy_server.owner.finish(Duration::from_secs(1)).unwrap();
    }
}
