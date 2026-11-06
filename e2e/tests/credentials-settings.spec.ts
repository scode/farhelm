/** Real catalog persistence, write-only replies and the shared account key.
 * Marker values are fictional; every test deletes only identities it creates.
 */
import { expect, test } from "./helpers/evidence";
import { openSettings, selectSettingsSection } from "./helpers/settings";

/** Replacing by name is useful only if the editor never returns the saved
 * secret, keeps its identity fixed, and survives normal Settings navigation.
 * The JSON file picker must fill the same explicit draft as paste; cancelling
 * that draft must not accidentally create a catalog entry.
 */
test("credentials keep fixed names, replace write-only values and discard login drafts", async ({ page, request }, testInfo) => {
  const name = `catalog-${testInfo.workerIndex}`;
  const token = "fictional-browser-credential-first";
  const replacement = "fictional-browser-credential-second";
  const codex = "fictional-browser-codex-secret";
  const before = await request.get("/api/credentials");
  expect(before.ok()).toBe(true);
  expect((await before.json() as { name: string }[]).some((row) => row.name === name)).toBe(false);
  try {
    await page.goto("/");
    const dialog = await openSettings(page, "credentials");
    const panel = dialog.locator("#settings-credentials");
    await expect(panel.getByText("Not encrypted on disk", { exact: true })).toHaveCount(2);
    await panel.getByRole("button", { name: "Add GitHub token", exact: true }).click();
    const editor = panel.locator(".credential-editor");
    await expect(editor.getByRole("heading", { name: "New GitHub token", exact: true })).toBeFocused();
    await editor.getByLabel("Name", { exact: true }).fill(name);
    await editor.getByLabel("Token", { exact: true }).fill(token);
    await selectSettingsSection(page, "tensorlake");
    await selectSettingsSection(page, "credentials");
    await expect(editor.getByLabel("Token", { exact: true })).toHaveValue(token);
    await editor.getByRole("button", { name: "Save", exact: true }).click();
    const row = panel.locator(".credential-row").filter({ hasText: name });
    await expect(row).toContainText("Saved");
    let response = await request.get("/api/credentials");
    expect(await response.text()).not.toContain(token);
    await row.getByRole("button", { name: `Edit ${name}`, exact: true }).click();
    await expect(editor.getByLabel("Name", { exact: true })).not.toBeEditable();
    await expect(editor.getByLabel("Token", { exact: true })).toHaveValue("");
    await editor.getByLabel("Token", { exact: true }).fill(replacement);
    await editor.getByRole("button", { name: "Save", exact: true }).click();
    await expect(row).toContainText("Replaced");
    response = await request.get("/api/credentials");
    const text = await response.text();
    expect(text).not.toContain(token);
    expect(text).not.toContain(replacement);
    expect((JSON.parse(text) as { name: string; revision: number }[]).find((row) => row.name === name)?.revision).toBe(2);

    await page.setViewportSize({ width: 390, height: 700 });
    await panel.getByRole("button", { name: "Add agent login", exact: true }).click();
    await expect(editor.getByRole("heading", { name: "New agent login", exact: true })).toBeFocused();
    await editor.getByRole("radio", { name: "Codex", exact: true }).click();
    await expect(editor).toContainText("when one copy renews it, other copies cannot renew and need a fresh sign-in");
    await expect(editor).toContainText("prefer a proxy or load balancer through an API endpoint");
    await expect(editor).toContainText("provider's terms of service");
    await editor.getByLabel("Name", { exact: true }).fill(`${name}-login`);
    await editor.getByLabel("Choose auth.json file", { exact: true }).setInputFiles({
      name: "auth.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify({ future: { token: codex } })),
    });
    await expect(editor.getByLabel("auth.json", { exact: true })).toHaveValue(JSON.stringify({ future: { token: codex } }));
    // The warning must remain readable above the sticky answers on a phone;
    // DOM visibility alone would accept a callout painted behind those buttons.
    const warning = editor.locator(".credential-storage-warning");
    await warning.scrollIntoViewIfNeeded();
    await expect(warning).toHaveCSS("border-left-width", "4px");
    const warningBox = await warning.boundingBox();
    const answersBox = await editor.locator(".credential-editor-actions").boundingBox();
    expect(warningBox).not.toBeNull();
    expect(answersBox).not.toBeNull();
    expect(warningBox!.y + warningBox!.height).toBeLessThanOrEqual(answersBox!.y + 1);
    await dialog.screenshot({ path: testInfo.outputPath("credentials-codex-narrow.png") });
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await openSettings(page, "credentials");
    await expect(panel.locator(".credential-editor")).toHaveCount(0);
    expect((await (await request.get("/api/credentials")).json() as { name: string }[]).some((row) => row.name === `${name}-login`)).toBe(false);
    await panel.getByRole("button", { name: `Edit ${name}`, exact: true }).click();
    await editor.getByRole("button", { name: "Delete", exact: true }).click();
    await expect(row).toHaveCount(0);
  } finally {
    await page.goto("about:blank");
    const response = await request.delete(`/api/credentials/${name}`);
    expect([200, 404]).toContain(response.status());
  }
});

