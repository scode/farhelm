import { expectCompactRows } from "./helpers/list-header";
import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { expect, test } from "./helpers/evidence";
import {
  cleanupSession,
  createSession,
  observeFeedReaders,
  patchPreferences,
  pinAutoSelect,
  resetPreferences,
  stubFeed,
  waitForFeedReadersSettled,
} from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { waitForSessionReady } from "./helpers/terminal-readiness";

type PopupKind = "menu" | "bell";
type ListedRow = { id: string; title: string; last_activity_at: number; [field: string]: unknown };
type ListedHost = { id: number; kind: string; incarnation: number; state: { phase: string } };
type ArchiveListing = {
  issues: { id: string; path: string; message: string }[];
  checkouts: {
    id: string; name: string; repository: string; path: string; archived_at: number | null; bytes: number | null;
  }[];
};
const created: string[] = [];
const releases: (() => void)[] = [];

test.afterEach(async ({ page, request }, testInfo) => {
  await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
    ["held replies", () => { releases.splice(0).forEach((release) => release()); }],
    ["routes", () => page.unrouteAll({ behavior: "ignoreErrors" })],
    ...created.splice(0).reverse().map((id): [string, () => Promise<void>] => [
      `session ${id}`, () => cleanupSession(request, id),
    ]),
    ["preferences", () => resetPreferences(request)],
  ]);
});

/** Record real allocations before any later fixture step can fail. */
async function ownedSession(request: APIRequestContext, title: string) {
  const session = await createSession(request, { title, invocation: "sleep 300" });
  created.push(session.id);
  return session.id;
}

/** Session references remain opaque; only the real row is addressed. */
function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

/** Retire healthy feed work before checking a committed visible outcome.
 * Two ordinary render frames follow retirement; each caller also requires
 * the actual age, count, order or focus state relevant to its assertion.
 */
