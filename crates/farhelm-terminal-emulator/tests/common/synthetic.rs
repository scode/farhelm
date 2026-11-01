//! Deterministic synthetic recordings: terminal output with resizes, built to
//! reach the state real agent sessions rarely do. The alternate screen with
//! resizes while it shows, saved cursors, origin mode and scroll regions, tab
//! stops, character sets, the title stack, palette changes, cursor styles,
//! hyperlinks, wide and combining characters at row edges, erases, inserts and
//! deletes inside wide characters, REP, insert mode, autowrap off, queries,
//! OSC 52, synchronized updates (some never ended), and sequences cut short.
//!
//! This is the snapshot spike's generator, ported unchanged in what it
//! produces: seed `n` with 400,000 bytes gives the spike's `fuzz-n`
//! recording, over which its encoder passed every round trip. Keeping the
//! generator rather than the recordings keeps 3 MB of binary out of the
//! repository, and lets the tests pick sizes.

/// xorshift64*: small, and the same sequence on every platform.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..n` (0 when `n` is 0).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    /// A number in `low..=high`.
    pub fn range(&mut self, low: u64, high: u64) -> u64 {
        low + self.below(high - low + 1)
    }

    /// True with probability about `p`.
    pub fn chance(&mut self, p: f64) -> bool {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 <= p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
}

const WORDS: &[&str] = &[
    "alpha", "beta", "grid", "pane", "helm", "frame", "x", "ok", "ERR", "--", "|", "#",
];
const WIDE: &[char] = &['中', '文', '字', '🎉', '한', 'ｱ', '😀'];
const ZERO_WIDTH: &[char] = &['\u{301}', '\u{308}', '\u{200d}', '\u{fe0f}', '\u{20e3}'];

/// A run of printable text: words, wide and combining characters, box
/// drawing, tabs, runs that fill to the margin.
fn text(r: &mut Rng, out: &mut Vec<u8>) {
    for _ in 0..r.range(1, 30) {
        let k = r.below(100);
        let piece: String = if k < 55 {
            r.pick(WORDS).to_string() + " "
        } else if k < 70 {
            r.pick(WIDE).to_string()
        } else if k < 78 {
            format!("e{}", r.pick(ZERO_WIDTH))
        } else if k < 83 {
            "─│┌".to_owned()
        } else if k < 88 {
            "\t".to_owned()
        } else if k < 92 {
            // Sometimes exactly to the margin, to land wide characters and
            // wraps on the last column.
            "abcdefghijklmnopqrstuvwxyz0123456789"
                .chars()
                .take(r.range(1, 36) as usize)
                .collect()
        } else if k < 96 {
            "qxlmjk".to_owned()
        } else {
            "é".to_owned()
        };
        out.extend_from_slice(piece.as_bytes());
    }
}

/// An SGR sequence of one to four attributes or colors.
fn sgr(r: &mut Rng) -> String {
    let attributes: &[&str] = &[
        "0", "1", "2", "3", "4", "4:2", "4:3", "4:4", "4:5", "7", "8", "9", "21", "22", "23", "24",
        "27", "28", "29", "39", "49", "59", "5",
    ];
    let mut params = Vec::new();
    for _ in 0..r.range(1, 4) {
        params.push(match r.below(10) {
            0..=3 => r.pick(attributes).to_string(),
            4 => format!("{}", 30 + r.below(8)),
            5 => format!("{}", 40 + r.below(8)),
            6 => format!("{}", 90 + r.below(8)),
            7 => format!("38;5;{}", r.below(256)),
            8 => format!("48;2;{};{};{}", r.below(256), r.below(256), r.below(256)),
            _ => format!("58;5;{}", r.below(256)),
        });
    }
    format!("\x1b[{}m", params.join(";"))
}

