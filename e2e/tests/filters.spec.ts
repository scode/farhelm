// Browser evidence for the permanent, server-backed sidebar host selector.
// Rendering fewer rows alone cannot distinguish a real helm query from a
// client-side pass over a previously fetched page.
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
  await expect(page.locator(".filter-host")).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".host-row").first()).toBeVisible({ timeout: 20_000 });
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

/** Select one host and wait for its controlled native value, never incidental count copy. */
async function selectHost(page: Page, host: number | undefined): Promise<void> {
  const select = page.locator(".filter-host");
  await select.selectOption(host === undefined ? "" : String(host));
  await expect(select).toHaveValue(host === undefined ? "" : String(host), { timeout: 20_000 });
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
    await expect(page.locator(".session-count")).toHaveText(/^\d+ matching of \d+ sessions$/);
    await selectHost(page, remote!.id);
    await expect(row(page, remoteSession.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, localSession.id)).toHaveCount(0);
    await selectHost(page, undefined);
    await expect(row(page, localSession.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, remoteSession.id)).toBeVisible();
    await expect(page.locator(".session-count")).toHaveText(/^\d+ sessions$/);
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
    await expect(page.locator(".session-count")).toHaveText(/^0 matching of \d+ sessions$/);
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
    await expect(page.locator(".filter-host")).toBeVisible({ timeout: 20_000 });
    const reads = await watchListingReads(page); await selectHost(page, remote!.id); remove = true; feed.notify(2);
    const tombstone = page.locator(`.filter-host option[value="${remote!.id}"]`);
    await expect(tombstone).toContainText("no longer registered", { timeout: 20_000 });
    await expect(tombstone).toHaveJSProperty("disabled", true);
    await expect(page.locator(".filter-host")).toHaveValue(String(remote!.id));
    await expect.poll(() => reads.some((read) => read.url.searchParams.get("host") === String(remote!.id))).toBe(true);
  });

  /**
   * Registry presentation is mutable, but a filter is a stable host ID rather
   * than an option index or label. This follows one selection through every
   * registry transition that can otherwise make a native select repaint as
   * ALL: a rename, a changed phase, remote reordering, removal, and a later
   * restoration of that same ID.
   */
  test("registry changes retain the selected host ID and native option", async ({ page, request }) => {
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
    const select = page.locator(".filter-host");
    await expect(select.locator(`option[value="${local!.id}"]`)).toHaveText("This machine", { timeout: 20_000 });
    await waitForFeedReadersSettled(page);
    await select.selectOption(String(remote!.id));
    await select.focus();
    await expect(select).toBeFocused();

    registry = [
      other,
      {
        ...remote!,
        name: "renamed unavailable host",
        state: {
          phase: "unreachable-reprobing",
          cause: "connect-failed",
          last_error: "the registry fixture is unavailable",
        },
      },
      { ...local!, name: "another local alias" },
    ];
    feed.notify(2);
    await waitForFeedReadersSettled(page);
    const chosen = select.locator("option:checked");
    await expect(chosen).toHaveAttribute("value", String(remote!.id), { timeout: 20_000 });
    await expect(chosen).toContainText("renamed unavailable host");
    await expect(chosen).toContainText("unreachable, retrying");
    await expect(chosen).toHaveJSProperty("selected", true);
    await expect(select).toHaveValue(String(remote!.id));
    await expect(select).toBeFocused();
    await expect
      .poll(() => select.locator("option").evaluateAll((options) => options.map((option) => option.getAttribute("value"))))
      .toEqual(["", String(local!.id), String(other.id), String(remote!.id)]);

    registry = [{ ...local! }, other];
    feed.notify(3);
    await waitForFeedReadersSettled(page);
    await expect(chosen).toHaveAttribute("value", String(remote!.id), { timeout: 20_000 });
    await expect(chosen).toContainText("no longer registered");
    await expect(chosen).toHaveJSProperty("disabled", true);
    await expect(select).toHaveValue(String(remote!.id));
    await expect(select).toBeFocused();

    registry = [{ ...local! }, other, { ...remote!, name: "restored host" }];
    feed.notify(4);
    await waitForFeedReadersSettled(page);
    await expect(chosen).toHaveAttribute("value", String(remote!.id), { timeout: 20_000 });
    await expect(chosen).toHaveText("restored host");
    await expect(chosen).toHaveJSProperty("disabled", false);
    await expect(chosen).toHaveJSProperty("selected", true);
    await expect(select).toHaveValue(String(remote!.id));
    await expect(select).toBeFocused();
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
    const select = page.locator(".filter-host");
    await expect(select.locator('option[value="registry-unavailable"]')).toBeDisabled({ timeout: 20_000 });
    await expect(select.locator(`option[value="${local}"]`)).toHaveCount(0);
    await expect(select).toHaveValue("");
    await select.focus();
    await expect(select).toBeFocused();
    feed.notify(2);
    await expect(select).toBeFocused();

    fail = false;
    feed.notify(3);
    await expect(select.locator(`option[value="${local}"]`)).toHaveCount(1, { timeout: 20_000 });
    await select.selectOption(String(local));
    await select.focus();
    await expect(select).toBeFocused();

    fail = true;
    feed.notify(4);
    await expect(page.locator(".hosts-refresh-error")).toContainText("registry fixture refused this read", { timeout: 20_000 });
    await expect(select.locator(`option[value="${local}"]`)).toHaveCount(1);
    await expect(select).toHaveValue(String(local));
    await expect(select).toBeFocused();
  });

  /** Both native controls remain recovery tools while a listing is held or its queued replacement fails. */
  test("host selector and sort stay usable during held and failed listings", async ({ page, request }) => {
    const local = await localHostId(request);
    const listings = await holdReads(page, (url) => url.pathname === "/api/sessions");
    try {
      await page.goto("/");
      await listings.waitForCaptures(1);
      const host = page.locator(".filter-host");
      const sort = page.locator(".sort-select");
      await expect(host).toBeVisible({ timeout: 20_000 });
      await expect(sort).toBeVisible();
      await host.focus();
      await expect(host).toBeFocused();
      await host.selectOption(String(local));
      await sort.selectOption("title");
      await expect(host).toHaveValue(String(local));
      await expect(sort).toHaveValue("title");

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
      await host.selectOption("");
      await sort.selectOption("created");
      await expect(host).toHaveValue("");
      await expect(sort).toHaveValue("created");
    } finally {
      listings.releaseAll();
    }
  });

  /** Native keyboard selection and a narrow header retain ordinary browser focus behavior. */
  test("the selector keeps keyboard focus and fits a narrow header", async ({ page, request }) => {
    await localHostId(request); await page.setViewportSize({ width: 360, height: 720 }); await openList(page);
    const select = page.locator(".filter-host"); await select.focus(); await expect(select).toBeFocused();
    await select.press("ArrowDown"); await expect(select).not.toHaveValue(""); await select.press("Tab"); await expect(select).not.toBeFocused();
    const box = await page.locator(".list-header-controls").boundingBox(); expect(box).not.toBeNull();
    expect(box!.x).toBeGreaterThanOrEqual(0); expect(box!.x + box!.width).toBeLessThanOrEqual(360);
  });
});
