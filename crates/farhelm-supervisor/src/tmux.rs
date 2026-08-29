//! The tmux driver: a private, headless tmux server used purely as a PTY
//! holder and history store (SPEC_impl.md, "Terminal substrate").
//!
//! Nothing here ever runs a rendering `tmux attach`. Output is consumed
//! through a non-rendering control-mode client (`tmux -C`) — see
//! [`OutputStream`] — and input goes in as `send-keys -H` commands carried
//! by a SECOND, dedicated no-output control-mode client — see
//! [`InputClient`] for why a second client, rather than the output
//! client's stdin, is what carries input. A THIRD shape exists once a
//! session has any attachment at all: [`SessionSink`], one per tmux
//! session, which speaks for no terminal and exists purely so tmux always
//! has a client it can deliver every pane to. Sizing is pinned by explicit
//! `resize-window` calls, and replay is `capture-pane -e` history plus
//! re-synthesized pane modes. The motivations — native xterm.js scrolling,
//! no tmux UI anywhere — are recorded in SPEC_impl.md; this module is that
//! design made concrete.

mod control_codec;
mod input;
mod sink;
mod snapshot;
mod stream;
#[cfg(test)]
mod test_support;

pub use control_codec::unescape_control_output;
pub use input::InputClient;
pub use sink::SessionSink;
pub use snapshot::AltScreenCapture;
pub(crate) use snapshot::within_snapshot_cap;
pub(crate) use stream::ReplayStreamCandidate;
pub use stream::{OutputEvent, OutputReaper, OutputStream};

use anyhow::{Context, bail};
#[cfg(test)]
use control_codec::tmux_ordinal;
use control_codec::{
    PANE_FACT_FORMAT, PANE_MARKER_FORMAT, any_session_has_several_windows, join_pane_markers,
    parse_pane_facts,
};
use snapshot::parse_alt_screen_capture;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tracing::warn;

/// History (and therefore replay) floor. SPEC.md promises at least the
/// screen plus 10,000 lines; the margin covers lines scrolled during the
/// capture itself.
///
/// Doubles as the ceiling on the browser's own scrollback (PLAN_M2_5.md:
/// xterm.js scrollback capacity at most this floor, so a post-stall
/// catch-up's end state is observably equivalent to lossless slow
/// delivery). That cross-language invariant is pinned by an e2e test that
/// reads `crates/farhelm-ui/assets/terminal.js` directly, because nothing
/// else forces the two numbers to move together.
pub const HISTORY_LIMIT: u32 = 12_000;

/// How far behind a control client's pane stream may fall, in seconds,
/// before tmux applies its own flow control to it (`pause-after` — see
/// `attach_cutover_command`).
///
/// This is the overflow backstop under Farhelm's own flow control, not
/// the primary mechanism: PLAN_M2_5.md's browser-driven pause/resume
/// keeps the steady state far from here, and a real pause lasts seconds
/// at most (the drainable backlog is bounded by the UI's high-water mark
/// plus the bounded queues below it).
///
/// What tmux actually does once this trips is NOT one behavior, which is
/// worth knowing before reading the rest of this module (audited
/// 2026-07-29 on 3.3a, 3.4 and 3.7b, all three observed doing both across
/// repeated identical trials — it is not a version split; see
/// SPEC_impl.md's backpressure paragraph): tmux either throttles the pane
/// itself, blocking the agent's own `write` so nothing is queued and
/// delivery just continues on resume, or reads ahead into history and
/// cuts this client's stream with `%pause`, discarding what it had queued
/// for it. Only the second needs recovery, and
/// [`OutputStream::resume_paused_with_replay`] is it. Nothing here may
/// assume which one will happen.
///
/// Two later measurements sharpen that first branch, and both are the
/// reason [`SessionSink`] exists (audited 2026-08-02 on 3.4 and 3.7b, a
/// 16 MB/s producer against a client that stops reading; the effect is
/// rate-dependent — it does not reproduce at 800 KB/s at all, which is
/// why an audit can honestly report "sometimes"). First, the throttle is
/// not a property of the stalled client's OWN pane: tmux stops reading
/// the pane the client is behind on, whichever pane that is, so a stalled
/// tab viewer takes down the AGENT's pane just as readily. Second, the
/// throttle is NOT bounded by this constant the way the paragraph above
/// suggests — 4 of 5 trials per version stopped the pane for the entire
/// 45-second observation window, well past `pause-after`, ending only
/// when the stalled client went away. That is the residual [`SessionSink`]
/// closes by guaranteeing tmux always has one client that can consume
/// every pane.
///
/// Five seconds is long enough that an ordinary watermark pause rarely
/// trips this — tripping is not an error, but on the second path it costs
/// a full reset-and-replay catch-up — and short enough that a genuinely
/// wedged client cannot make the tmux server grow its own memory for
/// long. Without the flag at all, an undrained control client grows the
/// tmux server's RSS without bound (audited 2026-07-29: ~3.5 MB/s against
/// a `yes` pane on 3.4), which is the failure this exists to close.
///
/// The bound is per (CLIENT, PANE), not per client, and once terminal tabs
/// exist that detail is load-bearing rather than trivia (PLAN_M4.md item
/// 3): a stalled tab viewer falls behind on its own pane and on every
/// other pane of the session its control client hears, and tmux answers
/// per pane — so the panes it is behind on are cut for THAT client while
/// the agent's own client, which is keeping up, keeps receiving. That is
/// what makes one control client per attached terminal sufficient for
/// stall isolation; sharing one across a session's terminals would not be.
///
/// This flag is deliberately NOT set on [`SessionSink`]'s client. The sink
/// is the one client that must never be the flow-control victim, and a
/// paused sink is a sink that has stopped holding the session's panes
/// readable.
pub const TMUX_PAUSE_AFTER_SECS: u64 = 5;

/// How many lines of scrollback [`TmuxDriver::capture_pane_text`] looks
/// back over for a dead pane's last words.
///
/// One screen plus a margin, not the whole history: the caller is
/// building an error message, and a failed shell says what it has to say
/// in a line or two. Reaching further would mostly harvest the blank rows
/// a mostly-empty pane is padded with.
const LAST_WORDS_LINES: u32 = 50;

/// Reduce a `capture-pane` transcript to the last non-blank text it
/// holds, at most `max_bytes` of it.
///
/// Split out from [`TmuxDriver::capture_pane_text`] so the trimming and
/// tail-truncation rules are unit-testable against constructed strings —
/// the same reasoning [`parse_pane_facts`] and [`PaneModes::parse`] were
/// split out for.
///
/// Trailing blank rows go first, because a pane is padded to its full
/// height and those rows are not something anyone printed. What remains
/// is truncated from the FRONT when it is too long — see the caller's
/// docs for why the tail is the part worth keeping — landing on a
/// character boundary so the result is always valid UTF-8.
fn last_words(transcript: &str, max_bytes: usize) -> String {
    let trimmed = transcript.trim_end();
    if trimmed.len() <= max_bytes {
        return trimmed.to_string();
    }
    let start = trimmed.len() - max_bytes;
    let start = (start..trimmed.len())
        .find(|index| trimmed.is_char_boundary(*index))
        .unwrap_or(trimmed.len());
    trimmed[start..].to_string()
}

/// The tmux window option carrying a TAB's minted id (PLAN_M4.md item 2).
///
/// Windows are the substrate for terminal tabs, and tabs are deliberately
/// not durable metadata — SPEC.md says a reboot or an archive erases them
/// and nothing recreates them — so the marker on the window IS the record.
/// It has to be a marker rather than a position: a pane's own processes
/// inherit `TMUX` and can create windows on this private server, so a
/// "windows 1 and up" scan would adopt strangers with the wrong working
/// directory and the wrong teardown semantics.
///
/// The value is a MINTED uuid, never the tmux window id. Window ids come
/// from a server-wide counter that restarts with the server, so a client
/// holding a selector from before a reboot would otherwise be handed
/// whatever window later inherited that number — see `TabInfo::id`'s own
/// contract, which promises `NotFound` instead.
///
/// User options (`@`-prefixed) are the only option namespace tmux
/// guarantees never to interpret, and window-scoped ones survive
/// everything a window survives. They are also world-writable by anything
/// that inherited `TMUX`, which is why every read of this value is
/// shape-checked (`scope::is_uuid_shaped`) before it is trusted.
pub const TAB_WINDOW_OPTION: &str = "@farhelm-tab";

/// The tmux window option marking the AGENT's window, carrying its session
/// id (PLAN_M4.md item 2).
///
/// Window 0 remains the agent's window in practice — nothing about tabs
/// changes the layout — but identity is by marker, not by index. The
/// supervisor finds the agent's PANE through its own durable record and
/// never needs this to locate it; what this buys is the other direction:
/// a window carrying it is provably not a tab, so a tab scan can exclude
/// the agent positively instead of by assuming index 0.
///
/// Set at create (and at the restart that has to build a fresh tmux
/// session), best-effort: a session launched by an older build carries no
/// marker at all, and nothing degrades — tab discovery keys on
/// [`TAB_WINDOW_OPTION`] alone, which such a session's agent window
/// equally lacks.
pub const AGENT_WINDOW_OPTION: &str = "@farhelm-agent";

/// One deadline covers attaching the control client, taking the replay
/// snapshot, and enabling live output. A wedged tmux command must fail
/// the attach request instead of leaving it holding the global
/// attachment lock forever.
///
/// This is the PRODUCTION value and must stay tight: it bounds how long a
/// wedged tmux can hold the supervisor-wide attachments mutex, and every
/// [`TmuxDriver`] defaults to it. The e2e suite runs with a longer budget
/// instead — see the `SUITE_TMUX_EXCHANGE_TIMEOUT` doc in the e2e harness
/// for why (PLAN.md's M6.5 tracks this as a leading hypothesis for a class
/// of loaded-CI one-offs, not a confirmed diagnosis) — so the value
/// actually in effect is [`TmuxDriver`]'s `exchange_timeout` field,
/// injected at construction (see [`TmuxDriver::new_with_timeouts`]) rather
/// than read from here directly.
pub(crate) const CONTROL_EXCHANGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// How long an output-bearing control client gets to exit after stdin closes.
///
/// Before closing stdin, normal teardown switches the client back to
/// `no-output`. tmux applies that flag by discarding every pane block and
/// refusing new ones, which gives the client a race-free exit boundary even
/// if a new pane appears during teardown. Killing a client while those blocks
/// still exist can abort tmux 3.7b itself (`fatal: not enough data`). Two
/// seconds bounds each phase independently: a healthy local client completes
/// both in milliseconds, while a broken one must not hold the
/// supervisor-wide attachment lock forever.
const CONTROL_CLIENT_SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Backoff for a client whose acknowledged output-off boundary is not ready.
const CONTROL_CLEANUP_RETRY_BASE: std::time::Duration = std::time::Duration::from_millis(250);

/// Longest interval between retries of the safe output-off boundary.
const CONTROL_CLEANUP_RETRY_MAX: std::time::Duration = std::time::Duration::from_secs(5);

/// Grow cleanup retry delay exponentially without exceeding the process cap.
fn control_cleanup_retry_delay(failures: u32) -> std::time::Duration {
    let exponent = failures.saturating_sub(1).min(31);
    let factor = 1u32.checked_shl(exponent).unwrap_or(0);
    if factor == 0 {
        return CONTROL_CLEANUP_RETRY_MAX;
    }
    CONTROL_CLEANUP_RETRY_BASE
        .saturating_mul(factor)
        .min(CONTROL_CLEANUP_RETRY_MAX)
}

/// The format is deliberately comma-separated. See [`PaneModes::parse`].
///
/// `#{pane_dead}` rides along on this same query rather than a separate
/// `pane_process` call from the `Attach` handler — `pane_process` is a
/// plain (non-control-mode) tmux invocation of its own, and this format is
/// already fetched as part of the control-mode replay cutover command
/// group in `OutputStream::snapshot_then_cutover`, so folding the dead
/// flag in here avoids that extra round trip entirely rather than merely
/// avoiding a second CONTROL-mode one. The dead flag is what the `Attach`
/// handler (service.rs) uses to decide whether to append the alt-screen
/// stop snapshot.
const PANE_MODE_FORMAT: &str = "#{alternate_on},#{bracket_paste_flag},#{mouse_all_flag},\
                                #{mouse_button_flag},#{mouse_standard_flag},#{mouse_sgr_flag},\
                                #{cursor_flag},#{keypad_cursor_flag},#{cursor_x},#{cursor_y},\
                                #{pane_dead}";

/// How long `OutputStream::foreign_panes` gives tmux to list a
/// session's panes, independently of the attach's own budget.
///
/// The listing is an optimization's input, not the attach itself, so it
/// gets a short deadline of its own rather than a share of
/// [`CONTROL_EXCHANGE_TIMEOUT`]: a tmux slow to answer it must not be able
/// to consume the budget the replay capture and cutover still need. It is
/// still clamped to the attach deadline by the caller, so it can never
/// EXTEND the attach either.
///
/// Like [`CONTROL_EXCHANGE_TIMEOUT`], this is the production value only —
/// [`TmuxDriver::pane_list_timeout`] is what callers actually consult, and
/// it defaults to this constant.
pub(crate) const PANE_LIST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Handle to the private tmux server. All tmux invocations go through
/// this so the `-S <socket> -f <config>` isolation is impossible to
/// forget — the user's own tmux server and config must never be touched.
#[derive(Debug, Clone)]
pub struct TmuxDriver {
    socket: PathBuf,
    config: PathBuf,
    /// The tmux binary EVERY invocation from this driver runs, including
    /// the `-V` floor probe.
    ///
    /// Carried per-driver rather than hardcoded so the `--tmux` /
    /// `FARHELM_TMUX` override (see [`resolve_tmux_program`]) reaches all
    /// of them: an override honored by only some call sites would
    /// floor-check one binary and then drive another, which is worse than
    /// having no override at all. Constructors that do not name one — the
    /// test constructors, and any embedder that has no opinion — default
    /// to [`DEFAULT_TMUX_PROGRAM`].
    program: PathBuf,
    /// The budget for one control-mode exchange (attach, send-keys,
    /// filter command). See [`CONTROL_EXCHANGE_TIMEOUT`] for what this
    /// bounds and why production keeps it tight; carried per-instance
    /// (rather than read from the constant directly) so
    /// [`Self::new_with_timeouts`] can hand a test-only supervisor a
    /// longer budget without touching the value every real supervisor
    /// runs with.
    exchange_timeout: std::time::Duration,
    /// The budget for `OutputStream::foreign_panes`'s pane listing. See
    /// [`PANE_LIST_TIMEOUT`]; injectable for the same reason as
    /// `exchange_timeout`.
    pane_list_timeout: std::time::Duration,
    /// Limits aggregate safe-shutdown probes when a server is degraded.
    ///
    /// Every retry spawns a short-lived tmux process. Session-wide teardown can
    /// wake many reapers together, so clones share this admission limit rather
    /// than amplifying one stuck server into unbounded process churn.
    shutdown_admission: Arc<tokio::sync::Semaphore>,
    /// A deterministic hold around the shutdown command, tests only.
    ///
    /// The production handoff is an external tmux command whose process exit
    /// is its acknowledgement. Tests need to hold that exact boundary open so
    /// they can prove the output client remains alive until it completes;
    /// injecting a gate here does that without changing process environment or
    /// teaching production code about a fake tmux binary.
    #[cfg(test)]
    disable_output_gate: Option<Arc<DisableOutputGate>>,
}

/// Test control for the external `no-output` acknowledgement boundary.
#[cfg(test)]
#[derive(Debug, Default)]
struct DisableOutputGate {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    acknowledged: tokio::sync::Notify,
    finish: tokio::sync::Notify,
}

/// The two control-mode budgets a [`TmuxDriver`] is constructed with.
///
/// A named pair rather than two adjacent `Duration` parameters on
/// [`TmuxDriver::new_with_timeouts`]: both fields are the same type, so two
/// bare `Duration`s invite a transposed-argument bug the compiler cannot
/// catch, where the exchange budget and the pane-list budget silently swap.
/// A named field per budget makes a swap a compile error at the call site
/// instead of a latent bug that only a slow pane-list (or a too-eager
/// attach timeout) would ever surface.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TmuxBudgets {
    /// See [`CONTROL_EXCHANGE_TIMEOUT`].
    pub(crate) exchange: std::time::Duration,
    /// See [`PANE_LIST_TIMEOUT`].
    pub(crate) pane_list: std::time::Duration,
}

impl Default for TmuxBudgets {
    /// The production values — see [`TmuxDriver::new`], the only
    /// constructor that uses this default.
    fn default() -> Self {
        TmuxBudgets {
            exchange: CONTROL_EXCHANGE_TIMEOUT,
            pane_list: PANE_LIST_TIMEOUT,
        }
    }
}

/// A tmux release as Farhelm orders it: the numeric `major.minor` pair
/// plus tmux's patch-release letter.
///
/// The letter is not decoration, which is why this type exists at all.
/// [`TMUX_FLOOR`] names one specific regression-tested build, and the
/// letter is the only thing distinguishing it from the distinct
/// below-floor releases it shares a `major.minor` with: 3.7, 3.7a and
/// 3.7b are three separate releases, and 3.7b in particular is heavily
/// exercised in this file's own history — below the current pin, not
/// untried. The version check this replaced compared `(major, minor)`
/// alone, so it could not tell them apart and would have accepted any of
/// them as the pinned one.
///
/// Ordering is the derived lexicographic one, and it matches tmux's own
/// release order because `None < Some('a')`: a bare `3.7` shipped before
/// `3.7a`, which shipped before `3.7b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TmuxVersion {
    major: u32,
    minor: u32,
    /// tmux's patch-release letter (`3.7b`'s `b`); `None` for a bare
    /// `3.7`. At most one lowercase letter — see [`parse_tmux_version`]
    /// for why anything else is refused rather than approximated.
    patch: Option<char>,
}

impl std::fmt::Display for TmuxVersion {
    /// Round-trips [`parse_tmux_version`]'s input spelling, because this
    /// is what the refusal message and the release pin are compared
    /// against — a version that printed differently from how tmux spells
    /// it would make both unreadable.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}", self.major, self.minor)?;
        if let Some(patch) = self.patch {
            write!(formatter, "{patch}")?;
        }
        Ok(())
    }
}

/// The oldest tmux Farhelm will drive: the version its own regression
/// suites are run against, not the oldest one that happens to work.
///
/// Farhelm drives tmux far harder than interactive use does — control
/// mode, output-client teardown, pane-death timing — and versions are not
/// interchangeable there. Crashes have been observed on older ones (a
/// distro 3.4 server hosting live sessions aborted on 2026-08-19, and
/// BUGS.md records the same abort class reproduced on distro 3.6), while
/// the pinned build carries a crash-regression suite of its own
/// (`scripts/test-tmux-pinned-shutdown.sh`). So the floor tracks the pinned
/// build in `.github/release/source-pins.env` rather than tracking what
/// distributions ship.
///
/// It is DESIGNED to exclude current distro packages (Ubuntu 24.04 ships
/// 3.4, 26.04 about 3.6, Debian 13 and Fedora 42 3.5a). The documented
/// ways to satisfy it are Homebrew/Linuxbrew, Farhelm's own private build
/// that provisioning installs, and the `--tmux` / `FARHELM_TMUX` override
/// (see [`resolve_tmux_program`]).
///
/// Bumping it is a deliberate act taken TOGETHER with the pinned build and
/// its regression suite, never on its own. The tests'
/// `floor_and_release_pin_cannot_drift` enforces that half mechanically:
/// it fails the moment this constant and `TMUX_VERSION=` in
/// `.github/release/source-pins.env` disagree.
pub const TMUX_FLOOR: TmuxVersion = TmuxVersion {
    major: 3,
    minor: 7,
    patch: Some('c'),
};

/// The exact prefix `tmux -V` puts in front of the version. Nothing else
/// is accepted: a line that does not start this way did not come from a
/// tmux this project knows how to reason about.
const TMUX_VERSION_LINE_PREFIX: &str = "tmux ";

/// Parse a whole `tmux -V` line — `"tmux 3.7b\n"` — into an orderable
/// version.
///
/// Conservative by contract: anything this cannot read exactly is an
/// error, never a guess. Every caller turns that error into "refuse" or
/// "install our own build", which is the safe direction — a version
/// Farhelm cannot name is a version nobody has audited it against, and
/// guessing high buys a wedged or crashing substrate.
///
/// THE WHOLE LINE is validated, not just a token lifted out of it. An
/// earlier revision took `split_whitespace().nth(1)`, which happily read
/// `not-tmux 3.7c`, `tmux 3.7c vendor-patch` and `tmux +3.07c` as clean
/// releases. That is not a cosmetic gap: this same parser decides whether
/// provisioning SKIPS installing Farhelm's own audited build, so a
/// decorated or forged line that scans as an audited release is exactly
/// how an unaudited substrate gets adopted silently. So: the `tmux `
/// prefix exactly, one version token and nothing after it, at most one
/// trailing newline, and canonical decimal components (no sign, no
/// leading zeros).
///
/// It takes the whole line rather than a bare number because that is the
/// shape callers have — the provisioning probe forwards a remote host's
/// `tmux -V` output verbatim, and "tmux is not installed" text must fail
/// parsing rather than resemble a version. [`parse_tmux_version_number`]
/// is the entry point for the places that legitimately hold a bare
/// version instead (tmux's `#{version}` format).
///
/// DELIBERATELY NARROW, and this is a product decision rather than a
/// parser limitation: Farhelm supports stable releases and tmux's
/// single-letter patch releases, so tmux's official development
/// (`next-3.8`) and release-candidate (`3.8-rc`, `3.8-rc2`) spellings are
/// refused even though they are numerically above the floor. Ordering
/// those stages against a stable release is guesswork, and the `--tmux`
/// escape hatch exists for anyone who wants to own that risk in a build
/// of their own. See the `--tmux` flag's help, which states the same
/// contract to users.
pub fn parse_tmux_version(v_output: &str) -> anyhow::Result<TmuxVersion> {
    // Exactly one optional trailing newline; a second one, a `\r`, or any
    // other trailing whitespace falls through to the token check below
    // and is refused there.
    let line = v_output.strip_suffix('\n').unwrap_or(v_output);
    let version = line
        .strip_prefix(TMUX_VERSION_LINE_PREFIX)
        .with_context(|| {
            format!("tmux -V returned {line:?}, which is not a {TMUX_VERSION_LINE_PREFIX:?} line")
        })?;
    if version.is_empty() || version.contains(char::is_whitespace) {
        bail!("tmux -V returned {line:?}, which is not exactly one version token");
    }
    parse_tmux_version_number(version)
        .with_context(|| format!("tmux -V returned an unrecognized version {version:?}"))
}

/// [`parse_tmux_version`] minus the `tmux ` prefix and the line framing.
///
/// Split out for two callers, not just for tidiness: the `-V` line parser
/// above, and the adopted-server check, which reads tmux's `#{version}`
/// format — verified against 3.7b and 3.7c to print the bare version
/// (`3.7c\n`) with no `tmux ` prefix. The error context naming the
/// offending token is added once, by whichever of the two called in.
fn parse_tmux_version_number(version: &str) -> anyhow::Result<TmuxVersion> {
    let (major, rest) = version
        .split_once('.')
        .context("expected a major.minor version")?;
    let major = parse_version_component(major, "major")?;
    let suffix_at = rest
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(rest.len());
    let (minor, suffix) = rest.split_at(suffix_at);
    let minor = parse_version_component(minor, "minor")?;
    // tmux's own patch spelling is exactly one lowercase letter. Anything
    // longer is a build this project has never seen, and pretending the
    // first letter of it is a patch level would silently admit it.
    let patch = match suffix.as_bytes() {
        [] => None,
        [letter] if letter.is_ascii_lowercase() => Some(char::from(*letter)),
        _ => bail!("unrecognized patch suffix {suffix:?}"),
    };
    Ok(TmuxVersion {
        major,
        minor,
        patch,
    })
}

/// One numeric component of a version, in tmux's own canonical spelling.
///
/// Stricter than `u32::from_str` on purpose. `parse` accepts `+3` and
/// `0007`, both of which would then round-trip through [`TmuxVersion`]'s
/// `Display` as a DIFFERENT string from the one tmux printed — and the
/// refusal message, the release-pin lockstep test, and provisioning's
/// floor comparison all trade in those printed forms. Refusing the
/// non-canonical spellings keeps parse and print exact inverses.
fn parse_version_component(text: &str, which: &str) -> anyhow::Result<u32> {
    if text.is_empty() {
        bail!("the {which} version is empty");
    }
    if !text.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!("the {which} version {text:?} is not a plain decimal number");
    }
    if text.len() > 1 && text.starts_with('0') {
        bail!("the {which} version {text:?} has a leading zero");
    }
    text.parse()
        .with_context(|| format!("parsing the {which} version {text:?}"))
}

/// Where one discovered tmux version sits relative to [`TMUX_FLOOR`].
///
/// A pure verdict, deliberately separated from the messages: the SAME
/// policy has to govern two different subjects — the client executable
/// this driver spawns, and the long-lived server it may ADOPT (see
/// [`TmuxDriver::require_supported_server`]) — and those two need
/// different prose while sharing one definition of acceptable. Splitting
/// them also makes the policy testable without capturing log output, so
/// "3.7c must not warn, 3.7d must" is an ordinary assertion instead of a
/// tracing-subscriber fixture.
///
/// Public because `farhelm helm setup` decides the same question about a
/// tmux it is about to pin into a unit file, and must reach that verdict
/// through this policy rather than its own comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TmuxSupport {
    /// Older than the regression-tested build: refuse.
    BelowFloor,
    /// Exactly the pinned, regression-tested build: the silent, blessed
    /// case.
    AtFloor,
    /// Newer than the pin: accepted, but nobody has audited Farhelm
    /// against it, so the caller warns.
    AbovePin,
}

/// Classify a discovered version against the floor.
///
/// Newer-than-pinned is deliberately NOT a refusal. Homebrew will ship a
/// version nobody has audited Farhelm against long before this floor
/// moves, and refusing it would strand users on a release the project
/// itself recommends installing; the warning its callers emit records
/// that the combination is untested so a later bug report starts from the
/// right place.
pub fn classify_tmux_version(found: TmuxVersion) -> TmuxSupport {
    match found.cmp(&TMUX_FLOOR) {
        std::cmp::Ordering::Less => TmuxSupport::BelowFloor,
        std::cmp::Ordering::Equal => TmuxSupport::AtFloor,
        std::cmp::Ordering::Greater => TmuxSupport::AbovePin,
    }
}

/// What running `<program> -V` proved about one candidate tmux.
///
/// `program` is echoed back because the caller's candidate may have come
/// from a `PATH` search it did not do itself, and every message about a
/// rejected tmux has to name which binary answered.
#[derive(Debug, Clone)]
pub struct TmuxProbe {
    pub program: PathBuf,
    pub version: TmuxVersion,
    pub support: TmuxSupport,
}

