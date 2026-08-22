//! The fake agent: a deterministic scripted TUI standing in for Claude
//! Code / Codex in tests.
//!
//! Why it exists (PLAN_M1.md): end-to-end tests — including replay and
//! mode restoration — must run without vendor auth, tokens, or
//! nondeterminism. The scripts exercise exactly the terminal behaviors
//! the real agents use: colored output, a prompt that echoes input,
//! bracketed paste mode, the alternate screen, and output that continues
//! while a client reattaches. Real-agent smoke testing stays manual.
//!
//! Contract with tests: every script prints `FAKE-AGENT READY` once
//! its modes are set and it is listening — tests key on that marker
//! instead of sleeping.

use anyhow::Context;
use std::io::{BufRead, Read, Write};

/// Which terminal behavior to act out. A closed set, so clap validates it
/// at parse time and `--help` documents it, rather than failing at
/// runtime on a typo in a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Script {
    /// Prompt-and-echo with color and bracketed paste enabled.
    Basic,
    /// Full-screen app on the alternate screen.
    Altscreen,
    /// Like `Altscreen`, but also spawns a SIGTERM-ignoring child before
    /// installing its own restore-and-exit handler. The acceptance
    /// fixture for a mid-stop attach landing between the pane going dead
    /// and `StopSession` actually publishing its snapshot: this process's
    /// OWN pid restores the primary screen and exits within milliseconds
    /// of SIGTERM (the pane goes dead almost immediately), while
    /// `kill_process_tree` must still run its full SIGSTOP-quiesce-then-
    /// SIGKILL escalation against the lingering child — several hundred
    /// milliseconds during which the pane is dead but the stop itself has
    /// not finished. See `Supervisor::pending_snapshots`'s docs
    /// (farhelm-supervisor's service.rs) for the gap this fixture
    /// exercises.
    AltscreenStubbornChild,
    /// Enters the alternate screen and IGNORES SIGTERM entirely
    /// (`SIG_IGN`, no handler at all — never restores the primary
    /// screen). The acceptance fixture for `StopSession` actually
    /// finishing a stop against an alt-screen app that never cooperates:
    /// `kill_process_tree` must run its full grace/SIGSTOP-quiesce/SIGKILL
    /// escalation against THIS process itself (SIGTERM does nothing;
    /// SIGKILL — which cannot be ignored — is what finally ends it), and
    /// it dies still on the alternate screen, with no restore. This is
    /// the case the alt-screen stop-snapshot feature exists for: without
    /// it, reattaching afterward would find nothing tmux itself could
    /// still show (see the `Attach` handler's snapshot-existence gate in
    /// service.rs for why that is, empirically, not "the ordinary prefill
    /// already has it").
    AltscreenIgnoresTerm,
    /// Like `AltscreenIgnoresTerm`, but this process itself uses the
    /// DEFAULT SIGTERM disposition (dies within milliseconds, still on
    /// the alternate screen, no restore — same end state as
    /// `AltscreenIgnoresTerm`, reached the fast way instead) while a
    /// spawned child ignores SIGTERM and lingers. Combines
    /// `AltscreenStubbornChild`'s "pane goes dead almost immediately,
    /// `kill_process_tree` keeps running against the child for hundreds
    /// of milliseconds more" window with `AltscreenIgnoresTerm`'s
    /// dies-still-on-the-alternate-screen end state — the acceptance
    /// fixture for a mid-stop attach landing in that window while the
    /// dead pane is STILL on the alternate screen (as opposed to
    /// `AltscreenStubbornChild`'s already-restored-to-primary case),
    /// pinning that the `\x1b[?1049l` alt-exit escape and the
    /// `Supervisor::pending_snapshots` fallback compose correctly.
    AltscreenStubbornChildStaysAlt,
    /// Raw non-UTF-8 output for byte-fidelity tests.
    Binary,
    /// Numbered records emitted as fast as the pty will take them, for
    /// backpressure tests (PLAN_M2_5.md). Deliberately unlike `Counter`,
    /// which paces itself: this one exists to be FASTER than any consumer
    /// on the path and to emit far more bytes than every bound on it
    /// combined, so that pausing it genuinely provokes tmux's own
    /// `pause-after` rather than being absorbed by a buffer somewhere.
    /// See `flood`'s own docs for the record shape tests key on.
    Flood,
    /// Like `Flood`, but blocks after `FAKE-AGENT READY` until it has read
    /// exactly one input byte before emitting anything. See `flood_gated`'s
    /// own docs for why this is a SEPARATE script rather than a flag on
    /// `Flood` itself.
    FloodGated,
    /// Continuous numbered records for replay/live cutover tests.
    Counter,
    /// Raw-mode hex echo of every input byte, for input-fidelity tests.
    Hexecho,
    /// Enables mouse reporting on cue (`legacy` for DECSET 1000 alone,
    /// `sgr` to add DECSET 1006 on top), while hex-echoing every input
    /// byte like [`Script::Hexecho`]. The reattach-restoration fixture for
    /// `PaneModes`' mouse fields (PLAN_M6_5.md item 2) — see
    /// [`mouse_modes`]'s own docs for why the two cues are separate and
    /// how the echo is shared with `Hexecho` rather than reimplemented.
    MouseModes,
    /// Spawns a child process and prints both pids, for process-tree-kill
    /// tests.
    Spawner,
    /// Accepts `spawn <cwd>` on stdin and runs this binary's `spawn`
    /// command as the current session, reporting the child id. The
    /// `spawn-parented <cwd>` form supplies this session's id explicitly,
    /// so browser tests can observe the public parent filter too.
    ///
    /// The spawned child inherits this same profile and script but remains
    /// idle until driven, which keeps the fixture deterministic instead of
    /// recursively creating descendants on launch.
    Spawn,
    /// Like `Spawner`, but the child ignores SIGTERM — the acceptance
    /// subject for the SIGKILL half of `kill_process_tree`'s sequence.
    /// Its child writes `stubborn-ready` in the session's working
    /// directory once the trap is actually installed, since a test
    /// cannot otherwise observe when that has happened (the child's own
    /// stdio is not connected to the terminal).
    SpawnerStubborn,
    /// A doubly-forked daemon that reparents to init while still carrying
    /// the session's environment marker — see `spawner_reparent`'s docs.
    /// The acceptance fixture for the marker-scan half of
    /// `kill_process_tree`.
    SpawnerReparent,
    /// A daemon that is invisible to BOTH halves of `kill_process_tree` —
    /// see `spawner_cloaked`'s docs. The acceptance fixture for the cgroup
    /// hardening (PLAN_M3.md item 10), and the ONE process shape whose
    /// death proves a scope kill happened rather than a sweep.
    SpawnerCloaked,
    /// A child that ignores both SIGTERM and SIGHUP (so it keeps running,
    /// and keeps forking, straight through `kill_process_tree`'s grace
    /// period rather than dying to the very first signal OR to the
    /// SIGHUP cascade the kernel sends this process's whole foreground
    /// group once the pane's session-leader process dies) and
    /// continuously forks new marked grandchildren, each of which (also
    /// deliberately) outlives the sweep's own timing — the acceptance
    /// fixture for the SIGSTOP-quiesce phase: without it, a fork landing
    /// in the gap between rounds could slip past the sweep entirely and
    /// survive indefinitely. All three properties are load-bearing
    /// (verified empirically): a plain child that dies to SIGTERM or the
    /// SIGHUP cascade stops forking almost immediately, and a
    /// short-enough-lived grandchild dies on its own regardless of
    /// whether the sweep ever reaches it — any one of these alone would
    /// let this fixture pass even with quiescing removed, the opposite of
    /// what it exists to catch.
    ///
    /// Self-expiring regardless: the forking loop is bounded to ~120s of
    /// total runtime and each grandchild to a 120s lifetime, not an
    /// unbounded `while true`/`sleep 3600` — a test that fails before
    /// ever calling stop must not leak processes that run indefinitely.
    /// A test-side drop guard is the primary cleanup; this is the
    /// backstop under it.
    SpawnerForkStorm,
    /// Writes a Claude-Code-shaped conversation record on first input, then
    /// echoes like `Basic`. See [`record_agent`] for the whole contract —
    /// including the `append` and `fork` commands that stand in for a
    /// resume and an explicit fork.
    ClaudeRecord,
    /// Writes a Codex-shaped rollout record on first input; otherwise
    /// identical to [`Script::ClaudeRecord`].
    CodexRecord,
    /// Echoes one rc-file-sourced environment variable
    /// ([`RC_MARKER_VAR`]) at startup, then behaves like [`Script::Basic`].
    ///
    /// The fixture for SPEC.md's "the environment is evaluated at each
    /// launch: edit your rc files and the next launch or restart sees the
    /// change" (PLAN_M3.md acceptance 9). A test points the launch at a
    /// private `HOME` whose rc files export that variable, rewrites them
    /// between launches, and reads the two different values back off this
    /// script's own output — no test-process environment is touched at any
    /// point (see `SupervisorSeams::launch_env`).
    EnvEcho,
}

/// The variable [`Script::EnvEcho`] reports, exported by the rc files a
/// test writes into its private `HOME`. Named here so the fixture and its
/// tests cannot drift apart, the same discipline the record markers below
/// follow.
pub const RC_MARKER_VAR: &str = "FARHELM_RC_MARKER";

