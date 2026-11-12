//! Real SSH coverage of the Mac job's process-survival and removal contracts.
//!
//! This module belongs under provisioning's existing test module. The explicit
//! ignored test requires an operator-owned UUID root; it never discovers or
//! cleans the target's ordinary installation. Every payload must already be a
//! valid Darwin executable. Mutating a Mach-O to manufacture an update would
//! invalidate its signature and test the wrong failure.

use super::*;
use futures_util::FutureExt as _;

/// Read a required fixture input without putting a private SSH destination in
/// assertion output. The recorder must retain these inputs before test startup.
fn required_input(name: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| panic!("the ignored Mac test requires {name}"))
}

/// Keep transport errors available without publishing a private destination in
/// panic text. The recorder supplies this private directory before test startup;
/// an I/O failure retaining the diagnostic must not replace the original failure.
fn mac_fixture_failure(context: &str, error: impl std::fmt::Display) -> ! {
    use std::{io::Write as _, os::unix::fs::OpenOptionsExt as _};
    if let Some(directory) = std::env::var_os("FARHELM_TEST_TRACE_DIR") {
        let result = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(PathBuf::from(directory).join("mac-fixture-error.txt"))
            .and_then(|mut file| writeln!(file, "{context}: {error:#}"));
        if result.is_err() {
            eprintln!("could not retain the private Mac fixture diagnostic");
        }
    }
    panic!("{context}; see the recorder's private mac-fixture-error.txt");
}

/// A single verified Darwin file, replaceable only with another operator input.
#[derive(Debug)]
struct MacPayload(Mutex<PathBuf>);

#[async_trait]
impl PayloadSource for MacPayload {
    async fn path(&self, target: PayloadTarget) -> anyhow::Result<PathBuf> {
        anyhow::ensure!(
            target == PayloadTarget::MacosAarch64,
            "wrong Mac payload target"
        );
        Ok(self.0.lock().unwrap().clone())
    }
}

/// Check the native arm64 Mach-O header before transferring anything. Execution
/// and signature validity are checked on the Mac after the production install.
fn darwin_payload(path: PathBuf) -> PathBuf {
    use std::io::Read as _;
    let mut header = [0; 8];
    std::fs::File::open(&path)
        .and_then(|mut file| file.read_exact(&mut header))
        .expect("read the supplied Darwin payload header");
    assert_eq!(
        header,
        [0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0, 0, 1],
        "the Mac fixture needs a native arm64 Mach-O executable"
    );
    path
}

/// Frozen cleanup authority for one remote run. Its short child name keeps
/// canonical /private/tmp socket paths within Darwin's Unix-socket limit; the
/// containing root and job label both carry UUIDs.
struct OwnedMac {
    backend: Arc<SystemBackend>,
    target: ProvisioningTarget,
    root: PathBuf,
    state: PathBuf,
    lib: PathBuf,
    plist: PathBuf,
    domain: String,
    label: String,
}

impl OwnedMac {
    /// Allocate a new child of the explicitly supplied /tmp/<uuid> directory.
    /// Existing children and symlink roots are refused before setup can run.
    async fn create(backend: Arc<SystemBackend>, destination: String) -> Self {
        let supplied = PathBuf::from(required_input("FARHELM_TEST_MAC_ROOT"));
        assert_eq!(
            supplied.parent(),
            Some(Path::new("/tmp")),
            "the Mac fixture root must be /tmp/<uuid>"
        );
        uuid::Uuid::parse_str(supplied.file_name().unwrap().to_str().unwrap())
            .expect("the Mac fixture root's final component must be a UUID");
        let nonce = uuid::Uuid::new_v4();
        let child = supplied.join(format!("r-{}", &nonce.simple().to_string()[..8]));
        let target = ProvisioningTarget::ssh(destination);
        let script = format!(
            "[ \"$(uname -s)\" = Darwin ] && [ \"$(uname -m)\" = arm64 ] || exit 1\n\
             [ ! -L {parent} ] || exit 1\n\
             mkdir -p -- {parent} && mkdir -- {child} || exit 1\n\
             readlink -f -- {child} || exit 1\n\
             id -u\n",
            parent = shell_path(&supplied).unwrap(),
            child = shell_path(&child).unwrap(),
        );
        let output = backend
            .require_shell(&target, &script, "allocate the owned Mac fixture")
            .await
            .unwrap_or_else(|error| mac_fixture_failure("Mac fixture allocation failed", error));
        let text = std::str::from_utf8(&output.stdout).expect("fixture paths are UTF-8");
        let mut lines = text.lines();
        let root = PathBuf::from(lines.next().expect("canonical fixture root"));
        assert!(
            root.ends_with(child.strip_prefix("/tmp").unwrap()),
            "canonical fixture root changed its UUID components"
        );
        let uid: u32 = lines
            .next()
            .expect("remote uid")
            .parse()
            .expect("numeric remote uid");
        assert!(
            lines.next().is_none(),
            "unexpected fixture allocation output"
        );
        let label = format!("io.farhelm.test.{}", nonce.simple());
        Self {
            state: root.join("s"),
            lib: root.join("lib"),
            plist: root.join("jobs").join(format!("{label}.plist")),
            root,
            backend,
            target,
            domain: format!("user/{uid}"),
            label,
        }
    }

    /// Use production's bounded SSH capture but omit transport diagnostics from
    /// panic output: OpenSSH can include names other than the destination alias.
    async fn shell(&self, script: &str, what: &str) -> String {
        let output = self
            .backend
            .require_shell(&self.target, script, what)
            .await
            .unwrap_or_else(|error| mac_fixture_failure(what, error));
        String::from_utf8(output.stdout).expect("Mac fixture output is UTF-8")
    }

    /// Confirm the loaded job belongs to the frozen plist before reading its
    /// process or signalling it. A nonce label alone is not cleanup authority.
    fn job_script(&self) -> String {
        let mut script =
            super::super::macos::job_inspection_script(&self.domain, &self.label).unwrap();
        script.push_str(&format!(
            "if [ \"$active\" = active ]; then [ \"$fragment\" = {} ] || exit 1; fi\n",
            shell_path(&self.plist).unwrap(),
        ));
        script
    }

