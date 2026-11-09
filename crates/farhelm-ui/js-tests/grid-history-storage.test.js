// Ownership and validation contracts for compact scrollback. Text-only checks
// cannot distinguish lazy bytes from a retained run/style tree, so these tests
// inspect the model's queue and records as well as its public row reads.
const assert = require("node:assert/strict");
const test = require("node:test");
const grid = require("../assets/grid-model.js");
const buffer = require("../assets/grid-buffer.js");
const { encode } = require("./grid-test-encoder.js");

const PLAIN = [0, -1, -1];

/** A readable ASCII row whose columns need no terminal-width inference. */
function row(text, style = PLAIN, link) {
  const run = [0, text.length, text, style];
  if (link !== undefined) run.push(link);
  return { w: false, r: text ? [run] : [] };
}

/** A fresh deferred attachment makes arrival and drain separately observable. */
function started(cap = 10, cols = 20) {
  const model = new grid.GridModel();
  model.apply({ t: "hello", cols, rows: 1, history_cap: cap });
  model.deferHistory = true;
  model.takeDamage();
  return model;
}

/** All test history crosses the production binary descriptor boundary. */
function append(model, first, rows, links, cols = model.cols) {
  const message = grid.decodeFrame(encode({ t: "history", first, cols, rows, links }));
  model.apply(message);
  return message;
}

/** Inspect the retained shape without decoding text or inventing GC timing promises. */
function assertCompact(record) {
  assert.deepEqual(Object.keys(record).sort(), ["body", "cols", "links", "token"]);
  assert.equal(record.body.constructor, ArrayBuffer);
  assert.ok(record.body.byteLength >= 3, "a body contains at least its wrap and run count");
  assert.deepEqual(Reflect.ownKeys(record.token), []);
  assert.equal(record.token === record, false);
}

/** Readable raw test data must never become a second production history path. */
test("history application requires the decoder's indexed input shape", () => {
  const model = started();
  assert.throws(() => model.apply({ t: "history", first: 0, cols: 20, rows: [row("raw")] }), grid.GridProtocolError);
  assert.equal(model.historyLength, 0);
  assert.equal(model.historyPending, false);
  assert.equal(model.historyExpected, null);
});

/** Arrival indexes rows only; decoding a styled catch-up must stay sliced. */
test("queued history owns bytes and a typed index without row or style trees", () => {
  const model = started(100, 20);
  const runs = Array.from({ length: 20 }, (_, col) => [col, 1, "x", [0, col, -1]]);
  const message = append(model, 40, Array.from({ length: 30 }, () => ({ w: true, r: runs })));
  const entry = model.historyQueue[0];
  assert.deepEqual(Object.keys(entry).sort(), ["bytes", "cols", "count", "first", "links", "offset", "offsets"]);
  assert.equal(entry.bytes, message.binaryRows.bytes);
  assert.ok(entry.offsets instanceof Uint32Array);
  assert.equal(entry.offsets.length, 31);
  assert.equal(entry.offsets[0], 0);
  assert.equal(entry.offsets[30], entry.bytes.length);
  assert.equal(entry.bytes.buffer.byteLength, entry.bytes.length);
  assert.equal(message.rows, undefined);
  assert.equal(model.historyLength, 0);
  assert.equal(model.historyIdentity(0), undefined);
  assert.equal(model.historyLine(0), undefined);
  assert.equal(model.styles.size, 0);
  assert.equal(model.screenBase, 70);
  model.drainHistory(() => true);
  assert.equal(model.historyLength, 1);
  assert.equal(entry.offset, 1);
  assertCompact(model.history.get(0));
  assert.equal(model.styles.size, 0);
});

/** A tiny input view must neither pin its large owner nor observe later mutations.
 * Buffer.slice has different ownership semantics from Uint8Array.slice, so
 * both input types need this premise before any row drains.
 */
