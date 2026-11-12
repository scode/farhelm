/** Session-owned UI uses a real remote session, with displayed ownership and
 * vendor state controlled at the HTTP boundary. Recipe admission is refused
 * deliberately: this proves consent and placement without pretending the fake
 * vendor provides a dialable new sandbox. Backend tests cover that task. */
import { expect, test } from "./helpers/evidence";
import type { APIRequestContext, Page } from "@playwright/test";
import { cleanupSession, createSession, localHostId, openHostMenu, openHostsPanel, openRowMenu, patchPreferences, pinAutoSelect, readPreferences, stubFeed } from "./helpers/fleet";
import { drainedRoute } from "./helpers/drained-route";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { answerYolo, fillCreateForm, waitForTermText } from "./helpers/term";
import { routeGate } from "./helpers/route-gate";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";
import { hostChoice } from "./helpers/host-rail";

/** Every controlled response retains the serving build stamp. Owned remote
 * identity and session transport stay real; only its presentation is replaced.
 * Ordinary-host mode exercises Resume-before-create without New choosing the
 * fresh recipe that a session-owned sandbox deliberately defaults to. A host that
 * is not suspended at start offers Suspend and Stop and suspend directly, and
 * `refresh` announces a new revision so the page reads its listings again.
 * `present` shows the host in another phase the fake cannot reach on its own
 * (a sandbox being deleted after a seal); it takes effect at the page's next
 * hosts and sessions reads, which then show the host's sessions stale as the
 * helm does for a host it has no connection to. The real connection and the
 * sessions themselves are untouched. */
async function ownedHost(page: Page, request: APIRequestContext, sessionOwned = true, suspendedAtStart = true) {
  const response = await request.get("/api/hosts");
  expect(response.ok()).toBe(true);
  const build = response.headers()["x-farhelm-build"];
  expect(build).toBeTruthy();
  const remote = (await response.json()).hosts.find((host: any) => host.kind === "ssh" && host.state.phase === "connected");
  expect(remote, "owned stack has a connected remote supervisor").toBeTruthy();
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  let revision = 1;
  let suspended = suspendedAtStart;
  let presented: object | null = null;
  // Set by `remove`: the hosts read no longer lists the sandbox, as after
  // its destruction (another window's sealing Delete, or Hosts' Delete).
  let removed = false;
  const stops: Array<() => Promise<void>> = [];
  const calls: string[] = [];
  stops.push(await drainedRoute(page, "**/api/hosts", async (route) => {
    const reply = await route.fetch();
    const json = await reply.json();
    json.hosts = json.hosts.filter((host: any) => !(removed && host.id === remote.id)).map((host: any) => host.id === remote.id ? {
      ...host,
      name: "Owned sandbox",
      kind: "tensorlake",
      is_session_owned: sessionOwned,
      state: presented ?? (suspended
        ? { phase: "suspended", observed_at: Date.now(), suspended_by_farhelm_at: null, resume_failed: null }
        : host.state),
      tensorlake: { sandbox: "farhelm-sess-e2e-owned", recipe: "session-ready", size: { cpus: 2, memory_gb: 4, disk_gb: 20 } },
    } : host);
    await route.fulfill({ response: reply, json });
  }));
  stops.push(await drainedRoute(page, /\/api\/sessions(\?|$)/, async (route) => {
    if (route.request().method() !== "GET") return route.fallback();
    const reply = await route.fetch();
    if (!reply.ok()) return route.fulfill({ response: reply });
    const json = await reply.json();
    // The actual helm marks a disconnected host's cached sessions stale. The
    // presentation fixture must reproduce that fact as well as the host phase.
    // Every phase `present` shows (being deleted) has no connection either.
    json.sessions = json.sessions.map((session: any) => session.host === remote.id
      ? { ...session, stale: suspended || presented !== null || session.stale }
      : session);
    await route.fulfill({ response: reply, json });
  }));
  for (const verb of ["suspend", "resume"]) {
    stops.push(await drainedRoute(page, `**/api/hosts/${remote.id}/${verb}`, async (route) => {
      expect(route.request().method()).toBe("POST");
      calls.push(verb);
      suspended = verb === "suspend";
      await route.fulfill({ status: 200, headers: { "x-farhelm-build": build }, json: {} });
      await feed.waitForConnection(1);
      feed.notify(++revision);
    }));
  }
  return {
    remote, build, calls, stops,
    refresh: () => feed.notify(++revision),
    present: (state: object | null) => { presented = state; },
    remove: () => { removed = true; },
  };
}

/** A ready recipe and an unbuilt recipe differ only in the prerequisite the
 * picker must explain. No saved account or vendor resource is changed. */
async function recipeChoices(page: Page, build: string) {
  const stops = [];
  stops.push(await drainedRoute(page, "**/api/tensorlake", async (route) => {
    const reply = await route.fetch();
    const json = await reply.json();
    json.api_key_set = true;
    json.create_without_asking = false;
    json.ssh_key = {
      state: "ready", public_key: "ssh-ed25519 fixture", public_key_path: "fixture.pub",
      name: "fixture", register_command: "tl sbx ssh keys add --name fixture fixture.pub",
    };
    await route.fulfill({ response: reply, json });
  }));
  stops.push(await drainedRoute(page, "**/api/tensorlake/recipes", async (route) => {
    const base = { kind: "tensorlake_ubuntu" };
    const recipe = {
      size: { cpus: 2, memory_gb: 4, disk_gb: 20 }, base, packages: [], claude: null, codex: null,
      claude_login_credential: null, codex_login_credential: null, github_token_credential: null, tunnels: [],
    };
    await route.fulfill({ status: 200, headers: { "x-farhelm-build": build }, json: { recipes: [
      { ...recipe, name: "session-ready", snapshot: {
        id: "fixture-built", built_at: 1, base, disk_gb: 20, packages: [], claude_version: null, codex_version: null,
      } },
      { ...recipe, name: "session-unbuilt", snapshot: null },
    ] } });
  }));
  return stops;
}

/** Billing consent must precede a recipe request and retain modal ownership.
 * Only the admitted request frees navigation; its refusal restores the same
 * draft and must never fall back to ordinary session creation on Local. */
