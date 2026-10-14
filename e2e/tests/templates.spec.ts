/**
 * Launch templates in the browser (SPEC.md, Launch templates): the Templates
 * panel beside New stores them, and `tl:name` in the launcher applies one as
 * the edits it contains, all or nothing.
 *
 * Templates are helm-wide and shared by every spec on the stack, so each
 * test names its own with a timestamp and deletes them afterwards.
 */
import { expect, test } from "./helpers/evidence";
import { type APIRequestContext, type Page } from "@playwright/test";
import { cleanupSession, createSession, listHosts, openRowMenu, patchPreferences, readPreferences } from "./helpers/fleet";
import { routeGate } from "./helpers/route-gate";

/** Store a template through the helm's API, as the panel would. */
async function putTemplate(request: APIRequestContext, name: string, fields: Record<string, unknown>) {
  const response = await request.put(`/api/templates/${encodeURIComponent(name)}`, { data: fields });
  expect(response.ok(), `storing template ${name}: ${response.status()}`).toBe(true);
}

/** Remove a template, tolerating one a test already deleted. */
async function deleteTemplate(request: APIRequestContext, name: string) {
  const response = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
  expect(response.ok() || response.status() === 404, `deleting template ${name}`).toBe(true);
}

/** Keys move between saved names independently of the bolt. Keyboard Escape
 * closes only the nested menu; rename must retain the key, while delete/Undo
 * retires it permanently. An ineligible saved name still offers removal.
 */
