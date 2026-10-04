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
 * Spec: the Templates panel beside New creates a template from its form,
 * lists it with what it sets, loads it back for editing, saves the edit, and
 * deletes it.
 *
 * Why: the panel is the only place a person creates or changes templates
 * (agents can only apply them), so its round trip through the helm is the
 * feature's whole write path.
 */
test("the Templates panel creates, edits and deletes a template", async ({ page, request }) => {
  const name = `e2e-panel-${Date.now()}`;
  try {
    await page.goto("/");
    await page.getByRole("button", { name: "templates", exact: true }).click();
    const dialog = page.locator('.templates-dialog[role="dialog"]');
    await expect(dialog).toBeVisible();
    await dialog.locator(".templates-name").fill(name);
    await dialog.getByLabel("agent type").selectOption("codex");
    await dialog.getByLabel("effort").selectOption("high");
    await dialog.locator(".templates-save").click();
    const row = dialog.locator(".templates-row", { hasText: name });
    await expect(row).toBeVisible();
    await expect(row.locator(".templates-row-summary")).toHaveText("Codex · effort high");

    await row.locator(".templates-edit").click();
    await expect(dialog.getByLabel("agent type")).toHaveValue("codex");
    await dialog.getByLabel("effort").selectOption("low");
    await dialog.locator(".templates-save").click();
    await expect(row.locator(".templates-row-summary")).toHaveText("Codex · effort low");
    const stored = await (await request.get("/api/templates")).json();
    expect(stored.templates.find((template: any) => template.name === name)?.fields).toEqual({
      agent: "codex",
      effort: "low",
    });

    await row.locator(".templates-delete").click();
    await expect(row).toHaveCount(0);
  } finally {
    await deleteTemplate(request, name);
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
    await applyTemplate(form, name);
    await expect(form.locator(".launch-composer-template-error")).toContainText("this dialog keeps the session's host");
  } finally {
    await deleteTemplate(request, name);
    await cleanupSession(request, source.id);
  }
});
