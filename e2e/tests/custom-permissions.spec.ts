/** Custom flags remain an editable, explicit YOLO choice across launcher surfaces.
 * Real helm refusals and template storage exercise the browser/API boundary;
 * these tests never confirm a launch into an installed vendor agent.
 */
import { expect, test } from "./helpers/evidence";
import { type Page } from "@playwright/test";
import { agentLaunchRow, listHosts, localHostId, openRowMenu, SESSION_LISTING, setLocalYoloWithoutAsking } from "./helpers/fleet";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";

installTerminalSuiteHooks();

/** Wait for the real launcher before acting on its agent and permission controls. */
async function openNew(page: Page) {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Claude", exact: true }).click();
  return form;
}

/** Search must focus the custom editor; invalid quotes remain repairable, and
 * words retain literal expansion syntax. Changing agents cannot reuse flags
 * written for a different command-line interface.
 */
test("custom permission search focuses literal words and switching agents clears them", async ({ page }) => {
  const form = await openNew(page);
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await search.fill("perms:custom");
  await form.getByRole("option", { name: "Permissions: custom", exact: true }).click();
  const flags = form.locator(".custom-permission-flags");
  await expect(flags).toBeFocused();
  await flags.fill("  ");
  await expect(flags).toHaveValue("  ");
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  await flags.fill("--mode '");
  await expect(flags).toHaveValue("--mode '");
  await expect(form.locator(".custom-permission-choice [role=alert]")).toContainText("invalid permission flags");
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  await flags.fill('--mode "ask me" "$HOME"');
  await expect(form.locator(".custom-permission-words code")).toHaveText(["--mode", "ask me", "$HOME"]);
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  await expect(flags).toHaveCount(0);
  await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "custom", exact: true }).click();
  await expect(flags).toHaveValue("");
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Goose", exact: true }).click();
  await expect(form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "custom", exact: true })).toHaveCount(0);
});

/** Enter on Custom opens its required editor instead of attempting a launch.
 * The focus assertion distinguishes the handoff from a disabled-primary refusal.
 */
test("custom choice Enter focuses its empty editor without submitting", async ({ page }) => {
  const form = await openNew(page);
  const posts: string[] = [];
  page.on("request", request => { if (request.method() === "POST") posts.push(request.url()); });
  const custom = form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "custom", exact: true });
  await custom.focus();
  await expect(custom).toBeFocused();
  await custom.press("Enter");
  await expect(form.locator(".custom-permission-flags")).toBeFocused();
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  expect(posts).toEqual([]);
});

