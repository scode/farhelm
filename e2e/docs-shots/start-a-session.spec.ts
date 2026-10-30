/**
 * Screenshots for the docs page "Start a session"
 * (website/src/content/docs/docs/using/start-a-session.mdx).
 *
 * Each test is one named shot. The annotation wording is part of the page's
 * content and carries as much of it as the picture can (website/EDITORIAL_RULES.md,
 * "Rules from feedback"), so it changes when the page does; the steps that reach each state
 * change whenever the UI moves under them, and are rewritten to show the
 * same thing (docs/docs-shots/SPEC.md, "Refreshing").
 *
 * Nothing here launches a session or changes a setting: every shot stops at
 * the state it photographs, so shots cannot affect each other or the fleet.
 */
import { expect, test } from "@playwright/test";
import { hostRowByName, openHostMenu } from "../tests/helpers/fleet";
import { open, openLauncher } from "./open";
import { shot } from "./shot";

const PAGE = "start-a-session";

test("new button", async ({ page, request }) => {
  const { director } = await open(page, request);
  const button = page.locator(".new-session-button");
  // Above the button's row, over the terminal, so it covers neither the
  // neighbouring controls nor the list.
  await director.callout(button, "Opens the session launcher, where every session starts", { side: "right", dy: -70 });
  await shot(page, `${PAGE}/new-button`, [button, page.locator(".session-list .session-row").first()]);
});

test("launch button", async ({ page, request }) => {
  // Put annotations above the pinned footer at docs width. Left-margin
  // callouts would clamp together at the viewport's bottom edge.
  await page.setViewportSize({ width: 930, height: 1100 });
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  await form.locator(".launch-composer-harness-choice button", { hasText: /^Codex$/ }).click();
  const submit = form.locator(".create-session-submit");
  await expect(submit).toBeEnabled();
  await director.highlight(submit, { pad: 3 });
  // Explain the visible target in the footer, rather than
  // claiming the button itself repeats the complete destination.
  await director.callout(
    form.locator(".launch-composer-launch-context"),
    "What Launch will start: the agent, host and directory. Launch stays grey until the setup is complete; hover it to see what is missing.",
    { side: "top", dx: -50 },
  );
  // Beside the first callout, clear of the target and action hit areas.
  await director.callout(
    form.locator(".launch-composer-summary"),
    "The choices the launch will use. Check it before launching: YOLO shows in red.",
    { side: "top", dx: 350 },
  );
  // Keep the complete target, choices and compact action together. This
  // viewport makes the whole annotated footer fit the docs' text column.
  await shot(page, `${PAGE}/launch-button`, [submit, form.locator(".launch-composer-launch-context"), form.locator(".launch-composer-summary")], { pad: 28, maxWidth: 930 });
});

test("destination", async ({ page, request }) => {
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  const destination = form.locator(".launch-composer-destination");
  await destination.getByRole("combobox", { name: "Folder", exact: true }).focus();
  const folders = destination.getByRole("listbox", { name: "Folders", exact: true });
  await expect(folders).toBeVisible();
  // The dropdown floats past the end of the launcher's scrolling body, which
  // clips it; scrolling its last entry into view is what a person does to
  // reach it, and keeps local home, which the last callout names, in the shot.
  await folders.getByRole("option", { name: "Reset destination to local home" }).scrollIntoViewIfNeeded();
  // Four callouts stacked down the left margin, each nudged (dy) so the
  // boxes clear one another while the arrows stay level with their controls.
  await director.callout(
    destination.locator(".destination-type-control"),
    "Folder starts in a directory that already exists; Managed checkout clones a GitHub repository into a new one.",
    { side: "left", dy: -70 },
  );
  await director.callout(
    destination.locator(".create-session-host"),
    "The host the session runs on. Starts on the host of the session you have open.",
    { side: "left", dy: 10 },
  );
  await director.callout(
    destination.getByLabel("Folder", { exact: true }),
    "The directory the agent starts in. ~ is your home on that host, not on the machine in front of you.",
    { side: "left", dy: 75 },
  );
  await director.callout(
    folders,
    "Opens on a click or ↓: up to eight recent directories on this host, newest first; typing filters them. Home is ~ on that host, Local home ~ on the machine in front of you.",
    { side: "left", dy: 100 },
  );
  // The folder list hangs below the destination part, so it is cropped in on its own.
  await shot(page, `${PAGE}/destination`, [destination, folders]);
});

