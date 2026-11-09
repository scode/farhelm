// Unit coverage for grid-model.js: decoding grid stream messages and
// applying them to the screen/history model, driven by recorded message
// streams under fixtures/grid/.
//
// Two things matter here. The model must apply the format's semantics
// exactly (a frame commits its rows, a shift scrolls, a resize blanks,
// history is contiguous and capped), because the renderer draws whatever
// the model holds. And every message is peer input: a program in the
// terminal controls the cell text, colors and link targets, and under
// `--ssh` the supervisor is another machine. The hostile cases below pin
// that anything outside the format's bounds is refused as a protocol error
// and leaves the model untouched, and that markup-looking text is kept as
// plain data.
//
// The streams under fixtures/grid/ are hand-written, to show the model's
// behavior across whole attaches. The format's golden fixtures, one message
// each, live in `farhelm-proto` and are applied by
// grid-proto-fixtures.test.js, which holds this decoder to the Rust one.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const grid = require("../assets/grid-model.js");
const { encode } = require("./grid-test-encoder.js");

const FIXTURES = path.join(__dirname, "fixtures/grid");
const PLAIN = [0, -1, -1];

/** Readable fixture messages cross the same binary boundary as Rust output. */
function frames(name) {
  return fs
    .readFileSync(path.join(FIXTURES, name), "utf8")
    .split("\n")
    .filter((line) => line.trim() !== "")
    .map((line) => encode(line));
}

function applyAll(model, list) {
  for (const frame of list) model.apply(grid.decodeFrame(frame));
  return model;
}

function load(name) {
  return applyAll(new grid.GridModel(), frames(name));
}

/** The text of a screen or history row, with blanks for uncovered cells. */
function rowText(row) {
  let text = "";
  let col = 0;
  for (const run of row.runs) {
    text += " ".repeat(run.col - col) + run.text;
    col = run.col + run.width;
  }
  return text;
}

function screenText(model) {
  return model.screen.map(rowText);
}

function historyText(model) {
  const lines = [];
  for (let i = 0; i < model.historyLength; i++) lines.push(rowText(model.historyLine(i)));
  return lines;
}

/** A model that has had its hello, ready for a message under test. */
function started(cols = 10, rows = 3, historyCap = 5) {
  const model = new grid.GridModel();
  model.apply({ t: "hello", cols, rows, history_cap: historyCap });
  model.takeDamage();
  return model;
}

function frame(rev, extra = {}) {
  return {
    t: "frame",
    rev,
    cols: 10,
    rows: 3,
    cursor: { x: 0, y: 0, visible: true, shape: "block", blink: true },
    ...extra,
  };
}

function row(runs, wrapped = false) {
  return { w: wrapped, r: runs };
}

// ## The attach sequence

test("the attach sequence builds the screen, cursor, state and history", () => {
  // hello, state, a full screen, then history oldest first: the order the
  // format promises on every attach.
  const model = load("attach.jsonl");
  assert.equal(model.cols, 20);
  assert.equal(model.rows, 4);
  assert.deepEqual(screenText(model), ["$ echo", "hello 中文 web", "", ""]);
  assert.equal(model.screen[1].wrapped, true, "the soft-wrap flag is kept for selection");
  assert.deepEqual(model.cursor, { x: 2, y: 2, visible: true, shape: "block", blink: true });
  assert.equal(model.state.mouse, "off");
  assert.equal(model.rev, 1);
  // Four history lines arrived with a cap of three: the oldest is dropped
  // and the first kept line's absolute number moves with it.
  assert.deepEqual(historyText(model), ["", "three", "four"]);
  assert.equal(model.historyFirst, 41);
});

test("styles and links arrive validated and resolved", () => {
  const model = load("attach.jsonl");
  const [hello, wide, , web] = model.screen[1].runs;
  assert.deepEqual(hello.style, { flags: 1, fg: 2, bg: -1, ul: -1, key: "1,2,-1,-1" });
  assert.equal(wide.width, 2);
  assert.equal(wide.text, "中");
  assert.equal(web.style.ul, 0x1000000 + 0xff);
  assert.deepEqual(web.link, { id: "1", uri: "https://example.com/" });
  assert.equal(hello.link, null);
});

