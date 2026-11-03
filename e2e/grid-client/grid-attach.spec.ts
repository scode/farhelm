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
  "grid-buffer.js",
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
async function mountTerminal(
  page: Page,
  replay?: Record<string, unknown>,
  { attach, reconnect, spec: extra }: { attach?: unknown; reconnect?: unknown; spec?: Record<string, unknown> } = {},
): Promise<void> {
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
  await page.evaluate(({ attach, reconnect, extra }) => {
    // The page's own document listeners, before any terminal exists.
    (window as any).listenersBeforeMount = (window as any).documentListeners();
    const spec = { el: "term", banner: "banner", status: "status", connecting: "connecting", notice: null, copyHint: null, path: "/api/sessions/s1/term", gen: 1, primary: true, focus: true, unavailable: false, sessionId: "s1", ...extra };
    // With a reconnect policy the island is mounted the way a session view
    // mounts it, through `sync`, which is what hands the policy over.
    if (reconnect) (window as any).farhelmTerm.sync("https://grid.test", [spec], attach, reconnect);
    else (window as any).farhelmTerm.mountWhenReady(spec, "https://grid.test", attach, false);
  }, { attach, reconnect, extra: extra || {} });
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

/** Whether this browser is a Mac by xterm.js's exact platform list. */
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

test("a grid notice swaps in the grid component and keeps the catch-up, input and resize", async ({ page }) => {
  const mac = await macField(page);
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
    { binary: JSON.stringify({ t: "key", key: "Enter", code: "Enter", kc: 13, mods: 0, ...mac }) },
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
  const mac = await viewerIsMac(page);
  const macKey = await macField(page);
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
    { binary: JSON.stringify({ t: "key", key: "a", code: "KeyA", kc: 65, mods: 0, ...macKey }) },
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

// ## Tier 2: paste, drop, copy, links and connection behavior

// `attachments::attachment_policy`'s shape, with this spec's own wording.
// The classification table is the Rust one: file beats image beats text.
const ATTACH_POLICY = {
  upload: "/api/sessions/s1/attachments",
  build: "",
  buildHeader: "x-farhelm-build",
  classify: ["none", "text", "image", "image", "file", "file", "file", "file"],
  extensionAliases: {},
  maxExtensionLength: 8,
  fallbackExtension: "bin",
  namePrefix: "pasted-",
  placeholderStem: "image",
  placeholderMaxAgeMs: 0,
  separator: " ",
  safePathChars: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789/._-",
  text: {
    busyOne: "attaching {name}",
    busyMany: "attaching {count} files",
    failed: "{name} failed: {reason}",
    directory: "{name} is a directory",
    omittedImage: "{name} omitted",
    unreadable: "{name} unreadable",
    detached: "not connected",
    landed: "{name} landed at {path}",
    interruptedUnsent: "{name} interrupted",
    interruptedUnknown: "{name} maybe interrupted",
    httpStatus: "HTTP {status}",
    noPath: "no path",
    skew: "skew {helm}",
    skewSilent: "skew",
  },
};

// A reconnect policy (`reconnect::reconnect_policy`'s shape) whose ladder
// never fires on its own within a test: attempts are made with
// `reconnectNow`.
const RECONNECT_POLICY = {
  delaysMs: [600_000],
  probeIntervalMs: 600_000,
  auto: true,
  heartbeat: null,
  text: { attempting: "attempting", waiting: "waiting", probing: "probing", probingWaiting: "probing later", manualOnly: "manual", manual: "Reconnect now" },
};

/** Open the island's socket `index`, make it a grid stream and replay the attach fixture. */
async function attachGrid(page: Page, index = 0): Promise<void> {
  await page.evaluate(([i, lines]) => {
    const ws = (window as any).FakeSocket.all[i as number];
    ws.open();
    ws.text({ type: "stream", kind: "grid" });
    for (const line of lines as string[]) ws.binary(line);
    ws.text({ type: "replay_complete" });
  }, [index, stream("attach.jsonl")] as const);
  await page.waitForFunction(() => (window as any).__farhelmTest?.replay.revealed);
}

/** The input messages (binary frames) socket `index` has carried. */
async function inputMessages(page: Page, index = 0): Promise<any[]> {
  return page.evaluate((i) =>
    (window as any).FakeSocket.all[i].sent.filter((frame: any) => typeof frame !== "string").map((frame: any) => JSON.parse(frame.binary)), index);
}

/** Dispatch a paste or drop carrying `text` and `files` at `selector`, as terminal-attachments.spec.ts does. */
async function dispatchPayload(page: Page, selector: string, kind: "paste" | "drop", text: string, files: { name: string; content: string }[] = []) {
  return page.evaluate(({ selector, kind, text, files }) => {
    const made = files.map((entry) => new File([entry.content], entry.name, { type: "text/plain" }));
    const data = {
      items: made.map((file) => ({ kind: "file", type: file.type, getAsFile: () => file, webkitGetAsEntry: () => null })),
      files: made,
      types: [...(made.length ? ["Files"] : []), ...(text ? ["text/plain"] : [])],
      dropEffect: "none",
      getData: (type: string) => (type === "text/plain" ? text : ""),
    };
    const event = new Event(kind, { bubbles: true, cancelable: true });
    Object.defineProperty(event, kind === "paste" ? "clipboardData" : "dataTransfer", { value: data });
    document.querySelector(selector)!.dispatchEvent(event);
    return event.defaultPrevented;
  }, { selector, kind, text, files });
}

/**
 * The page position in grid cell (`x`, `y`) of the pane's grid of `cols` x
 * `rows` (the attach fixture's 20 x 4 by default), `fx` of the way across
 * the cell (the middle by default; a drag's end goes to the nearest cell
 * boundary, so drags end clearly to one side of the middle). Read from the
 * DOM, so it works on a snapshot whose island is gone.
 */
async function gridCell(page: Page, x: number, y: number, cols = 20, rows = 4, fx = 0.5): Promise<{ x: number; y: number }> {
  return page.evaluate(([cx, cy, c, r, f]) => {
    const screen = document.querySelector("#term .grid-terminal-screen")!.getBoundingClientRect();
    return { x: screen.left + (cx + f) * (screen.width / c), y: screen.top + (cy + 0.5) * (screen.height / r) };
  }, [x, y, cols, rows, fx]);
}

test("a paste into a grid terminal goes through today's attachment interception as paste messages", async ({ page }) => {
  // Paste is the critical path: text is sanitized and pasted, and a copied
  // file is uploaded and its host path pasted, through the same
  // classification and status line as an xterm.js terminal's.
  await mountTerminal(page, undefined, { attach: ATTACH_POLICY });
  const uploads: string[] = [];
  await page.route(`${ORIGIN}/api/**`, (route) => {
    uploads.push(new URL(route.request().url()).searchParams.get("filename")!);
    return route.fulfill({ contentType: "application/json", body: JSON.stringify({ path: "/state/attachments/notes.txt" }) });
  });
  await attachGrid(page);
  expect(await dispatchPayload(page, "#term .grid-terminal-input", "paste", "echo hi\nmore\x1b[201~tail")).toBe(true);
  // terminal.js's sanitizing removes the embedded end-of-paste marker.
  await expect.poll(() => inputMessages(page)).toEqual([{ t: "paste", data: "echo hi\nmoretail", last: true }]);
  // A copied file wins over the text the clipboard also carries.
  await dispatchPayload(page, "#term .grid-terminal-input", "paste", "/elsewhere/notes.txt", [{ name: "notes.txt", content: "x" }]);
  await expect.poll(() => inputMessages(page)).toEqual([
    { t: "paste", data: "echo hi\nmoretail", last: true },
    { t: "paste", data: "/state/attachments/notes.txt ", last: true },
  ]);
  expect(uploads).toEqual(["notes.txt"]);
});

test("a drop on a grid terminal never navigates the page and says drops are not supported yet", async ({ page }) => {
  // Drag and drop into grid terminals is not built (the maintainer's call);
  // the drop must still be prevented, so the page cannot navigate to a
  // dropped file, and must not be lost silently or blamed on the connection.
  await mountTerminal(page, undefined, { attach: ATTACH_POLICY });
  await attachGrid(page);
  const before = page.url();
  expect(await dispatchPayload(page, "#term .grid-terminal", "drop", "", [{ name: "dropped.txt", content: "x" }])).toBe(true);
  await expect(page.locator("#status")).toHaveText("Dropping into this terminal is not supported yet; copy and paste instead");
  expect(page.url()).toBe(before);
  expect(await inputMessages(page)).toEqual([]);
});

test("a completed selection reaches the system clipboard through the page's queue", async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as any;
    w.copied = [];
    w.__farhelmNativeClipboardWrite = (text: string) => w.copied.push(text);
  });
  await mountTerminal(page);
  await attachGrid(page);
  const from = await gridCell(page, 0, 0);
  // Clearly past boundary 5, so the selection ends there in both engines.
  const to = await gridCell(page, 5, 0, 20, 4, 0.2);
  await page.mouse.move(from.x - 2, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 3 });
  await page.mouse.up();
  await expect.poll(() => page.evaluate(() => (window as any).copied)).toEqual(["$ ech"]);
});

