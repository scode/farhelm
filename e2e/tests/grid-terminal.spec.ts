// A terminal in a terminal host, in the real app, against a real helm and
// supervisor. A session created there runs in a host, and its terminal
// reaches the browser as a grid stream that the grid terminal component
// draws.
//
// The static specs in e2e/grid-client/ pin the component against recorded
// streams, and the Rust tests pin the supervisor's and the helm's halves; this
// is where all of them meet: the browser opens its terminal socket, the helm
// relays the grid stream, and the supervisor streams the host terminal's
// emulator.
import type { Locator, Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { cleanupSession, createSession } from "./helpers/fleet";
import { attachSession } from "./helpers/term";

/**
 * Open the app on session `id` (the fake agent's `basic` script) and wait
 * until its terminal's grid component is mounted and focused, showing the
 * fake agent's ready line.
 */
async function openGridTerminal(page: Page, id: string): Promise<Locator> {
  await page.goto("/");
  await attachSession(page, id);
  const grid = page.locator("#terminal .grid-terminal");
  await expect(grid).toHaveCount(1);
  await expect(grid).toContainText("FAKE-AGENT READY", { timeout: 30_000 });
  // Typing goes to the grid's own input element, so the focus handoff is
  // established before any key is pressed, not assumed from the attach.
  await grid.click();
  await expect(grid).toHaveClass(/\bgrid-terminal-focused\b/);
  return grid;
}

/**
 * The grid terminal draws a host terminal and the keyboard reaches its
 * program.
 *
 * Why: this is the smallest spec in which a browser shows a terminal that
 * runs in a terminal host end to end. If the helm did not relay the grid
 * stream, or the supervisor did not stream the host terminal, the session
 * would show nothing or an attach refusal; if keys did not travel back as
 * grid input, the terminal would draw but never answer.
 * Spec: a new session's terminal mounts as the grid component, draws the agent's
 * `FAKE-AGENT READY` line, and after a line is typed and Enter pressed, draws
 * the agent's `echo:` of that line. The session is the test's own and the
 * line is stamped per invocation, absent from the screen until typed, so an
 * echo left by an earlier run or repetition cannot pass for this one's.
 */
test("a host terminal draws as a grid and takes typing", async ({ page, request }) => {
  const stamp = Date.now();
  const session = await createSession(request, { title: `grid-terminal-${stamp}` });
  try {
    const grid = await openGridTerminal(page, session.id);
    const line = `typed-in-${stamp}`;
    await expect(grid, "premise: the line is not on screen before it is typed").not.toContainText(line);
    await page.keyboard.type(line);
    await page.keyboard.press("Enter");
    await expect(grid).toContainText(`echo:${line}`, { timeout: 15_000 });
  } finally {
    await cleanupSession(request, session.id);
  }
});
