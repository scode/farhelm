//! What the desktop app does differently in a build with the test-automation bridge (the `agent` feature): it tells the
//! harness that launched it where its embedded helm is and how to reach it.
//!
//! NOTE: Debug builds only, and never shipped (farhelm-gui refuses the feature in an optimized build). An ordinary build
//! keeps the helm's address and credential in memory, unreachable from outside the process (SPEC.md, "Security", "Client
//! to helm"); this module exists so that a bridge run can seed sessions and read state through the same API the GUI
//! uses, without a second way in.
//!
//! The harness (`scripts/gui-agent.py`, docs/gui-agent.md) names a file in its private run directory through
//! `FARHELM_DESKTOP_AGENT_HELM_FILE`. With the variable unset this does nothing, so an agent build launched by hand
//! behaves like any other. The harness's other agent-build variables, the first window's size and a forced appearance,
//! are read by the GUI itself (`farhelm_gui::agent_startup`), because it opens the window and installs the theme.

use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;

use anyhow::Context as _;

use crate::DesktopBootstrap;

/// The variable naming the file [`publish_helm`] writes.
pub(crate) const HELM_FILE_ENV: &str = "FARHELM_DESKTOP_AGENT_HELM_FILE";

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
