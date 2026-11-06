// A terminal in a terminal host, in the real app, against a real helm and a
// supervisor started with `--new-terminals host` (this directory's
// playwright.config.ts). The stack's startup session (`e2e-session`, the fake
// agent's `basic` script) is therefore created in a host, and its terminal
// reaches the browser as a grid stream that the grid terminal component draws.
//
// The static specs in e2e/grid-client/ pin the component against recorded
// streams, and the Rust tests pin the supervisor's and the helm's halves; this
// is where all of them meet: the browser asks for a grid on its terminal
// socket, the helm relays it, and the supervisor streams the host terminal's
// emulator.
import type { Locator, Page } from "@playwright/test";
import { expect, test } from "../tests/helpers/evidence";

/** The startup session's row, matched on its whole title. */
function startupRow(page: Page): Locator {
  return page.locator(".session-row").filter({
    has: page.locator(".session-title", { hasText: /^e2e-session$/ }),
  });
}

/**
 * Count every xterm.js `Terminal` the page constructs, in
 * `window.__xtermTerminalsBuilt`, from before any script runs.
 *
 * xterm.js's bundle assigns its exports onto the global object, so an
 * accessor installed first sees the class arrive and hands terminal.js a
 * subclass that counts. The scripts still load (old terminals need them);
 * the count says whether anything used them.
 */
async function countXtermTerminals(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const w = window as any;
    w.__xtermTerminalsBuilt = 0;
    let counted: unknown;
    Object.defineProperty(w, "Terminal", {
      configurable: true,
      get: () => counted,
      set(Xterm: any) {
        counted = class extends Xterm {
          constructor(...args: unknown[]) {
            super(...args);
            w.__xtermTerminalsBuilt += 1;
          }
        };
      },
    });
  });
}

/**
 * Open the app on the startup session and wait until its terminal is the
 * grid component, mounted and focused, showing the fake agent's ready line.
 *
 * The session listing says the startup session's agent is a new terminal,
 * so terminal.js mounts the grid directly and never builds xterm.js for it:
 * `window.__farhelmGrid` with no `window.__farhelmTerm`, and no xterm.js
 * `Terminal` constructed at all while xterm.js's scripts did load, is the
 * premise that this session's terminal really is a host terminal drawn
 * without the old component.
 */
async function openGridTerminal(page: Page): Promise<Locator> {
  await countXtermTerminals(page);
  await page.goto("/");
  const row = startupRow(page);
  await expect(row).toBeVisible({ timeout: 20_000 });
  if ((await row.getAttribute("data-session-selected")) !== "true") {
    await row.locator(".session-row-open").click();
  }
  await expect
    .poll(
      () =>
        page.evaluate(() => {
          const w = window as any;
          return { grid: !!w.__farhelmGrid, xterm: !!w.__farhelmTerm };
        }),
      { timeout: 30_000, message: "waiting for the grid terminal to mount" },
    )
    .toEqual({ grid: true, xterm: false });
  const grid = page.locator(".grid-terminal");
  await expect(grid).toHaveCount(1);
  await expect(grid).toContainText("FAKE-AGENT READY", { timeout: 30_000 });
  expect(
    await page.evaluate(() => ({ loaded: typeof (window as any).Terminal === "function", built: (window as any).__xtermTerminalsBuilt })),
    "xterm.js's scripts load, and no xterm.js terminal is built for a new terminal",
  ).toEqual({ loaded: true, built: 0 });
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
 * Why: this is the first spec where a browser shows a terminal that runs in
 * a terminal host. If the browser did not ask for a grid, the helm did not
 * pass the request on, or the supervisor did not stream the host terminal,
 * the session would show nothing or an attach refusal; if keys did not
 * travel back as grid input, the terminal would draw but never answer.
 * Spec: the startup session's terminal mounts as the grid component, with
 * no xterm.js terminal ever constructed for it, draws the agent's
 * `FAKE-AGENT READY` line, and after a line is
 * typed and Enter pressed, draws the agent's `echo:` of that line. The line
 * carries the project's name, since both engines' projects share one session.
 */
test("a host terminal draws as a grid and takes typing", async ({ page }, testInfo) => {
  const grid = await openGridTerminal(page);
  const line = `typed-in-${testInfo.project.name}`;
  await page.keyboard.type(line);
  await page.keyboard.press("Enter");
  await expect(grid).toContainText(`echo:${line}`, { timeout: 15_000 });
});
