//! Restart of a Claude session whose report named a transcript: the resume command when Claude saved the
//! conversation, the start command when it saved nothing (SPEC.md, Restart).
//!
//! Each test drives the public Restart path on a supervisor with real terminal hosts and reads the launch spec the
//! supervisor published for the relaunched generation, which is what the launch shim would execute. The shim itself is
//! a stand-in that never consumes the spec: a `sleep` that keeps the old agent alive, so Restart takes its ordinary
//! stop path. A save made after the stop is placed through the supervisor's `restart_stopped` seam, which runs once the
//! old agent is gone and before the check.

use super::*;
use crate::launch::LaunchSpec;

/// How the transcript Claude reported stands when the Restart runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Transcript {
    /// Never written: Claude got no prompt before the Restart.
    Missing,
    /// A regular file: Claude saved the conversation.
    Saved,
    /// A directory where the file should be: neither missing nor a transcript.
    NotAFile,
    /// Missing when the Restart begins, and written once the old agent has stopped.
    SavedWhileStopping,
    /// None recorded: a bare id, as an older build stored it.
    NotRecorded,
}

/// The kind of launch the session was created with.
#[derive(Clone, Copy, Debug)]
enum Launch {
    /// A command launch declaring Claude, whose start and resume command texts differ.
    Command,
    /// An agent launch, whose start and resume argv differ.
    Agent,
}

/// The conversation id the fixture's Claude reported.
const ID: &str = "conv-1";

/// An initial prompt word the session is created with, which a Restart must never send again.
const PROMPT: &str = "initial-prompt-marker";

/// Which Restart the fixture runs.
enum How {
    /// A plain Restart.
    Plain,
    /// Restart with this replacement launch.
    With(Box<SessionLaunch>),
    /// Restart with the same launch and a named conversation, `named-target`.
    Named,
}

/// The launch spec the supervisor published for `generation` of session `id`.
fn spec_of(state: &StateDir, id: &str, generation: i64) -> LaunchSpec {
    let spec_path = crate::launch::spec_path_for_launch(state.path(), id, generation);
    serde_json::from_slice(&std::fs::read(&spec_path).expect("the spec is still on disk"))
        .expect("the spec parses")
}

/// The session's launch: start and resume commands that say which one ran, as literal words the spec carries.
fn claude_launch(launch: Launch) -> SessionLaunch {
    match launch {
        Launch::Command => declared_command(
            "echo ran-start {farhelm_args}",
            LaunchHarness::Claude,
            Some("echo ran-resume {conversation} {farhelm_args}"),
        ),
        Launch::Agent => SessionLaunch::Agent {
            custom_reporting: None,
            selection: farhelm_proto::LaunchSelection {
                custom_agent: None,
                harness: LaunchHarness::Claude,
                model: None,
                effort: None,
                permissions: None,
                wrapper_prefix: None,
                container: None,
                workspace_trust: None,
                permission_flags: None,
            },
            start: vec!["ran-start".into(), "{farhelm_args}".into()],
            resume: Some(vec![
                "ran-resume".into(),
                "--resume".into(),
                "{conversation}".into(),
                "{farhelm_args}".into(),
            ]),
        },
    }
}

/// Whether the published spec runs the start command (`true`) or the resume command of [`ID`] (`false`). Panics on
/// anything else, so a spec that runs neither cannot pass as either.
fn ran_start(spec: &LaunchSpec) -> bool {
    let words = spec.argv.join(" ");
    let start = words.contains("ran-start");
    let resume = words.contains("ran-resume") && words.contains(ID);
    assert!(
        start != resume,
        "the spec runs exactly one of the commands: {:?}",
        spec.argv
    );
    start
}

