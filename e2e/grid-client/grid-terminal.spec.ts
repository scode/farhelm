// The grid terminal component in a real browser, on a static page fed
// recorded grid streams (the same fixtures the node unit tests use).
//
// Why a browser at all: the node tests pin the decisions (decoding,
// styles, which key events become which messages), but three things only a
// layout engine can show. Columns must line up when wide and non-ASCII
// text sits in exact-width boxes beside text that flows at the font's
// advance; the cascade must actually produce the colors and decorations the
// inline styles name; and a real keyboard and mouse must turn into the
// input messages the grid's keyboard and mouse rules call for. Both engines run, WebKit
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
const TEST_ENCODER = readFileSync(join(__dirname, "../../crates/farhelm-ui/js-tests/grid-test-encoder.js"), "utf8");
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
<script>${TEST_ENCODER}</script>
<script>${LISTENER_LEDGER}</script>
<link rel="stylesheet" href="/assets/app.css">
<script src="/assets/terminal-theme.js"></script>
<script src="/assets/copy-on-select.js"></script>
<script src="/assets/grid-model.js"></script>
<script src="/assets/grid-style.js"></script>
<script src="/assets/grid-input.js"></script>
<script src="/assets/grid-buffer.js"></script>
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
 * holds the component, every input message it sent, the sizes it
 * reported, the selections it copied, the links it reported clicked and
 * any late protocol error; `harness.withhold` is what `linkPress` answers.
 * Both bundled terminal font weights have loaded before this returns, as
 * they do before an application terminal mounts. Otherwise bold fixture
 * runs can still use fallback glyphs when layout assertions measure them.
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
    await Promise.all([
      document.fonts.load('14px "JetBrains Mono"'),
      document.fonts.load('700 14px "JetBrains Mono"'),
    ]);
    const sent: unknown[] = [];
    const resizes: unknown[] = [];
    const copies: string[] = [];
    const links: unknown[] = [];
    const errors: string[] = [];
    const w = window as any;
    const grid = w.farhelmGridTerminal.create(document.getElementById("term"), {
      fontSize: 14,
      fontFamily: '"JetBrains Mono", monospace',
      theme: w.farhelmTerminalTheme,
      platform: navigator.platform,
      send: (message: unknown) => sent.push(message),
      onResize: (cols: number, rows: number) => resizes.push({ cols, rows }),
      onCopy: (text: string) => copies.push(text),
      // The component's own links (hyperlinks and printed URLs), recording
      // clicks; terminal.js's provider adds today's displays and checks.
      links: {
        row: (abs: number, done: (found: unknown[]) => void) => done(w.harness.grid.rowLinks(abs).map((found: any) => ({
          ...found,
          web: true,
          decorations: { underline: true, pointerCursor: true },
          hover() {},
          leave() {},
          activate: () => links.push({ kind: found.kind, uri: found.uri }),
        }))),
      },
      linkPress: () => w.harness.withhold,
      onProtocolError: (error: Error) => errors.push(error.message),
      sanitizePaste: (text: string) => text.replace(/\x1b/g, ""),
    });
    grid.fit();
    const encoder = (window as any).farhelmGridTestEncoder;
    w.harness = {
      grid,
      sent,
      resizes,
      copies,
      links,
      errors,
      withhold: false,
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

/** Whether this browser is a Mac by the grid's exact platform list (grid-input.js's `platformOf`). */
async function viewerIsMac(page: Page): Promise<boolean> {
  return page.evaluate(() => ["Macintosh", "MacIntel", "MacPPC", "Mac68K"].includes(navigator.platform));
}

/**
 * What this browser's key messages carry of the `mac` flag (the format's
 * `key` bullet): `mac: true` on a Mac, and nothing elsewhere, since an
 * absent `mac` means false and that is how `farhelm-proto` encodes it (its
 * golden fixture `key.json`).
 */
async function macField(page: Page): Promise<{ mac?: true }> {
  return (await viewerIsMac(page)) ? { mac: true } : {};
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
  // Bold green draws bright green (#B9CA4A), as in old terminals.
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
  //
  // The measurement alone cannot be what keeps characters outside the
  // flowing families boxed: a character can measure exactly one cell wide
  // alone and still shape against its neighbors in a span, which is why
  // flow is limited to box drawing, blocks and braille at all. So an
  // accented Latin letter, mapped to the bundled face at its own size so
  // that it measures exactly the cell width, must still be a box: that
  // assertion fails if the range limit in `flows` is removed, where the
  // Arabic cells need not: nothing makes their fallback glyph measure
  // exactly a cell wide alone, so the measurement may box them by itself.
  await openGrid(page);
  await page.addStyleTag({
    content: [
      '@font-face { font-family: "JetBrains Mono"; src: url("/assets/JetBrainsMonoNerdFont-Regular.woff2") format("woff2"); unicode-range: U+2591; size-adjust: 150%; }',
      '@font-face { font-family: "JetBrains Mono"; src: url("/assets/JetBrainsMonoNerdFont-Regular.woff2") format("woff2"); unicode-range: U+00E9; }',
    ].join("\n"),
  });
  const premise = await page.evaluate(async () => {
    await document.fonts.load('14px "JetBrains Mono"', "\u2591\u00e9");
    const probe = document.createElement("span");
    probe.style.cssText = 'font: 14px "JetBrains Mono"; white-space: pre; position: absolute;';
    document.body.appendChild(probe);
    probe.textContent = "W";
    const w = probe.getBoundingClientRect().width;
    probe.textContent = "\u2591";
    const shade = probe.getBoundingClientRect().width;
    probe.textContent = "\u00e9";
    const acute = probe.getBoundingClientRect().width;
    probe.remove();
    return { w, shade, acute };
  });
  // The fixture premises: the mapped block element is wider than a cell,
  // and the accented letter is exactly a cell wide (within the tolerance
  // `flows` itself allows), so only the range limit can box it.
  expect(premise.shade).toBeGreaterThan(premise.w * 1.2);
  expect(Math.abs(premise.acute - premise.w)).toBeLessThan(0.01);
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
            [11, 1, "\u00e9", plain],
            [12, 2, "gh", plain],
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
  // The accented letter measures exactly a cell wide and is still a box of
  // its own: outside the flowing families, the measurement never decides.
  const acute = first.spans.find((span) => span.text === "\u00e9");
  expect(acute, "the accented letter is a span of its own, not flowing with its neighbors").toBeDefined();
  expect(acute!.box).toBe(true);
  expect(acute!.left).toBeCloseTo(11 * width, 0);
  // Arabic cells are boxes.
  expect(second.spans.filter((span) => span.text === "\u06fb").every((span) => span.box)).toBe(true);
  // Every ASCII letter in its column.
  const columns = { a: 3, b: 4, c: 6, d: 7, e: 9, f: 10, g: 12, h: 13 };
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

test("typing sends key messages for keys with a sequence and text messages for typed characters", async ({ page }) => {
  const mac = await macField(page);
  // Keys with a sequence of their own go as `key` messages for the
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
    { t: "key", key: "a", code: "KeyA", kc: 65, mods: 0, ...mac },
    { t: "text", data: "A" },
    { t: "text", data: " " },
    { t: "key", key: "Enter", code: "Enter", kc: 13, mods: 1, ...mac },
    { t: "key", key: "c", code: "KeyC", kc: 67, mods: 4, ...mac },
    { t: "key", key: "ArrowUp", code: "ArrowUp", kc: 38, mods: 0, ...mac },
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
  // DECSET 1004 is answered with the state at that moment, so a
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
  // The drag leaves the terminal and is released outside it; the grid
  // follows both on the document and reports them at its edge.
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
    // Outside the grid, clamped to its last cell.
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

test("a right click under mouse reporting keeps the browser's context menu", async ({ page }) => {
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
  // text size must shrink it.
  await openGrid(page);
  const first = await page.evaluate(() => (window as any).harness.resizes.slice());
  expect(first).toHaveLength(1);
  await page.evaluate(() => (window as any).harness.grid.setFontSize(20));
  const resizes = await page.evaluate(() => (window as any).harness.resizes.slice());
  expect(resizes).toHaveLength(2);
  expect(resizes[1].cols).toBeLessThan(resizes[0].cols);
  expect(resizes[1].rows).toBeLessThan(resizes[0].rows);
});

// ## Tier 2: scrollback, selection and copy, links, paste

const PLAIN = [0, -1, -1];

/** A wire row of one plain ASCII run at column 0 (`wrapped`: a soft wrap). */
function textRow(text: string, wrapped = false) {
  return { w: wrapped, r: text ? [[0, text.length, text, PLAIN]] : [] };
}

/**
 * An attach of a `cols` x `rows` terminal whose screen rows read
 * "screen <y>" and whose `lines` history lines read "line <n>", numbered
 * from 0, as the supervisor sends it: hello, state, screen, history.
 */
function attachWithHistory(cols: number, rows: number, lines: number, extraState: Record<string, unknown> = {}): string[] {
  const screen = Array.from({ length: rows }, (_, y) => [y, textRow(`screen ${y}`)]);
  const out = [
    JSON.stringify({ t: "hello", cols, rows, history_cap: 1000 }),
    state(extraState),
    JSON.stringify({ t: "rows", rows: screen }),
    JSON.stringify({ t: "frame", rev: 1, cols, rows, cursor: { x: 0, y: rows - 1, visible: true, shape: "block", blink: true } }),
  ];
  if (lines) {
    out.push(JSON.stringify({ t: "history", first: 0, cols, rows: Array.from({ length: lines }, (_, n) => textRow(`line ${n}`)) }));
  }
  return out;
}

/** The visible rows' text, trimmed. */
async function visibleRows(page: Page): Promise<string[]> {
  return page.evaluate(() => [...document.querySelectorAll(".grid-terminal-row")].map((row) => row.textContent!.trimEnd()));
}

async function viewport(page: Page): Promise<{ top: number; total: number; rows: number; following: boolean }> {
  return page.evaluate(() => (window as any).harness.grid.viewport());
}

/** Wait until queued history has been applied and drawn. */
async function settled(page: Page): Promise<void> {
  await page.waitForFunction(() => !(window as any).harness.grid.snapshot().historyPending);
  await page.evaluate(() => (window as any).harness.grid.flush());
}

/**
 * The page position in cell (`x`, visible row `y`), `fx` of the way across
 * it (the middle by default). A selection's ends move to the nearest cell
 * boundary, and exactly mid-cell to the left one (old terminals' rounding);
 * a drag ending exactly mid-cell would still leave the rounding to sub-pixel
 * layout, so the drags below end clearly to one side, and the midpoint
 * itself is tested where its position is a whole pixel.
 */
async function cellAt(page: Page, x: number, y: number, fx = 0.5): Promise<{ x: number; y: number }> {
  const { width, height } = await cell(page);
  const screen = (await page.locator(".grid-terminal-screen").boundingBox())!;
  return { x: screen.x + (x + fx) * width, y: screen.y + (y + 0.5) * height };
}

test("the wheel scrolls back through history, which stays put while output arrives, and typing returns to the bottom", async ({ page }) => {
  const mac = await macField(page);
  // Scrollback is the client's own view over the history it holds. A
  // scrolled-back view is anchored to its lines, as an old terminal's
  // is, so new output does not drag it along; any input sent to the
  // program brings it back to the bottom (`scrollOnUserInput`).
  await openGrid(page);
  await feed(page, attachWithHistory(20, 4, 30));
  await settled(page);
  expect(await visibleRows(page)).toEqual(["screen 0", "screen 1", "screen 2", "screen 3"]);
  expect(await viewport(page)).toEqual({ top: 30, total: 34, rows: 4, following: true });
  const at = await cellAt(page, 2, 1);
  await page.mouse.move(at.x, at.y);
  // One notch (120 wheel units in Chromium's normalization, 50 pixels) is
  // about three lines at this text size; scroll well back.
  for (let i = 0; i < 4; i++) await page.mouse.wheel(0, -100);
  await expect.poll(async () => (await viewport(page)).following).toBe(false);
  // Wheel events can still be arriving, and rows are drawn at the next
  // animation frame: wait until two reads agree on the top and the rows
  // show it.
  let previous = -1;
  await expect.poll(async () => {
    const top = (await viewport(page)).top;
    const settledTop = top === previous && (await visibleRows(page))[0] === `line ${top}`;
    previous = top;
    return settledTop;
  }).toBe(true);
  const back = await viewport(page);
  expect(back.top).toBeLessThan(30);
  const shown = await visibleRows(page);
  // Output scrolls the screen by one line: the new history line arrives,
  // then the frame. The view keeps showing the same lines.
  await feed(page, [
    JSON.stringify({ t: "history", first: 30, cols: 20, rows: [textRow("screen 0")] }),
    JSON.stringify({ t: "rows", rows: [[3, textRow("screen 4")]] }),
    JSON.stringify({ t: "frame", rev: 2, cols: 20, rows: 4, shift: 1, cursor: { x: 0, y: 3, visible: true, shape: "block", blink: true } }),
  ]);
  await settled(page);
  expect(await visibleRows(page)).toEqual(shown);
  // Typing goes to the program and returns the view to the bottom.
  await page.evaluate(() => (window as any).harness.grid.focus());
  await expect(page.locator(".grid-terminal-input")).toBeFocused();
  await page.keyboard.press("a");
  await expect.poll(() => sent(page)).toEqual([{ t: "key", key: "a", code: "KeyA", kc: 65, mods: 0, ...mac }]);
  await expect.poll(() => visibleRows(page)).toEqual(["screen 1", "screen 2", "screen 3", "screen 4"]);
  expect((await viewport(page)).following).toBe(true);
});

test("Shift+PageUp and PageDown scroll by a page without reaching the program, and the scrollbar drags", async ({ page }) => {
  await openGrid(page);
  await feed(page, attachWithHistory(20, 5, 40));
  await settled(page);
  await page.evaluate(() => (window as any).harness.grid.focus());
  await page.keyboard.press("Shift+PageUp");
  // A page is the rows less one.
  await expect.poll(async () => (await viewport(page)).top).toBe(40 - 4);
  await page.keyboard.press("Shift+PageUp");
  await expect.poll(async () => (await viewport(page)).top).toBe(40 - 8);
  await page.keyboard.press("Shift+PageDown");
  await expect.poll(async () => (await viewport(page)).top).toBe(40 - 4);
  expect(await sent(page)).toEqual([]);
  // The slider, shown while the pointer is over the terminal: dragging it
  // to the top shows the oldest line.
  const terminal = (await page.locator(".grid-terminal").boundingBox())!;
  await page.mouse.move(terminal.x + 20, terminal.y + 20);
  const slider = page.locator(".grid-terminal-slider");
  await expect(page.locator(".grid-terminal-scrollbar")).toHaveCSS("opacity", "1");
  const box = (await slider.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2, terminal.y - 50);
  await page.mouse.up();
  await expect.poll(async () => (await viewport(page)).top).toBe(0);
  await expect.poll(async () => (await visibleRows(page))[0]).toBe("line 0");
  // Pressing the slider reached neither the program nor a selection.
  expect(await sent(page)).toEqual([]);
  expect(await page.evaluate(() => (window as any).harness.grid.hasSelection())).toBe(false);
});

test("the alternate screen has no history to scroll, and lost history is marked above the oldest line", async ({ page }) => {
  // A full-screen program's view is its own business (the alternate
  // screen has no scrollback); `history_trimmed` says recovery
  // lost some history, which the view says where the gap is.
  await openGrid(page);
  await feed(page, attachWithHistory(30, 3, 5, { history_trimmed: true }));
  await settled(page);
  expect(await viewport(page)).toEqual({ top: 6, total: 9, rows: 3, following: true });
  await page.evaluate(() => {
    const grid = (window as any).harness.grid;
    grid.scrollLines(-100);
    grid.flush();
  });
  await expect.poll(() => visibleRows(page)).toEqual(["Some earlier output could not be recovered", "line 0", "line 1"]);
  await feed(page, [state({ alt_screen: true, history_trimmed: true })]);
  await expect.poll(() => visibleRows(page)).toEqual(["screen 0", "screen 1", "screen 2"]);
  expect(await viewport(page)).toEqual({ top: 0, total: 3, rows: 3, following: true });
  await expect(page.locator(".grid-terminal-scrollbar")).toBeHidden();
});

/**
 * A selection pressed exactly in the middle of a cell starts at that cell's
 * left boundary, keeping its character, and a pixel further right starts
 * after it.
 *
 * Why: that is the grid's documented rounding, `ceil(x / width + 0.5) - 1`
 * (old terminals round the same way), and the grid once rounded the midpoint
 * right instead (`Math.round`), so a live drag starting 4px into an 8px cell
 * lost the first character it was aimed at. Spec: on a row
 * `abcdefghij`, a drag from the exact midpoint of a cell `k` to the row's end
 * copies the row from `k`'s character; from one pixel right of it, from the
 * next character. The midpoint is chosen where it falls on a whole pixel, so
 * the browser delivers it exactly.
 */
test("a selection pressed exactly mid-cell starts at that cell", async ({ page }) => {
  await openGrid(page);
  await feed(page, [
    JSON.stringify({ t: "hello", cols: 10, rows: 3, history_cap: 100 }),
    state({}),
    JSON.stringify({ t: "rows", rows: [[0, textRow("abcdefghij")], [1, textRow("")], [2, textRow("$ ")]] }),
    JSON.stringify({ t: "frame", rev: 1, cols: 10, rows: 3, cursor: { x: 2, y: 2, visible: true, shape: "block", blink: true } }),
  ]);
  await settled(page);
  const { width, height } = await cell(page);
  const screen = (await page.locator(".grid-terminal-screen").boundingBox())!;
  // Premise: a cell whose midpoint is a whole pixel, short of the last cell
  // so a pixel to its right is still inside the row.
  const k = [...Array(8).keys()].find((index) => {
    const x = screen.x + (index + 0.5) * width;
    return Math.abs(x - Math.round(x)) < 1e-6;
  });
  expect(k, `a cell of width ${width} at ${screen.x} must have a whole-pixel midpoint`).not.toBeUndefined();
  const mid = Math.round(screen.x + (k! + 0.5) * width);
  const y = screen.y + 0.5 * height;
  const end = await cellAt(page, 9, 0, 0.8);
  for (const startX of [mid, mid + 1]) {
    await page.mouse.move(startX, y);
    await page.mouse.down();
    await page.mouse.move(end.x, end.y, { steps: 4 });
    await page.mouse.up();
  }
  await expect.poll(() => page.evaluate(() => (window as any).harness.copies.slice())).toEqual([
    "abcdefghij".slice(k!),
    "abcdefghij".slice(k! + 1),
  ]);
});

test("a drag selects across history and screen, joins soft wraps, and copies every selection", async ({ page }) => {
  // Copy-on-select as today: a plain drag with mouse reporting off copies
  // when it ends, a wrapped row continues without a newline, and the same
  // selection made again is copied again.
  await openGrid(page);
  await feed(page, [
    JSON.stringify({ t: "hello", cols: 10, rows: 3, history_cap: 100 }),
    state({}),
    JSON.stringify({ t: "rows", rows: [[0, textRow("abcdefghij", true)], [1, textRow("klm")], [2, textRow("$ ")]] }),
    JSON.stringify({ t: "frame", rev: 1, cols: 10, rows: 3, cursor: { x: 2, y: 2, visible: true, shape: "block", blink: true } }),
    JSON.stringify({ t: "history", first: 0, cols: 10, rows: [textRow("old one"), textRow("old two")] }),
  ]);
  await settled(page);
  await page.evaluate(() => {
    const grid = (window as any).harness.grid;
    grid.scrollLines(-1);
    grid.flush();
  });
  expect(await visibleRows(page)).toEqual(["old two", "abcdefghij", "klm"]);
  const from = await cellAt(page, 0, 0);
  const to = await cellAt(page, 2, 2, 0.8);
  for (let pass = 0; pass < 2; pass++) {
    await page.mouse.move(from.x - 2, from.y);
    await page.mouse.down();
    await page.mouse.move(to.x, to.y, { steps: 4 });
    await page.mouse.up();
  }
  await expect.poll(() => page.evaluate(() => (window as any).harness.copies.slice())).toEqual([
    "old two\nabcdefghijklm",
    "old two\nabcdefghijklm",
  ]);
  // The selection is drawn, and nothing reached the program.
  await expect(page.locator(".grid-terminal-selection > div")).toHaveCount(3);
  expect(await sent(page)).toEqual([]);
  // A plain click clears it and copies nothing.
  await page.mouse.click(to.x, to.y);
  await expect(page.locator(".grid-terminal-selection > div")).toHaveCount(0);
  expect(await page.evaluate(() => (window as any).harness.copies.length)).toBe(2);
});

test("under mouse reporting a plain drag is the program's and the forcing modifier selects locally", async ({ page }) => {
  await openGrid(page);
  await feed(page, [...stream("attach.jsonl"), state({ mouse: "drag" })]);
  const from = await cellAt(page, 0, 0);
  const to = await cellAt(page, 5, 0, 0.2);
  await page.mouse.move(from.x - 2, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 3 });
  await page.mouse.up();
  const kinds = (await sent(page)).map((m: any) => m.kind);
  expect(kinds[0]).toBe("down");
  expect(kinds.at(-1)).toBe("up");
  expect(kinds.slice(1, -1).every((kind: string) => kind === "move")).toBe(true);
  expect(kinds.length).toBeGreaterThan(2);
  expect(await page.evaluate(() => (window as any).harness.copies.length)).toBe(0);
  const mac = await viewerIsMac(page);
  const before = (await sent(page)).length;
  const forcing = mac ? "Alt" : "Shift";
  await page.keyboard.down(forcing);
  await page.mouse.move(from.x - 2, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 3 });
  await page.mouse.up();
  await page.keyboard.up(forcing);
  await expect.poll(() => page.evaluate(() => (window as any).harness.copies.slice())).toEqual(["$ ech"]);
  expect((await sent(page)).length).toBe(before);
});

test("a double click selects a word, a triple click the wrapped line, and a drag past the edge scrolls", async ({ page }) => {
  await openGrid(page);
  await feed(page, [
    ...attachWithHistory(20, 3, 20).slice(0, 4),
    JSON.stringify({ t: "rows", rows: [[0, textRow("cat (foo.txt) and", false)], [1, textRow("a wrapped line that ", true)], [2, textRow("ends")]] }),
    JSON.stringify({ t: "frame", rev: 2, cols: 20, rows: 3, cursor: { x: 0, y: 2, visible: true, shape: "block", blink: true } }),
    JSON.stringify({ t: "history", first: 0, cols: 20, rows: Array.from({ length: 20 }, (_, n) => textRow(`line ${n}`)) }),
  ]);
  await settled(page);
  const word = await cellAt(page, 6, 0);
  await page.mouse.dblclick(word.x, word.y);
  await expect.poll(() => page.evaluate(() => (window as any).harness.copies.at(-1))).toBe("foo.txt");
  const line = await cellAt(page, 1, 2);
  await page.mouse.click(line.x, line.y, { clickCount: 3 });
  await expect.poll(() => page.evaluate(() => (window as any).harness.copies.at(-1))).toBe("a wrapped line that ends");
  // A drag held above the screen scrolls history in and selects it, from
  // the oldest line to where the drag started (cell boundary 3 of "ends").
  const start = await cellAt(page, 2, 2, 0.8);
  const screen = (await page.locator(".grid-terminal-screen").boundingBox())!;
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  await page.mouse.move(screen.x - 30, screen.y - 30, { steps: 3 });
  await expect.poll(async () => (await viewport(page)).top, { timeout: 10_000 }).toBe(0);
  await page.mouse.up();
  const copied = await page.evaluate(() => (window as any).harness.copies.at(-1));
  expect(copied.startsWith("line 0\nline 1\n")).toBe(true);
  expect(copied.endsWith("line 19\ncat (foo.txt) and\na wrapped line that end")).toBe(true);
});

test("a click on a link reports it; a drag across one never does; a withheld press reaches the program not at all", async ({ page }) => {
  // The click/drag distinction and the press withholding are the
  // component's; whether to open is terminal.js's (and covered there).
  await openGrid(page);
  await feed(page, [
    JSON.stringify({ t: "hello", cols: 40, rows: 2, history_cap: 0 }),
    state({}),
    JSON.stringify({
      t: "rows",
      rows: [[0, textRow("see https://example.com/x now")], [1, { w: false, r: [[0, 4, "docs", PLAIN, 0]] }]],
      links: [["d", "https://docs.example/"]],
    }),
    JSON.stringify({ t: "frame", rev: 1, cols: 40, rows: 2, cursor: { x: 0, y: 1, visible: false, shape: "block", blink: true } }),
  ]);
  const url = await cellAt(page, 8, 0);
  await page.mouse.move(url.x, url.y);
  // Hovering underlines the link and shows a hand.
  await expect(page.locator(".grid-terminal")).toHaveClass(/grid-terminal-link-hover/);
  await expect(page.locator(".grid-terminal-link-underline > div")).toHaveCount(1);
  await page.mouse.click(url.x, url.y);
  const osc = await cellAt(page, 1, 1);
  await page.mouse.click(osc.x, osc.y);
  const links = () => page.evaluate(() => (window as any).harness.links.slice());
  expect(await links()).toEqual([
    { kind: "text", uri: "https://example.com/x" },
    { kind: "osc", uri: "https://docs.example/" },
  ]);
  // A drag that starts and ends on the link, but moves away in between.
  await page.mouse.move(url.x, url.y);
  await page.mouse.down();
  await page.mouse.move(url.x + 40, url.y, { steps: 2 });
  await page.mouse.move(url.x, url.y, { steps: 2 });
  await page.mouse.up();
  expect((await links()).length).toBe(2);
  // Under mouse reporting a withheld press, its drag and release are never
  // reported, and the click still counts; a press elsewhere is reported.
  await feed(page, [state({ mouse: "drag" })]);
  await page.evaluate(() => ((window as any).harness.withhold = true));
  await page.mouse.click(url.x, url.y);
  expect((await links()).length).toBe(3);
  expect(await sent(page)).toEqual([]);
  const plain = await cellAt(page, 35, 0);
  await page.mouse.click(plain.x, plain.y);
  expect((await sent(page)).map((m: any) => m.kind)).toEqual(["down", "up"]);
});

test("a paste goes to the program as paste messages and brings the view back", async ({ page }) => {
  // Without terminal.js's attachment interception (covered with the
  // island) the component pastes plain text itself, as an old terminal's
  // own handler does. The supervisor adds bracketed-paste markers and converts
  // line endings; the client only splits at the format's bounds.
  await openGrid(page);
  await feed(page, attachWithHistory(20, 3, 10));
  await settled(page);
  await page.evaluate(() => {
    const grid = (window as any).harness.grid;
    grid.scrollLines(-5);
    grid.flush();
  });
  const big = "x".repeat(40 * 1024);
  await page.evaluate((text) => {
    const input = document.querySelector(".grid-terminal-input")!;
    const event = new Event("paste", { bubbles: true, cancelable: true });
    Object.defineProperty(event, "clipboardData", { value: { getData: (type: string) => (type === "text/plain" ? text : "") } });
    input.dispatchEvent(event);
  }, `line one\r\nline two\x1b[201~${big}`);
  const messages = await sent(page);
  expect(messages.map((m: any) => [m.t, m.last])).toEqual([["paste", false], ["paste", true]]);
  expect(messages.map((m: any) => m.data).join("")).toBe(`line one\r\nline two[201~${big}`);
  expect((await viewport(page)).following).toBe(true);
});

/**
 * Typing and pasting each clear the grid's selection, and leave no document
 * selection behind.
 *
 * Why: input is the user moving on from what they selected, so the
 * highlight must go (as it does in every terminal people use), and a
 * document selection left behind would be what the next copy or the
 * browser's own selection handling acts on. Both paths reach the program
 * through the grid's user-input path; this pins that it clears the
 * selection on each, which the old-terminal set's "input dismisses both the
 * xterm and native selections" pins for xterm.js.
 * Spec: with mouse reporting off, a plain drag across the screen's first row
 * selects it (premise: `hasSelection()` is true); a key typed into the
 * focused grid clears it, and the document selection is empty. A second
 * drag selects again (premise again), and a paste clears it the same way.
 * Both inputs reach the program.
 */
test("typing and pasting clear the selection and leave no document selection", async ({ page }) => {
  await openGrid(page);
  await feed(page, attachWithHistory(20, 3, 10));
  await settled(page);
  const select = async () => {
    const from = await cellAt(page, 0, 0, 0.2);
    const to = await cellAt(page, 4, 0, 0.8);
    await page.mouse.move(from.x, from.y);
    await page.mouse.down();
    await page.mouse.move(to.x, to.y, { steps: 3 });
    await page.mouse.up();
    expect(await page.evaluate(() => (window as any).harness.grid.hasSelection()), "premise: a selection").toBe(true);
  };
  const selections = () =>
    page.evaluate(() => ({
      grid: (window as any).harness.grid.hasSelection() as boolean,
      document: String(window.getSelection() ?? ""),
    }));

  await select();
  await page.evaluate(() => (window as any).harness.grid.focus());
  expect((await selections()).grid, "premise: focusing keeps the selection").toBe(true);
  await page.keyboard.press("a");
  await expect.poll(selections).toEqual({ grid: false, document: "" });

  await select();
  await page.evaluate(() => {
    const input = document.querySelector(".grid-terminal-input")!;
    const event = new Event("paste", { bubbles: true, cancelable: true });
    Object.defineProperty(event, "clipboardData", { value: { getData: (type: string) => (type === "text/plain" ? "pasted" : "") } });
    input.dispatchEvent(event);
  });
  await expect.poll(selections).toEqual({ grid: false, document: "" });
  expect((await sent(page)).map((m: any) => m.t)).toEqual(["key", "paste"]);
});

test("the context menu finds the input under the pointer, holding the selection", async ({ page }) => {
  // The input moves under the pointer, so the browser's own menu then
  // offers Paste into the terminal and Copy of the selection.
  await openGrid(page);
  await feed(page, stream("attach.jsonl"));
  await page.evaluate(() => (window as any).harness.grid.selectAll());
  const at = await cellAt(page, 3, 1);
  await page.evaluate(({ x, y }) => {
    document.querySelector(".grid-terminal-screen")!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: x, clientY: y }));
  }, at);
  const input = page.locator(".grid-terminal-input");
  await expect(input).toBeFocused();
  const box = (await input.boundingBox())!;
  expect(Math.abs(box.x + box.width / 2 - at.x)).toBeLessThan(2);
  expect(await input.inputValue()).toBe(await page.evaluate(() => (window as any).harness.grid.selectionText()));
  // The next key parks it again and empties it.
  await page.keyboard.press("Shift");
  expect(await input.inputValue()).toBe("");
});