test("number keys move, follow rename, and stay freed after undo", async ({ page, request }) => {
  const prefix = `number-bindings-${Date.now()}`;
  const [a, b, partial, renamed] = ["a", "b", "partial", "renamed"].map(s => `${prefix}-${s}`);
  try {
    await patchPreferences(request, { template_shortcuts: null, quick_launch_template: null });
    await putTemplate(request, a, { kind: "agent", agent: "claude" });
    await putTemplate(request, b, { kind: "agent", agent: "codex" });
    await putTemplate(request, partial, { effort: "high" });
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.getByRole("dialog", { name: "templates", exact: true });
    await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    const keyButton = (name: string) => dialog.getByRole("button", { name: `number shortcut ${name}`, exact: true });
    await expect(keyButton(partial)).toBeDisabled();
    await expect(keyButton(a)).toHaveText("Ctrl+Shift+ –");
    await expect(keyButton(a)).toHaveAttribute("data-tooltip", /right away.*Only one template/);
    await expect(dialog.getByRole("button", { name: `quick launch ${a}`, exact: true }))
      .toHaveAttribute("data-tooltip", /without opening New.*Only one template/);
    await keyButton(a).focus();
    await expect(keyButton(a)).toBeFocused();
    await page.keyboard.press("ArrowDown");
    const menu = dialog.getByRole("menu", { name: `number key for ${a}`, exact: true });
    await expect(menu.getByRole("menuitem", { name: "no shortcut", exact: true })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(menu).toHaveCount(0);
    await expect(dialog).toBeVisible();
    await expect(keyButton(a)).toBeFocused();
    await keyButton(a).click();
    await menu.getByRole("menuitem", { name: "Ctrl+Shift+1", exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).template_shortcuts).toEqual({ "1": a });
    await keyButton(b).click();
    const other = dialog.getByRole("menu", { name: `number key for ${b}`, exact: true });
    await expect(other.getByRole("menuitem", { name: `Ctrl+Shift+1 ${a} (moves here)`, exact: true })).toBeVisible();
    await other.getByRole("menuitem", { name: `Ctrl+Shift+1 ${a} (moves here)`, exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).template_shortcuts).toEqual({ "1": b });
    await keyButton(b).click();
    await other.getByRole("menuitem", { name: "Ctrl+Shift+0", exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).template_shortcuts).toEqual({ "0": b });
    await dialog.getByRole("button", { name: `quick launch ${b}`, exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).quick_launch_template).toBe(b);
    await dialog.locator(".templates-row-name").getByText(b, { exact: true }).click();
    await dialog.locator(".templates-name").fill(renamed);
    await dialog.getByRole("button", { name: "save", exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).template_shortcuts).toEqual({ "0": renamed });
    await expect.poll(async () => (await readPreferences(request)).quick_launch_template).toBe(renamed);
    await dialog.getByRole("button", { name: "delete", exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).template_shortcuts ?? {}).toEqual({});
    await dialog.getByRole("button", { name: "undo", exact: true }).click();
    await expect(keyButton(renamed)).toBeVisible();
    await expect(keyButton(renamed)).toHaveText("Ctrl+Shift+ –");
    await dialog.locator(".templates-close").click();
    await patchPreferences(request, { template_shortcuts: { "3": partial } });
    await page.reload();
    await page.locator(".templates-button").click();
    await expect(keyButton(partial)).toBeEnabled();
    await keyButton(partial).click();
    const remove = dialog.getByRole("menu", { name: `number key for ${partial}`, exact: true });
    await expect(remove.getByRole("menuitem")).toHaveCount(1);
    await remove.getByRole("menuitem", { name: "no shortcut", exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).template_shortcuts ?? {}).toEqual({});
    await expect(keyButton(partial)).toBeDisabled();
  } finally {
    await patchPreferences(request, { template_shortcuts: null, quick_launch_template: null });
    for (const name of [a, b, partial, renamed]) await deleteTemplate(request, name);
  }
});

/** An open toggle owns its pointerdown until click. Holding the button must
 * not let the shared outside relay close first and turn the click into reopen.
 */
test("number popup toggle stays open during pointerdown and closes on click", async ({ page, request }) => {
  const name = `number-pointer-${Date.now()}`;
  try {
    await putTemplate(request, name, { kind: "agent", agent: "claude" });
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.getByRole("dialog", { name: "templates", exact: true });
    const toggle = dialog.getByRole("button", { name: `number shortcut ${name}`, exact: true });
    await toggle.click();
    const menu = dialog.getByRole("menu", { name: `number key for ${name}`, exact: true });
    await expect(menu).toBeVisible();
    await test.info().attach("number-menu", { body: await page.screenshot(), contentType: "image/png" });
    await toggle.hover();
    await page.mouse.down();
    // sleep-ok: deliberate held-pointer stimulus lets the outside-dismiss timer run before the native click.
    await page.waitForTimeout(100);
    await expect(menu, "pointerdown on its own toggle cannot dismiss the menu").toBeVisible();
    await page.mouse.up();
    await expect(menu).toHaveCount(0);
    await expect(dialog).toBeVisible();
  } finally {
    await page.mouse.up();
    await deleteTemplate(request, name);
  }
});

for (const movement of ["scroll", "resize", "width-only resize"]) {
  /** Fixed coordinates are valid only while the owning row stays put. The
   * dialog must really overflow before scrolling; popup-internal scrolling
   * remains available, and dismissal must not drag the dialog back to its row.
   */
  test(`number popup closes when the dialog moves by ${movement}`, async ({ page, request }) => {
    const prefix = `number-layout-${movement.replaceAll(" ", "-")}-${Date.now()}`;
    const names = Array.from({ length: 16 }, (_, n) => `${prefix}-${String(n).padStart(2, "0")}`);
    try {
      for (const name of names) await putTemplate(request, name, { kind: "agent", agent: "claude" });
      await page.setViewportSize({ width: 1100, height: 500 });
      await page.goto("/");
      await page.locator(".templates-button").click();
      const dialog = page.getByRole("dialog", { name: "templates", exact: true });
      await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
      expect(await dialog.evaluate(node => node.scrollHeight > node.clientHeight), "premise: dialog overflows").toBe(true);
      await dialog.getByRole("button", { name: `number shortcut ${names[0]}`, exact: true }).click();
      const menu = dialog.getByRole("menu", { name: `number key for ${names[0]}`, exact: true });
      await expect(menu).toBeVisible();
      await expect(menu.getByRole("menuitem", { name: "no shortcut", exact: true })).toBeFocused();
      if (movement === "scroll") {
        await dialog.evaluate(node => { node.scrollTop = 120; });
        await expect.poll(() => dialog.evaluate(node => node.scrollTop)).toBe(120);
      } else if (movement === "resize") {
        await page.setViewportSize({ width: 900, height: 650 });
        expect(page.viewportSize()).toEqual({ width: 900, height: 650 });
      } else {
        const before = await dialog.boundingBox();
        expect(before).not.toBeNull();
        await page.setViewportSize({ width: 1300, height: 500 });
        const after = await dialog.boundingBox();
        expect(after).not.toBeNull();
        expect(after!.width, "premise: the dialog stays at its width cap").toBe(before!.width);
        expect(after!.x, "premise: centering moved the dialog").toBeGreaterThan(before!.x);
      }
      await expect(menu).toHaveCount(0);
      if (movement === "scroll") expect(await dialog.evaluate(node => node.scrollTop)).toBe(120);
      await expect(dialog).toBeVisible();
    } finally {
      for (const name of names) await deleteTemplate(request, name);
    }
  });
}

/** A delayed outside relay can arrive after another popup owns focus, especially
 * across the desktop bridge. Open both by keyboard to hold that ordering without
 * a timer race, then deliver the old relay twice: neither arrival may take focus.
 */
test("number popup delayed close keeps another popup's focus", async ({ page, request }) => {
  const names = ["a", "b"].map(s => `number-focus-${Date.now()}-${s}`);
  try {
    for (const name of names) await putTemplate(request, name, { kind: "agent", agent: "claude" });
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.getByRole("dialog", { name: "templates", exact: true });
    const menu = (name: string) => dialog.getByRole("menu", { name: `number key for ${name}`, exact: true });
    for (const name of names) {
      const toggle = dialog.getByRole("button", { name: `number shortcut ${name}`, exact: true });
      await toggle.focus();
      await expect(toggle).toBeFocused();
      await page.keyboard.press("ArrowDown");
      await expect(menu(name).getByRole("menuitem", { name: "no shortcut", exact: true })).toBeFocused();
    }
    await expect(menu(names[0])).toBeVisible();
    const current = menu(names[1]).getByRole("menuitem", { name: "no shortcut", exact: true });
    const relay = dialog.locator(`[data-row-menu="template-number:${names[0]}"]`);
    await expect(relay).toHaveCount(1);
    for (let delivery = 0; delivery < 2; delivery++) {
      await expect(current).toBeFocused();
      await relay.evaluate(node => (node as HTMLButtonElement).click());
      await expect(menu(names[0])).toHaveCount(0);
      await expect(current).toBeFocused();
    }
    await page.keyboard.press("Escape");
    await expect(menu(names[1])).toHaveCount(0);
    await expect(dialog).toBeVisible();
  } finally {
    for (const name of names) await deleteTemplate(request, name);
  }
});

/** Committing an IME candidate is still editing, not consent to save the
 * template. Edit to a distinct final name before ordinary Enter so an async
 * save started by composition cannot masquerade as that later authorized save. */
test("save as template leaves composing Enter with the name editor", async ({ page, request }) => {
  const name = `e2e-ime-${Date.now()}`;
  const unfinished = `${name}-unfinished`;
  let writes = 0;
  page.on("request", (request) => {
    if (request.method() === "PUT" && new URL(request.url()).pathname.startsWith("/api/templates/")) writes++;
  });
  try {
    const form = await openNew(page);
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await form.locator(".launch-composer-save-template").click();
    const panel = form.locator(".save-template-panel");
    const field = panel.locator(".save-template-name");
    await expect(panel).toBeVisible();
    await field.fill(unfinished);
    await expect(field).toBeFocused();
    await field.evaluate((node) => {
      const event = new KeyboardEvent("keydown", { key: "Enter", code: "Enter", isComposing: true, bubbles: true, cancelable: true });
      node.dispatchEvent(event);
    });
    await expect(panel).toBeVisible();
    await expect(field).toHaveValue(unfinished);
    expect(writes, "composition must not submit a template write").toBe(0);
    await expect(field).toBeEnabled();
    await field.fill(name);
    await field.press("Enter");
    await expect(page.locator('.templates-dialog[role="dialog"]')).toBeVisible();
    expect(writes).toBe(1);
    expect((await storedTemplate(request, name)).fields.agent).toBe("codex");
    expect(await storedTemplate(request, unfinished)).toBeUndefined();
  } finally {
    await deleteTemplate(request, name);
    await deleteTemplate(request, unfinished);
  }
});

/** Open New and return its form. */
async function openNew(page: Page) {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  return form;
}

/** Accept the `tl:` search result for `name`. */
async function applyTemplate(form: ReturnType<Page["locator"]>, name: string) {
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await search.fill(`tl:${name}`);
  await form.getByRole("option", { name: `Template: ${name}`, exact: true }).click();
}

/** A failed catalog must explain missing search results and recover without losing the launch draft. */
test("tl: search reports a failed template read and retries it", async ({ page, request }) => {
  const name = `e2e-launcher-list-retry-${Date.now()}`;
  await putTemplate(request, name, { kind: "agent", agent: "codex" });
  // Keep the simulated outage on this helm's build; an absent header would
  // also trigger the unrelated build-skew banner.
  const templatesReply = await request.get("/api/templates");
  expect(templatesReply.ok()).toBe(true);
  const helmBuild = templatesReply.headers()["x-farhelm-build"];
  expect(helmBuild).toBeTruthy();
  let failReads = true;
  let failedReads = 0;
  await page.route("**/api/templates", async (route) => {
    if (route.request().method() === "GET" && failReads) {
      failedReads++;
      await route.fulfill({ status: 503, contentType: "text/plain", headers: { "x-farhelm-build": helmBuild }, body: "temporary template list failure" });
    } else await route.continue();
  });
  try {
    const form = await openNew(page);
    await form.getByLabel("name (optional)").fill("preserved draft");
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    await search.fill("tl:");
    const error = form.locator(".launch-composer-templates-error");
    await expect(error).toContainText("couldn't load templates");
    await expect(error).toContainText("temporary template list failure");
    expect(failedReads).toBeGreaterThan(0);
    await form.getByRole("button", { name: "Codex", exact: true }).click();
    await expect(form.getByRole("button", { name: "Codex", exact: true })).toHaveAttribute("aria-pressed", "true");
    failReads = false;
    await error.getByRole("button", { name: "retry", exact: true }).click();
    await expect(error).toHaveCount(0);
    // Choice and Retry clicks dismiss the search list. Typing again opens
    // it within the same draft, so this checks the refreshed catalog.
    await search.fill(`tl:${name}`);
    await expect(form.getByRole("option", { name: `Template: ${name}`, exact: true })).toBeVisible();
    await expect(form.getByLabel("name (optional)")).toHaveValue("preserved draft");
  } finally {
    await page.unroute("**/api/templates");
    await deleteTemplate(request, name);
  }
});

/**
 * Open within the current app lifetime and establish the name-list premise.
 * Close/reopen tests need this path so navigation cannot erase the state whose
 * ownership they are checking.
 */
async function openTemplatesInPlace(page: Page) {
  await page.getByRole("button", { name: "templates", exact: true }).click();
  const dialog = page.locator('.templates-dialog[role="dialog"]');
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
  await expect(dialog.locator(".templates-retry")).toHaveCount(0);
  await expect(dialog.locator(".templates-count")).toHaveText(/\d+ templates?/);
  return dialog;
}

/** Start from the session list, then establish the editor's loaded-list premise. */
async function openTemplates(page: Page) {
  await page.goto("/");
  return openTemplatesInPlace(page);
}

/** Adding is an explicit presence edit; no hidden field controls count. */
async function addField(dialog: ReturnType<Page["locator"]>, name: string) {
  await dialog.locator(".templates-add").click();
  await dialog.locator(".templates-add-menu").getByRole("button", { name, exact: true }).click();
}

/** Fetch the stored fields so assertions distinguish a draft from an API write. */
async function storedTemplate(request: APIRequestContext, name: string) {
  const response = await request.get("/api/templates");
  expect(response.ok()).toBe(true);
  const body = await response.json();
  return body.templates.find((template: any) => template.name === name);
}

/** An approvals-only template deliberately leaves the agent unset. Saving
 * non-YOLO choices must preserve that omission, since the agent selected at
 * launch owns the eventual compatibility decision. */
test("an approvals-only template saves every offered approval choice", async ({ page, request }) => {
  const name = `partial-approvals-${Date.now()}`;
  try {
    const dialog = await openTemplates(page);
    await dialog.locator(".templates-new").click();
    await dialog.locator(".templates-name").fill(name);
    await addField(dialog, "approvals");
    const choices = dialog.getByLabel("approvals", { exact: true });
    for (const choice of ["guarded", "approve", "smart_approve", "chat", "yolo"]) {
      await choices.selectOption(choice);
      await expect(dialog.locator(".templates-save")).toBeEnabled();
      await dialog.locator(".templates-save").click();
      await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
      expect((await storedTemplate(request, name)).fields).toEqual({ kind: "agent", permissions: choice });
      await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    }
  } finally {
    await deleteTemplate(request, name);
  }
});

/** A supported permission remains an explicit stored edit even when it is
 * also the agent's current default. The editor must not mark it unoffered or
 * block saving merely to avoid a duplicate reset label. */
test("templates save explicit guarded and default-YOLO permissions", async ({ page, request }) => {
  const name = `explicit-permissions-${Date.now()}`;
  try {
    const dialog = await openTemplates(page);
    await dialog.locator(".templates-new").click();
    await dialog.locator(".templates-name").fill(name);
    await addField(dialog, "agent type");
    await dialog.getByLabel("agent type", { exact: true }).selectOption("codex");
    await addField(dialog, "approvals");
    for (const [agent, permission] of [["codex", "guarded"], ["omp", "yolo"], ["goose", "yolo"]]) {
      await dialog.getByLabel("agent type", { exact: true }).selectOption(agent);
      const choices = dialog.getByLabel("approvals", { exact: true });
      await expect(choices.locator(`option[value="${permission}"]`)).not.toHaveText(/not offered/);
      await choices.selectOption(permission);
      await expect(dialog.locator(".templates-save")).toBeEnabled();
      await dialog.locator(".templates-save").click();
      await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
      expect((await storedTemplate(request, name)).fields).toMatchObject({ kind: "agent", agent, permissions: permission });
      await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    }
  } finally {
    await deleteTemplate(request, name);
  }
});

/** Save's authoritative list read can lag the successful write. A synthetic
 * Delete bypasses rendered disabled controls to exercise the handler guard;
 * after the held read lands, Delete and Undo must use the newly saved fields. */
test("delete waits for the saved template list before offering undo", async ({ page, request }) => {
  const name = `saved-delete-${Date.now()}`;
  const gate = routeGate();
  let hold = false;
  let held = false;
  let deletes = 0;
  await putTemplate(request, name, { name: "old title" });
  await page.route("**/api/templates", async (route) => {
    if (hold && route.request().method() === "GET") {
      held = true;
      await gate.wait();
    }
    await route.fallback();
  });
  page.on("request", (r) => { if (r.method() === "DELETE" && r.url().endsWith(`/api/templates/${name}`)) deletes++; });
  try {
    const dialog = await openTemplates(page);
    await dialog.locator(".templates-row").filter({ has: page.getByText(name, { exact: true }) }).click();
    await dialog.getByLabel("session name", { exact: true }).fill("new title");
    hold = true;
    await dialog.locator(".templates-save").click();
    await expect.poll(() => held).toBe(true);
    await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "true");
    expect((await storedTemplate(request, name)).fields.name).toBe("new title");
    await dialog.locator(".templates-delete").dispatchEvent("click");
    // sleep-ok: observe the absence of a queued Delete while the saved list is deliberately held.
    await page.waitForTimeout(250);
    expect(deletes).toBe(0);
    await expect(dialog.locator(".templates-undo")).toHaveCount(0);
    hold = false;
    gate.release();
    await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    await dialog.locator(".templates-delete").click();
    await expect(dialog.locator(".templates-undo")).toBeVisible();
    expect(deletes).toBe(1);
    await dialog.locator(".templates-undo").click();
    await expect(dialog.locator(".templates-undo")).toHaveCount(0);
    expect((await storedTemplate(request, name)).fields.name).toBe("new title");
  } finally {
    gate.release();
    await page.unrouteAll({ behavior: "ignoreErrors" });
    await deleteTemplate(request, name);
  }
});

