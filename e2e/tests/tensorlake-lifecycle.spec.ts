/** Mounted lifecycle surfaces use the real connected remote host, with only
 * its displayed Tensorlake state and lifecycle replies controlled here. Rust
 * lifecycle tests cover vendor calls and connection teardown against the helm's
 * controlled sandbox service; these cases never dial Tensorlake's gateway. */
import { expect, test } from "./helpers/evidence";
import type { APIRequestContext, Page } from "@playwright/test";
import {
  cleanupSession,
  createSession,
  openHostsPanel,
  patchPreferences,
  pinAutoSelect,
  readPreferences,
  stubFeed,
} from "./helpers/fleet";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { chooseLauncherHost } from "./helpers/host-picker";
import { fillLauncherFolder } from "./helpers/launcher";
import { routeGate } from "./helpers/route-gate";
import { drainedRoute } from "./helpers/drained-route";

/** Fleet rewrites can still be reading when the page cancels a refresh.
 * Retain every installed stop, even if later fixture setup fails, so teardown
 * drains the owned callbacks before Playwright retires its route handlers. */
const fleetRouteStops = new WeakMap<Page, Array<() => Promise<void>>>();

/** Preserve real install identity and the build stamp. Resume changes the
 * displayed connection token, so a launch carrying the old token fails. */
async function fleet(
  page: Page,
  request: APIRequestContext,
  initiallySuspended = true,
  runningState?: Record<string, unknown>,
) {
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
  let publishedState: Record<string, unknown> | undefined;
  let nextRead: { gate: ReturnType<typeof routeGate>; entered: () => void } | undefined;
  const calls: string[] = [];
  const stops: Array<() => Promise<void>> = [];
  fleetRouteStops.set(page, stops);
  stops.push(
    await drainedRoute(page, "**/api/hosts", async (route) => {
      const response = await route.fetch();
      const json = await response.json();
      json.hosts = json.hosts.map((h: any) =>
        h.id === remote.id
          ? {
            ...h,
            name: "Sandbox test",
            kind: "tensorlake",
            incarnation: suspended ? remote.incarnation + 10000 : h.incarnation,
            state: publishedState ?? (suspended
              ? { phase: "suspended", observed_at: Date.now(), suspended_by_farhelm_at: null, resume_failed: null }
              : runningState ?? h.state),
            tensorlake: { sandbox: "farhelm-host-test", recipe: "test", cpus: 2, memory_gb: 4, disk_gb: 20 },
          }
          : h
      );
      const held = nextRead;
      nextRead = undefined;
      if (held) {
        held.entered();
        await held.gate.wait();
      }
      await route.fulfill({ response, json });
    }),
  );
  for (const verb of ["suspend", "resume"]) {
    stops.push(
      await drainedRoute(page, `**/api/hosts/${remote.id}/${verb}`, async (route) => {
        calls.push(verb);
        suspended = verb === "suspend";
        await route.fulfill({
          status: 200,
          headers: { "x-farhelm-build": response.headers()["x-farhelm-build"] },
          json: {},
        });
        await feed.waitForConnection(1);
        feed.notify(++revision);
      }),
    );
  }
  return {
    remote,
    calls,
    buildStamp,
    suspended: () => suspended,
    /** Keep the next displayed host read stale while the resume helper makes
     * a separate fresh GET. Arm this before its POST returns, when only the
     * page's ordinary reader can have a GET in flight. */
    holdNextRead() {
      expect(nextRead, "the fixture holds only one host read at a time").toBeUndefined();
      const gate = routeGate();
      let entered!: () => void;
      const observed = new Promise<void>((resolve) => {
        entered = resolve;
      });
      nextRead = { gate, entered };
      // Teardown also wakes a POST waiting for a read that never arrived.
      return {
        entered: observed,
        release: () => {
          gate.release();
          entered();
        },
      };
    },
    /** Publish one lifecycle boundary while its POST or reconnect is held.
     * Only displayed state changes: the owned remote remains the real launch
     * target, and Connected restores its real connection token. */
    async publishState(state?: Record<string, unknown>) {
      publishedState = state;
      suspended = state?.phase === "suspended";
      await feed.waitForConnection(1);
      feed.notify(++revision);
    },
    /** Publish a detected host-state change without resolving a held session
     * read, so a test can distinguish click-time from post-read evidence. */
    async setRunningState(state: Record<string, unknown>) {
      runningState = state;
      await feed.waitForConnection(1);
      feed.notify(++revision);
    },
  };
}

