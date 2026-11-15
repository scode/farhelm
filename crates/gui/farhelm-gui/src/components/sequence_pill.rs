//! The quick-launch sequence's amber pill and the candidate list under it: the old stylesheet's
//! `.launch-sequence-indicator`, `.launch-sequence-pill`, `.launch-sequence-chip`, `.launch-sequence-word` and
//! `.launch-sequence-candidates` at reference commit `24bc0580`.
//!
//! It is display only, and has to be: the layer manager draws it above every layer through `layers::show_pill`, and a
//! press on it must reach whatever it covers (SPEC.md, "Quick-launch sequences": "Neither the pill nor the list takes
//! focus or pointer input"). So nothing here asks for the pointer or holds focus: no listener, hover style, cursor,
//! hover help, scrolling or focus tracking (the NOTE on `layers::Pill` lists what would break that). That is also why
//! its constructor takes no hover help or focus handle, unlike a control's; like [`super::Banner`], it is a container
//! for text, not a control. Its role is the old indicator's `status`.
//!
//! The words are the feature's: the pill draws the label, chips, word, hint and candidates it is handed. Template names
//! are host-supplied text (an agent may have written a template's name), so they arrive as [`HostText`] and are drawn
//! through the host-text element, each its own text element. The typed word is the user's own, drawn the same way so
//! that an invisible character in it shows escaped, as it would in the names it is matched against.
//!
//! NOTE: The old pill came in with a 200 ms fade and scale (`launch-sequence-enter`), off under reduced motion. It is
//! not drawn here: GPUI's one-shot animations are banned throughout the GUI crates (`crates/gui/clippy.toml`), the
//! component layer included, and only an owner listed in the foundation ownership table's lint exceptions may switch a
//! ban off. The pill appears at once until that exception exists; the entrance would wrap the element [`SequencePill`]
//! renders.

use gpui::{
    App, BoxShadow, Div, InteractiveElement as _, IntoElement, ParentElement, Pixels, RenderOnce,
    Role, SharedString, StatefulInteractiveElement as _, Styled, Window, div, point,
    prelude::FluentBuilder as _, px,
};

use crate::host_text::{self, HostText};
use crate::layers;
use crate::theme::{self, metrics};

/// `.launch-sequence-indicator`'s `max-width: calc(100% - 28px)`: the pill keeps this far from each side of the area it
/// sits in.
const SIDE_CLEARANCE: f32 = 14.;
/// `.launch-sequence-pill`: the gap between its parts and its padding (10 pixels by 18).
const PILL_GAP: f32 = 6.;
const PILL_PAD: (f32, f32) = (10., 18.);
/// `--radius-launch-sequence`: a single line still reads as a capsule, and a wrapped hint keeps its text inside the
/// border instead of under a tall semicircle (`min(var(--radius-pill), var(--radius-launch-sequence))`).
const PILL_RADIUS: f32 = 24.;
/// The halo: `0 0 24px` of `--warn` at 28%, drawn under the menu shadow the old pill also had.
const HALO_BLUR: f32 = 24.;
const HALO_ALPHA: f32 = 0.28;
/// `.launch-sequence-chip`: a border of `--warn` at 50% and 1 pixel by 5 of padding.
const CHIP_BORDER_ALPHA: f32 = 0.5;
const CHIP_PAD: (f32, f32) = (1., 5.);
/// `.launch-sequence-candidates`: 6 pixels below the pill, gaps of 6 by 12 between names, 6 by 10 of padding.
const CANDIDATES_MARGIN: f32 = 6.;
const CANDIDATES_GAP: (f32, f32) = (6., 12.);
const CANDIDATES_PAD: (f32, f32) = (6., 10.);
/// `.launch-sequence-indicator[data-in-launcher="true"]`'s `bottom: 14px`: the gap the bottom placement keeps above the
/// area's bottom edge.
const BOTTOM_GAP: f32 = 14.;

/// One queued entry as the pill shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SequenceChip {
    /// A template's name or a typed word, drawn as host text.
    Name(HostText),
    /// The app's own words for an entry with no name ("Unbound key 3").
    Words(SharedString),
}

/// The sequence pill: `label`, the queued chips, the word in progress and `hint` on one amber capsule that wraps, and
/// the candidate names in a box below it. Laid out for the layer manager's pill slot, which centers the element over
/// its anchor (or sets it on the anchor's bottom edge) in a box the anchor's size.
#[derive(IntoElement)]
pub struct SequencePill {
    label: SharedString,
    chips: Vec<SequenceChip>,
    word: Option<HostText>,
    hint: SharedString,
    candidates: Vec<HostText>,
    at_bottom: bool,
    lowered: Pixels,
}