/// The variable that tells a record-writing script which conversation it
/// is RESUMING, standing in for the `--resume <id>` a real agent takes on
/// its command line.
///
/// It is an environment variable rather than a flag for a mundane reason
/// with a real consequence: this binary's argument parser lives in
/// `main.rs`, and the restart tests must be able to substitute a
/// conversation id into an argv slot WITHOUT changing that parser. A test's
/// resume template therefore ends in a tiny `sh -c` wrapper that moves the
/// substituted argv element into this variable before exec'ing the fixture
/// — which still exercises exactly what matters (item 7's `{conversation}`
/// element is substituted as its own slot, never spliced into a string,
/// and the value reaches the relaunched process).
pub const RESUME_ENV_VAR: &str = "FARHELM_FAKE_AGENT_RESUME";

/// Act out one script and exit. Runs synchronously on blocking stdio on
/// purpose: this stands in for a real agent's terminal behavior, and
/// nothing about it should depend on an async runtime being present.
///
/// `record_home` is the root the record-writing scripts hang their
/// `.claude`/`.codex` trees off — the fixture side of the supervisor's own
/// injectable agent home (`SupervisorSeams::agent_home`). A flag rather
/// than `$HOME` because this repo's tests never mutate the test process's
/// environment, and because several harnesses run concurrently and would
/// otherwise share one tree. `None` is every other script, which writes no
/// records at all.
pub fn run(script: Script, record_home: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    match script {
        Script::Basic => basic(),
        Script::Altscreen => altscreen(),
        Script::AltscreenStubbornChild => altscreen_stubborn_child(),
        Script::AltscreenIgnoresTerm => altscreen_ignores_term(),
        Script::AltscreenStubbornChildStaysAlt => altscreen_stubborn_child_stays_alt(),
        Script::Binary => binary(),
        Script::Flood => flood(),
        Script::FloodGated => flood_gated(),
        Script::Counter => counter(),
        Script::Hexecho => hexecho(),
        Script::MouseModes => mouse_modes(),
        Script::Spawner => spawn_and_echo("sleep 3600", "spawner"),
        Script::Spawn => spawn_session(),
        Script::SpawnerStubborn => spawn_and_echo(
            "trap '' TERM; touch stubborn-ready; sleep 3600",
            "spawner-stubborn",
        ),
        Script::SpawnerReparent => spawner_reparent(),
        Script::SpawnerCloaked => spawner_cloaked(),
        Script::SpawnerForkStorm => spawn_and_echo(
            // Self-expiring: the outer loop runs at most 2400 iterations
            // (~120s at the 0.05s step) rather than forever, and each
            // grandchild lives at most 120s rather than an hour — both
            // bounds exist so a test that fails before ever calling stop
            // (an assertion panic, say) cannot leak processes that run
            // indefinitely; the e2e test's own drop guard is the primary
            // cleanup, this is the defense-in-depth backstop under it.
            // 120s is still comfortably longer than kill_process_tree's
            // own ~2s grace-plus-confirm window, so the fixture's
            // discriminating power (see SpawnerForkStorm's docs) is
            // unaffected.
            "trap '' TERM HUP; i=0; while [ $i -lt 2400 ]; do sh -c 'sleep 120' & sleep 0.05; \
             i=$((i + 1)); done",
            "spawner-fork-storm",
        ),
        Script::ClaudeRecord => record_agent(RecordShape::Claude, record_home),
        Script::CodexRecord => record_agent(RecordShape::Codex, record_home),
        Script::EnvEcho => env_echo(),
    }
}

/// Drive the public spawn CLI from inside a supervised session.
///
/// Keeping this fixture on the real executable is the point: the browser
/// acceptance leg crosses launch-time credential injection, the private
/// supervisor admission path, profile derivation, and the public session
/// list before it observes success. The line protocol avoids a test-only
/// environment knob and lets one long-lived terminal choose an isolated
/// temporary directory at runtime.
fn spawn_session() -> anyhow::Result<()> {
    let parent = std::env::var("FARHELM_SESSION_ID")
        .context("the spawn fake agent must run inside a supervised session")?;
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    writeln!(out, "FAKE-AGENT READY\r")?;
    out.flush()?;

    for line in stdin.lock().lines() {
        let line = line.context("reading a spawn fixture command")?;
        let (cwd, parented, marker) = if let Some(cwd) = line.strip_prefix("spawn-parented ") {
            (cwd, true, "SPAWNED-PARENTED")
        } else if let Some(cwd) = line.strip_prefix("spawn ") {
            (cwd, false, "SPAWNED")
        } else {
            writeln!(out, "SPAWN-ERROR: expected spawn <cwd>\r")?;
            out.flush()?;
            continue;
        };
        // Resolve by name on purpose. The fixture proves the launch shim
        // prepended its own binary directory after shell initialization;
        // using `current_exe` here would bypass the contract under test.
        let mut command = std::process::Command::new("farhelm");
        command.arg("spawn").arg("--cwd").arg(cwd);
        if parented {
            command.arg("--parent").arg(&parent);
        }
        let result = command
            .output()
            .context("running farhelm spawn from the fake agent")?;
        if result.status.success() {
            let child = String::from_utf8(result.stdout)
                .context("farhelm spawn wrote a non-UTF-8 session id")?;
            writeln!(out, "{marker}:{}\r", child.trim_end())?;
        } else {
            let diagnostic = String::from_utf8_lossy(&result.stderr);
            writeln!(out, "SPAWN-ERROR:{}\r", diagnostic.trim_end())?;
        }
        out.flush()?;
    }
    Ok(())
}

/// Report [`RC_MARKER_VAR`] as the launch's shell resolved it, then run
/// [`basic`]'s loop.
///
/// The value is printed BEFORE the ready marker so a test that waits for
/// readiness has the line in hand by then, and an absent variable prints
/// as an explicit empty value rather than nothing at all — "the rc file
/// was not sourced" and "the fixture never got that far" must not look
/// alike to a test.
fn env_echo() -> anyhow::Result<()> {
    let value = std::env::var(RC_MARKER_VAR).unwrap_or_default();
    {
        let mut out = std::io::stdout().lock();
        writeln!(out, "ENV:{RC_MARKER_VAR}={value}\r")?;
        out.flush()?;
    }
    basic()
}

/// Which agent's on-disk record layout [`record_agent`] imitates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordShape {
    Claude,
    Codex,
}

/// Prompt-and-echo like `basic`, but writing a conversation record the way
/// the real agents do — the deterministic fixture PLAN_M3.md item 8's
/// capture tests are built on.
///
/// Every property here exists to make one audited constraint reproducible
/// in CI rather than only reasoned about:
///
/// - **The record appears on FIRST INPUT, not at launch.** This is the
///   constraint that forces correlation onto first-input time and forbids
///   any timeout measured from creation, so the fixture must not write
///   anything until a line arrives. A test can therefore create a session,
///   wait as long as it likes, and only then provoke the record.
/// - **The correlators are per-line JSON**, including the working
///   directory as a FIELD — which is what lets the munged-cwd-collision
///   test put two records for two different directories in one project
///   directory and still expect them told apart.
/// - **`append` appends under the SAME id**, standing in for a plain
///   resume, so re-verification has something to confirm.
/// - **`fork` writes a NEW id** in the same place, standing in for an
///   explicit fork, so the "a fork must not displace the captured
///   identity" test has a real second record to be ignored.
///
/// Markers (`RECORD-WRITTEN:`, `RECORD-APPENDED:`, `RECORD-FORKED:`) are
/// printed so tests key on the record genuinely existing rather than on a
/// sleep — the same discipline `FAKE-AGENT READY` established.
fn record_agent(shape: RecordShape, home: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    let home = home.context(
        "the record-writing fake-agent scripts need --record-home; without it there is no \
         tree to write into and a test would silently observe no capture",
    )?;
    let cwd = std::env::current_dir().context("reading the fixture's working directory")?;
    let cwd = cwd.to_string_lossy().into_owned();

    let mut out = std::io::stdout().lock();
    write!(out, "\x1b[?2004h")?;
    writeln!(
        out,
        "\x1b[1;32mfake-agent\x1b[0m starting (script=record)\r"
    )?;
    // The launch's own argv, echoed so a restart test can assert what was
    // actually RUN rather than inferring it from a side effect. Joined with
    // single spaces: no test needs to recover the original word boundaries,
    // and every value these tests look for (a conversation id) contains no
    // whitespace.
    writeln!(
        out,
        "FAKE-AGENT ARGV:{}\r",
        std::env::args().collect::<Vec<_>>().join(" ")
    )?;

    // The record this fixture owns. Created lazily on the first line — its
    // absence before then IS the behavior under test — unless this launch
    // is RESUMING one, in which case the record already exists and is
    // adopted here, exactly as a real agent's `--resume` picks up an
    // existing conversation rather than starting a new one.
    let mut current: Option<(String, std::path::PathBuf)> = None;
    if let Ok(resumed) = std::env::var(RESUME_ENV_VAR)
        && !resumed.is_empty()
    {
        // `record_path` stamps Codex paths with TODAY's date, but a resumed
        // conversation's record was created when the conversation started —
        // resume a session across UTC midnight and the recomputed path names
        // a directory the record was never in. A real `codex resume` finds
        // the rollout file wherever it sits, so the fixture searches the
        // sessions tree by id before concluding the record is missing.
        let path = Some(record_path(shape, &home, &cwd, &resumed))
            .filter(|p| p.exists())
            .or_else(|| find_codex_record(shape, &home, &resumed));
        if let Some(path) = path {
            writeln!(out, "RECORD-RESUMED:{resumed}\r")?;
            current = Some((resumed, path));
        } else {
            // Never silently start a fresh conversation instead: a test
            // asserting a resume must fail loudly if the id it substituted
            // named nothing, not quietly pass against a new record.
            writeln!(out, "RECORD-RESUME-MISSING:{resumed}\r")?;
        }
    }
    writeln!(out, "FAKE-AGENT READY\r")?;
    write!(out, "> ")?;
    out.flush()?;

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.replace("\x1b[200~", "").replace("\x1b[201~", "");
        let trimmed = line.trim();
        match (trimmed, &current) {
            ("quit", _) => {
                writeln!(out, "bye\r")?;
                out.flush()?;
                return Ok(());
            }
            // A resume: the real agents append to the existing record
            // under the same id rather than starting a new one.
            ("append", Some((id, path))) => {
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .open(path)
                    .context("reopening the record to append")?;
                writeln!(file, "{}", record_line(shape, id, &cwd))?;
                file.flush()?;
                writeln!(out, "RECORD-APPENDED:{id}\r")?;
            }
            // An explicit fork: a genuinely new conversation id, in a new
            // file, exactly as `--fork-session` produces.
            // The forked record's path is deliberately dropped rather
            // than adopted as `current`: a fork is a different
            // conversation, so the fixture keeps writing to the ORIGINAL
            // and a later `append` still exercises the captured record.
            ("fork", Some(_)) => {
                let (id, _forked_path) = write_record(shape, &home, &cwd)?;
                writeln!(out, "RECORD-FORKED:{id}\r")?;
            }
            _ => {
                if current.is_none() {
                    let (id, path) = write_record(shape, &home, &cwd)?;
                    writeln!(out, "RECORD-WRITTEN:{id}\r")?;
                    current = Some((id, path));
                }
                writeln!(out, "echo:\x1b[36m{trimmed}\x1b[0m\r")?;
            }
        }
        write!(out, "> ")?;
        out.flush()?;
    }
    Ok(())
}

