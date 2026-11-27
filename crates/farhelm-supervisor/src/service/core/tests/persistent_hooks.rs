use super::*;
use farhelm_proto::PersistentHookSetup;

/// Prepare a real Grok generation up to the point where its host can start. The row lets the tests check the same
/// facts a supervisor would reload after an interruption, without running a vendor binary.
async fn prepare_grok(
    state: &StateDir,
    home: &Path,
    hook_policy: crate::agent_kind::AgentHooks,
    requested: bool,
    captured: Option<&str>,
) -> (Arc<Supervisor>, PreparedLaunch) {
    let contents = state.path().join("Farhelm.app/Contents");
    let sup = Supervisor::new_with_seams(
        state.path(),
        contents.join("Versions/9.9.9/farhelm"),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            agent_hooks: hook_policy,
            user_home: Some(home.to_path_buf()),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor with isolated vendor home");
    let command = "grok {farhelm_args}";
    let launch = SessionLaunch::Command(farhelm_proto::session_launch::CommandLaunch {
        custom_agent: None,
        command: command.into(),
        yolo: false,
        agent: Some(LaunchHarness::Grok),
        resume: None,
    });
    launch.validate_new().expect("valid declared Grok command");
    let cwd = state.path().to_str().unwrap();
    sup.store
        .insert_session(
            StoredSession {
                id: "grok".into(),
                parent: None,
                title: "grok".into(),
                created_at: now_unix(),
                last_activity_at: now_unix(),
                last_work_started_at: 0,
                creation_seq: 0,
                cwd: cwd.into(),
                launch: launch.clone(),
                terminal_address: String::new(),
                terminal_kind: crate::store::TerminalKind::Host,
                outcome: LastOutcome::Launching,
                canonical_cwd: Some(cwd.into()),
                captured_conversation: captured.map(str::to_owned),
                conversation_source: captured.map(|_| "hook".into()),
                capture_ownership_version: i64::from(captured.is_some()),
                generation: 0,
                launch_scoped: false,
                omp_reporter_asset: None,
                omp_launch_program: None,
                launch_hooked: false,
                handoff_marker_expected: false,
                launch_boot_ended: false,
                persistent_hook_setup: PersistentHookSetup::NotRequested,
                setup_persistent_hooks_on_create: requested,
            },
            None,
        )
        .await
        .unwrap();
    let row = sup.store.session("grok").await.unwrap().unwrap();
    assert_eq!(row.captured_conversation.as_deref(), captured);
    assert_eq!(sup.session_farhelm_exe, contents.join("MacOS/farhelm"));
    assert_ne!(sup.session_farhelm_exe, sup.farhelm_exe);
    let prepared = sup
        .prepare_launch(
            "grok",
            "credential",
            0,
            launch.start_argv().expect("parse the launch's command"),
            Some(ShellSource::Command(command)),
            &IntegrationSnapshot::of(&launch),
            FarhelmArgsMode::Expand(crate::agent_kind::LaunchPhase::Start),
            cwd,
            None,
            None,
            None,
            requested,
            None,
            None,
        )
        .await
        .expect("hook setup cannot prevent the host from starting the launch");
    (sup, prepared)
}

/// A declared Grok command needs the files before spawn. Its Farhelm argument slot expands to no hook flags.
#[farhelm_testtrace::test]
async fn persistent_hook_install_arms_the_launch_before_spawn() {
    let state = StateDir::new();
    let home = state.path().join("isolated-home");
    let hook_file = home.join(".grok/hooks/farhelm.json");
    assert!(!hook_file.exists());
    let (sup, launch) = prepare_grok(
        &state,
        &home,
        crate::agent_kind::AgentHooks::All,
        true,
        None,
    )
    .await;
    assert!(launch.hooked);
    assert!(launch.hook_skip.is_none());
    assert_eq!(launch.persistent_hook_setup, PersistentHookSetup::Installed);
    let spec: crate::launch::LaunchSpec =
        serde_json::from_slice(&std::fs::read(launch.artifacts.spec()).unwrap()).unwrap();
    let [shell, flag, source] = spec.argv.as_slice() else {
        panic!("expected a shell command, got {:?}", spec.argv);
    };
    assert_eq!([shell, flag], ["bash", "-c"]);
    assert_eq!(shell_words::split(source).unwrap(), ["grok"]);
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(hook_file).unwrap()).unwrap();
    let command = config["hooks"]["SessionStart"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert_eq!(
        shell_words::split(command).unwrap(),
        [
            sup.session_farhelm_exe_str.as_str(),
            "internal",
            "hook",
            "--vendor",
            "grok"
        ]
    );
    let row = sup.store.session("grok").await.unwrap().unwrap();
    assert!(row.launch_hooked);
    assert!(row.setup_persistent_hooks_on_create);
    assert_eq!(row.persistent_hook_setup, launch.persistent_hook_setup);
}

/// Failure leaves a reason for Restart and the existing setup warning while the launch spec remains usable.
#[farhelm_testtrace::test]
async fn persistent_hook_install_failure_preserves_the_launch() {
    let state = StateDir::new();
    let home = state.path().join("isolated-home");
    std::fs::create_dir(&home).unwrap();
    let blocked = home.join(".grok");
    std::fs::write(&blocked, "regular file blocks directory creation").unwrap();
    assert!(blocked.is_file());
    let (sup, launch) = prepare_grok(
        &state,
        &home,
        crate::agent_kind::AgentHooks::All,
        true,
        Some("earlier-conversation"),
    )
    .await;
    assert!(launch.artifacts.spec().is_file());
    assert!(!launch.hooked);
    let PersistentHookSetup::Failed { reason } = &launch.persistent_hook_setup else {
        panic!("expected failure, got {:?}", launch.persistent_hook_setup);
    };
    assert!(reason.contains(".grok/hooks"), "{reason}");
    let Some(crate::service::notifications::HookSkip::PersistentSetup(warning)) = &launch.hook_skip
    else {
        panic!("hook setup failure should reach the launch notification");
    };
    assert_eq!(warning, reason);
    let row = sup.store.session("grok").await.unwrap().unwrap();
    assert!(!row.launch_hooked);
    assert_eq!(row.persistent_hook_setup, launch.persistent_hook_setup);
    sup.notify_hook_skip("grok", 0, launch.hook_skip.unwrap())
        .await;
    let notifications = sup.store.session_notifications("grok").await.unwrap();
    assert_eq!(notifications.len(), 1);
    assert!(notifications[0].text.contains(reason));
}

/// Both the helm's consent and the host's policy are required before Farhelm writes to the agent's home.
#[farhelm_testtrace::test]
async fn persistent_hook_install_respects_consent_and_host_opt_out() {
    for (policy, requested, expected) in [
        (
            crate::agent_kind::AgentHooks::All,
            false,
            PersistentHookSetup::NotRequested,
        ),
        (
            crate::agent_kind::parse_agent_hooks("claude,antigravity"),
            true,
            PersistentHookSetup::Failed {
                reason: crate::agent_kind::HOOKS_DISABLED_REASON.into(),
            },
        ),
    ] {
        let state = StateDir::new();
        let home = state.path().join("isolated-home");
        assert!(!home.exists());
        let (sup, launch) = prepare_grok(&state, &home, policy, requested, None).await;
        assert!(!home.exists());
        assert!(!launch.hooked);
        assert!(launch.hook_skip.is_none());
        assert_eq!(launch.persistent_hook_setup, expected);
        let row = sup.store.session("grok").await.unwrap().unwrap();
        assert!(!row.launch_hooked);
        assert_eq!(row.persistent_hook_setup, expected);
    }
}
