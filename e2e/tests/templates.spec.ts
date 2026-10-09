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
import { cleanupSession, createSession, listHosts, openRowMenu } from "./helpers/fleet";

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
      form.getByRole("group", { name: "runs without approval prompts" }).getByLabel("no", { exact: true }),
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
 */
for (const kind of ["agent", "command"] as const) {
  test(`save as template captures the ${kind} setup without launching`, async ({ page, request }) => {
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
        await form.getByRole("group", { name: "runs without approval prompts" }).getByLabel("no", { exact: true }).check();
      }
      await form.getByLabel("folder", { exact: true }).fill("/tmp");
      await form.locator(".launch-composer-save-template").click();
      const panel = form.locator(".save-template-panel");
      await expect(panel).toBeVisible();
      await expect(panel.locator('[data-field="destination"]')).toBeChecked();
      await panel.locator('[data-field="destination"]').uncheck();
      if (kind === "agent") await expect(panel.locator('[data-field="agent"]')).toBeChecked();
      else await expect(panel.locator('[data-field="command"]')).toBeChecked();
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
