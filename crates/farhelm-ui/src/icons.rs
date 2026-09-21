//! Inline SVG glyphs for compact sidebar facts.
//!
//! This is the app's first icon vocabulary (2026-09-03): before this
//! module the only glyph anywhere in the UI was the "⋯" menu
//! toggle, which is plain text rather than a drawn shape. Two considerations
//! ruled out the two obvious alternatives to inline SVG. An icon FONT ties
//! legibility to whatever glyph shapes the two engines' font stacks happen to
//! agree on, which is exactly the kind of cross-engine variance this project
//! avoids by keeping the desktop app on the same WebKit family the browser
//! suite already covers (see `docs/desktop-web-triage.md`). An icon FILE would
//! have to be added to the desktop build's `asset!()` set and the web bundle
//! in lockstep (`scripts/check-desktop-assets.sh` fails the moment the two
//! diverge) for two glyphs simple enough to draw directly in markup — a
//! second asset-parity surface with nothing to show for it. Inline SVG pays
//! neither cost: the shape is markup, `currentColor` lets the caller style
//! its meaning, and there is no file for the two build targets to disagree
//! about. Remote glyphs inherit the surrounding text color; session-local
//! glyphs retain an explicit red caution color across row states.
//!
//! Both components below deliberately carry no accessible name of their own
//! (`aria-hidden="true"`, and neither takes a `title` prop) — see each
//! caller's own doc for why: the word belongs to a sibling element the caller
//! controls, using the same clip-not-remove `.visually-hidden` pattern
//! `status::StatusBadgeView` uses for a status word beside a color-only dot.
//! A caller that forgot to add it would render a locality mark with nothing
//! for a screen reader to read, so REMEMBER to pair the icon with the word
//! rather than reaching for the icon alone.
//!
//! The locality pair was the first two glyphs; the harness marks, permission
//! locks, ended-status shapes, and qualifier joined later under the same
//! inline rule. The module doc for `list::shared::HostLocality` is where a
//! future alias or tooltip feature (TODO.md's "host aliases" entry, which
//! reuses this same title-line slot) would extend what appears beside these
//! icons, not this file's shape.
//!
//! Both roots also carry `data-glyph="local"`/`"remote"` — the shared
//! `host-kind-icon` class sizes and positions either glyph identically, so
//! nothing in the DOM otherwise distinguishes which SHAPE actually
//! rendered. Without a per-glyph marker, a bug that swapped the two
//! components (or matched a locality to the wrong one) would render the
//! wrong picture while every existing assertion — icon count, the sibling
//! hidden word, `data-host-locality` — still passed, because those check
//! that A glyph rendered and that its accessible word agrees with the row's
//! own verdict, never that the SPECIFIC svg shape did. `data-glyph` is
//! this module's own answer, independent of the caller's separately
//! spoken word, so a test can pin the actual glyph rather than trusting it
//! by association.

use dioxus::prelude::*;

/// The harness identity a sidebar badge can establish without trusting a
/// profile name. `Terminal` is deliberately the fallback: it says that the
/// stored command exists without pretending Farhelm knows what runs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HarnessGlyph {
    Cursor,
    Codex,
    Claude,
    Muse,
    Goose,
    Pi,
    /// OMP — the Pi fork's terminal command. Carries OMP's own block-pi
    /// mark, which is visibly a different drawing from Pi's pixel-grid "P"
    /// even at the smallest supported row width.
    Omp,
    Grok,
    OpenCode,
    Terminal,
}

/// The compact mark that qualifies a recognized harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PermissionGlyph {
    Yolo,
    FullAuto,
    Approve,
    SmartApprove,
    Chat,
}

/// An ended session's distinct sidebar silhouette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndedGlyph {
    Stopped,
    Exited,
    Interrupted,
    Error,
}

/// A state qualifier that compact mode must preserve without adding words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QualifierGlyph {
    Stale,
}