/** A 30 x 4 screen with a text URL on row 0 and an OSC 8 link to `osc` on row 1. */
function linkScreen(osc: string, rev = 2): string[] {
  const plain = [0, -1, -1];
  const text = "go https://example.com/a ok";
  return [
    JSON.stringify({
      t: "rows",
      rows: [[0, { w: false, r: [[0, text.length, text, plain]] }], [1, { w: false, r: [[0, 4, "docs", plain, 0]] }]],
      links: [["d", osc]],
    }),
    JSON.stringify({ t: "frame", rev, cols: 30, rows: 4, cursor: { x: 0, y: 3, visible: false, shape: "block", blink: true } }),
  ];
}

test("links in a grid terminal open under today's rules", async ({ page }) => {
  // A text URL opens on click; an OSC 8 web link shows its target on hover
  // and opens; the link-click setting's "program only" opens nothing; a
  // non-web OSC target never opens.
  await page.addInitScript(() => {
    const w = window as any;
    w.opened = [];
    w.open = (url: string) => w.opened.push(url);
  });
  await mountTerminal(page);
  await attachGrid(page);
  await page.evaluate((lines) => {
    const ws = (window as any).FakeSocket.all[0];
    for (const line of lines) ws.binary(line);
  }, linkScreen("https://docs.example/x"));
  const opened = () => page.evaluate(() => (window as any).opened.slice());
  const url = await gridCell(page, 6, 0, 30);
  await page.mouse.click(url.x, url.y);
  await expect.poll(opened).toEqual(["https://example.com/a"]);
  const osc = await gridCell(page, 1, 1, 30);
  await page.mouse.move(osc.x, osc.y);
  await expect(page.locator("#term .terminal-link-target")).toContainText("docs.example");
  await page.mouse.click(osc.x, osc.y);
  await expect.poll(opened).toEqual(["https://example.com/a", "https://docs.example/x"]);
  await page.evaluate(() => ((window as any).farhelmTerminalLinkClickMode = "program"));
  await page.mouse.click(url.x, url.y);
  await page.evaluate(() => ((window as any).farhelmTerminalLinkClickMode = "intercept"));
  await page.evaluate((lines) => {
    const ws = (window as any).FakeSocket.all[0];
    for (const line of lines) ws.binary(line);
  }, linkScreen("javascript:alert(1)", 3));
  await page.mouse.click(osc.x, osc.y);
  expect(await opened()).toEqual(["https://example.com/a", "https://docs.example/x"]);
});

