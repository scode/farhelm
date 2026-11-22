//! Opt-in checks that the installed CLIs still accept Farhelm's launch and resume commands.
//!
//! Each test owns a new workspace, supervisor state and terminal host. Vendors retain their ordinary login and
//! manual hook configuration. The tracked tests plant a code word, Restart, and ask the new process to recall it.
//! A fresh accepted hook report must also leave the original resume command intact.

use super::{
    SCREEN_BYTES, StreamText, reported_runtime_identity, serving_supervisor, which_binary,
};
use crate::harness::*;
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, RestartOffer};
use farhelm_supervisor::hosts::{HostTerminal, Liveness};
use std::path::{Path, PathBuf};

/// The runner supplies a private result file. A crash, timeout or missing result is a runner-visible failure.
const VERDICT_FILE: &str = "FARHELM_DRIFT_VERDICT_FILE";
const COLS: u16 = 115;
const ROWS: u16 = 35;
const START_LIMIT: Duration = Duration::from_secs(90);
const TURN_LIMIT: Duration = Duration::from_secs(180);
// Spell the prefix separately so neither the prompt echo nor Turn 1 can supply the recall witness.
const RECALL_PROMPT: &str = "Join R E C A L L, a hyphen, and my test label on one line. No tools.";

/// An unavailable prerequisite gives no verdict. Once a turn succeeds, a startup or resume refusal is a failure.
#[derive(Debug)]
enum DriftError {
    CouldNotRun(String),
    Failed(anyhow::Error),
}

impl From<anyhow::Error> for DriftError {
    fn from(error: anyhow::Error) -> Self {
        Self::Failed(error)
    }
}

type DriftResult = Result<(), DriftError>;

/// Keep the installed CLI version under test through both Start and Restart.
#[derive(Clone, Copy)]
enum UpdateOverride {
    Muse,
    Omp,
}

/// Some report paths acknowledge subordinate or stale callbacks without changing the captured conversation.
/// Those profiles need an acknowledgment that names the runtime identity in the snapshot.
#[derive(Clone, Copy)]
enum HookWitness {
    Fresh,
    MatchingIdentity,
}

/// Per-harness launch inputs and rendered composer cues. An omitted model uses the user's vendor setting.
struct Harness {
    binary: &'static str,
    model: Option<&'static str>,
    effort: Option<LaunchEffort>,
    composer: &'static [&'static str],
    manual_hook: Option<&'static str>,
    hook_witness: Option<HookWitness>,
    trust_at_launch: bool,
    update_override: Option<UpdateOverride>,
}

/// Adding an agent requires a deliberate test profile. Shell sessions have no reporting contract.
#[warn(clippy::wildcard_enum_match_arm)]
fn profile(kind: LaunchHarness) -> Option<Harness> {
    use LaunchHarness::*;
    // Naming an OpenRouter model selects that provider. Otherwise preserve the vendor's configured provider and model.
    let flash = std::env::var_os("OPENROUTER_API_KEY")
        .filter(|key| !key.is_empty())
        .map(|_| "z-ai/glm-5.3-flash");
    let (binary, model, effort, composer, manual_hook, witness, trust_at_launch, update_override) =
        match kind {
            Claude => (
                "claude",
                Some("haiku"),
                Some(LaunchEffort::Low),
                &["claude code v", "❯"][..],
                None,
                Some(HookWitness::MatchingIdentity),
                false,
                None,
            ),
            Codex => (
                "codex",
                Some("gpt-6-luna"),
                Some(LaunchEffort::Low),
                &["openai codex", "›"][..],
                None,
                Some(HookWitness::MatchingIdentity),
                true,
                None,
            ),
            Antigravity => (
                "agy",
                Some("gemini-3.8-flash"),
                Some(LaunchEffort::Low),
                &["antigravity", "type a message"][..],
                Some("antigravity"),
                Some(HookWitness::MatchingIdentity),
                false,
                None,
            ),
            Goose => (
                "goose",
                flash,
                None,
                &["goose session", "enter your message", "( o)>"][..],
                None,
                Some(HookWitness::Fresh),
                false,
                None,
            ),
            Grok => (
                "grok",
                None,
                None,
                &["grok", "❯"][..],
                Some("grok"),
                Some(HookWitness::Fresh),
                false,
                None,
            ),
            Muse => (
                "muse",
                Some("muse-spark-1.3-contributor"),
                None,
                // Startup warnings can truncate the model name in the composer footer.
                &["muse-spark-1.3-"][..],
                None,
                Some(HookWitness::MatchingIdentity),
                true,
                Some(UpdateOverride::Muse),
            ),
            Pi => (
                "pi",
                flash,
                Some(LaunchEffort::Low),
                &["pi v", "/hotkeys", "ctrl+c"][..],
                None,
                Some(HookWitness::Fresh),
                false,
                None,
            ),
            Omp => (
                "omp",
                flash,
                Some(LaunchEffort::Low),
                &[
                    "omp v",
                    "oh-my-pi",
                    "/hotkeys",
                    "start your prompt",
                    "to change thinking effort",
                    // A resumed conversation can fill the screen and leave only the editor border.
                    "\n╰─",
                ][..],
                None,
                Some(HookWitness::Fresh),
                false,
                Some(UpdateOverride::Omp),
            ),
            Cursor => (
                "cursor-agent",
                None,
                None,
                &["cursor agent", "→"][..],
                None,
                None,
                false,
                None,
            ),
            OpenCode => (
                "opencode",
                None,
                None,
                &["opencode", "ask anything", "ctrl+p"][..],
                None,
                None,
                false,
                None,
            ),
            Bash | Zsh => return None,
        };
    Some(Harness {
        binary,
        model,
        effort,
        composer,
        manual_hook,
        hook_witness: witness,
        trust_at_launch,
        update_override,
    })
}

