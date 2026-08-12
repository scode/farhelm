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
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

const APP_STATE_FILE: &str = "desktop-client.json";
const DEFAULT_DESKTOP_PORT: u16 = 7433;
const DESKTOP_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

/// Values the webview needs to validate or mint its own device session.
///
/// The bootstrap token crosses the native/webview boundary through Dioxus
/// IPC after the document exists. It is never placed in a URL, page markup,
/// or command line.
#[derive(Clone, PartialEq)]
pub struct WebviewBootstrap {
    pub(crate) base: String,
    pub(crate) persisted_secret: Option<String>,
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
        let executable_dir = farhelm
            .parent()
            .context("the bundled Farhelm CLI has no executable directory")?;
        let mut supervisor_paths = vec![executable_dir.to_path_buf()];
        if let Some(path) = std::env::var_os("PATH") {
            supervisor_paths.extend(std::env::split_paths(&path));
        }
        let supervisor_path = std::env::join_paths(supervisor_paths)
            .context("constructing the bundled supervisor PATH")?;
        let mut supervisor = match runtime.block_on(farhelm_helm::discover_local_supervisor(
            &farhelm, &state_dir,
        ))? {
            farhelm_helm::LocalSupervisorDiscovery::Answering => None,
            farhelm_helm::LocalSupervisorDiscovery::Absent => Some(
                Command::new(&farhelm)
                    .args(["supervisor", "run", "--exit-on-stdin-close", "--state-dir"])
                    .arg(&state_dir)
                    // Finder does not include Contents/MacOS on PATH. Putting
                    // the CLI's directory first makes its private tmux the
                    // authoritative substrate while retaining ordinary tools.
                    .env("PATH", supervisor_path)
                    // Retaining the write end tethers only the child this app
                    // owns, including GUI exits that skip Rust destructors.
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .with_context(|| {
                        format!("starting bundled supervisor at {}", farhelm.display())
                    })?,
            ),
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
                let result = (|| {
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
                })();
                result
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
    let mut state = read_state(state_path)?;
    state.webview_device_secret = Some(secret);
    state.webview_auth_generation = state.webview_auth_generation.saturating_add(1);
    write_state(state_path, &state)
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
    let mut state = read_state(state_path)?;
    state.native_device_secret = Some(secret.clone());
    write_state(state_path, &state)?;
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
