// Unit coverage for grid-input.js, the grid terminal's input decisions.
//
// New terminals type the way terminals of the xterm family do, because
// shells and agents read keys that way: the same keys produce sequences,
// the same keys are left to the browser's text input, and the mouse lands
// on the same cells. The page does not encode anything itself (the
// supervisor does, from these messages), so what this file pins is the
// DECISION side: which keydowns become `key` messages, which become
// `text`, and which never reach the program.
//
// The whole-key-space checks are in grid-input-tables.test.js, which holds
// grid-input.js to Farhelm's checked-in key, keydown and Alt-click tables.
// This file has the readable cases: one rule each, with its reason.
const test = require("node:test");
const assert = require("node:assert/strict");
const input = require("../assets/grid-input.js");

function keyEvent(overrides) {
  return Object.assign(
    { key: "", code: "", keyCode: 0, shiftKey: false, altKey: false, ctrlKey: false, metaKey: false, isComposing: false },
    overrides,
  );
}

// keydownAction: which keydowns reach the program at keydown.

const LINUX = "Linux x86_64";
const MAC = "MacIntel";
const WINDOWS = "Win32";

test("an ordinary letter, Enter, Tab, Escape, arrows and Ctrl+C are sent as key messages and cancelled", () => {
  // These keys have a sequence of their own at keydown; sending them as `key`
  // lets the supervisor encode them with the program's current modes.
  for (const ev of [
    keyEvent({ key: "a", code: "KeyA", keyCode: 65 }),
    keyEvent({ key: "Enter", code: "Enter", keyCode: 13 }),
    keyEvent({ key: "Enter", code: "Enter", keyCode: 13, shiftKey: true }),
    keyEvent({ key: "Tab", code: "Tab", keyCode: 9 }),
    keyEvent({ key: "Escape", code: "Escape", keyCode: 27 }),
    keyEvent({ key: "ArrowUp", code: "ArrowUp", keyCode: 38 }),
    keyEvent({ key: "c", code: "KeyC", keyCode: 67, ctrlKey: true }),
    keyEvent({ key: "o", code: "KeyO", keyCode: 79, altKey: true }),
  ]) {
    assert.deepEqual(input.keydownAction(ev, LINUX, false), { action: "send", cancel: true, deadKeyPending: false }, ev.key);
  }
});

test("Shift+Enter is an ordinary key message carrying the Shift bit", () => {
  // SPEC.md's single-write ESC CR is the supervisor's encoding of this
  // message; the page must neither drop the chord nor split it.
  const ev = keyEvent({ key: "Enter", code: "Enter", keyCode: 13, shiftKey: true });
  assert.equal(input.keydownAction(ev, MAC, false).action, "send");
  assert.deepEqual(input.keyMessage(ev, MAC), { t: "key", key: "Enter", code: "Enter", kc: 13, mods: 1, mac: true });
});

test("keys whose character comes from text input are passed to the browser uncancelled", () => {
  // Space and capitals arrive through the input event (as in old
  // terminals); cancelling them here would lose the character.
  assert.deepEqual(input.keydownAction(keyEvent({ key: " ", code: "Space", keyCode: 32 }), LINUX, false), {
    action: "pass",
    cancel: false,
    deadKeyPending: false,
  });
  assert.equal(input.keydownAction(keyEvent({ key: "A", code: "KeyA", keyCode: 65, shiftKey: true }), LINUX, false).action, "pass");
});

test("Option on a Mac and AltGr on Windows compose characters instead of sending Meta sequences", () => {
  // Third-level shift: `ø` from Option+o, `@` from AltGr+q on a German
  // layout. These are left to text input, uncancelled.
  assert.deepEqual(input.keydownAction(keyEvent({ key: "ø", code: "KeyO", keyCode: 79, altKey: true }), MAC, false), {
    action: "pass",
    cancel: false,
    deadKeyPending: false,
  });
  const altGr = keyEvent({ key: "@", code: "KeyQ", keyCode: 81, altKey: true, ctrlKey: true });
  assert.equal(input.keydownAction(altGr, WINDOWS, false).action, "pass");
  // Option with an arrow key is not third-level: it is a modified arrow.
  assert.equal(input.keydownAction(keyEvent({ key: "ArrowLeft", code: "ArrowLeft", keyCode: 37, altKey: true }), MAC, false).action, "send");
});

