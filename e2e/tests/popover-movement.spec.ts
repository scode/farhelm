import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { expect, test } from "./helpers/evidence";
import {
  cleanupSession,
  createSession,
  observeFeedReaders,
  openRowMenu,
  patchPreferences,
  pinAutoSelect,
  readPreferences,
  resetPreferences,
  stubFeed,
  waitForFeedReadersSettled,
} from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { waitForSessionReady } from "./helpers/terminal-readiness";

// These regressions distinguish causal movement dismissal from an outside
// pointer press or a late mount-time read. Real sessions supply the terminal
// and action routes; a frozen listing keeps classification and order unchanged
// while each test exercises one existing movement source.
type PopupKind = "menu" | "bell";
/** Only these fields change later; the rest remain from the real listing. */
type FixtureRow = {
  id: string;
  title: string;
  status: { state: string; exit_code?: number };
  annotation?: string;
  host?: number;
  stale?: boolean;
};
const created: string[] = [];
const releases: (() => void)[] = [];

test.afterEach(async ({ page, request }, testInfo) => {
  await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
    ["held requests", () => { releases.splice(0).forEach((release) => release()); }],
    ["routes", () => page.unrouteAll({ behavior: "ignoreErrors" })],
    ...created.splice(0).reverse().map((id): [string, () => Promise<void>] => [
      `session ${id}`, () => cleanupSession(request, id),
    ]),
    ["preferences", () => resetPreferences(request)],
  ]);
});

/** Each completed allocation enters cleanup before another allocation can fail. */
async function ownedSession(request: APIRequestContext, title: string): Promise<string> {
  const session = await createSession(request, { title, invocation: "sleep 300" });
  created.push(session.id);
  return session.id;
}

/** Locate the exact real row; session references remain opaque routing identities. */
function row(page: Page, id: string): Locator {
  return page.locator(`[data-session-id="${id}"]`);
}

/** Freeze only listing data, leaving the selected session's attachment real.
 *
 * All rows are live and equally tall, and selection is below both measured
 * rows. The preceding row therefore offers a manual seen action without an
 * automatic mark-on-open entering the held route. The already-read target
 * notification keeps ordinary bell dismissal from starting another refresh.
 * An optional second prefix row permits a permutation wholly above the target
 * without changing the three-row premise of the original wiring regressions.
 */
async function prepare(page: Page, request: APIRequestContext, extraAbove = false) {
  const stamp = Date.now();
  const above = await ownedSession(request, `aaa-movement-${stamp}`);
  const target = await ownedSession(request, `bbb-movement-${stamp}`);
  const selected = await ownedSession(request, `ccc-movement-${stamp}`);
  const otherAbove = extraAbove ? await ownedSession(request, `aab-movement-${stamp}`) : undefined;
  await patchPreferences(request, { welcome_seen: true, compact: false, list_sort: "title" });
  await pinAutoSelect(page, selected);
  const response = await request.get("/api/sessions");
  expect(response.ok(), "the real fixture listing must succeed").toBe(true);
  const build = requireHelmBuild(response, "popover movement fixture");
  const listing = await response.json();
  const now = Math.floor(Date.now() / 1000);
  const ids = [above, ...(otherAbove ? [otherAbove] : []), target, selected];
  const sessions: FixtureRow[] = ids.map((id) => {
    const session = listing.sessions.find((candidate: { id: string }) => candidate.id === id);
    expect(session, "every owned session must be present before its listing is frozen").toBeTruthy();
    return {
      ...session,
      status: { state: "idle" },
      stale: false,
      pinned: false,
      last_activity_at: now - 30,
      seen_activity_at: 0,
      notifications: id === target ? [{ seq: 1, at: now, text: "movement fixture notification" }] : [],
      notifications_read_through: 1,
    };
  });
  await page.route((url) => url.pathname === "/api/sessions", async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    await route.fulfill({
      headers: { "x-farhelm-build": build, "content-type": "application/json" },
      json: { ...listing, sessions, total: sessions.length, matching: sessions.length, truncated: false },
    });
  });
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  await observeFeedReaders(page);
  await page.setViewportSize({ width: 1280, height: 1200 });
  await page.goto("/");
  await waitForSessionReady(page, selected);
  await feed.waitForConnection(1);
  await expect(page.locator(".hosts-status", { hasText: "loading hosts" })).toHaveCount(0);
  await expect(page.locator(".host-row").first()).toBeVisible();
  await expect(page.locator(".session-row")).toHaveCount(sessions.length);
  await expect(page.locator(".session-row").nth(0)).toHaveAttribute("data-session-id", above);
  await expect(page.locator(".session-row").nth(extraAbove ? 2 : 1)).toHaveAttribute("data-session-id", target);
  await expect(row(page, target)).toHaveAttribute("data-session-selected", "false");
  await expect(page.locator(".compact-toggle input")).not.toBeChecked();
  expect(await page.locator(".app-sidebar").evaluate((node) => node.scrollHeight <= node.clientHeight),
    "fixture rows must fit without scrolling").toBe(true);
  await waitForFeedReadersSettled(page);
  let revision = 1;
  return {
    above, otherAbove, target, selected, build, sessions,
    // Host-only cases observe the frozen listing and gated host reply
    // separately; both still use the same monotonically advancing feed.
    notify() { feed.notify(++revision); },
    /** A completed stamped reply and render boundary precede outcome checks.
     * Changed row content/order remains the independent oracle at each call
     * site; the frame pair also drains an unchanged held-order render before
     * a negative assertion, as the existing frozen-listing regression does.
     */
    async refresh() {
      const reply = page.waitForResponse((response) =>
        new URL(response.url()).pathname === "/api/sessions" && response.request().method() === "GET"
      );
      feed.notify(++revision);
      const response = await reply;
      expect(response.ok(), "the controlled listing refresh must succeed").toBe(true);
      expect(response.headers()["x-farhelm-build"]).toBe(build);
      expect((await response.json()).sessions.map((session: { id: string }) => session.id),
        "the completed response must carry the staged order").toEqual(sessions.map((session) => session.id));
      expect(await response.finished(), "the controlled reply must finish").toBeNull();
      await waitForFeedReadersSettled(page);
      await finishRender(page);
    },
  };
}

