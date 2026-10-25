/**
 * Launch templates in the browser (SPEC.md, Launch templates): the Templates
 * panel beside New stores them, and `tl:name` in the launcher applies one as
 * the edits it contains, all or nothing.
 *
 * Templates are helm-wide and shared by every spec on the stack, so each
 * test names its own with a timestamp and deletes them afterwards.
 */
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { type APIRequestContext, type Page } from "@playwright/test";
import { cleanupSession, createSession, listHosts, openRowMenu, patchPreferences, pinAutoSelect, readPreferences } from "./helpers/fleet";
import { routeGate } from "./helpers/route-gate";
import { requireHelmBuild } from "./helpers/helm-build";

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

/** Older stored commands and session names must reach save validation unchanged. The current
 * API deliberately refuses these values, so the existing list interception
 * supplies an old-rules row; any attempted PUT is counted and refused.
 */
for (const field of ["command", "resume_command", "name"] as const) {
  test(`older template ${field} stays read-only without bypassing save refusal`, async ({ page }) => {
    const name = `legacy-command-${field}-${Date.now()}`;
    const fields = { kind: "command", yolo: false, command: "echo safe", resume_command: "echo resume", [field]: "echo 'first\r\nsecond'" };
    let writes = 0;
    await page.route("**/api/templates", async route => {
      const response = await route.fetch();
      const body = await response.json();
      await route.fulfill({ response, json: { ...body, templates: [...body.templates, { name, fields }] } });
    });
    await page.route("**/api/templates/*", async route => {
      if (route.request().method() === "PUT") writes++;
      await route.fulfill({ status: 400, body: "old template must not be written" });
    });
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.locator(".templates-dialog");
    await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    await dialog.locator(".templates-row-name").getByText(name, { exact: true }).click();
    const command = dialog.locator(field === "name" ? ".templates-session-name" : field === "command" ? ".templates-command" : ".templates-resume");
    const other = dialog.locator(field === "command" ? ".templates-resume" : ".templates-command");
    await expect(command).toHaveJSProperty("readOnly", true);
    await expect(other).toBeEditable();
    await expect(dialog.locator(".templates-value-readonly")).toContainText("Remove this field with ✕ and add it again");
    await expect(command).toHaveValue("echo 'first<U+000D><U+000A>second'");
    const rendered = await command.inputValue();
    await command.focus();
    await expect(command).toBeFocused();
    await page.keyboard.press("End");
    await page.keyboard.type("changed");
    await expect(command).toHaveValue(rendered);
    // Saving an unrelated edit must still validate the untouched raw value.
    await other.fill("echo updated");
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-error").last()).toContainText("control or invisible formatting characters");
    expect(writes, "the untouched raw field must still fail local shape validation").toBe(0);
  });
}

/** A catalog name must expose formatting without turning the display spelling
 * into a rename. An unrelated save preserves the API seed, including an emoji
 * joiner; explicit replacement restores literal editing without decoding escapes.
 */
test("template name escapes formatting until explicitly replaced", async ({ page, request }) => {
  const prefix = `unsafe-name-${Date.now()}`;
  const name = `${prefix}\u200b\u202e👩‍💻`;
  const shown = `${prefix}<U+200B><U+202E>👩<U+200D>💻`;
  const replacement = `${prefix}-literal<U+200B>`;
  const fields = { kind: "agent", agent: "claude", name: "before" };
  let bodyFailed = false;
  try {
    await putTemplate(request, name, fields);
    expect((await storedTemplate(request, name)).fields).toEqual(fields);
    const dialog = await openTemplates(page);
    await dialog.locator(".templates-row-name").getByText(shown, { exact: true }).click();
    const input = dialog.locator(".templates-name");
    await expect(input).toHaveValue(shown);
    await expect(input).toHaveJSProperty("readOnly", true);
    await input.focus();
    await expect(input).toBeFocused();
    await page.keyboard.type("changed");
    await expect(input).toHaveValue(shown);
    await dialog.locator(".templates-session-name").fill("after");
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, name)).fields).toEqual({ ...fields, name: "after" });
    await dialog.locator(".templates-replace-name").click();
    await expect(input).toBeEditable();
    await expect(input).toBeFocused();
    await input.fill(replacement);
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, replacement)).fields).toEqual({ ...fields, name: "after" });
    expect(await storedTemplate(request, name)).toBeUndefined();
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["original template", () => deleteTemplate(request, name)],
      ["replacement template", () => deleteTemplate(request, replacement)],
    ]);
  }
});

/** Host-derived model and folder values must be visibly escaped without
 * storing the display spelling. A real API seed and later reads distinguish
 * raw-byte preservation from an input that silently rewrites its own display.
 * Removing and re-adding each field must restore ordinary editing.
 */
test("unsafe template model and folder stay raw until explicitly replaced", async ({ page, request }) => {
  const name = `unsafe-values-${Date.now()}`;
  const fields = { kind: "agent", agent: "claude", model: "model\u200b", destination: { folder: "/tmp/\u202eproject\r\nnext" }, name: "before" };
  const suggested = "suggested\u200b";
  await page.route("**/api/launch-catalog", async route => {
    const response = await route.fetch();
    const catalog = await response.json();
    const seed = catalog.find((model: any) => model.harness === "claude");
    expect(seed, "the real catalog must offer a Claude model for this fixture").toBeTruthy();
    await route.fulfill({ response, json: [...catalog, { ...seed, id: suggested }] });
  });
  try {
    await putTemplate(request, name, fields);
    expect((await storedTemplate(request, name)).fields).toEqual(fields);
    const dialog = await openTemplates(page);
    await dialog.locator(".templates-row-name").getByText(name, { exact: true }).click();
    await expect(dialog.locator(".templates-model")).toHaveValue("model<U+200B>");
    await expect(dialog.locator(".templates-destination")).toHaveValue("/tmp/<U+202E>project<U+000D><U+000A>next");
    for (const field of ["model", "destination"]) {
      const input = dialog.locator(`.templates-${field}`);
      await expect(input).toHaveJSProperty("readOnly", true);
      await expect(dialog.locator(`.templates-set-field[data-field="${field}"] .templates-value-readonly`)).toContainText("Remove this field with ✕ and add it again");
      await input.focus();
      await expect(input).toBeFocused();
      await page.keyboard.press("End");
      await page.keyboard.type("changed");
    }
    await dialog.locator(".templates-session-name").fill("after");
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, name)).fields).toEqual({ ...fields, name: "after" });
    for (const field of ["model", "destination"]) {
      await dialog.getByRole("button", { name: `remove ${field}`, exact: true }).click();
    }
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, name)).fields).toEqual({ kind: "agent", agent: "claude", name: "after" });
    for (const [field, value] of [["model", "replacement model"], ["destination", "/tmp/replacement"]]) {
      await addField(dialog, field);
      const input = dialog.locator(`.templates-${field}`);
      await expect(input).toBeEditable();
      await input.fill(value);
    }
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, name)).fields).toEqual({ kind: "agent", agent: "claude", name: "after", model: "replacement model", destination: { folder: "/tmp/replacement" } });
    const suggestion = dialog.getByRole("button", { name: "suggested<U+200B>", exact: true });
    await expect(suggestion).toBeVisible();
    await expect(suggestion).toHaveAttribute("data-tooltip", "set model to suggested<U+200B>");
    await suggestion.click();
    await expect(dialog.locator(".templates-model")).toHaveValue("suggested<U+200B>");
    await expect(dialog.locator(".templates-model")).toHaveJSProperty("readOnly", true);
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, name)).fields.model).toBe(suggested);
  } finally {
    await deleteTemplate(request, name);
  }
});

