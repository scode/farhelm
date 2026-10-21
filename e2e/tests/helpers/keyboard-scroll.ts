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

/**
 * An enabled body control supplies scroll entry without a redundant region stop.
 *
 * Tab must reach that exact control from its real preceding control. Native
 * Home and PageDown then prove that its enclosing scrollport receives keyboard
 * scrolling even in WebKit; neither focus nor scroll offset is assigned to the
 * scrollport. Home also removes any offset left by earlier Copy assertions.
 */
export async function expectKeyboardScrollFromControl(
  page: Page,
  adjacent: Locator,
  entry: Locator,
  scrollport: Locator,
) {
  expect(await scrollport.evaluate((node) => node.scrollHeight > node.clientHeight + 1),
    "the control's scrollport must actually overflow").toBe(true);
  await expect(entry).toBeEnabled();
  await adjacent.focus();
  await expect(adjacent).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(entry).toBeFocused();
  await page.keyboard.press("Home");
  await expect.poll(() => scrollport.evaluate((node) => node.scrollTop),
    { message: "Home reaches the start through the focused control" }).toBe(0);
  await page.keyboard.press("PageDown");
  await expect.poll(() => scrollport.evaluate((node) => node.scrollTop),
    { message: "PageDown scrolls the focused control's scrollport" }).toBeGreaterThan(0);
}
