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
        // Window lifetime, not page lifetime: this root never unmounts, so
        // the bridge route registered here stays until the window closes.
        use_click_detail_bridge();
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

/// What one spacer press does. Exactly one variant runs per press; no press
/// both drags and toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// The decision helpers below are called only from the macOS desktop
// press path, but stay compiled everywhere so Linux unit tests cover
// the truth table; outside that path they are test-only by design.
#[cfg_attr(not(all(feature = "desktop", target_os = "macos")), allow(dead_code))]
pub(crate) enum PressAction {
    /// AppKit classified this press as a repeat click: zoom/restore once.
    ToggleMaximized,
    /// An ordinary press: the existing native drag.
    Drag,
    /// No native action: a non-primary button, a guarded toggle, or a
    /// cross-target repeat (no zoom AND no drag — see `decide_press`).
    None,
}

/// One bridge report: the press's DOM click count plus whether it may zoom.
///
/// `eligible` is the DOM side's targeting verdict: the immediately
/// preceding primary press landed on the same connected spacer node, with
/// no intervening press elsewhere, non-primary press, or spacer
/// replacement. The native count classifies timing (fast enough to be a
/// repeat); eligibility classifies targeting (both presses belong here).
/// The two arrive in one header value so they cannot split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(all(feature = "desktop", target_os = "macos")), allow(dead_code))]
pub(crate) struct ClickReport {
    pub detail: i32,
    pub eligible: bool,
}

/// Decide drag versus maximize-toggle from the bridged report.
///
/// `report` is what the click-detail bridge POSTed for THIS press (`None`
/// when no POST preceded the event — the script missing, the route
/// unregistered, or a malformed report). `None` means drag, the
/// pre-bridge behavior: a missing report against an empty slot, or a
/// count stuck at 1, degrades to ordinary dragging. That fail-safe is
/// bounded to those cases; it is not a claim about every partial
/// transport failure (see the slot's own docs for the residual).
///
/// A repeat press toggles only with an eligible predecessor — double,
/// triple, rapid pairs alike — and no repeat press ever drags, so
/// triple-clicking cannot fling the window. A repeat WITHOUT one (a
/// cross-target pair: header text or a control, then the spacer) does
/// nothing at all, deliberately: no zoom AND no drag, so a foreign
/// press can neither zoom the window nor start moving it. The press
/// still counts as the eligible predecessor for the NEXT press, so two
/// consecutive spacer presses always form a valid pair whatever came
/// before them. Fullscreen suppresses the toggle explicitly:
/// `toggle_maximized` has no fullscreen check of its own (unlike
/// `drag`), and Tao defers a fullscreen maximize change until exit,
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
#[cfg_attr(not(all(feature = "desktop", target_os = "macos")), allow(dead_code))]
pub(crate) fn decide_press(
    primary: bool,
    report: Option<ClickReport>,
    fullscreen: bool,
) -> PressAction {
    if !primary {
        return PressAction::None;
    }
    match report {
        Some(report) if report.detail >= 2 && report.eligible => {
            if fullscreen {
                PressAction::None
            } else {
                PressAction::ToggleMaximized
            }
        }
        // A cross-target repeat is neither a zoom nor a drag: its pair
        // partner was not on this spacer, so this press starts no gesture.
        Some(report) if report.detail >= 2 => PressAction::None,
        _ => PressAction::Drag,
    }
}

/// Read the bridge report the header carries.
///
/// The header value is `<detail>:<eligible>` — one DOM `detail` as ASCII
/// digits plus `1` or `0`. Anything else — absent, unparseable, negative,
/// a second colon — is `None`, which decides as an ordinary drag.
#[cfg_attr(not(all(feature = "desktop", target_os = "macos")), allow(dead_code))]
pub(crate) fn parse_click_report(value: &str) -> Option<ClickReport> {
    let (detail, eligible) = value.trim().split_once(':')?;
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
    Some(ClickReport { detail, eligible })
}

/// First path segment the click-detail bridge claims.
///
/// Dioxus routes a request to the asset handler registered under its
/// first path segment (`protocol.rs`), so this must not collide with the
/// `assets` route or the exact-matched `__events`/`__file_dialog` paths.
/// The JS half derives the same URL from the interpreter's events path.
#[cfg(all(feature = "desktop", target_os = "macos"))]
const CLICK_DETAIL_ROUTE: &str = "fh-click-detail";

