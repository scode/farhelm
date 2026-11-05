import { chooseSort } from "./helpers/list-header";
import type { APIRequestContext, Page } from "@playwright/test";
// A real menu write and a second client prove shared pins; DOM positions
// prove the sidebar partition rather than a changed helm/quick-switcher sort.
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { resolveToken } from "./helpers/color";
import {
  cleanupSession,
  createSession,
  listSessions,
  openRowMenu,
  patchPreferences,
  renameSession,
  resetPreferences,
  stubFeed,
} from "./helpers/fleet";

/** Only owned rows count: unrelated sessions can appear between unpinned fixtures on the shared stack. */
async function ownedOrder(page: Page, ids: string[]): Promise<string[]> {
  return await page.locator(".session-row").evaluateAll(
    (rows, owned) => rows.map((row) => row.getAttribute("data-session-id") ?? "").filter((id) => owned.includes(id)),
    ids,
  );
}

/** Metadata setup uses the same protected API as the menu; a refusal is a fixture failure. */
async function pinSession(request: APIRequestContext, id: string): Promise<void> {
  const response = await request.put(`/api/sessions/${encodeURIComponent(id)}/pin`, { data: { pinned: true } });
  expect(response.ok(), await response.text()).toBe(true);
}

/** A second client's relative move changes saved order without releasing the first page's pointer hold. */
async function movePin(request: APIRequestContext, id: string, neighbor: string, placement: "before" | "after"): Promise<void> {
  const response = await request.put(`/api/sessions/${encodeURIComponent(id)}/pin-order`, { data: { neighbor, placement } });
  expect(response.ok(), await response.text()).toBe(true);
}

/** Pin/unpin must survive a second client, and pinned rows move to the top once the pointer is off the list.
 *
 * This does not exercise the pointer hold itself: closing the last row's menu
 * leaves the pointer below the list, so the hold is already released by then.
 * sort.spec.ts's hold tests cover keeping rows still under the pointer.
 * Each owned release is independent, so an observer or preference failure
 * cannot leave this test's sessions running for later tests. */
test("pin from the menu groups the sidebar and unpin restores title order", async ({ page, request, context }, testInfo) => {
  const stamp = Date.now();
  const created: string[] = [];
  const other = await context.newPage();
  // The sort choice below is a helm-wide preference every later spec shares.
  await resetPreferences(request);
  let bodyFailed = false;
  try {
    const first = await createSession(request, { title: `a-pin-${stamp}` });
    created.push(first.id);
    const last = await createSession(request, { title: `z-pin-${stamp}` });
    created.push(last.id);
    await page.goto("/");
    await chooseSort(page, "title");
    const a = page.locator(`.session-row[data-session-id="${first.id}"]`);
    const z = page.locator(`.session-row[data-session-id="${last.id}"]`);
    await expect(a).toBeVisible({ timeout: 20_000 });
    await expect(z).toBeVisible();
    await expect(a).toHaveAttribute("data-session-pinned", "false");
    await page.mouse.move(0, 0);
    const order = () => page.locator(".session-row").evaluateAll((rows) => rows.map((row) => row.getAttribute("data-session-id")));
    await expect.poll(async () => (await order()).indexOf(first.id) < (await order()).indexOf(last.id)).toBe(true);
    await other.goto("/");
    const shared = other.locator(`.session-row[data-session-id="${last.id}"]`);
    await expect(shared).toHaveAttribute("data-session-pinned", "false", { timeout: 20_000 });
    await openRowMenu(z);
    await z.getByRole("menuitem", { name: "Pin to top", exact: true }).click();
    await expect(z.getByRole("img", { name: "Pinned", exact: true })).toBeVisible();
    // A pin stays secondary metadata on either ordinary or selected rows.
    const pinRole = await z.getAttribute("data-session-selected") === "true" ? "--on-accent-dim" : "--fg-1";
    await expect(z.locator(".session-pin")).toHaveCSS("color", await resolveToken(page, pinRole));
    await expect(z.locator(".session-row-open")).toHaveAccessibleName(/Pinned/);
    await page.mouse.move(0, 0);
    await expect.poll(async () => (await order()).indexOf(last.id) < (await order()).indexOf(first.id)).toBe(true);
    await expect(page.locator(".session-pin-separator")).toHaveCount(1);
    await expect(shared).toHaveAttribute("data-session-pinned", "true", { timeout: 20_000 });
    await testInfo.attach("pinned-sidebar", { body: await page.locator(".app-sidebar").screenshot(), contentType: "image/png" });
    const reply = await listSessions(request, "sort=title");
    expect(reply.sessions.findIndex((row) => row.id === first.id)).toBeLessThan(reply.sessions.findIndex((row) => row.id === last.id));
    await openRowMenu(z);
    await z.getByRole("menuitem", { name: "Unpin", exact: true }).click();
    await expect(z.locator(".session-pin")).toHaveCount(0);
    await page.mouse.move(0, 0);
    await expect.poll(async () => (await order()).indexOf(first.id) < (await order()).indexOf(last.id)).toBe(true);
    await expect(page.locator(".session-pin-separator")).toHaveCount(0);
    await expect(shared).toHaveAttribute("data-session-pinned", "false", { timeout: 20_000 });
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["preferences", () => resetPreferences(request)],
      ["observer page", () => other.close()],
      ...created.reverse().map((id): [string, () => Promise<void>] => [
        `session ${id}`,
        () => cleanupSession(request, id),
      ]),
    ]);
  }
});