// ===== Harness marks ==================================================
//
// Every harness mark is a filled or stroked path normalized into the shared
// 12-unit box by ONE rule: the drawing's native bounding box is scaled so its
// longer side spans 10 of the 12 units and then centred, leaving a one-unit
// margin so the mark is optically the same weight as the lock beside it. The
// `transform` strings below are the precomputed result of that rule for each
// mark's native coordinates, so the row's marks agree on size by construction
// rather than by eye. The rule, the provenance of every geometry, and the
// brand constraints that shaped the choices are in `docs/harness-marks.md`;
// read that before adding, replacing, or "tidying" a mark here.
//
// Official geometry is used UNALTERED apart from that uniform scale — several
// of the brand terms forbid modification, and keeping the path bytes identical
// to the pinned upstream file is what lets `THIRD_PARTY_NOTICES.md` cite a
// commit rather than describe a derivative. Never render a mark as an SVG
// `text` element: the earlier letter marks did, and a capital at font-size 9
// came out roughly three quarters the height of the drawn marks beside it.

/// Farhelm's own Claude mark: an eight-ray tapered starburst, generated as
/// eight triangles from a 2.2-unit-wide base at the centre of a 24-unit box
/// to a tip 11 units out. It deliberately is NOT the Anthropic spark:
/// Anthropic's trademark guidelines grant no self-serve right to display
/// the logo and forbid recolouring it (see `docs/harness-marks.md`), so
/// the sidebar evokes "spark" without copying the mark.
const CLAUDE_STARBURST: &str = "M14.20 12.00L12.00 1.00L9.80 12.00ZM13.56 13.56L19.78 4.22L10.44 10.44ZM12.00 14.20L23.00 12.00L12.00 9.80ZM10.44 13.56L19.78 19.78L13.56 10.44ZM9.80 12.00L12.00 23.00L14.20 12.00ZM10.44 10.44L4.22 19.78L13.56 13.56ZM12.00 9.80L1.00 12.00L12.00 14.20ZM13.56 10.44L4.22 4.22L10.44 13.56Z";

/// OpenAI's monochrome "blossom", the mark Codex ships under everywhere
/// (there is no Codex-specific logo). Bytes of the `d` attribute of
/// `OpenAI-black-monoblossom.svg` from OpenAI's official brand download,
/// unaltered; OpenAI's brand terms permit referential use but forbid
/// modification, which is why the 2.5 KB path lives in its own file
/// rather than being simplified for this size.
const OPENAI_BLOSSOM: &str = include_str!("icons/openai-blossom.svgpath");

/// Cursor's official 2D cube (`CUBE_2D_LIGHT.svg` from cursor.com/brand),
/// unaltered. Cursor's own site renders this same geometry in
/// `currentColor`, so a monochrome rendering matches the owner's use.
const CURSOR_CUBE: &str = "M457.43,125.94L244.42,2.96c-6.84-3.95-15.28-3.95-22.12,0L9.3,125.94c-5.75,3.32-9.3,9.46-9.3,16.11v247.99c0,6.65,3.55,12.79,9.3,16.11l213.01,122.98c6.84,3.95,15.28,3.95,22.12,0l213.01-122.98c5.75-3.32,9.3-9.46,9.3-16.11v-247.99c0-6.65-3.55-12.79-9.3-16.11h-.01ZM444.05,151.99l-205.63,356.16c-1.39,2.4-5.06,1.42-5.06-1.36v-233.21c0-4.66-2.49-8.97-6.53-11.31L24.87,145.67c-2.4-1.39-1.42-5.06,1.36-5.06h411.26c5.84,0,9.49,6.33,6.57,11.39h-.01Z";

/// Goose's official glyph, the 24-unit path from the project's own
/// `Goose.tsx` icon component (Apache-2.0), unaltered. A detailed goose
/// silhouette; the project's own smallest use is 24px, so at sidebar size it
/// reads as a bird-shaped mark rather than a goose. Kept because it is the
/// mark and it is permitted, not because it is crisp.
const GOOSE_GLYPH: &str = include_str!("icons/goose.svgpath");