    /// Return the actual supervisor PID published by the owned launchd job.
    async fn supervisor_pid(&self) -> u32 {
        let script = format!(
            "{}\n[ \"$active\" = active ] || exit 1\nprintf '%s\\n' \"$job_output\" | awk '$1 == \"pid\" && $2 == \"=\" {{ print $3 }}'\n",
            self.job_script()
        );
        let output = self.shell(&script, "read the owned supervisor PID").await;
        let pid: u32 = output.trim().parse().expect("one positive launchd PID");
        assert!(pid > 1);
        pid
    }

    /// Inspect only the four processes the two interactive shells identified.
    /// PID plus start time and command distinguishes survival from reconstruction;
    /// post-replay shell variables separately prove the programs kept their state.
    async fn terminal_processes(&self) -> Vec<(u32, String)> {
        let mut processes = Vec::new();
        for name in ["agent.pid", "tab.pid"] {
            let output = self
                .shell(
                    &format!("cat -- {}", shell_path(&self.root.join(name)).unwrap()),
                    "read a terminal's owned PID pair",
                )
                .await;
            let pair = output
                .split_whitespace()
                .map(|value| value.parse::<u32>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(pair.len(), 2, "a terminal records its shell and parent");
            for (index, pid) in pair.into_iter().enumerate() {
                assert!(pid > 1);
                let identity = self
                    .shell(
                        &format!("ps -p {pid} -o pid=,lstart=,command="),
                        "read an owned terminal process identity",
                    )
                    .await;
                assert!(
                    !identity.trim().is_empty(),
                    "the recorded terminal process is alive"
                );
                if index == 1 {
                    assert!(
                        identity.contains("internal terminal-host")
                            && identity.contains(self.state.join("hosts").to_str().unwrap()),
                        "the shell parent must be a terminal host in this fixture"
                    );
                }
                processes.push((pid, identity));
            }
        }
        assert_eq!(
            processes
                .iter()
                .map(|(pid, _)| pid)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            4,
            "the agent and tab must have distinct shells and hosts"
        );
        processes
    }

    /// Guard every executable plan path before confirmation, including upload
    /// temporaries. Linux-only actions are rejected outright.
    fn assert_plan(&self, plan: &ProvisioningPlan) {
        assert!(
            plan.target == self.target,
            "plan changed the fixture's SSH transport"
        );
        assert_eq!(plan.host_arch, PayloadTarget::MacosAarch64);
        assert_eq!(plan.farhelm_path, self.lib.join("farhelm"));
        assert_eq!(plan.state_dir, self.state);
        for action in &plan.actions {
            match action {
                ProvisioningAction::EnsureDirectories { directories } => {
                    for directory in directories {
                        assert!(directory.path.starts_with(&self.root));
                    }
                }
                ProvisioningAction::UploadPayload {
                    arch,
                    destination,
                    temporary,
                }
                | ProvisioningAction::InstallPayload {
                    arch,
                    destination,
                    temporary,
                } => {
                    assert_eq!(*arch, PayloadTarget::MacosAarch64);
                    assert_eq!(destination, &self.lib.join("farhelm"));
                    assert!(temporary.starts_with(&self.lib));
                }
                ProvisioningAction::WriteLaunchAgent {
                    destination,
                    temporary,
                    content,
                } => {
                    assert_eq!(destination, &self.plist);
                    assert!(temporary.starts_with(self.plist.parent().unwrap()));
                    assert_eq!(
                        content,
                        &super::super::macos::supervisor_plist(
                            &self.lib.join("farhelm"),
                            &self.state,
                            &self.label,
                        )
                        .unwrap(),
                        "the fixture job must use its own binary and state"
                    );
                }
                ProvisioningAction::StartLaunchAgent {
                    domain,
                    label,
                    plist,
                    ..
                } => {
                    assert_eq!(domain, &self.domain);
                    assert_eq!(label, &self.label);
                    assert_eq!(plist, &self.plist);
                }
                ProvisioningAction::RemoveLaunchAgent { destination } => {
                    assert_eq!(destination, &self.plist)
                }
                ProvisioningAction::StopLaunchAgent {
                    domain,
                    label,
                    farhelm_program,
                    state_dir,
                } => {
                    assert_eq!(domain, &self.domain);
                    assert_eq!(label, &self.label);
                    assert_eq!(farhelm_program, &self.lib.join("farhelm"));
                    assert_eq!(state_dir, &self.state);
                }
                ProvisioningAction::RemoveDirectory { path } => assert_eq!(path, &self.lib),
                ProvisioningAction::AttachSupervisor | ProvisioningAction::ForgetHost => {}
                _ => panic!("Mac fixture plan contains an unexpected action"),
            }
        }
    }

    /// Stop the frozen job and its hosts even after an assertion panic. Keep the
    /// data directory for evidence; failure to confirm termination prevents any
    /// library removal. Never walk or remove the operator's containing root.
    async fn cleanup(&self) -> bool {
        let mut script = self.job_script();
        script.push_str(
            &super::super::macos::stop_script(
                &self.domain,
                &self.label,
                &self.lib.join("farhelm"),
                &self.state,
            )
            .unwrap(),
        );
        script.push_str(&format!(
            "\n[ ! -L {lib} ] || exit 1\nrm -f -- {plist} && rm -rf -- {lib}\n",
            lib = shell_path(&self.lib).unwrap(),
            plist = shell_path(&self.plist).unwrap(),
        ));
        self.backend
            .require_shell(&self.target, &script, "clean the owned Mac fixture")
            .await
            .is_ok()
    }
}

/// Send a command only after this attachment's replay completes, then require a
/// fresh transformed token in the grid. The expected uppercase token is absent
/// from the command, so terminal echo cannot masquerade as executed input.
async fn mac_command(
    client: &crate::client::SupervisorClient,
    session: &str,
    selector: farhelm_proto::TerminalSelector,
    command: &str,
) {
    use crate::client::TermEvent;
    use farhelm_proto::grid::{GridInput, GridOutput, Row};
    let token = format!("macinput{}", uuid::Uuid::new_v4().simple());
    let expected = token.to_ascii_uppercase();
    let (channel, mut terminal) = client
        .attach_terminal(session, 160, 24, selector, "")
        .await
        .expect("attach the owned terminal");
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            match terminal.recv().await {
                Some(TermEvent::ReplayComplete) => break,
                Some(TermEvent::Data(_)) => {}
                _ => panic!("owned terminal ended before replay completed"),
            }
        }
        let command = format!("{command} && printf '%s\\n' '{token}' | tr '[:lower:]' '[:upper:]'");
        for input in [
            GridInput::Text { data: command },
            GridInput::Key {
                key: "Enter".into(),
                code: "Enter".into(),
                kc: 13,
                mods: 0,
                mac: true,
            },
        ] {
            client
                .send_grid_input(channel, input.encode().unwrap())
                .await
                .expect("send input to the owned terminal");
        }
        let text = |row: &Row| {
            row.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>()
        };
        loop {
            let found = match terminal.recv().await {
                Some(TermEvent::Data(bytes)) => match GridOutput::decode(&bytes).unwrap() {
                    GridOutput::Rows { rows, .. } => {
                        rows.iter().any(|row| text(&row.row).contains(&expected))
                    }
                    GridOutput::History { rows, .. } => {
                        rows.iter().any(|row| text(row).contains(&expected))
                    }
                    _ => false,
                },
                Some(TermEvent::ReplayComplete) => false,
                _ => panic!("owned terminal ended before its fresh input completed"),
            };
            if found {
                break;
            }
        }
    })
    .await
    .expect("the owned terminal did not execute fresh post-replay input");
    client.detach(channel).await;
}