test("under mouse reporting a press on a web link is withheld from the program by default", async ({ page }) => {
  // SPEC.md: the press, its drag and its release go to Farhelm alone, so a
  // program that opens links itself cannot open a second tab; elsewhere the
  // program still gets its clicks.
  await page.addInitScript(() => {
    const w = window as any;
    w.opened = [];
    w.open = (url: string) => w.opened.push(url);
  });
  await mountTerminal(page);
  await attachGrid(page);
  await page.evaluate((lines) => {
    const ws = (window as any).FakeSocket.all[0];
    for (const line of lines) ws.binary(line);
    ws.binary(JSON.stringify({ t: "state", mouse: "click", focus: false, alt_screen: false, default_fg: null, default_bg: null, history_trimmed: false, exit: null }));
  }, linkScreen("https://docs.example/x"));
  const url = await gridCell(page, 6, 0, 30);
  await page.mouse.click(url.x, url.y);
  await expect.poll(() => page.evaluate(() => (window as any).opened.slice())).toEqual(["https://example.com/a"]);
  expect(await inputMessages(page)).toEqual([]);
  const plain = await gridCell(page, 25, 2, 30);
  await page.mouse.click(plain.x, plain.y);
  await expect.poll(async () => (await inputMessages(page)).map((m) => m.kind)).toEqual(["down", "up"]);
});

test("a dropped connection keeps the grid's last screen while it recovers, and a takeover keeps it as a usable snapshot", async ({ page }) => {
  // SPEC.md: a recovering terminal keeps showing what the user had, and a
  // client displaced by a takeover keeps a non-live snapshot with its
  // take-control action. For a grid terminal the snapshot is the frozen
  // grid itself: still selectable and copyable, sending nothing.
  await page.addInitScript(() => {
    const w = window as any;
    w.copied = [];
    w.__farhelmNativeClipboardWrite = (text: string) => w.copied.push(text);
  });
  await mountTerminal(page, undefined, { reconnect: RECONNECT_POLICY });
  await attachGrid(page);
  await socket(page, "close");
  await expect(page.locator("#connecting")).toHaveAttribute("data-reconnect-phase", "retrying");
  await page.evaluate(() => (window as any).farhelmTerm.reconnectNow("term"));
  await page.waitForFunction(() => (window as any).FakeSocket.all.length === 2);
  // The held screen sits beside the replacement mount while it attaches.
  const held = page.locator(".terminal-pane > .grid-terminal.terminal-snapshot");
  await expect(held).toHaveCount(1);
  await expect(held.locator(".grid-terminal-row").nth(0)).toHaveText("$ echo");
  // The new attachment is refused: another client holds the session.
  await page.evaluate(() => {
    const ws = (window as any).FakeSocket.all[1];
    ws.open();
    ws.text({ type: "stream", kind: "grid" });
    ws.text({ type: "detached", code: "taken_over", reason: "another client attached" });
  });
  await expect(page.locator("#banner")).toContainText("Detached: another client attached");
  await expect(page.locator("#banner .banner-reclaim")).toHaveCount(1);
  expect(await page.evaluate(() => (window as any).__farhelmTombstones)).toEqual(["term"]);
  const snapshot = page.locator("#term > .grid-terminal");
  await expect(snapshot).toHaveCount(1);
  await expect(snapshot).not.toHaveClass(/terminal-snapshot/);
  await expect(snapshot.locator(".grid-terminal-row").nth(0)).toHaveText("$ echo");
  // Selecting on the snapshot still copies.
  const from = await gridCell(page, 0, 0);
  // Clearly past boundary 5, so the selection ends there in both engines.
  const to = await gridCell(page, 5, 0, 20, 4, 0.2);
  await page.mouse.move(from.x - 2, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 3 });
  await page.mouse.up();
  await expect.poll(() => page.evaluate(() => (window as any).copied)).toEqual(["$ ech"]);
  // Take control attaches again from scratch: the snapshot goes, a new
  // socket opens, and the replay rebuilds the screen.
  await page.locator("#banner .banner-reclaim").click();
  await page.waitForFunction(() => (window as any).FakeSocket.all.length === 3);
  expect(await page.evaluate(() => (window as any).__farhelmTombstones)).toEqual([]);
  await attachGrid(page, 2);
  await expect(page.locator("#term .grid-terminal")).toHaveCount(1);
  await expect(page.locator("#term .grid-terminal-row").nth(0)).toHaveText("$ echo");
});

