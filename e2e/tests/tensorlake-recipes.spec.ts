/** Tensorlake recipes in Settings: the list, the in-place editor, and the
 * helm's recipe API behind them. Nothing here talks to Tensorlake (recipes
 * are local records until a sandbox is created), so the real helm is used
 * throughout and the API is read directly to check what was stored. Recipe
 * names carry an `e2e-` prefix and every test removes what it created. */
import { expect, test } from "./helpers/evidence";
import type { APIRequestContext, Locator, Page } from "@playwright/test";
import { openSettings } from "./helpers/settings";

/** A token that must never appear in a reply or on the page once saved. */
const TOKEN = "e2e-private-endpoint-token";

/** Every recipe the helm lists, as the API returns them. */
async function recipes(request: APIRequestContext): Promise<any[]> {
  const response = await request.get("/api/tensorlake/recipes");
  expect(response.ok(), await response.text()).toBe(true);
  return (await response.json()).recipes;
}

/** Delete the named recipes if they exist; cleanup after a test, ignoring 404s. */
async function removeRecipes(request: APIRequestContext, names: string[]) {
  for (const name of names) {
    const response = await request.delete(`/api/tensorlake/recipes/${name}`);
    expect([200, 404], await response.text()).toContain(response.status());
  }
}

/** Open Settings → Tensorlake and return its recipe list region. */
async function openRecipes(page: Page): Promise<Locator> {
  const dialog = await openSettings(page, "tensorlake");
  const list = dialog.getByRole("region", { name: "Recipes", exact: true });
  await expect(list.getByRole("button", { name: "New recipe", exact: true })).toBeVisible();
  return list;
}

/** Spec: a managed recipe created in the editor is stored with its size,
 * packages (split on spaces and new lines), tunnel and Claude Code endpoint;
 * the list summarizes it; the token is never returned, and reopening shows the
 * name read-only and an empty token field that says a token is saved. An edit
 * that leaves the token blank keeps it, and Delete asks first and removes it.
 *
 * Why: this is the whole create/edit/delete loop a user goes through, and the
 * write-only token is the part a regression would quietly break (a token
 * echoed into the page, or cleared by an unrelated edit). */
