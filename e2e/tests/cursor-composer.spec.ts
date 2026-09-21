import { expect, test } from "./helpers/evidence";

/**
 * Cursor's launch UI must disclose the missing tracking integration while
 * preserving a literal model and permission choice in the create request.
 * Intercepting POST avoids requiring Cursor installation or authentication;
 * the helm unit test independently covers argv and Generic/no-Resume behavior.
 */
test("Cursor launches without claiming session tracking", async ({ page, request }) => {
  const catalog = await request.get("/api/launch-catalog");
  expect(catalog.ok()).toBe(true);
  const build = catalog.headers()["x-farhelm-build"];
  expect(build).toBeTruthy();
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Cursor", exact: true }).click();
  await expect(form).toContainText("Cursor session tracking and Resume are not supported.");
  await expect(form.locator(".launch-composer-effort-choice")).toHaveCount(0);
  const permissions = form.locator(".launch-composer-permissions-choice");
  await expect(permissions.getByRole("button")).toHaveCount(2);
  await permissions.getByRole("button", { name: "yolo", exact: true }).click();
  await form.getByRole("combobox", { name: "model", exact: true }).fill("composer-2.5");
  await form.getByRole("option", { name: "composer-2.5", exact: true }).click();
  await form.getByLabel("folder", { exact: true }).fill("/tmp");
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    await route.fulfill({ status: 400, headers: { "x-farhelm-build": build }, body: "fixture captured Cursor launch" });
  });
  const submitted = page.waitForRequest(
    (request) => new URL(request.url()).pathname === "/api/sessions" && request.method() === "POST",
  );
  await form.locator(".create-session-submit").click();
  expect((await submitted).postDataJSON().launch).toEqual({
    harness: "cursor", model: "composer-2.5", effort: null, permissions: "yolo",
  });
  await expect(form).toContainText("fixture captured Cursor launch");

  // The inactive structured draft must not leak its notice into another
  // profile, and both built-in Cursor profiles disclose the same limitation.
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "other / command", exact: true }).click();
  const profiles = form.locator(".create-session-profile");
  await profiles.selectOption("builtin-cursor");
  await expect(form).toContainText("Cursor session tracking and Resume are not supported.");
  await profiles.selectOption("builtin-cursor-yolo");
  await expect(form).toContainText("Cursor session tracking and Resume are not supported.");
  await profiles.selectOption("");
  await expect(form).not.toContainText("Cursor session tracking and Resume are not supported.");
});
