//! `farhelm helm setup`: install, own, and remove the systemd user units
//! that run a helm and its supervisor on this machine.
//!
//! This replaces the unit files releases used to ship for the operator to
//! copy by hand (plan D9). The units are rendered from the templates
//! compiled into `farhelm-helm`, so remote provisioning and local setup
//! cannot drift, and every file setup writes carries
//! [`farhelm_helm::units::MANAGED_MARKER`] as its first line.
//!
//! ## Ownership
//!
//! Setup overwrites or removes ONLY marked files. A unit somebody else
//! wrote — by hand, or by a distribution package — makes setup refuse and
//! say so, rather than replacing it. The matching protection on the other
//! side is in the helm: the hosts panel's local row never installs or
//! updates a supervisor on the helm's own machine.
//!
//! ## Why everything is injected
//!
//! [`SetupContext`] carries the whole process environment this command
//! depends on — the executable, `HOME`, `PATH`, `XDG_CONFIG_HOME`,
//! `FARHELM_TMUX`, and the temporary directory — captured ONCE in `main`.
//! [`UnitManager`] is the seam for `systemctl --user`, and output goes
//! through a `Write`. Nothing below reads the environment, spawns
//! `systemctl`, or prints to stdout on its own, which is what lets the
//! tests drive the whole command (including its refusals and its exact
//! command sequences) without mutating the test process's environment.
//!
//! The one thing that is NOT injected is the filesystem: setup reads and
//! writes real unit files, and the tests point it at temporary
//! directories.

use anyhow::{Context as _, bail};
use farhelm_helm::units::{
    HELM_UNIT_NAME, HelmUnitInputs, SUPERVISOR_UNIT_NAME, SupervisorUnitInputs, is_managed,
    managed, render_helm_unit, render_supervisor_unit, user_unit_dir, user_unit_dir_for,
};
use farhelm_supervisor::tmux::{
    TMUX_FLOOR, TmuxProbeError, TmuxSupport, candidates_on_path, probe_tmux,
};
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};

/// The process facts setup is allowed to depend on, captured once by the
/// caller.
///
/// `exe` is the binary the units will point at — the running one, because
/// "set up the thing I just ran" is the only interpretation that cannot
/// surprise the operator. `temp_dir` is here only for the build-tree
/// check; it is a parameter because `/tmp` is where a test's fixture
/// binaries live and the check has to be exercisable.
///
/// Both XDG variables are carried, and for different reasons.
/// `xdg_config_home` decides which directory the user manager searches for
/// units; `xdg_state_home` decides where a helm or supervisor started
/// from those units will keep its state, so setup has to resolve the
/// default state directory the same way they will or the supervisor it
/// pins and the helm it enables would look for each other's socket in
/// different places.
///
/// `cwd` is what every relative path the operator types is resolved
/// against, and it is here rather than read on demand because it is
/// LOAD-BEARING: systemd starts a user service with its own working
/// directory (the user's home, absent a `WorkingDirectory=`), so a
/// relative path copied verbatim into a unit means a different directory
/// at start time than it did on the command line. Everything setup pins
/// is made absolute against this before it reaches a unit file.
pub struct SetupContext {
    pub exe: PathBuf,
    pub home: PathBuf,
    pub cwd: PathBuf,
    pub path: OsString,
    pub xdg_config_home: Option<OsString>,
    pub xdg_state_home: Option<OsString>,
    pub tmux_env: Option<OsString>,
    pub temp_dir: PathBuf,
}

impl SetupContext {
    /// Resolve one operator-supplied path the way the operator meant it:
    /// relative to where they ran the command.
    ///
    /// Symlinks are deliberately NOT resolved. Homebrew's `bin/tmux` is a
    /// symlink into a versioned Cellar directory, and pinning the resolved
    /// path would break the unit on the next `brew upgrade`; the same
    /// applies to a state directory reached through a stable symlink.
    fn absolute(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        }
    }
}

/// What the operator asked for on the command line.
///
/// This IS the CLI surface — `main.rs` hands the parsed value straight to
/// [`run_setup`]. Keeping one type means a new flag is declared, helped,
/// and consumed in one place instead of copied between a clap struct and
/// a mirror of it. Tests build it with `..Default::default()`.
#[derive(clap::Args, Default)]
pub struct SetupOptions {
    /// State directory to pin. Default:
    /// ${XDG_STATE_HOME:-$HOME/.local/state}/farhelm.
    ///
    /// Resolved once and pinned into BOTH units either way, so the helm
    /// and its supervisor cannot end up on different trees and lose each
    /// other's socket — the default is resolved HERE, from this command's
    /// environment, rather than left to each service to work out from the
    /// systemd user manager's. A relative path is resolved against the
    /// directory setup runs in, because systemd would otherwise resolve it
    /// against the service's own working directory.
    #[arg(long)]
    pub state_dir: Option<PathBuf>,
    /// Port to pin into the helm unit (default: the helm's own).
    #[arg(long)]
    pub port: Option<u16>,
    /// Pin this tmux binary instead of the one on PATH.
    #[arg(long = "tmux", value_name = "PATH")]
    pub tmux: Option<PathBuf>,
    /// Manage only the helm unit, leaving the supervisor to whoever owns
    /// it here — a unit you wrote yourself, another service manager, or a
    /// supervisor somebody starts by hand. Setup then neither writes nor
    /// touches farhelm-supervisor.service, and does not look for a tmux.
    #[arg(long)]
    pub no_supervisor: bool,
    /// Print what would be written and run, and change nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// Remove the units setup wrote, and only those.
    #[arg(
        long,
        conflicts_with_all = ["state_dir", "port", "tmux", "no_supervisor"]
    )]
    pub uninstall: bool,
}

