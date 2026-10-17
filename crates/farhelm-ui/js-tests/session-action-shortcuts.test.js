/** Pin both clients' chords against the shipped matcher. The browser suite
 * exercises dispatch, while Mac desktop modifiers cannot be sent by that suite.
 */
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const { sessionActionKey, sessionActionTextField } = require("../assets/session-action-shortcuts.js");

/** Complete modifiers make each refusal differ at just the named boundary. */
function event(overrides = {}) {
  return { code: "KeyW", key: "layout-specific", ctrlKey: false, shiftKey: false, altKey: false,
    metaKey: false, isComposing: false, ...overrides };
}

test("Mac desktop session actions reserve only Cmd-W and Cmd-L", () => {
  for (const [code, action] of [["KeyW", "delete"], ["KeyL", "rename"]]) {
    assert.equal(sessionActionKey(event({ code, metaKey: true }), true), action);
    for (const override of [{ metaKey: false }, { ctrlKey: true }, { shiftKey: true }, { altKey: true }, { isComposing: true }]) {
      assert.equal(sessionActionKey(event({ code, metaKey: true, ...override }), true), null);
    }
  }
  assert.equal(sessionActionKey(event({ code: "Backspace", metaKey: true }), true), null);
});

test("browsers reserve Ctrl-Shift-Backspace and Ctrl-Shift-L and keep Cmd chords", () => {
  for (const [code, action] of [["Backspace", "delete"], ["KeyL", "rename"]]) {
    assert.equal(sessionActionKey(event({ code, ctrlKey: true, shiftKey: true }), false), action);
    for (const override of [{ ctrlKey: false }, { shiftKey: false }, { metaKey: true }, { altKey: true }, { isComposing: true }]) {
      assert.equal(sessionActionKey(event({ code, ctrlKey: true, shiftKey: true, ...override }), false), null);
    }
  }
  for (const code of ["KeyW", "KeyN", "Delete", "", "keyl"]) {
    assert.equal(sessionActionKey(event({ code, ctrlKey: true, shiftKey: true }), false), null);
    assert.equal(sessionActionKey(event({ code, metaKey: true }), true), code === "KeyW" ? "delete" : null);
  }
});

test("browser Delete leaves ordinary editors alone and consumes xterm input", () => {
  const target = (terminal, control, isContentEditable = false) => ({
    isContentEditable,
    closest(selector) { return selector === ".xterm-helper-textarea" ? terminal : control; },
  });
  assert.equal(sessionActionTextField(target(true, true)), false);
  assert.equal(sessionActionTextField(target(false, true)), true);
  assert.equal(sessionActionTextField(target(false, false, true)), true);
  assert.equal(sessionActionTextField(target(false, false)), false);
  assert.equal(sessionActionTextField(null), false);
});

test("the installed Mac Delete handler consumes absent busy and repeated actions without a click", () => {
  let handler;
  let trigger = null;
  let clicks = 0;
  const window = { addEventListener(name, callback, capture) {
    assert.equal(name, "keydown");
    assert.equal(capture, true);
    handler = callback;
  } };
  const document = { querySelectorAll() { return []; }, querySelector() { return trigger; } };
  vm.runInNewContext(fs.readFileSync(require.resolve("../assets/session-action-shortcuts.js"), "utf8"), { window, document });
  window.installSessionActionShortcuts(true);
  const press = (repeat = false) => {
    let prevented = false;
    let stopped = false;
    handler({ ...event({ metaKey: true }), repeat,
      preventDefault() { prevented = true; }, stopPropagation() { stopped = true; } });
    assert.equal(prevented, true, "Cmd-W must never fall through to native window close");
    assert.equal(stopped, true, "the chord must not reach terminal input");
  };
  press();
  trigger = { disabled: true, click() { clicks += 1; } };
  press();
  trigger.disabled = false;
  press(true);
  assert.equal(clicks, 0);
  press();
  assert.equal(clicks, 1, "one deliberate chord uses the current header control");
});
