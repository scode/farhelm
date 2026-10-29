// Unit coverage for tooltip.js's pure decisions and menu focus ownership, run with node's built-in
// test runner (the same shape as the other files in this directory: `node
// --test` loads the exact file the page ships).
//
// Placement is the rule worth pinning here rather than in a browser: the
// maintainer chose ABOVE-first with a small gap and a much larger gap BELOW,
// because the cursor covers the space under the hot spot and never the space
// above it (see tooltip.js's module docs). A regression that flipped the
// preference, shrank the below gap, or let a tooltip run off a window edge
// would still "show a tooltip" in a browser test, so the browser suite only
// proves the wiring and these cases pin the geometry. The follow-on and
// scroll-relevance helpers are pinned for the same reason: their failure
// modes (a delay at every icon in a row; a tooltip that vanishes whenever
// the terminal scrolls under live output) are behaviors, not crashes.
const test = require("node:test");
const assert = require("node:assert/strict");
const { placeTooltip, instantFollowOn, scrollMovesTarget, install } = require("../assets/tooltip.js");
const { installMenuFocus } = require("../assets/menu-focus.js");

const VIEWPORT = { width: 1000, height: 800 };

/** A target rectangle from its left/top corner and size, as getBoundingClientRect reports one. */
function rect(left, top, width, height) {
  return { left, top, right: left + width, bottom: top + height, width, height };
}

// ---------------------------------------------------------------------------
// Placement
// ---------------------------------------------------------------------------

test("a target with room above gets the tooltip centred 6 px above it", () => {
  const place = placeTooltip(rect(500, 400, 20, 20), { width: 100, height: 30 }, VIEWPORT);
  assert.deepEqual(place, { left: 460, top: 364, side: "above" });
});

test("a target near the top edge falls back below with the larger 28 px gap", () => {
  const place = placeTooltip(rect(500, 10, 20, 20), { width: 100, height: 30 }, VIEWPORT);
  assert.deepEqual(place, { left: 460, top: 58, side: "below" });
});

test("above is kept while it fits exactly at the 4 px edge margin", () => {
  // top - 6 - 30 = 4: the tooltip just fits, so the cursor-safe side wins.
  const place = placeTooltip(rect(500, 40, 20, 20), { width: 100, height: 30 }, VIEWPORT);
  assert.equal(place.side, "above");
  assert.equal(place.top, 4);
});

test("a target near the left edge shifts the tooltip right to stay inside", () => {
  const place = placeTooltip(rect(0, 400, 16, 16), { width: 200, height: 30 }, VIEWPORT);
  assert.equal(place.left, 4);
  assert.equal(place.side, "above");
});

test("a target near the right edge shifts the tooltip left to stay inside", () => {
  const place = placeTooltip(rect(990, 400, 10, 16), { width: 200, height: 30 }, VIEWPORT);
  assert.equal(place.left, 1000 - 4 - 200);
});

test("a tooltip wider than the window is pinned to the left edge, where its text starts", () => {
  const place = placeTooltip(rect(400, 400, 20, 20), { width: 1200, height: 30 }, VIEWPORT);
  assert.equal(place.left, 4);
});

test("a tooltip that fits on neither side takes the roomier side, clamped inside the window", () => {
  // 300 px tall in a 400 px window around a target in the lower half: there
  // is more room above, so it goes above and is clamped to the top margin,
  // overlapping the target rather than leaving the screen.
  const small = { width: 1000, height: 400 };
  const lower = placeTooltip(rect(500, 250, 20, 20), { width: 100, height: 300 }, small);
  assert.deepEqual(lower, { left: 460, top: 4, side: "above" });
  // The same tooltip around a target in the upper half goes below, clamped
  // so its bottom stays 4 px above the window's bottom edge.
  const upper = placeTooltip(rect(500, 100, 20, 20), { width: 100, height: 300 }, small);
  assert.deepEqual(upper, { left: 460, top: 96, side: "below" });
});

// ---------------------------------------------------------------------------
// Follow-on and scroll relevance
// ---------------------------------------------------------------------------

test("a new target shows at once while a tooltip is showing", () => {
  assert.equal(instantFollowOn(10_000, -Infinity, true), true);
});

