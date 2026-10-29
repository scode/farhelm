//! Real rootless Podman through the shipped launch shim and lifecycle handlers.
//! Tests use only an offline Ubuntu base and remove only their UUID containers.
//! Missing prerequisites fail unless the caller explicitly opts out; a skip never
//! certifies isolation or cleanup coverage.

use crate::harness::*;
use farhelm_proto::{
    ContainerConfig, ContainerEngine, LaunchHarness, LaunchSelection, SessionLaunch,
};
use std::path::{Path, PathBuf};
use std::process::Command;

/// An owned container name remains a cleanup handle even when an assertion panics
/// before Delete can run. The base image and unrelated storage are never removed.
struct ContainerCleanup(String);
impl Drop for ContainerCleanup {
    fn drop(&mut self) {
        let _ = Command::new("podman")
            .args(["rm", "-f", "--ignore", &self.0])
            .output();
    }
}

/// Read-only prerequisite checks cannot silently downgrade this suite's promises.
fn available() -> bool {
    if std::env::var("FARHELM_TEST_SKIP_PODMAN").as_deref() == Ok("1") {
        eprintln!(
            "SKIPPED containerized_sessions: explicitly disabled by FARHELM_TEST_SKIP_PODMAN=1"
        );
        return false;
    }
    let output = Command::new("podman")
        .args(["image", "exists", "docker.io/library/ubuntu:24.04"])
        .output()
        .expect("podman is required; opt out explicitly with FARHELM_TEST_SKIP_PODMAN=1");
    assert!(
        output.status.success(),
        "offline docker.io/library/ubuntu:24.04 is required; no test pulls an image"
    );
    true
}

/// Build a real managed-checkout registry fixture through its allocation primitives.
/// Merely making a Git folder would not establish the managed-membership premise.
fn managed_checkout(h: &Harness, root: &Path) -> PathBuf {
    use farhelm_supervisor::working_copies::{PlannedWorkingCopy, allocate, record_planned};
    let conn = farhelm_supervisor::db::open_private_existing(
        &h.state.path().join("supervisor.db"),
        "container registry fixture",
    )
    .unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    record_planned(
        &conn,
        &PlannedWorkingCopy {
            id: id.clone(),
            canonical_root: root.canonicalize().unwrap().to_str().unwrap().into(),
            repo_owner: "fixture".into(),
            repo_name: "project".into(),
            original_basename: "checkout,colon:".into(),
            origin_session_id: uuid::Uuid::new_v4().to_string(),
            root_identity: None,
            preparation_snapshot: None,
        },
    )
    .unwrap();
    let checkout = allocate(&conn, &id, None).unwrap().canonical_text;
    assert!(
        Command::new("git")
            .args(["init", "--quiet", "--initial-branch=main", &checkout])
            .status()
            .unwrap()
            .success()
    );
    checkout.into()
}

/// One launch with a real terminal but no vendor dependency. The declared harness
/// would receive reporter arguments ordinarily; container policy must strip them.
fn launch(config: ContainerConfig, script: String) -> SessionLaunch {
    SessionLaunch::Agent {
        selection: LaunchSelection {
            harness: LaunchHarness::Claude,
            model: None,
            effort: None,
            permissions: None,
            permission_flags: None,
            wrapper_prefix: None,
            container: Some(config),
            workspace_trust: None,
        },
        start: vec![
            "bash".into(),
            "-c".into(),
            script,
            "container-agent".into(),
            "{cwd}".into(),
            "{farhelm_args}".into(),
        ],
        resume: Some(vec![
            "bash".into(),
            "{conversation}".into(),
            "{farhelm_args}".into(),
        ]),
    }
}

/// Inspect the actual state rather than equating a Podman command exit with the
/// container's lifetime. This is also the premise before every teardown assertion.
fn state(name: &str) -> String {
    let output = Command::new("podman")
        .args([
            "container",
            "inspect",
            "--format",
            "{{.State.Status}}",
            name,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "inspect fixture: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

/// One loopback server supplies a positive proxy oracle. The second stays bound
/// without forwarding; reachability is tested both directly and via the gateway.
async fn listener() -> (u16, tokio::task::JoinHandle<()>) {
    use tokio::io::AsyncWriteExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let _ = stream.write_all(b"proxy-ready\n").await;
        }
    });
    (port, task)
}

