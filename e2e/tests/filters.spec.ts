// Browser evidence for the permanent, server-backed sidebar host rail.
// Rendering fewer rows alone cannot distinguish a real helm query from a
// client-side pass over a previously fetched page.
import { chooseSort } from "./helpers/list-header";
import { chooseHost, hostChoice, selectedHost } from "./helpers/host-rail";
import { expect, test } from "./helpers/evidence";
import { Page, Route } from "@playwright/test";
import { cleanupSession, createSession, holdReads, listHosts, listSessions, localHostId, observeFeedReaders, SessionPage, stubFeed, waitForFeedReadersSettled } from "./helpers/fleet";

/** Return the rendered list row for one stable session identity. */
function row(page: Page, id: string) { return page.locator(`[data-session-id="${id}"]`); }

/** Arrive after registry and list reads make the permanent control usable without moving focus. */
async function openList(page: Page): Promise<void> {
  const feed = await stubFeed(page);
  await page.goto("/");
  await feed.waitForConnection(1);
  feed.notify(1);
  await expect(hostChoice(page)).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".host-rail-avatar").first()).toBeVisible({ timeout: 20_000 });
}

interface ListingRead { url: URL; body: SessionPage & { matching?: number; total?: number }; }

/** Preserve the fetched response so observations are of the bytes ListView decoded. */
async function watchListingReads(page: Page): Promise<ListingRead[]> {
  const reads: ListingRead[] = [];
  await page.route((url) => url.pathname === "/api/sessions", async (route: Route) => {
    if (route.request().method() !== "GET") return route.continue();
    const response = await route.fetch();
    reads.push({ url: new URL(route.request().url()), body: (await response.json()) as ListingRead["body"] });
    await route.fulfill({ response });
  });
  return reads;
}

/** Choose the real rail control; listing assertions below prove server membership. */
async function selectHost(page: Page, host: number | undefined): Promise<void> {
  await chooseHost(page, host);
}

