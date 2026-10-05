//! The installed Mac app's updater: it checks GitHub for the latest stable
//! release, runs the ordinary installer in the background when that release
//! is newer, and publishes what it is doing for the app bar to show.
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
//!
//! The network, the installer, the clock and the setting are all reached
//! through [`Deps`], plain functions the worker is handed, so the decisions
//! above are tested on any platform without a network or a real bundle.

use super::*;

use std::ffi::OsString;
use std::sync::Condvar;
use std::time::SystemTime;

use farhelm_helm::{build_is_newer, is_development_build};

/// Where GitHub answers which stable release is the latest: a redirect to
/// that release's tag. The same URL `install.sh` asks; prereleases are never
/// "latest", which is what keeps this updater on the stable channel.
const RELEASES_LATEST_URL: &str = "https://github.com/scode/farhelm/releases/latest";

/// The installer the README tells users to pipe to `sh`, from main.
const INSTALLER_URL: &str =
    "https://raw.githubusercontent.com/scode/farhelm/main/scripts/install.sh";

/// How often the worker wakes when nothing asks it to: often enough that an
/// update installed from a terminal shows up within about a minute, and
/// cheap, since a wake that is not due for a check only reads one small file.
const TICK: Duration = Duration::from_secs(60);

/// Wall-clock time between automatic checks.
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Bound on the latest-release probe, which is one small request.
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// Bound on downloading the installer script, which is tens of kilobytes.
const INSTALLER_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

/// The largest installer script accepted. The real one is far smaller; the
/// cap only keeps a misbehaving server from filling memory.
const INSTALLER_MAX_BYTES: usize = 1024 * 1024;

/// The environment variable that pins the installer to one release.
const VERSION_PIN_ENV: &str = "FARHELM_VERSION";

// ===== What the updater publishes ==========================================

/// What the updater is doing, as the app bar may show it.
///
/// Only a run the user asked for (or an automatic run a user's request
/// joined) is ever published as anything but [`Activity::Idle`]: an automatic
/// run changes nothing on screen until the Installed record does, and its
/// failures go to the log only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Activity {
    Idle,
    Checking,
    /// The installer is running for this version.
    Installing(String),
    /// The check finished and this version, the latest stable release, is
    /// not newer than what is installed.
    UpToDate(String),
    /// The check or the install failed, for this reason (one plain
    /// sentence for the readout's hover).
    Failed(String),
    /// Restart to update could not start the helper that reopens the app,
    /// so the app stayed open. Not a check outcome, so it has words of its
    /// own.
    RestartFailed,
}

