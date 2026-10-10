// Layout and selection are client behavior. Stage controlled approval
// replies over the real UI and owned session, while the spawn spec retains the
// real agent-to-helm approval path. The controlled feed makes expiry explicit.
import { expect, test } from "./helpers/evidence";
import { type Page } from "@playwright/test";
import {
  cleanupSession,
  createSession,
  pinAutoSelect,
  SESSION_LISTING,
  stubFeed,
  type SessionRow,
} from "./helpers/fleet";
import { attachSession } from "./helpers/term";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";

// Capture the real helm's build stamp before fabricating replies. Missing
// stamps deliberately put the UI into build-skew mode and close its feed.
installTerminalSuiteHooks();

/** Permit action-specific fields while keeping listing identity explicit in the fixture. */
type Approval = { id: string; host_name: string; [field: string]: unknown };

/** A mockup-sized launch with every decision field present and labelled. */
function launchRequest(id: string, host = "devbox"): Approval {
  return {
    id,
    host_id: host === "devbox" ? 1 : 2,
    host_name: host,
    session: { id: "7c1e9a52-4b0d-4f7e-9a3c-2d81f6b0e413", title: "fix-flaky-ci", host_name: host },
    expires_at_ms: Date.now() + 540_000,
    action: {
      kind: "launch",
      verb: "create",
      host_name: "buildbox",
      cwd: "~/src/app/services/ingest",
      title: null,
      source: null,
      launch: {
        kind: "agent",
        selection: { harness: "claude", model: "opus", effort: "high", permissions: "approve" },
        start: [
          "claude", "--model", "opus", "--effort", "high", "--permission-mode", "default", "{farhelm_args}",
          "--", "Investigate why tests/e2e/ingest_retry.rs fails on CI and propose a fix",
        ],
        resume: null,
      },
    },
  };
}

/** Own the listing and answer replies; expiry is a new listing, never a client timer. */
async function stageApprovals(page: Page, initial: Approval[]) {
  let waiting = initial;
  const answers: Array<{ id: string; answer: string }> = [];
  let revision = 1;
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  /** Listing mutations need a connected feed so the UI must read the changed reply. */
  async function notifyListing() {
    await expect.poll(() => feed.openSockets(), { message: "listing change needs the owned live feed" }).toBe(1);
    feed.notify(++revision);
  }
  await page.route((url) => url.pathname === "/api/approvals", async (route) => {
    await fulfillAsHelm(route, { json: { approvals: waiting } });
  });
  await page.route((url) => url.pathname.startsWith("/api/approvals/"), async (route) => {
    const id = decodeURIComponent(new URL(route.request().url()).pathname.split("/").at(-1)!);
    answers.push({ id, answer: route.request().postDataJSON().answer });
    waiting = waiting.filter((card) => card.id !== id);
    await fulfillAsHelm(route, { status: 204 });
  });
  return {
    answers,
    async replace(next: Approval[]) {
      waiting = next;
      await notifyListing();
    },
    async expire(id: string) {
      waiting = waiting.filter((card) => card.id !== id);
      await notifyListing();
    },
  };
}

let session: SessionRow | undefined;

// The real selected view supplies the header and tabs whose bounds matter.
// Cleanup includes partially completed setup; no shared fixture is renamed.
test.beforeEach(async ({ page, request }) => {
  session = undefined;
  session = await createSession(request, { title: `approval-layout-${Date.now()}` });
  await pinAutoSelect(page, session.id);
});
test.afterEach(async ({ request }) => {
  if (session) await cleanupSession(request, session.id);
});

/** Establish the selected view before geometry or modal assertions use its chrome. */
async function openSelected(page: Page) {
  await page.goto("/");
  await attachSession(page, session!.id);
  await expect(page.locator(".app-main .tab-strip")).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".approval-card .approval-allow")).toBeEnabled();
}

/** Read just the rectangles needed to distinguish pane-relative placement from a window corner. */
async function placement(page: Page) {
  return page.evaluate(() => {
    const rect = (selector: string) => {
      const { x, y, width, height, bottom, right } = document.querySelector(selector)!.getBoundingClientRect();
      return { x, y, width, height, bottom, right };
    };
    return {
      main: rect(".app-main"),
      tabs: rect(".app-main .tab-strip"),
      card: rect(".approval-card"),
      region: rect(".approval-cards"),
      actions: rect(".approval-card-actions"),
    };
  });
}