test("recipe placement explains prerequisites and requires billing consent", async ({ page, request }, testInfo) => {
  const response = await request.get("/api/hosts");
  const build = response.headers()["x-farhelm-build"];
  expect(build).toBeTruthy();
  const stops = await recipeChoices(page, build);
  const bodies: any[] = [];
  const held = routeGate();
  const owned: string[] = [];
  let uncertainReply = false;
  let ordinaryCreates = 0;
  stops.push(await drainedRoute(page, "**/api/environment-sessions", async (route) => {
    expect(route.request().method()).toBe("POST");
    bodies.push(route.request().postDataJSON());
    await held.wait();
    if (uncertainReply) return route.fulfill({ status: 200, headers: {
      "x-farhelm-build": build, "x-farhelm-environment-host": "3",
      "x-farhelm-environment-run": "1ddba07b-2a1a-46c6-945a-b09d87e741a8",
    }, body: "incomplete launch result" });
    await route.fulfill({ status: 409, headers: { "x-farhelm-build": build }, body: "recipe fixture refused launch" });
  }));
  stops.push(await drainedRoute(page, "**/api/sessions", async (route) => {
    if (route.request().method() === "POST") ordinaryCreates++;
    await route.continue();
  }));
  try {
    const source = await createSession(request, { title: `recipe-source-${Date.now()}` });
    owned.push(source.id);
    const other = await createSession(request, { title: `recipe-other-${Date.now()}` });
    owned.push(other.id);
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await form.getByRole("button", { name: "New sandbox", exact: true }).click();
    const recipes = form.getByRole("combobox", { name: "Recipe", exact: true });
    await expect(recipes).toBeVisible();
    const unbuilt = recipes.locator("option").filter({ hasText: "session-unbuilt" });
    await expect(unbuilt).toContainText("No snapshot yet");
    // The option owns this restriction. Playwright retargets its disabled-state
    // check through the wrapping label to the enabled recipe select.
    await expect(unbuilt).toHaveJSProperty("disabled", true);
    await recipes.selectOption("session-ready");
    await expect(recipes).toHaveValue("session-ready");
    await form.getByRole("tab", { name: "Command", exact: true }).click();
    await form.getByLabel("Agent command", { exact: true }).fill("sleep 300");
    await answerYolo(form, true);
    await form.locator(".create-session-submit").click();
    const question = form.getByRole("alertdialog", { name: "Create a sandbox for this session?", exact: true });
    await expect(question).toContainText("bills while running");
    await expect(question).toContainText("30 minutes");
    await expect(form).toHaveAttribute("aria-modal", "true");
    await page.locator(`[data-session-id="${other.id}"] .session-row-open`).focus();
    expect(await form.evaluate(node => node.contains(document.activeElement)), "billing consent still owns the keyboard").toBe(true);
    expect(bodies).toEqual([]);
    await question.getByRole("button", { name: "Cancel", exact: true }).click();
    expect(bodies).toEqual([]);
    await form.locator(".create-session-submit").click();
    await question.getByRole("button", { name: "Create and launch", exact: true }).click();
    await expect.poll(() => bodies.length, { message: "the confirmed recipe POST is held" }).toBe(1);
    await expect(form).toHaveCount(1);
    await expect(form).toBeHidden();
    await expect(form).not.toHaveAttribute("aria-modal", "true");
    const otherRow = page.locator(`[data-session-id="${other.id}"]`);
    await otherRow.locator(".session-row-open").click();
    await expect(otherRow).toHaveAttribute("data-session-selected", "true");
    held.release();
    await expect(form.getByRole("alert")).toContainText("recipe fixture refused launch");
    await expect(form).toBeVisible();
    await expect(form).toHaveAttribute("aria-modal", "true");
    await expect(form.getByLabel("Agent command", { exact: true })).toHaveValue("sleep 300");
    await expect(form.getByRole("combobox", { name: "Recipe", exact: true })).toHaveValue("session-ready");
    await expect.poll(() => form.evaluate(node => node.contains(document.activeElement)),
      { message: "the recipe refusal restores its modal draft's keyboard ownership" }).toBe(true);
    expect(bodies).toHaveLength(1);
    expect(bodies[0]).toMatchObject({ recipe: "session-ready", place: {kind:"tensorlake_account"}, confirmed: true, request: { cwd: "~" } });
    expect(bodies[0].request).not.toHaveProperty("host");
    expect(bodies[0].request).not.toHaveProperty("expected_incarnation");
    expect(ordinaryCreates).toBe(0);
    // Losing the admitted result cannot establish that the owned task refused
    // to launch. It must point to Hosts without the definite refusal prefix.
    uncertainReply = true;
    await form.locator(".create-session-submit").click();
    await question.getByRole("button", { name: "Create and launch", exact: true }).click();
    await expect(form.getByRole("alert")).toContainText("Check Hosts");
    await expect(form.getByRole("alert")).not.toContainText("Not launched.");
    expect(bodies).toHaveLength(2);
    expect(ordinaryCreates).toBe(0);
  } finally {
    held.release();
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...stops.map((stop, index): [string, () => Promise<void>] => [`recipe route ${index}`, stop]),
      ...owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});

/** A submitted launch on a suspended existing host first waits for Resume.
 * That preparation is already owned work, so it must free navigation too and
 * survive the source view unmounting before the create's refusal returns. */
test("an existing-host launch stays nonmodal while its Resume is held", async ({ page, request }, testInfo) => {
  const fixture = await ownedHost(page, request, false);
  const held = routeGate();
  const owned: string[] = [];
  const bodies: any[] = [];
  let resumes = 0;
  try {
    const source = await createSession(request, { title: `launch-resume-source-${Date.now()}`, host: fixture.remote.id });
    owned.push(source.id);
    const other = await createSession(request, { title: `launch-resume-other-${Date.now()}`, host: await localHostId(request) });
    owned.push(other.id);
    fixture.stops.push(await drainedRoute(page, `**/api/hosts/${fixture.remote.id}/resume`, async route => {
      expect(route.request().method()).toBe("POST");
      resumes++;
      await held.wait();
      await route.fallback();
    }));
    fixture.stops.push(await drainedRoute(page, "**/api/sessions", async route => {
      if (route.request().method() !== "POST") return route.fallback();
      bodies.push(route.request().postDataJSON());
      await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.build }, body: "resumed-host create refused" });
    }));
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    // This ordinary-host fixture keeps real session details live. Its host
    // snapshot, rather than a stale-session overlay, authorizes launch Resume.
    await expect(page.locator(`[data-session-id="${source.id}"]`)).toHaveAttribute("data-session-selected", "true");
    await expect(hostChoice(page, fixture.remote.id)).toHaveAccessibleName(/: Suspended$/);
    const form = await fillCreateForm(page, { cwd: "/tmp", invocation: "true", title: "resume-owned draft" });
    await expect(form.getByRole("combobox", { name: "Host", exact: true })).toContainText("Owned sandbox");
    await form.locator(".create-session-submit").click();
    await expect.poll(() => resumes, { message: "launch preparation is held at its existing-host Resume" }).toBe(1);
    expect(bodies).toHaveLength(0);
    await expect(form).toHaveCount(1);
    await expect(form).toBeHidden();
    await expect(form).not.toHaveAttribute("aria-modal", "true");
    const otherRow = page.locator(`[data-session-id="${other.id}"]`);
    await otherRow.locator(".session-row-open").click();
    await expect(otherRow).toHaveAttribute("data-session-selected", "true");
    held.release();
    await expect(form.getByRole("alert")).toContainText("resumed-host create refused");
    await expect(form).toBeVisible();
    await expect(form).toHaveAttribute("aria-modal", "true");
    await expect(form.getByLabel("Name (optional)")).toHaveValue("resume-owned draft");
    await expect.poll(() => form.evaluate(node => node.contains(document.activeElement)),
      { message: "a resumed-host refusal returns keyboard ownership to the same draft" }).toBe(true);
    expect(resumes).toBe(1);
    expect(fixture.calls).toEqual(["resume"]);
    expect(bodies).toHaveLength(1);
    expect(bodies[0].host).toBe(fixture.remote.id);
    await expect(otherRow).toHaveAttribute("data-session-selected", "true");
  } finally {
    held.release();
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => [`launch Resume route ${index}`, stop]),
      ...owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});

/** Template quick launch must retain recipe placement and ask the same billing
 * question. An obsolete or refused recipe cannot become an installed host. */