/// The updater's whole published state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpdaterState {
    /// The version compiled into this app.
    pub(crate) running: String,
    /// The version `Contents/Versions/installed` names, when it could be
    /// read.
    pub(crate) installed: Option<String>,
    pub(crate) activity: Activity,
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
    /// before every automatic check and again before an automatic install,
    /// so nothing else needs telling. A write that fails leaves the file as
    /// it was and returns a short reason, so the dialog can keep showing the
    /// choice that is actually in force rather than one that was not saved.
    pub(crate) fn set_automatic_updates(&self, on: bool) -> Result<(), String> {
        state::update_state(&self.state_path, |state| {
            state.install_updates_automatically = Some(on);
        })
        .map(|_| ())
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
/// the helper did.
fn spawn_relaunch_helper(
    pid: u32,
    bundle: &Path,
    opener: &Path,
    wait_tenths: u32,
    seen_alive: Option<&Path>,
) -> io::Result<Child> {
    let mut command = Command::new("/bin/sh");
    command
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
    command.spawn()
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
    let deps = Deps {
        probe: Box::new(probe_latest_release),
        install: Box::new(run_installer),
        read_installed: Box::new(move || read_installed_record(&installed_record)),
        automatic_enabled: Box::new(move || automatic_updates_enabled(&state_path)),
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
    /// Wall-clock now.
    pub(super) now: Box<dyn Fn() -> SystemTime + Send>,
}

// ===== The version readout ==================================================

/// How the sidebar's version readout presents the updater's state.
///
/// Pure, so every state's look and words are tested without a component
/// tree. `update_ready` turns the readout red with an up-arrow (SPEC.md
/// "Installation and updates"); the hover text is where a check the user
/// started ends visibly, and otherwise says what a restart would do.
pub(crate) fn readout(state: &UpdaterState) -> crate::app_updater::Readout {
    let ready = state.update_ready();
    let installed = state.installed.as_deref().unwrap_or_default();
    let ready_text = format!(
        "Farhelm {installed} is installed; restarting Farhelm finishes the update (select for restart to update or what's new)"
    );
    let tooltip = match &state.activity {
        Activity::Checking => "checking for a newer Farhelm…".to_string(),
        Activity::Installing(version) => {
            format!("installing Farhelm {version} in the background…")
        }
        Activity::Failed(reason) if ready => {
            format!("the update check failed: {reason}. {ready_text}")
        }
        Activity::Failed(reason) => format!("the update check failed: {reason}"),
        Activity::RestartFailed => format!(
            "Farhelm could not restart itself; quit and reopen it to finish updating to Farhelm {installed}"
        ),
        _ if ready => ready_text,
        Activity::UpToDate(latest) => {
            format!("Farhelm is up to date ({latest} is the latest release)")
        }
        Activity::Idle => crate::app_updater::idle_tooltip(&state.running),
    };
    crate::app_updater::Readout {
        update_ready: ready,
        tooltip,
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
    /// The outcome of the last run a user saw through, until the next run
    /// starts. Published while no run is visible.
    shown_outcome: Option<Activity>,
    /// A user asked for a check and has not yet been shown its outcome. The
    /// run in progress (or the next one) is then visible.
    user_waiting: bool,
    /// A user's request has not been picked up by the worker yet.
    requested: bool,
}

impl Shared {
    pub(super) fn new(running: &str) -> Self {
        let (published, _) = tokio::sync::watch::channel(UpdaterState {
            running: running.to_string(),
            installed: None,
            activity: Activity::Idle,
        });
        Self {
            status: Mutex::new(Status {
                running: running.to_string(),
                installed: None,
                current: Activity::Idle,
                shown_outcome: None,
                user_waiting: false,
                requested: false,
            }),
            wake: Condvar::new(),
            published,
        }
    }

    /// Ask for a check now, from any trigger the user has.
    ///
    /// When a run is already in progress the request joins it: from here on
    /// that run is shown (its current step at once), and its outcome is the
    /// answer. Otherwise the worker starts one.
    pub(super) fn check_now(&self) {
        let mut status = self.status.lock().expect("updater status poisoned");
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

impl Engine {
    pub(super) fn new(deps: Deps, shared: Arc<Shared>) -> Self {
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
            status.installed = installed;
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

    /// Set what the worker is doing and publish it.
    fn set_current(&self, activity: Activity) {
        let mut status = self.shared.status.lock().expect("updater status poisoned");
        status.current = activity;
        self.shared.publish(&status);
    }

    /// One check, and the install it calls for.
    fn run_check(&mut self) {
        self.last_check = Some((self.deps.now)());
        {
            let mut status = self.shared.status.lock().expect("updater status poisoned");
            status.requested = false;
            status.shown_outcome = None;
            status.current = Activity::Checking;
            self.shared.publish(&status);
        }
        let outcome = self.check_and_install();
        let installed = (self.deps.read_installed)();
        let mut status = self.shared.status.lock().expect("updater status poisoned");
        status.installed = installed;
        status.current = Activity::Idle;
        if status.user_waiting {
            status.user_waiting = false;
            // A finished install needs no words of its own: the update
            // marker shows it.
            status.shown_outcome = outcome.filter(|outcome| *outcome != Activity::Idle);
        } else if let Some(Activity::Failed(reason)) = &outcome {
            tracing::warn!("updater: automatic update failed: {reason}");
        }
        self.shared.publish(&status);
    }

    /// Probe, compare, and install when the latest release is newer than
    /// what is installed. The outcome is `Idle` after a successful install,
    /// `UpToDate` or `Failed` otherwise.
    fn check_and_install(&self) -> Option<Activity> {
        let latest = match (self.deps.probe)() {
            Ok(latest) => latest,
            Err(error) => {
                tracing::info!("updater: checking for the latest release failed: {error:#}");
                return Some(Activity::Failed(
                    "GitHub did not say which release is the latest".to_string(),
                ));
            }
        };
        // The probe can take a while, and the user may have run the
        // installer from a terminal meanwhile, so the decision uses the
        // Installed record as it is now, not as it was when the run began.
        // A newer version installed that way would otherwise be replaced
        // by an older release, and an automatic run must still hold back
        // once an update waits.
        let installed = (self.deps.read_installed)();
        let (baseline, automatic_should_wait) = {
            let mut status = self.shared.status.lock().expect("updater status poisoned");
            status.installed = installed;
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
            return Some(Activity::Idle);
        }
        if !build_is_newer(&latest, &baseline) {
            tracing::info!("updater: {latest} is the latest release; {baseline} is installed");
            return Some(Activity::UpToDate(latest));
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
            return Some(Activity::Idle);
        }
        tracing::info!("updater: installing {latest} (installed: {baseline})");
        self.set_current(Activity::Installing(latest.clone()));
        if let Err(error) = (self.deps.install)(&latest) {
            tracing::warn!("updater: the installer for {latest} reported: {error:#}");
        }
        if (self.deps.read_installed)().as_deref() == Some(latest.as_str()) {
            tracing::info!("updater: {latest} is installed; a restart finishes the update");
            let running = self
                .shared
                .status
                .lock()
                .expect("updater status poisoned")
                .running
                .clone();
            if build_is_newer(&latest, &running) {
                // The update marker says it from here on.
                Some(Activity::Idle)
            } else {
                // Installed went back up to the running version (it named
                // an older one the user pinned); no marker will appear, so
                // the user's check needs its own answer.
                Some(Activity::UpToDate(latest))
            }
        } else {
            Some(Activity::Failed(format!(
                "installing Farhelm {latest} did not finish (the app's log has the installer's output)"
            )))
        }
    }
}

// ===== The real probe and installer ========================================

/// Ask GitHub which stable release is the latest, the way `install.sh`
/// does: a HEAD request whose redirect names the tag, not followed.
fn probe_latest_release() -> anyhow::Result<String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("starting the updater's runtime")?;
    runtime.block_on(async {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(PROBE_TIMEOUT)
            .build()
            .context("building the update check's HTTP client")?;
        let response = client
            .head(RELEASES_LATEST_URL)
            .send()
            .await
            .context("asking GitHub for the latest release")?;
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        latest_from_redirect(response.status().as_u16(), location.as_deref())
    })
}

/// The latest release's version from the `releases/latest` answer: a
/// redirect whose `Location` ends in the tag (`…/releases/tag/v1.2.3`).
///
/// Anything else is an error rather than a guess: another status (GitHub
/// down, rate limited, no release yet), a missing `Location`, or a tag that
/// is not a release version. The leading `v` is dropped, because the
/// Installed record holds the bare version.
fn latest_from_redirect(status: u16, location: Option<&str>) -> anyhow::Result<String> {
    if !(300..400).contains(&status) {
        bail!("GitHub answered the latest-release request with HTTP {status}");
    }
    let location = location.context("GitHub's latest-release redirect had no Location")?;
    let tag = location
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("");
    let version = tag.strip_prefix('v').unwrap_or(tag);
    let parsed = semver::Version::parse(version)
        .with_context(|| format!("GitHub's latest release {tag:?} is not a release version"))?;
    if !parsed.pre.is_empty() {
        bail!("GitHub's latest release {tag:?} is not a stable release");
    }
    Ok(version.to_string())
}

/// Download the installer and run it pinned to `version`, logging its
/// output.
///
/// Downloaded to a file and run with `/bin/sh`, rather than `curl | sh`,
/// because a pipe exits 0 when the download fails (`sh` reads an empty
/// script); here a failed download is an error before anything runs. The
/// result is advisory: the caller judges success by the Installed record.
///
/// The installer must outlive the app: a quit mid-install is ordinary,
/// since nothing on screen says an install is running. So it gets its own
/// process group (no signal aimed at the app's group reaches it), stdin from
/// `/dev/null`, and its output in a file rather than a pipe. A pipe would
/// break when the app exits, and `install.sh` dies of SIGPIPE without its
/// cleanup, leaving its lock behind for every later install to refuse on.
/// The file is copied into the app's log after the installer exits; if the
/// app quits first, the directory holding the script and the file stays in
/// the temporary directory, which the system clears.
fn run_installer(version: &str) -> anyhow::Result<()> {
    let script = download_installer()?;
    let dir = private_temp_dir()?;
    let path = dir.join("install.sh");
    let log_path = dir.join("install.log");
    let outcome = (|| {
        std::fs::write(&path, &script).with_context(|| format!("writing {}", path.display()))?;
        let log =
            File::create(&log_path).with_context(|| format!("creating {}", log_path.display()))?;
        let log_for_stderr = log
            .try_clone()
            .context("sharing the installer's log file")?;
        let mut command = Command::new("/bin/sh");
        command
            .arg(&path)
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
    })();
    let _ = std::fs::remove_dir_all(&dir);
    outcome
}

/// The installer script's bytes, from main on GitHub over HTTPS.
fn download_installer() -> anyhow::Result<Vec<u8>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("starting the updater's runtime")?;
    runtime.block_on(async {
        let client = reqwest::Client::builder()
            .https_only(true)
            .timeout(INSTALLER_DOWNLOAD_TIMEOUT)
            .build()
            .context("building the installer download's HTTP client")?;
        let mut response = client
            .get(INSTALLER_URL)
            .send()
            .await
            .context("downloading the installer")?
            .error_for_status()
            .context("downloading the installer")?;
        // Read chunk by chunk so the cap holds while downloading: a whole
        // body buffered first would already be in memory when checked.
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.context("downloading the installer")? {
            if bytes.len() + chunk.len() > INSTALLER_MAX_BYTES {
                bail!("the downloaded installer is over {INSTALLER_MAX_BYTES} bytes, which cannot be it");
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            bail!("the downloaded installer is empty");
        }
        Ok(bytes)
    })
}

/// A fresh directory only this user can read, for the downloaded script.
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
/// `FARHELM_VERSION` is set afresh afterwards.
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

    /// Spec: the latest release is read from the redirect's tag, without
    /// its `v`, and anything that is not a stable release version is a
    /// failed check rather than a guess.
    #[farhelm_testtrace::test]
    fn the_latest_release_comes_from_the_redirect_tag() {
        let location = "https://github.com/scode/farhelm/releases/tag/v0.23.1";
        assert_eq!(latest_from_redirect(302, Some(location)).unwrap(), "0.23.1");
        assert_eq!(
            latest_from_redirect(302, Some("/scode/farhelm/releases/tag/0.23.1")).unwrap(),
            "0.23.1",
            "a host-relative Location and a bare tag"
        );
        assert!(
            latest_from_redirect(200, Some(location)).is_err(),
            "not a redirect"
        );
        assert!(latest_from_redirect(429, None).is_err(), "rate limited");
        assert!(latest_from_redirect(302, None).is_err(), "no Location");
        assert!(
            latest_from_redirect(302, Some("https://github.com/scode/farhelm/releases")).is_err(),
            "no tag"
        );
        assert!(
            latest_from_redirect(302, Some(".../tag/v0.24.0-rc.1")).is_err(),
            "a prerelease"
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
    }

    fn make_rig(
        running: &str,
        installed: Option<&str>,
        latest: Option<&str>,
        install_works: bool,
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
        }
    }

    impl Rig {
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

    /// Spec: nothing is installed when the latest release is not newer than
    /// the installed version, and a failed automatic check or install shows
    /// nothing and is tried again at the next check.
    #[farhelm_testtrace::test]
    fn automatic_failures_and_up_to_date_checks_stay_invisible() {
        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.0.0"), true);
        rig.engine.wake_once();
        assert!(rig.installs.lock().unwrap().is_empty());
        assert_eq!(rig.published().activity, Activity::Idle);

        let mut rig = make_rig("1.0.0", Some("1.0.0"), None, true);
        rig.engine.wake_once();
        assert_eq!(rig.published().activity, Activity::Idle, "an offline probe");

        let mut rig = make_rig("1.0.0", Some("1.0.0"), Some("1.1.0"), false);
        rig.engine.wake_once();
        assert_eq!(rig.published().activity, Activity::Idle, "a failed install");
        assert!(!rig.published().update_ready());
        rig.advance(CHECK_INTERVAL);
        rig.engine.wake_once();
        assert_eq!(
            rig.installs.lock().unwrap().len(),
            2,
            "retried at the next check"
        );
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

    // ---- the readout ----

    fn state(installed: Option<&str>, activity: Activity) -> UpdaterState {
        UpdaterState {
            running: "1.0.0".to_string(),
            installed: installed.map(str::to_string),
            activity,
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
            "Farhelm 1.1.0 is installed; restarting Farhelm finishes the update (select for restart to update or what's new)"
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
            assert_eq!(plain.tooltip, "this client was built as farhelm 1.0.0");
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
            "Farhelm could not restart itself; quit and reopen it to finish updating to Farhelm 1.1.0"
        );
    }

    /// Spec: a check the user started ends visibly in the hover: checking,
    /// installing which version, up to date, or failed with the reason;
    /// a failure with an update already waiting still mentions the restart.
    #[farhelm_testtrace::test]
    fn the_readout_hover_carries_a_user_started_check() {
        assert_eq!(
            readout(&state(None, Activity::Checking)).tooltip,
            "checking for a newer Farhelm…"
        );
        assert_eq!(
            readout(&state(None, Activity::Installing("1.2.0".to_string()))).tooltip,
            "installing Farhelm 1.2.0 in the background…"
        );
        assert_eq!(
            readout(&state(
                Some("1.0.0"),
                Activity::UpToDate("1.0.0".to_string())
            ))
            .tooltip,
            "Farhelm is up to date (1.0.0 is the latest release)"
        );
        assert_eq!(
            readout(&state(
                Some("1.0.0"),
                Activity::Failed("offline".to_string())
            ))
            .tooltip,
            "the update check failed: offline"
        );
        let failed_with_update = readout(&state(
            Some("1.1.0"),
            Activity::Failed("offline".to_string()),
        ));
        assert!(failed_with_update.update_ready);
        assert_eq!(
            failed_with_update.tooltip,
            "the update check failed: offline. Farhelm 1.1.0 is installed; restarting Farhelm finishes the update (select for restart to update or what's new)"
        );
    }
}
