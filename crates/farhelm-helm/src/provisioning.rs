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

mod backend;
mod e2e;
mod http;
mod payloads;
mod plan;
mod service;

pub use backend::{LocalSupervisorDiscovery, discover_local_supervisor};
pub(crate) use http::{probe_host, provision_host, provisioning_state, update_host};
pub(crate) use service::ProvisioningService;

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    use super::backend::*;
    #[allow(unused_imports)]
    use super::e2e::*;
    use super::http::*;
    use super::payloads::*;
    use super::plan::*;
    #[allow(unused_imports)]
    use super::service::*;
    use crate::AppState;
    use crate::manager::{ConnectionManager, HostState};
    use crate::rest_harness::{FleetBuilder, Harness, HostScript};
    use crate::store::{DialedAs, HelmStore, HostId};
    use anyhow::{Context as _, bail};
    use async_trait::async_trait;
    use axum::body::{Body, to_bytes};
    use axum::extract::{Path as AxPath, State};
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use farhelm_proto::ControlMsg;
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};
    use std::process::Stdio;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    use tower::ServiceExt;

    const PAYLOAD_CHILD_ENV: &str = "FARHELM_EMBEDDED_PAYLOAD_TEST_CHILD";

    /// Build the real helm crate in a child Cargo process so `build.rs`, the
    /// generated manifest, and `production_payloads()` are tested as one
    /// boundary rather than as three independently plausible pieces.
    #[test]
    fn embedded_payload_build_maps_every_sentinel_to_its_runtime_selection() {
        let fixture = tempfile::tempdir().unwrap();
        let payload_root = fixture.path().join("payloads");
        std::fs::create_dir(&payload_root).unwrap();
        for (filename, bytes) in [
            (
                "farhelm-x86_64-unknown-linux-musl",
                b"farhelm-x86".as_slice(),
            ),
            ("tmux-x86_64-unknown-linux-musl", b"tmux-x86".as_slice()),
            (
                "farhelm-aarch64-unknown-linux-musl",
                b"farhelm-arm".as_slice(),
            ),
            ("tmux-aarch64-unknown-linux-musl", b"tmux-arm".as_slice()),
        ] {
            std::fs::write(payload_root.join(filename), bytes).unwrap();
        }
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        let status = std::process::Command::new(env!("CARGO"))
            .current_dir(workspace)
            .args([
                "test",
                "--quiet",
                "-p",
                "farhelm-helm",
                "provisioning::tests::embedded_payload_child_reads_exact_manifest_bytes",
                "--",
                "--exact",
                "--nocapture",
            ])
            .env("CARGO_TARGET_DIR", fixture.path().join("target"))
            .env("FARHELM_PAYLOAD_DIR", &payload_root)
            .env(PAYLOAD_CHILD_ENV, &payload_root)
            .status()
            .unwrap();
        assert!(status.success(), "child payload build failed with {status}");
    }

    /// Child half of the build-script integration test. A normal test run
    /// has no marker and returns immediately; the parent rebuild supplies
    /// four distinct bytes and this test reads them only through production's
    /// materialized payload source.
    #[test]
    fn embedded_payload_child_reads_exact_manifest_bytes() {
        let Some(root) = std::env::var_os(PAYLOAD_CHILD_ENV).map(PathBuf::from) else {
            return;
        };
        let materialized_root = tempfile::tempdir().unwrap();
        let payloads = production_payloads(materialized_root.path()).unwrap();
        let cache_root = materialized_root.path().join("embedded-payloads");
        assert!(
            !cache_root.exists(),
            "constructing the release source must not eagerly write every payload"
        );
        std::fs::create_dir(&cache_root).unwrap();
        let stale = cache_root.join("payload-from-older-generation");
        std::fs::write(&stale, b"stale").unwrap();
        for (kind, arch, filename) in [
            (
                PayloadKind::Farhelm,
                PayloadArch::X86_64,
                "farhelm-x86_64-unknown-linux-musl",
            ),
            (
                PayloadKind::Tmux,
                PayloadArch::X86_64,
                "tmux-x86_64-unknown-linux-musl",
            ),
            (
                PayloadKind::Farhelm,
                PayloadArch::Aarch64,
                "farhelm-aarch64-unknown-linux-musl",
            ),
            (
                PayloadKind::Tmux,
                PayloadArch::Aarch64,
                "tmux-aarch64-unknown-linux-musl",
            ),
        ] {
            let materialized = payloads.path(kind, arch).unwrap();
            assert!(
                !stale.exists(),
                "first payload access must clean files absent from the current manifest"
            );
            assert!(
                materialized.starts_with(materialized_root.path().join("embedded-payloads")),
                "embedded payload escaped the app-owned state directory: {}",
                materialized.display()
            );
            assert_eq!(
                std::fs::read(materialized).unwrap(),
                std::fs::read(root.join(filename)).unwrap(),
                "{kind:?}/{arch:?} selected the wrong embedded payload"
            );
        }
    }

    /// A payload source whose bytes are irrelevant to the fake executor.
    struct FixedPayloads(PathBuf);

    impl PayloadSource for FixedPayloads {
        fn path(&self, _payload: PayloadKind, _arch: PayloadArch) -> anyhow::Result<PathBuf> {
            Ok(self.0.clone())
        }
    }

    /// A release fixture with Farhelm present but the required tmux artifact
    /// missing, used to prove whole-plan payload preflight.
    struct MissingTmuxPayload {
        farhelm: PathBuf,
        requested: Mutex<Vec<PayloadKind>>,
    }

    impl PayloadSource for MissingTmuxPayload {
        fn path(&self, payload: PayloadKind, _arch: PayloadArch) -> anyhow::Result<PathBuf> {
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
                    needs_tmux: false,
                    tmux_dir: Some(PathBuf::from("/usr/bin")),
                })),
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
        ProvisioningService::injected(
            harness.store.clone(),
            Arc::clone(&harness.manager),
            backend,
            Arc::new(FixedPayloads(payload)),
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
    #[tokio::test]
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
    #[tokio::test]
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
    #[tokio::test]
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
    #[tokio::test]
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
                    target: ProbeDestination::Local,
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
                message.contains("this build carries no provisioning payloads")
            }));
            assert_eq!(failed.steps[0].status, StepStatus::Pending);
            assert_eq!(failed.steps[1].status, StepStatus::Failed);
            assert!(backend.operations.lock().unwrap().is_empty());
            assert!(service.memory.lock().await.busy.is_empty());
        }
    }

    /// Preflight visits every required payload before mutating, even when an
    /// earlier payload exists and only the private-tmux artifact is missing.
    #[tokio::test]
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
            reach.tmux_dir = None;
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
                target: ProbeDestination::Local,
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
    #[tokio::test]
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
    #[tokio::test]
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
    #[tokio::test]
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
    #[tokio::test]
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

    /// Manual and backend planning failures retain no UPDATE claim; once
    /// inspection recovers, the same host can immediately produce a plan.
    #[tokio::test]
    async fn update_planning_failures_release_the_host_for_retry() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let host = harness.store.list_hosts().await.unwrap()[0].id;
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
            needs_tmux: false,
            tmux_dir: Some(PathBuf::from("/usr/bin")),
        });
        *backend.inspect_failure.lock().unwrap() = Some("inspection broke".to_string());
        assert!(service.plan_update(host).await.is_err());
        assert!(service.memory.lock().await.busy.is_empty());

        let recovered = service.plan_update(host).await.unwrap();
        assert!(!recovered.probe_id.is_empty());
    }

    /// UPDATE is not an identity-resolution mechanism: both frozen mismatch
    /// and duplicate rows are refused before host inspection begins.
    #[tokio::test]
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
    #[tokio::test]
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
    #[tokio::test]
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
    #[tokio::test]
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
    #[tokio::test]
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
    #[tokio::test]
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
                target: ProbeDestination::Local,
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
                target: ProbeDestination::Local,
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
    #[tokio::test]
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
                    target: ProbeDestination::Local,
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
    #[tokio::test]
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
                    .body(Body::from(r#"{"target":{"kind":"local"}}"#))
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
    #[tokio::test]
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
    #[tokio::test]
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
        let host = harness.store.list_hosts().await.unwrap()[0].id;

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
    #[tokio::test]
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
    #[tokio::test]
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

    #[test]
    fn tmux_floor_accepts_suffixes_but_not_older_or_malformed_versions() {
        assert!(tmux_at_least_3_3("tmux 3.3a"));
        assert!(tmux_at_least_3_3("tmux 4.0"));
        assert!(!tmux_at_least_3_3("tmux 3.2"));
        assert!(!tmux_at_least_3_3("not installed"));
    }

    /// The production classifier requires both the private marker and exit
    /// 75; authentication failure, unmarked 75, malformed hello, and skew
    /// remain errors even when process creation itself succeeds.
    #[tokio::test]
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

    /// Child capture fails closed on either unbounded peer output or a
    /// deadline, returning only after the offending process has been killed
    /// and reaped.
    #[tokio::test]
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
    #[tokio::test]
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
    #[tokio::test]
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

    /// Reach parsing refuses malformed fields and unsupported platforms,
    /// selects both payload architectures, and requests tmux when absent or
    /// below the supported floor.
    #[test]
    fn reach_output_parser_covers_platform_and_tool_boundaries() {
        let supported = |os: &str, arch: &str, tmux_path: &str, tmux: &str, manager: &str| {
            format!(
                "{REACH_RECORD_MARKER}\0{os}\0/home/test\0{arch}\0{tmux_path}\0{tmux}\0{manager}\0/home/test/.config/systemd/user\0"
            )
        };
        assert!(matches!(
            parse_reach_output(
                supported("debian", "x86_64", "/usr/bin/tmux", "tmux 3.3", "usable").as_bytes()
            )
            .unwrap(),
            ReachOutcome::Manual(_)
        ));
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
        let ReachOutcome::Supported(old) = parse_reach_output(
            supported("ubuntu", "aarch64", "/usr/bin/tmux", "tmux 3.2", "usable").as_bytes(),
        )
        .unwrap() else {
            panic!("aarch64 Ubuntu with a user manager is supported")
        };
        assert_eq!(old.arch, PayloadArch::Aarch64);
        assert!(old.needs_tmux);
        let ReachOutcome::Supported(relative_tmux) = parse_reach_output(
            supported("ubuntu", "x86_64", "bin/tmux", "tmux 3.4", "usable").as_bytes(),
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
    #[test]
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
                    needs_tmux: true,
                    tmux_dir: None,
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
    }

    /// Production layout has no test overrides: local and SSH plans use the
    /// documented library, state, unit, and nonce-scoped temporary paths.
    #[test]
    fn production_layout_uses_the_exact_deployment_paths() {
        let home = PathBuf::from("/home/provisioned");
        let user_units = PathBuf::from("/xdg/systemd/user");
        let local_state = PathBuf::from("/helm/state");
        let reach = Reach {
            home: home.clone(),
            user_unit_dir: user_units.clone(),
            arch: PayloadArch::X86_64,
            needs_tmux: false,
            tmux_dir: Some(PathBuf::from("/usr/bin")),
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
    #[tokio::test]
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
            reach.tmux_dir = None;
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
                target: ProbeDestination::Local,
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
    #[tokio::test]
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

    /// Systemd specifiers are escaped literally and unrepresentable paths
    /// fail before a plan can be offered.
    #[test]
    fn systemd_rendering_is_fallible_and_escapes_percent() {
        let unit = supervisor_unit(
            Path::new("/tmp/%h/farhelm"),
            Path::new("/tmp/state"),
            Path::new("/tmp/%h"),
            None,
        )
        .unwrap();
        assert!(unit.contains("/tmp/%%h/farhelm"));
        assert!(unit.contains("PATH=/tmp/%%h:"));
        assert!(unit.contains("KillMode=process"));
        assert!(!unit.contains("After=default.target"));
        assert!(!unit.contains("@FARHELM@"));
        assert!(!unit.contains("@STATE_DIR@"));
        assert!(!unit.contains("@PATH@"));
        assert!(
            supervisor_unit(
                Path::new("/tmp/farhelm"),
                Path::new("/tmp/state\nother"),
                Path::new("/tmp"),
                None,
            )
            .is_err()
        );
        assert!(
            supervisor_unit(
                Path::new("/tmp/farhelm"),
                Path::new("/tmp/state"),
                Path::new("/tmp/with:colon"),
                None,
            )
            .is_err()
        );
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let non_utf8 = PathBuf::from(std::ffi::OsString::from_vec(vec![
                b'/', b't', b'm', b'p', b'/', 0xff,
            ]));
            assert!(
                supervisor_unit(
                    Path::new("/tmp/farhelm"),
                    &non_utf8,
                    Path::new("/tmp"),
                    None,
                )
                .is_err()
            );
        }
    }

    /// Systemd argument escaping handles each special character in its own
    /// grammar rather than borrowing shell quoting rules.
    #[test]
    fn systemd_argument_rendering_covers_every_supported_escape() {
        for (path, expected) in [
            ("/tmp/a b", "\"/tmp/a b\""),
            ("/tmp/a\"b", "\"/tmp/a\\\"b\""),
            ("/tmp/a\\b", "\"/tmp/a\\\\b\""),
            ("/tmp/%h", "\"/tmp/%%h\""),
        ] {
            assert_eq!(systemd_arg(Path::new(path)).unwrap(), expected);
        }
    }

    /// Remote shell words round-trip hostile but representable paths and
    /// reject bytes that cannot cross the text command boundary unchanged.
    #[test]
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
    #[test]
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
    #[test]
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
    #[tokio::test]
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
    #[tokio::test]
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
    #[tokio::test]
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
                target: ProbeDestination::Local,
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
                target: ProbeDestination::Local,
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
    struct MutablePayload {
        farhelm: Mutex<PathBuf>,
        tmux: PathBuf,
    }

    impl PayloadSource for MutablePayload {
        fn path(&self, payload: PayloadKind, _arch: PayloadArch) -> anyhow::Result<PathBuf> {
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

    async fn self_ssh_available() -> bool {
        tokio::process::Command::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "StrictHostKeyChecking=yes",
                "-o",
                "ConnectTimeout=10",
                "localhost",
                "true",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .is_ok_and(|status| status.success())
    }

    /// Resolve the executable payload contract for real provisioning tests.
    ///
    /// `FARHELM_TEST_BINARY` lets clean or nonstandard target layouts name a
    /// known artifact. The conventional workspace debug path remains a
    /// convenience for the CI sequence, which builds binaries before tests.
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
    fn debug_tmux() -> Option<PathBuf> {
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

    async fn wait_real_run(
        service: &ProvisioningService,
        host: HostId,
    ) -> anyhow::Result<ProvisioningView> {
        tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                let view = service.view(host).await?;
                if matches!(view.status, RunStatus::Completed | RunStatus::Failed) {
                    return Ok(view);
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .context("real provisioning run did not finish")?
    }

    /// Wait through the manager's intentional disconnect/retry window until
    /// a test can issue a command on the current supervisor incarnation.
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

    /// Exercise the complete installer against a real user manager, either
    /// through direct local process/file operations or through ssh+sftp to
    /// localhost. All install/state paths are fixture-owned and linger is a
    /// simulated action. The unit file stays below the fixture root and the
    /// user manager sees it only through a nonce-scoped runtime link removed
    /// by [`UnitGuard`] on every exit.
    async fn real_provisioning_case(use_ssh: bool, update: bool) {
        let test_name = if use_ssh {
            "provisioning_over_ssh_to_localhost"
        } else {
            "provisioning_over_the_direct_local_transport"
        };
        if !user_manager_available().await {
            eprintln!("SKIPPED {test_name}: no usable systemd user manager exists on this host");
            return;
        }
        if use_ssh && !self_ssh_available().await {
            eprintln!(
                "SKIPPED {test_name}: passwordless, already-trusted ssh localhost is unavailable"
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
            eprintln!("SKIPPED {test_name}: no absolute tmux executable is available");
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
        let unit = format!("farhelm-provisioning-test-{}.service", uuid::Uuid::new_v4());
        let unit_path = unit_dir.join(&unit);
        let mut guard = UnitGuard {
            unit: unit.clone(),
            unit_path: unit_path.clone(),
            state_dir: supervisor_state.clone(),
            cleaned: false,
        };
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
        let mut system_backend =
            SystemBackend::with_simulated_linger(helm_state.clone(), Ok(()), true);
        system_backend.launcher = Arc::clone(&launcher) as Arc<dyn CommandLauncher>;
        let service = ProvisioningService::injected(
            store.clone(),
            Arc::clone(&manager),
            Arc::new(system_backend),
            Arc::clone(&payloads) as Arc<dyn PayloadSource>,
            PlanLayout {
                local_state_dir: supervisor_state.clone(),
                override_lib_dir: Some(lib_dir.clone()),
                override_farhelm_path: None,
                override_state_dir: Some(supervisor_state.clone()),
                override_unit_dir: Some(unit_dir),
                unit_name: unit.clone(),
            },
            payload.clone(),
        );
        let request = ProbeRequest {
            target: if use_ssh {
                ProbeDestination::Ssh {
                    destination: "localhost".to_string(),
                }
            } else {
                ProbeDestination::Local
            },
            remote_farhelm: use_ssh.then(|| {
                probe_farhelm
                    .to_str()
                    .expect("temp path is UTF-8")
                    .to_string()
            }),
            remote_state_dir: use_ssh.then(|| {
                probe_state
                    .to_str()
                    .expect("temp path is UTF-8")
                    .to_string()
            }),
        };

        let response = service.probe(request).await.unwrap();
        let ProbeResponse::Provisionable { probe_id, plan, .. } = response else {
            panic!("an absent isolated supervisor must produce a plan")
        };
        assert_eq!(
            matches!(plan.target, ProvisioningTarget::Ssh { .. }),
            use_ssh,
            "the local case must never silently become SSH-to-self"
        );
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        let completed = match wait_real_run(&service, accepted.host_id).await {
            Ok(completed) => completed,
            Err(error) => {
                service.abort_run(accepted.host_id).await;
                guard.cleanup().expect("cleanup after timed-out ADD");
                panic!("{error:#}");
            }
        };
        assert_eq!(completed.status, RunStatus::Completed, "{completed:?}");
        assert!(lib_dir.join("farhelm").is_file());
        assert!(unit_path.is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o7777;
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

        // ADD reruns discovery and uses the answering supervisor as-is. It
        // does not reinterpret an explicit retry as permission to update.
        let previous_run = completed.run_id;
        let rerun = service
            .probe(ProbeRequest {
                target: if use_ssh {
                    ProbeDestination::Ssh {
                        destination: "localhost".to_string(),
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

        let client = wait_real_client(&manager, accepted.host_id).await;
        let cwd = root.path().to_str().expect("temp path is UTF-8");
        let session = client
            .create_session(cwd, "/bin/sh", None, 80, 24)
            .await
            .expect("create an operable session through the provisioned host");

        if update {
            let newer = root.path().join("farhelm-newer");
            let mut bytes = tokio::fs::read(&payload).await.unwrap();
            bytes.extend_from_slice(b"farhelm-test-newer-payload");
            tokio::fs::write(&newer, &bytes).await.unwrap();
            set_mode(&newer, 0o755).await.unwrap();
            *payloads.farhelm.lock().unwrap() = newer;
            let update_plan = service.plan_update(accepted.host_id).await.unwrap();
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
                    guard.cleanup().expect("cleanup after timed-out UPDATE");
                    panic!("{error:#}");
                }
            };
            assert_eq!(completed.status, RunStatus::Completed, "{completed:?}");
            assert_eq!(
                hex_sha256(&tokio::fs::read(lib_dir.join("farhelm")).await.unwrap()),
                hex_sha256(&bytes),
                "UPDATE must converge to the newer payload"
            );
        }
        let client = wait_real_client(&manager, accepted.host_id).await;
        assert_session_operable(&client, &session.id).await;
        client.delete_session(&session.id).await.unwrap();

        service.abort_run(accepted.host_id).await;
        manager.shutdown();
        drop(service);
        drop(manager);
        drop(store);
        guard.cleanup().expect("checked nonce resource teardown");
        drop(guard);
        assert!(!unit_path.exists(), "the nonce unit file survived teardown");
        let inactive = tokio::process::Command::new("systemctl")
            .args(["--user", "is-active", "--", &unit])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .unwrap();
        assert!(!inactive.success(), "the nonce unit survived teardown");
        let root_path = root.path().to_path_buf();
        drop(root);
        assert!(
            !root_path.exists(),
            "the temporary install/state root survived teardown"
        );
    }

    /// The CI-shaped transport proof: real ssh and sftp into isolated paths,
    /// then an SSH UPDATE that preserves and operates a tmux-held session.
    #[tokio::test]
    async fn provisioning_and_update_over_ssh_to_localhost_preserve_an_operable_session() {
        real_provisioning_case(true, true).await;
    }

    /// The local path performs no SSH, and the explicit UPDATE replaces the
    /// payload, restarts only the supervisor, and preserves its tmux session.
    #[tokio::test]
    async fn local_provisioning_and_update_preserve_a_running_session() {
        real_provisioning_case(false, true).await;
    }

    /// A planted failure is followed by checked teardown of a real active
    /// runtime unit and tmux server, then verified again from the outer scope.
    #[tokio::test]
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
}
