// The grid terminal component in a real browser, on a static page fed
// recorded grid streams (the same fixtures the node unit tests use).
//
// Why a browser at all: the node tests pin the decisions (decoding,
// styles, which key events become which messages), but three things only a
// layout engine can show. Columns must line up when wide and non-ASCII
// text sits in exact-width boxes beside text that flows at the font's
// advance; the cascade must actually produce the colors and decorations the
// inline styles name; and a real keyboard and mouse must turn into the
// input messages today's terminal would have sent. Both engines run, WebKit
// standing in for the desktop app's.
//
// No Farhelm stack is involved: the page and the shipped assets are served
// through request routing. The stack-level specs (a real supervisor sending
// grid streams) come with the parts of the stack that send them.
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const ASSETS = join(__dirname, "../../crates/farhelm-ui/assets");
const FIXTURES = join(__dirname, "../../crates/farhelm-ui/js-tests/fixtures/grid");
const ORIGIN = "https://grid.test";

// Records the document's live listeners, so a spec can tell whether a drag's
// listeners were removed. Each entry is "type|capture|id", the id naming the
// listener function.
const LISTENER_LEDGER = `
(() => {
  const ids = new WeakMap();
  let next = 0;
  const live = new Map();
  const key = (type, listener, options) => {
    if (!ids.has(listener)) ids.set(listener, ++next);
    const capture = typeof options === "boolean" ? options : !!(options && options.capture);
    return type + "|" + capture + "|" + ids.get(listener);
  };
  const add = document.addEventListener;
  const remove = document.removeEventListener;
  document.addEventListener = function (type, listener, options) {
    if (listener) live.set(key(type, listener, options), true);
    return add.call(this, type, listener, options);
  };
  document.removeEventListener = function (type, listener, options) {
    if (listener) live.delete(key(type, listener, options));
    return remove.call(this, type, listener, options);
  };
  window.documentListeners = () => [...live.keys()].sort();
})();
`;

const PAGE = `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<script>${LISTENER_LEDGER}</script>
<link rel="stylesheet" href="/assets/app.css">
<script src="/assets/terminal-theme.js"></script>
<script src="/assets/copy-on-select.js"></script>
<script src="/assets/grid-model.js"></script>
<script src="/assets/grid-style.js"></script>
<script src="/assets/grid-input.js"></script>
<script src="/assets/grid-terminal.js"></script>
</head>
<body style="margin: 0">
<div id="term" style="position: relative; width: 820px; height: 420px"></div>
<input id="elsewhere">
</body>
</html>`;

/** The fixture stream's messages, one JSON text per line. */
function stream(name: string): string[] {
  return readFileSync(join(FIXTURES, name), "utf8").split("\n").filter((line) => line.trim() !== "");
}

/**
 * Open the static page with a mounted grid terminal. `window.harness`
 * holds the component, every input message it sent, and the sizes it
 * reported; the bundled terminal font has loaded before this returns, so
 * cell metrics are the real ones.
 */
async function openGrid(page: Page): Promise<void> {
  await page.route(`${ORIGIN}/**`, async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/") return route.fulfill({ contentType: "text/html", body: PAGE });
    const name = path.replace(/^\/assets\//, "");
    if (!/^[A-Za-z0-9.-]+$/.test(name)) return route.abort();
    const file = name.endsWith(".woff2") ? join(ASSETS, "fonts", name) : join(ASSETS, name);
    return route.fulfill({ path: file });
  });
  await page.goto(`${ORIGIN}/`);
  await page.evaluate(async () => {
    await document.fonts.load('14px "JetBrains Mono"');
    const sent: unknown[] = [];
    const resizes: unknown[] = [];
    const w = window as any;
    const grid = w.farhelmGridTerminal.create(document.getElementById("term"), {
      fontSize: 14,
      fontFamily: '"JetBrains Mono", monospace',
      theme: w.farhelmTerminalTheme,
      platform: navigator.platform,
      send: (message: unknown) => sent.push(message),
      onResize: (cols: number, rows: number) => resizes.push({ cols, rows }),
    });
    grid.fit();
    const encoder = new TextEncoder();
    w.harness = {
      grid,
      sent,
      resizes,
      feed(lines: string[]) {
        for (const line of lines) grid.receive(encoder.encode(line));
        grid.flush();
      },
    };
  });
}

