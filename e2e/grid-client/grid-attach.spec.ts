// terminal.js's attach plumbing for grid streams, driven against a fake
// socket on a static page.
//
// An island cannot know which component its terminal needs until the helm
// says so on the socket, so every mount builds xterm.js as it always has
// and a `{"type":"stream","kind":"grid"}` notice swaps the grid component
// in. These specs pin the parts of that which only the real terminal.js can
// show: the socket asks for grid streams, a grid notice replaces the
// surface and keeps the catch-up and its reveal, input leaves as binary
// frames, the text-size step resizes a grid terminal, a stream with no
// notice (an older helm) stays on xterm.js, and a grid frame that breaks
// the format ends the attachment visibly. The fake socket stands in for
// the helm, which does not send grid streams yet.
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const ASSETS = join(__dirname, "../../crates/farhelm-ui/assets");
const FIXTURES = join(__dirname, "../../crates/farhelm-ui/js-tests/fixtures/grid");
const ORIGIN = "https://grid.test";

// Records the document's live listeners, installed before any other
// script, so a spec can tell whether the swap left xterm.js's page-level
// listeners (copy-on-select, the link gesture observers) behind. Each entry
// is "type|capture|id", the id naming the listener function.
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

// The quick switcher's chord listener, as lib.rs installs it (an inline
// eval, not an asset): read from the source so the spec runs the shipped
// code rather than a copy.
const QUICK_SWITCHER = (() => {
  const source = readFileSync(join(__dirname, "../../crates/farhelm-ui/src/lib.rs"), "utf8");
  const at = source.indexOf("fn install_quick_switcher_shortcut()");
  const open = source.indexOf('r#"', at);
  const close = source.indexOf('"#', open);
  if (at < 0 || open < 0 || close < 0) throw new Error("lib.rs no longer installs the quick switcher's chord inline");
  return source.slice(open + 3, close);
})();

// The template shortcuts' definition as lib.rs serializes
// `launch_sequence::TEMPLATE_SHORTCUTS` for a browser build (the same
// mirror js-tests/template-shortcuts.test.js uses).
const TEMPLATE_SHORTCUTS = (() => {
  const modifiers = (source: string) => ({
    source,
    mac: { meta: true, ctrl: false, shift: false, alt: false },
    other: { meta: false, ctrl: true, shift: true, alt: false },
  });
  return {
    mac_desktop: false,
    bolt: { code: "KeyP", modifiers: modifiers("platform") },
    number: { keys: Array.from("1234567890", (value) => ({ code: `Digit${value}`, value })), modifiers: modifiers("mac_desktop_build") },
    prefix: { code: "KeyX", modifiers: modifiers("mac_desktop_build") },
    template_list: { code: "KeyJ", modifiers: modifiers("platform") },
  };
})();

// Every script terminal.js's mount gate waits for, in lib.rs's order.
const SCRIPTS = [
  "vendor/xterm.js",
  "vendor/addon-fit.js",
  "vendor/addon-unicode-graphemes.js",
  "vendor/addon-clipboard.js",
  "vendor/addon-web-links.js",
  "term-bytes.js",
  "terminal-theme.js",
  "clipboard-name.js",
  "shift-enter-key.js",
  "copy-on-select.js",
  "clipboard-writer.js",
  "terminal-links.js",
  "terminal-files.js",
  "tooltip.js",
  "grid-model.js",
  "grid-style.js",
  "grid-input.js",
  "grid-terminal.js",
];