test("a large history is applied in slices: the screen draws at once and the rest fills in", async ({ page }) => {
  // An attach sends the whole history after the screen. Applying it in
  // one task held the page for seconds at the 12,000-line worst case (a
  // true color per cell at 200 columns); the component applies one short
  // slice per message and the rest on later tasks, so the screen is drawn
  // before history is done. The messages are built in the page: 25 of
  // them, 120 such lines each, within the format's 1 MiB.
  await openGrid(page);
  const result = await page.evaluate(() => {
    const h = (window as any).harness;
    const encoder = (window as any).farhelmGridTestEncoder;
    const send = (message: unknown) => h.grid.receive(encoder.encode(JSON.stringify(message)));
    const plain = [0, -1, -1];
    send({ t: "hello", cols: 200, rows: 2, history_cap: 5000 });
    send({ t: "state", mouse: "off", focus: false, alt_screen: false, default_fg: null, default_bg: null, history_trimmed: false, exit: null });
    send({ t: "rows", rows: [[0, { w: false, r: [[0, 8, "screen 0", plain]] }], [1, { w: false, r: [[0, 8, "screen 1", plain]] }]] });
    send({ t: "frame", rev: 1, cols: 200, rows: 2, cursor: { x: 0, y: 1, visible: true, shape: "block", blink: true } });
    let seed = 7;
    const color = () => 0x1000000 + ((seed = (seed * 1103515245 + 12345) & 0xffffff));
    const row = () => ({ w: false, r: Array.from({ length: 200 }, (_, col) => [col, 1, "\u2580", [0, color(), color()]]) });
    let pendingAfterFirst = null;
    for (let first = 0; first < 3000; first += 120) {
      send({ t: "history", first, cols: 200, rows: Array.from({ length: 120 }, row) });
      if (pendingAfterFirst === null) pendingAfterFirst = h.grid.snapshot().historyPending;
    }
    h.grid.flush();
    return { pendingAfterFirst, rows: [...document.querySelectorAll(".grid-terminal-row")].map((r) => r.textContent!.trimEnd()) };
  });
  // One slice cannot apply 24,000 true-color cells; the rest waits.
  expect(result.pendingAfterFirst).toBe(true);
  expect(result.rows).toEqual(["screen 0", "screen 1"]);
  await page.waitForFunction(() => !(window as any).harness.grid.snapshot().historyPending, null, { timeout: 60_000 });
  expect(await page.evaluate(() => (window as any).harness.grid.model.historyLength)).toBe(3000);
});

