/** Mounted lifecycle surfaces use the real connected remote host, with only
 * its displayed Tensorlake state and lifecycle replies controlled here. Rust
 * route tests and the live run cover vendor calls and connection teardown. */
import { expect, test } from "./helpers/evidence";
import type { APIRequestContext, Page } from "@playwright/test";
import { cleanupSession, createSession, openHostsPanel, pinAutoSelect, stubFeed } from "./helpers/fleet";
import { chooseLauncherHost } from "./helpers/host-picker";
import { fillLauncherFolder } from "./helpers/launcher";
import { routeGate } from "./helpers/route-gate";
import { drainedRoute } from "./helpers/drained-route";

/** Preserve real install identity and the build stamp. Resume changes the
 * displayed connection token, so a launch carrying the old token fails. */
async function fleet(page: Page, request: APIRequestContext, initiallySuspended = true, runningState?: Record<string, unknown>) {
  const response = await request.get("/api/hosts");
  expect(response.ok()).toBe(true);
  const buildStamp = response.headers()["x-farhelm-build"];
  expect(buildStamp, "fixture has the serving helm's build stamp").toBeTruthy();
  const remote = (await response.json()).hosts.find((h: any) => h.kind === "ssh" && h.state.phase === "connected");
  expect(remote, "owned stack has a connected remote supervisor").toBeTruthy();
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  let revision = 1;
  let suspended = initiallySuspended;
  const calls: string[] = [];
  await page.route("**/api/hosts", async route => {
    const response = await route.fetch();
    const json = await response.json();
    json.hosts = json.hosts.map((h: any) => h.id === remote.id ? {
      ...h, name: "Sandbox test", kind: "tensorlake",
      incarnation: suspended ? remote.incarnation + 10000 : h.incarnation,
      state: suspended ? { phase: "suspended", observed_at: Date.now(), suspended_by_farhelm_at: null, resume_failed: null } : runningState ?? h.state,
      tensorlake: { sandbox: "farhelm-host-test", recipe: "test", cpus: 2, memory_gb: 4, disk_gb: 20 },
    } : h);
    await route.fulfill({ response, json });
  });
  for (const verb of ["suspend", "resume"]) {
    await page.route(`**/api/hosts/${remote.id}/${verb}`, async route => {
      calls.push(verb);
      suspended = verb === "suspend";
      await route.fulfill({ status: 200, headers: { "x-farhelm-build": response.headers()["x-farhelm-build"] }, json: {} });
      await feed.waitForConnection(1);
      feed.notify(++revision);
    });
  }
  return { remote, calls, buildStamp, suspended: () => suspended,
    /** Publish a detected host-state change without resolving a held session
     * read, so a test can distinguish click-time from post-read evidence. */
    async setRunningState(state: Record<string, unknown>) {
      runningState = state;
      await feed.waitForConnection(1);
      feed.notify(++revision);
    },
  };
}

test.afterEach(async ({ page }) => { await page.unrouteAll({ behavior: "wait" }); });

/** An inline Resume and the menu both route through the mutation owner.
 * With no working sessions Suspend acts without a confirmation dialog. */
test("a suspended host has inline Resume and can suspend from its menu", async ({ page, request }) => {
  const fixture = await fleet(page, request);
  await page.goto("/");
  await openHostsPanel(page);
  const row = page.locator(`[data-host-id="${fixture.remote.id}"]`).filter({ has: page.locator(".host-row-main") }).first();
  await expect(row).toContainText("Suspended");
  await row.getByRole("button", { name: "Resume", exact: true }).click();
  await expect.poll(() => fixture.calls).toEqual(["resume"]);
  await expect(row).toContainText("Connected");
  await row.locator(".host-row-menu").click();
  await row.getByRole("menuitem", { name: "Suspend", exact: true }).click();
  await expect.poll(() => fixture.calls).toEqual(["resume", "suspend"]);
  await expect(page.getByRole("dialog", { name: "Suspend host", exact: true })).toHaveCount(0);
  await expect(row).toContainText("Suspended");
});