test.afterEach(async ({ page }, testInfo) => {
  const stops = fleetRouteStops.get(page) ?? [];
  fleetRouteStops.delete(page);
  await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
    ...stops.map((stop, index): [string, () => Promise<void>] => [`fleet route ${index}`, stop]),
    ["remaining routes", () => page.unrouteAll({ behavior: "wait" })],
  ]);
});

/** A lifecycle request being accepted is not confirmation of its target.
 * Hold the actual POST and inspect the published intermediate state before
 * releasing it; a fast fixture response would hide the premature label. */
for (const verb of ["suspend", "resume"] as const) {
  test(`a pending ${verb} names its transition rather than a confirmed suspension`, async ({ page, request }) => {
    const fixture = await fleet(page, request, verb === "resume");
    const gate = routeGate();
    let sent = 0;
    const stop = await drainedRoute(page, `**/api/hosts/${fixture.remote.id}/${verb}`, async (route) => {
      expect(route.request().method()).toBe("POST");
      sent++;
      await fixture.publishState({
        phase: "tensorlake-changing",
        direction: verb === "resume" ? "resuming" : "suspending",
        requested_at: Date.now() - 4_000,
      });
      await gate.wait();
      await fixture.publishState(
        verb === "suspend"
          ? {
            phase: "suspended",
            observed_at: Date.now(),
            suspended_by_farhelm_at: Date.now(),
            resume_failed: null,
          }
          : undefined,
      );
      await route.fulfill({ status: 200, headers: { "x-farhelm-build": fixture.buildStamp }, json: {} });
    });
    try {
      await page.goto("/");
      await openHostsPanel(page);
      const row = page.locator(`.host-row[data-host-id="${fixture.remote.id}"]`);
      await expect(row).toContainText(verb === "resume" ? "Suspended" : "Connected");
      if (verb === "resume") await row.getByRole("button", { name: "Resume", exact: true }).click();
      else {
        await row.locator(".host-row-menu").click();
        await row.getByRole("menuitem", { name: "Suspend", exact: true }).click();
      }
      await expect.poll(() => sent).toBe(1);
      await expect(row).toContainText(verb === "resume" ? "Resuming" : "Suspending");
      await expect(row).not.toContainText("Suspended outside Farhelm");
      if (verb === "suspend") await expect(row.getByRole("button", { name: "Resume", exact: true })).toBeDisabled();
      gate.release();
      await expect(row).toContainText(verb === "resume" ? "Connected" : "Suspended from Farhelm");
      expect(sent).toBe(1);
    } finally {
      gate.release();
      await stop();
    }
  });
}

/** An inline Resume and the menu both route through the mutation owner.
 * With no working sessions Suspend acts without a confirmation dialog. */
