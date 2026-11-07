// Holds grid-input.js to Farhelm's checked-in input tables
// (grid-input-tables.js says what each one pins and how to change it).
//
// Why: every row of these tables is a key or click a shell or agent reads
// in a particular way, and the tables are the only place that behavior is
// written down whole. A change to grid-input.js that moves any row fails
// here, with the regeneration command, so it is made on purpose and
// reviewed as a diff of rows rather than slipping through. The key table
// is the same file the supervisor's Rust test holds its encoder to, so a
// change on one side only fails one of the two.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const tables = require("./grid-input-tables.js");

const REGENERATE = "run `node crates/farhelm-ui/js-tests/grid-input-tables.js --write` and review the diff";

/**
 * The first line where `want` and `got` differ, numbered from 1, or null.
 * The tables are thousands of lines; naming the line is what makes a
 * failure readable.
 */
function firstDifference(want, got) {
  const a = want.split("\n");
  const b = got.split("\n");
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    if (a[i] !== b[i]) return { line: i + 1, checkedIn: a[i], computed: b[i] };
  }
  return null;
}

/** Fail with the first differing line when `path`'s contents are not `computed`. */
function assertTable(path, computed, name) {
  const checkedIn = fs.readFileSync(path, "utf8");
  const diff = firstDifference(checkedIn, computed);
  assert.equal(diff, null, `${name} differs from what grid-input.js computes (${REGENERATE}): ${JSON.stringify(diff)}`);
}

test("every keydown sends the bytes the key table says, on every platform and cursor-key mode", () => {
  assertTable(tables.KEY_TABLE_PATH, tables.keyTable(), "grid-keys.jsonl");
});

test("every keydown is sent, left to text input, or kept local as the keydown table says", () => {
  assertTable(tables.KEYDOWN_TABLE_PATH, tables.keydownTable(), "keydown-actions.jsonl");
});

test("every Alt-click types the arrow keys the Alt-click table says", () => {
  assertTable(tables.MOVE_TABLE_PATH, tables.moveTable(), "move-to-cell.txt");
});

test("the tables cover the whole key space and both kinds of screen", () => {
  // A renderer change that quietly narrowed a sweep would leave the
  // comparisons above passing over less.
  const inputs = tables.tableInputs();
  const codes = new Set(inputs.map((input) => input.kc));
  for (let kc = 0; kc < 256; kc++) assert.ok(codes.has(kc), `key code ${kc} is missing`);
  const has = (kc, key, code) => inputs.some((input) => input.kc === kc && input.key === key && input.code === code);
  assert.ok(has(13, "Enter", "Enter"), "Enter, for Shift+Enter");
  assert.ok(has(0, "UIKeyInputUpArrow", ""), "the iOS arrow names");
  assert.ok(has(69, "Dead", "KeyE"), "a dead key under Alt");
  assert.ok(has(189, "_", "Minus") && has(50, "@", "Digit2"), "Ctrl with _ and @");
  const keydownRows = fs.readFileSync(tables.KEYDOWN_TABLE_PATH, "utf8").split("\n")
    .filter((line) => line && !line.startsWith("#") && !line.startsWith('["p"'));
  assert.ok(keydownRows.length > 1500, "the keydown table covers every key code with several keys each");
  const cases = tables.moveCases();
  assert.equal(cases.length, 1600);
  assert.ok(cases.some((c) => c.hasScrollback) && cases.some((c) => !c.hasScrollback), "both screens");
  assert.ok(cases.some((c) => c.wrapped.some(Boolean)), "screens with soft wraps");
});