async function feed(page: Page, lines: string[]): Promise<void> {
  await page.evaluate((all) => (window as any).harness.feed(all), lines);
}

async function sent(page: Page): Promise<any[]> {
  return page.evaluate(() => (window as any).harness.sent.slice());
}

/** The measured cell, read from the component's own screen size. */
async function cell(page: Page): Promise<{ width: number; height: number }> {
  return page.evaluate(() => {
    const h = (window as any).harness;
    const screen = document.querySelector(".grid-terminal-screen") as HTMLElement;
    const rect = screen.getBoundingClientRect();
    return { width: rect.width / h.grid.model.cols, height: rect.height / h.grid.model.rows };
  });
}

const STATE = {
  t: "state",
  mouse: "off",
  focus: false,
  alt_screen: false,
  default_fg: null,
  default_bg: null,
  history_trimmed: false,
  exit: null,
};

function state(change: Record<string, unknown>): string {
  return JSON.stringify({ ...STATE, ...change });
}

test("a recorded attach draws its text, wide boxes, styles and cursor in their cells", async ({ page }) => {
  // The fixture's second row puts two wide characters in a box between
  // ASCII runs: the run after them must still start exactly at its column.
  await openGrid(page);
  await feed(page, stream("attach.jsonl"));
  const { width, height } = await cell(page);
  const rows = page.locator(".grid-terminal-row");
  await expect(rows).toHaveCount(4);
  await expect(rows.nth(0)).toHaveText("$ echo");
  const layout = await page.evaluate(() => {
    const row = document.querySelectorAll(".grid-terminal-row")[1];
    const left = row.getBoundingClientRect().left;
    return Array.from(row.children, (span) => {
      const rect = span.getBoundingClientRect();
      const style = getComputedStyle(span);
      return {
        text: span.textContent,
        left: rect.left - left,
        width: rect.width,
        color: style.color,
        weight: style.fontWeight,
        line: style.textDecorationLine,
        decoration: style.textDecorationColor,
      };
    });
  });
  const byText = (text: string) => layout.find((span) => span.text === text)!;
  // Columns: "hello" at 0, the wide characters at 6 and 8, "web" at 11.
  expect(byText("中").left).toBeCloseTo(6 * width, 0);
  expect(byText("中").width).toBeCloseTo(2 * width, 0);
  expect(byText("文").left).toBeCloseTo(8 * width, 0);
  expect(byText("web").left).toBeCloseTo(11 * width, 0);
  // Bold green draws bright green (#B9CA4A), as xterm.js does.
  expect(byText("hello").color).toBe("rgb(185, 202, 74)");
  expect(Number(byText("hello").weight)).toBeGreaterThanOrEqual(700);
  // An underlined run with its own underline color (blue).
  expect(byText("web").line).toContain("underline");
  expect(byText("web").decoration).toBe("rgb(0, 0, 255)");
  // The cursor covers cell (2, 2); without focus it is that cell's outline.
  const cursor = await page.locator(".grid-terminal-cursor").boundingBox();
  const screen = await page.locator(".grid-terminal-screen").boundingBox();
  expect(cursor!.x - screen!.x).toBeCloseTo(2 * width, 0);
  expect(cursor!.y - screen!.y).toBeCloseTo(2 * height, 0);
  expect(cursor!.width).toBeCloseTo(width, 0);
});