test("a new target shows at once within the 300 ms grace after a hide, and not after it", () => {
  assert.equal(instantFollowOn(1_299, 1_000, false), true);
  assert.equal(instantFollowOn(1_300, 1_000, false), false);
  assert.equal(instantFollowOn(5_000, -Infinity, false), false);
});

test("a scroll of the document or of a container holding the target moves it", () => {
  const target = {};
  const container = { contains: (node) => node === target };
  const doc = { documentElement: {}, body: {} };
  assert.equal(scrollMovesTarget(doc, target, doc), true);
  assert.equal(scrollMovesTarget(doc.documentElement, target, doc), true);
  assert.equal(scrollMovesTarget(doc.body, target, doc), true);
  assert.equal(scrollMovesTarget(container, target, doc), true);
});

test("a scroll of an unrelated container (the terminal under live output) does not", () => {
  const target = {};
  const terminalViewport = { contains: () => false };
  const doc = { documentElement: {}, body: {} };
  assert.equal(scrollMovesTarget(terminalViewport, target, doc), false);
});

// ---------------------------------------------------------------------------
// Pointer-made menu focus
// ---------------------------------------------------------------------------

/** Run both shipped listener sets with synchronous focus and an owned clock.
 * The fixture forces :focus-visible even for pointer-made focus: engines can
 * retain keyboard modality across script focus, which is the regression here.
 * Timers advance only at requested boundaries, without wall-clock sleeps.
 */
function menuTooltipPage(menuFirst) {
  const listeners = new Map();
  const timers = new Map();
  const tooltips = [];
  let now = 0;
  let timerId = 0;
  const doc = {
    activeElement: null,
    addEventListener(type, listener, capture) {
      if (type !== "visibilitychange") assert.equal(capture, true);
      if (!listeners.has(type)) listeners.set(type, []);
      listeners.get(type).push(listener);
    },
    querySelector(selector) {
      assert.equal(selector, "input[data-menu-focus-epoch]");
      return null;
    },
    createElement(tag) {
      assert.equal(tag, "div");
      const tip = element();
      tip.isConnected = false;
      tooltips.push(tip);
      return tip;
    },
  };
  /** Dispatch in registration order, including focus nested within pointermove. */
  function dispatch(type, target, fields = {}) {
    for (const listener of listeners.get(type) || []) listener({ target, ...fields });
  }
  /** Only the DOM contracts these assets use are modeled; unknown selectors fail. */
  function element(attributes = {}, parentElement = null) {
    return {
      nodeType: 1, parentElement, isConnected: true, style: {}, dataset: {}, hidden: true,
      getAttribute(name) { return attributes[name] ?? null; },
      setAttribute(name, value) { attributes[name] = value; },
      matches(selector) {
        assert.equal(selector, ":focus-visible");
        return true;
      },
      closest(selector) {
        assert.ok(selector === "[data-menu-focus]" || selector === '[role="menu"]');
        for (let node = this; node; node = node.parentElement) {
          if (selector === "[data-menu-focus]" && node.getAttribute("data-menu-focus") !== null) return node;
          if (selector === '[role="menu"]' && node.getAttribute("role") === "menu") return node;
        }
        return null;
      },
      contains(node) {
        for (; node; node = node.parentElement) if (node === this) return true;
        return false;
      },
      focus() {
        if (doc.activeElement === this) return;
        const previous = doc.activeElement;
        doc.activeElement = this;
        if (previous) dispatch("focusout", previous);
        dispatch("focusin", this);
      },
      getBoundingClientRect() { return rect(100, 100, 80, 20); },
      appendChild(node) { node.isConnected = true; },
      replaceChildren() {},
    };
  }
  doc.body = element();
  const win = {
    document: doc, farhelmTooltip: {}, innerWidth: 1000, innerHeight: 800,
    addEventListener() {}, performance: { now: () => now },
    setTimeout(callback, delay) {
      const id = ++timerId;
      timers.set(id, { callback, at: now + delay });
      return id;
    },
    clearTimeout(id) { timers.delete(id); },
    setInterval(callback, delay) {
      const id = ++timerId;
      timers.set(id, { callback, at: now + delay, interval: delay });
      return id;
    },
    clearInterval(id) { timers.delete(id); },
  };
  if (menuFirst) installMenuFocus(doc, win);
  install(win);
  if (!menuFirst) installMenuFocus(doc, win);
  const panel = element({ role: "menu" });
  return {
    doc, dispatch,
    item(key) { return element({ "data-menu-focus": key, "data-tooltip": `${key} help` }, panel); },
    outside: doc.body,
    visible() { return tooltips.filter(tip => !tip.hidden); },
    /** Fire every due task in deadline order, including recurring watch ticks. */
    advance(ms) {
      const end = now + ms;
      for (;;) {
        const next = [...timers].filter(([, timer]) => timer.at <= end).sort((a, b) => a[1].at - b[1].at)[0];
        if (!next) break;
        const [id, timer] = next;
        now = timer.at;
        if (timer.interval) timer.at += timer.interval;
        else timers.delete(id);
        timer.callback();
      }
      now = end;
    },
  };
}

