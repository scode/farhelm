/** Mounted lifecycle surfaces use the real connected remote host, with only
 * its displayed Tensorlake state and lifecycle replies controlled here. Rust
 * lifecycle tests cover vendor calls and connection teardown against the helm's
 * controlled sandbox service; these cases never dial Tensorlake's gateway. */
import { expect, test } from "./helpers/evidence";
import type { APIRequestContext, Locator, Page } from "@playwright/test";
import {
  cleanupSession,
  createSession,
  FAKE_AGENT,
  localHostId,
  openHostsPanel,
  listSessions,
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
import { attachSession, terminalInput, waitForTermText } from "./helpers/term";
import { waitForSessionReady } from "./helpers/terminal-readiness";
import { type InputFrame, programInput, recordInput } from "./helpers/program-input";
import { selectSessionActionWith } from "./helpers/session-actions";

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
  // Stable like the helm's own marks, which a launch's wake compares to tell
  // a new suspension from the one it started from.
  let suspendedAt = Date.now();
  let reads = 0;
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
              ? { phase: "suspended", observed_at: suspendedAt, suspended_by_farhelm_at: null, resume_failed: null }
              : runningState ?? h.state),
            tensorlake: { sandbox: "farhelm-host-test", recipe: "test", size: { cpus: 2, memory_gb: 4, disk_gb: 20 } },
          }
          : h
      );
      reads++;
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
        if (suspended) suspendedAt = Date.now();
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
    /** Host listings served so far, to wait for the page to read a state. */
    reads: () => reads,
    /** The suspended phase as currently displayed, marks included. */
    suspendedState: () => ({ phase: "suspended", observed_at: suspendedAt, suspended_by_farhelm_at: null, resume_failed: null }),
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

/** Record transient launcher admission, not only its final hidden appearance.
 * Attribute old values expose a reveal/re-hide in one task. The terminal
 * branch also records isolation writes on B's ancestor path; a switcher may
 * legitimately isolate that path, so that case observes only the launcher.
 * Observation never wraps focus, changes a product hook, or prevents input. */
async function observeWakeTransition(form: Locator, protectTerminal: boolean) {
  return form.evaluateHandle((node, protectTerminal) => {
    const root = node as HTMLElement;
    const input = document.querySelector('#terminal :is(.grid-terminal-input, .xterm-helper-textarea)');
    if (!input || document.activeElement !== input || input.closest('[inert]') ||
      !root.hidden || root.getAttribute('aria-modal') === 'true') {
      throw new Error('wake observation requires a released hidden launcher and focused B terminal');
    }
    const events: string[] = [];
    let dropped = 0;
    const note = (event: string) => { if (events.length < 64) events.push(event); else dropped++; };
    const presentation = new MutationObserver((records) => {
      for (const record of records) {
        if (record.attributeName === 'aria-modal' &&
          (record.oldValue === 'true' || root.getAttribute('aria-modal') === 'true')) note('launcher admitted');
        if (record.attributeName === 'hidden' && (record.oldValue === null || !root.hidden)) note('launcher revealed');
      }
    });
    presentation.observe(root, { attributes: true, attributeOldValue: true, attributeFilter: ['hidden', 'aria-modal'] });
    const isolation = new MutationObserver((records) => {
      for (const record of records) {
        if (protectTerminal && record.target instanceof Element && record.target.contains(input)) {
          note('B ancestor inert write');
        }
      }
    });
    isolation.observe(document.body, { subtree: true, attributes: true, attributeFilter: ['inert'] });
    const onFocus = (event: FocusEvent) => {
      if (event.target instanceof Node && root.contains(event.target)) note('launcher took focus');
    };
    document.addEventListener('focusin', onFocus, true);
    return {
      events,
      get dropped() { return dropped; },
      dispose() {
        presentation.disconnect();
        isolation.disconnect();
        document.removeEventListener('focusin', onFocus, true);
      },
    };
  }, protectTerminal);
}

/** Resume is preparation for the accepted create, not a modal Return.
 * Keep the next create held so a final refusal cannot hide an intervening
 * flash. A switcher stays open past the helper's fresh Connected read: a
 * blocked create then means dialog admission, not host readiness, delayed it. */