test("a recovered grid terminal replays into a new grid and drops the held screen", async ({ page }) => {
  const mac = await macField(page);
  await mountTerminal(page, undefined, { reconnect: RECONNECT_POLICY });
  await attachGrid(page);
  await socket(page, "close");
  await page.evaluate(() => (window as any).farhelmTerm.reconnectNow("term"));
  await page.waitForFunction(() => (window as any).FakeSocket.all.length === 2);
  await expect(page.locator(".grid-terminal.terminal-snapshot")).toHaveCount(1);
  await attachGrid(page, 1);
  await expect(page.locator(".grid-terminal.terminal-snapshot")).toHaveCount(0);
  await expect(page.locator(".grid-terminal")).toHaveCount(1);
  await expect(page.locator("#connecting")).not.toHaveAttribute("data-reconnect-phase", /.+/);
  // The new grid is live: typing reaches the new socket.
  await page.evaluate(() => (window as any).__farhelmGrid.focus());
  await page.keyboard.press("x");
  await expect.poll(() => inputMessages(page, 1)).toEqual([{ t: "key", key: "x", code: "KeyX", kc: 88, mods: 0, ...mac }]);
});

// ## Tier 3: the hyperlink mismatch warning, file paths, tunnels
//
// Today's link code (terminal-links.js, terminal-files.js) runs on the grid
// through an adapter of xterm.js's buffer API; these specs hold the grid to
// what that code shows and allows, with hostile link text and targets.

/**
 * Replace the screen of socket `index` with `rows` (each `[text, link]`,
 * `link` an index into `links`, or a full wire row object) at `cols` x 4, as
 * frame `rev`, and paint it. The paint matters to the specs that measure
 * cells next: a frame that changes the size lays the screen out anew only
 * when it is painted, and a cell measured before that is the old size's.
 */
async function showScreen(
  page: Page,
  rows: (string | [string, number] | object)[],
  { cols = 40, links, rev = 2, index = 0 }: { cols?: number; links?: [string, string][]; rev?: number; index?: number } = {},
): Promise<void> {
  const plain = [0, -1, -1];
  const wire = rows.map((row, y) => {
    if (typeof row === "string") return [y, { w: false, r: row ? [[0, row.length, row, plain]] : [] }];
    if (Array.isArray(row)) return [y, { w: false, r: [[0, row[0].length, row[0], plain, row[1]]] }];
    return [y, row];
  });
  await page.evaluate(([i, messages]) => {
    const ws = (window as any).FakeSocket.all[i as number];
    for (const message of messages as unknown[]) ws.binary(JSON.stringify(message));
    (window as any).__farhelmGrid.flush();
  }, [index, [
    { t: "rows", rows: wire, ...(links ? { links } : {}) },
    { t: "frame", rev, cols, rows: 4, cursor: { x: 0, y: 3, visible: false, shape: "block", blink: true } },
  ]] as const);
}

/** Record the session host's file requests and answer them from `answers` (by operation). */
async function routeFiles(page: Page, answers: Record<string, (body: any) => { status?: number; json?: unknown; body?: string }>) {
  const requests: { op: string; body: any }[] = [];
  await page.route(`${ORIGIN}/api/sessions/s1/files/**`, (route) => {
    const op = new URL(route.request().url()).pathname.split("/").pop()!;
    const body = JSON.parse(route.request().postData() || "{}");
    requests.push({ op, body });
    const answer = answers[op](body);
    return route.fulfill({
      status: answer.status ?? 200,
      contentType: answer.json === undefined ? "application/octet-stream" : "application/json",
      body: answer.json === undefined ? answer.body ?? "" : JSON.stringify(answer.json),
    });
  });
  return requests;
}

test("an OSC 8 link whose text is another address warns, judging the text across the link's rows", async ({ page }) => {
  // SPEC.md: hovering a program's hyperlink shows its exact target, and
  // turns into a warning when the underlined text is a web address that
  // goes elsewhere, compared across the rows of one link. A lookalike split
  // over two rows must not pass because each half is harmless alone.
  await page.addInitScript(() => {
    const w = window as any;
    w.opened = [];
    w.open = (url: string) => w.opened.push(url);
  });
  await mountTerminal(page);
  await attachGrid(page);
  const target = "https://evil.example/login";
  await showScreen(page, [["https://goo", 0], ["d.example/", 0], ["plain", 1]], { cols: 20, links: [["x", target], ["y", "https://plain.example/"]] });
  const first = await gridCell(page, 2, 0, 20);
  await page.mouse.move(first.x, first.y);
  const display = page.locator("#term .terminal-link-target");
  await expect(display).toHaveClass(/terminal-link-target-mismatch/);
  await expect(display).toContainText("Text says: https://good.example/");
  await expect(display).toContainText("evil.example");
  // The other link's text is no address: the ordinary display, host first.
  const plain = await gridCell(page, 1, 2, 20);
  await page.mouse.move(plain.x, plain.y);
  await expect(display).not.toHaveClass(/terminal-link-target-mismatch/);
  await expect(display).toContainText("plain.example");
  // The warning does not stop the click: the display is the safeguard.
  await page.mouse.move(first.x, first.y);
  await expect(display).toHaveClass(/terminal-link-target-mismatch/);
  await page.mouse.click(first.x, first.y);
  await expect.poll(() => page.evaluate(() => (window as any).opened.slice())).toEqual([target]);
});