/** Destination type is a presentation choice over the existing stored schema.
 * Saving each type proves the editor does not confuse a repository with a
 * directory or silently change the template's other fields. */
test("template destination uses the shared folder and managed checkout control", async ({ page, request }) => {
  const name = `e2e-destination-${Date.now()}`;
  try {
    const dialog = await openTemplates(page);
    await dialog.locator(".templates-new").click();
    await dialog.locator(".templates-name").fill(name);
    await addField(dialog, "destination");
    const types = dialog.getByRole("group", { name: "destination type", exact: true });
    await expect(types.locator('[data-glyph="branch"]')).toHaveCount(1);
    await expect(types.locator('[data-glyph="folder"]')).toHaveCount(1);
    await expect(types.getByRole("button", { name: "folder", exact: true })).toHaveAttribute("aria-pressed", "true");
    await types.getByRole("button", { name: "managed checkout", exact: true }).click();
    await dialog.getByLabel("repository", { exact: true }).fill("acme/bar");
    await expect(dialog.locator(".templates-hint", { hasText: "Each session from this template" })).toContainText("moves to the trash");
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, name)).fields.destination).toEqual({ github: "acme/bar" });
    await types.getByRole("button", { name: "folder", exact: true }).click();
    await dialog.getByLabel("folder path", { exact: true }).fill("/tmp/project");
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, name)).fields.destination).toEqual({ folder: "/tmp/project" });
  } finally {
    await deleteTemplate(request, name);
  }
});