test("rows are invisible until the frame that commits them", () => {
  // A frame is never half-drawn: a renderer reading the model between a
  // `rows` message and its `frame` sees the previous screen.
  const model = started();
  model.apply({ t: "rows", rows: [[0, row([[0, 2, "hi", PLAIN]])]] });
  assert.deepEqual(screenText(model), ["", "", ""]);
  model.apply(frame(1));
  assert.deepEqual(screenText(model), ["hi", "", ""]);
});

test("a shift scrolls the screen up and the new rows land at the bottom", () => {
  const model = load("scroll.jsonl");
  assert.deepEqual(screenText(model), ["l4", "l5", "l6"]);
  assert.deepEqual(historyText(model), ["l1", "l2", "l3"]);
  assert.equal(model.historyFirst, 0);
  assert.equal(model.rev, 5, "revisions may skip, as when frames are coalesced");
  assert.deepEqual(model.cursor, { x: 2, y: 2, visible: false, shape: "underline", blink: true });
});

test("state carries the modes, redefined defaults and exit status", () => {
  const model = load("scroll.jsonl");
  assert.deepEqual(model.state, {
    mouse: "drag",
    focus: true,
    altScreen: true,
    defaultFg: 0x1000000 + 0x102030,
    defaultBg: null,
    historyTrimmed: true,
    exit: { code: 3 },
  });
  model.apply({ ...stateMessage(), exit: { signal: 9 } });
  assert.deepEqual(model.state.exit, { signal: 9 });
});

function stateMessage() {
  return {
    t: "state",
    mouse: "off",
    focus: false,
    alt_screen: false,
    default_fg: null,
    default_bg: null,
    history_trimmed: false,
    exit: null,
  };
}

test("a resize blanks every row the resizing frame does not update", () => {
  const model = started();
  model.apply({ t: "rows", rows: [[0, row([[0, 1, "a", PLAIN]])], [1, row([[0, 1, "b", PLAIN]])]] });
  model.apply(frame(1));
  model.apply({ t: "rows", rows: [[1, row([[0, 1, "c", PLAIN]])]] });
  model.apply(frame(2, { cols: 12, rows: 4, shift: 1 }));
  assert.deepEqual(screenText(model), ["", "c", "", ""]);
  assert.equal(model.takeDamage().all, true);
});

// ## Damage

test("damage follows rows through scrolls so a renderer can move them", () => {
  // Two scrolling frames between paints: the renderer moves its rows up by
  // the total and redraws only the rows whose content is new.
  const model = started(10, 5);
  model.apply({ t: "rows", rows: [[1, row([[0, 1, "x", PLAIN]])]] });
  model.apply(frame(1, { rows: 5 }));
  model.takeDamage();
  model.apply({ t: "rows", rows: [[1, row([[0, 1, "x", PLAIN]])]] });
  model.apply(frame(2, { rows: 5, shift: 1 }));
  model.apply({ t: "rows", rows: [[2, row([[0, 1, "y", PLAIN]])]] });
  model.apply(frame(3, { rows: 5, shift: 1 }));
  const damage = model.takeDamage();
  assert.equal(damage.all, false);
  assert.equal(damage.shift, 2);
  // Frame 2 wrote row 1 after its scroll, and frame 3's scroll moved it to
  // 0; frame 3 wrote row 2 after its own scroll; rows 3 and 4 were
  // uncovered by the two scrolls. Row 1 is the old row 3, which moved
  // without changing and needs no redraw.
  assert.deepEqual([...damage.rows].sort(), [0, 2, 3, 4]);
  assert.equal(model.takeDamage().shift, 0, "taking damage resets it");
});

test("a scroll of the whole screen or more redraws everything", () => {
  const model = started(10, 3);
  model.apply(frame(1, { shift: 2 }));
  model.apply(frame(2, { shift: 1 }));
  assert.equal(model.takeDamage().all, true);
});

// ## History

test("history must be contiguous, and a reset lets it start anywhere", () => {
  const model = started();
  model.apply({ t: "history", cols: 10, first: 7, rows: [row([[0, 1, "a", PLAIN]])] });
  assert.throws(() => model.apply({ t: "history", cols: 10, first: 9, rows: [] }), grid.GridProtocolError);
  model.apply({ t: "history_reset" });
  assert.equal(model.historyLength, 0);
  model.apply({ t: "history", cols: 10, first: 100, rows: [row([[0, 1, "b", PLAIN]])] });
  assert.equal(model.historyFirst, 100);
  assert.deepEqual(historyText(model), ["b"]);
});

