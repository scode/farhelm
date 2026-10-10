import { type APIRequestContext, type Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { cleanupSession, createSession } from "./helpers/fleet";
import { chooseLauncherHost, launcherHostOptions } from "./helpers/host-picker";
import { installTerminalSuiteHooks } from "./helpers/terminal-suite";

// Host invalidations can start route.fetch during the final assertion. The
// shared teardown drains those handlers before browser disposal, and resets
// the real fixture fleet between engines.
installTerminalSuiteHooks();

/** Keep the real connected fleet and change only its displayed identity.
 * Requests still target the actual hosts; the test owns no extra supervisor. */
async function identityFleet(page: Page, request: APIRequestContext) {
  const response = await request.get("/api/hosts");
  expect(response.ok()).toBe(true);
  const listing = await response.json();
  const local = listing.hosts.find((host: any) => host.kind === "local");
  const remote = listing.hosts.find((host: any) => host.kind === "ssh" && host.state.phase === "connected");
  expect(local?.kind, "the fleet must contain its reserved local host").toBe("local");
  expect(remote?.state.phase, "the fixture must have a connected remote target").toBe("connected");
  await page.route("**/api/hosts", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    body.hosts = body.hosts.map((host: any) => host.id === remote.id
      ? { ...host, name: "Build Box", icon: "rocket", color: "teal" }
      : host);
    await route.fulfill({ response, json: body });
  });
  return { local, remote };
}

/** Browsing is provisional: every navigation key leaves the submitted target
 * alone until Enter or Space commits. Escape and blur preserve that target. */
test("host picker supports mouse, navigation, typeahead, cancellation, and focus", async ({ page, request }) => {
  const { local, remote } = await identityFleet(page, request);
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  const host = form.getByRole("combobox", { name: "host", exact: true });
  await expect(host).toHaveAttribute("data-host-id", String(local.id));
  const options = await launcherHostOptions(host);
  await expect(options).toHaveCount(2);
  await expect(options.nth(0)).toHaveAttribute("data-host-id", String(local.id));
  await expect(options.nth(1)).toHaveAttribute("data-host-id", String(remote.id));
  await expect(options.nth(0).locator(".host-kind-icon")).toHaveAttribute("data-glyph", "local");
  await expect(options.nth(0).locator(".host-kind-icon")).toHaveCSS("color", "rgb(224, 128, 128)");
  await expect(options.nth(1).locator(".host-kind-icon")).toHaveAttribute("data-glyph", "rocket");
  await expect(options.nth(1).locator(".host-kind-icon")).toHaveCSS("color", "rgb(79, 200, 192)");
  await test.info().attach("launcher-host-picker", { body: await form.screenshot(), contentType: "image/png" });
  await chooseLauncherHost(host, String(remote.id));
  await expect(host.locator(".host-kind-icon")).toHaveAttribute("data-glyph", "rocket");
  await host.focus();
  await expect(host).toBeFocused();
  await host.press("z");
  await expect(options.nth(1), "an unmatched opening keeps the committed host active").toHaveAttribute("aria-selected", "true");
  await host.press("Enter");
  await expect(host).toHaveAttribute("data-host-id", String(remote.id));
  await host.press("Space");
  await host.press("Home");
  await expect(options.nth(0)).toHaveAttribute("aria-selected", "true");
  await expect(host).toHaveAttribute("data-host-id", String(remote.id));
  await host.press("Space");
  await expect(host).toHaveAttribute("data-host-id", String(local.id));
  await host.press("ArrowDown");
  await host.press("ArrowDown");
  await expect(options.nth(1)).toHaveAttribute("aria-selected", "true");
  await host.press("ArrowUp");
  await expect(options.nth(0)).toHaveAttribute("aria-selected", "true");
  await host.press("End");
  await expect(options.nth(1)).toHaveAttribute("aria-selected", "true");
  await host.press("Escape");
  await expect(host).toHaveAttribute("aria-expanded", "false");
  await expect(host).toHaveAttribute("data-host-id", String(local.id));
  await expect(form).toBeVisible();
  await host.press("b");
  await host.press("u");
  await host.press("i");
  await expect(options.nth(1)).toHaveAttribute("aria-selected", "true");
  await expect(host).toHaveAttribute("data-host-id", String(local.id));
  await host.press("Enter");
  await expect(host).toHaveAttribute("data-host-id", String(remote.id));
  await expect(host).toHaveAttribute("aria-expanded", "false");
  await expect(host).toBeFocused();
  await expect(form, "open-menu Enter chooses rather than launching").toBeVisible();
  await host.press("Space");
  await host.press("Tab");
  await expect(host).toHaveAttribute("aria-expanded", "false");
  await expect(form.getByRole("button", { name: "browse this path", exact: true })).toBeFocused();
  await form.getByRole("button", { name: "cancel", exact: true }).click();
  await expect(page.locator("#launcher-host-options")).toHaveCount(0);
});

/** Safari and the macOS desktop app do not focus a button that is clicked,
 * and this menu closes on blur and takes its keys on the button. A synthetic
 * click never moves focus in any engine, so it stands in for that platform:
 * the menu must take focus itself, answer the keyboard, and close on a click
 * elsewhere in the launcher. */