test("a managed recipe is created, edited and deleted without exposing its token", async ({ page, request }) => {
  const name = "e2e-rust";
  await page.goto("/");
  expect((await recipes(request)).map((r) => r.name), "fixture premise: no leftover recipe").not.toContain(name);
  try {
    const list = await openRecipes(page);
    await list.getByRole("button", { name: "New recipe", exact: true }).click();
    const editor = page.getByRole("region", { name: "New recipe", exact: true });
    await expect(editor).toBeVisible();
    await editor.getByRole("textbox", { name: "Name", exact: true }).fill(name);
    await expect(editor.getByRole("spinbutton", { name: /^CPUs/ })).toHaveValue("2");
    await expect(editor.getByRole("spinbutton", { name: /^Memory, GB/ })).toHaveValue("4");
    await expect(editor.getByRole("spinbutton", { name: /^Disk, GB/ })).toHaveValue("20");
    await expect(editor.getByRole("button", { name: /^Install packages/ })).toHaveAttribute("aria-pressed", "true");
    await editor.getByRole("textbox", { name: "Ubuntu packages", exact: true }).fill("git ripgrep\njq git");
    await editor.getByRole("button", { name: "Add tunnel", exact: true }).click();
    const tunnel = editor.getByRole("group", { name: "Tunnel 1", exact: true });
    await tunnel.getByRole("spinbutton", { name: "Sandbox port", exact: true }).fill("8080");
    await tunnel.getByRole("textbox", { name: "Host", exact: true }).fill("127.0.0.1");
    await tunnel.getByRole("spinbutton", { name: "Port", exact: true }).fill("8080");
    const claude = editor.getByRole("group", { name: "Claude Code endpoint", exact: true });
    await claude.getByRole("textbox", { name: "Claude Code base URL", exact: true }).fill("http://127.0.0.1:8080");
    await claude.getByLabel("Claude Code auth token", { exact: true }).fill(TOKEN);
    await expect(editor.getByText("Tokens are stored unencrypted in the helm's state on this machine.", { exact: false })).toBeVisible();
    await editor.getByRole("button", { name: "Save", exact: true }).click();

    // Saving returns to the list, which shows the stored recipe.
    const row = list.getByRole("listitem").filter({ hasText: name });
    await expect(row).toContainText("2 CPU · 4 GB · 20 GB · 3 packages · 1 tunnel");
    await expect(row).toContainText("Snapshot not built yet");
    const listed = await request.get("/api/tensorlake/recipes");
    const body = await listed.text();
    expect(body).not.toContain(TOKEN);
    const stored = JSON.parse(body).recipes.find((r: any) => r.name === name);
    expect(stored).toEqual({
      name,
      size: { cpus: 2, memory_gb: 4, disk_gb: 20 },
      source: { kind: "packages", packages: ["git", "ripgrep", "jq"] },
      claude: { base_url: "http://127.0.0.1:8080", token_set: true },
      codex: null,
      tunnels: [{ sandbox_port: 8080, host: "127.0.0.1", port: 8080 }],
    });
    expect(await page.content()).not.toContain(TOKEN);

    // Reopen: the name is fixed and the saved token stays out of the form.
    await row.getByRole("button", { name: `Edit ${name}`, exact: true }).click();
    const reopened = page.getByRole("region", { name: `Recipe: ${name}`, exact: true });
    await expect(reopened.getByRole("textbox", { name: "Name", exact: true })).not.toBeEditable();
    const token = reopened.getByLabel("Claude Code auth token", { exact: true });
    await expect(token).toHaveValue("");
    await expect(token).toHaveAttribute("placeholder", "Token saved; leave blank to keep it");
    await expect(reopened.getByRole("textbox", { name: "Ubuntu packages", exact: true })).toHaveValue("git\nripgrep\njq");
    await reopened.getByRole("spinbutton", { name: /^Disk, GB/ }).fill("30");
    await reopened.getByRole("button", { name: "Save", exact: true }).click();
    await expect(row).toContainText("2 CPU · 4 GB · 30 GB · 3 packages · 1 tunnel");
    const edited = (await recipes(request)).find((r) => r.name === name);
    expect(edited.size.disk_gb).toBe(30);
    expect(edited.claude.token_set, "a blank token field kept the saved token").toBe(true);

    // Delete asks first, then removes the recipe.
    await row.getByRole("button", { name: `Edit ${name}`, exact: true }).click();
    const deleting = page.getByRole("region", { name: `Recipe: ${name}`, exact: true });
    await deleting.getByRole("button", { name: "Delete", exact: true }).click();
    await expect(deleting.getByText(`Delete recipe ${name}? This can't be undone.`, { exact: true })).toBeVisible();
    expect((await recipes(request)).map((r) => r.name), "asking deleted nothing yet").toContain(name);
    await deleting.getByRole("button", { name: "Delete", exact: true }).click();
    await expect(list.getByRole("listitem").filter({ hasText: name })).toHaveCount(0);
    expect((await recipes(request)).map((r) => r.name)).not.toContain(name);
  } finally {
    await removeRecipes(request, [name]);
  }
});

/** Spec: the editor refuses an invalid name locally, shows the helm's refusal
 * for a name that already exists (without overwriting it), asks before Back
 * drops unsaved changes, and stores a reused image as "uses image" with the
 * systemd requirement spelled out in user terms.
 *
 * Why: "New recipe" must never overwrite another recipe, refusals must leave
 * the draft editable, and leaving with unsaved changes must not lose them
 * silently. The systemd wording is a user decision ("never the bare phrase
 * 'must boot systemd'"). */
