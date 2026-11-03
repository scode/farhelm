//! Recorded agent sessions replayed through tmux and the emulator, with the
//! status readers run on both at every cut.
//!
//! A recording is a directory holding `raw.bin`, everything an agent wrote to
//! its pane (as tmux's `pipe-pane` saw it, so after the terminal's output
//! processing), and `events.jsonl`, one JSON object per line with the pane
//! size the agent had from a byte offset on (`{"offset": 0, "cols": 133,
//! "rows": 42}`). The prototype for the terminal host recorded Claude Code
//! and Codex sessions this way.
//!
//! Recordings are not in the repository. They hold the conversation, the
//! working directory and whatever else the agent printed, and the repository
//! is public; scrubbing them well enough to publish would also change the
//! very text being compared. So this test is `#[ignore]`d and reads the
//! recordings named in `FARHELM_TEST_AGENT_RECORDINGS`, as
//! `<harness>=<directory>` entries separated like `PATH` (for example
//! `claude=/tmp/rec-claude:codex=/tmp/rec-codex`), where `<harness>` is
//! `claude`, `codex` or `muse`. `.agents/validation.md` gives the command.
//! The queries those sessions sent are in the repository (`queries`), since
//! they carry nothing personal.

use std::path::{Path, PathBuf};
use std::time::Instant;

use farhelm_proto::AgentKind;
use farhelm_terminal_emulator::{Collected, Emulator, FeedMode, Size};

use super::harness::{Cut, Rig, TmuxServer, agent_kind, pinned_tmux_or_skip, read};

/// Where the recordings to replay are named (see the module docs).
const RECORDINGS_VARIABLE: &str = "FARHELM_TEST_AGENT_RECORDINGS";

/// About how many bytes apart cuts are. A cut goes at the last point before
/// this where the emulator is clean, which for agents that draw every frame
/// inside a synchronized update means between frames: inside one, the two
/// terminals are not comparable (see the harness).
const CUT_EVERY: usize = 256;

/// One recording: who made it, its bytes, and its size changes in order.
struct Recording {
    kind: AgentKind,
    path: PathBuf,
    bytes: Vec<u8>,
    /// `(offset, size)`, starting at offset 0.
    sizes: Vec<(usize, Size)>,
}

impl Recording {
    fn load(harness: &str, path: &Path) -> Recording {
        let kind = agent_kind(harness);
        let bytes = std::fs::read(path.join("raw.bin"))
            .unwrap_or_else(|e| panic!("reading {}/raw.bin: {e}", path.display()));
        let events = std::fs::read_to_string(path.join("events.jsonl"))
            .unwrap_or_else(|e| panic!("reading {}/events.jsonl: {e}", path.display()));
        let sizes: Vec<(usize, Size)> = events
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let event: serde_json::Value = serde_json::from_str(line).expect("event JSON");
                let field = |name: &str| {
                    event[name]
                        .as_u64()
                        .unwrap_or_else(|| panic!("event without {name}: {line}"))
                };
                let size = Size {
                    columns: u16::try_from(field("cols")).expect("columns"),
                    lines: u16::try_from(field("rows")).expect("rows"),
                };
                (usize::try_from(field("offset")).expect("offset"), size)
            })
            .collect();
        assert_eq!(
            sizes.first().map(|(offset, _)| *offset),
            Some(0),
            "{}: the first event must give the size at offset 0",
            path.display()
        );
        // A resize is applied where its offset falls between two bytes (or
        // after the last one), and the replay walks them in order: offsets
        // must rise, and none may lie past the recording's end.
        for pair in sizes.windows(2) {
            assert!(
                pair[0].0 < pair[1].0 && pair[1].0 <= bytes.len(),
                "{}: size events must have rising offsets within the recording's {} bytes, \
                 got {} then {}",
                path.display(),
                bytes.len(),
                pair[0].0,
                pair[1].0
            );
        }
        Recording {
            kind,
            path: path.to_path_buf(),
            bytes,
            sizes,
        }
    }
}

/// The recordings named in the environment; `None`, after saying so, when it
/// names none.
fn recordings() -> Option<Vec<Recording>> {
    let Some(value) = std::env::var_os(RECORDINGS_VARIABLE) else {
        eprintln!("SKIPPED replaying recorded agent sessions: {RECORDINGS_VARIABLE} is not set");
        return None;
    };
    let loaded: Vec<Recording> = std::env::split_paths(&value)
        .map(|entry| {
            let entry = entry.to_str().expect("UTF-8 entry").to_owned();
            let (harness, path) = entry.split_once('=').unwrap_or_else(|| {
                panic!("{RECORDINGS_VARIABLE}: {entry:?} is not <harness>=<directory>")
            });
            Recording::load(harness, Path::new(path))
        })
        .collect();
    assert!(
        !loaded.is_empty(),
        "{RECORDINGS_VARIABLE} names no recording"
    );
    Some(loaded)
}

