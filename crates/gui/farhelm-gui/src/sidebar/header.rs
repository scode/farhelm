//! The app header: the row across the top of the sidebar with the wordmark at its left and the build's version at its
//! right (the old `.app-bar`, without the updater's controls, which are the desktop-integration area's).
//!
//! Geometry and colors are the old stylesheet's: at least 40 pixels high, 8 by 12 pixels of padding, the sidebar's
//! `--bg-1` with a `--border-row` hairline under it. The 48 pixels and the room for the window buttons the old app gave
//! it on macOS come with the macOS title band, so it is 40 pixels on both platforms for now.
//!
//! - **The wordmark** is the brand file itself (`assets::WORDMARK_DARK` or `WORDMARK_LIGHT`, the one for the current
//!   appearance's ground), drawn 18 pixels high in its own two colors, never shrinking: when the row is narrow the
//!   version gives way first, since a clipped mark reads as a rendering fault. It is named "farhelm" for assistive
//!   technology, as the file's own `aria-label` named it in the old app.
//! - **The version** is this window's build in the monospace face at 12 pixels in `--fg-1`, pushed to the row's right
//!   end and cut short with an ellipsis when it does not fit. Its hover help says what kind of build it is: a
//!   development build ("not a release") or a release by number. The old app showed the helm's build instead when the
//!   helm reported a different one, with hover help naming both; the GUI does not learn the helm's build yet, so that
//!   case waits (the parity file records its wording).

use farhelm_proto::{BUILD_VERSION, is_development_build};
use gpui::{
    App, InteractiveElement as _, IntoElement, ParentElement, StatefulInteractiveElement as _,
    Styled, div, img, px,
};

use crate::assets;
use crate::components::{HoverHelp, Role, StatusMark};
use crate::theme::{self, Palette};

/// `--sidebar-title-height` off macOS: the row's least height.
const HEIGHT: f32 = 40.;

/// `.app-bar`'s padding, vertical then horizontal.
const PAD: (f32, f32) = (8., 12.);

/// `.app-wordmark svg`'s height, and the width that keeps the brand file's 284 by 72 proportions at it.
const WORDMARK_HEIGHT: f32 = 18.;
const WORDMARK_WIDTH: f32 = 284. * WORDMARK_HEIGHT / 72.;

/// The header row.
pub(super) fn header(cx: &App) -> impl IntoElement {
    let palette = theme::palette(cx);
    div()
        .flex()
        .flex_row()
        .items_center()
        .flex_none()
        .min_h(px(HEIGHT))
        .py(px(PAD.0))
        .px(px(PAD.1))
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
                .child(
                    StatusMark::new(
                        "app-version",
                        BUILD_VERSION,
                        HoverHelp::tooltip(version_help(BUILD_VERSION)),
                        Role::Label,
                        BUILD_VERSION,
                    )
                    .monospace()
                    .truncate(),
                ),
        )
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
