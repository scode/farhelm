/** The actual shipped matcher must use physical digits and a build decision.
 * Browser tests cannot drive a Mac desktop build, so these cases pin both
 * clients' modifiers without consulting or changing any process environment.
 */
const test = require("node:test");
const assert = require("node:assert/strict");
const { templateNumberKey } = require("../assets/template-number-shortcuts.js");

/** A complete event keeps each refusal case limited to the changed boundary. */
function event(overrides = {}) {
  return { code: "Digit3", key: "#", ctrlKey: false, shiftKey: false, altKey: false,
    metaKey: false, isComposing: false, ...overrides };
}

test("the Mac desktop chord accepts all physical digits without Shift", () => {
  for (const key of "1234567890") {
    assert.equal(templateNumberKey(event({ code: `Digit${key}`, key: "layout-specific", metaKey: true }), true), key);
  }
  for (const override of [{ ctrlKey: true }, { shiftKey: true }, { altKey: true }, { isComposing: true }]) {
    assert.equal(templateNumberKey(event({ metaKey: true, ...override }), true), null);
  }
});

test("every other build uses Ctrl+Shift regardless of the printed character", () => {
  for (const key of "1234567890") {
    assert.equal(templateNumberKey(event({ code: `Digit${key}`, key: "unrelated", ctrlKey: true, shiftKey: true }), false), key);
  }
  for (const override of [{ ctrlKey: false }, { shiftKey: false }, { metaKey: true }, { altKey: true }, { isComposing: true }]) {
    assert.equal(templateNumberKey(event({ ctrlKey: true, shiftKey: true, ...override }), false), null);
  }
  assert.equal(templateNumberKey(event({ metaKey: true }), false), null);
});

test("keypad, partial codes and unrelated keys are never number shortcuts", () => {
  for (const code of ["Numpad3", "Digit", "Digit10", "digit3", "KeyP", ""]) {
    assert.equal(templateNumberKey(event({ code, metaKey: true }), true), null);
    assert.equal(templateNumberKey(event({ code, ctrlKey: true, shiftKey: true }), false), null);
  }
});