test("a suspended host has inline Resume and can suspend from its menu", async ({ page, request }) => {
  const fixture = await fleet(page, request);
  await page.goto("/");
  await openHostsPanel(page);
  const row = page.locator(`[data-host-id="${fixture.remote.id}"]`).filter({ has: page.locator(".host-row-main") })
    .first();
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
    await page.route("**/api/sessions**", async (route) => {
      if (route.request().method() !== "GET") return route.continue();
      const response = await route.fetch();
      if (!response.ok()) return route.fulfill({ response });
      const json = await response.json();
      if (Array.isArray(json.sessions)) {
        json.sessions = json.sessions.map((s: any) => s.id === session.id ? { ...s, stale: fixture.suspended() } : s);
      } else if (json.id === session.id) json.stale = fixture.suspended();
      await route.fulfill({ response, json });
    });
    await page.goto("/");
    const banner = page.getByRole("region", { name: "Session unavailable", exact: true });
    await expect(banner).toContainText("is suspended; this session continues when it resumes");
    await banner.getByRole("button", { name: "Resume host", exact: true }).click();
    await expect.poll(() => fixture.calls).toEqual(["resume"]);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/** Working or uncertain activity needs consent. Unknown is a legitimate
 * create-time state, and cached Idle does not establish current inactivity.
 * Cancel sends nothing; accepting sends exactly one lifecycle request. */
for (
  const activity of [
    { name: "working", state: "running", stale: false },
    { name: "unknown", state: "unknown", stale: false },
    { name: "stale idle", state: "idle", stale: true },
  ]
) {
  test(`suspending an agent with ${activity.name} status asks before sending the request`, async ({ page, request }) => {
    const fixture = await fleet(page, request, false);
    const session = await createSession(request, { title: "working-suspend", host: fixture.remote.id });
    try {
      let observed = false;
      await page.route("**/api/sessions?**", async (route) => {
        const response = await route.fetch();
        const json = await response.json();
        expect(json.sessions.some((s: any) => s.id === session.id), "owned agent is present in the activity read").toBe(
          true,
        );
        observed = true;
        json.sessions = json.sessions.map((s: any) =>
          s.id === session.id ? { ...s, stale: activity.stale, status: { state: activity.state } } : s
        );
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
    } finally {
      await cleanupSession(request, session.id);
    }
  });
}

/** Definite current Idle/Waiting evidence allows immediate Suspend even
 * with an agent session present; merely testing an empty host misses this. */
for (const state of ["idle", "waiting"]) {
  test(`suspending a current ${state} agent acts immediately`, async ({ page, request }) => {
    const fixture = await fleet(page, request, false);
    const session = await createSession(request, { title: `inactive-${state}`, host: fixture.remote.id });
    try {
      let observed = false;
      await page.route("**/api/sessions?**", async (route) => {
        const response = await route.fetch();
        const json = await response.json();
        expect(json.sessions.some((s: any) => s.id === session.id)).toBe(true);
        observed = true;
        json.sessions = json.sessions.map((s: any) =>
          s.id === session.id ? { ...s, stale: false, status: { state } } : s
        );
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
    } finally {
      await cleanupSession(request, session.id);
    }
  });
}

/** Supervisor reachability says nothing about whether the sandbox bills.
 * An unreachable running sandbox must still offer Suspend, with uncertainty
 * confirmation when session activity cannot be read. */
test("an unreachable supervisor still offers Suspend for its running sandbox", async ({ page, request }) => {
  const fixture = await fleet(page, request, false, {
    phase: "unreachable-reprobing",
    cause: "unknown",
    last_error: "Supervisor unavailable",
  });
  let guardRead = false;
  await page.route("**/api/sessions?**", async (route) => {
    if (new URL(route.request().url()).searchParams.get("host") !== String(fixture.remote.id)) return route.continue();
    // Preserve the real build stamp: an unavailable supervisor is not a
    // different helm build. The fixture already read that stamp; fetching
    // a body this refusal discards leaves a late fulfill racing retirement
    // of the application's request. Answer this owned stub immediately.
    guardRead = true;
    await route.fulfill({
      headers: { "x-farhelm-build": fixture.buildStamp },
      status: 503,
      json: { error: "Supervisor unavailable" },
    });
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
for (const duringRead of [false, true]) {
  test(`an empty cached listing asks when the host disconnects ${duringRead ? "during the read" : "before the click"}`, async ({ page, request }) => {
    const unreachable = { phase: "unreachable-reprobing", cause: "unknown", last_error: "Supervisor unavailable" };
    const fixture = await fleet(page, request, false, duringRead ? undefined : unreachable);
    let reads = 0;
    let release!: () => void;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route("**/api/sessions?**", async (route) => {
      if (new URL(route.request().url()).searchParams.get("host") !== String(fixture.remote.id)) {
        return route.continue();
      }
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
    } finally {
      release();
    }
  });
}

/** Existing-folder preparation spans the vendor call and the new connection.
 * Neither boundary may flash an unavailable warning or send create early;
 * the real eventual create proves the fresh connection token is accepted. */
test("launching in an existing folder resumes before creating", async ({ page, request }) => {
  const fixture = await fleet(page, request);
  const gate = routeGate();
  let resumes = 0;
  let creates = 0;
  let created: string | undefined;
  const stop = await drainedRoute(page, `**/api/hosts/${fixture.remote.id}/resume`, async (route) => {
    resumes++;
    await fixture.publishState({ phase: "tensorlake-changing", direction: "resuming", requested_at: Date.now() });
    await gate.wait();
    await fixture.publishState({ phase: "connecting", attempt: 0, stage: null, last_error: null });
    await route.fulfill({ status: 200, headers: { "x-farhelm-build": fixture.buildStamp }, json: {} });
  });
  page.on("request", (request) => {
    if (new URL(request.url()).pathname === "/api/sessions" && request.method() === "POST") creates++;
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
    const accepted = page.waitForResponse((r) =>
      new URL(r.url()).pathname === "/api/sessions" && r.request().method() === "POST"
    );
    const resumed = page.waitForResponse((r) => new URL(r.url()).pathname === `/api/hosts/${fixture.remote.id}/resume`);
    await form.locator(".create-session-submit").click();
    await expect.poll(() => resumes).toBe(1);
    const preparation = form.locator(".host-resume-progress");
    await expect(preparation).toHaveAttribute("role", "status");
    await expect(preparation).toContainText("Resuming Sandbox test");
    await expect(preparation).toContainText("the session starts when it’s ready");
    await expect(form).not.toContainText("The selected host is unavailable");
    expect(creates, "the held vendor call cannot dispatch a session").toBe(0);
    gate.release();
    expect((await resumed).ok()).toBe(true);
    await expect(page.locator(`.host-row[data-host-id="${fixture.remote.id}"]`)).toContainText("Connecting");
    await expect(preparation).toBeVisible();
    await expect(form).not.toContainText("The selected host is unavailable");
    expect(creates, "a successful Resume is not yet a connected launch target").toBe(0);
    await fixture.publishState();
    const response = await accepted;
    expect(response.ok(), await response.text()).toBe(true);
    expect(response.request().postDataJSON().expected_incarnation).toBe(fixture.remote.incarnation);
    created = (await response.json()).id;
    await expect(preparation).toHaveCount(0);
    expect(resumes).toBe(1);
    expect(creates).toBe(1);
  } finally {
    gate.release();
    await stop();
    if (created) await cleanupSession(request, created);
  }
});

/** The helper's private GET can see Connected before the displayed host reader.
 * Hold that reader across helper success and the ensuing create, so absence of
 * the unavailable warning cannot depend on feed/read scheduling luck. */
test("a successful resume stays honest while the displayed host read catches up", async ({ page, request }) => {
  const fixture = await fleet(page, request);
  const resumeGate = routeGate();
  const createGate = routeGate();
  let lag: ReturnType<typeof fixture.holdNextRead> | undefined;
  let resumes = 0;
  let creates = 0;
  let retiring = false;
  const stopResume = await drainedRoute(page, `**/api/hosts/${fixture.remote.id}/resume`, async (route) => {
    resumes++;
    await fixture.publishState({ phase: "tensorlake-changing", direction: "resuming", requested_at: Date.now() });
    await resumeGate.wait();
    if (!retiring) {
      lag = fixture.holdNextRead();
      await fixture.publishState({ phase: "connecting", attempt: 0, stage: null, last_error: null });
      await lag.entered;
    }
    await fixture.publishState();
    await route.fulfill({ status: 200, headers: { "x-farhelm-build": fixture.buildStamp }, json: {} });
  });
  const stopCreate = await drainedRoute(page, "**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    expect(route.request().postDataJSON()).toMatchObject({
      host: fixture.remote.id,
      expected_incarnation: fixture.remote.incarnation,
    });
    creates++;
    await createGate.wait();
    await route.fulfill({
      status: 409,
      headers: { "x-farhelm-build": fixture.buildStamp },
      json: { error: "handoff create boundary" },
    });
  });
  try {
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await form.getByRole("tab", { name: "Command", exact: true }).click();
    await form.getByLabel("Agent command", { exact: true }).fill("sleep 300");
    await form.getByRole("group", { name: "Runs in YOLO mode" }).getByLabel("No", { exact: true }).check();
    await chooseLauncherHost(form.getByRole("combobox", { name: "Host", exact: true }), String(fixture.remote.id));
    await fillLauncherFolder(form, "/tmp");
    await form.locator(".create-session-submit").click();
    await expect.poll(() => resumes).toBe(1);
    await expect(form.locator(".host-resume-progress")).toBeVisible();
    await expect(form.getByRole("combobox", { name: "Host", exact: true })).toContainText("Resuming");
    resumeGate.release();
    await expect.poll(() => creates, { message: "private Connected read has authorized the held create" }).toBe(1);
    await expect(form).not.toContainText("The selected host is unavailable");
    lag!.release();
    await expect(page.locator(`.host-row[data-host-id="${fixture.remote.id}"]`)).toContainText("Connected");
    await expect(form).not.toContainText("The selected host is unavailable");
    createGate.release();
    await expect(form.getByRole("alert")).toContainText("handoff create boundary");
  } finally {
    retiring = true;
    resumeGate.release();
    lag?.release();
    createGate.release();
    await stopResume();
    await stopCreate();
  }
});

/** A form owns only its own preparation. A lifecycle transition started
 * elsewhere must retain the ordinary unavailable warning and refuse launch,
 * even though that same phase is allowed during this form's own Resume. */
for (const direction of ["suspending", "resuming"] as const) {
  test(`another surface ${direction} the host does not make the launcher available`, async ({ page, request }) => {
    const fixture = await fleet(page, request, false);
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await form.getByRole("tab", { name: "Command", exact: true }).click();
    await form.getByLabel("Agent command", { exact: true }).fill("sleep 300");
    await form.getByRole("group", { name: "Runs in YOLO mode" }).getByLabel("No", { exact: true }).check();
    await chooseLauncherHost(form.getByRole("combobox", { name: "Host", exact: true }), String(fixture.remote.id));
    await fillLauncherFolder(form, "/tmp");
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    await fixture.publishState({ phase: "tensorlake-changing", direction, requested_at: Date.now() });
    await expect(form).toContainText("The selected host is unavailable");
    await expect(form.locator(".host-resume-progress")).toHaveCount(0);
    await expect(form.locator(".create-session-submit")).toBeDisabled();
    expect(fixture.calls, "this form must not start a competing lifecycle request").toEqual([]);
  });
}

/** Hidden quick launch reaches the same resume preparation without opening
 * the composer. A held POST proves its pill is preparation rather than the
 * later create-progress timer, and the refusal reveals exactly one sent create. */
test("quick launch shows preparation while resuming its suspended host", async ({ page, request }) => {
  await page.addInitScript(() => {
    Object.defineProperty(Navigator.prototype, "platform", { get: () => "Linux x86_64" });
  });
  const fixture = await fleet(page, request);
  expect(fixture.remote.identity, "the quick template binds a real supervisor identity").toBeTruthy();
  const previous = (await readPreferences(request)).quick_launch_template ?? null;
  const name = `suspended-quick-${Date.now()}`;
  const gate = routeGate();
  let resumes = 0;
  const creates: Record<string, unknown>[] = [];
  let bodyFailed = false;
  const stopResume = await drainedRoute(page, `**/api/hosts/${fixture.remote.id}/resume`, async (route) => {
    resumes++;
    await fixture.publishState({ phase: "tensorlake-changing", direction: "resuming", requested_at: Date.now() });
    await gate.wait();
    await fixture.publishState();
    await route.fulfill({ status: 200, headers: { "x-farhelm-build": fixture.buildStamp }, json: {} });
  });
  const stopCreate = await drainedRoute(page, "**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    creates.push(route.request().postDataJSON());
    await route.fulfill({
      status: 409,
      headers: { "x-farhelm-build": fixture.buildStamp },
      json: { error: "quick resume create boundary" },
    });
  });
  try {
    const saved = await request.put(`/api/templates/${encodeURIComponent(name)}`, {
      data: {
        kind: "command",
        command: "sleep 300",
        yolo: false,
        name: "suspended quick launch",
        host: fixture.remote.identity,
        destination: { folder: "/tmp" },
      },
    });
    expect(saved.ok(), await saved.text()).toBe(true);
    await patchPreferences(request, { quick_launch_template: name });
    expect((await readPreferences(request)).quick_launch_template).toBe(name);
    await page.goto("/");
    await expect(page.locator(".quick-launch-trigger")).toHaveCount(1);
    await page.keyboard.press("Control+Shift+p");
    await expect.poll(() => resumes).toBe(1);
    const preparation = page.locator(".launch-progress-pill .host-resume-progress");
    await expect(preparation).toBeVisible();
    await expect(preparation).toContainText("Resuming Sandbox test");
    await expect(page.locator(".create-session-form")).toBeHidden();
    expect(creates, "a hidden launch still waits for resume before creating").toHaveLength(0);
    gate.release();
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.getByRole("alert")).toContainText("quick resume create boundary");
    await expect(page.locator(".host-resume-progress")).toHaveCount(0);
    expect(resumes).toBe(1);
    expect(creates).toHaveLength(1);
    expect(creates[0]).toMatchObject({ host: fixture.remote.id, expected_incarnation: fixture.remote.incarnation });
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    gate.release();
    await cleanupOwnedSteps(bodyFailed, [
      ["resume route", stopResume],
      ["create route", stopCreate],
      ["quick-launch choice", () => patchPreferences(request, { quick_launch_template: previous })],
      ["quick-launch template", async () => {
        const removed = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }],
    ]);
  }
});

/** A failed Resume finishes preparation without dispatching a create. Its
 * error must use the launch-attempt refusal, never the unknown-outcome promise
 * reserved for a session mutation that might have reached the supervisor. */
test("a failed resume refuses an existing-folder launch without creating", async ({ page, request }) => {
  const fixture = await fleet(page, request);
  let resumes = 0;
  let creates = 0;
  await page.route(`**/api/hosts/${fixture.remote.id}/resume`, async (route) => {
    resumes++;
    await route.fulfill({
      status: 409,
      headers: { "x-farhelm-build": fixture.buildStamp },
      json: { error: "fixture resume refused" },
    });
  });
  await page.route("**/api/sessions", async (route) => {
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
  const stop = await drainedRoute(page, "**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    const input = route.request().postDataJSON();
    expect(input.host).toBe(fixture.remote.id);
    expect(input.expected_incarnation).toBe(fixture.remote.incarnation);
    expect(fixture.calls).toEqual(["resume"]);
    expect(fixture.suspended()).toBe(false);
    creates++;
    await gate.wait();
    await route.fulfill({
      status: 409,
      headers: { "x-farhelm-build": fixture.buildStamp },
      json: { error: "fixture create refused" },
    });
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
  const gate = routeGate();
  let lag: ReturnType<typeof fixture.holdNextRead> | undefined;
  let resumes = 0;
  let retiring = false;
  const stop = await drainedRoute(page, `**/api/hosts/${fixture.remote.id}/resume`, async (route) => {
    resumes++;
    await fixture.publishState({ phase: "tensorlake-changing", direction: "resuming", requested_at: Date.now() });
    await gate.wait();
    if (!retiring) {
      lag = fixture.holdNextRead();
      await fixture.publishState({ phase: "connecting", attempt: 0, stage: null, last_error: null });
      await lag.entered;
    }
    await fixture.publishState();
    await route.fulfill({ status: 200, headers: { "x-farhelm-build": fixture.buildStamp }, json: {} });
  });
  try {
    const history = await request.get(`/api/launch-history?host=${fixture.remote.id}`);
    expect(history.ok()).toBe(true);
    const configRevision = (await history.json()).checkout_config_revision;
    let creates = 0;
    const previewConnections: number[] = [];
    await page.route("**/api/github-repositories", async (route) => {
      const input = route.request().postDataJSON();
      await route.fulfill({
        status: 200,
        headers: { "x-farhelm-build": (await request.get("/api/hosts")).headers()["x-farhelm-build"] },
        json: {
          host: String(input.host),
          incarnation: input.expected_incarnation,
          installation_identity: fixture.remote.identity,
          repos: [],
          truncated: true,
          scan_error: "fixture scan unavailable",
        },
      });
    });
    await page.route("**/api/github-checkout-preview", async (route) => {
      const response = await request.get("/api/hosts");
      const input = route.request().postDataJSON();
      previewConnections.push(input.expected_incarnation);
      await route.fulfill({
        status: 200,
        headers: { "x-farhelm-build": response.headers()["x-farhelm-build"] },
        json: {
          canonical_root: "/tmp",
          basename: "bar-1",
          cwd: "/tmp/bar-1",
          config_revision: configRevision,
          host: String(input.host),
          incarnation: input.expected_incarnation,
          installation_identity: fixture.remote.identity,
        },
      });
    });
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() === "POST") {
        creates++;
        await route.abort();
      } else await route.continue();
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
    await expect.poll(() => resumes).toBe(1);
    const preparation = form.locator(".host-resume-progress");
    await expect(preparation).toContainText("Resuming Sandbox test");
    await expect(preparation).toContainText("review the checkout and launch again");
    await expect(form.getByRole("combobox", { name: "Host", exact: true })).toContainText("Resuming");
    await expect(preparation).not.toContainText("the session starts when it’s ready");
    await expect(form).not.toContainText("The selected host is unavailable");
    expect(creates, "preparation cannot authorize a checkout from the new connection").toBe(0);
    gate.release();
    await expect(form).toContainText("Review the refreshed checkout preview");
    await expect(form).not.toContainText("The selected host is unavailable");
    lag!.release();
    await expect.poll(() => previewConnections).toContain(fixture.remote.incarnation);
    await expect(preparation).toHaveCount(0);
    expect(creates).toBe(0);
  } finally {
    retiring = true;
    gate.release();
    lag?.release();
    await stop();
  }
});
