// Browser-level coverage for terminal link activation: both the OSC 8
// hyperlinks an agent's process emits (the kind Claude Code prints for file
// paths and URLs) and the bare plain-text http(s) URLs the vendored
// WebLinks addon linkifies. A click on either opens the target in a NEW
// tab, with no `confirm()` dialog in between. The dialog is what the OSC 8
// half pins against: xterm.js ships a default link handler that, absent a
// configured `linkHandler`, pops "Do you want to navigate to …? WARNING:
// This link could potentially be dangerous" on every click — a blanket
// warning the user cannot act on, and one that surfaced as "some popup
// about possibly being dangerous" in real use. terminal.js now configures
// its own handler (the `linkHandler` option at `new Terminal({...})` plus
// the shared opener both adapters call); this file is the proof the
// configured path is the one a real click takes.
//
// Only the WEB branch of that opener (`window.open`) is reachable here.
// The opener's other branch — navigating the page itself, which the
// desktop webview's navigation handler turns into a system-browser open —
// keys on the `dioxus:` page origin no browser run can present, and stays
// with manual desktop passes (it was verified by hand on the macOS build
// when the fix was written) plus the `node --test` opener cases in
// js-tests/terminal-links.test.js, which are the only automated coverage
// of that branch.
//
// A dedicated file, per this suite's standing convention for new coverage
// (mouse-modes.spec.ts's header): one subject, findable and runnable on
// its own. Both engines run it: link activation goes through xterm's
// `Linkifier` mouse handling, which is exactly the kind of input path the
// WebKit projects exist to cross-check against the desktop renderer family.
// That includes the selection/copy legs — deliberately NOT copying
// terminal-clipboard.spec.ts's WebKit skip. The clipboard suite skips
// WebKit because Playwright cannot grant it real clipboard permissions;
// the link suite needs no such grant (it stubs `window.open`, and the one
// copy-payload assertion uses an observed write seam on WebKit while
// keeping real-clipboard coverage on Chromium), so a WebKit skip here
// would cost real input-path coverage for no reason.
//
// Every link is emitted through a `read`-gated invocation, for the reason
// terminal-clipboard.spec.ts's "Gated invocations" header spells out: tmux
// starts the process at session CREATION, and output printed before this
// test attaches would only ever reach the terminal via replay — a path
// the replay test below covers deliberately, but not the live path the
// rest of this coverage is about.
import { expect, test } from "./helpers/evidence";
import { type Page } from "@playwright/test";
import { cleanupSession, createSession } from "./helpers/fleet";
import { attachSession, waitForTermText } from "./helpers/term";
import { replayRecord } from "./helpers/terminal-readiness";

/** One recorded `window.open` call: destination plus the isolation args. */
interface OpenedLink {
  url: string;
  target: string;
  features: string;
}

/**
 * Replace `window.open` before any page script runs, recording every call
 * instead of opening anything — a headless test has no use for a real
 * second tab, and the assertion is about WHICH URL the terminal asked to
 * open, not about the browser honoring it. Registered via `addInitScript`
 * so the stub is in place before terminal.js constructs its handler, even
 * though the handler only reads `window.open` at click time. Init scripts
 * survive `page.reload()`, so the replay test's second attachment records
 * into a fresh array with no re-install needed.
 *
 * Records target and features alongside the URL (not merely the
 * destination) so the opener's isolation contract — `_blank` plus
 * `noopener` — is asserted, not assumed.
 */
async function recordWindowOpen(page: Page): Promise<() => Promise<OpenedLink[]>> {
  await page.addInitScript(() => {
    (window as any).__openedLinks = [];
    window.open = ((url: string, target: string, features: string) => {
      (window as any).__openedLinks.push({
        url: String(url),
        target: String(target),
        features: String(features),
      });
      return null;
    }) as typeof window.open;
  });
  return () => page.evaluate(() => (window as any).__openedLinks as OpenedLink[]);
}

/**
 * Collect every dialog the page raises for the test's whole lifetime, so
 * each activation test can assert xterm's default `confirm()` never ran.
 * Installed per test (not per file) because `addInitScript`-style global
 * setup cannot observe dialogs, and a file-level listener would attribute
 * one test's dialog to whatever test happened to run next.
 */
function recordDialogs(page: Page): string[] {
  const dialogs: string[] = [];
  page.on("dialog", (dialog) => {
    dialogs.push(`${dialog.type()}: ${dialog.message()}`);
    void dialog.dismiss();
  });
  return dialogs;
}

/**
 * Prove the link wiring under test actually loaded: the vendored addon's
 * UMD global and the shared opener/filter helper. Without both,
 * `mountWhenReady` never mounts at all (its gate waits rather than
 * mounting without links), so a missing script would otherwise surface as
 * a confusing attach timeout instead of naming the broken precondition.
 */
async function assertLinkWiring(page: Page): Promise<void> {
  const wiring = await page.evaluate(() => ({
    addon: typeof (window as any).WebLinksAddon?.WebLinksAddon,
    helper: typeof (window as any).farhelmTerminalLinks?.openTerminalUrl,
    filter: typeof (window as any).farhelmTerminalLinks?.isPlainWebUrl,
  }));
  expect(wiring.addon, "the vendored WebLinks addon global must be loaded").toBe("function");
  expect(wiring.helper, "the shared link opener must be loaded").toBe("function");
  expect(wiring.filter, "the plain-text URL filter must be loaded").toBe("function");
}

