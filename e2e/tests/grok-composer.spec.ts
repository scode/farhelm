import { expect, test } from "./helpers/evidence";

/**
 * Grok has no verified model or effort command-line contract in this release.
 * Exercise the rendered composer because Rust can accept valid RSX that leaves
 * a hidden-state transition wired to the wrong control. The test starts with
 * explicit Codex values, switches through the searchable Grok row, and checks
 * both the visible controls and the serialized request that crosses the API.
 */
test("the Grok composer clears and hides unsupported model and effort choices", async ({ page, request }) => {
  const response = await request.get("/api/launch-catalog");
  expect(response.ok()).toBe(true);
  const build = response.headers()["x-farhelm-build"];
  expect(build).toBeTruthy();

  await page.route("**/api/launch-history**", (route) =>
    route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": build },
      body: JSON.stringify({ launches: [], folders: [] }),
    }),
  );
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();

  const harnesses = form.locator(".launch-composer-harness-choice");
  await harnesses.getByRole("button", { name: "Codex", exact: true }).click();
  const model = form.getByRole("combobox", { name: "model", exact: true });
  await model.fill("stale-custom-model");
  await model.press("Enter");
  await expect(model).toHaveAttribute("aria-expanded", "false");
  await expect(model).toHaveValue("stale-custom-model");
  const highEffort = form
    .locator(".launch-composer-effort-choice")
    .getByRole("button", { name: "high", exact: true });
  await highEffort.click();
  await expect(highEffort).toHaveAttribute("aria-pressed", "true");

  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await search.fill("grok");
  await form.getByRole("option", { name: "Harness: Grok", exact: true }).click();
  await expect(harnesses.getByRole("button", { name: "Grok", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  await expect(form.locator(".launch-composer-model-choice")).toHaveCount(0);
  await expect(form.locator(".launch-composer-effort-choice")).toHaveCount(0);
  const permissions = form.locator(".launch-composer-permissions-choice");
  await expect(permissions.getByRole("button")).toHaveCount(2);
  await permissions.getByRole("button", { name: "default", exact: true }).click();

  await form.getByLabel("folder", { exact: true }).fill("/tmp");
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    await route.fulfill({
      status: 400,
      headers: { "x-farhelm-build": build },
      body: "fixture captured launch",
    });
  });
  const submitted = page.waitForRequest(
    (request) => new URL(request.url()).pathname === "/api/sessions" && request.method() === "POST",
  );
  await form.locator(".create-session-submit").click();
  expect((await submitted).postDataJSON().launch).toEqual({
    harness: "grok",
    model: null,
    effort: null,
    permissions: null,
  });
  await expect(form).toContainText("fixture captured launch");
});