test("a bad history row found after the frame that carried it is reported as a protocol error", async ({ page }) => {
  // The rows of a history message are checked as they are applied, which
  // may be after `receive` returned; the error then comes through
  // `onProtocolError`, which terminal.js treats as a bad frame.
  await openGrid(page);
  await feed(page, attachWithHistory(200, 2, 0));
  const outcome = await page.evaluate(() => {
    const h = (window as any).harness;
    const encoder = (window as any).farhelmGridTestEncoder;
    let seed = 3;
    const color = () => 0x1000000 + ((seed = (seed * 1103515245 + 12345) & 0xffffff));
    const rows: unknown[] = Array.from({ length: 120 }, () => ({ w: false, r: Array.from({ length: 200 }, (_, col) => [col, 1, "\u2580", [0, color(), color()]]) }));
    rows.push({ w: false, r: [[0, 1, "x", [0, 999, -1]]] });
    try {
      h.grid.receive(encoder.encode(JSON.stringify({ t: "history", first: 0, cols: 200, rows })));
      return "returned";
    } catch (error) {
      return `thrown: ${(error as Error).message}`;
    }
  });
  // The premise: one slice did not reach the bad row.
  expect(outcome).toBe("returned");
  await expect.poll(() => page.evaluate(() => (window as any).harness.errors.slice())).toEqual(["bad foreground color"]);
});