/**
 * Build a session invocation that blocks on `read _gate` until this test
 * presses Enter, then runs `script`, then sleeps to keep the pane alive —
 * the same shape (and the same no-single-quote constraint on `script`) as
 * terminal-clipboard.spec.ts's `gatedShellInvocation`, which this file's
 * header cites for why the gate exists. Every caller here splices `printf`
 * calls with URL/ASCII payloads, none of which can produce a quote.
 */
function gatedShellInvocation(script: string): string {
  return `sh -c 'read _gate; ${script}; sleep 300'`;
}

interface RowGeometry {
  rows: number;
  cols: number;
  rowIndex: number;
  rowText: string;
}

/** One-shot scan for the first viewport row containing `needle`. */
async function scanViewportRow(page: Page, needle: string): Promise<RowGeometry> {
  return page.evaluate((needleText) => {
    const t = (window as any).__farhelmTerm;
    const buf = t.buffer.active;
    for (let i = 0; i < t.rows; i++) {
      const line = buf.getLine(buf.viewportY + i);
      const text = line ? line.translateToString(true) : "";
      if (text.includes(needleText)) {
        return { rows: t.rows, cols: t.cols, rowIndex: i, rowText: text };
      }
    }
    return { rows: t.rows, cols: t.cols, rowIndex: -1, rowText: "" };
  }, needle);
}

/**
 * Poll for the viewport row currently containing `needle`, returning its
 * index, the live grid size, and the row's text.
 *
 * Found live rather than assumed: the login shell is free to print above
 * the invocation's own output, and the rare font-backstop path can still
 * resize the terminal shortly after attach and make tmux redraw the pane
 * (the same reason terminal-clipboard.spec.ts's `findViewportRow` polls
 * rather than scanning once). Polling through `expect.poll` keeps this
 * free of fixed sleeps.
 */
async function findViewportRow(page: Page, needle: string): Promise<RowGeometry> {
  await expect
    .poll(() => scanViewportRow(page, needle).then((found) => found.rowIndex), {
      timeout: 10_000,
      message: `waiting for a viewport row containing ${needle}`,
    })
    .toBeGreaterThanOrEqual(0);
  const geometry = await scanViewportRow(page, needle);
  expect(geometry.rowIndex, `no viewport row contains ${needle}`).toBeGreaterThanOrEqual(0);
  return geometry;
}

/**
 * Pixel coordinates for the center of `clickText` where it appears in the
 * viewport row containing `rowNeedle` — real geometry from
 * `.xterm-screen`'s live box and xterm's own grid size, the same approach
 * terminal-clipboard.spec.ts's `dragRow` uses and for the same reasons
 * (the screen layer is where xterm's mouse handlers live; the box is
 * measured after the row is found so a late re-fit cannot shift
 * coordinates under the gesture).
 *
 * `rowNeedle` and `clickText` are usually the same string; they differ
 * when the click target is a substring of a longer row (a URL's middle,
 * a wrapped segment, one cell of a negative-scheme line).
 */
async function linkCell(
  page: Page,
  rowNeedle: string,
  clickText: string,
): Promise<{ x: number; y: number }> {
  const geometry = await findViewportRow(page, rowNeedle);
  const col = geometry.rowText.indexOf(clickText);
  expect(col, `row containing ${rowNeedle} must contain ${clickText}`).toBeGreaterThanOrEqual(0);
  const screen = page.locator("#terminal .xterm-screen");
  const box = (await screen.boundingBox())!;
  const cellWidth = box.width / geometry.cols;
  const cellHeight = box.height / geometry.rows;
  return {
    x: box.x + (col + clickText.length / 2) * cellWidth,
    y: box.y + (geometry.rowIndex + 0.5) * cellHeight,
  };
}

/**
 * Move the pointer onto `clickText` and wait for xterm's link decoration
 * (the pointer cursor class) to resolve — the observable proof the hovered
 * cell is a live link, not merely link-shaped text.
 *
 * Starts each attempt by visiting the row's far edge and then leaving the
 * screen entirely: xterm retains its last cell on mouseleave and ignores
 * re-entry at that same cell, so arriving fresh (and clearing any earlier
 * link first, asserted, so a stale decoration cannot satisfy readiness) is
 * what makes the wait meaningful. Targets must therefore not sit on the
 * row's own last cell, where the clearing visit would itself land on the
 * link; no caller here puts one there.
 *
 * `attempts` re-derives coordinates and re-hovers when decoration does not
 * resolve — for the resize test only, where tmux reflow can still be
 * shifting rows under the first hover. Everywhere else a single attempt
 * keeps a genuinely dead linkifier a fast, loud failure.
 */
async function hoverLink(
  page: Page,
  rowNeedle: string,
  clickText: string,
  attempts = 1,
): Promise<{ x: number; y: number }> {
  const screen = page.locator("#terminal .xterm-screen");
  let lastError: unknown = null;
  for (let attempt = 0; attempt < attempts; attempt++) {
    const geometry = await findViewportRow(page, rowNeedle);
    const col = geometry.rowText.indexOf(clickText);
    expect(col, `row containing ${rowNeedle} must contain ${clickText}`).toBeGreaterThanOrEqual(0);
    const box = (await screen.boundingBox())!;
    const cellWidth = box.width / geometry.cols;
    const cellHeight = box.height / geometry.rows;
    const x = box.x + (col + clickText.length / 2) * cellWidth;
    const y = box.y + (geometry.rowIndex + 0.5) * cellHeight;
    await page.mouse.move(box.x + box.width - cellWidth / 2, y);
    await page.mouse.move(box.x + box.width / 2, box.y - 1);
    await expect(screen).not.toHaveClass(/\bxterm-cursor-pointer\b/);
    await page.mouse.move(x, y);
    try {
      await expect(screen).toHaveClass(/\bxterm-cursor-pointer\b/, { timeout: 2_000 });
      return { x, y };
    } catch (error) {
      lastError = error;
    }
  }
  throw lastError;
}