/** A retained session must explain suspension and expose a usable Resume
 * action above its offline terminal rather than only a generic outage. */
test("a suspended session banner resumes its host", async ({ page, request }) => {
  const fixture = await fleet(page, request);
  const session = await createSession(request, { title: "suspended-banner", host: fixture.remote.id });
  try {
    await pinAutoSelect(page, session.id);
    await page.route("**/api/sessions**", async route => {
      if (route.request().method() !== "GET") return route.continue();
      const response = await route.fetch();
      if (!response.ok()) return route.fulfill({ response });
      const json = await response.json();
      if (Array.isArray(json.sessions)) json.sessions = json.sessions.map((s: any) => s.id === session.id ? { ...s, stale: fixture.suspended() } : s);
      else if (json.id === session.id) json.stale = fixture.suspended();
      await route.fulfill({ response, json });
    });
    await page.goto("/");
    const banner = page.getByRole("region", { name: "Session unavailable", exact: true });
    await expect(banner).toContainText("is suspended; this session continues when it resumes");
    await banner.getByRole("button", { name: "Resume host", exact: true }).click();
    await expect.poll(() => fixture.calls).toEqual(["resume"]);
  } finally { await cleanupSession(request, session.id); }
});

/** Working or uncertain activity needs consent. Unknown is a legitimate
 * create-time state, and cached Idle does not establish current inactivity.
 * Cancel sends nothing; accepting sends exactly one lifecycle request. */
