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
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  await form.locator(".launch-composer-harness-choice button", { hasText: /^Codex$/ }).click();
  const submit = form.locator(".create-session-submit");
  await expect(submit).toBeEnabled();
  await director.highlight(submit, { pad: 3 });
  // In the margin left of the launcher, pointing in at the button.
  await director.callout(
    submit,
    "Spells out what it will start: the agent, the host, and the directory. Disabled until an agent is chosen and the host is connected.",
    { side: "left" },
  );
  // Also in the left margin, under the first, so the crop stays narrow.
  await director.callout(
    form.locator(".launch-composer-summary"),
    "The choices the launch will use. Check it before launching: yolo shows in red.",
    { side: "left", dy: 90 },
  );
  // Just the button, its neighbour, and the summary: the launcher is wider
  // than the docs' text column, and a crop that wide would be shrunk until
  // it is hard to read.
  await shot(page, `${PAGE}/launch-button`, [submit, form.locator(".launch-composer-cancel")], { pad: 28 });
});

test("destination", async ({ page, request }) => {
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  const destination = form.locator(".launch-composer-destination");
  await director.callout(
    destination.locator("select.create-session-host"),
    "The host the session runs on. Starts on the host of the session you have open.",
    { side: "left", dy: -30 },
  );
  await director.callout(
    destination.getByLabel("folder", { exact: true }),
    "The directory the agent starts in. ~ is your home on that host, not on the machine in front of you.",
    { side: "left", dy: 20 },
  );
  await director.callout(
    destination.getByLabel("recent folders"),
    "Directories you launch in most on this host. Local home is your home on the machine in front of you.",
    { side: "left", dy: 80 },
  );
  await shot(page, `${PAGE}/destination`, [destination]);
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
  // of the column so the arrows show.
  await director.callout(
    harness,
    "Pick the agent first. Nothing is chosen when the launcher opens, and the choices below change to what that agent offers.",
    { side: "right", dx: 60 },
  );
  await director.callout(
    model,
    "Which model. harness default leaves it to the agent's own setting; type a name to use one the list doesn't show.",
    { side: "right", dx: 60 },
  );
  await shot(page, `${PAGE}/harness-and-model`, [harness, model]);
});

test("effort, permissions, and trust", async ({ page, request }) => {
  const { director } = await open(page, request);
  const form = await openLauncher(page);
  await form.locator(".launch-composer-harness-choice button", { hasText: /^Codex$/ }).click();
  const effort = form.locator(".launch-composer-effort-choice");
  const permissions = form.locator(".launch-composer-permissions-choice");
  const trust = form.locator(".launch-composer-trust-choice");
  await expect(trust).toBeVisible();
  // The overlay does not keep callouts apart, and these controls are close
  // together and of different widths. Each callout is pushed right (dx) to a
  // shared column beside the widest control and nudged up or down (dy) so
  // the three boxes stack without overlapping.
  await director.callout(effort, "How much effort the model puts in. default leaves it to the agent.", {
    side: "right",
    dx: 160,
    dy: -20,
  });
  await director.callout(
    permissions,
    "default keeps the agent's approval prompts. yolo turns them off: the agent runs commands and changes files without asking you.",
    { side: "right", dx: 290, dy: 25 },
  );
  await director.callout(trust, "Whether the agent treats this directory as trusted. Only some agents have it.", {
    side: "right",
    dy: 45,
  });
  await shot(page, `${PAGE}/effort-permissions-trust`, [effort, permissions, trust]);
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
  // A case where the prefix is required: without name:, a session name is
  // never offered, since any word could be one.
  await search.fill("name: fix login bug");
  const results = form.locator(".launch-composer-search-results");
  await expect(results.getByRole("option").first()).toBeVisible();
  await director.callout(
    search,
    "Some things need a prefix. name: sets the session's name, which a plain word never does.",
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
  // Left of and below the buttons, where the launcher's own controls are
  // what they cover, rather than the confirmation's text. Cancel needs no
  // callout: the confirmation itself says nothing has started.
  // Both below their buttons, over the launcher's own controls, so the crop
  // stays within the launcher and out of the session list beside it.
  await director.callout(
    confirmation.locator(".yolo-confirm"),
    "Starts this launch. The host still asks next time.",
    { side: "bottom", dx: -20 },
  );
  await director.callout(
    confirmation.locator(".yolo-confirm-stop-asking"),
    "Starts it, and this host stops asking from now on.",
    { side: "bottom", dx: 140 },
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
  // Left of the dialog, so the setting's own explanation stays readable.
  await director.callout(
    toggle,
    "Ticked: YOLO launches on this host start without asking. Unticked: Farhelm asks every time.",
    { side: "left" },
  );
  await shot(page, `${PAGE}/host-yolo-setting`, [dialog]);
});