/** An unselected repository is search text, even when it parses as owner/repo.
 * Saving the other launch choices must omit it rather than create a template
 * that later fails to apply or silently accepts a destination never selected. */
for (const query of ["", "invalid!", "acme/bar"]) {
  test(`save as template omits an unselected repository ${JSON.stringify(query)}`, async ({ page, request }) => {
    const name = `e2e-unselected-${Date.now()}`;
    try {
      const form = await openNew(page);
      await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
      await form.getByRole("group", { name: "destination type", exact: true }).getByRole("button", { name: "managed checkout", exact: true }).click();
      await form.getByRole("combobox", { name: "repository", exact: true }).fill(query);
      await expect(form.locator(".launch-composer-checkout-preview")).toHaveCount(0);
      await expect(form.locator(".create-session-submit")).toBeDisabled();
      await form.locator(".launch-composer-save-template").click();
      const panel = form.locator(".save-template-panel");
      await expect(panel).toBeVisible();
      await expect(panel.locator('[data-field="destination"]')).toHaveCount(0);
      await panel.locator(".save-template-name").fill(name);
      await panel.locator(".save-template-save").click();
      await expect(page.locator('.templates-dialog[role="dialog"]')).toBeVisible();
      const stored = await storedTemplate(request, name);
      expect(stored.fields.agent).toBe("codex");
      expect(stored.fields.destination).toBeUndefined();
    } finally {
      await deleteTemplate(request, name);
    }
  });
}

/**
 * The selected row stays reachable, saves retain the editor, and rename writes
 * the new name before removing the old. Delete/undo must restore the actual
 * stored fields rather than an unsaved draft. This is the person's write path.
 */
test("the Templates dialog creates, edits, renames, deletes and undoes", async ({ page, request }) => {
  const name = `e2e-panel-${Date.now()}`;
  const renamed = `${name}-renamed`;
  try {
    const dialog = await openTemplates(page);
    await dialog.locator(".templates-new").click();
    await dialog.locator(".templates-name").fill(name);
    await addField(dialog, "agent type");
    await dialog.getByLabel("agent type", { exact: true }).selectOption("codex");
    await addField(dialog, "effort");
    await dialog.getByLabel("effort", { exact: true }).selectOption("high");
    await dialog.locator(".templates-save").click();
    const row = dialog.locator(".templates-row", { hasText: name });
    await expect(row).toBeVisible();
    await expect(row).toHaveAttribute("aria-pressed", "true");
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    // Leave the saved draft first: reselecting its current row is a no-op.
    await dialog.locator(".templates-new").click();
    await expect(row).toHaveAttribute("aria-pressed", "false");
    await row.click();
    await expect(dialog.getByLabel("agent type", { exact: true })).toHaveValue("codex");
    await expect(dialog.getByLabel("effort", { exact: true })).toHaveValue("high");
    await dialog.getByLabel("effort", { exact: true }).selectOption("low");
    await dialog.locator(".templates-name").fill(renamed);
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-row-name", { hasText: renamed })).toBeVisible();
    expect(await storedTemplate(request, name)).toBeUndefined();
    expect((await storedTemplate(request, renamed)).fields).toEqual({ kind: "agent", agent: "codex", effort: "low" });
    await dialog.locator(".templates-delete").click();
    await expect(dialog.locator(".templates-undo")).toBeVisible();
    expect(await storedTemplate(request, renamed)).toBeUndefined();
    await dialog.locator(".templates-undo").click();
    await expect(dialog.locator(".templates-undo")).toHaveCount(0);
    expect((await storedTemplate(request, renamed)).fields).toEqual({ kind: "agent", agent: "codex", effort: "low" });
  } finally {
    for (const value of [name, renamed]) await deleteTemplate(request, value);
  }
});