test("Uint8Array and Buffer input views become exact independent queued allocations", () => {
  for (const kind of ["Uint8Array", "Buffer"]) {
    const model = started();
    const bytes = encode({ t: "history", first: 0, cols: 20, rows: [row("owned")] });
    const backing = kind === "Buffer" ? Buffer.alloc(bytes.length * 8) : new Uint8Array(bytes.length * 8);
    backing.set(bytes, 37);
    const view = backing.subarray(37, 37 + bytes.length);
    assert.ok(view.buffer.byteLength > view.byteLength, `${kind}: premise: a larger allocation owns the view`);
    const message = grid.decodeFrame(view);
    const owned = message.binaryRows.bytes;
    assert.equal(owned.constructor, Uint8Array);
    assert.equal(owned.byteOffset, 0);
    assert.equal(owned.buffer.byteLength, owned.byteLength);
    assert.notEqual(owned.buffer, view.buffer);
    model.apply(message);
    assert.equal(model.historyLength, 0, "premise: mutation happens before validation/drain");
    view.fill(0xff);
    assert.equal(grid.historyRowDescription(message, 0).r[0][2], "owned");
    model.drainHistory();
    const record = model.history.get(0);
    assertCompact(record);
    assert.notEqual(record.body, owned.buffer);
    assert.equal(record.body.byteLength, owned.byteLength - 4, "only this row's exact body survives its length prefix");
    assert.deepEqual(new Uint8Array(record.body), owned.subarray(4));
    assert.equal(model.historyLine(0).runs[0].text, "owned");
  }
});

/** Full validation belongs to drain even when no consumer ever reads history.
 * Earlier valid rows may apply, but the malformed row must never get a
 * record/token. These mutations keep the envelope and row lengths valid.
 */
test("retained malformed row semantics fail before a lazy read or row identity", () => {
  const cases = [
    ["foreground", row("x", [0, 999, -1])],
    ["background", row("x", [0, -1, 999])],
    ["underline", row("x", [0, -1, -1, 999])],
    ["link", row("x", PLAIN, 0)],
    ["ASCII width", { w: false, r: [[0, 2, "x", PLAIN]] }],
    ["non-ASCII width", { w: false, r: [[0, 3, "中", PLAIN]] }],
    ["empty text", { w: false, r: [[0, 1, "", PLAIN]] }],
    ["text budget", { w: false, r: [[0, 1, "\u0301".repeat(35), PLAIN]] }],
    ["zero width", { w: false, r: [[0, 0, "x", PLAIN]] }],
    ["ordering", { w: false, r: [[1, 1, "x", PLAIN], [0, 1, "y", PLAIN]] }],
    ["width", { w: false, r: [[19, 2, "xy", PLAIN]] }],
  ];
  for (const [name, bad] of cases) {
    const model = started();
    append(model, 0, [row("ok"), bad]);
    assert.equal(model.queuedRows, 2, `${name}: premise: arrival accepted both envelopes`);
    assert.equal(model.historyLength, 0);
    assert.throws(() => model.drainHistory(), grid.GridProtocolError, name);
    assert.equal(model.historyLength, 1, name);
    assert.ok(model.historyIdentity(0));
    assert.equal(model.history.get(1), undefined);
    assert.equal(model.historyIdentity(1), undefined);
    assert.equal(model.queuedRows, 1);
    assert.equal(model.styles.size, 0);
  }
});

/** Binary-only violations remain bounded within the indexed row body.
 * Arrival can index hostile content, but no retained row may escape the same
 * UTF-8, wrap, canonical-color, count and exact-consumption checks as screen.
 */
test("binary row validation is complete when history drains", () => {
  const cases = [
    ["wrap", (body) => { body[0] = 2; }],
    ["count", (body) => { new DataView(body.buffer, body.byteOffset, body.byteLength).setUint16(1, 21, true); }],
    ["UTF-8", (body) => { body[body.length - 1] = 0xff; }],
    ["canonical color", (body) => { new DataView(body.buffer, body.byteOffset, body.byteLength).setInt32(11, -1, true); }],
    ["trailing bytes", (body) => { new DataView(body.buffer, body.byteOffset, body.byteLength).setUint16(1, 0, true); }],
    ["optional read", (body) => { new DataView(body.buffer, body.byteOffset, body.byteLength).setUint16(7, 0xf000, true); }],
  ];
  for (const [name, mutate] of cases) {
    const model = started();
    const message = grid.decodeFrame(encode({ t: "history", first: 0, cols: 20, rows: [row("ok"), row("x", [0, 7, -1])] }));
    assert.deepEqual(grid.historyRowDescription(message, 1), row("x", [0, 7, -1]), `${name}: valid original row`);
    const rows = message.binaryRows;
    const body = rows.bytes.subarray(rows.offsets[1] + 4, rows.offsets[2]);
    mutate(body);
    model.apply(message);
    assert.equal(model.historyLength, 0);
    assert.equal(model.queuedRows, 2);
    assert.throws(() => model.drainHistory(), grid.GridProtocolError, name);
    assert.equal(model.historyLength, 1);
    assert.equal(model.historyIdentity(1), undefined);
  }
});