test("Cmd chords on a Mac that have no sequence stay with the browser", () => {
  // Cmd+C and Cmd+V must keep their default actions (copy, paste); Cmd+A
  // is the terminal's own select-all, which is local and never sent.
  assert.deepEqual(input.keydownAction(keyEvent({ key: "c", code: "KeyC", keyCode: 67, metaKey: true }), MAC, false), {
    action: "pass",
    cancel: false,
    deadKeyPending: false,
  });
  assert.equal(input.keydownAction(keyEvent({ key: "a", code: "KeyA", keyCode: 65, metaKey: true }), MAC, false).action, "select-all");
});

test("Shift+PageUp and Shift+PageDown scroll the local view and are cancelled", () => {
  assert.deepEqual(input.keydownAction(keyEvent({ key: "PageUp", code: "PageUp", keyCode: 33, shiftKey: true }), LINUX, false), {
    action: "page-up",
    cancel: true,
    deadKeyPending: false,
  });
  assert.equal(input.keydownAction(keyEvent({ key: "PageDown", code: "PageDown", keyCode: 34, shiftKey: true }), LINUX, false).action, "page-down");
});

test("a dead key leaves the next sending key to text input, then the state clears", () => {
  // `´` then `e` must type `é`, not `´` and a separately sent `e`.
  const dead = input.keydownAction(keyEvent({ key: "Dead", code: "Quote", keyCode: 222 }), LINUX, false);
  assert.deepEqual(dead, { action: "pass", cancel: false, deadKeyPending: true });
  const next = input.keydownAction(keyEvent({ key: "e", code: "KeyE", keyCode: 69 }), LINUX, dead.deadKeyPending);
  assert.deepEqual(next, { action: "pass", cancel: false, deadKeyPending: false });
  const after = input.keydownAction(keyEvent({ key: "e", code: "KeyE", keyCode: 69 }), LINUX, next.deadKeyPending);
  assert.equal(after.action, "send");
});

test("keys an input method is composing are ignored", () => {
  for (const ev of [keyEvent({ key: "a", keyCode: 229 }), keyEvent({ key: "a", keyCode: 65, isComposing: true })]) {
    assert.equal(input.keydownAction(ev, LINUX, false).action, "ignore");
  }
});

// Messages and their bounds.

test("key messages carry the browser's values and refuse fields past 64 bytes", () => {
  const ev = keyEvent({ key: "ArrowUp", code: "ArrowUp", keyCode: 38, shiftKey: true, ctrlKey: true, metaKey: true });
  assert.deepEqual(input.keyMessage(ev, LINUX), { t: "key", key: "ArrowUp", code: "ArrowUp", kc: 38, mods: 13 });
  assert.equal(input.keyMessage(keyEvent({ key: "x".repeat(65), keyCode: 65 })), null);
  assert.equal(input.keyMessage(keyEvent({ key: "a", code: "é".repeat(33), keyCode: 65 })), null);
});

test("text is split into 16 KiB messages on character boundaries", () => {
  assert.deepEqual(input.textMessages(""), []);
  assert.deepEqual(input.textMessages("hi"), [{ t: "text", data: "hi" }]);
  // 16 KiB of three-byte characters plus a four-byte one that must not be
  // split across messages.
  const long = "€".repeat(5461) + "😀" + "x";
  const parts = input.textMessages(long);
  assert.equal(parts.map((m) => m.data).join(""), long);
  for (const part of parts) assert.ok(Buffer.byteLength(part.data) <= input.MAX_TEXT_BYTES);
  assert.equal(parts.length, 2);
  assert.ok(parts[1].data.startsWith("😀"));
});

test("control-heavy text is split within the encoded bound, and nothing is dropped", () => {
  // JSON writes a control character as a six-byte escape, so 16 KiB of
  // them would encode to about 96 KiB; every part must still encode within
  // 64 KiB, or `encodeInput` would refuse it and the text would be lost.
  for (const text of ["\x01".repeat(20000), "\x1b[A".repeat(9000), '"\\'.repeat(20000), "\ud800".repeat(12000)]) {
    const parts = input.textMessages(text);
    assert.equal(parts.map((part) => part.data).join(""), text);
    for (const part of parts) {
      const bytes = input.encodeInput(part);
      assert.notEqual(bytes, null, "every part must encode within the bound");
      assert.ok(Buffer.byteLength(part.data) <= input.MAX_TEXT_BYTES);
    }
  }
});

