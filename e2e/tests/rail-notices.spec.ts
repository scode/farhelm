/**
 * The shipped bell and Trash owner run against controlled host observations.
 * SQLite history, ordering and actual permission failures have separate Rust
 * coverage; these cases pin rendering, exact close marks, focus and cleanup
 * dispatch in both engines without deleting any directory.
 */
import type { APIRequestContext, Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { localHostId, observeFeedReaders, readFeedReaders, stubFeed } from "./helpers/fleet";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";

installTerminalSuiteHooks();

type Notice = {
  id: number; host: number; raised_at: number; read: boolean; resolved: boolean;
  content: { kind: "stuck_trash"; checkout_id: string; name: string; repository: string;
    bytes: number | null; reason: string; at: number };
};

/** Real connection metadata supplies the incarnation frozen by Retry. Only the
 * display name, notice history and archive facts are controlled at HTTP edges. */
async function fixture(page: Page, request: APIRequestContext, extraHost = false) {
  const host = await localHostId(request);
  const otherHost = 9030;
  const model = {
    phase: "gone" as "stuck" | "removing" | "gone",
    notices: [] as Notice[],
    marks: [] as { action: string; ids: number[] }[],
    deletes: [] as { host: number; ids: string[]; expected_incarnation: number }[],
    listingReads: 0,
    listingHosts: [] as number[],
    noticeError: null as string | null,
  };
  const feed = await stubFeed(page);
  feed.notifyOnConnect(0);
  await page.route("**/api/hosts", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    const local = body.hosts.find((entry: { id: number }) => entry.id === host);
    expect(local, "the fixture retains a real registered host").toBeTruthy();
    body.hosts = [{ ...local, name: "Build host" }];
    // A display-only peer exercises independent readers; its archive routes
    // are intercepted and no session or terminal is served from this identity.
    if (extraHost) body.hosts.push({ ...local, id: otherHost, name: "Other host" });
    await route.fulfill({ response, json: body });
  });
  await page.route("**/api/notices", async (route) => {
    await fulfillAsHelm(route, { json: { notices: model.notices } });
  });
  await page.route("**/api/notices/marks", async (route) => {
    const mark = route.request().postDataJSON();
    model.marks.push(mark);
    if (mark.action === "read") {
      for (const notice of model.notices) if (mark.ids.includes(notice.id)) notice.read = true;
    } else {
      expect(mark.action).toBe("clear_resolved");
      model.notices = model.notices.filter((notice) => !notice.resolved || !mark.ids.includes(notice.id));
    }
    await fulfillAsHelm(route, { status: 204 });
  });
  await page.route("**/api/checkout-trash/list", async (route) => {
    const selected = route.request().postDataJSON().host;
    expect(extraHost ? [host, otherHost] : [host]).toContain(selected);
    model.listingReads++;
    model.listingHosts.push(selected);
    await fulfillAsHelm(route, { json: {
      reports_retired: true, removing: model.phase === "removing", issues: [],
      ...(model.noticeError ? { notice_error: model.noticeError } : {}),
      checkouts: model.phase === "gone" ? [] : [{
        id: "stuck-checkout", name: "archived-work", repository: "example/project",
        path: "/work/.farhelm-trash/.rm:archived-work", archived_at: 1_700_000_000, bytes: 4096,
        removing: model.phase === "removing",
        ...(model.phase === "stuck" ? { stuck: { reason: "Permission denied <retained>", at: 1_700_000_060 } } : {}),
      }],
    } });
  });
  await page.route("**/api/checkout-trash/delete", async (route) => {
    const selected = route.request().postDataJSON();
    model.deletes.push(selected);
    model.phase = "removing";
    await fulfillAsHelm(route, { json: { removed: selected.ids, issues: [] } });
  });
  return { host, otherHost, model, feed };
}

/** Stable condition identity and deliberately unread resolved history exercise
 * the UI's own quietness rule independently of the server's read projection. */
function notice(host: number, id = 1): Notice {
  return {
    id, host, raised_at: 1_700_000_000 + id, read: false, resolved: false,
    content: { kind: "stuck_trash", checkout_id: "stuck-checkout", name: "archived-work",
      repository: "example/project", bytes: 4096, reason: "Permission denied <retained>", at: 1_700_000_060 },
  };
}

