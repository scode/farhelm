//! Real rootless Podman through the shipped launch shim and lifecycle handlers.
//! Tests use only an offline Ubuntu base and remove only their UUID containers.
//! Missing prerequisites fail unless the caller explicitly opts out; a skip never
//! certifies isolation or cleanup coverage.

use crate::harness::*;
use farhelm_proto::{
    ContainerConfig, ContainerEngine, LaunchHarness, LaunchSelection, SessionLaunch,
};
use farhelm_supervisor::container::HostAccount;
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

/// Check the account through the container's user database, not a particular UID.
/// A real write also catches a plausible HOME string whose directory is unusable;
/// the temporary file stays inside the disposable container's chosen home.
fn account_checks() -> &'static str {
    r#"account=$(getent passwd "$(id -u)") && test -n "$(id -un)" && test "$HOME" = "$(printf '%s\n' "$account" | cut -d: -f6)" && test -d "$HOME" && home_probe=$(mktemp "$HOME/.farhelm-home-XXXXXX") && rm -- "$home_probe""#
}

/// A host write must survive the keep-id mapping as the actual host account.
/// Injected account numbers describe the image-selection input, not host owners.
fn assert_host_owned(path: &Path) {
    use std::os::unix::fs::MetadataExt;
    assert_eq!(std::fs::metadata(path).unwrap().uid(), unsafe {
        libc::geteuid()
    });
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

/// Select competing facts from the unmodified offline Ubuntu image. HomeOwner
/// supplies daemon's UID so selecting Ubuntu proves rule 2 beat rule 3.
#[derive(Clone, Copy)]
enum AccountRule {
    Create,
    HomeOwner,
    Uid,
}

/// Establish the image-side premise before any checkout mount can create a home.
/// The inspection owns its UUID container and uses no volume, keep-id or workdir;
/// no derived image or host-account configuration is needed for these cases.
fn assert_image_account_premise(image: &str, host: &HostAccount, rule: AccountRule) {
    let name = format!("farhelm-account-premise-{}", uuid::Uuid::new_v4());
    let _cleanup = ContainerCleanup(name.clone());
    let boundary = match rule {
        AccountRule::Create => {
            r#"if getent passwd "$2"; then exit 1; else test "$?" = 2; fi
if getent group "$2"; then exit 1; else test "$?" = 2; fi
test ! -e "$3""#
        }
        AccountRule::HomeOwner => {
            r#"test "$(getent passwd "$2" | cut -d: -f1)" = daemon; test -d "$3"; test "$(stat -c %u "$3")" = 1000"#
        }
        AccountRule::Uid => {
            r#"test "$(getent passwd "$2" | cut -d: -f1)" = ubuntu; test ! -e "$3""#
        }
    };
    let script = format!(
        r#"
command -v getent >/dev/null
test "$(getent passwd ubuntu | cut -d: -f3-4)" = 1000:1000
test "$(getent passwd ubuntu | cut -d: -f6)" = /home/ubuntu
test -d /home/ubuntu
test "$(stat -c %u /home/ubuntu)" = 1000
test -f /etc/skel/.bashrc
test -f /etc/skel/.profile
if getent passwd "$1"; then exit 1; else test "$?" = 2; fi
{boundary}
printf 'IMAGE PREMISE READY\n'
"#
    );
    let output = Command::new("podman")
        .args([
            "run",
            "--rm",
            "--name",
            &name,
            "--pull=never",
            "--network",
            "none",
            "--user",
            "0:0",
            "--entrypoint",
            "/bin/sh",
            image,
            "-ec",
            &script,
            "image-premise",
            &host.name,
            &host.uid.to_string(),
        ])
        .arg(&host.home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "offline image account premise failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"IMAGE PREMISE READY\n");
    assert_eq!(
        Command::new("podman")
            .args(["container", "exists", &name])
            .status()
            .unwrap()
            .code(),
        Some(1),
        "the image-premise container must have been removed"
    );
}

/// Exercise selection through production Create, agent exec and a later tab.
/// For a created account the home is the checkout's parent: Podman's bind setup
/// creates that directory, but it must still receive skel without altering files
/// inside the bind. Each identity is injected, never installed on the host.
async fn exercise_account_rule(rule: AccountRule) {
    use crate::terminal_tabs::run_in_shell;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    if !available() {
        return;
    }
    let root = farhelm_teststate::tempdir().unwrap();
    let image = "docker.io/library/ubuntu:24.04";
    let host = HostAccount {
        name: format!("fh{}", &uuid::Uuid::new_v4().simple().to_string()[..20]),
        uid: match rule {
            AccountRule::Create => 23456,
            AccountRule::HomeOwner => 1,
            AccountRule::Uid => 1000,
        },
        gid: match rule {
            AccountRule::Create => 23456,
            AccountRule::HomeOwner => 1,
            AccountRule::Uid => 1000,
        },
        home: match rule {
            AccountRule::HomeOwner => PathBuf::from("/home/ubuntu"),
            AccountRule::Create | AccountRule::Uid => root.path().to_path_buf(),
        },
    };
    assert_image_account_premise(image, &host, rule);
    let (expected_name, expected_uid, expected_gid, expected_home) = match rule {
        AccountRule::Create => (host.name.as_str(), host.uid, host.gid, host.home.as_path()),
        AccountRule::HomeOwner | AccountRule::Uid => {
            ("ubuntu", 1000, 1000, Path::new("/home/ubuntu"))
        }
    };
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            container_host_account: Some(host.clone()),
            scopes: Arc::new(farhelm_supervisor::scope::ScopeManager::disabled()),
            ..Default::default()
        },
    )
    .await;
    let checkout = managed_checkout(&h, root.path());
    assert!(checkout.starts_with(root.path()));
    let protected = checkout.join(".bashrc");
    std::fs::write(
        &protected,
        "checkout content must not be seeded or chowned\n",
    )
    .unwrap();
    std::fs::set_permissions(&protected, std::fs::Permissions::from_mode(0o640)).unwrap();
    let before = std::fs::metadata(&protected).unwrap();
    let token = root.path().join("account-token");
    std::fs::write(&token, "account-fixture-token\n").unwrap();
    let (port, server) = listener().await;
    let checks = format!(
        "{} && test \"$(id -un)\" = {} && test \"$(id -u)\" = {expected_uid} && test \"$(id -g)\" = {expected_gid} && test \"$HOME\" = {}",
        account_checks(),
        shell_words::quote(expected_name),
        shell_words::quote(expected_home.to_str().unwrap()),
    );
    let seed_check = if matches!(rule, AccountRule::Create) {
        "cmp /etc/skel/.bashrc \"$HOME/.bashrc\" && cmp /etc/skel/.profile \"$HOME/.profile\" && test \"$(stat -c %u \"$HOME\")\" = \"$(id -u)\" && test \"$(stat -c %u \"$HOME/.bashrc\")\" = \"$(id -u)\" &&"
    } else {
        ""
    };
    let session = h
        .client
        .create_session_with_extras(
            checkout.to_str().unwrap(),
            launch(
                ContainerConfig {
                    engine: ContainerEngine::Podman,
                    image: image.into(),
                    api_proxy_port: port,
                    github_token_file: Some(token.to_str().unwrap().into()),
                },
                format!(
                    "set -eu\n{checks} && {seed_check} printf agent-owned > account-agent && printf 'ACCOUNT READY\\n'; read -r finish"
                ),
            ),
            None,
            120,
            30,
            Default::default(),
        )
        .await
        .unwrap();
    let name = format!("farhelm-{}", session.id);
    let _cleanup = ContainerCleanup(name.clone());
    let (_, mut seen, mut rx) = h
        .client
        .attach_terminal_live(
            &session.id,
            120,
            30,
            TerminalSelector::Agent,
            "container-tab",
        )
        .await
        .unwrap();
    wait_for(&mut rx, &mut seen, "ACCOUNT READY", 60).await;
    assert_eq!(state(&name), "running");
    assert_eq!(
        std::fs::read_to_string(checkout.join("account-agent")).unwrap(),
        "agent-owned"
    );
    assert_host_owned(&checkout.join("account-agent"));
    // This exec has no user, HOME or workdir overrides. It distinguishes container
    // defaults from fixes confined to Farhelm's agent or terminal-tab command.
    let direct = Command::new("podman")
        .args([
            "exec",
            &name,
            "bash",
            "-ec",
            &format!(
                "{checks} && printf exec-owned > account-exec && printf 'DIRECT EXEC READY\\n'"
            ),
        ])
        .output()
        .unwrap();
    assert!(
        direct.status.success(),
        "default exec account failed: {}",
        String::from_utf8_lossy(&direct.stderr)
    );
    assert_eq!(direct.stdout, b"DIRECT EXEC READY\n");
    assert_eq!(
        std::fs::read_to_string(checkout.join("account-exec")).unwrap(),
        "exec-owned"
    );
    assert_host_owned(&checkout.join("account-exec"));
    let mut tab = open_checked_container_tab(&h, &session.id, &checkout).await;
    run_in_shell(
        &h.client,
        tab.channel,
        &mut tab.rx,
        &mut tab.seen,
        &format!("{checks} && printf tab-owned > account-tab && printf 'ACCOUNT%s\\n' TAB"),
        "ACCOUNTTAB",
        20,
    )
    .await;
    assert_eq!(
        std::fs::read_to_string(checkout.join("account-tab")).unwrap(),
        "tab-owned"
    );
    assert_host_owned(&checkout.join("account-tab"));
    assert_eq!(
        std::fs::read_to_string(&protected).unwrap(),
        "checkout content must not be seeded or chowned\n"
    );
    let after = std::fs::metadata(&protected).unwrap();
    assert_eq!(
        (after.uid(), after.gid(), after.mode()),
        (before.uid(), before.gid(), before.mode())
    );
    assert!(
        !checkout.join(".profile").exists(),
        "skel must stay outside the checkout bind"
    );
    h.client.stop_session(&session.id).await.unwrap();
    assert_eq!(state(&name), "exited");
    h.client.delete_session(&session.id).await.unwrap();
    assert_eq!(
        Command::new("podman")
            .args(["container", "exists", &name])
            .status()
            .unwrap()
            .code(),
        Some(1),
    );
    server.abort();
}

