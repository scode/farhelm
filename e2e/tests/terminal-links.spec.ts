// Browser-level coverage for OSC 8 hyperlink activation: a click on a link
// an agent's process emitted (the kind Claude Code prints for file paths
// and URLs) opens the target in a NEW tab, with no `confirm()` dialog in
// between. The dialog is what this pins against: xterm.js ships a default
// link handler that, absent a configured `linkHandler`, pops "Do you want
// to navigate to …? WARNING: This link could potentially be dangerous" on
// every click — a blanket warning the user cannot act on, and one that
// surfaced as "some popup about possibly being dangerous" in real use.
// terminal.js now configures its own handler (the `linkHandler` option at
// `new Terminal({...})`); this file is the proof the configured path is the
// one a real click takes.
//
// Only the WEB branch of that handler (`window.open`) is reachable here.
// The handler's other branch — navigating the page itself, which the
// desktop webview's navigation handler turns into a system-browser open —
// keys on the `dioxus:` page origin no browser run can present, and stays
// with manual desktop passes (it was verified by hand on the macOS build
// when the fix was written).
//
// A dedicated file, per this suite's standing convention for new coverage
// (mouse-modes.spec.ts's header): one subject, findable and runnable on
// its own. Both engines run it: link activation goes through xterm's
// `Linkifier` mouse handling, which is exactly the kind of input path the
// WebKit projects exist to cross-check against the desktop renderer family.
//
// The link is emitted through a `read`-gated invocation, for the reason
// terminal-clipboard.spec.ts's "Gated invocations" header spells out: tmux
// starts the process at session CREATION, and a link printed before this
// test attaches would only ever reach the terminal via replay — a path
// that works for OSC 8 (unlike OSC 52, the link survives in the rendered
// grid) but is not the live path this coverage is about.
import { expect, test } from "./helpers/evidence";
import { type Page } from "@playwright/test";
import { cleanupSession, createSession } from "./helpers/fleet";
import { attachSession, waitForTermText } from "./helpers/term";

/**
 * Replace `window.open` before any page script runs, recording every call
 * instead of opening anything — a headless test has no use for a real
 * second tab, and the assertion is about WHICH URL the terminal asked to
 * open, not about the browser honoring it. Registered via `addInitScript`
 * so the stub is in place before terminal.js constructs its handler, even
 * though the handler only reads `window.open` at click time.
 */
async function recordWindowOpen(page: Page): Promise<() => Promise<string[]>> {
  await page.addInitScript(() => {
    (window as any).__openedLinks = [];
    window.open = ((url: string) => {
      (window as any).__openedLinks.push(String(url));
      return null;
    }) as typeof window.open;
  });
  return () => page.evaluate(() => (window as any).__openedLinks as string[]);
}

/**
 * Click the first cell of the viewport row containing `needle` — real pixel
 * coordinates from `.xterm-screen`'s live box, the same geometry approach
 * terminal-clipboard.spec.ts's `dragRow` uses and for the same reasons
 * (the screen layer is where xterm's mouse handlers live, and the row is
 * found by scanning rather than assumed because the login shell is free to
 * print above the invocation's own output).
 *
 * Column 0 is safe to target because the gated `sh -c` invocation prints
 * the link straight after `read` returns, with no prompt in front of it.
 */
async function clickLinkRow(page: Page, needle: string): Promise<void> {
  const geometry = await page.evaluate((needleText) => {
    const t = (window as any).__farhelmTerm;
    const buf = t.buffer.active;
    for (let i = 0; i < t.rows; i++) {
      const line = buf.getLine(buf.viewportY + i);
      if (line && line.translateToString(true).includes(needleText)) {
        return { rows: t.rows, cols: t.cols, rowIndex: i };
      }
    }
    return { rows: t.rows, cols: t.cols, rowIndex: -1 };
  }, needle);
  expect(geometry.rowIndex, `no viewport row contains ${needle}`).toBeGreaterThanOrEqual(0);
  const box = (await page.locator("#terminal .xterm-screen").boundingBox())!;
  const cellWidth = box.width / geometry.cols;
  const cellHeight = box.height / geometry.rows;
  const x = box.x + 1.5 * cellWidth;
  const y = box.y + (geometry.rowIndex + 0.5) * cellHeight;
  // xterm only decorates (and activates) a link after a mousemove has
  // given its Linkifier a chance to resolve the cell under the pointer;
  // a click that teleports straight to the cell can land before that
  // resolution and fall through to plain-text handling.
  await page.mouse.move(x, y);
  await page.waitForTimeout(200);
  await page.mouse.click(x, y);
}

test("clicking an OSC 8 hyperlink opens it in a new tab with no confirm dialog", async ({
  page,
  request,
}) => {
  const openedLinks = await recordWindowOpen(page);
  const dialogs: string[] = [];
  page.on("dialog", (dialog) => {
    dialogs.push(dialog.message());
    void dialog.dismiss();
  });

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
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    await waitForTermText(page, text);

    await clickLinkRow(page, text);

    await expect
      .poll(openedLinks, { timeout: 10_000, message: "waiting for the link click to reach window.open" })
      .toEqual([url]);
    expect(dialogs, "xterm's default confirm() must not run").toEqual([]);
  } finally {
    await cleanupSession(request, session.id);
  }
});