/// Create one new record file with a fresh conversation id, returning the
/// id and where it landed.
fn write_record(
    shape: RecordShape,
    home: &std::path::Path,
    cwd: &str,
) -> anyhow::Result<(String, std::path::PathBuf)> {
    let id = fresh_conversation_id();
    let path = record_path(shape, home, cwd, &id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("creating the fixture's record directory")?;
    }
    std::fs::write(&path, format!("{}\n", record_line(shape, &id, cwd)))
        .context("writing the fixture's conversation record")?;
    Ok((id, path))
}

/// Where each agent puts a record for `cwd`.
///
/// Claude's project directory is the MUNGED working directory — reused
/// from the supervisor's own implementation rather than reimplemented,
/// because a fixture that munged differently would make the collision test
/// pass for the wrong reason (or fail for one).
fn record_path(
    shape: RecordShape,
    home: &std::path::Path,
    cwd: &str,
    id: &str,
) -> std::path::PathBuf {
    match shape {
        RecordShape::Claude => home
            .join(".claude")
            .join("projects")
            .join(farhelm_supervisor::agent_kind::munge_cwd(cwd))
            .join(format!("{id}.jsonl")),
        RecordShape::Codex => {
            // `YYYY-MM-DDT...` — the date components the real rollout tree
            // nests by, taken from the same formatter the supervisor parses
            // so the two can never disagree about the calendar.
            let stamp = farhelm_supervisor::agent_kind::format_rfc3339(
                farhelm_supervisor::agent_kind::now_unix(),
            );
            home.join(".codex")
                .join("sessions")
                .join(&stamp[0..4])
                .join(&stamp[5..7])
                .join(&stamp[8..10])
                .join(format!("rollout-{id}.jsonl"))
        }
    }
}

/// Locate an existing Codex rollout record by conversation id anywhere in
/// the date-nested sessions tree. Only the resume path needs this: a fresh
/// record is written under today's date by construction, but a RESUMED
/// conversation's record lives under the date it was created, which
/// `record_path`'s now-stamped reconstruction gets wrong across a UTC
/// midnight. Claude records are not date-nested, so their recomputed path
/// is always right and this returns `None` for that shape.
fn find_codex_record(
    shape: RecordShape,
    home: &std::path::Path,
    id: &str,
) -> Option<std::path::PathBuf> {
    if shape != RecordShape::Codex {
        return None;
    }
    fn walk(dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
        for entry in std::fs::read_dir(dir).ok()? {
            let path = entry.ok()?.path();
            if path.is_dir() {
                if let Some(found) = walk(&path, name) {
                    return Some(found);
                }
            } else if path.file_name().is_some_and(|f| f == name) {
                return Some(path);
            }
        }
        None
    }
    walk(
        &home.join(".codex").join("sessions"),
        &format!("rollout-{id}.jsonl"),
    )
}

/// One record line in the shape the corresponding real agent writes.
///
/// Serialized by `serde_json` rather than assembled by `format!`. The
/// fixture's whole job is to be the thing the supervisor's parser is
/// trusted against, and a working directory is arbitrary bytes chosen by
/// the test: a path containing a quote, a backslash, a newline, or a tab
/// is exactly the input a hand-rolled escaper gets wrong, and the failure
/// would surface as a parser bug rather than a fixture bug. Paying one
/// dependency to make that class of confusion impossible is the right
/// trade.
fn record_line(shape: RecordShape, id: &str, cwd: &str) -> String {
    let timestamp =
        farhelm_supervisor::agent_kind::format_rfc3339(farhelm_supervisor::agent_kind::now_unix());
    let value = match shape {
        RecordShape::Claude => serde_json::json!({
            "type": "user",
            "sessionId": id,
            "cwd": cwd,
            "timestamp": timestamp,
        }),
        RecordShape::Codex => serde_json::json!({
            "timestamp": timestamp,
            "type": "session_meta",
            "payload": {
                "id": id,
                "cwd": cwd,
                "timestamp": timestamp,
            },
        }),
    };
    // `to_string` is the compact form, which is also the only form that is
    // legal here: JSONL requires the whole record on one line, and
    // `serde_json`'s default writer never emits a bare newline inside a
    // string.
    value.to_string()
}

/// A conversation id unique across every fixture process on this host.
///
/// pid plus nanoseconds rather than a UUID: this binary has no `uuid`
/// dependency, the value is opaque to everything that reads it, and the
/// pair is already unique enough that two concurrently-running harnesses
/// cannot collide — which is the only property the tests need, since a
/// collision would silently merge two conversations.
fn fresh_conversation_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("fake-{}-{nanos}", std::process::id())
}

/// Prompt-and-echo with color, bracketed paste, and test control commands.
///
/// Ordinary lines come back as `echo:<line>`; `spam`, `size`, and `quit`
/// exercise scrollback, real PTY geometry, and clean exit respectively.
/// Bracketed paste stays enabled so reattach tests can assert the mode
/// survives replay (the audited silent-loss case in SPEC_impl.md).
fn basic() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    // Bracketed paste on. Set before the ready marker so a test that
    // reattaches the moment it sees the marker always finds the mode
    // already live in tmux's pane state.
    write!(out, "\x1b[?2004h")?;
    writeln!(out, "\x1b[1;32mfake-agent\x1b[0m starting (script=basic)\r")?;
    writeln!(out, "FAKE-AGENT READY\r")?;
    write!(out, "> ")?;
    out.flush()?;

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        // Strip bracketed paste markers so pasted and typed input assert
        // identically in tests.
        let line = line.replace("\x1b[200~", "").replace("\x1b[201~", "");
        let trimmed = line.trim();
        if trimmed == "quit" {
            writeln!(out, "bye\r")?;
            out.flush()?;
            return Ok(());
        }
        // Report the real PTY geometry on demand. The browser resize test
        // uses this rather than trusting xterm's local dimensions, which
        // can change even when the WebSocket-to-tmux resize path is
        // disconnected.
        if trimmed == "size" {
            let size = std::process::Command::new("stty")
                .arg("size")
                .stdin(std::process::Stdio::inherit())
                .output()?;
            if !size.status.success() {
                anyhow::bail!(
                    "stty size failed: {}",
                    String::from_utf8_lossy(&size.stderr).trim()
                );
            }
            writeln!(
                out,
                "size:{}\r",
                String::from_utf8_lossy(&size.stdout).trim()
            )?;
            write!(out, "> ")?;
            out.flush()?;
            continue;
        }
        // `spam N` emits N numbered lines, so replay tests can push
        // content off the visible screen and prove that scrollback —
        // not just the current frame — comes back on reattach.
        if let Some(count) = trimmed.strip_prefix("spam ")
            && let Ok(n) = count.trim().parse::<usize>()
        {
            for i in 1..=n {
                writeln!(out, "spam-line-{i}\r")?;
            }
            write!(out, "> ")?;
            out.flush()?;
            continue;
        }
        writeln!(out, "echo:\x1b[36m{trimmed}\x1b[0m\r")?;
        write!(out, "> ")?;
        out.flush()?;
    }
    Ok(())
}

