//! The installed Mac app's updater: it asks get.farhelm.io for the latest
//! stable release and, when that release is newer, verifies the release's
//! signed checksums and its installer, then runs that installer in the
//! background, and publishes what it is doing for the app bar to show.
//!
//! SPEC.md "Installation and updates" is the behavior; SPEC_impl.md "The
//! desktop app's updater" is the design and its reasons. In short:
//!
//! - It runs only in an app that `install.sh` installed and can update
//!   ([`active_bundle`]). Anywhere else nothing starts, which is also what
//!   keeps the desktop smoke test and Linux CI off the network.
//! - It has no part in the helm. The requests are this process's own reqwest
//!   calls, never the webview's, and the state reaches the component tree
//!   through the Dioxus context.
//! - One worker thread ([`Engine`]) owns every check and install, so they are
//!   single-flight across the automatic schedule and every on-demand trigger.
//! - Success is judged by the Installed record the installer writes, never by
//!   the installer's exit status.
//! - Nothing it runs is trusted on TLS alone: the installer runs only once
//!   the release's signed `SHA256SUMS` verifies against the compiled-in key
//!   ring and the script matches its entry there ([`install_release`]), and
//!   the script is handed those verified checksums for every archive it
//!   downloads. A release that fails that check leaves a notice that
//!   this app must be reinstalled ([`UpdaterState::needs_reinstall`]).
//!
//! The network, the installer, the clock and the setting are all reached
//! through [`Deps`], plain functions the worker is handed, so the decisions
//! above are tested on any platform without a network or a real bundle.

use super::*;

use std::ffi::OsString;
use std::sync::Condvar;
use std::time::SystemTime;

use farhelm_helm::{build_is_newer, is_development_build};
use sha2::Digest as _;

/// The one origin every release is published on, in the layout SPEC_impl.md
/// "Verification chain (D3)" fixes: `/latest`, and each release under
/// `/v<version>/`.
const SITE_URL: &str = "https://get.farhelm.io";

/// Release tests can select a stable candidate before `/latest` names it.
/// Only the version selection changes; the download origin and verification
/// chain remain the same as an ordinary update.
const LATEST_OVERRIDE_ENV: &str = "FARHELM_DESKTOP_UPDATE_LATEST";

/// The command the reinstall notice tells the user to run: the installer
/// over TLS, which needs no key, so it works however far behind this app is.
const REINSTALL_COMMAND: &str = "curl -fsSL https://get.farhelm.io/install.sh | sh";

/// The largest `/latest` answer accepted: one tag and a newline.
const LATEST_MAX_BYTES: usize = 256;

/// The largest `SHA256SUMS` accepted; the helm's own download allows the same.
const SUMS_MAX_BYTES: usize = 64 * 1024;

/// The largest signature accepted; a real one is a few hundred bytes.
const SIGNATURE_MAX_BYTES: usize = 4 * 1024;

/// How often the worker wakes when nothing asks it to: often enough that an
/// update installed from a terminal shows up within about a minute, and
/// cheap, since a wake that is not due for a check only reads one small file.
const TICK: Duration = Duration::from_secs(60);

/// Wall-clock time between automatic checks.
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// A failed automatic probe becomes visible only after this failure streak,
/// rather than after an idle gap in which no probe actually failed.
const PROBE_FAILURE_NOTICE_AFTER: Duration = Duration::from_secs(3 * 24 * 60 * 60);

/// Bound on the latest-release probe, which is one small request.
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// Bound on each of the install step's downloads (the checksums, their
/// signature, the installer script), each a few kilobytes to tens.
const INSTALLER_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

/// The largest installer script accepted. The real one is far smaller; the
/// cap only keeps a misbehaving server from filling memory.
const INSTALLER_MAX_BYTES: usize = 1024 * 1024;

/// The environment variable that pins the installer to one release.
const VERSION_PIN_ENV: &str = "FARHELM_VERSION";

/// The environment variable that hands the installer the verified
/// `SHA256SUMS`, so it checks every archive against signed hashes and fetches
/// no checksums of its own. With [`VERSION_PIN_ENV`] it is the installer's
/// whole interface to this updater, a permanent contract with every future
/// release's `install.sh` (SPEC_impl.md, "The desktop app's updater").
const SUMS_FILE_ENV: &str = "FARHELM_INSTALL_SUMS_FILE";

// ===== What the updater publishes ==========================================

use crate::app_updater::Activity;

/// The updater's whole published state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpdaterState {
    /// The version compiled into this app.
    pub(crate) running: String,
    /// The version `Contents/Versions/installed` names, when it could be
    /// read.
    pub(crate) installed: Option<String>,
    pub(crate) activity: Activity,
    /// Identity of the last visible check or qualifying automatic failure,
    /// retained across watch coalescing. Record refreshes and nonqualifying
    /// automatic work cannot make an old outcome new again.
    pub(crate) notice_id: u64,
    /// A successful user-visible check ended with an update waiting. This is
    /// announcement provenance, not the source of truth for the red readout.
    pub(crate) user_update_ready: bool,
    /// A release this app tried to install failed verification (no key in
    /// its ring verified the signature, the signature was for another
    /// version, or the installer did not match its signed checksum), so the
    /// app cannot update itself and must be reinstalled. Shown whether the
    /// check was automatic or the user's, and kept while this app runs until
    /// a later check verifies and installs or finds nothing newer, or a
    /// newer Farhelm is installed some other way. It lives in memory only:
    /// a relaunched app shows it again once its first check fails the same
    /// way (at startup, with automatic updates on). Network and HTTP
    /// failures never set it.
    pub(crate) needs_reinstall: bool,
}

impl UpdaterState {
    /// Whether a newer version is installed than the one running, so that a
    /// restart finishes an update. Not a state of its own: it holds however
    /// that version was installed, and whatever the updater is doing.
    pub(crate) fn update_ready(&self) -> bool {
        newer_installed(self.installed.as_deref(), &self.running)
    }
}

/// Whether `installed` names a newer version than `running`: the one rule,
/// shared by what the readout shows and what the worker decides.
fn newer_installed(installed: Option<&str>, running: &str) -> bool {
    installed.is_some_and(|installed| build_is_newer(installed, running))
}

// ===== Activation ===========================================================

/// The bundle's `Contents` directory when this app should run an updater,
/// `None` when it should not.
///
/// All three conditions come from SPEC_impl.md: the running program is the
/// main program of `<home>/Applications/Farhelm.app` (both sides
/// canonicalized, so a symlinked path to the same bundle still counts and a
/// copy elsewhere does not), that bundle has `Contents/Versions/` (the
/// side-by-side layout the installer updates in place), and `version` is a
/// release. `install.sh` only ever writes that one bundle, so an app running
/// from anywhere else would install every day and never see its own
/// Installed record change; and a development build has no place in the
/// release order to compare from.
pub(super) fn active_bundle(current_exe: &Path, home: &Path, version: &str) -> Option<PathBuf> {
    if is_development_build(version) || semver::Version::parse(version).is_err() {
        return None;
    }
    let exe = std::fs::canonicalize(current_exe).ok()?;
    let contents = farhelm_supervisor::app_bundle::bundle_contents_of_main_program(&exe)?;
    let expected = std::fs::canonicalize(home.join("Applications/Farhelm.app/Contents")).ok()?;
    (contents == expected && contents.join("Versions").is_dir()).then_some(contents)
}

/// The component tree's handle on a running updater: what it publishes,
/// the way to ask for a check, the automatic-updates setting, and the
/// restart into an installed update. Present in the Dioxus context only
/// when [`start`] started one, which is how every surface knows whether to
/// offer anything update-related at all.
#[derive(Clone)]
pub(crate) struct UpdaterHandle {
    shared: Arc<Shared>,
    /// The `Farhelm.app` bundle a restart opens.
    bundle: PathBuf,
    /// The desktop state file holding the automatic-updates setting.
    state_path: PathBuf,
}

impl PartialEq for UpdaterHandle {
    /// One updater per process, so identity is the only equality there is.
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }
}

impl UpdaterHandle {
    /// Ask for a check now (see [`Shared::check_now`]).
    pub(crate) fn check_now(&self) {
        self.shared.check_now();
    }

    /// A new subscription to the published state.
    pub(crate) fn subscribe(&self) -> tokio::sync::watch::Receiver<UpdaterState> {
        self.shared.subscribe()
    }

    /// Whether automatic updates are on, as the settings dialog shows it.
    pub(crate) fn automatic_updates(&self) -> bool {
        automatic_updates_enabled(&self.state_path)
    }

    /// Turn automatic updates on or off. The worker reads the setting
    /// before every automatic check and again before an automatic install.
    /// Re-enabling also clears the probe-failure streak in memory and disk,
    /// even if no worker wake observed the off period. A failed write keeps
    /// both the saved choice and in-memory history unchanged and returns a
    /// short reason, so the dialog can keep showing the
    /// choice that is actually in force rather than one that was not saved.
    pub(crate) fn set_automatic_updates(&self, on: bool) -> Result<(), String> {
        // The worker uses the same status-before-file lock order. Hold it
        // through the saved choice so an in-flight failure cannot write an
        // old streak back over a successful re-enable.
        let mut status = self.shared.status.lock().expect("updater status poisoned");
        let mut reenabled = false;
        state::update_state(&self.state_path, |state| {
            reenabled = on && !state.install_updates_automatically.unwrap_or(true);
            state.install_updates_automatically = Some(on);
            if reenabled {
                state.automatic_probe_failure_since = None;
            }
        })
        .map(|_| {
            if reenabled {
                status.probe_failure_since = None;
                status.last_probe_clock = None;
            }
        })
        .map_err(|error| {
            tracing::warn!("updater: saving the automatic-updates setting: {error:#}");
            "the choice could not be saved; the app's log has the reason".to_string()
        })
    }

    /// Start the helper that reopens the app once this process has exited,
    /// for Restart to update. `true` means the caller should now quit; on
    /// `false` the failure is already shown in the readout's hover, and the
    /// app must stay open, since quitting would leave it closed.
    pub(crate) fn start_relaunch(&self) -> bool {
        match spawn_relaunch_helper(
            std::process::id(),
            &self.bundle,
            Path::new(RELAUNCH_OPENER),
            RELAUNCH_WAIT_TENTHS,
            None,
        ) {
            // Never waited for: the app is about to exit, and the helper
            // runs on in its own process group.
            Ok(_helper) => true,
            Err(error) => {
                tracing::warn!("updater: starting the relaunch helper: {error}");
                self.shared.show_outcome(Activity::RestartFailed);
                false
            }
        }
    }
}

// ===== Restart to update ====================================================

/// The program that opens the bundle on macOS, the same way the Dock or
/// Finder would.
const RELAUNCH_OPENER: &str = "/usr/bin/open";

/// How long the helper waits for this process to exit, in tenths of a
/// second: about a minute, far longer than a quit takes, so only an app that
/// is stuck makes the helper give up.
const RELAUNCH_WAIT_TENTHS: u32 = 600;

/// The helper's script: wait for the process `$1` to exit, bounded at `$4`
/// tenths of a second, then run the opener `$3` on the bundle `$2` with
/// `-n`, which asks for a new instance: macOS can briefly keep listing an
/// app as running after its process has exited, and a plain `open` would
/// then only try to bring that old instance forward. If the
/// process is still there at the bound, give up without opening anything:
/// opening a bundle whose app is still running only brings that app to the
/// front, and an app that has not quit in a minute is not one to relaunch
/// over.
///
/// `$5`, empty in the app, names a file the helper creates each time it has
/// seen the process alive. It is the tests' readiness signal: they release
/// their stand-in app only once the helper is provably waiting on it.
const RELAUNCH_SCRIPT: &str = r#"pid="$1"; app="$2"; opener="$3"; tenths="$4"; seen="$5"; waited=0
while kill -0 "$pid" 2>/dev/null; do
  [ -n "$seen" ] && : > "$seen"
  [ "$waited" -ge "$tenths" ] && exit 0
  sleep 0.1
  waited=$((waited + 1))
done
exec "$opener" -n "$app""#;

/// Spawn the relaunch helper, detached in its own process group so the
/// app's exit does not take it along, with every standard stream closed.
///
/// The app never waits for the returned child; tests do, to observe what
/// the helper did. The release-test version override is removed before
/// spawning, so the opener does not inherit it for the next app instance.
fn spawn_relaunch_helper(
    pid: u32,
    bundle: &Path,
    opener: &Path,
    wait_tenths: u32,
    seen_alive: Option<&Path>,
) -> io::Result<Child> {
    relaunch_command(pid, bundle, opener, wait_tenths, seen_alive).spawn()
}

/// Configure the detached opener without carrying a release-test candidate
/// into the next app instance. Keeping construction separate makes this
/// environment contract inspectable without mutating the test environment.
fn relaunch_command(
    pid: u32,
    bundle: &Path,
    opener: &Path,
    wait_tenths: u32,
    seen_alive: Option<&Path>,
) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .env_remove(LATEST_OVERRIDE_ENV)
        .arg("-c")
        .arg(RELAUNCH_SCRIPT)
        .arg("farhelm-relaunch")
        .arg(pid.to_string())
        .arg(bundle)
        .arg(opener)
        .arg(wait_tenths.to_string())
        .arg(seen_alive.unwrap_or(Path::new("")))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}

/// Start the updater for this app, or return `None` when [`active_bundle`]
/// says it should not run.
///
/// `state_path` is the desktop state file, read on every wake for the
/// automatic-updates setting. The worker thread lives as long as the
/// process; quitting simply ends it, and an installer it started keeps
/// running to completion on its own, which the installer is built to
/// survive (SPEC.md: an update stopped at any point leaves a launchable
/// app).
pub(super) fn start(state_path: PathBuf) -> Option<UpdaterHandle> {
    let running = env!("CARGO_PKG_VERSION");
    let current_exe = std::env::current_exe().ok()?;
    let home = std::env::var_os("HOME")?;
    let contents = active_bundle(&current_exe, Path::new(&home), running)?;
    let installed_record = contents.join("Versions").join("installed");
    let handle_state_path = state_path.clone();
    // Capture once: automatic and on-demand checks must test the same
    // candidate for this app's lifetime, including when the value is invalid.
    let latest_override = std::env::var_os(LATEST_OVERRIDE_ENV);
    if latest_override.is_some() {
        tracing::info!("updater: {LATEST_OVERRIDE_ENV} override is active");
    }
    let deps = Deps {
        probe: probe_with_latest_override(latest_override, Box::new(probe_latest_release)),
        install: Box::new(run_installer),
        read_installed: Box::new(move || read_installed_record(&installed_record)),
        automatic_enabled: Box::new({
            let path = state_path.clone();
            move || automatic_updates_enabled(&path)
        }),
        read_probe_failure_since: Box::new({
            let path = state_path.clone();
            move || read_probe_failure_since(&path)
        }),
        write_probe_failure_since: Box::new(move |since| {
            state::update_state(&state_path, |state| {
                state.automatic_probe_failure_since = since;
            })
            .map(|_| ())
        }),
        now: Box::new(SystemTime::now),
    };
    let shared = Arc::new(Shared::new(running));
    let engine = Engine::new(deps, Arc::clone(&shared));
    let bundle = contents.parent()?.to_path_buf();
    match std::thread::Builder::new()
        .name("farhelm-updater".to_string())
        .spawn(move || engine.run_forever())
    {
        Ok(_) => {
            tracing::info!("updater: active for {}", bundle.display());
            Some(UpdaterHandle {
                shared,
                bundle,
                state_path: handle_state_path,
            })
        }
        Err(error) => {
            tracing::warn!("could not start the updater: {error}");
            None
        }
    }
}

