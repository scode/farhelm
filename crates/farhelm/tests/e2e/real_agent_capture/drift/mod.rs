//! Opt-in checks that the installed CLIs still accept Farhelm's launch and resume commands.
//!
//! Each test owns a new workspace, supervisor state and terminal host. Vendors keep their ordinary login and
//! settings. An agent whose Farhelm hooks are global files gets Farhelm's real setup run into a private copy of its
//! configuration instead of the maintainer's (`private_config`). Each category declares which agents it exercises
//! and reuses this module's supervisor, terminal and verdict handling.

mod private_config;

use super::{
    SCREEN_BYTES, StreamText, hook_log_names_identity, reported_runtime_identity,
    serving_supervisor, which_binary,
};
use crate::harness::*;
use farhelm_proto::session_launch::CONVERSATION_PLACEHOLDER;
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchSelection, RestartOffer};
use farhelm_supervisor::agent_kind::AgentHooks;
use farhelm_supervisor::hook_report::{HookLogLine, ReportAdmission, parse_log_line};
use farhelm_supervisor::hosts::{HostTerminal, Liveness};
use futures_util::FutureExt as _;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, resume_unwind};
use std::path::{Path, PathBuf};
use std::process::Stdio;

/// The runner supplies a private result file. A crash, timeout or missing result is a runner-visible failure.
const VERDICT_FILE: &str = "FARHELM_DRIFT_VERDICT_FILE";
const COLS: u16 = 115;
const ROWS: u16 = 35;
const START_LIMIT: Duration = Duration::from_secs(90);
const TURN_LIMIT: Duration = Duration::from_secs(180);
/// A vendor's ID-discovery command has its own budget inside the live case.
const LOOKUP_LIMIT: Duration = Duration::from_secs(30);
/// How long an unchanged trust prompt stays on screen before the same answer is sent again. A key the dialog drops
/// would otherwise strand the case until START_LIMIT.
const TRUST_RETRY: Duration = Duration::from_secs(3);
// Spell the prefix separately so neither the prompt echo nor Turn 1 can supply the recall witness.
const RECALL_PROMPT: &str = "Join R E C A L L, a hyphen, and my test label on one line. Reply with only that line. No tools.";

/// Different categories need different evidence from the trust screen. A launch smoke check can stop there, while
/// a test of the explicit trust choice must reject an unexpected prompt.
#[derive(Clone, Copy)]
enum TrustPolicy<'a> {
    AutoAnswer,
    StopAtPrompt,
    /// A trusted launch must produce its post-trust composer without rendering any known question.
    FailOnPrompt {
        composer: &'a [&'a str],
        /// A category can recognize a current vendor screen that ordinary starts would not answer.
        extra_prompt: Option<fn(&str) -> bool>,
    },
}

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
    /// Claude once installed an update between two launches of one case.
    Claude,
    Muse,
    Omp,
}

/// Some report paths acknowledge subordinate or stale callbacks. The strict profiles need a write for the runtime
/// identity after Restart, because an untouched binding can still contain the right identity and resume command.
#[derive(Clone, Copy)]
enum HookWitness {
    Fresh,
    MatchingIdentity,
    CommittedIdentity { resumed_generation: Option<i64> },
}

/// Per-harness launch inputs and rendered composer cues. An omitted model uses the user's vendor setting.
struct Harness {
    binary: &'static str,
    model: Option<&'static str>,
    effort: Option<LaunchEffort>,
    composer: &'static [&'static str],
    hook_witness: Option<HookWitness>,
    trust_at_launch: bool,
    update_override: Option<UpdateOverride>,
}

