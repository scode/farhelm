// Request-local progress must follow the operation, not whichever surface
// happened to send it. These fixtures hold only the mutation under test;
// listing and detail reads keep answering while the page clock advances.
import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { agentLaunchRow, localHostId, openRowMenu, SESSION_LISTING } from "./helpers/fleet";
import { selectSessionActionWith } from "./helpers/session-actions";
import { routeGate } from "./helpers/route-gate";
import { drainedRoute } from "./helpers/drained-route";
import { mountNativeTunnelPreview } from "./helpers/native-tunnel-preview";
import { fulfillAsHelm, installTerminalSuiteHooks, rowByTitle } from "./helpers/terminal-suite";

installTerminalSuiteHooks();
test.use({ fabricatedTerminals: true });

const SOURCE_ID = "11111111-2222-3333-4444-555555555555";
const SOURCE_TITLE = "slow lifecycle request";
const HOST_NAME = "slow host";

/**
 * Supply a resumable session without creating a live process.
 * Its detail and listing agree, so refreshes cannot replace the premise while
 * a held mutation exercises the real component and production deadline.
 */
async function openSource(
  page: Page, request: APIRequestContext,
  { state = "interrupted", hostName = HOST_NAME, launch }: {
    state?: string; hostName?: string; launch?: Record<string, unknown>;
  } = {},
): Promise<Locator> {
  const host = await localHostId(request);
  const source = {
    id: SOURCE_ID,
    title: SOURCE_TITLE,
    cwd: "/tmp",
    invocation: "claude",
    ...(launch ? { launch, agent_kind: "codex", invocation: "codex" } : {}),
    status: { state },
    restart_offer: "resume",
    created_at: 0,
    last_activity_at: 0,
    tabs: [],
    host,
    host_name: hostName,
  };
  await page.route("**/api/hosts", async route => {
    const response = await route.fetch();
    const listing = await response.json();
    const local = listing.hosts.find((row: { id: number }) => row.id === host);
    expect(local, "the captured request still uses the real local connection").toBeTruthy();
    local.name = hostName;
    await route.fulfill({ response, json: listing });
  });
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() !== "GET") return route.fallback();
    const response = await route.fetch();
    const listing = await response.json();
    listing.sessions.push(source);
    listing.total += 1;
    await route.fulfill({ response, json: listing });
  });
  await page.route(`**/api/sessions/${SOURCE_ID}`, async (route) => {
    if (route.request().method() !== "GET") return route.fallback();
    await fulfillAsHelm(route, { json: source });
  });
  await page.clock.install();
  await page.goto("/");
  const row = rowByTitle(page, SOURCE_TITLE);
  await expect(row).toBeVisible();
  await row.locator(".session-row-open").click();
  if (state === "interrupted") {
    await expect(page.locator(".interrupted-card")).toBeVisible();
    await expect(page.locator(".session-details .status-badge")).toHaveText("Interrupted");
  } else {
    await expect(page.locator(".interrupted-card")).toHaveCount(0);
    await expect(page.locator(".tab-strip")).toBeVisible();
  }
  await expect(page.locator(".restart-primary")).toHaveAttribute("aria-label", "Resume conversation");
  return row;
}

/**
 * Freeze after any menu's measurement has finished, immediately before the
 * final action. Its own animation-frame placement needs a running clock;
 * only the pending request's time is deliberately held by these tests.
 */
async function freezeForRequest(page: Page) {
  const now = await page.evaluate(() => Date.now());
  await page.clock.pauseAt(new Date(now + 1_000));
}

/**
 * Keep a mutation pending until the test has observed its exact boundary.
 * Releasing gives a real refusal, allowing the same test to distinguish a
 * reply from the production deadline expiring. No supervisor sees this POST.
 */
async function holdMutation(page: Page, operation: "restart" | "replace") {
  const gate = routeGate();
  const bodies: Record<string, unknown>[] = [];
  const stop = await drainedRoute(page, `**/api/sessions/${SOURCE_ID}/${operation}`, async (route) => {
    bodies.push(route.request().postDataJSON());
    await gate.wait();
    await fulfillAsHelm(route, {
      status: 409,
      contentType: "text/plain",
      body: "the host refused this operation",
    });
  });
  return { gate, bodies, stop };
}

/**
 * A held request stays visually quiet before two seconds, then names its
 * original host and counts up. The shared helper is used only after a POST
 * has been observed, so disabled controls alone cannot establish the premise.
 */
async function expectPendingStatus(page: Page, status: Locator, action: string) {
  await page.clock.runFor(1_990);
  await expect(status).toHaveCount(0);
  await page.clock.runFor(110);
  await expect(status).toBeVisible();
  await expect(status).toContainText(`${action} on ${HOST_NAME}`);
  await expect(status).toContainText("2 s");
  const announcement = page.locator(".launch-progress [role=status]");
  await expect(announcement).toHaveCount(1);
  const announced = await announcement.textContent();
  await page.clock.runFor(1_000);
  await expect(status).toContainText("3 s");
  await expect(announcement).toHaveText(announced!);
}