// A socket the test drives: it records what the page sends and lets the
// test deliver frames and open or close it. Installed before terminal.js
// runs, so every island's `new WebSocket` gets one.
const FAKE_SOCKET = `
class FakeSocket {
  constructor(url) {
    this.url = url;
    this.readyState = FakeSocket.CONNECTING;
    this.sent = [];
    this.binaryType = "blob";
    FakeSocket.all.push(this);
    // A spec that times the catch-up opens the socket as soon as it exists,
    // as a helm does, rather than a test round trip later.
    if (window.openSocketsAtOnce) setTimeout(() => this.open(), 0);
  }
  send(data) {
    this.sent.push(typeof data === "string" ? data : { binary: new TextDecoder().decode(data) });
  }
  close() {
    if (this.readyState === FakeSocket.CLOSED) return;
    this.readyState = FakeSocket.CLOSED;
    if (this.onclose) this.onclose({});
  }
  open() {
    this.readyState = FakeSocket.OPEN;
    if (this.onopen) this.onopen({});
  }
  text(message) {
    this.onmessage({ data: JSON.stringify(message) });
  }
  binary(text) {
    this.onmessage({ data: new TextEncoder().encode(text).buffer });
  }
}
FakeSocket.CONNECTING = 0;
FakeSocket.OPEN = 1;
FakeSocket.CLOSING = 2;
FakeSocket.CLOSED = 3;
FakeSocket.all = [];
window.FakeSocket = FakeSocket;
window.WebSocket = FakeSocket;
`;

const PAGE = `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<link rel="stylesheet" href="/assets/vendor/xterm.css">
<link rel="stylesheet" href="/assets/app.css">
<script>${LISTENER_LEDGER}</script>
<script>${FAKE_SOCKET}</script>
${SCRIPTS.map((name) => `<script src="/assets/${name}"></script>`).join("\n")}
<script src="/assets/terminal.js"></script>
<script src="/assets/session-action-shortcuts.js"></script>
<script>window.installSessionActionShortcuts(false);</script>
<script src="/assets/template-shortcuts.js"></script>
<script>window.installTemplateShortcuts(${JSON.stringify(TEMPLATE_SHORTCUTS)});</script>
<script>${QUICK_SWITCHER}</script>
</head>
<body style="margin: 0">
<button class="session-rename-trigger" onclick="window.renames = (window.renames || 0) + 1">Rename</button>
<button class="quick-switcher-trigger" onclick="window.switchers = (window.switchers || 0) + 1">Switch</button>
<button class="quick-switcher-template-trigger" onclick="window.templateLists = (window.templateLists || 0) + 1">Templates</button>
<div class="terminal-panes" style="position: relative; width: 820px; height: 420px">
  <div class="terminal-pane selected">
    <div id="banner" class="banner"></div>
    <div id="status" class="attach-status"></div>
    <div id="connecting" class="terminal-connecting"></div>
    <div id="term" class="terminal"></div>
  </div>
</div>
</body>
</html>`;

function stream(name: string): string[] {
  return readFileSync(join(FIXTURES, name), "utf8").split("\n").filter((line) => line.trim() !== "");
}

/**
 * Open the page and mount the agent terminal the way a session view does,
 * returning once its socket exists. No reconnect policy is passed, so a
 * failed attachment shows its banner instead of retrying.
 */
async function mountTerminal(page: Page, replay?: Record<string, unknown>): Promise<void> {
  // A mount that throws reports it on a timer's stack, not to the test;
  // surface it so a failure says why.
  page.on("pageerror", (error) => console.log(`page error: ${error.stack ?? error}`));
  await page.route(`${ORIGIN}/**`, async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/") return route.fulfill({ contentType: "text/html", body: PAGE });
    const name = path.replace(/^\/assets\//, "");
    if (!/^(vendor\/)?[A-Za-z0-9.-]+$/.test(name)) return route.abort();
    const file = name.endsWith(".woff2") ? join(ASSETS, "fonts", name) : join(ASSETS, name);
    return route.fulfill({ path: file });
  });
  await page.goto(`${ORIGIN}/`);
  // Test controls for the catch-up (terminal.js's `replayControls`).
  if (replay) await page.evaluate((controls) => ((window as any).__farhelmTestReplay = controls), replay);
  await page.evaluate(() => {
    // The page's own document listeners, before any terminal exists.
    (window as any).listenersBeforeMount = (window as any).documentListeners();
    (window as any).farhelmTerm.mountWhenReady(
      { el: "term", banner: "banner", status: "status", connecting: "connecting", notice: null, copyHint: null, path: "/api/sessions/s1/term", gen: 1, primary: true, focus: true },
      "https://grid.test",
      undefined,
      false,
    );
  });
  await page.waitForFunction(() => (window as any).FakeSocket.all.length === 1);
}

