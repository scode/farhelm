/**
 * Screenshots for the docs website's Release notes page: the visual changes of
 * a release, each shown under the entry it belongs to behind a "show
 * screenshot" link, as website/src/release-notes/<version>.json attaches them
 * (`website/.agents/release-notes.md`, "Release notes").
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
import { hostRowByName, openHostMenu, openRowMenu } from "../tests/helpers/fleet";
import { open, openLauncher, rowOf } from "./open";
import { skipFrozenReleaseShot } from "./release-shots";
import { shot } from "./shot";

const PAGE = "release-notes";

/** The image choice is a draft: photographing it neither pulls nor starts an image. */
test("v0.33.0 container image", async ({ page, request }) => {
  const name = `${PAGE}/v0-33-0-container-image`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 930, height: 1800 });
  const { director } = await open(page, request);
  const form = await openLauncherWithClaude(page);
  await form.locator(".destination-type-control button", { hasText: "Managed checkout" }).click();
  const choice = form.locator(".launch-composer-container-choice");
  await choice.getByRole("checkbox", { name: "Containerized", exact: true }).check();
  const image = choice.getByRole("textbox", { name: "Image", exact: true });
  await expect(image).toBeVisible();
  await image.fill("localhost/agent:latest");
  await choice.scrollIntoViewIfNeeded();
  await form.focus();
  await director.callout(image, "Name an image already in this host's local Podman storage. Farhelm does not pull images.", { side: "bottom", dy: 100 });
  await shot(page, name, [choice], { maxWidth: 930 });
});

/** Help shows the actual client's keys and bindings without changing either. */
test("v0.33.0 keyboard shortcuts", async ({ page, request }) => {
  const name = `${PAGE}/v0-33-0-keyboard-shortcuts`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 930, height: 1600 });
  const { director } = await open(page, request);
  await page.getByRole("button", { name: "Help", exact: true }).click();
  await page.getByRole("menu", { name: "Help", exact: true }).locator('[data-bar-menu-item="Keyboard shortcuts"]').click();
  const dialog = page.getByRole("dialog", { name: "Keyboard shortcuts", exact: true });
  await expect(dialog).toBeVisible();
  await dialog.focus();
  await director.callout(dialog.locator(".keyboard-shortcuts-section").last(), "See your keyboard shortcuts and the templates assigned to quick launch and number keys.", { side: "bottom", dy: 50 });
  await shot(page, name, [dialog], { maxWidth: 930 });
});

/** Stop before Enter: composing template names must not start a session. */
test("v0.33.0 typed template combinations", async ({ page, request }) => {
  const name = `${PAGE}/v0-33-0-template-names`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 930, height: 1100 });
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  await expect(form.locator('.launch-composer-search input[role="combobox"]')).toBeFocused();
  await page.keyboard.press("Control+Shift+KeyX");
  await page.keyboard.type("codex-deep api");
  const indicator = page.locator(".launch-sequence-indicator");
  await expect(indicator).toBeVisible();
  await expect(indicator.locator(".launch-sequence-candidates")).toContainText("api-repo");
  await director.callout(indicator, "Combine templates by typing their names separated by spaces. Tab completes a matching name.", { side: "top", dy: -60 });
  await shot(page, name, [indicator], { maxWidth: 930 });
  await page.keyboard.press("Escape");
});

/** Expand the navigation beside the list; the shot changes only this device's layout. */
test("v0.33.0 expanded navigation rail", async ({ page, request }) => {
  const name = `${PAGE}/v0-33-0-navigation-rail`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 800 });
  const { director } = await open(page, request);
  // The label changes with the layout; keep the locator on the same control.
  const toggle = page.locator("#host-rail-toggle");
  await expect(toggle).toHaveAccessibleName("Expand navigation rail");
  await toggle.click();
  await expect(toggle).toHaveAccessibleName("Collapse navigation rail");
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  await director.callout(toggle, "Expand or collapse the navigation rail beside the list. This device remembers your choice.", { side: "right", dy: -100 });
  // The rail's footer is a separate region; keep the annotated toggle in the crop.
  await shot(page, name, [page.locator(".host-rail"), page.locator(".session-list"), toggle], { maxWidth: 930 });
});

/** Saved controls are photographed without marking a template or saving an edit. */
test("v0.33.0 template controls", async ({ page, request }) => {
  const name = `${PAGE}/v0-33-0-template-controls`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 930, height: 1100 });
  const { director } = await open(page, request);
  await page.locator(".templates-button").click();
  const dialog = page.locator(".templates-dialog");
  await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
  const row = dialog.locator(".templates-row").filter({ has: page.getByText("codex-deep", { exact: true }) });
  await row.click();
  const save = dialog.locator(".templates-save");
  await expect(save).toHaveAttribute("aria-disabled", "true");
  await dialog.focus();
  // The bolt belongs to the list item beside its separate template-name button.
  const bolt = row.locator("..").locator(".templates-quick-mark");
  await expect(bolt).toBeVisible();
  await expect(bolt).toHaveAttribute("aria-pressed", "false");
  await director.callout(bolt.locator(".templates-quick-icon"), "An outline bolt means this template is not marked for quick launch.", { side: "bottom", dy: 100 });
  await director.callout(save, "Save is greyed out when there are no unsaved changes.", { side: "top", dx: -100, dy: -80 });
  await shot(page, name, [dialog], { maxWidth: 930 });
});

// v0.32.0 captions retain the release-note author's wording. These shots
// change only local launcher drafts and navigation; no setting is saved and
// no session starts. Device appearance follows Playwright's media preference.

/** Show the new host rail and status choices beside the real staged list. */
test("v0.32.0 host and status filters", async ({ page, request }) => {
  const name = `${PAGE}/v0-32-0-host-status-filters`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1000 });
  const { director } = await open(page, request);
  const rail = page.locator(".host-rail");
  const statuses = page.getByRole("radiogroup", { name: "Session status", exact: true });
  await expect(rail).toBeVisible();
  await expect(statuses.getByRole("radio")).toHaveCount(4);
  await director.callout(statuses,
    "The session list can now be filtered by host, from a new host rail beside it that shows each host's mark and state, and by status, choosing All, Running, Waiting or Idle below the list's title.",
    { side: "right", dy: 50 });
  await shot(page, name, [rail.locator(".host-rail-all"), rail.locator(".host-rail-avatar").last(), page.locator(".session-list")], { maxWidth: 930 });
});

