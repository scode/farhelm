//! In-session conversation rotation: `/clear`, `/new` and their aliases replace the agent's conversation inside the
//! same process. There is no new session, launch or first input for Farhelm to correlate, so only the agent's own
//! report can say what happened. If that report stops arriving, Restart quietly resumes the conversation the user just
//! walked away from, or nothing at all (SPEC.md calls a new conversation Farhelm never learns of a defect).
//!
//! One driver takes every harness through the same lifecycle. It seeds a code word, rotates, seeds a new word in the
//! new conversation, and requires an accepted capture naming a different resume target. It then stops the session,
//! restarts it, and requires the resumed agent to recall the new word with the rotated resume arguments intact. Recall
//! of a word from a conversation whose only history is that word already proves the abandoned conversation was not
//! loaded, so no turn asks for the old word. A harness with several commands chains them in one session: each rotation
//! starts from the conversation the previous Restart resumed, which keeps the case inside the runner's per-harness
//! budget.
//!
//! Pi and OMP also switch conversations through two commands that do not start empty, so each gets its own oracle:
//!
//! - A fork copies the current conversation, so the fork knows the old word too. The driver waits for the vendor's
//!   notice that the fork exists, seeds a new word in the fork, and the recall after Restart asks for the newest word:
//!   only the fork knows it, so the answer proves Restart resumed the fork and not its parent.
//! - A resume switches back to an existing conversation, so it needs two. It follows a `/new`, which leaves the session
//!   in the second conversation while the first still exists, and selects the first in the vendor's picker by its code
//!   word. The capture must return to the first conversation's resume target before any prompt (the vendor reports the
//!   switch as it happens, with the file already saved), and Restart must recall the first word. No seed turn runs.
//!
//! Per-harness knowledge lives only in [`rotation_spec`]: which commands to run, the vendor's menu line that shows a
//! command matched, how to drive a command's picker, and the checks between the command and the next prompt. Those
//! checks are limited to what the standalone audits this category replaced pinned (Claude's report on `/clear` itself,
//! Codex's `/new` lag until the next prompt, and the `source` each of those reports carries) plus what SPEC.md states.
//! For every other rotation the contract is only that the next prompt's turn commits the new conversation and Restart
//! resumes it.
//!
//! NOTE: The checks that compare a hook runtime ID ([`Capture::holds`], and [`committed_report`] as the startup,
//! [`BeforePrompt::NewCapture`] and `source` checks call it) go through `reported_runtime_identity`, which extracts
//! the ID only from Codex, Grok and Claude locators. A Pi or OMP locator comes back whole, so for those two harnesses
//! these checks could not match. No Pi or OMP rotation uses them today (a resume's return check compares the resume
//! target instead); one that does needs that parser extended first.

use super::*;
use farhelm_helm::CreateExtras;
use farhelm_proto::SessionStatus;

/// How long a slash command may take to show its menu, and then to leave it after Enter.
const COMMAND_LIMIT: Duration = Duration::from_secs(60);
/// How long an unanswered keystroke waits before it is sent again. Keystroke bursts were seen to vanish on a loaded
/// machine, while a retry sent before the vendor rendered the first one would leave `/new/new` in the composer.
const KEY_RETRY: Duration = Duration::from_secs(2);
/// The negative observation after Codex's `/new`: long enough for a report that came at the wrong moment to land,
/// short enough not to stretch a case that already spends several turns.
const BEFORE_PROMPT: Duration = Duration::from_secs(5);

// ---------------------------------------------------------------------------------------------------------------------
// Per-harness descriptors
// ---------------------------------------------------------------------------------------------------------------------

/// One in-session command that replaces the agent's conversation.
#[derive(Clone, Copy, Debug)]
struct Rotation {
    /// The literal text typed into the composer.
    command: &'static str,
    /// Lowercase text the vendor's command menu shows once it has matched `command`. Enter before the menu renders can
    /// submit the command as a chat message, which spends a turn and rotates nothing.
    menu: &'static str,
    /// What must hold between the command and the next prompt.
    before_prompt: BeforePrompt,
    /// The `source` the vendor's hook gives the accepted report of the new conversation, where a replaced audit pinned
    /// it. Farhelm ignores the field; a change here is vendor drift worth seeing, not a Farhelm failure mode.
    source: Option<&'static str>,
    /// What kind of conversation the command leaves the session in, which decides the oracle.
    operation: Operation,
}

/// The three ways a command can change the conversation, each with its own proof that Restart follows it.
#[derive(Clone, Copy, Debug)]
enum Operation {
    /// A new, empty conversation (`/clear`, `/new`): seed a new word, recall it.
    Fresh,
    /// A copy of the current conversation: seed a new word, recall the newest word.
    Fork(Fork),
    /// The session's first conversation again, chosen in the vendor's picker: no seed, recall the first word.
    Resume(ResumePicker),
}

/// How a vendor's fork command makes the fork and says it did.
#[derive(Clone, Copy, Debug)]
struct Fork {
    /// The message picker the command opens first, where the vendor has one.
    picker: Option<ForkPicker>,
    /// Lowercase notice the vendor shows once the fork exists and the session is in it. The seed waits for it: a
    /// seed typed while the switch is still running could land in the parent, which the fork would then copy, and the
    /// recall of the newest word would no longer tell the fork from its parent.
    notice: &'static str,
}

/// A fork command's message picker. The driver takes the row the picker opens on (Pi 1.6.1 opens on the latest user
/// message) and never filters: any row gives a conversation that does not know the word seeded next.
#[derive(Clone, Copy, Debug)]
struct ForkPicker {
    /// Lowercase heading shown only while the picker is open.
    title: &'static str,
    /// Keys that empty the composer after the fork. Pi puts the chosen message back into the composer, and a seed
    /// typed after it would be submitted as one message with it.
    clear: &'static [u8],
}

/// A resume command's session picker, driven by typing the first conversation's code word into its search box.
#[derive(Clone, Copy, Debug)]
struct ResumePicker {
    /// Lowercase heading shown only while the picker is open.
    title: &'static str,
    /// What the highlighted row shows before its title; any one of them marks the row Enter will open.
    highlight: &'static [&'static str],
    /// How the search box matches what is typed into it.
    search: Search,
}

/// The search syntax a resume picker gives a code word. A fuzzy search can also list a row whose text merely contains
/// the word's characters in order, which is why the driver checks the highlighted row rather than the row count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Search {
    /// The word in double quotes matches it exactly (Pi 1.6.1's `"phrase"` syntax).
    Phrase,
    /// The bare word, matched fuzzily (OMP 19.2.1).
    Fuzzy,
}

impl Search {
    /// The text typed into the search box to find the conversation seeded with `word`.
    fn query(self, word: &str) -> String {
        match self {
            Search::Phrase => format!("\"{word}\""),
            Search::Fuzzy => word.to_owned(),
        }
    }
}

/// The pre-prompt rotation timing a harness is held to. Each non-default variant comes from a vendor fact the replaced
/// standalone audits pinned; anything else would be a guess about when a vendor reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BeforePrompt {
    /// Nothing is promised before the next prompt. Muse and Grok defer the report of a `/new` to a later event, Pi
    /// withdraws Resume until its new conversation has a file, and Codex may leave a `/clear` pending until `Stop`
    /// (SPEC.md). The shared wait after the next prompt still requires a resumable capture of a different
    /// conversation, so a pending shape passes only once it is confirmed.
    Unspecified,
    /// Claude fires `SessionStart` on `/clear` itself, with no model turn: a new resumable capture appears before any
    /// prompt on the cleared chat.
    NewCapture,
    /// Codex fires nothing on `/new` until the next prompt, so the old conversation stays the Restart target until
    /// then. This is the lag a user sees too.
    OldCaptureStays,
}

/// Everything the shared driver needs to know about one harness's rotation commands.
#[derive(Clone, Copy, Debug)]
struct RotationSpec {
    /// The `source` of the report the agent makes at process start, before any prompt, when the replaced audit pinned
    /// that the capture precedes input.
    startup_source: Option<&'static str>,
    /// Lowercase markers of a vendor overlay that can cover the composer after a reply. When all are visible, Escape
    /// dismisses it without choosing anything.
    overlay: &'static [&'static str],
    /// Lowercase markers the vendor shows only while a turn is still running. A reply can render, and Farhelm's status
    /// can read at rest, before the vendor will accept a command; while any of these is visible the driver waits.
    busy: &'static [&'static str],
    /// The commands, run in order in one session.
    rotations: &'static [Rotation],
}

/// A rotation whose only contract is the next prompt's report.
const fn fresh(command: &'static str, menu: &'static str) -> Rotation {
    Rotation {
        command,
        menu,
        before_prompt: BeforePrompt::Unspecified,
        source: None,
        operation: Operation::Fresh,
    }
}

/// A rotation that switches to a fork or back to an earlier conversation. Neither has a pinned pre-prompt timing of
/// the kind [`BeforePrompt`] names; a resume's own return check lives with its [`Operation`].
const fn switch(command: &'static str, menu: &'static str, operation: Operation) -> Rotation {
    Rotation {
        command,
        menu,
        before_prompt: BeforePrompt::Unspecified,
        source: None,
        operation,
    }
}

