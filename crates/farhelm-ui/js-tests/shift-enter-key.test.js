// Unit coverage for shift-enter-key.js, run with node's built-in test
// runner (matches term-bytes.test.js's rationale: two small functions do
// not earn a bundler, and `node --test` already loads the exact file the
// page ships).
//
// Two axes, one per pure function. The `shiftEnterKeyAction` matrix pins
// what counts as "the Shift+Enter newline chord": each case fixes one
// modifier, event-type, or IME boundary the exact-match check depends on —
// losing any of them would either steal a chord this binding does not own
// (e.g. Ctrl+Shift+Enter, an AltGraph layout's own composed character) or
// fail to fire on the one chord it exists for. The `mergeArmedPrefix`
// cases pin the single-write delivery contract: the armed ESC leaves glued
// to xterm's `\r` in ONE chunk, never as its own message (the split is
// what made Codex read Escape-then-Enter — see the module header), and a
// fizzled arm drops the ESC rather than emitting a stray one.
const test = require("node:test");
const assert = require("node:assert/strict");
const { shiftEnterKeyAction, mergeArmedPrefix } = require("../assets/shift-enter-key.js");

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

test("keydown Shift+Enter arms the one-shot merge and defers to xterm", () => {
  assert.equal(shiftEnterKeyAction(keyEvent({ shiftKey: true })), "arm");
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
  assert.equal(typeof sandbox.window.farhelmShiftEnterKey.mergeArmedPrefix, "function");
  assert.equal(
    sandbox.window.farhelmShiftEnterKey.shiftEnterKeyAction(keyEvent({ shiftKey: true })),
    "arm",
  );
});

test("armed merge glues the ESC onto xterm's CR as one chunk and consumes the arm", () => {
  // The single-write contract itself: `\x1b` and `\r` leave in ONE string,
  // which downstream becomes one websocket message, one send-keys command,
  // and one pty write — the shape verified to make Codex and Claude Code
  // insert a newline instead of reading Escape-then-Enter. Consuming the
  // arm here is what makes a second CR in the same dispatch impossible to
  // double-prefix.
  assert.deepEqual(mergeArmedPrefix(true, "\r"), { send: "\x1b\r", armed: false });
});

test("unarmed chunks pass through byte-for-byte", () => {
  // The wrapper sits on every `term.onData` text chunk — keystrokes,
  // pastes, CR included (mouse reports and other `onBinary` payloads take
  // a separate path entirely) — so its no-op path must be provably
  // transparent.
  assert.deepEqual(mergeArmedPrefix(false, "hello"), { send: "hello", armed: false });
  assert.deepEqual(mergeArmedPrefix(false, "\r"), { send: "\r", armed: false });
});

test("an armed non-CR chunk passes through and the arm survives it", () => {
  // xterm can flush a pending IME composition commit ahead of the chord's
  // own `\r` in the same dispatch. Consuming the arm on that chunk would
  // strip the prefix off the `\r` right behind it — so the arm must ride
  // past non-CR chunks untouched. Bounding how LONG it rides is not this
  // function's job: terminal.js expires the arm in a microtask queued at
  // arm time (end of the same synchronous keydown dispatch), and the
  // browser test asserting Shift+Enter-then-Enter yields ESC,CR,CR is
  // what pins that wiring — a stale arm there would produce ESC,CR,ESC,CR.
  assert.deepEqual(mergeArmedPrefix(true, "composed"), { send: "composed", armed: true });
});

test("an armed chunk that STARTS with CR takes the prefix on the front", () => {
  // xterm emits a bare "\r" for Enter today; if a future xterm batches
  // trailing bytes into the same chunk, the ESC still belongs at the very
  // front and the rest must ride along untouched.
  assert.deepEqual(mergeArmedPrefix(true, "\rrest"), { send: "\x1b\rrest", armed: false });
});
