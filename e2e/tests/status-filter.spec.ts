/** Status choices must be real helm queries, while navigation stays independent of list membership. */
import { expect, test } from "./helpers/evidence";
import { cleanupSession, createSession, listHosts, listSessions, localHostId, pinAutoSelect, resetPreferences, stubFeed } from "./helpers/fleet";
import { attachSession } from "./helpers/term";
import { chooseStatus } from "./helpers/list-header";
import { chooseHost, hostChoice } from "./helpers/host-rail";
import { cleanupOwnedSteps } from "./helpers/cleanup";

const owned: string[] = [];
test.afterEach(async ({ request }, testInfo) => {
  await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
    ...owned.splice(0).reverse().map((id): [string, () => Promise<void>] => ["owned session", () => cleanupSession(request, id)]),
    ["preferences", () => resetPreferences(request)],
  ]);
});

/** Real quiet and ended agents establish distinct membership before the control is exercised.
 * A filtered-out selection must keep its terminal, and native arrows must issue the same query as a click.
 */
test("status segments query the helm, count matches and retain a hidden terminal", async ({ page, request }) => {
  const live = await createSession(request, { title: `status-live-${Date.now()}` });
  owned.push(live.id);
  const ended = await createSession(request, { title: `status-ended-${Date.now()}`, invocation: "true" });
  owned.push(ended.id);
  await expect.poll(async () => {
    const rows = (await listSessions(request)).sessions;
    return [rows.find((s) => s.id === live.id)?.status?.state, rows.find((s) => s.id === ended.id)?.status?.state];
  }, { timeout: 20_000, message: "the real sampler must classify the quiet agent and ended command" }).toEqual(["idle", "exited"]);
  const reads: { status: string | null; states: string[] }[] = [];
  await page.route((url) => url.pathname === "/api/sessions", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    reads.push({ status: new URL(route.request().url()).searchParams.get("status"), states: body.sessions.map((s: any) => s.status.state) });
    await route.fulfill({ response });
  });
  await pinAutoSelect(page, live.id);
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  await page.goto("/");
  await attachSession(page, live.id);
  const terminal = page.locator("#terminal");
  await expect(terminal).toBeVisible();
  await chooseStatus(page, "Idle");
  await expect(page.locator(`[data-session-id="${live.id}"]`)).toBeVisible();
  await expect(page.locator(`[data-session-id="${ended.id}"]`)).toHaveCount(0);
  await expect(page.locator(".session-count")).toHaveText(/^\d+ of \d+$/);
  await expect.poll(() => reads.some((read) => read.status === "idle" && read.states.length > 0 && read.states.every((state) => state === "idle"))).toBe(true);
  const idle = page.getByRole("radio", { name: "Idle", exact: true });
  await idle.focus();
  await expect(idle).toBeFocused();
  await idle.press("ArrowLeft");
  await expect(page.getByRole("radio", { name: "Waiting", exact: true })).toBeChecked();
  await expect(page.locator(".filter-empty")).toBeVisible();
  await expect(page.locator(`[data-session-id="${live.id}"]`)).toHaveCount(0);
  await expect(terminal).toBeVisible();
  await expect(page.locator(".session-details")).toContainText(live.title);
  await expect.poll(() => reads.some((read) => read.status === "waiting" && read.states.length === 0)).toBe(true);
  await chooseStatus(page, "All");
  await expect(page.locator(`[data-session-id="${live.id}"]`)).toHaveAttribute("data-session-selected", "true");
  await expect(page.locator(`[data-session-id="${ended.id}"]`)).toBeVisible();
  await expect(page.locator(".session-count")).toHaveText(/^\d+$/);
});

