// The desktop window's press bridge (farhelm-ui/src/window_chrome.rs).
//
// WHY THIS FILE EXISTS: the macOS window's title band is made of drag
// surfaces (the sidebar's app bar, the session top row, and a stand-in
// strip when no session is open), each with one Rust onmousedown that drags
// or zooms the window. That handler learns about presses through Dioxus's
// event transport, which serializes mouse coordinates, buttons, and
// modifiers but NOT the press's click count (dioxus-interpreter-js 0.7.10's
// serializeMouseEvent has no `detail` field) and nothing about which
// element was pressed. So it cannot tell a double-click, nor a press on
// empty background from a press on a button inside the same surface. This
// file carries both facts across the transport gap: a capture-phase
// listener classifies every press inside a surface and synchronously POSTs
// a report to a Rust asset-handler route, ahead of the interpreter's own
// event send for the same press. The Rust handler then pairs the two by
// ORDER — the POST's response necessarily precedes the interpreter's send
// because both are synchronous XHRs on this one JS thread — and decides
// drag, maximize-toggle or nothing, with no timing heuristic.
//
// The click count comes from AppKit: it is assumed to classify the press
// (NSEvent.clickCount), and WebKit copies that onto every DOM mousedown's
// `detail` (see window_chrome.rs for the conversion chain with sources, and
// for why the AppKit half is an assumption pending native validation). The
// count alone does not earn a zoom: WebKit does not reset it for a
// different DOM element, so a press on header text followed by a fast
// background press can carry count 2. Every report therefore carries an
// eligibility bit: whether the immediately preceding primary press landed
// on the background of the same connected surface. An intervening press
// elsewhere, a control press, a non-primary press, or a surface the page
// has since replaced clears it, and the Rust side treats an ineligible
// repeat as no action at all (no zoom AND no drag). The third field says
// whether this press landed on empty background (`classifyPress` below);
// only those act. The DECISION stays in the Rust handler.
//
// WHAT THIS FILE DOES NOT DO: it never prevents default handling, never
// stops propagation, and never acts on the press. A press inside a surface
// always POSTs, even on a control, because that surface's handler receives
// it by bubbling and must consume a report of its own; a press outside
// every surface only invalidates the tracker and owes no POST.
//
// INERTNESS: the script is rendered only in the desktop shell, and even
// there it no-ops unless `window.interpreter` exists (set solely by the
// desktop index template — the web build has no such global) AND the press
// lands in a surface inside the macOS shell class (so nothing on Linux or
// in ordinary browsers). Browser tests that force the shell class onto
// production markup still take the no-interpreter path, and load this file
// directly to run `classifyPress`. Every failure below is swallowed: a dead
// bridge must degrade to missing reports, which the Rust side reads as no
// action, never break the page. That covers missing reports against an
// empty slot; it is not a claim about every partial transport failure (see
// window_chrome.rs).
(function () {
  var ROUTE = "fh-click-detail";
  var HEADER = "x-fh-click-detail";

  // Derive this route's endpoint from the interpreter's own events URL.
  // The injected base URI carries a trailing slash, so the events path
  // ends in a doubled slash (`dioxus://index.html//__events`); routing
  // splits the path on `/` without trimming, so the doubled slash must
  // be collapsed or the first segment reads empty and the POST misses
  // the registered handler. Pure, so js-tests can pin it directly.
  function detailEndpointFor(eventsPath) {
    if (typeof eventsPath !== "string") {
      return null;
    }
    var base = eventsPath.replace(/\/+__events$/, "");
    if (base === eventsPath) {
      return null;
    }
    return base + "/" + ROUTE;
  }

  // Render one bridge report: the press's DOM detail, whether an eligible
  // preceding primary press on the same connected surface backs a repeat,
  // and whether this press landed on empty surface background (only those
  // drag or zoom). One header value, so the fields arrive atomically.
  function reportFor(detail, eligible, drag) {
    return String(detail) + ":" + (eligible ? "1" : "0") + ":" + (drag ? "1" : "0");
  }

  // What inside a drag surface keeps its own interactions instead of moving
  // the window: controls, anything focusable or with a role, and text. The
  // wordmark is the exception, since it is the window's title; it is
  // checked first because it is a span.
  var NOT_BACKGROUND =
    "button, input, textarea, select, a[href], label, summary, dialog, [contenteditable], " +
    "[tabindex]:not([tabindex='-1']), [role], p, span, dt, dd, li, h1, h2, h3, h4, h5, h6";

  // Whether a press at viewport point (x, y) on `target` inside `surface`
  // landed on empty background. Two cases need the point, not just the
  // target. A surface can lay out descendants beyond its own box, such as a
  // session's confirmation panel hanging below the top row; only the band
  // itself is a title bar, so a press outside the surface's box never
  // drags. And hit testing skips `inert` elements (a stale session's tab
  // strip), so a press over a visible but inert tab targets the surface
  // behind it; such a press must stay inactive rather than move the window.
  function isEmptyBackground(target, surface, x, y) {
    if (typeof x !== "number" || typeof y !== "number") {
      return false;
    }
    var within = function (box) {
      return x >= box.left && x < box.right && y >= box.top && y < box.bottom;
    };
    if (!within(surface.getBoundingClientRect())) {
      return false;
    }
    var inert = surface.querySelectorAll("[inert]");
    for (var j = 0; j < inert.length; j++) {
      if (within(inert[j].getBoundingClientRect())) {
        return false;
      }
    }
    // A popover a surface lays out over the band (a confirmation panel's top
    // overlaps the row it hangs from) is not band background either: any
    // absolutely or fixed positioned element between the target and the
    // surface keeps its own interactions.
    for (var node = target; node && node !== surface; node = node.parentElement) {
      var position = getComputedStyle(node).position;
      if (position === "absolute" || position === "fixed") {
        return false;
      }
    }
    if (target.closest(".app-wordmark")) {
      return true;
    }
    var blocker = target.closest(NOT_BACKGROUND);
    if (blocker && surface.contains(blocker)) {
      return false;
    }
    for (var i = 0; i < target.childNodes.length; i++) {
      var node = target.childNodes[i];
      if (node.nodeType === 3 && /\S/.test(node.textContent)) {
        return false;
      }
    }
    return true;
  }

  // Classify one press on `target` at viewport point (x, y): the drag
  // surface it landed in (null outside every surface or outside the macOS
  // shell, where nothing may drag), and whether it landed on that surface's
  // empty background. The press listener and the browser test
  // (e2e/tests/window-chrome.spec.ts) share this, so the test measures the
  // page's real decision.
  function classifyPress(target, x, y) {
    var surface = target instanceof Element ? target.closest(".window-drag-surface") : null;
    if (!surface || !surface.closest(".macos-window")) {
      return { surface: null, drag: false };
    }
    return { surface: surface, drag: isEmptyBackground(target, surface, x, y) };
  }

  // Track whether the immediately preceding primary press landed on the
  // background of the same surface, so a cross-target pair can never borrow
  // a repeat count.
  // Pure state machine over mousedowns in dispatch order; the listener
  // below feeds it every press, and js-tests drive it directly.
  function createPressEligibility() {
    var lastSurface = null;
    // Observe one mousedown. `surface` is the surface whose background was
    // pressed, or null for any other press (a control, text, or outside
    // every surface); `primary` is whether it used the primary button.
    // Returns this press's eligibility, or null for a press with no
    // surface. The listener decides separately whether a report is owed.
    function observe(surface, primary) {
      if (!surface || !primary) {
        lastSurface = null;
        return surface ? false : null;
      }
      var eligible = lastSurface === surface && surface.isConnected;
      lastSurface = surface;
      return eligible;
    }
    return { observe: observe };
  }

  function endpoint() {
    var interpreter = window.interpreter;
    if (!interpreter) {
      return null;
    }
    return detailEndpointFor(interpreter.eventsPath);
  }

  // Guarded on `window` existing: under `require()` (js-tests) there is
  // none, and the file must still load so its pure half is testable.
  if (typeof window !== "undefined") {
    if (!window.__farhelmClickDetailInstalled) {
      window.__farhelmClickDetailInstalled = true;
      var eligibility = createPressEligibility();
      // Capture, not bubble: the interpreter delegates every mousedown
      // to ONE bubble-phase listener on its root, so a document-level
      // capture listener runs strictly before it for the same dispatch
      // (DOM propagation order, not registration order). The synchronous
      // POST below therefore always lands — and is answered — before the
      // interpreter's synchronous send for the same press begins.
      document.addEventListener(
        "mousedown",
        function (event) {
          try {
            var primary = event.button === 0;
            var press = classifyPress(event.target, event.clientX, event.clientY);
            // A press outside every surface only invalidates: no surface
            // handler will consume a report for it.
            if (!press.surface) {
              eligibility.observe(null, primary);
              return;
            }
            // Every press inside a surface POSTs, because the surface's
            // handler receives them all; only background presses may drag.
            var drag = primary && press.drag;
            var eligible = eligibility.observe(drag ? press.surface : null, primary) === true;
            var url = endpoint();
            if (!url) {
              return;
            }
            // Synchronous, mirroring the interpreter's own event send:
            // the response is the ordering edge the Rust side pairs on.
            // The report rides a header rather than a body for the same
            // reason the interpreter's event send does — its Android
            // workaround put bodies in headers, and headers provably
            // arrive (every `__events` send and the file-dialog request
            // depend on one). No body is sent.
            var xhr = new XMLHttpRequest();
            xhr.open("POST", url, false);
            xhr.setRequestHeader(HEADER, reportFor(event.detail, eligible, drag));
            xhr.send();
          } catch (error) {
            // A bridge failure degrades to a missing or unpaired report,
            // which the Rust side reads as no action against an empty
            // slot. Never let reporting break the dispatch the
            // press itself still needs.
          }
        },
        true
      );
    }
  }

  // Exported for js-tests; the page itself needs only the listener.
  if (typeof module !== "undefined" && module.exports) {
    module.exports = {
      detailEndpointFor: detailEndpointFor,
      reportFor: reportFor,
      createPressEligibility: createPressEligibility,
      classifyPress: classifyPress,
      route: ROUTE,
      header: HEADER,
    };
  }
})();
