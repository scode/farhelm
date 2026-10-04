//! The sticky sidebar bar (wordmark, build identity and helm settings) and the session-list
//! profile control.
//!
//! The helm's reported build is available through the existing skew latch, so
//! the version stays a read-only view of that signal. It deliberately shows
//! the client build until a reported mismatch gives it a more useful helm
//! value; a page never needs a loading placeholder to identify its bundle.

use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;
use web_time::Instant;

use std::collections::HashMap;

use crate::menu_panel::{
    MenuFocusQueue, MenuOpenIntent, MenuOrder, MenuWiring, PanelPlacement, ROW_MENU_OUTSIDE_RELAY,
    cancel_menu_focus, closed_toggle_key_intent, focus_menu_toggle, forget_menu_focus,
    handle_menu_key, install_row_menu_outside_dismiss, measurement_outcome, remember_menu_item,
    row_menu_relay_key, session_menu_placement_style, session_menu_pointer_style,
    should_measure_on_mount,
};
use crate::ops::OpLock;
use crate::peer::display_peer;
use crate::profiles::{
    CatalogSurface, FOCUS_SETTLE_MS, FOCUS_TRANSIT_GRACE_MS, FocusCoordinator, ProfilesPopup,
};
use crate::reader::{Trigger, finish_before, sleep_ms};
use crate::skew::{self, Skew};

/// Keep the popup inside the viewport while anchoring its left edge to the
/// profile trigger whenever the viewport has room.
fn profiles_popover_placement_style(placement: PanelPlacement) -> String {
    const MARGIN: f64 = 8.0;
    const GAP: f64 = 2.0;
    const WIDTH: f64 = 320.0;
    match placement {
        PanelPlacement::Unmeasured => "opacity: 0; pointer-events: none;".to_string(),
        PanelPlacement::Measured(rect) => {
            let top = rect.max_y() + GAP;
            let left = rect.min_x();
            format!(
                "opacity: 1; pointer-events: auto; right: auto; \
                 --profiles-popover-top: max({MARGIN}px, min({top}px, calc(100vh - {MARGIN}px))); \
                 --profiles-popover-left: max({MARGIN}px, min({left}px, calc(100vw - {MARGIN}px))); \
                 top: var(--profiles-popover-top); left: var(--profiles-popover-left); \
                 max-width: min({WIDTH}px, \
                 calc(100vw - {MARGIN}px - var(--profiles-popover-left)), \
                 calc(100vw - {}px)); \
                 max-height: calc(100vh - {MARGIN}px - var(--profiles-popover-top));",
                MARGIN * 2.0,
            )
        }
        PanelPlacement::Fallback => format!(
            "opacity: 1; pointer-events: auto; right: auto; \
             top: {MARGIN}px; left: {MARGIN}px; \
             max-width: min({WIDTH}px, calc(100vw - {}px)); \
             max-height: calc(100vh - {}px);",
            MARGIN * 2.0,
            MARGIN * 2.0,
        ),
    }
}

/// Select the version string the sidebar should show for the current skew state.
///
/// `None` means no mismatch has been latched — either no reply has arrived yet,
/// or every reply so far agreed with this client's build, which is the healthy
/// steady state and the common one. In both cases the compiled client build IS
/// the helm's version as far as anything can tell (agreement means the two
/// stamps are the same string), so showing it is exact, not a placeholder. A
/// silent helm is the skew banner's business. A reported stamp identifies the
/// helm that actually answered, and is shown as sent.
fn displayed_version(skew: Option<&Skew>) -> &str {
    match skew {
        Some(Skew::Reported(stamp)) => stamp,
        Some(Skew::Silent) | None => skew::CLIENT_BUILD,
    }
}

/// Return keyboard focus to the control that owns the profiles popup.
///
/// Layout and deferred focus-out dismissal run after the event that moved
/// focus, so a declarative autofocus attribute cannot restore this already
/// mounted control. The query is constant and carries no peer text.
fn focus_profiles_toggle() {
    document::eval("document.querySelector('.profiles-toggle')?.focus({ preventScroll: true });");
}

