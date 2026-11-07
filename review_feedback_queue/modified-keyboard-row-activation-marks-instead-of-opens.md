# Modified keyboard row activation marks instead of opens

Reviewed commit: 2d46033f3f3e89e97bbb9f8e110d8e5a93a20a7b

## TLDR

Shift-modified keyboard activation can mark sessions instead of opening them in the inspected browser sources; native
platform coverage remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-ui/src/list/row.rs:1776 at 2d46033f3f3e89e97bbb9f8e110d8e5a93a20a7b`.

Source assessment (read with the later corrections below) (review record g015/c2, event 26):

Reviewed Farhelm commit 2d46033f3f3e89e97bbb9f8e110d8e5a93a20a7b. TLDR: activating a focused session with Shift held can
mark a range instead of opening the session; this is a keyboard route, separate from g086-f3 macOS Control-click pointer
delivery. Actual checks: e2e/package-lock.json locks Playwright 1.64.0. Its primary browsers.json
(https://github.com/microsoft/playwright/blob/v1.64.0/packages/playwright-core/browsers.json) names Chromium
156.0.8078.4 and WebKit 2370/27.2. Chromium at that exact version: html_button_element.cc:293 calls
HandleKeyboardActivation; html/html_element.cc:4276-4305 sends Return keypress or active Space keyup to
DispatchSimulatedClick with the original event; core/events/simulated_event_util.cc:85-112 copies underlying
UIEventWithKeyState modifiers. Immutable version URLs are under
https://github.com/chromium/chromium/blob/156.0.8078.4/third_party/blink/renderer/. Playwright's
browser_patches/webkit/UPSTREAM_CONFIG.sh at v1.64.0 pins 56453fdfe0b0ca6258c23e6453b34f70b885d13f. At
https://github.com/WebKit/WebKit/blob/56453fdfe0b0ca6258c23e6453b34f70b885d13f/Source/WebCore/html/HTMLButtonElement.cpp:311-334
Return/Space likewise passes KeyboardEvent, and dom/SimulatedClick.cpp:100-105 preserves modifierKeys. Neither button
path excludes Shift. Farhelm row.rs:1776 passes all click modifiers to marking.rs:38-44, where Shift means Range.
row.rs:1702-1710 only handles Escape and does not cancel modified Enter. UI Events
https://www.w3.org/TR/uievents/#event-type-keydown specifies Enter/Space activation dispatching click. Thus a delivered
Shift+Enter/held-Shift Space sequence has a source-backed route; consider ignoring marking modifiers for
keyboard-generated click (detail=0) while preserving pointer gestures. Ordinary macOS Command+Space is excluded: Apple's
https://support.apple.com/en-euro/guide/mac-help/mh26783/mac assigns it to Spotlight, so no browser delivery is
presumed. Suggested bucket other, confidence confirmed for the inspected browser-source mechanism. Remaining
native-platform coverage is unverified after investigation: no installed native WebKit build or OS key-routing
experiment was inspected; its precise version/overrides could differ. A source match for that build or a later
authorized native keyboard regression would establish its platform extent. No runtime/browser/process experiment
performed. proposed_drop:false; possible_cover: bulk UI keyboard owner, parent checks overlap. No destructive dispatch
or highest consequence shown.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g015/c2`; supporting records
`g086/f4`, `g087/x2`. The retained private log preserves earlier wording and its corrections. This record does not claim
runtime reproduction.