/// The three ways a candidate tmux fails to produce a verdict, kept apart
/// because they are different things to tell a user: one binary could not
/// be run at all, one ran and said something this project cannot read, and
/// one ran but would not shut up or finish.
#[derive(Debug)]
pub enum TmuxProbeError {
    NotRunnable(std::io::Error),
    Unparseable(String),
    /// The candidate exceeded the probe's time or output budget. Carries a
    /// human-readable statement of which.
    Overran(String),
}

impl std::fmt::Display for TmuxProbeError {
    /// The detail a refusal message appends after naming the candidate.
    /// Deliberately a fragment, not a sentence: the caller supplies the
    /// subject ("a tmux at /x/tmux that could not be run").
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRunnable(error) => write!(formatter, "{error}"),
            Self::Unparseable(printed) if printed.is_empty() => {
                formatter.write_str("it printed nothing")
            }
            Self::Unparseable(printed) => write!(formatter, "it printed {printed:?}"),
            Self::Overran(detail) => formatter.write_str(detail),
        }
    }
}

/// How long a candidate gets to answer `-V`.
///
/// Generous for a program whose whole job here is to print one line, and
/// short enough that a wedged candidate cannot hold up a CLI command the
/// operator is watching.
const PROBE_DEADLINE: Duration = Duration::from_secs(5);

/// How much of each stream the probe will read.
///
/// A version line is tens of bytes. Anything past this is not an answer,
/// and reading it unbounded is how a hostile or broken candidate turns a
/// version check into an out-of-memory kill.
const PROBE_CAPTURE_LIMIT: usize = 4 * 1024;

/// Ask a specific tmux binary what version it is.
///
/// Synchronous and side-effect free by design: `tmux -V` answers without
/// touching the socket, the config, or any server, which is what lets
/// `farhelm helm setup` — an ordinary CLI command with no async runtime —
/// use the same floor policy the supervisor enforces at startup.
///
/// BOUNDED in both directions, because setup points this at whatever the
/// operator named or `PATH` produced, which is not necessarily tmux: the
/// candidate gets [`PROBE_DEADLINE`] to exit and [`PROBE_CAPTURE_LIMIT`]
/// bytes per stream, and exceeding either is [`TmuxProbeError::Overran`]
/// rather than a hang or an unbounded allocation. A candidate that spawns
/// a descendant holding the captured pipes open cannot stall the probe
/// either: the collection step has its own short budget and gives up on
/// the readers rather than waiting for EOF.
///
/// A non-zero exit is [`TmuxProbeError::Unparseable`] carrying the trimmed
/// STDERR — something ran under that name and did not answer as tmux,
/// which is the same actionable fact as an unreadable version line, and
/// stderr is where such a program says why. `program` is used verbatim, so
/// a bare name is resolved by the OS against the caller's own `PATH`
/// rather than this function's idea of one.
pub fn probe_tmux(program: &Path) -> Result<TmuxProbe, TmuxProbeError> {
    use std::os::unix::process::CommandExt as _;

    let mut child = std::process::Command::new(program)
        .arg("-V")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        // Its OWN process group. That is what makes the containment below
        // possible at all: a candidate that forks before misbehaving
        // otherwise leaves descendants this function has no handle on, no
        // way to name, and no way to kill — and they inherited the pipes,
        // so they can hold the reader threads open after the probe has
        // returned. One group means one kill reaches everything the probe
        // started.
        .process_group(0)
        .spawn()
        .map_err(TmuxProbeError::NotRunnable)?;
    // `process_group(0)` makes the leader's pid the group id.
    let group = ProbeGroup(child.id());
    let stdout = capture_bounded(child.stdout.take().expect("piped probe stdout"));
    let stderr = capture_bounded(child.stderr.take().expect("piped probe stderr"));

    let deadline = std::time::Instant::now() + PROBE_DEADLINE;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() >= deadline => {
                return Err(group.retire(
                    &mut child,
                    stdout,
                    stderr,
                    format!(
                        "it did not answer -V within {} seconds",
                        PROBE_DEADLINE.as_secs()
                    ),
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => return Err(TmuxProbeError::NotRunnable(error)),
        }
    };

    // The leader is gone, which does NOT mean the pipes are closed: a
    // descendant it forked inherited them and can hold them open forever.
    // Collection is bounded for that reason, and an expired budget is
    // itself the evidence that something is still alive in the group.
    let collected = match (
        stdout.receiver.recv_timeout(PROBE_DRAIN_BUDGET),
        stderr.receiver.recv_timeout(PROBE_DRAIN_BUDGET),
    ) {
        (Ok(Ok(out)), Ok(Ok(err))) => Ok((out, err)),
        (Ok(Err(())), _) => Err(overflowed("stdout")),
        (_, Ok(Err(()))) => Err(overflowed("stderr")),
        (Err(_), _) => Err(still_held("stdout")),
        (_, Err(_)) => Err(still_held("stderr")),
    };
    let (out, err) = match collected {
        Ok(streams) => streams,
        Err(detail) => return Err(group.retire(&mut child, stdout, stderr, detail)),
    };
    // Nothing is left to contain on this path: both readers reached EOF,
    // which every writer had to close for. Join them so the probe owns no
    // live thread when it returns.
    stdout.finish();
    stderr.finish();

    if !status.success() {
        return Err(TmuxProbeError::Unparseable(err.trim().to_string()));
    }
    let version = parse_tmux_version(&out)
        .map_err(|_| TmuxProbeError::Unparseable(out.trim().to_string()))?;
    Ok(TmuxProbe {
        program: program.to_path_buf(),
        version,
        support: classify_tmux_version(version),
    })
}

fn overflowed(which: &str) -> String {
    format!("it printed more than {PROBE_CAPTURE_LIMIT} bytes on {which}")
}

fn still_held(which: &str) -> String {
    format!("its {which} was still held open after it exited")
}

/// The process group one probe created, and the only handle that reaches
/// everything that probe started.
struct ProbeGroup(u32);

impl ProbeGroup {
    /// Kill the whole group, reap the leader, and join both reader
    /// threads, then report the overrun that got us here.
    ///
    /// The order is load-bearing. The group is killed BEFORE the leader is
    /// reaped where that is still possible, because a group whose members
    /// have all been reaped no longer reserves its id. The joins come last
    /// and are bounded by the kill: with every writer dead, the pipes are
    /// closed and both readers see EOF.
    ///
    /// The residual risk is honest to state: if the leader was already
    /// reaped and the group is now genuinely empty, its id could in
    /// principle have been reused by an unrelated group, and this signals
    /// that one. It takes a full pid-space wrap between the two moments,
    /// and the alternative — leaving a hostile candidate's descendants
    /// running with our pipes — is the worse trade.
    fn retire(
        &self,
        child: &mut std::process::Child,
        stdout: Capture,
        stderr: Capture,
        detail: String,
    ) -> TmuxProbeError {
        // SAFETY: `kill` with a negative pid signals a process group and
        // touches no memory. A group that is already gone answers ESRCH,
        // which is the outcome this wants anyway.
        unsafe { libc::kill(-(self.0 as i32), libc::SIGKILL) };
        let _ = child.kill();
        let _ = child.wait();
        stdout.finish();
        stderr.finish();
        TmuxProbeError::Overran(detail)
    }
}

/// One child stream being read on its own thread, bounded.
struct Capture {
    receiver: std::sync::mpsc::Receiver<Result<String, ()>>,
    reader: std::thread::JoinHandle<()>,
}

impl Capture {
    /// Wait for the reader thread to end. Only safe to call once every
    /// writer for its pipe is gone, which is why the callers either saw
    /// EOF or killed the group first.
    fn finish(self) {
        let _ = self.reader.join();
    }
}

/// Read at most [`PROBE_CAPTURE_LIMIT`] bytes from one child stream on a
/// thread, reporting overflow rather than growing.
///
/// A thread rather than sequential reads because the child writes both
/// streams and a single-threaded reader can deadlock against the pipe
/// buffer of the one it is not reading. The result is SENT before the
/// thread ends, so a caller can have the answer while the thread is still
/// waiting for EOF on a pipe some descendant holds.
fn capture_bounded(mut stream: impl std::io::Read + Send + 'static) -> Capture {
    let (tx, receiver) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        use std::io::Read as _;

        let mut buffer = Vec::new();
        // One byte past the limit is how overflow is detected without
        // reading the rest of whatever the candidate is producing.
        let read = stream
            .by_ref()
            .take(PROBE_CAPTURE_LIMIT as u64 + 1)
            .read_to_end(&mut buffer);
        let answer = match read {
            Ok(_) if buffer.len() > PROBE_CAPTURE_LIMIT => Err(()),
            Ok(_) => Ok(String::from_utf8_lossy(&buffer).into_owned()),
            Err(_) => Ok(String::new()),
        };
        let _ = tx.send(answer);
    });
    Capture { receiver, reader }
}

/// How long the probe waits for a reader once the child is gone.
///
/// The child has exited by then, so this is only about a DESCENDANT it
/// left holding the pipe. Waiting for that would reintroduce the hang the
/// deadline exists to prevent, so an expired budget becomes an overrun —
/// and, unlike before, the group kill that follows retires the descendant
/// and the reader with it.
const PROBE_DRAIN_BUDGET: Duration = Duration::from_millis(250);

/// Find one program by name on a `PATH` value.
///
/// `path` is a parameter rather than a read of this process's environment
/// so callers can search the environment they captured (and so tests never
/// have to mutate their own). Returns `None` when nothing on `PATH` under
/// that name looks executable — a caller that wants the OS's own error
/// should spawn the bare name instead.
///
/// NOTE that "looks executable" is an approximation (see
/// [`is_executable_file`]), so the first hit is not guaranteed to run. A
/// caller that will actually SPAWN the result should walk
/// [`candidates_on_path`] instead and skip the ones that fail to start,
/// which is what `execvp` does and what this cannot do on its own.
pub fn find_on_path(path: &std::ffi::OsStr, name: &str) -> Option<PathBuf> {
    candidates_on_path(path, name).next()
}

/// Every `PATH` entry that plausibly holds this program, in `PATH` order.
///
/// Exists because [`find_on_path`]'s answer can be wrong in a way only
/// spawning reveals: a `noexec` mount, an execute bit set for a group this
/// process is not in, or an LSM denial all look executable to a metadata
/// check. `execvp` skips such an entry and keeps walking, so a caller that
/// spawns must be able to as well — otherwise one unusable shadow early on
/// `PATH` hides a perfectly good tmux later.
pub fn candidates_on_path<'a>(
    path: &'a std::ffi::OsStr,
    name: &'a str,
) -> impl Iterator<Item = PathBuf> + 'a {
    let program = Path::new(name);
    std::env::split_paths(path)
        // An empty PATH entry means the current directory to the shell.
        // Skipping it here is deliberate: the result is going to be
        // written into a systemd unit, and "whatever directory setup
        // happened to run in" is never a defensible thing to pin.
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(move |dir| dir.join(program))
        .filter(|candidate| is_executable_file(candidate))
}

/// The shared search behind [`find_on_path`] and [`program_display_path`].
fn search_path(path_var: &std::ffi::OsStr, program: &Path) -> Option<PathBuf> {
    std::env::split_paths(path_var)
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable_file(candidate))
}

/// Enforce the version floor against the binary this driver will actually
/// run, before a server is started or adopted.
///
/// `program` is the path the caller probed, and it is in the refusal text
/// on purpose: the failure this message exists for is "the wrong tmux was
/// on PATH", which is unfixable without knowing WHICH tmux answered. It is
/// the loudest on macOS, where a GUI app inherits no shell PATH.
///
/// This covers only HALF the substrate. Clearing it means a fresh server
/// would be started from an acceptable binary; it says nothing about a
/// server already running on the private socket, which `start-server`
/// adopts instead of replacing. [`TmuxDriver::require_supported_server`]
/// is the other half and runs right after.
fn require_supported_tmux(program: &Path, v_output: &str) -> anyhow::Result<()> {
    let found = parse_tmux_version(v_output)
        .with_context(|| format!("checking the tmux at {}", program.display()))?;
    match classify_tmux_version(found) {
        TmuxSupport::BelowFloor => bail!(
            "tmux {found} at {} is below Farhelm's floor {TMUX_FLOOR} (see README: tmux)",
            program.display()
        ),
        TmuxSupport::AtFloor => {}
        TmuxSupport::AbovePin => warn!(
            tmux = %program.display(),
            found = %found,
            tested = %TMUX_FLOOR,
            "tmux is newer than the version Farhelm is tested against; this combination is \
             unaudited"
        ),
    }
    Ok(())
}

/// Wording for a `-V` probe that failed because the OS could not find
/// `program` at all (`io::ErrorKind::NotFound`) — as opposed to one that ran
/// and refused (parsed by [`require_supported_tmux`]) or one that could not
/// be spawned for some other reason (permissions, `ENOEXEC`, ...), which
/// keep the ordinary "checking the tmux version of ..." context instead.
///
/// Two spellings because a bare name and a resolved path failing point at
/// different repairs: a bare `tmux` failing means nothing on `PATH`
/// answered to that name, so the fix is installing tmux somewhere on
/// `PATH` (or pointing `--tmux`/`FARHELM_TMUX` at one); a resolved path
/// failing means whatever named it — `--tmux`, `FARHELM_TMUX`, or a
/// systemd unit's environment — points at something wrong, and the message
/// names that exact path so the operator knows which override to fix.
///
/// The bare-name wording is deliberately SOURCE-NEUTRAL: `program` alone
/// cannot tell this function whether the name is [`DEFAULT_TMUX_PROGRAM`]'s
/// unconfigured fallback or an explicit one-word `--tmux custom-tmux` /
/// `FARHELM_TMUX=custom-tmux` override — both are indistinguishable
/// one-component paths by the time they reach here. Earlier wording
/// guessed "unconfigured" and said `FARHELM_TMUX unset, nothing on PATH`
/// unconditionally, which is simply false for an operator who set an
/// explicit override that PATH could not resolve.
///
/// Neither spelling claims outright that `program` does not EXIST: on
/// Unix, `ENOENT` from a spawn attempt also covers a script whose shebang
/// interpreter is gone or a binary whose dynamic loader is missing, and
/// distinguishing that from "nothing there at all" would need an extra
/// existence check this function does not perform. "Could not be run" is
/// accurate for every one of those causes without pretending to know
/// which.
///
/// This replaces the misleading `"checking the tmux version of tmux"` this
/// project shipped before: that phrasing is `program_display_path`'s
/// fall-back spelling for a bare name PATH could not resolve, and read
/// like tmux ran and failed rather than like tmux was never found at all.
fn tmux_not_found_message(program: &Path) -> String {
    if program.components().count() == 1 {
        format!(
            "no tmux could be run: `{}` was not found on PATH, or its interpreter or loader is \
             missing",
            program.display()
        )
    } else {
        format!(
            "no tmux could be run at `{}`: not found, or its interpreter or loader is missing",
            program.display()
        )
    }
}

/// The environment variable naming the tmux binary to drive.
///
/// Public because the desktop app sets it on the supervisor it spawns:
/// macOS GUI apps inherit no shell PATH, so the app probes the Homebrew
/// prefixes itself and hands the winner down through this variable.
pub const TMUX_PROGRAM_ENV: &str = "FARHELM_TMUX";

/// What Farhelm drives when nothing overrides it: plain `tmux`, resolved
/// by the operating system against `PATH` at spawn time, exactly as every
/// release before the override knob existed.
pub const DEFAULT_TMUX_PROGRAM: &str = "tmux";

/// Resolve which tmux binary a supervisor drives: `--tmux` wins, then
/// [`TMUX_PROGRAM_ENV`], then [`DEFAULT_TMUX_PROGRAM`] off `PATH`.
///
/// Both inputs are parameters rather than reads of this process's
/// environment, which is what makes the precedence testable: this repo's
/// tests never mutate the test process's environment, and a resolution
/// that read `FARHELM_TMUX` itself could only be exercised by doing so.
/// Production reaches the real environment through
/// [`resolve_tmux_program_from_env`].
///
/// An empty `FARHELM_TMUX` counts as unset. A systemd unit or shell
/// profile that writes `FARHELM_TMUX=` means "no override" to whoever
/// wrote it, and the alternative reading — spawn a program with an empty
/// name — can only ever fail.
///
/// Choosing a binary here does not exempt it from anything: the result is
/// floor-checked like any other candidate and refused by name if it is too
/// old (see [`require_supported_tmux`]). The override means "you own the
/// substrate", not "this configuration is supported" — Farhelm drives tmux
/// harder than interactive use, versions below [`TMUX_FLOOR`] have crashed
/// under it, and versions above it are unaudited.
pub fn resolve_tmux_program(flag: Option<&Path>, env: Option<&std::ffi::OsStr>) -> PathBuf {
    if let Some(flag) = flag {
        return flag.to_path_buf();
    }
    match env {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => PathBuf::from(DEFAULT_TMUX_PROGRAM),
    }
}

/// [`resolve_tmux_program`] against this process's real environment.
///
/// The single caller is `farhelm supervisor run`'s startup, which is what
/// makes "chosen once, then threaded everywhere" true: no code below it
/// ever consults the environment again, so no launch path can disagree
/// with another about which tmux this supervisor drives.
///
/// What is fixed here is the SPELLING, not a file. A bare name stays a
/// bare name and is looked up against `PATH` afresh by each spawn — see
/// [`program_display_path`] for why that is deliberate, and for the
/// replacement race it leaves open.
pub fn resolve_tmux_program_from_env(flag: Option<&Path>) -> PathBuf {
    resolve_tmux_program(flag, std::env::var_os(TMUX_PROGRAM_ENV).as_deref())
}

/// Spell a program the way a human can act on it: a bare name is reported
/// as the `PATH` entry it resolves to.
///
/// ADVISORY, not authoritative, and the distinction is the whole reason
/// this doc exists. Nothing is executed through the returned path: the
/// driver keeps the SPELLING it was given and lets `Command` redo the
/// `PATH` lookup on every spawn, because resolving here and spawning by
/// absolute path would open a time-of-check gap the OS's own lookup does
/// not have. The flip side is honest to state: between this lookup and
/// the spawn, the winning `PATH` entry can be replaced, so the path in a
/// diagnostic is the best available name for what answered, not proof of
/// which inode did.
///
/// It exists because "tmux 3.6 is too old" is unactionable while "the
/// tmux 3.6 at /usr/bin/tmux is too old" tells the reader which entry to
/// fix. `path_var` is a parameter so the search is testable without
/// touching the test process's environment.
///
/// Public so the desktop app's own below-floor refusal can name the same
/// resolved binary this driver's server-side refusal does, rather than
/// printing whatever bare spelling `--tmux`/`FARHELM_TMUX` happened to
/// carry — see `farhelm_ui::desktop::run_tmux_preflight_or_exit`.
pub fn program_display_path(program: &Path, path_var: Option<&std::ffi::OsStr>) -> PathBuf {
    // A name with any separator is already a path; the OS would not
    // consult PATH for it either.
    if program.components().count() != 1 {
        return program.to_path_buf();
    }
    path_var
        .and_then(|path_var| search_path(path_var, program))
        // Falling back to the bare name is right: the spawn that follows
        // will fail with the OS's own "no such file" and that error is
        // clearer than a path this function invented.
        .unwrap_or_else(|| program.to_path_buf())
}

/// Whether a `PATH` candidate LOOKS like something the OS would execute.
///
/// An approximation of `execvp`'s test, and knowingly so: it asks whether
/// ANY execute bit is set rather than whether THIS process's uid/gid can
/// use the one that applies, and it cannot see a `noexec` mount or an
/// LSM denial. That is the right trade for a diagnostic — the cheap check
/// removes the shadows that actually occur (data files, unstripped
/// permissions on a copied binary) without a permission model this code
/// would then have to keep true.
///
/// Because it is an approximation, a caller that will SPAWN the result
/// must be prepared for it to fail anyway and move on; that is what
/// [`candidates_on_path`] is for.
fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    // Windows has no execute bit; being a regular file with the right
    // name is as close as this diagnostic can get.
    #[cfg(not(unix))]
    {
        true
    }
}

/// Pane state needed to make a fresh xterm.js behave as if it had been
/// attached all along. Captured from tmux format variables at attach
/// time; content replay alone silently loses these (SPEC_impl.md:
/// bracketed paste and mouse reporting are the headline casualties).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaneModes {
    pub alternate_on: bool,
    pub bracket_paste: bool,
    /// DECSET 1003 (any-event mouse tracking), captured from tmux's own
    /// `#{mouse_all_flag}` — NOT `#{mouse_any_flag}`, which despite its
    /// name is an umbrella "some mouse protocol is active" bit that also
    /// reads 1 whenever `mouse_button` or `mouse_standard` does; using it
    /// here was audited and rejected (see `post_content_sequences`'s
    /// historical note). `mouse_all_flag` is the field tmux dedicates to
    /// this ONE protocol specifically, verified empirically (tmux 3.7b)
    /// to read 1 only when 1003 alone is active.
    pub mouse_all: bool,
    pub mouse_button: bool,
    pub mouse_standard: bool,
    pub mouse_sgr: bool,
    pub cursor_visible: bool,
    pub app_cursor_keys: bool,
    /// Cursor column, 0-based as tmux reports it. The CUP escape sequence
    /// is 1-based, so `post_content_sequences` adds one — do not
    /// pre-adjust here or the cursor lands a cell off on every reattach.
    pub cursor_x: u16,
    /// Cursor row, 0-based; see `cursor_x`.
    pub cursor_y: u16,
    /// tmux's `#{pane_dead}` at the moment replay was captured — the same
    /// flag [`PaneProcess::dead`] reports, fetched here instead so the
    /// `Attach` handler (service.rs) can decide whether to append the
    /// alt-screen stop snapshot without a second tmux round trip. See
    /// [`PANE_MODE_FORMAT`]'s docs.
    pub pane_dead: bool,
}

impl PaneModes {
    /// Parse the comma-separated format expansion produced by
    /// `TmuxDriver::pane_modes`. Empty or unparseable fields take the
    /// mode's default (off, except cursor visibility which defaults on),
    /// so a tmux build lacking one format name loses exactly that one
    /// mode rather than corrupting the rest.
    fn parse(line: &str) -> PaneModes {
        let mut it = line.split(',');
        let mut flag = |default: bool| -> bool {
            match it.next() {
                Some("") | None => default,
                Some(v) => v == "1",
            }
        };
        let alternate_on = flag(false);
        let bracket_paste = flag(false);
        let mouse_all = flag(false);
        let mouse_button = flag(false);
        let mouse_standard = flag(false);
        let mouse_sgr = flag(false);
        let cursor_visible = flag(true);
        let app_cursor_keys = flag(false);
        let mut num = || -> u16 { it.next().and_then(|v| v.parse().ok()).unwrap_or(0) };
        let cursor_x = num();
        let cursor_y = num();
        // `num`'s borrow of `it` ends at `cursor_y` above (its last use),
        // so this can read `it` directly rather than reusing either
        // closure. `Some("")` and `None` both fall through to `false`
        // here exactly as `flag`'s default does, since `matches!` only
        // matches the literal `Some("1")`.
        let pane_dead = matches!(it.next(), Some("1"));
        PaneModes {
            alternate_on,
            bracket_paste,
            mouse_all,
            mouse_button,
            mouse_standard,
            mouse_sgr,
            cursor_visible,
            app_cursor_keys,
            cursor_x,
            cursor_y,
            pane_dead,
        }
    }

    /// Escape sequences that must precede the content prefill.
    ///
    /// The alternate-screen switch belongs here and nowhere else:
    /// `\x1b[?1049h` switches to a *cleared* alternate buffer, so
    /// emitting it after the prefill would wipe the very content just
    /// replayed and leave the reattaching user staring at a blank
    /// screen. The cutover snapshot returns alt-screen contents when the
    /// pane is on the alternate screen, so the buffer must be selected
    /// first.
    pub fn pre_content_sequences(&self) -> String {
        if self.alternate_on {
            "\x1b[?1049h".to_string()
        } else {
            String::new()
        }
    }

    /// Escape sequences that follow the content prefill: input modes,
    /// then cursor placement. These must come after the content because
    /// writing the prefill moves the cursor.
    pub fn post_content_sequences(&self) -> String {
        let mut s = String::new();
        if self.bracket_paste {
            s.push_str("\x1b[?2004h");
        }
        // Mouse tracking is one protocol slot in tmux's own model:
        // `mouse_button`/`mouse_standard`/`mouse_all` are mutually
        // exclusive by construction (verified empirically, tmux 3.7b —
        // each of `?1000h`/`?1002h`/`?1003h` sent alone sets exactly one
        // of the three), so at most one `if` below ever fires. The
        // `else if` chain is belt-and-suspenders against an impossible
        // mixed capture, not load-bearing precedence logic; kept anyway
        // because "at most one" is an invariant of tmux's behavior, not
        // of Rust's type system, and a defensive shape costs nothing here.
        //
        // Historical note: an earlier version of this struct captured
        // `#{mouse_any_flag}` instead of `#{mouse_all_flag}` for this
        // field, on the mistaken belief that "any" meant "DECSET 1003".
        // It does not — `mouse_any_flag` is an UMBRELLA bit that reads 1
        // whenever `mouse_button` OR `mouse_standard` does too, so restoring
        // it as if it meant 1003 silently re-asserted any-event tracking on
        // EVERY reattach, regardless of what the pane's application had
        // actually requested (caught by `mouse-modes.spec.ts`'s
        // `mouse-modes-restored-on-reattach`, PLAN_M6_5.md item 2: a plain
        // `?1000h` pane came back from a detach/reattach cycle reporting
        // motion the agent never asked for). `#{mouse_all_flag}` is the
        // field tmux dedicates to 1003 specifically — use that one here,
        // never `mouse_any_flag`.
        if self.mouse_button {
            s.push_str("\x1b[?1002h");
        } else if self.mouse_standard {
            s.push_str("\x1b[?1000h");
        } else if self.mouse_all {
            s.push_str("\x1b[?1003h");
        }
        if self.mouse_sgr {
            s.push_str("\x1b[?1006h");
        }
        if self.app_cursor_keys {
            s.push_str("\x1b[?1h");
        }
        // Cursor position is 1-based in the escape sequence.
        s.push_str(&format!(
            "\x1b[{};{}H",
            self.cursor_y + 1,
            self.cursor_x + 1
        ));
        // Visibility last so a hidden cursor stays hidden through the
        // positioning above.
        if !self.cursor_visible {
            s.push_str("\x1b[?25l");
        }
        s
    }
}

