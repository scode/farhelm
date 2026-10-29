/**
 * Update reminders must use the rail's actual update set, not the broader
 * Hosts-panel Update all offer. Browser routes supply the same explicit age
 * flags as the existing host-version tests: a development helm deliberately
 * does not infer optional updates from version strings.
 */
import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { drainedRoute } from "./helpers/drained-route";
import {
  cleanupSession,
  createSession,
  observeFeedReaders,
  patchPreferences,
  readPreferences,
  showSessionsPanel,
  stubFeed,
  waitForFeedReadersSettled,
} from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { expectKeyboardScroll } from "./helpers/keyboard-scroll";
import { routeGate } from "./helpers/route-gate";

type Age = "optional" | "required" | "current" | "newer" | "unreachable";
type FixtureHost = { id: number; name: string; kind: string; state: Record<string, unknown> } & Record<string, unknown>;
const IDS = { one: 9411, two: 9412, current: 9413, unreachable: 9414, newer: 9415, busy: 9416 };
const cleanup: [string, () => unknown | Promise<unknown>][] = [];
const releases: (() => void)[] = [];

/** Release held responses before draining their handlers, even after failure. */
test.afterEach(async ({}, info) => {
  for (const release of releases.splice(0)) release();
  await cleanupOwnedSteps(info.status !== info.expectedStatus, cleanup.splice(0).reverse());
});

/** Keep immutable IDs and destinations while changing only reported age. */
function remote(id: number, name: string, age: Age, build: string): FixtureHost {
  const state = age === "required"
    ? {
      phase: "version-skew",
      peer_protocol: 1,
      our_protocol: 2,
      peer_build: "0.1.0",
      our_build: build,
      remediation: "update the host",
    }
    : age === "unreachable"
    ? { phase: "unreachable-reprobing", cause: "transport-failure", last_error: "fixture host is offline" }
    : {
      phase: "connected",
      identity: `fixture-${id}`,
      build_version: age === "current" ? build : "0.1.0",
      old_version: age === "optional",
      newer_version: age === "newer",
      refresh: { status: "ok", sessions: 0 },
    };
  return {
    id,
    kind: "ssh",
    destination: `e2e@host-${id}.invalid`,
    alias: name,
    name,
    identity: `fixture-${id}`,
    incarnation: id,
    remote_farhelm: null,
    remote_state_dir: null,
    state,
  };
}

/** Find the same rail identity that the reminder classifier must agree with. */
function avatar(page: Page, id: number) {
  return page.locator(`.host-rail-avatar[data-rail-host-id="${id}"]`);
}

/**
 * Freeze host/provisioning facts without registering or updating any real host.
 * The real local row is retained, and every fake host gets a complete progress
 * reply so a 404 cannot accidentally suppress its Update offer. A generation
 * marker on the current host proves each later publication reached the DOM.
 */
