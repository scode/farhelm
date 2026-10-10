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

/** Settings switches must fit beside their labels and keep native keyboard
 * behavior. The modal also isolates the page and returns focus to its opener;
 * styling a replacement control must not bypass either lifecycle.
 */
test("settings dismisses a row menu, isolates the page and returns focus on Escape", async ({ page, request }, testInfo) => {
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
    await expect(dialog.getByRole("checkbox")).toHaveCount(5);
    const setup = dialog.getByRole("checkbox", { name: "set up new hosts without asking", exact: true });
    await expect(setup).toBeFocused();
    await expect(setup).not.toBeChecked();
    await expect(dialog.getByRole("checkbox", { name: "remove hosts without asking", exact: true })).not.toBeChecked();
    expect(await dialog.evaluate((el) => el.closest(".app-bar"))).toBeNull();
    expect(await gear.evaluate((el) => Boolean(el.closest("[inert]")))).toBe(true);
    await page.evaluate(() => document.fonts.ready);
    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: 800 });
      await dialog.screenshot({ path: testInfo.outputPath(`settings-switches-${width}.png`) });
      // Measure rendered text independently of the label's full-width box:
      // a checkbox above or before its caption must fail this contract.
      const rows = await dialog.locator(".app-settings-choice").evaluateAll((labels) => labels.map((label) => {
        const input = label.querySelector("input")!;
        const range = document.createRange();
        const text = Array.from(label.childNodes).find((node) => node !== input && node.textContent?.trim())!;
        range.selectNode(text);
        const caption = range.getBoundingClientRect();
        const control = input.getBoundingClientRect();
        const row = label.getBoundingClientRect();
        return {
          textRight: caption.right, textTop: caption.top, textBottom: caption.bottom,
          controlLeft: control.left, controlRight: control.right,
          controlTop: control.top, controlBottom: control.bottom,
          width: control.width, height: control.height, rowRight: row.right,
          appearance: getComputedStyle(input).appearance,
        };
      }));
      expect(rows).toHaveLength(5);
      for (const row of rows) {
        expect(row.textRight).toBeLessThan(row.controlLeft);
        expect(row.controlTop).toBeLessThan(row.textBottom);
        expect(row.controlBottom).toBeGreaterThan(row.textTop);
        expect(row.controlRight).toBeCloseTo(row.rowRight, 0);
        expect(row.width).toBe(34);
        expect(row.height).toBe(20);
        expect(row.appearance).toBe("none");
      }
      const alignment = await dialog.evaluate((el) => ({
        choices: Array.from(el.querySelectorAll(".app-settings-choice"), (row) => row.getBoundingClientRect().left),
        help: Array.from(el.querySelectorAll(".host-settings-help"), (help) => help.getBoundingClientRect().left),
        heading: el.querySelector("section[aria-label='sounds'] h3")!.getBoundingClientRect().left,
        overflowing: el.scrollWidth > el.clientWidth,
      }));
      expect(alignment.overflowing).toBe(false);
      for (const left of [...alignment.choices, ...alignment.help]) expect(left).toBeCloseTo(alignment.heading, 0);
    }
    // Space exercises the native input rather than Playwright's checked-state
    // setter, and the real helm reply proves the preference writer still runs.
    await setup.focus();
    await expect(setup).toBeFocused();
    await setup.press("Space");
    await expect(setup).toBeChecked();
    await expect.poll(async () => (await readPreferences(request)).skip_host_setup_confirmation).toBe(true);
    await setup.press("Space");
    await expect(setup).not.toBeChecked();
    await expect.poll(async () => (await readPreferences(request)).skip_host_setup_confirmation).toBe(false);
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