/// What one recording's replay found.
#[derive(Default)]
struct Tally {
    cuts: usize,
    /// Cuts with text, cursor, title or answers differing, described.
    differences: Vec<String>,
    /// Cuts where a reader concluded differently, described.
    readings: Vec<String>,
}

/// Compare a cut and record what differs. Every cut here is outside an
/// update (the replay cuts only at clean offsets, or flushes), which this
/// checks rather than skips.
fn check(recording: &Recording, rig: &Rig<'_>, cut: &Cut, tally: &mut Tally) {
    tally.cuts += 1;
    assert!(
        !cut.in_update,
        "{}: the cut at byte {} is inside an update",
        recording.path.display(),
        cut.offset
    );
    let differences = cut.differences(&rig.host);
    if !differences.is_empty() {
        tally.differences.push(differences.join("\n"));
    }
    let tmux = read(recording.kind, &cut.tmux_tail(), &cut.tmux_title);
    let emulator = read(recording.kind, &cut.emulator_tail, &cut.emulator_title);
    if tmux != emulator {
        tally.readings.push(format!(
            "at byte {}: tmux reads {tmux:?} (state, anchored), emulator {emulator:?}\n  tmux:     {:?} / {:?}\n  emulator: {:?} / {:?}",
            cut.offset, cut.tmux_title, cut.tmux_text, cut.emulator_title, cut.emulator_text,
        ));
    }
}

/// What feeding a recording to a scout emulator one byte at a time shows.
struct Scout {
    /// `clean[offset]`: whether an emulator fed the first `offset` bytes is
    /// clean (`Emulator::is_clean`).
    clean: Vec<bool>,
    /// The offsets at which a synchronized update ended and released answers
    /// to queries sent inside it: tmux answered those when they arrived, the
    /// emulator only now (the emulator's `queries` module docs). An update
    /// ends at its end marker, at a resize (which flushes it), or at the end
    /// of the recording, where the replay flushes it too.
    late_answers: Vec<usize>,
    /// The scout's size after the whole recording, which shows that every
    /// size event, including one after the last byte, was applied.
    final_size: Size,
}

/// Feed `recording` to a scout emulator one byte at a time, resizing it where
/// the recording does, at one instant as in the rig. The emulator in the rig
/// is fed the same bytes and resizes, so it is clean at the same offsets;
/// only there may a cut go (see the harness).
fn scout(recording: &Recording) -> Scout {
    let mut scout = Emulator::new(recording.sizes[0].1, 1000);
    let mut sizes = recording.sizes[1..].iter().peekable();
    let mut clean = vec![true];
    let mut late_answers = Vec::new();
    let now = Instant::now();
    // Whether ending an update released answers: an update that was open
    // and is no longer, with replies in what ending it produced.
    let released = |open_before: bool, scout: &Emulator, effects: &Collected| {
        open_before && scout.sync_deadline().is_none() && !effects.replies.is_empty()
    };
    for (offset, byte) in recording.bytes.iter().enumerate() {
        if let Some((_, size)) = sizes.next_if(|(at, _)| *at == offset) {
            let open_before = scout.sync_deadline().is_some();
            let mut effects = Collected::default();
            scout.resize(*size, FeedMode::Live, &mut effects);
            if released(open_before, &scout, &effects) {
                late_answers.push(offset);
            }
        }
        let open_before = scout.sync_deadline().is_some();
        let mut effects = Collected::default();
        scout.feed(&[*byte], now, FeedMode::Live, &mut effects);
        if released(open_before, &scout, &effects) {
            late_answers.push(offset + 1);
        }
        clean.push(scout.is_clean());
    }
    // A resize after the last byte, then the end of the recording.
    let end = recording.bytes.len();
    if let Some((_, size)) = sizes.next_if(|(at, _)| *at == end) {
        let open_before = scout.sync_deadline().is_some();
        let mut effects = Collected::default();
        scout.resize(*size, FeedMode::Live, &mut effects);
        if released(open_before, &scout, &effects) {
            late_answers.push(end);
        }
    }
    let open_before = scout.sync_deadline().is_some();
    let mut effects = Collected::default();
    scout.flush_sync(FeedMode::Live, &mut effects);
    if released(open_before, &scout, &effects) {
        late_answers.push(end);
    }
    Scout {
        clean,
        late_answers,
        final_size: scout.size(),
    }
}