/// A pane's process state as stop/delete need it: the OS pid
/// `kill_process_tree` (service.rs) reaps from, and whether tmux has
/// already marked the pane dead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneProcess {
    /// The pane's ORIGINAL child, the one `new-session`/`split-window`
    /// forked (a shell, or the exec'd argv when tmux runs it directly) —
    /// not whatever the agent turns into after its own exec chain. Every
    /// process the agent starts keeps that pid as an ancestor, which is
    /// exactly the property a tree-kill needs.
    ///
    /// Meaningful only when `dead` is `false`: once tmux marks a pane
    /// dead, this is the LAST pid it remembers for it — stale, and must
    /// never be walked or signaled, since `remain-on-exit`'s whole point
    /// is keeping the pane around after that process (and everything
    /// under it) is long gone, and the number may already be recycled to
    /// something unrelated by the time anyone reads it.
    pub pid: u32,
    /// tmux's own `#{pane_dead}` flag: set once the process backing this
    /// pane has exited. A dead pane means nothing is running in it — stop
    /// and delete both treat this as "no process tree to touch" rather
    /// than attempting to signal the stale `pid` above.
    pub dead: bool,
}

/// Outcome of [`TmuxDriver::pane_process`]: a recorded pane id is either
/// still this session's terminal, gone, or demonstrably somebody else's.
///
/// The third case exists because tmux pane ids (`%N`) come from a
/// server-wide counter that is monotonic only WITHIN one server's
/// lifetime: it restarts at `%0` with the server. Several unrelated
/// things therefore produce a foreign owner, and the list is not closed:
/// the recorded id may predate the current server generation and have
/// been handed to a session that started later (the 2026-08-16 incident:
/// the private tmux server segfaulted out from under a running
/// supervisor, whose in-memory `Terminal` rows kept pointing at the dead
/// server's `%0`); the pane's own session may have been renamed
/// out-of-band, so the pane is still ours but no longer answers to the
/// recorded name; or panes may have been rearranged across sessions
/// (`move-pane`/`break-pane`). Nothing in farhelm does the latter two on
/// its private server, but the socket is reachable by whoever owns the
/// account.
///
/// Those causes do NOT share a safety profile, which is why this type
/// deliberately stops at "the recorded name is not what tmux reports" and
/// leaves the verdict to callers. A recycled id means the old terminal
/// provably died with its server; a rename or a move may mean the
/// recorded pane is still a LIVE agent of ours under another name, and
/// tearing it down would kill an agent without consent. The discriminator
/// — is `owner` the tmux name of another session this supervisor knows? —
/// and the reasoning behind it live at
/// `Supervisor::known_session_tmux_name`, which every lifecycle call site
/// consults.
///
/// Modelled as an outcome rather than an error — the precedent is
/// [`AltScreenCapture::SessionMismatch`], which already distinguishes the
/// same hazard for captures — because a foreign owner is not a failure to
/// ask tmux anything. It is a decisive answer about the NAME, and each
/// caller decides what that means for its own operation: the lifecycle
/// verbs proceed on other kill mechanisms for a recycled id and refuse
/// otherwise, while a pane this process created moments ago being foreign
/// is an invariant violation under any cause. What NO caller may do is
/// read the foreign pane's pid: it anchors kills and descendant sweeps,
/// and pointing those at a stranger is precisely the cross-wiring the
/// session scoping exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneProbe {
    /// The pane exists and `#{session_name}` matches the session it was
    /// recorded under: the recorded terminal is real and this pid may
    /// anchor a kill (subject to [`PaneProcess::dead`]).
    Owned(PaneProcess),
    /// The pane id resolves to nothing — an unassigned id, a session
    /// killed out from under us, or no server at all. Callers read this
    /// as "nothing is running", the same as a positively dead pane.
    Gone,
    /// The pane exists but `#{session_name}` is `owner`, not the name it
    /// was recorded under. `owner` is the COMPLETE name including any
    /// spaces, because callers compare it against their own session names
    /// to tell a recycled id from a rename (see the type's docs); a
    /// truncated name would silently fail that comparison. The pid is
    /// deliberately not carried, because no caller has any business using
    /// it.
    ForeignOwner { owner: String },
}

/// One read-buffer's worth of bytes pulled per `read` call while
/// streaming a bounded capture. 64 KiB balances syscall count against
/// wasted allocation for the common case (a snapshot far under the cap).
const ALT_SCREEN_READ_CHUNK: usize = 64 * 1024;

/// Headroom `capture_pane_plain` retains beyond the caller's `max_bytes`
/// while streaming a capture.
///
/// [`last_words`] trims trailing blank rows only AFTER the read has
/// finished, so a window sized to `max_bytes` exactly could be filled
/// entirely by the padding of a very tall, mostly-empty pane and push the
/// real text out. 64 KiB covers tmux's 10,000-row ceiling with an order of
/// magnitude to spare while keeping the peak allocation per capture in the
/// tens of kilobytes rather than the tens of megabytes an unbounded read
/// of that same pane would reach.
const TAIL_RETAIN_ALLOWANCE: usize = 64 * 1024;

/// Slack added on top of a caller's `max_bytes` cap before
/// [`TmuxDriver::capture_alt_screen_if_active`] gives up and discards.
///
/// The combined invocation's output is the `display-message` header
/// (`"<0|1> <session-name>\n"`, at most a couple hundred bytes for any
/// realistic tmux session name) followed by the `capture-pane` body,
/// which is what the cap is actually meant to bound. Without this slack,
/// a capture whose BODY is legitimately exactly at the cap would be
/// discarded purely because the header pushed the combined stream a few
/// bytes over — an off-by-header-size rejection with no bearing on
/// whether the body itself was reasonably sized.
const ALT_SCREEN_HEADER_SLACK: usize = 1024;

/// One pane's liveness as [`TmuxDriver::pane_states`] reports it —
/// `ListSessions`'s cheaper cousin of [`PaneProcess`]: no pid (status
/// never needs one, only whether the pane is alive and, if not, its exit
/// code), and gathered for every pane on the server in one query rather
/// than one round trip per session.
///
/// No `Default`, deliberately: every field here is either read directly
/// from tmux or not constructed at all (a pane absent from the map is a
/// missing HASHMAP ENTRY, never a fabricated `PaneState`). A derived
/// `Default` would hand out `dead: false` — alive — for a value nothing
/// ever actually observed, which is exactly the fabricated-liveness claim
/// `session_status`'s whole "absent means Exited" contract exists to
/// avoid; this type has no honest zero value to offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneState {
    /// The tmux session this pane belonged to AT QUERY TIME
    /// (`#{session_name}`). Pane ids reset to `%0` on a fresh tmux server
    /// (verified empirically: killing the server and creating a new
    /// session hands its first pane `%0` again), so a caller matching
    /// only on pane id could have a stale, never-reloaded `SessionEntry`
    /// silently inherit an unrelated NEW session's liveness merely
    /// because both happen to share a recycled pane number. Requiring
    /// this to also match the entry's own remembered `tmux_name` (see
    /// `service.rs`'s `session_status`) is what closes that gap — a
    /// mismatch on EITHER pane id or session name is treated as "this
    /// pane is not the one we are asking about" and falls back to the
    /// same honest `Exited { exit_code: None }` as an outright absent
    /// pane.
    pub session_name: String,
    /// tmux's own `#{pane_dead}` flag, exactly as [`PaneProcess::dead`].
    pub dead: bool,
    /// The id (`@N`) of the window this pane lives in.
    ///
    /// Carried for ORDERING, not for addressing (every window-scoped
    /// command here goes through [`pane_in_session`] instead — see its
    /// docs for why a bare window id is unsafe to act on). tmux assigns
    /// window ids from a monotonically increasing per-server counter, so
    /// within one tmux-server lifetime their numeric order IS the order
    /// the windows were created in — which is exactly what
    /// `SessionInfo::tabs` promises its readers.
    pub window: String,
    /// The numeric part of [`Self::window`] (`@7` → 7), parsed once here
    /// because every consumer wants creation ORDER and `@10` sorts before
    /// `@9` as a string.
    pub window_ordinal: u64,
    /// The numeric part of this pane's own id (`%12` → 12), for the same
    /// reason: comparing pane ids as strings puts `%10` before `%9`.
    pub pane_ordinal: u64,
    /// `#{window_index}` — the window's POSITION in its session, as
    /// opposed to the server-wide creation counter [`Self::window`] is.
    ///
    /// Only one consumer: the last-resort agent-window fallback for a
    /// session that carries no markers at all (a session created by a
    /// build that predates them). Position is exactly what that fallback
    /// wants and exactly what nothing else may use.
    pub window_index: u64,
    /// This pane's window's [`TAB_WINDOW_OPTION`] marker, when it carries
    /// one whose SYNTAX matches an id this supervisor mints.
    ///
    /// `None` covers three cases the caller does not need to tell apart,
    /// because all three mean "not one of our tabs": the agent's own
    /// window, a window someone conjured behind the supervisor's back
    /// (pane processes inherit `TMUX`), and a window whose marker holds
    /// something this supervisor would never have minted.
    ///
    /// SYNTAX is all this establishes, and the distinction matters: tmux
    /// window options are writable by anything that inherited `TMUX`, so a
    /// pane can mark its own window — or another window of any session on
    /// this private server — with a perfectly well-formed uuid. What the
    /// validation buys is that such a value cannot be malformed in a way
    /// that shifts the parse or names something outside tmux's own
    /// namespace; what makes acting on it safe is that every operation
    /// addresses the pane paired with its session
    /// ([`pane_in_session`]), so a marker can never reach ANOTHER
    /// session's terminal. Provenance itself is not authenticated and
    /// deliberately not claimed to be.
    pub tab: Option<String>,
    /// This pane's window's [`AGENT_WINDOW_OPTION`] marker — the session
    /// id the window was created for — under the same syntax-only caveat
    /// as [`Self::tab`].
    ///
    /// Read, not merely written: a session whose durable pane record is
    /// empty recovers its agent terminal by preferring the window marked
    /// for that session, rather than by position (see `service.rs`'s
    /// reload). `None` means the window carries no recognizable agent
    /// marker — a tab's window, a foreign window, or a session created
    /// before markers existed.
    pub agent: Option<String>,
    /// `#{pane_dead_status}` when `dead` and parseable as a plain integer.
    /// `None` while alive, and also `None` for a dead pane whose status
    /// tmux could not express as one (a signal death, chiefly) — the same
    /// honest gap `SessionStatus::Exited`'s own docs describe, just
    /// discovered here instead of invented by this struct.
    pub exit_code: Option<i32>,
}

#[cfg(test)]
impl PaneState {
    /// A pane state with everything but the fields a test cares about
    /// filled in plausibly — for the many tests across this crate whose
    /// subject is one field and whose noise floor is the other seven.
    ///
    /// Deliberately NOT a `Default` impl: the production type has no
    /// honest zero value (see the struct's own docs on why `dead: false`
    /// must never be fabricated), and a `Default` would be reachable from
    /// production code as soon as somebody wrote `..Default::default()`.
    pub(crate) fn for_test(session_name: &str, pane: &str, window: &str) -> PaneState {
        PaneState {
            session_name: session_name.to_string(),
            dead: false,
            window: window.to_string(),
            window_ordinal: tmux_ordinal(window, '@').expect("test window ids are well formed"),
            pane_ordinal: tmux_ordinal(pane, '%').expect("test pane ids are well formed"),
            window_index: 0,
            tab: None,
            agent: None,
            exit_code: None,
        }
    }

    /// Builder sugar for the marker under test.
    pub(crate) fn with_tab(mut self, tab: &str) -> PaneState {
        self.tab = Some(tab.to_string());
        self
    }

    /// Builder sugar for the agent marker under test.
    pub(crate) fn with_agent(mut self, session_id: &str) -> PaneState {
        self.agent = Some(session_id.to_string());
        self
    }

    /// Builder sugar for the dead/exit-code pair, which are only ever
    /// meaningful together.
    pub(crate) fn dead_with(mut self, exit_code: Option<i32>) -> PaneState {
        self.dead = true;
        self.exit_code = exit_code;
        self
    }

    /// Builder sugar for the window INDEX, which only the legacy
    /// agent-window fallback reads.
    pub(crate) fn at_index(mut self, index: u64) -> PaneState {
        self.window_index = index;
        self
    }
}

/// tmux's raw, unmodified stderr from a non-zero exit — attached as the
/// ROOT CAUSE of the `anyhow::Error` [`TmuxDriver::run`]/`run_bytes`
/// return (with the human-readable "tmux {args:?} failed (...): ..."
/// message layered on top via `.context(...)`), so a caller can recover
/// exactly what tmux printed via `downcast_ref` regardless of how much
/// further context piles on afterward — the same `anyhow` pattern
/// `service.rs`'s `RequestError` uses, and for the same reason: searching
/// the RENDERED error string is not always safe.
///
/// [`TmuxDriver::pane_states`] is the one caller that needs this: it must
/// recognize a handful of tmux's own diagnostic shapes exactly, and doing
/// that against the rendered message (this driver's own formatting, plus —
/// for the "no server running on `<path>`" diagnostic — a caller-controlled
/// state-dir path that tmux itself bakes into its own text) risks a false
/// match whenever that path happens to CONTAIN one of the recognized
/// phrases as a substring. Matching against this untouched stderr instead
/// closes that hole; see [`is_tolerated_list_panes_diagnostic`]'s own docs
/// for the anchored comparison this makes possible.
#[derive(Debug)]
struct TmuxCommandFailure {
    stderr: Vec<u8>,
}

impl TmuxCommandFailure {
    /// tmux's stderr with surrounding whitespace stripped — the exact text
    /// every diagnostic [`is_tolerated_list_panes_diagnostic`] recognizes
    /// must equal VERBATIM (never merely contain), since tmux emits each
    /// of those diagnostics as a complete, standalone message with nothing
    /// else on the line.
    fn stderr_trimmed(&self) -> String {
        String::from_utf8_lossy(&self.stderr).trim().to_string()
    }
}

impl std::fmt::Display for TmuxCommandFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.stderr_trimmed())
    }
}

impl std::error::Error for TmuxCommandFailure {}

/// Render environment entries as the `NAME=value` strings tmux's `-e`
/// flag takes, one per entry.
///
/// A free function so `new-session` and `respawn-pane` build them
/// identically: the two paths launch the same session's agent, and an
/// environment that differed between a first launch and a relaunch would
/// be exactly the kind of divergence SPEC.md's environment contract
/// forbids. Returns owned strings because the caller borrows them into an
/// argv that outlives this call.
/// The tmux target that names `pane` WITHIN `session` — `=<session>:.<pane>`.
///
/// The leading `=` is tmux's exact-match prefix (no fnmatch, no prefix
/// matching), and pairing the two is what makes a pane reference safe to
/// carry across time: pane ids come from a server-wide counter that
/// restarts at `%0` with the server, so a remembered `%N` can, after a
/// tmux restart, name a live pane belonging to a completely different
/// session. Commands that REPLACE or KILL what they name must not be able
/// to act on that stranger, and tmux refuses a mismatched pairing itself
/// ("can't find pane") — an atomic check rather than a probe that could go
/// stale between asking and acting.
///
/// This doubles as the safe WINDOW target (PLAN_M4.md item 2), which is
/// why window-scoped commands here take a pane rather than a window id: a
/// tmux command given a window target resolves it to the window CONTAINING
/// the named pane, so `resize-window`/`kill-window`/`set-option -w` all
/// inherit the session pairing above. A bare `@N` window id would not —
/// audited empirically against tmux 3.7b, `set-option -w -t
/// '=other-session:@1'` happily acts on `@1` even when `@1` belongs to a
/// DIFFERENT session, because tmux resolves a window id directly and
/// ignores the session qualifier in front of it. Window ids reset with the
/// server exactly as pane ids do, so that combination is the stale-handle
/// hazard above with its one safeguard removed.
///
/// # What this form does NOT do: detect a VANISHED pane
///
/// Audited against tmux 3.7b, and surprising enough to have caused a real
/// bug: when the named pane no longer exists at all, this target does not
/// fail — the empty window component falls back to the session's CURRENT
/// window and its active pane, so the command silently acts on a
/// DIFFERENT terminal of the same session. (`display-message -t
/// '=fh:.%1'` after `%1`'s window was killed reported `%0`, exit 0.)
///
/// So this form answers "never another session's terminal", not "exactly
/// this pane or nothing". Every caller must therefore address a pane it
/// has just resolved and holds still — which is what the session lifecycle
/// claim buys the tab operations — and the two queries whose whole job is
/// to DETECT a vanished or recycled pane ([`TmuxDriver::pane_process`] and
/// [`TmuxDriver::capture_alt_screen_if_active`]) deliberately use the bare
/// pane id instead: it expands empty for a pane that is gone, which is the
/// signal they are built to read, and each cross-checks `#{session_name}`
/// from the same atomic invocation for the scoping this form would have
/// given them.
fn pane_in_session(session: &str, pane: &str) -> String {
    format!("={session}:.{pane}")
}

fn env_assignments(env: &[(String, String)]) -> Vec<String> {
    env.iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect()
}

/// What [`TmuxDriver::plan_pane_relaunch`] arranged before a respawn,
/// and what the caller still owes because of it.
///
/// Split from the respawn itself because the two halves have to
/// straddle a step that belongs to neither: the preamble has to be in
/// the launch spec BEFORE the new process starts, and the geometry has
/// to be restored AFTER it. Returning a plan rather than doing it all
/// inline is what lets the caller sequence spec-write → respawn →
/// restore without this module knowing about launch specs.
///
/// ## Why either half exists
///
/// `respawn-pane` retains a pane's scrollback HISTORY but
/// reinitializes its visible grid (measured on every version audited
/// below), so what the user last saw is precisely what a naive respawn
/// throws away. Two mechanisms recover it, and which one applies is
/// decided here:
///
/// - **The shrink** (`restore`): shrinking a window scrolls the lines
///   it no longer has room for into history, and growing it back pulls
///   them out. Shrinking to one row therefore moves the ENTIRE visible
///   screen into history before the respawn can clear it. Applies to a
///   pane on the primary screen with more than one row.
/// - **The carried-over frame** (`carry_over`): an ALTERNATE-screen
///   grid has no history to scroll into (that is what the alternate
///   screen is), so shrinking preserves nothing; a one-row window has
///   nothing to shrink. In both cases the frame is captured instead and
///   handed back for the new run to re-emit through its launch spec
///   (`launch::LaunchSpec::preamble`) — the same content the stop
///   snapshot already captures for a dead alt-screen pane, put to the
///   one use that survives a respawn.
///
/// Version floor: `respawn-pane -k`, `-e` environment injection,
/// history retention, the shrink-scrolls-into-history rule, and the
/// grow-pulls-back rule were all audited empirically against tmux 3.3a
/// (the crate's floor when this was written), 3.4 (Ubuntu 24.04's
/// package, so CI's), and 3.7b (the development host at the time; today's
/// [`TMUX_FLOOR`] is newer still). All three behave identically; nothing
/// here needs a version gate, and raising the floor past them only
/// narrowed the range this evidence has to cover.
///
/// Every step is BEST EFFORT and never fails the relaunch: a resize or
/// capture that does not land costs scrollback fidelity, while refusing
/// to relaunch over it would cost the user their agent. A shrink whose
/// restore never runs (a crash mid-restart) is not permanent either —
/// every attach resizes the window to its own client size.
pub struct PaneRelaunchPlan {
    /// The geometry to restore after the respawn, when this plan
    /// shrank the window to push its visible screen into history.
    pub restore: Option<(u16, u16)>,
    /// The prior run's last visible frame, for the cases the shrink
    /// cannot cover; the caller emits it through the new launch.
    pub carry_over: Option<Vec<u8>>,
}

impl TmuxDriver {
    /// `state_dir` owns the socket and generated config. The config file
    /// is rewritten whenever the driver starts, while a server already
    /// running on the private socket retains its live option values until
    /// explicitly changed or restarted.
    ///
    /// Uses the production control-mode budgets ([`CONTROL_EXCHANGE_TIMEOUT`],
    /// [`PANE_LIST_TIMEOUT`]); callers that need different ones (integration
    /// tests running on a loaded CI box) go through
    /// [`Self::new_with_timeouts`] instead.
    pub fn new(state_dir: &Path) -> TmuxDriver {
        Self::new_with_timeouts(state_dir, TmuxBudgets::default())
    }

    /// Like [`Self::new`], with the two control-mode budgets supplied
    /// explicitly and the tmux program left at the `PATH` default.
    ///
    /// The budgets-only convenience: it exists for callers that need to
    /// stretch a control-mode deadline (a loaded CI box) but have no
    /// opinion about WHICH tmux runs. `pub(crate)`, not `pub`: the
    /// production constants remain the only values any real supervisor
    /// uses (see [`Self::new`]), so the only legitimate callers are this
    /// crate's own `Supervisor::new_with_seams` (threading
    /// `SupervisorTimeouts` through) and this module's own tests — never
    /// an embedder, and never an e2e test directly (those go through
    /// `SupervisorTimeouts`, not this driver).
    pub(crate) fn new_with_timeouts(state_dir: &Path, budgets: TmuxBudgets) -> TmuxDriver {
        Self::new_with_program(state_dir, budgets, PathBuf::from(DEFAULT_TMUX_PROGRAM))
    }

    /// The one real constructor, and the production route: everything
    /// above it supplies a default for one of its arguments.
    ///
    /// `program` is the resolved tmux binary (see [`resolve_tmux_program`]).
    /// It carries BOTH the control-mode budgets and the program choice,
    /// which is what the supervisor's own startup needs — the single
    /// production call site that has an opinion about the program. It is
    /// a separate constructor rather than a fourth parameter on the
    /// existing ones because nearly every test in this crate wants the
    /// `PATH` default, and churning several dozen call sites to say "no
    /// opinion" would obscure the one that has one. The exceptions are
    /// this module's focused override tests, which name a stand-in binary
    /// here precisely to prove the choice reaches the floor probe and the
    /// command funnel.
    pub(crate) fn new_with_program(
        state_dir: &Path,
        budgets: TmuxBudgets,
        program: PathBuf,
    ) -> TmuxDriver {
        TmuxDriver {
            socket: state_dir.join("tmux.sock"),
            config: state_dir.join("tmux.conf"),
            program,
            exchange_timeout: budgets.exchange,
            pane_list_timeout: budgets.pane_list,
            shutdown_admission: Arc::new(tokio::sync::Semaphore::new(4)),
            #[cfg(test)]
            disable_output_gate: None,
        }
    }

    /// The generated server config. Every line is load-bearing:
    /// - `exit-empty off`: without it `start-server` on a fresh socket
    ///   leaves no server at all (verified: `list-sessions` reports "no
    ///   server running"), and an existing server would exit the moment
    ///   its last session was killed.
    /// - `status off` / no prefix: tmux UI must never appear (SPEC_impl).
    /// - `history-limit`: the SPEC.md replay floor. New servers derive it
    ///   from the same `HISTORY_LIMIT` that replay capture requests. An
    ///   adopted server can retain an older live value, which is why
    ///   changing this constant needs an explicit option-migration path.
    /// - `remain-on-exit on`: dead panes stay viewable (SPEC.md).
    /// - `default-terminal xterm-256color`: what xterm.js actually is;
    ///   inner apps probe $TERM.
    /// - `escape-time 0`: tmux waits after a lone ESC byte to see whether
    ///   an escape sequence follows. The default is 500ms before tmux 3.5
    ///   and 10ms from 3.5 on — half a second of visibly laggy Esc
    ///   handling in agent TUIs and vim on the older builds, 10ms of it
    ///   on everything at today's [`TMUX_FLOOR`]; 0 removes it entirely.
    ///   Kept rather than dropped with the floor's rise because 10ms of
    ///   latency on every Esc is still latency Farhelm has no reason to
    ///   pay. Like every line here it reaches FRESH servers only — tmux
    ///   reads a `-f` config when it starts a server, so an adopted one
    ///   keeps whatever `escape-time` it was started with, and this
    ///   driver does not reconcile it after the fact (`focus-events`,
    ///   below, is the one option that gets that treatment).
    ///
    /// NOT set here: `focus-events`. It is a server option that must be
    /// reconciled on EVERY `ensure_server` call, not just a fresh start —
    /// see `ensure_server`'s own doc for why a config-file line alone
    /// cannot do that job, and for what the option actually buys us.
    ///
    /// NOT set here: `window-size manual`. It crashes the tmux 3.4 server
    /// outright — `new-session` then returns "server exited unexpectedly",
    /// in any spelling (`set -g`, `setw -g`, `set -w -g`) — and 3.4 is
    /// what Ubuntu 24.04 ships, so it took out every session on CI while
    /// passing locally on 3.7. It is unnecessary anyway: the supervisor's
    /// control-mode client never declares a size (no `refresh-client -C`),
    /// so tmux ignores it for sizing, and `resize-window` sets
    /// `window-size manual` on the window it touches. Both verified
    /// against 3.4 directly.
    ///
    /// Option tables are named explicitly — `set -s` for server options,
    /// `set -g` for session, `setw -g` for window — rather than relying
    /// on tmux inferring the table. Inference works on recent versions
    /// and explicit tables work on every version back to 3.3, so being
    /// explicit costs nothing and is kept even though [`TMUX_FLOOR`] now
    /// excludes the versions that needed it: a config line tmux cannot
    /// place is a config line that silently does nothing, and that is not
    /// a failure mode worth re-earning on the next floor change.
    fn config_body() -> String {
        format!(
            "set -s exit-empty off\n\
             set -s escape-time 0\n\
             set -s default-terminal 'xterm-256color'\n\
             set -g status off\n\
             set -g prefix None\n\
             set -g history-limit {HISTORY_LIMIT}\n\
             setw -g remain-on-exit on\n"
        )
    }

    /// A tmux `Command` already pointed at the private server.
    ///
    /// EVERY tmux invocation in this module is built here, which is what
    /// makes the `-S`/`-f` isolation unforgettable: touching the user's
    /// own tmux server and config would be a serious violation, and a
    /// forgotten flag is exactly how that happens. The same funnel is what
    /// makes the `--tmux` / `FARHELM_TMUX` override total — see
    /// [`TmuxDriver::program`] — with NO exceptions, `ensure_server`'s
    /// `-V` floor probe included. That probe runs before any server
    /// exists, but the extra `-S`/`-f` arguments cost it nothing: tmux
    /// answers `-V` from the client binary and exits without contacting a
    /// server or loading the config file (verified against 3.7c with both
    /// paths pointing at a directory that does not exist). Routing it
    /// here anyway is what keeps "every invocation runs the chosen
    /// program" a property of the code rather than a claim in a comment.
    fn command(&self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.arg("-S").arg(&self.socket).arg("-f").arg(&self.config);
        cmd
    }

    /// Run one tmux command against the private server and return its
    /// stdout, turning a non-zero exit into an error carrying tmux's own
    /// stderr — tmux explains its refusals in prose ("can't find session",
    /// "command too long") and that text is the whole diagnostic.
    async fn run(&self, args: &[&str]) -> anyhow::Result<String> {
        let out = self.run_bytes(args).await?;
        Ok(String::from_utf8_lossy(&out).into_owned())
    }

