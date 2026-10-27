// Browser evidence for session notifications (SPEC.md, Status): the bell on a
// sidebar row, its loud and quiet states, its list, read-on-close and clear.
//
// The Playwright stack runs a plain build without the supervisor's test
// seams, so nothing here can make a real supervisor record a notification
// (the 65-second tripwire alone would cost a minute per test). The listing is
// stubbed instead: real sessions come from the real helm, and this spec
// injects notifications into their rows and answers the two notification
// routes itself, keeping the read and cleared marks the way the helm does.
// The supervisor's recording and the helm's marks have their own Rust tests.
import { setCompact } from "./helpers/list-header";
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { Locator, Page, Route } from "@playwright/test";
import { cleanupSession, createSession, forceBuildSkew, openRowMenu, patchPreferences, pinAutoSelect, resetPreferences, stubFeed } from "./helpers/fleet";
import { replayRecord, waitForReplayReveal, waitForSessionMounted, waitForSessionReady } from "./helpers/terminal-readiness";

interface StubNotification { seq: number; at: number; text: string; resolved?: boolean }

/**
 * The helm's half of the feature, for one session: its notifications and the
 * two marks. `writes` records every mark request and its addressed session,
 * so a request aimed at another real session cannot count as a correct mark.
 */
class NotificationStub {
  readThrough = 0;
  clearedThrough = 0;
  writes: { session: string; route: "read" | "cleared"; through: number }[] = [];
  constructor(public notifications: StubNotification[]) {}

  /** What the helm would put on the row: uncleared entries, newest first. */
  row(): { notifications: StubNotification[]; notifications_read_through: number } {
    return {
      notifications: this.notifications
        .filter((n) => n.seq > this.clearedThrough)
        .sort((a, b) => b.seq - a.seq),
      notifications_read_through: this.readThrough,
    };
  }
}

/** Serve one session's notifications and mutate only that session's marks.
 * Record other destinations too: a real wrong-session route can succeed,
 * but must neither change this stub nor disappear from the test's evidence. */
async function serveNotifications(page: Page, id: string, stub: NotificationStub): Promise<void> {
  await page.route((url) => url.pathname === "/api/sessions", async (route: Route) => {
    if (route.request().method() !== "GET") return route.continue();
    const response = await route.fetch();
    const body = await response.json();
    for (const row of body.sessions) {
      if (row.id === id) Object.assign(row, stub.row());
    }
    await route.fulfill({ response, json: body });
  });
  await page.route(
    (url) => /^\/api\/sessions\/[^/]+\/notifications\/(read|cleared)$/.test(url.pathname),
    async (route: Route) => {
      const which = route.request().url().endsWith("/cleared") ? "cleared" : "read";
      const session = decodeURIComponent(new URL(route.request().url()).pathname.split("/")[3]);
      const through = (route.request().postDataJSON() as { through: number }).through;
      stub.writes.push({ session, route: which, through });
      if (session === id) {
        stub.readThrough = Math.max(stub.readThrough, through);
        if (which === "cleared") stub.clearedThrough = Math.max(stub.clearedThrough, through);
      }
      // The real helm still answers (a no-op there, since it holds no
      // notifications for this session), so the reply carries its build
      // stamp: a reply without one latches the page's build-skew notice and
      // withdraws its feed.
      const response = await route.fetch();
      await route.fulfill({ response, json: {} });
    },
  );
}

/** Return the rendered list row for one stable session identity. */
function row(page: Page, id: string): Locator {
  return page.locator(`[data-session-id="${id}"]`);
}

const NOW = Math.floor(Date.now() / 1000);

/** Drain this suite's ownership before any release can fail. Inject only its
 * three release operations so failure tests exercise the same orchestration
 * as the real hook, including restoration after a route or session failure. */