async function settle(page: Page) {
  await waitForFeedReadersSettled(page);
  await page.evaluate(() => new Promise<void>((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
  ));
}

/** Valid accepted archive data tests UI count/layout, not filesystem archival.
 * Entries never become deletion targets. Their distinct IDs and complete
 * metadata cross the same typed decoder as real per-host trash replies.
 */
function archiveListing(host: number, count: number): ArchiveListing {
  return {
    issues: [],
    checkouts: Array.from({ length: count }, (_, index) => ({
      id: `width-archive-${host}-${index}`,
      name: `width-archive-${index}`,
      repository: "example/repository",
      path: `/work/checkouts/.farhelm-trash/width-archive-${index}`,
      archived_at: 1_700_000_000 + index,
      bytes: null,
    })),
  };
}

/** Keep all rows real and the served listing/host registry stable.
 * A is the first row; B has a stable hour token and its own real bell.
 * A separate selected terminal prevents either tested anchor from being
 * replaced by selection. Only explicit fixture refreshes change activity.
 */
async function prepare(page: Page, request: APIRequestContext, clock = false) {
  const stamp = Date.now();
  const target = await ownedSession(request, `aaa-width-${stamp}`);
  const stable = await ownedSession(request, `bbb-width-${stamp}`);
  const selected = await ownedSession(request, `ccc-width-${stamp}`);
  // Recently active no longer reaches the supported-width wrap threshold.
  // Newest created still has to pass the actual fit premise below. The frozen
  // listing owns row order, independently of the requested sort label.
  await patchPreferences(request, { welcome_seen: true, compact: true, list_sort: "created" });
  await pinAutoSelect(page, selected);
  const response = await request.get("/api/sessions");
  expect(response.ok()).toBe(true);
  const build = requireHelmBuild(response, "popover width fixture");
  const listing = await response.json();
  const now = Math.floor(Date.now() / 1000);
  const rows: ListedRow[] = [target, stable, selected].map((id) => {
    const original = listing.sessions.find((session: { id: string }) => session.id === id);
    expect(original, "every owned session must be listed before its row is frozen").toBeTruthy();
    return {
      ...original, status: { state: "idle" }, stale: false, pinned: false, created_at: now - 10_800,
      last_activity_at: id === stable ? now - 3700 : now - 30,
      seen_activity_at: 0,
      // Keep the open list's own age metadata at a stable hour token while
      // the row's separately controlled minute token crosses its boundary.
      notifications: id === selected ? [] : [{ seq: 1, at: now - 7200, text: "width movement fixture" }],
      notifications_read_through: 1,
    };
  });
  expect(new Set(rows.map((session) => session.id)).size).toBe(3);
  const hostsResponse = await request.get("/api/hosts");
  expect(hostsResponse.ok()).toBe(true);
  expect(requireHelmBuild(hostsResponse, "width host fixture")).toBe(build);
  const registry = await hostsResponse.json();
  const hosts = registry.hosts as ListedHost[];
  const local = hosts.find((host) => host.kind === "local");
  expect(local, "trash reads use the real reserved local host").toBeTruthy();
  expect(local!.incarnation).toBeGreaterThan(0);
  expect(local!.state.phase).toBe("connected");
  await page.route((url) => url.pathname === "/api/hosts", async (route) => {
    expect(route.request().method()).toBe("GET");
    await route.fulfill({ headers: { "x-farhelm-build": build }, json: registry });
  });
  let reads = 0;
  await page.route((url) => url.pathname === "/api/sessions", async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    reads++;
    await route.fulfill({
      headers: { "x-farhelm-build": build },
      json: { ...listing, sessions: rows, total: rows.length, matching: rows.length, truncated: false },
    });
  });
  let trashCount = 0;
  let trashGate = Promise.resolve();
  let heldTrashStarted = false;
  await page.route((url) => url.pathname === "/api/checkout-trash/list", async (route) => {
    expect(route.request().method()).toBe("POST");
    const body = route.request().postDataJSON() as { host: number; expected_incarnation: number; measure_sizes: boolean };
    const host = hosts.find((host) => host.id === body.host);
    expect(host, "only real retained registry identities may supply count replies").toBeTruthy();
    expect(body.expected_incarnation).toBe(host!.incarnation);
    expect(host!.state.phase).toBe("connected");
    expect(typeof body.measure_sizes).toBe("boolean");
    const reply = archiveListing(body.host, body.host === local!.id ? trashCount : 0);
    if (body.host === local!.id && !body.measure_sizes) {
      heldTrashStarted = true;
      await trashGate;
    }
    await route.fulfill({ headers: { "x-farhelm-build": build }, json: reply });
  });
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  await observeFeedReaders(page);
  // Install before navigation so the actual ActivityClock's existing timer
  // belongs to Clock. Keep it ticking through normal mount/focus/measurement.
  if (clock) await page.clock.install();
  await page.setViewportSize({ width: 1280, height: 1400 });
  await page.goto("/");
  await waitForSessionReady(page, selected);
  await feed.waitForConnection(1);
  await settle(page);
  await trashReady(page, 0);
  await expectCompactRows(page, true);
  await expect(page.locator(".session-heading .session-count")).toHaveText("3");
  await expect(page.locator(".session-row")).toHaveCount(3);
  expect(await page.locator(".session-row").evaluateAll((nodes) => nodes.map((node) => node.getAttribute("data-session-id"))))
    .toEqual([target, stable, selected]);
  expect(await page.locator(".sidebar-content").evaluate((node) => node.scrollHeight <= node.clientHeight),
    "the entire compact fixture must fit without scrolling").toBe(true);
  let revision = 1;
  return {
    target, stable, selected, rows, build,
    reads: () => reads,
    async refresh() {
      const reply = page.waitForResponse((response) =>
        new URL(response.url()).pathname === "/api/sessions" && response.request().method() === "GET"
      );
      feed.notify(++revision);
      const response = await reply;
      expect(response.ok()).toBe(true);
      expect(response.headers()["x-farhelm-build"]).toBe(build);
      expect(await response.finished()).toBeNull();
      await settle(page);
    },
    /** Opening/closing the real dialog requests the existing count reader.
     * Its accepted measurement first retains the old count. Only the final
     * closed-dialog read carries the new count, held before any target opens.
     */
    async holdTrash(nextCount: number) {
      await trashReady(page, trashCount);
      const measured = page.waitForResponse((response) =>
        new URL(response.url()).pathname === "/api/checkout-trash/list"
        && response.request().postDataJSON().host === local!.id
        && response.request().postDataJSON().measure_sizes === true
      );
      await page.locator(".checkout-trash-button").click();
      const dialog = page.locator(".checkout-trash-dialog");
      await expect(dialog).toBeVisible();
      const measuredResponse = await measured;
      expect(measuredResponse.ok()).toBe(true);
      expect(measuredResponse.headers()["x-farhelm-build"]).toBe(build);
      expect(await measuredResponse.finished()).toBeNull();
      await settle(page);
      await trashReady(page, trashCount);
      let release = () => {};
      trashGate = new Promise<void>((resolve) => { release = resolve; });
      releases.push(release);
      heldTrashStarted = false;
      const oldCount = trashCount;
      trashCount = nextCount;
      const completed = page.waitForResponse((response) =>
        new URL(response.url()).pathname === "/api/checkout-trash/list"
        && response.request().postDataJSON().host === local!.id
        && response.request().postDataJSON().measure_sizes === false
      );
      const close = dialog.locator(".trash-close");
      await close.focus();
      await expect(close).toBeFocused();
      await close.press("Enter");
      await expect(dialog).toHaveCount(0);
      await expect.poll(() => heldTrashStarted, { message: "the real closed-dialog count read must reach its held response" }).toBe(true);
      await expect(page.locator(".checkout-trash-button")).toHaveAttribute("data-count", String(oldCount));
      await settle(page);
      return async () => {
        release();
        const response = await completed;
        expect(response.ok()).toBe(true);
        expect(response.headers()["x-farhelm-build"]).toBe(build);
        expect(await response.finished()).toBeNull();
        await settle(page);
        await trashReady(page, nextCount);
      };
    },
  };
}

