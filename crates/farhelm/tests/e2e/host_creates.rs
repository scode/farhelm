//! Creating a session's agent in a terminal host, with the supervisor's
//! internal switch turned on (`SupervisorSeams::new_terminals`): what the
//! host and its child are given, how the create's launch identity holds
//! when the create is interrupted, races Delete or its host fails to start,
//! and that Stop's sweep spares the host.
//!
//! Every supervisor here is the e2e suite's in-process one with the switch
//! on and `host_program` pointing at the built `farhelm`: the supervisor
//! runs inside this test binary, which has no `internal terminal-host`.
//! The host's own fork happens in that `farhelm` process, never in this
//! test process. Hosts live in the harness's state directory, whose drop
//! ends any a test leaves behind; tests that leave a host running end it
//! themselves once done, so that their own runtime's connection does not
//! fight the cleanup.
//!
//! Most tests turn the user manager off (`ScopeManager::disabled`): a
//! launch scope would answer a retried create's evidence before the hosts
//! are asked, and the sweep's marker fallback reaches every process these
//! tests start. The one test about the scope wrapper uses the real manager
//! and says `SKIPPED` without one. The container launch kind is tested in
//! `containerized_sessions.rs`, beside that suite's Podman fixtures.

use crate::harness::*;
use crate::host_terminals::{
    WAIT_SECS, host_process, listed_info, stored_outcome, wait_for_liveness, wait_for_screen,
};
use farhelm_supervisor::hosts::{HostTerminal, Liveness, StopLimits};
use farhelm_supervisor::service::NewTerminals;
use farhelm_supervisor::store::TerminalKind;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

// ---- Fixtures -------------------------------------------------------------------

/// Seams that turn the switch on for an in-process supervisor: new agents
/// start in hosts, started from the built `farhelm`, with no user manager.
pub(crate) fn host_seams() -> SupervisorSeams {
    SupervisorSeams {
        new_terminals: NewTerminals::Host,
        host_program: Some(farhelm_bin().into()),
        scopes: Arc::new(farhelm_supervisor::scope::ScopeManager::disabled()),
        ..SupervisorSeams::default()
    }
}

/// A harness whose creates start their agents in hosts.
async fn host_harness() -> Harness {
    harness_with_seams(SupervisorTimeouts::default(), host_seams()).await
}

/// The published host runtime of `session_id`'s agent.
async fn agent_host(h: &Harness, session_id: &str) -> HostTerminal {
    h.sup
        .agent_host_for_test(session_id)
        .await
        .expect("the session's agent terminal is a host")
}

/// The session's durable row.
async fn stored_row(h: &Harness, session_id: &str) -> Option<StoredSession> {
    SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("open the store")
        .session(session_id)
        .await
        .expect("read the row")
}

/// The sockets in the harness's host socket directory, by name.
fn host_sockets(h: &Harness) -> Vec<OsString> {
    let Ok(entries) = std::fs::read_dir(h.state.path().join("hosts")) else {
        return Vec::new();
    };
    let mut names: Vec<OsString> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            use std::os::unix::fs::FileTypeExt as _;
            entry
                .file_type()
                .ok()?
                .is_socket()
                .then(|| entry.file_name())
        })
        .collect();
    names.sort();
    names
}

/// Every process whose command line names a socket in the harness's host
/// socket directory after `--socket`: the hosts running there, found
/// without connecting to any (which would take one over).
fn host_processes(h: &Harness) -> Vec<u32> {
    host_processes_in(h.state.path())
}

/// [`host_processes`] for the state directory `state`.
fn host_processes_in(state: &Path) -> Vec<u32> {
    let dir = state.join("hosts");
    let dir = dir.as_os_str().as_encoded_bytes();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| {
            std::fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|raw| {
                let args: Vec<&[u8]> = raw.split(|byte| *byte == 0).collect();
                args.windows(2).any(|pair| {
                    pair[0] == b"--socket"
                        && pair[1].starts_with(dir)
                        && pair[1].get(dir.len()) == Some(&b'/')
                })
            })
        })
        .filter(|pid| !process_is_gone(*pid))
        .collect()
}

/// A process's argv, as the kernel reports it.
fn cmdline(pid: u32) -> Vec<String> {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).expect("read the command line");
    raw.split(|byte| *byte == 0)
        .filter(|word| !word.is_empty())
        .map(|word| String::from_utf8_lossy(word).into_owned())
        .collect()
}

/// A process's initial environment, as the kernel reports it.
fn environ(pid: u32) -> BTreeMap<String, String> {
    let raw = std::fs::read(format!("/proc/{pid}/environ")).expect("read the environment");
    parse_env(&raw)
}

/// `NAME=value` entries separated by NUL bytes (`/proc/<pid>/environ`,
/// `env -0`), by name.
fn parse_env(raw: &[u8]) -> BTreeMap<String, String> {
    raw.split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| {
            let entry = String::from_utf8_lossy(entry);
            let (name, value) = entry.split_once('=')?;
            Some((name.to_string(), value.to_string()))
        })
        .collect()
}

/// Wait until `path` exists and is not empty, and return its contents.
async fn wait_for_file(path: &Path, secs: u64) -> Vec<u8> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        if let Ok(contents) = std::fs::read(path)
            && !contents.is_empty()
        {
            return contents;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{} was not written within {secs}s",
            path.display()
        );
        // sleep-ok: polling interval for a file the session's own program writes.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Write `body` as an executable `/bin/sh` script at `path`, and run it
/// once with `--warmup` (which it must answer by exiting 0) before
/// returning, as `hosts::spawn`'s tests do: exec'ing a file just written
/// races `ETXTBSY` against another thread's fork that still holds the
/// descriptor that wrote it.
fn script(path: &Path, body: &str) -> PathBuf {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o755)
        .open(path)
        .expect("write the fixture script");
    write!(
        file,
        "#!/bin/sh\ncase \"$1\" in --warmup) exit 0;; esac\n{body}\n"
    )
    .expect("write the fixture script");
    drop(file);
    for _ in 0..200 {
        match std::process::Command::new(path).arg("--warmup").output() {
            Ok(_) => return path.to_path_buf(),
            Err(error) if error.raw_os_error() == Some(libc::ETXTBSY) => {
                // sleep-ok: retry only ETXTBSY while an inherited writer closes; a successful exec is the fixture boundary.
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(error) => panic!("fixture {} is not runnable: {error}", path.display()),
        }
    }
    panic!("fixture {} stayed busy", path.display());
}

/// End the host at `socket` the way the supervisor ends one it cannot use,
/// for a test that leaves one running, and require that it ended.
async fn end_host_at(socket: &Path) {
    let outcome = farhelm_supervisor::hosts::end_host_at(socket, StopLimits::default()).await;
    assert!(
        matches!(outcome, farhelm_supervisor::hosts::HostOutcome::Stopped(_)),
        "the test's host at {} ended: {outcome:?}",
        socket.display()
    );
}

/// End a published host through its runtime, so that the harness's cleanup
/// does not race the runtime for the host's connection, and require that
/// its process is gone.
pub(crate) async fn end_runtime(host: &HostTerminal, pid: u32) {
    let report = tokio::time::timeout(Duration::from_secs(WAIT_SECS), host.shutdown())
        .await
        .expect("the host's shutdown finishes in time")
        .expect("the runtime was still running the host");
    assert!(report.ended, "the host ended: {report:?}");
    wait_until_pid_gone(pid, WAIT_SECS).await;
}

// ---- Creates --------------------------------------------------------------------

/// A create under the switch starts its agent in a host and records it as a
/// host row, and Stop's sweep ends the agent but never the host.
///
/// Why: this is the switch's whole effect on a create (the plan's PR 16):
/// the row's kind and its `pane` holding the socket's name are what a
/// restarted supervisor will read the session by (D-14), a create that
/// still made a tmux session would leave a husk Delete must find, and a
/// host that Stop's marker sweep could reach would take the final screen
/// with it (D-17). Spec: a create of the fake agent returns; its row is a
/// host row whose `pane` names the one socket in `<state>/hosts`, which the
/// published runtime serves; the private tmux server has no session for
/// it; the host's screen shows the agent's ready marker and the session
/// lists as live. After Stop, the agent's process is gone, the host's
/// process still runs, the runtime reports the child's exit (not the
/// host's end) with the marker still on screen, and the session lists as
/// stopped by the user.
#[farhelm_testtrace::test]
async fn a_create_starts_its_agent_in_a_terminal_host() {
    let h = host_harness().await;
    let (session, _work) = basic_session_mid_launch(&h).await;
    let host = agent_host(&h, &session.id).await;
    wait_for_screen(&host, AGENT_READY_MARKER).await;

    let row = stored_row(&h, &session.id).await.expect("the row exists");
    assert_eq!(row.terminal_kind, TerminalKind::Host);
    assert_eq!(row.outcome, LastOutcome::Running);
    let socket_name = host.socket().file_name().expect("a socket name");
    assert_eq!(OsString::from(&row.pane), socket_name);
    assert_eq!(host_sockets(&h), vec![socket_name.to_os_string()]);
    assert_eq!(
        host.socket(),
        h.state.path().join("hosts").join(socket_name)
    );
    let tmux = tmux_query(
        &h.state.path().join("tmux.sock"),
        &["has-session", "-t", &format!("=fh-{}", session.id)],
    )
    .await;
    assert!(
        !tmux.status.success(),
        "a host agent's create made no tmux session"
    );
    assert!(listed_info(&h, &session.id).await.status.is_live());

    let host_pid = host_process(host.socket()).expect("the host's process serves its socket");
    let child = host.status().child_pid;
    assert!(!process_is_gone(child), "premise: the agent runs");
    h.client.stop_session(&session.id).await.expect("stop");
    wait_until_pid_gone(child, WAIT_SECS).await;
    wait_for_liveness(&host, "the host reporting the agent's exit", |liveness| {
        matches!(liveness, Liveness::Exited { .. })
    })
    .await;
    assert!(
        !process_is_gone(host_pid),
        "Stop's sweep left the host itself running"
    );
    assert!(
        host.screen_tail(4096).contains(AGENT_READY_MARKER),
        "the host keeps the final screen: {:?}",
        host.screen_tail(4096)
    );
    let info = listed_info(&h, &session.id).await;
    assert_eq!(info.annotation.as_deref(), Some("stopped by user"));
    end_runtime(&host, host_pid).await;
}

