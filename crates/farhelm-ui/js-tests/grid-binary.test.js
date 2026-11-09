// Structural refusals at the binary boundary, apart from model semantics.
// Rust-produced fixtures provide the independent cross-language authority;
// these mutations specifically exercise JavaScript read bounds and exact
// counters, where silently accepting another representation is dangerous.
const assert = require("node:assert/strict");
const test = require("node:test");
const grid = require("../assets/grid-model.js");
const { encode } = require("./grid-test-encoder.js");

const PLAIN = [0, -1, -1];
const CURSOR = { x: 0, y: 0, visible: true, shape: "bar", blink: false };
const FRAME = { t: "frame", rev: 1, cols: 4, rows: 2, cursor: CURSOR };

/** A decoder result crosses the model's independent semantic validation too. */
function apply(model, value) {
  model.apply(grid.decodeFrame(encode(value)));
}

/** A valid attachment prefix makes refusal tests depend on their mutation alone. */
function started() {
  const model = new grid.GridModel();
  apply(model, { t: "hello", cols: 4, rows: 2, history_cap: 10 });
  apply(model, {
    t: "state",
    mouse: "off",
    focus: false,
    alt_screen: false,
    default_fg: null,
    default_bg: null,
    history_trimmed: false,
    exit: null,
  });
  return model;
}

/** Binary styles retain default underline presence, links, wide cells and combining text. */
test("binary rows and history retain the shared row semantics", () => {
  const model = started();
  const row = { w: true, r: [[0, 2, "中", [0xfff, 7, 0x1000042, -1], 0], [2, 1, "e\u0301", PLAIN]] };
  const links = [["fixture", "https://example.com/"]];
  apply(model, { t: "rows", rows: [[0, row]], links });
  assert.equal(model.screen[0].runs.length, 0, "rows remain buffered until their commit");
  apply(model, FRAME);
  apply(model, { t: "history", first: 7, cols: 4, rows: [row], links });
  const screen = model.screen[0];
  assert.equal(screen.wrapped, true);
  assert.equal(screen.runs[0].text, "中");
  assert.deepEqual(screen.runs[0].link, { id: "fixture", uri: "https://example.com/" });
  assert.deepEqual(screen.runs[0].style, { flags: 0xfff, fg: 7, bg: 0x1000042, ul: -1, key: "4095,7,16777282,-1" });
  assert.deepEqual(model.historyLine(0).runs, screen.runs);
  assert.equal(model.historyLine(0).cols, 4);
});

/** A byte-truncated row cannot consume the next message or become a partial frame. */
test("every truncated binary prefix and trailing bytes are refused", () => {
  const value = { t: "rows", rows: [[0, { w: false, r: [[0, 1, "x", PLAIN]] }]] };
  const bytes = encode(value);
  const decoded = grid.decodeFrame(bytes);
  assert.deepEqual(decoded, value, "premise: the complete message decodes");
  for (let end = 1; end < bytes.length; end++) {
    assert.throws(() => grid.decodeFrame(bytes.slice(0, end)), grid.GridProtocolError, `prefix ${end}`);
  }
  const trailing = new Uint8Array(bytes.length + 1);
  trailing.set(bytes);
  assert.throws(() => grid.decodeFrame(trailing), /trailing/);
  assert.throws(() => grid.decodeFrame(Uint8Array.of(0, 9)), /unknown binary/);
});

/** Presence metadata has one spelling for default foreground/background colors. */
test("binary counts, UTF-8, wrap flags and default-color spelling are checked", () => {
  const bytes = encode({ t: "rows", rows: [[0, { w: false, r: [[0, 1, "x", [0, 7, -1]]] }]] });
  const original = new DataView(bytes.buffer);
  assert.equal(bytes.length, 30);
  assert.equal(original.getUint32(4, true), 1);
  assert.equal(bytes[14], 0);
  assert.equal(original.getInt32(25, true), 7);
  const badCount = bytes.slice();
  new DataView(badCount.buffer).setUint32(4, 0xffffffff, true);
  assert.throws(() => grid.decodeFrame(badCount), /entry count/);
  const badWrap = bytes.slice();
  badWrap[14] = 2;
  assert.throws(() => grid.decodeFrame(badWrap), /wrap flag/);
  const badColor = bytes.slice();
  new DataView(badColor.buffer).setInt32(25, -1, true);
  assert.throws(() => grid.decodeFrame(badColor), /noncanonical/);
  const badText = bytes.slice();
  badText[29] = 0xff;
  assert.throws(() => grid.decodeFrame(badText), /not UTF-8/);
});

/** Counter rounding must not make distinct revisions or history lines compare equal. */
test("binary counters remain exact at the largest safe integer", () => {
  const bytes = encode({ ...FRAME, rev: Number.MAX_SAFE_INTEGER });
  assert.equal(bytes.length, 21);
  assert.equal(grid.decodeFrame(bytes).rev, Number.MAX_SAFE_INTEGER);
  const unsafe = bytes.slice();
  new DataView(unsafe.buffer).setUint32(6, 0x200000, true);
  assert.throws(() => grid.decodeFrame(unsafe), /safe integer/);
  const cursor = bytes.slice();
  cursor[20] |= 0x80;
  assert.throws(() => grid.decodeFrame(cursor), /cursor flags/);
});

/** A new protocol has one row format; accepting old JSON would hide a mismatched producer. */
test("row-bearing JSON is refused while controls stay JSON", () => {
  for (const value of [FRAME, { t: "rows", rows: [] }, { t: "history", first: 0, cols: 4, rows: [] }]) {
    assert.throws(() => grid.decodeFrame(JSON.stringify(value)), /not binary/);
  }
  const reset = { t: "history_reset" };
  assert.deepEqual(grid.decodeFrame(encode(reset)), reset);
});
