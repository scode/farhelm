//! The macOS window shares its top edge with the application's own header.
//!
//! Native traffic lights remain AppKit-owned. Only the desktop macOS build
//! reserves their space and turns the window's top band into a title bar:
//! the band's drag surfaces (the sidebar's app bar, the session top row, and
//! a stand-in strip when no session is open) drag the window from empty
//! background and zoom it on a double-click. Which presses count as empty
//! background is decided per press by `assets/click-detail.js`; this module
//! decides what to do with that verdict and starts the native drag. Other
//! builds render the same markup with inert handlers, so browser tests can
//! exercise production markup without enabling native interactions.

use dioxus::prelude::*;

/// Select the native header treatment without changing browser builds on a Mac.
///
/// The target OS alone is insufficient: the desktop feature distinguishes
/// native window chrome from the shared UI rendered in an ordinary browser.
pub(crate) const fn shell_class() -> &'static str {
    if cfg!(all(native_desktop, target_os = "macos")) {
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
    #[cfg(all(native_desktop, target_os = "macos"))]
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
        // Window lifetime, not page lifetime: this root never unmounts, so
        // the bridge route registered here stays until the window closes.
        use_click_detail_bridge();
    }
    let native = cfg!(all(native_desktop, target_os = "macos"));
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

/// What one press on a drag surface does. Exactly one variant runs per
/// press; no press both drags and toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// The decision helpers below are called only from the macOS desktop
// press path, but stay compiled everywhere so Linux unit tests cover
// the truth table; outside that path they are test-only by design.
#[cfg_attr(not(all(native_desktop, target_os = "macos")), allow(dead_code))]
pub(crate) enum PressAction {
    /// AppKit classified this press as a repeat click: zoom/restore once.
    ToggleMaximized,
    /// An ordinary press: the existing native drag.
    Drag,
    /// No native action: a press on a control or text, a missing report, a
    /// non-primary button, a guarded toggle, or a cross-target repeat (no
    /// zoom AND no drag — see `decide_press`).
    None,
}

/// One bridge report: the press's DOM click count, whether it may zoom, and
/// whether it landed on empty background.
///
/// `eligible` is the DOM side's targeting verdict: the immediately
/// preceding primary press landed on the background of the same connected
/// surface, with no intervening press elsewhere, non-primary press, control
/// press, or surface replacement. The native count classifies timing (fast
/// enough to be a repeat); eligibility classifies targeting (both presses
/// belong here). The fields arrive in one header value so they cannot split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(all(native_desktop, target_os = "macos")), allow(dead_code))]
pub(crate) struct ClickReport {
    pub detail: i32,
    pub eligible: bool,
    /// Whether the press landed on empty surface background rather than a
    /// control, text, or anything else with interactions of its own. Only
    /// such presses drag or zoom; the surface's handler receives every
    /// press inside it, controls included, because events bubble.
    pub drag: bool,
}

