// Unit coverage for click-detail.js — the desktop window's click-count
// bridge (farhelm-ui/src/window_chrome.rs), run with node's built-in
// test runner like the other asset tests.
//
// Why this file exists: the bridge's correctness rests on small facts no
// Rust test can see — the endpoint derivation collapses the interpreter's
// doubled slash (or the POST misses the registered handler), the listener
// attaches in the CAPTURE phase (or it races the interpreter's delegated
// bubble send it must precede), only spacer presses POST, and a repeat
// reports eligible only after a primary press on the same connected
// spacer (or a cross-target pair zooms). All are one-line mistakes, and
// the native behavior they feed is macOS-only, so nothing else in the
// suite would catch the regression. The PURE half (endpoint derivation,
// report rendering, the eligibility tracker) is `require`d directly; the
// listener half runs the file's ACTUAL source in `node:vm` with a fake
// document, so deleting the behavior under test fails the test.
const test = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const fs = require("node:fs");
const path = require("node:path");
const {
  detailEndpointFor,
  reportFor,
  createPressEligibility,
  route,
  header,
} = require("../assets/click-detail.js");

const SOURCE = fs.readFileSync(path.join(__dirname, "../assets/click-detail.js"), "utf8");

// The interpreter builds its events URL as `${baseUri}/__events` where
// the injected base URI already ends in a slash — the doubled slash is
// the production input, not an edge case.
test("the bridge endpoint collapses the interpreter's doubled slash", () => {
  assert.equal(
    detailEndpointFor("dioxus://index.html//__events"),
    "dioxus://index.html/fh-click-detail"
  );
});

test("the bridge endpoint tolerates a single slash and rejects the rest", () => {
  assert.equal(
    detailEndpointFor("dioxus://index.html/__events"),
    "dioxus://index.html/fh-click-detail"
  );
  assert.equal(detailEndpointFor("dioxus://index.html//__eventsX"), null);
  assert.equal(detailEndpointFor("dioxus://index.html/"), null);
  assert.equal(detailEndpointFor(""), null);
  assert.equal(detailEndpointFor(null), null);
  assert.equal(detailEndpointFor(undefined), null);
  assert.equal(detailEndpointFor(42), null);
});

test("the report renders count, eligibility and drag in one header value", () => {
  assert.equal(reportFor(1, false, true), "1:0:1");
  assert.equal(reportFor(2, true, true), "2:1:1");
  assert.equal(reportFor(2, false, true), "2:0:1");
  assert.equal(reportFor(1, false, false), "1:0:0");
  assert.equal(reportFor(3, true, false), "3:1:0");
});

// The route and header names are a cross-language contract with
// window_chrome.rs. This pins the JS side's spelling, so a JS-side
// rename fails loudly here; it cannot see the Rust constants, so a
// Rust-only rename needs the reviewer, not this test.
test("the exported route and header match the Rust contract", () => {
  assert.equal(route, "fh-click-detail");
  assert.equal(header, "x-fh-click-detail");
  const endpoint = detailEndpointFor("dioxus://index.html//__events");
  const firstSegment = new URL(endpoint).pathname.split("/")[1];
  assert.equal(firstSegment, route);
});

// The eligibility tracker is the targeting repair: a repeat earns its
// zoom only when the immediately preceding primary press landed on the
// same connected spacer. Driven directly, press by press.
test("eligibility requires consecutive primary presses on the same connected spacer", () => {
  const spacer = { isConnected: true };
  const eligibility = createPressEligibility();
  assert.equal(eligibility.observe(spacer, true), false);
  assert.equal(eligibility.observe(spacer, true), true);
  assert.equal(eligibility.observe(spacer, true), true);
});

test("a press elsewhere invalidates eligibility and owes no report", () => {
  const spacer = { isConnected: true };
  const eligibility = createPressEligibility();
  assert.equal(eligibility.observe(spacer, true), false);
  assert.equal(eligibility.observe(null, true), null);
  assert.equal(eligibility.observe(spacer, true), false);
});

test("a non-primary press invalidates eligibility but still owes a report", () => {
  const spacer = { isConnected: true };
  const eligibility = createPressEligibility();
  assert.equal(eligibility.observe(spacer, true), false);
  assert.equal(eligibility.observe(spacer, false), false);
  assert.equal(eligibility.observe(spacer, true), false);
});

