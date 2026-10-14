/**
 * Command help must show the helm's exact two commands and copy their raw
 * bytes. The nested Add host entry must preserve its draft and modal keyboard
 * ownership; seeing a command must never itself start SSH or add that host.
 */
import { randomUUID } from "node:crypto";
import { Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { resetPreferences } from "./helpers/fleet";

test.beforeEach(async ({ request }) => {
  await resetPreferences(request);
});

/** Open through the ordinary help menu and await the dialog's focus handoff. */
async function openHelp(page: Page) {
  await page.getByRole("button", { name: "help", exact: true }).click();
  const menu = page.getByRole("menu", { name: "help", exact: true });
  await expect(menu).toBeVisible();
  await menu.locator('[data-bar-menu-item="ssh needs 2FA?"]').click();
  const dialog = page.getByRole("dialog", { name: "ssh needs 2FA?", exact: true });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("button", { name: "close", exact: true })).toBeFocused();
  return dialog;
}

/** A real registered destination needs both purposes even while unreachable.
 * Clipboard inspection uses the native bridge seam; no test writes the host's
 * actual clipboard or treats a successful browser click as a copy receipt.
 */
test("ssh help displays and copies both registered-host commands", async ({ page, request }) => {
  const destination = `help-${randomUUID()}.invalid`;
  const added = await request.post("/api/hosts", { data: { ssh: destination } });
  expect(added.ok()).toBe(true);
  const { id } = await added.json();
  try {
    const response = await request.post("/api/ssh-connections", { data: {} });
    expect(response.ok()).toBe(true);
    const help = await response.json();
    const host = help.hosts.find((value: { destination: string }) => value.destination === destination);
    expect(host).toBeDefined();
    expect(host.sessions).toBeTruthy();
    expect(host.setup).toBeTruthy();
    await page.goto("/");
    await page.evaluate(() => {
      (window as any).__sshHelpCopies = [];
      (window as any).__farhelmNativeClipboardWrite = (value: string) => {
        (window as any).__sshHelpCopies.push(value);
        return Promise.resolve();
      };
    });
    const dialog = await openHelp(page);
    const section = dialog.locator(".ssh-help-host").filter({ hasText: destination });
    await expect(section).toHaveCount(1);
    await expect(section.locator("pre")).toHaveText([host.sessions, host.setup]);
    await section.getByRole("button", { name: "copy sessions connection", exact: true }).click();
    await section.getByRole("button", { name: "copy setup and updates connection", exact: true }).click();
    await expect.poll(() => page.evaluate(() => (window as any).__sshHelpCopies)).toEqual([host.sessions, host.setup]);
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(page.getByRole("button", { name: "help", exact: true })).toBeFocused();
  } finally {
    const removed = await request.delete(`/api/hosts/${id}`);
    expect(removed.ok()).toBe(true);
  }
});

/** Empty help remains useful rather than mistaking the local row for an SSH
 * host. A controlled response isolates this UI case from the shared fleet.
 */
test("ssh help names the empty state and remains dismissible during a failed read", async ({ page }) => {
  await page.route("**/api/ssh-connections", (route) => route.fulfill({
    json: { unavailable: [], hosts: [], extra: null },
  }));
  await page.goto("/");
  let dialog = await openHelp(page);
  await expect(dialog).toContainText("No ssh hosts are registered. Use add host");
  await dialog.getByRole("button", { name: "close", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole("button", { name: "help", exact: true })).toBeFocused();
  await page.route("**/api/ssh-connections", (route) => route.fulfill({ status: 500, body: "help unavailable" }));
  dialog = await openHelp(page);
  await expect(dialog.locator(".create-session-error")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
});

/** Viewing commands for a typed destination is not registration. Closing the
 * child must restore the still-mounted form, its bytes and its modal focus.
 */
test("add host opens ssh help for its draft without losing the form", async ({ page, request }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "add host", exact: true }).click();
  const form = page.getByRole("dialog", { name: "add host", exact: true });
  const input = form.locator(".add-host-ssh");
  const link = form.getByRole("button", { name: "Start its connections yourself first.", exact: true });
  await expect(input).toBeFocused();
  await link.click();
  let dialog = page.getByRole("dialog", { name: "ssh needs 2FA?", exact: true });
  await expect(form).toBeHidden();
  await expect(dialog).toContainText("Enter an ssh destination in Add host first.");
  await expect(dialog.getByRole("button", { name: "close", exact: true })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(form).toBeVisible();
  await expect(link).toBeFocused();
  await expect.poll(() => page.locator('.app-help-toggle').evaluate((element) => element.closest('[inert]') !== null)).toBe(true);
  const destination = `draft-${randomUUID()}.invalid`;
  await input.fill(destination);
  await link.click();
  await expect(form).toBeHidden();
  dialog = page.getByRole("dialog", { name: "ssh needs 2FA?", exact: true });
  await expect(dialog.locator(".ssh-help-host").first()).toContainText("the host you're adding");
  await expect(dialog.locator(".ssh-help-host").first().locator("pre")).toHaveCount(2);
  await expect(dialog.locator(".ssh-help-host").first()).toContainText(destination);
  await dialog.getByRole("button", { name: "close", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(input).toHaveValue(destination);
  await expect(link).toBeFocused();
  await expect(form).toBeVisible();
  await expect.poll(() => page.locator('.app-help-toggle').evaluate((element) => element.closest('[inert]') !== null)).toBe(true);
  await page.keyboard.press('Tab');
  await expect(form.getByRole('button', { name: 'cancel', exact: true })).toBeFocused();
  const hosts = await request.get("/api/hosts");
  expect(hosts.ok()).toBe(true);
  expect((await hosts.json()).hosts.some((host: { destination?: string }) => host.destination === destination)).toBe(false);
  await page.keyboard.press("Escape");
  await expect(form).toHaveCount(0);
});