async function cleanupNotifications(created: string[], bodyFailed: boolean, cleanup: {
  routes(): Promise<unknown>;
  session(id: string): Promise<unknown>;
  preferences(): Promise<unknown>;
}) {
  await cleanupOwnedSteps(bodyFailed, [
    ["routes", cleanup.routes],
    ...created.splice(0).reverse().map((id): [string, () => Promise<unknown>] => [
      `session ${id}`, () => cleanup.session(id),
    ]),
    ["preferences", cleanup.preferences],
  ]);
}

/** Earlier failures must not strand later sessions or shared preferences.
 * Inject both route and session failures, with and without a pending body
 * exception, to distinguish all releases from an ordinary successful teardown. */
test("notification cleanup attempts every release and preserves a body failure", async () => {
  for (const bodyFailed of [false, true]) {
    for (const failAt of ["routes", "two"]) {
      const created = ["one", "two"];
      const attempts: string[] = [];
      const secondary = new Error("injected cleanup failure");
      const primary = new Error("original body failure");
      const release = async (name: string) => {
        attempts.push(name);
        if (name === failAt) throw secondary;
      };
      let thrown: unknown;
      try {
        try {
          if (bodyFailed) throw primary;
        } finally {
          await cleanupNotifications(created, bodyFailed, {
            routes: () => release("routes"),
            session: release,
            preferences: () => release("preferences"),
          });
        }
      } catch (error) {
        thrown = error;
      }
      expect(attempts).toEqual(["routes", "two", "one", "preferences"]);
      expect(created).toEqual([]);
      if (bodyFailed) expect(thrown).toBe(primary);
      else {
        expect(thrown).toBeInstanceOf(AggregateError);
        expect((thrown as AggregateError).errors[0].cause).toBe(secondary);
      }
    }
  }
});