/// The host's own command line holds only its socket and its environment
/// is empty, while its child, the agent, carries the session's markers and
/// none of tmux's.
///
/// Why: the goal's "the child's environment never travels on the command
/// line" and "markers stay off the host": a host lives for weeks with a
/// command line and environment any same-user process can read, and a host
/// carrying the session's markers would be killed by Stop's sweep. Nothing
/// here needs a user manager, so it runs on every machine. Spec: a created
/// agent's host has the command line `farhelm internal terminal-host
/// --socket <socket>` and an empty environment, and its command line does
/// not hold the session's credential; the host's child has
/// `FARHELM_SESSION_ID` and `FARHELM_AGENT_ID` set to the session and no
/// `TMUX` or `TMUX_PANE`.
#[farhelm_testtrace::test]
async fn the_host_holds_no_credential_or_marker() {
    let h = host_harness().await;
    let (session, _work) = basic_session_mid_launch(&h).await;
    let host = agent_host(&h, &session.id).await;
    wait_for_screen(&host, AGENT_READY_MARKER).await;
    let token = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("open the store")
        .session_token(&session.id)
        .await
        .expect("read the token")
        .expect("the session has a token");

    let host_pid = host_process(host.socket()).expect("the host's process serves its socket");
    assert_eq!(
        cmdline(host_pid),
        vec![
            farhelm_bin().to_string(),
            "internal".to_string(),
            "terminal-host".to_string(),
            "--socket".to_string(),
            host.socket().to_string_lossy().into_owned(),
        ]
    );
    let host_env = environ(host_pid);
    assert!(host_env.is_empty(), "the host's environment: {host_env:?}");
    let raw_cmdline = std::fs::read(format!("/proc/{host_pid}/cmdline")).unwrap();
    assert!(
        !String::from_utf8_lossy(&raw_cmdline).contains(&token),
        "the credential is on the host's command line"
    );

    let child = host.status().child_pid;
    let child_env = environ(child);
    assert_eq!(
        child_env.get("FARHELM_SESSION_ID"),
        Some(&session.id),
        "premise: the child is the agent, which carries the markers"
    );
    assert_eq!(child_env.get("FARHELM_AGENT_ID"), Some(&session.id));
    assert!(!child_env.contains_key("TMUX"), "{child_env:?}");
    assert!(!child_env.contains_key("TMUX_PANE"), "{child_env:?}");
    end_runtime(&host, host_pid).await;
}

/// The agent in a host runs in its launch's scope, and the host in none.
///
/// Why: the scope wrapper must stay exactly where a tmux pane's command has
/// it, around the agent (Stop and Delete kill the scope), and a host must get
/// no scope of its own: it lives in the supervisor's control group, as the
/// tmux server does. Spec: with the real user manager (else `SKIPPED`), a
/// created agent's process is in the launch's scope unit and its host is
/// not; Stop ends the agent and leaves the host running.
#[farhelm_testtrace::test]
async fn the_agent_in_a_host_keeps_its_launch_scope() {
    let Some(scopes) = probed_scope_manager("the_agent_in_a_host_keeps_its_launch_scope").await
    else {
        return;
    };
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            scopes,
            ..host_seams()
        },
    )
    .await;
    let (session, _work) = basic_session_mid_launch(&h).await;
    let host = agent_host(&h, &session.id).await;
    wait_for_screen(&host, AGENT_READY_MARKER).await;
    let host_pid = host_process(host.socket()).expect("the host's process serves its socket");
    let child = host.status().child_pid;
    let unit = farhelm_supervisor::scope::unit_name(&session.id, 0).expect("a scope unit name");
    let in_unit = |pid: u32| {
        std::fs::read_to_string(format!("/proc/{pid}/cgroup"))
            .expect("read the cgroup")
            .lines()
            .any(|line| line.trim_end_matches('/').ends_with(&format!("/{unit}")))
    };
    assert!(in_unit(child), "the agent runs in its launch scope {unit}");
    assert!(!in_unit(host_pid), "the host is outside the launch scope");
    h.client.stop_session(&session.id).await.expect("stop");
    wait_until_pid_gone(child, WAIT_SECS).await;
    assert!(!process_is_gone(host_pid), "the stop left the host running");
    end_runtime(&host, host_pid).await;
}

/// The agent's login shell starts with the environment a tmux pane's
/// command gets, except for the pane's own `TMUX` and `TMUX_PANE`, and
/// startup files see a `TMUX` that holds back a tmux autostart.
///
/// Why: the plan's D-13 promises the child's environment baseline equals a
/// tmux pane's (SPEC.md's "as if the user had SSHed in" contract was
/// delivered by tmux passing the supervisor's environment through), and
/// D-12 keeps the protection a pane gives against a startup file that
/// starts tmux when `TMUX` is unset: without it, every agent's login shell
/// would start the user's own tmux and the agent would never run in the
/// terminal Farhelm drives. Spec: two supervisors, one creating in tmux and
/// one in hosts, run the same command in the same directory through a login
/// shell (a stand-in that records its environment and then runs Bash with
/// a fixture home whose `.bash_profile` records `TMUX` and marks an
/// autostart when `TMUX` is unset). Every recording is written to a
/// temporary name and renamed into place, so it is read only once whole.
/// The recorded environments are equal once `TMUX` and `TMUX_PANE` are
/// left out (`TERM_PROGRAM=tmux` and `TERM_PROGRAM_VERSION` included);
/// tmux set both (premise), and the host only `TMUX`, which is the startup
/// placeholder; in both, the profile saw `TMUX` set, no autostart was
/// marked, and the agent's own environment has neither `TMUX` nor
/// `TMUX_PANE`.
#[farhelm_testtrace::test]
async fn the_login_shell_gets_a_panes_environment_and_startup_files_see_tmux_set() {
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let home = fixture.path().join("home");
    std::fs::create_dir(&home).unwrap();
    std::fs::write(
        home.join(".bash_profile"),
        "printf '%s' \"${TMUX-}\" > \"$HOME/tmux-during-startup-$FIXTURE_KIND\"\n\
         if [ -z \"$TMUX\" ]; then\n  : > \"$HOME/tmux-autostarted-$FIXTURE_KIND\"\nfi\n",
    )
    .unwrap();
    let work = farhelm_teststate::tempdir().expect("workdir");
    let bash = ["/bin/bash", "/usr/bin/bash"]
        .into_iter()
        .find(|path| Path::new(path).is_file())
        .expect("premise: Bash is installed");

    let mut recorded = BTreeMap::new();
    for kind in [NewTerminals::Tmux, NewTerminals::Host] {
        let name = match kind {
            NewTerminals::Tmux => "tmux",
            NewTerminals::Host => "host",
        };
        let dump = fixture.path().join(format!("shell-{name}.env"));
        let shell = script(
            &fixture.path().join(format!("login-shell-{name}")),
            &format!(
                "/usr/bin/env -0 > {partial} && /bin/mv {partial} {dump} || exit 1\n\
                 FIXTURE_KIND={name}\nexport FIXTURE_KIND\nexec {bash} \"$@\"",
                partial = shell_words::quote(&format!("{}.partial", dump.to_string_lossy())),
                dump = shell_words::quote(&dump.to_string_lossy()),
            ),
        );
        let h = harness_with_seams(
            SupervisorTimeouts::default(),
            SupervisorSeams {
                new_terminals: kind,
                launch_shell: Some(shell.to_string_lossy().into_owned()),
                launch_env: vec![("HOME".into(), home.to_string_lossy().into_owned())],
                ..host_seams()
            },
        )
        .await;
        let agent_dump = work.path().join(format!("agent-{name}.env"));
        h.client
            .create_session(
                &work.path().to_string_lossy(),
                &format!(
                    "env -0 > {partial} && mv {partial} {dump} && exec sleep 600",
                    partial =
                        shell_words::quote(&format!("{}.partial", agent_dump.to_string_lossy())),
                    dump = shell_words::quote(&agent_dump.to_string_lossy())
                ),
                None,
                80,
                24,
            )
            .await
            .expect("create");
        let shell_env = parse_env(&wait_for_file(&dump, WAIT_SECS).await);
        let agent_env = parse_env(&wait_for_file(&agent_dump, WAIT_SECS).await);
        assert!(!agent_env.contains_key("TMUX"), "{name}: {agent_env:?}");
        assert!(
            !agent_env.contains_key("TMUX_PANE"),
            "{name}: {agent_env:?}"
        );
        let during_startup =
            std::fs::read_to_string(home.join(format!("tmux-during-startup-{name}")))
                .expect("the profile ran");
        assert!(
            !during_startup.is_empty(),
            "{name}: startup files saw TMUX unset"
        );
        assert!(
            !home.join(format!("tmux-autostarted-{name}")).exists(),
            "{name}: a startup file would have started tmux"
        );
        recorded.insert(name, (shell_env, during_startup));
        // A host is ended through its runtime before the harness goes, so
        // that the runtime does not fight the state directory's cleanup for
        // the host's connection; a tmux session ends with its server.
        if kind == NewTerminals::Host {
            let session = &h.client.list_sessions().await.expect("list").sessions[0];
            let host = agent_host(&h, &session.id).await;
            let pid = host_process(host.socket()).expect("the host's process serves its socket");
            end_runtime(&host, pid).await;
        }
        drop(h);
    }

    let (mut pane, pane_startup) = recorded.remove("tmux").unwrap();
    let (mut host, host_startup) = recorded.remove("host").unwrap();
    assert!(
        pane_startup.contains("tmux.sock"),
        "premise: a pane's startup files see the private server: {pane_startup}"
    );
    assert_eq!(host_startup, "/dev/null/farhelm-terminal-host,0,0");
    assert_eq!(host.remove("TMUX").as_deref(), Some(host_startup.as_str()));
    assert!(pane.remove("TMUX").is_some());
    assert!(
        pane.remove("TMUX_PANE").is_some(),
        "premise: tmux sets TMUX_PANE in a pane"
    );
    assert!(
        !host.contains_key("TMUX_PANE"),
        "a host's child got TMUX_PANE"
    );
    assert_eq!(host.get("TERM").map(String::as_str), Some("xterm-256color"));
    assert_eq!(host.get("TERM_PROGRAM").map(String::as_str), Some("tmux"));
    assert!(
        pane.contains_key("TERM_PROGRAM_VERSION"),
        "premise: tmux sets TERM_PROGRAM_VERSION in a pane"
    );
    let differing: Vec<_> = pane
        .keys()
        .chain(host.keys())
        .filter(|name| pane.get(*name) != host.get(*name))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|name| (name, pane.get(name), host.get(name)))
        .collect();
    assert!(
        differing.is_empty(),
        "a host's child sees another environment than a pane's command (name, pane, host): \
         {differing:#?}"
    );
}

