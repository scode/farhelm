//! The original drift checks cover one ordinary launch per harness, each completing a real turn. The agents that
//! report a conversation also have to resume it, recall a code word and accept a fresh hook report for the same
//! identity; Cursor and OpenCode, which report none, stop after their one turn.
//!
//! Codex also has to keep the shell commands it runs inside the session's process tree (SPEC_impl.md, "Codex without
//! the shared server"): a command handed to Codex's shared background server would run outside the session, beyond
//! Stop's and Delete's reach. The base case checks that for Farhelm's default Codex launch. Two other parts of that
//! guarantee are checked elsewhere rather than repeated here. Codex's hook reports already come from inside the tree,
//! because the supervisor refuses a Codex report whose process ancestry does not reach the session's terminal, and
//! this case's tracked turns require committed reports. And Codex still starts with `--no-daemon` when hooks are off,
//! which the `capture_timing_hooks` category checks.

use super::*;
use farhelm_proto::LaunchPermission;
use farhelm_supervisor::hosts::ProcessRoot;

/// Drive Start and, for agents that report a conversation, Stop and Restart through the same supervisor; then, for
/// Codex, the process-tree probe in a second supervisor.
async fn execute(kind: LaunchHarness) -> DriftResult {
    let profile = profile(kind).expect("a drift case must name a supported agent");
    base_session(kind, &profile).await?;
    if probes_command_tree(kind) {
        command_tree_probe(kind, &profile).await?;
    }
    Ok(())
}

/// The ordinary base launch, with the profile's model, effort and trust choice.
async fn base_session(kind: LaunchHarness, profile: &Harness) -> DriftResult {
    let selection = LaunchSelection {
        custom_agent: None,
        harness: kind,
        model: profile.model.map(str::to_owned),
        effort: profile.effort.map(Into::into),
        // Exercise the two untracked agents' actual permission flags. YOLO also keeps their one turn from stopping
        // at a tool approval.
        permissions: profile
            .hook_witness
            .is_none()
            .then_some(LaunchPermission::Yolo),
        permission_flags: None,
        wrapper_prefix: None,
        container: None,
        // Codex's modal and Muse's startup redraw can lose input. Trust only the test workspace for this launch.
        workspace_trust: profile.trust_at_launch.then_some(true),
    };
    with_drift_supervisor(profile, selection, None, async |env, launch| {
        env.with_session(launch, farhelm_helm::CreateExtras::default(), async |id| {
            check_base_session(&env, profile, id).await
        })
        .await
    })
    .await
}

