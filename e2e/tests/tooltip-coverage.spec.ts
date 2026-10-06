/**
 * Every visible clickable control carries hover text.
 *
 * The maintainer asked for hover help on every icon and every clickable
 * control, plain-word buttons included, because a user otherwise has to
 * guess what an unlabeled icon means or what a terse button will do before
 * clicking it. Hover text is only ever added one control at a time, so a new
 * button without it would slip in unnoticed; this sweep is what notices.
 *
 * What counts as a clickable control is a fixed selector list (below), not a
 * `cursor: pointer` heuristic, so the test's verdict does not depend on
 * styling. A control passes when it carries a non-empty `data-tooltip`
 * (Farhelm's own tooltip, assets/tooltip.js). Natively disabled controls are
 * checked for the attribute too, so the text is there once they enable; the
 * tooltip itself is not required to show on them, because some engines send
 * no pointer events to a disabled button.
 *
 * The one standing exemption is the maintainer's: an item in a row's actions
 * menu that already shows a description line under its label gets no
 * tooltip, since the description already says it. Every other exemption is
 * listed in `EXEMPT` with its reason.
 *
 * Each visited state also asserts that no element carries a native `title`,
 * which would bring the browser's slow tooltip back on top of Farhelm's.
 *
 * The sweep visits only states the suite's existing fixtures reach: the
 * sidebar with the shared session in both row densities, the session's own
 * view with an extra terminal tab, the row and host menus, the rename and
 * delete confirmations, the help menu, settings, feedback, templates, the
 * host settings and add-host dialogs, and both tabs of the new-session form.
 * States it cannot reach (a host mid-update, an interrupted session, a
 * provisioning plan) are covered by review of the inventory instead.
 */
import { expect, test } from "./helpers/evidence";
import { type Page } from "@playwright/test";
import { localHostId, openHostMenu, openRowMenu, patchPreferences, resetPreferences } from "./helpers/fleet";
import { addTab, installTerminalSuiteHooks, openTerminal, sharedSessionRow } from "./helpers/terminal-suite";

installTerminalSuiteHooks();

/**
 * What the sweep treats as a clickable control. Checkboxes and radio
 * buttons are included because a click on them changes something; their
 * hover text conventionally sits on the `label` that wraps them, so for
 * those two a tooltip-bearing ancestor counts, exactly as the tooltip script
 * itself resolves the nearest `data-tooltip`. Every other control must carry
 * its own, so a wrapper's text cannot stand in for a button's.
 */
const CONTROLS =
  "button, [role=button], [role=tab], [role=menuitem], a[href], select, summary, input[type=checkbox], input[type=radio]";

/**
 * Controls that may lack hover text, each with the reason. Matched with
 * `Element.matches`.
 */
const EXEMPT: { selector: string; reason: string }[] = [
  {
    // The bar menus reuse the row menus' item classes but are not row
    // menus, so they are outside the exemption.
    selector: ".session-row-menu-item:not(.bar-menu-item):has(.session-row-menu-description)",
    reason: "a menu item whose description line already says what it does (the maintainer's exemption)",
  },
];

/**
 * How many visible controls the sweep looked at, and the ones among them
 * that lack hover text, as short descriptions a failure message can name. Visibility is the element having
 * a box and not being hidden by `visibility`; controls under an inert
 * subtree (behind a modal) still count, because they are visible and will
 * be hoverable once the modal closes.
 */
async function sweep(page: Page, root: string): Promise<{ checked: number; missing: string[] }> {
  return page.evaluate(({ controls, exempt, root }) => {
    const scope = document.querySelector(root);
    const missing: string[] = [];
    let checked = 0;
    for (const element of document.querySelectorAll<HTMLElement>(controls)) {
      const box = element.getBoundingClientRect();
      if (box.width === 0 || box.height === 0) continue;
      if (getComputedStyle(element).visibility === "hidden") continue;
      if (scope !== null && scope.contains(element)) checked += 1;
      if (exempt.some((selector) => element.matches(selector))) continue;
      const owner = element.matches("input[type=checkbox], input[type=radio]")
        ? element.closest("[data-tooltip]")
        : element;
      const text = owner?.getAttribute("data-tooltip") ?? null;
      if (text !== null && text.trim() !== "") continue;
      const label = element.getAttribute("aria-label") ?? element.textContent?.trim().slice(0, 40) ?? "";
      missing.push(`<${element.tagName.toLowerCase()} class="${element.className}"> ${JSON.stringify(label)}`);
    }
    return { checked, missing };
  }, { controls: CONTROLS, exempt: EXEMPT.map((entry) => entry.selector), root });
}