// A script focus may match :focus-visible after keyboard use. It must neither
// bypass the pointer's rest timer nor keep help showing after the pointer leaves.
test("menu hover after keyboard focus keeps the rest delay and pointer-leave dismissal", () => {
  for (const menuFirst of [true, false]) {
    for (const leaveWindow of [true, false]) {
      const p = menuTooltipPage(menuFirst);
      const keyboard = p.item("keyboard");
      const hovered = p.item("hovered");
      keyboard.focus();
      assert.equal(p.doc.activeElement, keyboard);
      assert.equal(hovered.matches(":focus-visible"), true);
      p.advance(100);
      p.dispatch("pointerover", hovered, { pointerType: "mouse" });
      p.advance(200);
      p.dispatch("pointermove", hovered, { pointerType: "mouse" });
      assert.equal(p.doc.activeElement, hovered, "the real menu listener must make the focus change");
      p.advance(299);
      assert.equal(p.visible().length, 0, "pointer-made focus must preserve the full rest delay");
      p.advance(1);
      assert.equal(p.visible().length, 1);
      assert.equal(p.visible()[0].textContent, "hovered help");
      if (leaveWindow) p.dispatch("pointerout", hovered, { relatedTarget: null });
      else p.dispatch("pointerover", p.outside, { pointerType: "mouse" });
      assert.equal(p.doc.activeElement, hovered, "leaving retains the menu's current item");
      assert.equal(p.visible().length, 0, "retained focus must not retain pointer help");
    }
  }
});

// An arrow can move focus away while the pointer stays on its old item. A
// subsequent move within that item has no pointerover to restart hover help.
test("motion inside a still-hovered menu item restores pointer help after an arrow", () => {
  for (const menuFirst of [true, false]) {
    const p = menuTooltipPage(menuFirst);
    const hovered = p.item("hovered");
    const keyboard = p.item("keyboard");
    p.dispatch("pointerover", hovered, { pointerType: "mouse" });
    p.dispatch("pointermove", hovered, { pointerType: "mouse" });
    assert.equal(p.doc.activeElement, hovered);
    keyboard.focus();
    assert.equal(p.doc.activeElement, keyboard);
    p.advance(100);
    // The pointer never left this item: dispatch movement without an over.
    p.dispatch("pointermove", hovered, { pointerType: "mouse" });
    assert.equal(p.doc.activeElement, hovered);
    p.advance(299);
    assert.equal(p.visible().length, 0, "rest begins again after focus returns by pointer");
    p.advance(1);
    assert.equal(p.visible().length, 1, "help must be scheduled without another pointerover");
    assert.equal(p.visible()[0].textContent, "hovered help");
    p.dispatch("pointerover", p.outside, { pointerType: "mouse" });
    assert.equal(p.doc.activeElement, hovered);
    assert.equal(p.visible().length, 0);
  }
});

// The pointer provenance exception must not weaken actual keyboard ownership:
// focus help still survives an unrelated pointer leaving while focus stays put.
test("keyboard menu focus retains its help when the pointer leaves", () => {
  const p = menuTooltipPage(true);
  const keyboard = p.item("keyboard");
  keyboard.focus();
  assert.equal(p.doc.activeElement, keyboard);
  p.advance(300);
  assert.equal(p.visible().length, 1);
  p.dispatch("pointerover", p.outside, { pointerType: "mouse" });
  p.dispatch("pointerout", p.outside, { relatedTarget: null });
  assert.equal(p.visible().length, 1);
});