/// Each harness that tracks conversations decides its rotation coverage here, so a new one is a compile error until
/// it does. The menu lines were read from the installed CLIs at a 115-column terminal on 2026-11-27: Claude Code
/// 2.1.332, Codex 0.172.0, Muse Code 1.4.25, Pi 1.6.1, OMP 19.2.1, Grok Build 1.0.92 and Antigravity CLI 1.5.18. Pi's
/// and OMP's picker text was read the same day with `scripts/agent-tui.py`.
#[warn(clippy::wildcard_enum_match_arm)]
fn rotation_spec(harness: LaunchHarness) -> Option<RotationSpec> {
    use BeforePrompt::*;
    use LaunchHarness::*;
    let plain = |rotations| RotationSpec {
        startup_source: None,
        overlay: &[],
        busy: &[],
        rotations,
    };
    // The running-turn hint Claude Code 2.1.332, Codex 0.172.0 and Muse Code 1.4.25 show beside their work indicator.
    const INTERRUPTIBLE: &[&str] = &["esc to interrupt"];
    Some(match harness {
        // The replaced audit pinned `startup` at process start and `clear` on the command.
        Claude => RotationSpec {
            startup_source: Some("startup"),
            overlay: &[],
            busy: INTERRUPTIBLE,
            rotations: const {
                &[Rotation {
                    command: "/clear",
                    menu: "start a new session with empty context",
                    before_prompt: NewCapture,
                    source: Some("clear"),
                    operation: Operation::Fresh,
                }]
            },
        },
        // Codex reports `startup` again after `/new`, the reason nothing in Farhelm keys on `source`. Its rate-limit
        // notice can cover the composer after a reply; the replaced audit dismissed it the same way.
        Codex => RotationSpec {
            startup_source: None,
            overlay: &["approaching rate limits", "switch to", "esc back"],
            busy: INTERRUPTIBLE,
            rotations: const {
                &[
                    Rotation {
                        command: "/new",
                        menu: "start a new chat during a conversation",
                        before_prompt: OldCaptureStays,
                        source: Some("startup"),
                        operation: Operation::Fresh,
                    },
                    // SPEC.md handles a `clear` report that names a transcript not yet on disk (old conversation
                    // withdrawn, new one pending until a `Stop` confirms it), but it does not promise when Codex
                    // reports, and the replaced audit never looked before the prompt. So nothing is checked there.
                    // On 2026-11-27, 0.172.0 sent its `clear` report with the next prompt, already resumable. The
                    // shared wait after that prompt accepts either shape, because it needs a resumable capture of a
                    // different conversation, which a pending clear reaches only through its `Stop`.
                    Rotation {
                        command: "/clear",
                        menu: "clear the terminal and start a new chat",
                        before_prompt: Unspecified,
                        source: Some("clear"),
                        operation: Operation::Fresh,
                    },
                ]
            },
        },
        // Both commands show the same description; SPEC.md defers either one's report to the next prompt. Muse 1.4.25
        // kept "Double checking (… esc to interrupt)" after its reply and its `Stop` on 2026-11-27, refused `/clear`
        // as "can't run while a turn is active", after the session's status had already read at rest.
        Muse => RotationSpec {
            startup_source: None,
            overlay: &[],
            busy: INTERRUPTIBLE,
            rotations: const {
                &[
                    fresh("/clear", "clear terminal and start a fresh session"),
                    fresh("/new", "clear terminal and start a fresh session"),
                ]
            },
        },
        // `/resume` follows `/new` and comes before `/fork`: it needs a second conversation to switch away from, and a
        // fork made earlier would list a row with the first conversation's own opening message beside it (OMP's
        // picker titles a session by its first message). Pi's fork picker lists the user messages to fork at; its
        // resume picker renders below the conversation, whose lines carry no `›`, so that marker only means the
        // highlighted row.
        Pi => plain(
            const {
                &[
                    fresh("/new", "start a new session"),
                    switch(
                        "/resume",
                        "resume another session",
                        Operation::Resume(ResumePicker {
                            title: "resume session (current folder)",
                            highlight: &["› "],
                            search: Search::Phrase,
                        }),
                    ),
                    switch(
                        "/fork",
                        "create a new fork from a previous user message",
                        Operation::Fork(Fork {
                            picker: Some(ForkPicker {
                                title: "fork from message",
                                // Ctrl-U deletes to the start of the line, which empties the one-line message Pi
                                // copied.
                                clear: b"\x15",
                            }),
                            notice: "forked to new session",
                        }),
                    ),
                ]
            },
        ),
        // OMP forks the whole conversation at once, with no picker, leaves the composer empty, and then shows "Session
        // forked · return to original: omp --resume <id> or /resume" (read with `scripts/agent-tui.py` on
        // 2026-11-27). Its resume picker is a full-screen box whose highlighted row starts with a Nerd Font chevron
        // (U+F054).
        Omp => plain(
            const {
                &[
                    fresh("/new", "start a new session"),
                    switch(
                        "/resume",
                        "resume a different session",
                        Operation::Resume(ResumePicker {
                            title: "resume session (current folder)",
                            highlight: &["\u{f054} "],
                            search: Search::Fuzzy,
                        }),
                    ),
                    switch(
                        "/fork",
                        "fork this session, or open it in a multiplexer pane or window",
                        Operation::Fork(Fork {
                            picker: None,
                            notice: "session forked",
                        }),
                    ),
                ]
            },
        ),
        // Grok offers no `/fork` (its docs page says so).
        Grok => plain(const { &[fresh("/new", "start a new session")] }),
        // `/new` is an alias that the menu resolves to `/clear (new)`; the canonical command is typed.
        Antigravity => {
            plain(const { &[fresh("/clear", "clear conversation and start a new one")] })
        }
        // Goose's rotation coverage has not been decided. A case would first need a live look at what its reporter
        // does when the conversation changes inside one process.
        Goose => return None,
        // Cursor and OpenCode report no conversation, so a rotation has nothing to change.
        Cursor | OpenCode => return None,
        // Shells and custom agents have no vendor command; a custom agent's own reporter owns its timing.
        Bash | Zsh | Custom => return None,
    })
}

// ---------------------------------------------------------------------------------------------------------------------
// Pure screen and capture predicates (unit-tested below)
// ---------------------------------------------------------------------------------------------------------------------

/// How often the command and its menu line were already on screen before anything was typed. Vendor chrome can name a
/// command (Claude's tips have named `/clear`), so a bare `contains` would call the command typed before a key was
/// pressed. Counting against this baseline is what makes the echo and the menu evidence of this input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Baseline {
    command: usize,
    menu: usize,
}

impl Baseline {
    /// Count on the lowercased screen sampled before typing.
    fn of(screen: &str, rotation: &Rotation) -> Self {
        Self {
            command: screen.matches(rotation.command).count(),
            menu: screen.matches(rotation.menu).count(),
        }
    }

    /// The menu line is shown more often than before typing, so the composer holds a matched command.
    fn menu_open(&self, screen: &str, rotation: &Rotation) -> bool {
        screen.matches(rotation.menu).count() > self.menu
    }
}

/// What to do next while getting a command into the composer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Typing {
    /// A vendor overlay covers the composer; dismiss it first.
    Dismiss,
    /// Neither the command nor its menu shows beyond the baseline; (re)type it.
    Type,
    /// Some evidence is there but not all; typing again would double the command.
    Wait,
    /// The command echoes and its menu matched it; Enter can run it.
    Matched,
}

/// Decide the next typing action from one lowercased screen sample.
fn typing_step(screen: &str, rotation: &Rotation, overlay: &[&str], baseline: &Baseline) -> Typing {
    if !overlay.is_empty() && overlay.iter().all(|marker| screen.contains(marker)) {
        return Typing::Dismiss;
    }
    let echoed = screen.matches(rotation.command).count() > baseline.command;
    match (echoed, baseline.menu_open(screen, rotation)) {
        (true, true) => Typing::Matched,
        (false, false) => Typing::Type,
        (true, false) | (false, true) => Typing::Wait,
    }
}

/// What to do next while a resume command's session picker is up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Picking {
    /// The picker has not rendered yet, or its list is still settling; look again.
    Wait,
    /// The search box does not show the query; (re)type it.
    Search,
    /// The target row is listed but another row is highlighted; move down.
    Down,
    /// The highlighted row is the one to open; press Enter.
    Select,
    /// The picker has closed after Enter.
    Closed,
}

/// A row the search should find: the query typed into the search box and the code word the row shows. [`pick`] takes
/// them as typed; [`picking_step`] compares their lowercased forms with the lowercased screen.
#[derive(Clone, Copy, Debug)]
struct PickTarget<'a> {
    query: &'a str,
    word: &'a str,
}