test("non-ASCII cells keep their columns: only covered box drawing, blocks and braille flow", async ({ page }) => {
  // Box drawing the terminal font draws at the cell width flows inline,
  // sharing a span with its neighbors (the benchmark's remedy for an
  // element per cell). Everything else is boxed: a block element this page
  // maps to a face 1.5 times too wide (so the premise does not depend on
  // which fonts the machine has), Arabic (the review's WebKit reproducer:
  // two U+06FB cells that measure at the cell width when repeated but
  // shape narrower together), and a cell whose text is only a bidi
  // control. Every later column must land exactly where it belongs; a
  // renderer that let everything flow fails both halves.
  await openGrid(page);
  await page.addStyleTag({
    content: '@font-face { font-family: "JetBrains Mono"; src: url("/assets/JetBrainsMonoNerdFont-Regular.woff2") format("woff2"); unicode-range: U+2591; size-adjust: 150%; }',
  });
  const premise = await page.evaluate(async () => {
    await document.fonts.load('14px "JetBrains Mono"', "\u2591");
    const probe = document.createElement("span");
    probe.style.cssText = 'font: 14px "JetBrains Mono"; white-space: pre; position: absolute;';
    document.body.appendChild(probe);
    probe.textContent = "W";
    const w = probe.getBoundingClientRect().width;
    probe.textContent = "\u2591";
    const shade = probe.getBoundingClientRect().width;
    probe.remove();
    return { w, shade };
  });
  // The fixture premise: the mapped block element is wider than a cell.
  expect(premise.shade).toBeGreaterThan(premise.w * 1.2);
  const plain = [0, -1, -1];
  await feed(page, [
    JSON.stringify({ t: "hello", cols: 40, rows: 2, history_cap: 0 }),
    JSON.stringify({
      t: "rows",
      rows: [
        [0, {
          w: false,
          r: [
            [0, 1, "\u2500", plain],
            [1, 1, "\u253c", plain],
            [2, 1, "\u2500", plain],
            [3, 2, "ab", plain],
            [5, 1, "\u2591", plain],
            [6, 2, "cd", plain],
            [8, 1, "\u200e", plain],
            [9, 2, "ef", plain],
          ],
        }],
        [1, { w: false, r: [[0, 1, "\u06fb", plain], [1, 1, "\u06fb", plain], [2, 1, "X", plain]] }],
      ],
    }),
    JSON.stringify({ t: "frame", rev: 1, cols: 40, rows: 2, cursor: { x: 0, y: 0, visible: false, shape: "block", blink: false } }),
  ]);
  const { width } = await cell(page);
  const rows = await page.evaluate(() =>
    [...document.querySelectorAll(".grid-terminal-row")].map((row) => {
      const rowLeft = row.getBoundingClientRect().left;
      const spans = [...row.children].map((span) => ({
        text: span.textContent,
        box: span.classList.contains("grid-terminal-box"),
        left: span.getBoundingClientRect().left - rowLeft,
        width: span.getBoundingClientRect().width,
      }));
      // Where each ASCII letter is drawn, from the text itself.
      const letters: Record<string, number> = {};
      for (const span of row.children) {
        const text = span.firstChild;
        if (!text) continue;
        for (let i = 0; i < text.textContent!.length; i++) {
          const ch = text.textContent![i];
          if (!/[a-zA-Z]/.test(ch)) continue;
          const range = document.createRange();
          range.setStart(text, i);
          range.setEnd(text, i + 1);
          letters[ch] = range.getBoundingClientRect().left - rowLeft;
        }
      }
      return { spans, letters };
    })
  );
  const [first, second] = rows;
  // Covered box drawing flows with the ASCII after it.
  expect(first.spans[0].box).toBe(false);
  expect(first.spans[0].text!.startsWith("\u2500\u253c\u2500ab")).toBe(true);
  // The too-wide block element and the bidi-only cell are boxes of exactly
  // one cell, the latter drawing nothing.
  const shade = first.spans.find((span) => span.text === "\u2591")!;
  expect(shade.box).toBe(true);
  expect(shade.left).toBeCloseTo(5 * width, 0);
  expect(shade.width).toBeCloseTo(width, 0);
  const empty = first.spans.find((span) => span.text === "")!;
  expect(empty.box).toBe(true);
  expect(empty.left).toBeCloseTo(8 * width, 0);
  // Arabic cells are boxes.
  expect(second.spans.filter((span) => span.text === "\u06fb").every((span) => span.box)).toBe(true);
  // Every ASCII letter in its column.
  const columns = { a: 3, b: 4, c: 6, d: 7, e: 9, f: 10 };
  for (const [letter, column] of Object.entries(columns)) {
    expect(Math.abs(first.letters[letter] - column * width), `${letter} at column ${column}`).toBeLessThan(1);
  }
  expect(Math.abs(second.letters.X - 2 * width), "X at column 2").toBeLessThan(1);
});

