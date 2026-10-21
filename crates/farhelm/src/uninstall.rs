//! Remove the selected standalone installation while preserving user data.
//!
//! On macOS the installation is the app bundle with its side-by-side versions
//! (see `app`); on Linux it is a standalone bin directory an earlier installer
//! left (see `ownership`). File ownership is established before confirmation
//! and re-checked after it, under the locks install, setup and (on macOS) the
//! runtime use; see `locks`.
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
    let mut held = lock_and_recheck(inputs, home, &state, &plan, services.as_ref(), manager)?;
    let mut report = String::new();
    let result = (|| {
        if let Some(services) = &services {
            crate::setup::remove_selected_services_locked(services, false, manager, &mut report)?;
        }
        match &plan {
            ownership::OwnershipPlan::App(app) => remove_app_locked(app, &mut held, &mut report),
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
             cannot be locked, uninstall cannot tell what still uses Farhelm",
            version.name.to_string_lossy(),
            path_text(&version.program)
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
    // The desktop app, opened from Finder or the Dock, does not see a
    // shell's `XDG_STATE_HOME`, so its supervisor and helm use the default
    // location even when this shell names another; both are checked.
    let mut running_records = Vec::new();
    let runtime = if macos {
        let mut state_dirs = vec![state.to_path_buf()];
        let default = farhelm_supervisor::default_state_dir_for(None, home);
        if default != state {
            state_dirs.push(default);
        }
        running_records = state_dirs
            .iter()
            .map(|dir| dir.join(farhelm_supervisor::app_bundle::RUNNING_RECORD))
            .collect();
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
    Ok(HeldLocks {
        running_records,
        _install: install,
        installation,
        app_folder,
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
}