/// Create a Claude session whose old agent is alive (with `prompt` as its initial prompt, if any), record a capture
/// of [`ID`] for it as `transcript` says, run the Restart `how` says, and return the spec the relaunched generation
/// published, with the session's id.
async fn restart_claude(
    state: &StateDir,
    transcript: Transcript,
    launch: Launch,
    how: How,
    prompt: Option<&str>,
) -> (Arc<Supervisor>, String, LaunchSpec) {
    let path = state.path().join("projects").join(format!("{ID}.jsonl"));
    std::fs::create_dir_all(path.parent().unwrap()).expect("the transcript's directory");
    match transcript {
        Transcript::Saved => std::fs::write(&path, b"{}\n").expect("write the transcript"),
        Transcript::NotAFile => {
            std::fs::create_dir(&path).expect("a directory where the file goes")
        }
        Transcript::Missing | Transcript::SavedWhileStopping | Transcript::NotRecorded => {}
    }
    // The old agent: alive until stopped, so Restart stops it the ordinary way.
    let shim = state.path().join("old-agent");
    std::fs::write(&shim, "#!/bin/sh\nexec sleep 300\n").expect("write the old agent stand-in");
    std::fs::set_permissions(&shim, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("make the old agent stand-in executable");
    let mut seams = host_seams();
    // For a save after the stop, the transcript appears once the old agent is gone: only a check made after the stop
    // can see it. The seam also records whether the old agent's process is really gone when it runs, so the test pins
    // the seam's place, not only the order of seam and check.
    let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let old_agent = Arc::new(std::sync::atomic::AtomicU32::new(0));
    if transcript == Transcript::SavedWhileStopping {
        let (path, stopped, old_agent) = (path.clone(), stopped.clone(), old_agent.clone());
        seams.faults.restart_stopped = Some(Arc::new(move || {
            let pid = old_agent.load(std::sync::atomic::Ordering::SeqCst);
            // SAFETY: signal 0 only checks that the process exists; it sends nothing.
            let alive = pid != 0 && unsafe { libc::kill(pid as libc::pid_t, 0) } == 0;
            std::fs::write(&path, b"{}\n").expect("save the transcript after the stop");
            stopped.store(pid != 0 && !alive, std::sync::atomic::Ordering::SeqCst);
        }));
    }
    let sup = Supervisor::new_with_seams(state.path(), shim, SupervisorTimeouts::default(), seams)
        .await
        .expect("supervisor");
    let work = state.path().join("work");
    std::fs::create_dir(&work).expect("the session's directory");
    let created = sup
        .create_session(
            CreateInputs {
                setup_persistent_hooks: false,
                prompt: prompt.map(str::to_string),
                named_start: None,
                github_checkout: None,
                cwd: work.to_str().expect("UTF-8 fixture path"),
                parent: None,
                launch: claude_launch(launch),
                title: None,
                cols: 80,
                rows: 24,
                extra_instructions: None,
            },
            None,
        )
        .await
        .expect("create the Claude session");
    let captured = match transcript {
        Transcript::NotRecorded => ID.to_string(),
        _ => crate::agent_kind::claude::stored_conversation(
            ID,
            Some(&serde_json::json!(
                path.to_str().expect("UTF-8 fixture path")
            )),
        ),
    };
    assert_eq!(
        captured.starts_with(farhelm_proto::conversation::CLAUDE_LOCATOR_PREFIX),
        transcript != Transcript::NotRecorded,
        "the fixture premise: the capture carries a transcript exactly when one was recorded"
    );
    sup.record_conversation_for_test(&created.id, &captured)
        .await;
    let ready = sup.session_snapshot(&created.id).await.unwrap().unwrap();
    assert_eq!(ready.restart_offer, RestartOffer::Resume);
    let entry = sup.sessions.lock().await[&created.id].clone();
    let live = entry
        .terminal
        .as_ref()
        .expect("an agent terminal")
        .probe()
        .expect("the agent probe")
        .live_pid()
        .expect("the fixture premise: the old agent runs, so Restart has to stop it");
    old_agent.store(live, std::sync::atomic::Ordering::SeqCst);
    if let Some(prompt) = prompt {
        assert!(
            spec_of(state, &created.id, 0)
                .argv
                .iter()
                .any(|word| word == prompt),
            "the fixture premise: the first launch carried the initial prompt"
        );
    }
    assert_eq!(
        path.exists(),
        matches!(transcript, Transcript::Saved | Transcript::NotAFile),
        "the fixture premise: the transcript's state before the Restart"
    );

    match how {
        How::Plain => sup.restart_session(&created.id, true, None, None).await,
        How::With(replacement) => {
            sup.restart_session(&created.id, true, Some(*replacement), None)
                .await
        }
        How::Named => {
            let launch = claude_launch(launch);
            let named = farhelm_proto::NamedConversationStart {
                conversation: "named-target".into(),
                argv: IntegrationSnapshot::of(&launch)
                    .filled_named_resume_argv("named-target")
                    .expect("the named resume argv"),
            };
            sup.restart_session_named(
                &created.id,
                true,
                Some(launch),
                None,
                Some(named),
                Default::default(),
                None,
            )
            .await
        }
    }
    .expect("the Restart");
    if transcript == Transcript::SavedWhileStopping {
        assert!(
            stopped.load(std::sync::atomic::Ordering::SeqCst) && path.is_file(),
            "the fixture premise: the transcript was saved after the old agent's process was gone"
        );
    }
    let generation = sup
        .store
        .session(&created.id)
        .await
        .unwrap()
        .unwrap()
        .generation;
    assert_eq!(generation, 1, "the Restart relaunched once");
    let spec = spec_of(state, &created.id, generation);
    (sup, created.id, spec)
}

/// The bug this fixes: Claude reports its id at launch but saves nothing until the first prompt, so resuming before
/// that fails with "No conversation found" and leaves an empty terminal. Specified, for a command launch and an agent
/// launch: with the reported transcript missing, a plain Restart runs the session's start command, and the capture is
/// kept, so Restart still offers Resume and checks again next time. The agent launch is created with an initial
/// prompt, which the fresh start must not send again: a prompt is never stored, and a Restart passes none.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn a_restart_before_claude_saved_anything_runs_the_start_command() {
    for (launch, prompt) in [(Launch::Command, None), (Launch::Agent, Some(PROMPT))] {
        let state = StateDir::new();
        let (sup, id, spec) =
            restart_claude(&state, Transcript::Missing, launch, How::Plain, prompt).await;
        assert!(ran_start(&spec), "{launch:?}: {:?}", spec.argv);
        assert!(
            !spec.argv.iter().any(|word| word.contains(PROMPT)),
            "{launch:?}: the fresh start sends no initial prompt: {:?}",
            spec.argv
        );
        let row = sup.store.session(&id).await.unwrap().unwrap();
        assert!(
            row.captured_conversation.as_deref().is_some_and(
                |stored| stored.starts_with(farhelm_proto::conversation::CLAUDE_LOCATOR_PREFIX)
            ),
            "{launch:?}: the capture is kept: {:?}",
            row.captured_conversation
        );
        assert_eq!(
            sup.session_snapshot(&id)
                .await
                .unwrap()
                .unwrap()
                .restart_offer,
            RestartOffer::Resume,
            "{launch:?}"
        );
    }
}

