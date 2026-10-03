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

// --- The drag that copies nothing -----------------------------------------
//
// These pin the notice terminal.js raises when a plain drag goes to a
// program that has mouse reporting on and copies nothing. Why they matter:
// the notice must appear only in that one case (a notice after a drag that
// DID copy, or after a click, would be noise users learn to ignore), and the
// key it names must be the one xterm actually honors on the platform.
const {
  forcingModifier,
  dragMayHaveCopiedNothing,
  dragCopyNoticeText,
  takeNoticeOnce,
  pressForcesSelection,
  isOsc52Write,
  DRAG_THRESHOLD_PX,
} = require("../assets/copy-on-select.js");

/** A gesture that should raise the notice; each test changes one field. */
const plainUncopiedDrag = () => ({
  button: 0,
  moved: DRAG_THRESHOLD_PX + 10,
  onScreen: true,
  trackingAtPress: true,
  forced: false,
  hasSelection: false,
  osc52SincePress: false,
});

test("a plain drag the program took without copying raises the notice", () => {
  assert.equal(dragMayHaveCopiedNothing(plainUncopiedDrag()), true);
});

test("no notice for a gesture that copied or never tried to", () => {
  const cases = {
    "a click (pointer barely moved)": { moved: DRAG_THRESHOLD_PX - 1 },
    "a drag the program answered with OSC 52": { osc52SincePress: true },
    "a forced Option/Shift drag (Farhelm made its own selection)": { forced: true, hasSelection: true },
    // Option/Shift held but the drag stayed inside one cell, so xterm made
    // no selection: the user is already holding the key the notice names.
    "a forced drag that selected nothing": { forced: true, hasSelection: false },
    // An earlier forced selection survives a plain press under mouse
    // tracking; the existing copy path re-copies it, so no notice either.
    "a plain drag over a surviving local selection": { hasSelection: true },
    "a pane with no mouse tracking at the press": { trackingAtPress: false },
    "a press outside the terminal screen (the scrollbar)": { onScreen: false },
    "a right or middle button drag": { button: 2 },
  };
  for (const [name, change] of Object.entries(cases)) {
    assert.equal(dragMayHaveCopiedNothing({ ...plainUncopiedDrag(), ...change }), false, name);
  }
  assert.equal(dragMayHaveCopiedNothing(null), false, "no gesture at all");
});

test("the forcing modifier follows xterm's own Mac platform list", () => {
  // Exactly the `navigator.platform` values the vendored xterm.js treats as
  // Mac; anything else, iPad included, forces with Shift.
  for (const mac of ["Macintosh", "MacIntel", "MacPPC", "Mac68K"]) {
    assert.equal(forcingModifier(mac), "Option", mac);
  }
  for (const other of ["Linux x86_64", "Win32", "iPad", "", undefined]) {
    assert.equal(forcingModifier(other), "Shift", String(other));
  }
});

test("the notice names the platform's key, with the agent's own instruction when known", () => {
  const generic = dragCopyNoticeText({ platform: "Linux x86_64", appHint: null });
  assert.match(generic, /handles mouse selection itself/);
  assert.match(generic, /own copy command/);
  assert.match(generic, /hold Shift while dragging/);

  const codex = dragCopyNoticeText({
    platform: "MacIntel",
    appHint: "Codex handles mouse selection itself: press Ctrl+C while it is still highlighted.",
  });
  assert.match(codex, /^Codex handles mouse selection itself/);
  assert.doesNotMatch(codex, /own copy command/, "the agent's instruction replaces the generic one");
  assert.match(codex, /hold Option while dragging/);
});

test("each distinct notice shows once per page", () => {
  const shown = new Set();
  assert.equal(takeNoticeOnce(shown, "generic"), true);
  assert.equal(takeNoticeOnce(shown, "generic"), false, "the same text is not repeated");
  assert.equal(takeNoticeOnce(shown, "codex"), true, "a different text still shows");
});

test("the forcing modifier is read from the press with xterm's rule", () => {
  // Option on xterm's Mac platforms, Shift elsewhere; the other key does
  // nothing, so holding it is not a forced press.
  assert.equal(pressForcesSelection({ altKey: true, shiftKey: false }, "MacIntel"), true);
  assert.equal(pressForcesSelection({ altKey: false, shiftKey: true }, "MacIntel"), false);
  assert.equal(pressForcesSelection({ altKey: false, shiftKey: true }, "Linux x86_64"), true);
  assert.equal(pressForcesSelection({ altKey: true, shiftKey: false }, "Linux x86_64"), false);
});

test("only an OSC 52 write counts as the program copying, never a read query", () => {
  assert.equal(isOsc52Write("c;YW5zd2VyZWQ="), true);
  assert.equal(isOsc52Write(";YW5zd2VyZWQ="), true, "an empty selection target is still a write");
  assert.equal(isOsc52Write("c;?"), false);
  assert.equal(isOsc52Write("?"), false);
});
