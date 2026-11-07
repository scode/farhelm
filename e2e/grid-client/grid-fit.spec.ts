// The fit: in a pane, the rows and columns a terminal gives its program are
// exactly what grid-terminal.js's "Size" section documents, computed from
// the character the browser measures, at every text size and pixel ratio.
//
// Why it matters: the fit is a contract, not a detail. A grid that fitted
// even one column differently from that formula would resize the program
// whenever a terminal whose kind the page did not know swaps from an old
// terminal to the grid (the socket URL carried the old terminal's size),
// redrawing the program and rebuilding its history; and a pane would show a
// different size depending on which component holds it. The formula is the
// old terminal's (xterm.js 6.0's measurement, cell rounding and fit
// add-on), and it is written down as Farhelm's own.
//
// Two tests hold the two components to the written formula, each on its
// own, rather than to each other:
//
// - the grid, with no xterm.js on the page: new terminals' fit is defined
//   by the formula, not by what xterm.js does;
// - old terminals' xterm.js, which only the compatibility swap depends on:
//   it shows that the size an old terminal's socket URL carries still
//   follows the same formula, so the swap does not resize the program. This
//   test goes when old terminals do.
//
// Not frozen row and column counts: the character's measured size differs
// across engines, fonts' hinting and machines, so expected counts are
// worked out from each run's own measurement.
//
// Each case mounts the component in a `.terminal` container (the class the
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
const OLD_TERMINAL_SCRIPTS = ["terminal-theme.js", "vendor/xterm.js", "vendor/addon-fit.js"];

// Pane sizes: an ordinary one, and one chosen so neither dimension is a
// round multiple of a common cell size.
const PANES = [
  { width: 820, height: 420 },
  { width: 1003, height: 611 },
];
// The supported range's ends, the default and sizes between.
const FONT_SIZES = [9, 12, 14, 15, 17, 20, 23, 28];

type Component = "grid" | "xterm";

function pageFor(component: Component): string {
  const scripts = component === "grid" ? GRID_SCRIPTS : OLD_TERMINAL_SCRIPTS;
  const xtermCss = component === "xterm" ? '<link rel="stylesheet" href="/assets/vendor/xterm.css">\n' : "";
  return `<!doctype html>
<html>
<head>
<meta charset="utf-8">
${xtermCss}<link rel="stylesheet" href="/assets/app.css">
${scripts.map((name) => `<script src="/assets/${name}"></script>`).join("\n")}
</head>
<body style="margin: 0"><div id="slots"></div></body>
</html>`;
}

async function openPage(page: Page, component: Component): Promise<void> {
  const body = pageFor(component);
  await page.route(`${ORIGIN}/**`, async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/") return route.fulfill({ contentType: "text/html", body });
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

/**
 * For every font size, the size `component` fitted and the size the
 * documented formula gives, in one pane. The formula is restated here rather
 * than taken from grid-terminal.js, so a change to either side shows:
 *
 * - the character: an offscreen canvas's `measureText("W")`, its width and
 *   its font bounding box's ascent plus descent;
 * - the cell at the size being fitted from (80 x 24 before the first fit)
 *   and pixel ratio `dpr`: width `round(charWidth * dpr * cols / dpr) /
 *   cols`, height `round(floor(ceil(charHeight * dpr)) * rows / dpr) /
 *   rows`;
 * - the room: the pane's computed width and height (whole pixels) less the
 *   terminal element's padding, and 14 more pixels of width for the
 *   scrollbar;
 * - the size: the room over the cell, rounded down, at least 2 x 1 and at
 *   most 1000 x 1000.
 */
async function fittedSizes(page: Page, component: Component, pane: { width: number; height: number }) {
  return page.evaluate(({ component, pane, fontSizes }) => {
    const w = window as any;
    const slots = document.getElementById("slots")!;
    slots.replaceChildren();
    const box = document.createElement("div");
    box.className = "terminal";
    box.style.width = `${pane.width}px`;
    box.style.height = `${pane.height}px`;
    slots.appendChild(box);
    const family = '"JetBrains Mono", monospace';
    // The component under test, behind one shape: its element, a way to
    // change the text size (which refits), its fitted size, and disposal.
    let mounted: { element: HTMLElement; setFontSize(size: number): void; size(): { cols: number; rows: number }; dispose(): void };
    if (component === "grid") {
      const grid = w.farhelmGridTerminal.create(box, {
        fontFamily: family,
        fontSize: fontSizes[0],
        theme: w.farhelmTerminalTheme,
        platform: navigator.platform,
        send: () => {},
      });
      mounted = grid;
    } else {
      // An old terminal as terminal.js configures its size: scrollback on,
      // so the fit add-on reserves the scrollbar's column.
      const term = new w.Terminal({ fontFamily: family, fontSize: fontSizes[0], scrollback: 12000, theme: w.farhelmTerminalTheme });
      const fit = new w.FitAddon.FitAddon();
      term.loadAddon(fit);
      term.open(box);
      mounted = {
        element: term.element,
        setFontSize(size) {
          term.options.fontSize = size;
          fit.fit();
        },
        size: () => ({ cols: term.cols, rows: term.rows }),
        dispose: () => term.dispose(),
      };
    }
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
    let from = { cols: 80, rows: 24 };
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
      from = expected;
    }
    mounted.dispose();
    return { canvasMetrics, padding, results };
  }, { component, pane, fontSizes: FONT_SIZES });
}

for (const deviceScaleFactor of [1, 1.5, 2]) {
  test.describe(`at pixel ratio ${deviceScaleFactor}`, () => {
    test.use({ deviceScaleFactor });

    test("the grid fits to the documented rows and columns at every text size", async ({ page }) => {
      await openPage(page, "grid");
      expect(await page.evaluate(() => window.devicePixelRatio)).toBe(deviceScaleFactor);
      for (const pane of PANES) {
        const { canvasMetrics, padding, results } = await fittedSizes(page, "grid", pane);
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

    /**
     * Old terminals only: the compatibility swap's premise. Why: a terminal
     * whose kind the page did not know is attached at xterm.js's fitted size
     * and then swapped to the grid; if xterm.js's fit left the formula the
     * grid follows, the swap would resize the program. Spec: xterm.js with
     * its fit add-on, in the same pane and steps, fits to the same
     * documented formula.
     */
    test("old terminals' xterm.js fits to the same documented formula, so the swap to the grid keeps the size", async ({ page }) => {
      await openPage(page, "xterm");
      expect(await page.evaluate(() => window.devicePixelRatio)).toBe(deviceScaleFactor);
      for (const pane of PANES) {
        const { canvasMetrics, results } = await fittedSizes(page, "xterm", pane);
        expect(canvasMetrics, "canvas text metrics with a font bounding box").toBe(true);
        for (const result of results) {
          expect(result.charWidth, `a measured character at ${result.font}px`).toBeGreaterThan(0);
          expect(result.fitted, `${pane.width}x${pane.height} at ${result.font}px`).toEqual(result.expected);
        }
      }
    });
  });
}