/// Pi's official badge mark from the pi.dev press kit: a 4x4 pixel grid
/// "P" with a detached dot. Every coordinate is a multiple of 140 in a
/// 560 box, so the normalized mark lands on 2.5-unit cells exactly.
const PI_BADGE: &str =
    "M420 280H280V140H0V0H420V280ZM560 560H420V280H560V560ZM140 560H0V140H140V280H280V420H140V560Z";

/// OMP's official mark, the block-style pi from omp.sh's favicon, with the
/// favicon's background tile dropped. Three axis-aligned rectangles, so it
/// survives 12px with no anti-aliasing fuzz. This replaced an earlier
/// Greek omega that was Farhelm's own invention.
const OMP_PI: &str = "M14 16h36v8H40v32h-8V24h-6v22h-8V24h-4z";

/// Farhelm's own Grok mark: a block "G" drawn as one path. xAI does not
/// publish a self-serve brand kit for this use, so the sidebar names the
/// harness without copying or approximating xAI's mark.
const GROK_GLYPH: &str = "M21 4H8C4.686 4 2 6.686 2 10V14C2 17.314 4.686 20 8 20H21V11H12V15H16V16H8C6.895 16 6 15.105 6 14V10C6 8.895 6.895 8 8 8H21Z";

/// OpenCode's favicon cut: one ring drawn with the even-odd rule. The
/// project's logo file is a two-tone pair of nested frames whose inner
/// mid-grey square collapses under `currentColor`; the favicon is the
/// small-size simplification the project itself publishes.
const OPENCODE_RING: &str = "M384 416H128V96H384V416ZM320 160H192V352H320V160Z";

/// Muse Code's mark is borrowed from an unofficial community VS Code
/// extension (MIT), because Meta publishes no Muse Code mark at all and
/// Meta's own corporate marks require brand approval for any use. It is a
/// stroked "M" whose four terminals carry round dots; the source icon's
/// blue and its sparkle are dropped. Stroked rather than filled, so the
/// stroke widths are the source's own and scale with the group transform.
const MUSE_STEM: &str = "M4 20V5l6 9 6-9v15";
const MUSE_DOTS: &str = "M4 20h0M16 20h0M4 5h0M16 5h0";

