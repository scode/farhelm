/**
 * Hosts needing attention share one surface, while Update keeps the rail's
 * narrower eligibility. Routed registry and progress facts exercise the real
 * mounted readers without registering or updating a remote machine.
 */
import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { drainedRoute } from "./helpers/drained-route";
import {
  cleanupSession,
  createSession,
  observeFeedReaders,
  patchPreferences,
  readPreferences,
  showHostsPanel,
  showSessionsPanel,
  stubFeed,
  waitForFeedReadersSettled,
} from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { expectKeyboardScroll } from "./helpers/keyboard-scroll";
import { routeGate } from "./helpers/route-gate";
import { openSettings } from "./helpers/settings";

type Age = "optional" | "required" | "current" | "newer" | "unreachable" | "connecting";
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
    : age === "connecting"
    ? { phase: "connecting", attempt: 2, stage: "waiting-to-connect", last_error: "fixture SSH connection refused" }
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

/** Find the rail identity whose Update arrow must retain its old meaning. */
function avatar(page: Page, id: number) {
  return page.locator(`.host-rail-avatar[data-rail-host-id="${id}"]`);
}

/** Attention identities have their own hook; management keeps data-host-id. */
function attentionRow(page: Page, id: number) {
  return page.locator(`.host-attention-row[data-attention-host-id="${id}"]`);
}

/**
 * Freeze host/provisioning facts without registering or updating any real host.
 * The local row stays real. Complete progress replies prevent incidental 404s
 * from suppressing Update; a hidden management name proves feed publication.
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
  const originalBuild = requireHelmBuild(response, "host attention");
  const registry = await response.json() as { hosts: FixtureHost[] };
  const local = registry.hosts.find((host) => host.kind === "local");
  expect(local, "fixture must preserve the reserved local host").toBeTruthy();
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
  // Completed history must retain its run identity. Returning idle after a
  // known Running view would intentionally raise uncertainty in HostAttention.
  const knownRuns = new Set<number>(busy);
  const completed = new Map<number, number>();
  const stepCounts = new Map<number, number>();
  const connectionErrors = new Map<number, string>();
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
      const reply = await route.fetch();
      await route.fulfill({ response: reply, headers: { ...reply.headers(), "x-farhelm-build": build } });
    }),
  ]);
  const hosts = () => [
    local!,
    ...[...ages].map(([id, age]) => {
      const host = remote(id, id === IDS.current ? `current-${generation}` : names.get(id)!, age, originalBuild);
      if (age === "connecting" && connectionErrors.has(id)) host.state.last_error = connectionErrors.get(id);
      return host;
    }),
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
      const done = busy.has(id) ? completed.get(id) ?? 0 : 3;
      await route.fulfill({
        headers: headers(),
        json: {
          host_id: id,
          run_id: knownRuns.has(id) ? `busy-${id}` : null,
          operation: knownRuns.has(id) ? "add" : null,
          status: busy.has(id) ? "running" : "completed",
          steps: knownRuns.has(id)
            ? Array.from({ length: stepCounts.get(id) ?? 3 }, (_, index) => ({
              step: index === 0 ? "prepare-snapshot" : `fixture-step-${index}`,
              name: `setup step ${index + 1}`,
              explanation: index === 0 ? "Prepared once a day, or after Farhelm updates." : null,
              status: index < done ? "completed" : index === done ? "running" : "pending",
            }))
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
      await route.fulfill({ status: 409, headers: headers(), json: { error: "fixture stops at Update planning" } });
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
  await expect(avatar(page, IDS.busy)).toHaveAttribute("data-host-progress", "true");
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
    setBusy(id: number, running: boolean) {
      if (running) {
        busy.add(id);
        knownRuns.add(id);
      } else busy.delete(id);
    },
    setCompleted(id: number, done: number) {
      completed.set(id, done);
    },
    /** A one-step run has no disposable list height to mask rounding errors. */
    setStepCount(id: number, count: number) {
      stepCounts.set(id, count);
    },
    /** Keep a live connecting phase while varying only its peer diagnosis. */
    setConnectionError(id: number, error: string) {
      ages.set(id, "connecting");
      connectionErrors.set(id, error);
    },
    /** An attended filter read still observes new build stamps after skew stops the feed. */
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

/**
 * Keep automatic-update opt-in entirely in the page fixture. The routed hosts
 * do not exist in the test helm, so no PUT of this field may reach that helm
 * and authorize updates of its real hosts. Corrective GETs see the same fake
 * persisted value, while unrelated preference fields retain their real values.
 */
async function automaticUpdatePreference(page: Page, initiallyEnabled = false) {
  let enabled = initiallyEnabled;
  let refuse = false;
  let nextGate: ReturnType<typeof routeGate> | undefined;
  const writes: boolean[] = [];
  let reads = 0;
  cleanup.push(["automatic-update preference replies", await drainedRoute(page, "**/api/preferences", async (route) => {
    const outgoing = route.request();
    if (outgoing.method() === "GET") {
      reads++;
      const response = await route.fetch();
      const preferences = await response.json();
      await route.fulfill({ response, json: { ...preferences, auto_update_remote_hosts: enabled } });
    } else if (outgoing.method() === "PUT" && "auto_update_remote_hosts" in outgoing.postDataJSON()) {
      const patch = outgoing.postDataJSON();
      expect(Object.keys(patch)).toEqual(["auto_update_remote_hosts"]);
      expect(typeof patch.auto_update_remote_hosts).toBe("boolean");
      writes.push(patch.auto_update_remote_hosts);
      const fail = refuse;
      const gate = nextGate;
      nextGate = undefined;
      await gate?.wait();
      if (fail) {
        await route.fulfill({ status: 503, body: "fixture preference save refused" });
      } else {
        enabled = patch.auto_update_remote_hosts;
        await route.fulfill({ status: 204 });
      }
    } else await route.fallback();
  })]);
  return {
    writes,
    reads: () => reads,
    setRefused(value: boolean) { refuse = value; },
    /** Hold only the next save, retaining its refusal choice at admission. */
    holdNext() {
      const gate = routeGate();
      nextGate = gate;
      releases.push(gate.release);
      return gate;
    },
  };
}

