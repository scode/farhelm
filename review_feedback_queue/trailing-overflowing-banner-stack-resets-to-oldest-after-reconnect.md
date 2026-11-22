# Overflowing notice banner stack resets to the oldest notice after reconnect or Take back

Reviewed commit: e3d03d5651e9f0b417514e145ce847a1e45fffe8

## TLDR

When you have enough undismissed notices to scroll the banner stack and the window reconnects or takes the helm back
while the latest banner was already present before the disconnect, the banner stack jumps back to the oldest notice at
the top, hiding the newest notice (such as the connection-lost warning) off the bottom.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/main_view.rs:187-244`.

`notice_banners` (`crates/gui/farhelm-gui/src/main_view.rs:439-442`) keeps its `ScrollHandle` via
`scroll_handle("window-notice-scroll".into(), window, cx)`, which wraps `Window::use_keyed_state`
(`crates/gui/farhelm-gui/src/components/mod.rs:550-555`, `vendor/gpui-pre/src/window.rs:4798-4816`). Keyed state
survives only across consecutive frames that render it. While the channel is `Reconnecting` or `Displaced`,
`MainView::render` (`main_view.rs:180-244`) takes the `else { inert(&state, cx) }` branch and does not call
`notice_banners`, so GPUI drops `"window-notice-scroll"`. However, `self.drawn_banners` (`main_view.rs:95, 191`) is NOT
cleared in the `else` branch. When the window returns to `Connected` with an overflowing stack whose last notice was
already in `self.drawn_banners` (for example, an undismissed `Notice::ConnectionLost` at the end of the stack when a
second disconnect/reconnect or `Displaced` + `Take back` occurs, where `notices::post(Notice::ConnectionLost, cx)` at
`session.rs:221` is a no-op because `ConnectionLost` is already in `notices.posted`), `reveal_latest`
(`main_view.rs:188-190`) evaluates to `false`, and the freshly created `ScrollHandle` opens at `y = 0` (the top, oldest
banner) instead of keeping the latest banners in view. Fix direction: clear `self.drawn_banners` in the `else` (`inert`)
branch of `MainView::render` (or hold the `ScrollHandle` on `MainView` itself and clear/reveal appropriately on
reconnect).

Reviewer metadata (g20-uploads-notices-main-view/f2): suggested bucket: other (could be in `highest`: no — no security
or data-loss impact, requires an overflowing banner stack plus a second disconnect/take-back with the last banner still
undismissed); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g20-uploads-notices-main-view/f2`.
