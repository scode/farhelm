//! Host provisioning progress, as `GET /api/hosts/{id}/provisioning` and `GET /api/provisioning` serve it and the UI
//! channel pushes it. The list route's wrapper object stays in the helm: the GUI decodes the entries, never the wrapper.
//!
//! The helm owns provisioning (planning, runs, retained progress); this module only holds the shape of what it reports,
//! so the helm's serializer and the GUI's decoder are one type.

use serde::{Deserialize, Serialize};

/// Whether a plan is converging an absent install, explicitly updating an existing one, or removing what provisioning
/// installed. ADD never turns into UPDATE implicitly, and neither ever turns into UNINSTALL.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningOperation {
    Add,
    Update,
    /// Remove the supervisor's service, its unit file and Farhelm's private lib directory from a remote host, keep the
    /// host's state directory, and forget the host. Only ever planned for a remote row; the helm's own machine is
    /// uninstalled with `farhelm uninstall`.
    Uninstall,
}

/// A path in the helm's plan with its complete spelling and a home-relative spelling for narrow dialog chips.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanStepPath {
    pub full: String,
    pub short: String,
}

/// One action's observable progress. Completed runs remain readable until a later run replaces them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Skipped,
    Degraded,
    Failed,
}

/// One action's retained status and bounded explanatory message. The helm carries its plan wording into progress;
/// clients never name steps from ids.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepView {
    pub step: String,
    pub name: String,
    pub paths: Vec<PlanStepPath>,
    pub explanation: Option<String>,
    pub status: StepStatus,
    pub message: Option<String>,
}

/// Where a host's retained run stands as a whole. `Completed` and `Failed` are final for that run; the view stays
/// readable until a later run replaces it. An idle view (no run) also says `Completed`, with no run id.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
}

/// One host's provisioning progress: the run this helm process retains for it, or an explicit idle view for a host
/// that has none.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvisioningView {
    /// The registry id of the host the run is for.
    pub host_id: i64,
    pub run_id: Option<String>,
    pub operation: Option<ProvisioningOperation>,
    pub status: RunStatus,
    pub steps: Vec<StepView>,
    pub message: Option<String>,
}