test("history stays within its cap across many appends", () => {
  // The client keeps at most `history_cap` lines however much output
  // scrolls by; this is what bounds a long session's memory here.
  const model = started(10, 3, 1000);
  let first = 0;
  for (let batch = 0; batch < 50; batch++) {
    const rows = Array.from({ length: 100 }, (_, i) => row([[0, 6, String(first + i).padStart(6, "0"), PLAIN]]));
    model.apply({ t: "history", cols: 10, first, rows });
    first += 100;
  }
  assert.equal(model.historyLength, 1000);
  assert.equal(model.historyFirst, 4000);
  assert.equal(rowText(model.historyLine(0)), "004000");
  assert.equal(rowText(model.historyLine(999)), "004999");
});

test("evicted history rows are released at once, not at the next compaction", () => {
  // Rows hidden from readers but still referenced would keep their runs
  // alive: a cap's worth of them doubled history memory at cap turnover.
  // After every append, the backing store holds exactly the kept lines.
  const model = started(10, 3, 3000);
  let first = 0;
  for (let batch = 0; batch < 60; batch++) {
    const rows = Array.from({ length: 100 }, (_, i) => row([[0, 6, String(first + i).padStart(6, "0"), PLAIN]]));
    model.apply({ t: "history", cols: 10, first, rows });
    first += 100;
    assert.equal(model.history.retained, model.historyLength, `after ${first} lines`);
  }
  assert.equal(model.historyLength, 3000);
  assert.equal(rowText(model.historyLine(0)), "003000");
});

test("a history cap of zero keeps nothing but still checks contiguity", () => {
  const model = started(10, 3, 0);
  model.apply({ t: "history", cols: 10, first: 0, rows: [row([])] });
  assert.equal(model.historyLength, 0);
  assert.throws(() => model.apply({ t: "history", cols: 10, first: 5, rows: [] }), grid.GridProtocolError);
});

// ## Layout of runs

test("plain ASCII flows, any other run is one exact-width box", () => {
  // The format puts every non-ASCII cell in a run of its own, so a glyph
  // from a fallback font (whatever its advance) cannot shift the columns
  // after it.
  assert.equal(grid.layoutRun("hello", 5), null);
  assert.deepEqual(grid.layoutRun("中", 2), [["中", 2]]);
  assert.deepEqual(grid.layoutRun("─", 1), [["─", 1]]);
  assert.deepEqual(grid.layoutRun("e\u0301", 1), [["e\u0301", 1]], "a combining sequence is one cell");
  assert.deepEqual(grid.layoutRun("👨\u200d👩\u200d👧", 2), [["👨\u200d👩\u200d👧", 2]]);
  assert.deepEqual(grid.layoutRun("\t", 1), [["\t", 1]], "control characters never flow");
});

test("bidirectional formatting characters are dropped from what is drawn", () => {
  // They take no cell (an emulator keeps them with the cell before), and in
  // the page they would reorder the characters after them; the model keeps
  // them, only the drawn text loses them.
  assert.equal(grid.displayText("abc\u202edef"), "abcdef");
  assert.equal(grid.displayText("\u2066x\u2069\u200f\u061c"), "x");
  assert.equal(grid.displayText("plain"), "plain");
  const model = started();
  model.apply({ t: "rows", rows: [[0, row([[0, 2, "ab", PLAIN], [2, 1, "c\u202e", PLAIN], [3, 3, "def", PLAIN]])]] });
  model.apply(frame(1));
  assert.equal(model.screen[0].runs[1].text, "c\u202e");
  assert.deepEqual(model.cellAt(2, 0).text, "c");
  assert.equal(model.cellAt(3, 0).text, "d");
});

test("cellAt finds the character under the cursor, wide or narrow", () => {
  const model = load("attach.jsonl");
  assert.deepEqual(model.cellAt(1, 0), { text: " ", cells: 1, style: model.screen[0].runs[0].style, col: 1 });
  assert.deepEqual(model.cellAt(7, 1), { text: "中", cells: 2, style: model.screen[1].runs[1].style, col: 6 });
  assert.equal(model.cellAt(8, 1).text, "文");
  assert.deepEqual(model.cellAt(19, 3), { text: " ", cells: 1, style: null, col: 19 });
});

// ## Hostile and malformed input

