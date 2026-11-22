//! Workspace-trust policies for launches the helm sends to a supervisor.
//!
//! Keeping construction here makes choosing a policy a prerequisite for create
//! and Restart with dispatch. Approval and destination liveness remain the
//! routes' responsibility: this type neither grants permission nor holds a
//! lease on a host or directory.

use crate::sessions::invalid_request;
use crate::{AppState, SupervisorClient, launches};
use farhelm_proto::{LaunchSelection, SessionLaunch};

/// A launch whose workspace-trust policy has been chosen for this operation.
///
/// New compositions use destination Settings; plain Replace deliberately keeps
/// the stored choice. Neither policy proves YOLO approval or current liveness.
/// The private payload prevents routes from treating an arbitrary raw launch as
/// resolved, or replacing the launch after its trust choice was approved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrustResolvedLaunch(SessionLaunch);

impl TrustResolvedLaunch {
    /// Inspect the fixed launch for guards, card contents and bookkeeping.
    /// The borrow cannot change the trust choice carried to dispatch.
    pub(crate) fn as_launch(&self) -> &SessionLaunch {
        &self.0
    }

    /// Consume the policy evidence at the unchanged supervisor-client boundary.
    /// Callers must finish the existing prompt and approval guards before sending.
    pub(crate) fn into_launch(self) -> SessionLaunch {
        self.0
    }

    /// Freeze the directory that an agent-requested approval card will show.
    /// A checked path binds trust to the shown destination across the wait for
    /// Allow. The lookup uses the same host connection as the approval and
    /// does not consult checkout ownership to grant trust.
    pub(crate) async fn pin_trusted_approval_cwd(
        &self,
        client: &SupervisorClient,
        cwd: &mut String,
    ) -> anyhow::Result<()> {
        if self.pins_approval_cwd() {
            *cwd = crate::sessions::within_host_read_bound(client.canonical_launch_directory(cwd))
                .await?;
        }
        Ok(())
    }

    /// Whether [`Self::pin_trusted_approval_cwd`] checks the folder at all: only when this launch carries trust for an
    /// agent whose trust follows the folder. A caller that ties the approval to the host connection the check used
    /// does so only then, so a launch without trust does not depend on that connection surviving the wait for Allow.
    pub(crate) fn pins_approval_cwd(&self) -> bool {
        self.0.agent_selection().is_some_and(|selection| {
            selection.workspace_trust == Some(true)
                && launches::workspace_trust_pins_approval_cwd(selection.harness)
        })
    }
}

/// Choose the no-trust policy only when this launch has no workspace-trust knob.
///
/// Commands keep all literal words, even if they declare a trust-capable harness
/// or spell its trust switches. Unsupported agent harnesses likewise need no
/// Settings or destination read. `Err` returns the original trust-capable agent
/// launch for its existing resolution branch; it is not a request refusal.
#[allow(
    clippy::result_large_err,
    reason = "Err carries the unchanged input to another policy, not a failure"
)]
pub(crate) fn try_without_workspace_trust(
    launch: SessionLaunch,
) -> Result<TrustResolvedLaunch, SessionLaunch> {
    if launch
        .agent_selection()
        .is_some_and(|selection| selection.harness.offers_workspace_trust())
    {
        Err(launch)
    } else {
        Ok(TrustResolvedLaunch(launch))
    }
}

/// Apply current Settings to a new composition. A supported launch uses its
/// agent's switch in every directory. Commands and unsupported agents keep
/// their words without reading Settings.
pub(crate) async fn normalize_launch_workspace_trust(
    state: &AppState,
    launch: SessionLaunch,
) -> anyhow::Result<TrustResolvedLaunch> {
    let launch = match try_without_workspace_trust(launch) {
        Ok(resolved) => return Ok(resolved),
        Err(launch) => launch,
    };
    let config = state.store.resolve_checkout_config(None).await?;
    configured_launch_workspace_trust(launch, &config)
}

/// The setting can contribute trust only when it does not override the
/// user's own Antigravity settings in Custom flags.
fn trust_enabled_for_selection(
    selection: &LaunchSelection,
    config: &crate::checkout_config::ResolvedCheckoutConfig,
) -> bool {
    launches::workspace_trust_enabled(selection.harness, config)
        && !launches::workspace_trust_blocked_by_selection(selection)
}

/// Resolve the setting into the selection while preserving non-trust argv.
/// Fresh creates use the same Settings snapshot as their checkout and
/// post-clone command. Agent-requested launches carry the choice shown on the
/// card through dispatch. Commands retain their literal words.
pub(crate) fn configured_launch_workspace_trust(
    launch: SessionLaunch,
    config: &crate::checkout_config::ResolvedCheckoutConfig,
) -> anyhow::Result<TrustResolvedLaunch> {
    let trust = launch
        .agent_selection()
        .and_then(|selection| trust_enabled_for_selection(selection, config).then_some(true));
    launches::with_workspace_trust(launch, trust)
        .map(TrustResolvedLaunch)
        .map_err(invalid_request)
}

