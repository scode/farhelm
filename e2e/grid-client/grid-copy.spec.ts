// Copy across a wide character's wrap: the padding cell is not text.
//
// The emulator wraps a two-cell character that does not fit in the last
// column onto the next row and leaves the cell before the edge empty. That
// cell is padding, not text, so a copy across the wrap leaves it out (as an
// old terminal's xterm.js does); a grid terminal that copied it would put a
// space into commands, paths and URLs copied across such a wrap. This builds the grid rows the supervisor sends for such text
// and checks the copied text, on the screen and across history and screen,
// in a real browser's grid code. No xterm.js runs: the expected text is
// stated, not read from another terminal.
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import { join } from "node:path";
import { readFileSync } from "node:fs";

const ASSETS = join(__dirname, "../../crates/farhelm-ui/assets");
const TEST_ENCODER = readFileSync(join(__dirname, "../../crates/farhelm-ui/js-tests/grid-test-encoder.js"), "utf8");
const ORIGIN = "https://grid.test";

const PAGE = `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<script>${TEST_ENCODER}</script>
<script src="/assets/grid-model.js"></script>
<script src="/assets/grid-buffer.js"></script>
</head>
<body style="margin: 0"></body>
</html>`;

async function openPage(page: Page): Promise<void> {
  await page.route(`${ORIGIN}/**`, async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/") return route.fulfill({ contentType: "text/html", body: PAGE });
    const name = path.replace(/^\/assets\//, "");
    if (!/^[A-Za-z0-9.-]+$/.test(name)) return route.abort();
    return route.fulfill({ path: join(ASSETS, name) });
  });
  await page.goto(`${ORIGIN}/`);
}

/**
 * Why: a copy across a wide character's wrap is where a grid terminal's
 * text and the screen's cells differ, and the difference would land in what
 * users paste.
 * Spec: "abc" wrapping at four columns, then the wide character and "x" on
 * the next row, copies as "abc中x" from the screen and from history, with no
 * space for the empty last cell of the first row.
 */
test("a copy across a wide character's wrap leaves out the empty edge cell, on the screen and from history", async ({ page }) => {
  await openPage(page);
  const copies = await page.evaluate(async () => {
    const w = window as any;
    // The rows the supervisor sends for "abc中x" at four columns: "abc"
    // wrapping with its last cell empty, then the wide character and "x".
    const plain = [0, -1, -1];
    const wrapped = { w: true, r: [[0, 3, "abc", plain]] };
    const next = { w: false, r: [[0, 2, "中", plain], [2, 1, "x", plain]] };
    const cursor = { x: 0, y: 2, visible: true, shape: "block", blink: true };
    const screenModel = new w.farhelmGridModel.GridModel();
    screenModel.apply({ t: "hello", cols: 4, rows: 3, history_cap: 10 });
    screenModel.apply({ t: "rows", rows: [[0, wrapped], [1, next]] });
    screenModel.apply({ t: "frame", rev: 1, cols: 4, rows: 3, cursor });
    const historyModel = new w.farhelmGridModel.GridModel();
    historyModel.apply({ t: "hello", cols: 4, rows: 3, history_cap: 10 });
    historyModel.apply({ t: "rows", rows: [[0, next]] });
    historyModel.apply({ t: "frame", rev: 1, cols: 4, rows: 3, cursor });
    historyModel.apply(w.farhelmGridModel.decodeFrame(w.farhelmGridTestEncoder.encode({ t: "history", first: 0, cols: 4, rows: [wrapped] })));
    const buffer = w.farhelmGridBuffer;
    const screenBase = screenModel.screenBase;
    const historyBase = historyModel.screenBase;
    return {
      // The premise: the first row really is a wrapped row with an empty
      // last cell, in both models.
      screenWrapped: screenModel.screen[0].wrapped,
      historyLength: historyModel.historyLength,
      screen: buffer.selectionText(buffer.modelSource(screenModel), { abs: screenBase, col: 0 }, { abs: screenBase + 1, col: 4 }),
      history: buffer.selectionText(buffer.modelSource(historyModel), { abs: 0, col: 0 }, { abs: historyBase, col: 4 }),
    };
  });
  expect(copies.screenWrapped).toBe(true);
  expect(copies.historyLength).toBe(1);
  expect(copies.screen).toBe("abc中x");
  expect(copies.history).toBe("abc中x");
});
