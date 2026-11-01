// Copy parity with today's terminal where the two hold text differently.
//
// The emulator wraps a two-cell character that does not fit in the last
// column onto the next row and leaves the cell before the edge empty.
// xterm.js does the same and leaves that padding out of a copy; a grid
// terminal that copied it would put a space into commands, paths and URLs
// copied across such a wrap. This writes the same text into the shipped
// xterm.js and builds the grid rows the supervisor would send for it, then
// compares the two copies, on the screen and across history and screen.
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import { join } from "node:path";

const ASSETS = join(__dirname, "../../crates/farhelm-ui/assets");
const ORIGIN = "https://grid.test";

const PAGE = `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<link rel="stylesheet" href="/assets/vendor/xterm.css">
<script src="/assets/vendor/xterm.js"></script>
<script src="/assets/vendor/addon-unicode-graphemes.js"></script>
<script src="/assets/grid-model.js"></script>
<script src="/assets/grid-buffer.js"></script>
</head>
<body style="margin: 0"><div id="xterm"></div></body>
</html>`;

async function openPage(page: Page): Promise<void> {
  await page.route(`${ORIGIN}/**`, async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/") return route.fulfill({ contentType: "text/html", body: PAGE });
    const name = path.replace(/^\/assets\//, "");
    if (!/^(vendor\/)?[A-Za-z0-9.-]+$/.test(name)) return route.abort();
    return route.fulfill({ path: join(ASSETS, name) });
  });
  await page.goto(`${ORIGIN}/`);
}

test("a copy across a wide character's wrap matches xterm.js, on the screen and from history", async ({ page }) => {
  await openPage(page);
  const copies = await page.evaluate(async () => {
    const w = window as any;
    // xterm.js as terminal.js configures its widths (Unicode graphemes).
    const term = new w.Terminal({ cols: 4, rows: 3, allowProposedApi: true, scrollback: 10 });
    term.loadAddon(new w.UnicodeGraphemesAddon.UnicodeGraphemesAddon());
    term.open(document.getElementById("xterm"));
    const write = (data: string) => new Promise((done) => term.write(data, done));
    await write("abc中x");
    term.select(0, 0, 7);
    const xtermScreen = term.getSelection();
    // Push both rows into scrollback, leaving the second on the screen's top.
    await write("\r\n\r\n");
    const top = term.buffer.active.baseY;
    term.select(0, top - 1, 7);
    const xtermHistory = term.getSelection();
    // The rows the supervisor sends for that text: "abc" wrapping, then the
    // wide character and "x".
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
    historyModel.apply({ t: "history", first: 0, cols: 4, rows: [wrapped] });
    const buffer = w.farhelmGridBuffer;
    const screenBase = screenModel.screenBase;
    const gridScreen = buffer.selectionText(buffer.modelSource(screenModel), { abs: screenBase, col: 0 }, { abs: screenBase + 1, col: 4 });
    const historyBase = historyModel.screenBase;
    const gridHistory = buffer.selectionText(buffer.modelSource(historyModel), { abs: 0, col: 0 }, { abs: historyBase, col: 4 });
    return { xtermScreen, xtermHistory, gridScreen, gridHistory };
  });
  // The premise: xterm.js itself copies the wrap without the padding.
  expect(copies.xtermScreen).toBe("abc中x");
  expect(copies.xtermHistory).toBe("abc中x");
  expect(copies.gridScreen).toBe(copies.xtermScreen);
  expect(copies.gridHistory).toBe(copies.xtermHistory);
});
