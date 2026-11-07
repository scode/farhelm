// Unit coverage for grid-buffer.js: the grid client's scrollback as one
// buffer of lines, and what selection copies, what a double or triple
// click selects, and which link is under a cell.
//
// These rules are the old terminal's (xterm.js's selection service and the
// web-links add-on, ported), kept because users notice when they differ:
// copied text that gains or loses newlines at soft wraps, a wrapped URL that
// stops being clickable, a word selection that stops at the wrong character.
// The rows are built by grid-model.js from wire messages, so the tests
// exercise the shapes the renderer really holds, including history lines
// written at another width.
const test = require("node:test");
const assert = require("node:assert/strict");
const grid = require("../assets/grid-model.js");
const buffer = require("../assets/grid-buffer.js");

const PLAIN = [0, -1, -1];

/** A wire row from `[col, text]` pairs (plain style), each ASCII text one run. */
function wire(pieces, wrapped = false) {
  return { w: wrapped, r: pieces.map(([col, text, link]) => (link === undefined ? [col, text.length, text, PLAIN] : [col, text.length, text, PLAIN, link])) };
}

/** A wire row from explicit runs `[col, width, text]`. */
function cells(runs, wrapped = false) {
  return { w: wrapped, r: runs.map(([col, width, text]) => [col, width, text, PLAIN]) };
}

/**
 * A model with `history` (wire rows at `historyCols`) above a screen of
 * `screen` wire rows at `cols`, history numbered from 100.
 */
function build({ cols = 10, screen = [], history = [], historyCols = cols, links } = {}) {
  const model = new grid.GridModel();
  model.apply({ t: "hello", cols, rows: Math.max(1, screen.length), history_cap: 1000 });
  if (screen.length) {
    model.apply({ t: "rows", rows: screen.map((row, y) => [y, row]), links });
  }
  model.apply({ t: "frame", rev: 1, cols, rows: Math.max(1, screen.length), cursor: { x: 0, y: 0, visible: true, shape: "block", blink: true } });
  if (history.length) model.apply({ t: "history", first: 100, cols: historyCols, rows: history, links });
  return model;
}

function at(abs, col) {
  return { abs, col };
}

test("a row's text has blanks for uncovered cells and each wide character once", () => {
  // Copying and URL matching both read rows this way; a wide character
  // counted twice would shift every column after it.
  const model = build({ cols: 8, screen: [cells([[1, 1, "a"], [2, 2, "中"], [4, 1, "é"], [6, 2, "\u{1f600}"]])] });
  const row = model.screen[0];
  assert.equal(buffer.rowText(row, 8), " a中é \u{1f600}");
  // A range starting on a wide character's second cell leaves it out; one
  // ending there takes it.
  assert.equal(buffer.rowText(row, 8, 3, 5), "é");
  assert.equal(buffer.rowText(row, 8, 1, 3), "a中");
});

test("a selection joins soft-wrapped rows without a newline and trims lines that end", () => {
  // A wrapped row continues on the next with nothing between them, and
  // keeps its trailing blanks (they are part of the logical line); a row
  // that ends is trimmed on the right.
  const model = build({
    cols: 6,
    screen: [wire([[0, "echo"]]), wire([[0, "abc"]], true), wire([[0, "defg"]]), wire([[0, "x"]])],
  });
  const source = buffer.modelSource(model);
  const base = model.screenBase;
  assert.equal(buffer.selectionText(source, at(base, 0), at(base + 3, 6)), "echo\nabc   defg\nx");
  // Reversed ends select the same text.
  assert.equal(buffer.selectionText(source, at(base + 3, 6), at(base, 0)), "echo\nabc   defg\nx");
  // A selection ending inside the wrapped row trims there like any end.
  assert.equal(buffer.selectionText(source, at(base + 1, 1), at(base + 1, 5)), "bc");
});

test("history lines keep their own width when a selection joins them", () => {
  // After a resize history keeps the width it was written at; a wrapped
  // 40-column line must keep its blanks to column 40, not to the screen's 10.
  const model = build({
    cols: 10,
    screen: [wire([[0, "prompt"]])],
    history: [wire([[0, "wide line"]], true), wire([[0, "rest"]])],
    historyCols: 40,
  });
  const source = buffer.modelSource(model);
  const text = buffer.selectionText(source, at(100, 0), at(model.screenBase, 10));
  assert.equal(text, `wide line${" ".repeat(31)}rest\nprompt`);
});

