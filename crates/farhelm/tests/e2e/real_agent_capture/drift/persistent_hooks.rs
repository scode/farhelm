//! Farhelm's opt-in persistent hook files, checked against the real CLIs that read them.
//!
//! Grok and Antigravity (every agent for which `AgentKind::offers_persistent_hooks` is true) find Farhelm's
//! conversation hooks in files in their own configuration, which Farhelm writes on a launch the user consented to. Those
//! files stay behind, so every later run of the agent runs them, inside Farhelm or not. Each case checks both sides
//! against one private mirror of the vendor's configuration (`private_config`), never the maintainer's real files:
//!
//! 1. Inside Farhelm: a consented launch installs exactly the files Farhelm's setup plans into the mirror
//!    (`DriftEnv::with_session` checks that), and the real CLI finds and runs them, so one short turn gets an accepted
//!    conversation report.
//! 2. Outside Farhelm: the same CLI, started the way a user would start it, with none of Farhelm's environment and the
//!    mirror's variable set, reaches its composer and completes two short turns with no hook output on screen and the
//!    CLI still running. The second turn is the boundary for the first one's Stop hook: the CLI took new input after
//!    it, so a Stop hook that hung or failed loudly would show. With Farhelm's variables gone the hook has nowhere to
//!    report and writes nothing, so there is no positive evidence that it ran outside; the first phase, on the same
//!    mirror, is the evidence that the CLI discovers the files.
//!
//! What it does not show. The leak check knows Farhelm's own hook output and Antigravity's protocol reply
//! (`hook_invisible`), not a vendor's own wording for a failed hook. The first phase assumes the accepted report came
//! from the installed files, not from a manual Farhelm hook the mirror links to in the vendor's real configuration.
//! And Grok's outside run uses its private backend (`outside_argv`), so Grok's default shared leader, which runs the
//! hooks in whatever environment it started with, is not covered.
//!
//! The outside run is a plain command session (`DriftEnv::with_plain_session`), so the test's own terminal host,
//! attachment and Delete still own it. Its command removes every `FARHELM_*` variable before the CLI starts, so
//! Delete's sweep of descendants by `FARHELM_SESSION_ID` finds nothing, and only the launch's systemd scope keeps a
//! descendant that left the CLI's process tree within Delete's reach. The case therefore requires the CLI to run in
//! that scope before it types anything, and reports COULD NOT RUN without one (no systemd user manager).

use std::os::unix::ffi::OsStrExt as _;

use super::*;

/// How long the outside CLI is watched after its last reply, so that the CLI exiting just after the turn (a Stop hook
/// that made it quit, say) fails the case rather than passing at the reply. It bounds the observation only: the second
/// turn, not this window, is what shows the first turn's Stop hook let the CLI go on.
const OUTSIDE_SETTLE: Duration = Duration::from_secs(5);

/// How long the outside session's process may take to become the CLI. It is a few `exec`s after the login shell's
/// startup, normally well under a second; a short limit keeps the case inside the runner's per-case budget.
const OUTSIDE_EXEC_LIMIT: Duration = Duration::from_secs(30);

/// Both phases share one supervisor, and with it one mirror, so the outside run reads the files the consented launch
/// installed. The installed files and the untouched real ones are checked again between the phases and after them.
async fn execute(harness: LaunchHarness) -> DriftResult {
    let profile = profile(harness).expect("a persistent hook case has a drift profile");
    let argv =
        outside_argv(harness, &profile).expect("a persistent hook case has an outside command");
    with_drift_supervisor(
        &profile,
        ordinary_selection(harness, &profile),
        None,
        async |env, launch| {
            let private = env.private.ok_or_else(|| {
                anyhow::anyhow!("no private configuration for an agent with persistent hooks")
            })?;
            env.with_session(launch, farhelm_helm::CreateExtras::default(), async |id| {
                consented_turn(&env, harness, &profile, id).await
            })
            .await?;
            // Deleting the consented session must leave the installed files for the agent's later runs.
            private.check_hook_setup(true)?;
            let redirects: Vec<_> = private.redirects().collect();
            let command = outside_command(&argv, &redirects)?;
            env.with_plain_session(command, async |id| {
                outside_turn(&env, harness, &profile, &argv, &redirects, id).await
            })
            .await?;
            private.check_hook_setup(true)?;
            Ok(())
        },
    )
    .await
}