type WidthFixture = Awaited<ReturnType<typeof prepare>>;

/** Require accepted counts from every reachable host, not a retained loading value. */
async function trashReady(page: Page, count: number) {
  const button = page.locator(".checkout-trash-button");
  await expect(button).toHaveAttribute("data-count", String(count));
  await expect(button).not.toHaveAttribute("data-tooltip", /Click to see what is known/);
  if (count === 0) await expect(button.locator(".checkout-trash-count")).toHaveCount(0);
  else await expect(button.locator(".checkout-trash-count")).toHaveText(String(count));
}

/** Keyboard opening leaves the pointer outside and never establishes order hold. */
async function openPopup(page: Page, target: Locator, kind: PopupKind) {
  const size = page.viewportSize()!;
  await page.mouse.move(size.width - 8, size.height - 8);
  expect(await page.locator(".session-list").evaluate((node) => node.matches(":hover"))).toBe(false);
  const toggle = target.locator(kind === "menu" ? ".session-row-menu" : ".session-row-bell");
  const panel = target.locator(kind === "menu" ? ".session-row-menu-panel" : ".session-bell-panel");
  await toggle.focus();
  await expect(toggle).toBeFocused();
  await toggle.press(kind === "menu" ? "ArrowDown" : "Enter");
  const focus = kind === "menu" ? target.locator(".session-row-rename") : panel;
  await expect(focus).toBeFocused();
  const flyout = target.locator(kind === "menu" ? ".session-row-menu-flyout" : ".session-bell-flyout");
  await expect(flyout).toHaveCSS("opacity", "1");
  await expect(flyout).toHaveAttribute("style", /left: var\(--menu-left\)/);
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  if (kind === "bell") await expect(panel.locator(".session-bell-age")).toHaveText("2h ago");
  return { toggle, panel, focus };
}