for (const palette of ["dark", "light"] as const) {
  /** Independently pinned computed fills catch an outline bell accidentally
   * painted red, or a correct CSS token inherited by the wrong SVG element.
   * The idle client's trash count must refresh from the same content-free feed,
   * even though it issued no Delete and had no active completion poll. */
  test(`rail notices quiet, alerting and resolved in ${palette}`, async ({ page, request }, info) => {
    await page.addInitScript((appearance) => localStorage.setItem("farhelm.appearance", appearance), palette);
    const { host, model, feed } = await fixture(page, request);
    await page.goto("/");
    await feed.waitForConnection(1);
    const bell = page.locator(".rail-notices-toggle");
    const glyph = bell.locator(".bell-glyph");
    const fill = glyph.locator("path").first();
    const quiet = palette === "dark" ? "rgb(162, 170, 187)" : "rgb(79, 90, 112)";
    const loud = palette === "dark" ? "rgb(240, 149, 149)" : "rgb(161, 52, 67)";
    await expect(page.locator("html")).toHaveAttribute("data-appearance", palette);
    await expect(bell).toHaveAccessibleName("Notifications: 0 unread");
    await expect(fill).toHaveCSS("fill", "none");
    await expect(glyph).toHaveCSS("color", quiet);
    await expect(glyph).toHaveCSS("width", "16px");
    await bell.hover();
    await expect(bell).toHaveCSS("background-color", palette === "dark" ? "rgb(40, 46, 58)" : "rgb(255, 255, 255)");
    await expect(glyph).toHaveCSS("color", quiet);
    await page.mouse.move(1000, 200);
    await expect(page.locator(".checkout-trash-button")).toHaveAttribute("data-count", "0");
    await page.screenshot({ path: info.outputPath(`rail-notices-quiet-${palette}.png`) });

    model.phase = "stuck";
    model.notices = [notice(host)];
    const before = model.listingReads;
    feed.notify(1);
    await expect(bell).toHaveAccessibleName("Notifications: 1 unread");
    await expect(fill).toHaveCSS("fill", loud);
    await expect(bell.locator(".rail-notices-count")).toHaveText("1");
    await expect.poll(() => model.listingReads).toBeGreaterThan(before);
    await expect(page.locator(".checkout-trash-button")).toHaveAttribute("data-count", "1");
    await bell.click();
    const panel = page.locator(".rail-notices-panel");
    await expect(panel).toBeVisible();
    await expect(page.locator(".rail-notices-flyout")).toHaveCSS("pointer-events", "auto");
    await expect(panel).toContainText("Build host: trash could not be freed");
    await expect(panel).toContainText("Permission denied <retained>");
    await expect(panel).toContainText("4.0 KiB last measured");
    await expect(panel.locator("script")).toHaveCount(0);
    await page.screenshot({ path: info.outputPath(`rail-notices-alerting-${palette}.png`) });
    await expect(panel).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(panel).toHaveCount(0);
    await expect(bell).toBeFocused();
    await expect.poll(() => model.marks).toEqual([{ action: "read", ids: [1] }]);
    await expect(bell).toHaveAccessibleName("Notifications: 0 unread");

    // Repeated failure retains the shared read state rather than re-alerting.
    model.notices[0].content.at++;
    model.notices[0].content.reason = "Still denied";
    feed.notify(2);
    await expect(fill).toHaveCSS("fill", "none");
    model.notices = [{ ...notice(host), resolved: true }];
    model.phase = "gone";
    feed.notify(3);
    await bell.click();
    await expect(panel.locator(".rail-notice.resolved")).toHaveCount(1);
    await expect(panel.locator(".rail-notice.resolved")).toHaveCSS("color", quiet);
    await expect(bell).toHaveAccessibleName("Notifications: 0 unread");
    await expect(fill).toHaveCSS("fill", "none");
    await expect(panel.getByRole("button", { name: "Retry now", exact: true })).toHaveCount(0);
    await page.screenshot({ path: info.outputPath(`rail-notices-resolved-${palette}.png`) });
    await panel.getByRole("button", { name: "Clear resolved", exact: true }).click();
    await expect(panel).toContainText("No notifications");
    expect(model.marks.at(-1)).toEqual({ action: "clear_resolved", ids: [1] });
  });
}