/// The first phase: the consented launch's CLI runs the hooks Farhelm just installed, shown by an accepted,
/// resumable conversation report for a turn whose reply is on screen, with no hook output drawn.
async fn consented_turn(
    env: &DriftEnv<'_>,
    harness: LaunchHarness,
    profile: &Harness,
    id: &str,
) -> DriftResult {
    let hook_witness = profile
        .hook_witness
        .expect("an agent with persistent hooks reports its conversation");
    let mut term = Terminal::attach(env.sup, env.client, id, false).await?;
    term.ready(profile, TrustPolicy::AutoAnswer, false).await?;
    let (prompt, reply) = split_nonce_prompt();
    send_prompt(&mut term, &prompt, &reply, false).await?;
    let log = env.state.join("hook-log").join(format!("{id}.log"));
    wait_for_turn(
        &mut term,
        env.sup,
        id,
        &log,
        &TurnExpectation {
            reply: &reply,
            acked_before: 0,
            original_argv: None,
            hook_witness,
        },
    )
    .await?;
    hook_invisible(harness, &term)?;
    println!("the consented launch's hooks reported the conversation");
    Ok(())
}

/// The second phase: the CLI outside Farhelm. Before any key is sent (answering a trust question is one), the
/// session's process must be the vendor CLI with exactly the outside command's arguments, in the session's launch
/// scope, carrying no `FARHELM_*` variable and every mirror variable. It then has to finish two turns and stay up
/// through the settle window without drawing hook output. A login screen here is a failure: the first phase already
/// used the same login.
async fn outside_turn(
    env: &DriftEnv<'_>,
    harness: LaunchHarness,
    profile: &Harness,
    argv: &[String],
    redirects: &[(&str, &Path)],
    id: &str,
) -> DriftResult {
    let mut term = Terminal::attach(env.sup, env.client, id, true).await?;
    wait_for_outside_cli(&mut term, argv).await?;
    check_outside_scope(env, &term, id).await?;
    check_outside_environ(&read_child_proc(&term, "environ")?, redirects)?;
    term.ready(profile, TrustPolicy::AutoAnswer, true).await?;
    hook_invisible(harness, &term)?;
    untracked_turn(&mut term, true).await?;
    hook_invisible(harness, &term)?;
    untracked_turn(&mut term, true).await?;
    let settled = tokio::time::Instant::now() + OUTSIDE_SETTLE;
    while tokio::time::Instant::now() < settled {
        term.check(&term.host.screen_tail(SCREEN_BYTES).to_lowercase(), true)
            .await?;
        pace().await;
    }
    hook_invisible(harness, &term)?;
    println!("the CLI outside Farhelm finished two turns with the installed hooks silent");
    Ok(())
}

/// The session's process passes through the login shell and Farhelm's launch shim before the outside command's
/// `exec` makes it the vendor CLI; wait until its own argv is exactly the outside argv, failing early if the session
/// ends. Both CLIs are native executables, whose command line is the argv the command passed.
async fn wait_for_outside_cli(term: &mut Terminal<'_>, argv: &[String]) -> DriftResult {
    let deadline = tokio::time::Instant::now() + OUTSIDE_EXEC_LIMIT;
    loop {
        term.check(&term.host.screen_tail(SCREEN_BYTES).to_lowercase(), true)
            .await?;
        let cmdline = read_child_proc(term, "cmdline")?;
        if is_argv(&cmdline, argv) {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            // The command line stays out of the message: before the exec it is Farhelm's launch command.
            return Err(anyhow::anyhow!(
                "the session's process never became the outside command {argv:?}\n{}",
                term.diagnostics()
            )
            .into());
        }
        pace().await;
    }
}