test.describe("permanent host filtering", () => {
  const created: string[] = [];
  test.afterEach(async ({ request }) => { while (created.length) await cleanupSession(request, created.pop()!); });

  /** Local, remote, and ALL selections prove server membership, replies, rows, and count wording together. */
  test("selecting local, remote, and ALL uses helm membership and counts", async ({ page, request }) => {
    const local = await localHostId(request);
    const remote = (await listHosts(request)).find((host) => host.id !== local);
    expect(remote, "the e2e fleet needs a configured remote host").toBeTruthy();
    const stamp = Date.now();
    const localSession = await createSession(request, { title: `host-local-${stamp}`, host: local });
    const remoteSession = await createSession(request, { title: `host-remote-${stamp}`, host: remote!.id });
    created.push(localSession.id, remoteSession.id);
    const localReply = await listSessions(request, `host=${local}`);
    const remoteReply = await listSessions(request, `host=${remote!.id}`);
    expect(localReply.sessions.some((s) => s.id === localSession.id)).toBe(true);
    expect(localReply.sessions.some((s) => s.id === remoteSession.id)).toBe(false);
    expect(remoteReply.sessions.some((s) => s.id === remoteSession.id)).toBe(true);
    expect(remoteReply.sessions.some((s) => s.id === localSession.id)).toBe(false);
    await openList(page);
    await expect(row(page, localSession.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, remoteSession.id)).toBeVisible();
    const reads = await watchListingReads(page);
    await selectHost(page, local);
    await expect(row(page, localSession.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, remoteSession.id)).toHaveCount(0);
    await expect(page.locator(".session-count")).toHaveText(/^\d+ of \d+$/);
    await selectHost(page, remote!.id);
    await expect(row(page, remoteSession.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, localSession.id)).toHaveCount(0);
    await selectHost(page, undefined);
    await expect(row(page, localSession.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, remoteSession.id)).toBeVisible();
    await expect(page.locator(".session-count")).toHaveText(/^\d+$/);
    const localReads = reads.filter((read) => read.url.searchParams.get("host") === String(local));
    const remoteReads = reads.filter((read) => read.url.searchParams.get("host") === String(remote!.id));
    const allReads = reads.filter((read) => !read.url.searchParams.has("host"));
    expect(localReads.length).toBeGreaterThan(0); expect(remoteReads.length).toBeGreaterThan(0); expect(allReads.length).toBeGreaterThan(0);
    expect(localReads.every((read) => read.body.sessions.some((s) => s.id === localSession.id) && !read.body.sessions.some((s) => s.id === remoteSession.id))).toBe(true);
    expect(remoteReads.every((read) => read.body.sessions.some((s) => s.id === remoteSession.id) && !read.body.sessions.some((s) => s.id === localSession.id))).toBe(true);
  });

  /** Empty matches are a host-query result, not the empty-fleet placeholder. */
  test("a known empty remote selection says that the host filter matched nothing", async ({ page, request }) => {
    const local = await localHostId(request);
    const remote = (await listHosts(request)).find((host) => host.id !== local);
    expect(remote).toBeTruthy();
    expect((await listSessions(request, `host=${remote!.id}`)).sessions, "the empty fixture must be asserted before selection").toHaveLength(0);
    await openList(page); await selectHost(page, remote!.id);
    await expect(page.locator(".filter-empty")).toBeVisible({ timeout: 20_000 });
    await expect(page.locator(".session-count")).toHaveText(/^0 of \d+$/);
  });

  /** A removed selected host remains a disabled tombstone so its query cannot silently widen to ALL. */
  test("a removed selected host remains a named tombstone and query", async ({ page, request }) => {
    const local = await localHostId(request);
    const remote = (await listHosts(request)).find((host) => host.id !== local);
    expect(remote).toBeTruthy(); let remove = false;
    await page.route((url) => url.pathname === "/api/hosts", async (route: Route) => {
      if (route.request().method() !== "GET") return route.continue();
      const response = await route.fetch(); const body = await response.json();
      if (remove) body.hosts = body.hosts.filter((host: { id: number }) => host.id !== remote!.id);
      await route.fulfill({ response, json: body });
    });
    const feed = await stubFeed(page); await page.goto("/"); await feed.waitForConnection(1); feed.notify(1);
    await expect(hostChoice(page)).toBeVisible({ timeout: 20_000 });
    const reads = await watchListingReads(page); await selectHost(page, remote!.id); remove = true; feed.notify(2);
    const tombstone = page.locator(`.host-rail-tombstone[data-rail-host-id="${remote!.id}"]`);
    await expect(tombstone).toHaveAttribute("aria-label", /no longer registered/, { timeout: 20_000 });
    await expect(tombstone).toHaveJSProperty("disabled", true);
    await expect(tombstone).toHaveAttribute("aria-pressed", "true");
    await expect.poll(() => reads.some((read) => read.url.searchParams.get("host") === String(remote!.id))).toBe(true);
  });

  /**
   * Registry presentation is mutable, but a filter is a stable host ID rather
   * than an index or label. Same-order presentation updates retain avatar
   * focus; structural changes retain focus on persistent All while the host
   * choice survives reordering, removal and restoration of that same ID.
   */
  test("registry changes retain the selected host ID and rail entry", async ({ page, request }) => {
    const reply = await request.get("/api/hosts");
    const stamp = reply.headers()["x-farhelm-build"] ?? "";
    const initial = await reply.json();
    const local = initial.hosts.find((host: { kind: string }) => host.kind === "local");
    const remote = initial.hosts.find((host: { kind: string }) => host.kind !== "local");
    expect(stamp, "fabricated registry replies must retain the helm build stamp").toBeTruthy();
    expect(local, "the fixture needs the registered local host").toBeTruthy();
    expect(remote, "the fixture needs a configured remote host").toBeTruthy();

    const other = { ...remote!, id: remote!.id + 90_000, name: "other configured host" };
    let registry = [{ ...local!, name: "a local alias that must not relabel this machine" }, remote!, other];
    await page.route((url) => url.pathname === "/api/hosts", async (route: Route) => {
      if (route.request().method() !== "GET") return route.continue();
      await route.fulfill({
        headers: { "content-type": "application/json", "x-farhelm-build": stamp },
        json: { hosts: registry },
      });
    });

    // The handshake schedules its own host read. Settle it before changing
    // the routed registry, so the next notification is the transition this
    // test names rather than work already owed by page arrival.
    await observeFeedReaders(page);
    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    const all = hostChoice(page);
    await expect(hostChoice(page, local!.id)).toHaveAttribute("aria-label", /^This machine:/, { timeout: 20_000 });
    await waitForFeedReadersSettled(page);
    await chooseHost(page, remote!.id);
    const selectedAvatar = hostChoice(page, remote!.id);
    await selectedAvatar.focus();
    await expect(selectedAvatar).toBeFocused();

    registry = [
      { ...local!, name: "another local alias" },
      {
        ...remote!,
        name: "renamed unavailable host",
        state: {
          phase: "unreachable-reprobing",
          cause: "connect-failed",
          last_error: "the registry fixture is unavailable",
        },
      },
      other,
    ];
    feed.notify(2);
    await waitForFeedReadersSettled(page);
    const chosen = selectedHost(page);
    await expect(chosen).toHaveAttribute("data-rail-host-id", String(remote!.id), { timeout: 20_000 });
    await expect(chosen).toHaveAttribute("aria-label", /renamed unavailable host.*Unreachable, retrying/);
    await expect(chosen).toHaveAttribute("aria-pressed", "true");
    await expect(selectedAvatar).toBeFocused();
    await expect(hostChoice(page, local!.id)).toHaveAttribute("aria-label", /^This machine:/);

    // Moving a keyed avatar may retire its DOM focus. Establish a persistent
    // focus owner before structural transitions, without activating All or
    // changing the selected host filter.
    await all.focus();
    await expect(all).toBeFocused();
    registry = [other, registry[1], registry[0]];
    feed.notify(3);
    await waitForFeedReadersSettled(page);
    await expect
      .poll(() => page.locator(".host-rail-avatar").evaluateAll((buttons) => buttons.map((button) => button.getAttribute("data-rail-host-id"))))
      .toEqual([String(other.id), String(remote!.id), String(local!.id)]);
    await expect(chosen).toHaveAttribute("data-rail-host-id", String(remote!.id));
    await expect(chosen).toHaveAttribute("aria-pressed", "true");
    await expect(all).toBeFocused();
    registry = [{ ...local! }, other];
    feed.notify(4);
    await waitForFeedReadersSettled(page);
    await expect(chosen).toHaveAttribute("data-rail-host-id", String(remote!.id), { timeout: 20_000 });
    await expect(chosen).toHaveAttribute("aria-label", /no longer registered/);
    await expect(chosen).toHaveJSProperty("disabled", true);
    await expect(all).toBeFocused();

    registry = [{ ...local! }, other, { ...remote!, name: "restored host" }];
    feed.notify(5);
    await waitForFeedReadersSettled(page);
    await expect(chosen).toHaveAttribute("data-rail-host-id", String(remote!.id), { timeout: 20_000 });
    await expect(chosen).toHaveAttribute("aria-label", /^restored host:/);
    await expect(chosen).toHaveJSProperty("disabled", false);
    await expect(chosen).toHaveAttribute("aria-pressed", "true");
    await expect(all).toBeFocused();
  });

  /**
   * A missing first registry reply must leave ALL distinct from the local
   * placeholder. A later refresh error is different: the last trusted
   * registry remains selectable, and neither transition is allowed to take
   * keyboard focus from the native control.
   */
  test("unavailable and retained registry states neither guess local nor steal focus", async ({ page, request }) => {
    const stamp = (await request.get("/api/sessions")).headers()["x-farhelm-build"] ?? "";
    const local = await localHostId(request);
    expect(stamp, "fabricated failures must retain the helm build stamp").toBeTruthy();
    let fail = true;
    await page.route((url) => url.pathname === "/api/hosts", async (route: Route) => {
      if (route.request().method() !== "GET") return route.continue();
      if (!fail) return route.continue();
      await route.fulfill({
        status: 503,
        headers: { "content-type": "text/plain", "x-farhelm-build": stamp },
        body: "registry fixture refused this read",
      });
    });
    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    const all = hostChoice(page);
    await expect(page.locator(".host-rail-unavailable")).toBeVisible({ timeout: 20_000 });
    await expect(hostChoice(page, local)).toHaveCount(0);
    await expect(all).toHaveAttribute("aria-pressed", "true");
    await all.focus();
    await expect(all).toBeFocused();
    feed.notify(2);
    await expect(all).toBeFocused();

    fail = false;
    feed.notify(3);
    const localAvatar = hostChoice(page, local);
    await expect(localAvatar).toHaveCount(1, { timeout: 20_000 });
    await chooseHost(page, local);
    await localAvatar.focus();
    await expect(localAvatar).toBeFocused();

    fail = true;
    feed.notify(4);
    await expect(page.locator(".hosts-refresh-error")).toContainText("registry fixture refused this read", { timeout: 20_000 });
    await expect(localAvatar).toHaveCount(1);
    await expect(localAvatar).toHaveAttribute("aria-pressed", "true");
    await expect(localAvatar).toHaveAttribute("aria-label", /last state read, host list refresh failed/);
    await expect(localAvatar).toBeFocused();
  });

  /** Host and sort choices remain recovery tools while a listing is held or its queued replacement fails. */
  test("host selector and sort stay usable during held and failed listings", async ({ page, request }) => {
    const local = await localHostId(request);
    const listings = await holdReads(page, (url) => url.pathname === "/api/sessions");
    try {
      await page.goto("/");
      await listings.waitForCaptures(1);
      const host = hostChoice(page);
      const sort = page.locator(".sort-title");
      await expect(host).toBeVisible({ timeout: 20_000 });
      await expect(sort).toBeVisible();
      await host.focus();
      await expect(host).toBeFocused();
      await chooseHost(page, local);
      await chooseSort(page, "title");
      await expect(hostChoice(page, local)).toHaveAttribute("aria-pressed", "true");
      await expect(sort).toHaveAttribute("data-sort", "title");

      listings.release(1);
      await listings.waitForCaptures(2);
      listings.release(2, { status: 503, body: "held host listing failed" });
      // Establish the failure before changing the query. A later host or
      // sort action legitimately makes this reply stale, so visibility alone
      // would only prove that the controls survived loading.
      await expect(page.locator(".status.error"))
        .toContainText("held host listing failed", { timeout: 20_000 });
      await expect(host).toBeVisible({ timeout: 20_000 });
      await expect(sort).toBeVisible();
      await chooseHost(page);
      await chooseSort(page, "created");
      await expect(host).toHaveAttribute("aria-pressed", "true");
      await expect(sort).toHaveAttribute("data-sort", "created");
    } finally {
      listings.releaseAll();
    }
  });

  /** Native buttons expose selection through keyboard activation in the narrow rail. */
  test("the rail keeps keyboard focus and fits a narrow window", async ({ page, request }) => {
    const local = await localHostId(request); await page.setViewportSize({ width: 360, height: 720 }); await openList(page);
    const all = hostChoice(page); await all.focus(); await expect(all).toBeFocused();
    await all.press("Tab");
    const first = page.locator(".host-rail-avatar").first();
    await expect(first).toBeFocused(); await first.press("Space");
    await expect(first).toHaveAttribute("aria-pressed", "true");
    await hostChoice(page, local).focus(); await expect(hostChoice(page, local)).toBeFocused();
    const box = await page.locator(".host-rail").boundingBox(); expect(box).not.toBeNull();
    expect(box!.x).toBeGreaterThanOrEqual(0); expect(box!.x + box!.width).toBeLessThanOrEqual(360);
  });

  /** Both context gestures use the row's existing action set and return to
   * the avatar that supplied the actual anchor, without narrowing the list. */
  test("rail context menus share host actions and restore their opener", async ({ page, request }) => {
    const local = await localHostId(request);
    const remote = (await listHosts(request)).find((host) => host.id !== local);
    expect(remote, "the fixture needs a remote row whose menu can be compared").toBeTruthy();
    await openList(page);
    const avatar = hostChoice(page, remote!.id);
    const management = page.locator(`.host-row[data-host-id="${remote!.id}"]`);
    await expect(management).toHaveCount(1);
    await expect(management.locator(".host-row-main")).not.toBeVisible();
    await expect(avatar).toBeVisible();
    await avatar.focus(); await expect(avatar).toBeFocused();
    await avatar.press("Shift+F10");
    const menu = management.locator('[role="menu"]');
    await expect(menu).toBeVisible();
    const flyout = management.locator(".host-row-menu-flyout");
    await expect(flyout).toHaveCSS("opacity", "1");
    const anchor = (await avatar.boundingBox())!;
    expect((await flyout.boundingBox())!.x, "the menu measures the avatar, not the management toggle").toBeCloseTo(anchor.x + anchor.width + 10, 0);
    await expect(menu.getByRole("menuitem").first()).toBeFocused();
    await expect(hostChoice(page)).toHaveAttribute("aria-pressed", "true");
    await page.keyboard.press("Escape");
    await expect(menu).toHaveCount(0); await expect(avatar).toBeFocused();
    await avatar.click({ button: "right" });
    await expect(menu).toBeVisible();
    await page.locator(".app-main").click({ position: { x: 150, y: 150 } });
    await expect(menu).toHaveCount(0);
  });

  /** A short window makes the avatar list genuinely scroll; footer actions
   * must remain reachable independently of that navigation position. */
  test("rail scrolling leaves settings and help reachable", async ({ page }) => {
    await page.setViewportSize({ width: 900, height: 240 }); await openList(page);
    const rail = page.locator(".host-rail");
    await expect.poll(() => rail.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(true);
    await rail.evaluate((element) => { element.scrollTop = element.scrollHeight; });
    for (const selector of [".app-settings-toggle", ".app-help-toggle"]) {
      const control = page.locator(selector); await control.focus(); await expect(control).toBeFocused();
      const box = (await control.boundingBox())!;
      expect(box.y).toBeGreaterThanOrEqual(0); expect(box.y + box.height).toBeLessThanOrEqual(240);
    }
  });

  /** Footer Help uses its actual bottom anchor, including a short viewport.
   * A fixed reserve for taller session menus leaves its pointer detached. */
  test("rail Help menu points at its footer button", async ({ page }) => {
    for (const height of [700, 240]) {
      await page.setViewportSize({ width: 900, height });
      await openList(page);
      const toggle = page.locator(".app-help-toggle");
      await toggle.click();
      await expect(toggle).toHaveAttribute("aria-expanded", "true");
      const flyout = page.locator('.bar-menu-flyout[data-bar-menu="help"]');
      await expect(flyout).toHaveCSS("opacity", "1");
      const button = (await toggle.boundingBox())!;
      const pointer = (await flyout.locator(".host-row-menu-pointer").boundingBox())!;
      const panel = (await flyout.boundingBox())!;
      expect(Math.abs(pointer.y + pointer.height / 2 - button.y - button.height / 2)).toBeLessThan(8);
      expect(panel.y).toBeGreaterThanOrEqual(7);
      expect(panel.y + panel.height).toBeLessThanOrEqual(height);
      await page.keyboard.press("Escape");
      await expect(toggle).toBeFocused();
    }
  });

  for (const action of ["settings", "add"] as const) {
    /** A rail-launched dialog must not hand keyboard focus to an offscreen
     * management control. Only inert height is synthetic; every trigger,
     * dialog and return-focus path belongs to the application. */
    test(`rail ${action} dialog restores its visible opener`, async ({ page, request }) => {
      await page.setViewportSize({ width: 1100, height: 700 });
      await openList(page);
      const local = await localHostId(request);
      const remote = (await listHosts(request)).find((host) => host.id !== local);
      expect(remote, "the settings case needs the registered remote").toBeDefined();
      const content = page.locator(".sidebar-content");
      await content.evaluate((element) => {
        const filler = document.createElement("div");
        filler.style.height = "1200px";
        element.append(filler);
        element.scrollTop = element.scrollHeight;
      });
      expect(await content.evaluate((element) => element.scrollTop)).toBeGreaterThan(400);
      const opener = action === "settings" ? hostChoice(page, remote!.id) : page.locator(".host-rail-add");
      await opener.focus();
      await expect(opener).toBeFocused();
      if (action === "settings") {
        await opener.press("Shift+F10");
        const menu = page.locator(`.host-row[data-host-id="${remote!.id}"] [role="menu"]`);
        await expect(menu).toBeVisible();
        await menu.locator(".host-settings").click();
        await expect(page.locator(".host-settings-dialog")).toBeVisible();
        await page.locator(".host-settings-dialog .host-settings-close").click();
      } else {
        await opener.press("Enter");
        await expect(page.locator(".host-add-dialog")).toBeVisible();
        await page.locator(".host-add-dialog .add-host-cancel").click();
      }
      await expect(page.locator('.host-settings-dialog[role="dialog"]')).toHaveCount(0);
      await expect(opener).toBeFocused();
      expect(await content.evaluate((element) => element.scrollTop), "focus return does not scroll the old management target into view").toBeGreaterThan(400);
    });
  }
});
