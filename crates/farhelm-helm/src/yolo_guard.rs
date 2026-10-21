//! Require explicit confirmation before a YOLO launch unless the host permits those
//! launches without asking.
//!
//! Every host asks before YOLO launches until the user turns off that confirmation
//! (`HostRow::yolo_without_asking`, the host settings dialog). Starting an agent that bypasses
//! its safety checks on the wrong machine by accident is the failure this exists to
//! stop, so the check lives here, on every create path the helm has (a new session, a fresh
//! checkout, replace, and an agent's create and clone) and on restart-with, rather than in
//! any one client: no client can skip it. A plain restart or resume relaunches a choice the
//! user already made and is not checked.
//!
//! A refused launch is never dispatched. The browser answers the refusal
//! with a loud confirmation and retries the same request with the override
//! set. An agent has no override: its launches go through [`check_agent_wrapper`]
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
    "{host_name} asks before YOLO launches, and this launch is YOLO or uses the agent's own setting: nothing was \
     started. Confirm the YOLO launch explicitly in the GUI, or turn on \"start YOLO sessions here \
     without asking\" in the host's settings"
)]
pub(crate) struct YoloNeedsConfirmation {
    pub(crate) host_name: String,
}

/// Whether admission must treat a launch as YOLO, as the launch itself says
/// ([`farhelm_proto::SessionLaunch::yolo`]): an agent launch by its
/// stored permission (including an unclassified own-setting mode), a command
/// launch by the user's assertion. Farhelm
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

/// An exact, non-YOLO agent launch may still need one user's Allow when the
/// agent supplied its wrapper. This verdict is about this request, never a
/// durable property of the launch or a host setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentVerdict {
    Vouched,
    ApprovalRequired,
}

/// Test the ordinary recomposition rule without an agent-supplied wrapper.
#[cfg(test)]
pub(crate) async fn check_agent(
    state: &AppState,
    host: HostId,
    launch: &farhelm_proto::SessionLaunch,
) -> anyhow::Result<AgentVerdict> {
    check_agent_wrapper(state, host, launch, false).await
}

/// Refuse a launch the host's YOLO rule cannot vouch for, or require a direct
/// Allow when only its agent-supplied wrapper lacks approval.
///
/// A copied launch comes from the source supervisor, which could pair safe
/// choices with a different command. Recomposition must match both stored
/// command lines exactly before wrapper provenance matters. YOLO, legacy and
/// command launches stay refused on a host that asks first, even with an
/// approval; that decision is made before the card, then repeated after the
/// wait and at dispatch because the host setting can change in between.
pub(crate) async fn check_agent_wrapper(
    state: &AppState,
    host: HostId,
    launch: &farhelm_proto::SessionLaunch,
    agent_wrapper_supplied: bool,
) -> anyhow::Result<AgentVerdict> {
    let vouched = match launch {
        farhelm_proto::SessionLaunch::Agent { selection, .. } => {
            launch.yolo() == Some(false)
                && crate::launches::compile(selection.clone()).as_ref() == Ok(launch)
        }
        farhelm_proto::SessionLaunch::Command(_) | farhelm_proto::SessionLaunch::Legacy { .. } => {
            false
        }
    };
    let wrapper_needs_approval = vouched
        && agent_wrapper_supplied
        && launch
            .agent_selection()
            .is_some_and(|selection| selection.wrapper_prefix.is_some());
    if vouched && !wrapper_needs_approval {
        return Ok(AgentVerdict::Vouched);
    }
    let row = state
        .store
        .list_hosts()
        .await?
        .into_iter()
        .find(|row| row.id == host);
    match row {
        Some(row) if row.yolo_without_asking => Ok(AgentVerdict::Vouched),
        Some(_) if wrapper_needs_approval => Ok(AgentVerdict::ApprovalRequired),
        Some(row) => {
            let host_name = crate::aggregate::host_display_name(
                row.kind,
                row.destination.as_deref(),
                row.alias.as_deref(),
            );
            Err(anyhow::Error::new(crate::SupervisorError::helm(
                farhelm_proto::ErrorKind::Unauthorized,
                format!(
                    "{host_name} asks before YOLO launches, so an agent may not start a YOLO \
                     launch, a command launch, or an agent launch whose command line Farhelm \
                     did not compose itself (such as a copy of a session an older Farhelm \
                     started) there: nothing was started. To let agents do this, turn on \
                     \"start YOLO sessions here without asking\" in {host_name}'s settings"
                ),
            )))
        }
        None => Err(crate::sessions::no_such_host(host)),
    }
}