test.describe("session notifications", () => {
  const created: string[] = [];
  // The compact choice and the auto-selected row are helm-wide preferences
  // that outlive the page, so a test that changes them must not hand them to
  // the next spec.
  test.afterEach(async ({ page, request }, testInfo) => {
    // A successful read or clear now requests a fresh listing, so a test can
    // end while that read is still inside a listing stub; its response is
    // disposed with the page and would fail the finished test.
    await cleanupNotifications(created, testInfo.status !== testInfo.expectedStatus, {
      routes: () => page.unrouteAll({ behavior: "ignoreErrors" }),
      session: (id) => cleanupSession(request, id),
      preferences: () => resetPreferences(request),
    });
  });

  /** A layout dismissal must keep keyboard work at the bell, without
   * reclaiming focus after the user has chosen a different control. The
   * first Tab after closure proves no recovery key was swallowed. */
  test("automatic notification dismissal returns only the list's focus", async ({ page, request }) => {
    const session = await createSession(request, { title: `dismiss-focus-${Date.now()}` });
    created.push(session.id);
    await serveNotifications(page, session.id, new NotificationStub([{ seq: 1, at: NOW, text: "keep focus nearby" }]));
    await pinAutoSelect(page, session.id);
    await page.goto("/");
    await waitForSessionReady(page, session.id);
    const bell = row(page, session.id).locator(".session-row-bell");
    await bell.click();
    const panel = page.locator(".session-bell-panel");
    await expect(panel).toBeFocused();
    const viewport = page.viewportSize()!;
    const sidebarHeight = (await page.locator(".app-sidebar").boundingBox())!.height;
    await page.setViewportSize({ ...viewport, height: viewport.height - 40 });
    await expect.poll(async () => (await page.locator(".app-sidebar").boundingBox())!.height).toBeLessThan(sidebarHeight);
    await expect(panel).toHaveCount(0);
    await expect(bell).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(bell).not.toBeFocused();
    expect(await page.evaluate(() => document.activeElement !== document.body)).toBe(true);

    await bell.click();
    await expect(panel).toBeFocused();
    const elsewhere = page.getByRole("button", { name: "New session", exact: true });
    await elsewhere.focus();
    await expect(elsewhere).toBeFocused();
    await page.setViewportSize(viewport);
    await expect(panel).toHaveCount(0);
    await expect(elsewhere).toBeFocused();
  });

  /** Local deletion must retire a bell's measured position before losing the
   * old row order. Holding all later listings isolates that local path; a
   * quiet feed, stable selection and sidebar size exclude other dismissals. */
  test("a local delete closes the notification list on a row it moves", async ({ page, request }) => {
    const stamp = Date.now();
    const a = await createSession(request, { title: `bell-delete-${stamp}-a`, invocation: "sleep 300" });
    created.push(a.id);
    const b = await createSession(request, { title: `bell-delete-${stamp}-b`, invocation: "sleep 300" });
    created.push(b.id);
    const c = await createSession(request, { title: `bell-delete-${stamp}-c`, invocation: "sleep 300" });
    created.push(c.id);
    await patchPreferences(request, { list_sort: "title" });
    await serveNotifications(page, b.id, new NotificationStub([{ seq: 1, at: NOW, text: "anchored to B" }]));
    const feed = await stubFeed(page);
    await pinAutoSelect(page, c.id);
    await page.goto("/");
    await waitForSessionReady(page, c.id);
    await expect(row(page, a.id)).toBeVisible();
    await expect(row(page, b.id)).toBeVisible();
    await expect.poll(() => feed.connections()).toBeGreaterThan(0);
    let releaseDelete = () => {};
    let releaseListings = () => {};
    const deleteGate = new Promise<void>((resolve) => { releaseDelete = resolve; });
    const listingGate = new Promise<void>((resolve) => { releaseListings = resolve; });
    let deleteReached = false;
    try {
      await page.route((url) => url.pathname === "/api/sessions", async (route) => {
        if (route.request().method() === "GET") await listingGate;
        await route.fallback();
      });
      await page.route(`**/api/sessions/${encodeURIComponent(a.id)}`, async (route) => {
        if (route.request().method() === "DELETE") {
          deleteReached = true;
          await deleteGate;
        }
        await route.fallback();
      });
      await openRowMenu(row(page, a.id));
      await row(page, a.id).locator(".session-row-delete").click();
      await row(page, a.id).locator(".confirm-delete").click();
      await expect.poll(() => deleteReached).toBe(true);
      // Finish A's menu handback before opening B; switching both surfaces
      // in one click would exercise their separate focus-transfer race.
      await row(page, a.id).locator(".session-row-menu").click();
      await expect(row(page, a.id).locator(".session-row-menu-flyout")).toHaveCount(0);
      await expect(row(page, a.id).locator(".session-row-menu")).toBeFocused();
      const bell = row(page, b.id).locator(".session-row-bell");
      await bell.click();
      const panel = page.locator(".session-bell-panel");
      await expect(panel).toBeFocused();
      await expect(page.locator(".session-bell-flyout")).toHaveCSS("opacity", "1");
      const before = (await row(page, b.id).boundingBox())!;
      expect((await row(page, a.id).boundingBox())!.y).toBeLessThan(before.y);
      const sidebarHeight = (await page.locator(".app-sidebar").boundingBox())!.height;
      releaseDelete();
      await expect(row(page, a.id)).toHaveCount(0, { timeout: 20_000 });
      expect((await row(page, b.id).boundingBox())!.y).toBeLessThan(before.y);
      expect((await page.locator(".app-sidebar").boundingBox())!.height).toBe(sidebarHeight);
      await expect(panel).toHaveCount(0);
      await expect(bell).toBeFocused();
    } finally {
      releaseDelete();
      releaseListings();
      await page.unrouteAll({ behavior: "ignoreErrors" });
    }
  });

  /** Closing the list must reconcile its bell even when automatic reads stop.
   * A quiet stub feed prevents an unrelated fleet notice from repairing the
   * stale listing; the successful write alone must request the new state.
   * Hold the initial terminal replay until the list owns focus: a late reveal
   * used to steal that focus and close the list before the test could read it. */
  test("closing notifications quiets the bell under a build mismatch", async ({ page, request }) => {
    const session = await createSession(request, { title: `read-skew-${Date.now()}` });
    created.push(session.id);
    const stub = new NotificationStub([{ seq: 1, at: NOW, text: "read under the reload notice" }]);
    await pinAutoSelect(page, session.id);
    await page.addInitScript(() => {
      (window as any).__farhelmTestReplay = { holdMarker: true, idleMs: 60_000 };
    });
    // Route order is the mechanism: the newer notification handlers must
    // win over the broad skew route, while other API replies latch skew.
    await forceBuildSkew(page, "9.9.9-notification-read");
    await serveNotifications(page, session.id, stub);
    await stubFeed(page);
    await page.goto("/");
    await expect(page.locator(".build-skew")).toBeVisible();
    await waitForSessionMounted(page, session.id);
    await expect.poll(() => replayRecord(page, "terminal"), {
      message: "the selected terminal must be holding its initial replay before the bell opens",
    }).toMatchObject({ heldReason: "marker", revealed: false });
    const bell = row(page, session.id).locator(".session-row-bell");
    await expect(bell).toHaveClass(/\bloud\b/);
    await bell.click();
    const panel = page.locator(".session-bell-panel");
    await expect(panel).toBeFocused();
    await expect(page.locator(".session-bell-entry")).toHaveCount(1);
    await page.evaluate(() => (window as any).__farhelmIslands.terminal.test.releaseCatchUp());
    await waitForReplayReveal(page, "terminal");
    await expect(panel, "replay completion must leave the notification list's focus intact").toBeFocused();
    expect(stub.writes, "terminal reveal must not close and mark the list read").toEqual([]);
    await bell.click();
    await expect.poll(() => stub.writes).toEqual([{ session: session.id, route: "read", through: 1 }]);
    await expect(bell).not.toHaveClass(/\bloud\b/);
  });

  /** Clear must remove a quiet bell without borrowing read-on-close's refresh.
   * Every entry starts read, so closing sends no read mark. This isolates
   * Clear's own success path while the reload notice withdraws automatic reads. */
  test("clearing notifications removes the bell under a build mismatch", async ({ page, request }) => {
    const session = await createSession(request, { title: `clear-skew-${Date.now()}` });
    created.push(session.id);
    const stub = new NotificationStub([{ seq: 1, at: NOW, text: "clear under the reload notice" }]);
    stub.readThrough = 1;
    // Register skew first so the newer notification routes can serve their
    // marks; a different API reply latches the mismatch for the whole page.
    await forceBuildSkew(page, "9.9.9-notification-clear");
    await serveNotifications(page, session.id, stub);
    await stubFeed(page);
    await page.goto("/");
    await expect(page.locator(".build-skew")).toBeVisible();
    const bell = row(page, session.id).locator(".session-row-bell");
    await expect(bell).toBeVisible();
    await expect(bell).not.toHaveClass(/\bloud\b/);
    await bell.click();
    await expect(page.locator(".session-bell-entry")).toHaveCount(1);
    await page.getByRole("button", { name: "clear all", exact: true }).click();
    await expect.poll(() => stub.writes).toEqual([{ session: session.id, route: "cleared", through: 1 }]);
    await expect(page.locator(".session-bell-entry")).toHaveCount(0);
    await expect(bell).toHaveCount(0);
    expect(stub.writes.filter((write) => write.route === "read")).toEqual([]);
  });

  /** A failed best-effort read mark must be repairable on the next close.
   * The warning is the completion oracle: HTTP receipt alone precedes the
   * client's failure handling, whose cached mark this test needs to observe. */
  test("a failed notification read mark retries on the next close", async ({ page, request }) => {
    const session = await createSession(request, { title: `read-retry-${Date.now()}` });
    created.push(session.id);
    const stub = new NotificationStub([{ seq: 1, at: NOW, text: "retry this mark" }]);
    await serveNotifications(page, session.id, stub);
    let attempts = 0;
    await page.route(`**/api/sessions/${encodeURIComponent(session.id)}/notifications/read`, async (route) => {
      attempts++;
      if (attempts === 1) {
        const response = await route.fetch();
        await route.fulfill({ response, status: 500, body: "injected read mark refusal" });
      } else {
        await route.fallback();
      }
    });
    await page.goto("/");
    const bell = row(page, session.id).locator(".session-row-bell");
    await expect(bell).toHaveClass(/\bloud\b/);
    await bell.click();
    const entry = page.locator('.session-bell-entry[data-notification-seq="1"]');
    await expect(entry).toHaveClass(/\bnew\b/);
    const failed = page.waitForEvent("console", (message) => message.text().includes("could not mark the session's notifications read"));
    await bell.click();
    await failed;
    expect(attempts).toBe(1);
    expect(stub.readThrough).toBe(0);
    await bell.click();
    await expect(entry).toHaveClass(/\bnew\b/);
    await bell.click();
    await expect.poll(() => stub.readThrough).toBe(1);
    expect(attempts).toBe(2);
  });

  /** A delayed failure of mark 1 must not erase a newer completed mark 2.
   * Keep the listing's durable mark old to expose the local cache on reopen. */
  test("an older failed notification mark preserves a newer completed mark", async ({ page, request }) => {
    const session = await createSession(request, { title: `read-race-${Date.now()}` });
    created.push(session.id);
    const stub = new NotificationStub([{ seq: 1, at: NOW, text: "first" }]);
    await serveNotifications(page, session.id, stub);
    const feed = await stubFeed(page);
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    const marks: number[] = [];
    await page.route(`**/api/sessions/${encodeURIComponent(session.id)}/notifications/read`, async (route) => {
      const through = route.request().postDataJSON().through as number;
      marks.push(through);
      const response = await route.fetch();
      if (through === 1) {
        await gate;
        await route.fulfill({ response, status: 500, body: "injected older mark failure" });
      } else {
        await route.fulfill({ response, json: {} });
      }
    });
    try {
      await page.goto("/");
      await feed.waitForConnection(1);
      const bell = row(page, session.id).locator(".session-row-bell");
      await expect(bell).toHaveClass(/\bloud\b/);
      await bell.click();
      await expect(page.locator(".session-bell-entry")).toHaveCount(1);
      await bell.click();
      await expect.poll(() => marks).toEqual([1]);
      stub.notifications.push({ seq: 2, at: NOW, text: "second" });
      feed.notify(1);
      await bell.click();
      await expect(page.locator('.session-bell-entry[data-notification-seq="2"]')).toHaveClass(/\bnew\b/);
      const newer = page.waitForResponse((response) => response.url().endsWith(`/api/sessions/${encodeURIComponent(session.id)}/notifications/read`) && response.request().postDataJSON().through === 2);
      await bell.click();
      expect((await newer).ok()).toBe(true);
      const failed = page.waitForEvent("console", (message) => message.text().includes("could not mark the session's notifications read"));
      release();
      await failed;
      await bell.click();
      await expect(page.locator('.session-bell-entry[data-notification-seq="2"]')).not.toHaveClass(/\bnew\b/);
      expect(marks).toEqual([1, 2]);
    } finally {
      release();
    }
  });

  /** Recovery is read without opening the list or moving its shared marks.
   * Keep resolved history visible, then simulate a recurrence above a clear
   * mark to prove it reappears as new and unread rather than staying silent. */
  test("resolved history is quiet and a cleared warning can recur as new", async ({ page, request }) => {
    const session = await createSession(request, { title: `resolved-${Date.now()}` });
    created.push(session.id);
    const stub = new NotificationStub([{ seq: 1, at: NOW, text: "Restart stopped offering to resume" }]);
    await serveNotifications(page, session.id, stub);
    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    const bell = row(page, session.id).locator(".session-row-bell");
    await expect(bell).toHaveClass(/\bloud\b/, { timeout: 20_000 });
    stub.notifications[0].resolved = true;
    feed.notify(2);
    await expect(bell).not.toHaveClass(/\bloud\b/, { timeout: 20_000 });
    await expect(bell).toHaveAttribute("aria-label", `notifications for ${session.title}: none unread`);
    expect(stub.readThrough).toBe(0);
    expect(stub.writes).toEqual([]);
    await bell.click();
    const list = page.locator(".session-bell-flyout");
    const entry = list.locator(".session-bell-entry");
    await expect(entry).toHaveCount(1);
    await expect(entry).toHaveClass(/\bresolved\b/);
    await expect(entry).not.toHaveClass(/\bnew\b/);
    await expect(entry.locator(".session-bell-resolved")).toHaveText("resolved");
    await expect(entry.locator(".session-bell-new")).toHaveCount(0);
    await list.locator(".session-bell-clear").click();
    await expect.poll(() => stub.clearedThrough).toBe(1);
    feed.notify(3);
    await expect(bell).toHaveCount(0);
    stub.notifications = [{ seq: 2, at: NOW, text: "Restart stopped offering to resume again", resolved: false }];
    feed.notify(4);
    await expect(bell).toHaveClass(/\bloud\b/, { timeout: 20_000 });
    await bell.click();
    await expect(entry).toHaveAttribute("data-notification-seq", "2");
    await expect(entry).toHaveClass(/\bnew\b/);
    await expect(entry.locator(".session-bell-new")).toHaveText("new");
    await expect(entry.locator(".session-bell-resolved")).toHaveCount(0);
  });

  /**
   * The bell's whole visible contract on the row: only rows with
   * notifications have one, it is loud while any is unread and says how many
   * in its accessible name and hover help, and it sits between the agent
   * marks and the activity time without displacing either, in both
   * densities. The position check is what holds the overlay design to the
   * slot it is meant to cover (list/bell.rs). The hidden age copy must
   * retain the visible age's mono metrics while the row title uses Inter.
   */
  test("the bell appears only with notifications, loud while unread, beside the time", async ({ page, request }) => {
    const stamp = Date.now();
    const belled = await createSession(request, { title: `belled-${stamp}` });
    created.push(belled.id);
    const quiet = await createSession(request, { title: `quiet-${stamp}` });
    created.push(quiet.id);
    const stub = new NotificationStub([
      { seq: 1, at: NOW - 600, text: "older problem" },
      { seq: 2, at: NOW - 60, text: "newer problem" },
    ]);
    await serveNotifications(page, belled.id, stub);
    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);

    const bell = row(page, belled.id).locator(".session-row-bell");
    await expect(bell).toBeVisible({ timeout: 20_000 });
    await expect(row(page, quiet.id)).toBeVisible();
    await expect(row(page, quiet.id).locator(".session-row-bell")).toHaveCount(0);
    await expect(row(page, quiet.id).locator(".session-bell-slot")).toHaveCount(0);

    await expect(bell).toHaveClass(/\bloud\b/);
    await expect(bell).toHaveAttribute("aria-label", `notifications for belled-${stamp}: 2 unread`);
    await expect(bell).toHaveAttribute("data-tooltip", /^notifications: 2 unread/);

    for (const compact of [false, true]) {
      const target = row(page, belled.id);
      if (compact) {
        await setCompact(page, true);
        // Premise: the row really is compact before it is measured.
        await expect(target.locator(".session-row-meta")).toHaveCount(0);
      }
      await page.evaluate(() => document.fonts.ready);
      const age = target.locator(".status-time");
      const spacer = target.locator(".session-bell-time-spacer");
      await expect(age).toHaveCSS("font-family", /JetBrains Mono/);
      await expect(spacer).toHaveCSS("font-family", await age.evaluate((node) => getComputedStyle(node).fontFamily));
      expect(await spacer.textContent()).toBe(await age.textContent());
      const ageFont = await age.evaluate((node) => {
        const style = getComputedStyle(node);
        return [style.fontSize, style.fontWeight, style.lineHeight];
      });
      expect(await spacer.evaluate((node) => {
        const style = getComputedStyle(node);
        return [style.fontSize, style.fontWeight, style.lineHeight];
      })).toEqual(ageFont);
      const bellBox = (await bell.boundingBox())!;
      const slotBox = (await target.locator(".session-bell-slot").boundingBox())!;
      const agentBox = (await target.locator(".session-agent").boundingBox())!;
      const timeBox = (await target.locator(".status-time").boundingBox())!;
      expect(Math.abs(bellBox.x - slotBox.x), `compact=${compact}: the bell covers its slot`).toBeLessThanOrEqual(1);
      expect(bellBox.x, `compact=${compact}: after the agent marks`).toBeGreaterThanOrEqual(agentBox.x + agentBox.width);
      expect(bellBox.x + bellBox.width, `compact=${compact}: before the time`).toBeLessThanOrEqual(timeBox.x);
      expect(Math.abs(bellBox.y + bellBox.height / 2 - (timeBox.y + timeBox.height / 2)), `compact=${compact}: on the time's line`).toBeLessThanOrEqual(4);
    }

    stub.readThrough = 2;
    feed.notify(2);
    await expect(bell).not.toHaveClass(/\bloud\b/, { timeout: 20_000 });
    await expect(bell).toHaveAttribute("aria-label", `notifications for belled-${stamp}: none unread`);
  });

  /**
   * Opening the list: it floats outside the sidebar rather than being
   * clipped by it, does not open the session, lists entries newest first
   * with the new ones marked by a word, and closing it (Escape, a click
   * elsewhere, the bell again) marks everything it showed read, and sends
   * nothing when it showed nothing unread. Opening it takes focus, so
   * Escape works however the bell was clicked. These are the interaction
   * rules SPEC.md's Status section states.
   */
  test("the list floats, marks new entries, and closing marks them read", async ({ page, request }) => {
    const stamp = Date.now();
    const session = await createSession(request, { title: `list-${stamp}` });
    created.push(session.id);
    const other = await createSession(request, { title: `elsewhere-${stamp}` });
    created.push(other.id);
    // Auto-select opens the newest session on load; pin it elsewhere so the
    // bell's row being unselected afterwards says something about the click.
    await pinAutoSelect(page, other.id);
    const stub = new NotificationStub([
      { seq: 4, at: NOW - 3600, text: "already seen" },
      { seq: 5, at: NOW - 120, text: "just happened" },
    ]);
    stub.readThrough = 4;
    await serveNotifications(page, session.id, stub);
    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);

    const target = row(page, session.id);
    const bell = target.locator(".session-row-bell");
    await expect(bell).toBeVisible({ timeout: 20_000 });
    await expect(row(page, other.id)).toHaveAttribute("data-session-selected", "true", { timeout: 20_000 });
    await expect(target).toHaveAttribute("data-session-selected", "false");
    // Start from focus in the open session's terminal, where it usually is,
    // so the Escape below depends on the list taking focus when it opens
    // rather than on the click having focused the bell (which not every
    // engine does for a pointer click).
    const terminal = page.locator(".xterm-helper-textarea").first();
    await terminal.focus();
    await expect(terminal).toBeFocused();
    await bell.click();
    const list = page.locator(".session-bell-flyout");
    await expect(list).toBeVisible();
    await expect(bell).toHaveAttribute("aria-expanded", "true");
    await expect(list.locator(".session-bell-panel")).toBeFocused();
    await expect(target).toHaveAttribute("data-session-selected", "false");

    const sidebar = (await page.locator(".app-sidebar").boundingBox())!;
    const listBox = (await list.boundingBox())!;
    expect(listBox.x + listBox.width, "the list reaches past the sidebar instead of being clipped").toBeGreaterThan(sidebar.x + sidebar.width);

    const entries = list.locator(".session-bell-entry");
    await expect(entries).toHaveCount(2);
    await expect(entries.nth(0)).toContainText("just happened");
    await expect(entries.nth(0)).toHaveClass(/\bnew\b/);
    await expect(entries.nth(0).locator(".session-bell-new")).toHaveText("new");
    await expect(entries.nth(1)).toContainText("already seen");
    await expect(entries.nth(1)).not.toHaveClass(/\bnew\b/);
    await expect(entries.nth(0).locator(".session-bell-age")).toHaveText(/ago$/);

    await page.keyboard.press("Escape");
    await expect(list).toHaveCount(0);
    await expect(bell, "Escape hands focus back to the bell").toBeFocused();
    await expect.poll(() => stub.writes).toEqual([{ session: session.id, route: "read", through: 5 }]);
    feed.notify(2);
    await expect(bell).not.toHaveClass(/\bloud\b/, { timeout: 20_000 });

    // A click elsewhere closes it too, marking the entry that arrived since.
    stub.notifications.push({ seq: 6, at: NOW, text: "arrived later" });
    feed.notify(3);
    await expect(bell).toHaveClass(/\bloud\b/, { timeout: 20_000 });
    await bell.click();
    await expect(list).toBeVisible();
    await expect(list.locator(".session-bell-new")).toHaveCount(1);
    // Bottom right of the window: the open session's terminal, away from the
    // sidebar and from the list floating beside it.
    const viewport = page.viewportSize()!;
    await page.mouse.click(viewport.width - 20, viewport.height - 20);
    await expect(list).toHaveCount(0);
    await expect.poll(() => stub.writes.at(-1)).toEqual({ session: session.id, route: "read", through: 6 });

    // The bell itself toggles the list shut. With nothing unread left in it,
    // closing it has nothing to mark and sends no request.
    feed.notify(4);
    await expect(bell).not.toHaveClass(/\bloud\b/, { timeout: 20_000 });
    await bell.click();
    await expect(list).toBeVisible();
    await expect(list.locator(".session-bell-new")).toHaveCount(0);
    await bell.click();
    await expect(list).toHaveCount(0);
    await expect(bell).toHaveAttribute("aria-expanded", "false");
    expect(stub.writes).toHaveLength(2);
    expect(stub.writes.filter((write) => write.session === other.id), "closing this list must not mark the other open session").toEqual([]);
  });

  /**
   * Clear removes every notification the list showed, for every window, and
   * the bell leaves the row until a new one arrives. A notification recorded
   * after the clear brings the bell back, loud.
   */
  test("clear removes the bell until a new notification arrives", async ({ page, request }) => {
    const stamp = Date.now();
    const session = await createSession(request, { title: `clear-${stamp}` });
    created.push(session.id);
    const stub = new NotificationStub([{ seq: 7, at: NOW - 30, text: "clear me" }]);
    await serveNotifications(page, session.id, stub);
    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);

    const bell = row(page, session.id).locator(".session-row-bell");
    await expect(bell).toBeVisible({ timeout: 20_000 });
    await bell.click();
    await page.locator(".session-bell-clear").click();
    await expect(page.locator(".session-bell-flyout")).toHaveCount(0);
    await expect.poll(() => stub.writes.some((write) => write.session === session.id && write.route === "cleared" && write.through === 7)).toBe(true);
    feed.notify(2);
    await expect(bell).toHaveCount(0, { timeout: 20_000 });

    stub.notifications.push({ seq: 8, at: NOW, text: "a new one" });
    feed.notify(3);
    await expect(bell).toBeVisible({ timeout: 20_000 });
    await expect(bell).toHaveClass(/\bloud\b/);
  });
});