/// Each built-in agent needs a deliberate test profile for checking its vendor's behavior.
#[warn(clippy::wildcard_enum_match_arm)]
fn profile(kind: LaunchHarness) -> Option<Harness> {
    use LaunchHarness::*;
    // Naming an OpenRouter model selects that provider. Otherwise preserve the vendor's configured provider and model.
    let flash = std::env::var_os("OPENROUTER_API_KEY")
        .filter(|key| !key.is_empty())
        .map(|_| "z-ai/glm-5.3-flash");
    let (binary, model, effort, composer, witness, trust_at_launch, update_override) = match kind {
        Claude => (
            "claude",
            Some("haiku"),
            Some(LaunchEffort::Low),
            &["claude code v", "❯"][..],
            Some(HookWitness::MatchingIdentity),
            false,
            Some(UpdateOverride::Claude),
        ),
        Codex => (
            "codex",
            Some("gpt-6-luna"),
            Some(LaunchEffort::Low),
            &["openai codex", "›"][..],
            Some(HookWitness::CommittedIdentity {
                resumed_generation: None,
            }),
            true,
            None,
        ),
        Antigravity => (
            "agy",
            Some("gemini-3.8-flash"),
            Some(LaunchEffort::Low),
            // Not the bare name: 1.5.14's "Welcome to the Antigravity CLI ... Signing in..." splash, shown before
            // its trust prompt, contains it, and a prompt typed into the splash is lost. 1.5.14's composer has
            // "? for shortcuts" under the input box.
            &["? for shortcuts", "type a message"][..],
            Some(HookWitness::CommittedIdentity {
                resumed_generation: None,
            }),
            false,
            None,
        ),
        Goose => (
            "goose",
            flash,
            None,
            &["goose session", "enter your message", "( o)>"][..],
            Some(HookWitness::Fresh),
            false,
            None,
        ),
        Grok => (
            "grok",
            None,
            None,
            &["grok", "❯"][..],
            Some(HookWitness::CommittedIdentity {
                resumed_generation: None,
            }),
            false,
            None,
        ),
        Muse => (
            "muse",
            Some("muse-spark-1.3-contributor"),
            None,
            // Startup warnings can truncate the model name in the composer footer.
            &["muse-spark-1.3-"][..],
            Some(HookWitness::MatchingIdentity),
            true,
            Some(UpdateOverride::Muse),
        ),
        Pi => (
            "pi",
            flash,
            Some(LaunchEffort::Low),
            &[
                "pi v", "/hotkeys", "ctrl+c",
                // A resumed conversation can push the startup header, and with it every cue above, off the top of
                // the screen; 1.6.0 did on 2026-11-23 when startup warnings added lines. The context-usage figure in
                // the footer under the editor stays: "2.9%/262k", or "?/262k" while usage is unknown.
                "%/", "?/",
            ][..],
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
            false,
            None,
        ),
        OpenCode => (
            "opencode",
            None,
            None,
            &["opencode", "ask anything", "ctrl+p"][..],
            None,
            false,
            None,
        ),
        Bash | Zsh | Custom => return None,
    };
    Some(Harness {
        binary,
        model,
        effort,
        composer,
        hook_witness: witness,
        trust_at_launch,
        update_override,
    })
}