/**
 * Hover `clickText` and prove it is NOT a link: no pointer decoration
 * after the page has observably processed the hover. The round-trip after
 * the final mousemove is the barrier — hover resolution runs inside the
 * mousemove dispatch, so a subsequent evaluation proves the page looked
 * for a link there and found none, rather than the assertion racing the
 * hover. Callers pair every such negative with a positive control on the
 * same page (a hovered bare URL that DOES decorate), so a dead linkifier
 * cannot pass as a clean negative.
 */
async function hoverExpectNone(page: Page, rowNeedle: string, clickText: string): Promise<void> {
  const { x, y } = await linkCell(page, rowNeedle, clickText);
  const screen = page.locator("#terminal .xterm-screen");
  const box = (await screen.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y - 1);
  await expect(screen).not.toHaveClass(/\bxterm-cursor-pointer\b/);
  await page.mouse.move(x, y);
  await page.evaluate(() => (window as any).__farhelmTerm.cols);
  await expect(screen).not.toHaveClass(/\bxterm-cursor-pointer\b/);
}

/** Hover a link (proving it resolves) and click it for real. */
async function clickLink(
  page: Page,
  rowNeedle: string,
  clickText: string,
  attempts = 1,
): Promise<void> {
  const { x, y } = await hoverLink(page, rowNeedle, clickText, attempts);
  await page.mouse.click(x, y);
}

/**
 * Drag within one viewport row, from the center of `startCol` to the
 * center of `endCol` — real pixels from the live screen box, for gestures
 * whose start and end positions are the assertion (a drag starting AND
 * ending inside one URL). The short hop before the long sweep mirrors
 * `dragRow`'s: it makes xterm register the selection start before the
 * sweep, guarding against a threshold or coalescing quirk swallowing a
 * single large jump.
 */
async function dragRowSpan(
  page: Page,
  rowNeedle: string,
  startCol: number,
  endCol: number,
): Promise<void> {
  const geometry = await findViewportRow(page, rowNeedle);
  const box = (await page.locator("#terminal .xterm-screen").boundingBox())!;
  const cellWidth = box.width / geometry.cols;
  const cellHeight = box.height / geometry.rows;
  const y = box.y + (geometry.rowIndex + 0.5) * cellHeight;
  const x1 = box.x + (startCol + 0.5) * cellWidth;
  const x2 = box.x + (endCol + 0.5) * cellWidth;
  await page.mouse.move(x1, y);
  await page.mouse.down();
  await page.mouse.move(x1 + 8, y, { steps: 2 });
  await page.mouse.move(x2, y, { steps: 10 });
  await page.mouse.up();
}

/** The terminal's current local selection text (xterm's own API). */
async function getSelection(page: Page): Promise<string> {
  return page.evaluate(() => (window as any).__farhelmTerm.getSelection() as string);
}

