// Browser-level coverage for the copy-to-system-clipboard fix: the symptom
// was a session terminal showing an agent TUI's own "copied" toast while a
// paste elsewhere produced stale content, because nothing on this page ever
// touched the REAL system clipboard for either half of what a terminal
// selection can be. terminal.js's own module header ("Copy to the system
// clipboard: two selections, two mechanisms") has the full diagnosis; this
// file is the browser-level proof that both mechanisms actually work end to
// end, through the real webview APIs neither a `node --test` unit nor a
// unit test of the vendored addon in isolation can exercise — OSC 52
// (write-only; a read/query must never see real clipboard contents) and
// copy-on-select (every completed local selection, wherever the gesture
// ends, with no "already copied this" cache to silently suppress a
// legitimate re-copy).
//
// A dedicated file rather than an addition to terminal.spec.ts, per this
// suite's own standing convention for new coverage (mouse-modes.spec.ts's
// header) — one subject, findable and runnable together, in a file that
// does not make an already enormous project slower.
//
// ## Chromium only, and why that is not a weaker proof
//
// Every clipboard assertion here needs `context.grantPermissions([
// "clipboard-read", "clipboard-write"])`, and Playwright can only grant
// those two permissions in Chromium — WebKit has no such permission to
// grant at all, so a WebKit project would either hang waiting for a grant
// that never resolves or throw immediately, neither of which says anything
// about this fix. The MECHANISM under test is engine-neutral (xterm.js's
// OSC 52 parsing and its own `SelectionService`/mouse-tracking duality,
// both confirmed directly against the vendored source when this fix was
// written — see terminal.js's header again), so a WebKit-shaped false red
// here would cost real signal for zero real coverage: this suite's WebKit
// projects exist to catch engine-family rendering/input differences (the
// desktop app's WKWebView/WebKitGTK — see playwright.config.ts's own
// header), and neither xterm.js's parser nor its mouse-mode bookkeeping
// varies by engine in a way this addition could catch that the rest of the
// WebKit suite would not already catch first. `test.beforeEach` below
// skips WebKit LOUDLY (a `console.log` line, since `test.skip`'s own reason
// is invisible under CI's default reporter — the same discipline
// real-agent.spec.ts's env-gated legs use) rather than silently: this file
// still contributes ONE WebKit project (`webkit-terminal-clipboard`,
// per-spec-file project naming — see playwright.config.ts), and every test
// in it reports as skipped rather than the file silently vanishing from a
// WebKit run.
//
// ## Gated invocations, and the race they close
//
// Every fixture below that needs its process to emit something specific —
// an OSC 52 escape, a marker line, a mouse-mode DECSET — routes it through
// `gatedShellInvocation()`, which blocks on a `read` until this test presses
// Enter. The instinctive alternative — bake the output straight into the
// invocation — has a real race: tmux starts an invocation's process the
// moment the session is CREATED, independent of any client attaching, so an
// unheld command could easily finish before Playwright has even navigated.
// For OSC 52 specifically that is not merely slow but PERMANENTLY wrong:
// replay (the M5 catch-up buffer, `capture-pane -e`) reconstructs the
// pane's CURRENT rendered grid, not a literal log of every escape byte ever
// written, and OSC 52 leaves no mark on that grid at all — a client that
// only ever attached after the write finished would see nothing, ever. So
// every test presses Enter only once it has confirmed BOTH that the
// terminal mounted and that its WebSocket is genuinely OPEN (mount alone is
// not enough — terminal.js drops input sent before OPEN, exactly the gap
// terminal.spec.ts's own DECRPM test works around the same way): the write
// happens live, while this test is already attached and parsing, which is
// the path every one of these mechanisms is actually meant to serve.
import { expect, test, type Page } from "@playwright/test";
import { cleanupSession, createSession } from "./helpers/fleet";