/** Call a method of the island's fake socket in the page. */
async function socket(page: Page, method: string, ...args: unknown[]): Promise<void> {
  await page.evaluate(([name, rest]) => {
    const ws = (window as any).FakeSocket.all[0];
    ws[name as string](...(rest as unknown[]));
  }, [method, args] as const);
}

async function sentFrames(page: Page): Promise<any[]> {
  return page.evaluate(() => (window as any).FakeSocket.all[0].sent.slice());
}

test("a grid notice swaps in the grid component and keeps the catch-up, input and resize", async ({ page }) => {
  await mountTerminal(page);
  const url = await page.evaluate(() => (window as any).FakeSocket.all[0].url as string);
  expect(new URL(url).searchParams.get("grid")).toBe("1");
  await socket(page, "open");
  await socket(page, "text", { type: "stream", kind: "grid" });
  await page.waitForFunction(() => (window as any).__farhelmIslands.term.kind === "grid");
  // The xterm.js surface is gone; the grid's is in the same element.
  await expect(page.locator("#term .xterm")).toHaveCount(0);
  await expect(page.locator("#term .grid-terminal")).toHaveCount(1);
  expect(await page.evaluate(() => (window as any).__farhelmIslands.term.test.stream)).toBe("grid");
  // The catch-up still hides the terminal until the marker, then reveals
  // the current screen.
  for (const line of stream("attach.jsonl")) await socket(page, "binary", line);
  await expect(page.locator("#term")).toHaveCSS("visibility", "hidden");
  await socket(page, "text", { type: "replay_complete" });
  await page.waitForFunction(() => (window as any).__farhelmTest.replay.revealed);
  await expect(page.locator("#term")).toHaveCSS("visibility", "visible");
  await expect(page.locator(".grid-terminal-row").nth(0)).toHaveText("$ echo");
  // Typing leaves as a binary frame carrying the input message.
  await page.evaluate(() => (window as any).__farhelmGrid.focus());
  await page.keyboard.press("Enter");
  await expect.poll(async () => (await sentFrames(page)).filter((frame) => typeof frame !== "string")).toEqual([
    { binary: JSON.stringify({ t: "key", key: "Enter", code: "Enter", kc: 13, mods: 0 }) },
  ]);
  // The program was only ever asked for the size the URL carried: once at
  // open, as for every attachment, and once more because the fixture's
  // hello (20 x 4) is not the size this pane fits. The swap itself sent
  // nothing.
  const carried = { cols: Number(new URL(url).searchParams.get("cols")), rows: Number(new URL(url).searchParams.get("rows")) };
  expect(await resizes(page)).toEqual([carried, carried]);
  // A larger text size refits the grid and tells the program, once.
  await page.evaluate(() => (window as any).farhelmTerm.stepFontSize(4));
  await expect.poll(async () => (await resizes(page)).length).toBe(3);
  expect((await resizes(page))[2].cols).toBeLessThan(carried.cols);
});

/** The resize control messages the page has sent. */
async function resizes(page: Page): Promise<{ cols: number; rows: number }[]> {
  return (await sentFrames(page))
    .filter((frame) => typeof frame === "string" && JSON.parse(frame).type === "resize")
    .map((frame) => {
      const { cols, rows } = JSON.parse(frame);
      return { cols, rows };
    });
}

test("a program's clipboard write reaches the system clipboard once whole", async ({ page }) => {
  // OSC 52 writes are honored for grid terminals as for xterm.js ones: the
  // system clipboard only, through the page's one clipboard queue, and only
  // when the last part has arrived.
  await page.addInitScript(() => {
    const w = window as any;
    w.copied = [];
    w.__farhelmNativeClipboardWrite = (text: string) => w.copied.push(text);
  });
  await mountTerminal(page);
  await socket(page, "open");
  await socket(page, "text", { type: "stream", kind: "grid" });
  for (const line of stream("attach.jsonl")) await socket(page, "binary", line);
  await socket(page, "binary", JSON.stringify({ t: "clipboard", target: "clipboard", text: "copied ", last: false }));
  expect(await page.evaluate(() => (window as any).copied)).toEqual([]);
  await socket(page, "binary", JSON.stringify({ t: "clipboard", target: "clipboard", text: "text", last: true }));
  await socket(page, "binary", JSON.stringify({ t: "clipboard", target: "primary", text: "selection", last: true }));
  await expect.poll(() => page.evaluate(() => (window as any).copied)).toEqual(["copied text"]);
});

