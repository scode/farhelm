/**
 * The launch search is a sequential keyboard surface: each accepted word
 * changes one structured field, clears the box, and returns focus to the
 * same input, and Enter on the emptied box launches through the ordinary
 * launch button. These tests exist because the keyboard path used to stop
 * at the harness (no effort words, models unscoped, no launch from the
 * box), and nothing else in the suite drives the whole selection from the
 * one field.
 *
 * Codex, never Claude: the stack fakes exactly one structured harness
 * binary (`start-stack.sh` installs an owned fake `codex`), so a Codex
 * launch runs that fake while any other harness would exec whatever the
 * login shell finds — on a maintainer's machine, the real vendor CLI. The
 * model comes from the helm's own catalog rather than a hardcoded id so
 * the test keeps describing the actual offering.
 */
import { expect, test } from "./helpers/evidence";
import type { APIRequestContext } from "@playwright/test";
import { cleanupSession, FAKE_AGENT, SESSION_LISTING } from "./helpers/fleet";

type CatalogModel = {
  id: string;
  harness: string;
  efforts: string[];
};

type StoredLaunch = {
  harness: string;
  model: string | null;
  effort: string | null;
  permissions: string | null;
};

type ListedSession = {
  id: string;
  launch?: StoredLaunch | null;
};

/** Read the release-owned catalog and establish the model/effort premise:
 * a Codex model that offers at least one effort word. */
async function codexCatalogModel(request: APIRequestContext) {
  const response = await request.get("/api/launch-catalog");
  expect(response.ok(), "the helm must expose its launch catalog").toBe(true);
  const catalog = await response.json() as CatalogModel[];
  const model = catalog.find((candidate) => candidate.harness === "codex" && candidate.efforts.length > 0);
  expect(model, "the stack catalog must expose a Codex model with an effort").toBeTruthy();
  return model!;
}

/** Return the current session records for API-level assertions. */
async function listedSessions(request: APIRequestContext) {
  const response = await request.get("/api/sessions");
  expect(response.ok(), "the session listing must be readable").toBe(true);
  return (await response.json() as { sessions: ListedSession[] }).sessions;
}

/**
 * The complete word path applies harness, model, effort, and both permission
 * shorthand choices in sequence, then launches through the ordinary button
 * path with the stored selection intact. Each Enter is preceded by an assertion that the intended option
 * is the preselected one: the Enter handler takes whatever row the last
 * render highlighted, so the premise has to hold before the key. The full
 * model id is typed so the option Enter will accept is the one named; with
 * the shipped catalog no Codex id contains the harness word, so this test
 * does NOT exercise the exact-word preselection rule — the unit test
 * `default_search_index_prefers_exact_specific_words` in launch_composer.rs
 * is that rule's only coverage, and must stay.
 */
test("composer word search drives a structured launch", async ({ page, request }) => {
  const model = await codexCatalogModel(request);
  const before = new Set((await listedSessions(request)).map((session) => session.id));
  await page.goto("/");
  await page.locator(".new-session-button").click();

  const form = page.locator('.create-session-form[role="dialog"]');
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await expect(form).toBeVisible();
  await expect(search).toBeFocused();

  await search.fill("codex");
  await expect(form.getByRole("option", { name: "Harness: Codex", exact: true })).toHaveAttribute("aria-selected", "true");
  await search.press("Enter");
  await expect(form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }))
    .toHaveAttribute("aria-pressed", "true");
  await expect(search).toHaveValue("");
  await expect(search).toBeFocused();

  await search.fill(model.id);
  await expect(form.getByRole("option", { name: `Model: ${model.id} (Codex)`, exact: true })).toHaveAttribute("aria-selected", "true");
  await search.press("Enter");
  await expect(form.getByRole("combobox", { name: "model", exact: true })).toHaveValue(model.id);
  await expect(search).toHaveValue("");
  await expect(search).toBeFocused();

  const effort = model.efforts[0];
  await search.fill(effort);
  await expect(form.getByRole("option", { name: `Effort: ${effort}`, exact: true })).toHaveAttribute("aria-selected", "true");
  await search.press("Enter");
  await expect(form.locator(".launch-composer-effort-choice").getByRole("button", { name: effort, exact: true }))
    .toHaveAttribute("aria-pressed", "true");
  await expect(search).toHaveValue("");
  await expect(search).toBeFocused();

  await search.fill("perms:yolo");
  await expect(form.getByRole("group", { name: "Permissions" })).toBeVisible();
  await expect(form.getByRole("option", { name: "Permissions: yolo", exact: true, selected: true }))
    .toBeVisible();
  await search.press("Enter");
  await expect(form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }))
    .toHaveAttribute("aria-pressed", "true");
  await expect(search).toHaveValue("");
  await expect(search).toBeFocused();

  await search.fill("perms:default");
  await expect(form.getByRole("option", { name: "Permissions: default", exact: true, selected: true }))
    .toBeVisible();
  await search.press("Enter");
  await expect(form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "default", exact: true }))
    .toHaveAttribute("aria-pressed", "true");
  await expect(search).toHaveValue("");
  await expect(search).toBeFocused();

  await form.getByLabel("folder", { exact: true }).fill("/tmp");
  const launchButton = form.locator(".create-session-submit");
  // Premise for the empty-box launch shortcut: the button it activates is
  // enabled, and the box really is empty and focused. Filling the folder
  // field moved focus there, so the return to the search box is explicit —
  // the keyboard path being exercised is "back in the box, press Enter".
  await expect(launchButton).toBeEnabled();
  await search.focus();
  await expect(search).toHaveValue("");
  await expect(search).toBeFocused();
  await search.press("Enter");

  let created: ListedSession | undefined;
  await expect.poll(async () => {
    created = (await listedSessions(request)).find((session) => !before.has(session.id) && session.launch);
    return created?.launch ?? null;
  }, { message: "the ordinary Launch path must persist the structured selection" }).toMatchObject({
    harness: "codex",
    model: model.id,
    effort,
  });
  expect(created, "the persisted launch must identify the created session").toBeTruthy();
  await cleanupSession(request, created!.id);
});

