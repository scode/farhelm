//! Inline SVG glyphs for the sidebar's locality marks.
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
//! neither cost: the shape is markup, `currentColor` makes it follow the
//! row's own text color (dimmed on a stale row, inverted on a selected one)
//! with no rule of its own, and there is no file for the two build targets to
//! disagree about.
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

/// The local-session mark: a monitor on a stand.
///
/// Drawn as a single screen rather than the server shape below so the two
/// read as different OBJECTS at a glance, not just different arrangements of
/// the same lines — legibility at 12-14px depends on silhouette, not on a
/// reader parsing detail. `fill="none"` with a `currentColor` stroke keeps
/// the glyph a pure outline, which stays crisp at this size in both themes
/// without a fill weight to tune per background.
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
            rect { x: "2", y: "2", width: "12", height: "8", rx: "1" }
            line { x1: "8", y1: "10", x2: "8", y2: "12.5" }
            line { x1: "5", y1: "13", x2: "11", y2: "13" }
        }
    }
}

/// The remote-session mark: two stacked server units with activity lights.
///
/// Deliberately a HORIZONTAL, stacked silhouette against the local glyph's
/// single upright rectangle — the two shapes differ in outline, not only in
/// the fill-vs-stroke detail that gets lost first as glyphs shrink. The
/// lights are filled dots (`fill: currentColor`) rather than more outline,
/// which is what keeps the shape readable as "server rack" instead of
/// collapsing into two bars indistinguishable from the local glyph's screen
/// at 12px.
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
            rect { x: "2", y: "2", width: "12", height: "5", rx: "1" }
            rect { x: "2", y: "9", width: "12", height: "5", rx: "1" }
            circle { cx: "5", cy: "4.5", r: "0.6", fill: "currentColor", stroke: "none" }
            circle { cx: "5", cy: "11.5", r: "0.6", fill: "currentColor", stroke: "none" }
        }
    }
}
