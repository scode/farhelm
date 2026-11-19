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
    fn new() -> Self {
        let work = farhelm_teststate::tempdir().unwrap();
        symlink(fixtures_bin(), work.path().join("conmon")).unwrap();
        let engine = work.path().join("podman");
        let dir = shell_quote(work.path().to_str().unwrap());
        std::fs::write(
            &engine,
            format!(
                "#!/bin/sh\n\
                 printf '%s\\n' \"$*\" >> {dir}/podman-calls\n\
                 [ \"$1\" = --remote=false ] || exit 2\n\
                 shift\n\
                 [ \"$2 $3 $4 $5\" = 'stop --time 5 --' ] || exit 2\n\
                 pid=$(cat {dir}/\"$6\".pid) || exit 2\n\
                 case $(ps -o stat= -p \"$pid\") in ''|Z*) exit 2;; esac\n\
                 printf '%s\\n' \"$6\" >> {dir}/live-stops\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(engine, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self { work }
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
    let fixture = PodmanFixture::new();
    let scopes = ScopeManager::systemd().with_podman(fixture.work.path().join("podman"));
    let Some(scopes) = probe_scope_manager(
        "lifecycle_stops_containers_in_the_selected_scopes_before_signalling",
        scopes,
    )
    .await
    else {
        return;
    };
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            scopes,
            ..SupervisorSeams::default()
        },
    )
    .await;
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