test("markup-looking text is kept verbatim as data", () => {
  // The renderer inserts cell text with textContent; the model must not
  // transform it in a way that suggests it was ever interpreted.
  const model = started(40, 1);
  const text = "<img src=x onerror=alert(1)>&amp;";
  model.apply({ t: "rows", rows: [[0, row([[0, text.length, text, PLAIN]])]] });
  model.apply(frame(1, { cols: 40, rows: 1 }));
  assert.equal(model.screen[0].runs[0].text, text);
});

/** Assert `message` is refused and leaves the model exactly as it was. */
function refuses(model, message, why) {
  const before = JSON.stringify({ screen: model.screen, pending: [...model.pending], cursor: model.cursor, rev: model.rev, state: model.state });
  assert.throws(() => model.apply(message), grid.GridProtocolError, why);
  const after = JSON.stringify({ screen: model.screen, pending: [...model.pending], cursor: model.cursor, rev: model.rev, state: model.state });
  assert.equal(after, before, `${why}: a refused message must not change the model`);
}

test("frames that are not JSON objects with a type are refused", () => {
  for (const [data, why] of [
    [Buffer.from("not json"), "not JSON"],
    [Buffer.from([0xff, 0xfe, 0x7b]), "not UTF-8"],
    [Buffer.from("[1,2]"), "an array"],
    [Buffer.from('{"rows":[]}'), "no type"],
    [Buffer.from('{"t":5}'), "a non-string type"],
    [Buffer.alloc(grid.GRID_MAX_MESSAGE + 1, 0x20), "over 1 MiB"],
  ]) {
    assert.throws(() => grid.decodeFrame(data), grid.GridProtocolError, why);
  }
});

test("the first message must be the only hello, and unknown types are refused", () => {
  assert.throws(() => new grid.GridModel().apply(frame(1)), grid.GridProtocolError);
  const model = started();
  refuses(model, { t: "hello", cols: 10, rows: 3, history_cap: 5 }, "second hello");
  refuses(model, { t: "bell" }, "unknown type");
  for (const hello of [
    { t: "hello", cols: 0, rows: 3, history_cap: 0 },
    { t: "hello", cols: 1001, rows: 3, history_cap: 0 },
    { t: "hello", cols: 10, rows: 3.5, history_cap: 0 },
    { t: "hello", cols: 10, rows: 3, history_cap: 100001 },
    { t: "hello", cols: "10", rows: 3, history_cap: 0 },
  ]) {
    assert.throws(() => new grid.GridModel().apply(hello), grid.GridProtocolError, JSON.stringify(hello));
  }
});

test("unknown fields are ignored", () => {
  const model = started();
  model.apply({ t: "rows", rows: [[0, { w: false, r: [[0, 1, "a", PLAIN]], future: 1 }]], extra: true });
  model.apply(frame(1, { extra: "x" }));
  assert.deepEqual(screenText(model), ["a", "", ""]);
});

test("runs out of range, out of order or overlapping are refused", () => {
  const model = started();
  const cases = [
    [[[-1, 1, "a", PLAIN]], "negative column"],
    [[[0, 0, "", PLAIN]], "zero width"],
    [[[0, 1.5, "a", PLAIN]], "fractional width"],
    [[[999, 2, "ab", PLAIN]], "past the largest grid"],
    [[[2, 2, "ab", PLAIN], [3, 1, "c", PLAIN]], "overlapping runs"],
    [[[4, 1, "a", PLAIN], [2, 1, "b", PLAIN]], "runs out of order"],
    [[[0, 1, 7, PLAIN]], "non-string text"],
    [[[0, 1, "a"]], "missing style"],
    [[[0, 1, "a", PLAIN, 0, 0]], "too many fields"],
    [[[0, 1, "a", PLAIN, 0]], "link index with no links table"],
  ];
  for (const [runs, why] of cases) refuses(model, { t: "rows", rows: [[0, row(runs)]] }, why);
  refuses(model, { t: "rows", rows: [[0, { r: [] }]] }, "missing wrap flag");
  refuses(model, { t: "rows", rows: [[1000, row([])]] }, "row index past the largest grid");
  refuses(model, { t: "rows", rows: [[0]] }, "rows entry without a row");
});

