/** The shared opt-out reaches every session action through the real helm.
 * Controlled status reads establish the running/interrupted screens without
 * timing a classifier or reboot; requests, guards and resulting IDs stay real.
 */
import { expect, test } from "./helpers/evidence";
import type { Page, Route } from "@playwright/test";
import {
  cleanupSession, createResumableSession, createSession, listSessions,
  openRowMenu, patchPreferences, readPreferences, stopSession,
} from "./helpers/fleet";
import { installTerminalSuiteHooks } from "./helpers/terminal-suite";
import { routeGate } from "./helpers/route-gate";

installTerminalSuiteHooks();

// The preference is helm-wide and every later spec shares this helm. A test
// whose body is abandoned (a background route error fails it without running
// its `finally`) would otherwise leave confirmations skipped for the rest of
// the suite, which a landing run saw cascade into two dozen unrelated
// failures. Null is the default every spec starts from.
test.afterEach(async ({ request }) => {
  await patchPreferences(request, { skip_session_confirmations: null });
});

/** Change only the named fixture's displayed status. Keeping the real reply's
 * launch and host facts makes each direct action reach the ordinary backend. */
async function showStatus(page: Page, id: string, state: "running" | "interrupted") {
  // End the previous page's reads before installing the rewrite. Navigating
  // with route.fetch still active can dispose that response mid-fulfilment.
  await page.goto("about:blank");
  const matches = (url: URL) => url.pathname === "/api/sessions" || url.pathname === `/api/sessions/${id}`;
  // The catch covers one lifecycle race only, as in terminal-reconnect's
  // build stamp: the page polls the list continuously, so the unroute below
  // (or teardown after a failure elsewhere) can catch a read mid-handler, and
  // `route.fulfill` then throws "Route is already handled!" from the
  // background, failing the test and abandoning its preference restore. Seen
  // once under a landing run's load in Chromium. Any other error rethrows.
  const rewrite = async (route: Route) => {
    if (route.request().method() !== "GET") return route.fallback();
    try {
      const reply = await route.fetch();
      if (!reply.ok()) return await route.fulfill({ response: reply });
      const body = await reply.json();
      for (const session of body.sessions ?? [body]) {
        if (session.id === id) session.status = { state };
      }
      await route.fulfill({ response: reply, json: body });
    } catch (e) {
      const raced = e instanceof Error
        && /Route is already handled|Target page, context or browser has been closed|Test ended/.test(e.message);
      if (!raced) throw e;
    }
  };
  await page.route(matches, rewrite);
  // Unregister and drain together: unroute alone races a pending route.fetch.
  return () => page.unrouteAll({ behavior: "wait" });
}

/** One switch must remove all seven questions immediately, persist across
 * reload, and be reversible. Each request is observed independently of the
 * disappearance of a prompt; replacing also proves selection moved to a new ID.
 */