/// A missing name and UID create a skel-seeded account even when mounting the
/// checkout first creates its parent home; existing checkout files stay intact.
#[farhelm_testtrace::test]
async fn podman_creates_host_account_with_home_above_checkout() {
    exercise_account_rule(AccountRule::Create).await;
}

/// An existing home owner wins over another existing UID. Daemon's UID competes
/// with Ubuntu's home here, so a UID-first implementation cannot pass.
#[farhelm_testtrace::test]
async fn podman_existing_home_owner_precedes_existing_host_uid() {
    exercise_account_rule(AccountRule::HomeOwner).await;
}

/// Without the requested home, an existing UID keeps its image account and home.
/// Both agent and later tab must inherit that selection without per-exec state.
#[farhelm_testtrace::test]
async fn podman_existing_host_uid_uses_its_image_home() {
    exercise_account_rule(AccountRule::Uid).await;
}

/// Wrapper source is evaluated inside the container, after its HOME and token
/// have replaced the host environment. The real compiler's model and one-time
/// prompt remain literal argv even though both contain executable shell syntax.
/// An owned fake Claude in the shared checkout avoids a vendor dependency.
#[farhelm_testtrace::test]
async fn podman_wrapper_expands_inside_the_container_and_quotes_the_agent_suffix() {
    use std::os::unix::fs::PermissionsExt;
    if !available() {
        return;
    }
    let root = farhelm_teststate::tempdir().unwrap();
    let image = "docker.io/library/ubuntu:24.04";
    let host = HostAccount {
        name: format!("fh{}", &uuid::Uuid::new_v4().simple().to_string()[..20]),
        uid: 1000,
        gid: 1000,
        home: root.path().to_path_buf(),
    };
    assert_image_account_premise(image, &host, AccountRule::Uid);
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            // Podman resolves its rootless image store from the host HOME.
            // Keep it intact; the distinct host token is the positive oracle
            // that expansion happened after entering the container.
            launch_env: vec![("GH_TOKEN".into(), "host-wrapper-token".into())],
            container_host_account: Some(host),
            scopes: Arc::new(farhelm_supervisor::scope::ScopeManager::disabled()),
            ..Default::default()
        },
    )
    .await;
    let checkout = managed_checkout(&h, root.path());
    let executable = checkout.join("claude");
    std::fs::write(
        &executable,
        format!(
            r#"#!/bin/bash
set -eu
{}
test "$WRAPPER_HOME" = "$HOME"
test "$WRAPPER_TOKEN" = "$GH_TOKEN"
test -z "${{FARHELM_SESSION_ID+x}}${{FARHELM_SESSION_TOKEN+x}}${{FARHELM_SUPERVISOR_SOCK+x}}"
printf '%s\0' "$@" > wrapper-argv
printf '%s' "$WRAPPER_HOME" > wrapper-home
printf '%s' "$WRAPPER_TOKEN" > wrapper-token
printf 'CONTAINER WRAPPER READY\n'
read -r finish
"#,
            account_checks()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    let token = root.path().join("wrapper-token");
    std::fs::write(&token, "container-wrapper-token\n").unwrap();
    let (port, server) = listener().await;
    let model = "candidate'42;$GH_TOKEN*";
    let prompt = "one ' \"$HOME\" *; $(touch wrapper-injected)\n{cwd}";
    let selection = LaunchSelection {
        harness: LaunchHarness::Claude,
        model: Some(model.into()),
        effort: None,
        permissions: Some(farhelm_proto::LaunchPermission::Guarded),
        permission_flags: None,
        wrapper_prefix: Some(vec![
            "env".into(),
            "PATH=\"$PWD:$PATH\"".into(),
            "WRAPPER_HOME=\"$HOME\"".into(),
            "WRAPPER_TOKEN=\"$GH_TOKEN\"".into(),
        ]),
        container: Some(ContainerConfig {
            engine: ContainerEngine::Podman,
            image: image.into(),
            api_proxy_port: port,
            github_token_file: Some(token.to_str().unwrap().into()),
        }),
        workspace_trust: None,
    };
    let launch = farhelm_helm::compile_agent_launch(selection.clone()).unwrap();
    let session = h
        .client
        .create_session_with_extras(
            checkout.to_str().unwrap(),
            launch,
            None,
            120,
            30,
            farhelm_helm::CreateExtras {
                prompt: Some(prompt.into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(session.launch.agent_selection(), Some(&selection));
    let name = format!("farhelm-{}", session.id);
    let _cleanup = ContainerCleanup(name.clone());
    let (_, mut seen, mut rx) = h
        .client
        .attach_terminal_live(
            &session.id,
            120,
            30,
            TerminalSelector::Agent,
            "container-wrapper",
        )
        .await
        .unwrap();
    wait_for(&mut rx, &mut seen, "CONTAINER WRAPPER READY", 60).await;
    assert_eq!(state(&name), "running");
    let raw = std::fs::read(checkout.join("wrapper-argv")).unwrap();
    let argv = raw
        .strip_suffix(&[0])
        .expect("the fake agent terminates every argument")
        .split(|byte| *byte == 0)
        .map(|word| std::str::from_utf8(word).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        argv,
        ["--model", model, "--permission-mode", "auto", "--", prompt],
        "container policy must remove hook arguments and preserve the compiled permission/model/prompt words"
    );
    assert_eq!(
        std::fs::read_to_string(checkout.join("wrapper-home")).unwrap(),
        "/home/ubuntu",
        "the raw HOME expression must see the image account"
    );
    assert_eq!(
        std::fs::read_to_string(checkout.join("wrapper-token")).unwrap(),
        "container-wrapper-token",
        "the raw token expression must run after container credentials are installed"
    );
    assert!(!checkout.join("wrapper-injected").exists());
    assert_host_owned(&checkout.join("wrapper-argv"));
    h.client.stop_session(&session.id).await.unwrap();
    assert_eq!(state(&name), "exited");
    h.client.delete_session(&session.id).await.unwrap();
    assert_eq!(
        Command::new("podman")
            .args(["container", "exists", &name])
            .status()
            .unwrap()
            .code(),
        Some(1)
    );
    server.abort();
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
    let image = "docker.io/library/ubuntu:24.04";
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
            image: image.into(),
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
        // A UUID name establishes absence without touching any existing image.
        // Admission must refuse it before publishing a session or creator ticket.
        let missing_image = format!("localhost/missing-{}:latest", uuid::Uuid::new_v4());
        assert_eq!(
            Command::new("podman")
                .args(["image", "exists", &missing_image])
                .status()
                .unwrap()
                .code(),
            Some(1)
        );
        let mut missing = config.clone();
        missing.image = missing_image.clone();
        let refused = h
            .client
            .create_session_with_extras(
                cwd.to_str().unwrap(),
                launch(missing, "exit 0".into()),
                None,
                80,
                24,
                Default::default(),
            )
            .await
            .unwrap_err();
        let detail = format!("{refused:#}");
        assert!(
            detail.contains(&missing_image)
                && detail.contains("not in Podman's local storage on this host"),
            "{detail}"
        );
        assert!(h.client.list_sessions().await.unwrap().sessions.is_empty());
        let quote = |path: &Path| shell_words::quote(path.to_str().unwrap()).into_owned();
        let script = format!(
            r#"
set -eu
{account_checks}
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
            socket = quote(&h.state.path().join("supervisor.sock")),
            account_checks = account_checks()
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
        assert_host_owned(&cwd.join("witness"));
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
    failed_spec_retry_removes_its_predecessor_container(image, &token, port).await;
    server.abort();
    other_server.abort();
}

/// Tabs must inhabit the same container after natural agent exit, tab close
/// and whole-container Stop. Real shell round trips and host file routes also
/// prove that the shared boundary stays the canonical checkout, not its alias
/// or just the working subdirectory. Both scope modes exercise actual Podman.
#[farhelm_testtrace::test]
async fn podman_tabs_and_downloads_share_the_existing_container() {
    use crate::terminal_tabs::{listed_tabs, run_in_shell, write_daemon_script};
    use std::os::unix::fs::PermissionsExt;
    if !available() {
        return;
    }
    let root = farhelm_teststate::tempdir().unwrap();
    let image = "docker.io/library/ubuntu:24.04";
    let token = root.path().join("tab-token");
    let (port, server) = listener().await;
    for scoped in [false, true] {
        std::fs::write(&token, "initial-agent-token\n").unwrap();
        let host_starts = root.path().join(format!("host-starts-{scoped}"));
        let host_shell = root.path().join(format!("host-shell-{scoped}"));
        std::fs::write(
            &host_shell,
            format!(
                "#!/bin/sh\nprintf x >> {}\nexec /bin/bash --noprofile --norc \"$@\"\n",
                shell_words::quote(host_starts.to_str().unwrap())
            ),
        )
        .unwrap();
        std::fs::set_permissions(&host_shell, std::fs::Permissions::from_mode(0o700)).unwrap();
        let seams = SupervisorSeams {
            launch_shell: Some(host_shell.to_str().unwrap().into()),
            scopes: if scoped {
                Arc::new(farhelm_supervisor::scope::ScopeManager::systemd())
            } else {
                Arc::new(farhelm_supervisor::scope::ScopeManager::disabled())
            },
            ..Default::default()
        };
        let scopes = Arc::clone(&seams.scopes);
        let h = harness_with_seams(SupervisorTimeouts::default(), seams).await;
        let checkout_root = farhelm_teststate::tempdir().unwrap();
        let checkout = managed_checkout(&h, checkout_root.path());
        let cwd = checkout.join("nested");
        std::fs::create_dir(&cwd).unwrap();
        let alias = checkout_root.path().join("alias");
        std::os::unix::fs::symlink(&cwd, &alias).unwrap();
        let outside = checkout_root.path().join("outside.txt");
        std::fs::write(&outside, "host-only").unwrap();
        let session = h
            .client
            .create_session_with_extras(
                alias.to_str().unwrap(),
                launch(
                    ContainerConfig {
                        engine: ContainerEngine::Podman,
                        image: image.into(),
                        api_proxy_port: port,
                        github_token_file: Some(token.to_str().unwrap().into()),
                    },
                    "printf 'CONTAINER READY\\n'; read -r finish".into(),
                ),
                None,
                120,
                30,
                Default::default(),
            )
            .await
            .unwrap();
        let name = format!("farhelm-{}", session.id);
        let _cleanup = ContainerCleanup(name.clone());
        // Agent and tabs belong to the same viewer. A different lease would
        // deliberately take over the session and revoke the agent's input.
        let (agent, mut agent_seen, mut agent_rx) = h
            .client
            .attach_terminal_live(
                &session.id,
                120,
                30,
                TerminalSelector::Agent,
                "container-tab",
            )
            .await
            .unwrap();
        wait_for(&mut agent_rx, &mut agent_seen, "CONTAINER READY", 60).await;
        assert_eq!(state(&name), "running");
        let initialization = std::fs::read(&host_starts).unwrap();
        assert!(
            !initialization.is_empty(),
            "the agent used the injected host shell"
        );

        // A hand-retargeted alias must not redirect later container tabs or
        // relative host reads away from the canonical workdir fixed at Create.
        std::fs::remove_file(&alias).unwrap();
        std::os::unix::fs::symlink(root.path(), &alias).unwrap();
        check_container_download_routes(&h, &session.id, &checkout, &cwd, &outside).await;
        std::fs::write(&token, " first-tab-token \n").unwrap();
        let mut first = open_checked_container_tab(&h, &session.id, &cwd).await;
        assert_eq!(
            std::fs::read_to_string(cwd.join("tab-token")).unwrap(),
            "first-tab-token"
        );
        assert_eq!(
            std::fs::read(&host_starts).unwrap(),
            initialization,
            "a container tab must not execute host shell initialization first"
        );
        // Reparent and leave the controlling terminal before closing the tab.
        // A merely backgrounded child remains reachable by the ordinary sweep
        // and would not establish the detached-work survival contract.
        let detached_pid_file = cwd.join("tab-detached.pid");
        write_daemon_script(&cwd, "tab-daemon.sh", &detached_pid_file, false);
        run_in_shell(
            &h.client,
            first.channel,
            &mut first.rx,
            &mut first.seen,
            "bash ./tab-daemon.sh && printf 'TAB%s\\n' DETACHED",
            "TABDETACHED",
            20,
        )
        .await;
        let detached = wait_for_pid_file(&detached_pid_file, 20).await;
        assert!(container_process_alive(&name, first.pid));
        assert!(container_process_alive(&name, detached));
        h.client.close_tab(&session.id, &first.id).await.unwrap();
        wait_for_container_process_exit(&name, first.pid).await;
        assert!(
            container_process_alive(&name, detached),
            "detached work survives closing its shell"
        );
        assert_eq!(state(&name), "running");

        std::fs::write(&token, "second-tab-token\n").unwrap();
        let mut second = open_checked_container_tab(&h, &session.id, &cwd).await;
        assert_eq!(
            std::fs::read_to_string(cwd.join("tab-token")).unwrap(),
            "second-tab-token"
        );
        let persistent = format!("/tmp/{}-tab-witness", session.id);
        run_in_shell(
            &h.client,
            second.channel,
            &mut second.rx,
            &mut second.seen,
            &format!(
                "printf retained > {} && printf 'FILE%s\\n' RETAINED",
                shell_words::quote(&persistent)
            ),
            "FILERETAINED",
            20,
        )
        .await;
        if !scoped {
            h.client.send_input(agent, b"finish\r".to_vec()).await;
            wait_for_exit(&h, &session.id).await;
            run_in_shell(
                &h.client,
                second.channel,
                &mut second.rx,
                &mut second.seen,
                "printf 'AGENT%s\\n' EXITED",
                "AGENTEXITED",
                20,
            )
            .await;
        }
        assert!(container_process_alive(&name, detached));
        h.client.stop_session(&session.id).await.unwrap();
        assert_eq!(
            state(&name),
            "exited",
            "Stop ends tabs and detached work together"
        );
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        loop {
            let tabs = listed_tabs(&h.client, &session.id).await;
            if tabs.is_empty() {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "stopped tabs remained listed: {tabs:?}"
            );
            // sleep-ok: wait for tmux's dead-tab listing after confirmed whole-container Stop.
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        // This in-process harness has no activity ticker. Drive the same dead
        // window cleanup that production's ticker performs before expecting
        // the explicit attachment ending; shell death alone cannot send it.
        h.client.close_tab(&session.id, &second.id).await.unwrap();
        expect_detached(&mut second.rx, 10).await;

        std::fs::write(&token, "after-stop-token\n").unwrap();
        let mut resumed = open_checked_container_tab(&h, &session.id, &cwd).await;
        assert_eq!(
            std::fs::read_to_string(cwd.join("tab-token")).unwrap(),
            "after-stop-token"
        );
        run_in_shell(
            &h.client,
            resumed.channel,
            &mut resumed.rx,
            &mut resumed.seen,
            &format!(
                "test \"$(cat {})\" = retained && printf 'SAME%s\\n' CONTAINER",
                shell_words::quote(&persistent)
            ),
            "SAMECONTAINER",
            20,
        )
        .await;
        assert_eq!(std::fs::read(&host_starts).unwrap(), initialization);
        if scoped && scopes.available().await {
            let output = Command::new("podman")
                .args([
                    "container",
                    "inspect",
                    "--format",
                    "{{.State.ConmonPid}}",
                    &name,
                ])
                .output()
                .unwrap();
            assert!(output.status.success());
            let pid: u32 = String::from_utf8(output.stdout)
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            let environment = std::fs::read(format!("/proc/{pid}/environ")).unwrap();
            assert!(
                !environment
                    .split(|byte| *byte == 0)
                    .any(|entry| entry.starts_with(b"FARHELM_"))
            );
            let cgroup = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap();
            assert!(
                cgroup.contains(&format!("farhelm-{}-container.scope", session.id)),
                "{cgroup}"
            );
        } else if scoped {
            eprintln!("SKIPPED restarted container scope assertion: no usable user manager");
        }
        h.client.close_tab(&session.id, &resumed.id).await.unwrap();
        assert_eq!(
            state(&name),
            "running",
            "closing the restarted tab keeps its container"
        );
        h.client.stop_session(&session.id).await.unwrap();
        assert_eq!(state(&name), "exited");
        assert!(
            Command::new("podman")
                .args(["rm", "-f", &name])
                .status()
                .unwrap()
                .success()
        );
        let error = h.client.open_tab(&session.id).await.unwrap_err();
        assert!(format!("{error:#}").contains("does not exist"), "{error:#}");
        assert_eq!(
            Command::new("podman")
                .args(["container", "exists", &name])
                .status()
                .unwrap()
                .code(),
            Some(1)
        );
        h.client.delete_session(&session.id).await.unwrap();
    }
    server.abort();
}

/// A tab fixture owns a post-replay attachment so each later round trip proves
/// a live shell, rather than rediscovering its own echoed command or scrollback.
struct ContainerTab {
    id: String,
    channel: u32,
    rx: TermStream,
    seen: Vec<u8>,
    pid: u32,
}

/// Assert Bash login, canonical cwd, a usable account and no Farhelm authority
/// before returning a usable tab. The token crosses only the real exec boundary;
/// host-side ownership of its witnesses proves the tab kept the keep-id mapping.
async fn open_checked_container_tab(h: &Harness, id: &str, cwd: &Path) -> ContainerTab {
    use crate::terminal_tabs::{run_in_shell, wait_for_shell};
    let tab = h.client.open_tab(id).await.unwrap();
    let (channel, mut seen, mut rx) = h
        .client
        .attach_terminal_live(
            id,
            120,
            30,
            TerminalSelector::Tab { id: tab.id.clone() },
            "container-tab",
        )
        .await
        .unwrap();
    wait_for_shell(&h.client, channel, &mut rx, &mut seen, "CONTAINER-TAB").await;
    run_in_shell(&h.client, channel, &mut rx, &mut seen, &format!(
        "shopt -q login_shell && {} && test \"$PWD\" = {} && test -z \"${{FARHELM_SESSION_ID+x}}${{FARHELM_SESSION_TOKEN+x}}${{FARHELM_SUPERVISOR_SOCK+x}}${{FARHELM_AGENT_ID+x}}${{FARHELM_TAB_ID+x}}\" && printf %s \"$GH_TOKEN\" > tab-token && printf %s \"$$\" > tab-shell.pid && printf 'TAB%s\\n' CHECKED",
        account_checks(), shell_words::quote(cwd.to_str().unwrap())), "TABCHECKED", 20).await;
    assert_host_owned(&cwd.join("tab-token"));
    assert_host_owned(&cwd.join("tab-shell.pid"));
    let pid = std::fs::read_to_string(cwd.join("tab-shell.pid"))
        .unwrap()
        .parse()
        .unwrap();
    ContainerTab {
        id: tab.id,
        channel,
        rx,
        seen,
        pid,
    }
}

/// Query only a fixture-owned PID inside its still-running container. Podman
/// control failures must not be mistaken for the intended process's absence.
fn container_process_alive(name: &str, pid: u32) -> bool {
    assert_eq!(state(name), "running");
    let output = Command::new("podman")
        .args([
            "exec",
            name,
            "bash",
            "-c",
            "read -r pid comm state rest < /proc/\"$1\"/stat && test \"$state\" != Z",
            "process-probe",
            &pid.to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        matches!(output.status.code(), Some(0 | 1)),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.status.success()
}

/// Closing a tab can return before init reaps its shell. Observe that exact
/// process under a deadline while retaining a live-container premise.
async fn wait_for_container_process_exit(name: &str, pid: u32) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while container_process_alive(name, pid) {
        assert!(
            tokio::time::Instant::now() < deadline,
            "container tab shell {pid} remained alive"
        );
        // sleep-ok: pace bounded Podman process-state probes after tab close.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Stat and transfer independently enforce the whole shared checkout. Include
/// a parent file, relative cwd, ordinary symlink, escapes and a symlink changed
/// after a successful stat so a hover cannot become download authority.
async fn check_container_download_routes(
    h: &Harness,
    id: &str,
    checkout: &Path,
    cwd: &Path,
    outside: &Path,
) {
    use farhelm_proto::DownloadFileStatus;
    let body = vec![0x73; farhelm_proto::UPLOAD_WINDOW_BYTES as usize + 17];
    let shared = checkout.join("file with spaces.txt");
    std::fs::write(&shared, &body).unwrap();
    std::fs::write(cwd.join("relative.txt"), b"relative-body").unwrap();
    std::os::unix::fs::symlink(&shared, cwd.join("inside.txt")).unwrap();
    std::os::unix::fs::symlink(outside, cwd.join("escape.txt")).unwrap();
    for (path, expected) in [
        ("../file with spaces.txt", body.as_slice()),
        ("inside.txt", body.as_slice()),
        ("relative.txt", b"relative-body".as_slice()),
    ] {
        let info = h.client.stat_file(id, path).await.unwrap();
        assert_eq!(info.status, DownloadFileStatus::Ready, "{path}");
        assert_eq!(info.size, Some(expected.len() as u64));
        let mut download = h.client.begin_download(id, path).await.unwrap();
        let mut received = Vec::new();
        tokio::time::timeout(Duration::from_secs(15), async {
            while let Some(chunk) = download.recv().await.unwrap() {
                received.extend_from_slice(&chunk);
            }
        })
        .await
        .expect("container file transfer stalled");
        assert_eq!(received, expected, "{path}");
    }
    for path in [outside.to_str().unwrap(), "../../outside.txt", "escape.txt"] {
        assert_eq!(
            h.client.stat_file(id, path).await.unwrap().status,
            DownloadFileStatus::NotReadable,
            "{path}"
        );
        assert!(h.client.begin_download(id, path).await.is_err(), "{path}");
    }
    std::os::unix::fs::symlink(&shared, cwd.join("changed.txt")).unwrap();
    assert_eq!(
        h.client.stat_file(id, "changed.txt").await.unwrap().status,
        DownloadFileStatus::Ready
    );
    std::fs::remove_file(cwd.join("changed.txt")).unwrap();
    std::os::unix::fs::symlink(outside, cwd.join("changed.txt")).unwrap();
    assert!(h.client.begin_download(id, "changed.txt").await.is_err());
    assert_eq!(std::fs::read_to_string(outside).unwrap(), "host-only");
}

/// A new spec-write failure does not prove that an older attempt started nothing.
/// Seed a real predecessor container behind a pending Create, then refuse the new
/// spec through actual directory permissions. Dropping the row before removing
/// that predecessor would leave an untracked running container.
async fn failed_spec_retry_removes_its_predecessor_container(image: &str, token: &Path, port: u16) {
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
        image: image.into(),
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
            "--pull=never",
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