test("rows that do not fit the committing frame are refused with the whole frame", () => {
  // Bounds that depend on the frame are checked at commit, before the
  // screen changes, so a refused frame leaves the previous one standing.
  const model = started();
  model.apply({ t: "rows", rows: [[0, row([[0, 1, "a", PLAIN]])]] });
  model.apply(frame(1));
  model.apply({ t: "rows", rows: [[0, row([[8, 4, "wide", PLAIN]])]] });
  refuses(model, frame(2), "a run past the last column");
  const other = started();
  other.apply({ t: "rows", rows: [[5, row([])]] });
  refuses(other, frame(1), "a row below the last row");
});

test("huge runs past the per-cell text budget are refused", () => {
  const model = started();
  // 68 bytes per cell: a character and 16 combining characters of 4 bytes.
  model.apply({ t: "rows", rows: [[0, row([[0, 1, "a" + "\u0301".repeat(33), PLAIN]])]] });
  refuses(model, { t: "rows", rows: [[0, row([[0, 1, "x".repeat(69), PLAIN]])]] }, "69 bytes in one cell");
  refuses(model, { t: "rows", rows: [[0, row([[0, 2, "😀".repeat(35), PLAIN]])]] }, "140 bytes in two cells");
});

test("bogus colors and style shapes are refused", () => {
  const model = started();
  for (const style of [
    [0, 256, -1],
    [0, -2, -1],
    [0, 0x2000000, -1],
    [0, 0xffffff, -1],
    [0, 1.5, -1],
    [0, "red", -1],
    [0, "#fff", -1],
    [0, -1, -1, 300],
    [-1, -1, -1],
    ["1", -1, -1],
    [0, -1],
    [0, -1, -1, -1, -1],
    { flags: 0 },
  ]) {
    refuses(model, { t: "rows", rows: [[0, row([[0, 1, "a", style]])]] }, JSON.stringify(style));
  }
});

test("runs with the same style share one style object", () => {
  // History holds up to its cap whether or not it is drawn; sharing the
  // handful of styles most lines use keeps that memory down.
  const model = started(10, 3, 10);
  model.apply({ t: "history", cols: 10, first: 0, rows: [row([[0, 1, "a", [1, 2, -1]], [2, 1, "b", [1, 2, -1]]]), row([[0, 1, "c", [1, 2, -1]]])] });
  const styles = [model.historyLine(0).runs[0].style, model.historyLine(0).runs[1].style, model.historyLine(1).runs[0].style];
  assert.equal(styles[0], styles[1]);
  assert.equal(styles[0], styles[2]);
});

test("style bits cannot overlap binary presence metadata", () => {
  // A new flag must earn a protocol bump; masking it would confuse it
  // with a field-presence bit in the encoder's shared metadata word.
  assert.throws(() => grid.decodeStyle([0x1000 | 1, -1, -1]), grid.GridProtocolError);
  assert.throws(() => encode({ t: "rows", rows: [[0, row([[0, 1, "x", [0x1000, -1, -1]]])]] }), /style bits/);
});

test("link tables past their bounds are refused", () => {
  const model = started();
  const linked = (links) => ({ t: "rows", rows: [[0, row([[0, 1, "a", PLAIN, 0]])]], links });
  model.apply(linked([["i", "https://example.com/"]]));
  refuses(model, linked(Array.from({ length: 257 }, (_, i) => [String(i), "https://e.com/"])), "257 links");
  refuses(model, linked([["x".repeat(251), "https://e.com/"]]), "a 251-byte id");
  refuses(model, linked([["i", "https://e.com/" + "a".repeat(2083)]]), "a 2098-byte target");
  refuses(model, linked([["i"]]), "a link without a target");
  refuses(model, linked("https://e.com/"), "a links field that is not a table");
  refuses(model, { t: "rows", rows: [[0, row([[0, 1, "a", PLAIN, 1]])]], links: [["i", "u"]] }, "a link index past the table");
});

test("frames with bad revisions, sizes, shifts or cursors are refused", () => {
  const model = started();
  model.apply(frame(4));
  refuses(model, frame(4), "a revision that did not increase");
  refuses(model, frame(3), "a revision that went back");
  refuses(model, frame(5, { cols: 0 }), "zero columns");
  refuses(model, frame(5, { rows: 1001 }), "too many rows");
  refuses(model, frame(5, { shift: 4 }), "a shift past the screen");
  refuses(model, frame(5, { shift: -1 }), "a negative shift");
  refuses(model, frame(5, { cursor: { x: 0, y: 0, visible: true, shape: "beam", blink: false } }), "an unknown cursor shape");
  refuses(model, frame(5, { cursor: { x: -1, y: 0, visible: true, shape: "bar", blink: false } }), "a negative cursor column");
  refuses(model, frame(5, { cursor: null }), "no cursor");
});