/// Restore the primary screen and exit immediately, from inside a signal
/// handler.
///
/// This is what makes `altscreen` a faithful stand-in for a real
/// full-screen agent (claude, chiefly) rather than just an app that
/// happens to draw on the alternate screen: claude's own SIGTERM handling
/// leaves the alternate buffer before exiting, which is EXACTLY the
/// behavior the alt-screen-stop-snapshot feature (SPEC_impl.md,
/// e2e.rs's `stop_replays_the_alt_screen_snapshot` and friends) exists to
/// work around — tmux never records alternate-screen content in history,
/// so a graceful-looking exit silently destroys the app's last frame
/// unless something captured it first. Without this handler, `altscreen`
/// under the default SIGTERM disposition dies mid-alt-screen instead,
/// which exercises a DIFFERENT (also real, but not this one) case.
///
/// Only async-signal-safe operations happen here: a raw `write(2)`
/// straight to the pane's stdout fd (bypassing Rust's buffered,
/// lock-based `Stdout` entirely — safe to call from a signal handler,
/// unlike anything that allocates or takes a lock) followed by `_exit`,
/// which unlike `exit` never runs atexit handlers or flushes buffered
/// I/O that might not be in a signal-safe state.
extern "C" fn restore_primary_screen_and_exit(_signal: libc::c_int) {
    let restore = b"\x1b[?1049l";
    // SAFETY: `write` and `_exit` are both on the POSIX async-signal-safe
    // list; neither allocates, takes a lock, or touches libstd's own
    // buffered stdout state, all of which are unsafe to touch from a
    // signal handler.
    unsafe {
        libc::write(
            libc::STDOUT_FILENO,
            restore.as_ptr() as *const libc::c_void,
            restore.len(),
        );
        libc::_exit(0);
    }
}

/// Enter the alternate screen, draw a full-screen frame, wait for a line,
/// then leave. A SIGTERM instead of a line makes it leave the SAME way
/// (see `restore_primary_screen_and_exit`), exercising the graceful-
/// alt-screen-exit case an ordinary `quit`-then-line does not need signal
/// handling to reach. Lets tests assert alt-screen passthrough end to
/// end, alt-screen replay on reattach, and (via the SIGTERM path) the
/// stop-time snapshot feature's own motivating scenario.
///
/// Draws a SECOND row (`STATUS BAR`) whose background color is painted
/// via `\x1b[K` (erase-to-end-of-line under the current SGR) rather than
/// printed as literal space characters — trailing cells styled this way,
/// with no visible glyph in them, are exactly what `capture-pane`'s `-N`
/// flag exists to preserve (see `TmuxDriver::capture_alt_screen_if_active`'s
/// docs); without `-N` tmux trims them from the capture, which is what
/// e2e.rs's `-N`-coverage test pins.
fn altscreen() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    write!(out, "\x1b[?1049h\x1b[2J\x1b[H")?;
    writeln!(out, "\x1b[7m ALT-SCREEN APP \x1b[0m\r")?;
    // Blue background, print a short label, then erase the rest of the
    // row under that same background (no trailing spaces are ever
    // printed) before resetting SGR for whatever comes next.
    write!(out, "\x1b[44m STATUS BAR \x1b[K\x1b[0m\r\n")?;

    // The SIGTERM handler must be live BEFORE the ready marker is even
    // written, let alone flushed: a test attaches and may send SIGTERM
    // (via `stop`) the instant it observes READY, and a stop landing
    // between the marker and the handler installation would hit the
    // DEFAULT SIGTERM disposition instead — silently defeating the whole
    // point of this fixture (see `restore_primary_screen_and_exit`'s
    // docs for what that default-disposition case looks like, and why it
    // is a genuinely different scenario from the one this fixture exists
    // to reproduce).
    // SAFETY: installs a handler via the POSIX `signal(2)` API exposed by
    // `libc`; the handler itself is `restore_primary_screen_and_exit`,
    // whose own docs cover why it only performs async-signal-safe work.
    unsafe {
        libc::signal(
            libc::SIGTERM,
            restore_primary_screen_and_exit as *const () as libc::sighandler_t,
        );
    }

    writeln!(out, "FAKE-AGENT READY\r")?;
    out.flush()?;

    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;

    write!(out, "\x1b[?1049l")?;
    writeln!(out, "left alt screen\r")?;
    out.flush()?;
    Ok(())
}

/// See `Script::AltscreenStubbornChild`'s docs for the gap this fixture
/// exists to widen. `kill_process_tree` (service.rs) walks the PPID
/// closure from THIS process's own pid, so the child spawned below stays
/// reachable and still gets swept even after this process itself has
/// long since restored the screen and exited — a real OS descendant, not
/// merely a delayed exit of the same process, is what forces the sweep
/// through its full escalation.
fn altscreen_stubborn_child() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    write!(out, "\x1b[?1049h\x1b[2J\x1b[H")?;
    writeln!(out, "\x1b[7m ALT-SCREEN APP \x1b[0m\r")?;

    // Installed BEFORE the child spawn below, not just before the ready
    // marker: a SIGTERM landing between spawning the child and this call
    // would hit the default disposition instead, exiting without
    // restoring the screen — defeating the fixture's whole point (see
    // `altscreen`'s identical ordering rationale).
    // SAFETY: see `restore_primary_screen_and_exit`'s own docs for why
    // the handler itself only performs async-signal-safe work.
    unsafe {
        libc::signal(
            libc::SIGTERM,
            restore_primary_screen_and_exit as *const () as libc::sighandler_t,
        );
    }

    // A child that survives ordinary SIGTERM, exactly like
    // `SpawnerStubborn`'s own child — reused here so `kill_process_tree`
    // has something left to escalate against once this process itself
    // has already restored the screen and exited.
    std::process::Command::new("sh")
        .arg("-c")
        .arg("trap '' TERM; sleep 3600")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawning stubborn child")?;

    writeln!(out, "FAKE-AGENT READY\r")?;
    out.flush()?;

    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;

    write!(out, "\x1b[?1049l")?;
    writeln!(out, "left alt screen\r")?;
    out.flush()?;
    Ok(())
}

/// See `Script::AltscreenIgnoresTerm`'s docs. `SIG_IGN` rather than a
/// custom handler: there is nothing to run on SIGTERM at all, which is
/// the point — this process only ever dies to SIGKILL, near the very end
/// of `kill_process_tree`'s escalation, and SIGKILL cannot be blocked,
/// ignored, or handled, so no signal-handler code of ours ever runs on
/// the way out.
fn altscreen_ignores_term() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    write!(out, "\x1b[?1049h\x1b[2J\x1b[H")?;
    writeln!(out, "\x1b[7m ALT-SCREEN APP \x1b[0m\r")?;

    // SAFETY: `SIG_IGN` is a valid `sighandler_t` constant per POSIX
    // `signal(2)`; no handler code of ours runs as a result of this call
    // at all, so there is nothing here for async-signal-safety to say
    // anything about.
    unsafe {
        libc::signal(libc::SIGTERM, libc::SIG_IGN);
    }

    writeln!(out, "FAKE-AGENT READY\r")?;
    out.flush()?;

    // Never reached in the fixture's OWN acceptance tests (this process
    // is always killed via SIGKILL before anything sends it a line), but
    // kept for symmetry with `altscreen`/`altscreen_stubborn_child`: a
    // manual smoke test attaching and sending a line still gets a clean
    // exit rather than hanging forever.
    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;

    write!(out, "\x1b[?1049l")?;
    writeln!(out, "left alt screen\r")?;
    out.flush()?;
    Ok(())
}

/// See `Script::AltscreenStubbornChildStaysAlt`'s docs. Deliberately
/// installs NO SIGTERM handler at all (unlike `altscreen_stubborn_child`,
/// whose whole point is installing one) — the default disposition
/// terminates this process within milliseconds of SIGTERM, still on the
/// alternate screen, with nothing having run to restore it. The spawned
/// child ignores SIGTERM exactly like `altscreen_stubborn_child`'s,
/// forcing `kill_process_tree` through its full escalation regardless of
/// how quickly this process itself is gone.
fn altscreen_stubborn_child_stays_alt() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    write!(out, "\x1b[?1049h\x1b[2J\x1b[H")?;
    writeln!(out, "\x1b[7m ALT-SCREEN APP \x1b[0m\r")?;

    // A child that survives ordinary SIGTERM, exactly like
    // `SpawnerStubborn`'s and `altscreen_stubborn_child`'s own children —
    // reused here so `kill_process_tree` has something left to escalate
    // against once this process itself is already gone (to the default
    // SIGTERM disposition, not any handler of ours).
    std::process::Command::new("sh")
        .arg("-c")
        .arg("trap '' TERM; sleep 3600")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawning stubborn child")?;

    writeln!(out, "FAKE-AGENT READY\r")?;
    out.flush()?;

    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;

    write!(out, "\x1b[?1049l")?;
    writeln!(out, "left alt screen\r")?;
    out.flush()?;
    Ok(())
}

/// Emit a byte that is invalid UTF-8, then remain alive for streaming.
///
/// This script exists because terminal output is bytes, not text. A
/// lossy conversion in the live control path would replace 0xff and
/// still leave every ordinary fake-agent test green. Capture replay is
/// deliberately outside this contract: tmux may canonicalize invalid
/// source bytes when it stores the terminal grid.
fn binary() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    out.write_all(b"\xffBINARY-MARKER\r\nFAKE-AGENT READY\r\n")?;
    out.flush()?;

    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    Ok(())
}