/// Whether `bytes` begin with a REP (`CSI digits b`), before which no cut may
/// go (see the harness's module docs).
fn starts_with_rep(bytes: &[u8]) -> bool {
    let Some(rest) = bytes.strip_prefix(b"\x1b[") else {
        return false;
    };
    let digits = rest.iter().take_while(|byte| byte.is_ascii_digit()).count();
    rest.get(digits) == Some(&b'b')
}

/// Replay one recording, cutting at the last usable offset at most
/// [`CUT_EVERY`] bytes on (or the first one after, when a sequence or update
/// runs longer), and at each size change, which is applied to both terminals
/// at its offset and must fall on a clean offset. A usable offset is clean
/// and not right before a REP. If the recording ends inside an update with
/// the parser between sequences, the last cut feeds the rest and flushes the
/// update first, which tmux has applied as it went; a recording that ends
/// inside a sequence fails, naming what could not be compared.
fn replay(server: &mut TmuxServer, recording: &Recording) -> Tally {
    let scout = scout(recording);
    let bytes = &recording.bytes;
    let usable = |offset: usize| scout.clean[offset] && !starts_with_rep(&bytes[offset..]);
    assert!(
        scout.late_answers.is_empty(),
        "{}: answers to queries sent inside an update, released at bytes {:?}",
        recording.path.display(),
        scout.late_answers
    );
    let mut tally = Tally::default();
    let mut rig = Rig::new(server, recording.sizes[0].1);
    let mut position = 0;
    let mut sizes = recording.sizes[1..].iter().peekable();
    let end = bytes.len();
    while position < end {
        let resize_at = sizes.peek().map(|(offset, _)| *offset);
        let segment_end = resize_at.unwrap_or(end);
        let target = (position + CUT_EVERY).min(segment_end);
        let cut_at = (position + 1..=target)
            .rev()
            .find(|&offset| usable(offset))
            .or_else(|| (target + 1..=segment_end).find(|&offset| usable(offset)));
        let Some(cut_at) = cut_at else {
            assert!(
                resize_at.is_none(),
                "{}: no clean offset between byte {position} and the size change at {segment_end}",
                recording.path.display()
            );
            // The end of the recording, inside an update or a sequence.
            let cut = rig.step_and_flush(&bytes[position..]);
            check(recording, &rig, &cut, &mut tally);
            break;
        };
        let cut = rig.step(&bytes[position..cut_at]);
        check(recording, &rig, &cut, &mut tally);
        position = cut_at;
        if Some(position) == resize_at {
            let (_, size) = sizes.next().expect("peeked");
            rig.resize(*size);
            let cut = rig.step(b"");
            check(recording, &rig, &cut, &mut tally);
        }
    }
    tally
}

