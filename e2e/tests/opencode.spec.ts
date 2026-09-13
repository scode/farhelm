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
  await expect(form.locator(".launch-composer-effort-choice")).toHaveCount(0);
  await expect(harness.getByRole("button", { name: "other / command", exact: true })).toBeVisible();
  await expect(form.getByText("choose an OpenCode model before launching", { exact: true })).toBeVisible();
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  const model = modelRow.getByRole("combobox", { name: "model", exact: true });
  await model.focus();
  // Premise: focusing opened the list (its toggle row is present), so the
  // missing default row below is an omission, not an unopened listbox.
  await expect(modelRow.locator("#launch-composer-model-results").getByRole("option", { name: "show every harness's models", exact: true })).toBeVisible();
  await expect(modelRow.getByRole("option", { name: "harness default", exact: true })).toHaveCount(0);
  for (const id of ids) {
    await model.fill(id);
    await modelRow.getByRole("option", { name: id, exact: true }).click();
    await expect(model).toHaveValue(id);
    await expect(form.locator(".create-session-submit")).toBeEnabled();
  }
  await model.fill("custom'42;$literal");
  await model.press("Enter");
  // The submit click below blurs the field; if Enter had not applied the
  // draft, blur would discard it and the POST would carry the previous id.
  await expect(model).toHaveValue("custom'42;$literal");
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