/** Materialized history is disposable; a caller cannot mutate future reads.
 * This also guards the screen-only interner against merely reading history.
 */
test("history reads are independent and leave only compact records behind", () => {
  const model = started();
  append(model, 5, [row("first", [1, 0x1000123, -1]), row("second")]);
  model.drainHistory();
  const token = model.historyIdentity(0);
  const first = model.historyLine(0);
  const again = model.historyLine(0);
  assert.deepEqual(first, again);
  assert.notEqual(first, again);
  assert.notEqual(first.runs, again.runs);
  assert.notEqual(first.runs[0], again.runs[0]);
  assert.notEqual(first.runs[0].style, again.runs[0].style);
  first.cols = 1;
  first.runs[0].text = "changed";
  first.runs[0].style.fg = 7;
  first.runs.length = 0;
  assert.deepEqual(model.historyLine(0), again);
  assert.equal(model.historyIdentity(0), token);
  assert.equal(model.styles.size, 0);
  for (let i = 0; i < model.historyLength; i++) assertCompact(model.history.get(i));
  assert.equal(model.historyLine(-1), undefined);
  assert.equal(model.historyLine(2), undefined);
  assert.equal(model.historyIdentity(2), undefined);
});

/** Offset advancement avoids copying a cap's worth of data per new row.
 * The one partial front entry keeps only its bounded original backing region;
 * completed entries disappear, while stored rows each own an exact body.
 */
test("partial drain and repeated partial trims preserve backing arrays and numbering", () => {
  const model = started(8);
  const message = append(model, 100, Array.from({ length: 8 }, (_, i) => row(String(100 + i))));
  const entry = model.historyQueue[0];
  const bytes = entry.bytes;
  const offsets = entry.offsets;
  model.drainHistory(() => true);
  assert.equal(entry.offset, 1);
  assert.equal(model.historyEnd, 101);
  for (let first = 108; first < 113; first++) {
    append(model, first, [row(String(first))]);
    assert.equal(model.queuedRows, 8);
    assert.equal(entry.bytes, bytes, "trimming cannot copy the remaining region");
    assert.equal(entry.offsets, offsets, "trimming cannot rebuild the remaining index");
    assert.equal(model.screenBase, first + 1);
  }
  assert.equal(entry.offset, 5);
  assert.equal(bytes, message.binaryRows.bytes);
  assert.ok(bytes.buffer.byteLength < grid.GRID_MAX_MESSAGE);
  assert.ok(offsets.buffer.byteLength < 0.6 * 1024 * 1024);
  assert.equal(model.historyQueue.filter((item) => !item.reset && item.offset > 0).length, 1);
  model.drainHistory(() => true);
  assert.equal(model.historyFirst, 105);
  assert.equal(model.historyEnd, 106);
  assert.equal(entry.offset, 6);
  assert.equal(model.takeDamage().historyReset, false);
  model.drainHistory();
  assert.equal(model.historyFirst, 105);
  assert.equal(model.historyEnd, 113);
  assert.equal(model.screenBase, 113);
  assert.deepEqual(Array.from({ length: 8 }, (_, i) => model.historyLine(i).runs[0].text), ["105", "106", "107", "108", "109", "110", "111", "112"]);
  assert.deepEqual(model.historyQueue, []);
  for (let i = 0; i < model.historyLength; i++) {
    assertCompact(model.history.get(i));
    assert.notEqual(model.history.get(i).body, bytes.buffer);
  }
});

/** Only genuinely referenced targets survive a drained message. Original high
 * wire indices stay intact without reserving a sparse per-line link array.
 */
