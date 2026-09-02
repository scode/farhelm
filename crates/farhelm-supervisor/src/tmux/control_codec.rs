//! Tmux control-mode framing and output decoding, plus the authoritative
//! pane-discovery parser.

use super::{PaneState, strip_line_ending};
use anyhow::{Context, bail};
use std::collections::{HashMap, HashSet};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

/// One tmux control-mode notification may expand each terminal byte into
/// a four-byte octal escape. Bound the line above the protocol frame cap
/// without rejecting the largest valid escaped notification.
const MAX_CONTROL_LINE: usize = farhelm_proto::MAX_FRAME_LEN as usize * 4 + 1024;

/// The AUTHORITATIVE half of pane discovery: everything tmux itself owns
/// about a pane, and nothing a pane can write.
///
/// Field order is not arbitrary, and both ends of it are load-bearing.
/// `#{session_name}` goes LAST and is taken as the whole remainder of the
/// line, because a session name may legally contain spaces (and a pane
/// that inherited `TMUX` can `rename-session` into one); every field
/// before it is a fixed tmux-owned shape that the parse validates
/// outright. `#{pane_dead_status}` is the one tmux field that expands
/// EMPTY for a healthy pane, so it rides through tmux's own `#{?cond,a,b}`
/// conditional into a literal `-` placeholder rather than vanishing and
/// shifting everything after it.
///
/// The exit status carries a literal `s` PREFIX rather than riding tmux's
/// `#{?cond,a,b}` conditional, and that is a bug fix rather than a style
/// choice: tmux's conditional treats the string `"0"` as FALSE, so a pane
/// that exited CLEANLY — the single most common exit code there is —
/// expanded to the "no status" branch and every clean exit read back as
/// `Exited { exit_code: None }`. A constant prefix cannot be falsy.
///
/// No window user options here, deliberately — see
/// [`PANE_MARKER_FORMAT`], which fetches them separately precisely
/// because they are the fields a pane can write.
pub(super) const PANE_FACT_FORMAT: &str = concat!(
    "#{pane_id} #{window_id} #{window_index} #{pane_dead} ",
    "s#{pane_dead_status} #{session_name}"
);

/// The PANE-WRITABLE half of pane discovery: the two farhelm window
/// markers, fetched in a query of their own.
///
/// Separate from [`PANE_FACT_FORMAT`] because these are the only fields
/// anything outside this supervisor can set, and a value carrying a
/// NEWLINE can fabricate an entire extra row in any format output. Keeping
/// them out of the authoritative query is what makes that fabrication
/// harmless: [`join_pane_markers`] admits a marker row only for a pane the
/// fact query independently reported, and drops BOTH rows when a pane id
/// appears twice — so a forged row either names a pane that does not exist
/// (dropped) or collides with that pane's real row (both dropped, leaving
/// the pane merely unmarked). Neither outcome lets one pane's option value
/// hand another pane a marker.
///
/// Both markers still ride ONE row rather than one query each: with the
/// join and the duplicate rule above, ordering between two writable fields
/// no longer decides anything.
pub(super) const PANE_MARKER_FORMAT: &str = concat!(
    "#{pane_id} #{?#{@farhelm-agent},#{@farhelm-agent},-} ",
    "#{?#{@farhelm-tab},#{@farhelm-tab},-}"
);

/// The placeholder both formats emit where a value would otherwise be
/// empty. Never a legal id, so it needs no special case beyond the shape
/// checks every field already gets.
const EMPTY_FIELD: &str = "-";

/// The numeric part of a tmux id (`%12` → 12, `@7` → 7), or `None` when
/// the id is not exactly `<sigil><digits>`.
///
/// Strict on purpose. It doubles as the shape check for a pane or window
/// id — the ids a fabricated row would have to spell — and as the parse
/// for the ordinals every consumer actually wants, because tmux hands both
/// out from monotonic counters and `@10` sorts before `@9` as a string.
pub(super) fn tmux_ordinal(id: &str, sigil: char) -> Option<u64> {
    let digits = id.strip_prefix(sigil)?;
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}

/// A marker value as read back out of a tmux window option, or `None` when
/// it is not one this supervisor could have written.
///
/// SYNTAX only — see [`PaneState::tab`] for what that does and does not
/// establish. The empty placeholder and any other shape both answer
/// `None`, so callers have one case to handle rather than three.
fn parse_marker(value: &str) -> Option<String> {
    (value != EMPTY_FIELD && crate::scope::is_uuid_shaped(value)).then(|| value.to_string())
}