/// Draw the harness mark used in the sidebar's agent track.
///
/// Which harnesses carry an official mark, which carry one Farhelm drew, and
/// why, is documented per constant above and in `docs/harness-marks.md`; the
/// attributions for the vendored geometry are in `THIRD_PARTY_NOTICES.md`.
/// Everything stays inline so the web and desktop bundles cannot drift and
/// so no client downloads branding at runtime.
#[component]
pub(crate) fn HarnessIcon(glyph: HarnessGlyph) -> Element {
    let token = match glyph {
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
    };
    // Each transform is the normalization rule applied to the mark's native
    // bounding box; the box is recorded beside it so a reader can check the
    // arithmetic (tx = (12 - w*s)/2 - x*s, likewise for y, s = 10/max(w,h)).
    rsx! {
        svg {
            class: "sidebar-glyph harness-glyph",
            "data-glyph": "{token}",
            view_box: "0 0 12 12",
            "aria-hidden": "true",
            match glyph {
                // bbox 1,1 22x22 in a 24 box
                HarnessGlyph::Claude => rsx! { g { transform: "translate(0.5455 0.5455) scale(0.454545)", path { d: CLAUDE_STARBURST, fill: "currentColor" } } },
                // bbox 118.557,119.958 484.139x479.818 in a 721 box
                HarnessGlyph::Codex => rsx! { g { transform: "translate(-1.4488 -1.4331) scale(0.020655)", path { d: OPENAI_BLOSSOM, fill: "currentColor" } } },
                // bbox 0,0 466.74x532.095
                HarnessGlyph::Cursor => rsx! { g { transform: "translate(1.6141 1.0000) scale(0.018794)", path { d: CURSOR_CUBE, fill: "currentColor" } } },
                // bbox 4.328,3.789 16.628x16.628 in a 24 box
                HarnessGlyph::Goose => rsx! { g { transform: "translate(-1.6030 -1.2788) scale(0.601409)", path { d: GOOSE_GLYPH, fill: "currentColor" } } },
                // bbox 0,0 560x560
                HarnessGlyph::Pi => rsx! { g { transform: "translate(1.0000 1.0000) scale(0.017857)", path { d: PI_BADGE, fill: "currentColor" } } },
                // bbox 14,16 36x40 in a 64 box
                HarnessGlyph::Omp => rsx! { g { transform: "translate(-2.0000 -3.0000) scale(0.250000)", path { d: OMP_PI, fill: "currentColor" } } },
                // bbox 2,4 19x16 in a 24 box
                HarnessGlyph::Grok => rsx! { g { transform: "translate(-0.0526 -0.3158) scale(0.526316)", path { d: GROK_GLYPH, fill: "currentColor" } } },
                // bbox 128,96 256x320 in a 512 box
                HarnessGlyph::OpenCode => rsx! { g { transform: "translate(-2.0000 -2.0000) scale(0.031250)", path { d: OPENCODE_RING, fill: "currentColor", fill_rule: "evenodd" } } },
                // Stroke geometry: stem stroke 2.6 and dot stroke 4.2 with round
                // caps give a bbox of 1.9,2.9 16.2x19.2 in the source's 24 box.
                HarnessGlyph::Muse => rsx! {
                    g { transform: "translate(0.7917 -0.5104) scale(0.520833)", fill: "none", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: MUSE_STEM, stroke_width: "2.6" }
                        path { d: MUSE_DOTS, stroke_width: "4.2" }
                    }
                },
                HarnessGlyph::Terminal => rsx! { path { d: "M1.5 2h9v8h-9zM3.1 4l1.5 1.5L3.1 7M6 7h2.5", fill: "none", stroke: "currentColor", stroke_width: "1.2", stroke_linecap: "round", stroke_linejoin: "round" } },
            }
        }
    }
}

/// Draw the permission mark beside a known harness without relying on color.
#[component]
pub(crate) fn PermissionIcon(glyph: PermissionGlyph) -> Element {
    let (token, closed) = match glyph {
        PermissionGlyph::Yolo => ("yolo", false),
        PermissionGlyph::FullAuto => ("full-auto", true),
        PermissionGlyph::Approve => ("approve", true),
        PermissionGlyph::SmartApprove => ("smart-approve", true),
        PermissionGlyph::Chat => ("chat", true),
    };
    rsx! {
        svg { class: "sidebar-glyph permission-glyph", "data-glyph": "{token}", view_box: "0 0 12 12", fill: "none", stroke: "currentColor", stroke_width: "1.25", stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true",
            path { d: "M2.5 5.2h7v4.6h-7z" }
            if closed {
                path { d: "M4 5.2V3.8a2 2 0 0 1 4 0v1.4" }
            } else {
                path { d: "M4 5.2V3.8a2 2 0 0 1 3.4-1.4" }
            }
        }
    }
}

