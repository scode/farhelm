//! The desktop app's process: it owns one embedded helm and the local supervisor, and holds the credential the GUI
//! uses to reach that helm.
//!
//! The desktop app is not a remote-helm client. It owns one loopback helm for its lifetime and discovers or starts the
//! local supervisor against the same state directory, so the machine it runs on is itself a host. The helm serves this
//! app's GUI only. Its credential is minted in memory by the embedded helm and handed over at startup
//! ([`farhelm_helm::EmbeddedReady`]), never written to disk, and worthless once the process exits, so the window never
//! has to sign in (SPEC.md, "Security", "Client to helm").
//!
//! This crate is the only place the GUI and the helm meet (SPEC_impl.md, "GUI architecture"): GUI crates never depend
//! on the helm, and reach it through its API with the credential [`DesktopBootstrap`] holds. The bootstrap, the
//! startup refusal surface and the updater moved here unchanged from the old Dioxus UI crate when that UI was deleted
//! (GPUI migration, F1). [`run`] starts them and then hands the main thread to the GPUI app (`farhelm-gui`).
//!
//! This file holds the bootstrap of the helm and supervisor and the pre-window refusal path. The rest is split by
//! concern: `window_state` keeps the saved window frame's format and restore rules, `state` owns the app's private state
//! file and the atomic write both state files share, `bundle` finds the pieces installed beside the app, `updater`
//! keeps an installed app up to date, and `update_readout` turns the updater's state into the words a window shows.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

use farhelm_supervisor::managed_supervisor::{
    ManagedSupervisor, ORDERLY_STOP_GRACE, Observation, ProgramFile, StopOutcome,
};
use farhelm_supervisor::service::STATE_DIR_CLAIM_WAIT;

#[cfg(feature = "agent")]
mod agent;
pub mod bundle;
mod local_host;
pub mod state;
mod tab_reach;
pub mod update_readout;
pub mod updater;
pub mod window_state;

use bundle::{bundled_farhelm, desktop_state_dir, helm_payload_dir};
use state::{APP_STATE_FILE, update_state};
pub use updater::{UpdaterHandle, UpdaterState, readout};

const DESKTOP_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

/// Dispatch a file the embedded helm has just published to its default app.
/// This is separate from hardened webbrowser: local paths are admitted only
/// by the download handler's extension policy, never by the HTTP(S) opener.
/// Errors become the reason after `not opened:` in the save notice, so their
/// authored text stays a lowercase clause rather than a standalone label.
#[cfg(target_os = "macos")]
fn open_saved_file(path: &Path) -> Result<(), String> {
    let status = Command::new("/usr/bin/open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("could not ask the default app to open the file: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("the default-app opener exited with {status}"))
    }
}

/// Hand the published file to the desktop's default application without
/// waiting for that application to exit. `xdg-open` may run a foreground
/// application synchronously; waiting for it in the helm's blocking pool
/// would leave the save response pending and could also block app shutdown.
/// GIO reports dispatch failure instead of the launched application's exit.
/// Its error is the lowercase continuation after `not opened:` in the save
/// notice, matching the other refusal reasons supplied by the helm.
#[cfg(target_os = "linux")]
fn open_saved_file(path: &Path) -> Result<(), String> {
    use gio::prelude::FileExt;

    let file = gio::File::for_path(path);
    gio::AppInfo::launch_default_for_uri(file.uri().as_str(), gio::AppLaunchContext::NONE)
        .map_err(|error| format!("could not ask the default app to open the file: {error}"))
}

/// The license notices of every third-party crate compiled into Farhelm's shipped binaries, generated from `Cargo.lock`
/// by `scripts/dependency-notices.py` and kept current by its `--check` in the ordinary checks (decision D-18: every
/// shipped binary carries them). `farhelm` embeds the same file; one file covers both binaries, so each over-covers
/// rather than under-covers.
const DEPENDENCY_NOTICES: &str = include_str!("../../farhelm/notices/dependencies.txt");

/// The Unicode License V3 notice for the Unicode-derived property tables in the vendored terminal emulator
/// (`vendor/alacritty_terminal`, see `THIRD_PARTY_NOTICES.md`). This binary links that emulator through the supervisor
/// it embeds, just as `farhelm` does, so it carries the same notice `farhelm licenses` prints. The vendored tree changes
/// only through its patch files, so the notice is embedded by its users rather than beside the tables.
const UNICODE_NOTICE: &str = include_str!("../../../vendor/UNICODE-LICENSE.txt");

/// The SIL Open Font License text of the Inter font files the GUI crate embeds in this binary (`farhelm-gui`'s
/// `assets`), which the license requires to travel with the fonts.
const INTER_FONT_NOTICE: &str = include_str!("../../gui/farhelm-gui/assets/fonts/OFL-Inter.txt");

/// What `farhelm-desktop --licenses` prints: every notice that must travel with this binary. That is the notices of the
/// code Farhelm ported from other projects that this binary links (the xterm.js-derived input encoding and the
/// emulator's Unicode tables, both reached through the embedded supervisor), its crates.io and vendored dependencies'
/// notices, and the license of the fonts it embeds. Code ported later and linked into this binary (the GUI's terminal
/// ports, for instance) adds its notice here in the change that first links it (SPEC_impl.md, "Dependencies and
/// licenses").
///
/// `--licenses` is the interim surface decision D-22 allowed until the app's "Open-Source Notices" window (D-25)
/// exists; that window shows the same text and replaces this flag as the user-facing surface.
pub fn licenses_text() -> String {
    format!(
        "{}\n{UNICODE_NOTICE}\n{DEPENDENCY_NOTICES}\n{RULE}\nInter font (SIL Open Font License 1.1)\n\n{INTER_FONT_NOTICE}",
        farhelm_supervisor::grid_input::PORTED_CODE_NOTICES,
    )
}

/// The separator `scripts/dependency-notices.py` puts between notice sections, reused so the font's section reads like
/// the others.
const RULE: &str =
    "================================================================================";

/// Run the desktop app until it quits.
///
/// Starts the embedded helm and the local supervisor, then hands the main thread to the GPUI app, which owns it until
/// the app quits (GPUI's event loop has to run there, on macOS because AppKit requires it). The window is the GPUI
/// app's empty first screen until the foundations it draws with are built; nothing feeds it data yet.
///
/// A bootstrap failure, and a window that fails to open, go through [`refuse_and_exit`], the pre-window refusal path:
/// there is no window in which to show an error, and a launcher launch still needs its native surface. SIGTERM and Ctrl+C quit the app the ordinary
/// way. Quitting asks the embedded helm to stop; the local supervisor keeps running, as it always has after a quit. On
/// Linux this function then waits for the helm to finish stopping. On macOS quitting ends the process from inside
/// GPUI's event loop, so the helm gets only GPUI's quit deadline to stop and this function never returns.
pub fn run() -> anyhow::Result<()> {
    init_tracing();
    let mut desktop = match DesktopBootstrap::start() {
        Ok(desktop) => desktop,
        Err(error) => refuse_and_exit(&format!("farhelm-desktop: {error:#}")),
    };
    #[cfg(feature = "agent")]
    agent::publish_helm(&desktop)?;
    let quit_requested = quit_on_signal()?;
    let helm_shutdown = desktop.shutdown_requester();
    farhelm_gui::bootstrap::run(farhelm_gui::bootstrap::Host {
        quit_requested: Box::pin(async move {
            // A sender dropped without sending means the signal thread failed, not that anyone asked to quit, so the
            // app keeps running and only its user can quit it.
            if quit_requested.await.is_err() {
                std::future::pending::<()>().await;
            }
        }),
        on_quit: Box::new(move || helm_shutdown.request()),
        // Exits the process: no window opened, so the refusal is the only way the user hears why (a machine with no
        // usable graphics driver is the realistic case). The helm dies with the process, as on any other refusal.
        on_startup_failure: Box::new(|error| {
            refuse_and_exit(&format!(
                "farhelm-desktop: the window could not open: {error}"
            ))
        }),
        tab_reach: Box::new(tab_reach::current),
    });
    // Linux only: GPUI's event loop has returned. Dropping the bootstrap waits for the helm the quit asked to stop.
    tracing::info!("the app quit; waiting for the embedded helm to stop");
    drop(desktop);
    tracing::info!("the embedded helm stopped");
    Ok(())
}

/// Turn SIGTERM and SIGINT into one quit request for the GPUI app.
///
/// The handlers are installed before this returns, so a signal that arrives while the window is still opening is not
/// lost. They are installed for the life of the process: after the first signal, further ones are swallowed rather
/// than killing the app halfway through its quit (a caller that needs it gone sooner sends SIGKILL).
fn quit_on_signal() -> anyhow::Result<tokio::sync::oneshot::Receiver<()>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("starting the signal runtime")?;
    let (mut terminate, mut interrupt) = runtime.block_on(async {
        use tokio::signal::unix::{SignalKind, signal};
        anyhow::Ok((
            signal(SignalKind::terminate()).context("installing the SIGTERM handler")?,
            signal(SignalKind::interrupt()).context("installing the SIGINT handler")?,
        ))
    })?;
    let (quit, requested) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("quit-signals".into())
        .spawn(move || {
            let name = runtime.block_on(async {
                tokio::select! {
                    _ = terminate.recv() => "SIGTERM",
                    _ = interrupt.recv() => "SIGINT",
                }
            });
            tracing::info!(signal = name, "quitting on a signal");
            let _ = quit.send(());
        })
        .context("starting the signal thread")?;
    Ok(requested)
}