/// Decide the next picker action from one lowercased screen sample, with `target` lowercased too. Enter waits until
/// the highlighted row shows the target's word: a fuzzy search can list a near match above it, and Enter on that would
/// open the wrong conversation. The search box's own echo also contains the word, so that line never counts as a row.
fn picking_step(
    screen: &str,
    title: &str,
    highlight: &[&str],
    target: PickTarget<'_>,
    entered: bool,
) -> Picking {
    if !screen.contains(title) {
        return if entered {
            Picking::Closed
        } else {
            Picking::Wait
        };
    }
    let echo = format!("> {}", target.query);
    if !screen.contains(&echo) {
        return Picking::Search;
    }
    let mut listed = false;
    for line in screen.lines() {
        if !line.contains(target.word) || line.contains(&echo) {
            continue;
        }
        let highlighted = highlight.iter().any(|marker| {
            line.find(marker)
                .is_some_and(|at| line[at..].contains(target.word))
        });
        if highlighted {
            return Picking::Select;
        }
        listed = true;
    }
    if listed { Picking::Down } else { Picking::Wait }
}

/// What to do next while emptying a composer that a fork picker refilled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Clearing {
    /// The copied message has not shown yet; look again.
    Wait,
    /// The copy is (or was) there and has not gone; send the clear keys when due.
    Clear,
    /// The copy's count fell below the most seen after a clear was sent: the composer no longer holds it.
    Cleared,
}

/// Follows how often the copied message shows across samples. The copy must be seen before a clear counts, so a refill
/// that lands after an early clear cannot be missed, and the count must fall below the most seen rather than reach zero,
/// because the fork's own history can hold the same text.
#[derive(Clone, Copy, Debug, Default)]
struct CopyWatch {
    most: usize,
    sent: bool,
}

impl CopyWatch {
    /// Decide from the copy's count on one lowercased screen sample.
    fn observe(&mut self, count: usize) -> Clearing {
        if self.sent && count < self.most {
            return Clearing::Cleared;
        }
        self.most = self.most.max(count);
        if self.most > 0 {
            Clearing::Clear
        } else {
            Clearing::Wait
        }
    }

    /// Record that the clear keys went out, after which a fall in the count is their effect.
    fn sent(&mut self) {
        self.sent = true;
    }
}

/// One sample of the session row, reduced to what the rotation checks compare.
#[derive(Clone, Copy, Debug)]
struct Capture<'a> {
    conversation: Option<&'a str>,
    offer: RestartOffer,
    /// The conversation argument of the resume command Restart would run, read from its template slot.
    selector: Option<&'a str>,
}

impl<'a> Capture<'a> {
    fn of(snapshot: &'a SessionSnapshot) -> Self {
        Self {
            conversation: snapshot.captured_conversation.as_deref(),
            offer: snapshot.restart_offer,
            selector: snapshot
                .resume_template
                .as_deref()
                .zip(snapshot.resume_argv.as_deref())
                .and_then(|(template, argv)| conversation_selector(template, argv)),
        }
    }

    /// The captured conversation when Restart would resume it and its resume target is not `previous`. The resume
    /// target is compared rather than the stored value, because a locator can gain file evidence without changing
    /// which conversation Restart opens.
    fn rotated_from(&self, previous: Option<&str>) -> Option<&'a str> {
        let conversation = self.conversation?;
        let selector = self.selector?;
        (self.offer == RestartOffer::Resume && previous != Some(selector)).then_some(conversation)
    }

    /// Whether Restart would resume the conversation whose resume target is `target`. A resume returns to a target
    /// the session already had, so unlike [`Capture::rotated_from`] the target must match rather than move. The row
    /// targets another conversation when the resume command runs, and a row moves only through a committed report, so
    /// a match is itself the evidence that the vendor reported the switch.
    fn returned_to(&self, target: &str) -> bool {
        self.offer == RestartOffer::Resume && self.selector == Some(target)
    }

    /// Whether the row still offers to resume the conversation whose hook runtime ID is `runtime`.
    ///
    /// Reliable only for harnesses whose locator `reported_runtime_identity` parses (Codex, Grok, Claude); see the
    /// module doc. Only Codex's [`BeforePrompt::OldCaptureStays`] calls it.
    fn holds(&self, runtime: &str) -> bool {
        self.offer == RestartOffer::Resume
            && self
                .conversation
                .is_some_and(|conversation| reported_runtime_identity(conversation) == runtime)
    }
}

/// Whether the supervisor committed a report naming `runtime`, with `source` when one is given. Only a committed
/// acknowledgment counts: a `written` line proves only that the hook ran, and an ignored one changed nothing. A Stop
/// callback carries no source, which the log spells `-`.
///
/// `runtime` must be a bare runtime ID. Callers derive it with `reported_runtime_identity`, which yields one only for
/// Codex, Grok and Claude locators (see the module doc). The driver calls it only for Claude and Codex, the two
/// harnesses with a startup source, a [`BeforePrompt::NewCapture`] or a pinned rotation `source`.
fn committed_report(log: &str, runtime: &str, source: Option<&str>) -> bool {
    acknowledged_reports(log).any(|entry| {
        matches!(entry.admission, Some(ReportAdmission::Committed { .. }))
            && entry
                .identity()
                .is_some_and(|reported| hook_log_names_identity(reported, runtime))
            && source
                .is_none_or(|expected| entry.about.is_some_and(|(_, source)| source == expected))
    })
}

/// An unpredictable label, so a reply can only come from the conversation that was told it.
fn code_word() -> String {
    let random = uuid::Uuid::new_v4().simple().to_string();
    format!("FH{}", random[..8].to_ascii_uppercase())
}

/// The reply is the label itself, so each seed turn has a reply no earlier turn on the screen could have produced.
fn seed_prompt(code_word: &str) -> String {
    format!("Remember test label {code_word}. Reply with only that label. No tools.")
}

/// Ask for `RECALL-<label>` without naming the label, so neither the prompt echo nor the seed turn can supply the
/// witness. The base category's spelled-out prefix (`R E C A L L`) was answered literally as `R-E-C-A-L-L-…` by Claude
/// Haiku on 2026-11-27, after a correct resume; the named-conversation category's wording avoids that.
const ROTATION_RECALL_PROMPT: &str = "Reply RECALL- then my test label. No spaces or tools.";

/// A fork holds the parent's label as well as its own, so the recall names which one. Only the fork was told the newest
/// label; a resumed parent could only answer with the old one.
const FORK_RECALL_PROMPT: &str = "Reply RECALL- then my newest test label. No spaces or tools.";

impl Operation {
    /// The recall prompt after Restart.
    fn recall_prompt(self) -> &'static str {
        match self {
            Operation::Fresh | Operation::Resume(_) => ROTATION_RECALL_PROMPT,
            Operation::Fork(_) => FORK_RECALL_PROMPT,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The shared driver
// ---------------------------------------------------------------------------------------------------------------------

/// A missing log is the normal state before any hook has run.
fn read_log(log: &Path) -> String {
    std::fs::read_to_string(log).unwrap_or_default()
}

/// The newest lines of the hook log, for a failure message.
fn log_tail(log: &Path) -> String {
    tail_of(&read_log(log))
}

/// The newest lines of a hook log already read.
fn tail_of(text: &str) -> String {
    let lines: Vec<_> = text.lines().rev().take(8).collect();
    lines.into_iter().rev().collect::<Vec<_>>().join("\n")
}

/// Sample the tested row while the fixture still owns it.
async fn sample(env: &DriftEnv<'_>, id: &str) -> Result<SessionSnapshot, DriftError> {
    env.sup
        .session_snapshot(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("the rotation session vanished").into())
}

/// Wait for the report the agent makes as it starts, before any input, and check its pinned source.
async fn startup_capture(
    term: &mut Terminal<'_>,
    env: &DriftEnv<'_>,
    id: &str,
    log: &Path,
    source: &str,
) -> DriftResult {
    let deadline = tokio::time::Instant::now() + START_LIMIT;
    loop {
        term.check(&term.host.screen_tail(SCREEN_BYTES).to_lowercase(), false)
            .await?;
        let text = read_log(log);
        let snapshot = sample(env, id).await?;
        if let Some(conversation) = Capture::of(&snapshot).rotated_from(None)
            && committed_report(
                &text,
                &reported_runtime_identity(conversation),
                Some(source),
            )
        {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "no accepted `{source}` report arrived before the first prompt\n{}\nhook log tail:\n{}",
                term.diagnostics(),
                log_tail(log)
            )
            .into());
        }
        pace().await;
    }
}

/// Wait for a rendered reply and an accepted resumable capture whose resume target is not `previous`. Unlike
/// `wait_for_turn`, the target must have moved: a report that merely re-acknowledged the old conversation, or a
/// command that was never run, cannot pass.
async fn wait_for_capture(
    term: &mut Terminal<'_>,
    env: &DriftEnv<'_>,
    id: &str,
    log: &Path,
    expected: &TurnExpectation<'_>,
    previous: Option<&str>,
    turned: bool,
) -> Result<SessionSnapshot, DriftError> {
    let deadline = tokio::time::Instant::now() + TURN_LIMIT;
    let mut answered = false;
    let mut last = String::from("no sample after the reply");
    loop {
        let screen = term.host.screen_tail(SCREEN_BYTES);
        answered |= expected.saw_reply(&screen);
        term.check(&screen.to_lowercase(), answered || turned)
            .await?;
        if answered {
            // Read the log before the row, as `wait_for_turn` does: a row sampled first could predate the report the
            // log then shows.
            let text = read_log(log);
            let snapshot = sample(env, id).await?;
            let capture = Capture::of(&snapshot);
            let accepted = capture.rotated_from(previous).is_some_and(|conversation| {
                expected.saw_report(&text, snapshot.resume_argv.as_deref(), conversation)
            });
            if accepted {
                return Ok(snapshot);
            }
            last = format!(
                "offer {:?}, resume target {}",
                capture.offer,
                match capture.selector {
                    Some(selector) if Some(selector) == previous => "unchanged",
                    Some(_) => "changed without a fresh accepted report",
                    None => "absent",
                }
            );
        }
        if tokio::time::Instant::now() >= deadline {
            let reason = if answered {
                "the reply arrived but no accepted capture of a different conversation followed"
            } else {
                "no reply to the seed prompt"
            };
            return Err(anyhow::anyhow!(
                "{reason} ({last})\n{}\nhook log tail:\n{}",
                term.diagnostics(),
                log_tail(log)
            )
            .into());
        }
        pace().await;
    }
}

