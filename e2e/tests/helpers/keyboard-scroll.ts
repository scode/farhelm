import { expect } from "./evidence";
import { type Locator, type Page } from "@playwright/test";

/**
 * A text-only scroll region must be reachable by Tab and scroll through native
 * key input. Focusing the region directly or assigning scrollTop would conceal
 * WebKit's missing automatic tab stop, the regression this helper checks.
 * The preceding control establishes the focus origin; no pointer enters the body.
 */
export async function expectKeyboardScroll(page: Page, preceding: Locator, region: Locator) {
  await expect(region).toHaveAttribute("role", "region");
  await expect(region).toHaveAccessibleName(/.+/);
  expect(await region.evaluate((node) => node.scrollHeight > node.clientHeight + 1),
    "the fixture must overflow the region before keyboard scrolling").toBe(true);
  await preceding.focus();
  await expect(preceding).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(region).toBeFocused();
  expect(await region.evaluate((node) => getComputedStyle(node).outlineStyle)).toBe("solid");
  const before = await region.evaluate((node) => node.scrollTop);
  await page.keyboard.press("PageDown");
  await expect.poll(() => region.evaluate((node) => node.scrollTop),
    { message: "PageDown scrolls the focused region" }).toBeGreaterThan(before);
}
