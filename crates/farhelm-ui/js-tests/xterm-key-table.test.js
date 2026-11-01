// Keeps the supervisor's golden key table in step with the vendored
// xterm.js.
//
// The supervisor encodes keys for terminals in terminal hosts, and its Rust
// test holds that encoder to `crates/farhelm-supervisor/tests/fixtures/xterm-keys.jsonl`.
// That only proves parity with today's terminal while the table is what the
// vendored bundle computes now, so this regenerates it from the bundle and
// compares. A re-vendored xterm.js that changes any key fails here, with the
// regeneration command in the message, rather than leaving the supervisor
// matching an old xterm.js.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const { TABLE_PATH, generateTable, tableInputs } = require("./xterm-key-table.js");

test("the checked-in golden key table is what the vendored xterm.js computes", () => {
  const checkedIn = fs.readFileSync(TABLE_PATH, "utf8");
  assert.ok(
    checkedIn === generateTable(),
    "xterm-keys.jsonl is stale: run `node crates/farhelm-ui/js-tests/xterm-key-table.js --write` and review the diff",
  );
});

test("the table covers every key code below 256 and the keys the function special-cases", () => {
  // The sweep is the table's whole point; a generator change that quietly
  // narrowed it would leave the Rust comparison passing over less.
  const inputs = tableInputs();
  const codes = new Set(inputs.map((input) => input.kc));
  for (let kc = 0; kc < 256; kc++) assert.ok(codes.has(kc), `key code ${kc} is missing`);
  const has = (kc, key, code) => inputs.some((input) => input.kc === kc && input.key === key && input.code === code);
  assert.ok(has(13, "Enter", "Enter"), "Enter, for Shift+Enter");
  assert.ok(has(0, "UIKeyInputUpArrow", ""), "the iOS arrow names");
  assert.ok(has(69, "Dead", "KeyE"), "a dead key under Alt");
  assert.ok(has(189, "_", "Minus") && has(50, "@", "Digit2"), "Ctrl with _ and @");
});

test("a control character name the generator does not define fails the extraction", () => {
  // Without this, a re-vendored function reading a new `C0.X` would write
  // the text "undefined" into rows, and the Rust test would report it as an
  // encoder bug rather than an extraction gap.
  const { C0 } = require("./xterm-vendored.js");
  assert.equal(C0.ESC, "\x1b");
  assert.throws(() => C0.ETX, /C0\.ETX/);
});
