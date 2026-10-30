//! Custom command text crosses Bash once; filled placeholders remain data.
//!
//! These cases use a private login-shell environment and the real shim/tmux
//! path. Literal argv unit tests cannot distinguish preserved shell source
//! from a reconstructed command, or prove that a filled value stays inert.

use crate::boot_id_durable_outcome::wait_for_dead_pane;
use crate::harness::*;
use farhelm_proto::text::shell_quote;

const ENV_VALUE: &str = "value ' $(touch env-injected) *";

/// Give the real launch path a controlled home without changing this process.
///
/// Skipping the operator's startup files makes the expected environment an
/// owned fixture premise. The outer shell still receives the production
/// login-shell arguments and execs the real launch shim; the command's inner
/// Bash is selected by the product code, not by this fixture.
async fn shell_harness(home: &std::path::Path) -> Harness {
    use std::os::unix::fs::PermissionsExt;

    let shell = home.join("login-shell");
    std::fs::write(
        &shell,
        "#!/bin/sh\nexec /bin/bash --noprofile --norc -i -c \"$4\"\n",
    )
    .expect("write the owned outer shell");
    std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o700))
        .expect("make the outer shell executable");
    harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_env: vec![
                ("HOME".to_string(), home.to_string_lossy().into_owned()),
                ("COMMAND_SHELL_VALUE".to_string(), ENV_VALUE.to_string()),
            ],
            launch_shell: Some(shell.to_string_lossy().into_owned()),
            ..SupervisorSeams::default()
        },
    )
    .await
}

/// Preserve expansion, pipelines and statement boundaries in user text.
///
/// Rejoining a literal split loses the comment/newline boundary, while direct
/// exec leaves the tilde, environment reference, glob and operators literal.
/// The exact resulting file distinguishes both regressions from real Bash.
#[farhelm_testtrace::test]
async fn command_text_keeps_bash_expansion_and_statement_boundaries() {
    let home = farhelm_teststate::tempdir().expect("owned login home");
    let h = shell_harness(home.path()).await;
    let work = farhelm_teststate::tempdir().expect("command working directory");
    for name in ["glob-1.txt", "glob-2.txt"] {
        std::fs::write(work.path().join(name), "fixture").expect("write glob input");
    }
    let command = concat!(
        "printf '%s\\n' ~ \"$HOME\" \"$COMMAND_SHELL_VALUE\" glob-*.txt > expansion.txt\n",
        "printf 'left|right\\n' | tr '|' ':' >> expansion.txt # ignored > comment-ran\n",
        "printf 'line-two\\n' >> expansion.txt\n",
    );
    let session = h
        .client
        .create_session(&work.path().to_string_lossy(), command, None, 80, 24)
        .await
        .expect("create a command with shell syntax");
    wait_for_exit_code(&h.client, &session.id, 0, 30).await;

    assert_eq!(
        std::fs::read_to_string(work.path().join("expansion.txt"))
            .expect("the command completed its output file"),
        format!(
            "{0}\n{0}\n{ENV_VALUE}\nglob-1.txt\nglob-2.txt\nleft:right\nline-two\n",
            home.path().display(),
        ),
    );
    assert!(!work.path().join("comment-ran").exists());
    assert!(!work.path().join("env-injected").exists());
}

/// Plain Bash honors explicitly configured startup input and exported functions.
///
/// Non-login/noninteractive does not suppress BASH_ENV or function import. An
/// owned outer shell exports both, so this distinguishes the selected Bash
/// contract from privileged mode or a new environment-scrubbing policy.
#[farhelm_testtrace::test]
async fn command_bash_honors_configured_startup_and_exported_functions() {
    let home = farhelm_teststate::tempdir().unwrap();
    let h = shell_harness(home.path()).await;
    let startup = home.path().join("bash-env");
    std::fs::write(&startup, "COMMAND_SHELL_STARTUP='owned startup'\n").unwrap();
    std::fs::write(
        home.path().join("login-shell"),
        format!(
            "#!/bin/sh\nexport BASH_ENV={}\nexec /bin/bash --noprofile --norc -i -c \
             'command_shell_exported() {{ printf imported; }}; export -f command_shell_exported; '\"$4\"\n",
            shell_quote(startup.to_str().unwrap()),
        ),
    ).unwrap();
    let work = farhelm_teststate::tempdir().unwrap();
    let session = h.client.create_session(
        work.path().to_str().unwrap(),
        "command_shell_exported > imported.txt; printf '%s' \"$COMMAND_SHELL_STARTUP\" > startup.txt",
        None, 80, 24,
    ).await.unwrap();
    wait_for_exit_code(&h.client, &session.id, 0, 30).await;
    assert_eq!(
        std::fs::read_to_string(work.path().join("imported.txt")).unwrap(),
        "imported"
    );
    assert_eq!(
        std::fs::read_to_string(work.path().join("startup.txt")).unwrap(),
        "owned startup"
    );
}

/// A filled cwd is one literal argument even when its name is shell syntax.
///
/// The embedded reserved-marker text also must not become a second expansion
/// pass. Checking the complete output proves preservation; the side-effect
/// witness separately catches command substitution escaping the quoting.
#[farhelm_testtrace::test]
async fn command_cwd_placeholder_does_not_execute_or_expand_its_value() {
    let home = farhelm_teststate::tempdir().expect("owned login home");
    let h = shell_harness(home.path()).await;
    let parent = farhelm_teststate::tempdir().expect("working-directory parent");
    let work = parent
        .path()
        .join("work ' ; $(touch cwd-injected) {conversation} $HOME");
    std::fs::create_dir(&work).expect("create a shell-sensitive working directory");
    let session = h
        .client
        .create_session(
            &work.to_string_lossy(),
            "printf '%s\\n' {cwd} > cwd.txt",
            None,
            80,
            24,
        )
        .await
        .expect("create a command with a cwd placeholder");
    wait_for_exit_code(&h.client, &session.id, 0, 30).await;

    assert_eq!(
        std::fs::read_to_string(work.join("cwd.txt")).expect("read the filled cwd"),
        format!("{}\n", work.display()),
    );
    assert!(!work.join("cwd-injected").exists());
}