test("host picker opened without click focus still owns the keyboard and closes outside", async ({ page, request }) => {
  const { local, remote } = await identityFleet(page, request);
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  const host = form.getByRole("combobox", { name: "host", exact: true });
  await expect(host).toHaveAttribute("data-host-id", String(local.id));
  await expect(host).toBeEnabled();
  await expect(host, "fixture: the picker starts unfocused").not.toBeFocused();
  await host.evaluate((button: HTMLButtonElement) => button.click());
  await expect(host).toHaveAttribute("aria-expanded", "true");
  await expect(host, "opening takes focus even when the click gave none").toBeFocused();
  const option = (id: string) => form.locator(`.launcher-host-option[data-host-id="${id}"]`);
  await expect(option(String(local.id))).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("ArrowDown");
  await expect(option(String(remote.id)), "arrows reach the open menu").toHaveAttribute("aria-selected", "true");
  // The open menu overlays the fields below the picker, so the outside
  // click goes to the form's own top-left padding, checked to be clear.
  const box = (await form.boundingBox())!;
  const outside = { x: box.x + 4, y: box.y + 4 };
  expect(await page.evaluate(({ x, y }) => {
    const hit = document.elementFromPoint(x, y);
    return !!hit && !hit.closest(".launcher-host-picker");
  }, outside), "fixture: the outside point is clear of the picker and its menu").toBe(true);
  await page.mouse.click(outside.x, outside.y);
  await expect(host, "a click elsewhere closes the menu").toHaveAttribute("aria-expanded", "false");
  await expect(host).toHaveAttribute("data-host-id", String(local.id));
  await form.getByRole("button", { name: "cancel", exact: true }).click();
});

/** New seeds its destination from the selected session before the picker has
 * ever opened. Unmatched typeahead must preserve that remote default; Enter
 * may choose the shown host, but must not silently move the launch to local. */
test("host picker unmatched typeahead preserves a remote launch default", async ({ page, request }) => {
  const { remote } = await identityFleet(page, request);
  const session = await createSession(request, { host: remote.id, title: `host-picker-default-${Date.now()}` });
  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${session.id}"]`);
    await expect(row).toBeVisible();
    await row.locator(".session-row-open").click();
    await expect(row).toHaveAttribute("data-session-selected", "true");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    const host = form.locator(".create-session-host");
    await expect(host).toHaveAttribute("data-host-id", String(remote.id));
    await host.focus();
    await expect(host).toBeFocused();
    await host.press("z");
    await expect(form.locator(`.launcher-host-option[data-host-id="${remote.id}"]`)).toHaveAttribute("aria-selected", "true");
    await host.press("Enter");
    await expect(host).toHaveAttribute("data-host-id", String(remote.id));
    await expect(host).toHaveAttribute("aria-expanded", "false");
    await expect(form).toBeVisible();
  } finally { await cleanupSession(request, session.id); }
});

/** The launcher can open before the registry arrives. An empty committed
 * value must stay empty and browsing cannot fabricate a hostless selection. */
test("host picker retains the empty value until hosts arrive", async ({ page }) => {
  let release!: () => void;
  const held = new Promise<void>((resolve) => (release = resolve));
  await page.route("**/api/hosts", async (route) => { await held; await route.continue(); });
  try {
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const host = page.locator(".create-session-host");
    await expect(host).toHaveAttribute("data-host-id", "");
    await expect(host.locator("bdi")).toHaveCount(0);
    await host.focus();
    await expect(host).toBeFocused();
    await host.press("ArrowDown");
    await expect(page.locator("#launcher-host-options")).toContainText("loading hosts");
    await host.press("Enter");
    await expect(host).toHaveAttribute("data-host-id", "");
    release();
    await expect(host).toHaveAttribute("data-host-id", /\d+/);
    await expect(host.locator("bdi")).not.toBeEmpty();
  } finally { release(); }
});

/** Closed Enter keeps the existing launch behavior; the whole create round
 * trip disables the target and cannot resurrect a menu after its refusal. */
test("host picker closed Enter launches and remains disabled during the write", async ({ page, request }) => {
  const { local } = await identityFleet(page, request);
  let release!: () => void;
  const held = new Promise<void>((resolve) => (release = resolve));
  const posts: any[] = [];
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    posts.push(route.request().postDataJSON());
    await held;
    await route.fulfill({ status: 500, body: "host-picker fixture refuses launch" });
  });
  try {
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    const host = form.locator(".create-session-host");
    await expect(host).toHaveAttribute("data-host-id", String(local.id));
    await form.getByRole("button", { name: "Codex", exact: true }).click();
    await host.focus();
    await expect(host).toBeFocused();
    await host.press("Enter");
    await expect.poll(() => posts.length).toBe(1);
    expect(posts[0].host).toBe(local.id);
    await expect(host).toBeDisabled();
    await expect(page.locator("#launcher-host-options")).toHaveCount(0);
    release();
    await expect(form.locator(".create-session-error")).toContainText("host-picker fixture refuses launch");
    await expect(host).toBeEnabled();
    await expect(host).toHaveAttribute("aria-expanded", "false");
  } finally { release(); }
});

/** The switcher uses the same host snapshot as the sidebar, even though its
 * session search spans the fleet. This proves the chosen mark at that seam. */
test("quick switcher draws a session's chosen host mark", async ({ page, request }) => {
  const { remote } = await identityFleet(page, request);
  const session = await createSession(request, { host: remote.id, title: `host-picker-switcher-${Date.now()}` });
  try {
    await page.goto("/");
    await expect(page.locator(`[data-session-id="${session.id}"] .host-kind-icon`)).toHaveAttribute("data-glyph", "rocket");
    await page.keyboard.press("Control+Shift+K");
    const dialog = page.locator(".quick-switcher-dialog");
    await expect(dialog).toBeVisible();
    const row = dialog.getByRole("option", { name: new RegExp(session.title) });
    await expect(row.locator(".quick-switcher-host .host-kind-icon")).toHaveAttribute("data-glyph", "rocket");
    await expect(row.locator(".quick-switcher-host .host-kind-icon")).toHaveCSS("color", "rgb(79, 200, 192)");
    await test.info().attach("quick-switcher-host-mark", { body: await dialog.screenshot(), contentType: "image/png" });
  } finally { await cleanupSession(request, session.id); }
});