test("a selection spans history and screen and copies Windows line endings there", () => {
  const model = build({ cols: 5, screen: [wire([[0, "s1"]]), wire([[0, "s2"]])], history: [wire([[0, "h1"]]), wire([[0, "h2"]])] });
  const source = buffer.modelSource(model);
  assert.equal(model.screenBase, 102);
  assert.equal(buffer.selectionText(source, at(101, 0), at(103, 5)), "h2\ns1\ns2");
  assert.equal(buffer.selectionText(source, at(100, 0), at(101, 5), "\r\n"), "h1\r\nh2");
});

test("non-breaking spaces copy as spaces, and markup-looking text copies as written", () => {
  const model = build({ cols: 12, screen: [cells([[0, 1, "a"], [1, 1, "\u00a0"], [2, 1, "b"]]), wire([[0, "<b>x</b>&lt;"]])] });
  const source = buffer.modelSource(model);
  assert.equal(buffer.selectionText(source, at(model.screenBase, 0), at(model.screenBase + 1, 12)), "a b\n<b>x</b>&lt;");
});

test("lines that are not there contribute nothing", () => {
  // History that has not been applied yet (the queue) or was dropped at the
  // cap is skipped rather than copied as blank lines.
  const model = build({ cols: 5, screen: [wire([[0, "s"]])], history: [wire([[0, "h"]])] });
  const source = buffer.modelSource(model);
  assert.equal(buffer.selectionText(source, at(50, 0), at(model.screenBase, 5)), "h\ns");
});

test("a double click selects a word up to the word separators", () => {
  const model = build({ cols: 30, screen: [wire([[0, "cat (foo.txt) and  more"]])] });
  const source = buffer.modelSource(model);
  const base = model.screenBase;
  const word = (col) => {
    const range = buffer.wordRange(source, at(base, col));
    return buffer.selectionText(source, range.start, range.end);
  };
  assert.equal(word(1), "cat");
  // Separators bound words; "." is not one.
  assert.equal(word(6), "foo.txt");
  // A click on a separator starts the word there.
  assert.equal(word(4), "(foo.txt");
  // A click on a space selects the run of spaces (copied trimmed).
  assert.deepEqual(buffer.wordRange(source, at(base, 18)), { start: at(base, 17), end: at(base, 19) });
});

test("a word continues across a soft wrap, and wide characters count once", () => {
  const model = build({ cols: 4, screen: [wire([[0, "a ab"]], true), wire([[0, "cd e"]]), cells([[0, 2, "中"], [2, 2, "文"]])] });
  const source = buffer.modelSource(model);
  const base = model.screenBase;
  const range = buffer.wordRange(source, at(base, 3));
  assert.deepEqual(range, { start: at(base, 2), end: at(base + 1, 2) });
  assert.equal(buffer.selectionText(source, range.start, range.end), "abcd");
  // Either cell of a wide character finds it.
  const wide = buffer.wordRange(source, at(base + 2, 1));
  assert.deepEqual(wide, { start: at(base + 2, 0), end: at(base + 2, 4) });
});

test("a triple click selects the whole soft-wrapped line", () => {
  const model = build({ cols: 4, screen: [wire([[0, "x"]]), wire([[0, "abcd"]], true), wire([[0, "ef"]]), wire([[0, "y"]])] });
  const source = buffer.modelSource(model);
  const base = model.screenBase;
  assert.deepEqual(buffer.lineRange(source, at(base + 2, 0)), { start: at(base + 1, 0), end: at(base + 2, 4) });
  assert.equal(buffer.lineRange(source, at(9999, 0)), null);
});

test("an OSC 8 run is a link covering its runs, whatever its text says", () => {
  // The program's hyperlink: its target is data, shown and checked by the
  // caller; here only which cells it covers.
  const links = [["id1", "https://example.com/a"], ["id2", "javascript:alert(1)"]];
  const model = build({ cols: 30, screen: [wire([[0, "see "], [4, "here", 0], [8, " "], [9, "now", 0], [13, "evil", 1]])], links });
  const source = buffer.modelSource(model);
  const base = model.screenBase;
  const link = buffer.linkAt(source, at(base, 5));
  assert.equal(link.kind, "osc");
  assert.equal(link.uri, "https://example.com/a");
  // Each contiguous stretch is a link of its own, as old terminals make
  // them; the underline is that stretch.
  assert.deepEqual(link.ranges, [{ start: at(base, 4), end: at(base, 8) }]);
  assert.deepEqual(buffer.linkAt(source, at(base, 10)).ranges, [{ start: at(base, 9), end: at(base, 12) }]);
  assert.deepEqual(buffer.rowLinks(source, base).map((l) => [l.kind, l.uri]), [
    ["osc", "https://example.com/a"],
    ["osc", "https://example.com/a"],
    ["osc", "javascript:alert(1)"],
  ]);
  // Any scheme comes through as data; the opener refuses what is not http(s).
  assert.equal(buffer.linkAt(source, at(base, 14)).uri, "javascript:alert(1)");
  assert.equal(buffer.linkAt(source, at(base, 1)), null);
});