/** Manual order must win over title/activity/creation sorting, survive reload, and reach a live second client.
 * Keyboard activation also pins the visible-neighbor request and focus handoff while a pointer hold is active. */
test("menu moves share manual pin order, retain focus and ignore sort choices", async ({ page, request, context }) => {
  const created: string[] = [];
  const other = await context.newPage();
  let bodyFailed = false;
  await resetPreferences(request);
  try {
    const stamp = Date.now();
    const a = await createSession(request, { title: `a-manual-${stamp}` });
    created.push(a.id);
    const b = await createSession(request, { title: `z-manual-${stamp}` });
    created.push(b.id);
    const c = await createSession(request, { title: `m-manual-${stamp}` });
    created.push(c.id);
    const unpinned = await createSession(request, { title: `n-manual-${stamp}` });
    created.push(unpinned.id);
    for (const id of [b.id, a.id, c.id]) await pinSession(request, id);
    await patchPreferences(request, { list_sort: "title" });
    await page.goto("/");
    await other.goto("/");
    await page.bringToFront();
    const row = (id: string) => page.locator(`.session-row[data-session-id="${id}"]`);
    await expect(page.locator('.session-row[data-session-pinned="true"]')).toHaveCount(3, { timeout: 20_000 });
    await expect.poll(() => ownedOrder(page, created)).toEqual([b.id, a.id, c.id, unpinned.id]);
    await expect.poll(() => ownedOrder(other, created)).toEqual([b.id, a.id, c.id, unpinned.id]);

    await openRowMenu(row(c.id));
    await expect(row(c.id).getByRole("menuitem", { name: "Move down", exact: true })).toHaveCount(0);
    const up = row(c.id).getByRole("menuitem", { name: "Move up", exact: true });
    await up.focus();
    await expect(up).toBeFocused();
    // Put the pointer back on the list while leaving keyboard focus on the item.
    // The accepted order must change well before five seconds of stillness could release the hold.
    await row(c.id).locator(".session-row-menu").hover();
    const requestUp = page.waitForRequest((r) => r.method() === "PUT" && r.url().endsWith(`/${encodeURIComponent(c.id)}/pin-order`));
    await page.keyboard.press("Enter");
    expect((await requestUp).postDataJSON()).toEqual({ neighbor: a.id, placement: "before" });
    await expect(row(c.id).locator(".session-row-menu-flyout")).toHaveCount(0);
    await expect.poll(() => ownedOrder(page, created), { timeout: 2_000 }).toEqual([b.id, c.id, a.id, unpinned.id]);
    await expect(row(c.id).locator(".session-row-menu")).toBeFocused();
    await expect.poll(() => ownedOrder(other, created)).toEqual([b.id, c.id, a.id, unpinned.id]);

    await openRowMenu(row(b.id));
    await expect(row(b.id).getByRole("menuitem", { name: "Move up", exact: true })).toHaveCount(0);
    const down = row(b.id).getByRole("menuitem", { name: "Move down", exact: true });
    await down.focus();
    await expect(down).toBeFocused();
    const requestDown = page.waitForRequest((r) => r.method() === "PUT" && r.url().endsWith(`/${encodeURIComponent(b.id)}/pin-order`));
    await page.keyboard.press("Enter");
    expect((await requestDown).postDataJSON()).toEqual({ neighbor: c.id, placement: "after" });
    await expect.poll(() => ownedOrder(page, created)).toEqual([c.id, b.id, a.id, unpinned.id]);
    await expect(row(b.id).locator(".session-row-menu")).toBeFocused();
    await expect.poll(() => ownedOrder(other, created)).toEqual([c.id, b.id, a.id, unpinned.id]);

    await openRowMenu(row(unpinned.id));
    await expect(row(unpinned.id).getByRole("menuitem", { name: /^Move (up|down)$/ })).toHaveCount(0);
    await page.keyboard.press("Escape");
    await page.mouse.move(0, 0);
    await page.reload();
    await expect.poll(() => ownedOrder(page, created)).toEqual([c.id, b.id, a.id, unpinned.id]);
    for (const sort of ["created", "activity", "title"] as const) {
      await chooseSort(page, sort);
      await page.mouse.move(0, 0);
      await expect.poll(() => ownedOrder(page, created)).toEqual([c.id, b.id, a.id, unpinned.id]);
    }
    // Explicit blur has no replacement focusin. After the next render boundary,
    // another client's move must not revive the toggle the user already left.
    const toggle = row(c.id).locator(".session-row-menu");
    await toggle.focus();
    await expect(toggle).toBeFocused();
    await toggle.evaluate((element: HTMLElement) => element.blur());
    await expect(page.locator("body")).toBeFocused();
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())));
    await movePin(request, c.id, a.id, "after");
    await expect.poll(() => ownedOrder(page, created)).toEqual([b.id, a.id, c.id, unpinned.id]);
    await expect(page.locator("body")).toBeFocused();
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["preferences", () => resetPreferences(request)],
      ["observer page", () => other.close()],
      ...created.reverse().map((id): [string, () => Promise<void>] => [`session ${id}`, () => cleanupSession(request, id)]),
    ]);
  }
});

