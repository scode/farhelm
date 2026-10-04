//! Require explicit confirmation before a YOLO launch unless the host permits those
//! launches without asking.
//!
//! Every host asks before YOLO launches until the user turns off that confirmation
//! (`HostRow::yolo_without_asking`, the host settings dialog). Starting an agent that runs
//! without approval prompts on the wrong machine by accident is the failure this exists to
//! stop, so the check lives here, on every create path the helm has (a new session, a fresh
//! checkout, replace, and an agent's create and clone) and on restart-with, rather than in
//! any one client: no client can skip it. A plain restart or resume relaunches a choice the
//! user already made and is not checked.
//!
//! A refused launch is never dispatched. The browser answers the refusal
//! with a loud confirmation and retries the same request with the override
//! set. An agent has no override: its launches go through [`check_agent`]
//! instead, which refuses outright what [`check`] would ask about, and more
//! (SPEC.md, Agent-spawned sessions).

use crate::AppState;
use crate::store::HostId;

/// A YOLO launch refused because its host asks before YOLO launches and the request did
/// not carry the override.
///
/// A distinct type so the HTTP layer can recognize it (and only it) and mark
/// the response with the confirmation header the browser acts on; the
/// message is what the command line and agents show.
#[derive(Debug, thiserror::Error)]
#[error(
    "{host_name} asks before YOLO launches, and this launch skips approval prompts: nothing was \
     started. Confirm the YOLO launch explicitly in the GUI, or turn on \"start YOLO sessions here \
     without asking\" in the host's settings"
)]
pub(crate) struct YoloNeedsConfirmation {
    pub(crate) host_name: String,
}

/// Whether a launch is YOLO, as the launch itself says
/// ([`farhelm_proto::SessionLaunch::yolo`]): an agent launch by its
/// effective permission, a command launch by the user's assertion. Farhelm
/// never reads a command line to second-guess either (SPEC.md). A legacy
/// launch carries no verdict and is never created anew, so it reads as not
/// YOLO here; nothing that can reach this check holds one.
pub(crate) fn create_is_yolo(launch: &farhelm_proto::SessionLaunch) -> bool {
    launch.yolo() == Some(true)
}

/// Refuse a YOLO launch on `host` unless the host allows YOLO without asking or the request
/// carries the override. Reads the host's setting from the store at the moment of the
/// decision, so a change in the settings dialog applies to the very next launch. A YOLO
/// launch to a host with no registry row is refused with the helm's unknown-host error
/// rather than allowed.
pub(crate) async fn check(
    state: &AppState,
    host: HostId,
    is_yolo: bool,
    confirm_yolo: bool,
) -> anyhow::Result<()> {
    if !is_yolo || confirm_yolo {
        return Ok(());
    }
    let row = state
        .store
        .list_hosts()
        .await?
        .into_iter()
        .find(|row| row.id == host);
    match row {
        Some(row) if row.yolo_without_asking => Ok(()),
        Some(row) => Err(anyhow::Error::new(YoloNeedsConfirmation {
            host_name: crate::aggregate::host_display_name(
                row.kind,
                row.destination.as_deref(),
                row.alias.as_deref(),
            ),
        })),
        // A host the registry no longer has is refused here, with the same
        // not-found error routing gives an unknown host
        // (`sessions::no_such_host`). Routing does not
        // cover it: create and restart-with take the host's connection
        // before this check, and host removal deletes the row before it
        // stops the connection, so a launch in that window would otherwise be
        // dispatched over the still-open connection unasked. (The wider race
        // of any create landing on a host being removed is accepted; this
        // only keeps the YOLO check from failing open.)
        None => Err(crate::sessions::no_such_host(host)),
    }
}