test("the recipe editor refuses bad names, asks before leaving, and saves a reused image", async ({ page, request }) => {
  const existing = "e2e-existing";
  const reuse = "e2e-reuse";
  await page.goto("/");
  const before = (await recipes(request)).map((r) => r.name);
  expect(before, "fixture premise: no leftover recipes").not.toContain(existing);
  expect(before).not.toContain(reuse);
  try {
    const created = await request.post("/api/tensorlake/recipes", {
      data: {
        name: existing,
        recipe: { size: { cpus: 1, memory_gb: 1, disk_gb: 10 }, source: { kind: "packages", packages: [] }, claude: null, codex: null, tunnels: [] },
      },
    });
    expect(created.ok(), await created.text()).toBe(true);

    const list = await openRecipes(page);
    await expect(list.getByRole("listitem").filter({ hasText: existing })).toContainText("1 CPU · 1 GB · 10 GB · 0 packages");
    await list.getByRole("button", { name: "New recipe", exact: true }).click();
    const editor = page.getByRole("region", { name: "New recipe", exact: true });
    const nameField = editor.getByRole("textbox", { name: "Name", exact: true });

    await nameField.fill("Bad Name");
    await editor.getByRole("button", { name: "Save", exact: true }).click();
    await expect(editor.getByRole("alert")).toContainText("lowercase letters, digits and hyphens");
    await expect(nameField).toHaveValue("Bad Name");

    await nameField.fill(existing);
    await editor.getByRole("button", { name: "Save", exact: true }).click();
    await expect(editor.getByRole("alert")).toHaveText(`a recipe named ${existing} already exists`);
    const kept = (await recipes(request)).find((r) => r.name === existing);
    expect(kept.size, "the existing recipe was not overwritten").toEqual({ cpus: 1, memory_gb: 1, disk_gb: 10 });

    // Back with unsaved changes asks; Keep editing stays, Discard leaves.
    await editor.getByRole("button", { name: "‹ Recipes", exact: true }).click();
    await expect(editor.getByText("Save these changes before leaving?", { exact: true })).toBeVisible();
    await editor.getByRole("button", { name: "Keep editing", exact: true }).click();
    await expect(nameField).toHaveValue(existing);
    await editor.getByRole("button", { name: "‹ Recipes", exact: true }).click();
    await editor.getByRole("button", { name: "Discard", exact: true }).click();
    await expect(editor).toHaveCount(0);

    // A reused image: the systemd requirement in user terms, no packages.
    await list.getByRole("button", { name: "New recipe", exact: true }).click();
    await nameField.fill(reuse);
    await editor.getByRole("button", { name: /^Use an existing image or snapshot/ }).click();
    await expect(editor.getByRole("button", { name: "Image name", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(editor.getByText(/Your image must have systemd installed, enabled, and running as the init system at startup/)).toBeVisible();
    await expect(editor.getByText(/must boot systemd/)).toHaveCount(0);
    await editor.getByRole("textbox", { name: "Image", exact: true }).fill("team-base");
    await editor.getByRole("button", { name: "Save", exact: true }).click();
    const row = list.getByRole("listitem").filter({ hasText: reuse });
    await expect(row).toContainText("2 CPU · 4 GB · 20 GB · uses image team-base");
    await expect(row).toContainText("Uses the image as is");
    expect((await recipes(request)).find((r) => r.name === reuse).source).toEqual({ kind: "image", image: "team-base" });
  } finally {
    await removeRecipes(request, [existing, reuse]);
  }
});

/** Spec: with unsaved changes in the recipe editor, Escape and both of
 * Settings' close controls ask "Save these changes before leaving?" instead of
 * closing. Keep editing keeps the dialog and the draft; Discard closes it and
 * stores nothing; Save stores the recipe and then closes. Without changes,
 * Close closes at once.
 *
 * Why: the editor sits inside the Settings modal, so the modal's own
 * dismissal paths would otherwise unmount it and drop a draft (tokens
 * included) that the editor's Back button promises to ask about. */
test("closing Settings with an unsaved recipe asks first and finishes the close", async ({ page, request }) => {
  const discarded = "e2e-close-discard";
  const saved = "e2e-close-save";
  await page.goto("/");
  const before = (await recipes(request)).map((r) => r.name);
  expect(before, "fixture premise: no leftover recipes").not.toContain(discarded);
  expect(before).not.toContain(saved);
  const dialog = page.getByRole("dialog", { name: "Settings", exact: true });
  const question = dialog.getByText("Save these changes before leaving?", { exact: true });
  try {
    // Escape: Keep editing stays, then Discard closes without storing.
    let list = await openRecipes(page);
    await list.getByRole("button", { name: "New recipe", exact: true }).click();
    const editor = dialog.getByRole("region", { name: "New recipe", exact: true });
    const nameField = editor.getByRole("textbox", { name: "Name", exact: true });
    await nameField.fill(discarded);
    await nameField.press("Escape");
    await expect(question).toBeVisible();
    await expect(dialog).toBeVisible();
    await editor.getByRole("button", { name: "Keep editing", exact: true }).click();
    await expect(question).toHaveCount(0);
    await expect(nameField).toHaveValue(discarded);
    await nameField.press("Escape");
    await expect(question).toBeVisible();
    await editor.getByRole("button", { name: "Discard", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    expect((await recipes(request)).map((r) => r.name), "Discard stored nothing").not.toContain(discarded);

    // The footer's Close: Save stores the recipe, then closes.
    list = await openRecipes(page);
    await list.getByRole("button", { name: "New recipe", exact: true }).click();
    await nameField.fill(saved);
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(question).toBeVisible();
    await editor.getByRole("button", { name: "Save", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    expect((await recipes(request)).map((r) => r.name)).toContain(saved);

    // The header's close control asks the same; an unchanged editor closes at once.
    list = await openRecipes(page);
    await list.getByRole("button", { name: "New recipe", exact: true }).click();
    await nameField.fill("e2e-close-header");
    await dialog.getByRole("button", { name: "Close settings", exact: true }).click();
    await expect(question).toBeVisible();
    await editor.getByRole("button", { name: "Discard", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    list = await openRecipes(page);
    await list.getByRole("button", { name: "New recipe", exact: true }).click();
    await expect(editor).toBeVisible();
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(dialog).toHaveCount(0);
  } finally {
    await removeRecipes(request, [discarded, saved, "e2e-close-header"]);
  }
});

/** Spec: when a re-read of the recipe list fails after an earlier read
 * succeeded, the list keeps its rows, says it may be out of date and offers
 * Retry loading; a successful retry shows the current recipes and clears the
 * error.
 *
 * Why: the invalidation-feed notice that triggered the failed read is spent,
 * so without a visible error and Retry the list would stay stale with no way
 * to recover short of reopening Settings. */
test("a failed recipe refresh says so and Retry loading recovers", async ({ page, request }) => {
  const name = "e2e-refresh";
  await page.goto("/");
  expect((await recipes(request)).map((r) => r.name), "fixture premise: no leftover recipe").not.toContain(name);
  try {
    const list = await openRecipes(page);
    await expect(list.getByRole("status")).toHaveCount(0);
    await expect(list.getByRole("alert")).toHaveCount(0);
    // Fail every browser read from now on; the API fixture below is not routed.
    let failing = true;
    let failedReads = 0;
    await page.route("**/api/tensorlake/recipes", async (route) => {
      if (failing && route.request().method() === "GET") {
        failedReads++;
        await route.fulfill({ status: 500, body: "recipe store unavailable" });
      } else {
        await route.continue();
      }
    });
    // Another client's create bumps the feed, which makes this list re-read.
    const created = await request.post("/api/tensorlake/recipes", {
      data: {
        name,
        recipe: { size: { cpus: 1, memory_gb: 1, disk_gb: 10 }, source: { kind: "packages", packages: [] }, claude: null, codex: null, tunnels: [] },
      },
    });
    expect(created.ok(), await created.text()).toBe(true);
    await expect(list.getByRole("alert")).toContainText("Could not refresh the recipes, so this list may be out of date");
    expect(failedReads, "the refresh went through the failing route").toBeGreaterThan(0);
    await expect(list.getByRole("listitem").filter({ hasText: name })).toHaveCount(0);
    failing = false;
    await list.getByRole("button", { name: "Retry loading", exact: true }).click();
    await expect(list.getByRole("listitem").filter({ hasText: name })).toBeVisible();
    await expect(list.getByRole("alert")).toHaveCount(0);
  } finally {
    await page.unroute("**/api/tensorlake/recipes");
    await removeRecipes(request, [name]);
  }
});
