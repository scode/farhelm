//! Remove the selected standalone installation while preserving user data.
//!
//! File ownership is established before confirmation and re-checked after it,
//! under the locks install, setup and (on macOS) the runtime use; see `locks`.
//! Runtime shutdown is an operator prerequisite; this command does not inspect
//! processes or sessions. Setup-owned services are the sole automatic shutdown
//! operation.

use anyhow::{Context as _, Result, bail};
use std::{
    ffi::OsString,
    fmt::Write as _,
    io::{BufRead, IsTerminal as _, Read as _, Write},
    path::{Path, PathBuf},
};

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
    let services = if inputs.ownership.platform == ownership::PlatformArtifacts::Linux {
        let directory = farhelm_helm::units::user_unit_dir(inputs.xdg_config_home.as_deref(), home);
        Some(crate::setup::preflight_selected_services(
            std::slice::from_ref(&plan.flat.cli),
            &directory,
            manager,
        )?)
    } else {
        None
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
    if let ownership::BundleInspection::Recognized(bundle) = &plan.bundle {
        writeln!(
            preview,
            "remove installer-owned app bundle {}",
            path_text(&bundle.root)
        )?;
    }
    if let ownership::BundleInspection::RetainedWithoutReceipt(root) = &plan.bundle {
        writeln!(
            preview,
            "retain app bundle {}: it has no Farhelm installer receipt, so nothing shows this installation made it",
            path_text(root)
        )?;
    }
    if let Some(path) = &plan.flat.desktop {
        writeln!(preview, "remove {}", path_text(path))?;
    }
    if let Some(path) = &plan.flat.retained_foreign_desktop {
        writeln!(preview, "retain unrelated file {}", path_text(path))?;
    }
    writeln!(preview, "remove {} last", path_text(&plan.flat.cli))?;
    writeln!(
        preview,
        "retain shared directory {}",
        path_text(&plan.flat.root)
    )?;
    let state = farhelm_supervisor::default_state_dir_for(inputs.xdg_state_home.as_deref(), home);
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
    let held = lock_and_recheck(inputs, home, &state, &plan, services.as_ref(), manager)?;
    let mut report = String::new();
    let result = (|| {
        if let Some(services) = &services {
            crate::setup::remove_selected_services_locked(services, false, manager, &mut report)?;
        }
        removal::remove(&plan, &mut report)
    })();
    drop(held);
    if result.is_err() {
        writeln!(
            report,
            "Uninstall is incomplete. The CLI remains at {}; resolve the reported failure and rerun uninstall.",
            path_text(&plan.flat.cli)
        )?;
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
struct HeldLocks {
    _runtime: Option<locks::RuntimeLocks>,
    _setup: Option<crate::setup::SetupLock>,
    _bundle: Option<locks::BundleLock>,
    _install: locks::InstallLock,
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
    manager: &mut dyn crate::setup::UnitManager,
) -> Result<HeldLocks> {
    let macos = inputs.ownership.platform == ownership::PlatformArtifacts::Macos;
    let install = locks::InstallLock::acquire(&plan.flat.root)?;
    let bundle = if macos {
        locks::BundleLock::acquire(&home.join("Applications"))?
    } else {
        None
    };
    let setup = match services {
        Some(services) => crate::setup::lock_for_selected_services(services)?,
        None => None,
    };
    // The desktop app, opened from Finder or the Dock, does not see a
    // shell's `XDG_STATE_HOME`, so its supervisor and helm use the default
    // location even when this shell names another; both are checked.
    let runtime = if macos {
        let mut state_dirs = vec![state.to_path_buf()];
        let default = farhelm_supervisor::default_state_dir_for(None, home);
        if default != state {
            state_dirs.push(default);
        }
        Some(locks::RuntimeLocks::acquire(&state_dirs)?)
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
    if fresh != *plan {
        return Err(changed(&format!(
            "the installed files under {}",
            path_text(&plan.flat.root)
        )));
    }
    if let Some(services) = services {
        let directory = farhelm_helm::units::user_unit_dir(inputs.xdg_config_home.as_deref(), home);
        let fresh = crate::setup::preflight_selected_services(
            std::slice::from_ref(&plan.flat.cli),
            &directory,
            manager,
        )
        .context("re-checking Farhelm's services after confirmation; nothing was removed")?;
        if fresh != *services {
            return Err(changed("the Farhelm services set up for this installation"));
        }
    }
    Ok(HeldLocks {
        _install: install,
        _bundle: bundle,
        _setup: setup,
        _runtime: runtime,
    })
}

/// Escape paths so a filename cannot forge an extra line of diagnostic output.
fn path_text(path: &Path) -> String {
    ownership::path_text(path)
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn inputs(fixture: &Fixture, state: &Path) -> Inputs {
        std::fs::create_dir_all(state.join("farhelm")).expect("state directory");
        Inputs {
            ownership: fixture.inputs(PlatformArtifacts::Macos),
            xdg_config_home: None,
            xdg_state_home: Some(state.as_os_str().to_os_string()),
            interactive: true,
        }
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
        let fixture = Fixture::new();
        fixture.flat(Some(b"desktop"));
        let state = tempfile::tempdir().expect("state");
        let desktop = fixture.install.join("farhelm-desktop");
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
            fixture.install.join("farhelm").exists(),
            "the CLI was not removed"
        );
        assert!(!fixture.install.join(".farhelm-install.lock").exists());
    }

    /// A held install lock refuses the uninstall before anything is removed.
    ///
    /// Why: the installer holds this lock while it replaces binaries and
    /// their ownership record; removing them meanwhile could delete what it
    /// just installed or leave the record and binaries disagreeing, which
    /// SPEC.md forbids. Spec: with the lock present at confirmation time,
    /// uninstall refuses naming it, removes nothing, and leaves the lock.
    #[test]
    fn a_held_install_lock_refuses_without_removing() {
        let fixture = Fixture::new();
        fixture.flat(Some(b"desktop"));
        let state = tempfile::tempdir().expect("state");
        let lock = fixture.install.join(".farhelm-install.lock");
        std::fs::create_dir(&lock).expect("installer lock");
        std::fs::write(lock.join("pid"), "1\n").expect("installer pid");
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
        .expect_err("a held lock refuses");
        assert!(
            error.to_string().contains(".farhelm-install.lock"),
            "{error:#}"
        );
        assert!(fixture.install.join("farhelm").exists());
        assert!(fixture.install.join("farhelm-desktop").exists());
        assert!(
            lock.join("pid").exists(),
            "another process's lock is left alone"
        );
    }

    /// The ordinary case still removes the installation and leaves no lock.
    ///
    /// Why: the locks must never outlive the uninstall that took them, or the
    /// next install would find a stale lock. Spec: with nothing else holding
    /// a lock, uninstall removes the CLI and leaves neither lock behind.
    #[test]
    fn an_uncontended_uninstall_removes_and_releases_its_locks() {
        let fixture = Fixture::new();
        fixture.flat(Some(b"desktop"));
        std::fs::create_dir(fixture.home.join("Applications")).expect("applications");
        let state = tempfile::tempdir().expect("state");
        run_with_inputs(
            &inputs(&fixture, state.path()),
            &Options {
                dry_run: false,
                yes: true,
            },
            &mut NoUnits,
            &mut std::io::Cursor::new(Vec::new()),
            &mut Vec::new(),
        )
        .expect("uninstall");
        assert!(!fixture.install.join("farhelm").exists());
        assert!(!fixture.install.join(".farhelm-install.lock").exists());
        assert!(!fixture.home.join("Applications/.farhelm-app.lock").exists());
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
        let fixture = Fixture::new();
        fixture.flat(Some(b"desktop"));
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
        assert!(fixture.install.join("farhelm").exists());
        assert!(fixture.install.join("farhelm-desktop").exists());
        assert!(!fixture.install.join(".farhelm-install.lock").exists());
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
        let fixture = Fixture::new();
        fixture.flat(Some(b"desktop"));
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
        assert!(fixture.install.join("farhelm").exists());
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
        let fixture = Fixture::new();
        fixture.flat(Some(b"desktop"));
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
        assert!(fixture.install.join("farhelm").exists());
    }

    fn fs_remove(path: &Path) {
        std::fs::remove_file(path).expect("remove fixture file");
    }
}