/// An installed manual hook may use a quoted absolute path. Look at the parsed command, without running it or
/// writing the vendor's configuration. A malformed or unreadable file is a prerequisite problem.
fn has_manual_hook(path: &Path, vendor: &str) -> bool {
    fn contains_command(value: &serde_json::Value, vendor: &str) -> bool {
        if let Some(entries) = value.as_object() {
            return entries.iter().any(|(key, entry)| {
                if key != "command" {
                    return contains_command(entry, vendor);
                }
                let Some(words) = entry
                    .as_str()
                    .and_then(|line| shell_words::split(line).ok())
                else {
                    return false;
                };
                words.split_first().is_some_and(|(command, args)| {
                    Path::new(command)
                        .file_name()
                        .is_some_and(|name| name == "farhelm")
                        && args.starts_with(&[
                            "internal".into(),
                            "hook".into(),
                            "--vendor".into(),
                            vendor.into(),
                        ])
                })
            });
        }
        value
            .as_array()
            .is_some_and(|entries| entries.iter().any(|entry| contains_command(entry, vendor)))
    }
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .is_some_and(|value| contains_command(&value, vendor))
}

/// Check the files the installed agent discovers. The drift test reads existing hook configuration.
fn manual_hook_configured(vendor: &str) -> bool {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match vendor {
        "antigravity" => home.is_some_and(|home| {
            let config = home.join(".gemini/config");
            has_manual_hook(&config.join("hooks.json"), vendor)
                || has_manual_hook(&config.join("plugins/farhelm/hooks.json"), vendor)
        }),
        "grok" => std::env::var_os("GROK_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|home| home.join(".grok")))
            .and_then(|home| std::fs::read_dir(home.join("hooks")).ok())
            .is_some_and(|files| {
                files.filter_map(Result::ok).any(|file| {
                    let path = file.path();
                    path.extension()
                        .is_some_and(|extension| extension == "json")
                        && has_manual_hook(&path, vendor)
                })
            }),
        _ => unreachable!("only the installed manual-hook vendors reach preflight"),
    }
}

/// A visible login or credential error, as opposed to a CLI rejecting Farhelm's argv.
fn auth_block(text: &str) -> Option<&'static str> {
    [
        "select a login method",
        "sign in to continue",
        "log in to continue",
        "please log in",
        "not logged in",
        "not authenticated",
        "authentication failed",
        "invalid api key",
        "api key is missing",
        "api key not found",
        "no api key",
        "missing api key",
        "401 unauthorized",
        "run /login",
        "login required",
        "sign in required",
        "your session has expired",
    ]
    .into_iter()
    .find(|marker| text.contains(marker))
}