/** A GET containing a newly arrived item can be in flight while an old panel
 * closes. Its close marks only rendered IDs; an acknowledged mark also fences
 * that old GET from restoring the read item's former unread projection. */
test("rail close marks only shown notices and preserves independent focus", async ({ page, request }) => {
  const { host, model, feed } = await fixture(page, request);
  model.phase = "stuck";
  model.notices = [notice(host)];
  let release!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  let waiting = false;
  try {
    await page.goto("/");
    await feed.waitForConnection(1);
    const bell = page.locator(".rail-notices-toggle");
    await bell.click();
    await expect(page.locator('.rail-notice[data-notice-id="1"]')).toBeVisible();
    await page.route("**/api/notices", async (route) => {
      const observed = structuredClone(model.notices);
      waiting = true;
      await held;
      await fulfillAsHelm(route, { json: { notices: observed } });
    });
    model.notices.unshift(notice(host, 2));
    feed.notify(1);
    await expect.poll(() => waiting).toBe(true);
    await bell.click();
    await expect(page.locator(".rail-notices-panel")).toHaveCount(0);
    await expect.poll(() => model.marks).toEqual([{ action: "read", ids: [1] }]);
    release();
    await expect(bell).toHaveAccessibleName("Notifications: 1 unread");
    await bell.click();
    await expect(page.locator('.rail-notice[data-notice-id="2"]')).toBeVisible();
    const settings = page.locator(".app-settings-toggle");
    await settings.focus();
    await expect(settings).toBeFocused();
    await expect(page.locator(".rail-notices-panel")).toHaveCount(0);
    await expect(settings).toBeFocused();
    await expect.poll(() => model.marks.length).toBe(2);
    expect(model.marks[1]).toEqual({ action: "read", ids: [2, 1] });
    await bell.click();
    await expect(page.locator(".rail-notices-panel")).toBeVisible();
    await page.locator(".hosts-panel-toggle").click();
    await expect(page.locator(".rail-notices-panel")).toHaveCount(0);
    await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "true");
  } finally {
    release();
    await page.unrouteAll({ behavior: "wait" });
  }
});

/** A burst received behind an old snapshot must retain one repair, without
 * queueing a request per revision or holding a healthy host behind the slow
 * one. Pull-only reader snapshots establish delivery before negative counts
 * and retirement before the final exact count; elapsed silence proves neither. */
test("fleet news coalesces Trash reads independently for each host", async ({ page, request }) => {
  await observeFeedReaders(page);
  const { host, otherHost, model, feed } = await fixture(page, request, true);
  let release!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  let localReads = 0;
  try {
    await page.goto("/");
    await feed.waitForConnection(1);
    const trigger = page.locator(".checkout-trash-button");
    const idleTrash = async () => {
      const readers = (await readFeedReaders(page)).filter((entry) => entry.role === "trash");
      return readers.length === 2 && readers.every((entry) => entry.caught_up === true &&
        entry.readers.length === 1 && entry.readers.every((reader) =>
          !reader.running && !reader.task && reader.demand === "None"));
    };
    await expect.poll(idleTrash).toBe(true);
    await expect(trigger).toHaveAccessibleName("Checkout trash, empty");
    const initial = (await readFeedReaders(page)).find((entry) => entry.role === "list")!;
    expect(initial.healthy).toBe(true);
    expect(initial.acted_on).toBe(initial.notices);
    const initialNotices = Number(initial.notices);
    await page.route("**/api/checkout-trash/list", async (route) => {
      if (route.request().postDataJSON().host !== host) return route.fallback();
      localReads++;
      if (localReads > 1) return route.fallback();
      // This answer belongs to the pre-failure world, even when delivered later.
      await held;
      await fulfillAsHelm(route, { json: {
        reports_retired: true, removing: false, issues: [], checkouts: [],
      } });
    });
    feed.notify(1);
    await expect.poll(() => localReads).toBe(1);
    model.phase = "stuck";
    for (let revision = 2; revision <= 20; revision++) feed.notify(revision);
    await expect.poll(async () => {
      const snapshots = await readFeedReaders(page);
      const list = snapshots.find((entry) => entry.role === "list");
      const slow = snapshots.find((entry) => entry.role === "trash" && entry.host === host);
      const fast = snapshots.find((entry) => entry.role === "trash" && entry.host === otherHost);
      return Number(list?.notices) >= initialNotices + 20 && list?.acted_on === list?.notices &&
        slow?.caught_up === true && slow.readers[0].running && slow.readers[0].demand === "Notice" &&
        fast?.caught_up === true && !fast.readers[0].running && !fast.readers[0].task &&
        fast.readers[0].demand === "None";
    }).toBe(true);
    await expect(trigger).toHaveAttribute("data-count", "1");
    expect(model.listingHosts).toContain(otherHost);
    expect(localReads, "all burst demand was delivered behind exactly one held RPC").toBe(1);
    release();
    await expect.poll(idleTrash).toBe(true);
    await expect(trigger).toHaveAttribute("data-count", "2");
    expect(localReads, "one coalesced repair retires the entire burst").toBe(2);
  } finally {
    release();
    await page.unrouteAll({ behavior: "wait" });
  }
});