/// A wrapper launch's wrapper runs inside the host, around the agent, as it
/// does in a pane.
///
/// Why: a wrapper prefix is raw shell the launch evaluates before the
/// agent; a host launch that composed the command differently would run
/// the agent without its wrapper. Spec: an agent launch of a fake Claude
/// with a wrapper prefix (a resident `sh -c` script that records that it
/// ran and then runs the agent) starts in a host; the wrapper's record
/// appears and the agent's ready marker is on the host's screen.
#[farhelm_testtrace::test]
async fn a_wrapper_launch_runs_its_wrapper_in_the_host() {
    let fixture = crate::structured_launches::fake_harness();
    let shell = fixture.bash_shell().to_string_lossy().into_owned();
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_env: vec![(
                "HOME".to_string(),
                fixture
                    .login_home_with_fake_path()
                    .to_string_lossy()
                    .into_owned(),
            )],
            launch_shell: Some(shell),
            ..host_seams()
        },
    )
    .await;
    let selection = farhelm_proto::LaunchSelection {
        permission_flags: None,
        harness: farhelm_proto::LaunchHarness::Claude,
        model: None,
        effort: None,
        permissions: Some(farhelm_proto::LaunchPermission::Guarded),
        wrapper_prefix: Some(vec![
            "sh".to_string(),
            "-c".to_string(),
            farhelm_proto::text::shell_quote(
                r#"printf 'wrapped\n' > wrapper-ran || exit 1; "$@"; exit $?"#,
            ),
            "wrapper".to_string(),
        ]),
        container: None,
        workspace_trust: None,
    };
    let launch = farhelm_helm::compile_agent_launch(selection).expect("compile the launch");
    let session = h
        .client
        .create_session_with_extras(
            &fixture.work().to_string_lossy(),
            launch,
            None,
            200,
            40,
            farhelm_helm::CreateExtras::default(),
        )
        .await
        .expect("create a wrapper session");
    let host = agent_host(&h, &session.id).await;
    wait_for_screen(&host, AGENT_READY_MARKER).await;
    assert_eq!(
        wait_for_file(&fixture.work().join("wrapper-ran"), WAIT_SECS).await,
        b"wrapped\n"
    );
    let host_pid = host_process(host.socket()).expect("the host's process serves its socket");
    end_runtime(&host, host_pid).await;
}

// ---- Launch identity -------------------------------------------------------------

/// A command that records each start of the agent in `runs` and then
/// waits: a way to count how many agents a create started.
fn counted_agent(runs: &Path) -> String {
    format!(
        "printf 'run\\n' >> {} && exec sleep 600",
        shell_words::quote(&runs.to_string_lossy())
    )
}

/// A keyed create interrupted after its host started and before the launch
/// was confirmed is retried without starting a second agent.
///
/// Why: the inventory's top risk 4. The interrupted attempt's row records
/// no socket, so a retry that could not find the host by its tag would
/// read the launch as never started and relaunch beside a running agent,
/// the duplicate SPEC.md forbids. Spec: with the create-lifecycle seam
/// failing the first attempt at `DuringLaunch` (after the host started),
/// the create fails; the row is a launching host row with no `pane`, one
/// host runs and its agent has started once. A retry with the same key
/// returns the same session; the reservation is settled as created; still
/// one host socket and one host process, the same one, and the agent has
/// still started once.
#[farhelm_testtrace::test]
async fn an_interrupted_create_is_retried_without_a_second_agent() {
    let interrupted = Arc::new(AtomicBool::new(false));
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            faults: FaultHooks {
                create_crash: Some({
                    let interrupted = Arc::clone(&interrupted);
                    Arc::new(move |stage| {
                        if stage == CreateStage::DuringLaunch
                            && !interrupted.swap(true, Ordering::SeqCst)
                        {
                            anyhow::bail!("interrupted before the launch was confirmed");
                        }
                        Ok(())
                    })
                }),
                ..FaultHooks::default()
            },
            ..host_seams()
        },
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let runs = work.path().join("runs");
    let command = counted_agent(&runs);
    let create = || {
        h.client.create_session_with_key(
            work.path().to_str().unwrap(),
            &command,
            None,
            80,
            24,
            Some("host-intent".to_string()),
        )
    };
    create()
        .await
        .expect_err("the interrupted attempt fails the create");
    assert!(
        interrupted.load(Ordering::SeqCst),
        "premise: the seam fired"
    );
    let rows = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .unwrap()
        .load_all()
        .await
        .unwrap();
    let [row] = rows.as_slice() else {
        panic!("one launching row: {rows:?}");
    };
    assert_eq!(row.outcome, LastOutcome::Launching);
    assert_eq!(row.terminal_kind, TerminalKind::Host);
    assert!(row.pane.is_empty(), "nothing confirmed the launch");
    assert_eq!(wait_for_file(&runs, WAIT_SECS).await, b"run\n");
    let sockets = host_sockets(&h);
    let hosts = host_processes(&h);
    assert_eq!(
        (sockets.len(), hosts.len()),
        (1, 1),
        "premise: one host runs"
    );

    let retried = create().await.expect("the retry answers");
    assert_eq!(retried.id, row.id, "the retry is the same session");
    let reservation = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .unwrap()
        .reservation("host-intent")
        .await
        .unwrap()
        .expect("the reservation");
    assert_eq!(reservation.outcome, ReservationOutcome::Created);
    assert_eq!(host_sockets(&h), sockets, "no second host was started");
    assert_eq!(host_processes(&h), hosts, "the first host still runs");
    assert_eq!(
        std::fs::read(&runs).unwrap(),
        b"run\n",
        "the agent started once"
    );
    end_host_at(&h.state.path().join("hosts").join(&sockets[0])).await;
}

/// A create that loses to Delete between its host's start and its
/// confirmation ends the host as well as the agent.
///
/// Why: once Delete has removed the row, nothing would ever find the host
/// again; a create that only swept the agent's processes would leave a host
/// (and the final screen of an agent nobody can list) running for good.
/// Spec: with the create-lifecycle seam deleting the session's row at
/// `DuringLaunch`, once the agent has started, the create fails as a
/// conflict that says the session was deleted while it was being created;
/// afterwards the agent's process is gone, no host process runs in the
/// state directory, no host socket is left, and no row exists.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn a_create_that_loses_to_delete_ends_its_host() {
    let work = farhelm_teststate::tempdir().expect("workdir");
    let pid_file = work.path().join("agent.pid");
    let deleted = Arc::new(AtomicBool::new(false));
    let state = farhelm_teststate::tempdir().expect("state");
    let db = state.path().join("supervisor.db");
    let seam: CreateCrashSeam = {
        let deleted = Arc::clone(&deleted);
        let pid_file = pid_file.clone();
        Arc::new(move |stage| {
            if stage != CreateStage::DuringLaunch || deleted.load(Ordering::SeqCst) {
                return Ok(());
            }
            let db = db.clone();
            let pid_file = pid_file.clone();
            tokio::task::block_in_place(move || {
                tokio::runtime::Handle::current().block_on(async move {
                    // The agent has started, so the sweep has a tree to end.
                    wait_for_pid_file(&pid_file, WAIT_SECS).await;
                    let store = SessionStore::open(&db, false).await?;
                    for row in store.load_all().await? {
                        store.delete_session_settling_reservations(&row.id).await?;
                    }
                    anyhow::Ok(())
                })
            })?;
            deleted.store(true, Ordering::SeqCst);
            Ok(())
        })
    };
    let sup = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        suite_timeouts(),
        SupervisorSeams {
            faults: FaultHooks {
                create_crash: Some(seam),
                ..FaultHooks::default()
            },
            ..host_seams()
        },
    )
    .await
    .expect("supervisor");
    let _tmux = TmuxServerGuard::new(state.path().join("tmux.sock"));
    let client = connect_client(&sup).await;
    let error = client
        .create_session(
            work.path().to_str().unwrap(),
            &format!(
                "echo $$ > {} && exec sleep 600",
                shell_words::quote(&pid_file.to_string_lossy())
            ),
            None,
            80,
            24,
        )
        .await
        .expect_err("a create whose session was deleted mid-launch fails");
    assert!(
        deleted.load(Ordering::SeqCst),
        "premise: the delete happened"
    );
    assert!(
        format!("{error:#}").contains("deleted while it was being created"),
        "{error:#}"
    );
    let agent = wait_for_pid_file(&pid_file, WAIT_SECS).await;
    wait_until_pid_gone(agent, WAIT_SECS).await;
    let hosts_dir = state.path().join("hosts");
    let left: Vec<_> = std::fs::read_dir(&hosts_dir)
        .map(|entries| entries.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "host sockets left: {left:?}");
    assert!(
        SessionStore::open(&state.path().join("supervisor.db"), false)
            .await
            .unwrap()
            .load_all()
            .await
            .unwrap()
            .is_empty()
    );
    let running: Vec<u32> = std::fs::read_dir("/proc")
        .unwrap()
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| {
            !process_is_gone(*pid)
                && std::fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|raw| {
                    String::from_utf8_lossy(&raw).contains(&*hosts_dir.to_string_lossy())
                })
        })
        .collect();
    assert!(running.is_empty(), "host processes left: {running:?}");
}