/** The unchanged Hosts arrow names all available updates, even dismissed ones. */
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
    const token = getComputedStyle(document.documentElement).getPropertyValue(
      kind === "required" ? "--danger-strong" : "--warn",
    ).trim();
    const probe = document.createElement("span");
    probe.style.color = token;
    document.body.append(probe);
    const expected = getComputedStyle(probe).color;
    probe.remove();
    return { actual: getComputedStyle(node).color, expected, token };
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

/** Hidden scrolled rows need not be hit-testable; the pinned header actions do. */
async function expectAttentionFits(page: Page): Promise<void> {
  const list = page.locator(".host-attention-list");
  await expect(list).toBeVisible();
  const sidebar = (await page.locator(".app-sidebar").boundingBox())!;
  const bounds = (await list.boundingBox())!;
  expect(bounds.x).toBeGreaterThanOrEqual(sidebar.x);
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(sidebar.x + sidebar.width);
  expect(await list.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
  for (const button of await list.getByRole("button").all()) {
    const inBody = await button.evaluate((node) => !!node.closest(".host-attention-body"));
    if (inBody) continue;
    await expect(button).toBeVisible();
    expect(
      await button.evaluate((node) => {
        const box = node.getBoundingClientRect();
        return node.contains(document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2));
      }),
      "each fixed attention action must remain hit-testable",
    ).toBe(true);
  }
  const placement = await list.evaluate((node) => ({
    inSessionScroller: !!node.closest(".sidebar-content"),
    bottom: node.getBoundingClientRect().bottom,
    heading: document.querySelector(".list-header")!.getBoundingClientRect().top,
  }));
  expect(placement.inSessionScroller, "attention stays outside the session-content scroller").toBe(false);
  expect(placement.bottom).toBeLessThanOrEqual(placement.heading + 1);
}

/** Real session controls must stay usable beside a bounded attention region. */
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
 * Freeze the measured local session so progress cannot borrow a concurrent
 * sort, status or identity change as an explanation for closing its menu.
 */
async function prepareSessionMenu(page: Page, request: APIRequestContext) {
  const session = await createSession(request, { title: "attention-menu-anchor", invocation: "sleep 300" });
  cleanup.push(["remove anchor session", () => cleanupSession(request, session.id)]);
  const preferences = await readPreferences(request);
  cleanup.push(["restore density", () => patchPreferences(request, { compact: preferences.compact ?? null })]);
  await patchPreferences(request, { compact: true });
  await observeFeedReaders(page);
  const fixture = await prepare(page, request);
  const reply = await request.get("/api/sessions");
  expect(reply.ok(), await reply.text()).toBe(true);
  const listing = await reply.json();
  const owned = listing.sessions.find((candidate: { id: string; host: number }) => candidate.id === session.id);
  expect(owned, "the owned session must exist before its listing is frozen").toBeTruthy();
  expect(owned.host).toBe(fixture.localId);
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
  return {
    fixture,
    row,
    toggle,
    panel,
    /** Keyboard opening measures this row without taking a pointer order hold. */
    async open() {
      expect(await page.locator(".session-list").evaluate((node) => node.matches(":hover"))).toBe(false);
      await toggle.focus();
      await expect(toggle).toBeFocused();
      await toggle.press("ArrowDown");
      await expect(row.locator(".session-row-rename")).toBeFocused();
      await expect(row.locator(".session-row-menu-flyout")).toHaveCSS("opacity", "1");
      await expect(toggle).toHaveAttribute("aria-expanded", "true");
    },
    /** Capture scroll, order and outer geometry beside the actual anchor coordinate. */
    geometry: () =>
      row.evaluate((node) => {
        const outer = document.querySelector(".app-sidebar")!.getBoundingClientRect();
        const box = node.getBoundingClientRect();
        return {
          y: box.y,
          x: box.x,
          width: box.width,
          outerWidth: outer.width,
          outerHeight: outer.height,
          scroll: document.querySelector(".sidebar-content")!.scrollTop,
          listScroll: document.querySelector(".session-list")!.scrollTop,
          index: [...document.querySelectorAll(".session-row")].indexOf(node),
        };
      }),
  };
}

/** Distinct classifications must coexist; opening one never changes the host filter. */
test("attention groups problems, work and updates and opens the exact host", async ({ page, request }) => {
  const fixture = await prepare(page, request);
  const list = page.locator(".host-attention-list");
  const rows = list.locator(".host-attention-row");
  await expect(rows).toHaveCount(4);
  expect(await rows.evaluateAll((nodes) => nodes.map((node) => node.getAttribute("data-attention-host-id"))))
    .toEqual([IDS.unreachable, IDS.busy, IDS.one, IDS.two].map(String));
  await expect(list).toContainText("4 hosts need attention");
  await expect(attentionRow(page, IDS.unreachable)).toHaveAttribute("data-attention-kind", "problem");
  await expect(attentionRow(page, IDS.unreachable)).toContainText("Unreachable, retrying");
  await expect(attentionRow(page, IDS.unreachable).getByRole("button", { name: "Details", exact: true })).toBeVisible();
  await expect(attentionRow(page, IDS.busy)).toHaveAttribute("data-attention-kind", "working");
  await expect(attentionRow(page, IDS.busy)).toContainText("Setting up");
  await expect(attentionRow(page, IDS.busy).getByRole("button", { name: /^(Details|Update)$/ })).toHaveCount(0);
  await expect(attentionRow(page, IDS.busy).getByRole("progressbar")).toHaveAttribute("aria-valuemax", "3");
  await expect(attentionRow(page, IDS.one)).toHaveAttribute("data-attention-kind", "update");
  await expect(attentionRow(page, IDS.one)).toHaveAttribute("data-update-kind", "optional");
  for (const id of [fixture.localId, IDS.current, IDS.newer]) await expect(attentionRow(page, id)).toHaveCount(0);
  await expect(page.locator(".host-update-banner, .host-update-card")).toHaveCount(0);
  await expectBadge(page, 2, "optional");
  await expect(page.locator(".host-rail-all")).toHaveAttribute("aria-pressed", "true");
  await attentionRow(page, IDS.unreachable).getByRole("button", { name: "Details", exact: true }).click();
  await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "true");
  await expect(list).toHaveCount(0);
  const target = page.locator(`[data-host-id="${IDS.unreachable}"] .host-row-menu`);
  await expect(target).toBeFocused();
  await expect(target).toBeInViewport();
  await expect(page.locator(".host-rail-all")).toHaveAttribute("aria-pressed", "true");
  await showSessionsPanel(page);
  await attentionRow(page, IDS.two).getByText("build-two", { exact: true }).click();
  await expect(page.locator(`[data-host-id="${IDS.two}"] .host-row-menu`)).toBeFocused();
  expect(fixture.updates).toEqual([]);
});