/// A flag refusal takes precedence over an auth hint. After a successful turn every failure is actionable.
fn refusal(text: &str, tail: &str, turned: bool) -> Option<DriftError> {
    let tail = tail.to_lowercase();
    if [
        "unexpected argument",
        "unrecognized argument",
        "unknown option",
        "unknown flag",
        "unrecognized option",
    ]
    .iter()
    .any(|marker| text.contains(marker) || tail.contains(marker))
    {
        return Some(
            anyhow::anyhow!(
                "CLI refused launch arguments\nscreen:\n{text}\nrecent output:\n{tail}"
            )
            .into(),
        );
    }
    if !turned && let Some(marker) = auth_block(text).or_else(|| auth_block(&tail)) {
        return Some(DriftError::CouldNotRun(format!(
            "vendor login or credentials needed ({marker})"
        )));
    }
    None
}

/// Distinguish the known workspace questions from composer banners containing an agent name.
fn trust_prompt(text: &str) -> bool {
    [
        "do you trust",
        "is this a project you created",
        "trust this workspace",
        "trust this directory",
        "trust the contents",
        "trust this folder?",
    ]
    .iter()
    .any(|marker| text.contains(marker))
}

/// Compare the selected option before answering a known workspace question in the test's empty directory.
fn trust_answer(text: &str) -> Option<&'static [u8]> {
    for line in text.lines().map(str::trim) {
        let Some(option) = line.strip_prefix(['❯', '⟩', '›', '>']) else {
            continue;
        };
        let option = option.trim_start();
        if option.contains("trust") && !option.contains("don't") && !option.contains("do not") {
            return Some(b"\r");
        }
        if option.contains("no, exit") && text.contains("yes, i trust") {
            // The menu can consume a batched arrow and Enter as one input. Wait for the affirmative
            // selection to render before confirming it on the next pass.
            return Some(b"\x1b[B");
        }
    }
    None
}

/// One connected terminal and its live output after replay. Reading the on-screen composer through the supervisor
/// is safe while the attachment drains the stream used to send input.
struct Terminal<'a> {
    host: HostTerminal,
    client: &'a SupervisorClient,
    channel: u32,
    replay_tail: Vec<u8>,
    output: StreamText,
    drain: tokio::task::JoinHandle<()>,
}

impl Drop for Terminal<'_> {
    fn drop(&mut self) {
        self.drain.abort();
    }
}

