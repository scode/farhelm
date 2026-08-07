//! Archive lifecycle coverage against a real supervisor and private tmux.

use crate::create_idempotency::handoff_to_new_supervisor;
use crate::harness::*;
use farhelm_proto::{AgentKind, RestartMode, STOP_ANNOTATION};
use farhelm_supervisor::service::{ArchiveGate, ArchiveStage};

/// Create the archive fixture with both provenance dimensions populated;
/// the high-level client intentionally leaves parent selection to spawn.
async fn create_profile_child(
    h: &Harness,
    parent: &str,
    profile_id: &str,
    cwd: &str,
) -> SessionInfo {
    let (client_side, server_side) = tokio::io::duplex(1 << 20);
    let sup = Arc::clone(&h.sup);
    tokio::spawn(async move {
        let _ = handle_connection(sup, server_side).await;
    });
    let (read_half, write_half) = tokio::io::split(client_side);
    let mut reader = FrameReader::new(read_half);
    let mut writer = FrameWriter::new(write_half);
    handshake(&mut reader, &mut writer, "helm")
        .await
        .expect("handshake");
    writer
        .write_control(&ControlMsg::CreateSession {
            req_id: 1,
            parent: Some(parent.to_string()),
            cwd: cwd.to_string(),
            invocation: None,
            profile_id: Some(profile_id.to_string()),
            profile_name: None,
            title: Some("archive contract".to_string()),
            cols: 80,
            rows: 24,
            intent_key: None,
            agent_kind: None,
            resume_template: None,
        })
        .await
        .expect("send child create");
    let reply = parse_control(
        &reader
            .read_frame()
            .await
            .expect("read child reply")
            .expect("connection stayed open"),
    )
    .expect("decode child reply");
    let ControlMsg::SessionCreated { session, .. } = reply else {
        panic!("profile child creation was refused: {reply:?}");
    };
    session
}

