/** The shipped observer must publish the press before a delegated user handler,
 * without consuming input. Each fixture owns its window, clock and relay.
 */
const test = require("node:test");
const assert = require("node:assert/strict");
const { installLaunchActivity, SUBMIT_REPEAT_MS } = require("../assets/launch-activity.js");

/** A strict event bridge makes ordering observable without changing Node globals.
 * The relay can unmount while the page listener stays alive, as authentication
 * does; a fresh relay must receive the retained counter when it mounts.
 */
function page() {
  const listeners = new Map();
  const publications = [];
  let now = 100;
  const relay = {
    value: "",
    dispatchEvent(event) {
      assert.equal(event.type, "input");
      assert.equal(event.bubbles, true);
      publications.push(JSON.parse(this.value));
      this.onInput?.(JSON.parse(this.value));
    },
  };
  const doc = {
    relay,
    body: { nodeType: 1 },
    documentElement: { nodeType: 1 },
    querySelector(selector) {
      assert.equal(selector, "input[data-launch-activity]");
      return this.relay;
    },
  };
  const win = {
    Event: class {
      constructor(type, options) { Object.assign(this, { type }, options); }
    },
    performance: { now: () => now },
    addEventListener(type, listener, capture) {
      assert.equal(capture, true);
      listeners.set(type, [...(listeners.get(type) || []), listener]);
    },
  };
  installLaunchActivity(doc, win);
  /** A target's form and control identities survive hiding, but a different
   * opening changes its attribute even when a renderer reuses the DOM node.
   */
  function target(form = null, kind = "input") {
    const node = {
      form, nodeType: 1,
      closest(selector) {
        if (selector === '.create-session-form[role="dialog"]') return form;
        assert.equal(selector, ".create-session-submit");
        return kind === "submit" ? node : null;
      },
      matches(selector) {
        assert.equal(selector, 'input:not([type="button"]):not([type="checkbox"]):not([type="radio"])');
        return kind === "input";
      },
    };
    return node;
  }
  return {
    doc, win, relay, publications, target,
    advance(ms) { now += ms; },
    form(opening = "1") {
      return {
        opening, isConnected: true, hidden: false,
        contains(target) { return target?.form === this; },
        getAttribute(name) { assert.equal(name, "data-opening-generation"); return this.opening; },
      };
    },
    dispatch(type, fields = {}, afterCapture = () => {}) {
      const event = {
        type, target: target(), repeat: false, key: "x", button: 0, detail: 1, clientX: 100, clientY: 100, ...fields,
        preventDefault() { assert.fail("the observer must not consume input"); },
        stopPropagation() { assert.fail("the observer must not consume input"); },
      };
      for (const listener of listeners.get(type) || []) listener(event);
      afterCapture(event);
    },
    read() { return win.__farhelmLaunchActivity(); },
    listenerCount() { return [...listeners.values()].reduce((total, entries) => total + entries.length, 0); },
  };
}

/** Baselines belong to the current press, including shortcut keys that another
 * listener will convert to a programmatic click during the same dispatch.
 */
test("capture publishes before the original handler without changing input", () => {
  const p = page();
  let mirrored = null;
  p.relay.onInput = count => { mirrored = count; };
  p.win.__farhelmLaunchActivitySync();
  assert.equal(mirrored, 0);
  p.dispatch("keydown", { key: "p", ctrlKey: true }, () => assert.equal(mirrored, 1));
  p.dispatch("pointerdown", { button: 2 }, () => assert.equal(mirrored, 2));
  assert.deepEqual(p.publications, [0, 1, 2]);
});

/** A held key or assembled modifier chord adds no new choice; a modified
 * ordinary key does. Key names are tested independently of the modifier mask.
 */
test("auto repeat and lone modifiers do not advance the page count", () => {
  const p = page();
  for (const key of ["Alt", "AltGraph", "CapsLock", "Control", "Fn", "Meta", "Shift"]) p.dispatch("keydown", { key });
  p.dispatch("keydown", { key: "x", repeat: true });
  assert.equal(p.read(), 0);
  p.dispatch("keydown", { key: "x", metaKey: true });
  assert.equal(p.read(), 1);
});

/** Ignore accidental own-submit repeats even when PointerEvent.detail is zero,
 * while a later intentional press and another pointer button remain choices.
 */
test("own-submit pointer repeats are bounded and scoped to the button", () => {
  const p = page();
  const submit = p.target(p.form(), "submit");
  p.dispatch("pointerdown", { target: submit, detail: 0 });
  p.advance(SUBMIT_REPEAT_MS - 1);
  p.dispatch("pointerdown", { target: submit, detail: 0 });
  assert.equal(p.read(), 1);
  p.advance(1);
  p.dispatch("pointerdown", { target: submit, detail: 2 });
  assert.equal(p.read(), 1);
  p.advance(SUBMIT_REPEAT_MS + 1);
  p.dispatch("pointerdown", { target: submit, detail: 0 });
  p.dispatch("pointerdown", { target: submit, button: 2, detail: 0 });
  assert.equal(p.read(), 3);
});

/** The page underneath receives the second click after submit hides the form.
 * PointerEvent.detail may be zero, so the captured coordinates must carry the
 * gesture across that target change. Hiding and removal are both valid exits.
 */