test("the cursor follows focus, shape and visibility", async ({ page }) => {
  await openGrid(page);
  await feed(page, stream("attach.jsonl"));
  await page.evaluate(() => (window as any).harness.grid.focus());
  const cursor = page.locator(".grid-terminal-cursor");
  // Focused, a blinking block shows the cell's character in the accent color.
  await expect(cursor).toHaveClass(/grid-terminal-cursor-blink/);
  await expect(cursor).toHaveCSS("background-color", "rgb(255, 255, 255)");
  await page.locator("#elsewhere").focus();
  await expect(cursor).not.toHaveClass(/grid-terminal-cursor-blink/);
  await expect(cursor).toHaveCSS("outline-style", "solid");
  // Hidden, then shown again exactly where and as it was: it must come back.
  const at = (rev: number, visible: boolean) =>
    JSON.stringify({ t: "frame", rev, cols: 20, rows: 4, cursor: { x: 0, y: 0, visible, shape: "bar", blink: false } });
  await feed(page, [at(2, true)]);
  await expect(cursor).toBeVisible();
  await feed(page, [at(3, false)]);
  await expect(cursor).toBeHidden();
  await feed(page, [at(4, true)]);
  await expect(cursor).toBeVisible();
  // Focused, a bar is a one-pixel line at the cell's left edge.
  await page.evaluate(() => (window as any).harness.grid.focus());
  await expect(cursor).toHaveCSS("box-shadow", /inset/);
});

test("markup-looking, bidi-overriding and right-to-left text is drawn as literal cells in column order", async ({ page }) => {
  // Program output is data: no element may come of it. And cells are
  // already in visual order: neither a right-to-left override (zero-width,
  // so an emulator keeps it with the cell before) nor right-to-left letters
  // may make the browser reorder them.
  await openGrid(page);
  const markup = "<img src=x onerror=window.pwned=1>";
  const plain = [0, -1, -1];
  await feed(page, [
    JSON.stringify({ t: "hello", cols: 60, rows: 2, history_cap: 0 }),
    JSON.stringify({
      t: "rows",
      rows: [
        [0, { w: false, r: [[0, markup.length, markup, plain]] }],
        [1, {
          w: false,
          r: [
            [0, 2, "ab", plain],
            [2, 1, "c\u202e", plain],
            [3, 3, "def", plain],
            [6, 1, "\u05e9", plain],
            [7, 1, "\u05dc", plain],
            [8, 1, "\u05d5", plain],
          ],
        }],
      ],
    }),
    JSON.stringify({ t: "frame", rev: 1, cols: 60, rows: 2, cursor: { x: 0, y: 0, visible: true, shape: "block", blink: false } }),
  ]);
  await expect(page.locator(".grid-terminal-row").nth(0)).toHaveText(markup);
  expect(await page.locator(".grid-terminal img").count()).toBe(0);
  expect(await page.evaluate(() => (window as any).pwned)).toBeUndefined();
  await expect(page.locator(".grid-terminal-row").nth(1)).toHaveText("abcdef\u05e9\u05dc\u05d5");
  // Each character's position, whichever span or box holds it.
  const lefts = await page.evaluate(() => {
    const row = document.querySelectorAll(".grid-terminal-row")[1];
    const rowLeft = row.getBoundingClientRect().left;
    const out: number[] = [];
    for (const span of row.children) {
      const text = span.firstChild!;
      for (let i = 0; i < text.textContent!.length; i++) {
        const range = document.createRange();
        range.setStart(text, i);
        range.setEnd(text, i + 1);
        out.push(range.getBoundingClientRect().left - rowLeft);
      }
    }
    return out;
  });
  const { width } = await cell(page);
  // Every character in its own column, left to right, the right-to-left
  // letters included (columns 6, 7 and 8).
  expect(lefts.map((left) => Math.round(left / width))).toEqual([0, 1, 2, 3, 4, 5, 6, 7, 8]);
});

