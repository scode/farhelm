/**
 * Screenshots for the docs page "Launch templates"
 * (website/src/content/docs/docs/using/launch-templates.mdx).
 *
 * The templates shown are the scenario's, which staging created on the helm
 * (docs/docs-shots/scenario.json5). Nothing here saves or deletes a template
 * or launches a session: the panel's edit form is opened and photographed,
 * and templates are applied in the launcher, which only fills in its choices.
 */
import { expect, test, type Locator, type Page } from "@playwright/test";
import { open, openLauncher } from "./open";
import { shot } from "./shot";

const PAGE = "launch-templates";

/** Open the templates panel from the session list's header. */
async function openPanel(page: Page): Promise<Locator> {
  await page.locator(".templates-button").click();
  const dialog = page.locator(".templates-dialog");
  await expect(dialog.locator(".templates-row").first()).toBeVisible();
  return dialog;
}

/** The panel's row for the template called `name`. */
function templateRow(dialog: Locator, name: string): Locator {
  return dialog.locator(".templates-row").filter({
    has: dialog.page().locator(".templates-row-name", { hasText: new RegExp(`^${name}$`) }),
  });
}

/**
 * Apply the template called `name` in an open launcher the way a person
 * would: type tl: and part of the name, then choose the offered template.
 */
async function applyTemplate(form: Locator, query: string, name: string): Promise<void> {
  const search = form.locator(".launch-composer-search input[role=combobox]");
  await search.fill(query);
  const option = form.locator(".launch-composer-search-results").getByRole("option", { name: new RegExp(name) });
  await option.click();
  await expect(search).toHaveValue("");
}

test("templates button", async ({ page, request }) => {
  const { director } = await open(page, request);
  const button = page.locator(".templates-button");
  await director.callout(button, "Opens the templates panel, where you create, edit, and delete templates.", {
    side: "right",
    dx: 40,
    dy: -60,
  });
  await shot(page, `${PAGE}/templates-button`, [button, page.locator(".new-session-button")], { maxWidth: 930 });
});

test("panel", async ({ page, request }) => {
  const { director } = await open(page, request);
  const dialog = await openPanel(page);
  // Both callouts down the left margin, outside the dialog: the list and the
  // form are what this shot is for, and the dialog leaves no room inside.
  await director.callout(
    dialog.locator(".templates-list"),
    "Your templates and what each sets. Editing or deleting one never changes a session started from it.",
    { side: "left", dy: -30 },
  );
  await director.callout(
    dialog.locator(".templates-form-title"),
    "Name a new template and set only what it should change. A field left as is stays as the launcher has it.",
    { side: "left", dy: 90 },
  );
  await shot(page, `${PAGE}/panel`, [dialog], { maxWidth: 930 });
});

test("edit form", async ({ page, request }) => {
  const { director } = await open(page, request);
  const dialog = await openPanel(page);
  await templateRow(dialog, "codex-deep").locator(".templates-edit").click();
  await expect(dialog.locator(".templates-form-title")).toHaveText("edit template");
  await expect(dialog.locator(".templates-model")).toHaveValue("gpt-6.1-sol");
  const fields = dialog.locator(".templates-field");
  await director.callout(
    fields.filter({ hasText: /^agent type/ }),
    "codex-deep makes it an agent launch with Codex, its model, and its effort, and nothing else.",
    { side: "left" },
  );
  await director.callout(
    fields.filter({ hasText: /^destination/ }),
    "Left as is: applying it keeps whatever host, directory, and name the launcher already has.",
    { side: "left" },
  );
  await shot(page, `${PAGE}/edit-form`, [dialog], { maxWidth: 930 });
});

test("apply in the launcher", async ({ page, request }) => {
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  const search = form.locator(".launch-composer-search input[role=combobox]");
  await search.fill("tl:");
  const results = form.locator(".launch-composer-search-results");
  await expect(results.getByRole("option").first()).toBeVisible();
  await director.callout(search, "tl: lists your templates. Type part of a name to narrow the list.", {
    side: "left",
  });
  await director.callout(
    results,
    "Choosing one makes its choices in the launcher, just as if you had made them by hand.",
    { side: "left", dy: 60 },
  );
  await shot(page, `${PAGE}/apply`, [search, results], { maxWidth: 900 });
});

test("applied in turn", async ({ page, request }) => {
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  // The stacking case the page describes: one template picks the agent and
  // how it runs, the next picks where, and the launch combines both.
  await applyTemplate(form, "tl:codex", "codex-deep");
  await applyTemplate(form, "tl:api", "api-repo");
  const submit = form.locator(".create-session-submit");
  const summary = form.locator(".launch-composer-summary");
  await expect(submit).toContainText("build-box");
  await expect(summary).toContainText("gpt-6.1-sol");
  await director.callout(
    submit,
    "After codex-deep, then api-repo: Codex from the first, build-box and ~/src/api from the second.",
    { side: "left" },
  );
  await director.callout(summary, "The model and effort codex-deep set. Change anything before you launch.", {
    side: "left",
    dy: 100,
  });
  await shot(page, `${PAGE}/applied`, [submit, summary], { maxWidth: 930 });
});