/// About `size` bytes of output, and the terminal sizes in force from given
/// offsets: `(offset, columns, lines)`, the first at offset 0 and some
/// repeated at one offset (resizes with no output between them).
pub fn generate(seed: u64, size: usize) -> (Vec<u8>, Vec<(usize, u16, u16)>) {
    let mut r = Rng::new(seed);
    let mut out: Vec<u8> = Vec::with_capacity(size + 1024);
    let (mut columns, mut lines) = (r.range(20, 100), r.range(5, 30));
    let mut sizes = vec![(0, columns as u16, lines as u16)];
    let mut next_resize = r.range(5_000, 40_000) as usize;
    while out.len() < size {
        if out.len() >= next_resize {
            let count = if r.chance(0.2) { 2 } else { 1 };
            for _ in 0..count {
                columns = r.range(8, 120);
                lines = r.range(3, 40);
                sizes.push((out.len(), columns as u16, lines as u16));
            }
            next_resize = out.len() + r.range(5_000, 40_000) as usize;
        }
        let piece: String = match r.below(1000) {
            0..=299 => {
                text(&mut r, &mut out);
                continue;
            }
            300..=349 => "\r\n".into(),
            350..=369 => "\n".into(),
            370..=379 => "\r".into(),
            380..=389 => "\x08".into(),
            390..=429 => format!(
                "\x1b[{};{}H",
                r.range(0, lines + 2),
                r.range(0, columns + 2)
            ),
            430..=449 => format!(
                "\x1b[{}{}",
                r.range(0, 5),
                *r.pick(&['A', 'B', 'C', 'D', 'E', 'F', 'G', 'd', 'a', 'e', '`'])
            ),
            450..=519 => sgr(&mut r),
            520..=534 => {
                if r.chance(0.5) {
                    "\x1b]8;;\x1b\\".into()
                } else if r.chance(0.5) {
                    format!(
                        "\x1b]8;id=l{};https://e.example/{}\x1b\\",
                        r.below(5),
                        r.below(100)
                    )
                } else {
                    format!("\x1b]8;;https://n.example/{}?a=1;b=2\x07", r.below(100))
                }
            }
            535..=574 => {
                let erase = *r.pick(&["J", "1J", "2J", "3J", "K", "1K", "2K"]);
                if r.chance(0.5) {
                    format!("\x1b[{erase}")
                } else {
                    format!(
                        "\x1b[{}{}",
                        r.range(1, 6),
                        *r.pick(&['X', '@', 'P', 'L', 'M', 'S', 'T', 'b'])
                    )
                }
            }
            575..=584 => (*r.pick(&["\x1bM", "\x1bD", "\x1bE"])).into(),
            585..=599 => {
                if r.chance(0.3) {
                    "\x1b[r".into()
                } else {
                    let top = r.range(0, lines);
                    let bottom = r.range(top, lines + 2);
                    format!("\x1b[{top};{bottom}r")
                }
            }
            600..=624 => (*r.pick(&["\x1b7", "\x1b8", "\x1b[s", "\x1b[u"])).into(),
            625..=674 => {
                let modes = [
                    "?1049", "?1049", "?1049", "?6", "?7", "4", "20", "?25", "?1", "?1000",
                    "?1002", "?1003", "?1005", "?1006", "?2004", "?1004", "?1007", "?1042", "?12",
                ];
                let mode = r.pick(&modes);
                format!("\x1b[{mode}{}", if r.chance(0.5) { 'h' } else { 'l' })
            }
            675..=679 => (*r.pick(&["\x1b=", "\x1b>"])).into(),
            680..=699 => (*r.pick(&[
                "\x1b(0", "\x1b(B", "\x1b)0", "\x1b)B", "\x0e", "\x0f", "\x1b*0", "\x1b+0",
            ]))
            .into(),
            700..=719 => {
                (*r.pick(&["\x1bH", "\x1b[g", "\x1b[3g", "\x1b[2I", "\x1b[Z", "\t"])).into()
            }
            720..=734 => format!("\x1b[{}b", r.range(1, 8)),
            735..=749 => match r.below(4) {
                0 => format!("\x1b]2;title {}\x1b\\", r.below(50)),
                1 => format!("\x1b]0; spaced {} ;x\x07", r.below(50)),
                2 => "\x1b[22t".into(),
                _ => "\x1b[23t".into(),
            },
            750..=764 => match r.below(4) {
                0 => format!(
                    "\x1b]4;{};rgb:{:02x}/{:02x}/{:02x}\x1b\\",
                    r.below(256),
                    r.below(256),
                    r.below(256),
                    r.below(256)
                ),
                1 => format!(
                    "\x1b]{};#{:02x}{:02x}{:02x}\x07",
                    r.range(10, 12),
                    r.below(256),
                    r.below(256),
                    r.below(256)
                ),
                2 => format!("\x1b]104;{}\x07", r.below(256)),
                _ => format!("\x1b]{}\x07", r.pick(&["110", "111", "112", "104"])),
            },
            765..=774 => format!("\x1b[{} q", r.below(7)),
            775..=776 => "\x1b#8".into(),
            777 => {
                if r.chance(0.3) {
                    "\x1bc".into()
                } else {
                    String::new()
                }
            }
            778..=789 => (*r.pick(&[
                "\x1b[c",
                "\x1b[>c",
                "\x1b[5n",
                "\x1b[6n",
                "\x1b[?2026$p",
                "\x1b[?25$p",
                "\x1b]11;?\x07",
                "\x1b[18t",
                "\x1b[14t",
                "\x1b[?u",
            ]))
            .into(),
            790..=794 => "\x07".into(),
            795..=799 => format!(
                "\x1b]52;c;{}\x07",
                ["aGVsbG8=", "d29ybGQ=", "Zm9v"][r.below(3) as usize]
            ),
            800..=829 => {
                // A synchronized update around a burst of drawing, now and
                // then never ended.
                let mut frame = String::from("\x1b[?2026h");
                for _ in 0..r.range(1, 10) {
                    frame += &format!("\x1b[{};{}H", r.range(1, lines), r.range(1, columns));
                    frame += &sgr(&mut r);
                    frame += "frame ";
                    frame += r.pick(&["中", "x", "─"]);
                }
                if !r.chance(0.05) {
                    frame += "\x1b[?2026l";
                }
                frame
            }
            830..=839 => {
                // A sequence cut short and abandoned: by CAN, by ESC, or by
                // plain text.
                let start = *r.pick(&["\x1b[12", "\x1b]2;half", "\x1b[?10", "\x1bP1$", "\x1b_apc"]);
                let end = *r.pick(&["\x18", "\x1b\\", "", "\x1a"]);
                format!("{start}{end}")
            }
            840..=844 => format!(
                "\x1b[{}m{}",
                r.below(108),
                "\u{4e2d}".repeat(r.range(1, 3) as usize)
            ),
            845..=849 => format!("\x1bP+q{}\x1b\\", "544e"),
            _ => {
                text(&mut r, &mut out);
                continue;
            }
        };
        out.extend_from_slice(piece.as_bytes());
    }
    (out, sizes)
}
