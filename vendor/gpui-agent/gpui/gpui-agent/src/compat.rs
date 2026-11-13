// Modified for Farhelm: changed or added by the patch layers in vendor/patches/gpui/ (vendor/README.md, "GPUI").
//! The calls whose shape differs between Zed's GPUI and the `gpui-pre 0.3.9` snapshot that
//! gpui-kit 0.8 pins. The `gpui-pre-0-3-9` feature picks the older shape; `zed` and
//! `gpui-pre-0-3-12` share the newer one.

use gpui::{
    App, KeybindingKeystroke, Keystroke, Modifiers, Pixels, PlatformInput, Point, ScrollDelta,
};

#[cfg(not(feature = "gpui-pre-0-3-9"))]
pub fn parse_keystroke(source: &str, cx: &App) -> Result<Keystroke, String> {
    Keystroke::parse_with_os(source, cx.platform_info().os).map_err(|error| error.to_string())
}

#[cfg(feature = "gpui-pre-0-3-9")]
pub fn parse_keystroke(source: &str, _cx: &App) -> Result<Keystroke, String> {
    Keystroke::parse(source).map_err(|error| error.to_string())
}

#[cfg(not(feature = "gpui-pre-0-3-9"))]
pub fn unparse(keystroke: &KeybindingKeystroke, cx: &App) -> String {
    keystroke.unparse(cx.platform_info().os)
}

#[cfg(feature = "gpui-pre-0-3-9")]
pub fn unparse(keystroke: &KeybindingKeystroke, _cx: &App) -> String {
    keystroke.unparse()
}

#[cfg(not(feature = "gpui-pre-0-3-9"))]
pub fn scroll(position: Point<Pixels>, delta: ScrollDelta, modifiers: Modifiers) -> PlatformInput {
    PlatformInput::Scroll(gpui::ScrollEvent {
        position,
        delta,
        modifiers,
        ..Default::default()
    })
}

#[cfg(feature = "gpui-pre-0-3-9")]
pub fn scroll(position: Point<Pixels>, delta: ScrollDelta, modifiers: Modifiers) -> PlatformInput {
    PlatformInput::ScrollWheel(gpui::ScrollWheelEvent {
        position,
        delta,
        modifiers,
        ..Default::default()
    })
}
