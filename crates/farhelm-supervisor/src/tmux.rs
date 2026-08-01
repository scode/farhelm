//! The tmux driver: a private, headless tmux server used purely as a PTY
//! holder and history store (SPEC_impl.md, "Terminal substrate").
//!
//! Nothing here ever runs a rendering `tmux attach`. Output is consumed
//! through a non-rendering control-mode client (`tmux -C`) — see
//! [`OutputStream`] — and input goes in as `send-keys -H` commands carried
//! by a SECOND, dedicated no-output control-mode client — see
//! [`InputClient`] for why a second client, rather than the output
//! client's stdin, is what carries input. Sizing is pinned by explicit
//! `resize-window` calls, and replay is `capture-pane -e` history plus
//! re-synthesized pane modes. The motivations — native xterm.js scrolling,
//! no tmux UI anywhere — are recorded in SPEC_impl.md; this module is that
//! design made concrete.

use anyhow::{Context, bail};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
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
/// Five seconds is long enough that an ordinary watermark pause rarely
/// trips this — tripping is not an error, but on the second path it costs
/// a full reset-and-replay catch-up — and short enough that a genuinely
/// wedged client cannot make the tmux server grow its own memory for
/// long. Without the flag at all, an undrained control client grows the
/// tmux server's RSS without bound (audited 2026-07-29: ~3.5 MB/s against
/// a `yes` pane on 3.4), which is the failure this exists to close.
pub const TMUX_PAUSE_AFTER_SECS: u64 = 5;

/// One tmux control-mode notification may expand each terminal byte into
/// a four-byte octal escape. Bound the line above the protocol frame cap
/// without rejecting the largest valid escaped notification.
const MAX_CONTROL_LINE: usize = farhelm_proto::MAX_FRAME_LEN as usize * 4 + 1024;

