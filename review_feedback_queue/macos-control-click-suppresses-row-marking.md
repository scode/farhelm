# Macos control click suppresses row marking

Reviewed commit: a9c7d4da4d043329249a9d99a82bd7699f4ba024

## TLDR

On the inspected macOS Chromium route, Control-click is consumed as a context-menu gesture instead of marking the
session.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-ui/src/list/row.rs:1771-1776 at a9c7d4da4d043329249a9d99a82bd7699f4ba024`.

Source assessment (read with the later corrections below) (review record g086/f3, event 15):

Reviewed commit: a9c7d4da4d043329249a9d99a82bd7699f4ba024. TLDR: Control-click does not toggle session marks in
Chromium-based browsers on macOS, despite the promised cross-platform modifier behavior. Command-click remains
available. Suggested bucket: other, because this is a convenience/input portability defect with no established security
or work-loss route. Confidence: confirmed by primary engine source for the Chromium route. proposed_drop: false.
possible_cover: unknown; coverage sources intentionally not searched. Farhelm mechanism: row.rs:1771-1775 prevents
default only for Shift; :1776 dispatches the gesture only from onclick. marking.rs:36-45 recognizing CONTROL cannot help
when no click arrives. SPEC.md Session list :1792-1798 promises Cmd+click or Ctrl+click on every platform. Decisive
immutable primary source: Chromium commit 9e54c5b962aa31c56d66ca516415942ed347ce4c (file's latest commit dated
2026-10-15),
https://github.com/chromium/chromium/blob/9e54c5b962aa31c56d66ca516415942ed347ce4c/third_party/blink/renderer/core/input/mouse_event_manager.cc#L332-L356.
DispatchMouseClickIfNeeded sets context_menu_event on macOS for left-button plus Control and explicitly requires
!context_menu_event before dispatching click. This is sufficient source evidence without a browser experiment.
Engine/platform qualification: this finding does not claim all macOS engines fail. Primary browser-implementer report
https://lists.w3.org/Archives/Public/public-webapps-github/2020May/1062.html (May 2020, now tracked at
https://github.com/w3c/pointerevents/issues/581) specifically reports WebKit delivering Control-click click, Blink
suppressing it, and Firefox behavior changing at that time. MDN
https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/ctrlKey has a generic interception warning but is less
precise than the engine source. Exact release desktop investigation: pinned Cargo.lock selects dioxus-desktop 0.7.10;
primary registry source src/webview.rs:441-455 installs a document-level bubbling contextmenu listener calling
preventDefault when disable_context_menu is true. Farhelm desktop.rs:234 enables that in release. It does not filter a
native mouse event or synthesize click. Modern primary WebKit source at immutable
9c374e77a488b33ed72c0c871e3b9745d018570a (2026-11-05), Source/WebKit/WebProcess/WebPage/WebFrame.cpp:1574-1640, handles
the context gesture and mouse release separately, with release click invalidation only when the platform reports gesture
cancellation; Source/WebCore/page/EventHandler.cpp:2519-2552 dispatches primary click by left button, with no
Blink-style Control exclusion. Thus the initial unsupported suggestion that release desktop necessarily loses
Control-click is withdrawn; release context-menu cancellation is material counterevidence, not a reason to generalize
Blink behavior to WKWebView. Exact installed macOS/WKWebView and Safari versions are not pinned in this review, and no
native reproduction was run, so actual delivery on every shipped OS version remains unverified after investigation and
is not alleged as an additional desktop defect. This uncertainty does not weaken the independently established Chromium
branch. Browser-test modifier clicks (e2e/tests/bulk-session-actions.spec.ts:431-436) do not establish native Mac
delivery. Temporal check: tip 8a8c9579e04299410652f0e4681975a7198dfc7b row.rs:1771-1776 retains the same onclick-only
handler; no later desktop/Cargo.lock changes repair it. Fix direction: support the macOS Control gesture at a suitable
pointer/contextmenu boundary while avoiding duplicate toggles on engines that also send click; verify Control and
Command on actual Mac Chromium and WKWebView. No runtime tests, browser/process/network experiments or live-install
actions occurred; external operations were primary source/docs fetches only.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g086/f3`; supporting records
`g087/x1`. The retained private log preserves earlier wording and its corrections. This record does not claim runtime
reproduction.