/** Old-rules templates cannot be restored through today's save path. Supply
 * them at the list API boundary and prove the warning precedes the DELETE;
 * a completed reload, rather than a delay, establishes the no-Undo boundary.
 */
for (const [field, value] of [
  ["name", "joined 👩‍💻"],
  ["command", "echo 'first\nsecond'"],
  ["resume_command", "echo 'first\rsecond'"],
] as const) {
  test(`older template ${field} deletion warns before permanently deleting`, async ({ page }) => {
    const name = `legacy-delete-${field}-${Date.now()}`;
    const fields = { kind: "command", yolo: false, command: "echo safe", [field]: value };
    let deleted = false;
    let deletes = 0;
    let writes = 0;
    await page.route("**/api/templates", async route => {
      const response = await route.fetch();
      const body = await response.json();
      await route.fulfill({ response, json: { ...body, templates: [...body.templates, ...deleted ? [] : [{ name, fields }]] } });
    });
    await page.route("**/api/templates/*", async route => {
      if (route.request().method() === "DELETE") {
        expect(new URL(route.request().url()).pathname).toBe(`/api/templates/${name}`);
        deletes++;
        deleted = true;
        await route.fulfill({ status: 204 });
      } else {
        if (route.request().method() === "PUT") writes++;
        await route.fulfill({ status: 400, body: "old template must not be written" });
      }
    });
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.locator(".templates-dialog");
    await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    const row = dialog.locator(".templates-row").filter({ has: page.getByText(name, { exact: true }) });
    await row.click();
    await expect(row).toHaveAttribute("aria-pressed", "true");
    // Repairing the draft cannot make the stored snapshot restorable.
    if (field === "name") {
      await expect(dialog.locator(".templates-session-name")).toHaveJSProperty("readOnly", true);
      await dialog.getByRole("button", { name: "remove session name", exact: true }).click();
      await addField(dialog, "session name");
      const input = dialog.locator(".templates-session-name");
      await expect(input).toBeEditable();
      await input.fill("repaired draft");
      await expect(input).toHaveValue("repaired draft");
    }
    const remove = dialog.locator(".templates-delete");
    await expect(remove).toHaveText("delete permanently");
    await expect(remove).toHaveAttribute("data-tooltip", /current save rules.*undo is not available/);
    expect(deletes, "the permanent warning must be visible before any delete").toBe(0);
    await remove.click();
    await expect(row).toHaveCount(0);
    await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    await expect(dialog.locator(".templates-undo")).toHaveCount(0);
    expect(deletes).toBe(1);
    expect(writes, "permanent deletion must not attempt restoration").toBe(0);
  });
}

/** Keys move between saved names independently of the bolt. Keyboard Escape
 * closes only the nested menu; rename must retain the key, while delete/Undo
 * retires it permanently. The popup's arrows must still move focus while the
 * surrounding dialog isolates the rest of the page. Partial templates retain
 * the same complete key menu.
 * An external move made while this dialog stays open must survive its next edit.
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
    await expect(keyButton(partial)).toBeEnabled();
    await expect(keyButton(a)).toHaveText("Ctrl+Shift+ –");
    await expect(keyButton(a)).toHaveAttribute("data-tooltip", /right away.*Only one template/);
    await expect(dialog.getByRole("button", { name: `quick launch ${a}`, exact: true }))
      .toHaveAttribute("data-tooltip", /without opening New.*Only one template/);
    await keyButton(a).focus();
    await expect(keyButton(a)).toBeFocused();
    await page.keyboard.press("ArrowDown");
    const menu = dialog.getByRole("menu", { name: `number key for ${a}`, exact: true });
    await expect(menu.getByRole("menuitem", { name: "no shortcut", exact: true })).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(menu.getByRole("menuitem", { name: "Ctrl+Shift+1", exact: true })).toBeFocused();
    await page.keyboard.press("ArrowUp");
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
    await patchPreferences(request, { template_shortcuts: { "1": a }, quick_launch_template: a });
    expect((await readPreferences(request)).template_shortcuts).toEqual({ "1": a });
    await expect(keyButton(a)).toHaveText("Ctrl+Shift+ –");
    await keyButton(renamed).click();
    await dialog.getByRole("menu", { name: `number key for ${renamed}`, exact: true })
      .getByRole("menuitem", { name: "Ctrl+Shift+2", exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).template_shortcuts).toEqual({ "1": a, "2": renamed });
    await patchPreferences(request, { quick_launch_template: renamed });
    expect((await readPreferences(request)).quick_launch_template).toBe(renamed);
    const bolt = dialog.getByRole("button", { name: `quick launch ${renamed}`, exact: true });
    await expect(bolt).toHaveAttribute("aria-pressed", "false");
    // The stale unlit bolt asks to mark a template an agent has already marked: the mark must survive.
    await bolt.click();
    await expect(bolt).toHaveAttribute("aria-pressed", "true");
    expect((await readPreferences(request)).quick_launch_template).toBe(renamed);
    await dialog.locator(".templates-close").click();
    await patchPreferences(request, { template_shortcuts: { "3": partial } });
    await page.reload();
    await page.locator(".templates-button").click();
    await expect(keyButton(partial)).toBeEnabled();
    await keyButton(partial).click();
    const remove = dialog.getByRole("menu", { name: `number key for ${partial}`, exact: true });
    await expect(remove.getByRole("menuitem")).toHaveCount(11);
    await remove.getByRole("menuitem", { name: "no shortcut", exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).template_shortcuts ?? {}).toEqual({});
    await expect(keyButton(partial)).toBeEnabled();
  } finally {
    await patchPreferences(request, { template_shortcuts: null, quick_launch_template: null });
    for (const name of [a, b, partial, renamed]) await deleteTemplate(request, name);
  }
});

/** A refusal and a lost reply share uncertainty wording. Only a fresh helm read
 * decides which keys to show: a committed PUT whose reply was lost must appear,
 * while an unsuccessful re-read discards the optimistic choice and says the
 * current keys are unknown. A later edit never replays the failed choice over
 * an agent's new binding (SPEC.md, "One GUI at a time").
 */