test("a URL printed as text is a link, with the web-links addon's matcher", () => {
  const model = build({ cols: 60, screen: [wire([[0, "open https://example.com/path?q=1. or ftp://x.org"]])] });
  const source = buffer.modelSource(model);
  const base = model.screenBase;
  const link = buffer.linkAt(source, at(base, 10));
  assert.equal(link.kind, "text");
  // Trailing punctuation is not part of it.
  assert.equal(link.uri, "https://example.com/path?q=1");
  assert.deepEqual(link.ranges, [{ start: at(base, 5), end: at(base, 33) }]);
  // Only http(s).
  assert.equal(buffer.linkAt(source, at(base, 40)), null);
  assert.equal(buffer.linkAt(source, at(base, 2)), null);
});

test("a text URL continues across soft wraps", () => {
  // The addon reads the wrapped line around the hovered row, so a long URL
  // the program wrapped is one link from any of its rows.
  const model = build({ cols: 10, screen: [wire([[0, "x https://"]], true), wire([[0, "example.co"]], true), wire([[0, "m/a end"]])] });
  const source = buffer.modelSource(model);
  const base = model.screenBase;
  for (const pos of [at(base, 3), at(base + 1, 4), at(base + 2, 1)]) {
    const link = buffer.linkAt(source, pos);
    assert.equal(link && link.uri, "https://example.com/a", JSON.stringify(pos));
    assert.deepEqual(link.ranges, [
      { start: at(base, 2), end: at(base, 10) },
      { start: at(base + 1, 0), end: at(base + 1, 10) },
      { start: at(base + 2, 0), end: at(base + 2, 3) },
    ]);
  }
  assert.equal(buffer.linkAt(source, at(base + 2, 5)), null);
});

test("a text URL that overlaps an OSC 8 run gives way to it", () => {
  // A program's hyperlink wins, and printed URLs it overlaps are dropped.
  const links = [["id", "https://target.example/"]];
  const model = build({ cols: 40, screen: [wire([[0, "https://shown.example/ "], [23, "x", 0]])], links });
  const source = buffer.modelSource(model);
  // The URL text itself has no OSC run, so it is a text link.
  assert.equal(buffer.linkAt(source, at(model.screenBase, 3)).kind, "text");
  const covered = build({ cols: 40, screen: [wire([[0, "https://shown.example/x", 0]])], links });
  const link = buffer.linkAt(buffer.modelSource(covered), at(covered.screenBase, 3));
  assert.equal(link.kind, "osc");
  assert.equal(link.uri, "https://target.example/");
});

test("a matched URL the parser would rewrite is not offered", () => {
  // The addon's second check: the text must start with what the URL parser
  // makes of its scheme and host (case aside): a percent-escaped host
  // name is decoded by the parser, so it is not offered.
  const model = build({ cols: 50, screen: [wire([[0, "http://EXAMPLE.com/ http://ex%61mple.com/x"]])] });
  const source = buffer.modelSource(model);
  assert.equal(buffer.linkAt(source, at(model.screenBase, 3)).uri, "http://EXAMPLE.com/");
  assert.equal(buffer.linkAt(source, at(model.screenBase, 25)), null);
});

test("the line source numbers history and screen and hides history on request", () => {
  const model = build({ cols: 5, screen: [wire([[0, "s"]])], history: [wire([[0, "h"]])] });
  const source = buffer.modelSource(model);
  assert.equal(source.first, 100);
  assert.equal(source.last, 101);
  assert.equal(buffer.rowText(source.line(100).row, 5), "h    ");
  const alternate = buffer.modelSource(model, false);
  assert.equal(alternate.first, 101);
  assert.equal(alternate.line(100), null);
});

// ## The link code on the grid's lines
//
// The hyperlink mismatch warning, printed-path detection and the file
// hover's redraw check are terminal-links.js and terminal-files.js code
// written against link lines (terminal-links.js, "Link lines"), which the
// grid offers directly (`linkLines`). These tests run that shipped code on
// the grid's lines, so a grid terminal is held to the same answers as an old
// terminal read through xterm.js's adapter.
const links = require("../assets/terminal-links.js");
const files = require("../assets/terminal-files.js");