async function prepare(page: Page, request: APIRequestContext) {
  const preferences = await readPreferences(request);
  cleanup.push([
    "restore welcome preference",
    () => patchPreferences(request, { welcome_seen: preferences.welcome_seen ?? null }),
  ]);
  await patchPreferences(request, { welcome_seen: true });
  const response = await request.get("/api/hosts");
  expect(response.ok(), await response.text()).toBe(true);
  const originalBuild = requireHelmBuild(response, "host-update reminders");
  const registry = await response.json() as { hosts: FixtureHost[] };
  const local = registry.hosts.find((host) => host.kind === "local");
  expect(local, "fixture must preserve the real reserved local host").toBeTruthy();
  expect(local!.state.phase, "the retained local fixture must be connected").toBe("connected");
  expect(Object.values(IDS)).not.toContain(local!.id);
  let build = originalBuild;
  let generation = 0;
  const ages = new Map<number, Age>([
    [IDS.one, "optional"],
    [IDS.two, "optional"],
    [IDS.current, "current"],
    [IDS.unreachable, "unreachable"],
    [IDS.newer, "newer"],
    [IDS.busy, "optional"],
  ]);
  const names = new Map<number, string>([
    [IDS.one, "build-one"],
    [IDS.two, "build-two"],
    [IDS.current, "current"],
    [IDS.unreachable, "offline"],
    [IDS.newer, "too-new"],
    [IDS.busy, "busy"],
  ]);
  const busy = new Set<number>([IDS.busy]);
  const updates: { id: number; method: string; body: string | null }[] = [];
  const admission = routeGate();
  releases.push(admission.release);
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  cleanup.push(["close fixture feed", () => {
    if (feed.openSockets() > 0) feed.kill();
  }]);
  const headers = () => ({ "x-farhelm-build": build, "content-type": "application/json" });
  cleanup.push([
    "listing build stamps",
    await drainedRoute(page, (url) => url.pathname === "/api/sessions", async (route) => {
      const response = await route.fetch();
      await route.fulfill({ response, headers: { ...response.headers(), "x-farhelm-build": build } });
    }),
  ]);
  const hosts = () => [
    local!,
    ...[...ages].map(([id, age]) =>
      remote(
        id,
        id === IDS.current ? `current-${generation}` : names.get(id)!,
        age,
        originalBuild,
      )
    ),
  ];
  cleanup.push([
    "host replies",
    await drainedRoute(page, (url) => url.pathname === "/api/hosts", async (route) => {
      expect(route.request().method()).toBe("GET");
      await route.fulfill({ headers: headers(), json: { hosts: hosts() } });
    }),
  ]);
  cleanup.push([
    "progress replies",
    await drainedRoute(page, (url) => /^\/api\/hosts\/\d+\/provisioning$/.test(url.pathname), async (route) => {
      expect(route.request().method()).toBe("GET");
      const id = Number(new URL(route.request().url()).pathname.split("/")[3]);
      expect([local!.id, ...ages.keys()]).toContain(id);
      await route.fulfill({
        headers: headers(),
        json: {
          host_id: id,
          run_id: busy.has(id) ? `busy-${id}` : null,
          operation: busy.has(id) ? "add" : null,
          status: busy.has(id) ? "running" : "completed",
          steps: busy.has(id)
            ? [{ step: "attach-supervisor", name: "connect to the supervisor", status: "running" }]
            : [],
          message: null,
        },
      });
    }),
  ]);
  cleanup.push([
    "update replies",
    await drainedRoute(page, (url) => /^\/api\/hosts\/\d+\/update$/.test(url.pathname), async (route) => {
      const outgoing = route.request();
      updates.push({
        id: Number(new URL(outgoing.url()).pathname.split("/")[3]),
        method: outgoing.method(),
        body: outgoing.postData(),
      });
      await admission.wait();
      await route.fulfill({
        status: 409,
        headers: headers(),
        body: JSON.stringify({ error: "fixture stops at Update planning" }),
      });
    }),
  ]);
  await page.goto("/");
  await expect(page.locator(`[data-host-id="${IDS.current}"] .host-name`)).toHaveText("current-0");
  await expect(avatar(page, IDS.one).locator(".host-rail-update")).toHaveCount(1);
  await expect(avatar(page, IDS.two).locator(".host-rail-update")).toHaveCount(1);
  await expect(page.locator(`[data-host-id="${IDS.busy}"] .provisioning-run`)).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(avatar(page, IDS.busy).locator(".host-rail-update")).toHaveCount(0);
  let revision = 1;
  return {
    updates,
    localId: local!.id,
    originalBuild,
    setAge(id: number, age: Age) {
      ages.set(id, age);
    },
    setName(id: number, name: string) {
      names.set(id, name);
    },
    setBuild(next: string) {
      build = next;
    },
    /** A real filter action still reads through Rust after skew withdraws the feed. */
    async observeBuildThroughFilter(hostId?: number) {
      const reply = page.waitForResponse((response) =>
        new URL(response.url()).pathname === "/api/sessions" &&
        response.request().method() === "GET" && response.headers()["x-farhelm-build"] === build
      );
      const choice = hostId === undefined ? page.locator(".host-rail-all") : avatar(page, hostId);
      await choice.click();
      await expect(choice).toHaveAttribute("aria-pressed", "true");
      const response = await reply;
      expect(response.ok(), "the attended listing read must succeed").toBe(true);
      expect(await response.finished()).toBeNull();
    },
    /** Feed notification and a rendered marker jointly prove new facts arrived. */
    async publish() {
      generation++;
      await expect.poll(() => feed.openSockets(), { message: "fixture feed must be live before notifying" })
        .toBeGreaterThan(0);
      feed.notify(++revision);
      await expect(page.locator(`[data-host-id="${IDS.current}"] .host-name`)).toHaveText(`current-${generation}`);
    },
  };
}