/// Install this process's `tracing` subscriber.
///
/// Without a subscriber, every `tracing::error!`/`warn!` this binary emits, the
/// embedded helm's included, reaches the default no-op dispatcher and simply
/// vanishes.
/// `crates/farhelm`'s CLI installs its own subscriber (`init_tracing` there)
/// for `farhelm helm run` and `farhelm supervisor run`, but those are that
/// OTHER binary's subcommands, running in a spawned subprocess; this desktop
/// app embeds a helm directly in ITS OWN process ([`DesktopBootstrap::start`])
/// and never goes through that code path, so it needs the same setup
/// independently. Mirrors that function's filter default (`RUST_LOG`, else
/// `info`) and its choice of stderr.
fn init_tracing() {
    // Build-sensitive default: debug builds (the development flow's default)
    // keep debug-level events, release stays at info. `RUST_LOG` overrides both.
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

/// The app-owned processes and authentication state kept alive by `main`.
pub struct DesktopBootstrap {
    /// Loopback origin of the embedded helm, e.g. `http://127.0.0.1:41233`.
    base: String,
    /// The GUI's in-memory credential for this launch, minted by the embedded
    /// helm ([`farhelm_helm::EmbeddedReady`]). Never persisted.
    native_device_secret: String,
    /// The state root was resolved and made private during bootstrap.
    state_dir: PathBuf,
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
    /// Discover, start or replace the local supervisor and own one embedded
    /// helm.
    ///
    /// An answering supervisor is reused unchanged unless it is the one an
    /// earlier launch of the app started (by its record) and runs another
    /// program than this launch would start; see [`plan_local_supervisor`].
    /// A supervisor this launch starts is detached and outlives the app; it
    /// is monitored only until startup is done.
    pub fn start() -> anyhow::Result<Self> {
        let state_dir = desktop_state_dir()?;
        let runtime =
            tokio::runtime::Runtime::new().context("starting desktop bootstrap runtime")?;
        runtime.block_on(farhelm_supervisor::ensure_private_dir(&state_dir))?;

        // Also holds this app's own version for the rest of the process, so
        // it must come before discovery probes anything or a supervisor is
        // spawned from it (see `bundle::supervisor_farhelm`).
        let farhelm = bundled_farhelm()?;
        // The program this launch would start is read before anything is
        // spawned from it: the record of a supervisor started from it must
        // name the file it was started from, not one put at that path later
        // (see `farhelm_supervisor::managed_supervisor`).
        let absolute = std::path::absolute(&farhelm)
            .context("resolving the managed supervisor program path")?;
        let program = ProgramFile::read(&absolute).with_context(|| {
            format!(
                "reading the managed supervisor program {}",
                absolute.display()
            )
        })?;
        // The whole decision, and any stop or start, happens under the
        // helm's lock (what an open app's embedded helm holds), taken before
        // anything is read: an open app makes the launch refuse before it
        // touches anything, and two launches cannot act on what they saw
        // before the other changed it. Every writer and remover of the
        // supervisor record holds this lock (uninstall takes it too), so
        // what is read here stays true until it is released, just before
        // this launch's own embedded helm takes the same lock below. A
        // launch that slips in between makes a fresh decision of its own.
        let app_lock = refuse_if_app_open(&state_dir, LAUNCH_LOCK_WAIT)?;
        let own = own_supervisor(&state_dir);
        let found = discovered(
            runtime.block_on(farhelm_helm::discover_local_supervisor(
                &farhelm, &state_dir,
            )),
            own.is_some(),
        )?;
        let spawn = SpawnInputs {
            farhelm: &farhelm,
            absolute: &absolute,
            program: &program,
            state_dir: &state_dir,
        };
        let plan = plan_local_supervisor(
            own.map(|own| {
                let current = own.started_from(&program);
                (own, current)
            }),
            found,
        );
        // The child is held only while startup runs, so a supervisor this
        // launch started that exits early is reported. It runs detached and
        // outlives the app: the handle is dropped without waiting for it, and
        // nothing stops it when the app quits.
        let mut supervisor = match plan {
            SupervisorPlan::Reuse => None,
            SupervisorPlan::Spawn => {
                spawn.check_prerequisites()?;
                let log = spawn.open_log()?;
                Some(spawn.spawn(log)?)
            }
            SupervisorPlan::Replace { own, wait_first } => {
                // Everything that could refuse comes first: a launch that
                // would refuse anyway must not have stopped the supervisor
                // that was serving the user. That includes a new program
                // file that cannot run at all, which discovery cannot tell
                // apart from a predecessor that does not answer, and a log
                // that cannot be opened.
                spawn.check_prerequisites()?;
                spawn.check_program_runs()?;
                let log = spawn.open_log()?;
                let answered = wait_first
                    && runtime.block_on(await_own_supervisor_answering(
                        &own,
                        MANAGED_SUPERVISOR_SERVING_WAIT,
                        || async {
                            Ok(
                                farhelm_helm::discover_local_supervisor(&farhelm, &state_dir)
                                    .await?
                                    == farhelm_helm::LocalSupervisorDiscovery::Answering,
                            )
                        },
                    )) == OwnSupervisorWait::Answered;
                if answered {
                    None
                } else {
                    // Under the lock no other launch can have started one
                    // meanwhile. A supervisor started by hand in this window
                    // races the new one for the state directory; if it wins,
                    // this launch fails to start, and the next launch
                    // reuses it.
                    stop_own_supervisor(&own, &state_dir)?;
                    Some(spawn.spawn(log)?)
                }
            }
        };
        ensure_managed_supervisor_running(&mut supervisor)?;
        if let Some(child) = &mut supervisor {
            runtime.block_on(await_managed_supervisor_serving(
                child,
                MANAGED_SUPERVISOR_SERVING_WAIT,
                || async {
                    Ok(
                        farhelm_helm::discover_local_supervisor(&farhelm, &state_dir).await?
                            == farhelm_helm::LocalSupervisorDiscovery::Answering,
                    )
                },
            ))?;
        }
        drop(app_lock);

        // The embedded helm is an internal implementation detail of this
        // process. Port 0 lets the kernel choose a fresh loopback port for
        // each launch, so another local process cannot predict or reserve a
        // stable endpoint before the desktop starts.
        let port = 0;
        let (ready_tx, ready_rx) = mpsc::channel::<farhelm_helm::EmbeddedReady>();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let helm_state = state_dir.clone();
        let helm = std::thread::Builder::new()
            .name("farhelm-embedded-helm".to_string())
            .spawn(move || {
                farhelm_helm::run_embedded(
                    farhelm_helm::HelmArgs {
                        port,
                        state_dir: Some(helm_state),
                        test_credential_file: None,
                        ensure_hosts: None,
                        payload_dir: helm_payload_dir(),
                        release_base_url: None,
                        backstop_refresh_secs: None,
                    },
                    Some(std::sync::Arc::new(|url: &str| {
                        webbrowser::open(url).map_err(|error| error.to_string())
                    })),
                    // Resolve on the native side. An unavailable Downloads
                    // folder refuses a save.
                    dirs::download_dir().map(|directory| farhelm_helm::NativeDownloads {
                        directory,
                        opener: std::sync::Arc::new(open_saved_file),
                    }),
                    ready_tx,
                    shutdown_rx,
                )
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
        runtime.block_on(await_local_supervisor(
            &base,
            &state_dir,
            &native_secret,
            &mut supervisor,
        ))?;

        // Rewrite the state file once, before any window exists. A file from
        // an older build still holds that build's device secrets, and this
        // drops them at launch; a file that cannot be read or written fails
        // here, on the pre-window refusal path, instead of later inside the
        // window.
        let state_path = state_dir.join(APP_STATE_FILE);
        update_state(&state_path, |_| {})?;

        startup_failure.disarm();
        Ok(Self {
            base,
            native_device_secret: native_secret,
            state_dir,
            helm_monitor: Some(helm_monitor),
            helm_shutdown: Some(shutdown_tx),
            expected_helm_shutdown,
        })
    }

    /// Loopback origin of this process's embedded helm.
    pub fn api_base(&self) -> &str {
        &self.base
    }

    /// The GUI's credential for the embedded helm: valid for this process only.
    pub fn native_device_secret(&self) -> &str {
        &self.native_device_secret
    }

    /// The private, already-created state directory this launch resolved.
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// A handle that asks the embedded helm to stop without waiting for it, for the GUI's quit handler, which runs
    /// under GPUI's 200 ms quit deadline. Dropping the bootstrap afterwards still waits for the helm to finish. Only the
    /// first requester can stop the helm; a later one does nothing.
    pub fn shutdown_requester(&mut self) -> HelmShutdown {
        HelmShutdown {
            shutdown: self.helm_shutdown.take(),
            expected: Arc::clone(&self.expected_helm_shutdown),
        }
    }
}

/// Asks the embedded helm to stop; see [`DesktopBootstrap::shutdown_requester`].
///
/// Dropping it without [`Self::request`] also marks the helm's stop as expected: the dropped sender stops the helm, and
/// that must not read as the helm dying. GPUI can drop its quit handlers uncalled (when the app is torn down while the
/// quit callback cannot borrow it), and the helm's monitor would otherwise show a false "stopped unexpectedly" refusal.
pub struct HelmShutdown {
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    expected: Arc<AtomicBool>,
}

impl HelmShutdown {
    /// Ask the helm to stop. Marks the stop as expected first, so the helm's monitor does not report it as the helm
    /// dying.
    pub fn request(mut self) {
        self.expected.store(true, Ordering::Release);
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

impl Drop for HelmShutdown {
    fn drop(&mut self) {
        // Before the sender drops (fields drop after this body), so the monitor sees the flag first.
        self.expected.store(true, Ordering::Release);
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

/// Stops the embedded helm and waits for it. The supervisor is left running:
/// it outlives the app by design. On Linux a quit reaches this `Drop` after
/// GPUI's event loop returns ([`run`]); on macOS GPUI ends the process from
/// inside its event loop first, so this never runs on a quit there.
impl Drop for DesktopBootstrap {
    fn drop(&mut self) {
        self.expected_helm_shutdown.store(true, Ordering::Release);
        if let Some(shutdown) = self.helm_shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(monitor) = self.helm_monitor.take() {
            let _ = monitor.join();
        }
    }
}

/// What startup does about the local supervisor. `T` is the app's own
/// supervisor from an earlier launch, carried into the one plan that acts on
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SupervisorPlan<T> {
    /// Use the supervisor that answers, as it is.
    Reuse,
    /// Start one: none answers and none of the app's own is running.
    Spawn,
    /// Stop the app's own supervisor the orderly way, then start one.
    /// `wait_first` when it runs the current program but does not answer:
    /// it may still be starting, so startup first gives it a while to
    /// answer and reuses it if it does.
    Replace { own: T, wait_first: bool },
}

/// Decide what startup does about the local supervisor.
///
/// A supervisor the app did not start (no matching record) is an ownership
/// boundary: reused if it answers, never stopped, and only absence permits
/// starting one. The app's own supervisor is replaced when it runs another
/// program than this launch would start (an update, or a development build
/// replacing the file), and when it does not answer at all (wedged, holding
/// the state directory without serving), and reused otherwise. Comparing
/// program files rather than versions matters: every untagged build reports
/// the same version, so a version comparison would leave each development
/// build after the first talking to the first one's supervisor.
///
/// A current-program supervisor that does not answer gets a wait first,
/// whether nothing is listening or its hello fails: it may still be
/// starting. Before it binds its socket it waits for the state directory or
/// migrates its database after an update; after binding it runs startup
/// sweeps (`Supervisor::serve`: the launch directory, the repository cache,
/// attachment reconciliation) before it accepts, so a probe in that window
/// connects but gets no hello. Only after the wait is it treated as wedged.
/// The extra wait costs only a supervisor that really is wedged, which is
/// rare; stopping a healthy slow one would interrupt its startup.
///
/// The record never proves that the answering supervisor is the recorded
/// process. When another program's supervisor of the app's own is stopped
/// while a supervisor started by hand answers, this launch's new supervisor
/// loses the state directory to it and this launch fails to start; the next
/// launch finds no supervisor of its own and reuses the hand-started one.
///
/// `own` is the app's own supervisor (its record matched a live process; see
/// [`own_supervisor`]) with whether it was started from the program file this
/// launch would start (same resolved path, same device and inode). A failed
/// discovery without one never reaches the planner ([`discovered`] refuses
/// it); it is mapped to `Reuse`, the plan that touches nothing.
fn plan_local_supervisor<T>(own: Option<(T, bool)>, found: Discovered) -> SupervisorPlan<T> {
    match (own, found) {
        (None, Discovered::Absent) => SupervisorPlan::Spawn,
        (None, Discovered::Answering | Discovered::Failed) => SupervisorPlan::Reuse,
        (Some((own, false)), _) => SupervisorPlan::Replace {
            own,
            wait_first: false,
        },
        (Some(_), Discovered::Answering) => SupervisorPlan::Reuse,
        (Some((own, true)), Discovered::Absent | Discovered::Failed) => SupervisorPlan::Replace {
            own,
            wait_first: true,
        },
    }
}

/// How waiting for the app's own non-answering supervisor ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnSupervisorWait {
    /// It answered: it was still starting, not wedged.
    Answered,
    /// It is gone, no longer the recorded process, or could not be read
    /// (the stop that follows re-reads it and refuses if it still cannot).
    Gone,
    /// It neither answered nor went within the wait.
    TimedOut,
}

/// Give the app's own supervisor that runs the current program but did not
/// answer up to `wait` to start answering, before startup treats it as
/// wedged.
///
/// A supervisor can be legitimately silent for a while: waiting for the
/// state directory a just-quit predecessor held (`STATE_DIR_CLAIM_WAIT`),
/// or a slow first start after an update (a database migration). Stopping it
/// then, and killing it if it does not stop within the grace, could
/// interrupt a migration, and a user who relaunches during that window
/// would loop. `answers` is the probe (startup passes the same
/// hello-based discovery); a probe error counts as not answering yet.
async fn await_own_supervisor_answering<P, F>(
    own: &ManagedSupervisor,
    wait: Duration,
    mut answers: P,
) -> OwnSupervisorWait
where
    P: FnMut() -> F,
    F: std::future::Future<Output = anyhow::Result<bool>>,
{
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        if own.observe() != Ok(Observation::Matches) {
            return OwnSupervisorWait::Gone;
        }
        // Each probe is bounded by what is left of the wait, so one slow
        // probe cannot stretch it, and none starts once the wait is over.
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return OwnSupervisorWait::TimedOut;
        }
        if matches!(tokio::time::timeout(left, answers()).await, Ok(Ok(true))) {
            return OwnSupervisorWait::Answered;
        }
        tokio::time::sleep(Duration::from_millis(500).min(left)).await;
    }
}

/// The launch lock: the helm's lock in `state_dir`, which every launch takes
/// before it reads the supervisor record, so the whole decision is made on a
/// snapshot no other launch or uninstall can change; or a refusal when an
/// open app (its embedded helm) or another launch holds it.
///
/// The open-app refusal is the second reason it is taken this early: a
/// second launch whose program differs (the Linux desktop app after a
/// rebuild, or `open -n` on the Mac) would otherwise stop the supervisor
/// serving the open app and only then refuse at the helm's own lock. The
/// file is created if missing (0600, the helm's mode); it is released before
/// this launch's own embedded helm takes it.
///
/// A brief holder (an offline `farhelm helm token show`, say) is waited out
/// for up to `wait`; startup passes [`LAUNCH_LOCK_WAIT`].
fn refuse_if_app_open(state_dir: &Path, wait: Duration) -> anyhow::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let path = state_dir.join(farhelm_helm::HELM_LOCK_FILE);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(&path)
        .with_context(|| format!("opening {}", path.display()))?;
    let deadline = std::time::Instant::now() + wait;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(std::fs::TryLockError::WouldBlock) => bail!(
                "another process owns token control in {} (usually Farhelm already open); \
                 nothing was changed",
                state_dir.display()
            ),
            Err(std::fs::TryLockError::Error(error)) => {
                return Err(error).with_context(|| format!("locking {}", path.display()));
            }
        }
    }
}

/// What discovery found, for [`plan_local_supervisor`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Discovered {
    /// A supervisor completed the hello.
    Answering,
    /// Positively nothing listening.
    Absent,
    /// The discovery failed (the socket accepted but no hello came back
    /// within the probe timeout, say): only possible to act on for the app's
    /// own supervisor. Not proof of a wedge: a supervisor running its
    /// post-bind startup sweeps looks the same.
    Failed,
}