test("stored links project used wire indices and discard unrelated targets", () => {
  const model = started();
  const links = Array.from({ length: 256 }, (_, i) => [`id${i}`, `https://example.com/${i}/${"u".repeat(100)}`]);
  const message = append(model, 0, [row("link", PLAIN, 255), row("plain"), row("")], links);
  assert.equal(message.links.length, 256, "premise: unused targets were declared");
  assert.equal(grid.historyRowDescription(message, 0).r[0][4], 255);
  model.drainHistory();
  const record = model.history.get(0);
  assert.ok(record.links instanceof Map);
  assert.deepEqual([...record.links.keys()], [255]);
  assert.equal(record.links.get(255), message.links[255]);
  assert.ok(Object.isFrozen(record.links.get(255)));
  assert.equal(model.historyLine(0).runs[0].link, message.links[255]);
  assert.equal(model.history.get(1).links, null);
  assert.equal(model.history.get(2).links, null);
  assertCompact(model.history.get(2));
  assert.equal(model.history.get(2).body.byteLength, 3, "blank rows still have exact bytes and identity");
  assert.notEqual(model.historyIdentity(1), model.historyIdentity(2));
  append(model, 3, [], links);
  assert.deepEqual(model.historyQueue, [{ first: 3, count: 0, offset: 0 }]);
  model.drainHistory();
  assert.equal(model.historyLength, 3);
  assert.equal(model.screenBase, 3);
});

/** Eviction clears slots immediately; a kept row never owns sibling bytes.
 * Reset retains displayed rows only until its established drain boundary,
 * then same-number rows must get different evidence tokens.
 */
test("cap eviction and reset release compact storage while tokens stay detached", () => {
  const model = started(2);
  append(model, 0, [row("old0"), row("old1")]);
  model.drainHistory();
  const evicted = model.history.get(0);
  const oldToken = model.historyIdentity(1);
  append(model, 2, [row("new2")]);
  model.drainHistory();
  assert.equal(model.historyFirst, 1);
  assert.equal(model.history.retained, 2);
  assert.ok(!model.history.lines.includes(evicted));
  assert.equal(model.history.lines[0], undefined);
  assert.notEqual(model.history.get(0).body, evicted.body);
  assert.equal(model.historyIdentity(0), oldToken);
  const oldStore = model.history.lines;
  append(model, 3, [row("queued")]);
  const queued = model.historyQueue[0];
  model.apply({ t: "history_reset" });
  assert.deepEqual(model.historyQueue, [{ reset: true }]);
  assert.ok(!model.historyQueue.includes(queued));
  assert.equal(model.historyIdentity(0), oldToken, "displayed history clears at drain, not acceptance");
  append(model, 1, [row("old1")]);
  model.drainHistory();
  assert.notEqual(model.history.lines, oldStore);
  assert.equal(model.historyFirst, 1);
  assert.notEqual(model.historyIdentity(0), oldToken);
  assert.deepEqual(Object.keys(oldToken), []);
  assert.equal(model.historyLength, 1);
  assert.equal(model.takeDamage().historyReset, true);
});

/** A description must retain explicit underline presence and BOM text while
 * readers still observe the ordinary normalized history shape and link data.
 */
test("history descriptions preserve wire distinctions and lazy text consumers", () => {
  const model = started(10, 6);
  const rows = [
    { w: true, r: [[0, 3, "abc", [0, -1, -1, -1]]] },
    { w: false, r: [[0, 1, "\ufeff", PLAIN], [1, 2, "中", PLAIN]] },
  ];
  const message = append(model, 50, rows);
  assert.deepEqual(grid.historyRowDescription(message, 0), rows[0]);
  assert.deepEqual(grid.historyRowDescription(message, 1), rows[1]);
  assert.equal(grid.historyRowDescription(message, 2), undefined);
  model.drainHistory();
  const source = buffer.modelSource(model);
  assert.equal(buffer.selectionText(source, { abs: 50, col: 0 }, { abs: 51, col: 3 }), "abc   \ufeff中");
  assert.equal(model.styles.size, 0);
  assertCompact(model.history.get(0));
  assertCompact(model.history.get(1));
});