/// Decide drag versus maximize-toggle from the bridged report.
///
/// `report` is what the click-detail bridge POSTed for THIS press (`None`
/// when no POST preceded the event — the script missing, the route
/// unregistered, or a malformed report). Only a press the report places on
/// empty background may act. `None` therefore means no action: a surface
/// also contains controls and text whose presses reach the same handler, so
/// without a verdict a press cannot be told apart from a button press, and
/// moving the window from under a button is worse than a missed drag. (The
/// single-spacer design this replaced could treat `None` as a drag, because
/// nothing but empty space was ever under that handler.) A count stuck at 1
/// still degrades to ordinary dragging. The slot's own docs name the
/// residual partial-transport case.
///
/// A repeat press toggles only with an eligible predecessor — double,
/// triple, rapid pairs alike — and no repeat press ever drags, so
/// triple-clicking cannot fling the window. A repeat WITHOUT one (a
/// cross-target pair: header text or a control, then background) does
/// nothing at all, deliberately: no zoom AND no drag, so a foreign
/// press can neither zoom the window nor start moving it. The press
/// still counts as the eligible predecessor for the NEXT press, so two
/// consecutive background presses always form a valid pair whatever came
/// before them. Fullscreen suppresses the toggle explicitly:
/// `toggle_maximized` has no fullscreen check of its own (unlike
/// [`start_native_drag`]), and Tao defers a fullscreen maximize change until exit,
/// which would corrupt the restored frame. A guarded toggle does
/// nothing at all, not even a drag.
///
/// The count itself rests on an assumption, stated plainly: WebKit is
/// verified to propagate an incoming native count to DOM `detail`, but
/// AppKit assigning 2 to the press after a drag-consumed release is
/// unobserved here and awaits the manual Mac checklist.
///
/// Pure and platform-free so the truth table is unit-tested on Linux; the
/// macOS call site supplies the three inputs and performs the action.
#[cfg_attr(not(all(native_desktop, target_os = "macos")), allow(dead_code))]
pub(crate) fn decide_press(
    primary: bool,
    report: Option<ClickReport>,
    fullscreen: bool,
) -> PressAction {
    if !primary {
        return PressAction::None;
    }
    // A surface contains controls and text too, so a press with no report
    // cannot be told apart from a button press and must not move the window.
    let Some(report) = report.filter(|report| report.drag) else {
        return PressAction::None;
    };
    if report.detail < 2 {
        return PressAction::Drag;
    }
    // A cross-target repeat is neither a zoom nor a drag: its pair partner
    // was not on this surface's background, so this press starts no gesture.
    if !report.eligible || fullscreen {
        return PressAction::None;
    }
    PressAction::ToggleMaximized
}

/// Read the bridge report the header carries.
///
/// The header value is `<detail>:<eligible>:<drag>` — one DOM `detail` as
/// ASCII digits, then `1` or `0` for each flag. Anything else — absent, unparseable, negative,
/// a field other than `0`/`1` — is `None`, which decides as no action.
#[cfg_attr(not(all(native_desktop, target_os = "macos")), allow(dead_code))]
pub(crate) fn parse_click_report(value: &str) -> Option<ClickReport> {
    let mut fields = value.trim().split(':');
    let (Some(detail), Some(eligible), Some(drag), None) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return None;
    };
    let drag = match drag.trim() {
        "1" => true,
        "0" => false,
        _ => return None,
    };
    let detail = detail
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|count| *count >= 0)?;
    let eligible = match eligible.trim() {
        "1" => true,
        "0" => false,
        _ => return None,
    };
    Some(ClickReport {
        detail,
        eligible,
        drag,
    })
}

/// First path segment the click-detail bridge claims.
///
/// Dioxus routes a request to the asset handler registered under its
/// first path segment (`protocol.rs`), so this must not collide with the
/// `assets` route or the exact-matched `__events`/`__file_dialog` paths.
/// The JS half derives the same URL from the interpreter's events path.
#[cfg(all(native_desktop, target_os = "macos"))]
const CLICK_DETAIL_ROUTE: &str = "fh-click-detail";

/// The header carrying one press's bridge report.
///
/// A header rather than a POST body, mirroring the interpreter's own
/// event send (whose bodies live in headers as an Android workaround):
/// headers provably arrive — every `__events` send depends on one —
/// which is more than this bridge claims to know about custom-scheme
/// POST bodies on macOS.
#[cfg(all(native_desktop, target_os = "macos"))]
const CLICK_DETAIL_HEADER: &str = "x-fh-click-detail";