/// `/proc/<pid>/cmdline` (each argument NUL-terminated) holds exactly `argv`.
fn is_argv(cmdline: &[u8], argv: &[String]) -> bool {
    let words: Vec<&[u8]> = cmdline
        .strip_suffix(b"\0")
        .unwrap_or(cmdline)
        .split(|byte| *byte == 0)
        .collect();
    words.len() == argv.len()
        && words
            .iter()
            .zip(argv)
            .all(|(word, arg)| *word == arg.as_bytes())
}

/// The outside CLI runs in its launch's systemd scope. Its command strips the session marker that Delete's fallback
/// sweep looks for, so without the scope a descendant that left the CLI's process tree would survive Delete, and the
/// mirror's copy-back would run while it may still write through the mirror's links. No scope is an unavailable
/// prerequisite (no systemd user manager), not a failure.
async fn check_outside_scope(env: &DriftEnv<'_>, term: &Terminal<'_>, id: &str) -> DriftResult {
    let snapshot = env
        .sup
        .session_snapshot(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("the outside session vanished"))?;
    let unit = farhelm_supervisor::scope::unit_name(id, snapshot.generation)
        .ok_or_else(|| anyhow::anyhow!("the session id {id} has no launch scope name"))?;
    if !in_unit(&read_child_proc(term, "cgroup")?, &unit) {
        return Err(DriftError::CouldNotRun(format!(
            "the CLI outside Farhelm is not in its launch scope {unit}; without a systemd user manager, Delete could \
             not reach a descendant of a CLI run without Farhelm's session marker"
        )));
    }
    Ok(())
}

/// `/proc/<pid>/cgroup` places the process in `unit` or a cgroup below it.
fn in_unit(cgroup: &[u8], unit: &str) -> bool {
    String::from_utf8_lossy(cgroup).lines().any(|line| {
        line.splitn(3, ':')
            .nth(2)
            .is_some_and(|path| path.split('/').any(|part| part == unit))
    })
}

/// The CLI as a user starts it outside Farhelm: the profile's binary with its model and effort spelled the vendor's
/// way, and nothing of Farhelm's. `None` for an agent without persistent hooks, which has no case here. Each arm spells
/// every flag its agent gets, the way Farhelm's own launch does: Grok takes no model or effort from Farhelm.
///
/// Grok also gets `--no-leader`, which a user need not pass, so the case covers Grok's private backend running the
/// hooks, not its default shared leader. Its default leader listens at `~/.grok/leader.sock`, and the mirror links every
/// entry of the real directory, so the test could reach a leader the maintainer runs, one that never ran the installed
/// files and may carry another environment.
#[warn(clippy::wildcard_enum_match_arm)]
fn outside_argv(harness: LaunchHarness, profile: &Harness) -> Option<Vec<String>> {
    use LaunchHarness::*;
    let mut argv = vec![profile.binary.to_owned()];
    match harness {
        Grok => argv.push("--no-leader".into()),
        Antigravity => {
            if let Some(effort) = profile.effort {
                argv.extend(["--effort".into(), effort.as_cli_arg().into()]);
            }
            if let Some(model) = profile.model {
                argv.extend(["--model".into(), model.into()]);
            }
        }
        Claude | Codex | Goose | Muse | Pi | Omp | Cursor | OpenCode | Bash | Zsh | Custom => {
            return None;
        }
    }
    Some(argv)
}

/// The command text for the outside session. Farhelm's launch shim runs a command launch's text with Bash after the
/// login shell's startup files, so this text is the last word on the environment: it unsets every exported
/// `FARHELM_*` variable whatever its name, sets each mirror variable again (a startup file could have changed it), and
/// `exec`s the CLI so that the session's process becomes the CLI itself.
fn outside_command(argv: &[String], redirects: &[(&str, &Path)]) -> anyhow::Result<String> {
    let mut command = String::from("unset -v $(compgen -e FARHELM_)");
    for (variable, path) in redirects {
        let path = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("the {variable} mirror path is not UTF-8"))?;
        command.push_str(&format!("; export {variable}={}", shell_words::quote(path)));
    }
    command.push_str("; exec ");
    command.push_str(&shell_words::join(argv));
    Ok(command)
}

