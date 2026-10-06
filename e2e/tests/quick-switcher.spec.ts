/** Keyboard navigation must cross the modal boundary before terminal focus. */
import { expect, test } from "./helpers/evidence";
import { type Page } from "@playwright/test";
import {
  cleanupSession, createSession, listHosts, listSessions, localHostId,
  pinAutoSelect, resetPreferences, stubFeed, openRowMenu,
} from "./helpers/fleet";
import { attachSession, waitForTermText } from "./helpers/term";
import { waitForSessionReady } from "./helpers/terminal-readiness";
import { routeGate } from "./helpers/route-gate";

/** Override the platform before the shortcut and xterm both classify it. */
async function platform(page: Page, mac: boolean): Promise<void> {
  await page.addInitScript((value) => {
    Object.defineProperty(Navigator.prototype, "platform", { get: () => value });
  }, mac ? "MacIntel" : "Linux x86_64");
}

/** The authenticated list-owned trigger proves the shortcut can do work. */
async function openSwitcher(page: Page, mac = false) {
  await expect(page.locator(".quick-switcher-trigger")).toHaveCount(1);
  await page.keyboard.press(mac ? "Meta+k" : "Control+Shift+k");
  const dialog = page.getByRole("dialog", { name: "quick switcher" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("combobox")).toBeFocused();
  return dialog;
}

test.afterEach(async ({ request }) => { await resetPreferences(request); });

for (const mac of [false, true]) {
  /**
   * Real binary input frames distinguish taking the chord from merely
   * showing a dialog after xterm also sent it. A live echo after Escape and
   * after a different-session jump proves both focus handoffs.
   */
  test(`the ${mac ? "Mac" : "non-Mac"} chord owns terminal input and restores focus`, async ({ page, request }) => {
    await platform(page, mac);
    const first = await createSession(request, { title: `switch-origin-${Date.now()}` });
    const second = await createSession(request, { title: `switch-target-${Date.now()}` });
    const binary: Buffer[] = [];
    page.on("websocket", (socket) => {
      if (!new URL(socket.url()).pathname.endsWith("/term")) return;
      socket.on("framesent", ({ payload }) => { if (Buffer.isBuffer(payload)) binary.push(payload); });
    });
    try {
      await pinAutoSelect(page, first.id);
      await page.goto("/");
      await attachSession(page, first.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      const terminal = page.locator("#terminal .xterm-helper-textarea");
      await expect(terminal).toBeFocused();
      let dialog = await openSwitcher(page, mac);
      await dialog.getByRole("combobox").fill(first.title);
      await expect(dialog.getByRole("option")).toHaveCount(1);
      await page.keyboard.press("Escape");
      await expect(dialog).toHaveCount(0);
      await expect(terminal).toBeFocused();
      const escaped = `escape-restored-${Date.now()}`;
      await page.keyboard.type(escaped);
      await page.keyboard.press("Enter");
      await waitForTermText(page, escaped);
      expect(Buffer.concat(binary).toString()).toBe(`${escaped}\r`);
      if (!mac) {
        await page.keyboard.press("Control+k");
        await expect.poll(() => Buffer.concat(binary).toString()).toContain("\x0b");
        await expect(page.locator(".quick-switcher-dialog")).toHaveCount(0);
      }

      // Close remains a native keyboard button, even with another row active.
      dialog = await openSwitcher(page, mac);
      await dialog.getByRole("combobox").fill(second.title);
      await page.keyboard.press("Tab");
      await expect(dialog.getByRole("button", { name: "close", exact: true })).toBeFocused();
      await page.keyboard.press("Enter");
      await expect(dialog).toHaveCount(0);
      await expect(page.locator(`[data-session-id="${first.id}"]`)).toHaveAttribute("data-session-selected", "true");
      await expect(terminal).toBeFocused();

      // A select survives navigation; restoring it would veto terminal reveal.
      await page.locator(".filter-host").focus();
      await expect(page.locator(".filter-host")).toBeFocused();
      dialog = await openSwitcher(page, mac);
      await dialog.getByRole("combobox").fill(second.title);
      await expect(dialog.getByRole("option")).toHaveCount(1);
      await page.keyboard.press("Enter");
      await expect(dialog).toHaveCount(0);
      await waitForSessionReady(page, second.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      await expect(terminal).toBeFocused();
      const jumped = `jump-restored-${Date.now()}`;
      await page.keyboard.type(jumped);
      await page.keyboard.press("Enter");
      await waitForTermText(page, jumped);

      // Picking the current session remounts nothing; it still owes focus.
      dialog = await openSwitcher(page, mac);
      await dialog.getByRole("combobox").fill(second.title);
      await expect(dialog.getByRole("option")).toHaveCount(1);
      await page.keyboard.press("Enter");
      await expect(dialog).toHaveCount(0);
      await expect(terminal).toBeFocused();
      await page.locator(".new-session-button").click();
      await expect(page.locator('.create-session-form[aria-modal="true"]')).toBeVisible();
      await page.keyboard.press(mac ? "Meta+k" : "Control+Shift+k");
      await expect(page.locator(".quick-switcher-dialog")).toHaveCount(0);
      expect(await page.evaluate(() => (window as any).__farhelmQuickSwitcherFocus)).toBeUndefined();
    } finally {
      await cleanupSession(request, second.id);
      await cleanupSession(request, first.id);
    }
  });
}

/** A filtered sidebar cannot supply the switcher's fleet-wide snapshot. */
test("a jump to a hidden host resets the selector to ALL", async ({ page, request }) => {
  await platform(page, false);
  const local = await localHostId(request);
  const remote = (await listHosts(request)).find((host) => host.id !== local);
  expect(remote, "the owned browser fleet includes its second host").toBeDefined();
  const first = await createSession(request, { title: `switch-local-${Date.now()}`, host: local });
  const other = await createSession(request, { title: `switch-remote-${Date.now()}`, host: remote!.id });
  try {
    expect((await listSessions(request)).sessions.some((session) => session.id === other.id)).toBe(true);
    await pinAutoSelect(page, first.id);
    await page.goto("/");
    await attachSession(page, first.id);
    await page.locator(".filter-host").selectOption(String(local));
    await expect(page.locator(`[data-session-id="${other.id}"]`)).toHaveCount(0);
    const dialog = await openSwitcher(page);
    await dialog.getByRole("combobox").fill(other.title);
    await expect(dialog.getByRole("option")).toHaveCount(1);
    await page.keyboard.press("Enter");
    await expect(dialog).toHaveCount(0);
    await expect(page.locator(".filter-host")).toHaveValue("");
    await expect(page.locator(`[data-session-id="${other.id}"]`)).toHaveAttribute("data-session-selected", "true");
    await waitForSessionReady(page, other.id);
  } finally {
    await cleanupSession(request, other.id);
    await cleanupSession(request, first.id);
  }
});

/** A refused pick preserves the selected session, host filter and focus. */
test("a busy pick leaves navigation and the host filter untouched", async ({ page, request }) => {
  await platform(page, false);
  const local = await localHostId(request);
  const remote = (await listHosts(request)).find((host) => host.id !== local);
  expect(remote, "the owned fleet includes a second host").toBeDefined();
  const owned: string[] = [];
  const gate = routeGate();
  let requested = false;
  try {
    const first = await createSession(request, { title: `busy-origin-${Date.now()}`, host: local });
    owned.push(first.id);
    const other = await createSession(request, { title: `busy-hidden-${Date.now()}`, host: remote!.id });
    owned.push(other.id);
    await pinAutoSelect(page, first.id);
    await page.goto("/");
    await attachSession(page, first.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await page.locator(".filter-host").selectOption(String(local));
    const row = page.locator(`[data-session-id="${first.id}"]`);
    await page.route(`**/api/sessions/${first.id}/stop`, async (route) => {
      requested = true;
      await gate.wait();
      await route.fulfill({ status: 503, body: "fixture operation refused" });
    });
    await openRowMenu(row);
    await row.locator(".session-row-stop").click();
    await expect.poll(() => requested).toBe(true);
    // Stop holds the row navigation lock, independently of New's global lock.
    await expect(row.locator(".session-row-open")).toBeDisabled();
    const terminal = page.locator("#terminal .xterm-helper-textarea");
    await terminal.focus();
    await expect(terminal).toBeFocused();
    const dialog = await openSwitcher(page);
    await dialog.getByRole("combobox").fill(other.title);
    await expect(dialog.getByRole("option")).toHaveCount(1);
    await page.keyboard.press("Enter");
    await expect(dialog).toHaveCount(0);
    await expect(terminal).toBeFocused();
    await expect(page.locator(".filter-host")).toHaveValue(String(local));
    await expect(row).toHaveAttribute("data-session-selected", "true");
    gate.release();
    await expect(row.locator(".session-row-open")).toBeEnabled();
  } finally {
    gate.release();
    await page.unrouteAll({ behavior: "wait" });
    for (const id of owned.reverse()) await cleanupSession(request, id);
  }
});

/** Loading must not masquerade as no matches or accept Enter prematurely. */
test("loading has no selection and a failed listing says so", async ({ page }) => {
  await platform(page, false);
  const feed = await stubFeed(page);
  await page.goto("/");
  await feed.waitForConnection(1);
  await expect(page.locator(".quick-switcher-trigger")).toHaveCount(1);
  const gate = routeGate();
  let requested = false;
  await page.route((url) => url.pathname === "/api/sessions", async (route) => {
    requested = true;
    await gate.wait();
    const response = await route.fetch();
    await route.fulfill({ response, status: 503, body: "fixture listing unavailable" });
  });
  try {
    const dialog = await openSwitcher(page);
    await expect.poll(() => requested).toBe(true);
    await expect(dialog.getByRole("status")).toHaveText("loading sessions…");
    await expect(dialog.getByRole("option")).toHaveCount(0);
    await page.keyboard.press("Enter");
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole("status")).toHaveText("loading sessions…");
    await page.keyboard.press("Tab");
    await expect(dialog.getByRole("button", { name: "close", exact: true })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(dialog).toHaveCount(0);
    const reopened = await openSwitcher(page);
    await expect(reopened.getByRole("status")).toHaveText("loading sessions…");
    gate.release();
    await expect(reopened.getByRole("alert")).toContainText("Couldn't load sessions");
    await expect(reopened).not.toContainText("no sessions match");
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
  } finally { gate.release(); }
});

/** All matches stay reachable, and title rank beats newer metadata matches. */
test("ranking, highlighting and the listing-cap notice use one snapshot", async ({ page }) => {
  await platform(page, false);
  const feed = await stubFeed(page);
  await page.goto("/");
  await feed.waitForConnection(1);
  await expect(page.locator(".quick-switcher-trigger")).toHaveCount(1);
  await page.route((url) => url.pathname === "/api/sessions", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    const template = body.sessions[0];
    expect(template, "the shared fleet has a real row to preserve wire shape").toBeDefined();
    body.sessions = [
      { ...template, id: "metadata", title: "most recent", host_name: "api-host" },
      ...Array.from({ length: 40 }, (_, index) => ({
        ...template, id: `title-${index}`, title: `api-${index}`, host_name: "build",
        annotation: null,
        status: index === 0 ? { state: "exited", exit_code: 17 }
          : index === 1 ? { state: "error", detail: "cannot launch" } : template.status,
      })),
    ];
    body.truncated = true;
    await route.fulfill({ response, json: body });
  });
  const dialog = await openSwitcher(page);
  await dialog.getByRole("combobox").fill("api");
  const options = dialog.getByRole("option");
  await expect(options).toHaveCount(41);
  await expect(options.first()).toContainText("api-0");
  await expect(options.last()).toContainText("most recent");
  await expect(options.first().locator(".quick-switcher-title mark")).toHaveText("api");
  await expect(dialog.locator(".quick-switcher-cap")).toContainText("most recently active");
  // Word badges must show their status and exit code, not inherit the dot track.
  const ended = options.first().locator(".status-badge");
  await expect(ended).toHaveText("exited (code 17)");
  await expect(options.nth(1).locator(".status-badge")).toContainText("error");
  for (const width of [1280, 390]) {
    await page.setViewportSize({ width, height: 844 });
    await expect.poll(() => ended.evaluate((node) => node.clientWidth >= node.scrollWidth)).toBe(true);
    await test.info().attach(`switcher-ended-${width}`, { body: await dialog.screenshot(), contentType: "image/png" });
  }
  await page.setViewportSize({ width: 1280, height: 844 });
  for (let index = 0; index < 40; index++) await page.keyboard.press("ArrowDown");
  await expect(options.last()).toHaveAttribute("aria-selected", "true");
  await expect(options.last()).toBeInViewport();
  await test.info().attach("switcher-desktop", { body: await dialog.screenshot(), contentType: "image/png" });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(dialog).toBeInViewport();
  await test.info().attach("switcher-phone", { body: await dialog.screenshot(), contentType: "image/png" });
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
});
