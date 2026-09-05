// Agent profiles in a real browser: the app bar's helm-wide CRUD popup, the
// create dialog's shared picker and its ask-don't-guess
// fallback, and SPEC.md's snapshot rule as the session list shows it.
//
// A per-area spec of its own, per this milestone's convention (see
// feed.spec.ts's and sidebar.spec.ts's headers). Its helpers are the shared
// ones in helpers/fleet.ts, extended there with the profile CRUD calls these
// fixtures need.
//
// ## Most tests stub the feed; one deliberately does not
//
// Stubbed for filters.spec.ts's reason: the shared stack's other sessions keep
// changing status, and each change is a revision bump that would re-read — and
// therefore repaint — underneath the assertions below at unpredictable
// moments. A silent feed makes each test's DOM change exactly when the test
// asks it to, which is also what lets the invalidation tests prove that a
// notification is what moved a surface rather than a poll that happened to
// land.
//
// The exception is the two-client test, whose whole subject is the real path
// end to end: a real second browser, a real edit through the UI, a real feed
// on the observer, and no injected notification anywhere. A stub cannot detect
// a mutation that fails to publish, which is precisely what that test is for.
//
// ## Every test cleans its profiles up, and registers them before asserting
//
// Cleanup is not tidiness. A successful profile-backed create writes that
// helm's REMEMBERED DEFAULT, which every later create dialog — in this file and
// in every other spec — then answers for. Neither outcome is neutral: a LIVE
// remembered profile is preselected (so the command field is disabled), and a
// DELETED one leaves the dialog selecting nothing and blocking the create until
// somebody answers. Both would break a spec that means to type a command, which
// is why every create helper in this suite now states "custom command"
// explicitly rather than relying on whatever the stack was left in — and why
// these fixtures still clean up after themselves, so what is left behind is at
// least a state the suite has a name for.
//
// Registration happens as soon as the object EXISTS server-side, before any
// assertion about the page — a repaint that never happens must not leak a
// profile or a session into a stack every later run shares.
import { APIRequestContext, expect, Page, Route, test } from "@playwright/test";
import {
  cleanupProfile,
  cleanupSession,
  createProfile,
  createSession,
  FAKE_AGENT,
  listHosts,
  listProfiles,
  listSessions,
  localHostId,
  openHostMenu,
  openHostsPanel,
  openRowMenu,
  ProfileRow,
  stubFeed,
  updateProfile,
} from "./helpers/fleet";
import type { FeedStub } from "./helpers/fleet";
import { sharedSessionRow } from "./helpers/terminal-suite";

/** The value of the picker's placeholder — `profiles::UNRESOLVED_VALUE`, the
 * option a dialog shows while nothing is selected and a create is blocked. */
const UNRESOLVED = "__unresolved__";

/** The row for one session id, as the list renders it. */
function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

/** The helm-wide profiles popup opened from the sidebar app bar. */
function section(page: Page) {
  return page.locator(".profiles-popover");
}

/** One profile's row inside the popup. */
function profileRow(page: Page, id: string) {
  return section(page).locator(`[data-profile-id="${id}"]`);
}

/**
 * Load the list with a stubbed, healthy feed, and hand the stub back.
 *
 * The handshake is played explicitly (`notify`) rather than armed, because
 * every test here wants the page HEALTHY from the start: an ungreeted socket
 * is torn down and reopened on the client's own schedule, and the fallback
 * poll would then be running underneath assertions about what a notification
 * caused.
 */
/**
 * Fulfil a held route with a helm-style refusal that still carries the
 * helm's build stamp.
 *
 * A fabricated reply without `x-farhelm-build` reads to the page as a helm
 * that predates the stamp: it latches version skew, raises the skew banner
 * at the top of the sidebar, and that banner is a layout change which closes
 * the popup the test is watching. Whether the banner lands before or after
 * the popup opens is scheduling, which is how an unstamped refusal turned
 * into an engine-dependent flake rather than a steady failure.
 */
async function fulfillRefusal(
  route: Route,
  request: APIRequestContext,
  body: string,
  status = 409,
): Promise<void> {
  const probe = await request.get("/api/profiles");
  const build = probe.headers()["x-farhelm-build"] ?? "";
  expect(build, "fabricated API replies must retain the helm build stamp").toBeTruthy();
  await route.fulfill({
    status,
    body,
    headers: { "content-type": "text/plain", "x-farhelm-build": build },
  });
}

/**
 * A point inside the sidebar that a real click can reach and that focuses
 * nothing: outside the profiles popup, and not on any control.
 *
 * The candidates are the two ends of the session-count line and the
 * sidebar's bottom-left corner. Which of them the popup covers depends on
 * how many host rows sit above the session header, so the choice is made at
 * click time from the live geometry rather than fixed in the test.
 */
async function inertSidebarPoint(page: Page): Promise<{ x: number; y: number }> {
  const point = await page.evaluate(() => {
    const popover = document.querySelector(".profiles-popover")?.getBoundingClientRect();
    const count = document.querySelector(".session-count")?.getBoundingClientRect();
    const sidebar = document.querySelector(".app-sidebar")?.getBoundingClientRect();
    const candidates: [number, number][] = [];
    if (count) {
      candidates.push([count.left + 2, count.top + 2], [count.right - 2, count.top + 2]);
    }
    if (sidebar) candidates.push([sidebar.left + 4, sidebar.bottom - 4]);
    for (const [x, y] of candidates) {
      const covered = popover
        && x >= popover.left && x <= popover.right && y >= popover.top && y <= popover.bottom;
      if (covered) continue;
      const target = document.elementFromPoint(x, y);
      if (!target || target.closest(".profiles-popover")) continue;
      if (target.closest("button, a, input, select, textarea, [tabindex]")) continue;
      return { x, y };
    }
    return null;
  });
  expect(point, "the sidebar must offer one inert, uncovered spot").not.toBeNull();
  return point!;
}

async function listWithStubbedFeed(page: Page): Promise<FeedStub> {
  const feed = await stubFeed(page);
  await page.goto("/");
  await feed.waitForConnection(1);
  feed.notify(1);
  // Arrival check: the first always-visible host row proves the hosts read landed.
  await expect(page.locator(".host-row").first()).toBeVisible({ timeout: 20_000 });
  return feed;
}

/** Open the helm-wide popup from the app bar and wait for its catalog. */
async function openProfiles(page: Page) {
  await page.locator(".profiles-toggle").click();
  await expect(section(page)).toBeVisible({ timeout: 20_000 });
}

/** A focus-out scenario starts only after opening has placed focus inside.
 * Visibility precedes that asynchronous handoff. Moving straight from the
 * toggle to an outside control emits no popup focus-out event at all. */
async function openFocusedProfiles(page: Page) {
  await openProfiles(page);
  await expect(section(page).locator(".new-profile-button")).toBeFocused();
}

/** Close the popup with the same app-bar toggle that opened it. */
async function closeProfiles(page: Page) {
  await page.locator(".profiles-toggle").click();
  await expect(section(page)).toHaveCount(0, { timeout: 20_000 });
}

/**
 * Start a real profile save and hold its helm request after the operation lock
 * is claimed. Provenance tests use this shared boundary so only the event that
 * follows the claim differs between their otherwise identical busy windows.
 */
async function beginHeldProfileSave(page: Page, profile: ProfileRow) {
  let release: (() => void) | undefined;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route(
    (url) => new RegExp(`/api/profiles/${profile.id}$`).test(url.pathname),
    async (route: Route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }
      await held;
      await route.continue();
    },
  );
  const profileRowLocator = profileRow(page, profile.id);
  await profileRowLocator.locator(".profile-edit").click();
  await profileRowLocator.locator(".profile-name-input").fill(`${profile.name}-saved`);
  await profileRowLocator.locator(".profile-save").click();
  await expect(profileRowLocator.locator(".profile-save")).toBeDisabled();
  return {
    row: profileRowLocator,
    release: () => release!(),
  };
}

/** Open the create dialog and wait for its agent picker. */
async function openCreateDialog(page: Page) {
  await page.locator(".new-session-button").click();
  await expect(page.locator(".create-session-profile")).toBeVisible({ timeout: 20_000 });
}

/** Wait until the picker offers one profile from the shared helm catalog. */
async function waitForOption(page: Page, id: string) {
  await expect(page.locator(`.create-session-profile option[value="${id}"]`)).toHaveCount(1, {
    timeout: 20_000,
  });
}

/**
 * Hold exactly one reveal for the primary terminal at its real replay marker.
 * Scoping the global test hook to `#terminal` keeps later tab mounts and
 * remounts from accidentally consuming this test's lifecycle gate.
 */
async function holdPrimaryTerminalReveal(page: Page) {
  await page.addInitScript(() => {
    (window as any).__farhelmTestReplay = {
      holdMarker: true,
      targetEl: "terminal",
      idleMs: 60_000,
    };
  });
}

/** Wait until the selected primary terminal has reached the held marker. */
async function waitForHeldPrimaryReveal(page: Page) {
  await expect
    .poll(
      () =>
        page.evaluate(
          () => (window as any).__farhelmIslands?.terminal?.test?.replay?.heldReason ?? null,
        ),
      { timeout: 60_000 },
    )
    .toBe("marker");
}

/** Release the primary terminal through terminal.js's production reveal path. */
async function releasePrimaryReveal(page: Page) {
  await page.evaluate(() => (window as any).__farhelmIslands.terminal.test.releaseCatchUp());
}

/**
 * Observe the page's REAL feed sockets without interfering with them.
 *
 * A route would replace the socket, which is the one thing the two-client test
 * must not do; Playwright's `websocket` event only reports. What it makes
 * checkable is the attribution the test would otherwise have to assume: that
 * the observer was subscribed before the change was made, and that no
 * reconnect happened in between — a reconnect's own handshake triggers a
 * re-read, so an update seen across one says nothing about publication.
 */
function watchFeedSockets(page: Page): {
  opened(): number;
  closed(): number;
  greeted(): number;
} {
  let opened = 0;
  let closed = 0;
  let greeted = 0;
  page.on("websocket", (socket) => {
    if (!socket.url().includes("/api/events")) return;
    opened += 1;
    // The `websocket` event fires when the page ASKS for a socket, which is
    // not the same as being subscribed — the upgrade may still be in flight,
    // and a page that has not been greeted yet will re-read on its handshake
    // whether or not anything was published. Counting received FRAMES is what
    // establishes the subscription: the helm answers every (re)subscription
    // with the current revision immediately, so the first frame IS the proof.
    socket.on("framereceived", () => {
      greeted += 1;
    });
    socket.on("close", () => {
      closed += 1;
    });
  });
  return { opened: () => opened, closed: () => closed, greeted: () => greeted };
}

/** The bodies of every create this page POSTs, in order — what a claim about
 * idempotency keys and creation modes has to be made against, since neither is
 * visible in the DOM. */
async function watchCreateBodies(page: Page): Promise<Record<string, unknown>[]> {
  const bodies: Record<string, unknown>[] = [];
  await page.route(
    (url) => url.pathname === "/api/sessions",
    async (route: Route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }
      bodies.push(route.request().postDataJSON());
      await route.continue();
    },
  );
  return bodies;
}