/** The badge's urgency is visible paint, not just a classifier attribute. */
async function expectBadge(page: Page, count: number, urgency: "optional" | "required") {
  const toggle = page.locator(".hosts-panel-toggle");
  const phrase = count === 1 ? "1 host has an update" : `${count} hosts have updates`;
  await expect(toggle).toHaveAccessibleName(`Hosts: ${phrase}`);
  await expect(toggle).toHaveAttribute("data-tooltip", new RegExp(phrase));
  const badge = toggle.locator(".hosts-toggle-update");
  await expect(badge).toBeVisible();
  await expect(badge).toHaveText("↑");
  await expect(badge).toHaveAttribute("data-update-kind", urgency);
  const paint = await badge.evaluate((node, kind) => {
    const style = getComputedStyle(node);
    const token = getComputedStyle(document.documentElement).getPropertyValue(
      kind === "required" ? "--danger-strong" : "--warn",
    ).trim();
    const probe = document.createElement("span");
    probe.style.color = token;
    document.body.append(probe);
    const expected = getComputedStyle(probe).color;
    probe.remove();
    return { actual: style.color, expected, token };
  }, urgency);
  expect(paint.token, "the urgency palette token must exist").not.toBe("");
  expect(paint.actual, "the rendered arrow must carry its urgency tone").toBe(paint.expected);
}

/** Exercise the supported minimum width through the actual separator gesture. */
async function narrowSidebar(page: Page): Promise<void> {
  const separator = page.getByRole("separator", { name: "Sidebar width" });
  await expect(separator).toHaveAttribute("data-sidebar-bound", "true");
  const box = (await separator.boundingBox())!;
  const current = Number(await separator.getAttribute("aria-valuenow"));
  const x = box.x + box.width / 2;
  const y = box.y + Math.min(200, box.height / 2);
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 296 - current, y, { steps: 8 });
  await page.mouse.up();
  await expect(separator).toHaveAttribute("aria-valuenow", "296");
  await page.evaluate(() => document.fonts.ready);
}

/**
 * A visible button can still be clipped or covered. At the supported 240px
 * content minimum, require contained geometry and the actual pointer target.
 * Screenshots retained by the callers also expose the card-to-Hosts arrow.
 */
async function expectReminderFits(page: Page, reminder: Locator): Promise<void> {
  await expect(reminder).toBeVisible();
  const sidebar = (await page.locator(".app-sidebar").boundingBox())!;
  const bounds = (await reminder.boundingBox())!;
  expect(bounds.x).toBeGreaterThanOrEqual(sidebar.x);
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(sidebar.x + sidebar.width);
  expect(await reminder.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
  for (const button of await reminder.getByRole("button").all()) {
    await expect(button).toBeVisible();
    const box = (await button.boundingBox())!;
    expect(box.x).toBeGreaterThanOrEqual(bounds.x);
    expect(box.x + box.width).toBeLessThanOrEqual(bounds.x + bounds.width);
    expect(box.y).toBeGreaterThanOrEqual(bounds.y);
    expect(box.y + box.height).toBeLessThanOrEqual(bounds.y + bounds.height);
    expect(
      await button.evaluate((node) => {
        const box = node.getBoundingClientRect();
        return node.contains(document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2));
      }),
      "each reminder action must be hit-testable",
    ).toBe(true);
  }
  await expect(page.locator(".hosts-panel-toggle")).toBeVisible();
}

/**
 * The card belongs at the sidebar's bottom, with its arrow pointing at Hosts
 * even when the names region grows. Derive the transformed arrow tip from
 * its actual CSS box rather than assuming a fixed card height. Two pixels
 * allow border/subpixel rounding without accepting a misplaced reminder.
 */