/**
 * The two surfaces must expose the same live run rather than stale copied
 * diagnostics. Separate hidden sources keep row selectors unambiguous while
 * pointer and keyboard entry both reach the shared tooltip owner.
 */
test("host progress cards agree across surfaces and follow connection detail", async ({ page, request }) => {
  const fixture = await prepare(page, request);
  const attention = attentionRow(page, IDS.busy);
  const open = attention.locator(".host-attention-open");
  await expect(attention).toHaveAttribute("data-attention-kind", "working");
  await expect(attention.locator(".host-attention-state")).toHaveText("Setting up · setup step 1");
  await expect(attention.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "0");
  await expect(open).not.toHaveAttribute("data-tooltip", /\S/);
  await expect(attention.locator(".host-kind-icon")).not.toHaveAttribute("data-tooltip", /\S/);
  await expect(attention.getByRole("progressbar")).not.toHaveAttribute("data-tooltip", /\S/);
  await expect(attention.locator(".host-progress-card")).toHaveCount(0);
  const attentionSource = await open.getAttribute("data-tooltip-card");
  expect(attentionSource, "a running attention identity needs its own card source").toBeTruthy();
  await expect(page.locator(`[id="${attentionSource}"]`)).toBeHidden();
  const tip = page.locator(".farhelm-tooltip");
  const card = tip.locator(".host-progress-card");
  await attention.getByRole("progressbar").hover();
  await expect(card).toBeVisible();
  await expect(card.locator(".host-progress-title")).toHaveText("busy");
  await expect(card).toContainText("Setting up: 0 of 3 steps done");
  await expect(card).toContainText("Prepared once a day, or after Farhelm updates.");
  await expect(card.locator(".host-progress-connection")).toHaveCount(0);
  const steps = await card.locator(".provisioning-step").evaluateAll((nodes) => nodes.map((node) => ({
    id: node.getAttribute("data-step"), status: node.getAttribute("data-status"), text: node.textContent,
  })));
  expect(steps).toHaveLength(3);
  await expect(tip).toHaveAttribute("data-side", "right");
  await expect(open).toHaveClass(/tooltip-target-ring/);
  const placement = await tip.boundingBox();
  await open.hover();
  await expect(card).toBeVisible();
  expect(await tip.boundingBox()).toEqual(placement);
  await attention.getByRole("progressbar").hover();
  await expect(card).toBeVisible();
  expect(await tip.boundingBox()).toEqual(placement);
  await expect(open).toHaveClass(/tooltip-target-ring/);

  // The run is unchanged: only the freshly read host phase supplies this line.
  fixture.setAge(IDS.busy, "connecting");
  await fixture.publish();
  const connection = card.locator(".host-progress-connection");
  await expect(connection).toContainText("Attempt 2: waiting for the next connection attempt");
  await expect(connection).toContainText("the last one failed: fixture SSH connection refused");
  const diagnostic = await connection.textContent();
  await showHostsPanel(page);
  const row = page.locator(`[data-host-id="${IDS.busy}"]`);
  const label = row.locator(".host-update-running");
  await expect(label).toBeVisible();
  const hostsSource = await label.getAttribute("data-tooltip-card");
  expect(hostsSource).toBeTruthy();
  expect(hostsSource, "the always-mounted Hosts source must not alias the attention source").not.toBe(attentionSource);
  await expect(row.locator(".host-progress-card")).toHaveCount(0);
  await expect(page.locator(`[id="${hostsSource}"]`)).toBeHidden();
  await label.hover();
  await expect(card).toBeVisible();
  await expect(connection).toHaveText(diagnostic!);
  expect(await card.locator(".provisioning-step").evaluateAll((nodes) => nodes.map((node) => ({
    id: node.getAttribute("data-step"), status: node.getAttribute("data-status"), text: node.textContent,
  })))).toEqual(steps);
  fixture.setAge(IDS.busy, "current");
  await fixture.publish();
  await expect(connection).toHaveCount(0);

  await showSessionsPanel(page);
  await expect(open).toBeVisible();
  const viewport = page.viewportSize()!;
  await page.mouse.move(viewport.width - 8, viewport.height - 8);
  await page.keyboard.press("Tab");
  await open.focus();
  await expect(open).toBeFocused();
  expect(await open.evaluate((node) => node.matches(":focus-visible")), "keyboard provenance must own this opening").toBe(true);
  await expect(card).toBeVisible();
  await expect(card).toContainText("Prepared once a day, or after Farhelm updates.");
  await expect(connection).toHaveCount(0);
  expect(fixture.updates).toEqual([]);
});