/**
 * Name and host labels are picker actions, so accepting them must change the
 * visible draft without sending a create. The host action uses the same
 * selector and focus contract by pointer and keyboard. The aliased-local
 * identity case lives in the Rust search test; this browser fixture keeps
 * the local row's default name.
 */
test("name and host labels apply one draft choice without launching", async ({ page }) => {
  let creates = 0;
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() === "POST") creates += 1;
    await route.continue();
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator('.create-session-form[role="dialog"]');
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  const host = form.locator(".create-session-host");
  const local = host.locator("option").filter({ hasText: "local (this machine)" });
  const localId = await local.getAttribute("value");
  expect(localId, "the local row must have a stable selector value").toBeTruthy();
  const remote = host.locator("option").filter({ hasNotText: "local (this machine)" }).first();
  const remoteId = await remote.getAttribute("value");
  const remoteLabel = (await remote.textContent())?.trim();
  expect(remoteId, "the fixture must offer another host to filter").toBeTruthy();
  expect(remoteLabel).toBeTruthy();

  await search.fill("name:fix: parser");
  await expect(form.getByRole("option", { name: "Set session name: fix: parser", exact: true, selected: true }))
    .toBeVisible();
  await search.press("Enter");
  await expect(form.getByLabel("name (optional)")).toHaveValue("fix: parser");
  await expect(search).toHaveValue("");
  await expect(search).toBeFocused();

  await search.fill(`host:${remoteLabel}`);
  await form.getByRole("option", { name: `Host: ${remoteLabel}`, exact: true }).click();
  await expect(host).toHaveValue(remoteId!);
  await expect(search).toHaveValue("");
  await expect(search).toBeFocused();

  await search.fill("host:local");
  await expect(form.getByRole("option", { name: "Host: local (this machine)", exact: true, selected: true }))
    .toBeVisible();
  await search.press("Enter");
  await expect(host).toHaveValue(localId!);
  await expect(search).toHaveValue("");
  await expect(search).toBeFocused();
  expect(creates, "search acceptance must not submit the form").toBe(0);
});

/**
 * An empty search is a launch shortcut only after the dialog has a valid
 * complete selection; an incomplete draft must keep the dialog open and make
 * no create request. The launch itself is asynchronous on the page side (the
 * key hands off to the browser's implicit submission), so the negative
 * assertions are made only after a later UI round-trip through the same
 * input has completed, which orders them after anything the Enter started.
 */
