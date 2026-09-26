/**
 * A terminal never takes keyboard focus from an open modal dialog.
 *
 * The terminal view focuses a terminal when its selection changes, and that
 * can happen under a dialog without the user doing anything: a selected tab
 * whose shell exits, or that another client closes, drops out and the view
 * falls back to the agent. The restart-with dialog has its own spec for this
 * (restart-with.spec.ts); this file covers the other modal dialogs, which have
 * no `inert` isolation behind them and so depend entirely on the terminal's
 * own open-dialog veto.
 */
import { expect, test } from "./helpers/evidence";
import { openRowMenu } from "./helpers/fleet";
import { attachSession, cleanupSession } from "./helpers/term";
import {
  addTab,
  armTerminalFocusSpy,
  closeTabAndAwaitAgentFallback,
  createTabSession,
  installTerminalFocusSpy,
  installTerminalSuiteHooks,
  selectTabOverRevealedAgent,
  terminalFocusCount,
} from "./helpers/terminal-suite";

// The tab sweep removes the scratch working directory `createTabSession`
// makes for this file's session.
installTerminalSuiteHooks({ tabSweep: true });

/**
 * The rename dialog keeps focus, and so the title being typed, when the
 * selected tab goes away under it.
 *
 * The rename dialog is modal but, unlike restart-with, does not mark the page
 * inert, so before the terminal's veto covered every open modal dialog the
 * agent terminal took focus here and the rest of the typed title went to the
 * agent. This specifies that with the rename field focused, removing the
 * selected tab leaves focus in the field, typing lands in the field, and the
 * agent terminal gets no focus event.
 */
test("rename keeps focus in its field when the selected tab goes away", async ({ page, request }) => {
  test.setTimeout(120_000);
  let id: string | undefined;
  try {
    const session = await createTabSession(request, `modal-focus-rename-${Date.now()}`);
    id = session.id;
    await installTerminalFocusSpy(page);

    await page.goto("/");
    await attachSession(page, id);
    const tabId = await addTab(page, 0);
    await selectTabOverRevealedAgent(page, id, tabId);

    const row = page.locator(`[data-session-id="${id}"]`);
    await openRowMenu(row);
    await row.locator(".session-row-rename").click();
    const field = page.locator(".rename-dialog .rename-input");
    await expect(field).toBeFocused();
    await field.fill("draft");
    await armTerminalFocusSpy(page);

    await closeTabAndAwaitAgentFallback(page, request, id, tabId);

    await expect(field).toBeFocused();
    await page.keyboard.type("-ok");
    await expect(field).toHaveValue("draft-ok");
    expect(
      await terminalFocusCount(page),
      "the agent terminal beneath the rename dialog must never take focus",
    ).toBe(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});