/// Build a helm whose registry has never registered the fixture's Mac.
///
/// Only its reserved Local row may exist before actors start. Reusing the
/// payload source and frozen remote paths supplies no discovery state; each
/// helm gets its own database, transport and control directory.
async fn fresh_mac_helm(
    helm_state: PathBuf,
    fixture: &OwnedMac,
    payloads: &Arc<MacPayload>,
    local_farhelm: &Path,
) -> (HelmStore, Arc<ConnectionManager>, Arc<ProvisioningService>) {
    assert!(
        !helm_state.join("helm.db").exists(),
        "a fresh helm must start without a registry database"
    );
    // Every replacement helm owns a new directory, not only a new database.
    // The store opens its file there; it does not create the parent for us.
    std::fs::create_dir_all(&helm_state).expect("create the fresh helm state directory");
    let store = HelmStore::open(&helm_state.join("helm.db")).await.unwrap();
    let rows = store.list_hosts().await.unwrap();
    assert_eq!(rows.len(), 1, "only the reserved Local row exists");
    assert!(rows.iter().all(|row| row.kind == HostKind::Local));
    let backend = Arc::new(SystemBackend::new(helm_state.clone()));
    let manager = ConnectionManager::start(
        store.clone(),
        Arc::new(crate::manager::SystemTransport::new(&helm_state)),
        crate::manager::Cadence::default(),
    )
    .await
    .unwrap();
    let service = ProvisioningService::injected(
        store.clone(),
        Arc::clone(&manager),
        backend as Arc<dyn ProvisioningBackend>,
        Arc::clone(payloads) as Arc<dyn PayloadSource>,
        PlanLayout {
            local_state_dir: helm_state,
            override_lib_dir: Some(fixture.lib.clone()),
            override_farhelm_path: None,
            override_state_dir: Some(fixture.state.clone()),
            override_unit_dir: Some(fixture.plist.parent().unwrap().into()),
            unit_name: fixture.label.clone(),
        },
        local_farhelm.to_owned(),
    );
    (store, manager, service)
}

/// Wait for the manager's published replacement connection, rather than merely
/// seeing the old client which Update, a crash or discovery is retiring.
async fn mac_new_connection(manager: &ConnectionManager, host: HostId, previous: u64) {
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            if manager
                .status(host)
                .is_some_and(|status| status.incarnation != previous && status.client.is_some())
            {
                return;
            }
            // sleep-ok: poll the manager's new connected incarnation, bounded by the outer deadline.
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("the Mac supervisor did not reconnect with a new incarnation");
}

/// The immutable observations which distinguish retained terminals from new
/// shells reconstructed around the same session and tab IDs.
#[derive(Clone, Copy)]
struct MacTerminals<'a> {
    session: &'a str,
    tab: &'a str,
    nonce: &'a str,
    identity: &'a str,
    processes: &'a [(u32, String)],
}

/// Both terminal identities and program state must survive each disruption.
/// A fresh post-replay command covers agent and tab independently; a listing
/// alone would accept terminals reconstructed around restarted shells. A status
/// deadline records whether the original processes still exist, rather than
/// leaving a slow classification indistinguishable from a lost program.
async fn mac_assert_survival(
    fixture: &OwnedMac,
    manager: &ConnectionManager,
    host: HostId,
    expected: MacTerminals<'_>,
    phase: &str,
) {
    eprintln!("Mac fixture: {phase}");
    let MacTerminals {
        session,
        tab,
        nonce,
        identity,
        processes,
    } = expected;
    let client = wait_real_client(manager, host).await;
    assert_eq!(client.peer_hello().host_identity.as_deref(), Some(identity));
    // A completed hello can precede the recovered terminal's first status
    // sample. Match the existing real-host fixture's readiness contract:
    // wait only for Unknown, and never wait away a definite ended status.
    let mut last_status = None;
    let observed = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let listing = client
                .list_sessions()
                .await
                .expect("list the owned Mac session");
            let row = listing
                .sessions
                .into_iter()
                .find(|row| row.id == session)
                .expect("same session id");
            last_status = Some(row.status.clone());
            if !matches!(row.status, farhelm_proto::SessionStatus::Unknown) {
                break row;
            }
            // sleep-ok: bound the wait for a recovered terminal's first classified status sample.
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    let processes_unchanged = fixture.terminal_processes().await.as_slice() == processes;
    let row = observed.unwrap_or_else(|_| mac_fixture_failure(
        "Mac agent status did not become ready",
        format!("{phase}: last status {last_status:?}; four process identities unchanged: {processes_unchanged}"),
    ));
    if !row.status.is_live() {
        mac_fixture_failure(
            "Mac agent survival failed",
            format!(
                "{phase}: status {:?}; four process identities unchanged: {processes_unchanged}",
                row.status
            ),
        );
    }
    assert!(
        row.tabs.iter().any(|row| row.id == tab),
        "the same host tab is still listed"
    );
    assert!(
        processes_unchanged,
        "{phase}: an owned terminal process changed"
    );
    let check = format!(
        "test \"$farhelm_mac_nonce\" = {}",
        crate::ssh::shell_quote(nonce)
    );
    mac_command(
        &client,
        session,
        farhelm_proto::TerminalSelector::Agent,
        &check,
    )
    .await;
    mac_command(
        &client,
        session,
        farhelm_proto::TerminalSelector::Tab { id: tab.into() },
        &check,
    )
    .await;
}