impl<'a> Terminal<'a> {
    /// Attach after the replay boundary so input and post-launch output belong to the tested generation.
    async fn attach(
        sup: &Supervisor,
        client: &'a SupervisorClient,
        id: &str,
        turned: bool,
    ) -> Result<Self, DriftError> {
        let host = sup
            .agent_host_for_test(id)
            .await
            .ok_or_else(|| anyhow::anyhow!("no terminal host for {id}"))?;
        let (channel, mut stream) =
            client
                .attach_at_boundary(id, COLS, ROWS)
                .await
                .map_err(|error| {
                    let screen = host.screen_tail(SCREEN_BYTES).to_lowercase();
                    refusal(&screen, "", turned).unwrap_or_else(|| error.context(screen).into())
                })?;
        // A CLI can report missing credentials and close before ReplayComplete. Keep the suffix needed for that verdict.
        let mut replay = Vec::new();
        let attached = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                match stream.recv().await {
                    Some(TermEvent::Data(bytes)) => {
                        replay.extend_from_slice(&grid_text(&bytes));
                        let excess = replay.len().saturating_sub(4096);
                        replay.drain(..excess);
                    }
                    Some(TermEvent::ReplayComplete) => return Ok(()),
                    Some(TermEvent::Detached(reason)) => return Err(reason.to_string()),
                    None => return Err("attachment closed".into()),
                }
            }
        })
        .await;
        if !matches!(attached, Ok(Ok(()))) {
            let screen = host.screen_tail(SCREEN_BYTES).to_lowercase();
            let tail = String::from_utf8_lossy(&replay);
            return Err(refusal(&screen, &tail, turned).unwrap_or_else(|| {
                anyhow::anyhow!("attachment ended before ReplayComplete: {attached:?}\nscreen:\n{screen}\nreplay:\n{tail}").into()
            }));
        }
        let output = StreamText::default();
        let drain = output.record(stream);
        Ok(Self {
            host,
            client,
            channel,
            replay_tail: replay,
            output,
            drain,
        })
    }

    /// Keep failure diagnostics on the owned terminal. The transcript tail survives a final redraw before exit.
    fn diagnostics(&self) -> String {
        format!(
            "screen:\n{}\nrecent live output:\n{}",
            self.host.screen_tail(SCREEN_BYTES),
            self.output.tail(4096)
        )
    }

    /// Check startup refusals before interpreting any ready marker. Credential failures only count as an
    /// unavailable prerequisite before this test has seen a successful vendor turn.
    async fn check(&mut self, text: &str, turned: bool) -> DriftResult {
        let ended = self.drain.is_finished()
            || !matches!(self.host.liveness(), Liveness::Running { child: Some(_) });
        let tail = if ended {
            // An exit record can reach the host reader before the attachment queues its last rows and closes.
            let _ = tokio::time::timeout(Duration::from_secs(2), &mut self.drain).await;
            format!(
                "{}\n{}",
                String::from_utf8_lossy(&self.replay_tail),
                self.output.tail(4096)
            )
        } else {
            String::new()
        };
        if let Some(error) = refusal(text, &tail, turned) {
            return Err(error);
        }
        if ended {
            return Err(anyhow::anyhow!(
                "agent terminal ended: {:?}\n{}",
                self.host.liveness(),
                self.diagnostics()
            )
            .into());
        }
        Ok(())
    }

    /// Await an interactive screen. Trust responses are sent only once per rendered prompt, and never for the
    /// launch-only smoke cases, which already establish that the CLI entered its TUI.
    async fn ready(&mut self, profile: &Harness, turned: bool) -> DriftResult {
        let deadline = tokio::time::Instant::now() + START_LIMIT;
        let mut answered = None;
        loop {
            let rendered = self.host.screen_tail(SCREEN_BYTES);
            let text = rendered.to_lowercase();
            self.check(&text, turned).await?;
            if trust_prompt(&text) {
                if profile.hook_witness.is_none() {
                    return Ok(());
                }
                if answered.as_ref() != Some(&rendered)
                    && let Some(keys) = trust_answer(&text)
                {
                    self.client.send_keys(self.channel, keys.to_vec()).await;
                    answered = Some(rendered);
                }
            } else if profile.composer.iter().any(|marker| text.contains(marker)) {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "interactive screen did not become ready\n{}",
                    self.diagnostics()
                )
                .into());
            }
            pace().await;
        }
    }

    /// Type first, then require a rendered echo before Enter. This catches lost input and keeps Enter separate from
    /// paste, which some composers would treat as a literal line break.
    async fn type_text(&mut self, value: &str, turned: bool) -> DriftResult {
        self.check(&self.host.screen_tail(SCREEN_BYTES).to_lowercase(), turned)
            .await?;
        self.client
            .send_keys(self.channel, value.as_bytes().to_vec())
            .await;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
        loop {
            let text = self.host.screen_tail(SCREEN_BYTES);
            self.check(&text.to_lowercase(), turned).await?;
            if text.contains(value) {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "composer did not echo the new input\n{}",
                    self.diagnostics()
                )
                .into());
            }
            pace().await;
        }
    }
}

/// Pace rendered-screen and hook polling while leaving readiness to each observer.
async fn pace() {
    // sleep-ok: shared cadence for polling the real CLI's screen and accepted-report evidence.
    tokio::time::sleep(Duration::from_millis(350)).await;
}

/// A model response and an accepted resumable report are independent evidence. After Restart the report must be
/// newer than the stopped generation's baseline and keep the exact argv the supervisor derived from Turn 1.
struct TurnExpectation<'a> {
    reply: &'a str,
    acked_before: usize,
    original_argv: Option<&'a [String]>,
    hook_witness: HookWitness,
}

impl TurnExpectation<'_> {
    /// Full-line matching allows common answer bullets and casing. A word embedded in the prompt is just input echo.
    fn saw_reply(&self, text: &str) -> bool {
        text.lines().map(str::trim).any(|line| {
            line.trim_start_matches(['•', '●', '✦', '◆', '-', '*'])
                .trim()
                .eq_ignore_ascii_case(self.reply)
        })
    }

    /// After Restart both turns must resolve to the same resume command. If admission can acknowledge a callback
    /// without applying it, the new report also has to name the runtime identity in the sampled snapshot.
    fn saw_report(&self, log: &str, resume_argv: Option<&[String]>, conversation: &str) -> bool {
        if !resume_argv
            .is_some_and(|argv| self.original_argv.is_none_or(|original| argv == original))
        {
            return false;
        }
        let mut fresh = acknowledged_reports(log).skip(self.acked_before);
        match self.hook_witness {
            HookWitness::Fresh => fresh.next().is_some(),
            HookWitness::MatchingIdentity => {
                let expected = reported_runtime_identity(conversation);
                fresh.any(|incoming| incoming == expected)
            }
        }
    }
}

