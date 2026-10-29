import { expectCompactRows } from "./helpers/list-header";
import { randomUUID } from "node:crypto";
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

// Notice text is controlled at the wire boundary after a real owned Delete
// completes. These tests cover its publication/layout, not the backend's
// filesystem conditions for generating a preservation notice.
const NOTICE = "Layout fixture notice: the checkout was left in place, unmanaged.";
type PopupKind = "menu" | "bell";
type ListedRow = { id: string; title: string; [field: string]: unknown };
type DeleteOutcome = { archived?: boolean; notice?: string | null };
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

/** Register each real allocation before the next setup operation can fail. */
async function ownedSession(request: APIRequestContext, title: string) {
  const session = await createSession(request, { title, invocation: "sleep 300" });
  created.push(session.id);
  return session.id;
}

/** Keep session references opaque when addressing the rendered row. */
function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

/** Reader retirement and two render frames precede negative outcome checks.
 * The caller still supplies a visible count, title, notice or order oracle;
 * network completion alone does not prove the corresponding UI committed.
 */
async function settle(page: Page) {
  await waitForFeedReadersSettled(page);
  await page.evaluate(() => new Promise<void>((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
  ));
}

/** Freeze a coherent whole listing while retaining real action/terminal routes.
 * Target and selection are always real; count-only filler rows carry unique
 * entry-bound UUIDs and are never opened or acted on. Lower Delete sources
 * are real owned sessions, so the operation remains a genuine server Delete.
 */
async function prepare(page: Page, request: APIRequestContext, options: { lowerCount?: number; fillerCount?: number } = {}) {
  const stamp = Date.now();
  const target = await ownedSession(request, `aaa-header-${stamp}`);
  const selected = await ownedSession(request, `bbb-header-${stamp}`);
  const lower: string[] = [];
  for (let i = 0; i < (options.lowerCount ?? 0); i++) {
    lower.push(await ownedSession(request, `zzz-delete-${i}-${stamp}`));
  }
  // Recently active no longer reaches the wrap threshold at a supported
  // width. Try Newest created; the explicit fit premise below still governs.
  // Row order comes from the controlled listing, not this preference.
  await patchPreferences(request, {
    welcome_seen: true, compact: true, list_sort: "created", skip_session_confirmations: true,
  });
  await pinAutoSelect(page, selected);
  const response = await request.get("/api/sessions");
  expect(response.ok()).toBe(true);
  const build = requireHelmBuild(response, "popover header fixture");
  const source = await response.json();
  const now = Math.floor(Date.now() / 1000);
  const rows: ListedRow[] = [target, selected, ...lower].map((id) => {
    const original = source.sessions.find((session: { id: string }) => session.id === id);
    expect(original, "every owned session must be listed before its data is frozen").toBeTruthy();
    return {
      ...original, status: { state: "idle" }, stale: false, pinned: false,
      last_activity_at: now - 30, seen_activity_at: 0,
      notifications: id === target ? [{ seq: 1, at: now, text: "header movement fixture" }] : [],
      notifications_read_through: 1,
    };
  });
  const template = { ...rows[0], notifications: [], notifications_read_through: 0 };
  expect(target, "real session references must carry their host-entry prefix").toContain(":");
  const prefix = target.slice(0, target.indexOf(":"));
  expect(prefix, "controlled row IDs retain the real host-entry prefix").toBeTruthy();
  const filler = (index: number): ListedRow => ({
    ...template, id: `${prefix}:${randomUUID()}`, title: `zzz-count-${String(index).padStart(2, "0")}`,
  });
  for (let i = 0; i < (options.fillerCount ?? 0); i++) rows.push(filler(i));
  expect(new Set(rows.map((session) => session.id)).size).toBe(rows.length);
  const hostsResponse = await request.get("/api/hosts");
  expect(hostsResponse.ok(), "the stable host fixture must come from the real registry").toBe(true);
  expect(requireHelmBuild(hostsResponse, "header host fixture")).toBe(build);
  const hosts = await hostsResponse.json();
  await page.route((url) => url.pathname === "/api/hosts", async (route) => {
    expect(route.request().method()).toBe("GET");
    await route.fulfill({
      headers: { "x-farhelm-build": build, "content-type": "application/json" }, json: hosts,
    });
  });
  let gate = Promise.resolve();
  let reads = 0;
  // A latch rather than a one-shot: the list retries a failed read on its
  // own, so a single failure would end at a moment the test did not choose.
  let failing = false;
  await page.route((url) => url.pathname === "/api/sessions", async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    const body = { ...source, sessions: rows.slice(), total: rows.length, matching: rows.length, truncated: false };
    reads++;
    await gate;
    if (failing) {
      return route.fulfill({ status: 503, headers: { "x-farhelm-build": build }, body: "listing fixture failure" });
    }
    await route.fulfill({
      headers: { "x-farhelm-build": build, "content-type": "application/json" }, json: body,
    });
  });
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  await observeFeedReaders(page);
  await page.setViewportSize({ width: 1280, height: 1400 });
  await page.goto("/");
  await waitForSessionReady(page, selected);
  await feed.waitForConnection(1);
  await expect(page.locator(".host-rail-avatar").first()).toBeVisible();
  await settle(page);
  await expect(page.locator(".session-row").first()).toHaveAttribute("data-session-id", target);
  await expect(page.locator(".session-row")).toHaveCount(rows.length);
  await expect(page.locator(".session-heading .session-count")).toHaveText(String(rows.length));
  await expectCompactRows(page, true);
  await expect(page.locator(".checkout-trash-button")).toHaveAttribute("data-count", "0");
  await expect(page.locator(".delete-notice")).toHaveCount(0);
  expect(await page.locator(".sidebar-content").evaluate((node) => node.scrollHeight <= node.clientHeight),
    "all controlled compact rows must fit without scrolling").toBe(true);
  let revision = 1;
  const notify = () => feed.notify(++revision);
  return {
    target, selected, lower, rows, build, filler, notify,
    reads: () => reads,
    failListings(on: boolean) {
      failing = on;
    },
    holdListings() {
      let release = () => {};
      gate = new Promise<void>((resolve) => { release = resolve; });
      releases.push(release);
      return release;
    },
    async refresh() {
      const reply = page.waitForResponse((response) =>
        new URL(response.url()).pathname === "/api/sessions" && response.request().method() === "GET"
      );
      notify();
      const response = await reply;
      expect(response.ok()).toBe(true);
      expect(response.headers()["x-farhelm-build"]).toBe(build);
      expect(await response.finished()).toBeNull();
      await settle(page);
      await expect(page.locator(".session-row")).toHaveCount(rows.length);
      await expect(page.locator(".session-heading .session-count")).toHaveText(String(rows.length));
    },
  };
}

