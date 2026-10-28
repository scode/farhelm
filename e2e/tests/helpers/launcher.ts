import type { Locator } from "@playwright/test";
import { expect } from "./evidence";

/** Finish typing an ordinary folder before testing another launcher control.
 * Suggestions deliberately float over later fields and keep focus on the
 * folder while their options are pressed. Escape dismisses that surface
 * without accepting a suggestion or cancelling the dialog. Use this only for
 * setup; tests of the open list, typed Enter or blur keep their own input.
 */
export async function fillLauncherFolder(form: Locator, path: string): Promise<void> {
  const folder = form.getByLabel("folder", { exact: true });
  const suggestions = form.getByRole("listbox", { name: "folders", exact: true });
  await folder.fill(path);
  await expect(folder).toBeFocused();
  await expect(suggestions).toBeVisible();
  await folder.press("Escape");
  await expect(suggestions).toHaveCount(0);
  await expect(folder).toHaveValue(path);
  await expect(form).toBeVisible();
}
