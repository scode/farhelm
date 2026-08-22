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
const PREFERENCE_DEBOUNCE: Duration = Duration::from_millis(150);
const PREFERENCE_EVAL_TIMEOUT: Duration = Duration::from_secs(5);

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
    /// Full launch snapshot, including `None` fields that clear stale storage.
    pub(crate) preferences: DesktopPreferences,
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

/// A launch snapshot or sparse runtime patch of desktop preference values.
///
/// In `WebviewBootstrap`, `None` means the durable snapshot has no value and
/// the page must clear a stale copy. In the change queue, omitted `None`
/// fields mean that update leaves the other preference untouched. Keeping one
/// wire shape is what lets the page store the browser's ordinary keys while
/// native merges only the choice the user changed.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub(crate) struct DesktopPreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) remembered_selection: Option<crate::list::RememberedSelection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) list_sort: Option<String>,
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
    remembered_selection: Option<crate::list::RememberedSelection>,
    list_sort: Option<String>,
}

#[derive(Default)]
struct PreferenceChangeQueue {
    pending: DesktopPreferences,
    in_flight: DesktopPreferences,
    worker_running: bool,
}

static STATE_FILE_WRITE: Mutex<()> = Mutex::new(());
static PREFERENCE_CHANGES: LazyLock<Mutex<PreferenceChangeQueue>> = LazyLock::new(Mutex::default);

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
        let state_dir = desktop_state_dir()?;
        let runtime =
            tokio::runtime::Runtime::new().context("starting desktop bootstrap runtime")?;
        runtime.block_on(farhelm_supervisor::ensure_private_dir(&state_dir))?;

        let farhelm = bundled_farhelm()?;
        let supervisor_tmux = resolve_supervisor_tmux(
            std::env::var_os("FARHELM_TMUX"),
            macos_tmux_prefixes(),
            is_executable_file,
        );
        let mut supervisor = match runtime.block_on(farhelm_helm::discover_local_supervisor(
            &farhelm, &state_dir,
        ))? {
            farhelm_helm::LocalSupervisorDiscovery::Answering => None,
            farhelm_helm::LocalSupervisorDiscovery::Absent => Some({
                let mut command = Command::new(&farhelm);
                command
                    .args(["supervisor", "run", "--exit-on-stdin-close", "--state-dir"])
                    .arg(&state_dir)
                    // The supervisor inherits this process's PATH unchanged.
                    // The bundle's own directory used to be prepended so a
                    // bundled tmux would win; there is no bundled tmux any
                    // more (TODO.md's 2026-08-22 floor decision) — the
                    // `FARHELM_TMUX` below is what names the substrate — and
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
                    format!("starting bundled supervisor at {}", farhelm.display())
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
        let ui_dist = bundled_web_ui()?;
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
            preferences: DesktopPreferences {
                remembered_selection: persisted.remembered_selection,
                list_sort: persisted.list_sort,
            },
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

/// Report one selection preference through the webview's page context.
///
/// The native component already knows the value, but the eval is the bridge
/// that keeps the page's browser-shaped localStorage record and the native
/// state file in one flow. The renderer mirror is updated by the caller
/// before this best-effort work begins, so neither an unavailable bridge nor
/// a failed disk write can undo the user's current selection.
pub(crate) fn report_selection_preference(selection: crate::list::RememberedSelection) {
    report_preference_change(DesktopPreferences {
        remembered_selection: Some(selection),
        list_sort: None,
    });
}

/// Report one chosen list order through the same page/native preference
/// channel as a selection.
pub(crate) fn report_sort_preference(sort: String) {
    report_preference_change(DesktopPreferences {
        remembered_selection: None,
        list_sort: Some(sort),
    });
}

/// Enqueue a page preference for one coalesced eval and state-file
/// replacement.
///
/// This is intentionally best-effort. Authentication already makes this
/// eval channel part of the desktop app's owned failure surface; preferences
/// add no new kind of bridge failure, and unlike authentication they do not
/// need a visible error. A failure is diagnostic-only and costs persistence
/// across the next launch, never the choice already applied in this one.
fn report_preference_change(update: DesktopPreferences) {
    let mut queue = PREFERENCE_CHANGES
        .lock()
        .expect("desktop preference queue lock poisoned");
    merge_preferences(&mut queue.pending, update);
    if queue.worker_running {
        return;
    }
    queue.worker_running = true;
    drop(queue);
    let state_path = RUNTIME_AUTH
        .get()
        .expect("desktop authentication runtime is initialized before rendering")
        .state_path
        .clone();
    dioxus::core::spawn_forever(flush_preference_changes_with(
        &PREFERENCE_CHANGES,
        PREFERENCE_DEBOUNCE,
        WebviewPreferenceReporter,
        FilePreferenceWriter { state_path },
    ));
}

trait PreferenceReporter {
    async fn report(&mut self, update: DesktopPreferences) -> bool;
}

trait PreferenceWriter {
    async fn persist(&mut self, update: DesktopPreferences) -> anyhow::Result<()>;
}

struct WebviewPreferenceReporter;

impl PreferenceReporter for WebviewPreferenceReporter {
    async fn report(&mut self, update: DesktopPreferences) -> bool {
        report_preference_batch(update).await
    }
}

struct FilePreferenceWriter {
    state_path: PathBuf,
}

impl PreferenceWriter for FilePreferenceWriter {
    async fn persist(&mut self, update: DesktopPreferences) -> anyhow::Result<()> {
        let state_path = self.state_path.clone();
        tokio::task::spawn_blocking(move || persist_preferences(&state_path, update))
            .await
            .context("joining desktop preference state-file writer")?
    }
}

/// Make cancellation leave the queue eligible to run again.
///
/// An interrupted eval or blocking-write await can otherwise strand
/// `worker_running = true` after the component that started it unmounts. The
/// in-flight sparse patch is put ahead of newer pending fields so the newer
/// user choices still win when a replacement worker starts.
struct PreferenceWorkerGuard {
    queue: &'static Mutex<PreferenceChangeQueue>,
    armed: bool,
}

impl PreferenceWorkerGuard {
    fn new(queue: &'static Mutex<PreferenceChangeQueue>) -> Self {
        Self { queue, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PreferenceWorkerGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let mut queue = self
            .queue
            .lock()
            .expect("desktop preference queue lock poisoned");
        let mut retry = std::mem::take(&mut queue.in_flight);
        merge_preferences(&mut retry, std::mem::take(&mut queue.pending));
        queue.pending = retry;
        queue.worker_running = false;
    }
}

/// Persist every accepted burst without tying the worker to a component.
///
/// One worker serializes the evals as well as the writes. That is not merely
/// an optimization: two independent round trips may answer out of order, and
/// letting the older click persist last would make the next launch disagree
/// with the selection this process ended on. Disk work is delegated to the
/// writer because the desktop runtime must remain free to render and service
/// IPC while the atomic replacement performs its fsyncs.
async fn flush_preference_changes_with<R, W>(
    queue: &'static Mutex<PreferenceChangeQueue>,
    debounce: Duration,
    mut reporter: R,
    mut writer: W,
) where
    R: PreferenceReporter,
    W: PreferenceWriter,
{
    let mut cancellation = PreferenceWorkerGuard::new(queue);
    loop {
        tokio::time::sleep(debounce).await;
        let update = {
            let mut queue = queue
                .lock()
                .expect("desktop preference queue lock poisoned");
            let update = std::mem::take(&mut queue.pending);
            queue.in_flight = update.clone();
            update
        };
        if reporter.report(update.clone()).await
            && let Err(error) = writer.persist(update).await
        {
            tracing::warn!(
                target: "desktop_preferences",
                error = %format_args!("{error:#}"),
                "could not persist desktop preferences"
            );
        }
        let mut queue = queue
            .lock()
            .expect("desktop preference queue lock poisoned");
        queue.in_flight = DesktopPreferences::default();
        if queue.pending == DesktopPreferences::default() {
            queue.worker_running = false;
            cancellation.disarm();
            return;
        }
    }
}

trait PreferenceEval {
    fn send(&mut self, value: serde_json::Value) -> Result<(), String>;
    async fn recv(&mut self) -> Result<DesktopPreferences, String>;
}

impl PreferenceEval for dioxus::document::Eval {
    fn send(&mut self, value: serde_json::Value) -> Result<(), String> {
        dioxus::document::Eval::send(self, value).map_err(|error| error.to_string())
    }

    async fn recv(&mut self) -> Result<DesktopPreferences, String> {
        dioxus::document::Eval::recv(self)
            .await
            .map_err(|error| error.to_string())
    }
}

/// Accept a preference batch only when the page acknowledges that exact patch.
///
/// The acknowledgement proves the browser-shaped copy was attempted, not a
/// replacement record native should trust. Success therefore authorizes the
/// caller to persist its original sparse patch; any send, receive, timeout,
/// or mismatch rejects the batch without touching the state file.
async fn report_preference_batch(update: DesktopPreferences) -> bool {
    let eval = dioxus::prelude::document::eval(
        "await window.__farhelmDesktopPreferences.report(\
         { recv: function () { return dioxus.recv(); }, \
           send: function (value) { dioxus.send(value); } }, \
         window.localStorage);",
    );
    report_preference_batch_with(eval, &update, PREFERENCE_EVAL_TIMEOUT).await
}

async fn report_preference_batch_with(
    mut eval: impl PreferenceEval,
    update: &DesktopPreferences,
    timeout: Duration,
) -> bool {
    if let Err(error) =
        eval.send(serde_json::to_value(update).expect("desktop preferences are serializable"))
    {
        tracing::warn!(
            target: "desktop_preferences",
            %error,
            "could not send a desktop preference to the webview"
        );
        return false;
    }
    let acknowledged = match tokio::time::timeout(timeout, eval.recv()).await {
        Ok(Ok(acknowledged)) => acknowledged,
        Ok(Err(error)) => {
            tracing::warn!(
                target: "desktop_preferences",
                %error,
                "desktop preference eval did not answer"
            );
            return false;
        }
        Err(_) => {
            tracing::warn!(
                target: "desktop_preferences",
                "desktop preference eval timed out"
            );
            return false;
        }
    };
    if acknowledged != *update {
        tracing::warn!(
            target: "desktop_preferences",
            "desktop preference eval returned a different record"
        );
        return false;
    }
    true
}

/// Flush the native preference mirrors at the real desktop close edge.
///
/// Tao's event loop does not return, so `DesktopBootstrap::drop` cannot cover
/// ordinary window closure. Wry delivers `CloseRequested` before Dioxus tears
/// down the webview and exits; synchronously merging both sparse native slots
/// there closes the debounce/eval loss window without asking the dying page.
fn flush_preferences_on_shutdown() {
    let Some(runtime) = RUNTIME_AUTH.get() else {
        return;
    };
    flush_preferences_on_shutdown_at(
        &runtime.state_path,
        &PREFERENCE_CHANGES,
        crate::list::desktop_preferences_snapshot(),
    );
}

fn flush_preferences_on_shutdown_at(
    state_path: &Path,
    changes: &Mutex<PreferenceChangeQueue>,
    native: DesktopPreferences,
) {
    let mut update = DesktopPreferences::default();
    let mut queue = changes
        .lock()
        .expect("desktop preference queue lock poisoned");
    merge_preferences(&mut update, std::mem::take(&mut queue.in_flight));
    merge_preferences(&mut update, std::mem::take(&mut queue.pending));
    merge_preferences(&mut update, native);
    queue.worker_running = false;
    drop(queue);
    if update == DesktopPreferences::default() {
        return;
    }
    if let Err(error) = persist_preferences(state_path, update) {
        tracing::warn!(
            target: "desktop_preferences",
            error = %format_args!("{error:#}"),
            "could not flush desktop preferences during shutdown"
        );
    }
}

/// Register the process-lifetime preference flush before the window can exit.
pub(crate) fn use_preference_close_flush() {
    use dioxus::desktop::tao::event::Event;
    use dioxus::desktop::{WindowEvent, use_wry_event_handler};

    use_wry_event_handler(|event, _| {
        if matches!(
            event,
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } | Event::LoopDestroyed
        ) {
            flush_preferences_on_shutdown();
        }
    });
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

/// Merge only fields the page changed, preserving credentials and the other
/// preference already in `desktop-client.json`.
fn persist_preferences(path: &Path, update: DesktopPreferences) -> anyhow::Result<()> {
    update_state(path, |state| {
        if let Some(selection) = update.remembered_selection {
            state.remembered_selection = Some(selection);
        }
        if let Some(sort) = update.list_sort {
            state.list_sort = Some(sort);
        }
    })
    .map(|_| ())
}

fn merge_preferences(current: &mut DesktopPreferences, update: DesktopPreferences) {
    if update.remembered_selection.is_some() {
        current.remembered_selection = update.remembered_selection;
    }
    if update.list_sort.is_some() {
        current.list_sort = update.list_sort;
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
            .context("monitoring the bundled supervisor during desktop startup")?
    {
        bail!("bundled supervisor exited during desktop startup with {status}");
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
/// Precedence, per TODO.md's 2026-08-22 tmux floor decision (part 3, "a
/// simple, documented 'pick your own' override ... honored by every launch
/// path, desktop app included"): an `ambient_override` already present in the
/// app's own environment is passed through UNTOUCHED and the probe is
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

fn desktop_state_dir() -> anyhow::Result<PathBuf> {
    match std::env::var_os("FARHELM_DESKTOP_STATE_DIR") {
        Some(path) => Ok(PathBuf::from(path)),
        None => farhelm_supervisor::default_state_dir(),
    }
}

fn bundled_farhelm() -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os("FARHELM_DESKTOP_FARHELM") {
        return Ok(PathBuf::from(path));
    }
    let current = std::env::current_exe().context("locating desktop executable")?;
    let sibling = current.with_file_name("farhelm");
    sibling
        .is_file()
        .then_some(sibling)
        .context("the desktop bundle does not contain its Farhelm CLI sibling")
}

fn bundled_web_ui() -> anyhow::Result<Option<PathBuf>> {
    if let Some(path) = std::env::var_os("FARHELM_DESKTOP_UI_DIST") {
        return Ok(Some(PathBuf::from(path)));
    }
    let current = std::env::current_exe().context("locating desktop executable")?;
    let resources = current
        .parent()
        .and_then(Path::parent)
        .map(|contents| contents.join("Resources/web"));
    match resources {
        Some(path) if path.join("index.html").is_file() => Ok(Some(path)),
        _ => bail!("the desktop bundle does not contain Resources/web/index.html"),
    }
}

fn read_state(path: &Path) -> anyhow::Result<PersistedState> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .with_context(|| format!("reading desktop state from {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PersistedState::default()),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

/// Serialize every read-modify-replace of the shared desktop state file.
///
/// Authentication refresh and the debounced preference worker can arrive
/// concurrently. Locking the whole merge is what prevents either writer
/// from reinstalling a snapshot that silently drops fields the other just
/// committed.
fn update_state(
    path: &Path,
    mutate: impl FnOnce(&mut PersistedState),
) -> anyhow::Result<PersistedState> {
    update_state_after_read(path, mutate, || {})
}

fn update_state_after_read(
    path: &Path,
    mutate: impl FnOnce(&mut PersistedState),
    after_read: impl FnOnce(),
) -> anyhow::Result<PersistedState> {
    let _guard = STATE_FILE_WRITE
        .lock()
        .expect("desktop state-file lock poisoned");
    let mut state = read_state(path)?;
    after_read();
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
    use std::collections::VecDeque;
    use std::future::pending;
    use std::io::Read as _;
    use std::net::TcpListener;
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    const PROXY_CHILD_ENV: &str = "FARHELM_DESKTOP_PROXY_TEST_CHILD";
    const PROXY_TARGET_ENV: &str = "FARHELM_DESKTOP_PROXY_TEST_TARGET";

    fn selection(helm: &str, id: &str) -> crate::list::RememberedSelection {
        crate::list::RememberedSelection {
            helm: helm.to_string(),
            id: id.to_string(),
        }
    }

    /// A `FARHELM_TMUX` already present in the app's own environment is the
    /// user's explicit override (TODO.md's floor decision, part 3) and must
    /// win outright — probing must not even run. Asserted here by making the
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

    enum EvalReply {
        Value(Result<DesktopPreferences, String>),
        Never,
    }

    struct FakeEval {
        send_error: Option<String>,
        sent: Arc<Mutex<Vec<serde_json::Value>>>,
        reply: EvalReply,
    }

    impl PreferenceEval for FakeEval {
        fn send(&mut self, value: serde_json::Value) -> Result<(), String> {
            if let Some(error) = self.send_error.take() {
                return Err(error);
            }
            self.sent.lock().unwrap().push(value);
            Ok(())
        }

        async fn recv(&mut self) -> Result<DesktopPreferences, String> {
            match std::mem::replace(&mut self.reply, EvalReply::Never) {
                EvalReply::Value(result) => result,
                EvalReply::Never => pending().await,
            }
        }
    }

    struct ImmediateReporter {
        results: VecDeque<bool>,
    }

    impl PreferenceReporter for ImmediateReporter {
        async fn report(&mut self, _update: DesktopPreferences) -> bool {
            self.results.pop_front().unwrap_or(true)
        }
    }

    struct BlockingReporter {
        arrived: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
        result: bool,
        first: bool,
    }

    impl PreferenceReporter for BlockingReporter {
        async fn report(&mut self, _update: DesktopPreferences) -> bool {
            if self.first {
                self.first = false;
                self.arrived.notify_one();
                self.release.notified().await;
                return self.result;
            }
            true
        }
    }

    struct MemoryWriter {
        attempts: Arc<Mutex<Vec<DesktopPreferences>>>,
        persisted: Arc<Mutex<Vec<DesktopPreferences>>>,
        fail_first: bool,
    }

    impl PreferenceWriter for MemoryWriter {
        async fn persist(&mut self, update: DesktopPreferences) -> anyhow::Result<()> {
            self.attempts.lock().unwrap().push(update.clone());
            if std::mem::take(&mut self.fail_first) {
                bail!("injected disk failure");
            }
            self.persisted.lock().unwrap().push(update);
            Ok(())
        }
    }

    /// State files from desktop builds predating preference persistence stay
    /// valid and mean exactly "no remembered choice" for both new fields.
    #[test]
    fn persisted_state_decodes_without_preference_fields() {
        let state: PersistedState = serde_json::from_str(
            r#"{
                "native_device_secret":"native-old",
                "webview_device_secret":"webview-old",
                "webview_auth_generation":7
            }"#,
        )
        .unwrap();

        assert_eq!(state.native_device_secret.as_deref(), Some("native-old"));
        assert_eq!(state.webview_device_secret.as_deref(), Some("webview-old"));
        assert_eq!(state.webview_auth_generation, 7);
        assert_eq!(state.remembered_selection, None);
        assert_eq!(state.list_sort, None);
    }

    /// The durable selection keeps the browser record's `{helm, id}` shape,
    /// while sort remains the helm's bare wire word.
    #[test]
    fn persisted_state_decodes_preference_fields() {
        let state: PersistedState = serde_json::from_str(
            r#"{
                "native_device_secret":"native-new",
                "webview_device_secret":"webview-new",
                "webview_auth_generation":8,
                "remembered_selection":{"helm":"helm-a","id":"session-1"},
                "list_sort":"title"
            }"#,
        )
        .unwrap();

        assert_eq!(
            state.remembered_selection,
            Some(crate::list::RememberedSelection {
                helm: "helm-a".to_string(),
                id: "session-1".to_string(),
            })
        );
        assert_eq!(state.list_sort.as_deref(), Some("title"));
    }

    /// Preference write-back is a field merge, never a reconstructed
    /// preferences-only file that discards either authenticated client.
    #[test]
    fn preference_write_back_preserves_authentication_material() {
        let root = tempfile::tempdir().unwrap();
        let state_path = root.path().join(APP_STATE_FILE);
        write_state(
            &state_path,
            &PersistedState {
                native_device_secret: Some("native-secret".to_string()),
                webview_device_secret: Some("webview-secret".to_string()),
                webview_auth_generation: 11,
                remembered_selection: None,
                list_sort: Some("created".to_string()),
            },
        )
        .unwrap();

        persist_preferences(
            &state_path,
            DesktopPreferences {
                remembered_selection: Some(crate::list::RememberedSelection {
                    helm: "helm-a".to_string(),
                    id: "session-picked".to_string(),
                }),
                list_sort: None,
            },
        )
        .unwrap();

        let state = read_state(&state_path).unwrap();
        assert_eq!(state.native_device_secret.as_deref(), Some("native-secret"));
        assert_eq!(
            state.webview_device_secret.as_deref(),
            Some("webview-secret")
        );
        assert_eq!(state.webview_auth_generation, 11);
        assert_eq!(state.list_sort.as_deref(), Some("created"));
        assert_eq!(
            state.remembered_selection,
            Some(selection("helm-a", "session-picked"))
        );
    }

    /// Closing at either side of the eval handoff commits both native sparse
    /// mirrors as one merge and preserves every unrelated state-file field.
    #[test]
    fn close_flush_persists_pending_and_in_flight_native_values() {
        let root = tempfile::tempdir().unwrap();
        let state_path = root.path().join(APP_STATE_FILE);
        write_state(
            &state_path,
            &PersistedState {
                native_device_secret: Some("native-secret".to_string()),
                webview_device_secret: Some("webview-secret".to_string()),
                webview_auth_generation: 13,
                remembered_selection: None,
                list_sort: None,
            },
        )
        .unwrap();
        let changes = Mutex::new(PreferenceChangeQueue {
            pending: DesktopPreferences {
                remembered_selection: None,
                list_sort: Some("created".to_string()),
            },
            in_flight: DesktopPreferences {
                remembered_selection: Some(selection("helm-a", "session-old")),
                list_sort: None,
            },
            worker_running: true,
        });

        flush_preferences_on_shutdown_at(
            &state_path,
            &changes,
            DesktopPreferences {
                remembered_selection: Some(selection("helm-a", "session-picked")),
                list_sort: Some("title".to_string()),
            },
        );

        let state = read_state(&state_path).unwrap();
        assert_eq!(state.native_device_secret.as_deref(), Some("native-secret"));
        assert_eq!(
            state.webview_device_secret.as_deref(),
            Some("webview-secret")
        );
        assert_eq!(state.webview_auth_generation, 13);
        assert_eq!(
            state.remembered_selection,
            Some(selection("helm-a", "session-picked"))
        );
        assert_eq!(state.list_sort.as_deref(), Some("title"));
        let changes = changes.lock().unwrap();
        assert_eq!(changes.pending, DesktopPreferences::default());
        assert_eq!(changes.in_flight, DesktopPreferences::default());
        assert!(!changes.worker_running);
    }

    /// The eval protocol authorizes persistence only for an exact, timely
    /// acknowledgement of the caller's original sparse patch.
    #[tokio::test]
    async fn preference_report_rejects_every_ipc_failure_shape() {
        let update = DesktopPreferences {
            remembered_selection: Some(selection("helm-a", "session-picked")),
            list_sort: None,
        };
        let sent = Arc::new(Mutex::new(Vec::new()));
        let eval = |send_error, reply| FakeEval {
            send_error,
            sent: Arc::clone(&sent),
            reply,
        };

        assert!(
            !report_preference_batch_with(
                eval(Some("send failed".to_string()), EvalReply::Never),
                &update,
                Duration::from_millis(10),
            )
            .await
        );
        assert!(
            !report_preference_batch_with(
                eval(None, EvalReply::Value(Err("receive failed".to_string()))),
                &update,
                Duration::from_millis(10),
            )
            .await
        );
        assert!(
            !report_preference_batch_with(
                eval(None, EvalReply::Never),
                &update,
                Duration::from_millis(10),
            )
            .await
        );
        assert!(
            !report_preference_batch_with(
                eval(
                    None,
                    EvalReply::Value(Ok(DesktopPreferences {
                        remembered_selection: None,
                        list_sort: Some("title".to_string()),
                    }))
                ),
                &update,
                Duration::from_millis(10),
            )
            .await
        );
        assert!(
            report_preference_batch_with(
                eval(None, EvalReply::Value(Ok(update.clone()))),
                &update,
                Duration::from_millis(10),
            )
            .await
        );
    }

    /// A cancelled root worker requeues its in-flight patch and clears the
    /// running bit, so a later worker can durably complete the same choice.
    #[tokio::test]
    async fn cancelled_preference_worker_does_not_wedge_later_updates() {
        let queue: &'static Mutex<PreferenceChangeQueue> =
            Box::leak(Box::new(Mutex::new(PreferenceChangeQueue {
                pending: DesktopPreferences {
                    remembered_selection: Some(selection("helm-a", "session-picked")),
                    list_sort: None,
                },
                in_flight: DesktopPreferences::default(),
                worker_running: true,
            })));
        let arrived = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let persisted = Arc::new(Mutex::new(Vec::new()));
        let worker = tokio::spawn(flush_preference_changes_with(
            queue,
            Duration::ZERO,
            BlockingReporter {
                arrived: Arc::clone(&arrived),
                release,
                result: true,
                first: true,
            },
            MemoryWriter {
                attempts: Arc::clone(&attempts),
                persisted: Arc::clone(&persisted),
                fail_first: false,
            },
        ));
        arrived.notified().await;
        worker.abort();
        assert!(worker.await.unwrap_err().is_cancelled());
        {
            let mut queue = queue.lock().unwrap();
            assert!(!queue.worker_running);
            assert_eq!(
                queue.pending.remembered_selection,
                Some(selection("helm-a", "session-picked"))
            );
            queue.worker_running = true;
        }

        flush_preference_changes_with(
            queue,
            Duration::ZERO,
            ImmediateReporter {
                results: VecDeque::from([true]),
            },
            MemoryWriter {
                attempts,
                persisted: Arc::clone(&persisted),
                fail_first: false,
            },
        )
        .await;

        assert_eq!(
            persisted.lock().unwrap().as_slice(),
            &[DesktopPreferences {
                remembered_selection: Some(selection("helm-a", "session-picked")),
                list_sort: None,
            }]
        );
    }

    /// Rejected and failed writes do not terminate serialization; a newer
    /// pending batch is still reported and can complete normally.
    #[tokio::test]
    async fn preference_worker_recovers_after_rejection_and_disk_failure() {
        async fn run(first_report: bool, fail_first_write: bool) -> Vec<DesktopPreferences> {
            let queue: &'static Mutex<PreferenceChangeQueue> =
                Box::leak(Box::new(Mutex::new(PreferenceChangeQueue {
                    pending: DesktopPreferences {
                        remembered_selection: Some(selection("helm-a", "session-old")),
                        list_sort: None,
                    },
                    in_flight: DesktopPreferences::default(),
                    worker_running: true,
                })));
            let arrived = Arc::new(tokio::sync::Notify::new());
            let release = Arc::new(tokio::sync::Notify::new());
            let persisted = Arc::new(Mutex::new(Vec::new()));
            let worker = tokio::spawn(flush_preference_changes_with(
                queue,
                Duration::ZERO,
                BlockingReporter {
                    arrived: Arc::clone(&arrived),
                    release: Arc::clone(&release),
                    result: first_report,
                    first: true,
                },
                MemoryWriter {
                    attempts: Arc::new(Mutex::new(Vec::new())),
                    persisted: Arc::clone(&persisted),
                    fail_first: fail_first_write,
                },
            ));
            arrived.notified().await;
            {
                let mut queue = queue.lock().unwrap();
                queue.pending.list_sort = Some("title".to_string());
            }
            release.notify_one();
            worker.await.unwrap();
            Arc::try_unwrap(persisted).unwrap().into_inner().unwrap()
        }

        assert_eq!(
            run(false, false).await,
            vec![DesktopPreferences {
                remembered_selection: None,
                list_sort: Some("title".to_string()),
            }]
        );
        assert_eq!(
            run(true, true).await,
            vec![DesktopPreferences {
                remembered_selection: None,
                list_sort: Some("title".to_string()),
            }]
        );
    }

    /// A preference merge paused after its read forces the auth writer to
    /// queue behind the same mutex; both complete without reinstalling stale
    /// snapshots over the other's fields.
    #[test]
    fn concurrent_preference_and_auth_writes_preserve_the_whole_record() {
        let root = tempfile::tempdir().unwrap();
        let state_path = root.path().join(APP_STATE_FILE);
        write_state(
            &state_path,
            &PersistedState {
                native_device_secret: Some("native-old".to_string()),
                webview_device_secret: Some("webview-old".to_string()),
                webview_auth_generation: 4,
                remembered_selection: None,
                list_sort: Some("activity".to_string()),
            },
        )
        .unwrap();
        let read = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let preference_path = state_path.clone();
        let preference_read = Arc::clone(&read);
        let preference_release = Arc::clone(&release);
        let preference = std::thread::spawn(move || {
            update_state_after_read(
                &preference_path,
                |state| {
                    state.remembered_selection = Some(selection("helm-a", "session-picked"));
                    state.list_sort = Some("title".to_string());
                },
                || {
                    preference_read.wait();
                    preference_release.wait();
                },
            )
            .unwrap();
        });
        read.wait();
        let auth_path = state_path.clone();
        let authentication = std::thread::spawn(move || {
            update_state(&auth_path, |state| {
                state.native_device_secret = Some("native-new".to_string());
                state.webview_device_secret = Some("webview-new".to_string());
                state.webview_auth_generation += 1;
            })
            .unwrap();
        });
        release.wait();
        preference.join().unwrap();
        authentication.join().unwrap();

        let state = read_state(&state_path).unwrap();
        assert_eq!(state.native_device_secret.as_deref(), Some("native-new"));
        assert_eq!(state.webview_device_secret.as_deref(), Some("webview-new"));
        assert_eq!(state.webview_auth_generation, 5);
        assert_eq!(
            state.remembered_selection,
            Some(selection("helm-a", "session-picked"))
        );
        assert_eq!(state.list_sort.as_deref(), Some("title"));
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

        let target = TcpListener::bind("127.0.0.1:0").unwrap();
        let target_addr = target.local_addr().unwrap();
        target.set_nonblocking(true).unwrap();
        let target_thread = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                match target.accept() {
                    Ok((mut stream, _)) => {
                        let mut request = [0_u8; 1024];
                        let _ = stream.read(&mut request);
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