/// Emit an unbroken numbered stream until the surrounding tmux session
/// kills the process.
///
/// Each record is flushed separately and fits in one PTY write. Replay
/// cutover tests can therefore distinguish a real gap or overlap from
/// buffering in this fixture: the expected transcript is a consecutive
/// integer range with every value appearing once.
fn counter() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    writeln!(out, "FAKE-AGENT READY\r")?;
    out.flush()?;
    for sequence in 0_u64.. {
        writeln!(out, "CUTOVER-{sequence:08}\r")?;
        out.flush()?;
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    unreachable!("the counter fixture runs until its session is killed")
}

/// Number of `FLOOD-` records [`flood`] emits. See its docs for the
/// sizing argument; at 16 bytes per record this is ~12 MiB.
pub const FLOOD_RECORDS: u64 = 800_000;

/// Emit [`FLOOD_RECORDS`] numbered records at full speed, then say so and
/// wait to be killed.
///
/// The acceptance fixture for PLAN_M2_5.md's backpressure work, and every
/// property here is chosen against a specific way the tests could lie:
///
/// - **Volume.** ~12 MiB exceeds every Farhelm-side bound on the path
///   combined (the supervisor's per-connection writer and the helm's
///   per-terminal queue), so a test that pauses mid-burst is genuinely
///   provoking flow control rather than watching a queue quietly swallow
///   the whole run. It says nothing about tmux's own limit: `pause-after`
///   bounds by the AGE of queued output, not by bytes, so no volume makes
///   tmux's behavior deterministic — which is why the tests that care
///   force the pause outright.
/// - **Speed.** Unpaced writes — unlike [`counter`], which sleeps between
///   records so replay-cutover tests can reason about individual PTY
///   writes. Buffered through one `BufWriter` for throughput, since this
///   fixture wants bytes on the wire fast rather than per-record write
///   boundaries. A producer slower than the consumer could never fill
///   anything.
/// - **Consecutive, fixed-width record numbers.** This is what makes both
///   loss and duplication detectable at all: correct delivery across a
///   pause is exactly `n, n+1, n+2, ...`, so a gap is loss and a repeat
///   or a step backwards is duplicated replay. A marker without a number
///   could show neither.
/// - **A terminal `FLOOD-DONE` marker, then an idle wait.** Tests need a
///   definite end to assert a tail against, and the process must not exit
///   — a dead pane would replace the pane's content with tmux's "Pane is
///   dead" placeholder and destroy the very scrollback under test.
fn flood() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    writeln!(out, "FAKE-AGENT READY\r")?;
    out.flush()?;
    emit_flood_records(&mut out)
}

/// Identical to [`flood`], except it blocks after `FAKE-AGENT READY` until
/// exactly one input byte has arrived, and only then emits a single record.
///
/// Why a gate at all: `flood`'s whole point (see its own docs) is emitting
/// fast enough to outrun every consumer on the path, which on a fast host
/// can mean the ENTIRE burst is already sitting in tmux's pane history
/// before a test even finishes attaching — the browser-driven watermark
/// tests (PLAN_M2_5.md, e2e/tests/terminal-flood.spec.ts) need the opposite: a
/// LIVE producer racing a client that is already attached, instrumented,
/// and watching, not a sub-watermark replay of a burst that already
/// finished. Gating on real terminal input — sent only once a test's own
/// WebSocket is open and its instrumentation installed — removes that race
/// entirely: nothing streams until the test says so.
///
/// Why a SEPARATE script rather than a flag on `Flood` itself: `Flood` is
/// also the Rust integration suite's own fixture (`crates/farhelm/tests/e2e.rs`'s
/// `flood_session`), and several of those tests attach and expect output
/// flowing immediately, with no input round trip at all (their own comments
/// say so — e.g. "the flood fixture builds a full history quickly"). Gating
/// `Flood` itself would silently hang every one of them; a distinct script
/// leaves that fixture's existing contract untouched.
fn flood_gated() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    // Raw mode BEFORE the ready marker, same reasoning as `hexecho`'s own
    // docs: terminal mode bits live in the pty's line discipline, not the
    // pane, so a test that sends its gate byte the instant it sees READY
    // must always find raw mode already established. Without this, the
    // pty's default canonical mode would hold the gate byte in its own
    // line buffer until a newline arrived, since one arbitrary byte is not
    // itself a complete line — turning a single-byte gate into a
    // newline-shaped one and defeating the point of sending exactly one
    // byte.
    set_raw_mode()?;
    writeln!(out, "FAKE-AGENT READY\r")?;
    out.flush()?;

    let mut gate = [0u8; 1];
    std::io::stdin().lock().read_exact(&mut gate)?;

    emit_flood_records(&mut out)
}

/// Emit [`FLOOD_RECORDS`] numbered records at full speed, then say so and
/// wait to be killed. Shared by [`flood`] and [`flood_gated`], which differ
/// only in what happens before the first byte goes out.
fn emit_flood_records(out: &mut impl Write) -> anyhow::Result<()> {
    // One buffered writer, flushed once at the end, deliberately: this
    // fixture wants throughput, not per-record write boundaries. Records
    // split across PTY writes are fine — every consumer on this path is
    // byte-oriented, and the tests scan for records in the byte stream
    // rather than assuming any framing.
    let mut buffered = std::io::BufWriter::with_capacity(64 * 1024, &mut *out);
    for sequence in 0..FLOOD_RECORDS {
        writeln!(buffered, "FLOOD-{sequence:08}\r")?;
    }
    buffered.flush()?;
    drop(buffered);
    writeln!(out, "FLOOD-DONE\r")?;
    out.flush()?;

    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    Ok(())
}

/// Echo every input byte back as lowercase hex, one space-separated line
/// per read, so a test can observe input-byte fidelity through terminal
/// OUTPUT.
///
/// Exists because the tmux `paste-buffer` input-mangling bug (control
/// bytes like DEL/ESC/ETX arriving caret-escaped as `^?`/`^[`/`^C`) was
/// invisible to `basic` and every other script here: they read stdin in
/// the pty's default canonical mode, where the kernel line discipline
/// itself intercepts or reinterprets those bytes (erase, escape, SIGINT)
/// before an app ever sees them — so a mangled byte and a correct one
/// produced the same visible effect. `hexecho` puts its stdin into raw
/// mode specifically to remove that filter: whatever byte crosses the
/// wire is exactly the byte printed here, which is what makes the fixture
/// able to tell "arrived as 0x7f" apart from "arrived as the two
/// characters `^` and `?`".
fn hexecho() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();

    // Raw mode BEFORE the ready marker — unlike `basic`'s bracketed-paste
    // enable, which is a pane escape sequence tmux records the instant it
    // is written. Terminal mode bits live in the pty's line discipline,
    // not the pane, so a test that sends input the instant it sees READY
    // must always find raw mode already established; printing READY
    // first would let a fast test race a still-canonical pty and land its
    // control bytes in cooked mode, defeating the whole point of this
    // fixture.
    set_raw_mode()?;

    writeln!(out, "FAKE-AGENT READY\r")?;
    out.flush()?;

    hex_echo_loop(&mut out, |_out, _chunk| Ok(()))
}

/// Read stdin until EOF, hex-echoing every chunk exactly as [`hexecho`]
/// always has, but first handing the raw chunk to `on_chunk` — the shared
/// body behind both `hexecho` and [`mouse_modes`], which reacts to input
/// (turning on a DECSET mode) while STILL owing the caller the same
/// byte-visible echo. Splitting the reaction out as a callback rather than
/// duplicating the read loop is what keeps there being exactly one place
/// that decides how input bytes become hex text; a second hand-rolled copy
/// is exactly the drift this refactor exists to prevent.
///
/// Caller order matters and is deliberately NOT this function's job: raw
/// mode and the READY marker must both be established before the first
/// byte can arrive (see `hexecho`'s own docs for why), and different
/// scripts may need script-specific setup in between — `mouse_modes` has
/// none today, but a future caller might.
fn hex_echo_loop<W: Write>(
    out: &mut W,
    mut on_chunk: impl FnMut(&mut W, &[u8]) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    use std::fmt::Write as _;
    let mut stdin = std::io::stdin().lock();
    let mut buf = [0u8; 4096];
    loop {
        let n = stdin.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        let chunk = &buf[..n];
        on_chunk(out, chunk)?;
        let mut line = String::with_capacity(n * 3);
        for (i, byte) in chunk.iter().enumerate() {
            if i > 0 {
                line.push(' ');
            }
            write!(line, "{byte:02x}").expect("String write is infallible");
        }
        writeln!(out, "{line}\r")?;
        out.flush()?;
    }
}

/// Map a recognized cue word directly to the escape sequence it triggers,
/// or `None` for anything else [`CueScanner`] might produce (a typo, or an
/// unrelated line) — a test that types a typo must not silently enable a
/// mode. One function rather than an enum plus two matches over it:
/// nothing else in this file needs the cue as a distinct TYPE, only this
/// word-to-escape mapping (the marker `mouse_modes` prints uses the WORD
/// itself, not anything derived from a cue type).
///
/// `legacy` DECRSTs 1006 before asserting 1000, rather than only
/// asserting 1000 — necessary, not decorative: DECSET only ever turns
/// bits ON, so cueing `sgr` and then `legacy` must still leave the pane
/// in PLAIN legacy tracking. Without the explicit DECRST, `sgr`'s earlier
/// DECSET 1006 would still be latched in tmux's pane state, and every
/// report after the "corrective" `legacy` cue would keep arriving in SGR
/// shape regardless — silently defeating the very cue meant to select
/// the other encoding.
fn mouse_escape(word: &str) -> Option<&'static str> {
    match word {
        "legacy" => Some("\x1b[?1006l\x1b[?1000h"),
        "sgr" => Some("\x1b[?1000h\x1b[?1006h"),
        _ => None,
    }
}