/// Classify a discovery result.
///
/// A discovery that failed outright (the socket accepted but the hello
/// never came back within the probe timeout, say) is fatal when no
/// supervisor of the app's own is running: the app cannot tell what holds
/// the state directory and must not start a rival. When the app's own
/// supervisor is running (`own`), the same failure means that supervisor is
/// not serving: still starting, or wedged and holding its socket and state
/// directory, which is the case the replacement exists for (the planner
/// waits before treating it as wedged). A failure that comes from
/// this launch's own program not running at all also lands here; the
/// replacement path checks that the program runs before it stops anything.
fn discovered(
    discovery: anyhow::Result<farhelm_helm::LocalSupervisorDiscovery>,
    own: bool,
) -> anyhow::Result<Discovered> {
    match discovery {
        Ok(farhelm_helm::LocalSupervisorDiscovery::Answering) => Ok(Discovered::Answering),
        Ok(farhelm_helm::LocalSupervisorDiscovery::Absent) => Ok(Discovered::Absent),
        Err(error) if own => {
            tracing::warn!(
                error = format!("{error:#}"),
                "the supervisor an earlier launch started does not answer"
            );
            Ok(Discovered::Failed)
        }
        Err(error) => Err(error),
    }
}

/// The record of the supervisor an earlier launch started in `state_dir`,
/// when it still describes a live process exactly (pid, start identity and
/// running program; see `ManagedSupervisor::observe`).
///
/// Anything less is "not ours": no record, a damaged one, a process that is
/// gone or runs something else, or one that could not be read (logged).
/// That is the safe direction, since a supervisor that is not ours is never
/// stopped.
///
/// A recorded supervisor that reads as gone is first given a few seconds to
/// finish exiting (`ManagedSupervisor::wait_exited`). One that died on its
/// own just before this launch can still hold the state directory's lock
/// and its socket while its last threads exit, and discovery would then
/// reach a dying listener and refuse the launch for a supervisor it cannot
/// identify. One still exiting after the wait is logged and treated as
/// gone, leaving discovery and the new supervisor's claim to deal with it.
fn own_supervisor(state_dir: &Path) -> Option<ManagedSupervisor> {
    let record = match ManagedSupervisor::read(state_dir) {
        Ok(record) => record?,
        Err(error) => {
            tracing::warn!(%error, "reading the desktop app's supervisor record");
            return None;
        }
    };
    match record.observe() {
        Ok(Observation::Matches) => Some(record),
        Ok(Observation::Gone) => {
            match record.wait_exited() {
                Ok(true) => {}
                Ok(false) => tracing::warn!(
                    pid = record.pid,
                    "the supervisor an earlier launch started is still exiting"
                ),
                Err(error) => tracing::warn!(
                    pid = record.pid,
                    %error,
                    "could not check whether the supervisor an earlier launch started has \
                     finished exiting"
                ),
            }
            None
        }
        Ok(Observation::RunsSomethingElse) => None,
        Err(error) => {
            tracing::warn!(
                pid = record.pid,
                %error,
                "could not check the supervisor an earlier launch started; leaving it alone"
            );
            None
        }
    }
}

