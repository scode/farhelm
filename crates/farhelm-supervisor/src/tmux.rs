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
mod snapshot;

pub use control_codec::unescape_control_output;
pub use snapshot::AltScreenCapture;
pub(crate) use snapshot::within_snapshot_cap;

use anyhow::{Context, bail};
use control_codec::{
    ControlLine, ControlMarker, PANE_FACT_FORMAT, PANE_MARKER_FORMAT, PassthroughDecoder,
    any_session_has_several_windows, classify_control_line, decode_output_payload,
    join_pane_markers, parse_control_marker, parse_pane_facts, read_command_block,
    read_control_line, strip_command_output_terminator, warn_once_about_missing_bracket_paste,
};
#[cfg(test)]
use control_codec::{read_control_line_before, tmux_ordinal};
use snapshot::parse_alt_screen_capture;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
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
/// [`attach_cutover_command`]).
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
/// group in [`OutputStream::snapshot_then_cutover`], so folding the dead
/// flag in here avoids that extra round trip entirely rather than merely
/// avoiding a second CONTROL-mode one. The dead flag is what the `Attach`
/// handler (service.rs) uses to decide whether to append the alt-screen
/// stop snapshot.
const PANE_MODE_FORMAT: &str = "#{alternate_on},#{bracket_paste_flag},#{mouse_all_flag},\
                                #{mouse_button_flag},#{mouse_standard_flag},#{mouse_sgr_flag},\
                                #{cursor_flag},#{keypad_cursor_flag},#{cursor_x},#{cursor_y},\
                                #{pane_dead}";

/// How long [`OutputStream::foreign_panes`] gives tmux to list a
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

/// The largest number of ` -A <pane>:off` pairs allowed to ride the
/// attach cutover; the rest follow it as their own commands.
///
/// tmux refuses a command line carrying on the order of a thousand
/// arguments ("command too long" — the same ceiling
/// [`InputClient::MAX_CHUNK`] is sized against), and each pane here costs
/// two argv entries on top of the cutover's own flags. A session with
/// hundreds of tabs is unusual but not forbidden, and the failure mode
/// would be the worst kind: attach refused outright, for a session whose
/// only sin was having many terminals. Chunking keeps the cutover a fixed,
/// safe size and pushes the remainder onto the post-cutover path, which
/// has no length pressure because it can use as many commands as it likes.
const MAX_CUTOVER_PANE_FILTERS: usize = 200;

/// Upper bound on [`OutputStream::silenced`], the per-attachment memo of
/// panes already filtered.
///
/// The memo exists only to keep a foreign pane's OUTPUT RATE from driving
/// a command rate (see that field), so losing it costs at most one extra
/// idempotent `refresh-client` per pane that speaks again — which is why a
/// long-lived attachment on a session churning through tabs may simply
/// forget everything and start over rather than grow without bound. 512 is
/// far above any real session's simultaneous pane count and still a
/// trivial amount of memory.
const MAX_SILENCED_PANES: usize = 512;

/// The command that lists every pane of `session`, one pane id per line,
/// asked over a control client rather than as a separate `tmux` process.
///
/// Used only by [`OutputStream::foreign_panes`], which needs the
/// answer on the very client it is about to configure — a separate
/// invocation would cost a process spawn to learn something the client's
/// own connection can answer inside the attach exchange it is already
/// having.
fn list_session_panes_command(session: &str) -> String {
    format!("list-panes -s -t \"={session}\" -F \"#{{pane_id}}\"")
}

/// `refresh-client` arguments that turn off a control client's delivery
/// for panes it does not speak for — one ` -A <pane>:off` per pane, tmux's
/// documented per-client pane filter.
///
/// Arguments rather than a whole command because at attach time the first
/// [`MAX_CUTOVER_PANE_FILTERS`] of them MUST share an invocation with the
/// `no-output` flip (see [`attach_cutover_command`]). Once output is live,
/// [`silence_live_pane_commands`] adds a preceding `pause` instead of using
/// these off-only arguments.
///
/// # Why this is safe only alongside [`SessionSink`]
///
/// tmux stops READING a pane whose every attached client has turned it
/// off — documented in `refresh-client`'s own man page, and confirmed
/// empirically (2026-08-02, 3.4 and 3.7b): with this as the only client,
/// turning a busy pane off froze that pane's producer for the entire
/// 10-second observation window. It is the sink — an attached client that
/// never turns any pane off and never falls behind — that keeps this from
/// being a way to freeze the terminal nobody happens to be looking at.
/// The same audit confirmed the other half: with the sink attached, the
/// filtered client received exactly zero notifications for the silenced
/// pane while its own pane's output kept arriving, and the silenced
/// pane's producer never stalled.
fn silence_pane_args(panes: &[String]) -> String {
    let mut args = String::new();
    for pane in panes {
        // Quoted for `continue_pane_command`'s reason: the argument
        // contains a `:`.
        let _ = write!(args, " -A \"{pane}:off\"");
    }
    args
}

/// One command that safely filters panes after output is live.
///
/// tmux 3.3 through 3.6 must see `pause` before `off` once a control client
/// can have pane output queued. Keeping both transitions in one command
/// prevents more output from interleaving between them. This renderer is
/// shared by attach-time overflow and panes discovered later so those two
/// live paths cannot drift on the compatibility-sensitive ordering.
fn silence_live_pane_command(panes: &[String]) -> String {
    let mut command = String::from("refresh-client");
    for pane in panes {
        let _ = write!(command, " -A \"{pane}:pause\" -A \"{pane}:off\"");
    }
    command
}

/// Chunk live pane filters below tmux's argument ceiling.
///
/// Each pane costs twice as many arguments as it does in the cutover, so a
/// live command carries half as many panes to preserve the same argv budget.
fn silence_live_pane_commands(panes: &[String]) -> Vec<String> {
    panes
        .chunks(MAX_CUTOVER_PANE_FILTERS / 2)
        .map(silence_live_pane_command)
        .collect()
}

/// The command that ends the attach-time replay command group and hands
/// the client over to live output.
///
/// Two client flags in one comma-separated `-f` list, and both halves are
/// load-bearing. `!no-output` (the leading `!` is tmux's own
/// clear-this-flag form) is the cutover itself: the client attached with
/// `no-output` set, so this is the instant pane bytes start flowing.
/// `pause-after=N` is what makes those bytes safe to fall behind on — see
/// [`TMUX_PAUSE_AFTER_SECS`]. Setting `pause-after` here rather than on
/// the original `attach` is deliberate: it belongs to the same atomic
/// hand-over as the cutover, so there is no window in which output flows
/// without the backstop.
///
/// Enabling `pause-after` also switches this client's output dialect —
/// pane bytes arrive as `%extended-output` instead of `%output`, and
/// `%pause`/`%continue` notifications appear (see [`OutputEvent`]).
/// Verified against tmux 3.7b that the combined flag list is accepted and
/// does exactly this.
///
/// # Why the pane filter rides HERE rather than being sent beforehand
///
/// `silenced` is the session's other panes — at most
/// [`MAX_CUTOVER_PANE_FILTERS`] of them, the caller having split off any
/// overflow — and folding them into this one invocation is not tidiness;
/// it is the only spelling that works.
/// Clearing `no-output` makes tmux discard the client's per-pane state, so
/// a `refresh-client -A <pane>:off` sent as its own command during the
/// attach handshake is wiped by the very cutover that starts the output it
/// was meant to filter. Audited 2026-08-02 on tmux 3.4 and 3.7b: the
/// separate-command form let 1840 and 1847 foreign notifications through
/// in six seconds respectively, while this combined form let through
/// exactly zero on both, with the client's own pane unaffected. That the
/// filter is silently lost rather than refused is what makes this worth a
/// paragraph — the failure mode is a performance regression nobody would
/// look for.
///
/// The overflow does NOT share this hazard, which is why splitting it off
/// is safe: it is sent after the cutover, when nothing changes the client's
/// `no-output` flag again and so nothing resets its per-pane state.
fn attach_cutover_command(silenced: &[String]) -> String {
    debug_assert!(
        silenced.len() <= MAX_CUTOVER_PANE_FILTERS,
        "the caller must split the overflow off; see MAX_CUTOVER_PANE_FILTERS"
    );
    format!(
        "refresh-client{} -f !no-output,pause-after={TMUX_PAUSE_AFTER_SECS}",
        silence_pane_args(silenced)
    )
}

/// The command that lifts a tmux-side pause on `pane` — `refresh-client
/// -A <pane-id>:continue`, tmux's documented pane-state form.
///
/// Used as the FINAL command of the catch-up replay group (see
/// [`OutputStream::resume_paused_with_replay`]), which is why it is a
/// command rather than a flag: it is the resume path's cutover, exactly
/// as [`attach_cutover_command`] is the attach path's. tmux acknowledges
/// it with a `%continue <pane-id>` notification and resumes the stream
/// immediately after the command group's own `%end` (verified against
/// tmux 3.7b). The argument is quoted because it contains a `:`; the
/// pane id's leading `%` is required by tmux.
fn continue_pane_command(pane: &str) -> String {
    format!("refresh-client -A \"{pane}:continue\"")
}

/// The one command group a replay cutover sends: pane modes, history
/// snapshot, visible snapshot, then `cutover` as the final command whose
/// `%end` is the replay/live boundary.
///
/// Shared rather than duplicated because the four commands and their
/// ORDER are the contract [`OutputStream::snapshot_then_cutover`] reads
/// replies against: modes, history snapshot, visible snapshot, cutover.
/// A second copy of this string for the resume path would be a second
/// place for that block-count contract to drift.
fn replay_command_group(session: &str, pane: &str, cutover: &str) -> String {
    // Every pane target is paired with its session ([`pane_in_session`]),
    // quoted because the pairing contains a `:`. A control client attaches
    // to a session but its COMMANDS resolve server-wide, so a bare `%N`
    // here would let a pane id that went stale across a tmux-server
    // restart capture — and replay — a completely different session's
    // terminal into this one.
    //
    // The captures and the cutover travel as ONE command group on
    // purpose, and the adjacency is a correctness property, not styling:
    // output the pane emits between the visible capture and the cutover
    // is delivered to nobody, so any command inserted between them — or
    // any retry scheme that re-runs the captures before a separately
    // written cutover — widens that loss window from effectively nothing
    // into something the cutover tests catch losing real records through.
    // (An issue-4 attempt at bracketing the captures with a second mode
    // sample and retrying on mismatch failed exactly that way; the fresh-
    // tab tear it chased is prevented at its source instead, by pre-sizing
    // the tab window at open so the attach-time resize stops provoking a
    // mid-capture repaint.)
    let target = format!("\"{}\"", pane_in_session(session, pane));
    format!(
        "display-message -p -t {target} '{PANE_MODE_FORMAT}' ; \
         capture-pane -p -e -N -t {target} -S -{HISTORY_LIMIT} ; \
         capture-pane -p -e -N -t {target} ; \
         {cutover}\n"
    )
}