/// Refuse an agent's launch on `host` when the host asks before YOLO launches
/// and the launch is one Farhelm cannot vouch for: YOLO by its own verdict,
/// unclassified, a command launch whatever its YOLO assertion says, or an
/// agent launch whose command lines are not what Farhelm composes for its
/// choices.
///
/// SPEC.md lets an agent start, on such a host, only what runs the way the
/// user already approved. Farhelm never reads a command line, so a command
/// launch's assertion is the caller's word, and an agent's word is exactly
/// what this host's setting says not to take. An agent launch is classified
/// by its choices, which is exact only while its stored command lines are the
/// ones Farhelm composes from those choices: a clone copies its source's
/// stored launch as the SOURCE host's supervisor reports it, and a
/// compromised one could pair "default permissions" with a YOLO command line.
/// So the launch is recomposed here (`launches::compile`) and vouched for
/// only when it matches exactly; one composed by an older Farhelm, or forged,
/// is refused like a command launch. The refusal comes
/// before the approval card, so the user is never asked to approve something
/// the host's own setting rules out, and again when the user approves (the
/// setting can change during the wait). Reads the setting at the moment of
/// the decision, like [`check`].
pub(crate) async fn check_agent(
    state: &AppState,
    host: HostId,
    launch: &farhelm_proto::SessionLaunch,
) -> anyhow::Result<()> {
    let vouched = match launch {
        farhelm_proto::SessionLaunch::Agent { selection, .. } => {
            launch.yolo() == Some(false)
                && crate::launches::compile(selection.clone()).as_ref() == Ok(launch)
        }
        farhelm_proto::SessionLaunch::Command(_) | farhelm_proto::SessionLaunch::Legacy { .. } => {
            false
        }
    };
    if vouched {
        return Ok(());
    }
    let row = state
        .store
        .list_hosts()
        .await?
        .into_iter()
        .find(|row| row.id == host);
    match row {
        Some(row) if row.yolo_without_asking => Ok(()),
        Some(row) => {
            let host_name = crate::aggregate::host_display_name(
                row.kind,
                row.destination.as_deref(),
                row.alias.as_deref(),
            );
            Err(anyhow::Error::new(crate::SupervisorError {
                origin: crate::client::ErrorOrigin::Helm,
                kind: farhelm_proto::ErrorKind::Unauthorized,
                message: format!(
                    "{host_name} asks before YOLO launches, so an agent may not start a YOLO \
                     launch, a command launch, or an agent launch whose command line Farhelm \
                     did not compose itself (such as a copy of a session an older Farhelm \
                     started) there: nothing was started. To let agents do this, turn on \
                     \"start YOLO sessions here without asking\" in {host_name}'s settings"
                ),
            }))
        }
        None => Err(crate::sessions::no_such_host(host)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: a YOLO launch checked against a host id the registry has no row for is refused
    /// with the helm's unknown-host error ("no such host", the same text routing gives),
    /// not allowed; a launch that is not YOLO, or carries the override, is not this check's
    /// to refuse.
    ///
    /// Why: create and restart-with hold the host's connection before this check, and host
    /// removal deletes the row before stopping that connection, so a missing row does not
    /// mean routing will refuse. When this check let a missing row through, a YOLO launch
    /// in that window started an approval-free agent without the required confirmation.
    #[farhelm_testtrace::test]
    async fn a_yolo_check_against_a_host_with_no_registry_row_is_refused() {
        let harness = crate::rest_harness::idle_helm().await;
        let missing: HostId = 999_999;
        assert!(
            harness
                .store
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .all(|row| row.id != missing),
            "premise: no registry row has this id"
        );
        let refused = check(&harness.state, missing, true, false)
            .await
            .expect_err("a YOLO launch to a missing host must be refused");
        let refusal = refused
            .downcast_ref::<crate::SupervisorError>()
            .unwrap_or_else(|| panic!("refused with the helm's own error, got: {refused:#}"));
        assert_eq!(refusal.kind, crate::ErrorKind::NotFound);
        assert_eq!(
            refusal.message,
            format!("no such host: {missing}"),
            "the same text as any launch to an unknown host"
        );
        assert!(check(&harness.state, missing, false, false).await.is_ok());
        assert!(check(&harness.state, missing, true, true).await.is_ok());
    }

    /// Spec: on a host that asks before YOLO launches, the agent rule allows
    /// a non-YOLO agent launch only when its command lines are exactly what
    /// Farhelm composes for its choices, and refuses one with the same choices
    /// but different command lines.
    ///
    /// Why: a clone copies its source's stored launch as the source host's
    /// supervisor reports it. A compromised source could pair non-YOLO
    /// choices with a YOLO command line, and classifying by the choices alone
    /// would let that run on a host whose user said to ask first.
    #[farhelm_testtrace::test]
    async fn the_agent_rule_vouches_only_for_launches_farhelm_composed() {
        let harness = crate::rest_harness::idle_helm().await;
        let local = crate::rest_harness::local_id(&harness.store).await;
        let selection = farhelm_proto::LaunchSelection {
            harness: farhelm_proto::LaunchHarness::Claude,
            model: None,
            effort: None,
            permissions: None,
            workspace_trust: None,
        };
        let composed = crate::launches::compile(selection.clone()).expect("compiles");
        assert_eq!(composed.yolo(), Some(false), "premise: a non-YOLO launch");
        check_agent(&harness.state, local, &composed)
            .await
            .expect("Farhelm's own composition is vouched for");
        let farhelm_proto::SessionLaunch::Agent { resume, .. } = &composed else {
            panic!("an agent launch");
        };
        let forged = farhelm_proto::SessionLaunch::Agent {
            selection,
            start: vec![
                "claude".to_string(),
                "--dangerously-skip-permissions".to_string(),
                farhelm_proto::session_launch::FARHELM_ARGS_PLACEHOLDER.to_string(),
            ],
            resume: resume.clone(),
        };
        let refused = check_agent(&harness.state, local, &forged)
            .await
            .expect_err("forged command lines are not vouched for");
        assert!(format!("{refused:#}").contains("may not start a YOLO launch"));
    }
}