/// The longest word [`mouse_escape`] recognizes (`legacy`), in bytes.
/// [`CueScanner`] uses this to bound its own accumulator — see
/// `CueScanState::Invalid`'s docs — so a future cue word longer than this
/// one must bump it, or the new word could never complete.
const MAX_CUE_WORD_LEN: usize = 6;

/// Recognizes the plain-word cues (`legacy`, `sgr`) `mouse_modes` reads
/// while correctly ignoring every byte that is part of a MOUSE REPORT
/// instead of a typed command — the reason this needs a state machine
/// rather than a bare "accumulate until CR" scan. A report's own data
/// bytes are arbitrary (a column or row value plus 32 can land anywhere in
/// the printable ASCII range) and share the wire with real cue words, so
/// treating every printable byte as a candidate cue character would let a
/// click's own bytes corrupt whatever cue is typed next — confirmed while
/// designing this fixture: a naive version left `sgr` never matching once
/// even one prior click's report bytes had been folded into the
/// accumulator.
///
/// Feed bytes one at a time via [`CueScanner::feed`]; it returns the
/// completed word once a bare CR arrives OUTSIDE a report, and `None`
/// otherwise (still accumulating a word, the byte belonged to a report
/// being skipped, or the word overran [`MAX_CUE_WORD_LEN`]).
#[derive(Debug, Default)]
struct CueScanner {
    state: CueScanState,
    word: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
enum CueScanState {
    #[default]
    Normal,
    /// Saw a bare ESC; the next byte decides whether a report is starting.
    Esc,
    /// Saw `ESC [`; the next byte picks which report shape follows.
    EscBracket,
    /// Inside a legacy (X10-style) report body: `ESC [ M` was seen, and
    /// this many more DATA bytes remain — always starts at 3 (button,
    /// column, row) because that format has no terminator of its own and
    /// is fixed-length by construction, unlike SGR's below.
    LegacyBody(u8),
    /// Inside an SGR report body: `ESC [ <` was seen; everything up to and
    /// including the terminating `M` (button press/motion) or `m` (button
    /// release) belongs to the report.
    SgrBody,
    /// The current word already exceeds [`MAX_CUE_WORD_LEN`] and can
    /// therefore never complete a real cue, no matter what follows —
    /// everything is swallowed (not accumulated) until the next
    /// delimiter. Without this state, an unboundedly long run of
    /// lowercase letters typed (or pasted) by mistake would keep growing
    /// `word` forever, and — worse — a cue word hiding inside it as a
    /// SUFFIX (e.g. `xlegacy`) could shift back into range and fire once
    /// the accumulator happened to end in exactly `legacy` or `sgr`. ESC
    /// is still honored here rather than swallowed too, so a mouse report
    /// immediately following an over-long run (no delimiter in between)
    /// is still parsed correctly instead of having its own bytes
    /// misread as more garbage.
    Invalid,
}

impl CueScanner {
    fn feed(&mut self, byte: u8) -> Option<String> {
        match self.state {
            CueScanState::Normal => match byte {
                0x1b => self.state = CueScanState::Esc,
                b'\r' => {
                    let word = std::mem::take(&mut self.word);
                    return (!word.is_empty()).then_some(word);
                }
                // Only lowercase ASCII letters can be part of `legacy` or
                // `sgr`; anything else (a stray control byte, a digit)
                // starts the next candidate word fresh rather than gluing
                // a botched keystroke onto whatever follows it.
                b'a'..=b'z' => {
                    self.word.push(byte as char);
                    if self.word.len() > MAX_CUE_WORD_LEN {
                        self.word.clear();
                        self.state = CueScanState::Invalid;
                    }
                }
                _ => self.word.clear(),
            },
            CueScanState::Esc => {
                self.state = if byte == b'[' {
                    CueScanState::EscBracket
                } else {
                    // Not a CSI sequence this scanner understands; drop
                    // back to Normal rather than latching into a report
                    // state this fixture would then never leave.
                    CueScanState::Normal
                };
            }
            CueScanState::EscBracket => {
                self.state = match byte {
                    b'M' => CueScanState::LegacyBody(3),
                    b'<' => CueScanState::SgrBody,
                    _ => CueScanState::Normal,
                };
            }
            CueScanState::LegacyBody(remaining) => {
                self.state = if remaining <= 1 {
                    CueScanState::Normal
                } else {
                    CueScanState::LegacyBody(remaining - 1)
                };
            }
            CueScanState::SgrBody => {
                if byte == b'M' || byte == b'm' {
                    self.state = CueScanState::Normal;
                }
            }
            CueScanState::Invalid => match byte {
                0x1b => self.state = CueScanState::Esc,
                b'\r' => self.state = CueScanState::Normal,
                _ => {}
            },
        }
        None
    }
}

/// Enable mouse reporting on cue, hex-echoing every input byte the whole
/// time (via [`hex_echo_loop`], shared with [`hexecho`] rather than
/// reimplemented). The reattach-restoration fixture for the one
/// `PaneModes` branch with no end-to-end coverage before this
/// (PLAN_M6_5.md item 2): mouse modes set by an agent must survive a
/// client detaching and reattaching, exactly like bracketed paste already
/// does.
///
/// Two SEPARATE cues, `legacy\r` and `sgr\r`, rather than one "enable
/// mouse" command — because they exercise DIFFERENT client code paths and
/// a test needs to provoke each independently:
///
/// - `legacy` turns on DECSET 1000 alone: reports use the X10-derived
///   encoding, which vendored xterm.js routes through `onBinary`
///   UNCONDITIONALLY whenever that encoding is the active one — not
///   because of any particular byte value, but because `onBinary` is
///   simply where this encoding's reports are always delivered. The
///   format nonetheless CAN carry bytes above 0x7f (`column`/`row` are
///   each offset by 32, and a column or row past 95 already crosses that
///   line), and it is only by clicking at such a coordinate that a test
///   actually forces a high byte through the wire — proving the
///   `onBinary` path byte-for-byte rather than merely exercising it.
///   `term-bytes.js`'s extracted byte-domain conversion (PLAN_M6_5.md
///   item 1) is exactly what that high byte's fidelity depends on:
///   without a high-coordinate click, that extraction's unit test could
///   pass while the browser-side global sat entirely unwired, and this
///   fixture would never notice.
/// - `sgr` layers DECSET 1006 on top: reports switch to SGR's pure-ASCII
///   encoding, delivered through `onData` instead — the ordinary text
///   path every other fake-agent script's input already exercises.
///
/// Every byte this script receives — a cue word, a real mouse report,
/// anything else — is echoed back as hex regardless of whether it meant
/// anything to [`CueScanner`], so a test can assert a report arrived
/// without this script knowing or caring what a report looks like; only
/// the cue-recognition path needs to know that. Recognizing a cue ALSO
/// prints a plain `MOUSE-MODE:<word>` marker, so a test can wait for the
/// mode to actually be live before clicking, instead of racing the hex
/// echo of the cue's own bytes (which, split across reads one keystroke
/// at a time, has no single line a test could reliably wait for).
fn mouse_modes() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();

    // Same ordering rationale as `hexecho`: raw mode must be live before
    // the ready marker is even flushed, or a fast test's first cue could
    // land in a still-canonical pty.
    set_raw_mode()?;

    writeln!(out, "FAKE-AGENT READY\r")?;
    out.flush()?;

    let mut scanner = CueScanner::default();
    hex_echo_loop(&mut out, |out, chunk| {
        for &byte in chunk {
            if let Some(word) = scanner.feed(byte)
                && let Some(escape) = mouse_escape(&word)
            {
                write!(out, "{escape}")?;
                // A plain, VISIBLE marker — distinct from the hex echo
                // below, which cannot tell a test "the cue byte arrived"
                // apart from "the cue was RECOGNIZED and acted on" (a
                // typo hex-echoes identically to a real cue). Matches
                // this file's own marker convention (its header docs):
                // a test keys on text like this instead of a sleep timed
                // against how fast the agent happens to react, and it is
                // written AFTER the escape above so a test that waits for
                // it never races the mode actually taking effect.
                writeln!(out, "MOUSE-MODE:{word}\r")?;
                out.flush()?;
            }
        }
        Ok(())
    })
}