/// Submit one seed turn and wait for its capture. A reply already on screen would let the wait pass on old text.
#[allow(clippy::too_many_arguments)]
async fn seed(
    term: &mut Terminal<'_>,
    env: &DriftEnv<'_>,
    id: &str,
    log: &Path,
    code_word: &str,
    hook_witness: HookWitness,
    previous: Option<&str>,
    turned: bool,
) -> Result<SessionSnapshot, DriftError> {
    if reply_on_screen(&term.host.screen_tail(SCREEN_BYTES), code_word) {
        return Err(anyhow::anyhow!("the seed reply was already visible before its prompt").into());
    }
    let acked_before = acknowledged_reports(&read_log(log)).count();
    term.type_text(&seed_prompt(code_word), turned).await?;
    term.client.send_keys(term.channel, b"\r".to_vec()).await;
    let expected = TurnExpectation {
        reply: code_word,
        acked_before,
        original_argv: None,
        hook_witness,
    };
    wait_for_capture(term, env, id, log, &expected, previous, turned).await
}

/// Whether the agent will take a command: Farhelm's own status detection reads it at rest and the vendor shows none
/// of its running-turn markers. Either alone has let a command in too early, and a refused command closes the menu
/// just as an accepted one does.
fn at_rest(status: &SessionStatus, screen: &str, busy: &[&str]) -> bool {
    matches!(status, SessionStatus::Idle | SessionStatus::Waiting)
        && !busy.iter().any(|marker| screen.contains(marker))
}

/// Wait until [`at_rest`] holds. A reply renders before the vendor's turn ends, so the capture that follows it says
/// nothing about whether the composer takes commands yet.
async fn wait_until_at_rest(
    term: &mut Terminal<'_>,
    env: &DriftEnv<'_>,
    id: &str,
    spec: &RotationSpec,
) -> DriftResult {
    let deadline = tokio::time::Instant::now() + TURN_LIMIT;
    loop {
        let screen = term.host.screen_tail(SCREEN_BYTES).to_lowercase();
        term.check(&screen, true).await?;
        let status = env
            .client
            .list_sessions()
            .await?
            .sessions
            .into_iter()
            .find(|session| session.id == id)
            .ok_or_else(|| anyhow::anyhow!("the rotation session vanished"))?
            .status;
        if at_rest(&status, &screen, spec.busy) {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "the agent never came to rest after its turn (status {status:?})\n{}",
                term.diagnostics()
            )
            .into());
        }
        pace().await;
    }
}

/// Type the command until it echoes and its menu matches it, then press Enter until the menu closes. A menu still open
/// means the Enter was lost; a closed one means the vendor took the command. Whether the command actually rotated is
/// left to the checks that follow, which observe the capture.
async fn run_command(
    term: &mut Terminal<'_>,
    spec: &RotationSpec,
    rotation: &Rotation,
) -> DriftResult {
    let command = rotation.command;
    let baseline = Baseline::of(
        &term.host.screen_tail(SCREEN_BYTES).to_lowercase(),
        rotation,
    );
    let deadline = tokio::time::Instant::now() + COMMAND_LIMIT;
    let mut sent: Option<tokio::time::Instant> = None;
    loop {
        let screen = term.host.screen_tail(SCREEN_BYTES).to_lowercase();
        term.check(&screen, true).await?;
        let due = sent.is_none_or(|at| at.elapsed() >= KEY_RETRY);
        match typing_step(&screen, rotation, spec.overlay, &baseline) {
            Typing::Matched => break,
            Typing::Dismiss if due => {
                term.client.send_keys(term.channel, b"\x1b".to_vec()).await;
                sent = Some(tokio::time::Instant::now());
            }
            Typing::Type if due => {
                term.client
                    .send_keys(term.channel, command.as_bytes().to_vec())
                    .await;
                sent = Some(tokio::time::Instant::now());
            }
            Typing::Dismiss | Typing::Type | Typing::Wait => {}
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "the {command} menu line {:?} never rendered for the typed command\n{}",
                rotation.menu,
                term.diagnostics()
            )
            .into());
        }
        pace().await;
    }
    let deadline = tokio::time::Instant::now() + COMMAND_LIMIT;
    let mut pressed: Option<tokio::time::Instant> = None;
    loop {
        let screen = term.host.screen_tail(SCREEN_BYTES).to_lowercase();
        term.check(&screen, true).await?;
        let open = baseline.menu_open(&screen, rotation);
        if pressed.is_some() && !open {
            println!("{command}: the command menu closed after Enter");
            return Ok(());
        }
        if pressed.is_none_or(|at| at.elapsed() >= KEY_RETRY) {
            term.client.send_keys(term.channel, b"\r".to_vec()).await;
            pressed = Some(tokio::time::Instant::now());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "Enter never closed the {command} menu\n{}",
                term.diagnostics()
            )
            .into());
        }
        pace().await;
    }
}

/// Drive a resume command's session picker until Enter closes it: type the target's query, move the highlight to the
/// row showing its word, and press Enter. Keys repeat only after [`KEY_RETRY`], for the same reason as
/// [`run_command`]'s. A repeated Enter that lands after the picker closed reaches the composer a resume leaves empty,
/// unlike a fork picker's (see [`FORK_ENTER_RETRY`]).
async fn pick(
    term: &mut Terminal<'_>,
    command: &str,
    picker: &ResumePicker,
    target: PickTarget<'_>,
) -> DriftResult {
    let (query, word) = (target.query.to_lowercase(), target.word.to_lowercase());
    let lowered = PickTarget {
        query: &query,
        word: &word,
    };
    let deadline = tokio::time::Instant::now() + COMMAND_LIMIT;
    let mut sent: Option<tokio::time::Instant> = None;
    let mut entered = false;
    // Whether the title ever showed, so a timeout can tell a picker that never opened from one that never closed.
    let mut opened = false;
    loop {
        let screen = term.host.screen_tail(SCREEN_BYTES).to_lowercase();
        term.check(&screen, true).await?;
        opened |= screen.contains(picker.title);
        let due = sent.is_none_or(|at| at.elapsed() >= KEY_RETRY);
        let keys: Option<&[u8]> =
            match picking_step(&screen, picker.title, picker.highlight, lowered, entered) {
                Picking::Closed => {
                    println!("{command}: the picker closed on the chosen row");
                    return Ok(());
                }
                Picking::Search if due => Some(target.query.as_bytes()),
                Picking::Down if due => Some(b"\x1b[B"),
                Picking::Select if due => {
                    entered = true;
                    Some(b"\r")
                }
                Picking::Wait | Picking::Search | Picking::Down | Picking::Select => None,
            };
        if let Some(keys) = keys {
            term.client.send_keys(term.channel, keys.to_vec()).await;
            sent = Some(tokio::time::Instant::now());
        }
        if tokio::time::Instant::now() >= deadline {
            let state = match (opened, entered) {
                (false, _) => "the picker never opened",
                (true, false) => "Enter not pressed",
                (true, true) => "Enter pressed",
            };
            return Err(anyhow::anyhow!(
                "the {command} picker {:?} never closed on the {} row ({state})\n{}",
                picker.title,
                target.word,
                term.diagnostics()
            )
            .into());
        }
        pace().await;
    }
}

/// How long a fork picker waits after Enter before it presses Enter again. Longer than [`KEY_RETRY`] because a late
/// Enter is not harmless here: once Pi's picker closes, the composer holds the copied message, and an Enter landing
/// then would submit it as an unplanned turn in the fork. A picker closes locally, without a model turn, so a picker
/// still open this long after Enter means the key was lost.
const FORK_ENTER_RETRY: Duration = Duration::from_secs(6);