test("a frozen grid sends nothing but still selects and copies", async ({ page }) => {
  // A takeover's snapshot: the screen stays readable and copyable, and
  // nothing typed or clicked reaches anyone.
  await openGrid(page);
  await feed(page, [...stream("attach.jsonl"), state({ mouse: "click" })]);
  await page.evaluate(() => (window as any).harness.grid.freeze());
  await page.evaluate(() => (window as any).harness.grid.focus());
  await page.keyboard.press("a");
  const mac = await viewerIsMac(page);
  const forcing = mac ? "Alt" : "Shift";
  const from = await cellAt(page, 0, 0);
  const to = await cellAt(page, 5, 0, 0.2);
  await page.keyboard.down(forcing);
  await page.mouse.move(from.x - 2, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 2 });
  await page.mouse.up();
  await page.keyboard.up(forcing);
  await page.mouse.click(to.x, to.y);
  expect(await sent(page)).toEqual([]);
  expect(await page.evaluate(() => (window as any).harness.copies.slice())).toEqual(["$ ech"]);
});

// ## Round 2: scrolling, mouse and lookup regressions

test("a wheel step after the history cap dropped lines moves by the step alone", async ({ page }) => {
  // The view is anchored to its line; the wheel's own accumulator must not
  // keep a position measured before the oldest lines were dropped.
  await openGrid(page);
  const row = (n: number) => textRow(String(n).padStart(2, "0"));
  await feed(page, [
    JSON.stringify({ t: "hello", cols: 4, rows: 3, history_cap: 10 }),
    state({}),
    JSON.stringify({ t: "frame", rev: 1, cols: 4, rows: 3, cursor: { x: 0, y: 0, visible: false, shape: "block", blink: true } }),
    JSON.stringify({ t: "history", first: 0, cols: 4, rows: Array.from({ length: 10 }, (_, n) => row(n)) }),
  ]);
  await settled(page);
  await page.evaluate(() => {
    const grid = (window as any).harness.grid;
    grid.scrollLines(-5);
    grid.flush();
  });
  expect((await viewport(page)).top).toBe(5);
  await feed(page, [
    JSON.stringify({ t: "history", first: 10, cols: 4, rows: [row(10), row(11), row(12)] }),
    JSON.stringify({ t: "frame", rev: 2, cols: 4, rows: 3, shift: 3, cursor: { x: 0, y: 0, visible: false, shape: "block", blink: true } }),
  ]);
  await settled(page);
  // Three lines dropped at the cap: the same line is now at index 2.
  expect((await viewport(page)).top).toBe(2);
  expect((await visibleRows(page))[0]).toBe("05");
  // One wheel step of exactly one row down (50 px a notch).
  await page.evaluate(() => {
    const element = document.querySelector(".grid-terminal") as HTMLElement;
    const height = document.querySelector(".grid-terminal-screen")!.getBoundingClientRect().height / 3;
    const event = new Event("wheel", { bubbles: true, cancelable: true });
    Object.defineProperty(event, "wheelDeltaY", { value: (-120 * height) / 50 });
    element.dispatchEvent(event);
    (window as any).harness.grid.flush();
  });
  expect((await viewport(page)).top).toBe(3);
});