test("a cursor must lie on the committing frame's screen", () => {
  // The format puts the cursor on the screen (`x < cols`, `y < rows`), and
  // `farhelm-proto` refuses one past either edge. This decoder used to
  // clamp it instead, so the two sides disagreed about what a stream may
  // carry; the golden fixtures pin the one-past case for both. Spec: the
  // last column and row are accepted as sent, one past either is refused,
  // and the bound is the committing frame's size, not the previous one's,
  // whether the frame grows the screen or shrinks it.
  const model = started();
  const at = (x, y) => ({ x, y, visible: true, shape: "bar", blink: false });
  model.apply(frame(1, { cursor: at(9, 2) }));
  assert.deepEqual(model.cursor, at(9, 2));
  refuses(model, frame(2, { cursor: at(10, 0) }), "a cursor one past the last column");
  refuses(model, frame(2, { cursor: at(0, 3) }), "a cursor one past the last row");
  model.apply(frame(2, { cols: 11, rows: 4, cursor: at(10, 3) }));
  assert.deepEqual(model.cursor, at(10, 3), "a growing frame's cursor is checked against its new size");
  refuses(model, frame(3, { cols: 5, rows: 2, cursor: at(9, 2) }), "a cursor that fits only the screen a shrinking frame leaves");
});

test("bad state messages are refused", () => {
  const model = started();
  for (const change of [
    { mouse: "all" },
    { focus: 1 },
    { alt_screen: null },
    { default_fg: 5 },
    { default_bg: "#000000" },
    { history_trimmed: undefined },
    { exit: { code: "1" } },
    { exit: { signal: 0 } },
    { exit: 3 },
  ]) {
    refuses(model, { ...stateMessage(), ...change }, JSON.stringify(change));
  }
});

test("bad history messages are refused", () => {
  const model = started();
  refuses(model, { t: "history", cols: 10, first: -1, rows: [] }, "a negative line number");
  refuses(model, { t: "history", cols: 10, first: 2 ** 60, rows: [] }, "an unsafe line number");
  refuses(model, { t: "history", cols: 10, first: 0, rows: [{ w: false, r: [[0, 1, "a", [0, 999, -1]]] }] }, "a bogus color in history");
});

test("a non-ASCII run holds exactly one cell, and an ASCII run one character per cell", () => {
  // Only the emulator knows how characters group into cells; the format
  // never asks the browser to split a run, so a run that would need it is
  // refused rather than guessed at.
  const model = started();
  refuses(model, { t: "rows", rows: [[0, row([[0, 3, "\u2500\u253c\u2500", PLAIN]])]] }, "three box-drawing cells in one run");
  refuses(model, { t: "rows", rows: [[0, row([[0, 4, "\u4e2d\u6587", PLAIN]])]] }, "two wide characters in one run");
  refuses(model, { t: "rows", rows: [[0, row([[0, 3, "ab", PLAIN]])]] }, "ASCII whose width is not its length");
  refuses(model, { t: "rows", rows: [[0, row([[0, 1, "", PLAIN]])]] }, "an empty run");
  model.apply({ t: "rows", rows: [[0, row([[0, 2, "\u4e2d", PLAIN], [2, 1, "e\u0301", PLAIN], [3, 1, "\t", PLAIN]])]] });
});

test("history lines keep the width their message says they were written at", () => {
  // Joining soft-wrapped history lines for selection needs each line's own
  // width, which after a resize is not the screen's.
  const model = started(10, 3, 10);
  model.apply({ t: "history", cols: 40, first: 0, rows: [row([[30, 5, "right", PLAIN]], true)] });
  assert.equal(model.historyLine(0).cols, 40);
  for (const cols of [undefined, 0, 1001, 2.5, "40"]) {
    refuses(model, { t: "history", cols, first: 1, rows: [] }, `history width ${cols}`);
  }
  refuses(model, { t: "history", cols: 20, first: 1, rows: [row([[18, 3, "abc", PLAIN]])] }, "a run past its line's width");
});