test("a recipe template quick launch asks before one recipe request", async ({ page, request }, testInfo) => {
  const name = `e2e-session-template-${Date.now()}`;
  const response = await request.get("/api/hosts");
  const build = response.headers()["x-farhelm-build"];
  expect(build).toBeTruthy();
  const saved = await request.put(`/api/templates/${name}`, { data: {
    kind: "command", command: "sleep 300", yolo: false, recipe: "session-ready", destination: { folder: "~" },
  } });
  expect(saved.ok(), await saved.text()).toBe(true);
  const stops = await recipeChoices(page, build);
  stops.push(await drainedRoute(page, "**/api/preferences", async (route) => {
    if (route.request().method() !== "GET") return route.fallback();
    const response = await route.fetch();
    const preferences = await response.json();
    await route.fulfill({ response, json: { ...preferences, template_shortcuts: { "1": name } } });
  }));
  const bodies: any[] = [];
  stops.push(await drainedRoute(page, "**/api/environment-sessions", async (route) => {
    bodies.push(route.request().postDataJSON());
    await route.fulfill({ status: 409, headers: { "x-farhelm-build": build }, body: "template recipe fixture refused" });
  }));
  try {
    await page.goto("/");
    await expect(page.locator(".template-number-trigger")).toHaveCount(10);
    await page.keyboard.press("Control+Shift+Digit1");
    const question = page.getByRole("alertdialog", { name: "Create a sandbox for this session?", exact: true });
    await expect(question).toContainText("session-ready");
    expect(bodies).toEqual([]);
    await question.getByRole("button", { name: "Create and launch", exact: true }).click();
    await expect(page.locator(".create-session-form").getByRole("alert")).toContainText("template recipe fixture refused");
    expect(bodies).toHaveLength(1);
    expect(bodies[0]).toMatchObject({ recipe: "session-ready", confirmed: true });
    expect(bodies[0].request).not.toHaveProperty("host");
  } finally {
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...stops.map((stop, index): [string, () => Promise<void>] => [`template route ${index}`, stop]),
      ["template", async () => {
        const removed = await request.delete(`/api/templates/${name}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }],
    ]);
  }
});

/** Leaving an environment keeps Host editable so the selected installed host
 * can be saved. An active Existing host click must retain that identity.
 * A separately added Host field still starts present and empty. */
test("template placement keeps Host editable after leaving an environment", async ({ page, request }, testInfo) => {
  const name = `e2e-recipe-editor-${Date.now()}`;
  const ordinary = `${name}-ordinary`;
  const response = await request.get("/api/hosts");
  expect(response.ok()).toBe(true);
  const build = response.headers()["x-farhelm-build"];
  expect(build).toBeTruthy();
  const remote = (await response.json()).hosts.find((host: any) => host.kind === "ssh" && host.state.phase === "connected");
  expect(remote, "the owned stack has a connected ordinary remote host").toBeTruthy();
  expect(remote.identity, "the selected host has an installed identity to store").toBeTruthy();
  for (const [target, fields] of [
    [name, { recipe: "session-ready" }], [ordinary, { kind: "command", command: "sleep 300", yolo: false }],
  ] as const) {
    const saved = await request.put(`/api/templates/${target}`, { data: fields });
    expect(saved.ok(), await saved.text()).toBe(true);
  }
  const stops = await recipeChoices(page, build);
  try {
    await page.goto("/");
    await page.getByRole("button", { name: "Templates", exact: true }).click();
    const dialog = page.locator('.templates-dialog[role="dialog"]');
    await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    await dialog.locator(".templates-row").filter({ has: page.getByText(name, { exact: true }) }).click();
    const recipe = dialog.getByRole("combobox", { name: "Recipe", exact: true });
    await expect(recipe).toHaveValue("session-ready");
    await expect(dialog.getByRole("button", {name:"New sandbox",exact:true})).toHaveAttribute("aria-pressed","true");
    await dialog.getByRole("button", { name: "Existing host", exact: true }).click();
    const host = dialog.getByRole("combobox", { name: "Host", exact: true });
    await expect(host).toBeVisible();
    await expect(host).toHaveValue("");
    await expect(host.locator("option:checked")).toHaveText("Choose a host");
    await host.selectOption(remote.identity);
    await expect(host).toHaveValue(remote.identity);
    await dialog.getByRole("button", { name: "Existing host", exact: true }).click();
    await expect(host).toBeVisible();
    await expect(host).toHaveValue(remote.identity);
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    const saved = await request.get("/api/templates");
    expect(saved.ok()).toBe(true);
    const stored = (await saved.json()).templates.find((template: any) => template.name === name);
    expect(stored, "the edited template remains stored").toBeTruthy();
    expect(stored.fields.host).toBe(remote.identity);
    expect(stored.fields.target ?? null).toBeNull();
    expect(stored.fields.recipe ?? null).toBeNull();
    await dialog.locator(".templates-row").filter({ has: page.getByText(ordinary, { exact: true }) }).click();
    await dialog.locator(".templates-add").click();
    await dialog.locator(".templates-add-menu").getByRole("button", { name: "Host", exact: true }).click();
    await expect(host).toBeVisible();
    await expect(host).toHaveValue("");
    await expect(host.locator("option:checked")).toHaveText("Choose a host");
  } finally {
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ["page", () => page.goto("about:blank").then(() => {})],
      ...stops.map((stop, index): [string, () => Promise<void>] => [`editor route ${index}`, stop]),
      ...[name, ordinary].map((target): [string, () => Promise<void>] => [target, async () => {
        const removed = await request.delete(`/api/templates/${target}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }]),
    ]);
  }
});

/** Row Stop-and-suspend is available even when ordinary Stop skips its question.
 * Its own question covers the sandbox-wide consequence, and dispatch uses the combined
 * helm endpoint rather than a client-side chain of independent mutations. */
test("Resume restores owned controls and Stop and suspend keeps its host-wide question", async ({ page, request }, testInfo) => {
  const fixture = await ownedHost(page, request);
  const previous = await readPreferences(request);
  const session = await createSession(request, { title: "owned-resume", host: fixture.remote.id });
  const requests: any[] = [];
  let activityReads = 0;
  fixture.stops.push(await drainedRoute(page, "**/api/sessions?**", async (route) => {
    if (route.request().method() !== "GET" || new URL(route.request().url()).searchParams.get("host") !== String(fixture.remote.id)) {
      return route.fallback();
    }
    const reply = await route.fetch();
    const json = await reply.json();
    expect(json.sessions.some((current: any) => current.id === session.id), "activity read contains the owned agent").toBe(true);
    activityReads++;
    json.sessions = json.sessions.map((current: any) => current.id === session.id
      ? { ...current, stale: false, status: { state: "running" } }
      : current);
    await route.fulfill({ response: reply, json });
  }));
  fixture.stops.push(await drainedRoute(page, `**/api/sessions/${encodeURIComponent(session.id)}/stop-and-suspend`, async (route) => {
    expect(route.request().method()).toBe("POST");
    requests.push(route.request().postDataJSON());
    await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.build }, body: "combined action fixture refused" });
  }));
  try {
    await patchPreferences(request, { skip_session_confirmations: true });
    await pinAutoSelect(page, session.id);
    await page.goto("/");
    const card = page.getByRole("region", { name: "Session unavailable", exact: true });
    await expect(card).toContainText("Session suspended");
    await card.getByRole("button", { name: "Resume", exact: true }).click();
    await expect.poll(() => fixture.calls).toEqual(["resume"]);
    await expect(card).toHaveCount(0);
    await expect(page.locator(".header-stop")).toBeEnabled();
    await expect(page.locator(".header-stop-anchor > .sandbox-session-controls")
      .getByRole("button", { name: "Suspend", exact: true })).toBeVisible();
    const row = page.locator(".session-row").filter({ hasText: "owned-resume" });
    await expect(row).toHaveCount(1);
    await openRowMenu(row);
    await page.getByRole("menuitem", { name: "Replace with", exact: true }).click();
    const replacement = page.locator(".create-session-form");
    await expect(replacement).toBeVisible();
    await expect(replacement.getByRole("combobox", { name: "Host", exact: true })).toContainText("Owned sandbox");
    await replacement.locator(".launch-composer-cancel").click();
    await expect(replacement).toHaveCount(0);
    await openRowMenu(row);
    const beforeActivity = activityReads;
    await page.getByRole("menuitem", { name: /^Stop and suspend/ }).click();
    const question = page.getByRole("alertdialog", { name: "Stop and suspend sandbox?", exact: true });
    await expect(question).toContainText("Every other process freezes");
    expect(activityReads).toBeGreaterThan(beforeActivity);
    expect(requests).toEqual([]);
    await question.getByRole("button", { name: "Stop and suspend", exact: true }).click();
    await expect.poll(() => requests.length).toBe(1);
    // Combined Stop failures use the persistent lifecycle result, visible on
    // both the source row and its selected header rather than local controls.
    await expect(row.locator(".action-error").first()).toContainText("combined action fixture refused");
    await expect(page.locator(".stop-error")).toContainText("combined action fixture refused");
    expect(requests).toHaveLength(1);
    expect(fixture.calls).toEqual(["resume"]);
  } finally {
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => [`resume route ${index}`, stop]),
      ["session", () => cleanupSession(request, session.id)],
      ["preferences", () => patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null })],
    ]);
  }
});