test("a mouse report brings a scrolled-back view to the bottom and clears the selection, as input does", async ({ page }) => {
  // A report is user input, which scrolls to the bottom and clears the
  // selection; the program then sees the click on the screen it shows.
  await openGrid(page);
  await feed(page, [...attachWithHistory(20, 3, 10), state({ mouse: "click" })]);
  await settled(page);
  // A forced local selection first.
  const forcing = (await viewerIsMac(page)) ? "Alt" : "Shift";
  const from = await cellAt(page, 0, 0, 0.2);
  const to = await cellAt(page, 4, 0, 0.2);
  await page.keyboard.down(forcing);
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 2 });
  await page.mouse.up();
  await page.keyboard.up(forcing);
  expect(await page.evaluate(() => (window as any).harness.grid.hasSelection())).toBe(true);
  await page.evaluate(() => {
    const grid = (window as any).harness.grid;
    grid.scrollLines(-4);
    grid.flush();
  });
  expect((await viewport(page)).following).toBe(false);
  const at = await cellAt(page, 2, 1);
  await page.mouse.click(at.x, at.y);
  await expect.poll(async () => (await viewport(page)).following).toBe(true);
  expect((await sent(page)).map((m: any) => m.kind)).toEqual(["down", "up"]);
  expect(await page.evaluate(() => (window as any).harness.grid.hasSelection())).toBe(false);
});