/**
 * A long summary cannot size the list wider than the dialog. Editing that row
 * must be reachable and collision refusals must preserve the other template,
 * including when another client takes a deleted template's name before undo.
 */
test("long summaries stay reachable and taken names refuse save and undo", async ({ page, request }) => {
  const name = `e2e-long-${Date.now()}`;
  const taken = `${name}-taken`;
  const fields = { kind: "agent", agent: "claude", model: "long-model-".repeat(70) };
  try {
    await putTemplate(request, name, fields);
    await putTemplate(request, taken, { name: "untouched" });
    const dialog = await openTemplates(page);
    const row = dialog.locator(".templates-row").filter({ has: page.getByText(name, { exact: true }) });
    await expect(row).toBeVisible();
    const bounds = await row.evaluate((node) => {
      const row = node.getBoundingClientRect();
      const dialog = node.closest(".templates-dialog")!.getBoundingClientRect();
      return { right: row.right, edge: dialog.right };
    });
    expect(bounds.right).toBeLessThanOrEqual(bounds.edge);
    await row.click();
    await expect(dialog.locator(".templates-model")).toHaveValue(fields.model);
    await dialog.locator(".templates-name").fill(taken);
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-error")).toContainText("already exists");
    expect((await storedTemplate(request, taken)).fields).toEqual({ name: "untouched" });
    await dialog.locator(".templates-name").fill(name);
    await dialog.locator(".templates-delete").click();
    await expect(dialog.locator(".templates-undo")).toBeVisible();
    expect(await storedTemplate(request, name)).toBeUndefined();
    await putTemplate(request, name, { name: "replacement" });
    await dialog.locator(".templates-undo").click();
    await expect(dialog.locator(".templates-error")).toContainText("already exists");
    expect((await storedTemplate(request, name)).fields).toEqual({ name: "replacement" });
  } finally {
    for (const value of [name, taken]) await deleteTemplate(request, value);
  }
});

/**
 * Every departure uses an inline guard. The pending target survives save, and
 * discarding before duplicate copies the stored version rather than the edits
 * the user just discarded. Escape also returns focus through the modal close.
 */
test("unsaved departures support keep editing, discard and save before leaving", async ({ page, request }) => {
  const name = `e2e-departure-${Date.now()}`;
  const other = `${name}-other`;
  const copy = `${name} copy`;
  try {
    await putTemplate(request, name, { kind: "agent", name: "stored" });
    await putTemplate(request, other, { kind: "command", yolo: false });
    const dialog = await openTemplates(page);
    const row = dialog.locator(".templates-row").filter({ has: page.getByText(name, { exact: true }) });
    await row.click();
    await dialog.locator(".templates-session-name").fill("edited");
    await dialog.locator(".templates-new").click();
    await expect(dialog.locator(".templates-departure")).toBeVisible();
    await dialog.locator(".templates-keep-editing").click();
    await expect(dialog.locator(".templates-session-name")).toHaveValue("edited");
    await dialog.locator(".templates-duplicate").click();
    await dialog.locator(".templates-discard").click();
    await expect(dialog.locator(".templates-name")).toHaveValue(copy);
    await expect(dialog.locator(".templates-session-name")).toHaveValue("stored");
    await expect(dialog.locator(".templates-unsaved")).toBeVisible();
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    await dialog.locator(".templates-session-name").fill("saved-before-leaving");
    await dialog.locator(".templates-row").filter({ has: page.getByText(other, { exact: true }) }).click();
    await dialog.locator(".templates-save-leave").click();
    await expect(dialog.locator(".templates-name")).toHaveValue(other);
    expect((await storedTemplate(request, copy)).fields.name).toBe("saved-before-leaving");
    await dialog.getByLabel("approvals", { exact: true }).selectOption("true");
    await dialog.locator(".templates-close").click();
    await dialog.locator(".templates-keep-editing").click();
    await expect(dialog.locator(".templates-departure")).toHaveCount(0);
    await dialog.locator(".templates-name").focus();
    await expect(dialog.locator(".templates-name")).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(dialog.locator(".templates-departure")).toBeVisible();
    await dialog.locator(".templates-discard").click();
    await expect(dialog).toHaveCount(0);
    await expect(page.locator(".templates-button")).toBeFocused();
    expect((await storedTemplate(request, other)).fields.yolo).toBe(false);
  } finally {
    for (const value of [name, other, copy]) await deleteTemplate(request, value);
  }
});

/**
 * Legacy inference is an unsaved editor interpretation, not a migration on
 * read. Unsupported approvals stay visible after agent changes, while custom
 * model text remains saveable. Mobile Back must use the same draft guard.
 */
test("legacy switches and agent choices remain explicit at phone width", async ({ page, request }) => {
  const name = `e2e-legacy-${Date.now()}`;
  try {
    await putTemplate(request, name, { agent: "goose", permissions: "smart_approve", model: "future custom model" });
    await page.setViewportSize({ width: 390, height: 844 });
    const dialog = await openTemplates(page);
    await dialog.locator(".templates-row").filter({ has: page.getByText(name, { exact: true }) }).click();
    await expect(dialog.locator(".templates-sidebar")).toBeHidden();
    await expect(dialog.locator(".templates-inferred")).toBeVisible();
    await expect(dialog.locator(".templates-unsaved")).toBeVisible();
    expect((await storedTemplate(request, name)).fields.kind).toBeUndefined();
    // Opening an existing template must initialize each native select from
    // its saved value, even when that value is not the first offered option.
    await expect(dialog.getByLabel("agent type", { exact: true })).toHaveValue("goose");
    await expect(dialog.getByLabel("approvals", { exact: true })).toHaveValue("smart_approve");
    await dialog.getByLabel("agent type", { exact: true }).selectOption("claude");
    await expect(dialog.locator('.templates-set-field[data-field="permissions"] .templates-error')).toContainText("does not offer");
    await dialog.locator(".templates-save").click();
    const refusal = dialog.locator('p.templates-error[role="status"]');
    await expect(refusal).toContainText("approvals");
    // A refusal belongs below the complete editor. A height-limited flex
    // dialog must scroll its content, not shrink the editor and overlay it.
    await expect.poll(async () => refusal.evaluate((element) => {
      const footer = element.closest(".templates-dialog")!.querySelector(".templates-editor-footer")!;
      return element.getBoundingClientRect().top >= footer.getBoundingClientRect().bottom;
    })).toBe(true);
    expect((await storedTemplate(request, name)).fields.agent).toBe("goose");
    await dialog.getByLabel("approvals", { exact: true }).selectOption("default");
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, name)).fields).toEqual({ kind: "agent", agent: "claude", permissions: null, model: "future custom model" });
    await dialog.locator(".templates-model").fill("another model");
    await dialog.locator(".templates-back").click();
    await expect(dialog.locator(".templates-departure")).toBeVisible();
    await dialog.locator(".templates-discard").click();
    await expect(dialog.locator(".templates-sidebar")).toBeVisible();
    await expect(dialog.locator(".templates-editor")).toBeHidden();
  } finally {
    await deleteTemplate(request, name);
  }
});