type HeaderFixture = Awaited<ReturnType<typeof prepare>>;

/** Open by keyboard with no outside pointer press or display-order hold. */
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
  return { toggle, panel, focus };
}

/** Keep the target's coordinate separate from other possible layout causes. */
async function geometry(target: Locator) {
  return target.evaluate((node) => {
    const hosts = document.querySelector(".hosts-panel")!;
    const heading = document.querySelector(".session-heading")!;
    const sidebar = document.querySelector(".sidebar-content")!;
    const shell = document.querySelector(".app-shell")!;
    return {
      y: node.getBoundingClientRect().y,
      index: [...document.querySelectorAll(".session-row")].indexOf(node),
      headingHeight: heading.getBoundingClientRect().height,
      headingY: heading.getBoundingClientRect().y,
      headingWidth: heading.getBoundingClientRect().width,
      hostHeight: hosts.getBoundingClientRect().height,
      hostWidth: hosts.getBoundingClientRect().width,
      hostIds: [...hosts.querySelectorAll(".host-row")].map((host) => host.getAttribute("data-host-id")),
      hostDetails: hosts.querySelectorAll(".host-detail").length,
      viewport: [innerWidth, innerHeight],
      scroll: [sidebar.scrollTop, sidebar.scrollLeft, shell.scrollTop, shell.scrollLeft],
      trashCount: document.querySelector(".checkout-trash-button")!.getAttribute("data-count"),
    };
  });
}