for (const mode of ["form", "quick"] as const) {
  for (const elsewhere of ["terminal", "switcher"] as const) {
    test(`wake-to-create leaves ${elsewhere} alone (${mode})`, async ({ page, request }, testInfo) => {
      const fixture = await fleet(page, request);
      const previous = await readPreferences(request);
      const resumeGate = routeGate();
      const createGate = routeGate();
      let stopResume: (() => Promise<void>) | undefined;
      let stopCreate: (() => Promise<void>) | undefined;
      const template = `wake-completion-${mode}-${elsewhere}-${Date.now()}`;
      let other: Awaited<ReturnType<typeof createSession>> | undefined;
      let trace: Awaited<ReturnType<typeof observeWakeTransition>> | undefined;
      let lag: ReturnType<typeof fixture.holdNextRead> | undefined;
      let retiring = false;
      let resumes = 0;
      const creates: Array<{ host: number; expected_incarnation: number }> = [];
      let bodyFailed = true;
      try {
        expect(fixture.remote.identity, "the suspended target retains its real installation identity").toBeTruthy();
        const local = await localHostId(request);
        expect(local, "B must remain on the real Local installation").not.toBe(fixture.remote.id);
        other = await createSession(request, { title: `wake-bystander-${mode}-${elsewhere}`, host: local });
        const frames: InputFrame[] = [];
        const otherId = other.id;
        page.on("websocket", (socket) => {
          if (new URL(socket.url()).pathname === `/api/sessions/${encodeURIComponent(otherId)}/term`) {
            recordInput(socket, frames);
          }
        });
        if (mode === "quick") {
          const saved = await request.put(`/api/templates/${encodeURIComponent(template)}`, { data: {
            kind: "command", command: "sleep 300", yolo: false,
            host: fixture.remote.identity, destination: { folder: "/tmp" },
          } });
          expect(saved.ok(), await saved.text()).toBe(true);
        }
        await patchPreferences(request, { welcome_seen: true,
          ...(mode === "quick" ? { quick_launch_template: template } : {}) });
        stopResume = await drainedRoute(page, `**/api/hosts/${fixture.remote.id}/resume`, async (route) => {
          expect(route.request().method()).toBe("POST");
          resumes++;
          await fixture.publishState({ phase: "tensorlake-changing", direction: "resuming", requested_at: Date.now() });
          await resumeGate.wait();
          if (!retiring) {
            // Hold the ordinary reader's Connecting snapshot. The helper's
            // independent read is then the only Connected reply we observe.
            lag = fixture.holdNextRead();
            await fixture.publishState({ phase: "connecting", attempt: 0, stage: null, last_error: null });
            await lag.entered;
          }
          await fixture.publishState();
          await route.fulfill({ status: 200, headers: { "x-farhelm-build": fixture.buildStamp }, json: {} });
        });
        stopCreate = await drainedRoute(page, "**/api/sessions", async (route) => {
          if (route.request().method() !== "POST") return route.fallback();
          const body = route.request().postDataJSON();
          expect(body).toMatchObject({ host: fixture.remote.id, expected_incarnation: fixture.remote.incarnation });
          creates.push(body);
          await createGate.wait();
          await route.fulfill({ status: 409,
            headers: { "x-farhelm-build": fixture.buildStamp, "x-farhelm-create-outcome": "definitely-unaccepted" },
            body: "wake handoff fixture refused" });
        });
        await pinAutoSelect(page, other.id);
        await page.goto("/");
        await attachSession(page, other.id);
        await waitForTermText(page, "FAKE-AGENT READY");
        await waitForTermText(page, "> ");
        const mac = await page.evaluate(() => /Mac/.test(navigator.platform));
        const form = page.locator('.create-session-form[role="dialog"]');
        if (mode === "quick") {
          await expect(page.locator(".quick-launch-trigger")).toBeEnabled();
          await page.keyboard.press(mac ? "Meta+p" : "Control+Shift+p");
        } else {
          await page.locator(".new-session-button").click();
          await form.getByRole("tab", { name: "Command", exact: true }).click();
          await form.getByLabel("Agent command", { exact: true }).fill("sleep 300");
          await form.getByRole("group", { name: "Runs in YOLO mode" }).getByLabel("No", { exact: true }).check();
          await chooseLauncherHost(form.getByRole("combobox", { name: "Host", exact: true }), String(fixture.remote.id));
          await expect(form.getByRole("combobox", { name: "Host", exact: true })).toContainText("Suspended");
          await fillLauncherFolder(form, "/tmp");
          await form.locator(".create-session-submit").click();
        }
        await expect.poll(() => resumes, { message: "one Resume is held before exercising the released page" }).toBe(1);
        await expect(form).toBeHidden();
        await expect(form).not.toHaveAttribute("aria-modal", "true");
        await expect(page.locator(".launch-progress-pill .host-resume-progress")).toBeVisible();
        expect(creates).toHaveLength(0);
        await terminalInput(page).focus();
        await waitForSessionReady(page, other.id);
        const prefix = `wake-input-${mode}-${elsewhere}`;
        frames.length = 0;
        await page.keyboard.type(prefix);
        await expect.poll(() => programInput(frames)).toBe(prefix);
        trace = await observeWakeTransition(form, elsewhere === "terminal");
        const switcher = page.getByRole("dialog", { name: "Quick switcher", exact: true });
        if (elsewhere === "switcher") {
          await page.keyboard.press(mac ? "Meta+k" : "Control+Shift+k");
          await expect(switcher).toBeVisible();
          await expect(switcher.getByRole("combobox")).toBeFocused();
        }
        const connected = page.waitForResponse(async (response) => {
          if (new URL(response.url()).pathname !== "/api/hosts" || !response.ok()) return false;
          const json = await response.json();
          return json.hosts.some((host: any) => host.id === fixture.remote.id &&
            host.incarnation === fixture.remote.incarnation && host.state.phase === "connected");
        });
        resumeGate.release();
        await connected;
        await expect.poll(() => creates.length, {
          timeout: 10_000,
          message: `a fresh Connected reply must dispatch create while ${elsewhere} still owns the page`,
        }).toBe(1);
        const observed = await trace.evaluate(({ events, dropped }) => ({ events, dropped }));
        expect(observed, "waking the host must not briefly admit, reveal or isolate the launcher").toEqual({ events: [], dropped: 0 });
        await expect(form).toBeHidden();
        await expect(form).not.toHaveAttribute("aria-modal", "true");
        if (elsewhere === "switcher") {
          await expect(switcher.getByRole("combobox")).toBeFocused();
          await page.keyboard.press("Escape");
          await expect(switcher).toHaveCount(0);
        }
        await expect(terminalInput(page)).toBeFocused();
        expect(await terminalInput(page).evaluate((input) => !!input.closest("[inert]"))).toBe(false);
        frames.length = 0;
        await page.keyboard.press("Enter");
        await expect.poll(() => programInput(frames)).toBe("\r");
        await waitForTermText(page, `echo:${prefix}`);
        await expect(page.locator(`[data-session-id="${other.id}"]`)).toHaveAttribute("data-session-selected", "true");
        bodyFailed = false;
      } finally {
        retiring = true;
        resumeGate.release();
        lag?.release();
        await cleanupOwnedSteps(bodyFailed, [
          ["wake observation", async () => {
            if (!trace) return;
            const observed = await trace.evaluate(({ events, dropped }) => ({ events, dropped }));
            await testInfo.attach("wake-transition.json", { body: JSON.stringify(observed), contentType: "application/json" });
            await trace.evaluate((state) => state.dispose());
            await trace.dispose();
          }],
          ["switcher", async () => {
            const switcher = page.getByRole("dialog", { name: "Quick switcher", exact: true });
            if (await switcher.count()) {
              await switcher.getByRole("button", { name: "Close", exact: true }).click();
              await expect(switcher).toHaveCount(0);
            }
          }],
          // The create refusal is a legitimate modal Return. Retire the
          // transition observation before allowing that different phase.
          ["create gate", () => createGate.release()],
          ["Resume route", async () => { await stopResume?.(); }],
          // Closing the switcher can admit the old source's late create.
          // Keep its trap until navigation cancels the mounted launcher;
          // drain Resume first while its fixture can still use the feed.
          ["submitted owner", async () => { await page.goto("about:blank"); }],
          ["create route", async () => { await stopCreate?.(); }],
          ["owned bystander", async () => { if (other) await cleanupSession(request, other.id); }],
          ["quick template", async () => {
            if (mode !== "quick") return;
            const removed = await request.delete(`/api/templates/${encodeURIComponent(template)}`);
            expect(removed.ok() || removed.status() === 404).toBe(true);
          }],
          ["preferences", () => patchPreferences(request, {
            welcome_seen: previous.welcome_seen ?? null,
            quick_launch_template: previous.quick_launch_template ?? null,
          })],
        ]);
      }
    });
  }
}

/** Wake and create are steps of one accepted quick launch, not new answers.
 * No physical input follows the shortcut until the real created row is selected
 * and its normal naming editor opens. This catches an internal reentry losing
 * the original press baseline, which refusal-only wake cases cannot detect. */