/// Repeat the host check after a wait and at dispatch. Only the exact forced
/// wrapper card grants the approval required by this verdict; a host command
/// shortcut or an unrelated card cannot supply it.
pub(crate) async fn check_agent_approved(
    state: &AppState,
    host: HostId,
    launch: &farhelm_proto::SessionLaunch,
    agent_wrapper_supplied: bool,
    wrapper_approved: bool,
) -> anyhow::Result<()> {
    match check_agent_wrapper(state, host, launch, agent_wrapper_supplied).await? {
        AgentVerdict::Vouched => Ok(()),
        AgentVerdict::ApprovalRequired if wrapper_approved => Ok(()),
        AgentVerdict::ApprovalRequired => Err(anyhow::Error::new(crate::SupervisorError::helm(
            farhelm_proto::ErrorKind::Unauthorized,
            "the agent's wrapper needs an Allow answer for this launch; nothing was started"
                .to_string(),
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A copied pre-upgrade command keeps its own-setting classification:
    /// neither a GUI replace nor an agent clone/inherit may pass it off as a
    /// newly compiled guarded launch on a host that asks first.
    #[farhelm_testtrace::test]
    async fn old_no_flag_launches_and_their_copies_require_confirmation() {
        let harness = crate::rest_harness::idle_helm().await;
        let host = crate::rest_harness::local_id(&harness.store).await;
        let old = crate::launches::test_agent_launch(farhelm_proto::LaunchSelection {
            permission_flags: None,
            harness: farhelm_proto::LaunchHarness::Claude,
            model: None,
            effort: None,
            permissions: None,
            wrapper_prefix: None,
            workspace_trust: None,
        });
        assert_eq!(
            old.yolo(),
            Some(true),
            "premise: no guarded flags were recorded"
        );
        for copied in [
            old.clone(),
            serde_json::from_str(&serde_json::to_string(&old).unwrap()).unwrap(),
        ] {
            assert!(create_is_yolo(&copied));
            assert!(
                check(&harness.state, host, create_is_yolo(&copied), false)
                    .await
                    .unwrap_err()
                    .is::<YoloNeedsConfirmation>()
            );
            assert!(check_agent(&harness.state, host, &copied).await.is_err());
        }
    }

    /// Custom words can name a guarded vendor mode without proving one. The
    /// user's YOLO consent is therefore required, and an agent cannot borrow
    /// the exact-rebuild guarded exemption merely because the words look safe.
    #[farhelm_testtrace::test]
    async fn custom_flags_require_the_yolo_gate_and_cannot_be_agent_preapproved() {
        let harness = crate::rest_harness::idle_helm().await;
        let host = crate::rest_harness::local_id(&harness.store).await;
        let launch = crate::launches::compile(farhelm_proto::LaunchSelection {
            harness: farhelm_proto::LaunchHarness::Claude,
            model: None,
            effort: None,
            permissions: Some(farhelm_proto::LaunchPermission::Custom),
            permission_flags: Some(vec!["--permission-mode".into(), "auto".into()]),
            wrapper_prefix: None,
            workspace_trust: None,
        })
        .unwrap();
        assert_eq!(launch.yolo(), Some(true));
        assert!(
            !harness
                .store
                .list_hosts()
                .await
                .unwrap()
                .into_iter()
                .find(|row| row.id == host)
                .unwrap()
                .yolo_without_asking
        );
        assert!(
            check(&harness.state, host, create_is_yolo(&launch), false)
                .await
                .unwrap_err()
                .is::<YoloNeedsConfirmation>()
        );
        assert!(
            check(&harness.state, host, create_is_yolo(&launch), true)
                .await
                .is_ok()
        );
        assert!(check_agent(&harness.state, host, &launch).await.is_err());
    }

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
        assert_eq!(refusal.kind(), crate::ErrorKind::NotFound);
        assert_eq!(
            refusal.message(),
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
            permission_flags: None,
            harness: farhelm_proto::LaunchHarness::Claude,
            model: None,
            effort: None,
            permissions: None,
            wrapper_prefix: None,
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

    /// A matching command proves what Farhelm composed, but only request
    /// provenance says whether the agent chose its wrapper. The approval
    /// exception covers that one choice without admitting a YOLO launch.
    #[farhelm_testtrace::test]
    async fn agent_wrapper_needs_an_explicit_answer_on_a_guarded_host() {
        let harness = crate::rest_harness::idle_helm().await;
        let host = crate::rest_harness::local_id(&harness.store).await;
        assert!(
            !harness
                .store
                .list_hosts()
                .await
                .unwrap()
                .into_iter()
                .find(|row| row.id == host)
                .unwrap()
                .yolo_without_asking,
            "premise: this host asks before YOLO"
        );
        let selection = farhelm_proto::LaunchSelection {
            harness: farhelm_proto::LaunchHarness::Claude,
            model: None,
            effort: None,
            permissions: None,
            permission_flags: None,
            wrapper_prefix: Some(vec!["with-endpoint".into(), "--".into()]),
            workspace_trust: None,
        };
        let launch = crate::launches::compile(selection.clone()).expect("composed wrapper");
        assert_eq!(launch.yolo(), Some(false), "premise: guarded agent");
        assert_eq!(
            check_agent_wrapper(&harness.state, host, &launch, false)
                .await
                .unwrap(),
            AgentVerdict::Vouched,
            "a named template supplied the wrapper"
        );
        assert_eq!(
            check_agent_wrapper(&harness.state, host, &launch, true)
                .await
                .unwrap(),
            AgentVerdict::ApprovalRequired,
            "the request flags supplied the same wrapper"
        );
        assert!(
            check_agent_approved(&harness.state, host, &launch, true, false)
                .await
                .is_err()
        );
        check_agent_approved(&harness.state, host, &launch, true, true)
            .await
            .expect("the forced card was answered Allow");

        let yolo = crate::launches::compile(farhelm_proto::LaunchSelection {
            permissions: Some(farhelm_proto::LaunchPermission::Yolo),
            ..selection
        })
        .expect("composed YOLO wrapper");
        assert_eq!(yolo.yolo(), Some(true), "premise: YOLO choice");
        assert!(
            check_agent_approved(&harness.state, host, &yolo, true, true)
                .await
                .is_err(),
            "a wrapper approval cannot override the host YOLO guard"
        );
    }
}