/** Every element still carrying a native `title`, which would show the browser's slow tooltip. */
async function nativeTitles(page: Page): Promise<string[]> {
  return page.evaluate(() =>
    // An SVG `<title>` child shows the same slow native tooltip.
    [...document.querySelectorAll("[title], svg title")].map((node) => node.outerHTML.slice(0, 120))
  );
}

/**
 * Assert the current page state: every visible control in the page has hover
 * text and nothing has a native title. `root` is the state's own container
 * and `minimum` a floor on how many visible controls the sweep found inside
 * it: the state's premise, counted there rather than page-wide because the
 * sidebar alone would meet any floor, so a selector or visibility mistake
 * that hid the state's own controls from the sweep fails instead of passing
 * vacuously.
 */
async function expectCovered(page: Page, state: string, root: string, minimum: number) {
  const { checked, missing } = await sweep(page, root);
  expect(checked, `${state}: premise: the sweep sees the state's controls`).toBeGreaterThanOrEqual(minimum);
  expect(missing, `${state}: controls without hover text`).toEqual([]);
  expect(await nativeTitles(page), `${state}: elements with a native title`).toEqual([]);
}

test.afterEach(async ({ request }) => {
  await resetPreferences(request);
});

/**
 * The sidebar, its menus and dialogs, and the new-session form.
 *
 * Why: these are the surfaces a user meets first and where most of the
 * terse icon-only controls live (the row and host menus, the gear, the help
 * button, the host icons).
 */