/**
 * A connection failure can exceed an entire short window. Preview only that
 * overflow, keep the actual current step readable, and make the promised full
 * diagnosis reachable through the ordinary identity button. Re-cloning must
 * restore whole text when either the window grows or the diagnosis gets shorter.
 * A run without its first steps still needs the same diagnostic preview; only
 * the populated variant has a current step whose glyphs must remain readable.
 */
for (const stepCount of [3, 0]) {
test(stepCount === 0 ? "host progress previews long diagnostics before any steps arrive"
  : "host progress previews only diagnostics that cannot fit beside its current step", async ({ page, request }) => {
  const fixture = await prepare(page, request);
  const diagnosis = "SSH failed while reconnecting. ".repeat(18) + "Final diagnosis: permission denied.";
  fixture.setStepCount(IDS.busy, stepCount);
  fixture.setConnectionError(IDS.busy, diagnosis);
  await fixture.publish();
  const open = attentionRow(page, IDS.busy).locator(".host-attention-open");
  await expect(open).toBeVisible();
  await expect(attentionRow(page, IDS.busy)).toHaveAttribute("data-attention-kind", "working");
  const card = page.locator(".farhelm-tooltip .host-progress-card");
  const detail = card.locator(".host-progress-connection-detail");
  const notice = card.locator(".host-progress-preview-notice");

  /** Reveal scrolling dismisses help, even when its event arrives after hover.
   * Keep reveal separate from pointer entry and cross the browser's scroll-event
   * phase before entering. This is a viewport-opening boundary only: live
   * diagnosis changes below must update the already-open card without re-entry.
   */
  const hoverAtHeight = async (height: number) => {
    await page.mouse.move(0, 0);
    await page.setViewportSize({ width: 1024, height });
    await page.evaluate(() => document.fonts.ready);
    await open.scrollIntoViewIfNeeded();
    await expect(open, "the working identity must fit its actual scroller and viewport before entry")
      .toBeInViewport({ ratio: 1 });
    // Scroll steps precede animation-frame callbacks. Flush the reveal's queued
    // scroll before hover, rather than cancelling the tooltip's pending timer.
    await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => resolve())));
    await expect.poll(() => open.evaluate(node => {
      const rect = node.getBoundingClientRect();
      return node.contains(document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2));
    }), { message: "the revealed identity owns the hover hit point" }).toBe(true);
    await open.hover();
    await expect(card).toBeVisible();
  };

  await hoverAtHeight(300);
  await expect(card.locator(".provisioning-step")).toHaveCount(stepCount);
  await expect(card.locator(".provisioning-step[data-current]")).toHaveCount(stepCount === 0 ? 0 : 1);
  await expect(detail).toContainText(diagnosis);
  await expect(notice).toBeVisible();
  // DOM visibility accepts partially clipped text. Both the current name and
  // the notice must keep every glyph inside their clipping boxes and viewport.
  const readablePreview = () => card.evaluate((node, stepCount) => {
    if (!node.isConnected) return { current: false, notice: false };
    const readable = (text: Element, clip: Element = node) => {
      const range = document.createRange();
      range.selectNodeContents(text);
      const rects = Array.from(range.getClientRects());
      const clips = [node.getBoundingClientRect(), clip.getBoundingClientRect(), new DOMRect(0, 0, innerWidth, innerHeight)];
      return rects.length > 0 && rects.every((rect) => clips.every((bounds) =>
        rect.top >= bounds.top - 0.5 && rect.bottom <= bounds.bottom + 0.5
        && rect.left >= bounds.left - 0.5 && rect.right <= bounds.right + 0.5));
    };
    return {
      current: stepCount === 0 ? null
        : readable(node.querySelector(".host-progress-step-current")!, node.querySelector(".host-progress-steps")!),
      notice: readable(node.querySelector(".host-progress-preview-notice")!),
    };
  }, stepCount);
  const expectedPreview = { current: stepCount === 0 ? null : true, notice: true };
  await expect.poll(readablePreview, { message: "a long diagnosis must leave its current step and full-details notice readable" })
    .toEqual(expectedPreview);
  await expect.poll(() => detail.evaluate((node) => node.isConnected && node.scrollHeight > node.clientHeight)).toBe(true);
  // The elapsed clock replaces clones. Sample visibility and style on the same
  // connected copy instead of accepting a detached element's empty style.
  await expect.poll(() => card.evaluate(node =>
    node.isConnected && node.getClientRects().length > 0 ? getComputedStyle(node).pointerEvents : null
  )).toBe("none");

  await hoverAtHeight(1200);
  await expect(notice).toBeHidden();
  await expect.poll(() => detail.evaluate((node) => node.isConnected && node.scrollHeight <= node.clientHeight + 1)).toBe(true);
  await expect(detail).toContainText(diagnosis);
  await hoverAtHeight(300);
  await expect(notice).toBeVisible();
  fixture.setConnectionError(IDS.busy, "Short connection failure.");
  await fixture.publish();
  await expect(detail).toContainText("Short connection failure.");
  await expect(notice).toBeHidden();
  await expect.poll(() => detail.evaluate((node) => node.isConnected && node.scrollHeight <= node.clientHeight + 1)).toBe(true);

  fixture.setConnectionError(IDS.busy, diagnosis);
  await fixture.publish();
  await expect(notice).toBeVisible();
  await expect.poll(readablePreview).toEqual(expectedPreview);
  await open.click();
  await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "true");
  await page.locator(".host-details-toggle").check();
  const fullDetail = page.locator(`[data-host-id="${IDS.busy}"] .host-detail`);
  await expect(fullDetail).toContainText(diagnosis);
  // The full row can scroll normally; its text itself must have no preview cap.
  expect(await fullDetail.evaluate((node) => getComputedStyle(node).maxHeight)).toBe("none");
  expect(fixture.updates).toEqual([]);
});
}