/// Whether automatic updates are on: the desktop state file's setting, on
/// when it is absent and when the file cannot be read, since a broken file
/// is no answer from the user and the setting's default is on.
fn automatic_updates_enabled(state_path: &Path) -> bool {
    match state::read_state(state_path) {
        Ok(state) => state.install_updates_automatically.unwrap_or(true),
        Err(error) => {
            tracing::warn!("updater: reading the automatic-updates setting: {error:#}");
            true
        }
    }
}

/// Read once at worker startup; a broken file provides no failure history.
/// Subsequent writes are best effort, with memory remaining authoritative.
fn read_probe_failure_since(path: &Path) -> Option<SystemTime> {
    match state::read_state(path) {
        Ok(state) => state.automatic_probe_failure_since,
        Err(error) => {
            tracing::warn!("updater: reading the probe failure streak: {error:#}");
            None
        }
    }
}

/// The version an Installed record names: its first line, trimmed, or
/// `None` when the file is missing, unreadable or blank.
fn read_installed_record(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let version = text.lines().next()?.trim();
    (!version.is_empty()).then(|| version.to_string())
}

// ===== The worker ===========================================================

/// Runs the installer pinned to the version it is given.
type InstallFn = dyn Fn(&str) -> anyhow::Result<()> + Send;

/// Answers the latest stable release's version.
type ProbeFn = dyn Fn() -> anyhow::Result<String> + Send;

/// Everything the worker reaches outside itself, as plain functions so a
/// test can stand in for each.
pub(super) struct Deps {
    /// The latest stable release's version.
    pub(super) probe: Box<ProbeFn>,
    /// Run the installer pinned to this version. Its result is logged; the
    /// Installed record decides whether the install happened.
    pub(super) install: Box<InstallFn>,
    /// The version the Installed record names now.
    pub(super) read_installed: Box<dyn Fn() -> Option<String> + Send>,
    /// Whether automatic updates are on now.
    pub(super) automatic_enabled: Box<dyn Fn() -> bool + Send>,
    /// Optional persisted start, read once. File failures mean no known streak.
    pub(super) read_probe_failure_since: Box<dyn Fn() -> Option<SystemTime> + Send>,
    /// Best-effort storage, serialized under the shared status lock so the
    /// saved setting's re-enable cannot race a stale streak write.
    pub(super) write_probe_failure_since:
        Box<dyn Fn(Option<SystemTime>) -> anyhow::Result<()> + Send>,
    /// Wall-clock now.
    pub(super) now: Box<dyn Fn() -> SystemTime + Send>,
}

// ===== The version readout ==================================================

/// How the sidebar's version readout presents the updater's state.
///
/// Pure, so every state's look and words are tested without a component
/// tree. `update_ready` turns the readout red with an up-arrow (SPEC.md
/// "Installation and updates"). Activity also drives the inline progress and
/// outcome, whose expiry belongs to the component; the hover wording persists.
pub(crate) fn readout(state: &UpdaterState) -> crate::app_updater::Readout {
    let readout = activity_readout(state);
    if !state.needs_reinstall {
        return readout;
    }
    // Shown whatever the updater is doing: this app can no longer update
    // itself, and nothing else on screen would say so.
    crate::app_updater::Readout {
        needs_reinstall: true,
        // The notice leads, and the command goes last with nothing after
        // it, because the user retypes it from a hover: trailing
        // punctuation would read as part of it.
        tooltip: format!(
            "This Farhelm can no longer verify its updates, so it cannot update itself. {} \
             Reinstall it by running this in a terminal: {REINSTALL_COMMAND}",
            readout.tooltip
        ),
        ..readout
    }
}

/// Activity wording preserves a waiting update even after a failed check.
///
/// Checking and installing describe current work; an idle or up-to-date answer
/// yields to the installed-version restart advice. The reinstall notice wraps
/// this result without losing that activity or changing the readiness marker.
fn activity_readout(state: &UpdaterState) -> crate::app_updater::Readout {
    let ready = state.update_ready();
    let installed = state.installed.as_deref().unwrap_or_default();
    let ready_text = format!(
        "Farhelm {installed} is installed. Restart Farhelm to finish the update. Select this to open the menu for Restart to update or What's new."
    );
    let tooltip = match &state.activity {
        Activity::Checking => "Farhelm is checking for a newer version.".to_string(),
        Activity::Installing(version) => {
            format!("Farhelm {version} is being installed in the background.")
        }
        Activity::Failed(reason) if ready => {
            format!(
                "The update check failed: {}. {ready_text}",
                reason.trim_end_matches('.')
            )
        }
        Activity::Failed(reason) => {
            format!("The update check failed: {}.", reason.trim_end_matches('.'))
        }
        Activity::RestartFailed => format!(
            "Farhelm could not restart itself. Quit and reopen it to finish updating to Farhelm {installed}."
        ),
        _ if ready => ready_text,
        Activity::UpToDate(latest) => {
            format!("Farhelm is up to date. Farhelm {latest} is the latest release.")
        }
        Activity::Idle => crate::app_updater::idle_tooltip(&state.running, true),
    };
    crate::app_updater::Readout {
        update_ready: ready,
        needs_reinstall: false,
        tooltip,
        activity: state.activity.clone(),
        notice_id: state.notice_id,
        user_update_ready: state.user_update_ready,
    }
}

/// The state the worker and the on-demand triggers share.
pub(super) struct Shared {
    status: Mutex<Status>,
    /// Wakes the worker for an on-demand request.
    wake: Condvar,
    /// What the app bar reads. Only [`Shared::publish`] writes it, under the
    /// status lock, so subscribers never see two writers race.
    published: tokio::sync::watch::Sender<UpdaterState>,
}

/// The worker's bookkeeping, behind [`Shared::status`].
struct Status {
    running: String,
    installed: Option<String>,
    /// What the worker is doing now, published or not.
    current: Activity,
    /// The last visible outcome, published while no user-joined run is
    /// visible. Automatic failures survive until success or UI dismissal.
    shown_outcome: Option<Activity>,
    /// An automatic-origin failure gives its outcome a lifetime across later
    /// unsuccessful runs, including a manual result that takes its place.
    /// UI dismissal is still keyed by the published identity, not this flag.
    retain_failure: bool,
    /// The probe streak is memory-authoritative; its optional disk record only
    /// carries the start across launches. The last sample detects backwards
    /// clock movement even when the clock remains later than the start.
    probe_failure_since: Option<SystemTime>,
    last_probe_clock: Option<SystemTime>,
    /// A user asked for a check and has not yet been shown its outcome. The
    /// run in progress (or the next one) is then visible.
    user_waiting: bool,
    /// See the published presentation bookkeeping on [`UpdaterState`].
    notice_id: u64,
    user_update_ready: bool,
    /// A user's request has not been picked up by the worker yet.
    requested: bool,
    /// See [`UpdaterState::needs_reinstall`].
    needs_reinstall: bool,
}

impl Status {
    /// Record what the Installed record names now.
    ///
    /// A change to a version newer than the running one also drops the
    /// reinstall notice: something installed a newer Farhelm since the
    /// notice went up (most likely the user, following it), and a restart
    /// into that app is the next step, which the update marker already
    /// says. Automatic checks stop while an update waits, so otherwise the
    /// notice would keep asking for a reinstall that already happened.
    fn record_installed(&mut self, installed: Option<String>) {
        if installed != self.installed {
            // The completion referred to the previous record. Retaining it
            // through loss/recovery would announce an external install as if
            // the user had just finished another check. Expiry identity stays.
            self.user_update_ready = false;
            if newer_installed(installed.as_deref(), &self.running) {
                self.needs_reinstall = false;
                // A retained failure said an update did not install. Once a
                // newer version is installed by any route (the installer run
                // from a terminal, say) it is stale, and nothing else would
                // clear it: automatic checks hold back while an update waits,
                // so it would sit beside the red update-ready readout until
                // dismissed or the app restarted. A run that fails after this
                // point still sets its own failure afterwards.
                if self.retain_failure {
                    self.shown_outcome = None;
                    self.retain_failure = false;
                }
            }
        }
        self.installed = installed;
    }
}

impl Shared {
    pub(super) fn new(running: &str) -> Self {
        let (published, _) = tokio::sync::watch::channel(UpdaterState {
            running: running.to_string(),
            installed: None,
            activity: Activity::Idle,
            notice_id: 0,
            user_update_ready: false,
            needs_reinstall: false,
        });
        Self {
            status: Mutex::new(Status {
                running: running.to_string(),
                installed: None,
                current: Activity::Idle,
                shown_outcome: None,
                retain_failure: false,
                probe_failure_since: None,
                last_probe_clock: None,
                user_waiting: false,
                notice_id: 0,
                user_update_ready: false,
                requested: false,
                needs_reinstall: false,
            }),
            wake: Condvar::new(),
            published,
        }
    }

    /// Ask for a check now, from any trigger the user has.
    ///
    /// When a run is already in progress the request joins it: from here on
    /// that run is shown (its current step at once), and its outcome is the
    /// answer. Otherwise the worker starts one. A new user-visible request gets
    /// a persistent presentation identity so subscribers can distinguish equal
    /// final answers even when watch delivery coalesces their intermediate steps.
    pub(super) fn check_now(&self) {
        let mut status = self.status.lock().expect("updater status poisoned");
        if !status.user_waiting {
            status.notice_id = status.notice_id.wrapping_add(1);
            status.user_update_ready = false;
        }
        status.user_waiting = true;
        if status.current == Activity::Idle {
            status.requested = true;
            self.wake.notify_all();
        }
        self.publish(&status);
    }

    /// A new subscription to the published state.
    pub(super) fn subscribe(&self) -> tokio::sync::watch::Receiver<UpdaterState> {
        self.published.subscribe()
    }

    /// Show an outcome the user caused outside a check (a failed Restart to
    /// update), until the next run starts.
    fn show_outcome(&self, outcome: Activity) {
        let mut status = self.status.lock().expect("updater status poisoned");
        status.shown_outcome = Some(outcome);
        status.retain_failure = false;
        status.user_update_ready = false;
        self.publish(&status);
    }

    /// Send what `status` says the app bar should show, if it changed.
    fn publish(&self, status: &Status) {
        let activity = if status.user_waiting {
            status.current.clone()
        } else {
            status.shown_outcome.clone().unwrap_or(Activity::Idle)
        };
        let next = UpdaterState {
            running: status.running.clone(),
            installed: status.installed.clone(),
            activity,
            notice_id: status.notice_id,
            user_update_ready: status.user_update_ready,
            needs_reinstall: status.needs_reinstall,
        };
        self.published.send_if_modified(|state| {
            if *state == next {
                false
            } else {
                *state = next;
                true
            }
        });
    }
}

/// The single worker that runs every check and install.
pub(super) struct Engine {
    deps: Deps,
    shared: Arc<Shared>,
    /// When the last check started, in wall-clock time. In memory only: the
    /// check right after startup covers a restart.
    last_check: Option<SystemTime>,
}

/// The probe boundary determines which automatic failures may surface. A
/// verification refusal retains its separate reinstall contract; successful
/// probes end the offline streak even if a later install fails.
enum CheckResult {
    Success(Activity),
    ProbeFailed(String),
    InstallFailed(String),
    VerificationFailed(String),
}

impl CheckResult {
    /// Presentation keeps one existing activity vocabulary for both origins.
    fn activity(&self) -> Activity {
        match self {
            Self::Success(activity) => activity.clone(),
            Self::ProbeFailed(reason)
            | Self::InstallFailed(reason)
            | Self::VerificationFailed(reason) => Activity::Failed(reason.clone()),
        }
    }
}

impl Engine {
    pub(super) fn new(deps: Deps, shared: Arc<Shared>) -> Self {
        shared
            .status
            .lock()
            .expect("updater status poisoned")
            .probe_failure_since = (deps.read_probe_failure_since)();
        Self {
            deps,
            shared,
            last_check: None,
        }
    }

    /// Wake once a [`TICK`] or on request, forever.
    fn run_forever(mut self) {
        loop {
            self.wake_once();
            let status = self.shared.status.lock().expect("updater status poisoned");
            let (_status, _) = self
                .shared
                .wake
                .wait_timeout_while(status, TICK, |status| !status.requested)
                .expect("updater status poisoned");
        }
    }

    /// One wake: re-read the Installed record, then run a check if a user
    /// asked for one or the automatic schedule is due.
    pub(super) fn wake_once(&mut self) {
        let installed = (self.deps.read_installed)();
        let requested = {
            let mut status = self.shared.status.lock().expect("updater status poisoned");
            status.record_installed(installed);
            self.shared.publish(&status);
            status.requested
        };
        if requested || self.automatic_due() {
            self.run_check();
        }
    }

    /// Whether the automatic schedule wants a check now.
    ///
    /// Never while an installed update waits for a restart (SPEC_impl.md
    /// "The desktop app's updater": it saves a download a day, and a third
    /// version could replace the folder of the one still running). A wall
    /// clock that moved backwards past the last check counts as due, so a
    /// clock correction cannot postpone checks indefinitely.
    fn automatic_due(&self) -> bool {
        let clock_due = match self.last_check {
            None => true,
            Some(last) => (self.deps.now)()
                .duration_since(last)
                .map_or(true, |elapsed| elapsed >= CHECK_INTERVAL),
        };
        // The clock first, so the setting (a file read, which warns when
        // the file is broken) is consulted once a day rather than every
        // minute.
        clock_due && !self.update_ready() && (self.deps.automatic_enabled)()
    }

    fn update_ready(&self) -> bool {
        let status = self.shared.status.lock().expect("updater status poisoned");
        newer_installed(status.installed.as_deref(), &status.running)
    }

    /// Set or clear the reinstall notice and publish it.
    fn set_needs_reinstall(&self, needs: bool) {
        let mut status = self.shared.status.lock().expect("updater status poisoned");
        status.needs_reinstall = needs;
        self.shared.publish(&status);
    }

    /// Set what the worker is doing and publish it.
    fn set_current(&self, activity: Activity) {
        let mut status = self.shared.status.lock().expect("updater status poisoned");
        status.current = activity;
        self.shared.publish(&status);
    }