for (const failure of ["refused", "lost reply", "unreadable keys"] as const) {
test(`a failed number-key save is reported and never saved later: ${failure}`, async ({ page, request }) => {
  const prefix = `number-failed-${Date.now()}`;
  const [a, b] = ["a", "b"].map(s => `${prefix}-${s}`);
  try {
    await patchPreferences(request, { template_shortcuts: null, quick_launch_template: null });
    await putTemplate(request, a, { kind: "agent", agent: "claude" });
    await putTemplate(request, b, { kind: "agent", agent: "codex" });
    let writes = 0;
    let rereads = 0;
    await page.route("**/api/preferences", async route => {
      const put = route.request();
      if (put.method() === "GET" && writes > 0) {
        rereads += 1;
        if (failure === "unreadable keys") return route.fulfill({ status: 503, body: "read refused by the test" });
      }
      if (put.method() !== "PUT" || !put.postData()?.includes("template_shortcuts")) return route.fallback();
      writes += 1;
      if (failure === "lost reply") {
        const saved = await route.fetch();
        expect(saved.ok(), "premise: the real helm accepts the write before its reply is lost").toBe(true);
        expect((await readPreferences(request)).template_shortcuts).toEqual({ "1": a });
        return route.abort("failed");
      }
      await route.fulfill({ status: 503, body: "refused by the test" });
    });
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.getByRole("dialog", { name: "templates", exact: true });
    await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    const keyButton = dialog.getByRole("button", { name: `number shortcut ${a}`, exact: true });
    const menu = dialog.getByRole("menu", { name: `number key for ${a}`, exact: true });
    await expect(keyButton).toHaveText("Ctrl+Shift+ –");
    await keyButton.click();
    await menu.getByRole("menuitem", { name: "Ctrl+Shift+1", exact: true }).click();
    const detail = failure === "unreadable keys" ? "current keys could not be read" : "showing the keys read from Farhelm";
    await expect(dialog.locator(".templates-error")).toHaveText(`the number key may not have been saved; ${detail}`);
    expect(writes).toBe(1);
    expect(rereads, "a preferences read followed the failed write").toBeGreaterThan(0);
    await expect(keyButton).toHaveText(failure === "lost reply" ? "Ctrl+Shift+1" : "Ctrl+Shift+ –");
    await page.unroute("**/api/preferences");
    await patchPreferences(request, { template_shortcuts: { "1": b } });
    await keyButton.click();
    await menu.getByRole("menuitem", { name: "Ctrl+Shift+2", exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).template_shortcuts).toEqual({ "1": b, "2": a });
  } finally {
    await patchPreferences(request, { template_shortcuts: null, quick_launch_template: null });
    for (const name of [a, b]) await deleteTemplate(request, name);
  }
});
}

/** A model-only key is useful for composition even though it cannot launch
 * alone. Hold catalog loading past the notice lifetime to prove that loading
 * cannot consume the explanation; editing and elapsed visible time must both
 * dismiss it without dispatching a session create.
 */
