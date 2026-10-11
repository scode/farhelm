/**
 * Screenshots for the docs website's Release notes page: the visual changes of
 * a release, each shown under the entry it belongs to behind a "show
 * screenshot" link, as website/src/release-notes/<version>.json attaches them
 * (website/AGENTS.md, "Release notes").
 *
 * Each test is one release's shot, named release-notes/v<X-Y-Z>-<what>, and
 * shows the release it is named for. Once published, a shot is frozen: every
 * test here calls skipFrozenReleaseShot first, so a capture leaves it alone
 * and the publish carries the published image over, however much the UI has
 * moved since (docs/docs-shots/SPEC.md, "Release shots are frozen"). That is
 * why these tests are not updated in a screenshot refresh, a sweep, or a
 * rename: a published test's steps only matter again for a deliberate
 * correction, made with `--recapture-release-shot`. A new release's shots are
 * added here and captured normally until their first publish. Like every docs
 * shot, nothing here launches a session, sends feedback, or saves a setting.
 */
import { expect, test, type Page } from "@playwright/test";
import { hostRowByName, openHostMenu } from "../tests/helpers/fleet";
import { open, openLauncher } from "./open";
import { skipFrozenReleaseShot } from "./release-shots";
import { shot } from "./shot";

const PAGE = "release-notes";

/** v0.25.0: Cmd+K / Ctrl+Shift+K finds a session on any host. */
test("v0.25.0 quick switcher", async ({ page, request }) => {
  const name = `${PAGE}/v0-25-0-quick-switcher`;
  skipFrozenReleaseShot(name);
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
  await shot(page, name, [dialog], { maxWidth: 930 });
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
  const name = `${PAGE}/v0-25-0-templates-dialog`;
  skipFrozenReleaseShot(name);
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
  await shot(page, name, [dialog], { maxWidth: 930 });
});

/** v0.25.0: Send feedback can remember how to reach you. */
test("v0.25.0 feedback contact", async ({ page, request }) => {
  const name = `${PAGE}/v0-25-0-feedback-contact`;
  skipFrozenReleaseShot(name);
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
  await shot(page, name, [dialog.locator(".host-settings-title"), reuse], {
    pad: 12,
    maxWidth: 930,
  });
});

// v0.26.1. Two pairs below photograph one dialog each (Settings, and a remote
// host's settings) under different entries, cropped to the part each entry is
// about, so a reader opening one entry's screenshot sees that change rather
// than a whole dialog with the change somewhere in it.