/** Reconciliation must retain the hold's original pin membership across more than one reply.
 * A newly pinned row below a gap stays there with no Move items, even after another client moves it to the saved top.
 * Keeping that row's menu open makes the hold's lifetime explicit rather than depending on fast setup or sleeps. */
test("new pins remain held across replies until the original hold ends", async ({ page, request, context }) => {
  const created: string[] = [];
  const other = await context.newPage();
  let bodyFailed = false;
  await resetPreferences(request);
  try {
    const stamp = Date.now();
    for (const title of ["a", "b", "c", "d"]) {
      created.push((await createSession(request, { title: `${title}-held-pin-${stamp}` })).id);
    }
    const [a, b, c, d] = created;
    for (const id of [b, a]) await pinSession(request, id);
    await patchPreferences(request, { list_sort: "title" });
    const feed = await stubFeed(page);
    feed.notifyOnConnect(1);
    await page.goto("/");
    await feed.waitForConnection(1);
    await other.goto("/");
    await page.bringToFront();
    const row = (id: string) => page.locator(`.session-row[data-session-id="${id}"]`);
    const heldOrder = [b, a, c, d];
    await expect.poll(() => ownedOrder(page, created)).toEqual(heldOrder);
    await row(a).hover();
    await pinSession(request, d);
    feed.notify(2);
    await expect(row(d)).toHaveAttribute("data-session-pinned", "true");
    await expect(row(d).locator(".session-pin-grip")).toHaveCount(0);
    expect(await ownedOrder(page, created)).toEqual(heldOrder);
    await expect(row(a).locator("xpath=following-sibling::*[1]")).toHaveClass(/session-pin-separator/);
    await openRowMenu(row(d));
    await expect(row(d).getByRole("menuitem", { name: /^Move (up|down)$/ })).toHaveCount(0);

    // The renamed title proves another reply was accepted and reconciled while the same hold remained active.
    await renameSession(request, c, `c-held-pin-second-${stamp}`);
    feed.notify(3);
    await expect(row(c)).toContainText(`c-held-pin-second-${stamp}`);
    await expect(row(d).locator(".session-row-menu")).toHaveAttribute("aria-expanded", "true");
    await movePin(request, d, b, "before");
    await renameSession(request, c, `c-held-pin-third-${stamp}`);
    feed.notify(4);
    await expect(row(c)).toContainText(`c-held-pin-third-${stamp}`);
    expect(await ownedOrder(page, created), "later replies must not recapture the new pin as an original pin").toEqual(heldOrder);
    await expect(row(d).locator(".session-pin-grip")).toHaveCount(0);
    await expect(row(d).locator(".session-row-menu")).toHaveAttribute("aria-expanded", "true");
    await expect.poll(() => ownedOrder(other, created)).toEqual([d, b, a, c]);

    await row(d).getByRole("menuitem", { name: "Unpin", exact: true }).focus();
    await page.keyboard.press("Escape");
    await page.mouse.move(0, 0);
    await expect.poll(() => ownedOrder(page, created), { timeout: 2_000 }).toEqual([d, b, a, c]);
    await expect(row(d).locator(".session-pin-grip")).toHaveCount(1);
    await openRowMenu(row(d));
    await expect(row(d).getByRole("menuitem", { name: "Move down", exact: true })).toBeVisible();
    await expect(row(d).getByRole("menuitem", { name: "Move up", exact: true })).toHaveCount(0);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["preferences", () => resetPreferences(request)],
      ["observer page", () => other.close()],
      ...created.reverse().map((id): [string, () => Promise<void>] => [`session ${id}`, () => cleanupSession(request, id)]),
    ]);
  }
});