/** Observe the bell coordinate separately from unchanged row/menu/layout inputs. */
async function geometry(target: Locator) {
  return target.evaluate((node) => {
    const rect = (element: Element) => {
      const box = element.getBoundingClientRect();
      return { x: box.x, y: box.y, width: box.width, height: box.height };
    };
    const bell = rect(node.querySelector(".session-row-bell")!);
    const sidebar = document.querySelector(".sidebar-content")!;
    const shell = document.querySelector(".app-shell")!;
    const hosts = document.querySelector(".hosts-panel")!;
    const heading = document.querySelector(".session-heading")!;
    return {
      bellX: bell.x,
      bellY: bell.y,
      bellWidth: bell.width,
      bellHeight: bell.height,
      row: rect(node),
      actions: rect(node.querySelector(".session-row-menu")!),
      index: [...document.querySelectorAll(".session-row")].indexOf(node),
      host: rect(hosts),
      hostIds: [...hosts.querySelectorAll(".host-row")].map((host) => host.getAttribute("data-host-id")),
      hostDetails: hosts.querySelectorAll(".host-detail").length,
      heading: rect(heading),
      viewport: [innerWidth, innerHeight],
      scroll: [sidebar.scrollTop, sidebar.scrollLeft, shell.scrollTop, shell.scrollLeft],
    };
  });
}

/** Stage the age before opening, then tick its real 30-second publisher.
 * No clock is paused during opening: the ordinary focus/measurement oracles
 * remain active. The visible token and actual viewer elapsed value establish
 * enough margin for the later observed boundary; a slow setup fails that
 * premise rather than silently testing a different age transition.
 */
async function prepareAge(page: Page, fixture: WidthFixture, minutes: number) {
  const now = await page.evaluate(() => Math.floor(Date.now() / 1000));
  const source = fixture.rows.find((session) => session.id === fixture.target)!;
  source.last_activity_at = now - (minutes * 60 + 5);
  await fixture.refresh();
  await page.clock.fastForward(30_000);
  await settle(page);
  await expect(row(page, fixture.target).locator(".status-time")).toHaveText(`${minutes}m`);
  await expect(row(page, fixture.target).locator(".session-bell-time-spacer")).toHaveText(`${minutes}m`);
}

/** Advance only the viewer's installed timers; listing bytes stay frozen.
 * A due ActivityClock timer is the only publisher that can change the age.
 * Read retirement plus the final visible token distinguishes a real render
 * from merely moving Date or counting timer callbacks.
 */
async function advanceAge(page: Page, fixture: WidthFixture, fromMinutes: number, toMinutes: number) {
  const source = fixture.rows.find((session) => session.id === fixture.target)!;
  const elapsed = await page.evaluate((activity) => Math.floor(Date.now() / 1000) - activity, source.last_activity_at);
  expect(elapsed, "the measured-open fixture must still be in its prepared age bucket").toBeGreaterThanOrEqual(fromMinutes * 60 + 30);
  expect(elapsed, "a late fixture must not silently test the next boundary").toBeLessThan((fromMinutes + 1) * 60);
  const reads = fixture.reads();
  await page.clock.fastForward((toMinutes - fromMinutes) * 60_000);
  await settle(page);
  await expect(row(page, fixture.target).locator(".status-time")).toHaveText(`${toMinutes}m`);
  await expect(row(page, fixture.target).locator(".session-bell-time-spacer")).toHaveText(`${toMinutes}m`);
  expect(fixture.reads(), "the actual clock publisher must change the token without another listing").toBe(reads);
}

/** The spacer grows leftward while its row and actions anchor stay fixed. */
test("the actual activity clock moves a 9m bell at 10m and closes it", async ({ page, request }) => {
  const fixture = await prepare(page, request, true);
  await prepareAge(page, fixture, 9);
  const target = row(page, fixture.target);
  const { toggle, panel, focus } = await openPopup(page, target, "bell");
  const { bellX: beforeX, ...before } = await geometry(target);
  await expect(focus).toBeFocused();
  await advanceAge(page, fixture, 9, 10);
  const { bellX: afterX, ...after } = await geometry(target);
  expect(afterX).toBeLessThan(beforeX);
  expect(after, "only the bell's horizontal anchor must move").toEqual(before);
  await expect(panel).toHaveCount(0);
  await expect(toggle).toHaveAttribute("aria-expanded", "false");
});

