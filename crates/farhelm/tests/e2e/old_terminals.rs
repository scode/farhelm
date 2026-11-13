//! A supervisor of this release ending the old terminals it finds at startup
//! (`service::core::old_terminals`): the private tmux server an earlier
//! release ran sessions in, and what still ran there.
//!
//! Every test builds that earlier release's layout by hand, with whatever
//! `tmux` the test machine has (and skips, saying so, on a machine with none:
//! `farhelm_teststate::tmux::machine_tmux_or_skip`): a server on
//! `<state dir>/tmux.sock` started with `-S` and `-f <state dir>/tmux.conf` as
//! the supervisor started it, one tmux session `fh-<id>` per Farhelm session, windows marked as the agent's or
//! a tab's (`@farhelm-agent`, `@farhelm-tab`) whose panes carry the
//! `FARHELM_*` markers, and old-kind rows in the store naming those panes.
//! Then a supervisor of this release starts on the same state directory and
//! the test looks at processes, files, rows and the listing's notifications.
//! The window options and markers are spelled out here rather than taken from
//! the supervisor's tmux module, which goes away while these tests stay.

use crate::harness::*;
use farhelm_supervisor::store::{StoredSession, TerminalKind};
use std::path::{Path, PathBuf};

/// The text that marks the notice a session gets when its old terminal is
/// ended, and the one that marks an old-terminal warning.
const ENDED_NOTICE: &str = "release of Farhelm ended";
const OLD_WARNING: &str = "upcoming Farhelm release will forcibly terminate";

/// An earlier release's private tmux server, built by hand on a state
/// directory; stopped by its guard if a test leaves it running.
struct OldServer {
    socket: PathBuf,
    config: PathBuf,
    _guard: TmuxServerGuard,
}

impl OldServer {
    /// The server's socket and config paths on `state` (canonical, as the
    /// supervisor spells them), with its config written. The server itself
    /// starts with the first window.
    fn on(state: &Path) -> Self {
        let state = state.canonicalize().expect("canonical state directory");
        let socket = state.join("tmux.sock");
        let config = state.join("tmux.conf");
        // Dead panes stay, as the supervisor's own config kept them.
        std::fs::write(&config, "set -s exit-empty on\nsetw -g remain-on-exit on\n")
            .expect("write the old config");
        let guard = TmuxServerGuard::new(socket.clone());
        Self {
            socket,
            config,
            _guard: guard,
        }
    }

    /// Run tmux against this server, failing the test unless it succeeds,
    /// and return its output.
    fn tmux(&self, args: &[&str]) -> String {
        let out = std::process::Command::new("tmux")
            .arg("-u")
            .arg("-S")
            .arg(&self.socket)
            .arg("-f")
            .arg(&self.config)
            .args(args)
            .output()
            .expect("run tmux");
        assert!(out.status.success(), "tmux {args:?}: {out:?}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// Open a window of session `session` running `script` with `sh`: the
    /// agent's, or tab `tab`'s, marked and with the markers in its
    /// environment as an earlier release launched it. Returns its pane id.
    fn window(&self, session: &str, tab: Option<&str>, script: &Path) -> String {
        let name = format!("fh-{session}");
        let session_marker = format!("FARHELM_SESSION_ID={session}");
        let kind_marker = match tab {
            Some(tab) => format!("FARHELM_TAB_ID={tab}"),
            None => format!("FARHELM_AGENT_ID={session}"),
        };
        let command = format!("sh {}", shell_words::quote(&script.to_string_lossy()));
        let exists = std::process::Command::new("tmux")
            .arg("-S")
            .arg(&self.socket)
            .args(["has-session", "-t", &name])
            .output()
            .is_ok_and(|out| out.status.success());
        let mut args = if exists {
            vec!["new-window", "-d", "-t", &name]
        } else {
            vec!["new-session", "-d", "-s", &name]
        };
        args.extend([
            "-P",
            "-F",
            "#{pane_id}",
            "-e",
            &session_marker,
            "-e",
            &kind_marker,
            &command,
        ]);
        let pane = self.tmux(&args);
        let (option, value) = match tab {
            Some(tab) => ("@farhelm-tab", tab),
            None => ("@farhelm-agent", session),
        };
        self.tmux(&["set-option", "-w", "-t", &pane, option, value]);
        pane
    }

    /// The server's process id.
    fn pid(&self) -> u32 {
        self.tmux(&["display-message", "-p", "#{pid}"])
            .parse()
            .expect("the server's pid")
    }
}

/// Write `body` to `dir/name` as a shell script and return its path.
fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).expect("write a script");
    path
}

