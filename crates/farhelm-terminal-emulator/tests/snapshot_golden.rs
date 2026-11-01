//! Golden fixtures of snapshot format version 1: blobs made from scripted
//! terminal states, checked in under `tests/fixtures/snapshots/v1/` as hex
//! text (`<name>.hex`, sixteen bytes a line, like the terminal host's protocol
//! fixtures) beside a description of the state each restores to
//! (`<name>.state.txt`, from `common::describe`).
//!
//! A terminal host keeps a blob for as long as it runs, across supervisor
//! updates, so every later build must restore every blob an earlier build
//! wrote, to the same state (`snapshot` module docs, "Compatibility"). These
//! tests hold the format to that:
//!
//! - every checked-in blob restores to its checked-in description, including
//!   blobs whose script has since been removed from this file;
//! - each script, run now, still encodes to its checked-in blob byte for
//!   byte, so an accidental change of format fails here first;
//! - each script's live emulator matches the description too, so a
//!   description cannot drift from what the script means.
//!
//! The fixtures are never rewritten. `write_missing_golden_fixtures` (an
//! ignored test, run by hand) only creates the files of a new script. A
//! deliberate change of format is a new version with its own directory.
//!
//! Scripts give every hyperlink an explicit `id`: alacritty numbers links
//! without one from a counter shared by the whole test process, which would
//! make the blobs depend on which tests ran first.

mod common;

use std::path::{Path, PathBuf};

use common::describe::describe;
use common::t0;
use farhelm_terminal_emulator::alacritty_terminal::grid::Dimensions;
use farhelm_terminal_emulator::alacritty_terminal::index::{Column, Line};
use farhelm_terminal_emulator::alacritty_terminal::term::cell::Flags;
use farhelm_terminal_emulator::{Collected, Emulator, FeedMode, Size, Snapshot};

/// Where version 1's fixtures live.
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/snapshots/v1")
}

/// Where in a version 1 blob the width table (Unicode version and
/// fingerprint) sits: after the magic (4), version (2), flags (1), columns
/// and lines (2 + 2), history (4) and trimmed count (4).
const WIDTH_TABLE: std::ops::Range<usize> = 19..30;