/** Open Settings from the app bar and wait for its sound switches, which mount after the dialog. */
async function openSettings(page: Page) {
  await page.locator(".app-settings-toggle").click();
  const dialog = page.locator(".app-settings-dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog.locator("[data-sound-event]")).toHaveCount(3);
  return dialog;
}

/**
 * Open a remote host's settings. Only remote hosts have the Appearance
 * section, so the scenario's first remote host is the one shown.
 */
async function openRemoteHostSettings(page: Page, scenario: { hosts: { kind: string; alias: string }[] }) {
  const remote = scenario.hosts.find((host) => host.kind === "remote");
  if (!remote) throw new Error("the docs scenario needs a remote host");
  const row = hostRowByName(page, remote.alias);
  await openHostMenu(row);
  await row.locator(".host-settings").click();
  const dialog = page.locator(".host-settings-dialog");
  await expect(dialog.locator(".host-settings-appearance")).toBeVisible();
  return dialog;
}

/** v0.26.1: Settings can play a sound when a session needs you. */
test("v0.26.1 sounds", async ({ page, request }) => {
  skipFrozenReleaseShot(`${PAGE}/v0-26-1-sounds`);
  await page.setViewportSize({ width: 1300, height: 1300 });
  const { director } = await open(page, request);
  const dialog = await openSettings(page);
  const sounds = dialog.getByRole("region", { name: "sounds" });
  // Right of the switches, which sit at the dialog's right edge, so the
  // callouts cover none of the labels.
  await director.callout(
    sounds.locator('[data-sound-event="waiting"]'),
    "A bell when a session starts waiting for your answer, and a ding-dong when an agent asks for your approval. Both start on.",
    { side: "right", dy: -10 },
  );
  await director.callout(
    sounds.locator('[data-sound-event="finished"]'),
    "A soft pluck when an agent finishes. Off until you turn it on.",
    { side: "right", dy: 85 },
  );
  // The heading and the switches rather than the whole section, whose top
  // margin would pull the checkout folder's save button into the crop.
  await shot(page, `${PAGE}/v0-26-1-sounds`, [sounds.locator("h3"), sounds.locator(".app-settings-choice").last()], {
    pad: 12,
    maxWidth: 930,
  });
});

/** v0.26.1: where Farhelm's GitHub checkouts go, set in Settings. */
test("v0.26.1 checkout folder", async ({ page, request }) => {
  skipFrozenReleaseShot(`${PAGE}/v0-26-1-checkout-folder`);
  await page.setViewportSize({ width: 1300, height: 1300 });
  const { director } = await open(page, request);
  const dialog = await openSettings(page);
  const field = dialog.locator(".checkout-root-field");
  const folder = field.getByRole("textbox", { name: "checkout folder", exact: true });
  // Typed to show what goes in the field. Nothing is saved: only the field's
  // own save button would write it.
  await folder.fill("~/checkouts");
  await director.callout(
    folder,
    "Where every host puts the GitHub checkouts Farhelm makes for new sessions; ~ is each host's own home. Until it is set, the session launcher offers this field when you choose a managed checkout.",
    { side: "left", dy: 20 },
  );
  await shot(page, `${PAGE}/v0-26-1-checkout-folder`, [field], { maxWidth: 930 });
});

/** v0.26.1: a remote host's own icon and color. */
test("v0.26.1 host appearance", async ({ page, request }) => {
  skipFrozenReleaseShot(`${PAGE}/v0-26-1-host-appearance`);
  await page.setViewportSize({ width: 1300, height: 1300 });
  const { scenario, director } = await open(page, request);
  const dialog = await openRemoteHostSettings(page, scenario);
  const appearance = dialog.locator(".host-settings-appearance");
  // All three in the margin left of the dialog: to the right, the crop would
  // pass the docs' text column and cut them off.
  await director.callout(
    appearance.locator(".host-appearance-icons"),
    "Pick an icon for the host. A click saves it straight away.",
    { side: "left" },
  );
  await director.callout(appearance.locator(".host-appearance-colors"), "And a color for it.", { side: "left", dy: 10 });
  await director.callout(
    appearance.locator(".host-appearance-preview-row"),
    "The host's sessions show this mark in the session list, so you can tell hosts apart at a glance.",
    { side: "left", dy: 30 },
  );
  await shot(page, `${PAGE}/v0-26-1-host-appearance`, [appearance], { maxWidth: 930 });
});

/** v0.26.1: host settings in sections, with switches. */
test("v0.26.1 host settings", async ({ page, request }) => {
  skipFrozenReleaseShot(`${PAGE}/v0-26-1-host-settings`);
  await page.setViewportSize({ width: 1300, height: 1300 });
  const { scenario, director } = await open(page, request);
  const dialog = await openRemoteHostSettings(page, scenario);
  // One callout per section, all in the left margin for the same reason as
  // the appearance shot above.
  await director.callout(
    dialog.getByRole("region", { name: "Connection" }),
    "How Farhelm reaches the host, and the name it goes by.",
    { side: "left" },
  );
  await director.callout(dialog.getByRole("region", { name: "Appearance" }), "The host's icon and color.", {
    side: "left",
  });
  await director.callout(
    dialog.getByRole("region", { name: "Permissions" }),
    "What Farhelm asks you before doing on this host, each a switch now.",
    { side: "left" },
  );
  await shot(page, `${PAGE}/v0-26-1-host-settings`, [dialog], { maxWidth: 930 });
});

/** v0.26.1: the session launcher saves its current setup as a template. */
test("v0.26.1 save as template", async ({ page, request }) => {
  skipFrozenReleaseShot(`${PAGE}/v0-26-1-save-template`);
  // Narrow enough that the launcher's buttons and the save panel fit the
  // docs' text column.
  await page.setViewportSize({ width: 930, height: 1100 });
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  await form.locator(".launch-composer-harness-choice button", { hasText: /^Claude$/ }).click();
  const button = form.locator(".launch-composer-save-template");
  await button.click();
  const panel = form.locator(".save-template-panel");
  await expect(panel).toBeVisible();
  const name = panel.locator(".save-template-name");
  // The panel gives the name field keyboard focus, which shows its hover
  // text over the checklist. Clicking into it makes the focus a pointer
  // focus, which hides that text the way a click does in the app.
  await name.click();
  await expect(page.locator(".farhelm-tooltip").filter({ visible: true })).toHaveCount(0);
  // A name typed to show what goes there. Nothing is saved: only the
  // panel's save button would create the template.
  await name.fill("claude-here");
  await director.highlight(button, { pad: 3 });
  // In the empty right half of the panel, beside the checklist it explains:
  // dx carries it past the end of the panel's instruction line, and dy keeps
  // it below that line.
  await director.callout(
    panel.locator(".save-template-choice .peer-value").first(),
    "Tick what the template keeps; what you chose yourself starts ticked. Saving it launches nothing.",
    { side: "right", dx: 380, dy: 40 },
  );
  await shot(page, `${PAGE}/v0-26-1-save-template`, [button, panel], { maxWidth: 930 });
});

// v0.27.0. Two of the shots below photograph the host dialogs that one
// changelog entry redesigned (#1866 and #1867), one dialog each, so a reader
// sees both the setup and the removal question in their new layout.

/** v0.27.0: the post-clone command, set for every host in Settings. */
test("v0.27.0 post-clone command", async ({ page, request }) => {
  const name = `${PAGE}/v0-27-0-post-clone-command`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1300 });
  const { director } = await open(page, request);
  const dialog = await openSettings(page);
  const field = dialog.locator(".checkout-post-clone-field");
  const command = field.locator("textarea");
  // Typed to show what goes in the field. Nothing is saved: only the
  // field's own save button would write it, so the fleet's command stays
  // unset for every other shot.
  await command.fill("npm ci");
  // The field's own help says when the command runs, so the callout says
  // what is new instead.
  await director.callout(
    command,
    "One command for every host, now set here rather than from the command line on the helm's machine.",
    { side: "left", dy: 20 },
  );
  // A narrow pad keeps the checkout folder's save button, just above the
  // field, out of the crop.
  await shot(page, name, [field], { pad: 8, maxWidth: 930 });
});