/** The common launch fits above the prompt; sidebar changes and narrow widths still follow the pane. */
test("a compact card follows the main pane and keeps the answer buttons visible", async ({ page }, info) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  const staged = await stageApprovals(page, [launchRequest("layout")]);
  await openSelected(page);
  await expect.poll(async () => {
    const { main, card, tabs } = await placement(page);
    return Math.abs(card.x + card.width / 2 - main.x - main.width / 2) < 1
      && card.y >= tabs.bottom + 11 && card.x >= main.x + 11;
  }, { message: "card centered below the tab strip and clear of the sidebar" }).toBe(true);
  const wide = await placement(page);
  expect(wide.card.width).toBeCloseTo(660, 0);
  expect(wide.card.height, "mockup-sized request stays compact").toBeLessThan(380);
  expect(wide.actions.bottom, "answers visible without scrolling").toBeLessThanOrEqual(wide.region.bottom);
  expect(wide.region.bottom, "the terminal's bottom remains clear").toBeLessThan(wide.main.bottom - 100);
  await page.screenshot({ path: info.outputPath("approval-common.png") });

  // Change actual pane geometry, without depending on the separate sidebar
  // resizing feature. A hardcoded sidebar offset fails this observation.
  await page.locator(".app-sidebar").evaluate((node) => { (node as HTMLElement).style.width = "500px"; });
  await expect.poll(async () => {
    const { main, card } = await placement(page);
    return Math.abs(card.x + card.width / 2 - main.x - main.width / 2);
  }).toBeLessThan(1);
  await page.locator(".app-sidebar").evaluate((node) => { (node as HTMLElement).style.removeProperty("width"); });
  await page.setViewportSize({ width: 520, height: 800 });
  await page.locator(".app-shell").evaluate((node) => {
    // Header actions can extend the shell's scroll range beyond the pane.
    // Reveal the pane's left edge, not the far end of that overflow.
    node.scrollLeft = (node.querySelector(".app-main") as HTMLElement).offsetLeft;
  });
  await expect.poll(async () => {
    const { main, card } = await placement(page);
    return card.x >= Math.max(11, main.x + 11) && card.right <= main.right - 11 && card.width <= 296;
  }, { message: "320px pane contains the whole card" }).toBe(true);
  await expect.poll(() => page.locator(".approval-card-rows").evaluate((node) =>
    getComputedStyle(node).gridTemplateColumns.split(" ").length
  )).toBe(1);
  expect(await page.locator(".approval-cards").evaluate((node) => node.scrollWidth - node.clientWidth)).toBe(0);
  await page.screenshot({ path: info.outputPath("approval-narrow.png") });

  // A minimum-width pane moves when its sidebar grows, but does not resize.
  // Observing the pane's size alone would leave the overlay behind.
  const beforeResize = await placement(page);
  await page.locator(".app-sidebar").evaluate((node) => { (node as HTMLElement).style.width = "500px"; });
  await expect.poll(async () => {
    const { main, card } = await placement(page);
    return Math.abs(card.x + card.width / 2 - main.x - main.width / 2);
  }, { message: "sidebar resize moves the minimum-width pane and its overlay together" }).toBeLessThan(1);
  const afterResize = await placement(page);
  expect(afterResize.main.width).toBeCloseTo(beforeResize.main.width, 0);
  expect(afterResize.main.x - beforeResize.main.x).toBeGreaterThan(100);
  await page.locator(".app-sidebar").evaluate((node) => { (node as HTMLElement).style.removeProperty("width"); });
  await expect.poll(async () => {
    const { main, card } = await placement(page);
    return Math.abs(card.x + card.width / 2 - main.x - main.width / 2);
  }).toBeLessThan(1);

  // A flex row can wrap whole buttons while leaving a long host token wider
  // than the card. The peer text must wrap inside the permanent-allow button.
  const longHost = `remote-${"x".repeat(80)}`;
  await staged.replace([launchRequest("long-host", longHost)]);
  await expect(page.locator(".approval-card")).toHaveAttribute("data-approval-id", "long-host");
  await expect(page.locator(".approval-always-allow")).toContainText(longHost);
  await expect.poll(() => page.locator(".approval-cards").evaluate((node) => node.scrollWidth - node.clientWidth)).toBe(0);
  await expect.poll(() => page.locator(".approval-always-allow").evaluate((node) => node.scrollWidth - node.clientWidth)).toBe(0);
  await page.screenshot({ path: info.outputPath("approval-narrow-long-host.png") });
});