test("a paste splits within its bounds, at characters, never between CR and LF", () => {
  // The supervisor converts line endings and adds bracketed-paste markers
  // over the whole paste; a CRLF split across parts would convert as a lone
  // CR and a lone LF.
  assert.deepEqual(input.pasteMessages(""), []);
  assert.deepEqual(input.pasteMessages("one"), [{ t: "paste", data: "one", last: true }]);
  const crlfAtBoundary = "x".repeat(32 * 1024 - 1) + "\r\n" + "tail";
  const parts = input.pasteMessages(crlfAtBoundary);
  assert.equal(parts.map((part) => part.data).join(""), crlfAtBoundary);
  assert.deepEqual(parts.map((part) => part.last), [false, true]);
  assert.ok(parts[1].data.startsWith("\r\n"), "the CR stays with its LF");
  for (const text of ["\u20ac".repeat(30000) + "\u{1f600}", "\x01".repeat(40000), "a\r\n".repeat(20000)]) {
    const split = input.pasteMessages(text);
    assert.equal(split.map((part) => part.data).join(""), text);
    for (const part of split) {
      assert.ok(Buffer.byteLength(part.data) <= 32 * 1024);
      assert.notEqual(input.encodeInput(part), null);
    }
    for (let i = 1; i < split.length; i++) {
      assert.ok(!(split[i - 1].data.endsWith("\r") && split[i].data.startsWith("\n")), "no split between CR and LF");
    }
    assert.equal(split.filter((part) => part.last).length, 1);
    assert.equal(split[split.length - 1].last, true);
  }
});

test("an encoded input message is JSON bytes, and none past 64 KiB is produced", () => {
  const bytes = input.encodeInput({ t: "text", data: "a\"<b>" });
  assert.deepEqual(JSON.parse(Buffer.from(bytes).toString("utf8")), { t: "text", data: "a\"<b>" });
  assert.equal(input.encodeInput({ t: "text", data: "x".repeat(70 * 1024) }), null);
});

// Mouse and wheel.

test("pointer offsets map to cells with the grid's rounding and clamping", () => {
  // Cell 8 px wide, 17 px tall, 80x24. A boundary pixel belongs to the
  // cell before it (round up, then subtract one), and anything
  // outside the grid clamps to its edge.
  assert.deepEqual(input.mouseCell(0, 0, 8, 17, 80, 24), { x: 0, y: 0 });
  assert.deepEqual(input.mouseCell(8, 17, 8, 17, 80, 24), { x: 0, y: 0 });
  assert.deepEqual(input.mouseCell(8.5, 17.5, 8, 17, 80, 24), { x: 1, y: 1 });
  assert.deepEqual(input.mouseCell(-50, 10000, 8, 17, 80, 24), { x: 0, y: 23 });
  assert.deepEqual(input.mouseCell(100000, -1, 8, 17, 80, 24), { x: 79, y: 0 });
});

test("only the three main buttons are reported, and a move reports the held one", () => {
  assert.equal(input.pressButton(0), 0);
  assert.equal(input.pressButton(2), 2);
  assert.equal(input.pressButton(3), null);
  assert.equal(input.moveButton(0), -1);
  assert.equal(input.moveButton(1), 0);
  assert.equal(input.moveButton(4), 1);
  assert.equal(input.moveButton(2), 2);
});

test("the program's mouse mode decides which events are sent", () => {
  const table = [
    ["off", "down", 0, false],
    ["off", "wheel", 0, false],
    ["click", "down", 0, true],
    ["click", "up", 0, true],
    ["click", "wheel", 0, true],
    ["click", "move", 0, false],
    ["drag", "move", 0, true],
    ["drag", "move", -1, false],
    ["motion", "move", -1, true],
  ];
  for (const [mode, kind, button, sent] of table) {
    assert.equal(input.modeReports(mode, kind, button), sent, `${mode} ${kind} ${button}`);
  }
});