/// Real Mac setup, Update, SIGKILL recovery and rediscovery preserve an agent and
/// a tab; uninstall ends their retained hosts while keeping the data directory.
///
/// This intentionally never silently skips. Set SSH destination, valid Darwin
/// FARHELM_TEST_BINARY, and an operator-owned FARHELM_TEST_MAC_ROOT=/tmp/<uuid>.
/// FARHELM_TEST_MAC_UPDATE_BINARY optionally supplies different signed bytes;
/// without it Update still exercises the production launchd restart with the
/// same payload. The parent recorder must retain all four named inputs.
#[farhelm_testtrace::test]
#[ignore = "requires an explicitly owned remote Apple-silicon Mac and Darwin payload"]
async fn remote_macos_setup_update_crash_recovery_and_uninstall() {
    let payload = darwin_payload(PathBuf::from(required_input("FARHELM_TEST_BINARY")));
    let update = std::env::var_os("FARHELM_TEST_MAC_UPDATE_BINARY")
        .map(PathBuf::from)
        .map(darwin_payload);
    if let Some(update) = &update {
        assert_ne!(
            hex_sha256(&std::fs::read(&payload).unwrap()),
            hex_sha256(&std::fs::read(update).unwrap()),
            "a supplied update payload must contain different valid Darwin bytes"
        );
    }
    let local = tempfile::tempdir().unwrap();
    let helm_state = local.path().join("helm");
    // Fixture allocation itself uses SSH sharing before the database is open.
    // OpenSSH needs its local ControlPath parent to exist for that first command.
    std::fs::create_dir_all(&helm_state).unwrap();
    let backend = Arc::new(SystemBackend::new(helm_state.clone()));
    let fixture = OwnedMac::create(
        Arc::clone(&backend),
        required_input("FARHELM_TEST_SSH_DESTINATION"),
    )
    .await;
    let payloads = Arc::new(MacPayload(Mutex::new(payload.clone())));
    let (mut store, mut manager, mut service) =
        fresh_mac_helm(helm_state, &fixture, &payloads, &payload).await;
    let mut active_host = None;
    // Keep cleanup outside the caught future. Ordinary errors and assertion
    // panics alike must abort provisioning before stopping its frozen job.
    let outcome = std::panic::AssertUnwindSafe(async {
        fixture.shell(&format!("{}\n[ \"$active\" = inactive ] && [ ! -e {lib} ] && [ ! -e {state} ]",
            fixture.job_script(), lib = shell_path(&fixture.lib).unwrap(), state = shell_path(&fixture.state).unwrap()),
            "confirm the owned installation is absent before setup").await;
        let ProvisioningTarget::Remote(crate::access::RemoteRoute::Ssh { destination, .. }) = &fixture.target else { unreachable!() };
        let probe = || ProbeRequest {
            target: ProbeDestination::Ssh { destination: destination.clone() },
            remote_farhelm: Some(fixture.lib.join("farhelm").to_str().unwrap().into()),
            remote_state_dir: Some(fixture.state.to_str().unwrap().into()), new_entry: None,
        };
        let response = service.probe(probe()).await.unwrap_or_else(|error| mac_fixture_failure("Mac setup probe failed", error));
        let ProbeResponse::Provisionable { probe_id, plan, .. } = response else { panic!("isolated Mac setup did not offer a plan") };
        fixture.assert_plan(&plan);
        let accepted = service.start_add(ProvisionRequest { probe_id }).await
            .unwrap_or_else(|_| panic!("Mac setup confirmation failed"));
        let mut host = accepted.host_id;
        active_host = Some(host);
        let completed = wait_real_run(&service, host).await.unwrap_or_else(|_| panic!("Mac setup stalled"));
        if completed.status != RunStatus::Completed {
            mac_fixture_failure("Mac setup failed", format!("{completed:?}"));
        }
        fixture.shell(&format!("codesign --verify --strict {} && {} --version && printf 'kept\\n' > {}",
            shell_path(&fixture.lib.join("farhelm")).unwrap(), shell_path(&fixture.lib.join("farhelm")).unwrap(),
            shell_path(&fixture.state.join("kept-by-uninstall")).unwrap()), "verify the installed Darwin executable").await;
        let cwd = fixture.root.to_str().unwrap().to_owned();
        let key = uuid::Uuid::new_v4().to_string();
        let session = through_the_current_connection(&manager, host, "create the owned Mac agent", |client| {
            let cwd = cwd.clone(); let key = key.clone();
            async move { client.create_session_with_key(&cwd, "/bin/sh", None, 160, 24, Some(key)).await }
        }).await;
        let client = wait_real_client(&manager, host).await;
        let identity = client.peer_hello().host_identity.clone().expect("Mac supervisor has a durable identity");
        let tab = client.open_tab(&session.id).await.expect("open the owned Mac tab");
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        for (selector, name) in [(farhelm_proto::TerminalSelector::Agent, "agent.pid"),
            (farhelm_proto::TerminalSelector::Tab { id: tab.id.clone() }, "tab.pid")] {
            mac_command(&client, &session.id, selector, &format!(
                "farhelm_mac_nonce={} && printf '%s %s\\n' \"$$\" \"$PPID\" > {}",
                crate::ssh::shell_quote(&nonce), shell_path(&fixture.root.join(name)).unwrap())).await;
        }
        let processes = fixture.terminal_processes().await;
        let retained = MacTerminals { session: &session.id, tab: &tab.id, nonce: &nonce, identity: &identity, processes: &processes };
        mac_assert_survival(&fixture, &manager, host, retained, "initial setup survival").await;

        let before_update = fixture.supervisor_pid().await;
        if let Some(update) = &update { *payloads.0.lock().unwrap() = update.clone(); }
        let planned = service.plan_update(host).await.unwrap_or_else(|_| panic!("Mac Update planning failed"));
        fixture.assert_plan(&planned.plan);
        assert!(planned.plan.actions.iter().any(|action| matches!(action, ProvisioningAction::StartLaunchAgent { refresh: true, .. })));
        service.start_update(host, ProvisionRequest { probe_id: planned.probe_id }).await
            .unwrap_or_else(|_| panic!("Mac Update confirmation failed"));
        let completed = wait_real_run(&service, host).await.unwrap_or_else(|_| panic!("Mac Update stalled"));
        if completed.status != RunStatus::Completed {
            mac_fixture_failure("Mac Update failed", format!("{completed:?}"));
        }
        assert_ne!(fixture.supervisor_pid().await, before_update, "Update replaced the supervisor process");
        let expected = hex_sha256(&std::fs::read(update.as_ref().unwrap_or(&payload)).unwrap());
        let hash = fixture.shell(&format!("codesign --verify --strict {} && sha256sum -- {}",
            shell_path(&fixture.lib.join("farhelm")).unwrap(), shell_path(&fixture.lib.join("farhelm")).unwrap()),
            "verify the Update payload and signature").await;
        assert_eq!(hash.split_whitespace().next(), Some(expected.as_str()));
        mac_assert_survival(&fixture, &manager, host, retained, "Update survival").await;

        let incarnation = manager.status(host).unwrap().incarnation;
        let before_crash = fixture.supervisor_pid().await;
        fixture.shell(&format!("{}\n[ \"$active\" = active ] || exit 1\nlaunchctl kill SIGKILL \"$job_target\"",
            fixture.job_script()), "SIGKILL the owned supervisor through launchd").await;
        mac_new_connection(&manager, host, incarnation).await;
        assert_ne!(fixture.supervisor_pid().await, before_crash, "launchd restarted the crashed supervisor");
        mac_assert_survival(&fixture, &manager, host, retained, "crash survival").await;

        let before_discovery = fixture.supervisor_pid().await;
        let incarnation = manager.status(host).unwrap().incarnation;
        let discovered = service.probe(probe()).await.unwrap_or_else(|_| panic!("Mac rediscovery failed"));
        let ProbeResponse::Discovered { host_id, host_identity, .. } = discovered else { panic!("Mac rediscovery did not adopt the supervisor") };
        assert_eq!(host_id, host); assert_eq!(host_identity.as_deref(), Some(identity.as_str()));
        mac_new_connection(&manager, host, incarnation).await;
        assert_eq!(fixture.supervisor_pid().await, before_discovery, "discovery restarted the supervisor");
        mac_assert_survival(&fixture, &manager, host, retained, "existing-registry discovery survival").await;

        // Join the original helm before another one can discover this Mac.
        // A new database must learn the running installation independently,
        // without an old actor's connection or cached session list helping it.
        let before_registry_loss = fixture.supervisor_pid().await;
        let original_entry = store.entry_uuid_of(host).await.unwrap().expect("original host entry");
        let retired_client = wait_real_client(&manager, host).await;
        service.abort_run(host).await;
        tokio::time::timeout(Duration::from_secs(30), manager.shutdown_and_wait())
            .await.expect("the original helm's actors did not finish");
        assert!(manager.snapshots().is_empty(), "the original helm still routes a host");
        tokio::time::timeout(Duration::from_secs(10), retired_client.closed())
            .await.expect("the original helm left its supervisor connection open");
        drop(retired_client);
        active_host = None;
        (store, manager, service) = fresh_mac_helm(
            local.path().join("helm-after-registry-loss"), &fixture, &payloads, &payload,
        ).await;
        let recovered = service.probe(probe()).await
            .unwrap_or_else(|_| panic!("the fresh helm's Mac discovery failed"));
        let ProbeResponse::Discovered { host_id, host_identity, .. } = recovered else {
            panic!("the fresh helm offered setup instead of adopting the running Mac")
        };
        host = host_id;
        active_host = Some(host);
        assert_eq!(host_identity.as_deref(), Some(identity.as_str()));
        let recovered_entry = store.entry_uuid_of(host).await.unwrap().expect("recovered host entry");
        assert_ne!(recovered_entry, original_entry, "recovery reused the original helm entry");
        let registered = store.list_hosts().await.unwrap();
        let remote_rows = registered.iter().filter(|row| row.kind != HostKind::Local).collect::<Vec<_>>();
        assert_eq!(remote_rows.len(), 1, "the fresh helm must register exactly one remote host");
        assert_eq!(remote_rows[0].id, host);
        assert_eq!(remote_rows[0].host_identity.as_deref(), Some(identity.as_str()));
        assert_eq!(fixture.supervisor_pid().await, before_registry_loss, "lost-registry recovery restarted the supervisor");
        mac_assert_survival(&fixture, &manager, host, retained, "lost-registry discovery survival").await;

        let client = wait_real_client(&manager, host).await;
        client.close_tab(&session.id, &tab.id).await.expect("end the owned tab");
        client.stop_session(&session.id).await.expect("end the owned agent");
        let listing = client.list_sessions().await.expect("list the ended Mac agent");
        assert!(listing.sessions.iter().any(|row| row.id == session.id && row.status.has_ended()));
        let planned = service.plan_uninstall(host).await.unwrap_or_else(|_| panic!("Mac uninstall planning failed"));
        fixture.assert_plan(&planned.plan);
        assert!(planned.plan.actions.iter().any(|action| matches!(action, ProvisioningAction::StopLaunchAgent { .. })));
        service.start_uninstall(host, ProvisionRequest { probe_id: planned.probe_id }).await
            .unwrap_or_else(|_| panic!("Mac uninstall confirmation failed"));
        tokio::time::timeout(STEP_STALL_TIMEOUT, async {
            loop {
                if store.list_hosts().await.unwrap().iter().all(|row| row.id != host) && manager.status(host).is_none() { break; }
                if let Ok(view) = service.view(host).await { assert_ne!(view.status, RunStatus::Failed, "Mac uninstall failed"); }
                // sleep-ok: poll the registry and actor removal which complete a real uninstall.
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }).await.expect("Mac uninstall did not forget the owned host");
        fixture.shell(&format!("{}\n[ \"$active\" = inactive ] && [ ! -e {plist} ] && [ ! -e {lib} ] && [ ! -e {hosts} ] && [ \"$(cat {kept})\" = kept ]",
            fixture.job_script(), plist = shell_path(&fixture.plist).unwrap(), lib = shell_path(&fixture.lib).unwrap(),
            hosts = shell_path(&fixture.state.join("hosts")).unwrap(), kept = shell_path(&fixture.state.join("kept-by-uninstall")).unwrap()),
            "verify job and hosts gone with data kept").await;
        for (pid, identity) in &processes {
            let current = fixture.shell(&format!(
                "if found=$(ps -p {pid} -o pid=,lstart=,command= 2>&1); then printf '%s\\n' \"$found\"; else status=$?; [ \"$status\" = 1 ] && [ -z \"$found\" ] || exit 1; fi"),
                "check the former terminal process after uninstall").await;
            assert_ne!(&current, identity, "uninstall left an owned terminal process running");
        }
    }).catch_unwind().await;
    if let Some(host) = active_host {
        service.abort_run(host).await;
    }
    // A failed local join must still reach the frozen remote cleanup below.
    let manager_stopped =
        tokio::time::timeout(Duration::from_secs(30), manager.shutdown_and_wait())
            .await
            .is_ok();
    let cleaned = fixture.cleanup().await;
    drop(service);
    drop(manager);
    drop(store);
    assert!(
        manager_stopped,
        "the current helm's actors did not finish before remote teardown"
    );
    assert!(
        cleaned,
        "Mac fixture cleanup could not confirm its own job and hosts stopped; retained owned root for investigation"
    );
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}
/// Append private observations without holding the ledger across an SSH spawn.
/// Each value is an object. The ledger excludes the SSH destination but retains
/// frozen cleanup paths and local timestamps for the parent's reboot accounting.
fn mac_reboot_note(ledger: &Path, mut value: serde_json::Value) {
    use std::io::Write as _;
    value["local_unix_ms"] = serde_json::json!(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(ledger)
        .unwrap();
    serde_json::to_writer(&mut file, &value).unwrap();
    file.write_all(b"\n").unwrap();
    file.sync_all().unwrap();
}

impl OwnedMac {
    /// Freeze one home-root fixture and one real LaunchAgents entry before any
    /// remote write. The existing shared directory is a fixture prerequisite;
    /// neither its permissions nor its other jobs belong to this test.
    async fn for_reboot(backend: Arc<SystemBackend>, ledger: &Path) -> Self {
        use std::os::unix::fs::OpenOptionsExt as _;
        let target = ProvisioningTarget::ssh(required_input("FARHELM_TEST_SSH_DESTINATION"));
        let reply = backend.require_shell(&target,
            "[ \"$(uname -s)\" = Darwin ] && [ \"$(uname -m)\" = arm64 ] || exit 1\n\
             home=$(readlink -f -- \"$HOME\") && jobs=$(readlink -f -- \"$HOME/Library/LaunchAgents\") || exit 1\n\
             [ -d \"$home\" ] && [ -d \"$jobs\" ] || exit 1\n\
             printf '%s\\0%s\\0' \"$home\" \"$jobs\"; id -u",
            "inspect reboot fixture coordinates").await
            .unwrap_or_else(|_| panic!("the reboot fixture needs a reachable Mac and an existing LaunchAgents directory"));
        let text = std::str::from_utf8(&reply.stdout).unwrap();
        let parts = text.split('\0').collect::<Vec<_>>();
        assert_eq!(parts.len(), 3);
        let home = PathBuf::from(parts[0]);
        let jobs = PathBuf::from(parts[1]);
        assert!(home.is_absolute() && jobs.is_absolute());
        shell_path(&home).unwrap();
        shell_path(&jobs).unwrap();
        let uid: u32 = parts[2].trim().parse().unwrap();
        let supplied = PathBuf::from(required_input("FARHELM_TEST_MAC_ROOT"));
        let owner = uuid::Uuid::parse_str(supplied.file_name().unwrap().to_str().unwrap()).unwrap();
        let nonce = uuid::Uuid::new_v4();
        let parent = home.join(owner.to_string());
        let root = parent.join(format!("b-{}", &nonce.simple().to_string()[..8]));
        let label = format!("io.farhelm.reboot.{}", nonce.simple());
        let fixture = Self {
            backend,
            target,
            state: root.join("s"),
            lib: root.join("lib"),
            plist: jobs.join(format!("{label}.plist")),
            root,
            domain: format!("user/{uid}"),
            label,
        };
        assert!(
            fixture.state.join("supervisor.sock").as_os_str().len() < 104,
            "the reboot fixture home is too long for a Darwin supervisor socket"
        );
        fixture
            .shell(
                &format!(
                    "{}\n[ \"$active\" = inactive ] && [ ! -e {plist} ] && [ ! -L {plist} ] && \
             [ ! -e {root} ] && [ ! -L {root} ] && [ ! -L {parent} ] || exit 1",
                    fixture.job_script(),
                    plist = shell_path(&fixture.plist).unwrap(),
                    root = shell_path(&fixture.root).unwrap(),
                    parent = shell_path(&parent).unwrap()
                ),
                "prove the nonce job and paths are absent",
            )
            .await;
        assert!(
            ledger.is_absolute(),
            "supply an absolute private reboot ledger path"
        );
        drop(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(ledger)
                .unwrap(),
        );
        mac_reboot_note(
            ledger,
            serde_json::json!({ "phase": "planned", "root": fixture.root,
            "state": fixture.state, "lib": fixture.lib, "plist": fixture.plist,
            "domain": fixture.domain, "label": fixture.label }),
        );
        fixture
    }
}

/// Use production SSH construction and bounded capture while retaining status
/// 255 as transport loss. A native command error must never become job absence.
async fn mac_reboot_shell(
    fixture: &OwnedMac,
    script: &str,
    budget: Duration,
) -> Option<CommandResult> {
    let ProvisioningTarget::Remote(route) = &fixture.target else {
        unreachable!()
    };
    let mut command = fixture
        .backend
        .remote_command(route, script.into())
        .unwrap_or_else(|_| panic!("could not construct the reboot observation"));
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    isolate_process_group(&mut command);
    let child = command
        .spawn()
        .unwrap_or_else(|_| panic!("could not start the reboot observation"));
    capture_child(child, budget, "the reboot observation")
        .await
        .ok()
}

/// Read process names and argv, never runner files, and wait while a native or
/// dotnet-hosted worker is present. The filter runs in this test, so its own SSH
/// command cannot match. No process found by this observation is signalled.
async fn mac_wait_runner_idle(fixture: &OwnedMac) {
    tokio::time::timeout(Duration::from_secs(30 * 60), async {
        loop {
            let names = fixture
                .shell("ps -axo comm=,args=", "inspect runner idleness")
                .await;
            assert!(
                !names.trim().is_empty(),
                "the process inspection returned no processes"
            );
            if !names.split_whitespace().any(|word| {
                matches!(
                    Path::new(word)
                        .file_name()
                        .and_then(std::ffi::OsStr::to_str),
                    Some("Runner.Worker" | "Runner.Worker.dll")
                )
            }) {
                return;
            }
            // sleep-ok: wait for the observed Actions worker to finish before the authorized reboot.
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    })
    .await
    .expect("the Actions worker stayed busy; no reboot was attempted");
}

/// A loaded job alone is not readiness: prove the real supervisor answers from
/// this fixture's frozen program and state before relying on its lifecycle.
async fn mac_reboot_ready(fixture: &OwnedMac) {
    let target = ProbeTarget {
        transport: fixture.target.clone(),
        probe_farhelm: fixture.lib.join("farhelm"),
        probe_state_dir: Some(fixture.state.clone()),
    };
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            match fixture.backend.probe(&target).await {
                Ok(ProbeObservation::Supervisor {
                    dial_farhelm,
                    dial_state_dir,
                    ..
                }) => {
                    assert_eq!(dial_farhelm, target.probe_farhelm);
                    assert_eq!(dial_state_dir.as_deref(), Some(fixture.state.as_path()));
                    return;
                }
                Ok(ProbeObservation::Absent) => {}
                _ => panic!("the owned reboot supervisor did not answer the expected protocol"),
            }
            // sleep-ok: poll the owned supervisor's completed protocol hello, not elapsed startup time.
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("the owned reboot supervisor never became ready");
}

/// Read a baseline, or the first successful observation from a different boot.
/// The baseline exercises every native parser before reboot is attempted.
/// Retries cover only SSH loss or a capture timeout; a native command error,
/// including production's strict job inspection, fails immediately.
async fn mac_boot_observation(
    fixture: &OwnedMac,
    previous: Option<&str>,
    ledger: &Path,
    budget: Duration,
) -> Vec<String> {
    let script = format!(
        r#"export LC_ALL=C TZ=UTC
boot=$(sysctl -n kern.bootsessionuuid) && now=$(date +%s) && times=$(sysctl -n kern.boottime) || exit 1
seconds=${{times#*sec = }}; seconds=${{seconds%%,*}}
case "$seconds" in ''|*[!0-9]*) exit 1 ;; esac
{}
pid=0; started=0
if [ "$active" = active ]; then
    candidate=$(printf '%s\n' "$job_output" | awk '$1 == "pid" && $2 == "=" {{ print $3 }}') || exit 1
    if [ -n "$candidate" ]; then
        case "$candidate" in *[!0-9]*) exit 1 ;; esac
        pid=$candidate
        born=$(ps -p "$pid" -o lstart=) && started=$(date -j -f '%a %b %e %T %Y' "$born" +%s) || exit 1
    fi
fi
printf '%s\0%s\0%s\0%s\0%s\0%s\0' "$boot" "$now" "$seconds" "$active" "$pid" "$started"
"#,
        fixture.job_script()
    );
    let deadline = tokio::time::Instant::now() + budget;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        assert!(
            !remaining.is_zero(),
            "the sandbox Mac did not complete its boot observation within the declared budget"
        );
        let attempted = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        // Let capture_child finish its own termination/reap path; cancelling
        // that future at the outer deadline could strand an SSH helper.
        let response =
            mac_reboot_shell(fixture, &script, remaining.min(Duration::from_secs(10))).await;
        mac_reboot_note(
            ledger,
            serde_json::json!({ "phase": "ssh-observation", "attempted_unix_ms": attempted,
                "status": response.as_ref().and_then(|value| value.code) }),
        );
        if let Some(response) = response {
            match response.code {
                Some(0) => {
                    let text = String::from_utf8(response.stdout).unwrap();
                    let fields = text
                        .split_terminator('\0')
                        .map(str::to_owned)
                        .collect::<Vec<_>>();
                    assert_eq!(fields.len(), 6, "malformed reboot observation");
                    uuid::Uuid::parse_str(&fields[0]).expect("boot UUID");
                    for index in [1, 2, 4, 5] {
                        fields[index]
                            .parse::<u64>()
                            .expect("numeric reboot observation");
                    }
                    assert!(matches!(fields[3].as_str(), "active" | "inactive"));
                    if previous.is_none_or(|before| fields[0] != before) {
                        return fields;
                    }
                }
                Some(255) => {}
                _ => panic!("post-reboot native inspection failed; this is not job absence"),
            }
        }
        // sleep-ok: bounded SSH recovery polling waits for a changed boot UUID after the authorized reboot.
        tokio::time::sleep(
            deadline
                .saturating_duration_since(tokio::time::Instant::now())
                .min(Duration::from_secs(2)),
        )
        .await;
    }
}

/// Measure real LaunchAgents loading across an explicitly authorized reboot.
///
/// This records presence at the first successful post-boot query, not a promise
/// that it preceded SSH login: the parent's resource sampler may log in first.
/// A fresh, private FARHELM_TEST_MAC_REBOOT_LEDGER retains cleanup authority.
/// FARHELM_TEST_MAC_ALLOW_REBOOT=1 is required in addition to the ordinary Mac
/// payload/destination/root inputs. Run this case alone, outside nextest's
/// ordinary five-minute timeout; the parent must log the reboot before launch.
#[farhelm_testtrace::test]
#[ignore = "reboots an explicitly authorized sandbox Mac after checking runner idleness"]
async fn remote_macos_reboot_job_measurement() {
    assert_eq!(required_input("FARHELM_TEST_MAC_ALLOW_REBOOT"), "1");
    let ledger = PathBuf::from(required_input("FARHELM_TEST_MAC_REBOOT_LEDGER"));
    let payload = darwin_payload(PathBuf::from(required_input("FARHELM_TEST_BINARY")));
    let local = tempfile::tempdir().unwrap();
    // OpenSSH creates its shared-connection socket on the first observation,
    // before a store or provisioning run has any opportunity to make this path.
    let control = local.path().join("ssh");
    std::fs::create_dir_all(&control).unwrap();
    let fixture = OwnedMac::for_reboot(Arc::new(SystemBackend::new(control)), &ledger).await;
    // Cleanup stays outside the caught future so an assertion or a refused
    // reboot cannot bypass removal of the frozen job and its one shared file.
    let outcome = std::panic::AssertUnwindSafe(async {
        fixture.shell(&format!("umask 077; mkdir -p -- {} && mkdir -- {}",
            shell_path(fixture.root.parent().unwrap()).unwrap(), shell_path(&fixture.root).unwrap()),
            "create the owned home reboot root").await;
        fixture.backend.ensure_directories(&fixture.target, &[
            DirectorySpec { path: fixture.lib.clone(), mode: 0o755, shared: false },
            DirectorySpec { path: fixture.state.clone(), mode: 0o700, shared: false },
        ]).await.unwrap_or_else(|_| panic!("could not create the owned reboot installation"));
        let staged = stage_payload(&payload).await.unwrap();
        let program = fixture.lib.join("farhelm");
        let temporary = fixture.lib.join(format!(".farhelm.farhelm-tmp-{}", uuid::Uuid::new_v4()));
        fixture.backend.upload_path(&fixture.target, &staged, &program, &temporary).await
            .unwrap_or_else(|_| panic!("could not upload the owned reboot payload"));
        fixture.backend.install_path(&fixture.target, &staged, &program, &temporary, 0o755).await
            .unwrap_or_else(|_| panic!("could not install the owned reboot payload"));
        let plist = super::super::macos::supervisor_plist(&program, &fixture.state, &fixture.label).unwrap();
        // Exclusive creation adds only this nonce entry to the shared directory;
        // the ordinary atomic plist installation is covered by the lifecycle test.
        fixture.shell(&format!("umask 077; set -C; printf '%s' {} > {}",
            crate::ssh::shell_quote(&plist), shell_path(&fixture.plist).unwrap()), "create the owned reboot plist").await;
        fixture.backend.launch_agent(&fixture.target, &fixture.domain, &fixture.label, &fixture.plist, false).await
            .unwrap_or_else(|_| panic!("could not start the owned reboot job"));
        mac_reboot_ready(&fixture).await;
        fixture.shell(&format!("codesign --verify --strict {}", shell_path(&program).unwrap()),
            "verify the reboot payload signature").await;
        let baseline = mac_boot_observation(&fixture, None, &ledger, Duration::from_secs(30)).await;
        assert_eq!(baseline[3], "active");
        assert!(baseline[4].parse::<u32>().unwrap() > 1);
        let before = &baseline[0];
        mac_reboot_note(&ledger, serde_json::json!({ "phase": "ready", "boot": before, "pid": baseline[4],
            "remote_unix_seconds": baseline[1], "boot_unix_seconds": baseline[2], "process_start_unix_seconds": baseline[5] }));
        mac_wait_runner_idle(&fixture).await;
        let reboot_started = tokio::time::Instant::now();
        mac_reboot_note(&ledger, serde_json::json!({ "phase": "reboot-requested" }));
        let result = mac_reboot_shell(&fixture, "sudo -n shutdown -r now", Duration::from_secs(10)).await;
        if let Some(result) = result { assert!(matches!(result.code, Some(0 | 255)),
            "MEASUREMENT_UNAVAILABLE: passwordless reboot was refused; no reboot result is claimed"); }
        let budget = Duration::from_secs(30 * 60).saturating_sub(reboot_started.elapsed());
        let observed = mac_boot_observation(&fixture, Some(before), &ledger, budget).await;
        mac_reboot_note(&ledger, serde_json::json!({ "phase": "new-boot", "boot": observed[0],
            "remote_unix_seconds": observed[1], "boot_unix_seconds": observed[2], "loaded": observed[3],
            "pid": observed[4], "process_start_unix_seconds": observed[5], "boot_vs_ssh_attribution": "unresolved" }));
        println!("Mac reboot observed: loaded={}, pid={}, boot_seconds={}, process_start_seconds={}, observed_seconds={}; boot versus first SSH remains unresolved",
            observed[3], observed[4], observed[2], observed[5], observed[1]);
        // A first-query absence can race launchd's loading. Preserve it, then
        // observe a bounded window before explicit recovery can mask a later
        // automatic start. Absence in this window is not a forever guarantee.
        if observed[3] == "inactive" {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
            loop {
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() { break; }
                // sleep-ok: observe delayed automatic loading before explicit bootstrap changes the result.
                tokio::time::sleep(remaining.min(Duration::from_secs(2))).await;
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() { break; }
                let later = mac_boot_observation(&fixture, None, &ledger, remaining).await;
                assert_eq!(later[0], observed[0], "the Mac rebooted again during automatic-loading observation");
                mac_reboot_note(&ledger, serde_json::json!({ "phase": "automatic-loading-window",
                    "loaded": later[3], "pid": later[4], "remote_unix_seconds": later[1],
                    "process_start_unix_seconds": later[5] }));
                if later[3] == "active" { break; }
            }
        }
        // Observe before recovering. A successful bootstrap after absence proves
        // recovery, but would destroy evidence of automatic loading if done first.
        fixture.backend.launch_agent(&fixture.target, &fixture.domain, &fixture.label, &fixture.plist, false).await
            .unwrap_or_else(|_| panic!("explicit setup recovery after reboot failed"));
        mac_reboot_ready(&fixture).await;
        mac_reboot_note(&ledger, serde_json::json!({ "phase": "explicit-recovery-ready" }));
    }).catch_unwind().await;
    let cleaned = fixture.cleanup().await;
    mac_reboot_note(
        &ledger,
        serde_json::json!({ "phase": "cleanup", "confirmed": cleaned }),
    );
    if !cleaned {
        eprintln!(
            "owned reboot fixture cleanup is unconfirmed; retain its private ledger and root"
        );
    }
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    assert!(cleaned, "could not confirm owned reboot fixture cleanup");
}
