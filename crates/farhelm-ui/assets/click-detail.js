// The desktop window's click-count bridge (farhelm-ui/src/window_chrome.rs).
//
// WHY THIS FILE EXISTS: the spacer that owns native window dragging learns
// about presses through Dioxus's event transport, which serializes mouse
// coordinates, buttons, and modifiers but NOT the press's click count
// (dioxus-interpreter-js 0.7.10's serializeMouseEvent has no `detail`
// field, and MouseData cannot recover what was never sent). AppKit is
// assumed to have classified the press — NSEvent.clickCount — and WebKit
// copies that classification onto every DOM mousedown's `detail` (see
// window_chrome.rs for the conversion chain with sources, and for why the
// AppKit half of that sentence is an assumption pending native validation
// rather than a proved guarantee). This file carries that one number
// across the transport gap: a capture-phase listener reads `event.detail`
// on spacer presses and synchronously POSTs it to a Rust asset-handler
// route, ahead of the interpreter's own event send for the same press.
// The Rust spacer handler then pairs the two by ORDER — the POST's
// response necessarily precedes the interpreter's send because both are
// synchronous XHRs on this one JS thread — and decides drag versus
// maximize-toggle from AppKit's count, with no timing heuristic.
//
// The count alone does not earn a zoom: WebKit copies the platform count
// without resetting it for a different DOM element, so a press on header
// text followed by a fast spacer press can carry count 2. Every report
// therefore carries an eligibility bit too: whether the immediately
// preceding primary press landed on the same connected spacer node. An
// intervening press elsewhere — or a non-primary press, or a spacer the
// page has since replaced — clears eligibility, and the Rust side treats
// an ineligible repeat as no action at all (no zoom AND no drag), while
// an ordinary press still drags as before. The DECISION stays in the Rust
// component's own onmousedown, which remains the single gesture owner.
//
// WHAT THIS FILE DOES NOT DO: it never prevents default handling, never
// stops propagation, and never acts on the press. It observes presses and
// reports count plus eligibility; control presses only invalidate the
// tracker and owe no POST (no Rust spacer event consumes one for them).
//
// INERTNESS: the script is rendered only in the desktop shell, and even
// there it no-ops unless `window.interpreter` exists (set solely by the
// desktop index template — the web build has no such global) AND the
// press lands on `.window-drag-region` (hidden outside the macOS shell
// class, so unreachable on Linux and in ordinary browsers). Browser
// geometry tests that force the shell class onto production markup still
// take the no-interpreter path. Every failure below is swallowed: a dead
// bridge must degrade to missing reports — which the Rust side reads as
// ordinary dragging — never break the page. That fail-safe covers missing
// reports against an empty slot and stuck-at-1 counts; it is not a claim
// about every partial transport failure (see window_chrome.rs).
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

  // Render one bridge report: the press's DOM detail plus whether an
  // eligible preceding primary press on the same connected spacer backs
  // a repeat. One header value, so the two fields arrive atomically.
  function reportFor(detail, eligible) {
    return String(detail) + ":" + (eligible ? "1" : "0");
  }

  // Track whether the immediately preceding primary press landed on the
  // spacer, so a cross-target pair can never borrow a repeat count.
  // Pure state machine over mousedowns in dispatch order; the listener
  // below feeds it every press, and js-tests drive it directly.
  function createPressEligibility() {
    var lastSpacer = null;
    // Observe one mousedown. `spacer` is the pressed spacer node, or
    // null for a press elsewhere; `primary` is whether it used the
    // primary button. Returns the eligibility to report for this press,
    // or null when no report is owed: a press elsewhere only invalidates
    // (no Rust spacer event will consume a report for it), while every
    // spacer press POSTs — even an ineligible or non-primary one — so
    // its report overwrites any stale slot instead of leaving it behind.
    function observe(spacer, primary) {
      if (!spacer || !primary) {
        lastSpacer = null;
        return spacer ? false : null;
      }
      var eligible = lastSpacer === spacer && spacer.isConnected;
      lastSpacer = spacer;
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
            var target = event.target;
            // A press with no element target cannot be a spacer press;
            // invalidate like any other press elsewhere.
            var spacer = target instanceof Element ? target.closest(".window-drag-region") : null;
            var primary = event.button === 0;
            var eligible = eligibility.observe(spacer, primary);
            if (eligible === null) {
              return;
            }
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
            xhr.setRequestHeader(HEADER, reportFor(event.detail, eligible));
            xhr.send();
          } catch (error) {
            // A bridge failure degrades to a missing or unpaired report,
            // which the Rust side reads as an ordinary drag against an
            // empty slot. Never let reporting break the dispatch the
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
      route: ROUTE,
      header: HEADER,
    };
  }
})();