test("hostile link text and targets stay data in the hover display", async ({ page }) => {
  // Program-controlled text and targets reach the page only as text, with
  // presentation-unsafe characters made visible, and a scheme outside the
  // allowlist shows nothing and opens nothing.
  await page.addInitScript(() => {
    const w = window as any;
    w.opened = [];
    w.open = (url: string) => w.opened.push(url);
  });
  await mountTerminal(page);
  await attachGrid(page);
  // The right-to-left override is a zero-width character, so it shares the
  // cell of the "/" before it (a non-ASCII run is one cell's content).
  const plainStyle = [0, -1, -1];
  await showScreen(page, [
    { w: false, r: [[0, 18, "https://ok.example", plainStyle, 0], [18, 1, "/\u202e", plainStyle, 0], [19, 1, "x", plainStyle, 0]] },
    ["<img src=x onerror=alert(1)>", 1],
  ], { cols: 40, links: [["a", "https://bad.example/"], ["b", "javascript:alert(document.domain)"]] });
  const spoof = await gridCell(page, 3, 0, 40);
  await page.mouse.move(spoof.x, spoof.y);
  const display = page.locator("#term .terminal-link-target");
  await expect(display).toContainText("<U+202E>");
  await expect(display).toHaveClass(/terminal-link-target-mismatch/);
  expect(await page.locator("#term img").count()).toBe(0);
  const script = await gridCell(page, 3, 1, 40);
  await page.mouse.move(script.x, script.y);
  await expect(display).toBeHidden();
  await page.mouse.click(script.x, script.y);
  expect(await page.evaluate(() => (window as any).opened.slice())).toEqual([]);
  expect(await page.locator("#term img").count()).toBe(0);
});

test("a printed path hovers with its host check and downloads on click", async ({ page }) => {
  // SPEC.md's file paths: hover underlines and checks the path on the
  // session's host (a spinner, then host, full path and size), and a click
  // downloads it. A browser save names the file by its basename.
  await mountTerminal(page);
  const requests = await routeFiles(page, {
    stat: (body) => ({ json: { status: "ready", path: `/home/agent${body.path.slice(1)}`, host: "build-host", size: 1234, can_open: true } }),
    download: () => ({ body: "file contents" }),
  });
  await attachGrid(page);
  await showScreen(page, ["see ./out/notes.txt done"], { cols: 40 });
  const path = await gridCell(page, 8, 0, 40);
  await page.mouse.move(path.x, path.y);
  const hover = page.locator("#term .terminal-file-target");
  await expect(hover).toContainText("notes.txt");
  await expect(hover).toContainText("build-host:/home/agent/out/");
  await expect(hover).toContainText("1.2 KB");
  // A browser offers no opening, only the download.
  await expect(hover).toContainText("click to download");
  await expect(hover).not.toContainText("click to open");
  await expect(page.locator("#term .grid-terminal")).toHaveClass(/grid-terminal-link-hover/);
  await expect(page.locator("#term .grid-terminal-link-underline > div")).toHaveCount(1);
  await page.mouse.click(path.x, path.y);
  await expect(page.locator("#term .terminal-file-message")).toHaveText("Downloaded notes.txt");
  expect(requests.map((r) => [r.op, r.body.path])).toEqual([["stat", "./out/notes.txt"], ["download", "/home/agent/out/notes.txt"]]);
});

test("a redraw that changes the hovered path withdraws its click; the same text redrawn keeps it", async ({ page }) => {
  // SPEC.md: only a successful check over link text unchanged since that
  // hover admits a click, and a redraw of the same path at the same place
  // is not a new hover. The grid's redraw is a row replaced under the link.
  await mountTerminal(page);
  const requests = await routeFiles(page, {
    stat: (body) => ({ json: { status: "ready", path: body.path, host: "h", size: 10, can_open: false } }),
    download: () => ({ body: "x" }),
  });
  await attachGrid(page);
  await showScreen(page, ["/tmp/a.txt"], { cols: 40 });
  const path = await gridCell(page, 2, 0, 40);
  await page.mouse.move(path.x, path.y);
  await expect(page.locator("#term .terminal-file-target")).toContainText("10 B");
  // The same text, redrawn: no new check, still clickable.
  await showScreen(page, ["/tmp/a.txt"], { cols: 40, rev: 3 });
  await page.mouse.click(path.x, path.y);
  await expect(page.locator("#term .terminal-file-message")).toHaveText("Downloaded a.txt");
  expect(requests.filter((r) => r.op === "stat").length).toBe(1);
  // Different text under the pointer: the old link goes, and the new path
  // is checked afresh before anything is downloaded.
  await showScreen(page, ["/tmp/b.txt"], { cols: 40, rev: 4 });
  await expect(page.locator("#term .terminal-file-target")).toContainText("b.txt");
  await page.mouse.click(path.x, path.y);
  await expect(page.locator("#term .terminal-file-message")).toHaveText("Downloaded b.txt");
  expect(requests.map((r) => [r.op, r.body.path])).toEqual([
    ["stat", "/tmp/a.txt"],
    ["download", "/tmp/a.txt"],
    ["stat", "/tmp/b.txt"],
    ["download", "/tmp/b.txt"],
  ]);
  // A path that disappears takes its link with it.
  await showScreen(page, ["nothing here"], { cols: 40, rev: 5 });
  await expect(page.locator("#term .grid-terminal-link-underline > div")).toHaveCount(0);
  await expect(page.locator("#term .terminal-file-target")).toBeHidden();
});