/// Route focus changes and trusted outside choices into popup-owned relays.
///
/// Provenance is captured before pointer focus moves and from the Tab key that
/// caused keyboard focus to move. The relay handlers attach that provenance to
/// the exact Rust obligation sequence; no later DOM read has to reconstruct it.
/// Programmatic `focus()` is deliberately only an ordinary focus-out because
/// browsers may mark the resulting focus event trusted even though no user
/// chose that destination. Focus returning inside cancels the pending fact
/// directly: starting another asynchronous classifier there could let an old
/// cancellation finish after a newer outside choice. A trusted Tab reserves
/// its token at keydown, so the same provenance survives when focus leaves the
/// document and there is no outside `focusin` to report the destination.
///
/// The popup node also carries a synchronous outside-intent veto. A focus
/// commit already dispatched over the bridge can outlive its Rust future;
/// checking this veto before focus prevents that commit from undoing the
/// pointer event that just arrived. Each opening owns a different node.
///
/// A focus-out from a DISABLED control is not reported at all. Disabling the
/// focused control (every popup button binds `disabled` to the operation
/// lock) drops focus to `body` as a side effect of the product's own
/// re-render, not as a user choice — but the resulting event is
/// indistinguishable from a real move except by the target's disabled
/// state, and only at dispatch time. Reporting it lets the dismissal
/// classifier race save completion: if it samples `body` after the lock
/// releases but before completion focus commits, the popup closes over a
/// save the user never asked to dismiss. A control that was already
/// disabled cannot hold focus, so any such event IS a disable-blur.
fn install_profiles_outside_intent_tracking() {
    document::eval(
        "if (!window.__farhelmProfilesOutsideIntentTracking) { \
             window.__farhelmProfilesOutsideIntentTracking = true; \
             let pointerPopup = null; \
             let tabIntent = null; \
             const relay = (popup, trusted) => { \
                 if (trusted) popup.__farhelmProfilesOutsideIntent = true; \
                 popup.querySelector(trusted \
                     ? '.profiles-trusted-focusout-relay' \
                     : '.profiles-focusout-relay')?.click(); \
             }; \
             const cancel = (popup) => { \
                 popup.__farhelmProfilesOutsideIntent = false; \
                 popup.querySelector('.profiles-focusin-relay')?.click(); \
             }; \
             const reconsider = (popup) => \
                 popup.querySelector('.profiles-focus-recheck-relay')?.click(); \
             const reserveTab = (popup) => \
                 popup.querySelector('.profiles-tab-start-relay')?.click(); \
             const commitTab = (intent) => { \
                 if (tabIntent !== intent) return; \
                 tabIntent = null; \
                 intent.popup.__farhelmProfilesOutsideIntent = true; \
                 intent.popup.querySelector('.profiles-tab-commit-relay')?.click(); \
             }; \
             const cancelTab = (intent) => { \
                 if (tabIntent !== intent) return; \
                 tabIntent = null; \
                 cancel(intent.popup); \
             }; \
             document.addEventListener('pointerdown', (event) => { \
                 if (!event.isTrusted) return; \
                 const popup = document.querySelector('.profiles-popover'); \
                 const target = event.target; \
                 if (!popup || !(target instanceof Element)) return; \
                 if (popup.contains(target)) { \
                     pointerPopup = null; \
                     cancel(popup); \
                     return; \
                 } \
                 if (target.closest('.profiles-toggle')) return; \
                 pointerPopup = popup; \
                 relay(popup, true); \
                 setTimeout(() => { if (pointerPopup === popup) pointerPopup = null; }, 0); \
             }, true); \
             document.addEventListener('focusout', (event) => { \
                 const popup = document.querySelector('.profiles-popover'); \
                 if (!popup || !event.composedPath().includes(popup)) return; \
                 if (event.target instanceof Element && event.target.disabled) return; \
                 if (pointerPopup === popup) { pointerPopup = null; return; } \
                 const intent = tabIntent; \
                 if (intent?.popup === popup) { \
                     intent.focusout = true; \
                     setTimeout(() => { \
                         if (tabIntent !== intent) return; \
                         const active = document.activeElement; \
                         if (active && (popup.contains(active) || active.closest?.('.profiles-toggle'))) \
                             cancelTab(intent); \
                         else \
                             commitTab(intent); \
                     }, 0); \
                     return; \
                 } \
                 relay(popup, false); \
             }, true); \
             document.addEventListener('keydown', (event) => { \
                 const popup = document.querySelector('.profiles-popover'); \
                 const active = document.activeElement; \
                 if (event.isTrusted && !event.isComposing && event.key === 'Escape' && popup && \
                     (!active || active === document.body)) { \
                     event.preventDefault(); \
                     popup.querySelector('.profiles-escape-relay')?.click(); \
                     return; \
                 } \
                 if (!event.isTrusted || event.key !== 'Tab' || !popup || !active || !popup.contains(active)) return; \
                 if (tabIntent) cancelTab(tabIntent); \
                 const intent = { popup, focusout: false }; \
                 tabIntent = intent; \
                 reserveTab(popup); \
                 setTimeout(() => { if (tabIntent === intent && !intent.focusout) cancelTab(intent); }, 0); \
             }, true); \
             document.addEventListener('focusin', (event) => { \
                 const popup = document.querySelector('.profiles-popover'); \
                 const target = event.target; \
                 if (!popup || !(target instanceof Element)) return; \
                 const intent = tabIntent; \
                 if (popup.contains(target) || target.closest('.profiles-toggle')) { \
                     if (intent?.popup === popup) cancelTab(intent); else cancel(popup); \
                 } else if (intent?.popup === popup && intent.focusout && event.isTrusted) { \
                     commitTab(intent); \
                 } else if (!(event.relatedTarget instanceof Element) || \
                     (!popup.contains(event.relatedTarget) && !event.relatedTarget.closest('.profiles-toggle'))) { \
                     reconsider(popup); \
                 } \
             }, true); \
             window.addEventListener('blur', () => { \
                 const intent = tabIntent; \
                 if (intent?.focusout) commitTab(intent); \
             }, true); \
             window.addEventListener('focus', (event) => { \
                 if (event.target !== window) return; \
                 const popup = document.querySelector('.profiles-popover'); \
                 if (popup) reconsider(popup); \
             }, true); \
         }",
    );
}

/// Pause after a rectangle sample only when the browser test gate requests it.
///
/// The seam leaves production measurements untouched. Tests use it to change
/// the layout epoch while an old rectangle is genuinely awaiting acceptance.
async fn hold_profile_measurement_for_test() {
    let _ = document::eval(
        "const gate = window.__farhelmTestProfiles?.measurement; \
         if (!gate || gate.holds <= 0) return; \
         gate.holds -= 1; \
         gate.started = (gate.started || 0) + 1; \
         await new Promise((resolve) => { gate.release = resolve; });",
    )
    .await;
}

/// A focus-out obligation's identity across async classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FocusObligation {
    opening: u64,
    sequence: u64,
    /// Whether a trusted pointer or Tab destination caused this obligation.
    trusted_outside: bool,
    /// A later focus event can reconsider the same unresolved intent. Its
    /// revision fences out an older classifier without replacing the intent's
    /// sequence or losing the trusted outside choice that completion yields to.
    observation: u64,
}

