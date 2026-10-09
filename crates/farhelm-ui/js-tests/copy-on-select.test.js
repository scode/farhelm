// Unit coverage for copy-on-select.js's copy decisions and notice lifecycle, run with
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
  createDragCopyNotice,
  DRAG_NOTICE_MS,
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

// --- Notice placement and lifecycle --------------------------------------

/**
 * Minimal DOM and clock seam for the shipped notice controller. Time advances
 * explicitly, so expiry and replacement are asserted at their exact boundary
 * rather than inferred from a slow wall-clock wait. Pointer/focus behavior is
 * also exercised against real DOM in mouse-modes.spec.ts.
 */
function noticeFixture() {
  const { placeTooltip } = require("../assets/tooltip.js");
  const listeners = new Map();
  const text = { textContent: "" };
  const showing = new Set();
  const dismiss = {
    addEventListener: (type, fn) => listeners.set(type, fn),
    removeEventListener: (type, fn) => {
      assert.equal(listeners.get(type), fn);
      listeners.delete(type);
    },
  };
  const size = { width: 200, height: 50 };
  const viewport = { width: 800, height: 600 };
  const el = {
    querySelector: (selector) => selector.endsWith("-text") ? text : dismiss,
    classList: { add: (name) => showing.add(name), remove: (name) => showing.delete(name) },
    getBoundingClientRect: () => size,
    style: {},
  };
  let now = 0;
  let nextId = 0;
  const timers = new Map();
  const controller = createDragCopyNotice(el, {
    place: placeTooltip,
    viewport: () => viewport,
    setTimeout: (fn, ms) => { const id = ++nextId; timers.set(id, { fn, at: now + ms }); return id; },
    clearTimeout: (id) => timers.delete(id),
  });
  return {
    controller, el, text, showing, listeners, timers, viewport,
    advance(ms) {
      now += ms;
      for (const [id, timer] of timers) {
        if (timer.at <= now) { timers.delete(id); timer.fn(); }
      }
    },
  };
}

/** Placement must follow the release point, including drags at every window edge. */
test("the notice stays above the release and entirely inside the viewport", () => {
  const f = noticeFixture();
  f.controller.show("copied nothing", { x: 300, y: 400 });
  assert.deepEqual(f.el.style, { left: "200px", top: "344px" });
  for (const point of [{ x: 0, y: 0 }, { x: 800, y: 600 }, { x: 300, y: 10 }]) {
    f.controller.show("copied nothing", point);
    const left = parseFloat(f.el.style.left), top = parseFloat(f.el.style.top);
    assert.ok(left >= 4 && left + 200 <= 796);
    assert.ok(top >= 4 && top + 50 <= 596);
  }
  f.viewport.width = 400;
  f.viewport.height = 300;
  f.controller.reposition();
  assert.ok(parseFloat(f.el.style.left) + 200 <= 396);
  assert.ok(parseFloat(f.el.style.top) + 50 <= 296);
});

/** The same text is a fresh notice every time; an older deadline cannot retire its replacement. */
test("every show gets thirty seconds, even when it replaces identical text", () => {
  const f = noticeFixture();
  assert.equal(DRAG_NOTICE_MS, 30_000);
  f.controller.show("same notice", { x: 300, y: 400 });
  f.advance(29_999);
  assert.equal(f.showing.has("showing"), true);
  f.controller.show("same notice", { x: 500, y: 300 });
  assert.equal(f.el.style.left, "400px");
  assert.equal(f.timers.size, 1);
  f.advance(1);
  assert.equal(f.showing.has("showing"), true, "the first deadline cannot hide the replacement");
  f.advance(29_999);
  assert.equal(f.showing.has("showing"), false);
  assert.equal(f.text.textContent, "");
  assert.equal(f.timers.size, 0);
  f.controller.show("same notice", { x: 300, y: 400 });
  assert.equal(f.showing.has("showing"), true, "an expired notice may show again");
});

/** Dismissal keeps native focus and input local; disposal releases every owned listener and timer. */
test("dismiss and terminal-press hiding retire the notice without retaining input handlers", () => {
  const f = noticeFixture();
  const event = () => ({
    prevented: false, stopped: false,
    preventDefault() { this.prevented = true; },
    stopPropagation() { this.stopped = true; },
  });
  f.controller.show("notice", { x: 300, y: 400 });
  const press = event();
  f.listeners.get("mousedown")(press);
  assert.equal(press.prevented, true);
  assert.equal(press.stopped, true);
  assert.equal(f.showing.has("showing"), true, "pressing × is not the terminal press dismissal");
  const click = event();
  f.listeners.get("click")(click);
  assert.equal(click.prevented, true);
  assert.equal(click.stopped, true);
  assert.equal(f.showing.has("showing"), false);
  assert.equal(f.timers.size, 0);
  f.controller.show("notice", { x: 300, y: 400 });
  f.controller.hide();
  assert.equal(f.showing.has("showing"), false);
  f.controller.show("notice", { x: 300, y: 400 });
  f.controller.dispose();
  assert.equal(f.showing.has("showing"), false);
  assert.equal(f.listeners.size, 0);
  assert.equal(f.timers.size, 0);
});