/** Photograph the device's light appearance without persisting an override. */
test("v0.32.0 appearance", async ({ page, request }) => {
  const name = `${PAGE}/v0-32-0-appearance`;
  skipFrozenReleaseShot(name);
  await page.emulateMedia({ colorScheme: "light" });
  await page.setViewportSize({ width: 1300, height: 1000 });
  const { director } = await open(page, request);
  await page.locator(".app-settings-toggle").click();
  const dialog = page.locator(".app-settings-dialog");
  await dialog.getByRole("navigation").getByRole("button", { name: "Appearance", exact: true }).click();
  const appearance = dialog.locator("#settings-appearance");
  await expect(appearance.getByRole("radio", { name: "System", exact: true })).toBeChecked();
  await expect(page.locator("html")).toHaveAttribute("data-appearance", "light");
  await dialog.focus();
  await director.callout(appearance,
    "Farhelm now has a light appearance. A new Appearance section in Settings follows your device's appearance with System, or overrides it on that device with Dark or Light.",
    { side: "left", dy: 60 });
  await shot(page, name, [appearance], { maxWidth: 930 });
});

/** Show the background-update opt-in at its real default, leaving it off. */
test("v0.32.0 automatic host updates", async ({ page, request }) => {
  const name = `${PAGE}/v0-32-0-automatic-host-updates`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1100 });
  const { director } = await open(page, request);
  await page.locator(".app-settings-toggle").click();
  const dialog = page.locator(".app-settings-dialog");
  await dialog.getByRole("navigation").getByRole("button", { name: "Hosts", exact: true }).click();
  const choice = dialog.getByRole("checkbox", { name: "Update remote hosts automatically", exact: true });
  await expect(choice).toBeEnabled();
  await expect(choice).not.toBeChecked();
  await dialog.focus();
  await director.callout(choice.locator(".."),
    "Settings has Update remote hosts automatically, off by default, which updates remote hosts even with no window open. A failed automatic update is not retried until the host reconnects or the helm restarts.",
    { side: "left", dy: 60 });
  await shot(page, name, [choice.locator("..")], { maxWidth: 930 });
});

/** Host management now replaces the list, rather than covering the terminal. */
test("v0.32.0 hosts panel", async ({ page, request }) => {
  const name = `${PAGE}/v0-32-0-hosts-panel`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 900 });
  const { director } = await open(page, request);
  await page.locator(".hosts-panel-toggle").click();
  const hosts = page.locator(".host-row");
  await expect(hosts.first()).toBeVisible();
  await director.callout(hosts.first(), "Hosts now opens the host management panel in place of the session list.",
    { side: "right", dy: 30 });
  await shot(page, name, [page.locator(".host-rail-all"), page.locator(".host-rail-avatar").last(), hosts.first(), hosts.last()], { maxWidth: 930 });
});

/** The toolbar is shown with the live terminal, so its persistent controls have context. */
test("v0.32.0 session toolbar", async ({ page, request }) => {
  const name = `${PAGE}/v0-32-0-session-toolbar`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1270, height: 850 });
  const { director } = await open(page, request);
  const topbar = page.locator("header.session-topbar");
  await expect(topbar).toBeVisible();
  await director.callout(topbar,
    "The toolbar at the top of an open session uses icon-and-label controls, separate menus for Restart and Replace, and zoom buttons showing the terminal text size in points, which stay visible when tabs scroll.",
    { side: "bottom", dx: 100, dy: 110 });
  await shot(page, name, [topbar], { maxWidth: 930 });
});

/** Show the launcher's grouped fields and sticky footer, without launching. */
test("v0.32.0 session launcher", async ({ page, request }) => {
  const name = `${PAGE}/v0-32-0-session-launcher`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 930, height: 1800 });
  const { director } = await open(page, request);
  const form = await openLauncherWithClaude(page);
  // The scenario's displayed checkout is not a real host directory. A home
  // draft gives the ordinary layout a valid destination without staging an
  // invented validation reply or launching anything.
  const validation = page.waitForResponse((response) => response.url().endsWith("/api/browse-directory")
    && response.request().method() === "POST" && response.request().postDataJSON().cwd === "~"
    && response.request().postDataJSON().membership_only === true);
  await form.getByRole("combobox", { name: "Folder", exact: true }).fill("~");
  expect((await validation).ok()).toBe(true);
  await expect(form).toHaveAttribute("data-folder-check-pending", "false");
  await expect(form.locator(".launch-composer-folder-warning")).toHaveCount(0);
  await form.focus();
  await director.callout(form,
    "the session launcher groups its fields into labeled rows",
    { side: "bottom", dy: 10 });
  await shot(page, name, [form], { maxWidth: 930 });
});

/** Container inputs are launcher drafts; enabling them here starts no container. */
test("v0.32.0 containerized launcher", async ({ page, request }) => {
  const name = `${PAGE}/v0-32-0-containerized-launcher`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 930, height: 1800 });
  const { director } = await open(page, request);
  const form = await openLauncherWithClaude(page);
  await form.locator(".destination-type-control button", { hasText: "Managed checkout" }).click();
  const choice = form.locator(".launch-composer-container-choice");
  const toggle = choice.getByRole("checkbox", { name: "Containerized", exact: true });
  await expect(toggle).toBeEnabled();
  await toggle.check();
  await expect(choice.getByRole("textbox", { name: "Image file", exact: true })).toBeVisible();
  await choice.scrollIntoViewIfNeeded();
  await form.focus();
  await director.callout(choice,
    "Agent sessions on Linux hosts can now run in a rootless Podman container, using an image you provide and a local API proxy, by turning on Containerized in the session launcher for a managed checkout.",
    { side: "bottom", dy: 20 });
  await shot(page, name, [choice], { pad: 6, maxWidth: 930 });
});

/**
 * v0.31.0: the sidebar makes Settings' sections visible without a long scroll.
 * Only navigation changes here; no switch is toggled and no field is saved.
 */
test("v0.31.0 settings sections", async ({ page, request }) => {
  const name = `${PAGE}/v0-31-0-settings-sections`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 930, height: 1300 });
  const { director } = await open(page, request);
  await page.locator(".app-settings-toggle").click();
  const dialog = page.locator(".app-settings-dialog");
  const navigation = dialog.getByRole("navigation", { name: "settings sections", exact: true });
  await expect(navigation).toBeVisible();
  await navigation.getByRole("button", { name: "sessions", exact: true }).click();
  await director.callout(
    navigation,
    "Choose a section here. Moving between sections keeps your unsaved fields; closing Settings discards them.",
    { side: "bottom", dy: 110 },
  );
  await shot(page, name, [dialog], { maxWidth: 930 });
});

/**
 * v0.31.0: the new prepared-checkout switch explains the disk-space tradeoff.
 * The default is photographed as read from the staged helm, never set by this shot.
 */