test("model-only number key opens New with a temporary composition explanation", async ({ page, request }) => {
  const name = `model-key-${Date.now()}`;
  const response = await request.get("/api/launch-catalog");
  expect(response.ok()).toBe(true);
  const catalog = await response.json();
  const model = catalog.find((entry: { harness: string }) => entry.harness === "claude");
  expect(model, "the real catalog must offer a Claude model for this fixture").toBeTruthy();
  const gate = routeGate();
  let hold = false;
  let held = false;
  let creates = 0;
  await page.route("**/api/launch-catalog", async route => {
    if (hold) {
      held = true;
      await gate.wait();
    }
    await route.fallback();
  });
  await page.route("**/api/sessions", async route => {
    if (route.request().method() !== "POST") return route.fallback();
    creates++;
    await route.fulfill({ status: 500, body: "unexpected create from a partial-template key" });
  });
  try {
    await patchPreferences(request, { template_shortcuts: null, quick_launch_template: null });
    await putTemplate(request, name, { model: model.id });
    const dialog = await openTemplates(page);
    const key = dialog.getByRole("button", { name: `number shortcut ${name}`, exact: true });
    await expect(key).toBeEnabled();
    await expect(key).toHaveAttribute("data-tooltip", /cannot launch on its own.*Ctrl\+Shift\+X/);
    await expect(dialog.getByRole("button", { name: `quick launch ${name}`, exact: true })).toBeDisabled();
    await key.click();
    await dialog.getByRole("menu", { name: `number key for ${name}`, exact: true })
      .getByRole("menuitem", { name: "Ctrl+Shift+4", exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).template_shortcuts).toEqual({ "4": name });
    await dialog.locator(".templates-close").click();
    await expect(dialog).toHaveCount(0);
    hold = true;
    await page.keyboard.press("Control+Shift+Digit4");
    const form = page.locator(".create-session-form");
    const banner = form.getByRole("alert").filter({ hasText: "cannot launch on its own" });
    await expect.poll(() => held).toBe(true);
    await expect(form.getByRole("status")).toHaveText("loading session setup…");
    await expect(banner).toHaveCount(0);
    // sleep-ok: hold catalog loading beyond the six-second notice lifetime to distinguish mount expiry from form-open expiry.
    await page.waitForTimeout(6500);
    expect(held && hold, "the catalog gate must still own the loading boundary").toBe(true);
    await expect(form.getByRole("status")).toHaveText("loading session setup…");
    hold = false;
    gate.release();
    await expect(banner).toContainText(`${name} cannot launch on its own.`);
    await expect(banner).toContainText("Ctrl+Shift+X");
    await banner.click();
    await expect(banner, "reading the explanation is not an edit").toBeVisible();
    await expect(form.getByRole("combobox", { name: "model", exact: true })).toHaveValue(model.id);
    await test.info().attach("partial-template-notice", { body: await page.screenshot(), contentType: "image/png" });
    const modelInput = form.getByRole("combobox", { name: "model", exact: true });
    await modelInput.focus();
    await expect(modelInput).toBeFocused();
    await expect(banner).toBeVisible();
    await page.keyboard.type("edited");
    await expect(banner).toHaveCount(0);
    await form.locator(".launch-composer-cancel").click();
    await expect(form).toHaveCount(0);
    await page.keyboard.press("Control+Shift+Digit4");
    await expect(banner).toBeVisible();
    await form.getByRole("tab", { name: "command", exact: true }).click();
    await expect(banner, "button-based draft choices also dismiss the notice").toHaveCount(0);
    await form.locator(".launch-composer-cancel").click();
    await expect(form).toHaveCount(0);
    await page.keyboard.press("Control+Shift+Digit4");
    await expect(banner).toBeVisible();
    await expect(banner).toHaveCount(0, { timeout: 10000 });
    await expect(form).toBeVisible();
    expect(creates, "a partial-template key must leave New for deliberate completion").toBe(0);
  } finally {
    gate.release();
    await page.unrouteAll({ behavior: "ignoreErrors" });
    await patchPreferences(request, { template_shortcuts: null, quick_launch_template: null });
    await deleteTemplate(request, name);
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

/** Dismissing an input-method candidate must not discard the name or selected
 * fields. Ordinary Escape still closes only the inline panel, retaining the
 * launcher. This case never saves a template or launches a session. */
test("save as template keeps the draft on composing Escape", async ({ page }) => {
  const form = await openNew(page);
  const codex = form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true });
  await codex.click();
  await form.locator(".launch-composer-save-template").click();
  const panel = form.locator(".save-template-panel");
  const field = panel.locator(".save-template-name");
  const agent = panel.locator('[data-field="agent"]');
  await expect(panel).toBeVisible();
  await expect(agent).toBeChecked();
  await agent.uncheck();
  await field.fill("unfinished input-method name");
  await expect(field).toBeFocused();
  await expect(agent).not.toBeChecked();
  await field.evaluate((node) => {
    node.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", isComposing: true, bubbles: true, cancelable: true }));
  });
  await expect(panel).toBeVisible();
  await expect(field).toHaveValue("unfinished input-method name");
  await expect(agent).not.toBeChecked();
  await expect(field).toBeFocused();
  await field.press("Escape");
  await expect(panel).toHaveCount(0);
  await expect(form).toBeVisible();
  await expect(codex).toHaveAttribute("aria-pressed", "true");
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
  let bodyFailed = false;
  // The write may succeed even if a later setup step fails; deletion owns the
  // name before the first request and tolerates an absent template.
  try {
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
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["template route", () => page.unroute("**/api/templates")],
      ["template", () => deleteTemplate(request, name)],
    ]);
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

for (const width of [1100, 390]) {
  /**
   * A held write disables every visible control in both responsive editors.
   * The actual scrolling dialog must temporarily enter Tab order without
   * losing its dialog role/name, then retire that stop and recover Save when
   * the refusal restores the user's remembered control and unchanged draft.
   */
  test(`templates busy root stays keyboard-scrollable at width ${width}`, async ({ page, request }) => {
    const name = `e2e-scroll-busy-${width}-${Date.now()}`;
    const gate = routeGate();
    const submitted: unknown[] = [];
    let bodyFailed = true;
    try {
      await putTemplate(request, name, {
        kind: "command", command: "echo original", resume_command: "echo resume", name: "scrolling command session",
      });
      const build = requireHelmBuild(await request.get("/api/templates"), "held template scroll save");
      await page.route(`**/api/templates/${encodeURIComponent(name)}`, async (route) => {
        if (route.request().method() !== "PUT") return route.continue();
        submitted.push(route.request().postDataJSON());
        await gate.wait();
        await route.fulfill({ status: 409, headers: { "x-farhelm-build": build }, contentType: "text/plain", body: "fixture keeps the template draft" });
      });
      await page.setViewportSize({ width, height: 320 });
      const dialog = await openTemplates(page);
      await dialog.locator(".templates-row").filter({ has: page.getByText(name, { exact: true }) }).click();
      await expect(dialog.locator(".templates-name")).toBeFocused();
      await dialog.locator(".templates-command").fill("echo edited");
      await expect(dialog).toHaveAttribute("tabindex", "-1");
      expect(await dialog.evaluate((node) => node.scrollHeight > node.clientHeight + 1),
        "the actual editor root must overflow before measuring its entry").toBe(true);
      const save = dialog.locator(".templates-save");
      await save.focus();
      await expect(save).toBeFocused();
      await page.keyboard.press("Enter");
      await expect.poll(() => submitted.length, { message: "one actual template write remains held" }).toBe(1);
      const visibleControls = dialog.locator("button:visible, input:visible, select:visible, textarea:visible");
      expect(await visibleControls.count(), "the busy editor retains real visible controls").toBeGreaterThan(0);
      for (const control of await visibleControls.all()) await expect(control).toBeDisabled();
      await expect(dialog).toHaveAttribute("tabindex", "0");
      await expect(dialog).toHaveAttribute("role", "dialog");
      await expect(dialog).toHaveAccessibleName("templates");
      await expect(dialog).toHaveAttribute("aria-modal", "true");
      await expect(dialog.locator('[role="region"], form')).toHaveCount(0);
      await page.keyboard.press("Tab");
      await expect(dialog).toBeFocused();
      // Save can scroll the editor to its bottom before disabling. Home and
      // PageDown must move the real root without a test assigning its offset.
      await page.keyboard.press("Home");
      await expect.poll(() => dialog.evaluate((node) => node.scrollTop)).toBe(0);
      await page.keyboard.press("PageDown");
      await expect.poll(() => dialog.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
      for (const control of await visibleControls.all()) await expect(control).toBeDisabled();
      expect(submitted).toHaveLength(1);
      gate.release();
      await expect(dialog.locator(".templates-error").last()).toContainText("fixture keeps the template draft");
      await expect(dialog.locator(".templates-command")).toHaveValue("echo edited");
      await expect(save).toBeEnabled();
      await expect(dialog).toHaveAttribute("tabindex", "-1");
      await expect(save).toBeFocused();
      await page.keyboard.press("Tab");
      await expect(dialog.locator(".templates-close")).toBeFocused();
      expect((await storedTemplate(request, name)).fields.command, "the refused PUT did not change stored fields").toBe("echo original");
      bodyFailed = false;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["held template write", () => gate.release()],
        ["template write route", () => page.unrouteAll({ behavior: "wait" })],
        ["owned template", () => deleteTemplate(request, name)],
      ]);
    }
  });
}

/** Historical true, false and explicit-default trust values still decode.
 * Inject them on a real template's read, since current writes drop the field.
 * The composed request and subsequent PUT bodies distinguish ignoring trust
 * from merely hiding its control; no vendor process is needed for that proof.
 * Each legacy representation also exercises one editor write path. */
for (const [trust, write] of [[true, "edit"], [false, "duplicate"], [null, "undo"]] as const) {
  test(`old template trust ${trust} is ignored on launch and dropped on ${write}`, async ({ page, request }) => {
    const source = `legacy-trust-${write}-${Date.now()}`;
    const saved = `${source}-saved`;
    const copy = `${source} copy`;
    const fields = { kind: "agent", agent: "codex", permissions: "guarded", destination: { folder: "/tmp" }, name: "legacy-template" };
    let legacyReads = 0;
    const writes: { name: string; fields: Record<string, unknown> }[] = [];
    const creates: { cwd: string; launch: Record<string, unknown> }[] = [];
    try {
      await putTemplate(request, source, fields);
      const build = (await request.get("/api/sessions")).headers()["x-farhelm-build"];
      expect(build, "controlled create refusals retain the live helm identity").toBeTruthy();
      await page.route("**/api/templates", async route => {
        const response = await route.fetch();
        const json = await response.json();
        const legacy = json.templates.find((template: { name: string }) => template.name === source);
        if (legacy) {
          legacy.fields.workspace_trust = trust;
          await route.fulfill({ response, json });
          legacyReads++;
        } else await route.fulfill({ response, json });
      });
      await page.route("**/api/templates/*", async route => {
        if (route.request().method() === "PUT") writes.push({
          name: decodeURIComponent(new URL(route.request().url()).pathname.split("/").at(-1)!),
          fields: route.request().postDataJSON(),
        });
        await route.fallback();
      });
      await page.route("**/api/sessions", async route => {
        if (route.request().method() !== "POST") return route.fallback();
        creates.push(route.request().postDataJSON());
        await route.fulfill({ status: 400, headers: { "x-farhelm-build": build }, body: "legacy trust fixture refused create" });
      });
      const form = await openNew(page);
      await applyTemplate(form, source);
      await expect(form.getByLabel("name (optional)")).toHaveValue("legacy-template");
      await expect.poll(() => legacyReads, { message: "the launcher received the old-rules template" }).toBeGreaterThan(0);
      await expect(form.locator(".launch-composer-trust-choice")).toHaveCount(0);
      await expect(form.locator(".launch-composer-summary")).not.toContainText("trust:");
      await form.locator(".create-session-submit").click();
      await expect(form.locator(".launch-composer-refusal")).toContainText("legacy trust fixture refused create");
      expect(creates).toHaveLength(1);
      expect(creates[0]).toMatchObject({ cwd: "/tmp", launch: { harness: "codex", permissions: "guarded" } });
      expect(creates[0].launch).not.toHaveProperty("workspace_trust");

      await form.locator(".launch-composer-save-template").click();
      const panel = form.locator(".save-template-panel");
      await expect(panel).toBeVisible();
      await expect(panel.locator('[data-field="trust"]')).toHaveCount(0);
      await panel.locator(".save-template-name").fill(saved);
      await panel.locator(".save-template-save").click();
      const dialog = page.locator('.templates-dialog[role="dialog"]');
      await expect(dialog).toBeVisible();
      expect((await storedTemplate(request, saved)).fields).not.toHaveProperty("workspace_trust");
      await dialog.locator(".templates-close").click();
      await expect(dialog).toHaveCount(0);

      await openTemplatesInPlace(page);
      const row = dialog.locator(".templates-row").filter({ has: page.getByText(source, { exact: true }) });
      await row.click();
      await expect(dialog.locator(".templates-name")).toHaveValue(source);
      await expect(row.locator(".templates-row-summary")).not.toContainText("trust");
      await expect(dialog.locator('[data-field="trust"]')).toHaveCount(0);
      await dialog.locator(".templates-add").click();
      await expect(dialog.getByRole("group", { name: "add field", exact: true })).toBeVisible();
      await expect(dialog.locator(".templates-add-menu").getByRole("button", { name: "workspace trust", exact: true })).toHaveCount(0);
      await dialog.locator(".templates-add").click();
      let writtenName = source;
      if (write === "duplicate") {
        await dialog.locator(".templates-duplicate").click();
        // Normalizing an old record is an unsaved draft. Discard selects the
        // stored source before duplicating; that path must clear trust too.
        await expect(dialog.locator(".templates-departure")).toBeVisible();
        await dialog.locator(".templates-discard").click();
        await expect(dialog.locator(".templates-name")).toHaveValue(copy);
        writtenName = copy;
        await dialog.locator(".templates-save").click();
        await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
      } else if (write === "undo") {
        await dialog.locator(".templates-delete").click();
        await expect(dialog.locator(".templates-undo")).toBeVisible();
        await dialog.locator(".templates-undo").click();
        await expect(dialog.locator(".templates-undo")).toHaveCount(0);
      } else {
        await dialog.locator(".templates-session-name").fill("edited legacy template");
        await dialog.locator(".templates-save").click();
        await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
      }
      expect((await storedTemplate(request, writtenName)).fields).not.toHaveProperty("workspace_trust");
      expect(writes.map(entry => entry.name)).toContain(saved);
      expect(writes.map(entry => entry.name)).toContain(writtenName);
      for (const entry of writes) expect(entry.fields).not.toHaveProperty("workspace_trust");
    } finally {
      await page.unrouteAll({ behavior: "wait" });
      for (const name of [source, saved, copy]) await deleteTemplate(request, name);
    }
  });
}

/** Saving a launcher message must preserve the reviewed bytes rather than
 * their escaped preview. Unchecking it must produce an absent partial edit.
 * Template stacking must distinguish an absent message from an empty reset.
 */
for (const keepPrompt of [true, false]) {
  test(`initial prompt launcher template stacking and save selection ${keepPrompt}`, async ({ page, request }) => {
    const stamp = Date.now();
    const initial = `prompt-source-${stamp}`;
    const unchanged = `prompt-unchanged-${stamp}`;
    const clear = `prompt-clear-${stamp}`;
    const nameOnly = `prompt-name-${stamp}`;
    const saved = `prompt-saved-${stamp}`;
    const message = "first\u202e\nsecond";
    try {
      await putTemplate(request, initial, { kind: "agent", prompt: message });
      await putTemplate(request, unchanged, { agent: "codex", name: "other edit" });
      await putTemplate(request, clear, { prompt: "" });
      await putTemplate(request, nameOnly, { name: "unrelated edit" });
      const form = await openNew(page);
      await applyTemplate(form, initial);
      const prompt = form.getByLabel("initial prompt", { exact: true });
      await expect(prompt).toHaveCount(0);
      await applyTemplate(form, unchanged);
      await expect(prompt).toHaveValue("first<U+202E><U+000A>second");
      await expect(prompt).toHaveJSProperty("readOnly", true);
      await applyTemplate(form, unchanged);
      await expect(prompt).toHaveValue("first<U+202E><U+000A>second");
      await applyTemplate(form, clear);
      await expect(prompt).toHaveValue("");
      await expect(prompt).toBeEditable();
      await applyTemplate(form, initial);
      // A projected-out message must not be written back as absence by an
      // unrelated template. Both dormant launch kinds retain the raw draft.
      await form.getByRole("tab", { name: "command", exact: true }).click();
      await applyTemplate(form, nameOnly);
      await form.getByRole("tab", { name: "agent", exact: true }).click();
      await expect(prompt).toHaveValue("first<U+202E><U+000A>second");
      await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Goose", exact: true }).click();
      await expect(prompt).toHaveCount(0);
      await applyTemplate(form, nameOnly);
      await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
      await expect(prompt).toHaveValue("first<U+202E><U+000A>second");
      await form.locator(".launch-composer-save-template").click();
      const panel = form.locator(".save-template-panel");
      const choice = panel.locator(".save-template-choice").filter({ has: page.locator('[data-field="prompt"]') });
      await expect(choice).toContainText("initial prompt: first<U+202E>…");
      const checkbox = choice.locator('input[type="checkbox"]');
      await expect(checkbox).toBeChecked();
      if (!keepPrompt) await checkbox.uncheck();
      await panel.locator(".save-template-name").fill(saved);
      await panel.locator(".save-template-save").click();
      await expect(page.locator(".templates-dialog")).toBeVisible();
      expect((await storedTemplate(request, saved)).fields.prompt).toBe(keepPrompt ? message : undefined);
    } finally {
      for (const name of [initial, unchanged, clear, nameOnly, saved]) await deleteTemplate(request, name);
    }
  });
}

/** Recent setups omit the one-time message. Applying one must leave the
 * current message alone, while each new Clone/Replace opening clears it.
 * The source is an owned inert command session, never a vendor agent.
 */
test("initial prompt survives a recent but clears on Clone and Replace prefills", async ({ page, request }) => {
  const source = await createSession(request, { title: `prompt-prefill-${Date.now()}` });
  try {
    const response = await request.get("/api/sessions");
    const build = response.headers()["x-farhelm-build"];
    const hosts = await listHosts(request);
    const host = hosts.find(host => host.kind === "local");
    expect(host).toBeTruthy(); expect(build).toBeTruthy();
    await page.route("**/api/launch-history**", route => route.fulfill({
      status: 200, headers: { "content-type": "application/json", "x-farhelm-build": build },
      body: JSON.stringify({ folders: [], launches: [{ host: host!.id, canonical_cwd: "/tmp", cwd: "/tmp", selection: { harness: "codex", model: null, effort: "high", permissions: null, workspace_trust: null }, created_at: 1, creation_seq: 1 }] }),
    }));
    // New starts in the selected session's folder, where the staged recent lives; a page load opens the latest visit.
    await pinAutoSelect(page, source.id);
    const form = await openNew(page);
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await form.getByLabel("initial prompt", { exact: true }).fill("keep through recent");
    await expect(form.getByLabel("host", { exact: true })).toHaveAttribute("data-host-id", String(host!.id));
    const recent = form.locator(".launch-composer-recents button");
    await expect(recent).toHaveCount(1);
    await recent.click();
    await expect(form.getByLabel("initial prompt", { exact: true })).toHaveValue("keep through recent");
    await form.locator(".launch-composer-cancel").click();
    for (const selector of [".session-row-clone", ".session-row-replace-with"]) {
      const row = page.locator(`.session-row[data-session-id="${source.id}"]`);
      await expect(row).toBeVisible();
      await openRowMenu(row);
      await row.locator(selector).click();
      await expect(form).toBeVisible();
      await form.getByRole("tab", { name: "agent", exact: true }).click();
      await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
      await expect(form.getByLabel("initial prompt", { exact: true })).toHaveValue("");
      await form.getByLabel("initial prompt", { exact: true }).fill("discard for next prefill");
      await form.locator(".launch-composer-cancel").click();
    }
  } finally {
    await cleanupSession(request, source.id);
  }
});

/** A prompt is a partial template edit, preserved across textarea newlines.
 * Changing to an unsupported agent keeps the incompatible field visible until
 * the user removes it; the add menu must then stop offering it.
 */
test("initial prompt template field preserves multiline text and agent compatibility", async ({ page, request }) => {
  const name = `initial-prompt-${Date.now()}`;
  const message = "first line\n\t--literal {farhelm_args}";
  try {
    const dialog = await openTemplates(page);
    await dialog.locator(".templates-new").click();
    await dialog.locator(".templates-name").fill(name);
    await addField(dialog, "initial prompt");
    const prompt = dialog.getByLabel("initial prompt", { exact: true });
    await expect(prompt).toBeEditable();
    await prompt.fill(message);
    await expect(prompt).toHaveValue(message);
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, name)).fields.prompt).toBe(message);
    await expect(dialog.locator(".templates-row").filter({ has: page.getByText(name, { exact: true }) })
      .locator(".templates-row-summary")).toContainText("initial prompt");
    await addField(dialog, "agent type");
    await dialog.getByLabel("agent type", { exact: true }).selectOption("pi");
    await expect(dialog.locator('.templates-set-field[data-field="prompt"] .templates-error')).toContainText("does not fit");
    await expect(prompt).toHaveValue(message);
    await dialog.getByRole("button", { name: "remove initial prompt", exact: true }).click();
    await dialog.locator(".templates-add").click();
    await expect(dialog.locator(".templates-add-menu").getByRole("button", { name: "initial prompt", exact: true })).toHaveCount(0);
    await dialog.locator(".templates-add").click();
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    const saved = await storedTemplate(request, name);
    expect(saved.fields.agent).toBe("pi");
    expect(saved.fields.prompt).toBeUndefined();
  } finally {
    await deleteTemplate(request, name);
  }
});

