import { expect } from "./evidence";
import type { Locator, Page } from "@playwright/test";

/** Stable panel names survive presentation changes; App requires a desktop updater. */
export type SettingsSection =
  | "sessions"
  | "Appearance"
  | "hosts"
  | "terminal"
  | "checkouts"
  | "agent trust"
  | "notifications"
  | "app";

/** Navigation and headings use sentence case; panel identifiers keep their old names.
 * Keeping this distinction here lets callers select a section without coupling
 * draft-preservation tests to its visible spelling.
 */
export const settingsSectionLabels: Record<SettingsSection, string> = {
  sessions: "Sessions",
  Appearance: "Appearance",
  hosts: "Hosts",
  terminal: "Terminal",
  checkouts: "Checkouts",
  "agent trust": "Agent trust",
  notifications: "Notifications",
  app: "App",
};

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
    await dialog.getByRole("button", { name: "‹ Settings", exact: true }).click();
  }
  await navigation.getByRole("button", { name: settingsSectionLabels[section], exact: true }).click();
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