test("own-submit double click survives hiding or removing its original target", () => {
  for (const removed of [false, true]) {
    const p = page();
    const form = p.form();
    p.dispatch("pointerdown", { target: p.target(form, "submit"), clientX: 20, clientY: 30, detail: 0 });
    form.hidden = !removed;
    form.isConnected = !removed;
    p.advance(100);
    p.dispatch("pointerdown", { target: p.target(), clientX: 21, clientY: 31, detail: 0 });
    assert.equal(p.read(), 1);
    assert.deepEqual(p.publications, [1]);
    // The ignored repeat retained the first timestamp rather than extending
    // its authority for another half second after the second click.
    p.advance(SUBMIT_REPEAT_MS - 99);
    p.dispatch("pointerdown", { target: p.target(), clientX: 20, clientY: 30, detail: 2 });
    assert.equal(p.read(), 2);
  }
});

/** Count a deliberate click elsewhere even when the form just disappeared.
 * The browser's repeat hint cannot excuse clearly different coordinates,
 * and that unrelated press retires the old gesture before a later click.
 */
test("another page location or pointer button is new input after submit hides", () => {
  for (const fields of [{ clientX: 50, clientY: 60, detail: 2 }, { button: 2 }]) {
    const p = page();
    const form = p.form();
    p.dispatch("pointerdown", { target: p.target(form, "submit") });
    form.hidden = true;
    p.dispatch("pointerdown", fields);
    assert.equal(p.read(), 2);
    p.dispatch("pointerdown", { detail: 2 });
    assert.equal(p.read(), 3);
  }
  const p = page();
  const old = p.form();
  p.dispatch("pointerdown", { target: p.target(old, "submit") });
  old.hidden = true;
  p.dispatch("pointerdown", { target: p.target(p.form("2"), "submit") });
  assert.equal(p.read(), 2, "another launcher opening cannot borrow the old gesture");
});

/** Native focus falls to document after an Enter submit hides its control.
 * Only that exit is a repeated submit: another terminal, a visible form's
 * body target and any intervening key all remain independent input.
 */
test("repeated submit Enter survives a hidden form's document focus handoff", () => {
  for (const destination of ["body", "documentElement", "hidden-control"]) {
    const p = page();
    const form = p.form();
    p.dispatch("keydown", { target: p.target(form), key: "Enter" });
    form.hidden = true;
    const target = destination === "hidden-control" ? p.target(form, "other") : p.doc[destination];
    p.advance(100);
    p.dispatch("keydown", { target, key: "Enter" });
    assert.equal(p.read(), 1);
    p.dispatch("keydown", { target: p.doc.body, key: "a" });
    p.dispatch("keydown", { target: p.doc.body, key: "Enter" });
    assert.equal(p.read(), 3);
  }
  for (const hidden of [false, true]) {
    const p = page();
    const form = p.form();
    p.dispatch("keydown", { target: p.target(form), key: "Enter" });
    form.hidden = hidden;
    p.dispatch("keydown", { target: hidden ? p.target() : p.doc.body, key: "Enter" });
    assert.equal(p.read(), 2);
  }
});

/** Enter on another control, another opening or a terminal is input the user
 * moved on with. A global Enter debounce would silently swallow that evidence.
 */
test("Enter repeats never borrow authority across targets or launcher openings", () => {
  const p = page();
  const form = p.form();
  const first = p.target(form);
  p.dispatch("keydown", { target: first, key: "Enter" });
  p.dispatch("keydown", { target: first, key: "Enter" });
  assert.equal(p.read(), 1);
  p.dispatch("keydown", { target: p.target(form), key: "Enter" });
  form.opening = "2";
  p.dispatch("keydown", { target: first, key: "Enter" });
  p.dispatch("keydown", { key: "Enter" });
  p.dispatch("keydown", { key: "Enter" });
  assert.equal(p.read(), 5);
});

/** A repeat gesture no longer matches after intervening activity, and typing
 * inside the launcher is still activity even within the repeat interval.
 */
test("unrelated input retires own-submit repeat identity", () => {
  const p = page();
  const input = p.target(p.form());
  p.dispatch("keydown", { target: input, key: "Enter" });
  p.dispatch("keydown", { target: input, key: "a" });
  p.dispatch("keydown", { target: input, key: "Enter" });
  p.dispatch("pointerdown", { target: p.target(), detail: 2 });
  assert.equal(p.read(), 4);
});

/** DOM absence is not a fresh page: input while signed out still invalidates
 * an old baseline, and mounting a new relay publishes the retained value.
 */
test("remounting keeps one observer and resynchronizes the new relay", () => {
  const p = page();
  p.dispatch("keydown");
  p.doc.relay = null;
  p.dispatch("pointerdown");
  assert.equal(installLaunchActivity(p.doc, p.win), p.win.__farhelmLaunchActivity);
  assert.equal(p.listenerCount(), 2);
  p.doc.relay = p.relay;
  p.win.__farhelmLaunchActivitySync();
  assert.deepEqual(p.publications, [1, 2]);
  assert.equal(p.read(), 2);
});

/** Equality at a wrapped counter must never make an ancient answer current.
 * Unknown input evidence remains unknown after later presses and remounts.
 */
test("counter exhaustion fails closed instead of wrapping", () => {
  const p = page();
  p.win.__farhelmLaunchPressCount = Number.MAX_SAFE_INTEGER;
  p.dispatch("keydown");
  assert.equal(p.read(), null);
  p.dispatch("pointerdown");
  assert.equal(p.read(), null);
  installLaunchActivity(p.doc, p.win);
  assert.equal(p.read(), null);
  assert.deepEqual(p.publications, [null, null]);
});