test("v0.31.0 prepared checkouts", async ({ page, request }) => {
  const name = `${PAGE}/v0-31-0-prepared-checkouts`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1000 });
  const { director } = await open(page, request);
  await page.locator(".app-settings-toggle").click();
  const dialog = page.locator(".app-settings-dialog");
  await dialog.getByRole("navigation", { name: "settings sections", exact: true })
    .getByRole("button", { name: "checkouts", exact: true }).click();
  const field = dialog.locator("#checkout-spare");
  await expect(field.getByRole("checkbox")).toBeChecked();
  await director.callout(
    field.locator("label"),
    "On by default: prepare the next GitHub checkout in the background, using one extra checkout's disk space per repository and host.",
    { side: "left" },
  );
  await shot(page, name, [field], { maxWidth: 930 });
});

/** v0.31.0: an alias can be entered before the first host check, without adding a host. */
test("v0.31.0 add host alias", async ({ page, request }) => {
  const name = `${PAGE}/v0-31-0-add-host-alias`;
  skipFrozenReleaseShot(name);
  const { scenario, director } = await open(page, request);
  if (!scenario.add_host) throw new Error("the docs scenario needs an add_host");
  await page.locator(".add-host-button").click();
  const dialog = page.locator(".host-add-dialog");
  await expect(dialog).toBeVisible();
  await dialog.locator(".add-host-ssh").fill(scenario.add_host.ssh);
  const alias = dialog.getByRole("textbox", { name: /alias/i });
  await alias.fill("new-box");
  await director.callout(
    alias,
    "An optional name for the host, entered alongside its SSH destination before checking it.",
    { side: "left", dy: 20 },
  );
  await shot(page, name, [dialog], { maxWidth: 930 });
});

/**
 * v0.31.0: unmatched search text is offered as a session name.
 * Choosing or launching is unnecessary to show the new choice.
 */
test("v0.31.0 search name choice", async ({ page, request }) => {
  const name = `${PAGE}/v0-31-0-search-name-choice`;
  skipFrozenReleaseShot(name);
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  const search = form.locator(".launch-composer-search input[role=combobox]");
  await search.fill("investigate login timeout");
  const results = form.locator(".launch-composer-search-results");
  await expect(results).toContainText("investigate login timeout");
  await director.callout(
    results,
    "When your search matches no choice, you can use the words as the session's name.",
    { side: "bottom", dy: 20 },
  );
  await shot(page, name, [search, results], { maxWidth: 930 });
});

/**
 * v0.31.0: the larger amber sequence indicator stays readable over the terminal.
 * Stopping before Enter guarantees that the prefix starts no session.
 */
test("v0.31.0 quick launch indicator", async ({ page, request }) => {
  const name = `${PAGE}/v0-31-0-quick-launch-indicator`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1000 });
  const { director } = await open(page, request);
  await expect(page.locator('.launch-sequence-trigger[data-sequence-action="begin"]')).toBeEnabled();
  await page.keyboard.press("Control+Shift+KeyX");
  const indicator = page.locator(".launch-sequence-indicator");
  await expect(indicator).toContainText("press number keys");
  await director.callout(
    indicator,
    "The quick-launch sequence stays centered over the terminal. Open the session launcher and it moves to the bottom, leaving room to edit.",
    { side: "bottom", dy: 30 },
  );
  await shot(page, name, [indicator], { maxWidth: 930 });
});

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

// v0.28.0. A remote host's settings gained two link permissions (#1948), and
// Settings was regrouped under a heading and close button that stay in view
// (#1949). The link approval card itself is not photographed: a real one
// needs an agent asking to open a link, which no shot may set off, and a
// staged card would be a rewrite docs/docs-shots/SPEC.md does not allow.

/** v0.28.0: a remote host's permissions to open links without asking. */
test("v0.28.0 link permissions", async ({ page, request }) => {
  const name = `${PAGE}/v0-28-0-link-permissions`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1300 });
  const { scenario, director } = await open(page, request);
  const dialog = await openRemoteHostSettings(page, scenario);
  const permissions = dialog.getByRole("region", { name: "Permissions" });
  const links = permissions.getByRole("checkbox", { name: "Open links without asking", exact: true });
  const local = permissions.getByRole("checkbox", { name: "Open local-network links without asking", exact: true });
  await expect(links).not.toBeChecked();
  await expect(local).not.toBeChecked();
  // Both in the left margin, beside the switch each explains: to the right,
  // the crop would pass the docs' text column and cut them off. The two
  // switches sit closer together than the callouts are tall, so one is moved
  // up and the other down until they clear each other.
  await director.callout(
    permissions.locator("label").filter({ has: page.locator(".host-links-without-asking-toggle") }),
    "On: agents on this host open public links in your browser without asking. The web UI still needs one click on open.",
    { side: "left", dy: -30 },
  );
  await director.callout(
    permissions.locator("label").filter({ has: page.locator(".host-local-links-without-asking-toggle") }),
    "Links to your own machine or private network have their own switch. Both start off.",
    { side: "left", dy: 30 },
  );
  await shot(page, name, [permissions], { maxWidth: 930 });
});

/** v0.28.0: Settings in sections, with its heading and close button pinned. */
test("v0.28.0 app settings", async ({ page, request }) => {
  const name = `${PAGE}/v0-28-0-app-settings`;
  skipFrozenReleaseShot(name);
  // Short enough that the settings scroll, which is when the pinned heading
  // and close button matter.
  await page.setViewportSize({ width: 1300, height: 820 });
  const { director } = await open(page, request);
  const dialog = await openSettings(page);
  const body = dialog.locator(".host-settings-body");
  await expect
    .poll(() => body.evaluate((el) => el.scrollHeight > el.clientHeight), {
      message: "premise: the settings overflow the window",
    })
    .toBe(true);
  // All in the left margin, level with what each explains.
  await director.callout(
    dialog.locator(".host-settings-header"),
    "The heading and its close button stay in view while you scroll.",
    { side: "left" },
  );
  await director.callout(
    body.getByRole("region", { name: "managed checkouts" }).locator("h3"),
    "Settings come in sections: hosts, managed checkouts, sounds and, in the Mac app, updates.",
    { side: "left" },
  );
  await director.callout(
    dialog.locator(".host-settings-footer"),
    "So does close at the bottom. Switches save as you flip them.",
    { side: "left" },
  );
  await shot(page, name, [dialog], { maxWidth: 930 });
});

// v0.28.1. Clicking a web link in a terminal no longer reaches the program in
// it as well (#1962); the one thing a reader can see of that fix is the new
// choice in Settings that brings the old behaviour back, so that is the shot.