/** Execute a real lower Delete, but hold its browser completion after the server.
 *
 * A controlled notice field tests the UI publication seam honestly. Removing
 * the now-deleted source through a later listing settles row/count/header
 * effects before another popup opens. Completion's explicit listing refresh
 * then proves its callback passed publication, even when notice is absent.
 */
async function heldLowerDelete(page: Page, request: APIRequestContext, fixture: HeaderFixture, id: string, notice?: string) {
  let release = () => {};
  const held = new Promise<void>((resolve) => { release = resolve; });
  releases.push(release);
  let fetched = false;
  const pathname = `/api/sessions/${encodeURIComponent(id)}`;
  await page.route((url) => url.pathname === pathname, async (route) => {
    if (route.request().method() !== "DELETE") return route.continue();
    const response = await route.fetch();
    expect(response.ok(), "the real owned Delete must succeed before the wire fixture").toBe(true);
    expect(requireHelmBuild(response, "held Delete fixture")).toBe(fixture.build);
    const actual: DeleteOutcome = await response.json();
    expect(actual.archived ?? false, "an unmanaged fixture must not create a trash cue").toBe(false);
    expect(actual.notice ?? null, "the ordinary backend Delete supplies no preservation notice").toBeNull();
    const controlled: DeleteOutcome = notice === undefined ? actual : { ...actual, archived: false, notice };
    fetched = true;
    await held;
    await route.fulfill({ response, json: controlled });
  });
  const source = row(page, id);
  await openPopup(page, source, "menu");
  const action = source.locator(".session-row-delete");
  await action.focus();
  await expect(action).toBeFocused();
  await action.press("Enter");
  await expect.poll(() => fetched, { message: "the server Delete must complete while the browser reply remains held" }).toBe(true);
  await expect(source.locator(".confirm-delete")).toHaveCount(0);
  await expect.poll(async () => {
    const response = await request.get("/api/sessions");
    expect(response.ok()).toBe(true);
    return (await response.json()).sessions.some((session: { id: string }) => session.id === id);
  }, { message: "the owned source must really be absent on the server" }).toBe(false);
  const index = fixture.rows.findIndex((session) => session.id === id);
  expect(index, "only a row below the target may be removed").toBeGreaterThan(0);
  fixture.rows.splice(index, 1);
  await fixture.refresh();
  await expect(source).toHaveCount(0);
  await expect(page.locator(".checkout-trash-button")).toHaveAttribute("data-count", "0");
  return {
    async complete() {
      const deleted = page.waitForResponse((response) =>
        new URL(response.url()).pathname === pathname && response.request().method() === "DELETE"
      );
      const refreshed = page.waitForResponse((response) =>
        new URL(response.url()).pathname === "/api/sessions" && response.request().method() === "GET"
      );
      release();
      expect(await (await deleted).finished()).toBeNull();
      const response = await refreshed;
      expect(response.ok()).toBe(true);
      expect(response.headers()["x-farhelm-build"]).toBe(fixture.build);
      expect(await response.finished()).toBeNull();
      await settle(page);
      await expect(page.locator(".session-heading .session-count")).toHaveText(String(fixture.rows.length));
    },
  };
}

/** Resize through the actual separator before measuring/opening any popup. */
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

