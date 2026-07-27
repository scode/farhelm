// The M1 acceptance suite at the browser level (PLAN_M1.md criterion 5):
// create is exercised by the stack boot (startup session through the
// real creation API); these tests cover output rendering, input
// round-trip, reconnect replay, resize, and last-attach-wins takeover —
// all against a real helm, supervisor, tmux, and fake agent. No mocks.
//
// Assertions read the xterm.js BUFFER, not the DOM: the DOM renderer
// materializes only viewport rows, so scrolled-off content (exactly what
// replay tests care about) never appears in .xterm-rows. The buffer is
// the semantic truth of what the terminal holds.
import { test, expect, Page } from "@playwright/test";

/** Full text content of the terminal buffer (scrollback + viewport). */
async function termText(page: Page): Promise<string> {
  return page.evaluate(() => {
    const term = (window as any).__farhelmTerm;
    if (!term) return "";
    const buf = term.buffer.active;
    const lines: string[] = [];
    for (let i = 0; i < buf.length; i++) {
      lines.push(buf.getLine(i)?.translateToString(true) ?? "");
    }
    return lines.join("\n");
  });
}

/**
 * Poll the buffer until `needle` shows up. Polling, not a one-shot read:
 * terminal output arrives asynchronously over the WebSocket with no DOM
 * event to await, so there is nothing else to hook.
 */
async function waitForTermText(page: Page, needle: string, timeout = 15_000) {
  await expect
    .poll(() => termText(page), { timeout, message: `waiting for ${needle}` })
    .toContain(needle);
}

/**
 * Load the app and wait until the terminal is genuinely usable — mounted,
 * socket attached, agent listening. Every wait keys on a marker rather
 * than a sleep, which is why these tests are not flaky on a loaded CI box.
 */
async function openTerminal(page: Page) {
  await page.goto("/");
  // The island sets this once xterm is mounted and the WS is opening.
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  // The fake agent prints this banner once its modes are set.
  await waitForTermText(page, "FAKE-AGENT READY");
}

// First pixels: the whole stack standing up and putting an agent's output
// on screen. Everything below assumes this works, so when the suite goes
// red this is the test that says whether the problem is the stack or the
// behavior under test.
test("renders the session and the agent's TUI output", async ({ page }) => {
  await openTerminal(page);
  await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
  await waitForTermText(page, "fake-agent starting");
  // The visible viewport is real DOM text too — the DOM renderer is what
  // makes these tests semantic rather than pixel-diffing.
  await expect(page.locator(".xterm-rows")).toContainText("FAKE-AGENT READY");
});

// Real keystrokes through the whole chain: xterm's onData, the WebSocket,
// the framing protocol, tmux paste-buffer, and back out as pane output. This
// is the one test that would catch an input path wired up but dead — the
// failure a user would describe as "typing goes nowhere".
test("input round-trips through the real terminal path", async ({ page }) => {
  await openTerminal(page);
  await page.locator("#terminal").click();
  await page.keyboard.type("hello-from-playwright");
  await page.keyboard.press("Enter");
  await waitForTermText(page, "echo:hello-from-playwright", 10_000);
});

// SPEC.md's core durability promise seen from the browser: close the tab,
// come back, and the session looks as if you had never left. A reload is
// the harshest form of it — a brand-new xterm.js with an empty buffer, so
// everything on screen afterwards came from replay.
test("reload reattaches with replayed scrollback", async ({ page }) => {
  await openTerminal(page);
  await page.locator("#terminal").click();
  await page.keyboard.type("before-reload");
  await page.keyboard.press("Enter");
  await waitForTermText(page, "echo:before-reload");

  await page.reload();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  // Replay must bring back output produced before this attachment
  // existed — the reconnect-with-replay acceptance criterion.
  await waitForTermText(page, "echo:before-reload");
  await waitForTermText(page, "FAKE-AGENT READY");
});

// Exercise the complete browser-to-PTY resize chain. Xterm's local
// dimensions are only the requested geometry; the fake agent's `stty`
// result proves the WebSocket message reached tmux before later input.
test("resize reaches the real terminal", async ({ page }) => {
  await openTerminal(page);
  const before = await page.evaluate(() => {
    const t = (window as any).__farhelmTerm;
    return { cols: t.cols, rows: t.rows };
  });
  await page.setViewportSize({ width: 700, height: 500 });
  await expect
    .poll(
      () =>
        page.evaluate(() => {
          const t = (window as any).__farhelmTerm;
          return { cols: t.cols, rows: t.rows };
        }),
      { message: "viewport change must reflow the terminal via fit()" },
    )
    .not.toEqual(before);
  const geometry = await page.evaluate(() => {
    const t = (window as any).__farhelmTerm;
    return { cols: t.cols, rows: t.rows };
  });
  await page.locator("#terminal").click();
  await page.keyboard.type("size");
  await page.keyboard.press("Enter");
  await waitForTermText(
    page,
    `size:${geometry.rows} ${geometry.cols}`,
    10_000,
  );
});