/// Handle to the private tmux server. All tmux invocations go through
/// this so the `-S <socket> -f <config>` isolation is impossible to
/// forget — the user's own tmux server and config must never be touched.
#[derive(Debug, Clone)]
pub struct TmuxDriver {
    socket: PathBuf,
    config: PathBuf,
    /// The budget for one control-mode exchange (attach, send-keys,
    /// filter command). See [`CONTROL_EXCHANGE_TIMEOUT`] for what this
    /// bounds and why production keeps it tight; carried per-instance
    /// (rather than read from the constant directly) so
    /// [`Self::new_with_timeouts`] can hand a test-only supervisor a
    /// longer budget without touching the value every real supervisor
    /// runs with.
    exchange_timeout: std::time::Duration,
    /// The budget for [`OutputStream::foreign_panes`]'s pane listing. See
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

/// Enforce the control-mode floor before starting or adopting a server.
///
/// tmux suffixes patch releases (`3.3a`) and development builds with
/// letters; only the numeric major/minor pair decides compatibility.
fn require_supported_tmux(output: &str) -> anyhow::Result<()> {
    let version = output
        .split_whitespace()
        .nth(1)
        .context("tmux -V returned no version")?;
    let (major, minor) = version
        .split_once('.')
        .context("tmux -V returned an unrecognized version")?;
    let major: u32 = major.parse().context("parsing tmux major version")?;
    let minor_digits: String = minor
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();
    let minor: u32 = minor_digits.parse().context("parsing tmux minor version")?;
    if (major, minor) < (3, 3) {
        bail!("tmux {version} is unsupported; Farhelm requires tmux 3.3 or newer");
    }
    Ok(())
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
/// (this crate's documented floor — see `require_supported_tmux`), 3.4
/// (Ubuntu 24.04's package, so CI's), and 3.7b (the development host).
/// All three behave identically; nothing here needs a version gate.
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
    /// explicitly.
    ///
    /// `pub(crate)`, not `pub`: the production constants remain the only
    /// values any real supervisor uses (see [`Self::new`]), so the only
    /// legitimate callers of this seam are this crate's own
    /// `Supervisor::new_with_seams` (threading `SupervisorTimeouts`
    /// through) and this module's own tests — never an embedder, and never
    /// an e2e test directly (those go through `SupervisorTimeouts`, not
    /// this driver).
    pub(crate) fn new_with_timeouts(state_dir: &Path, budgets: TmuxBudgets) -> TmuxDriver {
        TmuxDriver {
            socket: state_dir.join("tmux.sock"),
            config: state_dir.join("tmux.conf"),
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
    ///   and 10ms from 3.5 on — so at the 3.3 floor the delay is half a
    ///   second, showing up as visibly laggy Esc handling in agent TUIs
    ///   and vim; 0 removes it entirely.
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
    /// on tmux inferring the table. Inference works on recent versions;
    /// being explicit works on every version at the 3.3 floor, and a
    /// config line tmux cannot place is a config line that silently does
    /// nothing.
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
    /// forgotten flag is exactly how that happens.
    fn command(&self) -> Command {
        let mut cmd = Command::new("tmux");
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

    /// Start (or adopt) the private server, then reconcile the one live
    /// option this driver actively manages after the fact, regardless of
    /// which path was taken.
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
        let version = Command::new("tmux")
            .arg("-V")
            .output()
            .await
            .context("checking tmux version")?;
        if !version.status.success() {
            bail!(
                "tmux -V failed ({}): {}",
                version.status,
                String::from_utf8_lossy(&version.stderr).trim()
            );
        }
        require_supported_tmux(&String::from_utf8_lossy(&version.stdout))?;
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

    /// Open one control client, capture replay, then turn on its live
    /// output without leaving a gap between the two.
    ///
    /// The client attaches with tmux's `no-output` flag. Mode query, two
    /// snapshots, and [`attach_cutover_command`] are then submitted as one
    /// command group through that same client. tmux runs a command group
    /// synchronously before returning to pane reads, so the final
    /// command's `%end` is the cutover: pane bytes before it are already
    /// represented by the selected snapshot, while bytes after it arrive
    /// as `%extended-output` on this stream (the cutover also sets
    /// `pause-after`, which switches the dialect — see [`OutputEvent`]).
    ///
    /// Both history and visible-only snapshots are taken because modes
    /// decide which one is valid. Normal-screen replay includes
    /// scrollback; alternate-screen replay must not mix in the normal
    /// screen's history. Keeping both captures in the same command group
    /// avoids a second mode/capture race without depending on nested
    /// `if-shell` reply blocks, whose shape changed across supported tmux
    /// releases.
    ///
    /// `session` and `pane` are BOTH needed and mean different things: a
    /// control client can only attach to a session, while everything this
    /// stream then carries is filtered down to the one pane — see
    /// [`OutputStream`] for why that filter exists and what it costs.
    pub async fn open_replay_stream(
        &self,
        session: &str,
        pane: &str,
    ) -> anyhow::Result<(PaneModes, Vec<u8>, OutputStream)> {
        Ok(self
            .open_replay_stream_candidate(session, pane)
            .await?
            .install())
    }

    /// Build a replay stream behind a cancellation-safe ownership guard.
    ///
    /// The supervisor keeps this guard until the input client is also ready.
    /// Abandoning it at any point safely reaps the output-bearing client rather
    /// than exercising the raw stream's emergency `kill_on_drop` fallback.
    pub(crate) async fn open_replay_stream_candidate(
        &self,
        session: &str,
        pane: &str,
    ) -> anyhow::Result<ReplayStreamCandidate> {
        let deadline = tokio::time::Instant::now() + self.exchange_timeout;
        let mut child = self
            .command()
            .arg("-C")
            .arg("attach")
            .arg("-f")
            .arg("no-output")
            .arg("-t")
            .arg(session)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("spawning tmux control-mode client")?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let stream = OutputStream {
            driver: self.clone(),
            client_target: format!(
                "client-{}",
                child
                    .id()
                    .context("tmux control-mode client has no process id")?
            ),
            output_disabled: false,
            child,
            // Exclusively owned: this client carries its own one-shot
            // replay-cutover command group (below), the attach-time pane
            // filter, and the occasional live `pause`-then-`off` filter —
            // all written from the single task that owns this stream. Input
            // travels on a wholly separate client — see
            // `open_input_client` — so there is no sharing concern here.
            stdin: Some(stdin),
            reader: BufReader::new(stdout),
            line: Vec::with_capacity(8192),
            passthrough: PassthroughDecoder::default(),
            pane: pane.to_string(),
            session: session.to_string(),
            silenced: HashSet::new(),
            pending_filter_replies: 0,
            foreign_dropped: 0,
            exchange_timeout: self.exchange_timeout,
            pane_list_timeout: self.pane_list_timeout,
            exit_reason: None,
        };
        let mut candidate = OutputStreamCandidate::new(stream);
        let opened = async {
            let stream = candidate.stream_mut();
            read_command_block(
                &mut stream.reader,
                &mut stream.line,
                deadline,
                "control-mode attach",
                pane,
            )
            .await?;
            // The session's other panes are learned while output is still off
            // and then silenced BY the cutover itself — see
            // `attach_cutover_command` for why they cannot be silenced by an
            // earlier command, and `OutputStream` for what the filter buys.
            // Any failure here propagates: this exchange shares the client's
            // stdout with the positionally-read replay group that follows, so
            // an exchange that did not complete cleanly leaves a stream nobody
            // can safely keep reading (see `foreign_panes`).
            let foreign = stream.foreign_panes(deadline).await?;
            // Only the first chunk can ride the cutover; the rest follow it
            // (see `MAX_CUTOVER_PANE_FILTERS`). Splitting BEFORE the cutover
            // rather than trimming inside it keeps the overflow addressed
            // rather than silently unfiltered.
            let split = foreign.len().min(MAX_CUTOVER_PANE_FILTERS);
            let (riding, overflow) = foreign.split_at(split);
            let (modes, prefill) = stream
                .snapshot_then_cutover(deadline, &attach_cutover_command(riding))
                .await?;
            for command in silence_live_pane_commands(overflow) {
                stream.send_filter_command(&command).await?;
            }
            stream.silenced.extend(foreign.iter().cloned());
            Ok::<_, anyhow::Error>((modes, prefill))
        }
        .await;
        match opened {
            Ok((modes, prefill)) => Ok(ReplayStreamCandidate::new(modes, prefill, candidate)),
            Err(error) => {
                candidate.shutdown().await;
                Err(error)
            }
        }
    }

    /// Open a control-mode client dedicated to carrying input into `pane`.
    ///
    /// Attaches with `-f no-output` PERMANENTLY, unlike the replay
    /// client's transient use of the same flag: this client must never
    /// see a pane-output notification — from THIS pane or from any other
    /// window's, since a control client attached to a session receives
    /// every pane on it — because [`InputClient::send`] reads exactly one
    /// command-reply block per chunk it writes, and an interleaved
    /// pane-output line would desynchronize that read from the write that
    /// produced it. See [`InputClient`] for why input gets its own client
    /// at all rather than riding the replay client's stdin.
    pub async fn open_input_client(
        &self,
        session: &str,
        pane: &str,
    ) -> anyhow::Result<InputClient> {
        let deadline = tokio::time::Instant::now() + self.exchange_timeout;
        let mut child = self
            .command()
            .arg("-C")
            .arg("attach")
            .arg("-f")
            .arg("no-output")
            .arg("-t")
            .arg(pane)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("spawning tmux input-control client")?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let mut reader = BufReader::new(stdout);
        let mut line = Vec::with_capacity(8192);
        // Mirrors `open_replay_stream`'s handshake: the very first thing
        // this client's stdout carries is the reply to its own implicit
        // `attach-session`, which must be drained before any `send-keys`
        // reply can be read cleanly.
        read_command_block(
            &mut reader,
            &mut line,
            deadline,
            "input-client attach",
            pane,
        )
        .await?;
        Ok(InputClient {
            child,
            stdin,
            reader,
            line,
            pane: pane.to_string(),
            // Quoted for `replay_command_group`'s reason: the pairing
            // contains a `:`, and input must not be able to reach another
            // session's pane through a stale id any more than replay can.
            target: format!("\"{}\"", pane_in_session(session, pane)),
            delivered_any_bytes: false,
            exchange_timeout: self.exchange_timeout,
        })
    }

    /// Open `session`'s SINK control client: attached, receiving every
    /// pane, and destined to be drained into nothing. See [`SessionSink`]
    /// for what it is for; this is only how it is built.
    ///
    /// # Attached with output already ON, deliberately
    ///
    /// Every other control client here attaches `-f no-output` and flips
    /// the flag later, so that the attach handshake's reply block — read
    /// positionally — cannot have pane output racing into it. The sink
    /// does the opposite, and the difference is the whole point: a client
    /// that is attached but silent is exactly the shape whose
    /// pane-holding behavior tmux does not document (see [`SessionSink`]'s
    /// note on the rejected no-output design), so a sink that spent its
    /// first moments that way would be relying, for that window, on the
    /// property it exists to avoid relying on. Attaching output-on makes
    /// it a client tmux can demonstrably deliver to from the instant it
    /// registers.
    ///
    /// The reason that is safe here and not elsewhere: this client's
    /// handshake block is read with no own-pane, so a neighbour's output
    /// arriving around it is ordinary chatter rather than an ordering
    /// violation, and tmux does not interleave notifications INSIDE a
    /// command's reply block. Audited 2026-08-02 on tmux 3.4 and 3.7b
    /// against a 16 MB/s producer: 10 of 10 attaches read a clean
    /// handshake with no payload trapped inside it, and 10 of 10 were
    /// receiving pane output immediately after.
    ///
    /// Returns only once the client is attached, because the caller's
    /// ORDERING depends on it: no per-terminal client may turn panes off
    /// until tmux already has this one (see [`silence_pane_args`]).
    pub async fn open_session_sink(&self, session: &str) -> anyhow::Result<SessionSink> {
        let deadline = tokio::time::Instant::now() + self.exchange_timeout;
        let mut child = self
            .command()
            .arg("-C")
            .arg("attach")
            .arg("-t")
            .arg(session)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("spawning tmux session-sink client")?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let sink = SessionSink {
            driver: Some(self.clone()),
            client_target: child
                .id()
                .map(|pid| format!("client-{pid}"))
                .context("tmux session-sink client has no process id")?,
            output_disabled: false,
            child,
            stdin: Some(stdin),
            reader: BufReader::new(stdout),
            line: Vec::with_capacity(8192),
        };
        let mut candidate = SessionSinkOpenCandidate::new(sink);
        if let Err(attach) = candidate
            .sink_mut()
            .read_block(deadline, "session-sink attach")
            .await
        {
            candidate.shutdown().await;
            return Err(attach);
        }
        Ok(candidate.install())
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

/// The one control client per tmux SESSION that exists purely so tmux
/// always has somebody it can deliver every pane to.
///
/// # Why a session needs one at all
///
/// tmux stops reading a pane's PTY when no attached client is able to
/// consume that pane's output — which blocks the pane's own process on
/// its next `write`. Two ways into that state matter here, and this type
/// closes both:
///
/// - A per-terminal client that STALLS. Audited 2026-08-02 on tmux 3.4 and
///   3.7b: against a 16 MB/s producer, a control client that stopped
///   reading froze that producer's pane in 4 of 5 trials per version, for
///   the entire 45-second observation window — not for `pause-after` and
///   not only for the stalled viewer's own pane. With an always-drained
///   second client attached, 5 of 5 trials per version showed no stall at
///   all (longest gap 0.20s, against a 0.15s no-client noise floor).
/// - A per-terminal client that turns foreign panes OFF, which is what
///   [`silence_pane_args`] does and what tmux's own man page says
///   stops the pane being read once EVERY client has done it.
///
/// The two are why this had to land WITH the pane filter rather than
/// before or after it: the filter is only safe because the sink is never
/// one of the clients turning anything off, and the sink is only useful
/// because it is never the client that falls behind.
///
/// # The shape, and why each part of it
///
/// Attached to the session with every pane ON (no per-pane filter) and
/// `pause-after` deliberately UNSET — a paused sink is a sink that has
/// stopped doing its job, so the one client that must never be the
/// flow-control victim does not carry the flag that makes clients
/// victims. It declares no size (no `refresh-client -C`), like every other
/// control client here, so window geometry still comes only from
/// `resize-window`. During normal operation output is on from the attach
/// itself (see [`TmuxDriver::open_session_sink`]), the sink writes no commands,
/// and it reads nothing but bytes it throws away. Orderly teardown alone
/// writes `no-output` so tmux can discard this client's queues safely.
///
/// # The one window that remains
///
/// A sink that DIES leaves its session without one until a replacement is
/// attached: a process spawn plus one round trip, retried with backoff by
/// its owner. During that window the session's terminals still have their
/// foreign panes filtered off, so a pane no terminal is watching can stop
/// being read — the very state the sink prevents. It is bounded (by the
/// owner's backoff cap) rather than eliminated, and eliminating it would
/// take a second sink standing by at all times, which trades a permanent
/// cost for a rare one. The honest statement of the guarantee is therefore
/// "except across a sink respawn", and SPEC_impl.md says so too.
///
/// A cheaper shape was measured and rejected: a client left permanently
/// `-f no-output` ALSO kept every pane readable in the same audit (6 of 6
/// trials per version), at zero delivery cost, because tmux treats a
/// no-output client as one that can never fall behind. That is an
/// implementation detail of tmux's flow control rather than anything its
/// documentation promises — the documented rule is about clients being
/// able to CONSUME — so relying on it would make this module's isolation
/// guarantee hostage to an unstated behavior. The sink pays one copy of
/// the session's traffic to rest on the documented rule instead; the pane
/// filter it enables removes N-1 copies in exchange (see [`OutputStream`]).
pub struct SessionSink {
    /// The independent command path used to disable this client's output.
    /// Test-only process doubles have no tmux driver and need no transition.
    driver: Option<TmuxDriver>,
    /// tmux's stable name for a control client spawned as this OS process.
    client_target: String,
    /// Set only after the external command has acknowledged `no-output`.
    output_disabled: bool,
    /// Reaped explicitly by orderly shutdown. `kill_on_drop` remains the
    /// cancellation and unwind fallback, tying the process to this value
    /// even when its owner cannot await teardown.
    child: Child,
    /// Held open for the sink's lifetime because stdin EOF detaches a control
    /// client. Normal operation never writes to it. Orderly shutdown disables
    /// output through the independent driver, then closes this pipe.
    stdin: Option<ChildStdin>,
    reader: BufReader<tokio::process::ChildStdout>,
    line: Vec<u8>,
}

impl SessionSink {
    /// Read one command-reply block during the attach exchange.
    ///
    /// The `own_pane` argument of [`read_command_block`] is deliberately
    /// empty here: that parameter exists so a client can tell "live output
    /// for MY pane arrived before the cutover" (an ordering violation)
    /// apart from "a neighbour is busy" (ordinary chatter). The sink
    /// speaks for no pane and every pane's output is chatter to it, so no
    /// pane id can be the wrong one to see — and an empty id matches
    /// nothing tmux can emit, since every pane id starts with `%`.
    async fn read_block(
        &mut self,
        deadline: tokio::time::Instant,
        purpose: &str,
    ) -> anyhow::Result<()> {
        read_command_block(&mut self.reader, &mut self.line, deadline, purpose, "").await?;
        Ok(())
    }

    /// The sink client's process id, or `None` once it has been reaped.
    ///
    /// Exists for the supervisor's own logging and for the test that kills
    /// the sink out from under a live attachment to prove it self-heals:
    /// the pid is the only handle on a process that a test can `kill -9`,
    /// and a respawn is only observable as the pid changing.
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// A sink backed by an arbitrary child process, for tests that need to
    /// drive the SUPERVISING LOOP rather than tmux.
    ///
    /// `run_session_sink`'s policy — when to retry, how long to wait,
    /// whether a run counted as healthy — is decided entirely by how long
    /// `drain` takes to return, and pinning it against a real tmux would
    /// mean killing real clients and hoping the timing lands. A `cat`
    /// whose stdin the test holds is the same thing without the hope: it
    /// drains until the test decides it should end.
    #[cfg(test)]
    pub(crate) fn from_child_for_tests(mut child: Child) -> SessionSink {
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        SessionSink {
            driver: None,
            client_target: String::new(),
            output_disabled: true,
            child,
            stdin: Some(stdin),
            reader: BufReader::new(stdout),
            line: Vec::new(),
        }
    }

    /// Read and discard this client's output until it ends.
    ///
    /// Returns when the client exits or its stream fails — the sink has no
    /// other terminal state, and both cases mean the same thing to the
    /// caller: this session no longer has a sink and needs a new one. The
    /// bytes are genuinely thrown away; nothing about a session's panes is
    /// learned here, and the read exists only so tmux never has an unread
    /// client to apply flow control against.
    ///
    /// Reads in fixed-size chunks rather than lines on purpose. A control
    /// client's output is line-oriented, but the sink parses none of it,
    /// and a line-based read would make an enormous single notification
    /// (a pane emitting a megabyte with no newline) an allocation the sink
    /// has no reason to make.
    pub async fn drain(&mut self) {
        let mut scratch = [0u8; 64 * 1024];
        loop {
            match self.reader.read(&mut scratch).await {
                Ok(0) => return,
                Ok(_) => {}
                Err(e) => {
                    warn!(error = %e, "session-sink control client read failed");
                    return;
                }
            }
        }
    }

    /// Disable this sink's output before detaching its control client.
    async fn disable_output_before_shutdown(&mut self) -> anyhow::Result<()> {
        if self.output_disabled
            || self
                .child
                .try_wait()
                .context("checking whether the session-sink client already exited")?
                .is_some()
        {
            return Ok(());
        }
        let Some(driver) = &self.driver else {
            // Process doubles used by the supervising-loop tests are not tmux
            // clients and have no pane queue to invalidate.
            self.output_disabled = true;
            return Ok(());
        };
        if let Err(error) = driver
            .disable_control_client_output(&self.client_target)
            .await
        {
            if self
                .child
                .try_wait()
                .context("rechecking whether the session-sink client exited")?
                .is_some()
            {
                return Ok(());
            }
            return Err(error).context("disabling session-sink output before shutdown");
        }
        self.output_disabled = true;
        Ok(())
    }

    /// Detach this control client and confirm that the process has exited.
    ///
    /// `kill_on_drop` remains the cancellation fallback, but it cannot
    /// establish a handoff boundary: dropping requests termination without
    /// waiting for the old tmux client to disappear. More importantly, a
    /// process kill can invalidate output tmux has already queued for this
    /// client and crash the server; [`shutdown_output_control_client`] closes
    /// stdin and drains the tail instead. Orderly last-owner teardown uses
    /// this path before a queued reattach may create its replacement.
    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.disable_output_before_shutdown().await?;
        shutdown_output_control_client(
            &mut self.child,
            self.stdin.take(),
            &mut self.reader,
            "session-sink control client",
        )
        .await
    }
}

/// Own a sink client until its attach handshake commits or cleanup finishes.
///
/// `open_session_sink` spawns the process before its first awaited reply. This
/// guard makes cancellation hand the client to the runtime, which retries the
/// output-off boundary without keeping the request future alive.
struct SessionSinkOpenCandidate {
    sink: Option<SessionSink>,
    runtime: tokio::runtime::Handle,
}

impl SessionSinkOpenCandidate {
    fn new(sink: SessionSink) -> Self {
        Self {
            sink: Some(sink),
            runtime: tokio::runtime::Handle::current(),
        }
    }

    fn sink_mut(&mut self) -> &mut SessionSink {
        self.sink.as_mut().expect("session-sink candidate is live")
    }

    fn install(mut self) -> SessionSink {
        self.sink.take().expect("session-sink candidate is live")
    }

    async fn shutdown(mut self) {
        if let Some(mut sink) = self.sink.take() {
            reap_session_sink_candidate(&mut sink).await;
        }
    }
}

impl Drop for SessionSinkOpenCandidate {
    fn drop(&mut self) {
        let Some(mut sink) = self.sink.take() else {
            return;
        };
        self.runtime.spawn(async move {
            reap_session_sink_candidate(&mut sink).await;
        });
    }
}

/// Retry an uncommitted sink's safe boundary without flooding logs or forks.
async fn reap_session_sink_candidate(sink: &mut SessionSink) {
    let mut failures = 0u32;
    loop {
        match sink.shutdown().await {
            Ok(()) => return,
            Err(error) => {
                failures = failures.saturating_add(1);
                if failures.is_power_of_two() {
                    warn!(
                        error = %format!("{error:#}"),
                        failures,
                        "uncommitted session-sink cleanup is not yet safe; retrying"
                    );
                }
            }
        }
        tokio::time::sleep(control_cleanup_retry_delay(failures)).await;
    }
}

/// A control-mode client streaming ONE PANE's output.
///
/// It starts with output disabled and is only returned after replay
/// capture and the live cutover have completed. The client counts as
/// attached in tmux's eyes but never declares a size (no
/// `refresh-client -C`), so tmux ignores it for sizing entirely —
/// geometry comes only from explicit `resize-window` calls. Orderly teardown
/// closes the command pipe and drains the client before reaping it.
/// `kill_on_drop` remains only for cancellation and unwinding, where an async
/// shutdown cannot run.
///
/// # One pane, out of a whole session's stream
///
/// tmux control clients attach to a SESSION, not to a window or a pane,
/// and a control client receives `%output`/`%extended-output` for every
/// pane in every window of that session — audited empirically against
/// tmux 3.7b with a second window present, which is the shape terminal
/// tabs introduce (PLAN_M4.md item 2). Narrowing that down takes BOTH of
/// the mechanisms below, and neither is redundant.
///
/// **tmux-side, and the one that matters.** Every pane of the session
/// except this stream's own is turned off for this client with
/// `refresh-client -A <pane>:off` — at attach for the panes that exist
/// then ([`Self::foreign_panes`], folded into the cutover), and after a
/// protective `pause` when the filter must be installed after live output
/// begins. That second path covers both attach-time overflow and a pane
/// first discovered later ([`Self::silence_late_pane`]). This is
/// what makes a session's control-mode read work O(session traffic)
/// instead of O(attached terminals × session traffic), and — more
/// importantly than the arithmetic — it is what stops a busy tab's bytes
/// from queueing in front of the agent's bytes inside that viewer's own
/// client. That head-of-line delay was measured at 3.3–3.9s of
/// agent-output latency under a flooding neighbour, against 0.36–0.38s
/// with the filter on (2026-08-02, tmux 3.4 and 3.7b).
///
/// This is only safe because [`SessionSink`] exists: tmux stops READING a
/// pane whose every attached client has turned it off, and the sink is the
/// client that never does. Turning foreign panes off WITHOUT a sink froze
/// the silenced pane's producer for an entire 10-second observation window
/// in the same audit. The two mechanisms ship together or not at all.
///
/// **Local, as belt and braces.** A notification naming another pane is
/// still dropped on the way through. tmux's filter is a per-client
/// setting, so there is always a window — an attach in flight, a pane
/// created a moment ago — in which a foreign notification can legitimately
/// arrive, and a stray one must be harmless rather than merely unlikely.
/// The drop runs BEFORE any decoding: [`classify_control_line`] splits the
/// pane id out of each notification, and a line naming another pane is
/// dropped there — never fed to [`PassthroughDecoder`], which carries
/// wrapper state across notifications and would be corrupted by a
/// stranger's bytes landing inside a wrapper of ours.
///
/// # Stall isolation, and what it now rests on
///
/// A stalled viewer on one terminal costs that terminal alone. Three
/// separate properties combine to that, and dropping any one of them
/// weakens it:
///
/// - `pause-after` is per (client, pane), so tmux cuts the stalled
///   client's stream rather than anybody else's (see
///   [`TMUX_PAUSE_AFTER_SECS`]).
/// - The pane filter above means a stalled terminal is only ever behind on
///   its own pane in the first place.
/// - [`SessionSink`] guarantees tmux always has a client it can deliver
///   every pane to, so tmux's OTHER answer to a lagging client — stop
///   reading the pane, which blocks the pane's process on its next write
///   and is not bounded by `pause-after` — has no client to take against.
///
/// Before the sink this last branch was a real, measured residual, and the
/// honest qualification that used to live here said so. It is now closed
/// rather than merely narrowed: with the sink attached, no trial across
/// tmux 3.4 and 3.7b reproduced a pane stall that stalled-client trials
/// without it reproduced 4 times in 5.
pub struct OutputStream {
    /// Independent access to the private server for the shutdown boundary.
    driver: TmuxDriver,
    /// tmux names non-terminal control clients `client-<pid>`.
    client_target: String,
    /// Whether the server has acknowledged client-wide `no-output`.
    output_disabled: bool,
    child: Child,
    /// Held open for the lifetime of the client. Carries the attach-time
    /// exchanges (the pane list, then the replay-cutover command group)
    /// and, afterwards, the occasional fire-and-forget pane filter for a
    /// pane that appeared late — never terminal input, which travels on a
    /// wholly separate [`InputClient`]. So this is a plain,
    /// exclusively-owned handle rather than a shared, lockable one: only
    /// the one task driving this stream ever writes to it.
    stdin: Option<ChildStdin>,
    reader: BufReader<tokio::process::ChildStdout>,
    line: Vec<u8>,
    /// Stateful because tmux may split one passthrough wrapper across
    /// several output notifications.
    passthrough: PassthroughDecoder,
    /// The one pane this stream speaks for — see the type's own docs for
    /// why a session-wide client needs to know that at all.
    ///
    /// Held as the raw tmux pane id (`%N`) so every comparison is a plain
    /// byte equality against what a notification carries, with no
    /// re-parsing per line.
    pane: String,
    /// The tmux session that pane belongs to, kept so the catch-up replay
    /// can address it the same session-paired way the attach did — and so
    /// no caller has to carry a second copy of either handle (see
    /// [`Self::pane`]).
    session: String,
    /// Panes this client has already told tmux to stop delivering (see the
    /// type's own docs).
    ///
    /// Purely a memo against re-sending: the state that matters lives in
    /// the tmux server, and `refresh-client -A <pane>:off` is idempotent
    /// there. What this prevents is a pane that manages to speak once more
    /// before the off takes effect turning into a command per notification
    /// — an unbounded write rate driven by a foreign pane's output rate,
    /// which is exactly the coupling the filter exists to remove.
    ///
    /// Bounded by [`MAX_SILENCED_PANES`] rather than pruned precisely.
    /// tmux's control protocol has no pane-close notification to prune on
    /// (`%window-close` names a window, and a pane can also go without
    /// one), and reconciling against a live `list-panes` would mean an
    /// exchange on a client whose replies nothing is positioned to read.
    /// Forgetting wholesale is sound because every entry is only a memo:
    /// the worst a cleared set can cost is one redundant, idempotent
    /// command per pane that speaks again.
    silenced: HashSet<String>,
    /// Filter-command reply blocks tmux still owes this client.
    ///
    /// Every command written outside a synchronous exchange
    /// ([`Self::send_filter_command`]) is answered by a `%begin`/`%end`
    /// pair that arrives on the same stdout the CATCH-UP path reads
    /// positionally. Left unaccounted, one such reply landing just before
    /// [`Self::resume_paused_with_replay`] would be mistaken for the
    /// modes reply and shift the whole four-block group by one — a replay
    /// that silently returns the wrong capture, which is exactly the class
    /// of bug positional reads exist to be careful about.
    ///
    /// So the count goes up when a command is written and down when
    /// [`Self::next_output`] passes a block terminator, and the catch-up
    /// path drains whatever is still outstanding before it begins. Nothing
    /// else ever writes to this client, so every terminator this stream
    /// sees while live belongs to one of these.
    pending_filter_replies: usize,
    /// How many foreign-pane notifications this stream dropped locally.
    ///
    /// Not used in production — it exists so a test can assert the tmux
    /// filter is doing the work rather than the local drop quietly
    /// covering for it. "The right bytes came out" is true either way; only
    /// this number distinguishes a filter that was installed from one that
    /// was not. Read directly by the tests in this module; deliberately
    /// without an accessor, so nothing outside can grow a dependency on a
    /// diagnostic counter.
    foreign_dropped: u64,
    /// This stream's copy of [`TmuxDriver::exchange_timeout`], carried
    /// here because [`Self::send_filter_command`] and
    /// [`Self::resume_paused_with_replay`] compute their own fresh
    /// deadlines well after the attach that created this stream — they
    /// have no deadline handed to them by a caller, so the budget has to
    /// live on the stream itself.
    exchange_timeout: std::time::Duration,
    /// This stream's copy of [`TmuxDriver::pane_list_timeout`], consulted
    /// by [`Self::foreign_panes`] for the same reason as
    /// `exchange_timeout`.
    pane_list_timeout: std::time::Duration,
    /// Why tmux said this control client was going away, if it said so at
    /// all: `None` means no `%exit` was ever seen — the stream simply hit
    /// EOF, which is what a tmux process killed outright looks like from
    /// here — while `Some("")` means tmux announced a bare `%exit` with no
    /// reason.
    ///
    /// Purely diagnostic, and deliberately private. Nothing in production
    /// branches on it: `Ok(None)` from [`Self::next_output`] already means
    /// everything the forwarder needs to act on, and the reason is logged
    /// at the moment it is set. It is KEPT because this module's own tests
    /// — a descendant module, so private is reach enough — need to name
    /// the cause when a control client vanishes mid-test, which was
    /// previously unrecoverable: three CI failures on a vanished client
    /// yielded zero evidence of what had happened to it.
    exit_reason: Option<String>,
}

/// The optional supervisor barrier completed by a provisional replay stream.
type ReplayCompletionSender = tokio::sync::watch::Sender<Option<Result<(), std::sync::Arc<str>>>>;

/// A replay stream that has not yet been committed to an attachment.
///
/// The supervisor can hold this across input-client setup. Cancellation or an
/// input setup failure then reaps the already-output-bearing client before the
/// optional handoff barrier completes; installing it transfers ownership to
/// the forwarder and completes that barrier immediately.
pub(crate) struct ReplayStreamCandidate {
    modes: Option<PaneModes>,
    prefill: Option<Vec<u8>>,
    stream: Option<OutputStreamCandidate>,
    completion: Option<ReplayCompletionSender>,
    runtime: tokio::runtime::Handle,
}

impl ReplayStreamCandidate {
    fn new(modes: PaneModes, prefill: Vec<u8>, stream: OutputStreamCandidate) -> Self {
        Self {
            modes: Some(modes),
            prefill: Some(prefill),
            stream: Some(stream),
            completion: None,
            runtime: tokio::runtime::Handle::current(),
        }
    }

    /// Tie abandonment to a supervisor-published terminal handoff barrier.
    pub(crate) fn set_completion(&mut self, completion: ReplayCompletionSender) {
        self.completion = Some(completion);
    }

    /// Commit the client to a forwarder and release its provisional barrier.
    pub(crate) fn install(mut self) -> (PaneModes, Vec<u8>, OutputStream) {
        if let Some(completion) = self.completion.take() {
            completion.send_replace(Some(Ok(())));
        }
        (
            self.modes.take().expect("replay candidate has modes"),
            self.prefill.take().expect("replay candidate has prefill"),
            self.stream
                .take()
                .expect("replay candidate has a stream")
                .install(),
        )
    }
}

impl Drop for ReplayStreamCandidate {
    fn drop(&mut self) {
        let Some(stream) = self.stream.take() else {
            return;
        };
        let completion = self.completion.take();
        self.runtime.spawn(async move {
            stream.shutdown().await;
            if let Some(completion) = completion {
                completion.send_replace(Some(Ok(())));
            }
        });
    }
}

/// Own an output client until attach either commits it or reaps it safely.
///
/// The attach exchange can be cancelled after tmux has enabled live output.
/// Dropping the raw stream there would invoke `kill_on_drop` while pane blocks
/// may still be queued, which is the tmux 3.7b server-abort trigger. This guard
/// transfers every abandoned client to the runtime and keeps retrying the
/// acknowledged `no-output` boundary instead.
struct OutputStreamCandidate {
    stream: Option<OutputStream>,
    runtime: tokio::runtime::Handle,
}

impl OutputStreamCandidate {
    fn new(stream: OutputStream) -> Self {
        Self {
            stream: Some(stream),
            runtime: tokio::runtime::Handle::current(),
        }
    }

    fn stream_mut(&mut self) -> &mut OutputStream {
        self.stream.as_mut().expect("output candidate is live")
    }

    fn install(mut self) -> OutputStream {
        self.stream.take().expect("output candidate is live")
    }

    async fn shutdown(mut self) {
        if let Some(stream) = self.stream.take()
            && let Err(reaper) = stream.shutdown().await
        {
            let _ = reaper.run().await;
        }
    }
}

impl Drop for OutputStreamCandidate {
    fn drop(&mut self) {
        let Some(stream) = self.stream.take() else {
            return;
        };
        self.runtime.spawn(async move {
            if let Err(reaper) = stream.shutdown().await {
                let _ = reaper.run().await;
            }
        });
    }
}

impl OutputStream {
    /// The pane this stream carries.
    ///
    /// Exists so the forwarder driving this stream does not keep its own
    /// copy of the pane id: two fields that must agree, in two structs
    /// with different lifetimes, is exactly the kind of duplication that
    /// goes wrong quietly — a forwarder replaying the wrong pane's
    /// history would look like a plain terminal glitch.
    pub fn pane(&self) -> &str {
        &self.pane
    }

    /// Every pane of this stream's session except its own, asked while
    /// output is still off so the answer can be folded into the cutover.
    ///
    /// # Why a failure here fails the whole attach
    ///
    /// What this produces is only an optimization's input — the local
    /// filter in [`Self::next_output`] drops whatever arrives anyway, and
    /// [`SessionSink`] keeps every pane readable regardless — so it is
    /// tempting to swallow errors and attach unfiltered. That would be
    /// wrong for a reason that has nothing to do with the filter: this
    /// exchange shares one stdout with the replay group that follows, and
    /// that group reads its four reply blocks POSITIONALLY. An exchange
    /// that timed out or hit EOF mid-block may have left an unconsumed
    /// `%end` behind, and continuing would then pair the modes reply with
    /// the history capture — a replay that returns the wrong bytes and
    /// says nothing. There is no way to tell a cleanly-refused command
    /// from a half-read one at this layer, so every failure is treated as
    /// the dangerous one and the caller discards the client.
    ///
    /// The deadline is [`PANE_LIST_TIMEOUT`], clamped to the attach's own:
    /// a tmux slow to answer an optimization's question must not eat the
    /// budget the replay still needs, and must not extend the attach
    /// either.
    ///
    /// The obvious race — a pane closing between this listing and the
    /// cutover that names it — is benign, and that was worth confirming
    /// rather than assuming, because the alternative would have been an
    /// attach that fails whenever a tab happens to close at the wrong
    /// moment. tmux accepts `-A` for a pane id it cannot find and replies
    /// `%end`, not `%error` (audited 2026-08-02 on 3.4 and 3.7b), so a
    /// stale id in the cutover costs nothing at all.
    async fn foreign_panes(
        &mut self,
        attach_deadline: tokio::time::Instant,
    ) -> anyhow::Result<Vec<String>> {
        let deadline = attach_deadline.min(tokio::time::Instant::now() + self.pane_list_timeout);
        let command = list_session_panes_command(&self.session);
        let listing = self
            .exchange(deadline, &command, "session pane list")
            .await
            .context("listing a session's panes for this terminal's pane filter")?;
        Ok(listing
            .split(|&byte| byte == b'\n')
            .map(strip_line_ending)
            .filter(|pane| is_pane_id_shaped(pane) && *pane != self.pane.as_bytes())
            .map(|pane| String::from_utf8_lossy(pane).into_owned())
            .collect())
    }

    /// Write one pane-filter command and record that tmux owes a reply for
    /// it — the only way this type writes once output is live.
    ///
    /// Fire-and-forget in the sense that nothing waits for the reply HERE:
    /// the stream's own reader passes it as chatter (and decrements the
    /// debt), or the catch-up path drains it before its positional reads
    /// begin. See [`Self::pending_filter_replies`] for why the debt is
    /// counted rather than assumed to have cleared.
    ///
    /// The write is bounded by [`CONTROL_EXCHANGE_TIMEOUT`] and a failure
    /// is returned rather than logged. Both matter and neither is
    /// theoretical: this runs from inside [`Self::next_output`], so an
    /// unbounded `write_all` on a pipe whose reader has wedged would park
    /// the forwarder — a terminal that stops updating with nothing
    /// anywhere reporting a fault — and a swallowed error would leave the
    /// stream believing a filter is installed that never was.
    async fn send_filter_command(&mut self, command: &str) -> anyhow::Result<()> {
        let line = format!("{command}\n");
        let stdin = self
            .stdin
            .as_mut()
            .context("the tmux pane-filter client is already shutting down")?;
        tokio::time::timeout(self.exchange_timeout, async {
            stdin
                .write_all(line.as_bytes())
                .await
                .context("writing a tmux pane-filter command")?;
            stdin
                .flush()
                .await
                .context("flushing a tmux pane-filter command")
        })
        .await
        .context("timed out writing a tmux pane-filter command")??;
        self.pending_filter_replies += 1;
        Ok(())
    }

    /// Consume every filter-command reply block tmux still owes, so a
    /// positional read can start from a known boundary.
    ///
    /// Called only by [`Self::resume_paused_with_replay`], and only ever
    /// with this stream's own pane already paused — which is what makes
    /// [`read_command_block`]'s own-pane guard the right one to pass:
    /// live output for our pane arriving here really would mean the
    /// ordering assumption had broken.
    async fn settle_filter_replies(
        &mut self,
        deadline: tokio::time::Instant,
    ) -> anyhow::Result<()> {
        while self.pending_filter_replies > 0 {
            let pane = self.pane.clone();
            read_command_block(
                &mut self.reader,
                &mut self.line,
                deadline,
                "pane filter reply",
                &pane,
            )
            .await?;
            self.pending_filter_replies -= 1;
        }
        Ok(())
    }

    /// Stop delivery of a pane this stream had never heard from before —
    /// the tab opened, or the pane split, after this terminal attached.
    ///
    /// tmux's pane filter names panes one at a time and has no wildcard
    /// (`refresh-client -A` takes a pane id; only `-B` accepts `%*`), and a
    /// pane that did not exist at attach time is delivered ON by default.
    /// Reacting to the pane's first notification is what covers that
    /// without any plumbing from the tab-open path down into every
    /// attached terminal's forwarder: the notification IS the discovery.
    /// The cost is at most one command per pane per attachment, since
    /// [`Self::silenced`] remembers.
    ///
    /// The command goes out through [`Self::send_filter_command`], so its
    /// reply is accounted for rather than merely expected to be harmless,
    /// and a write failure ends the stream instead of being logged: a
    /// terminal whose control client cannot be written to is not a
    /// terminal that should keep pretending.
    ///
    /// Unlike the attach-time filter this CAN be its own command: nothing
    /// changes the client's `no-output` flag after the cutover, so there is
    /// no per-pane state reset to lose it to (see
    /// [`attach_cutover_command`]).
    ///
    /// A pane id that is not shaped like one, or one already remembered,
    /// is a no-op — the first because nothing unvalidated reaches a
    /// command line, the second because [`Self::silenced`] exists exactly
    /// to keep a chatty foreign pane from driving a command per line.
    async fn silence_late_pane(&mut self, pane: &[u8]) -> anyhow::Result<()> {
        if !is_pane_id_shaped(pane) {
            return Ok(());
        }
        let pane = String::from_utf8_lossy(pane).into_owned();
        if self.silenced.contains(&pane) {
            return Ok(());
        }
        // Forget everything rather than grow without bound; see
        // `MAX_SILENCED_PANES` for why that is sound and what it costs.
        if self.silenced.len() >= MAX_SILENCED_PANES {
            self.silenced.clear();
        }
        self.silenced.insert(pane.clone());
        let command = silence_live_pane_command(std::slice::from_ref(&pane));
        self.send_filter_command(&command).await
    }

    /// Send one command on this client and read back its reply block.
    ///
    /// Only usable while output is off (the attach exchange): once pane
    /// bytes flow, replies have to be read by whatever is draining the
    /// stream, which is why every later command goes through
    /// [`Self::send_filter_command`] instead.
    async fn exchange(
        &mut self,
        deadline: tokio::time::Instant,
        command: &str,
        purpose: &str,
    ) -> anyhow::Result<Vec<u8>> {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let pane = self.pane.clone();
        let stdin = self
            .stdin
            .as_mut()
            .context("the tmux exchange client is already shutting down")?;
        tokio::time::timeout(remaining, async {
            stdin
                .write_all(format!("{command}\n").as_bytes())
                .await
                .with_context(|| format!("writing the tmux {purpose} command"))?;
            stdin
                .flush()
                .await
                .with_context(|| format!("flushing the tmux {purpose} command"))
        })
        .await
        .with_context(|| format!("timed out writing the tmux {purpose} command"))??;
        read_command_block(&mut self.reader, &mut self.line, deadline, purpose, &pane).await
    }
}

/// What one classified control line means to [`OutputStream::next_output`],
/// in a form that outlives the borrow of the line buffer it was read from.
///
/// Exists purely so the read loop can act on a line after releasing that
/// borrow — the foreign-pane arm writes a command through `&mut self`, and
/// the terminator arm touches the reply-debt counter, neither of which can
/// happen while a classified slice of `self.line` is still alive.
enum Decision {
    /// Hand this to the caller and stop reading.
    Event(OutputEvent),
    /// The client is gone, carrying whatever reason tmux gave for it
    /// (empty for a bare `%exit`) — see [`ControlLine::Exit`].
    Exit(String),
    /// Our pane's bytes ended mid-passthrough-wrapper: nothing to hand
    /// back yet, keep reading.
    Incomplete,
    /// A foreign pane already filtered — count it, do nothing else.
    ForeignKnown,
    /// A foreign pane heard from for the first time, carrying its id.
    ForeignNew(Vec<u8>),
    /// Anything else tmux says. Only a block terminator is acted on; see
    /// the read loop.
    Chatter,
}

/// Whether `candidate` has the shape of a tmux pane id (`%` followed by at
/// least one digit, nothing else).
///
/// Every id this module puts into a `refresh-client -A` argument passes
/// through here first. Pane ids reach this process from tmux itself, but
/// they arrive as bytes on a stream shared with pane CONTENT — a
/// notification's shape is the only thing separating the two — so a value
/// that is about to be interpolated into a command line is shape-checked
/// rather than trusted, the same defensive reading `scope::is_uuid_shaped`
/// applies to tmux user options.
fn is_pane_id_shaped(candidate: &[u8]) -> bool {
    matches!(candidate.split_first(), Some((b'%', digits))
        if !digits.is_empty() && digits.iter().all(u8::is_ascii_digit))
}

/// One thing read off a control client's stream that the forwarder cares
/// about. Everything else tmux says (command replies, layout changes,
/// window renames) is consumed and discarded inside
/// [`OutputStream::next_output`].
///
/// The two non-byte variants exist because `pause-after` (see
/// [`TMUX_PAUSE_AFTER_SECS`]) makes tmux's flow-control state visible on
/// this stream, and the supervisor must act on it rather than treat it as
/// chatter: a `Paused` means tmux CUT this client's pane stream and the
/// bytes it dropped are gone from the live path for good, recoverable
/// only by replaying history (PLAN_M2_5.md's reset-then-replay catch-up).
/// Swallowing it would leave the client silently missing a chunk of its
/// terminal with nothing anywhere noticing.
#[derive(Debug, PartialEq, Eq)]
pub enum OutputEvent {
    /// Decoded pane bytes, ready for the wire. Never empty.
    Bytes(Vec<u8>),
    /// `%pause`: tmux stopped sending this pane's output to this client
    /// because it fell further behind than `pause-after` allows, and
    /// discarded what it had queued. The stream stays quiet until
    /// [`OutputStream::resume_paused_with_replay`].
    ///
    /// Its ABSENCE is not evidence of anything: tmux answers a lagging
    /// client either this way or by throttling the pane instead, and
    /// which one is timing-dependent rather than version-dependent (see
    /// [`TMUX_PAUSE_AFTER_SECS`]). Code may act on this event but must
    /// never wait for one.
    Paused,
}

impl OutputStream {
    /// Capture modes and content, then hand the client over to live
    /// output at the final command block boundary.
    ///
    /// `cutover` is that final command — [`attach_cutover_command`] on the
    /// attach path, [`continue_pane_command`] on the catch-up path. The
    /// four expected blocks are deliberately explicit. Treating the group
    /// as one opaque reply makes it too easy to enable output after only
    /// the first `%end`, which reintroduces the replay/live overlap this
    /// method exists to remove.
    async fn snapshot_then_cutover(
        &mut self,
        deadline: tokio::time::Instant,
        cutover: &str,
    ) -> anyhow::Result<(PaneModes, Vec<u8>)> {
        let command = replay_command_group(&self.session, &self.pane, cutover);
        let pane = self.pane.clone();
        let pane = pane.as_str();
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let stdin = self
            .stdin
            .as_mut()
            .context("the tmux replay client is already shutting down")?;
        tokio::time::timeout(remaining, async {
            stdin
                .write_all(command.as_bytes())
                .await
                .context("writing tmux replay cutover commands")?;
            stdin
                .flush()
                .await
                .context("flushing tmux replay cutover commands")
        })
        .await
        .context("timed out writing tmux replay cutover commands")??;

        let modes_output = read_command_block(
            &mut self.reader,
            &mut self.line,
            deadline,
            "pane modes",
            pane,
        )
        .await?;
        let history_output = read_command_block(
            &mut self.reader,
            &mut self.line,
            deadline,
            "history snapshot",
            pane,
        )
        .await?;
        let visible_output = read_command_block(
            &mut self.reader,
            &mut self.line,
            deadline,
            "visible snapshot",
            pane,
        )
        .await?;
        let _cutover = read_command_block(
            &mut self.reader,
            &mut self.line,
            deadline,
            "live-output cutover",
            pane,
        )
        .await?;

        let modes_output = strip_command_output_terminator(&modes_output);
        let modes_text = String::from_utf8_lossy(modes_output);
        warn_once_about_missing_bracket_paste(&modes_text);
        let modes = PaneModes::parse(&modes_text);
        let snapshot = if modes.alternate_on {
            visible_output
        } else {
            history_output
        };
        let snapshot = strip_command_output_terminator(&snapshot);
        Ok((modes, normalize_capture(snapshot)))
    }

    /// Next event of interest, or `None` when the client exits (session
    /// killed, server gone). Notifications this stream has no use for —
    /// command replies, layout changes, window renames — are consumed and
    /// ignored; they are not terminal content and not flow control.
    ///
    /// BOTH output dialects are accepted unconditionally, in every call,
    /// rather than switching on whether `pause-after` was set: `%output`
    /// is what tmux emits without the flag (and what an older or
    /// differently-configured client would still send), `%extended-output`
    /// is what it emits with it. The two carry identical payloads and go
    /// through identical decoding — see [`decode_output_payload`] — so a
    /// stream that changes dialect mid-life, or interleaves the two, loses
    /// nothing.
    ///
    /// Reads BYTES, never lines-as-`String`: tmux's control-mode escaping
    /// only octal-escapes bytes below 0x20 (and backslash), so anything
    /// ≥ 0x80 crosses this stream raw. Decoding as UTF-8 would fail on
    /// any pane emitting binary or non-UTF-8 output — `cat` of a binary
    /// file — and one such byte would kill the terminal for good.
    ///
    /// NOT cancel-safe: a cancelled call can leave a partially-read line
    /// behind, which the next call discards — and, since the foreign-pane
    /// path below writes, a half-written filter command on the client's
    /// stdin, which would desynchronize any later command on it. Callers
    /// may only abandon this future on a path that tears the whole stream
    /// down (the stall detach does exactly that), never to resume reading
    /// afterwards. That was already the rule; the write only widens what
    /// breaking it would cost.
    ///
    /// Notifications about ANOTHER pane are chatter here, discarded with
    /// the same indifference as a `%layout-change`. That drop is the
    /// belt-and-braces half of the pane filter (see the type's own docs):
    /// without it, opening a tab would start spraying the tab's shell
    /// output into every already-attached terminal of that session, and a
    /// foreign pane's `%pause` would send this terminal through a
    /// reset-and-replay catch-up for bytes it was never carrying. Seeing
    /// one is also the trigger for asking tmux to stop sending that pane
    /// at all ([`Self::silence_late_pane`]), which is how a tab opened
    /// after this terminal attached ends up filtered server-side like
    /// every other.
    pub async fn next_output(&mut self) -> anyhow::Result<Option<OutputEvent>> {
        loop {
            self.line.clear();
            let n = read_control_line(&mut self.reader, &mut self.line).await?;
            if n == 0 {
                return Ok(None);
            }
            // Classify under a borrow of the line buffer, then ACT on an
            // owned decision. The split exists because acting can need
            // `&mut self` (writing a filter command), which cannot coexist
            // with a classified borrow of one of self's own fields.
            let decision = {
                let OutputStream {
                    line,
                    pane,
                    passthrough,
                    silenced,
                    ..
                } = &mut *self;
                let own_pane = pane.as_bytes();
                match classify_control_line(strip_line_ending(line)) {
                    Some(ControlLine::Payload { pane, escaped }) if pane == own_pane => {
                        let bytes = decode_output_payload(passthrough, escaped);
                        if bytes.is_empty() {
                            // A chunk that ended mid-wrapper: not an event,
                            // and not chatter either — keep reading.
                            Decision::Incomplete
                        } else {
                            Decision::Event(OutputEvent::Bytes(bytes))
                        }
                    }
                    Some(ControlLine::Paused { pane }) if pane == own_pane => {
                        Decision::Event(OutputEvent::Paused)
                    }
                    // Another window's pane, on the same session-attached
                    // control client. Deliberately NOT fed to the
                    // passthrough decoder: that decoder carries wrapper
                    // state across notifications, and mixing a stranger's
                    // bytes into it would corrupt a wrapper of ours that
                    // happened to be open at the time.
                    Some(
                        ControlLine::Payload { pane: other, .. }
                        | ControlLine::Paused { pane: other },
                    ) => {
                        // Copied only when this pane is new. A pane already
                        // filtered can still be mid-flight for a moment, and
                        // allocating per notification for one we have
                        // nothing left to do about would put the allocation
                        // rate back under a foreign pane's control.
                        if std::str::from_utf8(other).is_ok_and(|id| silenced.contains(id)) {
                            Decision::ForeignKnown
                        } else {
                            Decision::ForeignNew(other.to_vec())
                        }
                    }
                    Some(ControlLine::Exit { reason }) => {
                        Decision::Exit(String::from_utf8_lossy(reason).into_owned())
                    }
                    None => Decision::Chatter,
                }
            };
            match decision {
                Decision::Event(event) => return Ok(Some(event)),
                Decision::Exit(reason) => {
                    // Diagnostics only: the `Ok(None)` below is unchanged,
                    // and every caller still reads it as "this stream
                    // ended". What the reason buys is the difference
                    // between a tmux server that was killed and one that
                    // died on its own, which is otherwise erased here and
                    // unrecoverable afterwards.
                    self.exit_reason = Some(reason);
                    warn!(
                        pane = %self.pane,
                        // The stored copy rather than the local, so this
                        // field has a production reader and cannot drift
                        // from what the tests later report.
                        // Debug-formatted, and that is a security choice
                        // rather than a style one: tmux names the SESSION
                        // in several of its exit reasons, a pane inherits
                        // `$TMUX` and can rename its own session to
                        // anything at all — control characters included —
                        // so this string is pane-influenced text arriving
                        // at a log an operator reads in a terminal.
                        // `Display` would replay those bytes verbatim;
                        // `Debug` escapes them. `session` beside it is
                        // safe under `Display` because it is the
                        // supervisor's own stored name, never read back
                        // from tmux.
                        session = %self.session,
                        reason = ?self.exit_reason,
                        "tmux control client exited"
                    );
                    return Ok(None);
                }
                Decision::Incomplete => {}
                Decision::ForeignKnown => self.foreign_dropped += 1,
                Decision::ForeignNew(pane) => {
                    self.foreign_dropped += 1;
                    self.silence_late_pane(&pane).await?;
                }
                Decision::Chatter => {
                    // The one piece of chatter that is not ignorable: a
                    // block terminator settles part of the filter-reply
                    // debt the catch-up path would otherwise mistake for
                    // its own first reply (see `pending_filter_replies`).
                    if self.pending_filter_replies > 0
                        && matches!(
                            parse_control_marker(strip_line_ending(&self.line)),
                            Some(ControlMarker::End(_) | ControlMarker::Error(_))
                        )
                    {
                        self.pending_filter_replies -= 1;
                    }
                }
            }
        }
    }

    /// Lift a tmux-side pause and re-establish this client's stream as a
    /// reattach: snapshot the pane's modes and history, then continue it.
    ///
    /// The catch-up contract (PLAN_M2_5.md): the bytes tmux dropped while
    /// paused are exactly the bytes history can replay, so the resume path
    /// reuses the attach path's snapshot machinery rather than growing a
    /// second replay implementation — same command group, same block
    /// accounting, only the cutover command differs.
    ///
    /// Ordering is load-bearing and empirically verified (tmux 3.7b): the
    /// snapshot is taken while the pane is STILL paused, so no live output
    /// can interleave with the command group's reply blocks, and the
    /// continue is the group's LAST command, so the first live byte after
    /// its `%end` is the first byte past the snapshot. Continuing first and
    /// snapshotting after would race resumed output into
    /// [`read_command_block`], which would silently fold it into the
    /// captured history.
    ///
    /// The caller must reset the client's terminal before writing the
    /// returned prefill: the replay assumes an empty terminal, and this
    /// one lands in a terminal already showing the pre-pause bytes.
    pub async fn resume_paused_with_replay(&mut self) -> anyhow::Result<(PaneModes, Vec<u8>)> {
        // Discard any half-decoded passthrough wrapper left over from the
        // stream tmux just cut. `%pause` can land in the middle of a
        // wrapper split across notifications, and that partial state
        // belongs to bytes the client will never receive — carrying it
        // into the post-reset stream would make the decoder treat the
        // first fresh bytes as a wrapper continuation and swallow or
        // mangle them. The replay itself is capture-pane output, which
        // never contains a wrapper, so nothing is lost by clearing here.
        self.passthrough = PassthroughDecoder::default();
        let deadline = tokio::time::Instant::now() + self.exchange_timeout;
        // Start the positional reads from a known boundary. A pane filter
        // written moments before the `%pause` can still have its reply in
        // flight, and this group would otherwise read that reply as its
        // modes block and shift every block after it (see
        // `pending_filter_replies`). The pause is what makes this safe to
        // wait for: our own pane is quiet, so the only thing that can
        // arrive is the debt being settled.
        self.settle_filter_replies(deadline).await?;
        let cutover = continue_pane_command(&self.pane);
        self.snapshot_then_cutover(deadline, &cutover).await
    }

    /// Clear this client's queued pane blocks through an independent command.
    ///
    /// The output stream may be between any two bytes of an in-band command or
    /// reply when teardown wins its race. Targeting the client externally makes
    /// that partial protocol state irrelevant: the separate tmux process exits
    /// only after the server has applied client-wide `no-output`.
    async fn disable_output_before_shutdown(&mut self) -> anyhow::Result<()> {
        if self.output_disabled
            || self
                .child
                .try_wait()
                .context("checking whether the terminal-output client already exited")?
                .is_some()
        {
            return Ok(());
        }
        if let Err(error) = self
            .driver
            .disable_control_client_output(&self.client_target)
            .await
        {
            if self
                .child
                .try_wait()
                .context("rechecking whether the terminal-output client exited")?
                .is_some()
            {
                return Ok(());
            }
            return Err(error).context("disabling terminal output before shutdown");
        }
        self.output_disabled = true;
        Ok(())
    }

    /// Attempt the complete safe-boundary transition and process reap once.
    async fn try_shutdown(&mut self) -> anyhow::Result<()> {
        self.disable_output_before_shutdown().await?;
        shutdown_output_control_client(
            &mut self.child,
            self.stdin.take(),
            &mut self.reader,
            "terminal-output control client",
        )
        .await
    }

    /// Submit a raw command line on this client's stdin, for tests only.
    ///
    /// Exists so a test can force tmux into a state that is otherwise
    /// reachable only by timing — specifically `refresh-client -A
    /// <pane>:pause`, tmux's on-demand form of the same pane pause
    /// `pause-after` produces after a delay. Provoking that delay-driven
    /// pause deterministically is not possible from a test (it depends on
    /// how far tmux happens to have read ahead of a client that then
    /// stalls), so the on-demand form is what makes the catch-up path
    /// testable at all.
    ///
    /// Deliberately does NOT read the reply: the caller is expected to go
    /// on consuming the stream through [`Self::next_output`], which
    /// discards command-reply blocks like any other chatter. Production
    /// code must not use this — every real command this client sends is
    /// part of a command GROUP whose replies are read back in lockstep by
    /// [`Self::snapshot_then_cutover`].
    #[cfg(test)]
    async fn send_raw_command(&mut self, command: &str) -> anyhow::Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .context("the test stream is shut down")?;
        stdin.write_all(command.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }

    /// Read and discard control lines until this client's in-flight
    /// command reply closes with `%end`/`%error`. Tests only.
    ///
    /// The complement to [`Self::send_raw_command`]: tmux nests the
    /// `%pause`/`%continue` notification INSIDE the reply block of the
    /// command that caused it, so a test that stops reading the moment it
    /// sees the notification leaves the block's closing marker unread —
    /// and the replay group that follows reads reply blocks positionally,
    /// so one stray marker desynchronizes it. Waiting for the marker
    /// explicitly is what makes that deterministic; the obvious
    /// alternative (a short timeout on [`Self::next_output`], hoping it
    /// consumes the line) both races and cancels a read this type
    /// documents as not cancel-safe.
    #[cfg(test)]
    async fn drain_command_reply(&mut self) -> anyhow::Result<()> {
        let deadline = tokio::time::Instant::now() + self.exchange_timeout;
        loop {
            self.line.clear();
            let read = read_control_line_before(
                &mut self.reader,
                &mut self.line,
                deadline,
                "test command reply",
            )
            .await?;
            if read == 0 {
                bail!("control client exited before its command reply closed");
            }
            if matches!(
                parse_control_marker(strip_line_ending(&self.line)),
                Some(ControlMarker::End(_) | ControlMarker::Error(_))
            ) {
                return Ok(());
            }
        }
    }

    /// Detach the control-mode client and wait for it to actually be gone.
    ///
    /// The external `no-output` acknowledgement is load-bearing. Killing or
    /// closing a client while tmux has a pane block queued for it can abort
    /// tmux 3.7b with `fatal: not enough data`; [`Self::try_shutdown`] first
    /// makes tmux discard that queue, then closes stdin and reaps the client.
    /// A bounded kill is safe only after that boundary and exists for a client
    /// that refuses the normal EOF exit.
    ///
    /// The client must be dead before another attaches: overlapping control
    /// clients reproducibly froze the newcomer's stream after replay. The
    /// mechanism was never pinned down (in isolation two attached control
    /// clients do both receive pane output), so callers depend on the ordering,
    /// not that explanation.
    pub async fn shutdown(mut self) -> Result<(), OutputReaper> {
        match self.try_shutdown().await {
            Ok(()) => Ok(()),
            Err(error) => Err(OutputReaper {
                stream: self,
                last_error: format!("{error:#}"),
            }),
        }
    }
}

/// A runtime-owned retry of an output client's safe shutdown boundary.
///
/// Returning an ordinary error would drop `OutputStream`, whose
/// `kill_on_drop` child can invalidate a pane block before tmux has discarded
/// it. This value retains every process handle instead. The supervisor publishes
/// a per-terminal barrier before letting a replacement attach, while this task
/// retries until the server acknowledges `no-output` and the client is reaped.
pub struct OutputReaper {
    stream: OutputStream,
    last_error: String,
}

impl OutputReaper {
    /// Retry forever rather than turn an unconfirmed client into a safe one by
    /// assertion. The registry waiting on this task makes the failure visible
    /// and keeps only the affected terminal closed to replacement.
    pub async fn run(mut self) -> Result<(), std::sync::Arc<str>> {
        let mut failures = 1u32;
        loop {
            if failures.is_power_of_two() {
                warn!(
                    pane = %self.stream.pane,
                    error = %self.last_error,
                    failures,
                    "terminal-output client cleanup is not yet safe; retrying"
                );
            }
            tokio::time::sleep(control_cleanup_retry_delay(failures)).await;
            match self.stream.try_shutdown().await {
                Ok(()) => return Ok(()),
                Err(error) => {
                    failures = failures.saturating_add(1);
                    self.last_error = format!("{error:#}");
                }
            }
        }
    }
}

/// A SECOND control-mode client, dedicated to carrying input into one
/// pane, opened by [`TmuxDriver::open_input_client`].
///
/// This replaced two earlier designs, each broken in its own way.
/// `load-buffer -`/`paste-buffer -d -r` avoided hex-encoded input bytes on
/// a spawned process's argv (world-readable via `/proc/<pid>/cmdline`, and
/// input includes credentials typed at agent prompts — see SPEC_impl.md),
/// but paste-buffer caret-escapes control bytes on its way into the pane —
/// verified empirically against tmux 3.7b, DEL (0x7f) arrives as the two
/// literal characters `^?`, ESC as `^[`, ctrl-C as `^C` — so backspace,
/// arrow keys, and ctrl-C were all silently mangled. `send-keys -H`
/// delivers bytes verbatim and fixed that, but writing it to the shared
/// `OutputStream` client's stdin (the very next design) opened a
/// different hole: that call returned once `write_all`/`flush` proved the
/// OS pipe accepted the bytes, which is not the same as tmux having
/// executed them. Two failure modes hid behind that gap — an `%error`
/// reply had nowhere to go, because `OutputStream::next_output` discards
/// every notification it has no use for (command replies, layout-change
/// chatter, `%exit`), so a rejected `send-keys` vanished
/// silently instead of surfacing as dropped input; and a takeover could
/// kill the shared client after `send` returned `Ok` but before tmux
/// processed the buffered command, losing input that had already been
/// reported delivered.
///
/// A dedicated client closes both: nothing else ever writes to or reads
/// from it, so each `send-keys` command's `%begin`/`%end`/`%error` reply
/// can be read synchronously, in-line, via the same
/// `read_command_block` machinery `OutputStream` uses for its own
/// one-shot command group. `send` returning `Ok` now means tmux actually
/// executed the command — not merely that the pipe accepted the bytes.
/// The alternative — correlating replies on the shared output stream —
/// would need an actor owning that stream's stdout so a concurrent reader
/// could hand back the right reply; a second no-output client gets the
/// same synchronous request/reply property for free from machinery that
/// already exists.
///
/// Chunked at [`InputClient::MAX_CHUNK`] bytes per `send-keys` line
/// because tmux rejects a command carrying on the order of ~1000
/// arguments as "command too long", and each input byte becomes one hex
/// argument; 256 stays far below that ceiling with comfortable margin.
/// Chunks are written and their replies read strictly one at a time —
/// there is no pipelining — which is fine because nothing else ever
/// contends for this client's stdin/stdout: unlike `OutputStream`'s
/// former shared-stdin design, there is no concurrent-caller reordering
/// hazard here to guard against.
pub struct InputClient {
    /// Never read directly — `#[allow(dead_code)]` documents that this is
    /// deliberate, not an oversight. Kept alive purely so dropping this
    /// value (from any teardown path — takeover, detach, connection loss,
    /// or a failed `send`) kills the process via `kill_on_drop`, exactly
    /// like `OutputStream`'s client. Unlike `OutputStream`, there is no
    /// long-lived task driving this client's stdout in a loop, so
    /// `kill_on_drop` alone is enough; there is no
    /// cancelled-task-vs-clean-shutdown distinction to make, and so no
    /// explicit `shutdown` method that would read this field either.
    #[allow(dead_code)]
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<tokio::process::ChildStdout>,
    line: Vec<u8>,
    /// The pane this client addresses, for the block reader's own
    /// filtering.
    pane: String,
    /// The quoted `=<session>:.<pane>` target every `send-keys` names —
    /// see [`pane_in_session`] for why input is session-paired rather than
    /// addressed by bare pane id.
    target: String,
    /// Whether tmux has ever CONFIRMED executing a `send-keys` carrying at
    /// least one byte through this client.
    ///
    /// Exists for PLAN_M3.md item 8's correlator: conversation capture
    /// anchors on the moment input actually reached the pane, because that
    /// is the last moment before the agent's record can exist. Confirmed
    /// delivery — a chunk whose `%end` came back — is the only reading that
    /// supports that, which is why this is set here rather than inferred
    /// from `send` returning `Ok`: a send that failed part-way still
    /// delivered the chunks it had confirmed, and one of those may have
    /// carried the prompt's newline.
    ///
    /// Deliberately never reset. It answers "has anything ever landed",
    /// and the correlator it feeds is itself write-once.
    delivered_any_bytes: bool,
    /// This client's copy of [`TmuxDriver::exchange_timeout`], consulted by
    /// [`Self::send`] for every chunk write, flush, and reply read. Stored
    /// rather than passed in per call because `send` computes a fresh
    /// per-reply deadline of its own well after this client was opened —
    /// the same reason `OutputStream` carries its own copy.
    exchange_timeout: std::time::Duration,
}

impl InputClient {
    /// Bytes of input per `send-keys -H` command line. See the struct
    /// docs for why this exists and why 256 was chosen.
    const MAX_CHUNK: usize = 256;

    /// Commands in flight before replies are drained. See `send` for why
    /// this bound is deadlock avoidance, not tuning: 64 replies ≈ 2 KiB,
    /// comfortably inside the ~64 KiB pipe capacity that an unbounded
    /// pipeline would wedge against.
    const PIPELINE_BATCH: usize = 64;

    /// The control client's process id, for lifecycle fault-injection tests.
    pub(crate) fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Deliver `bytes` to the pane as keystrokes, verbatim, confirming
    /// every chunk executed before returning.
    ///
    /// Returning `Ok` is the whole point of this type existing: it means
    /// tmux's control-mode protocol closed every chunk's command block
    /// with `%end`, not merely that the OS pipe accepted the writes. A
    /// `%error` reply becomes an `Err` here instead of vanishing the way
    /// it did on the old shared-stdin design.
    ///
    /// Pipelined in bounded batches, not lock-step: up to
    /// [`Self::PIPELINE_BATCH`] chunk commands are written and flushed,
    /// then that batch's replies are read in order, then the next batch.
    /// tmux replies to control-mode commands in submission order, so
    /// pairing the Nth reply with the Nth chunk needs no correlation
    /// beyond counting — and the guarantee is unchanged, since nothing
    /// returns until the final `%end`. Lock-step write/read was measured
    /// at ~70 KB/s on a multi-megabyte paste (one round trip per 256-byte
    /// chunk); batching removes most of that per-chunk latency.
    ///
    /// The batch bound is not an optimization knob, it is deadlock
    /// avoidance: writing every command before reading any reply lets an
    /// unbounded reply volume accumulate against the client's ~64 KiB
    /// stdout pipe. Once that fills, tmux stops relaying, backpressure
    /// reaches this side's writes, and both ends block until the timeout
    /// fires — a large paste would fail instead of being slow. One
    /// batch's replies (~30 bytes each) stay far below the pipe capacity.
    pub async fn send(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        if bytes.is_empty() {
            // Nothing reaches the pane, so nothing is delivered — see
            // `delivered_any_bytes`, whose whole point is that an empty
            // frame must not start conversation capture's clock.
            return Ok(());
        }
        let mut line = String::with_capacity(32 + Self::MAX_CHUNK * 3);
        let chunks: Vec<&[u8]> = bytes.chunks(Self::MAX_CHUNK).collect();
        for batch in chunks.chunks(Self::PIPELINE_BATCH) {
            for chunk in batch {
                line.clear();
                write!(line, "send-keys -t {} -H", self.target)
                    .expect("String write is infallible");
                for byte in *chunk {
                    write!(line, " {byte:02x}").expect("String write is infallible");
                }
                line.push('\n');
                tokio::time::timeout(self.exchange_timeout, self.stdin.write_all(line.as_bytes()))
                    .await
                    .context("timed out writing tmux send-keys command")?
                    .context("writing tmux send-keys command")?;
            }
            tokio::time::timeout(self.exchange_timeout, self.stdin.flush())
                .await
                .context("timed out flushing tmux send-keys commands")?
                .context("flushing tmux send-keys commands")?;
            for _ in 0..batch.len() {
                // One deadline per reply, not one for the whole batch: a
                // wedged tmux on one reply must not inherit the unspent
                // budget of every reply before it.
                let deadline = tokio::time::Instant::now() + self.exchange_timeout;
                read_command_block(
                    &mut self.reader,
                    &mut self.line,
                    deadline,
                    "send-keys input",
                    &self.pane,
                )
                .await?;
                // Marked per CONFIRMED chunk, not once at the end: a send
                // that fails on a later chunk has still delivered this
                // one, and the correlator cares about the earliest byte
                // that landed rather than about the call succeeding.
                self.delivered_any_bytes = true;
            }
        }
        Ok(())
    }

    /// Whether this attachment has ever had input confirmed into its pane.
    /// See [`InputClient::delivered_any_bytes`] for why conversation
    /// capture keys on this rather than on a successful `send`.
    pub fn delivered_any_bytes(&self) -> bool {
        self.delivered_any_bytes
    }
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

    /// The supported-version check accepts patch suffixes but rejects the
    /// control-mode versions below the documented 3.3 floor.
    #[test]
    fn tmux_version_floor_is_enforced() {
        assert!(require_supported_tmux("tmux 3.3").is_ok());
        assert!(require_supported_tmux("tmux 3.3a").is_ok());
        assert!(require_supported_tmux("tmux 3.10").is_ok());
        assert!(require_supported_tmux("tmux 4.0").is_ok());
        assert!(require_supported_tmux("tmux 3.2a").is_err());
        assert!(require_supported_tmux("not-a-version").is_err());
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

    /// Run a scripted control-mode transcript through the exact
    /// classification and decoding `OutputStream::read_next_event` uses,
    /// collecting the events a forwarder would have observed.
    ///
    /// Line-per-line rather than through a reader on purpose: framing is
    /// already pinned by this module's `read_control_line` tests, and
    /// keeping this helper free of any I/O is what lets the notification
    /// GRAMMAR be tested without a tmux server. One shared
    /// [`PassthroughDecoder`] across the whole transcript is load-bearing,
    /// not incidental — it is what makes a wrapper split across
    /// notifications (or across dialects) observable here at all.
    fn events_from_transcript(transcript: &[&[u8]]) -> Vec<OutputEvent> {
        let mut decoder = PassthroughDecoder::default();
        let mut events = Vec::new();
        for line in transcript {
            match classify_control_line(line) {
                Some(ControlLine::Payload { escaped, .. }) => {
                    let bytes = decode_output_payload(&mut decoder, escaped);
                    if !bytes.is_empty() {
                        events.push(OutputEvent::Bytes(bytes));
                    }
                }
                Some(ControlLine::Paused { .. }) => events.push(OutputEvent::Paused),
                Some(ControlLine::Exit { .. }) => break,
                None => {}
            }
        }
        events
    }

    /// `%extended-output` — the dialect `pause-after` switches tmux into,
    /// and therefore the ONLY dialect a production attachment sees after
    /// this milestone. Pins the documented shape (`pane-id age ... :
    /// value`), the extensibility rule (unknown fields between the age
    /// and the `:` are ignored rather than counted, so a future tmux
    /// argument cannot shift the payload), and that a payload containing
    /// colons and spaces of its own is not mistaken for the separator.
    #[test]
    fn extended_output_is_decoded_with_and_without_extra_fields() {
        let events = events_from_transcript(&[
            br"%extended-output %0 0 : plain\015\012",
            b"%extended-output %0 12 future args here : with:colons and spaces",
            b"%extended-output %0 0 :",
        ]);
        assert_eq!(
            events,
            vec![
                OutputEvent::Bytes(b"plain\r\n".to_vec()),
                OutputEvent::Bytes(b"with:colons and spaces".to_vec()),
            ],
            "an empty payload yields no event at all, and neither extra fields nor colons in \
             the payload may shift the split"
        );
    }

    /// Plain `%output` must keep working unchanged: it is what tmux emits
    /// with no `pause-after` set, and the parser accepts both dialects
    /// unconditionally rather than switching on client state. Interleaving
    /// the two in one transcript is deliberate — passthrough-wrapper state
    /// is carried ACROSS notifications, so a dialect boundary landing
    /// inside a wrapper must decode exactly as if it never happened.
    #[test]
    fn both_output_dialects_share_one_decoder() {
        let events = events_from_transcript(&[
            br"%output %0 before\033Ptmux;\033\033]52;c;",
            br"%extended-output %0 0 : aGk=\007\033\134after",
        ]);
        assert_eq!(
            events,
            vec![
                // The wrapper opens in the `%output` notification and its
                // payload so far (an un-doubled ESC, then the OSC opener)
                // comes out with it; the rest, including the `ESC \` that
                // CLOSES the wrapper, arrives in the `%extended-output`
                // one and must decode as a continuation rather than as a
                // fresh, unwrapped chunk.
                OutputEvent::Bytes(b"before\x1b]52;c;".to_vec()),
                OutputEvent::Bytes(b"aGk=\x07after".to_vec()),
            ],
            "a passthrough wrapper opened in one dialect must close correctly in the other"
        );
    }

    /// `%pause` must surface as an event; `%continue` must NOT, and must
    /// not disturb the output around it.
    ///
    /// The asymmetry is the point. A swallowed `%pause` is the worst
    /// outcome in this file — tmux has cut the stream, the bytes it drops
    /// are gone from the live path forever, and nothing downstream would
    /// know to replay them, so the terminal would just silently miss a
    /// chunk. `%continue`, by contrast, is pure acknowledgement: it
    /// arrives inside the reply block of the very command that requested
    /// it, so nothing ever waits on it, and surfacing it would only give
    /// callers a variant they must remember to ignore. This pins that it
    /// is discarded like any other chatter AND that discarding it does not
    /// swallow or reorder the adjacent pane output.
    #[test]
    fn pause_surfaces_as_an_event_while_continue_is_discarded_intact() {
        let events = events_from_transcript(&[
            b"%extended-output %0 0 : a",
            b"%pause %0",
            b"%window-renamed @0 sh",
            b"%continue %0",
            b"%extended-output %0 0 : b",
        ]);
        assert_eq!(
            events,
            vec![
                OutputEvent::Bytes(b"a".to_vec()),
                OutputEvent::Paused,
                OutputEvent::Bytes(b"b".to_vec()),
            ],
            "`%continue` and unrelated notifications alike must be discarded without \
             disturbing the pane bytes on either side of them"
        );
    }

    /// The pane filter driven through the PRODUCTION path —
    /// [`OutputStream::next_output`] itself, over a real reader — rather
    /// than through `classify_control_line` alone.
    ///
    /// The grammar test above pins which lines carry which pane; this
    /// pins what the method built on it actually EMITS, which is a
    /// different claim and the one that matters. Two properties only this
    /// shape can show:
    ///
    /// - A passthrough wrapper of OUR pane, split across notifications,
    ///   survives foreign payloads interleaved into the split. The
    ///   decoder carries wrapper state across calls, so a filter that
    ///   dropped foreign lines only AFTER decoding would feed a
    ///   stranger's bytes into our half-open wrapper and corrupt it.
    /// - A foreign `%pause` produces no event at all. Surfacing it would
    ///   send this terminal through a full reset-and-replay catch-up for
    ///   bytes it never carried.
    ///
    /// An [`OutputStream`] over a canned transcript, plus the process
    /// standing in for tmux on its WRITE half.
    ///
    /// Both halves are real child processes rather than in-memory pipes,
    /// and both for reasons: the read half must be a `ChildStdout` because
    /// that is what the production type reads and what EOFs the way a
    /// control client's exit does, and the write half must be a pipe
    /// somebody is emptying because `next_output` now WRITES (the late
    /// pane filter). A write half nothing drained would fill at 64 KiB and
    /// park the stream under test — a hang, not an assertion failure.
    ///
    /// The returned `Child` is the write half's holder: dropping it kills
    /// that process and closes the pipe, so callers must keep it alive for
    /// as long as they drive the stream.
    fn stream_over_transcript(transcript: &'static [u8]) -> (OutputStream, Child) {
        let mut feeder = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawning the transcript feeder");
        let mut feeder_stdin = feeder.stdin.take().expect("piped stdin");
        let feeder_stdout = feeder.stdout.take().expect("piped stdout");
        tokio::spawn(async move {
            let _ = feeder_stdin.write_all(transcript).await;
        });
        let mut command_sink = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("spawning the command sink");
        let stdin = command_sink.stdin.take().expect("piped stdin");
        let stream = OutputStream {
            // This fixture exercises parsing over process-backed pipes, not
            // tmux teardown. Marking its synthetic client already disabled
            // keeps shutdown-only server state out of the fixture's contract.
            driver: TmuxDriver::new(std::path::Path::new("")),
            client_target: String::new(),
            output_disabled: true,
            child: feeder,
            stdin: Some(stdin),
            reader: BufReader::new(feeder_stdout),
            line: Vec::new(),
            passthrough: PassthroughDecoder::default(),
            pane: "%0".to_string(),
            session: "fh-s".to_string(),
            silenced: HashSet::new(),
            pending_filter_replies: 0,
            foreign_dropped: 0,
            // This helper drives `next_output` directly against a canned
            // transcript, never a real tmux, so neither budget is ever
            // consulted — production defaults are just the least
            // surprising filler.
            exchange_timeout: CONTROL_EXCHANGE_TIMEOUT,
            pane_list_timeout: PANE_LIST_TIMEOUT,
            exit_reason: None,
        };
        (stream, command_sink)
    }

    #[tokio::test]
    async fn next_output_emits_only_its_own_panes_events_through_a_split_wrapper() {
        let transcript: &[u8] = b"%output %0 before\\033Ptmux;\\033\\033]52;c;\n\
                                  %output %7 foreign-one\n\
                                  %extended-output %7 0 : foreign-two\n\
                                  %pause %7\n\
                                  %window-renamed @3 something\n\
                                  %extended-output %0 0 : aGk=\\007\\033\\134after\n\
                                  %pause %0\n\
                                  %exit\n";
        let (mut stream, _command_sink) = stream_over_transcript(transcript);
        let mut events = Vec::new();
        while let Some(event) = stream.next_output().await.expect("stream must not fail") {
            events.push(event);
        }
        assert_eq!(
            events,
            vec![
                // The wrapper opened in the first notification; its
                // payload so far comes out with it.
                OutputEvent::Bytes(b"before\x1b]52;c;".to_vec()),
                // ...and CLOSES correctly in ours, three foreign
                // notifications later, proving none of them reached the
                // decoder.
                OutputEvent::Bytes(b"aGk=\x07after".to_vec()),
                // Only OUR pane's pause surfaces.
                OutputEvent::Paused,
            ],
            "a foreign pane's payloads and pauses must be dropped before decoding, and this \
             pane's split wrapper must close intact across them"
        );
    }

    /// The exact attach-cutover text, pinned like every other generated
    /// tmux command in this module. Both flags ride one comma-separated
    /// `-f` list, and the `!` prefix is what CLEARS `no-output` rather
    /// than setting a flag by that name — get either detail wrong and the
    /// client silently either never receives output or never gets the
    /// `pause-after` backstop, neither of which fails loudly.
    ///
    /// The filtered form is pinned in the SAME test because the two must
    /// stay one invocation: splitting the `-A` arguments into a command of
    /// their own is exactly the mistake that loses the filter to tmux's
    /// per-pane state reset (see [`attach_cutover_command`]), and it would
    /// still read perfectly well at the call site.
    #[test]
    fn attach_cutover_sets_pause_after_while_clearing_no_output() {
        assert_eq!(
            attach_cutover_command(&[]),
            format!("refresh-client -f !no-output,pause-after={TMUX_PAUSE_AFTER_SECS}")
        );
        assert_eq!(
            attach_cutover_command(&["%1".to_string(), "%12".to_string()]),
            format!(
                "refresh-client -A \"%1:off\" -A \"%12:off\" \
                 -f !no-output,pause-after={TMUX_PAUSE_AFTER_SECS}"
            )
        );
    }

    /// The pane filter's exact arguments: one ` -A <pane>:off` per pane,
    /// quoted for the `:`, and nothing at all for an empty list.
    ///
    /// The empty case is not pedantry — it is what keeps a session with no
    /// other panes from turning its cutover into a differently-spelled
    /// command than the one the test above pins.
    #[test]
    fn the_pane_filter_names_every_foreign_pane_once() {
        assert_eq!(
            silence_pane_args(&["%1".to_string(), "%12".to_string()]),
            " -A \"%1:off\" -A \"%12:off\""
        );
        assert_eq!(silence_pane_args(&[]), "");
    }

    /// Live pane filters stay below tmux's argument ceiling and preserve order.
    ///
    /// tmux refuses a command line of roughly a thousand arguments, and
    /// each post-cutover pane costs four: `-A <pane>:pause` followed by
    /// `-A <pane>:off`. Without chunking, a session with enough tabs would
    /// make every attach on it fail outright. The boundary is checked
    /// exactly at the live cap and one past it, and every pane must retain
    /// both transitions in the required order.
    #[test]
    fn live_pane_filters_are_ordered_and_chunked_below_tmuxs_argument_ceiling() {
        let panes = |n: usize| (0..n).map(|i| format!("%{i}")).collect::<Vec<_>>();
        let live_cap = MAX_CUTOVER_PANE_FILTERS / 2;

        let at_cap = panes(live_cap);
        let commands = silence_live_pane_commands(&at_cap);
        assert_eq!(commands.len(), 1, "the cap itself must be one command");

        let over_cap = panes(live_cap + 1);
        let commands = silence_live_pane_commands(&over_cap);
        assert_eq!(commands.len(), 2, "one past the cap must split");
        for command in &commands {
            assert!(
                command.starts_with("refresh-client "),
                "a live pane filter must be a refresh-client command: {command}"
            );
            assert!(
                command.matches(" -A ").count() <= MAX_CUTOVER_PANE_FILTERS,
                "a command exceeded the established argument budget: {command}"
            );
        }
        // A split that drops its remainder or reverses either transition
        // silently restores the old crash path for that pane.
        let rendered = commands.join(" ");
        for pane in &over_cap {
            assert_eq!(
                rendered
                    .matches(&format!("-A \"{pane}:pause\" -A \"{pane}:off\""))
                    .count(),
                1,
                "{pane} must be paused and filtered exactly once, in order"
            );
        }
        assert!(silence_live_pane_commands(&[]).is_empty());
    }

    /// The per-attachment memo of already-filtered panes stays bounded
    /// however many distinct panes speak to one terminal.
    ///
    /// A session can churn through tabs indefinitely — open, close, open —
    /// and every one of them that ever emits a byte lands in this set.
    /// Unbounded, that is a slow leak keyed by a number the user controls,
    /// on a struct that lives as long as an attachment does. The memo is
    /// only an optimization (see [`MAX_SILENCED_PANES`]), so forgetting is
    /// free; what this pins is that it forgets rather than grows.
    #[tokio::test]
    async fn the_filtered_pane_memo_stays_bounded_under_pane_churn() {
        // One line per pane, more of them than the cap, so a set that
        // never forgot would end up strictly larger than it.
        static TRANSCRIPT: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| {
            let mut transcript = Vec::new();
            for pane in 1..=(MAX_SILENCED_PANES + 50) {
                transcript.extend_from_slice(format!("%output %{pane} x\n").as_bytes());
            }
            transcript.extend_from_slice(b"%exit\n");
            transcript
        });
        let (mut stream, _command_sink) = stream_over_transcript(&TRANSCRIPT);
        while stream
            .next_output()
            .await
            .expect("the stream must not fail")
            .is_some()
        {}
        assert!(
            stream.silenced.len() <= MAX_SILENCED_PANES,
            "the filtered-pane memo grew to {} entries",
            stream.silenced.len()
        );
        assert_eq!(
            stream.foreign_dropped,
            (MAX_SILENCED_PANES + 50) as u64,
            "every foreign notification must still have been counted and dropped"
        );
    }

    /// Pane ids are shape-checked before they are interpolated into a
    /// `refresh-client` command line.
    ///
    /// They arrive on the same stream as pane CONTENT — the notification
    /// grammar is the only thing separating the two — so this is a trust
    /// boundary, not a formatting nicety: without it a crafted
    /// notification could put arbitrary text into a command this process
    /// sends to its own tmux server.
    #[test]
    fn only_real_pane_ids_reach_the_pane_filter() {
        for good in ["%0", "%7", "%1234"] {
            assert!(is_pane_id_shaped(good.as_bytes()), "{good} is a pane id");
        }
        for bad in ["", "%", "0", "@1", "%1a", "%1 -A", "%1\"", " %1"] {
            assert!(
                !is_pane_id_shaped(bad.as_bytes()),
                "{bad:?} must not reach a command line"
            );
        }
    }

    /// The exact continue text. `-A <pane>:continue` is tmux's pane-state
    /// form; the pane id keeps its leading `%` and the argument is quoted
    /// because of the `:`. A wrong spelling here would surface only as a
    /// pane that never resumes after a stall.
    #[test]
    fn continue_command_uses_the_pane_state_form() {
        assert_eq!(
            continue_pane_command("%7"),
            "refresh-client -A \"%7:continue\""
        );
    }

    /// Late filtering discards queued output before turning the pane off.
    ///
    /// The order is the tmux 3.3–3.6 crash workaround: direct `off` can
    /// leave a queued block pointing at bytes tmux has already freed, while
    /// `pause` clears that queue first. Keeping both states in one command
    /// prevents pane output from reopening the vulnerable gap.
    #[test]
    fn late_pane_filter_pauses_before_turning_output_off() {
        assert_eq!(
            silence_live_pane_command(&["%7".to_string()]),
            "refresh-client -A \"%7:pause\" -A \"%7:off\""
        );
    }

    /// Abandoning a provisional replay client performs the safe transition.
    ///
    /// Attach can be cancelled after the replay cutover enabled output but
    /// before an input client or forwarder exists. The provisional guard must
    /// keep that client alive through the external `no-output` acknowledgement,
    /// then reap it without taking the private tmux server down.
    #[tokio::test]
    async fn an_abandoned_replay_candidate_is_reaped_after_no_output() {
        let mut server = ScratchServer::start().await;
        let pane = server
            .driver
            .create_session(
                "fh-abandoned-replay",
                "/",
                80,
                24,
                &[],
                &ticking_pane("ABANDONED"),
            )
            .await
            .expect("session");
        let gate = Arc::new(DisableOutputGate::default());
        server.driver.disable_output_gate = Some(Arc::clone(&gate));
        let candidate = server
            .driver
            .open_replay_stream_candidate("fh-abandoned-replay", &pane)
            .await
            .expect("replay candidate");
        let target = candidate
            .stream
            .as_ref()
            .expect("candidate stream")
            .stream
            .as_ref()
            .expect("output stream")
            .client_target
            .clone();

        drop(candidate);
        tokio::time::timeout(std::time::Duration::from_secs(10), gate.entered.notified())
            .await
            .expect("candidate cleanup reaches the external boundary");
        assert!(
            server
                .driver
                .run(&["list-clients", "-F", "#{client_name}"])
                .await
                .expect("listing clients before acknowledgement")
                .lines()
                .any(|name| name == target),
            "the candidate client closed before no-output was acknowledged"
        );

        gate.release.notify_one();
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            gate.acknowledged.notified(),
        )
        .await
        .expect("candidate cleanup acknowledges no-output");
        gate.finish.notify_one();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let clients = server
                .driver
                .run(&["list-clients", "-F", "#{client_name}"])
                .await
                .expect("listing clients after acknowledgement");
            if !clients.lines().any(|name| name == target) {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the abandoned output client was not reaped"
            );
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert!(
            server
                .driver
                .has_session("fh-abandoned-replay")
                .await
                .expect("checking the private server after candidate cleanup"),
            "candidate cleanup must not abort the private tmux server"
        );
    }

    /// Shutdown establishes `no-output` outside a dirty positional exchange.
    ///
    /// A forwarder can be cancelled after writing a multi-command replay group
    /// but before reading any reply. An in-band shutdown command would then
    /// mistake the first old `%end` for its own acknowledgement and close the
    /// output client while tmux still owns queued pane blocks. Holding the
    /// independent command immediately before and after its acknowledgement
    /// proves both sides of the boundary: the output client remains alive
    /// until `no-output` is visible, then it is reaped without taking down the
    /// private server.
    #[tokio::test]
    async fn shutdown_acks_no_output_despite_unread_positional_replies() {
        let mut server = ScratchServer::start().await;
        let pane = server
            .driver
            .create_session(
                "fh-shutdown-boundary",
                "/",
                80,
                24,
                &[],
                &ticking_pane("BOUNDARY"),
            )
            .await
            .expect("session");
        let gate = Arc::new(DisableOutputGate::default());
        server.driver.disable_output_gate = Some(Arc::clone(&gate));
        let (_modes, _prefill, mut stream) = server
            .driver
            .open_replay_stream("fh-shutdown-boundary", &pane)
            .await
            .expect("replay stream");
        let target = stream.client_target.clone();

        // Queue four replies and prove the group ran without consuming any of
        // them on this client's stdout. This is the interrupted replay shape
        // that makes an in-band shutdown acknowledgement ambiguous.
        let waiter_driver = server.driver.clone();
        let waiter = tokio::spawn(async move {
            waiter_driver
                .run(&["wait-for", "fh-shutdown-replies-ready"])
                .await
        });
        stream
            .send_raw_command(
                "display-message -p '#{pane_id}' ; \
                 capture-pane -p -t : ; \
                 display-message -p '#{session_name}' ; \
                 wait-for -S fh-shutdown-replies-ready",
            )
            .await
            .expect("queueing an unread four-block exchange");
        waiter
            .await
            .expect("joining the group sentinel")
            .expect("the group sentinel must run");

        let shutdown = tokio::spawn(async move { shutdown_test_stream(stream).await });
        tokio::time::timeout(std::time::Duration::from_secs(10), gate.entered.notified())
            .await
            .expect("shutdown must reach the external boundary");
        assert!(
            server
                .driver
                .run(&["list-clients", "-F", "#{client_name}"])
                .await
                .expect("listing clients before the boundary")
                .lines()
                .any(|name| name == target),
            "the output client closed before the safe transition began"
        );

        gate.release.notify_one();
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            gate.acknowledged.notified(),
        )
        .await
        .expect("the external command must acknowledge no-output");
        let clients = server
            .driver
            .run(&["list-clients", "-F", "#{client_name}|#{client_flags}"])
            .await
            .expect("inspecting the acknowledged client");
        let target_line = clients
            .lines()
            .find(|line| line.starts_with(&format!("{target}|")))
            .expect("the client must remain alive until shutdown receives the acknowledgement");
        assert!(
            target_line
                .split_once('|')
                .is_some_and(|(_, flags)| { flags.split(',').any(|flag| flag == "no-output") }),
            "the external command returned without applying no-output: {target_line}"
        );
        assert!(
            !shutdown.is_finished(),
            "the output client was reaped before the acknowledgement returned"
        );

        gate.finish.notify_one();
        tokio::time::timeout(std::time::Duration::from_secs(10), shutdown)
            .await
            .expect("shutdown must finish after acknowledgement")
            .expect("joining output shutdown");
        let clients = server
            .driver
            .run(&["list-clients", "-F", "#{client_name}"])
            .await
            .expect("listing clients after shutdown");
        assert!(
            clients.lines().all(|name| name != target),
            "the acknowledged output client was not reaped"
        );
        assert!(
            server
                .driver
                .run(&["has-session", "-t", "fh-shutdown-boundary"])
                .await
                .is_ok(),
            "safe output-client shutdown must not abort the private server"
        );
    }

    /// Every completed reply boundary of a four-command exchange is safe to stop at.
    ///
    /// Pause recovery reads its replies positionally, so cooperative teardown
    /// can win before the first block or after any one of the four. Each case
    /// leaves a different amount of old protocol state on the output client's
    /// stdout. Reopening a fresh client per boundary proves shutdown never
    /// depends on how many of those old replies the interrupted reader happened
    /// to consume.
    #[tokio::test]
    async fn shutdown_survives_every_four_block_reply_boundary() {
        let server = ScratchServer::start().await;
        let pane = server
            .driver
            .create_session(
                "fh-shutdown-boundaries",
                "/",
                80,
                24,
                &[],
                &["sh".to_string(), "-c".to_string(), "sleep 300".to_string()],
            )
            .await
            .expect("session");

        for replies_read in 0..=4 {
            let (_modes, _prefill, mut stream) = server
                .driver
                .open_replay_stream("fh-shutdown-boundaries", &pane)
                .await
                .unwrap_or_else(|error| {
                    panic!("opening boundary-{replies_read} stream: {error:#}")
                });
            let sentinel = format!("fh-shutdown-boundary-{replies_read}");
            let waiter_driver = server.driver.clone();
            let waiter_sentinel = sentinel.clone();
            let waiter =
                tokio::spawn(
                    async move { waiter_driver.run(&["wait-for", &waiter_sentinel]).await },
                );
            stream
                .send_raw_command(&format!(
                    "display-message -p '#{{pane_id}}' ; \
                     capture-pane -p -t : ; \
                     display-message -p '#{{session_name}}' ; \
                     wait-for -S {sentinel}"
                ))
                .await
                .unwrap_or_else(|error| {
                    panic!("queueing boundary-{replies_read} exchange: {error:#}")
                });
            waiter
                .await
                .expect("joining the boundary sentinel")
                .expect("the boundary exchange must run");

            let deadline = tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT;
            for block in 0..replies_read {
                read_command_block(
                    &mut stream.reader,
                    &mut stream.line,
                    deadline,
                    "shutdown-boundary test exchange",
                    &pane,
                )
                .await
                .unwrap_or_else(|error| {
                    panic!(
                        "reading block {block} before boundary {replies_read} shutdown: {error:#}"
                    )
                });
            }

            shutdown_test_stream(stream).await;
            assert!(
                server
                    .driver
                    .run(&["has-session", "-t", "fh-shutdown-boundaries"])
                    .await
                    .is_ok(),
                "shutdown after {replies_read} replies aborted the private server"
            );
        }
    }

    /// The replay group's command list and ORDER are the contract
    /// `snapshot_then_cutover` reads reply blocks against, and both the
    /// attach and catch-up paths depend on it being identical apart from
    /// the final cutover. Pinned against the complete string so a
    /// reordering — which would silently pair the modes reply with the
    /// history capture — cannot pass, and so nothing can be inserted
    /// between the visible capture and the cutover: their adjacency is
    /// what makes the cutover lossless (see the function's own comment
    /// for the issue-4 attempt this refuses to readmit).
    #[test]
    fn replay_command_group_differs_only_in_its_cutover() {
        let attach = replay_command_group("fh-s", "%1", &attach_cutover_command(&[]));
        let resume = replay_command_group("fh-s", "%1", &continue_pane_command("%1"));
        // Every pane target is the QUOTED session-paired form: a bare
        // `%1` would resolve server-wide, and a pane id that went stale
        // across a tmux-server restart would then capture another
        // session's terminal into this one.
        assert_eq!(
            attach,
            format!(
                "display-message -p -t \"=fh-s:.%1\" '{PANE_MODE_FORMAT}' ; \
                 capture-pane -p -e -N -t \"=fh-s:.%1\" -S -{HISTORY_LIMIT} ; \
                 capture-pane -p -e -N -t \"=fh-s:.%1\" ; \
                 refresh-client -f !no-output,pause-after={TMUX_PAUSE_AFTER_SECS}\n"
            )
        );
        let common = attach
            .strip_suffix(&format!("{}\n", attach_cutover_command(&[])))
            .expect("attach group ends with its cutover");
        assert_eq!(
            resume,
            format!("{common}{}\n", continue_pane_command("%1")),
            "the two groups must share every snapshot command verbatim"
        );
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

    /// A tmux server on a throwaway socket, killed on drop.
    ///
    /// The tests below are the only ones in this module that need a real
    /// tmux — everything else here is deliberately pure. Drop-based
    /// teardown for the same reason the e2e harness uses it: a test that
    /// fails an assertion never reaches an explicit cleanup call, and
    /// leaked tmux servers accumulate across runs.
    struct ScratchServer {
        driver: TmuxDriver,
        /// Also the scratch space tests put out-of-band fixtures in — the
        /// progress files a filtered pane writes so its liveness is
        /// observable when its output reaches nobody (see
        /// [`read_progress`]). It outlives the server by drop order.
        dir: tempfile::TempDir,
        /// Released once this server is gone, letting the next real-tmux
        /// test start its own — see [`REAL_TMUX_SLOTS`]. Declared last so
        /// it is released only after the tempdir and driver have been,
        /// which is after `Drop` has killed the server.
        _slot: tokio::sync::SemaphorePermit<'static>,
    }

    /// Caps how many real-tmux tests in THIS binary run at once.
    ///
    /// libtest runs every test in a binary concurrently and bounds only
    /// the thread count, not what those threads start. Each server below
    /// is a tmux process plus one or two panes running tight `echo` loops
    /// specifically designed to flood — a producer whose whole purpose is
    /// to saturate something. Unbounded, they load the machine enough that
    /// the multi-second timeouts these tests use stop meaning what they
    /// say, and a pane that simply never got scheduled reads as a filter
    /// that failed or a client that vanished.
    ///
    /// Two permits rather than one: these tests are dominated by waiting
    /// on a real tmux, so serializing them outright would roughly double
    /// the suite's wall time for no extra signal, while the step from two
    /// to unbounded is where the flooders start competing. Mirrors the
    /// e2e suite's own `SLOTS`, which exists for the same reason.
    static REAL_TMUX_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

    impl Drop for ScratchServer {
        fn drop(&mut self) {
            let _ = std::process::Command::new("tmux")
                .arg("-S")
                .arg(&self.driver.socket)
                .arg("kill-server")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }

    impl ScratchServer {
        /// Take a concurrency slot, then start a server on a fresh
        /// socket. The slot is acquired here rather than in each test so
        /// that standing up a real tmux is the thing that is bounded —
        /// there is no way to reach one in this module without going
        /// through this constructor.
        async fn start() -> ScratchServer {
            let slot = REAL_TMUX_SLOTS
                .acquire()
                .await
                .expect("semaphore is never closed");
            let dir = tempfile::tempdir().expect("tempdir");
            let driver = TmuxDriver::new(dir.path());
            driver.ensure_server().await.expect("tmux server");
            ScratchServer {
                driver,
                dir,
                _slot: slot,
            }
        }
    }

    /// Pull events until one satisfies `want`, failing rather than
    /// hanging if the stream goes quiet. Returns everything decoded along
    /// the way so callers can assert on the bytes that preceded the event.
    async fn pump_until(
        stream: &mut OutputStream,
        secs: u64,
        want: impl Fn(&OutputEvent) -> bool,
    ) -> Vec<u8> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(secs);
        let mut bytes = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let event = tokio::time::timeout(remaining, stream.next_output())
                .await
                .unwrap_or_else(|_| panic!("stream went quiet after {} bytes", bytes.len()))
                .expect("control stream failed")
                .expect("control client exited");
            if want(&event) {
                return bytes;
            }
            if let OutputEvent::Bytes(chunk) = event {
                bytes.extend_from_slice(&chunk);
            }
        }
    }

    /// Finish an output stream's fail-closed shutdown in real-tmux tests.
    ///
    /// A one-shot failure deliberately returns ownership instead of dropping
    /// the client. Production publishes that reaper through the per-terminal
    /// registry; a direct unit test has no registry, so it must drive the same
    /// retry object itself or risk leaking the very unsafe state under test.
    async fn shutdown_test_stream(stream: OutputStream) {
        if let Err(reaper) = stream.shutdown().await {
            reaper
                .run()
                .await
                .expect("the output client must eventually shut down safely");
        }
    }

    /// The catch-up path against a REAL tmux: a paused pane must surface
    /// as [`OutputEvent::Paused`], and
    /// [`OutputStream::resume_paused_with_replay`] must both hand back a
    /// usable replay and get the live stream flowing again.
    ///
    /// This is the one place the whole tmux-facing half of PLAN_M2_5.md's
    /// deep-stall recovery is exercised end to end, and it earns a real
    /// tmux because every interesting part is tmux's behavior, not ours:
    /// that a paused pane goes quiet, that a snapshot command group can
    /// still run cleanly on that same client while it is paused (the
    /// ordering the resume path depends on — continuing first would race
    /// resumed output into the snapshot), and that the continue command's
    /// `%end` is where live output picks back up.
    ///
    /// The pause is forced with tmux's on-demand `refresh-client -A
    /// <pane>:pause` rather than by starving the client until
    /// `pause-after` fires. That is not a shortcut, it is the only
    /// DETERMINISTIC option: whether the delay-driven pause triggers
    /// depends on how far tmux happens to have read ahead of a client that
    /// then stalls, and a client that was caught up when it stopped
    /// reading leaves tmux nothing buffered to age — tmux then throttles
    /// the pane instead and never pauses at all. Both outcomes were
    /// observed on every supported tmux generation (3.3a, 3.4, 3.7b) in
    /// repeated identical trials, so starving the client would make this a
    /// coin flip. The pane state reached is identical either way, which is
    /// what this test is about; the e2e suite covers the delay-driven path
    /// end to end, tolerating both outcomes.
    #[tokio::test]
    async fn a_paused_pane_surfaces_and_recovers_through_the_replay_path() {
        let server = ScratchServer::start().await;
        let pane = server
            .driver
            .create_session(
                "fh-pause-test",
                "/",
                80,
                24,
                &[],
                // A producer fast enough that "the stream went quiet"
                // genuinely means paused, and slow enough not to bury the
                // test in output.
                &[
                    "sh".to_string(),
                    "-c".to_string(),
                    "i=0; while :; do echo PAUSETEST-$i; i=$((i+1)); done".to_string(),
                ],
            )
            .await
            .expect("session");
        let (_modes, _prefill, mut stream) = server
            .driver
            .open_replay_stream("fh-pause-test", &pane)
            .await
            .expect("replay stream");

        // Prove output is flowing before pausing, so the quiet below is
        // attributable to the pause rather than to a pane that never
        // started.
        pump_until(&mut stream, 10, |event| {
            matches!(event, OutputEvent::Bytes(bytes) if bytes.windows(10).any(|w| w == b"PAUSETEST-"))
        })
        .await;

        stream
            .send_raw_command(&format!("refresh-client -A \"{pane}:pause\""))
            .await
            .expect("pause command");
        pump_until(&mut stream, 10, |event| *event == OutputEvent::Paused).await;
        // Consume the pause command's own closing marker before running
        // the replay group, which reads reply blocks positionally — see
        // `drain_command_reply` for why the notification arrives inside
        // that block, and why this is awaited explicitly rather than
        // approximated with a timeout on a read that is not cancel-safe.
        stream
            .drain_command_reply()
            .await
            .expect("pause command reply");

        let (modes, content) = stream
            .resume_paused_with_replay()
            .await
            .expect("catch-up replay");
        assert!(
            content.windows(10).any(|window| window == b"PAUSETEST-"),
            "the catch-up replay must carry the pane's history, not an empty capture"
        );
        assert!(
            !modes.pane_dead,
            "test premise: the producer must still be alive after the catch-up"
        );

        // Live output resuming is the other half of the contract: a
        // continue that returned a snapshot but left the pane paused
        // would look identical up to here and leave the terminal dead.
        pump_until(&mut stream, 10, |event| {
            matches!(event, OutputEvent::Bytes(bytes) if bytes.windows(10).any(|w| w == b"PAUSETEST-"))
        })
        .await;
        shutdown_test_stream(stream).await;
    }

    /// Read this stream continuously until `ticks` of its OWN pane's
    /// heartbeat have arrived, failing on anything that is not a clean
    /// read.
    ///
    /// Every loop here drives [`OutputStream::next_output`] to completion
    /// rather than racing it against a timer. That is not style: that
    /// future is documented as NOT cancel-safe, so a `timeout(_,
    /// next_output())` loop — the obvious way to "read for a while" —
    /// abandons a partially-read line on every tick and, now that the
    /// foreign-pane path writes, can abandon a partially-written command
    /// too. A test built that way exercises a state production never
    /// reaches and can pass or fail for reasons unrelated to its subject.
    /// Own-pane ticks are the clock instead, which also makes the
    /// observation window a number of PROVEN round trips rather than a
    /// duration that a loaded machine can turn into nothing.
    ///
    /// An `Err` or an EOF fails outright. Both mean the control client is
    /// gone, and a loop that swallowed them would keep counting an empty
    /// stream as "no foreign notifications arrived" — the exact reading
    /// these tests are trying to earn. That failure is reported through
    /// [`client_gone_report`] rather than a bare `expect`, because it has
    /// happened on CI three times and produced no evidence whatsoever
    /// about WHY the client vanished.
    ///
    /// `server` is taken purely for that report: the stream itself cannot
    /// say anything about the tmux server behind it.
    async fn pump_own_pane_ticks(
        server: &ScratchServer,
        stream: &mut OutputStream,
        ticks: usize,
        secs: u64,
    ) {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(secs);
        let mut seen = 0;
        while seen < ticks {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let event = tokio::time::timeout(remaining, stream.next_output())
                .await
                .unwrap_or_else(|_| {
                    panic!("only {seen} of {ticks} own-pane ticks arrived within {secs}s")
                });
            let event = match event {
                Ok(Some(event)) => event,
                Ok(None) => {
                    let report = client_gone_report(server, stream).await;
                    panic!("the control client exited after {seen} of {ticks} ticks; {report}");
                }
                Err(e) => {
                    let report = client_gone_report(server, stream).await;
                    panic!(
                        "the control stream failed after {seen} of {ticks} ticks: {e:#}; \
                         {report}"
                    );
                }
            };
            match event {
                OutputEvent::Bytes(bytes) if contains(&bytes, b"AGENT-TICK") => seen += 1,
                OutputEvent::Bytes(_) => {}
                // Recovered exactly the way production recovers (the
                // forwarder's `catch_up_after_tmux_pause`), and then the
                // tick count simply carries on. A pause is a legitimate
                // thing for tmux to do to a client that fell behind — this
                // helper's own callers can provoke one by flooding a
                // neighbouring pane — and treating it as fatal, or merely
                // failing to resume, would turn it into a 30-second "ticks
                // never arrived" that blames the wrong mechanism entirely.
                // The replay's own content is deliberately NOT counted:
                // those are history, not the fresh round trips these tests
                // use as their clock.
                OutputEvent::Paused => {
                    stream
                        .resume_paused_with_replay()
                        .await
                        .expect("a paused own pane must recover through the replay path");
                }
            }
        }
    }

    /// Everything cheap to know about why a control client is no longer
    /// there, gathered at the moment of failure.
    ///
    /// Exists because the alternative is what this suite actually had: a
    /// bare "the control client must not exit mid-test", which says only
    /// that the thing being measured stopped existing. Each piece answers
    /// a different candidate cause — tmux's own `%exit` reason says
    /// whether tmux announced the end or the pipe simply died; the child's
    /// wait status distinguishes a crashed or signalled `tmux -C` from one
    /// still running with a broken pipe; and the server probes say whether
    /// the whole scratch server went away underneath it (the shape a
    /// loaded CI box, an OOM kill, or a stray `kill-server` takes).
    ///
    /// Every probe is best-effort and its failure is folded into the text
    /// rather than raised: this runs on a path that is already panicking,
    /// and a probe that panics first would destroy the very report it was
    /// gathering. The same reasoning is why the two tmux probes are
    /// bounded by [`REPORT_PROBE_TIMEOUT`] — the most likely reason a
    /// control client vanished is a tmux in trouble, and asking a wedged
    /// tmux a question through the UNBOUNDED [`TmuxDriver::run`] would
    /// turn a test that was about to fail with a diagnosis into one that
    /// hangs until the harness kills it.
    async fn client_gone_report(server: &ScratchServer, stream: &mut OutputStream) -> String {
        let exit_reason = match stream.exit_reason.as_deref() {
            Some("") => "tmux announced a bare %exit with no reason".to_string(),
            Some(reason) => format!("tmux said: %exit {reason}"),
            None => "clean EOF, no %exit was ever announced".to_string(),
        };
        let child = match stream.child.try_wait() {
            Ok(Some(status)) => format!("the tmux -C child exited: {status}"),
            Ok(None) => "the tmux -C child is still running".to_string(),
            Err(e) => format!("the tmux -C child could not be waited on: {e}"),
        };
        let sessions = probe(
            server,
            "sessions",
            &["list-sessions", "-F", "#{session_name}"],
        )
        .await;
        let clients = probe(
            server,
            "clients",
            &["list-clients", "-F", "#{client_flags}"],
        )
        .await;
        format!("{exit_reason}; {child}; {sessions}; {clients}")
    }

    /// How long one of [`client_gone_report`]'s tmux probes may take
    /// before the report records that it hung instead of what it found.
    ///
    /// Short on purpose. These questions are one subprocess round trip
    /// against a server that is, at worst, on the same box — anything
    /// slower is itself the answer, and the panic this report decorates is
    /// already overdue by the time it runs.
    const REPORT_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

    /// One bounded tmux query for [`client_gone_report`], rendered as a
    /// report fragment whether it answered, failed, or hung.
    async fn probe(server: &ScratchServer, label: &str, args: &[&str]) -> String {
        match tokio::time::timeout(REPORT_PROBE_TIMEOUT, server.driver.run(args)).await {
            Ok(Ok(out)) => format!("{label}: [{}]", out.trim().replace('\n', ", ")),
            Ok(Err(e)) => format!("{label} query failed: {e:#}"),
            Err(_) => format!(
                "{label} query did not answer within {REPORT_PROBE_TIMEOUT:?} — tmux itself \
                 is not responding"
            ),
        }
    }

    /// A session whose OTHER window is flooding must deliver none of that
    /// flood to a terminal's control client — the tmux-side pane filter
    /// doing its job, against a real tmux.
    ///
    /// The local drop in `next_output` makes the terminal's BYTES correct
    /// whether or not the filter was ever installed, so "the right output
    /// arrived" proves nothing here. `foreign_dropped` is the only
    /// observable that distinguishes the two: a filter that silently
    /// stopped being applied would show up as a number climbing with the
    /// neighbour's output rate, and as nothing else at all until a busy
    /// tab started delaying somebody's agent.
    ///
    /// Two things beyond the count are asserted, because without them a
    /// PASS would be ambiguous. The sink's drain task must still be
    /// running: if it had ended, the filter's safety net would be gone and
    /// the reason no foreign notifications arrived might be that tmux had
    /// stopped reading the neighbour entirely. And the neighbour itself
    /// must still be making progress, observed outside tmux through a file
    /// it writes — the difference between "filtered" and "frozen", which
    /// is invisible from this client's stream by construction.
    #[tokio::test]
    async fn a_flooding_neighbour_reaches_a_terminals_control_client_not_at_all() {
        let server = ScratchServer::start().await;
        let progress = server.dir.path().join("neighbour-progress");
        let agent = server
            .driver
            .create_session("fh-filter-test", "/", 80, 24, &[], &ticking_pane("AGENT"))
            .await
            .expect("session");
        let (_window, flooder) = server
            .driver
            .new_window(
                "fh-filter-test",
                "/",
                &[],
                &bursting_pane("NEIGHBOUR", &progress),
            )
            .await
            .expect("a second window to flood from");
        assert_ne!(agent, flooder, "test premise: two distinct panes");

        // The sink first, exactly as the attach handler orders it: without
        // one, the filter this test is about would stop tmux reading the
        // neighbour's pane instead of merely not forwarding it.
        let mut sink = server
            .driver
            .open_session_sink("fh-filter-test")
            .await
            .expect("session sink");
        let sink_task = tokio::spawn(async move { sink.drain().await });

        let (_modes, _prefill, mut stream) = server
            .driver
            .open_replay_stream("fh-filter-test", &agent)
            .await
            .expect("replay stream");
        // Settle first: the neighbour can legitimately speak between the
        // pane listing and the cutover that filters it, so the count taken
        // over a FRESH window afterwards is the one that means anything.
        pump_own_pane_ticks(&server, &mut stream, 5, 30).await;
        let settled = stream.foreign_dropped;
        let before = read_progress(&progress);

        // Long enough to span several of the neighbour's bursts.
        pump_own_pane_ticks(&server, &mut stream, 15, 30).await;

        assert_eq!(
            stream.foreign_dropped,
            settled,
            "a flooding neighbour reached this terminal's control client {} more times after \
             the filter should have been in force; tmux's own pane filter is not being applied",
            stream.foreign_dropped - settled
        );
        assert!(
            !sink_task.is_finished(),
            "the sink stopped draining during the test, so a quiet stream proves nothing"
        );
        assert!(
            read_progress(&progress) > before,
            "the filtered neighbour stopped making progress — it was frozen, not filtered"
        );
        shutdown_test_stream(stream).await;
    }

    /// The sink is what keeps a pane every terminal has filtered off from
    /// being frozen by tmux — asserted as a MECHANISM, both directions.
    ///
    /// Every other test here would pass if the filter quietly stopped
    /// being installed. This one fails in that case and in the opposite
    /// one, because it checks the thing the filter is dangerous without: a
    /// pane no attached control client wants is a pane tmux stops reading,
    /// which blocks its process on its next write. The busy pane is turned
    /// off on the ONLY terminal client, and its progress is watched from
    /// outside tmux.
    ///
    /// The negative half runs first and is what gives the positive half
    /// its meaning: with the sink killed, the same configuration must
    /// freeze the pane. Without that control, "the pane kept running"
    /// could just mean the filter never took effect.
    #[tokio::test]
    async fn only_the_sink_keeps_a_filtered_pane_readable() {
        let server = ScratchServer::start().await;
        let progress = server.dir.path().join("busy-progress");
        let agent = server
            .driver
            .create_session("fh-sink-mech", "/", 80, 24, &[], &ticking_pane("AGENT"))
            .await
            .expect("session");
        server
            .driver
            .new_window("fh-sink-mech", "/", &[], &bursting_pane("BUSY", &progress))
            .await
            .expect("a busy window");

        // NO sink: the terminal client's filter is the only opinion tmux
        // has about the busy pane, and the man page says that stops the
        // read. Measured, not assumed.
        let (_modes, _prefill, mut stream) = server
            .driver
            .open_replay_stream("fh-sink-mech", &agent)
            .await
            .expect("replay stream");
        pump_own_pane_ticks(&server, &mut stream, 5, 30).await;
        let before = read_progress(&progress);
        pump_own_pane_ticks(&server, &mut stream, 25, 60).await;
        // At most ONE, not zero. The producer writes its counter and THEN
        // starts the burst that fills the pty, so a pane frozen by this
        // filter can still be one step past where it was when the filter
        // landed — it gets to announce the cycle it then blocks inside.
        // What it cannot do is keep cycling, which is what the sinked half
        // below shows it doing.
        let frozen_at = read_progress(&progress);
        assert!(
            frozen_at <= before + 1,
            "test premise: with no sink, a pane filtered off by every client must freeze — it \
             advanced from {before} to {frozen_at}. If this ever stops holding, the sink's \
             whole justification needs re-auditing"
        );

        // Now the sink, on the same already-frozen pane: attaching a
        // client that wants everything must set it running again.
        let mut sink = server
            .driver
            .open_session_sink("fh-sink-mech")
            .await
            .expect("session sink");
        let sink_task = tokio::spawn(async move { sink.drain().await });
        pump_own_pane_ticks(&server, &mut stream, 25, 60).await;
        let running_at = read_progress(&progress);
        assert!(
            running_at >= frozen_at + 2,
            "the sink did not restart the filtered pane's reads: {frozen_at} -> {running_at} \
             over a window in which an unblocked producer completes several cycles"
        );
        assert!(
            !sink_task.is_finished(),
            "the sink stopped draining, so the recovery above proves nothing"
        );
        shutdown_test_stream(stream).await;
    }

    /// The counter a [`bursting_pane`] keeps outside tmux, or 0 before it
    /// has written one.
    ///
    /// Read from a file rather than from the pane's own output because the
    /// whole question is what happens when that output is NOT being
    /// delivered to anyone the test can see. A frozen pane and a filtered
    /// pane look identical on a control client; they differ only here.
    fn read_progress(path: &std::path::Path) -> u64 {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(0)
    }

    /// A pane command that prints `<label>-TICK` five times a second,
    /// forever — a terminal whose liveness a test can keep re-checking
    /// instead of racing one echo.
    fn ticking_pane(label: &str) -> Vec<String> {
        vec![
            "sh".to_string(),
            "-c".to_string(),
            format!("while :; do echo {label}-TICK; sleep 0.2; done"),
        ]
    }

    /// A pane command that emits bursts of 5000 lines a second apart, and
    /// records how many bursts it has completed in `progress`.
    ///
    /// Bursty rather than free-running on purpose. An unpaced `echo` loop
    /// pins one tmux server at 100% CPU for as long as the test runs, and
    /// with this suite's parallelism that starved unrelated panes badly
    /// enough to make a quiet terminal's own output arrive seconds late —
    /// the test's producer becoming the test's flakiness. A burst is just
    /// as unmistakable to a filter that is not working (thousands of
    /// notifications per second while it lasts) at a fraction of the
    /// average cost.
    ///
    /// The counter is written BEFORE each burst rather than after, so it
    /// advances as soon as the pane is running again, and it is the only
    /// evidence a test has that the pane is alive at all once its output
    /// is filtered away from every client (see [`read_progress`]). A pane
    /// tmux has stopped reading blocks inside the burst, mid-`echo`, and
    /// so simply stops updating it.
    fn bursting_pane(label: &str, progress: &std::path::Path) -> Vec<String> {
        let progress = progress.display();
        vec![
            "sh".to_string(),
            "-c".to_string(),
            format!(
                "n=0; while :; do n=$((n+1)); echo $n > {progress}; \
                 i=0; while [ $i -lt 5000 ]; do echo {label}-$i; i=$((i+1)); done; \
                 sleep 1; done"
            ),
        ]
    }

    /// Whether `haystack` contains `needle`, for assertions over decoded
    /// pane bytes.
    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    /// A pane created AFTER a terminal attached is silenced on its first
    /// notification, not left streaming for the life of the attachment.
    ///
    /// `refresh-client -A` names one pane and has no wildcard, so the
    /// attach-time filter can only cover panes that already exist — which
    /// makes "the user opens a tab while watching the agent" precisely the
    /// case the attach-time pass cannot reach. Without the late path, that
    /// tab's entire output would be delivered to, and discarded by, every
    /// other terminal of the session for as long as it stayed open.
    ///
    /// Asserts on the count STOPPING rather than on it being small: some
    /// notifications necessarily arrive before the filter can be sent (the
    /// first one is what triggers it), so the property is that the stream
    /// converges to exactly zero new ones, and a late filter that never
    /// took effect would keep climbing with the producer.
    #[tokio::test]
    async fn a_pane_created_after_attach_is_silenced_when_it_first_speaks() {
        let server = ScratchServer::start().await;
        let progress = server.dir.path().join("late-progress");
        let agent = server
            .driver
            .create_session("fh-late-filter", "/", 80, 24, &[], &ticking_pane("AGENT"))
            .await
            .expect("session");
        let mut sink = server
            .driver
            .open_session_sink("fh-late-filter")
            .await
            .expect("session sink");
        let sink_task = tokio::spawn(async move { sink.drain().await });
        let (_modes, _prefill, mut stream) = server
            .driver
            .open_replay_stream("fh-late-filter", &agent)
            .await
            .expect("replay stream");

        // Only now does the neighbour exist — the shape the attach-time
        // filter provably cannot have covered.
        server
            .driver
            .new_window(
                "fh-late-filter",
                "/",
                &[],
                &bursting_pane("LATE", &progress),
            )
            .await
            .expect("a late window to flood from");

        // Settle: the first notifications from the new pane are what
        // TRIGGER the filter, so they are expected and are not what this
        // measures.
        pump_own_pane_ticks(&server, &mut stream, 10, 30).await;
        let settled = stream.foreign_dropped;
        assert!(
            settled > 0,
            "test premise: the late pane must have reached this client at least once, or the \
             late path was never exercised"
        );
        let before = read_progress(&progress);

        pump_own_pane_ticks(&server, &mut stream, 15, 30).await;
        assert_eq!(
            stream.foreign_dropped,
            settled,
            "a pane created after attach kept streaming to this terminal ({} more notifications \
             after it should have been silenced)",
            stream.foreign_dropped - settled
        );
        assert!(
            !sink_task.is_finished(),
            "the sink stopped draining during the test, so a quiet stream proves nothing"
        );
        assert!(
            read_progress(&progress) > before,
            "the late pane stopped making progress — it was frozen, not filtered"
        );
        shutdown_test_stream(stream).await;
    }

    /// A filter command written while output is live must not desynchronize
    /// the catch-up replay that may follow it immediately.
    ///
    /// The two share one stdout and read it differently: the filter's reply
    /// arrives as chatter whenever it happens to arrive, while
    /// `resume_paused_with_replay` reads four blocks POSITIONALLY. A filter
    /// reply still in flight when the catch-up starts would be taken for the
    /// modes block, shifting every block after it — a replay that returns
    /// the history capture as its mode string and the visible screen as its
    /// history, silently. This is the interleaving that actually orders
    /// them, and the reason `pending_filter_replies` is counted rather than
    /// assumed to have cleared.
    ///
    /// # Why the debt is created directly rather than raced for
    ///
    /// The natural setup — let a late pane speak, then pause — cannot
    /// reach the state under test against a real tmux, and that is worth
    /// recording so nobody "fixes" this into a race. `next_output` keeps
    /// reading until it has an event for its OWN pane, so the filter it
    /// writes for a late pane and the reply to that filter are both
    /// consumed inside the same call, hundreds of milliseconds before the
    /// call returns and the test regains control. The debt is real, and it
    /// is settled correctly; there is simply no instant a test can observe
    /// it from outside. Writing the command directly puts the stream in
    /// exactly the state a filter written moments before a `%pause` leaves
    /// it in, which is the state the catch-up path has to survive.
    #[tokio::test]
    async fn a_late_pane_filter_does_not_desynchronize_the_catch_up_replay() {
        let server = ScratchServer::start().await;
        let progress = server.dir.path().join("racing-progress");
        let agent = server
            .driver
            .create_session(
                "fh-filter-race",
                "/",
                80,
                24,
                &[],
                // A distinctive, continuous marker so the replay's content
                // can be recognized in the capture below.
                &ticking_pane("AGENT"),
            )
            .await
            .expect("session");
        let mut sink = server
            .driver
            .open_session_sink("fh-filter-race")
            .await
            .expect("session sink");
        let sink_task = tokio::spawn(async move { sink.drain().await });
        let (_window, neighbour) = server
            .driver
            .new_window(
                "fh-filter-race",
                "/",
                &[],
                &bursting_pane("RACER", &progress),
            )
            .await
            .expect("a neighbour window");
        let (_modes, _prefill, mut stream) = server
            .driver
            .open_replay_stream("fh-filter-race", &agent)
            .await
            .expect("replay stream");
        pump_own_pane_ticks(&server, &mut stream, 3, 30).await;

        // A real filter command, left with its reply outstanding.
        stream
            .send_filter_command(&silence_live_pane_command(std::slice::from_ref(&neighbour)))
            .await
            .expect("writing a pane filter");
        assert_eq!(
            stream.pending_filter_replies, 1,
            "a written filter command must be recorded as owed a reply"
        );

        stream
            .send_raw_command(&format!("refresh-client -A \"{agent}:pause\""))
            .await
            .expect("pause command");
        pump_until(&mut stream, 30, |event| *event == OutputEvent::Paused).await;
        stream
            .drain_command_reply()
            .await
            .expect("pause command reply");

        // The catch-up must settle the filter debt first; if it did not,
        // this returns the wrong block for every field it parses.
        let (modes, content) = stream
            .resume_paused_with_replay()
            .await
            .expect("catch-up replay");
        assert!(
            contains(&content, b"AGENT-TICK"),
            "the catch-up replay returned something other than this pane's history — the \
             positional block reads were shifted by an unconsumed filter reply"
        );
        assert!(
            !modes.pane_dead,
            "test premise: the ticking pane must still be alive"
        );
        assert_eq!(
            stream.pending_filter_replies, 0,
            "the catch-up must have settled the filter debt, not merely worked around it"
        );
        assert!(!sink_task.is_finished(), "the sink must still be draining");
        shutdown_test_stream(stream).await;
    }

    /// Poll `capture_pane_tail` until the pane has actually rendered
    /// `want`, rather than sampling once and racing tmux's own writes.
    ///
    /// Returns the tail that satisfied the wait so the caller can assert
    /// on the same bytes it waited for — sampling twice would reintroduce
    /// exactly the race this exists to close.
    async fn tail_containing(driver: &TmuxDriver, session: &str, pane: &str, want: &str) -> String {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let tail = driver
                .capture_pane_tail(session, pane, 64 * 1024)
                .await
                .expect("capture the pane's visible grid");
            if tail.contains(want) {
                return tail;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the pane never rendered {want:?}; last tail was:\n{tail}"
            );
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
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
