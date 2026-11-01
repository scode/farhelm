// Fit parity: in the same pane, the grid terminal must get exactly the rows
// and columns xterm.js gets, at every text size and pixel ratio.
//
// Why it matters: the socket URL carries xterm.js's fitted size, because an
// island measures before it knows its stream's kind. A grid that fitted
// even one column differently would resize the program right after every
// attach, which redraws it and, for a grid stream, rebuilds its history; and
// a pane would show a different terminal size depending on which component
// it holds. The grid copies xterm.js 6.0's measurement and fit step for
// step (grid-terminal.js, "Size"); this holds it to the shipped xterm.js
// rather than to a description of it.
//
// Each case mounts xterm.js with its fit addon and the grid in identical
// `.terminal` containers (the class the session view's mount point has),
// then steps both through the same text sizes, as the text-size shortcut
// does, comparing after each step.
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import { join } from "node:path";

const ASSETS = join(__dirname, "../../crates/farhelm-ui/assets");
const ORIGIN = "https://grid.test";

const SCRIPTS = [
  "vendor/xterm.js",
  "vendor/addon-fit.js",
  "terminal-theme.js",
  "copy-on-select.js",
  "grid-model.js",
  "grid-style.js",
  "grid-input.js",
  "grid-buffer.js",
  "grid-terminal.js",
];

// Pane sizes: an ordinary one, and one chosen so neither dimension is a
// round multiple of a common cell size.
const PANES = [
  { width: 820, height: 420 },
  { width: 1003, height: 611 },
];
// The supported range's ends, the default and sizes between.
const FONT_SIZES = [9, 12, 14, 15, 17, 20, 23, 28];

const PAGE = `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<link rel="stylesheet" href="/assets/vendor/xterm.css">
<link rel="stylesheet" href="/assets/app.css">
${SCRIPTS.map((name) => `<script src="/assets/${name}"></script>`).join("\n")}
</head>
<body style="margin: 0"><div id="slots"></div></body>
</html>`;

async function openPage(page: Page): Promise<void> {
  await page.route(`${ORIGIN}/**`, async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/") return route.fulfill({ contentType: "text/html", body: PAGE });
    const name = path.replace(/^\/assets\//, "");
    if (!/^(vendor\/)?[A-Za-z0-9.-]+$/.test(name)) return route.abort();
    const file = name.endsWith(".woff2") ? join(ASSETS, "fonts", name) : join(ASSETS, name);
    return route.fulfill({ path: file });
  });
  await page.goto(`${ORIGIN}/`);
  // The fixture premise: the bundled font is the one measured, as the
  // product's mount gate guarantees, not a fallback.
  const loaded = await page.evaluate(async () => {
    const regular = await document.fonts.load('14px "JetBrains Mono"');
    const bold = await document.fonts.load('700 14px "JetBrains Mono"');
    return regular.length > 0 && bold.length > 0;
  });
  expect(loaded).toBe(true);
}

/** Every font size's fitted size from both components, in one pane. */
async function fittedSizes(page: Page, pane: { width: number; height: number }) {
  return page.evaluate(({ pane, fontSizes }) => {
    const w = window as any;
    const slots = document.getElementById("slots")!;
    slots.replaceChildren();
    const box = () => {
      const el = document.createElement("div");
      el.className = "terminal";
      el.style.width = `${pane.width}px`;
      el.style.height = `${pane.height}px`;
      slots.appendChild(el);
      return el;
    };
    const family = '"JetBrains Mono", monospace';
    const term = new w.Terminal({ fontFamily: family, fontSize: fontSizes[0], scrollback: 12000, theme: w.farhelmTerminalTheme });
    const fit = new w.FitAddon.FitAddon();
    term.loadAddon(fit);
    term.open(box());
    const grid = w.farhelmGridTerminal.create(box(), {
      fontFamily: family,
      fontSize: fontSizes[0],
      theme: w.farhelmTerminalTheme,
      platform: navigator.platform,
      send: () => {},
    });
    const results = [];
    for (const size of fontSizes) {
      term.options.fontSize = size;
      fit.fit();
      grid.setFontSize(size);
      results.push({ font: size, xterm: { cols: term.cols, rows: term.rows }, grid: grid.size() });
    }
    grid.dispose();
    term.dispose();
    return results;
  }, { pane, fontSizes: FONT_SIZES });
}

for (const deviceScaleFactor of [1, 1.5, 2]) {
  test.describe(`at pixel ratio ${deviceScaleFactor}`, () => {
    test.use({ deviceScaleFactor });

    test("the grid fits to xterm.js's rows and columns at every text size", async ({ page }) => {
      await openPage(page);
      expect(await page.evaluate(() => window.devicePixelRatio)).toBe(deviceScaleFactor);
      for (const pane of PANES) {
        const results = await fittedSizes(page, pane);
        for (const result of results) {
          expect(result.grid, `${pane.width}x${pane.height} at ${result.font}px`).toEqual(result.xterm);
        }
      }
    });
  });
}