/// One step of a script.
enum Step {
    Feed(&'static str),
    /// Bytes that are not UTF-8 on their own, fed as one read.
    FeedBytes(&'static [u8]),
    /// Output too long to write out, made by a function.
    FeedMade(fn() -> String),
    Resize(u16, u16),
}

/// Which snapshot a script ends with.
enum Take {
    /// `Emulator::snapshot` with the host's limit.
    Full,
    /// `Emulator::snapshot` with a small limit, to trim.
    Limited(usize),
    /// `Emulator::history_seed`.
    Seed,
}

/// A scripted terminal state.
struct Script {
    name: &'static str,
    columns: u16,
    lines: u16,
    steps: &'static [Step],
    take: Take,
}

use Step::{Feed, FeedBytes, FeedMade, Resize};

/// An orphaned wide character (its spacer erased alone, so it is a fixup)
/// carrying version 1's most of everything a fixup cell carries: 16
/// combining characters and a hyperlink with a 250-byte `id` and a 2083-byte
/// URI.
fn fixup_at_the_limits() -> String {
    format!(
        "\x1b]8;id={};http://e/{}\x1b\\中{}\x1b]8;;\x1b\\\x1b[D\x1b[X",
        "i".repeat(250),
        "u".repeat(2083 - 9),
        "\u{301}".repeat(16)
    )
}

/// The scripts, together covering the format's features: both screens,
/// history and wrapped lines, wide and combining characters, hyperlinks,
/// attributes and colors, palette changes, modes, scroll regions (clamped
/// and empty ones too), tab stops, character sets, title and title stack,
/// saved cursors on both screens, a cursor template with a link, fixups,
/// pending wraps over each kind of last cell, the REP character (printed,
/// zero-width, and as a fixup), the trimmed and dropped flags, and the seed.
const SCRIPTS: &[Script] = &[
    Script {
        name: "history",
        columns: 12,
        lines: 4,
        steps: &[
            Feed("line 1\r\nline 2\r\nline 3\r\nline 4\r\nline 5\r\nline 6\r\n"),
            Feed("this line wraps across two rows\r\n"),
            Feed("last x\x1b[2b"),
        ],
        take: Take::Full,
    },
    Script {
        name: "styles",
        columns: 16,
        lines: 7,
        steps: &[
            Feed(
                "\x1b[1;1H\x1b[1mbold\x1b[2mdim\x1b[0;3mital\x1b[0;7minv\x1b[0;8mhid\x1b[0;9mst\x1b[0m",
            ),
            Feed(
                "\x1b[2;1H\x1b[4ma\x1b[4:2mb\x1b[4:3mc\x1b[4:4md\x1b[4:5me\x1b[58;5;9mf\x1b[58;2;1;2;3mg\x1b[0m",
            ),
            Feed(
                "\x1b[3;1H\x1b[31mr\x1b[91mR\x1b[38;5;200mi\x1b[38;2;1;2;3mt\x1b[42mg\x1b[102mG\x1b[48;5;17mb\x1b[48;2;9;8;7mB\x1b[0m",
            ),
            // Wide characters, an emoji, combining characters on one cell
            // and on a wide one, and a tab cell.
            Feed("\x1b[4;1H中😀e\u{301}\u{308}中\u{301}\ta"),
            Feed("\x1b[5;1H\x1b]8;id=doc;https://example.test/doc\x1b\\link\x1b]8;;\x1b\\ plain"),
            // A wide character that does not fit at the end of a row: a
            // leading spacer there, the character on the next row.
            Feed("\x1b[6;16H中"),
        ],
        take: Take::Full,
    },
    Script {
        name: "alternate-screen",
        columns: 10,
        lines: 4,
        steps: &[
            Feed("one\r\ntwo\r\nthree\r\nfour\r\nfive\r\n\x1b[2;3H\x1b7\x1b[4;1Hcursor"),
            Feed("\x1b[?1049h\x1b[44mfull\x1b[2;2Hscreen\x1b7\x1b[0m\x1b[3;1Hframe"),
            Resize(8, 5),
            Feed("\x1b[5;1Hafter"),
        ],
        take: Take::Full,
    },
    Script {
        name: "alternate-saved-cursor",
        columns: 10,
        lines: 3,
        steps: &[Feed(
            "\x1b[?1049h\x1b[2;3H\x1b[1m\x1b7\x1b[?1049lback on primary",
        )],
        take: Take::Full,
    },
    Script {
        name: "modes-and-state",
        columns: 20,
        lines: 5,
        steps: &[
            Feed("\x1b[?2004h\x1b[?1002h\x1b[?1006h\x1b[?1004h\x1b[?1h\x1b=\x1b[?25l\x1b[5 q"),
            Feed("\x1b[3g\x1b[1;5H\x1bH\x1b[1;13H\x1bH"),
            Feed("\x1b)0\x0elqk\x0f\x1b(0"),
            Feed(
                "\x1b]2;first\x07\x1b[22t\x1b]2;second;with;semicolons\x07\x1b[22t\x1b]2;third\x07",
            ),
            Feed(
                "\x1b]4;1;#102030\x07\x1b]4;200;rgb:ff/00/80\x07\x1b]10;#aabbcc\x07\x1b]11;#010203\x07\x1b]12;#ff00ff\x07",
            ),
            Feed("\x1b[2;4r\x1b[?6h\x1b[2;3Hin region\x1b[4h\x1b[20h\x1b[?7l"),
        ],
        take: Take::Full,
    },
    Script {
        name: "fixups",
        columns: 10,
        lines: 4,
        steps: &[
            // A wide character's spacer erased alone, then a wide character
            // shifted off a row by an insert.
            Feed("ab中cd\x1b[3D\x1b[X\r\n中中中中中\x1b[1G\x1b[@"),
            // A wrap pending at the margin, then the terminal narrowed.
            Feed("\x1b[3;1H0123456789"),
            Resize(7, 4),
            // The cursor restored outside the scroll region under origin
            // mode.
            Feed("\x1b[4;5H\x1b7\x1b[1;2r\x1b[?6h\x1b8"),
        ],
        take: Take::Full,
    },
    Script {
        name: "pending-wrap",
        columns: 10,
        lines: 3,
        steps: &[Feed("first\x1b[2;1H0123456789")],
        take: Take::Full,
    },
    Script {
        name: "bottom-row-wrap",
        columns: 10,
        lines: 4,
        // A wrapped line moved down onto the bottom row, which the stream
        // can only paint with its scroll region trick.
        steps: &[Feed(
            "top\x1b[4;1Habcdefghijkl\x1b[1;1H\x1bM\x1b[2;1Hmiddle",
        )],
        take: Take::Full,
    },
    Script {
        name: "untitled-title-stack",
        columns: 12,
        lines: 2,
        // Entries pushed before any title, then titled ones.
        steps: &[Feed(
            "\x1b[22t\x1b[22t\x1b]2;first\x07\x1b[22t\x1b]2;second\x07text",
        )],
        take: Take::Full,
    },
    Script {
        name: "one-column",
        columns: 4,
        lines: 3,
        steps: &[
            Feed("ab中cd"),
            Resize(1, 4),
            Feed("xy中z\x1b[1;1H\x1b[44mq"),
        ],
        take: Take::Full,
    },
    Script {
        name: "trimmed-history",
        columns: 20,
        lines: 3,
        steps: &[Feed(concat!(
            "\x1b[31mred line one\r\n\x1b[32mgreen line two\r\n\x1b[33myellow line three\r\n",
            "\x1b[34mblue line four\r\n\x1b[35mmagenta line five\r\n\x1b[36mcyan line six\r\n",
            "\x1b[37mwhite line seven\r\n\x1b[0mscreen",
        ))],
        take: Take::Limited(200),
    },
    Script {
        name: "screens-dropped",
        columns: 10,
        lines: 3,
        steps: &[Feed(concat!(
            "\x1b[?2004h\x1b]2;kept title\x07",
            "\x1b[31m0\x1b[32m1\x1b[33m2\x1b[34m3\x1b[35m4\x1b[36m5\x1b[37m6\x1b[90m7\x1b[91m8\x1b[92m9",
            "\x1b[93ma\x1b[94mb\x1b[95mc\x1b[96md\x1b[97me\x1b[0m",
        ))],
        take: Take::Limited(140),
    },
    Script {
        name: "scroll-region-clamped",
        columns: 6,
        lines: 4,
        // A bottom past the screen, which alacritty clamps.
        steps: &[Feed("\x1b[3;99rtext")],
        take: Take::Full,
    },
    Script {
        name: "scroll-region-empty",
        columns: 6,
        lines: 4,
        // A top below the screen: an empty region at its bottom edge.
        steps: &[Feed("abc\x1b[5;99r")],
        take: Take::Full,
    },
    Script {
        name: "rep-character-fixup",
        columns: 4,
        lines: 2,
        // Every cell taken, so the REP character (a combining mark on the
        // first cell) has no blank cell to be printed into.
        steps: &[Feed("xxxxxxxx\x1b[H\u{301}")],
        take: Take::Full,
    },
    Script {
        name: "zero-width-rep-character",
        columns: 8,
        lines: 3,
        // A combining mark printed last, after the row the stream paints
        // last: the stream prints it into a blank cell and erases it.
        steps: &[Feed("\x1b[2;1Hz\x1b[1;1He\u{301}\x1b[3;1H")],
        take: Take::Full,
    },
    Script {
        name: "pending-wrap-autowrap-on-then-off",
        columns: 10,
        lines: 3,
        // The wrap flag raised with autowrap on, which is then turned off:
        // still a wrap pending, as tmux has it.
        steps: &[Feed("first\x1b[2;1H0123456789\x1b[?7l")],
        take: Take::Full,
    },
    Script {
        name: "pending-wrap-on-a-wide-character",
        columns: 6,
        lines: 3,
        steps: &[Feed("abcd中")],
        take: Take::Full,
    },
    Script {
        name: "pending-wrap-on-a-blank",
        columns: 6,
        lines: 3,
        // A blank printed into the last column: nothing to paint there, so
        // the stream reprints a blank and fixes the cell up.
        steps: &[Feed("abc\x1b[1;6H ")],
        take: Take::Full,
    },
    Script {
        name: "two-column-pending-wrap",
        columns: 2,
        lines: 4,
        // A pending wrap on a wide character's spacer row, below a row
        // ending in a leading spacer.
        steps: &[Feed("a中中\x1bM")],
        take: Take::Full,
    },
    Script {
        name: "template-link",
        columns: 10,
        lines: 2,
        steps: &[Feed("text\x1b]8;id=t;http://t/\x1b\\")],
        take: Take::Full,
    },
    Script {
        name: "del-rep-character",
        columns: 8,
        lines: 2,
        // DEL printed last: vte makes it the REP character, alacritty shows
        // nothing for it, and the stream cannot print it (its reader refuses
        // the byte), so it is the REP fixup.
        steps: &[Feed("a\x7f")],
        take: Take::Full,
    },
    Script {
        name: "c1-rep-character",
        columns: 10,
        lines: 1,
        // U+0085 (NEL) whose encoding arrives split across two reads: vte
        // prints it rather than executing it, so it becomes the REP
        // character, which printing it whole in the stream would not do.
        steps: &[FeedBytes(b"x\xc2"), FeedBytes(b"\x85")],
        take: Take::Full,
    },
    Script {
        name: "fixup-at-the-limits",
        columns: 20,
        lines: 3,
        steps: &[FeedMade(fixup_at_the_limits)],
        take: Take::Full,
    },
    Script {
        name: "restart-seed",
        columns: 12,
        lines: 3,
        steps: &[
            Feed("\x1b[1mold\x1b[0m history\r\nand a wrapped long line\r\nmore\r\nscreen"),
            Feed("\x1b[?2004h\x1b]2;old title\x07\x1b[?1049hfull screen"),
        ],
        take: Take::Seed,
    },
];

/// Whether `needle` occurs in `haystack`.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Run a script and take its snapshot. Feeds are live, so the scripts must
/// not depend on effects (they ask nothing).
fn run(script: &Script) -> (Emulator, Snapshot) {
    let mut emulator = Emulator::new(
        Size {
            columns: script.columns,
            lines: script.lines,
        },
        1000,
    );
    for step in script.steps {
        match *step {
            Feed(text) => {
                emulator.feed(
                    text.as_bytes(),
                    t0(),
                    FeedMode::Live,
                    &mut Collected::default(),
                );
            }
            FeedBytes(bytes) => {
                emulator.feed(bytes, t0(), FeedMode::Live, &mut Collected::default());
            }
            FeedMade(make) => {
                emulator.feed(
                    make().as_bytes(),
                    t0(),
                    FeedMode::Live,
                    &mut Collected::default(),
                );
            }
            Resize(columns, lines) => {
                emulator.resize(
                    Size { columns, lines },
                    FeedMode::Live,
                    &mut Collected::default(),
                );
            }
        }
    }
    let snapshot = match script.take {
        Take::Full => emulator.snapshot(16 * 1024 * 1024),
        Take::Limited(max) => emulator.snapshot(max),
        Take::Seed => Ok(emulator.history_seed(16 * 1024 * 1024)),
    }
    .expect("scripts end at a clean boundary");
    (emulator, snapshot)
}

/// The description of a restored blob: the restore's own flags, then the
/// emulator's state.
fn restored_description(blob: &[u8]) -> String {
    let restored = Emulator::restore(blob, 1000).expect("a golden blob restores");
    format!(
        "screens dropped {}\n{}",
        restored.screens_dropped,
        describe(&restored.emulator)
    )
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}; for a new script, run the ignored write_missing_golden_fixtures",
            path.display()
        )
    })
}