test("skip session confirmations walks row header interrupted card and tab actions", async ({ page, request }) => {
  test.setTimeout(120_000);
  const previous = await readPreferences(request);
  const owned = new Set<string>();
  const closeGate = routeGate();
  const row = (id: string) => page.locator(`.session-row[data-session-id="${id}"]`);
  try {
    await patchPreferences(request, { skip_session_confirmations: null });
    await page.goto("/");
    const gear = page.getByRole("button", { name: "settings", exact: true });
    await gear.click();
    const settings = page.getByRole("dialog", { name: "settings", exact: true });
    const toggle = settings.getByRole("checkbox", { name: "skip session confirmations", exact: true });
    await expect(toggle).not.toBeChecked();
    await expect(settings.locator("#session-confirmations-help")).toContainText("YOLO, host and trash");
    await toggle.check();
    await expect.poll(async () => (await readPreferences(request)).skip_session_confirmations).toBe(true);
    await settings.getByRole("button", { name: "close", exact: true }).click();

    const first = await createSession(request, { title: "skip-row-delete" });
    owned.add(first.id);
    await expect(row(first.id)).toBeVisible();
    await expect(row(first.id).locator(".status-badge")).not.toHaveText(/exited|interrupted|error/);
    await openRowMenu(row(first.id));
    const deleted = page.waitForResponse((reply) => reply.request().method() === "DELETE"
      && new URL(reply.url()).pathname === `/api/sessions/${first.id}`);
    await row(first.id).locator(".session-row-delete").click();
    expect((await deleted).ok()).toBe(true);
    await expect(row(first.id)).toHaveCount(0);

    let source = await createSession(request, { title: "skip-replace" });
    owned.add(source.id);
    for (const surface of ["row", "header"] as const) {
      await expect(row(source.id)).toBeVisible();
      if (surface === "row") await openRowMenu(row(source.id));
      else await row(source.id).locator(".session-row-open").click();
      const replacement = page.waitForResponse((reply) => reply.request().method() === "POST"
        && new URL(reply.url()).pathname === `/api/sessions/${source.id}/replace`);
      await (surface === "row" ? row(source.id).locator(".session-row-replace") : page.locator(".header-replace")).click();
      const reply = await replacement;
      expect(reply.ok(), await reply.text()).toBe(true);
      const next = await reply.json();
      owned.add(next.id);
      expect(next.id).not.toBe(source.id);
      await expect(row(source.id)).toHaveCount(0);
      await expect(row(next.id)).toHaveAttribute("data-session-selected", "true");
      source = next;
    }
    await expect(page.locator(".header-delete")).toHaveAttribute("data-tooltip", "delete: remove this session and its state");
    const headerDeleted = page.waitForResponse((reply) => reply.request().method() === "DELETE"
      && new URL(reply.url()).pathname === `/api/sessions/${source.id}`);
    await page.locator(".header-delete").click();
    expect((await headerDeleted).ok()).toBe(true);
    await expect(row(source.id)).toHaveCount(0);

    const resumable = await createResumableSession(request, { title: "skip-restart-tabs" });
    owned.add(resumable.id);
    const stopShowingRunning = await showStatus(page, resumable.id, "running");
    await page.goto("/");
    await row(resumable.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .status-badge")).toHaveClass(/\brunning\b/);
    const restart = page.locator(".restart-primary");
    await expect(restart).not.toHaveAttribute("aria-disabled", "true");
    await expect(restart).toHaveAttribute("data-confirms", "false");
    const restarted = page.waitForResponse((reply) => reply.request().method() === "POST"
      && new URL(reply.url()).pathname === `/api/sessions/${resumable.id}/restart`);
    await restart.click();
    const restartReply = await restarted;
    expect(restartReply.request().postDataJSON().stop_if_running).toBe(true);
    expect(restartReply.ok(), await restartReply.text()).toBe(true);
    await expect(page.locator(".restart-confirm")).toHaveCount(0);
    await stopShowingRunning();
    const tabReply = await request.post(`/api/sessions/${resumable.id}/tabs`);
    expect(tabReply.ok(), await tabReply.text()).toBe(true);
    const { tab } = await tabReply.json();
    expect(tab.id, "the tab-open reply names the actual tab").toEqual(expect.any(String));
    const tabItem = page.locator(`.tab-slot[data-tab-id="${tab.id}"]`);
    await expect(tabItem).toBeVisible();
    const closesTab = (url: URL) => url.pathname === `/api/sessions/${resumable.id}/tabs/${tab.id}`;
    await page.route(closesTab, async (route) => {
      if (route.request().method() !== "DELETE") return route.fallback();
      await closeGate.wait();
      await route.continue();
    });
    const closeReached = page.waitForRequest((candidate) => candidate.method() === "DELETE"
      && closesTab(new URL(candidate.url())));
    const closed = page.waitForResponse((reply) => reply.request().method() === "DELETE"
      && new URL(reply.url()).pathname === `/api/sessions/${resumable.id}/tabs/${tab.id}`);
    await tabItem.locator(".tab-close").click();
    await closeReached;
    // Observe the actual prompt selector while the tab is still listed and
    // its DELETE is held. Success would otherwise hide a transient prompt.
    await expect(tabItem).toBeVisible();
    await expect(page.locator(".tab-confirm")).toHaveCount(0);
    closeGate.release();
    expect((await closed).ok()).toBe(true);
    await expect(tabItem).toHaveCount(0);

    const interrupted = await createSession(request, { title: "skip-interrupted-replace" });
    owned.add(interrupted.id);
    await stopSession(request, interrupted.id);
    // The premise is only that nothing is alive before the rewrite below
    // shows it as interrupted. A stop that lands before the fresh launch has
    // been confirmed settles as "error" rather than "exited" (seen once in
    // WebKit under a landing run's load); both are ended agents, and the
    // stop returns before either reaches the helm's cache, hence the wait.
    await expect
      .poll(
        async () => (await listSessions(request)).sessions.find((one) => one.id === interrupted.id)?.status.state,
        { timeout: 20_000 },
      )
      .toMatch(/^(exited|error)$/);
    const stopShowingInterrupted = await showStatus(page, interrupted.id, "interrupted");
    await page.goto("/");
    await row(interrupted.id).locator(".session-row-open").click();
    await expect(page.locator(".interrupted-card")).toBeVisible();
    const replaced = page.waitForResponse((reply) => reply.request().method() === "POST"
      && new URL(reply.url()).pathname === `/api/sessions/${interrupted.id}/replace`);
    await page.locator(".replace-from-notice").click();
    const cardReply = await replaced;
    expect(cardReply.request().postDataJSON().only_if_nothing_alive).toBe(true);
    expect(cardReply.ok(), await cardReply.text()).toBe(true);
    const cardReplacement = await cardReply.json();
    owned.add(cardReplacement.id);
    await expect(row(interrupted.id)).toHaveCount(0);
    await expect(row(cardReplacement.id)).toHaveAttribute("data-session-selected", "true");
    await stopShowingInterrupted();

    // Reload seeded the stored opt-out above; now reversing it must affect
    // the next header click without another reload.
    await gear.click();
    await expect(toggle).toBeChecked();
    await toggle.uncheck();
    await expect.poll(async () => (await readPreferences(request)).skip_session_confirmations).toBe(false);
    await settings.getByRole("button", { name: "close", exact: true }).click();
    await page.locator(".header-delete").click();
    await expect(page.locator(".header-delete-confirm")).toBeVisible();
    await page.locator(".header-delete-confirm .btn-neutral").click();
  } finally {
    closeGate.release();
    try {
      await page.unrouteAll({ behavior: "wait" });
      const results = await Promise.allSettled([...owned].map((id) => cleanupSession(request, id)));
      const failed = results.filter((result) => result.status === "rejected");
      expect(failed, "all owned sessions must be cleaned up").toEqual([]);
    } finally {
      await patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null });
    }
  }
});
