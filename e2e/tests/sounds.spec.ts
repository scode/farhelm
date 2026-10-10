// Use real sessions and stamped helm replies, but control status/approval data
// and record actual Web Audio oscillator starts. This proves the UI wiring
// without depending on hardware, vendor agents or heuristic status timing.
import { expect, newObservedContext, test } from "./helpers/evidence";
import { type Page, type APIRequestContext } from "@playwright/test";
import { cleanupSession, createSession, listHosts, localHostId, observeFeedReaders, pinAutoSelect, readFeedReaders, resetPreferences, stubFeed, waitForFeedReadersSettled } from "./helpers/fleet";

/** The recorder observes the shipped decision after it runs, never substitutes it. */
async function recordAudio(page: Page) {
  await page.addInitScript(() => {
    const target = window as any;
    target.__soundStarts = [];
    target.__soundSnapshot = null;
    let sounds: any;
    Object.defineProperty(window, "farhelmSounds", {
      configurable: true,
      get: () => sounds,
      set(value) {
        const observe = value.observe;
        value.observe = (...args: any[]) => {
          observe(...args);
          target.__soundSnapshot = args[1];
        };
        sounds = value;
      },
    });
    class RecordingAudioContext {
      state = "running";
      currentTime = 0;
      destination = {};
      resume() { return Promise.resolve(); }
      createGain() { return { gain: { value: 0, setValueAtTime() {}, exponentialRampToValueAtTime() {} }, connect() {} }; }
      createOscillator() {
        const oscillator = {
          frequency: { value: 0 }, type: "", connect() {}, stop() {},
          start() { target.__soundStarts.push(oscillator.frequency.value); },
        };
        return oscillator;
      }
    }
    target.AudioContext = RecordingAudioContext;
  });
}

/** A successful sound application proves it observed the intended sidebar scope. */
async function applied(page: Page, id: string, status: string | null, approval?: string) {
  await expect.poll(() => page.evaluate(({ id, status, approval }) => {
    const snapshot = (window as any).__soundSnapshot;
    return !!snapshot && (status === null ? !(id in snapshot.statuses) : snapshot.statuses[id] === status) &&
      (!approval || snapshot.approvals.some((ask: any) => ask.id === approval));
  }, { id, status, approval }), { timeout: 20_000, message: "sound reader must apply the controlled snapshot" }).toBe(true);
}

/** Read starts from Web Audio's boundary, including a bell's fundamental and overtones. */
async function starts(page: Page): Promise<number[]> {
  return page.evaluate(() => (window as any).__soundStarts);
}

/** Isolate changes to attention data while retaining the real helm's build stamp and filters. */
async function fixture(page: Page, request: APIRequestContext, created: string[]) {
  const local = await localHostId(request);
  const remote = (await listHosts(request)).find((host) => host.id !== local);
  expect(remote, "two-host substrate is required for the filter-scope proof").toBeTruthy();
  const session = await createSession(request, { title: `sound-local-${Date.now()}`, host: local, invocation: "sleep 300" });
  created.push(session.id);
  const other = await createSession(request, { title: `sound-remote-${Date.now()}`, host: remote!.id, invocation: "sleep 300" });
  created.push(other.id);
  const data = { status: "running", approvals: [] as any[] };
  await observeFeedReaders(page);
  await recordAudio(page);
  await pinAutoSelect(page, other.id);
  await page.route((url) => url.pathname === "/api/sessions", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    for (const row of body.sessions) if (row.id === session.id) row.status = { state: data.status };
    await route.fulfill({ response, json: body });
  });
  await page.route((url) => url.pathname === "/api/approvals", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, json: { approvals: data.approvals } });
  });
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  const ask = (id: string) => ({
    id, host_id: local, host_name: "fixture host",
    session: { id: session.id, title: session.title, host_name: "fixture host" },
    action: { kind: "stop", target: { id: other.id, title: other.title, host_name: "fixture host" } },
    expires_at_ms: Date.now() + 600_000,
  });
  return { session, other, remote: remote!, data, feed, ask };
}

