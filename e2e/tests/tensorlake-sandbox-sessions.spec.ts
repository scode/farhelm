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
 * `refresh` announces a new revision so the page reads its listings again. */
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
  const stops: Array<() => Promise<void>> = [];
  const calls: string[] = [];
  stops.push(await drainedRoute(page, "**/api/hosts", async (route) => {
    const reply = await route.fetch();
    const json = await reply.json();
    json.hosts = json.hosts.map((host: any) => host.id === remote.id ? {
      ...host,
      name: "Owned sandbox",
      kind: "tensorlake",
      is_session_owned: sessionOwned,
      state: suspended
        ? { phase: "suspended", observed_at: Date.now(), suspended_by_farhelm_at: null, resume_failed: null }
        : host.state,
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
    json.sessions = json.sessions.map((session: any) => session.host === remote.id
      ? { ...session, stale: suspended || session.stale }
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
  return { remote, build, calls, stops, refresh: () => feed.notify(++revision) };
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
 * Delete must still ask with the global skip on,
 * and then explicitly authorize whole-resource destruction without Resume. */
test("owned suspended sessions keep Hosts visibility and mandatory whole-sandbox Delete", async ({ page, request }, testInfo) => {
  const fixture = await ownedHost(page, request);
  const previous = await readPreferences(request);
  const session = await createSession(request, { title: "owned-suspended", host: fixture.remote.id });
  let deletes = 0;
  fixture.stops.push(await drainedRoute(page, `**/api/sessions/${encodeURIComponent(session.id)}?delete_sandbox=true`, async (route) => {
    expect(route.request().method()).toBe("DELETE");
    deletes++;
    await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.build }, body: "sandbox deletion fixture refused" });
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
    await page.locator(".header-delete").click();
    const question = page.locator(".header-delete-confirm");
    await expect(question).toContainText("Every session");
    await expect(question).toContainText("snapshot");
    await expect(question).toContainText("farhelm-sess-e2e-owned");
    expect(deletes).toBe(0);
    await question.getByRole("button", { name: "Delete session and sandbox", exact: true }).click();
    await expect.poll(() => deletes).toBe(1);
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