/** SPEC.md "Lifecycle operations": a Stop whose click showed no tabs is refused if
 * any tab is open, and one whose click showed an ended agent is refused if the
 * agent is alive again. Stop and suspend reads activity between the click and its
 * request; a listing refresh landing in that wait (a tab opened in another window,
 * the agent restarted from the command line) must not weaken either guard. The
 * activity read is held until the refreshed row is drawn, then the skipped
 * question's request must still carry both refusals. */
test("Stop and suspend sends the guards its click saw when the listing refreshes during its activity read", async ({ page, request }, testInfo) => {
  const fixture = await ownedHost(page, request, true, false);
  const session = await createSession(request, { title: "owned-click-guards", host: fixture.remote.id });
  const activity = routeGate();
  let activityHeld = false;
  let refreshed = false;
  const bodies: any[] = [];
  // Newest route: it serves both the sidebar listing and the activity read, which
  // differ only by the activity read's host filter.
  fixture.stops.push(await drainedRoute(page, /\/api\/sessions(\?|$)/, async (route) => {
    if (route.request().method() !== "GET") return route.fallback();
    const isActivity = new URL(route.request().url()).searchParams.get("host") === String(fixture.remote.id);
    if (isActivity) {
      activityHeld = true;
      await activity.wait();
    }
    const reply = await route.fetch();
    const json = await reply.json();
    json.sessions = json.sessions.map((current: any) => {
      if (isActivity && current.host === fixture.remote.id) {
        // Nothing working on the sandbox: Suspend's activity question is skipped.
        return { ...current, stale: false, status: { state: "exited", exit_code: 0 }, tabs: [] };
      }
      if (current.id !== session.id) return current;
      return refreshed
        ? { ...current, stale: false, status: { state: "running" }, tabs: [{ id: "e2e-opened-elsewhere" }] }
        : { ...current, stale: false, status: { state: "exited", exit_code: 0 }, tabs: [] };
    });
    await route.fulfill({ response: reply, json });
  }));
  fixture.stops.push(await drainedRoute(page, `**/api/sessions/${encodeURIComponent(session.id)}/stop-and-suspend`, async (route) => {
    bodies.push(route.request().postDataJSON());
    await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.build }, body: "guard fixture refused" });
  }));
  try {
    await page.goto("/");
    const row = page.locator(".session-row").filter({ hasText: "owned-click-guards" });
    await expect(row).toContainText("Exited");
    await openRowMenu(row);
    await page.getByRole("menuitem", { name: /^Stop and suspend/ }).click();
    await expect.poll(() => activityHeld).toBe(true);
    refreshed = true;
    fixture.refresh();
    // The refreshed render replaced the row's controls before the wait ends.
    await expect(row).toHaveCount(1);
    await expect(row).not.toContainText("Exited");
    activity.release();
    await expect.poll(() => bodies.length).toBe(1);
    expect(bodies[0]).toEqual({ refuse_if_tabs_open: true, refuse_if_agent_running: true });
  } finally {
    activity.release();
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => [`guard route ${index}`, stop]),
      ["session", () => cleanupSession(request, session.id)],
    ]);
  }
});

/** Resume is a session-owned action even though it calls the host endpoint.
 * Hold that fixture endpoint across a view switch, then apply its existing
 * connected transition. Delete is normally allowed while suspended, so its
 * refusal during the wait proves the operation claim rather than frozen state. */
test("an owned session's pending Resume survives switching and excludes its competing actions", async ({ page, request }, testInfo) => {
  const fixture = await ownedHost(page, request);
  const held = routeGate();
  const owned: string[] = [];
  let resumeRequests = 0;
  let deleteRequests = 0;
  try {
    const local = await localHostId(request);
    expect(local, "the independent session must not share the owned sandbox host").not.toBe(fixture.remote.id);
    const source = await createSession(request, { title: `owned-resume-switch-${Date.now()}`, host: fixture.remote.id });
    owned.push(source.id);
    const other = await createSession(request, { title: `owned-resume-independent-${Date.now()}`, host: local });
    owned.push(other.id);
    // This newest route only holds admission. Falling through after release
    // uses ownedHost's existing success, phase change and feed notification.
    fixture.stops.push(await drainedRoute(page, `**/api/hosts/${fixture.remote.id}/resume`, async (route) => {
      expect(route.request().method()).toBe("POST");
      resumeRequests++;
      await held.wait();
      await route.fallback();
    }));
    fixture.stops.push(await drainedRoute(page, `**/api/sessions/${encodeURIComponent(source.id)}?delete_sandbox=true`, async (route) => {
      deleteRequests++;
      await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.build }, body: "unexpected competing Delete" });
    }));
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    const card = page.getByRole("region", { name: "Session unavailable", exact: true });
    const sourceRow = page.locator(`.session-row[data-session-id="${source.id}"]`);
    const otherRow = page.locator(`.session-row[data-session-id="${other.id}"]`);
    await expect(sourceRow).toHaveAttribute("data-session-selected", "true");
    await expect(sourceRow).toHaveClass(/\bstale\b/);
    await expect(card).toContainText("Session suspended");
    await expect(page.locator(".header-delete"), "Delete works on a suspended sandbox before Resume claims it").toBeEnabled();
    const resume = page.locator(".header-stop-anchor > .sandbox-session-controls")
      .getByRole("button", { name: "Resume", exact: true });
    await resume.click();
    await expect.poll(() => resumeRequests, { message: "the accepted Resume request remains held before its fixture state change" }).toBe(1);
    expect(fixture.calls).toEqual([]);
    await expect(page.locator(".header-delete")).toBeDisabled();
    await otherRow.locator(".session-row-open").click();
    await expect(otherRow).toHaveAttribute("data-session-selected", "true");
    await waitForSessionRevealed(page, other.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(page.locator(".header-stop")).toBeEnabled();
    await sourceRow.locator(".session-row-open").click();
    await expect(card).toContainText("Session suspended");
    await expect(page.locator(".header-delete")).toBeDisabled();
    await page.locator(".header-delete").dispatchEvent("click");
    await resume.dispatchEvent("click");
    await expect(page.locator(".header-delete-confirm")).toHaveCount(0);
    expect(deleteRequests).toBe(0);
    await otherRow.locator(".session-row-open").click();
    await expect(otherRow).toHaveAttribute("data-session-selected", "true");
    held.release();
    await expect.poll(() => fixture.calls).toEqual(["resume"]);
    await expect(sourceRow).not.toHaveClass(/\bstale\b/);
    await expect(otherRow).toHaveAttribute("data-session-selected", "true");
    await sourceRow.locator(".session-row-open").click();
    await expect(card).toHaveCount(0);
    await expect(page.locator(".header-stop-anchor > .sandbox-session-controls")
      .getByRole("button", { name: "Suspend", exact: true })).toBeEnabled();
    await expect(page.locator(".header-stop")).toBeEnabled();
    await expect(page.locator(".header-delete")).toBeEnabled();
    expect(resumeRequests).toBe(1);
    expect(deleteRequests).toBe(0);
  } finally {
    held.release();
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => [`owned resume-switch route ${index}`, stop]),
      ...owned.map((id): [string, () => Promise<void>] => ["owned real session", () => cleanupSession(request, id)]),
    ]);
  }
});

/** Suspension preserves the real session while its owned-machine presentation
 * disables supervisor actions. The card describes preserved processes and
 * Tensorlake's session idle timeout, independently of Suspend's action help.
 * Delete must still ask with the global skip on. SPEC.md, "Sandbox sessions":
 * the helm cannot ask a suspended sandbox's Farhelm, so Delete goes straight
 * to the warned whole-sandbox confirmation (never the one that asks, and never
 * Resume), which names unlisted sessions and the way to the narrower Delete,
 * and then explicitly authorizes whole-resource destruction. */
