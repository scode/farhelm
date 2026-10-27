import { type Page } from "@playwright/test";
import { expect } from "./evidence";

/** Open the persistent caret before looking for its transient menu items.
 * Keeping this boundary explicit prevents callers from treating an unmounted
 * With item as a toolbar button or a launcher cancellation target. */
export async function openSessionActionMenu(page: Page, action: "Restart" | "Replace") {
  const toggle = page.getByRole("button", { name: `${action} options`, exact: true });
  await expect(toggle).toBeVisible();
  await expect(toggle).toBeEnabled();
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  const menu = page.getByRole("menu", { name: action, exact: true });
  await expect(menu).toBeVisible();
  return menu;
}

/** Enter the existing editable launcher through the user-visible split menu.
 * Availability belongs to the selected item, independently of its plain
 * action. The item must disappear when the launcher takes over. */
export async function selectSessionActionWith(page: Page, action: "Restart" | "Replace") {
  const menu = await openSessionActionMenu(page, action);
  const item = menu.getByRole("menuitem", { name: `${action} with…`, exact: true });
  await expect(item).toHaveAttribute("aria-disabled", "false");
  await item.click();
  await expect(menu).toHaveCount(0);
}