for (const action of ["clone", "replace-with"] as const) {
  /** An injected listing retains real host metadata while exposing a custom
   * launch the fake agent does not generate. Refused mutation routes keep the
   * proof on the editor/request boundary, without launching a vendor agent.
   */
  test(`${action} preserves custom words and shows the stored YOLO provenance`, async ({ page }) => {
    const id = "77777777-3333-4444-5555-666666666666";
    const words = ["--mode", "ask me", "$HOME", "hidden\u200Bword"];
    const selection = { harness: "claude", model: null, effort: null, permissions: "custom", permission_flags: words };
    await page.route(SESSION_LISTING, async route => {
      const response = await route.fetch();
      const listing = await response.json();
      expect(listing.sessions.length, "the stack must supply real host metadata").toBeGreaterThan(0);
      listing.sessions.push({ ...listing.sessions[0], id, title: "custom-source", cwd: "/tmp",
        invocation: "misleading-display", launch: { ...agentLaunchRow(selection),
          start: ["claude", ...words, "{farhelm_args}"],
          resume: ["claude", ...words, "--resume", "{conversation}", "{farhelm_args}"] },
        // This resumable command has no captured conversation yet. The offer
        // must use the actual wire vocabulary so the listing still decodes.
        status: { state: "interrupted" }, restart_offer: "not_captured" });
      listing.total += 1;
      await route.fulfill({ response, json: listing });
    });
    const bodies: any[] = [];
    await page.route(action === "clone" ? "**/api/sessions" : `**/api/sessions/${id}/replace`, async route => {
      if (route.request().method() !== "POST") return route.continue();
      bodies.push(route.request().postDataJSON());
      await fulfillAsHelm(route, { status: 400, contentType: "text/plain", body: "controlled custom refusal" });
    });
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row).toBeVisible();
    const shield = row.locator(".permission-glyph");
    await expect(shield).toHaveAttribute("data-glyph", "yolo");
    await shield.hover();
    const help = page.locator("body > .farhelm-tooltip");
    await expect(help).toContainText("custom permission flags");
    await expect(help).toContainText("treats this as YOLO");
    await expect(help).toContainText("hidden<U+200B>word");
    await expect(help).toContainText("Launched with: claude");
    await expect(help).not.toContainText("misleading-display");
    await openRowMenu(row);
    await row.locator(`.session-row-${action}`).click();
    const form = page.locator(".create-session-form");
    const flags = form.locator(".custom-permission-flags");
    await expect(flags).toHaveValue("--mode 'ask me' '$HOME' hidden<U+200B>word");
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    await form.locator(".create-session-submit").click();
    await expect(form.locator(".create-session-error")).toContainText("controlled custom refusal");
    expect(bodies).toHaveLength(1);
    expect(action === "clone" ? bodies[0].launch : bodies[0].with.launch).toMatchObject(selection);
    await flags.fill('--mode "edited words"');
    await form.locator(".create-session-submit").click();
    await expect.poll(() => bodies.length).toBe(2);
    expect((action === "clone" ? bodies[1].launch : bodies[1].with.launch).permission_flags).toEqual(["--mode", "edited words"]);
  });
}

/** The real host guard must ask even when custom words name a guarded vendor
 * mode. Inspect the actual refused request, then cancel: no vendor is started.
 */
test("custom permission words reach the real YOLO refusal with their own explanation", async ({ page, request }) => {
  const host = await localHostId(request);
  const before = (await listHosts(request)).find((row) => row.id === host);
  expect(before, "local host fixture must exist").toBeTruthy();
  try {
    await setLocalYoloWithoutAsking(request, false);
    const form = await openNew(page);
    await form.getByLabel("folder", { exact: true }).fill("/tmp");
    await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "custom", exact: true }).click();
    await form.locator(".custom-permission-flags").fill("--permission-mode auto");
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    const [response] = await Promise.all([
      page.waitForResponse((response) => response.request().method() === "POST" && new URL(response.url()).pathname === "/api/sessions"),
      form.locator(".create-session-submit").click(),
    ]);
    expect(response.status()).toBe(409);
    const body = response.request().postDataJSON();
    expect(body.launch.permissions).toBe("custom");
    expect(body.launch.permission_flags).toEqual(["--permission-mode", "auto"]);
    const confirmation = form.locator(".yolo-confirmation");
    await expect(confirmation).toContainText("uses custom permission flags");
    await expect(confirmation).toContainText("treats the session as YOLO");
    await confirmation.locator(".yolo-cancel").click();
    await expect(confirmation).toHaveCount(0);
  } finally {
    await setLocalYoloWithoutAsking(request, before!.yolo_without_asking);
  }
});

/** Stored template words must reappear as editable quoted text, then clear
 * when a later permission template overrides them. A template with words but
 * no Custom declaration is refused at the actual save endpoint.
 */
