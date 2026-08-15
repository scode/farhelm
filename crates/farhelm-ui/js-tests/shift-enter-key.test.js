// Unit coverage for shift-enter-key.js's `shiftEnterKeyAction`, run with
// node's built-in test runner (matches term-bytes.test.js's rationale: one
// small function does not earn a bundler, and `node --test` already loads
// the exact file the page ships).
//
// The matrix below is a single axis now that the design sends only an ESC
// PREFIX and defers to xterm for everything else (see shift-enter-key.js's
// module docs for why the earlier three-outcome/three-event-type scheme was
// replaced): does this event count as "the Shift+Enter newline chord" or
// not. Each case pins one modifier, event-type, or IME boundary the exact-
// match check depends on — losing any of them would either steal a chord
// this binding does not own (e.g. Ctrl+Shift+Enter, an AltGraph layout's
// own composed character) or fail to fire on the one chord it exists for.
const test = require("node:test");
const assert = require("node:assert/strict");
const { shiftEnterKeyAction } = require("../assets/shift-enter-key.js");

// A full modifier/composition-neutral event, so each test below only
// overrides the ONE field it means to exercise rather than restating every
// field — keeping the diff between "this case" and "the chord" legible.
function keyEvent(overrides) {
  return Object.assign(
    {
      type: "keydown",
      key: "Enter",
      keyCode: 13,
      shiftKey: false,
      ctrlKey: false,
      altKey: false,
      metaKey: false,
      isComposing: false,
    },
    overrides,
  );
}

test("keydown Shift+Enter prefixes with ESC and defers to xterm", () => {
  assert.equal(shiftEnterKeyAction(keyEvent({ shiftKey: true })), "prefix");
});

test("keypress Shift+Enter passes through — xterm's own _keyDownHandled bookkeeping dedupes it, nothing left for this handler to do", () => {
  assert.equal(
    shiftEnterKeyAction(keyEvent({ type: "keypress", shiftKey: true })),
    "pass",
  );
});

test("keyup Shift+Enter passes through — keyup never sends data in xterm to begin with", () => {
  assert.equal(
    shiftEnterKeyAction(keyEvent({ type: "keyup", shiftKey: true })),
    "pass",
  );
});

test("plain Enter (no Shift) passes through so xterm submits as usual", () => {
  assert.equal(shiftEnterKeyAction(keyEvent({})), "pass");
});

test("Ctrl+Enter (no Shift) passes through — a different chord entirely", () => {
  assert.equal(shiftEnterKeyAction(keyEvent({ ctrlKey: true })), "pass");
});

test("Alt+Enter (no Shift) passes through — a different chord entirely", () => {
  assert.equal(shiftEnterKeyAction(keyEvent({ altKey: true })), "pass");
});

test("Ctrl+Shift+Enter passes through — not this binding's chord to claim", () => {
  assert.equal(
    shiftEnterKeyAction(keyEvent({ shiftKey: true, ctrlKey: true })),
    "pass",
  );
});

test("Alt+Shift+Enter passes through — not this binding's chord to claim", () => {
  assert.equal(
    shiftEnterKeyAction(keyEvent({ shiftKey: true, altKey: true })),
    "pass",
  );
});

test("Meta+Shift+Enter passes through — not this binding's chord to claim", () => {
  assert.equal(
    shiftEnterKeyAction(keyEvent({ shiftKey: true, metaKey: true })),
    "pass",
  );
});

test("mid-composition Shift+Enter (isComposing) passes through so IME commit is not hijacked", () => {
  assert.equal(
    shiftEnterKeyAction(keyEvent({ shiftKey: true, isComposing: true })),
    "pass",
  );
});

test("mid-composition Shift+Enter (keyCode 229 sentinel, isComposing unset) passes through", () => {
  // Some WebKit/IME paths stamp the DOM's legacy in-composition marker
  // (keyCode 229) on the synthesized keydown without reliably setting
  // isComposing — this is the independent signal shift-enter-key.js's
  // header documents checking for that reason.
  assert.equal(
    shiftEnterKeyAction(keyEvent({ shiftKey: true, keyCode: 229 })),
    "pass",
  );
});

test("AltGraph+Shift+Enter passes through so an international layout's own chord is not stolen", () => {
  const ev = keyEvent({ shiftKey: true, getModifierState: (mod) => mod === "AltGraph" });
  assert.equal(shiftEnterKeyAction(ev), "pass");
});

test("a non-Enter key with Shift held passes through unconditionally", () => {
  assert.equal(
    shiftEnterKeyAction(keyEvent({ key: "a", shiftKey: true })),
    "pass",
  );
});

test("browser-global branch: window.farhelmShiftEnterKey exists with no module present", () => {
  // Mirrors term-bytes.test.js's `node:vm` check: every test above
  // `require()`s this file, which only ever exercises the `module.exports`
  // branch. A fresh vm context with `window` but no `module` is the one
  // environment shape that runs the OTHER branch, so this is what actually
  // pins the browser-visible global rather than assuming it from the
  // CommonJS export alone.
  const vm = require("node:vm");
  const fs = require("node:fs");
  const path = require("node:path");
  const source = fs.readFileSync(path.join(__dirname, "../assets/shift-enter-key.js"), "utf8");
  const sandbox = { window: {} };
  vm.createContext(sandbox);
  vm.runInContext(source, sandbox);

  assert.equal(typeof sandbox.window.farhelmShiftEnterKey.shiftEnterKeyAction, "function");
  assert.equal(
    sandbox.window.farhelmShiftEnterKey.shiftEnterKeyAction(keyEvent({ shiftKey: true })),
    "prefix",
  );
});