/**
 * A fitting one-step card has no spare list height to absorb measurement
 * rounding. Its multiline diagnosis must remain whole rather than losing a
 * notice-height of text because clientHeight rounded down a fractional line box.
 */
test("a one-step host card keeps a fitting multiline diagnosis whole", async ({ page, request }) => {
  const fixture = await prepare(page, request);
  const diagnosis = "Connection refused by the remote supervisor. ".repeat(2).trim();
  fixture.setStepCount(IDS.busy, 1);
  fixture.setConnectionError(IDS.busy, diagnosis);
  await fixture.publish();
  const open = attentionRow(page, IDS.busy).locator(".host-attention-open");
  await expect(open).toBeVisible();
  await open.hover();
  const card = page.locator(".farhelm-tooltip .host-progress-card");
  await expect(card).toBeVisible();
  await expect(card.locator(".provisioning-step")).toHaveCount(1);
  await expect(card.locator(".provisioning-step[data-current]")).toHaveCount(1);
  const detail = card.locator(".host-progress-connection-detail");
  await expect(detail).toContainText(diagnosis);
  await expect.poll(() => card.evaluate((node) => {
    if (!node.isConnected) return null;
    const detail = node.querySelector(".host-progress-connection-detail") as HTMLElement;
    const style = getComputedStyle(detail);
    return {
      belowCap: node.getBoundingClientRect().height < innerHeight - 20,
      multiline: detail.getBoundingClientRect().height > parseFloat(style.lineHeight) * 2,
      whole: detail.scrollHeight <= detail.clientHeight + 1,
      noticeHidden: (node.querySelector(".host-progress-preview-notice") as HTMLElement).hidden,
    };
  })).toEqual({ belowCap: true, multiline: true, whole: true, noticeHidden: true });
});

/**
 * Scrolling a whole expanded host with nearest can leave its focused header
 * above the viewport when the row spans both edges. Preserve that exact
 * geometry across the Sessions handoff, rather than testing a short row that
 * either scrolling target would reveal correctly.
 */
test("attention opens the menu header of an expanded host spanning the viewport", async ({ page, request }) => {
  await page.setViewportSize({ width: 1024, height: 420 });
  const fixture = await prepare(page, request);
  const selected = page.locator('.session-row[data-session-selected="true"]');
  await expect(selected).toHaveCount(1);
  const selectedId = await selected.getAttribute("data-session-id");
  const reply = await request.get("/api/sessions");
  expect(reply.ok(), await reply.text()).toBe(true);
  const listing = await reply.json();
  const template = listing.sessions.find((session: { id: string }) => session.id === selectedId);
  expect(template, "the selected session must be real before adding display-only filler").toBeTruthy();
  expect(selectedId, "filler identities need the real host-entry prefix").toContain(":");
  const prefix = selectedId!.slice(0, selectedId!.indexOf(":"));
  // Both modes use the same content scroller. Without enough ordinary session
  // rows, concealing management would clamp its offset to zero and erase the
  // regression premise. These valid count-only rows are never opened or acted on.
  listing.sessions.push(...Array.from({ length: 48 }, (_, index) => ({
    ...template,
    id: `${prefix}:${randomUUID()}`,
    title: `attention-scroll-filler-${index}`,
    created_at: 0,
    last_activity_at: 0,
    seen_activity_at: 0,
    pinned: false,
    notifications: [],
    notifications_read_through: 0,
  })));
  listing.total = listing.sessions.length;
  listing.matching = listing.sessions.length;
  cleanup.push([
    "tall-row scroll budget",
    await drainedRoute(page, (url) => url.pathname === "/api/sessions", async (route) => {
      await route.fulfill({ headers: { "x-farhelm-build": fixture.originalBuild }, json: listing });
    }),
  ]);
  cleanup.push([
    "expanded host progress",
    await drainedRoute(page, (url) => url.pathname === `/api/hosts/${IDS.busy}/provisioning`, async (route) => {
      expect(route.request().method()).toBe("GET");
      await route.fulfill({
        headers: { "x-farhelm-build": fixture.originalBuild },
        json: {
          host_id: IDS.busy,
          run_id: `busy-${IDS.busy}`,
          operation: "add",
          status: "running",
          steps: Array.from({ length: 40 }, (_, index) => ({
            step: `tall-step-${index}`,
            name: `host setup diagnostic step ${index + 1}`,
            status: index === 0 ? "running" : "pending",
          })),
          message: null,
        },
      });
    }),
  ]);
  await fixture.publish();
  await expect(page.locator(".session-row")).toHaveCount(listing.sessions.length);
  await expect(page.locator(`[data-session-id="${selectedId}"]`)).toHaveAttribute("data-session-selected", "true");
  await showHostsPanel(page);
  await page.locator(".host-details-toggle").check();
  const row = page.locator(`[data-host-id="${IDS.busy}"]`);
  const steps = row.locator(".provisioning-step");
  await expect(steps).toHaveCount(40);
  await steps.nth(19).scrollIntoViewIfNeeded();
  const before = await row.evaluate((node) => {
    const viewport = document.querySelector(".sidebar-content")!;
    const port = viewport.getBoundingClientRect();
    const bounds = node.getBoundingClientRect();
    const menu = node.querySelector(".host-row-main .host-row-menu")!.getBoundingClientRect();
    return {
      tall: bounds.height > port.height,
      above: bounds.top < port.top,
      below: bounds.bottom > port.bottom,
      menuAbove: menu.bottom < port.top,
      scroll: viewport.scrollTop,
    };
  });
  expect(before, "the expanded host must straddle the viewport with its menu offscreen").toMatchObject({
    tall: true,
    above: true,
    below: true,
    menuAbove: true,
  });
  expect(before.scroll).toBeGreaterThan(0);
  await showSessionsPanel(page);
  await expect.poll(() => page.locator(".sidebar-content").evaluate((node) => node.scrollTop), {
    message: "the ordinary session list must preserve the management scroll offset",
  }).toBe(before.scroll);
  const witness = await page.evaluateHandle((host) => {
    const panel = document.querySelector<HTMLElement>(".hosts-panel")!;
    const observed = { seen: false, tall: false, above: false, below: false, menuAbove: false };
    // Mutation delivery precedes the next animation frame. This read-only
    // witness captures the revealed layout before the handoff's focus/scroll,
    // so a mode switch that accidentally resets scroll cannot make the test pass.
    const observer = new MutationObserver(() => {
      if (panel.dataset.panelVisible !== "true") return;
      const target = panel.querySelector(`[data-host-id="${host}"]`)!;
      const bounds = target.getBoundingClientRect();
      const port = document.querySelector(".sidebar-content")!.getBoundingClientRect();
      const menu = target.querySelector(".host-row-main .host-row-menu")!.getBoundingClientRect();
      Object.assign(observed, {
        seen: true,
        tall: bounds.height > port.height,
        above: bounds.top < port.top,
        below: bounds.bottom > port.bottom,
        menuAbove: menu.bottom < port.top,
      });
      observer.disconnect();
    });
    observer.observe(panel, { attributes: true, attributeFilter: ["data-panel-visible"] });
    return { observed, disconnect: () => observer.disconnect() };
  }, IDS.busy);
  try {
    await attentionRow(page, IDS.busy).locator(".host-attention-name").click();
    await expect.poll(() => witness.evaluate(({ observed }) => observed), {
      message: "the visible-transition witness must prove the pre-scroll straddling premise",
    }).toEqual({ seen: true, tall: true, above: true, below: true, menuAbove: true });
    const menu = row.locator(".host-row-main .host-row-menu");
    await expect(menu).toBeFocused();
    await expect(menu).toBeInViewport();
    await expect(page.locator(`[data-session-id="${selectedId}"]`)).toHaveAttribute("data-session-selected", "true");
  } finally {
    await witness.evaluate(({ disconnect }) => disconnect());
    await witness.dispose();
  }
  expect(fixture.updates).toEqual([]);
});

