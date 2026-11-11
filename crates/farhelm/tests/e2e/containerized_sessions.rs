//! Compatibility for the retired supervisor-outside container model.
//!
//! New environments run an ordinary supervisor inside their container. These
//! fixtures retain old persisted choices solely to prove refusal and Delete.
//! They create no container and require no offline image.

use crate::harness::*;
use farhelm_proto::{ContainerConfig, LaunchHarness, LaunchSelection, SessionLaunch};

/// Use a command needing no interactive input with the old structured launch shape.
/// The selected harness supplies metadata only and spends no vendor turns.
fn launch(container: Option<ContainerConfig>) -> SessionLaunch {
    SessionLaunch::Agent {
        selection: LaunchSelection {
            harness: LaunchHarness::Codex,
            model: None,
            effort: None,
            permissions: None,
            permission_flags: None,
            wrapper_prefix: None,
            container,
            workspace_trust: None,
        },
        start: vec!["true".into(), "{farhelm_args}".into()],
        resume: Some(vec![
            "true".into(),
            "{conversation}".into(),
            "{farhelm_args}".into(),
        ]),
    }
}

/// Old file-backed configuration must still decode from stored rows, although
/// its image and token paths grant no new creation or execution authority.
fn legacy_config() -> ContainerConfig {
    serde_json::from_value(serde_json::json!({
        "engine": "podman", "image_file": "/gone/old-image.tar", "api_proxy_port": 43117,
        "github_token_file": "/gone/old-token"
    }))
    .unwrap()
}

/// Seed an actual historical row, then load it through an ordinary supervisor
/// restart. A current create is used only before its stored launch becomes old.
async fn legacy_session() -> (Harness, SessionInfo, farhelm_teststate::TestDir) {
    let h = harness().await;
    let cwd = farhelm_teststate::tempdir().unwrap();
    let session = h
        .client
        .create_session_with_extras(
            cwd.path().to_str().unwrap(),
            launch(None),
            None,
            80,
            24,
            Default::default(),
        )
        .await
        .unwrap();
    let old_launch = launch(Some(legacy_config()));
    let mut stored_launch = serde_json::to_value(&old_launch).unwrap();
    let config = stored_launch["selection"]["container"]
        .as_object_mut()
        .unwrap();
    config.remove("image");
    config.insert(
        "image_file".into(),
        serde_json::json!("/gone/old-image.tar"),
    );
    assert_eq!(
        serde_json::from_value::<SessionLaunch>(stored_launch.clone()).unwrap(),
        old_launch
    );
    {
        let conn = farhelm_supervisor::db::open_private_existing(
            &h.state.path().join("supervisor.db"),
            "retired container fixture",
        )
        .unwrap();
        assert_eq!(
            conn.execute(
                "UPDATE sessions SET session_launch = ?1 WHERE id = ?2",
                [
                    serde_json::to_string(&stored_launch).unwrap(),
                    session.id.clone()
                ],
            )
            .unwrap(),
            1
        );
    }
    let h = h
        .restart_supervisor(|state| async move { supervisor_on(&state).await.unwrap() })
        .await;
    let listed = h.client.list_sessions().await.unwrap();
    let stored = listed
        .sessions
        .iter()
        .find(|row| row.id == session.id)
        .unwrap();
    assert_eq!(
        stored.launch, old_launch,
        "premise: the old launch loaded through restart"
    );
    (h, session, cwd)
}

/// Direct clients cannot recreate the retired model, including with a missing
/// old image. Refusal precedes every row, checkout and engine side effect.
#[farhelm_testtrace::test]
async fn old_container_create_is_refused_without_registering_a_session() {
    let h = harness().await;
    let cwd = farhelm_teststate::tempdir().unwrap();
    let before = h.client.list_sessions().await.unwrap().sessions;
    let error = h
        .client
        .create_session_with_extras(
            cwd.path().to_str().unwrap(),
            launch(Some(legacy_config())),
            None,
            80,
            24,
            Default::default(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("retired"), "{error:#}");
    assert_eq!(h.client.list_sessions().await.unwrap().sessions, before);
    assert!(!h.state.path().join("container-launch").exists());
}

/// Persisted legacy sessions remain listed, but every terminal or ordinary
/// lifecycle action refuses before it can execute against the outside host.
#[farhelm_testtrace::test]
async fn retired_session_refuses_terminal_and_lifecycle_use() {
    let (h, session, _cwd) = legacy_session().await;
    let id = session.id.as_str();
    let errors = [
        h.client.stop_session(id).await.unwrap_err(),
        h.client.restart_session(id, true).await.unwrap_err(),
        h.client
            .restart_session_with(id, true, Some(launch(None)), None)
            .await
            .unwrap_err(),
        h.client.open_tab(id).await.unwrap_err(),
        h.client.close_tab(id, "unused-tab").await.unwrap_err(),
        h.client
            .rename_session(id, "changed", None)
            .await
            .unwrap_err(),
    ];
    for error in errors {
        assert!(error.to_string().contains("retired"), "{error:#}");
    }
    let attachment = h.client.attach(id, 80, 24).await;
    let error = match attachment {
        Ok(_) => panic!("retired terminal attached"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("retired"), "{error:#}");
    let listed = h.client.list_sessions().await.unwrap();
    assert_eq!(
        listed
            .sessions
            .iter()
            .find(|row| row.id == id)
            .unwrap()
            .title,
        session.title
    );
    h.client.delete_session(id).await.unwrap();
}

/// Legacy launch intent cannot authorize file access or prompt delivery on the
/// outside host. A real readable file distinguishes refusal from a missing path.
#[farhelm_testtrace::test]
async fn retired_session_refuses_prompts_uploads_and_downloads() {
    let (h, session, cwd) = legacy_session().await;
    let path = cwd.path().join("result.txt");
    std::fs::write(&path, b"outside host file").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"outside host file");
    let errors = [
        h.client
            .prompt_session(&session.id, "hello")
            .await
            .unwrap_err(),
        h.client
            .stat_file(&session.id, "result.txt")
            .await
            .unwrap_err(),
    ];
    for error in errors {
        assert!(error.to_string().contains("retired"), "{error:#}");
    }
    let download = h.client.begin_download(&session.id, "result.txt").await;
    let error = match download {
        Ok(_) => panic!("retired file download admitted"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("retired"), "{error:#}");
    let upload = h.client.begin_upload(&session.id, "new.txt", 1).await;
    let error = match upload {
        Ok(_) => panic!("retired upload admitted"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("retired"), "{error:#}");
    assert_eq!(std::fs::read(&path).unwrap(), b"outside host file");
    h.client.delete_session(&session.id).await.unwrap();
}

/// Deletion derives the owned name from the stored session UUID and ignores
/// vanished legacy image/token paths. With no container ever created here,
/// the installed engine must confirm absence or a missing engine ends cleanup.
#[farhelm_testtrace::test]
async fn retired_session_delete_keeps_stored_compatibility_and_confirms_absence() {
    let (h, session, _cwd) = legacy_session().await;
    h.client.delete_session(&session.id).await.unwrap();
    assert!(
        h.client
            .list_sessions()
            .await
            .unwrap()
            .sessions
            .iter()
            .all(|row| row.id != session.id)
    );
}