/**
 * Build a session invocation that blocks on `read _gate` until this test
 * presses Enter, then runs `script`, then sleeps to keep the pane alive —
 * see this file's header for why the gate exists at all. `script` is
 * spliced into a single-quoted `sh -c` argument (matching
 * terminal.spec.ts's own `sh -c 'trap "exit 7" TERM; sleep 300'` fixture),
 * so it must not itself contain an unescaped single quote — every caller
 * here only ever splices `printf` calls with base64 or ASCII-marker
 * payloads, neither of which can produce one.
 */
function gatedShellInvocation(script: string): string {
  return `sh -c 'read _gate; ${script}; sleep 300'`;
}

/**
 * Open a freshly created session and wait until its terminal is genuinely
 * usable — mounted AND socket-open, not merely mounted. The extra
 * `readyState` wait is what `openTerminal` in terminal.spec.ts does not
 * need (the shared "e2e-session" is long since idle by the time any test
 * attaches it) but every fixture in THIS file does: each session here is
 * brand new, and the DECRPM regression test's own docs record the gap this
 * closes — input sent between mount and socket-OPEN is silently dropped.
 */
async function attachSession(page: Page, id: string): Promise<void> {
  const target = page.locator(`[data-session-id="${id}"]`);
  await expect(target).toBeVisible({ timeout: 20_000 });
  await target.locator(".session-row-open").click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await page.waitForFunction(() => (window as any).__farhelmWs?.readyState === WebSocket.OPEN);
}

/** Full text content of the terminal buffer (scrollback + viewport) —
 * duplicated from terminal.spec.ts's identical helper rather than imported,
 * matching mouse-modes.spec.ts's own precedent: that file exports nothing,
 * and per this suite's per-area-file convention a new spec starts clean. */
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

/** Poll the buffer until `needle` shows up — terminal output arrives
 * asynchronously over the WebSocket with no DOM event to await. */
async function waitForTermText(page: Page, needle: string, timeout = 15_000) {
  await expect
    .poll(() => termText(page), { timeout, message: `waiting for ${needle}` })
    .toContain(needle);
}

/**
 * Poll for the viewport row currently containing `needle`, returning its
 * index and the terminal's current row count.
 *
 * Both are found live rather than assumed: the session's launch path runs
 * the invocation through a login shell, which is free to print above it, so
 * "the marker is on a specific fixed row" is exactly the kind of guess that
 * made an early version of this file's drag tests select a blank row
 * (xterm trims a blank-cells selection to `""`, so `hasSelection()` was
 * true while `getSelection()` had nothing to copy). And the search is
 * POLLED, not single-shot: the font-load refit (see terminal.js's font-swap
 * block) resizes the terminal shortly after attach, tmux redraws the pane,
 * and a one-time scan can land mid-redraw and see a buffer that briefly
 * lacks the marker even though a text-wait already saw it rendered.
 */
async function findViewportRow(
  page: Page,
  needle: string,
): Promise<{ rows: number; rowIndex: number }> {
  const scan = (text: string) =>
    page.evaluate((needleText) => {
      const t = (window as any).__farhelmTerm;
      const buf = t.buffer.active;
      for (let i = 0; i < t.rows; i++) {
        const line = buf.getLine(buf.viewportY + i);
        if (line && line.translateToString(true).includes(needleText)) {
          return { rows: t.rows, rowIndex: i };
        }
      }
      return { rows: t.rows, rowIndex: -1 };
    }, text);
  let geometry = await scan(needle);
  const deadline = Date.now() + 10_000;
  while (geometry.rowIndex < 0 && Date.now() < deadline) {
    await page.waitForTimeout(250);
    geometry = await scan(needle);
  }
  if (geometry.rowIndex < 0) throw new Error(`no viewport row contains ${needle}`);
  return geometry;
}

