/**
 * A menu has one current command, shared by pointer and keyboard navigation.
 * Computed fills catch the original double highlight and its reverse: an
 * arrow key must move the fill away from an item still under the pointer.
 * The real owned session supplies lifecycle offers; a healthy quiet feed
 * keeps unrelated sampler updates from replacing a menu during input.
 */
import { type Locator, type Page } from "@playwright/test";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { resolveToken } from "./helpers/color";
import { expect, test } from "./helpers/evidence";
import {
  cleanupSession,
  createResumableSession,
  listHosts,
  openHostMenu,
  openRowMenu,
  pinAutoSelect,
  stubFeed,
} from "./helpers/fleet";
import { openSessionActionMenu } from "./helpers/session-actions";
import { waitForSessionReady } from "./helpers/terminal-readiness";

let sessionId: string | undefined;

test.beforeEach(async ({ page, request }) => {
  const session = await createResumableSession(request, { title: `menu-highlight-${Date.now()}` });
  sessionId = session.id;
  await pinAutoSelect(page, session.id);
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  await page.goto("/");
  await feed.waitForConnection(1);
  await expect(page.locator(`[data-session-id="${session.id}"]`)).toBeVisible({ timeout: 20_000 });
  await waitForSessionReady(page, session.id);
});

test.afterEach(async ({ page, request }, testInfo) => {
  const owned = sessionId;
  sessionId = undefined;
  await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
    ["routes", () => page.unrouteAll({ behavior: "ignoreErrors" })],
    ...(owned ? [["session", () => cleanupSession(request, owned)] as [string, () => Promise<void>]] : []),
  ]);
});

/** All command roles share this identity, including sort radios and checkboxes. */
function items(menu: Locator): Locator {
  return menu.locator("[data-menu-focus]");
}

/**
 * Compare every item's actual background with the palette's accent role.
 * Checking the other backgrounds too catches the generic button hover fill
 * showing through after focus has moved to a different command.
 */
async function expectFill(page: Page, menu: Locator, current: Locator): Promise<void> {
  const key = await current.getAttribute("data-menu-focus");
  expect(key, "the current command must belong to the shared focus model").toBeTruthy();
  const disabled = await current.getAttribute("aria-disabled") === "true";
  const accent = await resolveToken(page, "--accent-fill");
  await expect.poll(async () =>
    items(menu).evaluateAll((nodes, { key, disabled, accent }) =>
      nodes.map((node) => ({
        key: node.getAttribute("data-menu-focus"),
        background: getComputedStyle(node).backgroundColor,
        expected: !disabled && node.getAttribute("data-menu-focus") === key ? accent : "rgba(0, 0, 0, 0)",
      })).filter((item) => item.background !== item.expected), { key, disabled, accent }), {
    message: "only the focused enabled command has the accent fill; other commands stay transparent",
  }).toEqual([]);
}

/** Park outside all menu and tooltip targets without clicking away focus. */
async function parkPointer(page: Page): Promise<void> {
  const point = { x: 1, y: page.viewportSize()!.height - 1 };
  expect(
    await page.evaluate(
      ({ x, y }) =>
        document.elementFromPoint(x, y)?.closest('[role="menu"], [data-tooltip], [data-tooltip-card]') !== null,
      point,
    ),
    "the parking point must be outside menus and hover help",
  ).toBe(false);
  await page.mouse.move(point.x, point.y);
}

/**
 * Exercise both input directions against one menu's rendered command order.
 * The pointer stays on the hovered command through the arrow sequence, so
 * two independent CSS fills cannot pass as one shared current command.
 */