/** Host-supplied formatting must be visible without rewriting the stored
 * message when an unrelated field is saved. Explicit removal/re-addition is
 * the editor's existing way to replace a value it cannot display directly.
 */
test("initial prompt template escapes formatting while preserving raw saved text", async ({ page, request }) => {
  const name = `initial-prompt-escaped-${Date.now()}`;
  const message = "first\u202e\nsecond";
  try {
    await putTemplate(request, name, { kind: "agent", prompt: message });
    const dialog = await openTemplates(page);
    await dialog.locator(".templates-row-name").getByText(name, { exact: true }).click();
    const prompt = dialog.getByLabel("initial prompt", { exact: true });
    await expect(prompt).toHaveJSProperty("readOnly", true);
    await expect(prompt).toHaveValue("first<U+202E><U+000A>second");
    await addField(dialog, "session name");
    await dialog.locator(".templates-session-name").fill("other edit");
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, name)).fields.prompt).toBe(message);
    await dialog.getByRole("button", { name: "remove initial prompt", exact: true }).click();
    await addField(dialog, "initial prompt");
    await expect(prompt).toBeEditable();
    await expect(prompt).toHaveValue("");
    await prompt.fill("replacement\nmessage");
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, name)).fields.prompt).toBe("replacement\nmessage");
  } finally {
    await deleteTemplate(request, name);
  }
});

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
 * including when another client takes a name after the dialog loaded its list:
 * a rename to it re-reads the names before writing, and so does undo.
 */
