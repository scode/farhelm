/** Helm-wide confirmation choices must change the next action in this client.
 * These tests keep the real preference route and host registry, so a checkbox
 * that changes only its own appearance cannot satisfy the round trip.
 */
import { expect, test } from "./helpers/evidence";
import {
  forceBuildSkew,
  hostRowByName,
  openHostMenu,
  patchPreferences,
  readPreferences,
} from "./helpers/fleet";

test("settings reverses a permanent removal answer without reloading", async ({ page, request }, testInfo) => {
  const previous = await readPreferences(request);
  const owned: number[] = [];
  const names: string[] = [];
  try {
    await patchPreferences(request, { skip_host_remove_confirmation: null });
    for (let n = 0; n < 3; n++) {
      const name = `settings-${testInfo.project.name}-${n}.invalid`;
      const reply = await request.post("/api/hosts", { data: { ssh: name } });
      expect(reply.ok(), await reply.text()).toBe(true);
      const host = await reply.json();
      owned.push(host.id);
      names.push(name);
    }
    await page.goto("/");
    const first = hostRowByName(page, names[0]);
    await expect(first).toBeVisible();
    await openHostMenu(first);
    await first.locator(".host-remove").click();
    const removal = page.locator(".host-remove-dialog");
    await expect(removal).toBeVisible();
    await expect(removal.getByText("You can turn this back on with the gear at the top of the sidebar.")).toBeVisible();
    await removal.getByRole("button", { name: "remove, and don't ask again", exact: true }).click();
    await expect(first).toHaveCount(0);
    await expect.poll(async () => (await readPreferences(request)).skip_host_remove_confirmation).toBe(true);

    // The permanent answer itself takes effect before settings changes it.
    const second = hostRowByName(page, names[1]);
    await openHostMenu(second);
    await second.locator(".host-remove").click();
    await expect(second).toHaveCount(0);
    await expect(removal).toHaveCount(0);

    const gear = page.getByRole("button", { name: "settings", exact: true });
    await gear.click();
    const settings = page.getByRole("dialog", { name: "settings", exact: true });
    const toggle = settings.getByRole("checkbox", { name: "remove hosts without asking", exact: true });
    await expect(toggle).toBeChecked();
    await toggle.uncheck();
    await expect(toggle).not.toBeChecked();
    await expect(settings.locator("#host-remove-preference-help")).toContainText("asks before forgetting");
    await expect.poll(async () => (await readPreferences(request)).skip_host_remove_confirmation).toBe(false);
    await settings.getByRole("button", { name: "close", exact: true }).click();
    await expect(gear).toBeFocused();

    const third = hostRowByName(page, names[2]);
    await openHostMenu(third);
    await third.locator(".host-remove").click();
    await expect(removal).toBeVisible();
    await expect(third).toBeVisible();
    await removal.getByRole("button", { name: "cancel", exact: true }).click();
    // Ticking the settings checkbox is also a permanent answer, not merely
    // a way to clear one. The same still-registered host now removes at once.
    await gear.click();
    await toggle.check();
    await expect.poll(async () => (await readPreferences(request)).skip_host_remove_confirmation).toBe(true);
    await settings.getByRole("button", { name: "close", exact: true }).click();
    await openHostMenu(third);
    await third.locator(".host-remove").click();
    await expect(third).toHaveCount(0);
    await expect(removal).toHaveCount(0);
  } finally {
    for (const id of owned) await request.delete(`/api/hosts/${id}`);
    await patchPreferences(request, { skip_host_remove_confirmation: previous.skip_host_remove_confirmation ?? null });
  }
});

/** The settings modal renders outside the sticky bar, makes background
 * controls inert and returns keyboard focus to its opener when dismissed.
 */
test("settings dismisses a row menu, isolates the page and returns focus on Escape", async ({ page, request }) => {
  const previous = await readPreferences(request);
  try {
    await patchPreferences(request, { skip_host_setup_confirmation: null, skip_host_remove_confirmation: false });
    await page.goto("/");
    const row = page.locator(".host-row").first();
    await expect(row).toBeVisible();
    await openHostMenu(row);
    await expect(row.locator('[role="menu"]')).toBeVisible();
    const gear = page.getByRole("button", { name: "settings", exact: true });
    await gear.click();
    const dialog = page.getByRole("dialog", { name: "settings", exact: true });
    await expect(dialog).toBeVisible();
    await expect(page.locator('[role="menu"]')).toHaveCount(0);
    await expect(dialog.getByRole("checkbox")).toHaveCount(2);
    const setup = dialog.getByRole("checkbox", { name: "set up new hosts without asking", exact: true });
    await expect(setup).toBeFocused();
    await expect(setup).not.toBeChecked();
    await expect(dialog.getByRole("checkbox", { name: "remove hosts without asking", exact: true })).not.toBeChecked();
    expect(await dialog.evaluate((el) => el.closest(".app-bar"))).toBeNull();
    expect(await gear.evaluate((el) => Boolean(el.closest("[inert]")))).toBe(true);
    await setup.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(gear).toBeFocused();
    expect(await gear.evaluate((el) => Boolean(el.closest("[inert]")))).toBe(false);
  } finally {
    await patchPreferences(request, {
      skip_host_setup_confirmation: previous.skip_host_setup_confirmation ?? null,
      skip_host_remove_confirmation: previous.skip_host_remove_confirmation ?? null,
    });
  }
});

/** A long reported version yields to the gear in both normal and macOS
 * header layouts. This exercises real hit testing, not AppKit dragging.
 */
test("settings gear stays clickable beside a long version in narrow headers", async ({ page }, testInfo) => {
  await forceBuildSkew(page, "9.9.9-" + "long-version-".repeat(4));
  await page.goto("/");
  const version = page.locator(".app-version");
  await expect(version).toContainText("long-version");
  const gear = page.getByRole("button", { name: "settings", exact: true });
  for (const macos of [false, true]) {
    await page.locator(".app-shell").evaluate((el, active) => el.classList.toggle("macos-window", active), macos);
    await page.locator(".window-root").evaluate((el, active) => el.classList.toggle("macos-root", active), macos);
    for (const width of [900, 400]) {
      await page.setViewportSize({ width, height: 600 });
      const bar = (await page.locator(".app-bar").boundingBox())!;
      const stamp = (await version.boundingBox())!;
      const button = (await gear.boundingBox())!;
      expect(button.width).toBeGreaterThan(0);
      expect(button.x).toBeGreaterThanOrEqual(stamp.x + stamp.width);
      expect(button.x + button.width).toBeLessThanOrEqual(bar.x + bar.width);
      // One 22px button row plus padding/border fits below 40px; macOS
      // reserves its existing 48px band for the native titlebar controls.
      expect(bar.height).toBeLessThanOrEqual(macos ? 48 : 40);
      expect(button.y).toBeGreaterThanOrEqual(bar.y);
      expect(button.y + button.height).toBeLessThanOrEqual(bar.y + bar.height);
      expect(await gear.evaluate((el) => Boolean(el.closest(".window-drag-region")))).toBe(false);
      await gear.click();
      const dialog = page.getByRole("dialog", { name: "settings", exact: true });
      await expect(dialog).toBeVisible();
      if (macos && width === 400) {
        // Retain the narrow modal for visual inspection alongside geometry evidence.
        await page.screenshot({ path: testInfo.outputPath("settings-narrow.png") });
      }
      await dialog.getByRole("button", { name: "close", exact: true }).click();
      await expect(gear).toBeFocused();
    }
  }
});