/**
 * Plain Restart reports a slow pending request and keeps a real refusal's
 * text. It must not inherit the launcher's idempotent-create retry promise.
 */
test("plain Restart shows host and elapsed time until a real refusal arrives", async ({ page, request }) => {
  await openSource(page, request);
  const { gate, bodies, stop } = await holdMutation(page, "restart");
  const progress = page.locator(".interrupted-card .launch-progress");
  try {
    await freezeForRequest(page);
    await page.locator(".restart-primary").click();
    await expect.poll(() => bodies.length, { message: "the Restart POST is held" }).toBe(1);
    await expect(page.locator(".restart-primary")).toBeDisabled();
    await expectPendingStatus(page, progress, "Restarting");
    gate.release();
    await expect(progress).toHaveCount(0);
    await expect(page.locator(".restart-error")).toHaveText("the host refused this operation");
    await expect(page.locator(".restart-error")).not.toContainText("Still waiting");
  } finally {
    gate.release();
    await stop();
  }
});

/**
 * A browser-driven 60-second expiry is a transport timeout, rather than a
 * fabricated server refusal or route abort. The action may still finish,
 * and a second Restart has no retained key that could make it harmless.
 */
test("a Restart deadline says to check the session before restarting again", async ({ page, request }) => {
  await openSource(page, request);
  const { gate, bodies, stop } = await holdMutation(page, "restart");
  try {
    await freezeForRequest(page);
    await page.locator(".restart-from-notice").click();
    await expect.poll(() => bodies.length, { message: "the Restart POST is held" }).toBe(1);
    await page.clock.runFor(60_100);
    const error = page.locator(".restart-error");
    await expect(error).toContainText(`Still waiting on ${HOST_NAME}`);
    await expect(error).toContainText(/check the session/i);
    await expect(error).not.toContainText("second copy");
    await expect(page.locator(".interrupted-card .launch-progress")).toHaveCount(0);
    expect(bodies).toHaveLength(1);
    expect(bodies[0]).not.toHaveProperty("intent_key");
  } finally {
    gate.release();
    await stop();
  }
});

type ReplaceSurface = "sidebar" | "header" | "interrupted card";

/** Reach a plain Replace through its own existing consent controls. */
async function submitReplace(page: Page, row: Locator, surface: ReplaceSurface) {
  if (surface === "sidebar") {
    await openRowMenu(row);
    await row.locator(".session-row-replace").click();
    await freezeForRequest(page);
    await row.locator(".confirm-replace").click();
  } else if (surface === "header") {
    await page.locator(".header-replace").click();
    await freezeForRequest(page);
    await page.locator(".header-replace-confirm .btn-danger").click();
  } else {
    await page.locator(".replace-from-notice").click();
    await freezeForRequest(page);
    await page.locator(".replace-confirm-submit").click();
  }
}

for (const surface of ["sidebar", "header", "interrupted card"] as const) {
  /**
   * All Replace adapters use one pending snapshot. A request from any surface
   * appears in the row and open session, then ends when its refusal arrives.
   */
  test(`Replace from the ${surface} shares progress and clears it on a refusal`, async ({ page, request }) => {
    const row = await openSource(page, request);
    const { gate, bodies, stop } = await holdMutation(page, "replace");
    const rowProgress = row.locator(".launch-progress");
    const cardProgress = page.locator(".interrupted-card .launch-progress");
    try {
      await submitReplace(page, row, surface);
      await expect.poll(() => bodies.length, { message: `the ${surface} Replace POST is held` }).toBe(1);
      await expectPendingStatus(page, rowProgress, "Replacing");
      await expect(cardProgress).toContainText(`Replacing on ${HOST_NAME}`);
      await expect(cardProgress).toContainText("3 s");
      expect(bodies[0].intent_key).toEqual(expect.any(String));
      gate.release();
      await expect(rowProgress).toHaveCount(0);
      await expect(cardProgress).toHaveCount(0);
      // Sidebar refusals remain in the open menu and a duplicate row banner;
      // only the menu copy belongs to the interaction under observation.
      const error = surface === "sidebar" ? row.locator(".session-row-menu-panel .action-error") : page.locator(".replace-error");
      await expect(error).toContainText("Replace: the host refused this operation");
      await expect(error).not.toContainText("Still waiting");
    } finally {
      gate.release();
      await stop();
    }
  });
}

/**
 * Timeout preserves plain Replace's page-owned key across surfaces. A retry
 * from the header after a sidebar timeout must reconcile the same replacement.
 */
