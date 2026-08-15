// Unit coverage for copy-on-select.js's `copySelectionOnMouseUp`, run with
// node's built-in test runner (matches shift-enter-key.test.js's rationale:
// one small decision does not earn a bundler, and `node --test` already
// loads the exact file the page ships).
//
// The decision is now trivial by design (see copy-on-select.js's header for
// why an earlier "does this differ from the last copy" cache was removed as
// a real bug, not simplified away for its own sake): copy whenever the
// gesture ended with a non-empty local selection, full stop. What these
// cases pin is that BOTH of xterm's own signals are consulted rather than
// either one alone — `hasSelection()` and `getSelection()` are read
// separately in terminal.js, and a caller that got them out of sync (never
// observed in practice, but not provably impossible) must not copy.
const test = require("node:test");
const assert = require("node:assert/strict");
const { copySelectionOnMouseUp } = require("../assets/copy-on-select.js");

test("a non-empty selection copies", () => {
  assert.equal(
    copySelectionOnMouseUp({ hasSelection: true, selectionText: "hello" }),
    true,
  );
});

test("a selection identical to one already copied still copies again", () => {
  // The property the removed cache used to (wrongly) suppress: reselecting
  // the SAME text is a legitimate, distinct copy request — e.g. after
  // something else has overwritten the system clipboard in between. This
  // function has no memory of anything it has copied before, so this is
  // really the same case as the one above, asserted under the name of the
  // bug it fixes rather than merely restated.
  assert.equal(
    copySelectionOnMouseUp({ hasSelection: true, selectionText: "hello" }),
    true,
  );
});

test("hasSelection true but empty selection text skips", () => {
  assert.equal(
    copySelectionOnMouseUp({ hasSelection: true, selectionText: "" }),
    false,
  );
});

test("a plain click without a drag skips (no selection, no text)", () => {
  assert.equal(
    copySelectionOnMouseUp({ hasSelection: false, selectionText: "" }),
    false,
  );
});

test("browser-global branch: window.farhelmCopyOnSelect exists with no module present", () => {
  // Mirrors shift-enter-key.test.js's `node:vm` check: every test above
  // `require()`s this file, which only ever exercises the `module.exports`
  // branch. A fresh vm context with `window` but no `module` is the one
  // environment shape that runs the OTHER branch, so this is what actually
  // pins the browser-visible global rather than assuming it from the
  // CommonJS export alone.
  const vm = require("node:vm");
  const fs = require("node:fs");
  const path = require("node:path");
  const source = fs.readFileSync(path.join(__dirname, "../assets/copy-on-select.js"), "utf8");
  const sandbox = { window: {} };
  vm.createContext(sandbox);
  vm.runInContext(source, sandbox);

  assert.equal(typeof sandbox.window.farhelmCopyOnSelect.copySelectionOnMouseUp, "function");
  assert.equal(
    sandbox.window.farhelmCopyOnSelect.copySelectionOnMouseUp({
      hasSelection: true,
      selectionText: "hello",
    }),
    true,
  );
});
