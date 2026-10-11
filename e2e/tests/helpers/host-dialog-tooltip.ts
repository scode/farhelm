import { expect } from "./evidence";
import type { Locator, Page } from "@playwright/test";

/**
 * Establish working keyboard help before opening a confirmation. A missing
 * tooltip asset must fail this positive control, rather than make every
 * later absence assertion look like successful initial-focus suppression.
 */
export async function expectKeyboardTooltip(page: Page, control: Locator): Promise<void> {
  await page.mouse.move(0, 0);
  await page.keyboard.press("Tab");
  await control.focus();
  await expect(control).toBeFocused();
  expect(await control.evaluate((element) => element.matches(":focus-visible"))).toBe(true);
  const text = await control.getAttribute("data-tooltip");
  expect(text, "positive control has explanatory hover text").toBeTruthy();
  await expect(page.locator("body > .farhelm-tooltip")).toBeVisible();
  await expect(page.locator("body > .farhelm-tooltip")).toHaveText(text!);
}

/**
 * Automatic Cancel focus must keep its keyboard ring without hover help.
 * Call after a keyboard-tooltip positive control and with the pointer parked;
 * observe beyond the show delay so a pending tooltip cannot count as quiet.
 */
export async function expectQuietInitialCancel(page: Page, cancel: Locator): Promise<void> {
  await expect(cancel).toBeFocused();
  const focus = await cancel.evaluate((element) => {
    const style = getComputedStyle(element);
    return { visible: element.matches(":focus-visible"), ring: style.outlineStyle !== "none" && parseFloat(style.outlineWidth) > 0 };
  });
  expect(focus, "Cancel retains the browser's keyboard focus and painted ring").toEqual({ visible: true, ring: true });
  expect(await page.evaluate(() => document.elementFromPoint(0, 0)?.closest("[data-tooltip]") ?? null),
    "the parked pointer cannot explain a tooltip").toBeNull();
  await page.waitForTimeout(600); // sleep-ok: initial-focus absence window exceeds the tooltip's 300 ms show delay
  await expect(page.locator("body > .farhelm-tooltip")).toBeHidden();
}

/**
 * Quiet initial focus must leave the first deliberate hover and later keyboard
 * re-entry usable. Run after layout captures so ordinary help does not cover
 * the dialog being compared with the mockup.
 */
export async function expectCancelTooltipReturns(page: Page, cancel: Locator, other: Locator): Promise<void> {
  const text = await cancel.getAttribute("data-tooltip");
  expect(text, "Cancel remains a tooltip target").toBeTruthy();
  await cancel.hover();
  await expect(page.locator("body > .farhelm-tooltip")).toBeVisible();
  await expect(page.locator("body > .farhelm-tooltip")).toHaveText(text!);
  await expectKeyboardTooltip(page, other);
  await cancel.focus();
  await expect(cancel).toBeFocused();
  await expect(page.locator("body > .farhelm-tooltip")).toBeVisible();
  await expect(page.locator("body > .farhelm-tooltip")).toHaveText(text!);
}