/**
 * Reselecting the open row is not a departure. Its list snapshot may predate
 * the draft or a save, so reopening it could silently undo an edit or rename.
 */
test("reselecting the current template preserves edits and renames", async ({ page, request }) => {
  const name = `e2e-reselect-${Date.now()}`;
  const renamed = `${name}-renamed`;
  try {
    await putTemplate(request, name, { kind: "agent", name: "stored" });
    const dialog = await openTemplates(page);
    const row = dialog.locator(".templates-row").filter({ has: page.getByText(name, { exact: true }) });
    await row.click();
    await expect(dialog.locator(".templates-session-name")).toHaveValue("stored");
    await dialog.locator(".templates-session-name").fill("first edit");
    await row.click();
    await expect(dialog.locator(".templates-departure")).toHaveCount(0);
    await expect(dialog.locator(".templates-session-name")).toHaveValue("first edit");
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, name)).fields.name).toBe("first edit");
    await dialog.locator(".templates-name").fill(renamed);
    await row.click();
    await expect(dialog.locator(".templates-departure")).toHaveCount(0);
    await expect(dialog.locator(".templates-name")).toHaveValue(renamed);
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-row-name", { hasText: renamed })).toBeVisible();
    expect(await storedTemplate(request, name)).toBeUndefined();
    expect((await storedTemplate(request, renamed)).fields).toEqual({ kind: "agent", name: "first edit" });
  } finally {
    for (const value of [name, renamed]) await deleteTemplate(request, value);
  }
});

/**
 * A name-list failure must preserve a phone draft while the connection recovers.
 * The hidden list cannot be the only place to retry: Back would require either
 * a save with an unknown name premise or discarding the person's work.
 */
test("a phone draft can retry a failed template list without discarding", async ({ page, request }) => {
  const name = `e2e-list-retry-${Date.now()}`;
  let failReads = true;
  let failedReads = 0;
  await page.route("**/api/templates", async (route) => {
    if (route.request().method() === "GET" && failReads) {
      failedReads += 1;
      await route.fulfill({ status: 503, body: "temporary template list failure" });
    } else {
      await route.continue();
    }
  });
  try {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.locator(".templates-dialog");
    await expect(dialog.locator(".templates-retry")).toBeVisible();
    expect(failedReads).toBeGreaterThan(0);
    await expect(dialog.locator(".templates-count")).toHaveCount(0);
    await dialog.locator(".templates-new").click();
    await expect(dialog.locator(".templates-sidebar")).toBeHidden();
    await dialog.locator(".templates-name").fill(name);
    await addField(dialog, "session name");
    await dialog.locator(".templates-session-name").fill("kept draft");
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-error").last()).toContainText("template list could not be loaded");
    failReads = false;
    await expect(dialog.locator(".templates-retry")).toBeVisible();
    await dialog.locator(".templates-retry").click();
    await expect(dialog.locator(".templates-retry")).toHaveCount(0);
    await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    await expect(dialog.locator(".templates-name")).toHaveValue(name);
    await expect(dialog.locator(".templates-session-name")).toHaveValue("kept draft");
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, name)).fields).toEqual({ kind: "agent", name: "kept draft" });
  } finally {
    await page.unroute("**/api/templates");
    await deleteTemplate(request, name);
  }
});

/**
 * Catalog choices remain available beside arbitrary model text. A tall phone
 * editor must reveal and focus the unsaved prompt itself, without a test's
 * scroll action masking an unreachable prompt. Escape first dismisses add-field.
 */
test("model suggestions and phone departure controls remain reachable", async ({ page, request }) => {
  const name = `e2e-suggestions-${Date.now()}`;
  try {
    await putTemplate(request, name, { kind: "agent", agent: "claude", model: "custom model", effort: null, permissions: null });
    await page.setViewportSize({ width: 390, height: 844 });
    const dialog = await openTemplates(page);
    await dialog.locator(".templates-row").filter({ has: page.getByText(name, { exact: true }) }).click();
    await expect(dialog.locator(".templates-name")).toBeFocused();
    await expect(dialog.locator(".templates-model")).toHaveValue("custom model");
    const suggestion = dialog.locator(".templates-suggestion").first();
    await expect(suggestion).toBeVisible();
    const model = await suggestion.innerText();
    expect(model.length).toBeGreaterThan(0);
    await suggestion.click();
    await expect(dialog.locator(".templates-model")).toHaveValue(model);
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, name)).fields.model).toBe(model);
    await dialog.locator(".templates-model").fill("another custom model");
    await expect(dialog.locator(".templates-suggestion").first()).toBeVisible();
    await dialog.locator(".templates-add").click();
    await expect(dialog.locator(".templates-add-menu")).toBeVisible();
    await expect(dialog.locator(".templates-add")).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(dialog.locator(".templates-add-menu")).toHaveCount(0);
    await expect(dialog.locator(".templates-departure")).toHaveCount(0);
    await dialog.locator(".templates-close").click();
    const prompt = dialog.locator(".templates-departure");
    await expect(prompt).toBeInViewport();
    await expect(dialog.locator(".templates-save-leave")).toBeFocused();
    await dialog.locator(".templates-keep-editing").click();
    await expect(prompt).toHaveCount(0);
    await expect(dialog.locator(".templates-name")).toBeFocused();
    await expect(dialog.locator(".templates-model")).toHaveValue("another custom model");
  } finally {
    await deleteTemplate(request, name);
  }
});

/**
 * Undo belongs to the latest deletion and the mounted dialog. Advancing the
 * browser's clock crosses the declared expiry boundaries without a real-time
 * readiness sleep; each notice and completed list refresh is established first.
 */