impl SequencePill {
    /// A pill that starts with `label` ("Launch sequence: ") and ends with `hint`, with nothing queued yet.
    pub fn new(label: impl Into<SharedString>, hint: impl Into<SharedString>) -> Self {
        SequencePill {
            label: label.into(),
            chips: Vec::new(),
            word: None,
            hint: hint.into(),
            candidates: Vec::new(),
            at_bottom: false,
            lowered: px(0.),
        }
    }

    /// Add a queued entry's chip, after the ones added before it.
    pub fn chip(mut self, chip: SequenceChip) -> Self {
        self.chips.push(chip);
        self
    }

    /// The word in progress, after the chips, in the monospace face. Leave it out while no word is in progress: an
    /// empty value would still draw its part.
    pub fn word(mut self, word: HostText) -> Self {
        self.word = Some(word);
        self
    }

    /// The candidate names, in order, in the box under the pill; none draws no box.
    pub fn candidates(mut self, names: impl IntoIterator<Item = HostText>) -> Self {
        self.candidates.extend(names);
        self
    }

    /// Lay the pill out for the bottom placement the layer manager uses while the launcher is open: the old
    /// launcher-context rules, with the candidates in the flow above the pill's bottom gap rather than hanging below it,
    /// so the whole group stays clear of the area's bottom edge.
    pub fn at_bottom(mut self) -> Self {
        self.at_bottom = true;
        self
    }

    /// Move the centered pill's center down by `by` (half of a top padding it adds): the old Mac window's correction for
    /// its taller title band, so the pill stays centered on the terminal rather than on the whole main area.
    pub fn lowered(mut self, by: Pixels) -> Self {
        self.lowered = by;
        self
    }
}

impl RenderOnce for SequencePill {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let warn = theme::color(palette.warn);
        let fill = theme::color(palette.warn_bg);
        let mut halo = vec![BoxShadow {
            color: warn.opacity(HALO_ALPHA),
            offset: point(px(0.), px(0.)),
            blur_radius: px(HALO_BLUR),
            spread_radius: px(0.),
            inset: false,
        }];
        halo.extend(layers::menu_shadow(palette));
        let chip = |chip: SequenceChip| {
            let content = match chip {
                SequenceChip::Name(name) => host_text::element(&name),
                SequenceChip::Words(words) => div().child(words),
            };
            content
                .border_1()
                .border_color(warn.opacity(CHIP_BORDER_ALPHA))
                .rounded(metrics::RADIUS_SM)
                .py(px(CHIP_PAD.0))
                .px(px(CHIP_PAD.1))
        };
        let pill =
            div()
                .debug_selector(|| "sequence-capsule".into())
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .justify_center()
                .gap(px(PILL_GAP))
                .py(px(PILL_PAD.0))
                .px(px(PILL_PAD.1))
                .border_1()
                .border_color(warn)
                .rounded(px(PILL_RADIUS))
                .bg(fill)
                .shadow(halo)
                .child(div().child(self.label))
                .children(self.chips.into_iter().map(chip))
                .children(self.word.map(|word| {
                    host_text::element(&word).font_family(crate::assets::TERMINAL_FONT)
                }))
                .child(div().child(self.hint));
        let candidates = (!self.candidates.is_empty()).then(|| {
            div()
                .debug_selector(|| "sequence-candidates".into())
                .flex()
                .flex_row()
                .flex_wrap()
                .justify_center()
                .gap_y(px(CANDIDATES_GAP.0))
                .gap_x(px(CANDIDATES_GAP.1))
                .max_w_full()
                .py(px(CANDIDATES_PAD.0))
                .px(px(CANDIDATES_PAD.1))
                .border_1()
                .border_color(warn)
                .rounded(metrics::RADIUS_MD)
                .bg(fill)
                .font_family(crate::assets::TERMINAL_FONT)
                .children(self.candidates.iter().map(host_text::element))
        });
        let group: Div = if self.at_bottom {
            div()
                .flex()
                .flex_col()
                .items_center()
                .child(pill)
                .children(candidates.map(|names| names.mt(px(CANDIDATES_MARGIN))))
        } else {
            // On the page the names hang below the pill, out of the flow, so the pill keeps its place as they come and
            // go (`position: absolute; top: 100%` in the old sheet), at most as wide as the pill and centered under it.
            div()
                .relative()
                .child(pill)
                .children(candidates.map(|names| {
                    div()
                        .absolute()
                        .top_full()
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .pt(px(CANDIDATES_MARGIN))
                        .child(names)
                }))
        };
        div()
            .id("sequence-pill")
            .debug_selector(|| "sequence-pill".into())
            .role(Role::Status)
            .px(px(SIDE_CLEARANCE))
            .pt(self.lowered * 2.)
            .when(self.at_bottom, |frame| frame.pb(px(BOTTOM_GAP)))
            .text_color(warn)
            .text_size(metrics::TEXT_MD)
            .child(group)
    }
}