/// Exercise the supported production lifecycle with and without a user-manager
/// scope. Actual in-container reads and writes distinguish isolation from an argv
/// snapshot, and a naturally exited agent still leaves a container Stop must end.
#[farhelm_testtrace::test]
async fn podman_isolation_and_stop_delete_cover_scoped_and_unscoped_launches() {
    if !available() {
        return;
    }
    let root = farhelm_teststate::tempdir().unwrap();
    let image = root.path().join("agent.oci");
    let saved = Command::new("podman")
        .args(["save", "--format", "oci-archive", "--output"])
        .arg(&image)
        .arg("docker.io/library/ubuntu:24.04")
        .output()
        .unwrap();
    assert!(saved.status.success(), "save offline fixture image");
    let token = root.path().join("token");
    std::fs::write(&token, " synthetic-container-token\n").unwrap();
    let home =
        tempfile::NamedTempFile::new_in(std::env::var_os("HOME").expect("host home")).unwrap();
    std::fs::write(home.path(), "host-home sentinel").unwrap();
    let (port, server) = listener().await;
    let (other, other_server) = listener().await;
    for scoped in [false, true] {
        let seams = if scoped {
            SupervisorSeams::default()
        } else {
            SupervisorSeams {
                scopes: Arc::new(farhelm_supervisor::scope::ScopeManager::disabled()),
                ..Default::default()
            }
        };
        let scopes = Arc::clone(&seams.scopes);
        let h = harness_with_seams(SupervisorTimeouts::default(), seams).await;
        let checkout_root = farhelm_teststate::tempdir().unwrap();
        let checkout = managed_checkout(&h, checkout_root.path());
        let cwd = checkout.join("nested");
        std::fs::create_dir(&cwd).unwrap();
        std::fs::write(checkout.join("parent-witness"), "checkout-parent").unwrap();
        let alias = checkout_root.path().join("alias");
        std::os::unix::fs::symlink(&cwd, &alias).unwrap();
        let outside = checkout_root.path().join("outside-checkout");
        std::fs::write(&outside, "host-only sentinel").unwrap();
        let config = ContainerConfig {
            engine: ContainerEngine::Podman,
            image_file: image.to_str().unwrap().into(),
            api_proxy_port: port,
            github_token_file: Some(token.to_str().unwrap().into()),
        };
        let refused = h
            .client
            .create_session_with_extras(
                root.path().to_str().unwrap(),
                launch(config.clone(), "exit 0".into()),
                None,
                80,
                24,
                Default::default(),
            )
            .await;
        assert!(format!("{:#}", refused.unwrap_err()).contains("managed checkout"));
        assert!(h.client.list_sessions().await.unwrap().sessions.is_empty());
        let quote = |path: &Path| shell_words::quote(path.to_str().unwrap()).into_owned();
        let script = format!(
            r#"
set -eu
test "$(id -u)" = 1000
test "$(id -g)" = 1000
test "$PWD" = {cwd}
test "$1" = {cwd}
test "$(cat ../parent-witness)" = checkout-parent
test -f ../.git/HEAD
test -z "${{FARHELM_SESSION_ID+x}}${{FARHELM_SESSION_TOKEN+x}}${{FARHELM_SUPERVISOR_SOCK+x}}${{FARHELM_AGENT_ID+x}}${{FARHELM_TAB_ID+x}}"
test -n "$GH_TOKEN"
printf %s "$GH_TOKEN" > token-witness
test "$#" = 1
test ! -e {outside}; (echo wrong > {outside}) 2>/dev/null || :
test ! -e {home}; (echo wrong > {home}) 2>/dev/null || :
test ! -e {state}; test ! -e {socket}
read -r response < /dev/tcp/127.0.0.1/{port}
test "$response" = proxy-ready
! timeout 2 bash -c 'echo wrong > /dev/tcp/127.0.0.1/{other}' 2>/dev/null
found=0
while read -r iface destination gateway rest; do
  if test "$destination" = 00000000; then
    found=1
    address="$((16#${{gateway:6:2}})).$((16#${{gateway:4:2}})).$((16#${{gateway:2:2}})).$((16#${{gateway:0:2}}))"
    ! timeout 2 bash -c "echo wrong > /dev/tcp/$address/{other}" 2>/dev/null
    break
  fi
done < /proc/net/route
test "$found" = 1
printf 'inside-owned\n' > witness
printf 'CONTAINER READY\n'
read -r finish
"#,
            outside = quote(&outside),
            cwd = quote(&cwd),
            home = quote(home.path()),
            state = quote(h.state.path()),
            socket = quote(&h.state.path().join("supervisor.sock"))
        );
        let session = h
            .client
            .create_session_with_extras(
                alias.to_str().unwrap(),
                launch(config, script),
                Some("container-fixture".into()),
                100,
                30,
                Default::default(),
            )
            .await
            .unwrap();
        let name = format!("farhelm-{}", session.id);
        let _cleanup = ContainerCleanup(name.clone());
        let (channel, mut stream) = h.client.attach(&session.id, 100, 30).await.unwrap();
        let mut seen = Vec::new();
        wait_for(&mut stream, &mut seen, "CONTAINER READY", 60).await;
        assert_eq!(state(&name), "running");
        assert_eq!(
            std::fs::read_to_string(&outside).unwrap(),
            "host-only sentinel"
        );
        assert_eq!(
            std::fs::read_to_string(home.path()).unwrap(),
            "host-home sentinel"
        );
        assert_eq!(
            std::fs::read_to_string(cwd.join("token-witness")).unwrap(),
            "synthetic-container-token"
        );
        let conmon = Command::new("podman")
            .args([
                "container",
                "inspect",
                "--format",
                "{{.State.ConmonPid}}",
                &name,
            ])
            .output()
            .unwrap();
        assert!(
            conmon.status.success(),
            "conmon inspect: {}",
            String::from_utf8_lossy(&conmon.stderr)
        );
        let pid: u32 = String::from_utf8(conmon.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let environment = std::fs::read(format!("/proc/{pid}/environ")).unwrap();
        assert!(
            !environment
                .split(|byte| *byte == 0)
                .any(|entry| entry.starts_with(b"FARHELM_")),
            "the shared container helper must not carry Farhelm ownership or credentials"
        );
        if scoped {
            if scopes.available().await {
                eprintln!("user manager available: asserting the dedicated container helper scope");
                let cgroup = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap();
                assert!(
                    cgroup.contains(&format!("farhelm-{}-container.scope", session.id)),
                    "container conmon must live outside the agent scope: {cgroup}"
                );
            } else {
                eprintln!(
                    "SKIPPED container helper scope assertion: no usable user manager; exercising unscoped fallback"
                );
            }
        }
        assert_eq!(
            std::fs::read_to_string(cwd.join("witness")).unwrap(),
            "inside-owned\n"
        );
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            std::fs::metadata(cwd.join("witness")).unwrap().uid(),
            unsafe { libc::geteuid() }
        );
        let guard = h
            .client
            .delete_session_with(&session.id, farhelm_proto::DeleteGuard::NothingAlive)
            .await;
        assert!(
            guard.is_err(),
            "a refused Delete cannot remove the container"
        );
        assert_eq!(state(&name), "running");
        assert!(h.client.restart_session(&session.id, true).await.is_err());
        if !scoped {
            // The dead-agent path still owns a running keeper that Stop must end.
            h.client.send_input(channel, b"finish\r".to_vec()).await;
            wait_for_exit(&h, &session.id).await;
            assert_eq!(
                state(&name),
                "running",
                "the container must outlive its agent exec"
            );
        }
        h.client.stop_session(&session.id).await.unwrap();
        assert_eq!(state(&name), "exited");
        h.client.delete_session(&session.id).await.unwrap();
        let exists = Command::new("podman")
            .args(["container", "exists", &name])
            .status()
            .unwrap();
        assert_eq!(exists.code(), Some(1));
    }
    failed_spec_retry_removes_its_predecessor_container(&image, &token, port).await;
    server.abort();
    other_server.abort();
}