test("a replaced or disconnected spacer is not the same spacer", () => {
  const first = { isConnected: true };
  const replacement = { isConnected: true };
  const eligibility = createPressEligibility();
  assert.equal(eligibility.observe(first, true), false);
  assert.equal(eligibility.observe(replacement, true), false);
  assert.equal(eligibility.observe(replacement, true), true);

  const gone = { isConnected: false };
  const detached = createPressEligibility();
  assert.equal(detached.observe(gone, true), false);
  assert.equal(detached.observe(gone, true), false);
});

// Runs the asset's real source against a fake page and returns the
// installed mousedown listener plus every recorded XHR. A fake surface is
// a distinct object with its own connected flag and an `inMac` flag for
// whether it sits inside the macOS shell. `fire` takes the press's detail
// and button, the surface it landed in (null for a press outside every
// surface), and whether it landed on that surface's empty background
// (false for a control or text inside it).
function installWith({ interpreterEventsPath }) {
  const requests = [];
  const listeners = [];
  function FakeXMLHttpRequest() {
    this.headers = {};
    this.body = "unsent";
  }
  FakeXMLHttpRequest.prototype.open = function (method, url, async) {
    this.method = method;
    this.url = url;
    this.async = async;
  };
  FakeXMLHttpRequest.prototype.setRequestHeader = function (name, value) {
    this.headers[name] = value;
  };
  FakeXMLHttpRequest.prototype.send = function (body) {
    this.body = body;
    requests.push(this);
  };
  const sandbox = {
    window: {},
    document: {
      addEventListener: (type, listener, capture) => {
        listeners.push({ type, listener, capture });
      },
    },
    Element: function () {},
    XMLHttpRequest: FakeXMLHttpRequest,
    // Targets marked `positioned` stand for a popover inside the surface.
    getComputedStyle: (node) => ({ position: node.positioned ? "absolute" : "static" }),
  };
  if (interpreterEventsPath !== undefined) {
    sandbox.window.interpreter = { eventsPath: interpreterEventsPath };
  }
  vm.createContext(sandbox);
  vm.runInContext(SOURCE, sandbox);
  const installed = listeners.filter((entry) => entry.type === "mousedown");
  assert.equal(installed.length, 1);
  const fire = ({ detail, button = 0, surface = null, background = true, x = 10, y = 10, positioned = false }) => {
    const target = new sandbox.Element();
    target.positioned = positioned;
    target.parentElement = surface;
    // The control a non-background press landed on, inside the surface.
    const control = {};
    target.childNodes = [];
    target.closest = (selector) => {
      if (selector === ".window-drag-surface") return surface;
      if (selector === ".app-wordmark") return null;
      // Every other lookup is the "keeps its own interactions" selector.
      return background ? null : control;
    };
    if (surface) {
      surface.contains = (node) => node === control;
    }
    installed[0].listener({ target, detail, button, clientX: x, clientY: y });
  };
  const headerValues = () => requests.map((request) => request.headers["x-fh-click-detail"]);
  return { capture: installed[0].capture, requests, fire, headerValues };
}

// A 300x48 surface at the viewport's top-left. `inertBoxes` are the boxes
// of inert descendants, which hit testing skips.
function macSurface({ inertBoxes = [] } = {}) {
  return {
    isConnected: true,
    closest: (selector) => (selector === ".macos-window" ? {} : null),
    getBoundingClientRect: () => ({ left: 0, right: 300, top: 0, bottom: 48 }),
    querySelectorAll: (selector) => {
      assert.equal(selector, "[inert]");
      return inertBoxes.map((box) => ({ getBoundingClientRect: () => box }));
    },
  };
}

const EVENTS = "dioxus://index.html//__events";

test("the listener attaches in the capture phase", () => {
  assert.equal(installWith({ interpreterEventsPath: EVENTS }).capture, true);
});

test("a background press synchronously POSTs its report as a header with no body", () => {
  const { requests, fire } = installWith({ interpreterEventsPath: EVENTS });
  fire({ detail: 1, surface: macSurface() });
  assert.equal(requests.length, 1);
  assert.equal(requests[0].method, "POST");
  assert.equal(requests[0].url, "dioxus://index.html/fh-click-detail");
  assert.equal(requests[0].async, false);
  assert.equal(requests[0].headers["x-fh-click-detail"], "1:0:1");
  assert.equal(requests[0].body, undefined);
});

test("consecutive background presses report an eligible repeat", () => {
  const { fire, headerValues } = installWith({ interpreterEventsPath: EVENTS });
  const surface = macSurface();
  fire({ detail: 1, surface });
  fire({ detail: 2, surface });
  fire({ detail: 3, surface });
  assert.deepEqual(headerValues(), ["1:0:1", "2:1:1", "3:1:1"]);
});