test("clicking an OSC 8 hyperlink opens it in a new tab with no confirm dialog", async ({
  page,
  request,
}) => {
  const openedLinks = await recordWindowOpen(page);
  const dialogs = recordDialogs(page);

  const stamp = Date.now();
  const url = `https://example.com/farhelm-e2e/${stamp}`;
  const text = `FARHELM-LINK-${stamp}`;
  // OSC 8 with BEL terminators: `ESC ] 8 ; ; <url> BEL <text> ESC ] 8 ; ; BEL`.
  // BEL rather than ST (`ESC \`) keeps a backslash out of the doubly-quoted
  // printf format; xterm accepts either terminator.
  const invocation =
    `sh -c 'read _gate; printf "\\033]8;;${url}\\007${text}\\033]8;;\\007\\n"; sleep 300'`;
  const session = await createSession(request, {
    title: `osc8-e2e-${stamp}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await assertLinkWiring(page);
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    await waitForTermText(page, text);

    await clickLink(page, text, text);

    await expect
      .poll(openedLinks, { timeout: 10_000, message: "waiting for the link click to reach window.open" })
      .toEqual([{ url, target: "_blank", features: "noopener" }]);
    expect(dialogs, "xterm's default confirm() must not run").toEqual([]);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("plain-text http and https URLs open in a new tab with no confirm dialog", async ({
  page,
  request,
}) => {
  const openedLinks = await recordWindowOpen(page);
  const dialogs = recordDialogs(page);

  const stamp = Date.now();
  const httpsUrl = `https://example.com/plain-${stamp}`;
  const httpUrl = `http://example.com/plain-${stamp}`;
  const invocation = gatedShellInvocation(
    `printf "HTTPS-${stamp} ${httpsUrl}\\nHTTP-${stamp} ${httpUrl}\\n"`,
  );
  const session = await createSession(request, {
    title: `plain-links-${stamp}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await assertLinkWiring(page);
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    // Both markers, before either click: the clicks below must run
    // against live output this attachment observably received, not against
    // a half-arrived line whose link range is still shifting.
    await waitForTermText(page, `HTTPS-${stamp}`);
    await waitForTermText(page, `HTTP-${stamp}`);

    await clickLink(page, `HTTPS-${stamp}`, httpsUrl);
    await clickLink(page, `HTTP-${stamp}`, httpUrl);

    await expect
      .poll(openedLinks, { timeout: 10_000, message: "waiting for both plain-text clicks" })
      .toEqual([
        { url: httpsUrl, target: "_blank", features: "noopener" },
        { url: httpUrl, target: "_blank", features: "noopener" },
      ]);
    expect(dialogs, "xterm's default confirm() must not run").toEqual([]);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("a URL wrapped across rows activates from its continuation row with the full target", async ({
  page,
  request,
}) => {
  const openedLinks = await recordWindowOpen(page);
  const dialogs = recordDialogs(page);

  // 300 characters of URL: far past any viewport width this suite runs at
  // (1280px with a 340px sidebar leaves on the order of a hundred
  // columns), so this MUST wrap — and the test asserts that premise
  // below rather than trusting the arithmetic.
  const stamp = Date.now();
  // The tail segment is distinctive (`end-<stamp>`, appearing NOWHERE
  // else in the line) so finding it proves the URL's last row — a bare
  // filler slice would also match the filler's start on the first row.
  const url = `https://example.com/wrapped-${stamp}/${"a".repeat(225)}end-${stamp}`;
  const head = url.slice(0, 20);
  const tail = `end-${stamp}`;
  const invocation = gatedShellInvocation(`printf "WRAP-${stamp} ${url}\\n"`);
  const session = await createSession(request, {
    title: `wrapped-link-${stamp}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await assertLinkWiring(page);
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    await waitForTermText(page, `WRAP-${stamp}`);
    await waitForTermText(page, tail);

    // The wrap premise, as buffer facts: head and tail render on
    // different viewport rows, so clicking the tail row exercises the
    // addon's wrapped-logical-line reconstruction, not a single-row
    // match. (`waitForTermText` joins rows with newlines, so it cannot
    // see the URL whole — which is exactly why head and tail are
    // awaited as separate segments.)
    const headRow = await findViewportRow(page, head);
    const tailRow = await findViewportRow(page, tail);
    expect(
      tailRow.rowIndex,
      "the long URL must wrap: its tail must render below its head",
    ).toBeGreaterThan(headRow.rowIndex);

    await clickLink(page, tail, tail);

    await expect
      .poll(openedLinks, { timeout: 10_000, message: "waiting for the wrapped-URL click" })
      .toEqual([{ url, target: "_blank", features: "noopener" }]);
    expect(dialogs, "xterm's default confirm() must not run").toEqual([]);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("a URL beginning above the viewport activates from its visible continuation", async ({
  page,
  request,
}) => {
  const openedLinks = await recordWindowOpen(page);
  const dialogs = recordDialogs(page);

  // The viewport-boundary sibling of the both-visible wrap test above:
  // the URL's head row sits ABOVE `buffer.active.viewportY` while its
  // tail segment still renders, so activation must reconstruct the
  // logical line across the viewport boundary rather than within it.
  // ~480 characters (≈5 rows) keeps the target scroll window several
  // rows wide; the tail segment is distinctive (`end-<stamp>`) for the
  // same reason as above.
  const stamp = Date.now();
  const url = `https://example.com/above-${stamp}/${"d".repeat(425)}end-${stamp}`;
  const head = url.slice(0, 20);
  const tail = `end-${stamp}`;
  const invocation = gatedShellInvocation(
    `printf "ABOVE-${stamp} ${url}\\n"; i=1; while [ $i -le 150 ]; do echo pad-$i; i=$((i + 1)); done`,
  );
  const session = await createSession(request, {
    title: `above-link-${stamp}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await assertLinkWiring(page);
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    // The last pad line is the receipt for the whole burst: only once
    // all 150 have landed is the URL known to start above the viewport.
    await waitForTermText(page, "pad-150");

    // Servo the viewport with real wheel gestures until the boundary
    // straddles the URL: head row above `viewportY`, tail segment
    // rendering. Each scan is the barrier — wheel up while the tail is
    // still below the viewport, wheel back down on any overshoot that
    // exposes the head — so no assumed notch size or fixed sleep
    // decides the position. Coarse notches far from the target, fine
    // ones near it (fine steps move about a row, well under the
    // several-row target window, so the servo cannot straddle past it
    // forever).
    const screen = page.locator("#terminal .xterm-screen");
    const box = (await screen.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    const locate = () =>
      page.evaluate(({ headText, tailText }) => {
        const t = (window as any).__farhelmTerm;
        const buf = t.buffer.active;
        let headRow = -1;
        let tailRow = -1;
        for (let i = 0; i < buf.length; i++) {
          const text = buf.getLine(i)?.translateToString(true) ?? "";
          if (headRow < 0 && text.includes(headText)) headRow = i;
          if (text.includes(tailText)) tailRow = i;
        }
        const top = buf.viewportY;
        return {
          headAbove: headRow >= 0 && headRow < top,
          tailVisible: tailRow >= top && tailRow < top + t.rows,
          distance: tailRow - top,
        };
      }, { headText: head, tailText: tail });
    let position = await locate();
    for (let notch = 0; notch < 120 && !(position.headAbove && position.tailVisible); notch++) {
      if (position.tailVisible) {
        await page.mouse.wheel(0, 20);
      } else if (position.distance < -10) {
        await page.mouse.wheel(0, -240);
      } else {
        await page.mouse.wheel(0, -20);
      }
      position = await locate();
    }
    expect(position.headAbove, "the URL must begin above the viewport").toBe(true);
    expect(position.tailVisible, "a continuation segment must stay visible").toBe(true);

    await clickLink(page, tail, tail);

    await expect
      .poll(openedLinks, { timeout: 10_000, message: "waiting for the above-viewport click" })
      .toEqual([{ url, target: "_blank", features: "noopener" }]);
    expect(dialogs, "xterm's default confirm() must not run").toEqual([]);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("a wrapped URL still activates after a resize reflows it", async ({
  page,
  request,
}) => {
  const openedLinks = await recordWindowOpen(page);
  const dialogs = recordDialogs(page);

  const stamp = Date.now();
  // Distinctive tail segment, per the wrapped-URL test above: a bare
  // filler slice would match the filler's start, not the last row.
  const url = `https://example.com/reflow-${stamp}/${"b".repeat(225)}end-${stamp}`;
  const tail = `end-${stamp}`;
  const invocation = gatedShellInvocation(`printf "REFLOW-${stamp} ${url}\\n"`);
  const session = await createSession(request, {
    title: `reflow-link-${stamp}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await assertLinkWiring(page);
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    await waitForTermText(page, tail);
    const colsBefore = await page.evaluate(() => (window as any).__farhelmTerm.cols as number);

    // Shrink hard: the terminal must refit to FEWER columns, so tmux
    // reflows the pane and the URL's row split changes under it. The
    // cols change is the barrier — without it this test would be
    // clicking a URL that never reflowed.
    await page.setViewportSize({ width: 800, height: 600 });
    await expect
      .poll(() => page.evaluate(() => (window as any).__farhelmTerm.cols as number), {
        timeout: 15_000,
        message: "waiting for xterm to refit after the viewport shrank",
      })
      .not.toBe(colsBefore);

    // Rows may still be shifting while tmux's reflow lands, so the hover
    // re-derives its coordinates and retries rather than aiming once at
    // a row that is still moving.
    await clickLink(page, tail, tail, 5);

    await expect
      .poll(openedLinks, { timeout: 10_000, message: "waiting for the post-reflow click" })
      .toEqual([{ url, target: "_blank", features: "noopener" }]);
    expect(dialogs, "xterm's default confirm() must not run").toEqual([]);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("a replayed plain-text URL activates after a reload with no second gate", async ({
  page,
  request,
}) => {
  const openedLinks = await recordWindowOpen(page);
  const dialogs = recordDialogs(page);

  const stamp = Date.now();
  const url = `https://example.com/replay-${stamp}`;
  const marker = `REPLAY-${stamp}`;
  const invocation = gatedShellInvocation(`printf "${marker} ${url}\\n"`);
  const session = await createSession(request, {
    title: `replay-link-${stamp}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await assertLinkWiring(page);
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    // The live boundary: this attachment observably received the output
    // while attached. The gate is spent — the process now sleeps — so
    // nothing below can be a live re-emission.
    await waitForTermText(page, marker);

    // A reload is the harshest detach/reattach: a brand-new xterm with an
    // empty buffer, so everything below came from this attach's own
    // replay. No Enter is pressed afterwards — there is no second gate
    // to release.
    await page.reload();
    await attachSession(page, session.id);
    await assertLinkWiring(page);
    // The replay boundary, two ways: the marker is back without any new
    // producer output, and the catch-up record proves the replay carried
    // bytes into xterm while hidden (not an empty catch-up that merely
    // revealed). `writesWhileHidden`, not `bufferedBytes`: the latter is
    // current holdings, reset to zero at reveal by design.
    await waitForTermText(page, marker);
    const replay = await replayRecord(page, "terminal");
    expect(
      replay.writesWhileHidden,
      "the replay must have written the pane's bytes while hidden",
    ).toBeGreaterThanOrEqual(1);

    await clickLink(page, marker, url);

    await expect
      .poll(openedLinks, { timeout: 10_000, message: "waiting for the replayed-URL click" })
      .toEqual([{ url, target: "_blank", features: "noopener" }]);
    expect(dialogs, "xterm's default confirm() must not run").toEqual([]);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("a URL scrolled out of view activates after scrolling back to it", async ({
  page,
  request,
}) => {
  const openedLinks = await recordWindowOpen(page);
  const dialogs = recordDialogs(page);

  const stamp = Date.now();
  const url = `https://example.com/scrollback-${stamp}`;
  const marker = `SCROLL-${stamp}`;
  const invocation = gatedShellInvocation(
    `printf "${marker} ${url}\\n"; i=1; while [ $i -le 200 ]; do echo filler-$i; i=$((i + 1)); done`,
  );
  const session = await createSession(request, {
    title: `scrollback-link-${stamp}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await assertLinkWiring(page);
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    // The whole burst arrived live: the last filler line is the receipt
    // for all 200 pushing the marker up and out.
    await waitForTermText(page, "filler-200");

    // The scrollback premise, as buffer facts: the marker's line sits
    // ABOVE the viewport (`viewportY`), genuinely out of view — not
    // merely off-screen by assumption.
    const position = await page.evaluate((needle) => {
      const t = (window as any).__farhelmTerm;
      const buf = t.buffer.active;
      let found = -1;
      for (let i = 0; i < buf.length; i++) {
        if (buf.getLine(i)?.translateToString(true).includes(needle)) {
          found = i;
          break;
        }
      }
      return { found, viewportY: buf.viewportY };
    }, marker);
    expect(position.found, "the marker line must exist in the buffer").toBeGreaterThanOrEqual(0);
    expect(
      position.found,
      "the marker must have scrolled above the viewport before scrolling back",
    ).toBeLessThan(position.viewportY);

    // A real wheel gesture back up, one notch at a time, until the
    // marker's row renders again — each scan is the barrier, so no
    // fixed sleep decides when the scroll "should" have landed.
    const screen = page.locator("#terminal .xterm-screen");
    const box = (await screen.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    let visible = await scanViewportRow(page, marker);
    for (let notch = 0; notch < 60 && visible.rowIndex < 0; notch++) {
      await page.mouse.wheel(0, -240);
      visible = await scanViewportRow(page, marker);
    }
    expect(visible.rowIndex, "wheeling up must bring the marker back into view").toBeGreaterThanOrEqual(0);

    await clickLink(page, marker, url);

    await expect
      .poll(openedLinks, { timeout: 10_000, message: "waiting for the scrolled-back URL click" })
      .toEqual([{ url, target: "_blank", features: "noopener" }]);
    expect(dialogs, "xterm's default confirm() must not run").toEqual([]);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("surrounding punctuation is trimmed but in-URL characters are kept", async ({
  page,
  request,
}) => {
  const openedLinks = await recordWindowOpen(page);
  const dialogs = recordDialogs(page);

  // Explicit expected targets: the parenthesized URL opens WITHOUT its
  // parens, the sentence URL WITHOUT its trailing period, and the
  // underscore/dash/tilde URL WHOLE.
  const stamp = Date.now();
  const parenUrl = `https://example.com/paren-${stamp}`;
  const sentUrl = `https://example.com/sent-${stamp}`;
  const inUrlChars = `https://example.com/a_b-c~d-${stamp}`;
  const invocation = gatedShellInvocation(
    `printf "PAREN-${stamp} (${parenUrl})\\nSENT-${stamp} See ${sentUrl}.\\nINURL-${stamp} ${inUrlChars}\\n"`,
  );
  const session = await createSession(request, {
    title: `punct-links-${stamp}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await assertLinkWiring(page);
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    await waitForTermText(page, `PAREN-${stamp}`);
    await waitForTermText(page, `SENT-${stamp}`);
    await waitForTermText(page, `INURL-${stamp}`);

    await clickLink(page, `PAREN-${stamp}`, parenUrl);
    await clickLink(page, `SENT-${stamp}`, sentUrl);
    await clickLink(page, `INURL-${stamp}`, inUrlChars);

    await expect
      .poll(openedLinks, { timeout: 10_000, message: "waiting for the three punctuation clicks" })
      .toEqual([
        { url: parenUrl, target: "_blank", features: "noopener" },
        { url: sentUrl, target: "_blank", features: "noopener" },
        { url: inUrlChars, target: "_blank", features: "noopener" },
      ]);
    expect(dialogs, "xterm's default confirm() must not run").toEqual([]);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("a drag starting and ending inside one URL selects and copies without opening", async ({
  page,
  request,
  context,
  browserName,
}) => {
  const openedLinks = await recordWindowOpen(page);
  const dialogs = recordDialogs(page);

  // A near-row-width URL on ONE row (sized after measuring nothing: the
  // 100-character filler fits every viewport this suite runs at without
  // wrapping, and the test asserts the single-row premise below), so the
  // drag's start and end cells are unambiguously inside one link.
  const stamp = Date.now();
  const url = `https://example.com/drag-${stamp}/${"c".repeat(60)}`;
  const prefix = `DRAG-${stamp} `;
  const urlStart = prefix.length;
  const invocation = gatedShellInvocation(`printf "${prefix}${url}\\n"`);
  const session = await createSession(request, {
    title: `drag-link-${stamp}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await assertLinkWiring(page);
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    await waitForTermText(page, `DRAG-${stamp}`);

    // Copy-payload observation, per engine: Chromium keeps REAL clipboard
    // coverage (granted, origin-scoped, like the clipboard suite —
    // without the grant the write silently refuses and the assertion
    // would fail for a reason outside this fix), while WebKit — where
    // Playwright has no clipboard grant at all — observes the write
    // through a recording stub installed post-attach (the copy path
    // looks the method up at call time, so patching now is equivalent).
    // Either way the assertion below is about the payload the gesture
    // produced, and NEITHER engine skips the behavior.
    let readCopiedPayload: () => Promise<string>;
    if (browserName === "webkit") {
      await page.evaluate(() => {
        (window as any).__copiedPayloads = [];
        navigator.clipboard.writeText = (text: string) => {
          (window as any).__copiedPayloads.push(text);
          return Promise.resolve();
        };
      });
      readCopiedPayload = () =>
        page.evaluate(() => ((window as any).__copiedPayloads as string[]).join("\n"));
    } else {
      await context.grantPermissions(["clipboard-read", "clipboard-write"], {
        origin: new URL(page.url()).origin,
      });
      readCopiedPayload = () => page.evaluate(() => navigator.clipboard.readText());
    }

    // The single-row premise: the URL's head and tail share one viewport
    // row, so the drag below cannot accidentally cross a wrap boundary.
    const headRow = await findViewportRow(page, url.slice(0, 20));
    const tailRow = await findViewportRow(page, url.slice(-15));
    expect(
      tailRow.rowIndex,
      "the drag URL must fit on one row",
    ).toBe(headRow.rowIndex);

    // Start AND end inside the URL, well clear of both ends: this is the
    // gesture xterm's Linkifier would activate (matching press/release
    // link identity, no selection guard of its own) if the plain-link
    // adapter did not suppress it.
    await dragRowSpan(page, `DRAG-${stamp}`, urlStart + 4, urlStart + url.length - 9);

    // The selection survived the gesture (not cleared by any activation
    // path), it is the dragged URL span (a URL substring of substantial
    // length — asserted loosely because pixel-to-cell edges can shift by
    // a cell, tightly enough that a collapsed or blank selection fails),
    // ordinary copy-on-select still copied it, and NOTHING opened.
    // Activation runs synchronously inside mouseup, so the selection
    // round-trip below also flushes any open the drag could have caused
    // before the empty-opens assertion reads — no sleep decides that.
    const selection = await getSelection(page);
    expect(selection.length, "the within-URL drag must leave a selection").toBeGreaterThan(40);
    expect(url.includes(selection), "the selection must be the dragged URL span").toBe(true);
    // Copy completion is its OWN async boundary, distinct from the
    // synchronous opener above: copy-on-select defers the read-and-write
    // past mouseup through an un-awaited `setTimeout(..., 0)` and never
    // awaits the clipboard write itself (terminal.js's
    // `handleCopyOnSelectMouseUp`), so a one-shot read here could fail
    // on correct behavior under scheduling or clipboard latency. Poll
    // for the payload to settle instead — same seam, both engines.
    await expect
      .poll(readCopiedPayload, { timeout: 10_000, message: "waiting for copy-on-select to settle" })
      .toBe(selection);
    expect(await openedLinks(), "a within-URL drag must not activate").toEqual([]);

    // And the guard must not break the ordinary case: clicking the same
    // link right after selecting still opens (mousedown clears the old
    // selection before mouseup runs, so the guard sees a plain click).
    await clickLink(page, `DRAG-${stamp}`, url.slice(0, 30));
    await expect
      .poll(openedLinks, { timeout: 10_000, message: "waiting for the post-selection click" })
      .toEqual([{ url, target: "_blank", features: "noopener" }]);
    expect(dialogs, "xterm's default confirm() must not run").toEqual([]);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("dragging from inside a URL into surrounding text selects without opening", async ({
  page,
  request,
}) => {
  const openedLinks = await recordWindowOpen(page);
  const dialogs = recordDialogs(page);

  const stamp = Date.now();
  const url = `https://example.com/across-${stamp}`;
  const prefix = `ACROSS-${stamp} `;
  const suffix = " trailing words here";
  const invocation = gatedShellInvocation(`printf "${prefix}${url}${suffix}\\n"`);
  const session = await createSession(request, {
    title: `drag-across-${stamp}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await assertLinkWiring(page);
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    await waitForTermText(page, `ACROSS-${stamp}`);

    // From inside the URL to the row's last text cell: the release lands
    // outside the link (so xterm's own identity check already declines)
    // AND a selection exists (so the adapter guard declines too) —
    // either way, nothing may open, and the selection must keep both
    // the URL span and the surrounding words.
    const rowText = (await findViewportRow(page, `ACROSS-${stamp}`)).rowText;
    await dragRowSpan(page, `ACROSS-${stamp}`, prefix.length + 4, rowText.length - 1);

    const selection = await getSelection(page);
    expect(selection, "the drag must keep the URL span").toContain(url.slice(4));
    expect(selection, "the drag must keep the surrounding words").toContain("trailing words");
    expect(await openedLinks(), "a drag out of a URL must not activate").toEqual([]);
    expect(dialogs, "xterm's default confirm() must not run").toEqual([]);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("an OSC 8 span wins over its https display text, beside a working bare URL", async ({
  page,
  request,
}) => {
  const openedLinks = await recordWindowOpen(page);
  const dialogs = recordDialogs(page);

  // The OSC span's DISPLAY text is itself https-looking: the plain-text
  // matcher would linkify it on its own, but the OSC provider registered
  // first, so the click must open the OSC TARGET — exactly once — while
  // the adjacent bare URL on the same line opens normally. (Partial
  // overlaps inside OSC-covered spans are deliberately NOT promised
  // here: overlap removal can discard a whole lower-priority candidate,
  // so only the full-span click plus the adjacent bare URL are pinned.)
  const stamp = Date.now();
  const display = `https://display.example/${stamp}`;
  const target = `https://target.example/${stamp}`;
  const bare = `https://bare.example/${stamp}`;
  const invocation =
    `sh -c 'read _gate; printf "OSC-${stamp} \\033]8;;${target}\\007${display}\\033]8;;\\007 ${bare}\\n"; sleep 300'`;
  const session = await createSession(request, {
    title: `osc-precedence-${stamp}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await assertLinkWiring(page);
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    await waitForTermText(page, display);
    await waitForTermText(page, bare);

    await clickLink(page, display, display);
    await clickLink(page, bare, bare);

    await expect
      .poll(openedLinks, { timeout: 10_000, message: "waiting for the OSC + bare clicks" })
      .toEqual([
        { url: target, target: "_blank", features: "noopener" },
        { url: bare, target: "_blank", features: "noopener" },
      ]);
    expect(dialogs, "xterm's default confirm() must not run").toEqual([]);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("forbidden and malformed schemes are neither decorated nor clickable", async ({
  page,
  request,
}) => {
  const openedLinks = await recordWindowOpen(page);
  const dialogs = recordDialogs(page);

  // One cell per rejected shape, plus a bare-URL control on the same
  // page: the control decorating and opening proves the linkifier was
  // alive throughout, so each negative is a true negative rather than a
  // dead hover path. None of the negative lines contains an http(s)
  // substring of its own (embedded-substring behavior has its own test
  // below); each is hovered (no pointer decoration) AND clicked (no
  // opener call, no navigation).
  const stamp = Date.now();
  const control = `https://control.example/${stamp}`;
  const cells: Array<{ needle: string; text: string }> = [
    { needle: `NEGJS-${stamp}`, text: "javascript:alert(document.domain)" },
    { needle: `NEGDATA-${stamp}`, text: "data:text/plain,hello" },
    { needle: `NEGFILE-${stamp}`, text: "file:///etc/passwd" },
    { needle: `NEGMAIL-${stamp}`, text: "mailto:someone@example.com" },
    { needle: `NEGFTP-${stamp}`, text: "ftp://files.example/pub/f" },
    { needle: `NEGPROTO-${stamp}`, text: "//cdn.example/lib.js" },
    { needle: `NEGMAL-${stamp}`, text: "http:broken" },
  ];
  const lines = cells.map((cell) => `${cell.needle} ${cell.text}`).concat([`NEGCTL-${stamp} ${control}`]);
  const invocation = gatedShellInvocation(`printf "${lines.join("\\n")}\\n"`);
  const session = await createSession(request, {
    title: `negative-links-${stamp}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await assertLinkWiring(page);
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    for (const cell of cells) {
      await waitForTermText(page, cell.needle);
    }
    await waitForTermText(page, `NEGCTL-${stamp}`);
    const pageUrl = page.url();

    // Control first: the decoration machinery resolves a real link on
    // this page before any negative runs.
    await hoverLink(page, `NEGCTL-${stamp}`, control);

    for (const cell of cells) {
      await hoverExpectNone(page, cell.needle, cell.text);
      const { x, y } = await linkCell(page, cell.needle, cell.text);
      await page.mouse.click(x, y);
      // The click round-trip flushes any activation the click could have
      // caused before the empty-opens read — same synchronous-mouseup
      // reasoning as the drag tests.
      await page.evaluate(() => (window as any).__farhelmTerm.cols);
      expect(await openedLinks(), `${cell.text} must not activate`).toEqual([]);
      expect(page.url(), `${cell.text} must not navigate`).toBe(pageUrl);
    }

    // Control last: the same bare URL still opens, proving the page's
    // link path worked end to end throughout the negatives.
    await clickLink(page, `NEGCTL-${stamp}`, control);
    await expect
      .poll(openedLinks, { timeout: 10_000, message: "waiting for the control click" })
      .toEqual([{ url: control, target: "_blank", features: "noopener" }]);
    expect(dialogs, "xterm's default confirm() must not run").toEqual([]);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("an http substring inside a forbidden scheme opens only the http part", async ({
  page,
  request,
}) => {
  const openedLinks = await recordWindowOpen(page);
  const dialogs = recordDialogs(page);

  // The addon's verified boundary behavior: it matches the independently
  // recognizable http substring, NOT the forbidden scheme around it. So
  // the `javascript:`/`data:` prefixes must neither decorate nor open,
  // while the embedded http spans are genuine links opening exactly
  // themselves — never the enclosing forbidden URL.
  const stamp = Date.now();
  const embeddedJs = `http://evil.example/js-${stamp}`;
  const embeddedData = `http://evil.example/data-${stamp}`;
  const invocation = gatedShellInvocation(
    `printf "EMBJS-${stamp} javascript:${embeddedJs}\\nEMBDATA-${stamp} data:text/plain,see-${embeddedData}\\n"`,
  );
  const session = await createSession(request, {
    title: `embedded-links-${stamp}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await assertLinkWiring(page);
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    await waitForTermText(page, `EMBJS-${stamp}`);
    await waitForTermText(page, `EMBDATA-${stamp}`);
    const pageUrl = page.url();

    // The forbidden prefixes: no decoration, and a click opens nothing
    // and navigates nowhere.
    await hoverExpectNone(page, `EMBJS-${stamp}`, "javascript:");
    await hoverExpectNone(page, `EMBDATA-${stamp}`, "data:text/plain,see-");
    for (const [needle, prefix] of [
      [`EMBJS-${stamp}`, "javascript:"],
      [`EMBDATA-${stamp}`, "data:text/plain,see-"],
    ] as const) {
      const { x, y } = await linkCell(page, needle, prefix);
      await page.mouse.click(x, y);
      await page.evaluate(() => (window as any).__farhelmTerm.cols);
      expect(await openedLinks(), `${prefix} must not activate`).toEqual([]);
      expect(page.url(), `${prefix} must not navigate`).toBe(pageUrl);
    }

    // The embedded http spans: genuine links, opening exactly themselves.
    await clickLink(page, `EMBJS-${stamp}`, embeddedJs);
    await clickLink(page, `EMBDATA-${stamp}`, embeddedData);
    await expect
      .poll(openedLinks, { timeout: 10_000, message: "waiting for the embedded-URL clicks" })
      .toEqual([
        { url: embeddedJs, target: "_blank", features: "noopener" },
        { url: embeddedData, target: "_blank", features: "noopener" },
      ]);
    expect(dialogs, "xterm's default confirm() must not run").toEqual([]);
  } finally {
    await cleanupSession(request, session.id);
  }
});
