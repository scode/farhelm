//! The harness marks' geometry, as plain data: which drawing identifies each agent harness, and how each one is placed
//! in the shared 12-unit box.
//!
//! Every mark is one or more SVG path strings in the mark's native coordinates, plus the transform that normalizes
//! them by ONE rule: scale the native bounding box so its longer side spans 10 of the 12 units, then centre it, leaving
//! a one-unit margin so every mark is optically the same weight as the others beside it. The transforms are
//! precomputed, and each mark records the bounding box they came from so the arithmetic can be checked; a test here
//! does exactly that. The rule, the provenance of every geometry, and the brand constraints that shaped the choices
//! are in `docs/harness-marks.md`; read that before adding, replacing, or "tidying" a mark here.
//!
//! Official geometry is used UNALTERED apart from that uniform scale: several of the brand terms forbid modification,
//! and keeping the path bytes identical to the pinned upstream file is what lets `THIRD_PARTY_NOTICES.md` cite a commit
//! rather than describe a derivative. Never render a mark as text: the old UI's letter marks did, and a capital at
//! font-size 9 came out roughly three quarters the height of the drawn marks beside it.
//!
//! Nothing here depends on GPUI. The geometry moved unchanged from the old Dioxus UI's inline SVG when that UI was
//! deleted (GPUI migration, F1); the GPUI views draw it, and own the color, sizing slot and hover text.

/// The harness identity a mark can establish without trusting a display label. `Terminal` is deliberately the
/// fallback: it says that the stored command exists without pretending Farhelm knows what runs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessGlyph {
    Cursor,
    Codex,
    Claude,
    Muse,
    Goose,
    Pi,
    /// OMP, the Pi fork's terminal command. Carries OMP's own block-pi mark, which is visibly a different drawing
    /// from Pi's pixel-grid "P" even at the smallest supported row width.
    Omp,
    Grok,
    OpenCode,
    Terminal,
}

impl HarnessGlyph {
    /// Every harness glyph, for callers and tests that must cover them all.
    pub const ALL: [HarnessGlyph; 10] = [
        HarnessGlyph::Cursor,
        HarnessGlyph::Codex,
        HarnessGlyph::Claude,
        HarnessGlyph::Muse,
        HarnessGlyph::Goose,
        HarnessGlyph::Pi,
        HarnessGlyph::Omp,
        HarnessGlyph::Grok,
        HarnessGlyph::OpenCode,
        HarnessGlyph::Terminal,
    ];

    /// The stable word naming this mark: the old UI's `data-glyph` value and the key its identity colors were
    /// assigned by (`docs/harness-marks.md`, "Colors").
    pub fn token(self) -> &'static str {
        match self {
            HarnessGlyph::Codex => "codex",
            HarnessGlyph::Claude => "claude",
            HarnessGlyph::Muse => "muse",
            HarnessGlyph::Cursor => "cursor",
            HarnessGlyph::Goose => "goose",
            HarnessGlyph::Pi => "pi",
            HarnessGlyph::Omp => "omp",
            HarnessGlyph::Grok => "grok",
            HarnessGlyph::OpenCode => "opencode",
            HarnessGlyph::Terminal => "terminal",
        }
    }

    /// This harness's mark.
    pub fn mark(self) -> &'static HarnessMark {
        match self {
            HarnessGlyph::Claude => &CLAUDE,
            HarnessGlyph::Codex => &CODEX,
            HarnessGlyph::Cursor => &CURSOR,
            HarnessGlyph::Goose => &GOOSE,
            HarnessGlyph::Pi => &PI,
            HarnessGlyph::Omp => &OMP,
            HarnessGlyph::Grok => &GROK,
            HarnessGlyph::OpenCode => &OPENCODE,
            HarnessGlyph::Muse => &MUSE,
            HarnessGlyph::Terminal => &TERMINAL,
        }
    }
}

/// The side of the square box every harness mark is drawn in.
pub const MARK_BOX: f32 = 12.0;

/// How long the longer side of a normalized mark is inside [`MARK_BOX`].
pub const MARK_SPAN: f32 = 10.0;

/// A rectangle in a mark's native coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A uniform scale followed by a translation, as SVG's `translate(tx ty) scale(s)` applies it: a native point `p` lands
/// at `p * scale + translate`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub translate_x: f32,
    pub translate_y: f32,
    pub scale: f32,
}