    /// One check, and the install it calls for.
    fn run_check(&mut self) {
        self.last_check = Some((self.deps.now)());
        let automatic = {
            let mut status = self.shared.status.lock().expect("updater status poisoned");
            status.requested = false;
            if !status.retain_failure {
                status.shown_outcome = None;
            }
            status.user_update_ready = false;
            status.current = Activity::Checking;
            self.shared.publish(&status);
            !status.user_waiting
        };
        let outcome = self.check_and_install();
        let installed = (self.deps.read_installed)();
        let mut status = self.shared.status.lock().expect("updater status poisoned");
        let qualifies = match &outcome {
            CheckResult::ProbeFailed(_) => {
                let now = (self.deps.now)();
                let since = status.probe_failure_since.unwrap_or(now);
                let backwards = status.last_probe_clock.is_some_and(|last| now < last);
                let since = if now < since || backwards { now } else { since };
                // Only automatic failures begin a streak; a manual failure can
                // repair an existing future/backwards timestamp but cannot
                // invent automatic failures that never occurred.
                let next = if !automatic && status.probe_failure_since.is_none() {
                    None
                } else {
                    Some(since)
                };
                self.save_streak(&mut status, next);
                status.last_probe_clock = Some(now);
                now.duration_since(since)
                    .is_ok_and(|elapsed| elapsed >= PROBE_FAILURE_NOTICE_AFTER)
            }
            CheckResult::InstallFailed(_) => true,
            _ => false,
        };
        status.record_installed(installed);
        status.current = Activity::Idle;
        if matches!(outcome, CheckResult::Success(_)) {
            status.shown_outcome = None;
            status.retain_failure = false;
        }
        if status.user_waiting {
            status.user_waiting = false;
            // Readiness alone can come from an automatic or external install.
            // Capture it only with a successful check the user saw through.
            status.user_update_ready = matches!(outcome, CheckResult::Success(_))
                && newer_installed(status.installed.as_deref(), &status.running);
            // A finished install needs no words of its own: the update
            // marker shows it.
            let activity = outcome.activity();
            status.shown_outcome = (activity != Activity::Idle).then_some(activity);
        } else if let Activity::Failed(reason) = outcome.activity() {
            tracing::warn!("updater: automatic update failed: {reason}");
            if qualifies {
                // Equal later failures must be fresh notices after dismissal,
                // even when watch delivery coalesces all intermediate states.
                status.notice_id = status.notice_id.wrapping_add(1);
                status.shown_outcome = Some(Activity::Failed(reason));
                status.retain_failure = true;
            }
        }
        self.shared.publish(&status);
    }

    /// Keep memory authoritative when durable storage is unavailable. All
    /// callers hold the status lock, including the settings re-enable path.
    fn save_streak(&self, status: &mut Status, since: Option<SystemTime>) {
        if status.probe_failure_since == since {
            return;
        }
        status.probe_failure_since = since;
        if let Err(error) = (self.deps.write_probe_failure_since)(since) {
            tracing::warn!("updater: saving the probe failure streak: {error:#}");
        }
    }

    /// Probe, compare, and install when the latest release is newer than
    /// what is installed. Distinct failure boundaries let the caller apply
    /// the grace period only to an unusable `/latest` answer; successful
    /// probes clear that streak before the slower install can fail.
    fn check_and_install(&self) -> CheckResult {
        let latest = match (self.deps.probe)() {
            Ok(latest) => latest,
            Err(error) => {
                tracing::info!("updater: checking for the latest release failed: {error:#}");
                return CheckResult::ProbeFailed(
                    "get.farhelm.io did not say which release is the latest".to_string(),
                );
            }
        };
        {
            let mut status = self.shared.status.lock().expect("updater status poisoned");
            self.save_streak(&mut status, None);
            status.last_probe_clock = Some((self.deps.now)());
        }
        // The probe can take a while, and the user may have run the
        // installer from a terminal meanwhile, so the decision uses the
        // Installed record as it is now, not as it was when the run began.
        // A newer version installed that way would otherwise be replaced
        // by an older release, and an automatic run must still hold back
        // once an update waits.
        let installed = (self.deps.read_installed)();
        let (baseline, automatic_should_wait) = {
            let mut status = self.shared.status.lock().expect("updater status poisoned");
            status.record_installed(installed);
            self.shared.publish(&status);
            let baseline = status
                .installed
                .clone()
                .unwrap_or_else(|| status.running.clone());
            let waiting = newer_installed(status.installed.as_deref(), &status.running);
            (baseline, waiting && !status.user_waiting)
        };
        if automatic_should_wait {
            tracing::info!(
                "updater: {baseline} was installed during the check; nothing more to do"
            );
            return CheckResult::Success(Activity::Idle);
        }
        if !build_is_newer(&latest, &baseline) {
            tracing::info!("updater: {latest} is the latest release; {baseline} is installed");
            // Nothing newer to verify, so nothing this app cannot install.
            self.set_needs_reinstall(false);
            return CheckResult::Success(Activity::UpToDate(latest));
        }
        // Automatic updates may have been turned off while the probe was
        // out. Off means no background install from then on, so a run no
        // user has joined stops here; a run the user asked for goes on.
        let user_waiting = self
            .shared
            .status
            .lock()
            .expect("updater status poisoned")
            .user_waiting;
        if !user_waiting && !(self.deps.automatic_enabled)() {
            tracing::info!("updater: automatic updates were turned off during the check");
            return CheckResult::Success(Activity::Idle);
        }
        tracing::info!("updater: installing {latest} (installed: {baseline})");
        self.set_current(Activity::Installing(latest.clone()));
        if let Err(error) = (self.deps.install)(&latest) {
            if error.downcast_ref::<UnverifiableRelease>().is_some() {
                // Nothing was run. Kept, and published whether or not a
                // user is watching: an app that cannot verify its updates
                // has stopped updating, and must say so (SPEC.md,
                // "Installation and updates").
                tracing::warn!("updater: refusing Farhelm {latest}: {error:#}");
                self.set_needs_reinstall(true);
                return CheckResult::VerificationFailed(format!(
                    "Farhelm {latest} could not be verified, so it was not installed"
                ));
            }
            tracing::warn!("updater: installing {latest} failed: {error:#}");
        }
        if (self.deps.read_installed)().as_deref() == Some(latest.as_str()) {
            tracing::info!("updater: {latest} is installed; a restart finishes the update");
            self.set_needs_reinstall(false);
            let running = self
                .shared
                .status
                .lock()
                .expect("updater status poisoned")
                .running
                .clone();
            if build_is_newer(&latest, &running) {
                // The update marker says it from here on.
                CheckResult::Success(Activity::Idle)
            } else {
                // Installed went back up to the running version (it named
                // an older one the user pinned); no marker will appear, so
                // the user's check needs its own answer.
                CheckResult::Success(Activity::UpToDate(latest))
            }
        } else {
            CheckResult::InstallFailed(format!(
                "installing Farhelm {latest} did not finish (the app's log says why)"
            ))
        }
    }
}

// ===== The real probe and installer ========================================

/// Select the captured release-test override or the ordinary site probe.
///
/// A present value is authoritative even when malformed or empty: every check fails
/// instead of silently testing whichever release `/latest` happens to name.
/// The site's stable-tag parser is shared, and the worker still decides
/// whether the selected version is newer before running its verified install.
/// Taking the value and site probe as inputs keeps tests off the environment
/// and network.
fn probe_with_latest_override(value: Option<OsString>, site_probe: Box<ProbeFn>) -> Box<ProbeFn> {
    match value {
        None => site_probe,
        Some(value) => Box::new(move || {
            latest_from_site(value.as_encoded_bytes())
                .with_context(|| format!("invalid {LATEST_OVERRIDE_ENV} override"))
        }),
    }
}

/// Ask get.farhelm.io which stable release is the latest: a plain GET of
/// `/latest`, one line naming the tag.
fn probe_latest_release() -> anyhow::Result<String> {
    let body = fetch_capped(
        &format!("{SITE_URL}/latest"),
        LATEST_MAX_BYTES,
        PROBE_TIMEOUT,
    )?;
    latest_from_site(&body)
}

/// The latest release's version from `/latest`'s answer: exactly one line
/// (a final newline allowed) holding `v` and a stable release version.
///
/// Anything else is an error rather than a guess: a prerelease (`/latest`
/// never names one, so one there is a broken or tampered answer), a second
/// line, stray bytes, or text that is not a version. The leading `v` is
/// dropped, because the Installed record holds the bare version.
fn latest_from_site(body: &[u8]) -> anyhow::Result<String> {
    let text = std::str::from_utf8(body).context("get.farhelm.io's /latest is not text")?;
    let tag = text.strip_suffix('\n').unwrap_or(text);
    let version = tag
        .strip_prefix('v')
        .with_context(|| format!("get.farhelm.io's /latest {tag:?} is not a release tag"))?;
    let parsed = semver::Version::parse(version)
        .with_context(|| format!("get.farhelm.io's /latest {tag:?} is not a release version"))?;
    if !parsed.pre.is_empty() || !parsed.build.is_empty() {
        bail!("get.farhelm.io's /latest {tag:?} is not a stable release");
    }
    Ok(version.to_string())
}

/// A release this updater refused to install because it did not verify: the
/// signature, the version it was signed for, or the installer's checksum.
/// Distinct from every other install failure because it is the one that
/// will not go away on retry and that the user must hear about
/// ([`UpdaterState::needs_reinstall`]).
#[derive(Debug)]
struct UnverifiableRelease(String);

impl std::fmt::Display for UnverifiableRelease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UnverifiableRelease {}

/// Verify release `version` and run its installer, in `dir`.
///
/// The install step's whole trust decision, with the network and the
/// process behind `fetch` (a file name under the release's directory on the
/// site, and a size cap) and `run` (the verified script, then the verified
/// checksums, both written in `dir`), so it is tested on real signed bytes
/// without either. In order: the signed `SHA256SUMS` must verify against
/// `ring` with trusted comment `farhelm v{version}`
/// (`farhelm_helm::verify_signed_sums`, the rules the helm applies to its
/// own downloads); it must list `install.sh`; the downloaded script must
/// match that entry. Only then is `run` called. A failure of any of those is
/// an [`UnverifiableRelease`]; a failed download is an ordinary error, and
/// either way nothing has been run.
fn install_release(
    version: &str,
    ring: &[&str],
    dir: &Path,
    fetch: &dyn Fn(&str, usize) -> anyhow::Result<Vec<u8>>,
    run: &dyn Fn(&Path, &Path) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let sums = fetch("SHA256SUMS", SUMS_MAX_BYTES)?;
    let signature = fetch("SHA256SUMS.minisig", SIGNATURE_MAX_BYTES)?;
    let entries = farhelm_helm::verify_signed_sums(ring, version, &sums, &signature)
        .map_err(|error| UnverifiableRelease(format!("{error:#}")))?;
    let expected = entries.get("install.sh").ok_or_else(|| {
        UnverifiableRelease(format!(
            "Farhelm {version}'s signed SHA256SUMS lists no install.sh"
        ))
    })?;
    // Compared before anything else is said about the body: an empty or
    // truncated answer is a script that does not match its signed checksum,
    // the same verification failure as any other altered one.
    let script = fetch("install.sh", INSTALLER_MAX_BYTES)?;
    let actual = hex_sha256(&script);
    if actual != *expected {
        return Err(UnverifiableRelease(format!(
            "Farhelm {version}'s install.sh does not match its signed checksum (expected \
             {expected}, got {actual})"
        ))
        .into());
    }
    let script_path = dir.join("install.sh");
    let sums_path = dir.join("SHA256SUMS");
    std::fs::write(&script_path, &script)
        .with_context(|| format!("writing {}", script_path.display()))?;
    std::fs::write(&sums_path, &sums)
        .with_context(|| format!("writing {}", sums_path.display()))?;
    run(&script_path, &sums_path)
}

/// Lowercase hex SHA-256, the form `SHA256SUMS` lists.
fn hex_sha256(bytes: &[u8]) -> String {
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Verify release `version` and run its installer, logging its output.
///
/// The installer is downloaded to a file and run with `/bin/sh`, rather than
/// `curl | sh`, so it can be verified first ([`install_release`]) and so a
/// failed download is an error before anything runs. It runs pinned to
/// `version` and handed the verified checksums; every other `FARHELM_*`
/// variable is scrubbed. The result is advisory: the caller judges success
/// by the Installed record.
///
/// The installer must outlive the app: a quit mid-install is ordinary,
/// since nothing on screen says an install is running. So it gets its own
/// process group (no signal aimed at the app's group reaches it), stdin from
/// `/dev/null`, and its output in a file rather than a pipe. A pipe would
/// break when the app exits, and `install.sh` dies of SIGPIPE without its
/// cleanup, leaving its lock behind for every later install to refuse on.
/// The file is copied into the app's log after the installer exits; if the
/// app quits first, the directory holding the script, the checksums and the
/// log stays in the temporary directory, which the system clears.
fn run_installer(version: &str) -> anyhow::Result<()> {
    let dir = private_temp_dir()?;
    let fetch = |name: &str, cap: usize| {
        fetch_capped(
            &format!("{SITE_URL}/v{version}/{name}"),
            cap,
            INSTALLER_DOWNLOAD_TIMEOUT,
        )
    };
    let run = |script: &Path, sums: &Path| spawn_installer(&dir, script, sums, version);
    let outcome = install_release(version, farhelm_helm::RELEASE_KEY_RING, &dir, &fetch, &run);
    let _ = std::fs::remove_dir_all(&dir);
    outcome
}

/// Run the verified `script` pinned to `version` with `sums` as its checksum
/// file, its output in a log in `dir`, and wait for it.
fn spawn_installer(dir: &Path, script: &Path, sums: &Path, version: &str) -> anyhow::Result<()> {
    let log_path = dir.join("install.log");
    let log =
        File::create(&log_path).with_context(|| format!("creating {}", log_path.display()))?;
    let log_for_stderr = log
        .try_clone()
        .context("sharing the installer's log file")?;
    let mut command = Command::new("/bin/sh");
    command
        .arg(script)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_for_stderr));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    for name in farhelm_variables(std::env::vars_os().map(|(name, _)| name)) {
        command.env_remove(name);
    }
    command.env(VERSION_PIN_ENV, version);
    command.env(SUMS_FILE_ENV, sums);
    let status = command
        .spawn()
        .context("starting the installer")?
        .wait()
        .context("waiting for the installer")?;
    let output = std::fs::read(&log_path).unwrap_or_default();
    for line in String::from_utf8_lossy(&output).lines() {
        tracing::info!("updater: installer: {line}");
    }
    if status.success() {
        Ok(())
    } else {
        bail!("the installer exited with {status}")
    }
}

/// GET `url` over https only, refusing a non-success status and anything
/// over `cap` bytes while it downloads.
///
/// Redirects are followed (still https only), as the site may serve a file
/// from elsewhere; integrity never rests on where it came from, since every
/// byte this updater acts on is checked against the signature first.
fn fetch_capped(url: &str, cap: usize, timeout: Duration) -> anyhow::Result<Vec<u8>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("starting the updater's runtime")?;
    runtime.block_on(async {
        let client = reqwest::Client::builder()
            .https_only(true)
            .timeout(timeout)
            .build()
            .context("building the updater's HTTP client")?;
        let mut response = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("fetching {url}"))?
            .error_for_status()
            .with_context(|| format!("fetching {url}"))?;
        // Read chunk by chunk so the cap holds while downloading: a whole
        // body buffered first would already be in memory when checked.
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .with_context(|| format!("fetching {url}"))?
        {
            if bytes.len() + chunk.len() > cap {
                bail!("{url} is over {cap} bytes, which cannot be it");
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    })
}

/// A fresh directory only this user can read, for the downloaded script, the
/// verified checksums and the installer's log.
fn private_temp_dir() -> anyhow::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("farhelm-update-{}", uuid::Uuid::new_v4()));
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(&dir)
        .with_context(|| format!("creating {}", dir.display()))?;
    Ok(dir)
}