/// The result of one `systemctl --user` invocation.
///
/// A non-zero `status` is data, not an error: `is-active` answers 3 for an
/// inactive unit, and setup reads that answer to decide whether a rewritten
/// unit needs restarting.
pub struct UnitCommand {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// The seam over `systemctl --user`.
///
/// `Err` means the child could not be EXECUTED OR COLLECTED — no
/// `systemctl` on PATH, a spawn refused, a wait or pipe read that failed.
/// Everything `systemctl` itself reported is an ordinary [`UnitCommand`]
/// with its status and both streams intact, including the failures that
/// matter most in practice: a user manager that is not running answers
/// through a non-zero exit, not a spawn error.
pub trait UnitManager {
    fn run(&mut self, args: &[&str]) -> anyhow::Result<UnitCommand>;
}

/// The production [`UnitManager`]: plain `systemctl --user <args>`.
pub struct SystemctlUnitManager;

impl UnitManager for SystemctlUnitManager {
    /// Synchronous and blocking on purpose — this is a CLI command, and
    /// wrapping its short, strictly ordered handful of `systemctl` calls
    /// (a status query per changed unit, one reload, one enable per unit,
    /// and a restart for each unit that was running) in an async runtime
    /// would buy nothing.
    ///
    /// stdin is closed so a `systemctl` that decides to page or prompt
    /// cannot wedge a non-interactive run. A signal-killed child reports
    /// status -1, which no systemctl exit code collides with.
    fn run(&mut self, args: &[&str]) -> anyhow::Result<UnitCommand> {
        let output = std::process::Command::new("systemctl")
            .arg("--user")
            .args(args)
            .stdin(std::process::Stdio::null())
            .output()
            .with_context(|| format!("running systemctl --user {}", args.join(" ")))?;
        Ok(UnitCommand {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// One unit file setup intends to have on disk, fully rendered.
struct PlannedUnit {
    name: &'static str,
    path: PathBuf,
    text: String,
}

/// Run the command. See the module docs for the injection contract.
///
/// Both halves live here: `opts.uninstall` removes the units setup owns,
/// anything else installs or converges them. Returns `Err` for every
/// refusal (the operator sees the message and a non-zero exit); everything
/// else is reported through `out`.
///
/// Both halves share one precondition, checked here before either runs:
/// the directory this environment selects has to be the one the RUNNING
/// user manager reads. Installing elsewhere publishes units nothing loads;
/// uninstalling elsewhere reports two files "absent" and leaves the real
/// ones in place, which is the same mistake wearing a reassuring answer.
///
/// The transcript is BUFFERED and written once, at the end, and that is a
/// correctness property rather than a performance one: this function
/// replaces unit files and drives systemd, and a closed stdout partway
/// through must not be able to abandon that work half done. Writing the
/// buffer is the last thing that happens, on the failure path too, so a
/// refusal still explains itself.
pub fn run_setup(
    ctx: &SetupContext,
    opts: &SetupOptions,
    units: &mut dyn UnitManager,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let unit_dir = user_unit_dir(ctx.xdg_config_home.as_deref(), &ctx.home);
    let mut report = String::new();
    let converged = require_the_managers_unit_directory(opts, units, &mut report, &unit_dir)
        .and_then(|()| {
            if opts.uninstall {
                uninstall(&unit_dir, opts, units, &mut report)
            } else {
                install(ctx, opts, &unit_dir, units, &mut report)
            }
        });
    let written = out
        .write_all(report.as_bytes())
        .and_then(|()| out.flush())
        .context("writing the setup report");
    // Convergence is the more important failure of the two: a broken
    // stdout is a presentation problem, an unfinished unit install is not.
    converged.and(written)
}

/// The transcript setup builds as it goes.
///
/// A `String` rather than the caller's writer, so no `writeln!` in the
/// convergence path can fail. `std::fmt::Write` on a `String` is
/// infallible, and the `expect` on it says so once rather than at every
/// call site.
type Report = String;

/// Append one line to the transcript.
macro_rules! line {
    ($report:expr) => {
        line!($report, "")
    };
    ($report:expr, $($arg:tt)*) => {{
        use std::fmt::Write as _;
        writeln!($report, $($arg)*).expect("writing to a String cannot fail");
    }};
}

/// Converge both unit files and the systemd state around them.
///
/// Two things outlive a single call and live in the unit directory beside
/// the units, both dot-prefixed so systemd ignores them:
///
/// - `.farhelm-setup.lock`, held from the ownership preflight through the
///   last mutation so two setups cannot interleave into a mixed pair (see
///   [`SetupLock`]).
/// - `.<unit>.restart-pending`, written before a RUNNING unit's bytes are
///   replaced and removed only once that unit's restart has succeeded, so
///   a run that failed anywhere in between leaves the obligation for the
///   next one to discharge (see [`restart_marker`]).
///
/// Neither is a configuration file and neither needs cleaning up by hand;
/// an uninstall takes the markers with the units.
fn install(
    ctx: &SetupContext,
    opts: &SetupOptions,
    unit_dir: &Path,
    units: &mut dyn UnitManager,
    report: &mut Report,
) -> anyhow::Result<()> {
    let exe = std::fs::canonicalize(&ctx.exe)
        .with_context(|| format!("resolving this farhelm binary at {}", ctx.exe.display()))?;
    if looks_like_a_build_tree(&exe, &ctx.temp_dir) {
        let refusal = format!(
            "refusing to point a unit at {}: that looks like a build tree. Install farhelm first \
             (see README, \"Install\") and run setup from the installed binary.",
            exe.display()
        );
        if !opts.dry_run {
            bail!(refusal);
        }
        line!(
            report,
            "note: {} looks like a build tree; a real setup would refuse",
            exe.display()
        );
    }
    // Resolved ONCE, absolutely, and written into BOTH units — including
    // when the operator gave no `--state-dir` at all.
    //
    // Two different traps close here. A relative path means what the
    // operator typed it to mean, relative to THEIR directory, while
    // systemd would resolve it against the service's own. And an omitted
    // flag is not a safe "let each service work it out": the helm would
    // resolve its default from the environment the systemd user manager
    // was started with, which is not the environment setup ran in, so
    // `XDG_STATE_HOME=/srv/state farhelm helm setup` would pin
    // `/srv/state/farhelm` into the supervisor (whose template cannot omit
    // it) and leave the helm on `~/.local/state/farhelm`. Both services
    // would start, and the helm would then look for the supervisor's
    // socket in a directory nothing listens in.
    let state_dir = match opts.state_dir.as_deref() {
        Some(pinned) => ctx.absolute(pinned),
        None => farhelm_supervisor::default_state_dir_for(ctx.xdg_state_home.as_deref(), &ctx.home),
    };

    let mut planned = Vec::new();
    if !opts.no_supervisor {
        // The stand-in keeps `--dry-run` useful on a machine that has no
        // acceptable tmux yet: the operator still sees the unit they would
        // get, with the one unknown spelled out.
        let tmux = match choose_tmux(ctx, opts) {
            TmuxDecision::Accepted { program, warning } => {
                if let Some(warning) = warning {
                    line!(report, "note: {warning}");
                }
                program
            }
            TmuxDecision::Refused(refusal) => {
                if !opts.dry_run {
                    bail!(refusal);
                }
                line!(report, "note: {refusal}");
                PathBuf::from("<tmux>")
            }
        };
        planned.push(PlannedUnit {
            name: SUPERVISOR_UNIT_NAME,
            path: unit_dir.join(SUPERVISOR_UNIT_NAME),
            text: managed(render_supervisor_unit(&SupervisorUnitInputs {
                farhelm: &exe,
                state_dir: &state_dir,
                tmux: &tmux,
            })?),
        });
    }
    planned.push(PlannedUnit {
        name: HELM_UNIT_NAME,
        path: unit_dir.join(HELM_UNIT_NAME),
        // The same resolved state directory the supervisor unit carries.
        // `--port` is the only flag left that appears solely when given;
        // see `HelmUnitInputs`.
        text: managed(render_helm_unit(&HelmUnitInputs {
            farhelm: &exe,
            state_dir: &state_dir,
            port: opts.port,
        })?),
    });

    // Held from here to the end of the function, which is what makes the
    // preflight below mean anything: the two unit files are ONE
    // configuration (they must name the same state directory), and two
    // setups that both passed the same preflight could otherwise
    // interleave their renames and leave a supervisor from one beside a
    // helm from the other.
    let _lock = if opts.dry_run {
        None
    } else {
        Some(lock_unit_directory(unit_dir)?)
    };

    // Every ownership check happens before the first write — and after
    // the lock, so what it reads cannot change underneath it — so a
    // refusal on the second unit cannot leave the first one replaced.
    let existing = planned
        .iter()
        .map(|unit| existing_managed_text(&unit.path))
        .collect::<anyhow::Result<Vec<_>>>()?;

    // Whether each planned unit's bytes differ from what is on disk. What
    // to do about a unit that was RUNNING the old bytes is recorded on
    // disk instead, by `owe_a_restart` — see `restart_marker`.
    let mut changed_units = Vec::new();
    for (unit, existing) in planned.iter().zip(&existing) {
        let changed = existing.as_deref() != Some(unit.text.as_str());
        if opts.dry_run {
            // The preview shows the status query too, because the restart
            // it decides is the one command here that can interrupt a
            // running helm — the last thing to spring on somebody who
            // asked what would happen.
            if changed {
                would_run(report, &["is-active", unit.name]);
            }
            line!(
                report,
                "{} {}\n{}",
                if changed { "would write" } else { "unchanged" },
                unit.path.display(),
                unit.text
            );
        } else if changed {
            // Asked BEFORE the write, because after it there is no way to
            // tell a unit that was already running from one this run just
            // started.
            if unit_is_active(units, report, unit.name)? {
                // ...and RECORDED before the write, because everything
                // between here and the restart can fail. See
                // `restart_marker`.
                owe_a_restart(unit_dir, unit.name)?;
            }
            write_unit(&unit.path, &unit.text)?;
            line!(report, "written {}", unit.path.display());
        } else {
            line!(report, "unchanged {}", unit.path.display());
        }
        changed_units.push(changed);
    }

    // Unconditional, and cheap: systemd only rereads what changed on disk.
    // Making it conditional on THIS run having written bytes is what made
    // setup non-convergent — a run that published a unit and then failed
    // to reload left the next run seeing identical bytes and skipping the
    // reload its predecessor never completed.
    require_command(units, opts, report, &["daemon-reload"])?;
    // `enable --now` runs for every unit on every run, changed or not: it
    // is idempotent, and "setup leaves your helm enabled and running" is
    // the promise the command makes. A unit that was enabled but stopped
    // by hand comes back here, which is the intended repair path.
    for unit in &planned {
        require_command(units, opts, report, &["enable", "--now", unit.name])?;
    }
    // Restarts are driven by the markers on disk, not by what THIS run
    // did. A previous run that published new bytes for a running unit and
    // then failed — at the reload, at an enable, at the other restart —
    // left its obligation here, and this run owes it even though the file
    // now matches and looks unchanged. `enable --now` does not repair
    // that: it starts a stopped unit, it does not restart a running one.
    for (unit, changed) in planned.iter().zip(&changed_units) {
        let owed = restart_marker(unit_dir, unit.name).exists();
        if opts.dry_run {
            if owed {
                // Already owed by an earlier run: this one is not
                // conditional on anything.
                would_run(report, &["restart", unit.name]);
            } else if *changed {
                // Dry-run asks systemd nothing beyond the read-only
                // environment query, so it states the condition instead of
                // pretending to know whether the unit is running.
                line!(
                    report,
                    "would run (if active): systemctl --user restart {}",
                    unit.name
                );
            }
        } else if owed {
            require_command(units, opts, report, &["restart", unit.name])?;
            settle_the_restart(unit_dir, unit.name)?;
        }
    }

    line!(report);
    line!(
        report,
        "loginctl enable-linger \"$USER\"   # start at boot and survive logout"
    );
    line!(
        report,
        "farhelm helm token show   # the browser sign-in token"
    );
    Ok(())
}

/// Remove the units setup owns, and only those.
///
/// `unit_dir` has already been reconciled with the running manager by
/// [`run_setup`], which matters as much here as it does for an install: a
/// caller whose `XDG_CONFIG_HOME` differs from the manager's would
/// otherwise be told both units are absent while the real ones stayed
/// enabled — a clean bill of health for a machine that was never touched.
fn uninstall(
    unit_dir: &Path,
    opts: &SetupOptions,
    units: &mut dyn UnitManager,
    report: &mut Report,
) -> anyhow::Result<()> {
    // The same lock the install path takes, for the same reason: an
    // uninstall that raced a setup could act on preflight results the
    // other command had already invalidated and leave exactly one of the
    // pair installed.
    let _lock = if opts.dry_run {
        None
    } else {
        Some(lock_unit_directory(unit_dir)?)
    };

    // BOTH targets are inspected before either is touched. Checking them
    // one at a time meant a foreign helm unit was discovered only after
    // the supervisor had already been disabled and deleted, leaving a
    // half-uninstalled machine and — because the refusal came before
    // `daemon-reload` — a systemd that still had the removed unit loaded.
    let targets = [SUPERVISOR_UNIT_NAME, HELM_UNIT_NAME]
        .map(|name| (name, unit_dir.join(name)))
        .map(|(name, path)| existing_managed_text(&path).map(|text| (name, path, text.is_some())));
    let targets = targets.into_iter().collect::<anyhow::Result<Vec<_>>>()?;

    let mut failure = None;
    for (name, path, present) in &targets {
        if !present {
            line!(report, "absent {}", path.display());
            continue;
        }
        // Disabled BEFORE the file goes away: systemd needs the unit file
        // present to remove the symlinks that enabled it.
        let removed =
            require_command(units, opts, report, &["disable", "--now", name]).and_then(|()| {
                if opts.dry_run {
                    line!(report, "would remove {}", path.display());
                } else {
                    std::fs::remove_file(path)
                        .with_context(|| format!("removing {}", path.display()))?;
                    line!(report, "removed {}", path.display());
                }
                Ok(())
            });
        if let Err(error) = removed {
            failure = Some(error);
            break;
        }
        // Nothing is owed a restart once its unit is gone, and a marker
        // left here would make the NEXT install bounce a service it had
        // just started.
        settle_the_restart(unit_dir, name)?;
    }
    // UNCONDITIONAL on every real run, including one that found both files
    // already absent. Deriving it from "did this invocation delete
    // something" is what made uninstall non-convergent: a run that deleted
    // both units and then failed at the reload left systemd holding
    // definitions for files that no longer exist, and the retry — seeing
    // nothing to delete — skipped the reload its predecessor never
    // finished and reported success.
    let reloaded = require_command(units, opts, report, &["daemon-reload"]);
    if failure.is_none() {
        failure = reloaded.err();
    }
    // Drop-ins are the operator's own configuration and setup never wrote
    // them, so it never deletes them either — but leaving them silently
    // would let a stale override apply to a unit written by a later setup.
    for (name, _, _) in &targets {
        let drop_ins = unit_dir.join(format!("{name}.d"));
        if drop_ins.is_dir() {
            line!(
                report,
                "drop-ins under {}/{name}.d/ were left in place",
                unit_dir.display()
            );
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// An exclusive, non-blocking lock over one unit directory, held for as
/// long as the value lives.
///
/// The two unit files are ONE configuration — they have to name the same
/// state directory or the helm cannot find the supervisor's socket — and
/// they are published through two separate renames. Without a lock, two
/// setups with different options can both pass the same ownership
/// preflight and interleave: A publishes its supervisor, B replaces that
/// supervisor and publishes its helm, A finally replaces the helm. Both
/// report success; the machine ends up with B's supervisor beside A's
/// helm. An uninstall racing a setup is the same hazard with one of the
/// pair left behind.
///
/// NON-BLOCKING on purpose. Waiting would turn a mistake (two setups at
/// once, usually a stuck first one) into a command that hangs with no
/// explanation; refusing says what is happening and what to do.
///
/// The one concession is a brief retry, and it is not flakiness
/// insurance: `flock` belongs to the open file description, so a `fork`
/// anywhere in the process between this file being opened and closed
/// hands the child a duplicate that keeps the lock until the child
/// `exec`s. Close-on-exec bounds that window to microseconds but does not
/// remove it, and a command refusing because of it would be refusing over
/// its own descendant. Anything still holding the lock after the budget
/// below is a second setup, which is what the refusal is for.
///
/// `--dry-run` does not take it: it writes nothing to serialize, and a
/// preview should never be refused because somebody else is mid-install.
struct SetupLock {
    /// Kept open because closing it releases the lock. Nothing reads it.
    _file: std::fs::File,
}

/// The lock file's own name. Dot-prefixed so systemd ignores it, and left
/// in place afterwards: an empty lock file is cheaper to keep than the
/// race that deleting it while another process holds it would open.
const LOCK_FILE: &str = ".farhelm-setup.lock";

fn lock_unit_directory(unit_dir: &Path) -> anyhow::Result<SetupLock> {
    use std::os::fd::AsRawFd as _;

    std::fs::create_dir_all(unit_dir)
        .with_context(|| format!("creating {}", unit_dir.display()))?;
    let path = unit_dir.join(LOCK_FILE);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .with_context(|| format!("opening {}", path.display()))?;
    /// Long enough to outlast an inherited descriptor's fork-to-exec
    /// window, short enough that a real second setup is still reported
    /// rather than waited on.
    const ACQUIRE_BUDGET: std::time::Duration = std::time::Duration::from_millis(250);

    let deadline = std::time::Instant::now() + ACQUIRE_BUDGET;
    loop {
        // SAFETY: `flock` takes a descriptor this function owns and
        // touches no memory. `LOCK_NB` makes it answer instead of
        // blocking.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(SetupLock { _file: file });
        }
        if std::time::Instant::now() >= deadline {
            bail!(
                "another farhelm helm setup is running for {}; wait for it to finish, then rerun",
                unit_dir.display()
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Where one unit's outstanding restart obligation is recorded.
///
/// A unit whose file changed while it was RUNNING has to be restarted:
/// `daemon-reload` teaches systemd the new definition and `enable --now`
/// starts a stopped unit, but neither makes a running process adopt a new
/// executable, state directory, tmux path, or port. Remembering that only
/// in this invocation's memory was a real hole — a run that published the
/// bytes and then failed (at the reload, at an enable, at the other
/// restart) left the next run seeing a file that already matched, calling
/// it unchanged, and reporting success over a process still running the
/// old configuration.
///
/// So the obligation is written down BEFORE the bytes are published and
/// cleared only once the restart has actually succeeded. Dot-prefixed so
/// systemd ignores the file; it lives beside the units because that is the
/// directory this command owns and the one the retry will look in.
fn restart_marker(unit_dir: &Path, unit: &str) -> PathBuf {
    unit_dir.join(format!(".{unit}.restart-pending"))
}

/// Record that `unit` is running bytes that are about to be replaced.
fn owe_a_restart(unit_dir: &Path, unit: &str) -> anyhow::Result<()> {
    let path = restart_marker(unit_dir, unit);
    std::fs::write(&path, b"")
        .with_context(|| format!("recording that {unit} needs a restart ({})", path.display()))
}

/// Clear one obligation, after the restart that discharged it — or when
/// the unit it belonged to has been removed.
fn settle_the_restart(unit_dir: &Path, unit: &str) -> anyhow::Result<()> {
    let path = restart_marker(unit_dir, unit);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("clearing the restart marker {}", path.display()))
        }
    }
}

/// The text of a unit file setup is allowed to touch, or `None` when there
/// is no such file.
///
/// Two failures, told apart because they call for different things from
/// the operator:
///
/// - The file is not setup's — a directory, a symlink, a socket, or a
///   regular file whose first line is not the marker. That is an ownership
///   refusal, and its message says to move the file aside.
/// - The file could not be READ — permissions, an I/O error, contents that
///   are not UTF-8. Those get the underlying error and the path, because
///   asserting ownership (and recommending deletion) over a file nobody
///   managed to open would be a conclusion this code has not earned.
fn existing_managed_text(path: &Path) -> anyhow::Result<Option<String>> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("inspecting {}", path.display()));
        }
    };
    let foreign = || {
        anyhow::anyhow!(
            "{} exists and was not written by farhelm helm setup; move it aside or delete it, \
             then rerun",
            path.display()
        )
    };
    if !metadata.is_file() {
        return Err(foreign());
    }
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading the existing unit file {}", path.display()))?;
    if is_managed(&text) {
        Ok(Some(text))
    } else {
        Err(foreign())
    }
}

/// Publish one unit file through a same-directory temporary file and a
/// rename, so a reader (systemd's own daemon-reload, or a concurrent
/// setup) never sees a half-written unit.
///
/// The staging file is created EXCLUSIVELY under a unique name. A
/// predictable one was a real hazard rather than a tidiness issue: two
/// setups racing would write the same inode while one renamed it, and a
/// symlink planted at that name would redirect the truncating open
/// somewhere else entirely.
///
/// The mode is set explicitly on the finished file rather than left to the
/// creation mode, which the process umask filters down — a unit published
/// 0600 under a restrictive umask is readable by this user's manager, but
/// the function would be claiming something it had not done.
fn write_unit(path: &Path, text: &str) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let directory = path
        .parent()
        .context("a unit path always has a parent directory")?;
    let name = path
        .file_name()
        .context("a unit path always has a file name")?;
    let mut staged = tempfile::Builder::new()
        .prefix(&format!(".{}.", name.to_string_lossy()))
        .tempfile_in(directory)
        .with_context(|| format!("staging a replacement for {}", path.display()))?;
    staged
        .write_all(text.as_bytes())
        .with_context(|| format!("writing a replacement for {}", path.display()))?;
    std::fs::set_permissions(staged.path(), std::fs::Permissions::from_mode(0o644))
        .with_context(|| format!("setting the mode of a replacement for {}", path.display()))?;
    staged
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("publishing {}", path.display()))?;
    Ok(())
}

/// Refuse unless the running systemd user manager reads units from the
/// directory setup is about to write to.
///
/// Setup picks that directory from the `XDG_CONFIG_HOME` of the process
/// that INVOKED it; the manager picked its own search path from the
/// environment it was started with, years of uptime ago as far as this
/// command knows. A one-shot `XDG_CONFIG_HOME=/srv/config farhelm helm
/// setup` therefore publishes two perfectly good unit files somewhere the
/// manager will never look, and then asks it to enable them by name. The
/// enable fails — after the files are on disk — and nothing about the
/// error says why.
///
/// `--uninstall` is guarded for the mirror-image reason: it would find
/// nothing in the wrong directory, report both units absent, and leave the
/// installed ones running. That failure is quieter than the install one
/// and therefore worse — it looks like success.
///
/// So the manager is asked outright. `show-environment` prints the
/// manager's own environment block, which is the only authority on this;
/// an absent `XDG_CONFIG_HOME` there means `$HOME/.config`, the same rule
/// [`user_unit_dir`] applies everywhere else.
///
/// `--dry-run` runs this command too. It is the one query setup issues
/// that changes nothing at all, and its answer is the whole point of the
/// check: a preview that stayed silent about a mismatch would be
/// answering a different question than the one the operator asked.
fn require_the_managers_unit_directory(
    opts: &SetupOptions,
    units: &mut dyn UnitManager,
    report: &mut Report,
    unit_dir: &Path,
) -> anyhow::Result<()> {
    let result = run_command(units, report, &["show-environment"])?;
    if result.status != 0 {
        let detail = command_detail(&result);
        bail!(
            "the systemd user manager did not answer: systemctl --user show-environment exited \
             {}{}{detail}. Setup writes units for a manager that is running; start one (or log in \
             on a system with a user manager) and rerun",
            result.status,
            if detail.is_empty() { "" } else { ": " }
        );
    }
    let manager_dir = manager_unit_dir(&result.stdout)?;
    if manager_dir == unit_dir {
        return Ok(());
    }
    let refusal = format!(
        "the running systemd user manager loads units from {}, but this environment selects {}; \
         run setup with the same XDG_CONFIG_HOME the manager was started with (or unset it), then \
         rerun",
        manager_dir.display(),
        unit_dir.display()
    );
    if opts.dry_run {
        line!(report, "note: {refusal}");
        return Ok(());
    }
    bail!(refusal)
}

/// The unit directory implied by a `systemctl --user show-environment`
/// block.
///
/// Derived from the MANAGER's environment and nothing else. That is the
/// whole point of asking: the caller's `HOME` is not evidence about the
/// manager, and using it as a fallback made `HOME=/srv/other farhelm helm
/// setup` compute the same directory on both sides of a comparison whose
/// entire job is to notice that they differ. An absolute
/// `XDG_CONFIG_HOME` answers outright; otherwise the manager's own `HOME`
/// does, exactly as [`user_unit_dir`] resolves it everywhere else.
///
/// A manager that reports neither is a FAILURE, not a default. Guessing
/// there would produce a confident wrong answer about the one thing this
/// function exists to establish.
///
/// Only the first assignment of each name is read; the block is one
/// `NAME=VALUE` per line. Systemd quotes a value that needs it, so a
/// surrounding pair of quotes is stripped — anything more exotic (an
/// embedded escape) is left alone, which makes the value fail the
/// absolute-path test and falls through to the next rule. That is the
/// safe direction: a mismatch refusal rather than a wrong directory.
fn manager_unit_dir(show_environment: &str) -> anyhow::Result<PathBuf> {
    let xdg = manager_assignment(show_environment, "XDG_CONFIG_HOME");
    let home =
        manager_assignment(show_environment, "HOME").filter(|value| Path::new(value).is_absolute());
    user_unit_dir_for(
        xdg.as_deref().map(OsStr::new),
        home.as_deref().map(Path::new),
    )
    .map_err(|_| {
        anyhow::anyhow!(
            "the systemd user manager reported neither XDG_CONFIG_HOME nor HOME, so setup cannot \
             tell which directory it loads units from; start the manager from a session with a \
             usable environment, then rerun"
        )
    })
}

/// One `NAME=VALUE` assignment out of a `show-environment` block, unquoted
/// and with empty values treated as absent.
fn manager_assignment(show_environment: &str, name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    let value = show_environment
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))?
        .trim();
    let unquoted = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value);
    (!unquoted.is_empty()).then(|| unquoted.to_string())
}