test("a click whose file hover is covered is refused with an explanation", async ({ page }) => {
  // The hover is the whole disclosure: covered by another Farhelm surface
  // (a marked link cover), it cannot admit a download.
  await mountTerminal(page);
  const requests = await routeFiles(page, {
    stat: (body) => ({ json: { status: "ready", path: body.path, host: "h", size: 10, can_open: false } }),
    download: () => ({ body: "x" }),
  });
  await attachGrid(page);
  await showScreen(page, ["/tmp/a.txt"], { cols: 40 });
  const path = await gridCell(page, 2, 0, 40);
  await page.mouse.move(path.x, path.y);
  const hover = page.locator("#term .terminal-file-target");
  await expect(hover).toContainText("10 B");
  const box = (await hover.boundingBox())!;
  await page.evaluate((rect) => {
    const cover = document.createElement("div");
    cover.setAttribute("data-link-cover", "");
    cover.style.cssText = `position:fixed;left:${rect.x}px;top:${rect.y}px;width:${rect.width}px;height:${rect.height}px;z-index:45;pointer-events:none`;
    document.body.appendChild(cover);
  }, box);
  await page.mouse.click(path.x, path.y);
  await expect(page.locator("#term .terminal-file-message")).toHaveText(
    "cannot download: full file details are covered by another Farhelm surface; move the pointer and hover again",
  );
  expect(requests.filter((r) => r.op === "download")).toEqual([]);
});

test("a host's file name with format characters is shown with them visible", async ({ page }) => {
  // The disclosed path comes from the host; a right-to-left override in a
  // name must not reorder what the user reads before clicking.
  await mountTerminal(page);
  await routeFiles(page, {
    stat: () => ({ json: { status: "ready", path: "/tmp/invoice\u202Etxt.exe", host: "h", size: 10, can_open: false } }),
    download: () => ({ body: "x" }),
  });
  await attachGrid(page);
  await showScreen(page, ["/tmp/a.txt"], { cols: 40 });
  const path = await gridCell(page, 2, 0, 40);
  await page.mouse.move(path.x, path.y);
  await expect(page.locator("#term .terminal-file-target")).toContainText("invoice<U+202E>txt.exe");
});

test("in the desktop app, the platform's modifier-click opens an allowed file after saving it", async ({ page }) => {
  // The native save goes to Downloads, and Cmd-click (Ctrl-click off a Mac)
  // also opens it when the hover offered opening.
  await page.addInitScript(() => ((window as any).__farhelmWebviewDeviceSecret = "desktop-secret"));
  await mountTerminal(page);
  const requests = await routeFiles(page, {
    stat: (body) => ({ json: { status: "ready", path: body.path, host: "h", size: 10, can_open: true } }),
    download: () => ({ json: { path: "/Users/u/Downloads/a.md", opened: true } }),
  });
  await attachGrid(page);
  await showScreen(page, ["/tmp/a.md"], { cols: 40 });
  const mac = await viewerIsMac(page);
  const modifier = mac ? "Meta" : "Control";
  const path = await gridCell(page, 2, 0, 40);
  await page.mouse.move(path.x, path.y);
  await expect(page.locator("#term .terminal-file-target")).toContainText(`${mac ? "⌘" : "Ctrl"}-click to open`);
  await page.keyboard.down(modifier);
  await page.mouse.click(path.x, path.y);
  await page.keyboard.up(modifier);
  await expect(page.locator("#term .terminal-file-message")).toHaveText("Saved to /Users/u/Downloads/a.md · opened");
  const download = requests.find((r) => r.op === "download")!;
  expect(download.body).toEqual({ path: "/tmp/a.md", save_to_downloads: true, open: true });
});

test("a container session offers only the paths its host admits", async ({ page }) => {
  // A container's output names its own filesystem, so each path is checked
  // on the host before it is offered at all (SPEC_impl's checkout admission).
  await mountTerminal(page, undefined, { spec: { containerized: true } });
  await routeFiles(page, {
    stat: (body) => ({ json: body.path === "/work/real.txt"
      ? { status: "ready", path: body.path, host: "h", size: 10, can_open: false }
      : { status: "not_found", path: body.path, host: "h" } }),
    download: () => ({ body: "x" }),
  });
  await attachGrid(page);
  await showScreen(page, ["/work/real.txt /work/gone.txt"], { cols: 40 });
  const gone = await gridCell(page, 18, 0, 40);
  await page.mouse.move(gone.x, gone.y);
  const real = await gridCell(page, 3, 0, 40);
  await page.mouse.move(real.x, real.y);
  await expect(page.locator("#term .terminal-file-target")).toContainText("real.txt");
  await page.mouse.move(gone.x, gone.y);
  await expect(page.locator("#term .terminal-file-target")).toBeHidden();
  await expect(page.locator("#term .grid-terminal-link-underline > div")).toHaveCount(0);
});