async function expectCardPlacement(page: Page, card: Locator): Promise<void> {
  const placement = await card.evaluate((node) => {
    const sidebar = document.querySelector(".app-sidebar")!;
    const toggle = document.querySelector(".hosts-panel-toggle")!;
    const box = node.getBoundingClientRect();
    const outer = sidebar.getBoundingClientRect();
    const target = toggle.getBoundingClientRect();
    const style = getComputedStyle(node);
    const parent = getComputedStyle(sidebar);
    const arrow = getComputedStyle(node, "::before");
    const pixels = (value: string) => Number.parseFloat(value) || 0;
    const width = pixels(arrow.width) +
      (arrow.boxSizing === "border-box"
        ? 0
        : pixels(arrow.paddingLeft) + pixels(arrow.paddingRight) + pixels(arrow.borderLeftWidth) +
          pixels(arrow.borderRightWidth));
    const height = pixels(arrow.height) +
      (arrow.boxSizing === "border-box"
        ? 0
        : pixels(arrow.paddingTop) + pixels(arrow.paddingBottom) + pixels(arrow.borderTopWidth) +
          pixels(arrow.borderBottomWidth));
    // The decorated left/bottom corner is the tip of the rotated square.
    const tip = new DOMPoint(-width / 2, height / 2).matrixTransform(new DOMMatrix(arrow.transform));
    const tipY = box.bottom - pixels(style.borderBottomWidth) - pixels(arrow.bottom) - height / 2 + tip.y;
    return {
      inContentScroller: !!node.closest(".sidebar-content"),
      actualBottom: box.bottom,
      expectedBottom: outer.bottom - pixels(parent.borderBottomWidth) - pixels(parent.paddingBottom) -
        pixels(style.marginBottom),
      tipY,
      hostsCenterY: target.y + target.height / 2,
      arrowContent: arrow.content,
      arrowWidth: width,
      arrowHeight: height,
    };
  });
  expect(placement.inContentScroller, "the complete card must not scroll with session content").toBe(false);
  expect(placement.arrowContent).not.toBe("none");
  expect(placement.arrowWidth).toBeGreaterThan(0);
  expect(placement.arrowHeight).toBeGreaterThan(0);
  expect(
    Math.abs(placement.actualBottom - placement.expectedBottom),
    "card bottom must follow the sidebar bottom margin",
  ).toBeLessThanOrEqual(2);
  expect(Math.abs(placement.tipY - placement.hostsCenterY), "the arrow tip must align vertically with Hosts")
    .toBeLessThanOrEqual(2);
}

/**
 * Measure usable session access through the actual content viewport, whose
 * height is independent of the intrinsic height of one row. The row must be
 * backed by the real registry/listing and remain fully visible with its open
 * and menu targets reachable after ordinary scrolling and hover.
 */
async function expectSessionAccess(page: Page, request: APIRequestContext, localId: number): Promise<void> {
  const response = await request.get("/api/sessions");
  expect(response.ok(), await response.text()).toBe(true);
  const listing = await response.json() as { sessions: { id: string; host: number }[] };
  const session = listing.sessions.find((candidate) => candidate.host === localId);
  expect(session, "the short-window fixture must contain a real local session").toBeTruthy();
  const row = page.locator(`[data-session-id="${session!.id}"]`);
  await expect(row).toBeVisible();
  await row.scrollIntoViewIfNeeded();
  await row.hover();
  const access = await row.evaluate((node) => {
    const content = document.querySelector(".sidebar-content")!;
    const viewport = content.getBoundingClientRect();
    const box = node.getBoundingClientRect();
    return {
      clientWidth: content.clientWidth,
      clientHeight: content.clientHeight,
      top: box.top,
      bottom: box.bottom,
      left: box.left,
      right: box.right,
      viewportTop: viewport.top,
      viewportBottom: viewport.bottom,
      viewportLeft: viewport.left,
      viewportRight: viewport.right,
    };
  });
  expect(access.clientWidth).toBeGreaterThan(0);
  expect(access.clientHeight).toBeGreaterThan(0);
  expect(access.top).toBeGreaterThanOrEqual(access.viewportTop - 1);
  expect(access.bottom).toBeLessThanOrEqual(access.viewportBottom + 1);
  expect(access.left).toBeGreaterThanOrEqual(access.viewportLeft - 1);
  expect(access.right).toBeLessThanOrEqual(access.viewportRight + 1);
  for (const selector of [".session-row-open", ".session-row-menu"]) {
    const control = row.locator(selector);
    await expect(control).toBeVisible();
    expect(
      await control.evaluate((node) => {
        const box = node.getBoundingClientRect();
        return node.contains(document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2));
      }),
      `the real session's ${selector} must remain hit-testable`,
    ).toBe(true);
  }
}

/**
 * Current, unreachable, local and busy rows can all have broad panel offers;
 * only the two rail arrows authorize this narrower batch. Hold planning so
 * the exact sent identities and bodyless Update contract stay observable.
 */