/// Ask systemd whether one unit is running, before it is replaced.
///
/// Only systemd's documented "this unit is not running" answers count as
/// false. Every other non-zero status is an operational failure — no user
/// manager to talk to, a permission problem, a signal — and swallowing
/// those as "inactive" is how a unit that WAS running gets its file
/// replaced and then never restarted, leaving the old process on the old
/// configuration with setup reporting success.
fn unit_is_active(
    units: &mut dyn UnitManager,
    report: &mut Report,
    unit: &str,
) -> anyhow::Result<bool> {
    /// `systemctl is-active` answers 3 for a unit the manager knows and
    /// is not running.
    const NOT_ACTIVE: i32 = 3;
    /// ...and 4 for one it has never heard of, which is the ORDINARY
    /// state on a first install: the file does not exist yet, so the
    /// manager has nothing to report. Reading that as an error stopped
    /// setup before it published its first unit — the exact case the
    /// command exists for. Both mean the same thing here ("it was not
    /// running, so there is nothing to restart"), and the file-absence
    /// preflight above has already established which of the two it is.
    const NO_SUCH_UNIT: i32 = 4;

    let result = run_command(units, report, &["is-active", unit])?;
    match result.status {
        0 => Ok(true),
        NOT_ACTIVE | NO_SUCH_UNIT => Ok(false),
        status => {
            let detail = command_detail(&result);
            bail!(
                "systemctl --user is-active {unit} exited {status}{}{detail}, so setup cannot \
                 tell whether replacing that unit would need a restart",
                if detail.is_empty() { "" } else { ": " }
            )
        }
    }
}

/// Whatever a failed `systemctl` said, from whichever stream it used.
fn command_detail(result: &UnitCommand) -> &str {
    match result.stderr.trim() {
        "" => result.stdout.trim(),
        stderr => stderr,
    }
}

/// Record one command the dry run would have issued.
fn would_run(report: &mut Report, args: &[&str]) {
    line!(report, "would run: systemctl --user {}", args.join(" "));
}

/// Run one `systemctl` command and echo it, without judging its exit
/// status. For the commands whose failure matters, use
/// [`require_command`].
fn run_command(
    units: &mut dyn UnitManager,
    report: &mut Report,
    args: &[&str],
) -> anyhow::Result<UnitCommand> {
    let result = units.run(args)?;
    line!(report, "ran: systemctl --user {}", args.join(" "));
    Ok(result)
}

/// Run one `systemctl` command that must succeed, or print what it would
/// have been under `--dry-run`.
///
/// Failure stops the run rather than pressing on: the commands are
/// ordered, and there is nothing useful to do after a `daemon-reload` or
/// an `enable` that systemd refused.
fn require_command(
    units: &mut dyn UnitManager,
    opts: &SetupOptions,
    report: &mut Report,
    args: &[&str],
) -> anyhow::Result<()> {
    if opts.dry_run {
        would_run(report, args);
        return Ok(());
    }
    let result = run_command(units, report, args)?;
    if result.status != 0 {
        // systemctl splits its diagnostics between the two streams
        // depending on the subcommand, so a failure that said nothing on
        // stderr still has something to show.
        let detail = command_detail(&result);
        bail!(
            "systemctl --user {} exited {}{}{}",
            args.join(" "),
            result.status,
            if detail.is_empty() { "" } else { ": " },
            detail
        );
    }
    Ok(())
}

/// Whether this executable is one nobody should point a systemd unit at.
///
/// Two shapes, both of them things an operator does by accident while
/// trying setup out: a `cargo build` artifact under some `target/`
/// directory, and a binary that was extracted or copied into the
/// temporary directory. A unit pointing at either one keeps working right
/// up until the directory is cleaned, and then fails at boot with nothing
/// to explain it.
///
/// `temp_dir` is canonicalized alongside the executable because `/tmp` is
/// a symlink on some systems, and a prefix comparison between a resolved
/// path and an unresolved one silently never matches.
fn looks_like_a_build_tree(exe: &Path, temp_dir: &Path) -> bool {
    if exe
        .components()
        .any(|component| component.as_os_str() == OsStr::new("target"))
    {
        return true;
    }
    // A temporary directory that cannot be resolved is compared as given;
    // it is the best available spelling of the same place.
    let resolved = std::fs::canonicalize(temp_dir).ok();
    exe.starts_with(resolved.as_deref().unwrap_or(temp_dir))
}

/// Which tmux the supervisor unit should pin, or why none of the
/// candidates will do.
enum TmuxDecision {
    Accepted {
        program: PathBuf,
        /// Printed as a note. Present only for a tmux newer than the
        /// version this project is tested against.
        warning: Option<String>,
    },
    Refused(String),
}

/// Pick a tmux the way the supervisor does — `--tmux`, then
/// `FARHELM_TMUX`, then `PATH` — and hold it to the same floor.
///
/// Setup NEVER installs tmux. That is a deliberate boundary: this command
/// manages systemd units on a machine the operator administers, and a CLI
/// that quietly fetched a terminal multiplexer would be doing something
/// the operator did not ask for. Remote provisioning is the place that
/// installs a tmux, and only onto hosts it also owns the layout of.
fn choose_tmux(ctx: &SetupContext, opts: &SetupOptions) -> TmuxDecision {
    // An explicitly named tmux is the operator's choice, so its failure is
    // the answer — searching on past it would silently pin something they
    // did not ask for.
    if let Some(named) = opts.tmux.as_deref().or_else(|| {
        ctx.tmux_env
            .as_ref()
            .filter(|value| !value.is_empty())
            .map(Path::new)
    }) {
        return decide_tmux(ctx.absolute(named));
    }
    // PATH is a list, and `execvp` treats it as one: an entry that looks
    // executable but cannot be spawned — a `noexec` mount, an execute bit
    // for a group this process is not in — is skipped rather than fatal.
    // Stopping at the first such shadow used to refuse setup outright on a
    // machine with a perfectly good tmux one entry further along.
    let mut first_failure = None;
    for candidate in candidates_on_path(&ctx.path, "tmux") {
        let candidate = ctx.absolute(&candidate);
        match probe_tmux(&candidate) {
            Err(error @ TmuxProbeError::NotRunnable(_)) => {
                first_failure.get_or_insert_with(|| refuse_tmux(&found_phrase(&candidate, &error)));
            }
            probed => return finish_tmux(candidate, probed),
        }
    }
    // Nothing on PATH ran. The first candidate's failure is the actionable
    // one — it is what `execvp` would have complained about, and what the
    // operator sees at the front of their PATH.
    first_failure.unwrap_or_else(|| refuse_tmux("none"))
}

/// Probe one named candidate and turn the outcome into a decision.
fn decide_tmux(candidate: PathBuf) -> TmuxDecision {
    let probed = probe_tmux(&candidate);
    finish_tmux(candidate, probed)
}

fn finish_tmux(
    candidate: PathBuf,
    probed: Result<farhelm_supervisor::tmux::TmuxProbe, TmuxProbeError>,
) -> TmuxDecision {
    let probe = match probed {
        Ok(probe) => probe,
        Err(error) => return refuse_tmux(&found_phrase(&candidate, &error)),
    };
    // The probe echoes back the program it actually spawned; using that
    // rather than the caller's spelling keeps the message true even when
    // the two could differ.
    let named = probe.program.display();
    if probe.support == TmuxSupport::BelowFloor {
        return refuse_tmux(&format!("tmux {} at {named}", probe.version));
    }
    TmuxDecision::Accepted {
        // Above the pin is accepted, not refused: Homebrew ships versions
        // nobody has audited Farhelm against long before the floor moves,
        // and refusing them would strand people on the release this
        // project itself recommends installing. The note is what makes a
        // later bug report start from the right place.
        warning: (probe.support == TmuxSupport::AbovePin).then(|| {
            format!(
                "the tmux {} at {named} is newer than the version Farhelm is tested against \
                 ({TMUX_FLOOR}); this combination is unaudited",
                probe.version
            )
        }),
        program: probe.program,
    }
}

/// How one rejected candidate is named in the refusal, with the detail the
/// probe collected — the errno, or what the program actually printed.
fn found_phrase(candidate: &Path, error: &TmuxProbeError) -> String {
    let named = candidate.display();
    match error {
        TmuxProbeError::Unparseable(_) => format!("an unparseable tmux at {named} ({error})"),
        // A candidate that hung or flooded is reported the same way as one
        // that would not start: from the operator's side it is the same
        // problem, and the detail says which.
        _ => format!("a tmux at {named} that could not be run ({error})"),
    }
}