/// Spawn a child running `child_shell_cmd` under `sh -c`, print both pids,
/// then echo like `basic`. Shared body for every `Spawner*` script variant
/// in `run`'s match, which differ only in what the child does.
///
/// The acceptance subject for process-tree-kill tests (PLAN_M2.md step
/// 4): `stop`/`delete` must reap the agent's entire tree, and a script
/// that never spawns anything cannot distinguish "killed the agent" from
/// "killed the agent's whole tree". The child runs under `sh -c` rather
/// than a second copy of this binary — no argv-parsing or subcommand
/// plumbing needed, and every POSIX host this project targets has `sh` —
/// and `sh -c '<simple command>'` genuinely forks rather than exec-
/// replacing itself (verified empirically against `/bin/sh` on this
/// project's Linux targets), so the printed child pid and its own
/// eventual descendant (e.g. `sleep`, forked by `sh` to run it) form a
/// real three-level chain for tests that need one. The child deliberately
/// outlives this process without being waited on: nothing here calls
/// `Child::wait`, so it keeps running (invisible to us) until something
/// else — ordinarily the very process-tree kill this fixture exists to
/// test — signals it directly.
fn spawn_and_echo(child_shell_cmd: &str, script_name: &str) -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    write!(out, "\x1b[?2004h")?;
    writeln!(
        out,
        "\x1b[1;32mfake-agent\x1b[0m starting (script={script_name})\r"
    )?;

    let self_pid = std::process::id();
    let child_pid = std::process::Command::new("sh")
        .arg("-c")
        .arg(child_shell_cmd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawning child process")?
        .id();
    // The pids a test needs to poll `/proc` for, printed before the ready
    // marker so a test keying on READY has already seen both lines.
    writeln!(out, "SELF-PID:{self_pid}\r")?;
    writeln!(out, "CHILD-PID:{child_pid}\r")?;
    writeln!(out, "FAKE-AGENT READY\r")?;
    write!(out, "> ")?;
    out.flush()?;

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.replace("\x1b[200~", "").replace("\x1b[201~", "");
        let trimmed = line.trim();
        if trimmed == "quit" {
            writeln!(out, "bye\r")?;
            out.flush()?;
            return Ok(());
        }
        writeln!(out, "echo:\x1b[36m{trimmed}\x1b[0m\r")?;
        write!(out, "> ")?;
        out.flush()?;
    }
    Ok(())
}

/// Spawn a doubly-forked daemon that reparents to init while still
/// inheriting the session's `FARHELM_SESSION_ID` marker, and print its
/// own pid plus a ready marker. The daemon also forks its OWN child with
/// that marker deliberately stripped (`env -u FARHELM_SESSION_ID`) —
/// see below for why.
///
/// The acceptance fixture for the marker-scan half of `kill_process_tree`
/// (lore/2026-07-27-m2-process-tree-stop.md): `(setsid sh -c '...' &)`
/// backgrounds a new-session child and then the launching subshell exits
/// immediately, so by the time anything looks for it, the daemon's
/// parent is already gone and it has been reparented to init — no longer
/// reachable by any PPID walk from this process at all. Its environment
/// still carries the session marker regardless, because environment
/// variables are inherited across fork and exec unless something along
/// the way deliberately scrubs or replaces its own environment (which
/// the daemon itself does not do); finding it is only possible via that
/// marker.
///
/// The daemon's OWN child strips the marker before running, so it is
/// UNMARKED and unreachable by the marker scan directly — but it IS a
/// genuine OS child of the (marked) daemon. This is the acceptance
/// fixture for closure SEEDING (service.rs's `enumerate_tree`): marker
/// pids seed the PPID closure before it expands, so the daemon becomes a
/// root the closure walks from, and this unmarked grandchild is reachable
/// through that closure even though the marker scan alone would never
/// find it. Both pids are written to files in the (inherited) working
/// directory, since by the time either process is running there is no
/// ancestor left that could report them — a test polls for those files
/// rather than trusting any fixed timing. Both processes' lifetimes are
/// bounded (120s, not `sleep 3600`) for the same self-expiry reason as
/// `SpawnerForkStorm` — a test-side drop guard is the primary cleanup,
/// this is the backstop under it.
fn spawner_reparent() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    write!(out, "\x1b[?2004h")?;
    writeln!(
        out,
        "\x1b[1;32mfake-agent\x1b[0m starting (script=spawner-reparent)\r"
    )?;

    let self_pid = std::process::id();
    // Waited on deliberately: this launcher subshell itself exits almost
    // immediately (it only forks the detached daemon and backgrounds it),
    // so waiting for IT to finish is a real synchronization point — proof
    // the daemon has at least been launched — without depending on the
    // daemon itself (which outlives this wait) ever being waited on.
    //
    // The unmarked child's own pid is captured via `$!` in the DAEMON's
    // shell (the job-control "pid of the last backgrounded command"),
    // right after backgrounding it — not via a `$$` inside a further
    // nested `sh -c "..."` string, which would need escaping through
    // three quoting levels at once. `env` execs its target directly (no
    // extra fork), so the backgrounded job's pid is already the real
    // `sh -c 'sleep 120'` process, identical to what that process would
    // report for its own `$$`.
    std::process::Command::new("sh")
        .arg("-c")
        .arg(
            "(setsid sh -c 'echo $$ > reparented.pid; \
             env -u FARHELM_SESSION_ID sh -c \"sleep 120\" & \
             echo $! > unmarked-child.pid; sleep 120' &)",
        )
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawning the reparenting daemon's launcher")?
        .wait()
        .context("waiting for the launcher to finish backgrounding the daemon")?;

    writeln!(out, "SELF-PID:{self_pid}\r")?;
    writeln!(out, "FAKE-AGENT READY\r")?;
    write!(out, "> ")?;
    out.flush()?;

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.replace("\x1b[200~", "").replace("\x1b[201~", "");
        let trimmed = line.trim();
        if trimmed == "quit" {
            writeln!(out, "bye\r")?;
            out.flush()?;
            return Ok(());
        }
        writeln!(out, "echo:\x1b[36m{trimmed}\x1b[0m\r")?;
        write!(out, "> ")?;
        out.flush()?;
    }
    Ok(())
}

/// Spawn a daemon that neither half of `kill_process_tree` can find, and
/// print a ready marker; its pid goes to `cloaked.pid` in the working
/// directory, since nothing left alive is related to it closely enough to
/// report it. The recorded pid is the daemon ITSELF, not a shell wrapping
/// it — see the `exec` below for why that distinction decides whether this
/// fixture proves anything at all.
///
/// This is the acceptance fixture for the cgroup hardening (PLAN_M3.md
/// item 10), and it is deliberately the exact residual
/// lore/2026-07-27-m2-process-tree-stop.md records as the sweep's one
/// accepted blind spot — BOTH cloaking steps are load-bearing and neither
/// alone would do:
///
/// - `(setsid ... &)` double-forks, so the daemon has reparented to init
///   before anything could walk down to it: the PPID closure never reaches
///   it. `spawner-reparent` already covers this half on its own.
/// - `env -u FARHELM_SESSION_ID` strips the marker BEFORE the daemon
///   starts, so the `/proc/*/environ` scan cannot see it either — and,
///   unlike `spawner-reparent`'s unmarked grandchild, this process has no
///   marked ancestor left to be found through, because its parent shell
///   exits immediately.
///
/// The consequence is what makes it useful: on a host with no systemd user
/// manager this daemon SURVIVES a stop (which is why no test asserts
/// otherwise there), and on a host with one it dies — so its death is
/// positive evidence that the scope kill ran and reached what only a
/// cgroup can reach, not merely that something killed something.
///
/// Self-expiring at 120s for the same reason every other spawner fixture
/// is (`SpawnerForkStorm`): a test that fails before calling stop must not
/// leak an immortal process, least of all one designed to be unfindable.
fn spawner_cloaked() -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    write!(out, "\x1b[?2004h")?;
    writeln!(
        out,
        "\x1b[1;32mfake-agent\x1b[0m starting (script=spawner-cloaked)\r"
    )?;

    let self_pid = std::process::id();
    // Waited on for the same reason `spawner_reparent` waits: this
    // launcher subshell exits as soon as it has backgrounded the daemon,
    // so its exit is a real synchronization point proving the daemon was
    // at least started, without depending on the daemon itself.
    // `exec sleep` after the write, not a plain `sleep`, and this is the
    // difference between a fixture that works and one that quietly lies:
    // `$$` is the SHELL's pid, so without the `exec` the recorded pid would
    // name a shell whose `sleep` child is a separate process — a test
    // killing the recorded pid would leave the real survivor running, and a
    // test asserting the recorded pid died would pass while the thing that
    // actually had to die lived on. `exec` collapses the two into one
    // process, so the pid in the file IS the process under test.
    std::process::Command::new("sh")
        .arg("-c")
        .arg(
            "(setsid env -u FARHELM_SESSION_ID sh -c \
             'echo $$ > cloaked.pid; exec sleep 120' &)",
        )
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawning the cloaked daemon's launcher")?
        .wait()
        .context("waiting for the launcher to finish backgrounding the cloaked daemon")?;

    writeln!(out, "SELF-PID:{self_pid}\r")?;
    writeln!(out, "FAKE-AGENT READY\r")?;
    write!(out, "> ")?;
    out.flush()?;

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.replace("\x1b[200~", "").replace("\x1b[201~", "");
        let trimmed = line.trim();
        if trimmed == "quit" {
            writeln!(out, "bye\r")?;
            out.flush()?;
            return Ok(());
        }
        writeln!(out, "echo:\x1b[36m{trimmed}\x1b[0m\r")?;
        write!(out, "> ")?;
        out.flush()?;
    }
    Ok(())
}