test("an uninterrupted quick launch opens its session and naming after wake", async ({ page, request }) => {
  const fixture = await fleet(page, request);
  const previous = await readPreferences(request);
  const name = `attended-wake-${Date.now()}`;
  const owned: string[] = [];
  const resumeGate = routeGate();
  const createGate = routeGate();
  let stopResume: (() => Promise<void>) | undefined;
  let stopCreate: (() => Promise<void>) | undefined;
  let resumes = 0;
  let creates = 0;
  let created: string | undefined;
  let retiring = false;
  let bodyFailed = true;
  try {
    expect(fixture.remote.identity, "the shortcut targets the real remote installation").toBeTruthy();
    const other = await createSession(request, { title: `${name}-origin`, host: await localHostId(request) });
    owned.push(other.id);
    const saved = await request.put(`/api/templates/${encodeURIComponent(name)}`, { data: {
      kind: "command", command: FAKE_AGENT, yolo: false, name,
      host: fixture.remote.identity, destination: { folder: "/tmp" },
    } });
    expect(saved.ok(), await saved.text()).toBe(true);
    await patchPreferences(request, {
      welcome_seen: true, quick_launch_template: name, name_session_on_quick_launch: true,
    });
    stopResume = await drainedRoute(page, `**/api/hosts/${fixture.remote.id}/resume`, async (route) => {
      expect(route.request().method()).toBe("POST");
      resumes++;
      await fixture.publishState({ phase: "tensorlake-changing", direction: "resuming", requested_at: Date.now() });
      await resumeGate.wait();
      await fixture.publishState();
      await route.fulfill({ status: 200, headers: { "x-farhelm-build": fixture.buildStamp }, json: {} });
    });
    stopCreate = await drainedRoute(page, "**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.fallback();
      expect(route.request().postDataJSON()).toMatchObject({
        host: fixture.remote.id, expected_incarnation: fixture.remote.incarnation,
      });
      creates++;
      await createGate.wait();
      if (retiring) {
        await route.fulfill({ status: 409, headers: {
          "x-farhelm-build": fixture.buildStamp, "x-farhelm-create-outcome": "definitely-unaccepted",
        }, body: "retired owned quick launch" });
        return;
      }
      const response = await route.fetch();
      expect(response.ok(), await response.text()).toBe(true);
      created = (await response.json()).id;
      expect(created).toEqual(expect.any(String));
      owned.push(created!);
      await route.fulfill({ response });
    });
    await pinAutoSelect(page, other.id);
    await page.goto("/");
    await attachSession(page, other.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await waitForTermText(page, "> ");
    const mac = await page.evaluate(() => /Mac/.test(navigator.platform));
    await expect(page.locator(".quick-launch-trigger")).toBeEnabled();
    await page.keyboard.press(mac ? "Meta+p" : "Control+Shift+p");

    // Only route gates and observations run until naming has opened. Even a
    // harmless click here would turn this into the moved-on branch instead.
    await expect.poll(() => resumes, { message: "the accepted shortcut is waiting for its own Resume" }).toBe(1);
    const form = page.locator(".create-session-form");
    await expect(form).toBeHidden();
    await expect(form).not.toHaveAttribute("aria-modal", "true");
    await expect(page.locator(".launch-progress-pill .host-resume-progress")).toBeVisible();
    expect(creates).toBe(0);
    resumeGate.release();
    await expect.poll(() => creates, { message: "the same shortcut dispatches one create after wake" }).toBe(1);
    await expect(form).toBeHidden();
    await expect(page.locator(`[data-session-id="${other.id}"]`)).toHaveAttribute("data-session-selected", "true");
    createGate.release();
    await expect.poll(() => created, { message: "the real supervisor accepted the owned quick create" }).toEqual(expect.any(String));
    const createdRow = page.locator(`.session-row[data-session-id="${created!}"]`);
    await expect(createdRow).toHaveAttribute("data-session-selected", "true");
    const naming = page.locator(".rename-dialog");
    await expect(naming).toBeVisible();
    await expect(naming.getByLabel("New session title", { exact: true })).toHaveValue(name);
    await expect(naming.getByLabel("New session title", { exact: true })).toBeFocused();
    await expect(form).toHaveCount(0);
    expect(creates).toBe(1);
    expect(resumes).toBe(1);

    // Naming has already proved the uninterrupted completion took its normal
    // branch. Its ordinary Cancel now hands the actual created terminal focus.
    await naming.locator(".rename-cancel").click();
    await expect(naming).toHaveCount(0);
    await waitForSessionReady(page, created!);
    await waitForTermText(page, "FAKE-AGENT READY");
    await waitForTermText(page, "> ");
    await expect.poll(async () => (await readPreferences(request)).last_selected).toBe(created);
    bodyFailed = false;
  } finally {
    retiring = true;
    resumeGate.release();
    createGate.release();
    await cleanupOwnedSteps(bodyFailed, [
      ["Resume route", async () => { await stopResume?.(); }],
      ["submitted owner", async () => { await page.goto("about:blank"); }],
      ["create route", async () => { await stopCreate?.(); }],
      // A real create may finish during the route drain above. Read ownership
      // only now, so an id captured by that callback is included in cleanup.
      ["owned quick sessions", async () => {
        await cleanupOwnedSteps(bodyFailed, owned.reverse().map((id): [string, () => Promise<void>] =>
          ["owned quick session", () => cleanupSession(request, id)]));
      }],
      ["quick template", async () => {
        const removed = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }],
      ["preferences", () => patchPreferences(request, {
        welcome_seen: previous.welcome_seen ?? null,
        quick_launch_template: previous.quick_launch_template ?? null,
        name_session_on_quick_launch: previous.name_session_on_quick_launch ?? null,
      })],
    ]);
  }
});

/** A launch's wake beside other lifecycle work (SPEC.md, "Topology"): a Resume
 * refused only because the host is busy is sent again and the launch goes on;
 * the host suspended again (a later mark) while the wake waits fails the launch
 * at once, long before its 60-second budget; and the suspension it started from
 * still showing is not mistaken for that. The real helm shows that starting
 * reading while the wake's Resume is refused as busy, not after an accepted
 * one; the third case serves it after acceptance only to pin the rule itself. */