/// After a fork command: take the row its picker opened on (where it has one), wait for the vendor's notice that the
/// fork exists, and empty the composer if the picker refilled it. `notices_before` counts the notice on the screen
/// sampled before the command, so vendor text from an earlier fork cannot stand in for this one. `copied` is the
/// latest prompt of the conversation being forked, which Pi's picker opens on and puts back into the composer.
async fn settle_fork(
    term: &mut Terminal<'_>,
    command: &str,
    fork: &Fork,
    notices_before: usize,
    copied: &str,
) -> DriftResult {
    let deadline = tokio::time::Instant::now() + COMMAND_LIMIT;
    let mut entered: Option<tokio::time::Instant> = None;
    let mut opened = false;
    loop {
        let screen = term.host.screen_tail(SCREEN_BYTES).to_lowercase();
        term.check(&screen, true).await?;
        // The notice, not the picker's title going away, ends this phase: it is the vendor saying the session is in
        // the fork, which is what the seed needs, and it also stops the Enter retry below at the earliest evidence.
        if screen.matches(fork.notice).count() > notices_before {
            println!("{command}: the vendor reported the fork");
            break;
        }
        if let Some(picker) = fork.picker
            && screen.contains(picker.title)
        {
            opened = true;
            if entered.is_none_or(|at| at.elapsed() >= FORK_ENTER_RETRY) {
                term.client.send_keys(term.channel, b"\r".to_vec()).await;
                entered = Some(tokio::time::Instant::now());
            }
        }
        if tokio::time::Instant::now() >= deadline {
            let state = match (fork.picker, opened, entered) {
                (None, ..) => "",
                (Some(_), false, _) => " (the fork picker never opened)",
                (Some(_), true, None) => " (Enter not pressed in the fork picker)",
                (Some(_), true, Some(_)) => " (Enter pressed in the fork picker)",
            };
            return Err(anyhow::anyhow!(
                "the fork notice {:?} never appeared after {command}{state}\n{}",
                fork.notice,
                term.diagnostics()
            )
            .into());
        }
        pace().await;
    }
    match fork.picker {
        Some(picker) => clear_copied(term, command, &picker, copied).await,
        None => Ok(()),
    }
}

/// Empty the composer a fork picker refilled with `copied`, and show that it is empty: a seed typed after the copied
/// message would be submitted as one message with it. [`CopyWatch`] decides from the screen; the clear repeats at
/// [`KEY_RETRY`] until it reports the copy gone, since a lost keystroke would otherwise leave the copy in place.
async fn clear_copied(
    term: &mut Terminal<'_>,
    command: &str,
    picker: &ForkPicker,
    copied: &str,
) -> DriftResult {
    let copied = copied.to_lowercase();
    let deadline = tokio::time::Instant::now() + COMMAND_LIMIT;
    let mut watch = CopyWatch::default();
    let mut sent: Option<tokio::time::Instant> = None;
    loop {
        let screen = term.host.screen_tail(SCREEN_BYTES).to_lowercase();
        term.check(&screen, true).await?;
        match watch.observe(screen.matches(copied.as_str()).count()) {
            Clearing::Cleared => {
                println!("{command}: the composer no longer holds the copied message");
                return Ok(());
            }
            Clearing::Clear if sent.is_none_or(|at| at.elapsed() >= KEY_RETRY) => {
                term.client
                    .send_keys(term.channel, picker.clear.to_vec())
                    .await;
                watch.sent();
                sent = Some(tokio::time::Instant::now());
            }
            Clearing::Clear | Clearing::Wait => {}
        }
        if tokio::time::Instant::now() >= deadline {
            let reason = if sent.is_none() {
                "the fork never put the copied message back in the composer (if the vendor stopped doing that, its \
                 descriptor's clear step is obsolete)"
            } else {
                "the composer still holds the copied message after the clear keys"
            };
            return Err(anyhow::anyhow!("{reason} after {command}\n{}", term.diagnostics()).into());
        }
        pace().await;
    }
}

/// After a resume, wait until Restart would resume the first conversation again. Pi and OMP report the switch as it
/// happens, with the file already saved, so no prompt is needed. `previous` is the resume target before the command,
/// which a timeout compares against to say whether the row stayed, moved elsewhere, or lost its target.
async fn wait_for_return(
    term: &mut Terminal<'_>,
    env: &DriftEnv<'_>,
    id: &str,
    log: &Path,
    origin: &Origin,
    previous: &str,
) -> Result<SessionSnapshot, DriftError> {
    let deadline = tokio::time::Instant::now() + START_LIMIT;
    loop {
        term.check(&term.host.screen_tail(SCREEN_BYTES).to_lowercase(), true)
            .await?;
        let snapshot = sample(env, id).await?;
        let capture = Capture::of(&snapshot);
        if capture.returned_to(&origin.selector) {
            return Ok(snapshot);
        }
        if tokio::time::Instant::now() >= deadline {
            let target = match capture.selector {
                Some(selector) if selector == previous => "still the conversation the resume left",
                Some(selector) if selector == origin.selector => {
                    "the first conversation, but not resumable"
                }
                Some(_) => "a conversation other than the first",
                None => "absent",
            };
            return Err(anyhow::anyhow!(
                "the capture never returned to the first conversation (offer {:?}, resume target {target})\n{}\n\
                 hook log tail:\n{}",
                capture.offer,
                term.diagnostics(),
                log_tail(log)
            )
            .into());
        }
        pace().await;
    }
}

/// The conversation the session's first seed turn created, which a resume selects: its code word, which also finds
/// it in the picker, and the resume target the row showed for it.
struct Origin {
    word: String,
    selector: String,
}

/// What the chained rotations carry from one to the next.
struct Chain {
    /// The session's first conversation, for a resume.
    origin: Origin,
    /// The row the previous step left: the first seed turn's, or the last Restart's recall turn's.
    current: SessionSnapshot,
    /// The last prompt submitted in the current conversation. Pi's fork picker opens on it and copies it back into
    /// the composer, so emptying the composer means watching for this text.
    latest_prompt: String,
}

/// Run the command, its picker or fork notice, and its pinned pre-prompt check.
async fn rotate(
    term: &mut Terminal<'_>,
    env: &DriftEnv<'_>,
    id: &str,
    log: &Path,
    spec: &RotationSpec,
    rotation: &Rotation,
    chain: &Chain,
) -> DriftResult {
    let command = rotation.command;
    let before = Capture::of(&chain.current);
    let (Some(conversation), Some(previous)) = (before.conversation, before.selector) else {
        return Err(anyhow::anyhow!(
            "no resumable conversation to rotate away from with {command}"
        )
        .into());
    };
    let previous_runtime = reported_runtime_identity(conversation);
    wait_until_at_rest(term, env, id, spec).await?;
    // Only a fork reads this baseline, but it has to be sampled before the command runs.
    let notices_before = match rotation.operation {
        Operation::Fork(fork) => term
            .host
            .screen_tail(SCREEN_BYTES)
            .to_lowercase()
            .matches(fork.notice)
            .count(),
        Operation::Fresh | Operation::Resume(_) => 0,
    };
    run_command(term, spec, rotation).await?;
    match rotation.operation {
        Operation::Fresh => {}
        Operation::Fork(fork) => {
            settle_fork(term, command, &fork, notices_before, &chain.latest_prompt).await?;
        }
        Operation::Resume(picker) => {
            let query = picker.search.query(&chain.origin.word);
            let target = PickTarget {
                query: &query,
                word: &chain.origin.word,
            };
            pick(term, command, &picker, target).await?;
        }
    }
    match rotation.before_prompt {
        // A fork and a resume waited above for the vendor's own evidence that it switched. After a fresh rotation no
        // readiness wait follows the command, and that is deliberate. Nothing observable says these vendors have
        // finished switching: their reports wait for the next prompt, and the at-rest check above reads a status and
        // running-turn markers that no turn changes during a slash command, so repeating it would prove nothing about
        // the switch. A prompt typed into a vendor still switching cannot pass falsely: a redraw that wipes its
        // echo fails `type_text`, and a prompt that lands in the old conversation leaves the resume target unchanged,
        // which `wait_for_capture` reports. Either failure after a rotation command can be this race rather than drift.
        BeforePrompt::Unspecified => Ok(()),
        BeforePrompt::NewCapture => {
            let deadline = tokio::time::Instant::now() + START_LIMIT;
            loop {
                term.check(&term.host.screen_tail(SCREEN_BYTES).to_lowercase(), true)
                    .await?;
                let text = read_log(log);
                let snapshot = sample(env, id).await?;
                if let Some(rotated) = Capture::of(&snapshot).rotated_from(Some(previous))
                    && committed_report(&text, &reported_runtime_identity(rotated), None)
                {
                    println!(
                        "{command}: a new resumable conversation was captured before the next prompt"
                    );
                    return Ok(());
                }
                if tokio::time::Instant::now() >= deadline {
                    return Err(anyhow::anyhow!(
                        "{command} reported no new resumable conversation before the next prompt\n{}\nhook log tail:\n{}",
                        term.diagnostics(),
                        log_tail(log)
                    )
                    .into());
                }
                pace().await;
            }
        }
        BeforePrompt::OldCaptureStays => {
            let deadline = tokio::time::Instant::now() + BEFORE_PROMPT;
            while tokio::time::Instant::now() < deadline {
                // An agent that died after the command would still leave the old capture in place, so the window
                // would pass on a dead peer; check it is alive before trusting the row.
                term.check(&term.host.screen_tail(SCREEN_BYTES).to_lowercase(), true)
                    .await?;
                // Apply any report the hooks have dropped, so a report that came at the wrong moment cannot hide in
                // the supervisor's queue for the length of this window.
                env.sup.reconcile_for_test().await;
                let snapshot = sample(env, id).await?;
                if !Capture::of(&snapshot).holds(&previous_runtime) {
                    return Err(anyhow::anyhow!(
                        "the capture moved after {command} before the next prompt (offer {:?})\nhook log tail:\n{}",
                        snapshot.restart_offer,
                        log_tail(log)
                    )
                    .into());
                }
                pace().await;
            }
            println!(
                "{command}: the previous conversation stayed the Restart target until the next prompt"
            );
            Ok(())
        }
    }
}