test("optional reminder lists and updates exactly the rail update set", async ({ page, request }, info) => {
  const fixture = await prepare(page, request);
  await expectBadge(page, 2, "optional");
  const card = page.locator(".host-update-card");
  await expect(card).toBeVisible();
  await expect(card).toContainText("2 hosts have updates");
  await expect(card.locator(".host-update-names .peer-value")).toHaveText(["build-one", "build-two"]);
  await expect(page.locator(".host-update-banner")).toHaveCount(0);
  for (const id of [fixture.localId, IDS.current, IDS.unreachable, IDS.newer, IDS.busy]) {
    await expect(avatar(page, id).locator(".host-rail-update")).toHaveCount(0);
  }
  await narrowSidebar(page);
  await expectReminderFits(page, card);
  await expectCardPlacement(page, card);
  await info.attach("optional-reminder-minimum-sidebar", {
    body: await page.locator(".app-sidebar").screenshot(),
    contentType: "image/png",
  });
  await card.getByRole("button", { name: "Show hosts", exact: true }).click();
  await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByRole("region", { name: "Host management", exact: true })).toBeVisible();
  await expect(card).toHaveCount(0);
  await expect(page.locator(".host-update-banner")).toHaveCount(0);
  await expectBadge(page, 2, "optional");
  await showSessionsPanel(page);
  await expect(card).toBeVisible();
  await card.getByRole("button", { name: "Update all now", exact: true }).click();
  await expect.poll(() => fixture.updates.map((sent) => sent.id).sort()).toEqual([IDS.one, IDS.two]);
  for (const id of [IDS.one, IDS.two]) {
    await expect(page.locator(`[data-host-id="${id}"] .host-update-pending`)).toBeAttached();
  }
  expect(fixture.updates).toEqual(expect.arrayContaining([
    { id: IDS.one, method: "POST", body: null },
    { id: IDS.two, method: "POST", body: null },
  ]));
  expect(fixture.updates).toHaveLength(2);
});

/**
 * Dismissal remembers the shown identities, rather than count or current
 * membership. Removing and restoring an acknowledged host is still quiet;
 * another host, a page restart or a different helm build is new attention.
 */
test("optional dismissal retains its badge until new attention or reload", async ({ page, request }) => {
  const fixture = await prepare(page, request);
  const card = page.locator(".host-update-card");
  const dismiss = () => card.getByRole("button", { name: "Dismiss host update reminder", exact: true }).click();
  await expect(card).toBeVisible();
  await dismiss();
  await expect(card).toHaveCount(0);
  await expectBadge(page, 2, "optional");
  fixture.setAge(IDS.two, "current");
  await fixture.publish();
  await expectBadge(page, 1, "optional");
  await expect(card).toHaveCount(0);
  fixture.setAge(IDS.two, "optional");
  await fixture.publish();
  await expectBadge(page, 2, "optional");
  await expect(card).toHaveCount(0);
  fixture.setAge(IDS.unreachable, "optional");
  await fixture.publish();
  await expect(card).toBeVisible();
  await expect(card).toContainText("3 hosts have updates");
  await expect(card.locator(".host-update-names .peer-value")).toHaveText(["build-one", "build-two", "offline"]);
  await dismiss();
  await expect(card).toHaveCount(0);
  await page.reload();
  await expect(card).toBeVisible();
  await expectBadge(page, 3, "optional");
  await dismiss();
  await expect(card).toHaveCount(0);
  fixture.setBuild(`${fixture.originalBuild}-reminder-fixture`);
  await fixture.publish();
  await expect(page.locator(".build-skew")).toBeVisible();
  await expect(card).toBeVisible();
  await expectBadge(page, 3, "optional");
  // The mismatch notice deliberately remembers B forever. A later attended
  // reply naming C must independently retire the dismissal remembered for B.
  await fixture.observeBuildThroughFilter(fixture.localId);
  await dismiss();
  await expect(card).toHaveCount(0);
  fixture.setBuild(`${fixture.originalBuild}-another-reminder-fixture`);
  await fixture.observeBuildThroughFilter();
  await expect(card).toBeVisible();
  await expectBadge(page, 3, "optional");
  await expect(page.locator(".build-skew")).toContainText(`${fixture.originalBuild}-reminder-fixture`);
  expect(fixture.updates).toEqual([]);
});

/**
 * Banner insertion and removal move even compact local rows, despite the
 * unrelated remote host changing no displayed session detail. Freeze actual
 * listing facts to exclude sorting/status changes, and keep the pointer out
 * of the list so the menu's movement lifecycle is the only dismissal cause.
 */