test.describe("agent profiles", () => {
  const created: string[] = [];
  const profiles: string[] = [];

  test.afterEach(async ({ request }) => {
    while (created.length) {
      const id = created.pop();
      if (id) await cleanupSession(request, id);
    }
    // After the sessions, deliberately: a session outliving its profile is
    // fine (that is the snapshot rule), and deleting the profile first would
    // leave the rows this suite creates describing a deleted profile for the
    // moment in between — which is a state some other spec could then read.
    while (profiles.length) {
      const profile = profiles.pop();
      if (profile) await cleanupProfile(request, profile);
    }
  });

  /**
   * Find the profile this test just created through the UI and register it for
   * cleanup, before anything is asserted about the page.
   *
   * The order is the point. A profile that exists server-side and is not
   * registered is a leak the moment any later assertion fails, and it is not a
   * harmless one: a live profile changes what every later create dialog — in
   * this file and in every other spec — preselects.
   */
  async function registerByName(
    request: Parameters<typeof listProfiles>[0],
    name: string,
  ): Promise<ProfileRow> {
    let found: ProfileRow | undefined;
    await expect
      .poll(
        async () => {
          found = (await listProfiles(request)).profiles.find(
            (profile) => profile.name === name,
          );
          return found?.id;
        },
        { timeout: 20_000, message: `the profile ${name} must reach the helm catalog` },
      )
      .toBeTruthy();
    profiles.push(found!.id);
    return found!;
  }

  /**
   * The app-bar trigger opens the helm-wide popup with every seeded starter.
   * This pins both the new entry point and the fact that management consumes
   * the same complete catalog as session creation.
   */
  test("the app-bar popup lists all four starter profiles", async ({ page }) => {
    await listWithStubbedFeed(page);
    await openProfiles(page);
    await expect(section(page).locator("[data-profile-id]")).toHaveCount(4);
    await expect(page.locator(".profiles-toggle")).toHaveAttribute("aria-expanded", "true");
  });

  /**
   * Opening the popup is an attended retry even after the first catalog read
   * failed and latched build skew. Background work must stand down in that
   * state, but a person reopening a consumer must still be able to recover
   * once the route answers again.
   */
  test("opening profiles retries a failed mount read under latched skew", async ({
    page,
    request,
  }) => {
    const profile = await createProfile(request, { name: `retry-${Date.now()}` });
    profiles.push(profile.id);
    let reads = 0;
    await page.route(
      (url) => url.pathname === "/api/profiles",
      async (route: Route) => {
        if (route.request().method() !== "GET") {
          await route.continue();
          return;
        }
        reads += 1;
        if (reads === 1) {
          await route.fulfill({
            status: 503,
            headers: {
              "content-type": "text/plain",
              "x-farhelm-build": "intentionally-mismatched-build",
            },
            body: "catalog temporarily unavailable",
          });
          return;
        }
        await route.continue();
      },
    );

    await page.goto("/");
    await expect.poll(() => reads, { timeout: 20_000 }).toBe(1);
    await expect(page.locator(".build-skew")).toBeVisible({ timeout: 20_000 });
    await openProfiles(page);
    await expect(profileRow(page, profile.id)).toBeVisible({ timeout: 20_000 });
    expect(reads, "the open transition must issue an attended retry").toBeGreaterThanOrEqual(2);
  });

  /**
   * The create form has the same attended-retry contract as the management
   * popup. This separately pins its closed-to-open edge so recovery cannot be
   * accidentally left only on the app-bar entry point.
   */
  test("opening create retries a failed mount read under latched skew", async ({
    page,
    request,
  }) => {
    const profile = await createProfile(request, { name: `retry-create-${Date.now()}` });
    profiles.push(profile.id);
    let reads = 0;
    await page.route(
      (url) => url.pathname === "/api/profiles",
      async (route: Route) => {
        if (route.request().method() !== "GET") {
          await route.continue();
          return;
        }
        reads += 1;
        if (reads === 1) {
          await route.fulfill({
            status: 503,
            headers: {
              "content-type": "text/plain",
              "x-farhelm-build": "intentionally-mismatched-build",
            },
            body: "catalog temporarily unavailable",
          });
          return;
        }
        await route.continue();
      },
    );

    await page.goto("/");
    await expect.poll(() => reads, { timeout: 20_000 }).toBe(1);
    await expect(page.locator(".build-skew")).toBeVisible({ timeout: 20_000 });
    await openCreateDialog(page);
    await waitForOption(page, profile.id);
    expect(reads, "the create open transition must issue an attended retry")
      .toBeGreaterThanOrEqual(2);
  });

  /**
   * A refresh requested by a short-lived popup belongs to the page reader.
   * Starting from idle, this holds that refresh, unmounts its requester, and
   * specifies exactly one serialized follow-up with no duplicate third walk.
   */
  test("the popup and create picker share one always-active catalog reader", async ({
    page,
    request,
  }) => {
    let releaseInitial: (() => void) | undefined;
    const initial = new Promise<void>((resolve) => {
      releaseInitial = resolve;
    });
    let releasePopup: (() => void) | undefined;
    const popupRead = new Promise<void>((resolve) => {
      releasePopup = resolve;
    });
    let reads = 0;
    let active = 0;
    let maximumActive = 0;
    let holdPopupRead = false;
    let popupReadHeld = false;
    await page.route(
      (url) => url.pathname === "/api/profiles",
      async (route: Route) => {
        if (route.request().method() !== "GET") {
          await route.continue();
          return;
        }
        reads += 1;
        active += 1;
        maximumActive = Math.max(maximumActive, active);
        const response = await route.fetch();
        if (reads === 1) await initial;
        if (holdPopupRead && !popupReadHeld) {
          popupReadHeld = true;
          await popupRead;
        }
        await route.fulfill({ response });
        active -= 1;
      },
    );

    await listWithStubbedFeed(page);
    await expect.poll(() => reads, { timeout: 20_000 }).toBe(1);
    expect(active, "the controlled mount read must still own the reader").toBe(1);
    releaseInitial!();
    await expect.poll(() => active, { timeout: 20_000 }).toBe(0);
    const baseline = reads;
    expect(active, "the baseline must be idle before the lifecycle boundary").toBe(0);
    holdPopupRead = true;
    await openProfiles(page);
    await expect.poll(() => popupReadHeld, { timeout: 20_000 }).toBe(true);
    expect(reads).toBe(baseline + 1);
    await page.locator(".profiles-toggle").click();
    await expect(section(page)).toHaveCount(0);

    const profile = await createProfile(request, { name: `shared-reader-${Date.now()}` });
    profiles.push(profile.id);
    await openCreateDialog(page);
    await expect(page.locator(".create-session-profile")).toBeVisible({ timeout: 20_000 });
    expect(reads, "the surviving consumer queues behind the held requester refresh")
      .toBe(baseline + 1);

    releasePopup!();
    await waitForOption(page, profile.id);
    expect(maximumActive, "the shared surface permits only one catalog GET at a time").toBe(1);
    await expect.poll(() => reads, { timeout: 20_000 }).toBe(baseline + 2);
    await expect.poll(() => active, { timeout: 20_000 }).toBe(0);
    expect(reads, "the coalesced follow-up drains demand without a third post-baseline GET")
      .toBe(baseline + 2);
  });

  /** The popup is a keyboard-dismissable transient surface: its first action
   * receives focus, Escape returns focus to the trigger, and moving focus
   * outside closes it without stealing that destination. */
  test("the profiles popup follows its focus and Escape dismissal contract", async ({ page }) => {
    await listWithStubbedFeed(page);
    const toggle = page.locator(".profiles-toggle");
    await openFocusedProfiles(page);

    await page.keyboard.press("Escape");
    await expect(section(page)).toHaveCount(0);
    await expect(toggle).toBeFocused();

    await openFocusedProfiles(page);
    const destination = page.locator(".host-details-toggle");
    await destination.focus();
    await expect(section(page)).toHaveCount(0);
    await expect(destination).toBeFocused();
  });

  /**
   * A delayed in-popup request cannot replace a control chosen outside, and a
   * superseded request cannot wake during the render gap and apply afterwards.
   * The test hook hides only the target lookup; Rust still owns the production
   * retry and cancellation loop.
   */
  test("delayed and superseded profile focus requests preserve newer intent", async ({ page }) => {
    await listWithStubbedFeed(page);
    await openProfiles(page);
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles = { hideFocusTarget: true };
    });
    await section(page).locator(".new-profile-button").click();
    const outside = page.locator(".host-details-toggle");
    await outside.focus();
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles.hideFocusTarget = false;
    });
    await expect(section(page)).toHaveCount(0, { timeout: 20_000 });
    await expect(outside).toBeFocused();

    await openProfiles(page);
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles.hideFocusTarget = true;
    });
    await section(page).locator(".new-profile-button").click();
    await section(page).locator(".profile-cancel").dispatchEvent("click");
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles.hideFocusTarget = false;
    });
    await expect(section(page).locator(".new-profile-button")).toBeFocused();
  });

  /**
   * Browser-evaluation latency is part of the 250 ms wall-clock budget. The
   * target remains one connected node throughout, so focus proves two delayed
   * observations fit inside the deadline rather than succeeding after it.
   */
  test("profile focus counts delayed evaluations against one deadline", async ({ page }) => {
    await listWithStubbedFeed(page);
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles = {
        focusEvalDelayMs: 90,
        focusAttempts: 0,
      };
    });
    await page.locator(".profiles-toggle").click();
    const target = section(page).locator(".new-profile-button");
    await expect(target).toBeAttached();
    const original = await target.elementHandle();
    expect(original).not.toBeNull();
    await expect(target).toBeFocused({ timeout: 2_000 });
    const timing = await page.evaluate(() => {
      const testState = (window as any).__farhelmTestProfiles;
      return {
        attempts: testState.focusAttempts,
        elapsed: testState.focusedAt - testState.focusStartedAt,
      };
    });
    expect(timing.attempts).toBe(2);
    expect(timing.elapsed).toBeLessThanOrEqual(250);
    expect(await original!.evaluate((node) => node.isConnected)).toBe(true);
    expect(await original!.evaluate((node) => document.activeElement === node)).toBe(true);
  });

  /**
   * One observation that outlives the 250 ms request budget is consumed as
   * unknown. Its late JavaScript continuation has no focus side effect, so it
   * cannot place focus after Rust has stopped owning the request.
   */
  test("an overdue focus observation cannot focus late", async ({ page }) => {
    await listWithStubbedFeed(page);
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles = {
        focusEvalDelayMs: 400,
        focusAttempts: 0,
      };
    });
    await page.locator(".profiles-toggle").click();
    const target = section(page).locator(".new-profile-button");
    await expect(target).toBeAttached();
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.focusSettled)
    ).toBe("unknown");
    await page.waitForTimeout(250);
    await expect(target).not.toBeFocused();
    expect(await page.evaluate(() => (window as any).__farhelmTestProfiles.focusAttempts)).toBe(1);
    expect(await page.evaluate(() => (window as any).__farhelmTestProfiles.focusedAt)).toBeUndefined();
  });

  /**
   * The renderer checks the request's absolute browser deadline immediately
   * before focus. Even though a delayed commit keeps running after Rust times
   * out, it expires without moving focus when it finally resumes.
   */
  test("an overdue focus commit expires before its side effect", async ({ page }) => {
    await listWithStubbedFeed(page);
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles = {
        focusBrowserBudgetMs: 100,
        focusCommitDelayMs: 120,
        focusCommitAttempts: 0,
      };
    });
    await page.locator(".profiles-toggle").click();
    const target = section(page).locator(".new-profile-button");
    await expect(target).toBeAttached();
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.focusCommitAttempts)
    ).toBe(1);
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.focusSettled)
    ).toBe("unknown");
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.focusCommitExpired)
    ).toBe(true);
    await expect(target).not.toBeFocused();
    expect(await page.evaluate(() => (window as any).__farhelmTestProfiles.focusedAt)).toBeUndefined();
  });

  /**
   * An unknown attempt breaks the stable-node handshake. Found, error, Found
   * is not consecutive evidence, so the same connected target needs a fourth
   * observation before it may receive focus.
   */
  test("an unknown focus attempt breaks consecutive target observations", async ({ page }) => {
    await listWithStubbedFeed(page);
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles = {
        focusAttempts: 0,
        focusEvalErrorAttempts: [2],
      };
    });
    await page.locator(".profiles-toggle").click();
    const target = section(page).locator(".new-profile-button");
    await expect(target).toBeAttached();
    const original = await target.elementHandle();
    expect(original).not.toBeNull();
    await expect(target).toBeFocused({ timeout: 2_000 });
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.focusAttempts)
    ).toBe(4);
    expect(await original!.evaluate((node) => node.isConnected)).toBe(true);
  });

  /**
   * Renderer errors and timeouts are absence of evidence, not proof of
   * document transit. Persistent focus-placement failures and an overdue
   * classifier consume bounded work without dismissing or stealing focus.
   */
  test("focus evaluation errors never become dismissal evidence", async ({ page }) => {
    await listWithStubbedFeed(page);
    await openProfiles(page);
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles = { focusEvalErrors: 100 };
    });
    await section(page).locator(".new-profile-button").click();
    await expect(section(page)).toBeVisible();
    await page.waitForTimeout(400);
    await expect(section(page)).toBeVisible();

    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles = { classification: { delayMs: 500 } };
    });
    const outside = page.locator(".host-details-toggle");
    await outside.focus();
    await page.waitForTimeout(500);
    await expect(section(page)).toBeVisible();
    await expect(outside).toBeFocused();
  });

  /**
   * A failed classification followed by document transit still waits for the
   * popup's live focus request. Unknown evidence cannot skip that settlement
   * loop and turn `body` into an early dismissal destination. The delayed
   * placement observation ends without evidence when its deadline expires;
   * merely hiding a known target would instead settle as Missing and permit
   * transit dismissal once that request was no longer pending.
   */
  test("unknown then transit waits for the pending focus request", async ({ page }) => {
    await listWithStubbedFeed(page);
    await openFocusedProfiles(page);
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles = {
        hideFocusTarget: true,
        focusEvalDelayMs: 500,
        focusAttempts: 0,
        classificationErrors: 1,
        classificationAttempts: 0,
      };
    });
    await section(page).locator(".new-profile-button").click();
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.focusAttempts)
    ).toBeGreaterThanOrEqual(1);
    await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.classificationAttempts)
    ).toBeGreaterThanOrEqual(2);
    await page.waitForTimeout(400);
    // Verify the uncertainty branch itself: a hidden target that settled as
    // Missing would test a different dismissal contract even if still visible.
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.focusSettled)
    ).toBe("unknown");
    await expect(section(page)).toBeVisible();
    await expect.poll(() => page.evaluate(() => document.activeElement === document.body))
      .toBe(true);
  });

  /**
   * Focus-out tasks may finish out of order within one opening or after a new
   * opening. Full obligation tokens prevent either stale classifier from
   * clearing the newer dismissal it does not own.
   */
  test("stale focus-out classifiers cannot clear newer obligations", async ({ page }) => {
    await listWithStubbedFeed(page);
    await openFocusedProfiles(page);
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles = {
        classification: { holds: 2, started: 0, releases: [] },
      };
    });
    const outside = page.locator(".host-details-toggle");
    await outside.focus();
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.classification.started)
    ).toBe(1);
    await section(page).locator(".new-profile-button").focus();
    await outside.focus();
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.classification.started)
    ).toBe(2);
    await page.evaluate(() => (window as any).__farhelmTestProfiles.classification.releases.shift()());
    await expect(section(page)).toBeVisible();
    await page.evaluate(() => (window as any).__farhelmTestProfiles.classification.releases.shift()());
    await expect(section(page)).toHaveCount(0, { timeout: 20_000 });

    await openFocusedProfiles(page);
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles.classification = { holds: 2, started: 0, releases: [] };
    });
    await outside.focus();
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.classification.started)
    ).toBe(1);
    await page.locator(".profiles-toggle").click();
    await openFocusedProfiles(page);
    await outside.focus();
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.classification.started)
    ).toBe(2);
    await page.evaluate(() => (window as any).__farhelmTestProfiles.classification.releases.shift()());
    await expect(section(page)).toBeVisible();
    await page.evaluate(() => (window as any).__farhelmTestProfiles.classification.releases.shift()());
    await expect(section(page)).toHaveCount(0, { timeout: 20_000 });
  });

  /**
   * A background terminal reveal must not pull focus through a mounted popup.
   * Holding the real replay marker specifies that the popup's focused entry
   * control remains active when the selected terminal becomes visible.
   */
  test("a held terminal reveal does not steal focus from profiles", async ({ page }) => {
    await holdPrimaryTerminalReveal(page);
    await listWithStubbedFeed(page);
    await sharedSessionRow(page).click();
    await waitForHeldPrimaryReveal(page);
    await openProfiles(page);
    const popupFocus = section(page).locator(".new-profile-button");
    await expect(popupFocus).toBeFocused();

    await releasePrimaryReveal(page);
    await expect
      .poll(() =>
        page.evaluate(
          () => (window as any).__farhelmIslands.terminal.test.replay.revealed,
        ),
      )
      .toBe(true);
    await expect(section(page)).toBeVisible();
    await expect(popupFocus).toBeFocused();
  });

  /**
   * A reveal vetoed by the popup is not retained for dismissal. Escape keeps
   * focus on the popup toggle; the terminal accepts focus only after the user
   * explicitly clicks it.
   */
  test("a popup-vetoed terminal reveal requires a later click", async ({ page }) => {
    await holdPrimaryTerminalReveal(page);
    await listWithStubbedFeed(page);
    await sharedSessionRow(page).click();
    await waitForHeldPrimaryReveal(page);
    await openProfiles(page);
    await releasePrimaryReveal(page);
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmIslands.terminal.test.replay.revealed)
    ).toBe(true);
    await page.keyboard.press("Escape");
    await expect(section(page)).toHaveCount(0);
    await expect(page.locator(".profiles-toggle")).toBeFocused();
    await expect
      .poll(() => page.evaluate(() => document.activeElement?.closest("#terminal") !== null))
      .toBe(false);

    await page.locator("#terminal").click();
    await expect
      .poll(() => page.evaluate(() => document.activeElement?.closest("#terminal") !== null))
      .toBe(true);
  });

  /**
   * An inert sidebar click closes profiles after focus remains on the document
   * body, matching the adjacent filter popover without stealing focus back to
   * the profiles toggle.
   */
  test("an inert sidebar click dismisses the profiles popup", async ({ page }) => {
    await listWithStubbedFeed(page);
    await openProfiles(page);

    // A real click, because synthetic dispatch would not move focus; on a spot
    // chosen from live geometry, because which part of the sidebar the popup
    // leaves uncovered depends on how many host rows sit above the header.
    const { x, y } = await inertSidebarPoint(page);
    await page.mouse.click(x, y);
    await expect.poll(() => page.evaluate(() => document.activeElement === document.body))
      .toBe(true);
    await expect(section(page)).toHaveCount(0);
    await expect(page.locator(".profiles-toggle")).not.toBeFocused();
  });

  /**
   * Every state change inside the popup names the next keyboard position.
   *
   * These assertions protect the transitions that remove the focused node:
   * entering and leaving an editor, saving it, and deleting a row. Without an
   * explicit successor, browsers fall back to the document body and Escape is
   * no longer reachable from the still-open popup.
   */
  test("profile form transitions keep keyboard focus inside the popup", async ({
    page,
    request,
  }) => {
    const first = await createProfile(request, { name: `focus-a-${Date.now()}` });
    profiles.push(first.id);
    const second = await createProfile(request, { name: `focus-b-${Date.now()}` });
    profiles.push(second.id);

    await listWithStubbedFeed(page);
    await openProfiles(page);
    const firstRow = profileRow(page, first.id);

    await firstRow.locator(".profile-edit").click();
    await expect(firstRow.locator(".profile-name-input")).toBeFocused();
    await firstRow.locator(".profile-cancel").click();
    await expect(firstRow.locator(".profile-edit")).toBeFocused();

    await firstRow.locator(".profile-edit").click();
    await firstRow.locator(".profile-name-input").fill(`${first.name}-saved`);
    await firstRow.locator(".profile-save").click();
    await expect(firstRow.locator(".profile-edit")).toBeFocused({ timeout: 20_000 });

    const catalog = (await listProfiles(request)).profiles;
    const firstIndex = catalog.findIndex((profile) => profile.id === first.id);
    const next = catalog[firstIndex + 1];
    expect(next, "the first fixture must have a following row for the delete focus check")
      .toBeTruthy();
    await firstRow.locator(".profile-delete").click();
    await expect(firstRow.locator(".profile-cancel-delete")).toBeFocused();
    await firstRow.locator(".profile-confirm-delete").click();
    await expect(profileRow(page, next.id).locator(".profile-edit")).toBeFocused({
      timeout: 20_000,
    });
  });

  /**
   * The popup's measured border box, not only its content area, stays inside a
   * constrained viewport. Padding and borders used to extend past the inline
   * maximums because the popup inherited content-box sizing.
   */
  test("the profiles popup border box stays inside a constrained viewport", async ({ page }) => {
    await page.setViewportSize({ width: 260, height: 220 });
    await listWithStubbedFeed(page);
    await openProfiles(page);

    const box = (await section(page).boundingBox())!;
    expect(box.x).toBeGreaterThanOrEqual(8);
    expect(box.y).toBeGreaterThanOrEqual(8);
    expect(box.x + box.width).toBeLessThanOrEqual(252);
    expect(box.y + box.height).toBeLessThanOrEqual(212);
  });

  /**
   * A layout event from the closed state must not invalidate a fresh opening.
   * This delivers a sidebar scroll and activation in one browser turn, then
   * specifies that only a later scroll closes the measured popup and restores
   * its toggle.
   */
  test("only layout changes after a profiles opening invalidate its geometry", async ({ page }) => {
    await listWithStubbedFeed(page);
    await page.evaluate(() => {
      document.querySelector(".app-sidebar")?.dispatchEvent(new Event("scroll"));
      const toggle = document.querySelector(".profiles-toggle") as HTMLButtonElement;
      toggle.focus();
      toggle.click();
    });
    await expect(section(page)).toBeVisible();
    await expect(section(page).locator(".new-profile-button")).toBeFocused();

    await page.evaluate(() =>
      document.querySelector(".app-sidebar")?.dispatchEvent(new Event("scroll"))
    );
    await expect(section(page)).toHaveCount(0);
    await expect(page.locator(".profiles-toggle")).toBeFocused();
  });

  /**
   * A rectangle sampled before a layout epoch change is never accepted. The
   * measurement gate holds the sampled value across a real sidebar scroll, so
   * the second measurement is observable rather than inferred from timing.
   */
  test("profile placement retries a rectangle made stale while awaiting acceptance", async ({
    page,
  }) => {
    await listWithStubbedFeed(page);
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles = {
        measurement: { holds: 2, started: 0 },
      };
    });
    await page.locator(".profiles-toggle").click();
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.measurement.started)
    ).toBe(1);
    await page.evaluate(() => {
      document.querySelector(".app-sidebar")?.dispatchEvent(new Event("scroll"));
      (window as any).__farhelmTestProfiles.measurement.release();
    });
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.measurement.started)
    ).toBe(2);
    await page.evaluate(() => (window as any).__farhelmTestProfiles.measurement.release());
    await expect(section(page)).toBeVisible();
  });

  /**
   * Creating before the first catalog answer cannot target a row that is not
   * renderable yet. The stable New Profile control receives focus and keeps
   * the popup open until the released catalog makes the accepted row visible.
   */
  test("a create before the initial catalog answer uses the stable focus fallback", async ({
    page,
    request,
  }) => {
    let release: (() => void) | undefined;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route(
      (url) => url.pathname === "/api/profiles",
      async (route: Route) => {
        if (route.request().method() === "GET") {
          await held;
        }
        await route.continue();
      },
    );

    await listWithStubbedFeed(page);
    await openProfiles(page);
    await section(page).locator(".new-profile-button").click();
    const name = `pending-create-${Date.now()}`;
    const form = section(page).locator(".profile-form");
    await form.locator(".profile-name-input").fill(name);
    await form.locator(".profile-invocation-input").fill(FAKE_AGENT);
    await form.locator(".profile-save").click();

    const stored = await registerByName(request, name);
    profiles.push(stored.id);
    await expect(form).toHaveCount(0, { timeout: 20_000 });
    await expect(section(page).locator(".new-profile-button")).toBeFocused();
    await page.waitForTimeout(400);
    await expect(section(page)).toBeVisible();

    release!();
    await expect(profileRow(page, stored.id)).toBeVisible({ timeout: 20_000 });
  });

  /**
   * A differing confirmation that changes only a peer profile establishes
   * whether Dioxus preserves an already-focused control in a keyed target row.
   * The answer decides whether production needs any reconciliation replay at
   * all, so this test records both node identity and focus after the patch.
   */
  test("a differing catalog preserves focus in an unchanged keyed profile row", async ({
    page,
    request,
  }) => {
    const target = await createProfile(request, { name: `keyed-target-${Date.now()}` });
    profiles.push(target.id);
    let releaseRead: (() => void) | undefined;
    const heldRead = new Promise<void>((resolve) => {
      releaseRead = resolve;
    });
    let holdNextRead = false;
    let readHeld = false;
    await page.route(
      (url) => url.pathname === "/api/profiles",
      async (route: Route) => {
        if (route.request().method() !== "GET" || !holdNextRead) {
          await route.continue();
          return;
        }
        holdNextRead = false;
        readHeld = true;
        await heldRead;
        await route.continue();
      },
    );

    await listWithStubbedFeed(page);
    await openProfiles(page);
    const targetRow = profileRow(page, target.id);
    await targetRow.locator(".profile-edit").click();
    await targetRow.locator(".profile-name-input").fill(`${target.name}-saved`);
    holdNextRead = true;
    await targetRow.locator(".profile-save").click();
    await expect.poll(() => readHeld).toBe(true);
    const originalEdit = await targetRow.locator(".profile-edit").elementHandle();
    expect(originalEdit).not.toBeNull();
    await expect(targetRow.locator(".profile-edit")).toBeFocused();

    const peer = await createProfile(request, { name: `keyed-peer-${Date.now()}` });
    profiles.push(peer.id);
    releaseRead!();
    await expect(profileRow(page, peer.id)).toBeVisible({ timeout: 20_000 });
    expect(await originalEdit!.evaluate((node) => node.isConnected)).toBe(true);
    expect(await originalEdit!.evaluate((node) => document.activeElement === node)).toBe(true);
  });

  /**
   * The whole CRUD round trip, driven through the panel: define a profile,
   * edit it, delete it — each step confirmed in the catalog the helm proxies
   * from the helm catalog.
   *
   * The API assertions are what make this more than a DOM test. A create that
   * only repainted locally, or a delete that only hid a row, would look
   * identical on screen; reading the helm catalog back proves the request
   * reached its authority.
   */
  test("profile CRUD round-trips from the app-bar popup to the helm", async ({
    page,
    request,
  }) => {
    const name = `panel-profile-${Date.now()}`;
    const renamed = `${name}-edited`;

    await listWithStubbedFeed(page);
    await openProfiles(page);

    await section(page).locator(".new-profile-button").click();
    const form = section(page).locator(".profile-form");
    await form.locator(".profile-name-input").fill(name);
    await form.locator(".profile-invocation-input").fill(FAKE_AGENT);
    await form.locator(".profile-save").click();

    // Registered from the API's answer the moment it exists, before any
    // assertion about the page — see `registerByName`.
    const stored = await registerByName(request, name);
    await expect(profileRow(page, stored.id)).toBeVisible({ timeout: 20_000 });

    // Editing replaces the definition; the row is the same row (same id) with
    // a new name, which is what a rename IS here.
    await profileRow(page, stored.id).locator(".profile-edit").click();
    await profileRow(page, stored.id).locator(".profile-name-input").fill(renamed);
    await profileRow(page, stored.id).locator(".profile-save").click();
    await expect(profileRow(page, stored.id).locator(".profile-name")).toHaveText(renamed, {
      timeout: 20_000,
    });
    expect(
      (await listProfiles(request)).profiles.find((p) => p.id === stored.id)?.name,
    ).toBe(renamed);

    // Deleting confirms first — wry ships no native JS dialogs on macOS's
    // WKWebView, so every confirmation in this UI is in-page — and the
    // consequence it opens with is the snapshot rule itself.
    await profileRow(page, stored.id).locator(".profile-delete").click();
    await expect(profileRow(page, stored.id).locator(".confirm-consequence")).toContainText(
      "leaves every session already created from it running",
    );
    await profileRow(page, stored.id).locator(".profile-confirm-delete").click();

    await expect(profileRow(page, stored.id)).toHaveCount(0, { timeout: 20_000 });
    expect(
      (await listProfiles(request)).profiles.some((p) => p.id === stored.id),
      "the delete must reach the catalog, not merely the DOM",
    ).toBe(false);
  });

  /**
   * Editing ONE field of a profile leaves every other field exactly as it was
   * — kind, invocation, and a resume template the editor cannot fully express.
   *
   * This is the test the name-only round trip above cannot be: a save REPLACES
   * the whole definition, so the editor is one omitted field away from
   * silently clearing something, and one naive re-split away from rewriting an
   * argv it merely displayed. The fixture is deliberately hostile on both
   * counts — a non-generic kind (which the picker must preselect rather than
   * default away from) and a resume argv with a space inside an element (which
   * the single-line field can only round-trip by sending back what it was
   * seeded with).
   */
  test("editing only the name preserves the rest of a profile's definition", async ({
    page,
    request,
  }) => {
    const name = `verbatim-${Date.now()}`;
    // `{conversation}` is required for an integrated kind, and `--note=a b` is
    // the element that cannot survive a re-split.
    const template = ["claude", "--resume", "{conversation}", "--note=a b"];
    const invocation = "claude --dangerously-skip-permissions";
    const profile = await createProfile(request, {
      name,
      invocation,
      agent_kind: "claude",
      resume_template: template,
    });
    profiles.push(profile.id);

    await listWithStubbedFeed(page);
    await openProfiles(page);
    const editing = profileRow(page, profile.id);
    await expect(editing).toBeVisible({ timeout: 20_000 });
    await editing.locator(".profile-edit").click();

    // The editor must OPEN on what is stored, not on a default: a kind select
    // showing `generic` here would save `generic`.
    await expect(editing.locator(".profile-kind-select")).toHaveValue("claude");
    await expect(editing.locator(".profile-invocation-input")).toHaveValue(invocation);
    // The field's exact spelling is the editor's business — it quotes what has
    // to be quoted so the value can be typed back (`profiles::resume_text`) —
    // so this asserts what the user can SEE rather than a rendering. The real
    // contract is the round trip, which the API read-back at the end of this
    // test makes: save with only the name touched, and the stored argv is
    // unchanged.
    const shownResume = await editing.locator(".profile-resume-input").inputValue();
    for (const argument of template) {
      expect(shownResume, `the field must show every argument, including ${argument}`)
        .toContain(argument);
    }

    await editing.locator(".profile-name-input").fill(`${name}-edited`);
    await editing.locator(".profile-save").click();
    await expect(editing.locator(".profile-name")).toHaveText(`${name}-edited`, {
      timeout: 20_000,
    });

    const stored = await (await request.get("/api/profiles")).json();
    const after = stored.profiles.find((p: { id: string }) => p.id === profile.id);
    expect(after.name).toBe(`${name}-edited`);
    expect(after.agent_kind, "the kind must not be rewritten by an edit that never touched it")
      .toBe("claude");
    expect(after.invocation).toBe(invocation);
    expect(
      after.resume_template,
      "an untouched resume field must send back the argv that was stored, not a re-split of how " +
        "it was displayed",
    ).toEqual(template);
  });

  /**
   * A save is visible to the NEXT editor immediately, even while the
   * authoritative catalog read is still in flight.
   *
   * The window is small and what happens inside it is durable: the operation
   * token is released when the save's own reply lands, so the row can be
   * reopened before the re-read commits. An editor seeded from the pre-edit
   * definition there would save it back and undo an update the helm had
   * already accepted — silently, since both saves succeed.
   */
  test("a saved profile is what the next editor sees, before the re-read lands", async ({
    page,
    request,
  }) => {
    const name = `delayed-read-${Date.now()}`;
    const profile = await createProfile(request, { name });
    profiles.push(profile.id);

    await listWithStubbedFeed(page);
    // One route for the whole test, with a FLAG rather than a route added
    // later: adding one leaves a gap (a read already in flight cannot be
    // un-issued), and a flag can be flipped in the same statement sequence as
    // the click it must precede.
    let held = false;
    await page.route(
      (url) => /^\/api\/profiles$/.test(url.pathname),
      async (route: Route) => {
        if (route.request().method() === "GET" && held) {
          await route.abort();
          return;
        }
        await route.continue();
      },
    );
    await openProfiles(page);
    const editing = profileRow(page, profile.id);
    await expect(editing).toBeVisible({ timeout: 20_000 });

    // From here on the authoritative read cannot answer, so everything below
    // is what the SAVE's own reply produced.
    held = true;
    await editing.locator(".profile-edit").click();
    // Opening places focus asynchronously on the name field. Let that
    // handoff finish before fill focuses the invocation field; otherwise
    // WebKit can deliver its text into the name after focus moves mid-fill.
    await expect(editing.locator(".profile-name-input")).toBeFocused();
    await editing.locator(".profile-invocation-input").fill("edited-invocation");
    await editing.locator(".profile-save").click();
    // The form closes on success; the row now has to show the accepted
    // definition from the reply alone.
    await expect(editing.locator(".profile-form")).toHaveCount(0, { timeout: 20_000 });
    await expect(editing.locator(".profile-invocation")).toContainText("edited-invocation");

    // And reopening seeds the editor from it rather than from what the last
    // successful read said.
    await editing.locator(".profile-edit").click();
    await expect(
      editing.locator(".profile-invocation-input"),
      "an editor seeded from the pre-edit definition would save it back and undo the update",
    ).toHaveValue("edited-invocation");
    held = false;
  });

  /**
   * The create dialog defaults to the profile a session was last created from
   * in the helm catalog — SPEC.md's creation rule, first half.
   *
   * The remembered default is the HELM's own state, written only by a
   * successful profile-backed create, so the fixture makes one through the
   * API: this test is about what the dialog does with that memory, not about
   * how it is recorded.
   *
   * The disabled command field is asserted alongside, because it is the
   * user-visible half of the wire's mutual exclusion: a create names either an
   * invocation or a profile and is refused for naming both, so a live field
   * beside a selected profile would invite typing a command that is not what
   * launches.
   */
  test("the create dialog preselects the profile last used in the helm", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const profile = await createProfile(request, { name: `remembered-${Date.now()}` });
    profiles.push(profile.id);
    const session = await createSession(request, {
      title: `remembered-session-${Date.now()}`,
      profile_id: profile.id,
      host: local,
    });
    created.push(session.id);

    await listWithStubbedFeed(page);
    await openCreateDialog(page);

    await expect(page.locator(".create-session-profile")).toHaveValue(profile.id, {
      timeout: 20_000,
    });
    await expect(page.locator(".create-session-profile-note")).toHaveCount(0);
    await expect(page.locator(".create-session-form input[type=\"text\"]").nth(1)).toBeDisabled();
    await expect(page.locator(".create-session-submit")).toBeEnabled();
  });

  /**
   * A failed refresh leaves the retained catalog usable but visibly stale in
   * the create form. Submission stays available because the helm resolves the
   * selected id against its authoritative catalog at submit time.
   */
  test("the create picker reports a retained catalog refresh failure", async ({
    page,
    request,
  }) => {
    const profile = await createProfile(request, { name: `stale-picker-${Date.now()}` });
    profiles.push(profile.id);
    const probe = await request.get("/api/profiles");
    const build = probe.headers()["x-farhelm-build"] ?? "";
    expect(build, "fabricated API replies must retain the helm build stamp").toBeTruthy();
    let fail = false;
    await page.route(
      (url) => url.pathname === "/api/profiles",
      async (route: Route) => {
        if (route.request().method() !== "GET" || !fail) {
          await route.continue();
          return;
        }
        await route.fulfill({
          status: 503,
          headers: { "content-type": "text/plain", "x-farhelm-build": build },
          body: "catalog refresh refused by fixture",
        });
      },
    );

    const feed = await listWithStubbedFeed(page);
    await openProfiles(page);
    await expect(profileRow(page, profile.id)).toBeVisible({ timeout: 20_000 });
    await closeProfiles(page);
    fail = true;
    feed.notify(2);
    await openCreateDialog(page);
    await waitForOption(page, profile.id);
    await expect(page.locator(".create-session-profile-refresh-error"))
      .toContainText("showing the last catalog this client read");
    await expect(page.locator(".create-session-profile-refresh-error"))
      .toContainText("catalog refresh refused by fixture");
    await page.locator(".create-session-profile").selectOption(profile.id);
    await expect(page.locator(".create-session-submit")).toBeEnabled();
  });

  /**
   * A remembered profile that no longer exists selects NOTHING and BLOCKS the
   * create until the user answers — SPEC.md's ask-don't-guess rule, and the
   * half of it that is easy to lose.
   *
   * Two substitutions are ruled out here, and the second is the one that hid
   * behind a friendly-looking fallback: quietly choosing another profile, and
   * quietly reverting to the command field — which is not empty, because a
   * user who typed a command before picking a profile still has it there.
   * Either would launch something nobody selected from a dialog whose own note
   * says nothing is.
   *
   * The catalog is deliberately NOT empty when this runs, so preselecting
   * nothing is a choice rather than the only option available.
   */
  test("a deleted last-used profile blocks the create until an agent is chosen", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const doomed = await createProfile(request, { name: `doomed-${Date.now()}` });
    profiles.push(doomed.id);
    const survivor = await createProfile(request, { name: `survivor-${Date.now()}` });
    profiles.push(survivor.id);

    const session = await createSession(request, {
      title: `doomed-session-${Date.now()}`,
      profile_id: doomed.id,
      host: local,
    });
    created.push(session.id);
    await cleanupProfile(request, doomed.id);
    expect(
      (await listProfiles(request)).default_profile,
      "the helm keeps the remembered id after the profile is gone; without that there is nothing " +
        "to ask about",
    ).toBe(doomed.id);

    await listWithStubbedFeed(page);
    await openCreateDialog(page);
    await waitForOption(page, survivor.id);

    await expect(page.locator(".create-session-profile")).toHaveValue(UNRESOLVED);
    await expect(page.locator(".create-session-profile-note")).toContainText("no longer exists");
    await expect(
      page.locator(".create-session-submit"),
      "a dialog that cannot say what it would launch must not be submittable",
    ).toBeDisabled();
    // The survivor is OFFERED rather than chosen: the dialog has profiles
    // available and still picks none.
    await expect(page.locator(`.create-session-profile option[value="${survivor.id}"]`))
      .toHaveCount(1);

    // Choosing the command path explicitly is one of the two ways out, and it
    // unblocks the dialog — which is what makes the block a question rather
    // than a dead end.
    await page.locator(".create-session-profile").selectOption("");
    await expect(page.locator(".create-session-submit")).toBeEnabled();
    await expect(page.locator(".create-session-form input[type=\"text\"]").nth(1)).toBeEnabled();
  });

  /**
   * An explicit choice survives an edit to the profile it names, and a DELETE
   * of it blocks rather than substituting — with a surviving profile right
   * there to be substituted with.
   *
   * The rename half proves the choice is held by id rather than by label: the
   * picker follows the profile through the change instead of losing it. The
   * delete half is where a "helpful" implementation goes wrong, and the
   * presence of the survivor is what makes the assertion mean something.
   *
   * The host-change step pins the shared-catalog rule: choosing another target
   * must not discard an explicit profile choice.
   */
  test("an explicit choice follows a rename, survives a host change, and blocks on delete", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const remote = (await listHosts(request)).find((host) => host.id !== local);
    expect(remote, "the e2e stack registers a second host; without it this test proves nothing")
      .toBeTruthy();
    const stamp = Date.now();
    const chosen = await createProfile(request, { name: `chosen-${stamp}` });
    profiles.push(chosen.id);
    const survivor = await createProfile(request, { name: `bystander-${stamp}` });
    profiles.push(survivor.id);

    const feed = await listWithStubbedFeed(page);
    await openCreateDialog(page);
    await waitForOption(page, chosen.id);
    await page.locator(".create-session-profile").selectOption(chosen.id);

    // A rename keeps the id, so the choice holds and only the label moves.
    await updateProfile(request, chosen.id, { name: `chosen-${stamp}-renamed` });
    feed.notify(2);
    await expect(page.locator(`.create-session-profile option[value="${chosen.id}"]`))
      .toHaveText(`chosen-${stamp}-renamed`, { timeout: 20_000 });
    await expect(page.locator(".create-session-profile")).toHaveValue(chosen.id);
    await expect(page.locator(".create-session-submit")).toBeEnabled();

    // Every target consumes this helm's catalog, so changing hosts keeps both
    // the option and the explicit choice.
    await page.locator(".create-session-host").selectOption(String(remote!.id));
    await expect(page.locator(`.create-session-profile option[value="${chosen.id}"]`))
      .toHaveCount(1);
    await expect(page.locator(".create-session-profile")).toHaveValue(chosen.id);

    // A delete takes the choice away, and nothing replaces it.
    await cleanupProfile(request, chosen.id);
    feed.notify(3);
    await expect(page.locator(".create-session-profile")).toHaveValue(UNRESOLVED, {
      timeout: 20_000,
    });
    await expect(page.locator(".create-session-profile-note")).toContainText("no longer in this helm");
    await expect(page.locator(".create-session-profile")).not.toHaveValue(survivor.id);
    await expect(page.locator(".create-session-submit")).toBeDisabled();
  });

  /**
   * Choosing a profile creates from it and the session records the snapshot —
   * the create dialog's half of SPEC.md's "creating a session offers the
   * helm's profiles".
   *
   * Asserted through the created session's own `source_profile` rather than
   * through the request body: the snapshot is what every later surface reads,
   * and a create that sent the profile but recorded nothing would leave the
   * session list unable to say what it was launched from.
   */
  test("choosing a profile creates from it and the session records the snapshot", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const profile = await createProfile(request, { name: `chosen-create-${Date.now()}` });
    profiles.push(profile.id);
    const title = `chosen-session-${Date.now()}`;

    await listWithStubbedFeed(page);
    await openCreateDialog(page);
    await waitForOption(page, profile.id);
    await page.locator(".create-session-profile").selectOption(profile.id);
    const form = page.locator(".create-session-form");
    // Directory and title only: the command field is inert while a profile is
    // selected, which is the point — the profile says what runs.
    await form.locator('input[type="text"]').nth(0).fill("/tmp");
    await form.locator('input[type="text"]').nth(2).fill(title);
    await form.locator('button[type="submit"]').click();

    // Registered as soon as the session exists, before the view is asserted
    // about: a create that succeeded and then failed to navigate must not
    // leave an agent running in the shared stack.
    let matching: { id: string }[] = [];
    await expect
      .poll(
        async () => {
          matching = (await listSessions(request, `title=${encodeURIComponent(title)}`)).sessions;
          // EVERY match is registered as it is seen, not just the first: if a
          // second one somehow exists, the assertion below fails and the
          // cleanup still reaps both rather than leaving an agent running in
          // the shared stack.
          for (const found of matching) {
            if (!created.includes(found.id)) created.push(found.id);
          }
          return matching.length;
        },
        { timeout: 30_000, message: "the create must produce a session" },
      )
      .toBeGreaterThan(0);
    expect(
      matching.length,
      "one press of create is one session — the idempotency key exists so a retry cannot make two",
    ).toBe(1);
    const session = matching[0];

    await expect(page.locator(".titlebar .title")).toHaveText(title, { timeout: 30_000 });

    const detail = await (await request.get(`/api/sessions/${session.id}`)).json();
    expect(detail.source_profile?.id, "the session must record the profile it came from").toBe(
      profile.id,
    );
    expect(detail.source_profile?.existence).toBe("present");
    expect(
      (await listProfiles(request)).default_profile,
      "and a successful profile-backed create is what makes a profile the remembered default",
    ).toBe(profile.id);
  });

  /**
   * A successful profile-backed create fences the retained remembered default
   * before the dialog can reopen. The new form waits for the explicit refresh
   * instead of consuming the previous default and latching the wrong choice.
   */
  test("reopening after a profile create waits for its fresh remembered default", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const stamp = Date.now();
    const previous = await createProfile(request, { name: `default-old-${stamp}` });
    profiles.push(previous.id);
    const chosen = await createProfile(request, { name: `default-new-${stamp}` });
    profiles.push(chosen.id);
    const anchor = await createSession(request, {
      title: `default-anchor-${stamp}`,
      host: local,
      profile_id: previous.id,
    });
    created.push(anchor.id);

    let hold = false;
    let release: (() => void) | undefined;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route(
      (url) => url.pathname === "/api/profiles",
      async (route: Route) => {
        if (route.request().method() === "GET" && hold) await held;
        await route.continue();
      },
    );

    await listWithStubbedFeed(page);
    await openCreateDialog(page);
    await waitForOption(page, chosen.id);
    const form = page.locator(".create-session-form");
    await page.locator(".create-session-profile").selectOption(chosen.id);
    await form.locator('input[type="text"]').nth(0).fill("/tmp");
    await form.locator('input[type="text"]').nth(2).fill(`default-created-${stamp}`);
    hold = true;
    const [response] = await Promise.all([
      page.waitForResponse(
        (reply) => reply.request().method() === "POST" && reply.url().endsWith("/api/sessions"),
      ),
      form.locator(".create-session-submit").click(),
    ]);
    const createdSession = await response.json();
    created.push(createdSession.id as string);

    await expect(page.locator(".create-session-form")).toHaveCount(0, { timeout: 20_000 });
    await openCreateDialog(page);
    await expect(page.locator(".create-session-profile")).toHaveValue(UNRESOLVED);
    await expect(page.locator(".create-session-submit")).toBeDisabled();

    release!();
    await expect(page.locator(".create-session-profile")).toHaveValue(chosen.id, {
      timeout: 20_000,
    });
  });

  /**
   * The idempotency key is bound to the creation MODE, not only to the text
   * fields: changing which agent a create would launch starts a new intent,
   * and the key that travels is paired with the mode that travels beside it.
   *
   * Only the request bodies can show this — neither the key nor the mode is on
   * screen — so the POSTs are recorded and compared. The create is made to
   * FAIL (a directory that does not exist), which is what keeps the form open
   * for the next submit and is also the case the key exists for: a retry of an
   * unchanged intent must reuse it.
   *
   * NOT covered here, deliberately: a selection changing DURING the key mint.
   * That window is a browser-side UUID call, too short to drive from
   * Playwright without pretending; what it must produce — one binding per
   * (fields, host, mode) triple — is pinned as a unit test over the binding
   * itself (`list::an_intent_binding_changes_with_the_host_incarnation_and_with_the_fields`).
   */
  test("the create's idempotency key is re-minted when its agent changes", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const stamp = Date.now();
    const first = await createProfile(request, { name: `key-a-${stamp}` });
    profiles.push(first.id);
    const second = await createProfile(request, { name: `key-b-${stamp}` });
    profiles.push(second.id);

    await listWithStubbedFeed(page);
    const bodies = await watchCreateBodies(page);
    await openCreateDialog(page);
    await waitForOption(page, first.id);

    const form = page.locator(".create-session-form");
    await form.locator('input[type="text"]').nth(0).fill("/nonexistent/definitely/not/here");
    await form.locator('input[type="text"]').nth(2).fill(`key-session-${stamp}`);
    await page.locator(".create-session-profile").selectOption(first.id);

    await form.locator('button[type="submit"]').click();
    await expect(form.locator(".create-session-error")).toBeVisible({ timeout: 20_000 });
    // An unchanged retry is the SAME intent — the whole reason the key
    // survives a failure.
    await form.locator('button[type="submit"]').click();
    await expect.poll(() => bodies.length, { timeout: 20_000 }).toBe(2);
    expect(bodies[0].profile_id).toBe(first.id);
    // The create names the connection it was prepared against, byte for byte
    // as the hosts read reported it — a profile-mode create is exactly the
    // request that would otherwise succeed on the WRONG install, because
    // starter ids collide across them.
    expect(bodies[0].expected_incarnation).toBe(
      (await listHosts(request)).find((host) => host.id === local)!.incarnation,
    );
    expect(bodies[0].invocation, "a profile-backed create must not also name an invocation")
      .toBeUndefined();
    expect(bodies[1].intent_key, "a retry of an unchanged intent reuses its key").toBe(
      bodies[0].intent_key,
    );

    // A different profile is a different intended create.
    await page.locator(".create-session-profile").selectOption(second.id);
    await form.locator('button[type="submit"]').click();
    await expect.poll(() => bodies.length, { timeout: 20_000 }).toBe(3);
    expect(bodies[2].profile_id).toBe(second.id);
    expect(bodies[2].intent_key).not.toBe(bodies[0].intent_key);

    // So is switching modes entirely.
    await page.locator(".create-session-profile").selectOption("");
    await form.locator('input[type="text"]').nth(1).fill(FAKE_AGENT);
    await form.locator('button[type="submit"]').click();
    await expect.poll(() => bodies.length, { timeout: 20_000 }).toBe(4);
    expect(bodies[3].profile_id, "a command create must not name a profile").toBeUndefined();
    expect(bodies[3].invocation).toBe(FAKE_AGENT);
    expect(bodies[3].intent_key).not.toBe(bodies[2].intent_key);
  });

  /**
   * A definition created in the management popup belongs to the helm, so the
   * create picker must retain it when the target moves between two hosts.
   */
  test("a popup-created profile is offered on every host", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const remote = (await listHosts(request)).find((host) => host.id !== local);
    expect(remote, "the e2e stack registers a second host; without it this test proves nothing")
      .toBeTruthy();
    const name = `shared-${Date.now()}`;

    await listWithStubbedFeed(page);
    await openProfiles(page);
    await section(page).locator(".new-profile-button").click();
    const form = section(page).locator(".profile-form");
    await form.locator(".profile-name-input").fill(name);
    await form.locator(".profile-invocation-input").fill(FAKE_AGENT);
    await form.locator(".profile-save").click();
    const profile = await registerByName(request, name);
    await closeProfiles(page);

    await openCreateDialog(page);
    await waitForOption(page, profile.id);
    await page.locator(".create-session-profile").selectOption(profile.id);
    await page.locator(".create-session-host").selectOption(String(remote!.id));
    await expect(page.locator(`.create-session-profile option[value="${profile.id}"]`))
      .toHaveCount(1);
    await expect(page.locator(".create-session-profile")).toHaveValue(profile.id);
  });

  /** Profiles are managed from the app bar, so no host row may advertise a
   * second profile surface in its actions menu. */
  test("the host row menu has no profiles item", async ({ page, request }) => {
    const local = await localHostId(request);
    await listWithStubbedFeed(page);
    await openHostsPanel(page);
    const host = page.locator(`[data-host-id="${local}"]`);
    await openHostMenu(host);
    await expect(host.locator(".host-row-menu-panel")).not.toContainText("profiles");
    await expect(host.locator(".host-profiles-toggle")).toHaveCount(0);
  });

  /**
   * Editing a profile does not touch the sessions already created from it —
   * SPEC.md's snapshot rule, as the list shows it.
   *
   * The rename happens with the page ALREADY OPEN and is then settled and
   * announced, rather than being staged before the page loads. Both halves are
   * deliberate. Renaming under an open page is the case the rule is about, and
   * the settle-then-notify is the stubbed-feed convention: a session's
   * `existence` is derived per reply by the helm and reaches the merged list
   * only when its session cache next refreshes, so a
   * notification played before that lands would have the page re-read a view
   * that has not moved.
   */
  test("an edited profile leaves existing sessions naming what they snapshotted", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const before = `snapshot-before-${Date.now()}`;
    const after = `${before}-renamed`;
    const title = `snapshot-session-${Date.now()}`;
    const profile = await createProfile(request, { name: before });
    profiles.push(profile.id);
    const session = await createSession(request, {
      title,
      profile_id: profile.id,
      host: local,
    });
    created.push(session.id);

    const feed = await listWithStubbedFeed(page);
    // The profile chip moved into the row's actions panel with the rest
    // of the per-session controls; it only exists in the DOM while the
    // panel is open.
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });
    await openRowMenu(row(page, session.id));
    const label = row(page, session.id).locator(".session-profile");
    await expect(label).toBeVisible({ timeout: 20_000 });
    await expect(label).toContainText(before);
    await expect(label).toHaveAttribute("data-profile-existence", "present", { timeout: 20_000 });

    // The row's OWN meta-line badge (`.session-invocation`, always in the
    // DOM regardless of the panel) carries the same snapshotted name — it
    // is a second surface the panel chip's assertions above never touch,
    // and the row.rs source this checks against is a different render
    // branch than the panel chip's.
    const badge = row(page, session.id).locator(".session-invocation");
    await expect(badge).toHaveText(before);
    await expect(badge).toHaveAttribute("title", `profile: ${before} — ${FAKE_AGENT}`);

    await updateProfile(request, profile.id, { name: after });
    await settleExistence(request, title, "renamed");
    feed.notify(2);

    await expect(label).toHaveAttribute("data-profile-existence", "renamed", { timeout: 20_000 });
    await expect(
      label,
      "the row must keep the name it snapshotted; adopting the profile's new one would rewrite " +
        "what the session was created from",
    ).toContainText(before);
    await expect(label).not.toContainText(after);

    // The meta-line badge keeps the same snapshot, with its `title`
    // qualified the same way `source_profile_label` qualifies the panel
    // chip's tooltip.
    await expect(badge).toHaveText(before);
    await expect(badge).not.toHaveText(after);
    await expect(badge).toHaveAttribute(
      "title",
      `profile: ${before} (renamed since) — ${FAKE_AGENT}`,
    );

    // The catalog, meanwhile, says the new name — the two surfaces disagree
    // on purpose.
    await openProfiles(page);
    await expect(profileRow(page, profile.id).locator(".profile-name")).toHaveText(after, {
      timeout: 20_000,
    });
  });

  /**
   * A near-limit unbroken profile name stays constrained inside the
   * actions panel instead of widening it out of the sidebar.
   *
   * The chip moved from the row line into the 300px-max panel, whose
   * column layout is a new overflow context for it; every other profile
   * test uses short names, so this is the only place the ellipsis rule
   * is actually exercised where the chip now lives.
   */
  test("a long profile name ellipsizes inside the actions panel", async ({ page, request }) => {
    const local = await localHostId(request);
    const name = `long-${"p".repeat(180)}`;
    const title = `long-profile-session-${Date.now()}`;
    const profile = await createProfile(request, { name });
    profiles.push(profile.id);
    const session = await createSession(request, { title, profile_id: profile.id, host: local });
    created.push(session.id);

    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await openRowMenu(target);

    const chip = target.locator(".session-profile");
    await expect(chip).toBeVisible();
    const sidebarBox = (await page.locator(".app-sidebar").boundingBox())!;
    const panelBox = (await target.locator(".session-row-menu-panel").boundingBox())!;
    const chipBox = (await chip.boundingBox())!;
    // The panel keeps to the sidebar, the chip keeps to the panel...
    expect(panelBox.x + panelBox.width).toBeLessThanOrEqual(sidebarBox.x + sidebarBox.width + 1);
    expect(chipBox.x + chipBox.width).toBeLessThanOrEqual(panelBox.x + panelBox.width + 1);
    // ...and the name really is being clipped, proving the ellipsis rule
    // did the constraining rather than a conveniently short fixture.
    expect(await chip.evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
  });

  /**
   * A deleted profile's sessions keep the snapshotted name as a plain label.
   *
   * The row must outlive the catalog definition, but absence from this helm is
   * not a warning about an old session or one imported from another helm. The
   * immutable name therefore renders exactly as it did while the row existed.
   */
  test("a deleted profile's sessions keep their plain snapshot label", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const name = `deleted-snapshot-${Date.now()}`;
    const title = `deleted-snapshot-session-${Date.now()}`;
    const profile = await createProfile(request, { name });
    profiles.push(profile.id);
    const session = await createSession(request, { title, profile_id: profile.id, host: local });
    created.push(session.id);

    const feed = await listWithStubbedFeed(page);
    // As above: the chip lives in the actions panel now.
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });
    await openRowMenu(row(page, session.id));
    const label = row(page, session.id).locator(".session-profile");
    await expect(label).toHaveAttribute("data-profile-existence", "present", { timeout: 20_000 });
    const presentLabelColor = await label.evaluate((element) => getComputedStyle(element).color);

    // The row's own meta-line badge, same present-state snapshot as the
    // panel chip — see the rename test above for why this is a second
    // surface worth its own assertion.
    const badge = row(page, session.id).locator(".session-invocation");
    await expect(badge).toHaveText(name);
    await expect(badge).toHaveAttribute("title", `profile: ${name} — ${FAKE_AGENT}`);
    const presentBadgeColor = await badge.evaluate((element) => getComputedStyle(element).color);

    await cleanupProfile(request, profile.id);
    await settleExistence(request, title, "deleted");
    feed.notify(2);

    await expect(label).toHaveAttribute("data-profile-existence", "deleted", { timeout: 20_000 });
    await expect(label).toContainText(name);
    await expect(
      row(page, session.id),
      "a session outlives the profile it was created from; removing the row would destroy the " +
        "record of what it launched",
    ).toBeVisible();

    // The badge keeps the same snapshot as a plain historical label.
    await expect(badge).toHaveText(name);
    await expect(badge).toHaveAttribute(
      "title",
      `profile: ${name} — ${FAKE_AGENT}`,
    );
    expect(await label.evaluate((element) => getComputedStyle(element).color))
      .toBe(presentLabelColor);
    expect(await badge.evaluate((element) => getComputedStyle(element).color))
      .toBe(presentBadgeColor);
  });

  /**
   * A profile deleted elsewhere closes whatever this client had open on it —
   * an editor or a delete confirmation — and says so.
   *
   * Otherwise the row and its form vanish with the refresh while the draft,
   * the confirmation and their state go on existing invisibly: the next save
   * or confirm would then act on a profile that is gone, and the user would
   * have no idea why it failed. Both states are staged, because they are
   * tracked separately and only one of them would have been noticed.
   */
  test("a profile deleted elsewhere closes an open editor and confirmation", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const stamp = Date.now();
    const edited = await createProfile(request, { name: `vanish-edit-${stamp}` });
    profiles.push(edited.id);
    const confirmed = await createProfile(request, { name: `vanish-delete-${stamp}` });
    profiles.push(confirmed.id);

    const feed = await listWithStubbedFeed(page);
    await openProfiles(page);

    // An open EDITOR on a profile that then goes away.
    await profileRow(page, edited.id).locator(".profile-edit").click();
    await expect(profileRow(page, edited.id).locator(".profile-form")).toBeVisible();
    await cleanupProfile(request, edited.id);
    feed.notify(2);
    await expect(profileRow(page, edited.id)).toHaveCount(0, { timeout: 20_000 });
    const notice = section(page).locator(".profiles-notice");
    await expect(notice).toContainText("no longer in this helm");

    // And an open CONFIRMATION, tracked separately.
    await profileRow(page, confirmed.id).locator(".profile-delete").click();
    await expect(profileRow(page, confirmed.id).locator(".profile-confirm-delete"))
      .toBeVisible();
    await cleanupProfile(request, confirmed.id);
    feed.notify(3);
    await expect(profileRow(page, confirmed.id)).toHaveCount(0, { timeout: 20_000 });
    await expect(notice).toContainText("already gone");
    // Nothing is left that could act on either: the section is back to its
    // ordinary state, with no form and no prompt anywhere in it.
    await expect(section(page).locator(".profile-form")).toHaveCount(0);
    await expect(section(page).locator(".profile-confirm-delete")).toHaveCount(0);
  });

  /**
   * A refused save keeps every draft field and changes nothing server-side.
   *
   * The refusal is a REAL one from the helm — a definition past the
   * per-profile size cap — rather than a routed reply, because the sentence
   * the user acts on is the helm's and a fabricated one would prove only
   * that this UI can render a string it was handed. What must survive is the
   * whole draft: a refused name is usually one keystroke from an accepted one,
   * and a form that cleared itself would make the user retype a definition
   * that was nearly right.
   */
  test("a refused profile save keeps the draft and leaves the catalog alone", async ({
    page,
    request,
  }) => {
    const before = (await listProfiles(request)).profiles.length;
    const oversized = "x".repeat(9_000);

    await listWithStubbedFeed(page);
    await openProfiles(page);
    await section(page).locator(".new-profile-button").click();
    const form = section(page).locator(".profile-form");
    await form.locator(".profile-name-input").fill(oversized);
    await form.locator(".profile-invocation-input").fill(FAKE_AGENT);
    await form.locator(".profile-kind-select").selectOption("codex");
    await form.locator(".profile-save").click();

    await expect(form.locator(".profile-form-error")).toBeVisible({ timeout: 20_000 });
    await expect(form.locator(".profile-name-input")).toBeFocused();
    await page.waitForTimeout(400);
    await expect(section(page)).toBeVisible();
    // Preserved, not cleared or reset — including the fields the refusal was
    // not about.
    await expect(form.locator(".profile-invocation-input")).toHaveValue(FAKE_AGENT);
    await expect(form.locator(".profile-kind-select")).toHaveValue("codex");
    expect(await form.locator(".profile-name-input").inputValue()).toHaveLength(oversized.length);
    expect(
      (await listProfiles(request)).profiles.length,
      "a refused create must leave the catalog exactly as it was",
    ).toBe(before);
  });

  /**
   * A success this build cannot READ is still a success: the form closes, the
   * page says its reply was unreadable, and the catalog re-read is what
   * settles what happened.
   *
   * The distinction is the whole point of the warning line. Treating an
   * undecodable 2xx as a refusal would tell the user their profile was
   * rejected when it demonstrably exists — and they would create it again.
   */
  test("a profile create whose reply cannot be read warns rather than refusing", async ({
    page,
    request,
  }) => {
    const name = `unreadable-${Date.now()}`;
    // ONE route for this path, handling both verbs. Two routes over the same
    // pattern would not compose: Playwright runs the LAST matching handler
    // and the earlier one never sees the request, so a second route added for
    // the reads would silently take the writes with it — and this test's
    // malformed reply would quietly become the real, readable one.
    //
    // The write is answered with a body this build cannot decode while the
    // real request still happens (the profile IS created). The read is HELD
    // while `held` is set, so what the section shows in between is the
    // client's own decision rather than a re-read papering over it.
    let held = true;
    await page.route(
      (url) => /^\/api\/profiles$/.test(url.pathname),
      async (route: Route) => {
        if (route.request().method() === "POST") {
          const response = await route.fetch();
          await route.fulfill({
            response,
            body: "{ this is not the profile you are looking for",
            headers: { ...response.headers(), "content-type": "application/json" },
          });
          return;
        }
        if (held) {
          await route.abort();
          return;
        }
        await route.continue();
      },
    );

    await listWithStubbedFeed(page);
    held = false;
    await openProfiles(page);
    const before = section(page).locator(".profile-row");
    await expect(before.first()).toBeVisible({ timeout: 20_000 });
    await section(page).locator(".new-profile-button").click();
    const form = section(page).locator(".profile-form");
    await form.locator(".profile-name-input").fill(name);
    await form.locator(".profile-invocation-input").fill(FAKE_AGENT);
    held = true;
    await form.locator(".profile-save").click();

    const stored = await registerByName(request, name);
    await expect(section(page).locator(".profiles-warning")).toBeVisible({
      timeout: 20_000,
    });
    // The held catalog is DROPPED, not left reopenable: this build cannot say
    // what the accepted change produced, so every row it still held is
    // suspect — and an editor seeded from one would save a definition known to
    // be superseded.
    await expect(
      section(page).locator(".profile-row"),
      "a success this build could not read must not leave stale rows editable",
    ).toHaveCount(0);
    // Deliberately NOT asserting which of the two empty states is showing.
    // The held read fails rather than hanging, so the section may be pending
    // or reporting that failure depending on whether the request had been
    // issued yet — and both are honest. What must hold is that nothing is
    // there to reopen and edit.
    await expect(section(page).locator(".profile-edit")).toHaveCount(0);

    // Only the authoritative read fills it back in.
    held = false;
    // An accepted change closes its form — the same as a readable success,
    // because the change is what closed it.
    await expect(form).toHaveCount(0, { timeout: 20_000 });
    // And the authoritative read is what puts the rows back, which is exactly
    // what the warning says it will.
    await expect(profileRow(page, stored.id)).toBeVisible({ timeout: 30_000 });
  });

  /**
   * A refused DELETE keeps the row, reports the refusal on that row, and
   * leaves its controls usable.
   *
   * The refusal a user meets here is the host-state one ("… is
   * unreachable-reprobing, so this operation is refused"), which cannot be
   * staged against a healthy stack for a host the test also needs alive — so
   * this one IS routed, and what it pins is the client's handling: the row
   * must not be removed optimistically, the message must land on the row it is
   * about rather than in a shared slot, and the operation token must be
   * released so the next attempt is possible.
   */
  test("a refused profile delete keeps the row and reports on it", async ({ page, request }) => {
    const profile = await createProfile(request, { name: `undeletable-${Date.now()}` });
    profiles.push(profile.id);
    await page.route(
      (url) => /^\/api\/profiles\/[^/]+$/.test(url.pathname),
      async (route: Route) => {
        if (route.request().method() !== "DELETE") {
          await route.continue();
          return;
        }
        await fulfillRefusal(
          route,
          request,
          "host is unreachable-reprobing, so this operation is refused and nothing was queued",
        );
      },
    );

    await listWithStubbedFeed(page);
    await openProfiles(page);
    const target = profileRow(page, profile.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await target.locator(".profile-delete").click();
    await target.locator(".profile-confirm-delete").click();

    await expect(target.locator(".profile-error")).toContainText("refused", { timeout: 20_000 });
    await expect(target.locator(".profile-edit")).toBeFocused();
    await page.waitForTimeout(400);
    await expect(section(page)).toBeVisible();
    await expect(target, "a refused delete must not remove the row").toBeVisible();
    await expect(
      target.locator(".profile-delete"),
      "the operation token must be released, or the popup is inert with nothing explaining why",
    ).toBeEnabled();
    expect(
      (await listProfiles(request)).profiles.some((p) => p.id === profile.id),
      "and the catalog still holds it",
    ).toBe(true);
  });

  /**
   * A profile edited elsewhere reaches an open profiles popup on the next
   * notification, and NOT before — the popup's half of the
   * invalidation contract.
   *
   * The stub is what makes both halves provable: the socket is silent until
   * this test sends a revision, so the repaint cannot be a poll that happened
   * to land, and the assertion made BEFORE the notification is what shows the
   * surface is genuinely notification-driven rather than merely fast.
   *
   * No settle step here, deliberately, unlike the session-row tests: the
   * awaited helm edit has already committed by the time it answers, so polling
   * for it would assert nothing.
   */
  test("a profile edited elsewhere reaches an open profiles popup on notification", async ({
    page,
    request,
  }) => {
    const before = `feed-profile-${Date.now()}`;
    const after = `${before}-elsewhere`;
    const profile = await createProfile(request, { name: before });
    profiles.push(profile.id);

    const feed = await listWithStubbedFeed(page);
    await openProfiles(page);
    const name = profileRow(page, profile.id).locator(".profile-name");
    await expect(name).toHaveText(before, { timeout: 20_000 });

    await updateProfile(request, profile.id, { name: after });
    // Still showing the old name: nothing polls this surface while the feed is
    // healthy, which is what makes the assertion after the notice meaningful.
    await expect(name).toHaveText(before);

    feed.notify(2);
    await expect(name).toHaveText(after, { timeout: 20_000 });
  });

  /**
   * A dialog that is ASKING does not stop asking because another client's
   * create supplied a new remembered default.
   *
   * Staged with the dialog in the unresolved state deliberately: an explicit
   * choice would pass this test before the fix as well as after, because a
   * choice is what the old consumption check looked for. What it could not
   * represent is a dialog whose first catalog answered "the profile you last
   * used is gone" — that leaves NO choice behind, so every later refresh
   * re-consulted the default, and another client creating a session would
   * quietly select a profile under someone reading the question. The latch is
   * what makes the first answer the answer.
   */
  test("a dialog that is asking keeps asking when the remembered default moves", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const stamp = Date.now();
    // The dialog's first catalog must have a remembered default that is GONE.
    const doomed = await createProfile(request, { name: `ask-doomed-${stamp}` });
    profiles.push(doomed.id);
    const survivor = await createProfile(request, { name: `ask-survivor-${stamp}` });
    profiles.push(survivor.id);
    const first = await createSession(request, {
      title: `ask-first-${stamp}`,
      profile_id: doomed.id,
      host: local,
    });
    created.push(first.id);
    await cleanupProfile(request, doomed.id);

    const feed = await listWithStubbedFeed(page);
    await openCreateDialog(page);
    await waitForOption(page, survivor.id);
    await expect(page.locator(".create-session-profile")).toHaveValue(UNRESOLVED);
    await expect(page.locator(".create-session-submit")).toBeDisabled();

    // Another client creates from the SURVIVOR, which makes it this host's
    // remembered default — a profile that resolves, so a dialog still
    // consulting the default would select it.
    const second = await createSession(request, {
      title: `ask-second-${stamp}`,
      profile_id: survivor.id,
      host: local,
    });
    created.push(second.id);
    await expect
      .poll(async () => (await listProfiles(request)).default_profile, { timeout: 20_000 })
      .toBe(survivor.id);
    feed.notify(2);

    // The page re-reads the catalog on that notice (the option list is proof
    // it did) — and the question is still the question.
    await expect(page.locator(`.create-session-profile option[value="${survivor.id}"]`))
      .toHaveCount(1, { timeout: 20_000 });
    await expect(
      page.locator(".create-session-profile"),
      "the first catalog decided, once; a later default is not this dialog's answer",
    ).toHaveValue(UNRESOLVED);
    await expect(page.locator(".create-session-submit")).toBeDisabled();
    await expect(page.locator(".create-session-profile-note")).toBeVisible();
  });

  /**
   * A selection changed and submitted in the SAME turn sends the new
   * selection, not the one the last render computed.
   *
   * One JavaScript turn is the whole window, and it is entirely reachable: a
   * keyboard user tabbing off the picker onto the create button produces
   * exactly this ordering. A handler that used a value captured at render time
   * would send the PREVIOUS profile under a freshly minted key — a key that
   * faithfully describes an intent nobody had.
   */
  test("a selection changed in the same turn as the submit is what gets sent", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const stamp = Date.now();
    const first = await createProfile(request, { name: `turn-a-${stamp}` });
    profiles.push(first.id);
    const second = await createProfile(request, { name: `turn-b-${stamp}` });
    profiles.push(second.id);

    await listWithStubbedFeed(page);
    const bodies = await watchCreateBodies(page);
    await openCreateDialog(page);
    await waitForOption(page, second.id);
    const form = page.locator(".create-session-form");
    await form.locator('input[type="text"]').nth(0).fill("/nonexistent/definitely/not/here");
    await form.locator('input[type="text"]').nth(2).fill(`turn-session-${stamp}`);
    await page.locator(".create-session-profile").selectOption(first.id);

    // Both events dispatched from ONE evaluation, with no chance for a render
    // to land in between: the select changes and the form submits in the same
    // turn.
    await page.evaluate((id) => {
      const picker = document.querySelector<HTMLSelectElement>(".create-session-profile")!;
      picker.value = id;
      picker.dispatchEvent(new Event("change", { bubbles: true }));
      document.querySelector<HTMLFormElement>(".create-session-form")!.requestSubmit();
    }, second.id);

    await expect.poll(() => bodies.length, { timeout: 20_000 }).toBeGreaterThan(0);
    expect(
      bodies[0].profile_id,
      "the create must carry the selection as it stood when the button was pressed",
    ).toBe(second.id);
  });

  /**
   * An explicit command chosen while the catalog is pending remains
   * authoritative after a late answer supplies a live remembered profile.
   * The picker, visible invocation, and submitted wire mode must agree; any
   * one of them moving would let a late default replace the user's decision.
   */
  test("a command chosen before a late catalog default remains authoritative", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const profile = await createProfile(request, { name: `pending-${Date.now()}` });
    profiles.push(profile.id);
    const anchor = await createSession(request, {
      title: `pending-default-${Date.now()}`,
      host: local,
      profile_id: profile.id,
    });
    created.push(anchor.id);
    let release: (() => void) | undefined;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route(
      (url) => /^\/api\/profiles$/.test(url.pathname),
      async (route: Route) => {
        if (route.request().method() === "GET") await held;
        await route.continue();
      },
    );

    await listWithStubbedFeed(page);
    const bodies = await watchCreateBodies(page);
    await openCreateDialog(page);
    await expect(page.locator(".create-session-profile")).toHaveValue(UNRESOLVED, {
      timeout: 20_000,
    });
    await expect(
      page.locator(".create-session-submit"),
      "a dialog that cannot say what it would launch must not be submittable",
    ).toBeDisabled();

    const form = page.locator(".create-session-form");
    const command = `${FAKE_AGENT} --late-default-test`;
    await form.locator('input[type="text"]').nth(1).fill(command);
    await expect(page.locator(".create-session-submit")).toBeEnabled();

    release!();
    await waitForOption(page, profile.id);
    await expect(page.locator(".create-session-profile")).toHaveValue("");
    await expect(form.locator('input[type="text"]').nth(1)).toHaveValue(command);

    await form.locator('input[type="text"]').nth(0).fill("/nonexistent/late-default-test");
    await form.locator('input[type="text"]').nth(2).fill(`late-default-${Date.now()}`);
    await form.locator(".create-session-submit").click();
    await expect.poll(() => bodies.length, { timeout: 20_000 }).toBe(1);
    expect(bodies[0].profile_id, "command mode must not send the late default id").toBeUndefined();
    expect(bodies[0].invocation).toBe(command);
  });

  /**
   * The editor shows peer text ESCAPED, and saving an untouched field writes
   * back the original bytes.
   *
   * API clients can store control characters in profile names and invocations,
   * and an `<input>` is the one place this UI cannot isolate what it renders:
   * a right-to-left override stays active there, so what a person reads while
   * editing can differ from what they save — on a value that is about to be
   * executed. The escaped form is what makes the field say what is stored; the
   * API read-back is what proves nothing was mangled by saying so.
   */
  test("an editor shows escaped peer text and saves the original bytes", async ({
    page,
    request,
  }) => {
    // A right-to-left override inside the name, and a zero-width space inside
    // the invocation.
    const name = `rlo-‮txt.exe-${Date.now()}`;
    const invocation = `${FAKE_AGENT}​`;
    const profile = await createProfile(request, { name, invocation });
    profiles.push(profile.id);

    await listWithStubbedFeed(page);
    await openProfiles(page);
    const editing = profileRow(page, profile.id);
    await expect(editing).toBeVisible({ timeout: 20_000 });
    await editing.locator(".profile-edit").click();

    await expect(
      editing.locator(".profile-name-input"),
      "the field must show the escaped form: an active override inside an input can make the " +
        "text read as something other than what is stored",
    ).toHaveValue(name.replace("‮", "<U+202E>"));
    await expect(editing.locator(".profile-invocation-input")).toHaveValue(
      invocation.replace("​", "<U+200B>"),
    );

    // Save with only the KIND touched: every text field is untouched, so every
    // one of them must round-trip byte for byte.
    await editing.locator(".profile-kind-select").selectOption("codex");
    await editing.locator(".profile-save").click();
    await expect(editing.locator(".profile-form")).toHaveCount(0, { timeout: 20_000 });

    const stored = (await listProfiles(request)).profiles.find((p) => p.id === profile.id);
    expect(stored?.name, "an untouched field saves what it was seeded with").toBe(name);
    expect(stored?.invocation).toBe(invocation);
    expect(stored?.agent_kind).toBe("codex");
  });

  /**
   * A host menu refused by the operation token leaves the helm-wide popup
   * exactly as it was. A mutation keeps its surface mounted until the reply
   * settles, so another popover cannot discard its outcome mid-request.
   */
  /**
   * A profile mutation keeps its form mounted through every transient-surface
   * layout dismissal attempt. Once the reply releases the operation lock, the
   * queued invalidation closes the stale popup and returns focus to its owner.
   */
  test("busy profile work defers dismissal and layout invalidation until completion", async ({
    page,
    request,
  }) => {
    const profile = await createProfile(request, { name: `busy-focus-${Date.now()}` });
    profiles.push(profile.id);
    let release: (() => void) | undefined;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route(
      (url) => new RegExp(`/api/profiles/${profile.id}$`).test(url.pathname),
      async (route: Route) => {
        if (route.request().method() !== "POST") {
          await route.continue();
          return;
        }
        await held;
        await route.continue();
      },
    );

    await listWithStubbedFeed(page);
    await openProfiles(page);
    const editing = profileRow(page, profile.id);
    await editing.locator(".profile-edit").click();
    await editing.locator(".profile-name-input").fill(`${profile.name}-saved`);
    await editing.locator(".profile-save").click();
    await expect(editing.locator(".profile-save")).toBeDisabled();

    await page.keyboard.press("Escape");
    await expect(section(page), "Escape cannot unmount the in-flight reply destination")
      .toBeVisible();
    // The open popup physically covers this control, so drive the busy-surface
    // contract directly rather than pretending a pointer can reach it.
    await page.locator(".filter-toggle").dispatchEvent("click");
    await expect(page.locator(".filter-popover"), "a competing filter is refused while busy")
      .toHaveCount(0);
    await expect(section(page)).toBeVisible();
    await page.setViewportSize({ width: 900, height: 650 });
    await expect(section(page), "resize dismissal waits for the mutation reply").toBeVisible();

    release!();
    await expect(section(page)).toHaveCount(0, { timeout: 20_000 });
    await expect(page.locator(".profiles-toggle")).toBeFocused();
  });

  /**
   * Two stale placement samples use the same busy-aware layout obligation as
   * a later scroll. The second stale result cannot unmount an in-flight form;
   * dismissal waits for the mutation reply and then restores the toggle.
   */
  test("twice-stale placement defers its busy layout dismissal", async ({
    page,
    request,
  }) => {
    const profile = await createProfile(request, { name: `busy-measure-${Date.now()}` });
    profiles.push(profile.id);
    let releaseSave: (() => void) | undefined;
    const heldSave = new Promise<void>((resolve) => {
      releaseSave = resolve;
    });
    await page.route(
      (url) => new RegExp(`/api/profiles/${profile.id}$`).test(url.pathname),
      async (route: Route) => {
        if (route.request().method() !== "POST") {
          await route.continue();
          return;
        }
        await heldSave;
        await route.continue();
      },
    );
    await listWithStubbedFeed(page);
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles = {
        measurement: { holds: 2, started: 0 },
      };
    });
    await page.locator(".profiles-toggle").click();
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.measurement.started)
    ).toBe(1);
    const row = profileRow(page, profile.id);
    await row.locator(".profile-edit").dispatchEvent("click");
    await row.locator(".profile-name-input").fill(`${profile.name}-saved`);
    await row.locator(".profile-save").dispatchEvent("click");
    await expect(row.locator(".profile-save")).toBeDisabled();

    await page.evaluate(() => {
      document.querySelector(".app-sidebar")?.dispatchEvent(new Event("scroll"));
      (window as any).__farhelmTestProfiles.measurement.release();
    });
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.measurement.started)
    ).toBe(2);
    await page.evaluate(() => {
      document.querySelector(".app-sidebar")?.dispatchEvent(new Event("scroll"));
      (window as any).__farhelmTestProfiles.measurement.release();
    });
    await expect(section(page)).toBeAttached();
    releaseSave!();
    await expect(section(page)).toHaveCount(0, { timeout: 20_000 });
    await expect(page.locator(".profiles-toggle")).toBeFocused();
  });

  /**
   * A real inert click during busy work has causal precedence over completion
   * focus. The popup stays mounted for the reply, then dismisses without a
   * late row-focus request swallowing the outside interaction.
   */
  test("busy profile work preserves an inert focus-out obligation", async ({
    page,
    request,
  }) => {
    const profile = await createProfile(request, { name: `busy-outside-${Date.now()}` });
    profiles.push(profile.id);
    let release: (() => void) | undefined;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route(
      (url) => new RegExp(`/api/profiles/${profile.id}$`).test(url.pathname),
      async (route: Route) => {
        if (route.request().method() !== "POST") {
          await route.continue();
          return;
        }
        await held;
        await route.continue();
      },
    );

    await listWithStubbedFeed(page);
    await openProfiles(page);
    const row = profileRow(page, profile.id);
    await row.locator(".profile-edit").click();
    await row.locator(".profile-name-input").fill(`${profile.name}-saved`);
    await row.locator(".profile-save").click();
    await expect(row.locator(".profile-save")).toBeDisabled();

    const { x, y } = await inertSidebarPoint(page);
    await page.mouse.click(x, y);
    await expect.poll(() => page.evaluate(() => document.activeElement === document.body))
      .toBe(true);
    await expect(section(page)).toBeVisible();
    release!();
    await expect(section(page)).toHaveCount(0, { timeout: 20_000 });
    await expect.poll(() => page.evaluate(() => document.activeElement === document.body))
      .toBe(true);
  });

  /**
   * Scripted pointer events carry no user provenance. Fabricating one over an
   * inert outside target cannot turn a busy control's body transit into a
   * dismissal obligation or suppress mutation-completion focus.
   */
  test("a synthetic outside pointerdown is not dismissal intent", async ({ page, request }) => {
    const profile = await createProfile(request, { name: `synthetic-outside-${Date.now()}` });
    profiles.push(profile.id);
    await listWithStubbedFeed(page);
    await openProfiles(page);
    const save = await beginHeldProfileSave(page, profile);
    const point = await inertSidebarPoint(page);
    await page.evaluate(({ x, y }) => {
      document.elementFromPoint(x, y)?.dispatchEvent(
        new PointerEvent("pointerdown", { bubbles: true, clientX: x, clientY: y }),
      );
    }, point);

    save.release();
    await expect(save.row.locator(".profile-edit")).toBeFocused({ timeout: 20_000 });
    await expect(section(page)).toBeVisible();
  });

  /**
   * Tab supplies trusted keyboard provenance before its outside focusin. That
   * destination wins during a held mutation, so completion never focuses the
   * row and idle transition dismisses without restoring the toggle.
   */
  test("Tab to an outside control preserves busy dismissal intent", async ({ page, request }) => {
    const profile = await createProfile(request, { name: `tab-outside-${Date.now()}` });
    profiles.push(profile.id);
    await listWithStubbedFeed(page);
    await openProfiles(page);
    const save = await beginHeldProfileSave(page, profile);
    await save.row.locator(".profile-save").evaluate((element: HTMLButtonElement) => {
      // The operation lock disables the focused Save control on its next
      // render. Re-enable only this DOM test origin so the trusted Tab starts
      // inside the still-busy popup; the application state remains locked.
      element.disabled = false;
      element.focus();
    });
    for (let presses = 0; presses < 20; presses += 1) {
      await page.keyboard.press("Tab");
      const reachedOutside = await page.evaluate(() => {
        const active = document.activeElement;
        const popup = document.querySelector(".profiles-popover");
        if (
          !(active instanceof HTMLElement) ||
          active === document.body ||
          popup?.contains(active) ||
          active.matches(".profiles-toggle")
        ) {
          return false;
        }
        active.id = "trusted-tab-destination";
        return true;
      });
      if (reachedOutside) break;
    }
    const destination = page.locator("#trusted-tab-destination");
    await expect(destination).toBeFocused();

    save.release();
    await expect(section(page)).toHaveCount(0, { timeout: 20_000 });
    await expect(destination).toBeFocused();
  });

  /**
   * Trusted Tab provenance survives when the last page control yields focus
   * to `body` or browser chrome and no outside focusin follows. Releasing the
   * held mutation dismisses the popup without completion pulling focus back.
   */
  test("Tab leaving the document preserves busy dismissal intent", async ({ page, request }) => {
    const profile = await createProfile(request, { name: `tab-document-${Date.now()}` });
    profiles.push(profile.id);
    await listWithStubbedFeed(page);
    await openProfiles(page);
    const save = await beginHeldProfileSave(page, profile);
    await save.row.locator(".profile-save").evaluate((origin: HTMLButtonElement) => {
      for (const element of document.querySelectorAll<HTMLElement>(
        "button, a, input, select, textarea, [tabindex]",
      )) {
        element.tabIndex = -1;
      }
      // Leave one final page tab stop inside the popup. The next real Tab
      // crosses the document boundary instead of landing on an outside node.
      origin.disabled = false;
      origin.tabIndex = 0;
      origin.focus();
    });
    await expect(save.row.locator(".profile-save")).toBeFocused();
    await page.keyboard.press("Tab");
    await expect.poll(() => page.evaluate(() => document.activeElement === document.body))
      .toBe(true);

    save.release();
    await expect(section(page)).toHaveCount(0, { timeout: 20_000 });
    await expect.poll(() => page.evaluate(() => document.activeElement === document.body))
      .toBe(true);
  });

  /**
   * A page-owned focus call after the save handler claims the operation lock
   * is not user provenance, even when its focus event is browser-trusted. The
   * popup stays open after completion and the programmatic destination keeps
   * focus because completion may not replace an outside active control.
   */
  test("same-turn programmatic focus is not busy dismissal intent", async ({ page, request }) => {
    const profile = await createProfile(request, { name: `programmatic-outside-${Date.now()}` });
    profiles.push(profile.id);
    await listWithStubbedFeed(page);
    await openProfiles(page);
    await page.evaluate(() => {
      const handler = (event: MouseEvent) => {
        const target = event.target;
        if (!(target instanceof Element) || !target.closest(".profile-save")) return;
        window.removeEventListener("click", handler);
        (document.querySelector(".host-details-toggle") as HTMLButtonElement).focus();
      };
      window.addEventListener("click", handler);
    });
    const save = await beginHeldProfileSave(page, profile);
    const destination = page.locator(".host-details-toggle");
    await expect(destination).toBeFocused();

    save.release();
    await expect(save.row.locator(".profile-edit")).toBeVisible({ timeout: 20_000 });
    await expect(section(page)).toBeVisible();
    await expect(destination).toBeFocused();
  });

  /**
   * Classification can start while idle and find the operation lock busy only
   * after its await. The obligation must remain armed through that late claim
   * and dismiss as soon as the held mutation releases the lock.
   */
  test("a late busy claim rearms an in-flight focus-out classifier", async ({
    page,
    request,
  }) => {
    const profile = await createProfile(request, { name: `late-busy-${Date.now()}` });
    profiles.push(profile.id);
    let releaseSave: (() => void) | undefined;
    const heldSave = new Promise<void>((resolve) => {
      releaseSave = resolve;
    });
    await page.route(
      (url) => new RegExp(`/api/profiles/${profile.id}$`).test(url.pathname),
      async (route: Route) => {
        if (route.request().method() !== "POST") {
          await route.continue();
          return;
        }
        await heldSave;
        await route.continue();
      },
    );
    await listWithStubbedFeed(page);
    await openProfiles(page);
    const row = profileRow(page, profile.id);
    await row.locator(".profile-edit").click();
    await row.locator(".profile-name-input").fill(`${profile.name}-saved`);
    // Opening the editor replaced the focused edit button, and that transit
    // starts a classifier of its own. Let it finish before arming the hold,
    // or on a slow machine it consumes the hold instead of the inert click's
    // classifier, which then runs free and dismisses the popup under this
    // test. Quiescence is "no new classification attempt for a while".
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles = { classificationAttempts: 0 };
    });
    await expect
      .poll(
        async () => {
          const before = await page.evaluate(
            () => (window as any).__farhelmTestProfiles.classificationAttempts,
          );
          await page.waitForTimeout(400);
          const after = await page.evaluate(
            () => (window as any).__farhelmTestProfiles.classificationAttempts,
          );
          return after === before;
        },
        { timeout: 20_000, intervals: [100] },
      )
      .toBe(true);
    await page.evaluate(() => {
      (window as any).__farhelmTestProfiles = {
        classification: { holds: 1, started: 0, releases: [] },
      };
    });
    const point = await inertSidebarPoint(page);
    await page.mouse.click(point.x, point.y);
    await expect.poll(() =>
      page.evaluate(() => (window as any).__farhelmTestProfiles.classification.started)
    ).toBe(1);
    await row.locator(".profile-save").dispatchEvent("click");
    await expect(row.locator(".profile-save")).toBeDisabled();
    await page.evaluate(() => (window as any).__farhelmTestProfiles.classification.releases.shift()());
    await expect(section(page)).toBeVisible();
    releaseSave!();
    await expect(section(page)).toHaveCount(0, { timeout: 20_000 });
  });

  /**
   * Profiles and filters are mutually exclusive in both opening directions.
   * Testing each direction prevents two independent reactive effects from
   * drifting into an asymmetric two-popover state.
   */
  test("profiles and filters exclude each other in both opening directions", async ({ page }) => {
    await listWithStubbedFeed(page);

    await page.locator(".filter-toggle").click();
    await expect(page.locator(".filter-popover")).toBeVisible();
    await openProfiles(page);
    await expect(page.locator(".filter-popover")).toHaveCount(0);
    await closeProfiles(page);

    await openProfiles(page);
    // The open popup physically covers this control, so drive mutual exclusion
    // directly rather than pretending a pointer can reach it.
    await page.locator(".filter-toggle").dispatchEvent("click");
    await expect(page.locator(".filter-popover")).toBeVisible();
    await expect(section(page)).toHaveCount(0);
  });

  /**
   * A refused competing surface may focus its own toggle as part of handling
   * the refusal, but that programmatic side effect is not an outside choice.
   * The busy profiles operation therefore keeps its popup and confirmation.
   */
  test("a host menu refused by the operation token leaves the profiles popup alone", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const profile = await createProfile(request, { name: `fold-${Date.now()}` });
    profiles.push(profile.id);

    // A profile delete that never answers holds the page's operation token.
    let release: (() => void) | undefined;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route(
      (url) => /^\/api\/profiles\/[^/]+$/.test(url.pathname),
      async (route: Route) => {
        if (route.request().method() !== "DELETE") {
          await route.continue();
          return;
        }
        await held;
        await fulfillRefusal(route, request, "refused");
      },
    );
    await listWithStubbedFeed(page);
    // The row's menu toggle is always mounted, so details stay closed: an
    // expanded details column keeps resizing as host reads land, and the popup
    // answers any such layout change by closing once the operation settles,
    // which is not the refusal path this test is about.
    await openProfiles(page);
    await profileRow(page, profile.id).locator(".profile-delete").click();
    await profileRow(page, profile.id).locator(".profile-confirm-delete").click();

    // With the token held, an attempted host menu closes immediately and the
    // popup survives. This preserves one floating surface while keeping the
    // in-flight reply's destination mounted.
    const localRow = page.locator(`[data-host-id="${local}"]`);
    // The open popup physically covers this control, so drive the busy-surface
    // contract directly rather than pretending a pointer can reach it.
    await localRow.locator(".host-row-menu").dispatchEvent("click");
    await expect(localRow.locator(".host-row-menu-panel")).toHaveCount(0);
    await expect(
      section(page),
      "an incompatible popover must not discard the busy popup",
    ).toBeVisible();

    release!();
    await expect(profileRow(page, profile.id).locator(".profile-error")).toBeVisible({
      timeout: 20_000,
    });
  });

  /**
   * A save the helm REFUSES is shown on the open editor, as the helm's own
   * sentence, and is not retried; the request carries no precondition.
   *
   * Profile writes are last-write-wins (SPEC.md, Concepts / Agent profile):
   * the request names nothing about which connection or which definition it
   * was prepared against, and a 409 from a profile route is an ordinary
   * refusal rather than "the world moved, re-read". What is pinned is the
   * client's half of that contract — the wire body has no `expected_*`
   * fields, the editor stays open with the draft still in it (a refused name
   * is usually one keystroke from an accepted one), and the client does not
   * insist by resubmitting. The refusal is staged because the helm has no
   * reason of its own to refuse a well-formed edit here.
   */
  test("a refused save is shown on the open editor and does not retry", async ({
    page,
    request,
  }) => {
    const profile = await createProfile(request, { name: `refused-${Date.now()}` });
    profiles.push(profile.id);

    const sent: Record<string, unknown>[] = [];
    await page.route(
      (url) => /^\/api\/profiles\/[^/]+$/.test(url.pathname),
      async (route: Route) => {
        if (route.request().method() !== "POST") {
          await route.continue();
          return;
        }
        sent.push(route.request().postDataJSON());
        await fulfillRefusal(route, request, "the helm declined this edit");
      },
    );

    await listWithStubbedFeed(page);
    await openProfiles(page);
    const editing = profileRow(page, profile.id);
    await expect(editing).toBeVisible({ timeout: 20_000 });
    await editing.locator(".profile-edit").click();
    const draft = `refused-${Date.now()}-edited`;
    await editing.locator(".profile-name-input").fill(draft);
    await editing.locator(".profile-save").click();

    const formError = editing.locator(".profile-form-error");
    await expect(formError).toBeVisible({ timeout: 20_000 });
    await expect(formError).toContainText("declined this edit");
    await expect(
      editing.locator(".profile-form"),
      "a refusal leaves the editor open with the draft still in it",
    ).toHaveCount(1);
    // The draft's VALUE, not merely the form's existence: a form rebuilt and
    // reseeded from the catalog would pass the count assertion while losing
    // the user's edit.
    expect(
      await editing.locator(".profile-name-input").inputValue(),
      "the draft survives the refusal byte for byte",
    ).toBe(draft);
    await expect(
      section(page).locator(".profiles-notice"),
      "an ordinary refusal is the form's business, not a section-level notice",
    ).toHaveCount(0);

    // Exactly one attempt: the save's operation token is released only after
    // the refusal has been rendered, so once the form error is on screen any
    // retry the client was going to make has had its chance. The body carries
    // no precondition of either kind, which is the contract SPEC.md states
    // for profile writes.
    await expect(section(page).locator(".new-profile-button")).toBeEnabled({
      timeout: 20_000,
    });
    expect(sent.length, "a refusal must never be retried automatically").toBe(1);
    expect(sent[0].expected_incarnation, "profile writes name no connection").toBeUndefined();
    expect(sent[0].expected_definition, "and no definition fingerprint").toBeUndefined();
  });

  /**
   * Two real clients: an edit made through ONE browser's panel reaches the
   * other's open profiles popup, with no stub and no injected notification
   * anywhere.
   *
   * This is the milestone's multi-client promise for profiles, executable —
   * and it is the one test here that a stubbed feed cannot stand in for. A
   * stub proves the observer re-reads when TOLD to; only the real path proves
   * the helm tells it, which is a mutation that fails to publish away from
   * being silently broken.
   *
   * The observer therefore runs on the real socket. Everything it asserts is
   * bounded by generous timeouts rather than by a settle-then-notify handshake,
   * because on this path the test does not control when the news arrives.
   *
   * ## What this cannot prove, stated rather than implied
   *
   * The stack is shared and never truly quiet — another spec's session
   * settling into `idle` bumps the same revision counter — so a re-read that
   * follows the edit cannot be attributed to the edit with certainty by
   * watching the observer alone. What IS established is the pair that makes
   * the failure it guards against impossible to pass silently: the observer's
   * socket was open before the edit (so it cannot have re-read merely because
   * it connected), it stayed open throughout (so no reconnect handshake
   * explains the update), and the new name appeared on it without this test
   * injecting anything. A helm that published nothing would leave the observer
   * showing the old name until some unrelated churn happened to arrive — which
   * is exactly why the deterministic half of this contract is pinned by the
   * stubbed test above, where silence is guaranteed.
   */
  test("a profile edited in another browser reaches this one over the real feed", async ({
    page,
    browser,
    request,
  }) => {
    const before = `two-client-${Date.now()}`;
    const after = `${before}-elsewhere`;
    const profile = await createProfile(request, { name: before });
    profiles.push(profile.id);

    // The OBSERVER: a real page with a real feed, and the socket is proven up
    // BEFORE the edit is made. Without that, an observer that connected late
    // would re-read on its own handshake and look exactly like one that was
    // told — which is the difference this test exists to establish.
    const observerSockets = watchFeedSockets(page);
    await page.goto("/");
    await expect
      .poll(() => observerSockets.greeted(), {
        timeout: 20_000,
        message:
          "the observer must be SUBSCRIBED before the edit is made — a socket that has only been " +
          "requested proves nothing, and one greeted afterwards would re-read on its own handshake",
      })
      .toBeGreaterThan(0);
    await openProfiles(page);
    const observed = profileRow(page, profile.id).locator(".profile-name");
    await expect(observed).toHaveText(before, { timeout: 20_000 });

    const second = await browser.newContext({ baseURL: new URL(page.url()).origin });
    try {
      const editor = await second.newPage();
      await editor.goto("/");
      await openProfiles(editor);
      const editing = profileRow(editor, profile.id);
      await expect(editing).toBeVisible({ timeout: 20_000 });
      await editing.locator(".profile-edit").click();
      await editing.locator(".profile-name-input").fill(after);
      await editing.locator(".profile-save").click();
      await expect(editing.locator(".profile-name")).toHaveText(after, { timeout: 20_000 });

      await expect(
        observed,
        "the helm must PUBLISH a profile edit, and the other client must re-read on it — with " +
          "nothing in this test telling it to",
      ).toHaveText(after, { timeout: 30_000 });
      expect(
        observerSockets.closed(),
        "and it must have arrived on the socket that was already open, not on a reconnect that " +
          "would have re-read regardless",
      ).toBe(0);
    } finally {
      await second.close();
    }
  });
});

/**
 * Wait until the helm's own merged view reports `existence` for the session
 * with this title.
 *
 * The settle half of the stubbed-feed convention, and it is needed for exactly
 * one reason: a session's `existence` is derived per reply by the owning
 * supervisor and reaches the merged list only when the helm's session cache
 * next refreshes. A notification played before that lands tells the page to
 * re-read a view that has not moved — which passes or fails on timing rather
 * than on the rule under test.
 *
 * Deliberately NOT used for catalog reads: those are live reads from the
 * supervisor, so an awaited mutation has already settled by the time it
 * answers, and a poll there would assert nothing.
 */
async function settleExistence(
  request: Parameters<typeof listSessions>[0],
  title: string,
  existence: string,
): Promise<void> {
  await expect
    .poll(
      async () =>
        (await listSessions(request, `title=${encodeURIComponent(title)}`)).sessions[0]
          ?.source_profile?.existence,
      {
        timeout: 20_000,
        message:
          "the helm's own view must carry the derived existence before the page is told to " +
          "re-read; until its session cache refreshes there is nothing new to see",
      },
    )
    .toBe(existence);
}