/** The link lines the grid gives the link code, over a model's lines. */
function linesOver(model) {
  return buffer.linkLines(buffer.modelSource(model));
}

test("link lines read cells, wraps and trimmed text", () => {
  const model = build({ cols: 6, screen: [cells([[0, 1, "a"], [1, 2, "中"], [3, 1, "b"]], true), wire([[0, "cd"]])] });
  const lines = linesOver(model);
  const base = model.screenBase;
  const first = lines.line(base);
  assert.equal(first.length, 6);
  assert.equal(first.wrapped, false);
  assert.equal(lines.line(base + 1).wrapped, true);
  assert.deepEqual([0, 1, 2, 3, 4].map((x) => [first.cell(x).chars, first.cell(x).width, first.cell(x).tag]), [
    ["a", 1, 0], ["中", 2, 0], ["", 0, 0], ["b", 1, 0], ["", 1, 0],
  ]);
  assert.equal(first.cell(6), undefined);
  // Untouched cells read as spaces and trim away; a range starting on a
  // wide character's second cell reads a space there.
  assert.equal(first.text(0, 6, false), "a中b  ");
  assert.equal(first.text(0, 6, true), "a中b");
  assert.equal(first.text(2, 4, false), " b");
  assert.equal(lines.line(base + 9), undefined);
});

test("an OSC 8 link's text is joined across its rows for the mismatch warning", () => {
  // The warning compares the whole underlined text of one link with its
  // target; a lookalike split across rows must be judged whole. Rows belong
  // to one link when their entries share id and target, across messages.
  const target = "https://evil.example/login";
  const model = new grid.GridModel();
  model.apply({ t: "hello", cols: 12, rows: 3, history_cap: 0 });
  model.apply({ t: "rows", rows: [[0, wire([[0, "https://goo", 0]])]], links: [["x", target]] });
  model.apply({ t: "rows", rows: [[1, wire([[0, "d.example/", 0]])], [2, wire([[0, "plain"]])]], links: [["x", target]] });
  model.apply({ t: "frame", rev: 1, cols: 12, rows: 3, cursor: { x: 0, y: 0, visible: true, shape: "block", blink: true } });
  const range = { start: { abs: model.screenBase, col: 0 }, end: { abs: model.screenBase, col: 11 } };
  const hover = links.linkHoverText(linesOver(model), range, target);
  assert.deepEqual(hover, { kind: "whole", text: "https://good.example/" });
  assert.equal(links.linkTextMismatch(hover.text, target), true);
  // A link whose text is its target stays quiet.
  assert.equal(links.linkTextMismatch("https://evil.example/login", target), false);
  // Another link with the same id but another target is not joined.
  const other = new grid.GridModel();
  other.apply({ t: "hello", cols: 12, rows: 2, history_cap: 0 });
  other.apply({ t: "rows", rows: [[0, wire([[0, "https://goo", 0]])]], links: [["x", target]] });
  other.apply({ t: "rows", rows: [[1, wire([[0, "d.example/", 0]])]], links: [["x", "https://good.example/"]] });
  other.apply({ t: "frame", rev: 1, cols: 12, rows: 2, cursor: { x: 0, y: 0, visible: true, shape: "block", blink: true } });
  const alone = links.linkHoverText(linesOver(other), range, target);
  assert.deepEqual(alone, { kind: "whole", text: "https://goo" });
});

test("restarted anonymous hyperlinks underline and compare their whole adjacent text", () => {
  // Every id-less OSC start gets a new emulator id. The browser must recover
  // the honest address from those pieces rather than judge its tail alone.
  const target = "https://apnews.com/x";
  const model = build({ cols: 24, screen: [wire([[0, "https://apn", 0], [11, "ews.com/", 1], [19, "x", 2]])], links: [["0_alacritty", target], ["1_alacritty", target], ["2_alacritty", target]] });
  const source = buffer.modelSource(model);
  const base = model.screenBase;
  const found = buffer.rowLinks(source, base);
  assert.equal(found.length, 1);
  assert.deepEqual(found[0].ranges, [{ start: at(base, 0), end: at(base, 20) }]);
  const lines = linesOver(model);
  const row = lines.line(base);
  assert.equal(row.cell(0).tag, row.cell(19).tag);
  assert.deepEqual(links.linkHoverText(lines, found[0].ranges[0], target), { kind: "whole", text: target });
});