/** v0.28.1: Settings chooses what clicking a web link in a terminal does. */
test("v0.28.1 terminal links", async ({ page, request }) => {
  const name = `${PAGE}/v0-28-1-terminal-links`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1300 });
  const { director } = await open(page, request);
  const dialog = await openSettings(page);
  const links = dialog.getByRole("region", { name: "terminal links" });
  const choices = links.getByRole("radio");
  await expect(choices).toHaveCount(3);
  // The staged fleet has never saved this choice, so the shot shows the
  // default every reader starts from.
  await expect(choices.first()).toBeChecked();
  // Both in the left margin, level with the choice each explains: to the
  // right, the crop would pass the docs' text column and cut them off.
  await director.callout(
    links.locator(".app-settings-choice").first(),
    "The default: a click opens the link once, and the program in the terminal never sees it.",
    { side: "left" },
  );
  await director.callout(
    links.locator(".app-settings-choice").nth(1),
    "For a program that needs link clicks, pass them on too, or only to the program.",
    { side: "left", dy: 40 },
  );
  await shot(page, name, [links], { maxWidth: 930 });
});

// v0.29.0. A large release, so its shots are spread over the entries that
// added something a reader can see: Settings gained three sections or
// switches (#1986, #1993, #2089), the session menu, the Templates list and
// the ? menu gained controls (#2064, #1994, #2008, #2006), the welcome card
// is new (#2014), the session launcher gained custom permission flags and a
// warning on a riskier agent switch (#2163, #2055), and a terminal file's
// hover was redesigned (#2034).
//
// Each shot is listed under a number only its own entry cites. That is why
// the Guarded default is explained on the custom flags shot rather than shown
// under its own entry: #1992 is also cited by the entry about updating remote
// hosts, so a shot listed under it would show up there too. The same goes for
// starting in a named conversation (#2012). The ssh needs 2FA? dialog is not
// photographed at all: its commands name the capturing machine's ssh
// destinations and the run's state directory, which the staged fleet does not
// rewrite (docs/docs-shots/SPEC.md, "What is real and what is not"). Nothing
// here pins a session, marks a template, binds a key, or saves a setting.

/** v0.29.0: ntfy alerts, set up in Settings. */
test("v0.29.0 ntfy alerts", async ({ page, request }) => {
  const name = `${PAGE}/v0-29-0-ntfy-alerts`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1300 });
  const { director } = await open(page, request);
  const dialog = await openSettings(page);
  const ntfy = dialog.getByRole("region", { name: "ntfy alerts" });
  const enable = ntfy.getByRole("checkbox", { name: "send ntfy alerts", exact: true });
  // The section reads the helm's saved ntfy settings before it unlocks; the
  // staged fleet has none, so the shot shows the form a reader starts from.
  await expect(enable).toBeEnabled();
  await expect(enable).not.toBeChecked();
  await ntfy.scrollIntoViewIfNeeded();
  // Both in the left margin, level with what each explains, and aimed at
  // the whole row rather than its switch or field, so the callout sits
  // beside the row instead of over its label. To the right, the crop would
  // pass the docs' text column and cut them off. The row is found by the
  // control inside it, named from the page: `has` looks for its locator
  // inside each label, so one chained from the section would never match.
  await director.callout(
    ntfy.locator("label").filter({ has: page.getByRole("checkbox", { name: "send ntfy alerts", exact: true }) }),
    "Sends a notification to your ntfy topic when a session starts waiting for you, so it reaches your phone even when Farhelm is not in front of you.",
    { side: "left" },
  );
  await director.callout(
    ntfy.locator("label").filter({ has: page.getByRole("textbox", { name: "topic", exact: true }) }),
    "Subscribe to the same topic in the ntfy app. Anyone who knows an ntfy.sh topic can read it, so pick one that is hard to guess.",
    { side: "left", dy: 40 },
  );
  await shot(page, name, [ntfy], { maxWidth: 930 });
});

/** v0.29.0: Settings can skip the confirmations for session actions. */
test("v0.29.0 session confirmations", async ({ page, request }) => {
  const name = `${PAGE}/v0-29-0-session-confirmations`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1300 });
  const { director } = await open(page, request);
  const dialog = await openSettings(page);
  const sessions = dialog.getByRole("region", { name: "sessions" });
  const skip = sessions.getByRole("checkbox", { name: "skip session confirmations", exact: true });
  await expect(skip).not.toBeChecked();
  // Aimed at the whole row, so the callout sits in the margin beside it
  // rather than over its label.
  await director.callout(
    sessions.locator("label").filter({ has: page.getByRole("checkbox", { name: "skip session confirmations", exact: true }) }),
    "On: delete, replace, restart and closing a shell tab act on the first click. Off by default. YOLO, host and trash questions still ask.",
    // Moved down so its top stays below the dialog's heading.
    { side: "left", dy: 50 },
  );
  // The heading and the row's help rather than the whole section, whose top
  // margin would pull the dialog's heading into the crop.
  await shot(page, name, [sessions.locator("h3"), sessions.locator(".host-settings-help")], {
    pad: 12,
    maxWidth: 930,
  });
});

/** v0.29.0: Settings can answer Claude's trust question for new managed checkouts. */
test("v0.29.0 claude trust", async ({ page, request }) => {
  const name = `${PAGE}/v0-29-0-claude-trust`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1300 });
  const { director } = await open(page, request);
  const dialog = await openSettings(page);
  const trust = dialog.locator("#checkout-claude-trust");
  const answer = trust.getByRole("checkbox", { name: /trust this folder/ });
  // The switch is locked until the helm's saved choice has been read.
  await expect(answer).toBeEnabled();
  await expect(answer).not.toBeChecked();
  await trust.scrollIntoViewIfNeeded();
  // Aimed at the whole row, as on the ntfy alerts shot.
  await director.callout(
    trust.locator("label").filter({ has: page.getByRole("checkbox", { name: /trust this folder/ }) }),
    "Off by default. On: Farhelm marks each new managed checkout trusted in Claude's own settings file, so Claude does not ask about it.",
    // Moved down so its top stays below the save button above the switch.
    { side: "left", dy: 60 },
  );
  await director.callout(
    trust.locator("#claude-trust-caveats"),
    "Read these before turning it on: Claude has no supported way to do this, so it is best effort.",
    { side: "left", dy: 40 },
  );
  // A narrow pad keeps the post-clone command's save button, just above the
  // switch, out of the crop.
  await shot(page, name, [trust], { pad: 8, maxWidth: 930 });
});