test("typing sends the same key and text messages today's terminal would", async ({ page }) => {
  // Keys xterm.js encodes at keydown go as `key` messages for the
  // supervisor to encode; keys whose character comes from text input
  // (capitals, space) go as `text`. Shift+Enter is a key with the Shift bit.
  await openGrid(page);
  await feed(page, stream("attach.jsonl"));
  await page.evaluate(() => (window as any).harness.grid.focus());
  await page.keyboard.press("a");
  await page.keyboard.press("Shift+A");
  await page.keyboard.press("Space");
  await page.keyboard.press("Shift+Enter");
  await page.keyboard.press("Control+c");
  await page.keyboard.press("ArrowUp");
  await expect.poll(() => sent(page)).toEqual([
    { t: "key", key: "a", code: "KeyA", kc: 65, mods: 0 },
    { t: "text", data: "A" },
    { t: "text", data: " " },
    { t: "key", key: "Enter", code: "Enter", kc: 13, mods: 1 },
    { t: "key", key: "c", code: "KeyC", kc: 67, mods: 4 },
    { t: "key", key: "ArrowUp", code: "ArrowUp", kc: 38, mods: 0 },
  ]);
});

test("the mouse reports cells only in the modes the program asked for", async ({ page }) => {
  await openGrid(page);
  await feed(page, stream("attach.jsonl"));
  const { width, height } = await cell(page);
  const screen = (await page.locator(".grid-terminal-screen").boundingBox())!;
  const at = (x: number, y: number) => ({ x: screen.x + (x + 0.5) * width, y: screen.y + (y + 0.5) * height });
  // Mouse reporting off: a click focuses the terminal and sends nothing.
  await page.mouse.click(at(3, 1).x, at(3, 1).y);
  expect(await sent(page)).toEqual([]);
  // Turning focus reporting on while focused reports focus at once.
  await feed(page, [state({ mouse: "click", focus: true })]);
  await page.mouse.click(at(3, 1).x, at(3, 1).y);
  await page.mouse.wheel(0, 120);
  await page.locator("#elsewhere").focus();
  await expect.poll(() => sent(page)).toEqual([
    { t: "focus", in: true },
    { t: "mouse", kind: "down", button: 0, x: 3, y: 1, mods: 0 },
    { t: "mouse", kind: "up", button: 0, x: 3, y: 1, mods: 0 },
    { t: "mouse", kind: "wheel", button: 1, x: 3, y: 1, mods: 0 },
    { t: "focus", in: false },
  ]);
});

test("turning focus reporting on reports the current focus, focused or not", async ({ page }) => {
  // xterm.js answers DECSET 1004 with the state at that moment, so a
  // program that dims when unfocused starts out right either way.
  await openGrid(page);
  await feed(page, stream("attach.jsonl"));
  await page.evaluate(() => (window as any).harness.grid.focus());
  await feed(page, [state({ focus: true })]);
  await expect.poll(() => sent(page)).toEqual([{ t: "focus", in: true }]);
  await feed(page, [state({ focus: false })]);
  await page.locator("#elsewhere").focus();
  await feed(page, [state({ focus: true })]);
  // Leaving while reporting was off sent nothing; enabling it again while
  // unfocused reports focus-out.
  await expect.poll(() => sent(page)).toEqual([{ t: "focus", in: true }, { t: "focus", in: false }]);
});