/// A host start whose first stage dies after the host reported ready is
/// rolled back with nothing left running.
///
/// Why: the inventory's top risk 5 (create half). A first stage killed by
/// a signal is an ambiguous start (the host may have reported ready just
/// before), and a create that took it for a refusal would drop the row of
/// a running agent; one that kept the row of a host the spawn already
/// ended would show a session that never ran. Spec: with the host program
/// a stand-in that runs the real first stage and then kills itself with
/// SIGKILL once that stage reported ready, the create fails; no row exists;
/// no host socket is left and no host process runs in the state directory;
/// the agent, which may have started, is not running.
#[farhelm_testtrace::test]
async fn a_start_that_dies_after_readiness_leaves_nothing() {
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let program = script(
        &fixture.path().join("farhelm"),
        &format!(
            "{} \"$@\" || exit $?\nkill -9 $$",
            shell_words::quote(farhelm_bin())
        ),
    );
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            host_program: Some(program),
            ..host_seams()
        },
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let pid_file = work.path().join("agent.pid");
    let error = h
        .client
        .create_session(
            work.path().to_str().unwrap(),
            &format!(
                "echo $$ > {} && exec sleep 600",
                shell_words::quote(&pid_file.to_string_lossy())
            ),
            None,
            80,
            24,
        )
        .await
        .expect_err("the start did not finish");
    assert!(format!("{error:#}").contains("signal 9"), "{error:#}");
    assert!(
        SessionStore::open(&h.state.path().join("supervisor.db"), false)
            .await
            .unwrap()
            .load_all()
            .await
            .unwrap()
            .is_empty(),
        "the create was rolled back"
    );
    assert!(host_sockets(&h).is_empty(), "{:?}", host_sockets(&h));
    assert!(host_processes(&h).is_empty(), "{:?}", host_processes(&h));
    if let Ok(pid) = std::fs::read_to_string(&pid_file) {
        let pid: u32 = pid.trim().parse().expect("a pid");
        wait_until_pid_gone(pid, WAIT_SECS).await;
    }
}

/// A host program stand-in whose "host" binds the socket and never
/// answers, as a wedged or incompatible host: the first stage exits 0 once
/// the listener is bound, and the listener writes its pid to `pid_file`.
/// With `signallable`, the listener's command line ends with `--socket
/// <socket>`, as a host's does, so the verified signal may end it;
/// otherwise one more argument follows, so it never will. With
/// `first_stage_dies`, the first stage kills itself with SIGKILL once the
/// listener is bound instead of exiting 0: a start that did not finish,
/// whose socket the spawn's own cleanup cannot end (the listener never
/// answers its `Shutdown`), so the spawn reports that a host may remain.
fn silent_host_program(
    dir: &Path,
    pid_file: &Path,
    signallable: bool,
    first_stage_dies: bool,
) -> PathBuf {
    let tail = if signallable { "" } else { " not-a-host" };
    let end = if first_stage_dies {
        "kill -9 $$"
    } else {
        "exit 0"
    };
    // The listener's arguments after `-c` are `--socket <socket> [tail]`,
    // so its socket is `sys.argv[2]`. It never accepts: a connection waits
    // in its backlog, unanswered, until the client's deadline.
    let listener = "import socket, sys, time\n\
                    s = socket.socket(socket.AF_UNIX)\n\
                    s.bind(sys.argv[2])\n\
                    s.listen(16)\n\
                    time.sleep(3600)";
    script(
        &dir.join("farhelm"),
        &format!(
            "/bin/cat > /dev/null\n\
             /usr/bin/python3 -c {listener} --socket \"$4\"{tail} < /dev/null > /dev/null 2>&1 &\n\
             echo $! > {pid}\n\
             while [ ! -S \"$4\" ]; do /bin/sleep 0.05; done\n\
             {end}",
            listener = shell_words::quote(listener),
            pid = shell_words::quote(&pid_file.to_string_lossy()),
        ),
    )
}

/// A host that started but cannot be used is ended by the verified signal,
/// and the create is rolled back.
///
/// Why: a host the supervisor cannot use (wedged, or speaking another
/// protocol) after a ready start is still this launch's own; a failed
/// create must leave nothing running, and only the verified SIGTERM can
/// end a host the protocol cannot. Spec: with a stand-in host program
/// whose listener never answers and whose command line ends with
/// `--socket <socket>`, the create fails saying it could not connect to
/// the new host; no row exists; the listener's process is gone.
#[farhelm_testtrace::test]
async fn an_unusable_new_host_is_ended_and_the_create_rolled_back() {
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let pid_file = fixture.path().join("listener.pid");
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            host_program: Some(silent_host_program(fixture.path(), &pid_file, true, false)),
            ..host_seams()
        },
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let error = h
        .client
        .create_session(
            work.path().to_str().unwrap(),
            "exec sleep 600",
            None,
            80,
            24,
        )
        .await
        .expect_err("the host cannot be used");
    assert!(
        format!("{error:#}").contains("connecting to the session's new terminal host"),
        "{error:#}"
    );
    let listener = wait_for_pid_file(&pid_file, WAIT_SECS).await;
    wait_until_pid_gone(listener, WAIT_SECS).await;
    assert!(
        SessionStore::open(&h.state.path().join("supervisor.db"), false)
            .await
            .unwrap()
            .load_all()
            .await
            .unwrap()
            .is_empty(),
        "the create was rolled back"
    );
}

/// A host that started but can be neither used nor confirmed ended keeps
/// the create's launching record.
///
/// Why: removing the row is removing the last handle on whatever the
/// launch started; a host that may still run (and that the verified signal
/// refuses to touch, since its command line does not prove it is the host)
/// must stay reachable, as a tmux session that exists despite a failure
/// does. Spec: with a stand-in host program whose listener never answers
/// and whose command line does not end with its socket, the create fails
/// saying a terminal host may still be running at that socket and the
/// session is kept as a launching record; the row exists as a launching
/// host row; the listener still runs (the test then ends it).
#[farhelm_testtrace::test]
async fn an_unusable_host_that_cannot_be_ended_keeps_the_launching_record() {
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let pid_file = fixture.path().join("listener.pid");
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            host_program: Some(silent_host_program(fixture.path(), &pid_file, false, false)),
            ..host_seams()
        },
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let error = h
        .client
        .create_session(
            work.path().to_str().unwrap(),
            "exec sleep 600",
            None,
            80,
            24,
        )
        .await
        .expect_err("the host cannot be used");
    let message = format!("{error:#}");
    assert!(
        message.contains("may still be running at") && message.contains("launching record"),
        "{message}"
    );
    let rows = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .unwrap()
        .load_all()
        .await
        .unwrap();
    let [row] = rows.as_slice() else {
        panic!("the launching row is kept: {rows:?}");
    };
    assert_eq!(row.outcome, LastOutcome::Launching);
    assert_eq!(row.terminal_kind, TerminalKind::Host);
    let listener = wait_for_pid_file(&pid_file, WAIT_SECS).await;
    assert!(!process_is_gone(listener), "the listener was left alone");
    // SAFETY: ending the test's own stand-in listener.
    unsafe { libc::kill(listener as libc::pid_t, libc::SIGKILL) };
    wait_until_pid_gone(listener, WAIT_SECS).await;
}

/// Restart and opening a tab are refused for an agent created in a host,
/// before anything stops.
///
/// Why: Restart of a host agent needs a new host seeded with its history,
/// and a tab beside a host agent needs a tab host; neither exists yet, and
/// a refusal that stopped the agent first would leave the user a stopped
/// session they never asked for. Spec: for a created host agent, a
/// consented Restart fails saying the agent runs in a terminal host and
/// nothing was stopped, opening a tab fails saying tabs cannot be opened
/// beside one yet, and the agent still runs.
#[farhelm_testtrace::test]
async fn restart_and_tabs_stay_refused_for_a_created_host_agent() {
    let h = host_harness().await;
    let (session, _work) = resumable_basic_session_in_host(&h).await;
    let host = agent_host(&h, &session.id).await;
    let child = host.status().child_pid;

    let refused = h
        .client
        .restart_session(&session.id, true)
        .await
        .expect_err("no restart into a new host yet");
    let refused = format!("{refused:#}");
    assert!(refused.contains("nothing was stopped"), "{refused}");
    let refused = h
        .client
        .open_tab(&session.id)
        .await
        .expect_err("no tab beside a host agent yet");
    assert!(
        format!("{refused:#}").contains("cannot open terminal tabs beside one yet"),
        "{refused:#}"
    );
    assert!(!process_is_gone(child), "the agent runs on");
    assert_eq!(stored_outcome(&h, &session.id).await, LastOutcome::Running);
    let host_pid = host_process(host.socket()).expect("the host's process serves its socket");
    end_runtime(&host, host_pid).await;
}

