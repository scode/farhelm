import { expect } from "./evidence";
import { type Locator, type Page } from "@playwright/test";

/**
 * A scroll region must be reachable by Tab and scroll through native
 * key input. Focusing the region directly or assigning scrollTop would conceal
 * WebKit's missing automatic tab stop, the regression this helper checks.
 * The adjacent control establishes the focus origin; no pointer enters the body.
 * Reverse Tab covers regions whose first ordinary control follows the text.
 */
export async function expectKeyboardScroll(
  page: Page,
  adjacent: Locator,
  region: Locator,
  key: "Tab" | "Shift+Tab" = "Tab",
) {
  await expect(region).toHaveAttribute("role", "region");
  await expect(region).toHaveAccessibleName(/.+/);
  expect(await region.evaluate((node) => node.scrollHeight > node.clientHeight + 1),
    "the fixture must overflow the region before keyboard scrolling").toBe(true);
  await adjacent.focus();
  await expect(adjacent).toBeFocused();
  await page.keyboard.press(key);
  await expect(region).toBeFocused();
  expect(await region.evaluate((node) => getComputedStyle(node).outlineStyle)).toBe("solid");
  const before = await region.evaluate((node) => node.scrollTop);
  await page.keyboard.press("PageDown");
  await expect.poll(() => region.evaluate((node) => node.scrollTop),
    { message: "PageDown scrolls the focused region" }).toBeGreaterThan(before);
}
