import { expect } from "./evidence";
import type { Locator, Page } from "@playwright/test";

/** App belongs to this set only where a desktop updater supplies its setting. */
export type SettingsSection = "sessions" | "hosts" | "terminal" | "checkouts" | "agent trust" | "notifications" | "app";

/** Select through the same navigation at desktop and phone widths.
 * Only visibility is awaited: callers also exercise locked initial reads and
 * pending saves, whose controls must not become ready as a helper side effect.
 */
export async function selectSettingsSection(page: Page, section: SettingsSection): Promise<Locator> {
  const dialog = page.getByRole("dialog", { name: "settings", exact: true });
  await expect(dialog).toBeVisible();
  const panel = dialog.locator(`section.app-settings-panel[aria-label="${section}"]`);
  if (await panel.isVisible()) return panel;
  const navigation = dialog.getByRole("navigation", { name: "settings sections", exact: true });
  if (!await navigation.isVisible()) {
    await dialog.getByRole("button", { name: "‹ settings", exact: true }).click();
  }
  await navigation.getByRole("button", { name: section, exact: true }).click();
  await expect(panel).toBeVisible();
  return panel;
}

/** Open from the gear, preserving sessions' ordinary initial focus.
 * Trust-band entry tests use their real band instead, so this convenience
 * cannot conceal a broken choice of the initial trust pane.
 */
export async function openSettings(page: Page, section: SettingsSection = "sessions"): Promise<Locator> {
  await page.getByRole("button", { name: "settings", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "settings", exact: true });
  await expect(dialog).toBeVisible();
  // Assert the gear's reset before navigating, so the helper cannot repair a
  // remembered pane and conceal a regression in the opening contract.
  await expect(dialog.locator('section.app-settings-panel[aria-label="sessions"]')).toBeVisible();
  if (section !== "sessions") await selectSettingsSection(page, section);
  return dialog;
}