/// After the turn on the new conversation, where a replaced audit pinned it: the `source` of the accepted report that
/// named the new conversation, judged on `text`, the hook log read right after that turn's capture was accepted.
fn confirm(rotation: &Rotation, rotated: &SessionSnapshot, text: &str) -> DriftResult {
    let command = rotation.command;
    let conversation = rotated
        .captured_conversation
        .as_deref()
        .expect("an accepted rotated turn has a conversation");
    let runtime = reported_runtime_identity(conversation);
    if let Some(source) = rotation.source
        && !committed_report(text, &runtime, Some(source))
    {
        return Err(anyhow::anyhow!(
            "no accepted report with source `{source}` named the conversation after {command}\nhook log tail:\n{}",
            tail_of(text)
        )
        .into());
    }
    Ok(())
}

/// What the resumed agent is asked after Restart, and the code word its answer must carry.
#[derive(Clone, Copy, Debug)]
struct Recall<'w> {
    word: &'w str,
    prompt: &'static str,
}

/// Stop, drain the reports, Restart, and require the resumed agent to answer the recall prompt with its code word
/// under the rotated resume arguments, with a fresh accepted report on the new generation. Returns the resumed
/// terminal and row, the starting point of the next rotation.
async fn restart_and_recall<'a>(
    env: &DriftEnv<'a>,
    profile: &Harness,
    id: &str,
    log: &Path,
    rotated: &SessionSnapshot,
    recall: Recall<'_>,
    hook_witness: HookWitness,
) -> Result<(Terminal<'a>, SessionSnapshot), DriftError> {
    // Listing observes the stopped row but does not apply reports. Drain them before taking the baseline so Restart
    // cannot acknowledge a pending callback from the stopped generation and count it as a new-generation witness.
    env.client.stop_session(id).await?;
    env.client.list_sessions().await?;
    env.sup.reconcile_for_test().await;
    let acked_before = acknowledged_reports(&read_log(log)).count();
    let stopped = sample(env, id).await?;
    // Separate Farhelm losing the rotated capture from the vendor failing to resume it.
    let rotated_target = Capture::of(rotated).selector;
    let stopped_capture = Capture::of(&stopped);
    if stopped_capture.offer != RestartOffer::Resume || stopped_capture.selector != rotated_target {
        return Err(anyhow::anyhow!(
            "after Stop, Restart no longer targets the rotated conversation (offer {:?})\nhook log tail:\n{}",
            stopped_capture.offer,
            log_tail(log)
        )
        .into());
    }
    env.client.restart_session(id, false).await?;
    let resumed = sample(env, id).await?;
    if resumed.generation <= rotated.generation {
        return Err(anyhow::anyhow!("Restart did not start a new generation").into());
    }
    let mut term = Terminal::attach(env.sup, env.client, id, true).await?;
    term.ready(profile, TrustPolicy::AutoAnswer, true).await?;
    let reply = format!("RECALL-{}", recall.word);
    let expected = TurnExpectation {
        reply: &reply,
        acked_before,
        original_argv: Some(
            rotated
                .resume_argv
                .as_deref()
                .expect("an accepted rotated turn has resume argv"),
        ),
        hook_witness: match hook_witness {
            HookWitness::CommittedIdentity { .. } => HookWitness::CommittedIdentity {
                resumed_generation: Some(resumed.generation),
            },
            HookWitness::Fresh | HookWitness::MatchingIdentity => hook_witness,
        },
    };
    if expected.saw_reply(&term.host.screen_tail(SCREEN_BYTES)) {
        return Err(anyhow::anyhow!(
            "the recall witness was already present before the recall prompt"
        )
        .into());
    }
    term.type_text(recall.prompt, true).await?;
    term.client.send_keys(term.channel, b"\r".to_vec()).await;
    let recalled = wait_for_turn(&mut term, env.sup, id, log, &expected).await?;
    Ok((term, recalled))
}

/// The whole lifecycle in one session: an optional startup capture, a seed turn, then each rotation with its pinned
/// checks, the new conversation's proof (a seed turn, or a resume's return to the first conversation), and Restart
/// with recall.
async fn rotate_session<'a>(
    env: &DriftEnv<'a>,
    profile: &Harness,
    spec: &RotationSpec,
    hook_witness: HookWitness,
    id: &str,
) -> DriftResult {
    let log = env.state.join("hook-log").join(format!("{id}.log"));
    let mut term = Terminal::attach(env.sup, env.client, id, false).await?;
    term.ready(profile, TrustPolicy::AutoAnswer, false).await?;
    if let Some(source) = spec.startup_source {
        startup_capture(&mut term, env, id, &log, source).await?;
        println!("captured a `{source}` report before the first prompt");
    }
    let first_word = code_word();
    let first = seed(
        &mut term,
        env,
        id,
        &log,
        &first_word,
        hook_witness,
        None,
        false,
    )
    .await?;
    let mut chain = Chain {
        origin: Origin {
            selector: Capture::of(&first)
                .selector
                .expect("an accepted seed turn has a resume target")
                .to_owned(),
            word: first_word.clone(),
        },
        current: first,
        latest_prompt: seed_prompt(&first_word),
    };
    for rotation in spec.rotations {
        let command = rotation.command;
        rotate(&mut term, env, id, &log, spec, rotation, &chain).await?;
        let previous = Capture::of(&chain.current).selector;
        let (rotated, word) = match rotation.operation {
            Operation::Fresh | Operation::Fork(_) => {
                let word = code_word();
                let rotated = seed(
                    &mut term,
                    env,
                    id,
                    &log,
                    &word,
                    hook_witness,
                    previous,
                    true,
                )
                .await?;
                println!("{command}: the next turn committed a different conversation");
                (rotated, word)
            }
            Operation::Resume(_) => {
                let previous = previous.expect("`rotate` refused a row without a resume target");
                let returned =
                    wait_for_return(&mut term, env, id, &log, &chain.origin, previous).await?;
                println!("{command}: the first conversation was captured again before any prompt");
                (returned, chain.origin.word.clone())
            }
        };
        // The `source` check runs after Restart on purpose: a relabelled `source` is vendor drift Farhelm does not act
        // on, so it must not hide whether the user-visible contract (Restart resumes the rotated conversation) still
        // holds. It judges the log as it stands right after that turn, though, so the resumed launch's own reports
        // cannot supply the source the rotation's report lacked.
        let rotated_log = read_log(&log);
        let recall = Recall {
            word: &word,
            prompt: rotation.operation.recall_prompt(),
        };
        let (resumed_term, resumed) =
            restart_and_recall(env, profile, id, &log, &rotated, recall, hook_witness).await?;
        println!("{command}: Restart resumed the rotated conversation and recalled its word");
        confirm(rotation, &rotated, &rotated_log)?;
        term = resumed_term;
        chain.current = resumed;
        chain.latest_prompt = recall.prompt.to_owned();
    }
    Ok(())
}

/// Use the base category's launch choices, so the rotation is the only change under test.
async fn execute(harness: LaunchHarness) -> DriftResult {
    let spec =
        rotation_spec(harness).expect("a rotation case names a harness with a rotation spec");
    let profile = profile(harness).expect("a rotation case has a drift profile");
    let hook_witness = profile
        .hook_witness
        .expect("a rotation case reports a conversation");
    let selection = LaunchSelection {
        custom_agent: None,
        harness,
        model: profile.model.map(str::to_owned),
        effort: profile.effort.map(Into::into),
        permissions: None,
        permission_flags: None,
        wrapper_prefix: None,
        container: None,
        workspace_trust: profile.trust_at_launch.then_some(true),
    };
    with_drift_supervisor(&profile, selection, None, async |env, launch| {
        env.with_session(launch, CreateExtras::default(), async |id| {
            rotate_session(&env, &profile, &spec, hook_witness, id).await
        })
        .await
    })
    .await
}