/** The ordinary Trash trigger is hidden during marking and its ancestor is
 * hidden in Hosts. Open Trash must reveal the permanent owner directly, then
 * return focus to the visible bell. Retry freezes the same current ID and
 * incarnation as the row action, remains unresolved on acceptance and disables
 * itself while cleanup runs. A notice-save warning must not hide real rows. */
test("rail Retry and Open Trash share the permanent guarded cleanup owner", async ({ page, request }) => {
  const { host, model, feed } = await fixture(page, request);
  model.phase = "stuck";
  model.notices = [notice(host)];
  model.noticeError = "fixture shared notice save unavailable";
  await page.goto("/");
  await feed.waitForConnection(1);
  const row = page.locator(".session-row-open").first();
  await expect(row).toBeVisible();
  await row.click({ modifiers: ["Control"] });
  await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "1");
  const trigger = page.locator(".checkout-trash-button");
  await expect(trigger).toBeHidden();
  await page.locator(".hosts-panel-toggle").click();
  await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "true");
  const bell = page.locator(".rail-notices-toggle");
  await bell.click();
  const panel = page.locator(".rail-notices-panel");
  await expect(panel).toBeVisible();
  await panel.getByRole("button", { name: "Open Trash", exact: true }).click();
  const dialog = page.locator(".checkout-trash-dialog");
  await expect(dialog).toBeVisible();
  await expect(panel).toHaveCount(0);
  await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "false");
  await expect(trigger).toBeHidden();
  await expect(dialog.locator(".checkout-trash-entry")).toHaveCount(1);
  await expect(dialog.locator(".checkout-trash-notice-error")).toContainText(model.noticeError!);
  await expect(dialog.locator(".checkout-trash-total")).not.toContainText("partial");
  await dialog.locator(".trash-close").click();
  await expect(dialog).toHaveCount(0);
  await expect(bell).toBeFocused();
  await bell.click();
  const retry = panel.getByRole("button", { name: "Retry now", exact: true });
  await expect(retry).toBeEnabled();
  await retry.click();
  await expect.poll(() => model.deletes.length).toBe(1);
  expect(model.deletes[0]).toMatchObject({ host, ids: ["stuck-checkout"] });
  expect(model.deletes[0].expected_incarnation).toBeGreaterThan(0);
  await expect(retry).toBeDisabled();
  await expect(panel.locator(".rail-notice.resolved")).toHaveCount(0);
  await expect(trigger).toHaveAttribute("data-count", "1");
  model.phase = "gone";
  model.notices[0].resolved = true;
  feed.notify(1);
  await expect(panel.locator(".rail-notice.resolved")).toHaveCount(1);
  await expect(trigger).toHaveAttribute("data-count", "0");
  expect(model.deletes).toHaveLength(1);
});