/** A real captured pointer sends one visible-neighbor move, shared and saved by the helm.
 * Escape, an outside release and source removal must retire paint without changing pin membership or order.
 * The asset unit tests separately distinguish no-send cleanup from the Rust bridge rejecting a stale intent. */
test("drag pins within their group and cancel interrupted moves", async ({ page, request, context }, testInfo) => {
  const created: string[] = [];
  const other = await context.newPage();
  const moves: { id: string; body: unknown }[] = [];
  let bodyFailed = false;
  await resetPreferences(request);
  try {
    const stamp = Date.now();
    for (const title of ["z", "a", "m", "n"]) {
      created.push((await createSession(request, { title: `${title}-drag-pin-${stamp}` })).id);
    }
    const [a, b, c, unpinned] = created;
    for (const id of [a, b, c]) await pinSession(request, id);
    await patchPreferences(request, { list_sort: "title" });
    page.on("request", (r) => {
      if (r.method() !== "PUT") return;
      const match = new URL(r.url()).pathname.match(/^\/api\/sessions\/([^/]+)\/pin-order$/);
      if (match) moves.push({ id: decodeURIComponent(match[1]), body: r.postDataJSON() });
    });
    await page.goto("/");
    await other.goto("/");
    await page.bringToFront();
    const row = (id: string) => page.locator(`.session-row[data-session-id="${id}"]`);
    await expect.poll(() => ownedOrder(page, created)).toEqual([a, b, c, unpinned]);
    await expect.poll(() => ownedOrder(other, created)).toEqual([a, b, c, unpinned]);
    await expect(row(unpinned).locator(".session-pin-grip")).toHaveCount(0);
    const grip = row(c).locator(".session-pin-grip");
    await row(c).hover();
    await expect(grip).toHaveCSS("opacity", "1");
    await expect(grip).toBeEnabled();
    const source = await grip.boundingBox();
    const target = await row(a).boundingBox();
    expect(source).not.toBeNull();
    expect(target).not.toBeNull();
    await page.mouse.move(source!.x + source!.width / 2, source!.y + source!.height / 2);
    await page.mouse.down();
    await page.mouse.move(target!.x + target!.width / 2, target!.y + 3);
    await expect(row(c)).toHaveAttribute("data-pin-dragging", "true");
    await expect(row(a)).toHaveAttribute("data-pin-drop", "before");
    expect(await ownedOrder(page, created), "paint must not optimistically reorder the rows").toEqual([a, b, c, unpinned]);
    await testInfo.attach("dragging-pin", { body: await page.locator(".app-sidebar").screenshot(), contentType: "image/png" });
    await page.mouse.up();
    await expect.poll(() => moves).toEqual([{ id: c, body: { neighbor: a, placement: "before" } }]);
    await expect.poll(() => ownedOrder(page, created), { timeout: 2_000 }).toEqual([c, a, b, unpinned]);
    await expect.poll(() => ownedOrder(other, created)).toEqual([c, a, b, unpinned]);
    await expect(page.locator("[data-pin-dragging], [data-pin-drop]")).toHaveCount(0);
    await page.mouse.move(0, 0);
    await page.reload();
    await expect.poll(() => ownedOrder(page, created)).toEqual([c, a, b, unpinned]);

    // Every cancellation begins with a real valid target, so an inert bridge
    // or an unarmed pointer cannot satisfy the cleanup and unchanged-order checks.
    for (const cancellation of ["escape", "outside", "separator"] as const) {
      const from = row(b).locator(".session-pin-grip");
      await row(b).hover();
      const start = await from.boundingBox();
      const before = await row(c).boundingBox();
      expect(start).not.toBeNull();
      expect(before).not.toBeNull();
      await page.mouse.move(start!.x + start!.width / 2, start!.y + start!.height / 2);
      await page.mouse.down();
      await page.mouse.move(before!.x + before!.width / 2, before!.y + 3);
      await expect(row(c)).toHaveAttribute("data-pin-drop", "before");
      if (cancellation === "escape") await page.keyboard.press("Escape");
      else if (cancellation === "outside") {
        const list = await page.locator(".session-list").boundingBox();
        expect(list).not.toBeNull();
        await page.mouse.move(list!.x + list!.width + 20, before!.y + 3);
      } else {
        const below = await row(unpinned).boundingBox();
        expect(below).not.toBeNull();
        await page.mouse.move(below!.x + below!.width / 2, below!.y + below!.height / 2);
      }
      await page.mouse.up();
      await expect(page.locator("[data-pin-dragging], [data-pin-drop]")).toHaveCount(0);
      await page.mouse.move(0, 0);
      await page.reload();
      await expect.poll(() => ownedOrder(page, created)).toEqual([c, a, b, unpinned]);
      expect(moves).toHaveLength(1);
    }

    const removedGrip = await row(b).locator(".session-pin-grip").boundingBox();
    const before = await row(c).boundingBox();
    expect(removedGrip).not.toBeNull();
    expect(before).not.toBeNull();
    await page.mouse.move(removedGrip!.x + removedGrip!.width / 2, removedGrip!.y + removedGrip!.height / 2);
    await page.mouse.down();
    await page.mouse.move(before!.x + before!.width / 2, before!.y + 3);
    await expect(row(c)).toHaveAttribute("data-pin-drop", "before");
    await cleanupSession(request, b);
    await expect(row(b)).toHaveCount(0);
    await expect(page.locator("[data-pin-dragging], [data-pin-drop]")).toHaveCount(0);
    await page.mouse.up();
    await page.mouse.move(0, 0);
    await page.reload();
    await expect.poll(() => ownedOrder(page, created)).toEqual([c, a, unpinned]);
    expect(moves).toHaveLength(1);
    const saved = await listSessions(request);
    for (const id of [c, a]) expect(saved.sessions.find((session) => session.id === id)?.pinned).toBe(true);
    expect(saved.sessions.find((session) => session.id === unpinned)?.pinned).toBe(false);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["captured pointer", () => page.mouse.up()],
      ["preferences", () => resetPreferences(request)],
      ["observer page", () => other.close()],
      ...created.reverse().map((id): [string, () => Promise<void>] => [`session ${id}`, () => cleanupSession(request, id)]),
    ]);
  }
});