/// The supervisor writes the reported identity as the third field of an acknowledgment.
fn acknowledged_reports(log: &str) -> impl Iterator<Item = &str> {
    log.lines().filter_map(|line| {
        let mut fields = line.split_whitespace().skip(1);
        (fields.next()? == "acked").then_some(fields.next()?)
    })
}

/// The hook log belongs to this new session only. A startup hook can fire before the prompt, so both an accepted
/// resumable identity and a rendered model answer are required.
async fn wait_for_turn(
    terminal: &mut Terminal<'_>,
    sup: &Supervisor,
    id: &str,
    hook_log: &Path,
    expected: &TurnExpectation<'_>,
) -> Result<SessionSnapshot, DriftError> {
    let deadline = tokio::time::Instant::now() + TURN_LIMIT;
    let mut answered = false;
    let mut acked = 0;
    loop {
        let text = terminal.host.screen_tail(SCREEN_BYTES);
        answered |= expected.saw_reply(&text);
        terminal
            .check(
                &text.to_lowercase(),
                answered || expected.original_argv.is_some(),
            )
            .await?;
        if answered {
            // Read the acknowledgment first. A snapshot sampled before the report could still show the original
            // conversation inherited from Restart even when the new report names a different one.
            let log = std::fs::read_to_string(hook_log).unwrap_or_default();
            acked = acknowledged_reports(&log).count();
            let snapshot = sup
                .session_snapshot(id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("launched session vanished"))?;
            if let Some(conversation) = snapshot.captured_conversation.as_deref()
                && snapshot.restart_offer == RestartOffer::Resume
                && expected.saw_report(&log, snapshot.resume_argv.as_deref(), conversation)
            {
                return Ok(snapshot);
            }
        }
        if tokio::time::Instant::now() >= deadline {
            let reason = if !answered && expected.original_argv.is_some() {
                "the restarted agent did not recall the original code word"
            } else if answered && acked > expected.acked_before && expected.original_argv.is_some()
            {
                "recall arrived, but the new acknowledgments did not confirm the original resume command"
            } else if answered {
                "reply arrived but no accepted resumable conversation report followed"
            } else {
                "no response to the drift prompt"
            };
            let hook_tail = std::fs::read_to_string(hook_log).map_or_else(
                |error| format!("log unavailable: {error}"),
                |log| {
                    log.lines()
                        .rev()
                        .take(8)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect::<Vec<_>>()
                        .join("\n")
                },
            );
            return Err(anyhow::anyhow!(
                "{reason}\n{}\nhook log tail:\n{hook_tail}",
                terminal.diagnostics()
            )
            .into());
        }
        pace().await;
    }
}

/// Disable OMP's automatic install for both start and resume without editing its saved settings.
/// Preserve the caller's config overlays and append only this launch's update override.
fn omp_update_override(state: &Path) -> anyhow::Result<Vec<String>> {
    let overlay = state.join("drift-omp.yml");
    std::fs::write(&overlay, "update:\n  auto: false\n")?;
    let mut overlays: Vec<_> = std::env::var_os("PI_CONFIG_FILES")
        .iter()
        .flat_map(std::env::split_paths)
        .collect();
    overlays.push(overlay);
    let paths = std::env::join_paths(overlays)?;
    let paths = paths
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("OMP config paths are not UTF-8"))?;
    let word = format!("PI_CONFIG_FILES={paths}");
    Ok(vec!["env".into(), shell_words::quote(&word).into_owned()])
}

