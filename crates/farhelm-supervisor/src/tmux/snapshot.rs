//! Alternate-screen snapshot parsing and line-boundary sanitization.

use super::normalize_capture;

/// Outcome of [`super::TmuxDriver::capture_alt_screen_if_active`]: whether there
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
/// Split out from [`super::TmuxDriver::capture_alt_screen_if_active`] purely so
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
pub(super) fn parse_alt_screen_capture(
    out: &[u8],
    session: &str,
    max_bytes: usize,
) -> AltScreenCapture {
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
/// `claude` never showed (the snapshot tests in
/// `crates/farhelm/tests/e2e/session_lifecycle.rs` pin this boundary).
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
/// from [`super::TmuxDriver::capture_alt_screen_if_active`]'s own `max_bytes`).
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
pub(super) fn sanitize_snapshot_lines(normalized: &[u8], budget: usize) -> Option<Vec<u8>> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