/** Read natural flex-item widths while the wide header fits on one line. */
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
  /** The lower source and count already disappeared before opening the target.
   * Releasing only the typed notice completion must insert the band and move
   * the surviving target, without relying on row reorder or header wrapping.
   */
  test(`a completed Delete notice moves the anchor and closes an open ${kind}`, async ({ page, request }) => {
    const fixture = await prepare(page, request, { lowerCount: 1 });
    const deletion = await heldLowerDelete(page, request, fixture, fixture.lower[0], NOTICE);
    await expect(page.locator(".delete-notice")).toHaveCount(0);
    const target = row(page, fixture.target);
    const { toggle, panel, focus } = await openPopup(page, target, kind);
    const { y: beforeY, ...before } = await geometry(target);
    expect(before.index).toBe(0);
    await expect(focus).toBeFocused();
    await deletion.complete();
    await expect(page.locator(".delete-notice .status")).toHaveText(NOTICE);
    expect((await page.locator(".delete-notice").boundingBox())!.height).toBeGreaterThan(0);
    const { y: afterY, ...after } = await geometry(target);
    expect(afterY).toBeGreaterThan(beforeY);
    expect(after, "the notice alone must move the target after count and lower-row removal settled").toEqual(before);
    await expect(panel).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
  });

  /** A retained band is a baseline for a later opening. Activate the real
   * Dismiss button without a pointerdown/focus transfer, which would otherwise
   * close the popup before the removal being tested.
   */
  test(`dismissing a retained Delete notice moves the anchor and closes an open ${kind}`, async ({ page, request }) => {
    const fixture = await prepare(page, request, { lowerCount: 1 });
    await (await heldLowerDelete(page, request, fixture, fixture.lower[0], NOTICE)).complete();
    const notice = page.locator(".delete-notice");
    await expect(notice.locator(".status")).toHaveText(NOTICE);
    const target = row(page, fixture.target);
    const { toggle, panel, focus } = await openPopup(page, target, kind);
    const { y: beforeY, ...before } = await geometry(target);
    await expect(focus).toBeFocused();
    const activation = await notice.getByRole("button", { name: "Dismiss", exact: true }).evaluate((button: HTMLButtonElement) => {
      const focused = document.activeElement;
      const popupOwnedFocus = !!focused?.closest(".session-row-menu-panel, .session-bell-panel");
      button.click();
      return { popupOwnedFocus, unchangedOrRemoved: document.activeElement === focused || focused?.isConnected === false };
    });
    expect(activation).toEqual({ popupOwnedFocus: true, unchangedOrRemoved: true });
    await expect(notice).toHaveCount(0);
    const { y: afterY, ...after } = await geometry(target);
    expect(afterY).toBeLessThan(beforeY);
    expect(after, "dismissing the band must leave the header and all other layout inputs unchanged").toEqual(before);
    await expect(panel).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
  });

  /** publish(None) preserves an existing notice. A real second Delete finishes
   * after its row/count have already settled, so it must leave the popup and
   * physical anchor unchanged rather than dismiss on every successful Delete.
   */
  test(`a Delete with no notice preserves the retained band and an open ${kind}`, async ({ page, request }) => {
    const fixture = await prepare(page, request, { lowerCount: 2 });
    await (await heldLowerDelete(page, request, fixture, fixture.lower[0], NOTICE)).complete();
    await expect(page.locator(".delete-notice .status")).toHaveText(NOTICE);
    const deletion = await heldLowerDelete(page, request, fixture, fixture.lower[1]);
    const target = row(page, fixture.target);
    const { toggle, panel, focus } = await openPopup(page, target, kind);
    const before = await geometry(target);
    await expect(focus).toBeFocused();
    await deletion.complete();
    await expect(page.locator(".delete-notice .status")).toHaveText(NOTICE);
    expect(await geometry(target), "a no-notice completion must not move the settled anchor").toEqual(before);
    await expect(panel).toBeVisible();
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await expect(focus).toBeFocused();
  });

  /** A tenth row appended below an unchanged prefix changes only CountBanner's
   * digit width at the prepared header threshold. A preceding title-only
   * refresh proves an unchanged banner/prefix does not dismiss the popup.
   */
  test(`a wrapping count banner moves the anchor and closes an open ${kind}`, async ({ page, request }) => {
    const fixture = await prepare(page, request, { fillerCount: 7 });
    await sidebarWidth(page, 656);
    const nine = await headerMetrics(page);
    const tenth = fixture.filler(7);
    expect(fixture.rows.some((session) => session.id === tenth.id), "the appended row must have a unique routing identity").toBe(false);
    fixture.rows.push(tenth);
    await fixture.refresh();
    const ten = await headerMetrics(page);
    expect(ten.total, "the extra decimal digit must increase the real header's natural width").toBeGreaterThan(nine.total);
    fixture.rows.pop();
    await fixture.refresh();
    // The fit interval can straddle the supported minimum even when its
    // midpoint falls below it. Try that boundary; both fit assertions below
    // still have to prove that nine fits and ten really wraps.
    const desired = Math.max(296, Math.round(nine.sidebar + (nine.total + ten.total) / 2 - nine.available));
    await sidebarWidth(page, desired);
    const fitted = await headerMetrics(page);
    expect(fitted.height, "nine sessions must still fit the original single header line").toBe(nine.height);
    expect(fitted.actionsX).toBeGreaterThanOrEqual(fitted.titleRight);
    expect(fitted.available).toBeGreaterThanOrEqual(nine.total);
    expect(fitted.available).toBeLessThan(ten.total);
    const target = row(page, fixture.target);
    const { toggle, panel, focus } = await openPopup(page, target, kind);
    const initial = await geometry(target);
    const last = fixture.rows[fixture.rows.length - 1];
    last.title = `${last.title}-unchanged-banner-committed`;
    await fixture.refresh();
    await expect(row(page, last.id).locator(".session-title")).toHaveText(last.title);
    expect(await geometry(target), "an unchanged banner and below-row title update must move nothing").toEqual(initial);
    await expect(panel).toBeVisible();
    await expect(focus).toBeFocused();
    const prefix = fixture.rows.map((session) => session.id);
    const reads = fixture.reads();
    const release = fixture.holdListings();
    fixture.rows.push(tenth);
    const reply = page.waitForResponse((response) =>
      new URL(response.url()).pathname === "/api/sessions" && response.request().method() === "GET"
    );
    fixture.notify();
    await expect.poll(fixture.reads, { message: "the tenth-row response must be held before publication" }).toBeGreaterThan(reads);
    await expect(page.locator(".session-heading .session-count")).toHaveText("9");
    await expect(panel).toBeVisible();
    await expect(focus).toBeFocused();
    expect(await geometry(target)).toEqual(initial);
    release();
    const response = await reply;
    expect(response.ok()).toBe(true);
    expect(response.headers()["x-farhelm-build"]).toBe(fixture.build);
    expect(await response.finished()).toBeNull();
    await settle(page);
    await expect(page.locator(".session-heading .session-count")).toHaveText("10");
    const { y: beforeY, headingHeight: beforeHeight, ...before } = initial;
    const { y: afterY, headingHeight: afterHeight, ...after } = await geometry(target);
    expect(afterHeight, "the actions must wrap onto a second header line").toBeGreaterThan(beforeHeight);
    expect(afterY, "header growth must move the unchanged first row").toBeGreaterThan(beforeY);
    expect(after, "the banner must not rely on scroll, host or viewport movement").toEqual(before);
    const wrapped = await headerMetrics(page);
    expect(wrapped.actionsY).toBeGreaterThan(wrapped.titleBottom);
    const shown = await page.locator(".session-row").evaluateAll((nodes) => nodes.map((node) => node.getAttribute("data-session-id")));
    expect(shown.slice(0, prefix.length)).toEqual(prefix);
    expect(shown.at(-1)).toBe(tenth.id);
    await expect(panel).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
  });

  /** A failed read replaces every row with the error line, and the reply
   * that recovers redraws them. A popup left open across the failure must
   * not come back with them, wherever its row now lands. This covers the
   * ordering where the error line renders; a recovery landing before it
   * renders is not something this fixture can schedule.
   */
  test(`recovering from a failed listing closes an open ${kind}`, async ({ page, request }) => {
    const fixture = await prepare(page, request);
    const target = row(page, fixture.target);
    const { toggle, panel } = await openPopup(page, target, kind);
    fixture.failListings(true);
    const failed = page.waitForResponse((response) =>
      new URL(response.url()).pathname === "/api/sessions" && response.request().method() === "GET"
    );
    fixture.notify();
    expect((await failed).status()).toBe(503);
    await expect(page.locator(".status.error")).toContainText("Failed to load sessions");
    await expect(page.locator(".session-row")).toHaveCount(0);
    fixture.failListings(false);
    await fixture.refresh();
    await expect(panel).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
  });
}