test("in the desktop app, Cmd-click on a remote session's loopback link opens it through a tunnel", async ({ page }) => {
  // The hover names the remote host and port and says a plain click opens
  // on this Mac; the tunnel goes through the app's bridge with the
  // session's tunnel generation. A plain click opens the address as is.
  await page.addInitScript(() => {
    const w = window as any;
    w.__farhelmWebviewDeviceSecret = "desktop-secret";
    w.farhelmTunnelSessions = { s1: { available: true, host_name: "remote-box", generation: 7 } };
    w.tunnels = [];
    w.farhelmTunnelOpen = (request: unknown) => w.tunnels.push(request);
    w.opened = [];
    w.open = (url: string) => w.opened.push(url);
  });
  await mountTerminal(page);
  await attachGrid(page);
  await showScreen(page, ["dev server: http://localhost:3000/app"], { cols: 40 });
  const link = await gridCell(page, 20, 0, 40);
  await page.mouse.move(link.x, link.y);
  await expect(page.locator("#term .terminal-link-target")).toContainText("This is port 3000 on remote-box, not on this Mac.");
  await page.keyboard.down("Meta");
  await page.mouse.click(link.x, link.y);
  await page.keyboard.up("Meta");
  await expect.poll(() => page.evaluate(() => (window as any).tunnels.slice())).toEqual([
    { session_id: "s1", url: "http://localhost:3000/app", generation: 7 },
  ]);
  expect(await page.evaluate(() => (window as any).opened.slice())).toEqual([]);
});

// ## Round 2: a hover rests on what the link shows

test("an equal-length relabel of a hovered OSC 8 link under a resting pointer is judged again", async ({ page }) => {
  // The program redraws the same hyperlink with other text of the same
  // length: the target and cells are unchanged, but the text the warning
  // compares is not, so the hover is redone and now warns.
  await mountTerminal(page);
  await attachGrid(page);
  const target = "https://evil.example/";
  await showScreen(page, [["https://evil.example/", 0]], { cols: 40, links: [["x", target]] });
  const at = await gridCell(page, 3, 0, 40);
  await page.mouse.move(at.x, at.y);
  const display = page.locator("#term .terminal-link-target");
  await expect(display).toContainText("evil.example");
  await expect(display).not.toHaveClass(/terminal-link-target-mismatch/);
  await showScreen(page, [["https://bank.example/", 0]], { cols: 40, links: [["x", target]], rev: 3 });
  await expect(page.locator("#term .grid-terminal-row").nth(0)).toHaveText("https://bank.example/");
  await expect(display).toHaveClass(/terminal-link-target-mismatch/);
  await expect(display).toContainText("Text says: https://bank.example/");
});

test("a change to only an adjacent row of a hovered link is judged again", async ({ page }) => {
  // The comparison joins the rows of one link; the hovered row staying put
  // must not keep a verdict about a continuation that changed.
  await mountTerminal(page);
  await attachGrid(page);
  const target = "https://good.example/";
  const links: [string, string][] = [["x", target]];
  await showScreen(page, [["https://goo", 0], ["d.example/", 0]], { cols: 20, links });
  const at = await gridCell(page, 2, 0, 20);
  await page.mouse.move(at.x, at.y);
  const display = page.locator("#term .terminal-link-target");
  await expect(display).toContainText("good.example");
  await expect(display).not.toHaveClass(/terminal-link-target-mismatch/);
  // Only row 1 is sent: row 0, under the pointer, is not touched.
  await page.evaluate((message) => (window as any).FakeSocket.all[0].binary(message), JSON.stringify({
    t: "rows", rows: [[1, { w: false, r: [[0, 10, "x.example/", [0, -1, -1], 0]] }]], links,
  }));
  await page.evaluate((message) => (window as any).FakeSocket.all[0].binary(message), JSON.stringify({
    t: "frame", rev: 3, cols: 20, rows: 4, cursor: { x: 0, y: 3, visible: false, shape: "block", blink: true },
  }));
  await expect(display).toHaveClass(/terminal-link-target-mismatch/);
  await expect(display).toContainText("Text says: https://goox.example/");
});

test("a relabelled OSC 8 file link is checked afresh and downloads", async ({ page }) => {
  // The label changes under a resting pointer while the target stays: the
  // old check's permission goes with the old text, and the new hover checks
  // again, so the next click downloads.
  await mountTerminal(page);
  const requests = await routeFiles(page, {
    stat: (body) => ({ json: { status: "ready", path: body.path, host: "h", size: 10, can_open: false } }),
    download: () => ({ body: "x" }),
  });
  await attachGrid(page);
  const links: [string, string][] = [["f", "file:///tmp/a.txt"]];
  await showScreen(page, [["a.txt", 0]], { cols: 40, links });
  const at = await gridCell(page, 1, 0, 40);
  await page.mouse.move(at.x, at.y);
  await expect(page.locator("#term .terminal-file-target")).toContainText("10 B");
  await showScreen(page, [["b.txt", 0]], { cols: 40, links, rev: 3 });
  await expect.poll(() => requests.filter((r) => r.op === "stat").length).toBe(2);
  await expect(page.locator("#term .terminal-file-target")).toContainText("10 B");
  await page.mouse.click(at.x, at.y);
  await expect(page.locator("#term .terminal-file-message")).toHaveText("Downloaded a.txt");
  expect(requests.map((r) => r.op)).toEqual(["stat", "stat", "download"]);
});

