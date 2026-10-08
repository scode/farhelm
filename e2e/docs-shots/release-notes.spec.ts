/**
  * Screenshots for the docs website's Release notes page: the visual changes of
 * a release, each shown under the entry it belongs to behind a "show
 * screenshot" link, as website/src/release-notes/<version>.json attaches them
 * (website/AGENTS.md, "Release notes").
 *
 * Each test is one release's shot, named release-notes/v<X-Y-Z>-<what>, and
 * is meant to show the release it is named for. The capture cannot keep them
 * that way yet: a refresh recaptures them from the UI of the day like every
 * other shot, so once a later release changes one of these dialogs, its shot
 * needs the maintainer's decision rather than a rewrite to the new look
 * (website/AGENTS.md, "Release notes"). Like every docs shot, nothing here
 * launches a session, sends feedback, or saves a setting.
 */
import { expect, test, type Page } from "@playwright/test";
import { open } from "./open";
import { shot } from "./shot";

const PAGE = "release-notes";

/** v0.25.0: Cmd+K / Ctrl+Shift+K finds a session on any host. */
test("v0.25.0 quick switcher", async ({ page, request }) => {
  // Narrow enough that the whole dialog fits the docs' text column.
  await page.setViewportSize({ width: 930, height: 1000 });
  const { director } = await open(page, request);
  // The staged fleet is the web UI on Linux, so the switcher's shortcut is
  // Ctrl+Shift+K here; on a Mac it is Cmd+K, as the callout says.
  await page.keyboard.press("Control+Shift+K");
  const dialog = page.locator(".quick-switcher-dialog");
  await expect(dialog).toBeVisible();
  const search = dialog.locator(".quick-switcher-search");
  // Opening by keyboard gives the search box keyboard focus, which shows its
  // hover text over the dialog. Clicking into it makes the focus a pointer
  // focus, which hides that text the way a click does in the app.
  await search.click();
  await expect(page.locator(".farhelm-tooltip").filter({ visible: true })).toHaveCount(0);
  await search.fill("tok");
  await expect(dialog.locator(".quick-switcher-row").first()).toBeVisible();
  // One callout, under the dialog: the dialog spans the docs' text column,
  // so a callout beside it would cover the results it is about.
  await director.callout(
    dialog,
    "Cmd+K on a Mac, Ctrl+Shift+K elsewhere, even from inside a terminal. Type part of a session's name; Enter jumps to it on whichever host it runs, and Escape takes you back.",
    { side: "bottom", dy: 10 },
  );
  await shot(page, `${PAGE}/v0-25-0-quick-switcher`, [dialog], { maxWidth: 930 });
});

/** Open the Templates dialog once its staged list has loaded (as on the Launch templates page). */
async function openTemplates(page: Page) {
  await page.locator(".templates-button").click();
  const dialog = page.locator(".templates-dialog");
  await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
  await expect(dialog.locator(".templates-row").first()).toBeVisible();
  return dialog;
}

/** v0.25.0: the redesigned Templates dialog. */
test("v0.25.0 templates dialog", async ({ page, request }) => {
  await page.setViewportSize({ width: 930, height: 1100 });
  const { director } = await open(page, request);
  const dialog = await openTemplates(page);
  await dialog.locator(".templates-row").filter({ has: page.getByText("codex-deep", { exact: true }) }).click();
  await expect(dialog.locator(".templates-editor-heading h3")).toHaveText("edit template");
  // Focus the dialog itself so the name field's help does not enter the crop.
  await dialog.focus();
  // Inside the list's empty lower half, under its last row, where it covers
  // nothing; the other two sit in the left margin beside what they explain.
  await director.callout(
    dialog.locator(".templates-row").last(),
    "Your templates, listed beside the one you are editing.",
    { side: "bottom", dy: 60 },
  );
  await director.callout(
    dialog.locator(".templates-add"),
    "Each setting is a field you add or remove; one the template leaves out is not shown.",
    { side: "left", dy: 40 },
  );
  await director.callout(
    dialog.getByRole("button", { name: "duplicate", exact: true }),
    "Duplicate a template, or delete it and undo for about ten seconds. Nothing is saved until you press save.",
    { side: "left", dy: 90 },
  );
  await shot(page, `${PAGE}/v0-25-0-templates-dialog`, [dialog], { maxWidth: 930 });
});

/** v0.25.0: Send feedback can remember how to reach you. */
test("v0.25.0 feedback contact", async ({ page, request }) => {
  const { director } = await open(page, request);
  await page.locator(".app-help-toggle").click();
  await page.locator('[data-bar-menu-item="send feedback"]').click();
  const dialog = page.locator(".feedback-dialog");
  await expect(dialog).toBeVisible();
  // The checkbox appears once there is a contact to remember. Typing one
  // sends nothing; only the dialog's send button would.
  await dialog.locator(".feedback-contact").fill("you@example.com");
  const reuse = dialog.locator(".feedback-reuse");
  await expect(reuse).toBeVisible();
  await director.callout(
    reuse,
    "Ticked, your helm keeps the contact and fills it in next time, in the Mac app and the browser alike.",
    { side: "left" },
  );
  // Cropped above the "sent with it" list, which names the capturing
  // browser's operating system rather than one a reader would recognise.
  await shot(page, `${PAGE}/v0-25-0-feedback-contact`, [dialog.locator(".host-settings-title"), reuse], {
    pad: 12,
    maxWidth: 930,
  });
});