test("long summaries stay reachable and taken names refuse save and undo", async ({ page, request }) => {
  const name = `e2e-long-${Date.now()}`;
  const taken = `${name}-taken`;
  const late = `${name}-late`;
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
    // The dialog read its list on opening, so only a fresh read sees this one.
    await putTemplate(request, late, { name: "outside" });
    await expect(dialog.locator(".templates-row-name", { hasText: late })).toHaveCount(0);
    await dialog.locator(".templates-name").fill(late);
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-error")).toContainText(`"${late}" already exists`);
    expect((await storedTemplate(request, late)).fields).toEqual({ name: "outside" });
    expect((await storedTemplate(request, name)).fields).toEqual(fields);
    await dialog.locator(".templates-name").fill(name);
    await dialog.locator(".templates-delete").click();
    await expect(dialog.locator(".templates-undo")).toBeVisible();
    expect(await storedTemplate(request, name)).toBeUndefined();
    await putTemplate(request, name, { name: "replacement" });
    await dialog.locator(".templates-undo").click();
    await expect(dialog.locator(".templates-error")).toContainText("already exists");
    expect((await storedTemplate(request, name)).fields).toEqual({ name: "replacement" });
  } finally {
    for (const value of [name, taken, late]) await deleteTemplate(request, value);
  }
});