/// Parse [`PANE_FACT_FORMAT`]'s `list-panes -a` output into a per-pane
/// map. Split out from [`super::TmuxDriver::pane_states`] purely so this parsing
/// is unit-testable against constructed strings, without a real tmux
/// server behind it — the same reasoning `PaneModes::parse` and
/// `parse_stat` split their own parsing out for elsewhere in this
/// codebase.
///
/// Every fixed field is validated by SHAPE, not merely by presence: pane
/// and window ids must be `%N`/`@N`, the window index must be numeric,
/// `pane_dead` must be exactly `"0"` or `"1"`. A row failing any of them
/// is skipped outright — not inserted with a guessed value — because a
/// fabricated liveness claim is precisely what `session_status`'s "missing
/// from the map means Exited" contract exists to avoid, and a row this
/// function could not parse is not evidence of anything.
///
/// A pane id appearing TWICE drops the pane entirely rather than letting
/// either row win. tmux emits exactly one row per pane, so a second one
/// can only have been fabricated — by a newline inside the one remaining
/// caller-controlled field, a session name (`rename-session` is available
/// to any pane that inherited `TMUX`). Dropping is the safe direction: the
/// affected session reports `Exited { exit_code: None }` through the
/// ordinary absent-pane path, which is a nuisance a same-server pane can
/// inflict and never a liveness claim it can forge.
pub(super) fn parse_pane_facts(out: &str) -> HashMap<String, PaneState> {
    let mut states: HashMap<String, PaneState> = HashMap::new();
    let mut seen_twice: HashSet<String> = HashSet::new();
    for line in out.lines() {
        // `splitn` on the LAST field: a session name may contain spaces,
        // so everything past the fixed prefix belongs to it verbatim.
        let mut fields = line.splitn(6, ' ');
        let Some(pane_ordinal) = fields.next().and_then(|id| tmux_ordinal(id, '%')) else {
            continue;
        };
        let pane_id = format!("%{pane_ordinal}");
        let Some(window) = fields.next() else {
            continue;
        };
        let Some(window_ordinal) = tmux_ordinal(window, '@') else {
            continue;
        };
        let Some(window_index) = fields.next().and_then(|index| index.parse::<u64>().ok()) else {
            continue;
        };
        let dead = match fields.next() {
            Some("0") => false,
            Some("1") => true,
            _ => continue,
        };
        // Only trust the status field once the pane is actually dead:
        // tmux leaves it at a stale or placeholder value while alive, and
        // parsing it regardless would risk fabricating an exit code for a
        // pane that has not exited at all.
        // `s`-prefixed so the field is never empty (see the format's own
        // docs); `s` alone is tmux reporting no status at all.
        let status = fields.next().and_then(|field| field.strip_prefix('s'));
        let exit_code = if dead {
            status.and_then(|s| s.parse().ok())
        } else {
            None
        };
        let Some(session_name) = fields.next().filter(|name| !name.is_empty()) else {
            continue;
        };
        if states.contains_key(&pane_id) {
            seen_twice.insert(pane_id);
            continue;
        }
        states.insert(
            pane_id,
            PaneState {
                session_name: session_name.to_string(),
                dead,
                window: window.to_string(),
                window_ordinal,
                pane_ordinal,
                window_index,
                tab: None,
                agent: None,
                exit_code,
            },
        );
    }
    for pane in seen_twice {
        states.remove(&pane);
    }
    states
}

/// Whether any tmux session in `states` holds panes from more than one
/// window — the cheap precondition [`super::TmuxDriver::pane_states`] uses to
/// decide whether the marker query can matter at all.
pub(super) fn any_session_has_several_windows(states: &HashMap<String, PaneState>) -> bool {
    let mut first_window: HashMap<&str, &str> = HashMap::new();
    states.values().any(|state| {
        first_window
            .insert(state.session_name.as_str(), state.window.as_str())
            .is_some_and(|seen| seen != state.window)
    })
}

/// Fold [`PANE_MARKER_FORMAT`]'s output into an already-parsed fact map.
///
/// The authoritative map decides which panes exist; this only ever
/// DECORATES panes already in it, which is what makes a fabricated marker
/// row unable to invent a pane. A pane id appearing twice among the
/// syntactically valid marker rows leaves that pane unmarked, for
/// [`parse_pane_facts`]'s reason applied to the fields most likely to be
/// hostile — and unmarked is the safe degradation here, since it costs a
/// tab its listing rather than pointing an operation at the wrong pane.
///
/// Rows are validated whole: exactly three space-separated tokens, a
/// well-formed pane id, and two fields that are each either the empty
/// placeholder or a complete minted-shaped id. `uuid` plus trailing
/// garbage is not a uuid.
pub(super) fn join_pane_markers(states: &mut HashMap<String, PaneState>, out: &str) {
    let mut seen: HashSet<String> = HashSet::new();
    let mut ambiguous: HashSet<String> = HashSet::new();
    for line in out.lines() {
        let fields: Vec<&str> = line.split(' ').filter(|f| !f.is_empty()).collect();
        let [pane, agent, tab] = fields[..] else {
            continue;
        };
        let Some(ordinal) = tmux_ordinal(pane, '%') else {
            continue;
        };
        let pane_id = format!("%{ordinal}");
        if !states.contains_key(&pane_id) {
            // A marker row for a pane the authoritative query never
            // reported: either fabricated outright, or a pane that
            // appeared between the two queries. Neither is something to
            // invent a pane from.
            continue;
        }
        if !seen.insert(pane_id.clone()) {
            ambiguous.insert(pane_id);
            continue;
        }
        let (agent, tab) = (parse_marker(agent), parse_marker(tab));
        if let Some(state) = states.get_mut(&pane_id) {
            state.agent = agent;
            state.tab = tab;
        }
    }
    for pane in ambiguous {
        if let Some(state) = states.get_mut(&pane) {
            state.agent = None;
            state.tab = None;
        }
    }
}