for (const scenario of ["busy, then accepted", "suspended again", "stale suspension after the resume"] as const) {
  test(`a quick launch's wake: ${scenario}`, async ({ page, request }) => {
    const fixture = await fleet(page, request);
    const previous = await readPreferences(request);
    const name = `wake-${scenario.split(",")[0].replaceAll(" ", "-")}-${Date.now()}`;
    const owned: string[] = [];
    const answered = routeGate();
    let stopResume: (() => Promise<void>) | undefined;
    let stopCreate: (() => Promise<void>) | undefined;
    let resumes = 0;
    let creates = 0;
    let created: string | undefined;
    let bodyFailed = true;
    try {
      const other = await createSession(request, { title: `${name}-origin`, host: await localHostId(request) });
      owned.push(other.id);
      const saved = await request.put(`/api/templates/${encodeURIComponent(name)}`, { data: {
        kind: "command", command: FAKE_AGENT, yolo: false, name,
        host: fixture.remote.identity, destination: { folder: "/tmp" },
      } });
      expect(saved.ok(), await saved.text()).toBe(true);
      await patchPreferences(request, { welcome_seen: true, quick_launch_template: name, name_session_on_quick_launch: false });
      const starting = fixture.suspendedState();
      stopResume = await drainedRoute(page, `**/api/hosts/${fixture.remote.id}/resume`, async (route) => {
        if (++resumes === 1 && scenario === "busy, then accepted") {
          await route.fulfill({ status: 409, body: "This host is busy; try again after its current operation finishes", headers: {
            "x-farhelm-build": fixture.buildStamp, "x-farhelm-host-busy": "operation-in-progress",
          } });
          return;
        }
        if (scenario === "busy, then accepted") return route.fallback();
        // Accepted, with the host not yet connected: the starting suspension
        // (stale) or a connection attempt the host published itself.
        await fixture.publishState(scenario === "suspended again" ? { phase: "connecting", attempt: 1, last_error: null } : starting);
        await route.fulfill({ status: 200, headers: { "x-farhelm-build": fixture.buildStamp }, json: {} });
        answered.release();
      });
      stopCreate = await drainedRoute(page, "**/api/sessions", async (route) => {
        if (route.request().method() !== "POST") return route.fallback();
        creates++;
        const response = await route.fetch();
        // Accepted by the real helm and supervisor, on the woken connection:
        // a create sent with the suspended connection's token is refused.
        expect(response.ok(), await response.text()).toBe(true);
        created = (await response.json()).id;
        owned.push(created!);
        await route.fulfill({ response });
      });
      await pinAutoSelect(page, other.id);
      await page.goto("/");
      await attachSession(page, other.id);
      await waitForTermText(page, "> ");
      const mac = await page.evaluate(() => /Mac/.test(navigator.platform));
      await expect(page.locator(".quick-launch-trigger")).toBeEnabled();
      await page.keyboard.press(mac ? "Meta+p" : "Control+Shift+p");

      if (scenario === "busy, then accepted") {
        await expect.poll(() => resumes, { message: "the busy refusal is followed by a second Resume" }).toBe(2);
      } else {
        await answered.wait();
        // Two more listings after the answer: the wake has read the state the
        // Resume left before anything changes it.
        const read = fixture.reads();
        await expect.poll(() => fixture.reads() - read).toBeGreaterThanOrEqual(2);
        expect(creates, "no create while the host has not connected").toBe(0);
        if (scenario === "suspended again") {
          const now = Date.now();
          await fixture.publishState({ phase: "suspended", observed_at: now, suspended_by_farhelm_at: now, resume_failed: null });
          await expect(page.getByText("The host was suspended again while it was waking")).toBeVisible({ timeout: 15_000 });
          expect(creates, "a launch whose host was suspended again sends no create").toBe(0);
          bodyFailed = false;
          return;
        }
        await fixture.publishState();
      }
      await expect.poll(() => created, { message: "the woken launch creates its session" }).toEqual(expect.any(String));
      expect(creates).toBe(1);
      await expect(page.getByText(/Resuming the host failed|stayed busy|suspended again/)).toHaveCount(0);
      bodyFailed = false;
    } finally {
      answered.release();
      await cleanupOwnedSteps(bodyFailed, [
        ["Resume route", async () => { await stopResume?.(); }],
        ["submitted owner", async () => { await page.goto("about:blank"); }],
        ["create route", async () => { await stopCreate?.(); }],
        ["owned sessions", async () => {
          await cleanupOwnedSteps(bodyFailed, owned.reverse().map((id): [string, () => Promise<void>] =>
            ["owned session", () => cleanupSession(request, id)]));
        }],
        ["quick template", async () => {
          const removed = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
          expect(removed.ok() || removed.status() === 404).toBe(true);
        }],
        ["preferences", () => patchPreferences(request, {
          welcome_seen: previous.welcome_seen ?? null,
          quick_launch_template: previous.quick_launch_template ?? null,
          name_session_on_quick_launch: previous.name_session_on_quick_launch ?? null,
        })],
      ]);
    }
  });
}

/** Managed Replace with must claim its source at acceptance, including work
 * before any replace POST exists. Wake and the fresh preview retain that same
 * claim; a preparation refusal settles it before the returning draft can pass
 * another modal. B's actual Stop handler distinguishes per-session exclusion
 * from accidentally retaining the old page-wide lock. */