async function exerciseHover(page: Page, menu: Locator, hovered: Locator): Promise<void> {
  const commands = items(menu);
  const count = await commands.count();
  expect(count, "the fixture needs a command distinct from the opening command").toBeGreaterThan(1);
  await expect(commands.first()).toBeFocused();
  await expectFill(page, menu, commands.first());
  const key = await hovered.getAttribute("data-menu-focus");
  const index = await commands.evaluateAll(
    (nodes, key) => nodes.findIndex((node) => node.getAttribute("data-menu-focus") === key),
    key,
  );
  expect(index, "hover must leave the initially focused first command").toBeGreaterThan(0);

  await hovered.hover();
  await expect.poll(() => hovered.evaluate((node) => node.matches(":hover"))).toBe(true);
  // This is the focused red proof on main: first-item focus and second-item
  // hover otherwise paint two accent backgrounds in the real browser.
  await expectFill(page, menu, hovered);
  await expect(hovered).toBeFocused();
  await expect(hovered).toHaveAttribute("tabindex", "0");
  await expect(menu.locator('[data-menu-focus][tabindex="0"]')).toHaveCount(1);

  for (
    const [key, position] of [
      ["ArrowDown", (index + 1) % count],
      ["ArrowUp", index],
      ["ArrowUp", (index + count - 1) % count],
    ] as const
  ) {
    await page.keyboard.press(key);
    await expect(commands.nth(position)).toBeFocused();
    expect(await hovered.evaluate((node) => node.matches(":hover")), "arrow input must leave the pointer in place")
      .toBe(true);
    await expectFill(page, menu, commands.nth(position));
  }

  await parkPointer(page);
  await hovered.hover();
  await expect(hovered).toBeFocused();
  await parkPointer(page);
  expect(await hovered.evaluate((node) => node.matches(":hover"))).toBe(false);
  await expect(hovered, "leaving an open menu retains its last hovered command").toBeFocused();
  await expectFill(page, menu, hovered);
}

/** Hovering Clone must make Enter open the editable launcher, never Rename.
 * Opening a launcher is a safe observable activation: no launch is submitted.
 */
test("session menu shares one highlight between hover and arrow keys", async ({ page }) => {
  const row = page.locator(`[data-session-id="${sessionId}"]`);
  await openRowMenu(row);
  const menu = row.getByRole("menu");
  const clone = menu.getByRole("menuitem", { name: "Clone", exact: true });
  await expect(clone).not.toHaveAttribute("aria-disabled", "true");
  await exerciseHover(page, menu, clone);
  await clone.hover();
  await expect(clone).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(menu).toHaveCount(0);
  const launcher = page.locator(".create-session-form");
  await expect(launcher).toBeVisible();
  await launcher.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(launcher).toHaveCount(0);
});

/**
 * Disabled host actions remain readable and focusable, but cannot be filled
 * or activated. The local host supplies a disabled Update without SSH or any
 * attempted update to the machine running the tests.
 */
test("host menu hover transfers focus and disabled actions never fill or run", async ({ page, request }) => {
  const local = (await listHosts(request)).find((host) => host.kind === "local");
  expect(local, "the fixture requires the real local host").toBeTruthy();
  const row = page.locator(`.host-row[data-host-id="${local!.id}"]`);
  await openHostMenu(row);
  const menu = row.getByRole("menu");
  const settings = menu.locator(".host-settings");
  const update = menu.locator(".provisioning-update");
  await expect(settings).not.toHaveAttribute("aria-disabled", "true");
  await expect(update).toHaveAttribute("aria-disabled", "true");
  await exerciseHover(page, menu, settings);

  const mutations: string[] = [];
  page.on("request", (request) => {
    if (request.method() !== "GET" && new URL(request.url()).pathname.startsWith("/api/hosts/")) {
      mutations.push(`${request.method()} ${new URL(request.url()).pathname}`);
    }
  });
  await update.hover();
  await expect(update).toBeFocused();
  await expectFill(page, menu, update);
  await page.keyboard.press("Enter");
  await expect(menu).toBeVisible();
  await expect(update).toBeFocused();
  await expect(update).toHaveAttribute("aria-disabled", "true");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(mutations, "disabled Update must not send a host mutation").toEqual([]);
});

/** Both split menus use the same current-item contract and safe With action.
 * Their two entries also exercise wrapping while the pointer stays on With.
 */
test("restart and replace menus keep one highlight and Enter uses the hovered choice", async ({ page }) => {
  for (const action of ["Restart", "Replace"] as const) {
    const menu = await openSessionActionMenu(page, action);
    const withItem = menu.getByRole("menuitem", { name: `${action} with…`, exact: true });
    await expect(withItem).toHaveAttribute("aria-disabled", "false");
    await exerciseHover(page, menu, withItem);
    await withItem.hover();
    await expect(withItem).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(menu).toHaveCount(0);
    const launcher = page.locator(`.create-session-form[data-launcher-mode="${action.toLowerCase()}"]`);
    await expect(launcher).toBeVisible();
    await launcher.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(launcher).toHaveCount(0);
  }
});

