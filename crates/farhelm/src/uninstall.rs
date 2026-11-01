//! Remove the selected standalone installation while preserving user data.
//!
//! On macOS the installation is the app bundle with its side-by-side versions
//! (see `app`); on Linux it is a standalone bin directory an earlier installer
//! left (see `ownership`). File ownership is established before confirmation
//! and re-checked after it, under the locks install, setup and (on macOS) the
//! runtime use; see `locks`.
//! Runtime shutdown is an operator prerequisite; this command does not inspect
//! processes or sessions, with two exceptions: setup-owned services, and the
//! supervisor the desktop app started (which outlives the app, so quitting the
//! app no longer stops it). That supervisor is stopped only when the app's own
//! record identifies the live process exactly and its program is one of the
//! versions inside the app being removed
//! (`farhelm_supervisor::managed_supervisor`), the orderly way and never by
//! force.

use anyhow::{Context as _, Result, bail};
use std::{
    ffi::OsString,
    fmt::Write as _,
    io::{BufRead, IsTerminal as _, Read as _, Write},
    path::{Path, PathBuf},
};

mod app;
mod locks;
pub(crate) mod ownership;
mod removal;

/// One confirmation controls the entire removal; --yes never bypasses ownership.
#[derive(clap::Args)]
pub(crate) struct Options {
    /// Show removal and retained data without changing anything.
    #[arg(long)]
    dry_run: bool,
    /// Remove without prompting. Required when stdin is not a terminal.
    #[arg(long)]
    yes: bool,
}

/// Capture invocation facts once so the implementation needs no ambient reads.
struct Inputs {
    ownership: ownership::InspectionInputs,
    xdg_config_home: Option<OsString>,
    xdg_state_home: Option<OsString>,
    interactive: bool,
    /// How long uninstall waits for the desktop app's supervisor to stop
    /// before refusing: [`ORDERLY_STOP_GRACE`] outside tests, which shorten
    /// it to check the refusal without waiting that long.
    ///
    /// [`ORDERLY_STOP_GRACE`]: farhelm_supervisor::managed_supervisor::ORDERLY_STOP_GRACE
    supervisor_stop_grace: std::time::Duration,
}

/// The CLI boundary owns environment capture and real terminal handles.
pub(crate) fn run(options: Options) -> Result<()> {
    let stdin = std::io::stdin();
    let inputs = Inputs {
        ownership: ownership::InspectionInputs {
            current_exe: std::env::current_exe().context("locating this installation's CLI")?,
            home: std::env::var_os("HOME")
                .filter(|s| !s.is_empty())
                .map(PathBuf::from),
            // SAFETY: geteuid observes this process and has no pointer arguments.
            effective_uid: unsafe { libc::geteuid() },
            platform: if cfg!(target_os = "macos") {
                ownership::PlatformArtifacts::Macos
            } else {
                ownership::PlatformArtifacts::Linux
            },
        },
        xdg_config_home: std::env::var_os("XDG_CONFIG_HOME"),
        xdg_state_home: std::env::var_os("XDG_STATE_HOME"),
        interactive: stdin.is_terminal(),
        supervisor_stop_grace: farhelm_supervisor::managed_supervisor::ORDERLY_STOP_GRACE,
    };
    run_with_inputs(
        &inputs,
        &options,
        &mut crate::setup::SystemctlUnitManager,
        &mut stdin.lock(),
        &mut std::io::stdout().lock(),
    )
}