test("managed Replace with owns its source through wake and settles before Switcher return", async ({ page, request }) => {
  const fixture = await fleet(page, request, false);
  const previous = await readPreferences(request);
  const name = `managed-replace-wake-${Date.now()}`;
  const owned: string[] = [];
  const stops: Array<() => Promise<void>> = [];
  const resumeGate = routeGate();
  const previewGate = routeGate();
  let stopResume: (() => Promise<void>) | undefined;
  let lag: ReturnType<typeof fixture.holdNextRead> | undefined;
  let resumes = 0;
  let freshPreviews = 0;
  let sourceStops = 0;
  let otherStops = 0;
  let replaces = 0;
  let retiring = false;
  let bodyFailed = true;
  try {
    const local = await localHostId(request);
    expect(local, "B is independent of the displayed sandbox lifecycle").not.toBe(fixture.remote.id);
    const source = await createSession(request, { title: name, host: fixture.remote.id });
    owned.push(source.id);
    const other = await createSession(request, { title: `${name}-other`, host: local });
    owned.push(other.id);
    const history = await request.get(`/api/launch-history?host=${fixture.remote.id}`);
    expect(history.ok()).toBe(true);
    const configRevision = (await history.json()).checkout_config_revision;
    await patchPreferences(request, { welcome_seen: true, skip_session_confirmations: true });
    stops.push(await drainedRoute(page, "**/api/github-repositories", async (route) => {
      const input = route.request().postDataJSON();
      await route.fulfill({ status: 200, headers: { "x-farhelm-build": fixture.buildStamp }, json: {
        host: String(input.host), incarnation: input.expected_incarnation,
        installation_identity: fixture.remote.identity, repos: [], truncated: true, scan_error: "fixture scan unavailable",
      } });
    }));
    stops.push(await drainedRoute(page, "**/api/github-checkout-preview", async (route) => {
      const input = route.request().postDataJSON();
      expect(String(input.host)).toBe(String(fixture.remote.id));
      if (resumes > 0 && input.expected_incarnation === fixture.remote.incarnation) {
        freshPreviews++;
        await previewGate.wait();
        await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.buildStamp },
          json: { error: "held managed preparation refused" } });
        return;
      }
      await route.fulfill({ status: 200, headers: { "x-farhelm-build": fixture.buildStamp }, json: {
        canonical_root: "/tmp", basename: "managed-source-preview", cwd: "/tmp/managed-source-preview",
        config_revision: configRevision, host: String(input.host), incarnation: input.expected_incarnation,
        installation_identity: fixture.remote.identity,
      } });
    }));
    stopResume = await drainedRoute(page, `**/api/hosts/${fixture.remote.id}/resume`, async (route) => {
      expect(route.request().method()).toBe("POST");
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
    for (const [id, isSource] of [[source.id, true], [other.id, false]] as const) {
      stops.push(await drainedRoute(page, `**/api/sessions/${encodeURIComponent(id)}/stop`, async (route) => {
        expect(route.request().method()).toBe("POST");
        if (isSource) sourceStops++;
        else otherStops++;
        await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.buildStamp },
          body: isSource ? "source Stop handler admitted" : "independent B Stop admitted" });
      }));
    }
    stops.push(await drainedRoute(page, `**/api/sessions/${encodeURIComponent(source.id)}/replace`, async (route) => {
      replaces++;
      await route.fulfill({ status: 409, headers: {
        "x-farhelm-build": fixture.buildStamp, "x-farhelm-create-outcome": "definitely-unaccepted",
      }, body: "unexpected replace dispatch after failed preparation" });
    }));
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await selectSessionActionWith(page, "Replace");
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await form.getByRole("tab", { name: "Agent", exact: true }).click();
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    await search.fill("gh:acme/bar");
    await expect(form.getByRole("option", { name: "Managed checkout: acme/bar", exact: true })).toBeVisible();
    await search.press("Enter");
    // A typed checkout name makes the held refusal final; automatic naming
    // would legitimately try another suffix after an occupied-name reply.
    await form.getByLabel("Name (optional)").fill("held-replacement");
    await expect(form.locator(".launch-composer-checkout-preview")).toHaveAttribute("data-preview-state", "ready");
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    await expect(page.locator(".header-stop")).toBeEnabled();

    // Open an editable connected-host form first. Opening Replace with after
    // suspension would test the separate source-placement refusal instead.
    await fixture.publishState({ phase: "suspended", observed_at: Date.now(), suspended_by_farhelm_at: null, resume_failed: null });
    await expect(form.getByRole("combobox", { name: "Host", exact: true })).toContainText("Suspended");
    await expect(form.locator(".launch-composer-checkout-preview")).toHaveAttribute("data-preview-state", "ready");
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    const opening = await form.getAttribute("data-opening-generation");
    // Dispatch a competing real handler in the submit's own browser turn.
    // Synthetic click deliberately bypasses a later disabled render, so only
    // the synchronous source claim can refuse this stale queued action.
    await form.locator(".create-session-submit").evaluate((submit) => {
      const stop = document.querySelector<HTMLButtonElement>(".header-stop");
      if (!stop) throw new Error("the connected source Stop handler is missing");
      (submit as HTMLButtonElement).click();
      stop.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await expect.poll(() => resumes, { message: "Replace with is held in its hidden wake preparation" }).toBe(1);
    await expect(form).toBeHidden();
    await expect(form).not.toHaveAttribute("aria-modal", "true");
    await expect(page.locator(".launch-progress-pill .host-resume-progress")).toBeVisible();
    expect(sourceStops, "the queued source handler must not acquire a competing claim").toBe(0);
    expect(replaces).toBe(0);
    await attachSession(page, other.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(page.locator(".header-stop")).toBeEnabled();
    await page.locator(".header-stop").click();
    const otherRow = page.locator(`.session-row[data-session-id="${other.id}"]`);
    await expect(otherRow.locator(".session-row-error")).toContainText("independent B Stop admitted");
    expect(otherStops).toBe(1);
    expect(sourceStops).toBe(0);

    resumeGate.release();
    await expect.poll(() => freshPreviews, { message: "Resume continues into a preview on the real fresh SSH incarnation" }).toBe(1);
    expect(lag, "ordinary display refresh is held separately from Resume's fresh host read").toBeDefined();
    lag!.release();
    // Connected has no warning suffix. Rule out the sandbox's frozen-state
    // rendering before attributing the source's disabled action to its claim.
    await expect(form.locator(".create-session-host bdi")).toHaveText("Sandbox test");
    await attachSession(page, source.id);
    await expect(page.locator(".header-stop")).toBeDisabled();
    await page.locator(".header-stop").dispatchEvent("click");
    await expect(form).toBeHidden();
    await expect(page.locator(".launch-progress-pill .host-resume-progress")).toBeVisible();
    expect(sourceStops, "the original source owner survives internal preview reentry").toBe(0);
    const mac = await page.evaluate(() => /Mac/.test(navigator.platform));
    await page.keyboard.press(mac ? "Meta+k" : "Control+Shift+k");
    const switcher = page.getByRole("dialog", { name: "Quick switcher", exact: true });
    await expect(switcher.getByRole("combobox")).toBeFocused();
    previewGate.release();
    const sourceRow = page.locator(`.session-row[data-session-id="${source.id}"]`);
    await expect(sourceRow.locator(".session-row-error")).toContainText("held managed preparation refused");
    await expect(sourceRow.locator(".launch-progress")).toHaveCount(0);
    await expect(page.locator(".header-stop")).toBeEnabled();
    await expect(page.locator(".header-replace")).toBeEnabled();
    await expect(form).toBeHidden();
    await expect(form).not.toHaveAttribute("aria-modal", "true");
    await expect(form).toHaveAttribute("data-opening-generation", opening!);
    await expect(page.locator(".new-session-button")).toBeDisabled();
    await expect(switcher.getByRole("combobox")).toBeFocused();
    expect(sourceStops).toBe(0);
    expect(replaces, "a failed checkout preparation never dispatches Replace").toBe(0);
    // Probe the same handler while Return is still blocked. An enabled render
    // alone would miss a leaked claim in the underlying operation owner.
    await page.locator(".header-stop").dispatchEvent("click");
    await expect(sourceRow.locator(".session-row-error")).toContainText("source Stop handler admitted");
    expect(sourceStops).toBe(1);
    await expect(form).toBeHidden();
    await expect(switcher.getByRole("combobox")).toBeFocused();
    await switcher.getByRole("combobox").press("Escape");
    await expect(switcher).toHaveCount(0);
    await expect(form).toBeVisible();
    await expect(form.getByRole("alert")).toContainText("held managed preparation refused");
    await expect(page.locator(".host-resume-progress")).toHaveCount(0);
    await expect(form).toHaveAttribute("data-opening-generation", opening!);
    await form.locator(".launch-composer-cancel").click();
    await expect(form).toHaveCount(0);
    expect(sourceStops).toBe(1);
    bodyFailed = false;
  } finally {
    retiring = true;
    resumeGate.release();
    previewGate.release();
    lag?.release();
    await cleanupOwnedSteps(bodyFailed, [
      // Drain the lifecycle callback while its feed is still mounted, then
      // cancel the owner before removing traps for any late continuation.
      ["Resume route", async () => { await stopResume?.(); }],
      ["submitted owner", async () => { await page.goto("about:blank"); }],
      ...stops.map((stop, index): [string, () => Promise<void>] => [`managed preparation route ${index}`, stop]),
      ...owned.reverse().map((id): [string, () => Promise<void>] => ["owned managed-source session", () => cleanupSession(request, id)]),
      ["preferences", () => patchPreferences(request, {
        welcome_seen: previous.welcome_seen ?? null,
        skip_session_confirmations: previous.skip_session_confirmations ?? null,
      })],
    ]);
  }
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
  await expect(page.getByRole("dialog", { name: "Suspend Sandbox test?", exact: true })).toHaveCount(0);
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
    let stopActivity: (() => Promise<void>) | undefined;
    try {
      // The consent scenario needs a listed agent. Establish that premise
      // through the same host-scoped listing before controlling its activity.
      await expect.poll(async () => (await listSessions(request, `host=${fixture.remote.id}`)).sessions
        .some((row) => row.id === session.id)).toBe(true);
      let observed = false;
      stopActivity = await drainedRoute(page, "**/api/sessions?**", async (route) => {
        // The sidebar and other readers may ask for a different host. Only
        // Suspend's host-scoped read establishes the activity this test claims.
        if (new URL(route.request().url()).searchParams.get("host") !== String(fixture.remote.id)) {
          return route.continue();
        }
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
      const dialog = page.getByRole("dialog", { name: "Suspend Sandbox test?", exact: true });
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
      // A deletion can trigger another read. Retire this premise while the
      // session it requires still exists, before publishing that deletion.
      await stopActivity?.();
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
    let stopActivity: (() => Promise<void>) | undefined;
    try {
      await expect.poll(async () => (await listSessions(request, `host=${fixture.remote.id}`)).sessions
        .some((row) => row.id === session.id)).toBe(true);
      let observed = false;
      stopActivity = await drainedRoute(page, "**/api/sessions?**", async (route) => {
        if (new URL(route.request().url()).searchParams.get("host") !== String(fixture.remote.id)) {
          return route.continue();
        }
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
      await expect(page.getByRole("dialog", { name: "Suspend Sandbox test?", exact: true })).toHaveCount(0);
    } finally {
      // Match the uncertain-activity fixture's lifetime: no assertion about
      // this agent may outlive the agent's own listing entry.
      await stopActivity?.();
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
  const dialog = page.getByRole("dialog", { name: "Suspend Sandbox test?", exact: true });
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
      const dialog = page.getByRole("dialog", { name: "Suspend Sandbox test?", exact: true });
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

/** Submitted existing-folder preparation releases the page across the vendor call and new connection.
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
    await expect(form).toBeHidden();
    await expect(form).not.toHaveAttribute("aria-modal", "true");
    const preparation = page.locator(".launch-progress-pill .host-resume-progress");
    await expect(preparation).toBeVisible();
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

/** Submitted Resume stays non-modal when its private GET sees Connected before the displayed reader.
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
    await expect(form).toBeHidden();
    await expect(form).not.toHaveAttribute("aria-modal", "true");
    await expect(page.locator(".launch-progress-pill .host-resume-progress")).toBeVisible();
    const host = form.getByRole("combobox", { name: "Host", exact: true, includeHidden: true });
    await expect(host).toContainText("Resuming");
    await expect(host).toBeDisabled();
    resumeGate.release();
    await expect.poll(() => creates, { message: "private Connected read has authorized the held create" }).toBe(1);
    await expect(form).not.toContainText("The selected host is unavailable");
    lag!.release();
    await expect(page.locator(`.host-row[data-host-id="${fixture.remote.id}"]`)).toContainText("Connected");
    await expect(form).not.toContainText("The selected host is unavailable");
    createGate.release();
    await expect(form).toBeVisible();
    await expect(form).toHaveAttribute("aria-modal", "true");
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

/** Resume replaces the suspended connection before create starts in the non-modal pill. Hold that
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
    await expect(form).toBeHidden();
    await expect(form).not.toHaveAttribute("aria-modal", "true");
    const status = page.locator(".launch-progress-pill .launch-progress");
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

/** A fresh checkout spends one press only after the form observes Resume's
 * connection and its own preview. Held host/preview replies prove both fences;
 * named templates also exercise the existing occupied-name suffix search.
 * The submitted owner retains its pill and token through that preview wait,
 * so successful Resume alone neither reveals the draft nor admits another New.
 * Creates stop at a controlled refusal: this specifies launcher dispatch, while
 * github-checkouts.spec.ts exercises actual cloning and lost directory races. */
for (const mode of ["form", "recent-enter", "typed-taken", "quick", "number", "lost-race"] as const) {
  test(`a suspended fresh checkout uses one press: ${mode}`, async ({ page, request }) => {
    const fixture = await fleet(page, request);
    const gate = routeGate();
    const previewGate = routeGate();
    const historyGate = routeGate();
    let delayedHistoryReads = 0;
    const previous = await readPreferences(request);
    const templateName = `suspended-checkout-${mode}-${Date.now()}`;
    const stops: Array<() => Promise<void>> = [];
    let lag: ReturnType<typeof fixture.holdNextRead> | undefined;
    let resumes = 0;
    let retiring = false;
    let bodyFailed = true;
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
      const templated = mode === "quick" || mode === "number";
      // A fresh stack starts at revision0, which hides late-discovery bugs.
      // Force a newer authoritative revision, unknown at the shortcut press.
      const configRevision = (await history.json()).checkout_config_revision + (templated ? 1 : 0);
      const creates: any[] = [];
      const previewConnections: number[] = [];
      const freshTitles: string[] = [];
      if (mode === "recent-enter" || templated) {
        stops.push(await drainedRoute(page, "**/api/launch-history**", async (route) => {
          const response = await route.fetch();
          const json = await response.json();
          if (new URL(route.request().url()).searchParams.get("host") === String(fixture.remote.id)) {
            if (templated) {
              delayedHistoryReads++;
              await historyGate.wait();
              json.checkout_config_revision = configRevision;
            } else json.launches = [{ host: fixture.remote.id, cwd: "/tmp/previous-checkout", canonical_cwd: "/tmp/previous-checkout",
              github_repo: { source: "github.com", owner: "acme", name: "bar" },
              selection: { harness: "codex", model: null, effort: null, permissions: "guarded", workspace_trust: null },
              created_at: 1, creation_seq: 1 }];
          }
          await route.fulfill({ response, json });
        }));
      }
      if (templated) {
        const saved = await request.put(`/api/templates/${encodeURIComponent(templateName)}`, { data: {
          kind: "agent", agent: "codex", permissions: "guarded", host: fixture.remote.identity,
          destination: { github: "acme/bar" }, name: "review", name_kind: "base",
        } });
        expect(saved.ok(), await saved.text()).toBe(true);
      }
      await patchPreferences(request, {
        quick_launch_template: mode === "quick" ? templateName : null,
        template_shortcuts: mode === "number" ? { "1": templateName } : {},
      });
      stops.push(await drainedRoute(page, "**/api/github-repositories", async (route) => {
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
      }));
      stops.push(await drainedRoute(page, "**/api/github-checkout-preview", async (route) => {
        const response = await request.get("/api/hosts");
        const input = route.request().postDataJSON();
        previewConnections.push(input.expected_incarnation);
        if (input.expected_incarnation === fixture.remote.incarnation) {
          freshTitles.push(input.title);
          await previewGate.wait();
          if (mode === "typed-taken" || (templated && input.title === "review")) {
            await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.buildStamp },
              json: { error: "a directory with that checkout name already exists" } });
            return;
          }
        }
        const basename = creates.length ? "bar-2" : (input.title ? `bar-${input.title}` : "bar-1");
        await route.fulfill({
          status: 200,
          headers: { "x-farhelm-build": response.headers()["x-farhelm-build"] },
          json: {
            canonical_root: "/tmp",
            basename,
            cwd: `/tmp/${basename}`,
            config_revision: configRevision,
            host: String(input.host),
            incarnation: input.expected_incarnation,
            installation_identity: fixture.remote.identity,
          },
        });
      }));
      stops.push(await drainedRoute(page, "**/api/sessions", async (route) => {
        if (route.request().method() === "POST") {
          const body = route.request().postDataJSON();
          expect(body.expected_incarnation).toBe(fixture.remote.incarnation);
          expect(body.github_checkout.preview.incarnation).toBe(fixture.remote.incarnation);
          creates.push(body);
          await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.buildStamp,
            "x-farhelm-create-outcome": "definitely-unaccepted" },
            json: { error: mode === "lost-race" ? "checkout path taken; nothing was accepted" : "fresh create boundary" } });
        } else await route.continue();
      }));
      await page.goto("/");
      if (templated) {
        await expect(page.locator(".quick-launch-trigger")).toBeEnabled();
        if (mode === "quick") {
          const mac = await page.evaluate(() => /Mac/.test(navigator.platform));
          await page.keyboard.press(mac ? "Meta+p" : "Control+Shift+p");
        } else {
          await expect(page.locator('.template-number-trigger[data-number-key="1"]')).toBeEnabled();
          await page.keyboard.press("Control+Shift+Digit1");
        }
      } else {
        await page.locator(".new-session-button").click();
        const form = page.locator(".create-session-form");
        await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
        await chooseLauncherHost(form.getByRole("combobox", { name: "Host", exact: true }), String(fixture.remote.id));
        if (mode === "recent-enter") {
          // Enter applies and submits in one handler. There has deliberately
          // been no GitHub draft/preview in the preceding render.
          await expect(form.locator(".launch-composer-checkout-preview")).toHaveCount(0);
          const recent = form.locator(".launch-composer-recents button").filter({ hasText: "acme/bar" });
          await expect(recent).toHaveCount(1);
          await recent.focus();
          await expect(recent).toBeFocused();
          await recent.press("Enter");
        } else {
          const search = form.locator('.launch-composer-search input[role="combobox"]');
          await search.fill("gh:acme/bar");
          await expect(form.getByRole("option", { name: "Managed checkout: acme/bar", exact: true })).toBeVisible();
          await search.press("Enter");
          if (mode === "typed-taken") await form.getByLabel("Name (optional)").fill("taken");
          const preview = form.locator(".launch-composer-checkout-preview");
          await expect(preview).toContainText(mode === "typed-taken" ? "named after the session" : "picked automatically");
          await expect(preview.locator(".font-mono")).toHaveCount(0);
          await expect(preview).toHaveAttribute("data-preview-state", "ready");
          await expect(form.getByRole("textbox", { name: "Folder", exact: true })).toHaveValue("");
          await expect(form.locator(".create-session-submit")).not.toContainText("/tmp");
          await form.locator(".create-session-submit").click();
        }
      }
      const form = page.locator(".create-session-form");
      await expect.poll(() => resumes).toBe(1);
      if (templated) {
        await expect.poll(() => delayedHistoryReads, { message: "the shortcut pressed before this host's history completed" }).toBeGreaterThan(0);
        historyGate.release();
      }
      // Resume follows the accepted launch press for every entry path. The
      // pill owns its visible preparation while the retained draft stays busy.
      await expect(form).toBeHidden();
      await expect(form).not.toHaveAttribute("aria-modal", "true");
      const progress = page.locator(".launch-progress-pill .host-resume-progress");
      await expect(progress).toBeVisible();
      await expect(progress).toContainText("Resuming Sandbox test");
      await expect(progress).toContainText("the session starts when it’s ready");
      await expect(form).not.toContainText("The selected host is unavailable");
      expect(creates, "preparation cannot authorize a checkout from the new connection").toHaveLength(0);
      gate.release();
      await expect.poll(() => previewConnections, {
        message: "Resume's fresh connection reaches the held checkout preview",
      }).toContain(fixture.remote.incarnation);
      expect(creates, "a held current preview cannot be replaced by an old Ready offer").toHaveLength(0);
      // The fresh preview request proves Resume's helper finished. Its
      // retained presentation and token still belong to this accepted submit;
      // neither the stale displayed host nor the held preview owes a modal.
      await expect(progress).toBeVisible();
      await expect(form).toBeHidden();
      await expect(form).not.toHaveAttribute("aria-modal", "true");
      await expect(page.locator(".new-session-button")).toBeDisabled();
      expect(lag, "the displayed host reader remains held independently of Resume's helper").toBeDefined();
      lag!.release();
      previewGate.release();
      if (mode === "typed-taken") {
        await expect(form.getByRole("alert")).toContainText("a directory with that checkout name already exists");
        expect(creates).toHaveLength(0);
        expect(freshTitles).toEqual(["taken"]);
      } else {
        await expect(form).toBeVisible();
        await expect(form.getByRole("alert")).toContainText(mode === "lost-race" ? "nothing was accepted" : "fresh create boundary");
        if (mode === "lost-race") {
          await expect(form.locator(".launch-composer-checkout-preview")).toContainText("/tmp/bar-2");
        }
        expect(creates, "preview refreshes never retry a spent press").toHaveLength(1);
        if (templated) {
          expect(freshTitles).toContain("review-2");
          expect(creates[0].title).toBe("review-2");
        }
      }
      await expect(progress).toHaveCount(0);
      expect(resumes).toBe(1);
      bodyFailed = false;
    } finally {
      retiring = true;
      gate.release();
      previewGate.release();
      lag?.release();
      historyGate.release();
      await cleanupOwnedSteps(bodyFailed, [
        ["resume route", stop],
        ...stops.map((stop, index): [string, () => Promise<void>] => [`checkout route ${index}`, stop]),
        ["shortcut choices", () => patchPreferences(request, {
          quick_launch_template: previous.quick_launch_template ?? null, template_shortcuts: previous.template_shortcuts ?? {},
        })],
        ["checkout template", async () => {
          const removed = await request.delete(`/api/templates/${encodeURIComponent(templateName)}`);
          expect(removed.ok() || removed.status() === 404).toBe(true);
        }],
      ]);
    }
  });
}

/** The submit that continues a quick launch after its wake can be refused with
 * no error message, leaving the launcher's own prerequisite text to explain
 * it. Here the woken host stops being available for launch while the fresh
 * checkout preview loads. That launcher used to stay hidden while Return made
 * it the page's modal dialog, so an invisible launcher swallowed every click
 * and key until Escape. It must come back visibly, with its draft and the
 * reason, and without having sent a create. */
test("a quick launch refused without an error after its wake returns visibly", async ({ page, request }) => {
  await page.addInitScript(() => {
    Object.defineProperty(Navigator.prototype, "platform", { get: () => "Linux x86_64" });
  });
  const fixture = await fleet(page, request);
  const previewGate = routeGate();
  const previous = await readPreferences(request);
  const templateName = `silent-refusal-after-wake-${Date.now()}`;
  const stops: Array<() => Promise<void>> = [];
  const creates: unknown[] = [];
  let resumes = 0;
  let freshPreviews = 0;
  let configRevision = 0;
  let bodyFailed = true;
  stops.push(await drainedRoute(page, `**/api/hosts/${fixture.remote.id}/resume`, async (route) => {
    resumes++;
    await fixture.publishState({ phase: "tensorlake-changing", direction: "resuming", requested_at: Date.now() });
    await fixture.publishState();
    await route.fulfill({ status: 200, headers: { "x-farhelm-build": fixture.buildStamp }, json: {} });
  }));
  stops.push(await drainedRoute(page, "**/api/github-repositories", async (route) => {
    const input = route.request().postDataJSON();
    await route.fulfill({
      status: 200,
      headers: { "x-farhelm-build": fixture.buildStamp },
      json: {
        host: String(input.host),
        incarnation: input.expected_incarnation,
        installation_identity: fixture.remote.identity,
        repos: [],
        truncated: true,
        scan_error: "fixture scan unavailable",
      },
    });
  }));
  stops.push(await drainedRoute(page, "**/api/github-checkout-preview", async (route) => {
    const input = route.request().postDataJSON();
    if (input.expected_incarnation === fixture.remote.incarnation) {
      freshPreviews++;
      await previewGate.wait();
    }
    await route.fulfill({
      status: 200,
      headers: { "x-farhelm-build": fixture.buildStamp },
      json: {
        canonical_root: "/tmp",
        basename: "bar-review",
        cwd: "/tmp/bar-review",
        config_revision: configRevision,
        host: String(input.host),
        incarnation: input.expected_incarnation,
        installation_identity: fixture.remote.identity,
      },
    });
  }));
  stops.push(await drainedRoute(page, "**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    creates.push(route.request().postDataJSON());
    await route.fulfill({ status: 409, headers: { "x-farhelm-build": fixture.buildStamp,
      "x-farhelm-create-outcome": "definitely-unaccepted" }, json: { error: "no create was expected" } });
  }));
  try {
    const history = await request.get(`/api/launch-history?host=${fixture.remote.id}`);
    expect(history.ok()).toBe(true);
    configRevision = (await history.json()).checkout_config_revision;
    const saved = await request.put(`/api/templates/${encodeURIComponent(templateName)}`, { data: {
      kind: "agent", agent: "codex", permissions: "guarded", host: fixture.remote.identity,
      destination: { github: "acme/bar" }, name: "review", name_kind: "base",
    } });
    expect(saved.ok(), await saved.text()).toBe(true);
    await patchPreferences(request, { quick_launch_template: templateName });
    await page.goto("/");
    await expect(page.locator(".quick-launch-trigger")).toBeEnabled();
    await page.keyboard.press("Control+Shift+p");
    const form = page.locator(".create-session-form");
    await expect.poll(() => freshPreviews, {
      message: "the wake succeeded and the form asked for a preview on the fresh connection",
    }).toBe(1);
    expect(resumes).toBe(1);
    // Premise: the launch is still hidden preparation, waiting on that preview.
    await expect(form).toBeHidden();
    await expect(form.locator(".launch-composer-checkout-preview")).toHaveAttribute("data-preview-state", "pending");
    await fixture.publishState({ phase: "unreachable-reprobing", cause: "transport-failure", last_error: "fixture host is offline" });
    // The hidden form's host choice names the phase once the page has read it.
    await expect(form.getByRole("combobox", { name: "Host", exact: true, includeHidden: true }))
      .toContainText("Unreachable, retrying");
    previewGate.release();
    await expect(form).toBeVisible();
    await expect(form).toHaveAttribute("aria-modal", "true");
    await expect(form).toContainText("The selected host is unavailable; choose a connected host before launching");
    // The refused attempt is explained by the launcher's prerequisite, not by
    // an error message, which would have revealed the launcher on its own.
    await expect(form.getByRole("alert")).toHaveText(/^\W*Not launched\. Choose a connected host before launching$/);
    // The draft and its fresh preview come back with it.
    await expect(form.locator(".launch-composer-checkout-preview")).toContainText("/tmp/bar-review");
    await expect(page.locator(".launch-progress-pill .host-resume-progress")).toHaveCount(0);
    expect(creates, "a refused continuation sends no create").toHaveLength(0);
    bodyFailed = false;
  } finally {
    previewGate.release();
    await cleanupOwnedSteps(bodyFailed, [
      ...stops.map((stop, index): [string, () => Promise<void>] => [`silent refusal route ${index}`, stop]),
      ["quick-launch choice", () => patchPreferences(request, {
        quick_launch_template: previous.quick_launch_template ?? null,
      })],
      ["quick-launch template", async () => {
        const removed = await request.delete(`/api/templates/${encodeURIComponent(templateName)}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }],
    ]);
  }
});

/** Suspension consent needs the same readable frame as other host questions.
 * Capture the real dialog and details in each palette, prove its actions are
 * outside the scroll body, and keep every dismissal free of lifecycle calls. */
for (const palette of ["dark", "light"] as const) {
  test(
    `suspend confirmation and sandbox details use the standard frame in ${palette}`,
    async ({ page, request }, info) => {
      await page.emulateMedia({ colorScheme: palette });
      await page.setViewportSize({ width: 1280, height: 800 });
      const fixture = await fleet(page, request, false);
      const session = await createSession(request, { title: `suspend-frame-${palette}`, host: fixture.remote.id });
      let reads = 0;
      let bodyFailed = false;
      const stopActivity = await drainedRoute(page, "**/api/sessions?**", async (route) => {
        if (new URL(route.request().url()).searchParams.get("host") !== String(fixture.remote.id)) {
          return route.continue();
        }
        const response = await route.fetch();
        expect(response.ok(), "the consent fixture has a readable host listing").toBe(true);
        const json = await response.json();
        expect(json.sessions.some((row: any) => row.id === session.id), "the owned session participates in the guard")
          .toBe(true);
        json.sessions = json.sessions.map((row: any) =>
          row.id === session.id ? { ...row, stale: false, status: { state: "unknown" } } : row
        );
        reads++;
        await route.fulfill({ response, json });
      });
      try {
        await page.goto("/");
        await expect(page.locator("html")).toHaveAttribute("data-appearance", palette);
        await openHostsPanel(page);
        await page.getByRole("checkbox", { name: "Details", exact: true }).check();
        const row = page.locator(`.host-row[data-host-id="${fixture.remote.id}"]`);
        await expect(row).toContainText("Connected");
        const details = row.locator(".host-tensorlake-detail");
        await expect(details).toContainText("Sandbox: farhelm-host-test · Recipe: test · 2 CPUs · 4 GB memory");
        await expect(details).toHaveCSS("font-size", "12px");
        await expect(details).toHaveCSS("padding-left", "12px");
        await info.attach(`sandbox-details-${palette}`, {
          body: await page.locator(".hosts-panel").screenshot(),
          contentType: "image/png",
        });
        /** Each dismissal retires its question; reopening must read activity
         * again and install Cancel focus for that new modal opening. */
        const openQuestion = async () => {
          await row.locator(".host-row-menu").click();
          await row.getByRole("menuitem", { name: "Suspend", exact: true }).click();
          const dialog = page.getByRole("dialog", { name: "Suspend Sandbox test?", exact: true });
          await expect(dialog).toBeVisible();
          await expect(dialog.getByRole("button", { name: "Cancel", exact: true })).toBeFocused();
          return dialog;
        };
        const dialog = await openQuestion();
        expect(reads, "the unknown session made the consent question necessary").toBeGreaterThan(0);
        const header = dialog.locator(":scope > .dialog-header");
        const body = dialog.locator(":scope > .host-settings-body");
        const footer = dialog.locator(":scope > .dialog-footer");
        await expect(header.getByRole("heading", { level: 2 })).toHaveText("Suspend Sandbox test?");
        await expect(body).toContainText("continues when you resume the host");
        await expect(body.getByRole("button")).toHaveCount(0);
        await expect(footer.getByRole("button", { name: "Cancel", exact: true })).toBeVisible();
        await expect(footer.getByRole("button", { name: "Suspend", exact: true })).toBeVisible();
        const frameBox = await dialog.boundingBox();
        const footerBox = await footer.boundingBox();
        expect(frameBox).not.toBeNull();
        expect(footerBox).not.toBeNull();
        expect(footerBox!.y + footerBox!.height).toBeLessThanOrEqual(frameBox!.y + frameBox!.height + 1);
        expect(footerBox!.y + footerBox!.height).toBeLessThanOrEqual(800);
        await page.keyboard.press("Tab");
        await expect(footer.getByRole("button", { name: "Suspend", exact: true })).toBeFocused();
        await info.attach(`suspend-confirmation-${palette}`, {
          body: await dialog.screenshot(),
          contentType: "image/png",
        });
        await footer.getByRole("button", { name: "Cancel", exact: true }).click();
        await expect(dialog).toHaveCount(0);
        await openQuestion();
        await page.keyboard.press("Escape");
        await expect(dialog).toHaveCount(0);
        await openQuestion();
        await header.getByRole("button", { name: "Close", exact: true }).click();
        await expect(dialog).toHaveCount(0);
        expect(fixture.calls, "Cancel, Escape and Close never authorize suspension").toEqual([]);
      } catch (error) {
        bodyFailed = true;
        throw error;
      } finally {
        await cleanupOwnedSteps(bodyFailed, [["activity route", stopActivity], [
          "owned session",
          () => cleanupSession(request, session.id),
        ]]);
      }
    },
  );
}