/// A visible login or credential error, as opposed to a CLI rejecting Farhelm's argv.
fn auth_block(text: &str) -> Option<&'static str> {
    [
        "select a login method",
        "sign in to continue",
        "log in to continue",
        "please log in",
        "press any key to log in",
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

/// Run a selector lookup in the same workspace as the interactive launch. A refusal after a successful turn belongs
/// to the test result, even if the same response could signal missing credentials before the first turn.
async fn vendor_lookup(
    work: &Path,
    binary: &str,
    args: &[&str],
    turned: bool,
) -> Result<String, DriftError> {
    let mut command = tokio::process::Command::new(binary);
    command
        .args(args)
        .current_dir(work)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(LOOKUP_LIMIT, command.output())
        .await
        .map_err(|_| anyhow::anyhow!("{binary} {args:?} timed out"))?
        .map_err(|error| {
            anyhow::Error::new(error).context(format!("could not run {binary} {args:?}"))
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(
            refusal(&stderr.to_lowercase(), &stdout, turned).unwrap_or_else(|| {
                anyhow::anyhow!("{binary} {args:?} exited {}\n{stderr}", output.status).into()
            }),
        );
    }
    String::from_utf8(output.stdout).map_err(|error| anyhow::Error::from(error).into())
}

/// Call after a successful OpenCode turn in `work`. The lookup must find exactly one conversation there.
/// A refusal, missed match or ambiguity at this point fails the selected drift case.
async fn opencode_workspace_session(work: &Path, binary: &str) -> Result<String, DriftError> {
    let output = vendor_lookup(
        work,
        binary,
        &[
            "session",
            "list",
            "--pure",
            "--format",
            "json",
            "--max-count",
            "32",
        ],
        true,
    )
    .await?;
    opencode_session(&output, work).map_err(Into::into)
}

/// Accept either spelling of a symlinked workspace. Keep the raw path when resolution fails, and require
/// a single match so an unrelated session or a second launch can never supply the selector.
fn opencode_session(output: &str, work: &Path) -> anyhow::Result<String> {
    #[derive(serde::Deserialize)]
    struct Session {
        id: String,
        directory: PathBuf,
    }

    let sessions: Vec<Session> = serde_json::from_str(output).map_err(|error| {
        anyhow::Error::new(error).context("could not decode the OpenCode session list")
    })?;
    let listed = sessions.len();
    let canonical = work.canonicalize().unwrap_or_else(|_| work.to_path_buf());
    let mut matching = sessions.into_iter().filter(|session| {
        session
            .directory
            .canonicalize()
            .as_deref()
            .unwrap_or(&session.directory)
            == canonical.as_path()
    });
    let owned = matching.next().ok_or_else(|| {
        anyhow::anyhow!("OpenCode listed no session for the seed workspace among {listed} rows")
    })?;
    anyhow::ensure!(
        matching.next().is_none(),
        "OpenCode listed more than one session for the seed workspace"
    );
    Ok(owned.id)
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
    // Grok Build 1.0.92 marks the selection by colour alone, which the screen text does not carry, and gives each
    // answer its own key ("Yes, proceed   y"). The key answers regardless of the selection. Grok asks only once a
    // hook is installed, so its case first reached this question when it began setting up its hooks.
    if text
        .lines()
        .map(str::trim)
        .any(|line| line.starts_with("yes, proceed") && line.split_whitespace().last() == Some("y"))
    {
        return Some(b"y");
    }
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
        Self::attach_with_width(sup, client, id, turned, COLS).await
    }

    /// Model verification can widen the attach to keep long model IDs visible beside CLI notices.
    async fn attach_with_width(
        sup: &Supervisor,
        client: &'a SupervisorClient,
        id: &str,
        turned: bool,
        cols: u16,
    ) -> Result<Self, DriftError> {
        let host = sup
            .agent_host_for_test(id)
            .await
            .ok_or_else(|| anyhow::anyhow!("no terminal host for {id}"))?;
        let (channel, mut stream) =
            client
                .attach_at_boundary(id, cols, ROWS)
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

    /// Await an interactive screen. A trust response is sent once per newly rendered prompt, and again when that
    /// same rendering is still on screen after TRUST_RETRY. The caller decides whether the prompt itself suffices,
    /// the workspace can be trusted, or the prompt means the launch choice was ignored.
    ///
    /// The retry exists because Claude Code 2.1.330 lost the answer twice in a row on 2026-11-22: once it reset the
    /// selection to "No, exit" after the Enter, and once the Down never registered. Answering each rendering only
    /// once then waited out START_LIMIT on an unchanged dialog. Answering the same dialog by hand worked every time.
    async fn ready(
        &mut self,
        profile: &Harness,
        trust: TrustPolicy<'_>,
        turned: bool,
    ) -> DriftResult {
        // A trust check needs evidence of the composer beyond the dialog. A CLI's startup banner can draw earlier.
        let (composer, extra_prompt) = match trust {
            TrustPolicy::FailOnPrompt {
                composer,
                extra_prompt,
            } => (composer, extra_prompt),
            TrustPolicy::AutoAnswer | TrustPolicy::StopAtPrompt => (profile.composer, None),
        };
        let deadline = tokio::time::Instant::now() + START_LIMIT;
        let mut answered: Option<(String, tokio::time::Instant)> = None;
        loop {
            let rendered = self.host.screen_tail(SCREEN_BYTES);
            let text = rendered.to_lowercase();
            self.check(&text, turned).await?;
            if trust_prompt(&text) || extra_prompt.is_some_and(|is_prompt| is_prompt(&text)) {
                match trust {
                    TrustPolicy::StopAtPrompt => return Ok(()),
                    TrustPolicy::FailOnPrompt { .. } => {
                        return Err(anyhow::anyhow!(
                            "unexpected workspace trust prompt\n{}",
                            self.diagnostics()
                        )
                        .into());
                    }
                    TrustPolicy::AutoAnswer => {}
                }
                let unanswered = answered
                    .as_ref()
                    .is_none_or(|(screen, at)| *screen != rendered || at.elapsed() >= TRUST_RETRY);
                if unanswered && let Some(keys) = trust_answer(&text) {
                    self.client.send_keys(self.channel, keys.to_vec()).await;
                    answered = Some((rendered, tokio::time::Instant::now()));
                }
            } else if composer.iter().any(|marker| text.contains(marker)) {
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

/// A captured conversation's launch selector, including any vendor-specific spelling present in saved argv.
/// The resume template is the source of its position because agents put the value after different flags.
fn conversation_selector<'a>(
    resume_template: &[String],
    resume_argv: &'a [String],
) -> Option<&'a str> {
    let slot = resume_template
        .iter()
        .position(|word| word == CONVERSATION_PLACEHOLDER)?;
    resume_argv.get(slot).map(String::as_str)
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
    fn saw_reply(&self, text: &str) -> bool {
        reply_on_screen(text, self.reply)
    }

    /// An acknowledgment for an earlier launch cannot prove admission on the resumed launch, even if the vendor
    /// sends the same ID and the snapshot still offers the same resume command.
    fn saw_report(&self, log: &str, resume_argv: Option<&[String]>, conversation: &str) -> bool {
        if !resume_argv
            .is_some_and(|argv| self.original_argv.is_none_or(|original| argv == original))
        {
            return false;
        }
        if let HookWitness::CommittedIdentity {
            resumed_generation: Some(generation),
        } = self.hook_witness
        {
            let expected = reported_runtime_identity(conversation);
            return acknowledged_reports(log).any(|entry| {
                entry.admission == Some(ReportAdmission::Committed { generation })
                    && entry
                        .identity()
                        .is_some_and(|id| hook_log_names_identity(id, &expected))
            });
        }
        let mut fresh = acknowledged_reports(log).skip(self.acked_before);
        match self.hook_witness {
            HookWitness::Fresh => fresh.next().is_some(),
            HookWitness::MatchingIdentity | HookWitness::CommittedIdentity { .. } => {
                let expected = reported_runtime_identity(conversation);
                fresh.any(|entry| {
                    entry
                        .identity()
                        .is_some_and(|id| hook_log_names_identity(id, &expected))
                })
            }
        }
    }
}

/// A reply starts a line after any answer bullet, in any casing. Goose and OMP can add prose after "OK", so
/// match the word at a boundary. The prompt echo can mention the same word later in a line and supplies no reply
/// evidence.
fn reply_on_screen(text: &str, reply: &str) -> bool {
    text.lines().map(str::trim).any(|line| {
        let line = line
            .trim_start_matches(['•', '●', '✦', '◆', '-', '*'])
            .trim_start();
        line.get(..reply.len()).is_some_and(|word| {
            word.eq_ignore_ascii_case(reply)
                && !line[reply.len()..]
                    .chars()
                    .next()
                    .is_some_and(char::is_alphanumeric)
        })
    })
}

/// Count acknowledgments for the legacy baseline without mistaking them for committed writes.
fn acknowledged_reports(log: &str) -> impl Iterator<Item = HookLogLine<'_>> {
    log.lines()
        .filter_map(parse_log_line)
        .filter(|entry| entry.word == "acked" && entry.about.is_some())
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
    wait_for_turn_matching(terminal, sup, id, hook_log, expected, |text| {
        expected.saw_reply(text)
    })
    .await
}

/// Categories can require a complete reply across several screen rows while keeping the same capture evidence.
async fn wait_for_turn_matching(
    terminal: &mut Terminal<'_>,
    sup: &Supervisor,
    id: &str,
    hook_log: &Path,
    expected: &TurnExpectation<'_>,
    mut saw_reply: impl FnMut(&str) -> bool,
) -> Result<SessionSnapshot, DriftError> {
    let deadline = tokio::time::Instant::now() + TURN_LIMIT;
    let mut answered = false;
    let mut acked = 0;
    loop {
        let text = terminal.host.screen_tail(SCREEN_BYTES);
        answered |= saw_reply(&text);
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
                && !matches!(
                    expected.hook_witness,
                    HookWitness::CommittedIdentity {
                        resumed_generation: Some(generation),
                    } if generation != snapshot.generation
                )
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
                "recall arrived, but the new hook evidence did not confirm the original conversation on this launch"
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

/// The private supervisor and workspace a category borrows for its sessions. A named start can reuse the same
/// workspace as the session that produced its conversation.
struct DriftEnv<'a> {
    harness: LaunchHarness,
    sup: &'a Supervisor,
    client: &'a SupervisorClient,
    work: &'a Path,
    state: &'a Path,
    private: Option<&'a private_config::PrivateConfig>,
    session_cleanup: &'a SessionCleanup,
    /// Whether this fixture's supervisor may install this agent's persistent reporter files.
    installs_hooks: bool,
}

/// A create can leave an agent behind even when the supervisor fails to return a session ID.
/// Count each attempt until Delete confirms cleanup. Terminal-host shutdown alone cannot provide that confirmation.
#[derive(Default)]
struct SessionCleanup(Cell<usize>);

impl SessionCleanup {
    fn begin_create(&self) {
        self.0.set(self.0.get() + 1);
    }

    fn record_delete(&self, result: &anyhow::Result<()>) {
        if result.is_ok() {
            self.0.set(self.0.get() - 1);
        }
    }

    fn allows_copy_back(&self, terminal_hosts_closed: bool) -> bool {
        terminal_hosts_closed && self.0.get() == 0
    }

    /// Once Create returned an ID, Delete owns the process-tree sweep even if the live assertion unwinds.
    async fn check_and_delete<T>(
        &self,
        check: impl Future<Output = Result<T, DriftError>>,
        delete: impl Future<Output = anyhow::Result<()>>,
    ) -> Result<T, DriftError> {
        let result = AssertUnwindSafe(check).catch_unwind().await;
        let deleted = delete.await;
        self.record_delete(&deleted);
        finish_check(result, deleted)
    }
}

impl DriftEnv<'_> {
    /// Start with the category's one-time extras and delete the session after its checks, even on failed attachment
    /// or startup. Hook setup always targets the private copy for agents with global hook files.
    async fn with_session<T>(
        &self,
        launch: farhelm_proto::SessionLaunch,
        mut extras: farhelm_helm::CreateExtras,
        check: impl AsyncFnOnce(&str) -> Result<T, DriftError>,
    ) -> Result<T, DriftError> {
        // The case, binary preflight and hook mirror belong to one harness. Several distinct harnesses share
        // the generic integration, so compare the harness itself.
        if launch.agent_type() != Some(self.harness) {
            return Err(anyhow::anyhow!(
                "launch harness {:?} does not match the drift fixture for {:?}",
                launch.agent_type(),
                self.harness
            )
            .into());
        }
        extras.setup_persistent_hooks =
            self.harness.agent_kind().offers_persistent_hooks() && self.private.is_some();
        self.session_cleanup.begin_create();
        let session = self
            .client
            .create_session_with_extras(
                &self.work.to_string_lossy(),
                launch,
                None,
                COLS,
                ROWS,
                extras,
            )
            .await?;
        self.session_cleanup
            .check_and_delete(
                async {
                    if let Some(private) = self.private {
                        private.check_hook_setup(self.installs_hooks)?;
                    }
                    check(&session.id).await
                },
                self.client.delete_session(&session.id),
            )
            .await
    }
}

/// Keep the installed CLI version under test through both Start and Restart.
fn update_override_prefix(profile: &Harness, state: &Path) -> anyhow::Result<Option<Vec<String>>> {
    match profile.update_override {
        // Documented at https://code.claude.com/docs/en/env-vars; it leaves the user's own settings alone.
        Some(UpdateOverride::Claude) => {
            Ok(Some(vec!["env".into(), "DISABLE_AUTOUPDATER=1".into()]))
        }
        Some(UpdateOverride::Muse) => Ok(Some(vec!["env".into(), "MUSE_NO_AUTO_UPDATE=1".into()])),
        Some(UpdateOverride::Omp) => omp_update_override(state).map(Some),
        None => Ok(None),
    }
}

/// Give the category an owned supervisor and its compiled launch. Agents with global hooks always use the mirror
/// planned by Farhelm. A category may supply a mirror for another kind, such as Claude's config for a trust check.
/// Confirm the terminal hosts are gone before reconciling any vendor writes to that mirror.
async fn with_drift_supervisor(
    profile: &Harness,
    selection: LaunchSelection,
    extra_config: Option<fn(&Path) -> anyhow::Result<private_config::PrivateConfig>>,
    run: impl AsyncFnOnce(DriftEnv<'_>, farhelm_proto::SessionLaunch) -> DriftResult,
) -> DriftResult {
    with_drift_supervisor_using(
        profile,
        selection,
        extra_config,
        SupervisorSeams::default(),
        run,
    )
    .await
}

/// The capture category can turn hooks off while keeping the same ownership and mirror checks as ordinary drift.
async fn with_drift_supervisor_hooks(
    profile: &Harness,
    selection: LaunchSelection,
    extra_config: Option<fn(&Path) -> anyhow::Result<private_config::PrivateConfig>>,
    agent_hooks: AgentHooks,
    run: impl AsyncFnOnce(DriftEnv<'_>, farhelm_proto::SessionLaunch) -> DriftResult,
) -> DriftResult {
    let seams = SupervisorSeams {
        agent_hooks,
        ..SupervisorSeams::default()
    };
    with_drift_supervisor_using(profile, selection, extra_config, seams, run).await
}

/// Start with an explicit supervisor policy (its hook setting, its agent instructions) while preserving the
/// fixture's private paths and teardown.
async fn with_drift_supervisor_using(
    profile: &Harness,
    mut selection: LaunchSelection,
    extra_config: Option<fn(&Path) -> anyhow::Result<private_config::PrivateConfig>>,
    mut seams: SupervisorSeams,
    run: impl AsyncFnOnce(DriftEnv<'_>, farhelm_proto::SessionLaunch) -> DriftResult,
) -> DriftResult {
    if which_binary(profile.binary).is_none() {
        return Err(DriftError::CouldNotRun(format!(
            "{} is not installed on PATH",
            profile.binary
        )));
    }
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    // The private vendor configuration must outlive the agent reading it.
    let config = farhelm_teststate::tempdir().map_err(anyhow::Error::from)?;
    let harness = selection.harness;
    let kind = harness.agent_kind();
    let private = if kind.offers_persistent_hooks() {
        private_config::PrivateConfig::prepare(kind, config.path()).await?
    } else {
        extra_config
            .map(|prepare| prepare(config.path()))
            .transpose()?
    };
    let installs_hooks = seams.agent_hooks.allows(kind);
    if let Some(private) = &private {
        private.apply(&mut seams);
    }
    // State reaps the terminal host if startup fails, and drops before the workspace and configuration.
    let work = farhelm_teststate::tempdir().map_err(anyhow::Error::from)?;
    let state = farhelm_teststate::tempdir().map_err(anyhow::Error::from)?;
    let mut prefix = update_override_prefix(profile, state.path())?.unwrap_or_default();
    if let Some(home) = &seams.grok_home {
        // Set GROK_HOME in the launch wrapper after the login shell has loaded its startup files. Start and Restart
        // must read the same mirror where the test installed its hooks.
        prefix.extend(["env".into(), format!("GROK_HOME={}", home.display())]);
    }
    if !prefix.is_empty() {
        prefix.extend(selection.wrapper_prefix.take().unwrap_or_default());
        selection.wrapper_prefix = Some(prefix);
    }
    let launch = farhelm_helm::compile_agent_launch(selection).map_err(anyhow::Error::msg)?;
    let (sup, client, accepting) = serving_supervisor(state.path(), seams).await;
    let session_cleanup = SessionCleanup::default();
    let result = AssertUnwindSafe(async {
        run(
            DriftEnv {
                harness,
                sup: &sup,
                client: &client,
                work: work.path(),
                state: state.path(),
                private: private.as_ref(),
                session_cleanup: &session_cleanup,
                installs_hooks,
            },
            launch,
        )
        .await
    })
    .catch_unwind()
    .await;
    let accept_stopped = accepting.stop_checked().await;
    drop(client);
    drop(sup);
    finish_case(result, |result| {
        let stopped = state.close().map_err(anyhow::Error::from);
        let finished = private.as_ref().map_or(Ok(()), |config| {
            config
                .finish(session_cleanup.allows_copy_back(stopped.is_ok()))
                .map(drop)
        });
        after_cleanup(
            after_cleanup(after_cleanup(result, accept_stopped), stopped),
            finished,
        )
    })
}

/// Report a failed Delete before propagating a check panic to the outer case cleanup.
fn finish_check<T>(
    result: std::thread::Result<Result<T, DriftError>>,
    deleted: anyhow::Result<()>,
) -> Result<T, DriftError> {
    match result {
        Ok(result) => after_cleanup(result, deleted),
        Err(payload) => {
            if let Err(error) = deleted {
                eprintln!("drift: Delete after panic failed: {error:#}");
            }
            resume_unwind(payload)
        }
    }
}

/// Keep cleanup diagnostics when a case panics, then resume that same panic so the runner still sees a failure.
fn finish_case(
    result: std::thread::Result<DriftResult>,
    cleanup: impl FnOnce(DriftResult) -> DriftResult,
) -> DriftResult {
    match result {
        Ok(result) => cleanup(result),
        Err(payload) => {
            if let Err(DriftError::Failed(error)) = cleanup(Ok(())) {
                eprintln!("drift: cleanup after panic failed: {error:#}");
            }
            resume_unwind(payload)
        }
    }
}

/// Keep the original check's evidence if cleanup also failed. Cleanup failure makes the whole case fail even
/// when the live check passed or a missing login otherwise would have made its coverage unavailable.
fn after_cleanup<T>(
    result: Result<T, DriftError>,
    cleanup: anyhow::Result<()>,
) -> Result<T, DriftError> {
    let Err(cleanup) = cleanup else {
        return result;
    };
    Err(match result {
        Ok(_) => cleanup,
        Err(DriftError::CouldNotRun(reason)) => {
            cleanup.context(format!("drift could not run: {reason}"))
        }
        Err(DriftError::Failed(error)) => {
            error.context(format!("drift cleanup also failed: {cleanup:#}"))
        }
    }
    .into())
}

/// A direct ignored test fails loudly when a prerequisite is absent. The manual runner turns that same outcome
/// into an explicit unknown verdict and keeps the ordinary test recorder's evidence.
async fn run_case(kind: LaunchHarness, execute: impl AsyncFnOnce(LaunchHarness) -> DriftResult) {
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

/// Register a category's ignored entry points and keep its coverage list available to the category's unit tests.
macro_rules! drift_cases {
    ($execute:path; $($name:ident => $kind:ident),+ $(,)?) => {
        const HARNESSES: &[LaunchHarness] = &[$(LaunchHarness::$kind),+];
        $(
        #[farhelm_testtrace::test]
        #[ignore = "manual live vendor drift check"]
        async fn $name() {
            run_case(LaunchHarness::$kind, $execute).await;
        }
    )+ };
}

mod base;
mod capture_timing_hooks;
mod initial_prompt;
mod instructions_delivery;
mod model_selection;
mod named_conversation;
mod permission_modes;
mod reasoning_effort;
mod workspace_trust;

#[cfg(test)]
mod tests {
    use super::*;

    /// Reaping the terminal hosts cannot make up for a failed Create or an unconfirmed Delete.
    /// A later successful Delete accounts for only its own create.
    #[farhelm_testtrace::test]
    fn copy_back_requires_every_created_session_to_be_accounted_for() {
        let cleanup = SessionCleanup::default();
        cleanup.begin_create();
        assert!(!cleanup.allows_copy_back(true), "no session ID returned");
        cleanup.begin_create();
        cleanup.record_delete(&Ok(()));
        assert!(
            !cleanup.allows_copy_back(true),
            "earlier attempt still unconfirmed"
        );

        let cleanup = SessionCleanup::default();
        cleanup.begin_create();
        cleanup.record_delete(&Err(anyhow::anyhow!("owned Delete refused")));
        assert!(!cleanup.allows_copy_back(true), "Delete failed");
        cleanup.record_delete(&Ok(()));
        assert!(
            !cleanup.allows_copy_back(false),
            "terminal hosts still unconfirmed"
        );
        assert!(cleanup.allows_copy_back(true));
    }

    /// A case panic after Create must attempt Delete before the outer mirror cleanup decides whether it can copy.
    #[farhelm_testtrace::test]
    async fn a_panicking_drift_check_runs_cleanup_and_resumes_the_original_panic() {
        let session_cleanup = SessionCleanup::default();
        session_cleanup.begin_create();
        let result = AssertUnwindSafe(session_cleanup.check_and_delete::<()>(
            async { panic!("fixture check panicked") },
            async {
                assert!(!session_cleanup.allows_copy_back(true));
                Ok(())
            },
        ))
        .catch_unwind()
        .await;
        let cleaned = Cell::new(false);
        let resumed = std::panic::catch_unwind(AssertUnwindSafe(|| {
            finish_case(result, |result| {
                assert!(
                    session_cleanup.allows_copy_back(true),
                    "Delete settled the prior create"
                );
                cleaned.set(true);
                result
            })
        }));
        assert!(cleaned.get());
        assert_eq!(
            resumed.unwrap_err().downcast_ref::<&str>(),
            Some(&"fixture check panicked")
        );
    }

    /// Other directories are never fallback candidates. Two rows in the same directory cannot identify the seed.
    #[farhelm_testtrace::test]
    fn opencode_only_resumes_a_session_in_the_test_workspace() {
        let scratch = farhelm_teststate::tempdir().expect("owned workspace");
        let work = scratch.path().canonicalize().expect("workspace exists");
        let other = serde_json::json!({"id": "ses_other", "directory": work.join("another")});
        let seed = serde_json::json!({"id": "ses_seed", "directory": work});
        let single = serde_json::json!([other, seed]).to_string();
        assert_eq!(opencode_session(&single, &work).unwrap(), "ses_seed");
        let missing = serde_json::json!([other]).to_string();
        assert!(
            opencode_session(&missing, &work)
                .unwrap_err()
                .to_string()
                .contains("listed no session")
        );
        let ambiguous =
            serde_json::json!([other, seed, {"id": "ses_second", "directory": work}]).to_string();
        assert!(
            opencode_session(&ambiguous, &work)
                .unwrap_err()
                .to_string()
                .contains("listed more than one session")
        );
    }

    /// OpenCode may write the workspace's canonical name or the name the test opened. A different spelling can
    /// identify the same session, but two rows using those spellings are still ambiguous.
    #[farhelm_testtrace::test]
    fn opencode_matches_a_symlinked_test_workspace_from_either_side() {
        let scratch = farhelm_teststate::tempdir().expect("owned workspace");
        let work = scratch.path().join("workspace");
        let alias = scratch.path().join("alias");
        std::fs::create_dir(&work).expect("workspace exists");
        std::os::unix::fs::symlink(&work, &alias).expect("workspace alias");
        assert_ne!(work, alias);
        assert_eq!(alias.canonicalize().unwrap(), work.canonicalize().unwrap());

        let real = serde_json::json!({"id": "ses_real", "directory": work});
        let linked = serde_json::json!({"id": "ses_linked", "directory": alias});
        assert_eq!(
            opencode_session(&serde_json::json!([real]).to_string(), &alias).unwrap(),
            "ses_real"
        );
        assert_eq!(
            opencode_session(&serde_json::json!([linked]).to_string(), &work).unwrap(),
            "ses_linked"
        );
        assert!(
            opencode_session(&serde_json::json!([real, linked]).to_string(), &work)
                .unwrap_err()
                .to_string()
                .contains("listed more than one session")
        );
    }

    /// The selector comes from the corresponding argv position even when another argument would look like a
    /// conversation ID. A partial saved command cannot supply that evidence.
    #[farhelm_testtrace::test]
    fn conversation_selector_uses_the_templates_slot() {
        let template = [
            "agent".into(),
            "--session".into(),
            CONVERSATION_PLACEHOLDER.into(),
        ];
        let argv = ["agent".into(), "--session".into(), "vendor:91abc".into()];
        assert_eq!(
            conversation_selector(&template, &argv),
            Some("vendor:91abc")
        );
        assert_eq!(conversation_selector(&template, &argv[..2]), None);
        assert_eq!(conversation_selector(&argv, &argv), None);
    }

    /// A prior report is already in the log, and the old conversation command is still in memory after Restart.
    fn after_restart(original_argv: &[String]) -> TurnExpectation<'_> {
        TurnExpectation {
            reply: "RECALL-FH78C1E9AF",
            acked_before: 1,
            original_argv: Some(original_argv),
            hook_witness: HookWitness::CommittedIdentity {
                resumed_generation: Some(7),
            },
        }
    }

    /// A rendered recall line and a second accepted report prove two separate parts of the same-conversation check.
    #[farhelm_testtrace::test]
    fn the_resumed_turn_recalls_the_code_word_and_keeps_its_identity() {
        let argv = ["resume".into(), "original".into()];
        let expected = after_restart(&argv);
        assert!(expected.saw_reply("Agent\n  • recall-fh78c1e9af\nComposer"));
        let log = "1 acked original stop\tcommitted gen=6\n2 written original start\n3 acked original start\tcommitted gen=7\n";
        assert!(expected.saw_report(log, Some(&argv), "original"));
    }

    /// The ready binding survives Restart. An ignored callback with that same identity supplies no new write.
    #[farhelm_testtrace::test]
    fn same_identity_acknowledgment_needs_a_write_in_the_resumed_generation() {
        let argv = ["resume".into(), "original".into()];
        let expected = after_restart(&argv);
        for suffix in ["\tignored", "\tcommitted gen=6", ""] {
            let log =
                format!("1 acked original stop\tcommitted gen=6\n2 acked original start{suffix}\n");
            assert!(!expected.saw_report(&log, Some(&argv), "original"), "{log}");
        }
    }

    /// Grok's log can truncate the file evidence after its ID. The ID and generation still have to match exactly.
    #[farhelm_testtrace::test]
    fn grok_restart_matches_the_logged_id_in_a_truncated_locator() {
        let argv = ["resume".into(), "example".into()];
        let expected = after_restart(&argv);
        let stored = r#"grok:{"session_id":"example","session_file":"path"}"#;
        let log = "1 acked grok:{\"version\":1, -\tcommitted gen=7 id=example";
        assert!(expected.saw_report(log, Some(&argv), stored));
        let different = log.replace("id=example", "id=example-other");
        assert!(!expected.saw_report(&different, Some(&argv), stored));
    }

    /// Retained Turn-1 history, typed input and a line merely containing the token are insufficient witnesses.
    #[farhelm_testtrace::test]
    fn replay_and_prompt_echo_cannot_supply_the_recall_line() {
        let argv = ["resume".into(), "original".into()];
        let expected = after_restart(&argv);
        let screen = format!(
            "Remember test label FH78C1E9AF. Reply with O and K joined into one word, and nothing else. No tools.\n• OK\n› {RECALL_PROMPT}"
        );
        assert!(!expected.saw_reply(&screen));
        assert!(!expected.saw_reply("I can't remember the code word."));
        assert!(!expected.saw_reply("› RECALL-FH78C1E9AF"));
        assert!(!expected.saw_reply("The code is RECALL-FH78C1E9AF."));
        // A longer token that merely starts with the label is a different answer.
        assert!(!expected.saw_reply("RECALL-FH78C1E9AF0"));
    }

    /// Grok's trust menu shows no selection glyph in text, only a key per answer; a bare mention of the words is not
    /// an option line, and menus with a selection glyph keep their own handling.
    #[farhelm_testtrace::test]
    fn a_trust_menu_with_answer_keys_is_answered_with_the_affirmative_key() {
        let grok = "do you trust the contents of this directory?\n  /tmp/x\n\n   yes, proceed                 y\n   \
                    no, quit                     n\n";
        assert!(trust_prompt(grok));
        assert_eq!(trust_answer(grok), Some(&b"y"[..]));
        assert_eq!(
            trust_answer("do you trust this?\nsay yes, proceed if you like"),
            None
        );
        assert_eq!(
            trust_answer("do you trust the contents?\n> yes, i trust this folder\n  no, exit"),
            Some(&b"\r"[..])
        );
    }

    /// Weak models decorate the answer they were told to give alone; the word still has to open the line.
    #[farhelm_testtrace::test]
    fn a_reply_may_continue_after_the_expected_word() {
        let turn_one = TurnExpectation {
            reply: "OK",
            acked_before: 0,
            original_argv: None,
            hook_witness: HookWitness::Fresh,
        };
        assert!(turn_one.saw_reply("OK (test label FH2713A357 noted)."));
        assert!(turn_one.saw_reply(" OK — test label FH98E78B3E noted."));
        assert!(turn_one.saw_reply("◆ ok"));
        assert!(!turn_one.saw_reply("OKAY"));
        assert!(!turn_one.saw_reply("Noted, OK."));
        assert!(!turn_one.saw_reply("O"));
    }

    /// A recalled word proves access to history, while the accepted report must preserve the resume identity.
    #[farhelm_testtrace::test]
    fn recall_with_a_changed_or_missing_resume_identity_is_incomplete() {
        let original = ["resume".into(), "original".into()];
        let other = ["resume".into(), "different".into()];
        let expected = after_restart(&original);
        let log =
            "1 acked original stop\tcommitted gen=6\n2 acked different start\tcommitted gen=7\n";
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
