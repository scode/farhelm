//! Scope attribution and signal ordering through the supervisor's public
//! lifecycle actions, with stand-ins for conmon and Podman.

use crate::harness::*;
use crate::terminal_tabs::wait_for_shell;
use farhelm_proto::text::shell_quote;
use farhelm_supervisor::scope::{ScopeManager, tab_unit_name, unit_name};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;

/// The same private directory supplies the fake engine, its call log, and
/// each monitor's readiness record. The fake only accepts a stop while the
/// named monitor is still alive, so observing a call proves its ordering.
struct PodmanFixture {
    work: farhelm_teststate::TestDir,
}

impl PodmanFixture {
    fn new(fail_stop: bool) -> Self {
        let work = farhelm_teststate::tempdir().unwrap();
        symlink(fixtures_bin(), work.path().join("conmon")).unwrap();
        let engine = work.path().join("podman");
        let dir = shell_quote(work.path().to_str().unwrap());
        let stop_code = if fail_stop { 125 } else { 0 };
        std::fs::write(
            &engine,
            format!(
                "#!/bin/sh\n\
                 printf '%s\\n' \"$*\" >> {dir}/podman-calls\n\
                 [ \"$1\" = --remote=false ] || exit 2\n\
                 shift\n\
                 if [ \"$2 $3\" = 'container inspect' ]; then echo running; exit 0; fi\n\
                 [ \"$2 $3 $4 $5\" = 'stop --time 5 --' ] || exit 2\n\
                 pid=$(cat {dir}/\"$6\".pid) || exit 2\n\
                 case $(ps -o stat= -p \"$pid\") in ''|Z*) exit 2;; esac\n\
                 printf '%s\\n' \"$6\" >> {dir}/live-stops\n\
                 exit {stop_code}\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(engine, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self { work }
    }

    /// Use the exact scope manager that passed the production probe, with
    /// this fixture's engine path as the sole difference from a real spawn.
    async fn harness(&self, test: &str) -> Option<Harness> {
        let scopes = ScopeManager::systemd().with_podman(self.work.path().join("podman"));
        let scopes = probe_scope_manager(test, scopes).await?;
        Some(
            harness_with_seams(
                SupervisorTimeouts::default(),
                SupervisorSeams {
                    scopes,
                    ..SupervisorSeams::default()
                },
            )
            .await,
        )
    }

    /// The real parser sees the main or exec form of conmon's command line.
    /// The engine's cleanup arguments include a non-default storage root.
    fn monitor_command(&self, id: &str, exec: bool) -> String {
        let dir = shell_quote(self.work.path().to_str().unwrap());
        let exec_flag = if exec { " --exec" } else { "" };
        format!(
            "{dir}/conmon -c {id} -n demo{exec_flag} --fixture-ready {dir}/{id}.pid \
             --exit-command-arg --root --exit-command-arg {dir}/storage \
             --exit-command-arg container --exit-command-arg cleanup --exit-command-arg {id}"
        )
    }

    /// Refuse a fallback unscoped launch as a setup failure. Arm cleanup
    /// before asserting so a failing premise still reaps the fixture.
    async fn monitor_in_scope(&self, id: &str, unit: &str) -> (u32, PidKillGuard) {
        let pid = wait_for_pid_file(&self.work.path().join(format!("{id}.pid")), 15).await;
        let guard = PidKillGuard::arm(pid);
        let membership = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap();
        assert!(!process_is_gone(pid), "monitor {pid} must be alive");
        assert!(
            membership
                .lines()
                .any(|line| line.starts_with("0::") && line.ends_with(&format!("/{unit}"))),
            "monitor {pid} must belong to {unit}, got {membership:?}"
        );
        (pid, guard)
    }

    /// Check the complete command and the liveness witness independently.
    /// Call order between monitors inside one operation is unconstrained.
    fn assert_stops(&self, ids: &[&str]) {
        let mut expected = ids.to_vec();
        expected.sort_unstable();
        let live = self.read_calls("live-stops");
        let mut actual: Vec<_> = live.lines().collect();
        actual.sort_unstable();
        assert_eq!(actual, expected, "Podman must run before the monitor dies");
        let calls = self.read_calls("podman-calls");
        let mut actual: Vec<_> = calls.lines().collect();
        actual.sort_unstable();
        let expected: Vec<_> = expected
            .into_iter()
            .map(|id| {
                format!(
                    "--remote=false --root={}/storage stop --time 5 -- {id}",
                    self.work.path().display()
                )
            })
            .collect();
        assert_eq!(actual, expected);
    }

    fn read_calls(&self, name: &str) -> String {
        std::fs::read_to_string(self.work.path().join(name)).unwrap()
    }
}

/// Use an actual tab shell so the monitor joins the scope the production
/// open path created. PID readiness is checked by the caller.
async fn start_in_tab(h: &Harness, session: &str, tab: &str, command: &str) {
    let (channel, mut seen, mut rx) = h
        .client
        .attach_terminal_live(
            session,
            80,
            24,
            TerminalSelector::Tab { id: tab.into() },
            "podman-tab",
        )
        .await
        .expect("attach tab");
    wait_for_shell(&h.client, channel, &mut rx, &mut seen, "PODMAN").await;
    h.client
        .send_keys(channel, format!("{command}\r").into_bytes())
        .await;
}

/// Keep an agent scope populated with a monitor through the normal launch
/// path. The fake engine records the monitor's state at the time of Stop.
async fn launch_monitor(h: &Harness, work: &Path, command: &str) -> SessionInfo {
    h.client
        .create_session(work.to_str().unwrap(), command, None, 80, 24)
        .await
        .expect("launch monitor")
}

/// Tab close may stop only its tab's container, including when that tab has
/// a Podman exec monitor. Stop reaps the agent's container. Delete collects
/// monitors across the agent and tab scopes before reaping either scope.
#[farhelm_testtrace::test]
async fn lifecycle_stops_containers_in_the_selected_scopes_before_signalling() {
    let fixture = PodmanFixture::new(false);
    let Some(h) = fixture
        .harness("lifecycle_stops_containers_in_the_selected_scopes_before_signalling")
        .await
    else {
        return;
    };
    let work = fixture.work.path();
    let [agent, tab_main, tab_exec, delete_agent, delete_tab] =
        ['a', 'b', 'c', 'd', 'e'].map(|letter| letter.to_string().repeat(64));

    let session = launch_monitor(&h, work, &fixture.monitor_command(&agent, false)).await;
    let (agent_pid, _agent_guard) = fixture
        .monitor_in_scope(&agent, &unit_name(&session.id, 0).unwrap())
        .await;
    let tab = h.client.open_tab(&session.id).await.unwrap();
    let unit = tab_unit_name(&session.id, &tab.id).unwrap();
    let command = format!(
        "{} & {}",
        fixture.monitor_command(&tab_main, false),
        fixture.monitor_command(&tab_exec, true)
    );
    start_in_tab(&h, &session.id, &tab.id, &command).await;
    let (tab_pid, _tab_guard) = fixture.monitor_in_scope(&tab_main, &unit).await;
    let (exec_pid, _exec_guard) = fixture.monitor_in_scope(&tab_exec, &unit).await;

    h.client.close_tab(&session.id, &tab.id).await.unwrap();
    fixture.assert_stops(&[&tab_main]);
    assert!(
        !process_is_gone(agent_pid),
        "tab close must leave the agent"
    );
    wait_until_pid_gone(tab_pid, 10).await;
    wait_until_pid_gone(exec_pid, 10).await;

    h.client.stop_session(&session.id).await.unwrap();
    fixture.assert_stops(&[&tab_main, &agent]);
    wait_until_pid_gone(agent_pid, 10).await;

    let session = launch_monitor(&h, work, &fixture.monitor_command(&delete_agent, false)).await;
    let (agent_pid, _delete_agent_guard) = fixture
        .monitor_in_scope(&delete_agent, &unit_name(&session.id, 0).unwrap())
        .await;
    let tab = h.client.open_tab(&session.id).await.unwrap();
    start_in_tab(
        &h,
        &session.id,
        &tab.id,
        &fixture.monitor_command(&delete_tab, false),
    )
    .await;
    let (tab_pid, _delete_tab_guard) = fixture
        .monitor_in_scope(&delete_tab, &tab_unit_name(&session.id, &tab.id).unwrap())
        .await;
    h.client.delete_session(&session.id).await.unwrap();
    fixture.assert_stops(&[&tab_main, &agent, &delete_agent, &delete_tab]);
    wait_until_pid_gone(agent_pid, 10).await;
    wait_until_pid_gone(tab_pid, 10).await;
}

/// Read what the app sees, bypassing capture warnings from unrelated features
/// that may appear in the same session's ordinary notification history.
async fn podman_notices(h: &Harness, session_id: &str) -> Vec<farhelm_proto::SessionNotification> {
    h.client
        .list_sessions()
        .await
        .unwrap()
        .sessions
        .into_iter()
        .find(|session| session.id == session_id)
        .expect("the session is listed")
        .notifications
        .into_iter()
        .filter(|notice| {
            !notice.resolved
                && notice
                    .text
                    .contains("could not confirm that Podman stopped")
        })
        .collect()
}

/// A failed engine command leaves the process sweep in charge. Stop and
/// tab close must surface that advisory separately from action success.
/// A second tab's failure refreshes the notice in the same launch so a
/// mark on the earlier report cannot hide the next container.
#[farhelm_testtrace::test]
async fn failed_podman_stop_warns_without_failing_stop_or_tab_close() {
    let fixture = PodmanFixture::new(true);
    let Some(h) = fixture
        .harness("failed_podman_stop_warns_without_failing_stop_or_tab_close")
        .await
    else {
        return;
    };
    let id = "a".repeat(64);
    let session = launch_monitor(
        &h,
        fixture.work.path(),
        &fixture.monitor_command(&id, false),
    )
    .await;
    let (pid, _guard) = fixture
        .monitor_in_scope(&id, &unit_name(&session.id, 0).unwrap())
        .await;
    h.client.stop_session(&session.id).await.unwrap();
    wait_until_pid_gone(pid, 10).await;
    let notices = podman_notices(&h, &session.id).await;
    assert_eq!(notices.len(), 1);
    assert!(notices[0].text.contains(&id) && notices[0].text.contains("podman rm -f"));

    let (session, _work) = basic_session(&h).await;
    let mut previous_seq = 0;
    for id in ['b', 'c'].map(|letter| letter.to_string().repeat(64)) {
        let tab = h.client.open_tab(&session.id).await.unwrap();
        start_in_tab(
            &h,
            &session.id,
            &tab.id,
            &fixture.monitor_command(&id, false),
        )
        .await;
        let (pid, _guard) = fixture
            .monitor_in_scope(&id, &tab_unit_name(&session.id, &tab.id).unwrap())
            .await;
        h.client.close_tab(&session.id, &tab.id).await.unwrap();
        wait_until_pid_gone(pid, 10).await;
        let notices = podman_notices(&h, &session.id).await;
        assert_eq!(notices.len(), 1, "both closes belong to the same launch");
        assert!(notices[0].text.contains(&id));
        assert!(notices[0].seq > previous_seq);
        previous_seq = notices[0].seq;
    }
}

/// Restart publishes the prior launch's warning alongside the new agent.
/// Delete removes the notification history, so a later failure is sent
/// with Delete's successful reply instead.
#[farhelm_testtrace::test]
async fn failed_podman_stop_warns_on_restart_and_in_the_delete_reply() {
    let fixture = PodmanFixture::new(true);
    let Some(h) = fixture
        .harness("failed_podman_stop_warns_on_restart_and_in_the_delete_reply")
        .await
    else {
        return;
    };
    let id = "a".repeat(64);
    let work = fixture.work.path();
    let script = format!(
        "if [ ! -s {} ]; then {} & fi\nexec {}\n",
        shell_quote(work.join(format!("{id}.pid")).to_str().unwrap()),
        fixture.monitor_command(&id, false),
        fixture_cmd("fake-agent --script basic"),
    );
    let session = create_resumable_session(
        &h,
        work.to_str().unwrap(),
        &format!("sh -c {}", shell_quote(&script)),
        80,
        24,
    )
    .await;
    let (pid, _guard) = fixture
        .monitor_in_scope(&id, &unit_name(&session.id, 0).unwrap())
        .await;
    wait_for_agent_ready(&h, &session.id).await;
    h.client.restart_session(&session.id, true).await.unwrap();
    wait_until_pid_gone(pid, 10).await;
    wait_for_agent_ready(&h, &session.id).await;
    let notices = podman_notices(&h, &session.id).await;
    assert_eq!(notices.len(), 1);
    assert!(notices[0].text.contains(&id));
    let store = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .unwrap();
    assert!(
        store
            .has_session_notification(&session.id, 0, "podman_stop_failed")
            .await
            .unwrap(),
        "the warning belongs to the launch whose container Farhelm tried to stop"
    );
    assert!(
        !store
            .has_session_notification(&session.id, 1, "podman_stop_failed")
            .await
            .unwrap(),
        "the relaunched agent has its own notification budget"
    );

    let id = "b".repeat(64);
    let tab = h.client.open_tab(&session.id).await.unwrap();
    start_in_tab(
        &h,
        &session.id,
        &tab.id,
        &fixture.monitor_command(&id, false),
    )
    .await;
    let (pid, _guard) = fixture
        .monitor_in_scope(&id, &tab_unit_name(&session.id, &tab.id).unwrap())
        .await;
    let deleted = h
        .client
        .delete_session_with(&session.id, farhelm_proto::DeleteGuard::Unconditional)
        .await
        .unwrap();
    wait_until_pid_gone(pid, 10).await;
    let notice = deleted
        .notice
        .expect("successful Delete carries the advisory");
    assert!(notice.contains(&id) && notice.contains("podman rm -f"));
    assert!(
        !h.client
            .list_sessions()
            .await
            .unwrap()
            .sessions
            .iter()
            .any(|listed| listed.id == session.id)
    );
}