test("undo expires independently of earlier deletions and ends on close", async ({ page, request }) => {
  const first = `e2e-undo-window-${Date.now()}`;
  const second = `${first}-second`;
  try {
    for (const name of [first, second]) await putTemplate(request, name, { kind: "agent", name });
    let dialog = await openTemplates(page);
    await page.clock.install();
    // install alone still advances at wall-clock speed. Pause before deleting
    // so close/reopen cannot pass merely because the ten-second window elapsed.
    // This test advances expiry explicitly and does not assert focus/rAF work.
    await page.clock.pauseAt(new Date(Date.now() + 5_000));
    const remove = async (name: string) => {
      await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
      await dialog.locator(".templates-row").filter({ has: page.getByText(name, { exact: true }) }).click();
      await expect(dialog.locator(".templates-name")).toHaveValue(name);
      await dialog.locator(".templates-delete").click();
      await expect(dialog.locator(".templates-undo-notice")).toContainText(`Deleted ${name}.`);
      expect(await storedTemplate(request, name)).toBeUndefined();
    };
    await remove(first);
    await page.clock.fastForward(9_000);
    await remove(second);
    await page.clock.fastForward(1_001);
    await expect(dialog.locator(".templates-undo-notice")).toContainText(`Deleted ${second}.`);
    await page.clock.fastForward(9_000);
    await expect(dialog.locator(".templates-undo")).toHaveCount(0);
    await dialog.locator(".templates-close").click();
    await expect(dialog).toHaveCount(0);
    await putTemplate(request, second, { kind: "agent", name: "another deletion" });
    dialog = await openTemplatesInPlace(page);
    await remove(second);
    await dialog.locator(".templates-close").click();
    await expect(dialog).toHaveCount(0);
    // Reopen without navigation while this deletion's undo window is live:
    // only the dialog's unmount may have cleared its notice.
    dialog = await openTemplatesInPlace(page);
    await expect(dialog.locator(".templates-undo")).toHaveCount(0);
    expect(await storedTemplate(request, second)).toBeUndefined();
  } finally {
    for (const name of [first, second]) await deleteTemplate(request, name);
  }
});

/**
 * Spec: `tl:name` applies a template's edits through the launcher's own
 * controls, and a second template stacks on the first, the later winning
 * where they overlap; a template whose field does not apply is refused,
 * naming the field, with nothing from it applied.
 *
 * Why: "applying a template is exactly making its edits by hand" and
 * stacking are SPEC.md's definition of the feature, and the all-or-nothing
 * refusal is what keeps a half-applied template from launching something
 * nobody chose.
 */