fn refuse_tmux(found: &str) -> TmuxDecision {
    TmuxDecision::Refused(format!(
        "farhelm needs tmux {TMUX_FLOOR} or newer on this machine and found {found}. Install one \
         (Linuxbrew: brew install tmux; or a distro package at or above {TMUX_FLOOR}) or pass \
         --tmux /path/to/tmux, then rerun setup. Setup never installs tmux."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    // The grammar tests drive the real binary's parser, which is the only
    // way to prove `farhelm helm setup` is reachable under these names.
    use clap::Parser as _;
    use std::collections::HashSet;

    /// A stand-in user manager: records every `systemctl --user`
    /// invocation and answers the two queries setup's decisions depend on.
    ///
    /// `is-active` follows systemd's own exit codes, and the DEFAULT
    /// matters: a unit this manager has never heard of answers 4, not 3.
    /// That is the state of both units on a clean first install, and a
    /// fake that answered 3 for everything hid a production refusal that
    /// stopped setup before it wrote its first file.
    ///
    /// `show-environment` answers with an empty block by default, which
    /// means the manager reads units from `$HOME/.config/systemd/user` —
    /// what a fixture context selects too, so the ordinary tests match.
    ///
    /// `scripted` overrides one exact command outright. The interesting
    /// failures are not "inactive": a manager that cannot be reached, a
    /// permission problem, or a signal all arrive as some OTHER non-zero
    /// status, and setup must tell those apart from a stopped unit.
    #[derive(Default)]
    struct FakeUnits {
        commands: Vec<String>,
        /// Units the manager reports as running (exit 0).
        active: HashSet<String>,
        /// Units it has loaded but is not running (exit 3). `active`
        /// implies known; anything in neither set is exit 4.
        known: HashSet<String>,
        /// command → (status, stdout, stderr).
        scripted: std::collections::HashMap<String, (i32, String, String)>,
    }

    impl FakeUnits {
        fn active(mut self, units: &[&str]) -> Self {
            for unit in units {
                self.active.insert((*unit).to_string());
                self.known.insert((*unit).to_string());
            }
            self
        }

        /// Units the manager has loaded but is not running.
        fn loaded(mut self, units: &[&str]) -> Self {
            for unit in units {
                self.known.insert((*unit).to_string());
            }
            self
        }

        /// What this manager reports for its own environment block.
        fn reporting(self, environment: &str) -> Self {
            self.script("show-environment", 0, environment, "")
        }

        /// No manager at all: `show-environment` fails the way systemctl
        /// does when it cannot reach one.
        fn without_a_manager(self, stderr: &str) -> Self {
            self.script("show-environment", 1, "", stderr)
        }

        fn script(mut self, command: &str, status: i32, stdout: &str, stderr: &str) -> Self {
            self.scripted.insert(
                command.to_string(),
                (status, stdout.to_string(), stderr.to_string()),
            );
            self
        }
    }

    impl UnitManager for FakeUnits {
        fn run(&mut self, args: &[&str]) -> anyhow::Result<UnitCommand> {
            let command = args.join(" ");
            self.commands.push(command.clone());
            if let Some((status, stdout, stderr)) = self.scripted.get(&command) {
                return Ok(UnitCommand {
                    status: *status,
                    stdout: stdout.clone(),
                    stderr: stderr.clone(),
                });
            }
            let status = match args {
                // systemd's own exit codes: 0 running, 3 loaded but not
                // running, 4 never heard of it.
                ["is-active", unit] if self.active.contains(*unit) => 0,
                ["is-active", unit] if self.known.contains(*unit) => 3,
                ["is-active", _] => 4,
                _ => 0,
            };
            Ok(UnitCommand {
                status,
                stdout: String::new(),
                stderr: String::new(),
            })
        }
    }

    /// A fixture machine: a home directory, a unit directory, a fake
    /// installed farhelm, and a `PATH` holding whatever tmux the test
    /// wants found.
    struct Fixture {
        root: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let fixture = Self {
                root: tempfile::tempdir().expect("fixture root"),
            };
            std::fs::create_dir_all(fixture.root.path().join("home")).unwrap();
            std::fs::create_dir_all(fixture.root.path().join("bin")).unwrap();
            std::fs::write(fixture.root.path().join("bin/farhelm"), b"#!/bin/sh\n").unwrap();
            fixture
        }

        fn home(&self) -> PathBuf {
            self.root.path().join("home")
        }

        fn unit_dir(&self) -> PathBuf {
            self.home().join(".config/systemd/user")
        }

        /// The path a `PATH` search should find, holding a script that
        /// prints `version` for `-V`.
        fn tmux_dir(&self, version: &str) -> PathBuf {
            let dir = self.root.path().join("tmuxbin");
            std::fs::create_dir_all(&dir).unwrap();
            write_script(&dir.join("tmux"), &format!("printf '{version}\\n'"));
            dir
        }

        /// The directory a fixture run pretends to have been started in.
        /// Injected rather than taken from the test process, which must
        /// never change its own working directory.
        fn cwd(&self) -> PathBuf {
            self.root.path().join("cwd")
        }

        /// A user manager whose environment matches this fixture's:
        /// same `HOME`, no `XDG_CONFIG_HOME`, so it reads the same unit
        /// directory a fixture context selects.
        ///
        /// Every test needs one, because setup refuses outright when the
        /// manager reports neither variable — that is a machine whose unit
        /// directory cannot be located, not a default.
        fn manager(&self) -> FakeUnits {
            FakeUnits::default().reporting(&format!("HOME={}\n", self.home().display()))
        }

        /// A context whose `PATH` is exactly `dirs`, so no tmux from the
        /// machine running the tests can leak into a decision.
        fn context(&self, dirs: &[PathBuf]) -> SetupContext {
            SetupContext {
                exe: self.root.path().join("bin/farhelm"),
                home: self.home(),
                cwd: self.cwd(),
                path: std::env::join_paths(dirs).unwrap(),
                xdg_config_home: None,
                xdg_state_home: None,
                tmux_env: None,
                // Deliberately NOT the fixture root: the fixture lives
                // under the real temporary directory, and pointing the
                // build-tree check at that would make every test refuse.
                temp_dir: self.root.path().join("no-such-temp"),
            }
        }
    }

    fn write_script(path: &Path, body: &str) {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o755)
            .open(path)
            .unwrap();
        file.write_all(format!("#!/bin/sh\n{body}\n").as_bytes())
            .unwrap();
        drop(file);
        wait_until_executable(path);
    }

    /// Run the freshly written script once, retrying `ETXTBSY`, so the
    /// test that follows cannot fail on it.
    ///
    /// This is a real race and it bit a full-suite run: writing an
    /// executable and immediately exec'ing it fails with `ETXTBSY` while
    /// ANY process holds the file open for writing, and in a
    /// multi-threaded test binary another thread's `fork` can duplicate
    /// this file's descriptor into a child that has not reached its own
    /// `exec` yet. Close-on-exec bounds that window to microseconds but
    /// does not remove it. Once this succeeds, no descriptor to the file
    /// is open for writing anywhere and every later spawn is safe.
    fn wait_until_executable(path: &Path) {
        const ETXTBSY: i32 = 26;
        for _ in 0..200 {
            match std::process::Command::new(path).arg("-V").output() {
                Ok(_) => return,
                Err(error) if error.raw_os_error() == Some(ETXTBSY) => {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(error) => panic!("fixture script {} is not runnable: {error}", path.display()),
            }
        }
        panic!("fixture script {} stayed busy", path.display());
    }

    fn run(ctx: &SetupContext, opts: &SetupOptions, units: &mut FakeUnits) -> (String, String) {
        let mut out = Vec::new();
        match run_setup(ctx, opts, units, &mut out) {
            Ok(()) => (String::from_utf8(out).unwrap(), String::new()),
            Err(error) => (String::from_utf8(out).unwrap(), format!("{error:#}")),
        }
    }

    /// The whole point of the command: a first run writes both units,
    /// reloads, enables, and starts them. The command sequence is asserted
    /// exactly because its ORDER is the contract — reload before enable,
    /// and nothing restarted when nothing was running.
    ///
    /// This is also the regression for the state a clean machine is
    /// ACTUALLY in: the manager has never heard of either unit, so
    /// `is-active` answers 4 rather than 3. Treating that as an error
    /// stopped setup before it published its first file — a command that
    /// worked on every machine except the ones it was written for.
    #[farhelm_testtrace::test]
    fn a_first_install_writes_both_units_reloads_and_enables() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        let mut units = fixture.manager();
        assert_eq!(
            units
                .run(&["is-active", "farhelm-helm.service"])
                .unwrap()
                .status,
            4,
            "a unit the manager has never seen answers 4, which is what this run must survive"
        );
        units.commands.clear();
        let (output, error) = run(&ctx, &SetupOptions::default(), &mut units);
        assert!(error.is_empty(), "{error}");
        assert_eq!(
            units.commands,
            [
                "show-environment",
                "is-active farhelm-supervisor.service",
                "is-active farhelm-helm.service",
                "daemon-reload",
                "enable --now farhelm-supervisor.service",
                "enable --now farhelm-helm.service",
            ]
        );
        let supervisor =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-supervisor.service")).unwrap();
        assert!(is_managed(&supervisor), "{supervisor}");
        assert!(!supervisor.contains('@'), "{supervisor}");
        assert!(supervisor.contains("FARHELM_TMUX="));
        let helm =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-helm.service")).unwrap();
        assert!(is_managed(&helm), "{helm}");
        // `enable --now` is what starts both services; the units name the
        // same resolved state directory so they find each other's socket.
        let state_dir = fixture.home().join(".local/state/farhelm");
        assert!(
            helm.contains(&format!(
                " helm run --state-dir \"{}\"\n",
                state_dir.display()
            )),
            "{helm}"
        );
        assert!(
            supervisor.contains(&format!("--state-dir \"{}\"", state_dir.display())),
            "{supervisor}"
        );
        assert!(output.contains("written "));
        assert!(
            output
                .contains("loginctl enable-linger \"$USER\"   # start at boot and survive logout")
        );
        assert!(output.contains("farhelm helm token show   # the browser sign-in token"));
    }

    /// Rerunning setup unchanged writes nothing and restarts nothing — a
    /// helm that is serving right now must not be bounced by a no-op run.
    ///
    /// It DOES still reload and enable, and that is what makes setup
    /// convergent rather than merely idempotent: if an earlier run
    /// published a unit and then failed at `daemon-reload`, the bytes are
    /// already correct, so a reload conditional on "this run wrote
    /// something" would skip forever the step its predecessor never
    /// finished. Both are cheap and systemd ignores them when there is
    /// nothing to do.
    #[farhelm_testtrace::test]
    fn an_unchanged_rerun_writes_nothing_but_still_reloads_and_enables() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        let mut units = fixture.manager().active(&["farhelm-helm.service"]);
        let (output, error) = run(&ctx, &SetupOptions::default(), &mut units);
        assert!(error.is_empty(), "{error}");
        assert_eq!(
            units.commands,
            [
                "show-environment",
                "daemon-reload",
                "enable --now farhelm-supervisor.service",
                "enable --now farhelm-helm.service",
            ]
        );
        assert!(output.contains("unchanged "), "{output}");
    }

    /// A unit whose bytes changed while it was RUNNING has to be
    /// restarted, or the process keeps running the old configuration
    /// until something else stops it. A unit that was not running must
    /// not be restarted — `enable --now` already started it, and a
    /// restart on top of that would be a second, pointless bounce.
    #[farhelm_testtrace::test]
    fn only_a_changed_unit_that_was_active_is_restarted() {
        for active in [true, false] {
            let fixture = Fixture::new();
            let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
            run(&ctx, &SetupOptions::default(), &mut fixture.manager());
            let mut units = if active {
                fixture.manager().active(&["farhelm-helm.service"])
            } else {
                fixture.manager()
            };
            let changed = SetupOptions {
                port: Some(7999),
                ..SetupOptions::default()
            };
            let (_, error) = run(&ctx, &changed, &mut units);
            assert!(error.is_empty(), "{error}");
            let mut expected = vec![
                "show-environment".to_string(),
                "is-active farhelm-helm.service".to_string(),
                "daemon-reload".to_string(),
                "enable --now farhelm-supervisor.service".to_string(),
                "enable --now farhelm-helm.service".to_string(),
            ];
            if active {
                expected.push("restart farhelm-helm.service".to_string());
            }
            assert_eq!(units.commands, expected, "active={active}");
        }
    }

    /// `--no-supervisor` is for a machine whose supervisor is somebody
    /// else's job. It must not render, probe tmux for, enable, or so much
    /// as READ a supervisor unit — and with no tmux on PATH at all it must
    /// still succeed, which is the property that proves the tmux check was
    /// skipped rather than merely tolerated.
    ///
    /// The case that matters is the flag's actual use: a supervisor unit
    /// somebody else already wrote is sitting there. Ordinary setup would
    /// refuse to touch it; `--no-supervisor` must leave it byte-for-byte
    /// alone AND still install the helm, rather than tripping over an
    /// ownership check on a file it was told not to manage.
    #[farhelm_testtrace::test]
    fn no_supervisor_leaves_a_foreign_supervisor_unit_untouched() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[]);
        std::fs::create_dir_all(fixture.unit_dir()).unwrap();
        let foreign = fixture.unit_dir().join("farhelm-supervisor.service");
        let bytes = b"[Unit]\nDescription=not farhelm's\n";
        std::fs::write(&foreign, bytes).unwrap();

        let mut units = fixture.manager();
        let opts = SetupOptions {
            no_supervisor: true,
            ..SetupOptions::default()
        };
        let (output, error) = run(&ctx, &opts, &mut units);
        assert!(error.is_empty(), "{error}");
        assert_eq!(
            units.commands,
            [
                "show-environment",
                "is-active farhelm-helm.service",
                "daemon-reload",
                "enable --now farhelm-helm.service",
            ]
        );
        assert_eq!(std::fs::read(&foreign).unwrap(), bytes);
        assert!(!output.contains("farhelm-supervisor.service"), "{output}");
        assert!(
            fixture.unit_dir().join("farhelm-helm.service").is_file(),
            "the helm unit is still setup's to manage"
        );
    }

    /// The pinned flags are what make the units reproducible: one
    /// resolved state directory in BOTH units, and `--port` in the helm
    /// unit when it was given.
    #[farhelm_testtrace::test]
    fn pinned_flags_reach_the_units_they_belong_to() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        let opts = SetupOptions {
            state_dir: Some(PathBuf::from("/srv/state")),
            port: Some(9000),
            ..SetupOptions::default()
        };
        let (_, error) = run(&ctx, &opts, &mut fixture.manager());
        assert!(error.is_empty(), "{error}");
        let helm =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-helm.service")).unwrap();
        assert!(
            helm.contains(" helm run --state-dir \"/srv/state\" --port 9000\n"),
            "{helm}"
        );
        let supervisor =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-supervisor.service")).unwrap();
        assert!(
            supervisor.contains("--state-dir \"/srv/state\""),
            "{supervisor}"
        );
    }

    /// A unit pointing into a build tree works until `cargo clean` or a
    /// reboot clears the directory, and then fails at boot with nothing
    /// to explain it. `--dry-run` still renders, because seeing the unit
    /// is exactly what a developer is trying the flag for.
    #[farhelm_testtrace::test]
    fn a_binary_in_a_build_tree_is_refused_but_still_renders_under_dry_run() {
        let fixture = Fixture::new();
        let build_tree = fixture.root.path().join("target/debug");
        std::fs::create_dir_all(&build_tree).unwrap();
        std::fs::write(build_tree.join("farhelm"), b"#!/bin/sh\n").unwrap();
        let mut ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        ctx.exe = build_tree.join("farhelm");

        let (_, error) = run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        assert!(
            error.contains("that looks like a build tree. Install farhelm first (see README, \"Install\") and run setup from the installed binary."),
            "{error}"
        );
        assert!(!fixture.unit_dir().exists());

        let mut units = fixture.manager();
        let dry = SetupOptions {
            dry_run: true,
            ..SetupOptions::default()
        };
        let (output, error) = run(&ctx, &dry, &mut units);
        assert!(error.is_empty(), "{error}");
        assert!(
            output.contains("looks like a build tree; a real setup would refuse"),
            "{output}"
        );
        assert!(output.contains("KillMode=process"), "{output}");
        // The one read-only query is all a dry run issues.
        assert_eq!(units.commands, ["show-environment"]);
    }

    /// The same refusal covers a binary run out of the temporary
    /// directory (an extracted archive nobody installed yet). `/tmp` is a
    /// symlink on some systems, so the check has to compare resolved
    /// paths.
    #[farhelm_testtrace::test]
    fn a_binary_under_the_temporary_directory_is_refused() {
        let fixture = Fixture::new();
        let mut ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        ctx.temp_dir = fixture.root.path().to_path_buf();
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        assert!(error.contains("looks like a build tree"), "{error}");
    }

    /// Every way a tmux can fail to qualify has its own clause in the
    /// refusal, because "install a newer tmux" and "the binary you named
    /// does not run" need different fixes. The text is asserted whole:
    /// it is the entire guidance a stuck operator gets.
    ///
    /// Each clause carries the probe's own detail — the errno, or what the
    /// candidate actually printed — because "could not be run" without it
    /// leaves the operator guessing between a typo, a permission problem,
    /// and a missing interpreter.
    #[farhelm_testtrace::test]
    fn each_unusable_tmux_is_named_in_the_refusal() {
        let fixture = Fixture::new();
        let empty = fixture.root.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();

        let missing = fixture.root.path().join("nowhere/tmux");
        let old = fixture.tmux_dir("tmux 3.6").join("tmux");
        let fixture_garbage = Fixture::new();
        let garbage = fixture_garbage.tmux_dir("not-tmux 3.7c").join("tmux");
        for (tmux, expected) in [
            (None, "none".to_string()),
            (
                Some(missing.clone()),
                format!(
                    "a tmux at {} that could not be run (No such file or directory (os error 2))",
                    missing.display()
                ),
            ),
            (Some(old.clone()), format!("tmux 3.6 at {}", old.display())),
            (
                Some(garbage.clone()),
                format!(
                    "an unparseable tmux at {} (it printed \"not-tmux 3.7c\")",
                    garbage.display()
                ),
            ),
        ] {
            let ctx = fixture.context(std::slice::from_ref(&empty));
            let opts = SetupOptions {
                tmux,
                ..SetupOptions::default()
            };
            let (_, error) = run(&ctx, &opts, &mut fixture.manager());
            assert_eq!(
                error,
                format!(
                    "farhelm needs tmux 3.7c or newer on this machine and found {expected}. \
                     Install one (Linuxbrew: brew install tmux; or a distro package at or above \
                     3.7c) or pass --tmux /path/to/tmux, then rerun setup. Setup never installs \
                     tmux."
                )
            );
        }
    }

    /// A refused tmux must not stop `--dry-run` from showing the unit:
    /// the stand-in names the one field the operator still has to supply.
    #[farhelm_testtrace::test]
    fn a_dry_run_renders_a_placeholder_for_a_tmux_it_would_refuse() {
        let fixture = Fixture::new();
        let empty = fixture.root.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let ctx = fixture.context(&[empty]);
        let opts = SetupOptions {
            dry_run: true,
            ..SetupOptions::default()
        };
        let (output, error) = run(&ctx, &opts, &mut fixture.manager());
        assert!(error.is_empty(), "{error}");
        assert!(output.contains("note: farhelm needs tmux"), "{output}");
        assert!(
            output.contains("Environment=\"FARHELM_TMUX=<tmux>\""),
            "{output}"
        );
        assert!(!fixture.unit_dir().exists());
    }

    /// A tmux above the pin is accepted — refusing it would strand people
    /// on Homebrew's current release — but the operator is told, so a
    /// later bug report starts from the right place.
    #[farhelm_testtrace::test]
    fn a_tmux_newer_than_the_pin_is_accepted_with_a_warning() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 9.9")]);
        let (output, error) = run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        assert!(error.is_empty(), "{error}");
        assert!(
            output.contains("is newer than the version Farhelm is tested against (3.7c); this combination is unaudited"),
            "{output}"
        );
    }

    /// The ownership rule, in the direction that matters: setup must
    /// never overwrite a unit somebody else wrote. Checked before any
    /// write, so a refusal on the second unit does not leave the first
    /// one replaced.
    #[farhelm_testtrace::test]
    fn an_unmarked_unit_file_is_refused_rather_than_overwritten() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        std::fs::create_dir_all(fixture.unit_dir()).unwrap();
        let path = fixture.unit_dir().join("farhelm-supervisor.service");
        std::fs::write(&path, b"[Unit]\nDescription=mine\n").unwrap();
        let mut units = fixture.manager();
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut units);
        assert_eq!(
            error,
            format!(
                "{} exists and was not written by farhelm helm setup; move it aside or delete it, \
                 then rerun",
                path.display()
            )
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "[Unit]\nDescription=mine\n"
        );
        assert!(!fixture.unit_dir().join("farhelm-helm.service").exists());
        assert_eq!(units.commands, ["show-environment"]);
    }

    /// Uninstall is the other half of ownership: it disables and removes
    /// exactly what setup wrote, reports what was already gone, and
    /// leaves the operator's own drop-in directory alone while saying so
    /// — a stale override would otherwise apply silently to whatever a
    /// later setup writes.
    #[farhelm_testtrace::test]
    fn uninstall_removes_only_marked_units_and_reports_drop_ins() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        // BOTH units get a drop-in: a regression that hard-coded the
        // notice or the preservation to one of them would otherwise pass.
        let drop_ins: Vec<_> = ["farhelm-supervisor.service", "farhelm-helm.service"]
            .into_iter()
            .map(|unit| {
                let directory = fixture.unit_dir().join(format!("{unit}.d"));
                std::fs::create_dir_all(&directory).unwrap();
                let override_file = directory.join(format!("{unit}-override.conf"));
                std::fs::write(&override_file, b"[Service]\n").unwrap();
                (unit, override_file)
            })
            .collect();

        let mut units = fixture.manager();
        let opts = SetupOptions {
            uninstall: true,
            ..SetupOptions::default()
        };
        let (output, error) = run(&ctx, &opts, &mut units);
        assert!(error.is_empty(), "{error}");
        assert_eq!(
            units.commands,
            [
                "show-environment",
                "disable --now farhelm-supervisor.service",
                "disable --now farhelm-helm.service",
                "daemon-reload",
            ]
        );
        assert!(!fixture.unit_dir().join("farhelm-helm.service").exists());
        for (unit, override_file) in &drop_ins {
            assert!(
                output.contains(&format!(
                    "drop-ins under {}/{unit}.d/ were left in place",
                    fixture.unit_dir().display()
                )),
                "{output}"
            );
            assert!(override_file.exists(), "{}", override_file.display());
        }

        // A second uninstall has nothing to delete, and still reloads:
        // "both files are absent" is also the state a run that deleted
        // them and then failed at the reload leaves behind, and the two
        // are indistinguishable from here. It asks the manager where
        // units live first, because "absent" is only meaningful for the
        // directory the manager actually reads.
        let mut units = fixture.manager();
        let (output, error) = run(&ctx, &opts, &mut units);
        assert!(error.is_empty(), "{error}");
        assert_eq!(units.commands, ["show-environment", "daemon-reload"]);
        assert!(output.contains("absent "), "{output}");
    }

    /// Uninstall refuses a hand-written unit for the same reason install
    /// does, and the refusal has to come before anything is disabled.
    #[farhelm_testtrace::test]
    fn uninstall_refuses_a_unit_it_does_not_own() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[]);
        std::fs::create_dir_all(fixture.unit_dir()).unwrap();
        let path = fixture.unit_dir().join("farhelm-supervisor.service");
        std::fs::write(&path, b"[Unit]\n").unwrap();
        let mut units = fixture.manager();
        let opts = SetupOptions {
            uninstall: true,
            ..SetupOptions::default()
        };
        let (_, error) = run(&ctx, &opts, &mut units);
        assert!(
            error.contains("was not written by farhelm helm setup"),
            "{error}"
        );
        assert!(path.exists());
        assert_eq!(units.commands, ["show-environment"]);
    }

    /// `--dry-run --uninstall` reports what it would remove and touches
    /// neither systemd nor the disk.
    #[farhelm_testtrace::test]
    fn a_dry_run_uninstall_changes_nothing() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        let mut units = fixture.manager();
        let opts = SetupOptions {
            uninstall: true,
            dry_run: true,
            ..SetupOptions::default()
        };
        let (output, error) = run(&ctx, &opts, &mut units);
        assert!(error.is_empty(), "{error}");
        assert!(output.contains("would remove "), "{output}");
        assert_eq!(units.commands, ["show-environment"]);
        assert!(fixture.unit_dir().join("farhelm-helm.service").exists());
    }

    /// `XDG_CONFIG_HOME` decides which directory the user manager
    /// searches. Writing to the wrong one leaves a valid-looking unit
    /// that is never loaded, so setup has to follow the variable rather
    /// than assume `~/.config` — and, since the variable it can see is
    /// the CALLER's, only when the manager agrees it is the same
    /// directory.
    #[farhelm_testtrace::test]
    fn the_unit_directory_follows_xdg_config_home() {
        let fixture = Fixture::new();
        let xdg = fixture.root.path().join("xdg");
        let mut ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        ctx.xdg_config_home = Some(xdg.clone().into_os_string());
        let mut units = fixture
            .manager()
            .reporting(&format!("XDG_CONFIG_HOME={}\n", xdg.display()));
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut units);
        assert!(error.is_empty(), "{error}");
        assert!(xdg.join("systemd/user/farhelm-helm.service").is_file());
        assert!(!fixture.unit_dir().exists());
    }

    /// A `XDG_CONFIG_HOME` that only this SHELL has is the trap this
    /// check exists for. `XDG_CONFIG_HOME=/srv/config farhelm helm setup`
    /// does not reach the long-running user manager, so setup would
    /// publish two perfectly good unit files where the manager never
    /// looks and then ask it to enable them by name. The enable fails
    /// after the files are on disk, saying nothing about why.
    ///
    /// The refusal is asserted whole: it has to name both directories and
    /// the way out, because nothing else in the failure does.
    #[farhelm_testtrace::test]
    fn a_unit_directory_the_manager_does_not_read_is_refused_before_any_write() {
        let fixture = Fixture::new();
        let elsewhere = fixture.root.path().join("srv-config");
        let mut ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        ctx.xdg_config_home = Some(elsewhere.clone().into_os_string());
        // The manager was started without one, so it reads $HOME/.config.
        let mut units = fixture.manager();
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut units);
        assert_eq!(
            error,
            format!(
                "the running systemd user manager loads units from {}, but this environment \
                 selects {}; run setup with the same XDG_CONFIG_HOME the manager was started with \
                 (or unset it), then rerun",
                fixture.unit_dir().display(),
                elsewhere.join("systemd/user").display()
            )
        );
        assert!(!elsewhere.exists(), "nothing may be written");
        assert!(!fixture.unit_dir().exists());
        assert_eq!(units.commands, ["show-environment"]);
    }

    /// `--uninstall` is guarded by the same check, and its failure mode
    /// is the quieter one: pointed at a directory the manager does not
    /// read, it would find nothing, report both units absent, and leave
    /// the installed ones enabled and running. That looks like success,
    /// which is worse than an install that fails loudly.
    ///
    /// The dry run reports the same mismatch as a note and still removes
    /// nothing, since that is what a dry run promises either way.
    #[farhelm_testtrace::test]
    fn uninstall_refuses_a_unit_directory_the_manager_does_not_read() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        // Install through the manager's own directory first, so there is
        // something real for a mis-pointed uninstall to miss.
        run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        let installed = fixture.unit_dir().join("farhelm-supervisor.service");
        assert!(installed.is_file());

        let elsewhere = fixture.root.path().join("srv-config");
        let mut mispointed = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        mispointed.xdg_config_home = Some(elsewhere.clone().into_os_string());
        let opts = SetupOptions {
            uninstall: true,
            ..SetupOptions::default()
        };
        let mut units = fixture.manager();
        let (output, error) = run(&mispointed, &opts, &mut units);
        assert_eq!(
            error,
            format!(
                "the running systemd user manager loads units from {}, but this environment \
                 selects {}; run setup with the same XDG_CONFIG_HOME the manager was started with \
                 (or unset it), then rerun",
                fixture.unit_dir().display(),
                elsewhere.join("systemd/user").display()
            )
        );
        assert!(
            !output.contains("absent"),
            "a mis-pointed uninstall must not report the real units absent: {output}"
        );
        assert!(installed.is_file(), "nothing may be removed");
        assert_eq!(units.commands, ["show-environment"]);

        let dry = SetupOptions {
            uninstall: true,
            dry_run: true,
            ..SetupOptions::default()
        };
        let mut units = fixture.manager();
        let (output, error) = run(&mispointed, &dry, &mut units);
        assert!(error.is_empty(), "{error}");
        assert!(
            output.contains("note: the running systemd user manager loads units from"),
            "{output}"
        );
        assert!(installed.is_file());
        assert_eq!(units.commands, ["show-environment"]);
    }

    /// The caller's `HOME` is not evidence about the manager, and the
    /// guard has to say so on every path.
    ///
    /// `HOME=/srv/other farhelm helm setup` used to compute
    /// `/srv/other/.config/systemd/user` for BOTH sides of the comparison
    /// — the caller's environment for one, the caller's environment again
    /// as a fallback for the other — so a manager actually reading the
    /// real home passed a check whose entire purpose is to notice that.
    /// Install published units nothing loaded, uninstall reported both
    /// absent while the real services kept running, and dry-run said
    /// nothing at all.
    #[farhelm_testtrace::test]
    fn the_callers_home_never_stands_in_for_the_managers() {
        let fixture = Fixture::new();
        // The manager's home, and the units that really exist there.
        let managers_home = fixture.root.path().join("managers-home");
        let managers_units = managers_home.join(".config/systemd/user");
        std::fs::create_dir_all(&managers_units).unwrap();
        let installed = managers_units.join("farhelm-supervisor.service");
        std::fs::write(&installed, managed("[Unit]\n".to_string())).unwrap();

        // The caller was run with some other HOME and no XDG_CONFIG_HOME,
        // so it selects a directory of its own.
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        let manager =
            || FakeUnits::default().reporting(&format!("HOME={}\n", managers_home.display()));
        let expected = format!(
            "the running systemd user manager loads units from {}, but this environment selects \
             {}; run setup with the same XDG_CONFIG_HOME the manager was started with (or unset \
             it), then rerun",
            managers_units.display(),
            fixture.unit_dir().display()
        );

        let (_, error) = run(&ctx, &SetupOptions::default(), &mut manager());
        assert_eq!(error, expected, "install must refuse");
        assert!(!fixture.unit_dir().exists());

        let uninstall = SetupOptions {
            uninstall: true,
            ..SetupOptions::default()
        };
        let mut units = manager();
        let (output, error) = run(&ctx, &uninstall, &mut units);
        assert_eq!(error, expected, "uninstall must refuse");
        assert!(
            !output.contains("absent"),
            "a mis-pointed uninstall must not certify the real units absent: {output}"
        );
        assert!(installed.is_file(), "the manager's own units must survive");
        assert_eq!(units.commands, ["show-environment"]);

        let dry = SetupOptions {
            dry_run: true,
            ..SetupOptions::default()
        };
        let (output, error) = run(&ctx, &dry, &mut manager());
        assert!(error.is_empty(), "{error}");
        assert!(output.contains(&format!("note: {expected}")), "{output}");
    }

    /// A manager that reports neither variable is a machine setup cannot
    /// locate units for. Guessing there — with the caller's home, or with
    /// anything else — is exactly the confident wrong answer this guard
    /// exists to prevent, so it fails closed.
    #[farhelm_testtrace::test]
    fn a_manager_with_no_usable_environment_is_refused() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        let mut units = FakeUnits::default().reporting("LANG=C\n");
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut units);
        assert!(
            error.contains("reported neither XDG_CONFIG_HOME nor HOME"),
            "{error}"
        );
        assert!(!fixture.unit_dir().exists());
        assert_eq!(units.commands, ["show-environment"]);
    }

    /// Two setups with different options must not be able to interleave
    /// into a mixed pair. The unit files are one configuration — they have
    /// to name the same state directory — but they are published through
    /// two separate renames, so without a lock A's supervisor can end up
    /// beside B's helm, each command reporting success.
    ///
    /// The lock is taken before the ownership preflight and held through
    /// every write and every systemctl call. This test holds it the same
    /// way a second setup would, on its own descriptor.
    #[farhelm_testtrace::test]
    fn a_second_setup_holding_the_lock_is_refused_before_anything_is_written() {
        use std::os::fd::AsRawFd as _;

        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        std::fs::create_dir_all(fixture.unit_dir()).unwrap();
        let held = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(fixture.unit_dir().join(".farhelm-setup.lock"))
            .unwrap();
        // SAFETY: an owned descriptor, and the same non-blocking exclusive
        // request the command itself makes.
        assert_eq!(
            unsafe { libc::flock(held.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );

        let expected = format!(
            "another farhelm helm setup is running for {}; wait for it to finish, then rerun",
            fixture.unit_dir().display()
        );
        let mut units = fixture.manager();
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut units);
        assert_eq!(error, expected);
        assert!(!fixture.unit_dir().join("farhelm-helm.service").exists());
        assert!(
            !fixture
                .unit_dir()
                .join("farhelm-supervisor.service")
                .exists()
        );
        assert_eq!(units.commands, ["show-environment"]);

        // Uninstall is guarded by the same lock, for the same reason.
        let uninstall = SetupOptions {
            uninstall: true,
            ..SetupOptions::default()
        };
        let (_, error) = run(&ctx, &uninstall, &mut fixture.manager());
        assert_eq!(error, expected);

        // A dry run writes nothing to serialize and must not be refused
        // because somebody else is mid-install.
        let dry = SetupOptions {
            dry_run: true,
            ..SetupOptions::default()
        };
        let (output, error) = run(&ctx, &dry, &mut fixture.manager());
        assert!(error.is_empty(), "{error}");
        assert!(output.contains("KillMode=process"), "{output}");

        // Releasing it lets the next command through.
        drop(held);
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        assert!(error.is_empty(), "{error}");
        assert!(fixture.unit_dir().join("farhelm-helm.service").is_file());
    }

    /// A dry run answers the question the operator actually asked, so it
    /// issues this one read-only query and reports a mismatch as a note
    /// rather than a refusal — then goes on to render the units, which is
    /// what they ran the flag for.
    #[farhelm_testtrace::test]
    fn a_dry_run_notes_a_unit_directory_the_manager_does_not_read() {
        let fixture = Fixture::new();
        let elsewhere = fixture.root.path().join("srv-config");
        let mut ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        ctx.xdg_config_home = Some(elsewhere.clone().into_os_string());
        let opts = SetupOptions {
            dry_run: true,
            ..SetupOptions::default()
        };
        let mut units = fixture.manager();
        let (output, error) = run(&ctx, &opts, &mut units);
        assert!(error.is_empty(), "{error}");
        assert!(
            output.contains(&format!(
                "note: the running systemd user manager loads units from {}",
                fixture.unit_dir().display()
            )),
            "{output}"
        );
        assert!(output.contains("KillMode=process"), "{output}");
        assert_eq!(units.commands, ["show-environment"]);
        assert!(!elsewhere.exists());
    }

    /// With no user manager to write units for, there is nothing useful
    /// to do and nothing safe to guess. The refusal carries systemctl's
    /// own stderr, which is the only thing that distinguishes "no manager
    /// on this machine" from "not the session you think you are in".
    #[farhelm_testtrace::test]
    fn setup_refuses_when_no_user_manager_answers() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        let mut units = fixture
            .manager()
            .without_a_manager("Failed to connect to bus: No medium found");
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut units);
        assert!(
            error.contains("the systemd user manager did not answer"),
            "{error}"
        );
        assert!(error.contains("Failed to connect to bus"), "{error}");
        assert!(!fixture.unit_dir().exists());
        assert_eq!(units.commands, ["show-environment"]);
    }

    /// Everything about the manager's directory comes from the manager's
    /// own block, and its quoting has to survive the round trip: systemd
    /// quotes a value when it needs to, and reading the quotes as part of
    /// the path would turn every such machine into a spurious mismatch
    /// refusal.
    ///
    /// The caller's `HOME` appears nowhere here, and that is the point.
    /// Falling back to it made `HOME=/srv/other farhelm helm setup`
    /// compute `/srv/other/.config/systemd/user` on BOTH sides of a
    /// comparison whose only job is to notice that the manager reads
    /// somewhere else.
    #[farhelm_testtrace::test]
    fn the_managers_directory_comes_only_from_the_managers_own_environment() {
        let expected = PathBuf::from("/srv/c/systemd/user");
        for block in [
            "XDG_CONFIG_HOME=/srv/c\n",
            "LANG=C\nXDG_CONFIG_HOME=/srv/c\nPATH=/usr/bin\n",
            "XDG_CONFIG_HOME=\"/srv/c\"\n",
            "XDG_CONFIG_HOME='/srv/c'\n",
            // An absolute XDG_CONFIG_HOME answers outright, whatever the
            // manager's HOME says.
            "HOME=/home/manager\nXDG_CONFIG_HOME=/srv/c\n",
        ] {
            assert_eq!(manager_unit_dir(block).unwrap(), expected, "{block:?}");
        }

        // With no usable XDG_CONFIG_HOME, the manager's OWN home decides.
        for block in [
            "HOME=/home/manager\n",
            "LANG=C\nHOME=/home/manager\n",
            "XDG_CONFIG_HOME=\nHOME=/home/manager\n",
            // A relative XDG value is ignored per the XDG spec, so the
            // home rule applies.
            "XDG_CONFIG_HOME=relative\nHOME=/home/manager\n",
            "HOME=\"/home/manager\"\n",
        ] {
            assert_eq!(
                manager_unit_dir(block).unwrap(),
                PathBuf::from("/home/manager/.config/systemd/user"),
                "{block:?}"
            );
        }

        // Neither usable is a refusal, not a default: guessing is exactly
        // the wrong answer for the one fact this establishes.
        for block in [
            "",
            "LANG=C\n",
            "HOME=\n",
            "HOME=relative\n",
            "XDG_CONFIG_HOME=\n",
        ] {
            let error = manager_unit_dir(block).expect_err("{block:?}");
            assert!(
                error
                    .to_string()
                    .contains("reported neither XDG_CONFIG_HOME nor HOME"),
                "{block:?}: {error}"
            );
        }
    }

    /// `XDG_STATE_HOME` has to reach BOTH units setup writes, because a
    /// helm that resolved its own default would resolve it from the
    /// systemd user manager's environment — not from the shell setup ran
    /// in. This is the split that test proves cannot happen: the caller
    /// exports one value, the manager reports another, and the two
    /// services still name the same tree, because setup resolved it once
    /// and wrote it down twice.
    ///
    /// Without that, `XDG_STATE_HOME=/srv/state farhelm helm setup` gave
    /// the supervisor `/srv/state/farhelm` and the helm
    /// `~/.local/state/farhelm`: both start, and the helm then looks for
    /// the supervisor's socket in a directory nothing listens in.
    #[farhelm_testtrace::test]
    fn the_default_state_directory_follows_xdg_state_home() {
        let fixture = Fixture::new();
        let mut ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        ctx.xdg_state_home = Some(PathBuf::from("/srv/xdg-state").into_os_string());
        // The manager disagrees about the state tree (and agrees about
        // the unit directory, which is the only thing setup refuses over).
        let mut units = fixture.manager().reporting(&format!(
            "HOME={}\nXDG_STATE_HOME=/var/manager-state\nLANG=C\n",
            fixture.home().display()
        ));
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut units);
        assert!(error.is_empty(), "{error}");
        let supervisor =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-supervisor.service")).unwrap();
        assert!(
            supervisor.contains("--state-dir \"/srv/xdg-state/farhelm\""),
            "{supervisor}"
        );
        let helm =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-helm.service")).unwrap();
        assert!(
            helm.contains(" helm run --state-dir \"/srv/xdg-state/farhelm\"\n"),
            "the helm must not be left to resolve its own: {helm}"
        );
        // An explicit --state-dir still wins over the environment: it is
        // the operator saying where state goes.
        let opts = SetupOptions {
            state_dir: Some(PathBuf::from("/srv/pinned")),
            ..SetupOptions::default()
        };
        let (_, error) = run(&ctx, &opts, &mut fixture.manager());
        assert!(error.is_empty(), "{error}");
        let supervisor =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-supervisor.service")).unwrap();
        assert!(
            supervisor.contains("--state-dir \"/srv/pinned\""),
            "{supervisor}"
        );
    }

    /// `FARHELM_TMUX` is how the desktop app and a systemd unit name a
    /// tmux that is not on the shell's `PATH`; setup honours the same
    /// variable, and `--tmux` still wins over it.
    #[farhelm_testtrace::test]
    fn the_tmux_environment_capture_is_honoured_and_the_flag_beats_it() {
        let fixture = Fixture::new();
        let empty = fixture.root.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let from_env = fixture.tmux_dir("tmux 3.7c").join("tmux");
        let mut ctx = fixture.context(&[empty]);
        ctx.tmux_env = Some(from_env.clone().into_os_string());
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        assert!(error.is_empty(), "{error}");
        let supervisor =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-supervisor.service")).unwrap();
        assert!(
            supervisor.contains(&format!("FARHELM_TMUX={}", from_env.display())),
            "{supervisor}"
        );

        let other = Fixture::new();
        let flagged = other.tmux_dir("tmux 3.7c").join("tmux");
        let opts = SetupOptions {
            tmux: Some(flagged.clone()),
            ..SetupOptions::default()
        };
        let (_, error) = run(&ctx, &opts, &mut fixture.manager());
        assert!(error.is_empty(), "{error}");
        let supervisor =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-supervisor.service")).unwrap();
        assert!(
            supervisor.contains(&format!("FARHELM_TMUX={}", flagged.display())),
            "{supervisor}"
        );
    }

    /// An EMPTY `FARHELM_TMUX` means "no override" to whoever wrote it —
    /// a unit or profile line with nothing after the `=`. Reading it as a
    /// program name could only ever fail, so discovery must fall through
    /// to PATH, which is what the supervisor's own resolver does.
    #[farhelm_testtrace::test]
    fn an_empty_tmux_environment_capture_falls_through_to_path() {
        let fixture = Fixture::new();
        let on_path = fixture.tmux_dir("tmux 3.7c");
        let mut ctx = fixture.context(std::slice::from_ref(&on_path));
        ctx.tmux_env = Some(OsString::new());
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        assert!(error.is_empty(), "{error}");
        let supervisor =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-supervisor.service")).unwrap();
        assert!(
            supervisor.contains(&format!("FARHELM_TMUX={}", on_path.join("tmux").display())),
            "{supervisor}"
        );
    }

    /// A relative path means "relative to where I ran this", and systemd
    /// would read it as "relative to the service's working directory" —
    /// the user's home for a user unit. Pinning one verbatim is how a
    /// supervisor ends up on a different state tree than the helm, so
    /// every path setup writes is absolute.
    ///
    /// The working directory is INJECTED: this repository's tests never
    /// change the test runner's own.
    #[farhelm_testtrace::test]
    fn relative_paths_are_resolved_against_the_directory_setup_ran_in() {
        let fixture = Fixture::new();
        let tmux_dir = fixture.tmux_dir("tmux 3.7c");
        // The fixture's cwd holds a relative tmux of its own, so `--tmux
        // ./tmux` has something to resolve to.
        std::fs::create_dir_all(fixture.cwd()).unwrap();
        write_script(&fixture.cwd().join("tmux"), "printf 'tmux 3.7c\\n'");

        let ctx = fixture.context(&[tmux_dir]);
        let opts = SetupOptions {
            state_dir: Some(PathBuf::from("relative/state")),
            tmux: Some(PathBuf::from("./tmux")),
            ..SetupOptions::default()
        };
        let (_, error) = run(&ctx, &opts, &mut fixture.manager());
        assert!(error.is_empty(), "{error}");
        let supervisor =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-supervisor.service")).unwrap();
        assert!(
            supervisor.contains(&format!(
                "--state-dir \"{}/relative/state\"",
                fixture.cwd().display()
            )),
            "{supervisor}"
        );
        assert!(
            supervisor.contains(&format!("FARHELM_TMUX={}/./tmux", fixture.cwd().display())),
            "{supervisor}"
        );
        // The helm unit pins the same resolved directory, so the two
        // halves cannot disagree about where state lives.
        let helm =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-helm.service")).unwrap();
        assert!(
            helm.contains(&format!(
                "--state-dir \"{}/relative/state\"",
                fixture.cwd().display()
            )),
            "{helm}"
        );
    }

    /// A state directory whose name begins with a hyphen is a legal
    /// filesystem path and an illegal-looking CLI option. Because setup
    /// makes every pinned path absolute, the rendered word starts with
    /// `/` and the helm's own parser reads it as the value — checked here
    /// against the REAL parser, since "the renderer is fine but the child
    /// rejects it" is the failure this guards.
    #[farhelm_testtrace::test]
    fn a_hyphen_leading_state_directory_survives_the_child_parser() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        let opts = SetupOptions {
            state_dir: Some(PathBuf::from("--port")),
            ..SetupOptions::default()
        };
        let (_, error) = run(&ctx, &opts, &mut fixture.manager());
        assert!(error.is_empty(), "{error}");
        let pinned = fixture.cwd().join("--port");
        let helm =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-helm.service")).unwrap();
        assert!(
            helm.contains(&format!(" helm run --state-dir \"{}\"\n", pinned.display())),
            "{helm}"
        );
        assert!(pinned.is_absolute());

        let parsed = crate::Cli::try_parse_from([
            std::ffi::OsStr::new("farhelm"),
            std::ffi::OsStr::new("helm"),
            std::ffi::OsStr::new("run"),
            std::ffi::OsStr::new("--state-dir"),
            pinned.as_os_str(),
        ])
        .expect("the helm must accept the state directory setup pinned for it");
        assert!(matches!(
            parsed.command,
            crate::Cmd::Helm {
                command: crate::HelmCmd::Run(args)
            } if args.state_dir.as_deref() == Some(pinned.as_path())
        ));
    }

    /// A RELATIVE `PATH` entry resolves against the directory the command
    /// runs in — that is what the OS does with one, and it is why setup
    /// captures that directory. The candidate found through such an entry
    /// is spelled relatively, and pinning that spelling into a unit would
    /// hand systemd a path it resolves against the service's own working
    /// directory instead.
    ///
    /// The fixture reaches its temporary directory through a relative path
    /// from where the test process happens to be, and hands setup that
    /// same directory as its `cwd`, so PATH resolution and setup's
    /// resolution agree exactly as they do in production. Reading the test
    /// process's working directory is fine; changing it is what this
    /// repository forbids.
    #[farhelm_testtrace::test]
    fn a_relative_path_entry_is_pinned_as_an_absolute_program() {
        /// The spelling of `target` relative to an absolute `base`.
        fn relative_from(base: &Path, target: &Path) -> PathBuf {
            let mut relative = PathBuf::new();
            for _ in base.components().skip(1) {
                relative.push("..");
            }
            for component in target.components().skip(1) {
                relative.push(component);
            }
            relative
        }

        let fixture = Fixture::new();
        let absolute = fixture.tmux_dir("tmux 3.7c");
        let here = std::env::current_dir().expect("the test process has a working directory");
        let entry = relative_from(&here, &absolute);
        assert!(entry.is_relative(), "{}", entry.display());

        let mut ctx = fixture.context(&[entry]);
        ctx.cwd = here;
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        assert!(error.is_empty(), "{error}");
        let supervisor =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-supervisor.service")).unwrap();
        let pinned = supervisor
            .lines()
            .find_map(|line| line.strip_prefix("Environment=\"FARHELM_TMUX="))
            .and_then(|value| value.strip_suffix('"'))
            .expect("the supervisor unit pins a tmux");
        assert!(
            Path::new(pinned).is_absolute(),
            "a relative PATH entry must still pin an absolute program: {pinned}"
        );
        assert_eq!(
            std::fs::canonicalize(pinned).unwrap(),
            std::fs::canonicalize(absolute.join("tmux")).unwrap()
        );
    }

    /// PATH is a list and `execvp` treats it as one: an entry that looks
    /// executable but will not spawn is skipped. Refusing there would
    /// strand an operator whose PATH happens to carry a broken `tmux`
    /// ahead of a working one — a `noexec` mount or a wrong-group execute
    /// bit produces exactly that shape.
    #[farhelm_testtrace::test]
    fn an_unrunnable_path_entry_does_not_hide_a_usable_tmux() {
        let fixture = Fixture::new();
        let shadow = fixture.root.path().join("shadow");
        std::fs::create_dir_all(&shadow).unwrap();
        write_script(&shadow.join("tmux"), "true");
        // Executable, but there is no such interpreter, so the spawn
        // itself fails — the same answer the OS gives for `noexec`.
        std::fs::write(shadow.join("tmux"), b"#!/nonexistent/interpreter\n").unwrap();
        let usable = fixture.tmux_dir("tmux 3.7c");

        let ctx = fixture.context(&[shadow, usable.clone()]);
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        assert!(error.is_empty(), "{error}");
        let supervisor =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-supervisor.service")).unwrap();
        assert!(
            supervisor.contains(&format!("FARHELM_TMUX={}", usable.join("tmux").display())),
            "{supervisor}"
        );
    }

    /// `is-active` answers 3 for an inactive unit and 0 for a running one.
    /// ANY other status is an operational failure — no user manager, a
    /// permission problem, a signal — and reading it as "inactive" would
    /// replace a running unit's file and then skip the restart, leaving
    /// the old process on the old configuration while setup claims
    /// success. So it stops, before the file is touched.
    #[farhelm_testtrace::test]
    fn an_unclassifiable_is_active_result_stops_before_the_unit_is_replaced() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        let mut units = fixture.manager().script(
            "is-active farhelm-supervisor.service",
            1,
            "",
            "Failed to connect to bus: No such file or directory",
        );
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut units);
        assert!(
            error.contains("is-active farhelm-supervisor.service exited 1"),
            "{error}"
        );
        assert!(error.contains("Failed to connect to bus"), "{error}");
        assert!(
            error.contains("would need a restart"),
            "the refusal must say what it could not decide: {error}"
        );
        assert!(
            !fixture
                .unit_dir()
                .join("farhelm-supervisor.service")
                .exists()
        );
        assert_eq!(
            units.commands,
            ["show-environment", "is-active farhelm-supervisor.service"]
        );
    }

    /// Systemd has TWO "not running" answers and setup has to accept
    /// both: 3 for a unit the manager has loaded and stopped, 4 for one it
    /// has never heard of. They differ only in whether the file existed
    /// before, which the ownership preflight has already settled; neither
    /// is a reason to restart anything, and neither is an error.
    #[farhelm_testtrace::test]
    fn both_of_systemds_not_running_answers_mean_no_restart() {
        for stopped in [true, false] {
            let fixture = Fixture::new();
            let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
            run(&ctx, &SetupOptions::default(), &mut fixture.manager());
            // Loaded-but-stopped answers 3; the default fake has never
            // heard of the unit and answers 4.
            let mut units = if stopped {
                fixture.manager().loaded(&["farhelm-helm.service"])
            } else {
                fixture.manager()
            };
            let changed = SetupOptions {
                port: Some(7999),
                ..SetupOptions::default()
            };
            let (_, error) = run(&ctx, &changed, &mut units);
            assert!(error.is_empty(), "stopped={stopped}: {error}");
            assert_eq!(
                units.commands,
                [
                    "show-environment",
                    "is-active farhelm-helm.service",
                    "daemon-reload",
                    "enable --now farhelm-supervisor.service",
                    "enable --now farhelm-helm.service",
                ],
                "stopped={stopped}"
            );
            let helm =
                std::fs::read_to_string(fixture.unit_dir().join("farhelm-helm.service")).unwrap();
            assert!(helm.contains("--port 7999"), "{helm}");
        }
    }

    /// A restart owed by a failed run is still owed by the next one.
    ///
    /// Publishing new bytes for a RUNNING unit leaves a process on the old
    /// configuration until something restarts it, and `daemon-reload` and
    /// `enable --now` are not that something: the first teaches systemd
    /// the new definition, the second starts a stopped unit. If the first
    /// run dies anywhere between the write and the restart, the file now
    /// matches what the retry wants, so the retry calls it unchanged —
    /// and used to report success over both services still running the
    /// old executable, state directory, tmux path, and port.
    ///
    /// Every failure point between publication and the last restart is
    /// covered, including the gap BETWEEN the two restarts, which is the
    /// one that can leave a split configuration: a restarted supervisor on
    /// the new state tree beside a helm still on the old one.
    #[farhelm_testtrace::test]
    fn a_restart_owed_by_a_failed_run_is_paid_by_the_next_one() {
        let supervisor = "farhelm-supervisor.service";
        let helm = "farhelm-helm.service";
        for (failing, still_owed) in [
            // Published, then the reload failed: both are owed.
            ("daemon-reload", vec![supervisor, helm]),
            // Reloaded, then an enable failed: both are still owed.
            (
                "enable --now farhelm-supervisor.service",
                vec![supervisor, helm],
            ),
            ("enable --now farhelm-helm.service", vec![supervisor, helm]),
            // The supervisor restarted; the helm's failed. Only the helm
            // is owed, and leaving it owed is what keeps the pair from
            // settling on two different state trees.
            ("restart farhelm-helm.service", vec![helm]),
        ] {
            let fixture = Fixture::new();
            let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
            run(&ctx, &SetupOptions::default(), &mut fixture.manager());

            // Both units change (the state directory reaches both) and
            // both are running, so both are restart candidates.
            let changed = SetupOptions {
                state_dir: Some(fixture.root.path().join("moved-state")),
                ..SetupOptions::default()
            };
            let mut units = fixture.manager().active(&[supervisor, helm]).script(
                failing,
                1,
                "",
                "planted failure",
            );
            let (_, error) = run(&ctx, &changed, &mut units);
            assert!(error.contains("planted failure"), "{failing}: {error}");
            let published = std::fs::read_to_string(fixture.unit_dir().join(helm)).unwrap();
            assert!(
                published.contains("moved-state"),
                "{failing}: the bytes were published before the failure"
            );

            // The retry: same options, nothing to write, everything to
            // finish. It must restart exactly what is still owed.
            let mut units = fixture.manager().active(&[supervisor, helm]);
            let (_, error) = run(&ctx, &changed, &mut units);
            assert!(error.is_empty(), "{failing}: {error}");
            let mut expected = vec![
                "show-environment".to_string(),
                "daemon-reload".to_string(),
                format!("enable --now {supervisor}"),
                format!("enable --now {helm}"),
            ];
            expected.extend(still_owed.iter().map(|unit| format!("restart {unit}")));
            assert_eq!(units.commands, expected, "{failing}");

            // Paid: nothing is owed a third time.
            for unit in [supervisor, helm] {
                let marker = fixture.unit_dir().join(format!(".{unit}.restart-pending"));
                assert!(!marker.exists(), "{failing}: {} survived", marker.display());
            }
            let mut units = fixture.manager().active(&[supervisor, helm]);
            let (_, error) = run(&ctx, &changed, &mut units);
            assert!(error.is_empty(), "{failing}: {error}");
            assert_eq!(
                units.commands,
                [
                    "show-environment",
                    "daemon-reload",
                    &format!("enable --now {supervisor}")[..],
                    &format!("enable --now {helm}")[..],
                ],
                "{failing}: a settled run must restart nothing"
            );
        }
    }

    /// A unit that was NOT running when its bytes changed owes nothing:
    /// `enable --now` starts it, and a restart on top of that would be a
    /// second, pointless bounce. The marker is the record of that
    /// distinction, so it must not appear.
    #[farhelm_testtrace::test]
    fn a_changed_but_stopped_unit_owes_no_restart() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        let changed = SetupOptions {
            port: Some(7999),
            ..SetupOptions::default()
        };
        let mut units = fixture.manager().loaded(&["farhelm-helm.service"]);
        let (_, error) = run(&ctx, &changed, &mut units);
        assert!(error.is_empty(), "{error}");
        assert!(
            !units
                .commands
                .iter()
                .any(|command| command.starts_with("restart")),
            "{:?}",
            units.commands
        );
        assert!(
            !fixture
                .unit_dir()
                .join(".farhelm-helm.service.restart-pending")
                .exists()
        );
    }

    /// Uninstall clears any outstanding obligation with the unit it
    /// belonged to. A marker left behind would make the NEXT install
    /// restart a service `enable --now` had just started.
    #[farhelm_testtrace::test]
    fn uninstall_clears_an_outstanding_restart_obligation() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        let changed = SetupOptions {
            port: Some(7999),
            ..SetupOptions::default()
        };
        // Fail the reload so the run ends owing both restarts.
        let mut units = fixture
            .manager()
            .active(&["farhelm-supervisor.service", "farhelm-helm.service"])
            .script("daemon-reload", 1, "", "planted failure");
        let (_, error) = run(&ctx, &changed, &mut units);
        assert!(error.contains("planted failure"), "{error}");
        let marker = fixture
            .unit_dir()
            .join(".farhelm-helm.service.restart-pending");
        assert!(marker.exists());

        let uninstall = SetupOptions {
            uninstall: true,
            ..SetupOptions::default()
        };
        let (_, error) = run(&ctx, &uninstall, &mut fixture.manager());
        assert!(error.is_empty(), "{error}");
        assert!(!marker.exists(), "the obligation died with its unit");
    }

    /// A reload that failed after the files were deleted must be retried.
    ///
    /// The filesystem already looks fully uninstalled at that point, so a
    /// retry that decided from "did I delete something" found nothing to
    /// do and reported success — leaving the manager holding definitions
    /// for units that no longer exist, startable until something else
    /// reloaded it.
    #[farhelm_testtrace::test]
    fn an_uninstall_retry_issues_a_reload_that_failed_after_deletion() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        let uninstall = SetupOptions {
            uninstall: true,
            ..SetupOptions::default()
        };

        let mut units = fixture
            .manager()
            .script("daemon-reload", 1, "", "planted failure");
        let (_, error) = run(&ctx, &uninstall, &mut units);
        assert!(error.contains("planted failure"), "{error}");
        assert!(!fixture.unit_dir().join("farhelm-helm.service").exists());
        assert!(
            !fixture
                .unit_dir()
                .join("farhelm-supervisor.service")
                .exists(),
            "the deletions succeeded; only the reload failed"
        );

        let mut units = fixture.manager();
        let (output, error) = run(&ctx, &uninstall, &mut units);
        assert!(error.is_empty(), "{error}");
        assert_eq!(units.commands, ["show-environment", "daemon-reload"]);
        assert!(output.contains("absent "), "{output}");
    }

    /// A closed stdout is a presentation failure and must not abandon a
    /// half-finished machine. The transcript is buffered and written once
    /// at the end, so every unit file and every systemctl command still
    /// happens; the write error is reported afterwards.
    #[farhelm_testtrace::test]
    fn a_failing_writer_still_converges_the_machine() {
        struct BrokenWriter;
        impl Write for BrokenWriter {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "stdout closed",
                ))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        let mut units = fixture.manager();
        let error = run_setup(
            &ctx,
            &SetupOptions::default(),
            &mut units,
            &mut BrokenWriter,
        )
        .expect_err("a broken writer is still an error");
        assert!(
            error.to_string().contains("writing the setup report"),
            "{error}"
        );
        assert!(
            fixture
                .unit_dir()
                .join("farhelm-supervisor.service")
                .is_file()
        );
        assert!(fixture.unit_dir().join("farhelm-helm.service").is_file());
        assert_eq!(
            units.commands,
            [
                "show-environment",
                "is-active farhelm-supervisor.service",
                "is-active farhelm-helm.service",
                "daemon-reload",
                "enable --now farhelm-supervisor.service",
                "enable --now farhelm-helm.service",
            ]
        );
    }

    /// A unit file that cannot be READ is not evidence of anything about
    /// its ownership. Reporting "was not written by farhelm helm setup;
    /// move it aside or delete it" for a permission error or a file with
    /// a stray byte in it asserts a conclusion nobody reached and
    /// recommends destroying the evidence.
    #[farhelm_testtrace::test]
    fn an_unreadable_unit_file_reports_the_read_failure_not_an_ownership_claim() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        std::fs::create_dir_all(fixture.unit_dir()).unwrap();
        let path = fixture.unit_dir().join("farhelm-supervisor.service");
        // Valid marker line, invalid UTF-8 after it: read_to_string fails
        // on content that is unmistakably setup's own.
        let mut bytes = format!("{}\n[Unit]\n", farhelm_helm::units::MANAGED_MARKER).into_bytes();
        bytes.push(0xff);
        std::fs::write(&path, &bytes).unwrap();

        let mut units = fixture.manager();
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut units);
        assert!(error.contains("reading the existing unit file"), "{error}");
        assert!(
            !error.contains("move it aside or delete it"),
            "an unread file must not be called somebody else's: {error}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(units.commands, ["show-environment"]);
    }

    /// The ownership refusal has to cover every shape a target can take,
    /// and it has to happen for BOTH targets before either is written.
    /// The dangerous order is the one the first test cannot reach: a
    /// perfectly ordinary supervisor unit, and a foreign helm unit
    /// discovered only after the supervisor was already replaced.
    #[farhelm_testtrace::test]
    fn foreign_targets_are_refused_in_every_shape_before_anything_is_written() {
        // A directory and a symlink are both "not a regular file", and a
        // symlink is the one that could otherwise redirect a write.
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        std::fs::create_dir_all(fixture.unit_dir()).unwrap();
        let supervisor = fixture.unit_dir().join("farhelm-supervisor.service");
        std::fs::create_dir_all(&supervisor).unwrap();
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        assert!(
            error.contains("was not written by farhelm helm setup"),
            "{error}"
        );
        assert!(supervisor.is_dir(), "the directory must survive");
        assert!(!fixture.unit_dir().join("farhelm-helm.service").exists());

        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        std::fs::create_dir_all(fixture.unit_dir()).unwrap();
        let target = fixture.root.path().join("elsewhere.service");
        std::fs::write(&target, managed("[Unit]\n".to_string())).unwrap();
        let link = fixture.unit_dir().join("farhelm-supervisor.service");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        assert!(
            error.contains("was not written by farhelm helm setup"),
            "a symlink to marked content is still not setup's file: {error}"
        );
        assert!(link.is_symlink(), "the symlink must survive");
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            managed("[Unit]\n".to_string()),
            "the symlink's target must not be written through"
        );

        // The second target is the one that proves preflight order: an
        // installable supervisor, a foreign helm unit.
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        std::fs::create_dir_all(fixture.unit_dir()).unwrap();
        let helm = fixture.unit_dir().join("farhelm-helm.service");
        std::fs::write(&helm, b"[Unit]\nDescription=mine\n").unwrap();
        let mut units = fixture.manager();
        let (_, error) = run(&ctx, &SetupOptions::default(), &mut units);
        assert!(
            error.contains("was not written by farhelm helm setup"),
            "{error}"
        );
        assert_eq!(
            std::fs::read_to_string(&helm).unwrap(),
            "[Unit]\nDescription=mine\n"
        );
        assert!(
            !fixture
                .unit_dir()
                .join("farhelm-supervisor.service")
                .exists(),
            "the first target must not be written when the second is foreign"
        );
        assert_eq!(units.commands, ["show-environment"]);
    }

    /// Uninstall preflights both targets too, and for a sharper reason:
    /// checking them one at a time meant the supervisor was disabled and
    /// DELETED before the foreign helm unit was noticed, leaving a
    /// half-uninstalled machine whose retry then skipped the reload as
    /// well.
    #[farhelm_testtrace::test]
    fn uninstall_refuses_a_foreign_second_target_before_removing_the_first() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        run(&ctx, &SetupOptions::default(), &mut fixture.manager());
        let helm = fixture.unit_dir().join("farhelm-helm.service");
        std::fs::write(&helm, b"[Unit]\nDescription=mine\n").unwrap();

        let mut units = fixture.manager();
        let opts = SetupOptions {
            uninstall: true,
            ..SetupOptions::default()
        };
        let (_, error) = run(&ctx, &opts, &mut units);
        assert!(
            error.contains("was not written by farhelm helm setup"),
            "{error}"
        );
        assert!(
            fixture
                .unit_dir()
                .join("farhelm-supervisor.service")
                .is_file(),
            "the supervisor must survive a refusal on the helm unit"
        );
        assert_eq!(units.commands, ["show-environment"]);
    }

    /// The dry-run transcript is the command's promise about what a real
    /// run would do, and the RESTART is the operationally significant part
    /// of it — the one command that interrupts a serving helm. It cannot
    /// be predicted without asking systemd, which a dry run must not do,
    /// so the preview names the query and states the condition instead.
    /// Order matters as much as content: this is the sequence a reader
    /// will compare against what actually happens.
    #[farhelm_testtrace::test]
    fn a_dry_run_previews_the_status_query_and_the_conditional_restart() {
        let fixture = Fixture::new();
        let ctx = fixture.context(&[fixture.tmux_dir("tmux 3.7c")]);
        // A first install leaves the helm unit unchanged for the dry run
        // below, so the transcript covers both a changed and an unchanged
        // unit in one pass.
        run(&ctx, &SetupOptions::default(), &mut fixture.manager());

        let mut units = fixture.manager();
        let opts = SetupOptions {
            port: Some(7999),
            dry_run: true,
            ..SetupOptions::default()
        };
        let (output, error) = run(&ctx, &opts, &mut units);
        assert!(error.is_empty(), "{error}");
        let transcript: Vec<&str> = output
            .lines()
            .filter(|line| line.starts_with("would run") || line.starts_with("would write"))
            .collect();
        assert_eq!(
            transcript,
            [
                // The query comes before the write, exactly as it does in
                // a real run: after the file changes there is no way to
                // tell a unit that was already running from one this run
                // started.
                "would run: systemctl --user is-active farhelm-helm.service",
                &format!(
                    "would write {}",
                    fixture.unit_dir().join("farhelm-helm.service").display()
                )[..],
                "would run: systemctl --user daemon-reload",
                "would run: systemctl --user enable --now farhelm-supervisor.service",
                "would run: systemctl --user enable --now farhelm-helm.service",
                "would run (if active): systemctl --user restart farhelm-helm.service",
            ],
            "{output}"
        );
        // Nothing was asked of systemd beyond the read-only environment
        // query, and nothing was written.
        assert_eq!(units.commands, ["show-environment"]);
        let helm =
            std::fs::read_to_string(fixture.unit_dir().join("farhelm-helm.service")).unwrap();
        assert!(!helm.contains("--port"), "{helm}");
    }

    /// The CLI grammar itself: that `farhelm helm setup` is reachable
    /// under these exact flag names, and that `--uninstall` refuses the
    /// four flags that would ask it to install something at the same
    /// time. Every other test here builds `SetupOptions` directly and so
    /// proves nothing about the parser.
    #[farhelm_testtrace::test]
    fn the_setup_command_line_parses_and_enforces_its_conflicts() {
        let parsed = crate::Cli::try_parse_from([
            "farhelm",
            "helm",
            "setup",
            "--state-dir",
            "/srv/state",
            "--port",
            "7433",
            "--tmux",
            "/usr/bin/tmux",
            "--no-supervisor",
            "--dry-run",
        ])
        .expect("the ordinary invocation must parse");
        let crate::Cmd::Helm {
            command: crate::HelmCmd::Setup(options),
        } = parsed.command
        else {
            panic!("helm setup must reach the setup arm")
        };
        assert_eq!(options.state_dir.as_deref(), Some(Path::new("/srv/state")));
        assert_eq!(options.port, Some(7433));
        assert_eq!(options.tmux.as_deref(), Some(Path::new("/usr/bin/tmux")));
        assert!(options.no_supervisor);
        assert!(options.dry_run);
        assert!(!options.uninstall);

        // Removal previews are legal; removal combined with any flag that
        // configures an install is not.
        assert!(
            crate::Cli::try_parse_from(["farhelm", "helm", "setup", "--uninstall", "--dry-run"])
                .is_ok()
        );
        for conflicting in [
            vec!["--state-dir", "/srv/state"],
            vec!["--port", "7433"],
            vec!["--tmux", "/usr/bin/tmux"],
            vec!["--no-supervisor"],
        ] {
            let mut argv = vec!["farhelm", "helm", "setup", "--uninstall"];
            argv.extend(conflicting.iter().copied());
            assert!(
                crate::Cli::try_parse_from(&argv).is_err(),
                "{argv:?} must be refused"
            );
        }
    }
}