/** A batch includes every listed Update, even when required and optional coexist. */
test("Update all includes required and optional rows but no other attention", async ({ page, request }) => {
  const fixture = await prepare(page, request);
  fixture.setAge(IDS.one, "required");
  fixture.setAge(IDS.busy, "unreachable");
  await fixture.publish();
  const list = page.locator(".host-attention-list");
  await expectBadge(page, 2, "required");
  await expect(attentionRow(page, IDS.one)).toHaveAttribute("data-update-kind", "required");
  await expect(attentionRow(page, IDS.one)).toContainText(/sessions.*unreachable.*updated/i);
  await expect(attentionRow(page, IDS.two)).toHaveAttribute("data-update-kind", "optional");
  await expect(attentionRow(page, IDS.busy)).toHaveAttribute("data-attention-kind", "working");
  await list.getByRole("button", { name: "Update all", exact: true }).click();
  await expect.poll(() => fixture.updates.map((sent) => sent.id).sort()).toEqual([IDS.one, IDS.two]);
  expect(fixture.updates).toHaveLength(2);
  expect(fixture.updates.every((sent) => sent.method === "POST" && sent.body === null)).toBe(true);
  for (const id of [IDS.one, IDS.two]) {
    await expect(page.locator(`[data-host-id="${id}"] .host-update-pending`)).toBeAttached();
  }
});

/** A single Update remains directly actionable rather than offering an oversized batch. */
test("one listed update uses its own Update action", async ({ page, request }) => {
  const fixture = await prepare(page, request);
  fixture.setAge(IDS.two, "current");
  await fixture.publish();
  const list = page.locator(".host-attention-list");
  await expect(list.getByRole("button", { name: "Update all", exact: true })).toHaveCount(0);
  await attentionRow(page, IDS.one).getByRole("button", { name: "Update", exact: true }).click();
  await expect.poll(() => fixture.updates).toEqual([{ id: IDS.one, method: "POST", body: null }]);
});

for (const count of [1, 2]) {
  /** Opt-in must use the displayed Update targets, excluding busy/current/newer hosts. */
  test(`Always update requests exactly ${count} displayed hosts and enables the shared setting`, async ({ page, request }) => {
    const preference = await automaticUpdatePreference(page);
    const fixture = await prepare(page, request);
    fixture.setAge(IDS.one, "required");
    if (count === 1) fixture.setAge(IDS.two, "current");
    await fixture.publish();
    const list = page.locator(".host-attention-list");
    await expect(list.locator('[data-attention-kind="update"]')).toHaveCount(count);
    await expect(list.getByRole("button", { name: "Update all", exact: true })).toHaveCount(count === 1 ? 0 : 1);
    const action = list.getByRole("button", { name: "Always update", exact: true });
    await expect(action).toBeVisible();
    await expect(action).toHaveAttribute("data-tooltip", /listed hosts now.*later updates.*Settings/);
    await action.click();
    const wanted = count === 1 ? [IDS.one] : [IDS.one, IDS.two];
    await expect.poll(() => fixture.updates.map((sent) => sent.id).sort()).toEqual(wanted);
    await expect.poll(() => preference.writes).toEqual([true]);
    expect(fixture.updates.every((sent) => sent.method === "POST" && sent.body === null)).toBe(true);
    await expect(action).toHaveCount(0);
    const dialog = await openSettings(page, "hosts");
    await expect(dialog.getByRole("checkbox", { name: "Update remote hosts automatically", exact: true })).toBeChecked();
    expect(fixture.updates).toHaveLength(count);
  });
}

/** An existing opt-in needs no second action; ordinary manual updates stay available. */
test("Always update is absent when automatic updates are already enabled", async ({ page, request }) => {
  const preference = await automaticUpdatePreference(page, true);
  await prepare(page, request);
  const list = page.locator(".host-attention-list");
  await expect(list.locator('[data-attention-kind="update"]')).toHaveCount(2);
  await expect(list.getByRole("button", { name: "Update all", exact: true })).toBeVisible();
  await expect(list.getByRole("button", { name: "Always update", exact: true })).toHaveCount(0);
  expect(preference.writes).toEqual([]);
});

