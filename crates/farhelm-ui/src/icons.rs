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
//! Two glyphs today, sized and structured for more to join them: the module
//! doc for `list::shared::HostLocality` is where a future alias or
//! tooltip feature (TODO.md's "host aliases" entry, which reuses this same
//! title-line slot) would extend what appears beside these icons, not this
//! file's shape.
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
    Codex,
    Claude,
    Muse,
    Goose,
    Pi,
    /// OMP — the Pi fork's terminal command. Drawn as the Greek capital
    /// omega rather than a second letter mark: at sidebar size a Latin
    /// letter would sit beside Pi's "P" as two near-identical strokes, while
    /// the omega's open bowl reads as a different mark even at the smallest
    /// supported row width.
    Omp,
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

/// Draw the one-character harness mark used in the sidebar's agent track.
///
/// The C/M/L paths are Farhelm's interim letter marks. The OpenCode geometry
/// is the two-path mark from anomalyco/opencode commit
/// `e03db9bc6908f75c9334d8aa997deeaac81c0298`, licensed MIT, Copyright (c)
/// 2025 opencode. It stays inline so the web and desktop bundles cannot drift
/// and so no client downloads branding at runtime.
#[component]
pub(crate) fn HarnessIcon(glyph: HarnessGlyph) -> Element {
    let token = match glyph {
        HarnessGlyph::Codex => "codex",
        HarnessGlyph::Claude => "claude",
        HarnessGlyph::Muse => "muse",
        HarnessGlyph::Goose => "goose",
        HarnessGlyph::Pi => "pi",
        HarnessGlyph::Omp => "omp",
        HarnessGlyph::OpenCode => "opencode",
        HarnessGlyph::Terminal => "terminal",
    };
    rsx! {
        svg {
            class: "sidebar-glyph harness-glyph",
            "data-glyph": "{token}",
            view_box: "0 0 12 12",
            "aria-hidden": "true",
            match glyph {
                HarnessGlyph::Codex => rsx! { path { d: "M10 2.4A4.8 4.8 0 1 0 10 9.6L8.5 8.1A2.7 2.7 0 1 1 8.5 3.9Z", fill: "currentColor" } },
                HarnessGlyph::Claude => rsx! { path { d: "M2 2h2v6h6v2H2z", fill: "currentColor" } },
                HarnessGlyph::Muse => rsx! { path { d: "M1.2 10V2h1.9l2.9 4.6L8.9 2h1.9v8H9V5.2L6.8 8.7H5.2L3 5.2V10z", fill: "currentColor" } },
                HarnessGlyph::Goose => rsx! { text { x: "2", y: "9", fill: "currentColor", font_size: "9", "G" } },
                HarnessGlyph::Pi => rsx! { text { x: "3", y: "9", fill: "currentColor", font_size: "9", "P" } },
                HarnessGlyph::Omp => rsx! { text { x: "2", y: "9", fill: "currentColor", font_size: "9", "Ω" } },
                HarnessGlyph::OpenCode => rsx! {
                    // Source geometry uses a 240×300 canvas. A nested group
                    // preserves that ratio inside this common 12px glyph box.
                    g { transform: "scale(.05 .04)",
                        path { d: "M180 240H60V120H180V240Z", fill: "currentColor" }
                        path { d: "M180 60H60V240H180V60ZM240 300H0V0H240V300Z", fill: "currentColor", fill_rule: "evenodd" }
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