test("page shortcuts pressed in a grid terminal act and never reach the program", async ({ page }) => {
  // The text-size keys and the session actions are window capture
  // listeners, the same ones that keep them from xterm.js; a grid terminal
  // must get no new handlers and leak no key messages.
  await mountTerminal(page);
  await socket(page, "open");
  await socket(page, "text", { type: "stream", kind: "grid" });
  for (const line of stream("attach.jsonl")) await socket(page, "binary", line);
  await socket(page, "text", { type: "replay_complete" });
  await page.waitForFunction(() => (window as any).__farhelmTest.replay.revealed);
  await page.evaluate(() => (window as any).__farhelmGrid.focus());
  // The premise: the keys go to the grid's input, so a leak would show.
  const input = page.locator("#term .grid-terminal-input");
  await expect(input).toBeFocused();
  const resizesBefore = (await resizes(page)).length;
  const mac = await page.evaluate(() => /Mac/.test(navigator.platform));
  // Text size is Cmd+Shift on a Mac; the switcher chords are Cmd alone.
  const textSize = mac ? "Meta+Shift" : "Control+Shift";
  const platform = mac ? "Meta" : "Control+Shift";
  await page.keyboard.press(`${textSize}+Equal`);
  await expect.poll(async () => (await resizes(page)).length).toBe(resizesBefore + 1);
  await page.keyboard.press("Control+Shift+KeyL");
  await expect.poll(() => page.evaluate(() => (window as any).renames)).toBe(1);
  // The quick switcher (Cmd+K on a Mac, Ctrl+Shift+K elsewhere) and the
  // template list (Cmd+J, Ctrl+Shift+J).
  await page.keyboard.press(`${platform}+KeyK`);
  await expect.poll(() => page.evaluate(() => (window as any).switchers)).toBe(1);
  await page.keyboard.press(`${platform}+KeyJ`);
  await expect.poll(() => page.evaluate(() => (window as any).templateLists)).toBe(1);
  await expect(input).toBeFocused();
  // An ordinary key afterwards reaches the program as exactly one frame:
  // the input path was live throughout, so the empty list above means the
  // shortcuts were kept from it, not that nothing could have been sent.
  await page.keyboard.press("a");
  await expect.poll(async () => (await sentFrames(page)).filter((frame) => typeof frame !== "string")).toEqual([
    { binary: JSON.stringify({ t: "key", key: "a", code: "KeyA", kc: 65, mods: 0 }) },
  ]);
});

test("the swap removes every page-level listener the xterm.js surface installed", async ({ page }) => {
  // Copy-on-select and the link gesture observers listen on the document;
  // left behind, they would act on a disposed terminal on every click.
  await mountTerminal(page);
  const beforeMount = await page.evaluate(() => (window as any).listenersBeforeMount);
  await page.waitForFunction(() => !!(window as any).__farhelmIslands?.term);
  const mounted = await page.evaluate(() => (window as any).documentListeners());
  expect(mounted.length).toBeGreaterThan(beforeMount.length);
  await socket(page, "open");
  await socket(page, "text", { type: "stream", kind: "grid" });
  await page.waitForFunction(() => (window as any).__farhelmIslands.term.kind === "grid");
  expect(await page.evaluate(() => (window as any).documentListeners())).toEqual(beforeMount);
});

test("a grid notice after the idle reveal keeps keyboard focus in the terminal", async ({ page }) => {
  // A silent attach is revealed by the catch-up's idle bound and may have
  // been focused before the notice arrives; disposing xterm.js must hand
  // that focus to the grid rather than drop it on the page.
  await page.addInitScript(() => ((window as any).openSocketsAtOnce = true));
  await mountTerminal(page, { idleMs: 300 });
  await page.waitForFunction(() => (window as any).__farhelmTest.replay.revealed);
  expect(await page.evaluate(() => (window as any).__farhelmTest.replay.revealReason)).toBe("idle");
  await page.evaluate(() => (window as any).__farhelmTerm.focus());
  await expect(page.locator("#term .xterm-helper-textarea")).toBeFocused();
  await socket(page, "text", { type: "stream", kind: "grid" });
  await expect(page.locator("#term .grid-terminal-input")).toBeFocused();
});

