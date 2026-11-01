//! Generated output through tmux and the emulator, compared after every
//! piece: hand-picked screens for each kind of sequence agents draw with,
//! seeded random streams that mix them, and the known differences pinned on
//! both sides.
//!
//! Nothing here may pass by not comparing: every update a stream opens is
//! closed, every scenario and stream ends on a compared cut, and the cuts
//! skipped inside updates are counted and bounded (`harness`, "What is
//! compared, and what is not").

use farhelm_terminal_emulator::Size;

use super::harness::{Rig, TmuxServer, pinned_tmux_or_skip};

/// A small deterministic generator (xorshift64*), so that a failing stream
/// is reproduced by its seed alone and the test needs no randomness crate.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        // Any nonzero state works; mixing keeps nearby seeds unrelated.
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..n`.
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// A number in `1..=max`.
    fn upto(&mut self, max: u64) -> u64 {
        self.below(max) + 1
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
}

/// One piece of a scenario or stream: output, or a resize of both
/// terminals. A cut follows each.
#[derive(Clone)]
enum Piece {
    Output(Vec<u8>),
    Resize(Size),
}

/// Readable in failures: output as (lossy) text, a resize as its size.
impl std::fmt::Debug for Piece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Piece::Output(bytes) => write!(f, "{:?}", String::from_utf8_lossy(bytes)),
            Piece::Resize(size) => write!(f, "resize to {}x{}", size.columns, size.lines),
        }
    }
}

/// Output as a piece.
fn out(text: &str) -> Piece {
    Piece::Output(text.as_bytes().to_vec())
}

/// A terminal size.
const fn size(columns: u16, lines: u16) -> Size {
    Size { columns, lines }
}

/// Text that agents and shells print, beyond ASCII: what the status readers
/// match on (box drawing, bullets, spinners, the prompt marks), wide CJK and
/// emoji, and combining marks. Not an emoji made wide by a variation
/// selector, which is a known difference (`KNOWN_DIFFERENCES`).
const GLYPHS: &[&str] = &[
    "─",
    "│",
    "╭",
    "╮",
    "╰",
    "╯",
    "⏺",
    "⎿",
    "✻",
    "✶",
    "·",
    "❯",
    "›",
    "•",
    "…",
    "⚠",
    "▐",
    "▛",
    "█",
    "▜",
    "▝",
    "⠋",
    "⠙",
    "中",
    "文",
    "😀",
    "🚀",
    "✅",
    "e\u{301}",
    "n\u{303}\u{301}",
];

/// The sizes generated streams start at and resize to.
const SIZES: &[Size] = &[size(20, 6), size(13, 5), size(40, 10), size(27, 8)];

/// What a generated stream has open, so that it closes what it opens and
/// sends the sequences whose results differ only in the pinned forms.
struct Stream {
    size: Size,
    /// On the alternate screen; leaving it otherwise restores a cursor that
    /// was never saved (pinned).
    alternate: bool,
    /// Pieces left before an open synchronized update is closed.
    update_left: Option<u64>,
    /// A cursor saved with DECSC since the last alternate screen switch: the
    /// emulator shares that save with mode 1049, tmux does not (pinned), so
    /// DECRC is sent only after a save on the same side of a switch.
    saved: bool,
    /// Origin mode is on: setting a scroll region then homes the cursor to
    /// different places (pinned), so regions are set only with it off.
    origin: bool,
    /// A resize to send after the cursor-homing piece before it: with the
    /// cursor beyond the new width or height, tmux and the emulator place it
    /// differently (pinned).
    resize: Option<Size>,
}

impl Stream {
    /// The next piece.
    ///
    /// The mix holds every kind of sequence the recorded Claude Code and Codex
    /// sessions sent (counted per kind from the raw recordings: CUP, CHA, CUU,
    /// CUD, CUF, CUB, EL, ED, SGR, DECSTBM, DECSC and DECRC, SCS, DECSCUSR, the
    /// private modes 1049, 2026, 25, 2004, 1004, 1000, 1002, 1003, 1006, 1007
    /// and 2031, kitty keyboard pushes and pops, modifyOtherKeys, queries,
    /// titles), and more; the arms below name each. It leaves out exactly the
    /// forms whose results are pinned as known differences
    /// (`KNOWN_DIFFERENCES`): the DEC line-drawing character set, a DECSC with
    /// a wrap pending or restored across an origin mode change, REP other than
    /// right after a printed character in the same piece, insert and delete
    /// line outside the scroll region, tabs (read back from the cells as they
    /// are now), CHT, DECRC without a DECSC on the same side of an alternate
    /// screen switch, a scroll region set in origin mode, insert mode (which
    /// differs across a wrap), autowrap off, a resize of anything but an empty
    /// primary screen with the cursor at the top left, an emoji made wide by
    /// VS16, and anything that acts on a pending wrap other than printing
    /// (moving the cursor, a backspace, erasing characters). Each of those is
    /// sent here only in a form that avoids the pinned case, or not at all. A
    /// wide character that does not fit at the margin over text is in the mix;
    /// the harness allows for it.
    fn piece(&mut self, rng: &mut Rng) -> Piece {
        if let Some(size) = self.resize.take() {
            self.size = size;
            return Piece::Resize(size);
        }
        if let Some(left) = self.update_left {
            if left == 0 {
                self.update_left = None;
                return out("\x1b[?2026l");
            }
            self.update_left = Some(left - 1);
        }
        let lines = u64::from(self.size.lines);
        let text = match rng.below(31) {
            0..=4 => {
                let words = [
                    "ok",
                    "Working",
                    "esc to interrupt",
                    "a",
                    "trailing   ",
                    "  lead",
                    "x",
                ];
                rng.pick(&words).to_string()
            }
            5..=7 => {
                let count = rng.upto(4);
                (0..count).map(|_| *rng.pick(GLYPHS)).collect()
            }
            8 => rng.pick(&["\r\n", "\r", "\n", "\x07"]).to_string(),
            9 => format!("\x1b[{};{}H", rng.upto(12), rng.upto(45)),
            // Movement after an absolute column, which ends a pending wrap
            // in both terminals (acting on one is pinned).
            10 => format!(
                "\x1b[{}G\x1b[{}{}",
                rng.upto(45),
                rng.upto(5),
                rng.pick(&["A", "B", "C", "D", "E", "F", "d", "Z"])
            ),
            11 => format!("\x1b[{}G\x08", rng.upto(45)),
            12 => format!("\x1b[{}J", rng.below(4)),
            13 => format!("\x1b[{}K", rng.below(3)),
            14 => format!(
                "\x1b[{};{}H\x1b[{}{}",
                rng.upto(12),
                rng.upto(45),
                rng.upto(4),
                rng.pick(&["X", "P", "@", "S", "T"])
            ),
            15 | 16 if self.origin => {
                self.origin = false;
                self.saved = false;
                "\x1b[?6l".to_string()
            }
            15 => {
                let top = rng.upto(4);
                format!("\x1b[{};{}r", top, top + rng.upto(6))
            }
            // Insert and delete line inside the region they set.
            16 => {
                let top = rng.upto(lines - 1);
                let bottom = top + rng.upto(lines - top);
                let row = top + rng.below(bottom - top + 1);
                format!(
                    "\x1b[{top};{bottom}r\x1b[{row};{}H\x1b[{}{}",
                    rng.upto(45),
                    rng.upto(3),
                    rng.pick(&["L", "M"])
                )
            }
            17 => rng.pick(&["\x1b[r", "\x1bM", "\x1bD", "\x1bE"]).to_string(),
            18 => rng
                .pick(&[
                    "\x1b[0m",
                    "\x1b[1;31m",
                    "\x1b[7m",
                    "\x1b[38;5;208m",
                    "\x1b[48;2;40;44;52m",
                    "\x1b[2;3;4m",
                    "\x1b[41m",
                ])
                .to_string(),
            19 => {
                self.alternate = !self.alternate;
                self.saved = false;
                if self.alternate {
                    "\x1b[?1049h"
                } else {
                    "\x1b[?1049l"
                }
                .to_string()
            }
            20 => rng
                .pick(&[
                    "\x1b]0;\u{280b} Working\x07",
                    "\x1b]2;[ ! ] Action Required\x1b\\",
                    "\x1b]2;✳ Claude Code\x07",
                    "\x1b]2;\x07",
                ])
                .to_string(),
            21 if self.update_left.is_none() => {
                self.update_left = Some(rng.upto(4));
                "\x1b[?2026h".to_string()
            }
            21 => "\x1b[?2026h".to_string(),
            // REP right after the character it repeats, from a row's start,
            // so that it neither follows another sequence nor passes the
            // margin (both pinned).
            22 => format!(
                "\x1b[{};1H{}\x1b[{}b",
                rng.upto(lines),
                rng.pick(&["-", "─", "x"]),
                rng.upto(u64::from(self.size.columns) - 2)
            ),
            23 => {
                // The cursor at the right margin, where wraps are pending.
                let col = rng.pick(&[19u64, 20, 21, 39, 40, 41]);
                // Narrow glyphs only: a wide one that does not fit over text
                // is allowed for by the harness, and covered by the glyphs.
                let glyph = rng.pick(&["x", "─", "╮", "│", "⏺", "…"]);
                format!("\x1b[{};{}H{glyph}", rng.upto(6), col)
            }
            24 if self.saved => {
                self.saved = false;
                "\x1b8".to_string()
            }
            // Saved after an absolute column: a cursor saved with a wrap
            // pending is restored differently (pinned).
            24 => {
                self.saved = true;
                format!("\x1b[{}G\x1b7", rng.upto(45))
            }
            // Origin mode, which changes where text goes.
            25 => {
                let mode = *rng.pick(&["\x1b[?6h", "\x1b[?6l"]);
                self.origin = mode == "\x1b[?6h";
                // DECRC restores origin mode in tmux only (pinned).
                self.saved = false;
                format!("\x1b[{}G{mode}", rng.upto(45))
            }
            // Resize, with the next piece, an empty primary screen without
            // history and the cursor at its top left: how the two reflow
            // content, history and the cursor differs (pinned), and the
            // hand-picked screens cover reflows that agree.
            26 => {
                self.resize = Some(*rng.pick(SIZES));
                self.origin = false;
                self.saved = false;
                let leave = if std::mem::take(&mut self.alternate) {
                    "\x1b[?1049l"
                } else {
                    ""
                };
                format!("{leave}\x1b[?6l\x1b[2J\x1b[3J\x1b[H")
            }
            // Sequences the recorded agents send that change no text:
            // the ASCII character set (line drawing is pinned), cursor styles,
            // kitty keyboard pushes and pops, modifyOtherKeys, and the
            // private modes for the cursor, input and mouse.
            27 => rng
                .pick(&[
                    "\x1b[?25l",
                    "\x1b[?25h",
                    "\x1b[?2004h",
                    "\x1b[?1004h",
                    "\x1b[?1000h\x1b[?1006h",
                    "\x1b[?1002h\x1b[?1003h",
                    "\x1b[?1007l",
                    "\x1b[?2031h",
                    "\x1b(B",
                    "\x1b[1 q",
                    "\x1b[5 q",
                    "\x1b[0 q",
                    "\x1b[>1u",
                    "\x1b[>5u",
                    "\x1b[<u",
                    "\x1b[>4;2m",
                    "\x1b[>4;0m",
                ])
                .to_string(),
            _ => rng
                .pick(&[
                    "\x1b[6n",
                    "\x1b[c",
                    "\x1b[5n",
                    "\x1b[?2026$p",
                    "\x1b[?2004$p",
                    "\x1b]11;?\x1b\\",
                    "\x1b]10;?\x07",
                    "\x1b[18t",
                ])
                .to_string(),
        };
        Piece::Output(text.into_bytes())
    }

    /// Pieces that end the stream on a compared cut: close an open update,
    /// then one plain piece after it.
    fn finish(&mut self) -> Vec<Piece> {
        let mut pieces = Vec::new();
        if let Some(size) = self.resize.take() {
            pieces.push(Piece::Resize(size));
        }
        if self.update_left.take().is_some() {
            pieces.push(out("\x1b[?2026l"));
        }
        pieces.push(out("\r\nend"));
        pieces
    }
}

/// What feeding a list of pieces showed: the first divergence, described
/// with how to reproduce it, and which cuts were inside an update.
struct Comparison {
    failure: Option<String>,
    in_update: Vec<bool>,
}

/// Feed `pieces` to a fresh pane and emulator of `start`, cutting after each.
fn compare(server: &mut TmuxServer, name: &str, start: Size, pieces: &[Piece]) -> Comparison {
    let mut rig = Rig::new(server, start);
    let mut in_update = Vec::new();
    for (index, piece) in pieces.iter().enumerate() {
        let cut = match piece {
            Piece::Output(bytes) => rig.step(bytes),
            Piece::Resize(size) => {
                rig.resize(*size);
                rig.step(b"")
            }
        };
        in_update.push(cut.in_update);
        let differences = cut.differences(&rig.host);
        if !differences.is_empty() {
            return Comparison {
                failure: Some(format!(
                    "{name} (from {}x{}), after piece {index} {piece:?}:\n{}\n  pieces so far: {:?}",
                    start.columns,
                    start.lines,
                    differences.join("\n"),
                    &pieces[..=index],
                )),
                in_update,
            };
        }
    }
    Comparison {
        failure: None,
        in_update,
    }
}

/// Hand-picked screens: each names one behavior status detection depends
/// on, as a list of pieces with a cut after each.
fn scenarios() -> Vec<(&'static str, Size, Vec<Piece>)> {
    let small = size(20, 6);
    vec![
        (
            "wide and combining characters",
            small,
            vec![
                out("中文x\r\n"),
                out("😀y\r\n"),
                out("e\u{301}x n\u{303}\u{301}\r\n"),
                out("z ✅ 🚀"),
            ],
        ),
        (
            "wraps, including a wide character at the margin",
            small,
            vec![
                out("abcdefghijklmnopqrstuvwxyz0123"),
                out("\r\n"),
                out("abcdefghijklmnopqrs中z"),
                out("\x1b[3;20Hx"),
                out("y"),
            ],
        ),
        (
            "trailing blanks, written, colored and erased",
            small,
            vec![
                out("abc   \r\n"),
                out("def\x1b[41m   \x1b[0m\r\n"),
                out("   \r\n"),
                out("ghi\x1b[1K"),
                out("\x1b[2;2H\x1b[K"),
            ],
        ),
        (
            "colors leave text alone",
            small,
            vec![
                out("\x1b[1;31mred\x1b[0m plain \x1b[38;2;1;2;3mrgb\x1b[48;5;9mbg"),
                out("\x1b[0m\r\n\x1b[7m   \x1b[0m"),
            ],
        ),
        (
            "the alternate screen and back",
            small,
            vec![
                out("primary\r\n"),
                out("\x1b[?1049h"),
                out("\x1b[Halt one\r\nalt two"),
                out("\x1b[?1049l"),
                out("back"),
            ],
        ),
        (
            "scroll regions",
            small,
            vec![
                out("1\r\n2\r\n3\r\n4\r\n5\r\n6"),
                out("\x1b[2;4r"),
                out("\x1b[4;1Hnew\n\nnewer"),
                out("\x1b[2;1H\x1bM\x1bM"),
                out("\x1b[3S"),
                out("\x1b[2T"),
                // Relative moves stop at the margins from inside the region.
                out("\x1b[3;2H\x1b[9Ax\x1b[9By\x1b[9Fz"),
                // Insert and delete line inside the region.
                out("\x1b[3;1H\x1b[1L\x1b[2;1H\x1b[2M"),
                // Origin mode: positions are relative to the region.
                out("\x1b[?6h\x1b[1;1Ho\x1b[9;1Hp\x1b[2;1H\x1b[3Gq\x1b[?6l"),
                out("\x1b[r\x1b[6;1H\n\nend"),
            ],
        ),
        (
            "erase operations",
            small,
            vec![
                out("abcdef\r\nghijkl\r\nmnopqr"),
                out("\x1b[1;3H\x1b[2X"),
                out("\x1b[2;3H\x1b[1K"),
                out("\x1b[2;5H\x1b[0K"),
                out("\x1b[3;2H\x1b[2P"),
                out("\x1b[3;2H\x1b[2@"),
                out("\x1b[2;1H\x1b[1J"),
                out("\x1b[2J"),
                // With a wrap pending, erasing below keeps the last column.
                out("\x1b[1;20Hz\x1b[J\x1b[6n"),
            ],
        ),
        (
            "autowrap off at the last column: reports and erases",
            small,
            vec![
                out("abc\x1b[?7l\x1b[1;20Hx\x1b[6n"),
                out("\x1b[1;20Hy\x1b[K"),
                out("\x1b[2;18Hzzz\x1b[J"),
                out("\x1b[3;19Hpq\x1b[6n"),
                // Written with autowrap off, then autowrap on: the cursor
                // stays on the last column, so the query and the erases go
                // by that column.
                out("\x1b[4;20Hu\x1b[?7h\x1b[6n"),
                out("\x1b[K"),
                out("\x1b[?7l\x1b[5;20Hv\x1b[?7h\x1b[J"),
                // Written with autowrap on, then autowrap off: a wrap stays
                // pending, so the cursor is past the last column.
                out("\x1b[4;20Hw\x1b[?7l\x1b[6n"),
                out("\x1b[K"),
                out("\x1b[?7h\x1b[5;20Hz\x1b[?7l\x1b[J"),
                // A wide character dropped with autowrap off moves nothing:
                // the wrap stays pending.
                out("\x1b[?7h\x1b[4;20Hw\x1b[?7l\u{4e00}\x1b[6n"),
                out("\x1b[K"),
                out("\x1b[?7h\x1b[5;20Hz\x1b[?7l\u{4e00}\x1b[J"),
                // Written with autowrap off, then autowrap on: the next
                // character overwrites the last column, and a tab there moves
                // nothing.
                out("\x1b[?7l\x1b[3;20Hs\x1b[?7h"),
                out("t"),
                out("\x1b[?7l\x1b[2;20Hm\x1b[?7h\t"),
                out("n"),
                // A combining mark joins the cell before the cursor, which
                // stays on the last column, as in tmux.
                out("\x1b[?7l\x1b[2;20He\u{301}\u{2500}\x1b[?7h"),
                // A wide character that does not fit at the last column
                // with autowrap off is dropped, and nothing is pending: the
                // next character goes in the last column.
                out("\x1b[?7l\x1b[3;19Hk\u{4e00}\x1b[?7hl"),
                // The corner-cell idiom: no scroll.
                out("\x1b[?7l\x1b[6;20Hq\x1b[?7h"),
                out("r"),
                out("\x1b[?7h\r\nend"),
            ],
        ),
        (
            "autowrap off at the last column, then a resize",
            small,
            vec![
                out("\x1b[?7labcdefghijklmnopqrst\x1b[?7h"),
                Piece::Resize(size(26, 6)),
                out("x"),
                out("\r\nend"),
            ],
        ),
        (
            "autowrap off at the last column of the alternate screen, then a wider screen",
            small,
            vec![
                // Nothing is pending, so the cursor stays on the column the
                // resize keeps, and a combining mark joins the character
                // written there.
                out("\x1b[?1049h\x1b[?7l\x1b[1;17Habcd\x1b[?7h"),
                Piece::Resize(size(26, 6)),
                out("e\u{301}"),
                out("\r\nend"),
            ],
        ),
        (
            "lines inserted and deleted",
            small,
            vec![
                out("a\r\nb\r\nc\r\nd"),
                out("\x1b[2;1H\x1b[2L"),
                out("\x1b[1;1H\x1b[1M"),
                out("\x1b[5;1Hz"),
            ],
        ),
        (
            "insert mode and saved cursors",
            small,
            vec![
                out("abcdef\r\nghijkl"),
                out("\x1b[1;3H\x1b[4hXY\x1b[4l"),
                out("\x1b[2;2H\x1b7\x1b[5;5Hz\x1b8w"),
            ],
        ),
        (
            "tabs",
            small,
            vec![
                out("a\tb\r\n12345678\t9"),
                out("\r\n\tc"),
                out("\r\nx\ty"),
                out("\x1b[4;20H\t"),
                out("\x1b[5;18H\t\tz"),
            ],
        ),
        (
            "titles",
            small,
            vec![
                out("\x1b]0;\u{280b} Working\x07"),
                out("\x1b]2;[ ! ] Action Required\x1b\\"),
                out("\x1b]2;  spaced  \x07"),
                out("\x1b]2;\x07"),
            ],
        ),
        (
            SYNC_SCENARIO,
            small,
            vec![
                out("before"),
                out("\x1b[?2026h\x1b[2J\x1b[Hhalf"),
                out(" drawn"),
                out("\x1b[?2026l"),
                out("after"),
            ],
        ),
        (
            "a prompt box at a production pane's width",
            size(133, 42),
            vec![
                out("\x1b[40;1H╭"),
                out("─\x1b[131b╮\r\n"),
                out("│ > \x1b[133G│\r\n"),
                out("╰─\x1b[130b╯"),
                out("\x1b[39;1H✻ Thinking… (esc to interrupt)"),
            ],
        ),
        (
            "narrowing and widening wrapped text",
            small,
            vec![
                out("abcdefghijklmnopqrstuvwxyz0123\r\nshort"),
                Piece::Resize(size(12, 6)),
                Piece::Resize(size(25, 6)),
                Piece::Resize(size(20, 6)),
                out("!"),
            ],
        ),
        (
            "the cursor on a wrapped line through resizes",
            small,
            vec![
                out("abcdefghijklmnopqrstuvwxy"),
                Piece::Resize(size(10, 6)),
                out("Z"),
                Piece::Resize(size(30, 6)),
                out("Q"),
            ],
        ),
        (
            "fewer and more lines",
            small,
            vec![
                out("1\r\n2\r\n3\r\n4\r\n5\r\n6"),
                Piece::Resize(size(20, 3)),
                Piece::Resize(size(20, 6)),
                out("x"),
            ],
        ),
        (
            "resizing while the alternate screen shows",
            small,
            vec![
                out("primary line one\r\nprimary two"),
                out("\x1b[?1049h\x1b[Halt screen\x1b[H"),
                Piece::Resize(size(10, 4)),
                Piece::Resize(size(20, 6)),
                out("\x1b[?1049l"),
                out("!"),
            ],
        ),
    ]
}

/// The scenario whose cuts inside an update are checked by position.
const SYNC_SCENARIO: &str = "a synchronized update is compared once it ends";

/// Each hand-picked screen captures, places the cursor, titles and answers
/// as tmux 3.7c's, at every cut, including after resizes.
///
/// Why: agent status detection reads screen text and titles that, for
/// terminals in a terminal host, come from the emulator instead of tmux. The
/// readers were written against tmux's text, so the emulator must produce the
/// same text for the sequences agents use to draw: wide and combining
/// characters, wraps, trailing blanks, colors, the alternate screen, scroll
/// regions, erase operations, and the reflow a viewer's resize causes. Spec:
/// every cut's `capture-pane -p` equals `Emulator::screen_text`, the cursors
/// agree, `#{pane_title}` equals `Emulator::title` (with the two documented
/// title differences), and both answer the cut's queries alike; every
/// scenario ends on a compared cut, and in the synchronized-update scenario
/// exactly the two cuts inside the update are not compared.
#[test]
fn hand_picked_screens_match_tmux() {
    if !pinned_tmux_or_skip("hand_picked_screens_match_tmux") {
        return;
    }
    let mut server = TmuxServer::new();
    let mut failures = Vec::new();
    for (name, start, pieces) in scenarios() {
        let comparison = compare(&mut server, name, start, &pieces);
        failures.extend(comparison.failure);
        assert_eq!(
            comparison.in_update.last(),
            Some(&false),
            "{name} does not end on a compared cut"
        );
        if name == SYNC_SCENARIO {
            assert_eq!(
                comparison.in_update,
                [false, true, true, false, false],
                "{name}: the cuts inside the update, and only those, go uncompared"
            );
        } else {
            assert!(
                !comparison.in_update.contains(&true),
                "{name} has an uncompared cut"
            );
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// How many generated streams, and pieces in each before the closing ones:
/// about 1,000 cuts, each a round trip through tmux and a tmux command, so
/// tens of seconds on a quiet machine and a few minutes on a loaded one.
const STREAMS: u64 = 20;
const PIECES: usize = 50;

/// The share of generated cuts that must be compared. Updates last one to
/// four pieces and open in about one piece in thirty, so about a tenth of the
/// cuts fall inside one; a regression that left an update open, or made
/// every cut look like one, would fall far below this.
const MIN_COMPARED: f64 = 0.8;

/// Seeded random streams mixing every kind of sequence above (and the
/// queries agents send, and resizes) match tmux 3.7c after every piece.
///
/// Why: the hand-picked screens test each sequence near a clean state;
/// interactions (a wide character written at the margin inside a scroll
/// region, an erase over half of a wide character, a resize mid-frame) are
/// where two terminals' rules part, and generated streams reach them. Spec:
/// as for the hand-picked screens; every stream closes what it opens and
/// ends on a compared cut; at least [`MIN_COMPARED`] of all cuts are
/// compared; a failure names the seed and the pieces that led to it.
#[test]
fn generated_streams_match_tmux() {
    if !pinned_tmux_or_skip("generated_streams_match_tmux") {
        return;
    }
    let mut server = TmuxServer::new();
    let mut failures = Vec::new();
    let (mut cuts, mut compared) = (0, 0);
    for seed in 1..=STREAMS {
        let mut rng = Rng::new(seed);
        let start = *rng.pick(SIZES);
        let mut stream = Stream {
            size: start,
            alternate: false,
            update_left: None,
            saved: false,
            origin: false,
            resize: None,
        };
        let mut pieces: Vec<Piece> = (0..PIECES).map(|_| stream.piece(&mut rng)).collect();
        pieces.extend(stream.finish());
        let comparison = compare(&mut server, &format!("seed {seed}"), start, &pieces);
        if let Some(failure) = comparison.failure {
            failures.push(failure);
            continue;
        }
        assert_eq!(
            comparison.in_update.last(),
            Some(&false),
            "seed {seed} does not end on a compared cut"
        );
        cuts += comparison.in_update.len();
        compared += comparison
            .in_update
            .iter()
            .filter(|inside| !**inside)
            .count();
    }
    assert!(
        failures.is_empty(),
        "{} of {STREAMS} streams diverged:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
    eprintln!("{compared} of {cuts} generated cuts compared");
    assert!(
        compared as f64 >= MIN_COMPARED * cuts as f64,
        "only {compared} of {cuts} cuts were compared"
    );
}

/// One output whose result is known to differ between tmux 3.7c and the
/// emulator, with what each shows: screen text and cursor `(column, line)`.
struct KnownDifference {
    name: &'static str,
    size: Size,
    output: &'static str,
    /// Sizes to resize both to after the output, in order.
    resizes: &'static [Size],
    /// Output after the resizes, for a difference that shows only in what
    /// comes next; empty for none.
    then: &'static str,
    tmux: (&'static str, (usize, usize)),
    emulator: (&'static str, (usize, usize)),
}

/// A known difference on a 20x4 screen without a resize.
const fn known(
    name: &'static str,
    output: &'static str,
    tmux: (&'static str, (usize, usize)),
    emulator: (&'static str, (usize, usize)),
) -> KnownDifference {
    KnownDifference {
        name,
        size: size(20, 4),
        output,
        resizes: &[],
        then: "",
        tmux,
        emulator,
    }
}

/// The known differences between tmux 3.7c and the emulator.
///
/// None of these sequences changed the screen differently in the recorded
/// Claude Code and Codex sessions (most never appear in them), and no frozen
/// screen holds a tab or a variation selector, which is why they are
/// recorded here rather than fixed. They are places where alacritty and tmux
/// chose differently; which of the two xterm or xterm.js, which viewers see
/// today, matches was not checked case by case. A difference at a pending
/// wrap that only autowrap toggles, tabs or resizes of the alternate screen
/// reach is pinned here, not fixed in the vendored crate, unless it shows in
/// the recordings or the frozen screens or changes what a status reader
/// finds: the recorded agents never toggle autowrap or send a tab, and
/// status readers read text, not the cursor. The generated streams send
/// each of these sequences only in a form that avoids the difference
/// (`Stream::piece`).
const KNOWN_DIFFERENCES: &[KnownDifference] = &[
    known(
        "REP past the right margin: tmux stops at the margin, the emulator wraps",
        "x\x1b[25b",
        ("xxxxxxxxxxxxxxxxxxxx\n\n\n\n", (20, 0)),
        ("xxxxxxxxxxxxxxxxxxxx\nxxxxxx\n\n\n", (6, 1)),
    ),
    known(
        "REP after another sequence: tmux repeats nothing, the emulator the last character",
        "x\x1b[1C\x1b[3b",
        ("x\n\n\n\n", (2, 0)),
        ("x xxx\n\n\n\n", (5, 0)),
    ),
    known(
        "insert line above the scroll region: tmux inserts, the emulator ignores it",
        "a\r\nb\r\nc\x1b[2;3r\x1b[1;1H\x1b[1L",
        ("\na\nb\nc\n", (0, 0)),
        ("a\nb\nc\n\n", (0, 0)),
    ),
    known(
        "delete line above the scroll region: tmux deletes, the emulator ignores it",
        "a\r\nb\r\nc\x1b[2;3r\x1b[1;1H\x1b[1M",
        ("b\nc\n\n\n", (0, 0)),
        ("a\nb\nc\n\n", (0, 0)),
    ),
    known(
        "an emoji made wide by VS16: two columns in tmux, one in the emulator",
        "\u{2764}\u{fe0f}xxxxxxxxxxxxxxxxxxx",
        ("\u{2764}\u{fe0f}xxxxxxxxxxxxxxxxxx\nx\n\n\n", (1, 1)),
        ("\u{2764}\u{fe0f}xxxxxxxxxxxxxxxxxxx\n\n\n\n", (20, 0)),
    ),
    known(
        "a tab stop set inside a tab already written",
        "a\tb\x1b[1;3H\x1bH",
        ("a\tb\n\n\n\n", (2, 0)),
        ("a\t      b\n\n\n\n", (2, 0)),
    ),
    known(
        "a tab over a wide character that is later overwritten",
        "\x1b[1;7H😀\x1b[1;1H\t\x1b[1;8H ",
        ("\n\n\n\n", (8, 0)),
        ("\t\n\n\n\n", (8, 0)),
    ),
    known(
        "CHT after insert character",
        "\x1b[1;3rx  lead\x1b[1@\x1b[1I\x08a",
        ("x  leaa\n\n\n\n", (7, 0)),
        ("x  leada\n\n\n\n", (8, 0)),
    ),
    KnownDifference {
        name: "CHT past the last tab stop",
        size: size(40, 4),
        output: "esc to interrupt\x1b[5Ia",
        resizes: &[],
        then: "",
        tmux: ("esc to interrupta\n\n\n\n", (17, 0)),
        emulator: ("esc to interrupt                       a\n\n\n\n", (40, 0)),
    },
    known(
        "moving back with a wrap pending: tmux starts past the last column",
        "\x1b[1;20Hx\x1b[1Dy",
        ("                   y\n\n\n\n", (20, 0)),
        ("                  yx\n\n\n\n", (19, 0)),
    ),
    known(
        "a backspace with a wrap pending: tmux starts past the last column",
        "\x1b[1;20Hx\x08y",
        ("                   y\n\n\n\n", (20, 0)),
        ("                  yx\n\n\n\n", (19, 0)),
    ),
    known(
        "moving to a line with a wrap pending: tmux keeps the cursor past the last column",
        "\x1b[1;20Hx\x1b[2dy",
        ("                   x\n\ny\n\n", (1, 2)),
        ("                   x\n                   y\n\n\n", (20, 1)),
    ),
    known(
        "erasing characters with a wrap pending: tmux keeps the last column",
        "\x1b[1;20Hx\x1b[1X",
        ("                   x\n\n\n\n", (20, 0)),
        ("\n\n\n\n", (20, 0)),
    ),
    known(
        "writing with a wrap pending and autowrap off: tmux drops the character, the emulator \
         overwrites the last column",
        "\x1b[1;20Hx\x1b[?7ly\x1b[?7h",
        ("                   x\n\n\n\n", (20, 0)),
        ("                   y\n\n\n\n", (20, 0)),
    ),
    known(
        "a wide character that does not fit over existing text: tmux keeps the text",
        "abcdefghijklmnopqrst\x1b[1;20H\u{4e2d}",
        ("abcdefghijklmnopqrst\n中\n\n\n", (2, 1)),
        ("abcdefghijklmnopqrs\n中\n\n\n", (2, 1)),
    ),
    known(
        "restoring a cursor never saved, after the alternate screen",
        "\x1b[2;5H\x1b[?1049h\x1b[?1049l\x1b[1;1H\x1b8X",
        ("X\n\n\n\n", (1, 0)),
        ("\n    X\n\n\n", (5, 1)),
    ),
    known(
        "DEC line drawing: tmux's capture shows the ASCII letters, the emulator the lines",
        "\x1b(0lqk\x1b(B",
        ("lqk\n\n\n\n", (3, 0)),
        ("┌─┐\n\n\n\n", (3, 0)),
    ),
    known(
        "restoring a cursor saved with a wrap pending: tmux restores it onto the last column",
        "\x1b[1;20Hx\x1b7\x1b[3;3H\x1b8",
        ("                   x\n\n\n\n", (19, 0)),
        ("                   x\n\n\n\n", (20, 0)),
    ),
    known(
        "restoring a cursor saved in origin mode: tmux restores origin mode too",
        "\x1b[2;4r\x1b[?6h\x1b7\x1b[?6l\x1b8\x1b[1;1HX\x1b[?6l",
        ("\nX\n\n\n", (0, 0)),
        ("X\n\n\n\n", (0, 0)),
    ),
    known(
        "restoring a cursor saved before the alternate screen: the emulator restores 1049's",
        "\x1b[2;5H\x1b7\x1b[3;3H\x1b[?1049h\x1b[?1049l\x1b[1;1H\x1b8X",
        ("\n    X\n\n\n", (5, 1)),
        ("\n\n  X\n\n", (3, 2)),
    ),
    known(
        "setting a scroll region in origin mode: tmux homes to the screen, the emulator to the region",
        "\x1b[?6h\x1b[2;3rX\x1b[?6l",
        ("X\n\n\n\n", (0, 0)),
        ("\nX\n\n\n", (0, 0)),
    ),
    KnownDifference {
        name: "a wide character at the new margin, with the cursor after it",
        size: size(20, 4),
        output: "abcdefghij中文xyz",
        resizes: &[size(11, 4)],
        then: "",
        tmux: ("中文xyz\n\n\n\n", (7, 0)),
        emulator: ("中文xyz\n\n\n\n", (6, 0)),
    },
    KnownDifference {
        name: "more lines after a clear",
        size: size(20, 4),
        output: "one\r\ntwo\x1b[2J\x1b[H",
        resizes: &[size(20, 8)],
        then: "",
        tmux: ("\n\n\n\n\n\n\n\n", (0, 0)),
        emulator: ("one\ntwo\n\n\n\n\n\n\n", (0, 2)),
    },
    KnownDifference {
        name: "narrowing then widening with the cursor at the top: tmux rejoins the row on \
               screen, the emulator in history",
        size: size(20, 6),
        output: "abcdefghij中文xyz\x1b[H",
        resizes: &[size(11, 6), size(20, 6)],
        then: "",
        tmux: ("abcdefghij中文xyz\n\n\n\n\n\n", (10, 0)),
        emulator: ("\n\n\n\n\n\n", (1, 0)),
    },
    known(
        "wrapping in insert mode onto a written line: tmux drops a character",
        "\x1b[2;1Hxxxxx\x1b[4h\x1b[1;20H\u{23fa}ok\x1b[4l",
        ("                   ⏺\nokxxxx\n\n\n", (2, 1)),
        ("                   ⏺\nokxxxxx\n\n\n", (2, 1)),
    ),
    known(
        "a tab with a wrap pending: tmux moves nothing, the emulator wraps",
        "\x1b[1;20Hx\t",
        ("                   x\n\n\n\n", (20, 0)),
        ("                   x\n\n\n\n", (0, 1)),
    ),
    known(
        "a wide character after the corner-cell idiom: the emulator blanks the corner",
        "\x1b[?7l\x1b[4;20Hq\x1b[?7h\u{4e00}",
        ("\n\n                   q\n一\n", (2, 3)),
        ("\n\n\n一\n", (2, 3)),
    ),
    KnownDifference {
        name: "a wider alternate screen with a wrap pending: tmux writes on, the emulator wraps",
        size: size(20, 4),
        output: "\x1b[?1049h\x1b[1;20Hx",
        resizes: &[size(26, 4)],
        then: "y",
        tmux: ("                   xy\n\n\n\n", (21, 0)),
        emulator: ("                   x\ny\n\n\n", (1, 1)),
    },
    KnownDifference {
        name: "a wider alternate screen with a wrap pending and autowrap off: tmux writes on, the \
               emulator overwrites the old last column",
        size: size(20, 4),
        output: "\x1b[?1049h\x1b[1;20Hx\x1b[?7l",
        resizes: &[size(26, 4)],
        then: "y",
        tmux: ("                   xy\n\n\n\n", (21, 0)),
        emulator: ("                   y\n\n\n\n", (21, 0)),
    },
];

/// Each known difference between tmux 3.7c and the emulator still shows
/// exactly the documented screen and cursor on each side.
///
/// Why: the generated streams avoid these cases so that a real regression is
/// not lost among expected differences; pinning both sides keeps the
/// omission honest. If either terminal changes (a tmux upgrade, an emulator
/// fix), this fails and the case either leaves the list or is re-pinned with
/// a reason. Spec: for each case, after its output and its resize, tmux's
/// capture and cursor and the emulator's text and cursor equal the listed
/// ones.
#[test]
fn known_differences_stay_as_documented() {
    if !pinned_tmux_or_skip("known_differences_stay_as_documented") {
        return;
    }
    let mut server = TmuxServer::new();
    let mut failures = Vec::new();
    for case in KNOWN_DIFFERENCES {
        let mut rig = Rig::new(&mut server, case.size);
        let mut cut = rig.step(case.output.as_bytes());
        for size in case.resizes {
            rig.resize(*size);
            cut = rig.step(b"");
        }
        if !case.then.is_empty() {
            cut = rig.step(case.then.as_bytes());
        }
        let tmux = (cut.tmux_text.as_str(), cut.tmux_cursor);
        let emulator = (cut.emulator_text.as_str(), cut.emulator_cursor);
        if tmux != case.tmux || emulator != case.emulator {
            failures.push(format!(
                "{}:\n  tmux:     {tmux:?} (listed {:?})\n  emulator: {emulator:?} (listed {:?})",
                case.name, case.tmux, case.emulator
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