/// [`resumable_basic_session`] for a harness whose creates start in hosts:
/// the same fake agent, waited for on the host's screen rather than in
/// tmux.
async fn resumable_basic_session_in_host(h: &Harness) -> (SessionInfo, farhelm_teststate::TestDir) {
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = create_resumable_session(
        h,
        &work.path().to_string_lossy(),
        &fixture_cmd("fake-agent --script basic"),
        80,
        24,
    )
    .await;
    wait_for_screen(&agent_host(h, &session.id).await, AGENT_READY_MARKER).await;
    (session, work)
}

/// A start whose first stage dies after binding a socket the spawn cannot
/// end is reported as possibly left, and the create ends that host itself
/// before rolling back.
///
/// Why: when the spawn cannot confirm that an unfinished start left
/// nothing, only the supervisor can still end the host (with the verified
/// signal, since the protocol does not answer); rolling the row back
/// without that would drop the last record of a host that may run.
/// `a_start_that_dies_after_readiness_leaves_nothing` covers the other
/// branch, where the spawn's own cleanup ends the host. Spec: with a
/// stand-in first stage that binds a never-answering listener whose command
/// line ends with `--socket <socket>` and then kills itself with SIGKILL,
/// the create fails saying something may be left at the socket; the
/// listener's process is gone, its socket file is removed, and no row
/// exists.
#[farhelm_testtrace::test]
async fn a_host_the_spawn_may_have_left_is_ended_by_the_create() {
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let pid_file = fixture.path().join("listener.pid");
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            host_program: Some(silent_host_program(fixture.path(), &pid_file, true, true)),
            ..host_seams()
        },
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let error = h
        .client
        .create_session(
            work.path().to_str().unwrap(),
            "exec sleep 600",
            None,
            80,
            24,
        )
        .await
        .expect_err("the start did not finish");
    assert!(
        format!("{error:#}").contains("something may be left at"),
        "the spawn reported a host that may remain: {error:#}"
    );
    let listener = wait_for_pid_file(&pid_file, WAIT_SECS).await;
    wait_until_pid_gone(listener, WAIT_SECS).await;
    assert!(host_sockets(&h).is_empty(), "{:?}", host_sockets(&h));
    assert!(
        SessionStore::open(&h.state.path().join("supervisor.db"), false)
            .await
            .unwrap()
            .load_all()
            .await
            .unwrap()
            .is_empty(),
        "the create was rolled back"
    );
}

/// A host program stand-in whose first run is held at the point a crashed
/// supervisor would leave a first stage: it takes the whole spec, then
/// waits for `gate` before running the real first stage on it (as the same
/// process, so the recorded identity stays its own). Later runs start at
/// once.
fn gated_host_program(dir: &Path, gate: &Path) -> PathBuf {
    script(
        &dir.join("farhelm"),
        &format!(
            "spec={spec}.$$\n\
             /bin/cat > \"$spec\"\n\
             if /bin/mkdir {held} 2>/dev/null; then\n\
             \x20 while [ ! -e {gate} ]; do /bin/sleep 0.05; done\n\
             fi\n\
             exec {farhelm} \"$@\" < \"$spec\"",
            spec = shell_words::quote(&dir.join("spec").to_string_lossy()),
            held = shell_words::quote(&dir.join("held").to_string_lossy()),
            gate = shell_words::quote(&gate.to_string_lossy()),
            farhelm = shell_words::quote(farhelm_bin()),
        ),
    )
}

/// The first stage recorded in a launch's host record, by pid.
fn recorded_starter(h: &Harness, session_id: &str) -> u32 {
    let record = std::fs::read_to_string(
        h.state
            .path()
            .join("launch")
            .join(format!("{session_id}.0.host")),
    )
    .expect("the launch's host record");
    record
        .lines()
        .find_map(|line| line.strip_prefix("starter "))
        .and_then(|rest| rest.split(' ').next())
        .and_then(|pid| pid.parse().ok())
        .expect("the record names its first stage")
}

/// Seams for a create whose first host start is interrupted as a crash
/// would interrupt it, with `gate`'s stand-in as the host program.
fn interrupted_seams(
    dir: &Path,
    gate: &Path,
    waiting: Option<Arc<dyn Fn() + Send + Sync>>,
) -> SupervisorSeams {
    let interrupted = Arc::new(AtomicBool::new(false));
    SupervisorSeams {
        host_program: Some(gated_host_program(dir, gate)),
        faults: FaultHooks {
            host_start_interrupt: Some(Arc::new(move || {
                if !interrupted.swap(true, Ordering::SeqCst) {
                    anyhow::bail!("interrupted with the first stage holding its spec");
                }
                Ok(())
            })),
            launch_starter_waiting: waiting,
            ..FaultHooks::default()
        },
        ..host_seams()
    }
}

/// A retried create waits for a first stage that outlived the interrupted
/// attempt, and finds the host it starts instead of starting another.
///
/// Why: the sol review's P1. A supervisor that dies after its host's first
/// stage has the spec leaves a first stage that can still start the host,
/// with the socket not yet bound; a retry that judged the launch absent
/// from the socket alone would start a second host and agent beside it.
/// The launch's host record is what fences that first stage. Spec: with the
/// first start interrupted after its first stage took the spec (and held
/// there), the create fails; the row is a launching host row with no
/// `pane`, its host record names a first stage that runs, and no socket
/// exists. A retry with the same key waits for that first stage (the test
/// releases it from the wait's own hook) and then returns the same session;
/// the reservation is created; exactly one host socket and one host
/// process exist; the agent started once.
#[farhelm_testtrace::test]
async fn a_retry_waits_for_an_interrupted_first_stage_and_finds_its_host() {
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let gate = fixture.path().join("gate");
    let released = gate.clone();
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        interrupted_seams(
            fixture.path(),
            &gate,
            Some(Arc::new(move || {
                std::fs::write(&released, b"").expect("release the first stage");
            })),
        ),
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let runs = work.path().join("runs");
    let command = counted_agent(&runs);
    let create = || {
        h.client.create_session_with_key(
            work.path().to_str().unwrap(),
            &command,
            None,
            80,
            24,
            Some("held-start".to_string()),
        )
    };
    create().await.expect_err("the interrupted attempt fails");
    let rows = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .unwrap()
        .load_all()
        .await
        .unwrap();
    let [row] = rows.as_slice() else {
        panic!("one launching row: {rows:?}");
    };
    assert_eq!(
        (row.outcome.clone(), row.terminal_kind, row.pane.as_str()),
        (LastOutcome::Launching, TerminalKind::Host, "")
    );
    let starter = recorded_starter(&h, &row.id);
    assert!(
        !process_is_gone(starter),
        "premise: the first stage is held"
    );
    assert!(host_sockets(&h).is_empty(), "premise: no socket yet");
    assert!(!gate.exists(), "premise: not released yet");

    let retried = create().await.expect("the retry answers");
    assert!(gate.exists(), "the retry waited for the first stage");
    assert_eq!(retried.id, row.id);
    let reservation = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .unwrap()
        .reservation("held-start")
        .await
        .unwrap()
        .expect("the reservation");
    assert_eq!(reservation.outcome, ReservationOutcome::Created);
    assert_eq!(wait_for_file(&runs, WAIT_SECS).await, b"run\n");
    let sockets = host_sockets(&h);
    assert_eq!(sockets.len(), 1, "{sockets:?}");
    assert_eq!(host_processes(&h).len(), 1, "one host runs");
    assert_eq!(std::fs::read(&runs).unwrap(), b"run\n", "one agent started");
    end_host_at(&h.state.path().join("hosts").join(&sockets[0])).await;
}

/// A retried create ends a first stage that outlived the interrupted
/// attempt and does not finish, and then relaunches with nothing beside it.
///
/// Why: the other half of the fence. A first stage that never finishes
/// must not hold the create forever, and must not be able to start its
/// host after the retry relaunched. Spec: with the first start interrupted
/// after its first stage took the spec (held, and never released), a
/// retry with the same key returns the same session; the recorded first
/// stage is gone (ended by the verified signal); exactly one host socket
/// and one host process exist (the retry's) and the agent started once.
#[farhelm_testtrace::test]
async fn a_retry_ends_an_interrupted_first_stage_that_does_not_finish() {
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let gate = fixture.path().join("gate");
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        interrupted_seams(fixture.path(), &gate, None),
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let runs = work.path().join("runs");
    let command = counted_agent(&runs);
    let create = || {
        h.client.create_session_with_key(
            work.path().to_str().unwrap(),
            &command,
            None,
            80,
            24,
            Some("stuck-start".to_string()),
        )
    };
    create().await.expect_err("the interrupted attempt fails");
    let id = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .unwrap()
        .load_all()
        .await
        .unwrap()
        .pop()
        .expect("the launching row")
        .id;
    let starter = recorded_starter(&h, &id);
    assert!(
        !process_is_gone(starter),
        "premise: the first stage is held"
    );

    let retried = create().await.expect("the retry relaunches");
    assert_eq!(retried.id, id);
    assert!(process_is_gone(starter), "the held first stage was ended");
    assert_eq!(wait_for_file(&runs, WAIT_SECS).await, b"run\n");
    let sockets = host_sockets(&h);
    assert_eq!(sockets.len(), 1, "{sockets:?}");
    assert_eq!(host_processes(&h).len(), 1, "one host runs");
    assert_eq!(std::fs::read(&runs).unwrap(), b"run\n", "one agent started");
    let host = agent_host(&h, &id).await;
    let pid = host_process(host.socket()).expect("the host's process serves its socket");
    end_runtime(&host, pid).await;
}

