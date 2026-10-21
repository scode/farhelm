import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { expect, test } from "./helpers/evidence";
import {
  cleanupSession,
  createSession,
  openRowMenu,
  patchPreferences,
  pinAutoSelect,
  readPreferences,
  resetPreferences,
  stubFeed,
} from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { waitForSessionReady } from "./helpers/terminal-readiness";

// These regressions distinguish causal movement dismissal from an outside
// pointer press or a late mount-time read. Real sessions supply the terminal
// and action routes; a frozen listing keeps classification and order unchanged
// while each test exercises one existing movement source.
type PopupKind = "menu" | "bell";
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
 */
async function prepare(page: Page, request: APIRequestContext) {
  const stamp = Date.now();
  const above = await ownedSession(request, `aaa-movement-${stamp}`);
  const target = await ownedSession(request, `bbb-movement-${stamp}`);
  const selected = await ownedSession(request, `ccc-movement-${stamp}`);
  await patchPreferences(request, { welcome_seen: true, compact: false, list_sort: "title" });
  await pinAutoSelect(page, selected);
  const response = await request.get("/api/sessions");
  expect(response.ok(), "the real fixture listing must succeed").toBe(true);
  const build = requireHelmBuild(response, "popover movement fixture");
  const listing = await response.json();
  const now = Math.floor(Date.now() / 1000);
  const sessions = [above, target, selected].map((id) => {
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
  await page.setViewportSize({ width: 1280, height: 1200 });
  await page.goto("/");
  await waitForSessionReady(page, selected);
  await feed.waitForConnection(1);
  await expect(page.locator(".hosts-status", { hasText: "loading hosts" })).toHaveCount(0);
  await expect(page.locator(".host-row").first()).toBeVisible();
  await expect(page.locator(".session-row")).toHaveCount(3);
  await expect(page.locator(".session-row").nth(0)).toHaveAttribute("data-session-id", above);
  await expect(page.locator(".session-row").nth(1)).toHaveAttribute("data-session-id", target);
  await expect(row(page, target)).toHaveAttribute("data-session-selected", "false");
  await expect(page.locator(".compact-toggle input")).not.toBeChecked();
  expect(await page.locator(".app-sidebar").evaluate((node) => node.scrollHeight <= node.clientHeight),
    "three fixture rows must fit without scrolling").toBe(true);
  return { above, target, build };
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