/// The settled destinations relevant to popup focus-out dismissal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProfileFocus {
    /// No control owns focus while the browser commits a replacement.
    Transit,
    /// The popup or its trigger still owns the interaction.
    Inside,
    /// A deliberate destination elsewhere on the page owns focus.
    Outside,
    /// The renderer returned no evidence from which dismissal may be inferred.
    Unknown,
}

/// Classify the document's active element without relying on `relatedTarget`.
///
/// The desktop bridge does not preserve `relatedTarget`, while active-element
/// lookup behaves the same in both renderers. `body` and no active element are
/// transit because a focused control can disappear before its replacement is
/// committed. The caller's monotonic deadline bounds the bridge itself; a
/// renderer that does not answer in time supplies `Unknown`, not transit.
/// Captured provenance scopes an optional test hold; it never changes the
/// active-element classification or the production dismissal decision.
async fn classify_profile_focus(deadline: Instant, trusted_outside: bool) -> ProfileFocus {
    let classification = document::eval(&format!(
        "const trustedOutside = {trusted_outside}; {}",
        "const test = window.__farhelmTestProfiles; \
         if (test) test.classificationAttempts = (test.classificationAttempts || 0) + 1; \
         const gate = test?.classification; \
         if (gate?.holds > 0 && (!gate.trustedOnly || trustedOutside)) { \
             gate.holds -= 1; \
             gate.started = (gate.started || 0) + 1; \
             await new Promise((resolve) => { (gate.releases ||= []).push(resolve); }); \
         } \
         if (gate?.delayMs > 0) \
             await new Promise((resolve) => setTimeout(resolve, gate.delayMs)); \
         if (test?.classificationErrors > 0) { \
             test.classificationErrors -= 1; \
             throw new Error('held focus classification'); \
         } \
         const active = document.activeElement; \
         if (!active || active === document.body) return 'transit'; \
         if (active === document.querySelector('.profiles-toggle') || \
             document.querySelector('.profiles-popover')?.contains(active)) return 'inside'; \
         return 'outside';",
    ));
    match finish_before(deadline, classification.join::<String>()).await {
        Some(Ok(value)) if value == "inside" => ProfileFocus::Inside,
        Some(Ok(value)) if value == "outside" => ProfileFocus::Outside,
        Some(Ok(value)) if value == "transit" => ProfileFocus::Transit,
        Some(Err(_)) | Some(Ok(_)) | None => ProfileFocus::Unknown,
    }
}

/// Wait for a pending popup placement before classifying transit one last time.
///
/// The total wait is the focus worker's full budget plus its documented grace.
/// Polling the shared pending signal lets a successful or failed request settle
/// dismissal early, while the deadline keeps a broken destination bounded. An
/// initial `Unknown` may be retried, but a later transit sample still enters
/// the same pending-request loop rather than becoming dismissal evidence alone.
/// Every evaluation retains the initiating obligation's provenance, including
/// retries, so a scoped test hold cannot be consumed by an older form transition.
async fn settled_profile_focus(focus: FocusCoordinator, trusted_outside: bool) -> ProfileFocus {
    let deadline = Instant::now() + Duration::from_millis(FOCUS_SETTLE_MS + FOCUS_TRANSIT_GRACE_MS);
    // Let the pointer event finish first. Focus-out precedes the click whose
    // handler records a replacement request, so classifying in the same task
    // would observe `body` before that request exists.
    sleep_ms(0).await;
    let mut classification = classify_profile_focus(deadline, trusted_outside).await;
    if classification == ProfileFocus::Unknown {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let delay = remaining.as_millis().min(25) as u64;
        if delay == 0 {
            return ProfileFocus::Unknown;
        }
        sleep_ms(delay).await;
        classification = classify_profile_focus(deadline, trusted_outside).await;
    }
    if classification != ProfileFocus::Transit {
        return classification;
    }
    while focus.pending() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let delay = remaining.as_millis().min(10) as u64;
        if delay == 0 {
            break;
        }
        sleep_ms(delay).await;
    }
    if focus.pending() || focus.unknown() {
        return ProfileFocus::Unknown;
    }
    classify_profile_focus(deadline, trusted_outside).await
}

/// The Farhelm wordmark for a dark ground, exactly as the brand files ship it.
///
/// CLAUDE.md makes `packaging/farhelm-desktop/wordmark-*.svg` the one source
/// for anything that shows the name as a mark: its letters are outlines of the
/// vendored font, generated by `website/scripts/render-svgs.mjs`, so they
/// render identically whether or not the font has loaded, and a regeneration
/// reaches the UI with no copy to forget. The UI is dark-only, so only the dark
/// variant is used. Inlined at compile time, like `icons.rs`'s marks, so the
/// web bundle and the desktop build both carry it without an asset entry that
/// `scripts/check-desktop-assets.sh` would have to keep in parity. The file
/// carries its own `role="img"` and `aria-label="farhelm"`, which is the
/// accessible name the bar's mark needs.
const WORDMARK_SVG: &str = include_str!("../../../packaging/farhelm-desktop/wordmark-dark.svg");

/// What the help menu offers, in the order it shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum HelpAction {
    SendFeedback,
    Documentation,
}

impl HelpAction {
    /// The item's label, which is also its `data-help-action` marker.
    fn label(self) -> &'static str {
        match self {
            HelpAction::SendFeedback => "send feedback",
            HelpAction::Documentation => "documentation",
        }
    }

    /// The description line under the label.
    fn description(self) -> &'static str {
        match self {
            HelpAction::SendFeedback => "privately, to Farhelm's maintainer",
            HelpAction::Documentation => "opens farhelm.io/docs in your browser",
        }
    }
}