/** A popup must own focus and have measured placement before movement starts. */
async function openPopup(page: Page, target: Locator, kind: PopupKind) {
  const toggle = target.locator(kind === "menu" ? ".session-row-menu" : ".session-row-bell");
  const panel = target.locator(kind === "menu" ? ".session-row-menu-panel" : ".session-bell-panel");
  if (kind === "menu") {
    await openRowMenu(target);
    await expect(target.locator(".session-row-rename")).toBeFocused();
  } else {
    await toggle.click();
    await expect(panel).toBeFocused();
    const flyout = target.locator(".session-bell-flyout");
    await expect(flyout).toHaveCSS("opacity", "1");
    await expect(flyout).toHaveAttribute("style", /left: var\(--menu-left\)/);
  }
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  return { toggle, panel };
}

/** Record competing layout causes beside the target's actual screen position.
 *
 * A closed popup alone cannot identify the source. Stable viewport, scrolling,
 * host shape and row index exclude existing invalidators while target y proves
 * that the compact/refusal transition really changes the measured anchor.
 */
async function geometry(target: Locator) {
  return target.evaluate((node) => {
    const sidebar = document.querySelector(".app-sidebar")!;
    const shell = document.querySelector(".app-shell")!;
    const hosts = document.querySelector(".hosts-panel")!;
    return {
      y: node.getBoundingClientRect().y,
      index: [...document.querySelectorAll(".session-row")].indexOf(node),
      sidebarTop: sidebar.scrollTop,
      sidebarLeft: sidebar.scrollLeft,
      shellTop: shell.scrollTop,
      shellLeft: shell.scrollLeft,
      viewport: [innerWidth, innerHeight],
      hostsHeight: hosts.getBoundingClientRect().height,
      hostIds: [...hosts.querySelectorAll(".host-row")].map((host) => host.getAttribute("data-host-id")),
      hostDetails: hosts.querySelectorAll(".host-detail").length,
    };
  });
}

