// The fit: in a pane, the rows and columns a terminal gives its program are
// exactly what grid-terminal.js's "Size" section documents, computed from
// the character the browser measures, at every text size and pixel ratio.
//
// Why it matters: fitting from a previous answer made the same pane depend
// on its resize history. The grid uses a fixed 80 x 24 proposal reference;
// old terminals retain their legacy calculation. A compatibility handoff
// explicitly preserves its inherited answer until an effective input
// change, rather than relying on both calculations always agreeing.
//
// Two tests hold the components to their written formulas, each on its
// own, rather than to each other:
//
// - the grid, with no xterm.js on the page: new terminals' fit is defined
//   by the formula, not by what xterm.js does;
// - old terminals' xterm.js, with its existing fit-from-current-size rule.
//   Direct inherited-size and attachment tests establish handoff behavior.
//   Legacy formula coverage goes when old terminals do.
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

/** A newly mounted view must not resize a quiet program again just because its observer reports the same box.
 * Session selection creates a fresh component. Rounding at its initial size
 * can choose a different column count on a second fit, even though the pane
 * and font did not change. Supply the fractional advance observed in WebKit
 * at 13px so both engines exercise that boundary; real-font formula coverage
 * remains separate below.
 */
test("an unchanged pane keeps its first fitted size", async ({ page }, testInfo) => {
  await openPage(page, "grid");
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

/** A renderer handoff keeps a size that deliberately differs from direct fitting.
 * Replacing a renderer alone must not resize the program or rebuild history.
 * The inherited answer is local to this mount; a real later pane change must
 * release it and use the same answer as a fresh direct grid in that new pane.
 */
test("an inherited size survives unchanged fits and yields to a real pane change", async ({ page }) => {
  await openPage(page, "grid");
  const result = await page.evaluate(() => {
    const w = window as any;
    const measureText = OffscreenCanvasRenderingContext2D.prototype.measureText;
    OffscreenCanvasRenderingContext2D.prototype.measureText = function (text) {
      const metrics = measureText.call(this, text);
      Object.defineProperty(metrics, "width", { value: 7.8000030517578125 });
      return metrics;
    };
    const carried = { cols: 84, rows: 31 };
    const resizes: { cols: number; rows: number }[] = [];
    /** Mount equal panes so only the explicit inherited-size contract differs. */
    const create = (size?: { cols: number; rows: number }) => {
      const box = document.createElement("div");
      box.className = "terminal";
      box.style.width = "677px";
      box.style.height = "410px";
      document.getElementById("slots")!.appendChild(box);
      const grid = w.farhelmGridTerminal.create(box, {
        fontFamily: '"JetBrains Mono", monospace', fontSize: 13,
        theme: w.farhelmTerminalTheme, platform: navigator.platform, size,
        send: () => {}, onResize: size ? (cols: number, rows: number) => resizes.push({ cols, rows }) : undefined,
      });
      return { box, grid };
    };
    const inherited = create(carried);
    const direct = create();
    try {
      direct.grid.fit();
      const canonical = direct.grid.size();
      if (canonical.cols !== 83) throw new Error("controlled pane no longer differs from the inherited 84 columns");
      for (let i = 0; i < 3; i++) inherited.grid.fit();
      const kept = inherited.grid.size();
      const before = [...resizes];
      inherited.box.style.width = direct.box.style.width = "679px";
      inherited.grid.fit();
      direct.grid.fit();
      return { carried, canonical, kept, before, changed: inherited.grid.size(), directChanged: direct.grid.size(), resizes };
    } finally {
      inherited.grid.dispose();
      direct.grid.dispose();
      inherited.box.remove();
      direct.box.remove();
    }
  });
  expect(result.canonical.cols, "premise: direct sizing differs from the carried 84 columns").toBe(83);
  expect(result.kept).toEqual(result.carried);
  expect(result.before, "changing renderers alone must send no new resize").toEqual([]);
  expect(result.changed).toEqual(result.directChanged);
  expect(result.resizes).toEqual([result.directChanged]);
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
    await openPage(page, "grid");
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
 * For every font size, the size `component` fitted and the size the
 * documented formula gives, in one pane. The formula is restated here rather
 * than taken from grid-terminal.js, so a change to either side shows:
 *
 * - the character: an offscreen canvas's `measureText("W")`, its width and
 *   its font bounding box's ascent plus descent;
 * - the cell at a fixed 80 x 24 reference for the grid, or the previous
 *   fitted size for xterm (80 x 24 before the first fit),
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
      if (component === "xterm") from = expected;
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
     * Old terminals retain their legacy fit-from-current-size calculation.
     * The grid's new policy must not accidentally change this independent
     * compatibility surface. Handoff preservation is tested directly above
     * and through terminal.js's attachment fixture, not inferred from equal
     * formulas across the two renderers.
     */
    test("old terminals' xterm.js retains its documented legacy fit", async ({ page }) => {
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
