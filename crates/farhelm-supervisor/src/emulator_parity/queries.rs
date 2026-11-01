//! Query answers through tmux and the emulator: the startup probes recorded
//! agents send, and every query in the emulator's answer table.

use farhelm_terminal_emulator::{Size, VERSION_REPORT};

use super::harness::{Rig, TmuxServer, pinned_tmux_or_skip};

/// A production pane's size, which the cursor position answers depend on.
const PANE: Size = Size {
    columns: 133,
    lines: 42,
};

/// The queries Claude Code 2.1.316 and Codex 0.166.0 sent when they started
/// in a tmux 3.7c pane with Farhelm's settings, byte for byte and in the
/// order sent, each burst one write. Only the queries of the recordings are
/// copied here: the recordings themselves hold conversation text and paths,
/// and stay out of the repository (`recordings`).
///
/// Claude Code's second burst depends on the first: it asks for mode 2026
/// only after an answer to XTVERSION, which is why the emulator answers that
/// query at all (`farhelm_terminal_emulator`'s `queries` module docs).
const AGENT_PROBES: &[(&str, &str)] = &[
    (
        "Claude Code 2.1.316, first burst",
        "\x1b[>0q\x1b[?u\x1b]7501;?\x07\x1b[c",
    ),
    ("Claude Code 2.1.316, second burst", "\x1b[?2026$p\x1b[c"),
    (
        "Codex 0.166.0",
        "\x1b[?u\x1b[c\x1b[6n\x1b]10;?\x1b\\\x1b]11;?\x1b\\",
    ),
];