/// A saved conversation must resume exactly as before; the check may only ever add a fresh start where resuming
/// would have failed. Specified: with the transcript a regular file, the resume command of the reported id runs.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn a_restart_after_claude_saved_its_conversation_resumes_it() {
    let state = StateDir::new();
    let (_, _, spec) =
        restart_claude(&state, Transcript::Saved, Launch::Command, How::Plain, None).await;
    assert!(!ran_start(&spec), "{:?}", spec.argv);
}

/// Only a missing file means "nothing saved": an unreadable answer is not evidence that the conversation is gone, and
/// starting over on one would lose a conversation a resume could still reopen. Specified: with a directory where the
/// transcript should be, the resume command runs.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn a_transcript_that_is_not_a_file_still_resumes() {
    let state = StateDir::new();
    let (_, _, spec) = restart_claude(
        &state,
        Transcript::NotAFile,
        Launch::Command,
        How::Plain,
        None,
    )
    .await;
    assert!(!ran_start(&spec), "{:?}", spec.argv);
}

/// The choice is made once the old agent has stopped, so a prompt Claude saved at the last moment is resumed rather
/// than lost to a check made too early. Specified: with the transcript missing when the Restart begins and written
/// only once the old agent is gone (before the check), the resume command runs.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn a_transcript_saved_while_the_old_agent_stops_is_resumed() {
    let state = StateDir::new();
    let (_, _, spec) = restart_claude(
        &state,
        Transcript::SavedWhileStopping,
        Launch::Command,
        How::Plain,
        None,
    )
    .await;
    assert!(!ran_start(&spec), "{:?}", spec.argv);
}

/// A capture an older build stored has no transcript to check, and must keep resuming as it always did. Specified:
/// with a bare id and no file anywhere, the resume command runs.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn a_bare_id_from_an_older_build_resumes_unchecked() {
    let state = StateDir::new();
    let (_, _, spec) = restart_claude(
        &state,
        Transcript::NotRecorded,
        Launch::Command,
        How::Plain,
        None,
    )
    .await;
    assert!(!ran_start(&spec), "{:?}", spec.argv);
}

/// Restart with runs the replacement launch, so a fresh start must run the replacement's start command, not the old
/// launch's. Specified: with the transcript missing, Restart with a command launch whose start command differs runs
/// that new start command.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn restart_with_starts_the_replacement_when_nothing_was_saved() {
    let state = StateDir::new();
    let replacement = declared_command(
        "echo ran-start-replacement {farhelm_args}",
        LaunchHarness::Claude,
        Some("echo ran-resume {conversation} {farhelm_args}"),
    );
    let (_, _, spec) = restart_claude(
        &state,
        Transcript::Missing,
        Launch::Command,
        How::With(Box::new(replacement)),
        None,
    )
    .await;
    assert!(ran_start(&spec), "{:?}", spec.argv);
    assert!(
        spec.argv.join(" ").contains("ran-start-replacement"),
        "the replacement's start command runs: {:?}",
        spec.argv
    );
}

/// A named Restart runs the conversation the user chose, so whether Claude saved the old one is beside the point and
/// must not turn it into a fresh start. Specified: with the transcript of the old capture missing, a Restart with a
/// named conversation runs the resume command of that named conversation.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn a_named_restart_runs_its_conversation_whatever_was_saved() {
    let state = StateDir::new();
    let (_, _, spec) =
        restart_claude(&state, Transcript::Missing, Launch::Agent, How::Named, None).await;
    let words = spec.argv.join(" ");
    assert!(
        words.contains("ran-resume")
            && words.contains("named-target")
            && !words.contains("ran-start"),
        "the named conversation's resume runs: {:?}",
        spec.argv
    );
}