test("a drag under drag reporting follows the held button and its release outside the terminal", async ({ page }) => {
  // The drag leaves the terminal and is released outside it; xterm.js
  // follows both on the document and reports them at the grid's edge.
  // Once released, a buttonless move is not a drag and reports nothing.
  await openGrid(page);
  await feed(page, [...stream("attach.jsonl"), state({ mouse: "drag" })]);
  const { width, height } = await cell(page);
  const screen = (await page.locator(".grid-terminal-screen").boundingBox())!;
  const at = (x: number, y: number) => ({ x: screen.x + (x + 0.5) * width, y: screen.y + (y + 0.5) * height });
  // Moves without a button are not reported under "drag".
  const idle = await page.evaluate(() => (window as any).documentListeners());
  await page.mouse.move(at(1, 0).x, at(1, 0).y);
  await page.mouse.down();
  // The drag follows the pointer on the document while the button is down.
  expect((await page.evaluate(() => (window as any).documentListeners())).length).toBe(idle.length + 2);
  await page.mouse.move(at(4, 2).x, at(4, 2).y);
  await page.mouse.move(at(4, 2).x + 1, at(4, 2).y);
  await page.mouse.move(screen.x + 900, screen.y + 500);
  await page.mouse.up();
  expect(await page.evaluate(() => (window as any).documentListeners())).toEqual(idle);
  await page.mouse.move(at(2, 2).x, at(2, 2).y);
  await expect.poll(() => sent(page)).toEqual([
    { t: "mouse", kind: "down", button: 0, x: 1, y: 0, mods: 0 },
    { t: "mouse", kind: "move", button: 0, x: 4, y: 2, mods: 0 },
    // Outside the grid, clamped to its last cell, as xterm.js does.
    { t: "mouse", kind: "move", button: 0, x: 19, y: 3, mods: 0 },
    { t: "mouse", kind: "up", button: 0, x: 19, y: 3, mods: 0 },
  ]);
});

test("disposing the grid mid-drag removes the drag's document listeners", async ({ page }) => {
  // A terminal torn down (a reconnect, a closed tab) while a button is
  // held must not leave listeners reporting into a dead attachment.
  await openGrid(page);
  await feed(page, [...stream("attach.jsonl"), state({ mouse: "drag" })]);
  const idle = await page.evaluate(() => (window as any).documentListeners());
  const screen = (await page.locator(".grid-terminal-screen").boundingBox())!;
  await page.mouse.move(screen.x + 5, screen.y + 5);
  await page.mouse.down();
  expect((await page.evaluate(() => (window as any).documentListeners())).length).toBe(idle.length + 2);
  await page.evaluate(() => (window as any).harness.grid.dispose());
  expect(await page.evaluate(() => (window as any).documentListeners())).toEqual(idle);
  await page.mouse.up();
});

test("a hello at another size than the pane fits asks once for the fitted size", async ({ page }) => {
  // The hello carries the program's real size. If the attach-time resize
  // failed, or the supervisor clamped it, the grid asks again; at the
  // fitted size it asks nothing.
  await openGrid(page);
  const fitted = await page.evaluate(() => (window as any).harness.grid.size());
  const resizes = () => page.evaluate(() => (window as any).harness.resizes.slice());
  expect(await resizes()).toEqual([fitted]);
  await feed(page, [JSON.stringify({ t: "hello", cols: fitted.cols, rows: fitted.rows, history_cap: 0 })]);
  expect(await resizes()).toEqual([fitted]);
  // A fresh page and grid in the same pane.
  await openGrid(page);
  await feed(page, [JSON.stringify({ t: "hello", cols: 80, rows: 24, history_cap: 0 })]);
  expect(await resizes()).toEqual([fitted, fitted]);
});

test("a right click under mouse reporting keeps the browser's context menu, as xterm.js does", async ({ page }) => {
  await openGrid(page);
  await feed(page, [...stream("attach.jsonl"), state({ mouse: "click" })]);
  const prevented = await page.evaluate(() => {
    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    document.querySelector(".grid-terminal-screen")!.dispatchEvent(event);
    return event.defaultPrevented;
  });
  expect(prevented).toBe(false);
});