/** Both key forms are mounted together. Saving and clearing in either must
 * update the other immediately, without returning the secret or discarding
 * that other form's independent unsaved replacement draft.
 */
test("the two Tensorlake key forms share acknowledged metadata and separate drafts", async ({ page, request }) => {
  const original = await (await request.get("/api/tensorlake")).json() as { api_key_set: boolean };
  expect(original.api_key_set, "fixture has no existing secret to restore").toBe(false);
  try {
    await page.goto("/");
    const dialog = await openSettings(page, "credentials");
    let panel = dialog.locator("#settings-credentials");
    const key = panel.getByRole("region", { name: "Tensorlake API key", exact: true });
    await expect(key.getByLabel("API key", { exact: true })).toBeEditable();
    await key.getByLabel("API key", { exact: true }).fill("fictional-tensorlake-key");
    await key.getByRole("button", { name: "Save", exact: true }).click();
    await expect(key.getByText("API key saved", { exact: true })).toBeVisible();
    panel = await selectSettingsSection(page, "tensorlake");
    const other = panel.getByRole("region", { name: "Tensorlake API key", exact: true });
    await expect(other.getByLabel("API key", { exact: true })).toHaveAttribute("placeholder", "Key saved; paste a new one to replace it");
    await other.getByLabel("API key", { exact: true }).fill("unsaved-independent-key-draft");
    await selectSettingsSection(page, "credentials");
    await key.getByRole("button", { name: "Clear", exact: true }).click();
    await expect(key.getByText("API key cleared", { exact: true })).toBeVisible();
    await selectSettingsSection(page, "tensorlake");
    await expect(other.getByRole("button", { name: "Test", exact: true })).toBeDisabled();
    await expect(other.getByLabel("API key", { exact: true })).toHaveValue("unsaved-independent-key-draft");
    const response = await request.get("/api/tensorlake");
    expect(await response.text()).not.toContain("fictional-tensorlake-key");
  } finally {
    await page.goto("about:blank");
    const response = await request.put("/api/tensorlake", { data: { clear_api_key: true } });
    expect(response.ok()).toBe(true);
  }
});

/** Reference notices must reach an editor that is already open without
 * replacing its unsaved secret. Recipe pickers must filter by kind and retain
 * a disappeared draft choice until the user explicitly selects None.
 */
