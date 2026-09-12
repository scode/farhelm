import { expect, test } from "./helpers/evidence";

/**
 * The explicit-model exception must stay inside the existing composer rows.
 * RSX accepts a misplaced conditional as valid nesting, so compilation cannot
 * prove that hiding one default button preserved the command escape and
 * model layout. Use the real catalog and rendered controls; intercept only
 * history and the final create so no vendor executable or credential is used.
 */
test("OpenCode keeps composer controls while requiring a suggested or custom Zen model", async ({ page, request }) => {
  const response = await request.get("/api/launch-catalog");
  expect(response.ok()).toBe(true);
  const build = response.headers()["x-farhelm-build"];
  expect(build).toBeTruthy();
  const models = await response.json();
  const ids = ["opencode/glm-5.3-flash", "opencode/grok-4.5", "opencode/grok-4.6", "opencode/glm-5.3"];
  expect(models.filter((model: { harness: string }) => model.harness === "open_code")
    .map((model: { id: string }) => model.id)).toEqual(ids);
  await page.route("**/api/launch-history**", (route) => route.fulfill({
    status: 200,
    headers: { "content-type": "application/json", "x-farhelm-build": build },
    body: JSON.stringify({ launches: [], folders: [] }),
  }));
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.getByLabel("folder", { exact: true }).fill("/tmp");
  const harness = form.locator(".launch-composer-harness-choice");
  await harness.getByRole("button", { name: "Codex", exact: true }).click();
  await form.locator(".launch-composer-effort-choice").getByRole("button", { name: "high", exact: true }).click();
  await harness.getByRole("button", { name: "OpenCode", exact: true }).click();
  const modelRow = form.locator(".launch-composer-model-choice");
  await expect(harness.locator(".launch-composer-model-choice")).toHaveCount(0);
  await expect(modelRow.getByRole("button", { name: /harness default$/ })).toHaveCount(0);
  await expect(form.locator(".launch-composer-effort-choice")).toHaveCount(0);
  await expect(harness.getByRole("button", { name: "other / command", exact: true })).toBeVisible();
  await expect(form.getByText("choose an OpenCode model before launching", { exact: true })).toBeVisible();
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  const choices = modelRow.locator(":scope > .launch-composer-options > button");
  await expect(choices).toHaveCount(4);
  for (const id of ids) {
    const choice = choices.filter({ hasText: id }).filter({ hasText: new RegExp(`${id.replaceAll(".", "\\.")}$`) });
    await choice.click();
    await expect(choice).toHaveAttribute("aria-pressed", "true");
    await expect(form.locator(".create-session-submit")).toBeEnabled();
  }
  await modelRow.locator("summary").click();
  await modelRow.getByPlaceholder("custom model id").fill("custom'42;$literal");
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    await route.fulfill({ status: 400, headers: { "x-farhelm-build": build }, body: "fixture captured launch" });
  });
  const submitted = page.waitForRequest((request) =>
    new URL(request.url()).pathname === "/api/sessions" && request.method() === "POST"
  );
  await form.locator(".create-session-submit").click();
  expect((await submitted).postDataJSON().launch).toEqual({
    harness: "open_code", model: "custom'42;$literal", effort: null, permissions: null,
  });
  await expect(form).toContainText("fixture captured launch");
  await harness.getByRole("button", { name: "other / command", exact: true }).click();
  await expect(form.locator(".create-session-profile")).toBeEnabled();
});