/// The header carrying one press's bridge report.
///
/// A header rather than a POST body, mirroring the interpreter's own
/// event send (whose bodies live in headers as an Android workaround):
/// headers provably arrive — every `__events` send depends on one —
/// which is more than this bridge claims to know about custom-scheme
/// POST bodies on macOS.
#[cfg(all(feature = "desktop", target_os = "macos"))]
const CLICK_DETAIL_HEADER: &str = "x-fh-click-detail";

/// The latest bridge report, awaiting its own mousedown event.
///
/// Written by the bridge route handler, consumed by the spacer's
/// onmousedown. Pairing is by ORDER, and the ordering is structural:
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
#[cfg(all(feature = "desktop", target_os = "macos"))]
static CLICK_DETAIL: std::sync::Mutex<Option<ClickReport>> = std::sync::Mutex::new(None);

/// Claim the bridge route for the window's lifetime.
///
/// Registered from `WindowFrame`, which never unmounts; Dioxus removes
/// the handler if the registering scope ever drops. The handler records
/// the count synchronously and answers immediately — the response IS the
/// ordering edge the spacer's event handler pairs on, so nothing here
/// may await or defer.
#[cfg(all(feature = "desktop", target_os = "macos"))]
pub(crate) fn use_click_detail_bridge() {
    dioxus::desktop::use_asset_handler(CLICK_DETAIL_ROUTE, |request, responder| {
        let report = request
            .headers()
            .get(CLICK_DETAIL_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_click_report);
        // A poisoned lock degrades like a missing POST: the press drags.
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
/// `None` on a poisoned lock, which decides as an ordinary drag.
#[cfg(all(feature = "desktop", target_os = "macos"))]
fn take_click_detail() -> Option<ClickReport> {
    CLICK_DETAIL.lock().ok().and_then(|mut slot| slot.take())
}

/// Give empty sidebar-header space a native drag action, never its controls.
///
/// This separate, non-focusable element has no interactive descendants. A
/// parent-level mouse handler would also receive clicks from Profiles or the
/// version label, stealing normal control activation and text selection —
/// and it would break the bridge pairing, which assumes exactly one Rust
/// press handler consumes each POST. Dioxus's native drag helper ignores
/// dragging while the window is fullscreen.
///
/// Double-click maximize/restore is decided HERE, on the primary mousedown,
/// from the bridge report POSTed ahead of the event (see
/// `assets/click-detail.js`): an eligible repeat press zooms exactly once
/// and never drags; an ordinary press drags as before; a cross-target
/// repeat does nothing at all. There is deliberately no `dblclick`
/// handler — after the first press enters native dragging, AppKit may
/// withhold its mouse-up from the view, so click/dblclick synthesis
/// cannot be relied upon, while every new press still arrives as a
/// mousedown carrying the native count in `detail` (assuming AppKit
/// classified it as a repeat, which awaits native validation). Other
/// builds leave this spacer hidden and its event handler inert.
#[component]
pub(crate) fn WindowDragRegion() -> Element {
    rsx! {
        div {
            class: "window-drag-region",
            aria_hidden: "true",
            onmousedown: move |event: MouseEvent| {
                #[cfg(all(feature = "desktop", target_os = "macos"))]
                {
                    let window = dioxus::desktop::window();
                    let report = take_click_detail();
                    let primary =
                        event.trigger_button() == Some(dioxus::html::input_data::MouseButton::Primary);
                    // `drag` guards fullscreen internally; `toggle_maximized`
                    // does not, so the guard lives in the decision.
                    let fullscreen = window.window.fullscreen().is_some();
                    match decide_press(primary, report, fullscreen) {
                        PressAction::ToggleMaximized => window.toggle_maximized(),
                        PressAction::Drag => window.drag(),
                        PressAction::None => {}
                    }
                }
                #[cfg(not(all(feature = "desktop", target_os = "macos")))]
                let _ = event;
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The press decision table is the whole gesture contract: which press
    /// drags, which zooms, what a cross-target repeat does, and what
    /// fullscreen and dead-bridge presses do. macOS-only bodies
    /// (registration, window calls) cannot execute on Linux; this table
    /// is the integration's honestly testable core.
    #[test]
    fn press_decision_covers_buttons_reports_and_fullscreen() {
        let ordinary = ClickReport {
            detail: 1,
            eligible: false,
        };
        let first = ClickReport {
            detail: 0,
            eligible: false,
        };
        let repeat = ClickReport {
            detail: 2,
            eligible: true,
        };
        let triple = ClickReport {
            detail: 3,
            eligible: true,
        };
        let rapid = ClickReport {
            detail: 7,
            eligible: true,
        };
        let foreign = ClickReport {
            detail: 2,
            eligible: false,
        };
        let foreign_triple = ClickReport {
            detail: 3,
            eligible: false,
        };
        // Ordinary presses drag, including a missing bridge report, and
        // eligibility is irrelevant below a repeat count.
        assert_eq!(decide_press(true, None, false), PressAction::Drag);
        assert_eq!(decide_press(true, Some(first), false), PressAction::Drag);
        assert_eq!(decide_press(true, Some(ordinary), false), PressAction::Drag);
        assert_eq!(
            decide_press(
                true,
                Some(ClickReport {
                    detail: 1,
                    eligible: true
                }),
                false
            ),
            PressAction::Drag
        );
        // Every eligible repeat press toggles exactly once and never drags.
        assert_eq!(
            decide_press(true, Some(repeat), false),
            PressAction::ToggleMaximized
        );
        assert_eq!(
            decide_press(true, Some(triple), false),
            PressAction::ToggleMaximized
        );
        assert_eq!(
            decide_press(true, Some(rapid), false),
            PressAction::ToggleMaximized
        );
        // A cross-target repeat is neither a zoom nor a drag: its pair
        // partner was not on this spacer, so it starts no gesture.
        assert_eq!(decide_press(true, Some(foreign), false), PressAction::None);
        assert_eq!(
            decide_press(true, Some(foreign_triple), false),
            PressAction::None
        );
        assert_eq!(decide_press(true, Some(foreign), true), PressAction::None);
        // Fullscreen suppresses the toggle without substituting a drag;
        // ordinary presses keep the existing drag path (self-guarding).
        assert_eq!(decide_press(true, Some(repeat), true), PressAction::None);
        assert_eq!(decide_press(true, Some(triple), true), PressAction::None);
        assert_eq!(decide_press(true, None, true), PressAction::Drag);
        assert_eq!(decide_press(true, Some(ordinary), true), PressAction::Drag);
        // Non-primary buttons never act, whatever the report claims.
        assert_eq!(decide_press(false, None, false), PressAction::None);
        assert_eq!(
            decide_press(false, Some(ordinary), false),
            PressAction::None
        );
        assert_eq!(decide_press(false, Some(repeat), false), PressAction::None);
        assert_eq!(decide_press(false, Some(repeat), true), PressAction::None);
    }

    /// The bridge header carries one `<detail>:<eligible>` report; anything
    /// else must read as a missing report (ordinary drag), never as a
    /// fabricated zoom.
    #[test]
    fn click_report_header_parsing_rejects_non_reports() {
        let report = |detail, eligible| Some(ClickReport { detail, eligible });
        assert_eq!(parse_click_report("0:0"), report(0, false));
        assert_eq!(parse_click_report("1:0"), report(1, false));
        assert_eq!(parse_click_report("1:1"), report(1, true));
        assert_eq!(parse_click_report("2:1"), report(2, true));
        assert_eq!(parse_click_report("2:0"), report(2, false));
        assert_eq!(parse_click_report(" 3:1 "), report(3, true));
        assert_eq!(parse_click_report(""), None);
        assert_eq!(parse_click_report("2"), None);
        assert_eq!(parse_click_report("2:"), None);
        assert_eq!(parse_click_report(":1"), None);
        assert_eq!(parse_click_report("two:1"), None);
        assert_eq!(parse_click_report("2:2"), None);
        assert_eq!(parse_click_report("2:yes"), None);
        assert_eq!(parse_click_report("-1:0"), None);
        assert_eq!(parse_click_report("2.0:1"), None);
        assert_eq!(parse_click_report("1:0:0"), None);
    }
}