/// The identity tmux repeats on one command reply's begin/end markers.
///
/// Pane content is allowed to start with `%`, including text that looks
/// like some other command's marker. Only an end marker with this exact
/// identity closes the block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ControlBlockId {
    timestamp: u64,
    command: u64,
    flags: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ControlMarker {
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
pub(super) fn parse_control_marker(line: &[u8]) -> Option<ControlMarker> {
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
///
/// `own_pane` is the pane the caller's client speaks for, and it makes
/// the difference between "the cutover ordering broke" and "some other
/// window is busy". A control client attached to a session hears every
/// pane on it (see [`super::OutputStream`]), and on the CATCH-UP path — where
/// only this stream's own pane is paused while the rest of the session
/// keeps running — a neighbour's output notification landing between two
/// of this group's reply blocks is entirely ordinary. Treating that as
/// the ordering violation it would be for our OWN pane (which is exactly
/// what this did before tabs existed, when a session had one pane and the
/// two cases could not be told apart) would fail a perfectly healthy
/// resume, tearing down the attachment.
///
/// The foreign-output tolerance is scoped to BETWEEN blocks and does not
/// extend inside one, deliberately. Block bodies are terminal content:
/// `capture-pane` output is unescaped, so a pane displaying a control-mode
/// transcript can legitimately contain a line that reads exactly like a
/// notification, and dropping those would corrupt the replay. tmux is
/// audited (3.7b, a neighbouring pane driven at full tilt across a
/// complete replay group) never to interleave notifications INSIDE a
/// command's reply block — commands run to completion in the server's own
/// loop before queued pane output is flushed — which is the same
/// assumption this function has always made and the reason its body loop
/// appends everything unconditionally.
pub(super) async fn read_command_block<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    line: &mut Vec<u8>,
    deadline: tokio::time::Instant,
    purpose: &str,
    own_pane: &str,
) -> anyhow::Result<Vec<u8>> {
    let own_pane = own_pane.as_bytes();
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
            // Both dialects, for the same reason: live output for OUR pane
            // arriving BETWEEN reply blocks means the cutover ordering
            // broke, and silently folding those bytes into the next
            // block's captured output would manufacture terminal history.
            // Which dialect it arrives in depends only on whether
            // `pause-after` is set on this client, which is not this
            // function's business to know.
            None if classify_control_line(stripped).is_some_and(
                |line| matches!(line, ControlLine::Payload { pane, .. } if pane == own_pane),
            ) =>
            {
                bail!("tmux emitted live output before the replay cutover completed");
            }
            None if stripped.starts_with(b"%exit") => {
                bail!("tmux control client exited before the {purpose} reply");
            }
            // Everything else — a neighbouring pane's output or pause, a
            // `%layout-change`, a `%window-renamed` — is chatter between
            // blocks, exactly as it is to `OutputStream::next_output`.
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
pub(super) async fn read_control_line_before<R: AsyncRead + Unpin>(
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
pub(super) async fn read_control_line<R: AsyncRead + Unpin>(
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
pub(super) fn warn_once_about_missing_bracket_paste(line: &str) {
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

/// Incrementally remove tmux passthrough wrappers before bytes reach the
/// real terminal.
///
/// tmux may split `ESC P tmux; ... ESC \` across output
/// notifications (either dialect), including inside the opener or on either side of an
/// escaped `ESC`. Keeping only the parser's few bytes of boundary state
/// avoids both split-wrapper corruption and an unbounded whole-wrapper
/// buffer for large inline images.
#[derive(Default)]
pub(super) struct PassthroughDecoder {
    opener: Vec<u8>,
    in_wrapper: bool,
    pending_escape: bool,
}

/// One contiguous decoded output segment and whether tmux forwarded it.
///
/// A passthrough wrapper tells tmux not to parse its payload. The supervisor
/// therefore needs this provenance after unwrapping: terminal-query filtering
/// is safe only for ordinary pane bytes that tmux could already have answered.
pub(super) struct DecodedOutput {
    pub(super) bytes: Vec<u8>,
    pub(super) passthrough: bool,
}

/// Append a byte without losing the boundary that determines downstream
/// terminal-query filtering.
fn push_decoded_byte(output: &mut Vec<DecodedOutput>, byte: u8, passthrough: bool) {
    if let Some(segment) = output
        .last_mut()
        .filter(|segment| segment.passthrough == passthrough)
    {
        segment.bytes.push(byte);
    } else {
        output.push(DecodedOutput {
            bytes: vec![byte],
            passthrough,
        });
    }
}

impl PassthroughDecoder {
    const OPEN: &'static [u8] = b"\x1bPtmux;";

    /// Decode one chunk while retaining whether tmux parsed each output run.
    ///
    /// Empty output means the chunk ended inside an opener or immediately
    /// after an escaped byte; the next call resumes from that exact state.
    /// Separate runs are kept in order because ordinary pane bytes can sit on
    /// either side of a passthrough payload in one notification.
    fn push(&mut self, bytes: &[u8]) -> Vec<DecodedOutput> {
        let mut out = Vec::with_capacity(bytes.len());
        for &byte in bytes {
            if self.in_wrapper {
                if self.pending_escape {
                    self.pending_escape = false;
                    match byte {
                        // Payload ESC bytes are doubled inside a wrapper.
                        0x1b => push_decoded_byte(&mut out, 0x1b, true),
                        // A single ESC + backslash closes the wrapper.
                        b'\\' => self.in_wrapper = false,
                        // Malformed but lossless: preserve a lone ESC and
                        // its follower instead of inventing a sequence.
                        other => {
                            push_decoded_byte(&mut out, 0x1b, true);
                            push_decoded_byte(&mut out, other, true);
                        }
                    }
                } else if byte == 0x1b {
                    self.pending_escape = true;
                } else {
                    push_decoded_byte(&mut out, byte, true);
                }
                continue;
            }

            self.opener.push(byte);
            while !Self::OPEN.starts_with(&self.opener) {
                push_decoded_byte(&mut out, self.opener.remove(0), false);
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
pub(super) fn unwrap_passthrough(bytes: &[u8]) -> Vec<u8> {
    let mut decoder = PassthroughDecoder::default();
    let mut out = flatten_decoded_output(decoder.push(bytes));
    if decoder.in_wrapper {
        // A one-shot caller cannot wait for a split wrapper. Preserve it
        // byte-for-byte; the streaming path never needs this fallback.
        return bytes.to_vec();
    }
    out.append(&mut decoder.opener);
    out
}

/// Flatten decoded segments for test-only callers that do not act on their
/// provenance. Production live output keeps the segments separate until it
/// decides whether query filtering may inspect them.
#[cfg(test)]
fn flatten_decoded_output(output: Vec<DecodedOutput>) -> Vec<u8> {
    output
        .into_iter()
        .flat_map(|segment| segment.bytes)
        .collect()
}

/// Remove the newline control mode adds after one command's stdout.
///
/// This is separate from [`super::normalize_capture`]: capture-pane already
/// terminates its final row, so a command block contains two trailing
/// newlines. One belongs to control mode and is removed here; the other
/// belongs to capture-pane and is removed during terminal normalization.
pub(super) fn strip_command_output_terminator(output: &[u8]) -> &[u8] {
    // Same trailing-CRLF-or-LF shape as a notification's own terminator,
    // so it shares that implementation rather than repeating it; the two
    // stay distinct FUNCTIONS because they answer different questions
    // (see this one's docs on the two newlines a command block carries).
    strip_line_ending(output)
}

/// What one control-mode notification line means to
/// [`super::OutputStream::next_output`], before any decoding happens.
///
/// Split out from the read loop as a pure function over one line
/// specifically so the notification GRAMMAR is unit-testable without a
/// tmux server or a live process behind it — the same reasoning
/// [`parse_pane_facts`] and [`super::PaneModes::parse`] were split out for.
/// Shapes worth testing here are otherwise awkward or impossible to
/// provoke on demand from a real server: a `%pause` requires stalling a
/// client for seconds, and a future tmux's extra `%extended-output`
/// argument does not exist yet at all.
///
/// `None` is "nothing this stream cares about" — command replies,
/// `%layout-change`, `%window-renamed`, and anything a newer tmux invents.
pub(super) enum ControlLine<'a> {
    /// Escaped pane bytes from either output dialect, still to be
    /// unescaped and passthrough-decoded, together with the pane id the
    /// notification named.
    ///
    /// The pane id was dropped on the floor before PLAN_M4.md item 2: a
    /// session had exactly one pane, so every notification a control
    /// client saw was necessarily its own. Tabs make a session's control
    /// client see other windows' panes too, so the id has to survive
    /// classification for [`super::OutputStream::next_output`] to filter on.
    Payload { pane: &'a [u8], escaped: &'a [u8] },
    /// `%pause <pane-id>`, carrying that pane id for the same reason
    /// `Payload` does.
    Paused { pane: &'a [u8] },
    /// `%exit`: this control client is going away. Carries no pane —
    /// unlike the two above it is a statement about the CLIENT, so it
    /// applies whichever pane this stream is filtering for.
    ///
    /// It does carry tmux's own REASON, which is the whole of the
    /// diagnostic that survives this event: everything downstream
    /// collapses to "the stream ended", and a client that vanished on a
    /// loaded CI machine is otherwise indistinguishable from one whose
    /// session was deliberately killed. tmux writes either a bare `%exit`
    /// or `%exit <reason>` (`server exited`, `no server running`, and
    /// friends), so this is empty for the bare form rather than absent.
    Exit { reason: &'a [u8] },
}

/// Classify one already-terminator-stripped notification line. See
/// [`ControlLine`] for why this is a free function.
pub(super) fn classify_control_line(line: &[u8]) -> Option<ControlLine<'_>> {
    if let Some(rest) = line.strip_prefix(b"%output ") {
        // Format: "%<pane-id> <escaped-data>".
        split_output_payload(rest).map(|(pane, escaped)| ControlLine::Payload { pane, escaped })
    } else if let Some(rest) = line.strip_prefix(b"%extended-output ") {
        // Format: "%<pane-id> <age> ... : <escaped-data>".
        split_extended_output_payload(rest)
            .map(|(pane, escaped)| ControlLine::Payload { pane, escaped })
    } else if let Some(pane) = line.strip_prefix(b"%pause ") {
        // Format: "%pause %<pane-id>", the whole line. Trailing content
        // would mean a shape this build does not understand, so the id is
        // taken verbatim and simply fails to match any stream's pane.
        Some(ControlLine::Paused { pane })
    } else if let Some(rest) = line.strip_prefix(b"%exit") {
        // Both documented forms in one arm: a bare `%exit` leaves nothing
        // after the marker, `%exit <reason>` leaves a space then the
        // reason. The separator is stripped so the reason reads cleanly in
        // a log line; anything else trailing the marker is carried through
        // verbatim rather than rejected, since this is diagnostics.
        let reason = rest.strip_prefix(b" ").unwrap_or(rest);
        Some(ControlLine::Exit { reason })
    } else {
        None
    }
}

/// Turn one notification's escaped payload into provenance-carrying pane runs.
///
/// This undoes control-mode octal escaping, then unwraps the stateful tmux
/// passthrough framing. A returned passthrough run was forwarded by tmux and
/// must not be treated as a query tmux already answered.
///
/// One function for BOTH dialects, deliberately. That matters more than
/// it looks: [`PassthroughDecoder`] carries state ACROSS notifications (a
/// `ESC P tmux;` wrapper may be split over any number of them), so a
/// second decode path — or one dialect bypassing the decoder — would
/// corrupt any wrapper that happens to straddle a dialect boundary. An
/// empty result is normal and means the chunk ended mid-wrapper; the caller
/// keeps reading.
pub(super) fn decode_output_payload(
    decoder: &mut PassthroughDecoder,
    escaped: &[u8],
) -> Vec<DecodedOutput> {
    decoder.push(&unescape_control_output(escaped))
}

/// Split a `%output` notification's tail (everything past `"%output "`)
/// into its pane id and its escaped payload.
///
/// `None` for a malformed line with no separator at all — treated as
/// chatter rather than as empty output, so a truncated notification can
/// never be mistaken for the pane genuinely emitting nothing.
fn split_output_payload(rest: &[u8]) -> Option<(&[u8], &[u8])> {
    let separator = rest.iter().position(|&byte| byte == b' ')?;
    Some((&rest[..separator], &rest[separator + 1..]))
}

/// Split an `%extended-output` notification's tail (everything past
/// `"%extended-output "`) into its pane id and its escaped payload.
///
/// The documented shape is `pane-id age ... : value`, where tmux
/// explicitly reserves the space between the age and the `:` for future
/// arguments a client "should ignore". So this deliberately does NOT
/// count fields: it takes the FIRST field as the pane id (tmux documents
/// it first and has never moved it) and then scans space-separated fields
/// for the first one that is EXACTLY `":"`, taking everything after it —
/// which is what makes a future tmux adding a field a no-op here instead
/// of a payload shifted by one token. Anchoring on a lone `:` field (not
/// the first `:` byte) matters because the payload itself routinely
/// contains colons — pane output is arbitrary bytes.
///
/// `None` when no such separator field exists, for the same reason
/// [`split_output_payload`] returns `None`: a line this function cannot
/// locate a payload in is chatter, not empty output.
fn split_extended_output_payload(rest: &[u8]) -> Option<(&[u8], &[u8])> {
    let pane_end = rest
        .iter()
        .position(|&byte| byte == b' ')
        .unwrap_or(rest.len());
    let pane = &rest[..pane_end];
    let mut offset = pane_end + 1;
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
            return Some((pane, rest.get(end + 1..).unwrap_or(&[])));
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
    use super::super::{CONTROL_EXCHANGE_TIMEOUT, normalize_capture};
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
            let mut actual = flatten_decoded_output(decoder.push(&wrapped[..split]));
            actual.extend(flatten_decoded_output(decoder.push(&wrapped[split..])));
            assert!(!decoder.in_wrapper, "wrapper left open at split {split}");
            assert!(
                !decoder.pending_escape,
                "escape left pending at split {split}"
            );
            actual.extend(&decoder.opener);
            assert_eq!(actual, expected, "failed at split {split}");
        }
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
            "%0",
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
                "%0",
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

    /// The other side of that guard, and the one terminal tabs made
    /// necessary: a NEIGHBOURING pane's output between reply blocks is
    /// ordinary chatter, not a broken cutover.
    ///
    /// A control client attaches to a tmux SESSION and therefore hears
    /// every window's panes. On the catch-up path only this stream's own
    /// pane is paused, so a busy tab in another window emits between the
    /// replay group's blocks routinely — audited against tmux 3.7b by
    /// driving a second pane at full tilt across a complete resume group.
    /// Failing there would tear down a perfectly healthy attachment every
    /// time the user had a busy tab open, which is precisely the case
    /// tabs make common.
    ///
    /// Both dialects, and both a payload and a `%pause`, because a foreign
    /// pane can produce either.
    #[tokio::test]
    async fn a_neighbouring_panes_notifications_between_blocks_are_not_a_broken_cutover() {
        // TWO complete blocks read back to back, with foreign chatter
        // before the first, BETWEEN them, and in both dialects — the
        // shape a replay group actually meets on the catch-up path, where
        // only this stream's own pane is paused.
        let mut input =
            b"%output %7 other window\n%extended-output %7 0 : more\n%pause %7\n".to_vec();
        input.extend_from_slice(b"%begin 10 20 1\nmodes\n%end 10 20 1\n");
        input.extend_from_slice(b"%output %7 still busy\n%extended-output %7 3 x : lots\n");
        input.extend_from_slice(b"%layout-change @3 whatever\n");
        input.extend_from_slice(b"%begin 10 21 1\nhistory\n%end 10 21 1\n");
        let mut reader = BufReader::new(&input[..]);
        let mut line = Vec::new();
        let deadline = tokio::time::Instant::now() + CONTROL_EXCHANGE_TIMEOUT;
        let modes = read_command_block(&mut reader, &mut line, deadline, "pane modes", "%0")
            .await
            .expect("a neighbouring pane's chatter must not fail the first block");
        let history = read_command_block(&mut reader, &mut line, deadline, "history", "%0")
            .await
            .expect("nor the second");
        assert_eq!(
            (modes.as_slice(), history.as_slice()),
            (&b"modes\n"[..], &b"history\n"[..]),
            "each block's own output must be exactly its own, with the chatter around it              neither folded in nor treated as a broken cutover"
        );
    }

    /// Notifications must carry their pane id through classification, and
    /// [`crate::tmux::OutputStream::next_output`] must drop the ones that are not its
    /// own pane's.
    ///
    /// This is the whole of "each terminal's stream carries only its own
    /// pane" (PLAN_M4.md item 3). Without it, opening a tab would spray
    /// that tab's shell output into every already-attached terminal of the
    /// same session, and a foreign pane's `%pause` would send this
    /// terminal through a reset-and-replay catch-up for bytes it never
    /// carried. Exercised through `classify_control_line` (the same
    /// function `next_output` calls) rather than a live tmux, so the
    /// notification GRAMMAR is what is pinned.
    #[test]
    fn notifications_carry_their_pane_id_so_a_stream_can_keep_only_its_own() {
        let mine = b"%0";
        let mut kept = Vec::new();
        let mut paused_for_me = false;
        for line in [
            &b"%output %0 mine"[..],
            &b"%output %11 theirs"[..],
            &b"%extended-output %0 0 : mine-extended"[..],
            &b"%extended-output %11 0 : theirs-extended"[..],
            &b"%pause %11"[..],
            &b"%pause %0"[..],
        ] {
            match classify_control_line(line) {
                Some(ControlLine::Payload { pane, escaped }) if pane == mine => {
                    kept.push(String::from_utf8_lossy(escaped).into_owned());
                }
                Some(ControlLine::Paused { pane }) if pane == mine => paused_for_me = true,
                _ => {}
            }
        }
        assert_eq!(
            kept,
            vec!["mine".to_string(), "mine-extended".to_string()],
            "only this pane's payloads may reach the decoder"
        );
        assert!(
            paused_for_me,
            "this pane's own %pause must still surface — a swallowed one loses bytes for good"
        );
        // `%0` must not match `%01` or `%011`: pane ids are compared as
        // complete fields, and a prefix match would fuse two panes.
        assert!(matches!(
            classify_control_line(b"%output %01 not mine"),
            Some(ControlLine::Payload { pane, .. }) if pane != mine
        ));
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
            "%0",
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
            "%0",
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
            read_command_block(&mut reader, &mut line, deadline, purpose, "%0")
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

    /// The common case: a live pane and a dead one with a parseable exit
    /// code, keyed by PANE id (`%N`) exactly as `service.rs`'s
    /// `session_status` looks them up via `Terminal::pane` — not by
    /// session name, which two panes in the same session's window could
    /// share (see `pane_states`'s own docs on why keying by session name
    /// would let one pane's entry silently clobber another's).
    ///
    /// Also pins that the ORDINALS are parsed rather than left to the
    /// caller: every consumer wants creation order, and `@10` sorts
    /// before `@9` as a string.
    #[test]
    fn parse_pane_facts_reads_live_and_dead_panes() {
        let out = "%0 @0 0 0 s fh-alive\n%1 @4 2 1 s3 fh-dead\n";
        let states = parse_pane_facts(out);
        assert_eq!(
            states.get("%0"),
            Some(&PaneState::for_test("fh-alive", "%0", "@0"))
        );
        assert_eq!(
            states.get("%1"),
            Some(
                &PaneState::for_test("fh-dead", "%1", "@4")
                    .at_index(2)
                    .dead_with(Some(3))
            )
        );
        assert_eq!(states["%1"].window_ordinal, 4);
        assert_eq!(states["%1"].pane_ordinal, 1);
    }

    /// A session name may legally contain SPACES — `rename-session` is
    /// available to anything that inherited `TMUX` — which is why it is
    /// the last field and is taken as the whole remainder of the line. A
    /// parser that split it on whitespace would silently truncate the
    /// name, and `session_status`'s pane-belongs-to-this-session check
    /// compares it verbatim.
    #[test]
    fn parse_pane_facts_takes_a_session_name_with_spaces_whole() {
        let states = parse_pane_facts("%0 @0 0 0 s a session with spaces\n");
        assert_eq!(states["%0"].session_name, "a session with spaces");
    }

    /// A live pane's trailing status field must never be trusted, even if
    /// tmux happens to report a leftover nonzero value there — only a
    /// dead pane's status is a real exit code. Parsing it regardless would
    /// fabricate a death that has not happened.
    #[test]
    fn parse_pane_facts_ignores_status_of_a_live_pane() {
        let states = parse_pane_facts("%0 @0 0 0 s7 fh-live\n");
        assert_eq!(
            states.get("%0"),
            Some(&PaneState::for_test("fh-live", "%0", "@0"))
        );
    }

    /// A dead pane whose status tmux could not express as a plain integer
    /// (a signal death, empirically an empty field on some tmux builds)
    /// must decode as `exit_code: None`, not fail the whole parse — this
    /// is the same honest gap `SessionStatus::Exited` documents. A bare
    /// `s` is what that empty field arrives as.
    ///
    /// The sibling case is the one this format's `s` prefix exists for at
    /// all, and it is covered above: a pane that exited with code ZERO
    /// must read back as `Some(0)`. tmux's `#{?cond,a,b}` conditional
    /// treats the string `"0"` as false, so routing the status through
    /// one turned every clean exit into "no code".
    #[test]
    fn parse_pane_facts_tolerates_an_unparseable_dead_status() {
        let states = parse_pane_facts("%0 @0 0 1 s fh-signalled\n");
        assert_eq!(
            states.get("%0"),
            Some(&PaneState::for_test("fh-signalled", "%0", "@0").dead_with(None))
        );
    }

    /// Every fixed field is validated by SHAPE, and a row failing any of
    /// them is skipped rather than inserted with a guessed value — a
    /// fabricated liveness claim is exactly what `session_status`'s
    /// "missing means Exited" contract exists to avoid. Covers each field
    /// in turn: a truncated row, a pane id that is not `%N`, a window id
    /// that is not `@N`, a non-numeric window index, an unrecognized
    /// `pane_dead`, and an empty session name.
    #[test]
    fn parse_pane_facts_skips_every_malformed_row_shape() {
        let states = parse_pane_facts(
            "%0\n\
             %1 @0\n\
             notapane @0 0 0 s fh\n\
             %2 window0 0 0 s fh\n\
             %3 @0 index 0 s fh\n\
             %4 @0 0 maybe s fh\n\
             %5 @0 0 2 s fh\n",
        );
        assert!(
            states.is_empty(),
            "every row here is malformed and must be skipped: {states:?}"
        );
    }

    /// A pane id appearing TWICE drops the pane entirely rather than
    /// letting either row win.
    ///
    /// tmux emits exactly one row per pane, so a second one can only have
    /// been fabricated — a newline inside the one caller-controlled field
    /// left in this query, the session name. Dropping is the safe
    /// direction: the pane reports through the ordinary absent-pane path
    /// as `Exited { exit_code: None }`, which is a nuisance a same-server
    /// pane can inflict and never a liveness claim it can forge.
    #[test]
    fn parse_pane_facts_drops_a_pane_that_appears_twice() {
        let states =
            parse_pane_facts("%0 @0 0 0 s fh-real\n%0 @9 9 0 s fh-forged\n%1 @1 1 0 s fh-real\n");
        assert!(
            !states.contains_key("%0"),
            "a duplicated pane id must yield NO entry, not the first or last one: {states:?}"
        );
        assert!(
            states.contains_key("%1"),
            "an unaffected pane must survive its neighbour's forgery"
        );
    }

    /// The markers are the fields a pane can WRITE, so the join is where
    /// their syntax is settled: exactly three tokens, a well-formed pane
    /// id, and two values that are each either the empty placeholder or a
    /// COMPLETE minted-shaped id.
    ///
    /// What this establishes is syntax, not provenance — a pane can mark
    /// its own window with a perfectly well-formed uuid, and nothing here
    /// or anywhere else authenticates who wrote a window option. What it
    /// buys is that a marker can never be malformed in a way that shifts
    /// a parse or names something outside tmux's own namespace; safety in
    /// USE comes from every operation addressing a pane paired with its
    /// session (`pane_in_session`).
    #[test]
    fn join_pane_markers_accepts_only_complete_minted_shaped_values() {
        const TAB: &str = "9c3d5a71-0000-4000-8000-0000000000ff";
        const AGENT: &str = "2b1f0e4c-0000-4000-8000-000000000001";
        let mut states: HashMap<String, PaneState> = ["%0", "%1", "%2", "%3", "%4"]
            .into_iter()
            .map(|pane| (pane.to_string(), PaneState::for_test("fh-s", pane, "@0")))
            .collect();
        join_pane_markers(
            &mut states,
            &format!(
                "%0 - -\n\
                 %1 - {TAB}\n\
                 %2 {AGENT} -\n\
                 %3 - {TAB}trailing\n\
                 %4 - ../../etc/passwd\n",
            ),
        );
        assert_eq!(states["%0"].tab, None);
        assert_eq!(states["%0"].agent, None);
        assert_eq!(states["%1"].tab.as_deref(), Some(TAB));
        assert_eq!(states["%2"].agent.as_deref(), Some(AGENT));
        assert_eq!(
            states["%3"].tab, None,
            "a uuid with trailing garbage is not a uuid"
        );
        assert_eq!(states["%4"].tab, None);
    }

    /// The fabrication a newline inside a marker value can attempt, and
    /// the two rules that contain it.
    ///
    /// A hostile value can inject an entire extra marker row. If it names
    /// a pane the AUTHORITATIVE query never reported, the join drops it —
    /// a marker query may only decorate panes, never invent them. If it
    /// names a real pane, it collides with that pane's own row and BOTH
    /// are discarded, leaving the victim merely unmarked. Neither outcome
    /// lets one pane's option value hand another pane a marker.
    #[test]
    fn join_pane_markers_contains_a_row_fabricated_by_a_newline() {
        const TAB: &str = "9c3d5a71-0000-4000-8000-0000000000ff";
        let mut states: HashMap<String, PaneState> = ["%0", "%1"]
            .into_iter()
            .map(|pane| (pane.to_string(), PaneState::for_test("fh-s", pane, "@0")))
            .collect();
        join_pane_markers(
            &mut states,
            &format!(
                // %0's own row, then a forged row for the real pane %1,
                // then a forged row for a pane that does not exist.
                "%0 - -\n\
                 %1 - {TAB}\n\
                 %1 - {TAB}\n\
                 %77 - {TAB}\n",
            ),
        );
        assert_eq!(
            states["%1"].tab, None,
            "a pane whose marker row was duplicated must end up unmarked, not marked by \
             whichever row happened to be last"
        );
        assert!(
            !states.contains_key("%77"),
            "a marker row may decorate a pane, never invent one"
        );
    }

    /// The second query is skipped when it cannot matter — the
    /// optimization that keeps a tab-less deployment paying exactly the
    /// one subprocess it always paid.
    #[test]
    fn the_marker_query_is_skipped_only_when_no_session_has_two_windows() {
        let one_each: HashMap<String, PaneState> = [("%0", "fh-a", "@0"), ("%1", "fh-b", "@1")]
            .into_iter()
            .map(|(pane, session, window)| {
                (pane.to_string(), PaneState::for_test(session, pane, window))
            })
            .collect();
        assert!(!any_session_has_several_windows(&one_each));

        let mut with_a_tab = one_each.clone();
        with_a_tab.insert("%2".to_string(), PaneState::for_test("fh-a", "%2", "@2"));
        assert!(any_session_has_several_windows(&with_a_tab));

        // Two panes in ONE window (a split) is not two windows.
        let mut split: HashMap<String, PaneState> = HashMap::new();
        split.insert("%0".to_string(), PaneState::for_test("fh-a", "%0", "@0"));
        split.insert("%1".to_string(), PaneState::for_test("fh-a", "%1", "@0"));
        assert!(!any_session_has_several_windows(&split));
    }

    /// Parser-level robustness only: an empty string must parse as an
    /// empty map, not panic. This is NOT the genuinely-empty-server case
    /// (`pane_states`'s own `LIST_PANES_EMPTY_SERVER_DIAGNOSTIC` handling)
    /// — a real empty server makes the tmux COMMAND itself fail with `"no
    /// current target"` before this function ever sees any output to
    /// parse, so `parse_pane_facts` in production never actually
    /// receives an empty string from that path. This test exists purely
    /// so the parser itself does not panic or misbehave if it ever did
    /// receive one (a future caller feeding it something else, say).
    #[test]
    fn parse_pane_facts_handles_empty_output() {
        assert!(parse_pane_facts("").is_empty());
    }
}