/// One deadline covers attaching the control client, taking the replay
/// snapshot, and enabling live output. A wedged tmux command must fail
/// the attach request instead of leaving it holding the global
/// attachment lock forever.
const CONTROL_EXCHANGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

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
const PANE_MODE_FORMAT: &str = "#{alternate_on},#{bracket_paste_flag},#{mouse_any_flag},\
                                #{mouse_button_flag},#{mouse_standard_flag},#{mouse_sgr_flag},\
                                #{cursor_flag},#{keypad_cursor_flag},#{cursor_x},#{cursor_y},\
                                #{pane_dead}";

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
fn attach_cutover_command() -> String {
    format!("refresh-client -f !no-output,pause-after={TMUX_PAUSE_AFTER_SECS}")
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

/// The replay command group both the attach and the catch-up paths
/// submit through their control client, differing only in `cutover` —
/// the final command whose `%end` is the replay/live boundary.
///
/// Shared rather than duplicated because the four commands and their
/// ORDER are the contract [`OutputStream::snapshot_then_cutover`] reads
/// replies against: modes, history snapshot, visible snapshot, cutover.
/// A second copy of this string for the resume path would be a second
/// place for that block-count contract to drift.
fn replay_command_group(pane: &str, cutover: &str) -> String {
    format!(
        "display-message -p -t {pane} '{PANE_MODE_FORMAT}' ; \
         capture-pane -p -e -N -t {pane} -S -{HISTORY_LIMIT} ; \
         capture-pane -p -e -N -t {pane} ; \
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
    pub mouse_any: bool,
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
        let mouse_any = flag(false);
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
            mouse_any,
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
        if self.mouse_standard {
            s.push_str("\x1b[?1000h");
        }
        if self.mouse_button {
            s.push_str("\x1b[?1002h");
        }
        if self.mouse_any {
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

/// Outcome of [`TmuxDriver::capture_alt_screen_if_active`]: whether there
/// is a snapshot worth storing, and if not, why not — the caller
/// (service.rs's `StopSession` handler) needs the distinction to decide
/// what, if anything, to log.
///
/// `NotAlternate` and `SessionMismatch` are both deliberately NOT
/// `anyhow::Error`s: a primary-screen pane and a stale pane id are
/// ordinary, expected outcomes of calling this on every stop (regardless
/// of screen state), not failures of the tmux call itself. `TooLarge` is
/// the third such outcome, added for the size-bounded reader — see that
/// method's docs.
pub enum AltScreenCapture {
    /// The pane genuinely belongs to the queried session and was on the
    /// alternate screen at capture time. Normalized and ready to store or
    /// replay.
    Captured(Vec<u8>),
    /// The pane was on the PRIMARY screen — nothing to snapshot; its real
    /// scrollback already survives a kill on its own.
    NotAlternate,
    /// The captured pane's `#{session_name}` did not match the session
    /// this call was made for. Pane ids (`%N`) are assigned by a
    /// server-wide counter that resets whenever the private tmux server
    /// restarts, so a caller holding a pane id from before a restart can
    /// otherwise capture (and store!) a DIFFERENT session's screen under
    /// this session's name — the same stale-pane-id hazard `pane_process`
    /// guards against with its own `#{session_name}` check.
    SessionMismatch,
    /// The capture exceeded the caller-supplied `max_bytes` cap — either
    /// the combined invocation's RAW output did, before it even finished
    /// (the tmux child was killed and whatever had been read so far was
    /// discarded rather than kept growing; this can happen regardless of
    /// whether the pane actually turns out to be alternate or primary —
    /// see the method's own docs for why that is an accepted cost of the
    /// single-invocation design, not a bug), or the SANITIZED body did
    /// once [`sanitize_snapshot_lines`] finished growing it with
    /// boundary resets and restores (checked in
    /// [`parse_alt_screen_capture`], against the same cap, so a capture
    /// that just barely cleared the raw-stream check can never still
    /// slip a just-barely-over-cap file onto disk).
    TooLarge,
}

/// One read-buffer's worth of bytes pulled per `read` call while
/// streaming a bounded capture. 64 KiB balances syscall count against
/// wasted allocation for the common case (a snapshot far under the cap).
const ALT_SCREEN_READ_CHUNK: usize = 64 * 1024;

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

/// Whether a byte length is within a snapshot size cap — INCLUSIVE at the
/// cap itself, rejecting anything past it.
///
/// Factored out as its own pure decision, used identically by the
/// capture-side bounded reader below (`capture_alt_screen_if_active`) and
/// the replay-side bounded file read (service.rs's
/// `read_bounded_snapshot_file`), so both share one definition of "at the
/// cap" vs. "one byte over" — and so that boundary is unit-testable
/// directly, without spawning tmux or touching the filesystem.
pub(crate) fn within_snapshot_cap(len: usize, cap: usize) -> bool {
    len <= cap
}

/// Parse the combined `display-message ; capture-pane` invocation's raw
/// output into an [`AltScreenCapture`] outcome.
///
/// Split out from [`TmuxDriver::capture_alt_screen_if_active`] purely so
/// this parsing — locating the header's newline, reading the
/// `#{alternate_on}`/`#{session_name}` fields, matching the session, and
/// slicing off the capture body — is unit-testable against constructed
/// byte buffers, the same way service.rs's `environ_contains_marker` is
/// split out from its own `/proc`-walking caller. `out` is assumed to
/// already be within the RAW-stream size cap (the caller's read loop
/// enforces that before this ever runs); `max_bytes` is passed through
/// again here, this time as the budget [`sanitize_snapshot_lines`] itself
/// enforces against the GROWN, sanitized body — see below.
///
/// The `Captured` body is run through [`sanitize_snapshot_lines`] on top
/// of the ordinary [`normalize_capture`] — deliberately HERE, at
/// capture-normalization time, and not at replay time: this is the one
/// path that produces bytes destined to become a STORED snapshot (the
/// file and the stop-in-progress pending-map entry both), so sanitizing
/// before either of those ever sees the bytes means both inherit
/// hygienic content for free, and replay code needs no awareness that
/// anything was changed. The ordinary attach prefill calls
/// `normalize_capture` directly (see its other call site) and is
/// untouched — its trailing-attribute behavior predates this transform
/// and is not the bug this closes.
///
/// Sanitizing GROWS the body (a reset and, per boundary, a restore —
/// see `sanitize_snapshot_lines`'s docs), so a capture that cleared the
/// caller's RAW-stream cap by a hair can still end up over `max_bytes`
/// once sanitized. `sanitize_snapshot_lines` takes `max_bytes` as its
/// OWN budget and aborts (`None`) the instant any single append would
/// cross it, rather than building the complete oversized frame first and
/// only rejecting it afterward — an over-cap capture with thousands of
/// boundaries would otherwise transiently allocate far more than
/// `max_bytes` ever permits before a post-hoc check could catch it.
fn parse_alt_screen_capture(out: &[u8], session: &str, max_bytes: usize) -> AltScreenCapture {
    let split_at = out.iter().position(|&b| b == b'\n').unwrap_or(out.len());
    let header = String::from_utf8_lossy(&out[..split_at]);
    let mut fields = header.split_whitespace();
    let alternate_on = fields.next() == Some("1");
    let found_session = fields.next().unwrap_or_default();
    if found_session != session {
        return AltScreenCapture::SessionMismatch;
    }
    if !alternate_on {
        return AltScreenCapture::NotAlternate;
    }
    // `split_at` points at the header's own newline; the capture-pane
    // output (if any) starts one byte past it. An empty pane (no capture
    // output at all, though `capture-pane` always emits at least a blank
    // final row in practice) would leave nothing past `split_at`, which
    // `get` turns into an empty slice rather than a panic.
    let capture = out.get(split_at + 1..).unwrap_or(&[]);
    match sanitize_snapshot_lines(&normalize_capture(capture), max_bytes) {
        Some(sanitized) => AltScreenCapture::Captured(sanitized),
        None => AltScreenCapture::TooLarge,
    }
}

/// Upper bound, in bytes, on the SGR sequences [`CarryoverLog`] holds at
/// once — a FIDELITY bound, distinct from the total-output BYTE BUDGET
/// [`sanitize_snapshot_lines`] separately enforces (its own `budget`
/// parameter, checked via [`try_append`] on every write). This constant
/// only ever governs how much state one line boundary's restore can
/// carry; it has no bearing on how large the finished snapshot is
/// allowed to get.
///
/// A real terminal app resets or re-styles far more often than this in
/// practice (this has never been observed to bind against genuine
/// `capture-pane` output); the bound exists purely so a pathological
/// capture — thousands of distinct, never-reset SGR sequences packed
/// into one row — cannot make one line boundary's restore grow without
/// bound. Crossing it is not an error: see [`CarryoverLog::record`] for
/// the recovery semantics that follow.
const MAX_SGR_CARRYOVER_LOG_BYTES: usize = 4 * 1024;

/// Every SGR (`CSI ... m`) sequence seen since the most recent full
/// reset, in the order [`sanitize_snapshot_lines`] encountered them —
/// replaying this verbatim right after a line boundary reproduces
/// whatever attribute state a real terminal would still be carrying
/// into the next row. See that function's own docs for why "replay the
/// deltas since the last reset" is sufficient without this needing to
/// understand what any individual parameter means.
struct CarryoverLog {
    bytes: Vec<u8>,
    /// Set once `bytes` would have exceeded [`MAX_SGR_CARRYOVER_LOG_BYTES`].
    /// See [`record`](Self::record) for exactly what this suppresses and
    /// how it clears.
    overflowed: bool,
}

impl CarryoverLog {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            overflowed: false,
        }
    }

    /// Feed every SGR sequence found in `line`, in order, updating the
    /// log and its overflow state.
    ///
    /// A full reset (`ESC[0m` and its siblings — see
    /// [`sgr_is_full_reset`]) always clears both the log and any prior
    /// overflow: it is, definitionally, the one point at which "the
    /// running state" becomes exactly "whatever this sequence sets",
    /// discarding everything carried before it. Once overflowed,
    /// non-resetting sequences are dropped on the floor rather than
    /// appended — growing an already-too-large log further would only
    /// make the eventual truncation less predictable, and there is
    /// nothing useful left to restore anyway once fidelity is already
    /// lost for this stretch.
    ///
    /// The tradeoff this pins: a pathological run of distinct,
    /// never-reset styles loses ALL continuation styling (not just the
    /// overflowing part) from the moment it overflows until the source
    /// stream's own next full reset — bounded and predictable, rather
    /// than an unbounded log or a silently-partial, order-losing one.
    fn record(&mut self, line: &[u8]) {
        for_each_sgr_sequence(line, |sequence| {
            if sgr_is_full_reset(sequence) {
                self.bytes.clear();
                self.overflowed = false;
            } else if self.overflowed {
                return;
            }
            self.bytes.extend_from_slice(sequence);
            if self.bytes.len() > MAX_SGR_CARRYOVER_LOG_BYTES {
                self.bytes.clear();
                self.overflowed = true;
            }
        });
    }

    /// What to replay after a line boundary to restore the running
    /// state — empty while overflowed, since nothing accumulated during
    /// an overflow is trustworthy enough to replay (see
    /// [`record`](Self::record)'s docs on why a later full reset, not
    /// this method, is what resumes restoration).
    fn restore_bytes(&self) -> &[u8] {
        if self.overflowed { &[] } else { &self.bytes }
    }
}

/// Force an SGR reset at every line boundary of a normalized snapshot,
/// and restore whatever style was still running across that boundary —
/// so a replayed frame can neither inherit a still-active attribute onto
/// cells the original screen never painted, nor silently lose a style
/// the next row was actually relying on. Returns `None` the instant
/// producing that output would exceed `budget` bytes; see the "Byte
/// budget" section below.
///
/// # The bug this closes
///
/// `capture-pane -e` reconstructs each row's escape sequences from
/// tmux's own cell grid, but emits no reset at the row's end — whatever
/// attribute the row's last styled cell left active is simply still
/// "on" when the terminator arrives. On replay, a real terminal's
/// scroll/line-feed handling fills newly-revealed cells using the
/// CURRENTLY ACTIVE background (background-color-erase) rather than a
/// blank default — so every cell from the terminator onward inherits
/// that dangling attribute, producing a highlight band a real live
/// `claude` never showed (see the module's snapshot-path callers for
/// the manual-testing writeup that found this).
///
/// # Why a bare boundary reset is not enough
///
/// `capture-pane -e` only emits an SGR sequence when a cell's style
/// CHANGES from the previous one — a background painted on row 1 and
/// simply carried, unchanged, into row 2 is never re-stated in row 2's
/// own bytes; row 2's first styled cell relies entirely on state
/// established earlier. An earlier, simpler version of this function
/// injected only a closing reset at each boundary, which silently erased
/// exactly that carried-forward styling from every row that depended on
/// it — trading one visible bug for another. This version instead
/// RESTORES the running state immediately after the boundary, via
/// [`CarryoverLog`] — see that type's docs for how the log itself is
/// built and recovers from its own bound.
///
/// # Byte budget
///
/// `budget` is the same cap the caller ultimately wants the STORED
/// snapshot bounded by (`MAX_ALT_SCREEN_SNAPSHOT_BYTES`, threaded down
/// from [`TmuxDriver::capture_alt_screen_if_active`]'s own `max_bytes`).
/// Every append below — a line's own content, its closing reset, or its
/// restore — goes through [`try_append`], which refuses (and this
/// function then returns `None`) the instant that append would cross
/// `budget`. Enforcing this INSIDE construction, rather than sanitizing
/// unconditionally and checking the finished length afterward, matters
/// because sanitizing itself grows the frame per boundary: a naive
/// build-then-check design would let a capture with thousands of
/// boundaries transiently allocate far more than `budget` ever permits
/// before a post-hoc check could reject it. `None` here is exactly the
/// caller's `AltScreenCapture::TooLarge` outcome.
///
/// # Scope
///
/// Deliberately narrow: sequences are scanned only to detect the
/// full-reset forms [`sgr_is_full_reset`] enumerates, never otherwise
/// interpreted, and intra-line bytes are never rewritten — a row's own
/// styling survives byte-for-byte. The only NEW bytes this function
/// ever introduces are, per boundary, one closing reset before the
/// terminator and (fidelity and budget permitting) one restore replay
/// right after it. A line that already ends in its own reset (or even
/// several) is not special-cased out of the injection — it just gets
/// another one appended, which is harmless in effect, and is
/// deliberately pinned rather than "fixed" (detecting "already reset"
/// would require exactly the semantic SGR parsing this function exists
/// to avoid). End-of-input gets only the closing reset: there is no
/// following row for a restore to matter to.
///
/// Takes ALREADY-`normalize_capture`d bytes (CRLF line endings, no
/// terminator on the final line) and must run before anything treats the
/// result as "the stored snapshot" — seeing this transform's output,
/// not raw `capture-pane` bytes, is what makes the file written to disk
/// (and the pending-map entry served mid-stop) hygienic without replay
/// code needing to know anything changed.
fn sanitize_snapshot_lines(normalized: &[u8], budget: usize) -> Option<Vec<u8>> {
    const SGR_RESET: &[u8] = b"\x1b[0m";
    if normalized.is_empty() {
        // Nothing captured (e.g. a blank pane) — no line boundary exists
        // to reset, and appending one unconditionally would manufacture
        // a stray reset in front of content that was never there.
        return Some(Vec::new());
    }

    // Reserve for the bytes this function ALWAYS adds (one closing reset
    // per boundary, plus one for the final line), capped at `budget`:
    // the finished output can never exceed `budget` anyway (a crossing
    // aborts below), so reserving past it — the very over-allocation
    // this budget exists to prevent — would defeat the point on a
    // capture with a huge `boundary_count` but a small `budget`. The
    // variable-length restore replays are impossible to size up front
    // and are left to ordinary reallocation within that same ceiling.
    let boundary_count = normalized
        .windows(2)
        .filter(|window| *window == b"\r\n")
        .count();
    let capacity_hint = normalized.len() + SGR_RESET.len() * (boundary_count + 1);
    let mut out = Vec::with_capacity(capacity_hint.min(budget.saturating_add(1)));

    let mut carryover = CarryoverLog::new();
    let mut rest = normalized;
    while let Some(pos) = rest.windows(2).position(|window| window == b"\r\n") {
        let line = &rest[..pos];
        carryover.record(line);
        try_append(&mut out, budget, line)?;
        try_append(&mut out, budget, SGR_RESET)?;
        try_append(&mut out, budget, b"\r\n")?;
        try_append(&mut out, budget, carryover.restore_bytes())?;
        rest = &rest[pos + 2..];
    }
    // The final line never carries a terminator (`normalize_capture`
    // strips it), so it gets only the closing reset — there is no
    // following row for a restore to serve, hence no need to feed it
    // through `carryover.record` either (nothing would ever read the
    // log again after this point).
    try_append(&mut out, budget, rest)?;
    try_append(&mut out, budget, SGR_RESET)?;
    Some(out)
}

/// Append `bytes` to `out` unless doing so would cross `budget` (checked
/// via the same [`within_snapshot_cap`] boundary the caller ultimately
/// enforces on the STORED snapshot) — `None` on refusal, so
/// [`sanitize_snapshot_lines`] can propagate an abort with `?` the
/// instant any single write would blow the budget, mid-construction,
/// instead of finishing an oversized build first.
fn try_append(out: &mut Vec<u8>, budget: usize, bytes: &[u8]) -> Option<()> {
    if !within_snapshot_cap(out.len() + bytes.len(), budget) {
        return None;
    }
    out.extend_from_slice(bytes);
    Some(())
}

/// Walk `line`, calling `on_sgr_sequence` with each complete `ESC [
/// <params> m` sequence's exact byte span, in order.
///
/// `<params>` accepts ASCII digits, `;`, and `:` before requiring the
/// final `m` — digits and `;` cover classic SGR (`\x1b[31m`), while `:`
/// covers the colon-delimited SUBPARAMETER form modern terminals
/// (including tmux) emit for compound attributes such as underline
/// style/color (`\x1b[4:3m`, a curly underline). Without accepting `:`,
/// such a sequence's params would stop short of the `m`, this scanner
/// would not recognize it as SGR at all, and a style expressed only in
/// colon form could be reset at a boundary but never restored. Any other
/// CSI sequence (cursor moves, erases, ...) is left entirely alone: this
/// scanner has no opinion on it and does not even skip past it
/// specially, since the very next byte position picks the scan back up.
/// Matches [`sanitize_snapshot_lines`]'s scope: this only ever LOOKS AT
/// sequences to build the carryover log, never rewrites anything a line
/// already contains.
fn for_each_sgr_sequence<'a>(line: &'a [u8], mut on_sgr_sequence: impl FnMut(&'a [u8])) {
    let mut i = 0;
    while i + 1 < line.len() {
        if line[i] == 0x1b && line[i + 1] == b'[' {
            let mut end = i + 2;
            while end < line.len()
                && (line[end].is_ascii_digit() || line[end] == b';' || line[end] == b':')
            {
                end += 1;
            }
            if end < line.len() && line[end] == b'm' {
                on_sgr_sequence(&line[i..=end]);
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
}

/// Whether `sequence` (a complete `ESC[<params>m` span, as produced by
/// [`for_each_sgr_sequence`]) is one of the full-reset forms
/// [`sanitize_snapshot_lines`]'s docs enumerate: no parameters at all,
/// or a first parameter of `0`.
///
/// Only the FIRST parameter is inspected — `ESC[0;1m` ("reset, then
/// bold") is a reset exactly as much as bare `ESC[0m` is; anything after
/// the leading `0` is additional state layered on top of a clean slate,
/// which is why [`CarryoverLog::record`] clears its log and then still
/// logs the sequence itself, rather than trying to strip the `0;` prefix
/// out — that would require interpreting parameter semantics this
/// module deliberately stays out of.
fn sgr_is_full_reset(sequence: &[u8]) -> bool {
    // `sequence` is `[ESC, b'[', ..params.., b'm']`; params live strictly
    // between those four fixed bytes.
    let params = &sequence[2..sequence.len() - 1];
    let first_param = params.split(|&b| b == b';').next().unwrap_or(b"");
    first_param.is_empty() || first_param == b"0"
}

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
    /// `#{pane_dead_status}` when `dead` and parseable as a plain integer.
    /// `None` while alive, and also `None` for a dead pane whose status
    /// tmux could not express as one (a signal death, chiefly) — the same
    /// honest gap `SessionStatus::Exited`'s own docs describe, just
    /// discovered here instead of invented by this struct.
    pub exit_code: Option<i32>,
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
    pub fn new(state_dir: &Path) -> TmuxDriver {
        TmuxDriver {
            socket: state_dir.join("tmux.sock"),
            config: state_dir.join("tmux.conf"),
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
    /// Three callers, all of them tearing something down: a create
    /// unwinding a window whose SQLite insert failed (`create_session`'s
    /// failure-ordering contract), `DeleteSession`'s teardown, and a
    /// RESTART clearing the husk of a tmux session whose pane it can no
    /// longer find before building a fresh terminal under the same name. The already-gone case this tolerates is NOT the agent
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
        if let Err(e) = self.resize_window(session, cols, 1).await {
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

    /// Resize a session's window. `cols`/`rows` are clamped to tmux's
    /// accepted range: a browser reporting 0 columns (or an absurd
    /// value) must not turn into a tmux error, because callers treat
    /// resize as fire-and-forget.
    pub async fn resize_window(&self, name: &str, cols: u16, rows: u16) -> anyhow::Result<()> {
        let cols = cols.clamp(1, 10_000);
        let rows = rows.clamp(1, 10_000);
        self.run(&[
            "resize-window",
            "-t",
            name,
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
    /// That scoping matters because pane ids (`%N`) are assigned by a
    /// server-wide counter that resets to `%0` whenever the private tmux
    /// server itself restarts (the restart-gap case, PLAN_M2.md) — a
    /// `Terminal` is only ever populated for a pane this process has
    /// separately confirmed belongs to a currently-live server (see
    /// `Supervisor::reload_sessions`'s `has_session` check), so in
    /// practice no caller today can hand this a pane from a dead server.
    /// The `#{session_name}` check below is the same belt this crate
    /// applies everywhere else it can afford to (see e.g. `has_session`'s
    /// exact-match `=` prefix): a structural guarantee that does not rely
    /// on every future call site preserving that invariant by convention.
    ///
    /// Returns `Ok(None)` — not an error — when the pane or its session no
    /// longer exists at all: the same tolerated tmux diagnostics
    /// `has_session`/`kill_session` already treat as "not there" (a
    /// concurrent external `kill-session`, or the whole private server
    /// having gone away). Stop and delete both read that as "nothing is
    /// running", the same as a positively dead pane.
    pub async fn pane_process(
        &self,
        session: &str,
        pane: &str,
    ) -> anyhow::Result<Option<PaneProcess>> {
        let out = match self
            .run(&[
                "display-message",
                "-p",
                "-t",
                pane,
                "#{session_name} #{pane_pid} #{pane_dead}",
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
                return Ok(None);
            }
            Err(e) => return Err(e).context("querying pane process state"),
        };
        // A stale pane id against a server that has since dropped every
        // session it knew (this one killed, nothing else ever created)
        // does NOT error the way `has_session`'s docs describe for
        // `has-session` — verified empirically against this exact
        // config's `exit-empty off` server — it exits zero with every
        // format variable expanding empty, mirroring the same "nothing to
        // inspect" shape `bracket_paste_flag_is_missing` already handles
        // for pane-mode queries. `split_whitespace` never yields an empty
        // token, so an entirely-blank `out` (that all-empty-expansion
        // case) makes `.next()` return `None` directly — no separate
        // emptiness check needed, and `None` here IS the tell: a real
        // session is never named the empty string, so this means "no
        // such pane", not "a pane named ''".
        let mut fields = out.split_whitespace();
        let Some(found_session) = fields.next() else {
            return Ok(None);
        };
        if found_session != session {
            bail!(
                "pane {pane} belongs to session {found_session:?}, not {session:?}; refusing to \
                 treat it as this session's terminal (a stale or reused pane id would otherwise \
                 cross-wire two sessions' process trees)"
            );
        }
        let pid = fields
            .next()
            .with_context(|| format!("pane process query returned no pane_pid: {out:?}"))?
            .parse()
            .with_context(|| format!("parsing pane_pid from {out:?}"))?;
        let dead = fields
            .next()
            .with_context(|| format!("pane process query returned no pane_dead: {out:?}"))?
            == "1";
        Ok(Some(PaneProcess { pid, dead }))
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
    pub async fn pane_states(&self) -> anyhow::Result<HashMap<String, PaneState>> {
        let out = match self
            .run(&[
                "list-panes",
                "-a",
                "-F",
                "#{pane_id} #{session_name} #{pane_dead} #{pane_dead_status}",
            ])
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
                let tolerated = e
                    .downcast_ref::<TmuxCommandFailure>()
                    .is_some_and(|failure| {
                        is_tolerated_list_panes_diagnostic(&failure.stderr_trimmed(), &self.socket)
                    });
                if tolerated {
                    return Ok(HashMap::new());
                }
                return Err(e).context("querying pane states");
            }
        };
        Ok(parse_pane_states(&out))
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
    pub async fn open_replay_stream(
        &self,
        session: &str,
        pane: &str,
    ) -> anyhow::Result<(PaneModes, Vec<u8>, OutputStream)> {
        let deadline = tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT;
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
        let mut stream = OutputStream {
            child,
            // Exclusively owned: this client carries only its own
            // one-shot replay-cutover command group (below) and then
            // never writes again. Input travels on a wholly separate
            // client — see `open_input_client` — so there is no sharing
            // concern here.
            stdin,
            reader: BufReader::new(stdout),
            line: Vec::with_capacity(8192),
            passthrough: PassthroughDecoder::default(),
        };
        read_command_block(
            &mut stream.reader,
            &mut stream.line,
            deadline,
            "control-mode attach",
        )
        .await?;
        let (modes, prefill) = stream
            .snapshot_then_cutover(pane, deadline, &attach_cutover_command())
            .await?;
        Ok((modes, prefill, stream))
    }

    /// Open a control-mode client dedicated to carrying input into `pane`.
    ///
    /// Attaches with `-f no-output` PERMANENTLY, unlike the replay
    /// client's transient use of the same flag: this client must never
    /// see a pane-output notification, because [`InputClient::send`] reads
    /// exactly one command-reply block per chunk it writes, and an
    /// interleaved pane-output line would desynchronize that read from
    /// the write that produced it. See [`InputClient`] for why input gets
    /// its own client at all rather than riding the replay client's
    /// stdin.
    pub async fn open_input_client(&self, pane: &str) -> anyhow::Result<InputClient> {
        let deadline = tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT;
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
        read_command_block(&mut reader, &mut line, deadline, "input-client attach").await?;
        Ok(InputClient {
            child,
            stdin,
            reader,
            line,
            pane: pane.to_string(),
            delivered_any_bytes: false,
        })
    }
}

/// A control-mode client streaming one session's pane output.
///
/// It starts with output disabled and is only returned after replay
/// capture and the live cutover have completed. The client counts as
/// attached in tmux's eyes but never declares a size (no
/// `refresh-client -C`), so tmux ignores it for sizing entirely —
/// geometry comes only from explicit `resize-window` calls. Dropping it
/// kills the client process (`kill_on_drop`), which detaches it; the tmux
/// server and pane are unaffected.
pub struct OutputStream {
    child: Child,
    /// Held open for the lifetime of the client. Carries only the
    /// one-shot replay-cutover command group written at attach time;
    /// nothing writes to it afterwards. Input travels on a wholly
    /// separate [`InputClient`], so this is a plain, exclusively-owned
    /// handle rather than a shared, lockable one.
    stdin: ChildStdin,
    reader: BufReader<tokio::process::ChildStdout>,
    line: Vec<u8>,
    /// Stateful because tmux may split one passthrough wrapper across
    /// several output notifications.
    passthrough: PassthroughDecoder,
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
        pane: &str,
        deadline: tokio::time::Instant,
        cutover: &str,
    ) -> anyhow::Result<(PaneModes, Vec<u8>)> {
        let command = replay_command_group(pane, cutover);
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        tokio::time::timeout(remaining, async {
            self.stdin
                .write_all(command.as_bytes())
                .await
                .context("writing tmux replay cutover commands")?;
            self.stdin
                .flush()
                .await
                .context("flushing tmux replay cutover commands")
        })
        .await
        .context("timed out writing tmux replay cutover commands")??;

        let modes_output =
            read_command_block(&mut self.reader, &mut self.line, deadline, "pane modes").await?;
        let history_output = read_command_block(
            &mut self.reader,
            &mut self.line,
            deadline,
            "history snapshot",
        )
        .await?;
        let visible_output = read_command_block(
            &mut self.reader,
            &mut self.line,
            deadline,
            "visible snapshot",
        )
        .await?;
        let _cutover = read_command_block(
            &mut self.reader,
            &mut self.line,
            deadline,
            "live-output cutover",
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
    /// behind, which the next call discards. Callers may only abandon this
    /// future on a path that tears the whole stream down (the stall detach
    /// does exactly that), never to resume reading afterwards.
    pub async fn next_output(&mut self) -> anyhow::Result<Option<OutputEvent>> {
        // Fields are destructured up front so the borrow checker can see
        // that the scratch line buffer, the reader, and the passthrough
        // decoder are disjoint — which is what lets a classified borrow OF
        // the line coexist with the mutable borrow of the decoder, without
        // moving the buffer out and back on every call.
        let OutputStream {
            reader,
            line,
            passthrough,
            ..
        } = self;
        loop {
            line.clear();
            let n = read_control_line(reader, line).await?;
            if n == 0 {
                return Ok(None);
            }
            match classify_control_line(strip_line_ending(line)) {
                Some(ControlLine::Payload(escaped)) => {
                    let bytes = decode_output_payload(passthrough, escaped);
                    if !bytes.is_empty() {
                        return Ok(Some(OutputEvent::Bytes(bytes)));
                    }
                }
                Some(ControlLine::Paused) => return Ok(Some(OutputEvent::Paused)),
                Some(ControlLine::Exit) => return Ok(None),
                None => {}
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
    pub async fn resume_paused_with_replay(
        &mut self,
        pane: &str,
    ) -> anyhow::Result<(PaneModes, Vec<u8>)> {
        // Discard any half-decoded passthrough wrapper left over from the
        // stream tmux just cut. `%pause` can land in the middle of a
        // wrapper split across notifications, and that partial state
        // belongs to bytes the client will never receive — carrying it
        // into the post-reset stream would make the decoder treat the
        // first fresh bytes as a wrapper continuation and swallow or
        // mangle them. The replay itself is capture-pane output, which
        // never contains a wrapper, so nothing is lost by clearing here.
        self.passthrough = PassthroughDecoder::default();
        let deadline = tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT;
        self.snapshot_then_cutover(pane, deadline, &continue_pane_command(pane))
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
        self.stdin.write_all(command.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
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
        let deadline = tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT;
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

    /// Kill the control-mode client and wait for it to actually be gone.
    ///
    /// The orderly teardown for a forwarder that ran to completion. A
    /// forwarder cancelled mid-flight never reaches this — the task stops
    /// at an await point and `kill_on_drop` does the killing instead,
    /// which is why the takeover path aborts *and then awaits* the
    /// forwarder rather than just aborting it. Either way the client must
    /// be dead before another attaches: overlapping control clients
    /// reproducibly froze the newcomer's stream after the replay. The
    /// mechanism was never pinned down (in isolation two attached control
    /// clients DO both receive pane output — audited), so treat the
    /// ordering, not any particular explanation, as the invariant; the
    /// attach handler's open-stream comment tells the same story.
    pub async fn shutdown(mut self) {
        let _ = self.child.kill().await;
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
    /// The pane this client addresses every `send-keys -t <pane>` at.
    pane: String,
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
                write!(line, "send-keys -t {} -H", self.pane).expect("String write is infallible");
                for byte in *chunk {
                    write!(line, " {byte:02x}").expect("String write is infallible");
                }
                line.push('\n');
                tokio::time::timeout(
                    CONTROL_EXCHANGE_TIMEOUT,
                    self.stdin.write_all(line.as_bytes()),
                )
                .await
                .context("timed out writing tmux send-keys command")?
                .context("writing tmux send-keys command")?;
            }
            tokio::time::timeout(CONTROL_EXCHANGE_TIMEOUT, self.stdin.flush())
                .await
                .context("timed out flushing tmux send-keys commands")?
                .context("flushing tmux send-keys commands")?;
            for _ in 0..batch.len() {
                // One deadline per reply, not one for the whole batch: a
                // wedged tmux on one reply must not inherit the unspent
                // budget of every reply before it.
                let deadline = tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT;
                read_command_block(
                    &mut self.reader,
                    &mut self.line,
                    deadline,
                    "send-keys input",
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
/// [`parse_pane_states`] and `PaneModes::parse` split their own parsing out
/// for elsewhere in this module. Any OTHER message returns `false`, which is
/// what sends an unclassified failure down `pane_states`'s error path
/// instead of being silently folded into an empty (and therefore
/// all-exited) map.
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

/// Parse `list-panes -a -F
/// '#{pane_id} #{session_name} #{pane_dead} #{pane_dead_status}'` output
/// into a per-pane map. Split out from [`TmuxDriver::pane_states`] purely
/// so this parsing is unit-testable against constructed strings, without
/// a real tmux server behind it — the same reasoning `PaneModes::parse`
/// and `parse_stat` split their own parsing out for elsewhere in this
/// codebase.
///
/// Strict on `pane_dead`: it must be the EXACT string `"0"` or `"1"`, not
/// merely "whatever wasn't `1`". A row whose `pane_dead` field is missing
/// entirely (an unexpectedly short line) or holds some other value (a
/// tmux format change this build does not understand) is skipped outright
/// — not inserted into the map at all — rather than defaulting to `dead:
/// false`. Defaulting to alive would be a silent liveness guess for a row
/// this function could not actually parse, exactly the kind of fabricated
/// answer `session_status`'s whole "missing from the map means Exited"
/// contract exists to avoid; skipping instead means the caller sees the
/// SAME honest "not found" outcome as any other absent pane.
fn parse_pane_states(out: &str) -> HashMap<String, PaneState> {
    let mut states = HashMap::new();
    for line in out.lines() {
        let mut fields = line.split_whitespace();
        let Some(pane_id) = fields.next() else {
            // A blank line is the same "nothing to inspect" shape
            // `pane_process` documents for an all-empty expansion — not a
            // row worth reporting.
            continue;
        };
        let Some(session_name) = fields.next() else {
            // A pane id with nothing after it is just as malformed as a
            // missing pane_dead field below — skip rather than guess.
            continue;
        };
        let dead = match fields.next() {
            Some("0") => false,
            Some("1") => true,
            // Missing, or some value this build does not recognize:
            // malformed, and skipped rather than guessed at.
            _ => continue,
        };
        // Only trust the status field once the pane is actually dead:
        // tmux leaves it at a stale or empty value while alive, and
        // parsing it regardless would risk fabricating an exit code for a
        // pane that has not exited at all.
        let exit_code = if dead {
            fields.next().and_then(|f| f.parse::<i32>().ok())
        } else {
            None
        };
        states.insert(
            pane_id.to_string(),
            PaneState {
                session_name: session_name.to_string(),
                dead,
                exit_code,
            },
        );
    }
    states
}

/// The identity tmux repeats on one command reply's begin/end markers.
///
/// Pane content is allowed to start with `%`, including text that looks
/// like some other command's marker. Only an end marker with this exact
/// identity closes the block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ControlBlockId {
    timestamp: u64,
    command: u64,
    flags: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlMarker {
    Begin(ControlBlockId),
    End(ControlBlockId),
    Error(ControlBlockId),
}

/// Parse only the three numeric marker forms tmux itself emits.
///
/// A line merely beginning with `%begin`, `%end`, or `%error` is not
/// enough: capture-pane output is unescaped, so terminal content may use
/// those words. Requiring the complete numeric shape keeps such content
/// inside the snapshot.
fn parse_control_marker(line: &[u8]) -> Option<ControlMarker> {
    let mut fields = line.split(|byte| *byte == b' ');
    let kind = fields.next()?;
    let parse_number = |field: &[u8]| {
        (!field.is_empty() && field.iter().all(u8::is_ascii_digit))
            .then(|| std::str::from_utf8(field).ok()?.parse::<u64>().ok())
            .flatten()
    };
    let id = ControlBlockId {
        timestamp: parse_number(fields.next()?)?,
        command: parse_number(fields.next()?)?,
        flags: parse_number(fields.next()?)?,
    };
    if fields.next().is_some() {
        return None;
    }
    match kind {
        b"%begin" => Some(ControlMarker::Begin(id)),
        b"%end" => Some(ControlMarker::End(id)),
        b"%error" => Some(ControlMarker::Error(id)),
        _ => None,
    }
}

/// Read one complete command reply without consuming the notification
/// after its closing marker.
///
/// tmux writes command output as raw lines between `%begin` and the
/// matching `%end`/`%error`. Mismatched marker-shaped lines remain
/// content; commands in this client are serialized, so another real
/// reply cannot nest here. EOF and timeout are hard errors because
/// accepting a partial snapshot would manufacture terminal history.
async fn read_command_block<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    line: &mut Vec<u8>,
    deadline: tokio::time::Instant,
    purpose: &str,
) -> anyhow::Result<Vec<u8>> {
    let id = loop {
        line.clear();
        let read = read_control_line_before(reader, line, deadline, purpose).await?;
        if read == 0 {
            bail!("tmux control client exited before the {purpose} reply began");
        }
        let stripped = strip_line_ending(line);
        match parse_control_marker(stripped) {
            Some(ControlMarker::Begin(id)) => break id,
            Some(ControlMarker::End(_) | ControlMarker::Error(_)) => {
                bail!("tmux control protocol ended a block before beginning the {purpose} reply");
            }
            // Both dialects, for the same reason: live output arriving
            // BETWEEN reply blocks means the cutover ordering broke, and
            // silently folding those bytes into the next block's captured
            // output would manufacture terminal history. Which dialect it
            // arrives in depends only on whether `pause-after` is set on
            // this client, which is not this function's business to know.
            None if stripped.starts_with(b"%output ")
                || stripped.starts_with(b"%extended-output ") =>
            {
                bail!("tmux emitted live output before the replay cutover completed");
            }
            None if stripped.starts_with(b"%exit") => {
                bail!("tmux control client exited before the {purpose} reply");
            }
            None => {}
        }
    };

    let mut output = Vec::new();
    loop {
        line.clear();
        let read = read_control_line_before(reader, line, deadline, purpose).await?;
        if read == 0 {
            bail!("tmux control client exited inside the {purpose} reply");
        }
        let stripped = strip_line_ending(line);
        match parse_control_marker(stripped) {
            Some(ControlMarker::End(end)) if end == id => return Ok(output),
            Some(ControlMarker::Error(end)) if end == id => {
                let reason = String::from_utf8_lossy(strip_command_output_terminator(&output));
                bail!("tmux {purpose} command failed: {}", reason.trim());
            }
            _ => output.extend_from_slice(line),
        }
    }
}

/// Read one control line before the shared exchange deadline.
async fn read_control_line_before<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    out: &mut Vec<u8>,
    deadline: tokio::time::Instant,
    purpose: &str,
) -> anyhow::Result<usize> {
    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
    tokio::time::timeout(remaining, read_control_line(reader, out))
        .await
        .with_context(|| format!("timed out waiting for {purpose}"))?
        .with_context(|| format!("reading tmux control protocol during {purpose}"))
}

/// Read one tmux control-mode notification without permitting an
/// unterminated line to grow memory without bound.
async fn read_control_line<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    out: &mut Vec<u8>,
) -> std::io::Result<usize> {
    read_control_line_with_limit(reader, out, MAX_CONTROL_LINE).await
}

/// Limit-parameterized core for small boundary tests; production always
/// uses [`MAX_CONTROL_LINE`].
async fn read_control_line_with_limit<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    out: &mut Vec<u8>,
    limit: usize,
) -> std::io::Result<usize> {
    let start = out.len();
    loop {
        let (take, complete) = {
            let available = reader.fill_buf().await?;
            if available.is_empty() {
                let partial = out.len() - start;
                if partial == 0 {
                    return Ok(0);
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    format!("tmux control-mode stream ended with a {partial}-byte partial line"),
                ));
            }
            let take = available
                .iter()
                .position(|&byte| byte == b'\n')
                .map_or(available.len(), |index| index + 1);
            if out.len() + take > limit {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("tmux control-mode line exceeds {limit} bytes"),
                ));
            }
            out.extend_from_slice(&available[..take]);
            (take, available[take - 1] == b'\n')
        };
        reader.consume(take);
        if complete {
            return Ok(out.len() - start);
        }
    }
}

/// Warn once per process if this tmux cannot report bracketed paste.
///
/// Checked here rather than at startup because it needs a real pane:
/// with no target every format expands empty, so a startup probe cannot
/// tell "tmux is too old" from "there is nothing to inspect yet" without
/// creating a throwaway session just to interrogate it.
fn warn_once_about_missing_bracket_paste(line: &str) {
    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if bracket_paste_flag_is_missing(line)
        && !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed)
    {
        tracing::warn!(
            "this tmux lacks bracket_paste_flag (added in tmux 3.7): bracketed paste will not \
             be restored when reattaching to a session. Everything else works."
        );
    }
}

/// Whether a pane-mode expansion shows a tmux without
/// `bracket_paste_flag`.
///
/// The distinction is drawn from the expansion itself: `alternate_on`
/// (the first field) exists on every supported tmux, so a populated
/// first field with an empty second means this tmux genuinely lacks
/// `bracket_paste_flag` (pre-3.7) — while an all-empty expansion means
/// there was no pane to inspect, which must NOT warn. A predecessor of
/// this check got that inverted (lore/: warned on every healthy start,
/// silent on old tmux), which is why the predicate is split out and
/// unit-tested separately from the once-per-process latch.
fn bracket_paste_flag_is_missing(line: &str) -> bool {
    let mut fields = line.split(',');
    let alternate = fields.next().unwrap_or("");
    let bracket = fields.next().unwrap_or("");
    !alternate.is_empty() && bracket.is_empty()
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

/// Incrementally remove tmux passthrough wrappers before bytes reach the
/// real terminal.
///
/// tmux may split `ESC P tmux; ... ESC \` across output
/// notifications (either dialect), including inside the opener or on either side of an
/// escaped `ESC`. Keeping only the parser's few bytes of boundary state
/// avoids both split-wrapper corruption and an unbounded whole-wrapper
/// buffer for large inline images.
#[derive(Default)]
struct PassthroughDecoder {
    opener: Vec<u8>,
    in_wrapper: bool,
    pending_escape: bool,
}

impl PassthroughDecoder {
    const OPEN: &'static [u8] = b"\x1bPtmux;";

    /// Decode one arbitrary output chunk. Empty output means the chunk
    /// ended inside an opener or immediately after an escaped byte; the
    /// next call resumes from that exact state.
    fn push(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(bytes.len());
        for &byte in bytes {
            if self.in_wrapper {
                if self.pending_escape {
                    self.pending_escape = false;
                    match byte {
                        // Payload ESC bytes are doubled inside a wrapper.
                        0x1b => out.push(0x1b),
                        // A single ESC + backslash closes the wrapper.
                        b'\\' => self.in_wrapper = false,
                        // Malformed but lossless: preserve a lone ESC and
                        // its follower instead of inventing a sequence.
                        other => {
                            out.push(0x1b);
                            out.push(other);
                        }
                    }
                } else if byte == 0x1b {
                    self.pending_escape = true;
                } else {
                    out.push(byte);
                }
                continue;
            }

            self.opener.push(byte);
            while !Self::OPEN.starts_with(&self.opener) {
                out.push(self.opener.remove(0));
            }
            if self.opener == Self::OPEN {
                self.opener.clear();
                self.in_wrapper = true;
            }
        }
        out
    }
}

/// One-shot test oracle for complete passthrough sequences. Production
/// streaming keeps a [`PassthroughDecoder`] across notifications.
#[cfg(test)]
fn unwrap_passthrough(bytes: &[u8]) -> Vec<u8> {
    let mut decoder = PassthroughDecoder::default();
    let mut out = decoder.push(bytes);
    if decoder.in_wrapper {
        // A one-shot caller cannot wait for a split wrapper. Preserve it
        // byte-for-byte; the streaming path never needs this fallback.
        return bytes.to_vec();
    }
    out.append(&mut decoder.opener);
    out
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

/// Remove the newline control mode adds after one command's stdout.
///
/// This is separate from [`normalize_capture`]: capture-pane already
/// terminates its final row, so a command block contains two trailing
/// newlines. One belongs to control mode and is removed here; the other
/// belongs to capture-pane and is removed during terminal normalization.
fn strip_command_output_terminator(output: &[u8]) -> &[u8] {
    // Same trailing-CRLF-or-LF shape as a notification's own terminator,
    // so it shares that implementation rather than repeating it; the two
    // stay distinct FUNCTIONS because they answer different questions
    // (see this one's docs on the two newlines a command block carries).
    strip_line_ending(output)
}

/// What one control-mode notification line means to
/// [`OutputStream::next_output`], before any decoding happens.
///
/// Split out from the read loop as a pure function over one line
/// specifically so the notification GRAMMAR is unit-testable without a
/// tmux server or a live process behind it — the same reasoning
/// [`parse_pane_states`] and [`PaneModes::parse`] were split out for.
/// Shapes worth testing here are otherwise awkward or impossible to
/// provoke on demand from a real server: a `%pause` requires stalling a
/// client for seconds, and a future tmux's extra `%extended-output`
/// argument does not exist yet at all.
///
/// `None` is "nothing this stream cares about" — command replies,
/// `%layout-change`, `%window-renamed`, and anything a newer tmux invents.
enum ControlLine<'a> {
    /// Escaped pane bytes from either output dialect, still to be
    /// unescaped and passthrough-decoded.
    Payload(&'a [u8]),
    /// `%pause <pane-id>`.
    Paused,
    /// `%exit`: this control client is going away.
    Exit,
}

/// Classify one already-terminator-stripped notification line. See
/// [`ControlLine`] for why this is a free function.
fn classify_control_line(line: &[u8]) -> Option<ControlLine<'_>> {
    if let Some(rest) = line.strip_prefix(b"%output ") {
        // Format: "%<pane-id> <escaped-data>".
        split_output_payload(rest).map(ControlLine::Payload)
    } else if let Some(rest) = line.strip_prefix(b"%extended-output ") {
        // Format: "%<pane-id> <age> ... : <escaped-data>".
        split_extended_output_payload(rest).map(ControlLine::Payload)
    } else if line.starts_with(b"%pause ") {
        Some(ControlLine::Paused)
    } else if line.starts_with(b"%exit") {
        Some(ControlLine::Exit)
    } else {
        None
    }
}

/// Turn one notification's escaped payload into pane bytes: undo
/// control-mode octal escaping, then feed the result through the
/// stateful passthrough decoder.
///
/// One function for BOTH dialects, deliberately. That matters more than
/// it looks: [`PassthroughDecoder`] carries state ACROSS notifications (a
/// `ESC P tmux;` wrapper may be split over any number of them), so a
/// second decode path — or one dialect bypassing the decoder — would
/// corrupt any wrapper that happens to straddle a dialect boundary. An
/// empty result is normal and means the chunk ended mid-wrapper; the
/// caller keeps reading.
fn decode_output_payload(decoder: &mut PassthroughDecoder, escaped: &[u8]) -> Vec<u8> {
    decoder.push(&unescape_control_output(escaped))
}

/// Split a `%output` notification's tail (everything past `"%output "`)
/// into just its escaped payload, dropping the pane id.
///
/// `None` for a malformed line with no separator at all — treated as
/// chatter rather than as empty output, so a truncated notification can
/// never be mistaken for the pane genuinely emitting nothing.
fn split_output_payload(rest: &[u8]) -> Option<&[u8]> {
    let separator = rest.iter().position(|&byte| byte == b' ')?;
    Some(&rest[separator + 1..])
}

/// Split an `%extended-output` notification's tail (everything past
/// `"%extended-output "`) into just its escaped payload.
///
/// The documented shape is `pane-id age ... : value`, where tmux
/// explicitly reserves the space between the age and the `:` for future
/// arguments a client "should ignore". So this deliberately does NOT
/// count fields: it scans space-separated fields for the first one that
/// is EXACTLY `":"` and takes everything after it, which is what makes a
/// future tmux adding a field a no-op here instead of a payload shifted
/// by one token. Anchoring on a lone `:` field (not the first `:` byte)
/// matters because the payload itself routinely contains colons — pane
/// output is arbitrary bytes.
///
/// `None` when no such separator field exists, for the same reason
/// [`split_output_payload`] returns `None`: a line this function cannot
/// locate a payload in is chatter, not empty output.
fn split_extended_output_payload(rest: &[u8]) -> Option<&[u8]> {
    let mut offset = 0;
    while offset < rest.len() {
        let end = rest[offset..]
            .iter()
            .position(|&byte| byte == b' ')
            .map_or(rest.len(), |index| offset + index);
        if &rest[offset..end] == b":" {
            // `end + 1` is in range whenever a space followed the marker;
            // an `%extended-output` whose payload is genuinely empty ends
            // right at the marker, which `get` turns into an empty slice
            // rather than a panic.
            return Some(rest.get(end + 1..).unwrap_or(&[]));
        }
        offset = end + 1;
    }
    None
}

/// Undo control-mode escaping.
///
/// tmux octal-escapes bytes below 0x20 *and* backslash itself — a literal
/// backslash arrives as `\134` (verified against tmux 3.7). Everything
/// else, including every byte ≥ 0x80, passes through verbatim, which is
/// why this works on bytes rather than `str`.
pub fn unescape_control_output(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\'
            && i + 3 < b.len()
            && matches!(b[i + 1], b'0'..=b'3')
            && matches!(b[i + 2], b'0'..=b'7')
            && matches!(b[i + 3], b'0'..=b'7')
        {
            let value = (b[i + 1] - b'0') * 64 + (b[i + 2] - b'0') * 8 + (b[i + 3] - b'0');
            out.push(value);
            i += 4;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The octal unescaping is the one lossy-looking transform in the
    /// output path; pin it against real control-mode escaping shapes.
    #[test]
    fn unescape_handles_octal_sequences() {
        assert_eq!(unescape_control_output(b"plain"), b"plain");
        assert_eq!(
            unescape_control_output(br"\033[1mhi\033[0m"),
            b"\x1b[1mhi\x1b[0m"
        );
        assert_eq!(unescape_control_output(br"bell\007"), b"bell\x07");
        // Invalid byte escapes stay literal rather than wrapping.
        assert_eq!(unescape_control_output(br"x\477"), br"x\477");
        // Trailing lone backslash must not panic or eat bytes.
        assert_eq!(unescape_control_output(br"x\"), b"x\\");
    }

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

    /// Passthrough payloads must survive the trip to xterm.js. tmux
    /// hands the wrapper through control mode intact (audited), so
    /// without unwrapping the terminal treats it as an unknown DCS and
    /// drops the contents — losing OSC 52 clipboard writes and inline
    /// images from any agent that uses them.
    #[test]
    fn passthrough_wrappers_are_unwrapped_with_esc_undoubled() {
        // ESC P tmux; <ESC ESC ]52;c;aGk= BEL> ESC backslash
        let wrapped = b"before\x1bPtmux;\x1b\x1b]52;c;aGk=\x07\x1b\\after";
        assert_eq!(
            unwrap_passthrough(wrapped),
            b"before\x1b]52;c;aGk=\x07after".to_vec()
        );
        // A payload that itself ends in ST (doubled to ESC ESC \ on the
        // wire) must not have the doubled pair mistaken for the wrapper's
        // terminator — that truncated the payload one byte early and
        // leaked the real close as garbage. This is the common case for
        // ST-terminated OSC and any DCS payload (sixel).
        let st_payload = b"\x1bPtmux;\x1b\x1b]52;c;aGk=\x1b\x1b\\\x1b\\tail";
        assert_eq!(
            unwrap_passthrough(st_payload),
            b"\x1b]52;c;aGk=\x1b\\tail".to_vec()
        );
        // Ordinary output is returned byte-for-byte.
        assert_eq!(
            unwrap_passthrough(b"plain\x1b[1m"),
            b"plain\x1b[1m".to_vec()
        );
        // A wrapper split across notifications must not be swallowed.
        let partial = b"\x1bPtmux;\x1b\x1b]52;c;";
        assert_eq!(unwrap_passthrough(partial), partial.to_vec());
    }

    /// `%output` boundaries are unrelated to terminal escape-sequence
    /// boundaries. Every possible two-chunk split of the wrapper must
    /// decode identically to the one-shot form, including splits inside
    /// the opener, doubled ESC, and closing ST.
    #[test]
    fn passthrough_decoder_survives_every_notification_split() {
        let wrapped = b"before\x1bPtmux;\x1b\x1b]52;c;aGk=\x1b\x1b\\\x1b\\after";
        let expected = b"before\x1b]52;c;aGk=\x1b\\after";
        for split in 0..=wrapped.len() {
            let mut decoder = PassthroughDecoder::default();
            let mut actual = decoder.push(&wrapped[..split]);
            actual.extend(decoder.push(&wrapped[split..]));
            assert!(!decoder.in_wrapper, "wrapper left open at split {split}");
            assert!(
                !decoder.pending_escape,
                "escape left pending at split {split}"
            );
            actual.extend(&decoder.opener);
            assert_eq!(actual, expected, "failed at split {split}");
        }
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

    /// An unterminated control notification must fail at the configured
    /// boundary rather than letting `read_until` grow a process-sized
    /// allocation.
    #[tokio::test]
    async fn control_mode_lines_are_bounded() {
        use tokio::io::AsyncWriteExt;

        let (reader, mut writer) = tokio::io::duplex(32);
        writer.write_all(b"12345").await.unwrap();
        writer.shutdown().await.unwrap();
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();

        let error = read_control_line_with_limit(&mut reader, &mut line, 4)
            .await
            .expect_err("line beyond the cap must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    /// EOF cannot turn a truncated notification into terminal data. Tmux
    /// control records are newline-delimited, so a partial final record
    /// means the control client died mid-write.
    #[tokio::test]
    async fn control_mode_partial_line_at_eof_is_an_error() {
        use tokio::io::AsyncWriteExt;

        let (reader, mut writer) = tokio::io::duplex(32);
        writer.write_all(b"%output %0 partial").await.unwrap();
        writer.shutdown().await.unwrap();
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();

        let error = read_control_line_with_limit(&mut reader, &mut line, 64)
            .await
            .expect_err("partial notification must not be accepted at EOF");
        assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
    }

    /// Snapshot content is raw command output, so lines beginning with
    /// `%` are ordinary content unless they exactly close this block.
    /// A loose prefix parser would truncate a pane displaying tmux
    /// protocol examples or diagnostics.
    #[tokio::test]
    async fn command_block_requires_its_exact_end_marker() {
        let input = b"%session-changed $0 session\n\
                      %begin 10 20 1\n\
                      ordinary\n\
                      %end 10 999 1\n\
                      %error not-a-marker\n\
                      %end 10 20 1\n";
        let mut reader = BufReader::new(&input[..]);
        let mut line = Vec::new();
        let output = read_command_block(
            &mut reader,
            &mut line,
            tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT,
            "test snapshot",
        )
        .await
        .expect("complete block");
        assert_eq!(output, b"ordinary\n%end 10 999 1\n%error not-a-marker\n");
    }

    /// Live pane output arriving BETWEEN reply blocks must fail the
    /// attach, in EITHER dialect.
    ///
    /// That guard is what stops the replay cutover silently
    /// manufacturing history: output landing before the cutover means the
    /// ordering broke, and folding those bytes into the next block would
    /// store them as captured content. `%extended-output` is the dialect
    /// every post-M2.5 attachment actually uses, so a guard that only knew
    /// `%output` would be the one that never fires in production. Both are
    /// asserted to produce the SAME error, since which dialect a client
    /// speaks depends only on whether `pause-after` is set and is not
    /// this function's business.
    #[tokio::test]
    async fn live_output_before_the_cutover_fails_the_attach_in_either_dialect() {
        for live in [
            &b"%output %0 live\n"[..],
            &b"%extended-output %0 0 : live\n"[..],
        ] {
            let mut input = live.to_vec();
            input.extend_from_slice(b"%begin 10 20 1\nmodes\n%end 10 20 1\n");
            let mut reader = BufReader::new(&input[..]);
            let mut line = Vec::new();
            let error = read_command_block(
                &mut reader,
                &mut line,
                tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT,
                "pane modes",
            )
            .await
            .expect_err("live output before the cutover must fail the attach");
            assert!(
                format!("{error:#}").contains("live output before the replay cutover"),
                "dialect {:?} produced an unexpected error: {error:#}",
                String::from_utf8_lossy(live)
            );
        }
    }

    /// `%error` closes the matching command block and must retain tmux's
    /// plain-text diagnostic. Otherwise an attach failure becomes an
    /// unexplained protocol error at the service boundary.
    #[tokio::test]
    async fn command_block_reports_tmux_error_text() {
        let input = b"%begin 10 20 1\ncan't find pane: %9\n%error 10 20 1\n";
        let mut reader = BufReader::new(&input[..]);
        let mut line = Vec::new();
        let error = read_command_block(
            &mut reader,
            &mut line,
            tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT,
            "history snapshot",
        )
        .await
        .expect_err("tmux error must fail the block");
        assert!(
            format!("{error:#}").contains("can't find pane: %9"),
            "tmux diagnostic was lost: {error:#}"
        );
    }

    /// EOF inside a block cannot turn a truncated capture into valid
    /// replay. The outer line reader also rejects a partial final line;
    /// this pins the distinct case where the last content line was
    /// complete but the closing marker never arrived.
    #[tokio::test]
    async fn command_block_rejects_eof_before_its_end_marker() {
        let input = b"%begin 10 20 1\ncomplete content line\n";
        let mut reader = BufReader::new(&input[..]);
        let mut line = Vec::new();
        let error = read_command_block(
            &mut reader,
            &mut line,
            tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT,
            "visible snapshot",
        )
        .await
        .expect_err("unterminated block must fail");
        assert!(
            format!("{error:#}").contains("inside the visible snapshot reply"),
            "unexpected error: {error:#}"
        );
    }

    /// Reading the final refresh reply must stop at its `%end` and leave
    /// the first live notification buffered for `next_output`, even when
    /// the underlying read splits that notification. This boundary is
    /// the whole no-gap handoff contract.
    #[tokio::test]
    async fn final_cutover_block_leaves_live_output_unconsumed() {
        use tokio::io::AsyncWriteExt;

        let (reader, mut writer) = tokio::io::duplex(1024);
        writer
            .write_all(
                b"%begin 10 20 1\nmodes\n%end 10 20 1\n\
                  %begin 10 21 1\nhistory\n%end 10 21 1\n\
                  %begin 10 22 1\nvisible\n%end 10 22 1\n\
                  %begin 10 23 1\n%end 10 23 1\n\
                  %output %0 live\\015",
            )
            .await
            .unwrap();
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();
        let deadline = tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT;
        for purpose in ["modes", "history", "visible", "cutover"] {
            read_command_block(&mut reader, &mut line, deadline, purpose)
                .await
                .expect("complete block");
        }
        writer.write_all(b"\\012\n").await.unwrap();
        line.clear();
        read_control_line(&mut reader, &mut line)
            .await
            .expect("read first live notification");
        assert_eq!(line, b"%output %0 live\\015\\012\n");
    }

    /// Control mode adds one newline around command output. Removing it
    /// before capture normalization preserves the capture's own final
    /// terminator, which normalization must remove separately.
    #[test]
    fn command_output_and_capture_terminators_are_distinct() {
        let block = b"row one\nrow two\n\n";
        assert_eq!(
            normalize_capture(strip_command_output_terminator(block)),
            b"row one\r\nrow two"
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

    /// The warn-on-old-tmux predicate has three shapes and got one of
    /// them wrong once before (lore/: a capability probe warned on every
    /// healthy tmux and stayed silent on genuinely old ones — precisely
    /// inverted). Pin all three: modern tmux quiet, old tmux warns,
    /// no-pane expansion quiet.
    #[test]
    fn bracket_paste_warning_fires_only_for_genuinely_old_tmux() {
        // Modern tmux: both fields populated.
        assert!(!bracket_paste_flag_is_missing("0,1,0,0,0,0,1,0,0,0"));
        assert!(!bracket_paste_flag_is_missing("1,0,0,0,0,0,1,0,3,7"));
        // Pre-3.7 tmux: alternate_on expands, bracket_paste_flag does not.
        assert!(bracket_paste_flag_is_missing("0,,0,0,0,0,1,0,0,0"));
        // No pane to inspect: everything empty — must NOT read as "old".
        assert!(!bracket_paste_flag_is_missing(""));
        assert!(!bracket_paste_flag_is_missing(",,,,,,,,,"));
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

    /// The core snapshot-hygiene guarantee, pinned against the COMPLETE
    /// output (not just a substring check) across three lines: every
    /// boundary gets a closing reset before its terminator, and the
    /// final, terminator-less line gets one at end-of-input. No SGR
    /// appears anywhere in this input, so the carryover log stays empty
    /// throughout and no restore bytes appear — isolating the boundary-
    /// reset behavior from the restore behavior pinned separately below.
    /// `usize::MAX` as the budget throughout this module's tests means
    /// "budget is not what this test is about" — see the dedicated
    /// budget-abort test for the byte-budget behavior itself.
    #[test]
    fn sanitize_snapshot_lines_resets_every_line_boundary_and_the_end() {
        let normalized = normalize_capture(b"one\ntwo\nthree\n");
        let sanitized = sanitize_snapshot_lines(&normalized, usize::MAX).unwrap();
        assert_eq!(sanitized, b"one\x1b[0m\r\ntwo\x1b[0m\r\nthree\x1b[0m");
    }

    /// Intra-line bytes are opaque to this transform: it must never
    /// rewrite SGR sequences a line already contains, and must survive
    /// non-UTF-8 pane content exactly like `normalize_capture` does (see
    /// that function's own pinned test). Pinned against the complete
    /// output byte vector, including a raw invalid-UTF-8 byte on the
    /// first line and a distinct escape on the second, later line. (A
    /// byte-string literal can embed `\xff` directly — it need not be
    /// valid UTF-8 the way an ordinary string literal would — so the
    /// expected value needs no separate `Vec` assembled via
    /// `extend_from_slice`.)
    #[test]
    fn sanitize_snapshot_lines_preserves_intra_line_bytes_exactly() {
        let normalized = normalize_capture(b"\xffa\x1b[31mb\nc\x1b[1md\n");
        let sanitized = sanitize_snapshot_lines(&normalized, usize::MAX).unwrap();
        assert_eq!(
            sanitized, b"\xffa\x1b[31mb\x1b[0m\r\n\x1b[31mc\x1b[1md\x1b[0m",
            "interior bytes on both lines must survive untouched — including the non-UTF-8 \
             byte and both escapes — with only the boundary reset and restore added: \
             {sanitized:?}"
        );
    }

    /// A line that already ends in its own reset (e.g. an app that
    /// explicitly wrote `\x1b[0m` right before its newline — see the
    /// `altscreen` fixture's `STATUS BAR` row) must NOT be special-cased
    /// out of the injection. Deliberately a SEMANTIC assertion (the
    /// bytes immediately before the terminator are a reset, redundant or
    /// not) rather than a pin of the complete output: the restore log
    /// this line's own reset feeds now also appears after the
    /// terminator (see the carryover tests below), so an exact full-
    /// output pin here would break the moment that unrelated behavior
    /// changed, despite this test's actual concern — "don't skip the
    /// boundary reset" — being unaffected by it.
    #[test]
    fn sanitize_snapshot_lines_does_not_skip_the_boundary_reset_on_an_already_reset_line() {
        let normalized = normalize_capture(b"styled\x1b[0m\nnext\n");
        let sanitized = sanitize_snapshot_lines(&normalized, usize::MAX).unwrap();
        let text = String::from_utf8_lossy(&sanitized);
        let boundary = text.find("\r\n").expect("single boundary in this input");
        assert!(
            text[..boundary].ends_with("\x1b[0m\x1b[0m"),
            "an already-reset line must still get its own boundary reset appended, doubling \
             harmlessly rather than being skipped: {text:?}"
        );
    }

    /// The carryover fix's own reason to exist: `capture-pane -e` only
    /// re-emits an SGR sequence when a cell's style CHANGES, so a
    /// background painted on row 1 and simply continued, unchanged, into
    /// row 2 is never re-stated in row 2's own bytes. A bare boundary
    /// reset (an earlier, simpler version of this transform) would
    /// therefore also silently erase row 2's inherited background — this
    /// pins that the fix instead restores it immediately after the
    /// terminator, while still closing row 1 off with its own reset.
    #[test]
    fn sanitize_snapshot_lines_restores_a_never_re_emitted_background_on_the_next_row() {
        let normalized = normalize_capture(b"\x1b[44mone\ntwo\n");
        let sanitized = sanitize_snapshot_lines(&normalized, usize::MAX).unwrap();
        assert_eq!(sanitized, b"\x1b[44mone\x1b[0m\r\n\x1b[44mtwo\x1b[0m");
    }

    /// Colon-delimited SGR subparameters (`\x1b[4:3m`, a curly
    /// underline — the form modern tmux emits for compound attributes)
    /// must carry over exactly like semicolon-delimited ones: this is
    /// `for_each_sgr_sequence`'s `:`-acceptance (item 2 of the swarm
    /// review) exercised through the full carryover path, not just the
    /// scanner in isolation.
    #[test]
    fn sanitize_snapshot_lines_restores_a_colon_form_sgr_sequence_on_the_next_row() {
        let normalized = normalize_capture(b"\x1b[4:3mone\ntwo\n");
        let sanitized = sanitize_snapshot_lines(&normalized, usize::MAX).unwrap();
        assert_eq!(sanitized, b"\x1b[4:3mone\x1b[0m\r\n\x1b[4:3mtwo\x1b[0m");
    }

    /// A full reset must actually CLEAR everything accumulated before
    /// it, not merely append onto the same log — pinned as an EXACT
    /// output comparison across two boundaries so that removing the
    /// `.clear()` call in `CarryoverLog::record` fails this test: row 1
    /// sets a background and then, still on row 1, resets it, so the
    /// restore that follows must carry only the reset (4 bytes), not the
    /// discarded background too (which `.clear()`'s removal would leave
    /// as a dead, un-erased prefix — 5 extra bytes this exact-match pin
    /// would catch even though it happens not to change the FINAL
    /// on-screen state either way, since a real reset wipes prior state
    /// regardless of what preceded it in the replayed byte stream).
    #[test]
    fn sanitize_snapshot_lines_a_full_reset_clears_prior_carryover() {
        let normalized = normalize_capture(b"\x1b[44mone\x1b[0m\ntwo\x1b[1mthree\nfour\n");
        let sanitized = sanitize_snapshot_lines(&normalized, usize::MAX).unwrap();
        assert_eq!(
            sanitized,
            b"\x1b[44mone\x1b[0m\x1b[0m\r\n\x1b[0mtwo\x1b[1mthree\x1b[0m\r\n\x1b[0m\x1b[1mfour\x1b[0m"
        );
    }

    /// [`MAX_SGR_CARRYOVER_LOG_BYTES`]'s own boundary, and the recovery
    /// semantics `CarryoverLog::record` documents: a log that lands
    /// EXACTLY at the cap still restores in full; one byte further
    /// clears it and suppresses BOTH restoration and further recording
    /// until the source stream's own next full reset, at which point
    /// restoration resumes from that reset's own sequence.
    #[test]
    fn sanitize_snapshot_lines_recovers_restoration_after_a_carryover_overflow() {
        // A single SGR sequence of digit-only params, sized so the WHOLE
        // `ESC[...]m` span lands exactly on the cap. Its only parameter
        // is a long run of `1`s (never `0`), so it is syntactically
        // valid SGR and ordinary (non-reset) carryover state, not a
        // special case.
        let params_len = MAX_SGR_CARRYOVER_LOG_BYTES - "\x1b[".len() - "m".len();
        let at_cap_sequence = format!("\x1b[{}m", "1".repeat(params_len));
        assert_eq!(at_cap_sequence.len(), MAX_SGR_CARRYOVER_LOG_BYTES);

        let at_cap_input = format!("{at_cap_sequence}one\ntwo\n");
        let sanitized =
            sanitize_snapshot_lines(&normalize_capture(at_cap_input.as_bytes()), usize::MAX)
                .unwrap();
        let expected = format!("{at_cap_sequence}one\x1b[0m\r\n{at_cap_sequence}two\x1b[0m");
        assert_eq!(
            sanitized,
            expected.as_bytes(),
            "a carryover log landing exactly at the cap must still restore in full"
        );

        // One byte over (a second, distinct SGR sequence tacked onto the
        // same row) must overflow the log, suppressing restoration for
        // row 2 AND row 3 (no reset appears until row 3's own
        // `\x1b[0;7m`) — row 4 must then restore exactly that reset's
        // sequence, proving recovery rather than permanent suppression.
        let over_cap_input = format!("{at_cap_sequence}\x1b[1mone\ntwo\n\x1b[0;7mthree\nfour\n");
        let sanitized =
            sanitize_snapshot_lines(&normalize_capture(over_cap_input.as_bytes()), usize::MAX)
                .unwrap();
        let expected = format!(
            "{at_cap_sequence}\x1b[1mone\x1b[0m\r\ntwo\x1b[0m\r\n\x1b[0;7mthree\x1b[0m\r\n\
             \x1b[0;7mfour\x1b[0m"
        );
        assert_eq!(
            sanitized,
            expected.as_bytes(),
            "an overflowed log must restore nothing until the source's own next full reset, \
             then resume restoring exactly that reset's sequence"
        );
    }

    /// An empty capture (nothing on the pane, or a header immediately
    /// followed by nothing) must not manufacture a reset out of thin
    /// air — there is no line boundary to sanitize.
    #[test]
    fn sanitize_snapshot_lines_of_empty_input_stays_empty() {
        assert_eq!(sanitize_snapshot_lines(b"", usize::MAX).unwrap(), b"");
    }

    /// The byte-budget guarantee (item 1 of the swarm review): a capture
    /// whose sanitized form would exceed a small, deliberately synthetic
    /// budget must abort with `None` mid-construction rather than ever
    /// finishing the oversized build — the whole point of threading the
    /// budget INTO this function instead of checking the finished
    /// length afterward. A large `boundary_count` (many short lines)
    /// with a tiny budget is exactly the amplification shape a post-hoc
    /// recheck would have let through transiently before rejecting.
    #[test]
    fn sanitize_snapshot_lines_aborts_mid_construction_over_budget() {
        let many_lines = "a\n".repeat(10_000);
        let normalized = normalize_capture(many_lines.as_bytes());
        assert!(
            sanitize_snapshot_lines(&normalized, 16).is_none(),
            "a capture whose sanitized size vastly exceeds a small budget must abort"
        );
    }

    /// [`sgr_is_full_reset`]'s three recognized shapes, plus the
    /// negative case that must NOT be mistaken for one of them: an
    /// ordinary attribute-setting sequence whose first parameter is
    /// non-zero.
    #[test]
    fn sgr_is_full_reset_recognizes_all_three_reset_shapes() {
        assert!(sgr_is_full_reset(b"\x1b[m"));
        assert!(sgr_is_full_reset(b"\x1b[0m"));
        assert!(sgr_is_full_reset(b"\x1b[0;1m"));
        assert!(!sgr_is_full_reset(b"\x1b[31m"));
    }

    /// [`for_each_sgr_sequence`] must find sequences ANYWHERE in a line
    /// (not just at its start or end) and must leave anything that is
    /// not a bare digits-and-semicolons-then-`m` CSI sequence alone,
    /// including a cursor-motion CSI sequence that happens to share the
    /// `ESC[` opener.
    #[test]
    fn for_each_sgr_sequence_finds_every_sgr_and_skips_other_csi_sequences() {
        let mut found = Vec::new();
        for_each_sgr_sequence(b"a\x1b[31mb\x1b[2;3Hc\x1b[0md", |seq| {
            found.push(seq.to_vec())
        });
        assert_eq!(found, vec![b"\x1b[31m".to_vec(), b"\x1b[0m".to_vec()]);
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

    /// The matching, alternate-screen case: header says "1 <this
    /// session>", so the capture body must come back (through
    /// `normalize_capture` then `sanitize_snapshot_lines`, whose own
    /// transforms are pinned separately) as `Captured`.
    #[test]
    fn parse_alt_screen_capture_matching_session_on_alt_screen() {
        let out = b"1 fh-abc123\nhello\nworld\n";
        match parse_alt_screen_capture(out, "fh-abc123", usize::MAX) {
            AltScreenCapture::Captured(bytes) => {
                assert_eq!(
                    bytes,
                    sanitize_snapshot_lines(&normalize_capture(b"hello\nworld\n"), usize::MAX)
                        .unwrap()
                );
            }
            _ => panic!("expected Captured, got a different outcome"),
        }
    }

    /// A "0" (primary-screen) header must short-circuit to `NotAlternate`
    /// regardless of what the capture body contains — this function must
    /// never even look at the body once the flag says primary, since a
    /// primary-screen pane's content is never worth storing.
    #[test]
    fn parse_alt_screen_capture_primary_screen_is_not_alternate() {
        let out = b"0 fh-abc123\nirrelevant body\n";
        assert!(matches!(
            parse_alt_screen_capture(out, "fh-abc123", usize::MAX),
            AltScreenCapture::NotAlternate
        ));
    }

    /// A session-name mismatch must win over an alternate-screen "1" —
    /// the stale-pane-id guard must reject the capture even when the flag
    /// alone would otherwise say it is worth keeping, since the content
    /// might belong to an entirely different session's pane.
    #[test]
    fn parse_alt_screen_capture_session_mismatch_overrides_alternate_flag() {
        let out = b"1 fh-other-session\nsome content\n";
        assert!(matches!(
            parse_alt_screen_capture(out, "fh-this-session", usize::MAX),
            AltScreenCapture::SessionMismatch
        ));
    }

    /// A black-box check, at the `parse_alt_screen_capture` level, that
    /// growth from sanitizing (a reset, plus a restore, per boundary)
    /// really does yield `TooLarge` rather than an over-cap `Captured`:
    /// a two-line body whose normalized (pre-sanitize) size is 4 bytes,
    /// sanitized up to 12, against a cap of 6 that comfortably admits
    /// the former but not the latter. The MECHANISM behind this (an
    /// abort mid-construction inside `sanitize_snapshot_lines`, not a
    /// separate post-hoc recheck) is pinned directly by
    /// `sanitize_snapshot_lines_aborts_mid_construction_over_budget`;
    /// this test only cares that the outcome `parse_alt_screen_capture`
    /// callers observe is correct.
    #[test]
    fn parse_alt_screen_capture_rejects_a_capture_that_only_exceeds_the_cap_after_sanitizing() {
        let out = b"1 fh-abc123\nx\ny\n";
        assert!(
            within_snapshot_cap(normalize_capture(b"x\ny\n").len(), 6),
            "test premise: the pre-sanitize body must itself clear the cap"
        );
        assert!(matches!(
            parse_alt_screen_capture(out, "fh-abc123", 6),
            AltScreenCapture::TooLarge
        ));
    }

    /// A byte length exactly AT the cap must be accepted; one byte over
    /// must not — the inclusive boundary `within_snapshot_cap` exists to
    /// pin directly, since it is the one line deciding whether a
    /// borderline capture or stored file is kept or discarded.
    #[test]
    fn within_snapshot_cap_accepts_the_boundary_and_rejects_one_over() {
        assert!(
            within_snapshot_cap(100, 100),
            "exactly at the cap must be accepted"
        );
        assert!(
            !within_snapshot_cap(101, 100),
            "one byte over the cap must be rejected"
        );
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

    /// The common case: a live pane and a dead one with a parseable exit
    /// code, keyed by PANE id (`%N`) exactly as `service.rs`'s
    /// `session_status` looks them up via `Terminal::pane` — not by
    /// session name, which two panes in the same session's window could
    /// share (see `pane_states`'s own docs on why keying by session name
    /// would let one pane's entry silently clobber another's).
    #[test]
    fn parse_pane_states_reads_live_and_dead_panes() {
        let out = "%0 fh-alive 0 0\n%1 fh-dead 1 3\n";
        let states = parse_pane_states(out);
        assert_eq!(
            states.get("%0"),
            Some(&PaneState {
                session_name: "fh-alive".to_string(),
                dead: false,
                exit_code: None
            })
        );
        assert_eq!(
            states.get("%1"),
            Some(&PaneState {
                session_name: "fh-dead".to_string(),
                dead: true,
                exit_code: Some(3)
            })
        );
    }

    /// A live pane's trailing status field must never be trusted, even if
    /// tmux happens to report a leftover nonzero value there — only a
    /// dead pane's status is a real exit code. Parsing it regardless would
    /// fabricate a death that has not happened.
    #[test]
    fn parse_pane_states_ignores_status_of_a_live_pane() {
        let states = parse_pane_states("%0 fh-live 0 7\n");
        assert_eq!(
            states.get("%0"),
            Some(&PaneState {
                session_name: "fh-live".to_string(),
                dead: false,
                exit_code: None
            })
        );
    }

    /// A dead pane whose status tmux could not express as a plain integer
    /// (a signal death, empirically an empty field on some tmux builds)
    /// must decode as `exit_code: None`, not fail the whole parse — this
    /// is the same honest gap `SessionStatus::Exited` documents.
    #[test]
    fn parse_pane_states_tolerates_an_unparseable_dead_status() {
        let states = parse_pane_states("%0 fh-signalled 1 \n");
        assert_eq!(
            states.get("%0"),
            Some(&PaneState {
                session_name: "fh-signalled".to_string(),
                dead: true,
                exit_code: None
            })
        );
    }

    /// A row whose `pane_dead` field is missing entirely, or holds
    /// anything other than an exact `"0"`/`"1"`, must be skipped rather
    /// than silently defaulting to `dead: false` — the bug this strict
    /// parse exists to close. A prior, looser version of this parser
    /// treated "not exactly `1`" as "alive", which happily accepted a
    /// missing field as alive too; that pane simply must not appear in the
    /// map at all, so `session_status`'s existing "absent means Exited"
    /// handling covers it instead of a fabricated `Alive`. Covers every
    /// stage a row can go missing: no fields past `pane_id`, no fields
    /// past `session_name`, and a `pane_dead` field present but not
    /// recognized.
    #[test]
    fn parse_pane_states_skips_rows_with_a_missing_or_unrecognized_dead_field() {
        let states = parse_pane_states("%0\n%1 fh-sess\n%2 fh-sess maybe\n%3 fh-sess 2\n");
        assert!(
            states.is_empty(),
            "every row here is malformed and must be skipped: {states:?}"
        );
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
                Some(ControlLine::Payload(escaped)) => {
                    let bytes = decode_output_payload(&mut decoder, escaped);
                    if !bytes.is_empty() {
                        events.push(OutputEvent::Bytes(bytes));
                    }
                }
                Some(ControlLine::Paused) => events.push(OutputEvent::Paused),
                Some(ControlLine::Exit) => break,
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

    /// The exact attach-cutover text, pinned like every other generated
    /// tmux command in this module. Both flags ride one comma-separated
    /// `-f` list, and the `!` prefix is what CLEARS `no-output` rather
    /// than setting a flag by that name — get either detail wrong and the
    /// client silently either never receives output or never gets the
    /// `pause-after` backstop, neither of which fails loudly.
    #[test]
    fn attach_cutover_sets_pause_after_while_clearing_no_output() {
        assert_eq!(
            attach_cutover_command(),
            format!("refresh-client -f !no-output,pause-after={TMUX_PAUSE_AFTER_SECS}")
        );
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

    /// The replay group's command list and ORDER are the contract
    /// `snapshot_then_cutover` reads reply blocks against, and both the
    /// attach and catch-up paths depend on it being identical apart from
    /// the final cutover. Pinned against the complete string so a
    /// reordering — which would silently pair the modes reply with the
    /// history capture — cannot pass.
    #[test]
    fn replay_command_group_differs_only_in_its_cutover() {
        let attach = replay_command_group("%1", &attach_cutover_command());
        let resume = replay_command_group("%1", &continue_pane_command("%1"));
        assert_eq!(
            attach,
            format!(
                "display-message -p -t %1 '{PANE_MODE_FORMAT}' ; \
                 capture-pane -p -e -N -t %1 -S -{HISTORY_LIMIT} ; \
                 capture-pane -p -e -N -t %1 ; \
                 refresh-client -f !no-output,pause-after={TMUX_PAUSE_AFTER_SECS}\n"
            )
        );
        let common = attach
            .strip_suffix(&format!("{}\n", attach_cutover_command()))
            .expect("attach group ends with its cutover");
        assert_eq!(
            resume,
            format!("{common}{}\n", continue_pane_command("%1")),
            "the two groups must share every snapshot command verbatim"
        );
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
        _dir: tempfile::TempDir,
    }

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
        async fn start() -> ScratchServer {
            let dir = tempfile::tempdir().expect("tempdir");
            let driver = TmuxDriver::new(dir.path());
            driver.ensure_server().await.expect("tmux server");
            ScratchServer { driver, _dir: dir }
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
            .resume_paused_with_replay(&pane)
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
        stream.shutdown().await;
    }

    /// Parser-level robustness only: an empty string must parse as an
    /// empty map, not panic. This is NOT the genuinely-empty-server case
    /// (`pane_states`'s own `LIST_PANES_EMPTY_SERVER_DIAGNOSTIC` handling)
    /// — a real empty server makes the tmux COMMAND itself fail with `"no
    /// current target"` before this function ever sees any output to
    /// parse, so `parse_pane_states` in production never actually
    /// receives an empty string from that path. This test exists purely
    /// so the parser itself does not panic or misbehave if it ever did
    /// receive one (a future caller feeding it something else, say).
    #[test]
    fn parse_pane_states_handles_empty_output() {
        assert!(parse_pane_states("").is_empty());
    }
}