/// Put this process's controlling terminal into raw mode: no canonical
/// line editing, no signal-generating control characters, no local echo.
///
/// Nothing upstream configures the pty this way — tmux hands out an
/// ordinary cooked-mode pty, and `basic` deliberately reads via
/// `BufRead::lines` to exercise that default. `cfmakeraw` is not POSIX
/// itself (POSIX standardizes termios but not this convenience function);
/// it is a BSD-originated libc extension present on Linux and every other
/// target this project builds for, and it flips every relevant termios
/// flag at once, rather than hand-listing `ICANON`/`ECHO`/`ISIG`/`IXON`
/// and hoping the list stays complete.
fn set_raw_mode() -> anyhow::Result<()> {
    use std::os::fd::AsRawFd;

    let fd = std::io::stdin().as_raw_fd();
    let mut term: libc::termios = unsafe { std::mem::zeroed() };
    // SAFETY: `fd` is stdin's own fd, valid for the process lifetime, and
    // `term` is a plain-old-data struct large enough for `tcgetattr` to
    // fill in (zero-initialized so any field it doesn't touch is still
    // well-defined).
    if unsafe { libc::tcgetattr(fd, &mut term) } != 0 {
        anyhow::bail!("tcgetattr failed: {}", std::io::Error::last_os_error());
    }
    // SAFETY: `term` was just populated by `tcgetattr` above.
    unsafe { libc::cfmakeraw(&mut term) };
    // SAFETY: same fd and a `term` value `cfmakeraw` just produced from a
    // real `tcgetattr` snapshot.
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &term) } != 0 {
        anyhow::bail!("tcsetattr failed: {}", std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixture that produces invalid JSONL would make every capture test
    /// fail as if the supervisor's parser were broken, so the escaping
    /// contract is pinned here rather than left implicit in whatever
    /// working directories the e2e suite happens to create. Newlines and
    /// tabs are the load-bearing cases: both are legal in a POSIX path,
    /// both are illegal raw inside a JSON string, and a raw newline would
    /// additionally split one record across two JSONL lines.
    #[test]
    fn a_hostile_working_directory_still_produces_one_valid_jsonl_line() {
        let cwd = "/tmp/we\"ird\\path\nwith\tcontrol/chars";
        for shape in [RecordShape::Claude, RecordShape::Codex] {
            let line = record_line(shape, "fake-1-2", cwd);
            assert!(
                !line.contains('\n'),
                "{shape:?} record spans more than one JSONL line: {line}"
            );
            let parsed: serde_json::Value =
                serde_json::from_str(&line).expect("record line must be valid JSON");
            let recorded = match shape {
                RecordShape::Claude => &parsed["cwd"],
                RecordShape::Codex => &parsed["payload"]["cwd"],
            };
            assert_eq!(recorded.as_str(), Some(cwd), "{shape:?} lost the cwd");
        }
    }

    /// Feed every byte of `bytes` through a fresh [`CueScanner`], returning
    /// only the completed words it produced (dropping the `None`s that
    /// mark "still accumulating" or "byte belonged to a report").
    fn scan(bytes: &[u8]) -> Vec<String> {
        let mut scanner = CueScanner::default();
        bytes.iter().filter_map(|&b| scanner.feed(b)).collect()
    }

    /// The ordinary case: a plain typed word terminated by a bare CR (what
    /// a real terminal sends for Enter in raw mode) is recognized whole.
    #[test]
    fn a_plain_cue_word_is_recognized_on_bare_cr() {
        assert_eq!(scan(b"legacy\r"), vec!["legacy"]);
        assert_eq!(scan(b"sgr\r"), vec!["sgr"]);
    }

    /// The scenario this scanner exists for (see its own docs): a legacy
    /// X10-encoded mouse report's three data bytes are printable ASCII by
    /// construction whenever a click lands near the top-left of the
    /// screen, and a naive "accumulate every printable byte until CR" scan
    /// would fold them into whatever cue is typed next — silently
    /// preventing it from ever matching. A real button click reports
    /// press AND release, so this pins both reports in sequence, followed
    /// by a genuine cue, exactly as the browser-driven e2e test drives it.
    ///
    /// The three data bytes in each report are deliberately chosen as
    /// LOWERCASE ASCII LETTERS (`x`/`y`/`z`, `p`/`q`/`r`) rather than the
    /// punctuation a click position would actually produce: letters are
    /// the one byte class [`CueScanner::feed`]'s `Normal` state treats as
    /// a candidate cue character. An off-by-one in `LegacyBody`'s skip
    /// count (leaking the report's last data byte into `Normal` one byte
    /// early) would, with punctuation bytes, still just clear the
    /// accumulator and vanish without a trace — this test would pass
    /// whether or not the bug existed. With a leaked LETTER, that byte
    /// would glue itself onto the very next cue word instead, changing
    /// the final assertion's result and actually catching the bug.
    #[test]
    fn legacy_mouse_report_bytes_never_pollute_the_next_cue_word() {
        // `ESC [ M` + three lowercase-letter "data bytes" standing in for
        // button/column/row — see this test's own docs for why letters,
        // not the punctuation a real click would encode, are what makes
        // this assertion meaningful.
        let press = b"\x1b[Mxyz";
        let release = b"\x1b[Mpqr";
        let mut input = Vec::new();
        input.extend_from_slice(press);
        input.extend_from_slice(release);
        input.extend_from_slice(b"sgr\r");
        assert_eq!(
            scan(&input),
            vec!["sgr"],
            "the report bytes must contribute nothing; only the typed cue word may surface"
        );
    }

    /// The SGR-encoded counterpart: variable-length, digit/semicolon
    /// bodied, and terminated by `M` (press) or `m` (release) rather than
    /// legacy's fixed three bytes — a distinct shape the scanner must skip
    /// just as cleanly. Unlike the legacy test above, SGR's body bytes
    /// (digits, `;`, and the `M`/`m` terminator) are NEVER lowercase
    /// letters by construction, so no choice of realistic body bytes here
    /// could ever leak into a cue word undetected — this test's existing
    /// digit/semicolon bytes are already as sensitive as this shape can be
    /// made.
    #[test]
    fn sgr_mouse_report_bytes_never_pollute_the_next_cue_word() {
        let press = b"\x1b[<0;3;3M";
        let release = b"\x1b[<0;3;3m";
        let mut input = Vec::new();
        input.extend_from_slice(press);
        input.extend_from_slice(release);
        input.extend_from_slice(b"legacy\r");
        assert_eq!(scan(&input), vec!["legacy"]);
    }

    /// An over-length run of lowercase letters must never let a shorter
    /// cue hiding inside it as its own SUFFIX match once the run has
    /// shifted back into range (`CueScanState::Invalid`'s own docs cover
    /// why this bound exists at all), and normal service must resume
    /// cleanly once a real delimiter is seen.
    #[test]
    fn an_over_length_word_never_matches_even_when_a_cue_is_its_suffix() {
        assert_eq!(scan(b"xlegacy\r"), Vec::<String>::new());
        // No delimiter between the overflow and the trailing `sgr` at
        // all: everything after the overflow point is swallowed as one
        // run, exactly like `CueScanState::Invalid`'s docs describe.
        assert_eq!(scan(b"xxxxxxxxsgr\r"), Vec::<String>::new());
        // A delimiter DOES resync the scanner: a fresh, in-range word
        // right after it still matches normally.
        assert_eq!(scan(b"xlegacy\rsgr\r"), vec!["sgr"]);
    }

    /// [`mouse_escape`] is the ONE place a recognized word turns into a
    /// mode change; pinned against the EXACT bytes (not merely "some
    /// escape came back") because 1002/1003 (motion tracking) would
    /// satisfy a browser-level test just as well as 1000 — only an exact
    /// byte comparison catches the wrong DECSET code. `legacy`'s DECRST
    /// 1006 prefix is pinned here too: see `mouse_escape`'s own docs for
    /// why cueing `legacy` must actively turn SGR back off, not merely
    /// re-assert 1000.
    #[test]
    fn mouse_escape_selects_the_exact_decset_sequence() {
        assert_eq!(mouse_escape("legacy"), Some("\x1b[?1006l\x1b[?1000h"));
        assert_eq!(mouse_escape("sgr"), Some("\x1b[?1000h\x1b[?1006h"));
        assert_eq!(mouse_escape("quit"), None);
        assert_eq!(mouse_escape(""), None);
    }

    /// The scenario the review that added `legacy`'s DECRST prefix was
    /// written against: cueing `sgr` and then `legacy`, IN THAT ORDER,
    /// through the real scanner (not `mouse_escape` in isolation) must
    /// still end with SGR encoding turned back off. Before the DECRST fix
    /// this second escape was just `\x1b[?1000h` — a no-op against tmux's
    /// already-latched 1006, so every report after the "corrective" cue
    /// kept arriving in SGR shape regardless of what the user had just
    /// asked for.
    #[test]
    fn legacy_after_sgr_resets_sgr_encoding() {
        let mut scanner = CueScanner::default();
        let mut escapes = Vec::new();
        for &byte in b"sgr\rlegacy\r" {
            if let Some(word) = scanner.feed(byte)
                && let Some(escape) = mouse_escape(&word)
            {
                escapes.push(escape);
            }
        }
        assert_eq!(
            escapes,
            vec!["\x1b[?1000h\x1b[?1006h", "\x1b[?1006l\x1b[?1000h"],
            "the second cue must actively turn 1006 back off, not just re-assert 1000"
        );
    }
}
