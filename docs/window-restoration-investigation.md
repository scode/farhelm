# Native window restoration investigation

NOTE: This records why the near-term window-restoration item remains open. It does not specify a new persistence design.

Farhelm creates its desktop window through `desktop_window()` in
[`crates/farhelm-ui/src/desktop.rs`](../crates/farhelm-ui/src/desktop.rs). The builder configures native chrome, but
nothing in the release app saves or restores its bounds or zoomed state. The window-root actions already call Tao's
maximize API; that controls the current window only.

The pinned Dioxus Desktop 0.7.10 does contain a window-state save and restore path in its
[`app.rs`](https://docs.rs/dioxus-desktop/0.7.10/src/dioxus_desktop/app.rs.html). It is behind `debug_assertions`, so
the shipped build does not use it. It saves position and size, but not maximized or zoomed state. Although its record
includes a monitor name, the restore path does not check that the saved bounds fit a connected display. Enabling that
debug path in a release would therefore leave both parts of the requested behavior unresolved.

The pinned Tao 0.34.8 exposes window geometry, the maximized flag, and connected monitors through its
[`Window` API](https://docs.rs/tao/0.34.8/tao/window/struct.Window.html). It does not provide a complete release-state
store. On macOS, AppKit has a native frame-autosave primitive, but Tao does not wrap it. Reaching it from this crate
would add a macOS native bridge, and frame autosave alone would still need separate zoom-state storage and a check
against current displays. Linux would need its own event capture and persistence path. The required combination is a
state lifecycle and display-validation implementation, beyond the small existing-facility change authorized for this
goal.

The TODO remains open. A later implementation should first choose and verify its macOS persistence boundary on a real
Mac, including a changed display layout and reopening from zoomed state, then add only the Linux behavior that its
window manager can support reliably. True fullscreen restoration remains outside the near-term requirement.