/// Preserve plain Replace's exact stored launch, including historical trust.
///
/// The source must be the owning supervisor's live row, as required by
/// `mode_from_source`. Replace is a same-host, same-folder copy; consulting
/// today's Settings or recompiling historical words would change that contract.
/// Inherited spawn and agent clone compose for a destination and must normalize
/// instead. Legacy rows retain the existing Replace-with remedy and refusal.
pub(crate) fn retain_stored_launch_workspace_trust(
    source: &farhelm_proto::SessionInfo,
) -> anyhow::Result<TrustResolvedLaunch> {
    crate::sessions::mode_from_source(
        source,
        "use Replace with to start it over with a launch of your choosing",
    )
    .map(TrustResolvedLaunch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::{CommandLaunch, LaunchHarness, LaunchSelection};

    /// Command text is literal even when its declared harness supports trust.
    /// The synchronous constructor needs no Settings or destination inputs and
    /// must preserve trust-looking words in both start and resume commands.
    #[test]
    fn no_trust_policy_preserves_literal_command_words() {
        let launch = SessionLaunch::Command(CommandLaunch {
            command: "codex --trust-workspace --no-approve {farhelm_args}".into(),
            yolo: false,
            agent: Some(LaunchHarness::Codex),
            resume: Some("codex --no-approve {conversation} {farhelm_args}".into()),
        });
        if let SessionLaunch::Command(command) = &launch {
            command
                .validate()
                .expect("fixture is a valid literal command");
        }
        let resolved = try_without_workspace_trust(launch.clone()).unwrap();
        assert_eq!(resolved.into_launch(), launch);
    }

    /// A no-trust decision cannot be used to bypass a supported agent's
    /// destination policy. Unsupported agents preserve the complete compiled
    /// launch; supported ones return it unchanged for the existing trust branch.
    #[test]
    fn no_trust_policy_distinguishes_agent_capability_without_rewriting() {
        for harness in LaunchHarness::ALL.iter().copied() {
            let launch = launches::compile(LaunchSelection {
                harness,
                model: None,
                effort: None,
                permissions: None,
                permission_flags: None,
                wrapper_prefix: None,
                container: None,
                workspace_trust: harness.offers_workspace_trust().then_some(false),
            })
            .unwrap();
            match try_without_workspace_trust(launch.clone()) {
                Ok(resolved) => {
                    assert!(!harness.offers_workspace_trust(), "{harness:?}");
                    assert_eq!(resolved.into_launch(), launch);
                }
                Err(original) => {
                    assert!(harness.offers_workspace_trust(), "{harness:?}");
                    assert_eq!(original, launch);
                }
            }
        }
    }

    /// The approval and launch must use the same selection. User settings
    /// win without claiming automatic trust. The option before a bare `--`
    /// is still effective when later words merely spell `--settings`.
    #[test]
    fn antigravity_trust_preserves_custom_settings_and_positional_words() {
        use farhelm_proto::LaunchPermission;
        use farhelm_proto::session_launch::ANTIGRAVITY_TRUSTED_CWD_PLACEHOLDER as MARKER;

        let config = crate::checkout_config::ResolvedCheckoutConfig {
            keep_spare: false,
            claude_trust: false,
            codex_trust: false,
            muse_trust: false,
            pi_trust: false,
            antigravity_trust: true,
            root: None,
            post_clone: None,
            config_revision: 0,
        };
        for (words, expected) in [
            (vec!["--settings", "{}"], None),
            (vec!["--settings={}"], None),
            (vec!["--", "--settings={}"], Some(true)),
            (vec!["--mode", "auto"], Some(true)),
        ] {
            let mut selection = LaunchSelection {
                harness: LaunchHarness::Antigravity,
                model: None,
                effort: None,
                permissions: Some(LaunchPermission::Custom),
                permission_flags: Some(words.iter().map(ToString::to_string).collect()),
                wrapper_prefix: None,
                container: None,
                workspace_trust: None,
            };
            let bare = launches::compile(selection.clone()).unwrap();
            let resolved = configured_launch_workspace_trust(bare.clone(), &config)
                .unwrap()
                .into_launch();
            assert_eq!(
                resolved.agent_selection().unwrap().workspace_trust,
                expected
            );
            selection.workspace_trust = expected;
            assert_eq!(resolved, launches::compile(selection).unwrap());
            let start = resolved.start_argv().unwrap();
            if expected.is_some() {
                let marker = start.iter().position(|word| word == MARKER).unwrap();
                assert_eq!(start[marker - 1], "--settings");
                assert!(
                    start
                        .iter()
                        .position(|word| word == "--")
                        .is_none_or(|index| marker < index)
                );
            } else {
                assert_eq!(
                    resolved, bare,
                    "the caller's settings survive byte for byte"
                );
            }
            assert_eq!(
                launches::with_workspace_trust(resolved, None).unwrap(),
                bare,
                "clearing the opt-in preserves the complete original start and resume"
            );
        }
    }
}
