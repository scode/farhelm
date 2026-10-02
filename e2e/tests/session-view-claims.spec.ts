// An open session view's claim on the page's operation lock never outlives
// the view.
//
// The session view takes the window-wide operation lock for its Restart
// controls: the stop-first Restart prompt, the "Restart with" dialog, a
// Restart in flight, and the interrupted card's Replace prompt. While the
// lock is held the sidebar refuses to open another session and every other
// write action is disabled. The lock is owned by the page shell, not by the
// view, so a claim the view forgot to release survived the view going away
// (the session deleted from another client) and left the window inert until
// it was reloaded. Each test here holds one of those claims, deletes the
// session from outside the page so the view goes away, and checks the sidebar
// still opens another session.
//
// The "Restart with" dialog and the interrupted card's Replace prompt are
// covered by unit tests in crates/farhelm-ui/src/ops.rs instead. The dialog
// needs a captured conversation and the card a host reboot, neither of which
// this stack's fake agents produce, and attempts to stage them through
// stubbed listing, detail and feed routes did not get the page to re-read
// the staged session (2026-10-02), so it could not be made to go away. The
// dialog's claim takes the same path as the two Restart claims tested here
// (`PaneGate::claim_into`), and the card's prompt closes on the same
// condition that draws the card (`interrupted_card_shown` in
// session_view.rs).
import { type APIRequestContext, type Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import {
  cleanupSession,
  createSession,
  holdMutation,
} from "./helpers/fleet";
import { stackScratchDir } from "./helpers/scratch";
import { attachSession, waitForTermText } from "./helpers/term";

function row(page: Page, id: string) {
  return page.locator(`.session-row[data-session-id="${id}"]`);
}

/**
 * Two ordinary sessions to open once the claim's owner is gone. Two, because
 * the sidebar auto-selects a session when the selected one disappears, so
 * one of them may already be selected by then; the other is the one to open.
 */
async function bystanders(request: APIRequestContext, cwd: string): Promise<string[]> {
  const ids = [];
  for (const n of [1, 2]) {
    ids.push((await createSession(request, { title: `claims-bystander-${n}-${Date.now()}`, cwd })).id);
  }
  return ids;
}

/**
 * The property every test here ends on: the sidebar opens another session,
 * which it refuses while the operation lock is held. Opens whichever of `ids`
 * is not selected.
 */
async function expectSidebarOpens(page: Page, ids: string[]): Promise<void> {
  for (const id of ids) await expect(row(page, id)).toBeVisible({ timeout: 20_000 });
  const unselected = [];
  for (const id of ids) {
    if ((await row(page, id).getAttribute("data-session-selected")) !== "true") unselected.push(id);
  }
  expect(unselected.length, "at least one bystander is not selected").toBeGreaterThan(0);
  const target = row(page, unselected[0]);
  await target.locator(".session-row-open").click();
  await expect(target, "the window's write actions must not stay locked").toHaveAttribute(
    "data-session-selected",
    "true",
    { timeout: 10_000 },
  );
}

/**
 * A session deleted by another client while its view's stop-first Restart
 * prompt holds the lock leaves the sidebar usable.
 */
test("a Restart prompt's claim is released when another client deletes the session", async ({
  page,
  request,
}) => {
  const cwd = stackScratchDir("claims-restart-prompt-");
  const source = await createSession(request, { title: `claims-restart-prompt-${Date.now()}`, cwd });
  const others = await bystanders(request, cwd);
  try {
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    // Restart asks first only while the agent is working; `busy` keeps the
    // fixture's screen changing so its status stays working.
    await page.locator("#terminal").click();
    await page.keyboard.type("busy");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "busy-tick-");
    const restart = page.locator(".restart-primary");
    await expect(restart).toHaveAttribute("data-confirms", "true", { timeout: 15_000 });
    await restart.click();
    await expect(page.locator(".restart-confirm")).toBeVisible();
    // Premise, observed directly: the header's other lifecycle actions are
    // disabled while the view holds the operation lock.
    await expect(page.locator(".header-replace"), "premise: the view holds the operation lock").toBeDisabled();

    await cleanupSession(request, source.id);
    await expect(row(page, source.id)).toHaveCount(0, { timeout: 20_000 });
    await expectSidebarOpens(page, others);
  } finally {
    await cleanupSession(request, source.id);
    for (const id of others) await cleanupSession(request, id);
  }
});

/**
 * A session deleted by another client while a Restart from its view is still
 * in flight leaves the sidebar usable. The restart's reply is held so the
 * claim is certainly still owned by the running restart when the view goes.
 */
test("an in-flight Restart's claim is released when another client deletes the session", async ({
  page,
  request,
}) => {
  const cwd = stackScratchDir("claims-restart-flight-");
  const source = await createSession(request, { title: `claims-restart-flight-${Date.now()}`, cwd });
  const others = await bystanders(request, cwd);
  let release = () => {};
  try {
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    const restart = page.locator(".restart-primary");
    // An idle agent restarts at once, without a prompt.
    await expect(restart).toHaveAttribute("data-confirms", "false", { timeout: 15_000 });
    release = await holdMutation(page, (url) => url.pathname === `/api/sessions/${source.id}/restart`);
    const sent = page.waitForRequest(
      (candidate) => candidate.method() === "POST" && candidate.url().endsWith(`/api/sessions/${source.id}/restart`),
    );
    await restart.click();
    await sent;
    await expect(page.locator(".header-replace"), "premise: the restart holds the operation lock").toBeDisabled();

    await cleanupSession(request, source.id);
    await expect(row(page, source.id)).toHaveCount(0, { timeout: 20_000 });
    await expectSidebarOpens(page, others);
  } finally {
    release();
    await cleanupSession(request, source.id);
    for (const id of others) await cleanupSession(request, id);
  }
});