/**
 * Perform a real mouse drag starting just inside the row containing
 * `needle`, ending wherever `endpoint` says — real pixel coordinates
 * derived from `.xterm-screen`'s live bounding box and xterm's own
 * `rows`, the same geometry-from-live-layout approach mouse-modes.spec.ts's
 * `clickTerminalCell` uses, and for the same reason: the terminal's
 * on-screen size depends on viewport and font metrics this test does not
 * control directly.
 *
 * `.xterm-screen`, not `#terminal` itself: xterm registers its selection
 * mouse handlers against the screen layer, and the container's box includes
 * padding/border the screen does not occupy — a mousedown a couple of
 * pixels inside the CONTAINER can land outside the screen entirely and
 * never start a selection (observed as `hasSelection()` staying false on
 * this suite's first real run). The box is measured AFTER the row is
 * found, for the same font-refit reason `findViewportRow` polls: the
 * screen's on-page size can change under the swap, and coordinates derived
 * from a pre-refit box would drag across the wrong pixels.
 *
 * The two-stage move on the way out of mousedown (a short hop, then the
 * long sweep to `endpoint`) makes xterm register the drag as a selection
 * START before anything else happens, guarding against a threshold or
 * coalescing quirk swallowing a single large jump — found necessary on
 * this suite's own early runs, not defensive guessing.
 *
 * `modifierKey` holds a keyboard modifier for the whole gesture (F13's
 * Shift-forces-selection-under-mouse-reporting leg); released in a
 * `finally` so a thrown assertion never leaves it stuck down for whatever
 * test runs next.
 */
async function dragRow(
  page: Page,
  needle: string,
  endpoint: (screenBox: { x: number; y: number; width: number; height: number }, y: number) => {
    x: number;
    y: number;
  },
  modifierKey?: "Shift",
): Promise<void> {
  const { rows, rowIndex } = await findViewportRow(page, needle);
  const box = (await page.locator("#terminal .xterm-screen").boundingBox())!;
  const cellHeight = box.height / rows;
  const y = box.y + (rowIndex + 0.5) * cellHeight;
  if (modifierKey) await page.keyboard.down(modifierKey);
  try {
    await page.mouse.move(box.x + 4, y);
    await page.mouse.down();
    await page.mouse.move(box.x + 24, y, { steps: 3 });
    const end = endpoint(box, y);
    await page.mouse.move(end.x, end.y, { steps: 10 });
    await page.mouse.up();
  } finally {
    if (modifierKey) await page.keyboard.up(modifierKey);
  }
}

/** Drag the full width of the row containing `needle`, ending INSIDE the
 * terminal — the ordinary case every ordinary selection takes. */
async function dragAcrossRowContaining(
  page: Page,
  needle: string,
  modifierKey?: "Shift",
): Promise<void> {
  await dragRow(page, needle, (box, y) => ({ x: box.x + box.width - 4, y }), modifierKey);
}

/**
 * Drag from the row containing `needle` to a point well BELOW `#terminal`'s
 * own bottom edge (F1), still within its horizontal span.
 *
 * The direction is deliberate, and two other directions were tried first —
 * on this test's own real runs, not guessed. xterm's own coordinate mapping
 * (`Mouse.ts`'s `getCoords`) CLAMPS an out-of-bounds release into the
 * nearest valid row/column rather than leaving the selection open-ended, so
 * where a release lands relative to the terminal decides which row it
 * clamps to:
 *
 * - Landing ABOVE the terminal clamps to viewport row 1 — which, unless the
 *   marker happens to already be that row, drags the selection UPWARD
 *   across the blank rows before it instead of across the marker, proving
 *   only that xterm can select blank space.
 * - Landing to the LEFT, at the SAME row's `y`, clamps the COLUMN back to 1
 *   — the same column the drag's own mousedown started at, on the left
 *   edge of the same row — collapsing the whole gesture to a near
 *   zero-width selection. The sidebar's fixed-width column (an earlier
 *   version released there) is not a neutral choice either: it is full of
 *   real interactive rows, and a release landing on one produced session
 *   teardown noise in the helm's own log — a side effect this test must
 *   not risk causing merely to prove a drag ended off-element.
 * - Landing BELOW clamps the row to the LAST viewport row while the column
 *   still reflects the release's own `x` — so a release below-and-right of
 *   the start point selects from the marker's row all the way down,
 *   reliably containing the marker while landing squarely outside
 *   `#terminal`, with nothing interactive underneath it to trigger.
 */
