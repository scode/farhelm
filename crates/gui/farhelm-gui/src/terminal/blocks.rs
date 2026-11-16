//! The Unicode block elements (U+2580 to U+259F) drawn as filled parts of their cell instead of font glyphs.
//!
//! The bundled font's block glyphs are shorter than the terminal's cell at most text sizes, so stacked full blocks
//! showed a stripe of background under every row at every size except 14, 17, 20 and 28 points (measured on a 2x
//! display). Programs that draw bars, meters and pixel art from these characters expect them to tile. Drawing each one
//! as rectangles of the cell's own bounds makes them meet the same way cell backgrounds do, at every size. This
//! deviates from the old app, which drew them from the font; the maintainer chose it on 2026-11-14.
//!
//! NOTE: An eighth-wide part (▏, ▕, ▁, ▔) is narrower than one device pixel on a 1x display at small sizes (0.9 of a
//! pixel at 12 points), and GPUI rounds each edge to a device pixel on its own, so in some columns it rounds away to
//! nothing. At 2x an eighth is always wider than a device pixel. Known and left as is.
//!
//! The shades (U+2591 to U+2593) become flat fills of the whole cell at a quarter, half and three quarters of the
//! foreground's opacity rather than a dot pattern. Box drawing (U+2500 to U+257F) is not covered: its line glyphs
//! already reach past their cell and meet at every size measured.

use gpui::{Bounds, Pixels, point, size};

/// One block element's ink: the parts of the cell it fills, and how opaque the fill is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Block {
    /// Rectangles in eighths of the cell, as `[left, top, right, bottom]` measured from its top-left corner.
    parts: &'static [[u8; 4]],
    /// The fill's share of the foreground's opacity: 1 for every element but the three shades.
    pub(super) opacity: f32,
}

/// The block element `text` draws, when the text is exactly one block element.
///
/// A block element followed by a combining mark (or anything else in the same cell) returns `None` and stays a font
/// glyph, since the mark has to be shaped with its base.
pub(super) fn block(text: &str) -> Option<Block> {
    let mut chars = text.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let full = |parts| Block { parts, opacity: 1. };
    let shade = |opacity| Block {
        parts: &[[0, 0, 8, 8]],
        opacity,
    };
    Some(match c {
        '▀' => full(&[[0, 0, 8, 4]]),
        '▁' => full(&[[0, 7, 8, 8]]),
        '▂' => full(&[[0, 6, 8, 8]]),
        '▃' => full(&[[0, 5, 8, 8]]),
        '▄' => full(&[[0, 4, 8, 8]]),
        '▅' => full(&[[0, 3, 8, 8]]),
        '▆' => full(&[[0, 2, 8, 8]]),
        '▇' => full(&[[0, 1, 8, 8]]),
        '█' => full(&[[0, 0, 8, 8]]),
        '▉' => full(&[[0, 0, 7, 8]]),
        '▊' => full(&[[0, 0, 6, 8]]),
        '▋' => full(&[[0, 0, 5, 8]]),
        '▌' => full(&[[0, 0, 4, 8]]),
        '▍' => full(&[[0, 0, 3, 8]]),
        '▎' => full(&[[0, 0, 2, 8]]),
        '▏' => full(&[[0, 0, 1, 8]]),
        '▐' => full(&[[4, 0, 8, 8]]),
        '░' => shade(0.25),
        '▒' => shade(0.5),
        '▓' => shade(0.75),
        '▔' => full(&[[0, 0, 8, 1]]),
        '▕' => full(&[[7, 0, 8, 8]]),
        // The quadrants: upper left, upper right, lower left and lower right are each a quarter of the cell.
        '▖' => full(&[[0, 4, 4, 8]]),
        '▗' => full(&[[4, 4, 8, 8]]),
        '▘' => full(&[[0, 0, 4, 4]]),
        '▙' => full(&[[0, 0, 4, 8], [4, 4, 8, 8]]),
        '▚' => full(&[[0, 0, 4, 4], [4, 4, 8, 8]]),
        '▛' => full(&[[0, 0, 8, 4], [0, 4, 4, 8]]),
        '▜' => full(&[[0, 0, 8, 4], [4, 4, 8, 8]]),
        '▝' => full(&[[4, 0, 8, 4]]),
        '▞' => full(&[[4, 0, 8, 4], [0, 4, 4, 8]]),
        '▟' => full(&[[4, 0, 8, 8], [0, 4, 4, 8]]),
        _ => return None,
    })
}