test("history line numbers stay within JavaScript's safe integers", () => {
  const max = Number.MAX_SAFE_INTEGER;
  const model = started();
  refuses(model, { t: "history", cols: 10, first: max - 1, rows: [row([]), row([]), row([])] }, "a last line past the bound");
  model.apply({ t: "history", cols: 10, first: max, rows: [] });
  const other = started();
  other.apply({ t: "history", cols: 10, first: max - 1, rows: [row([]), row([])] });
  assert.equal(other.historyLength, 2);
});

test("a screen update is contiguous: nothing may come between its rows and its frame", () => {
  const model = started();
  model.apply({ t: "rows", rows: [[0, row([[0, 1, "a", PLAIN]])]] });
  refuses(model, stateMessage(), "a state inside an update");
  refuses(model, { t: "history", cols: 10, first: 0, rows: [] }, "history inside an update");
  refuses(model, { t: "clipboard", target: "clipboard", text: "x", last: true }, "a clipboard part inside an update");
  model.apply({ t: "rows", rows: [[1, row([[0, 1, "b", PLAIN]])]] });
  model.apply(frame(1));
  model.apply(stateMessage());
  assert.deepEqual(screenText(model), ["a", "b", ""]);
});

test("a clipboard write arrives in contiguous parts and is handed out whole", () => {
  // Programs copy with OSC 52; a long copy is split, and only the complete
  // text may reach the clipboard.
  const model = started();
  model.apply({ t: "clipboard", target: "clipboard", text: "first ", last: false });
  assert.deepEqual(model.takeClipboard(), []);
  refuses(model, stateMessage(), "another message between parts");
  refuses(model, { t: "clipboard", target: "primary", text: "x", last: true }, "a part for another target");
  model.apply({ t: "clipboard", target: "clipboard", text: "second", last: true });
  model.apply({ t: "clipboard", target: "primary", text: "sel", last: true });
  assert.deepEqual(model.takeClipboard(), [
    { target: "clipboard", text: "first second" },
    { target: "primary", text: "sel" },
  ]);
  assert.deepEqual(model.takeClipboard(), []);
  model.apply(stateMessage());
});

test("clipboard parts and writes are bounded", () => {
  const model = started();
  for (const part of [
    { t: "clipboard", target: "system", text: "x", last: true },
    { t: "clipboard", target: "clipboard", text: 5, last: true },
    { t: "clipboard", target: "clipboard", text: "x" },
    { t: "clipboard", target: "clipboard", text: "x".repeat(512 * 1024 + 1), last: true },
  ]) {
    refuses(model, part, JSON.stringify(part).slice(0, 60));
  }
  // 8 MiB in sixteen full parts is the most a write may hold.
  const part = "x".repeat(512 * 1024);
  const full = started();
  for (let i = 0; i < 15; i++) full.apply({ t: "clipboard", target: "clipboard", text: part, last: false });
  full.apply({ t: "clipboard", target: "clipboard", text: part, last: true });
  assert.equal(full.takeClipboard()[0].text.length, 8 * 1024 * 1024);
  const over = started();
  for (let i = 0; i < 16; i++) over.apply({ t: "clipboard", target: "clipboard", text: part, last: false });
  assert.throws(() => over.apply({ t: "clipboard", target: "clipboard", text: "x", last: true }), grid.GridProtocolError);
});

test("a clipboard write held open with endless small or empty parts holds memory by its bytes", () => {
  // Every part is within the format's bounds, so only the accumulator can
  // stop a peer from growing the page with parts that never finish a write.
  const model = started();
  for (let i = 0; i < 100000; i++) model.apply({ t: "clipboard", target: "clipboard", text: "", last: false });
  assert.equal(model.clipboardHeldStrings, 0, "empty parts hold nothing");
  model.apply({ t: "clipboard", target: "clipboard", text: "", last: true });
  assert.deepEqual(model.takeClipboard(), [{ target: "clipboard", text: "" }], "an empty last part still completes the write");
  for (let i = 0; i < 100000; i++) model.apply({ t: "clipboard", target: "clipboard", text: "x", last: false });
  // 100,000 one-byte parts: one gathered 64 KiB chunk and the rest pending.
  assert.equal(model.clipboardHeldStrings, 1 + (100000 - 65536));
  model.apply({ t: "clipboard", target: "clipboard", text: "y", last: true });
  assert.equal(model.takeClipboard()[0].text, "x".repeat(100000) + "y");
});

// ## History applied in slices (the history queue)