for (const activity of [
  { name: "working", state: "running", stale: false },
  { name: "unknown", state: "unknown", stale: false },
  { name: "stale idle", state: "idle", stale: true },
]) test(`suspending an agent with ${activity.name} status asks before sending the request`, async ({ page, request }) => {
  const fixture = await fleet(page, request, false);
  const session = await createSession(request, { title: "working-suspend", host: fixture.remote.id });
  try {
    let observed = false;
    await page.route("**/api/sessions?**", async route => {
      const response = await route.fetch();
      const json = await response.json();
      expect(json.sessions.some((s: any) => s.id === session.id), "owned agent is present in the activity read").toBe(true);
      observed = true;
      json.sessions = json.sessions.map((s: any) => s.id === session.id ? { ...s, stale: activity.stale, status: { state: activity.state } } : s);
      await route.fulfill({ response, json });
    });
    await page.goto("/");
    await openHostsPanel(page);
    const row = page.locator(`.host-row[data-host-id="${fixture.remote.id}"]`);
    await expect(row).toContainText("Connected");
    await row.locator(".host-row-menu").click();
    await row.getByRole("menuitem", { name: "Suspend", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Suspend host", exact: true });
    await expect(dialog).toContainText("continues when you resume the host");
    expect(observed, "confirmation consumed the controlled session activity").toBe(true);
    expect(fixture.calls).toEqual([]);
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    expect(fixture.calls).toEqual([]);
    await row.locator(".host-row-menu").click();
    await row.getByRole("menuitem", { name: "Suspend", exact: true }).click();
    await dialog.getByRole("button", { name: "Suspend", exact: true }).click();
    await expect.poll(() => fixture.calls).toEqual(["suspend"]);
  } finally { await cleanupSession(request, session.id); }
});

/** Definite current Idle/Waiting evidence allows immediate Suspend even
 * with an agent session present; merely testing an empty host misses this. */
for (const state of ["idle", "waiting"]) test(`suspending a current ${state} agent acts immediately`, async ({ page, request }) => {
  const fixture = await fleet(page, request, false);
  const session = await createSession(request, { title: `inactive-${state}`, host: fixture.remote.id });
  try {
    let observed = false;
    await page.route("**/api/sessions?**", async route => {
      const response = await route.fetch();
      const json = await response.json();
      expect(json.sessions.some((s: any) => s.id === session.id)).toBe(true);
      observed = true;
      json.sessions = json.sessions.map((s: any) => s.id === session.id ? { ...s, stale: false, status: { state } } : s);
      await route.fulfill({ response, json });
    });
    await page.goto("/");
    await openHostsPanel(page);
    const row = page.locator(`.host-row[data-host-id="${fixture.remote.id}"]`);
    await expect(row).toContainText("Connected");
    await row.locator(".host-row-menu").click();
    await row.getByRole("menuitem", { name: "Suspend", exact: true }).click();
    await expect.poll(() => fixture.calls).toEqual(["suspend"]);
    expect(observed).toBe(true);
    await expect(page.getByRole("dialog", { name: "Suspend host", exact: true })).toHaveCount(0);
  } finally { await cleanupSession(request, session.id); }
});

/** Supervisor reachability says nothing about whether the sandbox bills.
 * An unreachable running sandbox must still offer Suspend, with uncertainty
 * confirmation when session activity cannot be read. */
test("an unreachable supervisor still offers Suspend for its running sandbox", async ({ page, request }) => {
  const fixture = await fleet(page, request, false, {
    phase: "unreachable-reprobing", cause: "unknown", last_error: "Supervisor unavailable",
  });
  let guardRead = false;
  await page.route("**/api/sessions?**", async route => {
    if (new URL(route.request().url()).searchParams.get("host") !== String(fixture.remote.id)) return route.continue();
    // Preserve the real build stamp: an unavailable supervisor is not a
    // different helm build. The fixture already read that stamp; fetching
    // a body this refusal discards leaves a late fulfill racing retirement
    // of the application's request. Answer this owned stub immediately.
    guardRead = true;
    await route.fulfill({ headers: { "x-farhelm-build": fixture.buildStamp }, status: 503, json: { error: "Supervisor unavailable" } });
  });
  await page.goto("/");
  await openHostsPanel(page);
  const row = page.locator(`.host-row[data-host-id="${fixture.remote.id}"]`);
  await expect(row).toContainText("Unreachable");
  await row.locator(".host-row-menu").click();
  await row.getByRole("menuitem", { name: "Suspend", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Suspend host", exact: true });
  await expect(dialog).toBeVisible();
  expect(guardRead, "Suspend checked this host's unavailable session listing").toBe(true);
  expect(fixture.calls).toEqual([]);
  await dialog.getByRole("button", { name: "Suspend", exact: true }).click();
  await expect.poll(() => fixture.calls).toEqual(["suspend"]);
  await expect(row).toContainText("Suspended");
});

/** An HTTP 200 empty cache cannot establish inactivity while the supervisor is offline.
 * Check both an already-unreachable host and a disconnect detected during a
 * held read: the click's Connected snapshot must not bypass confirmation.
 * Cancel sends nothing, and acceptance sends exactly one mutation. */
for (const duringRead of [false, true]) test(`an empty cached listing asks when the host disconnects ${duringRead ? "during the read" : "before the click"}`, async ({ page, request }) => {
  const unreachable = { phase: "unreachable-reprobing", cause: "unknown", last_error: "Supervisor unavailable" };
  const fixture = await fleet(page, request, false, duringRead ? undefined : unreachable);
  let reads = 0;
  let release!: () => void;
  const held = new Promise<void>(resolve => { release = resolve; });
  await page.route("**/api/sessions?**", async route => {
    if (new URL(route.request().url()).searchParams.get("host") !== String(fixture.remote.id)) return route.continue();
    const response = await route.fetch();
    expect(response.ok(), "the activity read succeeds despite the offline supervisor").toBe(true);
    const json = await response.json();
    json.sessions = [];
    json.truncated = false;
    json.total = 0;
    json.matching = 0;
    reads++;
    if (duringRead && reads === 1) await held;
    await route.fulfill({ response, json });
  });
  try {
    await page.goto("/");
    await openHostsPanel(page);
    const row = page.locator(`.host-row[data-host-id="${fixture.remote.id}"]`);
    await expect(row).toContainText(duringRead ? "Connected" : "Unreachable");
    await row.locator(".host-row-menu").click();
    await row.getByRole("menuitem", { name: "Suspend", exact: true }).click();
    await expect.poll(() => reads).toBe(1);
    if (duringRead) {
      await fixture.setRunningState(unreachable);
      await expect(row).toContainText("Unreachable");
      release();
    }
    const dialog = page.getByRole("dialog", { name: "Suspend host", exact: true });
    await expect(dialog).toBeVisible();
    expect(fixture.calls).toEqual([]);
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    expect(fixture.calls).toEqual([]);
    await row.locator(".host-row-menu").click();
    await row.getByRole("menuitem", { name: "Suspend", exact: true }).click();
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: "Suspend", exact: true }).click();
    await expect.poll(() => fixture.calls).toEqual(["suspend"]);
    await expect(row).toContainText("Suspended");
  } finally { release(); }
});

/** Existing-folder launch must resume once, then submit against the newly
 * read connection. A real create proves the fresh token is accepted. */
test("launching in an existing folder resumes before creating", async ({ page, request }) => {
  const fixture = await fleet(page, request);
  let created: string | undefined;
  try {
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await form.getByRole("tab", { name: "Command", exact: true }).click();
    await form.getByLabel("Agent command", { exact: true }).fill("sleep 300");
    await form.getByRole("group", { name: "Runs in YOLO mode" }).getByLabel("No", { exact: true }).check();
    await chooseLauncherHost(form.getByRole("combobox", { name: "Host", exact: true }), String(fixture.remote.id));
    await expect(form.getByRole("combobox", { name: "Host", exact: true })).toContainText("Suspended");
    await fillLauncherFolder(form, "/tmp");
    const accepted = page.waitForResponse(r => new URL(r.url()).pathname === "/api/sessions" && r.request().method() === "POST");
    await form.locator(".create-session-submit").click();
    const response = await accepted;
    expect(response.ok(), await response.text()).toBe(true);
    expect(response.request().postDataJSON().expected_incarnation).toBe(fixture.remote.incarnation);
    created = (await response.json()).id;
    expect(fixture.calls).toEqual(["resume"]);
  } finally { if (created) await cleanupSession(request, created); }
});

/** A failed Resume finishes preparation without dispatching a create. Its
 * error must use the launch-attempt refusal, never the unknown-outcome promise
 * reserved for a session mutation that might have reached the supervisor. */
test("a failed resume refuses an existing-folder launch without creating", async ({ page, request }) => {
  const fixture = await fleet(page, request);
  let resumes = 0;
  let creates = 0;
  await page.route(`**/api/hosts/${fixture.remote.id}/resume`, async route => {
    resumes++;
    await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.buildStamp }, json: { error: "fixture resume refused" } });
  });
  await page.route("**/api/sessions", async route => {
    if (route.request().method() !== "POST") return route.continue();
    creates++;
    await route.abort();
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await form.getByRole("tab", { name: "Command", exact: true }).click();
  await form.getByLabel("Agent command", { exact: true }).fill("sleep 300");
  await form.getByRole("group", { name: "Runs in YOLO mode" }).getByLabel("No", { exact: true }).check();
  await chooseLauncherHost(form.getByRole("combobox", { name: "Host", exact: true }), String(fixture.remote.id));
  await expect(form.getByRole("combobox", { name: "Host", exact: true })).toContainText("Suspended");
  await fillLauncherFolder(form, "/tmp");
  await form.locator(".create-session-submit").click();
  const alert = form.getByRole("alert");
  await expect(alert).toContainText("Not launched.");
  await expect(alert).toContainText("fixture resume refused");
  await expect(alert).not.toContainText("The session may still start");
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  await expect(form.locator(".launch-progress")).toHaveCount(0);
  expect(resumes).toBe(1);
  expect(fixture.suspended()).toBe(true);
  expect(creates, "the finished preparation refusal sent no session mutation").toBe(0);
});

/** Resume replaces the suspended connection before create starts. Hold that
 * create at dispatch and advance the shipped progress timer: the request's
 * captured host name must not retain the selector's obsolete phase suffix. */
test("a delayed create after resume shows the host name without Suspended", async ({ page, request }) => {
  await page.clock.install();
  const fixture = await fleet(page, request);
  const gate = routeGate();
  let creates = 0;
  const stop = await drainedRoute(page, "**/api/sessions", async route => {
    if (route.request().method() !== "POST") return route.continue();
    const input = route.request().postDataJSON();
    expect(input.host).toBe(fixture.remote.id);
    expect(input.expected_incarnation).toBe(fixture.remote.incarnation);
    expect(fixture.calls).toEqual(["resume"]);
    expect(fixture.suspended()).toBe(false);
    creates++;
    await gate.wait();
    await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.buildStamp }, json: { error: "fixture create refused" } });
  });
  try {
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await form.getByRole("tab", { name: "Command", exact: true }).click();
    await form.getByLabel("Agent command", { exact: true }).fill("sleep 300");
    await form.getByRole("group", { name: "Runs in YOLO mode" }).getByLabel("No", { exact: true }).check();
    await chooseLauncherHost(form.getByRole("combobox", { name: "Host", exact: true }), String(fixture.remote.id));
    await expect(form.getByRole("combobox", { name: "Host", exact: true })).toContainText("Suspended");
    await fillLauncherFolder(form, "/tmp");
    await expect(form).toHaveAttribute("data-folder-check-pending", "false");
    await page.clock.pauseAt(new Date(Date.now() + 5_000));
    await form.locator(".create-session-submit").click();
    await expect.poll(() => creates, { message: "Resume finished and the create reply is held" }).toBe(1);
    const status = form.locator(".launch-progress");
    await page.clock.runFor(1_900);
    await expect(status).toHaveCount(0);
    await page.clock.runFor(300);
    await expect(status).toContainText("Launching on Sandbox test");
    await expect(status).toContainText("2 s");
    await expect(status).not.toContainText("Suspended");
    gate.release();
    await expect(form.getByRole("alert")).toContainText("Not launched.");
    await expect(status).toHaveCount(0);
    expect(creates).toBe(1);
  } finally {
    gate.release();
    await stop();
  }
});