test("second client takes over; first shows the detach banner", async ({
  browser,
  page,
}) => {
  await openTerminal(page);

  const second = await browser.newContext();
  const page2 = await second.newPage();
  await page2.goto("/");
  await page2.waitForFunction(() => (window as any).__farhelmTermReady === true);

  // SPEC.md: last attach wins, and the loser sees it happened.
  await expect(page.locator("#term-banner")).toBeVisible({ timeout: 10_000 });
  await expect(page.locator("#term-banner")).toContainText("Detached");

  // The winner is live: input still round-trips.
  await waitForTermText(page2, "FAKE-AGENT READY");
  await page2.locator("#terminal").click();
  await page2.keyboard.type("takeover-works");
  await page2.keyboard.press("Enter");
  await waitForTermText(page2, "echo:takeover-works", 10_000);
  await second.close();
});

// The creation API is the one true path (PLAN_M1.md: CLI flags feed the
// same API a UI dialog will call), so its HTTP surface needs coverage
// even though M1's UI never POSTs. Only the failure case is exercised
// here: a successful POST would add a second session to a stack whose UI
// shows one, which would perturb the tests above.
test("create API reports precondition failures verbatim", async ({ request }) => {
  const resp = await request.post("/api/sessions", {
    data: { cwd: "/nonexistent/definitely/not/here", invocation: "true" },
  });
  expect(resp.status()).toBe(500);
  expect(await resp.text()).toContain("working directory does not exist");
});

// Host/Origin validation is what keeps a hostile page from driving the
// helm through DNS rebinding; loopback binding alone does not.
test("requests from a foreign origin are refused", async ({ request }) => {
  const resp = await request.get("/api/sessions", {
    headers: { Origin: "http://evil.example" },
  });
  expect(resp.status()).toBe(403);
});

// The framing defense: the Origin check cannot stop an <iframe> pointed at
// the helm (a GET navigation sends no Origin), and a framed terminal is a
// clickjacking target where delivered keystrokes are command execution.
// The exact header values ARE the contract, so this is an exact-value
// assertion, not a change detector.
test("responses carry the anti-framing headers", async ({ request }) => {
  const resp = await request.get("/api/sessions");
  expect(resp.status()).toBe(200);
  expect(resp.headers()["x-frame-options"]).toBe("DENY");
  expect(resp.headers()["content-security-policy"]).toBe(
    "frame-ancestors 'none'",
  );
});

// An attach failure must arrive as a detach notice on the socket, not a
// bare close: the helm sends the reason before closing precisely because
// a bare close renders as a generic "connection closed" and tells the
// user nothing. Driven through a raw WebSocket because the UI only ever
// opens sockets for sessions the API listed.
test("a terminal socket for an unknown session reports why", async ({
  page,
}) => {
  await page.goto("/");
  const notice = await page.evaluate(
    () =>
      new Promise<string>((resolve, reject) => {
        const ws = new WebSocket(
          `ws://${location.host}/api/sessions/no-such-session/term`,
        );
        const timer = setTimeout(() => reject(new Error("no message")), 10_000);
        ws.onmessage = (ev) => {
          clearTimeout(timer);
          resolve(String(ev.data));
        };
        ws.onclose = () => {
          clearTimeout(timer);
          reject(new Error("socket closed with no detach notice"));
        };
      }),
  );
  const msg = JSON.parse(notice);
  expect(msg.type).toBe("detached");
  expect(msg.reason).toContain("no such session");
});

// The WebSocket message-size cap is sized for large pastes (xterm.js
// hands a whole clipboard paste over as ONE message), and lore records a
// review fix that nearly shipped a 1 MiB cap — which would have dropped
// the connection on exactly the paste chunking exists to support. Only a
// direct socket send can produce a multi-megabyte message, hence the
// __farhelmWs test hook.
//
// LAST in the file on purpose: the suite shares one session and runs in
// file order, and this payload's PTY echo pollutes the terminal state —
// observed directly: with the takeover test after this one, its fresh
// page's READY wait found only a wall of echoed 'a's. (How much echoes
// is bounded by canonical-mode input handling and was not pinned down;
// the placement rule, not the mechanism, is the contract.) Nothing may
// come after this test that depends on replay content.
test("a multi-megabyte message does not drop the terminal socket", async ({
  page,
}) => {
  await openTerminal(page);
  await page.evaluate(() => {
    const ws = (window as any).__farhelmWs as WebSocket;
    ws.send(new Uint8Array(2 * 1024 * 1024).fill(0x61));
  });
  // The send is async; poll until the socket has drained it, still open.
  await expect
    .poll(
      () =>
        page.evaluate(() => {
          const ws = (window as any).__farhelmWs as WebSocket;
          return { state: ws.readyState, buffered: ws.bufferedAmount };
        }),
      { timeout: 15_000, message: "socket must stay open and drain" },
    )
    .toEqual({ state: WebSocket.OPEN, buffered: 0 });
  await page.locator("#terminal").click();
  await page.keyboard.press("Enter");
  await page.keyboard.type("after-big-message");
  await page.keyboard.press("Enter");
  await waitForTermText(page, "echo:after-big-message", 15_000);
});