/// A new spec-write failure does not prove that an older attempt started nothing.
/// Seed a real predecessor container behind a pending Create, then refuse the new
/// spec through actual directory permissions. Dropping the row before removing
/// that predecessor would leave an untracked running container.
async fn failed_spec_retry_removes_its_predecessor_container(
    image: &Path,
    token: &Path,
    port: u16,
) {
    use std::os::unix::fs::PermissionsExt;
    let fired = AtomicBool::new(false);
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            scopes: Arc::new(farhelm_supervisor::scope::ScopeManager::disabled()),
            faults: FaultHooks {
                create_crash: Some(Arc::new(move |stage| {
                    if stage == CreateStage::AfterRecord && !fired.swap(true, Ordering::SeqCst) {
                        anyhow::bail!("leave the predecessor pending");
                    }
                    Ok(())
                })),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .await;
    let root = farhelm_teststate::tempdir().unwrap();
    let cwd = managed_checkout(&h, root.path());
    let config = ContainerConfig {
        engine: ContainerEngine::Podman,
        image_file: image.to_str().unwrap().into(),
        api_proxy_port: port,
        github_token_file: Some(token.to_str().unwrap().into()),
    };
    let launch = launch(config, "read -r finished".into());
    let extras = || farhelm_helm::CreateExtras {
        intent_key: Some("container-spec-retry".into()),
        ..Default::default()
    };
    let refused = h
        .client
        .create_session_with_extras(
            cwd.to_str().unwrap(),
            launch.clone(),
            None,
            80,
            24,
            extras(),
        )
        .await;
    assert!(
        refused.is_err(),
        "the first attempt must stop at AfterRecord"
    );
    let store = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .unwrap();
    let reservation = store
        .reservation("container-spec-retry")
        .await
        .unwrap()
        .unwrap();
    let id = reservation.session_id;
    assert!(
        store.session(&id).await.unwrap().is_some(),
        "the pending row is the ownership premise"
    );
    let name = format!("farhelm-{id}");
    let _cleanup = ContainerCleanup(name.clone());
    let started = Command::new("podman")
        .args([
            "run",
            "-d",
            "--init",
            "--name",
            &name,
            "--userns=keep-id:uid=1000,gid=1000",
            "--network",
            "none",
        ])
        .arg("docker.io/library/ubuntu:24.04")
        .args(["sleep", "3600"])
        .output()
        .unwrap();
    assert!(
        started.status.success(),
        "seed predecessor: {}",
        String::from_utf8_lossy(&started.stderr)
    );
    assert_eq!(state(&name), "running");
    let directory = spec_path_for_launch(h.state.path(), &id, 0)
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::create_dir_all(&directory).unwrap();
    /// Keep failure injection from preventing ordinary fixture cleanup on panic.
    struct RestorePermissions(PathBuf);
    impl Drop for RestorePermissions {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o700));
        }
    }
    let _restore = RestorePermissions(directory.clone());
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500)).unwrap();
    assert!(
        std::fs::write(directory.join("write-probe"), b"must refuse").is_err(),
        "the fixture must genuinely refuse file publication"
    );
    let refused = h
        .client
        .create_session_with_extras(cwd.to_str().unwrap(), launch, None, 80, 24, extras())
        .await;
    assert!(
        refused.is_err(),
        "the retry's actual spec publication must fail"
    );
    assert!(
        store.session(&id).await.unwrap().is_none(),
        "confirmed cleanup may discard the failed row"
    );
    let exists = Command::new("podman")
        .args(["container", "exists", &name])
        .status()
        .unwrap();
    assert_eq!(
        exists.code(),
        Some(1),
        "the earlier container must be gone before the row is discarded"
    );
}

/// Wait on the supervisor's authoritative terminal status, not a printed line
/// just before exit. The container is still alive during this readiness wait.
async fn wait_for_exit(h: &Harness, id: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let sessions = h.client.list_sessions().await.unwrap();
        let session = sessions
            .sessions
            .iter()
            .find(|session| session.id == id)
            .expect("retained session");
        if matches!(session.status, farhelm_proto::SessionStatus::Exited { .. }) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "agent did not exit: {:?}",
            session.status
        );
        // sleep-ok: bounded polling of the existing supervisor terminal-status oracle.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