for (const kind of ["menu", "bell"] as const) {
  /** Compact changes remove metadata above the anchor. Keyboard activation
   * avoids outside-pointer dismissal for menus. A bell intentionally closes
   * when focus leaves its panel, so its leg activates the actual checkbox with
   * HTMLElement.click(): native checking/change behavior, with no pointerdown
   * or focus transfer that could dismiss the bell before its row moves.
   */
  test(`compact rows move the anchor and close an open ${kind}`, async ({ page, request }) => {
    const fixture = await prepare(page, request);
    const target = row(page, fixture.target);
    await expect(row(page, fixture.above).locator(".session-row-meta")).toBeVisible();
    const { toggle, panel } = await openPopup(page, target, kind);
    const compact = page.locator(".compact-toggle input");
    if (kind === "menu") {
      await compact.focus();
      await expect(compact).toBeFocused();
      await expect(panel, "focus transfer alone must leave the actions menu open").toBeVisible();
      await expect(toggle).toHaveAttribute("aria-expanded", "true");
    }
    const { y: beforeY, ...before } = await geometry(target);
    if (kind === "menu") {
      await page.keyboard.press("Space");
    } else {
      await expect(panel).toBeFocused();
      const activation = await compact.evaluate((node: HTMLInputElement) => {
        const focused = document.activeElement;
        const panelOwnedFocus = focused?.classList.contains("session-bell-panel") === true;
        node.click();
        return {
          panelOwnedFocus,
          checked: node.checked,
          focusUnchangedOrPanelRemoved: document.activeElement === focused || focused?.isConnected === false,
        };
      });
      expect(activation).toEqual({ panelOwnedFocus: true, checked: true, focusUnchangedOrPanelRemoved: true });
    }
    await expect(compact).toBeChecked();
    await expect(row(page, fixture.above).locator(".session-row-meta")).toHaveCount(0);
    await expect.poll(async () => (await geometry(target)).y, {
      message: "removing the preceding row's metadata must move the target upward",
    }).toBeLessThan(beforeY);
    const { y: afterY, ...after } = await geometry(target);
    expect(afterY).toBeLessThan(beforeY);
    expect(after, "compact dismissal must not depend on scrolling, host shape, viewport or row reordering").toEqual(before);
    await expect(panel).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    // Complete the real preference write before cleanup resets its shared row.
    await expect.poll(async () => (await readPreferences(request)).compact).toBe(true);
  });

  /** A late manual-mark refusal adds height above an already-open popup.
   * Acceptance and its old focus handback finish before the target opens;
   * releasing the held stamped refusal is then the only changing input.
   */
  test(`a refusal above moves the anchor and closes an open ${kind}`, async ({ page, request }) => {
    const fixture = await prepare(page, request);
    let release = () => {};
    const held = new Promise<void>((resolve) => { release = resolve; });
    releases.push(release);
    let requests = 0;
    await page.route((url) => url.pathname === `/api/sessions/${encodeURIComponent(fixture.above)}/seen`, async (route) => {
      expect(route.request().method()).toBe("PUT");
      expect(route.request().postDataJSON().seen_activity_at,
        "only the unselected preceding row's manual read may enter the hold").not.toBeNull();
      requests++;
      await held;
      await route.fulfill({
        status: 500,
        headers: { "x-farhelm-build": fixture.build, "content-type": "text/plain" },
        body: "popover-movement-refusal-sentinel",
      });
    });
    const above = row(page, fixture.above);
    await openRowMenu(above);
    await above.getByRole("menuitem", { name: "mark read", exact: true }).click();
    await expect.poll(() => requests, { message: "the manual seen write must reach the held route" }).toBe(1);
    await expect(above.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(above.locator(".session-row-menu")).toBeFocused();
    await expect(above.locator(".action-error")).toHaveCount(0);
    const target = row(page, fixture.target);
    const { toggle, panel } = await openPopup(page, target, kind);
    const { y: beforeY, ...before } = await geometry(target);
    expect((await above.boundingBox())!.y, "the held refusal belongs above the target").toBeLessThan(beforeY);
    release();
    await expect(above.locator(".action-error")).toContainText("popover-movement-refusal-sentinel");
    await expect.poll(async () => (await geometry(target)).y, {
      message: "the preceding row's refusal must physically push the target down",
    }).toBeGreaterThan(beforeY);
    const { y: afterY, ...after } = await geometry(target);
    expect(afterY).toBeGreaterThan(beforeY);
    expect(after, "refusal dismissal must not depend on scrolling, host shape, viewport or row reordering").toEqual(before);
    await expect(panel).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(requests, "no automatic seen write may impersonate the manual failure").toBe(1);
  });
}

type MovementFixture = Awaited<ReturnType<typeof prepare>>;
const SHORT_DETAIL = "short-detail";
const LONG_DETAIL = `${Array.from({ length: 32 }, (_, i) => `wrapped-detail-${i}`).join(" ")} detail-end-sentinel`;

/** Cross two frame boundaries after observing a source response or error.
 * A later open must not inherit a still-pending refusal report or handback;
 * call sites still assert the corresponding rendered content and geometry.
 */
async function finishRender(page: Page) {
  await page.evaluate(() => new Promise<void>((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
  ));
}

/** A keyboard opening must not take the pointer's displayed-order hold.
 *
 * Hovering through openRowMenu would freeze the very crossing this regression
 * needs. Moving outside while every popup is closed lets a previous hold end;
 * the later rendered-order assertion independently proves that it did end.
 */
async function openKeyboardPopup(page: Page, target: Locator, kind: PopupKind) {
  const viewport = page.viewportSize()!;
  await page.mouse.move(viewport.width - 8, viewport.height - 8);
  expect(await page.locator(".session-list").evaluate((node) => node.matches(":hover")),
    "the pointer must remain outside the list before keyboard opening").toBe(false);
  const toggle = target.locator(kind === "menu" ? ".session-row-menu" : ".session-row-bell");
  const panel = target.locator(kind === "menu" ? ".session-row-menu-panel" : ".session-bell-panel");
  await toggle.focus();
  await expect(toggle).toBeFocused();
  await toggle.press(kind === "menu" ? "ArrowDown" : "Enter");
  await expect(kind === "menu" ? target.locator(".session-row-rename") : panel).toBeFocused();
  const flyout = target.locator(kind === "menu" ? ".session-row-menu-flyout" : ".session-bell-flyout");
  await expect(flyout).toHaveCSS("opacity", "1");
  await expect(flyout).toHaveAttribute("style", /left: var\(--menu-left\)/);
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  expect(await page.locator(".session-list").evaluate((node) => node.matches(":hover"))).toBe(false);
  return { toggle, panel };
}

/** Produce the retained error through the real manual action before reordering.
 *
 * The held request distinguishes acceptance/handback from refusal rendering.
 * Later tests change only listing order/content; no second error write occurs.
 */
async function retainRefusal(page: Page, fixture: MovementFixture) {
  let release = () => {};
  const held = new Promise<void>((resolve) => { release = resolve; });
  releases.push(release);
  let requests = 0;
  await page.route((url) => url.pathname === `/api/sessions/${encodeURIComponent(fixture.above)}/seen`, async (route) => {
    expect(route.request().method()).toBe("PUT");
    expect(route.request().postDataJSON().seen_activity_at,
      "the retained refusal must answer the unselected row's manual read").not.toBeNull();
    requests++;
    await held;
    await route.fulfill({
      status: 500,
      headers: { "x-farhelm-build": fixture.build, "content-type": "text/plain" },
      body: "retained-crossing-refusal-sentinel",
    });
  });
  const above = row(page, fixture.above);
  await openKeyboardPopup(page, above, "menu");
  const mark = above.getByRole("menuitem", { name: "mark read", exact: true });
  await mark.focus();
  await expect(mark).toBeFocused();
  await mark.press("Enter");
  await expect.poll(() => requests, { message: "the manual read must reach its held route" }).toBe(1);
  await expect(above.locator(".session-row-menu-panel")).toHaveCount(0);
  await expect(above.locator(".session-row-menu")).toBeFocused();
  await expect(above.locator(".action-error")).toHaveCount(0);
  release();
  await expect(above.locator(".action-error")).toContainText("retained-crossing-refusal-sentinel");
  await finishRender(page);
  return { text: await above.locator(".action-error").innerText(), requests: () => requests };
}

/** Change served title order without changing row identity or base height. */
function orderListing(fixture: MovementFixture, ids: string[], marker: string) {
  const reordered = ids.map((id) => {
    const session = fixture.sessions.find((candidate) => candidate.id === id);
    if (!session) throw new Error("the requested fixture order contains an unknown session");
    return session;
  });
  reordered.forEach((session, index) => { session.title = `${String(index).padStart(2, "0")}-${marker}`; });
  fixture.sessions.splice(0, fixture.sessions.length, ...reordered);
}

/** The DOM order is the movement premise; the helm's served order is not enough. */
async function renderedOrder(page: Page) {
  return page.locator(".session-row").evaluateAll((nodes) => nodes.map((node) => node.getAttribute("data-session-id")));
}

for (const kind of ["menu", "bell"] as const) {
  /** Existing detail presence and target index stay fixed while new annotation
   * text adds real lines. Counting detail rows alone misses this movement.
   */
  test(`rewrapping an existing detail moves the anchor and closes an open ${kind}`, async ({ page, request }) => {
    const fixture = await prepare(page, request);
    const changed = fixture.sessions.find((session) => session.id === fixture.above)!;
    changed.status = { state: "exited", exit_code: 0 };
    changed.annotation = SHORT_DETAIL;
    await fixture.refresh();
    const detail = row(page, fixture.above).locator(".session-row-detail");
    await expect(detail).toContainText(SHORT_DETAIL);
    await expect(page.locator(".session-row-detail")).toHaveCount(1);
    const target = row(page, fixture.target);
    const { toggle, panel } = await openKeyboardPopup(page, target, kind);
    const detailHeight = (await detail.boundingBox())!.height;
    const { y: beforeY, ...before } = await geometry(target);
    changed.annotation = LONG_DETAIL;
    await fixture.refresh();
    await expect(detail).toContainText("detail-end-sentinel");
    await expect(page.locator(".session-row-detail")).toHaveCount(1);
    expect((await detail.boundingBox())!.height, "the existing detail must really wrap onto more lines").toBeGreaterThan(detailHeight);
    const { y: afterY, ...after } = await geometry(target);
    expect(afterY, "the unchanged-index target must be pushed down by the rewrap").toBeGreaterThan(beforeY);
    expect(after, "detail rewrap must not be attributed to another layout source").toEqual(before);
    expect(await renderedOrder(page)).toEqual([fixture.above, fixture.target, fixture.selected]);
    await expect(panel).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
  });

  /** The same annotation is hidden in Compact. A real committed content change
   * must keep the popup open when it cannot move the displayed anchor.
   */
  test(`a compact-hidden detail change preserves an open ${kind}`, async ({ page, request }) => {
    const fixture = await prepare(page, request);
    const compact = page.locator(".compact-toggle input");
    await compact.check();
    await expect(compact).toBeChecked();
    await expect.poll(async () => (await readPreferences(request)).compact).toBe(true);
    const changed = fixture.sessions.find((session) => session.id === fixture.above)!;
    changed.status = { state: "exited", exit_code: 0 };
    changed.annotation = SHORT_DETAIL;
    await fixture.refresh();
    const ended = row(page, fixture.above).locator(".compact-ended-status");
    await expect(ended).toHaveAttribute("data-tooltip", new RegExp(SHORT_DETAIL));
    await expect(page.locator(".session-row-detail")).toHaveCount(0);
    const target = row(page, fixture.target);
    const { toggle, panel } = await openKeyboardPopup(page, target, kind);
    const before = await geometry(target);
    changed.annotation = LONG_DETAIL;
    changed.title = "aaa-hidden-detail-committed";
    await fixture.refresh();
    await expect(row(page, fixture.above).locator(".session-title")).toHaveText(changed.title);
    await expect(ended).toHaveAttribute("data-tooltip", /detail-end-sentinel/);
    await expect(page.locator(".session-row-detail")).toHaveCount(0);
    expect(await geometry(target), "hidden detail must not move the anchor or another layout input").toEqual(before);
    await expect(panel).toBeVisible();
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await expect(kind === "menu" ? target.locator(".session-row-rename") : panel).toBeFocused();
  });

  for (const direction of ["below to above", "above to below"] as const) {
    /** The error map is unchanged. Only the displayed prefix changes while
     * the anchor remains in the middle, defeating index-only reconciliation.
     */
    test(`an unchanged refusal crossing ${direction} closes an open ${kind}`, async ({ page, request }) => {
      const fixture = await prepare(page, request);
      const retained = await retainRefusal(page, fixture);
      const initial = direction === "below to above"
        ? [fixture.selected, fixture.target, fixture.above]
        : [fixture.above, fixture.target, fixture.selected];
      const incoming = [...initial].reverse();
      orderListing(fixture, initial, "crossing-before");
      await fixture.refresh();
      expect(await renderedOrder(page)).toEqual(initial);
      await expect(row(page, fixture.above).locator(".action-error")).toHaveText(retained.text);
      await expect(page.locator(".session-row-detail")).toHaveCount(0);
      await expect(page.locator(".session-pin-separator")).toHaveCount(0);
      const target = row(page, fixture.target);
      const { toggle, panel } = await openKeyboardPopup(page, target, kind);
      const { y: beforeY, ...before } = await geometry(target);
      expect(before.index, "the target must begin between the exchanging rows").toBe(1);
      orderListing(fixture, incoming, "crossing-after");
      await fixture.refresh();
      expect(await renderedOrder(page)).toEqual(incoming);
      await expect(row(page, fixture.above).locator(".session-title")).toContainText("crossing-after");
      await expect(row(page, fixture.above).locator(".action-error")).toHaveText(retained.text);
      await expect(page.locator(".session-row-detail")).toHaveCount(0);
      const { y: afterY, ...after } = await geometry(target);
      if (direction === "below to above") expect(afterY).toBeGreaterThan(beforeY);
      else expect(afterY).toBeLessThan(beforeY);
      expect(after, "only the retained refusal's displayed prefix contribution may move the anchor").toEqual(before);
      expect(await page.locator(".session-list").evaluate((node) => node.matches(":hover"))).toBe(false);
      await expect(panel).toHaveCount(0);
      await expect(toggle).toHaveAttribute("aria-expanded", "false");
      expect(retained.requests(), "crossing must not write or replace the retained error").toBe(1);
    });
  }

  /** Exchanging rows wholly above the target preserves the same detail and
   * error height. A prefix-identity comparison would dismiss unnecessarily.
   */
  test(`a harmless prefix permutation preserves an open ${kind}`, async ({ page, request }) => {
    const fixture = await prepare(page, request, true);
    const retained = await retainRefusal(page, fixture);
    const changed = fixture.sessions.find((session) => session.id === fixture.above)!;
    changed.status = { state: "exited", exit_code: 0 };
    changed.annotation = SHORT_DETAIL;
    await fixture.refresh();
    await expect(row(page, fixture.above).locator(".session-row-detail")).toContainText(SHORT_DETAIL);
    await expect(row(page, fixture.above).locator(".action-error")).toHaveText(retained.text);
    const target = row(page, fixture.target);
    const { toggle, panel } = await openKeyboardPopup(page, target, kind);
    const before = await geometry(target);
    expect(before.index).toBe(2);
    const incoming = [fixture.otherAbove!, fixture.above, fixture.target, fixture.selected];
    orderListing(fixture, incoming, "prefix-swapped");
    await fixture.refresh();
    expect(await renderedOrder(page)).toEqual(incoming);
    await expect(row(page, fixture.above).locator(".session-title")).toContainText("prefix-swapped");
    await expect(row(page, fixture.above).locator(".session-row-detail")).toContainText(SHORT_DETAIL);
    await expect(page.locator(".session-row-detail")).toHaveCount(1);
    await expect(row(page, fixture.above).locator(".action-error")).toHaveText(retained.text);
    expect(await geometry(target), "the unchanged aggregate prefix must leave every measured input alone").toEqual(before);
    await expect(panel).toBeVisible();
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await expect(kind === "menu" ? target.locator(".session-row-rename") : panel).toBeFocused();
    expect(retained.requests()).toBe(1);
  });

  /** A served crossing does not move a held display. This leg intentionally
   * uses the pointer-opening helper to take a hold, then proves new titles
   * were rendered in the old order while its popup and focus survive.
   */
  test(`a held served refusal reorder preserves an open ${kind}`, async ({ page, request }) => {
    const fixture = await prepare(page, request);
    const retained = await retainRefusal(page, fixture);
    const initial = [fixture.above, fixture.target, fixture.selected];
    expect(await renderedOrder(page)).toEqual(initial);
    const target = row(page, fixture.target);
    const { toggle, panel } = await openPopup(page, target, kind);
    const viewport = page.viewportSize()!;
    await page.mouse.move(viewport.width - 8, viewport.height - 8);
    expect(await page.locator(".session-list").evaluate((node) => node.matches(":hover"))).toBe(false);
    const before = await geometry(target);
    orderListing(fixture, [...initial].reverse(), "held-served-crossing");
    await fixture.refresh();
    await expect(row(page, fixture.above).locator(".session-title")).toContainText("held-served-crossing");
    expect(await renderedOrder(page)).toEqual(initial);
    await expect(row(page, fixture.above).locator(".action-error")).toHaveText(retained.text);
    expect(await geometry(target), "a served permutation under the popup's hold must move nothing").toEqual(before);
    await expect(panel).toBeVisible();
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await expect(kind === "menu" ? target.locator(".session-row-rename") : panel).toBeFocused();
    expect(retained.requests()).toBe(1);
  });
}

/** The real HostPhase payload whose protocol ordering changes the stale cause.
 * Keeping the tag fixed permits a cause change without a phase transition.
 */
type VersionSkewState = {
  phase: "version-skew";
  peer_protocol: number;
  peer_build: string;
  our_protocol: number;
  our_build: string;
  remediation: string;
};

/** Preserve the real host registry and vary one host's decoded version state.
 * The last publication can be held while the identical listing finishes, so
 * the test observes the popup immediately before the host reader publishes.
 */
async function controlHostCause(page: Page, request: APIRequestContext, fixture: MovementFixture, hostId: number) {
  const response = await request.get("/api/hosts");
  expect(response.ok(), "the real host registry must be available").toBe(true);
  expect(requireHelmBuild(response, "host-only movement fixture")).toBe(fixture.build);
  const body = await response.json();
  const host = body.hosts.find((candidate: { id: number }) => candidate.id === hostId);
  expect(host, "the stale row must name a real registered host").toBeTruthy();
  // Retire independent progress/disclosure causes before measuring the host
  // publication. Reuse completed real replies rather than fabricate another
  // presentation state whose trace behavior the test would have to assume.
  const progress = new Map<number, unknown>();
  for (const candidate of body.hosts as { id: number }[]) {
    const reply = await request.get(`/api/hosts/${candidate.id}/provisioning`);
    expect(reply.ok(), "fixture hosts must have readable provisioning state").toBe(true);
    expect(requireHelmBuild(reply, "frozen provisioning fixture")).toBe(fixture.build);
    const view = await reply.json();
    expect(view.status, "no provisioning transition may accompany the cause change").toBe("completed");
    progress.set(candidate.id, view);
  }
  await page.route((url) => /^\/api\/hosts\/\d+\/provisioning$/.test(url.pathname), async (route) => {
    expect(route.request().method()).toBe("GET");
    const id = Number(new URL(route.request().url()).pathname.split("/")[3]);
    expect(progress.has(id), "only captured fixture hosts may request progress").toBe(true);
    await route.fulfill({
      headers: { "x-farhelm-build": fixture.build, "content-type": "application/json" },
      json: progress.get(id),
    });
  });
  let newer = true;
  let gate = Promise.resolve();
  let requests = 0;
  const state = (isNewer: boolean): VersionSkewState => ({
    phase: "version-skew",
    peer_protocol: isNewer ? 3 : 1,
    peer_build: isNewer ? "9.0.0" : "0.1.0",
    our_protocol: 2,
    our_build: fixture.build,
    remediation: isNewer ? "update the helm" : "update the host",
  });
  await page.route((url) => url.pathname === "/api/hosts", async (route) => {
    expect(route.request().method()).toBe("GET");
    const snapshot = {
      ...body,
      hosts: body.hosts.map((candidate: { id: number }) =>
        candidate.id === hostId ? { ...candidate, state: state(newer) } : candidate
      ),
    };
    requests++;
    await gate;
    await route.fulfill({
      headers: { "x-farhelm-build": fixture.build, "content-type": "application/json" },
      json: snapshot,
    });
  });
  return {
    state,
    requests: () => requests,
    setNewer(value: boolean) { newer = value; },
    holdOlder() {
      newer = false;
      let release = () => {};
      gate = new Promise<void>((resolve) => { release = resolve; });
      releases.push(release);
      return release;
    },
  };
}

/** Record the rendered host shape that must remain unchanged at publication. */
async function hostShape(page: Page) {
  return page.locator(".hosts-panel").evaluate((node) => ({
    width: node.getBoundingClientRect().width,
    height: node.getBoundingClientRect().height,
    rows: [...node.querySelectorAll(".host-row")].map((host) => ({
      id: host.getAttribute("data-host-id"),
      kind: host.getAttribute("data-host-kind"),
      phase: host.getAttribute("data-host-phase"),
    })),
    errors: node.querySelectorAll(".hosts-load-error, .hosts-refresh-error").length,
    loading: node.querySelectorAll(".hosts-status").length,
    details: node.querySelectorAll(".host-detail").length,
    traces: node.querySelectorAll(".provisioning-trace").length,
  }));
}

/** Set a supported width through the real separator before any popup opens.
 * The midpoint leaves room for the shorter stale capsule beside the ended
 * text, while the longer capsule has to move to the next flex line. Reading
 * actual widths handles engine/font differences without altering CSS or DOM.
 */
async function fitStalePair(page: Page, detail: Locator, longerCapsuleWidth: number) {
  const measured = await detail.evaluate((node) => {
    const copy = node.querySelector(".session-detail-copy")!;
    return {
      available: copy.getBoundingClientRect().width,
      ended: node.querySelector(".status-badge")!.getBoundingClientRect().width,
      stale: node.querySelector(".stale-badge")!.getBoundingClientRect().width,
      gap: parseFloat(getComputedStyle(copy).columnGap),
      sidebar: parseFloat(getComputedStyle(document.querySelector(".app-sidebar")!).width),
    };
  });
  expect(longerCapsuleWidth, "the replacement cause must be wider than the baseline cause").toBeGreaterThan(measured.stale);
  const available = measured.ended + measured.gap + (measured.stale + longerCapsuleWidth) / 2;
  const desired = Math.round(measured.sidebar + available - measured.available);
  expect(desired, "the wrapping premise must fit the supported sidebar range").toBeGreaterThanOrEqual(240);
  expect(desired).toBeLessThanOrEqual(600);
  const handle = page.getByRole("separator", { name: "Sidebar width" });
  await expect(handle).toHaveAttribute("data-sidebar-bound", "true");
  const box = (await handle.boundingBox())!;
  const x = box.x + box.width / 2;
  const y = box.y + Math.min(200, box.height / 2);
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + desired - measured.sidebar, y, { steps: 8 });
  await page.mouse.up();
  await expect(handle).toHaveAttribute("aria-valuenow", String(desired));
  await finishRender(page);
  const fitted = await detail.evaluate((node) => {
    const copy = node.querySelector(".session-detail-copy")!;
    return {
      available: copy.getBoundingClientRect().width,
      ended: node.querySelector(".status-badge")!.getBoundingClientRect().width,
      stale: node.querySelector(".stale-badge")!.getBoundingClientRect().width,
      gap: parseFloat(getComputedStyle(copy).columnGap),
    };
  });
  expect(fitted.ended + fitted.gap + fitted.stale,
    "the short cause and ended text must fit together before publication").toBeLessThanOrEqual(fitted.available);
  expect(fitted.ended + fitted.gap + longerCapsuleWidth,
    "the longer cause must require another flex line at this supported width").toBeGreaterThan(fitted.available);
}

for (const kind of ["menu", "bell"] as const) {
  /** A successful host read can rewrap an existing stale detail without a
   * listing change or host-panel movement. Keeping version-skew on both sides
   * excludes the older broad shape/resize closer as the reason for dismissal.
   */
  test(`a host-only stale cause rewrap moves the anchor and closes an open ${kind}`, async ({ page, request }) => {
    const fixture = await prepare(page, request);
    const changed = fixture.sessions.find((session) => session.id === fixture.above)!;
    expect(typeof changed.host, "the real row must carry its registered host identity").toBe("number");
    const host = await controlHostCause(page, request, fixture, changed.host!);
    changed.status = { state: "exited", exit_code: 0 };
    changed.annotation = undefined;
    changed.stale = true;
    await fixture.refresh();
    const above = row(page, fixture.above);
    const detail = above.locator(".session-row-detail");
    const cause = detail.locator(".stale-badge");
    await expect(cause).toHaveText("host is too new");
    host.setNewer(false);
    await fixture.refresh();
    await expect(cause).toHaveText("host needs update");
    const longerCapsuleWidth = (await cause.boundingBox())!.width;
    host.setNewer(true);
    await fixture.refresh();
    await expect(cause).toHaveText("host is too new");
    await fitStalePair(page, detail, longerCapsuleWidth);
    await fixture.refresh();
    await expect(cause).toHaveText("host is too new");
    await expect(above).toHaveAttribute("data-session-stale", "true");
    await expect(page.locator(".session-row-detail")).toHaveCount(1);
    await expect(page.locator(".host-details-toggle")).not.toBeChecked();
    const target = row(page, fixture.target);
    const { toggle, panel } = await openKeyboardPopup(page, target, kind);
    const frozen = JSON.stringify(fixture.sessions);
    const endedText = await detail.locator(".status-badge").innerText();
    const requestsBefore = host.requests();
    const releaseHost = host.holdOlder();
    const listingReply = page.waitForResponse((response) =>
      new URL(response.url()).pathname === "/api/sessions" && response.request().method() === "GET"
    );
    const hostReply = page.waitForResponse((response) =>
      new URL(response.url()).pathname === "/api/hosts" && response.request().method() === "GET"
    );
    fixture.notify();
    await expect.poll(host.requests, { message: "the changed host read must be held before publication" }).toBeGreaterThan(requestsBefore);
    const unchangedListing = await listingReply;
    expect(unchangedListing.ok()).toBe(true);
    expect(unchangedListing.headers()["x-farhelm-build"]).toBe(fixture.build);
    expect(JSON.stringify((await unchangedListing.json()).sessions), "the concurrent listing must remain byte-equivalent data").toBe(frozen);
    expect(await unchangedListing.finished()).toBeNull();
    await finishRender(page);
    await expect(cause, "the changed host response has not been published yet").toHaveText("host is too new");
    await expect(panel).toBeVisible();
    await expect(kind === "menu" ? target.locator(".session-row-rename") : panel).toBeFocused();
    const detailHeight = (await detail.boundingBox())!.height;
    const { y: beforeY, ...before } = await geometry(target);
    const beforeHosts = await hostShape(page);
    expect(beforeHosts).toMatchObject({ errors: 0, loading: 0, details: 0, traces: 0 });
    releaseHost();
    const changedHost = await hostReply;
    expect(changedHost.ok()).toBe(true);
    expect(changedHost.headers()["x-farhelm-build"]).toBe(fixture.build);
    expect((await changedHost.json()).hosts.find((candidate: { id: number }) => candidate.id === changed.host).state).toEqual(host.state(false));
    expect(await changedHost.finished()).toBeNull();
    await waitForFeedReadersSettled(page);
    await finishRender(page);
    await expect(cause).toHaveText("host needs update");
    await expect(detail.locator(".status-badge")).toHaveText(endedText);
    await expect(page.locator(".session-row-detail")).toHaveCount(1);
    expect((await detail.boundingBox())!.height, "only the wider host cause must add a detail line").toBeGreaterThan(detailHeight);
    const { y: afterY, ...after } = await geometry(target);
    expect(afterY, "the host-only detail rewrap must move the lower target").toBeGreaterThan(beforeY);
    expect(after, "no scroll, viewport, index or host-panel movement may explain dismissal").toEqual(before);
    expect(await hostShape(page), "the old broad host shape/size inputs must stay unchanged").toEqual(beforeHosts);
    expect(JSON.stringify(fixture.sessions)).toBe(frozen);
    expect(await renderedOrder(page)).toEqual([fixture.above, fixture.target, fixture.selected]);
    await expect(panel).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
  });
}