/// An unconditional Delete of a session whose interrupted create left a
/// first stage holding its spec, not yet bound, fences that first stage
/// before it reports the session deleted.
///
/// Why: a supervisor that dies after its host's first stage has the spec
/// leaves a first stage that can still start the host and agent on its
/// own, with no socket bound yet. A Delete that looked only for sockets
/// would find nothing, delete the session, and let that first stage start
/// an agent nothing lists. The session's launch record names it, and Delete
/// fences it as a retried create does. Spec: with a create interrupted after
/// its first stage took the spec (held, and never released), and the
/// supervisor restarted on the same state (premises: the recorded first
/// stage runs and no socket exists), an unconditional delete succeeds; when
/// it returns the first stage is gone, no host socket exists, no host
/// process runs, and the agent never started.
#[farhelm_testtrace::test]
async fn deleting_a_session_fences_its_unbound_recorded_start() {
    let state = farhelm_teststate::tempdir().expect("state");
    let _tmux = TmuxServerGuard::new(state.path().join("tmux.sock"));
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let gate = fixture.path().join("gate");
    let work = farhelm_teststate::tempdir().expect("workdir");
    let runs = work.path().join("runs");
    let sup = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        suite_timeouts(),
        interrupted_seams(fixture.path(), &gate, None),
    )
    .await
    .expect("supervisor");
    let client = connect_client(&sup).await;
    client
        .create_session_with_key(
            work.path().to_str().unwrap(),
            &counted_agent(&runs),
            None,
            80,
            24,
            Some("deleted-start".to_string()),
        )
        .await
        .expect_err("the interrupted attempt fails");
    let id = SessionStore::open(&state.path().join("supervisor.db"), false)
        .await
        .unwrap()
        .load_all()
        .await
        .unwrap()
        .pop()
        .expect("the launching row")
        .id;
    let record = std::fs::read_to_string(state.path().join("launch").join(format!("{id}.0.host")))
        .expect("the launch's host record");
    let starter: u32 = record
        .lines()
        .find_map(|line| line.strip_prefix("starter "))
        .and_then(|rest| rest.split(' ').next())
        .and_then(|pid| pid.parse().ok())
        .expect("the record names its first stage");
    let sup = crate::create_idempotency::handoff_to_new_supervisor_with_seams(
        state.path(),
        sup,
        client,
        host_seams(),
    )
    .await;
    assert!(
        !process_is_gone(starter),
        "premise: the first stage is held"
    );
    let socket_count = || {
        std::fs::read_dir(state.path().join("hosts"))
            .map(|entries| entries.count())
            .unwrap_or(0)
    };
    assert_eq!(socket_count(), 0, "premise: no socket yet");

    connect_client(&sup)
        .await
        .delete_session(&id)
        .await
        .expect("delete");
    assert!(process_is_gone(starter), "the held first stage was ended");
    assert!(
        host_processes_in(state.path()).is_empty(),
        "nothing that names a host socket runs after the delete"
    );
    assert_eq!(socket_count(), 0, "no host socket exists");
    assert!(!runs.exists(), "the agent never started");
}

/// A create of `session`'s kind interrupted after its launching row, then
/// retried by a supervisor with the switch the other way: the retry's
/// session, and the supervisor that ran it (with its harness pieces).
async fn retry_across_a_switch_change(
    first: NewTerminals,
    then: NewTerminals,
) -> (
    SessionInfo,
    Arc<Supervisor>,
    farhelm_teststate::TestDir,
    TmuxServerGuard,
    farhelm_teststate::TestDir,
) {
    let state = farhelm_teststate::tempdir().expect("state");
    let tmux = TmuxServerGuard::new(state.path().join("tmux.sock"));
    let work = farhelm_teststate::tempdir().expect("workdir");
    let sup = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        suite_timeouts(),
        SupervisorSeams {
            new_terminals: first,
            faults: FaultHooks {
                create_crash: Some(crate::boot_id_durable_outcome::crash_at(
                    CreateStage::AfterRecord,
                )),
                ..FaultHooks::default()
            },
            ..host_seams()
        },
    )
    .await
    .expect("supervisor");
    let client = connect_client(&sup).await;
    let create = |client: Arc<SupervisorClient>| {
        let work = work.path().to_string_lossy().into_owned();
        async move {
            client
                .create_session_with_key(
                    &work,
                    &fixture_cmd("fake-agent --script basic"),
                    None,
                    80,
                    24,
                    Some("switched".to_string()),
                )
                .await
        }
    };
    create(Arc::clone(&client))
        .await
        .expect_err("the first attempt crashes after its row");
    let sup = crate::create_idempotency::handoff_to_new_supervisor_with_seams(
        state.path(),
        sup,
        client,
        SupervisorSeams {
            new_terminals: then,
            ..host_seams()
        },
    )
    .await;
    let client = connect_client(&sup).await;
    let session = create(client).await.expect("the retry launches");
    (session, sup, state, tmux, work)
}

/// A retry launches in the kind of terminal its row records, not the one
/// the switch now selects: a tmux row stays in tmux with the switch on.
///
/// Why: the opus review's finding 2. The evidence that let the retry take
/// the launch over was gathered for the row's kind; a retry in the other
/// kind would be refused by the takeover and replay a create that never
/// launched, a session with nothing behind it. Spec: a create interrupted
/// after its launching row under the switch off, retried after a restart
/// with the switch on, returns the same session running in tmux: its row
/// is a tmux row with a pane, its tmux session exists, and it has no host.
#[farhelm_testtrace::test]
async fn a_retry_keeps_a_tmux_rows_kind_with_the_switch_on() {
    let (session, sup, state, _tmux, _work) =
        retry_across_a_switch_change(NewTerminals::Tmux, NewTerminals::Host).await;
    let row = SessionStore::open(&state.path().join("supervisor.db"), false)
        .await
        .unwrap()
        .session(&session.id)
        .await
        .unwrap()
        .expect("the row");
    assert_eq!(row.terminal_kind, TerminalKind::Tmux);
    assert!(!row.pane.is_empty(), "the launch was confirmed");
    assert!(sup.agent_host_for_test(&session.id).await.is_none());
    wait_for_agent_ready(&state.path().join("tmux.sock"), &session.id).await;
}

/// The reverse: a host row stays in a host with the switch off.
///
/// Why: as above, and the direction a downgrade of the switch takes. Spec:
/// a create interrupted after its launching row under the switch on,
/// retried after a restart with the switch off, returns the same session
/// running in a host: its row is a host row whose `pane` names the host's
/// socket, the host shows the agent's ready marker, and the private tmux
/// server has no session for it.
#[farhelm_testtrace::test]
async fn a_retry_keeps_a_host_rows_kind_with_the_switch_off() {
    let (session, sup, state, _tmux, _work) =
        retry_across_a_switch_change(NewTerminals::Host, NewTerminals::Tmux).await;
    let row = SessionStore::open(&state.path().join("supervisor.db"), false)
        .await
        .unwrap()
        .session(&session.id)
        .await
        .unwrap()
        .expect("the row");
    assert_eq!(row.terminal_kind, TerminalKind::Host);
    let host = sup
        .agent_host_for_test(&session.id)
        .await
        .expect("the retry runs in a host");
    assert_eq!(
        OsString::from(&row.pane),
        host.socket().file_name().unwrap()
    );
    wait_for_screen(&host, AGENT_READY_MARKER).await;
    let tmux = tmux_query(
        &state.path().join("tmux.sock"),
        &["has-session", "-t", &format!("=fh-{}", session.id)],
    )
    .await;
    assert!(!tmux.status.success(), "no tmux session for a host agent");
    let pid = host_process(host.socket()).expect("the host's process serves its socket");
    end_runtime(&host, pid).await;
}

/// An agent launch that the shim cannot execute: a missing program, below
/// any shell, so the shim writes its failure sentinel (as in
/// `launch_sentinel_error_status.rs`).
fn unexecutable_agent(program: &Path) -> farhelm_proto::SessionLaunch {
    farhelm_proto::SessionLaunch::Agent {
        selection: farhelm_proto::LaunchSelection {
            harness: farhelm_proto::LaunchHarness::Goose,
            model: None,
            effort: None,
            permissions: None,
            permission_flags: None,
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        },
        start: vec![
            program.to_string_lossy().into_owned(),
            "{farhelm_args}".to_string(),
        ],
        resume: None,
    }
}