for (const hiddenBy of ["status alone", "host and status", "neither"] as const) {
  /** Only hidden fleet picks widen the sidebar; a visible pick preserves both choices. */
  test(`a switcher pick hidden by ${hiddenBy} ${hiddenBy === "neither" ? "preserves" : "resets"} both choices`, async ({ page, request }) => {
    await page.addInitScript(() => Object.defineProperty(Navigator.prototype, "platform", { get: () => "Linux x86_64" }));
    const local = await localHostId(request);
    const remote = (await listHosts(request)).find((host) => host.id !== local);
    expect(remote, "the owned fleet includes a remote for the combined query").toBeDefined();
    const live = await createSession(request, { title: `status-origin-${Date.now()}`, host: local });
    owned.push(live.id);
    const hidden = hiddenBy !== "neither";
    const target = await createSession(request, { title: `status-target-${Date.now()}`, host: hiddenBy === "host and status" ? remote!.id : local,
      ...(hidden ? { invocation: "true" } : {}) });
    owned.push(target.id);
    await expect.poll(async () => (await listSessions(request)).sessions.find((s) => s.id === target.id)?.status?.state,
      { timeout: 20_000, message: "the target must establish membership before the switcher opens" }).toBe(hidden ? "exited" : "idle");
    await pinAutoSelect(page, live.id);
    await page.goto("/");
    await attachSession(page, live.id);
    await chooseHost(page, local);
    await chooseStatus(page, "Idle");
    if (hidden) await expect(page.locator(`[data-session-id="${target.id}"]`)).toHaveCount(0);
    else await expect(page.locator(`[data-session-id="${target.id}"]`)).toBeVisible();
    await page.keyboard.press("Control+Shift+k");
    const dialog = page.getByRole("dialog", { name: "quick switcher" });
    await expect(dialog.getByRole("combobox")).toBeFocused();
    await dialog.getByRole("combobox").fill(target.title);
    await expect(dialog.locator(".quick-switcher-results").getByRole("option")).toHaveCount(1);
    await page.keyboard.press("Enter");
    await expect(dialog).toHaveCount(0);
    await expect(page.getByRole("radio", { name: hidden ? "All" : "Idle", exact: true })).toBeChecked();
    await expect(hostChoice(page, hidden ? undefined : local)).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator(`[data-session-id="${target.id}"]`)).toHaveAttribute("data-session-selected", "true");
  });
}

for (const [before, after] of [["idle", "running"], ["running", "idle"]] as const) {
  /** A switcher snapshot can outlive a status transition already admitted by the sidebar.
   * Controlled statuses isolate that handoff; the membership test above owns
   * real helm filtering. Every response here retains the real build stamp.
   */
  test(`a current ${after} sidebar reply overrides the switcher's old ${before} status`, async ({ page, request }) => {
    await page.addInitScript(() => Object.defineProperty(Navigator.prototype, "platform", { get: () => "Linux x86_64" }));
    const live = await createSession(request, { title: `stale-origin-${Date.now()}` });
    owned.push(live.id);
    const target = await createSession(request, { title: `stale-target-${Date.now()}` });
    owned.push(target.id);
    let targetStatus: string = before;
    await page.route((url) => url.pathname === "/api/sessions", async (route) => {
      const query = new URL(route.request().url());
      const wanted = query.searchParams.get("status");
      query.searchParams.delete("status");
      const response = await route.fetch({ url: query.toString() });
      const body = await response.json();
      expect(body.truncated, "the owned handoff fixture must have complete membership").toBe(false);
      for (const row of body.sessions) if (row.id === target.id) row.status = { state: targetStatus };
      if (wanted) body.sessions = body.sessions.filter((row: any) => row.status.state === wanted);
      body.matching = body.sessions.length;
      await route.fulfill({ response, json: body });
    });
    await pinAutoSelect(page, live.id);
    const feed = await stubFeed(page);
    feed.notifyOnConnect(1);
    await page.goto("/");
    await attachSession(page, live.id);
    await chooseStatus(page, "Idle");
    const row = page.locator(`[data-session-id="${target.id}"]`);
    if (before === "idle") await expect(row).toBeVisible();
    else await expect(row).toHaveCount(0);
    await page.keyboard.press("Control+Shift+k");
    const dialog = page.getByRole("dialog", { name: "quick switcher" });
    await expect(dialog.getByRole("combobox")).toBeFocused();
    await dialog.getByRole("combobox").fill(target.title);
    const option = dialog.locator(".quick-switcher-results").getByRole("option");
    await expect(option).toHaveCount(1);
    await expect(option.locator(`.status-badge.${before}`)).toBeVisible();
    targetStatus = after;
    feed.notify(2);
    if (after === "idle") await expect(row).toBeVisible();
    else await expect(row).toHaveCount(0);
    await expect(option.locator(`.status-badge.${before}`), "the switcher still holds its older snapshot").toBeVisible();
    await page.keyboard.press("Enter");
    await expect(dialog).toHaveCount(0);
    await expect(page.getByRole("radio", { name: after === "idle" ? "Idle" : "All", exact: true })).toBeChecked();
    await expect(row).toHaveAttribute("data-session-selected", "true");
  });
}
