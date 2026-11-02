//! The frozen agent screens, repainted through tmux and the emulator.
//!
//! The captured screens under `tests/fixtures/screens/` are tmux's text, not
//! the bytes the agents wrote: the capture tool stores `capture-pane -p` and
//! `#{pane_title}`, never the output stream. So the status readers cannot be
//! run on the emulator's version of the same session. What can be checked is
//! the next best thing: paint each captured screen into both terminals, row
//! by row at an absolute position, and compare. That holds the emulator to
//! tmux on exactly the characters agents draw (box drawing, block elements,
//! spinners, emoji, wide CJK), including how wide each one is, which a cursor
//! position report after every row reveals; and it runs each reader on the
//! emulator's text to show the frozen expectations survive the move. The
//! recorded sessions (`recordings`) cover the escape sequences themselves.
//!
//! A title of `host` is the capture tool's stand-in for the host name tmux
//! reports before a program sets a title; the repaint sets no title for it,
//! so the emulator's empty title is what reaches the reader, as it would for
//! a host terminal.

use farhelm_terminal_emulator::Size;

use super::harness::{Rig, TmuxServer, agent_kind, pinned_tmux_or_skip, read};
use crate::agent_kind::screen_fixtures::load_fixtures;
use crate::agent_kind::screen_reader::SAMPLE_TAIL_BYTES;
use crate::tmux::retain_pane_tail;

/// The pane width the capture tool uses (`scripts/capture-agent-screens.py`,
/// `PANE_COLUMNS`); a screen's height is its number of rows.
const CAPTURE_COLUMNS: u16 = 133;

/// The capture tool's replacement for the host name (see the module docs).
const SCRUBBED_HOST: &str = "host";

/// The bytes that paint `rows` and set `title`, with a cursor position
/// report after each row.
fn repaint(rows: &[&str], title: &str) -> Vec<u8> {
    let mut out = String::new();
    if title != SCRUBBED_HOST && !title.is_empty() {
        out.push_str(&format!("\x1b]2;{title}\x07"));
    }
    for (index, row) in rows.iter().enumerate() {
        out.push_str(&format!("\x1b[{};1H{row}\x1b[6n", index + 1));
    }
    out.into_bytes()
}

/// Every frozen agent screen, painted into tmux 3.7c and the emulator, reads
/// back identically from both, with identical cursor positions after every
/// row, and the emulator's text and title read as the state the screen was
/// captured in.
///
/// Why: status detection's readers are held to these screens, which are
/// frozen (re-captured only when the maintainer asks), and they must keep
/// their meaning when the text comes from the emulator. The screens exist
/// only as captured text, so this is the comparison they allow (module
/// docs). Spec: for each fixture, tmux's capture of the repaint equals the
/// fixture (the premise that the repaint reproduces the screen); the
/// emulator's text, cursor, title and cursor reports equal tmux's; and the
/// fixture's reader, given the emulator's own screen tail
/// (`Emulator::screen_tail`) and title, reaches the same state and anchoring
/// as on the fixture's text trimmed as the sampler trims tmux's, and title.
#[test]
fn frozen_agent_screens_read_the_same_from_the_emulator() {
    if !pinned_tmux_or_skip("frozen_agent_screens_read_the_same_from_the_emulator") {
        return;
    }
    let mut server = TmuxServer::new();
    let mut failures = Vec::new();
    let fixtures = load_fixtures();
    assert!(!fixtures.is_empty(), "no frozen screens found");
    for fixture in &fixtures {
        let name = format!(
            "{} {} {}-{}",
            fixture.harness, fixture.version, fixture.expected, fixture.scenario
        );
        let rows: Vec<&str> = fixture.screen.lines().collect();
        let size = Size {
            columns: CAPTURE_COLUMNS,
            lines: u16::try_from(rows.len()).expect("rows"),
        };
        let mut rig = Rig::new(&mut server, size);
        let cut = rig.step(&repaint(&rows, &fixture.title));
        assert!(!cut.in_update, "{name}: the repaint's cut is not compared");
        if cut.tmux_text != fixture.screen {
            failures.push(format!(
                "{name}: the repaint does not reproduce the screen in tmux:\n  tmux:    {:?}\n  fixture: {:?}",
                cut.tmux_text, fixture.screen
            ));
            continue;
        }
        let differences = cut.differences(&rig.host);
        if !differences.is_empty() {
            failures.push(format!("{name}:\n{}", differences.join("\n")));
            continue;
        }
        let kind = agent_kind(&fixture.harness);
        let tmux = read(
            kind,
            &retain_pane_tail(&fixture.screen, SAMPLE_TAIL_BYTES),
            &fixture.title,
        );
        // The emulator's side as a host terminal's sampler will read it.
        let emulator = read(kind, &cut.emulator_tail, &cut.emulator_title);
        if tmux != emulator {
            failures.push(format!(
                "{name}: the reader concludes {tmux:?} (state, anchored) from tmux's screen, \
                 {emulator:?} from the emulator's"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} screens differ:\n\n{}",
        failures.len(),
        fixtures.len(),
        failures.join("\n\n")
    );
}