/// A launch whose agent failed to start, interrupted before its host was
/// confirmed, keeps its host record through the error cleanup a later
/// supervisor runs, while the host still runs.
///
/// Why: the sol review's round-2 P1. The host of a failed agent stays up
/// with the final screen, and while the row's `pane` is empty the launch's
/// host record is the only thing naming its socket; an error cleanup that
/// removed it would leave a live host nothing can attribute. Spec: a create
/// interrupted at `DuringLaunch` (its host started, its launch not
/// confirmed) of a missing program leaves the shim's failure sentinel and a
/// running host; after a restart, the replacement supervisor's reload
/// records the error and runs the launch's error cleanup (the sentinel is
/// removed, its premise); the row is an error, and the host record still
/// exists and names a socket a live host process serves.
#[farhelm_testtrace::test]
async fn a_launch_error_keeps_the_host_record_of_a_live_host() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = farhelm_teststate::tempdir().expect("state");
    let _tmux = TmuxServerGuard::new(state.path().join("tmux.sock"));
    let work = farhelm_teststate::tempdir().expect("workdir");
    let sup = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        suite_timeouts(),
        SupervisorSeams {
            faults: FaultHooks {
                create_crash: Some(crate::boot_id_durable_outcome::crash_at(
                    CreateStage::DuringLaunch,
                )),
                ..FaultHooks::default()
            },
            ..host_seams()
        },
    )
    .await
    .expect("supervisor");
    let client = connect_client(&sup).await;
    client
        .create_session_with_extras(
            work.path().to_str().unwrap(),
            unexecutable_agent(&work.path().join("no-such-agent")),
            None,
            80,
            24,
            farhelm_helm::CreateExtras::default(),
        )
        .await
        .expect_err("the create is interrupted before its confirmation");
    let id = SessionStore::open(&state.path().join("supervisor.db"), false)
        .await
        .unwrap()
        .load_all()
        .await
        .unwrap()
        .pop()
        .expect("the launching row")
        .id;
    let launch = state.path().join("launch");
    let record = launch.join(format!("{id}.0.host"));
    let sentinel = launch.join(format!("{id}.0.status"));
    wait_for_file(&sentinel, WAIT_SECS).await;
    let hosts = host_processes_in(state.path());
    assert_eq!(hosts.len(), 1, "premise: the failed agent's host runs");

    let sup = crate::create_idempotency::handoff_to_new_supervisor_with_seams(
        state.path(),
        sup,
        client,
        host_seams(),
    )
    .await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(WAIT_SECS);
    while sentinel.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "premise: the error cleanup never ran"
        );
        sup.reconcile_for_test().await;
        // sleep-ok: polling interval for the supervisor's own cleanup pass.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let row = SessionStore::open(&state.path().join("supervisor.db"), false)
        .await
        .unwrap()
        .session(&id)
        .await
        .unwrap()
        .expect("the row");
    assert!(
        matches!(row.outcome, LastOutcome::Error { .. }),
        "{:?}",
        row.outcome
    );
    let text = std::fs::read_to_string(&record).expect("the host record is kept");
    let name = text
        .lines()
        .find_map(|line| line.strip_prefix("socket "))
        .expect("the record names its socket");
    let socket = state.path().join("hosts").join(name);
    assert_eq!(
        host_process(&socket),
        Some(hosts[0]),
        "the recorded socket is the live host's"
    );
    end_host_at(&socket).await;
    drop(sup);
    drop(slot);
}

/// A host program stand-in whose first run stands for a first stage that
/// forked its second stage and then ended: it takes the spec, starts a
/// stand-in second stage (a process whose command line names the socket,
/// as a forked second stage's does, and that waits for `gate`), and exits.
/// Later runs exec the real first stage.
fn forked_host_program(dir: &Path, gate: &Path) -> PathBuf {
    let stray = "import os, sys, time\n\
                 while not os.path.exists(sys.argv[1]):\n    time.sleep(0.05)";
    script(
        &dir.join("farhelm"),
        &format!(
            "if /bin/mkdir {forked} 2>/dev/null; then\n\
             \x20 /bin/cat > /dev/null\n\
             \x20 /usr/bin/python3 -c {stray} {gate} --socket \"$4\" < /dev/null > /dev/null 2>&1 &\n\
             \x20 exit 0\n\
             fi\n\
             exec {farhelm} \"$@\"",
            forked = shell_words::quote(&dir.join("forked").to_string_lossy()),
            stray = shell_words::quote(stray),
            gate = shell_words::quote(&gate.to_string_lossy()),
            farhelm = shell_words::quote(farhelm_bin()),
        ),
    )
}

/// Seams for a create whose first host start is interrupted (as a crash
/// would interrupt it) with [`forked_host_program`]'s stand-ins as the host
/// program, and `waiting` called when a retry starts waiting for the
/// stand-in second stage.
fn forked_seams(
    dir: &Path,
    gate: &Path,
    waiting: Option<Arc<dyn Fn() + Send + Sync>>,
) -> SupervisorSeams {
    let interrupted = Arc::new(AtomicBool::new(false));
    SupervisorSeams {
        host_program: Some(forked_host_program(dir, gate)),
        faults: FaultHooks {
            host_start_interrupt: Some(Arc::new(move || {
                if !interrupted.swap(true, Ordering::SeqCst) {
                    anyhow::bail!("interrupted with the first stage holding its spec");
                }
                Ok(())
            })),
            launch_stray_waiting: waiting,
            ..FaultHooks::default()
        },
        ..host_seams()
    }
}

/// A retry does not read the launch as absent while a process forked by
/// the interrupted first stage still names the socket, and gives up as
/// unresolved when that process does not go.
///
/// Why: a second stage the first stage forked before it ended still binds
/// the socket and starts the child before it finds its first stage gone,
/// so an empty socket path is not yet absence; a retry that read it as
/// absence could relaunch beside the host that second stage is about to
/// start. The real second stage cannot be held at that point from a test
/// (it is the host crate's own fork, between its `fork` and its `bind`), so
/// a stand-in process with the same command line stands for it. Spec: with the first start interrupted after its first
/// stage took the spec, and that stand-in first stage ended (exited) leaving
/// a stand-in second stage that names the socket and waits, a retry with
/// the same key fails as unresolved, saying a process may yet start the
/// host, and starts nothing (no agent run, no socket); once the stand-in
/// second stage is released and gone, a second retry relaunches the same
/// session, with one host and one agent.
#[farhelm_testtrace::test]
async fn a_retry_waits_out_a_forked_second_stage_before_relaunching() {
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let gate = fixture.path().join("gate");
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        forked_seams(fixture.path(), &gate, None),
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let runs = work.path().join("runs");
    let command = counted_agent(&runs);
    let create = || {
        h.client.create_session_with_key(
            work.path().to_str().unwrap(),
            &command,
            None,
            80,
            24,
            Some("forked-start".to_string()),
        )
    };
    create().await.expect_err("the interrupted attempt fails");
    let id = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .unwrap()
        .load_all()
        .await
        .unwrap()
        .pop()
        .expect("the launching row")
        .id;
    let starter = recorded_starter(&h, &id);
    wait_until_pid_gone(starter, WAIT_SECS).await;
    let strays = host_processes(&h);
    assert_eq!(strays.len(), 1, "premise: the stand-in second stage runs");

    let refused = create()
        .await
        .expect_err("a process may yet start the host");
    assert!(
        format!("{refused:#}").contains("may yet start"),
        "{refused:#}"
    );
    assert!(!runs.exists(), "nothing was relaunched");
    assert!(host_sockets(&h).is_empty(), "{:?}", host_sockets(&h));

    std::fs::write(&gate, b"").expect("release the stand-in second stage");
    wait_until_pid_gone(strays[0], WAIT_SECS).await;
    let retried = create().await.expect("the retry relaunches");
    assert_eq!(retried.id, id);
    assert_eq!(wait_for_file(&runs, WAIT_SECS).await, b"run\n");
    assert_eq!(host_sockets(&h).len(), 1);
    let host = agent_host(&h, &id).await;
    let pid = host_process(host.socket()).expect("the host's process serves its socket");
    end_runtime(&host, pid).await;
}

/// Ending a create's host while its runtime is held by a snapshot encode
/// falls back to ending the host by its socket, within the runtime's bound.
///
/// Why: the sol review's P2. A runtime takes up a shutdown only between its
/// other work, and a snapshot encode at full scrollback holds it for
/// seconds; a cleanup that waited on it without a bound would hold the
/// create (and directory admission with it) for as long as the encode.
/// Spec: with every encode of the new host's runtime held at an encode gate
/// and a checkpoint due after any output, and the create-lifecycle seam
/// deleting the row at `DuringLaunch` once an encode has reached the gate,
/// the create fails as having lost to Delete without saying the host could
/// not be ended, and it does so without waiting for the encode: the gate
/// has let no encode past but by its cancel (it neither was opened nor ran
/// out, which would show a cleanup that waited for the encode, as one
/// without the bound would; stopping the runtime for the fallback cancels
/// the encode it holds); it took at least the runtime's 15 s bound (so the
/// fallback ran);
/// no host process remains in the state directory. Only then is the gate
/// opened, and it never ran out.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn ending_a_host_whose_runtime_is_encoding_falls_back_in_bounded_time() {
    use farhelm_supervisor::hosts::terminal::EncodeGate;
    let gate = EncodeGate::closed();
    let state = farhelm_teststate::tempdir().expect("state");
    let db = state.path().join("supervisor.db");
    let work = farhelm_teststate::tempdir().expect("workdir");
    let encoding = gate.clone();
    let held = gate.clone();
    let seam: CreateCrashSeam = Arc::new(move |stage| {
        if stage != CreateStage::DuringLaunch {
            return Ok(());
        }
        let db = db.clone();
        let encoding = encoding.clone();
        tokio::task::block_in_place(move || {
            anyhow::ensure!(
                encoding.wait_entered(Duration::from_secs(WAIT_SECS)),
                "premise: no encode reached the gate"
            );
            tokio::runtime::Handle::current().block_on(async move {
                let store = SessionStore::open(&db, false).await?;
                for row in store.load_all().await? {
                    store.delete_session_settling_reservations(&row.id).await?;
                }
                anyhow::Ok(())
            })
        })
    });
    let sup = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        suite_timeouts(),
        SupervisorSeams {
            faults: FaultHooks {
                create_crash: Some(seam),
                host_terminal_options: Some(Arc::new(move |options| {
                    options.encode_gate = Some(held.clone());
                    options.checkpoints = farhelm_supervisor::hosts::CheckpointPolicy {
                        min_bytes: 1,
                        max_bytes: 1,
                    };
                })),
                ..FaultHooks::default()
            },
            ..host_seams()
        },
    )
    .await
    .expect("supervisor");
    let _tmux = TmuxServerGuard::new(state.path().join("tmux.sock"));
    let client = connect_client(&sup).await;
    let started = tokio::time::Instant::now();
    let error = client
        .create_session(
            work.path().to_str().unwrap(),
            "printf 'output for a checkpoint\\n' && exec sleep 600",
            None,
            80,
            24,
        )
        .await
        .expect_err("the create lost to Delete");
    let took = started.elapsed();
    // Unopened, the gate lets an encode past only when it runs out, which a
    // cleanup that waited for the encode would show, or when the encode is
    // cancelled, which stopping the runtime for the fallback does.
    assert!(
        !gate.passed() || gate.cancelled(),
        "the cleanup finished only after the held encode went on (expired: {}), after {took:?}",
        gate.expired()
    );
    assert!(
        !gate.expired(),
        "the held encode's gate ran out, after {took:?}"
    );
    let message = format!("{error:#}");
    assert!(
        message.contains("deleted while it was being created"),
        "{message}"
    );
    assert!(
        !message.contains("could not be ended"),
        "the host was ended: {message}"
    );
    assert!(
        took >= Duration::from_secs(15),
        "the runtime's bound did not run out, so the fallback was not shown: {took:?}"
    );
    assert!(
        host_processes_in(state.path()).is_empty(),
        "{:?}",
        host_processes_in(state.path())
    );
    gate.open();
    assert!(!gate.expired(), "the held encode's gate ran out");
}

