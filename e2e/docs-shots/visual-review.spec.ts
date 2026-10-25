/**
 * Review the real app across palettes and useful window widths on the docs fleet.
 * These unannotated pictures belong to development review, never to a docs page
 * or publication. Later Steel changes extend this set when they add a surface.
 */
import { expect, test } from "@playwright/test";
import { openRowMenu } from "../tests/helpers/fleet";
import { openSettings } from "../tests/helpers/settings";
import { open, openLauncher, rowOf } from "./open";
import { shot } from "./shot";

for (const colorScheme of ["dark", "light"] as const) {
  for (const width of ["wide", "main-650"] as const) {
    /**
     * The main-pane measurement keeps the narrow case meaningful when a later
     * plan widens the sidebar for the host rail. Native controls and real menus
     * remain visible in a full-window capture; no review overlay hides defects.
     */
    test(`${colorScheme} ${width}`, async ({ page, request }) => {
      await page.emulateMedia({ colorScheme, reducedMotion: "reduce" });
      await page.setViewportSize({ width: 1600, height: 1000 });
      const { scenario } = await open(page, request);
      const main = page.locator(".app-main");
      await expect(main).toBeVisible();
      if (width === "main-650") {
        const box = await main.boundingBox();
        expect(box, "the open main pane establishes the sidebar's occupied width").not.toBeNull();
        await page.setViewportSize({ width: Math.round(box!.x + 650), height: 1000 });
        await expect.poll(async () => (await main.boundingBox())?.width, {
          message: "the constrained capture has a 650px main pane",
        }).toBeCloseTo(650, 0);
      }
      await page.evaluate(() => document.fonts.ready.then(() => undefined));
      await expect(page.locator(".session-details")).toBeVisible();
      const name = `visual-review/${colorScheme}-${width}`;
      const window = [page.locator("body")];
      await shot(page, `${name}-main`, window, { pad: 0 });

      const row = rowOf(page, scenario, (session) => session.status === "idle" && session.seen === false);
      await openRowMenu(row);
      const menu = page.locator(".session-row-menu-panel");
      await expect(menu).toBeVisible();
      await shot(page, `${name}-session-menu`, window, { pad: 0 });
      await page.keyboard.press("Escape");
      await expect(menu).toBeHidden();

      const settings = await openSettings(page);
      await shot(page, `${name}-settings`, window, { pad: 0 });
      await page.keyboard.press("Escape");
      await expect(settings).toBeHidden();

      const launcher = await openLauncher(page);
      await launcher.locator(".launch-composer-harness-choice button", { hasText: /^Codex$/ }).click();
      await expect(launcher.locator(".launch-composer-model-choice")).toBeVisible();
      await shot(page, `${name}-launcher`, window, { pad: 0 });
    });
  }
}