/** Equal-width age tokens update real content without invalidating placement. */
test("the actual activity clock keeps an open bell across 10m to 11m", async ({ page, request }) => {
  const fixture = await prepare(page, request, true);
  await prepareAge(page, fixture, 10);
  const target = row(page, fixture.target);
  const { toggle, panel, focus } = await openPopup(page, target, "bell");
  const before = await geometry(target);
  await expect(focus).toBeFocused();
  await advanceAge(page, fixture, 10, 11);
  expect(await geometry(target)).toEqual(before);
  await expect(panel).toBeVisible();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  await expect(focus).toBeFocused();
});

/** A's real width change must not capture the shared registration's open B. */
test("A changing age width leaves B's stable measured bell open", async ({ page, request }) => {
  const fixture = await prepare(page, request, true);
  await prepareAge(page, fixture, 9);
  const source = row(page, fixture.target);
  const target = row(page, fixture.stable);
  await expect(target.locator(".session-bell-time-spacer")).toHaveText("1h");
  const { toggle, panel, focus } = await openPopup(page, target, "bell");
  const { bellX: beforeX, ...beforeSource } = await geometry(source);
  const beforeTarget = await geometry(target);
  await expect(focus).toBeFocused();
  await advanceAge(page, fixture, 9, 10);
  const { bellX: afterX, ...afterSource } = await geometry(source);
  expect(afterX).toBeLessThan(beforeX);
  expect(afterSource).toEqual(beforeSource);
  await expect(target.locator(".session-bell-time-spacer")).toHaveText("1h");
  expect(await geometry(target)).toEqual(beforeTarget);
  await expect(panel).toBeVisible();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  await expect(focus).toBeFocused();
});

/** The actions menu's fixed trailing anchor is independent of the bell spacer. */
test("a row's growing bell age does not close its own actions menu", async ({ page, request }) => {
  const fixture = await prepare(page, request, true);
  await prepareAge(page, fixture, 9);
  const target = row(page, fixture.target);
  const { toggle, panel, focus } = await openPopup(page, target, "menu");
  const { bellX: beforeX, ...before } = await geometry(target);
  await expect(focus).toBeFocused();
  await advanceAge(page, fixture, 9, 10);
  const { bellX: afterX, ...after } = await geometry(target);
  expect(afterX).toBeLessThan(beforeX);
  expect(after).toEqual(before);
  await expect(panel).toBeVisible();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  await expect(focus).toBeFocused();
});

/** Use the supported real separator before any target popup is measured. */
async function sidebarWidth(page: Page, desired: number) {
  expect(desired).toBeGreaterThanOrEqual(296);
  expect(desired).toBeLessThanOrEqual(656);
  const current = await page.locator(".app-sidebar").evaluate((node) => parseFloat(getComputedStyle(node).width));
  const handle = page.getByRole("separator", { name: "Sidebar width" });
  await expect(handle).toHaveAttribute("data-sidebar-bound", "true");
  const box = (await handle.boundingBox())!;
  const x = box.x + box.width / 2;
  const y = box.y + Math.min(200, box.height / 2);
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + desired - current, y, { steps: 8 });
  await page.mouse.up();
  await expect(handle).toHaveAttribute("aria-valuenow", String(desired));
  await settle(page);
}

/** Natural item widths choose an actual supported wrap threshold per engine. */
async function headerMetrics(page: Page) {
  return page.locator(".session-heading").evaluate((node) => {
    const box = node.getBoundingClientRect();
    const style = getComputedStyle(node);
    const count = node.querySelector(".session-count")!.getBoundingClientRect();
    const title = node.querySelector(".sort-title-owner")!.getBoundingClientRect();
    const actions = node.querySelector(".session-heading-actions")!.getBoundingClientRect();
    return {
      height: box.height,
      available: box.width - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight),
      total: count.width + title.width + actions.width + 2 * parseFloat(style.columnGap),
      actionsY: actions.y,
      actionsX: actions.x,
      titleBottom: title.bottom,
      titleRight: title.right,
      sidebar: parseFloat(getComputedStyle(document.querySelector(".app-sidebar")!).width),
    };
  });
}