/** Sort's non-menuitem roles and the Help bar share the delegated mechanics.
 * These cases catch a listener accidentally limited to role=menuitem or to
 * session-row markup; no shared preference or external link is activated.
 */
test("sort and help menus follow the same hover and keyboard highlight", async ({ page }) => {
  await page.locator(".sort-title").click();
  const sort = page.getByRole("menu", { name: "Sort", exact: true });
  await expect(sort).toBeVisible();
  await exerciseHover(page, sort, sort.getByRole("menuitemradio", { name: /^Newest created(?:$|\s)/ }));
  const compact = sort.getByRole("menuitemcheckbox", { name: /^Compact(?:$|\s)/ });
  await compact.hover();
  await expect(compact).toBeFocused();
  await expectFill(page, sort, compact);
  await page.keyboard.press("Escape");
  await expect(sort).toHaveCount(0);

  await page.locator(".app-help-toggle").click();
  const help = page.getByRole("menu", { name: "Help", exact: true });
  await expect(help).toBeVisible();
  await exerciseHover(page, help, help.locator('[data-bar-menu-item="Documentation"]'));
});

/**
 * Keyboard use can make a later script-focused hovered command match
 * :focus-visible. That does not give its help keyboard ownership: motion
 * must restart the pointer-rest delay, and leaving must hide help while the
 * hovered command keeps focus. The paused page clock owns that exact rest
 * interval rather than racing browser input against elapsed wall time.
 */
test("menu hover help stays pointer owned after keyboard focus", async ({ page }) => {
  await page.clock.install();
  await parkPointer(page);
  await page.clock.pauseAt(new Date(Date.now() + 5_000));
  const toggle = page.locator(".restart-menu-toggle");
  await toggle.focus();
  await expect(toggle).toBeFocused();
  await page.keyboard.press("ArrowDown");
  const menu = page.getByRole("menu", { name: "Restart", exact: true });
  const plain = menu.getByRole("menuitem", { name: "Restart", exact: true });
  const withItem = menu.getByRole("menuitem", { name: "Restart with…", exact: true });
  await expect(plain).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(withItem).toBeFocused();
  await expect.poll(() => withItem.evaluate((node) => node.matches(":focus-visible"))).toBe(true);
  // No page time has passed since opening, so no keyboard tooltip has shown
  // and pointer hover cannot use the intentional warm follow-on exception.
  const tip = page.locator("body > .farhelm-tooltip");
  await expect(tip).toBeHidden();
  const box = (await plain.boundingBox())!;
  const point = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  await page.mouse.move(point.x, point.y);
  await expect(plain).toBeFocused();
  expect(
    await plain.evaluate((node) => node.matches(":focus-visible")),
    "premise: hover's script focus retains the browser's keyboard focus-visible state",
  ).toBe(true);
  const words = await plain.getAttribute("data-tooltip");
  expect(words, "the hovered command must carry help text").toBeTruthy();
  await page.clock.runFor(200);
  await expect(tip).toBeHidden();
  await page.mouse.move(point.x + 2, point.y);
  expect(await plain.evaluate((node) => node.matches(":hover")), "the rest-reset motion stays inside one item").toBe(
    true,
  );
  await page.clock.runFor(200);
  await expect(tip, "motion restarts the 300 ms pointer-rest delay despite focus-visible").toBeHidden();
  // Arrow navigation leaves the pointer on plain Restart. Moving within
  // that same item must restart help without relying on a new pointerover.
  await page.keyboard.press("ArrowDown");
  await expect(withItem).toBeFocused();
  expect(await plain.evaluate((node) => node.matches(":hover"))).toBe(true);
  await page.mouse.move(point.x + 4, point.y);
  await expect(plain).toBeFocused();
  await page.clock.runFor(299);
  await expect(tip).toBeHidden();
  await page.clock.runFor(1);
  await expect(tip).toBeVisible();
  await expect(tip).toHaveText(words!);
  await parkPointer(page);
  await expect(plain).toBeFocused();
  await expect(tip, "pointer departure hides help without clearing menu focus").toBeHidden();
});