async function dragFromRowToOutsideTerminal(page: Page, needle: string): Promise<void> {
  await dragRow(page, needle, (box) => ({ x: box.x + box.width - 4, y: box.y + box.height + 15 }));
}

/**
 * Replace `navigator.clipboard.writeText` on the live page with one that
 * either rejects immediately or never settles at all — the two browser
 * behaviors F4's fix has to survive (a denied permission; a permission
 * prompt nobody has answered yet). Installed AFTER attach rather than via
 * `addInitScript`: the addon's provider looks the method up at CALL time,
 * not at construction, so patching it any time before the triggering write
 * is equally effective, and doing it post-attach keeps this test's own
 * WebSocket/mount machinery running through the real, unpatched API.
 */
async function stubClipboardWriteText(page: Page, behavior: "reject" | "hang"): Promise<void> {
  await page.evaluate((mode) => {
    navigator.clipboard.writeText = mode === "reject"
      ? () => Promise.reject(new Error("stubbed clipboard refusal"))
      // A Promise that never resolves OR rejects — the never-answered
      // permission-prompt shape.
      : () => new Promise<void>(() => {});
  }, behavior);
}

/**
 * Record every WebSocket frame this page SENDS (browser to server) into
 * `window.__sentInput`, decoded to text — the same pattern
 * terminal.spec.ts's DECRPM regression test uses to inspect exactly what
 * reached the pty. Must be installed via `addInitScript`, before
 * navigation: the terminal's WebSocket is constructed during `mount()`, and
 * a patch applied after that would miss the prototype method it already
 * captured a reference to.
 */
async function recordOutboundWebSocketText(page: Page): Promise<() => Promise<string>> {
  await page.addInitScript(() => {
    const realSend = WebSocket.prototype.send;
    (window as any).__sentInput = [];
    WebSocket.prototype.send = function (this: WebSocket, data: any) {
      if (data instanceof Uint8Array) {
        (window as any).__sentInput.push(Array.from(data));
      } else if (data instanceof ArrayBuffer) {
        (window as any).__sentInput.push(Array.from(new Uint8Array(data)));
      } else {
        (window as any).__sentInput.push(data);
      }
      return realSend.call(this, data);
    };
  });
  return () =>
    page.evaluate(() =>
      ((window as any).__sentInput as unknown[])
        .map((frame) => (Array.isArray(frame) ? String.fromCharCode(...(frame as number[])) : String(frame)))
        .join(""),
    );
}

test.beforeEach(async ({ context, browserName, baseURL }) => {
  if (browserName === "webkit") {
    console.log(
      "SKIPPED: terminal-clipboard — WebKit cannot be granted clipboard-read/clipboard-write " +
        "permissions under Playwright; see this file's header for why that is a Playwright/" +
        "engine limitation rather than evidence the fix is Chromium-specific.",
    );
  }
  test.skip(
    browserName === "webkit",
    "Playwright has no clipboard permission grant for WebKit, and the mechanism under test " +
      "(xterm.js OSC 52 parsing, its SelectionService/mouse-tracking duality) is engine-neutral " +
      "— a WebKit run here would be a false red, not real coverage.",
  );
  // Every test in this file reads or seeds `navigator.clipboard` from
  // page-context JS; without this grant Chromium's headless clipboard
  // silently refuses both directions and every assertion below would fail
  // for a reason that has nothing to do with the fix. Origin-scoped to the
  // stack's own baseURL (playwright.config.ts) rather than omitted: recent
  // Chromium requires an explicit origin for the clipboard permissions
  // specifically.
  await context.grantPermissions(["clipboard-read", "clipboard-write"], {
    origin: new URL(baseURL!).origin,
  });
});

