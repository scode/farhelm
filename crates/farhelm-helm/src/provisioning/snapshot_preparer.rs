//! Install a bootable supervisor without joining the host registry.
//!
//! The provider owns helper creation and checkpointing. This executor
//! returns install facts only after the enabled installation is stopped and
//! its entire identity/state tree is verified absent. It deliberately has no
//! credential delivery or connection-manager dependency.

use super::backend::{ActionOutcome, ProbeTarget, ProvisioningBackend, ReachOutcome};
use super::installation::{execute_install_action, prepare_payloads};
use super::payloads::PayloadSource;
use super::plan::{
    PlanLayout, ProvisioningAction, ProvisioningOperation, ProvisioningPlan, ProvisioningTarget,
};
use crate::environment::EnvironmentPreparer;
use crate::tensorlake::prepared::PreparedInstallFacts;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

/// A preparation-only composition of the same installer used by host setup.
/// Its backend must use one-shot SSH because recipe preparer names are reused.
pub(super) struct InstallerSnapshotPreparer {
    backend: Arc<dyn ProvisioningBackend>,
    payloads: Arc<dyn PayloadSource>,
    layout: PlanLayout,
    farhelm_version: String,
}

impl InstallerSnapshotPreparer {
    /// Capture already selected seams; environment selection and host services
    /// remain in production assembly, before Tensorlake or the manager starts.
    pub(super) fn new(
        backend: Arc<dyn ProvisioningBackend>,
        payloads: Arc<dyn PayloadSource>,
        layout: PlanLayout,
        farhelm_version: String,
    ) -> Self {
        Self {
            backend,
            payloads,
            layout,
            farhelm_version,
        }
    }

    /// Keep the final guard and facts publication in one operation: callers
    /// never obtain usable clone coordinates after unverified cleanup.
    async fn finish_installation(
        &self,
        plan: ProvisioningPlan,
        home: PathBuf,
    ) -> anyhow::Result<PreparedInstallFacts> {
        self.backend
            .finish_snapshot_install(&plan.target, &self.layout.unit_name, &plan.state_dir)
            .await?;
        Ok(PreparedInstallFacts {
            home,
            farhelm_path: plan.farhelm_path,
            state_dir: plan.state_dir,
            farhelm_version: self.farhelm_version.clone(),
        })
    }
}

#[async_trait]
impl EnvironmentPreparer for InstallerSnapshotPreparer {
    /// Install through the ordinary Add plan, then withhold all facts until
    /// its stopped, boot-enabled installation has passed the checkpoint guard.
    /// Inherited authentication is left alone; no delivery or attach runs here.
    async fn prepare(
        &self,
        route: &crate::access::RemoteRoute,
        home: PathBuf,
    ) -> anyhow::Result<PreparedInstallFacts> {
        let target = ProvisioningTarget::Remote(route.clone());
        let reach = match self
            .backend
            .inspect(&ProbeTarget {
                transport: target.clone(),
                probe_farhelm: "farhelm".into(),
                probe_state_dir: None,
            })
            .await?
        {
            ReachOutcome::Supported(reach) => reach,
            ReachOutcome::Manual(reason) => anyhow::bail!("{reason}"),
            ReachOutcome::SetupManaged => anyhow::bail!(
                "this sandbox has an installation managed by farhelm helm setup; use an image without a Farhelm installation"
            ),
        };
        let plan = self.layout.plan(
            ProvisioningOperation::Add,
            target,
            &reach,
            &uuid::Uuid::new_v4().to_string(),
        )?;
        self.backend
            .refuse_copied_supervisor(&plan.target, &plan.state_dir)
            .await?;
        let prepared = prepare_payloads(self.payloads.as_ref(), &plan)
            .await
            .map_err(|(_, error)| error)?;
        for action in &plan.actions {
            // Attachment would register a host and release credential/tunnel
            // hooks. Linger was already enabled with passwordless sudo by
            // sandbox preparation; repeating the ordinary unprivileged action
            // can fail through polkit even while boot linger is enabled. The
            // final guard verifies actual linger and persistent enablement.
            // The bundled tmux's removal is for hosts an earlier release set
            // up. Preparation installs into a sandbox image with no Farhelm
            // installation (it refuses setup-managed installs and copied
            // state), so the image is not expected to carry such a tmux.
            if matches!(
                action,
                ProvisioningAction::AttachSupervisor
                    | ProvisioningAction::EnableLinger { .. }
                    | ProvisioningAction::RemoveBundledTmux { .. }
            ) {
                continue;
            }
            let outcome =
                execute_install_action(self.backend.as_ref(), &plan, action, prepared.as_ref())
                    .await?;
            if let ActionOutcome::Degraded(reason) = outcome {
                anyhow::bail!("the snapshot installation is not boot-capable: {reason}");
            }
        }
        self.finish_installation(plan, home).await
    }
}