test("tl: applies templates, stacks them, and refuses one that does not apply", async ({ page, request }) => {
  const stamp = Date.now();
  const codex = `e2e-codex-${stamp}`;
  const low = `e2e-low-${stamp}`;
  const command = `e2e-command-${stamp}`;
  try {
    await putTemplate(request, codex, { agent: "codex", effort: "high" });
    await putTemplate(request, low, { effort: "low", name: "from-template" });
    // Its agent type (step 2) would apply; its command (step 4) does not, so
    // seeing the agent type unchanged is what shows nothing was applied.
    await putTemplate(request, command, { agent: "claude", command: "sleep 300" });
    const form = await openNew(page);
    const harness = form.locator(".launch-composer-harness-choice");
    const effort = form.locator(".launch-composer-effort-choice");

    await applyTemplate(form, codex);
    await expect(harness.getByRole("button", { name: "Codex", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(effort.getByRole("button", { name: "high", exact: true })).toHaveAttribute("aria-pressed", "true");

    await applyTemplate(form, low);
    await expect(effort.getByRole("button", { name: "low", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(harness.getByRole("button", { name: "Codex", exact: true }), "left out, kept").toHaveAttribute("aria-pressed", "true");
    await expect(form.getByLabel("name (optional)")).toHaveValue("from-template");

    // A command field on the agent launch kind applies to nothing here.
    await applyTemplate(form, command);
    await expect(form.locator(".launch-composer-template-error")).toContainText("its command applies to a command launch");
    await expect(form.getByRole("tab", { name: "agent", exact: true })).toHaveAttribute("aria-selected", "true");
    await expect(harness.getByRole("button", { name: "Codex", exact: true }), "nothing from it applied").toHaveAttribute("aria-pressed", "true");
    await expect(effort.getByRole("button", { name: "low", exact: true })).toHaveAttribute("aria-pressed", "true");
  } finally {
    for (const name of [codex, low, command]) await deleteTemplate(request, name);
  }
});

/**
 * Spec: a template can switch the launcher to the command launch kind and
 * fill its command, YOLO answer, declared agent type and resume command, and
 * set the folder; a later template resetting a choice to its default clears
 * it.
 *
 * Why: the command fields and the destination have no search action of
 * their own, so they are the part of application that cannot borrow the
 * launcher's existing behavior and needs its own observation; a reset is
 * how a template undoes a choice an earlier one made.
 */
test("a template fills the command tab and the folder, and a reset clears a choice", async ({ page, request }) => {
  const stamp = Date.now();
  const command = `e2e-cmd-${stamp}`;
  const effort = `e2e-effort-${stamp}`;
  const reset = `e2e-reset-${stamp}`;
  try {
    await putTemplate(request, command, {
      kind: "command",
      agent: "claude",
      command: "claude {farhelm_args}",
      yolo: false,
      resume_command: "claude --resume {conversation} {farhelm_args}",
      destination: { folder: "/tmp" },
    });
    await putTemplate(request, effort, { agent: "codex", effort: "high" });
    await putTemplate(request, reset, { effort: null });
    const form = await openNew(page);

    await applyTemplate(form, command);
    await expect(form.getByRole("tab", { name: "command", exact: true })).toHaveAttribute("aria-selected", "true");
    await expect(form.getByLabel("agent command")).toHaveValue("claude {farhelm_args}");
    await expect(
      form.getByRole("group", { name: "runs in YOLO mode" }).getByLabel("no", { exact: true }),
    ).toBeChecked();
    await expect(form.locator(".launch-command-agent")).toHaveValue("claude");
    await expect(form.locator(".launch-command-resume-toggle")).toBeChecked();
    await expect(form.locator(".launch-command-resume")).toHaveValue("claude --resume {conversation} {farhelm_args}");
    await expect(form.getByLabel("folder", { exact: true })).toHaveValue("/tmp");

    await form.getByRole("tab", { name: "agent", exact: true }).click();
    await applyTemplate(form, effort);
    const efforts = form.locator(".launch-composer-effort-choice");
    await expect(efforts.getByRole("button", { name: "high", exact: true })).toHaveAttribute("aria-pressed", "true");
    await applyTemplate(form, reset);
    await expect(efforts.getByRole("button", { name: "default", exact: true })).toHaveAttribute("aria-pressed", "true");
  } finally {
    for (const name of [command, effort, reset]) await deleteTemplate(request, name);
  }
});

/**
 * Spec: Replace with refuses a template that sets a host, because that
 * dialog keeps the session's host.
 *
 * Why: SPEC.md names this as the case of a template setting a field the
 * dialog holds fixed; applying it would either move the replacement to
 * another machine or silently drop part of the template.
 */
test("Replace with refuses a template that sets a host", async ({ page, request }) => {
  const name = `e2e-host-${Date.now()}`;
  const hosts = await listHosts(request);
  const identity = hosts.find((host) => host.kind === "local")?.identity;
  expect(identity, "premise: the local host has a recorded install identity").toBeTruthy();
  const source = await createSession(request, { title: `templates-replace-${Date.now()}` });
  try {
    await putTemplate(request, name, { host: identity });
    await page.goto("/");
    const row = page.locator(`.session-row[data-session-id="${source.id}"]`);
    await expect(row).toBeVisible({ timeout: 20_000 });
    await openRowMenu(row);
    await row.locator(".session-row-replace-with").click();
    const form = page.locator('.create-session-form[role="dialog"]');
    await expect(form).toBeVisible();
    await expect(form.locator(".launch-composer-save-template")).toHaveCount(0);
    await applyTemplate(form, name);
    await expect(form.locator(".launch-composer-template-error")).toContainText("this dialog keeps the session's host");
  } finally {
    await deleteTemplate(request, name);
    await cleanupSession(request, source.id);
  }
});

/**
 * A saved launcher snapshot contains only checked edits and opens directly in
 * the editor. Enter belongs to the inline name field, never to session launch.
 * Count create requests so an otherwise successful template save cannot hide a
 * second operation; read stored fields rather than trusting checkbox visuals.
 * Each checkbox also belongs beside its text, including the text's first line
 * at phone width: the launcher's generic stacked labels must not split these
 * checklist rows into a control line and a separate caption.
 */
for (const kind of ["agent", "command"] as const) {
  test(`save as template captures the ${kind} setup without launching`, async ({ page, request }, info) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    const name = `e2e-launcher-save-${kind}-${Date.now()}`;
    const creates: string[] = [];
    page.on("request", (r) => {
      if (r.method() === "POST" && /\/api\/sessions(?:\?|$)/.test(new URL(r.url()).pathname)) creates.push(r.url());
    });
    try {
      const form = await openNew(page);
      if (kind === "agent") {
        await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
      } else {
        await form.getByRole("tab", { name: "command", exact: true }).click();
        await form.getByLabel("agent command").fill("printf hello");
        await form.getByRole("group", { name: "runs in YOLO mode" }).getByLabel("no", { exact: true }).check();
      }
      await form.getByLabel("folder", { exact: true }).fill("/tmp");
      await form.locator(".launch-composer-save-template").click();
      const panel = form.locator(".save-template-panel");
      await expect(panel).toBeVisible();
      await expect(panel.locator('[data-field="destination"]')).toBeChecked();
      if (kind === "agent") await expect(panel.locator('[data-field="agent"]')).toBeChecked();
      else await expect(panel.locator('[data-field="command"]')).toBeChecked();
      const choices = panel.locator(".save-template-choice");
      expect(await choices.count(), "the staged setup offers a checklist to measure").toBeGreaterThan(0);
      await page.evaluate(() => document.fonts.ready);
      for (const width of [1280, 390]) {
        await page.setViewportSize({ width, height: 800 });
        // Keep the before image even when the geometry assertion rejects the
        // current CSS; a text label may wrap, but its box stays on the row.
        await page.screenshot({ path: info.outputPath(`save-template-${kind}-${width}.png`) });
        await expect.poll(() => choices.evaluateAll((labels) => labels.every((label) => {
          const input = label.querySelector("input")!.getBoundingClientRect();
          const range = document.createRange();
          range.selectNodeContents(label.querySelector("span")!);
          const firstLine = Array.from(range.getClientRects()).find((rect) => rect.width > 0)!;
          const center = input.top + input.height / 2;
          return center >= firstLine.top && center <= firstLine.bottom && input.right <= firstLine.left;
        })), { message: "every template checkbox shares its label's first text line" }).toBe(true);
      }
      await page.setViewportSize({ width: 1280, height: 800 });
      await panel.locator('[data-field="destination"]').uncheck();
      await panel.locator(".save-template-name").fill(name);
      await expect(panel.locator(".save-template-name")).toBeFocused();
      await panel.locator(".save-template-name").press("Enter");
      const editor = page.locator('.templates-dialog[role="dialog"]');
      await expect(editor).toBeVisible();
      await expect(form).toHaveCount(0);
      await expect(editor.locator(".templates-name")).toHaveValue(name);
      await expect(editor.locator(".templates-unsaved")).toHaveCount(0);
      const stored = await storedTemplate(request, name);
      expect(stored.fields.kind).toBe(kind);
      expect(stored.fields.destination).toBeUndefined();
      if (kind === "agent") expect(stored.fields.agent).toBe("codex");
      else expect(stored.fields).toEqual({ kind: "command", command: "printf hello", yolo: false });
      expect(creates, "saving a template sends no session create").toEqual([]);
    } finally {
      await deleteTemplate(request, name);
    }
  });
}

/**
 * A collision is a refusal, not a replacement. Escape dismisses only the
 * inline panel and leaves the setup editable, including after a failed save.
 */
test("launcher template save refuses a taken name and Escape keeps the launcher", async ({ page, request }) => {
  const name = `e2e-launcher-taken-${Date.now()}`;
  const fields = { kind: "command", command: "printf original", yolo: false };
  try {
    await putTemplate(request, name, fields);
    const form = await openNew(page);
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await form.locator(".launch-composer-save-template").click();
    const panel = form.locator(".save-template-panel");
    await panel.locator(".save-template-name").fill(name);
    await panel.locator(".save-template-save").click();
    await expect(panel.getByRole("alert")).toContainText("already exists");
    expect((await storedTemplate(request, name)).fields).toEqual(fields);
    await panel.locator(".save-template-name").focus();
    await expect(panel.locator(".save-template-name")).toBeFocused();
    await panel.locator(".save-template-name").press("Escape");
    await expect(panel).toHaveCount(0);
    await expect(form).toBeVisible();
    await expect(form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true })).toHaveAttribute("aria-pressed", "true");
  } finally {
    await deleteTemplate(request, name);
  }
});