test("a quick Alt-click (Option on a Mac) moves the program's cursor with arrow keys", async ({ page }) => {
  // The arrows grid-input.js's moveToCellSequence makes
  // (here five cells right on the cursor's own row), sent as key messages.
  await openGrid(page);
  await feed(page, [
    JSON.stringify({ t: "hello", cols: 20, rows: 3, history_cap: 0 }),
    state({}),
    JSON.stringify({ t: "rows", rows: [[2, textRow("$ ls")]] }),
    JSON.stringify({ t: "frame", rev: 1, cols: 20, rows: 3, cursor: { x: 2, y: 2, visible: true, shape: "block", blink: true } }),
  ]);
  const mac = await macField(page);
  const at = await cellAt(page, 7, 2);
  await page.keyboard.down("Alt");
  await page.mouse.click(at.x, at.y);
  await page.keyboard.up("Alt");
  const arrow = { t: "key", key: "ArrowRight", code: "ArrowRight", kc: 39, mods: 0, ...mac };
  await expect.poll(() => sent(page)).toEqual([arrow, arrow, arrow, arrow, arrow]);
});

test("Alt-drag off a Mac selects a column, copied one line per row", async ({ page }) => {
  // A column selection; on a Mac Option forces a selection instead.
  test.skip(await viewerIsMac(page), "Option is the forcing modifier on a Mac");
  await openGrid(page);
  await feed(page, [
    JSON.stringify({ t: "hello", cols: 10, rows: 3, history_cap: 0 }),
    state({}),
    JSON.stringify({ t: "rows", rows: [[0, textRow("abcdefgh", true)], [1, textRow("ijklmnop")]] }),
    JSON.stringify({ t: "frame", rev: 1, cols: 10, rows: 3, cursor: { x: 0, y: 2, visible: true, shape: "block", blink: true } }),
  ]);
  const from = await cellAt(page, 2, 0, 0.2);
  const to = await cellAt(page, 5, 1, 0.2);
  await page.keyboard.down("Alt");
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 3 });
  await page.mouse.up();
  await page.keyboard.up("Alt");
  await expect.poll(() => page.evaluate(() => (window as any).harness.copies.slice())).toEqual(["cde\nklm"]);
  await expect(page.locator(".grid-terminal-selection > div")).toHaveCount(2);
  // A drag is not a click: the cursor stays where the program put it.
  expect(await sent(page)).toEqual([]);
});

test("history trimmed after falling behind keeps a scrolled-back view at the oldest line", async ({ page }) => {
  // More history than the cap arriving at once is trimmed by the client,
  // not reset by the supervisor: line numbers hold, so the view stays on
  // the oldest kept line instead of jumping to the bottom. (A selection is
  // not voided by the trim either, but every line a selection could have
  // been on is older than the cap's worth just received, so it goes with
  // its lines; the unit test pins that the trim is no
  // reset.)
  await openGrid(page);
  const row = (n: number) => textRow(`l${n}`);
  await feed(page, [
    JSON.stringify({ t: "hello", cols: 10, rows: 3, history_cap: 10 }),
    state({}),
    JSON.stringify({ t: "rows", rows: [[0, textRow("screen a")], [1, textRow("screen b")]] }),
    JSON.stringify({ t: "frame", rev: 1, cols: 10, rows: 3, cursor: { x: 0, y: 2, visible: false, shape: "block", blink: true } }),
    JSON.stringify({ t: "history", first: 0, cols: 10, rows: Array.from({ length: 10 }, (_, n) => row(n)) }),
  ]);
  await settled(page);
  await page.evaluate(() => {
    const grid = (window as any).harness.grid;
    grid.scrollLines(-100);
    grid.flush();
  });
  // The premise: scrolled to the top, not following.
  expect(await viewport(page)).toMatchObject({ top: 0, following: false });
  await feed(page, [JSON.stringify({ t: "history", first: 10, cols: 10, rows: Array.from({ length: 15 }, (_, n) => row(10 + n)) })]);
  await settled(page);
  const view = await viewport(page);
  expect(view.following).toBe(false);
  expect(view.top).toBe(0);
  expect((await visibleRows(page))[0]).toBe("l15");
});

test("history keeps being applied after the page is hidden", async ({ page }) => {
  // A slice waiting for an animation frame would wait for the tab to come
  // back; hiding the page moves it to a task. The test stops frames from
  // running, as a hidden page does, and reports the page hidden.
  await openGrid(page);
  const pendingWhileVisible = await page.evaluate(() => {
    const w = window as any;
    w.requestAnimationFrame = () => 0;
    w.cancelAnimationFrame = () => {};
    const encoder = (window as any).farhelmGridTestEncoder;
    const send = (message: unknown) => w.harness.grid.receive(encoder.encode(JSON.stringify(message)));
    send({ t: "hello", cols: 200, rows: 2, history_cap: 5000 });
    send({ t: "state", mouse: "off", focus: false, alt_screen: false, default_fg: null, default_bg: null, history_trimmed: false, exit: null });
    let seed = 5;
    const color = () => 0x1000000 + ((seed = (seed * 1103515245 + 12345) & 0xffffff));
    for (let first = 0; first < 1200; first += 120) {
      send({ t: "history", first, cols: 200, rows: Array.from({ length: 120 }, () => ({ w: false, r: Array.from({ length: 200 }, (_, col) => [col, 1, "▀", [0, color(), color()]]) })) });
    }
    return w.harness.grid.snapshot().historyPending;
  });
  // The premise: frames do not run, so the backlog is stuck while visible.
  expect(pendingWhileVisible).toBe(true);
  await page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "hidden" });
    document.dispatchEvent(new Event("visibilitychange", { bubbles: true }));
  });
  // Polled on a timer: this page runs no animation frames now, which is
  // what the default polling waits on.
  await page.waitForFunction(() => !(window as any).harness.grid.snapshot().historyPending, null, { timeout: 60_000, polling: 100 });
  expect(await page.evaluate(() => (window as any).harness.grid.model.historyLength)).toBe(1200);
});

