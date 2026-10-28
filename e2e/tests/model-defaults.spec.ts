import { expect, test } from "./helpers/evidence";
import { patchPreferences } from "./helpers/fleet";

/** The composer must serialize an omitted model for every harness whose CLI
 * can use a configured default. The helm compiler tests pin the corresponding
 * argv, while these cases catch a disabled Launch button or an invented model
 * before the request reaches the helm. Model omission is independent of the
 * permission default: OMP starts Guarded, while the other cases start YOLO. */
for (const [label, harness, permission] of [
  ["OpenCode", "open_code", "yolo"],
  ["Goose", "goose", "yolo"],
  ["Pi", "pi", "yolo"],
  ["OMP", "omp", "guarded"],
] as const) {
  test(`${label} can launch with its configured model`, async ({ page, request }) => {
    const catalog = await request.get("/api/launch-catalog");
    expect(catalog.ok()).toBe(true);
    const build = catalog.headers()["x-farhelm-build"];
    expect(build).toBeTruthy();
    await page.route("**/api/launch-history**", (route) => route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": build },
      body: JSON.stringify({ launches: [], folders: [] }),
    }));
    await patchPreferences(request, { remembered_permissions: null });
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: label, exact: true }).click();
    await form.getByLabel("Folder", { exact: true }).fill("/tmp");
    await expect(form.getByRole("combobox", { name: "Model", exact: true })).toHaveValue("Harness default");
    const permissions = form.locator(".launch-composer-permissions-choice");
    await expect(permissions.getByRole("button", { name: "Default", exact: true })).toHaveCount(0);
    await expect(permissions.getByRole("button", { name: permission === "yolo" ? "YOLO" : "Guarded", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(form.locator(".create-session-submit")).toBeEnabled();

    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      await route.fulfill({ status: 400, headers: { "x-farhelm-build": build }, body: "fixture captured launch" });
    });
    const submitted = page.waitForRequest((entry) =>
      new URL(entry.url()).pathname === "/api/sessions" && entry.method() === "POST"
    );
    await form.locator(".create-session-submit").click();
    expect((await submitted).postDataJSON().launch).toMatchObject({ harness, model: null, permissions: permission });
    await expect(form).toContainText("fixture captured launch");
  });
}
