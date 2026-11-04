/** Tensorlake Add host through the real probe/provision routes. The API and
 * host execution are fakes; registration, one-use confirmation, steps, recipe
 * storage and sandbox deletion are the shipped implementations. */
import { expect, test } from "./helpers/evidence";
import { readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import { currentStackSlot } from "../stack-slots";
import { hostRowByName, openHostMenu, openHostsPanel, patchPreferences } from "./helpers/fleet";
import type { APIRequestContext } from "@playwright/test";

/** Atomic controls keep a running step from ever observing half a file. */
async function controls(kind: "tensorlake_fake" | "provisioning_backend", config: unknown) {
  const info = JSON.parse(await readFile(currentStackSlot().stackInfoPath, "utf8"));
  const root = info[kind];
  const next = path.join(root, `config.${process.pid}.next`);
  await writeFile(next, JSON.stringify(config));
  await rename(next, path.join(root, "config.json"));
  return root as string;
}

/** The registry is the readiness oracle, rather than the add dialog closing. */
async function hosts(request: APIRequestContext): Promise<any[]> {
  const response = await request.get("/api/hosts");
  expect(response.ok(), await response.text()).toBe(true);
  return (await response.json()).hosts;
}

/** Spec: SSH's skip preference cannot authorize billed creation. The managed
 * recipe builds, progress grows into ordinary setup steps, and deleting the
 * host terminates its sandbox. Why: a dialog-only assertion would miss billed
 * resources left behind or a setup placeholder that never becomes real work. */
test("adding a Tensorlake host confirms billing and keeps setup progress", async ({ page, request }) => {
  const name = "e2e-tensorlake-host";
  const recipe = "e2e-host-recipe";
  const fake = await controls("tensorlake_fake", { snapshot_polls: 0 });
  await controls("provisioning_backend", { default: { hold_actions: true } });
  expect((await hosts(request)).some((h) => h.alias === name)).toBe(false);
  expect((await request.put("/api/tensorlake", { data: { api_key: "test-key", create_without_asking: false } })).ok()).toBe(true);
  expect((await request.post("/api/tensorlake/recipes", { data: { name: recipe, recipe: { size: { cpus: 2, memory_gb: 4, disk_gb: 20 }, source: { kind: "packages", packages: [] }, claude: null, codex: null, tunnels: [] } } })).ok()).toBe(true);
  await patchPreferences(request, { skip_host_setup_confirmation: true });
  let host: any;
  try {
    await page.goto("/");
    await openHostsPanel(page);
    await page.getByRole("button", { name: "Add host", exact: true }).last().click();
    const dialog = page.getByRole("dialog", { name: "Add host", exact: true });
    await dialog.getByRole("button", { name: "Tensorlake sandbox", exact: true }).click();
    await dialog.getByRole("textbox", { name: "Name", exact: true }).fill(name);
    await dialog.getByRole("combobox", { name: "Recipe", exact: true }).selectOption(recipe);
    await dialog.getByRole("button", { name: "Continue", exact: true }).click();
    await expect(dialog.getByText("Create Tensorlake sandbox?", { exact: true })).toBeVisible();
    await expect(dialog.getByText("This sandbox bills while running.", { exact: false })).toBeVisible();
    await expect(dialog.getByRole("link", { name: "Tensorlake pricing", exact: true })).toHaveAttribute("href", "https://www.tensorlake.ai/pricing");
    await expect(dialog.getByText("On Usage Credits", { exact: false })).toBeVisible();
    expect((await hosts(request)).some((h) => h.alias === name), "confirmation alone creates nothing").toBe(false);
    await dialog.getByRole("button", { name: "Create sandbox", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await expect.poll(async () => { host = (await hosts(request)).find((h) => h.alias === name); return host?.kind; }).toBe("tensorlake");
    expect(host.yolo_without_asking).toBe(true);
    expect(host.tensorlake.recipe).toBe(recipe);
    await expect.poll(async () => (await (await request.get(`/api/hosts/${host.id}/provisioning`)).json()).steps.map((s: any) => s.step)).toContain("prepare-sandbox");
    await controls("provisioning_backend", {});
    await expect.poll(async () => (await (await request.get(`/api/hosts/${host.id}/provisioning`)).json()).status).toBe("completed");
    const progress = await (await request.get(`/api/hosts/${host.id}/provisioning`)).json();
    expect(progress.steps.map((s: any) => s.step)).toContain("attach-supervisor");
    expect(progress.steps.map((s: any) => s.step)).not.toContain("set-up-farhelm");
    expect((await request.delete(`/api/hosts/${host.id}`)).ok()).toBe(true);
    host = null;
    const model = JSON.parse(await readFile(path.join(fake, "state.json"), "utf8"));
    expect(model.sandboxes.every((s: any) => s.status === "terminated")).toBe(true);
  } finally {
    await controls("provisioning_backend", {});
    if (host) await request.delete(`/api/hosts/${host.id}`);
    await request.delete(`/api/tensorlake/recipes/${recipe}`);
    await request.put("/api/tensorlake", { data: { clear_api_key: true, create_without_asking: false } });
    await patchPreferences(request, { skip_host_setup_confirmation: false });
  }
});

/** Spec: the no-key form opens Settings at Tensorlake; an inline recipe's
 * dirty draft survives Escape until Save or Discard. Why: embedding the full
 * editor must preserve the same departure contract as Settings. */
test("the Tensorlake form links Settings and protects an inline recipe draft", async ({ page, request }) => {
  await controls("provisioning_backend", {});
  await request.put("/api/tensorlake", { data: { clear_api_key: true } });
  await page.goto("/");
  await openHostsPanel(page);
  await page.getByRole("button", { name: "Add host", exact: true }).last().click();
  const dialog = page.getByRole("dialog", { name: "Add host", exact: true });
  await dialog.getByRole("button", { name: "Tensorlake sandbox", exact: true }).click();
  await dialog.getByRole("button", { name: "Open Settings → Tensorlake", exact: true }).click();
  const settings = page.getByRole("dialog", { name: "Settings", exact: true });
  await expect(settings.getByRole("heading", { name: "Tensorlake", exact: true })).toBeVisible();
  await request.put("/api/tensorlake", { data: { api_key: "test-key" } });
  await page.keyboard.press("Escape");
  await expect(dialog.getByRole("combobox", { name: "Recipe", exact: true })).toBeVisible();
  await dialog.getByRole("combobox", { name: "Recipe", exact: true }).selectOption("__new__");
  const editor = dialog.getByRole("region", { name: "New recipe", exact: true });
  await editor.getByRole("textbox", { name: "Name", exact: true }).fill("e2e-unsaved");
  await page.keyboard.press("Escape");
  await expect(editor.getByText("Save these changes before leaving?", { exact: true })).toBeVisible();
  await editor.getByRole("button", { name: "Keep editing", exact: true }).click();
  await expect(editor.getByRole("textbox", { name: "Name", exact: true })).toHaveValue("e2e-unsaved");
  // The removed answer button no longer owns focus. Name the editor input
  // as the premise of the next keyboard close request.
  await editor.getByRole("textbox", { name: "Name", exact: true }).focus();
  await expect(editor.getByRole("textbox", { name: "Name", exact: true })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(editor.getByText("Save these changes before leaving?", { exact: true })).toBeVisible();
  await editor.getByRole("button", { name: "Discard", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await request.put("/api/tensorlake", { data: { clear_api_key: true } });
});

/** Spec: saving an inline reuse recipe selects it, and only the acknowledged
 * Tensorlake preference skips creation's confirmation. Why: this covers the
 * second add path without a managed build and keeps the two preferences apart. */
test("an inline reuse recipe can create without asking through the Tensorlake preference", async ({ page, request }) => {
  const name = "e2e-inline-sandbox";
  const recipe = "e2e-inline-reuse";
  await controls("provisioning_backend", {});
  await request.put("/api/tensorlake", { data: { api_key: "test-key", create_without_asking: true } });
  let host: any;
  try {
    await page.goto("/");
    await openHostsPanel(page);
    await page.getByRole("button", { name: "Add host", exact: true }).last().click();
    const dialog = page.getByRole("dialog", { name: "Add host", exact: true });
    await dialog.getByRole("button", { name: "Tensorlake sandbox", exact: true }).click();
    await dialog.getByRole("textbox", { name: "Name", exact: true }).fill(name);
    await dialog.getByRole("combobox", { name: "Recipe", exact: true }).selectOption("__new__");
    const editor = dialog.getByRole("region", { name: "New recipe", exact: true });
    await editor.getByRole("textbox", { name: "Name", exact: true }).fill(recipe);
    await editor.getByRole("button", { name: /^Use an existing image or snapshot/ }).click();
    await editor.getByRole("textbox", { name: "Image", exact: true }).fill("example/systemd");
    await editor.getByRole("button", { name: "Save", exact: true }).click();
    await expect(dialog.getByRole("combobox", { name: "Recipe", exact: true })).toHaveValue(recipe);
    await dialog.getByRole("button", { name: "Continue", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await expect.poll(async () => { host = (await hosts(request)).find((h) => h.alias === name); return host?.kind; }).toBe("tensorlake");
    await expect.poll(async () => (await (await request.get(`/api/hosts/${host.id}/provisioning`)).json()).status).toBe("completed");
    const progress = await (await request.get(`/api/hosts/${host.id}/provisioning`)).json();
    expect(progress.steps.some((s: any) => s.step === "build-snapshot")).toBe(false);
  } finally {
    if (host) await request.delete(`/api/hosts/${host.id}`);
    await request.delete(`/api/tensorlake/recipes/${recipe}`);
    await request.put("/api/tensorlake", { data: { clear_api_key: true, create_without_asking: false } });
  }
});

/** A confirmation may outlive its recipe. Refuse and roll back the new row
 * before billing when deletion won; a frozen plan must not leave a host
 * referring to a recipe the user can no longer edit. */
test("a recipe deleted during confirmation refuses without registering a host", async ({ request }) => {
  const recipe = "e2e-deleted-recipe";
  const name = "e2e-deleted-recipe-host";
  await request.put("/api/tensorlake", { data: { api_key: "test-key" } });
  try {
    expect((await request.post("/api/tensorlake/recipes", { data: { name: recipe, recipe: { size: { cpus: 2, memory_gb: 4, disk_gb: 20 }, source: { kind: "image", image: "example/systemd" }, claude: null, codex: null, tunnels: [] } } })).ok()).toBe(true);
    const planned = await request.post("/api/hosts/probe", { data: { target: { kind: "tensorlake", name, recipe } } });
    expect(planned.ok(), await planned.text()).toBe(true);
    const probe = await planned.json();
    expect((await request.delete(`/api/tensorlake/recipes/${recipe}`)).ok()).toBe(true);
    const refused = await request.post("/api/hosts/provision", { data: { probe_id: probe.probe_id } });
    expect(refused.status()).toBe(409);
    expect(await refused.text()).toContain("recipe was deleted");
    expect((await hosts(request)).some((host) => host.alias === name)).toBe(false);
  } finally {
    await request.delete(`/api/tensorlake/recipes/${recipe}`);
    await request.put("/api/tensorlake", { data: { clear_api_key: true } });
  }
});

/** Cancel and a queued confirm can run in one event burst before unmount.
 * The accepted close must synchronously retire creation authority. */
test("cancel retires a Tensorlake creation before a queued answer", async ({ page, request }) => {
  const recipe = "e2e-cancel-create";
  const name = "e2e-cancel-create-host";
  await request.put("/api/tensorlake", { data: { api_key: "test-key", create_without_asking: false } });
  expect((await request.post("/api/tensorlake/recipes", { data: { name: recipe, recipe: { size: { cpus: 2, memory_gb: 4, disk_gb: 20 }, source: { kind: "image", image: "example/systemd" }, claude: null, codex: null, tunnels: [] } } })).ok()).toBe(true);
  let submissions = 0;
  page.on("request", (req) => { if (req.method() === "POST" && new URL(req.url()).pathname === "/api/hosts/provision") submissions++; });
  try {
    await page.goto("/");
    await openHostsPanel(page);
    await page.getByRole("button", { name: "Add host", exact: true }).last().click();
    const dialog = page.getByRole("dialog", { name: "Add host", exact: true });
    await dialog.getByRole("button", { name: "Tensorlake sandbox", exact: true }).click();
    await dialog.getByRole("textbox", { name: "Name", exact: true }).fill(name);
    await dialog.getByRole("combobox", { name: "Recipe", exact: true }).selectOption(recipe);
    await dialog.getByRole("button", { name: "Continue", exact: true }).click();
    await expect(dialog.getByRole("button", { name: "Create sandbox", exact: true })).toBeEnabled();
    await dialog.evaluate((element) => {
      const cancel = element.querySelector<HTMLButtonElement>(".add-host-cancel")!;
      const confirm = element.querySelector<HTMLButtonElement>(".provisioning-confirm")!;
      cancel.click();
      confirm.click();
    });
    await expect(dialog).toHaveCount(0);
    expect(submissions).toBe(0);
    expect((await hosts(request)).some((host) => host.alias === name)).toBe(false);
  } finally {
    const host = (await hosts(request)).find((host) => host.alias === name);
    if (host) await request.delete(`/api/hosts/${host.id}`);
    await request.delete(`/api/tensorlake/recipes/${recipe}`);
    await request.put("/api/tensorlake", { data: { clear_api_key: true } });
  }
});

for (const skipTensorlake of [false, true]) {
  /** A failed run may not yet have created anything. Rerun must use only
   * Tensorlake's billing preference, with no answer that changes SSH's. */
  test(`rerun before first create uses Tensorlake skip=${skipTensorlake}`, async ({ page, request }) => {
    const name = `e2e-rerun-policy-${skipTensorlake}`;
    const recipe = `e2e-rerun-recipe-${skipTensorlake}`;
    let host: any;
    const fake = await controls("tensorlake_fake", { fail: { create_sandbox: { status: 401, message: "key refused" } } });
    await controls("provisioning_backend", {});
    expect((await request.put("/api/tensorlake", { data: { api_key: "test-key", create_without_asking: skipTensorlake } })).ok()).toBe(true);
    await patchPreferences(request, { skip_host_setup_confirmation: !skipTensorlake });
    expect((await request.post("/api/tensorlake/recipes", { data: { name: recipe, recipe: { size: { cpus: 2, memory_gb: 4, disk_gb: 20 }, source: { kind: "image", image: "example/systemd" }, claude: null, codex: null, tunnels: [] } } })).ok()).toBe(true);
    try {
      const probe = await (await request.post("/api/hosts/probe", { data: { target: { kind: "tensorlake", name, recipe } } })).json();
      const accepted = await (await request.post("/api/hosts/provision", { data: { probe_id: probe.probe_id } })).json();
      host = { id: accepted.host_id };
      await expect.poll(async () => (await (await request.get(`/api/hosts/${host.id}/provisioning`)).json()).status).toBe("failed");
      const model = JSON.parse(await readFile(path.join(fake, "state.json"), "utf8").catch(() => '{"sandboxes":[]}'));
      expect(model.sandboxes.filter((s: any) => s.status !== "terminated")).toHaveLength(0);
      await controls("tensorlake_fake", {});
      await page.goto("/");
      await openHostsPanel(page);
      const row = hostRowByName(page, name);
      await openHostMenu(row);
      await row.locator(".provisioning-rerun").click();
      const confirmation = page.getByRole("dialog", { name: "Host setup", exact: true });
      if (!skipTensorlake) {
        await expect(confirmation.getByText("This sandbox bills while running.", { exact: false })).toBeVisible();
        await expect(confirmation.getByRole("button", { name: "Set up, don't ask again", exact: true })).toHaveCount(0);
        expect((await (await request.get(`/api/hosts/${host.id}/provisioning`)).json()).status).toBe("failed");
        await confirmation.getByRole("button", { name: "Create sandbox", exact: true }).click();
      }
      await expect.poll(async () => (await (await request.get(`/api/hosts/${host.id}/provisioning`)).json()).status).toBe("completed");
      await expect(confirmation).toHaveCount(0);
      const prefs = await (await request.get("/api/preferences")).json();
      expect(prefs.skip_host_setup_confirmation).toBe(!skipTensorlake);
    } finally {
      await controls("tensorlake_fake", {});
      if (host) await request.delete(`/api/hosts/${host.id}`);
      await request.delete(`/api/tensorlake/recipes/${recipe}`);
      await request.put("/api/tensorlake", { data: { clear_api_key: true, create_without_asking: false } });
      await patchPreferences(request, { skip_host_setup_confirmation: false });
    }
  });
}
