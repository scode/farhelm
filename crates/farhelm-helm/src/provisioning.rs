//! Discovery-first supervisor provisioning and its host-scoped REST state.
//!
//! Provisioning is a convergence operation, not an installer transaction.
//! The confirmed [`plan::ProvisioningPlan`] is the only description of the work:
//! the confirmation renderer walks its actions, and the executor walks the
//! same actions after confirmation. A failed action leaves every completed
//! action in place so rerunning can resume from content and hash comparisons.
//!
//! Transport is deliberately below that plan. Local setup executes and
//! copies directly; remote setup uses the user's `ssh` and `sftp`, sharing
//! the option-safe SSH prefix with the steady-state connection manager.

/// The release asset inventory (plan §1) — the archive and binary names a
/// GitHub release publishes. Drives every Rust payload source directly
/// (this module's own docstring is the authoritative account of who reads
/// it, and how later non-Rust consumers stay aligned with it instead).
pub mod assets;
mod backend;
mod e2e;
mod http;
mod payloads;
mod plan;
mod release_payloads;
mod service;

pub use backend::{LocalSupervisorDiscovery, discover_local_supervisor};
pub(crate) use http::{probe_host, provision_host, provisioning_state, update_host};
pub(crate) use payloads::PayloadSelection;
pub(crate) use service::ProvisioningService;

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    use super::assets::*;
    use super::backend::*;
    #[allow(unused_imports)]
    use super::e2e::*;
    use super::http::*;
    use super::payloads::*;
    use super::plan::*;
    // The loopback release fixture lives with the download source it was
    // built for; this module borrows it for the one end-to-end test that
    // drives a real download through a real provisioning run.
    use super::release_payloads::test_support::{FixtureRelease, expected_member};
    #[allow(unused_imports)]
    use super::service::*;
    use crate::AppState;
    use crate::manager::{ConnectionManager, HostState};
    use crate::rest_harness::{FleetBuilder, Harness, HostScript};
    use crate::store::{DialedAs, HelmStore, HostId, HostKind};
    use anyhow::{Context as _, bail};
    use async_trait::async_trait;
    use axum::body::{Body, to_bytes};
    use axum::extract::{Path as AxPath, State};
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use farhelm_proto::ControlMsg;
    use std::collections::VecDeque;
    use std::path::{Component, Path, PathBuf};
    use std::process::Stdio;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    use tower::ServiceExt;

    /// A payload source whose bytes are irrelevant to the fake executor.
    #[derive(Debug)]
    struct FixedPayloads(PathBuf);

    #[async_trait]
    impl PayloadSource for FixedPayloads {
        async fn path(&self, _payload: PayloadKind, _arch: PayloadArch) -> anyhow::Result<PathBuf> {
            Ok(self.0.clone())
        }
    }

    /// A release fixture with Farhelm present but the required tmux artifact
    /// missing, used to prove whole-plan payload preflight.
    #[derive(Debug)]
    struct MissingTmuxPayload {
        farhelm: PathBuf,
        requested: Mutex<Vec<PayloadKind>>,
    }

    #[async_trait]
    impl PayloadSource for MissingTmuxPayload {
        async fn path(&self, payload: PayloadKind, _arch: PayloadArch) -> anyhow::Result<PathBuf> {
            self.requested.lock().unwrap().push(payload);
            match payload {
                PayloadKind::Farhelm => Ok(self.farhelm.clone()),
                PayloadKind::Tmux => bail!("the tmux payload is missing"),
            }
        }
    }

    struct ScriptLauncher {
        scripts: Mutex<VecDeque<String>>,
        programs: Mutex<Vec<String>>,
    }

    impl ScriptLauncher {
        fn new(scripts: impl IntoIterator<Item = String>) -> Arc<Self> {
            Arc::new(Self {
                scripts: Mutex::new(scripts.into_iter().collect()),
                programs: Mutex::new(Vec::new()),
            })
        }
    }

    impl CommandLauncher for ScriptLauncher {
        fn spawn(
            &self,
            command: &mut tokio::process::Command,
        ) -> std::io::Result<tokio::process::Child> {
            self.programs.lock().unwrap().push(
                command
                    .as_std()
                    .get_program()
                    .to_string_lossy()
                    .into_owned(),
            );
            let script = self
                .scripts
                .lock()
                .unwrap()
                .pop_front()
                .expect("one scripted child per launch");
            let mut scripted = tokio::process::Command::new("sh");
            scripted
                .args(["-c", &script])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            isolate_process_group(&mut scripted);
            scripted.spawn()
        }
    }

    struct RecordingLauncher {
        programs: Mutex<Vec<String>>,
    }

    impl RecordingLauncher {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                programs: Mutex::new(Vec::new()),
            })
        }
    }

    impl CommandLauncher for RecordingLauncher {
        fn spawn(
            &self,
            command: &mut tokio::process::Command,
        ) -> std::io::Result<tokio::process::Child> {
            self.programs.lock().unwrap().push(
                command
                    .as_std()
                    .get_program()
                    .to_string_lossy()
                    .into_owned(),
            );
            command.spawn()
        }
    }

    fn frame_script(frame: farhelm_proto::Frame) -> String {
        let mut bytes = Vec::new();
        frame.encode(&mut bytes).expect("encode scripted frame");
        let octal = bytes
            .iter()
            .map(|byte| format!("\\{byte:03o}"))
            .collect::<String>();
        format!("printf '{octal}'")
    }

    fn test_system_backend(root: &Path) -> SystemBackend {
        SystemBackend {
            control_dir: root.to_path_buf(),
            linger: LingerBehavior::Simulated(Ok(())),
            launcher: Arc::new(SystemLauncher),
            runtime_units: false,
            fail_before_rename: false,
        }
    }

    /// Exercise the production install path with the same immutable payload
    /// snapshot the orchestration layer supplies during a real run.
    async fn install_test_payload(
        backend: &SystemBackend,
        target: &ProvisioningTarget,
        source: &Path,
        destination: &Path,
        temporary: &Path,
        mode: u32,
    ) -> Result<ActionOutcome, BackendFailure> {
        let prepared = stage_payload(source).await?;
        backend
            .install_path(
                target,
                PayloadKind::Farhelm,
                &prepared,
                destination,
                temporary,
                mode,
            )
            .await
    }

    /// Recorder for orchestration tests, including planted failure, linger,
    /// registration-order, and in-flight barriers.
    struct FakeBackend {
        probe: Mutex<Option<Result<ProbeObservation, BackendFailure>>>,
        reach: Mutex<ReachOutcome>,
        /// What this machine's `systemd/user` directory holds for the unit
        /// the local-row rule asks about. `Ok(None)` is "no such file",
        /// which is the only state most tests care about; `Err` stands in
        /// for the lookup failures — permission, undecodable contents, an
        /// unlocatable directory — that must never read as absence.
        user_unit: Mutex<Result<Option<String>, String>>,
        /// Every unit name the service asked for, in order. Lets a test
        /// prove WHICH unit the ownership rule consults, and that paths
        /// which have no business reading local state never do.
        unit_reads: Mutex<Vec<String>>,
        inspect_failure: Mutex<Option<String>>,
        operations: Mutex<Vec<String>>,
        fail: Mutex<Option<String>>,
        skip: Mutex<Option<String>>,
        linger: Mutex<Option<Result<(), String>>>,
        registration: Mutex<Option<(HelmStore, String)>>,
        entered: tokio::sync::Notify,
        release: tokio::sync::Notify,
        block_first: std::sync::atomic::AtomicBool,
        stateful: std::sync::atomic::AtomicBool,
    }

    impl FakeBackend {
        fn absent(home: PathBuf) -> Arc<Self> {
            Arc::new(Self {
                probe: Mutex::new(Some(Ok(ProbeObservation::Absent))),
                reach: Mutex::new(ReachOutcome::Supported(Reach {
                    user_unit_dir: home.join(".config/systemd/user"),
                    home,
                    arch: PayloadArch::X86_64,
                    distro_id: "ubuntu".to_string(),
                    needs_tmux: false,
                    host_tmux: Some(PathBuf::from("/usr/bin/tmux")),
                })),
                user_unit: Mutex::new(Ok(None)),
                unit_reads: Mutex::new(Vec::new()),
                inspect_failure: Mutex::new(None),
                operations: Mutex::new(Vec::new()),
                fail: Mutex::new(None),
                skip: Mutex::new(None),
                linger: Mutex::new(Some(Ok(()))),
                registration: Mutex::new(None),
                entered: tokio::sync::Notify::new(),
                release: tokio::sync::Notify::new(),
                block_first: std::sync::atomic::AtomicBool::new(false),
                stateful: std::sync::atomic::AtomicBool::new(false),
            })
        }

        fn supervisor(home: PathBuf) -> Arc<Self> {
            let backend = Self::absent(home.clone());
            *backend.probe.lock().unwrap() = Some(Ok(ProbeObservation::Supervisor {
                build_version: "test-build".to_string(),
                host_identity: Some("test-identity".to_string()),
                dial_farhelm: home.join("farhelm"),
                dial_state_dir: Some(home.join("state")),
            }));
            backend
        }

        /// A probe answer shaped like a live-but-older supervisor: present,
        /// build known from the skew payload, identity unknowable. What the
        /// UPDATE flow must accept — see the skewed-supervisor tests.
        fn skewed(home: PathBuf) -> Arc<Self> {
            let backend = Self::absent(home.clone());
            *backend.probe.lock().unwrap() = Some(Ok(ProbeObservation::SkewedSupervisor {
                peer_build: "0.1.1-old".to_string(),
                dial_farhelm: home.join("farhelm"),
                dial_state_dir: Some(home.join("state")),
            }));
            backend
        }

        fn failing_probe(home: PathBuf, message: &str) -> Arc<Self> {
            let backend = Self::absent(home);
            *backend.probe.lock().unwrap() =
                Some(Err(BackendFailure::new("the ssh probe failed", message)));
            backend
        }

        fn manual(home: PathBuf, reason: &str) -> Arc<Self> {
            let backend = Self::absent(home);
            *backend.reach.lock().unwrap() = ReachOutcome::Manual(reason.to_string());
            backend
        }

        fn record(&self, operation: &str) -> Result<ActionOutcome, BackendFailure> {
            self.operations.lock().unwrap().push(operation.to_string());
            if self.fail.lock().unwrap().as_deref() == Some(operation) {
                return Err(BackendFailure::new(
                    format!("planted failure in {operation}"),
                    "\u{1b}[31mhost said no\nnext",
                ));
            }
            if self.skip.lock().unwrap().as_deref() == Some(operation) {
                return Ok(ActionOutcome::Skipped(format!(
                    "planted skip in {operation}"
                )));
            }
            Ok(ActionOutcome::Completed)
        }
    }

    #[async_trait]
    impl ProvisioningBackend for FakeBackend {
        async fn probe(&self, _target: &ProbeTarget) -> Result<ProbeObservation, BackendFailure> {
            self.probe
                .lock()
                .unwrap()
                .take()
                .unwrap_or(Ok(ProbeObservation::Absent))
        }

        async fn inspect(&self, _target: &ProbeTarget) -> Result<ReachOutcome, BackendFailure> {
            if let Some(message) = self.inspect_failure.lock().unwrap().take() {
                return Err(BackendFailure::new("planted inspection failure", message));
            }
            Ok(self.reach.lock().unwrap().clone())
        }

        async fn ensure_directories(
            &self,
            _target: &ProvisioningTarget,
            directories: &[DirectorySpec],
        ) -> Result<ActionOutcome, BackendFailure> {
            let registration = self.registration.lock().unwrap().take();
            if let Some((store, destination)) = registration {
                let registered = store
                    .list_hosts()
                    .await
                    .unwrap()
                    .iter()
                    .any(|row| row.destination.as_deref() == Some(destination.as_str()));
                assert!(
                    registered,
                    "the host row must exist before the first action"
                );
            }
            if self.stateful.load(std::sync::atomic::Ordering::SeqCst) {
                for directory in directories {
                    tokio::fs::create_dir_all(&directory.path)
                        .await
                        .map_err(|error| {
                            BackendFailure::new(
                                format!("creating stateful fixture {}", directory.path.display()),
                                error.to_string(),
                            )
                        })?;
                    set_mode(&directory.path, directory.mode).await?;
                }
            }
            let result = self.record("create-directories")?;
            if self
                .block_first
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                self.entered.notify_one();
                self.release.notified().await;
            }
            Ok(result)
        }

        async fn install_path(
            &self,
            _target: &ProvisioningTarget,
            kind: PayloadKind,
            payload: &PreparedPayload,
            destination: &Path,
            _temporary: &Path,
            _mode: u32,
        ) -> Result<ActionOutcome, BackendFailure> {
            if self.stateful.load(std::sync::atomic::Ordering::SeqCst) {
                tokio::fs::copy(payload.path(), destination)
                    .await
                    .map_err(|error| {
                        BackendFailure::new(
                            format!("installing stateful fixture {}", destination.display()),
                            error.to_string(),
                        )
                    })?;
            }
            self.record(match kind {
                PayloadKind::Farhelm => "install-farhelm",
                PayloadKind::Tmux => "install-tmux",
            })
        }

        async fn install_bytes(
            &self,
            _target: &ProvisioningTarget,
            content: &[u8],
            destination: &Path,
            _temporary: &Path,
            _mode: u32,
        ) -> Result<ActionOutcome, BackendFailure> {
            if self.stateful.load(std::sync::atomic::Ordering::SeqCst) {
                tokio::fs::write(destination, content)
                    .await
                    .map_err(|error| {
                        BackendFailure::new(
                            format!("writing stateful fixture {}", destination.display()),
                            error.to_string(),
                        )
                    })?;
            }
            self.record("write-unit")
        }

        async fn daemon_reload(
            &self,
            _target: &ProvisioningTarget,
        ) -> Result<ActionOutcome, BackendFailure> {
            self.record("daemon-reload")
        }

        async fn enable_now(
            &self,
            _target: &ProvisioningTarget,
            _unit: &str,
            _unit_path: &Path,
        ) -> Result<ActionOutcome, BackendFailure> {
            self.record("enable-supervisor")
        }

        async fn enable_linger(
            &self,
            _target: &ProvisioningTarget,
        ) -> Result<ActionOutcome, BackendFailure> {
            self.record("enable-linger")?;
            match self.linger.lock().unwrap().take().unwrap_or(Ok(())) {
                Ok(()) => Ok(ActionOutcome::Completed),
                Err(message) => Ok(ActionOutcome::Degraded(format!(
                    "linger was refused ({message}); starts at login, not at boot"
                ))),
            }
        }

        async fn restart(
            &self,
            _target: &ProvisioningTarget,
            _unit: &str,
        ) -> Result<ActionOutcome, BackendFailure> {
            self.record("restart-supervisor")
        }

        async fn read_user_unit(&self, name: &str) -> Result<Option<String>, BackendFailure> {
            self.unit_reads.lock().unwrap().push(name.to_string());
            self.user_unit
                .lock()
                .unwrap()
                .clone()
                .map_err(|message| BackendFailure::new(message, ""))
        }
    }

    /// The reserved local registry row, which every store mints on open.
    async fn local_row(harness: &Harness) -> HostId {
        harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.kind == HostKind::Local)
            .expect("the reserved local row always exists")
            .id
    }

    /// Build an isolated scripted helm whose local actor can reconnect after
    /// provisioning's attach step. No process, unit, or real user path is
    /// involved in these contract tests.
    async fn harness() -> Harness {
        let harness = FleetBuilder::new()
            .await
            .local(HostScript::default())
            .await
            .start()
            .await;
        let local = harness.store.list_hosts().await.unwrap()[0].id;
        harness.await_refreshed(local).await;
        harness
    }

    fn layout(root: &Path) -> PlanLayout {
        PlanLayout {
            local_state_dir: root.join("state"),
            override_lib_dir: Some(root.join("lib")),
            override_farhelm_path: None,
            override_state_dir: Some(root.join("state")),
            override_unit_dir: Some(root.join("units")),
            unit_name: format!("farhelm-provisioning-test-{}.service", uuid::Uuid::new_v4()),
        }
    }

    fn service(
        harness: &Harness,
        backend: Arc<FakeBackend>,
        root: &Path,
    ) -> Arc<ProvisioningService> {
        let payload = root.join("payload");
        std::fs::write(&payload, b"farhelm test payload").expect("write fixed payload fixture");
        service_with_payloads(harness, backend, root, Arc::new(FixedPayloads(payload)))
    }

    /// The same wiring as [`service`] with the payload source supplied —
    /// for the one test that drives a REAL source (the verified downloader)
    /// through the service instead of a double.
    fn service_with_payloads(
        harness: &Harness,
        backend: Arc<FakeBackend>,
        root: &Path,
        payloads: Arc<dyn PayloadSource>,
    ) -> Arc<ProvisioningService> {
        ProvisioningService::injected(
            harness.store.clone(),
            Arc::clone(&harness.manager),
            backend,
            payloads,
            layout(root),
            PathBuf::from("/test/farhelm"),
        )
    }

    async fn wait_finished(service: &ProvisioningService, host: HostId) -> ProvisioningView {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let view = service.view(host).await.unwrap();
                if matches!(view.status, RunStatus::Completed | RunStatus::Failed) {
                    return view;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("provisioning run did not finish")
    }

    /// A successful discovery is use-as-is: it creates the SSH registry row
    /// and returns the peer's hello without constructing an install plan.
    #[farhelm_testtrace::test]
    async fn probe_registers_an_answering_supervisor_as_is() {
        let (builder, expected_host) = FleetBuilder::new()
            .await
            .ssh(
                "user@example",
                HostScript {
                    reachable: false,
                    identity: Some("test-identity".to_string()),
                    ..HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness
            .await_state(expected_host, |state| {
                matches!(state, HostState::Unreachable { .. })
            })
            .await;
        harness.fleet.edit(expected_host, |script| {
            script.reachable = true;
        });
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::supervisor(root.path().to_path_buf());
        let provisioner = service(&harness, backend.clone(), root.path());
        let response = provisioner
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "user@example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        assert!(matches!(
            response,
            ProbeResponse::Discovered {
                host_id,
                build_version,
                host_identity: Some(identity),
                ..
            } if host_id == expected_host
                && build_version == "test-build"
                && identity == "test-identity"
        ));
        assert!(
            harness
                .store
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .any(|row| row.destination.as_deref() == Some("user@example"))
        );
        let row = harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.destination.as_deref() == Some("user@example"))
            .unwrap();
        assert_eq!(
            row.remote_farhelm.as_deref(),
            Some(root.path().join("farhelm").to_str().unwrap())
        );
        assert_eq!(
            row.remote_state_dir.as_deref(),
            Some(root.path().join("state").to_str().unwrap())
        );
        assert_eq!(row.host_identity.as_deref(), Some("test-identity"));
        harness
            .await_refreshed_as(expected_host, "test-identity", 0)
            .await;
        let client = harness
            .manager
            .status(expected_host)
            .and_then(|status| status.client)
            .expect("discovery must leave an operable manager client");
        assert!(client.list_sessions().await.unwrap().sessions.is_empty());
        assert!(backend.operations.lock().unwrap().is_empty());
    }

    /// Positive absence may offer a plan, but probing itself must not create
    /// a registry row, directory, unit, or payload transfer.
    #[farhelm_testtrace::test]
    async fn positive_absence_returns_a_concrete_plan_without_touching_the_host() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());
        let before = harness.store.list_hosts().await.unwrap();
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "user@absent".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable {
            probe_id,
            plan,
            confirmation,
        } = response
        else {
            panic!("positive absence must offer provisioning")
        };
        assert_eq!(harness.store.list_hosts().await.unwrap(), before);
        assert!(backend.operations.lock().unwrap().is_empty());
        assert!(confirmation.contains("starts at boot if linger succeeds"));
        assert!(confirmation.contains("starts at login, not at boot"));
        let ProvisioningAction::WriteUnit { unit, .. } = &plan.actions[2] else {
            panic!("the third action must write the unit")
        };
        let farhelm = root.path().join("lib/farhelm");
        let unit_path = root.path().join("units").join(unit);
        let expected = format!(
            "Farhelm will perform these steps for user@absent:\n\
             host: ubuntu, x86_64\n\
             1. create or reuse directories {} (mode 0755), {} (mode 0700), {} (mode 0755)\n\
             2. install Farhelm at {} via temporary file {} and atomic rename\n\
             3. write user unit {unit} at {} via temporary file {} and atomic rename\n\
             4. reload the systemd user manager\n\
             5. enable and start {unit}; the supervisor runs persistently under the systemd user manager\n\
             6. optionally enable linger: the supervisor starts at boot if linger succeeds; if privilege is refused, continue and report that it starts at login, not at boot\n\
             7. dial the supervisor and attach it to the already-registered host row\n",
            root.path().join("lib").display(),
            root.path().join("state").display(),
            root.path().join("units").display(),
            farhelm.display(),
            root.path()
                .join(format!("lib/.farhelm.farhelm-tmp-{probe_id}"))
                .display(),
            unit_path.display(),
            root.path()
                .join("units")
                .join(format!(".{unit}.farhelm-tmp-{probe_id}"))
                .display(),
        );
        assert_eq!(confirmation, expected);
    }

    /// Unsupported hosts stay on the manual-run path without retaining
    /// confirmation authority or touching either registry or executor.
    #[farhelm_testtrace::test]
    async fn manual_fallback_retains_nothing_and_mutates_nothing() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::manual(root.path().to_path_buf(), "unsupported fixture");
        let service = service(&harness, backend.clone(), root.path());
        let before = harness.store.list_hosts().await.unwrap();

        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "manual.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();

        assert!(matches!(
            response,
            ProbeResponse::Manual { reason } if reason == "unsupported fixture"
        ));
        assert_eq!(harness.store.list_hosts().await.unwrap(), before);
        assert!(backend.operations.lock().unwrap().is_empty());
        let memory = service.memory.lock().await;
        assert!(memory.plans.is_empty());
        assert!(memory.runs.is_empty());
        assert!(memory.busy.is_empty());
    }

    /// A development build resolves its absent payload before directories or
    /// any other host state changes, then releases the host for another run.
    #[farhelm_testtrace::test]
    async fn no_payloads_fails_before_the_first_mutation_and_releases_the_host() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = ProvisioningService::injected(
            harness.store.clone(),
            Arc::clone(&harness.manager),
            backend.clone(),
            Arc::new(NoPayloads),
            layout(root.path()),
            PathBuf::from("/test/farhelm"),
        );

        for attempt in 0..2 {
            let response = service
                .probe(ProbeRequest {
                    target: ProbeDestination::Ssh {
                        destination: "payloads.example".to_string(),
                    },
                    remote_farhelm: None,
                    remote_state_dir: None,
                })
                .await
                .unwrap();
            let ProbeResponse::Provisionable { probe_id, .. } = response else {
                panic!("attempt {attempt} must retain a plan")
            };
            let accepted = service
                .start_add(ProvisionRequest { probe_id })
                .await
                .expect("the released host must admit the retry");
            let failed = wait_finished(&service, accepted.host_id).await;
            assert_eq!(failed.status, RunStatus::Failed);
            assert!(failed.message.as_deref().is_some_and(|message| {
                message.contains(
                    "this farhelm was built from source and carries no provisioning payloads",
                )
            }));
            assert_eq!(failed.steps[0].status, StepStatus::Pending);
            assert_eq!(failed.steps[1].status, StepStatus::Failed);
            assert!(backend.operations.lock().unwrap().is_empty());
            assert!(service.memory.lock().await.busy.is_empty());
        }
    }

    /// Preflight visits every required payload before mutating, even when an
    /// earlier payload exists and only the private-tmux artifact is missing.
    #[farhelm_testtrace::test]
    async fn payload_preflight_validates_the_complete_required_set() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let farhelm = root.path().join("farhelm-payload");
        tokio::fs::write(&farhelm, b"farhelm").await.unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        {
            let mut reach = backend.reach.lock().unwrap();
            let ReachOutcome::Supported(reach) = &mut *reach else {
                panic!("fixture must be supported")
            };
            reach.needs_tmux = true;
            reach.host_tmux = None;
        }
        let payloads = Arc::new(MissingTmuxPayload {
            farhelm,
            requested: Mutex::new(Vec::new()),
        });
        let service = ProvisioningService::injected(
            harness.store.clone(),
            Arc::clone(&harness.manager),
            backend.clone(),
            Arc::clone(&payloads) as Arc<dyn PayloadSource>,
            layout(root.path()),
            PathBuf::from("/test/farhelm"),
        );
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "preflight.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("missing tmux still yields a confirmable plan")
        };
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        let failed = wait_finished(&service, accepted.host_id).await;
        assert_eq!(failed.status, RunStatus::Failed);
        assert!(failed.message.unwrap().contains("tmux payload is missing"));
        assert_eq!(
            payloads.requested.lock().unwrap().as_slice(),
            [PayloadKind::Farhelm, PayloadKind::Tmux]
        );
        assert!(backend.operations.lock().unwrap().is_empty());
    }

    /// A failed post-insert actor reconciliation rolls back only the new row,
    /// leaving the consumed confirmation recoverable through a fresh probe.
    #[farhelm_testtrace::test]
    async fn failed_registry_sync_rolls_back_a_newly_registered_host() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "rollback.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("absence must produce a plan")
        };
        service
            .fail_registry_sync
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let error = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .expect_err("the planted reconciliation failure must surface");
        assert!(
            error
                .to_string()
                .contains("planted registry synchronization failure")
        );
        assert!(
            harness
                .store
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .all(|row| row.destination.as_deref() != Some("rollback.example"))
        );
        assert!(backend.operations.lock().unwrap().is_empty());

        let retry = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "rollback.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        assert!(matches!(retry, ProbeResponse::Provisionable { .. }));
    }

    /// Confirming an already-registered but absent SSH host replaces stale
    /// dial coordinates with the installation paths before attachment.
    #[farhelm_testtrace::test]
    async fn confirmed_add_repairs_stale_registered_paths() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let host = harness
            .store
            .add_ssh_host(
                "stale.example",
                Some("/stale/farhelm"),
                Some("/stale/state"),
            )
            .await
            .unwrap();
        harness.manager.sync_registry().await.unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend, root.path());
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "stale.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("the stopped registered host must offer recovery")
        };
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        assert_eq!(accepted.host_id, host);
        assert_eq!(
            wait_finished(&service, host).await.status,
            RunStatus::Completed
        );
        let repaired = service.host_row(host).await.unwrap();
        assert_eq!(
            repaired.remote_farhelm.as_deref(),
            root.path().join("lib/farhelm").to_str()
        );
        assert_eq!(
            repaired.remote_state_dir.as_deref(),
            root.path().join("state").to_str()
        );
    }

    /// A remote literally named `local` remains an SSH destination; transport
    /// choice comes from the request tag rather than hostname spelling.
    #[farhelm_testtrace::test]
    async fn local_hostname_does_not_alias_the_local_transport() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend, root.path());
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "local".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { plan, .. } = response else {
            panic!("the absent SSH host must produce a plan")
        };
        assert!(matches!(
            plan.target,
            ProvisioningTarget::Ssh { destination } if destination == "local"
        ));
    }

    /// UPDATE previews the installation coordinates already stored on the
    /// row and leaves both registry and host untouched until confirmation.
    #[farhelm_testtrace::test]
    async fn update_plan_uses_registered_paths_without_mutation() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let farhelm = root.path().join("custom/bin/farhelm");
        let state_dir = root.path().join("custom/state");
        let host = harness
            .store
            .add_ssh_host("custom.example", farhelm.to_str(), state_dir.to_str())
            .await
            .unwrap();
        harness.manager.sync_registry().await.unwrap();
        let before = harness.store.list_hosts().await.unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());

        let preview = service.plan_update(host).await.unwrap();

        assert_eq!(preview.plan.farhelm_path, farhelm);
        assert_eq!(preview.plan.state_dir, state_dir);
        assert_eq!(harness.store.list_hosts().await.unwrap(), before);
        assert!(backend.operations.lock().unwrap().is_empty());
    }

    /// UPDATE against a skewed supervisor — the host left behind by a
    /// protocol bump — must plan and execute, with the recorded identity
    /// carried forward unverified rather than treated as a mismatch: the
    /// skew refusal happens before identity exchange, so demanding a match
    /// would make the one host UPDATE exists for permanently un-updatable.
    /// The plan targets the dial coordinates the skewed probe resolved, and
    /// the run completes end to end (plan → confirm-time revalidation, which
    /// sees the host STILL skewed → execution). Regression test for the
    /// 2026-09-01 field failure where the skew refusal aborted planning as
    /// a probe error.
    #[farhelm_testtrace::test]
    async fn update_plans_and_executes_against_a_skewed_supervisor() {
        let (builder, host) = FleetBuilder::new()
            .await
            .ssh(
                "skewed.example",
                HostScript {
                    identity: Some("recorded-identity".to_string()),
                    ..HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        // A recorded identity is the interesting precondition: it is what a
        // naive "identity must match" rule would wrongly compare against
        // the skewed probe's nothing.
        harness
            .await_refreshed_as(host, "recorded-identity", 0)
            .await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::skewed(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());

        let preview = service.plan_update(host).await.unwrap();
        assert_eq!(preview.plan.farhelm_path, root.path().join("farhelm"));
        assert_eq!(preview.plan.state_dir, root.path().join("state"));
        assert!(
            backend.operations.lock().unwrap().is_empty(),
            "planning must not mutate the host"
        );

        // Confirmation reprobes; the host is still skewed — the expected
        // state for an update that has not run yet.
        *backend.probe.lock().unwrap() = Some(Ok(ProbeObservation::SkewedSupervisor {
            peer_build: "0.1.1-old".to_string(),
            dial_farhelm: root.path().join("farhelm"),
            dial_state_dir: Some(root.path().join("state")),
        }));
        service
            .start_update(
                host,
                ProvisionRequest {
                    probe_id: preview.probe_id,
                },
            )
            .await
            .unwrap();
        assert_eq!(
            wait_finished(&service, host).await.status,
            RunStatus::Completed
        );
        assert!(
            !backend.operations.lock().unwrap().is_empty(),
            "the update run must actually execute its plan"
        );
        let row = harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        assert_eq!(
            row.host_identity.as_deref(),
            Some("recorded-identity"),
            "an unverifiable skewed probe must neither clear nor replace the recorded identity"
        );
    }

    /// ADD discovery of a skewed supervisor registers the host instead of
    /// erroring or installing over it: something live owns that state
    /// directory, and once registered the host carries the manager's
    /// version-skew state and the update action that fixes it. Identity is
    /// recorded as unknown — the refusal precedes identity exchange.
    #[farhelm_testtrace::test]
    async fn probe_registers_a_skewed_supervisor_for_update() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::skewed(root.path().to_path_buf());
        let provisioner = service(&harness, backend.clone(), root.path());
        let response = provisioner
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "user@skewed.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        assert!(matches!(
            &response,
            ProbeResponse::Discovered {
                build_version,
                host_identity: None,
                ..
            } if build_version == "0.1.1-old"
        ));
        let row = harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.destination.as_deref() == Some("user@skewed.example"))
            .expect("the skewed supervisor must be registered");
        assert_eq!(
            row.remote_farhelm.as_deref(),
            Some(root.path().join("farhelm").to_str().unwrap())
        );
        assert_eq!(row.host_identity, None);
        assert!(
            backend.operations.lock().unwrap().is_empty(),
            "discovery must not install over a live supervisor"
        );
    }

    /// A supervisor that turns SKEWED between ADD confirmation and
    /// execution — planned against absence, live-but-older by confirm time —
    /// must be registered and adopted rather than installed over: something
    /// live owns that state directory, and the finished run's message points
    /// the operator at the update action, the one thing that fixes skew.
    /// Covers the confirmation-revalidation arm the other skew tests do not
    /// reach.
    #[farhelm_testtrace::test]
    async fn add_confirmation_registers_a_supervisor_that_turned_skewed() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "turned-skewed.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("an absent host must yield an install plan")
        };
        *backend.probe.lock().unwrap() = Some(Ok(ProbeObservation::SkewedSupervisor {
            peer_build: "0.1.1-old".to_string(),
            dial_farhelm: root.path().join("farhelm"),
            dial_state_dir: Some(root.path().join("state")),
        }));
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        let finished = wait_finished(&service, accepted.host_id).await;
        assert_eq!(finished.status, RunStatus::Completed);
        assert!(
            finished
                .message
                .as_deref()
                .is_some_and(|message| message.contains("speaks another protocol")
                    && message.contains("update")),
            "the outcome must say what was found and what fixes it: {:?}",
            finished.message
        );
        assert!(
            backend.operations.lock().unwrap().is_empty(),
            "confirmation must not install over the live skewed supervisor"
        );
        let row = harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.destination.as_deref() == Some("turned-skewed.example"))
            .expect("the skewed supervisor must be registered");
        assert_eq!(row.host_identity, None);
    }

    /// An accepted HOST tmux must survive a stale private one sitting in
    /// Farhelm's own lib directory.
    ///
    /// This is the exact shape of a live shadowing bug. When the host's
    /// tmux clears the floor, provisioning deliberately skips installing
    /// the private payload — but the unit still puts Farhelm's lib
    /// directory first on PATH, so an obsolete `tmux` left there by an
    /// earlier install would win the name lookup. Provisioning would then
    /// report success, decline to replace that binary, and restart the
    /// supervisor straight onto it: a below-floor substrate reached
    /// through the ordinary upgrade path, with nothing in the run record
    /// suggesting anything went wrong.
    ///
    /// So the plan is required to NAME the accepted executable
    /// (`FARHELM_TMUX`) rather than describe where to look for one, and
    /// to still install no tmux payload. The stale file on disk is a
    /// fixture, not an input — the fix must hold whether or not planning
    /// ever looks at the lib directory, which is why the assertion is on
    /// the frozen unit text.
    #[farhelm_testtrace::test]
    async fn update_pins_the_accepted_host_tmux_over_a_stale_private_one() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let lib_dir = root.path().join(".local/lib/farhelm");
        tokio::fs::create_dir_all(&lib_dir).await.unwrap();
        tokio::fs::write(lib_dir.join("tmux"), b"an obsolete private tmux")
            .await
            .unwrap();

        let host = harness
            .store
            .add_ssh_host("stale-tmux.example", None, None)
            .await
            .unwrap();
        harness.manager.sync_registry().await.unwrap();
        // FakeBackend::absent reports a host tmux at /usr/bin/tmux that
        // already cleared the floor, which is what makes the payload
        // unnecessary and the shadow possible.
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());

        let preview = service.plan_update(host).await.unwrap();

        let unit = preview
            .plan
            .actions
            .iter()
            .find_map(|action| match action {
                ProvisioningAction::WriteUnit { content, .. } => Some(content.clone()),
                _ => None,
            })
            .expect("every plan writes the supervisor unit");
        assert!(
            unit.contains("Environment=\"FARHELM_TMUX=/usr/bin/tmux\""),
            "the unit must drive the accepted host tmux, not whatever PATH finds: {unit}"
        );
        assert!(
            !preview.plan.actions.iter().any(|action| matches!(
                action,
                ProvisioningAction::InstallPayload {
                    payload: PayloadKind::Tmux,
                    ..
                }
            )),
            "an accepted host tmux must still skip the private payload"
        );
    }

    /// Manual and backend planning failures retain no UPDATE claim; once
    /// inspection recovers, the same host can immediately produce a plan.
    #[farhelm_testtrace::test]
    async fn update_planning_failures_release_the_host_for_retry() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        // An SSH row: UPDATE on the reserved LOCAL row is refused outright
        // now, which would mask the retry behaviour under test.
        let host = harness
            .store
            .add_ssh_host("retry.example", None, None)
            .await
            .unwrap();
        harness.manager.sync_registry().await.unwrap();
        let backend = FakeBackend::manual(root.path().to_path_buf(), "manual only");
        let service = service(&harness, backend.clone(), root.path());

        assert!(service.plan_update(host).await.is_err());
        let idle = service.view(host).await.unwrap();
        assert_eq!(idle.status, RunStatus::Completed);
        assert!(idle.run_id.is_none());
        *backend.reach.lock().unwrap() = ReachOutcome::Supported(Reach {
            home: root.path().to_path_buf(),
            user_unit_dir: root.path().join("units"),
            arch: PayloadArch::X86_64,
            distro_id: "ubuntu".to_string(),
            needs_tmux: false,
            host_tmux: Some(PathBuf::from("/usr/bin/tmux")),
        });
        *backend.inspect_failure.lock().unwrap() = Some("inspection broke".to_string());
        assert!(service.plan_update(host).await.is_err());
        assert!(service.memory.lock().await.busy.is_empty());

        let recovered = service.plan_update(host).await.unwrap();
        assert!(!recovered.probe_id.is_empty());
    }

    /// UPDATE is not an identity-resolution mechanism: both frozen mismatch
    /// and duplicate rows are refused before host inspection begins.
    #[farhelm_testtrace::test]
    async fn update_refuses_identity_freeze_states() {
        let (builder, mismatch_host) = FleetBuilder::new()
            .await
            .ssh(
                "mismatch.example",
                HostScript {
                    identity: Some("identity-old".to_string()),
                    ..HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness
            .await_refreshed_as(mismatch_host, "identity-old", 0)
            .await;
        let harness = harness
            .restart_with(|fleet| {
                fleet.edit(mismatch_host, |script| {
                    script.identity = Some("identity-new".to_string());
                });
            })
            .await;
        harness
            .await_state(mismatch_host, |state| {
                matches!(state, HostState::IdentityMismatch { .. })
            })
            .await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let mismatch_service = service(&harness, backend.clone(), root.path());
        let error = mismatch_service
            .plan_update(mismatch_host)
            .await
            .expect_err("identity mismatch must refuse UPDATE");
        assert!(error.to_string().contains("identity is frozen"));
        assert!(backend.operations.lock().unwrap().is_empty());

        let shared = || HostScript {
            identity: Some("shared-identity".to_string()),
            ..HostScript::default()
        };
        let (builder, first) = FleetBuilder::new()
            .await
            .ssh("first.example", shared())
            .await;
        let (builder, second) = builder.ssh("second.example", shared()).await;
        let harness = builder.start().await;
        let duplicate = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                for host in [first, second] {
                    if matches!(
                        harness.manager.state(host),
                        Some(HostState::Duplicate { .. })
                    ) {
                        return host;
                    }
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("one shared identity row must freeze as duplicate");
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());
        let error = service
            .plan_update(duplicate)
            .await
            .expect_err("duplicate identity must refuse UPDATE");
        assert!(
            error
                .to_string()
                .contains("duplicates another registry row")
        );
        assert!(backend.operations.lock().unwrap().is_empty());
    }

    /// UPDATE binds the confirmed mutation to the identity observed during
    /// planning and refuses a different answer in the final preflight.
    #[farhelm_testtrace::test]
    async fn confirmed_update_rechecks_identity_before_mutation() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let host = harness
            .store
            .add_ssh_host("identity.example", None, None)
            .await
            .unwrap();
        let row = harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        harness
            .store
            .record_first_contact(host, &DialedAs::of(&row), "expected-identity")
            .await
            .unwrap();
        harness.manager.sync_registry().await.unwrap();
        let backend = FakeBackend::supervisor(root.path().to_path_buf());
        *backend.probe.lock().unwrap() = Some(Ok(ProbeObservation::Supervisor {
            build_version: "planned".to_string(),
            host_identity: Some("expected-identity".to_string()),
            dial_farhelm: root.path().join("lib/farhelm"),
            dial_state_dir: Some(root.path().join("state")),
        }));
        let service = service(&harness, backend.clone(), root.path());
        let plan = service.plan_update(host).await.unwrap();
        *backend.probe.lock().unwrap() = Some(Ok(ProbeObservation::Supervisor {
            build_version: "changed".to_string(),
            host_identity: Some("other-identity".to_string()),
            dial_farhelm: root.path().join("lib/farhelm"),
            dial_state_dir: Some(root.path().join("state")),
        }));
        let accepted = service
            .start_update(
                host,
                ProvisionRequest {
                    probe_id: plan.probe_id,
                },
            )
            .await
            .unwrap();
        let failed = wait_finished(&service, accepted.host_id).await;
        assert_eq!(failed.status, RunStatus::Failed);
        assert!(failed.message.unwrap().contains("identity changed"));
        assert!(backend.operations.lock().unwrap().is_empty());
    }

    /// Confirmation closes the absence race: a supervisor that starts after
    /// planning is registered and used as-is without any plan action.
    #[farhelm_testtrace::test]
    async fn confirmed_add_reprobes_and_uses_a_new_supervisor_as_is() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "race.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("initial absence must return a plan")
        };
        *backend.probe.lock().unwrap() = Some(Ok(ProbeObservation::Supervisor {
            build_version: "late-build".to_string(),
            host_identity: Some("late-identity".to_string()),
            dial_farhelm: root.path().join("late/farhelm"),
            dial_state_dir: Some(root.path().join("late/state")),
        }));
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        let view = wait_finished(&service, accepted.host_id).await;
        assert_eq!(view.status, RunStatus::Completed);
        assert!(
            view.steps
                .iter()
                .all(|step| step.status == StepStatus::Skipped)
        );
        assert!(backend.operations.lock().unwrap().is_empty());
        let row = service.host_row(accepted.host_id).await.unwrap();
        assert_eq!(row.host_identity.as_deref(), Some("late-identity"));
        assert_eq!(
            row.remote_farhelm.as_deref(),
            Some(root.path().join("late/farhelm").to_str().unwrap())
        );
    }

    /// An SSH/auth failure is an error with the host's diagnostic, never the
    /// absence result that unlocks setup.
    #[farhelm_testtrace::test]
    async fn ssh_failure_never_offers_provisioning() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::failing_probe(root.path().to_path_buf(), "Permission denied");
        let service = service(&harness, backend, root.path());
        let before = harness.store.list_hosts().await.unwrap();
        let error = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "user@denied".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .expect_err("an SSH failure must not become an offer");
        assert!(format!("{error:#}").contains("Permission denied"));
        assert_eq!(harness.store.list_hosts().await.unwrap(), before);
    }

    /// Confirmation registers the destination before the first plan action,
    /// and a second operation for that host is refused while the first waits.
    #[farhelm_testtrace::test]
    async fn registration_precedes_the_run_and_busy_hosts_refuse_a_second_run() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        backend
            .block_first
            .store(true, std::sync::atomic::Ordering::SeqCst);
        *backend.fail.lock().unwrap() = Some("daemon-reload".to_string());
        *backend.registration.lock().unwrap() =
            Some((harness.store.clone(), "user@ordered".to_string()));
        let service = service(&harness, backend.clone(), root.path());
        let first = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "user@ordered".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = first else {
            panic!("expected plan")
        };
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        backend.entered.notified().await;
        let second = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "user@ordered".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = second else {
            panic!("expected second plan")
        };
        let error = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .expect_err("the in-flight host must refuse another run");
        assert!(error.to_string().contains("in flight"));
        let progress = service.view(accepted.host_id).await.unwrap();
        assert_eq!(progress.run_id.as_deref(), Some(accepted.run_id.as_str()));
        backend.release.notify_one();
        let _ = wait_finished(&service, accepted.host_id).await;
    }

    /// Failure names the exact action and retains control-escaped host stderr;
    /// completed actions are not rolled back.
    #[farhelm_testtrace::test]
    async fn a_failing_step_is_reported_with_escaped_host_stderr() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        backend
            .stateful
            .store(true, std::sync::atomic::Ordering::SeqCst);
        *backend.fail.lock().unwrap() = Some("daemon-reload".to_string());
        let service = service(&harness, backend.clone(), root.path());
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "failing-step.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("expected plan")
        };
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        let view = wait_finished(&service, accepted.host_id).await;
        assert_eq!(view.status, RunStatus::Failed);
        let message = view.message.unwrap();
        assert!(message.contains("step 4 (daemon-reload) failed"));
        assert!(message.contains("\\u{1b}[31mhost said no\\nnext"));
        assert_eq!(
            backend.operations.lock().unwrap().as_slice(),
            [
                "create-directories",
                "install-farhelm",
                "write-unit",
                "daemon-reload"
            ]
        );
        assert!(root.path().join("lib").is_dir());
        assert!(root.path().join("state").is_dir());
        assert_eq!(
            std::fs::read(root.path().join("lib/farhelm")).unwrap(),
            b"farhelm test payload"
        );
        let unit_paths = std::fs::read_dir(root.path().join("units"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(unit_paths.len(), 1);
        assert!(
            std::fs::read_to_string(&unit_paths[0])
                .unwrap()
                .contains("KillMode=process")
        );

        *backend.fail.lock().unwrap() = None;
        let retry = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "failing-step.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = retry else {
            panic!("the failed run must release the host for recovery")
        };
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        assert_eq!(
            wait_finished(&service, accepted.host_id).await.status,
            RunStatus::Completed
        );
    }

    /// Linger success keeps the boot promise; privilege refusal is a
    /// completed degradation with the exact login-only wording.
    #[farhelm_testtrace::test]
    async fn conditional_linger_reports_both_outcomes() {
        for refused in [false, true] {
            let harness = harness().await;
            let root = tempfile::tempdir().unwrap();
            let backend = FakeBackend::absent(root.path().to_path_buf());
            *backend.linger.lock().unwrap() = Some(if refused {
                Err("permission denied".to_string())
            } else {
                Ok(())
            });
            let service = service(&harness, backend, root.path());
            let response = service
                .probe(ProbeRequest {
                    target: ProbeDestination::Ssh {
                        destination: "linger.example".to_string(),
                    },
                    remote_farhelm: None,
                    remote_state_dir: None,
                })
                .await
                .unwrap();
            let ProbeResponse::Provisionable { probe_id, .. } = response else {
                panic!("expected plan")
            };
            let accepted = service
                .start_add(ProvisionRequest { probe_id })
                .await
                .unwrap();
            let view = wait_finished(&service, accepted.host_id).await;
            assert_eq!(view.status, RunStatus::Completed);
            assert_eq!(
                view.message.as_deref(),
                refused.then_some("starts at login, not at boot")
            );
        }
    }

    /// Probe and confirmed ADD share the host-scoped progress reader:
    /// provision returns 202 with an identity, then GET exposes that run.
    #[farhelm_testtrace::test]
    async fn rest_surface_starts_and_rereads_a_host_scoped_run() {
        let mut harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend, root.path());
        harness.state = Arc::new(AppState::with_provisioning(
            Arc::clone(&harness.manager),
            harness.store.clone(),
            Arc::clone(&service),
        ));
        let router = harness.router();
        let probe = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/hosts/probe")
                    .header("host", "127.0.0.1:7433")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"target":{"kind":"ssh","destination":"rest.example"}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(probe.status(), StatusCode::OK);
        let probe: serde_json::Value =
            serde_json::from_slice(&to_bytes(probe.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let probe_id = probe["probe_id"].as_str().unwrap();
        let provision = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/hosts/provision")
                    .header("host", "127.0.0.1:7433")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({ "probe_id": probe_id }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(provision.status(), StatusCode::ACCEPTED);
        let accepted: serde_json::Value =
            serde_json::from_slice(&to_bytes(provision.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let host = accepted["host_id"].as_i64().unwrap();
        let run = accepted["run_id"].as_str().unwrap().to_string();
        let finished = wait_finished(&service, host).await;
        assert_eq!(finished.status, RunStatus::Completed);
        let progress = router
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri(format!("/api/hosts/{host}/provisioning"))
                    .header("host", "127.0.0.1:7433")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(progress.status(), StatusCode::OK);
        let progress: serde_json::Value =
            serde_json::from_slice(&to_bytes(progress.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(progress["run_id"], run);
        assert_eq!(progress["status"], "completed");
    }

    /// A valid host with no retained operation has a stable, explicit idle
    /// JSON shape rather than masquerading as missing progress.
    #[farhelm_testtrace::test]
    async fn progress_route_returns_the_idle_shape_before_any_run() {
        let mut harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let service = service(
            &harness,
            FakeBackend::absent(root.path().to_path_buf()),
            root.path(),
        );
        harness.state = Arc::new(AppState::with_provisioning(
            Arc::clone(&harness.manager),
            harness.store.clone(),
            service,
        ));
        let host = harness.store.list_hosts().await.unwrap()[0].id;
        let response = harness
            .router()
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/api/hosts/{host}/provisioning"))
                    .header("host", "127.0.0.1:7433")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let progress: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(progress["host_id"], host);
        assert!(progress["run_id"].is_null());
        assert!(progress["operation"].is_null());
        assert_eq!(progress["status"], "completed");
        assert_eq!(progress["steps"], serde_json::json!([]));
        assert_eq!(
            progress["message"],
            "no provisioning run has been recorded for this host"
        );
    }

    /// UPDATE uses the router-level plan/confirm handshake, returns a 202 run
    /// identity only after confirmation, exposes progress, and refuses a
    /// second confirmed plan while the host is busy.
    #[farhelm_testtrace::test]
    async fn update_route_plans_confirms_and_enforces_in_flight_exclusion() {
        let mut harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());
        let state = Arc::new(AppState::with_provisioning(
            Arc::clone(&harness.manager),
            harness.store.clone(),
            Arc::clone(&service),
        ));
        harness.state = Arc::clone(&state);
        let router = harness.router();
        // An SSH row: the reserved LOCAL row has no UPDATE action at all
        // now (D1), and this test is about the handshake, not that rule.
        let host = harness
            .store
            .add_ssh_host("update-route.example", None, None)
            .await
            .unwrap();
        harness.manager.sync_registry().await.unwrap();

        async fn plan(router: &axum::Router, host: HostId) -> String {
            let response = router
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri(format!("/api/hosts/{host}/update"))
                        .header("host", "127.0.0.1:7433")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let value: serde_json::Value =
                serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                    .unwrap();
            value["probe_id"].as_str().unwrap().to_string()
        }

        let first = plan(&router, host).await;
        let second = plan(&router, host).await;
        assert!(
            backend.operations.lock().unwrap().is_empty(),
            "UPDATE planning mutated the host before confirmation"
        );
        backend
            .block_first
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let confirm = |probe_id: &str| {
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/api/hosts/{host}/update"))
                .header("host", "127.0.0.1:7433")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "probe_id": probe_id }).to_string(),
                ))
                .unwrap()
        };
        let accepted = router.clone().oneshot(confirm(&first)).await.unwrap();
        assert_eq!(accepted.status(), StatusCode::ACCEPTED);
        let accepted: serde_json::Value =
            serde_json::from_slice(&to_bytes(accepted.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(accepted["host_id"], host);
        backend.entered.notified().await;
        let busy = router.clone().oneshot(confirm(&second)).await.unwrap();
        assert_eq!(busy.status(), StatusCode::CONFLICT);
        let progress = router
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/api/hosts/{host}/provisioning"))
                    .header("host", "127.0.0.1:7433")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let progress: serde_json::Value =
            serde_json::from_slice(&to_bytes(progress.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(progress["operation"], "update");
        assert_eq!(progress["status"], "running");
        backend.release.notify_one();
        assert_eq!(
            wait_finished(&service, host).await.status,
            RunStatus::Completed
        );
    }

    /// Host removal waits behind an in-flight run's write authority, then
    /// purges retained progress and unconsumed UPDATE confirmations with the
    /// durable row instead of leaving process-local ghosts.
    #[farhelm_testtrace::test]
    async fn removal_serializes_with_runs_and_purges_provisioning_memory() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let host = harness
            .store
            .add_ssh_host("remove.example", Some("farhelm"), Some("/tmp/state"))
            .await
            .unwrap();
        harness.manager.sync_registry().await.unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        backend
            .block_first
            .store(true, std::sync::atomic::Ordering::SeqCst);
        *backend.fail.lock().unwrap() = Some("daemon-reload".to_string());
        let service = service(&harness, backend.clone(), root.path());
        let first = service.plan_update(host).await.unwrap();
        let retained = service.plan_update(host).await.unwrap();
        let _accepted = service
            .start_update(
                host,
                ProvisionRequest {
                    probe_id: first.probe_id,
                },
            )
            .await
            .unwrap();
        backend.entered.notified().await;
        let state = Arc::new(AppState::with_provisioning(
            Arc::clone(&harness.manager),
            harness.store.clone(),
            Arc::clone(&service),
        ));
        let mut removal = tokio::spawn(async move {
            crate::hosts::remove_host(State(state), AxPath(host))
                .await
                .into_response()
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut removal)
                .await
                .is_err(),
            "removal bypassed the in-flight host write authority"
        );

        backend.release.notify_one();
        assert_eq!(removal.await.unwrap().status(), StatusCode::OK);
        let memory = service.memory.lock().await;
        assert!(!memory.runs.contains_key(&host));
        assert!(!memory.busy.contains(&host));
        assert!(!memory.plans.contains_key(&retained.probe_id));
        drop(memory);
        assert!(
            harness
                .store
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .all(|row| row.id != host)
        );
    }

    /// Typed provisioning failures keep their router-level status mapping:
    /// invalid input 400, consumed plans and busy hosts 409, backend failures
    /// 502, and missing hosts 404.
    #[farhelm_testtrace::test]
    async fn router_maps_all_provisioning_error_classes() {
        let mut harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::failing_probe(root.path().to_path_buf(), "denied");
        let service = service(&harness, backend, root.path());
        let state = Arc::new(AppState::with_provisioning(
            Arc::clone(&harness.manager),
            harness.store.clone(),
            service,
        ));
        harness.state = Arc::clone(&state);
        let router = harness.router();
        let post_probe = |body: serde_json::Value| {
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/hosts/probe")
                .header("host", "127.0.0.1:7433")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap()
        };
        let invalid = router
            .clone()
            .oneshot(post_probe(serde_json::json!({
                "target": { "kind": "ssh", "destination": "-bad" }
            })))
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
        let backend = router
            .clone()
            .oneshot(post_probe(serde_json::json!({
                "target": { "kind": "ssh", "destination": "host" }
            })))
            .await
            .unwrap();
        assert_eq!(backend.status(), StatusCode::BAD_GATEWAY);
        let unknown = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/hosts/provision")
                    .header("host", "127.0.0.1:7433")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"probe_id":"unknown"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unknown.status(), StatusCode::CONFLICT);
        let missing = router
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/hosts/999999/update")
                    .header("host", "127.0.0.1:7433")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }

    /// Host-tmux acceptance must use the SAME floor the remote supervisor
    /// enforces at startup. If this test drifted looser, provisioning
    /// would skip the private payload on a host whose tmux the supervisor
    /// then refuses — a "successful" provision that cannot start a
    /// session.
    ///
    /// The at-the-floor case is spelled through the constant rather than a
    /// literal so a future floor bump does not need this test edited; the
    /// too-old cases stay literal because the floor is DESIGNED to exclude
    /// exactly these distro packages (Ubuntu 24.04's 3.4, 26.04's 3.6,
    /// Debian 13 and Fedora 42's 3.5a) and a bump can only make that truer.
    #[farhelm_testtrace::test]
    fn host_tmux_acceptance_tracks_the_supervisor_floor() {
        let floor = farhelm_supervisor::tmux::TMUX_FLOOR;
        assert!(tmux_meets_floor(&format!("tmux {floor}")));
        assert!(tmux_meets_floor("tmux 99.0"));
        assert!(!tmux_meets_floor("tmux 3.6"));
        assert!(!tmux_meets_floor("tmux 3.5a"));
        assert!(!tmux_meets_floor("tmux 3.4"));
        assert!(!tmux_meets_floor("tmux 3.3a"));
        // A host with no tmux reports an empty field, and a host whose
        // tmux answered something unrecognizable is equally unknown; both
        // must request the private payload rather than be assumed usable.
        assert!(!tmux_meets_floor("not installed"));
        assert!(!tmux_meets_floor(""));
    }

    /// The production classifier requires both the private marker and exit
    /// 75; authentication failure, unmarked 75, and malformed hello remain
    /// errors even when process creation itself succeeds. The final skew
    /// case stays an ERROR here only because its script prints no resolved
    /// marker: skew per se is a positive observation now (see
    /// `a_skewed_hello_is_a_positive_supervisor_observation`), but a remote
    /// answer that never named its binary has nothing to dial. Which error
    /// arm catches it can vary — the script exits without reading stdin, so
    /// the helm's own hello write can EPIPE before the frame is read — and
    /// the assertion is deliberately indifferent: both arms error.
    #[farhelm_testtrace::test]
    async fn production_probe_classifier_distinguishes_positive_absence() {
        let root = tempfile::tempdir().unwrap();
        let local_target = ProbeTarget {
            transport: ProvisioningTarget::Local,
            probe_farhelm: PathBuf::from("scripted-local"),
            probe_state_dir: None,
        };
        let mut local_backend = test_system_backend(root.path());
        local_backend.launcher = ScriptLauncher::new(["exit 75".to_string()]);
        assert!(matches!(
            local_backend.probe(&local_target).await,
            Ok(ProbeObservation::Absent)
        ));

        let target = ProbeTarget {
            transport: ProvisioningTarget::Ssh {
                destination: "scripted.example".to_string(),
            },
            probe_farhelm: PathBuf::from("farhelm"),
            probe_state_dir: None,
        };
        let malformed = farhelm_proto::Frame {
            kind: farhelm_proto::FrameKind::Control,
            channel: 0,
            body: br#"{"not":"hello"}"#.to_vec(),
        };
        let skew = farhelm_proto::Frame::control(&ControlMsg::Hello {
            protocol_version: farhelm_proto::PROTOCOL_VERSION + 1,
            build_version: "future".to_string(),
            role: "supervisor".to_string(),
            host_identity: None,
            auth: None,
        });
        let cases = [
            (
                format!("printf '%s\\n' '{REMOTE_PROBE_MARKER}' >&2; exit 75"),
                true,
            ),
            (
                "printf 'Permission denied\\n' >&2; exit 255".to_string(),
                false,
            ),
            (
                format!(
                    "head -c {} /dev/zero | tr '\\0' x >&2; \
                     printf '\\n%s\\n' '{REMOTE_PROBE_MARKER}' >&2; exit 75",
                    MAX_CHILD_STREAM_BYTES + 4096
                ),
                true,
            ),
            ("exit 75".to_string(), false),
            (
                format!(
                    "printf '%s\\n' '{REMOTE_PROBE_MARKER}' >&2; {}; exit 75",
                    frame_script(malformed)
                ),
                false,
            ),
            (frame_script(skew), false),
        ];
        for (script, absent) in cases {
            let launcher = ScriptLauncher::new([script]);
            let backend = SystemBackend {
                control_dir: root.path().to_path_buf(),
                linger: LingerBehavior::Simulated(Ok(())),
                launcher,
                runtime_units: false,
                fail_before_rename: false,
            };
            let result = backend.probe(&target).await;
            assert_eq!(
                matches!(result, Ok(ProbeObservation::Absent)),
                absent,
                "unexpected classifier result: {result:?}"
            );
            if !absent {
                assert!(result.is_err());
            }
        }
    }

    /// A peer that answers the hello with a DIFFERENT protocol version is a
    /// POSITIVE presence observation, not a probe failure: only a live
    /// supervisor sends a hello at all, and the skew payload names its
    /// build. This is the probe-level half of making UPDATE work on skewed
    /// hosts — the exact hosts the update action exists for. Pinned because
    /// the original implementation classified the skew refusal as a
    /// transport failure ("the supervisor probe closed before hello
    /// completion with exit status 0"), which made a skewed host
    /// un-updatable; found 2026-09-01 on the first real cross-protocol
    /// update attempt (protocol-12 host, protocol-14 helm).
    ///
    /// Both transports, because they resolve the dial binary differently:
    /// local names it directly, remote reads the resolved marker the probe
    /// script printed before exec — and a remote answer WITHOUT that marker
    /// stays an error even under skew (the classifier test above keeps that
    /// case), since presence alone names no binary to dial.
    #[farhelm_testtrace::test]
    async fn a_skewed_hello_is_a_positive_supervisor_observation() {
        let root = tempfile::tempdir().unwrap();
        let old_hello = || {
            farhelm_proto::Frame::control(&ControlMsg::Hello {
                protocol_version: farhelm_proto::PROTOCOL_VERSION - 1,
                build_version: "0.1.1-old".to_string(),
                role: "supervisor".to_string(),
                host_identity: Some("identity-the-refusal-never-conveys".to_string()),
                auth: None,
            })
        };

        let local_target = ProbeTarget {
            transport: ProvisioningTarget::Local,
            probe_farhelm: PathBuf::from("scripted-local"),
            probe_state_dir: Some(PathBuf::from("/probe/state")),
        };
        let mut local_backend = test_system_backend(root.path());
        // `cat >/dev/null` keeps the scripted peer's stdin OPEN after it
        // writes its hello (stop_probe kills it later): a peer that exits
        // without reading races the helm's own hello write into an EPIPE
        // before the buffered skew frame is ever read — a fixture-only
        // shape (a real supervisor consumes the hello before refusing)
        // that made this test flaky under full-suite load.
        local_backend.launcher =
            ScriptLauncher::new([format!("{}; cat >/dev/null", frame_script(old_hello()))]);
        match local_backend.probe(&local_target).await {
            Ok(ProbeObservation::SkewedSupervisor {
                peer_build,
                dial_farhelm,
                dial_state_dir,
            }) => {
                assert_eq!(peer_build, "0.1.1-old");
                assert_eq!(dial_farhelm, PathBuf::from("scripted-local"));
                assert_eq!(dial_state_dir, Some(PathBuf::from("/probe/state")));
            }
            other => panic!("expected a skewed-supervisor observation, got {other:?}"),
        }

        let remote_target = ProbeTarget {
            transport: ProvisioningTarget::Ssh {
                destination: "scripted.example".to_string(),
            },
            probe_farhelm: PathBuf::from("farhelm"),
            probe_state_dir: None,
        };
        // Same stdin-open tail (`cat >/dev/null`) as the local case above, same
        // reason.
        let script = format!(
            "printf '%s\\n' '{REMOTE_PROBE_MARKER}' >&2; \
             printf '%s%s\\n' '{REMOTE_RESOLVED_PREFIX}' '/resolved/lib/farhelm' >&2; \
             {}; cat >/dev/null",
            frame_script(old_hello())
        );
        let remote_backend = SystemBackend {
            control_dir: root.path().to_path_buf(),
            linger: LingerBehavior::Simulated(Ok(())),
            launcher: ScriptLauncher::new([script]),
            runtime_units: false,
            fail_before_rename: false,
        };
        match remote_backend.probe(&remote_target).await {
            Ok(ProbeObservation::SkewedSupervisor {
                peer_build,
                dial_farhelm,
                dial_state_dir,
            }) => {
                assert_eq!(peer_build, "0.1.1-old");
                assert_eq!(dial_farhelm, PathBuf::from("/resolved/lib/farhelm"));
                assert_eq!(dial_state_dir, None);
            }
            other => panic!("expected a skewed-supervisor observation, got {other:?}"),
        }
    }

    /// The desktop-startup classifier must treat a skewed local supervisor
    /// as ANSWERING: it owns the socket and the state directory, so
    /// starting a rival because we cannot talk to it would be strictly
    /// worse than reusing it and letting the manager's version-skew state
    /// say what is wrong. Driven through the public entry point with a real
    /// on-disk script, because this is the path `farhelm-desktop` calls at
    /// startup.
    #[farhelm_testtrace::test]
    async fn discover_local_treats_a_skewed_supervisor_as_answering() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let hello = farhelm_proto::Frame::control(&ControlMsg::Hello {
            protocol_version: farhelm_proto::PROTOCOL_VERSION + 1,
            build_version: "9.9.9-future".to_string(),
            role: "supervisor".to_string(),
            host_identity: None,
            auth: None,
        });
        let script_path = root.path().join("fake-farhelm");
        std::fs::write(
            &script_path,
            // The stdin-open tail again — see the skewed-observation test.
            format!("#!/bin/sh\n{}\ncat >/dev/null\n", frame_script(hello)),
        )
        .unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let discovery = discover_local_supervisor(&script_path, root.path())
            .await
            .unwrap();
        assert_eq!(discovery, LocalSupervisorDiscovery::Answering);
    }

    /// Child capture fails closed on either unbounded peer output or a
    /// deadline, returning only after the offending process has been killed
    /// and reaped.
    #[farhelm_testtrace::test]
    async fn child_capture_bounds_output_and_reaps_timeouts() {
        let spawn = |script: &str| {
            let mut command = tokio::process::Command::new("sh");
            command
                .args(["-c", script])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            isolate_process_group(&mut command);
            command.spawn().unwrap()
        };

        let overflow = capture_child(
            spawn("head -c 70000 /dev/zero"),
            Duration::from_secs(5),
            "overflow fixture",
        )
        .await
        .expect_err("output beyond the cap must terminate the child");
        assert!(overflow.context.contains("exceeded"));

        let timeout = capture_child(
            spawn("sleep 300"),
            Duration::from_millis(20),
            "timeout fixture",
        )
        .await
        .expect_err("a timed-out child must be terminated");
        assert!(timeout.context.contains("timed out"));
    }

    /// Probe diagnostics stop retaining bytes at the cap but keep draining,
    /// so an oversized producer reaches EOF and a later marker is observed.
    #[farhelm_testtrace::test]
    async fn oversized_probe_stderr_never_blocks_its_producer() {
        let (mut producer, consumer) = tokio::io::duplex(1024);
        let bytes = vec![b'x'; MAX_CHILD_STREAM_BYTES + 4096];
        let writer = tokio::spawn(async move {
            producer.write_all(&bytes).await.unwrap();
            producer.write_all(b"\n").await.unwrap();
            producer
                .write_all(REMOTE_PROBE_MARKER.as_bytes())
                .await
                .unwrap();
            producer.write_all(b"\n").await.unwrap();
            producer.shutdown().await.unwrap();
        });
        let (signal, mut failures) = tokio::sync::mpsc::unbounded_channel();
        let drained =
            tokio::time::timeout(Duration::from_secs(2), drain_probe_stderr(consumer, signal))
                .await
                .expect("the bounded reader must continue draining to EOF");
        tokio::time::timeout(Duration::from_secs(2), writer)
            .await
            .expect("the oversized producer must finish")
            .unwrap();
        assert_eq!(drained.prefix.len(), MAX_CHILD_STREAM_BYTES);
        assert!(drained.command_started);
        assert!(failures.try_recv().is_err());
    }

    /// Silent command failures still identify whether the host returned an
    /// exit code or was terminated by a signal.
    #[farhelm_testtrace::test]
    async fn required_shell_diagnostics_always_include_termination_status() {
        let root = tempfile::tempdir().unwrap();
        let backend = test_system_backend(root.path());
        let exited = backend
            .require_shell(&ProvisioningTarget::Local, "exit 7", "silent failure")
            .await
            .expect_err("the nonzero shell must fail");
        assert!(exited.rendered().contains("exit status 7"));

        #[cfg(unix)]
        {
            let signalled = backend
                .require_shell(
                    &ProvisioningTarget::Local,
                    "kill -TERM $$",
                    "signal failure",
                )
                .await
                .expect_err("the signalled shell must fail");
            assert!(signalled.rendered().contains("terminated by signal 15"));
        }
    }

    /// Reach parsing does NOT gate on the distro ID: it refuses only the
    /// capabilities that actually matter (malformed fields, an
    /// unsupported architecture, an unusable systemd user manager), and
    /// carries whatever ID it saw — including none at all — into `Reach`
    /// for the confirmation plan to name. Distros other than Ubuntu used
    /// to be sent to the manual path outright; this test used to assert
    /// that for `debian` and now asserts the opposite, which is the point
    /// of this change (see the workspace's centos-gate task).
    #[farhelm_testtrace::test]
    fn reach_output_parser_covers_platform_and_tool_boundaries() {
        let supported = |os: &str, arch: &str, tmux_path: &str, tmux: &str, manager: &str| {
            format!(
                "{REACH_RECORD_MARKER}\0{os}\0/home/test\0{arch}\0{tmux_path}\0{tmux}\0{manager}\0/home/test/.config/systemd/user\0"
            )
        };
        // A non-Ubuntu ID with every real capability present is supported,
        // and the ID it reported rides along unchanged for the plan to
        // display — this is the behavior the ubuntu-only gate used to
        // block.
        let ReachOutcome::Supported(debian) = parse_reach_output(
            supported("debian", "x86_64", "/usr/bin/tmux", "tmux 3.3", "usable").as_bytes(),
        )
        .unwrap() else {
            panic!("a usable manager is supported regardless of distro ID")
        };
        assert_eq!(debian.distro_id, "debian");
        let ReachOutcome::Supported(centos) = parse_reach_output(
            supported("centos", "x86_64", "/usr/bin/tmux", "tmux 3.4", "usable").as_bytes(),
        )
        .unwrap() else {
            panic!("a usable manager is supported regardless of distro ID")
        };
        assert_eq!(centos.distro_id, "centos");
        // No /etc/os-release at all reports an empty ID over the wire; the
        // manager check is the real requirement, so an empty ID is still
        // supported rather than refused for lack of a name to show.
        let ReachOutcome::Supported(unknown) = parse_reach_output(
            supported("", "x86_64", "/usr/bin/tmux", "tmux 3.4", "usable").as_bytes(),
        )
        .unwrap() else {
            panic!("an empty distro ID is not a reason to refuse an otherwise usable host")
        };
        assert_eq!(unknown.distro_id, "");
        assert!(matches!(
            parse_reach_output(supported("ubuntu", "riscv64", "", "", "usable").as_bytes())
                .unwrap(),
            ReachOutcome::Manual(_)
        ));
        assert!(parse_reach_output(b"too\0short\0").is_err());
        let clean = supported("ubuntu", "x86_64", "/usr/bin/tmux", "tmux 3.4", "usable");
        assert!(parse_reach_output(format!("shell banner{clean}").as_bytes()).is_err());
        assert!(parse_reach_output(format!("{clean}trailing output").as_bytes()).is_err());
        assert!(matches!(
            parse_reach_output(supported("ubuntu", "x86_64", "", "", "unavailable").as_bytes())
                .unwrap(),
            ReachOutcome::Manual(_)
        ));
        let ReachOutcome::Supported(missing) =
            parse_reach_output(supported("ubuntu", "x86_64", "", "", "usable").as_bytes()).unwrap()
        else {
            panic!("x86_64 Ubuntu with a user manager is supported")
        };
        assert!(missing.needs_tmux);
        assert_eq!(missing.distro_id, "ubuntu");
        let ReachOutcome::Supported(old) = parse_reach_output(
            supported("ubuntu", "aarch64", "/usr/bin/tmux", "tmux 3.2", "usable").as_bytes(),
        )
        .unwrap() else {
            panic!("aarch64 Ubuntu with a user manager is supported")
        };
        assert_eq!(old.arch, PayloadArch::Aarch64);
        assert!(old.needs_tmux);
        // A host whose tmux clears the floor is the ONLY case that skips
        // the private payload; without it nothing here would notice an
        // acceptance path that had stopped accepting anything at all.
        let at_floor = format!("tmux {}", farhelm_supervisor::tmux::TMUX_FLOOR);
        let ReachOutcome::Supported(usable_tmux) = parse_reach_output(
            supported("ubuntu", "x86_64", "/usr/bin/tmux", &at_floor, "usable").as_bytes(),
        )
        .unwrap() else {
            panic!("x86_64 Ubuntu with a user manager is supported")
        };
        assert!(!usable_tmux.needs_tmux);
        // The EXECUTABLE, not its directory: the plan pins it into the
        // unit as FARHELM_TMUX so a leftover private tmux cannot shadow
        // the binary accepted here.
        assert_eq!(usable_tmux.host_tmux, Some(PathBuf::from("/usr/bin/tmux")));
        // The version clears the floor here, so a relative tmux path is
        // the only thing left that can force the payload.
        let ReachOutcome::Supported(relative_tmux) = parse_reach_output(
            supported("ubuntu", "x86_64", "bin/tmux", &at_floor, "usable").as_bytes(),
        )
        .unwrap() else {
            panic!("an otherwise supported host remains provisionable")
        };
        assert!(relative_tmux.needs_tmux);
        assert_eq!(
            relative_tmux.user_unit_dir,
            PathBuf::from("/home/test/.config/systemd/user")
        );
        assert!(matches!(
            parse_reach_output(
                supported("ubuntu", "x86_64", "/usr/bin/tmux", "tmux 3.4", "unsupported-xdg")
                    .as_bytes()
            )
            .unwrap(),
            ReachOutcome::Manual(reason) if reason.contains("XDG_CONFIG_HOME")
        ));
        #[cfg(unix)]
        {
            let mut non_utf8_home = format!("{REACH_RECORD_MARKER}\0ubuntu\0/home/").into_bytes();
            non_utf8_home.push(0xff);
            non_utf8_home.extend_from_slice(
                b"\0x86_64\0/usr/bin/tmux\0tmux 3.4\0usable\0/home/test/.config/systemd/user\0",
            );
            assert!(matches!(
                parse_reach_output(&non_utf8_home).unwrap(),
                ReachOutcome::Manual(reason)
                    if reason.contains("HOME") && reason.contains("explicit paths")
            ));
        }
    }

    /// A host needing private tmux receives that payload before the unit, and
    /// the rendered unit searches the isolated payload directory first.
    #[farhelm_testtrace::test]
    fn tmux_payload_plan_is_concrete_and_ordered() {
        let root = tempfile::tempdir().unwrap();
        let plan = layout(root.path())
            .plan(
                ProvisioningOperation::Add,
                ProvisioningTarget::Ssh {
                    destination: "host".to_string(),
                },
                &Reach {
                    home: root.path().join("home"),
                    user_unit_dir: root.path().join("home/.config/systemd/user"),
                    arch: PayloadArch::X86_64,
                    distro_id: "ubuntu".to_string(),
                    needs_tmux: true,
                    host_tmux: None,
                },
                "nonce",
            )
            .unwrap();
        assert!(matches!(
            &plan.actions[2],
            ProvisioningAction::InstallPayload {
                payload: PayloadKind::Tmux,
                arch: PayloadArch::X86_64,
                destination,
                ..
            } if destination == &root.path().join("lib/tmux")
        ));
        let ProvisioningAction::WriteUnit { content, .. } = &plan.actions[3] else {
            panic!("the unit follows both payloads")
        };
        assert!(content.contains(&format!("PATH={}", root.path().join("lib").display())));
        // The installed payload is named outright, not merely reachable:
        // the same PATH-shadowing hazard that can hide an accepted host
        // tmux (see `update_pins_the_accepted_host_tmux_over_a_stale_
        // private_one`) applies in reverse to anything else called `tmux`
        // that lands earlier in the search.
        assert!(
            content.contains(&format!(
                "Environment=\"FARHELM_TMUX={}\"",
                root.path().join("lib/tmux").display()
            )),
            "{content}"
        );
    }

    /// The confirmation's host line names whichever distro the reach probe
    /// saw, verbatim, and falls back to "unknown distribution" only when
    /// the host had no `/etc/os-release` at all — the one case
    /// `parse_reach_output` still lets through with an empty ID. Neither
    /// branch depends on ANY particular distro being present: the whole
    /// point of gating on capabilities is that this line is free to name
    /// whatever it saw.
    #[farhelm_testtrace::test]
    fn confirmation_host_line_names_the_distro_or_says_unknown() {
        let root = tempfile::tempdir().unwrap();
        let reach = |distro_id: &str| Reach {
            home: root.path().join("home"),
            user_unit_dir: root.path().join("home/.config/systemd/user"),
            arch: PayloadArch::Aarch64,
            distro_id: distro_id.to_string(),
            needs_tmux: false,
            host_tmux: Some(PathBuf::from("/usr/bin/tmux")),
        };
        let named = layout(root.path())
            .plan(
                ProvisioningOperation::Add,
                ProvisioningTarget::Ssh {
                    destination: "host".to_string(),
                },
                &reach("centos"),
                "nonce",
            )
            .unwrap();
        assert!(named.confirmation().contains("host: centos, aarch64\n"));
        let unknown = layout(root.path())
            .plan(
                ProvisioningOperation::Add,
                ProvisioningTarget::Ssh {
                    destination: "host".to_string(),
                },
                &reach(""),
                "nonce",
            )
            .unwrap();
        assert!(
            unknown
                .confirmation()
                .contains("host: unknown distribution, aarch64\n")
        );
    }

    /// Production layout has no test overrides: local and SSH plans use the
    /// documented library, state, unit, and nonce-scoped temporary paths.
    #[farhelm_testtrace::test]
    fn production_layout_uses_the_exact_deployment_paths() {
        let home = PathBuf::from("/home/provisioned");
        let user_units = PathBuf::from("/xdg/systemd/user");
        let local_state = PathBuf::from("/helm/state");
        let reach = Reach {
            home: home.clone(),
            user_unit_dir: user_units.clone(),
            arch: PayloadArch::X86_64,
            distro_id: "ubuntu".to_string(),
            needs_tmux: false,
            host_tmux: Some(PathBuf::from("/usr/bin/tmux")),
        };
        let layout = PlanLayout::production(local_state.clone());

        for (target, expected_state) in [
            (ProvisioningTarget::Local, local_state),
            (
                ProvisioningTarget::Ssh {
                    destination: "host.example".to_string(),
                },
                home.join(".local/state/farhelm"),
            ),
        ] {
            let plan = layout
                .plan(ProvisioningOperation::Add, target, &reach, "nonce")
                .unwrap();
            let lib = home.join(".local/lib/farhelm");
            let farhelm = lib.join("farhelm");
            let unit = user_units.join("farhelm-supervisor.service");
            assert_eq!(plan.farhelm_path, farhelm);
            assert_eq!(plan.state_dir, expected_state);
            let ProvisioningAction::EnsureDirectories { directories } = &plan.actions[0] else {
                panic!("production plan must begin with directories")
            };
            assert_eq!(directories[0].path, lib);
            assert_eq!(directories[1].path, expected_state);
            assert_eq!(directories[2].path, user_units);
            assert!(matches!(
                &plan.actions[1],
                ProvisioningAction::InstallPayload { destination, temporary, .. }
                    if destination == &farhelm
                        && temporary == &lib.join(".farhelm.farhelm-tmp-nonce")
            ));
            assert!(matches!(
                &plan.actions[2],
                ProvisioningAction::WriteUnit { unit: name, destination, temporary, .. }
                    if name == "farhelm-supervisor.service"
                        && destination == &unit
                        && temporary == &user_units.join(
                            ".farhelm-supervisor.service.farhelm-tmp-nonce"
                        )
            ));
        }
    }

    /// Execution resolves each install action through its own payload kind,
    /// so the private-tmux branch cannot receive the Farhelm executable.
    #[farhelm_testtrace::test]
    async fn tmux_install_uses_the_distinct_tmux_fixture() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        {
            let mut outcome = backend.reach.lock().unwrap();
            let ReachOutcome::Supported(reach) = &mut *outcome else {
                panic!("the fixture starts supported")
            };
            reach.needs_tmux = true;
            reach.host_tmux = None;
        }
        std::fs::write(root.path().join("farhelm-payload"), b"farhelm").unwrap();
        std::fs::write(root.path().join("tmux-payload"), b"tmux").unwrap();
        let payloads = Arc::new(MutablePayload {
            farhelm: Mutex::new(root.path().join("farhelm-payload")),
            tmux: root.path().join("tmux-payload"),
        });
        let service = ProvisioningService::injected(
            harness.store.clone(),
            Arc::clone(&harness.manager),
            backend.clone(),
            payloads,
            layout(root.path()),
            PathBuf::from("/test/farhelm"),
        );
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "tmux-fixture.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("the missing private tmux must produce a plan")
        };
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        assert_eq!(
            wait_finished(&service, accepted.host_id).await.status,
            RunStatus::Completed
        );
        let operations = backend.operations.lock().unwrap();
        assert!(
            operations
                .iter()
                .any(|operation| operation == "install-farhelm")
        );
        assert!(
            operations
                .iter()
                .any(|operation| operation == "install-tmux")
        );
    }

    /// Pending plans retain exactly the newest 64 opaque tokens.
    #[farhelm_testtrace::test]
    async fn pending_plan_cache_evicts_only_the_oldest_at_65() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let service = service(
            &harness,
            FakeBackend::absent(root.path().to_path_buf()),
            root.path(),
        );
        let mut ids = Vec::new();
        for index in 0..65 {
            let response = service
                .probe(ProbeRequest {
                    target: ProbeDestination::Ssh {
                        destination: format!("host-{index}.example"),
                    },
                    remote_farhelm: None,
                    remote_state_dir: None,
                })
                .await
                .unwrap();
            let ProbeResponse::Provisionable { probe_id, .. } = response else {
                panic!("absence must retain a plan")
            };
            ids.push(probe_id);
        }
        assert!(service.consume_plan(&ids[0]).await.is_err());
        assert!(service.consume_plan(&ids[1]).await.is_ok());
        assert!(service.consume_plan(ids.last().unwrap()).await.is_ok());
    }

    /// A supervisor unit on the helm's own machine that runs this farhelm
    /// belongs to whoever wrote it, and the panel says so with the
    /// remedy that fits (D9). Both wordings are asserted whole because
    /// they are the entire user-facing answer.
    ///
    /// The two differ on purpose: a unit `farhelm helm setup` wrote can be
    /// driven with `systemctl --user restart`, while a hand-written one is
    /// its author's to manage and is only reported as off limits to the
    /// panel — Farhelm has no idea what that author intended.
    ///
    /// Both the ADD path (which reaches this only after discovery finds
    /// nothing answering) and the UPDATE path produce it.
    #[farhelm_testtrace::test]
    async fn a_local_supervisor_unit_running_this_farhelm_is_off_limits_to_the_panel() {
        for managed in [true, false] {
            let harness = harness().await;
            let root = tempfile::tempdir().unwrap();
            let farhelm = root.path().join("farhelm");
            std::fs::write(&farhelm, b"#!/bin/sh\n").unwrap();
            let unit = crate::units::render_supervisor_unit(&crate::units::SupervisorUnitInputs {
                farhelm: &farhelm,
                state_dir: &root.path().join("state"),
                tmux: Path::new("/usr/bin/tmux"),
            })
            .unwrap();
            let backend = FakeBackend::absent(root.path().to_path_buf());
            *backend.user_unit.lock().unwrap() = Ok(Some(if managed {
                crate::units::managed(unit)
            } else {
                unit
            }));
            let service = ProvisioningService::injected(
                harness.store.clone(),
                Arc::clone(&harness.manager),
                backend.clone(),
                Arc::new(NoPayloads),
                layout(root.path()),
                farhelm.clone(),
            );

            let response = service
                .probe(ProbeRequest {
                    target: ProbeDestination::Local,
                    remote_farhelm: None,
                    remote_state_dir: None,
                })
                .await
                .unwrap();
            let ProbeResponse::Manual { reason } = response else {
                panic!("a setup-owned local supervisor must not be provisionable")
            };
            if managed {
                assert_eq!(
                    reason,
                    "farhelm-supervisor.service on this machine is managed by farhelm helm setup; \
                     it is not touched from the hosts panel. Start or restart it with: systemctl \
                     --user restart farhelm-supervisor.service"
                );
            } else {
                assert_eq!(
                    reason,
                    "farhelm-supervisor.service on this machine already runs this farhelm and was \
                     written by hand; it is not touched from the hosts panel"
                );
            }

            let error = service
                .plan_update(local_row(&harness).await)
                .await
                .expect_err("UPDATE is the path that would overwrite the unit");
            assert_eq!(error.to_string(), reason);
            // The ownership question is only ever asked about the
            // supervisor unit; the helm's own unit is none of the panel's
            // business.
            assert_eq!(
                backend.unit_reads.lock().unwrap().as_slice(),
                ["farhelm-supervisor.service", "farhelm-supervisor.service"]
            );
        }
    }

    /// With nothing answering on the helm's own machine, the panel stops
    /// offering to install a supervisor and hands the operator to
    /// `farhelm helm setup` instead (D1) — on the ADD path AFTER discovery
    /// has found nothing, and on the UPDATE path before any probe at all.
    /// This is what an ordinary first-time helm machine sees, so the
    /// wording is asserted whole.
    ///
    /// A unit for some OTHER farhelm falls through to this same generic
    /// answer: it is not this helm's supervisor, so there is no owner to
    /// name. A unit whose command cannot be parsed does NOT — see the
    /// unclassifiable case below.
    #[farhelm_testtrace::test]
    async fn an_absent_local_supervisor_is_answered_with_run_setup_here() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let other = root.path().join("other-farhelm");
        std::fs::write(&other, b"#!/bin/sh\n").unwrap();
        for unit in [
            None,
            Some(crate::units::managed(
                crate::units::render_supervisor_unit(&crate::units::SupervisorUnitInputs {
                    farhelm: &other,
                    state_dir: &root.path().join("state"),
                    tmux: Path::new("/usr/bin/tmux"),
                })
                .unwrap(),
            )),
        ] {
            let backend = FakeBackend::absent(root.path().to_path_buf());
            *backend.user_unit.lock().unwrap() = Ok(unit);
            let service = service(&harness, backend.clone(), root.path());
            let response = service
                .probe(ProbeRequest {
                    target: ProbeDestination::Local,
                    remote_farhelm: None,
                    remote_state_dir: None,
                })
                .await
                .unwrap();
            let ProbeResponse::Manual { reason } = response else {
                panic!("the local row no longer installs a supervisor")
            };
            assert_eq!(
                reason,
                "this is the helm's own machine; run farhelm helm setup here instead of \
                 provisioning from the panel"
            );
            // Discovery ran first: this answer reports what the probe
            // found, and a supervisor that ANSWERS is registered instead.
            assert!(backend.probe.lock().unwrap().is_none());
        }
    }

    /// UPDATE on the local row is refused for EVERY local row, with no
    /// unit file present and before any transport work — the alternate
    /// route to the install the ADD path stopped offering.
    ///
    /// This is the regression the narrower rule missed: a local row whose
    /// supervisor unit was absent (or unrecognizable) went straight on to
    /// the ordinary UPDATE planner, which installs the binary and the unit
    /// from nothing. That is precisely what D1 removed from the panel.
    #[farhelm_testtrace::test]
    async fn an_absent_local_supervisor_cannot_be_installed_through_update() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());
        let error = service
            .plan_update(local_row(&harness).await)
            .await
            .expect_err("UPDATE must not install a supervisor on the helm's own machine");
        assert_eq!(
            error.to_string(),
            "this is the helm's own machine; run farhelm helm setup here instead of provisioning \
             from the panel"
        );
        // Refused before probing: the transport is never touched, and no
        // plan is retained for a later confirmation to execute.
        assert!(backend.probe.lock().unwrap().is_some());
        assert!(backend.operations.lock().unwrap().is_empty());
        assert!(service.memory.lock().await.plans.is_empty());
    }

    /// A unit file that exists but names no command this parser can
    /// classify must fail CLOSED. "I cannot tell what this runs" is not
    /// "there is nothing here": answering the generic run-setup message
    /// would invite the operator to have setup overwrite a file whose
    /// contents nobody understood.
    #[farhelm_testtrace::test]
    async fn an_unclassifiable_local_unit_is_reported_as_somebody_elses() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        *backend.user_unit.lock().unwrap() = Ok(Some(
            "[Service]\nType=oneshot\nExecStop=/bin/true\n".to_string(),
        ));
        let service = service(&harness, backend, root.path());
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Local,
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Manual { reason } = response else {
            panic!("an unreadable local unit is not permission to install")
        };
        assert_eq!(
            reason,
            "farhelm-supervisor.service on this machine already runs this farhelm and was written \
             by hand; it is not touched from the hosts panel"
        );
    }

    /// A supervisor that is actually running on the helm's machine is
    /// discovered and registered, its unit untouched — including the case
    /// that matters most, where `farhelm helm setup` wrote that unit and
    /// it names this very binary. The ownership rule exists to stop the
    /// panel INSTALLING here, and must never stop it from adopting what
    /// is already running; a helm machine set up with `farhelm helm setup`
    /// joins its own fleet exactly this way.
    #[farhelm_testtrace::test]
    async fn a_running_local_supervisor_is_still_discovered() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let farhelm = root.path().join("farhelm");
        std::fs::write(&farhelm, b"#!/bin/sh\n").unwrap();
        let backend = FakeBackend::supervisor(root.path().to_path_buf());
        *backend.user_unit.lock().unwrap() = Ok(Some(crate::units::managed(
            crate::units::render_supervisor_unit(&crate::units::SupervisorUnitInputs {
                farhelm: &farhelm,
                state_dir: &root.path().join("state"),
                tmux: Path::new("/usr/bin/tmux"),
            })
            .unwrap(),
        )));
        let service = ProvisioningService::injected(
            harness.store.clone(),
            Arc::clone(&harness.manager),
            backend.clone(),
            Arc::new(NoPayloads),
            layout(root.path()),
            farhelm.clone(),
        );
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Local,
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        assert!(matches!(response, ProbeResponse::Discovered { .. }));
        // Discovery answered without ever consulting the unit file.
        assert!(backend.unit_reads.lock().unwrap().is_empty());
    }

    /// An ownership lookup that FAILS must stop both panel paths before
    /// any transport work. A permission error or an undecodable unit file
    /// is not evidence that the machine is free to provision — treating
    /// it as absence is how a protected unit gets overwritten by a helm
    /// that merely could not read it.
    #[farhelm_testtrace::test]
    async fn an_unreadable_local_unit_stops_both_panel_paths() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        *backend.user_unit.lock().unwrap() = Err("permission denied".to_string());
        let service = service(&harness, backend.clone(), root.path());

        let add = service
            .probe(ProbeRequest {
                target: ProbeDestination::Local,
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .expect_err("an unreadable unit is not permission to install");
        assert!(add.to_string().contains("permission denied"), "{add}");

        let update = service
            .plan_update(local_row(&harness).await)
            .await
            .expect_err("an unreadable unit is not permission to update");
        assert!(update.to_string().contains("permission denied"), "{update}");
        // The update path refuses before probing; the add path probes
        // first and fails on the lookup that follows, so neither one
        // reached the executor.
        assert!(backend.operations.lock().unwrap().is_empty());
    }

    /// An SSH probe must never consult this machine's unit directory: the
    /// local ownership seam answers a question about the HELM's machine,
    /// and a remote host's supervisor has nothing to do with it.
    #[farhelm_testtrace::test]
    async fn an_ssh_probe_never_reads_a_local_unit() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "remote.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        assert!(matches!(response, ProbeResponse::Provisionable { .. }));
        assert!(backend.unit_reads.lock().unwrap().is_empty());
    }

    /// The unit provisioning pushes to a host carries the reviewed
    /// lifecycle policy, stays free of the setup ownership marker (D9:
    /// remote units belong to the provisioning workflow, and a marked one
    /// would invite `farhelm helm setup` on that host to rewrite it), and
    /// refuses a path it cannot represent before any plan is offered.
    ///
    /// The escaping and PATH-composition rules themselves are exercised in
    /// `crate::units`, which owns them.
    #[farhelm_testtrace::test]
    fn the_provisioned_unit_carries_policy_and_no_ownership_marker() {
        let unit = supervisor_unit(
            Path::new("/tmp/%h/farhelm"),
            Path::new("/tmp/state"),
            Path::new("/tmp/%h/tmux"),
        )
        .unwrap();
        assert!(unit.contains("/tmp/%%h/farhelm"));
        assert!(unit.contains("PATH=/tmp/%%h:"));
        assert!(unit.contains("KillMode=process"));
        assert!(!unit.contains("After=default.target"));
        assert!(!unit.contains('@'), "{unit}");
        assert!(!crate::units::is_managed(&unit), "{unit}");
        assert!(
            supervisor_unit(
                Path::new("/tmp/farhelm"),
                Path::new("/tmp/state\nother"),
                Path::new("/tmp/tmux"),
            )
            .is_err()
        );
    }

    /// Remote shell words round-trip hostile but representable paths and
    /// reject bytes that cannot cross the text command boundary unchanged.
    #[farhelm_testtrace::test]
    fn remote_shell_path_encoding_round_trips_and_rejects_text_hazards() {
        for path in ["/tmp/a b", "/tmp/it's-here", "-leading-dash"] {
            let encoded = shell_path(Path::new(path)).unwrap();
            let output = std::process::Command::new("sh")
                .args(["-c", &format!("set -- {encoded}; printf '%s' \"$1\"")])
                .output()
                .unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, path.as_bytes());
        }
        assert!(shell_path(Path::new("/tmp/line\nbreak")).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let path = PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', 0xff]));
            assert!(shell_path(&path).is_err());
        }
    }

    /// SFTP batch paths use double-quote escaping of their own and reject
    /// record-breaking bytes before a batch is written.
    #[farhelm_testtrace::test]
    fn sftp_batch_path_encoding_has_an_independent_grammar() {
        for (path, expected) in [
            ("/tmp/a b", "\"/tmp/a b\""),
            ("/tmp/a\"b", "\"/tmp/a\\\"b\""),
            ("/tmp/a\\b", "\"/tmp/a\\\\b\""),
        ] {
            assert_eq!(sftp_path(Path::new(path)).unwrap(), expected);
        }
        for rejected in ["/tmp/a\nb", "/tmp/a\rb", "/tmp/a\0b"] {
            assert!(sftp_path(Path::new(rejected)).is_err());
        }
    }

    /// The locale-stable production linger classifier degrades only known
    /// authorization refusals; unrelated command failures remain fatal.
    #[farhelm_testtrace::test]
    fn linger_classifier_separates_refusal_from_failure() {
        assert!(linger_was_refused(Some(1), "Access denied"));
        assert!(linger_was_refused(
            Some(1),
            "Interactive authentication required"
        ));
        assert!(!linger_was_refused(Some(1), "Failed to connect to bus"));
        assert!(!linger_was_refused(Some(0), "Access denied"));
    }

    /// Remote absence has a dedicated exit while inspection failures retain
    /// stderr instead of being collapsed into `None`.
    #[farhelm_testtrace::test]
    async fn remote_metadata_distinguishes_absence_from_hash_failure() {
        let root = tempfile::tempdir().unwrap();
        let target = ProvisioningTarget::Ssh {
            destination: "scripted.example".to_string(),
        };
        let absent = ScriptLauncher::new(["exit 44".to_string()]);
        let mut backend = test_system_backend(root.path());
        backend.launcher = absent;
        assert!(
            backend
                .metadata_on_target(&target, Path::new("/missing"))
                .await
                .unwrap()
                .is_none()
        );

        let broken =
            ScriptLauncher::new(["printf 'sha256sum missing\\n' >&2; exit 127".to_string()]);
        backend.launcher = broken;
        let error = backend
            .metadata_on_target(&target, Path::new("/unreadable"))
            .await
            .expect_err("hash failures are not absence");
        assert!(error.rendered().contains("sha256sum missing"));
    }

    /// Local convergence streams hashes, installs one immutable payload
    /// snapshot, refuses unsafe temporary state, and preserves installed
    /// bytes across every pre-rename failure.
    #[farhelm_testtrace::test]
    async fn local_atomic_install_converges_binary_and_unit_content() {
        let root = tempfile::tempdir().unwrap();
        let mut backend = test_system_backend(root.path());
        let target = ProvisioningTarget::Local;
        for (name, mode) in [("farhelm", 0o755), ("unit.service", 0o644)] {
            let destination = root.path().join(name);
            let temporary = root.path().join(format!(".{name}.tmp"));
            let source = root.path().join(format!("{name}.source"));
            tokio::fs::write(&source, b"first").await.unwrap();
            install_test_payload(&backend, &target, &source, &destination, &temporary, mode)
                .await
                .unwrap();
            set_mode(&destination, 0o600).await.unwrap();
            assert!(matches!(
                install_test_payload(&backend, &target, &source, &destination, &temporary, mode)
                    .await
                    .unwrap(),
                ActionOutcome::Skipped(_)
            ));
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                tokio::fs::metadata(&destination)
                    .await
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o7777,
                mode
            );
            tokio::fs::write(&source, b"second").await.unwrap();
            install_test_payload(&backend, &target, &source, &destination, &temporary, mode)
                .await
                .unwrap();
            assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"second");
            tokio::fs::write(&source, b"third").await.unwrap();
            backend.fail_before_rename = true;
            assert!(
                install_test_payload(&backend, &target, &source, &destination, &temporary, mode)
                    .await
                    .is_err()
            );
            backend.fail_before_rename = false;
            assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"second");
            assert!(!temporary.exists());
        }

        let source = root.path().join("snapshot-source");
        let destination = root.path().join("snapshot-destination");
        let temporary = root.path().join(".snapshot-destination.tmp");
        tokio::fs::write(&source, b"validated bytes").await.unwrap();
        let prepared = stage_payload(&source).await.unwrap();
        tokio::fs::write(&source, b"replacement bytes")
            .await
            .unwrap();
        backend
            .install_path(
                &target,
                PayloadKind::Farhelm,
                &prepared,
                &destination,
                &temporary,
                0o755,
            )
            .await
            .unwrap();
        assert_eq!(
            tokio::fs::read(&destination).await.unwrap(),
            b"validated bytes"
        );

        #[cfg(unix)]
        {
            let victim = root.path().join("symlink-victim");
            let destination = root.path().join("symlink-destination");
            let temporary = root.path().join(".symlink-destination.tmp");
            let source = root.path().join("symlink-source");
            tokio::fs::write(&victim, b"victim").await.unwrap();
            tokio::fs::write(&source, b"installed").await.unwrap();
            std::os::unix::fs::symlink(&victim, &temporary).unwrap();
            install_test_payload(&backend, &target, &source, &destination, &temporary, 0o755)
                .await
                .unwrap();
            assert_eq!(tokio::fs::read(&victim).await.unwrap(), b"victim");
            assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"installed");
        }

        let blocked_temporary = root.path().join("blocked-temporary");
        let blocked_destination = root.path().join("blocked-destination");
        let blocked_source = root.path().join("blocked-source");
        tokio::fs::create_dir(&blocked_temporary).await.unwrap();
        tokio::fs::write(&blocked_destination, b"preserved")
            .await
            .unwrap();
        tokio::fs::write(&blocked_source, b"replacement")
            .await
            .unwrap();
        let error = install_test_payload(
            &backend,
            &target,
            &blocked_source,
            &blocked_destination,
            &blocked_temporary,
            0o755,
        )
        .await
        .expect_err("a non-file temporary cannot be ignored as absent");
        assert!(error.context.contains("removing temporary file"));
        assert_eq!(
            tokio::fs::read(&blocked_destination).await.unwrap(),
            b"preserved"
        );

        let large = root.path().join("large-installed-artifact");
        let large_bytes = vec![b'x'; PAYLOAD_COPY_BUFFER * 3 + 17];
        tokio::fs::write(&large, &large_bytes).await.unwrap();
        let metadata = backend
            .metadata_on_target(&target, &large)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(metadata.hash, hex_sha256(&large_bytes));

        let unit = root.path().join("content.service");
        let unit_temporary = root.path().join(".content.service.tmp");
        backend
            .install_bytes(&target, b"first unit", &unit, &unit_temporary, 0o644)
            .await
            .unwrap();
        assert!(matches!(
            backend
                .install_bytes(&target, b"first unit", &unit, &unit_temporary, 0o644,)
                .await
                .unwrap(),
            ActionOutcome::Skipped(_)
        ));
        backend
            .install_bytes(&target, b"second unit", &unit, &unit_temporary, 0o644)
            .await
            .unwrap();
        backend.fail_before_rename = true;
        assert!(
            backend
                .install_bytes(&target, b"third unit", &unit, &unit_temporary, 0o644,)
                .await
                .is_err()
        );
        assert_eq!(tokio::fs::read(&unit).await.unwrap(), b"second unit");
        assert!(!unit_temporary.exists());
    }

    /// Step transitions publish feed revisions and retain running/pending,
    /// completed/skipped/degraded, and failed states for rereads.
    #[farhelm_testtrace::test]
    async fn per_step_progress_and_feed_transitions_are_observable() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        *backend.skip.lock().unwrap() = Some("install-farhelm".to_string());
        *backend.linger.lock().unwrap() = Some(Err("permission denied".to_string()));
        backend
            .block_first
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let provisioner = service(&harness, backend.clone(), root.path());
        let mut revisions = harness.manager.events().subscribe();
        let initial_revision = *revisions.borrow_and_update();
        let response = provisioner
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "per-step.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("expected plan")
        };
        let accepted = provisioner
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        backend.entered.notified().await;
        let running = provisioner.view(accepted.host_id).await.unwrap();
        assert_eq!(running.steps[0].status, StepStatus::Running);
        assert!(
            running.steps[1..]
                .iter()
                .all(|step| step.status == StepStatus::Pending)
        );
        revisions.changed().await.unwrap();
        assert!(*revisions.borrow_and_update() > initial_revision);
        backend.release.notify_one();
        let completed = wait_finished(&provisioner, accepted.host_id).await;
        assert!(
            completed
                .steps
                .iter()
                .any(|step| step.status == StepStatus::Completed)
        );
        assert!(
            completed
                .steps
                .iter()
                .any(|step| step.status == StepStatus::Skipped)
        );
        assert!(
            completed
                .steps
                .iter()
                .any(|step| step.status == StepStatus::Degraded)
        );

        let failing = FakeBackend::absent(root.path().to_path_buf());
        *failing.fail.lock().unwrap() = Some("daemon-reload".to_string());
        let failing_service = service(&harness, failing, root.path());
        let response = failing_service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "per-step-failing.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("expected plan")
        };
        let accepted = failing_service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        let failed = wait_finished(&failing_service, accepted.host_id).await;
        assert!(
            failed
                .steps
                .iter()
                .any(|step| step.status == StepStatus::Failed)
        );
    }

    /// Per-kind sources prevent a private-tmux plan from ever substituting
    /// the supervisor executable merely because both artifacts share an
    /// architecture.
    #[derive(Debug)]
    struct MutablePayload {
        farhelm: Mutex<PathBuf>,
        tmux: PathBuf,
    }

    #[async_trait]
    impl PayloadSource for MutablePayload {
        async fn path(&self, payload: PayloadKind, _arch: PayloadArch) -> anyhow::Result<PathBuf> {
            Ok(match payload {
                PayloadKind::Farhelm => self.farhelm.lock().unwrap().clone(),
                PayloadKind::Tmux => self.tmux.clone(),
            })
        }
    }

    /// Failure-safe cleanup for every real user unit created below.
    ///
    /// Drop uses only the nonce-named unit and the fixture's own tmux socket.
    /// It never broad-matches Farhelm units or paths, and therefore remains
    /// safe on assertion failure as well as on the happy path.
    struct UnitGuard {
        unit: String,
        unit_path: PathBuf,
        state_dir: PathBuf,
        cleaned: bool,
    }

    impl UnitGuard {
        /// Finish teardown only after every fixture-owned resource is proven
        /// absent; callers must surface failure instead of trusting Drop.
        fn cleanup(&mut self) -> anyhow::Result<()> {
            let disable = std::process::Command::new("systemctl")
                .args(["--user", "--runtime", "disable", "--now", "--", &self.unit])
                .output()
                .context("stopping the nonce provisioning unit")?;
            if !disable.status.success()
                && !String::from_utf8_lossy(&disable.stderr).contains("does not exist")
            {
                bail!(
                    "stopping nonce unit {} failed: {}",
                    self.unit,
                    String::from_utf8_lossy(&disable.stderr)
                );
            }
            match std::fs::remove_file(&self.unit_path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("removing the fixture-owned unit file"),
            }
            let reload = std::process::Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .output()
                .context("reloading after nonce unit removal")?;
            if !reload.status.success() {
                bail!(
                    "reloading after nonce unit removal failed: {}",
                    String::from_utf8_lossy(&reload.stderr)
                );
            }
            let _ = std::process::Command::new("tmux")
                .arg("-S")
                .arg(self.state_dir.join("tmux.sock"))
                .arg("kill-server")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();

            let active = std::process::Command::new("systemctl")
                .args(["--user", "is-active", "--", &self.unit])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .context("verifying nonce unit inactivity")?;
            let enabled = std::process::Command::new("systemctl")
                .args(["--user", "is-enabled", "--", &self.unit])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .context("verifying nonce unit disablement")?;
            let tmux_alive = std::process::Command::new("tmux")
                .arg("-S")
                .arg(self.state_dir.join("tmux.sock"))
                .arg("has-session")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .context("verifying nonce tmux teardown")?;
            if active.success()
                || enabled.success()
                || self.unit_path.exists()
                || tmux_alive.success()
            {
                bail!(
                    "nonce teardown verification failed: active={}, enabled={}, file={}, tmux={}",
                    active.success(),
                    enabled.success(),
                    self.unit_path.exists(),
                    tmux_alive.success()
                );
            }
            self.cleaned = true;
            Ok(())
        }
    }

    /// Exercise an error return while the checked teardown guard is live.
    fn planted_failure(mut guard: UnitGuard) -> anyhow::Result<()> {
        guard.cleanup()?;
        bail!("planted failure")
    }

    impl Drop for UnitGuard {
        fn drop(&mut self) {
            if self.cleaned {
                return;
            }
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "--runtime", "disable", "--now", "--", &self.unit])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = std::fs::remove_file(&self.unit_path);
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = std::process::Command::new("tmux")
                .arg("-S")
                .arg(self.state_dir.join("tmux.sock"))
                .arg("kill-server")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            eprintln!(
                "provisioning test teardown fell back to Drop for nonce unit {}; explicit cleanup did not complete",
                self.unit
            );
        }
    }

    async fn user_manager_available() -> bool {
        tokio::process::Command::new("systemctl")
            .args(["--user", "show-environment"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .is_ok_and(|status| status.success())
    }

    /// Can this machine reach `destination` unattended, over the user's own
    /// ssh configuration, with no prompt of any kind?
    ///
    /// The option set is the transport's own: `BatchMode` refuses to ask for
    /// a passphrase or password, and `StrictHostKeyChecking=yes` refuses to
    /// ask about an unknown host key. A destination that fails this check
    /// would fail every ssh the provisioning run makes, so answering it here
    /// turns an unusable environment into a named skip instead of a
    /// mid-install failure.
    ///
    /// Takes the destination rather than assuming `localhost`: the same
    /// probe guards the ssh-to-localhost variant and the remote one
    /// [`ssh_test_destination`] selects.
    async fn ssh_available(destination: &str) -> bool {
        tokio::process::Command::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "StrictHostKeyChecking=yes",
                "-o",
                "ConnectTimeout=10",
                destination,
                "true",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .is_ok_and(|status| status.success())
    }

    /// The ssh destination the real-provisioning case installs onto, when
    /// something outside the test names one.
    ///
    /// This is the switch between the case's two shapes, and it exists
    /// because a helm and the host it provisions are only ever the same
    /// machine in a test. `scripts/test-provision-centos.sh` sets it to an
    /// ssh alias for a systemd-booted CentOS Stream 9 container, which is
    /// how the suite covers a helm on one distribution installing onto
    /// another — the case the removed `ID=ubuntu` gate used to forbid, and
    /// one no GitHub-hosted runner can otherwise stand in for.
    ///
    /// Absent — a developer laptop, and CI's own Ubuntu runner — the case
    /// keeps its localhost shape unchanged. Present, it becomes remote:
    /// installs land in the target's own default paths and every
    /// post-condition is read back through the helm, because the test
    /// process cannot see the target's filesystem.
    ///
    /// READ, never written. Tests in this binary share one process, so a
    /// test that set environment variables would race every other one; the
    /// value is supplied by whoever launched `cargo test`.
    fn ssh_test_destination() -> Option<String> {
        std::env::var("FARHELM_TEST_SSH_DESTINATION")
            .ok()
            .map(|destination| destination.trim().to_string())
            .filter(|destination| !destination.is_empty())
    }

    /// Resolve the executable payload contract for real provisioning tests.
    ///
    /// `FARHELM_TEST_BINARY` lets clean or nonstandard target layouts name a
    /// known artifact. The conventional workspace debug path remains a
    /// convenience for the CI sequence, which builds binaries before tests.
    ///
    /// It is also what makes a REMOTE target possible at all: the workspace
    /// debug binary is linked against the runner's glibc and would not
    /// execute on the CentOS Stream 9 container, so
    /// `scripts/test-provision-centos.sh` points this at the musl-static
    /// build the release actually ships. `FARHELM_TEST_TMUX` is its partner
    /// in [`debug_tmux`]; a remote run needs both, because a plan installs
    /// whichever of the two payloads the target lacks.
    fn debug_farhelm() -> Option<PathBuf> {
        if let Some(configured) = std::env::var_os("FARHELM_TEST_BINARY") {
            let configured = PathBuf::from(configured);
            assert!(
                configured.is_file(),
                "FARHELM_TEST_BINARY does not name a file: {}",
                configured.display()
            );
            return Some(configured);
        }
        let test_binary = std::env::current_exe().expect("locate the test binary");
        let debug_dir = test_binary
            .parent()
            .and_then(Path::parent)
            .expect("the test binary lives under target/debug/deps");
        let payload = debug_dir.join(format!("farhelm{}", std::env::consts::EXE_SUFFIX));
        payload.is_file().then_some(payload)
    }

    /// Locate a real tmux payload independently of Farhelm. The normal
    /// localhost reach check uses this installation directly, while the
    /// source remains available if the private-payload branch is selected.
    ///
    /// `FARHELM_TEST_TMUX` overrides the PATH lookup exactly as
    /// `FARHELM_TEST_BINARY` overrides [`debug_farhelm`]'s, and for the same
    /// reason: this host's tmux is a dynamically linked distribution package
    /// that cannot run on the target, and a remote target with no tmux of
    /// its own is precisely the case where the plan pushes this payload.
    /// The two variables are a pair — override one for a remote run and you
    /// must override the other.
    fn debug_tmux() -> Option<PathBuf> {
        if let Some(configured) = std::env::var_os("FARHELM_TEST_TMUX") {
            let configured = PathBuf::from(configured);
            assert!(
                configured.is_file(),
                "FARHELM_TEST_TMUX does not name a file: {}",
                configured.display()
            );
            return Some(configured);
        }
        let output = std::process::Command::new("sh")
            .args(["-c", "command -v tmux"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let path = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
        path.is_absolute().then_some(path)
    }

    /// How long [`wait_real_run`] tolerates seeing the SAME step list
    /// (no step's status changed) before declaring a run wedged.
    ///
    /// Bug history: this test used to poll under a single 60s
    /// `tokio::time::timeout` covering the whole run, which conflated "no
    /// progress" with "real work, summed across every step, took a while".
    /// On 2026-08-18 the real ADD-or-UPDATE run lost that budget during a
    /// heavily loaded full-suite run and passed in isolation right after;
    /// the shape is consistent with a contention-slowed run that was still
    /// advancing, though the original failure kept no progress trace to
    /// prove it. Either way a flat per-run deadline cannot tell that apart
    /// from an actually stuck run: it must be widened past whatever
    /// contention needs (a moving target with no honest ceiling) or replaced
    /// with a progress-rearmed one. This is that replacement, mirroring the
    /// stall-timeout shape `uploads.rs`'s `CLIENT_UPLOAD_STALL_TIMEOUT`
    /// already uses for the same reason.
    ///
    /// Sized to clear the LONGEST phase that legitimately holds one step at
    /// `Running` with no externally visible change, with scheduling margin
    /// on top. That phase is not `attach-supervisor`'s `ATTACH_TIMEOUT`
    /// (30s, `provisioning/service.rs`) but an SSH payload install, whose
    /// sub-operations each carry their own budget (`provisioning/backend.rs`:
    /// inspect and remove under `COMMAND_TIMEOUT` 30s each, the transfer
    /// under `TRANSFER_TIMEOUT` 60s, verify-and-install under
    /// `COMMAND_TIMEOUT` 30s) and can sum to about 150s of silence while
    /// every one of them is healthy. Both real-run tests share this wait —
    /// the ssh-to-localhost one exercises exactly that path — so the window
    /// must not mistake a slow install for a wedge.
    const STEP_STALL_TIMEOUT: Duration = Duration::from_secs(240);

    /// Absolute bound on how long [`wait_real_run`] keeps polling, on top
    /// of the stall detector.
    ///
    /// This bounds the WAIT, not the run: it exists so a bug in the
    /// progress bookkeeping (or a run whose steps keep flipping without
    /// ever reaching a terminal status) cannot hang the suite, and it is
    /// deliberately above the worst legitimate run. A full SSH update can
    /// perform three compound installs plus reload, enable, restart, and
    /// attach, whose per-operation budgets sum to roughly ten minutes
    /// before preflight, payload staging, and contention are counted — so
    /// the cap sits well past that rather than being tuned to typical
    /// runs. Expiry is reported neutrally: it says the wait ran out, not
    /// which side is at fault.
    const OVERALL_RUN_DEADLINE: Duration = Duration::from_secs(1200);

    /// One observation of a run's step list, as [`RunProgressWatch`] sees
    /// it: each step's name paired with its status, in plan order.
    type StepSnapshot = Vec<(String, StepStatus)>;

    /// The stall-versus-slow decision behind [`wait_real_run`], kept apart
    /// from the polling loop so it can be driven with chosen instants.
    ///
    /// The contract: a snapshot that differs from the previous one (any
    /// step's status changed, or a step appeared) rearms the stall window;
    /// an identical snapshot — including one that is merely an equal clone
    /// — does not; the stall window and the overall bound are judged at the
    /// instant the caller passes, never from a clock read inside. Terminal
    /// statuses are the caller's business: this type only answers "is
    /// waiting still justified at `now`", and the error it returns carries
    /// the last snapshot so a timeout report is self-describing.
    ///
    /// Exists because the previous version of this logic lived inline in
    /// the poll loop and had no deterministic test at all — its only
    /// exercise was the real provisioning runs, which use the host clock
    /// and cannot reach either timeout on purpose.
    struct RunProgressWatch {
        stall_window: Duration,
        last_steps: Option<StepSnapshot>,
        stall_deadline: tokio::time::Instant,
        overall_deadline: tokio::time::Instant,
    }

    impl RunProgressWatch {
        fn new(
            started: tokio::time::Instant,
            stall_window: Duration,
            overall: Duration,
        ) -> RunProgressWatch {
            RunProgressWatch {
                stall_window,
                last_steps: None,
                stall_deadline: started + stall_window,
                overall_deadline: started + overall,
            }
        }

        /// The earliest instant at which the next observation could fail,
        /// which is what a caller should race its blocking reads against:
        /// a read that outlives this instant has itself become the stall.
        fn next_deadline(&self) -> tokio::time::Instant {
            self.stall_deadline.min(self.overall_deadline)
        }

        /// Record `steps` as seen at `now`; `Err` means waiting is no longer
        /// justified, with the reason and the last snapshot spelled out.
        fn observe(
            &mut self,
            steps: StepSnapshot,
            now: tokio::time::Instant,
        ) -> Result<(), String> {
            if self.last_steps.as_ref() != Some(&steps) {
                self.last_steps = Some(steps);
                self.stall_deadline = now + self.stall_window;
            } else if now >= self.stall_deadline {
                return Err(format!(
                    "no step progress in {:?} (looks wedged, not merely slow); last observed steps: {:?}",
                    self.stall_window, self.last_steps
                ));
            }
            if now >= self.overall_deadline {
                return Err(format!(
                    "the overall wait bound elapsed while steps were still changing; last observed \
                     steps: {:?}",
                    self.last_steps
                ));
            }
            Ok(())
        }
    }

    /// Poll a real (non-simulated) provisioning run to completion.
    ///
    /// Progress is judged from the step list [`ProvisioningView`] already
    /// exposes, not from wall clock alone: every poll that observes a
    /// step's status change rearms [`STEP_STALL_TIMEOUT`], so a run that is
    /// merely slow under contention keeps getting fresh budget for as long
    /// as it keeps moving, while one that truly wedges is caught within
    /// that window regardless of how long the run has been alive overall.
    /// [`OVERALL_RUN_DEADLINE`] bounds the wait as a whole.
    ///
    /// Each progress read is raced against the watch's next deadline, so a
    /// `view` that blocks (it takes a semaphore, the store, and a mutex)
    /// trips the timeout instead of silently suspending both bounds. A
    /// terminal status is accepted whenever it is observed, even on a read
    /// that completes after the overall bound — the bound limits waiting,
    /// and an answer that arrived is not a wait.
    async fn wait_real_run(
        service: &ProvisioningService,
        host: HostId,
    ) -> anyhow::Result<ProvisioningView> {
        let mut watch = RunProgressWatch::new(
            tokio::time::Instant::now(),
            STEP_STALL_TIMEOUT,
            OVERALL_RUN_DEADLINE,
        );
        loop {
            let view = tokio::time::timeout_at(watch.next_deadline(), service.view(host))
                .await
                .with_context(|| {
                    format!(
                        "real provisioning run did not finish: reading its progress blocked past the \
                         wait's deadline; last observed steps: {:?}",
                        watch.last_steps
                    )
                })??;
            if matches!(view.status, RunStatus::Completed | RunStatus::Failed) {
                return Ok(view);
            }
            let steps: StepSnapshot = view
                .steps
                .iter()
                .map(|step| (step.step.clone(), step.status.clone()))
                .collect();
            if let Err(reason) = watch.observe(steps, tokio::time::Instant::now()) {
                bail!("real provisioning run did not finish: {reason}");
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// The stall-versus-slow rules of [`RunProgressWatch`], pinned with
    /// chosen instants rather than a ticking clock.
    ///
    /// Matters because the watch is the only thing standing between "a
    /// loaded box" and a red suite: a rule that rearmed on an equal clone
    /// would never time out, and one that judged silence from its own
    /// clock reads could not be tested at all. What is pinned: equal
    /// snapshots do not rearm; a status change rearms so silence shorter
    /// than the window after each change is tolerated indefinitely;
    /// silence of exactly the window fails; the overall bound fails even
    /// while steps keep changing; the failure text carries the snapshot.
    #[farhelm_testtrace::test]
    fn run_progress_watch_rearms_only_on_real_change_and_bounds_the_whole_wait() {
        let t0 = tokio::time::Instant::now();
        let at = |secs: u64| t0 + Duration::from_secs(secs);
        let snap = |status: StepStatus| vec![("install".to_string(), status)];
        let stall = Duration::from_secs(10);
        let overall = Duration::from_secs(35);

        let mut watch = RunProgressWatch::new(t0, stall, overall);
        assert!(watch.observe(snap(StepStatus::Running), at(1)).is_ok());
        // A value-equal clone is not progress: the deadline set at t=1 holds.
        assert!(watch.observe(snap(StepStatus::Running), at(10)).is_ok());
        let err = watch
            .observe(snap(StepStatus::Running), at(11))
            .expect_err("silence of a full window must fail");
        assert!(err.contains("no step progress in 10s"), "{err}");
        assert!(
            err.contains("install"),
            "the snapshot must ride along: {err}"
        );

        // Each status change buys a fresh window, so a slow run that keeps
        // moving never trips the stall rule...
        let mut watch = RunProgressWatch::new(t0, stall, overall);
        assert!(watch.observe(snap(StepStatus::Pending), at(0)).is_ok());
        assert!(watch.observe(snap(StepStatus::Running), at(9)).is_ok());
        assert!(watch.observe(snap(StepStatus::Running), at(18)).is_ok());
        assert!(watch.observe(snap(StepStatus::Completed), at(27)).is_ok());
        // ...until the overall bound, which holds even with fresh changes.
        let mut flipped = snap(StepStatus::Completed);
        flipped.push(("reload".to_string(), StepStatus::Running));
        let err = watch
            .observe(flipped, at(35))
            .expect_err("the overall bound must hold despite continual change");
        assert!(err.contains("overall wait bound"), "{err}");
        assert_eq!(watch.next_deadline(), at(35).min(at(35 + 10)));
    }

    /// Wait through the manager's intentional disconnect/retry window until
    /// the host publishes a connected client.
    ///
    /// What it returns is a SNAPSHOT, not a lease. The manager may withdraw
    /// and retire that connection at any moment — see
    /// [`through_the_current_connection`], which is what a call issued on
    /// the result has to be wrapped in.
    async fn wait_real_client(
        manager: &ConnectionManager,
        host: HostId,
    ) -> Arc<crate::client::SupervisorClient> {
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                if let Some(client) = manager.status(host).and_then(|status| status.client) {
                    return client;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("the supervisor did not expose a connected client")
    }

    /// Issue one supervisor call on whatever connection the host has RIGHT
    /// NOW, retrying it across a reconnect the manager decides to make in
    /// the middle.
    ///
    /// A published `Arc<SupervisorClient>` is a snapshot of a connection the
    /// manager owns and may end at any moment. `ConnectionManager::retry_now`
    /// — which `ProvisioningService`'s own `AttachSupervisor` action and
    /// every rediscovery `probe` of an already-registered host call — drops
    /// the live connection and returns as soon as the nudge is SENT, without
    /// waiting for the actor to act on it. So there is always an interval in
    /// which [`wait_real_client`] hands back a client the actor is about to
    /// withdraw, and `manager::retire_withdrawn` then fails everything that
    /// connection was carrying. A test that sampled a client and issued one
    /// request on it was racing that interval with no recovery: on a loaded
    /// machine the request lands on the wrong side and comes back
    /// `SupervisorTransportError::SentUnanswered`, which is how this failed
    /// on CI (the ssh leg against the CentOS container) while passing on
    /// every developer machine. Retiring used to leave such a request parked
    /// forever instead of failing it, so the same race was a hang nobody had
    /// hit rather than an error.
    ///
    /// Only the two CONNECTION-LOSS phases are retried. A refusal from the
    /// supervisor, a wrong reply, or any other error is the failure the test
    /// exists to catch and is reported as one.
    ///
    /// The caller owes idempotence across the retry: this helper cannot know
    /// whether a `SentUnanswered` request was performed before its answer was
    /// lost, so a MUTATION driven through here must carry whatever key makes
    /// a repeat harmless (the create below uses an intent key, which the
    /// supervisor keeps in its own store and therefore honours on a fresh
    /// connection).
    async fn through_the_current_connection<T, F, Fut>(
        manager: &ConnectionManager,
        host: HostId,
        what: &str,
        mut call: F,
    ) -> T
    where
        F: FnMut(Arc<crate::client::SupervisorClient>) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<T>>,
    {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
        loop {
            let client = wait_real_client(manager, host).await;
            let error = match call(client).await {
                Ok(value) => return value,
                Err(error) => error,
            };
            let lost = matches!(
                error.downcast_ref::<crate::client::SupervisorTransportError>(),
                Some(
                    crate::client::SupervisorTransportError::NotSent
                        | crate::client::SupervisorTransportError::SentUnanswered
                )
            );
            assert!(lost, "{what}: {error:#}");
            assert!(
                tokio::time::Instant::now() < deadline,
                "{what}: the host never held a connection long enough to answer; last ending: \
                 {error:#}"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Prove both halves of session survival: tmux still reports a live
    /// process, and the post-restart supervisor can drive its terminal.
    async fn assert_session_operable(client: &crate::client::SupervisorClient, session_id: &str) {
        let listing = client.list_sessions().await.expect("list live session");
        let session = listing
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .expect("the tmux-held session must remain listed");
        assert!(
            session.status.is_live(),
            "the retained session is not running: {:?}",
            session.status
        );

        let (channel, mut terminal) = client
            .attach(session_id, 80, 24)
            .await
            .expect("attach to the retained session");
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match terminal.recv().await {
                    Some(crate::client::TermEvent::ReplayComplete) => break,
                    Some(crate::client::TermEvent::Data(_)) => {}
                    Some(crate::client::TermEvent::Detached(reason)) => {
                        panic!("retained terminal detached during replay: {reason}")
                    }
                    None => panic!("retained terminal ended during replay"),
                }
            }
        })
        .await
        .expect("retained terminal replay did not complete");

        let token = format!("farhelmoperable{}", uuid::Uuid::new_v4().simple());
        let marker = token.to_ascii_uppercase();
        client
            .send_input(
                channel,
                format!("printf '%s\\n' '{token}' | tr '[:lower:]' '[:upper:]'\n").into_bytes(),
            )
            .await;
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut output = Vec::new();
            loop {
                match terminal.recv().await {
                    Some(crate::client::TermEvent::Data(bytes)) => {
                        output.extend(bytes);
                        if String::from_utf8_lossy(&output).contains(&marker) {
                            break;
                        }
                    }
                    Some(crate::client::TermEvent::ReplayComplete) => {}
                    Some(crate::client::TermEvent::Detached(reason)) => {
                        panic!("retained terminal detached before accepting input: {reason}")
                    }
                    None => panic!("retained terminal ended before accepting input"),
                }
            }
        })
        .await
        .expect("retained terminal did not echo post-restart input");
        client.detach(channel).await;
    }

    /// Exercise the complete installer against a real user manager, through
    /// direct local process/file operations, through ssh+sftp to localhost,
    /// or through ssh+sftp to a genuinely different machine.
    ///
    /// The two SELF-DIRECTED shapes (local, and ssh to localhost) keep every
    /// install/state path fixture-owned and linger simulated: the unit file
    /// stays below the fixture root and the user manager sees it only
    /// through a nonce-scoped runtime link removed by [`UnitGuard`] on every
    /// exit. That isolation exists because the target is the developer's own
    /// machine, and a test has no business installing into the real
    /// `~/.local/lib/farhelm`.
    ///
    /// The REMOTE shape — selected by [`ssh_test_destination`] — inverts
    /// that. The target is a disposable container on another distribution,
    /// so it installs into the target's own default paths (which is the
    /// layout the product actually ships) and there is nothing on this
    /// machine to guard. It also cannot assert on files: this process can
    /// see none of the target's filesystem, so convergence is read back
    /// through the helm instead — the run completes, the host connects, a
    /// session spawns and stays operable across an UPDATE that replaced the
    /// binary underneath it. Every fixture-path assertion below is therefore
    /// gated on `remote`, with the reason at the site.
    ///
    /// The cases enter through different panel actions since D1 — SSH
    /// through ADD, local through UPDATE — because the local row no longer
    /// offers to install anything. See the comment at the entry point below;
    /// everything after it is shared.
    async fn real_provisioning_case(use_ssh: bool, update: bool) {
        // An explicitly named destination is the remote shape; the ssh case
        // otherwise dials this machine as `localhost`, exactly as before.
        let remote_destination = use_ssh.then(ssh_test_destination).flatten();
        let remote = remote_destination.is_some();
        let destination = remote_destination.unwrap_or_else(|| "localhost".to_string());
        // Skips name the destination, so a log that says a run was skipped
        // also says WHICH host went uncovered. A remote leg exists to prove
        // one specific host got provisioned; "skipped" without that name is
        // indistinguishable from the localhost run that always skips on a
        // manager-less runner.
        let test_name = if use_ssh {
            format!("provisioning_over_ssh_to_{destination}")
        } else {
            "provisioning_over_the_direct_local_transport".to_string()
        };
        // A user manager is required HERE only when the install lands here.
        // The remote shape installs nothing on this machine, and the runner
        // that hosts the container has no user manager of its own — gating
        // on one would skip the leg exactly where it is meant to run.
        if !remote && !user_manager_available().await {
            eprintln!("SKIPPED {test_name}: no usable systemd user manager exists on this host");
            return;
        }
        if use_ssh && !ssh_available(&destination).await {
            eprintln!(
                "SKIPPED {test_name}: passwordless, already-trusted ssh to {destination} is unavailable"
            );
            return;
        }

        let Some(payload) = debug_farhelm() else {
            eprintln!(
                "SKIPPED {test_name}: no Farhelm payload exists at the workspace debug path; build it first or set FARHELM_TEST_BINARY"
            );
            return;
        };
        let Some(tmux_payload) = debug_tmux() else {
            eprintln!(
                "SKIPPED {test_name}: no absolute tmux executable is available; install one or set FARHELM_TEST_TMUX"
            );
            return;
        };
        let root = tempfile::tempdir().expect("isolated provisioning root");
        let lib_dir = root.path().join("lib");
        let supervisor_state = root.path().join("supervisor-state");
        let probe_state = root.path().join("probe-state-never-created");
        let probe_farhelm = root.path().join("probe-farhelm-never-created");
        let unit_dir = root.path().join("units");
        let helm_state = if use_ssh {
            root.path().join("helm-state")
        } else {
            supervisor_state.clone()
        };
        tokio::fs::create_dir_all(&helm_state).await.unwrap();
        // The unit name stays nonce-scoped in every shape. On this machine
        // that is what keeps the fixture from colliding with a real
        // installation; on the container it costs nothing and makes the run
        // identifiable in the target's journal.
        let unit = format!("farhelm-provisioning-test-{}.service", uuid::Uuid::new_v4());
        let unit_path = unit_dir.join(&unit);
        // No guard for a remote target: every resource it verifies —
        // this user manager's units, the fixture's tmux socket — belongs to
        // the machine running the test, and the remote install owns none of
        // them. The container is the teardown.
        let mut guard = (!remote).then(|| UnitGuard {
            unit: unit.clone(),
            unit_path: unit_path.clone(),
            state_dir: supervisor_state.clone(),
            cleaned: false,
        });
        let store = HelmStore::open(&helm_state.join("helm.db")).await.unwrap();
        let manager = ConnectionManager::start(
            store.clone(),
            Arc::new(crate::manager::SystemTransport::new(&helm_state)),
            crate::manager::Cadence::default(),
        )
        .await
        .unwrap();
        let payloads = Arc::new(MutablePayload {
            farhelm: Mutex::new(payload.clone()),
            tmux: tmux_payload,
        });
        let launcher = RecordingLauncher::new();
        // Linger stays simulated in every shape, including the remote one:
        // `scripts/test-provision-centos.sh` already enables it on the
        // container's account (the user manager has to survive the ssh
        // session that starts it), so a real `loginctl enable-linger` here
        // would prove nothing the harness has not already established, and
        // it keeps the backend identical across the three shapes.
        let mut system_backend =
            SystemBackend::with_simulated_linger(helm_state.clone(), Ok(()), true);
        system_backend.launcher = Arc::clone(&launcher) as Arc<dyn CommandLauncher>;
        // Fixture-owned paths exist to keep an install off the DEVELOPER's
        // machine. A remote target is a disposable container, so it takes
        // the production layout instead — which is also the only layout
        // whose directories the plan can be sure exist there, and the one
        // real users get.
        let layout = if remote {
            PlanLayout {
                local_state_dir: supervisor_state.clone(),
                override_lib_dir: None,
                override_farhelm_path: None,
                override_state_dir: None,
                override_unit_dir: None,
                unit_name: unit.clone(),
            }
        } else {
            PlanLayout {
                local_state_dir: supervisor_state.clone(),
                override_lib_dir: Some(lib_dir.clone()),
                override_farhelm_path: None,
                override_state_dir: Some(supervisor_state.clone()),
                override_unit_dir: Some(unit_dir),
                unit_name: unit.clone(),
            }
        };
        let service = ProvisioningService::injected(
            store.clone(),
            Arc::clone(&manager),
            Arc::new(system_backend),
            Arc::clone(&payloads) as Arc<dyn PayloadSource>,
            layout,
            payload.clone(),
        );
        // The two transports enter through different actions now. ADD is
        // the SSH path; the local row has no ADD any more (D1: the panel
        // never installs a supervisor on the helm's own machine, and an
        // absent one is answered with "run farhelm helm setup here"), so
        // the local case enters through the UPDATE action the panel still
        // offers on that row. UPDATE converges the same install from
        // nothing, which is what keeps the direct-local executor — file
        // operations and process spawns with no ssh anywhere — under a
        // real user manager.
        let (accepted, plan) = if use_ssh {
            // The self-directed probe points at paths under the fixture root
            // that are never created, which is how it guarantees "absent"
            // without depending on what this machine happens to have
            // installed. A remote probe passes neither: the container has no
            // supervisor at all, so the target's own defaults answer absent
            // on their own — and letting the probe use them is what makes
            // the recorded coordinates the ones a real ADD would record.
            let (probe_remote_farhelm, probe_remote_state_dir) = if remote {
                (None, None)
            } else {
                (
                    Some(
                        probe_farhelm
                            .to_str()
                            .expect("temp path is UTF-8")
                            .to_string(),
                    ),
                    Some(
                        probe_state
                            .to_str()
                            .expect("temp path is UTF-8")
                            .to_string(),
                    ),
                )
            };
            let response = service
                .probe(ProbeRequest {
                    target: ProbeDestination::Ssh {
                        destination: destination.clone(),
                    },
                    remote_farhelm: probe_remote_farhelm,
                    remote_state_dir: probe_remote_state_dir,
                })
                .await
                .unwrap();
            let ProbeResponse::Provisionable { probe_id, plan, .. } = response else {
                panic!("an absent isolated supervisor must produce a plan")
            };
            let accepted = service
                .start_add(ProvisionRequest { probe_id })
                .await
                .unwrap();
            (accepted, plan)
        } else {
            let local = store
                .list_hosts()
                .await
                .unwrap()
                .into_iter()
                .find(|row| row.kind == HostKind::Local)
                .expect("the reserved local row always exists");
            let planned = service
                .plan_update_for_local_executor_tests(local.id)
                .await
                .unwrap();
            let accepted = service
                .start_update(
                    local.id,
                    ProvisionRequest {
                        probe_id: planned.probe_id,
                    },
                )
                .await
                .unwrap();
            (accepted, planned.plan)
        };
        assert_eq!(
            matches!(plan.target, ProvisioningTarget::Ssh { .. }),
            use_ssh,
            "the local case must never silently become SSH-to-self"
        );
        let completed = match wait_real_run(&service, accepted.host_id).await {
            Ok(completed) => completed,
            Err(error) => {
                service.abort_run(accepted.host_id).await;
                if let Some(guard) = guard.as_mut() {
                    guard
                        .cleanup()
                        .expect("cleanup after the timed-out install");
                }
                panic!("{error:#}");
            }
        };
        assert_eq!(completed.status, RunStatus::Completed, "{completed:?}");
        // Everything in this block reads the TARGET's filesystem through
        // this process's own, which only holds where the target is this
        // machine. The remote shape's equivalent evidence is the connected
        // host and the operable session below: a supervisor cannot answer
        // the protocol hello unless the binary landed, the unit was written,
        // and the user manager started it.
        if !remote {
            assert!(lib_dir.join("farhelm").is_file());
            assert!(unit_path.is_file());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode =
                    |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o7777;
                assert_eq!(mode(&lib_dir), 0o755);
                assert_eq!(mode(&supervisor_state), 0o700);
                assert_eq!(mode(unit_path.parent().unwrap()), 0o755);
            }
            let active = tokio::process::Command::new("systemctl")
                .args(["--user", "is-active", "--", &unit])
                .output()
                .await
                .unwrap();
            assert!(
                active.status.success(),
                "the nonce unit is not active: {}",
                String::from_utf8_lossy(&active.stderr)
            );
        }
        assert!(
            manager
                .state(accepted.host_id)
                .is_some_and(|state| state.is_connected())
        );
        if !use_ssh {
            assert!(
                launcher
                    .programs
                    .lock()
                    .unwrap()
                    .iter()
                    .all(|program| { !program.ends_with("ssh") && !program.ends_with("sftp") }),
                "the direct local case launched ssh or sftp"
            );
        }

        // A rerun of discovery uses the answering supervisor as-is. It does
        // not reinterpret an explicit retry as permission to update — and
        // for the local row this is also the proof that a supervisor which
        // ANSWERS still discovers normally, rather than being swallowed by
        // the refusal an absent one now gets.
        let previous_run = completed.run_id;
        let rerun = service
            .probe(ProbeRequest {
                target: if use_ssh {
                    ProbeDestination::Ssh {
                        destination: destination.clone(),
                    }
                } else {
                    ProbeDestination::Local
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        assert!(matches!(rerun, ProbeResponse::Discovered { .. }));
        assert_eq!(
            service.view(accepted.host_id).await.unwrap().run_id,
            previous_run
        );

        // The working directory has to exist ON THE TARGET. The fixture root
        // does for a self-directed run; a remote target has never heard of
        // it, so use the one directory every POSIX host is required to have.
        let cwd = if remote {
            "/tmp".to_string()
        } else {
            root.path()
                .to_str()
                .expect("temp path is UTF-8")
                .to_string()
        };
        // The rerun probe immediately above ends with `retry_now`, which
        // drops this host's live connection without waiting for the actor,
        // so THIS is the call most likely to be issued on a connection the
        // manager is retiring underneath it — see
        // [`through_the_current_connection`]. The intent key is what makes
        // the retry safe: a create whose answer was lost is replayed rather
        // than repeated, so a second session can never appear behind this
        // test's back.
        let create_key = uuid::Uuid::new_v4().to_string();
        let session = through_the_current_connection(
            &manager,
            accepted.host_id,
            "create an operable session through the provisioned host",
            |client| {
                let cwd = cwd.clone();
                let key = create_key.clone();
                async move {
                    client
                        .create_session_with_key(&cwd, "/bin/sh", None, 80, 24, Some(key))
                        .await
                }
            },
        )
        .await;

        if update {
            let newer = root.path().join("farhelm-newer");
            let mut bytes = tokio::fs::read(&payload).await.unwrap();
            bytes.extend_from_slice(b"farhelm-test-newer-payload");
            tokio::fs::write(&newer, &bytes).await.unwrap();
            set_mode(&newer, 0o755).await.unwrap();
            *payloads.farhelm.lock().unwrap() = newer;
            // The SSH row plans through the panel's own action; the local
            // row has no panel action left and uses the executor seam.
            let update_plan = if use_ssh {
                service.plan_update(accepted.host_id).await.unwrap()
            } else {
                service
                    .plan_update_for_local_executor_tests(accepted.host_id)
                    .await
                    .unwrap()
            };
            let update_run = service
                .start_update(
                    accepted.host_id,
                    ProvisionRequest {
                        probe_id: update_plan.probe_id,
                    },
                )
                .await
                .unwrap();
            let completed = match wait_real_run(&service, update_run.host_id).await {
                Ok(completed) => completed,
                Err(error) => {
                    service.abort_run(update_run.host_id).await;
                    if let Some(guard) = guard.as_mut() {
                        guard.cleanup().expect("cleanup after timed-out UPDATE");
                    }
                    panic!("{error:#}");
                }
            };
            assert_eq!(completed.status, RunStatus::Completed, "{completed:?}");
            if remote {
                // No filesystem to hash. The convergence proof is what the
                // next few lines already do: the UPDATE transferred a
                // DIFFERENT binary (the hash guard would have skipped an
                // identical one, so a completed run means bytes moved),
                // restarted the unit onto it, and the helm reconnects to a
                // supervisor that still owns the session started before the
                // replacement. A payload that failed to land, landed
                // truncated, or lost its execute bit could not produce that.
            } else {
                assert_eq!(
                    hex_sha256(&tokio::fs::read(lib_dir.join("farhelm")).await.unwrap()),
                    hex_sha256(&bytes),
                    "UPDATE must converge to the newer payload"
                );
            }
        }
        // A bare snapshot is enough HERE, unlike at the create above: the
        // only `retry_now` left behind is the UPDATE run's own
        // `AttachSupervisor`, which does not complete until it has observed
        // a NEW incarnation, so `wait_real_run` returning means the
        // reconnect this test could race has already happened and no nudge
        // is outstanding. Nothing after this point asks the manager to
        // reconfigure or re-dial the row.
        let client = wait_real_client(&manager, accepted.host_id).await;
        assert_session_operable(&client, &session.id).await;
        client.delete_session(&session.id).await.unwrap();

        service.abort_run(accepted.host_id).await;
        manager.shutdown();
        drop(service);
        drop(manager);
        drop(store);
        if let Some(guard) = guard.as_mut() {
            guard.cleanup().expect("checked nonce resource teardown");
        }
        drop(guard);
        // Teardown verification is the guard's, and the guard only exists
        // where the resources do. A remote install is left running until the
        // container is destroyed, which is the harness's job, not the test's.
        if !remote {
            assert!(!unit_path.exists(), "the nonce unit file survived teardown");
            let inactive = tokio::process::Command::new("systemctl")
                .args(["--user", "is-active", "--", &unit])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await
                .unwrap();
            assert!(!inactive.success(), "the nonce unit survived teardown");
        }
        let root_path = root.path().to_path_buf();
        drop(root);
        assert!(
            !root_path.exists(),
            "the temporary install/state root survived teardown"
        );
    }

    /// The CI-shaped transport proof: real ssh and sftp, then an SSH UPDATE
    /// that preserves and operates a tmux-held session.
    ///
    /// Destination-agnostic on purpose. By default it dials `localhost` into
    /// fixture-owned paths, which is what a laptop and the Ubuntu CI runner
    /// can offer. With `FARHELM_TEST_SSH_DESTINATION` set it dials that host
    /// instead — `scripts/test-provision-centos.sh` points it at a
    /// systemd-booted CentOS Stream 9 container — and becomes the only
    /// coverage of a helm provisioning a DIFFERENT distribution than its
    /// own, which is the case the removed `ID=ubuntu` gate used to reject
    /// outright. See [`real_provisioning_case`] for how the two shapes
    /// differ.
    #[farhelm_testtrace::test]
    async fn provisioning_and_update_over_ssh_preserve_an_operable_session() {
        real_provisioning_case(true, true).await;
    }

    /// The local path performs no SSH, and the explicit UPDATE replaces the
    /// payload, restarts only the supervisor, and preserves its tmux session.
    ///
    /// Both of this case's runs are UPDATEs now: the hosts panel's local
    /// row stopped offering to install a supervisor (D1), so UPDATE is the
    /// only entry point left into the direct-local executor — and it is a
    /// real one, since a local row whose supervisor unit was not written
    /// by `farhelm helm setup` is still the panel's to converge.
    #[farhelm_testtrace::test]
    async fn local_provisioning_and_update_preserve_a_running_session() {
        real_provisioning_case(false, true).await;
    }

    /// A planted failure is followed by checked teardown of a real active
    /// runtime unit and tmux server, then verified again from the outer scope.
    #[farhelm_testtrace::test]
    async fn teardown_guard_runs_on_failure() {
        if !user_manager_available().await {
            eprintln!(
                "SKIPPED teardown_guard_runs_on_failure: no usable systemd user manager exists on this host"
            );
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let unit = format!("farhelm-provisioning-test-{}.service", uuid::Uuid::new_v4());
        let unit_path = root.path().join(&unit);
        std::fs::write(
            &unit_path,
            "[Service]\nType=simple\nExecStart=/usr/bin/sleep 300\n",
        )
        .unwrap();
        let activated = std::process::Command::new("systemctl")
            .args(["--user", "--runtime", "enable", "--now", "--"])
            .arg(&unit_path)
            .output()
            .unwrap();
        assert!(
            activated.status.success(),
            "failed to activate nonce unit: {}",
            String::from_utf8_lossy(&activated.stderr)
        );
        let tmux_socket = root.path().join("tmux.sock");
        let tmux_started = std::process::Command::new("tmux")
            .arg("-S")
            .arg(&tmux_socket)
            .args(["new-session", "-d", "/usr/bin/sleep", "300"])
            .output()
            .unwrap();
        assert!(
            tmux_started.status.success(),
            "failed to start nonce tmux server: {}",
            String::from_utf8_lossy(&tmux_started.stderr)
        );
        assert!(
            planted_failure(UnitGuard {
                unit: unit.clone(),
                unit_path: unit_path.clone(),
                state_dir: root.path().to_path_buf(),
                cleaned: false,
            })
            .is_err()
        );
        assert!(
            !unit_path.exists(),
            "the unit file survived failure teardown"
        );
        let active = std::process::Command::new("systemctl")
            .args(["--user", "is-active", "--", &unit])
            .status()
            .unwrap();
        let enabled = std::process::Command::new("systemctl")
            .args(["--user", "is-enabled", "--", &unit])
            .status()
            .unwrap();
        let tmux_alive = std::process::Command::new("tmux")
            .arg("-S")
            .arg(tmux_socket)
            .arg("has-session")
            .status()
            .unwrap();
        assert!(!active.success());
        assert!(!enabled.success());
        assert!(!tmux_alive.success());
    }

    /// Build a tiny but real `.tar.gz` archive at `<dir>/<package>-<target>.tar.gz`,
    /// nesting one member under `<package>-<target>/` — the dist archive
    /// layout both `DirectoryPayloads` and `ReleasePayloadSource` must
    /// locate `member` inside by basename rather than by an exact path.
    fn write_release_archive(
        dir: &Path,
        package: &str,
        target: &str,
        member: &str,
        contents: &[u8],
    ) {
        let archive_path = dir.join(format!("{package}-{target}.tar.gz"));
        let file = std::fs::File::create(&archive_path).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(
                &mut header,
                format!("{package}-{target}/{member}"),
                contents,
            )
            .unwrap();
        builder.into_inner().unwrap().finish().unwrap();
    }

    /// Append one bare member (no nesting) directly at `path` inside a tar
    /// builder — used to construct archives with zero or several members
    /// sharing `path`'s basename, which [`write_release_archive`]'s
    /// one-member-per-call shape cannot produce.
    fn append_tar_member(
        builder: &mut tar::Builder<flate2::write::GzEncoder<std::fs::File>>,
        path: &str,
        contents: &[u8],
    ) {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, path, contents).unwrap();
    }

    /// Build a release archive whose one entry named `member` is NOT a
    /// regular file — a directory, symlink, or hard link — the shape F3
    /// (review round 1) rejects outright rather than silently excluding
    /// from the exactly-one count or, worse, staging as if it were the
    /// binary.
    fn write_archive_with_non_regular_member(
        dir: &Path,
        package: &str,
        target: &str,
        member: &str,
        entry_type: tar::EntryType,
    ) {
        let archive_path = dir.join(format!("{package}-{target}.tar.gz"));
        let file = std::fs::File::create(&archive_path).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(entry_type);
        header.set_size(0);
        let entry_path = format!("{package}-{target}/{member}");
        match entry_type {
            tar::EntryType::Symlink | tar::EntryType::Link => {
                builder
                    .append_link(&mut header, &entry_path, "elsewhere-in-the-archive")
                    .unwrap();
            }
            _ => {
                header.set_cksum();
                builder
                    .append_data(&mut header, &entry_path, std::io::empty())
                    .unwrap();
            }
        }
        builder.into_inner().unwrap().finish().unwrap();
    }

    /// List `.extracted/<asset>.*.bin` snapshot files for `asset` inside
    /// `dir`. `DirectoryPayloads`' per-call snapshots are uniquely named
    /// (F2, review round 2), so there is no single fixed destination path
    /// left to assert about — tests instead assert about the whole
    /// matching SET. Built on `read_dir`, which enumerates directory
    /// entries without following them, so a hypothetical dangling symlink
    /// left by a broken implementation would still show up here — unlike
    /// `Path::exists()`, which follows symlinks and would report `false`
    /// for exactly that broken case (F11, review round 2).
    fn extracted_snapshot_files(dir: &Path, asset: &str) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(dir.join(".extracted")) else {
            return Vec::new();
        };
        let prefix = format!("{asset}.");
        entries
            .flatten()
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".bin"))
            })
            .map(|entry| entry.path())
            .collect()
    }

    /// Spec: `DirectoryPayloads` extracts the `farhelm` binary out of the
    /// published archive and copies the bare tmux binary verbatim, for both
    /// provisioning architectures, landing under `dir/.extracted/` with
    /// mode 0755 — the happy path plan lines 467–475 ask for, run against
    /// tiny real archives built in-test rather than committed binary
    /// fixtures (3b commits the signed release fixture set).
    ///
    /// F12 (review round 1) pins the RETURNED PATH's shape — inside
    /// `.extracted/`, named after the published asset — without pinning an
    /// exact filename, since F2 (review round 2) makes every call's
    /// destination a uniquely named private snapshot rather than a name
    /// shared across calls. F9 (review round 2) also assigns the SOURCE
    /// files distinct, non-executable modes before materializing, and
    /// requires those modes — not just their bytes — to survive unchanged:
    /// a regression that `chmod 0755`d the operator's own archive or tmux
    /// binary instead of only the extracted/copied output would satisfy
    /// every earlier assertion here and still be a real bug.
    #[farhelm_testtrace::test]
    async fn directory_payloads_extracts_farhelm_and_copies_tmux_for_both_arches() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().unwrap();
        let mut original_archives = std::collections::HashMap::new();
        let mut original_tmux = std::collections::HashMap::new();
        for arch in [PayloadArch::X86_64, PayloadArch::Aarch64] {
            let archive = farhelm_archive_for(arch);
            write_release_archive(
                dir.path(),
                archive.package,
                archive.target,
                archive.member,
                format!("farhelm-bytes-{arch:?}").as_bytes(),
            );
            let tmux_bytes = format!("tmux-bytes-{arch:?}").into_bytes();
            std::fs::write(dir.path().join(tmux_name(arch)), &tmux_bytes).unwrap();
            // Distinct, non-executable modes so a regression that chmods a
            // SOURCE instead of only its extracted/copied output cannot
            // hide behind a content-only comparison (F9, review round 2).
            std::fs::set_permissions(
                dir.path().join(archive_name(archive)),
                std::fs::Permissions::from_mode(0o640),
            )
            .unwrap();
            std::fs::set_permissions(
                dir.path().join(tmux_name(arch)),
                std::fs::Permissions::from_mode(0o600),
            )
            .unwrap();
            original_archives.insert(
                arch,
                std::fs::read(dir.path().join(archive_name(archive))).unwrap(),
            );
            original_tmux.insert(arch, tmux_bytes);
        }
        let payloads = DirectoryPayloads::new(dir.path().to_path_buf());
        for arch in [PayloadArch::X86_64, PayloadArch::Aarch64] {
            let archive = farhelm_archive_for(arch);
            let extracted_dir = dir.path().join(".extracted");
            let farhelm_asset = archive_name(archive);

            let farhelm_path = payloads.path(PayloadKind::Farhelm, arch).await.unwrap();
            assert_eq!(
                farhelm_path.parent(),
                Some(extracted_dir.as_path()),
                "the returned path must live in dir/.extracted/"
            );
            assert!(
                farhelm_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&format!("{farhelm_asset}."))
                        && name.ends_with(".bin")),
                "the returned filename must be {farhelm_asset}.<unique>.bin, got {farhelm_path:?}"
            );
            assert_eq!(
                std::fs::read(&farhelm_path).unwrap(),
                format!("farhelm-bytes-{arch:?}").as_bytes()
            );
            assert_eq!(
                std::fs::metadata(&farhelm_path)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o755,
                "extracted farhelm binary must be executable"
            );

            let tmux_asset = tmux_name(arch).to_string();
            let tmux_path = payloads.path(PayloadKind::Tmux, arch).await.unwrap();
            assert_eq!(
                tmux_path.parent(),
                Some(extracted_dir.as_path()),
                "the returned path must live in dir/.extracted/"
            );
            assert!(
                tmux_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&format!("{tmux_asset}."))
                        && name.ends_with(".bin")),
                "the returned filename must be {tmux_asset}.<unique>.bin, got {tmux_path:?}"
            );
            assert_eq!(
                std::fs::read(&tmux_path).unwrap(),
                format!("tmux-bytes-{arch:?}").as_bytes()
            );
            assert_eq!(
                std::fs::metadata(&tmux_path).unwrap().permissions().mode() & 0o777,
                0o755,
                "copied tmux binary must be executable"
            );

            assert_eq!(
                std::fs::read(dir.path().join(archive_name(archive))).unwrap(),
                original_archives[&arch],
                "materialization must never mutate the operator's staged archive"
            );
            assert_eq!(
                std::fs::metadata(dir.path().join(archive_name(archive)))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o640,
                "materialization must never change the operator's staged archive's mode"
            );
            assert_eq!(
                std::fs::read(dir.path().join(tmux_name(arch))).unwrap(),
                original_tmux[&arch],
                "materialization must never mutate the operator's staged tmux binary"
            );
            assert_eq!(
                std::fs::metadata(dir.path().join(tmux_name(arch)))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600,
                "materialization must never change the operator's staged tmux binary's mode"
            );
        }
    }

    /// Spec: an absent source asset names the exact path `DirectoryPayloads`
    /// expected to find it at, so an operator staging the directory gets a
    /// specific, actionable path rather than a generic "not found".
    #[farhelm_testtrace::test]
    async fn directory_payloads_missing_file_names_the_expected_path() {
        let dir = tempfile::tempdir().unwrap();
        let payloads = DirectoryPayloads::new(dir.path().to_path_buf());
        let error = payloads
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap_err();
        let expected = dir
            .path()
            .join(archive_name(farhelm_archive_for(PayloadArch::X86_64)));
        assert!(
            format!("{error:#}").contains(&expected.display().to_string()),
            "error must name {}: {error:#}",
            expected.display()
        );
    }

    /// Spec (F10, review round 2): `require_regular_file`'s non-regular
    /// branch is reachable at the published SOURCE path, not only inside a
    /// tar archive (a different code path entirely) — here the expected
    /// archive path is itself a directory.
    #[farhelm_testtrace::test]
    async fn directory_payloads_reports_a_directory_at_the_source_path_as_non_regular() {
        let dir = tempfile::tempdir().unwrap();
        let archive = farhelm_archive_for(PayloadArch::X86_64);
        std::fs::create_dir_all(dir.path().join(archive_name(archive))).unwrap();
        let payloads = DirectoryPayloads::new(dir.path().to_path_buf());
        let error = payloads
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap_err();
        let message = format!("{error:#}");
        assert!(
            message.contains("is not a regular file"),
            "a directory at the source path must be reported as present but non-regular: \
             {message}"
        );
        assert!(
            !message.contains("does not exist"),
            "a directory at the source path must not be reported as absent: {message}"
        );
    }

    /// Spec (F10, review round 2): a metadata failure that is NOT
    /// `NotFound` — a symlink loop, deterministically — must retain its
    /// underlying filesystem error rather than being folded into either
    /// the "missing asset" or the "not a regular file" message. Without
    /// this, swapping `std::fs::metadata` back for `Path::is_file()` could
    /// collapse this case into "missing asset" and the suite would stay
    /// green.
    #[farhelm_testtrace::test]
    async fn directory_payloads_reports_a_symlink_loop_as_an_inspection_failure_not_absence() {
        let dir = tempfile::tempdir().unwrap();
        let archive = farhelm_archive_for(PayloadArch::X86_64);
        let looped_path = dir.path().join(archive_name(archive));
        std::os::unix::fs::symlink(&looped_path, &looped_path).unwrap();
        let payloads = DirectoryPayloads::new(dir.path().to_path_buf());
        let error = payloads
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap_err();
        let message = format!("{error:#}");
        assert!(
            !message.contains("does not exist"),
            "a symlink loop must not be reported as absence: {message}"
        );
        assert!(
            !message.contains("is not a regular file"),
            "a symlink loop is a metadata failure, not a present-but-wrong-type object: {message}"
        );
        assert!(
            message.contains("inspecting"),
            "a symlink loop must retain the underlying inspection failure: {message}"
        );
    }

    /// Spec: zero members named `member` refuses with the exact wording
    /// plan line 460 specifies — an archive whose one file happens not to
    /// be called `farhelm` must not be silently accepted as if it were.
    /// F13 (review round 1) / F11 (review round 2): also proves no
    /// destination is EVER created — checked by listing `.extracted/` for
    /// this asset rather than a single fixed path, per F2's per-call
    /// unique naming.
    #[farhelm_testtrace::test]
    async fn directory_payloads_refuses_an_archive_with_no_matching_member() {
        let dir = tempfile::tempdir().unwrap();
        let archive = farhelm_archive_for(PayloadArch::X86_64);
        write_release_archive(
            dir.path(),
            archive.package,
            archive.target,
            "not-farhelm",
            b"nope",
        );
        let payloads = DirectoryPayloads::new(dir.path().to_path_buf());
        let error = payloads
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            format!(
                "{} contains 0 members named farhelm; expected exactly one",
                archive_name(archive)
            )
        );
        assert!(
            extracted_snapshot_files(dir.path(), &archive_name(archive)).is_empty(),
            "a refused archive must never produce a staged destination"
        );
    }

    /// Spec: several members sharing the same basename refuse the same way
    /// zero does — the extractor must not silently pick one of two
    /// candidates, since a dist archive that ever carried two `farhelm`
    /// entries would mean the release build itself went wrong. F13 (review
    /// round 1) / F11 (review round 2): also proves no destination is
    /// created — see the sibling zero-match test's docstring.
    #[farhelm_testtrace::test]
    async fn directory_payloads_refuses_an_archive_with_several_matching_members() {
        let dir = tempfile::tempdir().unwrap();
        let archive = farhelm_archive_for(PayloadArch::X86_64);
        let archive_path = dir.path().join(archive_name(archive));
        let file = std::fs::File::create(&archive_path).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        append_tar_member(&mut builder, "a/farhelm", b"one");
        append_tar_member(&mut builder, "b/farhelm", b"two");
        builder.into_inner().unwrap().finish().unwrap();
        let payloads = DirectoryPayloads::new(dir.path().to_path_buf());
        let error = payloads
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            format!(
                "{} contains 2 members named farhelm; expected exactly one",
                archive_name(archive)
            )
        );
        assert!(
            extracted_snapshot_files(dir.path(), &archive_name(archive)).is_empty(),
            "a refused archive must never produce a staged destination"
        );
    }

    /// Spec (F3, review round 1, SHOULD-FIX): a same-named directory,
    /// symlink, or hard link entry makes the archive malformed rather than
    /// silently dropping out of the count or becoming staged content —
    /// refused for each shape. F11 (review round 2): the cleanup assertion
    /// lists `.extracted/` for this asset rather than checking a single
    /// fixed path with `Path::exists()`, which follows symlinks and would
    /// have reported `false` for a dangling one left behind by a broken
    /// implementation even though a real entry remained.
    #[farhelm_testtrace::test]
    async fn directory_payloads_refuses_a_non_regular_entry_sharing_the_members_name() {
        for entry_type in [
            tar::EntryType::Directory,
            tar::EntryType::Symlink,
            tar::EntryType::Link,
        ] {
            let dir = tempfile::tempdir().unwrap();
            let archive = farhelm_archive_for(PayloadArch::X86_64);
            write_archive_with_non_regular_member(
                dir.path(),
                archive.package,
                archive.target,
                archive.member,
                entry_type,
            );
            let payloads = DirectoryPayloads::new(dir.path().to_path_buf());
            let error = payloads
                .path(PayloadKind::Farhelm, PayloadArch::X86_64)
                .await
                .unwrap_err();
            assert!(
                format!("{error:#}").contains("not a regular file"),
                "{entry_type:?} entry must be refused as malformed: {error:#}"
            );
            assert!(
                extracted_snapshot_files(dir.path(), &archive_name(archive)).is_empty(),
                "{entry_type:?} entry must never produce a staged destination"
            );
        }
    }

    /// Spec (F13, review round 1) / F2 (review round 2): a malformed
    /// archive must never disturb an existing snapshot that legitimately
    /// belongs to an earlier generation, and must never add a new one of
    /// its own. Seeds `.extracted/<asset>.sentinel.bin` — the shape a real
    /// earlier successful call would have left, since F2 makes every
    /// destination a private per-call snapshot rather than a name shared
    /// across calls — with sentinel bytes and a distinct mode, then
    /// attempts extraction from a several-members archive and requires the
    /// sentinel untouched and no additional snapshot present.
    #[farhelm_testtrace::test]
    async fn directory_payloads_preserves_existing_snapshots_when_extraction_fails() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().unwrap();
        let archive = farhelm_archive_for(PayloadArch::X86_64);
        let archive_path = dir.path().join(archive_name(archive));
        let file = std::fs::File::create(&archive_path).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        append_tar_member(&mut builder, "a/farhelm", b"one");
        append_tar_member(&mut builder, "b/farhelm", b"two");
        builder.into_inner().unwrap().finish().unwrap();

        let extracted_dir = dir.path().join(".extracted");
        std::fs::create_dir_all(&extracted_dir).unwrap();
        let sentinel = extracted_dir.join(format!("{}.sentinel.bin", archive_name(archive)));
        std::fs::write(&sentinel, b"sentinel").unwrap();
        std::fs::set_permissions(&sentinel, std::fs::Permissions::from_mode(0o600)).unwrap();

        let payloads = DirectoryPayloads::new(dir.path().to_path_buf());
        payloads
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap_err();

        assert_eq!(
            std::fs::read(&sentinel).unwrap(),
            b"sentinel",
            "a failed extraction must not touch an existing snapshot's bytes"
        );
        assert_eq!(
            std::fs::metadata(&sentinel).unwrap().permissions().mode() & 0o777,
            0o600,
            "a failed extraction must not touch an existing snapshot's mode"
        );
        assert_eq!(
            extracted_snapshot_files(dir.path(), &archive_name(archive)),
            vec![sentinel],
            "a failed extraction must not add any new snapshot"
        );
    }

    /// Spec (F2, review round 2, DECISION: per-call private snapshot):
    /// `DirectoryPayloads::path` materializes into a UNIQUE file on every
    /// call and never overwrites a name shared with another call. Two
    /// calls straddling a source refresh must return DISTINCT paths, and
    /// each must keep returning the bytes it was actually built from — the
    /// first caller's snapshot surviving the second caller's own
    /// materialization is exactly the generation-handoff safety a shared
    /// destination name did not, by itself, provide.
    #[farhelm_testtrace::test]
    async fn directory_payloads_snapshots_are_private_and_survive_a_later_refresh() {
        let dir = tempfile::tempdir().unwrap();
        let archive = farhelm_archive_for(PayloadArch::X86_64);
        write_release_archive(
            dir.path(),
            archive.package,
            archive.target,
            archive.member,
            b"generation-one",
        );
        std::fs::write(
            dir.path().join(tmux_name(PayloadArch::X86_64)),
            b"tmux-generation-one",
        )
        .unwrap();

        let payloads = DirectoryPayloads::new(dir.path().to_path_buf());
        let farhelm_path_one = payloads
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap();
        let tmux_path_one = payloads
            .path(PayloadKind::Tmux, PayloadArch::X86_64)
            .await
            .unwrap();

        // Replace both staged assets under the SAME published names.
        write_release_archive(
            dir.path(),
            archive.package,
            archive.target,
            archive.member,
            b"generation-two",
        );
        std::fs::write(
            dir.path().join(tmux_name(PayloadArch::X86_64)),
            b"tmux-generation-two",
        )
        .unwrap();

        let farhelm_path_two = payloads
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap();
        let tmux_path_two = payloads
            .path(PayloadKind::Tmux, PayloadArch::X86_64)
            .await
            .unwrap();

        assert_ne!(
            farhelm_path_one, farhelm_path_two,
            "each call must get its own private snapshot file"
        );
        assert_ne!(
            tmux_path_one, tmux_path_two,
            "each call must get its own private snapshot file"
        );
        assert_eq!(std::fs::read(&farhelm_path_two).unwrap(), b"generation-two");
        assert_eq!(
            std::fs::read(&tmux_path_two).unwrap(),
            b"tmux-generation-two"
        );
        // The FIRST caller's snapshot must remain exactly what it was
        // handed — the second call's materialization is not permitted to
        // reach back and disturb it.
        assert_eq!(
            std::fs::read(&farhelm_path_one).unwrap(),
            b"generation-one",
            "an earlier caller's snapshot must survive a later materialization"
        );
        assert_eq!(
            std::fs::read(&tmux_path_one).unwrap(),
            b"tmux-generation-one",
            "an earlier caller's snapshot must survive a later materialization"
        );
    }

    /// Marker set only in the child process
    /// [`directory_payloads_extracted_dir_is_mode_0700_under_a_permissive_umask`]
    /// re-execs itself as.
    const EXTRACTED_MODE_CHILD: &str = "FARHELM_HELM_EXTRACTED_MODE_TEST_CHILD";

    /// Spec (F6, review round 2, security): `.extracted` must end up mode
    /// 0700 regardless of the helm process's umask — proven here under
    /// umask 000, the most permissive real-world case. `umask` is
    /// process-wide state, so this MUST run in a genuinely separate child
    /// process, never merely a different thread of this shared test
    /// binary (which every OTHER concurrently running test also creates
    /// files in) — the same isolation shape `lib.rs`'s env-wiring child
    /// tests use, applied here to a different piece of ambient process
    /// state.
    #[test]
    fn directory_payloads_extracted_dir_is_mode_0700_under_a_permissive_umask() {
        if std::env::var_os(EXTRACTED_MODE_CHILD).is_some() {
            // SAFETY: this process exists only to run this one test in
            // isolation (see the parent branch below), so mutating
            // process-wide umask here can never race or leak into any
            // other test.
            unsafe {
                libc::umask(0o000);
            }
            use std::os::unix::fs::PermissionsExt as _;
            let payload_dir =
                PathBuf::from(std::env::var("FARHELM_HELM_EXTRACTED_MODE_TEST_DIR").unwrap());
            let archive = farhelm_archive_for(PayloadArch::X86_64);
            write_release_archive(
                &payload_dir,
                archive.package,
                archive.target,
                archive.member,
                b"bytes",
            );
            let payloads = DirectoryPayloads::new(payload_dir.clone());
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime
                .block_on(payloads.path(PayloadKind::Farhelm, PayloadArch::X86_64))
                .unwrap();
            let mode = std::fs::metadata(payload_dir.join(".extracted"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            println!("MODE={mode:o}");
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "provisioning::tests::directory_payloads_extracted_dir_is_mode_0700_under_a_permissive_umask",
                "--nocapture",
            ])
            .env(EXTRACTED_MODE_CHILD, "1")
            .env("FARHELM_HELM_EXTRACTED_MODE_TEST_DIR", dir.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "extracted-mode child failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mode = stdout
            .lines()
            .find_map(|line| line.strip_prefix("MODE="))
            .expect("child printed no MODE= line");
        assert_eq!(
            mode, "700",
            "under umask 000 the .extracted directory must still end up mode 0700"
        );
    }

    /// Spec (F9, review round 1, SHOULD-FIX): the exact settled recovery
    /// text (README, "Install") is a user-facing contract — pinned here
    /// with a direct equality assertion so it cannot silently drift while
    /// the broader provisioning-flow test below (which only checks a
    /// substring, because its point is proving the failure happens before
    /// host mutation) stays green regardless.
    #[farhelm_testtrace::test]
    async fn no_payloads_message_is_exactly_the_settled_recovery_text() {
        let error = NoPayloads
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            "this farhelm was built from source and carries no provisioning payloads; pass \
             --payload-dir <dir> holding the release files, or install a release build (see \
             README, \"Install\")"
        );
    }

    /// Spec: `production_payloads` resolves every `PayloadSelection` ×
    /// `release_build` combination the way D13 and plan lines 418–433
    /// describe — `Directory` always yields a working `DirectoryPayloads`;
    /// `Release` always yields a `ReleasePayloadSource` at the given URL,
    /// on a developer build as much as on a release build; `Default` splits
    /// on `release_build`, downloading from this build's own GitHub release
    /// when it is set and refusing with the `NoPayloads` message when it is
    /// not.
    ///
    /// This wiring is the one place D13's policy is expressed, and getting
    /// it wrong is silent: a release build that fell back to `NoPayloads`
    /// would look like a working helm right up until somebody added a host.
    ///
    /// `Directory` is exercised under BOTH values of `release_build` (F8,
    /// review round 1: an earlier version tried `release_build == false`
    /// only, which would have missed a regression letting the release-build
    /// download default override an explicitly selected local directory —
    /// exactly the case an air-gapped operator depends on).
    ///
    /// The concrete source is identified through `PayloadSource`'s `Debug`
    /// supertrait — an erased `Arc<dyn PayloadSource>` offers nothing else
    /// to match on, and the alternative (a `source_kind()` method that only
    /// tests call) would be production surface existing solely for this
    /// assertion. Nothing here performs a request: the `Release` arms are
    /// only inspected, never driven.
    #[farhelm_testtrace::test]
    async fn production_payloads_selects_by_payload_selection_and_release_build() {
        let state_dir = tempfile::tempdir().unwrap();
        let payloads = production_payloads(
            PayloadSelection::Default,
            state_dir.path(),
            false,
            state_dir.path(),
        )
        .unwrap();
        let error = payloads
            .path(PayloadKind::Farhelm, PayloadArch::X86_64)
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("this farhelm was built from source"),
            "a developer build must refuse rather than download: {error:#}"
        );

        let state_dir = tempfile::tempdir().unwrap();
        let payloads = production_payloads(
            PayloadSelection::Default,
            state_dir.path(),
            true,
            state_dir.path(),
        )
        .unwrap();
        let described = format!("{payloads:?}");
        assert!(
            described.starts_with("ReleasePayloadSource"),
            "a release build must download by default: {described}"
        );
        assert!(
            described.contains(&format!(
                "https://github.com/scode/farhelm/releases/download/v{}/",
                env!("CARGO_PKG_VERSION")
            )),
            "the default source must name THIS build's release: {described}"
        );

        // Both `release_build` values for `Directory`, because the case that
        // matters is the release-shaped one: `--payload-dir` has to keep
        // overriding download-by-default, which is the whole point of the
        // flag for an air-gapped operator.
        for release_build in [false, true] {
            let dir_state = tempfile::tempdir().unwrap();
            let payload_dir = tempfile::tempdir().unwrap();
            let archive = farhelm_archive_for(PayloadArch::X86_64);
            write_release_archive(
                payload_dir.path(),
                archive.package,
                archive.target,
                archive.member,
                b"directory-selection-bytes",
            );
            let payloads = production_payloads(
                PayloadSelection::Directory(payload_dir.path().to_path_buf()),
                dir_state.path(),
                release_build,
                dir_state.path(),
            )
            .unwrap();
            let path = payloads
                .path(PayloadKind::Farhelm, PayloadArch::X86_64)
                .await
                .unwrap();
            assert_eq!(
                std::fs::read(path).unwrap(),
                b"directory-selection-bytes",
                "an explicit Directory selection must win regardless of \
                 release_build={release_build}"
            );
        }

        for release_build in [false, true] {
            let state_dir = tempfile::tempdir().unwrap();
            let payloads = production_payloads(
                PayloadSelection::Release {
                    base_url: url::Url::parse("http://127.0.0.1:1/").unwrap(),
                },
                state_dir.path(),
                release_build,
                state_dir.path(),
            )
            .unwrap();
            let described = format!("{payloads:?}");
            assert!(
                described.starts_with("ReleasePayloadSource"),
                "--release-base-url must select a download source on any build: {described}"
            );
            assert!(
                described.contains("http://127.0.0.1:1/"),
                "the explicit base URL must win over the default: {described}"
            );
        }
    }

    /// Spec: the ONE production call site really does pass
    /// `env!("CARGO_PKG_VERSION")` into `ReleasePayloadSource`, not some
    /// other value a future refactor could substitute by accident.
    ///
    /// `release_payloads::VERSION`'s docstring names the incident this
    /// guards against: every method on that type used to read a
    /// module-level constant directly, so bumping the workspace version
    /// broke every fixture-backed test in the module at once — the
    /// committed fixtures are permanently signed for
    /// `test_support::FIXTURE_VERSION`, not whatever the crate version
    /// happens to be. The fix makes the version a constructor argument
    /// threaded all the way from `production_payloads`; this test is the
    /// oracle that PRODUCTION still threads through the real crate version.
    /// The version is part of the cache directory's name, so its presence
    /// in the `Debug` rendering is enough to check without a real download.
    #[farhelm_testtrace::test]
    fn production_wiring_binds_the_cache_to_the_crate_version() {
        let state_dir = tempfile::tempdir().unwrap();
        let payloads = production_payloads(
            PayloadSelection::Release {
                base_url: url::Url::parse("http://127.0.0.1:1/").unwrap(),
            },
            state_dir.path(),
            true,
            state_dir.path(),
        )
        .unwrap();
        let described = format!("{payloads:?}");
        assert!(
            described.contains(&format!("v{}-", env!("CARGO_PKG_VERSION"))),
            "the cache directory must be keyed by exactly CARGO_PKG_VERSION: {described}"
        );
    }

    /// Spec: pointing the helm at a release URL provisions a host with the
    /// VERIFIED downloaded bytes — the acceptance condition plan Step 3
    /// names, end to end.
    ///
    /// Everything above and below the download is real: production source
    /// selection resolves the URL, `ProvisioningService` runs a real plan,
    /// and the executor installs whatever the source handed it. The seams
    /// this covers are the ones unit tests structurally cannot — selection
    /// dropping the URL, payload preparation staging the wrong file,
    /// `install-tmux` receiving the farhelm binary — each of which would
    /// leave the downloader and the executor both passing their own tests
    /// while "add host" installed the wrong thing.
    ///
    /// `needs_tmux` is on so BOTH payload kinds travel the download path,
    /// and the fixture bytes differ per kind, so a crossed payload fails
    /// loudly rather than installing a plausible file. Everything is
    /// injected: a loopback fixture server, a temporary state directory, and
    /// the recording backend — no environment variable is read or written.
    #[farhelm_testtrace::test]
    async fn a_release_url_provisions_a_host_with_verified_downloaded_payloads() {
        let release = FixtureRelease::start(Vec::new()).await;
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        backend
            .stateful
            .store(true, std::sync::atomic::Ordering::SeqCst);
        *backend.reach.lock().unwrap() = ReachOutcome::Supported(Reach {
            user_unit_dir: root.path().join(".config/systemd/user"),
            home: root.path().to_path_buf(),
            arch: PayloadArch::X86_64,
            distro_id: "ubuntu".to_string(),
            needs_tmux: true,
            host_tmux: None,
        });

        // The fixture release is signed with the throwaway test key for
        // FIXTURE_VERSION, so this names both that trust anchor and that
        // version explicitly; everything else — the arms, the cache root,
        // the HTTP client — is the production policy.
        let payloads = production_payloads_with_key(
            PayloadSelection::Release {
                base_url: release.base_url.clone(),
            },
            state_dir.path(),
            false,
            state_dir.path(),
            super::release_payloads::test_support::FIXTURE_VERSION,
            super::release_payloads::test_support::test_pubkey(),
            // Production settings plus `no_proxy()`: without it an ambient
            // proxy variable would route this loopback fixture request off
            // the machine, which no test here may do.
            super::release_payloads::test_support::test_client(),
        )
        .unwrap();
        let service = service_with_payloads(&harness, backend.clone(), root.path(), payloads);

        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "downloads.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("expected a plan")
        };
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        let view = wait_finished(&service, accepted.host_id).await;
        assert_eq!(view.status, RunStatus::Completed, "{:?}", view.message);

        let operations = backend.operations.lock().unwrap().clone();
        assert!(
            operations.contains(&"install-farhelm".to_string())
                && operations.contains(&"install-tmux".to_string()),
            "both payloads must have been installed: {operations:?}"
        );
        assert_eq!(
            std::fs::read(root.path().join("lib/farhelm")).unwrap(),
            expected_member("farhelm", "x86_64-unknown-linux-musl"),
            "the host must receive the verified farhelm from the release"
        );
        assert_eq!(
            std::fs::read(root.path().join("lib/tmux")).unwrap(),
            expected_member("tmux", "x86_64-unknown-linux-musl"),
            "the host must receive the verified tmux from the release"
        );
    }

    /// Spec: a leftover `<state_dir>/embedded-payloads/` cache from a
    /// pre-D2 install is removed the first time `production_payloads` runs,
    /// regardless of which source is selected — this proves it for
    /// `Directory`, the selection least likely to accidentally exercise the
    /// same code path `Default` would.
    ///
    /// F11 (review round 1): also seeds sibling state — `helm.db` and a
    /// live `payloads/current` cache — and requires both untouched. The
    /// destructive boundary is the property actually worth protecting: a
    /// future cleanup that widened its blast radius to the whole state
    /// directory, or damaged an unrelated sibling, would still pass a test
    /// that checked only `embedded-payloads` itself was gone.
    #[farhelm_testtrace::test]
    fn production_payloads_removes_a_leftover_embedded_payloads_directory() {
        let state_dir = tempfile::tempdir().unwrap();
        let leftover = state_dir.path().join("embedded-payloads");
        std::fs::create_dir_all(leftover.join("stale-generation")).unwrap();
        std::fs::write(leftover.join("stale-generation").join("farhelm"), b"old").unwrap();

        std::fs::write(state_dir.path().join("helm.db"), b"durable registry").unwrap();
        std::fs::create_dir_all(state_dir.path().join("payloads")).unwrap();
        std::fs::write(
            state_dir.path().join("payloads").join("current"),
            b"live cache",
        )
        .unwrap();

        production_payloads(
            PayloadSelection::Directory(state_dir.path().join("staged")),
            state_dir.path(),
            false,
            state_dir.path(),
        )
        .unwrap();

        assert!(
            !leftover.exists(),
            "a leftover pre-D2 embedded payload cache must be removed"
        );
        assert_eq!(
            std::fs::read(state_dir.path().join("helm.db")).unwrap(),
            b"durable registry",
            "cleanup must never touch the durable registry"
        );
        assert_eq!(
            std::fs::read(state_dir.path().join("payloads").join("current")).unwrap(),
            b"live cache",
            "cleanup must never touch an unrelated live payload cache"
        );
    }

    /// Assert that `events` contains a legacy-cache-alias warning naming
    /// EXACTLY `expected_payload_dir`/`expected_legacy_cache` in its
    /// structured fields (F12, review round 2). Checking the FIELDS,
    /// rather than only counting messages matching the static text, is
    /// what catches a warning whose fields were dropped, swapped, or left
    /// over from a previous case — those are exactly the paths an operator
    /// reads to learn which selected directory was protected and which
    /// retired cache still needs to be moved before cleanup can proceed.
    fn assert_legacy_cache_alias_warning(
        events: &farhelm_testtrace::CaptureHandle,
        expected_payload_dir: &Path,
        expected_legacy_cache: &Path,
    ) {
        let expected_dir = expected_payload_dir.display().to_string();
        let expected_legacy = expected_legacy_cache.display().to_string();
        let matches = crate::test_capture::matching(
            events,
            "--payload-dir points at (or inside) the retired embedded-payloads cache",
        );
        assert!(
            matches.iter().any(|event| {
                event.field("payload_dir") == Some(expected_dir.as_str())
                    && event.field("legacy_cache") == Some(expected_legacy.as_str())
            }),
            "expected a warning naming payload_dir={expected_dir} legacy_cache={expected_legacy}, \
             got {matches:?}"
        );
    }

    /// Spec (F1, review round 1, BLOCKING; round 2 adds the per-case field
    /// check): an operator's explicitly selected `--payload-dir` must
    /// never be deleted as if it were the retired `embedded-payloads`
    /// cache — even when it happens to alias that location exactly, sit
    /// beneath it, or reach it through a symlink. Three setups share the
    /// assertion: the legacy directory (and the file staged inside it)
    /// survive `production_payloads`, and a `warn!` names EXACTLY that
    /// case's two paths, checked immediately after the case runs rather
    /// than only totalled at the end (F12, review round 2) — a total count
    /// would not notice fields that were absent, swapped, or stale from an
    /// earlier case.
    #[farhelm_testtrace::test]
    fn production_payloads_preserves_a_payload_dir_that_aliases_the_legacy_cache() {
        let events = crate::test_capture::current();

        // Case 1: --payload-dir names the legacy cache directly.
        let state_dir = tempfile::tempdir().unwrap();
        let legacy = state_dir.path().join("embedded-payloads");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(
            legacy.join("farhelm-x86_64-unknown-linux-musl.tar.gz"),
            b"staged",
        )
        .unwrap();
        production_payloads(
            PayloadSelection::Directory(legacy.clone()),
            state_dir.path(),
            false,
            state_dir.path(),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(legacy.join("farhelm-x86_64-unknown-linux-musl.tar.gz")).unwrap(),
            b"staged",
            "selecting the legacy directory itself must not delete it"
        );
        assert_legacy_cache_alias_warning(&events, &legacy, &legacy);

        // Case 2: --payload-dir names a directory nested beneath it.
        let state_dir = tempfile::tempdir().unwrap();
        let legacy = state_dir.path().join("embedded-payloads");
        let nested = legacy.join("staged-by-hand");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            nested.join("farhelm-x86_64-unknown-linux-musl.tar.gz"),
            b"staged",
        )
        .unwrap();
        production_payloads(
            PayloadSelection::Directory(nested.clone()),
            state_dir.path(),
            false,
            state_dir.path(),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(nested.join("farhelm-x86_64-unknown-linux-musl.tar.gz")).unwrap(),
            b"staged",
            "selecting a directory beneath the legacy cache must not delete it"
        );
        assert_legacy_cache_alias_warning(&events, &nested, &legacy);

        // Case 3: --payload-dir is a symlink resolving into it.
        use std::os::unix::fs::symlink;
        let state_dir = tempfile::tempdir().unwrap();
        let legacy = state_dir.path().join("embedded-payloads");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(
            legacy.join("farhelm-x86_64-unknown-linux-musl.tar.gz"),
            b"staged",
        )
        .unwrap();
        let alias = state_dir.path().join("payload-dir-alias");
        symlink(&legacy, &alias).unwrap();
        production_payloads(
            PayloadSelection::Directory(alias.clone()),
            state_dir.path(),
            false,
            state_dir.path(),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(legacy.join("farhelm-x86_64-unknown-linux-musl.tar.gz")).unwrap(),
            b"staged",
            "a symlink alias of the legacy cache must not be deleted through either path"
        );
        assert_legacy_cache_alias_warning(&events, &alias, &legacy);
    }

    /// Spec (F1, review round 2, BLOCKING): a symlink INSIDE the legacy
    /// cache that points OUTWARD to a different, populated directory must
    /// still be preserved when the operator selects that symlink's
    /// PATHNAME (not its resolved target) as `--payload-dir`. The
    /// canonical-only check round 1 shipped would resolve the symlink away
    /// from the legacy tree and conclude the two paths were unrelated,
    /// missing exactly this case; the lexical check added in round 2
    /// catches it because the SELECTED PATHNAME itself sits beneath
    /// `embedded-payloads/`.
    #[farhelm_testtrace::test]
    fn production_payloads_preserves_an_outward_symlink_selected_inside_the_legacy_cache() {
        use std::os::unix::fs::symlink;

        let events = crate::test_capture::current();
        let state_dir = tempfile::tempdir().unwrap();
        let legacy = state_dir.path().join("embedded-payloads");
        std::fs::create_dir_all(&legacy).unwrap();
        let external = state_dir.path().join("external-staging");
        std::fs::create_dir_all(&external).unwrap();
        std::fs::write(
            external.join("farhelm-x86_64-unknown-linux-musl.tar.gz"),
            b"staged",
        )
        .unwrap();
        let inward_name = legacy.join("alias");
        symlink(&external, &inward_name).unwrap();

        production_payloads(
            PayloadSelection::Directory(inward_name.clone()),
            state_dir.path(),
            false,
            state_dir.path(),
        )
        .unwrap();

        assert_eq!(
            std::fs::read(inward_name.join("farhelm-x86_64-unknown-linux-musl.tar.gz")).unwrap(),
            b"staged",
            "an outward-pointing symlink selected by its pathname inside the legacy cache must \
             not be broken by deleting the cache around it"
        );
        assert_legacy_cache_alias_warning(&events, &inward_name, &legacy);
    }

    /// Spec (F1, review round 2, BLOCKING): a dangling symlink AT the
    /// legacy `embedded-payloads` pathname — not a real directory at all —
    /// must never be passed to `remove_dir_all`, which would delete
    /// whatever the link points at (nothing, here) rather than the link
    /// itself. `production_payloads` still succeeds for an UNRELATED
    /// selection; only the legacy cleanup step is skipped.
    #[farhelm_testtrace::test]
    fn production_payloads_leaves_a_dangling_legacy_symlink_alone() {
        use std::os::unix::fs::symlink;

        let state_dir = tempfile::tempdir().unwrap();
        let legacy = state_dir.path().join("embedded-payloads");
        symlink(state_dir.path().join("nonexistent-target"), &legacy).unwrap();

        production_payloads(
            PayloadSelection::Directory(state_dir.path().join("unrelated-payloads")),
            state_dir.path(),
            false,
            state_dir.path(),
        )
        .unwrap();

        let metadata = std::fs::symlink_metadata(&legacy)
            .expect("the dangling symlink itself must still be present");
        assert!(
            metadata.file_type().is_symlink(),
            "the legacy path must remain exactly the symlink it was, not be replaced or removed"
        );
    }

    /// Build a RELATIVE path from `base` to the absolute `target`, by text
    /// only: enough `..` segments to walk from `base` up to the shared
    /// root, then `target`'s own components back down.
    ///
    /// Exists so the relative-`--payload-dir` regression below can
    /// construct a genuinely relative selection without ever calling
    /// `std::env::set_current_dir` — mutating this test process's actual
    /// working directory would race every other test in the same binary,
    /// and CLAUDE.md rules that out for the equivalent case of environment
    /// variables for the same reason. Anchoring the returned path at
    /// `base` — which the caller always passes as the REAL, unmodified
    /// `std::env::current_dir()` — means ordinary filesystem calls made by
    /// this same process resolve it correctly with no process-wide state
    /// changed at all.
    fn relative_from(base: &Path, target: &Path) -> PathBuf {
        let ups = base
            .components()
            .filter(|component| !matches!(component, Component::RootDir | Component::Prefix(_)))
            .count();
        let mut relative = PathBuf::new();
        for _ in 0..ups {
            relative.push("..");
        }
        for component in target.components() {
            if !matches!(component, Component::RootDir | Component::Prefix(_)) {
                relative.push(component.as_os_str());
            }
        }
        relative
    }

    /// Spec (F1, review round 3, BLOCKING): a `--payload-dir` selection
    /// spelled as a RELATIVE path evades the round-2 alias guard exactly
    /// like an outward symlink did — clap accepts a relative spelling
    /// unchanged, but the guard's lexical check compared it directly
    /// against the always-absolute legacy path (never equal, never a
    /// prefix, no matter how the two relate), and canonicalizing the
    /// selection followed the inward symlink straight out of the legacy
    /// tree before the two sides were ever compared. Both gaps close only
    /// once the selection is made absolute against `cwd` first.
    #[farhelm_testtrace::test]
    fn production_payloads_preserves_an_outward_symlink_selected_via_a_relative_path() {
        use std::os::unix::fs::symlink;

        let events = crate::test_capture::current();
        let state_dir = tempfile::tempdir().unwrap();
        let legacy = state_dir.path().join("embedded-payloads");
        std::fs::create_dir_all(&legacy).unwrap();
        let external = state_dir.path().join("external-staging");
        std::fs::create_dir_all(&external).unwrap();
        std::fs::write(
            external.join("farhelm-x86_64-unknown-linux-musl.tar.gz"),
            b"staged",
        )
        .unwrap();
        let inward_name = legacy.join("alias");
        symlink(&external, &inward_name).unwrap();

        // A relative spelling of `inward_name`, anchored at the test
        // process's REAL cwd — see `relative_from`'s docs for why this
        // never touches `std::env::set_current_dir`.
        let cwd = std::env::current_dir().unwrap();
        let selected = relative_from(&cwd, &inward_name);

        production_payloads(
            PayloadSelection::Directory(selected.clone()),
            state_dir.path(),
            false,
            &cwd,
        )
        .unwrap();

        assert_eq!(
            std::fs::read(inward_name.join("farhelm-x86_64-unknown-linux-musl.tar.gz")).unwrap(),
            b"staged",
            "a RELATIVE selection of an outward-pointing symlink inside the legacy cache must not \
             be broken by deleting the cache around it"
        );
        assert_legacy_cache_alias_warning(&events, &selected, &legacy);
    }

    /// Spec (F1, review round 3, BLOCKING): an absolute `--payload-dir`
    /// beginning with `/..` evades the guard for a different textual
    /// reason than the relative case above — `normalize_lexical` used to
    /// preserve a leading `..` past the root instead of clamping it there
    /// the way the kernel's own path resolution does, so `/..<legacy
    /// path>` compared unequal (and not-a-prefix) to `<legacy path>`'s own
    /// lexical form, even though both name the identical filesystem
    /// location.
    #[farhelm_testtrace::test]
    fn production_payloads_preserves_an_outward_symlink_selected_via_an_absolute_dot_dot_spelling()
    {
        use std::os::unix::fs::symlink;

        let events = crate::test_capture::current();
        let state_dir = tempfile::tempdir().unwrap();
        let legacy = state_dir.path().join("embedded-payloads");
        std::fs::create_dir_all(&legacy).unwrap();
        let external = state_dir.path().join("external-staging");
        std::fs::create_dir_all(&external).unwrap();
        std::fs::write(
            external.join("farhelm-x86_64-unknown-linux-musl.tar.gz"),
            b"staged",
        )
        .unwrap();
        let inward_name = legacy.join("alias");
        symlink(&external, &inward_name).unwrap();

        // Clamps right back to `inward_name`, exactly like the kernel's own
        // path resolution treats `/..` above the root as a no-op rather
        // than an escape to a parent that does not exist.
        let selected = PathBuf::from(format!("/..{}", inward_name.display()));
        let cwd = std::env::current_dir().unwrap();

        production_payloads(
            PayloadSelection::Directory(selected.clone()),
            state_dir.path(),
            false,
            &cwd,
        )
        .unwrap();

        assert_eq!(
            std::fs::read(inward_name.join("farhelm-x86_64-unknown-linux-musl.tar.gz")).unwrap(),
            b"staged",
            "an absolute /.. spelling of an outward-pointing symlink inside the legacy cache must \
             not be broken by deleting the cache around it"
        );
        assert_legacy_cache_alias_warning(&events, &selected, &legacy);
    }

    /// Spec (F2, review round 3, BLOCKING): the provisioning service runs
    /// up to four host installs at once, all sharing one
    /// `DirectoryPayloads`, so the very first two "add host" runs against a
    /// freshly staged directory can both observe `.extracted` absent
    /// before either creates it. A prior version turned the loser's
    /// harmless `AlreadyExists` from `DirBuilder::create` into a hard
    /// provisioning error; this proves both concurrent first calls now
    /// succeed and leave behind an ordinary mode-0700 directory rather than
    /// a spurious failure or a permissions mismatch.
    #[test]
    fn directory_payloads_concurrent_first_use_both_succeed() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().unwrap();
        let archive = farhelm_archive_for(PayloadArch::X86_64);
        write_release_archive(
            dir.path(),
            archive.package,
            archive.target,
            archive.member,
            b"race-bytes",
        );
        let payloads = Arc::new(DirectoryPayloads::new(dir.path().to_path_buf()));
        let runtime = tokio::runtime::Runtime::new().unwrap();

        // Both tasks await the SAME barrier immediately before their first
        // call, so neither task's `.extracted`-absent observation can be
        // sequenced strictly after the other task has already finished
        // creating the directory — the exact interleaving this fix must
        // survive.
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let tasks: Vec<_> = (0..2)
            .map(|_| {
                let payloads = Arc::clone(&payloads);
                let barrier = Arc::clone(&barrier);
                runtime.spawn(async move {
                    barrier.wait().await;
                    payloads
                        .path(PayloadKind::Farhelm, PayloadArch::X86_64)
                        .await
                })
            })
            .collect();

        for task in tasks {
            runtime.block_on(task).unwrap().expect(
                "both concurrent first calls must succeed; a loser observing AlreadyExists must \
                 not turn a harmless race into a provisioning failure",
            );
        }

        let metadata = std::fs::symlink_metadata(dir.path().join(".extracted")).unwrap();
        assert!(
            metadata.file_type().is_dir(),
            "the extraction cache must end up a plain directory, not a symlink or other object"
        );
        assert_eq!(
            metadata.permissions().mode() & 0o777,
            0o700,
            "the extraction cache must end up mode 0700 regardless of which call actually \
             created it"
        );
    }
}
