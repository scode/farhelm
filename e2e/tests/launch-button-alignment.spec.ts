/**
 * Launch context belongs beside a compact primary in a pinned footer.
 * An overflowing real draft distinguishes that layout from whole-form
 * scrolling: moving the fields must not move the summary or its actions.
 */
import { expect, test } from "./helpers/evidence";
import { type Locator, type Page } from "@playwright/test";

/** Populate both the command fields and inline snapshot editor so the body really overflows. */
async function openTallComposer(page: Page): Promise<Locator> {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form.locator('.launch-composer-search input[role="combobox"]')).toBeFocused();
  await form.getByLabel("Folder", { exact: true }).fill("/work/projects/a-long-folder-name-for-launch-summary");
  await form.getByLabel("Folder", { exact: true }).press("Escape");
  await expect(form.getByRole("listbox", { name: "Folders" })).toHaveCount(0);
  await form.getByRole("tab", { name: "Command", exact: true }).click();
  await form.getByLabel("Agent command", { exact: true }).fill("claude {farhelm_args}");
  await form.getByRole("radio", { name: "No", exact: true }).check();
  await form.getByRole("combobox", { name: "Agent type", exact: true }).selectOption("claude");
  await form.locator(".launch-command-resume-toggle").check();
  await form.getByLabel("Resume command", { exact: true }).fill("claude --resume {conversation} {farhelm_args}");
  await form.locator(".launch-composer-save-template").click();
  await expect(form.locator(".save-template-name")).toBeFocused();
  return form;
}

/** Both side-by-side and stacked hints must stay clear of the action hit targets. */
async function expectSeparateSummary(form: Locator) {
  const summary = await form.locator(".launch-composer-footer-summary").boundingBox();
  const actions = await form.locator(".launch-composer-actions").boundingBox();
  expect(summary, "the complete summary has a bounded footer box").not.toBeNull();
  expect(actions, "the action row has its own box").not.toBeNull();
  expect(
    summary!.y + summary!.height <= actions!.y + 1 ||
      summary!.x + summary!.width <= actions!.x + 1,
    "the summary sits above or beside actions without covering them",
  ).toBe(true);
}

for (const viewport of [
  { width: 1280, height: 540 },
  { width: 390, height: 844 },
  { width: 360, height: 300 },
]) {
  /** A long command draft cannot scroll its only launch/cancel controls out of reach. */
  test(`launcher summary and actions stay pinned at ${viewport.width}x${viewport.height}`, async ({ page }) => {
    await page.setViewportSize(viewport);
    const form = await openTallComposer(page);
    const body = form.locator(".launch-composer-body");
    const footer = form.locator(".launch-composer-footer");
    const primary = form.locator(".create-session-submit");
    await expect(primary).toHaveText("Launch");
    await expect(primary).toHaveAccessibleName(/^launch Command · .+ · \/work\/projects\/a-long-folder-name-for-launch-summary$/);
    await expect(footer.locator(".launch-composer-launch-context")).toContainText("Command · ");
    await expect(footer.locator(".launch-composer-summary")).toHaveCount(0);
    expect(await body.evaluate((node) => node.clientHeight > 0 && node.scrollHeight > node.clientHeight + 1),
      "the populated body's own scrollport must overflow before testing pinning").toBe(true);

    await body.evaluate((node) => { node.scrollTop = 0; });
    await expect.poll(() => body.evaluate((node) => node.scrollTop)).toBe(0);
    await expectSeparateSummary(form);
    const before = (await footer.boundingBox())!;
    for (const selector of [".launch-composer-cancel", ".launch-composer-save-template", ".create-session-submit"]) {
      await expect(form.locator(selector)).toBeInViewport({ ratio: 1 });
    }

    await body.evaluate((node) => { node.scrollTop = node.scrollHeight; });
    await expect.poll(() => body.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
    const after = (await footer.boundingBox())!;
    expect(Math.abs(after.y - before.y), "scrolling fields must not move the footer").toBeLessThan(1);
    expect(Math.abs(after.height - before.height), "scrolling fields must not resize the footer").toBeLessThan(1);
    await expectSeparateSummary(form);
    for (const selector of [".launch-composer-cancel", ".launch-composer-save-template", ".create-session-submit"]) {
      await expect(form.locator(selector)).toBeInViewport({ ratio: 1 });
    }
  });
}