test("an OSC 52 write from the terminal's own process reaches the system clipboard, byte for byte", async ({
  page,
  request,
}) => {
  // Non-ASCII on purpose (F18): an accented character and a supplementary-
  // plane emoji, so this test also pins that the addon's base64 round trip
  // — js-base64 inside the bundle, decoded again by this test via Node's
  // own UTF-8-aware `Buffer` — survives real Unicode, not merely ASCII.
  // Neither character reaches the SHELL COMMAND itself (only its base64
  // encoding does, which is pure ASCII), so this adds no quoting risk.
  const marker = `osc52-café-🎉-e2e-${Date.now()}`;
  const payload = Buffer.from(marker, "utf8").toString("base64");
  // Base64's alphabet (`[A-Za-z0-9+/=]`) contains nothing the outer
  // single-quoted shell token or the inner double-quoted printf format
  // string treats specially, so the payload is safe to splice in directly
  // with no further escaping.
  const invocation = gatedShellInvocation(`printf "\\033]52;c;${payload}\\007"`);
  const session = await createSession(request, {
    title: `osc52-e2e-${Date.now()}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");

    await expect
      .poll(() => page.evaluate(() => navigator.clipboard.readText()), {
        timeout: 15_000,
        message: "waiting for the OSC 52 write to land in the system clipboard",
      })
      .toBe(marker);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("an OSC 52 read query never receives real clipboard contents (F5)", async ({
  page,
  request,
}) => {
  const recordedText = await recordOutboundWebSocketText(page);
  const secret = `secret-e2e-${Date.now()}`;
  const invocation = gatedShellInvocation(`printf "\\033]52;c;?\\007"`);
  const session = await createSession(request, {
    title: `osc52-read-e2e-${Date.now()}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    await page.evaluate((value) => navigator.clipboard.writeText(value), secret);
    // Confirm the seed actually landed before trusting it as a baseline —
    // see the no-clobber test below for why this matters.
    await expect
      .poll(() => page.evaluate(() => navigator.clipboard.readText()))
      .toBe(secret);

    await attachSession(page, session.id);
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");

    // The addon answers a query SYNCHRONOUSLY when the provider's
    // `readText` is not a Promise (`ClipboardAddon.ts`'s own
    // `_setOrReportClipboard`), which `clipboardProvider` in terminal.js
    // is by design — so the report is sent as terminal INPUT
    // (`Terminal.input`, which rides the same `onData` -> WebSocket path
    // any keystroke does) almost immediately once xterm parses the query.
    await expect
      .poll(recordedText, {
        timeout: 15_000,
        message: "waiting for the addon's OSC 52 report of the (refused) query",
      })
      .toMatch(/\x1b\]52;c;[A-Za-z0-9+/=]*\x07/);

    const text = await recordedText();
    const report = text.match(/\x1b\]52;c;([A-Za-z0-9+/=]*)\x07/)!;
    expect(
      report[1],
      "a refused read must report an EMPTY payload, not merely a harmless-looking one",
    ).toBe("");
    expect(
      text,
      "the secret must never reach the terminal's own input/output in any form",
    ).not.toContain(secret);
    expect(
      text,
      "the secret's base64 form — the shape a real leak would actually take on the wire — " +
        "must not appear either",
    ).not.toContain(Buffer.from(secret, "utf8").toString("base64"));
    expect(
      await page.evaluate(() => navigator.clipboard.readText()),
      "the query must not have disturbed the real clipboard either",
    ).toBe(secret);
  } finally {
    await cleanupSession(request, session.id);
  }
});

