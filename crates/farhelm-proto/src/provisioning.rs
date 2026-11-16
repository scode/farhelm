//! Host provisioning as the helm reports it: progress, as `GET /api/hosts/{id}/provisioning` and `GET
//! /api/provisioning` serve it and the UI channel pushes it, and what a probe (`POST /api/hosts/probe`) found, with the
//! plan's steps it offers for confirmation. The list route's wrapper object stays in the helm: the GUI decodes the
//! entries, never the wrapper.
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
/// readable until a later run replaces it. A failed environment launch whose cleanup removed its host is never
/// replaced: the helm keeps listing it under the removed host's id until a bounded list of such runs pushes it out or
/// the helm restarts. An idle view (no run) also says `Completed`, with no run id.
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
    /// The run's own words about how it ended, for a person. A failed environment launch's message is the whole text
    /// to show, its "Not launched." lead included, so a client shows it as it is and adds no lead of its own.
    pub message: Option<String>,
    /// Set on a failed run whose outcome the helm does not know: an environment launch whose task panicked, which may
    /// have left its environment or even its session behind. A client branches on this marker, never on the message's
    /// words, to tell "may still exist" from "not launched". Absent on the wire, and so false, on every other run.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub outcome_unknown: bool,
}

/// One frozen action of a plan as the user sees it before and during a run. The id remains the executor's stable label;
/// the name is the helm's public wording.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanStep {
    pub id: String,
    pub name: String,
    pub paths: Vec<PlanStepPath>,
    pub explanation: Option<String>,
}

/// The single ordered description of a plan used for confirmation and progress: a lead line, the steps, and for an
/// uninstall the data it keeps, a separate note because no action touches that directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanSteps {
    pub lead: String,
    pub steps: Vec<PlanStep>,
    pub kept: Option<String>,
}

/// What `POST /api/hosts/probe` found: an answering supervisor, now registered as it is; a concrete plan to set one up,
/// which a later confirmation consumes by its `probe_id`; or why this host stays a manual install.
///
/// `Plan` is the helm's frozen plan, which it sends beside the steps. Only the helm reads it: a client shows the steps
/// and confirms by `probe_id`, and the helm acts only on the plan it kept under that id. So the helm fills in its own
/// plan type, and a client decodes the field as [`serde::de::IgnoredAny`], which keeps the helm's planning types (its
/// actions, targets and paths) out of this crate without a second, hand-kept copy of the reply's shape.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "kebab-case")]
pub enum ProbeReply<Plan> {
    TensorlakeProvisionable {
        probe_id: String,
        sandbox: String,
        destination: String,
        recipe: String,
        resources: crate::tensorlake::recipes::RecipeSize,
        confirmation: String,
        steps: Box<PlanSteps>,
    },
    Discovered {
        /// The registry id the helm filed the host under.
        host_id: i64,
        build_version: String,
        host_identity: Option<String>,
    },
    Provisionable {
        probe_id: String,
        plan: Plan,
        confirmation: String,
        /// Boxed so that the small outcomes stay small in memory; the JSON is the same.
        steps: Box<PlanSteps>,
    },
    Manual {
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A failed run whose outcome is unknown says so in its own field, which every other run leaves off the wire. Why:
    /// a client must tell a panicked environment launch (it may have left an environment behind) from one that was
    /// not launched without reading words written for people, and adding the field must not change what any other
    /// run looks like to a client that never heard of it. Specified: false is absent from the JSON and reads back as
    /// false, true is written and read back.
    #[test]
    fn a_run_marks_an_unknown_outcome_only_when_it_has_one() {
        let run = |outcome_unknown| ProvisioningView {
            host_id: 7,
            run_id: Some("run-7".into()),
            operation: Some(ProvisioningOperation::Add),
            status: RunStatus::Failed,
            steps: Vec::new(),
            message: Some("ended".into()),
            outcome_unknown,
        };
        for unknown in [false, true] {
            let wire = serde_json::to_value(run(unknown)).unwrap();
            assert_eq!(
                wire.get("outcome_unknown"),
                unknown.then_some(&serde_json::Value::Bool(true)),
                "{wire}"
            );
            let read: ProvisioningView = serde_json::from_value(wire).unwrap();
            assert_eq!(read, run(unknown));
        }
    }

    /// The GUI decodes the probe's reply through [`ProbeReply`] with the plan ignored, while the helm sends its own
    /// plan in that field; a reply the helm writes that the GUI cannot read would leave Add host stuck on an unreadable
    /// answer. Specified: a provisionable reply carrying an arbitrary plan object decodes with its probe id and steps
    /// intact, and a discovered and a manual reply decode by their `result` tags.
    #[test]
    fn a_client_reads_the_probe_reply_without_the_helms_plan() {
        let steps = PlanSteps {
            lead: "Farhelm will set up dev@box (ubuntu, x86_64):".into(),
            steps: vec![PlanStep {
                id: "install-farhelm".into(),
                name: "Install Farhelm".into(),
                paths: vec![PlanStepPath {
                    full: "/home/dev/.local/lib/farhelm".into(),
                    short: "~/.local/lib/farhelm".into(),
                }],
                explanation: None,
            }],
            kept: None,
        };
        let sent = ProbeReply::Provisionable {
            probe_id: "p-1".into(),
            plan: serde_json::json!({ "actions": [{ "kind": "anything the helm keeps" }] }),
            confirmation: "the plan as text".into(),
            steps: Box::new(steps.clone()),
        };
        let wire = serde_json::to_value(&sent).unwrap();
        assert_eq!(wire["result"], "provisionable");
        let read: ProbeReply<serde::de::IgnoredAny> = serde_json::from_value(wire).unwrap();
        let ProbeReply::Provisionable {
            probe_id,
            steps: read_steps,
            ..
        } = read
        else {
            panic!("the reply did not decode as provisionable");
        };
        assert_eq!((probe_id.as_str(), *read_steps), ("p-1", steps));

        let discovered = serde_json::json!({
            "result": "discovered", "host_id": 7, "build_version": "0.16.0", "host_identity": "id-1"
        });
        let read: ProbeReply<serde::de::IgnoredAny> = serde_json::from_value(discovered).unwrap();
        assert!(
            matches!(read, ProbeReply::Discovered { host_id: 7, .. }),
            "{read:?}"
        );
        let manual = serde_json::json!({ "result": "manual", "reason": "no systemd user manager" });
        let read: ProbeReply<serde::de::IgnoredAny> = serde_json::from_value(manual).unwrap();
        assert!(
            matches!(&read, ProbeReply::Manual { reason } if reason == "no systemd user manager"),
            "{read:?}"
        );
    }
}