/// A blob as hex text, sixteen bytes a line.
fn hex(bytes: &[u8]) -> String {
    let mut text = String::new();
    for chunk in bytes.chunks(16) {
        let line: Vec<String> = chunk.iter().map(|byte| format!("{byte:02x}")).collect();
        text.push_str(&line.join(" "));
        text.push('\n');
    }
    text
}

/// The blob in a hex fixture.
fn read_blob(path: &Path) -> Vec<u8> {
    let text = String::from_utf8(read(path)).expect("hex fixtures are text");
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).expect("hex digits"))
        .collect()
}

/// Every version 1 blob in the fixture directory restores to the state
/// described beside it.
///
/// Why: this is "formats are forever": a blob a host may still hold must
/// restore the same way in every later build. It reads the directory rather
/// than the scripts, so a blob keeps being checked after its script is gone.
/// Spec: `snapshot` module docs, "Compatibility".
#[test]
fn golden_blobs_restore_to_their_recorded_state() {
    let mut checked = 0;
    for entry in std::fs::read_dir(fixtures()).expect("the fixture directory") {
        let path = entry.expect("a fixture entry").path();
        if path.extension().is_some_and(|extension| extension == "hex") {
            let expected = String::from_utf8(read(&path.with_extension("state.txt"))).unwrap();
            let got = restored_description(&read_blob(&path));
            assert!(got == expected, "{}:\n{got}", path.display());
            checked += 1;
        }
    }
    assert!(checked >= SCRIPTS.len(), "only {checked} golden blobs");
}

