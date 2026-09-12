//! The sticky sidebar bar: helm build identity and the helm-wide profiles popup.
//!
//! The helm's reported build is available through the existing skew latch, so
//! this surface stays a read-only view of that signal. It deliberately shows
//! the client build until a reported mismatch gives it a more useful helm
//! value; a page never needs a loading placeholder to identify its bundle.

use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;
use web_time::Instant;

use crate::menu_panel::PanelPlacement;
use crate::ops::OpLock;
use crate::peer::display_peer;
use crate::profiles::{
    CatalogSurface, FOCUS_SETTLE_MS, FOCUS_TRANSIT_GRACE_MS, FocusCoordinator, ProfilesPopup,
};
use crate::reader::{Trigger, finish_before, sleep_ms};
use crate::skew::{self, Skew};

/// Keep the popup inside the viewport while anchoring its left edge to the
/// app-bar trigger whenever the viewport has room.
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

/// Render the sticky sidebar bar and its viewport-fixed profile manager.
///
/// The open signal belongs to the list page so every other floating surface
/// can enforce mutual exclusion. Geometry stays local because this component
/// owns the toggle handle the popup is measured against.
///
/// Focus-out dismissal treats an ambiguous `document.body` destination as a
/// handoff while the popup reports a pending request. Focus-out never restores
/// the toggle, so a real outside destination keeps the focus the user gave it.
#[component]
pub(crate) fn AppBar(
    mut profiles_open: Signal<bool>,
    profiles: CatalogSurface,
    ops: OpLock,
    layout_epoch: ReadSignal<u64>,
) -> Element {
    use_hook(install_profiles_outside_intent_tracking);
    let skew = skew::HELM_BUILD_SKEW.read();
    // A reported stamp is text the helm sent, so it goes through the same
    // display boundary every relayed value does (`peer.rs`): invisible and
    // direction-changing characters become visible escapes, and the element
    // is bidi-isolated. The client build takes the same path for uniformity.
    let version = display_peer(displayed_version(skew.as_ref()));
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
                    ProfileFocus::Inside => {
                        pending_focus_check.set(None);
                        focus_coordinator.clear_outside_obligation(obligation.sequence);
                    }
                    ProfileFocus::Outside | ProfileFocus::Transit if ops.busy_now() => {
                        // Programmatic focus is not user intent. A busy popup
                        // keeps its in-flight destination mounted and lets the
                        // completion request preserve any outside active control.
                        pending_focus_check.set(None);
                        focus_coordinator.clear_outside_obligation(obligation.sequence);
                    }
                    ProfileFocus::Outside | ProfileFocus::Transit => {
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

    rsx! {
        div {
            class: "app-bar",
            button {
                r#type: "button",
                class: "btn profiles-toggle",
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
            crate::window_chrome::WindowDragRegion {}
            span {
                class: "app-version peer-value",
                dir: "ltr",
                title: "this client was built as farhelm {skew::CLIENT_BUILD}",
                "{version}"
            }
        }
        if profiles_open() {
            div {
                class: "profiles-popover",
                style: profiles_popover_placement_style(placement()),
                onkeydown: move |evt| {
                    if evt.key() == Key::Escape && !ops.busy_now() {
                        evt.prevent_default();
                        profiles_open.set(false);
                        pending_focus_check.set(None);
                        focus_coordinator.invalidate();
                        focus_profiles_toggle();
                    }
                },
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