/** A listing error replaces the list DOM without remounting its surrounding view.
 * Recovery must create a working drag bridge; predecessor cleanup cannot retire the successor's listeners. */
test("pin dragging survives a failed listing and replacement list mount", async ({ page, request }) => {
  const created: string[] = [];
  let failListing = false;
  let bodyFailed = false;
  await resetPreferences(request);
  try {
    const stamp = Date.now();
    for (const title of ["a", "b"]) {
      const session = await createSession(request, { title: `${title}-remount-pin-${stamp}` });
      created.push(session.id);
      await pinSession(request, session.id);
    }
    const [a, b] = created;
    const feed = await stubFeed(page);
    feed.notifyOnConnect(1);
    await page.route((url) => url.pathname === "/api/sessions", async (route) => {
      if (!failListing || route.request().method() !== "GET") return route.continue();
      const response = await route.fetch();
      const headers = { ...response.headers(), "content-type": "text/plain" };
      delete headers["content-length"];
      await route.fulfill({ status: 503, headers, body: "controlled listing failure" });
    });
    await page.goto("/");
    await feed.waitForConnection(1);
    await expect.poll(() => ownedOrder(page, created)).toEqual([a, b]);
    const predecessor = await page.locator(".session-list").elementHandle();
    expect(predecessor).not.toBeNull();
    failListing = true;
    feed.notify(2);
    await expect(page.getByText(/Failed to load sessions:.*controlled listing failure/)).toBeVisible();
    await expect(page.locator(".session-list")).toHaveCount(0);
    expect(await predecessor!.evaluate((node) => node.isConnected)).toBe(false);
    failListing = false;
    feed.notify(3);
    await expect.poll(() => ownedOrder(page, created)).toEqual([a, b]);
    const row = (id: string) => page.locator(`.session-row[data-session-id="${id}"]`);
    await row(b).hover();
    const grip = await row(b).locator(".session-pin-grip").boundingBox();
    const target = await row(a).boundingBox();
    expect(grip).not.toBeNull();
    expect(target).not.toBeNull();
    await page.mouse.move(grip!.x + grip!.width / 2, grip!.y + grip!.height / 2);
    await page.mouse.down();
    await page.mouse.move(target!.x + target!.width / 2, target!.y + 3);
    await expect(row(a)).toHaveAttribute("data-pin-drop", "before");
    const move = page.waitForRequest((r) => r.method() === "PUT" && r.url().endsWith(`/${encodeURIComponent(b)}/pin-order`));
    await page.mouse.up();
    expect((await move).postDataJSON()).toEqual({ neighbor: a, placement: "before" });
    await expect.poll(() => ownedOrder(page, created)).toEqual([b, a]);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    failListing = false;
    await cleanupOwnedSteps(bodyFailed, [
      ["captured pointer", () => page.mouse.up()],
      ["listing route", () => page.unrouteAll({ behavior: "wait" })],
      ["preferences", () => resetPreferences(request)],
      ...created.reverse().map((id): [string, () => Promise<void>] => [`session ${id}`, () => cleanupSession(request, id)]),
    ]);
  }
});