/** v0.29.0: Pin to top in a session's menu. */
test("v0.29.0 pin to top", async ({ page, request }) => {
  const name = `${PAGE}/v0-29-0-pin-to-top`;
  skipFrozenReleaseShot(name);
  const { scenario, director } = await open(page, request);
  const row = rowOf(page, scenario, (session) => session.status === "idle" && session.seen === false);
  await openRowMenu(row);
  const panel = page.locator(".session-row-menu-panel");
  await expect(panel).toBeVisible();
  // Photographed, never clicked: a pin is shared by every client of the
  // helm, so it would move this row in every later shot.
  const pin = panel.locator(".session-row-pin");
  // Unpinned, so the item offers to pin rather than to unpin.
  await expect(pin).toHaveAttribute("aria-label", "Pin to top");
  await director.highlight(pin, { pad: 3 });
  await director.callout(
    pin,
    "Keeps this session above the others. Every browser and the desktop app using this helm see the pin, and it carries over when you replace the session.",
    { side: "right" },
  );
  await shot(page, name, [row, panel], { maxWidth: 930 });
});

/** v0.29.0: the ⚡ that marks the template Cmd+P or Ctrl+Shift+P starts. */
test("v0.29.0 quick launch", async ({ page, request }) => {
  const name = `${PAGE}/v0-29-0-quick-launch`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 930, height: 1100 });
  const { director } = await open(page, request);
  const dialog = await openTemplates(page);
  // Opened from the Templates button rather than with the shortcut, which
  // would start a session if a template were marked. No template is: the
  // bolt is photographed unlit, and clicking it would save the mark.
  const bolt = dialog.getByRole("button", { name: "quick launch quick-claude", exact: true });
  await expect(bolt).toHaveAttribute("aria-pressed", "false");
  await director.highlight(bolt, { pad: 3 });
  await director.callout(
    bolt,
    "Mark one template with ⚡. Cmd+P on a Mac, or Ctrl+Shift+P elsewhere, then starts it without opening the session launcher.",
    { side: "bottom", dy: 120 },
  );
  await shot(page, name, [dialog.locator(".templates-sidebar")], { maxWidth: 930 });
});

/** v0.29.0: a template's number key, chosen from the menu beside its ⚡. */
test("v0.29.0 number keys", async ({ page, request }) => {
  const name = `${PAGE}/v0-29-0-number-keys`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 930, height: 1100 });
  const { director } = await open(page, request);
  const dialog = await openTemplates(page);
  // Opening the menu binds nothing; only picking one of its keys would.
  const key = dialog.getByRole("button", { name: "number shortcut quick-claude", exact: true });
  await key.click();
  const menu = page.getByRole("menu", { name: "number key for quick-claude", exact: true });
  await expect(menu).toBeVisible();
  await director.callout(
    menu,
    "Give a template a number key. Cmd+0–9 in the Mac app, or Ctrl+Shift+0–9 elsewhere, then starts it without opening the session launcher.",
    { side: "right" },
  );
  // The template's bolt too, so the crop starts at the row's left edge and
  // shows which control the menu hangs from.
  await shot(page, name, [dialog.getByRole("button", { name: "quick launch quick-claude", exact: true }), key, menu], {
    maxWidth: 930,
  });
});

/**
 * v0.29.0: custom permission flags in the session launcher, with Guarded,
 * the new default for a session started without choosing YOLO.
 */
test("v0.29.0 custom permissions", async ({ page, request }) => {
  const name = `${PAGE}/v0-29-0-custom-permissions`;
  skipFrozenReleaseShot(name);
  // Wide, so the launcher leaves a left margin for the callouts.
  await page.setViewportSize({ width: 1600, height: 1300 });
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  await form.locator(".launch-composer-harness-choice button", { hasText: /^Claude$/ }).click();
  const permissions = form.locator(".launch-composer-permissions-choice");
  await permissions.getByRole("button", { name: "custom", exact: true }).click();
  const flags = form.locator(".custom-permission-choice");
  await expect(flags).toBeVisible();
  // Typed to show what goes there. Nothing launches: only the launcher's
  // own launch button would start a session.
  await flags.locator(".custom-permission-flags").fill("--permission-mode acceptEdits");
  await expect(flags.locator(".custom-permission-words code")).toHaveCount(2);
  await director.callout(
    permissions.getByRole("button", { name: "guarded", exact: true }),
    "Unless you choose YOLO, sessions now start in guarded: the agent's own automatic safety mode approves or blocks actions.",
    // Moved down so its top stays below the effort row above.
    { side: "left", dy: 50 },
  );
  await director.callout(
    flags,
    "custom passes your own permission flags instead of Farhelm's. Farhelm does not check them, so the session counts as YOLO.",
    { side: "left", dy: 30 },
  );
  await shot(page, name, [permissions, flags], { maxWidth: 930 });
});

/**
 * v0.29.0: the launcher warns, with Undo, when switching agents makes the
 * permission mode riskier. Claude in guarded switched to Pi, which has no
 * guarded mode, is the case the release's own browser test uses.
 */
test("v0.29.0 permission switch", async ({ page, request }) => {
  const name = `${PAGE}/v0-29-0-permission-switch`;
  skipFrozenReleaseShot(name);
  // Narrow enough that the launcher, and the warning across its top, fit
  // the docs' text column.
  await page.setViewportSize({ width: 930, height: 1100 });
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  const harness = form.locator(".launch-composer-harness-choice");
  await harness.getByRole("button", { name: "Claude", exact: true }).click();
  const permissions = form.locator(".launch-composer-permissions-choice");
  await permissions.getByRole("button", { name: "guarded", exact: true }).click();
  await harness.getByRole("button", { name: "Pi", exact: true }).click();
  const banner = form.locator(".permission-escalation-banner");
  await expect(banner).toContainText("Permission changed: Guarded → YOLO");
  // The warning goes away on its own after a few seconds unless the pointer
  // rests on it, as a reader's would while reading it. The shot keeps the
  // pointer there, and its animations running, so the warning is still up
  // when the picture is taken (see keepAnimations in ./shot).
  await banner.locator("span").first().hover();
  // Below the warning, over the launcher's destination fields, which this
  // shot is not about; beside it there is no room.
  await director.callout(
    banner.locator(".permission-escalation-undo"),
    "Switching to an agent that cannot keep the permission mode you chose says so. Undo puts the earlier agent and its settings back.",
    { side: "bottom", dx: -200, dy: 200 },
  );
  await shot(page, name, [banner, harness], { maxWidth: 930, keepPointer: true, keepAnimations: true });
});