test("repeated moves within one cell are sent once, other events always", () => {
  const dedupe = input.createMouseDeduper();
  const move = { t: "mouse", kind: "move", button: 0, x: 3, y: 4, mods: 0 };
  assert.equal(dedupe.shouldSend(move), true);
  assert.equal(dedupe.shouldSend({ ...move }), false);
  assert.equal(dedupe.shouldSend({ ...move, mods: 8 }), false, "Meta is not compared");
  assert.equal(dedupe.shouldSend({ ...move, x: 4 }), true);
  assert.equal(dedupe.shouldSend({ ...move, x: 4, kind: "up" }), true);
  assert.equal(dedupe.shouldSend({ ...move, x: 4, kind: "up" }), true, "releases are never deduplicated");
});

test("small pixel wheel deltas accumulate into whole lines", () => {
  // A trackpad's 4 px events at 17 px rows move 0.07 lines each (with
  // the 0.3 damping under 50 px), so 15 of them make one line.
  const wheel = input.createWheelAccumulator();
  let lines = 0;
  for (let i = 0; i < 14; i++) lines += wheel.consume({ deltaY: 4, deltaMode: 0 }, 17, 24);
  assert.equal(lines, 0);
  assert.equal(wheel.consume({ deltaY: 4, deltaMode: 0 }, 17, 24), 1);
  assert.equal(input.createWheelAccumulator().consume({ deltaY: 3, deltaMode: 1 }, 17, 24), 3);
  assert.equal(input.createWheelAccumulator().consume({ deltaY: 1, deltaMode: 2 }, 17, 24), 24);
  assert.equal(input.createWheelAccumulator().consume({ deltaY: -100, deltaMode: 0, shiftKey: true }, 17, 24), 0);
  assert.equal(input.createWheelAccumulator().consume({ deltaY: -34, deltaMode: 0, altKey: true }, 17, 24), -3);
});

test("the wheel goes to the program under mouse reporting or on the alternate screen", () => {
  assert.equal(input.wheelGoesToProgram({ mouse: "click", altScreen: false }), true);
  assert.equal(input.wheelGoesToProgram({ mouse: "off", altScreen: true }), true);
  assert.equal(input.wheelGoesToProgram({ mouse: "off", altScreen: false }), false);
});

test("key messages say whether the viewer is a Mac, by an exact platform list", () => {
  // The supervisor's encoder picks Mac-only encodings from this flag, and
  // the client's own decisions (Option as a third-level shift, Cmd+A) read
  // the same list, so both sides agree on which viewers are Macs (the list
  // old terminals' xterm.js uses): exact `navigator.platform` values, not
  // anything containing "Mac". Any other viewer leaves the flag out, the
  // format's canonical "false".
  const ev = keyEvent({ key: "a", code: "KeyA", keyCode: 65 });
  for (const platform of ["Macintosh", "MacIntel", "MacPPC", "Mac68K"]) {
    assert.equal(input.keyMessage(ev, platform).mac, true, platform);
    assert.equal(input.platformOf(platform).isMac, true, platform);
  }
  for (const platform of ["Linux x86_64", "Win32", "iPad", "iPhone", "MacSomething", "", undefined]) {
    assert.equal("mac" in input.keyMessage(ev, platform), false, String(platform));
    assert.equal(input.platformOf(platform).isMac, false, String(platform));
  }
  // Option on a Mac composes text rather than sending ESC-prefixed keys;
  // a platform outside the list is not treated as one.
  const option = keyEvent({ key: "\u00e5", code: "KeyA", keyCode: 65, altKey: true });
  assert.equal(input.keydownAction(option, "MacIntel", false).action, "pass");
  assert.equal(input.keydownAction(option, "MacSomething", false).action, "send");
});

/** `ev` with `getModifierState` answering whether AltGraph is held. */
function withModifierState(ev, altGraph) {
  return { ...ev, getModifierState: (name) => name === "AltGraph" && altGraph };
}

