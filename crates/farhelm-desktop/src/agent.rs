//! What the desktop app does differently in a build with the test-automation bridge (the `agent` feature): it tells the
//! harness that launched it where its embedded helm is and how to reach it, and it registers the hosts of a staged
//! fleet before its helm serves.
//!
//! NOTE: Debug builds only, and never shipped (farhelm-gui refuses the feature in an optimized build). An ordinary build
//! keeps the helm's address and credential in memory, unreachable from outside the process (SPEC.md, "Security", "Client
//! to helm"); this module exists so that a bridge run can seed sessions and read state through the same API the GUI
//! uses, without a second way in.
//!
//! The harness (`scripts/gui-agent.py`, docs/gui-agent.md) names a file in its private run directory through
//! `FARHELM_DESKTOP_AGENT_HELM_FILE`, and, for a staged run with remote hosts, the ensure-hosts file it wrote through
//! [`ENSURE_HOSTS_ENV`]. With the variables unset this does nothing, so an agent build launched by hand behaves like any
//! other. The harness's other agent-build variables, the first window's size, a forced appearance and the start dialogs
//! turned off, are read by the GUI itself (`farhelm_gui::agent_startup`), because it opens the window, installs the
//! theme and decides what opens at the window's start.

use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;

use anyhow::Context as _;

use crate::DesktopBootstrap;

/// The variable naming the file [`publish_helm`] writes.
pub(crate) const HELM_FILE_ENV: &str = "FARHELM_DESKTOP_AGENT_HELM_FILE";

/// The variable naming an ensure-hosts file the embedded helm registers before it serves, which `DesktopBootstrap::start`
/// reads in an agent build and passes on as the helm's `ensure_hosts` (`crate::embedded_helm_args`).
///
/// This is how a staged run (`scripts/gui-agent.py start --remote-host`) gives the app a fleet of several hosts: the
/// installed app's helm starts with no ensure-hosts file, so without this hook an agent build could only ever show its
/// local host. The file is the standalone helm's own `--ensure-hosts` format, and the helm reads it as it reads that
/// flag's: before serving, all or nothing, a missing or malformed file failing startup with the file named. The harness
/// always passes an absolute path.
pub(crate) const ENSURE_HOSTS_ENV: &str = "FARHELM_DESKTOP_AGENT_ENSURE_HOSTS";

/// Write the embedded helm's address and this process's credential as JSON (`{"api_base": ..., "credential": ...}`) to
/// the file `FARHELM_DESKTOP_AGENT_HELM_FILE` names, if it is set.
///
/// The file is created new with mode 0600 and never replaced: a file already there means another run's harness owns
/// the path, and the launch fails rather than overwrite it. The credential dies with this process, like every other
/// the helm mints.
pub(crate) fn publish_helm(desktop: &DesktopBootstrap) -> anyhow::Result<()> {
    let Some(path) = std::env::var_os(HELM_FILE_ENV) else {
        return Ok(());
    };
    let body = serde_json::json!({
        "api_base": desktop.api_base(),
        "credential": desktop.native_device_secret(),
    });
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .with_context(|| format!("creating the bridge's helm file {}", path.to_string_lossy()))?;
    file.write_all(body.to_string().as_bytes())
        .and_then(|()| file.sync_all())
        .with_context(|| format!("writing the bridge's helm file {}", path.to_string_lossy()))
}