test("owned suspended sessions keep Hosts visibility and mandatory whole-sandbox Delete", async ({ page, request }, testInfo) => {
  const fixture = await ownedHost(page, request);
  const previous = await readPreferences(request);
  const session = await createSession(request, { title: "owned-suspended", host: fixture.remote.id });
  let deletes = 0;
  let asked = 0;
  fixture.stops.push(await drainedRoute(page, `**/api/sessions/${encodeURIComponent(session.id)}?delete_sandbox=true`, async (route) => {
    expect(route.request().method()).toBe("DELETE");
    deletes++;
    await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.build }, body: "sandbox deletion fixture refused" });
  }));
  fixture.stops.push(await drainedRoute(page, `**/api/sessions/${encodeURIComponent(session.id)}?delete_environment_session=true`, async (route) => {
    asked++;
    await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.build }, body: "unexpected environment Delete" });
  }));
  try {
    await patchPreferences(request, { skip_session_confirmations: true });
    await pinAutoSelect(page, session.id);
    await page.goto("/");
    const card = page.getByRole("region", { name: "Session unavailable", exact: true });
    await expect(card).toContainText("Session suspended");
    await expect(card).toContainText("This session's sandbox is suspended. Its processes are preserved, including the agent's running command, and continue when you resume it. Only snapshot storage bills while suspended.");
    await expect(card).toContainText("Suspended outside Farhelm, noticed at");
    await expect(card).toContainText("Tensorlake suspends this sandbox after 30 minutes without a connection from Farhelm.");
    await expect(page.locator(".header-stop")).toBeDisabled();
    await expect(page.locator(".restart-primary")).toBeDisabled();
    await expect(page.locator(".header-replace")).toBeDisabled();
    const row = page.locator(".session-row").filter({ hasText: "owned-suspended" });
    await expect(row).toHaveClass(/\bstale\b/);
    await expect(row).toContainText("session-ready sandbox");
    await expect(row.locator(".status-badge").filter({ hasText: "Suspended" }).locator("svg")).toHaveCount(1);
    await expect(page.locator(".header-stop-anchor > .sandbox-session-controls")
      .getByRole("button", { name: "Resume", exact: true })).toHaveClass(/\bbtn-primary\b/);
    await expect(page.locator(".host-rail-avatar").first()).toBeVisible();
    await expect(page.locator(`.host-rail [data-rail-host-id="${fixture.remote.id}"]`)).toHaveCount(0);
    await expect(page.locator(".header-delete")).toHaveAttribute("data-tooltip", /whole sandbox and everything in it/);
    await page.locator(".header-delete").click();
    const question = page.locator(".header-delete-confirm");
    await expect(question).toContainText("deletes the whole sandbox: every session");
    await expect(question).toContainText("including sessions Farhelm does not list");
    await expect(question).toContainText("Resuming or updating the sandbox first lets Farhelm delete just this session");
    await expect(question).toContainText("snapshot");
    await expect(question).toContainText("farhelm-sess-e2e-owned");
    expect(deletes).toBe(0);
    await question.getByRole("button", { name: "Delete session and sandbox", exact: true }).click();
    await expect.poll(() => deletes).toBe(1);
    expect(asked, "a suspended sandbox's Delete never asks its Farhelm").toBe(0);
    expect(fixture.calls).toEqual([]);
    await openHostsPanel(page);
    await expect(page.locator(`.host-row[data-host-id="${fixture.remote.id}"]`)).toContainText("Owned sandbox");
  } finally {
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => [`owned route ${index}`, stop]),
      ["session", () => cleanupSession(request, session.id)],
      ["preferences", () => patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null })],
    ]);
  }
});

// ===== Environment session Delete ===========================================
//
// SPEC.md, "Sandbox sessions": a connected sandbox's Delete asks the
// sandbox's own Farhelm (`?delete_environment_session=true`), which alone
// knows whether the session is its last; when the helm cannot ask, it answers
// 409 with the "unasked" marker and the page offers the warned whole-sandbox
// Delete (`?delete_sandbox=true`). The owned stack's "sandbox" is really an
// ordinary remote host, whose helm refuses both of those queries, so the
// tests below answer them at the HTTP boundary in the helm's own shape. The
// real helm and supervisor side of both is covered by the Rust tests (the
// helm's own, and the real-Podman `podman_environments` scenario).

/** The helm's "nothing was decided" marker, `ENVIRONMENT_UNASKED_HEADER`. */
const UNASKED = { "x-farhelm-environment-unasked": "nothing-decided" };

/** What one faked environment Delete answers: its status, JSON or text body,
 * whether it carries the unasked marker, and whether the session is really
 * deleted first (through the ordinary Delete, which the stack's ordinary host
 * allows), so the listing the page reads next agrees with a success. */
type EnvironmentAnswer = { status: number; json?: object; body?: string; unasked?: boolean; removes?: boolean };

/** Answer the environment Deletes of `id` with `answer`, recording each one's
 * scope in `sent`: `session` for the Delete that asks the sandbox's Farhelm,
 * `whole` for the warned whole-sandbox Delete. A plain DELETE of the session
 * fails the test: the page must never send one for an environment session.
 * `answer` gets the scope and how many Deletes this route saw before. */
async function environmentDeletes(
  page: Page,
  request: APIRequestContext,
  build: string,
  id: string,
  answer: (scope: "session" | "whole", earlier: number) => EnvironmentAnswer,
) {
  const sent: Array<"session" | "whole"> = [];
  const path = `/api/sessions/${encodeURIComponent(id)}`;
  const stop = await drainedRoute(page, (url) => url.pathname === path, async (route) => {
    if (route.request().method() !== "DELETE") return route.fallback();
    const query = new URL(route.request().url()).searchParams;
    const scope = query.get("delete_environment_session") === "true" ? "session"
      : query.get("delete_sandbox") === "true" ? "whole" : null;
    expect(scope, `an environment session's Delete names its scope: ${route.request().url()}`).not.toBeNull();
    const reply = answer(scope!, sent.length);
    sent.push(scope!);
    if (reply.removes) await cleanupSession(request, id);
    const headers = { "x-farhelm-build": build, ...(reply.unasked ? UNASKED : {}) };
    await route.fulfill(reply.json
      ? { status: reply.status, headers, json: reply.json }
      : { status: reply.status, headers, body: reply.body ?? "" });
  });
  return { sent, stop };
}

/** Present `id` as an agent that ended and has no tabs. With "Skip session
 * confirmations" on, an ordinary session in that state is deleted without a
 * question, so a question shown for it can only come from the environment rule. */
async function endedWithoutTabs(page: Page, id: string) {
  return await drainedRoute(page, /\/api\/sessions(\?|$)/, async (route) => {
    if (route.request().method() !== "GET") return route.fallback();
    const reply = await route.fetch();
    if (!reply.ok()) return route.fulfill({ response: reply });
    const json = await reply.json();
    json.sessions = json.sessions.map((current: any) => current.id === id
      ? { ...current, stale: false, status: { state: "exited", exit_code: 0 }, tabs: [] }
      : current);
    await route.fulfill({ response: reply, json });
  });
}

/** The Delete of a session in a connected sandbox must not decide from the
 * page's list whether it is the last, so its question states both outcomes
 * and is never skipped: here it asks even with "Skip session confirmations"
 * on for an ended agent without tabs, which an ordinary session deletes at
 * once. The answer asks the sandbox's Farhelm. When the sandbox stays, the
 * notice quotes the count that Farhelm reported, which the fixture sets above
 * the one other session the page lists, so the notice cannot come from the
 * list; and only the deleted session leaves the list. */