/// Every script still encodes to its golden blob, byte for byte apart from
/// the width table's version, and its live emulator, where the snapshot is
/// whole, matches the recorded description.
///
/// Why: a change to the encoder that changes its output is a change of
/// format, which version 1's readers in the field cannot follow; it must
/// fail here rather than ship. The width table's version is data, not
/// format: an upgrade of `unicode-width` changes it legitimately. Spec:
/// `v1` module docs.
#[test]
fn scripts_still_encode_to_their_golden_blobs() {
    for script in SCRIPTS {
        let path = fixtures().join(format!("{}.hex", script.name));
        let mut golden = read_blob(&path);
        let (emulator, snapshot) = run(script);
        golden[WIDTH_TABLE].copy_from_slice(&snapshot.blob[WIDTH_TABLE]);
        assert!(
            snapshot.blob == golden,
            "{}: the encoder's output changed:\n  now:    {}\n  golden: {}",
            script.name,
            snapshot.blob.escape_ascii(),
            golden.escape_ascii()
        );
        if let Take::Full = script.take {
            let expected = String::from_utf8(read(&path.with_extension("state.txt"))).unwrap();
            let live = format!("screens dropped false\n{}", describe(&emulator));
            assert!(
                live == expected,
                "{}: the live state is\n{live}",
                script.name
            );
        }
    }
}

