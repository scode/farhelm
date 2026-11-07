# Drag preview neighbor diverges from drop unresolved

Reviewed commit: 720e18bf8e78339e59bb876af75a621bfd1a69e9

## TLDR

The drag preview and final drop can use different neighboring rows. A complete supported native ordering producing the
mismatch remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location:
`crates/farhelm-ui/assets/pin-order.js:107-131,152-158,175-176 at 720e18bf8e78339e59bb876af75a621bfd1a69e9`.

Source assessment (read with the later corrections below) (review record g085/l1, event 18):

Reviewed commit: 720e18bf8e78339e59bb876af75a621bfd1a69e9. TLDR: Scrolling or remotely reordering pinned rows during a
stationary drag may leave the visible insertion line at a different neighbor from the placement committed on release.
Unverified after investigation for the exact shipped-engine visible sequence; the absent specification guarantees and
inspected implementation paths are confirmed. Repository mechanism: sample updates the node-owned data-pin-drop line
only on pointermove/up (pin-order.js:107-131,152-164); activeListeners at 175-176 has no scroll/layout observer.
app.css:2643-2648 paints the line on the old neighbor. The sidebar scrolls (635-647); the wheel handler only refreshes
the hold clock (pin_drag.rs:80-87; list/view.rs:3220-3256), and rows.rs:545-560 allows original pins to adopt incoming
manual order during a hold. pointerup recomputes placement, cancels and sends synchronously (152-158), so its corrected
target cannot be shown before commitment. Example to verify: with scrollable content and four eligible visible pins
A/B/C/D, drag D before B, scroll one row while the pointer stays fixed, then release; absent an extra real
pointermove/cancellation, the prior line remains before B while release computes before C. Browser/spec investigation
completed this synthesis turn: (1) Official Pointer Events, section 3.1.4 Boundary events caused by layout changes,
https://www.w3.org/TR/pointerevents/ , delivered web lines 665-671 plus capture context 627-638, specifies boundary
notifications rather than pointermove for a stationary unchanged device; capture keeps events within its target. There
is no guaranteed repairing pointermove before pointerup. (2) Chromium primary source at revision
06445f6f55bb929daafd46bb6a73a82802f1fda3:
https://chromium.googlesource.com/chromium/src/+/06445f6f55bb929daafd46bb6a73a82802f1fda3/third_party/blink/renderer/core/input/mouse_event_manager.cc
, RecomputeMouseHoverState, delivered web lines 360-395, marks synthetic moves kRelativeMotionEvent even while pressed;
https://chromium.googlesource.com/chromium/src/+/06445f6f55bb929daafd46bb6a73a82802f1fda3/third_party/blink/renderer/core/input/pointer_event_manager.cc
, SendMousePointerEvent, delivered web lines 998-1073, suppresses that flag before DOM pointer dispatch after
capture/boundary processing. (3) Immutable official WebKit source
https://raw.githubusercontent.com/WebKit/WebKit/f2f0dead988af103f8106b25078617041fa67a03/Source/WebCore/page/EventHandler.cpp
, fetched read-only with curl after the web tool cache miss: updateMouseEventTargetAfterLayoutIfNeeded lines 3306-3360
retains the capturing element and only processes changed-target boundaries; dispatchFakeMouseMoveEventSoon lines
4166-4188 returns while pressed; fakeMouseMoveEventTimerFired 4212 onward asserts the unpressed state. This verifies the
substance of the earlier cached main source using an immutable snapshot; earlier cached line numbers do not apply to
this commit. (4) Official HTML Standard Event loops / Processing model,
https://html.spec.whatwg.org/multipage/webappapis.html#event-loop-processing-model , delivered web lines 2856-2898 and
2959-2966, allows consecutive tasks without a rendering update. Even an additional pointermove would not itself
guarantee that its updated feedback paints before commitment. Checks settle the universal event/paint questions and show
inspected engine paths do not supply the assumed repair. Remaining exact premise: whether a particular Farhelm-supported
installed browser build and platform input bridge uses these paths for a physically still captured mouse, or supplies an
additional real pointermove or cancellation before release. The repository's package declarations do not identify
arbitrary user browser/WKWebView builds, the inspected snapshots have not been mapped to an installed deployment, and
source-only forbids the runtime reproduction that would settle that last platform-specific observation. Browser-version
mapping plus a focused event/paint trace of the stated sequence would settle it. SPEC.md Session list says release moves
relative to the pin at the drop line; SPEC_impl.md Shared pins requires current geometry, so this concerns stale
feedback rather than opposing current-geometry sampling. Suggested fix if reproduced: update or cancel the armed gesture
when scrolling/layout/order changes. Suggested bucket other: misleading feedback and an easily recoverable pin-order
change, with no highest route. Confidence likely for the installed-visible symptom, confirmed for the inspected source
omission and absent guarantees. proposed_drop false. possible_cover unknown; excluded coverage sources were not
searched. Temporal status unchanged: JS and Rust bridge blobs are identical through pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b; focused tip hold, admission, held-order and CSS checks found no repair.
Synthesis distinguishes l1 from c1: Rust and the API preserve the captured intent correctly, while this concern occurs
before capture of the final JS drop intent. No runtime or browser experiment was run.

Later qualification (review record g085/l1, event 24):

Reviewed full hash 720e18bf8e78339e59bb876af75a621bfd1a69e9; no relevant repair through pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b. This event supplements, rather than replaces the detailed primary-source
investigation in the previous l1 event. TLDR and mechanism remain stale node-owned drop feedback after scrolling/layout
change, followed by a fresh release target; bucket other; confidence likely for the installed-visible symptom, confirmed
for absent specification guarantees and the inspected engine omissions. Unverified after investigation remains the exact
disposition: source checks of official Pointer Events, HTML event loops, Chromium
06445f6f55bb929daafd46bb6a73a82802f1fda3 and WebKit f2f0dead988af103f8106b25078617041fa67a03 do not identify a
particular deployed build/platform's extra real input/cancellation. No new evidence permits repeating or relabeling that
uncertainty. Discovery interface qualification: the complete One GUI at a time section read at
a9c7d4da4d043329249a9d99a82bd7699f4ba024 makes multi-GUI races best effort and accepts ordinary actions within a
sub-second pre-redraw window. Do not rely on a second GUI's remote reorder alone as independent severity evidence. The
same-client wheel sequence remains separate: if the prior line persists, leaving the pointer stationary for over a
second after scrolling before release tests feedback lasting beyond the short pre-redraw acceptance window. That exact
observable remains subject to the already-recorded installed-engine premise; no new runtime claim is made. Supplied
g083:c5 menu-focus and g084:c4 relative-store refutations do not refute this earlier-stage JS feedback concern. Supplied
g086:f1 is a separate known selection issue and must not be re-counted here. proposed_drop false; possible_cover
unknown, excluded coverage sources not searched. This is a qualification of the existing l1, not a novel discovery
finding.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g085/l1`; supporting records
`g082/g085:l1`, `g083/c7`. The retained private log preserves earlier wording and its corrections. This record does not
claim runtime reproduction.