for (const recovery of ["Try again", "Settings"] as const) {
  /** Hiding attention cannot lose a failed save; either deliberate save supersedes it without repeating host updates. */
  test(`Always update retains a hidden save failure and recovers through ${recovery}`, async ({ page, request }) => {
    const preference = await automaticUpdatePreference(page);
    preference.setRefused(true);
    const save = preference.holdNext();
    const fixture = await prepare(page, request);
    const list = page.locator(".host-attention-list");
    await expect(list.locator('[data-attention-kind="update"]')).toHaveCount(2);
    await list.getByRole("button", { name: "Always update", exact: true }).click();
    await expect.poll(() => preference.writes).toEqual([true]);
    await expect.poll(() => fixture.updates.map((sent) => sent.id).sort()).toEqual([IDS.one, IDS.two]);
    await showHostsPanel(page);
    await expect(list).toHaveCount(0);
    const reads = preference.reads();
    save.release();
    await expect.poll(preference.reads, { message: "failed save must trigger its corrective preference read" })
      .toBeGreaterThan(reads);
    await showSessionsPanel(page);
    const alert = list.getByRole("alert");
    await expect(alert).toContainText("Could not turn on automatic updates. Later updates will still wait for you.");
    await expect(alert.getByRole("button", { name: "Try again", exact: true })).toBeVisible();
    preference.setRefused(false);
    if (recovery === "Try again") {
      await alert.getByRole("button", { name: "Try again", exact: true }).click();
      await expect(alert).toHaveCount(0);
      const dialog = await openSettings(page, "hosts");
      await expect(dialog.getByRole("checkbox", { name: "Update remote hosts automatically", exact: true })).toBeChecked();
    } else {
      const dialog = await openSettings(page, "hosts");
      const checkbox = dialog.getByRole("checkbox", { name: "Update remote hosts automatically", exact: true });
      await expect(checkbox).not.toBeChecked();
      await checkbox.click();
      await expect(checkbox).toBeChecked();
      await dialog.getByRole("button", { name: "Close", exact: true }).click();
      await expect(alert).toHaveCount(0);
    }
    await expect.poll(() => preference.writes).toEqual([true, true]);
    expect(fixture.updates).toHaveLength(2);
  });
}

/** A late save warning moves session anchors even when every host row is unchanged. */
test("Always update save failure retires a session menu measured while the save was pending", async ({ page, request }) => {
  const preference = await automaticUpdatePreference(page);
  preference.setRefused(true);
  const save = preference.holdNext();
  const { fixture, open, panel, toggle } = await prepareSessionMenu(page, request);
  await page.locator(".host-attention-list").getByRole("button", { name: "Always update", exact: true }).click();
  await expect.poll(() => preference.writes).toEqual([true]);
  await expect.poll(() => fixture.updates.map((sent) => sent.id).sort()).toEqual([IDS.one, IDS.two]);
  for (const id of [IDS.one, IDS.two]) {
    await expect(attentionRow(page, id)).toHaveAttribute("data-attention-kind", "working");
  }
  await open();
  await expect(panel).toBeVisible();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  save.release();
  await expect(page.locator(".host-attention-list").getByRole("alert")).toBeVisible();
  await expect(toggle).toHaveAttribute("aria-expanded", "false");
  await expect(panel).not.toBeVisible();
});

/** Dismissal remembers update identities, while required, working and problem rows persist. */
test("optional dismissal leaves other attention visible and revives only for new updates", async ({ page, request }) => {
  const fixture = await prepare(page, request);
  const list = page.locator(".host-attention-list");
  const dismiss = () => list.getByRole("button", { name: "Dismiss optional host updates", exact: true }).click();
  await dismiss();
  await expect(list.locator('[data-attention-kind="update"]')).toHaveCount(0);
  await expect(attentionRow(page, IDS.unreachable)).toBeVisible();
  await expect(attentionRow(page, IDS.busy)).toBeVisible();
  await expectBadge(page, 2, "optional");
  fixture.setAge(IDS.one, "required");
  await fixture.publish();
  await expect(attentionRow(page, IDS.one)).toHaveAttribute("data-update-kind", "required");
  await expect(attentionRow(page, IDS.two)).toHaveCount(0);
  fixture.setAge(IDS.one, "optional");
  fixture.setAge(IDS.unreachable, "current");
  fixture.setAge(IDS.busy, "current");
  fixture.setBusy(IDS.busy, false);
  await fixture.publish();
  await expect(list).toHaveCount(0);
  fixture.setAge(IDS.two, "current");
  await fixture.publish();
  await expectBadge(page, 1, "optional");
  fixture.setAge(IDS.two, "optional");
  await fixture.publish();
  await expect(list).toHaveCount(0);
  fixture.setAge(IDS.unreachable, "optional");
  await fixture.publish();
  await expect(list.locator('[data-attention-kind="update"]')).toHaveCount(3);
  await dismiss();
  await expect(list).toHaveCount(0);
  await page.reload();
  await expect(list.locator('[data-attention-kind="update"]')).toHaveCount(3);
  await dismiss();
  await expect(list).toHaveCount(0);
  fixture.setBuild(`${fixture.originalBuild}-attention-fixture`);
  await fixture.publish();
  await expect(page.locator(".build-skew")).toBeVisible();
  await expect(list.locator('[data-attention-kind="update"]')).toHaveCount(3);
  await fixture.observeBuildThroughFilter(fixture.localId);
  await dismiss();
  await expect(list).toHaveCount(0);
  fixture.setBuild(`${fixture.originalBuild}-another-attention-fixture`);
  await fixture.observeBuildThroughFilter();
  await expect(list.locator('[data-attention-kind="update"]')).toHaveCount(3);
  await expect(page.locator(".build-skew")).toContainText(`${fixture.originalBuild}-attention-fixture`);
  expect(fixture.updates).toEqual([]);
});