impl Transform {
    /// Draw native coordinates as they are; for marks drawn directly in the 12-unit box.
    pub const IDENTITY: Transform = Transform {
        translate_x: 0.0,
        translate_y: 0.0,
        scale: 1.0,
    };

    /// The sizing rule: with `s = 10 / max(w, h)`, `translate((12 - w*s)/2 - x*s, (12 - h*s)/2 - y*s) scale(s)`.
    pub fn normalizing(bounds: BoundingBox) -> Transform {
        let scale = MARK_SPAN / bounds.width.max(bounds.height);
        Transform {
            translate_x: (MARK_BOX - bounds.width * scale) / 2.0 - bounds.x * scale,
            translate_y: (MARK_BOX - bounds.height * scale) / 2.0 - bounds.y * scale,
            scale,
        }
    }
}

/// How one path is painted, in the mark's single color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Paint {
    /// Filled with the nonzero rule.
    Fill,
    /// Filled with the even-odd rule, which is what leaves a ring's hole open.
    FillEvenOdd,
    /// Stroked with round caps and joins at this width, in native units (it scales with the transform).
    Stroke { width: f32 },
}

/// One path of a mark: SVG path data in the mark's native coordinates, and how it is painted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkPath {
    pub d: &'static str,
    pub paint: Paint,
}

/// A harness mark: its paths, and where they go in the 12-unit box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarnessMark {
    pub paths: &'static [MarkPath],
    /// The native bounding box the transform was computed from, or `None` for a mark drawn directly in the 12-unit
    /// box. For a stroked mark it includes the stroke's extent, round caps and all.
    pub bounds: Option<BoundingBox>,
    /// The precomputed result of [`Transform::normalizing`] for `bounds`, rounded as the old inline SVG wrote it.
    pub transform: Transform,
}

/// Farhelm's own Claude mark: an eight-ray tapered starburst, generated as eight triangles from a 2.2-unit-wide base at
/// the centre of a 24-unit box to a tip 11 units out. It deliberately is NOT the Anthropic spark: Anthropic's trademark
/// guidelines grant no self-serve right to display the logo and forbid recolouring it (see `docs/harness-marks.md`), so
/// the mark evokes "spark" without copying it.
const CLAUDE_STARBURST: &str = "M14.20 12.00L12.00 1.00L9.80 12.00ZM13.56 13.56L19.78 4.22L10.44 10.44ZM12.00 14.20L23.00 12.00L12.00 9.80ZM10.44 13.56L19.78 19.78L13.56 10.44ZM9.80 12.00L12.00 23.00L14.20 12.00ZM10.44 10.44L4.22 19.78L13.56 13.56ZM12.00 9.80L1.00 12.00L12.00 14.20ZM13.56 10.44L4.22 4.22L10.44 13.56Z";

/// OpenAI's monochrome "blossom", used here to identify Codex. Bytes of the `d` attribute of
/// `OpenAI-black-monoblossom.svg` from OpenAI's official brand download, unaltered; OpenAI's brand terms permit
/// referential use but forbid modification, which is why the 2.5 KB path lives in its own file rather than being
/// simplified for this size.
const OPENAI_BLOSSOM: &str = include_str!("marks/openai-blossom.svgpath");

/// Cursor's official 2D cube (`CUBE_2D_LIGHT.svg` from cursor.com/brand), unaltered. Cursor's own site renders this
/// same geometry in `currentColor`; Farhelm supplies an identity hue rather than claiming the owner's neutral variants
/// have an official chromatic color.
const CURSOR_CUBE: &str = "M457.43,125.94L244.42,2.96c-6.84-3.95-15.28-3.95-22.12,0L9.3,125.94c-5.75,3.32-9.3,9.46-9.3,16.11v247.99c0,6.65,3.55,12.79,9.3,16.11l213.01,122.98c6.84,3.95,15.28,3.95,22.12,0l213.01-122.98c5.75-3.32,9.3-9.46,9.3-16.11v-247.99c0-6.65-3.55-12.79-9.3-16.11h-.01ZM444.05,151.99l-205.63,356.16c-1.39,2.4-5.06,1.42-5.06-1.36v-233.21c0-4.66-2.49-8.97-6.53-11.31L24.87,145.67c-2.4-1.39-1.42-5.06,1.36-5.06h411.26c5.84,0,9.49,6.33,6.57,11.39h-.01Z";