test("required banner movement retires existing compact session menus and preserves fresh openings", async ({ page, request }) => {
  const session = await createSession(request, { title: "update-banner-anchor", invocation: "sleep 300" });
  cleanup.push(["remove anchor session", () => cleanupSession(request, session.id)]);
  const previous = await readPreferences(request);
  cleanup.push(["restore density", () => patchPreferences(request, { compact: previous.compact ?? null })]);
  await patchPreferences(request, { compact: true });
  await observeFeedReaders(page);
  const fixture = await prepare(page, request);
  const reply = await request.get("/api/sessions");
  expect(reply.ok(), await reply.text()).toBe(true);
  const listing = await reply.json();
  const owned = listing.sessions.find((candidate: { id: string; host: number }) => candidate.id === session.id);
  expect(owned, "the real session must exist before freezing its listing").toBeTruthy();
  expect(owned.host, "remote update changes must not change the measured local session").toBe(fixture.localId);
  cleanup.push([
    "frozen anchor listing",
    await drainedRoute(page, (url) => url.pathname === "/api/sessions", async (route) => {
      await route.fulfill({ headers: { "x-farhelm-build": fixture.originalBuild }, json: listing });
    }),
  ]);
  await page.reload();
  const row = page.locator(`[data-session-id="${session.id}"]`);
  await row.locator(".session-row-open").click();
  await expect(row).toHaveAttribute("data-session-selected", "true");
  await waitForFeedReadersSettled(page);
  await expect(row.locator(".session-identity-copy")).toHaveClass(/(?:^| )compact(?: |$)/);
  const toggle = row.locator(".session-row-menu");
  const panel = row.locator(".session-row-menu-panel");
  const viewport = page.viewportSize()!;
  await page.mouse.move(viewport.width - 8, viewport.height - 8);
  /** A real keyboard opening measures the row without taking a pointer hold. */
  const open = async () => {
    expect(await page.locator(".session-list").evaluate((node) => node.matches(":hover"))).toBe(false);
    await toggle.focus();
    await expect(toggle).toBeFocused();
    await toggle.press("ArrowDown");
    await expect(row.locator(".session-row-rename")).toBeFocused();
    await expect(row.locator(".session-row-menu-flyout")).toHaveCSS("opacity", "1");
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
  };
  /** Capture competing movement inputs beside the actual anchor coordinate. */
  const geometry = () =>
    row.evaluate((node) => {
      const sidebar = document.querySelector(".app-sidebar")!;
      const scroll = document.querySelector(".sidebar-content")!;
      const list = document.querySelector(".session-list")!;
      const box = node.getBoundingClientRect();
      const outer = sidebar.getBoundingClientRect();
      return {
        y: box.y,
        x: box.x,
        width: box.width,
        outerWidth: outer.width,
        outerHeight: outer.height,
        scroll: scroll.scrollTop,
        listScroll: list.scrollTop,
        index: [...document.querySelectorAll(".session-row")].indexOf(node),
      };
    });
  for (const age of ["required", "optional"] as const) {
    await waitForFeedReadersSettled(page);
    await open();
    const before = await geometry();
    expect(before.scroll).toBe(0);
    expect(before.listScroll).toBe(0);
    fixture.setAge(IDS.one, age);
    await fixture.publish();
    await waitForFeedReadersSettled(page);
    await expect(page.locator(".host-update-banner")).toHaveCount(age === "required" ? 1 : 0);
    const after = await geometry();
    expect(after.y, "the banner must actually move this row").not.toBe(before.y);
    expect({ ...after, y: before.y }, "no competing scroll, row order or outer resize may explain dismissal").toEqual(
      before,
    );
    await expect(panel).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    // A fresh menu must survive the completed publication and a later host
    // read with identical prompt content; deferred reports cannot borrow it.
    await open();
    await fixture.publish();
    await waitForFeedReadersSettled(page);
    await expect(panel).toBeVisible();
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await page.keyboard.press("Escape");
    await expect(panel).toHaveCount(0);
  }
  expect(fixture.updates).toEqual([]);
});

/**
 * Several valid maximum-length aliases can exhaust a short window. Names
 * must scroll through a real Tab stop while fixed actions and the session
 * list retain usable space; merely rendering the buttons is insufficient.
 */