/** Step text and counters repaint inside the row without invalidating a measured session menu. */
test("progress-only publications preserve an open session menu", async ({ page, request }) => {
  const { fixture, open, geometry, panel, toggle } = await prepareSessionMenu(page, request);
  const progress = attentionRow(page, IDS.busy).getByRole("progressbar");
  await expect(progress).toHaveAttribute("aria-valuenow", "0");
  await expect(progress).toHaveAttribute("aria-valuemax", "3");
  await expect(attentionRow(page, IDS.busy).locator(".host-attention-state")).toHaveText("Setting up · setup step 1");
  await open();
  const before = await geometry();
  fixture.setCompleted(IDS.busy, 1);
  await fixture.publish();
  await expect(progress).toHaveAttribute("aria-valuenow", "1");
  await expect(attentionRow(page, IDS.busy).locator(".host-attention-state")).toHaveText("Setting up · setup step 2");
  await waitForFeedReadersSettled(page);
  expect(await geometry(), "a progress-only update must leave the anchor unchanged").toEqual(before);
  await expect(panel).toBeVisible();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  await page.keyboard.press("Escape");
  await expect(panel).toHaveCount(0);
});

/** Real insertion/removal retires old openings, without borrowing a sort or scroll change. */
test("attention insertion and removal retire existing compact session menus", async ({ page, request }) => {
  const { fixture, open, geometry, panel, toggle } = await prepareSessionMenu(page, request);
  fixture.setAge(IDS.two, "current");
  fixture.setAge(IDS.unreachable, "current");
  fixture.setAge(IDS.busy, "current");
  fixture.setBusy(IDS.busy, false);
  await fixture.publish();
  await expect(page.locator(".host-attention-row")).toHaveCount(1);
  await waitForFeedReadersSettled(page);
  for (const age of ["current", "optional"] as const) {
    await open();
    const before = await geometry();
    expect(before.scroll).toBe(0);
    expect(before.listScroll).toBe(0);
    fixture.setAge(IDS.one, age);
    await fixture.publish();
    await waitForFeedReadersSettled(page);
    await expect(page.locator(".host-attention-list")).toHaveCount(age === "current" ? 0 : 1);
    const after = await geometry();
    expect(after.y, "the attention region must actually move this row").not.toBe(before.y);
    expect({ ...after, y: before.y }, "no competing scroll, order or outer resize may explain dismissal").toEqual(
      before,
    );
    await expect(panel).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
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

/** A bounded host scroller must preserve ordinary session access in both palettes. */
test("many attention rows scroll above usable sessions in a short narrow sidebar", async ({ page, request }, info) => {
  const fixture = await prepare(page, request);
  const ids = [IDS.one, IDS.two, IDS.unreachable, IDS.newer, ...Array.from({ length: 8 }, (_, i) => 9417 + i)];
  const names = ids.map((id) => (`host-${id}-` + "long-alias-".repeat(6)).slice(0, 64));
  expect(names.every((name) => name.length === 64), "aliases exercise their supported maximum").toBe(true);
  fixture.setAge(IDS.busy, "current");
  fixture.setBusy(IDS.busy, false);
  for (const [index, id] of ids.entries()) {
    fixture.setName(id, names[index]);
    fixture.setAge(id, "optional");
  }
  await fixture.publish();
  await expect(page.locator(".host-attention-row")).toHaveCount(ids.length);
  await page.setViewportSize({ width: 1024, height: 420 });
  await narrowSidebar(page);
  const list = page.locator(".host-attention-list");
  const body = list.locator(".host-attention-body");
  for (const palette of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: palette });
    await expect(page.locator("html")).toHaveAttribute("data-appearance", palette);
    await expectAttentionFits(page);
    const fixed = (await list.boundingBox())!;
    await expectKeyboardScroll(
      page,
      list.getByRole("button", { name: "Dismiss optional host updates", exact: true }),
      body,
    );
    await page.keyboard.press("End");
    await expect.poll(() => body.evaluate((node) => node.scrollTop + node.clientHeight >= node.scrollHeight - 1)).toBe(
      true,
    );
    const last = (await attentionRow(page, ids.at(-1)!).boundingBox())!;
    const port = (await body.boundingBox())!;
    expect(last.y + last.height, "keyboard scrolling must expose the last host row").toBeLessThanOrEqual(
      port.y + port.height + 1,
    );
    await expectSessionAccess(page, request, fixture.localId);
    expect((await list.boundingBox())!.y, "session scrolling cannot carry attention out of view").toBe(fixed.y);
    await expectAttentionFits(page);
    await info.attach(`attention-short-sidebar-${palette}`, {
      body: await page.locator(".app-sidebar").screenshot(),
      contentType: "image/png",
    });
    // Restore the scroll premise before checking the next palette's Tab path.
    await body.focus();
    await page.keyboard.press("Home");
    await expect.poll(() => body.evaluate((node) => node.scrollTop)).toBe(0);
  }
  // The last host cannot fit in this short management panel without scrolling.
  // Opening from its attention identity must perform that handoff itself.
  const lastHost = ids.at(-1)!;
  await attentionRow(page, lastHost).locator(".host-attention-name").click();
  const target = page.locator(`[data-host-id="${lastHost}"] .host-row-menu`);
  await expect(target).toBeFocused();
  await expect(target).toBeInViewport();
  expect(
    await target.evaluate((node) => {
      for (let parent = node.parentElement; parent; parent = parent.parentElement) {
        if (parent.scrollTop > 0) return true;
      }
      return false;
    }),
    "opening the last host must scroll its management row into view",
  ).toBe(true);
  expect(fixture.updates).toEqual([]);
});