test("text frames before the notice settle nothing; the notice still decides", async ({ page }) => {
  // The ordering contract: the notice precedes data and `replay_complete`;
  // a heartbeat answer may come first.
  await mountTerminal(page);
  await socket(page, "open");
  await socket(page, "text", { type: "pong" });
  expect(await page.evaluate(() => (window as any).__farhelmIslands.term.test.stream)).toBeNull();
  await socket(page, "text", { type: "stream", kind: "grid" });
  await page.waitForFunction(() => (window as any).__farhelmIslands.term.kind === "grid");
});

test("an older helm whose first frame is the end of an empty replay stays on xterm.js", async ({ page }) => {
  await mountTerminal(page);
  await socket(page, "open");
  await socket(page, "text", { type: "replay_complete" });
  await page.waitForFunction(() => (window as any).__farhelmTest.replay.revealed);
  expect(await page.evaluate(() => (window as any).__farhelmIslands.term.test.stream)).toBe("bytes");
  await socket(page, "binary", "live bytes");
  await expect
    .poll(() => page.evaluate(() => (window as any).__farhelmTerm.buffer.active.getLine(0).translateToString(true)))
    .toBe("live bytes");
});

test("a grid notice after byte data is a broken contract and ends the attachment", async ({ page }) => {
  await mountTerminal(page);
  await socket(page, "open");
  await socket(page, "binary", "some bytes");
  await socket(page, "text", { type: "stream", kind: "grid" });
  expect(await page.evaluate(() => (window as any).FakeSocket.all[0].readyState)).toBe(3);
  await expect(page.locator("#banner")).toHaveText("Terminal stream error: the stream kind changed mid-attachment");
});

test("a stream with no notice stays on xterm.js, as from an older helm", async ({ page }) => {
  await mountTerminal(page);
  await socket(page, "open");
  await socket(page, "binary", "hello from bytes");
  await socket(page, "text", { type: "replay_complete" });
  await page.waitForFunction(() => (window as any).__farhelmTest.replay.revealed);
  expect(await page.evaluate(() => (window as any).__farhelmIslands.term.test.stream)).toBe("bytes");
  await expect(page.locator("#term .xterm")).toHaveCount(1);
  await expect(page.locator("#term .grid-terminal")).toHaveCount(0);
  await expect
    .poll(() => page.evaluate(() => (window as any).__farhelmTerm.buffer.active.getLine(0).translateToString(true)))
    .toBe("hello from bytes");
});

test("a grid frame that breaks the format ends the attachment with a visible error", async ({ page }) => {
  // The format's answer to a bad message is a protocol error and a
  // reconnect; with no reconnect policy (as here) the banner says why.
  await mountTerminal(page);
  await socket(page, "open");
  await socket(page, "text", { type: "stream", kind: "grid" });
  await socket(page, "binary", stream("attach.jsonl")[0]);
  await socket(page, "binary", '{"t":"rows","rows":[[0,{"w":false,"r":[[0,1,"x",[0,999,-1]]]}]]}');
  expect(await page.evaluate(() => (window as any).FakeSocket.all[0].readyState)).toBe(3);
  await expect(page.locator("#banner")).toHaveText("Terminal stream error: bad foreground color");
  expect(await page.evaluate(() => (window as any).__farhelmTest.replay.revealReason)).toBe("protocol");
});

test("a stream kind this page cannot draw is refused the same way", async ({ page }) => {
  await mountTerminal(page);
  await socket(page, "open");
  await socket(page, "text", { type: "stream", kind: "pixels" });
  expect(await page.evaluate(() => (window as any).FakeSocket.all[0].readyState)).toBe(3);
  await expect(page.locator("#banner")).toContainText("Terminal stream error");
});