/** v0.29.0: the ? menu opens the release notes. */
test("v0.29.0 help menu", async ({ page, request }) => {
  const name = `${PAGE}/v0-29-0-help-menu`;
  skipFrozenReleaseShot(name);
  const { director } = await open(page, request);
  await page.locator(".app-help-toggle").click();
  const item = page.locator('[data-bar-menu-item="release notes"]');
  await expect(item).toBeVisible();
  await director.callout(item, "Opens these release notes in your browser.", { side: "right" });
  await shot(page, name, [page.locator(".app-help-toggle"), page.locator(".bar-menu-flyout")], { maxWidth: 930 });
});

/** v0.29.0: the welcome card, here reopened from the ? menu. */
test("v0.29.0 welcome", async ({ page, request }) => {
  const name = `${PAGE}/v0-29-0-welcome`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 930, height: 1000 });
  const { director } = await open(page, request);
  // The staged helm has already seen the welcome (e2e/global-setup.ts), so
  // the card is opened the way a reader brings it back. That changes no
  // setting: only dismissing a first-open card records it as seen.
  await page.locator(".app-help-toggle").click();
  await page.locator('[data-bar-menu-item="welcome"]').click();
  const dialog = page.locator(".welcome-dialog");
  await expect(dialog).toBeVisible();
  await director.callout(
    dialog,
    "Shown the first time you open Farhelm, and once after this update. Dismissing it applies to all your windows; welcome in the ? menu brings it back.",
    { side: "bottom", dy: 10 },
  );
  await shot(page, name, [dialog], { maxWidth: 930 });
});

/**
 * Hover a file path in the open session's terminal and wait for its hover
 * card. The terminal draws on a canvas, so the path has no element to hover,
 * only a position, found the way the Work in a session page's download shot
 * finds it.
 */
async function hoverTerminalPath(page: Page, needle: string) {
  let cell: { x: number; y: number } | null = null;
  await expect
    .poll(async () => {
      cell = await page.evaluate((needle) => {
        const term = (window as any).__farhelmTerm;
        if (!term) return null;
        const box = term.element.querySelector(".xterm-screen").getBoundingClientRect();
        for (let i = 0; i < term.rows; i++) {
          const text = term.buffer.active.getLine(term.buffer.active.viewportY + i)?.translateToString(true) || "";
          const column = text.indexOf(needle);
          if (column >= 0) {
            return {
              x: box.x + (column + needle.length / 2) * box.width / term.cols,
              y: box.y + (i + 0.5) * box.height / term.rows,
            };
          }
        }
        return null;
      }, needle);
      return cell !== null;
    }, { message: `the open session's terminal must show ${needle}` })
    .toBe(true);
  await page.mouse.move(cell!.x, cell!.y);
  const target = page.locator("#terminal .terminal-file-target");
  await expect(target).toBeVisible();
  return target;
}

/**
 * v0.29.0: a terminal file's redesigned hover. The check's answer is the
 * scenario's `terminal_files`, as on the Work in a session page, and the
 * shot never clicks: that would only start a download, which the staging
 * refuses anyway.
 */
test("v0.29.0 file hover", async ({ page, request }) => {
  const name = `${PAGE}/v0-29-0-file-hover`;
  skipFrozenReleaseShot(name);
  const { director } = await open(page, request, { terminalFiles: true });
  const target = await hoverTerminalPath(page, "e2e/attach.spec.ts");
  await expect(target).toContainText("click to download");
  await director.callout(
    target,
    "A file's hover shows an icon, its host and full path, and a readable size. In the desktop app, Cmd-click (Ctrl-click on Linux) saves it, and opens a .md or .txt file in its default app.",
    { side: "right", dx: 40 },
  );
  await shot(page, name, [target], { pad: 60, maxWidth: 930, keepPointer: true });
});

// v0.30.0. The shots follow the entries a reader can see: trust for Codex,
// Muse and Pi moved into Settings (#2256), the session launcher gained an
// initial prompt, a wrapper prefix and typed permission words (#2200, #2272,
// #2297), templates gained a key sequence and a list of their own (#2225,
// #2316), quick launch gained a naming switch (#2234), the local host gained
// its own mark and terminals a host-colored frame (#2266, #2269), host
// confirmations list their steps (#2298), the open session's details moved
// under the terminal (#2253).
//
// Each shot is listed under a number only its own entry cites, as for
// v0.29.0: #2255 and #2271 are also cited by the entry about updating remote
// hosts, so the trust and wrapper shots are listed under #2256 and #2272.
// Several entries are not photographed because their pictures need something
// no shot may do or stage. The naming offer after a quick launch, an agent's
// approval cards, the trust hint above a session's terminal and the outage
// card all need a launched or live session, or a host that has gone away; the
// tunnels and the downloaded file types are Mac app settings, which the web
// UI does not show. A session's hover summary (#2300) is left out because it
// shows when the session started, which staging does not rewrite: next to
// the staged last-active time it would read as starting after it was last
// active. The key sequence is photographed before any key is
// pressed: the staged fleet binds no number keys, and binding one, even in
// transit, would be a rewrite docs/docs-shots/SPEC.md does not allow. Nothing
// here launches a session, binds a key, or saves a setting.

/** v0.30.0: trust for Codex, Muse and Pi, now switches in Settings. */
test("v0.30.0 agent trust", async ({ page, request }) => {
  const name = `${PAGE}/v0-30-0-agent-trust`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1300 });
  const { director } = await open(page, request);
  const dialog = await openSettings(page);
  const codex = dialog.locator("#checkout-codex-trust");
  const muse = dialog.locator("#checkout-muse-trust");
  const pi = dialog.locator("#checkout-pi-trust");
  // Each switch is locked until the helm's saved choice has been read; the
  // staged fleet has saved none, so all three show the default, off.
  for (const block of [codex, muse, pi]) {
    const answer = block.getByRole("checkbox").first();
    await expect(answer).toBeEnabled();
    await expect(answer).not.toBeChecked();
  }
  await codex.scrollIntoViewIfNeeded();
  // The dialog focuses its first switch on opening, whose hover text would
  // otherwise float over the section; focusing the dialog itself hides it.
  await dialog.focus();
  // One callout in the left margin, aimed at the whole row so it sits beside
  // it rather than over its label, and moved down to sit level with Codex's
  // block: two callouts are taller together than the block, and the crop
  // would grow to take in the settings above and below it.
  await director.callout(
    codex.locator("label").first(),
    "Trust for Codex, Muse and Pi is now a switch here instead of a launcher choice: off by default, managed checkouts only. Codex and Muse show a hint beside their trust question that opens it.",
    { side: "left", dy: 70 },
  );
  // Codex's block alone: Muse's and Pi's below it repeat its shape.
  await shot(page, name, [codex], { pad: 8, maxWidth: 930 });
});