/// Stop the app's own supervisor from an earlier launch, so this launch can
/// start the current one.
///
/// The orderly stop first (SIGTERM, which closes its terminal-output
/// clients in order), and a kill only after [`ORDERLY_STOP_GRACE`]: this is
/// the app's own supervisor, and a wedged one would otherwise keep the
/// state directory forever. Its record goes once it is confirmed gone,
/// which the stop reports only when no thread of it can run any more: an
/// exiting supervisor holds the state directory's lock and its socket until
/// its last thread has finished, and the supervisor this launch starts next
/// needs both. A stop that cannot be confirmed refuses startup, keeping the
/// record, and so does one whose supervisor is still exiting after the
/// stop's few seconds.
fn stop_own_supervisor(own: &ManagedSupervisor, state_dir: &Path) -> anyhow::Result<()> {
    tracing::info!(
        pid = own.pid,
        program = %own.program.display(),
        "replacing the supervisor an earlier launch started"
    );
    match own
        .stop(ORDERLY_STOP_GRACE, true)
        .map_err(|error| anyhow::anyhow!("stopping the previous local supervisor: {error}"))?
    {
        StopOutcome::AlreadyGone | StopOutcome::Stopped => {}
        StopOutcome::Killed => tracing::warn!(
            pid = own.pid,
            "the previous local supervisor ignored the orderly stop and was killed"
        ),
        StopOutcome::StillRunning => bail!(
            "the previous local supervisor (process {}) did not stop, even when killed",
            own.pid
        ),
        StopOutcome::StillExiting => bail!(
            "the previous local supervisor (process {}) had not finished exiting a few \
             seconds after it ended; open Farhelm again in a moment",
            own.pid
        ),
    }
    if let Err(error) = ManagedSupervisor::remove(state_dir) {
        tracing::warn!(%error, "removing the previous supervisor's record");
    }
    Ok(())
}

/// The supervisor's log file in its state directory: the supervisor writes
/// its diagnostics there because it outlives the app whose stderr it used to
/// share (and a long-lived process must not hold a stderr that can go away;
/// a closed stderr once panicked one).
const SUPERVISOR_LOG: &str = "supervisor.log";

/// The previous log, kept once when the log is rotated.
const SUPERVISOR_LOG_PREVIOUS: &str = "supervisor.log.1";

/// The size at which a launch that starts a supervisor rotates its log
/// first. The bound is per start: a supervisor that runs for weeks appends
/// to one file for that long.
const SUPERVISOR_LOG_ROTATE_AT: u64 = 16 * 1024 * 1024;

/// Open the supervisor's log for appending (private, created if missing).
fn open_supervisor_log(state_dir: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(state_dir.join(SUPERVISOR_LOG))
}

/// The log a supervisor about to start should write to: `kept` (an open
/// append handle on `supervisor.log`) unless the log has reached
/// [`SUPERVISOR_LOG_ROTATE_AT`], in which case it is moved aside as
/// [`SUPERVISOR_LOG_PREVIOUS`] (replacing an older one) and a fresh one
/// opened.
///
/// Best-effort by design: it runs after a predecessor may have been
/// stopped, so a rotation that fails must not refuse the start; it is
/// logged and `kept` is used. Called only when a supervisor is about to be
/// started under the launch lock, so no supervisor the app knows of is
/// writing the file.
fn rotated_supervisor_log(state_dir: &Path, kept: File) -> File {
    if kept
        .metadata()
        .map_or(true, |metadata| metadata.len() < SUPERVISOR_LOG_ROTATE_AT)
    {
        return kept;
    }
    let rotated = std::fs::rename(
        state_dir.join(SUPERVISOR_LOG),
        state_dir.join(SUPERVISOR_LOG_PREVIOUS),
    )
    .and_then(|()| open_supervisor_log(state_dir));
    match rotated {
        Ok(fresh) => fresh,
        Err(error) => {
            tracing::warn!(%error, "rotating the supervisor log; appending to it instead");
            kept
        }
    }
}

/// What starting the managed supervisor needs, gathered once so both the
/// plain start and the replacement use the same checks and command.
struct SpawnInputs<'a> {
    /// The program as the app names it (possibly a relative developer
    /// override), used for the command itself.
    farhelm: &'a Path,
    /// `farhelm` resolved to an absolute path: what the session program
    /// check and `program` are about, whatever spelling the command uses.
    absolute: &'a Path,
    /// `absolute`'s file, read before the spawn, for the record.
    program: &'a ProgramFile,
    state_dir: &'a Path,
}

impl SpawnInputs<'_> {
    /// Refuse when the program this launch would start cannot run at all
    /// (not executable, a missing loader, a damaged file): checked before a
    /// replacement stops the supervisor that is running, since nothing else
    /// would notice until after the stop. Runs `<program> --version`, bounded
    /// in time.
    fn check_program_runs(&self) -> anyhow::Result<()> {
        let mut child = Command::new(self.farhelm)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("running {}", self.farhelm.display()))?;
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait()? {
                if status.success() {
                    return Ok(());
                }
                bail!("{} --version failed with {status}", self.farhelm.display());
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                bail!(
                    "{} --version did not finish within 10 seconds",
                    self.farhelm.display()
                );
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Open the supervisor's log for appending. Called before anything is
    /// stopped, so a log that cannot be opened refuses the launch while the
    /// old supervisor still runs; rotation happens later, at the spawn, and
    /// falls back to this handle.
    fn open_log(&self) -> anyhow::Result<File> {
        open_supervisor_log(self.state_dir)
            .with_context(|| format!("opening the supervisor log in {}", self.state_dir.display()))
    }

    /// Refuse before anything is started or stopped when the supervisor
    /// could not run: a session program path the shell cannot use.
    fn check_prerequisites(&self) -> anyhow::Result<()> {
        // Resolve a relative developer override before checking the
        // session PATH folder: its working directory can supply a colon.
        // Returning the refusal here gives startup's existing alert the
        // same folder and reason as the child would report on stderr.
        let session_program = farhelm_supervisor::app_bundle::versioned_program(self.absolute)
            .map_or_else(|| self.absolute.to_path_buf(), |program| program.forwarder);
        farhelm_supervisor::launch::validate_session_program_path(&session_program)?;
        Ok(())
    }

    /// Start the supervisor detached, and record it as the app's own.
    ///
    /// Detached means its own session (`setsid`: no controlling terminal, so
    /// a terminal's Ctrl-C or hangup aimed at a terminal-launched app never
    /// reaches it), stdin and stdout on `/dev/null`, and stderr to its log
    /// in the state directory. It is no longer tethered on stdin
    /// (`--exit-on-stdin-close` stays a supervisor option for tests); it
    /// keeps running after the app quits, as a Linux supervisor runs under
    /// systemd.
    ///
    /// The record is written right after the spawn. A crash in between
    /// leaves a supervisor without a record, which later launches treat as
    /// not the app's own: reused while it answers, and not replaced on an
    /// update until it exits. A record that cannot be written is logged and
    /// has the same effect.
    fn spawn(&self, log: File) -> anyhow::Result<Child> {
        use std::os::unix::process::CommandExt as _;
        let log = rotated_supervisor_log(self.state_dir, log);
        let mut command = Command::new(self.farhelm);
        command
            .args(["supervisor", "run", "--state-dir"])
            .arg(self.state_dir)
            // The supervisor inherits this process's PATH unchanged: it is
            // launched by absolute path, and the launch shim prepends the
            // directory the supervisor names for the spawn CLI inside
            // sessions, so nothing else needs the bundle's directory there.
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            // The payload directory is this app's embedded helm's setting
            // alone. Inherited, it would reach every session shell the
            // supervisor starts, so a `farhelm helm run` typed
            // in a session would silently push this launch's payloads,
            // possibly long after the app that set it has quit.
            .env_remove("FARHELM_HELM_PAYLOAD_DIR")
            // The reopen marker is for this one reopened app alone (design,
            // section 1: at most once). Inherited, it would reach every
            // session, where a Farhelm started from one would take it as its
            // own: refusing as "replaced by another installation", or never
            // reopening itself.
            .env_remove(bundle::REOPENED_ENV);
        // SAFETY: setsid is async-signal-safe and touches no memory; nothing
        // else runs between fork and exec here.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn().with_context(|| {
            format!(
                "starting the managed supervisor child through the farhelm at {}",
                self.farhelm.display()
            )
        })?;
        // A child already gone gets no record; startup's liveness check
        // reports why it went.
        if let Some(record) = ManagedSupervisor::for_process(child.id(), self.program)
            && let Err(error) = record.write(self.state_dir)
        {
            tracing::warn!(
                %error,
                "recording the supervisor this launch started; later launches will treat it as not the app's own"
            );
        }
        Ok(child)
    }
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