/** A name's policy survives copying and delete undo, but removing the name
 * discards it. Base saves must agree with the normalized API reply so a
 * successful save cannot immediately look dirty again. */
test("session name kind survives duplicate and undo and base saves stay clean", async ({ page, request }, info) => {
  const name = `name-kind-${Date.now()}`;
  const copy = `${name} copy`;
  try {
    await putTemplate(request, name, { kind: "agent", name: "review", name_kind: "fixed" });
    const dialog = await openTemplates(page);
    await dialog.locator(".templates-row-name").getByText(name, { exact: true }).click();
    const kinds = dialog.getByRole("group", { name: "session name kind", exact: true });
    await expect(kinds.getByRole("button", { name: "fixed name", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    await page.setViewportSize({ width: 390, height: 800 });
    await expect(kinds).toBeVisible();
    const bounds = await kinds.evaluate(node => ({ control: node.getBoundingClientRect().right, dialog: node.closest('[role="dialog"]')!.getBoundingClientRect().right }));
    expect(bounds.control, "the policy stays inside the phone-width editor").toBeLessThanOrEqual(bounds.dialog);
    await page.screenshot({ path: info.outputPath("template-name-kind-phone.png") });
    await page.setViewportSize({ width: 1280, height: 800 });
    await dialog.locator(".templates-duplicate").click();
    await expect(dialog.locator(".templates-name")).toHaveValue(copy);
    await expect(kinds.getByRole("button", { name: "fixed name", exact: true })).toHaveAttribute("aria-pressed", "true");
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, copy)).fields.name_kind).toBe("fixed");
    await dialog.locator(".templates-delete").click();
    await expect(dialog.locator(".templates-undo")).toBeVisible();
    expect(await storedTemplate(request, copy)).toBeUndefined();
    await dialog.locator(".templates-undo").click();
    await expect(dialog.locator(".templates-row-name").getByText(copy, { exact: true })).toBeVisible();
    expect((await storedTemplate(request, copy)).fields.name_kind).toBe("fixed");
    await dialog.locator(".templates-row-name").getByText(copy, { exact: true }).click();
    await kinds.getByRole("button", { name: "base name", exact: true }).click();
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, copy)).fields).toEqual({ kind: "agent", name: "review" });
    await dialog.getByRole("button", { name: "remove session name", exact: true }).click();
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, copy)).fields).toEqual({ kind: "agent" });
    await addField(dialog, "session name");
    await expect(kinds.getByRole("button", { name: "base name", exact: true })).toHaveAttribute("aria-pressed", "true");
    await dialog.locator(".templates-session-name").fill("review again");
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    expect((await storedTemplate(request, copy)).fields).toEqual({ kind: "agent", name: "review again" });
  } finally {
    for (const item of [name, copy]) await deleteTemplate(request, item);
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
      await form.getByLabel("name (optional)", { exact: true }).fill("typed review");
      await form.locator(".launch-composer-save-template").click();
      const panel = form.locator(".save-template-panel");
      await expect(panel).toBeVisible();
      await expect(panel.locator('[data-field="destination"]')).toBeChecked();
      await expect(panel.locator('[data-field="name"]')).toBeChecked();
      const nameKinds = panel.getByRole("group", { name: "session name kind", exact: true });
      await expect(nameKinds.getByRole("button", { name: "base name", exact: true })).toHaveAttribute("aria-pressed", "true");
      if (kind === "command") await nameKinds.getByRole("button", { name: "fixed name", exact: true }).click();
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
      if (kind === "agent") {
        expect(stored.fields.agent).toBe("codex");
        expect(stored.fields.name).toBe("typed review");
        expect(stored.fields.name_kind).toBeUndefined();
      } else expect(stored.fields).toEqual({ kind: "command", command: "printf hello", yolo: false, name: "typed review", name_kind: "fixed" });
      expect(creates, "saving a template sends no session create").toEqual([]);
    } finally {
      await deleteTemplate(request, name);
    }
  });
}