#[cfg(test)]
mod tests {
    use super::super::backend::{CommandLauncher, Reach, SystemBackend};
    use super::super::plan::PayloadTarget;
    use super::*;
    use crate::tensorlake::prepared::{SnapshotPreparer, TensorlakeSnapshotPreparer};
    use crate::tensorlake::ssh::TensorlakeSshAccess;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Sandbox preparation establishes linger before installation. An image
    /// may refuse a later unprivileged loginctl through polkit despite that
    /// enabled state. Preparation must reach its final boot/state guard without
    /// repeating that ordinary-host action or acquiring attachment authority.
    #[farhelm_testtrace::test]
    async fn installation_keeps_the_linger_established_by_sandbox_preparation() {
        use super::super::e2e::{E2ePayloads, E2eProvisioningBackend};

        let root = tempfile::tempdir().unwrap();
        let fixture = root.path().join("backend");
        std::fs::create_dir(&fixture).unwrap();
        let marker = fixture.join("ENABLED");
        std::fs::write(&marker, "farhelm-e2e-provisioning-v1\n").unwrap();
        std::fs::write(
            fixture.join("config.json"),
            r#"{"default":{"fail_action":"enable-linger"}}"#,
        )
        .unwrap();
        let backend = Arc::new(E2eProvisioningBackend::new(fixture.clone(), root.path()).unwrap());
        let target = ProvisioningTarget::Remote(crate::tensorlake::ssh::route(
            crate::tensorlake::ssh::destination("fixture-preparer"),
        ));
        assert!(
            backend.enable_linger(&target).await.is_err(),
            "premise: this image refuses the redundant ordinary linger action"
        );
        std::fs::write(fixture.join("events.jsonl"), "").unwrap();
        let installer = InstallerSnapshotPreparer::new(
            backend.clone(),
            Arc::new(E2ePayloads(marker)),
            PlanLayout::production(root.path().to_path_buf()),
            "fixture-version".into(),
        );
        let preparer =
            TensorlakeSnapshotPreparer::new(backend, Arc::new(installer), root.path()).unwrap();
        let access = TensorlakeSshAccess::for_state_dir(root.path()).unwrap();
        let facts = preparer.prepare("fixture-preparer", &access).await.unwrap();
        assert_eq!(facts.farhelm_version, "fixture-version");
        let events: Vec<serde_json::Value> = std::fs::read_to_string(fixture.join("events.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(events.first().unwrap()["event"], "prepare-sandbox");
        assert_eq!(events.last().unwrap()["event"], "finish-snapshot-install");
        assert!(
            events
                .iter()
                .any(|event| event["event"] == "enable-supervisor")
        );
        assert!(!events.iter().any(|event| {
            matches!(
                event["event"].as_str(),
                Some("enable-linger" | "attach-supervisor")
            )
        }));
    }

    /// Reject any attempt to resolve a payload at the final guard. The
    /// installation has already happened at this boundary.
    #[derive(Debug)]
    struct NoFinalPayloads;
    #[async_trait]
    impl PayloadSource for NoFinalPayloads {
        async fn path(&self, _: PayloadTarget) -> anyhow::Result<PathBuf> {
            panic!("finalization must not resolve payloads")
        }
    }

    /// Fail the real backend's last remote operation after checking its
    /// transport policy. Nothing executes on a service manager or SSH host.
    struct RefusedVerification(AtomicBool);
    impl CommandLauncher for RefusedVerification {
        fn spawn(
            &self,
            command: &mut tokio::process::Command,
        ) -> std::io::Result<tokio::process::Child> {
            let args: Vec<_> = command
                .as_std()
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect();
            assert!(args.iter().any(|arg| arg == "ControlMaster=no"));
            assert!(args.iter().any(|arg| arg == "ControlPath=none"));
            self.0.store(true, Ordering::SeqCst);
            let mut child = tokio::process::Command::new("sh");
            child.args(["-c", "printf 'retained supervisor state' >&2; exit 42"]);
            super::super::ssh_stand_in::spawn_remote(command, child)
        }
    }

    /// A failed final state check must not produce the install facts that
    /// allow the recipe engine to publish a checkpoint. This exercises the
    /// production backend's status conversion, rather than a string search
    /// over the preparer's implementation.
    #[farhelm_testtrace::test]
    async fn failed_verification_returns_no_install_facts() {
        let root = tempfile::tempdir().unwrap();
        let refused = Arc::new(RefusedVerification(AtomicBool::new(false)));
        let mut backend = SystemBackend::one_shot(root.path().to_path_buf());
        backend.launcher = refused.clone();
        let layout = PlanLayout::production(root.path().to_path_buf());
        let reach = Reach {
            home: root.path().join("remote-home"),
            user_unit_dir: root.path().join("remote-units"),
            arch: PayloadTarget::LinuxX86_64,
            distro_id: "fixture".into(),
            launchd_domain: None,
        };
        let plan = layout
            .plan(
                ProvisioningOperation::Add,
                ProvisioningTarget::Remote(crate::tensorlake::ssh::route(
                    crate::tensorlake::ssh::destination("fixture-preparer"),
                )),
                &reach,
                "fixture-nonce",
            )
            .unwrap();
        let preparer = InstallerSnapshotPreparer::new(
            Arc::new(backend),
            Arc::new(NoFinalPayloads),
            layout,
            "fixture-version".into(),
        );
        let result = preparer.finish_installation(plan, reach.home).await;
        assert!(
            refused.0.load(Ordering::SeqCst),
            "the final remote guard must execute"
        );
        let failure = result.expect_err("failed verification must withhold install facts");
        assert!(
            failure.to_string().contains("retained supervisor state"),
            "{failure:#}"
        );
    }
}