/// The variable names to remove from the installer's environment: every
/// `FARHELM_*` the app inherited.
///
/// The installer otherwise sees exactly the app's environment (`HOME`,
/// `PATH`, proxies), so it runs as the user's own run would. A `FARHELM_*`
/// variable is never something the user meant for this run, and one of them
/// (`FARHELM_INSTALL_TEST_BASE_URL`) would redirect the download.
/// `FARHELM_VERSION` and `FARHELM_INSTALL_SUMS_FILE` are set afresh
/// afterwards, and are the only ones the installer gets.
fn farhelm_variables(names: impl Iterator<Item = OsString>) -> Vec<OsString> {
    names
        .filter(|name| {
            name.to_str()
                .is_some_and(|name| name.starts_with("FARHELM_"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::AtomicUsize;

    // ---- activation ----

    /// Build `<home>/Applications/Farhelm.app` with a main program and,
    /// when asked, the versions folder; return the main program's path.
    fn fake_bundle(home: &Path, with_versions: bool) -> PathBuf {
        let contents = home.join("Applications/Farhelm.app/Contents");
        std::fs::create_dir_all(contents.join("MacOS")).unwrap();
        if with_versions {
            std::fs::create_dir_all(contents.join("Versions")).unwrap();
        }
        let exe = contents.join("MacOS/farhelm-desktop");
        std::fs::write(&exe, b"").unwrap();
        exe
    }

    /// Spec: the updater runs only for the installed bundle at
    /// `~/Applications/Farhelm.app` in the side-by-side layout, running a
    /// release build.
    ///
    /// The gate is what keeps a development build, a copy elsewhere and an
    /// old-layout app from installing every day without ever seeing their
    /// own Installed record change, and keeps CI and the desktop smoke test
    /// off the network.
    #[farhelm_testtrace::test]
    fn the_updater_runs_only_for_the_installed_release_bundle() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let exe = fake_bundle(&home, true);
        let contents =
            std::fs::canonicalize(home.join("Applications/Farhelm.app/Contents")).unwrap();
        assert_eq!(active_bundle(&exe, &home, "1.2.3"), Some(contents));

        assert_eq!(
            active_bundle(&exe, &home, "0.0.0-unreleased"),
            None,
            "a build from main"
        );
        assert_eq!(active_bundle(&exe, &home, "not a version"), None);

        let other_home = root.path().join("someone-else");
        std::fs::create_dir_all(&other_home).unwrap();
        assert_eq!(
            active_bundle(&exe, &other_home, "1.2.3"),
            None,
            "not this user's bundle"
        );

        let old_layout_home = root.path().join("old");
        let old_exe = fake_bundle(&old_layout_home, false);
        assert_eq!(
            active_bundle(&old_exe, &old_layout_home, "1.2.3"),
            None,
            "no Versions folder"
        );

        let loose = root.path().join("farhelm-desktop");
        std::fs::write(&loose, b"").unwrap();
        assert_eq!(
            active_bundle(&loose, &home, "1.2.3"),
            None,
            "not inside a bundle"
        );
    }

    /// Spec: a symlinked path to the installed bundle still counts, while a
    /// copy of it elsewhere does not.
    ///
    /// Both sides are canonicalized so the comparison is about which bundle
    /// is running, not how its path was spelled.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    fn a_symlinked_path_to_the_installed_bundle_counts() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        fake_bundle(&home, true);
        let link = root.path().join("link.app");
        std::os::unix::fs::symlink(home.join("Applications/Farhelm.app"), &link).unwrap();
        assert!(
            active_bundle(&link.join("Contents/MacOS/farhelm-desktop"), &home, "1.2.3").is_some()
        );

        let copy_home = root.path().join("copy");
        let copy = fake_bundle(&copy_home, true);
        assert_eq!(active_bundle(&copy, &home, "1.2.3"), None);
    }

    /// Spec: the Installed record's version is its first line, trimmed, and
    /// a missing or blank record is no version.
    #[farhelm_testtrace::test]
    fn the_installed_record_is_its_first_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("installed");
        assert_eq!(read_installed_record(&path), None);
        std::fs::write(&path, "0.22.0\n").unwrap();
        assert_eq!(read_installed_record(&path).as_deref(), Some("0.22.0"));
        std::fs::write(&path, "\n").unwrap();
        assert_eq!(read_installed_record(&path), None);
    }

    /// Spec: the automatic-updates setting is on when absent, when the
    /// state file is missing, and when it cannot be read; only an explicit
    /// `false` turns it off.
    #[farhelm_testtrace::test]
    fn automatic_updates_are_on_unless_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(state::APP_STATE_FILE);
        assert!(automatic_updates_enabled(&path), "no file");
        std::fs::write(&path, r#"{"webview_auth_generation":1}"#).unwrap();
        assert!(automatic_updates_enabled(&path), "no setting");
        std::fs::write(&path, r#"{"install_updates_automatically":false}"#).unwrap();
        assert!(!automatic_updates_enabled(&path));
        std::fs::write(&path, r#"{"install_updates_automatically":true}"#).unwrap();
        assert!(automatic_updates_enabled(&path));
        std::fs::write(&path, "not json").unwrap();
        assert!(automatic_updates_enabled(&path), "unreadable file");
    }

    // ---- Restart to update ----

    /// Spec: the opener's environment explicitly excludes the test override,
    /// even when the parent app inherited it. This guards the child launch
    /// contract without setting any variable in the test process; native
    /// LaunchServices forwarding still needs the Mac release test.
    #[farhelm_testtrace::test]
    fn the_relaunch_command_removes_the_latest_override() {
        let command = relaunch_command(1, Path::new("bundle"), Path::new("opener"), 0, None);
        assert!(
            command
                .get_envs()
                .any(|(key, value)| key == LATEST_OVERRIDE_ENV && value.is_none())
        );
    }

    /// A stand-in for `/usr/bin/open` that records the arguments it was
    /// given, so a test can see whether and how the helper relaunched.
    fn recording_opener(dir: &Path) -> (PathBuf, PathBuf) {
        let marker = dir.join("opened");
        let opener = dir.join("open");
        std::fs::write(
            &opener,
            format!("#!/bin/sh\nprintf '%s' \"$*\" > '{}'\n", marker.display()),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&opener, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        (opener, marker)
    }

    /// Wait, bounded, until the helper reports having seen the app alive.
    fn await_seen_alive(seen: &Path) {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !seen.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "the helper never checked the app"
            );
            std::thread::sleep(Duration::from_millis(20)); // sleep-ok: polling interval for the helper's readiness file, bounded by the deadline above
        }
    }

    /// Spec: Restart to update's helper keeps waiting while the app's
    /// process lives, and opens the bundle only once it has exited.
    ///
    /// Opening while the old app still runs would only bring it to the
    /// front, so the wait is what makes the restart a restart. The stand-in
    /// app is released only after the helper has provably seen it alive
    /// (its readiness file), so a helper that checked once and gave up, or
    /// opened at once, fails here. The test reaps its own child, since an
    /// unreaped child would still answer the helper's liveness check.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    fn the_relaunch_helper_opens_the_bundle_after_the_app_exits() {
        let dir = tempfile::tempdir().unwrap();
        let (opener, marker) = recording_opener(dir.path());
        let seen = dir.path().join("seen-alive");
        let bundle = dir.path().join("Farhelm App.app");
        let mut app = Command::new("/bin/sh")
            .args(["-c", "read line"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let mut helper =
            spawn_relaunch_helper(app.id(), &bundle, &opener, 600, Some(&seen)).unwrap();
        await_seen_alive(&seen);
        assert!(
            helper.try_wait().unwrap().is_none(),
            "the helper waits while the app lives"
        );
        assert!(!marker.exists(), "nothing opens while the app runs");
        // Closing the app's stdin ends it, as quitting ends the real app.
        drop(app.stdin.take());
        app.wait().unwrap();
        assert!(helper.wait().unwrap().success());
        assert_eq!(
            std::fs::read_to_string(&marker).unwrap(),
            format!("-n {}", bundle.display())
        );
    }

    /// Spec: if the app is still alive when the bound runs out, the helper
    /// gives up without opening anything.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    fn the_relaunch_helper_gives_up_on_an_app_that_does_not_exit() {
        let dir = tempfile::tempdir().unwrap();
        let (opener, marker) = recording_opener(dir.path());
        let seen = dir.path().join("seen-alive");
        let mut app = Command::new("/bin/sh")
            .args(["-c", "read line"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let mut helper =
            spawn_relaunch_helper(app.id(), dir.path(), &opener, 2, Some(&seen)).unwrap();
        await_seen_alive(&seen);
        assert!(helper.wait().unwrap().success());
        assert!(!marker.exists(), "an app that did not exit is not reopened");
        assert!(
            app.try_wait().unwrap().is_none(),
            "the app was still alive at the bound"
        );
        drop(app.stdin.take());
        app.wait().unwrap();
    }

    /// Spec: turning automatic updates off while a check is out stops that
    /// check from installing, unless a user's request joined it.
    ///
    /// Off means no background install from then on, and the probe can take
    /// tens of seconds, which is when someone opening the settings right
    /// after launch would untick the box.
    #[farhelm_testtrace::test]
    fn turning_automatic_updates_off_during_a_check_stops_its_install() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), true);
        let automatic = Arc::clone(&rig.automatic);
        rig.engine.deps.probe = Box::new(move || {
            *automatic.lock().unwrap() = false;
            Ok("1.1.0".to_string())
        });
        rig.engine.wake_once();
        assert!(rig.installs.lock().unwrap().is_empty());
        assert_eq!(rig.published().activity, Activity::Idle);

        // The same, with the user asking for a check while the probe is out.
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), true);
        let automatic = Arc::clone(&rig.automatic);
        let asked = Arc::clone(&rig.shared);
        rig.engine.deps.probe = Box::new(move || {
            *automatic.lock().unwrap() = false;
            asked.check_now();
            Ok("1.1.0".to_string())
        });
        rig.engine.wake_once();
        assert_eq!(*rig.installs.lock().unwrap(), ["1.1.0"]);
    }

    /// Spec: a setting that cannot be saved reports the failure and leaves
    /// the stored choice in force, so the dialog never shows an unsaved
    /// choice as the one Farhelm follows.
    #[farhelm_testtrace::test]
    fn a_setting_that_cannot_be_saved_reports_it() {
        let dir = tempfile::tempdir().unwrap();
        // A directory where the state file should be: every write fails.
        let state_path = dir.path().join(state::APP_STATE_FILE);
        std::fs::create_dir(&state_path).unwrap();
        let handle = UpdaterHandle {
            shared: Arc::new(Shared::new("1.0.0")),
            bundle: dir.path().join("Farhelm.app"),
            state_path,
        };
        assert!(handle.set_automatic_updates(false).is_err());
        assert!(handle.automatic_updates(), "the default, still in force");

        let ok_path = dir.path().join("ok.json");
        let handle = UpdaterHandle {
            state_path: ok_path,
            ..handle
        };
        assert!(handle.set_automatic_updates(false).is_ok());
        assert!(!handle.automatic_updates());
    }

    // ---- the probe and the installer's environment ----

    /// Spec: the latest release is read from get.farhelm.io's `/latest`, one
    /// `v`-prefixed stable tag (a final newline allowed), returned without
    /// its `v`; a prerelease, a second line, a missing `v` or anything that
    /// is not a version is a failed check rather than a guess.
    ///
    /// Why: `/latest` is the only thing that decides what an automatic update
    /// installs, and it is served over TLS alone. A prerelease there is a
    /// broken or tampered answer; this app's channel is stable releases.
    #[farhelm_testtrace::test]
    fn the_latest_release_comes_from_the_site() {
        assert_eq!(latest_from_site(b"v0.23.1\n").unwrap(), "0.23.1");
        assert_eq!(latest_from_site(b"v0.23.1").unwrap(), "0.23.1");
        for refused in [
            &b"v0.24.0-rc.1\n"[..],
            b"0.23.1\n",
            b"v0.23.1\nv0.23.2\n",
            b"v0.23.1\r\n",
            b"v0.23.1 \n",
            b"",
            b"<html>",
            b"v0.23.1+build\n",
        ] {
            assert!(
                latest_from_site(refused).is_err(),
                "{:?} must be refused",
                String::from_utf8_lossy(refused)
            );
        }
    }

    /// Spec: without a captured override each check still asks the site.
    ///
    /// Injecting the site probe distinguishes delegation from a cached
    /// answer without making the test depend on the live release service.
    #[farhelm_testtrace::test]
    fn an_absent_latest_override_uses_the_site_each_time() {
        let calls = Arc::new(AtomicUsize::new(0));
        let site_calls = Arc::clone(&calls);
        let probe = probe_with_latest_override(
            None,
            Box::new(move || {
                site_calls.fetch_add(1, Ordering::SeqCst);
                Ok("1.2.3".into())
            }),
        );
        assert_eq!(probe().unwrap(), "1.2.3");
        assert_eq!(probe().unwrap(), "1.2.3");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    /// Spec: a present override accepts the site's stable-tag grammar and
    /// never consults the site, even on repeated failures. Falling back
    /// would let a candidate test pass against a different release.
    #[farhelm_testtrace::test]
    fn a_latest_override_is_authoritative_even_when_invalid() {
        for value in ["v1.2.3", "v1.2.3\n"] {
            let probe = probe_with_latest_override(
                Some(value.into()),
                Box::new(|| panic!("a present override must not reach the site")),
            );
            for _ in 0..2 {
                assert_eq!(probe().unwrap(), "1.2.3", "{value:?}");
            }
        }
        for value in [
            "",
            "1.2.3",
            "v1.2.3-rc.1",
            "v1.2.3+build",
            "v1.2.3\nv1.2.4",
            "nonsense",
        ] {
            let probe = probe_with_latest_override(
                Some(value.into()),
                Box::new(|| panic!("a present override must not reach the site")),
            );
            for _ in 0..2 {
                assert!(
                    format!("{:#}", probe().unwrap_err()).contains(LATEST_OVERRIDE_ENV),
                    "{value:?}"
                );
            }
        }
    }

    /// Spec: an environment value that is not UTF-8 is a failed check,
    /// never an absent override. Unix environments can contain these bytes;
    /// lossy conversion would hide the value the release test actually set.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    fn a_non_text_latest_override_never_falls_back() {
        use std::os::unix::ffi::OsStringExt;
        let probe = probe_with_latest_override(
            Some(OsString::from_vec(b"v1.2.3\xff".to_vec())),
            Box::new(|| panic!("a non-text override must not reach the site")),
        );
        for _ in 0..2 {
            assert!(format!("{:#}", probe().unwrap_err()).contains(LATEST_OVERRIDE_ENV));
        }
    }

    /// Spec: automatic and manual checks use the same captured candidate,
    /// and selecting an older release still cannot downgrade the app.
    /// The worker must retain these rules regardless of probe selection.
    #[farhelm_testtrace::test]
    fn latest_override_preserves_both_triggers_and_version_order() {
        for manual in [false, true] {
            for (candidate, expected) in [("v1.2.0", vec!["1.2.0"]), ("v0.9.0", vec![])] {
                let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("9.0.0"), true);
                // Keep the rig's counting probe as the site probe: zero
                // calls then proves the candidate did not come from the site.
                let site_probe =
                    std::mem::replace(&mut rig.engine.deps.probe, Box::new(|| unreachable!()));
                rig.engine.deps.probe =
                    probe_with_latest_override(Some(candidate.into()), site_probe);
                if manual {
                    *rig.automatic.lock().unwrap() = false;
                    rig.shared.check_now();
                }
                rig.engine.wake_once();
                assert_eq!(
                    *rig.installs.lock().unwrap(),
                    expected,
                    "{candidate}, manual={manual}"
                );
                assert_eq!(
                    rig.probes.load(Ordering::SeqCst),
                    0,
                    "the site was not consulted"
                );
            }
        }
    }

    // ---- verifying a release before running its installer ----

    /// The fixture release's version (its signature's trusted comment is
    /// `farhelm v1.2.3`); see `tests/fixtures/updater-release/README.md`.
    const FIXTURE_VERSION: &str = "1.2.3";

    /// The bytes of one file of the fixture release.
    fn fixture(relative: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/updater-release")
                .join(relative),
        )
        .unwrap()
    }

    /// The fixture release's key, as a one-key ring.
    fn fixture_ring() -> Vec<String> {
        let key = String::from_utf8(fixture("test-key.pub")).unwrap();
        vec![
            key.lines()
                .nth(1)
                .expect("a comment line then the key line")
                .trim()
                .to_string(),
        ]
    }

    /// What the installer was handed, when it ran: the script, then the
    /// checksum file.
    type Ran = Option<(Vec<u8>, Vec<u8>)>;

    /// Run [`install_release`] for the fixture version against `served`
    /// (file name to bytes; anything else answers as a failed download),
    /// returning its result and, when it ran the installer, the script and
    /// checksum bytes it was handed. The ring is the fixture key's.
    fn try_install(served: HashMap<&'static str, Vec<u8>>) -> (anyhow::Result<()>, Ran) {
        let ring = fixture_ring();
        let ring: Vec<&str> = ring.iter().map(String::as_str).collect();
        try_install_with(&ring, served)
    }

    /// [`try_install`] with an explicit key ring.
    fn try_install_with(
        ring: &[&str],
        served: HashMap<&'static str, Vec<u8>>,
    ) -> (anyhow::Result<()>, Ran) {
        let dir = tempfile::tempdir().unwrap();
        let fetch = |name: &str, cap: usize| -> anyhow::Result<Vec<u8>> {
            let bytes = served
                .get(name)
                .cloned()
                .with_context(|| format!("404 for {name}"))?;
            anyhow::ensure!(bytes.len() <= cap, "{name} over its cap");
            Ok(bytes)
        };
        let ran = std::cell::RefCell::new(None);
        let run = |script: &Path, sums: &Path| -> anyhow::Result<()> {
            *ran.borrow_mut() = Some((std::fs::read(script)?, std::fs::read(sums)?));
            Ok(())
        };
        let result = install_release(FIXTURE_VERSION, ring, dir.path(), &fetch, &run);
        (result, ran.into_inner())
    }

    /// The fixture release as the site serves it.
    fn good_release() -> HashMap<&'static str, Vec<u8>> {
        HashMap::from([
            ("SHA256SUMS", fixture("SHA256SUMS")),
            ("SHA256SUMS.minisig", fixture("SHA256SUMS.minisig")),
            ("install.sh", fixture("install.sh")),
        ])
    }

    /// Spec: a release whose signed checksums verify against the ring with
    /// this version's trusted comment, and whose `install.sh` matches its
    /// signed entry, runs that exact script handed exactly those checksums
    /// (which list more than the installer).
    ///
    /// Why: this is the one path by which the app runs code it downloaded;
    /// the installer then checks every archive against the checksums it is
    /// handed, so they must be the verified bytes, not a second fetch.
    #[farhelm_testtrace::test]
    fn a_verified_release_runs_its_installer_with_the_signed_checksums() {
        assert!(
            String::from_utf8(fixture("SHA256SUMS"))
                .unwrap()
                .lines()
                .count()
                > 1,
            "fixture premise: the checksums list more than the installer"
        );
        let (result, ran) = try_install(good_release());
        result.expect("the fixture release verifies");
        let (script, sums) = ran.expect("the installer ran");
        assert_eq!(script, fixture("install.sh"));
        assert_eq!(sums, fixture("SHA256SUMS"));
    }

    /// Spec: a release is refused, with the installer never run and the
    /// refusal an [`UnverifiableRelease`], when its `SHA256SUMS` does not
    /// verify (tampered bytes, or a valid signature by a key outside this
    /// app's ring), when its signature names another version,
    /// when its signed checksums list no `install.sh`, or when the
    /// downloaded `install.sh` does not match its signed entry (an empty one
    /// included). A failed
    /// download is refused too, as an ordinary error, not as unverifiable.
    ///
    /// Why: each of these is a way to make the app run bytes nobody signed
    /// for this version, and the unverifiable ones must also leave the
    /// reinstall notice, which a passing network failure must not.
    #[farhelm_testtrace::test]
    fn an_unverifiable_release_runs_nothing() {
        let mut tampered = good_release();
        let mut sums = fixture("SHA256SUMS");
        sums[0] ^= 1;
        tampered.insert("SHA256SUMS", sums);

        let mut wrong_comment = good_release();
        wrong_comment.insert(
            "SHA256SUMS.minisig",
            fixture("variants/wrong-comment/SHA256SUMS.minisig"),
        );

        let no_installer = HashMap::from([
            ("SHA256SUMS", fixture("variants/no-installer/SHA256SUMS")),
            (
                "SHA256SUMS.minisig",
                fixture("variants/no-installer/SHA256SUMS.minisig"),
            ),
            ("install.sh", fixture("install.sh")),
        ]);

        let mut other_script = good_release();
        other_script.insert(
            "install.sh",
            b"#!/bin/sh\necho not the signed one\n".to_vec(),
        );

        let mut empty_script = good_release();
        empty_script.insert("install.sh", Vec::new());

        for (case, served, says) in [
            ("tampered checksums", tampered, "does not verify"),
            ("another version's signature", wrong_comment, "signed for"),
            ("no install.sh entry", no_installer, "lists no install.sh"),
            (
                "a different install.sh",
                other_script,
                "does not match its signed checksum",
            ),
            (
                "an empty install.sh",
                empty_script,
                "does not match its signed checksum",
            ),
        ] {
            let (result, ran) = try_install(served);
            let error = result.expect_err(case);
            assert!(ran.is_none(), "{case}: nothing may run");
            assert!(
                error.downcast_ref::<UnverifiableRelease>().is_some(),
                "{case}: {error:#}"
            );
            assert!(format!("{error:#}").contains(says), "{case}: {error:#}");
        }

        // The case the reinstall notice exists for: a release signed by a
        // key this app was built without.
        assert!(
            !farhelm_helm::RELEASE_KEY_RING.contains(&fixture_ring()[0].as_str()),
            "fixture premise: the production ring lacks the fixture key"
        );
        let (result, ran) = try_install_with(farhelm_helm::RELEASE_KEY_RING, good_release());
        let error = result.expect_err("a key outside the ring");
        assert!(ran.is_none(), "a key outside the ring: nothing may run");
        assert!(
            error.downcast_ref::<UnverifiableRelease>().is_some(),
            "a key outside the ring: {error:#}"
        );
        assert!(
            format!("{error:#}").contains("does not verify"),
            "a key outside the ring: {error:#}"
        );

        let mut missing = good_release();
        missing.remove("SHA256SUMS.minisig");
        let (result, ran) = try_install(missing);
        let error = result.expect_err("a failed download");
        assert!(ran.is_none());
        assert!(
            error.downcast_ref::<UnverifiableRelease>().is_none(),
            "a failed download is not a verification failure: {error:#}"
        );
        assert!(
            format!("{error:#}").contains("404 for SHA256SUMS.minisig"),
            "the cause survives: {error:#}"
        );
    }

    /// Spec: every inherited `FARHELM_*` variable is removed from the
    /// installer's environment, and nothing else is.
    ///
    /// An inherited `FARHELM_INSTALL_TEST_BASE_URL` would send the download
    /// somewhere else; the rest of the environment must stay the user's so
    /// the run matches one from their own terminal.
    #[farhelm_testtrace::test]
    fn only_farhelm_variables_leave_the_installer_environment() {
        let names = [
            "HOME",
            "PATH",
            "FARHELM_INSTALL_TEST_BASE_URL",
            "FARHELM_VERSION",
            "HTTPS_PROXY",
            "XFARHELM_X",
        ]
        .map(OsString::from);
        assert_eq!(
            farhelm_variables(names.into_iter()),
            vec![
                OsString::from("FARHELM_INSTALL_TEST_BASE_URL"),
                OsString::from("FARHELM_VERSION")
            ]
        );
    }

    /// Spec: with the reinstall notice set, the readout carries it (and the
    /// reinstall command in its hover) whatever else is true, including
    /// while an installed update waits for a restart, which keeps its own
    /// update-ready marker beside it.
    ///
    /// Why: an on-demand check can fail verification of a still newer
    /// release while an earlier update waits; the warning must not vanish
    /// behind the update marker.
    #[farhelm_testtrace::test]
    fn the_reinstall_notice_shows_beside_a_waiting_update() {
        for installed in [Some("1.0.0"), Some("1.1.0")] {
            let state = UpdaterState {
                needs_reinstall: true,
                ..state(installed, Activity::Idle)
            };
            let shown = readout(&state);
            assert!(shown.needs_reinstall, "{installed:?}");
            assert!(shown.tooltip.contains(REINSTALL_COMMAND), "{installed:?}");
            assert_eq!(shown.update_ready, installed == Some("1.1.0"));
        }
    }

    // ---- the worker ----

    /// A worker over fake dependencies: `latest` is what the probe answers
    /// (`None` fails it), `installed` is the Installed record, which the
    /// fake installer rewrites to the version it was given when
    /// `install_works`.
    struct Rig {
        engine: Engine,
        shared: Arc<Shared>,
        installed: Arc<Mutex<Option<String>>>,
        latest: Arc<Mutex<Option<String>>>,
        automatic: Arc<Mutex<bool>>,
        now: Arc<Mutex<SystemTime>>,
        probes: Arc<AtomicUsize>,
        installs: Arc<Mutex<Vec<String>>>,
        streak: Arc<Mutex<Option<SystemTime>>>,
    }

    /// A new worker with no durable history and a controlled clock. Fake
    /// probes/installers expose actual attempts rather than inferred timing.
    fn make_rig(
        running: &str,
        installed: Option<&str>,
        latest: Option<&str>,
        install_works: bool,
    ) -> Rig {
        make_rig_with_streak(
            running,
            installed,
            latest,
            install_works,
            Arc::new(Mutex::new(None)),
        )
    }

    /// Inject a restart's durable record without sharing a live worker. Tests
    /// can use the real file closures instead to exercise decoding/write errors.
    fn make_rig_with_streak(
        running: &str,
        installed: Option<&str>,
        latest: Option<&str>,
        install_works: bool,
        streak: Arc<Mutex<Option<SystemTime>>>,
    ) -> Rig {
        let shared = Arc::new(Shared::new(running));
        let installed = Arc::new(Mutex::new(installed.map(str::to_string)));
        let latest = Arc::new(Mutex::new(latest.map(str::to_string)));
        let automatic = Arc::new(Mutex::new(true));
        let now = Arc::new(Mutex::new(
            SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000),
        ));
        let probes = Arc::new(AtomicUsize::new(0));
        let installs = Arc::new(Mutex::new(Vec::new()));
        let deps = Deps {
            probe: Box::new({
                let latest = Arc::clone(&latest);
                let probes = Arc::clone(&probes);
                move || {
                    probes.fetch_add(1, Ordering::SeqCst);
                    latest.lock().unwrap().clone().context("offline")
                }
            }),
            install: Box::new({
                let installed = Arc::clone(&installed);
                let installs = Arc::clone(&installs);
                move |version| {
                    installs.lock().unwrap().push(version.to_string());
                    if install_works {
                        *installed.lock().unwrap() = Some(version.to_string());
                        Ok(())
                    } else {
                        bail!("download failed")
                    }
                }
            }),
            read_installed: Box::new({
                let installed = Arc::clone(&installed);
                move || installed.lock().unwrap().clone()
            }),
            automatic_enabled: Box::new({
                let automatic = Arc::clone(&automatic);
                move || *automatic.lock().unwrap()
            }),
            read_probe_failure_since: Box::new({
                let streak = Arc::clone(&streak);
                move || *streak.lock().unwrap()
            }),
            write_probe_failure_since: Box::new({
                let streak = Arc::clone(&streak);
                move |since| {
                    *streak.lock().unwrap() = since;
                    Ok(())
                }
            }),
            now: Box::new({
                let now = Arc::clone(&now);
                move || *now.lock().unwrap()
            }),
        };
        Rig {
            engine: Engine::new(deps, Arc::clone(&shared)),
            shared,
            installed,
            latest,
            automatic,
            now,
            probes,
            installs,
            streak,
        }
    }

    impl Rig {
        /// Exercise the production file reader and locked writer at startup,
        /// including a missing or broken file. Each call starts a new Engine
        /// over the same fake network/installer, never the installed app.
        fn with_state_file(mut self, path: &Path) -> Self {
            self.engine.deps.automatic_enabled = Box::new({
                let path = path.to_path_buf();
                move || automatic_updates_enabled(&path)
            });
            self.engine.deps.read_probe_failure_since = Box::new({
                let path = path.to_path_buf();
                move || read_probe_failure_since(&path)
            });
            self.engine.deps.write_probe_failure_since = Box::new({
                let path = path.to_path_buf();
                move |since| {
                    state::update_state(&path, |state| {
                        state.automatic_probe_failure_since = since;
                    })
                    .map(|_| ())
                }
            });
            let Engine { deps, shared, .. } = self.engine;
            self.engine = Engine::new(deps, shared);
            self
        }

        fn published(&self) -> UpdaterState {
            self.shared.subscribe().borrow().clone()
        }

        fn advance(&self, by: Duration) {
            let mut now = self.now.lock().unwrap();
            *now += by;
        }
    }

    /// Spec: the first wake checks, a newer release is installed in the
    /// background, and the published state shows nothing but the result:
    /// the Installed record now names a version newer than the running one.
    ///
    /// "Nothing on screen changes while it works" is the automatic run's
    /// whole contract, so the activity must stay idle throughout and the
    /// update marker must come from the record alone.
    #[farhelm_testtrace::test]
    fn an_automatic_check_installs_a_newer_release_silently() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), true);
        // A watch channel keeps only its latest value, so what the app bar
        // could see mid-run is sampled from inside the probe and the
        // installer, while the run is at those steps.
        let seen = Arc::new(Mutex::new(Vec::new()));
        let probe_view = Arc::clone(&rig.shared);
        let probe_seen = Arc::clone(&seen);
        rig.engine.deps.probe = Box::new(move || {
            probe_seen
                .lock()
                .unwrap()
                .push(probe_view.subscribe().borrow().activity.clone());
            Ok("1.1.0".to_string())
        });
        let install_view = Arc::clone(&rig.shared);
        let install_seen = Arc::clone(&seen);
        let installed = Arc::clone(&rig.installed);
        rig.engine.deps.install = Box::new(move |version| {
            install_seen
                .lock()
                .unwrap()
                .push(install_view.subscribe().borrow().activity.clone());
            *installed.lock().unwrap() = Some(version.to_string());
            Ok(())
        });
        rig.engine.wake_once();
        assert_eq!(*seen.lock().unwrap(), [Activity::Idle, Activity::Idle]);
        let state = rig.published();
        assert_eq!(state.activity, Activity::Idle);
        assert_eq!(state.installed.as_deref(), Some("1.1.0"));
        assert!(state.update_ready());
    }

    /// Spec: an automatic check whose install is refused as unverifiable
    /// publishes the reinstall notice even though no user is
    /// watching; a later check that finds nothing newer clears it, and so
    /// does one that verifies and installs. An ordinary install failure (a
    /// failed download) neither sets nor clears it.
    ///
    /// Why: an app that cannot verify its updates has stopped updating, and
    /// an automatic check's failures otherwise go only to the log, so this
    /// is the one failure the user must see without asking (SPEC.md,
    /// "Installation and updates"). A notice that never cleared would cry
    /// wolf after a fixed release; one a network blip set would be wrong.
    #[farhelm_testtrace::test]
    fn an_unverifiable_release_leaves_a_lasting_reinstall_notice() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), false);
        rig.engine.deps.install = Box::new(|_version| {
            Err(UnverifiableRelease("no key in the ring verifies it".to_string()).into())
        });
        rig.engine.wake_once();
        let state = rig.published();
        assert!(state.needs_reinstall, "set by an automatic check");
        assert_eq!(state.activity, Activity::Idle, "no user was watching");
        assert!(readout(&state).needs_reinstall);
        assert!(readout(&state).tooltip.contains(REINSTALL_COMMAND));

        // A plain failure later leaves it as it was.
        rig.engine.deps.install = Box::new(|_version| bail!("download failed"));
        rig.advance(CHECK_INTERVAL);
        rig.engine.wake_once();
        assert!(
            rig.published().needs_reinstall,
            "a failed download is not news"
        );

        // Nothing newer: nothing this app cannot install.
        *rig.latest.lock().unwrap() = Some("1.0.0".to_string());
        rig.advance(CHECK_INTERVAL);
        rig.engine.wake_once();
        assert!(
            !rig.published().needs_reinstall,
            "cleared when nothing newer"
        );

        // Set again, then cleared by a verified install.
        *rig.latest.lock().unwrap() = Some("1.1.0".to_string());
        rig.engine.deps.install = Box::new(|_version| {
            Err(UnverifiableRelease("signed for another version".to_string()).into())
        });
        rig.advance(CHECK_INTERVAL);
        rig.engine.wake_once();
        assert!(rig.published().needs_reinstall);
        let installed = Arc::clone(&rig.installed);
        rig.engine.deps.install = Box::new(move |version| {
            *installed.lock().unwrap() = Some(version.to_string());
            Ok(())
        });
        rig.engine.shared.check_now();
        rig.engine.wake_once();
        assert!(
            !rig.published().needs_reinstall,
            "cleared by a verified install"
        );

        // A verified install that ends at the running version (Installed
        // named an older version the user pinned) clears it too, which no
        // change to a newer Installed version would.
        let mut rig = make_rig("1.0.0", Some("0.9.0"), Some("1.0.0"), true);
        let install = std::mem::replace(
            &mut rig.engine.deps.install,
            Box::new(|_version| {
                Err(UnverifiableRelease("signed for another version".to_string()).into())
            }),
        );
        rig.engine.wake_once();
        assert!(rig.published().needs_reinstall);
        rig.engine.deps.install = install;
        rig.engine.shared.check_now();
        rig.engine.wake_once();
        let state = rig.published();
        assert_eq!(state.installed.as_deref(), Some("1.0.0"));
        assert!(!state.update_ready());
        assert!(
            !state.needs_reinstall,
            "cleared by a verified install of the running version"
        );
    }

    /// Spec: a check the user asked for sets the reinstall notice too, and a
    /// newer Farhelm installed some other way (the user following the
    /// notice) clears it at the next wake, without a check.
    ///
    /// Why: with an update waiting, automatic checks stop, so no check
    /// would clear a notice that asks for a reinstall the user already did.
    #[farhelm_testtrace::test]
    fn a_reinstall_from_a_terminal_clears_the_notice() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), false);
        *rig.automatic.lock().unwrap() = false;
        rig.engine.deps.install = Box::new(|_version| {
            Err(UnverifiableRelease("no key in the ring verifies it".to_string()).into())
        });
        rig.engine.shared.check_now();
        rig.engine.wake_once();
        assert!(rig.published().needs_reinstall, "set by the user's check");

        let probes = rig.probes.load(Ordering::SeqCst);
        *rig.installed.lock().unwrap() = Some("1.1.0".to_string());
        rig.engine.wake_once();
        let state = rig.published();
        assert!(!state.needs_reinstall, "cleared by the newer install");
        assert!(state.update_ready());
        assert_eq!(rig.probes.load(Ordering::SeqCst), probes, "without a check");
    }

    /// Spec: a version installed from a terminal while the release check is
    /// out is what the decision compares against: an automatic run then
    /// installs nothing, and a newer version installed that way is never
    /// replaced by an older release.
    ///
    /// The probe is a network round trip; deciding from the record read
    /// before it would reinstall over the user's own update.
    #[farhelm_testtrace::test]
    fn a_version_installed_during_the_check_is_respected() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), true);
        let installed = Arc::clone(&rig.installed);
        rig.engine.deps.probe = Box::new(move || {
            *installed.lock().unwrap() = Some("1.2.0-rc.1".to_string());
            Ok("1.1.0".to_string())
        });
        rig.engine.wake_once();
        assert!(rig.installs.lock().unwrap().is_empty());
        assert_eq!(rig.published().installed.as_deref(), Some("1.2.0-rc.1"));

        // Even a check the user asked for does not replace it with an
        // older release; it answers that Farhelm is up to date.
        rig.shared.check_now();
        rig.engine.wake_once();
        assert!(rig.installs.lock().unwrap().is_empty());
        assert!(rig.published().update_ready());
    }

    /// Successful automatic checks and the first offline probe stay quiet;
    /// an install failure shows immediately without changing the daily retry.
    /// This distinguishes a download failure after `/latest` from the probe
    /// failure for which the user explicitly chose a three-day grace period.
    #[farhelm_testtrace::test]
    fn automatic_success_and_first_probe_failure_stay_quiet_but_install_failure_shows() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.0.0"), true);
        rig.engine.wake_once();
        assert!(rig.installs.lock().unwrap().is_empty());
        assert_eq!(rig.published().activity, Activity::Idle);

        let mut rig = make_rig("1.0.0", Some("1.0.0"), None, true);
        rig.engine.wake_once();
        assert_eq!(rig.published().activity, Activity::Idle, "an offline probe");

        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), false);
        rig.engine.wake_once();
        let failure = rig.published();
        assert!(
            matches!(failure.activity, Activity::Failed(_)),
            "a failed download"
        );
        assert_eq!(failure.notice_id, 1);
        assert!(
            readout(&failure)
                .tooltip
                .contains("installing Farhelm 1.1.0 did not finish")
        );
        assert_eq!(
            *rig.streak.lock().unwrap(),
            None,
            "a successful probe ends the offline streak"
        );
        assert!(!rig.published().update_ready());
        rig.advance(CHECK_INTERVAL);
        rig.engine.wake_once();
        assert_eq!(
            rig.installs.lock().unwrap().len(),
            2,
            "retried at the next check"
        );
        assert_eq!(
            rig.published().notice_id,
            2,
            "a later qualifying failure is a new notice"
        );
    }

    /// Only actual automatic probe failures start the clock. Exact elapsed
    /// time, including subsecond precision, must distinguish just-before,
    /// exactly-at and just-after the three-day threshold without early notice.
    #[farhelm_testtrace::test]
    fn automatic_probe_failures_show_at_the_exact_three_day_boundary() {
        for (elapsed, visible) in [
            (PROBE_FAILURE_NOTICE_AFTER - Duration::from_nanos(1), false),
            (PROBE_FAILURE_NOTICE_AFTER, true),
            (PROBE_FAILURE_NOTICE_AFTER + Duration::from_nanos(1), true),
        ] {
            let mut rig = make_rig("1.0.0", Some("1.0.0"), None, true);
            rig.advance(Duration::from_nanos(123_456_789));
            let start = *rig.now.lock().unwrap();
            rig.engine.wake_once();
            assert_eq!(rig.published().activity, Activity::Idle);
            assert_eq!(*rig.streak.lock().unwrap(), Some(start));
            rig.advance(elapsed);
            rig.engine.wake_once();
            assert_eq!(rig.probes.load(Ordering::SeqCst), 2);
            assert_eq!(
                matches!(rig.published().activity, Activity::Failed(_)),
                visible,
                "{elapsed:?}"
            );
            assert_eq!(rig.published().notice_id, u64::from(visible));
        }
    }

    /// A long sleep/quit/off gap is not evidence that the site kept failing.
    /// The first failing probe after ten quiet days starts a new streak now.
    #[farhelm_testtrace::test]
    fn a_long_gap_without_checks_does_not_start_an_offline_notice_clock() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.0.0"), true);
        rig.engine.wake_once();
        assert_eq!(*rig.streak.lock().unwrap(), None);
        rig.advance(CHECK_INTERVAL * 10);
        *rig.latest.lock().unwrap() = None;
        rig.engine.wake_once();
        assert_eq!(rig.published().activity, Activity::Idle);
        assert_eq!(*rig.streak.lock().unwrap(), Some(*rig.now.lock().unwrap()));
    }

    /// An install failure must survive a later nonqualifying probe failure,
    /// including its dismissed state. Only a later qualifying failure gets a
    /// new identity; an equal reason is insufficient to distinguish the runs.
    #[farhelm_testtrace::test]
    fn automatic_notice_survives_failed_runs_and_dismissal_until_a_new_qualifying_failure() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), false);
        rig.engine.wake_once();
        let failure = rig.published();
        assert!(matches!(failure.activity, Activity::Failed(_)));
        let mut feedback = crate::app_updater::InlineFeedback::default();
        feedback.observe(&failure.activity, failure.notice_id);
        feedback.dismiss(&failure.activity, failure.notice_id);
        let dismissed = feedback.clone();
        rig.advance(CHECK_INTERVAL);
        *rig.latest.lock().unwrap() = None;
        rig.engine.wake_once();
        let retained = rig.published();
        assert_eq!(retained.activity, failure.activity);
        assert_eq!(retained.notice_id, failure.notice_id);
        assert!(
            !feedback.observe(&retained.activity, retained.notice_id),
            "no new identity or activity after dismissal"
        );
        assert_eq!(
            feedback, dismissed,
            "the exact dismissal survives the failed run"
        );
        rig.advance(PROBE_FAILURE_NOTICE_AFTER);
        rig.engine.wake_once();
        let later = rig.published();
        assert!(matches!(later.activity, Activity::Failed(_)));
        assert_eq!(later.notice_id, failure.notice_id + 1);
        assert!(
            readout(&later)
                .tooltip
                .contains("did not say which release")
        );
        feedback.observe(&later.activity, later.notice_id);
        let mut fresh = crate::app_updater::InlineFeedback::default();
        fresh.observe(&later.activity, later.notice_id);
        assert_eq!(feedback, fresh, "the later notice has no old suppression");
        // Expired/dismissed old callbacks cannot hide this newly shown result.
        feedback.dismiss(&failure.activity, failure.notice_id);
        assert_eq!(
            feedback, fresh,
            "a stale click cannot dismiss the new notice"
        );
    }

    /// Successful automatic and manual checks both end the streak and clear
    /// the sticky failure. Manual progress/result wins over an older notice,
    /// while ordinary manual failures do not manufacture an automatic streak.
    #[farhelm_testtrace::test]
    fn success_clears_automatic_notices_and_manual_failures_take_precedence() {
        for manual in [false, true] {
            let mut rig = make_rig("1.0.0", Some("1.0.0"), None, true);
            rig.engine.wake_once();
            rig.advance(PROBE_FAILURE_NOTICE_AFTER);
            rig.engine.wake_once();
            let first = rig.published();
            assert!(matches!(first.activity, Activity::Failed(_)));
            if manual {
                rig.shared.check_now();
            } else {
                rig.advance(CHECK_INTERVAL);
            }
            let shared = Arc::clone(&rig.shared);
            rig.engine.deps.probe = Box::new(move || {
                assert_eq!(
                    shared.subscribe().borrow().activity,
                    if manual {
                        Activity::Checking
                    } else {
                        first.activity.clone()
                    }
                );
                Ok("1.0.0".into())
            });
            rig.engine.wake_once();
            assert_eq!(*rig.streak.lock().unwrap(), None);
            assert_eq!(
                rig.published().activity,
                if manual {
                    Activity::UpToDate("1.0.0".into())
                } else {
                    Activity::Idle
                }
            );
        }

        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), false);
        rig.engine.wake_once();
        let automatic = rig.published();
        *rig.latest.lock().unwrap() = None;
        rig.shared.check_now();
        let request = rig.published().notice_id;
        rig.engine.wake_once();
        assert_eq!(rig.published().notice_id, request);
        assert_eq!(request, automatic.notice_id + 1);
        let manual_failure = rig.published();
        assert!(matches!(manual_failure.activity, Activity::Failed(_)));
        let mut feedback = crate::app_updater::InlineFeedback::default();
        feedback.observe(&manual_failure.activity, manual_failure.notice_id);
        feedback.dismiss(&manual_failure.activity, manual_failure.notice_id);
        let dismissed = feedback.clone();
        assert!(
            readout(&rig.published())
                .tooltip
                .contains("did not say which release")
        );
        assert_eq!(
            *rig.streak.lock().unwrap(),
            None,
            "a manual-only failure starts no automatic streak"
        );
        rig.advance(CHECK_INTERVAL);
        rig.engine.wake_once();
        assert_eq!(
            rig.published().notice_id,
            request,
            "a young automatic failure preserves the user's result"
        );
        let retained = rig.published();
        assert_eq!(
            retained.activity, manual_failure.activity,
            "identity alone cannot prove the retained message survived"
        );
        feedback.observe(&retained.activity, retained.notice_id);
        assert_eq!(
            feedback, dismissed,
            "the dismissed replacement must stay dismissed"
        );
    }

    /// Joining the first automatic failure gives it the user's existing
    /// identity exactly once, even with repeated requests during the probe.
    /// It remains an automatic attempt for failure-history accounting.
    #[farhelm_testtrace::test]
    fn joining_an_automatic_failure_does_not_create_two_notices() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), None, true);
        assert_eq!(
            *rig.streak.lock().unwrap(),
            None,
            "this is the first failing automatic attempt"
        );
        let first_failure_time = *rig.now.lock().unwrap();
        let shared = Arc::clone(&rig.shared);
        rig.engine.deps.probe = Box::new(move || {
            assert_eq!(shared.subscribe().borrow().activity, Activity::Idle);
            shared.check_now();
            let id = shared.subscribe().borrow().notice_id;
            shared.check_now();
            assert_eq!(shared.subscribe().borrow().notice_id, id);
            assert_eq!(shared.subscribe().borrow().activity, Activity::Checking);
            bail!("offline")
        });
        rig.engine.wake_once();
        assert_eq!(rig.published().notice_id, 1);
        assert!(matches!(rig.published().activity, Activity::Failed(_)));
        assert!(!rig.shared.status.lock().unwrap().user_waiting);
        assert!(!rig.shared.status.lock().unwrap().requested);
        assert_eq!(
            rig.shared.status.lock().unwrap().probe_failure_since,
            Some(first_failure_time)
        );
        assert_eq!(
            *rig.streak.lock().unwrap(),
            Some(first_failure_time),
            "joining must not erase automatic origin"
        );
        rig.engine.deps.probe = Box::new(|| bail!("offline, now unjoined"));
        rig.advance(PROBE_FAILURE_NOTICE_AFTER);
        rig.engine.wake_once();
        assert_eq!(
            rig.published().notice_id,
            2,
            "the original start qualifies a later unjoined failure"
        );
        assert!(matches!(rig.published().activity, Activity::Failed(_)));
        assert_eq!(*rig.streak.lock().unwrap(), Some(first_failure_time));
    }

    /// A backwards clock resets the streak, including when it remains later
    /// than the first failure. A future timestamp loaded after restart has
    /// the same rule; neither may trigger an immediate overdue notice.
    #[farhelm_testtrace::test]
    fn backwards_and_future_clocks_restart_the_failure_streak() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), None, true);
        rig.engine.wake_once();
        rig.advance(CHECK_INTERVAL * 2);
        rig.engine.wake_once();
        *rig.now.lock().unwrap() -= CHECK_INTERVAL;
        let reset = *rig.now.lock().unwrap();
        rig.engine.wake_once();
        assert_eq!(*rig.streak.lock().unwrap(), Some(reset));
        assert_eq!(rig.published().activity, Activity::Idle);
        rig.advance(PROBE_FAILURE_NOTICE_AFTER);
        rig.engine.wake_once();
        assert!(matches!(rig.published().activity, Activity::Failed(_)));

        let future = SystemTime::UNIX_EPOCH + Duration::from_secs(9_000_000);
        let persisted = Arc::new(Mutex::new(Some(future)));
        let mut rig = make_rig_with_streak("1.0.0", Some("1.0.0"), None, true, persisted);
        rig.engine.wake_once();
        assert_eq!(*rig.streak.lock().unwrap(), Some(*rig.now.lock().unwrap()));
        assert_eq!(rig.published().activity, Activity::Idle);
    }

    /// The real file carries exact streak time across new workers. Missing,
    /// malformed and unreadable files mean no history, and failed writes do
    /// not panic or prevent the in-memory streak reaching its threshold.
    #[farhelm_testtrace::test]
    fn probe_failure_persistence_survives_restart_and_file_failures_remain_live() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(state::APP_STATE_FILE);
        let mut first = make_rig("1.0.0", Some("1.0.0"), None, true).with_state_file(&path);
        first.advance(Duration::from_nanos(123_456_789));
        first.engine.wake_once();
        let start = *first.now.lock().unwrap();
        assert_eq!(
            state::read_state(&path)
                .unwrap()
                .automatic_probe_failure_since,
            Some(start)
        );
        let mut restarted = make_rig("1.0.0", Some("1.0.0"), None, true).with_state_file(&path);
        *restarted.now.lock().unwrap() = start + PROBE_FAILURE_NOTICE_AFTER;
        restarted.engine.wake_once();
        assert!(matches!(
            restarted.published().activity,
            Activity::Failed(_)
        ));
        *restarted.latest.lock().unwrap() = Some("1.0.0".into());
        restarted.advance(CHECK_INTERVAL);
        restarted.engine.wake_once();
        assert_eq!(
            state::read_state(&path)
                .unwrap()
                .automatic_probe_failure_since,
            None
        );

        for fixture in ["missing", "missing-field", "unreadable", "malformed"] {
            let path = root.path().join(fixture);
            match fixture {
                "missing-field" => {
                    std::fs::write(&path, r#"{"webview_auth_generation":7}"#).unwrap()
                }
                "unreadable" => std::fs::create_dir(&path).unwrap(),
                "malformed" => {
                    std::fs::write(&path, r#"{"automatic_probe_failure_since":"bad"}"#).unwrap()
                }
                _ => assert!(!path.exists()),
            }
            let mut rig = make_rig("1.0.0", Some("1.0.0"), None, true).with_state_file(&path);
            assert_eq!(
                rig.shared.status.lock().unwrap().probe_failure_since,
                None,
                "{fixture}"
            );
            rig.engine.wake_once();
            assert_eq!(rig.published().activity, Activity::Idle, "{fixture}");
            rig.advance(PROBE_FAILURE_NOTICE_AFTER);
            rig.engine.wake_once();
            assert!(
                matches!(rig.published().activity, Activity::Failed(_)),
                "{fixture}"
            );
            assert!(!rig.shared.status.lock().unwrap().user_waiting, "{fixture}");
            assert_eq!(rig.probes.load(Ordering::SeqCst), 2, "{fixture}");
        }
    }

    /// Re-enabling resets both memory and disk even if no worker wake observed
    /// the off period. Re-enabling during a probe likewise starts any new
    /// failure now, without crediting the earlier disabled period.
    #[farhelm_testtrace::test]
    fn re_enabling_automatic_updates_clears_the_streak_even_during_a_probe() {
        for during_probe in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join(state::APP_STATE_FILE);
            let mut rig = make_rig("1.0.0", Some("1.0.0"), None, true).with_state_file(&path);
            let handle = UpdaterHandle {
                shared: Arc::clone(&rig.shared),
                bundle: root.path().join("unused-test-bundle"),
                state_path: path.clone(),
            };
            rig.engine.wake_once();
            assert!(
                state::read_state(&path)
                    .unwrap()
                    .automatic_probe_failure_since
                    .is_some()
            );
            rig.advance(PROBE_FAILURE_NOTICE_AFTER);
            if during_probe {
                let path = path.clone();
                rig.engine.deps.probe = Box::new(move || {
                    handle.set_automatic_updates(false).unwrap();
                    handle.set_automatic_updates(true).unwrap();
                    assert_eq!(
                        state::read_state(&path)
                            .unwrap()
                            .automatic_probe_failure_since,
                        None
                    );
                    bail!("offline after re-enable")
                });
            } else {
                handle.set_automatic_updates(false).unwrap();
                assert!(!handle.automatic_updates());
                rig.engine.wake_once();
                assert_eq!(
                    rig.probes.load(Ordering::SeqCst),
                    1,
                    "off means no automatic probe"
                );
                handle.set_automatic_updates(true).unwrap();
                assert_eq!(
                    state::read_state(&path)
                        .unwrap()
                        .automatic_probe_failure_since,
                    None
                );
                assert_eq!(rig.shared.status.lock().unwrap().probe_failure_since, None);
            }
            rig.engine.wake_once();
            assert_eq!(rig.published().activity, Activity::Idle, "{during_probe}");
            assert_eq!(
                state::read_state(&path)
                    .unwrap()
                    .automatic_probe_failure_since,
                Some(*rig.now.lock().unwrap())
            );
        }
    }

    /// A failed settings write cannot pretend to re-enable or clear history.
    /// A worker persistence failure has the opposite contract: it is logged,
    /// but the in-memory streak and subsequent notice must still work.
    #[farhelm_testtrace::test]
    fn failed_streak_and_setting_writes_preserve_their_distinct_contracts() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), None, true);
        rig.engine.deps.write_probe_failure_since = Box::new(|_| bail!("fixture write refused"));
        rig.engine.wake_once();
        let since = rig.shared.status.lock().unwrap().probe_failure_since;
        assert!(since.is_some());
        assert_eq!(
            *rig.streak.lock().unwrap(),
            None,
            "durable record really was not written"
        );
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("occupied-state");
        std::fs::create_dir(&path).unwrap();
        let handle = UpdaterHandle {
            shared: Arc::clone(&rig.shared),
            bundle: root.path().join("unused-test-bundle"),
            state_path: path,
        };
        assert!(handle.set_automatic_updates(true).is_err());
        assert_eq!(rig.shared.status.lock().unwrap().probe_failure_since, since);
        rig.advance(PROBE_FAILURE_NOTICE_AFTER);
        rig.engine.wake_once();
        assert!(matches!(rig.published().activity, Activity::Failed(_)));
        *rig.latest.lock().unwrap() = Some("1.0.0".into());
        rig.advance(CHECK_INTERVAL);
        rig.engine.wake_once();
        assert_eq!(rig.shared.status.lock().unwrap().probe_failure_since, None);
        assert_eq!(rig.published().activity, Activity::Idle);
    }

    /// Verification refusals keep only the existing reinstall warning on an
    /// automatic run, even after a probe streak reaches its threshold. The
    /// successful probe ends that streak; no install-failure notice is minted.
    #[farhelm_testtrace::test]
    fn automatic_verification_failure_keeps_only_the_reinstall_warning() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), None, true);
        rig.engine.wake_once();
        rig.advance(PROBE_FAILURE_NOTICE_AFTER);
        *rig.latest.lock().unwrap() = Some("1.1.0".into());
        rig.engine.deps.install =
            Box::new(|_| Err(UnverifiableRelease("fixture signature refused".into()).into()));
        rig.engine.wake_once();
        let state = rig.published();
        assert!(state.needs_reinstall);
        assert_eq!(state.activity, Activity::Idle);
        assert_eq!(state.notice_id, 0);
        assert_eq!(*rig.streak.lock().unwrap(), None);
        assert!(readout(&state).tooltip.ends_with(REINSTALL_COMMAND));
    }

    /// The install boundary always qualifies, even after a long offline
    /// streak, and repeated equal failures undo a dismissal exactly once.
    /// Successful installs clear both history and the sticky notice.
    #[farhelm_testtrace::test]
    fn install_failure_ends_the_probe_streak_and_equal_later_failure_reappears() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), None, false);
        rig.engine.wake_once();
        rig.advance(CHECK_INTERVAL);
        *rig.latest.lock().unwrap() = Some("1.1.0".into());
        rig.engine.wake_once();
        let first = rig.published();
        assert!(matches!(first.activity, Activity::Failed(_)));
        assert_eq!(*rig.streak.lock().unwrap(), None);
        let mut feedback = crate::app_updater::InlineFeedback::default();
        feedback.observe(&first.activity, first.notice_id);
        feedback.dismiss(&first.activity, first.notice_id);
        rig.advance(CHECK_INTERVAL);
        rig.engine.wake_once();
        let second = rig.published();
        assert_eq!(second.activity, first.activity);
        assert_eq!(second.notice_id, first.notice_id + 1);
        feedback.observe(&second.activity, second.notice_id);
        let mut fresh = crate::app_updater::InlineFeedback::default();
        fresh.observe(&second.activity, second.notice_id);
        assert_eq!(feedback, fresh, "the equal later notice is shown again");
        let installed = Arc::clone(&rig.installed);
        rig.engine.deps.install = Box::new(move |version| {
            *installed.lock().unwrap() = Some(version.into());
            Ok(())
        });
        rig.advance(CHECK_INTERVAL);
        rig.engine.wake_once();
        assert_eq!(rig.published().activity, Activity::Idle);
        assert!(rig.published().update_ready());
        assert!(!rig.shared.status.lock().unwrap().retain_failure);
    }

    /// Spec: a retained automatic failure clears when a newer version becomes
    /// installed outside the updater. Automatic checks stop while an update
    /// waits, so without this the stale "did not finish" notice would sit
    /// beside the red update-ready readout indefinitely; the very next wake
    /// that sees the newer Installed record must clear it without a check.
    #[farhelm_testtrace::test]
    fn an_external_install_clears_a_retained_automatic_failure() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), false);
        rig.engine.wake_once();
        assert!(
            matches!(rig.published().activity, Activity::Failed(_)),
            "premise: the automatic install failure is shown"
        );
        assert!(rig.shared.status.lock().unwrap().retain_failure);
        let installs = rig.installs.lock().unwrap().len();
        *rig.installed.lock().unwrap() = Some("1.1.0".into());
        rig.engine.wake_once();
        assert_eq!(
            rig.installs.lock().unwrap().len(),
            installs,
            "premise: the wake ran no check of its own"
        );
        let after = rig.published();
        assert_eq!(after.activity, Activity::Idle);
        assert!(after.update_ready());
        assert!(!rig.shared.status.lock().unwrap().retain_failure);
    }

    /// Spec: automatic checks happen about once a day of wall-clock time:
    /// not again on the next wake, and again once 24 hours have passed.
    ///
    /// Wall clock is the point: a timer that stops while the Mac sleeps
    /// would never reach 24 hours on a laptop that sleeps nightly. A clock
    /// set backwards counts as due rather than postponing checks for as
    /// long as it was moved.
    #[farhelm_testtrace::test]
    fn automatic_checks_follow_the_wall_clock_daily() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.0.0"), true);
        rig.engine.wake_once();
        assert_eq!(rig.probes.load(Ordering::SeqCst), 1);
        rig.advance(CHECK_INTERVAL - Duration::from_secs(1));
        rig.engine.wake_once();
        assert_eq!(rig.probes.load(Ordering::SeqCst), 1, "not due yet");
        rig.advance(Duration::from_secs(1));
        rig.engine.wake_once();
        assert_eq!(rig.probes.load(Ordering::SeqCst), 2, "a day has passed");

        *rig.now.lock().unwrap() -= Duration::from_secs(3600);
        rig.engine.wake_once();
        assert_eq!(
            rig.probes.load(Ordering::SeqCst),
            3,
            "the clock went backwards"
        );
    }

    /// Spec: with automatic updates off there are no automatic checks, but
    /// a check the user asks for still runs and installs.
    #[farhelm_testtrace::test]
    fn turning_automatic_updates_off_leaves_on_demand_checks_working() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), true);
        *rig.automatic.lock().unwrap() = false;
        rig.engine.wake_once();
        assert_eq!(rig.probes.load(Ordering::SeqCst), 0);
        rig.shared.check_now();
        rig.engine.wake_once();
        assert_eq!(*rig.installs.lock().unwrap(), ["1.1.0"]);
        assert!(rig.published().update_ready());
    }

    /// Spec: while an installed update waits for a restart, the automatic
    /// check does nothing, while a check the user asks for still installs a
    /// release newer than the waiting one.
    ///
    /// Holding back saves a download a day and keeps a third version from
    /// replacing the folder of the version still running; the user's own
    /// request is the deliberate exception.
    #[farhelm_testtrace::test]
    fn one_update_waits_at_a_time_unless_the_user_asks() {
        let mut rig = make_rig("1.0.0", Some("1.1.0"), Some("1.2.0"), true);
        rig.engine.wake_once();
        assert_eq!(
            rig.probes.load(Ordering::SeqCst),
            0,
            "an update already waits"
        );
        rig.shared.check_now();
        rig.engine.wake_once();
        assert_eq!(*rig.installs.lock().unwrap(), ["1.2.0"]);
    }

    /// Spec: a version installed from a terminal shows up on the next wake,
    /// with no check needed.
    #[farhelm_testtrace::test]
    fn a_version_installed_by_hand_shows_up_on_the_next_wake() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.0.0"), true);
        rig.engine.wake_once();
        *rig.installed.lock().unwrap() = Some("1.1.0".to_string());
        rig.engine.wake_once();
        let state = rig.published();
        assert!(state.update_ready());
        assert_eq!(
            rig.probes.load(Ordering::SeqCst),
            1,
            "the second wake did not check"
        );
    }

    /// Spec: a check the user asked for ends in a visible outcome: up to
    /// date with the latest version, failed with a reason, or a finished
    /// install, which the update marker shows instead of any words.
    #[farhelm_testtrace::test]
    fn an_on_demand_check_ends_visibly() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.0.0"), true);
        rig.shared.check_now();
        assert_eq!(
            rig.published().activity,
            Activity::Idle,
            "not picked up yet"
        );
        rig.engine.wake_once();
        assert_eq!(
            rig.published().activity,
            Activity::UpToDate("1.0.0".to_string())
        );

        *rig.latest.lock().unwrap() = None;
        rig.shared.check_now();
        rig.engine.wake_once();
        assert!(matches!(rig.published().activity, Activity::Failed(_)));

        *rig.latest.lock().unwrap() = Some("1.1.0".to_string());
        rig.shared.check_now();
        rig.engine.wake_once();
        let state = rig.published();
        assert_eq!(state.activity, Activity::Idle);
        assert!(state.update_ready());
    }

    /// Spec: a check the user asked for that reinstalls the running version
    /// (Installed named an older release the user had pinned) still ends in
    /// a visible answer: up to date, since no update marker appears.
    #[farhelm_testtrace::test]
    fn reinstalling_the_running_version_answers_up_to_date() {
        let mut rig = make_rig("1.1.0", Some("1.0.0"), Some("1.1.0"), true);
        *rig.automatic.lock().unwrap() = false;
        rig.shared.check_now();
        rig.engine.wake_once();
        assert_eq!(*rig.installs.lock().unwrap(), ["1.1.0"]);
        let state = rig.published();
        assert!(!state.update_ready());
        assert_eq!(state.activity, Activity::UpToDate("1.1.0".to_string()));
    }

    /// Spec: an install whose Installed record does not name the target
    /// afterwards is a failure, even when the installer reported success.
    ///
    /// `curl … | sh` exits 0 when the download fails; the record is the
    /// only trustworthy evidence, so the exit status is never enough.
    #[farhelm_testtrace::test]
    fn success_is_judged_by_the_installed_record() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), true);
        // Replace the installer with one that claims success and installs
        // nothing.
        rig.engine.deps.install = Box::new(|_| Ok(()));
        rig.shared.check_now();
        rig.engine.wake_once();
        assert!(matches!(rig.published().activity, Activity::Failed(_)));
        assert!(!rig.published().update_ready());
    }

    /// Spec: a user's request that arrives during an automatic run joins
    /// it: the run's current step is shown at once, its outcome is the
    /// user's answer, and no second run starts.
    ///
    /// The installer blocks the worker for minutes; a request queued behind
    /// it would otherwise show nothing until a duplicate run had finished.
    #[farhelm_testtrace::test]
    fn a_request_during_an_automatic_run_joins_it() {
        let rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), false);
        let Rig {
            mut engine,
            shared,
            probes,
            ..
        } = rig;
        // The fake installer is where the run is "in progress"; ask from
        // there, as a click during a long install would.
        let asked = Arc::clone(&shared);
        let seen_during = Arc::new(Mutex::new(None));
        let seen = Arc::clone(&seen_during);
        engine.deps.install = Box::new(move |_| {
            asked.check_now();
            *seen.lock().unwrap() = Some(asked.subscribe().borrow().activity.clone());
            bail!("download failed")
        });
        engine.wake_once();
        assert_eq!(
            *seen_during.lock().unwrap(),
            Some(Activity::Installing("1.1.0".to_string()))
        );
        assert!(matches!(
            shared.subscribe().borrow().activity,
            Activity::Failed(_)
        ));
        assert_eq!(probes.load(Ordering::SeqCst), 1);
        engine.wake_once();
        assert_eq!(
            probes.load(Ordering::SeqCst),
            1,
            "the request did not start a second run"
        );
    }

    /// Spec: the outcome a user saw stays until the next run starts, and an
    /// automatic run clears it rather than leaving an old answer up.
    #[farhelm_testtrace::test]
    fn an_old_outcome_clears_when_the_next_run_starts() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), None, true);
        rig.shared.check_now();
        rig.engine.wake_once();
        assert!(matches!(rig.published().activity, Activity::Failed(_)));
        *rig.latest.lock().unwrap() = Some("1.0.0".to_string());
        rig.advance(CHECK_INTERVAL);
        rig.engine.wake_once();
        assert_eq!(rig.published().activity, Activity::Idle);
    }

    /// An identical final answer must still notify a watch consumer that missed
    /// checking. Repeated requests joining one visible run keep its identity;
    /// only a later request after completion starts a fresh presentation lifetime.
    #[farhelm_testtrace::test]
    fn notice_identity_survives_coalesced_identical_answers() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.0.0"), true);
        let mut receiver = rig.shared.subscribe();
        assert_eq!(receiver.borrow_and_update().notice_id, 0);
        rig.shared.check_now();
        let first_id = rig.published().notice_id;
        assert_eq!(first_id, 1);
        rig.shared.check_now();
        assert_eq!(rig.published().notice_id, first_id);
        rig.engine.wake_once();
        let first = receiver.borrow_and_update().clone();
        assert_eq!(first.activity, Activity::UpToDate("1.0.0".into()));

        rig.shared.check_now();
        rig.engine.wake_once();
        assert!(receiver.has_changed().unwrap());
        let second = receiver.borrow_and_update().clone();
        assert_eq!(second.activity, first.activity);
        assert_eq!(second.notice_id, first.notice_id + 1);
        rig.advance(CHECK_INTERVAL);
        rig.engine.wake_once();
        assert_eq!(rig.published().notice_id, second.notice_id);
    }

    /// Readiness can predate a check, so only a successful user-visible outcome
    /// may announce it. New checks, failures and restart failures supersede the
    /// captured answer without changing the red readout's independent readiness.
    #[farhelm_testtrace::test]
    fn ready_completion_is_captured_only_for_successful_user_checks() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), true);
        rig.shared.check_now();
        assert!(!rig.published().user_update_ready);
        rig.engine.wake_once();
        let installed = rig.published();
        assert_eq!(installed.activity, Activity::Idle);
        assert!(installed.update_ready());
        assert!(installed.user_update_ready);
        assert!(crate::app_updater::announce_update_ready(&readout(
            &installed
        )));

        rig.shared.check_now();
        assert!(!rig.published().user_update_ready);
        rig.engine.wake_once();
        assert_eq!(rig.published().activity, Activity::UpToDate("1.1.0".into()));
        assert!(rig.published().user_update_ready);
        rig.shared.show_outcome(Activity::RestartFailed);
        assert!(!rig.published().user_update_ready);

        *rig.latest.lock().unwrap() = None;
        rig.shared.check_now();
        rig.engine.wake_once();
        let failed = rig.published();
        assert!(failed.update_ready());
        assert!(matches!(failed.activity, Activity::Failed(_)));
        assert!(!failed.user_update_ready);
        assert!(!crate::app_updater::announce_update_ready(&readout(
            &failed
        )));
    }

    /// An automatic or terminal installation can make the readout red without
    /// completing a user request. Neither may acquire the completion announcement;
    /// a later automatic run also clears an earlier captured user outcome.
    #[farhelm_testtrace::test]
    fn background_readiness_never_becomes_user_completion() {
        let mut automatic = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), true);
        automatic.engine.wake_once();
        assert!(automatic.published().update_ready());
        assert_eq!(automatic.published().notice_id, 0);
        assert!(!automatic.published().user_update_ready);

        let mut external = make_rig("1.0.0", Some("1.0.0"), Some("1.0.0"), true);
        external.engine.wake_once();
        *external.installed.lock().unwrap() = Some("1.1.0".into());
        external.engine.wake_once();
        assert!(external.published().update_ready());
        assert_eq!(external.published().notice_id, 0);
        assert!(!external.published().user_update_ready);

        external.shared.check_now();
        external.engine.wake_once();
        assert!(external.published().user_update_ready);
        *external.installed.lock().unwrap() = Some("1.0.0".into());
        external.advance(CHECK_INTERVAL);
        external.engine.wake_once();
        assert!(!external.published().user_update_ready);
        assert_eq!(external.published().activity, Activity::Idle);
    }

    /// Record-only loss and recovery cannot replay an old user completion.
    /// Disable automatic checks so a new run cannot accidentally supply the
    /// invalidation this regression specifically requires from record refresh.
    #[farhelm_testtrace::test]
    fn record_changes_invalidate_old_completion_without_starting_a_check() {
        for missing in [Some("1.0.0".to_owned()), None] {
            let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), true);
            *rig.automatic.lock().unwrap() = false;
            assert!(!*rig.automatic.lock().unwrap());
            rig.shared.check_now();
            rig.engine.wake_once();
            let completed = rig.published();
            assert!(completed.update_ready());
            assert!(completed.user_update_ready);
            assert_eq!(rig.probes.load(Ordering::SeqCst), 1);
            rig.engine.wake_once();
            assert!(
                rig.published().user_update_ready,
                "unchanged record preserves provenance"
            );

            *rig.installed.lock().unwrap() = missing;
            rig.engine.wake_once();
            assert!(!rig.published().update_ready());
            assert!(!rig.published().user_update_ready);
            *rig.installed.lock().unwrap() = Some("1.1.0".into());
            rig.engine.wake_once();
            let recovered = rig.published();
            assert!(recovered.update_ready());
            assert!(!recovered.user_update_ready);
            assert!(!crate::app_updater::announce_update_ready(&readout(
                &recovered
            )));
            assert_eq!(recovered.notice_id, completed.notice_id);
            assert_eq!(recovered.activity, completed.activity);
            assert_eq!(
                rig.probes.load(Ordering::SeqCst),
                1,
                "record refreshes did not probe"
            );
        }
    }

    // ---- the readout ----

    /// An installed release with a controlled activity and Installed record.
    /// Keeping these inputs independent exercises the rule that a hand-installed
    /// update remains ready while a user-started check reports another outcome.
    fn state(installed: Option<&str>, activity: Activity) -> UpdaterState {
        UpdaterState {
            running: "1.0.0".to_string(),
            installed: installed.map(str::to_string),
            activity,
            notice_id: 0,
            user_update_ready: false,
            needs_reinstall: false,
        }
    }

    /// Spec: the readout is red exactly when the installed version is newer
    /// than the running one, whatever the updater is doing, and its hover
    /// then says which version is installed and that a restart finishes the
    /// update.
    ///
    /// A version installed by hand must light the marker too, and an
    /// installed version that is not newer (the running one, or an older
    /// one) must not.
    #[farhelm_testtrace::test]
    fn the_readout_is_red_exactly_when_a_newer_version_is_installed() {
        let ready = readout(&state(Some("1.1.0"), Activity::Idle));
        assert!(ready.update_ready);
        assert_eq!(
            ready.tooltip,
            "Farhelm 1.1.0 is installed. Restart Farhelm to finish the update. Select this to open the menu for Restart to update or What's new."
        );
        assert_eq!(
            readout(&state(
                Some("1.1.0"),
                Activity::UpToDate("1.1.0".to_string())
            ))
            .tooltip,
            ready.tooltip,
            "an up-to-date answer with an update waiting still points at the restart"
        );
        assert!(readout(&state(Some("1.1.0"), Activity::Checking)).update_ready);
        for installed in [Some("1.0.0"), Some("0.9.0"), None] {
            let plain = readout(&state(installed, Activity::Idle));
            assert!(!plain.update_ready, "{installed:?}");
            assert_eq!(
                plain.tooltip,
                "This is Farhelm 1.0.0. When a newer version is installed, this turns red and opens the Restart to update / What's new menu."
            );
        }
    }

    /// Spec: a Restart to update that could not start says so in its own
    /// words, not as a failed check, and tells the user the way that works.
    #[farhelm_testtrace::test]
    fn a_failed_restart_has_its_own_words() {
        let failed = readout(&state(Some("1.1.0"), Activity::RestartFailed));
        assert!(failed.update_ready);
        assert_eq!(
            failed.tooltip,
            "Farhelm could not restart itself. Quit and reopen it to finish updating to Farhelm 1.1.0."
        );
    }

    /// Spec: a check the user started ends visibly in the hover: checking,
    /// installing which version, up to date, or failed with the reason;
    /// a failure with an update already waiting still mentions the restart.
    #[farhelm_testtrace::test]
    fn the_readout_hover_carries_a_user_started_check() {
        assert_eq!(
            readout(&state(None, Activity::Checking)).tooltip,
            "Farhelm is checking for a newer version."
        );
        assert_eq!(
            readout(&state(None, Activity::Installing("1.2.0".to_string()))).tooltip,
            "Farhelm 1.2.0 is being installed in the background."
        );
        assert_eq!(
            readout(&state(
                Some("1.0.0"),
                Activity::UpToDate("1.0.0".to_string())
            ))
            .tooltip,
            "Farhelm is up to date. Farhelm 1.0.0 is the latest release."
        );
        assert_eq!(
            readout(&state(
                Some("1.0.0"),
                Activity::Failed("offline".to_string())
            ))
            .tooltip,
            "The update check failed: offline."
        );
        let failed_with_update = readout(&state(
            Some("1.1.0"),
            Activity::Failed("offline".to_string()),
        ));
        assert!(failed_with_update.update_ready);
        assert_eq!(
            failed_with_update.tooltip,
            "The update check failed: offline. Farhelm 1.1.0 is installed. Restart Farhelm to finish the update. Select this to open the menu for Restart to update or What's new."
        );
    }

    /// Every published activity reaches the pure inline selector unchanged.
    /// Installation readiness owns a successful answer, and the independent
    /// reinstall warning must not erase progress, failure, or either glyph.
    #[farhelm_testtrace::test]
    fn inline_activity_survives_the_readouts_warning_and_update_markers() {
        use crate::app_updater::{InlineStatus, inline_status};

        for ready in [false, true] {
            for warning in [false, true] {
                for (activity, expected) in [
                    (Activity::Idle, None),
                    (Activity::Checking, Some(InlineStatus::Checking)),
                    (
                        Activity::Installing("1.2.0".into()),
                        Some(InlineStatus::Installing),
                    ),
                    (
                        Activity::UpToDate("1.2.0".into()),
                        (!ready).then_some(InlineStatus::UpToDate),
                    ),
                    (
                        Activity::Failed("offline".into()),
                        Some(InlineStatus::Failed),
                    ),
                    (Activity::RestartFailed, None),
                ] {
                    let mut published = state(
                        Some(if ready { "1.1.0" } else { "1.0.0" }),
                        activity.clone(),
                    );
                    published.needs_reinstall = warning;
                    let shown = readout(&published);
                    assert_eq!(shown.activity, activity);
                    assert_eq!(shown.update_ready, ready);
                    assert_eq!(shown.needs_reinstall, warning);
                    assert_eq!(
                        inline_status(&shown.activity, shown.update_ready, None),
                        expected,
                        "{activity:?}, ready={ready}, warning={warning}"
                    );
                }
            }
        }
    }

    /// Verification failure must remain actionable during every update activity.
    /// Whole sentences avoid burying the current activity inside parentheses;
    /// the command stays last so copying it cannot pick up sentence punctuation.
    #[farhelm_testtrace::test]
    fn the_reinstall_notice_keeps_each_activity_and_ends_with_the_command() {
        for activity in [
            Activity::Idle,
            Activity::Checking,
            Activity::Installing("1.2.0".to_owned()),
            Activity::UpToDate("1.1.0".to_owned()),
            Activity::Failed("offline.".to_owned()),
            Activity::RestartFailed,
        ] {
            let mut state = state(Some("1.1.0"), activity);
            let activity_text = readout(&state).tooltip;
            state.needs_reinstall = true;
            let shown = readout(&state);
            assert!(shown.update_ready);
            assert!(shown.needs_reinstall);
            assert_eq!(
                shown.tooltip,
                format!(
                    "This Farhelm can no longer verify its updates, so it cannot update itself. {activity_text} Reinstall it by running this in a terminal: {REINSTALL_COMMAND}"
                )
            );
        }
    }
}