/// Preflight everything before showing one plan and obtaining one confirmation.
///
/// The preview is flushed before mutation. Mutation reporting is buffered so
/// a broken output pipe cannot strand an otherwise-completable removal midway.
fn run_with_inputs(
    inputs: &Inputs,
    options: &Options,
    manager: &mut dyn crate::setup::UnitManager,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Result<()> {
    let home = inputs.ownership.home.as_deref().context(
        "HOME is not set; cannot determine the app bundle, service or retained data locations",
    )?;
    let plan = ownership::inspect(&inputs.ownership)?;
    let services = match &plan {
        ownership::OwnershipPlan::Flat(flat) => {
            let directory =
                farhelm_helm::units::user_unit_dir(inputs.xdg_config_home.as_deref(), home);
            Some(crate::setup::preflight_selected_services(
                std::slice::from_ref(&flat.cli),
                &directory,
                manager,
            )?)
        }
        ownership::OwnershipPlan::App(_) => None,
    };
    let mut preview = String::from(
        "Stop local sessions and additional terminals, quit Farhelm Desktop, and stop manually started Farhelm processes before continuing. Uninstall does not check whether sessions or manually started processes are running.\n\n",
    );
    if let Some(services) = &services {
        for service in services.selected() {
            writeln!(
                preview,
                "stop and disable {}; remove {}",
                service.name(),
                path_text(service.path())
            )?;
        }
        for path in services.retained_paths() {
            writeln!(preview, "retain integration {}", path_text(path))?;
        }
        if !services.manager_available() {
            preview.push_str("No user service manager was available; no known service files were found in the selected configuration directory.\n");
        }
    }
    match &plan {
        ownership::OwnershipPlan::Flat(flat) => {
            if let Some(path) = &flat.desktop {
                writeln!(preview, "remove {}", path_text(path))?;
            }
            if let Some(path) = &flat.retained_foreign_desktop {
                writeln!(preview, "retain unrelated file {}", path_text(path))?;
            }
            writeln!(preview, "remove {} last", path_text(&flat.cli))?;
            writeln!(preview, "retain shared directory {}", path_text(&flat.root))?;
        }
        ownership::OwnershipPlan::App(app) => {
            writeln!(
                preview,
                "remove the app {}, with every Farhelm version kept inside it",
                path_text(&app.root)
            )?;
            match &app.link {
                app::LinkPlan::Remove(link) => {
                    writeln!(preview, "remove the Terminal link {} last", path_text(link))?;
                }
                app::LinkPlan::Absent(_) => {}
                app::LinkPlan::Retain(link) => writeln!(
                    preview,
                    "retain {}: it is no longer a link to this installation's app",
                    path_text(link)
                )?,
            }
        }
    }
    let state = farhelm_supervisor::default_state_dir_for(inputs.xdg_state_home.as_deref(), home);
    let app_supervisors = match &plan {
        ownership::OwnershipPlan::App(app)
            if inputs.ownership.platform == ownership::PlatformArtifacts::Macos =>
        {
            app_supervisors(&runtime_state_dirs(&state, home), app)?
        }
        _ => Vec::new(),
    };
    for stop in &app_supervisors {
        writeln!(
            preview,
            "stop the desktop app's supervisor (process {}) using {}",
            stop.record.pid,
            path_text(&stop.state_dir)
        )?;
    }
    writeln!(preview, "retain data under {}", path_text(&state))?;
    preview.push_str("Other custom data locations, projects, agent tools and dependencies are untouched.\nAn install or upgrade running now makes uninstall refuse, and one started during it refuses instead.\n");
    output
        .write_all(preview.as_bytes())
        .context("writing uninstall plan")?;
    output.flush().context("flushing uninstall plan")?;
    if options.dry_run {
        return Ok(());
    }
    if !options.yes {
        if !inputs.interactive {
            bail!("uninstall needs terminal confirmation; rerun with --yes after stopping Farhelm");
        }
        output.write_all(b"Remove this installation? [y/N] ")?;
        output.flush()?;
        let mut answer = String::new();
        input
            .take(64)
            .read_line(&mut answer)
            .context("reading uninstall confirmation")?;
        if answer.len() == 64 || !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
        {
            output.write_all(b"Cancelled; no changes made.\n")?;
            return Ok(());
        }
    }
    // Held until the removal below has finished, and released before the
    // report is written, so a blocked output pipe cannot keep them; see
    // `locks`.
    let mut held = lock_and_recheck(
        inputs,
        home,
        &state,
        &plan,
        services.as_ref(),
        &app_supervisors,
        manager,
    )?;
    let mut report = String::new();
    let result = (|| {
        if let Some(services) = &services {
            crate::setup::remove_selected_services_locked(services, false, manager, &mut report)?;
        }
        match &plan {
            ownership::OwnershipPlan::App(app) => remove_app_locked(app, &mut held, &mut report)
                .map_err(|error| after_stop(error, held.stopped_supervisor)),
            ownership::OwnershipPlan::Flat(_) => {
                removal::remove(&plan, &held.running_records, &mut report)
            }
        }
    })();
    drop(held);
    if result.is_err() {
        match &plan {
            ownership::OwnershipPlan::Flat(flat) => writeln!(
                report,
                "Uninstall is incomplete. The CLI remains at {}; resolve the reported failure and rerun uninstall.",
                path_text(&flat.cli)
            )?,
            ownership::OwnershipPlan::App(app) => writeln!(
                report,
                "Uninstall is incomplete. Resolve the reported failure and rerun `~/.local/bin/farhelm uninstall`; if that command is gone too, what remains of {} holds no data and can be moved to the Trash.",
                path_text(&app.root)
            )?,
        }
    } else {
        report.push_str("Farhelm uninstalled. User data was retained.\n");
    }
    let written = output
        .write_all(report.as_bytes())
        .and_then(|()| output.flush())
        .context("writing uninstall result");
    result.and(written)
}

/// Everything uninstall holds while it removes: dropped (released) when the
/// removal is over, in any outcome.
///
/// Fields drop in declaration order, which is the reverse of acquisition:
/// the install lock goes last, so an installer cannot slip in between the
/// release of the install lock and that of the app lock and commit
/// binaries only to refuse at its app step.
/// Remove the app with every lock held (design, section 6): each version
/// still having its program locked exclusively (this uninstall's own version
/// by converting the shared lock it holds as a running `farhelm`), the
/// ownership record moved to "removing" and flushed, then the removal
/// through the app's held folders in today's order.
///
/// A version something still uses refuses before anything is removed, and
/// says how to find what uses it: the lock cannot tell which process holds
/// it, and it need not be Farhelm.
fn remove_app_locked(app: &app::AppPlan, held: &mut HeldLocks, report: &mut String) -> Result<()> {
    let mut folders = held
        .app_folder
        .take()
        .context("internal error: the app folder was not held")?;
    let _versions = hold_versions(app, &folders)?;
    if let Some(contents) = folders
        .contents()
        .with_context(|| format!("opening {}; nothing was removed", path_text(&app.root)))?
    {
        use std::os::fd::AsFd as _;
        farhelm_supervisor::installation::mark_removing(contents.as_fd()).with_context(|| {
            format!(
                "recording that {} is being removed; nothing was removed",
                path_text(&app.root)
            )
        })?;
    }
    let applications = held
        .installation
        .as_ref()
        .map(|locks| locks.lock.applications());
    removal::remove_app_held(
        app,
        &mut folders,
        applications,
        &held.running_records,
        report,
    )
}

/// Take every version's lock exclusively, without waiting, and keep them
/// (the returned files) until the removal is done. A version whose program
/// is already gone is removal already done and needs no lock.
fn hold_versions(app: &app::AppPlan, folders: &removal::HeldFs) -> Result<Vec<std::fs::File>> {
    use std::fs::TryLockError;
    let in_use = |version: &app::VersionProgram| {
        anyhow::anyhow!(
            "Farhelm {} is still in use: some running program (not necessarily Farhelm) holds it, \
             so nothing was removed; `lsof {}` shows which. Quit it, then run uninstall again",
            version.name.to_string_lossy(),
            path_text(&version.program)
        )
    };
    let refuse = |version: &app::VersionProgram, error: std::io::Error| {
        anyhow::anyhow!(
            "locking Farhelm {} ({}) for removal failed: {error}; nothing was removed. Where files \
             cannot be locked, uninstall cannot tell what still uses Farhelm. To remove Farhelm \
             anyway, quit it and everything started from it, then delete {} and \
             ~/.local/bin/farhelm by hand",
            version.name.to_string_lossy(),
            path_text(&version.program),
            path_text(&app.root)
        )
    };
    let same_file = |a: &std::fs::File, b: &std::fs::File| -> std::io::Result<bool> {
        use std::os::unix::fs::MetadataExt as _;
        let (a, b) = (a.metadata()?, b.metadata()?);
        Ok((a.dev(), a.ino()) == (b.dev(), b.ino()))
    };
    let mut held = Vec::new();
    for version in &app.versions {
        // Through the held app folder, never following a link.
        let program = match folders.open_file(&version.program) {
            Ok(Some(program)) => program,
            Ok(None) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(refuse(version, error)),
        };
        // This uninstall's own version: the one shared lock it holds as a
        // running farhelm is converted, never a second one opened (two
        // holders in one process would refuse each other). Only when that
        // lock is on this very program: a running farhelm from another app
        // that Finder put at the same place has the same version names, and
        // converting its own lock there would leave this app's program
        // unlocked.
        if version.name == app.own_version
            && let Some(own) = farhelm_supervisor::version_lock::process_hold()
        {
            if !same_file(own, &program).map_err(|error| refuse(version, error))? {
                anyhow::bail!(
                    "{} is no longer the app this uninstall runs from (it was moved or replaced \
                     after you confirmed); nothing was removed; run uninstall again",
                    path_text(&app.root)
                );
            }
            match own.try_lock() {
                Ok(()) => continue,
                Err(TryLockError::WouldBlock) => return Err(in_use(version)),
                Err(TryLockError::Error(error)) => return Err(refuse(version, error)),
            }
        }
        match program.try_lock() {
            Ok(()) => held.push(program),
            Err(TryLockError::WouldBlock) => return Err(in_use(version)),
            Err(TryLockError::Error(error)) => return Err(refuse(version, error)),
        }
    }
    Ok(held)
}

struct HeldLocks {
    /// Whether uninstall stopped the desktop app's supervisor on its way
    /// here, so a refusal after that point can say so.
    stopped_supervisor: bool,
    /// The Running records of the state directories whose runtime locks are
    /// held, for removal: with those locks held no supervisor can be
    /// running from them.
    running_records: Vec<PathBuf>,
    _runtime: Option<locks::RuntimeLocks>,
    _setup: Option<crate::setup::SetupLock>,
    /// The installation lock and the older installers' lock folder (macOS),
    /// whose `~/Applications` descriptor the removal uses for leftovers
    /// beside the app.
    installation: Option<locks::InstallationLocks>,
    /// The app folder every step after the re-check goes through.
    app_folder: Option<removal::HeldFs>,
    _install: Option<locks::InstallLock>,
}

/// Take the locks the other lifecycle operations use, without waiting, and
/// confirm that what uninstall is about to remove is still exactly the plan
/// the user confirmed.
///
/// The re-check is what makes the locks sufficient: an install, update or
/// setup that ran between the plan and the locks may have changed the
/// binaries, the app bundle or the service files, and once the locks are
/// held nothing else can. A plan that changed refuses rather than removing
/// something the user was not shown. See `locks` for which locks and why.
fn lock_and_recheck(
    inputs: &Inputs,
    home: &Path,
    state: &Path,
    plan: &ownership::OwnershipPlan,
    services: Option<&crate::setup::SelectedServicePlan>,
    app_supervisors: &[AppSupervisor],
    manager: &mut dyn crate::setup::UnitManager,
) -> Result<HeldLocks> {
    let macos = inputs.ownership.platform == ownership::PlatformArtifacts::Macos;
    // The installer's bin-directory lock exists only for the flat layout;
    // the app layout's installer takes only the lock beside the bundle.
    let install = match plan {
        ownership::OwnershipPlan::Flat(flat) => Some(locks::InstallLock::acquire(&flat.root)?),
        ownership::OwnershipPlan::App(_) => None,
    };
    let installation = match plan {
        ownership::OwnershipPlan::App(_) if macos => Some(locks::InstallationLocks::acquire(
            &home.join("Applications"),
        )?),
        _ => None,
    };
    // The app folder, held before the re-check below so that the folder the
    // re-check reads is the one every later step locks and changes; it is
    // opened without following a link and compared with what the re-check
    // saw at the app's path.
    let app_folder = match plan {
        ownership::OwnershipPlan::App(app) => {
            Some(removal::HeldFs::open(&app.root).with_context(|| {
                format!("opening {}; nothing was removed", path_text(&app.root))
            })?)
        }
        ownership::OwnershipPlan::Flat(_) => None,
    };
    let setup = match services {
        Some(services) => crate::setup::lock_for_selected_services(services)?,
        None => None,
    };
    // Only the helm locks for now: an open desktop app holds its helm's, so
    // uninstall refuses here, before it has stopped anything. The supervisor
    // locks follow once the re-check below has passed and the app's own
    // supervisor is stopped.
    let mut running_records = Vec::new();
    let mut runtime = if macos {
        let state_dirs = runtime_state_dirs(state, home);
        running_records = state_dirs
            .iter()
            .map(|dir| dir.join(farhelm_supervisor::app_bundle::RUNNING_RECORD))
            .collect();
        Some(locks::RuntimeLocks::acquire_helm(&state_dirs)?)
    } else {
        None
    };
    let changed = |what: &str| {
        anyhow::anyhow!(
            "{what} changed after you confirmed (for example, an install, update or setup ran in \
             the meantime); nothing was removed; run uninstall again to see the current plan"
        )
    };
    let fresh = ownership::inspect(&inputs.ownership)
        .context("re-checking the installation after confirmation; nothing was removed")?;
    if let (Some(folder), ownership::OwnershipPlan::App(app)) = (&app_folder, plan)
        && !folder.is_at(&app.root)
    {
        return Err(changed(&format!("the app at {}", path_text(&app.root))));
    }
    if fresh != *plan {
        let root = match plan {
            ownership::OwnershipPlan::Flat(flat) => &flat.root,
            ownership::OwnershipPlan::App(app) => &app.root,
        };
        return Err(changed(&format!(
            "the installed files under {}",
            path_text(root)
        )));
    }
    if let (Some(services), ownership::OwnershipPlan::Flat(flat)) = (services, plan) {
        let directory = farhelm_helm::units::user_unit_dir(inputs.xdg_config_home.as_deref(), home);
        let fresh = crate::setup::preflight_selected_services(
            std::slice::from_ref(&flat.cli),
            &directory,
            manager,
        )
        .context("re-checking Farhelm's services after confirmation; nothing was removed")?;
        if fresh != *services {
            return Err(changed("the Farhelm services set up for this installation"));
        }
    }
    let mut stopped_supervisor = false;
    if let Some(runtime) = &mut runtime {
        stopped_supervisor = stop_app_supervisors(app_supervisors, inputs.supervisor_stop_grace)?;
        runtime
            .lock_supervisors()
            .map_err(|error| after_stop(error, stopped_supervisor))?;
    }
    Ok(HeldLocks {
        stopped_supervisor,
        running_records,
        _install: install,
        installation,
        app_folder,
        _setup: setup,
        _runtime: runtime,
    })
}

/// The state directories whose runtime locks uninstall takes on macOS: the
/// one this shell selects and the default one.
///
/// The desktop app, opened from Finder or the Dock, does not see a shell's
/// `XDG_STATE_HOME`, so its supervisor and helm use the default location even
/// when this shell names another; both are checked.
fn runtime_state_dirs(state: &Path, home: &Path) -> Vec<PathBuf> {
    let mut state_dirs = vec![state.to_path_buf()];
    let default = farhelm_supervisor::default_state_dir_for(None, home);
    if default != state {
        state_dirs.push(default);
    }
    state_dirs
}

/// A supervisor the desktop app started that uninstall will stop.
#[derive(Debug, Clone)]
struct AppSupervisor {
    state_dir: PathBuf,
    record: farhelm_supervisor::managed_supervisor::ManagedSupervisor,
}

/// The supervisors this installation's app started that are still running,
/// one per state directory at most.
///
/// Only a record that matches a live process exactly (pid, start identity
/// and running image; see `ManagedSupervisor::observe`) and whose program
/// file is one of the version programs of the app being removed counts. The
/// program is matched by file identity, not by path, so a path that merely
/// spells its way into the app (`..`), or a different app put at the same
/// place, does not count. A supervisor started from another installation, a
/// development build, or by hand is never stopped here, and still makes
/// uninstall refuse on its lock as before.
fn app_supervisors(state_dirs: &[PathBuf], app: &app::AppPlan) -> Result<Vec<AppSupervisor>> {
    use farhelm_supervisor::managed_supervisor::{ManagedSupervisor, Observation};
    use std::os::unix::fs::MetadataExt as _;
    let versions: Vec<(u64, u64)> = app
        .versions
        .iter()
        .filter_map(|version| std::fs::metadata(&version.program).ok())
        .map(|metadata| (metadata.dev(), metadata.ino()))
        .collect();
    let mut found = Vec::new();
    for state_dir in state_dirs {
        let record = ManagedSupervisor::read(state_dir).with_context(|| {
            format!(
                "reading the desktop app's supervisor record in {}",
                path_text(state_dir)
            )
        })?;
        let Some(record) = record else { continue };
        if !versions.contains(&(record.program_dev, record.program_ino))
            || found
                .iter()
                .any(|known: &AppSupervisor| known.record.pid == record.pid)
        {
            continue;
        }
        let observed = record.observe().map_err(|error| {
            anyhow::anyhow!(
                "checking the desktop app's supervisor (process {}) using {}: {error}",
                record.pid,
                path_text(state_dir)
            )
        })?;
        if observed == Observation::Matches {
            found.push(AppSupervisor {
                state_dir: state_dir.clone(),
                record,
            });
        }
    }
    Ok(found)
}

/// Stop the app's own supervisors that the confirmed plan listed, the
/// orderly way, or refuse with nothing removed. Returns whether any was
/// stopped, so later refusals can say so.
///
/// Each one is re-read first: a record that changed since the plan (another
/// supervisor started meanwhile) is not stopped, and its lock then refuses
/// as for any supervisor the app did not start. A supervisor that does not
/// exit within the grace refuses; uninstall never kills one, since a kill
/// can take its tmux server and every session with it. A stop that could not
/// be confirmed (the process could not be read or signalled) refuses too and
/// keeps the record, which is what lets a later uninstall or app launch
/// still recognize that supervisor. The record of a supervisor confirmed
/// gone is removed best-effort: a record whose process is gone authorizes
/// nothing, so failing to remove it is no reason to stop the uninstall.
fn stop_app_supervisors(planned: &[AppSupervisor], grace: std::time::Duration) -> Result<bool> {
    use farhelm_supervisor::managed_supervisor::{ManagedSupervisor, StopOutcome};
    let mut stopped = false;
    for stop in planned {
        let current = ManagedSupervisor::read(&stop.state_dir).with_context(|| {
            format!(
                "re-reading the desktop app's supervisor record in {}; nothing was removed",
                path_text(&stop.state_dir)
            )
        })?;
        if current.as_ref() != Some(&stop.record) {
            continue;
        }
        let outcome = stop.record.stop(grace, false).map_err(|error| {
            anyhow::anyhow!(
                "stopping the desktop app's supervisor (process {}) could not be confirmed: \
                 {error}; nothing was removed; stop it yourself, then run uninstall again",
                stop.record.pid
            )
        })?;
        match outcome {
            StopOutcome::AlreadyGone => {}
            StopOutcome::Stopped | StopOutcome::Killed => stopped = true,
            StopOutcome::StillRunning => bail!(
                "the desktop app's supervisor (process {}) did not stop within {} seconds; \
                 nothing was removed; stop it yourself, then run uninstall again",
                stop.record.pid,
                grace.as_secs()
            ),
        }
        let _ = ManagedSupervisor::remove(&stop.state_dir);
    }
    Ok(stopped)
}

/// Add to a refusal that comes after uninstall stopped the desktop app's
/// supervisor that it did: "nothing was removed" is true of files, but the
/// user's sessions have lost the supervisor serving them until the app is
/// opened again.
fn after_stop(error: anyhow::Error, stopped: bool) -> anyhow::Error {
    if stopped {
        error.context(
            "uninstall had already stopped the desktop app's supervisor; opening Farhelm starts \
             it again",
        )
    } else {
        error
    }
}

/// Escape paths so a filename cannot forge an extra line of diagnostic output.
fn path_text(path: &Path) -> String {
    ownership::path_text(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use app::tests::AppFixture;
    use ownership::{PlatformArtifacts, tests::Fixture};

    /// A unit manager for the macOS path, which never consults one.
    struct NoUnits;
    impl crate::setup::UnitManager for NoUnits {
        fn run(&mut self, args: &[&str]) -> Result<crate::setup::UnitCommand> {
            panic!("the macOS uninstall path ran systemctl {args:?}");
        }
    }

    /// A confirmation that answers "y" and, as it is read, runs `meanwhile`:
    /// the window between the user confirming and uninstall taking its
    /// locks, made deterministic.
    struct ConfirmWhile<F: FnMut()> {
        answer: std::io::Cursor<Vec<u8>>,
        meanwhile: Option<F>,
    }
    impl<F: FnMut()> std::io::Read for ConfirmWhile<F> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if let Some(mut meanwhile) = self.meanwhile.take() {
                meanwhile();
            }
            self.answer.read(buf)
        }
    }
    impl<F: FnMut()> BufRead for ConfirmWhile<F> {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            if let Some(mut meanwhile) = self.meanwhile.take() {
                meanwhile();
            }
            self.answer.fill_buf()
        }
        fn consume(&mut self, amount: usize) {
            self.answer.consume(amount);
        }
    }

    /// The inputs for a macOS uninstall of `fixture`, with `XDG_STATE_HOME`
    /// at `state`. The supervisor's state directory is `state/farhelm`,
    /// created here so the runtime-lock step is really reached rather than
    /// skipped for a missing directory.
    fn inputs(fixture: &AppFixture, state: &Path) -> Inputs {
        std::fs::create_dir_all(state.join("farhelm")).expect("state directory");
        Inputs {
            ownership: fixture.inputs(),
            xdg_config_home: None,
            xdg_state_home: Some(state.as_os_str().to_os_string()),
            interactive: true,
            supervisor_stop_grace: std::time::Duration::from_secs(10),
        }
    }

    /// The app's ownership record, parsed.
    fn record_of(fixture: &AppFixture) -> farhelm_supervisor::installation::OwnershipRecord {
        let bytes = std::fs::read(fixture.app.join("Contents/.farhelm-installation")).unwrap();
        farhelm_supervisor::installation::OwnershipRecord::parse(&bytes).expect("a valid record")
    }

    /// A version some program still runs from refuses the uninstall before
    /// anything is removed or recorded, and says how to find the program.
    ///
    /// Why: removing a version under a running program is the race this
    /// design closes (design, section 6); and the lock cannot say which
    /// process holds it, which need not even be Farhelm, so the refusal
    /// points at `lsof` rather than guessing. Spec: with a shared lock held
    /// on an old version's program, uninstall refuses naming that version
    /// and `lsof` with its path; every file stays, and the record is not
    /// moved to "removing".
    #[test]
    fn a_version_in_use_refuses_naming_it_and_removes_nothing() {
        let fixture = AppFixture::new();
        let state = tempfile::tempdir().expect("state");
        let program = fixture.app.join("Contents/Versions/1.2.3/farhelm");
        let user = std::fs::File::open(&program).expect("open");
        user.lock_shared()
            .expect("a running 1.2.3 holds its version");
        let error = run_with_inputs(
            &inputs(&fixture, state.path()),
            &Options {
                dry_run: false,
                yes: true,
            },
            &mut NoUnits,
            &mut std::io::Cursor::new(Vec::new()),
            &mut Vec::new(),
        )
        .expect_err("a version in use refuses");
        let text = format!("{error:#}");
        assert!(text.contains("Farhelm 1.2.3 is still in use"), "{text}");
        assert!(
            text.contains(&format!("lsof {}", path_text(&program))),
            "names lsof with the program's path, quoted for pasting: {text}"
        );
        assert!(program.exists(), "nothing removed");
        assert!(fixture.app.join("Contents/MacOS/farhelm-desktop").exists());
        assert_eq!(
            record_of(&fixture).state,
            farhelm_supervisor::installation::InstallState::Complete,
            "nothing recorded"
        );
    }

    /// Run a confirmed uninstall of `fixture` with `state` as its state root.
    fn uninstall_now(fixture: &AppFixture, state: &Path) -> Result<()> {
        run_with_inputs(
            &inputs(fixture, state),
            &Options {
                dry_run: false,
                yes: true,
            },
            &mut NoUnits,
            &mut std::io::Cursor::new(Vec::new()),
            &mut Vec::new(),
        )
    }

    /// The record says "removing" before the first piece of the app is
    /// removed, and an uninstall stopped at any point is finished by a
    /// retry.
    ///
    /// Why: the record's "removing" state is what keeps an update already on
    /// its way from finishing into a half-removed app and what lets the app
    /// and its updater say an uninstall was interrupted (design, sections 4
    /// and 6), so it must be durable before anything goes. Two stops prove
    /// it: one at the very first removal (the oldest version's program,
    /// whose folder cannot be written), where nothing is gone yet and the
    /// record already says "removing"; and one at the app program, where the
    /// early version is gone and the retry command's pieces remain. Each
    /// stop is a folder whose entries cannot be removed, which root removes
    /// anyway, so the check needs an ordinary user.
    #[test]
    fn an_interrupted_uninstall_records_removing_first_and_a_retry_finishes() {
        use std::os::unix::fs::PermissionsExt as _;
        if unsafe { libc::geteuid() } == 0 {
            eprintln!("skipped: root ignores the folder permissions this test stops removal with");
            return;
        }
        for (stop, gone, kept) in [
            (
                "Contents/Versions/1.2.3",
                None,
                "Contents/Versions/1.2.3/farhelm",
            ),
            (
                "Contents/MacOS",
                Some("Contents/Versions/1.2.3"),
                "Contents/MacOS/farhelm-desktop",
            ),
        ] {
            let fixture = AppFixture::new();
            let state = tempfile::tempdir().expect("state");
            let folder = fixture.app.join(stop);
            std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o500)).unwrap();
            let error = uninstall_now(&fixture, state.path()).expect_err("stopped");
            let text = format!("{error:#}");
            assert!(
                text.contains(&path_text(&fixture.app.join(kept))) && text.contains("ermission"),
                "{stop}: the stop is the expected removal, for the expected reason: {text}"
            );
            assert_eq!(
                record_of(&fixture).state,
                farhelm_supervisor::installation::InstallState::Removing,
                "{stop}: recorded before anything went"
            );
            if let Some(gone) = gone {
                assert!(
                    !fixture.app.join(gone).exists(),
                    "{stop}: earlier pieces are gone"
                );
            }
            assert!(
                fixture.app.join(kept).exists(),
                "{stop}: the failing piece stays"
            );
            assert!(
                fixture.app.join("Contents/Versions/1.2.4/farhelm").exists(),
                "{stop}: the retry command's own version stays"
            );
            std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
            uninstall_now(&fixture, state.path()).expect("a retry finishes");
            assert!(!fixture.app.exists(), "{stop}: the app is gone");
        }
    }

    /// A temporary file an installer or an earlier uninstall left inside
    /// the app (a regular `.farhelm-new.*` file, which is how both write
    /// files) does not refuse the uninstall: it is removed with the rest.
    ///
    /// Why: an uninstall killed while it wrote the "removing" record leaves
    /// exactly such a file in `Contents/`, and refusing it as foreign would
    /// make the documented retry impossible. A link with that name is still
    /// not the installer's, and still refuses.
    #[test]
    fn installer_temporaries_inside_the_app_are_removed_not_refused() {
        let fixture = AppFixture::new();
        let state = tempfile::tempdir().expect("state");
        std::fs::write(
            fixture
                .app
                .join("Contents/.farhelm-new.abc.farhelm-installation"),
            b"half written",
        )
        .unwrap();
        std::fs::write(
            fixture.app.join("Contents/MacOS/.farhelm-new.abc.farhelm"),
            b"x",
        )
        .unwrap();
        uninstall_now(&fixture, state.path()).expect("the temporaries are removed with the app");
        assert!(!fixture.app.exists());

        let fixture = AppFixture::new();
        std::os::unix::fs::symlink(
            "/nonexistent",
            fixture.app.join("Contents/.farhelm-new.link"),
        )
        .unwrap();
        let error = uninstall_now(&fixture, state.path()).expect_err("a link is not a temporary");
        assert!(
            format!("{error:#}").contains("unexpected entry"),
            "{error:#}"
        );
    }

    /// An installation that changes between the confirmation and the
    /// removal is not removed.
    ///
    /// Why: SPEC.md ("Concurrent and interrupted runs") forbids removing a
    /// file the command does not own or reporting a result that did not
    /// happen, and an install or update that runs while the user reads the
    /// prompt can change exactly what the confirmed plan listed. Uninstall
    /// takes its locks after the confirmation and re-checks the plan under
    /// them. Spec: when the installation changes at the moment the user
    /// confirms (here the desktop executable an update would remove goes
    /// away), uninstall refuses saying nothing was removed, every remaining
    /// file is still there, and no lock is left behind.
    #[test]
    fn a_plan_that_changed_after_confirmation_refuses_without_removing() {
        let fixture = AppFixture::new();
        let state = tempfile::tempdir().expect("state");
        let desktop = fixture.app.join("Contents/MacOS/farhelm-desktop");
        let mut input = ConfirmWhile {
            answer: std::io::Cursor::new(b"y\n".to_vec()),
            meanwhile: Some(|| fs_remove(&desktop)),
        };
        let error = run_with_inputs(
            &inputs(&fixture, state.path()),
            &Options {
                dry_run: false,
                yes: false,
            },
            &mut NoUnits,
            &mut input,
            &mut Vec::new(),
        )
        .expect_err("a changed plan refuses");
        assert!(
            error.to_string().contains("nothing was removed"),
            "{error:#}"
        );
        assert!(
            fixture.app.join("Contents/Versions/1.2.4/farhelm").exists(),
            "the CLI was not removed"
        );
        assert!(!fixture.home.join("Applications/.farhelm-app.lock").exists());
    }

    /// An app put at the same place while the confirmation waited (Finder
    /// moving the confirmed app aside and an identical-looking copy taking
    /// its place) is not removed, and neither is the confirmed one.
    ///
    /// Why: the copy has the same version names and record, so a re-check
    /// of files alone passes, and the version locks would then be taken on
    /// the wrong app: this uninstall's own lock is on the confirmed app's
    /// program, and a program of the copy that something runs would be
    /// removed unlocked (design, section 6: open the app's folder and check
    /// it is the installation, then lock every version). The plan carries
    /// the app folder's identity, so the swap reads as a change.
    #[test]
    fn an_app_swapped_in_after_confirmation_refuses_without_removing() {
        let fixture = AppFixture::new();
        let state = tempfile::tempdir().expect("state");
        let aside = fixture.home.join("Applications/Aside.app");
        let (app, moved) = (fixture.app.clone(), aside.clone());
        let mut input = ConfirmWhile {
            answer: std::io::Cursor::new(b"y\n".to_vec()),
            meanwhile: Some(move || {
                std::fs::rename(&app, &moved).expect("fixture: moving the app aside");
                let copied = std::process::Command::new("cp")
                    .arg("-Rp")
                    .arg(&moved)
                    .arg(&app)
                    .status()
                    .expect("fixture: running cp");
                assert!(copied.success(), "fixture: copying the app back");
            }),
        };
        let error = run_with_inputs(
            &inputs(&fixture, state.path()),
            &Options {
                dry_run: false,
                yes: false,
            },
            &mut NoUnits,
            &mut input,
            &mut Vec::new(),
        )
        .expect_err("a swapped app refuses");
        assert!(
            format!("{error:#}").contains("nothing was removed"),
            "{error:#}"
        );
        for app in [&fixture.app, &aside] {
            assert!(
                app.join("Contents/Versions/1.2.4/farhelm").exists()
                    && app.join("Contents/Versions/1.2.3/farhelm").exists(),
                "{} kept every version",
                app.display()
            );
        }
        assert_eq!(
            record_of(&fixture).state,
            farhelm_supervisor::installation::InstallState::Complete,
            "nothing was recorded either"
        );
    }

    /// A user unit manager that is not there, for the Linux path: with no
    /// known service files, uninstall plans no services.
    struct NoUserManager;
    impl crate::setup::UnitManager for NoUserManager {
        fn run(&mut self, _args: &[&str]) -> Result<crate::setup::UnitCommand> {
            anyhow::bail!("no user service manager in this test")
        }
    }

    /// A held install lock refuses a Linux uninstall before anything is
    /// removed.
    ///
    /// Why: an earlier installer holds this lock while it replaces binaries
    /// and their ownership record; removing them meanwhile could delete what
    /// it just installed or leave the record and binaries disagreeing, which
    /// SPEC.md forbids. Spec: with the lock present at confirmation time,
    /// uninstall refuses naming it, removes nothing, and leaves the lock.
    #[test]
    fn a_held_install_lock_refuses_without_removing() {
        let fixture = Fixture::new();
        fixture.flat(None);
        let config = tempfile::tempdir().expect("config");
        let lock = fixture.install.join(".farhelm-install.lock");
        std::fs::create_dir(&lock).expect("installer lock");
        std::fs::write(lock.join("pid"), "1\n").expect("installer pid");
        let error = run_with_inputs(
            &Inputs {
                ownership: fixture.inputs(PlatformArtifacts::Linux),
                xdg_config_home: Some(config.path().as_os_str().to_os_string()),
                xdg_state_home: None,
                interactive: true,
                supervisor_stop_grace: std::time::Duration::from_secs(10),
            },
            &Options {
                dry_run: false,
                yes: true,
            },
            &mut NoUserManager,
            &mut std::io::Cursor::new(Vec::new()),
            &mut Vec::new(),
        )
        .expect_err("a held lock refuses");
        assert!(
            error.to_string().contains(".farhelm-install.lock"),
            "{error:#}"
        );
        assert!(fixture.install.join("farhelm").exists());
        assert!(
            lock.join("pid").exists(),
            "another process's lock is left alone"
        );
    }

    /// The ordinary case still removes the installation and leaves no lock.
    ///
    /// Why: the locks must never outlive the uninstall that took them, or the
    /// next install would find a stale lock; and the Running records would
    /// otherwise name a version that no longer exists. Spec: with nothing
    /// else holding a lock, uninstall removes the app, the Terminal link and
    /// the Running records of both state directories it checked, and leaves
    /// no lock behind.
    #[test]
    fn an_uncontended_uninstall_removes_and_releases_its_locks() {
        let fixture = AppFixture::new();
        let state = tempfile::tempdir().expect("state");
        let default = fixture.home.join(".local/state/farhelm");
        std::fs::create_dir_all(&default).expect("default state");
        let inputs = inputs(&fixture, state.path());
        let records = [
            state.path().join("farhelm/running-version"),
            default.join("running-version"),
        ];
        for record in &records {
            std::fs::write(record, "1.2.4\n").expect("running record");
        }
        let mut output = Vec::new();
        run_with_inputs(
            &inputs,
            &Options {
                dry_run: false,
                yes: true,
            },
            &mut NoUnits,
            &mut std::io::Cursor::new(Vec::new()),
            &mut output,
        )
        .expect("uninstall");
        assert!(
            !fixture.app.exists(),
            "{}",
            String::from_utf8_lossy(&output)
        );
        assert!(std::fs::symlink_metadata(&fixture.link).is_err());
        for record in &records {
            assert!(!record.exists(), "{}", record.display());
        }
        assert!(!fixture.home.join("Applications/.farhelm-app.lock").exists());
        assert!(
            String::from_utf8_lossy(&output).contains("Farhelm uninstalled."),
            "{}",
            String::from_utf8_lossy(&output)
        );
    }

    /// A held supervisor lock (the open desktop app's) refuses the
    /// uninstall before anything is removed.
    ///
    /// Why: on macOS the desktop app's supervisor and helm hold their
    /// state-directory locks, and removing the app and binaries under a
    /// running app is the overlap SPEC.md's "Concurrent and interrupted
    /// runs" requires a correct outcome for. Spec: with `supervisor.lock`
    /// held by another descriptor, uninstall refuses naming it, removes
    /// nothing, and leaves no install lock behind.
    #[test]
    fn a_held_supervisor_lock_refuses_without_removing() {
        let fixture = AppFixture::new();
        let state = tempfile::tempdir().expect("state");
        let inputs = inputs(&fixture, state.path());
        let holder = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(state.path().join("farhelm/supervisor.lock"))
            .expect("holder");
        holder
            .try_lock()
            .expect("the app's supervisor holds the lock");
        let error = run_with_inputs(
            &inputs,
            &Options {
                dry_run: false,
                yes: true,
            },
            &mut NoUnits,
            &mut std::io::Cursor::new(Vec::new()),
            &mut Vec::new(),
        )
        .expect_err("a held supervisor lock refuses");
        assert!(error.to_string().contains("supervisor.lock"), "{error:#}");
        assert!(fixture.app.join("Contents/Versions/1.2.4/farhelm").exists());
        assert!(fixture.app.join("Contents/MacOS/farhelm-desktop").exists());
        assert!(!fixture.home.join("Applications/.farhelm-app.lock").exists());
    }

    /// A held app lock (the installer rebuilding the app) refuses the
    /// uninstall before anything is removed.
    ///
    /// Why: this is also what shows the uninstall really takes that lock,
    /// which the uncontended test, seeing only that no lock is left, cannot
    /// tell from never taking it. Spec: with `.farhelm-app.lock` present,
    /// uninstall refuses naming it and removes nothing.
    #[test]
    fn a_held_app_lock_refuses_without_removing() {
        let fixture = AppFixture::new();
        let applications = fixture.home.join("Applications");
        std::fs::create_dir_all(applications.join(".farhelm-app.lock"))
            .expect("installer app lock");
        let state = tempfile::tempdir().expect("state");
        let error = run_with_inputs(
            &inputs(&fixture, state.path()),
            &Options {
                dry_run: false,
                yes: true,
            },
            &mut NoUnits,
            &mut std::io::Cursor::new(Vec::new()),
            &mut Vec::new(),
        )
        .expect_err("a held app lock refuses");
        assert!(error.to_string().contains(".farhelm-app.lock"), "{error:#}");
        assert!(fixture.app.join("Contents/Versions/1.2.4/farhelm").exists());
        assert!(
            applications.join(".farhelm-app.lock").exists(),
            "the installer's lock is left alone"
        );
    }

    /// The default state directory is checked even when `XDG_STATE_HOME`
    /// names another.
    ///
    /// Why: the desktop app, opened from Finder or the Dock, does not see a
    /// shell's `XDG_STATE_HOME`, so its supervisor and helm hold their locks
    /// in the default location while uninstall, run from that shell, would
    /// otherwise only look at the XDG one. Spec: with `XDG_STATE_HOME`
    /// elsewhere and the default directory's `helm-token.lock` held,
    /// uninstall refuses naming it and removes nothing.
    #[test]
    fn a_lock_held_in_the_default_state_dir_refuses_despite_xdg() {
        let fixture = AppFixture::new();
        let state = tempfile::tempdir().expect("xdg state");
        let inputs = inputs(&fixture, state.path());
        let default = fixture.home.join(".local/state/farhelm");
        std::fs::create_dir_all(&default).expect("default state");
        let holder = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(default.join("helm-token.lock"))
            .expect("holder");
        holder.try_lock().expect("the app's helm holds the lock");
        let error = run_with_inputs(
            &inputs,
            &Options {
                dry_run: false,
                yes: true,
            },
            &mut NoUnits,
            &mut std::io::Cursor::new(Vec::new()),
            &mut Vec::new(),
        )
        .expect_err("a held default-dir lock refuses");
        assert!(error.to_string().contains("helm-token.lock"), "{error:#}");
        assert!(fixture.app.join("Contents/Versions/1.2.4/farhelm").exists());
    }

    fn fs_remove(path: &Path) {
        std::fs::remove_file(path).expect("remove fixture file");
    }

    // ---- The desktop app's own supervisor -------------------------------
    //
    // A supervisor the desktop app started can outlive the app and hold
    // `supervisor.lock` with the app closed. Uninstall stops it itself, but
    // only the one the app recorded, only once an open app has been ruled
    // out, and only the orderly way. The stand-ins below are copies of
    // `sleep` put in place of the fixture app's version programs, so each
    // really runs the program its record names; each holds a state
    // directory's supervisor lock as its inherited stdin, so the lock goes
    // when the process does.

    use farhelm_supervisor::managed_supervisor::{ManagedSupervisor, Observation, ProgramFile};

    /// A process standing in for a supervisor using `state_dir`, run from
    /// `program` (put in place with [`runnable`]), with the record the app
    /// would have written for it.
    ///
    /// Its exit status is collected on a thread, as init would collect a
    /// supervisor whose app has quit, so a stopped stand-in reads as gone
    /// rather than lingering as this test's zombie.
    struct StandIn {
        record: ManagedSupervisor,
        exit: std::sync::mpsc::Receiver<std::process::ExitStatus>,
    }

    impl StandIn {
        /// Start one and write its record into `state_dir`. With
        /// `ignore_term` it ignores SIGTERM, standing in for a supervisor
        /// that does not stop in time: it starts as a shell that ignores the
        /// signal and exec's `program` (an ignored disposition survives
        /// exec), and is waited for until its record matches. A `pre_exec`
        /// hook would avoid the shell but forces the slower fork path, whose
        /// longer window lets the child hold other tests' lock descriptors.
        fn start(state_dir: &Path, program: &Path, ignore_term: bool) -> Self {
            std::fs::create_dir_all(state_dir).expect("fixture: state directory");
            let lock = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(state_dir.join("supervisor.lock"))
                .expect("fixture: supervisor lock");
            lock.try_lock().expect("fixture: the lock is free");
            // Read before the spawn, as the app must.
            let file = ProgramFile::read(program).expect("fixture: reading the program");
            let mut command = if ignore_term {
                let mut command = std::process::Command::new("/bin/sh");
                command
                    .args(["-c", "trap '' TERM; exec \"$0\" 60"])
                    .arg(program);
                command
            } else {
                let mut command = std::process::Command::new(program);
                command.arg("60");
                command
            };
            let mut child = command
                .stdin(lock)
                .spawn()
                .expect("fixture: spawning the stand-in");
            let record = ManagedSupervisor::for_process(child.id(), &file)
                .expect("fixture: the stand-in is alive");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while record.observe() != Ok(Observation::Matches) {
                assert!(
                    std::time::Instant::now() < deadline,
                    "fixture: the stand-in never exec'd its program"
                );
                // sleep-ok: polling interval for the stand-in's exec.
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            record
                .write(state_dir)
                .expect("fixture: writing the record");
            let (send, exit) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = send.send(child.wait().expect("fixture: waiting for the stand-in"));
            });
            Self { record, exit }
        }

        /// Whether the stand-in still runs, by the same observation the
        /// code under test uses: a stopped or killed process reads as gone
        /// at once, without waiting for the collector thread.
        fn running(&self) -> bool {
            self.record.observe() == Ok(Observation::Matches)
        }

        /// The signal that ended the stand-in, waiting briefly for the
        /// collector thread to see an exit that has already happened.
        fn ended_by(&self) -> Option<i32> {
            use std::os::unix::process::ExitStatusExt as _;
            self.exit
                .recv_timeout(std::time::Duration::from_secs(10))
                .expect("the stand-in exited")
                .signal()
        }
    }

    impl Drop for StandIn {
        /// Kill a stand-in a test left running. The record's own
        /// observation guards the signal; between it and the kill the
        /// collector could reap the process and the pid pass on, which is
        /// the same short window `ManagedSupervisor::stop` accepts.
        fn drop(&mut self) {
            if self.running() {
                // SAFETY: kill has no memory preconditions.
                unsafe { libc::kill(self.record.pid as libc::pid_t, libc::SIGKILL) };
            }
        }
    }

    /// Make `program` (a fixture file) a runnable copy of `sleep`, and
    /// return it.
    ///
    /// Copied by `cp`, not in this process: a file this multi-threaded test
    /// process holds open for writing can be inherited by a child another
    /// test is spawning at that moment, and exec'ing the copy then fails
    /// with "Text file busy".
    fn runnable(program: &Path) -> PathBuf {
        // Replaced rather than overwritten: `cp` onto an existing file keeps
        // that file's (non-executable) mode.
        let _ = std::fs::remove_file(program);
        let copied = std::process::Command::new("cp")
            .arg("/bin/sleep")
            .arg(program)
            .status()
            .expect("fixture: running cp");
        assert!(copied.success(), "fixture: copying sleep");
        program.to_path_buf()
    }

    /// The program the app at `fixture` runs its supervisor from (a fixture
    /// file until [`runnable`] makes it a program).
    fn app_program(fixture: &AppFixture) -> PathBuf {
        fixture.app.join("Contents/Versions/1.2.4/farhelm")
    }

    /// Run a confirmed uninstall with `inputs`, returning its result and
    /// what it printed.
    fn uninstall_with(inputs: &Inputs) -> (Result<()>, String) {
        let mut output = Vec::new();
        let result = run_with_inputs(
            inputs,
            &Options {
                dry_run: false,
                yes: true,
            },
            &mut NoUnits,
            &mut std::io::Cursor::new(Vec::new()),
            &mut output,
        );
        (result, String::from_utf8_lossy(&output).into_owned())
    }

    /// The plan line naming the stop of `stand_in` in `dir`.
    fn stop_line(stand_in: &StandIn, dir: &Path) -> String {
        format!(
            "stop the desktop app's supervisor (process {}) using {}",
            stand_in.record.pid,
            path_text(dir)
        )
    }

    /// Uninstall stops the supervisor the app it removes started, in both
    /// state directories it checks, and then proceeds.
    ///
    /// Why: a supervisor the desktop app started can outlive the app, so
    /// after an ordinary quit it still holds its lock and its version, and
    /// without this stop every uninstall would refuse until the user found
    /// and stopped a process they never started themselves. Spec: with the
    /// app quit and its recorded supervisor alive in the `XDG_STATE_HOME`
    /// state directory and in the default one (each running one of the
    /// app's versions), the plan names both stops; each is ended by SIGTERM
    /// (the orderly stop, never a kill) and its record is removed; the app
    /// is removed.
    #[test]
    fn the_apps_own_supervisor_is_stopped_and_the_uninstall_proceeds() {
        let fixture = AppFixture::new();
        let state = tempfile::tempdir().expect("state");
        let inputs = inputs(&fixture, state.path());
        let old = runnable(&fixture.app.join("Contents/Versions/1.2.3/farhelm"));
        let dirs = [
            state.path().join("farhelm"),
            fixture.home.join(".local/state/farhelm"),
        ];
        let stand_ins = [
            StandIn::start(&dirs[0], &runnable(&app_program(&fixture)), false),
            StandIn::start(&dirs[1], &old, false),
        ];
        let (result, output) = uninstall_with(&inputs);
        result.expect("uninstall stops the app's supervisors and proceeds");
        for (dir, stand_in) in dirs.iter().zip(&stand_ins) {
            assert!(output.contains(&stop_line(stand_in, dir)), "{output}");
            assert_eq!(
                stand_in.ended_by(),
                Some(libc::SIGTERM),
                "{}",
                dir.display()
            );
            assert_eq!(
                ManagedSupervisor::read(dir).unwrap(),
                None,
                "the record of a stopped supervisor is removed"
            );
        }
        assert!(!fixture.app.exists(), "{output}");
    }

    /// A supervisor the app did not start is left running, and uninstall
    /// refuses on its lock and says how to find it.
    ///
    /// Why: uninstall may stop only what this installation's app started. A
    /// supervisor run by hand or from another build is the user's, and a
    /// record naming a program that is not one of this app's versions (here
    /// a path that spells its way into the app with `..`, and a plain one
    /// outside it) must not make it ours. Spec: with such a live record and
    /// that process holding the supervisor lock, the plan names no stop,
    /// uninstall refuses naming the lock and `lsof`, the process keeps
    /// running, and nothing is removed.
    #[test]
    fn a_supervisor_the_app_did_not_start_is_not_stopped() {
        let fixture = AppFixture::new();
        let elsewhere = fixture.home.join("Elsewhere");
        std::fs::create_dir_all(&elsewhere).expect("fixture: another folder");
        let foreign = runnable(&elsewhere.join("farhelm"));
        let spelled_in = fixture.app.join("../../Elsewhere/farhelm");
        for program in [&foreign, &spelled_in] {
            let state = tempfile::tempdir().expect("state");
            let inputs = inputs(&fixture, state.path());
            let dir = state.path().join("farhelm");
            let stand_in = StandIn::start(&dir, program, false);
            let (result, output) = uninstall_with(&inputs);
            let error = result.expect_err("a foreign supervisor's lock refuses");
            let text = format!("{error:#}");
            assert!(
                text.contains("supervisor.lock") && text.contains("lsof"),
                "{text}"
            );
            assert!(
                !output.contains("stop the desktop app's supervisor"),
                "{output}"
            );
            assert!(
                stand_in.running(),
                "a supervisor the app did not start keeps running"
            );
            assert!(app_program(&fixture).exists(), "nothing removed");
        }
    }

    /// An open app refuses the uninstall before its supervisor is stopped.
    ///
    /// Why: the app's helm holds `helm-token.lock` while it is open, and
    /// stopping its supervisor under it would end the user's sessions only
    /// for the uninstall to refuse anyway. Spec: with the helm lock held and
    /// the app's recorded supervisor alive, uninstall refuses telling the
    /// user to quit Farhelm Desktop, the supervisor keeps running with its
    /// record in place, and nothing is removed.
    #[test]
    fn an_open_app_refuses_before_its_supervisor_is_stopped() {
        let fixture = AppFixture::new();
        let state = tempfile::tempdir().expect("state");
        let inputs = inputs(&fixture, state.path());
        let dir = state.path().join("farhelm");
        let stand_in = StandIn::start(&dir, &runnable(&app_program(&fixture)), false);
        let helm = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join("helm-token.lock"))
            .expect("helm lock");
        helm.try_lock().expect("the open app's helm holds the lock");
        let (result, _) = uninstall_with(&inputs);
        let error = result.expect_err("an open app refuses");
        assert!(
            format!("{error:#}").contains("quit Farhelm Desktop"),
            "{error:#}"
        );
        assert!(stand_in.running(), "the supervisor was not stopped");
        assert_eq!(
            ManagedSupervisor::read(&dir).unwrap(),
            Some(stand_in.record.clone())
        );
        assert!(app_program(&fixture).exists(), "nothing removed");
    }

    /// An installation that changed after the user confirmed refuses before
    /// the supervisor is stopped.
    ///
    /// Why: SPEC.md orders the stop after the installation-ownership checks;
    /// stopping the user's supervisor for an uninstall that then refuses
    /// over a changed plan would end their sessions for nothing. Spec: when
    /// the app changes as the user confirms, uninstall refuses saying the
    /// installation changed, and the supervisor keeps running with its
    /// record in place.
    #[test]
    fn a_plan_that_changed_after_confirmation_stops_nothing() {
        let fixture = AppFixture::new();
        let state = tempfile::tempdir().expect("state");
        let dir = state.path().join("farhelm");
        let stand_in = StandIn::start(&dir, &runnable(&app_program(&fixture)), false);
        let desktop = fixture.app.join("Contents/MacOS/farhelm-desktop");
        let mut input = ConfirmWhile {
            answer: std::io::Cursor::new(b"y\n".to_vec()),
            meanwhile: Some(|| fs_remove(&desktop)),
        };
        let error = run_with_inputs(
            &inputs(&fixture, state.path()),
            &Options {
                dry_run: false,
                yes: false,
            },
            &mut NoUnits,
            &mut input,
            &mut Vec::new(),
        )
        .expect_err("a changed plan refuses");
        assert!(
            format!("{error:#}").contains("changed after you confirmed"),
            "{error:#}"
        );
        assert!(stand_in.running(), "the supervisor was not stopped");
        assert_eq!(
            ManagedSupervisor::read(&dir).unwrap(),
            Some(stand_in.record.clone())
        );
    }

    /// A dry run names the supervisor uninstall would stop, and stops
    /// nothing.
    ///
    /// Why: SPEC.md has `--dry-run` show what the uninstall would do without
    /// changing anything, and stopping a process is now part of that. Spec:
    /// with the app's recorded supervisor alive, `--dry-run` prints the stop
    /// with its process number and state directory, and the supervisor keeps
    /// running.
    #[test]
    fn a_dry_run_names_the_stop_and_stops_nothing() {
        let fixture = AppFixture::new();
        let state = tempfile::tempdir().expect("state");
        let inputs = inputs(&fixture, state.path());
        let dir = state.path().join("farhelm");
        let stand_in = StandIn::start(&dir, &runnable(&app_program(&fixture)), false);
        let mut output = Vec::new();
        run_with_inputs(
            &inputs,
            &Options {
                dry_run: true,
                yes: false,
            },
            &mut NoUnits,
            &mut std::io::Cursor::new(Vec::new()),
            &mut output,
        )
        .expect("dry run");
        let output = String::from_utf8_lossy(&output);
        assert!(output.contains(&stop_line(&stand_in, &dir)), "{output}");
        assert!(stand_in.running(), "a dry run stops nothing");
        assert!(app_program(&fixture).exists());
    }

    /// A supervisor that does not stop within the grace refuses the
    /// uninstall with nothing removed, and is not killed.
    ///
    /// Why: killing a supervisor can take its tmux server and every session
    /// with it, so uninstall never escalates; and removing the app under a
    /// supervisor still running from it is what the locks exist to prevent.
    /// Spec: with the app's recorded supervisor ignoring SIGTERM, uninstall
    /// refuses saying it did not stop and that nothing was removed; the
    /// supervisor keeps running with its record in place.
    #[test]
    fn a_supervisor_that_does_not_stop_refuses_with_nothing_removed() {
        let fixture = AppFixture::new();
        let state = tempfile::tempdir().expect("state");
        let mut inputs = inputs(&fixture, state.path());
        inputs.supervisor_stop_grace = std::time::Duration::from_millis(500);
        let dir = state.path().join("farhelm");
        let stand_in = StandIn::start(&dir, &runnable(&app_program(&fixture)), true);
        let (result, _) = uninstall_with(&inputs);
        let text = format!(
            "{:#}",
            result.expect_err("a supervisor that does not stop refuses")
        );
        assert!(
            text.contains("did not stop") && text.contains("nothing was removed"),
            "{text}"
        );
        assert!(stand_in.running(), "uninstall never kills");
        assert_eq!(
            ManagedSupervisor::read(&dir).unwrap(),
            Some(stand_in.record.clone())
        );
        assert!(app_program(&fixture).exists(), "nothing removed");
        assert_eq!(
            record_of(&fixture).state,
            farhelm_supervisor::installation::InstallState::Complete
        );
    }

    /// A refusal after the supervisor was stopped says so.
    ///
    /// Why: "nothing was removed" is true of files, but the user's sessions
    /// have lost their supervisor until the app is opened again, and the
    /// refusal is the only place they would learn it. Spec: when a version
    /// is still in use after uninstall stopped the app's supervisor, the
    /// refusal names the version and adds that the supervisor was stopped
    /// and opening Farhelm starts it again.
    #[test]
    fn a_refusal_after_the_stop_says_the_supervisor_was_stopped() {
        let fixture = AppFixture::new();
        let state = tempfile::tempdir().expect("state");
        let inputs = inputs(&fixture, state.path());
        let dir = state.path().join("farhelm");
        let stand_in = StandIn::start(&dir, &runnable(&app_program(&fixture)), false);
        let old =
            std::fs::File::open(fixture.app.join("Contents/Versions/1.2.3/farhelm")).expect("open");
        old.lock_shared()
            .expect("a running 1.2.3 holds its version");
        let (result, _) = uninstall_with(&inputs);
        let text = format!("{:#}", result.expect_err("a version in use refuses"));
        assert!(
            text.contains("Farhelm 1.2.3 is still in use")
                && text.contains("already stopped the desktop app's supervisor"),
            "{text}"
        );
        assert_eq!(stand_in.ended_by(), Some(libc::SIGTERM));
    }

    /// A record replaced after the user confirmed is not acted on: the
    /// supervisor the plan named is not stopped, and the lock then refuses.
    ///
    /// Why: the user confirmed stopping one particular process; another app
    /// launch between the plan and the stop may have replaced the record,
    /// and the stop must not follow it to a process the user was not shown.
    /// Spec: when the record changes as the user confirms (to another live
    /// process running one of the app's versions), uninstall stops neither
    /// process, refuses on the supervisor lock, and removes nothing.
    #[test]
    fn a_record_replaced_after_confirmation_is_not_acted_on() {
        let fixture = AppFixture::new();
        let state = tempfile::tempdir().expect("state");
        let dir = state.path().join("farhelm");
        let planned = StandIn::start(&dir, &runnable(&app_program(&fixture)), false);
        // The replacement holds another directory's lock, which is all a
        // stand-in for "another process" needs; its record is then written
        // over the planned one.
        let other = StandIn::start(
            &state.path().join("elsewhere"),
            &runnable(&fixture.app.join("Contents/Versions/1.2.3/farhelm")),
            false,
        );
        let (record_dir, replacement) = (dir.clone(), other.record.clone());
        let mut input = ConfirmWhile {
            answer: std::io::Cursor::new(b"y\n".to_vec()),
            meanwhile: Some(move || {
                replacement
                    .write(&record_dir)
                    .expect("fixture: replacing the record");
            }),
        };
        let error = run_with_inputs(
            &inputs(&fixture, state.path()),
            &Options {
                dry_run: false,
                yes: false,
            },
            &mut NoUnits,
            &mut input,
            &mut Vec::new(),
        )
        .expect_err("the planned supervisor still holds its lock");
        assert!(
            format!("{error:#}").contains("supervisor.lock"),
            "{error:#}"
        );
        assert!(planned.running(), "the planned supervisor was not stopped");
        assert!(other.running(), "the replacement was not stopped");
        assert!(app_program(&fixture).exists(), "nothing removed");
    }
}