/** Open the session launcher with Claude chosen, the way most of these shots start. */
async function openLauncherWithClaude(page: Page) {
  const form = await openLauncher(page);
  const claude = form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Claude", exact: true });
  await claude.click();
  await expect(claude).toHaveAttribute("aria-pressed", "true");
  // The click leaves the pointer resting on the button, whose hover text
  // would show in the picture.
  await page.mouse.move(0, 0);
  return form;
}

/** v0.30.0: an initial prompt, typed in the session launcher. */
test("v0.30.0 initial prompt", async ({ page, request }) => {
  const name = `${PAGE}/v0-30-0-initial-prompt`;
  skipFrozenReleaseShot(name);
  // Wide, so the launcher leaves a left margin for the callout.
  await page.setViewportSize({ width: 1600, height: 1300 });
  const { director } = await open(page, request);
  const form = await openLauncherWithClaude(page);
  const choice = form.locator(".initial-prompt-choice");
  const prompt = choice.getByLabel("initial prompt", { exact: true });
  // Typed to show what goes there. Nothing launches: only the launcher's own
  // launch button would start a session, and Enter in this field only
  // starts a new line.
  await prompt.fill("Find out why the attach test fails on CI and fix it.");
  await form.focus();
  await director.callout(
    prompt,
    "The agent gets this as its first message when the session starts, once. Templates and the command line can carry one too. Goose, Pi and custom commands do not take one.",
    { side: "left" },
  );
  // A narrow pad keeps the launcher fields above and below out of the crop.
  await shot(page, name, [choice], { pad: 6, maxWidth: 930 });
});

/** v0.30.0: a wrapper prefix, typed in the session launcher. */
test("v0.30.0 wrapper prefix", async ({ page, request }) => {
  const name = `${PAGE}/v0-30-0-wrapper-prefix`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1600, height: 1300 });
  const { director } = await open(page, request);
  const form = await openLauncherWithClaude(page);
  const choice = form.locator(".launch-composer-wrapper-choice");
  const wrapper = choice.locator(".launch-composer-wrapper");
  // Typed, not launched, as on the initial prompt shot; Enter here would
  // launch, so the shot never presses it. The wrapper is the release
  // notes' own example.
  await wrapper.fill("nice -n 10");
  await expect(wrapper).toHaveAttribute("aria-invalid", "false");
  await form.focus();
  await director.callout(
    wrapper,
    "A command that runs the agent for you, here at a lower priority. Farhelm still shows the agent's status, follows its conversation, and can restart it.",
    // Moved down, beside the field and its help, so the callout does not
    // reach up to the initial prompt above and pull it into the crop.
    { side: "left", dy: 50 },
  );
  // A narrow pad keeps the launcher fields above and below out of the crop.
  await shot(page, name, [choice], { pad: 3, maxWidth: 930 });
});

/** v0.30.0: typed permission words in the session launcher's search. */
test("v0.30.0 launcher permission words", async ({ page, request }) => {
  const name = `${PAGE}/v0-30-0-permission-words`;
  skipFrozenReleaseShot(name);
  // Narrow enough that the launcher, and so its search box, fits the docs'
  // text column.
  await page.setViewportSize({ width: 930, height: 1100 });
  const { director } = await open(page, request);
  const form = await openLauncherWithClaude(page);
  const search = form.locator(".launch-composer-search input[role=combobox]");
  // Typed and left: Enter would apply the highlighted choice, and a second
  // Enter in the then empty box would launch, so the shot presses neither.
  await search.fill("perms:");
  const results = form.locator(".launch-composer-search-results");
  const group = results.getByRole("group", { name: "Permissions" });
  await expect(group.getByRole("option")).toHaveCount(3);
  await director.callout(
    results,
    "perms: lists the approval modes the chosen agent offers. guarded on its own picks Guarded, and effort:default goes back to the agent's own effort.",
    // Under the results, over the launcher's lower controls, which this shot
    // is not about; beside the search box there is no room.
    { side: "bottom", dy: 10 },
  );
  await shot(page, name, [search, results], { maxWidth: 930 });
});

/** v0.30.0: Ctrl+Shift+X starts a sequence of template keys. */
test("v0.30.0 template sequence", async ({ page, request }) => {
  const name = `${PAGE}/v0-30-0-template-sequence`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1000 });
  const { director } = await open(page, request);
  // The sequence's keys are listened for only once the page has read its
  // preferences, which this trigger waits on.
  await expect(page.locator('.launch-sequence-trigger[data-sequence-action="begin"]')).toBeEnabled();
  // The staged fleet is the web UI on Linux, so the prefix is Ctrl+Shift+X;
  // in the Mac app it is Cmd+X, as the callout says. Nothing after it is
  // pressed, so nothing launches.
  await page.keyboard.press("Control+Shift+KeyX");
  const indicator = page.locator(".launch-sequence-indicator");
  await expect(indicator).toContainText("press number keys");
  await director.callout(
    indicator,
    "Cmd+X in the Mac app, Ctrl+Shift+X elsewhere, then the number keys of the templates to combine and Enter. With the session launcher open, the same keys fill it in instead.",
    { side: "top", dy: -20 },
  );
  await shot(page, name, [indicator], { maxWidth: 930 });
});

/** v0.30.0: Ctrl+Shift+J lists the templates to start one from. */
test("v0.30.0 template list", async ({ page, request }) => {
  const name = `${PAGE}/v0-30-0-template-list`;
  skipFrozenReleaseShot(name);
  // Narrow enough that the whole dialog fits the docs' text column, as on
  // the quick switcher shot it borrows its look from.
  await page.setViewportSize({ width: 930, height: 1000 });
  const { director } = await open(page, request);
  await expect(page.locator(".quick-switcher-template-trigger")).toHaveCount(1);
  await page.keyboard.press("Control+Shift+j");
  const dialog = page.getByRole("dialog", { name: "template launcher" });
  await expect(dialog).toBeVisible();
  const search = dialog.getByRole("combobox", { name: "search templates" });
  await expect(search).toBeFocused();
  // Clicking into the box makes its focus a pointer focus, which hides its
  // hover text the way a click does in the app. Enter would start the
  // highlighted template, so the shot never presses it.
  await search.click();
  await expect(page.locator(".farhelm-tooltip").filter({ visible: true })).toHaveCount(0);
  await expect(dialog.locator(".quick-switcher-template")).toHaveCount(3);
  await director.callout(
    dialog,
    "Cmd+J on a Mac, Ctrl+Shift+J elsewhere. Type part of a template's name and press Enter to start it; one that leaves choices open opens the session launcher to finish.",
    { side: "bottom", dy: 10 },
  );
  await shot(page, name, [dialog], { maxWidth: 930 });
});