/** The sibling region must follow both the empty pane and a newly mounted session view. */
test("placement follows the empty pane, session mount, and approval remount", async ({ page }) => {
  const staged = await stageApprovals(page, [launchRequest("empty-pane")]);
  await page.route(SESSION_LISTING, (route) => fulfillAsHelm(route, {
    status: 200,
    contentType: "application/json",
    body: JSON.stringify({ sessions: [], total: 0, matching: 0, truncated: false }),
  }));
  await page.goto("/");
  await expect(page.locator(".main-empty")).toHaveText("no sessions — create one");
  await expect(page.locator(".app-main .tab-strip")).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => {
    const main = document.querySelector(".app-main")!.getBoundingClientRect();
    const card = document.querySelector(".approval-card")!.getBoundingClientRect();
    return Math.abs(card.top - main.top - 12) < 1
      && Math.abs(card.x + card.width / 2 - main.x - main.width / 2) < 1;
  }), { message: "no-session card uses the actual pane top" }).toBe(true);

  await page.unroute(SESSION_LISTING);
  await staged.replace([launchRequest("empty-pane")]);
  await attachSession(page, session!.id);
  await expect(page.locator(".app-main .tab-strip")).toBeVisible();
  await expect.poll(async () => {
    const { card, tabs } = await placement(page);
    return Math.abs(card.y - tabs.bottom - 12);
  }, { message: "new session chrome replaces the no-session anchor" }).toBeLessThan(1);

  await staged.replace([]);
  await expect(page.locator(".approval-cards")).toHaveCount(0);
  await staged.replace([launchRequest("remounted")]);
  await expect(page.locator(".approval-card")).toHaveAttribute("data-approval-id", "remounted");
  await expect.poll(async () => {
    const { card, tabs } = await placement(page);
    return Math.abs(card.y - tabs.bottom - 12);
  }, { message: "remounted region owns a fresh measured anchor" }).toBeLessThan(1);
});

/** A stale delay must not arm a different request after rapid switches across hosts. */
test("one request stays selected across refreshes and rapid switches rearm the buttons", async ({ page }) => {
  await page.clock.install();
  const staged = await stageApprovals(page, [launchRequest("first"), launchRequest("second", "laptop"), launchRequest("third")]);
  await openSelected(page);
  const card = page.locator(".approval-card");
  await expect(card).toHaveCount(1);
  await expect(card).toHaveAttribute("data-approval-id", "first");
  await expect(page.locator(".approval-waiting")).toHaveCount(2);
  await expect(card.locator(".approval-card-count")).toHaveText("1 of 3 waiting");
  await page.clock.pauseAt(new Date(Date.now() + 5_000));
  await page.locator('.approval-waiting[data-approval-id="second"]').click();
  await expect(card).toHaveAttribute("data-approval-id", "second");
  await expect(card.locator(".approval-allow")).toBeDisabled();
  await page.clock.runFor(400);
  await page.locator('.approval-waiting[data-approval-id="first"]').click();
  await expect(card).toHaveAttribute("data-approval-id", "first");
  await page.clock.runFor(300);
  await expect(card.locator(".approval-allow"), "the previous card's timer cannot arm this card").toBeDisabled();
  await page.clock.runFor(400);
  await expect(card.locator(".approval-allow")).toBeEnabled();
  const second = page.locator('.approval-waiting[data-approval-id="second"]');
  await second.focus();
  await expect(second).toBeFocused();
  await second.press("Enter");
  await expect(card).toHaveAttribute("data-approval-id", "second");
  await page.clock.runFor(700);
  await staged.expire("third");
  await expect(page.locator(".approval-waiting")).toHaveCount(1);
  await expect(card).toHaveAttribute("data-approval-id", "second");
  await page.clock.runFor(700);
  await expect(card.locator(".approval-deny")).toBeEnabled();
  await card.locator(".approval-deny").click();
  await expect(card).toHaveAttribute("data-approval-id", "first");
  expect(staged.answers).toEqual([{ id: "second", answer: "deny" }]);
  await page.clock.runFor(700);
  await staged.expire("first");
  await expect(page.locator(".approval-cards")).toHaveCount(0);
});

/** A dialog cannot make an agent's visible approval request unanswerable. */
test("approval remains answerable while the quick switcher is open", async ({ page }) => {
  const staged = await stageApprovals(page, [launchRequest("dialog")]);
  await openSelected(page);
  const trigger = page.locator(".quick-switcher-trigger");
  await expect(trigger).toHaveCount(1);
  const terminal = page.locator("#terminal .xterm-helper-textarea");
  await terminal.focus();
  await expect(terminal).toBeFocused();
  const mac = await page.evaluate(() => navigator.platform.includes("Mac"));
  await page.keyboard.press(mac ? "Meta+k" : "Control+Shift+k");
  const dialog = page.getByRole("dialog", { name: "quick switcher" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("combobox")).toBeFocused();
  await expect(page.locator(".app-main")).toHaveAttribute("inert", "");
  await page.locator(".approval-card .approval-allow").click();
  await expect(page.locator(".approval-card")).toHaveCount(0);
  await expect(dialog).toBeVisible();
  expect(staged.answers).toEqual([{ id: "dialog", answer: "allow" }]);
});