test("a Replace deadline reports uncertainty and a cross-surface retry keeps its key", async ({ page, request }) => {
  const row = await openSource(page, request);
  const { gate, bodies, stop } = await holdMutation(page, "replace");
  try {
    await submitReplace(page, row, "sidebar");
    await expect.poll(() => bodies.length, { message: "the first Replace POST is held" }).toBe(1);
    await page.clock.runFor(60_100);
    // The sidebar menu owns the visible uncertainty while its row retains
    // another copy for display after the menu closes.
    const error = row.locator(".session-row-menu-panel .action-error");
    await expect(error).toContainText(`Still waiting on ${HOST_NAME}`);
    await expect(error).not.toContainText("Not launched.");
    await expect(row.locator(".launch-progress")).toHaveCount(0);
    await submitReplace(page, row, "header");
    await expect.poll(() => bodies.length, { message: "the cross-surface retry POST is held" }).toBe(2);
    expect(bodies[1].intent_key).toBe(bodies[0].intent_key);
  } finally {
    gate.release();
    await stop();
  }
});

/**
 * A long host must not spend the nonwrapping toolbar's control budget. The
 * native tunnel markup is a CSS preview only; the held Restart and its status
 * are the shipped components. A subthreshold wait must add no notice height.
 */
test("header progress keeps controls inside a 650px pane with a long host", async ({ page, request }, testInfo) => {
  const longHost = "build-host-with-a-deliberately-long-display-name-".repeat(5);
  await openSource(page, request, { state: "unknown", hostName: longHost });
  await page.locator(".app-shell").evaluate(node => node.classList.add("macos-window"));
  const occupied = (await page.locator(".app-main").boundingBox())!.x;
  await page.setViewportSize({ width: Math.round(occupied + 650), height: 700 });
  await page.evaluate(() => document.fonts.ready);
  await mountNativeTunnelPreview(page, "one");
  const topbar = page.locator(".session-topbar");
  const frame = (await topbar.boundingBox())!;
  expect(frame.width, "measure the supported main pane, not the whole window").toBeCloseTo(650, 0);
  const { gate, bodies, stop } = await holdMutation(page, "restart");
  try {
    await freezeForRequest(page);
    await page.locator(".restart-primary").click();
    await expect.poll(() => bodies.length, { message: "the non-interrupted Restart POST is held" }).toBe(1);
    await page.clock.runFor(1_900);
    expect((await page.locator(".session-action-progress").boundingBox())!.height).toBe(0);
    await page.clock.runFor(300);
    const status = page.locator(".session-action-progress .launch-progress");
    await expect(status).toContainText(longHost);
    await expect(status).toContainText("2 s");
    expect((await topbar.boundingBox())!.height).toBe(frame.height);
    for (const selector of [
      '[data-text-size="smaller"]', '[data-text-size="larger"]', ".restart-primary", ".restart-menu-toggle",
      ".header-replace", ".replace-menu-toggle", ".header-clone", ".header-delete", ".session-tunnel-toggle",
    ]) {
      const control = topbar.locator(selector);
      await expect(control).toBeVisible();
      const box = (await control.boundingBox())!;
      expect(box.x, selector).toBeGreaterThanOrEqual(frame.x);
      expect(box.x + box.width, selector).toBeLessThanOrEqual(frame.x + frame.width + 1);
    }
    const bounds = (await status.boundingBox())!;
    expect(bounds.x).toBeGreaterThanOrEqual(frame.x);
    expect(bounds.x + bounds.width).toBeLessThanOrEqual(frame.x + frame.width + 1);
    expect(bounds.y).toBeGreaterThanOrEqual(frame.y + frame.height);
    await testInfo.attach("pending-header-650", { body: await page.locator(".app-main").screenshot(), contentType: "image/png" });
    gate.release();
    await expect(status).toHaveCount(0);
  } finally {
    gate.release();
    await stop();
  }
});

/** The editable launcher also waits on a keyless Restart. Its timeout must
 * keep the draft visible and offer checking the session, never deduplication. */
test("Restart with keeps its draft and avoids a safe-retry promise after a deadline", async ({ page, request }) => {
  await openSource(page, request, { launch: {
    ...agentLaunchRow({ harness: "codex", model: null, effort: null, permissions: "guarded" }),
    resume: ["codex", "resume", "{conversation}", "{farhelm_args}"],
  } });
  await selectSessionActionWith(page, "Restart");
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.locator(".named-conversation").fill("progress-fixture-conversation");
  const { gate, bodies, stop } = await holdMutation(page, "restart");
  try {
    await freezeForRequest(page);
    await form.locator(".create-session-submit").click();
    await expect.poll(() => bodies.length, { message: "the edited Restart POST is held" }).toBe(1);
    await page.clock.runFor(2_200);
    await expect(form.locator(".launch-progress")).toContainText(`Restarting on ${HOST_NAME}`);
    await page.clock.runFor(57_900);
    const alert = form.getByRole("alert");
    await expect(alert).toContainText(`Still waiting on ${HOST_NAME}.`);
    await expect(alert).toContainText("Check the session before restarting again.");
    await expect(alert).not.toContainText("second copy");
    await expect(alert).not.toContainText("Restart error.");
    expect(bodies[0]).not.toHaveProperty("intent_key");
  } finally {
    gate.release();
    await stop();
  }
});