/// The latest bridge report, awaiting its own mousedown event.
///
/// Written by the bridge route handler, consumed by the drag surface's
/// onmousedown ([`window_surface_press`]). Pairing is by ORDER, and the ordering is structural:
/// the page's capture listener POSTs synchronously before the
/// interpreter's bubble-phase send for the same press, and both are
/// synchronous XHRs on the page's one JS thread — so each POST is fully
/// answered before its press's event send even starts, and presses
/// serialize in order. The event handler always TAKES (even for
/// non-primary buttons, even when it will ignore the value); each POST
/// overwrites, so a lost event heals as soon as the next report
/// arrives. What this does NOT cover is complementary loss in both
/// directions at once: report A arrives, event A never consumes it,
/// report B then fails to replace it, and event B consumes A. That
/// sequence borrows the wrong press's report, and no ordering argument
/// rules it out — it is the residual this design accepts, bounded by
/// the fact that the whole desktop app already depends on this same
/// custom-scheme transport for every event it handles.
///
/// A mutex, not a cell: the route handler and the event handler run on
/// scheme-handler threads, and those are not guaranteed to be one.
#[cfg(all(native_desktop, target_os = "macos"))]
static CLICK_DETAIL: std::sync::Mutex<Option<ClickReport>> = std::sync::Mutex::new(None);

/// Claim the bridge route for the window's lifetime.
///
/// Registered from `WindowFrame`, which never unmounts; Dioxus removes
/// the handler if the registering scope ever drops. The handler records
/// the count synchronously and answers immediately — the response IS the
/// ordering edge the surface's event handler pairs on, so nothing here
/// may await or defer.
#[cfg(all(native_desktop, target_os = "macos"))]
pub(crate) fn use_click_detail_bridge() {
    dioxus::desktop::use_asset_handler(CLICK_DETAIL_ROUTE, |request, responder| {
        let report = request
            .headers()
            .get(CLICK_DETAIL_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_click_report);
        // A poisoned lock degrades like a missing POST: the press does
        // nothing.
        if let Ok(mut slot) = CLICK_DETAIL.lock() {
            *slot = report;
        }
        responder.respond(
            dioxus::desktop::wry::http::Response::builder()
                .status(dioxus::desktop::wry::http::StatusCode::OK)
                .body(Vec::new())
                .expect("a status-only response is always well-formed"),
        );
    });
}

/// Take the bridge report for the press being handled, consuming the pair.
///
/// Always consumes, even when the caller will ignore the value: leaving
/// a report behind is how a later press would borrow the wrong one.
/// `None` on a poisoned lock, which decides as no action.
#[cfg(all(native_desktop, target_os = "macos"))]
fn take_click_detail() -> Option<ClickReport> {
    CLICK_DETAIL.lock().ok().and_then(|mut slot| slot.take())
}

/// Hand the press being handled to AppKit as a native window drag, as a real
/// left mouse-down, even when AppKit's current event is no longer one.
///
/// Tao's `drag_window` (and Dioxus's `drag`, which wraps it) passes
/// `NSApp.currentEvent` to `performWindowDragWithEvent:`, assuming the press is
/// still current when the Dioxus handler runs. It often is not. On a Force
/// Touch trackpad AppKit delivers pressure events (`NSEventTypePressure`) while
/// the button is down, and by the time the press has crossed the webview and
/// the click-detail bridge it is common for one of those, or a mouse-dragged
/// event, to have become current. AppKit silently ignores a drag started from
/// anything but a mouse-down, so on such a Mac many presses on the drag region
/// did nothing (observed in 2026-10: in one logged session, three of eight
/// presses found a pressure or mouse-dragged event current, and before this
/// workaround each such press left the window where it was; a press that
/// found its own mouse-down current dragged normally). When the current event is not a left
/// mouse-down, this builds one at the pointer's present location in this
/// window, which is what Tao itself does for one other event type, and drags
/// from that.
///
/// Does nothing in fullscreen, where AppKit owns the window frame; Dioxus's
/// `drag` had the same guard, and nothing else enforces it for a drag.
#[cfg(all(native_desktop, target_os = "macos"))]
fn start_native_drag(window: &dioxus::desktop::tao::window::Window) {
    use dioxus::desktop::tao::platform::macos::WindowExtMacOS;
    use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSWindow};

    if window.fullscreen().is_some() {
        return;
    }
    // Dioxus runs event handlers on the main thread; anything else is a bug
    // elsewhere, and doing nothing is the safe answer to it.
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    // SAFETY: Tao owns this NSWindow for the window's whole lifetime and
    // exposes its pointer for exactly this kind of platform call. It is only
    // borrowed here, on the main thread, for the duration of this function.
    let Some(ns_window) = (unsafe { (window.ns_window() as *const NSWindow).as_ref() }) else {
        return;
    };
    let current = NSApplication::sharedApplication(mtm).currentEvent();
    let press = match current {
        Some(event) if event.r#type() == NSEventType::LeftMouseDown => event,
        other => {
            // Only the event type is safe to ask of an arbitrary event: AppKit
            // throws when, say, a pressure event is asked for a click count.
            let timestamp = other.as_ref().map_or(0.0, |event| event.timestamp());
            let location = ns_window.convertPointFromScreen(NSEvent::mouseLocation());
            let Some(event) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                NSEventType::LeftMouseDown,
                location,
                NSEventModifierFlags(0),
                timestamp,
                ns_window.windowNumber(),
                None,
                0,
                1,
                1.0,
            ) else {
                return;
            };
            event
        }
    };
    ns_window.performWindowDragWithEvent(&press);
}