/// Goose's official glyph, the 24-unit path from the project's own `Goose.tsx` icon component (Apache-2.0), unaltered.
/// A detailed goose silhouette; the project's own smallest use is 24px, so at sidebar size it reads as a bird-shaped
/// mark rather than a goose. Kept because it is the mark and it is permitted, not because it is crisp.
const GOOSE_GLYPH: &str = include_str!("marks/goose.svgpath");

/// Pi's official badge mark from the pi.dev press kit: a 4x4 pixel grid "P" with a detached dot. Every coordinate is a
/// multiple of 140 in a 560 box, so the normalized mark lands on 2.5-unit cells exactly.
const PI_BADGE: &str =
    "M420 280H280V140H0V0H420V280ZM560 560H420V280H560V560ZM140 560H0V140H140V280H280V420H140V560Z";

/// OMP's official mark, the block-style pi from omp.sh's favicon, with the favicon's background tile dropped. Three
/// axis-aligned rectangles, so it survives 12px with no anti-aliasing fuzz. This replaced an earlier Greek omega that
/// was Farhelm's own invention.
const OMP_PI: &str = "M14 16h36v8H40v32h-8V24h-6v22h-8V24h-4z";

/// Farhelm's own Grok mark: a block "G" drawn as one path. The current official kit requires its logos unaltered; this
/// original drawing names the harness independently of xAI's artwork.
const GROK_GLYPH: &str = "M21 4H8C4.686 4 2 6.686 2 10V14C2 17.314 4.686 20 8 20H21V11H12V15H16V16H8C6.895 16 6 15.105 6 14V10C6 8.895 6.895 8 8 8H21Z";

/// OpenCode's favicon cut: one ring drawn with the even-odd rule. The project's logo file is a two-tone pair of nested
/// frames whose inner mid-grey square collapses to one color; the favicon is the small-size simplification the project
/// itself publishes.
const OPENCODE_RING: &str = "M384 416H128V96H384V416ZM320 160H192V352H320V160Z";

/// Muse Code's mark is borrowed from an unofficial community VS Code extension (MIT), because no Muse Code mark was
/// found in the checked sources and Meta's own corporate marks require brand approval for any use. It is a stroked "M"
/// whose four terminals carry round dots; the source icon's blue is replaced by a Farhelm hue and its sparkle is
/// omitted. Stroked rather than filled, so the stroke widths are the source's own and scale with the transform.
const MUSE_STEM: &str = "M4 20V5l6 9 6-9v15";
const MUSE_DOTS: &str = "M4 20h0M16 20h0M4 5h0M16 5h0";

/// Farhelm's own terminal mark, a window with a prompt, drawn directly in the 12-unit box.
const TERMINAL_WINDOW: &str = "M1.5 2h9v8h-9zM3.1 4l1.5 1.5L3.1 7M6 7h2.5";

/// Shorthand for the marks below, whose transforms are written as the old inline SVG had them.
const fn mark(
    paths: &'static [MarkPath],
    bounds: BoundingBox,
    translate_x: f32,
    translate_y: f32,
    scale: f32,
) -> HarnessMark {
    HarnessMark {
        paths,
        bounds: Some(bounds),
        transform: Transform {
            translate_x,
            translate_y,
            scale,
        },
    }
}

/// Shorthand for a native bounding box.
const fn bbox(x: f32, y: f32, width: f32, height: f32) -> BoundingBox {
    BoundingBox {
        x,
        y,
        width,
        height,
    }
}

const fn fill(d: &'static str) -> MarkPath {
    MarkPath {
        d,
        paint: Paint::Fill,
    }
}