/// Store a row for a new session whose agent ran in an old terminal at
/// `pane`, copied from `template`'s (so its launch and conversation are a
/// real session's, and Restart can resume it), with `outcome` and whether its
/// launch was `scoped`. Returns the new session's id.
async fn plant_old_row(
    state: &Path,
    template: &StoredSession,
    pane: &str,
    outcome: LastOutcome,
    scoped: bool,
) -> String {
    let mut row = template.clone();
    row.id = uuid::Uuid::new_v4().to_string();
    row.title = format!("old {}", &row.id[..8]);
    row.terminal_kind = TerminalKind::Tmux;
    row.terminal_address = pane.to_string();
    row.outcome = outcome;
    row.launch_scoped = scoped;
    row.generation = 0;
    row.launch_boot_ended = false;
    let store = SessionStore::open(&state.join("supervisor.db"), false)
        .await
        .expect("open the store");
    store
        .insert_session(row.clone(), None)
        .await
        .expect("store the old row");
    row.id
}

/// `session_id`'s stored row on `state`.
async fn row_of(state: &Path, session_id: &str) -> StoredSession {
    SessionStore::open(&state.join("supervisor.db"), false)
        .await
        .expect("open the store")
        .session(session_id)
        .await
        .expect("read the row")
        .expect("the row exists")
}

/// Record an unresolved old-terminal warning for `session_id`, as an
/// earlier release's ticker did.
async fn plant_old_warning(state: &Path, session_id: &str) {
    SessionStore::open(&state.join("supervisor.db"), false)
        .await
        .expect("open the store")
        .record_session_notification(
            session_id,
            0,
            "old_terminal",
            &format!("This session must be restarted... Otherwise an {OLD_WARNING} ..."),
            1,
            false,
        )
        .await
        .expect("record the old warning");
}

/// The notifications the listing carries for `session_id`, as `(text,
/// resolved)`.
async fn notifications(h: &Harness, session_id: &str) -> Vec<(String, bool)> {
    h.client
        .list_sessions()
        .await
        .expect("list")
        .sessions
        .into_iter()
        .find(|session| session.id == session_id)
        .unwrap_or_else(|| panic!("session {session_id} is listed"))
        .notifications
        .into_iter()
        .map(|notification| (notification.text, notification.resolved))
        .collect()
}