test("empty composer search does not launch an incomplete selection", async ({ page, request }) => {
  const before = new Set((await listedSessions(request)).map((session) => session.id));
  let creates = 0;
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() === "POST") creates += 1;
    await route.continue();
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();

  const form = page.locator('.create-session-form[role="dialog"]');
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await expect(search).toBeFocused();
  const launchButton = form.locator(".create-session-submit");
  await expect(launchButton, "the incomplete selection must disable the empty-box launch shortcut").toBeDisabled();
  // The in-page observable that distinguishes "no submit event fired" from
  // "a POST was not observed yet": the browser declining to click a disabled
  // default button means the form never receives `submit` at all, and a
  // listener on the form sees that synchronously, where the route counter
  // and the listing read below are only the outer belt.
  await page.evaluate(() => {
    const form = document.querySelector<HTMLFormElement>(".create-session-form")!;
    form.dataset.submitWitness = "0";
    form.addEventListener("submit", () => {
      form.dataset.submitWitness = "1";
    }, { once: true });
  });
  await search.press("Enter");

  // Ordering round-trip: a query typed after the Enter opens the result
  // surface, and the page cannot have rendered that before it processed the
  // Enter and whatever submission it did or did not start.
  await search.fill("no-result-after-empty-enter");
  await expect(search).toHaveAttribute("aria-expanded", "true");

  await expect(form).toBeVisible();
  await expect(form, "no submit event may fire for a disabled launch button").toHaveAttribute("data-submit-witness", "0");
  expect(creates, "a disabled Launch button must not receive the empty-search shortcut").toBe(0);
  const after = await listedSessions(request);
  expect(after.map((session) => session.id).sort(), "no session may have been created by the empty Enter")
    .toEqual([...before].sort());
});

/**
 * Command mode is a complete branch of the shared composer, not a secondary
 * form or an interpretation of arbitrary query text. The test keeps a
 * conflicting structured draft dormant while it checks shared controls,
 * search scope, mode-local draft restoration, focus, and the outgoing raw
 * request. Those observables distinguish a coherent mode from controls that
 * merely remain visible while another draft is submitted.
 */
test("New selects other command explicitly and preserves the raw invocation", async ({ page, request }) => {
  const model = await codexCatalogModel(request);
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator('.create-session-form[role="dialog"]');
  const search = form.locator('.launch-composer-search input[role="combobox"]');

  // Seed structured-only choices before leaving the mode. Command search and
  // summary must then ignore them without destroying the draft.
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Claude", exact: true }).click();
  await form.getByRole("button", { name: "yolo", exact: true }).click();

  await search.fill("command");
  await expect(form.getByRole("option", { name: "Other / command", exact: true })).toBeVisible();
  await search.press("Enter");
  await expect(form).toHaveAttribute("data-composer-mode", "command");
  await expect(search, "search acceptance keeps the shared combobox focused in command mode").toBeFocused();
  await expect(form.locator(".launch-composer-summary"), "structured settings are dormant in command mode").toHaveCount(0);
  await expect(form.locator(".create-session-host"), "destination controls stay mounted in command mode").toBeVisible();
  await expect(form.getByLabel("folder", { exact: true })).toBeVisible();
  await expect(form.getByLabel("name (optional)")).toBeVisible();
  const profile = form.locator(".create-session-profile");
  await profile.selectOption("");
  const command = form.getByLabel("agent command");
  await command.fill(FAKE_AGENT);

  await search.fill(model.id);
  await expect(form.getByRole("option", { name: `Model: ${model.id} (Codex)`, exact: true }),
    "command search exposes globally owned models instead of filtering by a dormant harness").toBeVisible();
  await search.press("Enter");
  await expect(form, "accepting an owned model explicitly activates its structured harness").toHaveAttribute("data-composer-mode", "structured");
  await expect(form.getByRole("combobox", { name: "model", exact: true })).toHaveValue(model.id);
  await expect(search).toBeFocused();
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "other / command", exact: true }).click();
  await expect(command, "the command draft survives a structured search round trip").toHaveValue(FAKE_AGENT);

  await search.fill(model.efforts[0]);
  await expect(form.getByRole("group", { name: "Efforts" }),
    "command search offers no action that only edits the dormant structured draft").toHaveCount(0);

  await search.fill("/tmp");
  // Recent folders can also contain /tmp. Enter accepts the selected direct
  // path action, independently of any matching entry in persisted history.
  await expect(form.getByRole("option", { name: "Use this path: /tmp", exact: true, selected: true })).toBeVisible();
  await search.press("Enter");
  await expect(form).toHaveAttribute("data-composer-mode", "command");
  await expect(form.getByLabel("folder", { exact: true })).toHaveValue("/tmp");
  await expect(search, "folder acceptance keeps focus in shared search").toBeFocused();

  const [response] = await Promise.all([
    page.waitForResponse((candidate) => candidate.request().method() === "POST" && candidate.url().endsWith("/api/sessions")),
    form.locator(".create-session-submit").click(),
  ]);
  const body = response.request().postDataJSON();
  expect(body).toMatchObject({ invocation: FAKE_AGENT });
  expect(body).not.toHaveProperty("launch");
  const created = await response.json() as ListedSession;
  await cleanupSession(request, created.id);
});