test.describe("device sounds", () => {
  const created: string[] = [];
  test.afterEach(async ({ request }) => {
    while (created.length) await cleanupSession(request, created.pop()!);
    await resetPreferences(request);
  });

  /** A hidden session stays quiet, revealing it seeds history, and approvals
   * still ring outside that view. Settings apply before event priority.
   */
  test("status sounds follow the current view while approvals ignore its filter", async ({ page, request }, testInfo) => {
    const f = await fixture(page, request, created);
    await page.goto("/");
    await f.feed.waitForConnection(1);
    await applied(page, f.session.id, "running");
    expect(await starts(page)).toEqual([]);
    f.data.status = "waiting"; f.feed.notify(2);
    await applied(page, f.session.id, "waiting");
    const bell = [784, 1568, 2352, 1047, 2094, 3141];
    expect(await starts(page)).toEqual(bell);
    await page.locator(".filter-host").selectOption(String(f.remote.id));
    await expect(page.locator(`[data-session-id="${f.other.id}"]`)).toBeVisible();
    await expect(page.locator(`[data-session-id="${f.session.id}"]`)).toHaveCount(0);
    await applied(page, f.session.id, null);
    f.data.status = "running"; f.feed.notify(3);
    await waitForFeedReadersSettled(page);
    f.data.status = "waiting"; f.feed.notify(4);
    await waitForFeedReadersSettled(page);
    expect(await starts(page)).toEqual(bell);
    await page.locator(".filter-host").selectOption("");
    await expect(page.locator(`[data-session-id="${f.session.id}"]`)).toBeVisible();
    await applied(page, f.session.id, "waiting");
    expect(await starts(page)).toEqual(bell);
    f.data.status = "running"; f.feed.notify(5);
    await applied(page, f.session.id, "running");
    f.data.status = "waiting"; f.feed.notify(6);
    await applied(page, f.session.id, "waiting");
    expect(await starts(page)).toEqual([...bell, ...bell]);

    await page.getByRole("button", { name: "settings", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "settings", exact: true });
    await expect(dialog.getByRole("checkbox", { name: "session waiting", exact: true })).toBeChecked();
    await expect(dialog.getByRole("checkbox", { name: "approval request", exact: true })).toBeChecked();
    await expect(dialog.getByRole("checkbox", { name: "turn finished", exact: true })).not.toBeChecked();
    await dialog.screenshot({ path: testInfo.outputPath("sound-settings.png") });
    await dialog.getByRole("checkbox", { name: "session waiting", exact: true }).uncheck();
    await dialog.getByRole("checkbox", { name: "turn finished", exact: true }).check();
    await dialog.getByRole("button", { name: "close", exact: true }).click();
    await page.locator(".filter-host").selectOption(String(f.remote.id));
    await applied(page, f.session.id, null);
    f.data.approvals = [f.ask("sound-approval")]; f.feed.notify(7);
    await applied(page, f.session.id, null, "sound-approval");
    expect(await starts(page)).toEqual([...bell, ...bell, 880, 1760, 2640, 698, 1396, 2094]);
    await page.locator(".filter-host").selectOption("");
    await applied(page, f.session.id, "waiting", "sound-approval");
    f.data.status = "running"; f.feed.notify(8);
    await applied(page, f.session.id, "running");
    f.data.status = "idle"; f.feed.notify(9);
    await applied(page, f.session.id, "idle");
    expect((await starts(page)).at(-1)).toBe(523);

    await page.reload();
    await f.feed.waitForConnection(2);
    await applied(page, f.session.id, "idle", "sound-approval");
    expect(await starts(page)).toEqual([]);
    await page.getByRole("button", { name: "settings", exact: true }).click();
    await expect(dialog.getByRole("checkbox", { name: "session waiting", exact: true })).not.toBeChecked();
    await expect(dialog.getByRole("checkbox", { name: "turn finished", exact: true })).toBeChecked();
  });

  /** The active selected session stays quiet, but moving attention away permits sound. */
  test("active open session is quiet for waiting and its approval", async ({ page, request }) => {
    const f = await fixture(page, request, created);
    await pinAutoSelect(page, f.session.id);
    await page.goto("/");
    await f.feed.waitForConnection(1);
    await applied(page, f.session.id, "running");
    await expect(page.locator(`[data-session-id="${f.session.id}"]`)).toHaveClass(/\bselected\b/);
    await page.bringToFront();
    await page.locator(".app-settings-toggle").focus();
    await expect(page.locator("html")).toHaveAttribute("data-window-active", "true");
    f.data.status = "waiting"; f.data.approvals = [f.ask("quiet-approval")]; f.feed.notify(2);
    await applied(page, f.session.id, "waiting", "quiet-approval");
    expect(await starts(page)).toEqual([]);
    await page.locator(`[data-session-id="${f.other.id}"]`).click();
    await expect(page.locator(`[data-session-id="${f.other.id}"]`)).toHaveClass(/\bselected\b/);
    f.data.status = "running"; f.feed.notify(3);
    await applied(page, f.session.id, "running");
    f.data.status = "waiting"; f.feed.notify(4);
    await applied(page, f.session.id, "waiting");
    expect((await starts(page))[0]).toBe(784);
  });

  /** Initial waiting and approval data are silent even though autoplay is available. */
  test("first read only records existing attention", async ({ page, request }) => {
    const f = await fixture(page, request, created);
    f.data.status = "waiting"; f.data.approvals = [f.ask("already-waiting")];
    await page.goto("/");
    await f.feed.waitForConnection(1);
    await applied(page, f.session.id, "waiting", "already-waiting");
    expect(await starts(page)).toEqual([]);
    f.data.approvals.push(f.ask("new-request")); f.feed.notify(2);
    await applied(page, f.session.id, "waiting", "new-request");
    expect((await starts(page))[0]).toBe(880);
  });

  /** Accepted rows, rather than approval latency, define a view's baseline.
   * Later changes in that view ring; returning to an old view seeds it anew.
   */
  for (const returning of [false, true]) {
    test(returning ? "returning view stays quiet when approval reads span both filter changes" : "new view keeps later status transitions while approval reads are held", async ({ page, request }) => {
      const f = await fixture(page, request, created);
      await page.goto("/");
      await f.feed.waitForConnection(1);
      await applied(page, f.session.id, "running");
      f.data.status = "waiting"; f.feed.notify(2);
      await applied(page, f.session.id, "waiting");
      const bell = [784, 1568, 2352, 1047, 2094, 3141];
      expect(await starts(page)).toEqual(bell);
      f.data.status = "running"; f.feed.notify(3);
      await applied(page, f.session.id, "running");
      await waitForFeedReadersSettled(page);

      let release!: () => void;
      const held = new Promise<void>((resolve) => { release = resolve; });
      let heldReads = 0;
      await page.route((url) => url.pathname === "/api/approvals", async (route) => {
        const response = await route.fetch();
        heldReads++;
        await held;
        await route.fulfill({ response, json: { approvals: f.data.approvals } });
      });
      try {
        f.feed.notify(4);
        await expect.poll(() => heldReads, { message: "approval read must be held before changing view" }).toBeGreaterThan(0);
        await expect.poll(async () => {
          const sounds = (await readFeedReaders(page)).find((reader) => reader.role === "sounds");
          return sounds?.readers[0].running;
        }, { message: "the sound reader itself must span both view changes" }).toBe(true);
        const local = await localHostId(request);
        await page.locator(".filter-host").selectOption(String(returning ? f.remote.id : local));
        if (returning) {
          await expect(page.locator(`[data-session-id="${f.session.id}"]`)).toHaveCount(0);
          await expect(page.locator(`[data-session-id="${f.other.id}"]`)).toBeVisible();
        } else {
          await expect(page.locator(`[data-session-id="${f.session.id}"] .status-badge.running`)).toHaveText("running");
          await expect(page.locator(`[data-session-id="${f.other.id}"]`)).toHaveCount(0);
        }
        f.data.status = "waiting";
        if (returning) await page.locator(".filter-host").selectOption("");
        else f.feed.notify(5);
        await expect(page.locator(`[data-session-id="${f.session.id}"] .status-badge.waiting`)).toHaveText("waiting");
        release();
        await applied(page, f.session.id, "waiting");
        await waitForFeedReadersSettled(page);
        expect(await starts(page)).toEqual(returning ? bell : [...bell, ...bell]);
      } finally {
        release();
      }
    });
  }

  /** The initial sidebar can establish its baseline before approvals respond.
   * A later waiting edge must ring while existing approvals remain first-load
   * context, rather than the whole snapshot being discarded as initial state.
   */
  test("initial status baseline does not wait for the first approval reply", async ({ page, request }) => {
    const f = await fixture(page, request, created);
    f.data.approvals = [f.ask("initial-held-request")];
    let release!: () => void;
    const held = new Promise<void>((resolve) => { release = resolve; });
    let heldReads = 0;
    await page.route((url) => url.pathname === "/api/approvals", async (route) => {
      const response = await route.fetch();
      heldReads++;
      await held;
      await route.fulfill({ response, json: { approvals: f.data.approvals } });
    });
    try {
      await page.goto("/");
      await f.feed.waitForConnection(1);
      await expect(page.locator(`[data-session-id="${f.session.id}"] .status-badge.running`)).toHaveText("running");
      await expect.poll(() => heldReads).toBeGreaterThan(0);
      await expect.poll(async () => {
        const sounds = (await readFeedReaders(page)).find((reader) => reader.role === "sounds");
        return sounds?.readers[0].running === true && sounds.baseline === false;
      }, { message: "first approval reply must still be held after initial rows are accepted" }).toBe(true);
      f.data.status = "waiting"; f.feed.notify(2);
      await expect(page.locator(`[data-session-id="${f.session.id}"] .status-badge.waiting`)).toHaveText("waiting");
      expect(await starts(page)).toEqual([]);
      release();
      await applied(page, f.session.id, "waiting", "initial-held-request");
      await waitForFeedReadersSettled(page);
      expect(await starts(page)).toEqual([784, 1568, 2352, 1047, 2094, 3141]);
    } finally {
      release();
    }
  });

  /** A trusted touch release resumes a real browser audio context. Deliberate
   * suspension gives both engines the same refusal premise even where headless
   * autoplay is allowed; this proves resume wiring, not a physical phone's policy.
   */
  test("touch input resumes real audio and synthetic release stays silent", async ({ browser, timeline }) => {
    const context = await newObservedContext(browser, timeline, { hasTouch: true });
    try {
      await context.addInitScript(() => {
        const target = window as any;
        const NativeAudioContext = target.AudioContext || target.webkitAudioContext;
        target.__realSoundStarts = 0;
        target.AudioContext = class extends NativeAudioContext {
          constructor() { super(); target.__realSoundContext = this; }
          createOscillator() {
            const oscillator = super.createOscillator();
            const start = oscillator.start.bind(oscillator);
            oscillator.start = (...args: any[]) => { target.__realSoundStarts++; start(...args); };
            return oscillator;
          }
        };
      });
      const page = await context.newPage();
      await page.goto("/");
      await expect.poll(() => page.evaluate(() => !!(window as any).__realSoundContext)).toBe(true);
      await page.evaluate(async () => { await (window as any).__realSoundContext.suspend(); });
      const observeWaiting = () => page.evaluate(() => {
        (window as any).farhelmSounds.observe({ statuses: { fixture: "running" }, approvalIds: [] },
          { statuses: { fixture: "waiting" }, approvals: [] }, null);
      });
      await observeWaiting();
      expect(await page.evaluate(() => (window as any).__realSoundStarts)).toBe(0);
      await page.evaluate(() => window.dispatchEvent(new PointerEvent("pointerup", { pointerType: "touch" })));
      expect(await page.evaluate(() => (window as any).__realSoundContext.state)).toBe("suspended");
      await page.getByRole("button", { name: "settings", exact: true }).tap();
      await expect.poll(() => page.evaluate(() => (window as any).__realSoundContext.state)).toBe("running");
      await observeWaiting();
      expect(await page.evaluate(() => (window as any).__realSoundStarts)).toBe(6);
    } finally {
      await context.close();
    }
  });
});