test("recipe references refresh deletion guards and preserve unavailable picker choices", async ({ page, request }) => {
  const endpoint = "e2e-live-endpoint";
  const github = "e2e-live-github";
  const recipeName = "e2e-live-recipe";
  const owned: string[] = [];
  let ownsRecipe = false;
  const definition = {
    size: { cpus: 2, memory_gb: 4, disk_gb: 20 }, source: { kind: "image", image: "example/systemd" },
    claude: { base_url: "https://proxy.example/v1", token_credential: endpoint as string | null }, codex: null, tunnels: [],
  };
  try {
    for (const [name, kind] of [[endpoint, "endpoint_token"], [github, "github_token"]]) {
      const created = await request.post("/api/credentials", { data: { name, kind, secret: "fictional-reference-secret" } });
      expect(created.status(), await created.text()).toBe(201);
      owned.push(name);
    }
    const created = await request.post("/api/tensorlake/recipes", { data: { name: recipeName, recipe: definition } });
    expect(created.ok(), await created.text()).toBe(true);
    ownsRecipe = true;
    await page.goto("/");
    const dialog = await openSettings(page, "credentials");
    const panel = dialog.locator("#settings-credentials");
    await panel.getByRole("button", { name: `Edit ${endpoint}`, exact: true }).click();
    const editor = panel.locator(".credential-editor");
    await expect(editor.getByRole("button", { name: "Delete", exact: true })).toBeDisabled();
    await expect(editor).toContainText(recipeName);
    await editor.getByLabel("Token", { exact: true }).fill("unsaved-reference-draft");
    definition.claude.token_credential = null;
    const cleared = await request.put(`/api/tensorlake/recipes/${recipeName}`, { data: definition });
    expect(cleared.ok(), await cleared.text()).toBe(true);
    await expect(editor.getByRole("button", { name: "Delete", exact: true })).toBeEnabled();
    await expect(editor.getByLabel("Token", { exact: true })).toHaveValue("unsaved-reference-draft");
    await editor.getByRole("button", { name: "Cancel", exact: true }).click();
    const catalog = await (await request.get("/api/credentials")).json() as { name: string; revision: number }[];
    expect(catalog.find((row) => row.name === endpoint)?.revision).toBe(1);

    await selectSettingsSection(page, "tensorlake");
    await dialog.getByRole("button", { name: `Edit ${recipeName}`, exact: true }).click();
    const recipeEditor = dialog.getByRole("region", { name: `Recipe: ${recipeName}`, exact: true });
    const picker = recipeEditor.getByLabel("Claude Code endpoint token", { exact: true });
    await expect(picker).toBeEnabled();
    expect(await picker.locator("option").allTextContents()).toContain(endpoint);
    expect(await picker.locator("option").allTextContents()).not.toContain(github);
    await picker.selectOption(endpoint);
    const removed = await request.delete(`/api/credentials/${endpoint}`);
    expect(removed.ok(), await removed.text()).toBe(true);
    await expect(picker).toHaveValue(endpoint);
    await expect(picker.locator("option:checked")).toHaveText(`${endpoint} (unavailable)`);
    const refused = page.waitForResponse((response) => response.url().endsWith(`/api/tensorlake/recipes/${recipeName}`) && response.request().method() === "PUT");
    await recipeEditor.getByRole("button", { name: "Save", exact: true }).click();
    expect((await refused).status()).toBe(400);
    await expect(recipeEditor.locator(".recipe-editor-sticky-actions").getByRole("alert")).toContainText("select an existing API endpoint token");
    await picker.selectOption("");
    await expect(picker).toHaveValue("");
    await expect(recipeEditor.getByText("The selected endpoint credential is unavailable. Choose another or None.", { exact: true })).toHaveCount(0);
    // None restores the saved selection, so change the URL to exercise an
    // actual successful save rather than clicking the unchanged-form answer.
    await recipeEditor.getByLabel("Claude Code base URL", { exact: true }).fill("https://another.example/v1");
    await expect(recipeEditor.getByRole("button", { name: "Save", exact: true })).toBeEnabled();
    await recipeEditor.getByRole("button", { name: "Save", exact: true }).click();
    await expect(recipeEditor).toHaveCount(0);
    const response = await request.get("/api/tensorlake/recipes");
    expect(response.ok(), await response.text()).toBe(true);
    const listed = await response.json() as { recipes: { name: string; claude: { token_credential: string | null } }[] };
    expect(listed.recipes.find((recipe) => recipe.name === recipeName)?.claude.token_credential).toBeNull();
  } finally {
    if (!page.isClosed()) await page.goto("about:blank");
    if (ownsRecipe) {
      const recipe = await request.delete(`/api/tensorlake/recipes/${recipeName}`);
      expect([200, 404], await recipe.text()).toContain(recipe.status());
    }
    for (const name of owned) {
      const removed = await request.delete(`/api/credentials/${name}`);
      expect([200, 404], await removed.text()).toContain(removed.status());
    }
  }
});