/// Draw an ended-status shape in the leading slot compact rows already own.
///
/// Stopped is a filled square inside a rounded-square outline — a bare
/// filled square at this size reads as an undifferentiated light mark, and
/// the dark gap between the 3-CSS-px center and the 1-CSS-px ring is what
/// keeps the two separable. Do not enlarge the center or thicken the ring:
/// either would consume the gap the symbol depends on. Keep the center
/// square, without rounded corners, to preserve the chosen stop silhouette.
/// The closed rectilinear silhouette is also what separates
/// stopped from the exited arrow, interrupted X, and error triangle; the
/// caller's tooltip and accessible "stopped by user" wording stay
/// authoritative either way, since the glyph alone can also read as a
/// small button.
#[component]
pub(crate) fn EndedStatusIcon(glyph: EndedGlyph) -> Element {
    let token = match glyph {
        EndedGlyph::Stopped => "stopped",
        EndedGlyph::Exited => "exited",
        EndedGlyph::Interrupted => "interrupted",
        EndedGlyph::Error => "error",
    };
    rsx! {
        svg { class: "sidebar-glyph ended-status-glyph", "data-glyph": "{token}", view_box: "0 0 12 12", fill: "none", stroke: "currentColor", stroke_width: "1.2", stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true",
            match glyph {
                EndedGlyph::Stopped => rsx! {
                    rect { x: "2", y: "2", width: "8", height: "8", rx: "2" }
                    rect { x: "4.2", y: "4.2", width: "3.6", height: "3.6", fill: "currentColor", stroke: "none" }
                },
                EndedGlyph::Exited => rsx! { path { d: "M2 2.5h4v7H2zM6 6h4M8.5 4.2 10.3 6 8.5 7.8" } },
                EndedGlyph::Interrupted => rsx! { path { d: "M2.2 3.2 3.2 2.2l6.6 6.6-1 1zM8.8 2.2l1 1-6.6 6.6-1-1z" } },
                EndedGlyph::Error => rsx! { path { d: "M6 1.7 10.5 10h-9zM6 4.4v2.4M6 8.4h.01" } },
            }
        }
    }
}

/// Draw a compact qualifier while keeping the qualifier's complete text next
/// to this decorative icon in the accessibility tree.
#[component]
pub(crate) fn QualifierIcon(glyph: QualifierGlyph) -> Element {
    let token = match glyph {
        QualifierGlyph::Stale => "stale",
    };
    rsx! {
        svg { class: "sidebar-glyph qualifier-glyph", "data-glyph": "{token}", view_box: "0 0 12 12", fill: "none", stroke: "currentColor", stroke_width: "1.2", stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true",
            match glyph {
                QualifierGlyph::Stale => rsx! { path { d: "M6 2v4l2.5 1.5M6 10a4 4 0 1 0 0-8 4 4 0 0 0 0 8z" } },
            }
        }
    }
}

/// The local-session mark: a laptop silhouette.
///
/// An open screen over a wider base reads as a different OBJECT from the
/// remote cloud at a glance, not just a different arrangement of the same
/// lines — legibility at 12px depends on silhouette, not on a reader
/// parsing detail. The outline stays quiet beside the status dot, lock,
/// and title, and it carries the red local caution color without becoming
/// a solid red patch the way a filled screen would.
#[component]
pub(crate) fn LocalHostIcon() -> Element {
    rsx! {
        svg {
            class: "host-kind-icon",
            "data-glyph": "local",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.3",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            rect { x: "3", y: "3", width: "10", height: "6.5", rx: "1" }
            line { x1: "1.8", y1: "12", x2: "14.2", y2: "12", stroke_width: "2" }
        }
    }
}

/// The remote-session mark: a lobed cloud outline.
///
/// Deliberately a different silhouette family from the laptop's straight
/// lines — the two shapes differ in outline, not only in detail that gets
/// lost first as glyphs shrink. The stroke stays at the shared 1.3 weight
/// rather than a bolder one: a cloud is a secondary identity cue, and a
/// heavier or filled mark would compete with the live status dot. A cloud
/// can suggest internet hosting rather than an ordinary remote machine, so
/// the caller's locality label stays authoritative and the glyph never
/// stands alone.
#[component]
pub(crate) fn RemoteHostIcon() -> Element {
    rsx! {
        svg {
            class: "host-kind-icon",
            "data-glyph": "remote",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.3",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            path { d: "M4 12.5a2.5 2.5 0 0 1-0.3-4.98A3.7 3.7 0 0 1 11 6.3a2.6 2.6 0 0 1 1.2 4.9V12.5Z" }
        }
    }
}
