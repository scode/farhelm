//! Remove the selected standalone installation while preserving user data.
//!
//! File ownership is established before confirmation. Runtime shutdown is an
//! operator prerequisite; this command does not inspect processes or sessions.
//! Setup-owned services are the sole automatic shutdown operation.

use anyhow::{Context as _, Result, bail};
use std::{
    ffi::OsString,
    fmt::Write as _,
    io::{BufRead, IsTerminal as _, Read as _, Write},
    path::{Path, PathBuf},
};

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
        "Stop local sessions and additional terminals, quit Farhelm Desktop, and stop manually started Farhelm processes before continuing. Uninstall does not check whether they are running.\n\n",
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
    preview.push_str("Other custom data locations, projects, agent tools and dependencies are untouched.\nKeep install, upgrade, setup and Farhelm startup stopped until uninstall finishes.\n");
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
    let mut report = String::new();
    let result = (|| {
        if let Some(services) = &services {
            crate::setup::remove_selected_services(services, false, manager, &mut report)?;
        }
        removal::remove(&plan, &mut report)
    })();
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

/// Escape paths so a filename cannot forge an extra line of diagnostic output.
fn path_text(path: &Path) -> String {
    ownership::path_text(path)
}