test("each keydown filter keeps its keys from being sent", () => {
  // The keydown table's rules (grid-input-tables.test.js), one readable
  // case each.
  const pass = (ev, platform, dead = false) => input.keydownAction(ev, platform, dead).action;
  // Option+F1-F12 on a Mac: third-level shift, so nothing at keydown.
  for (let keyCode = 112; keyCode <= 123; keyCode++) {
    assert.equal(pass(keyEvent({ key: `F${keyCode - 111}`, code: `F${keyCode - 111}`, keyCode, altKey: true }), MAC), "pass");
    // Elsewhere Alt+F5 is a modified function key.
    assert.equal(pass(keyEvent({ key: `F${keyCode - 111}`, code: `F${keyCode - 111}`, keyCode, altKey: true }), LINUX), "send");
  }
  // Option with the iOS arrow keys (key code 0) on a Mac.
  assert.equal(pass(keyEvent({ key: "UIKeyInputUpArrow", keyCode: 0, altKey: true }), MAC), "pass");
  assert.equal(pass(keyEvent({ key: "UIKeyInputUpArrow", keyCode: 0 }), MAC), "send");
  // Ctrl+Alt on Windows acts as AltGr, and so does the AltGraph modifier.
  assert.equal(pass(keyEvent({ key: "q", code: "KeyQ", keyCode: 81, ctrlKey: true, altKey: true }), WINDOWS), "pass");
  assert.equal(pass(keyEvent({ key: "q", code: "KeyQ", keyCode: 81, ctrlKey: true, altKey: true }), LINUX), "send");
  assert.equal(pass(withModifierState(keyEvent({ key: "@", code: "KeyQ", keyCode: 81 }), true), WINDOWS), "pass");
  // Capitals are left to text input.
  assert.equal(pass(keyEvent({ key: "Q", code: "KeyQ", keyCode: 81, shiftKey: true }), LINUX), "pass");
  // The key after a dead key (or AltGraph) is left to text input.
  assert.equal(pass(keyEvent({ key: "e", code: "KeyE", keyCode: 69 }), LINUX, true), "pass");
  assert.equal(input.keydownAction(keyEvent({ key: "AltGraph", code: "AltRight", keyCode: 225 }), LINUX, false).deadKeyPending, true);
});

test("a move equal to the last reported mouse event is dropped", () => {
  // A move is compared with the last event reported, whatever its
  // kind: same cell, button and Shift/Alt/Ctrl, and the same action. So a
  // run of moves in one cell sends one, a different button or modifier is
  // new, and a press, release or wheel step in between starts over.
  const dedupe = input.createMouseDeduper();
  const at = (kind, button, x, y, mods = 0) => ({ t: "mouse", kind, button, x, y, mods });
  assert.equal(dedupe.shouldSend(at("down", 0, 2, 2)), true);
  assert.equal(dedupe.shouldSend(at("move", 0, 2, 2)), true, "a move after a press in its cell is a different action");
  assert.equal(dedupe.shouldSend(at("move", 0, 2, 2)), false);
  assert.equal(dedupe.shouldSend(at("move", 1, 2, 2)), true, "another button");
  assert.equal(dedupe.shouldSend(at("move", 1, 2, 2, 1)), true, "Shift");
  assert.equal(dedupe.shouldSend(at("move", 1, 2, 2, 1 | 8)), false, "Meta is not compared");
  assert.equal(dedupe.shouldSend(at("move", 1, 2, 3, 1)), true, "another row");
  assert.equal(dedupe.shouldSend(at("wheel", 1, 2, 3, 1)), true);
  assert.equal(dedupe.shouldSend(at("move", 1, 2, 3, 1)), true, "a wheel step in between starts over");
  assert.equal(dedupe.shouldSend(at("move", -1, 2, 3, 1)), true, "no button held is a button of its own");
  assert.equal(dedupe.shouldSend(at("move", -1, 2, 3, 1)), false);
});

// ## Alt-click cursor moves

test("Alt-click's arrows become key messages the supervisor encodes", () => {
  const sequence = "\x1b[D\x1b[D\x1b[A\x1b[C";
  assert.deepEqual(input.arrowKeyMessages(sequence, "Linux x86_64"), [
    { t: "key", key: "ArrowLeft", code: "ArrowLeft", kc: 37, mods: 0 },
    { t: "key", key: "ArrowLeft", code: "ArrowLeft", kc: 37, mods: 0 },
    { t: "key", key: "ArrowUp", code: "ArrowUp", kc: 38, mods: 0 },
    { t: "key", key: "ArrowRight", code: "ArrowRight", kc: 39, mods: 0 },
  ]);
  assert.deepEqual(input.arrowKeyMessages("\x1b[A", "MacIntel"), [
    { t: "key", key: "ArrowUp", code: "ArrowUp", kc: 38, mods: 0, mac: true },
  ]);
  assert.deepEqual(input.arrowKeyMessages("", "MacIntel"), []);
});
