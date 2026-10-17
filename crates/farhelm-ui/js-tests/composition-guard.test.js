/** Exercise the shipped decision and event history without DOM or process state.
 * These traces replay the two engine orders; they do not drive a real IME.
 */
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const { createCompositionTracker, isEndingKey, ENDING_KEY_WINDOW_MS } = require("../assets/composition-guard.js");

/** Use identity-bearing targets so focus changes are part of each trace. */
function key(target, isComposing = false, keyCode = 13, code = "Enter") {
  return { target, isComposing, keyCode, code };
}

/** Newer Safari can report the real key code; the end, not 229, supplies evidence. */
test("a recent same-target end protects any ending key code exactly once", () => {
  for (const keyCode of [13, 27, 229, 38, 75]) {
    const tracker = createCompositionTracker();
    const target = {};
    tracker.end(target, 100);
    assert.equal(tracker.keydown(key(target, false, keyCode), 101), true);
    assert.equal(tracker.keydown(key(target, false, keyCode), 102), false);
  }
});

/** A released composing letter precedes WebKit's end; it is not its ending key. */
test("WebKit's end-before-keydown survives earlier composing typing", () => {
  const tracker = createCompositionTracker();
  const target = {};
  assert.equal(tracker.keydown(key(target, true, 229), 10), false);
  tracker.keyup(key(target));
  tracker.end(target, 20);
  assert.equal(tracker.keydown(key(target), 20), true);
  tracker.keyup(key(target));
  assert.equal(tracker.keydown(key(target), 21), false);
});

/** The conforming order already protected its ending key and must spare the next one. */
test("Chromium's composing keydown then end never arms another key", () => {
  const tracker = createCompositionTracker();
  const target = {};
  assert.equal(tracker.keydown(key(target, true), 10), false);
  tracker.end(target, 10);
  // A different physical key can arrive before the ending Enter is released.
  assert.equal(tracker.keydown(key(target, false, 27, "Escape"), 10), false);
  tracker.keyup(key(target));
  assert.equal(tracker.keydown(key(target), 10), false);
});

/** A bare 229 is also ordinary Pinyin Enter; another editor cannot lend it evidence. */
test("no composition, a different target and an intervening key all refuse marking", () => {
  const tracker = createCompositionTracker();
  const first = {}, second = {};
  assert.equal(tracker.keydown(key(first, false, 229), 10), false);
  tracker.end(first, 20);
  assert.equal(tracker.keydown(key(second), 20), false);
  assert.equal(tracker.keydown(key(first), 20), false);
});

/** The compatibility window is bounded even when the next key carries the IME sentinel. */
test("expired ends and backwards clocks do not swallow a real key", () => {
  const target = {};
  const ended = { target, at: 100 };
  assert.equal(isEndingKey(ended, key(target), 100 + ENDING_KEY_WINDOW_MS), true);
  assert.equal(isEndingKey(ended, key(target, false, 229), 101 + ENDING_KEY_WINDOW_MS), false);
  assert.equal(isEndingKey(ended, key(target), 99), false);
  assert.equal(isEndingKey(ended, key(target, true), 100), false);
});

/** A release without an ending keydown must not leave an old end armed. */
test("keyup retires an unconsumed end", () => {
  const tracker = createCompositionTracker();
  const target = {};
  tracker.end(target, 10);
  tracker.keyup(key(target));
  assert.equal(tracker.keydown(key(target), 10), false);
});

/** The conservative fallback also declines WebKit's earlier held typing and
 * modifiers. Releasing a different key is no evidence that this key ended.
 */
test("ambiguous held composing keys spare the next key, including after another release", () => {
  for (const code of ["KeyA", "ShiftLeft"]) {
    const tracker = createCompositionTracker();
    const target = {};
    tracker.keydown(key(target, true, 229, code), 10);
    tracker.keyup(key(target, false, 16, "ShiftRight"));
    tracker.end(target, 10);
    assert.equal(tracker.keydown(key(target, false, 27, "Escape"), 10), false);
  }
});

/** Installation must be capture-first, idempotent across remounts, and leave
 * terminal input and native behavior alone. The isolated window injects DOM
 * dependencies without changing the Node process's globals or environment.
 */
test("installation marks app input once and leaves the terminal untouched", () => {
  class Target {
    constructor(terminal = false) { this.terminal = terminal; }
    matches(selector) { return this.terminal && selector === ".xterm-helper-textarea"; }
  }
  const listeners = new Map();
  const window = {
    addEventListener(type, listener, capture) {
      assert.equal(capture, true);
      listeners.set(type, [...(listeners.get(type) || []), listener]);
    },
  };
  const context = { window, Element: Target, performance: { now: () => 100 } };
  vm.runInNewContext(fs.readFileSync(require.resolve("../assets/composition-guard.js"), "utf8"), context);
  window.installCompositionGuard();
  window.installCompositionGuard();
  assert.equal(listeners.size, 3);
  for (const callbacks of listeners.values()) assert.equal(callbacks.length, 1);
  const emit = (type, event) => listeners.get(type).forEach(listener => listener(event));
  const target = new Target();
  emit("compositionend", { target });
  const event = key(target);
  event.preventDefault = event.stopPropagation = () => assert.fail("the existing guard owns native behavior");
  emit("keydown", event);
  assert.equal(event.isComposing, true);
  emit("keyup", { target });
  const terminal = new Target(true);
  emit("compositionend", { target: terminal });
  const terminalEvent = key(terminal);
  emit("keydown", terminalEvent);
  assert.equal(terminalEvent.isComposing, false);
});