/// Each script reaches the feature its name promises (the trimming flags,
/// the seed's shape, pending wraps, the bottom-row wrap trick, untitled
/// title stack entries, one and two columns, combining characters, emoji and
/// tab cells, scroll regions, the REP character, a template's link), so
/// that its fixture pins that feature.
///
/// Why: a fixture only guards what it contains; a review found the `styles`
/// script overwriting its own wide and combining characters, so its fixture
/// covered neither. This keeps any script from silently drifting away from
/// its feature.
#[test]
fn scripts_exercise_their_feature() {
    for script in SCRIPTS {
        let (_, snapshot) = run(script);
        let restored = Emulator::restore(&snapshot.blob, 1000).unwrap();
        match script.name {
            "trimmed-history" => {
                assert!(snapshot.dropped_history_lines > 0);
                assert!(!snapshot.dropped_screens);
                assert!(restored.emulator.history_trimmed());
            }
            "screens-dropped" => assert!(restored.screens_dropped),
            "restart-seed" => {
                assert!(restored.emulator.term().grid().history_size() > 0);
                assert_eq!(restored.emulator.screen_text().trim(), "");
            }
            "pending-wrap" | "pending-wrap-autowrap-on-then-off" => {
                assert!(restored.emulator.term().grid().cursor.input_needs_wrap);
                // Erasing to the right and the next print go by it, live and
                // restored alike.
                let (mut live, _) = run(script);
                let mut restored = restored.emulator;
                for emulator in [&mut live, &mut restored] {
                    emulator.feed(b"\x1b[Kx", t0(), FeedMode::Live, &mut Collected::default());
                }
                assert_eq!(restored.screen_text(), live.screen_text());
            }
            "bottom-row-wrap" => {
                let grid = restored.emulator.term().grid();
                assert!(grid[Line(3)][Column(9)].flags.contains(Flags::WRAPLINE));
                // The stream sets its scroll region around the bottom row.
                assert!(contains(&snapshot.blob, b"\x1b[1;3r"));
            }
            "untitled-title-stack" => {
                let stack = restored.emulator.term().title_stack();
                assert_eq!(stack.first(), Some(&None));
                assert_eq!(stack.len(), 3);
            }
            "one-column" => assert_eq!(restored.emulator.size().columns, 1),
            "scroll-region-clamped" => {
                let region = restored.emulator.term().scroll_region();
                assert_eq!((region.start.0, region.end.0), (2, 4));
                assert!(contains(&snapshot.blob, b"\x1b[3;5r"));
            }
            "scroll-region-empty" => {
                let region = restored.emulator.term().scroll_region();
                assert_eq!((region.start.0, region.end.0), (4, 4));
                assert!(contains(&snapshot.blob, b"\x1b[5;6r"));
            }
            "rep-character-fixup" => {
                assert_eq!(restored.emulator.preceding_char(), Some('\u{301}'));
                // Tag 2 and the character, the blob's last field.
                assert!(snapshot.blob.ends_with(&[2, 0, 0, 3, 1]));
            }
            "zero-width-rep-character" => {
                assert_eq!(restored.emulator.preceding_char(), Some('\u{301}'));
                assert_eq!(snapshot.blob.last(), Some(&0));
            }
            "pending-wrap-on-a-wide-character" => {
                let grid = restored.emulator.term().grid();
                assert!(grid.cursor.input_needs_wrap);
                assert!(grid[Line(0)][Column(4)].flags.contains(Flags::WIDE_CHAR));
            }
            "pending-wrap-on-a-blank" => {
                let grid = restored.emulator.term().grid();
                assert!(grid.cursor.input_needs_wrap);
                assert_eq!(grid[Line(0)][Column(5)].c, ' ');
            }
            "two-column-pending-wrap" => {
                let grid = restored.emulator.term().grid();
                assert_eq!(restored.emulator.size().columns, 2);
                assert!(grid.cursor.input_needs_wrap);
                assert!(
                    grid[Line(0)][Column(1)]
                        .flags
                        .contains(Flags::LEADING_WIDE_CHAR_SPACER)
                );
            }
            "del-rep-character" | "c1-rep-character" => {
                let rep = if script.name == "del-rep-character" {
                    '\u{7f}'
                } else {
                    '\u{85}'
                };
                assert_eq!(restored.emulator.preceding_char(), Some(rep));
                // Tag 2 and the character, the blob's last field.
                assert!(snapshot.blob.ends_with(&[2, 0, 0, 0, rep as u8]));
                // REP after the restore repeats what it repeats live.
                let (mut live, _) = run(script);
                let mut restored = restored.emulator;
                for emulator in [&mut live, &mut restored] {
                    emulator.feed(b"\x1b[3b", t0(), FeedMode::Live, &mut Collected::default());
                }
                assert_eq!(restored.screen_text(), live.screen_text());
            }
            "fixup-at-the-limits" => {
                let cell = &restored.emulator.term().grid()[Line(0)][Column(0)];
                assert!(cell.flags.contains(Flags::WIDE_CHAR));
                assert_eq!(cell.zerowidth().map(<[char]>::len), Some(16));
                let link = cell.hyperlink().expect("the link");
                assert_eq!((link.id().len(), link.uri().len()), (250, 2083));
                // Carried by a fixup: the stream never painted the link.
                assert!(!contains(&snapshot.blob, b"\x1b]8;id=iii"));
                assert!(contains(&snapshot.blob, link.uri().as_bytes()));
            }
            "template-link" => {
                let template = &restored.emulator.term().grid().cursor.template;
                let link = template.hyperlink().expect("the template's link");
                assert_eq!((link.id(), link.uri()), ("t", "http://t/"));
            }
            "styles" => {
                let grid = restored.emulator.term().grid();
                assert_eq!(
                    grid[Line(3)][Column(4)].zerowidth(),
                    Some(&['\u{301}', '\u{308}'][..])
                );
                assert_eq!(grid[Line(3)][Column(7)].c, '\t');
                assert_eq!(grid[Line(3)][Column(2)].c, '😀');
            }
            _ => assert_eq!(snapshot.dropped_history_lines, 0, "{}", script.name),
        }
    }
}

/// Write the blob and description of every script that has none yet. Never
/// overwrites: an existing fixture is the format's history. Run by hand
/// after adding a script:
/// `cargo test -p farhelm-terminal-emulator --test snapshot_golden -- --ignored write_missing`.
#[test]
#[ignore = "writes fixtures; run by hand for a new script"]
fn write_missing_golden_fixtures() {
    std::fs::create_dir_all(fixtures()).unwrap();
    for script in SCRIPTS {
        let path = fixtures().join(format!("{}.hex", script.name));
        if path.exists() {
            continue;
        }
        let (_, snapshot) = run(script);
        std::fs::write(&path, hex(&snapshot.blob)).unwrap();
        std::fs::write(
            path.with_extension("state.txt"),
            restored_description(&snapshot.blob),
        )
        .unwrap();
        println!("wrote {}", path.display());
    }
}
