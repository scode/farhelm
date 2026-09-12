//! The macOS window shares its top edge with the application's own header.
//!
//! Native traffic lights remain AppKit-owned. Only the desktop macOS build
//! reserves their space and exposes a drag handle. Other builds retain the
//! same inert, hidden spacer so browser geometry tests can exercise production
//! markup without enabling native interactions.

use dioxus::prelude::*;

/// Select the native header treatment without changing browser builds on a Mac.
///
/// The target OS alone is insufficient: the desktop feature distinguishes
/// native window chrome from the shared UI rendered in an ordinary browser.
pub(crate) const fn shell_class() -> &'static str {
    if cfg!(all(feature = "desktop", target_os = "macos")) {
        "app-shell macos-window"
    } else {
        "app-shell"
    }
}

/// Keep native chrome alive before authentication and across page-level errors.
///
/// Wry owns the content view for the window's lifetime, not the authenticated
/// sidebar's lifetime. The root class also reserves native-button space for
/// startup and mismatch messages, which render outside the ordinary shell.
#[component]
pub(crate) fn WindowFrame(children: Element) -> Element {
    #[cfg(all(feature = "desktop", target_os = "macos"))]
    {
        let window = dioxus::desktop::use_window();
        use_hook(move || {
            use dioxus::desktop::wry::WebViewExtMacOS;

            // Wry replaces Tao's content view. Its own view must retain the
            // inset so redraws after resizing do not restore the default
            // button positions over application controls.
            if let Err(error) = window
                .webview
                .set_traffic_light_inset(dioxus::desktop::LogicalPosition::new(12.0, 16.0))
            {
                tracing::warn!(%error, "could not position native window controls");
            }
        });
    }
    let native = cfg!(all(feature = "desktop", target_os = "macos"));
    rsx! {
        if native {
            // Bootstrap failures must receive layout before AppBody mounts
            // its ordinary stylesheets. The desktop asset handler is already
            // registered by App before this child renders.
            document::Link { rel: "stylesheet", href: crate::APP_CSS }
        }
        div {
            class: if native { "window-root macos-root" } else { "window-root" },
            {children}
        }
    }
}

/// Give empty sidebar-header space a native drag action, never its controls.
///
/// This separate, non-focusable element has no interactive descendants. A
/// parent-level mouse handler would also receive clicks from Profiles or the
/// version label, stealing normal control activation and text selection.
/// Dioxus's native drag helper ignores dragging while the window is fullscreen.
/// Other builds leave this spacer hidden and its event handler inert.
#[component]
pub(crate) fn WindowDragRegion() -> Element {
    rsx! {
        div {
            class: "window-drag-region",
            aria_hidden: "true",
            onmousedown: move |event: MouseEvent| {
                #[cfg(all(feature = "desktop", target_os = "macos"))]
                if event.trigger_button() == Some(dioxus::html::input_data::MouseButton::Primary) {
                    dioxus::desktop::window().drag();
                }
                #[cfg(not(all(feature = "desktop", target_os = "macos")))]
                let _ = event;
            },
        }
    }
}