for (
  const behavior of [
    { mode: "reject" as const, label: "rejects" },
    { mode: "hang" as const, label: "never resolves" },
  ]
) {
  test(
    `an OSC 52 write survives navigator.clipboard.writeText that ${behavior.label} (F4/F15)`,
    async ({ page, request }) => {
      const pageErrors: Error[] = [];
      page.on("pageerror", (err) => pageErrors.push(err));

      const marker = `osc52-stub-${behavior.mode}-${Date.now()}`;
      const sentinel = `sentinel-${behavior.mode}-${Date.now()}`;
      const payload = Buffer.from(marker, "utf8").toString("base64");
      // The sentinel prints in the SAME invocation, right after the OSC 52
      // write — so waiting for it to render is proof the parser moved past
      // that escape sequence, not merely that the page never crashed.
      const invocation = gatedShellInvocation(
        `printf "\\033]52;c;${payload}\\007"; printf "${sentinel}\\n"`,
      );
      const session = await createSession(request, {
        title: `osc52-stub-${behavior.mode}-e2e-${Date.now()}`,
        cwd: "/tmp",
        invocation,
      });
      try {
        await page.goto("/");
        await attachSession(page, session.id);
        await stubClipboardWriteText(page, behavior.mode);
        await page.locator("#terminal").click();
        await page.keyboard.press("Enter");

        await waitForTermText(page, sentinel, 15_000);
        expect(
          pageErrors,
          "a stubbed clipboard failure must never surface as a page error",
        ).toEqual([]);
      } finally {
        await cleanupSession(request, session.id);
      }
    },
  );
}