/// Whether `path` exists, without following a symlink.
fn exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// A supervisor on `state` with `scopes`, as the restarted one.
async fn supervisor_with_scopes(
    state: PathBuf,
    scopes: Arc<farhelm_supervisor::scope::ScopeManager>,
) -> Arc<Supervisor> {
    Supervisor::new_with_seams(
        &state,
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            scopes,
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor")
}

/// An old agent that had exited, leaving a detached job in its launch
/// scope, is ended at the next start: the job, the server and its files go,
/// the row is interrupted with one notice that stays unresolved across a
/// later start, the old warning is resolved, and Restart starts the agent
/// in a new terminal host.
///
/// Why: this is the shape the kept minimum exists for. An old agent's
/// detached job (a dev server, an MCP helper) lives on in its scope after
/// the pane dies, and nothing but this start can reach it once the server
/// is gone; and the user must be told, once and durably, that the session
/// ended and how to get it back. Spec: with a stock tmux server holding a
/// dead agent pane of an old row stored as running and scoped, whose job
/// (environment scrubbed, so only the scope reaches it) runs in
/// `farhelm-<id>-0.scope`, and an unresolved old-terminal warning, a new
/// supervisor ends the server (gone, its socket and config removed), the
/// job is gone, the row reads interrupted with its old kind and pane id,
/// the listing shows the warning resolved and one unresolved ended notice;
/// after another start the notice is still the only one and unresolved;
/// Restart then gives the session a host row whose agent starts.
#[farhelm_testtrace::test]
async fn a_dead_old_agents_scoped_job_is_ended_and_the_session_restarts_in_a_host() {
    let test = "a_dead_old_agents_scoped_job_is_ended_and_the_session_restarts_in_a_host";
    if farhelm_teststate::tmux::machine_tmux_or_skip(test).is_none() {
        return;
    }
    let Some(scopes) = probed_scope_manager(test).await else {
        return;
    };
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            scopes: Arc::clone(&scopes),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let template = create_resumable_session(
        &h,
        &work.path().to_string_lossy(),
        &fixture_cmd("fake-agent --script basic"),
        80,
        24,
    )
    .await;
    let template = row_of(h.state.path(), &template.id).await;
    let server = OldServer::on(h.state.path());

    // The old row first, so the pane's scope can carry its id.
    let id = plant_old_row(h.state.path(), &template, "%0", LastOutcome::Running, true).await;
    let _cleanup = MarkerCleanupGuard::new(id.clone());
    let unit = format!("farhelm-{id}-0.scope");
    let job_pid_file = work.path().join("job.pid");
    let job = script(
        work.path(),
        "job.sh",
        &format!(
            "echo $$ > {}\nexec sleep 120\n",
            shell_words::quote(&job_pid_file.to_string_lossy())
        ),
    );
    // The agent exits only once its job has published its pid: a pane that
    // closes earlier hangs up the job before it has left the pane's session.
    let agent = script(
        work.path(),
        "agent.sh",
        &format!(
            "systemd-run --user --scope --quiet --collect --unit={unit} sh -c {} || exit 1\n\
             tries=0\n\
             while [ ! -s {pid} ] && [ $tries -lt 100 ]; do sleep 0.1; tries=$((tries + 1)); done\n\
             exit 7\n",
            shell_words::quote(&format!(
                "trap '' HUP; setsid env -i PATH=/usr/bin:/bin /bin/sh {} </dev/null >/dev/null 2>&1 &",
                shell_words::quote(&job.to_string_lossy())
            )),
            pid = shell_words::quote(&job_pid_file.to_string_lossy()),
        ),
    );
    let pane = server.window(&id, None, &agent);
    SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("open the store")
        .set_terminal_for_test(&id, TerminalKind::Tmux, &pane)
        .await
        .expect("record the pane");
    plant_old_warning(h.state.path(), &id).await;
    let job_pid = wait_for_pid_file(&job_pid_file, 15).await;
    let _job_cleanup = PidKillGuard::arm(job_pid);
    let server_pid = server.pid();

    let cgroup = std::fs::read_to_string(format!("/proc/{job_pid}/cgroup")).expect("job cgroup");
    assert!(
        cgroup
            .lines()
            .any(|line| line.trim_end_matches('/').ends_with(&format!("/{unit}"))),
        "premise: the job runs in the old launch's scope: {cgroup}"
    );
    assert!(
        !marked_pids(&id).contains(&job_pid),
        "premise: the job carries no marker, so only its scope can reach it"
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while server.tmux(&["list-panes", "-t", &pane, "-F", "#{pane_dead}"]) != "1" {
        assert!(
            tokio::time::Instant::now() < deadline,
            "premise: the old agent's pane dies"
        );
        // sleep-ok: polls a pane's death, which tmux reports to no one here.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let scopes_again = Arc::clone(&scopes);
    let h = h
        .restart_supervisor(|state| supervisor_with_scopes(state, scopes_again))
        .await;

    wait_until_pid_gone(job_pid, 15).await;
    wait_until_pid_gone(server_pid, 15).await;
    assert!(!exists(&server.socket), "the old socket is removed");
    assert!(!exists(&server.config), "the old config is removed");
    let row = row_of(h.state.path(), &id).await;
    assert_eq!(row.outcome, LastOutcome::Interrupted);
    assert_eq!(row.terminal_kind, TerminalKind::Tmux);
    assert_eq!(row.terminal_address, pane, "the recorded pane is kept");
    let check_notices = |listed: Vec<(String, bool)>| {
        let ended: Vec<_> = listed
            .iter()
            .filter(|(text, _)| text.contains(ENDED_NOTICE))
            .collect();
        assert_eq!(ended.len(), 1, "one ended notice: {listed:?}");
        assert!(!ended[0].1, "the ended notice stays unresolved: {listed:?}");
        assert!(
            listed
                .iter()
                .filter(|(text, _)| text.contains(OLD_WARNING))
                .all(|(_, resolved)| *resolved),
            "every old warning is resolved: {listed:?}"
        );
    };
    check_notices(notifications(&h, &id).await);

    let scopes_again = Arc::clone(&scopes);
    let h = h
        .restart_supervisor(|state| supervisor_with_scopes(state, scopes_again))
        .await;
    check_notices(notifications(&h, &id).await);

    h.client
        .restart_session(&id, false)
        .await
        .expect("Restart starts the ended session");
    assert!(
        is_host_row(h.state.path(), &id).await,
        "the session's agent now runs in a terminal host"
    );
    wait_for_agent_ready(&h, &id).await;
    drop(server);
}

/// With no server answering, an old agent's leftovers are found by its
/// row's identity, the files are removed, and an old row that had already
/// ended keeps its outcome with no notice.
///
/// Why: the server is often gone before the upgrade (a reboot of tmux, a
/// crash, an earlier tmux), and a daemon the agent left must still not
/// outlive its session's end; while a session whose agent had already
/// exited lost nothing it still had, so it is told nothing. Spec: with the
/// socket left by a SIGKILLed server (refusing connections), its config and
/// a staged config write, an old row stored as running (unscoped) whose
/// daemon carries its agent and session markers and ignores SIGHUP, and an
/// old row that exited with code 0 and has an old warning, a new supervisor
/// removes the three files, the daemon is gone, the running row reads
/// interrupted with one unresolved ended notice, and the exited row still
/// reads exited with code 0, with its warning resolved and no notice.
#[farhelm_testtrace::test]
async fn with_no_server_the_old_rows_leftovers_and_files_are_cleaned_up() {
    let test = "with_no_server_the_old_rows_leftovers_and_files_are_cleaned_up";
    if farhelm_teststate::tmux::machine_tmux_or_skip(test).is_none() {
        return;
    }
    let h = harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (template, _template_work) = basic_session(&h).await;
    let template = row_of(h.state.path(), &template.id).await;
    let server = OldServer::on(h.state.path());

    let running = plant_old_row(h.state.path(), &template, "%0", LastOutcome::Running, false).await;
    let _cleanup = MarkerCleanupGuard::new(running.clone());
    let exited = LastOutcome::Exited {
        exit_code: Some(0),
        annotation: None,
    };
    let ended = plant_old_row(h.state.path(), &template, "%1", exited.clone(), false).await;
    plant_old_warning(h.state.path(), &ended).await;

    let pane = script(work.path(), "pane.sh", "exec sleep 120\n");
    server.window(&running, None, &pane);
    let server_pid = server.pid();
    // SAFETY: the server is this test's own, by the pid it reported.
    unsafe { libc::kill(server_pid as libc::pid_t, libc::SIGKILL) };
    wait_until_pid_gone(server_pid, 10).await;
    let staged = server.socket.with_file_name(".tmux.conf.tmp-0123abcd");
    std::fs::write(&staged, "partial").expect("stage a config write");
    assert!(
        std::os::unix::net::UnixStream::connect(&server.socket)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::ConnectionRefused),
        "premise: the killed server left a socket that refuses connections"
    );

    let daemon_pid_file = work.path().join("daemon.pid");
    let mut daemon = std::process::Command::new("setsid")
        .args(["sh", "-c"])
        .arg(format!(
            "trap '' HUP; echo $$ > {}; exec sleep 120",
            shell_words::quote(&daemon_pid_file.to_string_lossy())
        ))
        .env("FARHELM_SESSION_ID", &running)
        .env("FARHELM_AGENT_ID", &running)
        .env_remove("FARHELM_TAB_ID")
        .stdin(std::process::Stdio::null())
        .spawn()
        .expect("start the old agent's daemon");
    let daemon_pid = wait_for_pid_file(&daemon_pid_file, 10).await;
    assert!(
        marked_pids(&running).contains(&daemon_pid),
        "premise: the daemon carries the old agent's markers"
    );

    let h = h
        .restart_supervisor(|state| async move { supervisor_on(&state).await.expect("supervisor") })
        .await;

    wait_until_pid_gone(daemon_pid, 15).await;
    let _ = daemon.wait();
    for path in [&server.socket, &server.config, &staged] {
        assert!(!exists(path), "{} is removed", path.display());
    }
    assert_eq!(
        row_of(h.state.path(), &running).await.outcome,
        LastOutcome::Interrupted
    );
    let listed = notifications(&h, &running).await;
    assert_eq!(
        listed
            .iter()
            .filter(|(text, resolved)| text.contains(ENDED_NOTICE) && !resolved)
            .count(),
        1,
        "{listed:?}"
    );
    assert_eq!(row_of(h.state.path(), &ended).await.outcome, exited);
    let listed = notifications(&h, &ended).await;
    assert!(
        !listed.iter().any(|(text, _)| text.contains(ENDED_NOTICE)),
        "an old agent that had ended gets no notice: {listed:?}"
    );
    assert!(
        listed
            .iter()
            .any(|(text, resolved)| text.contains(OLD_WARNING) && *resolved),
        "its old warning is resolved: {listed:?}"
    );
    drop(server);
}

/// An old tab of a session whose agent and other tab already run in
/// terminal hosts is ended, and those hosts are not.
///
/// Why: a session moved to a host keeps its old tabs in the old server
/// until they are closed, and its host terminals carry the same session
/// marker, so the cleanup must reach the old tab by its own id and never
/// sweep the session as a whole; the session lost no agent, so it gets no
/// notice. Spec: with a host session that has a host tab, and a stock tmux
/// server whose window for that session is an old tab whose shell started a
/// job that ignores SIGHUP (so the server's end alone does not end it), a
/// new supervisor ends the server and the job, while the host agent and the
/// host tab's shell still run, the row is still a running host row, and the
/// session has no ended notice. With a user manager, the host tab runs in
/// its own tab scope (asserted), so the sweep's listing of the session's tab
/// scopes finds it and must skip it by its host launch record; without one
/// that skip is not exercised, which the test says.
#[farhelm_testtrace::test]
async fn a_mixed_sessions_old_tab_is_ended_and_its_host_terminals_are_not() {
    let test = "a_mixed_sessions_old_tab_is_ended_and_its_host_terminals_are_not";
    if farhelm_teststate::tmux::machine_tmux_or_skip(test).is_none() {
        return;
    }
    let scopes = Arc::new(farhelm_supervisor::scope::ScopeManager::systemd());
    let scoped = scopes.available().await;
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            scopes: Arc::clone(&scopes),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (session, _session_work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let host_tab = h
        .client
        .open_tab(&session.id)
        .await
        .expect("open a host tab");
    let agent = agent_pid(&h, &session.id).await;
    let host_tab_shell = crate::terminal_tabs::tab_shell_pid(&h, &session.id, &host_tab.id).await;
    if scoped {
        let unit = farhelm_supervisor::scope::tab_unit_name(&session.id, &host_tab.id)
            .expect("the ids name a tab scope");
        let cgroup = std::fs::read_to_string(format!("/proc/{host_tab_shell}/cgroup"))
            .expect("the host tab shell's cgroup");
        assert!(
            cgroup
                .lines()
                .any(|line| line.trim_end_matches('/').ends_with(&format!("/{unit}"))),
            "premise: the host tab runs in its tab scope, which the sweep's listing finds: {cgroup}"
        );
    } else {
        eprintln!(
            "NOTE a_mixed_sessions_old_tab_is_ended_and_its_host_terminals_are_not: no usable \
             user manager, so the host tab is unscoped and the host-record skip is not exercised"
        );
    }
    let server = OldServer::on(h.state.path());

    let old_tab = uuid::Uuid::new_v4().to_string();
    let job_pid_file = work.path().join("tab-job.pid");
    let job = script(
        work.path(),
        "tab-job.sh",
        &format!(
            "trap '' HUP\necho $$ > {}\nexec sleep 120\n",
            shell_words::quote(&job_pid_file.to_string_lossy())
        ),
    );
    let tab = script(
        work.path(),
        "tab.sh",
        &format!(
            "sh {} &\nwait\n",
            shell_words::quote(&job.to_string_lossy())
        ),
    );
    server.window(&session.id, Some(&old_tab), &tab);
    let job_pid = wait_for_pid_file(&job_pid_file, 15).await;
    let _job_cleanup = PidKillGuard::arm(job_pid);
    let server_pid = server.pid();
    let status = std::fs::read_to_string(format!("/proc/{job_pid}/status")).expect("job status");
    let ignored = status
        .lines()
        .find_map(|line| line.strip_prefix("SigIgn:"))
        .map(|mask| u64::from_str_radix(mask.trim(), 16).expect("mask"))
        .expect("the job's ignored-signal mask");
    assert_ne!(
        ignored & (1 << (libc::SIGHUP - 1)),
        0,
        "premise: the old tab's job ignores SIGHUP, so the server's end alone does not end it"
    );

    let h = h
        .restart_supervisor(|state| supervisor_with_scopes(state, scopes))
        .await;

    wait_until_pid_gone(server_pid, 15).await;
    wait_until_pid_gone(job_pid, 15).await;
    assert!(!process_is_gone(agent), "the host agent still runs");
    assert!(
        !process_is_gone(host_tab_shell),
        "the host tab's shell still runs"
    );
    let row = row_of(h.state.path(), &session.id).await;
    assert_eq!(row.terminal_kind, TerminalKind::Host);
    assert_eq!(row.outcome, LastOutcome::Running);
    let listed = notifications(&h, &session.id).await;
    assert!(
        !listed.iter().any(|(text, _)| text.contains(ENDED_NOTICE)),
        "a session whose agent was not in an old terminal gets no notice: {listed:?}"
    );
    drop(server);
}