test("anonymous links join through three soft-wrapped rows and retain redraw evidence", () => {
  // The third row must invalidate a hover on the first, and the first must
  // contribute to a hover on the last; a two-row special case misses this.
  const target = "https://apnews.com/x";
  const model = build({ cols: 8, screen: [wire([[0, "https://", 0]], true), wire([[0, "apnews.c", 1]], true), wire([[0, "om/x", 2]])], links: [["0_alacritty", target], ["1_alacritty", target], ["2_alacritty", target]] });
  const source = buffer.modelSource(model);
  const base = model.screenBase;
  const lines = linesOver(model);
  for (let y = 0; y < 3; y++) {
    const found = buffer.linkAt(source, at(base + y, 1));
    assert.deepEqual(found.evidence, [base - 1, base, base + 1, base + 2, base + 3]);
    assert.deepEqual(links.linkHoverText(lines, found.ranges[0], target), { kind: "whole", text: target });
    assert.ok(buffer.answerLines(source, base + y).has(base + 2));
    assert.ok(buffer.answerLines(source, base + y).has(base));
  }
});

test("anonymous chains omit only the padding before a wrapped wide character", () => {
  // The sender omits this synthetic cell. Requiring the link to end at the
  // full width would split a chain exactly where a wide character wraps.
  const target = "https://example.com/";
  const model = build({ cols: 4, screen: [wire([[0, "abc", 0]], true), { w: false, r: [[0, 2, "中", PLAIN, 1]] }], links: [["0_alacritty", target], ["1_alacritty", target]] });
  const lines = linesOver(model);
  const base = model.screenBase;
  assert.equal(lines.line(base).cell(0).tag, lines.line(base + 1).cell(0).tag);
  const found = buffer.linkAt(buffer.modelSource(model), at(base + 1, 0));
  assert.deepEqual(links.linkHoverText(lines, found.ranges[0], target), { kind: "whole", text: "abc中" });
});

test("different targets, explicit ids and real gaps never become anonymous continuations", () => {
  // Preserve the distinction a program chose, and never hide intervening
  // text from the mismatch check by treating proximity as continuation.
  const target = "https://example.com/";
  for (const ids of [["0_alacritty", "named"], ["named", "0_alacritty"], ["0_alacritty", "x_alacritty"], ["0_alacritty", "_alacritty"]]) {
    const model = build({ screen: [wire([[0, "a", 0], [1, "b", 1]])], links: ids.map((id) => [id, target]) });
    assert.equal(buffer.rowLinks(buffer.modelSource(model), model.screenBase).length, 2);
  }
  const different = build({ screen: [wire([[0, "a", 0], [1, "b", 1]])], links: [["0_alacritty", target], ["1_alacritty", "https://other.example/"]] });
  assert.equal(buffer.rowLinks(buffer.modelSource(different), different.screenBase).length, 2);
  const gap = build({ screen: [wire([[0, "a", 0], [1, " "], [2, "b", 1]])], links: [["0_alacritty", target], ["1_alacritty", target]] });
  assert.equal(buffer.rowLinks(buffer.modelSource(gap), gap.screenBase).length, 2);
  for (const screen of [
    [wire([[0, "abcd", 0]]), wire([[0, "e", 1]])],
    [wire([[0, "abc", 0]], true), wire([[0, "e", 1]])],
    [wire([[0, "abcd", 0]], true), wire([[1, "e", 1]])],
  ]) {
    const model = build({ cols: 4, screen, links: [["0_alacritty", target], ["1_alacritty", target]] });
    const lines = linesOver(model);
    assert.notEqual(lines.line(model.screenBase).cell(0).tag, lines.line(model.screenBase + 1).cell(screen[1].r[0][0]).tag);
  }
});

test("a chain's later piece does not join an unwrapped row through its old generated id", () => {
  // Canonicalizing a piece to the first id changes its cursor-moved
  // continuation: that other row is now judged alone, as the plan accepts.
  const target = "https://apnews.com/x";
  const model = build({ cols: 12, screen: [wire([[0, "https://apn", 0], [11, "e", 1]]), wire([[0, "ws.com/x", 1]])], links: [["0_alacritty", target], ["1_alacritty", target]] });
  const source = buffer.modelSource(model);
  const found = buffer.linkAt(source, at(model.screenBase + 1, 0));
  assert.deepEqual(links.linkHoverText(linesOver(model), found.ranges[0], target), { kind: "whole", text: "ws.com/x" });
});