test("a connected sandbox's Delete asks its Farhelm and reports the sandbox that stays", async ({ page, request }, testInfo) => {
  const fixture = await ownedHost(page, request, true, false);
  const previous = await readPreferences(request);
  const owned: string[] = [];
  try {
    const kept = await createSession(request, { title: `sandbox-delete-kept-${Date.now()}`, host: fixture.remote.id });
    owned.push(kept.id);
    const gone = await createSession(request, { title: `sandbox-delete-gone-${Date.now()}`, host: fixture.remote.id });
    owned.push(gone.id);
    fixture.stops.push(await endedWithoutTabs(page, gone.id));
    const deletes = await environmentDeletes(page, request, fixture.build, gone.id,
      () => ({ status: 200, json: { environment_remaining: 2 }, removes: true }));
    fixture.stops.push(deletes.stop);
    await patchPreferences(request, { skip_session_confirmations: true });
    await pinAutoSelect(page, kept.id);
    await page.goto("/");
    const row = page.locator(`.session-row[data-session-id="${gone.id}"]`);
    const keptRow = page.locator(`.session-row[data-session-id="${kept.id}"]`);
    await expect(row).toContainText("Exited");
    await expect(keptRow, "premise: the page lists one other session in the sandbox").toHaveCount(1);
    await openRowMenu(row);
    await expect(row.locator(".session-row-delete")).toContainText("the sandbox goes too if this is its last session");
    await row.locator(".session-row-delete").click();
    const prompt = row.locator(".session-row-menu-panel");
    await expect(prompt).toContainText("Delete this session and its checkout?");
    await expect(prompt).toContainText(
      "If this is the last session in its sandbox, the sandbox goes too, with every process and file in it and its snapshots.");
    await expect(prompt).toContainText("farhelm-sess-e2e-owned");
    expect(deletes.sent, "the question is never skipped").toEqual([]);
    await expect(row.locator(".confirm-delete")).toHaveText("Delete session");
    await row.locator(".confirm-delete").click();
    await expect.poll(() => deletes.sent).toEqual(["session"]);
    await expect(page.locator(".delete-notice .status")).toHaveText("The sandbox stays: 2 other sessions are in it");
    await expect(row).toHaveCount(0);
    await expect(keptRow, "a Delete that kept the sandbox removes only its own session").toHaveCount(1);
    expect(deletes.sent).toEqual(["session"]);
  } finally {
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => [`kept-sandbox route ${index}`, stop]),
      ...owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
      ["preferences", () => patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null })],
    ]);
  }
});

/** The "stays" count is true only until the sandbox goes. When it goes some
 * other way than this page's own Delete (here, standing in for another
 * window's sealing Delete, its last session is deleted behind the page's
 * back and the hosts read stops listing the sandbox), the next successful
 * hosts read must retire the count, leaving the checkout notice that came
 * with it. SPEC.md accepts only a brief discrepancy before the list shows
 * the truth. */
test("a sandbox destroyed elsewhere retires its stays count but keeps the checkout notice", async ({ page, request }, testInfo) => {
  const fixture = await ownedHost(page, request, true, false);
  const owned: string[] = [];
  const checkout = "Farhelm left this session's checkout in place.";
  try {
    const kept = await createSession(request, { title: `sandbox-elsewhere-kept-${Date.now()}`, host: fixture.remote.id });
    owned.push(kept.id);
    const gone = await createSession(request, { title: `sandbox-elsewhere-gone-${Date.now()}`, host: fixture.remote.id });
    owned.push(gone.id);
    const deletes = await environmentDeletes(page, request, fixture.build, gone.id,
      () => ({ status: 200, json: { environment_remaining: 1, notice: checkout }, removes: true }));
    fixture.stops.push(deletes.stop);
    await pinAutoSelect(page, kept.id);
    await page.goto("/");
    const row = page.locator(`.session-row[data-session-id="${gone.id}"]`);
    const keptRow = page.locator(`.session-row[data-session-id="${kept.id}"]`);
    await expect(keptRow).toHaveCount(1);
    await openRowMenu(row);
    await row.locator(".session-row-delete").click();
    await row.locator(".confirm-delete").click();
    const band = page.locator(".delete-notice .status");
    await expect(band).toHaveText(`The sandbox stays: 1 other session is in it. ${checkout}`);
    // The sandbox's last session goes, and with it the sandbox, elsewhere.
    await cleanupSession(request, kept.id);
    fixture.remove();
    fixture.refresh();
    await expect(keptRow).toHaveCount(0);
    await expect(band).toHaveText(checkout);
    expect(deletes.sent).toEqual(["session"]);
  } finally {
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => [`elsewhere route ${index}`, stop]),
      ...owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});

/** SPEC.md: when the helm cannot ask the sandbox's Farhelm, nothing is
 * decided and Delete "offers the whole-sandbox Delete instead", never sends it
 * on its own. From a row: the helm's unasked answer to "Delete session" makes
 * the row offer the warned question, whose answer alone sends the
 * whole-sandbox Delete; that success takes the session off the list. */
test("a row's unasked environment Delete offers the warned whole-sandbox Delete", async ({ page, request }, testInfo) => {
  const fixture = await ownedHost(page, request, true, false);
  let session: { id: string } | undefined;
  try {
    session = await createSession(request, { title: `sandbox-unasked-row-${Date.now()}`, host: fixture.remote.id });
    const deletes = await environmentDeletes(page, request, fixture.build, session.id, (scope) => scope === "session"
      ? { status: 409, unasked: true, body: "Farhelm cannot reach this environment's own Farhelm right now, so nothing was deleted" }
      : { status: 200, json: {}, removes: true });
    fixture.stops.push(deletes.stop);
    await page.goto("/");
    const row = page.locator(`.session-row[data-session-id="${session.id}"]`);
    await openRowMenu(row);
    await row.locator(".session-row-delete").click();
    const prompt = row.locator(".session-row-menu-panel");
    await expect(prompt).toContainText("Delete this session and its checkout?");
    await row.locator(".confirm-delete").click();
    await expect.poll(() => deletes.sent).toEqual(["session"]);
    // The warned question waits on the row: shown in its menu, which opens
    // again here if the answer arrived after it closed.
    await openRowMenu(row);
    await expect(prompt).toContainText("deletes the whole sandbox: every session");
    await expect(prompt).toContainText("including sessions Farhelm does not list");
    await expect(prompt).toContainText("Resuming or updating the sandbox first lets Farhelm delete just this session");
    await expect(row.locator(".confirm-delete")).toHaveText("Delete session and sandbox");
    expect(deletes.sent, "the warned Delete waits for its own answer").toEqual(["session"]);
    await row.locator(".confirm-delete").click();
    await expect.poll(() => deletes.sent).toEqual(["session", "whole"]);
    await expect(row).toHaveCount(0);
  } finally {
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => [`unasked-row route ${index}`, stop]),
      ...(session ? [[session.id, () => cleanupSession(request, session!.id)] as [string, () => Promise<void>]] : []),
    ]);
  }
});

/** The header's Delete is a second surface for the same rule: its unasked
 * answer reopens the header's own question in the warned lane, whose answer
 * sends the whole-sandbox Delete. A refusal of that Delete is shown and sends
 * nothing more. */
