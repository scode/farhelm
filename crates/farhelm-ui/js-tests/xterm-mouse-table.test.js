// Keeps the supervisor's golden mouse table in step with the vendored
// xterm.js.
//
// The supervisor encodes mouse reports for terminals in terminal hosts, and
// its Rust test holds that encoder to
// `crates/farhelm-supervisor/tests/fixtures/xterm-mouse.jsonl`. That proves
// parity with today's terminal only while the table is what the vendored
// bundle's mouse service computes now, so this regenerates it and compares.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const { EVENTS, PLACES, PROTOCOLS, TABLE_PATH, generateTable } = require("./xterm-mouse-table.js");

test("the checked-in golden mouse table is what the vendored xterm.js computes", () => {
  const checkedIn = fs.readFileSync(TABLE_PATH, "utf8");
  assert.ok(
    checkedIn === generateTable(),
    "xterm-mouse.jsonl is stale: run `node crates/farhelm-ui/js-tests/xterm-mouse-table.js --write` and review the diff",
  );
});

test("the table covers every event kind, every tracked protocol and the default encoding's limit", () => {
  // A generator change that quietly narrowed the sweep would leave the Rust
  // comparison passing over less.
  assert.deepEqual([...new Set(EVENTS.map(([kind]) => kind))], ["down", "up", "move", "wheel"]);
  assert.deepEqual(PROTOCOLS, ["NONE", "VT200", "DRAG", "ANY"]);
  assert.ok(PLACES.some(([x]) => x === 222) && PLACES.some(([x]) => x === 223), "both sides of column 223");
  assert.ok(PLACES.some(([x, , cols]) => x === cols), "a column past the screen");
});