/// The flexible gap between the app bar's wordmark and its version.
///
/// Layout only: it keeps the version and the bar's buttons at the bar's
/// right end. It used to be the window's only drag handle, which left most
/// of the title band dead (only this gap dragged, and in a small window or
/// with a long version string it shrank to a sliver). Now it is simply part
/// of the app bar's drag surface, like the rest of the bar's background.
#[component]
pub(crate) fn WindowDragRegion() -> Element {
    rsx! {
        div { class: "window-drag-region", aria_hidden: "true" }
    }
}

/// Class marking a window drag surface: an element whose empty background
/// drags and zooms the macOS window. `assets/click-detail.js` classifies each
/// press inside one (empty background versus a control or text) and reports
/// it; [`window_surface_press`] is the surface's own `onmousedown`. Surfaces
/// must never nest, because each press must be consumed by exactly one
/// handler.
pub(crate) const WINDOW_DRAG_SURFACE: &str = "window-drag-surface";

/// The `onmousedown` of every window drag surface.
///
/// Receives every press inside the surface, controls included, since events
/// bubble; the bridge report POSTed ahead of the event (see
/// `assets/click-detail.js`) says which presses landed on empty background,
/// and only those act, so a button's own click is never disturbed (this
/// handler never prevents a press's default or stops its propagation).
///
/// Double-click maximize/restore is decided HERE, on the primary mousedown:
/// an eligible repeat press zooms exactly once and never drags; an ordinary
/// background press drags; a cross-target repeat, a press on a control or
/// text, and a press without a report do nothing at all. There is
/// deliberately no `dblclick` handler — after the first press enters native
/// dragging, AppKit may withhold its mouse-up from the view, so
/// click/dblclick synthesis cannot be relied upon, while every new press
/// still arrives as a mousedown carrying the native count in `detail`
/// (assuming AppKit classified it as a repeat, which awaits native
/// validation). Dragging goes through [`start_native_drag`]. Inert outside
/// the macOS desktop shell.
pub(crate) fn window_surface_press(event: MouseEvent) {
    #[cfg(all(native_desktop, target_os = "macos"))]
    {
        let window = dioxus::desktop::window();
        let report = take_click_detail();
        let primary =
            event.trigger_button() == Some(dioxus::html::input_data::MouseButton::Primary);
        // `start_native_drag` guards fullscreen itself; `toggle_maximized`
        // does not, so the guard lives in the decision.
        let fullscreen = window.window.fullscreen().is_some();
        match decide_press(primary, report, fullscreen) {
            PressAction::ToggleMaximized => window.toggle_maximized(),
            PressAction::Drag => start_native_drag(&window.window),
            PressAction::None => {}
        }
    }
    #[cfg(not(all(native_desktop, target_os = "macos")))]
    let _ = event;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A report for a press on a surface's empty background.
    fn background(detail: i32, eligible: bool) -> ClickReport {
        ClickReport {
            detail,
            eligible,
            drag: true,
        }
    }

    /// The press decision table is the whole gesture contract: which press
    /// drags, which zooms, what a cross-target repeat does, and what
    /// fullscreen, dead-bridge and control presses do. It matters because
    /// every drag surface also contains controls and text, and the handler
    /// receives their presses too: a wrong row here either moves the window
    /// out from under a button press or leaves the title band dead.
    /// macOS-only bodies (registration, window calls) cannot execute on
    /// Linux; this table is the integration's honestly testable core.
    #[test]
    fn press_decision_covers_buttons_reports_and_fullscreen() {
        // Background presses below a repeat count drag, eligible or not.
        assert_eq!(
            decide_press(true, Some(background(0, false)), false),
            PressAction::Drag
        );
        assert_eq!(
            decide_press(true, Some(background(1, false)), false),
            PressAction::Drag
        );
        assert_eq!(
            decide_press(true, Some(background(1, true)), false),
            PressAction::Drag
        );
        // Every eligible repeat press toggles exactly once and never drags.
        for detail in [2, 3, 7] {
            assert_eq!(
                decide_press(true, Some(background(detail, true)), false),
                PressAction::ToggleMaximized
            );
        }
        // A cross-target repeat is neither a zoom nor a drag: its pair
        // partner was not this surface's background, so it starts nothing.
        assert_eq!(
            decide_press(true, Some(background(2, false)), false),
            PressAction::None
        );
        assert_eq!(
            decide_press(true, Some(background(3, false)), false),
            PressAction::None
        );
        // A press on a control or text inside the surface does nothing,
        // whatever its count: the control keeps its own interaction.
        for (detail, eligible) in [(1, false), (1, true), (2, true), (2, false)] {
            let control = ClickReport {
                detail,
                eligible,
                drag: false,
            };
            assert_eq!(decide_press(true, Some(control), false), PressAction::None);
        }
        // No report means the press cannot be told apart from a control
        // press, so it must not move the window either.
        assert_eq!(decide_press(true, None, false), PressAction::None);
        // Fullscreen suppresses the toggle without substituting a drag; an
        // ordinary press keeps the drag path, which guards fullscreen itself.
        assert_eq!(
            decide_press(true, Some(background(2, true)), true),
            PressAction::None
        );
        assert_eq!(
            decide_press(true, Some(background(1, false)), true),
            PressAction::Drag
        );
        // Non-primary buttons never act, whatever the report claims.
        assert_eq!(decide_press(false, None, false), PressAction::None);
        assert_eq!(
            decide_press(false, Some(background(1, false)), false),
            PressAction::None
        );
        assert_eq!(
            decide_press(false, Some(background(2, true)), false),
            PressAction::None
        );
    }

    /// The bridge header carries one `<detail>:<eligible>:<drag>` report;
    /// anything else must read as a missing report (no action), never as a
    /// fabricated drag or zoom. This pins the wire format the page script's
    /// `reportFor` writes, so the two halves cannot drift apart silently.
    #[test]
    fn click_report_header_parsing_rejects_non_reports() {
        let report = |detail, eligible, drag| {
            Some(ClickReport {
                detail,
                eligible,
                drag,
            })
        };
        assert_eq!(parse_click_report("0:0:1"), report(0, false, true));
        assert_eq!(parse_click_report("1:0:1"), report(1, false, true));
        assert_eq!(parse_click_report("1:1:0"), report(1, true, false));
        assert_eq!(parse_click_report("2:1:1"), report(2, true, true));
        assert_eq!(parse_click_report(" 3:1:0 "), report(3, true, false));
        for malformed in [
            "", "2", "2:", ":1", "1:0", "2:1", "2:1:", "2:1:2", "2:1:yes", "two:1:1", "2:2:1",
            "-1:0:1", "2.0:1:1", "1:0:0:0",
        ] {
            assert_eq!(parse_click_report(malformed), None, "{malformed:?}");
        }
    }
}