/// Drive the shipped launch, Stop and Restart through a private supervisor.
async fn execute(kind: LaunchHarness) -> DriftResult {
    let profile = profile(kind).expect("a drift case must name a supported agent");
    if which_binary(profile.binary).is_none() {
        return Err(DriftError::CouldNotRun(format!(
            "{} is not installed on PATH",
            profile.binary
        )));
    }
    if let Some(vendor) = profile.manual_hook
        && !manual_hook_configured(vendor)
    {
        return Err(DriftError::CouldNotRun(format!(
            "the {vendor} Farhelm hook is absent or unreadable (see the agent setup guide)"
        )));
    }
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    // State drops before the work directory and reaps the terminal host on every return path.
    let work = farhelm_teststate::tempdir().map_err(anyhow::Error::from)?;
    let state = farhelm_teststate::tempdir().map_err(anyhow::Error::from)?;
    let wrapper_prefix = match profile.update_override {
        Some(UpdateOverride::Muse) => Some(vec!["env".into(), "MUSE_NO_AUTO_UPDATE=1".into()]),
        Some(UpdateOverride::Omp) => Some(omp_update_override(state.path())?),
        None => None,
    };
    let launch = farhelm_helm::compile_agent_launch(LaunchSelection {
        harness: kind,
        model: profile.model.map(str::to_owned),
        effort: profile.effort,
        // Exercise the two launch-only agents' actual permission flags without sending either a prompt.
        permissions: profile
            .hook_witness
            .is_none()
            .then_some(LaunchPermission::Yolo),
        permission_flags: None,
        wrapper_prefix,
        container: None,
        // Codex's modal and Muse's startup redraw can lose input. Trust only the test workspace for this launch.
        workspace_trust: profile.trust_at_launch.then_some(true),
    })
    .map_err(anyhow::Error::msg)?;
    let (sup, client, accepting) = serving_supervisor(state.path()).await;
    let session = client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            launch,
            None,
            COLS,
            ROWS,
            farhelm_helm::CreateExtras::default(),
        )
        .await?;
    let id = &session.id;
    let mut first = Terminal::attach(&sup, &client, id, false).await?;
    let result = async {
        first.ready(&profile, false).await?;
        let Some(hook_witness) = profile.hook_witness else {
            return Ok(());
        };
        let random = uuid::Uuid::new_v4().simple().to_string();
        let code_word = format!("FH{}", random[..8].to_ascii_uppercase());
        // Both turns can be answered from the conversation. A tool approval would strand the terminal on a dialog
        // that has nothing to do with whether the agent remembered the word.
        let prompt =
            format!("Remember test label {code_word}. Reply with O and K joined. No tools.");
        first.type_text(&prompt, false).await?;
        first.client.send_keys(first.channel, b"\r".to_vec()).await;
        let hook_log = state.path().join("hook-log").join(format!("{id}.log"));
        let captured = wait_for_turn(
            &mut first,
            &sup,
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
            return Err(
                anyhow::anyhow!("the accepted Turn-1 hook disappeared before Restart").into(),
            );
        }
        client.restart_session(id, false).await?;
        let resumed = sup
            .session_snapshot(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Restart lost the session"))?;
        if resumed.generation <= captured.generation {
            return Err(anyhow::anyhow!("Restart did not start a new generation").into());
        }
        let mut second = Terminal::attach(&sup, &client, id, true).await?;
        second.ready(&profile, true).await?;
        let expected = TurnExpectation {
            reply: &format!("RECALL-{code_word}"),
            acked_before,
            original_argv: Some(
                captured
                    .resume_argv
                    .as_deref()
                    .expect("accepted Turn 1 has resume argv"),
            ),
            hook_witness,
        };
        if expected.saw_reply(&second.host.screen_tail(SCREEN_BYTES)) {
            return Err(
                anyhow::anyhow!("the recall witness was already present before Turn 2").into(),
            );
        }
        second.type_text(RECALL_PROMPT, true).await?;
        second
            .client
            .send_keys(second.channel, b"\r".to_vec())
            .await;
        wait_for_turn(&mut second, &sup, id, &hook_log, &expected).await?;
        Ok(())
    }
    .await;
    drop(first);
    let cleanup = client.delete_session(id).await;
    accepting.stop().await;
    cleanup?;
    result
}