/// Bash finding no inner program is an ordinary exited command, not a shim error.
///
/// Reconcile before checking status so an unread sentinel cannot masquerade
/// as a plain dead pane. The retained terminal must carry Bash's diagnostic,
/// while the separate direct-agent tests keep proving exec-failure sentinels.
#[farhelm_testtrace::test]
async fn a_missing_command_program_exits_127_without_an_exec_failure_sentinel() {
    let home = farhelm_teststate::tempdir().expect("owned login home");
    let h = shell_harness(home.path()).await;
    let work = farhelm_teststate::tempdir().expect("command working directory");
    let missing = work.path().join("no-such-shell-program");
    assert!(!missing.exists(), "fixture program must be absent");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &shell_quote(&missing.to_string_lossy()),
            None,
            80,
            24,
        )
        .await
        .expect("Bash starts even though its inner program is absent");
    wait_for_dead_pane(
        &h.state.path().join("tmux.sock"),
        &format!("fh-{}", session.id),
    )
    .await;
    h.sup.reconcile_for_test().await;
    let ended = wait_for_exit_code(&h.client, &session.id, 127, 30).await;
    assert!(matches!(ended.status, SessionStatus::Exited { .. }));

    let (_channel, mut terminal, mut live) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("the failed command's terminal remains viewable");
    wait_for(&mut live, &mut terminal, "no-such-shell-program", 20).await;
    let text = String::from_utf8_lossy(&terminal);
    assert!(text.contains("bash:"), "expected Bash's diagnostic: {text}");
    assert!(!text.contains("exec_failed"), "{text}");
    assert!(
        !status_path_for_spec(&spec_path_for_launch(h.state.path(), &session.id, 0)).exists(),
        "the shim execed Bash successfully and must leave no failure sentinel",
    );
}

/// Restart and Restart with each execute the selected resume source through Bash.
///
/// Distinct generation markers prevent old scrollback from satisfying a new
/// launch. The replacement source has the same argument count as the original:
/// using the old source with newly filled argv would otherwise look valid.
/// Goose's plain captured id keeps this about shell execution rather than a
/// vendor's on-disk locator verification.
#[farhelm_testtrace::test]
async fn restart_and_restart_with_use_their_command_resume_source() {
    let home = farhelm_teststate::tempdir().expect("owned login home");
    let h = shell_harness(home.path()).await;
    let work = farhelm_teststate::tempdir().expect("command working directory");
    let line = shell_quote(&format!(
        "exec {}",
        fixture_cmd("fake-agent --script basic")
    ));
    let command = format!("sh -c {line} {{farhelm_args}}");
    let resume = |tag: &str| {
        format!(
            "printf '%s\\n' {tag} \"$COMMAND_SHELL_VALUE\" {{conversation}} {{cwd}} > {tag}.txt\n\
             printf 'COMMAND-SHELL-{tag}\\n'\n\
             exec sh -c {line} {{farhelm_args}}"
        )
    };
    let original_resume = resume("original");
    let original = declared_command(
        &command,
        farhelm_proto::LaunchHarness::Goose,
        Some(&original_resume),
    );
    let session = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            original,
            None,
            80,
            24,
            farhelm_helm::CreateExtras::default(),
        )
        .await
        .expect("create a resumable command");
    wait_for_agent_ready(&h.state.path().join("tmux.sock"), &session.id).await;
    let conversation = "conv;$HOME*$(touch${IFS}conversation-injected)";
    h.sup
        .record_conversation_for_test(&session.id, conversation)
        .await;
    let ready = wait_for_live_status(&h.client, &session.id, 30).await;
    assert_eq!(ready.restart_offer, farhelm_proto::RestartOffer::Resume);

    for tag in ["original", "replacement"] {
        let replacement_resume = resume(tag);
        let replacement = (tag == "replacement").then(|| {
            declared_command(
                &command,
                farhelm_proto::LaunchHarness::Goose,
                Some(&replacement_resume),
            )
        });
        let restarted = h
            .client
            .restart_session_with(&session.id, true, replacement.clone(), None)
            .await
            .expect("restart through the selected command resume source");
        if let Some(replacement) = replacement {
            assert_eq!(restarted.launch, replacement);
        }
        let (channel, mut terminal, mut live) = h
            .client
            .attach_live(&session.id, 80, 24)
            .await
            .expect("attach to the restarted command");
        wait_for(
            &mut live,
            &mut terminal,
            &format!("COMMAND-SHELL-{tag}"),
            20,
        )
        .await;
        assert_eq!(
            std::fs::read_to_string(work.path().join(format!("{tag}.txt")))
                .expect("the selected resume source wrote its output"),
            format!(
                "{tag}\n{ENV_VALUE}\n{conversation}\n{}\n",
                work.path().display()
            ),
        );
        assert!(!work.path().join("conversation-injected").exists());
        assert!(!work.path().join("env-injected").exists());
        wait_for_live_status(&h.client, &session.id, 30).await;
        h.client.detach(channel).await;
    }
}
