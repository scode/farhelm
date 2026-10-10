const assert = require("node:assert/strict");
const test = require("node:test");
const { storedWidth, clampWidth } = require("../assets/sidebar-width.js");

// Bad browser storage must not change a fresh profile's layout or accept
// the permissive number formats that Number() alone would silently parse.
test("stored sidebar width requires plain decimal digits", () => {
  for (const value of [null, "", " 400", "400 ", "-240", "+400", "3e2", "340.5", "NaN", "x"]) {
    assert.equal(storedWidth(value), 340);
  }
  assert.equal(storedWidth("420"), 420);
  assert.equal(storedWidth("00420"), 420);
});

// A remembered width from a larger display remains usable on this device;
// huge decimal strings also clamp instead of leaking Infinity into CSS.
test("stored widths clamp at both bounds", () => {
  assert.equal(storedWidth("0"), 240);
  assert.equal(storedWidth("240"), 240);
  assert.equal(storedWidth("600"), 600);
  assert.equal(storedWidth("999999"), 600);
  assert.equal(storedWidth("9".repeat(400)), 600);
  assert.equal(clampWidth(200), 240);
  assert.equal(clampWidth(650), 600);
});