/** `n` one-run history rows reading `prefix` and their line number. */
function numbered(first, n, prefix = "") {
  return Array.from({ length: n }, (_, i) => row([[0, String(first + i).length + prefix.length, prefix + (first + i), PLAIN]]));
}

test("deferred history is queued on arrival and applied in slices", () => {
  // An attach's whole history must not be decoded in one task (it held the
  // page for seconds); the renderer drains the queue in time slices. Until
  // a row is applied it is not in history, and the screen's line numbers
  // already count it, so anything anchored to them does not move as the
  // queue drains.
  const model = started(10, 3, 100);
  model.deferHistory = true;
  model.apply({ t: "history", cols: 10, first: 0, rows: numbered(0, 10) });
  model.apply({ t: "history", cols: 10, first: 10, rows: numbered(10, 10) });
  assert.equal(model.historyLength, 0);
  assert.equal(model.historyPending, true);
  assert.equal(model.screenBase, 20);
  // Contiguity is checked on arrival, not when applied.
  assert.throws(() => model.apply({ t: "history", cols: 10, first: 25, rows: [] }), grid.GridProtocolError);
  let rows = 0;
  assert.equal(model.drainHistory(() => ++rows >= 7), true);
  assert.equal(model.historyLength, 7);
  assert.equal(model.screenBase, 20);
  assert.equal(model.historyEnd, 7);
  assert.equal(model.drainHistory(), false);
  assert.equal(model.historyPending, false);
  assert.deepEqual(historyText(model), Array.from({ length: 20 }, (_, i) => String(i)));
  assert.equal(model.historyFirst, 0);
  assert.equal(model.takeDamage().history, true);
});

test("a queue longer than the cap drops its oldest rows unread", () => {
  // However far applying falls behind, the queue holds at most a cap's
  // worth: older rows would only be applied to be dropped again. A dropped
  // row is never decoded, so a bad one there is not an error.
  const model = started(10, 3, 5);
  model.deferHistory = true;
  model.apply({ t: "history", cols: 10, first: 0, rows: numbered(0, 3) });
  model.drainHistory();
  model.apply({ t: "history", cols: 10, first: 3, rows: [{ w: false, r: [[0, 1, "x", [0, 999, -1]]] }, ...numbered(4, 3)] });
  model.apply({ t: "history", cols: 10, first: 7, rows: numbered(7, 4) });
  assert.equal(model.queuedRows, 5);
  model.drainHistory();
  assert.deepEqual(historyText(model), ["6", "7", "8", "9", "10"]);
  assert.equal(model.historyFirst, 6);
  assert.equal(model.screenBase, 11);
  // Trimming is not a reset: the kept lines keep their numbers, so a view
  // or selection anchored to them stays (only `history` damage).
  const damage = model.takeDamage();
  assert.equal(damage.history, true);
  assert.equal(damage.historyReset, false);
});

test("a history reset discards what is queued before it", () => {
  const model = started(10, 3, 100);
  model.deferHistory = true;
  model.apply({ t: "history", cols: 10, first: 0, rows: numbered(0, 5) });
  model.apply({ t: "history_reset" });
  model.apply({ t: "history", cols: 10, first: 40, rows: numbered(40, 2) });
  model.drainHistory();
  assert.deepEqual(historyText(model), ["40", "41"]);
  assert.equal(model.historyFirst, 40);
  assert.equal(model.takeDamage().historyReset, true);
});

test("a bad history row is reported when it is applied", () => {
  const model = started(10, 3, 100);
  model.deferHistory = true;
  model.apply({ t: "history", cols: 10, first: 0, rows: [...numbered(0, 2), row([[8, 3, "abc", PLAIN]])] });
  assert.throws(() => model.drainHistory(), grid.GridProtocolError);
});

test("a real reset still voids anchors when the queue it is in is trimmed", () => {
  // A supervisor's history_reset followed by more than a cap of history
  // before it is applied: the trim replaces the queue's reset entry, which
  // must stay a real reset (line numbers start over).
  const model = started(10, 3, 10);
  model.deferHistory = true;
  model.apply({ t: "history", cols: 10, first: 0, rows: numbered(0, 3) });
  model.drainHistory();
  model.takeDamage();
  model.apply({ t: "history_reset" });
  model.apply({ t: "history", cols: 10, first: 50, rows: numbered(50, 11) });
  model.drainHistory();
  assert.equal(model.takeDamage().historyReset, true);
  assert.equal(model.historyFirst, 51);
});