/**
 * The launcher in managed checkout mode on a helm with no checkout directory
 * configured yet, which is the staged fleet's state: the launcher asks for
 * one in place of the directory preview. Stops before save, so the fleet's
 * configuration stays unset for every other shot.
 */
test("checkout setup", async ({ page, request }) => {
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  const destination = form.locator(".launch-composer-destination");
  await destination.locator(".destination-type-control button", { hasText: "Managed checkout" }).click();
  const field = destination.locator(".checkout-root-field");
  await expect(field).toBeVisible();
  await field.locator("input").fill("~/work");
  await director.callout(
    field.locator("input"),
    "Asked once: the directory every host's managed checkouts go into. ~ is each host's own home, and Farhelm creates the directory on first use.",
    { side: "left" },
  );
  await director.callout(
    field.getByRole("button", { name: "Save" }),
    "Saves it for every host. Change it later in Settings.",
    { side: "left", dy: 40 },
  );
  await shot(page, `${PAGE}/checkout-setup`, [destination], { maxWidth: 930 });
});

test("harness and model", async ({ page, request }) => {
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  const harness = form.locator(".launch-composer-harness-choice");
  await harness.locator("button", { hasText: /^Codex$/ }).click();
  const model = form.locator(".launch-composer-model-choice");
  await expect(model).toBeVisible();
  // Both targets span the launcher's column, so callouts on their right line
  // up in the empty margin without covering anything; dx pushes them clear
  // of the column so the arrows show, and dy keeps the two boxes, each
  // taller than its target, from touching.
  await director.callout(
    harness,
    "Pick the agent first. Nothing is chosen when the launcher opens, and the choices below change to what that agent offers.",
    { side: "right", dx: 60, dy: -60 },
  );
  await director.callout(
    model,
    "Which model. Harness default leaves it to the agent's own setting; type a name to use one the list doesn't show.",
    { side: "right", dx: 60, dy: 50 },
  );
  await shot(page, `${PAGE}/harness-and-model`, [harness, model]);
});

test("effort and permissions", async ({ page, request }) => {
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  await form.locator(".launch-composer-harness-choice button", { hasText: /^Codex$/ }).click();
  const effort = form.locator(".launch-composer-effort-choice");
  const permissions = form.locator(".launch-composer-permissions-choice");
  await expect(permissions).toBeVisible();
  // The overlay does not keep callouts apart, and these controls are close
  // together and of different widths. Each callout is pushed right (dx) to a
  // shared column beside the widest control and nudged up or down (dy) so
  // the boxes stack without overlapping. Trust is not here: it moved to
  // Settings, which the page links to.
  await director.callout(effort, "How much effort the model puts in. Default leaves it to the agent.", {
    side: "right",
    dx: 160,
    dy: -40,
  });
  await director.callout(
    permissions,
    "Guarded lets the agent approve its own actions through a safety check, which can still refuse or ask you. YOLO skips those checks: the agent runs commands and changes files without asking. Custom passes your own flags and counts as YOLO.",
    { side: "right", dx: 290, dy: 70 },
  );
  await shot(page, `${PAGE}/effort-and-permissions`, [effort, permissions]);
});

test("search", async ({ page, request }) => {
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  const search = form.locator(".launch-composer-search input[role=combobox]");
  // A plain word, the way search is mostly used: it matches across every
  // kind of thing at once, with no prefix needed.
  await search.fill("claude");
  const results = form.locator(".launch-composer-search-results");
  await expect(results.getByRole("option").first()).toBeVisible();
  await director.callout(
    search,
    "Type to find anything the launcher can set: hosts, agents, models, directories, recent setups.",
    { side: "left" },
  );
  await director.callout(
    results,
    "Choosing a result applies it and empties the box. Enter on the empty box launches.",
    { side: "left", dy: 60 },
  );
  await shot(page, `${PAGE}/search`, [search, results], { maxWidth: 900 });
});

test("search with a prefix", async ({ page, request }) => {
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  const search = form.locator(".launch-composer-search input[role=combobox]");
  // Plain "claude" finds the agent; this prefix makes the same word a
  // session name. Unmatched text already offers a name without a prefix.
  await search.fill("name: claude");
  const results = form.locator(".launch-composer-search-results");
  await expect(results.getByRole("option").first()).toBeVisible();
  await director.callout(
    search,
    "name: makes the words a session name even when they match another choice, such as the Claude agent.",
    { side: "left" },
  );
  await shot(page, `${PAGE}/search-name`, [search, results], { maxWidth: 900 });
});