test("the header's unasked environment Delete reopens as the warned whole-sandbox Delete", async ({ page, request }, testInfo) => {
  const fixture = await ownedHost(page, request, true, false);
  let session: { id: string } | undefined;
  try {
    session = await createSession(request, { title: `sandbox-unasked-header-${Date.now()}`, host: fixture.remote.id });
    const deletes = await environmentDeletes(page, request, fixture.build, session.id, (scope) => scope === "session"
      ? { status: 409, unasked: true, body: "this environment's own Farhelm did not answer in time" }
      : { status: 409, body: "sandbox deletion fixture refused" });
    fixture.stops.push(deletes.stop);
    await pinAutoSelect(page, session.id);
    await page.goto("/");
    await expect(page.locator(`.session-row[data-session-id="${session.id}"]`)).toHaveAttribute("data-session-selected", "true");
    await expect(page.locator(".header-delete")).toHaveAttribute("data-tooltip", /the sandbox goes too if this is its last session/);
    await page.locator(".header-delete").click();
    const question = page.locator(".header-delete-confirm");
    await expect(question).toContainText("If this is the last session in its sandbox, the sandbox goes too");
    await question.getByRole("button", { name: "Delete session", exact: true }).click();
    await expect.poll(() => deletes.sent).toEqual(["session"]);
    await expect(question).toContainText("deletes the whole sandbox: every session");
    await expect(question).toContainText("including sessions Farhelm does not list");
    expect(deletes.sent, "the warned Delete waits for its own answer").toEqual(["session"]);
    await question.getByRole("button", { name: "Delete session and sandbox", exact: true }).click();
    await expect.poll(() => deletes.sent).toEqual(["session", "whole"]);
    await expect(page.locator(".session-action-error, .session-row-error").filter({ hasText: "sandbox deletion fixture refused" }).first())
      .toBeVisible();
    expect(deletes.sent).toEqual(["session", "whole"]);
  } finally {
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => [`unasked-header route ${index}`, stop]),
      ...(session ? [[session.id, () => cleanupSession(request, session!.id)] as [string, () => Promise<void>]] : []),
    ]);
  }
});

/** SPEC.md: bulk Delete "sends one such request per marked session of a
 * sandbox the helm is connected to"; its Farhelm takes them one at a time and
 * the last one seals. Collapsing them into one whole-sandbox Delete, as the
 * page once did, would destroy sessions nobody marked. The modal marks those
 * rows as asking their sandbox and states both outcomes. The first reply's
 * "stays" count is only true until the sealing Delete, so it must be gone
 * once that one is handled. */
test("bulk Delete of a connected sandbox's sessions sends one environment Delete each", async ({ page, request }, testInfo) => {
  const fixture = await ownedHost(page, request, true, false);
  const owned: string[] = [];
  try {
    for (const name of ["first", "second"]) {
      owned.push((await createSession(request, { title: `sandbox-bulk-${name}-${Date.now()}`, host: fixture.remote.id })).id);
    }
    // Whichever request the pacing sends first keeps the sandbox for the
    // other; the second is the last and answers like a sealed Delete.
    let answered = 0;
    const routes = await Promise.all(owned.map((id) => environmentDeletes(page, request, fixture.build, id,
      () => ++answered === 1
        ? { status: 200, json: { environment_remaining: 1 }, removes: true }
        : { status: 200, json: {}, removes: true })));
    fixture.stops.push(...routes.map((route) => route.stop));
    await page.goto("/");
    for (const id of owned) await page.locator(`.session-row[data-session-id="${id}"] .session-row-open`).click({ modifiers: ["Control"] });
    await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "2");
    await page.locator(".marked-delete").click();
    const dialog = page.getByRole("dialog", { name: "Delete 2 sessions?", exact: true });
    await expect(dialog.locator('.bulk-delete-row[data-delete-scope="session"]')).toHaveCount(2);
    for (const id of owned) {
      await expect(dialog.locator(`[data-delete-target="${id}"] .bulk-delete-consequence`))
        .toContainText("If this is the last session in its sandbox, the sandbox goes too");
    }
    await expect(dialog.locator(".bulk-delete-confirm")).toHaveText("Delete sessions");
    await expect(dialog.locator(".bulk-delete-confirm")).toHaveAttribute("data-tooltip", /an environment goes too with its last session/);
    expect(routes.map((route) => route.sent)).toEqual([[], []]);
    await dialog.locator(".bulk-delete-confirm").click();
    await expect.poll(() => routes.map((route) => route.sent)).toEqual([["session"], ["session"]]);
    // Each row leaves only when its own reply has been handled, so with both
    // gone the first reply's "stays" count has been published and the sealing
    // reply, which has no count, has retired it: the band must not keep
    // claiming that the destroyed sandbox stays.
    for (const id of owned) await expect(page.locator(`.session-row[data-session-id="${id}"]`)).toHaveCount(0);
    await expect(page.locator(".delete-notice")).toHaveCount(0);
  } finally {
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => [`bulk route ${index}`, stop]),
      ...owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});

/** SPEC.md: while a sealed sandbox is being destroyed, "its sessions show as
 * being deleted with their actions unavailable". The helm reports that as the
 * host's `deleting` phase, which the fixture presents; the row says why, and
 * neither the row nor the header offers Delete or Clone, so nothing is sent
 * to a sandbox that is already going.
 *
 * The marked toolbar is a third way to act on the session, and it must hold
 * to the same rule: marked alone, the session leaves bulk Stop and Delete
 * disabled with the reason; marked beside an ordinary session, the batch acts
 * on that one and leaves the being-deleted row with the reason, still
 * sending nothing to the sandbox. Skipped session questions keep the ordinary
 * Delete free of its own dialog, so the batch dispatches as soon as it is
 * admitted, which is where the deleting row has to be left out. */
test("a session of a sandbox being deleted shows why its actions are unavailable", async ({ page, request }, testInfo) => {
  const fixture = await ownedHost(page, request, true, false);
  const previous = await readPreferences(request);
  let session: { id: string } | undefined;
  let other: { id: string } | undefined;
  try {
    session = await createSession(request, { title: `sandbox-being-deleted-${Date.now()}`, host: fixture.remote.id });
    other = await createSession(request, { title: `beside-being-deleted-${Date.now()}`, host: await localHostId(request) });
    await patchPreferences(request, { skip_session_confirmations: true });
    const deletes = await environmentDeletes(page, request, fixture.build, session.id,
      () => ({ status: 409, body: "unexpected Delete of a sandbox being deleted" }));
    fixture.stops.push(deletes.stop);
    // A lifecycle request naming the being-deleted session: any Delete, or
    // a Stop. (Viewing the session may write its seen state; that is not one.)
    const mutations: string[] = [];
    const sessionPath = `/api/sessions/${encodeURIComponent(session.id)}`;
    fixture.stops.push(await drainedRoute(page, (url) => url.pathname.startsWith(sessionPath), async (route) => {
      const url = new URL(route.request().url());
      if (route.request().method() === "DELETE" || url.pathname.includes("/stop")) {
        mutations.push(`${route.request().method()} ${url.pathname}${url.search}`);
      }
      await route.fallback();
    }));
    fixture.present({ phase: "deleting", failure: null });
    await pinAutoSelect(page, session.id);
    await page.goto("/");
    const row = page.locator(`.session-row[data-session-id="${session.id}"]`);
    const badge = row.locator(".status-badge.environment-deleting");
    const reason = "This session's sandbox is being deleted, so its actions are unavailable";
    await expect(badge).toHaveText("Deleting…");
    await expect(badge).toHaveAttribute("data-tooltip", reason);
    await expect(page.locator(".header-delete")).toBeDisabled();
    await expect(page.locator(".header-clone")).toBeDisabled();
    await expect(page.locator(".header-stop")).toBeDisabled();
    await openRowMenu(row);
    await expect(row.locator(".session-row-delete")).toHaveAttribute("aria-disabled", "true");
    await expect(row.locator(".session-row-delete")).toContainText("is being deleted, so its actions are unavailable");
    await expect(row.locator(".session-row-clone")).toHaveAttribute("aria-disabled", "true");
    await row.locator(".session-row-delete").dispatchEvent("click");
    await page.locator(".header-delete").dispatchEvent("click");
    await expect(row.locator(".confirm-delete")).toHaveCount(0);
    await expect(page.locator(".header-delete-confirm")).toHaveCount(0);
    await page.keyboard.press("Escape");
    await expect(row.locator(".session-row-menu-flyout")).toHaveCount(0);

    // Marked alone: no bulk lifecycle control is available, and each says why.
    await row.locator(".session-row-open").click({ modifiers: ["Control"] });
    await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "1");
    for (const selector of [".marked-stop", ".marked-delete"]) {
      await expect(page.locator(selector)).toBeDisabled();
      await expect(page.locator(selector)).toHaveAttribute("data-tooltip", /environments are being deleted/);
      await page.locator(selector).dispatchEvent("click");
    }
    await expect(page.locator(".bulk-delete-dialog")).toHaveCount(0);

    // Beside an ordinary session: the batch deletes that one and leaves the
    // being-deleted row with the reason.
    const otherRow = page.locator(`.session-row[data-session-id="${other.id}"]`);
    await otherRow.locator(".session-row-open").click({ modifiers: ["Control"] });
    await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "2");
    await expect(page.locator(".marked-delete")).toBeEnabled();
    await page.locator(".marked-delete").click();
    await expect(otherRow).toHaveCount(0);
    await expect(row.locator(".action-error")).toHaveText(`Delete: ${reason}`);
    await expect(row).toHaveCount(1);
    expect(deletes.sent).toEqual([]);
    expect(mutations).toEqual([]);
  } finally {
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => [`being-deleted route ${index}`, stop]),
      ["preferences", () => patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null })],
      ...(other ? [[other.id, () => cleanupSession(request, other!.id)] as [string, () => Promise<void>]] : []),
      ...(session ? [[session.id, () => cleanupSession(request, session!.id)] as [string, () => Promise<void>]] : []),
    ]);
  }
});