test("a real drag-select copies the local selection to the system clipboard exactly (F11/F17)", async ({
  page,
  request,
}) => {
  // The printf sits behind the gate for a SECOND reason beyond the general
  // one this file's header covers: the font swap terminal.js performs
  // shortly after mount re-fits the terminal, the resize makes tmux reflow
  // the pane, and content printed BEFORE that reflow can end up in
  // scrollback above a blank viewport (observed on this suite's early
  // runs: viewportY 3, every visible row empty, the marker in lines 0-2).
  // Printing only after attach + fonts settle puts the marker on the
  // screen the drag actually sees.
  const marker = `copysel-e2e-${Date.now()}`;
  const session = await createSession(request, {
    title: `copysel-e2e-${Date.now()}`,
    cwd: "/tmp",
    invocation: gatedShellInvocation(`printf "${marker}\\n"`),
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await page.evaluate(() => (document as any).fonts.ready.then(() => undefined));
    // Click to FOCUS the terminal before typing (the row click that
    // attached it leaves focus on the sidebar row); without it the Enter
    // never reaches the pty and the gated printf never runs.
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    await waitForTermText(page, marker);

    await dragAcrossRowContaining(page, marker);
    // Confirms the drag itself produced a LOCAL xterm selection before
    // trusting anything below — without this, a failure could mean either
    // "the drag selected nothing" or "the selection was never copied", and
    // only one of those is this fix's own bug.
    await expect
      .poll(() => page.evaluate(() => !!(window as any).__farhelmTerm?.hasSelection()), {
        timeout: 5_000,
        message: "the drag must produce a local xterm selection before any copy can be asserted",
      })
      .toBe(true);
    // Captured HERE, immediately after the drag settles, so the equality
    // check below is against exactly what this gesture selected — not a
    // second guess about what the row "should" contain (F11: `toBe`, not
    // `toContain`; the row carries trailing blank cells past the marker,
    // and this is the selection those cells are legitimately part of).
    const selected = await page.evaluate(() => (window as any).__farhelmTerm.getSelection());
    expect(selected, "the captured selection must at least contain the marker it was aimed at").toContain(
      marker,
    );

    await expect
      .poll(() => page.evaluate(() => navigator.clipboard.readText()), {
        timeout: 15_000,
        message: "waiting for the drag-ended selection to reach the system clipboard",
      })
      .toBe(selected);

    // F17: copying is an OBSERVER, not an editor — the selection this test
    // just proved landed in the clipboard must still be exactly what it
    // was, on screen, after the copy resolved.
    expect(
      await page.evaluate(() => (window as any).__farhelmTerm.hasSelection()),
      "the selection must still be present after copying",
    ).toBe(true);
    expect(
      await page.evaluate(() => (window as any).__farhelmTerm.getSelection()),
      "the selection's text must be unchanged by copying it",
    ).toBe(selected);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("a drag released outside the terminal element still copies (F1)", async ({
  page,
  request,
}) => {
  const marker = `outside-drag-e2e-${Date.now()}`;
  const session = await createSession(request, {
    title: `outside-drag-e2e-${Date.now()}`,
    cwd: "/tmp",
    invocation: gatedShellInvocation(`printf "${marker}\\n"`),
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await page.evaluate(() => (document as any).fonts.ready.then(() => undefined));
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    await waitForTermText(page, marker);

    await dragFromRowToOutsideTerminal(page, marker);
    await expect
      .poll(() => page.evaluate(() => !!(window as any).__farhelmTerm?.hasSelection()), {
        timeout: 5_000,
        message:
          "a drag released outside the terminal must still finalize a local xterm selection " +
          "(xterm's own document-level mouseup listener does this regardless of where this " +
          "file's copy-on-select listener lives)",
      })
      .toBe(true);
    const selected = await page.evaluate(() => (window as any).__farhelmTerm.getSelection());
    expect(selected).toContain(marker);

    await expect
      .poll(() => page.evaluate(() => navigator.clipboard.readText()), {
        timeout: 15_000,
        message:
          "waiting for the outside-released selection to reach the system clipboard — this is " +
          "exactly the gesture terminal.js's DOCUMENT-level mouseup listener exists to catch",
      })
      .toBe(selected);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("reselecting identical text re-copies after an external clipboard change (F2/F14)", async ({
  page,
  request,
}) => {
  const marker = `reselect-e2e-${Date.now()}`;
  const session = await createSession(request, {
    title: `reselect-e2e-${Date.now()}`,
    cwd: "/tmp",
    invocation: gatedShellInvocation(`printf "${marker}\\n"`),
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await page.evaluate(() => (document as any).fonts.ready.then(() => undefined));
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    await waitForTermText(page, marker);

    await dragAcrossRowContaining(page, marker);
    const selected = await page.evaluate(() => (window as any).__farhelmTerm.getSelection());
    await expect
      .poll(() => page.evaluate(() => navigator.clipboard.readText()), { timeout: 15_000 })
      .toBe(selected);

    // Something ELSE now owns the clipboard — another app, another
    // terminal, anything. The removed cache (copy-on-select.js's own
    // header has the history) would have silently refused to restore
    // `selected` on the next identical drag, because it looked
    // "unchanged" from what this mechanism last copied — even though the
    // SYSTEM clipboard plainly no longer holds it.
    const overwrite = `external-overwrite-e2e-${Date.now()}`;
    await page.evaluate((text) => navigator.clipboard.writeText(text), overwrite);
    await expect
      .poll(() => page.evaluate(() => navigator.clipboard.readText()), { timeout: 15_000 })
      .toBe(overwrite);

    // Reselect the EXACT same text. This must copy again.
    await dragAcrossRowContaining(page, marker);
    const reselected = await page.evaluate(() => (window as any).__farhelmTerm.getSelection());
    expect(reselected, "the same row must select the same text a second time").toBe(selected);
    await expect
      .poll(() => page.evaluate(() => navigator.clipboard.readText()), {
        timeout: 15_000,
        message: "reselecting identical text must re-copy it, overwriting the external change",
      })
      .toBe(selected);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("under mouse reporting: a plain drag copies nothing, Shift+drag copies exactly (F13)", async ({
  page,
  request,
}) => {
  const marker = `mousereport-e2e-${Date.now()}`;
  // DECSET 1000 (X10/normal mouse tracking) right after the marker line —
  // enough to disable xterm's own `SelectionService` (`bindMouse`'s
  // `onProtocolChange`) for the plain-drag leg below.
  const session = await createSession(request, {
    title: `mousereport-e2e-${Date.now()}`,
    cwd: "/tmp",
    invocation: gatedShellInvocation(`printf "${marker}\\n\\033[?1000h"`),
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await page.evaluate(() => (document as any).fonts.ready.then(() => undefined));
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    await waitForTermText(page, marker);
    // `Terminal.modes.mouseTrackingMode` is xterm's own PUBLIC readout of
    // what DECSET 1000 just did — waited on directly rather than inferred
    // from a settle delay, so this leg cannot start before mouse reporting
    // is actually live.
    await expect
      .poll(() => page.evaluate(() => (window as any).__farhelmTerm.modes.mouseTrackingMode), {
        timeout: 10_000,
        message: "waiting for DECSET 1000 to take effect",
      })
      .not.toBe("none");

    // Leg 1: a PLAIN drag under mouse reporting must not create a local
    // selection, and must leave the clipboard exactly as it found it — the
    // same no-clobber shape the dedicated test below uses, applied to the
    // OTHER reason a drag can produce nothing to copy.
    const baseline = `mousereport-baseline-e2e-${Date.now()}`;
    await page.evaluate((text) => navigator.clipboard.writeText(text), baseline);
    await expect
      .poll(() => page.evaluate(() => navigator.clipboard.readText()))
      .toBe(baseline);
    await dragAcrossRowContaining(page, marker);
    expect(
      await page.evaluate(() => !!(window as any).__farhelmTerm?.hasSelection()),
      "a plain drag while mouse reporting is on must not make a local xterm selection",
    ).toBe(false);
    // No poll here on purpose, matching the no-clobber test's own
    // reasoning: the claim is that nothing happens, and polling for
    // absence would only race a regression rather than prove one.
    await page.waitForTimeout(500);
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(baseline);

    // Leg 2: Shift+drag FORCES a local selection even with mouse reporting
    // on, and it must copy exactly like any other local selection.
    await dragAcrossRowContaining(page, marker, "Shift");
    await expect
      .poll(() => page.evaluate(() => !!(window as any).__farhelmTerm?.hasSelection()), {
        timeout: 5_000,
      })
      .toBe(true);
    const selected = await page.evaluate(() => (window as any).__farhelmTerm.getSelection());
    expect(selected).toContain(marker);
    await expect
      .poll(() => page.evaluate(() => navigator.clipboard.readText()), { timeout: 15_000 })
      .toBe(selected);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("a click without a drag does not clobber the system clipboard (F19: zero writeText calls)", async ({
  page,
  request,
}) => {
  const seeded = `preexisting-e2e-${Date.now()}`;
  const session = await createSession(request, {
    title: `no-clobber-e2e-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    await page.evaluate((text) => navigator.clipboard.writeText(text), seeded);
    // Confirm the seed actually landed before trusting it as a baseline —
    // a permission grant that silently failed would otherwise make this
    // test pass for the wrong reason (an empty clipboard "unchanged" by an
    // equally-failing click).
    await expect
      .poll(() => page.evaluate(() => navigator.clipboard.readText()))
      .toBe(seeded);

    await attachSession(page, session.id);

    // A counting proxy wraps the REAL `writeText` (F19): the assertion is
    // "copy-on-select never even ATTEMPTS a write" for this gesture, which
    // a plain before/after clipboard-content check cannot distinguish from
    // "it wrote the exact same text back" — a real, if unlikely, way this
    // property could look satisfied by accident.
    await page.evaluate(() => {
      const real = navigator.clipboard.writeText.bind(navigator.clipboard);
      (window as any).__writeTextCalls = 0;
      navigator.clipboard.writeText = (text: string) => {
        (window as any).__writeTextCalls += 1;
        return real(text);
      };
    });

    // Real mousedown then mouseup at the SAME point — no `mouse.move`
    // between them — is what makes this a click rather than a drag: xterm
    // never sets a selection end for a click with no movement (its
    // `_handleSingleClick`), so `term.hasSelection()` stays false and
    // copy-on-select's own guard skips it (see copy-on-select.js).
    const box = (await page.locator("#terminal").boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.up();

    // No poll here on purpose: the claim under test is that NOTHING
    // happens, and polling for absence would only race whatever async
    // clipboard write a regression might produce rather than proving it
    // never arrives. A short settle window plus one direct read is the
    // honest shape of a negative assertion.
    await page.waitForTimeout(500);
    expect(await page.evaluate(() => (window as any).__writeTextCalls)).toBe(0);
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(seeded);
  } finally {
    await cleanupSession(request, session.id);
  }
});