/**
 * v0.27.0: one checkout deleted from the trash on its own. The trash is the
 * scenario's (`checkoutTrash`), and the shot stops before any click: the
 * button deletes without asking, and the staging refuses deletion anyway.
 */
test("v0.27.0 trash delete one", async ({ page, request }) => {
  const name = `${PAGE}/v0-27-0-trash-delete-one`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 960, height: 1150 });
  const { director } = await open(page, request, { checkoutTrash: true });
  await page.locator(".checkout-trash-button").click();
  const dialog = page.locator(".checkout-trash-dialog");
  await expect(dialog.locator(".checkout-trash-entry")).toHaveCount(3);
  // The last checkout's button, the one nearest the dialog's bottom edge:
  // a callout beside the button would widen the crop past the docs' text
  // column, and one under the dialog reaches this button across nothing but
  // the empty end of its "on disk" line.
  const button = dialog.locator(".trash-entry-delete").last();
  await expect(button).toBeEnabled();
  await director.highlight(button, { pad: 3 });
  await director.callout(
    button,
    "Deletes just this checkout, at once and without asking. The host's delete and delete all still ask first.",
    { side: "bottom", dx: -120, dy: 60 },
  );
  await shot(page, name, [dialog], { maxWidth: 930 });
});

/** v0.27.0: the session launcher's folder field opens a dropdown. */
test("v0.27.0 folder dropdown", async ({ page, request }) => {
  const name = `${PAGE}/v0-27-0-folder-dropdown`;
  skipFrozenReleaseShot(name);
  // The default width, which leaves the left margin room for the callouts,
  // and tall enough that the whole dropdown, which floats over the launcher's
  // lower half, stays inside the viewport and so inside the crop.
  await page.setViewportSize({ width: 1600, height: 1300 });
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  const destination = form.locator(".launch-composer-destination");
  // Focus opens the dropdown, the way clicking into the field does.
  await destination.getByRole("combobox", { name: "folder", exact: true }).focus();
  const folders = destination.getByRole("listbox", { name: "folders", exact: true });
  await expect(folders).toBeVisible();
  // The dropdown floats past the end of the launcher's scrolling body, which
  // clips it; scrolling its last entry into view is what a person does to
  // reach it, and shows the whole list.
  await folders.getByRole("option", { name: "reset destination to local home" }).scrollIntoViewIfNeeded();
  // Both in the left margin, each level with the group it explains.
  await director.callout(
    folders.getByRole("group", { name: "recent" }),
    "Directories you launched in recently on this host, newest first. Typing in the field filters them.",
    { side: "left" },
  );
  await director.callout(
    folders.getByRole("group", { name: "reset" }),
    "Your home directory on the host, or on the machine in front of you.",
    { side: "left", dy: 20 },
  );
  await shot(page, name, [destination.getByRole("combobox", { name: "folder", exact: true }), folders], {
    maxWidth: 930,
  });
});