test("a link relabelled between its press and release does not open", async ({ page }) => {
  // xterm.js activates only when the release finds the link the press was
  // on; text that changed under a held button is not that link.
  await page.addInitScript(() => {
    const w = window as any;
    w.opened = [];
    w.open = (url: string) => w.opened.push(url);
  });
  await mountTerminal(page);
  await attachGrid(page);
  const links: [string, string][] = [["x", "https://evil.example/"]];
  await showScreen(page, [["https://evil.example/", 0]], { cols: 40, links });
  const at = await gridCell(page, 3, 0, 40);
  await page.mouse.move(at.x, at.y);
  await expect(page.locator("#term .terminal-link-target")).toContainText("evil.example");
  await page.mouse.down();
  await showScreen(page, [["https://bank.example/", 0]], { cols: 40, links, rev: 3 });
  await expect(page.locator("#term .grid-terminal-row").nth(0)).toHaveText("https://bank.example/");
  await page.mouse.up();
  // Released in place: the hover is redone for the new text at once, so the
  // warning shows without the pointer moving.
  await expect(page.locator("#term .terminal-link-target")).toHaveClass(/terminal-link-target-mismatch/);
  expect(await page.evaluate(() => (window as any).opened.slice())).toEqual([]);
  // The premise that a click would open: a click on the current link does.
  await page.mouse.click(at.x, at.y);
  await expect.poll(() => page.evaluate(() => (window as any).opened.slice())).toEqual(["https://evil.example/"]);
});

test("a pointer move between a relabel and its paint judges the new text", async ({ page }) => {
  // The model takes the new row when the frame arrives and the page paints
  // on the next animation frame; a move in between must not keep the hover
  // made for the old text.
  await mountTerminal(page);
  await attachGrid(page);
  const links: [string, string][] = [["x", "https://evil.example/"]];
  await showScreen(page, [["https://evil.example/", 0]], { cols: 40, links });
  const at = await gridCell(page, 3, 0, 40);
  await page.mouse.move(at.x, at.y);
  const display = page.locator("#term .terminal-link-target");
  await expect(display).toContainText("evil.example");
  await expect(display).not.toHaveClass(/terminal-link-target-mismatch/);
  // Frame and a 1 px move in one task, and the display read right after
  // the move, before any paint: only the move's own check can have redone
  // the hover by then (a paint would redo it as well, so reading later
  // could not tell the two apart).
  const afterMove = await page.evaluate(({ x, y, links }) => {
    const ws = (window as any).FakeSocket.all[0];
    ws.binary(JSON.stringify({ t: "rows", rows: [[0, { w: false, r: [[0, 21, "https://bank.example/", [0, -1, -1], 0]] }]], links }));
    ws.binary(JSON.stringify({ t: "frame", rev: 3, cols: 40, rows: 4, cursor: { x: 0, y: 3, visible: false, shape: "block", blink: true } }));
    document.querySelector("#term .grid-terminal-screen")!.dispatchEvent(new MouseEvent("mousemove", { bubbles: true, clientX: x + 1, clientY: y }));
    const shown = document.querySelector("#term .terminal-link-target")!;
    return { className: shown.className, text: shown.textContent };
  }, { x: at.x, y: at.y, links });
  expect(afterMove.className).toContain("terminal-link-target-mismatch");
  expect(afterMove.text).toContain("Text says: https://bank.example/");
});

/** Put a marked link cover over `locator`'s box (an approval card, say). */
async function coverWithLinkCover(page: Page, box: { x: number; y: number; width: number; height: number }): Promise<void> {
  await page.evaluate((rect) => {
    const cover = document.createElement("div");
    cover.setAttribute("data-link-cover", "");
    cover.style.cssText = `position:fixed;left:${rect.x}px;top:${rect.y}px;width:${rect.width}px;height:${rect.height}px;z-index:45;pointer-events:none`;
    document.body.appendChild(cover);
  }, box);
}

test("a web link or tunnel click whose target display is covered is refused with an explanation", async ({ page }) => {
  // SPEC.md: the hover display is the whole safeguard for an OSC 8 link and
  // for a Cmd-click tunnel; covered by another Farhelm surface, it cannot
  // admit the click.
  await page.addInitScript(() => {
    const w = window as any;
    w.__farhelmWebviewDeviceSecret = "desktop-secret";
    w.farhelmTunnelSessions = { s1: { available: true, host_name: "remote-box", generation: 7 } };
    w.tunnels = [];
    w.farhelmTunnelOpen = (request: unknown) => w.tunnels.push(request);
    w.opened = [];
    w.open = (url: string) => w.opened.push(url);
  });
  await mountTerminal(page);
  await attachGrid(page);
  await showScreen(page, [["docs", 0], "", "http://localhost:3000/app"], { cols: 40, links: [["d", "https://docs.example/"]] });
  const osc = await gridCell(page, 1, 0, 40);
  await page.mouse.move(osc.x, osc.y);
  const display = page.locator("#term .terminal-link-target");
  await expect(display).toContainText("docs.example");
  await coverWithLinkCover(page, (await display.boundingBox())!);
  await page.mouse.click(osc.x, osc.y);
  const message = page.locator("#term .terminal-file-message");
  await expect(message).toHaveText("cannot open: full link details are covered by another Farhelm surface; move the pointer and hover again");
  const tunnel = await gridCell(page, 10, 2, 40);
  await page.mouse.move(tunnel.x, tunnel.y);
  await expect(display).toContainText("This is port 3000 on remote-box");
  await coverWithLinkCover(page, (await display.boundingBox())!);
  await page.keyboard.down("Meta");
  await page.mouse.click(tunnel.x, tunnel.y);
  await page.keyboard.up("Meta");
  await expect(message).toHaveText("cannot open: full link details are covered by another Farhelm surface; move the pointer and hover again");
  expect(await page.evaluate(() => [(window as any).opened.slice(), (window as any).tunnels.slice()])).toEqual([[], []]);
});
