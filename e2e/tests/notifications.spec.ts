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
import { expect, test } from "./helpers/evidence";
import { Locator, Page, Route } from "@playwright/test";
import { cleanupSession, createSession, pinAutoSelect, resetPreferences, stubFeed } from "./helpers/fleet";

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
      const session = new URL(route.request().url()).pathname.split("/")[3];
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

test.describe("session notifications", () => {
  const created: string[] = [];
  // The compact choice and the auto-selected row are helm-wide preferences
  // that outlive the page, so a test that changes them must not hand them to
  // the next spec.
  test.afterEach(async ({ request }) => {
    while (created.length) await cleanupSession(request, created.pop()!);
    await resetPreferences(request);
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
    await page.route(`**/api/sessions/${session.id}/notifications/read`, async (route) => {
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
    await page.route(`**/api/sessions/${session.id}/notifications/read`, async (route) => {
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
      const newer = page.waitForResponse((response) => response.url().endsWith(`/api/sessions/${session.id}/notifications/read`) && response.request().postDataJSON().through === 2);
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
   * slot it is meant to cover (list/bell.rs).
   */
  test("the bell appears only with notifications, loud while unread, beside the time", async ({ page, request }) => {
    const stamp = Date.now();
    const belled = await createSession(request, { title: `belled-${stamp}` });
    const quiet = await createSession(request, { title: `quiet-${stamp}` });
    created.push(belled.id, quiet.id);
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
        await page.getByLabel("compact").check();
        // Premise: the row really is compact before it is measured.
        await expect(target.locator(".session-row-meta")).toHaveCount(0);
      }
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
    const other = await createSession(request, { title: `elsewhere-${stamp}` });
    created.push(session.id, other.id);
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