/// Every base case answers a workspace trust question in its empty directory, if one appears, and completes a real
/// turn under Farhelm's terminal host. An agent that reports a conversation must then prove recall and fresh hook
/// evidence on the new generation; one that reports none (Cursor, OpenCode) ends after its turn's reply is on screen.
async fn check_base_session(env: &DriftEnv<'_>, profile: &Harness, id: &str) -> DriftResult {
    let (sup, client, state) = (env.sup, env.client, env.state);
    let trust = TrustPolicy::AutoAnswer;
    let mut first = Terminal::attach(sup, client, id, false).await?;
    first.ready(profile, trust, false).await?;
    let Some(hook_witness) = profile.hook_witness else {
        return untracked_turn(&mut first).await;
    };
    let random = uuid::Uuid::new_v4().simple().to_string();
    let code_word = format!("FH{}", random[..8].to_ascii_uppercase());
    // Both turns can be answered from the conversation. A tool approval would strand the terminal on a dialog
    // that has nothing to do with whether the agent remembered the word.
    let prompt = format!(
        "Remember test label {code_word}. Reply with O and K joined into one word, and nothing else. No tools."
    );
    send_prompt(&mut first, &prompt, "OK", false).await?;
    let hook_log = state.join("hook-log").join(format!("{id}.log"));
    let captured = wait_for_turn(
        &mut first,
        sup,
        id,
        &hook_log,
        &TurnExpectation {
            reply: "OK",
            acked_before: 0,
            original_argv: None,
            hook_witness,
        },
    )
    .await?;

    // Listing observes the stopped row but does not apply reports. Drain them before taking the baseline
    // so Restart cannot acknowledge a pending Turn-1 callback and count it as a new-generation witness.
    client.stop_session(id).await?;
    client.list_sessions().await?;
    sup.reconcile_for_test().await;
    let hook_baseline = std::fs::read_to_string(&hook_log).map_err(anyhow::Error::from)?;
    let acked_before = acknowledged_reports(&hook_baseline).count();
    if acked_before == 0 {
        return Err(anyhow::anyhow!("the accepted Turn-1 hook disappeared before Restart").into());
    }
    client.restart_session(id, false).await?;
    let resumed = sup
        .session_snapshot(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Restart lost the session"))?;
    if resumed.generation <= captured.generation {
        return Err(anyhow::anyhow!("Restart did not start a new generation").into());
    }
    let mut second = Terminal::attach(sup, client, id, true).await?;
    second.ready(profile, trust, true).await?;
    let expected = TurnExpectation {
        reply: &format!("RECALL-{code_word}"),
        acked_before,
        original_argv: Some(
            captured
                .resume_argv
                .as_deref()
                .expect("accepted Turn 1 has resume argv"),
        ),
        hook_witness: match hook_witness {
            HookWitness::CommittedIdentity { .. } => HookWitness::CommittedIdentity {
                resumed_generation: Some(resumed.generation),
            },
            HookWitness::Fresh | HookWitness::MatchingIdentity => hook_witness,
        },
    };
    send_prompt(&mut second, RECALL_PROMPT, expected.reply, true).await?;
    wait_for_turn(&mut second, sup, id, &hook_log, &expected).await?;
    Ok(())
}

/// One short no-tools turn for an agent with no conversation report, proving that an interactive turn works under
/// Farhelm's terminal host and not only that the CLI starts. The answer is split in the prompt ("FH" and a random
/// nonce, joined only in the reply) so a wrapped prompt echo can never read as the answer, and the prompt is short:
/// `Terminal::type_text` waits for the whole prompt on one screen line, and OpenCode's composer wraps well before the
/// terminal's width. A login screen before the reply is still COULD NOT RUN, since no turn has succeeded yet.
async fn untracked_turn(terminal: &mut Terminal<'_>) -> DriftResult {
    let random = uuid::Uuid::new_v4().simple().to_string();
    let nonce = &random[..8];
    let reply = format!("FH{nonce}");
    let prompt = format!("Reply FH then {nonce}, no space. No tools.");
    send_prompt(terminal, &prompt, &reply, false).await?;
    wait_for_reply(terminal, &reply, false).await
}

// ---- Codex's command process tree --------------------------------------------------------------------------------

/// Whether the base case also proves that a shell command the agent runs stays inside the session's process tree.
/// Only Codex needs it: it can hand commands to a shared background server outside the session, which Farhelm's
/// launch keeps it from doing.
#[warn(clippy::wildcard_enum_match_arm)]
fn probes_command_tree(kind: LaunchHarness) -> bool {
    use LaunchHarness::*;
    match kind {
        Codex => true,
        Claude | Antigravity | Goose | Grok | Muse | Pi | Omp | Cursor | OpenCode | Bash | Zsh
        | Custom => false,
    }
}

/// The script the agent is asked to run. It walks from its own shell up to pid 1, appending each process's raw
/// `/proc/<pid>/stat` line and taking the parent from `/proc/<pid>/status` (no `ps` needed), and renames the file
/// into place at the end so that the test never reads a half-written chain. A walk cut short (an ancestor that exited
/// mid-walk) leaves a chain that does not reach pid 1, which `chain_verdict` refuses rather than misjudges.
const RECORD_TREE: &str = r#"p=$$
: > tree.txt.tmp
while [ -n "$p" ] && [ "$p" -gt 0 ]; do
  cat "/proc/$p/stat" >> tree.txt.tmp || break
  [ "$p" -eq 1 ] && break
  p=$(awk '/^PPid:/ { print $2 }' "/proc/$p/status")
done
mv tree.txt.tmp tree.txt
"#;

/// Start Codex under YOLO in its own supervisor, with Farhelm's default launch otherwise, check that its process
/// carries `--no-daemon`, have Codex run `RECORD_TREE` in its workspace through its shell mode (`run_in_shell_mode`),
/// and require that the command's ancestry reaches the session's pinned agent process directly, through no process
/// that leads a session of its own.
///
/// The session is a YOLO launch as a hedge: Codex 0.163.0 runs a `!` command without its sandbox in every permission
/// mode, but a later Codex that sandboxed it could hide or renumber `/proc` and make the recorded chain meaningless.
/// The chain is matched by pid and kernel start time, so a pid reused since the recording cannot pass for the agent.
/// The test waits for the file, never for anything Codex prints.
///
/// NOTE: This does not isolate `--no-daemon`. Farhelm's default launch also passes Codex's hook flags, and in the
/// Codex source SPEC_impl.md cites those already force the embedded mode as a side effect, so a Codex that ignored
/// `--no-daemon` would still pass here; only the argv check covers the argument. Isolating it needs a launch with
/// hooks off and no other configuration overrides (the trust and effort choices are `-c` overrides too), which in
/// turn needs Codex's own trust dialog answered; that is left as a plan follow-up in TODO.md.
async fn command_tree_probe(kind: LaunchHarness, profile: &Harness) -> DriftResult {
    let selection = LaunchSelection {
        custom_agent: None,
        harness: kind,
        model: profile.model.map(str::to_owned),
        effort: profile.effort.map(Into::into),
        permissions: Some(LaunchPermission::Yolo),
        permission_flags: None,
        wrapper_prefix: None,
        container: None,
        workspace_trust: profile.trust_at_launch.then_some(true),
    };
    with_drift_supervisor(profile, selection, None, async |env, launch| {
        env.with_session(launch, farhelm_helm::CreateExtras::default(), async |id| {
            let mut terminal = Terminal::attach(env.sup, env.client, id, false).await?;
            terminal
                .ready(profile, TrustPolicy::AutoAnswer, false)
                .await?;
            check_child_words(&terminal, "cmdline", &["--no-daemon"])?;
            let root = terminal
                .host
                .child_root()
                .ok_or_else(|| anyhow::anyhow!("the agent process is not live"))?;
            std::fs::write(env.work.join("record-tree.sh"), RECORD_TREE)
                .map_err(anyhow::Error::from)?;
            // `exec`, so that the recording shell is the very process Codex spawned (`chain_verdict` relies on it):
            // Codex starts a `!` command in a login shell that leads its own session, and a shell that did not replace
            // itself would leave that session leader between the script and the agent.
            run_in_shell_mode(&mut terminal, "exec sh ./record-tree.sh").await?;
            let chain = wait_for_chain(&mut terminal, &env.work.join("tree.txt")).await?;
            if terminal.host.child_root() != Some(root) {
                return Err(anyhow::anyhow!("the agent process changed during the probe").into());
            }
            if !chain_verdict(&chain, root)? {
                return Err(anyhow::anyhow!(
                    "the command ran outside the session's process tree: its ancestry does not reach the agent \
                     process, or reaches it through a process that leads its own session\n{chain}"
                )
                .into());
            }
            println!("the agent's shell command ran inside the session's process tree");
            Ok(())
        })
        .await
    })
    .await
}

/// Run `command` through Codex's shell mode (`!` in its composer) without a model turn: no vendor turn is spent, and
/// no model can decline it. That matters on a machine whose user instructions tell the model to do something else
/// first; Codex once refused to run the script when such a step failed. A `!` command is executed by Codex's app
/// server, the same process that runs the model's tool calls, embedded or shared, so where it runs is where Codex's
/// own commands run; it skips the sandbox and approval those take (Codex 0.163.0's source).
///
/// The `!` is sent alone and the footer's "Shell mode" awaited before the command is typed, because a `!` and the
/// command sent together left the command unsubmitted; the cue must be absent beforehand, or the wait could end at
/// once and bring that race back.
async fn run_in_shell_mode(terminal: &mut Terminal<'_>, command: &str) -> DriftResult {
    if terminal
        .host
        .screen_tail(SCREEN_BYTES)
        .to_lowercase()
        .contains("shell mode")
    {
        return Err(anyhow::anyhow!(
            "\"shell mode\" was on screen before the `!`\n{}",
            terminal.diagnostics()
        )
        .into());
    }
    terminal
        .client
        .send_keys(terminal.channel, b"!".to_vec())
        .await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let text = terminal.host.screen_tail(SCREEN_BYTES).to_lowercase();
        terminal.check(&text, false).await?;
        if text.contains("shell mode") {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "Codex did not enter its shell mode\n{}",
                terminal.diagnostics()
            )
            .into());
        }
        pace().await;
    }
    terminal.type_text(command, false).await?;
    terminal
        .client
        .send_keys(terminal.channel, b"\r".to_vec())
        .await;
    Ok(())
}