    /// Kill every control-mode client currently attached to the private
    /// server, returning how many were signaled.
    ///
    /// Called exactly once, from supervisor startup, right after
    /// [`Self::ensure_server`] — the one moment when "attached control
    /// client" and "stale" coincide by construction: this process owns
    /// no clients yet, the state dir is exclusively claimed, and the
    /// private server serves nobody else, so anything still attached
    /// belongs to a dead predecessor.
    ///
    /// Why a predecessor's clients can need reaping at all: teardown by
    /// protocol — stdin EOF when the dead owner's pipe ends close — is
    /// the primary mechanism and usually works, but tmux defers a
    /// control client's exit until its pending output drains
    /// (`server_client_check_exit` requires `control_all_done`, which
    /// requires an empty output buffer; same shape from tmux 3.4 through
    /// 3.7b), and nothing ever frees that buffer once the write side is
    /// dead. A client whose owner was SIGKILLed with output still queued
    /// therefore stays attached FOREVER: the EOF is seen, `CLIENT_EXIT`
    /// is set, and the exit never completes. Observed live on 2026-08-18
    /// (tmux 3.6): a killed supervisor's session sink outlived a
    /// 120-second deadline with EOF deliverable and a /proc scan proving
    /// no write end of its stdin remained anywhere.
    ///
    /// Each stale client is first put behind the module's acknowledged
    /// `no-output` boundary ([`Self::disable_control_client_output`]) and
    /// only then has its PROCESS killed. Both halves are load-bearing.
    /// The boundary first, because the stale condition specifically means
    /// the client has queued output, and closing or killing an
    /// output-bearing client is the exact shape that aborts tmux 3.7b's
    /// whole server (`fatal: not enough data` — the crash the pinned
    /// 3.7b regression suite exists for); the acknowledged flag discards
    /// every queued block server-side before anything is torn down. The
    /// process kill second, because it is the one lever that works on a
    /// client wedged in the deferred-exit trap: its death closes the
    /// client's server socket, and socket loss tears the client down
    /// regardless of buffer state, where `detach-client` would only set
    /// the same `CLIENT_EXIT` flag that is already stuck behind the
    /// drain.
    ///
    /// The acknowledged boundary doubles as the liveness check that
    /// makes the kill safe to aim: the ack proves tmux still knew
    /// `client-<pid>` an instant before the signal, shrinking the
    /// pid-reuse window to the microseconds between ack and kill. (A
    /// pidfd would close even that window but is Linux-only; this path
    /// must also run on macOS, and the residual window requires a fresh
    /// process to claim the exact freed pid within it.) A client the ack
    /// CANNOT reach is skipped, not killed — no acknowledgement, no
    /// standing to signal — and the roster verification below decides
    /// whether that skip mattered.
    ///
    /// Fails closed: if any control-mode client is still attached when
    /// the verification deadline expires, this returns an error and
    /// startup fails, because proceeding would bring up a fresh sink
    /// alongside the stale one — the precise corruption this sweep
    /// exists to prevent. The verification also insists the clean roster
    /// HOLDS for a beat, so a predecessor's client that was still mid-
    /// attach during the first listing does not slip past the sweep.
    ///
    /// Plain (non-control) clients are deliberately left alone: a
    /// human's `tmux attach` against the private socket is unsupported,
    /// but killing their terminal out from under them would be a hostile
    /// way to say so. The inverse carve-out is documented rather than
    /// defended: a control-mode client somebody's agent process opened
    /// against the private socket is indistinguishable from a dead
    /// supervisor's leftovers and WILL be reaped at the next supervisor
    /// start — private-server control clients belong to the supervisor,
    /// full stop.
    pub async fn reap_stale_control_clients(&self) -> anyhow::Result<usize> {
        let mut reaped = 0usize;
        let mut skipped: Vec<String> = Vec::new();
        let Some(listed) = self.list_client_pids_and_flags().await? else {
            return Ok(0);
        };
        for (pid, flags) in &listed {
            if !flags.contains("control-mode") {
                continue;
            }
            // The acknowledged no-output boundary; see the docstring for
            // why it must precede the kill and how it doubles as the
            // aim-check.
            if let Err(error) = self
                .disable_control_client_output(&format!("client-{pid}"))
                .await
            {
                skipped.push(format!(
                    "client-{pid}: no-output not acknowledged: {error:#}"
                ));
                continue;
            }
            // SAFETY: `libc::kill` validates the pid itself. ESRCH means
            // the client exited between the ack and the signal — the
            // outcome the reap wanted anyway; any other errno is a real
            // failure the verification below must weigh.
            let ret = unsafe { libc::kill(*pid as libc::pid_t, libc::SIGKILL) };
            if ret == 0 {
                reaped += 1;
            } else {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::ESRCH) {
                    reaped += 1;
                } else {
                    skipped.push(format!("client-{pid}: kill failed: {err}"));
                }
            }
        }
        let had_candidates = reaped > 0 || !skipped.is_empty();
        if had_candidates {
            // Bounded verification, run whenever candidates existed —
            // including when every signal failed, since that is exactly
            // when the roster most needs checking. The clean state must
            // hold across one extra beat so a client that was still
            // mid-attach during the first listing gets seen.
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
            let mut clean_once = false;
            loop {
                let control_remaining = match self.list_client_pids_and_flags().await? {
                    None => 0,
                    Some(listing) => listing
                        .iter()
                        .filter(|(_, flags)| flags.contains("control-mode"))
                        .count(),
                };
                if control_remaining == 0 {
                    if clean_once {
                        break;
                    }
                    clean_once = true;
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    continue;
                }
                clean_once = false;
                if tokio::time::Instant::now() >= deadline {
                    bail!(
                        "stale control clients survived the startup reap ({control_remaining} \
                         still attached; skipped: [{}]); refusing to start beside them",
                        skipped.join("; ")
                    );
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
        Ok(reaped)
    }

    /// One bounded `list-clients` snapshot as `(pid, flags)` rows, with
    /// the clientless-server error shape mapped to `None`.
    ///
    /// `list-clients` on a clientless server is an ERROR ("no current
    /// target"), not an empty listing — the ordinary fresh-start case
    /// must read as "nothing attached", while every other failure still
    /// propagates. Bounded with its own kill-on-drop process rather than
    /// [`Self::run`] because the reap runs during startup, where a
    /// wedged tmux server must fail construction instead of hanging it.
    async fn list_client_pids_and_flags(&self) -> anyhow::Result<Option<Vec<(u32, String)>>> {
        let mut query = self.command();
        query
            .args(["list-clients", "-F", "#{client_pid} [#{client_flags}]"])
            .stdin(Stdio::null())
            .kill_on_drop(true);
        let out = tokio::time::timeout(CONTROL_EXCHANGE_TIMEOUT, query.output())
            .await
            .context("list-clients timed out during the stale-control-client reap")?
            .context("running list-clients for the stale-control-client reap")?;
        if !out.status.success() {
            let error = anyhow::Error::new(TmuxCommandFailure { stderr: out.stderr });
            if self.is_definitively_empty(&error) {
                return Ok(None);
            }
            return Err(error.context("listing clients for the stale-control-client reap"));
        }
        let mut rows = Vec::new();
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let Some((pid, flags)) = line.split_once(' ') else {
                continue;
            };
            if let Ok(pid) = pid.parse::<u32>() {
                rows.push((pid, flags.to_string()));
            }
        }
        Ok(Some(rows))
    }

    /// Stop pane delivery to one live control client through a separate command.
    ///
    /// This deliberately does NOT use the output client's stdin or stdout.
    /// The stream being torn down may have been cancelled halfway through a
    /// line, a command write, or a positional reply group; reusing that stream
    /// would make the final acknowledgement indistinguishable from an older
    /// reply. A separate tmux process addresses the control client by the name
    /// tmux assigns from its OS pid (`client-<pid>`). Successful process exit
    /// proves the server applied `no-output`, which discards every queued pane
    /// block before the client itself is closed or killed.
    async fn disable_control_client_output(&self, target: &str) -> anyhow::Result<()> {
        let _admission = self
            .shutdown_admission
            .acquire()
            .await
            .context("the control-client shutdown admission limit closed")?;
        #[cfg(test)]
        let gate = self.disable_output_gate.as_ref().map(Arc::clone);
        #[cfg(test)]
        if let Some(gate) = &gate {
            gate.entered.notify_one();
            gate.release.notified().await;
        }

        let mut command = self.command();
        command
            .args(["refresh-client", "-t", target, "-f", "no-output"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            // Cancelling the deadline must not leave a diagnostic tmux
            // process behind. This process is not the output-bearing client;
            // killing it cannot invalidate that client's pane queues.
            .kill_on_drop(true);
        let output = tokio::time::timeout(CONTROL_CLIENT_SHUTDOWN_TIMEOUT, command.output())
            .await
            .context("timed out disabling a control client's output")?
            .context("spawning tmux to disable a control client's output")?;
        if !output.status.success() {
            bail!(
                "tmux could not disable control client {target}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        #[cfg(test)]
        if let Some(gate) = &gate {
            gate.acknowledged.notify_one();
            gate.finish.notified().await;
        }
        Ok(())
    }

    /// Like [`TmuxDriver::run`], but returning stdout as raw bytes.
    ///
    /// Avoids adding another lossy UTF-8 conversion to `capture-pane`
    /// output. Tmux may already canonicalize invalid source bytes while
    /// storing its terminal grid, but valid multibyte and non-ASCII
    /// content should reach replay unchanged. The live output path
    /// (`%output`/`%extended-output`) is byte-clean and bypasses the grid
    /// (see `OutputStream::next_output`).
    async fn run_bytes(&self, args: &[&str]) -> anyhow::Result<Vec<u8>> {
        // `stdin` is null so tmux can never inherit the supervisor's own
        // stdin — under `farhelm internal stdio` that stdin is the
        // protocol stream.
        let out = self
            .command()
            .args(args)
            .stdin(Stdio::null())
            .output()
            .await
            .context("spawning tmux")?;
        if !out.status.success() {
            // The human-readable message is layered on TOP of
            // `TmuxCommandFailure` via `.context(...)` rather than built by
            // `bail!` directly, so that struct — carrying tmux's raw,
            // unmodified stderr — survives as the root cause and stays
            // reachable via `downcast_ref` at any depth (see its own docs).
            // `{}`'s rendering of the returned error is unaffected: anyhow
            // displays only the outermost context by default, which is
            // exactly this formatted string, so every existing caller that
            // pattern-matches `e.to_string()` sees the same text as before.
            // Context formatted BEFORE the move, so the raw stderr can be
            // handed to `TmuxCommandFailure` without cloning the buffer.
            let context = format!(
                "tmux {:?} failed ({}): {}",
                args,
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            );
            return Err(anyhow::Error::new(TmuxCommandFailure {
                stderr: out.stderr,
            }))
            .context(context);
        }
        Ok(out.stdout)
    }

    /// Start (or adopt) the private server, floor-check whichever one we
    /// ended up with, then reconcile the one live option this driver
    /// actively manages, regardless of which path was taken.
    ///
    /// The running substrate itself — sessions, panes, scrollback history
    /// — is left exactly as it is on adoption, per the discovery-first
    /// rule: never restart a running substrate. `focus-events` is the one
    /// deliberate exception, normalized on every call rather than only on
    /// a fresh start, because a server started before this option existed
    /// (or by an older farhelm build) would otherwise keep whatever stale
    /// value it already had for the rest of its life — see the
    /// `set-option` call below for why a config-file line cannot do this
    /// job alone, and for what the option actually buys us.
    ///
    /// TWO floor checks, not one, and both are load-bearing: see
    /// [`Self::require_supported_server`] for why upgrading the client
    /// binary does not upgrade the server it adopts.
    pub async fn ensure_server(&self) -> anyhow::Result<()> {
        self.ensure_server_with_seam(crate::files::RealFs).await
    }

    /// [`Self::ensure_server`]'s real body, parameterized over the write-
    /// atomicity seam (items 7/8: production call sites must be
    /// injectable through their REAL path, not just through a synthetic
    /// call into `crate::files`). Production always calls
    /// [`Self::ensure_server`], which supplies [`crate::files::RealFs`].
    ///
    /// `seam` is a VALUE, `Copy + Send + 'static`, rather than a `&dyn`
    /// reference: it must cross into a `spawn_blocking` closure (the
    /// config write is blocking I/O that must not run on an async worker
    /// thread — `tokio::task::block_in_place` was considered instead, but
    /// it panics under this crate's current-thread test runtimes, which
    /// construct a `TmuxDriver` in nearly every integration-style test).
    pub async fn ensure_server_with_seam<S>(&self, seam: S) -> anyhow::Result<()>
    where
        S: crate::files::FaultSeam + Copy + Send + 'static,
    {
        // Resolved for the message only, never for the spawn: `Command`
        // does its own PATH lookup below, and pre-resolving would add a
        // time-of-check gap for nothing. See `program_display_path`.
        let named = program_display_path(&self.program, std::env::var_os("PATH").as_deref());
        // Through the ordinary funnel (see `command`): tmux answers `-V`
        // without touching the socket or the config, so the private
        // arguments are inert here and routing around them would only
        // create a call site the override could be forgotten at.
        let version = match self.command().arg("-V").stdin(Stdio::null()).output().await {
            Ok(output) => output,
            // `ENOENT`: nothing answered to `self.program` at all, which is
            // a materially different failure from "something ran and this
            // process refused to talk to it" — see `tmux_not_found_message`
            // for why the two spellings below matter to whoever reads them.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                bail!("{}", tmux_not_found_message(&self.program));
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("checking the tmux version of {}", named.display()));
            }
        };
        if !version.status.success() {
            bail!(
                "{} -V failed ({}): {}",
                named.display(),
                version.status,
                String::from_utf8_lossy(&version.stderr).trim()
            );
        }
        require_supported_tmux(&named, &String::from_utf8_lossy(&version.stdout))?;
        if let Some(dir) = self.socket.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        // Best-effort-atomic tier (`crate::files` module docs): this
        // config is rebuilt from `config_body()` on every call and
        // carries no state of its own, so a LOST update is harmless — the
        // next `ensure_server` regenerates it regardless — but a TORN one
        // could still wedge THIS very call (tmux reading a truncated `-f`
        // file on the fresh-start path), which is exactly the failure
        // mode this tier's rename-based publish (plus, since item 19, a
        // file fsync before that rename) rules out.
        let path = self.config.clone();
        let body = Self::config_body();
        tokio::task::spawn_blocking(move || {
            crate::files::overwrite_private_file_sync(&path, body.as_bytes(), &seam)
        })
        .await
        .map_err(|join_err| anyhow::anyhow!("tmux config write task panicked: {join_err}"))??;
        self.run(&["start-server"]).await?;
        // Only now is the substrate's version actually known: the call
        // above either started a server from the binary just checked, or
        // silently adopted one that was already there.
        self.require_supported_server().await?;

        // `focus-events` is deliberately NOT part of `config_body`: tmux
        // only reads a `-f` config when it STARTS a server, so a config
        // line would take effect on a fresh server but be silently
        // skipped when this call instead ADOPTS one already running (the
        // ordinary case across a supervisor restart or upgrade) — exactly
        // the gap this explicit, unconditional `set-option` closes,
        // cheaply (one more tmux round trip at startup) and idempotently
        // (setting an already-on option to on is a no-op) on both paths
        // alike, rather than trying to detect "did we just start this
        // server or adopt it" first.
        //
        // Why it matters at all: Claude Code inspects this option and
        // nags in-session when it reads off ("tmux focus-events off · add
        // 'set -g focus-events on' to ~/.tmux.conf and reattach"); turning
        // it on silences that nag and keeps the option truthful for any
        // app that queries it. That is ALL it does for us, verified
        // empirically (a scratch tmux plus a pane app enabling DEC
        // private mode 1004, probed with a real attached client): the
        // option only gates whether tmux relays a focus escape it
        // receives from a NORMAL ATTACHED CLIENT's pty down to a pane
        // that asked for it. Neither of our two byte paths is such a
        // client — input goes in as `send-keys -H` (see [`InputClient`]),
        // handing the pane literal bytes directly, never through the
        // client-side escape parsing this option gates; output comes out
        // through a non-rendering control-mode client (`tmux -C`, see
        // [`OutputStream`]), which has no pty and originates no
        // client-side escapes for this option to gate in the first place.
        // So this call does not, and cannot, deliver real focus-in/out
        // awareness through this system — it only makes the option's
        // advertised state match what a well-behaved agent expects to see.
        self.run(&["set-option", "-s", "focus-events", "on"])
            .await?;
        Ok(())
    }

    /// Apply the floor to the SERVER now on the private socket, which is
    /// not necessarily the version the `-V` probe cleared.
    ///
    /// Farhelm drives a long-lived tmux server through short-lived client
    /// processes, and it keeps that server across supervisor restarts on
    /// purpose so sessions survive an upgrade. `start-server` ADOPTS a
    /// server already owning the socket rather than replacing it. So
    /// installing a 3.7c client over a running 3.4 or 3.7b server clears
    /// the client-side check and then drives every subsequent command
    /// against the old, crash-prone substrate — precisely the situation
    /// [`TMUX_FLOOR`] exists to prevent, arrived at through the upgrade
    /// path most likely to be taken.
    ///
    /// The version comes from tmux's `#{version}` format, which reports
    /// the SERVER's version to the connecting client (verified: a 3.7c
    /// client against a 3.7b server prints `3.7b`) and, unlike `-V`,
    /// prints the bare number with no `tmux ` prefix.
    ///
    /// REFUSES WITHOUT KILLING. A below-floor server is hosting the
    /// user's live sessions; tearing it down to satisfy a version policy
    /// would destroy exactly what the never-restart-a-running-substrate
    /// rule protects. Draining it is a decision only its owner can make,
    /// so the refusal says so and names what to act on.
    async fn require_supported_server(&self) -> anyhow::Result<()> {
        let reported = self
            .run(&["display-message", "-p", "#{version}"])
            .await
            .context("asking the tmux server on the private socket for its version")?;
        let reported = reported.trim_end_matches('\n');
        let found = parse_tmux_version_number(reported).with_context(|| {
            format!(
                "the tmux server on {} reported an unrecognized version {reported:?}",
                self.socket.display()
            )
        })?;
        match classify_tmux_version(found) {
            TmuxSupport::BelowFloor => bail!(
                "the tmux server already running on {} is {found}, below Farhelm's floor \
                 {TMUX_FLOOR} (see README: tmux). Upgrading the tmux binary does not upgrade a \
                 server that is already running: it keeps serving its existing sessions until it \
                 exits. Farhelm will not kill it for you. Drain it deliberately — stop the \
                 sessions you still need elsewhere, then `tmux -S {} kill-server` once nothing on \
                 it matters — before running a supervisor on this state directory.",
                self.socket.display(),
                self.socket.display()
            ),
            TmuxSupport::AtFloor => {}
            TmuxSupport::AbovePin => warn!(
                socket = %self.socket.display(),
                found = %found,
                tested = %TMUX_FLOOR,
                "the tmux server on the private socket is newer than the version Farhelm is \
                 tested against; this combination is unaudited"
            ),
        }
        Ok(())
    }

    /// Whether a tmux session by this name still exists on the private
    /// server.
    ///
    /// Used only at startup (`Supervisor::reload_sessions`'s persisted-
    /// session load) to decide whether a session row read back from
    /// SQLite describes a still-live terminal or one that ended along
    /// with a prior tmux server.
    ///
    /// A nonzero exit alone does NOT answer the question: tmux uses the
    /// same "command failed" exit status for "no such session" as it does
    /// for "no server running at all" and other transient failures, and
    /// only the former is this probe's honest `false`. Conflating the two
    /// would permanently demote a live session to terminal-less on
    /// nothing more than a hiccup reaching the private tmux server —
    /// exactly the "do not guess" failure this module's callers are
    /// supposed to avoid. So this inspects tmux's own diagnostic text
    /// instead of trusting the exit code alone, and only two exact
    /// messages (verified empirically against a scratch server) count as
    /// the honest `false`:
    /// - `"can't find session"`, tmux's answer when the server has at
    ///   least one OTHER session but not this one;
    /// - `"no current target"`, tmux's answer when the server has NO
    ///   sessions at all (empirically, `has-session -t =name` degrades to
    ///   this generic message rather than naming the target it could not
    ///   find once there is nothing to compare it against) — the exact
    ///   shape of a freshly restarted supervisor's tmux server before any
    ///   session outlives the restart, so this case matters in practice,
    ///   not just in principle.
    ///
    /// Anything else propagates as an error, stderr included, for the
    /// caller to surface rather than silently misreport.
    ///
    /// The `=` prefix forces exact-name matching: bare `-t` targets fall
    /// back to prefix and fnmatch resolution, which for a liveness probe
    /// would mean a differently named session could answer for a dead
    /// one. Today's fixed-length `fh-<8 hex>` names cannot shadow each
    /// other, but the probe should not depend on that naming detail.
    pub async fn has_session(&self, name: &str) -> anyhow::Result<bool> {
        match self.run(&["has-session", "-t", &format!("={name}")]).await {
            Ok(_) => Ok(true),
            Err(e)
                if e.to_string().contains("can't find session")
                    || e.to_string().contains("no current target") =>
            {
                Ok(false)
            }
            Err(e) => Err(e).context("checking tmux session liveness"),
        }
    }

    /// Kill a tmux session by name, tolerating its absence.
    ///
    /// Four callers, all of them tearing something down: a create
    /// unwinding a window whose SQLite insert failed (`create_session`'s
    /// failure-ordering contract), `DeleteSession`, archive, and a RESTART
    /// clearing the husk of a tmux session whose pane it can no longer find
    /// before building a fresh terminal under the same name. The
    /// already-gone case this tolerates is NOT the agent
    /// exiting on its own — `remain-on-exit on` keeps a pane (and so its
    /// session) around after the process inside it dies, so that never
    /// races this call — but something outside the supervisor's own
    /// bookkeeping racing it against the same session: a concurrent
    /// `kill-server` on the private socket, or manual cleanup someone runs
    /// directly against it.
    ///
    /// Three tmux diagnostics (verified empirically) all mean the desired
    /// state — this name is not a live session — was already reached, and
    /// so are all tolerated the same way: `"can't find session"` (another
    /// session exists but not this one), `"no current target"` (the server
    /// is up but has no sessions at all), and `"no server running"` (the
    /// whole private tmux server is gone). This is DELIBERATELY wider than
    /// `has_session`'s own tolerance list: `has_session` propagates `"no
    /// server running"` as a real error (see its own docs) because for
    /// its one caller — deciding whether a PERSISTED row's tmux survived a
    /// restart — a vanished server is worth surfacing rather than
    /// silently guessing "not there". This function's caller, by
    /// contrast, is unwinding a session it JUST tried to create (or
    /// tearing one down that is being deleted anyway), where "the whole
    /// server is gone" and "this one session is gone" both mean the exact
    /// same thing: there is nothing left for `kill_session` to do.
    /// Anything else is a real failure worth surfacing.
    pub async fn kill_session(&self, name: &str) -> anyhow::Result<()> {
        match self.run(&["kill-session", "-t", name]).await {
            Ok(_) => Ok(()),
            Err(e)
                if [
                    "can't find session",
                    "no current target",
                    "no server running",
                ]
                .iter()
                .any(|diagnostic| e.to_string().contains(diagnostic)) =>
            {
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Create a session running `window_cmd` (argv, executed directly by
    /// tmux — no extra shell layer beyond the one in the argv itself).
    /// Returns the pane id (`%N`), the stable handle for all later
    /// operations on this terminal.
    ///
    /// Dimensions are clamped exactly like [`TmuxDriver::resize_window`]:
    /// `new-session -x 0` is a hard tmux error ("width too small"), and a
    /// caller that has no real terminal yet (a browser mid-layout, a
    /// script) must get a session, not a confusing tmux refusal.
    ///
    /// `env` is injected into the new session's environment with `-e`, one
    /// flag per entry; production passes none (see
    /// `service::SupervisorSeams::launch_env` for the one caller that does
    /// not, and why the launch environment needs an injection point at
    /// all). Values are passed as literal argv elements to tmux, never
    /// through a shell, so no quoting applies.
    pub async fn create_session(
        &self,
        name: &str,
        cwd: &str,
        cols: u16,
        rows: u16,
        env: &[(String, String)],
        window_cmd: &[String],
    ) -> anyhow::Result<String> {
        let cols_s = cols.clamp(1, 10_000).to_string();
        let rows_s = rows.clamp(1, 10_000).to_string();
        let env_args = env_assignments(env);
        // `-P -F` prints the pane id from the same invocation that
        // creates the session. One call, not new-session followed by a
        // display-message query: if the follow-up query failed, the
        // session (and the agent already launching in it) would exist
        // untracked — a live process with no owner, violating the
        // "failed create leaves nothing behind" contract.
        let mut args: Vec<&str> = vec![
            "new-session",
            "-d",
            "-P",
            "-F",
            "#{pane_id}",
            "-s",
            name,
            "-x",
            &cols_s,
            "-y",
            &rows_s,
            "-c",
            cwd,
        ];
        for assignment in &env_args {
            args.push("-e");
            args.push(assignment);
        }
        // `--` before the command: without it, an argv whose first element
        // began with a dash would be parsed as more flags. Nothing today
        // launches such a command (the window command always starts with a
        // shell path), and that is exactly the kind of thing a caller
        // should not have to keep true.
        args.push("--");
        args.extend(window_cmd.iter().map(String::as_str));
        let pane = self.run(&args).await?;
        Ok(pane.trim().to_string())
    }

    /// Prepare `pane` to be respawned so the prior run's last screen
    /// survives it; see [`PaneRelaunchPlan`] for what the two halves do and
    /// why the work is split around the caller's own launch publication.
    ///
    /// `max_carry_over` bounds the captured frame exactly like the stop
    /// snapshot bounds its own (see
    /// [`TmuxDriver::capture_alt_screen_if_active`]): the bytes travel
    /// through a launch spec and then through the pane, and an unbounded
    /// capture of a 10,000-row pane is not something either should have to
    /// carry.
    pub async fn plan_pane_relaunch(
        &self,
        session: &str,
        pane: &str,
        max_carry_over: usize,
    ) -> PaneRelaunchPlan {
        let geometry = match self
            .run(&[
                "display-message",
                "-p",
                "-t",
                &pane_in_session(session, pane),
                "#{window_width} #{window_height} #{alternate_on}",
            ])
            .await
        {
            Ok(out) => out,
            Err(e) => {
                warn!(
                    session, error = %e,
                    "could not inspect this window before a relaunch; relaunching without \
                     carrying the prior run's visible screen over"
                );
                return PaneRelaunchPlan {
                    restore: None,
                    carry_over: None,
                };
            }
        };
        let mut fields = geometry.split_whitespace();
        let parsed = fields
            .next()
            .and_then(|w| w.parse::<u16>().ok())
            .zip(fields.next().and_then(|h| h.parse::<u16>().ok()));
        let alternate = fields.next() == Some("1");
        let Some((cols, rows)) = parsed else {
            warn!(
                session,
                geometry = %geometry.trim(),
                "tmux reported an unparseable window size before a relaunch; relaunching \
                 without carrying the prior run's visible screen over"
            );
            return PaneRelaunchPlan {
                restore: None,
                carry_over: None,
            };
        };
        // The two cases the shrink cannot serve (see `PaneRelaunchPlan`):
        // an alternate-screen grid, which has no history to scroll into,
        // and a window with nothing to scroll.
        if alternate || rows <= 1 {
            let carry_over = match self
                .capture_alt_screen_if_active(session, pane, max_carry_over)
                .await
            {
                Ok(AltScreenCapture::Captured(bytes)) => Some(bytes),
                // A one-row PRIMARY screen lands here (nothing to capture
                // that is worth a second capture path), as does an
                // oversized or mismatched capture. Losing the frame costs
                // the same as the respawn's own grid reset would.
                Ok(_) => None,
                Err(e) => {
                    warn!(
                        session, error = %e,
                        "could not capture this pane's last frame before a relaunch; the new \
                         run starts without it above"
                    );
                    None
                }
            };
            return PaneRelaunchPlan {
                restore: None,
                carry_over,
            };
        }
        if let Err(e) = self.resize_window(session, pane, cols, 1).await {
            warn!(
                session, error = %e,
                "could not shrink this window before a relaunch; the prior run's visible \
                 screen may be cleared by the respawn"
            );
            return PaneRelaunchPlan {
                restore: None,
                carry_over: None,
            };
        }
        PaneRelaunchPlan {
            restore: Some((cols, rows)),
            carry_over: None,
        }
    }

    /// Run `window_cmd` in an EXISTING pane (PLAN_M3.md item 9's terminal
    /// reuse), replacing whatever was there.
    ///
    /// `respawn-pane -k` is the only tmux mechanism that runs a new process
    /// in the SAME pane: the pane id — the handle every attachment, replay,
    /// and status probe in this crate holds — survives, and `remain-on-exit`
    /// keeps applying (verified: a respawned process that exits leaves the
    /// pane dead with a readable `#{pane_dead_status}`). The alternatives
    /// were rejected for the same reason as each other: a `new-window` or
    /// `split-window` puts the new run in a DIFFERENT pane, so the prior
    /// run's output ends up somewhere the session's terminal view never
    /// shows — which is not "in scrollback" in any sense a user would
    /// recognize.
    ///
    /// The target binds the SESSION and the pane together
    /// (`pane_in_session`), which is not belt-and-braces: pane ids are
    /// assigned by a server-wide counter that restarts at `%0` whenever the
    /// tmux server does, so a bare `%N` carried across a server restart can
    /// name a pane belonging to an entirely different session — and this
    /// command REPLACES the process in whatever it names. tmux refuses the
    /// mismatched pairing itself ("can't find pane"), which makes the check
    /// atomic with the act rather than a probe that could go stale between
    /// the two (audited on 3.3a, 3.4 and 3.7b).
    pub async fn relaunch_in_pane(
        &self,
        session: &str,
        pane: &str,
        cwd: &str,
        env: &[(String, String)],
        window_cmd: &[String],
    ) -> anyhow::Result<()> {
        let target = pane_in_session(session, pane);
        let env_args = env_assignments(env);
        let mut args: Vec<&str> = vec!["respawn-pane", "-k", "-t", &target, "-c", cwd];
        for assignment in &env_args {
            args.push("-e");
            args.push(assignment);
        }
        args.push("--");
        args.extend(window_cmd.iter().map(String::as_str));
        self.run(&args)
            .await
            .map(|_| ())
            .context("respawning the session's pane for a relaunch")
    }

    /// Add a window to an existing session and run `window_cmd` in it —
    /// the substrate for a terminal tab (PLAN_M4.md item 2). Returns the
    /// new window's id and its pane id.
    ///
    /// `-d` is load-bearing rather than tidy: without it the new window
    /// becomes the session's CURRENT window, and every tmux command that
    /// resolves a bare session target (anything a future caller writes
    /// against `-t <session>` instead of a pane) would silently start
    /// addressing whichever tab was opened last. Opening a tab must change
    /// nothing about any other terminal.
    ///
    /// Both ids come back from the SAME invocation that creates the
    /// window, for `create_session`'s reason: a follow-up query that
    /// failed would leave a window (and the shell already starting in it)
    /// that nothing owns.
    ///
    /// `env`, `--`, and the dimension-free signature all match
    /// `create_session`'s contract — values are literal argv elements, so
    /// no quoting applies, and a new window inherits the session's
    /// geometry, which the attach that follows resizes to the client's own
    /// anyway.
    pub async fn new_window(
        &self,
        session: &str,
        cwd: &str,
        env: &[(String, String)],
        window_cmd: &[String],
    ) -> anyhow::Result<(String, String)> {
        let target = format!("={session}:");
        let env_args = env_assignments(env);
        let mut args: Vec<&str> = vec![
            "new-window",
            "-d",
            "-P",
            "-F",
            "#{window_id} #{pane_id}",
            "-t",
            &target,
            "-c",
            cwd,
        ];
        for assignment in &env_args {
            args.push("-e");
            args.push(assignment);
        }
        args.push("--");
        args.extend(window_cmd.iter().map(String::as_str));
        let out = self.run(&args).await?;
        let mut fields = out.split_whitespace();
        let window = fields
            .next()
            .with_context(|| format!("new-window returned no window id: {out:?}"))?;
        let pane = fields
            .next()
            .with_context(|| format!("new-window returned no pane id: {out:?}"))?;
        Ok((window.to_string(), pane.to_string()))
    }

    /// Set a window user option on the window CONTAINING `pane` — how a
    /// window is marked as farhelm's (see [`TAB_WINDOW_OPTION`] and
    /// [`AGENT_WINDOW_OPTION`]).
    ///
    /// Addressed through the pane rather than a window id so the session
    /// pairing actually binds; see [`pane_in_session`] for the audit that
    /// makes that necessary rather than stylistic.
    pub async fn mark_window(
        &self,
        session: &str,
        pane: &str,
        option: &str,
        value: &str,
    ) -> anyhow::Result<()> {
        self.run(&[
            "set-option",
            "-w",
            "-t",
            &pane_in_session(session, pane),
            option,
            value,
        ])
        .await
        .map(|_| ())
        .with_context(|| format!("marking a window with {option}"))
    }

    /// Kill the window containing `pane`, tolerating its absence.
    ///
    /// The tolerated diagnostics are the same ones [`Self::kill_session`]
    /// accepts, plus tmux's own "can't find pane" — the shape a window
    /// already gone leaves behind, since the target is expressed as a pane
    /// within a session. All of them mean the desired state (this window
    /// no longer exists) has already been reached, which is what makes
    /// `CloseTab` safe to retry after a partial failure.
    ///
    /// This is deliberately NOT the whole of closing a tab: it must run
    /// only AFTER the tab's process tree has been reaped, because killing
    /// the window first would destroy the live pane the descendant walk
    /// anchors on and leave the weaker marker scan as the only mechanism
    /// (PLAN_M4.md item 2).
    pub async fn kill_window(&self, session: &str, pane: &str) -> anyhow::Result<()> {
        match self
            .run(&["kill-window", "-t", &pane_in_session(session, pane)])
            .await
        {
            Ok(_) => Ok(()),
            // Classified against tmux's OWN raw stderr, anchored at its
            // start, rather than by searching the rendered error — the
            // rendered form embeds this driver's formatting AND the
            // caller-controlled target string, so a session named after
            // one of these phrases could make an unrelated failure look
            // tolerated (the hazard `is_tolerated_list_panes_diagnostic`
            // documents in full). Each of these is a complete tmux
            // message; the ones naming a target continue past the phrase,
            // which is why this anchors rather than compares whole.
            Err(e) if tmux_said_any(&e, TmuxDriver::WINDOW_ALREADY_GONE_DIAGNOSTICS) => Ok(()),
            Err(e) => Err(e).context("killing a terminal tab's window"),
        }
    }

    /// Destroy one pane outright, leaving the rest of its session alone.
    ///
    /// Test-only, and it exists because no PRODUCTION path removes a
    /// single pane — sessions end through `kill_session` and tabs through
    /// `kill_window`. What needs provoking is the state in between: a
    /// session whose agent pane is gone while a tab window survives, which
    /// is what a sampler addressing a just-vanished pane actually meets.
    /// Nothing but a deliberate `kill-pane` produces it, since a pane that
    /// merely EXITS is kept around by this driver's `remain-on-exit`.
    #[cfg(test)]
    pub(crate) async fn kill_pane_for_test(&self, pane: &str) -> anyhow::Result<()> {
        self.run(&["kill-pane", "-t", pane]).await.map(|_| ())
    }

    /// The current size of the window containing `pane`, or `None` when
    /// tmux's answer cannot be parsed.
    ///
    /// Exists for the tab-open path: a `new-window` inherits the SESSION
    /// default size, not the agent window's size, so a freshly opened tab
    /// would be laid out at a geometry no client asked for and then take
    /// a real resize (and a mid-capture shell repaint) at first attach.
    /// Reading the agent window's size and applying it to the new tab
    /// window before the open reply publishes it is what makes that
    /// attach-time resize a no-op (BUGS_BURNDOWN.md issue 4).
    pub async fn window_size(
        &self,
        session: &str,
        pane: &str,
    ) -> anyhow::Result<Option<(u16, u16)>> {
        let out = self
            .run(&[
                "display-message",
                "-p",
                "-t",
                &pane_in_session(session, pane),
                "#{window_width} #{window_height}",
            ])
            .await?;
        let mut fields = out.split_whitespace();
        Ok(fields
            .next()
            .and_then(|w| w.parse::<u16>().ok())
            .zip(fields.next().and_then(|h| h.parse::<u16>().ok())))
    }

    /// Resize the window containing `pane`. `cols`/`rows` are clamped to
    /// tmux's accepted range: a browser reporting 0 columns (or an absurd
    /// value) must not turn into a tmux error, because callers treat
    /// resize as fire-and-forget.
    ///
    /// Targeted through the pane, not through the session name, and that
    /// is a correctness fix rather than a refactor (PLAN_M4.md item 3:
    /// resize goes per window). A bare `-t <session>` resolves to the
    /// session's CURRENT window, which was unambiguous only while a
    /// session had exactly one — with tabs it would reflow whichever
    /// window tmux last made current, so a client resizing its agent view
    /// could reshape somebody's tab instead.
    pub async fn resize_window(
        &self,
        session: &str,
        pane: &str,
        cols: u16,
        rows: u16,
    ) -> anyhow::Result<()> {
        let cols = cols.clamp(1, 10_000);
        let rows = rows.clamp(1, 10_000);
        self.run(&[
            "resize-window",
            "-t",
            &pane_in_session(session, pane),
            "-x",
            &cols.to_string(),
            "-y",
            &rows.to_string(),
        ])
        .await?;
        Ok(())
    }

    /// Query a pane's process state, scoped through `session` so a stale
    /// or coincidentally-reused pane id can never be mistaken for a
    /// different session's terminal.
    ///
    /// This CLASSIFIES staleness rather than assuming it away, and the
    /// distinction is not academic. An earlier version of these docs
    /// argued that no caller could hand this a pane from a dead server,
    /// because a `Terminal` is only ever populated for a pane confirmed
    /// against a live server at load time (`Supervisor::reload_sessions`'s
    /// `has_session` check) — and it treated a session mismatch as a hard
    /// error on the strength of that. On 2026-08-16 the production tmux
    /// server segfaulted mid-flight: nothing refreshes `Terminal` rows
    /// while a supervisor runs, the replacement server's pane counter
    /// restarted at `%0`, another session claimed that id, and every
    /// lifecycle verb for the original session — delete, archive, stop,
    /// restart, close-tab — failed on the mismatch until the supervisor
    /// was restarted. The invariant holds only at load; the mismatch is
    /// reachable in normal operation.
    ///
    /// So the three outcomes of [`PaneProbe`] are the contract, and each
    /// caller owns the decision its own operation needs. The
    /// `#{session_name}` check that produces `ForeignOwner` stays exactly
    /// as strict as before — it is the same belt this crate applies
    /// everywhere it can afford to (see `has_session`'s exact-match `=`
    /// prefix), and the foreign pane's pid still never escapes this
    /// function — but refusing to answer is not the same as refusing to
    /// act on the answer.
    ///
    /// `Err` is reserved for a query that did not produce an answer at
    /// all: a tmux invocation that could not be spawned, tmux exiting
    /// nonzero with any diagnostic OUTSIDE the tolerated "not there" set,
    /// or output that would not parse. Those tolerated diagnostics —
    /// the same ones `has_session`/`kill_session` recognize, covering a
    /// concurrent external `kill-session` and the whole private server
    /// having gone away — are [`PaneProbe::Gone`] instead.
    pub async fn pane_process(&self, session: &str, pane: &str) -> anyhow::Result<PaneProbe> {
        let out = match self
            .run(&[
                "display-message",
                "-p",
                "-t",
                pane,
                // SPACE-FREE FIELDS FIRST, session name LAST — the same
                // ordering `PANE_FACT_FORMAT` uses, and for the same
                // reason. tmux session names may contain spaces, so a name
                // in any position but the last cannot be told apart from
                // the fields following it. With the name leading (as this
                // did until the spaced-name regression), a session called
                // `renamed session` truncated to `renamed`, and one called
                // `<expected name> suffix` matched the expected name on
                // its first token and then wedged every lifecycle verb on
                // a parse error trying to read `suffix` as a pid. As the
                // trailing remainder, neither is expressible.
                "#{pane_pid} #{pane_dead} #{session_name}",
            ])
            .await
        {
            Ok(out) => out,
            Err(e)
                if [
                    "can't find session",
                    "no current target",
                    "no server running",
                ]
                .iter()
                .any(|diagnostic| e.to_string().contains(diagnostic)) =>
            {
                return Ok(PaneProbe::Gone);
            }
            Err(e) => return Err(e).context("querying pane process state"),
        };
        // Only the line terminator is stripped, never surrounding spaces:
        // a session name's own leading or trailing whitespace is part of
        // the name, and the trailing field must survive verbatim so
        // `ForeignOwner` can carry the FULL name a caller would log or
        // compare against.
        let line = out.trim_end_matches(['\r', '\n']);
        // A stale pane id against a server that has since dropped every
        // session it knew (this one killed, nothing else ever created)
        // does NOT error the way `has_session`'s docs describe for
        // `has-session` — verified empirically against this exact
        // config's `exit-empty off` server — it exits zero with every
        // format variable expanding empty, mirroring the same "nothing to
        // inspect" shape `bracket_paste_flag_is_missing` already handles
        // for pane-mode queries. With the fields reordered, that case is
        // the whole line collapsing to the format's own two separator
        // spaces, which is why the tell is a blank line rather than a
        // missing field: a pane that exists always has a numeric
        // `pane_pid`, so nothing real expands to whitespace alone.
        if line.trim().is_empty() {
            return Ok(PaneProbe::Gone);
        }
        // Exactly two splits: the third piece is the session name, spaces
        // and all.
        let mut fields = line.splitn(3, ' ');
        let pid_field = fields
            .next()
            .with_context(|| format!("pane process query returned no pane_pid: {line:?}"))?;
        let dead_field = fields
            .next()
            .with_context(|| format!("pane process query returned no pane_dead: {line:?}"))?;
        let found_session = fields
            .next()
            .with_context(|| format!("pane process query returned no session_name: {line:?}"))?;
        if found_session != session {
            // Ownership is settled before the pid is parsed, so a
            // stranger's pid never becomes a value any caller could reach
            // for — and a foreign pane with output this parser would
            // reject still classifies rather than erroring.
            return Ok(PaneProbe::ForeignOwner {
                owner: found_session.to_string(),
            });
        }
        let pid = pid_field
            .parse()
            .with_context(|| format!("parsing pane_pid from {line:?}"))?;
        let dead = dead_field == "1";
        Ok(PaneProbe::Owned(PaneProcess { pid, dead }))
    }

    /// Atomically check whether `pane` is on the alternate screen and, if
    /// so, capture its visible content — in ONE tmux invocation, not two.
    ///
    /// The flag and the capture MUST come from the same tmux process
    /// call: two separate calls (a `display-message` to read
    /// `#{alternate_on}`, then a later `capture-pane`) can straddle a
    /// genuine screen transition happening between them, either
    /// capturing primary-screen content while believing it was the
    /// (now-vacated) alternate screen, or missing an alt-screen frame
    /// entirely because the flag read landed just before the app switched
    /// into it. Chaining `display-message ... ';' capture-pane ...` as
    /// one tmux command line closes that window: tmux executes both
    /// against the same pane state before this process ever sees either
    /// result. `;` needs no shell escaping here — these are literal
    /// argv-per-command elements to `tmux` itself (this driver never goes
    /// through a shell, see [`TmuxDriver::command`]), not a shell
    /// metacharacter.
    ///
    /// `#{session_name}` rides along in the SAME `display-message` for the
    /// same reason `pane_process` checks it: exists purely as the
    /// [`AltScreenCapture::SessionMismatch`] guard against a recycled pane
    /// id, at no extra round-trip cost since the flag query already needs
    /// one `display-message` regardless.
    ///
    /// `-N` on `capture-pane` preserves trailing-cell STYLING — background
    /// color painted past the last non-blank character, common in
    /// full-screen TUIs that fill a row with a colored bar. Without it
    /// tmux trims styled trailing blank cells from each captured line,
    /// silently losing that padding (verified empirically: an
    /// inverse-video banner padded with spaces loses its trailing color
    /// without `-N`). `-e` keeps the escape sequences themselves, same as
    /// ordinary replay capture.
    ///
    /// # Bounded reading
    ///
    /// `max_bytes` bounds how much of the combined invocation's stdout
    /// this reads into memory before giving up: a pane resized to tmux's
    /// own maximum (10,000×10,000, `-N` capturing styling for every one
    /// of those cells) can emit well over 100 MiB, and reading that
    /// wholesale — which is what a `Command::output()`-style call
    /// (`TmuxDriver::run_bytes`) does internally, buffering the ENTIRE
    /// stream before this function ever sees a single byte — would let a
    /// single oversized pane balloon this process's memory on every stop.
    /// This method instead streams the child's stdout in
    /// [`ALT_SCREEN_READ_CHUNK`]-sized reads, accumulating at most
    /// `max_bytes + `[`ALT_SCREEN_HEADER_SLACK`] bytes total before
    /// killing the child outright and returning [`AltScreenCapture::TooLarge`]
    /// — the discarded remainder is never read at all, so the OS pipe
    /// buffer (bounded, ~64 KiB) is the actual ceiling on how much of an
    /// enormous capture this process ever holds, not `max_bytes` itself.
    ///
    /// This bounding is what makes the single-invocation design (see
    /// above) affordable even though tmux itself has no way to skip
    /// `capture-pane` when it turns out the pane was not on the alternate
    /// screen: EVERY call here pays for however much of the (possibly
    /// oversized) capture gets read before the bound trips, whether the
    /// header says `NotAlternate`, `SessionMismatch`, or genuinely
    /// `Captured` — there is no way to know which, without decoding the
    /// header, before the capture body has already been requested and
    /// started arriving. Accepting that fixed, BOUNDED cost on every call
    /// (rather than optimizing away captures this function is about to
    /// discard anyway) is the price of keeping the flag-check and the
    /// capture atomic; a two-invocation design could skip the capture
    /// call entirely for a primary-screen pane, at the cost of
    /// reintroducing the screen-transition race this method exists to
    /// close.
    ///
    /// The child's stderr is drained concurrently on a background task
    /// (not sequentially after stdout) so a tmux failure message large
    /// enough to fill ITS OWN pipe buffer — vanishingly unlikely for the
    /// short diagnostics tmux actually emits, but not to be assumed away
    /// — can never wedge this function waiting on a stdout EOF that a
    /// stalled-on-stderr child will never produce.
    pub async fn capture_alt_screen_if_active(
        &self,
        session: &str,
        pane: &str,
        max_bytes: usize,
    ) -> anyhow::Result<AltScreenCapture> {
        let read_cap = max_bytes.saturating_add(ALT_SCREEN_HEADER_SLACK);
        let mut child = self
            .command()
            .args([
                "display-message",
                "-p",
                "-t",
                pane,
                "#{alternate_on} #{session_name}",
                ";",
                "capture-pane",
                "-e",
                "-p",
                "-N",
                "-t",
                pane,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("spawning tmux for alt-screen capture")?;
        let mut stdout = child.stdout.take().expect("piped stdout");
        let mut stderr = child.stderr.take().expect("piped stderr");
        let stderr_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            let _ = stderr.read_to_end(&mut buf).await;
            buf
        });

        let mut buf = Vec::new();
        // Heap, not `[0u8; ALT_SCREEN_READ_CHUNK]`. A 64 KiB array here is a
        // local held across an `.await`, so it becomes part of this
        // function's FUTURE — and every future that composes this one grows
        // by that much in turn, which on the restart path is a chain several
        // deep (`restart_session` → `relaunch` → `relaunch_into_terminal` →
        // `plan_pane_relaunch` → here). Inflating those futures inflates the
        // stack the `poll` chain needs to drive them, and a debug build's
        // restart tests were running within a few hundred kilobytes of a
        // test thread's default stack because of it; adding one more await
        // to that chain (the cgroup work's `reap_process_tree`) tipped it
        // over. One allocation per capture buys the whole chain back.
        let mut chunk = vec![0u8; ALT_SCREEN_READ_CHUNK];
        let too_large = loop {
            let n = stdout
                .read(&mut chunk)
                .await
                .context("reading tmux alt-screen capture output")?;
            if n == 0 {
                break false;
            }
            buf.extend_from_slice(&chunk[..n]);
            if !within_snapshot_cap(buf.len(), read_cap) {
                break true;
            }
        };
        // Dropping the read half here (rather than leaving it borrowed by
        // `child` past this point) is not load-bearing for correctness —
        // `kill_on_drop`/the explicit `kill` below make the child's exit
        // unconditional either way — but it does let the pipe's read end
        // close immediately if this function returns without reaching
        // `child.wait()` on some future error path, instead of staying
        // open for the remainder of this scope.
        drop(stdout);

        if too_large {
            // The child may still be trying to write an arbitrarily large
            // remaining capture into a pipe nobody is draining anymore —
            // kill it outright rather than continuing to read (and
            // discard) however much more it has queued up. `wait()`
            // afterward reaps it; `kill_on_drop` alone would only
            // guarantee that eventually, not before this function
            // returns.
            let _ = child.kill().await;
            let _ = child.wait().await;
            let _ = stderr_task.await;
            return Ok(AltScreenCapture::TooLarge);
        }

        let stderr_bytes = stderr_task.await.unwrap_or_default();
        let status = child
            .wait()
            .await
            .context("waiting for tmux alt-screen capture to exit")?;
        if !status.success() {
            bail!(
                "tmux alt-screen capture failed ({status}): {}",
                String::from_utf8_lossy(&stderr_bytes).trim()
            );
        }
        Ok(parse_alt_screen_capture(&buf, session, max_bytes))
    }

    /// The last thing `pane` printed, as plain text, bounded to
    /// `max_bytes`.
    ///
    /// Exists for exactly one caller: the refused `OpenTab`'s error detail
    /// (PLAN_M4.md item 2 — "the pane's last words"). Deliberately NOT
    /// [`Self::capture_alt_screen_if_active`]: this wants human-readable
    /// prose to put inside a protocol error message, so escape sequences
    /// are left out (`-e` omitted) and the alternate screen is
    /// irrelevant — a shell that could not start prints a line or two to
    /// the primary screen and dies.
    ///
    /// # Why history, not the visible screen
    ///
    /// The pane this is called on is a DEAD one, and `remain-on-exit`
    /// replaces a dead pane's visible grid with tmux's own
    /// `remain-on-exit-format` banner ("Pane is dead (status 9, …)") —
    /// verified against tmux 3.7b. A plain `capture-pane -p` therefore
    /// returns that banner and nothing else, which restates the exit code
    /// the caller already has and loses the one thing worth reporting. The
    /// shell's actual output is one scroll back, in history, which
    /// [`LAST_WORDS_LINES`] reaches.
    ///
    /// That is exactly the difference from [`Self::capture_pane_tail`],
    /// whose subject is a LIVE pane and which therefore must not reach
    /// back into history at all.
    pub async fn capture_pane_text(
        &self,
        session: &str,
        pane: &str,
        max_bytes: usize,
    ) -> anyhow::Result<String> {
        self.capture_pane_plain(session, pane, Some(LAST_WORDS_LINES), max_bytes)
            .await
    }

    /// What `pane`'s screen shows RIGHT NOW, as plain text, bounded to
    /// `max_bytes`.
    ///
    /// The status sampler's read (PLAN_M6_75.md item 1): each tick takes a
    /// BUDGETED number of these — a round-robin slice of the live
    /// sessions, not all of them — both to notice that output moved since
    /// that session's previous sample and to hand the per-kind sharpeners
    /// something to recognize a prompt or an approval request in.
    ///
    /// Visible grid only — no `-S`, and that is the whole point rather
    /// than a saving. A running agent's question sits on the CURRENT
    /// screen; scrollback would drag in text the agent has already moved
    /// past, which for change detection means a tail that keeps growing
    /// (every tick "different", so every session forever "running") and
    /// for sharpening means matching a prompt shape that was answered
    /// minutes ago. For a full-screen TUI this captures the alternate
    /// screen, because that is what tmux considers the pane's current
    /// grid — again what the sampler wants.
    ///
    /// Escape sequences are omitted (`-e`): both consumers read this as
    /// prose, and SGR noise would make every redraw of an unchanged screen
    /// look like new output.
    pub async fn capture_pane_tail(
        &self,
        session: &str,
        pane: &str,
        max_bytes: usize,
    ) -> anyhow::Result<String> {
        self.capture_pane_plain(session, pane, None, max_bytes)
            .await
    }

    /// The shared body of the two plain-text captures above:
    /// `capture-pane -p` over `history_lines` of scrollback (or the
    /// visible grid alone when `None`), reduced by [`last_words`].
    ///
    /// The TAIL is kept when the result is over `max_bytes`, not the head:
    /// for the dead-pane caller these are last words, and a shell that
    /// printed a wall of rc-file noise before its real complaint would
    /// otherwise have the complaint truncated away; for the sampler the
    /// bottom of the screen is where an agent's current question lives.
    /// Truncation lands on a CHARACTER boundary so the result is always
    /// valid UTF-8, and the text is lossy-decoded rather than refused for
    /// an exotic byte — mangling one beats declining to answer.
    ///
    /// # Why this streams instead of calling [`Self::run_bytes`]
    ///
    /// `run_bytes` uses `Command::output()`, which buffers the child's
    /// ENTIRE stdout before anything can truncate it. `capture-pane` is
    /// the one command in this driver whose output is bounded by the
    /// terminal grid rather than by a format string, and tmux's grid
    /// ceiling is 10,000 columns by 10,000 rows — a caller asking for a
    /// 4 KiB tail could transiently allocate a hundred megabytes on its
    /// way to it. Reading incrementally and retaining only the last
    /// [`TAIL_RETAIN_ALLOWANCE`] bytes past `max_bytes` makes the peak
    /// proportional to the ANSWER rather than to the pane, which matters
    /// because the sampler runs this on a schedule, once per live session
    /// per rotation.
    ///
    /// The retention slack exists because [`last_words`] trims trailing
    /// blank rows AFTER this point: a pane padded out with thousands of
    /// empty rows would otherwise have its real text pushed out of a
    /// window sized to `max_bytes` exactly.
    ///
    /// # The deadline
    ///
    /// Bounded by the same [`PANE_LIST_TIMEOUT`] budget the pane-listing
    /// query uses — a wedged tmux must not park the sampler (or a tab
    /// refusal's error path) indefinitely, and a test that loosens tmux
    /// budgets for a loaded CI runner should loosen this one with them.
    /// The child is killed and reaped on expiry rather than left to
    /// finish writing into a pipe nobody is draining.
    ///
    /// # Pane identity
    ///
    /// The pane is paired with its session ([`pane_in_session`]), and
    /// `capture-pane` REFUSES an unresolvable pairing rather than falling
    /// back to some other pane: verified empirically against tmux 3.7b for
    /// all three shapes that matter here — a pane id that no longer exists
    /// ("can't find pane: %0"), one that never existed, and one that
    /// exists but under a DIFFERENT session than the caller named. That
    /// refusal is what makes it safe for the sampler to address a pane it
    /// last saw a moment ago: an agent pane killed mid-pass yields an
    /// error the caller skips, never a sibling tab's screen recorded as
    /// the agent's. See this module's `pane_in_session` for the same
    /// property as every other pane-scoped command relies on it.
    async fn capture_pane_plain(
        &self,
        session: &str,
        pane: &str,
        history_lines: Option<u32>,
        max_bytes: usize,
    ) -> anyhow::Result<String> {
        let target = pane_in_session(session, pane);
        let start = history_lines.map(|lines| format!("-{lines}"));
        let mut args: Vec<&str> = vec!["capture-pane", "-p", "-t", &target];
        if let Some(start) = start.as_deref() {
            args.extend_from_slice(&["-S", start]);
        }
        let tail = self
            .run_bytes_tail(&args, max_bytes.saturating_add(TAIL_RETAIN_ALLOWANCE))
            .await
            .context("capturing a pane's text")?;
        Ok(last_words(&String::from_utf8_lossy(&tail), max_bytes))
    }

    /// Run one tmux command and return AT MOST the last `retain` bytes of
    /// its stdout, killing it if it outlives [`PANE_LIST_TIMEOUT`].
    ///
    /// [`Self::run_bytes`]'s bounded cousin, for the one command whose
    /// output size is the user's terminal rather than this driver's own
    /// format string. Everything about the failure shape is deliberately
    /// identical — a non-zero exit still returns a [`TmuxCommandFailure`]
    /// root carrying tmux's raw stderr under the same rendered context —
    /// so callers that classify diagnostics (`is_definitively_empty`) or
    /// quote tmux's prose behave the same whichever one they went
    /// through.
    ///
    /// The retained buffer is compacted only when it grows past twice
    /// `retain`, so the amortized cost is one memmove per `retain` bytes
    /// read rather than one per chunk.
    async fn run_bytes_tail(&self, args: &[&str], retain: usize) -> anyhow::Result<Vec<u8>> {
        let mut child = self
            .command()
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("spawning tmux")?;
        let mut stdout = child.stdout.take().expect("piped stdout");
        let mut stderr = child.stderr.take().expect("piped stderr");
        let stderr_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            let _ = stderr.read_to_end(&mut buf).await;
            buf
        });
        // Heap rather than a stack array, for the same future-size reason
        // `capture_alt_screen_if_active` spells out: this buffer is held
        // across an await and would otherwise inflate every future that
        // composes this one.
        let mut chunk = vec![0u8; ALT_SCREEN_READ_CHUNK];
        let mut tail: Vec<u8> = Vec::new();
        let drained = tokio::time::timeout(self.pane_list_timeout, async {
            loop {
                let n = stdout
                    .read(&mut chunk)
                    .await
                    .context("reading tmux capture output")?;
                if n == 0 {
                    return anyhow::Ok(());
                }
                tail.extend_from_slice(&chunk[..n]);
                if tail.len() > retain.saturating_mul(2) {
                    tail.drain(..tail.len() - retain);
                }
            }
        })
        .await;
        drop(stdout);
        let Ok(drained) = drained else {
            // Nobody is draining the pipe anymore, so a tmux still trying
            // to write a large capture would block forever. Kill and reap
            // here rather than relying on `kill_on_drop` to get to it
            // eventually.
            let _ = child.kill().await;
            let _ = child.wait().await;
            let _ = stderr_task.await;
            anyhow::bail!(
                "tmux {args:?} did not finish within {:?}",
                self.pane_list_timeout
            );
        };
        drained?;
        if tail.len() > retain {
            tail.drain(..tail.len() - retain);
        }
        let stderr_bytes = stderr_task.await.unwrap_or_default();
        let status = child.wait().await.context("waiting for tmux to exit")?;
        if !status.success() {
            let context = format!(
                "tmux {:?} failed ({}): {}",
                args,
                status,
                String::from_utf8_lossy(&stderr_bytes).trim()
            );
            return Err(anyhow::Error::new(TmuxCommandFailure {
                stderr: stderr_bytes,
            }))
            .context(context);
        }
        Ok(tail)
    }

    /// The tmux diagnostics that all mean "the window this names is
    /// already gone" — the tolerated outcomes of [`Self::kill_window`],
    /// which is what makes `CloseTab` safe to retry after a partial
    /// failure.
    ///
    /// The target is expressed as a pane within a session, so tmux
    /// reports a vanished window through whichever part of that pairing
    /// it failed to resolve first; all four spellings, plus the whole
    /// server being gone, are the same fact for a caller.
    const WINDOW_ALREADY_GONE_DIAGNOSTICS: &[&str] = &[
        "can't find pane",
        "can't find session",
        "can't find window",
        "no such window",
        "no current target",
        "no server running",
    ];

    /// A genuinely empty server (started, but nothing created on it yet)
    /// answers `list-panes -a` with EXACTLY this diagnostic and nothing
    /// else on the line (verified empirically against both tmux 3.4 and
    /// 3.7b) — distinct from [`Self::LIST_PANES_NO_SERVER_PREFIX`] and
    /// [`Self::LIST_PANES_SERVER_EXITED_DIAGNOSTIC`], which mean the
    /// private tmux server process itself is gone. All three are tolerated
    /// by `pane_states` today (see its own docs), but are kept as separate
    /// constants because they answer different questions: this one is "the
    /// server exists but has nothing on it", the other two are "there is no
    /// server left to ask at all", reached via two different tmux code
    /// paths (a clean absence vs. a server that died mid-request).
    const LIST_PANES_EMPTY_SERVER_DIAGNOSTIC: &str = "no current target";

    /// tmux's `list-panes -a` diagnostic PREFIX when the private tmux
    /// server process itself is gone (crash, OOM, an operator killing it
    /// out from under a still-running supervisor) — as opposed to
    /// [`Self::LIST_PANES_EMPTY_SERVER_DIAGNOSTIC`], where a server is up
    /// but empty. Verified empirically (both tmux 3.4 and 3.7b) to always
    /// be followed by the exact socket path passed via `-S`, with nothing
    /// else on the line — never a bare, path-free message — which is why
    /// [`is_tolerated_list_panes_diagnostic`] anchors against this prefix
    /// PLUS this driver's own known socket, rather than a substring search:
    /// the socket path is caller-controlled (it derives from the
    /// supervisor's state dir), so a naive `contains("no server running")`
    /// over the rendered error could misfire if that path happened to
    /// embed the phrase itself, folding an unrelated failure into "the
    /// server is gone". See `pane_states`'s own docs for why this
    /// diagnostic is tolerated exactly like the empty-server case rather
    /// than propagated as an error.
    const LIST_PANES_NO_SERVER_PREFIX: &str = "no server running on ";

    /// tmux's `list-panes -a` diagnostic when a request RACES a dying
    /// server — the exact crash/OOM/`kill-server` timing this fix exists
    /// for, where the server is mid-teardown rather than already fully
    /// gone (verified empirically against both tmux 3.4 and 3.7b: racing a
    /// `kill-server` with a concurrent query reliably produces this
    /// message on both, standalone with nothing else on the line). The
    /// harness's own `kill_tmux_server_and_wait` helper (crate `farhelm`'s
    /// e2e tests) documents seeing this exact text while polling for a
    /// server to finish dying. Tolerated identically to
    /// [`Self::LIST_PANES_NO_SERVER_PREFIX`]: both are tmux's own
    /// definitive statement that no pane can be answered for right now,
    /// which is the same "no panes exist" fact `pane_states` treats as an
    /// honest empty map either way.
    const LIST_PANES_SERVER_EXITED_DIAGNOSTIC: &str = "server exited unexpectedly";

    /// Every pane's liveness state, in ONE tmux round trip.
    ///
    /// `ListSessions` needs this per-session (PLAN_M2.md's "Proto growth":
    /// status is computed at list time), and a naive per-session
    /// `pane_process` call for each row would multiply one subprocess
    /// spawn into N — paid on every poll the UI makes, per session. One
    /// `list-panes -a` query instead returns every pane's state at once,
    /// keyed by `#{pane_id}` (`Terminal::pane`'s own value) rather than
    /// session name: a session's window can hold more than one pane (a
    /// future split, say), and keying by session name would let a second
    /// pane silently overwrite the first pane's entry in the returned map
    /// — pane id is the only identifier this module ever uses to address
    /// one specific pane (see `Terminal`'s own docs), so it is the only
    /// key that cannot collide between panes sharing a session.
    ///
    /// `#{session_name}` rides along in the same query, carried in each
    /// [`PaneState`] (see its own docs on why): pane ids reset to `%0` on
    /// a fresh tmux server, so pane id alone is not a stable enough
    /// identity across a server restart for `session_status` to trust
    /// blindly — it cross-checks the session name too.
    ///
    /// A genuinely empty server ([`LIST_PANES_EMPTY_SERVER_DIAGNOSTIC`]), a
    /// genuinely ABSENT server ([`LIST_PANES_NO_SERVER_PREFIX`]), and a
    /// server caught mid-teardown ([`LIST_PANES_SERVER_EXITED_DIAGNOSTIC`])
    /// all degrade to an empty map rather than an error — mirroring
    /// `pane_process`'s own empty-expansion handling for the same "nothing
    /// to inspect" case. `kill_session` tolerates the same "no server
    /// running" shape for the same underlying reason (both treat "nothing
    /// left to act on" as success); `has_session`, by contrast, DELIBERATELY
    /// propagates it as an error, because its one caller — deciding whether
    /// a PERSISTED row survived a restart — needs to tell "the query itself
    /// failed" apart from "this row's tmux is genuinely gone", and cannot
    /// safely guess between them the way this method's own callers can (see
    /// below).
    ///
    /// This supersedes an earlier design (recorded in this module's git
    /// history and in the e2e test this behavior change rewrote) that
    /// treated `"no server running"` as a real error here, reasoning that a
    /// still-live supervisor whose private tmux server vanished out from
    /// under it should not silently report every tracked session `Exited`
    /// off a "fabricated" empty map — that would be indistinguishable from
    /// an honestly observed mass exit, the reasoning went. That conflated
    /// two different things: an empty pane-states MAP is not an empty
    /// session LISTING. This method's return value plays no part in WHICH
    /// rows `ListSessions` selects for its reply — that is the session cap
    /// and byte budget's job (`service.rs`'s `LIST_SESSION_CAP` and
    /// `build_list_reply`), applied independently of tmux entirely — an
    /// empty map here only ever feeds `session_status`'s per-entry liveness
    /// lookup for whichever rows that selection already kept. `"no server
    /// running"` is a DEFINITIVE statement from tmux that no pane exists
    /// anywhere on this socket, so an empty map is not a guess in that case
    /// — it is the literal truth, and every terminal-bearing entry then
    /// correctly reports through `session_status`'s existing missing-pane
    /// branch as `Exited { exit_code: None }`, the same honest answer a
    /// restart-gap row already gets. The old behavior instead turned a dead
    /// tmux server into a hard `ListSessions` failure — every session
    /// unreachable THROUGH THE UI (which has no session ids left to act on,
    /// including for delete, once the list that would supply them fails to
    /// load) even though every one of them was intact in SQLite, and
    /// `DeleteSession`'s own handler was never itself refused — which is the
    /// actual failure mode this change exists to close. This method does
    /// NOT attempt to restart or resurrect the vanished server; recovery is
    /// M3 (PLAN.md), and until then an affected session simply reports
    /// `Exited` — a plain supervisor restart reloads its row terminal-less
    /// (the ordinary restart-gap case), still `Exited`, not "recovered".
    ///
    /// Any OTHER `pane_states` failure — one that does not match any
    /// tolerated diagnostic — still propagates as a real `Err`. That
    /// distinction is deliberate: an unclassified tmux failure is genuinely
    /// UNKNOWN, and laundering it into a confident "every session exited"
    /// would be exactly the kind of guessed liveness claim this module
    /// works hard everywhere else to avoid. The match is against tmux's OWN
    /// raw stderr (recovered via `downcast_ref::<TmuxCommandFailure>`, not
    /// a substring search over the rendered error) and anchored to the
    /// EXACT recognized shapes — see [`is_tolerated_list_panes_diagnostic`]
    /// for why a substring search is unsafe here.
    ///
    /// A pane absent from the returned map — or present under this pane
    /// id but for a DIFFERENT session name than the caller remembers
    /// (`PaneState::session_name`'s own docs: pane ids reset to `%0` on a
    /// fresh server, so a stale, never-reloaded entry's pane id can be
    /// recycled by an unrelated new session) — is deliberately NOT this
    /// method's problem to explain. The caller (`service.rs`'s
    /// `session_status`) is the one that knows what either case means for
    /// its own session (a race against the pane being removed mid-query,
    /// or that pane-id-reuse-after-restart scenario): reporting it as
    /// `Exited { exit_code: None }` is the same "do not guess a liveness
    /// claim" rule the restart-gap case already applies — that handling
    /// is unaffected by this method's own, narrower, error tolerance.
    /// Renaming a session's tmux name, notably, does NOT land in either
    /// bucket: pane id survives a rename (verified empirically), so a
    /// renamed session's pane is still found under its ORIGINAL pane id
    /// with its NEW session name — see `session_status`'s own docs on how
    /// it is expected to react to that specific mismatch.
    /// # Two queries, and when the second is skipped
    ///
    /// The window markers this also reports (`PaneState::tab`/`agent`) are
    /// the only fields anything outside this supervisor can write, so they
    /// are fetched by a SECOND query and joined against the first — see
    /// [`PANE_MARKER_FORMAT`] for the fabrication that separation closes.
    ///
    /// That second round trip is skipped entirely when no session on the
    /// server has more than one window, which keeps the pre-tabs cost of
    /// this call — one subprocess, paid on every `ListSessions` poll —
    /// exactly what it was. The skip is sound because a tab IS an
    /// additional window: a session with one window can have no tab, and
    /// the one consumer of the AGENT marker (reload's pane-less recovery)
    /// falls back to the single window it would have chosen anyway.
    pub async fn pane_states(&self) -> anyhow::Result<HashMap<String, PaneState>> {
        let facts = match self
            .run(&["list-panes", "-a", "-F", PANE_FACT_FORMAT])
            .await
        {
            Ok(out) => out,
            Err(e) => {
                // Classify against tmux's own RAW stderr, recovered from
                // the error chain rather than pattern-matched off the
                // rendered message — see `TmuxCommandFailure`'s and
                // `is_tolerated_list_panes_diagnostic`'s docs for why. An
                // error with no `TmuxCommandFailure` in its chain at all
                // (a spawn failure, say) is never tolerated: there is no
                // tmux diagnostic to even inspect.
                if self.is_definitively_empty(&e) {
                    return Ok(HashMap::new());
                }
                return Err(e).context("querying pane states");
            }
        };
        let mut states = parse_pane_facts(&facts);
        if !any_session_has_several_windows(&states) {
            return Ok(states);
        }
        match self
            .run(&["list-panes", "-a", "-F", PANE_MARKER_FORMAT])
            .await
        {
            Ok(markers) => join_pane_markers(&mut states, &markers),
            // The server going away between the two queries is the same
            // "nothing to report" the first query tolerates, and leaves
            // every pane honestly unmarked rather than failing a call
            // whose authoritative half already succeeded.
            Err(e) if self.is_definitively_empty(&e) => {}
            Err(e) => return Err(e).context("querying pane window markers"),
        }
        Ok(states)
    }

    /// Whether `error` is tmux definitively saying there is nothing on
    /// this driver's socket to answer for — an empty server, an absent
    /// one, or one caught mid-teardown.
    ///
    /// Public so callers that must tell "there is genuinely nothing here"
    /// apart from "the query failed" can ask the same question this
    /// module answers internally, against tmux's own raw stderr rather
    /// than a substring search over a rendered error (see
    /// [`is_tolerated_list_panes_diagnostic`]).
    pub fn is_definitively_empty(&self, error: &anyhow::Error) -> bool {
        error
            .downcast_ref::<TmuxCommandFailure>()
            .is_some_and(|failure| {
                is_tolerated_list_panes_diagnostic(&failure.stderr_trimmed(), &self.socket)
            })
    }
}

/// Close one output-bearing control client without invalidating tmux's queue.
///
/// The caller must first establish an acknowledged client-wide `no-output`
/// boundary through [`TmuxDriver::disable_control_client_output`]. tmux may
/// otherwise still own pane-output blocks for this client, and even stdin EOF
/// can make tmux 3.7b abort when a pending callback tries to finish bytes the
/// client teardown invalidated. After the boundary, EOF is the normal
/// control-mode exit and draining stdout lets the client finish even when its
/// last notifications would otherwise fill the pipe.
///
/// The timeout retains `kill_on_drop`'s bounded-cleanup property. Falling back
/// to a kill is safe only because the caller already established `no-output`;
/// it is still exceptional and logged because graceful EOF is the expected
/// control-mode exit.
async fn shutdown_output_control_client(
    child: &mut Child,
    stdin: Option<ChildStdin>,
    reader: &mut BufReader<ChildStdout>,
    kind: &'static str,
) -> anyhow::Result<()> {
    if child
        .try_wait()
        .with_context(|| format!("checking whether the {kind} already exited"))?
        .is_some()
    {
        return Ok(());
    }

    drop(stdin);
    let graceful = tokio::time::timeout(CONTROL_CLIENT_SHUTDOWN_TIMEOUT, async {
        let mut sink = tokio::io::sink();
        let drain = tokio::io::copy(reader, &mut sink);
        tokio::pin!(drain);
        tokio::select! {
            status = child.wait() => status,
            drained = &mut drain => {
                drained?;
                child.wait().await
            }
        }
    })
    .await;

    match graceful {
        Ok(Ok(_)) => return Ok(()),
        Ok(Err(error)) => {
            warn!(kind, error = %error, "control client could not finish gracefully; killing it");
        }
        Err(_) => {
            warn!(
                kind,
                "control client did not finish gracefully before its reap deadline; killing it"
            );
        }
    }

    if child
        .try_wait()
        .with_context(|| format!("rechecking whether the {kind} exited"))?
        .is_some()
    {
        return Ok(());
    }
    child
        .kill()
        .await
        .with_context(|| format!("killing and reaping the {kind}"))
}

/// Whether `stderr` — tmux's own RAW stderr from a failed `list-panes -a`,
/// already trimmed by [`TmuxCommandFailure::stderr_trimmed`] — is one of
/// the three diagnostics [`TmuxDriver::pane_states`] tolerates as "nothing
/// to report" rather than a real failure: a genuinely empty server
/// ([`TmuxDriver::LIST_PANES_EMPTY_SERVER_DIAGNOSTIC`]), a genuinely absent
/// one ([`TmuxDriver::LIST_PANES_NO_SERVER_PREFIX`] followed by `socket`),
/// or one caught mid-teardown
/// ([`TmuxDriver::LIST_PANES_SERVER_EXITED_DIAGNOSTIC`]).
///
/// EVERY comparison here is an exact match against the WHOLE trimmed
/// string, never a substring search — `stderr.contains(diagnostic)` would
/// look tempting, but `socket` is a caller-controlled path (it derives from
/// the supervisor's state dir) that tmux's own "no server running on
/// `<path>`" diagnostic bakes verbatim into its message. A state dir an
/// operator happened to name so its path CONTAINS one of these phrases —
/// `/tmp/no server running/tmux.sock`, however unlikely — would make an
/// entirely unrelated failure (a permission error, a corrupted socket file)
/// that merely MENTIONS that same path look like a tolerated diagnostic
/// under a substring search, silently laundering a genuine fault into "the
/// server is gone; report everyone exited". Anchoring to the exact,
/// complete message tmux is verified (both tmux 3.4 and 3.7b) to emit for
/// each of these three cases — and nothing else — closes that hole:
/// something merely CONTAINING one of these phrases inside a longer,
/// differently-shaped message is never one of them.
///
/// Split out from [`TmuxDriver::pane_states`] purely so this classification
/// is unit-testable against constructed strings, without spawning tmux or
/// killing a real server to provoke any of the three — the same reasoning
/// [`parse_pane_facts`] and `PaneModes::parse` split their own parsing out
/// for elsewhere in this module. Any OTHER message returns `false`, which is
/// what sends an unclassified failure down `pane_states`'s error path
/// instead of being silently folded into an empty (and therefore
/// all-exited) map.
/// Whether `error` carries tmux's own stderr and it BEGINS with any of
/// `prefixes`.
///
/// Anchored at the START rather than searched anywhere in the message,
/// and read off [`TmuxCommandFailure`] rather than off the rendered
/// `anyhow` chain. Both halves matter for the same reason
/// [`is_tolerated_list_panes_diagnostic`] spells out at length: the
/// rendered error embeds this driver's own formatting and the target
/// string the caller supplied, and a session or path named after one of
/// these phrases would otherwise make an unrelated failure look
/// tolerated. tmux emits each of these as a complete standalone message
/// beginning with the phrase, sometimes continuing with the target it
/// could not find — hence a prefix rather than an equality.
fn tmux_said_any(error: &anyhow::Error, prefixes: &[&str]) -> bool {
    error
        .downcast_ref::<TmuxCommandFailure>()
        .is_some_and(|failure| {
            let stderr = failure.stderr_trimmed();
            prefixes.iter().any(|prefix| stderr.starts_with(prefix))
        })
}

fn is_tolerated_list_panes_diagnostic(stderr: &str, socket: &Path) -> bool {
    stderr == TmuxDriver::LIST_PANES_EMPTY_SERVER_DIAGNOSTIC
        || stderr == TmuxDriver::LIST_PANES_SERVER_EXITED_DIAGNOSTIC
        || stderr
            == format!(
                "{}{}",
                TmuxDriver::LIST_PANES_NO_SERVER_PREFIX,
                socket.display()
            )
}

/// Turn raw `capture-pane` output into replayable terminal content.
///
/// Two transforms, both load-bearing (a past live bug — see lore/):
/// capture-pane emits LF line endings, and a terminal needs CRLF or
/// every line inherits the previous line's column; and the trailing
/// newline must go, because capture-pane terminates the LAST row too —
/// replaying it verbatim scrolls the screen one row past the content,
/// landing the restored cursor a row low on the normal screen and
/// destroying the app's top row on the alternate screen (which has no
/// scrollback to absorb the scroll).
fn normalize_capture(out: &[u8]) -> Vec<u8> {
    let body = out.strip_suffix(b"\n").unwrap_or(out);
    let mut normalized = Vec::with_capacity(body.len() + body.len() / 32);
    for &b in body {
        if b == b'\n' {
            normalized.push(b'\r');
        }
        normalized.push(b);
    }
    normalized
}

/// Strip the terminator from a control-mode notification line.
///
/// Every marker check in this module compares against the line *without*
/// its ending, because the bounded line reader leaves it attached and
/// tmux is not consistent about whether a `\r` precedes it. Applies only to
/// tmux's own notification lines — pane output is escaped payload inside
/// `%output`/`%extended-output` and must never be touched.
fn strip_line_ending(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::test_support::{ScratchServer, tail_containing};
    use super::*;

    /// The alternate-screen switch must be in the PRE-content half and
    /// nowhere else: it clears the buffer it switches to, so emitting it
    /// after the replay would erase the replay. Cursor placement belongs
    /// after the content, because writing content moves the cursor.
    #[test]
    fn alt_screen_switch_precedes_content_and_cursor_follows_it() {
        let modes = PaneModes {
            alternate_on: true,
            bracket_paste: true,
            cursor_visible: false,
            cursor_x: 4,
            cursor_y: 2,
            ..Default::default()
        };
        assert_eq!(modes.pre_content_sequences(), "\x1b[?1049h");

        let post = modes.post_content_sequences();
        assert!(!post.contains("\x1b[?1049h"));
        assert!(post.contains("\x1b[?2004h"));
        let pos = post.find("\x1b[3;5H").expect("1-based cursor position");
        let hide = post.find("\x1b[?25l").expect("hidden cursor stays hidden");
        assert!(pos < hide, "visibility must come after positioning");
    }

    /// Every tmux mode field maps to a distinct terminal escape. A
    /// swapped field still produces plausible output, so each branch
    /// needs an independent oracle rather than one all-flags snapshot.
    #[test]
    fn pane_modes_restore_each_mouse_and_cursor_mode() {
        let cases = [
            ("0,0,0,0,1,0,1,0,0,0", "\x1b[?1000h"),
            ("0,0,0,1,0,0,1,0,0,0", "\x1b[?1002h"),
            ("0,0,1,0,0,0,1,0,0,0", "\x1b[?1003h"),
            ("0,0,0,0,0,1,1,0,0,0", "\x1b[?1006h"),
            ("0,0,0,0,0,0,1,1,0,0", "\x1b[?1h"),
        ];
        for (fields, expected) in cases {
            let output = PaneModes::parse(fields).post_content_sequences();
            assert!(
                output.contains(expected),
                "{fields} did not restore {expected:?}: {output:?}"
            );
        }
    }

    /// One DECSET code per real tmux state, with the OTHER two mouse
    /// codes asserted absent per case — that absence check is what
    /// catches a reversion to additive (independent `if`-per-flag)
    /// emission, which would satisfy a presence-only assertion just fine
    /// (see `post_content_sequences`'s historical note).
    #[test]
    fn pane_modes_selects_one_mouse_protocol() {
        // fields: (mouse_all, mouse_button, mouse_standard). expected:
        // the one DECSET code that must appear; the other two mouse
        // codes must not.
        let cases: [(&str, &str, [&str; 2]); 3] = [
            ("0,0,1", "\x1b[?1000h", ["\x1b[?1002h", "\x1b[?1003h"]), // standard-only
            ("0,1,0", "\x1b[?1002h", ["\x1b[?1000h", "\x1b[?1003h"]), // button-only
            ("1,0,0", "\x1b[?1003h", ["\x1b[?1000h", "\x1b[?1002h"]), // all-only
        ];
        for (mouse_fields, expected, absent) in cases {
            let fields = format!("0,0,{mouse_fields},0,1,0,0,0");
            let output = PaneModes::parse(&fields).post_content_sequences();
            assert!(
                output.contains(expected),
                "{fields}: missing {expected:?} in {output:?}"
            );
            for code in absent {
                assert!(
                    !output.contains(code),
                    "{fields}: wrongly also emitted {code:?} in {output:?}"
                );
            }
        }

        // None set: no mouse DECSET code at all.
        let none = PaneModes::parse("0,0,0,0,0,0,1,0,0,0").post_content_sequences();
        for code in ["\x1b[?1000h", "\x1b[?1002h", "\x1b[?1003h"] {
            assert!(
                !none.contains(code),
                "no mouse flag set but emitted {code:?}: {none:?}"
            );
        }

        // SGR is an independent encoding bit, not a fourth protocol
        // state: it rides along with whichever protocol was selected.
        let standard_plus_sgr = PaneModes::parse("0,0,0,0,1,1,1,0,0,0").post_content_sequences();
        assert!(standard_plus_sgr.contains("\x1b[?1000h"));
        assert!(standard_plus_sgr.contains("\x1b[?1006h"));
        assert!(!standard_plus_sgr.contains("\x1b[?1003h"));
    }

    /// A pane on the normal screen must not emit the alt-screen switch —
    /// doing so would blank a perfectly good replay.
    #[test]
    fn normal_screen_emits_no_alt_switch() {
        let modes = PaneModes {
            cursor_visible: true,
            ..Default::default()
        };
        assert_eq!(modes.pre_content_sequences(), "");
    }

    /// Field parsing must survive a tmux build that does not know one of
    /// the format names. With whitespace splitting, the empty expansion
    /// collapsed and every later field shifted left — restoring wrong
    /// modes and misplacing the cursor. Comma-delimited fields degrade
    /// one position at a time, which is the documented contract.
    #[test]
    fn unknown_format_degrades_only_its_own_field() {
        // bracket_paste (field 2) unknown; everything after it must keep
        // its own value.
        let modes = PaneModes::parse("1,,0,0,0,1,1,0,7,3");
        assert!(modes.alternate_on);
        assert!(!modes.bracket_paste, "unknown field reads as off");
        assert!(modes.mouse_sgr, "field after the gap keeps its own value");
        assert!(modes.cursor_visible);
        assert_eq!((modes.cursor_x, modes.cursor_y), (7, 3));
    }

    /// Version ordering must separate a release from its own patch
    /// letters, which is the whole reason [`TmuxVersion`] exists: the
    /// check this replaced compared `(major, minor)` and could not tell
    /// the regression-tested 3.7c from a 3.7 nobody has run Farhelm on.
    ///
    /// Pins the two orderings that are easy to get wrong — a bare release
    /// sorts BEFORE its first patch letter (tmux shipped 3.7 then 3.7a),
    /// and the minor number is numeric, not textual, so 3.10 is newer
    /// than 3.9 rather than older.
    #[test]
    fn version_ordering_separates_patch_letters_and_compares_minors_numerically() {
        let parse = |line: &str| parse_tmux_version(line).expect("a well-formed version");
        assert!(parse("tmux 3.7") < parse("tmux 3.7a"));
        assert!(parse("tmux 3.7a") < parse("tmux 3.7b"));
        assert!(parse("tmux 3.7b") < parse("tmux 3.7c"));
        assert!(parse("tmux 3.7c") < parse("tmux 3.8"));
        assert!(parse("tmux 3.9") < parse("tmux 3.10"));
        assert!(parse("tmux 3.10") < parse("tmux 4.0"));
        assert_eq!(parse("tmux 3.7c"), TMUX_FLOOR);
    }

    /// Parsing is the trust boundary in front of the floor, so what it
    /// REFUSES matters as much as what it reads: every shape here would
    /// otherwise have to be guessed at, and a wrong guess admits an
    /// unaudited substrate. Also pins the round-trip through `Display`,
    /// since the refusal message and the release-pin lockstep test both
    /// compare printed versions.
    #[test]
    fn version_parsing_refuses_everything_it_cannot_read_exactly() {
        for line in ["tmux 3.7b", "tmux 3.7", "tmux 4.0", "tmux 3.10"] {
            let parsed = parse_tmux_version(line).expect("a well-formed version");
            assert_eq!(format!("tmux {parsed}"), line, "Display must round-trip");
        }
        // A trailing newline is the shape tmux actually emits, so it must
        // parse identically to the trimmed line.
        assert_eq!(
            parse_tmux_version("tmux 3.7c\n").expect("tmux's own output"),
            TMUX_FLOOR
        );
        // No version token at all, a development build, a decorated
        // distro string, and a suffix that is not tmux's single lowercase
        // patch letter.
        for line in [
            "",
            "tmux",
            "not-a-version",
            "tmux next-3.8",
            "tmux 3.7-rc",
            "tmux 3.7ab",
            "tmux 3.7B",
            "tmux 3",
            "tmux .7",
            "tmux 3.a",
        ] {
            assert!(
                parse_tmux_version(line).is_err(),
                "{line:?} must not parse as a version"
            );
        }
        // Whole-line shapes an earlier token-only parser read as clean
        // releases. Each one is a way an unaudited or forged substrate
        // could have been classified as the pinned build — the wrong
        // program answering, a vendor decoration, or a non-canonical
        // spelling that would then print back differently than it came
        // in. `tmux 3.8-rc` and `tmux next-3.8` above are the same
        // refusal by policy rather than by accident: see
        // [`parse_tmux_version`] on why tmux's official development and
        // release-candidate stages are out of contract.
        for line in [
            "not-tmux 3.7c",
            "TMUX 3.7c",
            "tmux3.7c",
            " tmux 3.7c",
            "tmux 3.7c vendor-patch",
            "tmux 3.7c\ntmux 3.6",
            "tmux 3.7c\n\n",
            "tmux 3.7c\r\n",
            "tmux 3.7c ",
            "tmux +3.7c",
            "tmux 3.+7c",
            "tmux -3.7c",
            "tmux 03.7c",
            "tmux 3.07c",
            "tmux 3.8-rc",
            "tmux 3.8-rc2",
        ] {
            assert!(
                parse_tmux_version(line).is_err(),
                "{line:?} must not parse as a version"
            );
        }
    }

    /// The floor refuses everything below the pinned build and accepts
    /// everything at or above it — including versions nobody has audited,
    /// which warn instead (a refusal there would strand users on the
    /// Homebrew release the project's own install instructions ask for).
    ///
    /// Malformed input stays a refusal: an unreadable version is an
    /// unknown version, and the conservative direction is the one that
    /// does not start a server.
    #[test]
    fn the_floor_refuses_older_builds_and_admits_newer_ones() {
        let check = |line: &str| require_supported_tmux(Path::new("/usr/bin/tmux"), line);
        assert!(check("tmux 3.7c").is_ok(), "the floor itself is accepted");
        assert!(
            check("tmux 3.8").is_ok(),
            "a newer release warns, not fails"
        );
        assert!(check("tmux 4.0").is_ok());
        assert!(
            check("tmux 3.7b").is_err(),
            "the previous pin is below the floor"
        );
        assert!(check("tmux 3.7a").is_err());
        assert!(check("tmux 3.7").is_err());
        assert!(check("tmux 3.6").is_err());
        assert!(check("tmux 3.4").is_err());
        assert!(check("tmux 3.3a").is_err());
        assert!(check("not-a-version").is_err());
    }

    /// The floor's THREE outcomes, pinned as values rather than as "the
    /// call returned Ok".
    ///
    /// The version this replaced asserted only success, so it stayed green
    /// if the newer-than-pinned warning were deleted, if it fired at the
    /// exact pinned version, or if a later patch letter of the pinned
    /// release were mishandled. Those are the three ways the policy can go
    /// wrong quietly: an unaudited substrate that says nothing, and noise
    /// on the one combination that is actually blessed. Classifying into
    /// an enum is what makes them assertable without installing a
    /// process-global tracing subscriber, which would collide with every
    /// other test in the binary.
    #[test]
    fn classification_is_silent_at_the_pin_and_flags_everything_newer() {
        let at = |line: &str| {
            classify_tmux_version(parse_tmux_version(line).expect("a well-formed version"))
        };
        assert_eq!(at("tmux 3.7c"), TmuxSupport::AtFloor);
        // The next patch letter of the SAME release is still not the
        // build the regression suites run against.
        assert_eq!(at("tmux 3.7d"), TmuxSupport::AbovePin);
        assert_eq!(at("tmux 3.8"), TmuxSupport::AbovePin);
        assert_eq!(at("tmux 4.0"), TmuxSupport::AbovePin);
        assert_eq!(at("tmux 3.7b"), TmuxSupport::BelowFloor);
        assert_eq!(at("tmux 3.6"), TmuxSupport::BelowFloor);
    }

    /// The refusal is the first thing a user hits on a distro tmux, and it
    /// is useless unless it answers "which binary, how old, and how old is
    /// too old" — the actual failure it exists for is "the wrong tmux was
    /// on PATH", which cannot be fixed without knowing which one answered.
    #[test]
    fn the_refusal_names_the_binary_the_version_and_the_floor() {
        let error = require_supported_tmux(Path::new("/usr/bin/tmux"), "tmux 3.6\n")
            .expect_err("3.6 is below the floor");
        let message = format!("{error:#}");
        assert!(message.contains("/usr/bin/tmux"), "{message}");
        assert!(message.contains("tmux 3.6"), "{message}");
        assert!(message.contains(&TMUX_FLOOR.to_string()), "{message}");
        assert!(message.contains("README"), "{message}");
    }

    /// `tmux_not_found_message` must tell the two "not found" shapes apart:
    /// a bare name failing means PATH search failed, and the fix is
    /// installing tmux somewhere on PATH (or setting `--tmux`/
    /// `FARHELM_TMUX`); a resolved path failing means the override itself
    /// is wrong, and the fix is fixing that override. Collapsing them into
    /// one wording (as the pre-fix `"checking the tmux version of tmux"`
    /// context did) reads like tmux ran and refused rather than like tmux
    /// was never found at all.
    #[test]
    fn not_found_wording_differs_for_a_bare_name_and_a_resolved_path() {
        let bare = tmux_not_found_message(Path::new("tmux"));
        assert!(bare.contains("PATH"), "{bare}");
        assert!(bare.contains('`'), "{bare}");

        let path = tmux_not_found_message(Path::new("/opt/nonexistent/tmux"));
        assert!(path.contains("/opt/nonexistent/tmux"), "{path}");
        // The path wording must not accuse PATH of being unset — an
        // explicit override was given and IT is what is wrong.
        assert!(!path.contains("PATH"), "{path}");
    }

    /// A bare name can be [`DEFAULT_TMUX_PROGRAM`]'s unconfigured fallback
    /// OR an explicit `--tmux custom-tmux` / `FARHELM_TMUX=custom-tmux`
    /// override — both spellings look identical by the time they reach
    /// `tmux_not_found_message`, which is exactly the provenance the
    /// function cannot see. The wording therefore has to work for BOTH
    /// without asserting which one happened: it must never claim
    /// `FARHELM_TMUX unset` for a name that may well have come FROM
    /// `FARHELM_TMUX`, which is what the previous wording did.
    #[test]
    fn a_bare_name_from_an_explicit_override_gets_the_same_source_neutral_wording() {
        let default_fallback = tmux_not_found_message(Path::new(DEFAULT_TMUX_PROGRAM));
        let explicit_override = tmux_not_found_message(Path::new("custom-tmux"));
        assert!(!default_fallback.contains("unset"), "{default_fallback}");
        assert!(!explicit_override.contains("unset"), "{explicit_override}");
        assert!(
            explicit_override.contains("custom-tmux"),
            "{explicit_override}"
        );
    }

    /// Every "not found" wording has to cover the Unix ambiguity in
    /// `ENOENT`: it also fires for an existing script whose shebang
    /// interpreter is gone, or a binary whose loader is missing, not only
    /// for a target that plain does not exist. A message that flatly said
    /// "not found" would send an operator chasing a reinstall when the
    /// real repair is restoring the interpreter or loader.
    #[test]
    fn not_found_wording_covers_a_missing_interpreter_or_loader_too() {
        for program in [Path::new("tmux"), Path::new("/opt/nonexistent/tmux")] {
            let message = tmux_not_found_message(program);
            assert!(message.contains("interpreter or loader"), "{message}");
        }
    }

    /// A version Farhelm cannot READ is as much a startup blocker as one
    /// that is too old, and it is a harder one to act on: the user sees a
    /// refusal for a tmux that looks fine to them. So the error chain has
    /// to carry both halves of the answer — the exact token that could
    /// not be parsed, and which binary produced it.
    ///
    /// Asserted against the full `{:#}` chain, not the outermost message,
    /// because the token and the program are contributed by different
    /// `context` layers; a refactor that drops either layer would leave an
    /// `is_err`-only test perfectly green.
    #[test]
    fn a_malformed_version_names_the_token_and_the_binary() {
        let error =
            require_supported_tmux(Path::new("/opt/weird/tmux"), "tmux 9.9zzz-vendor-mangled\n")
                .expect_err("an unreadable version must refuse");
        let message = format!("{error:#}");
        assert!(message.contains("9.9zzz-vendor-mangled"), "{message}");
        assert!(message.contains("/opt/weird/tmux"), "{message}");
    }

    /// The floor and the release pin must not be able to drift apart.
    ///
    /// [`TMUX_FLOOR`] means "the version Farhelm's regression suites
    /// actually run against", and the build that gets regression-tested is
    /// the one `.github/release/source-pins.env` names. Nothing else
    /// couples the two files, so bumping the pinned tarball without
    /// bumping the floor would silently leave the floor pointing at a
    /// build the project no longer ships — and bumping the floor alone
    /// would refuse the build provisioning installs. Failing here is the
    /// intended way to learn that the other half of the bump is missing.
    ///
    /// EVERY assignment is collected and exactly one is required, rather
    /// than reading the first. The pin file is SOURCED by shell scripts,
    /// where the LAST assignment wins, so an old line left above a new one
    /// would leave this test comparing the floor against a version nothing
    /// actually builds — the precise drift it exists to catch, dressed as
    /// a pass.
    #[test]
    fn floor_and_release_pin_cannot_drift() {
        // Reaching outside the crate with a relative `include_str!` is
        // fine here: this is a workspace-only repository, never packaged
        // or published as a standalone crate.
        const SOURCE_PINS: &str = include_str!("../../../.github/release/source-pins.env");
        let pinned: Vec<&str> = SOURCE_PINS
            .lines()
            .filter_map(|line| line.trim().strip_prefix("TMUX_VERSION="))
            .collect();
        assert_eq!(
            pinned.len(),
            1,
            "source-pins.env must declare TMUX_VERSION exactly once, found {pinned:?}"
        );
        assert_eq!(
            pinned[0],
            TMUX_FLOOR.to_string(),
            "TMUX_FLOOR and the pinned tmux release must be bumped together"
        );
    }

    /// Write one executable shell script fixture and hand back its path.
    ///
    /// `farhelm helm setup` points [`probe_tmux`] at whatever the operator
    /// named or `PATH` produced, so the probe's behaviour against programs
    /// that are NOT tmux is production behaviour and needs real child
    /// processes to exercise.
    #[cfg(unix)]
    fn probe_fixture(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;

        let path = dir.join(name);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o755)
            .open(&path)
            .expect("fixture script");
        // The warm-up guard is the first thing the script does, so the
        // ETXTBSY check below costs nothing even for a fixture whose whole
        // point is to sleep for two minutes or flood its stdout.
        write!(
            file,
            "#!/bin/sh\ncase \"$1\" in --probe-warmup) exit 0;; esac\n{body}\n"
        )
        .expect("write fixture");
        drop(file);
        // Exec the fresh script once before returning it: writing an
        // executable and immediately spawning it races ETXTBSY against any
        // other thread's fork window, and every caller here spawns.
        for _ in 0..200 {
            match std::process::Command::new(&path)
                .arg("--probe-warmup")
                .output()
            {
                Ok(_) => return path,
                Err(error) if error.raw_os_error() == Some(26) => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(error) => panic!("fixture {name} is not runnable: {error}"),
            }
        }
        panic!("fixture {name} stayed busy");
    }

    /// The probe is pointed at operator-supplied and PATH-supplied
    /// executables, so a candidate that hangs or floods must produce a
    /// refusal rather than wedging or killing the CLI. Both budgets are
    /// exercised with real children.
    #[cfg(unix)]
    #[test]
    fn a_candidate_that_hangs_or_floods_is_refused_within_the_budget() {
        let dir = tempfile::tempdir().expect("fixture dir");

        // Sleeps well past the deadline. The probe must return early, and
        // the elapsed time proves it did not simply wait for the child.
        let hanging = probe_fixture(dir.path(), "hangs", "sleep 120");
        let started = std::time::Instant::now();
        let error = probe_tmux(&hanging).expect_err("a hanging candidate is not an answer");
        assert!(matches!(error, TmuxProbeError::Overran(_)), "{error:?}");
        assert!(error.to_string().contains("did not answer -V"), "{error}");
        assert!(
            started.elapsed() < PROBE_DEADLINE + Duration::from_secs(5),
            "the probe waited {:?}, past its own deadline",
            started.elapsed()
        );

        // Writes far more than the capture limit. `head` keeps the fixture
        // from depending on how the probe closes its end of the pipe.
        let flooding = probe_fixture(dir.path(), "floods", "yes farhelm-flood | head -c 200000");
        let error = probe_tmux(&flooding).expect_err("a flood is not a version");
        assert!(matches!(error, TmuxProbeError::Overran(_)), "{error:?}");
        assert!(error.to_string().contains("more than"), "{error}");
    }

    /// The probe must not leave anything of its own running.
    ///
    /// The nastiest shape is not a slow candidate but a fast one that
    /// FORKS: the leader prints a perfectly good version line and exits,
    /// while the descendant it left behind holds the inherited stdout open
    /// for two minutes. The bounded collection alone would return an
    /// overrun and walk away from both the descendant and the reader
    /// thread blocked on that pipe, so a machine that probes such a
    /// candidate repeatedly accumulates one of each per refusal.
    ///
    /// The whole probe therefore runs in its own process group, and an
    /// overrun kills the group. This proves it: the descendant records its
    /// pid, and after the probe returns that pid is gone.
    #[cfg(unix)]
    #[test]
    fn a_descendant_holding_the_pipes_open_is_killed_with_the_group() {
        let dir = tempfile::tempdir().expect("fixture dir");
        let pidfile = dir.path().join("grandchild.pid");
        // The `sleep` inherits stdout; the leader records its pid, prints
        // a version, and exits immediately.
        let forking = probe_fixture(
            dir.path(),
            "forks",
            &format!(
                "sleep 120 & printf '%s\\n' \"$!\" > {}; printf 'tmux {TMUX_FLOOR}\\n'",
                pidfile.display()
            ),
        );

        let started = std::time::Instant::now();
        let error = probe_tmux(&forking).expect_err("a held-open pipe is not an answer");
        assert!(matches!(error, TmuxProbeError::Overran(_)), "{error:?}");
        assert!(error.to_string().contains("still held open"), "{error}");
        assert!(
            started.elapsed() < PROBE_DEADLINE,
            "collection must be bounded by the drain budget, not the deadline: {:?}",
            started.elapsed()
        );

        let pid: i32 = std::fs::read_to_string(&pidfile)
            .expect("the fixture recorded its descendant")
            .trim()
            .parse()
            .expect("a pid");
        // Reparenting and reaping are not instant, so allow a moment for
        // the kill to be collected — but only a moment: the point is that
        // the process is gone rather than merely doomed.
        let gone = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            // SAFETY: signal 0 performs the permission and existence check
            // without delivering anything.
            let alive = unsafe { libc::kill(pid, 0) } == 0;
            if !alive {
                break;
            }
            assert!(
                std::time::Instant::now() < gone,
                "the descendant holding the probe's stdout survived the probe"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The probe's ordinary verdicts, each with the payload a refusal
    /// message needs: which binary answered, what it printed, and where
    /// its version sits against the floor.
    #[cfg(unix)]
    #[test]
    fn a_probe_reports_the_program_version_and_what_a_non_tmux_printed() {
        let dir = tempfile::tempdir().expect("fixture dir");
        let root = dir.path().to_path_buf();

        let at_floor = probe_fixture(&root, "at-floor", &format!("printf 'tmux {TMUX_FLOOR}\\n'"));
        let probe = probe_tmux(&at_floor).expect("a floor build is an answer");
        assert_eq!(probe.program, at_floor);
        assert_eq!(probe.version, TMUX_FLOOR);
        assert_eq!(probe.support, TmuxSupport::AtFloor);

        let old = probe_fixture(&root, "old", "printf 'tmux 3.4\\n'");
        let probe = probe_tmux(&old).expect("an old build still answers");
        assert_eq!(probe.support, TmuxSupport::BelowFloor);
        assert_eq!(probe.version.to_string(), "3.4");

        // A non-zero exit reports STDERR, which is where a program that is
        // not tmux says what it is.
        let failing = probe_fixture(&root, "failing", "echo 'not a multiplexer' >&2; exit 3");
        let error = probe_tmux(&failing).expect_err("a failing candidate is not a version");
        assert!(
            matches!(&error, TmuxProbeError::Unparseable(printed) if printed == "not a multiplexer"),
            "{error:?}"
        );

        let decorated = probe_fixture(&root, "decorated", "printf 'tmux 3.7c vendor-patch\\n'");
        let error = probe_tmux(&decorated).expect_err("a decorated line is not a release");
        assert!(
            error.to_string().contains("tmux 3.7c vendor-patch"),
            "{error}"
        );

        assert!(matches!(
            probe_tmux(Path::new("/nonexistent/tmux")),
            Err(TmuxProbeError::NotRunnable(_))
        ));
    }

    /// PATH discovery must behave like `execvp`: an entry that merely
    /// LOOKS executable but cannot be spawned is skipped, not fatal. A
    /// `noexec` mount or a group-only execute bit produces exactly that
    /// shape, and stopping there would hide a perfectly good tmux later on
    /// PATH. Empty entries are dropped as well — the current directory is
    /// not something to pin into a unit file.
    #[cfg(unix)]
    #[test]
    fn path_candidates_are_offered_in_order_without_empty_entries() {
        let dir = tempfile::tempdir().expect("fixture dir");
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        // Looks executable, is not: a directory-less shebang is the
        // cheapest reproduction of "spawns and fails".
        probe_fixture(&first, "tmux", "true");
        std::fs::write(first.join("tmux"), b"#!/nonexistent/interpreter\n").unwrap();
        let usable = probe_fixture(&second, "tmux", &format!("printf 'tmux {TMUX_FLOOR}\\n'"));

        let path = std::env::join_paths([std::path::PathBuf::new(), first.clone(), second.clone()])
            .unwrap();
        let candidates: Vec<_> = candidates_on_path(&path, "tmux").collect();
        assert_eq!(candidates, [first.join("tmux"), usable.clone()]);
        assert_eq!(find_on_path(&path, "tmux"), Some(first.join("tmux")));
        // The first candidate is the one `find_on_path` reports and the
        // one a spawning caller must be able to skip.
        assert!(matches!(
            probe_tmux(&candidates[0]),
            Err(TmuxProbeError::NotRunnable(_))
        ));
        assert!(probe_tmux(&candidates[1]).is_ok());
    }

    /// The override's precedence is a user-facing contract: `--tmux` beats
    /// `FARHELM_TMUX` beats PATH, so an operator can override a unit
    /// file's environment from the command line without editing it.
    ///
    /// Both inputs are injected rather than read from the process, which
    /// is deliberate — this repo's tests never mutate the test process's
    /// environment, and a shared `FARHELM_TMUX` would leak into every
    /// concurrently running harness.
    #[test]
    fn the_flag_beats_the_environment_which_beats_path() {
        use std::ffi::OsStr;

        let flag = PathBuf::from("/opt/flag/tmux");
        let env = OsStr::new("/opt/env/tmux");
        assert_eq!(
            resolve_tmux_program(Some(&flag), Some(env)),
            PathBuf::from("/opt/flag/tmux")
        );
        assert_eq!(
            resolve_tmux_program(Some(&flag), None),
            PathBuf::from("/opt/flag/tmux")
        );
        assert_eq!(
            resolve_tmux_program(None, Some(env)),
            PathBuf::from("/opt/env/tmux")
        );
        assert_eq!(
            resolve_tmux_program(None, None),
            PathBuf::from(DEFAULT_TMUX_PROGRAM)
        );
        // An empty override is what a unit file or profile writes to mean
        // "no override"; the other reading spawns a program with no name.
        assert_eq!(
            resolve_tmux_program(None, Some(OsStr::new(""))),
            PathBuf::from(DEFAULT_TMUX_PROGRAM)
        );
    }

    /// Write an executable shell script, for the tmux stand-ins below.
    ///
    /// Factored out because several override tests need a program that
    /// exists, runs, and answers in a controlled way; a non-executable
    /// file would be refused by the OS long before the behavior under
    /// test.
    #[cfg(unix)]
    fn write_executable(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, body).expect("write the stand-in");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .expect("make the stand-in executable");
    }

    /// A tmux stand-in that answers `-V` with `version` and REFUSES any
    /// other invocation with exit 2.
    ///
    /// The strictness is the point. A stand-in that printed a version for
    /// every invocation could not distinguish "the probe asked for `-V`"
    /// from "the probe asked for something else and we answered anyway",
    /// so the earlier version of these tests proved only that a refusal
    /// happened, not that the right question was asked. This one also
    /// pins the funnel: the probe is built by `TmuxDriver::command`, so
    /// the exact argument vector must be `-S <socket> -f <config> -V`, and
    /// an invocation that lost the private isolation flags — the one
    /// mistake this module's whole `command()` discipline exists to
    /// prevent — fails here rather than silently touching a real server.
    #[cfg(unix)]
    fn write_version_standin(path: &Path, version: &str) {
        write_executable(
            path,
            &format!(
                "#!/bin/sh\n\
                 if [ \"$#\" -eq 5 ] && [ \"$1\" = -S ] && [ \"$3\" = -f ] && [ \"$5\" = -V ]; \
                 then echo '{version}'; exit 0; fi\n\
                 echo \"unexpected invocation: $*\" >&2\n\
                 exit 2\n"
            ),
        );
    }

    /// The override has to reach the FLOOR PROBE, not just the commands
    /// after it. A driver that checked whatever `tmux` PATH resolves to
    /// and then ran the overridden binary would be strictly worse than no
    /// override at all — it would clear the floor against one program and
    /// drive another — so this drives `ensure_server` through a stand-in
    /// binary that reports a too-old version and proves the refusal both
    /// happens and names that binary.
    ///
    /// The stand-in is a shell script rather than a real tmux because the
    /// probe under test is a `-V` exchange: nothing here needs a server,
    /// and a test that needed one could not run where the floor is unmet.
    #[cfg(unix)]
    #[tokio::test]
    async fn the_overridden_binary_is_the_one_the_floor_probe_checks() {
        let dir = tempfile::tempdir().expect("tempdir");
        let fake = dir.path().join("pretend-tmux");
        write_version_standin(&fake, "tmux 3.6");

        let driver = TmuxDriver::new_with_program(dir.path(), TmuxBudgets::default(), fake.clone());
        let error = driver
            .ensure_server()
            .await
            .expect_err("a 3.6 substrate is below the floor");
        let message = format!("{error:#}");
        assert!(message.contains(&fake.display().to_string()), "{message}");
        assert!(message.contains("tmux 3.6"), "{message}");
        assert!(message.contains(&TMUX_FLOOR.to_string()), "{message}");
        assert!(
            !driver.config.exists(),
            "a refused floor check must not have written a server config"
        );
    }

    /// The override must also survive the ORDINARY command path, which the
    /// floor-refusal tests never reach: they fail before the funnel runs.
    ///
    /// Two properties in one place because they are the two halves of the
    /// same promise. The chosen program is what gets spawned — otherwise
    /// the floor is enforced against one binary and the sessions run on
    /// another — and it does NOT displace the private `-S`/`-f`
    /// arguments, because an override that quietly dropped those would
    /// point Farhelm at the user's own tmux server and configuration.
    #[test]
    fn an_overridden_driver_keeps_the_program_and_the_private_arguments() {
        let dir = tempfile::tempdir().expect("tempdir");
        let fake = dir.path().join("pretend-tmux");
        let driver = TmuxDriver::new_with_program(dir.path(), TmuxBudgets::default(), fake.clone());

        let command = driver.command();
        let command = command.as_std();
        assert_eq!(Path::new(command.get_program()), fake);
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            args,
            vec![
                std::ffi::OsStr::new("-S"),
                driver.socket.as_os_str(),
                std::ffi::OsStr::new("-f"),
                driver.config.as_os_str(),
            ]
        );
    }

    /// A tmux the driver cannot even SPAWN must refuse by name and leave
    /// no trace, which is the ordinary shape of a mistyped `--tmux` or a
    /// unit file naming a path that a later upgrade removed.
    ///
    /// Pinned against the COMPLETE `tmux_not_found_message` wording, and
    /// against the stale `"checking the tmux version of ..."` context it
    /// replaced, rather than merely checking that the path appears
    /// somewhere in the error: that old context also contained the path,
    /// so a driver that quietly stopped calling the helper for this
    /// branch would still have passed a looser assertion here.
    ///
    /// Pinning "no config written" alongside the message matters because
    /// the config write sits between the probe and `start-server`: a
    /// reordering that wrote it first would leave a stale file behind on
    /// every failed startup, in a state directory the next supervisor
    /// reads.
    #[tokio::test]
    async fn a_tmux_that_cannot_be_spawned_refuses_by_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        let absent = dir.path().join("no-such-tmux");

        let driver =
            TmuxDriver::new_with_program(dir.path(), TmuxBudgets::default(), absent.clone());
        let error = driver
            .ensure_server()
            .await
            .expect_err("a program that does not exist cannot be driven");
        let message = format!("{error:#}");
        assert!(
            message.contains(&tmux_not_found_message(&absent)),
            "{message}"
        );
        assert!(
            !message.contains("checking the tmux version of"),
            "the stale pre-fix context must not reappear: {message}"
        );
        assert!(
            !driver.config.exists(),
            "a failed probe must not have written a server config"
        );
    }

    /// The same production branch, exercised through a BARE name rather
    /// than a resolved path. The two spellings render different wording
    /// (see `tmux_not_found_message`'s doc comment), and before this test
    /// only a resolved-path fixture existed — which meant a driver change
    /// that stopped routing a bare-name `NotFound` through the helper
    /// could still pass every `ensure_server` test in this module.
    ///
    /// The chosen name is implausible on any real `PATH` rather than
    /// engineered to be absent by mutating this test process's own
    /// environment: `Command` resolves a bare program against whatever
    /// `PATH` this process already has, and a sufficiently unlikely name
    /// fails to resolve the same way a genuinely absent one would.
    #[tokio::test]
    async fn a_missing_bare_program_name_refuses_through_the_same_helper() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bare = PathBuf::from("farhelm-test-nonexistent-tmux-4f19c2");

        let driver = TmuxDriver::new_with_program(dir.path(), TmuxBudgets::default(), bare.clone());
        let error = driver
            .ensure_server()
            .await
            .expect_err("a bare name absent from PATH cannot be driven");
        let message = format!("{error:#}");
        assert!(
            message.contains(&tmux_not_found_message(&bare)),
            "{message}"
        );
        assert!(
            !message.contains("checking the tmux version of"),
            "{message}"
        );
    }

    /// A tmux that runs but FAILS its `-V` is a different diagnostic from
    /// one that cannot be spawned, and the thing that makes it actionable
    /// is the program's own stderr — a missing shared library, a wrapper
    /// script refusing, a binary for the wrong architecture. Losing that
    /// text leaves "tmux -V failed" and nothing to act on, so the exit
    /// status and the stderr are both pinned.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_failing_version_probe_surfaces_its_status_and_stderr() {
        let dir = tempfile::tempdir().expect("tempdir");
        let broken = dir.path().join("broken-tmux");
        write_executable(
            &broken,
            "#!/bin/sh\necho 'libevent.so.2: cannot open shared object file' >&2\nexit 1\n",
        );

        let driver =
            TmuxDriver::new_with_program(dir.path(), TmuxBudgets::default(), broken.clone());
        let error = driver
            .ensure_server()
            .await
            .expect_err("a nonzero -V cannot be treated as a version");
        let message = format!("{error:#}");
        assert!(message.contains(&broken.display().to_string()), "{message}");
        assert!(message.contains("exit status: 1"), "{message}");
        assert!(
            message.contains("libevent.so.2: cannot open shared object file"),
            "{message}"
        );
        assert!(
            !driver.config.exists(),
            "a failed probe must not have written a server config"
        );
    }

    /// The refusal names a resolvable path even when the program was a
    /// bare name off PATH — which is the case that needs it most, since
    /// "some tmux on PATH is too old" leaves the reader with nothing to
    /// edit. A name that already carries a separator is reported as
    /// written, and an unresolvable one falls back to the bare spelling
    /// rather than inventing a path that does not exist.
    ///
    /// The shadow case is the one with teeth: `execvp` walks PAST a
    /// non-executable `tmux` and runs a later entry, so a lookup that
    /// stopped at the first file by name would print a path that never
    /// ran. That mismatch turns the diagnostic from helpful into
    /// actively misleading — the reader edits or deletes a file that had
    /// nothing to do with the version they were refused for.
    ///
    /// Unix-only: the shadow it pins is an execute-bit distinction, which
    /// [`is_executable_file`] cannot make on a platform without one.
    #[cfg(unix)]
    #[test]
    fn a_bare_program_name_is_reported_as_the_path_entry_it_resolves_to() {
        use std::ffi::OsString;

        let shadow = tempfile::tempdir().expect("tempdir");
        let dir = tempfile::tempdir().expect("tempdir");
        // Same name, earlier on PATH, not executable: the OS skips it.
        std::fs::write(shadow.path().join("tmux"), b"not a program\n")
            .expect("write the non-executable shadow");
        let binary = dir.path().join("tmux");
        write_executable(&binary, "#!/bin/sh\n");
        let path_var =
            std::env::join_paths([shadow.path(), dir.path()]).expect("a joinable PATH fixture");

        assert_eq!(
            program_display_path(Path::new("tmux"), Some(&path_var)),
            binary,
            "a non-executable shadow must not be reported as the tmux that answered"
        );
        assert_eq!(
            program_display_path(Path::new("/opt/homebrew/bin/tmux"), Some(&path_var)),
            PathBuf::from("/opt/homebrew/bin/tmux")
        );
        assert_eq!(
            program_display_path(Path::new("absent-tmux"), Some(&path_var)),
            PathBuf::from("absent-tmux")
        );
        assert_eq!(
            program_display_path(Path::new("tmux"), None),
            PathBuf::from("tmux")
        );
        // A PATH holding only the shadow resolves to nothing, and the
        // bare spelling is a better answer than a path that was skipped.
        let shadow_only = OsString::from(shadow.path());
        assert_eq!(
            program_display_path(Path::new("tmux"), Some(&shadow_only)),
            PathBuf::from("tmux")
        );
    }

    /// Pin every generated option and its table, while excluding the
    /// `window-size` setting that crashes tmux 3.4.
    ///
    /// Exact lines are the contract here: moving an option to a different
    /// tmux table can silently disable it while leaving a substring test
    /// green.
    #[test]
    fn generated_config_pins_every_load_bearing_option() {
        let cfg = TmuxDriver::config_body();
        assert_eq!(
            cfg,
            format!(
                "set -s exit-empty off\n\
                 set -s escape-time 0\n\
                 set -s default-terminal 'xterm-256color'\n\
                 set -g status off\n\
                 set -g prefix None\n\
                 set -g history-limit {HISTORY_LIMIT}\n\
                 setw -g remain-on-exit on\n"
            )
        );
    }

    /// Every executable `tmux` on `PATH`, paired with the version it
    /// reports.
    ///
    /// The adoption regression below needs two REAL tmux binaries of
    /// different versions, and it must stay CI-safe on a machine that has
    /// only one. Discovering them from `PATH` rather than from an
    /// environment variable or a hardcoded path is what makes that work
    /// without the test process mutating its own environment: CI already
    /// prepends the pinned build's directory to `PATH` while the distro's
    /// below-floor tmux stays at `/usr/bin`, so both ends of the scenario
    /// are present exactly where the suite already runs.
    #[cfg(unix)]
    fn tmux_binaries_on_path() -> Vec<(PathBuf, TmuxVersion)> {
        let Some(path_var) = std::env::var_os("PATH") else {
            return Vec::new();
        };
        let mut found: Vec<(PathBuf, TmuxVersion)> = Vec::new();
        for candidate in std::env::split_paths(&path_var).map(|dir| dir.join("tmux")) {
            if !is_executable_file(&candidate) || found.iter().any(|(seen, _)| seen == &candidate) {
                continue;
            }
            let Ok(output) = std::process::Command::new(&candidate).arg("-V").output() else {
                continue;
            };
            if !output.status.success() {
                continue;
            }
            if let Ok(version) = parse_tmux_version(&String::from_utf8_lossy(&output.stdout)) {
                found.push((candidate, version));
            }
        }
        found
    }

    /// Upgrading the tmux BINARY does not upgrade the tmux SERVER, and
    /// this is the test that keeps the floor honest about that.
    ///
    /// Farhelm drives a long-lived server through short-lived clients and
    /// deliberately keeps that server across supervisor restarts so
    /// sessions survive. `start-server` therefore ADOPTS whatever already
    /// owns the private socket. Without a server-side check, the ordinary
    /// upgrade path — install a floor-clearing tmux, restart the
    /// supervisor — clears the client probe and then runs every command
    /// against the old, crash-prone server it was supposed to replace.
    ///
    /// Three things are pinned, all of them regressions someone could
    /// reintroduce independently: the refusal HAPPENS, it names BOTH
    /// versions (the client's floor is meaningless without saying which
    /// server failed it), and the below-floor server is STILL RUNNING
    /// afterwards. The last one is not politeness — that server holds the
    /// user's live sessions, and a version policy that reaps them is worse
    /// than the problem it solves.
    ///
    /// Skips loudly, not silently, when `PATH` cannot supply one tmux on
    /// each side of the floor: a green run that never exercised adoption
    /// would be exactly the false assurance this test exists to remove.
    #[cfg(unix)]
    #[tokio::test]
    async fn an_adopted_below_floor_server_is_refused_without_being_killed() {
        let binaries = tmux_binaries_on_path();
        let old = binaries
            .iter()
            .find(|(_, version)| *version < TMUX_FLOOR)
            .cloned();
        let new = binaries
            .iter()
            .find(|(_, version)| *version >= TMUX_FLOOR)
            .cloned();
        let (Some((old, old_version)), Some((new, _))) = (old, new) else {
            println!(
                "SKIPPED an_adopted_below_floor_server_is_refused_without_being_killed: PATH \
                 needs both a tmux below {TMUX_FLOOR} and one at or above it; found {binaries:?}"
            );
            return;
        };

        let dir = tempfile::tempdir().expect("tempdir");
        let driver = TmuxDriver::new_with_program(dir.path(), TmuxBudgets::default(), new);
        // The same config the driver would write, so the pre-existing
        // server is the one `ensure_server` would have produced on this
        // socket — including `exit-empty off`, without which a
        // session-less server exits immediately and there is nothing to
        // adopt.
        std::fs::write(&driver.config, TmuxDriver::config_body()).expect("seed the server config");
        let started = std::process::Command::new(&old)
            .arg("-S")
            .arg(&driver.socket)
            .arg("-f")
            .arg(&driver.config)
            .arg("start-server")
            .status()
            .expect("spawn the below-floor tmux");
        assert!(started.success(), "the below-floor server must start");

        let error = driver
            .ensure_server()
            .await
            .expect_err("adopting a below-floor server must refuse");
        let message = format!("{error:#}");
        assert!(
            message.contains(&old_version.to_string()),
            "the refusal must name the server's version: {message}"
        );
        assert!(
            message.contains(&TMUX_FLOOR.to_string()),
            "the refusal must name the floor: {message}"
        );
        assert!(
            message.contains(&driver.socket.display().to_string()),
            "the refusal must name the socket to drain: {message}"
        );

        // Probed with the OLD binary: the point is that the server the
        // user's sessions live on survived the refusal untouched.
        let alive = std::process::Command::new(&old)
            .arg("-S")
            .arg(&driver.socket)
            .arg("-f")
            .arg(&driver.config)
            .args(["display-message", "-p", "#{version}"])
            .output()
            .expect("probe the adopted server");
        assert!(
            alive.status.success(),
            "the refused server must still be running: {}",
            String::from_utf8_lossy(&alive.stderr)
        );

        let _ = std::process::Command::new(&old)
            .arg("-S")
            .arg(&driver.socket)
            .arg("kill-server")
            .status();
    }

    /// Item 8: the generated-config write must be injectable through its
    /// REAL production call site (`ensure_server`), not only via a
    /// synthetic call directly into `crate::files`. A seam that fails the
    /// write step must surface through `ensure_server_with_seam`'s
    /// returned error, and — since the failure happens before `rename` —
    /// must never leave a config file behind at all.
    #[tokio::test]
    async fn ensure_server_with_seam_surfaces_an_injected_config_write_failure() {
        #[derive(Clone, Copy)]
        struct FailWrite;
        impl crate::files::FaultSeam for FailWrite {
            fn write(&self, _file: &mut std::fs::File, _bytes: &[u8]) -> std::io::Result<()> {
                Err(std::io::Error::other("injected config write failure"))
            }
        }

        let dir = tempfile::tempdir().expect("tempdir");
        let driver = TmuxDriver::new(dir.path());

        let err = driver
            .ensure_server_with_seam(FailWrite)
            .await
            .expect_err("an injected write failure must propagate");
        assert!(
            format!("{err:#}").contains("injected config write failure"),
            "the injected failure must be visible in the error: {err:#}"
        );
        assert!(
            !driver.config.exists(),
            "a failed config write must never publish a partial file"
        );
    }

    /// The replay-content normalization was a live bug (lore/: replay
    /// scrolled one row past the content, destroying the alt-screen top
    /// row), so its two transforms are pinned: LF→CRLF, and the trailing
    /// terminator dropped.
    #[test]
    fn capture_normalization_converts_line_endings_and_drops_the_last() {
        assert_eq!(normalize_capture(b"a\nb\n"), b"a\r\nb");
        assert_eq!(normalize_capture(b"a\nb"), b"a\r\nb");
        assert_eq!(normalize_capture(b"\n"), b"");
        assert_eq!(normalize_capture(b""), b"");
        // Escape sequences and blank interior lines pass through.
        assert_eq!(
            normalize_capture(b"\x1b[1mx\x1b[0m\n\ny\n"),
            b"\x1b[1mx\x1b[0m\r\n\r\ny"
        );
        // Non-UTF-8 pane content must survive byte-for-byte: replay goes
        // through here, and lossy decoding would render a `cat` of a
        // binary file differently after a reattach than it did live.
        assert_eq!(
            normalize_capture(&[0xff, 0xfe, b'\n', 0x80, b'\n']),
            vec![0xff, 0xfe, b'\r', b'\n', 0x80]
        );
    }

    /// A short or empty expansion must not panic or produce garbage.
    #[test]
    fn truncated_format_output_uses_defaults() {
        let modes = PaneModes::parse("");
        assert_eq!(modes, PaneModes::parse("0"));
        // Cursor visibility defaults on: hiding a cursor we know nothing
        // about would be a visible regression on every reattach.
        assert!(PaneModes::parse("").cursor_visible);
    }

    /// Pins `pane_states`'s diagnostic classification
    /// (`is_tolerated_list_panes_diagnostic`) directly against tmux's OWN
    /// raw stderr — not a rendered error string — since provoking every
    /// case against a real tmux server would require either killing one
    /// mid-test (slow, and already covered end to end by the e2e
    /// `list_sessions_survives_when_the_tmux_server_is_gone` test) or
    /// racing a `kill-server` closely enough to hit the mid-teardown
    /// shape reliably. Covers all three tolerated diagnostics — a
    /// genuinely empty server, a genuinely ABSENT one (the behavior this
    /// change adds), and one caught mid-teardown (ALSO new) — plus a
    /// plain unclassified failure.
    #[test]
    fn is_tolerated_list_panes_diagnostic_pins_all_three_tolerated_cases() {
        let socket = Path::new("/tmp/fh/tmux.sock");
        assert!(
            is_tolerated_list_panes_diagnostic("no current target", socket),
            "a genuinely empty server must tolerate"
        );
        assert!(
            is_tolerated_list_panes_diagnostic("no server running on /tmp/fh/tmux.sock", socket),
            "a genuinely absent server must ALSO tolerate — the behavior this change adds"
        );
        assert!(
            is_tolerated_list_panes_diagnostic("server exited unexpectedly", socket),
            "a server caught mid-teardown must ALSO tolerate — the same 'no panes exist' fact"
        );
        assert!(
            !is_tolerated_list_panes_diagnostic("unexpected tmux failure", socket),
            "an unclassified failure must not be laundered into an empty (all-exited) map"
        );
    }

    /// The anchoring this classifier exists for, pinned directly: a
    /// caller-controlled socket PATH that happens to CONTAIN the tolerated
    /// "no server running" phrase must not make an unrelated failure
    /// mentioning that same path look tolerated. An indiscriminate
    /// `stderr.contains(diagnostic)` classifier — the bug this test would
    /// have missed entirely, since its own inputs never embedded the
    /// phrase anywhere but at the front — passes this exact string (it
    /// DOES contain "no server running"), so asserting `!tolerated` here
    /// only means something because the phrase is genuinely present, just
    /// not as the whole, anchored message tmux actually emits for that
    /// diagnostic.
    #[test]
    fn is_tolerated_list_panes_diagnostic_rejects_a_path_that_merely_contains_a_tolerated_phrase() {
        let socket = Path::new("/tmp/no server running/tmux.sock");
        let unrelated_failure = "can't stat socket /tmp/no server running/tmux.sock: \
                                  Permission denied";
        assert!(
            unrelated_failure.contains("no server running"),
            "test premise: the unrelated message must genuinely contain the tolerated phrase, \
             or this test is not exercising the anchoring bug it claims to"
        );
        assert!(
            !is_tolerated_list_panes_diagnostic(unrelated_failure, socket),
            "a permission failure that merely MENTIONS a path containing the phrase must still \
             propagate as a real error, not be folded into an empty (all-exited) map"
        );
    }

    /// A dead pane's last words must survive the padding a pane comes
    /// with and the cap an error message imposes — keeping the TAIL,
    /// because the complaint a failing shell makes comes after whatever
    /// noise preceded it, and on a character boundary, because the result
    /// travels as a `String` on the wire.
    #[test]
    fn last_words_trims_padding_and_keeps_the_tail_within_the_cap() {
        assert_eq!(
            last_words("SHELL-REFUSED\n\n\n   \n", 1024),
            "SHELL-REFUSED",
            "the blank rows a pane is padded to its full height with are not output"
        );
        assert_eq!(
            last_words("noise\nthe real complaint", 15),
            "the real complaint"[3..],
            "an over-cap transcript keeps its END, not its beginning"
        );
        // A cap landing mid-character must not split it: the caller puts
        // this straight into a protocol error message.
        let multibyte = "aaaa\u{00e9}\u{00e9}\u{00e9}";
        let cut = last_words(multibyte, 5);
        assert!(
            multibyte.ends_with(&cut) && cut.len() <= 5,
            "expected a valid suffix within the cap, got {cut:?}"
        );
        assert_eq!(last_words("   \n\n", 1024), "");
    }

    /// The cross-session guarantee `pane_in_session` actually delivers,
    /// against a real tmux: a window-scoped command aimed at a pane that
    /// belongs to a DIFFERENT session is refused outright.
    ///
    /// Pane ids come from a server-wide counter that restarts at `%0`
    /// with the server, so a handle carried across a tmux restart can name
    /// a live pane of an unrelated session — and `resize-window` would
    /// then reflow a stranger's terminal. tmux refuses the mismatched
    /// pairing itself, which makes the check atomic with the act rather
    /// than a probe that can go stale; this pins that it really does.
    ///
    /// The complement is pinned in the same test, because it is the half
    /// that surprised us: the same target for a pane that no longer
    /// exists at all does NOT fail — the empty window component falls back
    /// to the session's current window — so this form means "never another
    /// session", not "exactly this pane". See `pane_in_session`'s docs for
    /// what that costs and which callers must therefore hold their pane
    /// still.
    #[tokio::test]
    async fn a_window_command_is_refused_for_another_sessions_pane() {
        let server = ScratchServer::start().await;
        let mine = server
            .driver
            .create_session("mine", "/", 80, 24, &[], &["sleep".into(), "60".into()])
            .await
            .expect("create the owning session");
        let theirs = server
            .driver
            .create_session("theirs", "/", 80, 24, &[], &["sleep".into(), "60".into()])
            .await
            .expect("create the other session");

        server
            .driver
            .resize_window("mine", &mine, 100, 30)
            .await
            .expect("a pane paired with its own session must resize");
        let refused = server
            .driver
            .resize_window("mine", &theirs, 40, 10)
            .await
            .expect_err("another session's pane must be refused, not silently resized");
        assert!(
            format!("{refused:#}").contains("can't find pane"),
            "expected tmux's own refusal, got: {refused:#}"
        );

        // The other session's geometry is untouched — the refusal is a
        // refusal, not a partially applied command.
        let geometry = server
            .driver
            .run(&[
                "display-message",
                "-p",
                "-t",
                &theirs,
                "#{window_width}x#{window_height}",
            ])
            .await
            .expect("querying the other session's geometry");
        assert_eq!(geometry.trim(), "80x24");
    }

    /// `capture_pane_tail` reads the VISIBLE grid and stops there, while
    /// `capture_pane_text` reaches back into scrollback.
    ///
    /// The distinction is the whole reason the two exist separately, and
    /// it is invisible in any single capture: both return plausible prose.
    /// It matters because the status sampler compares consecutive tails to
    /// decide whether output moved — a capture that dragged scrollback in
    /// would return a strictly growing string, so every session would look
    /// permanently busy — and because the per-kind sharpeners must match a
    /// prompt that is on screen NOW, not one answered a hundred lines ago.
    ///
    /// Adding `-S` back to the tail capture makes this fail on its first
    /// assertion, which is precisely the regression worth catching.
    #[tokio::test]
    async fn capture_pane_tail_reads_the_visible_grid_while_last_words_reaches_scrollback() {
        let server = ScratchServer::start().await;
        // The marker is printed first and then pushed off a 24-row screen
        // by 40 further lines — far enough to leave the visible grid, near
        // enough to stay inside `LAST_WORDS_LINES` of history.
        let argv = [
            "sh".to_string(),
            "-c".to_string(),
            "echo SCROLLED-AWAY-MARKER; i=1; while [ $i -le 40 ]; do echo \"line $i\"; \
             i=$((i+1)); done; sleep 300"
                .to_string(),
        ];
        let pane = server
            .driver
            .create_session("fh-scroll", "/", 80, 24, &[], &argv)
            .await
            .expect("create the fixture session");

        let tail = tail_containing(&server.driver, "fh-scroll", &pane, "line 40").await;
        assert!(
            !tail.contains("SCROLLED-AWAY-MARKER"),
            "the visible grid must not carry text that has scrolled into history:\n{tail}"
        );

        let last_words = server
            .driver
            .capture_pane_text("fh-scroll", &pane, 64 * 1024)
            .await
            .expect("capture the pane's last words");
        assert!(
            last_words.contains("SCROLLED-AWAY-MARKER"),
            "test premise: the marker must still be within scrollback reach, or the assertion \
             above proves nothing about WHERE the tail stopped:\n{last_words}"
        );
    }

    /// A pane that dies mid-sample yields an ERROR, never some other live
    /// pane's screen.
    ///
    /// The sampler addresses a pane it saw alive a moment earlier, so the
    /// window between the liveness probe and the capture is real and is
    /// hit whenever an agent exits under a poll. If tmux resolved an
    /// unknown pane id to the session's current pane instead of refusing,
    /// a session whose agent had just died would start reporting a
    /// TERMINAL TAB's activity as the agent's — a wrong status produced
    /// silently, and one that would survive every other test in this
    /// suite. Verified against tmux 3.7b; pinned here so a future version
    /// (or a future switch to a laxer target spelling) cannot change it
    /// quietly.
    #[tokio::test]
    async fn capture_pane_tail_refuses_a_dead_pane_rather_than_quoting_a_sibling() {
        let server = ScratchServer::start().await;
        let agent = server
            .driver
            .create_session(
                "fh-vanish",
                "/",
                80,
                24,
                &[],
                &["sh".to_string(), "-c".to_string(), "sleep 300".to_string()],
            )
            .await
            .expect("create the agent session");
        let (_window, tab) = server
            .driver
            .new_window(
                "fh-vanish",
                "/",
                &[],
                &[
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo TAB-PANE-TEXT; sleep 300".to_string(),
                ],
            )
            .await
            .expect("open a tab window in the same session");
        // The sibling must actually be rendering something distinctive, or
        // a misattribution would return an empty string and look like a
        // quiet agent rather than a wrong one.
        tail_containing(&server.driver, "fh-vanish", &tab, "TAB-PANE-TEXT").await;

        server
            .driver
            .kill_pane_for_test(&agent)
            .await
            .expect("kill the agent pane out from under the sampler");

        let refused = server
            .driver
            .capture_pane_tail("fh-vanish", &agent, 4096)
            .await;
        let error = format!(
            "{:#}",
            refused.expect_err("a vanished pane must not capture anything at all")
        );
        assert!(
            !error.contains("TAB-PANE-TEXT"),
            "the refusal must not carry the sibling's screen either: {error}"
        );
    }
}