// The surface's handler receives every press inside it, controls included,
// so a control press must POST too: otherwise that handler would consume a
// stale report, or none, and could move the window from under a button.
test("a control press inside a surface reports no drag and breaks the pair", () => {
  const { fire, headerValues } = installWith({ interpreterEventsPath: EVENTS });
  const surface = macSurface();
  fire({ detail: 1, surface });
  fire({ detail: 1, surface, background: false });
  fire({ detail: 3, surface });
  assert.deepEqual(headerValues(), ["1:0:1", "1:0:0", "3:0:1"]);
});

test("a press outside every surface posts nothing but breaks the pair", () => {
  const { fire, headerValues } = installWith({ interpreterEventsPath: EVENTS });
  const surface = macSurface();
  fire({ detail: 1, surface });
  fire({ detail: 1, surface: null });
  fire({ detail: 3, surface });
  assert.deepEqual(headerValues(), ["1:0:1", "3:0:1"]);
});

test("a control press first keeps the background press ineligible", () => {
  const { fire, headerValues } = installWith({ interpreterEventsPath: EVENTS });
  const surface = macSurface();
  fire({ detail: 1, surface, background: false });
  fire({ detail: 2, surface });
  assert.deepEqual(headerValues(), ["1:0:0", "2:0:1"]);
});

test("a non-primary background press never drags and interrupts the pair", () => {
  const { fire, headerValues } = installWith({ interpreterEventsPath: EVENTS });
  const surface = macSurface();
  fire({ detail: 1, surface });
  fire({ detail: 1, button: 2, surface });
  fire({ detail: 2, surface });
  assert.deepEqual(headerValues(), ["1:0:1", "1:0:0", "2:0:1"]);
});

test("a press on a different surface starts eligibility over", () => {
  const { fire, headerValues } = installWith({ interpreterEventsPath: EVENTS });
  fire({ detail: 1, surface: macSurface() });
  fire({ detail: 2, surface: macSurface() });
  assert.deepEqual(headerValues(), ["1:0:1", "2:0:1"]);
});

test("presses outside surfaces, outside the macOS shell, or without an interpreter never POST", () => {
  const outside = installWith({ interpreterEventsPath: EVENTS });
  outside.fire({ detail: 2, surface: null });
  assert.equal(outside.requests.length, 0);

  // Browser and Linux shells carry the surfaces' markup without the macOS
  // shell class; nothing there may drag.
  const notMac = installWith({ interpreterEventsPath: EVENTS });
  notMac.fire({ detail: 1, surface: { isConnected: true, closest: () => null } });
  assert.equal(notMac.requests.length, 0);

  // The web build has no interpreter global at all — geometry tests
  // that force the shell class onto production markup take this path.
  const web = installWith({ interpreterEventsPath: undefined });
  web.fire({ detail: 2, surface: macSurface() });
  assert.equal(web.requests.length, 0);
});

// A surface's descendants can extend below the band (a session's
// confirmation panel hangs below the top row); only the band itself is a
// title bar, so those presses report no drag rather than move the window.
test("a press inside a surface but outside its box reports no drag", () => {
  const { fire, headerValues } = installWith({ interpreterEventsPath: EVENTS });
  const surface = macSurface();
  fire({ detail: 1, surface, y: 60 });
  fire({ detail: 1, surface, x: 300 });
  fire({ detail: 1, surface, y: 47 });
  assert.deepEqual(headerValues(), ["1:0:0", "1:0:0", "1:0:1"]);
});

// Hit testing skips inert elements, so a press over a stale session's inert
// tab targets the surface behind it; it must stay inactive.
test("a press over an inert descendant reports no drag", () => {
  const { fire, headerValues } = installWith({ interpreterEventsPath: EVENTS });
  const surface = macSurface({ inertBoxes: [{ left: 0, right: 100, top: 0, bottom: 48 }] });
  fire({ detail: 1, surface, x: 50 });
  fire({ detail: 1, surface, x: 150 });
  assert.deepEqual(headerValues(), ["1:0:0", "1:0:1"]);
});

// A confirmation panel's top overlaps the band it hangs from; a press there
// lands inside the surface's box but on a popover, which keeps its own
// interactions.
test("a press on a positioned popover inside the band reports no drag", () => {
  const { fire, headerValues } = installWith({ interpreterEventsPath: EVENTS });
  const surface = macSurface();
  fire({ detail: 1, surface, positioned: true });
  fire({ detail: 1, surface });
  assert.deepEqual(headerValues(), ["1:0:0", "1:0:1"]);
});