test("redraws ask for links never with nothing hovered, and once per change to a hovered link's rows", async ({ page }) => {
  // The grid re-asks on a render only for a hovered link whose rows were
  // rendered; with nothing hovered it waits for the next move. A spinner
  // redrawn every frame under a resting pointer therefore costs nothing,
  // and a hovered link is asked for again exactly once per change to its
  // rows (in a container, one host check), never more.
  await openGrid(page);
  await page.evaluate(() => {
    const w = window as any;
    w.harness.grid.dispose();
    w.lookups = 0;
    w.offerLink = false;
    w.harness.grid = w.farhelmGridTerminal.create(document.getElementById("term"), {
      fontSize: 14,
      fontFamily: '"JetBrains Mono", monospace',
      theme: w.farhelmTerminalTheme,
      platform: navigator.platform,
      send: () => {},
      links: {
        row: (abs: number, done: (links: unknown[]) => void) => {
          w.lookups++;
          done(w.offerLink ? [{
            kind: "test",
            web: false,
            ranges: [{ start: { abs, col: 0 }, end: { abs, col: 6 } }],
            decorations: { underline: true, pointerCursor: true },
            hover() {},
            leave() {},
            activate() {},
          }] : []);
        },
      },
    });
    w.harness.grid.fit();
  });
  /** Send `texts` to row `y`, one frame each, painting after each. */
  const redraw = (y: number, texts: string[], from: number) => page.evaluate(async ({ y, texts, from }) => {
    const grid = (window as any).harness.grid;
    const encoder = (window as any).farhelmGridTestEncoder;
    let rev = from;
    for (const text of texts) {
      grid.receive(encoder.encode(JSON.stringify({ t: "rows", rows: [[y, { w: false, r: [[0, text.length, text, [0, -1, -1]]] }]] })));
      grid.receive(encoder.encode(JSON.stringify({ t: "frame", rev: rev++, cols: 20, rows: 2, cursor: { x: 0, y: 1, visible: false, shape: "block", blink: true } })));
      grid.flush();
      await new Promise((done) => requestAnimationFrame(done));
    }
  }, { y, texts, from });
  await page.evaluate((lines) => {
    const grid = (window as any).harness.grid;
    const encoder = (window as any).farhelmGridTestEncoder;
    for (const line of lines) grid.receive(encoder.encode(line));
    grid.flush();
  }, [
    JSON.stringify({ t: "hello", cols: 20, rows: 2, history_cap: 0 }),
    state({}),
    JSON.stringify({ t: "rows", rows: [[0, textRow("spin |")]] }),
    JSON.stringify({ t: "frame", rev: 1, cols: 20, rows: 2, cursor: { x: 0, y: 1, visible: false, shape: "block", blink: true } }),
  ]);
  const at = await cellAt(page, 2, 0);
  await page.mouse.move(at.x, at.y);
  await expect.poll(() => page.evaluate(() => (window as any).lookups)).toBe(1);
  // Nothing hovered: a spinner redrawn every frame asks for nothing.
  await redraw(0, Array.from({ length: 30 }, (_, i) => `spin ${"|/-\\"[i % 4]}`), 2);
  expect(await page.evaluate(() => (window as any).lookups)).toBe(1);
  // A link appears; the next move finds it.
  await page.evaluate(() => ((window as any).offerLink = true));
  await page.mouse.move(at.x + 1, at.y);
  await expect.poll(() => page.evaluate(() => (window as any).lookups)).toBe(2);
  // Its row redrawn ten times: ten fresh asks, one each.
  await redraw(0, Array(10).fill("spin |"), 40);
  expect(await page.evaluate(() => (window as any).lookups)).toBe(12);
  // Another row redrawn: nothing.
  await redraw(1, Array(10).fill("other"), 60);
  expect(await page.evaluate(() => (window as any).lookups)).toBe(12);
});

test("a column selection whose first lines drop at the cap keeps its columns", async ({ page }) => {
  // The trim moves only the row of a selection's start; a column
  // selection must not widen to column 0.
  test.skip(await viewerIsMac(page), "Option is the forcing modifier on a Mac");
  await openGrid(page);
  await feed(page, [
    JSON.stringify({ t: "hello", cols: 8, rows: 3, history_cap: 3 }),
    state({}),
    JSON.stringify({ t: "frame", rev: 1, cols: 8, rows: 3, cursor: { x: 0, y: 2, visible: false, shape: "block", blink: true } }),
    JSON.stringify({ t: "history", first: 0, cols: 8, rows: [textRow("abcdefgh"), textRow("ijklmnop"), textRow("qrstuvwx")] }),
  ]);
  await settled(page);
  await page.evaluate(() => {
    const grid = (window as any).harness.grid;
    grid.scrollLines(-100);
    grid.flush();
  });
  expect(await visibleRows(page)).toEqual(["abcdefgh", "ijklmnop", "qrstuvwx"]);
  const from = await cellAt(page, 2, 0, 0.2);
  const to = await cellAt(page, 5, 2, 0.2);
  await page.keyboard.down("Alt");
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 3 });
  await page.mouse.up();
  await page.keyboard.up("Alt");
  expect(await page.evaluate(() => (window as any).harness.grid.selectionText())).toBe("cde\nklm\nstu");
  // One more line: the first selected line drops.
  await feed(page, [JSON.stringify({ t: "history", first: 3, cols: 8, rows: [textRow("yz")] })]);
  await settled(page);
  expect(await page.evaluate(() => (window as any).harness.grid.selectionText())).toBe("klm\nstu");
});

// ## Round 4: late link answers

/**
 * Replace the harness grid with one whose provider holds every answer until
 * the test releases it (`window.answer()`), as a container's host checks
 * do, offering a link over columns 0-5 of the asked line. `window.hovers`
 * counts the hovers started.
 */
async function gridWithHeldAnswers(page: Page, rows: number, history: string[]): Promise<void> {
  await page.evaluate(({ rows, history }) => {
    const w = window as any;
    w.harness.grid.dispose();
    w.hovers = [];
    w.pending = [];
    w.answer = () => {
      const held = w.pending;
      w.pending = [];
      for (const release of held) release();
    };
    const grid = w.farhelmGridTerminal.create(document.getElementById("term"), {
      fontSize: 14,
      fontFamily: '"JetBrains Mono", monospace',
      theme: w.farhelmTerminalTheme,
      platform: navigator.platform,
      send: () => {},
      links: {
        row: (abs: number, done: (links: unknown[]) => void) => {
          w.pending.push(() => done([{
            kind: "test",
            web: false,
            ranges: [{ start: { abs, col: 0 }, end: { abs, col: 6 } }],
            decorations: { underline: true, pointerCursor: true },
            hover: () => w.hovers.push(abs),
            leave() {},
            activate() {},
          }]));
        },
      },
    });
    w.harness.grid = grid;
    grid.fit();
    const encoder = (window as any).farhelmGridTestEncoder;
    const send = (message: unknown) => grid.receive(encoder.encode(JSON.stringify(message)));
    const plain = [0, -1, -1];
    send({ t: "hello", cols: 20, rows, history_cap: 100 });
    send({ t: "state", mouse: "off", focus: false, alt_screen: false, default_fg: null, default_bg: null, history_trimmed: false, exit: null });
    send({ t: "rows", rows: Array.from({ length: rows }, (_, y) => [y, { w: false, r: [[0, 8, `screen ${y}`, plain]] }]) });
    send({ t: "frame", rev: 1, cols: 20, rows, cursor: { x: 0, y: rows - 1, visible: false, shape: "block", blink: true } });
    if (history.length) send({ t: "history", first: 0, cols: 20, rows: history.map((text) => ({ w: false, r: [[0, text.length, text, plain]] })) });
    grid.flush();
  }, { rows, history });
}

/** Send a frame (and rows, if any) to the harness grid and paint it. */
async function frame(page: Page, rev: number, rows: [number, string][], rowsInView = 3): Promise<void> {
  await page.evaluate(({ rev, rows, rowsInView }) => {
    const grid = (window as any).harness.grid;
    const encoder = (window as any).farhelmGridTestEncoder;
    const plain = [0, -1, -1];
    if (rows.length) grid.receive(encoder.encode(JSON.stringify({ t: "rows", rows: rows.map(([y, text]) => [y, { w: false, r: [[0, text.length, text, plain]] }]) })));
    grid.receive(encoder.encode(JSON.stringify({ t: "frame", rev, cols: 20, rows: rowsInView, cursor: { x: 3, y: 2, visible: true, shape: "block", blink: true } })));
    grid.flush();
  }, { rev, rows, rowsInView });
}

/**
 * A freshly decoded history row is not a redraw of that line. Both a
 * provider's delayed answer and an established hover must survive reads
 * and output elsewhere, or links would disappear while a terminal runs.
 */
