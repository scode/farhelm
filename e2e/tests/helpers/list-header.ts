import { expect, type Page } from "@playwright/test";

const SORT_LABELS = {
  activity: "Recently active",
  created: "Newest created",
  title: "Title A–Z",
} as const;

/** Pointer users click the painted label; clipped native radios keep checked state and keyboard focus. */
export async function chooseStatus(page: Page, choice: "All" | "Running" | "Waiting" | "Idle"): Promise<void> {
  const group = page.getByRole("radiogroup", { name: "Session status" });
  await group.locator("label").filter({ hasText: new RegExp(`^${choice}$`) }).locator("span").click();
  await expect(group.getByRole("radio", { name: choice, exact: true })).toBeChecked();
}

/** Choose through the public menu so request and preference tests exercise the real control. */
export async function chooseSort(page: Page, sort: keyof typeof SORT_LABELS): Promise<void> {
  await page.locator(".sort-title").click();
  const menu = page.getByRole("menu", { name: "Sort", exact: true });
  await expect(menu).toBeVisible();
  await menu.getByRole("menuitemradio", { name: new RegExp(`^${SORT_LABELS[sort]}(?:$|\\s)`) }).click();
  await expect(menu).toHaveCount(0);
  await expect(page.locator(".sort-title")).toHaveAttribute("data-sort", sort);
}

/** Change density through its menu checkbox, leaving focus on the surviving title button. */
export async function setCompact(page: Page, compact: boolean): Promise<void> {
  const title = page.locator(".sort-title");
  await title.click();
  const menu = page.getByRole("menu", { name: "Sort", exact: true });
  const choice = menu.getByRole("menuitemcheckbox", { name: /^Compact(?:$|\s)/ });
  await expect(choice).toBeVisible();
  if ((await choice.getAttribute("aria-checked")) !== String(compact)) {
    await choice.click();
  } else {
    await choice.focus();
    await expect(choice).toBeFocused();
    await page.keyboard.press("Escape");
  }
  await expect(menu).toHaveCount(0);
  await expect(title).toBeFocused();
}

/** Observe density without opening a menu that could disturb a popup or focus fixture. */
export async function expectCompactRows(page: Page, compact: boolean): Promise<void> {
  const identity = page.locator(".session-row").first().locator(".session-identity-copy");
  await expect(identity).toBeVisible({ timeout: 20_000 });
  if (compact) await expect(identity).toHaveClass(/(^| )compact( |$)/);
  else await expect(identity).not.toHaveClass(/(^| )compact( |$)/);
}