/// Wait up to TURN_LIMIT for the agent to write the recorded chain, checking the screen for refusals and an ended
/// agent on every pass.
async fn wait_for_chain(terminal: &mut Terminal<'_>, path: &Path) -> Result<String, DriftError> {
    let waited = tokio::time::timeout(TURN_LIMIT, async {
        loop {
            terminal
                .check(
                    &terminal.host.screen_tail(SCREEN_BYTES).to_lowercase(),
                    false,
                )
                .await?;
            match std::fs::read_to_string(path) {
                Ok(chain) => return Ok(chain),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(anyhow::Error::from(error).into()),
            }
            pace().await;
        }
    })
    .await;
    waited.unwrap_or_else(|_| {
        Err(anyhow::anyhow!(
            "the agent did not record its command's process tree\n{}",
            terminal.diagnostics()
        )
        .into())
    })
}

/// One process in a recorded chain: its pid, parent, session and kernel start time (`/proc/<pid>/stat` fields 1, 4,
/// 6 and 22). Pid and start time are the pair a `ProcessRoot` pins.
#[derive(Debug, PartialEq, Eq)]
struct ChainEntry {
    pid: u32,
    ppid: u32,
    session: u32,
    start: u64,
}

/// Parse one raw `/proc/<pid>/stat` line. The process name sits in parentheses and may itself hold spaces or `)`,
/// so the fields after it are found from the last `)`, and the pid is what comes before the first ` (`.
fn parse_stat_line(line: &str) -> Option<ChainEntry> {
    let (head, rest) = line.rsplit_once(')')?;
    let pid = head.split_once(" (")?.0.trim().parse().ok()?;
    // `rest` starts with the state, field 3: the parent is its 2nd token, the session its 4th, field 22 its 20th.
    let fields: Vec<&str> = rest.split_whitespace().collect();
    Some(ChainEntry {
        pid,
        ppid: fields.get(1)?.parse().ok()?,
        session: fields.get(3)?.parse().ok()?,
        start: fields.get(19)?.parse().ok()?,
    })
}