/// A direct ignored test fails loudly when a prerequisite is absent. The manual runner turns that same outcome
/// into an explicit unknown verdict and keeps the ordinary test recorder's evidence.
async fn run_case(kind: LaunchHarness) {
    let outcome = execute(kind).await;
    let (verdict, detail) = match &outcome {
        Ok(()) => ("passed", "completed the selected live checks".to_owned()),
        Err(DriftError::CouldNotRun(reason)) => ("could_not_run", reason.clone()),
        Err(DriftError::Failed(error)) => ("failed", format!("{error:#}")),
    };
    let result_file = std::env::var_os(VERDICT_FILE);
    if let Some(path) = &result_file {
        let payload = serde_json::json!({"verdict": verdict, "detail": detail});
        std::fs::write(path, payload.to_string()).expect("write the real-harness verdict");
    }
    match outcome {
        Err(DriftError::CouldNotRun(_)) if result_file.is_some() => {
            println!("COULD NOT RUN: {detail}")
        }
        Err(_) => panic!("{verdict}: {detail}"),
        Ok(()) => println!("PASSED: {detail}"),
    }
}

/// Register the ignored entry points the manual runner discovers. These tests spend real vendor turns.
macro_rules! drift_cases {
    ($($name:ident => $kind:ident),+ $(,)?) => {
        const _: () = assert!(LaunchHarness::is_ordering_of_all(&[
            $(LaunchHarness::$kind,)+ LaunchHarness::Bash, LaunchHarness::Zsh,
        ]));
        $(
        #[farhelm_testtrace::test]
        #[ignore = "manual live vendor drift check"]
        async fn $name() {
            run_case(LaunchHarness::$kind).await;
        }
    )+ };
}

drift_cases! {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A prior report is already in the log, and the old conversation command is still in memory after Restart.
    fn after_restart(original_argv: &[String]) -> TurnExpectation<'_> {
        TurnExpectation {
            reply: "RECALL-FH78C1E9AF",
            acked_before: 1,
            original_argv: Some(original_argv),
            hook_witness: HookWitness::MatchingIdentity,
        }
    }

    /// A rendered recall line and a second accepted report prove two separate parts of the same-conversation check.
    #[farhelm_testtrace::test]
    fn the_resumed_turn_recalls_the_code_word_and_keeps_its_identity() {
        let argv = ["resume".into(), "original".into()];
        let expected = after_restart(&argv);
        assert!(expected.saw_reply("Agent\n  • recall-fh78c1e9af\nComposer"));
        let log = "1 acked original stop\n2 written original start\n3 acked original start\n";
        assert!(expected.saw_report(log, Some(&argv), "original"));
    }

    /// Retained Turn-1 history, typed input and a line merely containing the token are insufficient witnesses.
    #[farhelm_testtrace::test]
    fn replay_and_prompt_echo_cannot_supply_the_recall_line() {
        let argv = ["resume".into(), "original".into()];
        let expected = after_restart(&argv);
        let screen = format!(
            "Remember test label FH78C1E9AF. Reply with O and K joined. No tools.\n• OK\n› {RECALL_PROMPT}"
        );
        assert!(!expected.saw_reply(&screen));
        assert!(!expected.saw_reply("I can't remember the code word."));
        assert!(!expected.saw_reply("› RECALL-FH78C1E9AF"));
        assert!(!expected.saw_reply("The code is RECALL-FH78C1E9AF."));
    }

    /// A recalled word proves access to history, while the accepted report must preserve the resume identity.
    #[farhelm_testtrace::test]
    fn recall_with_a_changed_or_missing_resume_identity_is_incomplete() {
        let original = ["resume".into(), "original".into()];
        let other = ["resume".into(), "different".into()];
        let expected = after_restart(&original);
        let log = "1 acked original stop\n2 acked different start\n";
        assert!(expected.saw_reply("RECALL-FH78C1E9AF"));
        assert!(!expected.saw_report(log, Some(&other), "different"));
        assert!(!expected.saw_report(log, None, "original"));
        // A callback may be acknowledged even if the supervisor keeps the old identity.
        assert!(!expected.saw_report(log, Some(&original), "original"));
    }

    /// Unchanged stored argv can survive the restart itself. A written or refused hook cannot confirm it afresh.
    #[farhelm_testtrace::test]
    fn the_saved_resume_identity_needs_a_new_accepted_report() {
        let argv = ["resume".into(), "original".into()];
        let expected = after_restart(&argv);
        let log =
            "1 acked original stop\n2 written original start\n3 refused stale original start\n";
        assert_eq!(acknowledged_reports(log).count(), 1);
        assert!(expected.saw_reply("RECALL-FH78C1E9AF"));
        assert!(!expected.saw_report(log, Some(&argv), "original"));
    }
}
