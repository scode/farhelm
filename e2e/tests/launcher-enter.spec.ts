/**
 * Launch refusals must be visible at the action that failed. Native disabled
 * used to swallow an incomplete draft's implicit Enter, while request errors
 * appeared below the scrolling controls. These cases distinguish an explained
 * refusal from silence without creating a session merely to inspect the UI.
 */
import { expect, test } from "./helpers/evidence";
import { type Locator } from "@playwright/test";

/** Pin the reason inside the initial dialog viewport, directly after its actions. */
async function expectReasonByLaunch(form: Locator, text: string) {
  const reason = form.locator(".launch-composer-refusal");
  await expect(reason).toBeVisible();
  await expect(reason).toContainText(text);
  const actions = await form.locator(".launch-composer-actions").boundingBox();
  const message = await reason.boundingBox();
  const dialog = await form.boundingBox();
  expect(actions, "the action row must be painted before measuring the reason").not.toBeNull();
  expect(message, "the refusal must have its own visible box").not.toBeNull();
  expect(dialog, "the dialog's initial viewport must be measurable").not.toBeNull();
  expect(message!.y).toBeGreaterThanOrEqual(actions!.y + actions!.height);
  expect(message!.y + message!.height).toBeLessThanOrEqual(dialog!.y + dialog!.height);
}

for (const attempt of ["Enter", "Launch"] as const) {
  /** An incomplete draft explains itself without sending a create; correcting it clears that reason. */
  test(`launcher ${attempt} explains an incomplete setup beside Launch`, async ({ page }) => {
    let posts = 0;
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() === "POST") posts += 1;
      await route.continue();
    });
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    const launch = form.locator(".create-session-submit");
    await expect(launch).toHaveAttribute("aria-disabled", "true");
    await expect(launch).toHaveAttribute("data-tooltip", "choose a structured harness before launching");
    await expect(form.locator(".launch-composer-refusal"), "opening an unfinished draft is not an error").toHaveCount(0);
    if (attempt === "Enter") {
      const search = form.getByRole("combobox", { name: "search folders, harnesses, models, and efforts", exact: true });
      await expect(search).toHaveValue("");
      await search.focus();
      await expect(search).toBeFocused();
      await search.press("Enter");
    } else {
      // aria-disabled describes the refusal but intentionally keeps native
      // activation alive. Playwright otherwise refuses this real pointer input.
      await launch.click({ force: true });
    }
    await expectReasonByLaunch(form, "choose a structured harness before launching");
    expect(posts, "a visible prerequisite refusal must post no create").toBe(0);
    await form.getByRole("button", { name: "Codex", exact: true }).click();
    await expect(launch).toHaveAttribute("aria-disabled", "false");
    await expect(form.locator(".launch-composer-refusal"), "correcting the prerequisite clears its reason").toHaveCount(0);
  });
}

/** Empty-command validation belongs to the visible refusal path, including implicit text-field Enter. */
test("launcher text-field Enter explains an empty command beside Launch", async ({ page }) => {
  let posts = 0;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() === "POST") posts += 1;
    await route.continue();
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.getByRole("tab", { name: "command", exact: true }).click();
  await form.getByRole("radio", { name: "no", exact: true }).check();
  const command = form.getByLabel("agent command", { exact: true });
  await expect(command).toHaveValue("");
  await expect(form.locator(".create-session-submit")).toHaveAttribute("aria-disabled", "false");
  await command.focus();
  await expect(command).toBeFocused();
  await command.press("Enter");
  await expectReasonByLaunch(form, "enter an agent command before launching");
  expect(posts, "the local empty-command refusal must post no create").toBe(0);
});

/** The ordinary submit path keeps the helm's refusal near Launch, within the initial viewport. */
test("launcher shows the helm's refusal beside Launch", async ({ page, request }) => {
  const build = (await request.get("/api/sessions")).headers()["x-farhelm-build"];
  expect(build, "the controlled response must preserve the live helm's build stamp").toBeTruthy();
  let posts = 0;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    posts += 1;
    await route.fulfill({ status: 400, headers: { "x-farhelm-build": build, "content-type": "text/plain" }, body: "launch fixture refused this request" });
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.getByRole("button", { name: "Codex", exact: true }).click();
  await form.getByLabel("folder", { exact: true }).fill("/tmp");
  await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "default", exact: true }).click();
  const launch = form.locator(".create-session-submit");
  await expect(launch).toHaveAttribute("aria-disabled", "false");
  await launch.click();
  await expectReasonByLaunch(form, "launch fixture refused this request");
  expect(posts, "the refusal must come from exactly one ordinary launch attempt").toBe(1);
});