/// Recorded agent sessions show the same screen text, cursor, title and
/// query answers through the emulator as through tmux 3.7c, and the agent's
/// status reader concludes the same from both, at every cut, through the
/// whole recording.
///
/// Why: the frozen screens are tmux's text without the bytes behind it, so
/// only a real session's output shows whether the emulator turns what agents
/// actually write (synchronized redraws, scroll regions, cursor-addressed
/// prompts, resizes) into the text the readers expect. Spec: replaying each
/// recording, every cut (all outside updates, the last one after a flush if
/// the recording ends inside one) has equal text, cursor, title (with the
/// documented title differences) and answers, and equal readings; no update
/// releases an answer to a query sent inside it, whether the update ends at
/// its end marker, a resize or the end of the recording
/// (`the_scout_reports_answers_held_by_an_update`); the test fails listing each
/// cut that does not, and reports how many cuts it made.
#[test]
#[ignore = "needs recordings that stay outside the repository; see the module docs"]
fn recorded_sessions_read_the_same_from_the_emulator() {
    if !pinned_tmux_or_skip("recorded_sessions_read_the_same_from_the_emulator") {
        return;
    }
    let Some(recordings) = recordings() else {
        return;
    };
    let mut server = TmuxServer::new();
    let mut failures = Vec::new();
    for recording in &recordings {
        let tally = replay(&mut server, recording);
        eprintln!(
            "{}: {} cuts over all {} bytes, {} differing, {} read differently",
            recording.path.display(),
            tally.cuts,
            recording.bytes.len(),
            tally.differences.len(),
            tally.readings.len()
        );
        assert!(tally.cuts > 0, "{}: no cuts", recording.path.display());
        for difference in tally.differences.iter().chain(&tally.readings) {
            failures.push(format!("{}: {difference}", recording.path.display()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// A recording of `bytes` at 20x6, resized at the given offsets.
#[cfg(test)]
fn synthetic(bytes: &[u8], resizes: &[(usize, Size)]) -> Recording {
    let mut sizes = vec![(
        0,
        Size {
            columns: 20,
            lines: 6,
        },
    )];
    sizes.extend_from_slice(resizes);
    Recording {
        kind: AgentKind::Claude,
        path: PathBuf::from("synthetic"),
        bytes: bytes.to_vec(),
        sizes,
    }
}

/// The scout reports an answer to a query inside a synchronized update
/// whichever way the update ends: its end marker, a resize, or the end of
/// the recording; and reports nothing for queries outside updates.
///
/// Why: tmux answers a query at once, the emulator only when the update
/// ends, and the replay compares answers only after updates, where the two
/// agree; this scout check is the only thing that notices an agent
/// depending on such an answer, so each way an update can end must reach
/// it. Spec: `late_answers` holds the offset where each update that held an
/// answer ended (after the end marker's last byte, before the resize's
/// offset, or the recording's length), and is empty for a query outside an
/// update, before one, and inside one that holds no query.
#[test]
fn the_scout_reports_answers_held_by_an_update() {
    let end_marker = b"\x1b[?2026h\x1b[6n\x1b[?2026l";
    assert_eq!(
        scout(&synthetic(end_marker, &[])).late_answers,
        [end_marker.len()]
    );
    let open_at_the_end = b"before\x1b[?2026h\x1b[6n";
    assert_eq!(
        scout(&synthetic(open_at_the_end, &[])).late_answers,
        [open_at_the_end.len()]
    );
    let resized = b"\x1b[?2026h\x1b[6nxyz";
    let wider = Size {
        columns: 30,
        lines: 6,
    };
    assert_eq!(
        scout(&synthetic(resized, &[(12, wider)])).late_answers,
        [12]
    );
    for clean in [
        &b"\x1b[6n"[..],
        b"\x1b[6n\x1b[?2026hx\x1b[?2026l",
        b"\x1b[?2026hx\x1b[?2026l\x1b[6n",
        b"\x1b[?2026hx",
    ] {
        assert_eq!(
            scout(&synthetic(clean, &[])).late_answers,
            Vec::<usize>::new(),
            "{:?}",
            String::from_utf8_lossy(clean)
        );
    }
}

/// A recording whose last resize comes after its last byte loads, and the
/// scout applies that resize at the end; size events at the same offset, or
/// past the end, are refused.
///
/// Why: a session recorded until the pane was resized and then closed ends
/// in exactly that shape, and the replay handles it (it cuts at the end,
/// resizes and compares once more), so the loader must not reject it; a
/// resize that falls past the bytes, or two at one offset, can only be a
/// broken recording. Spec: `events.jsonl` with offsets 0 (20x6) and the byte
/// count (30x8) loads; the scout ends at 30x8, which only applying that last
/// resize gives; offsets 0, 3, 3 and 0, count + 1 panic naming the rule.
#[test]
fn a_resize_after_the_last_byte_is_accepted() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let write = |events: &str, bytes: &[u8]| {
        std::fs::write(dir.path().join("raw.bin"), bytes).expect("raw.bin");
        std::fs::write(dir.path().join("events.jsonl"), events).expect("events.jsonl");
    };
    let held = b"\x1b[?2026h\x1b[6n";
    let event = |offset: usize| format!(r#"{{"offset": {offset}, "cols": 20, "rows": 6}}"#);
    let last = format!(r#"{{"offset": {}, "cols": 30, "rows": 8}}"#, held.len());
    write(&format!("{}\n{last}\n", event(0)), held);
    let recording = Recording::load("claude", dir.path());
    assert_eq!(recording.sizes.len(), 2);
    let scouted = scout(&recording);
    assert_eq!(
        scouted.final_size,
        Size {
            columns: 30,
            lines: 8
        }
    );
    assert_eq!(scouted.late_answers, [held.len()]);
    for offsets in [[0, 3, 3].as_slice(), &[0, held.len() + 1]] {
        let events: String = offsets.iter().map(|offset| event(*offset) + "\n").collect();
        write(&events, held);
        let refused = std::panic::catch_unwind(|| Recording::load("claude", dir.path()).sizes);
        let message = refused.expect_err("loaded a recording with bad size events");
        let message = message
            .downcast_ref::<String>()
            .map(String::as_str)
            .unwrap_or_default();
        assert!(message.contains("rising offsets"), "{offsets:?}: {message}");
    }
}