/// Archive reaches the whole live session while retaining exactly the state
/// a later restart needs.
///
/// This one scenario keeps the cross-phase contract together: a live agent
/// and tab disappear, metadata and an attachment remain, attach names the
/// archived state, a retry is harmless, and restart creates a new terminal
/// whose relaunched command can still read that attachment. Splitting those
/// assertions across isolated fixtures would miss the preservation boundary
/// between teardown and relaunch.
#[tokio::test]
async fn archive_tears_down_processes_and_tabs_but_restart_keeps_the_attachment() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let script = work.path().join("agent.sh");
    let first_pid = work.path().join("first.pid");
    let child_pid_path = work.path().join("child.pid");
    std::fs::write(
        &script,
        format!(
            "echo $$ > {}\nsleep 120 &\necho $! > {}\nwait\n",
            shell_words::quote(&first_pid.to_string_lossy()),
            shell_words::quote(&child_pid_path.to_string_lossy()),
        ),
    )
    .expect("write first launch script");
    let invocation = format!("/bin/sh {}", shell_words::quote(&script.to_string_lossy()));
    let profile = h
        .client
        .create_profile("Archive Fixture", &invocation, AgentKind::Generic, None)
        .await
        .expect("create non-default source profile");
    let parent = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            "/bin/sh -c 'sleep 120'",
            Some("archive parent".to_string()),
            80,
            24,
        )
        .await
        .expect("create parent");
    let session =
        create_profile_child(&h, &parent.id, &profile.id, &work.path().to_string_lossy()).await;
    assert_eq!(session.parent.as_deref(), Some(parent.id.as_str()));
    assert!(session.source_profile.is_some());
    wait_for_file(&first_pid, 10).await;
    let agent_pid: u32 = std::fs::read_to_string(&first_pid)
        .expect("read agent pid")
        .trim()
        .parse()
        .expect("parse agent pid");
    wait_for_file(&child_pid_path, 10).await;
    let child_pid: u32 = std::fs::read_to_string(&child_pid_path)
        .expect("read child pid")
        .trim()
        .parse()
        .expect("parse child pid");

    let tab = h.client.open_tab(&session.id).await.expect("open tab");
    assert_eq!(
        h.client
            .list_sessions()
            .await
            .expect("list before archive")
            .sessions
            .into_iter()
            .find(|row| row.id == session.id)
            .expect("created session")
            .tabs,
        vec![tab]
    );

    farhelm_supervisor::attachments::ensure_session_dirs(h.state.path(), &session.id)
        .await
        .expect("create attachment directories");
    let attachment =
        farhelm_supervisor::attachments::session_dir(h.state.path(), &session.id).join("kept.txt");
    std::fs::write(&attachment, b"ARCHIVE_ATTACHMENT_OK\n").expect("write attachment");
    let spec_path = spec_path_for_launch(h.state.path(), &session.id, 0);
    let status_path = status_path_for_spec(&spec_path);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while spec_path.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the launch shim never consumed its real spec"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    std::fs::write(&spec_path, b"credential-bearing launch spec").expect("plant launch spec");
    std::fs::write(&status_path, b"exec failure detail").expect("plant launch status");
    let snapshot = h.state.path().join("snapshots").join(&session.id);
    std::fs::create_dir_all(snapshot.parent().expect("snapshot parent")).unwrap();
    std::fs::write(&snapshot, b"terminal secret").expect("plant snapshot");
    let second_pid = work.path().join("second.pid");
    std::fs::write(
        &script,
        format!(
            "echo $$ > {}\ncat {}\nsleep 120\n",
            shell_words::quote(&second_pid.to_string_lossy()),
            shell_words::quote(&attachment.to_string_lossy()),
        ),
    )
    .expect("prepare restart script");

    let (_attached_channel, mut attached) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Agent,
            "archive-incumbent",
        )
        .await
        .expect("attach before archive");

    let archived = h
        .client
        .archive_session(&session.id)
        .await
        .expect("archive");
    assert!(archived.archived);
    assert_eq!(archived.title, "archive contract");
    assert_eq!(archived.cwd, work.path().to_string_lossy());
    assert_eq!(archived.invocation, invocation);
    assert_eq!(archived.annotation.as_deref(), Some(STOP_ANNOTATION));
    assert!(archived.tabs.is_empty());
    assert!(matches!(
        archived.status,
        SessionStatus::Exited { exit_code: None }
    ));
    let detach_reason = expect_detached(&mut attached, 10).await;
    assert!(
        detach_reason.contains("archived"),
        "the active attachment must learn why its terminal ended: {detach_reason}"
    );
    assert!(
        tokio::time::timeout(Duration::from_secs(1), attached.recv())
            .await
            .expect("the detached stream must close promptly")
            .is_none(),
        "Detached must be the terminal stream's final event"
    );
    let mut expected = session.clone();
    expected.archived = true;
    expected.status = SessionStatus::Exited { exit_code: None };
    expected.annotation = Some(STOP_ANNOTATION.to_string());
    expected.tabs.clear();
    assert_eq!(archived, expected, "only archive-owned metadata may change");
    assert!(
        attachment.exists(),
        "archive must retain committed attachments"
    );
    assert!(!spec_path.exists(), "archive removes every launch spec");
    assert!(
        !status_path.exists(),
        "archive removes launch status artifacts"
    );
    assert!(!snapshot.exists(), "archive removes terminal snapshots");

    let listed = h
        .client
        .list_sessions()
        .await
        .expect("list through the real supervisor after archive")
        .sessions
        .into_iter()
        .find(|row| row.id == session.id)
        .expect("the supervisor must retain the archived row for fleet drains");
    assert_eq!(listed, archived);

    let tmux_name = format!("fh-{}", session.id);
    let tmux = tmux_query(
        &h.state.path().join("tmux.sock"),
        &["has-session", "-t", &tmux_name],
    )
    .await;
    assert!(
        !tmux.status.success(),
        "archive must remove the agent pane and every tab window"
    );
    wait_until_pid_gone(agent_pid, 10).await;
    wait_until_pid_gone(child_pid, 10).await;

    let repeated = h
        .client
        .archive_session(&session.id)
        .await
        .expect("repeat archive");
    assert_eq!(repeated, archived, "an archive retry returns the same row");

    let attach_error = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Agent,
            "archived-lease",
        )
        .await
        .expect_err("an archived session has no terminal");
    let refusal = attach_error
        .downcast_ref::<SupervisorError>()
        .expect("supervisor refusal");
    assert_eq!(refusal.kind, ErrorKind::InvalidRequest);
    assert!(refusal.message.contains("archived") && refusal.message.contains("restart"));

    let restarted = h
        .client
        .restart_session(&session.id, RestartMode::Fresh, false)
        .await
        .expect("restart archived session");
    assert!(!restarted.archived);
    assert!(restarted.tabs.is_empty(), "archive's tabs must not return");
    let (_channel, mut terminal) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Agent,
            "restarted-lease",
        )
        .await
        .expect("attach fresh terminal");
    let mut seen = Vec::new();
    wait_for(&mut terminal, &mut seen, "ARCHIVE_ATTACHMENT_OK", 10).await;
    drop(terminal);
    wait_for_file(&second_pid, 10).await;
    assert!(attachment.exists());
}

/// Archive does not need the working directory that a later restart or tab
/// creation would use.
///
/// This keeps terminal teardown available after a checkout or scratch
/// directory has already disappeared. Refusing here would strand exactly
/// the dead sessions archive exists to retire.
#[tokio::test]
async fn archive_succeeds_after_the_working_directory_disappears() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let cwd = work.path().to_path_buf();
    let session = h
        .client
        .create_session(
            &cwd.to_string_lossy(),
            "/bin/sh -c 'sleep 120'",
            Some("vanished archive cwd".to_string()),
            80,
            24,
        )
        .await
        .expect("create");
    drop(work);
    assert!(
        !cwd.exists(),
        "the fixture must remove the working directory"
    );

    let archived = h
        .client
        .archive_session(&session.id)
        .await
        .expect("archive without a working directory");
    assert!(archived.archived);
    assert!(archived.tabs.is_empty());
}

