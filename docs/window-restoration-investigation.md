# Native window restoration investigation

NOTE: This records the native API limits that shape Farhelm's release persistence behavior.

Farhelm creates its desktop window through `desktop_window()` in
[`crates/farhelm-ui/src/desktop.rs`](../crates/farhelm-ui/src/desktop.rs). The builder configures native chrome, while
the release app stores a versioned geometry snapshot under its desktop state directory. It restores a validated normal
rectangle and maximized flag before the webview is created, and flushes the final snapshot on close. The window-root
actions already call Tao's maximize API; persistence observes that state without changing the interaction.

The pinned Dioxus Desktop 0.7.10 does contain a window-state save and restore path in its
[`app.rs`](https://docs.rs/dioxus-desktop/0.7.10/src/dioxus_desktop/app.rs.html). It is behind `debug_assertions`, so
the shipped build does not use it. It saves position and size, but not maximized or zoomed state. Although its record
includes a monitor name, the restore path does not check that the saved bounds fit a connected display. Enabling that
debug path in a release would therefore leave both parts of the requested behavior unresolved.

The pinned Tao 0.34.8 exposes window geometry, the maximized flag, and connected monitors through its
[`Window` API](https://docs.rs/tao/0.34.8/tao/window/struct.Window.html). It does not provide a complete release-state
store. On macOS, AppKit has a native frame-autosave primitive, but Tao does not wrap it. Reaching it from this crate
would add a macOS native bridge, and frame autosave alone would still need separate maximized-state storage and a check
against current displays. Farhelm instead uses Dioxus's public window and event hooks, one small state file, and Tao's
monitor data on both desktop platforms.

The persisted unit is Tao's physical-pixel outer-frame geometry. A saved rectangle must fit entirely on a connected
monitor; otherwise Farhelm centers a bounded fallback on the primary display. Missing state retains Dioxus's first-run
size. Wayland cannot provide a trustworthy global position, so Farhelm restores a usable saved size and leaves placement
to the compositor; move events do not replace the remembered position with `(0, 0)`. Tao exposes monitor rectangles
rather than work areas, so panels and docks are not measured separately. Restore converts the saved outer size to an
inner-size request using the current outer-minus-inner decoration extent. A real Mac check is still needed for
decoration and scaling details. Fullscreen observations are ignored for ordinary-frame persistence, and true fullscreen
restoration and webview zoom remain outside this behavior.