// In a 24 box.
const CLAUDE: HarnessMark = mark(
    &[fill(CLAUDE_STARBURST)],
    bbox(1.0, 1.0, 22.0, 22.0),
    0.5455,
    0.5455,
    0.454545,
);
// In a 721 box.
const CODEX: HarnessMark = mark(
    &[fill(OPENAI_BLOSSOM)],
    bbox(118.557, 119.958, 484.139, 479.818),
    -1.4488,
    -1.4331,
    0.020655,
);
const CURSOR: HarnessMark = mark(
    &[fill(CURSOR_CUBE)],
    bbox(0.0, 0.0, 466.74, 532.095),
    1.6141,
    1.0,
    0.018794,
);
// In a 24 box.
const GOOSE: HarnessMark = mark(
    &[fill(GOOSE_GLYPH)],
    bbox(4.328, 3.789, 16.628, 16.628),
    -1.6030,
    -1.2788,
    0.601409,
);
const PI: HarnessMark = mark(
    &[fill(PI_BADGE)],
    bbox(0.0, 0.0, 560.0, 560.0),
    1.0,
    1.0,
    0.017857,
);
// In a 64 box.
const OMP: HarnessMark = mark(
    &[fill(OMP_PI)],
    bbox(14.0, 16.0, 36.0, 40.0),
    -2.0,
    -3.0,
    0.25,
);
// In a 24 box.
const GROK: HarnessMark = mark(
    &[fill(GROK_GLYPH)],
    bbox(2.0, 4.0, 19.0, 16.0),
    -0.0526,
    -0.3158,
    0.526316,
);
// In a 512 box.
const OPENCODE: HarnessMark = mark(
    &[MarkPath {
        d: OPENCODE_RING,
        paint: Paint::FillEvenOdd,
    }],
    bbox(128.0, 96.0, 256.0, 320.0),
    -2.0,
    -2.0,
    0.03125,
);
// Stroke geometry: stem stroke 2.6 and dot stroke 4.2 with round caps give a bbox of 1.9,2.9 16.2x19.2 in the
// source's 24 box.
const MUSE: HarnessMark = mark(
    &[
        MarkPath {
            d: MUSE_STEM,
            paint: Paint::Stroke { width: 2.6 },
        },
        MarkPath {
            d: MUSE_DOTS,
            paint: Paint::Stroke { width: 4.2 },
        },
    ],
    bbox(1.9, 2.9, 16.2, 19.2),
    0.7917,
    -0.5104,
    0.520833,
);
const TERMINAL: HarnessMark = HarnessMark {
    paths: &[MarkPath {
        d: TERMINAL_WINDOW,
        paint: Paint::Stroke { width: 1.2 },
    }],
    bounds: None,
    transform: Transform::IDENTITY,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: every recorded transform is the sizing rule applied to its recorded bounding box.
    ///
    /// The marks agree on size only because each transform follows the rule; a hand-tuned transform ("make it look
    /// right") would pass review unnoticed without this check, which is the arithmetic `docs/harness-marks.md` asks a
    /// reader to be able to repeat. The tolerance covers the four-decimal rounding the transforms were written with.
    #[test]
    fn every_transform_follows_the_sizing_rule() {
        for glyph in HarnessGlyph::ALL {
            let mark = glyph.mark();
            let Some(bounds) = mark.bounds else {
                assert_eq!(mark.transform, Transform::IDENTITY, "{glyph:?}");
                continue;
            };
            let rule = Transform::normalizing(bounds);
            for (recorded, computed) in [
                (mark.transform.translate_x, rule.translate_x),
                (mark.transform.translate_y, rule.translate_y),
                (mark.transform.scale, rule.scale),
            ] {
                assert!(
                    (recorded - computed).abs() < 1e-3,
                    "{glyph:?}: recorded {:?}, rule gives {rule:?}",
                    mark.transform
                );
            }
        }
    }

    /// Spec: every mark has at least one path, and the vendored files are the whole upstream `d` attribute.
    ///
    /// A truncated or newline-terminated `.svgpath` file would still parse in most renderers and silently draw a
    /// partial mark, which is a modification the brand terms forbid.
    #[test]
    fn every_mark_has_path_data_and_vendored_files_are_single_paths() {
        for glyph in HarnessGlyph::ALL {
            let mark = glyph.mark();
            assert!(!mark.paths.is_empty(), "{glyph:?}");
            for path in mark.paths {
                assert!(path.d.starts_with('M'), "{glyph:?}: {:?}", &path.d[..8]);
            }
        }
        for vendored in [OPENAI_BLOSSOM, GOOSE_GLYPH] {
            assert!(!vendored.contains('\n'), "a vendored path is one line");
            assert!(
                vendored.trim_end().ends_with(['Z', 'z']),
                "a vendored path is complete"
            );
        }
    }
}