/// Whether the recorded chain shows the command inside the session: it reaches `root` (by pid and start time), and
/// no process strictly between the command's shell and `root` leads its own session. Codex starts its shared server
/// with `setsid()`, so a server it started itself would still descend from the agent; the session leader in between
/// is what tells it apart. The shell itself may lead a session, since Codex starts its commands that way.
///
/// A chain that cannot be trusted gives no verdict: a line that does not parse (it could have been the root's), or a
/// walk that does not link each process to the next as its parent and end at pid 1 is an error, never "outside".
fn chain_verdict(chain: &str, root: ProcessRoot) -> anyhow::Result<bool> {
    let entries = chain
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            parse_stat_line(line).ok_or_else(|| {
                anyhow::anyhow!("unparseable /proc stat line in the recorded chain: {line}")
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let linked = entries.windows(2).all(|pair| pair[0].ppid == pair[1].pid);
    anyhow::ensure!(
        linked && entries.last().is_some_and(|last| last.pid == 1),
        "the recorded chain is incomplete: it does not link each process to its parent up to pid 1"
    );
    let Some(at) = entries
        .iter()
        .position(|entry| entry.pid == root.pid && entry.start == root.start)
    else {
        return Ok(false);
    };
    anyhow::ensure!(
        at > 0,
        "the recorded chain starts at the agent process, not at a command it ran"
    );
    Ok(entries[1..at]
        .iter()
        .all(|entry| entry.session != entry.pid))
}

drift_cases! {
    execute;
    drift_claude => Claude,
    drift_codex => Codex,
    drift_antigravity => Antigravity,
    drift_goose => Goose,
    drift_grok => Grok,
    drift_muse => Muse,
    drift_pi => Pi,
    drift_omp => Omp,
    drift_cursor => Cursor,
    drift_opencode => OpenCode,
}

/// The base category is the default, so every built-in agent must have a case there.
#[farhelm_testtrace::test]
fn base_covers_all_builtin_agent_harnesses() {
    let mut kinds = HARNESSES.to_vec();
    kinds.extend([LaunchHarness::Bash, LaunchHarness::Zsh]);
    assert!(LaunchHarness::is_ordering_of_built_ins(&kinds));
}

/// Builds a `/proc/<pid>/stat` line with the given name, parent, session and start time, the other fields filled in.
#[cfg(test)]
fn stat_line(pid: u32, comm: &str, ppid: u32, session: u32, start: u64) -> String {
    format!(
        "{pid} ({comm}) S {ppid} {pid} {session} 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 1 0 {start} 1000 100"
    )
}

/// The probe's verdict rests on reading the agent's pinned process, its parent links and session leaders from the
/// recorded ancestry, whatever the process names hold. Specified: a name with spaces and `)` parses to its pid,
/// parent, session and field 22; a line with too few fields or a non-numeric pid does not parse.
#[farhelm_testtrace::test]
fn a_stat_line_parses_to_its_pid_parent_session_and_start_time_whatever_the_name() {
    assert_eq!(
        parse_stat_line(&stat_line(4242, "codex) (x y", 1, 4200, 987_654)),
        Some(ChainEntry {
            pid: 4242,
            ppid: 1,
            session: 4200,
            start: 987_654
        })
    );
    assert_eq!(parse_stat_line("4242 (sh) S 1 4242"), None);
    let line = stat_line(4242, "sh", 1, 4242, 5);
    assert_eq!(parse_stat_line(&line.replacen("4242", "pid", 1)), None);
}

/// A command counts as inside the session only when its ancestry holds the agent's process itself, directly rather
/// than through a server that detached into its own session, and a chain that cannot be trusted gives no verdict.
/// Premise these chains share with the probe: the first line is the process Codex spawned for the command (the probe
/// `exec`s the script so it is), which may lead its own session.
/// Specified: a chain through the root, with the shell leading its own session as Codex starts it, is inside; the
/// same pid with another start time is not; a chain that climbs from a detached server straight to pid 1 is not; a
/// server Codex started under itself with `setsid()` is not; an unparseable line, a broken parent link, a walk that
/// stops short of pid 1 and a chain that starts at the agent itself are errors.
#[farhelm_testtrace::test]
fn a_command_is_inside_only_through_the_root_and_no_session_leader() {
    let root = ProcessRoot {
        pid: 300,
        start: 5_000,
    };
    let agent = stat_line(300, "codex", 200, 300, 5_000);
    let above = [
        stat_line(200, "farhelm", 1, 200, 4_900),
        stat_line(1, "systemd", 0, 1, 1),
    ];
    let chain = |below: &[String], agent: &str| {
        let mut lines = below.to_vec();
        lines.push(agent.to_owned());
        lines.extend(above.iter().cloned());
        lines.join("\n")
    };
    let shell = stat_line(320, "sh", 300, 320, 5_200);
    assert!(chain_verdict(&chain(std::slice::from_ref(&shell), &agent), root).unwrap());
    let reused = stat_line(300, "codex", 200, 300, 9_999);
    assert!(!chain_verdict(&chain(std::slice::from_ref(&shell), &reused), root).unwrap());
    let detached = [
        stat_line(820, "sh", 810, 820, 6_200),
        stat_line(810, "codex app-server", 1, 810, 6_100),
        stat_line(1, "systemd", 0, 1, 1),
    ]
    .join("\n");
    assert!(!chain_verdict(&detached, root).unwrap());
    let own_server = [
        stat_line(420, "sh", 410, 420, 5_400),
        stat_line(410, "codex app-server", 300, 410, 5_300),
    ];
    assert!(!chain_verdict(&chain(&own_server, &agent), root).unwrap());
    let inside = chain(std::slice::from_ref(&shell), &agent);
    assert!(chain_verdict(&format!("{inside}\nnot a stat line"), root).is_err());
    let broken = chain(&[stat_line(320, "sh", 999, 320, 5_200)], &agent);
    assert!(chain_verdict(&broken, root).is_err());
    let short = [shell, agent.clone()].join("\n");
    assert!(chain_verdict(&short, root).is_err());
    assert!(chain_verdict(&chain(&[], &agent), root).is_err());
}