/// The probes recorded agents send at startup get the same answers from the
/// emulator as from tmux 3.7c, except that the version report names Farhelm.
///
/// Why: an agent decides what its terminal can do from these answers, and
/// those decisions were made against tmux's until now. Claude Code asks for
/// synchronized output only after a version report, and draws without it
/// when none comes; Codex picks its palette from the background color. Spec:
/// each burst, written at the same cursor position to both, gets byte-equal
/// answers once tmux's own version report (and its DA2, which these bursts do
/// not ask) are replaced by the emulator's; and Claude Code's first burst
/// does get a version report.
#[test]
fn recorded_agent_probes_get_tmux_answers() {
    if !pinned_tmux_or_skip("recorded_agent_probes_get_tmux_answers") {
        return;
    }
    let mut server = TmuxServer::new();
    let mut failures = Vec::new();
    for (name, probe) in AGENT_PROBES {
        let mut rig = Rig::new(&mut server, PANE);
        // Somewhere other than the origin, so cursor reports say something.
        rig.step(b"\x1b[31;3H");
        let cut = rig.step(probe.as_bytes());
        assert!(!cut.in_update, "{name}: the probe's cut is not compared");
        let differences = cut.differences(&rig.host);
        if !differences.is_empty() {
            failures.push(format!("{name}:\n{}", differences.join("\n")));
        }
        if probe.contains("\x1b[>0q") {
            let answers = String::from_utf8_lossy(&cut.emulator_answers).into_owned();
            assert!(
                answers.starts_with(VERSION_REPORT),
                "{name}: no version report first in {answers:?}"
            );
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Every query of the emulator's answer table, as the terminal-emulator
/// crate's own tests and the original tmux probe list name them.
const TABLE_QUERIES: &[&str] = &[
    "\x1b[c",
    "\x1b[0c",
    "\x1bZ",
    "\x1b[>c",
    "\x1b[=c",
    "\x1b[>q",
    "\x1b[>0q",
    "\x1b[>1q",
    "\x1b[5n",
    "\x1b[3;5H\x1b[6n",
    // The cursor at and past the last column, with autowrap on and off.
    "\x1b[2;79Hx\x1b[6n",
    "\x1b[2;80Hx\x1b[6n",
    "\x1b[?7l\x1b[2;80Hx\x1b[6n\x1b[?7h",
    // Autowrap changed after the column was written, either way.
    "\x1b[?7l\x1b[2;80Hx\x1b[?7h\x1b[6n",
    "\x1b[2;80Hx\x1b[?7l\x1b[6n\x1b[?7h",
    // A wide character dropped with autowrap off moves nothing.
    "\x1b[2;80Hx\x1b[?7l\u{4e00}\x1b[?7h\x1b[6n",
    "\x1b[?6n",
    "\x1b[?996n",
    "\x1b[?u",
    "\x1b[?4m",
    "\x1b[11t",
    "\x1b[13t",
    "\x1b[14t",
    "\x1b[14;2t",
    "\x1b[15t",
    "\x1b[16t",
    "\x1b[18t",
    "\x1b[19t",
    "\x1b[21t",
    "\x1b]10;?\x07",
    "\x1b]10;?\x1b\\",
    "\x1b]11;?\x07",
    "\x1b]11;?\x1b\\",
    "\x1b]12;?\x07",
    "\x1b]52;c;?\x07",
    "\x1bP$qm\x1b\\",
    "\x1bP$q q\x1b\\",
    "\x1bP$qr\x1b\\",
    "\x1bP+q524742\x1b\\",
    "\x05",
    "\x1b]4;1;?\x07",
    "\x1b]4;200;?\x1b\\",
    "\x1b[?2026h\x1b[?2026$p\x1b[?2026l",
    "\x1b[?2004h\x1b[?2004$p\x1b[?2004l",
    "\x1b[1 q\x1b[?12$p\x1b[0 q",
    "\x1b[?1003h\x1b[?1003$p\x1b[?1002$p\x1b[?1000$p\x1b[?1003l",
    // The UTF-8 and SGR mouse encodings are separate modes, both reported set
    // whichever was set first (a vendored patch; upstream kept the last).
    "\x1b[?1006h\x1b[?1005h\x1b[?1005$p\x1b[?1006$p\x1b[?1005l\x1b[?1006l",
    "\x1b[?1005h\x1b[?1006h\x1b[?1005$p\x1b[?1006$p\x1b[?1006l\x1b[?1005$p\x1b[?1005l",
    "\x1b[?47h\x1b[?1049$p\x1b[?47$p\x1b[?47l",
    "\x1b[?1047h\x1b[?1049$p\x1b[?1047$p\x1b[?1047l",
];

/// The DECRQM modes the original probe asked, ANSI and private.
const ANSI_MODES: &[u16] = &[4, 2, 12, 20, 34];
const PRIVATE_MODES: &[u16] = &[
    1, 3, 5, 6, 7, 8, 9, 12, 25, 45, 47, 66, 67, 69, 1000, 1001, 1002, 1003, 1004, 1005, 1006,
    1007, 1015, 1016, 1036, 1042, 1047, 1048, 1049, 2004, 2026, 2027, 2031, 2048,
];

/// The queries whose answers differ on purpose, with both answers pinned: tmux
/// 3.7c's (after the harness's DA2 and version substitutions) and the
/// emulator's. Each reason is in the terminal-emulator crate's `queries`
/// module docs; this test only holds both sides to them.
const KNOWN_DIFFERENCES: &[(&str, &str, &str)] = &[
    // vte reports DECID as DA1; tmux ignores it.
    ("\x1bZ", "", "\x1b[?1;2c"),
    // Unparsed by vte; no recorded agent sends them.
    ("\x1b[?996n", "\x1b[?997;1n", ""),
    ("\x1b[15t", "\x1b[5;768;1280t", ""),
    ("\x1b[16t", "\x1b[6;32;16t", ""),
    ("\x1b[19t", "\x1b[9;24;80t", ""),
    ("\x1bP$qm\x1b\\", "\x1bP0$r\x1b\\", ""),
    ("\x1bP$q q\x1b\\", "\x1bP1$r q0 q\x1b\\", ""),
    ("\x1bP$qr\x1b\\", "\x1bP0$r\x1b\\", ""),
    // tmux leaves these color queries to xterm.js today.
    ("\x1b]12;?\x07", "", "\x1b]12;rgb:ffff/ffff/ffff\x07"),
    ("\x1b]4;1;?\x07", "", "\x1b]4;1;rgb:cccc/6666/6666\x07"),
    (
        "\x1b]4;200;?\x1b\\",
        "",
        "\x1b]4;200;rgb:ffff/0000/d7d7\x1b\\",
    ),
    // Mode reports where tmux's answer would be untrue of the emulator.
    ("\x1b[20$p", "\x1b[20;0$y", "\x1b[20;2$y"),
    ("\x1b[?2031$p", "\x1b[?2031;2$y", "\x1b[?2031;0$y"),
];

/// Every query of the emulator's answer table gets tmux 3.7c's answer from
/// the emulator, or the different answer the table records for it.
///
/// Why: the table was written from a one-off probe of tmux; replaying it
/// against the pinned tmux on every run keeps the table and tmux from
/// drifting apart unnoticed, and a known difference that tmux or the
/// emulator stops having shows up as a failure too. Spec: for each query, fed
/// in a cut of its own to one 80x24 pane and emulator (after a cut that homes
/// the cursor; the stateful queries restore what they change), tmux's answer
/// (after the DA2 and version substitutions) equals the emulator's, unless
/// `KNOWN_DIFFERENCES` lists the query, in which case both answers equal the
/// listed ones.
#[test]
fn the_answer_table_matches_tmux() {
    if !pinned_tmux_or_skip("the_answer_table_matches_tmux") {
        return;
    }
    let mut queries: Vec<String> = TABLE_QUERIES.iter().map(|q| q.to_string()).collect();
    queries.extend(ANSI_MODES.iter().map(|m| format!("\x1b[{m}$p")));
    queries.extend(PRIVATE_MODES.iter().map(|m| format!("\x1b[?{m}$p")));
    let mut server = TmuxServer::new();
    let mut rig = Rig::new(
        &mut server,
        Size {
            columns: 80,
            lines: 24,
        },
    );
    let mut failures = Vec::new();
    for query in &queries {
        // Each query starts from the cursor at the origin and default modes;
        // the stateful ones above restore what they change.
        rig.step(b"\x1b[H");
        let cut = rig.step(query.as_bytes());
        assert!(!cut.in_update, "{query:?}: the cut is not compared");
        let tmux = String::from_utf8_lossy(&cut.tmux_answers).into_owned();
        let ours = String::from_utf8_lossy(&cut.emulator_answers).into_owned();
        let known = KNOWN_DIFFERENCES.iter().find(|(q, _, _)| q == query);
        let ok = match known {
            Some((_, theirs, mine)) => tmux == *theirs && ours == *mine,
            None => tmux == ours,
        };
        if !ok {
            failures.push(format!(
                "{query:?}: tmux {tmux:?}, emulator {ours:?}{}",
                known.map_or(String::new(), |(_, theirs, mine)| format!(
                    " (listed as tmux {theirs:?}, emulator {mine:?})"
                ))
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
