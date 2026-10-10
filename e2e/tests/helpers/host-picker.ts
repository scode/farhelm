import { expect, type Locator } from "@playwright/test";

/** Open the launcher's real single-choice menu without changing its target.
 * Keep options scoped to this control; the composer has other listboxes. */
export async function launcherHostOptions(control: Locator): Promise<Locator> {
  await expect(control).toBeEnabled();
  if (await control.getAttribute("aria-expanded") !== "true") await control.click();
  await expect(control).toHaveAttribute("aria-expanded", "true");
  const list = control.locator("..").getByRole("listbox", { name: "host", exact: true });
  await expect(list).toBeVisible();
  return list.getByRole("option");
}

/** Choose through the same mouse path as a person, then verify the committed
 * host id rather than assuming that a click changed the launch destination.
 * Labels remain exact, including an unavailable host's phase suffix. */
export async function chooseLauncherHost(control: Locator, choice: string | { label: string } | { index: number }): Promise<void> {
  const options = await launcherHostOptions(control);
  const target = typeof choice === "string"
    ? control.locator("..").locator(`.launcher-host-option[data-host-id="${choice}"]`)
    : "label" in choice
    ? options.getByText(choice.label, { exact: true }).locator("..")
    : options.nth(choice.index);
  await expect(target).toBeVisible();
  const id = await target.getAttribute("data-host-id");
  expect(id, "the offered host must carry its stable id").toBeTruthy();
  await target.click();
  await expect(control).toHaveAttribute("data-host-id", id!);
  await expect(control).toHaveAttribute("aria-expanded", "false");
}
