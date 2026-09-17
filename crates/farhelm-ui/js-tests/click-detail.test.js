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

test("the report renders count and eligibility in one header value", () => {
  assert.equal(reportFor(1, false), "1:0");
  assert.equal(reportFor(2, true), "2:1");
  assert.equal(reportFor(2, false), "2:0");
  assert.equal(reportFor(3, true), "3:1");
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
// installed mousedown listener plus every recorded XHR. Fake spacers
// are distinct objects with their own connected flag; `fire` takes the
// press's detail, button, and target spacer (null for a control press).
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
  };
  if (interpreterEventsPath !== undefined) {
    sandbox.window.interpreter = { eventsPath: interpreterEventsPath };
  }
  vm.createContext(sandbox);
  vm.runInContext(SOURCE, sandbox);
  const installed = listeners.filter((entry) => entry.type === "mousedown");
  assert.equal(installed.length, 1);
  const fire = ({ detail, button = 0, spacer = null }) => {
    const target = new sandbox.Element();
    target.closest = (selector) => {
      assert.equal(selector, ".window-drag-region");
      return spacer;
    };
    installed[0].listener({ target, detail, button });
  };
  const headerValues = () => requests.map((request) => request.headers["x-fh-click-detail"]);
  return { capture: installed[0].capture, requests, fire, headerValues };
}

function connectedSpacer() {
  return { isConnected: true };
}

test("the listener attaches in the capture phase", () => {
  const { capture } = installWith({
    interpreterEventsPath: "dioxus://index.html//__events",
  });
  assert.equal(capture, true);
});

test("a spacer press synchronously POSTs its report as a header with no body", () => {
  const { requests, fire } = installWith({
    interpreterEventsPath: "dioxus://index.html//__events",
  });
  fire({ detail: 1, spacer: connectedSpacer() });
  assert.equal(requests.length, 1);
  assert.equal(requests[0].method, "POST");
  assert.equal(requests[0].url, "dioxus://index.html/fh-click-detail");
  assert.equal(requests[0].async, false);
  assert.equal(requests[0].headers["x-fh-click-detail"], "1:0");
  assert.equal(requests[0].body, undefined);
});

test("consecutive spacer presses report an eligible repeat", () => {
  const { fire, headerValues } = installWith({
    interpreterEventsPath: "dioxus://index.html//__events",
  });
  const spacer = connectedSpacer();
  fire({ detail: 1, spacer });
  fire({ detail: 2, spacer });
  fire({ detail: 3, spacer });
  assert.deepEqual(headerValues(), ["1:0", "2:1", "3:1"]);
});

test("a control press between spacer presses breaks the pair explicitly", () => {
  const { fire, headerValues } = installWith({
    interpreterEventsPath: "dioxus://index.html//__events",
  });
  const spacer = connectedSpacer();
  // spacer → control → spacer: the control press POSTs nothing, and the
  // second spacer press still POSTs — an explicit ineligible report that
  // overwrites the slot — rather than leaving a stale report behind.
  fire({ detail: 1, spacer });
  fire({ detail: 1, spacer: null });
  fire({ detail: 3, spacer });
  assert.deepEqual(headerValues(), ["1:0", "3:0"]);
});

test("a control press first keeps the spacer press ineligible", () => {
  const { fire, headerValues } = installWith({
    interpreterEventsPath: "dioxus://index.html//__events",
  });
  fire({ detail: 1, spacer: null });
  fire({ detail: 2, spacer: connectedSpacer() });
  assert.deepEqual(headerValues(), ["2:0"]);
});

test("a non-primary spacer press interrupts eligibility and keeps pairing", () => {
  const { fire, headerValues } = installWith({
    interpreterEventsPath: "dioxus://index.html//__events",
  });
  const spacer = connectedSpacer();
  fire({ detail: 1, spacer });
  fire({ detail: 1, button: 2, spacer });
  fire({ detail: 2, spacer });
  assert.deepEqual(headerValues(), ["1:0", "1:0", "2:0"]);
});

test("a replaced spacer starts eligibility over", () => {
  const { fire, headerValues } = installWith({
    interpreterEventsPath: "dioxus://index.html//__events",
  });
  fire({ detail: 1, spacer: connectedSpacer() });
  fire({ detail: 2, spacer: connectedSpacer() });
  assert.deepEqual(headerValues(), ["1:0", "2:0"]);
});

test("control presses and missing interpreters never POST", () => {
  const control = installWith({
    interpreterEventsPath: "dioxus://index.html//__events",
  });
  control.fire({ detail: 2, spacer: null });
  assert.equal(control.requests.length, 0);

  // The web build has no interpreter global at all — geometry tests
  // that force the shell class onto production markup take this path.
  const web = installWith({ interpreterEventsPath: undefined });
  web.fire({ detail: 2, spacer: connectedSpacer() });
  assert.equal(web.requests.length, 0);
});