/** A fresh checkout cannot reuse a preview from before suspension. The first
 * press resumes only, and a second press requires the new preview. */
test("a fresh checkout resumes first and requests a new preview", async ({ page, request }) => {
  const fixture = await fleet(page, request);
  const history = await request.get(`/api/launch-history?host=${fixture.remote.id}`);
  expect(history.ok()).toBe(true);
  const configRevision = (await history.json()).checkout_config_revision;
  let creates = 0;
  const previewConnections: number[] = [];
  await page.route("**/api/github-repositories", async route => {
    const input = route.request().postDataJSON();
    await route.fulfill({ status: 200, headers: { "x-farhelm-build": (await request.get("/api/hosts")).headers()["x-farhelm-build"] }, json: {
      host: String(input.host), incarnation: input.expected_incarnation, installation_identity: fixture.remote.identity,
      repos: [], truncated: true, scan_error: "fixture scan unavailable",
    } });
  });
  await page.route("**/api/github-checkout-preview", async route => {
    const response = await request.get("/api/hosts");
    const input = route.request().postDataJSON();
    previewConnections.push(input.expected_incarnation);
    await route.fulfill({ status: 200, headers: { "x-farhelm-build": response.headers()["x-farhelm-build"] }, json: {
      canonical_root: "/tmp", basename: "bar-1", cwd: "/tmp/bar-1", config_revision: configRevision,
      host: String(input.host), incarnation: input.expected_incarnation, installation_identity: fixture.remote.identity,
    } });
  });
  await page.route("**/api/sessions", async route => {
    if (route.request().method() === "POST") { creates++; await route.abort(); }
    else await route.continue();
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  await chooseLauncherHost(form.getByRole("combobox", { name: "Host", exact: true }), String(fixture.remote.id));
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await search.fill("gh:acme/bar");
  await expect(form.getByRole("option", { name: "Managed checkout: acme/bar", exact: true })).toBeVisible();
  await search.press("Enter");
  await form.locator(".create-session-submit").click();
  await expect.poll(() => fixture.calls).toEqual(["resume"]);
  await expect(form).toContainText("Review the refreshed checkout preview");
  await expect.poll(() => previewConnections).toContain(fixture.remote.incarnation);
  expect(creates).toBe(0);
});