/// Delete of a session whose agent's runtime is held in a snapshot encode
/// ends the host that runtime adopted by its process, after another host
/// was renamed over its socket before the runtime could notice, and
/// neither shuts down nor signals that other host.
///
/// Why: both reviews' finding on round 3. A runtime held in an encode takes
/// up no shutdown, so Delete's end falls back after the runtime's 15 s
/// bound; a fallback that connected to the socket again would shut down
/// whatever answers there (the renamed-over host, someone else's terminal)
/// and report the session's own host ended while it still runs. The
/// fallback goes by the process the runtime adopted, which a rename cannot
/// change. Spec: with the switch on, every encode of a new host's runtime
/// held at an encode gate and a checkpoint due after any output, a created
/// session whose runtime has an encode at the gate (premise), and another
/// session's host started and its socket renamed over the agent host's
/// while both run (premise), an unconditional delete succeeds; when it
/// returns the agent's host is gone, the other host still runs and answers
/// at that socket with its own tag, and the session is no longer listed;
/// and it does so without waiting for the encode (the gate has let no
/// encode past but by its cancel, which stopping the runtime for the
/// fallback sends; a Delete that waited for the encode would show the gate
/// opened or run out) and only after the runtime's 15 s bound ran out (so
/// the fallback ran). The
/// gate is opened at the end.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn delete_ends_an_encoding_runtimes_host_by_its_process_after_a_rename() {
    use farhelm_supervisor::hosts::terminal::EncodeGate;
    let gate = EncodeGate::closed();
    let held = gate.clone();
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            faults: FaultHooks {
                host_terminal_options: Some(Arc::new(move |options| {
                    options.encode_gate = Some(held.clone());
                    options.checkpoints = farhelm_supervisor::hosts::CheckpointPolicy {
                        min_bytes: 1,
                        max_bytes: 1,
                    };
                })),
                ..FaultHooks::default()
            },
            ..host_seams()
        },
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            work.path().to_str().unwrap(),
            "printf 'output for a checkpoint\\n' && exec sleep 600",
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let encoding = gate.clone();
    assert!(
        tokio::task::block_in_place(move || encoding.wait_entered(Duration::from_secs(WAIT_SECS))),
        "premise: an encode of the agent's runtime is at the gate"
    );
    let socket = agent_host(&h, &session.id).await.socket().to_path_buf();
    let adopted = host_process(&socket).expect("the agent's host serves its socket");
    let _adopted_guard = PidKillGuard::arm(adopted);
    let other_session = uuid::Uuid::new_v4().to_string();
    let other = farhelm_supervisor::hosts::spawn_host(
        Path::new(farhelm_bin()),
        h.state.path(),
        farhelm_supervisor::hosts::ChildSpec {
            argv: vec!["/bin/sh".into(), "-c".into(), "sleep 1000".into()],
            cwd: "/".into(),
            env: vec![("PATH".into(), "/usr/bin:/bin".into())],
            size: terminal_host::protocol::Size { cols: 80, rows: 24 },
            journal_cap: 1024 * 1024,
            initial_snapshot: None,
        },
        &farhelm_supervisor::hosts::HostTag {
            session_id: other_session.clone(),
            created_ns: farhelm_supervisor::hosts::tag::creation_stamp(),
            role: farhelm_supervisor::hosts::HostRole::Agent { generation: 0 },
        },
    )
    .await
    .expect("the other host starts");
    let other_pid = host_process(&other.socket).expect("the other host serves its socket");
    let _other_guard = PidKillGuard::arm(other_pid);
    std::fs::rename(&other.socket, &socket).expect("rename the other host's socket over");
    assert!(
        !process_is_gone(adopted) && !process_is_gone(other_pid),
        "premise: both hosts run"
    );

    let started = tokio::time::Instant::now();
    h.client.delete_session(&session.id).await.expect("delete");
    let took = started.elapsed();
    // Unopened, the gate lets an encode past only when it runs out, which a
    // Delete that waited for the encode would show, or when the encode is
    // cancelled, which stopping the runtime for the fallback does.
    assert!(
        !gate.passed() || gate.cancelled(),
        "the Delete finished only after the held encode went on (expired: {}), after {took:?}",
        gate.expired()
    );
    assert!(
        !gate.expired(),
        "the held encode's gate ran out, after {took:?}"
    );
    assert!(
        took >= Duration::from_secs(15),
        "the runtime's bound did not run out, so the fallback was not shown: {took:?}"
    );
    assert!(
        process_is_gone(adopted),
        "the agent's host is gone when Delete returns"
    );
    assert!(
        !process_is_gone(other_pid),
        "the other host was neither shut down nor signalled"
    );
    let mut connection = farhelm_supervisor::hosts::HostConnection::connect(&socket)
        .await
        .expect("the other host still answers at the socket");
    let info = connection.info().await.expect("it reports its state");
    let tag = farhelm_supervisor::hosts::HostTag::decode(&info.tag).expect("its tag reads");
    assert_eq!(
        tag.session_id, other_session,
        "it is the other session's host"
    );
    connection
        .shutdown()
        .await
        .expect("the other host takes Shutdown");
    wait_until_pid_gone(other_pid, WAIT_SECS).await;
    assert!(
        h.client
            .list_sessions()
            .await
            .expect("list")
            .sessions
            .iter()
            .all(|info| info.id != session.id),
        "the session is deleted"
    );
    gate.open();
}

/// A forked second stage that goes while a retry waits for it lets that
/// same retry relaunch.
///
/// Why: this is what the retry's wait is for. A second stage that finds its
/// first stage gone ends itself shortly after (it ends the child and
/// removes the socket), and a retry that answered unresolved at the first
/// sight of it would fail a create that could simply go on. Spec: with the
/// first start interrupted after its first stage took the spec, and that
/// stand-in first stage ended leaving a stand-in second stage that names
/// the socket, a retry with the same key starts waiting for it (the test
/// releases the stand-in from the wait's own hook, so it ends while that
/// retry waits) and returns the same session, relaunched: the stand-in is
/// gone, one host socket and one host process exist, and the agent started
/// once.
#[farhelm_testtrace::test]
async fn a_forked_second_stage_that_ends_during_the_retry_lets_it_relaunch() {
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let gate = fixture.path().join("gate");
    let released = gate.clone();
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        forked_seams(
            fixture.path(),
            &gate,
            Some(Arc::new(move || {
                std::fs::write(&released, b"").expect("release the stand-in second stage");
            })),
        ),
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let runs = work.path().join("runs");
    let command = counted_agent(&runs);
    let create = || {
        h.client.create_session_with_key(
            work.path().to_str().unwrap(),
            &command,
            None,
            80,
            24,
            Some("forked-then-gone".to_string()),
        )
    };
    create().await.expect_err("the interrupted attempt fails");
    let id = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .unwrap()
        .load_all()
        .await
        .unwrap()
        .pop()
        .expect("the launching row")
        .id;
    wait_until_pid_gone(recorded_starter(&h, &id), WAIT_SECS).await;
    let strays = host_processes(&h);
    assert_eq!(strays.len(), 1, "premise: the stand-in second stage runs");
    assert!(!gate.exists(), "premise: not released yet");

    let retried = create().await.expect("the same retry relaunches");
    assert!(
        gate.exists(),
        "the retry waited for the stand-in second stage"
    );
    assert_eq!(retried.id, id);
    assert!(
        process_is_gone(strays[0]),
        "the stand-in second stage is gone"
    );
    assert_eq!(wait_for_file(&runs, WAIT_SECS).await, b"run\n");
    assert_eq!(host_sockets(&h).len(), 1);
    assert_eq!(host_processes(&h).len(), 1, "one host runs");
    assert_eq!(std::fs::read(&runs).unwrap(), b"run\n", "one agent started");
    let host = agent_host(&h, &id).await;
    let pid = host_process(host.socket()).expect("the host's process serves its socket");
    end_runtime(&host, pid).await;
}