test("a history link keeps its answer and hover across lazy reads and unrelated output", async ({ page }) => {
  await openGrid(page);
  await gridWithHeldAnswers(page, 3, ["history link", "second line", "third line"]);
  const premise = await page.evaluate(() => {
    const grid = (window as any).harness.grid;
    grid.scrollLines(-3);
    grid.flush();
    const first = grid.model.historyLine(0);
    const second = grid.model.historyLine(0);
    return { fresh: first !== second, text: first.runs[0].text, first: grid.model.historyFirst };
  });
  expect(premise).toEqual({ fresh: true, text: "history link", first: 0 });
  expect(await viewport(page)).toMatchObject({ following: false, top: 0 });
  const at = await cellAt(page, 2, 0);
  await page.mouse.move(at.x, at.y);
  await expect.poll(() => page.evaluate(() => (window as any).pending.length)).toBe(1);
  await frame(page, 2, []);
  await page.evaluate(() => (window as any).answer());
  await expect.poll(() => page.evaluate(() => (window as any).hovers.slice())).toEqual([0]);
  await frame(page, 3, [[2, "busy..."]]);
  await page.mouse.move(at.x + 1, at.y);
  expect(await page.evaluate(() => ({
    hovers: (window as any).hovers.slice(), pending: (window as any).pending.length,
  }))).toEqual({ hovers: [0], pending: 0 });
});

/**
 * A reset can reuse the same absolute line number and text. Only the
 * stored line's identity distinguishes the old permission check from a
 * fresh question about the replacement, with the pointer restored to it.
 */
test("a late history link answer is refused after a same-number same-text reset", async ({ page }) => {
  await openGrid(page);
  const history = ["history link", "second line", "third line"];
  await gridWithHeldAnswers(page, 3, history);
  await page.evaluate(() => {
    const w = window as any;
    w.harness.grid.scrollLines(-3);
    w.harness.grid.flush();
    w.oldHistoryIdentity = w.harness.grid.model.historyIdentity(0);
  });
  const at = await cellAt(page, 2, 0);
  await page.mouse.move(at.x, at.y);
  await expect.poll(() => page.evaluate(() => (window as any).pending.length)).toBe(1);
  const replacement = await page.evaluate((history) => {
    const w = window as any;
    const grid = w.harness.grid;
    const send = (message: unknown) => grid.receive(w.farhelmGridTestEncoder.encode(message));
    send({ t: "history_reset" });
    send({ t: "history", first: 0, cols: 20, rows: history.map((text) => ({ w: false, r: [[0, text.length, text, [0, -1, -1]]] })) });
    grid.flush();
    grid.scrollLines(-3);
    grid.flush();
    return {
      changed: w.oldHistoryIdentity !== grid.model.historyIdentity(0),
      first: grid.model.historyFirst, text: grid.model.historyLine(0).runs[0].text,
      pending: w.pending.length,
    };
  }, history);
  expect(replacement).toEqual({ changed: true, first: 0, text: "history link", pending: 1 });
  expect(await viewport(page)).toMatchObject({ following: false, top: 0 });
  await page.evaluate(() => (window as any).answer());
  expect(await page.evaluate(() => (window as any).hovers.slice())).toEqual([]);
  // A new question over the replacement still works; rejecting the old
  // answer must not merely have broken link lookup or lost the pointer.
  await page.mouse.move(at.x, at.y + 3 * (await cell(page)).height);
  await page.mouse.move(at.x, at.y);
  await expect.poll(() => page.evaluate(() => (window as any).pending.length)).toBe(1);
  await page.evaluate(() => (window as any).answer());
  await expect.poll(() => page.evaluate(() => (window as any).hovers.slice())).toEqual([0]);
});

test("a late link answer still hovers after output that did not touch its line", async ({ page }) => {
  // A container's link answers wait for host checks; a program writing
  // meanwhile (a cursor-only frame, another row) must not void them, or no
  // link appears while an agent is working.
  await openGrid(page);
  await gridWithHeldAnswers(page, 3, []);
  const at = await cellAt(page, 2, 0);
  await page.mouse.move(at.x, at.y);
  await expect.poll(() => page.evaluate(() => (window as any).pending.length)).toBe(1);
  await frame(page, 2, []);
  await frame(page, 3, [[2, "busy..."]]);
  await page.evaluate(() => (window as any).answer());
  await expect.poll(() => page.evaluate(() => (window as any).hovers.length)).toBe(1);
  // The control: an answer whose own line was replaced meanwhile is not used.
  // (Any output drops the line's cached answer, so the move back asks.)
  await page.mouse.move(at.x, at.y + 3 * (await cell(page)).height);
  await frame(page, 4, []);
  await page.mouse.move(at.x, at.y);
  await expect.poll(() => page.evaluate(() => (window as any).pending.length)).toBe(1);
  await frame(page, 5, [[0, "screen 0"]]);
  await page.evaluate(() => (window as any).answer());
  expect(await page.evaluate(() => (window as any).hovers.length)).toBe(1);
});

test("a late link answer is not used for a line the view scrolled away from the pointer", async ({ page }) => {
  // Scrolling while the answer is pending puts another line under the
  // pointer; hovering the asked line would show its display beside text it
  // is not about.
  await openGrid(page);
  await gridWithHeldAnswers(page, 3, ["h0", "h1", "h2"]);
  const at = await cellAt(page, 2, 1);
  await page.mouse.move(at.x, at.y);
  await expect.poll(() => page.evaluate(() => (window as any).pending.length)).toBe(1);
  await page.evaluate(() => {
    const grid = (window as any).harness.grid;
    grid.scrollLines(-1);
    grid.flush();
  });
  expect((await viewport(page)).following).toBe(false);
  await page.evaluate(() => (window as any).answer());
  expect(await page.evaluate(() => (window as any).hovers.slice())).toEqual([]);
});

test("a late link answer is dropped when a row of a long printed URL beyond the path detector's reach changes", async ({ page }) => {
  // On narrow rows a printed URL can span more rows than the path detector
  // reads (128); the URL found for a pending answer depends on all of them,
  // so replacing row 129 of a 132-row URL must void the answer.
  await openGrid(page);
  await gridWithHeldAnswers(page, 3, []);
  const premise = await page.evaluate(() => {
    const grid = (window as any).harness.grid;
    const encoder = (window as any).farhelmGridTestEncoder;
    const send = (message: unknown) => grid.receive(encoder.encode(JSON.stringify(message)));
    const plain = [0, -1, -1];
    const url = "https://example.com/" + "a".repeat(1298);
    const rows: unknown[] = [];
    for (let at = 0; at < url.length; at += 10) {
      const text = url.slice(at, at + 10);
      rows.push([rows.length, { w: at + 10 < url.length, r: [[0, text.length, text, plain]] }]);
    }
    send({ t: "rows", rows });
    send({ t: "frame", rev: 2, cols: 10, rows: rows.length, cursor: { x: 0, y: 0, visible: false, shape: "block", blink: true } });
    grid.flush();
    return rows.length;
  });
  expect(premise).toBe(132);
  const at = await cellAt(page, 3, 0);
  await page.mouse.move(at.x, at.y);
  await expect.poll(() => page.evaluate(() => (window as any).pending.length)).toBe(1);
  await page.evaluate(() => {
    const grid = (window as any).harness.grid;
    const encoder = (window as any).farhelmGridTestEncoder;
    grid.receive(encoder.encode(JSON.stringify({ t: "rows", rows: [[129, { w: true, r: [[0, 10, "bbbbbbbbbb", [0, -1, -1]]] }]] })));
    grid.receive(encoder.encode(JSON.stringify({ t: "frame", rev: 3, cols: 10, rows: 132, cursor: { x: 0, y: 0, visible: false, shape: "block", blink: true } })));
    grid.flush();
  });
  await page.evaluate(() => (window as any).answer());
  expect(await page.evaluate(() => (window as any).hovers.slice())).toEqual([]);
});
