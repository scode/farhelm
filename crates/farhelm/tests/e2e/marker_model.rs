//! Positive marker selection: an inner supervisor's own agents must be
//! reaped by its session's stop even when nested inside another tab.

use crate::harness::*;

use crate::terminal_tabs::{run_in_shell, wait_for_shell, window_rows};

// ---------------------------------------------------------------------------
// The marker model (PLAN_M4.md item 2, positive selection)
// ---------------------------------------------------------------------------

/// An agent launched by a supervisor that is ITSELF running inside
/// somebody's farhelm tab must still be reaped by its own session's stop —
/// and a genuine tab of that same session must still survive it.
///
/// This is the dogfooding hole positive marking exists to close. A
/// supervisor started inside a tab inherits that tab's `FARHELM_TAB_ID`,
/// its tmux server inherits it, and every inner agent inherits it too;
/// under the exclusion-based design that came first, every one of those
/// inner agents looked like a tab process and escaped stop entirely.
/// `launch_env` reproduces exactly that inheritance — it injects into
/// tmux, which is the same route the real ambient value would take.
///
/// Both halves are asserted in one run on purpose: a stop that reaped the
/// agent by simply ignoring tab markers altogether would pass the first
/// assertion and fail the second.
#[farhelm_testtrace::test]
async fn an_agent_wearing_an_ambient_tab_marker_is_still_reaped_while_a_real_tab_survives() {
    // A plausible outer-tab id: minted-shaped, so it is the value a real
    // ambient marker would carry rather than something the parse would
    // reject for its shape alone.
    let ambient_tab = "0e5d9a11-0000-4000-8000-00000000abcd".to_string();
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_env: vec![("FARHELM_TAB_ID".to_string(), ambient_tab.clone())],
            ..SupervisorSeams::default()
        },
    )
    .await;
    let work = farhelm_teststate::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-reparent"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_agent_chan, mut agent_seen, mut agent_rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let agent_pid = extract_pid(&agent_seen, "SELF-PID:");
    let agent_daemon = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (tab_chan, mut tab_seen, mut tab_rx) = h
        .client
        .attach_terminal_live(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "tab-lease",
        )
        .await
        .expect("attach the tab");
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;

    h.client.stop_session(&session.id).await.expect("stop");

    // The inner agent and its reparented daemon are gone despite wearing
    // (or having inherited into their window) an outer tab's marker.
    wait_until_pid_gone(agent_pid, 15).await;
    wait_until_pid_gone(agent_daemon, 15).await;
    // The session's OWN tab is untouched.
    run_in_shell(
        &h.client,
        tab_chan,
        &mut tab_rx,
        &mut tab_seen,
        "printf 'STILL%sHERE\\n' -",
        "STILL-HERE",
        20,
    )
    .await;
}

/// A daemon left behind by a session launched BEFORE either kind marker
/// existed — session-marked and nothing else — must still be reaped by
/// stop.
///
/// The legacy bucket is the third root set `SweepTarget::AgentOnly`
/// claims, and it exists because a purely agent-marker-keyed stop would
/// silently stop reaching such processes on every host that upgraded.
/// `MarkedDecoy` is exactly that shape: the session marker, no kind
/// marker, and outside every cgroup this session owns.
#[farhelm_testtrace::test]
async fn a_session_marked_process_with_no_kind_marker_is_still_reaped_by_stop() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut seen, mut rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    let legacy = MarkedDecoy::spawn(&session.id);
    let legacy_pid = legacy.pid();
    h.client.stop_session(&session.id).await.expect("stop");
    wait_until_pid_gone(legacy_pid, 15).await;
}

/// The decoy must scrub inherited kind markers, and that scrub must be
/// provable on a marker-clean host.
///
/// The bug this pins: a test runner that itself lives inside a Farhelm
/// session carries `FARHELM_AGENT_ID` (or, from a tab, `FARHELM_TAB_ID`),
/// and a decoy inheriting either stops being the legacy shape the tests
/// above depend on — the sweep reads the marker as "already claimed" and
/// correctly refuses the decoy, timing out both sweep-backstop tests. That
/// failure only manifests on such a host, so a live-child check would pass
/// on clean CI even with the scrub deleted. Asserting the CONFIGURED
/// operations on the builder (`Command::get_envs`, where an `env_remove`
/// appears as a `None` value) makes a dropped scrub visible everywhere.
#[farhelm_testtrace::test]
fn the_marked_decoy_scrubs_inherited_kind_markers_from_its_child() {
    let command = MarkedDecoy::command("some-session");
    let envs: Vec<(std::ffi::OsString, Option<std::ffi::OsString>)> = command
        .get_envs()
        .map(|(key, value)| (key.to_os_string(), value.map(|v| v.to_os_string())))
        .collect();
    let entry = |key: &str, value: Option<&str>| {
        (
            std::ffi::OsString::from(key),
            value.map(std::ffi::OsString::from),
        )
    };
    assert!(
        envs.contains(&entry("FARHELM_SESSION_ID", Some("some-session"))),
        "the decoy must still carry the session marker it exists to wear"
    );
    assert!(
        envs.contains(&entry("FARHELM_AGENT_ID", None)),
        "the decoy must scrub an inherited agent kind marker"
    );
    assert!(
        envs.contains(&entry("FARHELM_TAB_ID", None)),
        "the decoy must scrub an inherited tab kind marker"
    );
}

/// The agent's window carries its marker after a create AND after a
/// restart that reuses the same pane (PLAN_M4.md item 2).
///
/// The marker became load-bearing when pane-less reload learned to prefer
/// it: without it, a session whose durable pane record is empty could
/// recover onto a TAB's pane and then reap that tab on its next stop. So
/// its presence is asserted directly against tmux rather than inferred,
/// and asserted again after a restart — `respawn-pane` keeps the window
/// and its options, and this is what would notice if a future change
/// swapped it for a fresh window instead.
#[farhelm_testtrace::test]
async fn the_agent_window_keeps_its_marker_across_a_restart_in_place() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let agent_marker = |rows: &[String]| -> Option<String> {
        rows.iter().find_map(|row| {
            let fields: Vec<&str> = row.split('|').collect();
            (fields.get(1) == Some(&format!("fh-{}", session.id).as_str())
                && fields.get(2) == Some(&"@0"))
            .then(|| fields.get(4).unwrap_or(&"").to_string())
        })
    };
    assert_eq!(
        agent_marker(&window_rows(&h).await).as_deref(),
        Some(session.id.as_str()),
        "a created session's agent window must carry its own session id as the agent marker"
    );

    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart");
    assert_eq!(
        agent_marker(&window_rows(&h).await).as_deref(),
        Some(session.id.as_str()),
        "a restart that reuses the pane must leave the agent window's marker intact"
    );
}