test("sidebar surfaces give every control hover text", async ({ page, request }) => {
  test.setTimeout(120_000);
  await page.goto("/");
  const row = sharedSessionRow(page);
  await expect(row).toBeVisible({ timeout: 20_000 });
  // The sweep bites: a visible button without hover text is reported.
  await page.evaluate(() => {
    const probe = document.createElement("button");
    probe.id = "tt-coverage-probe";
    probe.textContent = "probe";
    probe.setAttribute("style", "position:fixed;left:300px;top:300px;z-index:45;");
    document.body.appendChild(probe);
  });
  expect((await sweep(page, "body")).missing.some((entry) => entry.includes('"probe"')), "the sweep must report a bare button")
    .toBe(true);
  await page.evaluate(() => document.getElementById("tt-coverage-probe")?.remove());
  await expectCovered(page, "sidebar", ".app-sidebar", 8);

  await openRowMenu(row);
  await expectCovered(page, "session row menu", ".session-row-menu-panel", 5);
  await page.locator(".session-row-delete").click();
  await expect(page.locator(".confirm-delete")).toBeVisible();
  await expectCovered(page, "delete confirmation", ".session-row-menu-panel", 2);
  await page.locator(".confirm-cancel").click();
  await expect(page.locator(".confirm-delete")).toHaveCount(0);

  await openRowMenu(row);
  await page.locator(".session-row-rename").click();
  await expect(page.locator(".rename-dialog .rename-form")).toBeVisible();
  await expectCovered(page, "rename dialog", ".rename-dialog", 2);
  await page.locator(".rename-cancel").click();
  await expect(page.locator(".rename-dialog")).toHaveCount(0);

  const localRow = page.locator(`.host-row[data-host-id="${await localHostId(request)}"]`);
  await openHostMenu(localRow);
  await expectCovered(page, "host row menu", ".host-row-menu-panel", 3);
  await page.locator(".host-settings").click();
  await expect(page.locator(".host-settings-dialog")).toBeVisible();
  await expectCovered(page, "host settings dialog", ".host-settings-dialog", 2);
  await page.locator(".host-settings-close").first().click();
  await expect(page.locator(".host-settings-dialog")).toHaveCount(0);

  await page.locator(".add-host-button").click();
  await expect(page.locator(".add-host-submit")).toBeVisible();
  await expectCovered(page, "add host form", ".add-host-form", 1);
  await page.locator(".add-host-cancel").click();
  await expect(page.locator(".add-host-submit")).toHaveCount(0);

  await page.locator(".app-help-toggle").click();
  const help = page.getByRole("menu", { name: "help", exact: true });
  await expect(help).toBeVisible();
  await expectCovered(page, "help menu", '[data-bar-menu="help"] [role="menu"]', 2);
  await help.locator('[data-bar-menu-item="send feedback"]').click();
  const feedback = page.getByRole("dialog", { name: "send feedback", exact: true });
  await expect(feedback).toBeVisible();
  await feedback.locator(".feedback-contact").fill("contact@example.test");
  await expect(feedback.locator(".feedback-reuse")).toBeChecked();
  await expectCovered(page, "feedback dialog", ".feedback-dialog", 3);
  await feedback.locator(".feedback-cancel").click();
  await expect(feedback).toHaveCount(0);

  await page.locator(".app-settings-toggle").click();
  await expect(page.locator(".app-settings-dialog")).toBeVisible();
  await expectCovered(page, "settings dialog", ".app-settings-dialog", 3);
  await page.locator(".app-settings-close").click();
  await expect(page.locator(".app-settings-dialog")).toHaveCount(0);

  await page.getByRole("button", { name: "templates", exact: true }).click();
  const templates = page.locator('.templates-dialog[role="dialog"]');
  await expect(templates).toBeVisible();
  await expectCovered(page, "templates dialog", ".templates-dialog", 2);
  await templates.locator(".templates-new").click();
  await templates.locator(".templates-add").click();
  await expect(templates.getByRole("group", { name: "add field", exact: true })).toBeVisible();
  await expectCovered(page, "templates add-field menu", ".templates-add-menu", 3);
  await templates.locator(".templates-add-menu").getByRole("button", { name: "agent type", exact: true }).click();
  await templates.getByLabel("agent type", { exact: true }).selectOption("codex");
  for (const name of ["model", "effort", "approvals", "workspace trust"]) {
    await templates.locator(".templates-add").click();
    await templates.locator(".templates-add-menu").getByRole("button", { name, exact: true }).click();
  }
  await expectCovered(page, "templates agent editor", ".templates-editor", 10);
  await templates.locator(".templates-close").click();
  await expect(templates.locator(".templates-departure")).toBeVisible();
  await expectCovered(page, "templates unsaved departure", ".templates-departure", 3);
  await templates.locator(".templates-discard").click();
  await expect(templates).toHaveCount(0);

  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await expectCovered(page, "new session form, agent tab", ".create-session-form", 8);
  // Picking an agent reveals the agent-dependent choices (model, effort,
  // permissions, workspace trust), and the search box offers results; both
  // carry hover text of their own. Codex is picked because it offers every
  // one of those choices.
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  await expect(form.locator(".launch-composer-trust-choice")).toBeVisible();
  await expectCovered(page, "new session form, Codex picked", ".create-session-form", 15);
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  for (const query of ["perms:", "trust:", "effort:"]) {
    await search.fill(query);
    await expect(page.locator("#launch-composer-search-results [role=option]").first()).toBeVisible();
    await expectCovered(page, `new session form, search "${query}"`, "#launch-composer-search-results", 2);
  }
  await search.fill("");
  await form.getByRole("tab", { name: "command", exact: true }).click();
  await expect(form.getByRole("tab", { name: "command", exact: true })).toHaveAttribute("aria-selected", "true");
  await expectCovered(page, "new session form, command tab", ".create-session-form", 5);
  await form.locator(".launch-composer-cancel").click();
  await expect(form).toHaveCount(0);

  // The compact density drops the metadata line and moves the directory
  // into the open button's own hover text.
  await patchPreferences(request, { compact: true });
  await page.reload();
  await expect(row).toBeVisible({ timeout: 20_000 });
  await expect(row.locator(".session-row-meta")).toHaveCount(0);
  await expectCovered(page, "compact sidebar", ".app-sidebar", 8);
});

/**
 * The open session's header, its tab strip with an extra terminal tab, and
 * the terminal's text-size controls.
 *
 * Why: the header packs the session's lifecycle actions into one row of
 * short labels (restart, replace, clone), which is exactly where a user
 * needs to know what a button will do before clicking it.
 */
test("the session view gives every control hover text", async ({ page }) => {
  test.setTimeout(120_000);
  await openTerminal(page);
  await addTab(page, 0);
  await expectCovered(page, "session view with a terminal tab", ".app-main .layout", 10);
  await page.locator(".header-delete").click();
  await expect(page.locator(".header-delete-confirm-submit")).toBeVisible();
  await expectCovered(page, "header delete confirmation", ".header-delete-confirm", 2);
});