/** v0.30.0: Settings turns off the offer to name a quick-launched session. */
test("v0.30.0 quick launch naming", async ({ page, request }) => {
  const name = `${PAGE}/v0-30-0-quick-launch-naming`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1300 });
  const { director } = await open(page, request);
  const dialog = await openSettings(page);
  const sessions = dialog.getByRole("region", { name: "sessions" });
  const offer = sessions.getByRole("checkbox", { name: "name sessions on quick launch", exact: true });
  await expect(offer).toBeChecked();
  await dialog.focus();
  await director.callout(
    sessions.locator("label").filter({ has: page.getByRole("checkbox", { name: "name sessions on quick launch", exact: true }) }),
    "On by default: after a quick launch, Farhelm offers to name the new session while its agent starts. Enter or Escape keeps the default name.",
    { side: "left" },
  );
  await shot(page, name, [sessions.locator("label").nth(1), sessions.locator("#quick-launch-naming-help")], {
    pad: 12,
    maxWidth: 930,
  });
});

/** v0.30.0: Settings colors each terminal's frame by its host. */
test("v0.30.0 terminal frames", async ({ page, request }) => {
  const name = `${PAGE}/v0-30-0-terminal-frames`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1300 });
  const { director } = await open(page, request);
  const dialog = await openSettings(page);
  const sessions = dialog.getByRole("region", { name: "sessions" });
  const frames = sessions.getByRole("checkbox", { name: "color terminal frames by host", exact: true });
  await expect(frames).not.toBeChecked();
  await dialog.focus();
  await director.callout(
    sessions.locator("label").filter({ has: page.getByRole("checkbox", { name: "color terminal frames by host", exact: true }) }),
    "Off by default. On: the thin frame around each session's terminal takes its host's color, so you can tell at a glance where it runs.",
    { side: "left" },
  );
  await shot(page, name, [sessions.locator("label").nth(2), sessions.locator("#terminal-frame-color-help")], {
    pad: 12,
    maxWidth: 930,
  });
});

/** v0.30.0: the open session's details in a slim bar under its terminal. */
test("v0.30.0 session details", async ({ page, request }) => {
  const name = `${PAGE}/v0-30-0-session-details`;
  skipFrozenReleaseShot(name);
  // Sized so the open session, beside the session list, is as wide as the
  // docs' text column, and only as tall as its transcript needs.
  await page.setViewportSize({ width: 1270, height: 650 });
  const { director } = await open(page, request);
  const topbar = page.locator("header.session-topbar");
  const details = page.locator(".session-details");
  await expect(topbar.locator(".tab-strip")).toBeVisible();
  await expect(details).toBeVisible();
  // Both inside the terminal, right of the transcript's lines (dx), where it
  // is empty, so neither covers the parts it is about or the agent's text.
  await director.callout(
    topbar,
    "The terminal tabs and the session's buttons share one row at the top.",
    { side: "bottom", dx: 250, dy: 20 },
  );
  // Aimed at the command, the bar's last field, so the arrow lands on the
  // bar's contents rather than its empty right end.
  await director.callout(
    details.locator(".header-copy").last(),
    "The session's name, host, directory and command sit in a slim bar under the terminal. Click the directory or the command to copy it.",
    { side: "top", dx: 60, dy: -20 },
  );
  await shot(page, name, [page.locator(".app-main .layout")], { pad: 0 });
});

/** v0.30.0: the local host's own icon and color, in its settings. */
test("v0.30.0 local host appearance", async ({ page, request }) => {
  const name = `${PAGE}/v0-30-0-local-host-appearance`;
  skipFrozenReleaseShot(name);
  await page.setViewportSize({ width: 1300, height: 1300 });
  const { director } = await open(page, request);
  const row = page.locator('.host-row[data-host-kind="local"]');
  await openHostMenu(row);
  await row.locator(".host-settings").click();
  const dialog = page.locator(".host-settings-dialog");
  const appearance = dialog.locator(".host-settings-appearance");
  await expect(appearance).toBeVisible();
  // Photographed, never clicked: a click on an icon or a color saves it.
  // The local host starts as the red laptop.
  await expect(appearance.getByRole("button", { name: "laptop", exact: true })).toHaveAttribute("aria-pressed", "true");
  // In the left margin, level with what each explains, as on the v0.26.1
  // remote host shot: to the right, the crop would pass the docs' text column.
  await director.callout(
    dialog.locator(".host-settings-mark"),
    "The machine the helm runs on, your Mac, now has a mark of its own, like a remote host.",
    { side: "left" },
  );
  await director.callout(
    appearance.locator(".host-appearance-icons"),
    "Pick its icon and color here. A click saves at once, and its sessions show the new mark.",
    { side: "left", dy: 30 },
  );
  await shot(page, name, [dialog.locator(".host-settings-header"), appearance], { maxWidth: 930 });
});

/**
 * v0.30.0: a host's setup confirmation as numbered steps. The add-host check
 * answers for the scenario's invented add_host, as on the v0.27.0 setup
 * shot, and nothing confirms the setup.
 */
test("v0.30.0 host steps", async ({ page, request }) => {
  const name = `${PAGE}/v0-30-0-host-steps`;
  skipFrozenReleaseShot(name);
  // Tall, so all of the plan's steps fit without scrolling.
  await page.setViewportSize({ width: 1300, height: 1500 });
  const { scenario, director } = await open(page, request);
  if (!scenario.add_host) throw new Error("the docs scenario needs an add_host");
  await page.locator(".add-host-button").click();
  const dialog = page.locator(".host-add-dialog");
  await expect(dialog).toBeVisible();
  await dialog.locator(".add-host-ssh").fill(scenario.add_host.ssh);
  await dialog.locator(".add-host-submit").click();
  const plan = dialog.locator(".provisioning-plan");
  await expect(plan).toContainText(scenario.add_host.ssh);
  const steps = plan.locator(".provisioning-plan-step");
  await expect(steps.first()).toBeVisible();
  // In the left margin, level with the first step. The steps' file names
  // carry a per-run suffix, so the callout does not quote them.
  await director.callout(
    steps.first(),
    "Every step setup will run, numbered, with the files each one writes. Update and uninstall list theirs the same way.",
    { side: "left" },
  );
  // A narrow pad keeps the dialog's heading, just above the plan, out of the
  // crop.
  await shot(page, name, [plan], { pad: 8, maxWidth: 930 });
});