test("one-cell anonymous restarts resolve a long chain without repeated backward walks", () => {
  // Hostile output can restart OSC 8 at every cell. Exercise many runs and
  // rows while asserting identity and evidence, with no timing threshold.
  const cols = 256;
  const count = 100;
  const target = "https://example.com/";
  const entries = Array.from({ length: cols }, (_, i) => [`${i}_alacritty`, target]);
  const screen = Array.from({ length: count }, (_, y) => wire(Array.from({ length: cols }, (_, x) => [x, "a", x]), y < count - 1));
  const model = build({ cols, screen, links: entries });
  const source = buffer.modelSource(model);
  const found = buffer.rowLinks(source, model.screenBase + 50);
  assert.equal(found.length, 1);
  assert.deepEqual(found[0].ranges, [{ start: at(model.screenBase + 50, 0), end: at(model.screenBase + 50, cols) }]);
  assert.equal(found[0].evidence.length, count + 2);
});

test("today's printed-path detector finds paths in grid text, across soft wraps", () => {
  const model = build({ cols: 10, screen: [wire([[0, "see /usr/s"]], true), wire([[0, "hare/x.txt"]]), wire([[0, "v1.2.3 ok"]])] });
  const lines = linesOver(model);
  const base = model.screenBase;
  const found = files.pathLinks(lines, base + 1);
  assert.deepEqual(found, [{ path: "/usr/share/x.txt", range: { start: { abs: base, col: 4 }, end: { abs: base + 1, col: 10 } } }]);
  // A version number is not a path.
  assert.deepEqual(files.pathLinks(lines, base + 2), []);
  // The file hover's redraw check reads the same cells back.
  assert.equal(links.linkRangeText(lines, found[0].range), "/usr/share/x.txt");
});

test("hostile text stays data on the grid's lines", () => {
  // The scheme-bearing token is not a path, and markup-looking text is just
  // text: the grid's answer is the detector's own on the same characters.
  const text = "javascript:alert(document.domain) <b>a.md</b>";
  const model = build({ cols: 50, screen: [wire([[0, text]])] });
  const found = files.pathLinks(linesOver(model), model.screenBase).map((link) => link.path);
  assert.deepEqual(found, files.pathTokens(text).map((token) => token.path));
  // Exactly these: `a.md` inside the markup, and `/b` from its closing tag
  // (the detector's own reading); nothing from the scheme-bearing token.
  assert.deepEqual(found, ["a.md", "/b"]);
});

test("link lines present history at its own width and wrap across the history/screen boundary", () => {
  // A history line written at another width keeps it (an old terminal
  // reflows, the grid does not), and the first screen row continues the last history
  // line when that line wrapped: the link code must see both.
  const model = build({
    cols: 6,
    screen: [wire([[0, "tail"]]), wire([[0, "next"]])],
    history: [wire([[0, "a long line"]], true)],
    historyCols: 12,
  });
  const lines = linesOver(model);
  const history = lines.line(100);
  assert.equal(history.length, 12);
  assert.equal(history.text(0, 12, false), "a long line ");
  const first = lines.line(model.screenBase);
  assert.equal(first.length, 6);
  assert.equal(first.wrapped, true);
  assert.equal(lines.line(model.screenBase + 1).wrapped, false);
});

test("a copy leaves out the padding before a wide character that wrapped", () => {
  // A two-cell character that does not fit in the last column wraps whole,
  // leaving the cell before the edge empty. That cell is not copied; a blank
  // the program printed there is text and is copied.
  const screen = build({ cols: 4, screen: [wire([[0, "abc"]], true), cells([[0, 2, "\u4e2d"], [2, 1, "x"]])] });
  const lines = buffer.modelSource(screen);
  const base = screen.screenBase;
  assert.equal(buffer.selectionText(lines, at(base, 0), at(base + 1, 4)), "abc\u4e2dx");
  // A printed space in that cell stays.
  const printed = build({ cols: 4, screen: [wire([[0, "abc "]], true), cells([[0, 2, "\u4e2d"], [2, 1, "x"]])] });
  assert.equal(buffer.selectionText(buffer.modelSource(printed), at(printed.screenBase, 0), at(printed.screenBase + 1, 4)), "abc \u4e2dx");
  // The same across history and screen, and in a word search.
  const split = build({ cols: 4, screen: [cells([[0, 2, "\u4e2d"], [2, 1, "x"]])], history: [wire([[0, "abc"]], true)] });
  const source = buffer.modelSource(split);
  assert.equal(buffer.selectionText(source, at(100, 0), at(split.screenBase, 4)), "abc\u4e2dx");
  const word = buffer.wordRange(source, at(100, 1));
  assert.equal(buffer.selectionText(source, word.start, word.end), "abc\u4e2dx");
  // Two ASCII characters at the start of the next line are not a wide
  // character: the blank before them is a cell, and is copied.
  const ascii = build({ cols: 4, screen: [wire([[0, "abc"]], true), wire([[0, "xy"]])] });
  assert.equal(buffer.selectionText(buffer.modelSource(ascii), at(ascii.screenBase, 0), at(ascii.screenBase + 1, 4)), "abc xy");
});

