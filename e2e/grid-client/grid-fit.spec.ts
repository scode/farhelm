// The fit: in a pane, the rows and columns a terminal gives its program are
// exactly what grid-terminal.js's "Size" section documents, computed from
// the character the browser measures, at every text size and pixel ratio.
//
// Why it matters: the fit is Farhelm's contract for how many rows and
// columns a pane gives its program, and fitting from a previous answer made
// the same pane depend on its resize history. The grid uses a fixed 80 x 24
// proposal reference, and these tests hold it to its written formula.
//
// Not frozen row and column counts: the character's measured size differs
// across engines, fonts' hinting and machines, so expected counts are
// worked out from each run's own measurement.
//
// Each case mounts the grid in a `.terminal` container (the class the
// session view's mount point has), then steps it through text sizes, as the
// text-size shortcut does, comparing after each step.
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import { join } from "node:path";

const ASSETS = join(__dirname, "../../crates/farhelm-ui/assets");
const ORIGIN = "https://grid.test";

const GRID_SCRIPTS = [
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

function pageFor(): string {
  return `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<link rel="stylesheet" href="/assets/app.css">
${GRID_SCRIPTS.map((name) => `<script src="/assets/${name}"></script>`).join("\n")}
</head>
<body style="margin: 0"><div id="slots"></div></body>
</html>`;
}

async function openPage(page: Page): Promise<void> {
  const body = pageFor();
  await page.route(`${ORIGIN}/**`, async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/") return route.fulfill({ contentType: "text/html", body });
    const name = path.replace(/^\/assets\//, "");
    if (!/^[A-Za-z0-9.-]+$/.test(name)) return route.abort();
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

/** A newly mounted view must not resize a quiet program again just because its observer reports the same box.
 * Session selection creates a fresh component. Rounding at its initial size
 * can choose a different column count on a second fit, even though the pane
 * and font did not change. Supply the fractional advance observed in WebKit
 * at 13px so both engines exercise that boundary; real-font formula coverage
 * remains separate below.
 */
test("an unchanged pane keeps its first fitted size", async ({ page }, testInfo) => {
  await openPage(page);
  const result = await page.evaluate(() => {
    const w = window as any;
    const family = '"JetBrains Mono", monospace';
    const measureText = OffscreenCanvasRenderingContext2D.prototype.measureText;
    OffscreenCanvasRenderingContext2D.prototype.measureText = function (text) {
      const metrics = measureText.call(this, text);
      Object.defineProperty(metrics, "width", { value: 7.8000030517578125 });
      return metrics;
    };
    const box = document.createElement("div");
    box.className = "terminal";
    // The actual stylesheet contributes 4px on each side; the fit also
    // reserves its 14px scrollbar. Assert those premises outside the page.
    box.style.width = "560px";
    box.style.height = "420px";
    document.getElementById("slots")!.appendChild(box);
    const resizes: { cols: number; rows: number }[] = [];
    const grid = w.farhelmGridTerminal.create(box, {
      fontFamily: family,
      fontSize: 13,
      theme: w.farhelmTerminalTheme,
      platform: navigator.platform,
      send: () => {},
      onResize: (cols: number, rows: number) => resizes.push({ cols, rows }),
    });
    const style = getComputedStyle(grid.element);
    const padding = [style.paddingLeft, style.paddingRight];
    const sizes = [];
    for (let i = 0; i < 3; i++) {
      grid.fit();
      sizes.push(grid.size());
    }
    grid.dispose();
    return { padding, sizes, resizes };
  });
  await testInfo.attach("same-pane-fits.json", { body: JSON.stringify(result), contentType: "application/json" });
  expect(result.padding).toEqual(["4px", "4px"]);
  expect(result.sizes[0].cols, "premise: fractional advance and fixed pane first fit to 68 columns").toBe(68);
  expect(result.sizes.slice(1), "the same pane must not change rows or columns").toEqual([result.sizes[0], result.sizes[0]]);
  expect(result.resizes, "repeated notifications must not resize the program again").toEqual([result.sizes[0]]);
});

test.describe("sizing after real pane changes", () => {
  test.use({ deviceScaleFactor: 1.5 });

  /** Returning to a pane must not carry a rounding answer from its earlier size.
   * A fractional advance makes the old fit-from-current-size rule choose 84
   * columns after a two-pixel growth but 83 on a fresh mount of that pane.
   * Compare whole rows and columns after several real width/height histories,
   * at a fractional pixel ratio, rather than only repeating an unchanged fit.
   */
  test("resized and freshly mounted panes give the same size", async ({ page }, testInfo) => {
    await openPage(page);
    const result = await page.evaluate(() => {
      const w = window as any;
      const measureText = OffscreenCanvasRenderingContext2D.prototype.measureText;
      OffscreenCanvasRenderingContext2D.prototype.measureText = function (text) {
        const metrics = measureText.call(this, text);
        Object.defineProperty(metrics, "width", { value: 7.8000030517578125 });
        return metrics;
      };
      const histories = [
        [[655, 402]],
        [[653, 400], [655, 402]],
        [[800, 500], [653, 400], [655, 402]],
      ];
      const observations = histories.map((history) => {
        const box = document.createElement("div");
        box.className = "terminal";
        document.getElementById("slots")!.appendChild(box);
        const grid = w.farhelmGridTerminal.create(box, {
          fontFamily: '"JetBrains Mono", monospace',
          fontSize: 13,
          theme: w.farhelmTerminalTheme,
          platform: navigator.platform,
          send: () => {},
        });
        try {
          const sizes = history.map(([room, height]) => {
            box.style.width = `${room + 22}px`;
            box.style.height = `${height + 8}px`;
            grid.fit();
            return grid.size();
          });
          const style = getComputedStyle(grid.element);
          return { history, sizes, padding: [style.paddingLeft, style.paddingRight] };
        } finally {
          grid.dispose();
          box.remove();
        }
      });
      return { ratio: window.devicePixelRatio, observations };
    });
    await testInfo.attach("resize-history-fits.json", { body: JSON.stringify(result), contentType: "application/json" });
    expect(result.ratio).toBe(1.5);
    for (const observation of result.observations) expect(observation.padding).toEqual(["4px", "4px"]);
    const fresh = result.observations[0].sizes.at(-1);
    expect(fresh!.cols, "premise: the controlled 655px room fits 83 columns on a fresh mount").toBe(83);
    for (const observation of result.observations.slice(1)) {
      expect(observation.sizes.at(-1), "resize history must not change the same pane's answer").toEqual(fresh);
    }
  });
});

/**
 * For every font size, the size the grid fitted and the size the
 * documented formula gives, in one pane. The formula is restated here rather
 * than taken from grid-terminal.js, so a change to either side shows:
 *
 * - the character: an offscreen canvas's `measureText("W")`, its width and
 *   its font bounding box's ascent plus descent;
 * - the cell at the fixed 80 x 24 reference and pixel ratio `dpr`: width
 *   `round(charWidth * dpr * cols / dpr) / cols`, height
 *   `round(floor(ceil(charHeight * dpr)) * rows / dpr) / rows`;
 * - the room: the pane's computed width and height (whole pixels) less the
 *   terminal element's padding, and 14 more pixels of width for the
 *   scrollbar;
 * - the size: the room over the cell, rounded down, at least 2 x 1 and at
 *   most 1000 x 1000.
 */
async function fittedSizes(page: Page, pane: { width: number; height: number }) {
  return page.evaluate(({ pane, fontSizes }) => {
    const w = window as any;
    const slots = document.getElementById("slots")!;
    slots.replaceChildren();
    const box = document.createElement("div");
    box.className = "terminal";
    box.style.width = `${pane.width}px`;
    box.style.height = `${pane.height}px`;
    slots.appendChild(box);
    const family = '"JetBrains Mono", monospace';
    const mounted = w.farhelmGridTerminal.create(box, {
      fontFamily: family,
      fontSize: fontSizes[0],
      theme: w.farhelmTerminalTheme,
      platform: navigator.platform,
      send: () => {},
    });
    const context = new OffscreenCanvas(100, 100).getContext("2d")!;
    const sample = context.measureText("W");
    const canvasMetrics = "width" in sample && "fontBoundingBoxAscent" in sample && "fontBoundingBoxDescent" in sample;
    const dpr = window.devicePixelRatio;
    const outer = getComputedStyle(box);
    const inner = getComputedStyle(mounted.element);
    const px = (style: CSSStyleDeclaration, name: string) => parseInt(style.getPropertyValue(name), 10);
    const padding = {
      left: px(inner, "padding-left"),
      right: px(inner, "padding-right"),
      top: px(inner, "padding-top"),
      bottom: px(inner, "padding-bottom"),
    };
    const roomWidth = px(outer, "width") - padding.left - padding.right;
    const roomHeight = px(outer, "height") - padding.top - padding.bottom;
    const from = { cols: 80, rows: 24 };
    const results = [];
    for (const size of fontSizes) {
      context.font = `${size}px ${family}`;
      const metrics = context.measureText("W");
      const charWidth = metrics.width;
      const charHeight = metrics.fontBoundingBoxAscent + metrics.fontBoundingBoxDescent;
      const cellWidth = Math.round((charWidth * dpr * from.cols) / dpr) / from.cols;
      const cellHeight = Math.round((Math.floor(Math.ceil(charHeight * dpr)) * from.rows) / dpr) / from.rows;
      const expected = {
        cols: Math.min(1000, Math.max(2, Math.floor((roomWidth - 14) / cellWidth))),
        rows: Math.min(1000, Math.max(1, Math.floor(roomHeight / cellHeight))),
      };
      mounted.setFontSize(size);
      results.push({ font: size, fitted: mounted.size(), expected, charWidth });
    }
    mounted.dispose();
    return { canvasMetrics, padding, results };
  }, { pane, fontSizes: FONT_SIZES });
}

for (const deviceScaleFactor of [1, 1.5, 2]) {
  test.describe(`at pixel ratio ${deviceScaleFactor}`, () => {
    test.use({ deviceScaleFactor });

    test("the grid fits to the documented rows and columns at every text size", async ({ page }) => {
      await openPage(page);
      expect(await page.evaluate(() => window.devicePixelRatio)).toBe(deviceScaleFactor);
      for (const pane of PANES) {
        const { canvasMetrics, padding, results } = await fittedSizes(page, pane);
        // The premises: this engine's canvas reports the metrics the grid's
        // own measurement requires, so the grid measures by the canvas as
        // the formula does, not by its span fallback; and the grid
        // element's padding (app.css, `.grid-terminal`) is what was read.
        expect(canvasMetrics, "canvas text metrics with a font bounding box").toBe(true);
        expect(padding).toEqual({ left: 4, right: 4, top: 4, bottom: 4 });
        for (const result of results) {
          expect(result.charWidth, `a measured character at ${result.font}px`).toBeGreaterThan(0);
          expect(result.fitted, `${pane.width}x${pane.height} at ${result.font}px`).toEqual(result.expected);
        }
      }
    });
  });
}
