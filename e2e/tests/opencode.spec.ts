import { expect, test } from "./helpers/evidence";

/**
 * OpenCode's optional model must stay inside the existing composer rows.
 * RSX accepts a misplaced conditional as valid nesting, so compilation cannot
 * prove that adding the default row preserved the command escape and model
 * layout. Use the real catalog and rendered controls; intercept only
 * history and the final create so no vendor executable or credential is used.
 */
test("OpenCode keeps composer controls while offering default and explicit Zen models", async ({ page, request }) => {
  const response = await request.get("/api/launch-catalog");
  expect(response.ok()).toBe(true);
  const build = response.headers()["x-farhelm-build"];
  expect(build).toBeTruthy();
  const models = await response.json();
  const ids = models.filter((model: { harness: string }) => model.harness === "open_code")
    .map((model: { id: string }) => model.id);
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
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  const model = modelRow.getByRole("combobox", { name: "model", exact: true });
  await expect(model).toHaveValue("harness default");
  await model.focus();
  // Focusing opens the list; the default row shares the same model field and
  // explicit Zen suggestions without displacing the command escape.
  await expect(modelRow.locator("#launch-composer-model-results").getByRole("option", { name: "show every harness's models", exact: true })).toBeVisible();
  await expect(modelRow.getByRole("option", { name: "harness default", exact: true })).toBeVisible();
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
  // OpenCode has one explicit permission mode, including for older omitted
  // selections. Its configured model does not imply a permission default.
  const permissions = form.locator(".launch-composer-permissions-choice");
  await expect(permissions.getByRole("button")).toHaveCount(1);
  await expect(permissions.getByRole("button", { name: "yolo", exact: true })).toHaveAttribute("aria-pressed", "true");
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    await route.fulfill({ status: 400, headers: { "x-farhelm-build": build }, body: "fixture captured launch" });
  });
  const submitted = page.waitForRequest((request) =>
    new URL(request.url()).pathname === "/api/sessions" && request.method() === "POST"
  );
  await form.locator(".create-session-submit").click();
  expect((await submitted).postDataJSON().launch).toEqual({
    harness: "open_code", model: "custom'42;$literal", effort: null, permissions: "yolo",
  });
  await expect(form).toContainText("fixture captured launch");
  await harness.getByRole("button", { name: "other / command", exact: true }).click();
  await expect(form.locator(".create-session-profile")).toBeEnabled();
  // Retained structured settings are a draft, not a second active launch mode.
  await expect(harness.locator('[aria-pressed="true"]')).toHaveCount(1);
  await expect(harness.getByRole("button", { name: "other / command", exact: true })).toHaveAttribute("aria-pressed", "true");
  await harness.getByRole("button", { name: "OpenCode", exact: true }).click();
  await expect(harness.locator('[aria-pressed="true"]')).toHaveCount(1);
  await expect(harness.getByRole("button", { name: "OpenCode", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(model).toHaveValue("custom'42;$literal");
});

/**
 * Why: OpenCode's bare Zen names include ones Codex also lists
 * (`gpt-6-luna`), and Enter used to switch an OpenCode draft to Codex while
 * the helm refused the OpenCode launch. Spec (SPEC.md, Launch composer and
 * OpenCode): typing never changes a selected harness. With OpenCode selected,
 * Enter on the bare name keeps OpenCode and the typed text, and the launch
 * body carries exactly that. With Codex selected, Enter on OpenCode's
 * qualified id names OpenCode and changes nothing. The real catalog is the
 * premise; only history and the final create are intercepted.
 */
test("typing a model never switches the selected harness, and bare OpenCode names stay OpenCode's", async ({ page, request }) => {
  const response = await request.get("/api/launch-catalog");
  expect(response.ok()).toBe(true);
  const build = response.headers()["x-farhelm-build"];
  expect(build).toBeTruthy();
  const models = await response.json() as { id: string; harness: string }[];
  // Premise: the bare name is Codex's catalog id and OpenCode lists it only
  // under its qualified spelling.
  expect(models.some((model) => model.id === "gpt-6-luna" && model.harness === "codex")).toBe(true);
  expect(models.some((model) => model.id === "opencode/gpt-6-luna" && model.harness === "open_code")).toBe(true);
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
  const model = form.locator(".launch-composer-model-choice").getByRole("combobox", { name: "model", exact: true });

  await harness.getByRole("button", { name: "Codex", exact: true }).click();
  await model.fill("opencode/gpt-6-luna");
  await model.press("Enter");
  await expect(form.locator(".launch-composer-choice-error")).toHaveText(
    "opencode/gpt-6-luna is offered by OpenCode; choose OpenCode to use it",
  );
  await expect(harness.getByRole("button", { name: "Codex", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(model).toHaveValue("opencode/gpt-6-luna");

  await harness.getByRole("button", { name: "OpenCode", exact: true }).click();
  await model.fill("gpt-6-luna");
  await model.press("Enter");
  await expect(model).toHaveValue("gpt-6-luna");
  await expect(harness.getByRole("button", { name: "OpenCode", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(form.locator(".launch-composer-choice-error")).toHaveCount(0);
  await expect(form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    await route.fulfill({ status: 400, headers: { "x-farhelm-build": build }, body: "fixture captured launch" });
  });
  const submitted = page.waitForRequest((request) =>
    new URL(request.url()).pathname === "/api/sessions" && request.method() === "POST"
  );
  await form.locator(".create-session-submit").click();
  expect((await submitted).postDataJSON().launch).toEqual({
    harness: "open_code", model: "gpt-6-luna", effort: null, permissions: "yolo",
  });
});