test("custom template words round trip into the launcher and clear on a stacked permission", async ({ page, request }) => {
  const name = `e2e-custom-${Date.now()}`;
  const later = `${name}-guarded`;
  const other = `${name}-other`;
  const otherWords = ["--mode", "second template words"];
  const fields = { kind: "agent", agent: "claude", permissions: "custom", permission_flags: ["--mode", "ask me", "$HOME"] };
  try {
    expect((await request.put(`/api/templates/${name}`, { data: fields })).ok()).toBe(true);
    expect((await request.put(`/api/templates/${other}`, { data: { ...fields, permission_flags: otherWords } })).ok()).toBe(true);
    expect((await request.put(`/api/templates/${later}`, { data: { kind: "agent", permissions: "guarded" } })).ok()).toBe(true);
    const refused = await request.put(`/api/templates/${name}-invalid`, { data: { ...fields, permissions: "guarded" } });
    expect(refused.status()).toBe(400);
    const form = await openNew(page);
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    await search.fill(`tl:${name}`);
    await form.getByRole("option", { name: `Template: ${name}`, exact: true }).click();
    await expect(form.locator(".custom-permission-words code")).toHaveText(fields.permission_flags);
    await expect(form.locator(".custom-permission-flags")).toHaveValue("--mode 'ask me' '$HOME'");
    await search.fill(`tl:${later}`);
    await form.getByRole("option", { name: `Template: ${later}`, exact: true }).click();
    await expect(form.locator(".custom-permission-flags")).toHaveCount(0);
    await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "custom", exact: true }).click();
    await expect(form.locator(".custom-permission-flags")).toHaveValue("");
    await form.locator(".launch-composer-cancel").click();
    await page.getByRole("button", { name: "templates", exact: true }).click();
    const dialog = page.locator(".templates-dialog");
    await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    await dialog.locator(".templates-row-name").filter({ hasText: new RegExp(`^${name}$`) }).click();
    const flags = dialog.locator(".custom-permission-flags");
    await expect(flags).toHaveValue("--mode 'ask me' '$HOME'");
    let writes = 0;
    page.on("request", request => {
      if (request.method() === "PUT" && new URL(request.url()).pathname.startsWith("/api/templates/")) writes++;
    });
    await flags.fill("--mode '");
    await expect(dialog.locator(".custom-permission-choice [role=alert]")).toContainText("invalid permission flags");
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator('.templates-error[role="status"]')).toContainText("custom permissions need permission flags");
    expect(writes, "an invalid draft must be refused before its API write").toBe(0);
    await flags.fill('--mode "edited template words"');
    await expect(dialog.locator(".templates-save")).toBeEnabled();
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    const stored = await request.get("/api/templates");
    expect(stored.ok()).toBe(true);
    expect((await stored.json()).templates.find((template: { name: string }) => template.name === name).fields.permission_flags)
      .toEqual(["--mode", "edited template words"]);
    // Switching saved sources must replace field-local raw text, without a
    // blanket parsed-word sync that would erase a quote being repaired.
    await dialog.locator(".templates-row-name").filter({ hasText: new RegExp(`^${other}$`) }).click();
    await expect(flags).toHaveValue("--mode 'second template words'");
    await expect(dialog.locator(".custom-permission-words code")).toHaveText(otherWords);
    await dialog.locator(".templates-add").click();
    await dialog.locator(".templates-add-menu").getByRole("button", { name: "model", exact: true }).click();
    await expect(dialog.locator(".templates-save")).toBeEnabled();
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    const after = await request.get("/api/templates");
    expect(after.ok()).toBe(true);
    expect((await after.json()).templates.find((template: { name: string }) => template.name === other).fields.permission_flags)
      .toEqual(otherWords);
    await flags.fill("--broken '");
    await expect(flags).toHaveValue("--broken '");
    await expect(dialog.locator(".custom-permission-choice [role=alert]")).toContainText("invalid permission flags");
    await dialog.locator(".templates-row-name").filter({ hasText: new RegExp(`^${name}$`) }).click();
    await dialog.locator(".templates-discard").click();
    await expect(flags).toHaveValue("--mode 'edited template words'");
    await expect(dialog.locator(".custom-permission-choice [role=alert]")).toHaveCount(0);
  } finally {
    for (const template of [name, later, other, `${name}-invalid`]) {
      const response = await request.delete(`/api/templates/${template}`);
      expect(response.ok() || response.status() === 404).toBe(true);
    }
  }
});