drift_cases! {
    execute;
    drift_claude => Claude,
    drift_codex => Codex,
    drift_muse => Muse,
    drift_pi => Pi,
    drift_omp => Omp,
    drift_grok => Grok,
    drift_antigravity => Antigravity,
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::RestartReadiness;

    /// The category must cover exactly the harnesses whose rotation the plan settled, and any harness that gains a
    /// descriptor must also gain a live case (and the reverse), so coverage cannot drift silently. Specified: the
    /// harnesses with a descriptor are exactly the registered cases, which are Claude, Codex, Muse, Pi, OMP, Grok and
    /// Antigravity, and each has a tracked drift profile and at least one command.
    #[farhelm_testtrace::test]
    fn rotation_covers_the_settled_harnesses_and_each_has_a_case() {
        let described: Vec<_> = LaunchHarness::ALL
            .iter()
            .copied()
            .filter(|harness| rotation_spec(*harness).is_some())
            .collect();
        assert_eq!(described.len(), HARNESSES.len(), "{described:?}");
        for harness in &described {
            assert!(
                HARNESSES.contains(harness),
                "{harness:?} has a descriptor but no case"
            );
            let profile = profile(*harness).expect("a described harness has a drift profile");
            assert!(
                profile.hook_witness.is_some(),
                "{harness:?} reports no conversation"
            );
            assert!(!rotation_spec(*harness).unwrap().rotations.is_empty());
        }
        use LaunchHarness::*;
        for harness in [Claude, Codex, Muse, Pi, Omp, Grok, Antigravity] {
            assert!(described.contains(&harness), "{harness:?}");
        }
    }

    /// Pre-prompt timing checks rest on when the vendor reports. A check set where the product's own declared timing
    /// contradicts it would fail live, or pass for the wrong reason. Specified: a startup source and a new capture on
    /// the command itself only where capture precedes input; the old capture staying only where the report waits for
    /// the first prompt; and the menu line never contains the command, so the echo and the menu remain independent
    /// evidence.
    #[farhelm_testtrace::test]
    fn pre_prompt_checks_agree_with_the_declared_capture_timing() {
        for harness in LaunchHarness::ALL.iter().copied() {
            let Some(spec) = rotation_spec(harness) else {
                continue;
            };
            let readiness = harness.agent_kind().restart_readiness();
            let at_start = matches!(
                readiness,
                Some(RestartReadiness::AtStart | RestartReadiness::ShortlyAfterStart)
            );
            assert!(spec.startup_source.is_none() || at_start, "{harness:?}");
            for rotation in spec.rotations {
                assert!(
                    !rotation.menu.contains(rotation.command),
                    "{harness:?} {rotation:?}"
                );
                assert_eq!(rotation.menu, rotation.menu.to_lowercase(), "{harness:?}");
                match rotation.before_prompt {
                    BeforePrompt::Unspecified => {}
                    BeforePrompt::NewCapture => assert!(at_start, "{harness:?}"),
                    BeforePrompt::OldCaptureStays => assert_eq!(
                        readiness,
                        Some(RestartReadiness::FirstPrompt),
                        "{harness:?}"
                    ),
                }
            }
        }
    }

    /// A resume proves something only when it leaves a conversation for an earlier one, and it finds that one by the
    /// first conversation's word, which a fork also carries in its opening message. A reordered chain would either
    /// resume the conversation the session is already in or face two rows with the same text. Specified: Pi and OMP
    /// each cover `/new`, `/resume` and `/fork`; in every spec a resume comes after a fresh rotation and before any
    /// fork; and every picker string the driver matches against the lowercased screen is lowercase.
    #[farhelm_testtrace::test]
    fn a_resume_follows_a_new_conversation_and_precedes_any_fork() {
        for harness in LaunchHarness::ALL.iter().copied() {
            let Some(spec) = rotation_spec(harness) else {
                continue;
            };
            let mut fresh_seen = false;
            let mut fork_seen = false;
            for rotation in spec.rotations {
                match rotation.operation {
                    Operation::Fresh => fresh_seen = true,
                    Operation::Fork(fork) => {
                        fork_seen = true;
                        assert_eq!(fork.notice, fork.notice.to_lowercase(), "{harness:?}");
                        if let Some(picker) = fork.picker {
                            assert_eq!(picker.title, picker.title.to_lowercase());
                        }
                    }
                    Operation::Resume(picker) => {
                        assert!(fresh_seen && !fork_seen, "{harness:?} {rotation:?}");
                        assert_eq!(picker.title, picker.title.to_lowercase());
                        for marker in picker.highlight {
                            assert_eq!(*marker, marker.to_lowercase());
                        }
                    }
                }
            }
        }
        for harness in [LaunchHarness::Pi, LaunchHarness::Omp] {
            let commands: Vec<_> = rotation_spec(harness)
                .unwrap()
                .rotations
                .iter()
                .map(|rotation| rotation.command)
                .collect();
            assert_eq!(commands, ["/new", "/resume", "/fork"], "{harness:?}");
        }
    }

    /// Enter in a session picker opens whatever row is highlighted, so a fuzzy near match above the target would
    /// silently resume the wrong conversation; and the search box echoes the word, so it must not read as a row.
    /// Specified, on screens shaped like Pi 1.6.1's and OMP 19.2.1's: no title means wait, or closed once Enter was
    /// pressed; an open picker without the query echoed asks for the query; a highlighted target row selects; a listed
    /// target under another highlighted row moves down; and an echo alone or an empty list waits.
    #[farhelm_testtrace::test]
    fn a_session_picker_opens_only_the_highlighted_target_row() {
        let title = "resume session (current folder)";
        let pi = &["› "][..];
        let omp = &["\u{f054} "][..];
        let phrase = PickTarget {
            query: "\"fhaaaa1111\"",
            word: "fhaaaa1111",
        };
        let fuzzy = PickTarget {
            query: "fhaaaa1111",
            word: "fhaaaa1111",
        };

        let history =
            "remember test label fhbbbb2222. reply with only that label. no tools.\n fhbbbb2222\n";
        assert_eq!(
            picking_step(history, title, pi, phrase, false),
            Picking::Wait
        );
        assert_eq!(
            picking_step(history, title, pi, phrase, true),
            Picking::Closed
        );

        let open = format!(
            "{history}resume session (current folder)   ◉ current folder | ○ all\n\
             tab scope · re:<pattern> regex · \"phrase\" exact\n>\n\
             › remember test label fhbbbb2222. reply with only that label. no tools.   3 now\n"
        );
        assert_eq!(
            picking_step(&open, title, pi, phrase, false),
            Picking::Search
        );
        let found = format!(
            "{history}resume session (current folder)\n> \"fhaaaa1111\"\n\
             › remember test label fhaaaa1111. reply with only that label. no tools.   3 1m\n"
        );
        assert_eq!(
            picking_step(&found, title, pi, phrase, false),
            Picking::Select
        );
        let filtering = format!("{history}resume session (current folder)\n> \"fhaaaa1111\"\n");
        assert_eq!(
            picking_step(&filtering, title, pi, phrase, false),
            Picking::Wait
        );

        let near_match = "╭─ resume session (current folder) ─╮\n│ > fhaaaa1111\n\
             │ \u{f054} remember test label fh0a1a2a3a. reply with only that label. no tools.\n\
             │   just now  ·  4.2kb  ·  current\n\
             │   remember test label fhaaaa1111. reply with only that label. no tools.\n\
             │   just now  ·  4.1kb\n";
        assert_eq!(
            picking_step(near_match, title, omp, fuzzy, false),
            Picking::Down
        );
        let moved = "╭─ resume session (current folder) ─╮\n│ > fhaaaa1111\n\
             │   remember test label fh0a1a2a3a. reply with only that label. no tools.\n\
             │ \u{f054} remember test label fhaaaa1111. reply with only that label. no tools.\n";
        assert_eq!(
            picking_step(moved, title, omp, fuzzy, true),
            Picking::Select
        );
        let none = "╭─ resume session (current folder) ─╮\n│ > fhaaaa1111\n\
             │ no sessions in current folder. press tab to view all.\n";
        assert_eq!(picking_step(none, title, omp, fuzzy, false), Picking::Wait);
    }

    /// A resume must leave Restart opening the first conversation again; a row still on the second one, or on any
    /// other, means the switch was not reported. Specified: a resumable row targeting the first conversation has
    /// returned; another target or a withdrawn offer has not.
    #[farhelm_testtrace::test]
    fn a_resume_returns_only_to_a_resumable_first_target() {
        assert!(
            capture(Some("pi:1"), RestartOffer::Resume, Some("/s/1.jsonl"))
                .returned_to("/s/1.jsonl")
        );
        assert!(
            !capture(Some("pi:2"), RestartOffer::Resume, Some("/s/2.jsonl"))
                .returned_to("/s/1.jsonl")
        );
        assert!(!capture(Some("pi:1"), RestartOffer::NotCaptured, None).returned_to("/s/1.jsonl"));
    }

    /// Pi refills the composer with the message it forked at, and a seed typed beside it would be sent as one message
    /// with it, while a clear sent before the refill would leave the refill in place. Specified: no copy seen waits;
    /// a copy seen asks for the clear; a fall in the count before any clear was sent is not taken as cleared; after a
    /// clear, a count rising past what was seen (a late refill beside the same text in history) keeps clearing, and a
    /// count falling below the most seen is cleared.
    #[farhelm_testtrace::test]
    fn a_refilled_composer_counts_as_empty_only_after_its_copy_is_seen_and_falls() {
        let mut watch = CopyWatch::default();
        assert_eq!(watch.observe(0), Clearing::Wait);
        assert_eq!(watch.observe(1), Clearing::Clear);
        assert_eq!(
            watch.observe(0),
            Clearing::Clear,
            "no clear was sent yet, so this fall is not its effect"
        );
        watch.sent();
        assert_eq!(watch.observe(1), Clearing::Clear);
        assert_eq!(watch.observe(2), Clearing::Clear, "a late refill");
        assert_eq!(watch.observe(1), Clearing::Cleared);

        let mut empty = CopyWatch::default();
        empty.sent();
        assert_eq!(
            empty.observe(0),
            Clearing::Wait,
            "a clear sent before any copy proves nothing"
        );
    }

    fn clear() -> Rotation {
        fresh("/clear", "start a new session with empty context")
    }

    /// Chrome that already names the command must not count as typed input, or the driver would press Enter at an
    /// empty composer and never type the command. Specified: before typing, a screen naming `/clear` in a tip asks for
    /// typing; a second occurrence alone waits for the menu; the echo plus the menu line beyond their baselines is a
    /// match; and a menu line already in the baseline proves nothing.
    #[farhelm_testtrace::test]
    fn slash_command_evidence_is_counted_against_the_screen_before_typing() {
        let rotation = clear();
        let before = "tip: use /clear to start fresh\n❯ \n";
        let baseline = Baseline::of(before, &rotation);
        assert_eq!(typing_step(before, &rotation, &[], &baseline), Typing::Type);
        let echoed = "tip: use /clear to start fresh\n❯ /clear\n";
        assert_eq!(typing_step(echoed, &rotation, &[], &baseline), Typing::Wait);
        let matched = "tip: use /clear to start fresh\n  /clear  start a new session with empty context\n❯ /clear\n";
        assert_eq!(
            typing_step(matched, &rotation, &[], &baseline),
            Typing::Matched
        );
        assert!(baseline.menu_open(matched, &rotation));

        let menu_in_chrome = "start a new session with empty context\n❯ \n";
        let baseline = Baseline::of(menu_in_chrome, &rotation);
        assert!(!baseline.menu_open(menu_in_chrome, &rotation));
        assert_eq!(
            typing_step(
                "start a new session with empty context\n❯ /clear\n",
                &rotation,
                &[],
                &baseline
            ),
            Typing::Wait
        );
    }

    /// A retype while the menu is up would leave `/clear/clear` in the composer, which matches no command and strands
    /// the case. Specified: a visible menu line without a counted echo waits rather than typing again.
    #[farhelm_testtrace::test]
    fn an_open_menu_without_a_counted_echo_is_not_retyped() {
        let rotation = clear();
        let baseline = Baseline::of("/clear\n❯ \n", &rotation);
        let screen = "  start a new session with empty context\n❯ /clear\n";
        assert_eq!(typing_step(screen, &rotation, &[], &baseline), Typing::Wait);
    }

    /// Codex's rate-limit notice covers the composer after a reply. Only the complete notice is dismissed, so ordinary
    /// text mentioning rate limits never sends Escape into a composer that holds the command. Specified: all markers
    /// visible means dismiss, even over a typed command; one marker alone does not.
    #[farhelm_testtrace::test]
    fn the_overlay_is_dismissed_only_when_all_its_markers_are_visible() {
        let spec = rotation_spec(LaunchHarness::Codex).unwrap();
        let rotation = spec.rotations[0];
        let baseline = Baseline::of("› \n", &rotation);
        let overlay = "approaching rate limits\nswitch to a cheaper model?\nenter confirm · esc back\n› /new\n";
        assert_eq!(
            typing_step(overlay, &rotation, spec.overlay, &baseline),
            Typing::Dismiss
        );
        let mention = "we are approaching rate limits soon\n› \n";
        assert_eq!(
            typing_step(mention, &rotation, spec.overlay, &baseline),
            Typing::Type
        );
    }

    /// A command sent while the vendor still runs a turn is refused, yet the menu closes as if it ran. Specified: at
    /// rest needs an Idle or Waiting status and no running-turn marker on screen; a Running status, or Idle beside
    /// Muse's "esc to interrupt" hint, is not at rest; a harness with no markers relies on the status alone.
    #[farhelm_testtrace::test]
    fn a_command_waits_for_both_the_status_and_the_vendors_running_turn_hint() {
        let busy = rotation_spec(LaunchHarness::Muse).unwrap().busy;
        let working = "◆ fh1a2b3c4d\n◈ double checking (3s · esc to interrupt)\n❯ \n";
        let quiet = "◆ fh1a2b3c4d\n❯ \n";
        assert!(at_rest(&SessionStatus::Idle, quiet, busy));
        assert!(at_rest(&SessionStatus::Waiting, quiet, busy));
        assert!(!at_rest(&SessionStatus::Idle, working, busy));
        assert!(!at_rest(&SessionStatus::Running, quiet, busy));
        assert!(at_rest(&SessionStatus::Idle, working, &[]));
    }

    /// The row a rotation leaves, in the shapes the checks distinguish.
    fn capture<'a>(
        conversation: Option<&'a str>,
        offer: RestartOffer,
        selector: Option<&'a str>,
    ) -> Capture<'a> {
        Capture {
            conversation,
            offer,
            selector,
        }
    }

    /// A rotation counts only when Restart would open a different conversation. Specified: a resumable capture with a
    /// new resume target is rotated; the same target (a locator that only gained file evidence), a withdrawn offer and
    /// a missing target are not; with no previous target any resumable capture counts.
    #[farhelm_testtrace::test]
    fn a_rotated_capture_needs_a_resumable_different_target() {
        let moved = capture(Some("conv-2"), RestartOffer::Resume, Some("conv-2"));
        assert_eq!(moved.rotated_from(Some("conv-1")), Some("conv-2"));
        assert_eq!(moved.rotated_from(None), Some("conv-2"));
        let enriched = capture(
            Some("conv-1-with-file"),
            RestartOffer::Resume,
            Some("conv-1"),
        );
        assert_eq!(enriched.rotated_from(Some("conv-1")), None);
        let withdrawn = capture(Some("conv-2"), RestartOffer::NotCaptured, None);
        assert_eq!(withdrawn.rotated_from(Some("conv-1")), None);
        let untargeted = capture(Some("conv-2"), RestartOffer::Resume, None);
        assert_eq!(untargeted.rotated_from(Some("conv-1")), None);
    }

    /// The negative pre-prompt window compares the hook's runtime ID, which a Codex locator keeps beside its persistent
    /// thread, so file evidence added to the same conversation does not read as a move. Specified: the old capture
    /// holds while it stays resumable under its own runtime; a withdrawn offer does not hold, and neither does another
    /// runtime, even one that is resumable at once (the shape a premature `/new` report would leave).
    #[farhelm_testtrace::test]
    fn the_old_capture_holds_only_while_resumable_under_its_runtime() {
        let old = r#"codex:{"version":1,"runtime_session_id":"run-1","session_file":"/x/1.jsonl","thread_id":"t-1","resumable":true}"#;
        let other = r#"codex:{"version":1,"runtime_session_id":"run-2","session_file":null,"thread_id":null,"resumable":false}"#;
        let other_resumable = r#"codex:{"version":1,"runtime_session_id":"run-2","session_file":"/x/2.jsonl","thread_id":"t-2","resumable":true}"#;
        assert!(capture(Some(old), RestartOffer::Resume, Some("t-1")).holds("run-1"));
        assert!(
            !capture(Some(other_resumable), RestartOffer::Resume, Some("t-2")).holds("run-1"),
            "a resumable capture under another runtime is a move"
        );
        assert!(!capture(Some(old), RestartOffer::NotCaptured, None).holds("run-1"));
        assert!(!capture(Some(other), RestartOffer::NotCaptured, None).holds("run-1"));
        assert!(!capture(None, RestartOffer::NotCaptured, None).holds("run-1"));
    }

    /// The hook writes a line for every callback, so only the supervisor's committed acknowledgment proves admission,
    /// and the source check must read the acknowledged report's own field. Specified: a committed ack for the runtime
    /// with the expected source matches; a written line, an ignored ack, another runtime or another source do not;
    /// a source-less Stop is matched by `-`.
    #[farhelm_testtrace::test]
    fn only_a_committed_acknowledgment_names_an_admitted_report() {
        let log = "1 written run-2 clear\n\
                   2 acked run-2 clear\tcommitted gen=4\n\
                   3 written run-2 -\tevent=Stop\n\
                   4 acked run-2 -\tcommitted gen=4\n\
                   5 acked run-3 startup\tignored\n";
        assert!(committed_report(log, "run-2", Some("clear")));
        assert!(committed_report(log, "run-2", Some("-")));
        assert!(committed_report(log, "run-2", None));
        assert!(!committed_report(log, "run-2", Some("startup")));
        assert!(
            !committed_report(log, "run-3", None),
            "an ignored ack changed nothing"
        );
        assert!(!committed_report(
            "1 written run-2 clear\n",
            "run-2",
            Some("clear")
        ));
        assert!(!committed_report(log, "run-9", None));
    }

    /// Each witness must come from the model, never from text the test typed or an earlier turn showed. Specified:
    /// the echoed seed and both recall prompts contain no line that reads as either reply, while the replies
    /// themselves do.
    #[farhelm_testtrace::test]
    fn the_prompt_echoes_cannot_supply_their_own_replies() {
        let word = "FH1A2B3C4D";
        let recall = format!("RECALL-{word}");
        let echo = format!(
            "❯ {}\n› {ROTATION_RECALL_PROMPT}\n{ROTATION_RECALL_PROMPT}\n{FORK_RECALL_PROMPT}\n› {FORK_RECALL_PROMPT}\n",
            seed_prompt(word)
        );
        assert!(!reply_on_screen(&echo, word));
        assert!(!reply_on_screen(&echo, &recall));
        assert!(reply_on_screen(&format!("{echo}● {word}\n"), word));
        assert!(reply_on_screen(&format!("{echo}● {recall}\n"), &recall));
    }
}