test("recent setups", async ({ page, request }) => {
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  const recents = form.locator(".launch-composer-recents");
  await expect(recents.locator(".launch-composer-recent-slots > button").first()).toBeVisible();
  await director.highlight(recents, { pad: 4 });
  // Below the list rather than beside it, which keeps the crop no wider than
  // the list itself.
  await director.callout(
    recents,
    "Setups you launched in this directory on this host, most used first. Click one to fill in the launcher; Enter on it launches straight away.",
    { side: "bottom", dx: 200 },
  );
  // The rows' right half is empty, so trimming it keeps the image readable
  // in the docs' text column.
  await shot(page, `${PAGE}/recent-setups`, [recents], { maxWidth: 900 });
});

test("yolo confirmation", async ({ page, request }) => {
  // Tall enough that the launcher ends above the window's bottom edge,
  // leaving room under its question for the callouts about its answers.
  await page.setViewportSize({ width: 1600, height: 1700 });
  const { director } = await open(page, request);
  // Pressing launch really sends the create; the helm answers with the
  // confirmation only because the host still asks before YOLO launches.
  // Should that ever change, this shot would start a real session, so it
  // fails instead of quietly mutating the fleet.
  const created: string[] = [];
  page.on("response", (response) => {
    const url = new URL(response.url());
    if (response.request().method() === "POST" && url.pathname === "/api/sessions" && response.ok()) {
      created.push(url.toString());
    }
  });
  const form = await openLauncher(page);
  await form.locator(".launch-composer-harness-choice button", { hasText: /^Codex$/ }).click();
  await form.locator(".launch-composer-permissions-choice .launch-composer-segment-danger").click();
  await form.locator(".create-session-submit").click();
  const confirmation = form.locator(".yolo-confirmation");
  await expect(confirmation).toBeVisible();
  // Side by side below the answers, clear of the question's text.
  await director.callout(
    confirmation.locator(".yolo-confirm"),
    "Starts this launch. The host still asks next time.",
    { side: "bottom", dx: -40, dy: 40 },
  );
  await director.callout(
    confirmation.locator(".yolo-confirm-stop-asking"),
    "Starts it, and this host stops asking from now on.",
    { side: "bottom", dx: -110, dy: 40 },
  );
  await shot(page, `${PAGE}/yolo-confirmation`, [confirmation, form.locator(".create-session-submit")], { maxWidth: 930 });
  expect(created, "the YOLO shot must not start a session").toEqual([]);
});

test("host menu", async ({ page, request }) => {
  const { scenario, director } = await open(page, request);
  const remote = scenario.hosts.find((host) => host.kind === "remote");
  if (!remote) throw new Error("the docs scenario needs a remote host for this shot");
  // The host list is always mounted; the hosts panel's details view (what
  // the suite's openHostsPanel opens) would add each host's identity and
  // destination lines, which this shot does not need.
  const row = hostRowByName(page, remote.alias);
  await openHostMenu(row);
  const settings = row.locator(".host-settings");
  await expect(settings).toBeVisible();
  await director.callout(
    settings,
    "Each host's own settings, including whether it asks before YOLO launches",
    { side: "right" },
  );
  await shot(page, `${PAGE}/host-menu`, [row, page.locator(".host-row-menu-panel")]);
});

test("host yolo setting", async ({ page, request }) => {
  // Tall enough that the Permissions section is not clipped by the dialog's
  // scrolling body.
  await page.setViewportSize({ width: 1600, height: 1700 });
  const { scenario, director } = await open(page, request);
  const remote = scenario.hosts.find((host) => host.kind === "remote");
  if (!remote) throw new Error("the docs scenario needs a remote host for this shot");
  const row = hostRowByName(page, remote.alias);
  await openHostMenu(row);
  await row.locator(".host-settings").click();
  const dialog = page.locator(".host-settings-dialog");
  await expect(dialog).toBeVisible();
  const toggle = dialog.locator(".host-yolo-without-asking-toggle");
  await director.highlight(toggle, { pad: 4 });
  // Right of the switch, which sits at the dialog's right edge, so the
  // setting's own label and explanation stay readable. The crop keeps only
  // the dialog's Permissions section: the rest of the dialog is Manage
  // hosts' to explain.
  await director.callout(
    toggle,
    "On: YOLO launches on this host start without asking. Off: Farhelm asks every time.",
    { side: "right" },
  );
  await shot(page, `${PAGE}/host-yolo-setting`, [dialog.locator("section[aria-label=Permissions]")]);
});