/// Delete still owns retained attachments after archive has removed the
/// terminal side of the session.
///
/// Archive is not a second storage boundary: it keeps the ordinary session
/// attachment directory in place, and a later delete must remove that
/// directory along with the retained row.
#[tokio::test]
async fn deleting_an_archived_session_removes_its_row_and_attachments() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            "/bin/sh -c 'sleep 120'",
            Some("archive then delete".to_string()),
            80,
            24,
        )
        .await
        .expect("create");
    farhelm_supervisor::attachments::ensure_session_dirs(h.state.path(), &session.id)
        .await
        .expect("create attachment directories");
    let session_dir = farhelm_supervisor::attachments::session_dir(h.state.path(), &session.id);
    let attachment = session_dir.join("retained.txt");
    std::fs::write(&attachment, b"delete me after archive\n").expect("write attachment");

    h.client
        .archive_session(&session.id)
        .await
        .expect("archive");
    assert!(attachment.exists(), "archive retains the attachment");
    h.client
        .delete_session(&session.id)
        .await
        .expect("delete archived session");
    assert!(
        !session_dir.exists(),
        "delete must remove the archived session's attachment directory"
    );
    assert!(
        h.client
            .list_sessions()
            .await
            .expect("list after delete")
            .sessions
            .into_iter()
            .all(|row| row.id != session.id),
        "delete must remove the retained archived row"
    );
}

/// Every uncertain teardown boundary fails closed: no archive flag is
/// published when the supervisor cannot prove the session is terminal-less.
#[tokio::test]
async fn archive_seam_failures_leave_the_session_unarchived() {
    for stage in [
        ArchiveStage::PaneProbe,
        ArchiveStage::TabRediscovery,
        ArchiveStage::ScopeEnumeration,
        ArchiveStage::Sweep,
        ArchiveStage::ArtifactRemoval,
    ] {
        let gate: ArchiveGate = Arc::new(move |reached| {
            Box::pin(async move {
                if reached == stage {
                    anyhow::bail!("injected archive failure at {stage:?}");
                }
                Ok(())
            })
        });
        let h = harness_with_seams(
            SupervisorTimeouts::default(),
            SupervisorSeams {
                archive_gate: Some(gate),
                ..SupervisorSeams::default()
            },
        )
        .await;
        let work = tempfile::tempdir().expect("workdir");
        let session = h
            .client
            .create_session(
                &work.path().to_string_lossy(),
                "/bin/sh -c 'sleep 120'",
                Some(format!("archive failure {stage:?}")),
                80,
                24,
            )
            .await
            .expect("create");

        h.client
            .archive_session(&session.id)
            .await
            .expect_err("the injected teardown uncertainty must refuse archive");
        let listed = h
            .client
            .list_sessions()
            .await
            .expect("list after refusal")
            .sessions
            .into_iter()
            .find(|row| row.id == session.id)
            .expect("the refused archive keeps its row");
        assert!(!listed.archived, "{stage:?} published the archive flag");
    }
}

/// Startup never rediscovers a same-named tmux husk as the terminal of an
/// archived row; only restart may clear the durable archive boundary.
#[tokio::test]
async fn reopening_an_archived_row_ignores_a_same_named_tmux_husk() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            "/bin/sh -c 'sleep 120'",
            Some("archived startup fence".to_string()),
            80,
            24,
        )
        .await
        .expect("create");
    h.client
        .archive_session(&session.id)
        .await
        .expect("archive");
    let tmux_name = format!("fh-{}", session.id);
    let socket = h.state.path().join("tmux.sock");
    let husk = tmux_query(
        &socket,
        &["new-session", "-d", "-s", &tmux_name, "sleep 120"],
    )
    .await;
    assert!(husk.status.success(), "plant same-named tmux husk");

    let Harness {
        client,
        sup,
        _tmux,
        state,
        _slot,
    } = h;
    let replacement = handoff_to_new_supervisor(state.path(), sup, client).await;
    let client = connect_client(&replacement).await;
    let row = client
        .list_sessions_page(None, None)
        .await
        .expect("list after reopen")
        .sessions
        .into_iter()
        .find(|row| row.id == session.id)
        .expect("archived row survives reopen");
    assert!(row.archived);
    assert!(matches!(
        row.status,
        SessionStatus::Exited { exit_code: None }
    ));
    assert_eq!(row.annotation.as_deref(), Some(STOP_ANNOTATION));
    assert!(row.tabs.is_empty());

    drop(client);
    drop(replacement);
    drop(_tmux);
    drop(state);
    drop(_slot);
}