test("a column selection copies the same columns of every line, without joining wraps", () => {
  // Column mode (Alt-drag off a Mac): each line cut to the span
  // and trimmed, one per line.
  const model = build({ cols: 8, screen: [wire([[0, "abcdefgh"]], true), wire([[0, "ijklmnop"]]), wire([[0, "qr"]])] });
  const source = buffer.modelSource(model);
  const base = model.screenBase;
  assert.equal(buffer.columnText(source, at(base, 2), at(base + 2, 5)), "cde\nklm\n");
  assert.equal(buffer.columnText(source, at(base, 5), at(base + 1, 2)), "cde\nklm");
  assert.equal(buffer.columnText(source, at(base, 3), at(base + 2, 3)), "");
  assert.equal(buffer.columnText(source, at(base, 0), at(base + 1, 2), "\r\n"), "ab\r\nij");
});

test("an OSC 8 link's evidence is the rows its hover text is joined from, and one beyond each end", () => {
  // The grid captures these rows when the hover starts and redoes the hover
  // when any of them is replaced: a change to a continuation row changes the
  // joined text the mismatch warning compares, even with the hovered row
  // untouched.
  const links = [["x", "https://a.example/"]];
  const model = build({ cols: 12, screen: [wire([[0, "https://goo", 0]]), wire([[0, "d.example/", 0]]), wire([[0, "plain"]])], links });
  const link = buffer.linkAt(buffer.modelSource(model), at(model.screenBase, 1));
  const base = model.screenBase;
  assert.deepEqual(link.evidence, [base - 1, base, base + 1, base + 2]);
  // Another link with the same id but another target is not joined.
  const other = build({ cols: 12, screen: [wire([[0, "https://goo", 0]]), wire([[0, "d.example/", 1]])], links: [["x", "https://a.example/"], ["x", "https://b.example/"]] });
  assert.deepEqual(buffer.linkAt(buffer.modelSource(other), at(other.screenBase, 1)).evidence, [other.screenBase - 1, other.screenBase, other.screenBase + 1]);
});

test("grid rows are immutable and replaced only when sent, with blank rows one shared object", () => {
  // The premise of the hover's identity check: a row object on a line stays
  // the same object until the supervisor sends that line again (or a resize
  // blanks it), so an unchanged object means unchanged text.
  // Three rows, the third never sent: it is the shared blank row. (A blank
  // row that is sent is a new object like any other.) Sharing one frozen
  // blank cannot make a changed line look current: a line holding it shows
  // nothing, so finding it there again means it still shows nothing.
  const model = new grid.GridModel();
  model.apply({ t: "hello", cols: 6, rows: 3, history_cap: 0 });
  model.apply({ t: "rows", rows: [[0, wire([[0, "keep"]])], [1, wire([[0, "send"]])]] });
  model.apply({ t: "frame", rev: 1, cols: 6, rows: 3, cursor: { x: 0, y: 0, visible: true, shape: "block", blink: true } });
  const [kept, sent, blank] = model.screen;
  assert.ok(Object.isFrozen(grid.BLANK_ROW));
  assert.equal(blank, grid.BLANK_ROW);
  model.apply({ t: "rows", rows: [[1, wire([[0, "send"]])]] });
  model.apply({ t: "frame", rev: 2, cols: 6, rows: 3, cursor: { x: 0, y: 0, visible: true, shape: "block", blink: true } });
  assert.equal(model.screen[0], kept, "a row not sent stays the same object");
  assert.notEqual(model.screen[1], sent, "a row sent again is a new object, even with the same text");
  assert.equal(model.screen[2], grid.BLANK_ROW);
  // A resize blanks rows not sent with it.
  model.apply({ t: "frame", rev: 3, cols: 7, rows: 3, cursor: { x: 0, y: 0, visible: true, shape: "block", blink: true } });
  assert.deepEqual(model.screen, [grid.BLANK_ROW, grid.BLANK_ROW, grid.BLANK_ROW]);
});

