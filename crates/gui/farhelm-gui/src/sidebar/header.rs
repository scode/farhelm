//! The app header: the row across the top of the sidebar with the wordmark at its left and the build's version at its
//! right (the old `.app-bar`). While the desktop updater runs, the version is its readout with the updater's controls
//! (`update_readout::view`); without one it is the plain build text below.
//!
//! Geometry and colors are the old stylesheet's: at least 40 pixels high, 8 by 12 pixels of padding, the sidebar's
//! `--bg-1` with a `--border-row` hairline under it. In the macOS window it is the app bar half of the window's title
//! band (`desktop::title_band`): 48 pixels high, with 88 pixels of left padding that keep the traffic lights clear
//! (none in native fullscreen, which hides them), and a press on its empty background (the wordmark included) drags the
//! window. The version claims its presses, as text and controls in the band do.
//!
//! - **The wordmark** is the brand file itself (`assets::WORDMARK_DARK` or `WORDMARK_LIGHT`, the one for the current
//!   appearance's ground), drawn 18 pixels high in its own two colors, never shrinking: when the row is narrow the
//!   version gives way first, since a clipped mark reads as a rendering fault. It is named "farhelm" for assistive
//!   technology, as the file's own `aria-label` named it in the old app.
//! - **The version** is this window's build in the monospace face at 12 pixels in `--fg-1`, pushed to the row's right
//!   end and cut short with an ellipsis when it does not fit. Its hover help says what kind of build it is: a
//!   development build ("not a release") or a release by number. The old app showed the helm's build instead when the
//!   helm reported a different one, with hover help naming both; this window does not learn the helm's build, so it
//!   always shows its own (the parity file records the deviation).

use farhelm_proto::{BUILD_VERSION, is_development_build};
use gpui::{
    App, Entity, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, Window, div, img, prelude::FluentBuilder as _, px,
};

use crate::assets;
use crate::components::{HoverHelp, Role, StatusMark};
use crate::desktop::title_band::{self, Row};
use crate::theme::{self, Palette};
use crate::update_readout::view::VersionReadout;

/// `--sidebar-title-height` off macOS: the row's least height.
const HEIGHT: f32 = 40.;

/// `.app-bar`'s padding, vertical then horizontal.
const PAD: (f32, f32) = (8., 12.);

/// `.app-wordmark svg`'s height, and the width that keeps the brand file's 284 by 72 proportions at it.
const WORDMARK_HEIGHT: f32 = 18.;
const WORDMARK_WIDTH: f32 = 284. * WORDMARK_HEIGHT / 72.;

/// The header row. `readout` is the version readout while the desktop updater runs; without one the version is the
/// plain build text.
pub(super) fn header(
    readout: Option<Entity<VersionReadout>>,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let palette = theme::palette(cx);
    let height = if title_band::ON {
        title_band::HEIGHT
    } else {
        px(HEIGHT)
    };
    let row = div()
        .debug_selector(|| "app-bar".into())
        .flex()
        .flex_row()
        .items_center()
        .flex_none()
        .min_h(height)
        .py(px(PAD.0))
        .px(px(PAD.1))
        .when_some(title_band::traffic_light_room(window), |row, room| {
            row.pl(room)
        })
        .bg(theme::color(palette.bg_1))
        .border_b_1()
        .border_color(theme::color(palette.border_row))
        .child(
            div()
                .id("app-wordmark")
                .role(Role::Image)
                .aria_label("farhelm")
                .flex_none()
                .child(
                    img(wordmark(palette))
                        .w(px(WORDMARK_WIDTH))
                        .h(px(WORDMARK_HEIGHT)),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_row()
                .justify_end()
                .child(match readout {
                    // Its controls keep their own presses; its status text claims them itself.
                    Some(readout) => div().min_w_0().child(readout).into_any_element(),
                    None => title_band::claims_presses(
                        div().min_w_0().child(
                            StatusMark::new(
                                "app-version",
                                BUILD_VERSION,
                                HoverHelp::tooltip(version_help(BUILD_VERSION)).below(),
                                Role::Label,
                                BUILD_VERSION,
                            )
                            .monospace()
                            .truncate(),
                        ),
                    )
                    .into_any_element(),
                }),
        );
    title_band::row(row, Row::AppBar)
}

/// The wordmark file for `palette`'s ground: light letters on a dark ground, dark letters on a light one.
pub(super) fn wordmark(palette: &Palette) -> &'static str {
    if palette.dark {
        assets::WORDMARK_DARK
    } else {
        assets::WORDMARK_LIGHT
    }
}

/// What the version readout's hover help says about the build `version`: the old app's words for a window with no
/// updater and no helm of another build.
pub(super) fn version_help(version: &str) -> String {
    if is_development_build(version) {
        "This is a development build of Farhelm, not a release.".to_owned()
    } else {
        format!("This is Farhelm {version}.")
    }
}