impl Block {
    /// The rectangles to fill inside `cell`. Each is placed and sized as eighths of the cell (never as a difference
    /// of two edges, which float rounding would make a hair off the cell's own size), so a full block is exactly the
    /// cell's bounds, as its background is, and neighbours meet where the backgrounds do.
    pub(super) fn rects(self, cell: Bounds<Pixels>) -> impl Iterator<Item = Bounds<Pixels>> {
        let eighths = |n: u8, extent: Pixels| extent * (f32::from(n) / 8.);
        self.parts.iter().map(move |&[left, top, right, bottom]| {
            let origin = cell.origin
                + point(
                    eighths(left, cell.size.width),
                    eighths(top, cell.size.height),
                );
            let size = size(
                eighths(right - left, cell.size.width),
                eighths(bottom - top, cell.size.height),
            );
            Bounds::new(origin, size)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    /// The cell in the tests: fractional, like a real cell on a 2x display, so the edge arithmetic is exercised.
    fn cell() -> Bounds<Pixels> {
        Bounds::new(point(px(16.8), px(18.5)), size(px(8.4), px(167. / 9.)))
    }

    /// Area covered, in 64ths of the cell, with overlapping parts counted once.
    fn coverage(block: Block) -> usize {
        (0..8u8)
            .flat_map(|x| (0..8u8).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                block
                    .parts
                    .iter()
                    .any(|&[l, t, r, b]| l <= x && x < r && t <= y && y < b)
            })
            .count()
    }

    /// Every block element in U+2580 to U+259F is drawn, and nothing either side of the range is, so a missed or
    /// mistyped arm in the table shows up here rather than as one character still striping.
    ///
    /// Spec: `block` answers for exactly the 32 characters U+2580 to U+259F; U+257F (the last box-drawing character)
    /// and U+25A0 (a geometric shape) stay glyphs.
    #[test]
    fn covers_exactly_the_block_elements() {
        for c in '\u{2580}'..='\u{259f}' {
            assert!(block(&c.to_string()).is_some(), "{c:?} is not drawn");
        }
        for c in ['\u{257f}', '\u{25a0}', 'A', '─'] {
            assert_eq!(block(&c.to_string()), None, "{c:?} is drawn");
        }
    }

    /// The table's geometry, checked by area against the Unicode names, so a part on the wrong side or of the wrong
    /// size is caught. The eighth blocks are where an off-by-one would hide.
    ///
    /// Spec: the lower and left blocks fill n eighths of the cell for n from 1 to 7; the upper half, lower half, left
    /// half and right half fill half; the upper and right eighths fill one eighth; each quadrant counts a quarter, so
    /// one quadrant fills 16 of 64, two fill 32 and three fill 48; the full block and the shades fill the whole cell,
    /// the shades at 0.25, 0.5 and 0.75 opacity and everything else at 1.
    #[test]
    fn each_element_fills_its_named_share() {
        let area = |c: char| coverage(block(&c.to_string()).unwrap());
        for (n, (lower, left)) in ['▁', '▂', '▃', '▄', '▅', '▆', '▇']
            .into_iter()
            .zip(['▏', '▎', '▍', '▌', '▋', '▊', '▉'])
            .enumerate()
        {
            assert_eq!(area(lower), (n + 1) * 8, "{lower:?}");
            assert_eq!(area(left), (n + 1) * 8, "{left:?}");
        }
        for c in ['▀', '▄', '▌', '▐'] {
            assert_eq!(area(c), 32, "{c:?}");
        }
        for c in ['▔', '▕'] {
            assert_eq!(area(c), 8, "{c:?}");
        }
        for (c, quarters) in [
            ('▖', 1),
            ('▗', 1),
            ('▘', 1),
            ('▝', 1),
            ('▚', 2),
            ('▞', 2),
            ('▙', 3),
            ('▛', 3),
            ('▜', 3),
            ('▟', 3),
        ] {
            assert_eq!(area(c), quarters * 16, "{c:?}");
        }
        for (c, opacity) in [('█', 1.), ('░', 0.25), ('▒', 0.5), ('▓', 0.75)] {
            let b = block(&c.to_string()).unwrap();
            assert_eq!((coverage(b), b.opacity), (64, opacity), "{c:?}");
        }
    }

    /// Which quarters of the cell each half and quadrant fills, and which edge each eighth sits on, from the
    /// characters' Unicode names: an area check alone passes a part on the wrong side (▖ swapped with ▗, ▐ mirrored).
    ///
    /// Spec: each half and quadrant fills exactly the quarters its name says (upper and lower, left and right); the
    /// upper and lower eighths sit on the top and bottom row of eighths, the left and right eighths on the first and
    /// last column of eighths.
    #[test]
    fn each_part_sits_where_its_name_says() {
        // The quarters as (column, row) of the cell's 2 by 2 grid, which `coverage`'s 8 by 8 cells map onto.
        let quarters = |c: char| -> Vec<(u8, u8)> {
            let b = block(&c.to_string()).unwrap();
            let mut filled: Vec<(u8, u8)> = (0..2u8)
                .flat_map(|x| (0..2u8).map(move |y| (x, y)))
                .filter(|&(x, y)| {
                    let (cx, cy) = (x * 4 + 2, y * 4 + 2);
                    b.parts
                        .iter()
                        .any(|&[l, t, r, bo]| l <= cx && cx < r && t <= cy && cy < bo)
                })
                .collect();
            filled.sort();
            filled
        };
        let (ul, ur, ll, lr) = ((0, 0), (1, 0), (0, 1), (1, 1));
        for (c, mut expected) in [
            ('▀', vec![ul, ur]),
            ('▄', vec![ll, lr]),
            ('▌', vec![ul, ll]),
            ('▐', vec![ur, lr]),
            ('▖', vec![ll]),
            ('▗', vec![lr]),
            ('▘', vec![ul]),
            ('▝', vec![ur]),
            ('▙', vec![ul, ll, lr]),
            ('▚', vec![ul, lr]),
            ('▛', vec![ul, ur, ll]),
            ('▜', vec![ul, ur, lr]),
            ('▞', vec![ur, ll]),
            ('▟', vec![ur, ll, lr]),
        ] {
            expected.sort();
            assert_eq!(quarters(c), expected, "{c:?}");
        }
        let only = |c: char| block(&c.to_string()).unwrap().parts;
        assert_eq!(only('▔'), &[[0, 0, 8, 1]], "upper eighth");
        assert_eq!(only('▁'), &[[0, 7, 8, 8]], "lower eighth");
        assert_eq!(only('▏'), &[[0, 0, 1, 8]], "left eighth");
        assert_eq!(only('▕'), &[[7, 0, 8, 8]], "right eighth");
    }

    /// The point of drawing blocks at all: a full block is the cell's own bounds, bit for bit, so it meets the blocks
    /// around it exactly where the backgrounds meet, and the upper and lower halves split the cell at one shared edge.
    ///
    /// Spec: `rects` of the full block in a fractional cell equals the cell; the upper half starts at the cell's top
    /// and ends exactly where the lower half starts; the lower half ends at the cell's bottom and the right eighth at
    /// its right edge, its left edge seven eighths across (within float rounding); a block followed by a combining
    /// mark is not drawn.
    #[test]
    fn rects_meet_at_the_cell_edges() {
        let cell = cell();
        let rects = |c: &str| block(c).unwrap().rects(cell).collect::<Vec<_>>();
        let near = |a: Pixels, b: Pixels| (f32::from(a) - f32::from(b)).abs() < 1e-4;
        assert_eq!(rects("█"), [cell]);
        assert_eq!(rects("▀")[0].bottom(), rects("▄")[0].top());
        assert_eq!(rects("▀")[0].top(), cell.top());
        assert!(near(rects("▄")[0].bottom(), cell.bottom()));
        assert!(near(rects("▕")[0].right(), cell.right()));
        assert!(near(
            rects("▕")[0].left(),
            cell.left() + cell.size.width * 0.875
        ));
        assert_eq!(block("█\u{301}"), None);
    }
}