/**
 * v0.27.0: the host setup question in the shared host dialog layout. The
 * add-host check answers for the scenario's invented add_host (as on the Add
 * a remote host page); no shot confirms the setup.
 */
test("v0.27.0 host setup dialog", async ({ page, request }) => {
  const name = `${PAGE}/v0-27-0-host-setup`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1100 });
  const { scenario, director } = await open(page, request);
  if (!scenario.add_host) throw new Error("the docs scenario needs an add_host");
  await page.locator(".add-host-button").click();
  const dialog = page.locator(".host-add-dialog");
  await expect(dialog).toBeVisible();
  await dialog.locator(".add-host-ssh").fill(scenario.add_host.ssh);
  await dialog.locator(".add-host-submit").click();
  await expect(dialog.locator(".provisioning-plan")).toContainText(scenario.add_host.ssh);
  await expect(dialog.locator(".provisioning-confirm")).toBeVisible();
  // All in the left margin, level with the part of the dialog each explains.
  await director.callout(dialog.locator(".host-settings-header"), "A heading names the host being set up.", {
    side: "left",
  });
  await director.callout(
    dialog.locator(".provisioning-plan"),
    "A long plan scrolls on its own, while the heading and the answers stay in view.",
    { side: "left" },
  );
  // Under the answers rather than beside them, so it covers none of them.
  await director.callout(
    dialog.locator(".host-setup-permanent"),
    "The answers say what they do: set up, and set up, don't ask again. They were yes, and yes, and don't ask in the future.",
    { side: "bottom", dx: -120, dy: 40 },
  );
  await shot(page, name, [dialog], { maxWidth: 930 });
});

/** v0.27.0: the remove host question in the shared host dialog layout. */
test("v0.27.0 host remove dialog", async ({ page, request }) => {
  const name = `${PAGE}/v0-27-0-host-remove`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1100 });
  const { scenario, director } = await open(page, request);
  const remote = scenario.hosts.find((host) => host.kind === "remote");
  if (!remote) throw new Error("the docs scenario needs a remote host");
  const row = hostRowByName(page, remote.alias);
  await openHostMenu(row);
  await row.locator(".host-remove").click();
  const dialog = page.locator(".host-remove-dialog");
  await expect(dialog).toBeVisible();
  // One in the left margin, one under the answers, so neither covers the
  // explanation or an answer.
  await director.callout(
    dialog.locator(".host-settings-header"),
    "The host and whether it is connected head the dialog, as in host settings.",
    { side: "left" },
  );
  await director.callout(
    dialog.locator(".host-dialog-answers .btn-outline"),
    "Renamed from remove, and don't ask again.",
    { side: "bottom", dx: -60, dy: 40 },
  );
  await shot(page, name, [dialog], { maxWidth: 930 });
});