/// The outside CLI's environment (`/proc/<pid>/environ`, NUL-separated) holds no `FARHELM_*` entry and exactly one
/// entry for each mirror variable, naming the mirror. Errors name variables, never their values, which can hold the
/// session's spawn credential.
fn check_outside_environ(environ: &[u8], redirects: &[(&str, &Path)]) -> anyhow::Result<()> {
    let entries: Vec<&[u8]> = environ
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .collect();
    let leaked: Vec<_> = entries
        .iter()
        .filter(|entry| entry.starts_with(b"FARHELM_"))
        .map(|entry| {
            let name = entry.split(|byte| *byte == b'=').next().unwrap_or_default();
            String::from_utf8_lossy(name).into_owned()
        })
        .collect();
    anyhow::ensure!(
        leaked.is_empty(),
        "the CLI outside Farhelm still has Farhelm's variables {leaked:?}"
    );
    for (variable, path) in redirects {
        let prefix = format!("{variable}=");
        let values: Vec<&[u8]> = entries
            .iter()
            .filter_map(|entry| entry.strip_prefix(prefix.as_bytes()))
            .collect();
        anyhow::ensure!(
            values == [path.as_os_str().as_bytes()],
            "the CLI outside Farhelm does not read {variable} from the mirror"
        );
    }
    Ok(())
}

drift_cases! {
    execute;
    drift_grok => Grok,
    drift_antigravity => Antigravity,
}

/// An agent that gains persistent hook files leaves files behind that its plain runs execute, so it needs a case here
/// as well as the private mirror. Specified: the category covers exactly the built-in agents that offer persistent
/// hooks, and each has an outside command.
#[farhelm_testtrace::test]
fn the_category_covers_every_agent_with_persistent_hooks() {
    let expected: Vec<_> = LaunchHarness::BUILT_INS
        .iter()
        .copied()
        .filter(|harness| harness.agent_kind().offers_persistent_hooks())
        .collect();
    assert_eq!(HARNESSES.len(), expected.len(), "{HARNESSES:?}");
    for harness in expected {
        assert!(
            HARNESSES.contains(&harness),
            "missing persistent hook case for {harness:?}"
        );
        let profile = profile(harness).expect("a persistent hook agent has a drift profile");
        assert!(
            outside_argv(harness, &profile).is_some(),
            "{harness:?} has no outside command"
        );
    }
}

/// The outside command is the one place that keeps the outside run from reporting to a supervisor: a `FARHELM_*`
/// variable that survived it would make the installed hook report to whichever supervisor it names, possibly the
/// maintainer's own. Specified: run by Bash with Farhelm's variables and the real mirror variable in its environment,
/// the command starts its argv with no `FARHELM_*` variable left and the mirror variable set to the mirror, even for
/// a path that needs quoting; the result passes the same environment check the live case applies.
#[farhelm_testtrace::test]
fn the_outside_command_strips_farhelm_and_pins_the_mirror() {
    let mirror = Path::new("/scratch/private grok's home");
    let redirects = [("GROK_HOME", mirror)];
    let argv = ["env".to_owned(), "-0".to_owned()];
    let command = outside_command(&argv, &redirects).unwrap();
    let output = std::process::Command::new("bash")
        .args(["-c", &command])
        .env("FARHELM_SESSION_ID", "session-under-test")
        .env("FARHELM_SESSION_TOKEN", "token-under-test")
        .env("GROK_HOME", "/real/grok")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    check_outside_environ(&output.stdout, &redirects).unwrap();
    assert!(
        output
            .stdout
            .split(|byte| *byte == 0)
            .any(|entry| entry.starts_with(b"PATH=")),
        "the command must keep the rest of the environment"
    );
}