test("the alternate screen sends wheel steps without mouse reporting", async ({ page }) => {
  // Today's terminal turns the wheel into arrow keys there; the supervisor
  // does that from the same wheel message.
  await openGrid(page);
  await feed(page, [...stream("attach.jsonl"), state({ alt_screen: true })]);
  const screen = (await page.locator(".grid-terminal-screen").boundingBox())!;
  await page.mouse.move(screen.x + 5, screen.y + 5);
  await page.mouse.wheel(0, -120);
  await expect.poll(() => sent(page)).toEqual([{ t: "mouse", kind: "wheel", button: 0, x: 0, y: 0, mods: 0 }]);
});

test("an ended program's exit status is shown by the client, below its last row", async ({ page }) => {
  // The exit line is the client's, outside the program's output: it may
  // move the screen up (as tmux's line scrolled the pane) but never covers
  // a row. The fixture's 4 rows leave plenty of room; a full-height screen
  // leaves less than a line.
  await openGrid(page);
  const exitLine = page.locator(".grid-terminal-exit");
  const overlap = () =>
    page.evaluate(() => {
      const rows = [...document.querySelectorAll(".grid-terminal-row")].map((row) => row.getBoundingClientRect());
      const line = document.querySelector(".grid-terminal-exit")!.getBoundingClientRect();
      const host = document.querySelector(".grid-terminal")!.getBoundingClientRect();
      const style = getComputedStyle(document.querySelector(".grid-terminal")!);
      const contentTop = host.top + parseFloat(style.paddingTop);
      // Anything of a row drawn in the top padding, where the element's own
      // clip (at its padding edge) would let a lifted row's bottom show.
      const padding = [1, 2, 3].some((dy) =>
        document.elementsFromPoint(host.left + 20, host.top + dy).some((el) => !!el.closest(".grid-terminal-row"))
      );
      return {
        // No row is cut by the top of the content box: whole rows only.
        sliver: padding || rows.some((row) => row.top < contentTop - 0.5 && row.bottom > contentTop + 0.5),
        coversRow: rows.some((row) => row.bottom > line.top + 0.5 && row.top < line.bottom - 0.5),
        belowLastRow: line.top >= rows[rows.length - 1].bottom - 0.5,
        inside: line.bottom <= host.bottom + 0.5,
      };
    });
  await feed(page, [...stream("attach.jsonl"), state({ exit: { code: 3 } })]);
  await expect(exitLine).toHaveText("Process exited with status 3");
  expect(await overlap()).toEqual({ sliver: false, coversRow: false, belowLastRow: true, inside: true });
  await feed(page, [state({ exit: { signal: 9 } })]);
  await expect(exitLine).toHaveText("Process ended by signal 9");
  // Now a screen of the fitted size, which fills the terminal, every row
  // with a red background, so a lifted row showing in the padding would be
  // a red strip there.
  const size = await page.evaluate(() => (window as any).harness.grid.size());
  const red = Array.from({ length: size.rows }, (_, y) => [y, { w: false, r: [[0, size.cols, " ".repeat(size.cols), [0, -1, 1]]] }]);
  await feed(page, [
    JSON.stringify({ t: "rows", rows: red }),
    JSON.stringify({ t: "frame", rev: 2, cols: size.cols, rows: size.rows, cursor: { x: 0, y: 0, visible: true, shape: "block", blink: true } }),
  ]);
  expect(await overlap()).toEqual({ sliver: false, coversRow: false, belowLastRow: true, inside: true });
  await feed(page, [state({ exit: null })]);
  await expect(exitLine).toBeHidden();
});

test("the grid fits its container and refits at a new text size", async ({ page }) => {
  // The fitted size is what the resize control message sends; a larger
  // text size must shrink it, as with xterm.js.
  await openGrid(page);
  const first = await page.evaluate(() => (window as any).harness.resizes.slice());
  expect(first).toHaveLength(1);
  await page.evaluate(() => (window as any).harness.grid.setFontSize(20));
  const resizes = await page.evaluate(() => (window as any).harness.resizes.slice());
  expect(resizes).toHaveLength(2);
  expect(resizes[1].cols).toBeLessThan(resizes[0].cols);
  expect(resizes[1].rows).toBeLessThan(resizes[0].rows);
});