/**
 * A collision is a refusal, not a replacement. Escape dismisses only the
 * inline panel and leaves the setup editable, including after Enter disabled
 * the name field during the failed save. Focus must recover without test help.
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
    await expect(panel.locator(".save-template-name")).toBeFocused();
    await panel.locator(".save-template-name").press("Enter");
    await expect(panel.getByRole("alert")).toContainText("already exists");
    expect((await storedTemplate(request, name)).fields).toEqual(fields);
    await expect(panel.locator(".save-template-name")).toBeFocused();
    await panel.locator(".save-template-name").press("Escape");
    await expect(panel).toHaveCount(0);
    await expect(form).toBeVisible();
    await expect(form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true })).toHaveAttribute("aria-pressed", "true");
  } finally {
    await deleteTemplate(request, name);
  }
});

/** Body Escape must resolve the innermost Cancel even though the launcher's
 * Cancel precedes it in the DOM. The next Escape may close the launcher. */
test("save as template handles body Escape before the launcher", async ({ page }) => {
  const form = await openNew(page);
  const codex = form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true });
  await codex.click();
  await form.locator(".launch-composer-save-template").click();
  const panel = form.locator(".save-template-panel");
  await expect(panel).toBeVisible();
  await expect(panel.locator(".save-template-name")).toBeFocused();
  await blurTemplateFocusToBody(page);
  await page.keyboard.press("Escape");
  await expect(panel).toHaveCount(0);
  await expect(form).toBeVisible();
  await expect(codex).toHaveAttribute("aria-pressed", "true");
  await blurTemplateFocusToBody(page);
  await page.keyboard.press("Escape");
  await expect(form).toHaveCount(0);
});

/** Assert the lost-focus premise before exercising the isolation safety net.
 * A key sent to a focused panel field would bypass the body routing bug. */
async function blurTemplateFocusToBody(page: Page) {
  expect(await page.evaluate(() => {
    (document.activeElement as HTMLElement | null)?.blur();
    return document.activeElement === document.body;
  }), "premise: the next key starts on body outside the launcher").toBe(true);
}

/** Cross the refusal's deferred focus handoff before checking a surviving owner.
 * A still-focused field can satisfy toBeFocused before recovery runs; two
 * browser frames cross rendering and the queued recovery without a time delay. */
async function afterTemplateFocusRecovery(page: Page) {
  await page.evaluate(() => new Promise<void>(resolve => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  }));
}

for (const focusOwner of ["lost", "search", "launch", "approval"] as const) {
  /** Hold a real save at its PUT so busy Escape and the eventual refusal
   * are observed separately. Recovery must yield to a launcher field the
   * user chose while saving, while lost focus returns to the template name.
   * The exempt native button models an approval answer's focus ownership;
   * this fixture does not exercise the separate agent approval protocol. */
  test(`save as template refusal respects ${focusOwner} focus`, async ({ page, request }) => {
    const name = `e2e-refused-save-${Date.now()}`;
    const gate = routeGate();
    let writes = 0;
    let bodyFailed = true;
    const build = requireHelmBuild(await request.get("/api/templates"), "refused launcher template save");
    await page.route(`**/api/templates/${encodeURIComponent(name)}`, async route => {
      if (route.request().method() !== "PUT") return route.continue();
      writes++;
      await gate.wait();
      await route.fulfill({ status: 409, headers: { "x-farhelm-build": build }, contentType: "text/plain", body: "fixture refused the template save" });
    });
    try {
      const form = await openNew(page);
      await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
      await form.locator(".launch-composer-save-template").click();
      const panel = form.locator(".save-template-panel");
      const field = panel.locator(".save-template-name");
      await field.fill(name);
      await expect(field).toBeFocused();
      await field.press("Enter");
      await expect.poll(() => writes, { message: "one actual PUT remains held" }).toBe(1);
      await expect(field).toBeDisabled();
      await expect(panel.locator(".save-template-cancel")).toBeDisabled();
      await blurTemplateFocusToBody(page);
      await page.keyboard.press("Escape");
      await expect(panel).toBeVisible();
      await expect(form).toBeVisible();
      if (focusOwner === "approval") {
        await page.evaluate(() => {
          const answer = document.createElement("button");
          answer.className = "fixture-approval-answer";
          answer.setAttribute("data-modal-exempt", "");
          answer.textContent = "fixture approval answer";
          document.body.append(answer);
        });
      }
      const chosen = focusOwner === "search" ? form.locator(".launch-composer-search input")
        : focusOwner === "launch" ? form.locator(".create-session-submit")
        : focusOwner === "approval" ? page.locator(".fixture-approval-answer") : field;
      if (focusOwner !== "lost") {
        if (focusOwner === "launch") {
          // ARIA unavailability does not remove native focus ownership.
          // Playwright's enabled/click checks include ARIA, so use native
          // focus directly after proving this is the intended unavailable action.
          await expect(chosen).toHaveAttribute("aria-disabled", "true");
          await expect(chosen).toHaveJSProperty("disabled", false);
        } else {
          await expect(chosen).toBeEnabled();
          // Only search models a pointer choice. The synthetic approval
          // answer has no real card's stacking above the scrim; this case
          // measures native focus ownership, not approval hit testing.
          if (focusOwner === "search") await chosen.click();
        }
        // WebKit does not focus a clicked button. Establish the chosen owner
        // explicitly before releasing the response whose recovery we measure.
        await chosen.focus();
        await expect(chosen).toBeFocused();
      }
      gate.release();
      await expect(panel.getByRole("alert")).toContainText("fixture refused the template save");
      await expect(field).toBeEnabled();
      await afterTemplateFocusRecovery(page);
      await expect(chosen).toBeFocused();
      await expect(field).toHaveValue(name);
      expect(writes, "body Escape must not submit another save").toBe(1);
      bodyFailed = false;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["held template PUT", () => gate.release()],
        ["template write route", () => page.unrouteAll({ behavior: "wait" })],
      ]);
    }
  });
}