test("long update names scroll in a short narrow sidebar without hiding actions", async ({ page, request }, info) => {
  const fixture = await prepare(page, request);
  const ids = [IDS.one, IDS.two, IDS.unreachable, IDS.newer, 9417, 9418];
  const names = ids.map((id) => `host-${id}-` + "long-alias-".repeat(6)).map((name) => name.slice(0, 64));
  expect(names.every((name) => name.length === 64), "aliases exercise the supported 64-character maximum").toBe(true);
  for (const [index, id] of ids.entries()) {
    fixture.setName(id, names[index]);
    fixture.setAge(id, "optional");
  }
  await fixture.publish();
  const card = page.locator(".host-update-card");
  await expect(card.locator(".host-update-names .peer-value")).toHaveText(names);
  await expectBadge(page, ids.length, "optional");
  await page.setViewportSize({ width: 1024, height: 420 });
  await narrowSidebar(page);
  await expectReminderFits(page, card);
  const body = card.locator(".host-update-body");
  await expectKeyboardScroll(
    page,
    card.getByRole("button", { name: "Dismiss host update reminder", exact: true }),
    body,
  );
  await page.keyboard.press("End");
  await expect.poll(() => body.evaluate((node) => node.scrollTop + node.clientHeight >= node.scrollHeight - 1)).toBe(
    true,
  );
  const last = (await card.locator(".host-update-names .peer-value").last().boundingBox())!;
  const port = (await body.boundingBox())!;
  expect(last.y + last.height, "keyboard scrolling must reveal the last listed host").toBeLessThanOrEqual(
    port.y + port.height + 1,
  );
  await expectSessionAccess(page, request, fixture.localId);
  await expectReminderFits(page, card);
  await expectCardPlacement(page, card);
  await info.attach("long-update-names-short-sidebar", {
    body: await page.locator(".app-sidebar").screenshot(),
    contentType: "image/png",
  });
  await card.getByRole("button", { name: "Show hosts", exact: true }).click();
  await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "true");
  expect(fixture.updates).toEqual([]);
});

/**
 * A required update takes priority over optional advice and has no dismissal.
 * Its batch must exclude optional hosts even though the toggle counts both.
 */
test("required banner replaces optional advice and updates only required hosts", async ({ page, request }, info) => {
  const fixture = await prepare(page, request);
  fixture.setAge(IDS.one, "required");
  await fixture.publish();
  const banner = page.locator(".host-update-banner");
  await expectBadge(page, 2, "required");
  await expect(banner).toBeVisible();
  await expect(banner).toContainText("build-one must be updated before you can use it");
  await expect(banner).toContainText(/older protocol/);
  await expect(banner).toContainText(/unreachable/);
  await expect(banner.getByRole("button")).toHaveCount(2);
  await expect(banner.getByRole("button", { name: "↑ Update now", exact: true })).toBeVisible();
  await expect(page.locator(".host-update-card")).toHaveCount(0);
  await narrowSidebar(page);
  await expectReminderFits(page, banner);
  await info.attach("required-reminder-minimum-sidebar", {
    body: await page.locator(".app-sidebar").screenshot(),
    contentType: "image/png",
  });
  await page.keyboard.press("Escape");
  await expect(banner).toBeVisible();
  await banner.getByRole("button", { name: "Show hosts", exact: true }).click();
  await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "true");
  await expect(banner).toHaveCount(0);
  await expect(page.locator(".host-update-card")).toHaveCount(0);
  await showSessionsPanel(page);
  await expect(banner).toBeVisible();
  // Resolution restores optional advice without requiring a page restart.
  fixture.setAge(IDS.one, "current");
  await fixture.publish();
  await expect(banner).toHaveCount(0);
  await expect(page.locator(".host-update-card")).toContainText("1 host has an update");
  await expectBadge(page, 1, "optional");
  fixture.setAge(IDS.one, "required");
  fixture.setAge(IDS.unreachable, "required");
  await fixture.publish();
  await expectBadge(page, 3, "required");
  await expect(banner).toContainText("2 hosts must be updated before you can use them");
  await expect(banner.getByRole("button")).toHaveCount(2);
  await banner.getByRole("button", { name: "↑ Update all two", exact: true }).click();
  await expect.poll(() => fixture.updates.map((sent) => sent.id).sort()).toEqual([IDS.one, IDS.unreachable]);
  for (const id of [IDS.one, IDS.unreachable]) {
    await expect(page.locator(`[data-host-id="${id}"] .host-update-pending`)).toBeAttached();
  }
  expect(fixture.updates).toHaveLength(2);
  expect(fixture.updates.every((sent) => sent.method === "POST" && sent.body === null)).toBe(true);
});