const HELP_ACTIONS: [HelpAction; 2] = [HelpAction::SendFeedback, HelpAction::Documentation];

/// The docs site the Documentation item opens.
const DOCS_URL: &str = "https://farhelm.io/docs/";

/// Open the docs site through the page's shared link opener
/// (`terminal-links.js`), which knows how to reach the system browser from
/// the desktop webview and opens a new tab elsewhere. The URL is a constant,
/// so it needs none of the opener callers' validation.
///
/// That opener is its own script asset, loaded asynchronously, so a click
/// right after the page loads can arrive before it exists. The fallback
/// repeats the opener's two branches (navigate the `dioxus:` page, which the
/// desktop shell turns into a system-browser open; otherwise a `noopener`
/// tab) so the item never silently does nothing.
fn open_documentation() {
    let url = serde_json::to_string(DOCS_URL).expect("a string always serializes");
    document::eval(&format!(
        "(() => {{
            const url = {url};
            if (window.farhelmTerminalLinks) {{
                window.farhelmTerminalLinks.openTerminalUrl(url);
            }} else if (window.location.protocol === 'dioxus:') {{
                window.location.assign(url);
            }} else {{
                window.open(url, '_blank', 'noopener');
            }}
        }})();"
    ));
}

/// The sidebar bar's help menu: a `?` toggle to the right of the settings
/// gear, opening a small menu with Send feedback and Documentation
/// (SPEC.md "Feedback").
///
/// It reuses the row menus' machinery (`menu_panel`) so it behaves like
/// them: arrow keys, Home/End, Escape and Tab inside the menu, focus back on
/// the toggle when it closes, the same side flyout and pointer, and the
/// shared outside-pointer dismissal, which reaches this menu through its own
/// relay button keyed `help:bar`. Its elements carry the host row menu's
/// panel and item classes for the shared look, plus `help-menu-*` classes of
/// their own, which is what the dismissal and focus helpers look for.
///
/// Its open state is local: unlike the row menus, nothing else on the page
/// needs to know or close it, beyond what the outside-pointer dismissal
/// already does.
#[component]
fn HelpMenu(layout_epoch: ReadSignal<u64>, on_send_feedback: EventHandler<()>) -> Element {
    let mut open = use_signal(|| false);
    // The layout epoch the open menu was measured under. Its coordinates
    // are a snapshot, so a later resize or scroll closes the menu, as it
    // does the session and host menus, rather than leaving it detached
    // from its toggle (the bar's controls move at the narrow-window
    // breakpoint).
    let mut opened_epoch = use_signal(|| *layout_epoch.peek());
    let mut toggle_handle = use_signal(|| None::<Rc<MountedData>>);
    let placement = use_signal(|| PanelPlacement::Unmeasured);
    let item_handles = use_signal(HashMap::new);
    let mut menu_focus = use_signal(|| None::<usize>);
    let mut menu_requested = use_signal(|| None::<usize>);
    let mut open_intent = use_signal(|| None::<MenuOpenIntent>);
    let focus_queue = MenuFocusQueue {
        target: use_signal(|| None::<Rc<MountedData>>),
        draining: use_signal(|| false),
    };
    let open_generation = use_signal(|| 0_u64);
    let order: MenuOrder<HelpAction, 2> = MenuOrder::pack(HELP_ACTIONS, |_| true);
    let close_menu = use_callback(move |()| open.set(false));
    let wiring = MenuWiring {
        order,
        handles: item_handles,
        focus: focus_queue,
        focused: menu_focus,
        requested: menu_requested,
        open_intent,
        close_menu,
    };
    let spawn_measurement = move || {
        let mut placement = placement;
        let generation = open_generation();
        spawn(async move {
            let measured = match toggle_handle.peek().clone() {
                Some(handle) => handle.get_client_rect().await.ok(),
                None => None,
            };
            if let Some(outcome) =
                measurement_outcome(generation, *open_generation.peek(), measured)
            {
                placement.set(outcome);
            }
        });
    };
    let mut begin_open = move |intent: MenuOpenIntent| {
        let mut open_generation = open_generation;
        let mut placement = placement;
        let mut item_handles = item_handles;
        open_generation += 1;
        placement.set(PanelPlacement::Unmeasured);
        item_handles.write().clear();
        cancel_menu_focus(focus_queue);
        menu_focus.set(None);
        menu_requested.set(None);
        open_intent.set(Some(intent));
        opened_epoch.set(*layout_epoch.peek());
        install_row_menu_outside_dismiss();
        open.set(true);
        spawn_measurement();
    };
    use_effect(move || {
        let epoch = layout_epoch();
        if *open.peek() && epoch != *opened_epoch.peek() {
            open.set(false);
        }
    });
    // The close teardown the row menus share: forget focus bookkeeping and,
    // when focus was inside the menu, hand it back to the toggle.
    use_effect(move || {
        if open() {
            return;
        }
        cancel_menu_focus(focus_queue);
        let was_inside = menu_focus.peek().is_some();
        menu_focus.set(None);
        menu_requested.set(None);
        open_intent.set(None);
        let mut item_handles = item_handles;
        item_handles.write().clear();
        if was_inside {
            focus_menu_toggle("data-help-menu", "bar", ".app-help-toggle");
        }
    });
    let tab_stop = menu_focus()
        .and_then(|position| order.get(position))
        .or_else(|| order.get(0));
    let mut choose = move |action: HelpAction| {
        open.set(false);
        match action {
            HelpAction::SendFeedback => on_send_feedback.call(()),
            HelpAction::Documentation => open_documentation(),
        }
    };

    rsx! {
        span { class: "app-help-menu", "data-help-menu": "bar",
            button {
                r#type: "button",
                class: "btn btn-neutral app-help-toggle",
                aria_label: "help",
                title: "help",
                aria_haspopup: "menu",
                aria_expanded: open(),
                onkeydown: move |evt| {
                    if !open() {
                        let Some(intent) = closed_toggle_key_intent(&evt.key()) else {
                            return;
                        };
                        evt.prevent_default();
                        begin_open(intent);
                        return;
                    }
                    handle_menu_key(&evt, None, wiring, &());
                },
                onfocusin: move |_| forget_menu_focus(wiring),
                onmounted: move |element| {
                    toggle_handle.set(Some(element.data()));
                    if should_measure_on_mount(open(), *placement.peek()) {
                        spawn_measurement();
                    }
                },
                onclick: move |_| {
                    if open() {
                        open.set(false);
                    } else {
                        begin_open(MenuOpenIntent::First);
                    }
                },
                crate::icons::HelpIcon {}
            }
            if open() {
                div {
                    class: "host-row-menu-flyout help-menu-flyout",
                    style: session_menu_placement_style(placement()),
                    if let Some(pointer_style) = session_menu_pointer_style(placement()) {
                        span { class: "host-row-menu-pointer", style: pointer_style, "aria-hidden": "true" }
                    }
                    div { class: "host-row-menu-panel help-menu-panel",
                        div {
                            class: "host-row-menu-items session-row-menu-items",
                            role: "menu",
                            aria_label: "help",
                            // Rendered from the same list keyboard order is
                            // built from, so the two cannot drift apart.
                            for action in HELP_ACTIONS {
                                button {
                                    key: "{action.label()}",
                                    r#type: "button",
                                    class: "btn session-row-menu-item host-row-menu-item help-menu-item",
                                    "data-help-action": action.label(),
                                    role: "menuitem",
                                    tabindex: if tab_stop == Some(action) { "0" } else { "-1" },
                                    onmounted: move |element| remember_menu_item(wiring, action, element.data()),
                                    onfocusin: move |_| menu_focus.set(order.position(action)),
                                    onfocusout: move |_| menu_focus.set(None),
                                    onkeydown: move |evt| handle_menu_key(&evt, order.position(action), wiring, &()),
                                    onclick: move |_| choose(action),
                                    span { class: "session-row-menu-copy",
                                        span { class: "session-row-menu-label", "{action.label()}" }
                                        span { class: "session-row-menu-description", "{action.description()}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            button {
                r#type: "button",
                class: ROW_MENU_OUTSIDE_RELAY,
                "data-row-menu": row_menu_relay_key("help", "bar"),
                hidden: true,
                tabindex: "-1",
                onclick: move |_| open.set(false),
            }
        }
    }
}

/// How long after the feedback dialog closes its success is announced: long
/// enough for the dialog's modal isolation to be released (one frame or
/// two), so the status region is in the accessibility tree when it changes.
const ANNOUNCE_AFTER_CLOSE_MS: u64 = 100;

/// Render the sticky sidebar bar: the Farhelm wordmark at the window's top
/// left, then the build identity, the helm-wide settings gear, and the help
/// menu.
/// The modal is a sibling so the sticky bar cannot cap its stacking order.
#[component]
pub(crate) fn AppBar(layout_epoch: ReadSignal<u64>) -> Element {
    let mut settings_open = use_signal(|| false);
    let mut feedback_open = use_signal(|| false);
    // The success announcement lives here, outside the dialog: a status
    // region inserted together with its text is not reliably announced, and
    // the dialog closes itself moments after success. This one is always
    // mounted (visually hidden) and only changes its text.
    let mut feedback_notice = use_signal(String::new);
    // Set when a send succeeds; the announcement waits for the dialog to
    // close (see `close_feedback`).
    let mut pending_thanks = use_signal(|| false);
    let close_feedback = move |_| {
        crate::feedback::return_focus_to_help();
        feedback_open.set(false);
        // Announce a success only now: while the dialog is open its modal
        // isolation marks this status region inert, and a live region that
        // changes while inert (or that is already populated when it becomes
        // visible again) is not reliably announced. The short wait lets the
        // release of isolation land before the text changes.
        if std::mem::take(&mut *pending_thanks.write()) {
            spawn(async move {
                sleep_ms(ANNOUNCE_AFTER_CLOSE_MS).await;
                feedback_notice.set(crate::feedback::THANKS.to_string());
            });
        }
    };
    let close_settings = move |_| {
        crate::settings::return_focus_to_gear();
        settings_open.set(false);
    };
    let skew = skew::HELM_BUILD_SKEW.read();
    // A reported stamp is text the helm sent, so it goes through the same
    // display boundary every relayed value does (`peer.rs`): invisible and
    // direction-changing characters become visible escapes, and the element
    // is bidi-isolated. The client build takes the same path for uniformity.
    let version = display_peer(displayed_version(skew.as_ref()));
    // The feedback dialog sends the version as the helm or this build
    // reported it (fitted to its cap) and displays that same string
    // bidi-isolated (`peer-value`) rather than escaped. Invisible formatting
    // characters, if a build stamp ever held any, would therefore be sent
    // without showing; the stamp comes from the user's own helm.
    let sent_version = displayed_version(skew.as_ref()).to_string();

    rsx! {
        div {
            class: "app-bar",
            // Trusted, compile-time markup from the repository's own brand
            // file, never peer data, which is what makes inner HTML safe here.
            span { class: "app-wordmark", dangerous_inner_html: WORDMARK_SVG }
            crate::window_chrome::WindowDragRegion {}
            span {
                class: "app-version peer-value",
                dir: "ltr",
                title: "this client was built as farhelm {skew::CLIENT_BUILD}",
                "{version}"
            }
            button {
                r#type: "button",
                class: "btn btn-neutral app-settings-toggle",
                aria_label: "settings",
                title: "settings",
                aria_haspopup: "dialog",
                onclick: move |_| settings_open.set(true),
                crate::icons::SettingsIcon {}
            }
            HelpMenu {
                layout_epoch,
                on_send_feedback: move |_| {
                    feedback_notice.set(String::new());
                    feedback_open.set(true);
                },
            }
        }
        if settings_open() {
            crate::settings::SettingsDialog { on_close: close_settings }
        }
        if feedback_open() {
            crate::feedback::FeedbackDialog {
                version: sent_version,
                on_sent: move |_| pending_thanks.set(true),
                on_close: close_feedback,
            }
        }
        p { class: "visually-hidden feedback-notice", role: "status", "{feedback_notice}" }
    }
}

/// Render the viewport-fixed helm-wide profile manager beside the session
/// list's New control.
///
/// Keeping this separate from the sticky bar groups helm-wide profile
/// management with the action that consumes it without changing the popup's
/// focus and placement state machine.
///
/// The open signal belongs to the list page so every other floating surface
/// can enforce mutual exclusion. Geometry stays local because this component
/// owns the toggle handle the popup is measured against.
///
/// Focus-out dismissal treats an ambiguous `document.body` destination as a
/// handoff while the popup reports a pending request. Focus-out never restores
/// the toggle, so a real outside destination keeps the focus the user gave it.
#[component]
pub(crate) fn ProfilesControl(
    mut profiles_open: Signal<bool>,
    profiles: CatalogSurface,
    ops: OpLock,
    layout_epoch: ReadSignal<u64>,
) -> Element {
    use_hook(install_profiles_outside_intent_tracking);
    let mut toggle_handle = use_signal(|| None::<Rc<MountedData>>);
    let mut placement = use_signal(|| PanelPlacement::Unmeasured);
    let mut open_generation = use_signal(|| 0_u64);
    // Layout and focus events that arrive while a mutation holds the popup
    // open are obligations, not discarded events. They are reconsidered as
    // soon as the operation lock becomes idle.
    let mut pending_layout_close = use_signal(|| false);
    let mut pending_focus_check = use_signal(|| None::<FocusObligation>);
    let mut running_focus_check = use_signal(|| None::<FocusObligation>);
    // A bridge call may time out without seeing a focus event that arrived
    // while it ran. Retain one notification for that exact observation; a
    // newer intent or opening makes it inert without starting parallel work.
    let mut queued_focus_recheck = use_signal(|| None::<FocusObligation>);
    let mut tentative_keyboard_focus = use_signal(|| None::<FocusObligation>);
    let mut focus_sequence = use_signal(|| 0_u64);
    let focus_coordinator = FocusCoordinator::new(
        use_signal(|| 0_u64),
        use_signal(|| false),
        use_signal(|| false),
        use_signal(|| None::<u64>),
    );
    // Layout events recorded before this popup opened cannot invalidate its
    // freshly measured position. Capture the epoch at each opening because a
    // reactive effect may consume an older event after the click that opens
    // the popup.
    let mut open_layout_epoch = use_signal(|| *layout_epoch.peek());

    let measure = move || {
        let handle = toggle_handle;
        let generation = open_generation();
        spawn(async move {
            for attempt in 0..2 {
                let measured_epoch = *layout_epoch.peek();
                let measured = match handle.peek().clone() {
                    Some(handle) => handle.get_client_rect().await.ok(),
                    None => None,
                };
                hold_profile_measurement_for_test().await;
                if generation != *open_generation.peek() || !*profiles_open.peek() {
                    return;
                }
                if measured_epoch != *layout_epoch.peek() {
                    if attempt == 0 {
                        continue;
                    }
                    if ops.busy_now() {
                        pending_layout_close.set(true);
                    } else {
                        profiles_open.set(false);
                        pending_focus_check.set(None);
                        focus_coordinator.invalidate();
                        focus_profiles_toggle();
                    }
                    return;
                }
                // The stored epoch describes the same sampling interval as
                // these coordinates; a post-await read could stamp stale
                // geometry with a newer layout.
                open_layout_epoch.set(measured_epoch);
                placement.set(match measured {
                    Some(rect) => PanelPlacement::Measured(rect),
                    None => PanelPlacement::Fallback,
                });
                return;
            }
        });
    };

    // The measured coordinates are a snapshot. A scroll or resize closes the
    // popup rather than leaving it visibly detached from its sticky trigger.
    use_effect(move || {
        let epoch = layout_epoch();
        if !*profiles_open.peek()
            || *placement.peek() == PanelPlacement::Unmeasured
            || epoch == *open_layout_epoch.peek()
        {
            return;
        }
        if ops.busy_now() {
            pending_layout_close.set(true);
        } else {
            profiles_open.set(false);
            pending_focus_check.set(None);
            focus_coordinator.invalidate();
            focus_profiles_toggle();
        }
    });

    // Revisit events that were deliberately deferred while a mutation kept
    // the form mounted. A layout change always dismisses; focus-out dismisses
    // only if focus is still outside when the lock releases.
    use_effect(move || {
        let busy = ops.busy();
        let focus_obligation = pending_focus_check();
        if !*profiles_open.peek() {
            return;
        }
        if !busy && *pending_layout_close.peek() {
            pending_layout_close.set(false);
            pending_focus_check.set(None);
            profiles_open.set(false);
            focus_coordinator.invalidate();
            focus_profiles_toggle();
            return;
        }
        if let Some(obligation) = focus_obligation
            && *running_focus_check.peek() != Some(obligation)
        {
            running_focus_check.set(Some(obligation));
            spawn(async move {
                let focus =
                    settled_profile_focus(focus_coordinator, obligation.trusted_outside).await;
                if obligation.opening != *open_generation.peek()
                    || *pending_focus_check.peek() != Some(obligation)
                {
                    if *running_focus_check.peek() == Some(obligation) {
                        running_focus_check.set(None);
                    }
                    return;
                }
                let recheck = *queued_focus_recheck.peek() == Some(obligation);
                if recheck {
                    queued_focus_recheck.set(None);
                }
                match focus {
                    ProfileFocus::Unknown => {
                        // No evidence means the obligation remains unresolved.
                        // A later focus event (or operation-lock transition)
                        // can reconsider it; a timer must neither keep a dead
                        // bridge busy nor discard the user's outside choice.
                        if recheck {
                            pending_focus_check.set(Some(FocusObligation {
                                observation: obligation.observation + 1,
                                ..obligation
                            }));
                        }
                    }
                    _ if obligation.trusted_outside && ops.busy_now() => {
                        // Keep the exact token pending. The operation lock's
                        // idle transition reruns this effect, and completion
                        // focus can see the same sequence through the shared
                        // coordinator instead of reconstructing provenance.
                    }
                    _ if obligation.trusted_outside => {
                        pending_focus_check.set(None);
                        focus_coordinator.clear_outside_obligation(obligation.sequence);
                        profiles_open.set(false);
                        focus_coordinator.invalidate();
                    }
                    ProfileFocus::Transit => {
                        // Replacing Delete with confirmation removes its
                        // focused node. If the bounded Cancel placement
                        // misses, body focus is still no outside choice.
                        // Trusted outside intent was handled above; retain
                        // the popup so confirmation remains reachable. Keep
                        // this obligation dormant: a later body-to-outside
                        // focus move has no popup focusout of its own.
                        if recheck {
                            pending_focus_check.set(Some(FocusObligation {
                                observation: obligation.observation + 1,
                                ..obligation
                            }));
                        }
                    }
                    ProfileFocus::Inside => {
                        pending_focus_check.set(None);
                        focus_coordinator.clear_outside_obligation(obligation.sequence);
                    }
                    ProfileFocus::Outside if ops.busy_now() => {
                        // Programmatic focus is not user intent. A busy popup
                        // keeps its in-flight destination mounted and lets the
                        // completion request preserve any outside active control.
                        pending_focus_check.set(None);
                        focus_coordinator.clear_outside_obligation(obligation.sequence);
                    }
                    ProfileFocus::Outside => {
                        pending_focus_check.set(None);
                        focus_coordinator.clear_outside_obligation(obligation.sequence);
                        profiles_open.set(false);
                        focus_coordinator.invalidate();
                    }
                }
                if *running_focus_check.peek() == Some(obligation) {
                    running_focus_check.set(None);
                }
            });
        }
    });

    // Both DOM relays end here, where provenance and the sequence become one
    // obligation before any classifier starts. The trusted sequence is also
    // mirrored through the coordinator because mutation completion lives in
    // the popup child and must yield to this exact pending outside choice.
    let mut record_focus_out = move |trusted_outside: bool| {
        focus_sequence += 1;
        let obligation = FocusObligation {
            opening: open_generation(),
            sequence: *focus_sequence.peek(),
            trusted_outside,
            observation: 0,
        };
        pending_focus_check.set(Some(obligation));
        focus_coordinator.set_outside_obligation(if trusted_outside {
            Some(obligation.sequence)
        } else {
            None
        });
    };

    // Outside-to-outside focus changes do not emit another popup focusout.
    // Reconsider the pending intent itself, preserving its trusted provenance.
    // A running observation normally samples the new destination, but an
    // unanswered bridge call cannot do so. Keep one notification to consume
    // after Unknown, without launching competing observers for the same intent.
    let mut reconsider_focus_out = move || {
        let pending = *pending_focus_check.peek();
        if let Some(mut obligation) = pending {
            if *running_focus_check.peek() == Some(obligation) {
                // A held browser eval can outlive its Rust observer. Tests
                // need a receipt from this owner to prove the recovery event
                // arrived before that observer retired, not merely before
                // the browser promise was released.
                document::eval(
                    "if (window.__farhelmTestProfiles) \
                     window.__farhelmTestProfiles.focusRecheckWhileRunning = true;",
                );
                queued_focus_recheck.set(Some(obligation));
                return;
            }
            obligation.observation += 1;
            pending_focus_check.set(Some(obligation));
        } else {
            // WebKit may remove the focused control without reporting a
            // popup focusout. A later body-to-outside focusin is then the
            // first evidence of departure, not a recheck of an old token.
            focus_sequence += 1;
            pending_focus_check.set(Some(FocusObligation {
                opening: open_generation(),
                sequence: *focus_sequence.peek(),
                trusted_outside: false,
                observation: 0,
            }));
        }
    };

    // Keyboard provenance is reserved at keydown, before the browser moves
    // focus out of the document and potentially stops producing focus events.
    // The commit relay publishes this same token only after that move settles.
    let mut reserve_keyboard_focus = move || {
        focus_sequence += 1;
        tentative_keyboard_focus.set(Some(FocusObligation {
            opening: open_generation(),
            sequence: *focus_sequence.peek(),
            trusted_outside: true,
            observation: 0,
        }));
    };
    let mut commit_keyboard_focus = move || {
        let Some(obligation) = *tentative_keyboard_focus.peek() else {
            return;
        };
        tentative_keyboard_focus.set(None);
        if obligation.opening != open_generation() || !profiles_open() {
            return;
        }
        pending_focus_check.set(Some(obligation));
        focus_coordinator.set_outside_obligation(Some(obligation.sequence));
    };

    // Both a popup key event and Escape during failed-placement body focus
    // dismiss this exact opening through the same synchronous operation gate.
    let dismiss_profiles = Callback::new(move |()| {
        if !ops.busy_now() {
            profiles_open.set(false);
            pending_focus_check.set(None);
            focus_coordinator.invalidate();
            focus_profiles_toggle();
        }
    });

    rsx! {
        Fragment {
            button {
                r#type: "button",
                class: "btn btn-neutral profiles-toggle",
                aria_expanded: profiles_open(),
                disabled: ops.busy(),
                onmounted: move |element| toggle_handle.set(Some(element.data())),
                onclick: move |_| {
                    if ops.busy_now() {
                        return;
                    }
                    if profiles_open() {
                        profiles_open.set(false);
                        pending_focus_check.set(None);
                        focus_coordinator.invalidate();
                    } else {
                        open_generation += 1;
                        focus_coordinator.invalidate();
                        placement.set(PanelPlacement::Unmeasured);
                        pending_layout_close.set(false);
                        pending_focus_check.set(None);
                        profiles.request(Trigger::Explicit);
                        profiles_open.set(true);
                        measure();
                    }
                },
                "profiles"
            }
            if profiles_open() {
                div {
                    class: "profiles-popover",
                    style: profiles_popover_placement_style(placement()),
                    onkeydown: move |evt| {
                        if evt.key() == Key::Escape && !evt.is_composing() && !ops.busy_now() {
                            evt.prevent_default();
                            dismiss_profiles.call(());
                        }
                    },
                    button {
                        r#type: "button",
                        class: "profiles-escape-relay",
                        hidden: true,
                        tabindex: "-1",
                        onclick: move |_| dismiss_profiles.call(()),
                    }
                    button {
                        r#type: "button",
                        class: "profiles-focusout-relay",
                        hidden: true,
                        tabindex: "-1",
                        onclick: move |_| record_focus_out(false),
                    }
                    button {
                        r#type: "button",
                        class: "profiles-trusted-focusout-relay",
                        hidden: true,
                        tabindex: "-1",
                        onclick: move |_| record_focus_out(true),
                    }
                    button {
                        r#type: "button",
                        class: "profiles-focus-recheck-relay",
                        hidden: true,
                        tabindex: "-1",
                        onclick: move |_| reconsider_focus_out(),
                    }
                    button {
                        r#type: "button",
                        class: "profiles-focusin-relay",
                        hidden: true,
                        tabindex: "-1",
                        onclick: move |_| {
                            tentative_keyboard_focus.set(None);
                            pending_focus_check.set(None);
                            focus_coordinator.set_outside_obligation(None);
                        },
                    }
                    button {
                        r#type: "button",
                        class: "profiles-tab-start-relay",
                        hidden: true,
                        tabindex: "-1",
                        onclick: move |_| reserve_keyboard_focus(),
                    }
                    button {
                        r#type: "button",
                        class: "profiles-tab-commit-relay",
                        hidden: true,
                        tabindex: "-1",
                        onclick: move |_| commit_keyboard_focus(),
                    }
                    ProfilesPopup {
                        surface: profiles,
                        ops,
                        focus_coordinator,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `None` is both the pre-reply state and the healthy steady state after
    /// agreeing replies; either way the client build is the exact answer, so
    /// there must be no loading placeholder and no wait for the network.
    #[farhelm_testtrace::test]
    fn no_skew_shows_the_client_build() {
        assert_eq!(displayed_version(None), skew::CLIENT_BUILD);
    }

    /// A silent helm is already represented by the skew system; the app bar
    /// keeps the client build visible because there is no remote stamp to show.
    #[farhelm_testtrace::test]
    fn silent_skew_shows_the_client_build() {
        assert_eq!(displayed_version(Some(&Skew::Silent)), skew::CLIENT_BUILD);
    }

    /// Once the helm reports a different build, the readout must expose that
    /// exact stamp so the user can identify the remote process behind the skew.
    #[farhelm_testtrace::test]
    fn reported_skew_shows_the_helm_build() {
        let skew = Skew::Reported("0.9.0-rc.1".to_string());
        assert_eq!(displayed_version(Some(&skew)), "0.9.0-rc.1");
    }

    /// The profile manager must stay inert until its trigger has been
    /// measured, then retain a viewport-bounded fallback if measurement is
    /// unavailable. This prevents the fixed panel flashing at the document
    /// origin or becoming unreachable on renderers that cannot report a rect.
    #[farhelm_testtrace::test]
    fn profile_popup_placement_is_hidden_until_measured_and_has_a_safe_fallback() {
        assert_eq!(
            profiles_popover_placement_style(PanelPlacement::Unmeasured),
            "opacity: 0; pointer-events: none;"
        );
        let fallback = profiles_popover_placement_style(PanelPlacement::Fallback);
        assert!(fallback.contains("opacity: 1; pointer-events: auto"));
        assert!(fallback.contains("max-width: min(320px"));
        assert!(fallback.contains("max-height: calc(100vh - 16px)"));
    }
}