/** SPEC.md: Suspend and Resume are not host registry changes. A sandbox
 * session's Suspend that is still waiting for its reply must not hold up a
 * launch on another host, and a launch on another host must not disable that
 * session's Resume. Before the host-scoped rule both waited on one page-wide
 * token, so New stayed disabled and the create never left the page; the held
 * Suspend and the held create are what make that difference observable. */
test("a held sandbox Suspend and a launch on another host proceed side by side", async ({ page, request }, testInfo) => {
  const fixture = await ownedHost(page, request, true, false);
  const suspendGate = routeGate();
  const createGate = routeGate();
  const owned: string[] = [];
  const bodies: any[] = [];
  let suspends = 0;
  try {
    const local = await localHostId(request);
    expect(local, "the launch must target a host other than the sandbox").not.toBe(fixture.remote.id);
    const sandboxed = await createSession(request, { title: `overlap-sandbox-${Date.now()}`, host: fixture.remote.id });
    owned.push(sandboxed.id);
    const other = await createSession(request, { title: `overlap-local-${Date.now()}`, host: local });
    owned.push(other.id);
    // Nothing on the sandbox is working, so Suspend sends without its question.
    fixture.stops.push(await drainedRoute(page, /\/api\/sessions(\?|$)/, async (route) => {
      if (route.request().method() !== "GET"
        || new URL(route.request().url()).searchParams.get("host") !== String(fixture.remote.id)) return route.fallback();
      const reply = await route.fetch();
      const json = await reply.json();
      json.sessions = json.sessions.map((current: any) => ({ ...current, stale: false, status: { state: "exited", exit_code: 0 } }));
      await route.fulfill({ response: reply, json });
    }));
    // Holds admission only; ownedHost's own route then records the suspension.
    fixture.stops.push(await drainedRoute(page, `**/api/hosts/${fixture.remote.id}/suspend`, async (route) => {
      suspends++;
      await suspendGate.wait();
      await route.fallback();
    }));
    fixture.stops.push(await drainedRoute(page, "**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.fallback();
      bodies.push(route.request().postDataJSON());
      await createGate.wait();
      await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.build }, body: "overlap create refused" });
    }));
    await pinAutoSelect(page, other.id);
    await page.goto("/");
    await expect(page.locator(`[data-session-id="${other.id}"]`)).toHaveAttribute("data-session-selected", "true");
    const sandboxRow = page.locator(`.session-row[data-session-id="${sandboxed.id}"]`);
    await openRowMenu(sandboxRow);
    await page.getByRole("menuitem", { name: "Suspend", exact: true }).click();
    await expect.poll(() => suspends, { message: "the Suspend request is held before its reply" }).toBe(1);
    expect(fixture.calls).toEqual([]);

    await expect(page.locator(".new-session-button")).toBeEnabled();
    const form = await fillCreateForm(page, { cwd: "/tmp", invocation: "true", title: "overlap draft" });
    await form.locator(".create-session-submit").click();
    await expect.poll(() => bodies.length, { message: "the launch on another host is sent while Suspend waits" }).toBe(1);
    expect(bodies[0].host).toBe(local);

    suspendGate.release();
    await expect.poll(() => fixture.calls).toEqual(["suspend"]);
    await expect(sandboxRow).toHaveClass(/\bstale\b/);
    expect(bodies, "premise: the other host's launch is still pending").toHaveLength(1);
    await openRowMenu(sandboxRow);
    await page.getByRole("menuitem", { name: "Resume", exact: true }).click();
    await expect.poll(() => fixture.calls, { message: "Resume is not refused for another host's launch" })
      .toEqual(["suspend", "resume"]);

    createGate.release();
    await expect(form.getByRole("alert")).toContainText("overlap create refused");
    expect(suspends).toBe(1);
  } finally {
    suspendGate.release();
    createGate.release();
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => [`overlap route ${index}`, stop]),
      ...owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});

/** The host-scoped rule's refusal: while this page's launch wakes a host, Hosts
 * offers no Resume or Suspend for that host, and Settings (a registry change)
 * still waits for the launch as it always did. Once the launch settles, the
 * host's lifecycle action is available again. */
test("Hosts refuses a host's Resume and Settings while a launch is waking that host", async ({ page, request }, testInfo) => {
  const fixture = await ownedHost(page, request, false);
  const wake = routeGate();
  const owned: string[] = [];
  let resumes = 0;
  let creates = 0;
  try {
    const source = await createSession(request, { title: `waking-source-${Date.now()}`, host: fixture.remote.id });
    owned.push(source.id);
    fixture.stops.push(await drainedRoute(page, `**/api/hosts/${fixture.remote.id}/resume`, async route => {
      resumes++;
      await wake.wait();
      await route.fallback();
    }));
    fixture.stops.push(await drainedRoute(page, "**/api/sessions", async route => {
      if (route.request().method() !== "POST") return route.fallback();
      creates++;
      await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.build }, body: "woken-host create refused" });
    }));
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    await expect(page.locator(`[data-session-id="${source.id}"]`)).toHaveAttribute("data-session-selected", "true");
    const form = await fillCreateForm(page, { cwd: "/tmp", invocation: "true", title: "waking draft" });
    await expect(form.getByRole("combobox", { name: "Host", exact: true })).toContainText("Owned sandbox");
    await form.locator(".create-session-submit").click();
    await expect.poll(() => resumes, { message: "the launch's wake is held" }).toBe(1);
    await expect(form).toBeHidden();

    await openHostsPanel(page);
    const row = page.locator(`[data-host-id="${fixture.remote.id}"]`).filter({ has: page.locator(".host-row-main") }).first();
    await expect(row).toContainText("Suspended");
    await expect(row.getByRole("button", { name: "Resume", exact: true })).toBeDisabled();
    await openHostMenu(row);
    await expect(row.getByRole("menuitem", { name: "Resume", exact: true })).toBeDisabled();
    await expect(row.getByRole("menuitem", { name: /^Settings/ })).toHaveAttribute("aria-disabled", "true");
    await page.keyboard.press("Escape");
    expect(fixture.calls, "Hosts sent nothing of its own").toEqual([]);

    wake.release();
    await expect.poll(() => fixture.calls).toEqual(["resume"]);
    await expect(form.getByRole("alert")).toContainText("woken-host create refused");
    expect(creates).toBe(1);
    await form.locator(".launch-composer-cancel").click();
    await expect(form).toHaveCount(0);
    await expect(row).toContainText("Connected");
    await openHostMenu(row);
    await expect(row.getByRole("menuitem", { name: "Suspend", exact: true })).toBeEnabled();
  } finally {
    wake.release();
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => [`waking route ${index}`, stop]),
      ...owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});