for (const kind of ["menu", "bell"] as const) {
  /** A real accepted count span inserts width into the actual wrapping header.
   * The count read starts with the dialog already closed, before the target
   * opens; only its accepted response is released after measurement/focus.
   */
  test(`trash count appearance wraps the header and closes an open ${kind}`, async ({ page, request }) => {
    const fixture = await prepare(page, request);
    await sidebarWidth(page, 656);
    const empty = await headerMetrics(page);
    await (await fixture.holdTrash(1))();
    const one = await headerMetrics(page);
    expect(one.total).toBeGreaterThan(empty.total);
    await (await fixture.holdTrash(0))();
    const desired = Math.round(empty.sidebar + (empty.total + one.total) / 2 - empty.available);
    await sidebarWidth(page, desired);
    const fitted = await headerMetrics(page);
    expect(fitted.height).toBe(empty.height);
    expect(fitted.actionsX).toBeGreaterThanOrEqual(fitted.titleRight);
    expect(fitted.available).toBeGreaterThanOrEqual(empty.total);
    expect(fitted.available).toBeLessThan(one.total);
    const complete = await fixture.holdTrash(1);
    const target = row(page, fixture.target);
    const { toggle, panel, focus } = await openPopup(page, target, kind);
    const before = await geometry(target);
    const ids = fixture.rows.map((session) => session.id);
    const reads = fixture.reads();
    await expect(page.locator(".checkout-trash-count")).toHaveCount(0);
    await expect(focus).toBeFocused();
    await complete();
    const after = await geometry(target);
    expect(after.heading.height, "the added actual count span must wrap the header").toBeGreaterThan(before.heading.height);
    const growth = after.heading.height - before.heading.height;
    expect(after.row.y - before.row.y).toBe(growth);
    expect(after.bellY - before.bellY).toBe(growth);
    expect(after.actions.y - before.actions.y).toBe(growth);
    const wrapped = await headerMetrics(page);
    expect(wrapped.actionsY).toBeGreaterThan(wrapped.titleBottom);
    expect({ ...after, heading: { ...after.heading, height: before.heading.height },
      row: { ...after.row, y: before.row.y }, actions: { ...after.actions, y: before.actions.y }, bellY: before.bellY })
      .toEqual(before);
    expect(fixture.reads(), "trash acceptance must not rely on a session listing change").toBe(reads);
    expect(await page.locator(".session-row").evaluateAll((nodes) => nodes.map((node) => node.getAttribute("data-session-id")))).toEqual(ids);
    await expect(page.locator(".session-heading .session-count")).toHaveText("3");
    await expect(panel).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
  });

  /** A 1→2 accepted count changes text with the same decimal column count.
   * Neither the full icon nor loading/ready summary changes may dismiss a
   * measured popup when its header and anchor remain physically unchanged.
   */
  test(`same-width trash counts keep an open ${kind} at its unchanged anchor`, async ({ page, request }) => {
    const fixture = await prepare(page, request);
    await (await fixture.holdTrash(1))();
    const complete = await fixture.holdTrash(2);
    const target = row(page, fixture.target);
    const { toggle, panel, focus } = await openPopup(page, target, kind);
    const before = await geometry(target);
    const reads = fixture.reads();
    await expect(page.locator(".checkout-trash-count")).toHaveText("1");
    await expect(focus).toBeFocused();
    await complete();
    await expect(page.locator(".checkout-trash-count")).toHaveText("2");
    expect(await geometry(target)).toEqual(before);
    expect(fixture.reads()).toBe(reads);
    await expect(panel).toBeVisible();
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await expect(focus).toBeFocused();
  });
}