/// How long startup waits for the supervisor child it just spawned to start
/// serving, before the embedded helm is started.
///
/// The child itself may spend up to [`STATE_DIR_CLAIM_WAIT`] waiting for a
/// Farhelm that was just quit to release the state directory, and then has
/// its own startup to do (opening its database). The margin covers that
/// startup on a slow machine; a child that is still not serving after it is
/// reported rather than waited for.
const MANAGED_SUPERVISOR_SERVING_WAIT: Duration =
    STATE_DIR_CLAIM_WAIT.saturating_add(Duration::from_secs(15));

/// How long a launch waits for the launch lock ([`refuse_if_app_open`]) before
/// refusing: the same wait the helm itself gives a brief holder of that lock.
const LAUNCH_LOCK_WAIT: Duration = farhelm_helm::HELM_LOCK_WAIT;

/// Wait until the supervisor child this app spawned answers, failing early if
/// the child exits.
///
/// Without this, the embedded helm starts dialing at once and backs off
/// (about 0, 1, 3, 7, 15 and 30 seconds) while a child that is waiting out a
/// just-quit predecessor is not serving yet. A child that starts serving
/// between two of those attempts sits unnoticed until the next one, which
/// can land after the 30 seconds [`await_local_supervisor`] allows, and the
/// app then refuses to open beside a supervisor that is running fine.
/// Starting the helm only once the child answers makes the helm's first
/// attempt the one that connects.
///
/// `answers` is the probe. Startup passes the same hello-based discovery it
/// ran before spawning, rather than a bare connect: a connection dropped
/// before its hello is logged by the supervisor as an error, which would put
/// a false alarm in every launch's log. Nothing here touches
/// `supervisor.lock`; taking that lock to test it is exactly what could make
/// the starting child find it held.
async fn await_managed_supervisor_serving<P, F>(
    child: &mut Child,
    wait: Duration,
    mut answers: P,
) -> anyhow::Result<()>
where
    P: FnMut() -> F,
    F: std::future::Future<Output = anyhow::Result<bool>>,
{
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        // A probe error is checked against the child before it is
        // reported: a child that just exited is the real cause, and the
        // probe's own complaint about it (a hello cut short, or a removed
        // program that will not spawn) would only hide that.
        let answered = answers().await;
        if let Some(status) = child
            .try_wait()
            .context("monitoring the managed supervisor child during desktop startup")?
        {
            bail!("the managed supervisor child exited during desktop startup with {status}");
        }
        if answered? {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            bail!(
                "the managed local supervisor did not start serving within {} seconds",
                wait.as_secs()
            );
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
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
///
/// Peer builds and identities need visible presentation here, before the
/// refusal reaches the native alert: AppleScript quoting only protects its
/// syntax. Identity evidence also exposes spaces and literal escape markers
/// so distinct installs cannot look alike. The app's own text and line breaks
/// keep their original spelling.
fn local_supervisor_refusal(
    hosts: &[farhelm_proto::ui_channel::HostView],
    state_dir: &Path,
    spawned: bool,
) -> Option<String> {
    let local = hosts
        .iter()
        .find(|host| crate::local_host::is_this_machine(host))?;
    let label = local_supervisor_label(spawned);
    let dir = state_dir.display();
    Some(match &local.state {
        farhelm_proto::ui_channel::HostStateView::VersionSkew {
            peer_protocol,
            peer_build,
            our_protocol,
            our_build,
            remediation,
        } => {
            // Our build and the local remedy are app-authored on this path.
            // Their original line breaks belong to the native alert's structure.
            let peer_build = crate::local_host::display_peer(peer_build);
            let our_build = our_build.as_str();
            let remediation = remediation.as_str();
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
        farhelm_proto::ui_channel::HostStateView::IdentityMismatch { recorded, reported } => {
            let recorded = crate::local_host::display_identity(recorded);
            let reported = crate::local_host::display_identity(reported);
            format!(
                "the {label} in {dir} reports identity {reported}, but this machine's host entry \
             recorded {recorded}. The supervisor's database (supervisor.db) and this app's own \
             (helm.db) in {dir} no longer describe the same install, which happens when one of \
             them is restored from a backup or replaced without the other, or when \
             supervisor.db is deleted. Restarting this app alone will not change it. To fix \
             it, quit Farhelm, stop the supervisor still running for {dir} (the app's own keeps \
             running after a quit; its process number is in desktop-supervisor.json there), and restore \
             both databases from the same backup, together with their -wal and -shm files \
             if the backup includes them. Before restoring, delete any current \
             supervisor.db-wal, supervisor.db-shm, helm.db-wal or helm.db-shm that the backup \
             does not include: a leftover journal could be applied to the restored database \
             and corrupt it."
            )
        }
        farhelm_proto::ui_channel::HostStateView::IdentityUnverified { recorded } => {
            let recorded = crate::local_host::display_identity(recorded);
            format!(
                "the {label} in {dir} reports no identity, but this machine's host entry recorded \
             {recorded}, so this app cannot tell whether it is the same supervisor."
            )
        }
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
            let hosts =
                tokio::time::timeout_at(deadline, crate::local_host::decode_hosts(response))
                    .await
                    .map_err(|_| not_connected_in_time(spawned))?
                    .map_err(anyhow::Error::msg)?;
            if let Some(refusal) = local_supervisor_refusal(&hosts, state_dir, spawned) {
                bail!(refusal);
            }
            if hosts.iter().any(|host| {
                crate::local_host::is_this_machine(host)
                    && matches!(
                        host.state,
                        farhelm_proto::ui_channel::HostStateView::Connected { .. }
                    )
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
/// stderr. The stderr line (`farhelm-desktop: <error>`) and exit status 1
/// remain the primary contract.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;
    use std::net::TcpListener;
    use std::time::Instant;

    /// The desktop binary ships on its own (its archive holds no `farhelm`), so it must carry the dependency notices
    /// itself (decision D-18); losing them would ship crates without their required attribution.
    ///
    /// Spec: `licenses_text` holds the embedded supervisor's ported-code notices (xterm.js and the Unicode License), the
    /// generated notices' header, a dependency only the desktop app links (taffy, through GPUI) with its own copyright
    /// line, and no SPDX template placeholder.
    #[farhelm_testtrace::test]
    fn the_desktop_binary_carries_the_dependency_notices() {
        let text = licenses_text();
        assert!(
            text.contains(
                "Copyright (c) 2017-2019, The xterm.js authors (https://github.com/xtermjs/xterm.js)"
            ),
            "the xterm.js notice for the supervisor's ported input encoding is missing"
        );
        assert!(
            text.contains("UNICODE LICENSE V3"),
            "the Unicode License notice for the emulator's tables is missing"
        );
        assert!(
            text.contains("SIL OPEN FONT LICENSE") && text.contains("Inter font"),
            "the embedded Inter font's license is missing"
        );
        assert!(
            text.contains(
                "Third-party license notices for Farhelm's farhelm and farhelm-desktop binaries"
            ),
            "the dependency notices are missing"
        );
        assert!(
            text.contains(" taffy 0."),
            "a desktop-only dependency (taffy) is not named"
        );
        assert!(
            text.contains("Copyright (c) 2018 Visly Inc."),
            "taffy's own copyright line is missing"
        );
        // Not a bare `<year>`: FreeType's license, shipped as its own notice, quotes a credit line with that
        // placeholder for users to fill in. The template's holder placeholder is what never belongs in the output.
        assert!(
            !text.contains("<copyright holders>") && !text.contains("<owner>"),
            "an SPDX template placeholder reached the notices"
        );
    }

    const PROXY_CHILD_ENV: &str = "FARHELM_DESKTOP_PROXY_TEST_CHILD";
    const PROXY_TARGET_ENV: &str = "FARHELM_DESKTOP_PROXY_TEST_TARGET";

    /// Linux dispatch must finish while the opened application is still alive:
    /// waiting for its exit would hold the save response and the helm runtime's
    /// blocking-pool shutdown. A subprocess owns private MIME associations;
    /// this never changes the test runner's environment or the user's defaults.
    #[cfg(target_os = "linux")]
    #[farhelm_testtrace::test]
    async fn native_file_dispatch_does_not_wait_for_the_opened_application() {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

        const CHILD_PATH: &str = "FARHELM_NATIVE_OPEN_FIXTURE";
        if let Some(path) = std::env::var_os(CHILD_PATH) {
            open_saved_file(Path::new(&path)).expect("dispatch fixture application");
            return;
        }

        let private = tempfile::tempdir().unwrap();
        let applications = private.path().join("applications");
        std::fs::create_dir(&applications).unwrap();
        let file = private.path().join("notes with spaces.txt");
        std::fs::write(&file, "saved document").unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let fixture = private.path().join("app.py");
        // The socket is both readiness and ownership: the app reports the
        // actual argv, answers a liveness probe, and exits on release or EOF.
        // Its timeout also bounds cleanup if the test process itself dies.
        std::fs::write(
            &fixture,
            "import json, socket, sys\n\
             with socket.create_connection(('127.0.0.1', int(sys.argv[1])), timeout=15) as peer:\n\
             \x20peer.sendall((json.dumps(sys.argv[2:]) + '\\n').encode())\n\
             \x20while peer.recv(1) == b'?':\n\
             \x20\x20peer.sendall(b'!')\n",
        )
        .unwrap();
        std::fs::write(
            applications.join("farhelm-fixture.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=fixture\nExec=/usr/bin/python3 {} {port} %f\nMimeType=text/plain;\nTerminal=false\n",
                fixture.display()
            ),
        )
        .unwrap();
        std::fs::write(
            private.path().join("mimeapps.list"),
            "[Default Applications]\ntext/plain=farhelm-fixture.desktop;\n",
        )
        .unwrap();
        let mut child = tokio::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::native_file_dispatch_does_not_wait_for_the_opened_application",
                "--nocapture",
            ])
            .env_clear()
            .env("HOME", private.path())
            .env("XDG_DATA_HOME", private.path())
            .env("XDG_CONFIG_HOME", private.path())
            .env("XDG_DATA_DIRS", private.path())
            .env(CHILD_PATH, &file)
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let (peer, _) = tokio::time::timeout(Duration::from_secs(10), listener.accept())
            .await
            .expect("fixture app must connect after dispatch")
            .unwrap();
        let mut peer = BufReader::new(peer);
        let mut argv = String::new();
        tokio::time::timeout(Duration::from_secs(5), peer.read_line(&mut argv))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(serde_json::from_str::<Vec<PathBuf>>(&argv).unwrap(), [file]);
        let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
            .await
            .expect("dispatch must finish before fixture app exits")
            .unwrap();
        assert!(status.success(), "dispatch subprocess failed: {status}");
        peer.get_mut().write_all(b"?").await.unwrap();
        let mut reply = [0];
        tokio::time::timeout(Duration::from_secs(5), peer.read_exact(&mut reply))
            .await
            .expect("opened app must still be alive after dispatch returns")
            .unwrap();
        assert_eq!(reply, *b"!");
        peer.get_mut().write_all(b".").await.unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), peer.read(&mut reply))
                .await
                .expect("released app must close its owned socket")
                .unwrap(),
            0
        );
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
        // The body the embedded helm would serve: the shared row type, serialized, as the real route does.
        let body = serde_json::json!({ "hosts": [crate::local_host::tests::host(
            "local",
            farhelm_proto::ui_channel::HostStateView::VersionSkew {
                peer_protocol: 9,
                peer_build: "old-build".into(),
                our_protocol: 10,
                our_build: "new-build".into(),
                remediation: "update the supervisor".into(),
            },
        )]})
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
    /// that disagree and says to restore both from one backup with no supervisor
    /// running. Backup sidecars must accompany them, and current sidecars absent
    /// from that backup must be removed so unrelated journal data cannot be replayed
    /// into the restored pair. Other local-host states, and refusing states on
    /// another host, leave startup waiting.
    #[farhelm_testtrace::test]
    fn a_refusing_local_supervisor_ends_startup_with_its_state_and_remedy() {
        let dir = Path::new("/home/someone/.local/state/farhelm");
        let host = crate::local_host::tests::host;
        let skew = farhelm_proto::ui_channel::HostStateView::VersionSkew {
            peer_protocol: 9,
            peer_build: "old-build".into(),
            our_protocol: 10,
            our_build: "new-build".into(),
            remediation: "update the supervisor".into(),
        };

        let refusal = local_supervisor_refusal(&[host("local", skew.clone())], dir, false)
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
                "local",
                farhelm_proto::ui_channel::HostStateView::IdentityMismatch {
                    recorded: "id-recorded".into(),
                    reported: "id-reported".into(),
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
            "stop the supervisor still running",
            "desktop-supervisor.json",
            "both databases from the same backup",
            "their -wal and -shm files",
            "if the backup includes them",
            "Before restoring, delete any current",
            "supervisor.db-wal",
            "supervisor.db-shm",
            "helm.db-wal",
            "helm.db-shm",
            "that the backup does not include",
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
                "local",
                farhelm_proto::ui_channel::HostStateView::IdentityUnverified {
                    recorded: "id-recorded".into(),
                },
            )],
            dir,
            false,
        )
        .expect("an unverifiable identity is a refusal");
        assert!(unverified.contains("no identity") && unverified.contains("id-recorded"));
        assert!(!unverified.contains("Stop the supervisor") && !unverified.contains("managed"));

        let own = local_supervisor_refusal(&[host("local", skew.clone())], dir, true)
            .expect("a refusal of the app's own supervisor still ends startup");
        assert!(own.contains("managed local supervisor"), "{own:?}");
        assert!(!own.contains("Stop the supervisor you started"), "{own:?}");
        assert!(
            own.contains("update the supervisor"),
            "with the app's own supervisor refused, the helm's remediation is the remedy: {own:?}"
        );

        assert_eq!(
            local_supervisor_refusal(&[host("ssh", skew)], dir, false),
            None,
            "another host's refusal is not the local supervisor's"
        );
        assert_eq!(
            local_supervisor_refusal(
                &[host(
                    "local",
                    farhelm_proto::ui_channel::HostStateView::Connecting {
                        attempt: 1,
                        stage: farhelm_proto::ui_channel::ConnectionStage::ConnectingToSupervisor,
                        last_error: None,
                    }
                )],
                dir,
                false
            ),
            None,
            "a local host still connecting keeps startup waiting"
        );
    }

    /// Native startup refusals must expose peer-controlled formatting without
    /// rewriting the app's diagnostic. AppleScript quoting alone leaves bidi
    /// and zero-width characters active; escaping the whole refusal would also
    /// destroy the app's structural newlines. Identity evidence has the stronger
    /// contract that spaces and literal escape markers cannot hide a difference.
    /// The real helper argv is inspected without launching either native helper.
    #[farhelm_testtrace::test]
    fn native_supervisor_refusals_escape_only_peer_fragments() {
        let dir = Path::new("/state/farhelm");
        let host = |state| crate::local_host::tests::host("local", state);

        // Builds use ordinary peer presentation: readable spaces and literal
        // markers survive, while peer line breaks and format controls do not.
        // Local values deliberately include those same bytes to catch accidental
        // escaping of the whole app-authored message.
        for (peer_build, shown_build) in [
            ("old-build", "old-build"),
            (
                "old build<U+0020>\u{202E}\u{200B}\npeer line",
                "old build<U+0020><U+202E><U+200B><U+000A>peer line",
            ),
        ] {
            let refusal = local_supervisor_refusal(
                &[host(
                    farhelm_proto::ui_channel::HostStateView::VersionSkew {
                        peer_protocol: 9,
                        peer_build: peer_build.into(),
                        our_protocol: 10,
                        our_build: "new build\u{200B}\nlocal build detail".into(),
                        remediation: "update the supervisor\nthen relaunch".into(),
                    },
                )],
                dir,
                true,
            )
            .expect("a local version skew is a refusal");
            let expected = format!(
                "the managed local supervisor in /state/farhelm speaks protocol 9 (build \
                 {shown_build}), but this app speaks protocol 10 (build new build\u{200B}\n\
                 local build detail): update the supervisor\nthen relaunch"
            );
            assert_eq!(refusal, expected);
            let argv = native_refusal_commands(&refusal);
            assert_eq!(argv[0][4], expected.replace('\n', " "));
            assert_eq!(
                argv[1][2],
                format!(
                    "display alert \"farhelm-desktop cannot start\" message \"{expected}\" as critical"
                )
            );
        }

        // Identity evidence exposes spaces and keeps actual controls distinct
        // from literal marker text. Reversing the space pair also exercises a
        // literal marker in the recorded-only, unverifiable-identity message.
        for (recorded, reported, shown_recorded, shown_reported) in [
            ("id-recorded", "id-reported", "id-recorded", "id-reported"),
            (
                "install id",
                "install<U+0020>id",
                "install<U+0020>id",
                "install<U+003C>U+0020>id",
            ),
            (
                "install<U+0020>id",
                "install id",
                "install<U+003C>U+0020>id",
                "install<U+0020>id",
            ),
            (
                "install\u{202E}\u{200B}",
                "install<U+202E><U+200B>",
                "install<U+202E><U+200B>",
                "install<U+003C>U+202E><U+003C>U+200B>",
            ),
        ] {
            assert_ne!(shown_recorded, shown_reported);
            let mismatch = local_supervisor_refusal(
                &[host(
                    farhelm_proto::ui_channel::HostStateView::IdentityMismatch {
                        recorded: recorded.into(),
                        reported: reported.into(),
                    },
                )],
                dir,
                false,
            )
            .expect("a local identity mismatch is a refusal");
            let evidence = format!(
                "reports identity {shown_reported}, but this machine's host entry recorded \
                 {shown_recorded}."
            );
            let unverified = local_supervisor_refusal(
                &[host(
                    farhelm_proto::ui_channel::HostStateView::IdentityUnverified {
                        recorded: recorded.into(),
                    },
                )],
                dir,
                false,
            )
            .expect("an unverifiable local identity is a refusal");
            let missing_evidence = format!(
                "reports no identity, but this machine's host entry recorded {shown_recorded},"
            );
            for (refusal, expected_evidence) in
                [(mismatch, evidence), (unverified, missing_evidence)]
            {
                let argv = native_refusal_commands(&refusal);
                assert!(argv[0][4].contains(&expected_evidence), "{argv:?}");
                assert!(argv[1][2].contains(&expected_evidence), "{argv:?}");
                for raw_control in ['\u{202E}', '\u{200B}'] {
                    assert!(!argv[0][4].contains(raw_control), "{argv:?}");
                    assert!(!argv[1][2].contains(raw_control), "{argv:?}");
                }
            }
        }
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

    /// Spawn a stand-in for the managed supervisor: `script` under `sh`,
    /// and confirm it is running before the test acts on it.
    fn stand_in_child(script: &str) -> Child {
        let mut child = Command::new("sh")
            .args(["-c", script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn the stand-in supervisor");
        assert!(
            child.try_wait().expect("poll the stand-in").is_none(),
            "fixture: the stand-in must still be running before the test acts on it"
        );
        child
    }

    /// End a stand-in a test is done with.
    fn end(mut child: Child) {
        let _ = child.kill();
        let _ = child.wait();
    }

    /// The replacement rule, case by case.
    ///
    /// Why: this decides whether startup stops a running supervisor, and so
    /// whether the user's sessions lose their supervisor for a moment, or a
    /// supervisor that is not the app's is ever touched. Spec: without the
    /// app's own record, an answering supervisor is reused and absence
    /// starts one (never a replacement); the app's own supervisor is
    /// replaced when it runs another program, answering or not, and when it
    /// does not answer at all, after a wait whether its hello fails or
    /// nothing listens (it may still be starting, before or after binding
    /// its socket); it is reused when it answers and runs the current
    /// program. A failed discovery without a record of
    /// the app's own never reaches the planner; it maps to the plan that
    /// touches nothing.
    #[farhelm_testtrace::test]
    fn the_app_replaces_only_its_own_supervisor_and_only_when_it_must() {
        use Discovered::{Absent, Answering, Failed};
        use SupervisorPlan::{Replace, Reuse, Spawn};
        let current = Some(((), true));
        let other = Some(((), false));
        let replace = |wait_first| Replace {
            own: (),
            wait_first,
        };
        for (own, found, expected) in [
            (None, Answering, Reuse),
            (None, Absent, Spawn),
            (None, Failed, Reuse),
            (current, Answering, Reuse),
            (current, Absent, replace(true)),
            (current, Failed, replace(true)),
            (other, Answering, replace(false)),
            (other, Absent, replace(false)),
            (other, Failed, replace(false)),
        ] {
            assert_eq!(
                plan_local_supervisor(own, found),
                expected,
                "own {own:?}, found {found:?}"
            );
        }
    }

    /// The record of a live `sleep` started as a stand-in for the app's own
    /// supervisor, with the child to end afterwards.
    fn own_stand_in() -> (Child, ManagedSupervisor) {
        let program = ProgramFile::read(Path::new("/bin/sleep")).expect("fixture: /bin/sleep");
        let child = Command::new(&program.path)
            .arg("60")
            .stdin(Stdio::null())
            .spawn()
            .expect("fixture: spawning the stand-in");
        let record = ManagedSupervisor::for_process(child.id(), &program)
            .expect("fixture: the stand-in is alive");
        (child, record)
    }

    /// Waiting for the app's own silent supervisor ends as soon as it
    /// answers, ends at once when it is gone, and otherwise ends at the
    /// deadline.
    ///
    /// Why: a supervisor that is merely still starting (waiting for the
    /// state directory, or migrating its database after an update) must not
    /// be stopped and possibly killed; but a gone or truly wedged one must
    /// not hold startup longer than the wait. Spec: a probe that answers on
    /// its third try ends the wait as answered; a stand-in that has exited
    /// ends it as gone without waiting; a probe that never answers (or
    /// errors) ends it as timed out, after the wait; a probe slower than the
    /// whole wait is cut off, so the wait ends near its deadline.
    #[farhelm_testtrace::test]
    fn waiting_for_the_apps_own_silent_supervisor_ends_on_answer_exit_or_deadline() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let (child, own) = own_stand_in();
        let probes = std::cell::Cell::new(0);
        let outcome = runtime.block_on(await_own_supervisor_answering(
            &own,
            Duration::from_secs(60),
            || {
                probes.set(probes.get() + 1);
                let answered = probes.get() >= 3;
                async move { Ok(answered) }
            },
        ));
        assert_eq!(outcome, OwnSupervisorWait::Answered);
        assert_eq!(probes.get(), 3);

        let started = std::time::Instant::now();
        let outcome = runtime.block_on(await_own_supervisor_answering(
            &own,
            Duration::from_millis(300),
            || async { Err(anyhow::anyhow!("hello timed out")) },
        ));
        assert_eq!(outcome, OwnSupervisorWait::TimedOut);
        assert!(started.elapsed() >= Duration::from_millis(300));

        // A probe that never finishes is cut off at the deadline rather than
        // stretching the wait.
        let started = std::time::Instant::now();
        let outcome = runtime.block_on(await_own_supervisor_answering(
            &own,
            Duration::from_millis(300),
            std::future::pending::<anyhow::Result<bool>>,
        ));
        assert_eq!(outcome, OwnSupervisorWait::TimedOut);
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "a slow probe stretched the wait to {:?}",
            started.elapsed()
        );

        end(child);
        let outcome = runtime.block_on(await_own_supervisor_answering(
            &own,
            Duration::from_secs(60),
            || async { Ok(false) },
        ));
        assert_eq!(outcome, OwnSupervisorWait::Gone);
    }

    /// The launch lock refuses while an open app (or another launch) holds
    /// it, waits out a brief holder, and is handed back when free.
    ///
    /// Why: every launch decides about its supervisor under this lock; a
    /// launch beside an open app must refuse before it reads or stops
    /// anything, and a CLI that holds the lock for a moment must not make a
    /// launch refuse. Spec: with `helm-token.lock` held for longer than the
    /// wait, `refuse_if_app_open` refuses naming token control and saying
    /// nothing was changed; a holder that lets go partway through the wait
    /// is waited out; without a holder it returns the held lock, which
    /// blocks a third party until dropped.
    #[farhelm_testtrace::test]
    fn the_launch_lock_refuses_beside_an_open_app_and_waits_out_a_brief_holder() {
        let state = tempfile::tempdir().expect("state");
        let wait = Duration::from_millis(300);
        let held = refuse_if_app_open(state.path(), wait).expect("a free lock is taken");
        let error = refuse_if_app_open(state.path(), wait).expect_err("a held lock refuses");
        let text = format!("{error:#}");
        assert!(
            text.contains("token control") && text.contains("nothing was changed"),
            "{text}"
        );
        let (released_tx, released_rx) = mpsc::channel();
        let holder = std::thread::spawn(move || {
            // sleep-ok: the brief holder's hold is the behavior under test.
            std::thread::sleep(Duration::from_millis(200));
            // Publish while still holding the lock: acquiring it must imply
            // that this signal is already observable, regardless of scheduling.
            released_tx.send(()).unwrap();
            drop(held);
        });
        refuse_if_app_open(state.path(), Duration::from_secs(10))
            .expect("a holder that lets go within the wait is waited out");
        released_rx
            .try_recv()
            .expect("the lock was taken only after the holder let go");
        holder.join().unwrap();
    }

    /// The replacement's check that its program runs at all.
    ///
    /// Why: a program file that cannot run looks to discovery like a
    /// predecessor that does not answer, so without this check a launch
    /// would stop a working supervisor and then fail to start one. Spec:
    /// a program whose `--version` succeeds passes; one that exits non-zero
    /// and one that is not executable refuse, naming the program.
    #[farhelm_testtrace::test]
    fn a_replacement_refuses_a_program_that_does_not_run() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().expect("dir");
        // Own the program fixtures: system utility paths differ between
        // macOS and Linux, and this check cares only whether --version succeeds.
        let runnable = dir.path().join("runnable");
        let failing = dir.path().join("failing");
        for (path, script) in [
            (&runnable, "#!/bin/sh\nexit 0\n"),
            (&failing, "#!/bin/sh\nexit 1\n"),
        ] {
            std::fs::write(path, script).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let program = ProgramFile::read(&runnable).expect("runnable fixture");
        let not_executable = dir.path().join("farhelm");
        std::fs::write(&not_executable, b"#!/bin/sh\nexit 0\n").unwrap();
        let state = dir.path().join("state");
        let check = |farhelm: &Path| {
            SpawnInputs {
                farhelm,
                absolute: farhelm,
                program: &program,
                state_dir: &state,
            }
            .check_program_runs()
        };
        check(&runnable).expect("a program that runs passes");
        let error = check(&failing).expect_err("a failing program refuses");
        assert!(
            format!("{error:#}").contains(&failing.display().to_string()),
            "{error:#}"
        );
        let error = check(&not_executable).expect_err("a non-executable file refuses");
        assert!(
            format!("{error:#}").contains(&not_executable.display().to_string()),
            "{error:#}"
        );
    }

    /// A discovery that fails outright means "not answering" only for the
    /// app's own supervisor, and stays fatal otherwise.
    ///
    /// Why: the app's own supervisor can be wedged in a way that keeps its
    /// socket accepting while the hello never comes back, and refusing to
    /// start then would leave the user stuck with it until they found and
    /// killed it by hand; but for a supervisor the app did not start, the
    /// same failure says nothing about who holds the state directory, and
    /// starting a rival there must not happen. Spec: an answer and an
    /// absence pass through as they are; a failure becomes "not answering"
    /// with a matching record of the app's own, and stays an error without.
    #[farhelm_testtrace::test]
    fn a_failed_discovery_counts_as_not_answering_only_for_the_apps_own_supervisor() {
        use farhelm_helm::LocalSupervisorDiscovery::{Absent, Answering};
        for own in [false, true] {
            assert_eq!(
                discovered(Ok(Answering), own).unwrap(),
                Discovered::Answering
            );
            assert_eq!(discovered(Ok(Absent), own).unwrap(), Discovered::Absent);
        }
        assert_eq!(
            discovered(Err(anyhow::anyhow!("hello timed out")), true).unwrap(),
            Discovered::Failed
        );
        assert!(discovered(Err(anyhow::anyhow!("hello timed out")), false).is_err());
    }

    /// The supervisor's log is appended to across starts, and moved aside
    /// once it reaches the rotation size.
    ///
    /// Why: the detached supervisor's diagnostics now live only in this
    /// file, so a restart must not lose the previous run's lines, and a log
    /// that grows across many starts must stay bounded. Spec: a small log is
    /// appended to; a log at the rotation size becomes `supervisor.log.1`
    /// (replacing an older one) and a fresh log starts; the log is private.
    #[farhelm_testtrace::test]
    fn the_supervisor_log_is_appended_to_and_rotated_at_its_size() {
        use std::os::unix::fs::PermissionsExt as _;
        let state = tempfile::tempdir().expect("state");
        let log = state.path().join(SUPERVISOR_LOG);
        writeln!(open_supervisor_log(state.path()).unwrap(), "first run").unwrap();
        writeln!(open_supervisor_log(state.path()).unwrap(), "second run").unwrap();
        assert_eq!(
            std::fs::read_to_string(&log).unwrap(),
            "first run\nsecond run\n"
        );
        assert_eq!(
            std::fs::metadata(&log).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // Below the size, the kept handle is used as it is.
        let kept = rotated_supervisor_log(state.path(), open_supervisor_log(state.path()).unwrap());
        drop(kept);
        assert!(!state.path().join(SUPERVISOR_LOG_PREVIOUS).exists());
        // At the size, the log moves aside (replacing an older one) and a
        // fresh one is used.
        std::fs::write(state.path().join(SUPERVISOR_LOG_PREVIOUS), b"older").unwrap();
        File::options()
            .append(true)
            .open(&log)
            .unwrap()
            .set_len(SUPERVISOR_LOG_ROTATE_AT)
            .unwrap();
        let mut fresh =
            rotated_supervisor_log(state.path(), open_supervisor_log(state.path()).unwrap());
        writeln!(fresh, "third run").unwrap();
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "third run\n");
        assert_eq!(
            std::fs::metadata(state.path().join(SUPERVISOR_LOG_PREVIOUS))
                .unwrap()
                .len(),
            SUPERVISOR_LOG_ROTATE_AT,
            "the full log replaced the older one"
        );
    }

    /// A rotation that fails falls back to the kept handle instead of
    /// refusing the start.
    ///
    /// Why: rotation runs after a predecessor may already have been
    /// stopped; refusing then would leave the user with no supervisor at
    /// all. Spec: with the log at the rotation size and a directory where
    /// the previous log would go, the kept handle is returned and still
    /// appends to `supervisor.log`, which keeps its earlier content (it is
    /// neither truncated nor replaced), and the directory is left alone.
    #[farhelm_testtrace::test]
    fn a_failed_rotation_appends_to_the_kept_log() {
        let state = tempfile::tempdir().expect("state");
        let log = state.path().join(SUPERVISOR_LOG);
        std::fs::create_dir(state.path().join(SUPERVISOR_LOG_PREVIOUS)).unwrap();
        std::fs::write(&log, b"x").unwrap();
        File::options()
            .append(true)
            .open(&log)
            .unwrap()
            .set_len(SUPERVISOR_LOG_ROTATE_AT)
            .unwrap();
        let mut kept =
            rotated_supervisor_log(state.path(), open_supervisor_log(state.path()).unwrap());
        writeln!(kept, "still here").unwrap();
        let tail = b"still here\n";
        assert_eq!(
            std::fs::metadata(&log).unwrap().len(),
            SUPERVISOR_LOG_ROTATE_AT + tail.len() as u64,
            "the kept log was appended to, not truncated or replaced"
        );
        use std::io::Seek as _;
        let mut end = vec![0u8; tail.len()];
        let mut file = File::open(&log).unwrap();
        file.seek(std::io::SeekFrom::End(-(tail.len() as i64)))
            .unwrap();
        file.read_exact(&mut end).unwrap();
        assert_eq!(end, tail);
        assert!(state.path().join(SUPERVISOR_LOG_PREVIOUS).is_dir());
    }

    /// Startup must hand over to the embedded helm as soon as the child it
    /// spawned answers, so the helm's first connection attempt is the one
    /// that succeeds. Pinned: the wait keeps probing while the child does not
    /// answer and returns on the first answer, with the child still running.
    #[farhelm_testtrace::test]
    fn waiting_for_the_managed_supervisor_returns_once_it_answers() {
        let mut child = stand_in_child("exec sleep 60");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let probes = std::cell::Cell::new(0);
        runtime
            .block_on(await_managed_supervisor_serving(
                &mut child,
                Duration::from_secs(60),
                || {
                    probes.set(probes.get() + 1);
                    let answered = probes.get() >= 3;
                    async move { Ok(answered) }
                },
            ))
            .expect("an answering supervisor ends the wait");
        assert_eq!(probes.get(), 3, "the wait ends on the first answer");
        assert!(
            child.try_wait().expect("poll the stand-in").is_none(),
            "the child is still running"
        );
        end(child);
    }

    /// A child that dies while startup waits for it must be reported as the
    /// failure it is, not waited out to the deadline. Pinned: the error names
    /// the child's exit while nothing ever answers.
    #[farhelm_testtrace::test]
    fn waiting_for_the_managed_supervisor_reports_its_exit() {
        let mut child = Command::new("sh")
            .args(["-c", "exit 3"])
            .stdin(Stdio::null())
            .spawn()
            .expect("spawn the stand-in supervisor");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let error = runtime
            .block_on(await_managed_supervisor_serving(
                &mut child,
                Duration::from_secs(60),
                || async { Ok(false) },
            ))
            .expect_err("an exited child must fail startup");
        assert!(
            format!("{error:#}").contains("exited during desktop startup"),
            "{error:#}"
        );
    }

    /// A child that neither serves nor exits must not hold startup forever.
    /// Pinned: the wait ends with an error that says it never started
    /// serving.
    #[farhelm_testtrace::test]
    fn waiting_for_the_managed_supervisor_gives_up_after_the_wait() {
        let mut child = stand_in_child("exec sleep 60");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let error = runtime
            .block_on(await_managed_supervisor_serving(
                &mut child,
                Duration::from_millis(200),
                || async { Ok(false) },
            ))
            .expect_err("a child that never serves must fail startup");
        assert!(
            format!("{error:#}").contains("did not start serving"),
            "{error:#}"
        );
        end(child);
    }

    /// A helm stop requester dropped without `request` still marks the stop
    /// as expected, before its sender closes; `request` marks it and sends.
    ///
    /// Why: GPUI can drop its quit handlers uncalled, and the dropped sender
    /// stops the helm. Without the flag the helm's monitor would show the
    /// "embedded helm stopped unexpectedly" refusal on an ordinary quit.
    /// Spec: after either path the flag is set and the helm's receiver sees
    /// its stop (a closed channel for the drop, a value for `request`).
    #[farhelm_testtrace::test]
    fn a_dropped_helm_stop_requester_still_expects_the_stop() {
        let expected = Arc::new(AtomicBool::new(false));
        let (sender, mut receiver) = tokio::sync::oneshot::channel();
        drop(HelmShutdown {
            shutdown: Some(sender),
            expected: Arc::clone(&expected),
        });
        assert!(
            expected.load(Ordering::Acquire),
            "a dropped requester expects the stop"
        );
        assert_eq!(
            receiver.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Closed),
            "the dropped sender stops the helm"
        );

        let expected = Arc::new(AtomicBool::new(false));
        let (sender, mut receiver) = tokio::sync::oneshot::channel();
        HelmShutdown {
            shutdown: Some(sender),
            expected: Arc::clone(&expected),
        }
        .request();
        assert!(
            expected.load(Ordering::Acquire),
            "a requested stop is expected"
        );
        assert_eq!(receiver.try_recv(), Ok(()), "request sends the stop");
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
                "tests::desktop_loopback_client_ignores_an_ambient_proxy",
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