/// The live case trusts this check to tell an outside CLI from one still carrying Farhelm's environment or reading the
/// real configuration. Specified: a clean environment passes; a `FARHELM_*` entry, a missing, different or repeated
/// mirror entry each fail; and the error names the variable without its value.
#[farhelm_testtrace::test]
fn the_outside_environment_check_refuses_farhelm_or_another_configuration() {
    let mirror = Path::new("/scratch/private-home");
    let redirects = [("HOME", mirror)];
    let check =
        |environ: &str| check_outside_environ(environ.replace('\n', "\0").as_bytes(), &redirects);
    check("PATH=/usr/bin\nHOME=/scratch/private-home\n").unwrap();
    let leaked =
        check("HOME=/scratch/private-home\nFARHELM_SESSION_TOKEN=secret-value\n").unwrap_err();
    let leaked = format!("{leaked:#}");
    assert!(leaked.contains("FARHELM_SESSION_TOKEN"), "{leaked}");
    assert!(!leaked.contains("secret-value"), "{leaked}");
    for environ in [
        "PATH=/usr/bin\n",
        "HOME=/home/someone\n",
        "HOME=/scratch/private-home\nHOME=/home/someone\n",
    ] {
        let error = check(environ).unwrap_err();
        assert!(!format!("{error:#}").contains("someone"), "{error:#}");
    }
}

/// The outside launch is meant to be the one a user types, so its words are pinned as literals; a profile change shows
/// up here as a deliberate edit. Specified: Antigravity gets the profile's effort and model in the vendor's spelling;
/// Grok gets `--no-leader` and nothing else (see `outside_argv`); an agent without persistent hooks gets none.
#[farhelm_testtrace::test]
fn the_outside_argv_is_the_vendors_own_launch() {
    let argv = |harness| outside_argv(harness, &profile(harness).unwrap());
    assert_eq!(
        argv(LaunchHarness::Antigravity),
        Some(
            ["agy", "--effort", "low", "--model", "gemini-3.8-flash"]
                .map(String::from)
                .to_vec()
        )
    );
    assert_eq!(
        argv(LaunchHarness::Grok),
        Some(["grok", "--no-leader"].map(String::from).to_vec())
    );
    assert_eq!(argv(LaunchHarness::Claude), None);
}

/// The pre-keystroke gate must not accept a process that is still Farhelm's launch chain, or a wrapper that merely
/// mentions the CLI's words. Specified: only a command line whose arguments are exactly the argv matches.
#[farhelm_testtrace::test]
fn the_outside_cli_is_recognized_by_its_exact_argv() {
    let argv = ["grok", "--no-leader"].map(String::from);
    assert!(is_argv(b"grok\0--no-leader\0", &argv));
    for cmdline in [
        &b"grok\0"[..],
        b"grok\0--no-leader\0--extra\0",
        b"/bin/bash\0-c\0exec grok --no-leader\0",
        b"env\0grok\0--no-leader\0",
        b"",
    ] {
        assert!(
            !is_argv(cmdline, &argv),
            "{:?}",
            String::from_utf8_lossy(cmdline)
        );
    }
}

/// The scope gate decides between running the outside phase and COULD NOT RUN, so it must read the cgroup file the way
/// the kernel writes it. Specified: the unified hierarchy's line naming the unit, or a cgroup below it, matches; a
/// different session's or generation's scope, or a unit name that is only a prefix of a component, does not.
#[farhelm_testtrace::test]
fn the_launch_scope_is_read_from_the_cgroup_file() {
    let unit = "farhelm-s1-2.scope";
    let slice = "0::/user.slice/user-1000.slice/user@1000.service/app.slice";
    assert!(in_unit(
        format!("{slice}/farhelm-s1-2.scope\n").as_bytes(),
        unit
    ));
    assert!(in_unit(
        format!("{slice}/farhelm-s1-2.scope/inner\n").as_bytes(),
        unit
    ));
    for other in [
        "farhelm-s1-3.scope",
        "farhelm-s2-2.scope",
        "farhelm-s1-2.scope.d",
        "session-3.scope",
    ] {
        assert!(
            !in_unit(format!("{slice}/{other}\n").as_bytes(), unit),
            "{other}"
        );
    }
}