test("a double click on a very long wrapped line searches only near the click", () => {
  // A program can print one soft-wrapped line of thousands of rows; the
  // word search reads at most a window of characters around the click.
  const cols = 1000;
  const rows = [];
  for (let i = 0; i < 300; i++) rows.push(wire([[0, "x".repeat(cols)]], i < 299));
  const model = new grid.GridModel();
  model.apply({ t: "hello", cols, rows: 1, history_cap: 1000 });
  model.apply({ t: "frame", rev: 1, cols, rows: 1, cursor: { x: 0, y: 0, visible: true, shape: "block", blink: true } });
  model.apply({ t: "history", first: 0, cols, rows });
  const started = process.hrtime.bigint();
  const word = buffer.wordRange(buffer.modelSource(model), at(150, 5));
  const elapsedMs = Number(process.hrtime.bigint() - started) / 1e6;
  assert.ok(word.end.abs - word.start.abs <= 6, JSON.stringify(word));
  assert.ok(elapsedMs < 2000, `took ${elapsedMs} ms`);
});

test("the word search's bound counts cells, so wide history next to a narrow screen stays bounded", () => {
  // The bound is the cells read, not a row count from the clicked line's
  // width: a one-column screen line continuing 1,000-column history lines
  // must not read them all.
  const rows = [];
  for (let i = 0; i < 100; i++) rows.push(wire([[0, "x".repeat(1000)]], true));
  const model = new grid.GridModel();
  model.apply({ t: "hello", cols: 1, rows: 1, history_cap: 1000 });
  model.apply({ t: "rows", rows: [[0, wire([[0, "x"]])]] });
  model.apply({ t: "frame", rev: 1, cols: 1, rows: 1, cursor: { x: 0, y: 0, visible: true, shape: "block", blink: true } });
  model.apply({ t: "history", first: 0, cols: 1000, rows });
  const source = buffer.modelSource(model);
  const span = buffer.wrappedSpan(source, model.screenBase, Infinity, 2048);
  assert.equal(model.screenBase - span.first, 2, "two 1,000-cell lines fit in 2,048 cells");
  const word = buffer.wordRange(source, at(model.screenBase, 0));
  assert.ok(model.screenBase - word.start.abs <= 2, JSON.stringify(word));
});

test("a late link answer depends on its soft-wrapped line, the line before it and an OSC link's joined rows", () => {
  // Only a replaced row among these makes a pending answer stale; output on
  // any other row must not, or links never appear while a program writes.
  const model = build({ cols: 6, screen: [wire([[0, "top"]]), wire([[0, "abcdef"]], true), wire([[0, "gh"]]), wire([[0, "far"]])] });
  const source = buffer.modelSource(model);
  const base = model.screenBase;
  const sorted = (lines) => [...lines].sort((a, b) => a - b);
  // The printed-URL search also looks at the row after the wrapped line.
  assert.deepEqual(sorted(buffer.answerLines(source, base + 2)), [base, base + 1, base + 2, base + 3]);
  assert.deepEqual(sorted(buffer.answerLines(source, base + 3)), [base + 2, base + 3, base + 4]);
  const links = [["x", "https://a.example/"]];
  const osc = build({ cols: 12, screen: [wire([[0, "https://goo", 0]]), wire([[0, "d.example/", 0]]), wire([[0, "plain"]])], links });
  assert.deepEqual(sorted(buffer.answerLines(buffer.modelSource(osc), osc.screenBase)), [osc.screenBase - 1, osc.screenBase, osc.screenBase + 1, osc.screenBase + 2]);
});

test("a late answer also depends on every row a long printed URL spans", () => {
  // The URL search reads 2,048 characters either way, which on narrow rows
  // reaches past the path detector's 128 rows: a row replaced there can
  // change the URL a pending answer found.
  const url = "https://example.com/" + "a".repeat(1298);
  const rows = [];
  for (let at = 0; at < url.length; at += 10) rows.push(wire([[0, url.slice(at, at + 10)]], at + 10 < url.length));
  const model = build({ cols: 10, screen: rows });
  const lines = buffer.answerLines(buffer.modelSource(model), model.screenBase);
  assert.equal(rows.length, 132);
  for (let y = 0; y < rows.length; y++) assert.ok(lines.has(model.screenBase + y), `row ${y}`);
  assert.equal(buffer.linkAt(buffer.modelSource(model), at(model.screenBase, 3)).uri, url);
});
