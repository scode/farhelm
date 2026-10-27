import { type APIRequestContext, type Locator, type Page, type Route } from "@playwright/test";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { expect, test } from "./helpers/evidence";
import {
  cleanupSession, createSession, observeFeedReaders, openHostsPanel, patchPreferences,
  pinAutoSelect, readFeedReaders, resetPreferences, type SessionRow, stubFeed, waitForFeedReadersSettled,
} from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { routeGate } from "./helpers/route-gate";
import { waitForSessionReady } from "./helpers/terminal-readiness";

// These wire-renderer fixtures exercise transport, actual markup and menus;
// they do not prove an SSH installation ran. Positive geometry changes may
// also reach the aggregate panel closer. The production hook/capture unit
// tests distinguish causal delivery from that competing mechanism. Ordinary
// host lines stay under popover-host-movement.spec.ts's atomic snapshot tests.

// ===== Wire fixtures and owned cleanup =================================
type Key = "a" | "b" | "c";
const KEYS: Key[] = ["a", "b", "c"];
const FAKE_IDS = { a: 9303, b: 9301, c: 9302 };
const LONG_PATH = "/opt/layout-fixture/" + "retained-payload-directory/".repeat(7) + "farhelm";
const LONG_MESSAGE = `${LONG_PATH} already has the requested payload`;
const SHORT_MESSAGE = "/opt/layout-fixture/farhelm already has the requested payload";
const MANUAL_SHORT = "run farhelm supervisor run --state-dir /opt/layout-fixture/state";
const MANUAL_LONG = `run farhelm supervisor run --state-dir ${LONG_PATH}/state`;

/** Production progress fields used by the expanded renderer; names remain plan-owned. */
type Step = {
  step: string; name: string; paths: { full: string; short: string }[];
  explanation: string | null;
  status: "pending" | "running" | "completed" | "skipped";
  message: string | null;
};
/** A complete retained/idle view, including the server's host identity field. */
type Progress = {
  host_id: number; run_id: string | null; operation: "update" | null;
  status: "running" | "completed"; steps: Step[]; message: string | null;
};
type Reply = { status: 200; json: Progress };
/** Preserve the real local row's additive registry fields while replacing only named wire inputs. */
type WireHost = { id: number; kind: string; name: string; state: Record<string, unknown> } & Record<string, unknown>;
type Registry = { hosts: WireHost[] };
/** Observable request ownership, not a fixture-side application state setter. */
type Held<T> = {
  reply: T; seen: boolean; completed: boolean; error?: string;
  wait(): Promise<void>; release(): void;
};

// Cleanup is registered at allocation, before another await can fail. Gates
// release before route retirement; route.fetch responses remain owned until
// their handler has forwarded them and the browser no longer waits for them.
let gates: { release(): void }[] = [];
let sessions: string[] = [];
let hosts: number[] = [];
let observers: (() => Promise<unknown>)[] = [];
let diagnostics: (() => Promise<unknown>)[] = [];

test.beforeEach(() => {
  gates = []; sessions = []; hosts = []; observers = []; diagnostics = [];
});
test.afterEach(async ({ page, request }, info) => {
  await cleanupOwnedSteps(info.status !== info.expectedStatus, [
    ["bounded fixture evidence", async () => {
      if (info.status !== info.expectedStatus) {
        for (let i = 0; i < diagnostics.length; i++) {
          await info.attach(`expanded-host-boundary-${i}`, {
            body: JSON.stringify(await diagnostics[i]()), contentType: "application/json",
          });
        }
      }
    }],
    ["held routes", () => { for (const gate of gates) gate.release(); }],
    ["read-only panel observers", async () => { for (const stop of observers) await stop(); }],
    ["route retirement", () => page.unrouteAll({ behavior: "wait" })],
    ...hosts.splice(0).reverse().map((id): [string, () => Promise<void>] => [
      `owned host ${id}`, async () => {
        const response = await request.delete(`/api/hosts/${id}`);
        expect([200, 204, 404], await response.text()).toContain(response.status());
      },
    ]),
    ...sessions.splice(0).reverse().map((id): [string, () => Promise<void>] => [
      `owned session ${id}`, () => cleanupSession(request, id),
    ]),
    ["preferences", () => resetPreferences(request)],
  ]);
});

/** A named request boundary owns a gate even if setup fails before dispatch. */
function held<T>(reply: T): Held<T> {
  const gate = routeGate();
  gates.push(gate);
  return { reply, seen: false, completed: false, ...gate };
}

/** Server idle has a completed status and no retained run, rather than an invented idle enum. */
function idle(id: number): Reply {
  return { status: 200, json: {
    host_id: id, run_id: null, operation: null, status: "completed", steps: [],
    message: "no provisioning run has been recorded for this host",
  } };
}

/** Advance fixed plan labels through reachable running-step outcomes.
 * A message records a skipped upload; without one, advancement completes the
 * upload before installation starts. Neither transition renames a plan step.
 */
function run(id: number, name: string, phase: "before" | "advanced" | "completed", message?: string): Reply {
  const step = (label: string, words: string, status: Step["status"], text: string | null = null): Step => ({
    step: label, name: words, paths: [], explanation: null, status, message: text,
  });
  return { status: 200, json: {
    host_id: id, run_id: name, operation: "update", status: phase === "completed" ? "completed" : "running",
    steps: [
      step("create-directories", "create directories", "completed"),
      step("upload-farhelm", "upload the farhelm binary", message !== undefined ? "skipped" : phase === "before" ? "running" : "completed", message ?? null),
      step("install-farhelm", "install the farhelm binary", phase === "completed" ? "completed" : phase === "advanced" ? "running" : "pending"),
    ],
    message: null,
  } };
}

/** Connected paint is controlled input; explicit version flags keep offers stable. */
function remote(id: number, key: Key): WireHost {
  const name = `layout-expanded-${key}.invalid`;
  return {
    id, kind: "ssh", name, destination: name, alias: null,
    identity: null, remote_farhelm: null, remote_state_dir: null, incarnation: id,
    yolo_without_asking: false, commands_without_asking: false,
    links_without_asking: false, local_links_without_asking: false,
    state: { phase: "connected", identity: null, build_version: "layout-stable-build",
      old_version: false, newer_version: false, refresh: { status: "ok", sessions: 0 } },
  };
}

/** Fulfill only authenticated read surfaces; mutating settings replies are forwarded unchanged. */
async function progressReply(route: Route, build: string, reply: Reply) {
  await route.fulfill({ status: reply.status, headers: {
    "x-farhelm-build": build, "content-type": "application/json",
  }, json: reply.json });
}

/** Per-case inputs change only replies on real UI read paths. */
type Options = {
  ids?: Record<Key, number>;
  progress?: (ids: Record<Key, number>) => Partial<Record<Key, Reply>>;
  localManual?: string;
};

/** Freeze unrelated reads while keeping an owned real terminal and healthy feed.
 * Every body is captured at request entry. A phase change cannot rewrite a
 * previously dispatched read. All mounted hosts decode an initial stamped
 * progress response before a menu can open; first-Body timing has a focused
 * production-hook unit fixture rather than a cold-page read deadline gate.
 */
async function prepare(page: Page, request: APIRequestContext, options: Options = {}) {
  const made = await createSession(request, { title: `expanded-layout-${Date.now()}`, invocation: "sleep 300" });
  sessions.push(made.id);
  await patchPreferences(request, { welcome_seen: true, compact: false, list_sort: "title" });
  await pinAutoSelect(page, made.id);
  const real = await request.get("/api/hosts");
  expect(real.ok(), "the actual registry must load before interception").toBe(true);
  const build = requireHelmBuild(real, "expanded host movement");
  const registry = await real.json() as Registry;
  const local = registry.hosts.find((host) => host.kind === "local");
  expect(local, "keep the one actual reserved local identity").toBeTruthy();
  expect(local!.state.phase, "the real selected terminal starts on a connected local host").toBe("connected");
  const ids = options.ids ?? { ...FAKE_IDS };
  expect(new Set([local!.id, ...KEYS.map((key) => ids[key])]).size).toBe(4);
  const listingResponse = await request.get("/api/sessions");
  expect(listingResponse.ok()).toBe(true);
  const listing = await listingResponse.json() as { sessions: SessionRow[] } & Record<string, unknown>;
  const selected = listing.sessions.find((row) => row.id === made.id);
  expect(selected, "register and list the owned selected session before freezing it").toBeTruthy();
  const now = Math.floor(Date.now() / 1000);
  const frozen = { ...selected!, status: { state: "idle" }, stale: false,
    last_activity_at: now - 30, seen_activity_at: now, notifications: [], notifications_read_through: 0 };
  let manual = options.localManual;
  let registryFailed = false;
  let revision = 1;
  let hostReplies = 0;
  let probes = 0;
  const replies = new Map<number, Reply>([local!.id, ...KEYS.map((key) => ids[key])].map((id): [number, Reply] => [id, idle(id)]));
  for (const [key, reply] of Object.entries(options.progress?.(ids) ?? {})) {
    if (reply) replies.set(ids[key as Key], reply);
  }
  const queues = new Map<number, Held<Reply>[]>();
  const counts = new Map<number, number>();
  const inflight = new Map<number, number>();
  const hostQueue: Held<{ manual?: string; failed: boolean }>[] = [];
  const unexpected: string[] = [];
  page.on("request", (sent) => {
    const path = new URL(sent.url()).pathname;
    if (sent.method() !== "GET" && /^\/api\/hosts\//.test(path) &&
      path !== "/api/hosts/probe" && !path.endsWith("/alias")) unexpected.push(`${sent.method()} ${path}`);
  });
  await page.route((url) => url.pathname === "/api/hosts", async (route) => {
    expect(route.request().method()).toBe("GET");
    const stop = hostQueue.shift();
    const value = stop?.reply ?? { manual, failed: registryFailed };
    if (stop) { stop.seen = true; await stop.wait(); }
    try {
      const headers = { "x-farhelm-build": build, "content-type": value.failed ? "text/plain" : "application/json" };
      if (value.failed) await route.fulfill({ status: 503, headers, body: "retained-host-read-error-stable" });
      else {
        const shownLocal = value.manual === undefined ? local! : { ...local!, state: {
          phase: "unreachable-reprobing", cause: "local-supervisor-not-running", last_error: value.manual,
        } };
        await route.fulfill({ status: 200, headers, json: { hosts: [shownLocal, ...KEYS.map((key) => remote(ids[key], key))] } });
      }
      hostReplies++;
      if (stop) stop.completed = true;
    } catch (error) { if (stop) stop.error = String(error); throw error; }
  });
  await page.route((url) => /^\/api\/hosts\/\d+\/provisioning$/.test(url.pathname), async (route) => {
    expect(route.request().method()).toBe("GET");
    const id = Number(new URL(route.request().url()).pathname.split("/")[3]);
    expect(replies.has(id), "every progress route belongs to a displayed fixture identity").toBe(true);
    const stop = queues.get(id)?.shift();
    const value = structuredClone(stop?.reply ?? replies.get(id)!);
    inflight.set(id, (inflight.get(id) ?? 0) + 1);
    if (stop) stop.seen = true;
    try {
      if (stop) await stop.wait();
      await progressReply(route, build, value);
      counts.set(id, (counts.get(id) ?? 0) + 1);
      if (stop) stop.completed = true;
    } catch (error) { if (stop) stop.error = String(error); throw error; }
    finally { inflight.set(id, inflight.get(id)! - 1); }
  });
  await page.route((url) => url.pathname === "/api/hosts/probe", async (route) => {
    expect(manual, "only the controlled local manual phase may probe").toBeDefined();
    expect(route.request().method()).toBe("POST");
    expect(route.request().postDataJSON()).toEqual({ target: { kind: "local" } });
    probes++;
    await route.fulfill({ status: 200, headers: { "x-farhelm-build": build },
      json: { result: "manual", reason: "local fixture uses the manual setup path" } });
  });
  await page.route((url) => url.pathname === "/api/sessions", async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    await route.fulfill({ headers: { "x-farhelm-build": build },
      json: { ...listing, sessions: [frozen], total: 1, matching: 1, truncated: false } });
  });
  await observeFeedReaders(page);
  const feed = await stubFeed(page);
  feed.notifyOnConnect(revision);
  await page.setViewportSize({ width: 1280, height: 1800 });
  await page.goto("/");
  await waitForSessionReady(page, made.id);
  await feed.waitForConnection(1);
  await waitForFeedReadersSettled(page);
  await openHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).toBeChecked();
  const host = (key: Key) => page.locator(`.host-row[data-host-id="${ids[key]}"]`);
  const localRow = page.locator(`.host-row[data-host-id="${local!.id}"]`);
  await expect(page.locator(".hosts-panel .host-row")).toHaveCount(4);
  await expect.poll(() => page.locator(".hosts-panel .host-row").evaluateAll((rows) =>
    rows.map((row) => Number(row.getAttribute("data-host-id"))))).toEqual([local!.id, ...KEYS.map((key) => ids[key])]);
  await expect.poll(() => [local!.id, ...KEYS.map((key) => ids[key])]
    .every((id) => (counts.get(id) ?? 0) > 0), { message: "all mounted hosts decoded a stamped progress reply" }).toBe(true);
  if (manual !== undefined) {
    await expect(localRow.locator(".provisioning-error")).toHaveText("local fixture uses the manual setup path");
    await expect(localRow.locator(".provisioning-manual")).toContainText(manual);
    expect(probes).toBe(1);
  }
  await expect(page.locator('[role="dialog"][aria-modal="true"]')).toHaveCount(0);
  await expect(page.locator(".provisioning-trace")).toHaveCount(0);
  await page.mouse.move(1200, 1600);
  const state = async () => ({ ids, hostReplies, probes, counts: [...counts], inflight: [...inflight], unexpected,
    readers: await readFeedReaders(page), healthySockets: feed.openSockets() });
  diagnostics.push(state);

  /** Queue exactly one accepted reply; subsequent reads retain the same endpoint state. */
  const holdProgress = (key: Key, value: Reply) => {
    replies.set(ids[key], value);
    const stop = held(value);
    queues.set(ids[key], [...(queues.get(ids[key]) ?? []), stop]);
    return stop;
  };
  /** Queue a whole read reply: Err retains its prior vector; manual text stays in its normal host field. */
  const holdRegistry = (next: { manual?: string; failed: boolean }) => {
    manual = next.manual; registryFailed = next.failed;
    const stop = held(next); hostQueue.push(stop); return stop;
  };
  /** Wake the real counted readers through the fixture's healthy feed socket. */
  const notice = async () => {
    expect(feed.openSockets(), "notify only the fixture's live healthy socket").toBeGreaterThan(0);
    feed.notify(++revision);
  };
  /** Check displayed membership and disclosure without hiding deliberately pending writes. */
  const quiet = async () => {
    expect(unexpected).toEqual([]);
    await expect(page.locator(".hosts-panel .host-row")).toHaveCount(4);
    await expect(page.locator(".hosts-status, .hosts-load-error")).toHaveCount(0);
    await expect(page.locator(".hosts-refresh-error")).toHaveCount(registryFailed ? 1 : 0);
    await expect(page.locator(".provisioning-trace")).toHaveCount(0);
    await expect(page.locator('[role="dialog"][aria-modal="true"]')).toHaveCount(0);
    await expect(page.locator(".host-details-toggle")).toBeChecked();
    expect(await page.locator(".hosts-panel .host-row").evaluateAll((rows) =>
      rows.map((row) => Number(row.getAttribute("data-host-id"))))).toEqual([local!.id, ...KEYS.map((key) => ids[key])]);
    expect(await page.locator(".sidebar-content").evaluate((node) => node.scrollHeight <= node.clientHeight)).toBe(true);
    expect(feed.openSockets()).toBeGreaterThan(0);
    for (const key of KEYS) await expect(host(key)).toHaveAttribute("data-host-phase", "connected");
  };
  await quiet();
  return { ids, host, localRow, holdProgress, holdRegistry, notice, quiet, state };
}

type Fixture = Awaited<ReturnType<typeof prepare>>;

// ===== Read-only movement evidence and keyboard admission ==============

/** Paint boundaries let native ResizeObserver deliver; they never delay by elapsed time. */
async function observedFrames(page: Page) {
  await page.evaluate(() => new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  }));
}

/** Exercise the real disclosure checkbox before opening a measured popup.
 * A global disclosure could hide an unexpected automatic expansion. Checking
 * folded output first proves the intended membership, then restores Details.
 */
async function disclosurePremise(page: Page, fixture: Fixture, localExpanded = false) {
  const toggle = page.locator(".host-details-toggle");
  await toggle.uncheck();
  await observedFrames(page);
  for (const key of KEYS) {
    await expect(fixture.host(key).locator(".provisioning-run, .provisioning-read-error")).toHaveCount(0);
  }
  if (localExpanded) {
    await expect(fixture.localRow.locator(".provisioning-manual")).toBeVisible();
    await expect(fixture.localRow.locator(".provisioning-error")).toBeVisible();
  }
  await toggle.check();
  await observedFrames(page);
  await fixture.quiet();
  await expect(page.locator(".provisioning-run-fold, .provisioning-trace")).toHaveCount(0);
  await page.mouse.move(1200, 1600);
}

/** Read all competing layout inputs without modifying DOM, styles or application state. */
async function geometry(toggle: Locator) {
  return toggle.evaluate((node) => {
    const box = (element: Element) => {
      const rect = element.getBoundingClientRect();
      return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
    };
    const panel = document.querySelector(".hosts-panel")!;
    const sidebar = document.querySelector(".sidebar-content")!;
    const shell = document.querySelector(".app-shell")!;
    const rows = [...panel.querySelectorAll(".host-row")];
    const row = node.closest(".host-row")!;
    return {
      target: box(node), index: rows.indexOf(row), panel: box(panel),
      ids: rows.map((host) => Number(host.getAttribute("data-host-id"))),
      phases: rows.map((host) => host.getAttribute("data-host-phase")),
      rowHeights: rows.map((host) => box(host).height),
      scroll: [scrollX, scrollY, sidebar.scrollLeft, sidebar.scrollTop, shell.scrollLeft, shell.scrollTop],
      viewport: [innerWidth, innerHeight], sidebar: box(sidebar),
      errors: panel.querySelectorAll(".hosts-refresh-error").length,
    };
  });
}
type Geometry = Awaited<ReturnType<typeof geometry>>;

/** Retain native layout evidence without wrapping the application's observer.
 * Integration positives allow panel growth but still exclude scroll/viewport
 * dismissal. Negative cases additionally reject every changed size sample,
 * including a resize-and-return. Overflow is always an explicit failure.
 */
async function observePanel(page: Page) {
  const handle = await page.evaluateHandle(() => {
    const panel = document.querySelector(".hosts-panel")!;
    const style = getComputedStyle(panel);
    const chrome = [style.borderTopWidth, style.borderBottomWidth, style.borderLeftWidth,
      style.borderRightWidth, style.paddingTop, style.paddingBottom, style.paddingLeft, style.paddingRight];
    const sizes: { width: number; height: number; outer: number[] }[] = [];
    let overflow = false;
    const sidebar = document.querySelector(".sidebar-content")!;
    const shell = document.querySelector(".app-shell")!;
    const scroll = () => [scrollX, scrollY, sidebar.scrollLeft, sidebar.scrollTop, shell.scrollLeft, shell.scrollTop];
    const initialScroll = scroll();
    const initialViewport = [innerWidth, innerHeight];
    let scrollChanges = 0;
    let viewportEvents = 0;
    const onScroll = () => {
      if (scroll().some((value, index) => value !== initialScroll[index])) scrollChanges++;
    };
    const onResize = () => { viewportEvents++; };
    document.addEventListener("scroll", onScroll, { capture: true, passive: true });
    window.addEventListener("resize", onResize);
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        if (sizes.length === 32) { overflow = true; continue; }
        const outer = panel.getBoundingClientRect();
        sizes.push({ width: entry.contentRect.width, height: entry.contentRect.height,
          outer: [outer.x, outer.y, outer.width, outer.height] });
      }
    });
    observer.observe(panel);
    return {
      read: () => ({ sizes: [...sizes], overflow, chrome, initialScroll, currentScroll: scroll(),
        initialViewport, currentViewport: [innerWidth, innerHeight], scrollChanges, viewportEvents,
        currentChrome: (() => {
          const current = getComputedStyle(panel);
          return [current.borderTopWidth, current.borderBottomWidth, current.borderLeftWidth,
            current.borderRightWidth, current.paddingTop, current.paddingBottom, current.paddingLeft, current.paddingRight];
        })() }),
      stop: () => {
        observer.disconnect();
        document.removeEventListener("scroll", onScroll, true);
        window.removeEventListener("resize", onResize);
      },
    };
  });
  const read = () => handle.evaluate((value) => value.read());
  diagnostics.push(read);
  observers.push(async () => { await handle.evaluate((value) => value.stop()); await handle.dispose(); });
  await expect.poll(async () => (await read()).sizes.length, { message: "drain the observer's initial baseline delivery" }).toBeGreaterThan(0);
  await observedFrames(page);
  /** Panel height may change in a positive; surrounding dismissal inputs may not. */
  const environmentUnchanged = async () => {
    await observedFrames(page);
    const result = await read();
    expect(result.overflow, "bounded size evidence must not overflow").toBe(false);
    expect(result.currentChrome).toEqual(result.chrome);
    expect(result.currentScroll).toEqual(result.initialScroll);
    expect(result.currentViewport).toEqual(result.initialViewport);
    expect(result.scrollChanges, "a scroll-and-return is a competing dismissal").toBe(0);
    expect(result.viewportEvents, "a resize-and-return is a competing broad close").toBe(0);
    return result;
  };
  return {
    environmentUnchanged,
    /** A negative needs unchanged native samples, not merely equal endpoint rectangles. */
    async unchanged() {
      const result = await environmentUnchanged();
      const baseline = result.sizes[0];
      for (const size of result.sizes) {
        expect(Math.abs(size.width - baseline.width), JSON.stringify(result)).toBeLessThan(0.5);
        expect(Math.abs(size.height - baseline.height), JSON.stringify(result)).toBeLessThan(0.5);
        for (let i = 0; i < 4; i++) expect(Math.abs(size.outer[i] - baseline.outer[i]), JSON.stringify(result)).toBeLessThan(0.5);
      }
    },
  };
}

/** Keyboard opening excludes pointer hold/outside dismissal as the cause of a result. */
async function openHost(page: Page, row: Locator) {
  const toggle = row.locator(".host-row-menu");
  await toggle.focus();
  await expect(toggle).toBeFocused();
  await page.keyboard.press("ArrowDown");
  const panel = row.locator(".host-row-menu-panel");
  await expect(panel).toBeVisible();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  await expect(row.locator(".host-retry")).toBeFocused();
  await expect(row.locator(".host-row-menu-flyout")).toHaveCSS("opacity", "1");
  await expect(row.locator(".host-row-menu-flyout")).toHaveAttribute("style", /left: var\(--menu-left\)/);
  diagnostics.push(async () => ({ geometry: await geometry(toggle),
    expanded: await toggle.getAttribute("aria-expanded"), labels: await commands(panel) }));
  return { toggle, panel };
}

/** Only item identities matter; busy commands may remain focusable aria-disabled. */
async function commands(panel: Locator) {
  return panel.getByRole("menuitem").locator(".session-row-menu-label").allTextContents();
}

/** Membership, anchor size and surrounding layout remain stable during an accepted reply.
 * Panel height is deliberately absent: positive integration cases add real
 * flow and may also invoke the existing aggregate resize closer.
 */
function stableInputs(before: Geometry, after: Geometry) {
  for (const key of ["x", "y", "width"] as const) {
    expect(Math.abs(after.panel[key] - before.panel[key]), `panel ${key}`).toBeLessThan(0.5);
  }
  for (const key of ["x", "y", "width", "height"] as const) {
    expect(Math.abs(after.sidebar[key] - before.sidebar[key]), `sidebar ${key}`).toBeLessThan(0.5);
  }
  expect(after.ids).toEqual(before.ids);
  expect(after.phases).toEqual(before.phases);
  expect(after.index).toBe(before.index);
  expect(after.scroll).toEqual(before.scroll);
  expect(after.viewport).toEqual(before.viewport);
  expect(after.errors).toBe(before.errors);
  expect(after.target.x).toBe(before.target.x);
  expect(after.target.width).toBe(before.target.width);
  expect(after.target.height).toBe(before.target.height);
}

/** A single request boundary proves the menu survived until the intended reply. */
async function awaitHeld(stop: Held<unknown>) {
  await expect.poll(() => ({ seen: stop.seen, completed: stop.completed, error: stop.error }), {
    message: "the named route boundary must be held and error-free before release",
  }).toEqual({ seen: true, completed: false, error: undefined });
}

/** Finish forwarding the one owned reply; there is no cross-request batching premise. */
async function releaseReply(stop: Held<unknown>) {
  stop.release();
  await expect.poll(() => ({ completed: stop.completed, error: stop.error }), {
    message: "the owned continuation must finish forwarding its reply",
  }).toEqual({ completed: true, error: undefined });
}

/** Real flow growth must move and close the same surviving anchor.
 * This is browser integration evidence. A panel resize can also close the
 * menu; production hook/capture tests isolate the added causal report.
 */
async function integrationOutcome(page: Page, fixture: Fixture, key: Key, opened: Awaited<ReturnType<typeof openHost>>,
  before: Geometry, labels: string[], observation: Awaited<ReturnType<typeof observePanel>>) {
  await observedFrames(page);
  await fixture.quiet();
  const after = await geometry(opened.toggle);
  stableInputs(before, after);
  await observation.environmentUnchanged();
  expect(Math.abs(after.target.y - before.target.y), "the same-index intermediate toggle must physically move").toBeGreaterThan(5);
  await expect(opened.panel).toHaveCount(0);
  await expect(opened.toggle).toHaveAttribute("aria-expanded", "false");
  const reopened = await openHost(page, fixture.host(key));
  expect(await commands(reopened.panel), "no command withdrawal may explain the close").toEqual(labels);
  return after;
}

/** A harmless change leaves the measured opening and every native size sample intact. */
async function unchangedOutcome(page: Page, fixture: Fixture, opened: Awaited<ReturnType<typeof openHost>>,
  before: Geometry, labels: string[], observation: Awaited<ReturnType<typeof observePanel>>) {
  await observedFrames(page);
  await fixture.quiet();
  const after = await geometry(opened.toggle);
  stableInputs(before, after);
  expect(Math.abs(after.panel.height - before.panel.height), "panel height").toBeLessThan(0.5);
  expect(after.rowHeights).toEqual(before.rowHeights);
  await observation.unchanged();
  expect(after.target.y).toBe(before.target.y);
  await expect(opened.panel).toBeVisible();
  await expect(opened.toggle).toHaveAttribute("aria-expanded", "true");
  expect(await commands(opened.panel)).toEqual(labels);
  return after;
}

// ===== Expanded progress and retained host admission ===================

/** One accepted progress reply changes the actual expanded row above B.
 * Retained Err is established before opening so its banner/broad close is
 * outside this integration boundary. No second reply counterbalances A.
 */
async function expandedCase(page: Page, request: APIRequestContext, retainedError = false) {
  const fixture = await prepare(page, request, { progress: (ids) => ({
    a: run(ids.a, "source-running", "before"),
  }) });
  const source = fixture.host("a");
  await expect(source.locator('[data-step="upload-farhelm"]')).toHaveAttribute("data-status", "running");
  await expect(source.locator('[data-step="install-farhelm"]')).toHaveAttribute("data-status", "pending");
  await expect(source.locator(".provisioning-step-message")).toHaveCount(0);
  await expect(page.locator(".provisioning-error, .provisioning-warning, .provisioning-read-error")).toHaveCount(0);
  await disclosurePremise(page, fixture);
  if (retainedError) {
    const failed = fixture.holdRegistry({ failed: true });
    await fixture.notice(); await awaitHeld(failed); await releaseReply(failed);
    await expect(page.locator(".hosts-refresh-error")).toContainText("retained-host-read-error-stable");
    // The legacy broad close/banner reflow happens before the tested opening.
    await observedFrames(page);
    await fixture.quiet();
  } else await waitForFeedReadersSettled(page);
  const observation = await observePanel(page);
  const opened = await openHost(page, fixture.host("b"));
  const labels = await commands(opened.panel);
  const before = await geometry(opened.toggle);
  const a = fixture.holdProgress("a", run(fixture.ids.a, "source-running", "advanced", LONG_MESSAGE));
  await fixture.notice(); await awaitHeld(a);
  await expect(opened.panel).toBeVisible();
  expect(await geometry(opened.toggle)).toEqual(before);
  await releaseReply(a);
  await expect(source.locator(".provisioning-step-message")).toHaveText(LONG_MESSAGE);
  await expect(source.locator('[data-step="upload-farhelm"]')).toHaveAttribute("data-status", "skipped");
  await expect(source.locator('[data-step="install-farhelm"]')).toHaveAttribute("data-status", "running");
  await expect(source.locator(".provisioning-run")).toHaveAttribute("data-provisioning-status", "running");
  await expect(page.locator(".provisioning-error, .provisioning-warning, .provisioning-read-error")).toHaveCount(0);
  if (!retainedError) await waitForFeedReadersSettled(page);
  const after = await integrationOutcome(page, fixture, "b", opened, before, labels, observation);
  const sourceIndex = before.ids.indexOf(fixture.ids.a);
  const growth = after.rowHeights[sourceIndex] - before.rowHeights[sourceIndex];
  expect(growth, "the new explanation must add real wrapped flow").toBeGreaterThan(5);
  expect(Math.abs(after.target.y - before.target.y - growth)).toBeLessThan(0.5);
  expect((await fixture.state()).probes).toBe(0);
}

/** Real expanded text growth moves a surviving lower anchor and closes its menu. */
test("expanded progress growth moves and closes the lower host menu", async ({ page, request }) => {
  await expandedCase(page, request);
});

/** A later row's status-only advancement must preserve the earlier opening.
 * The status column already contains completed/running labels. No message
 * or row is added; strict native observations must confirm unchanged flow.
 */
test("status-only expanded progress below an earlier host leaves its menu open", async ({ page, request }) => {
  const fixture = await prepare(page, request, { progress: (ids) => ({
    b: run(ids.b, "below-running", "before"),
  }) });
  const source = fixture.host("b");
  await expect(source.locator('[data-step="upload-farhelm"]')).toHaveAttribute("data-status", "running");
  await expect(source.locator('[data-step="install-farhelm"]')).toHaveAttribute("data-status", "pending");
  await expect(source.locator(".provisioning-step-message")).toHaveCount(0);
  await expect(page.locator(".provisioning-error, .provisioning-warning, .provisioning-read-error")).toHaveCount(0);
  await disclosurePremise(page, fixture);
  await waitForFeedReadersSettled(page);
  const observation = await observePanel(page);
  const opened = await openHost(page, fixture.host("a"));
  const labels = await commands(opened.panel);
  const before = await geometry(opened.toggle);
  const b = fixture.holdProgress("b", run(fixture.ids.b, "below-running", "advanced"));
  await fixture.notice(); await awaitHeld(b);
  await expect(opened.panel).toBeVisible();
  expect(await geometry(opened.toggle)).toEqual(before);
  await releaseReply(b);
  await expect(source.locator('[data-step="upload-farhelm"]')).toHaveAttribute("data-status", "completed");
  await expect(source.locator('[data-step="install-farhelm"]')).toHaveAttribute("data-status", "running");
  await expect(source.locator(".provisioning-run")).toHaveAttribute("data-provisioning-status", "running");
  await expect(source.locator(".provisioning-step-message")).toHaveCount(0);
  await expect(page.locator(".provisioning-error, .provisioning-warning, .provisioning-read-error")).toHaveCount(0);
  await waitForFeedReadersSettled(page);
  await unchangedOutcome(page, fixture, opened, before, labels, observation);
  expect((await fixture.state()).probes).toBe(0);
});

/** A failed registry refresh retains real rendered rows and their progress reader.
 * This checks browser integration; the actual callback unit test isolates
 * retained admission from the aggregate panel close mechanism.
 */
test("expanded progress growth closes a host menu retained across a failed registry read", async ({ page, request }) => {
  await expandedCase(page, request, true);
});

/** Local last_error belongs only to the manual child line in this phase.
 * Actual manual growth must move B, while the ordinary local detail and the
 * settled probe remain unchanged. Aggregate resize is allowed in this
 * integration positive; hook tests isolate the manual causal report.
 */
test("local manual remedy growth moves and closes the lower host menu", async ({ page, request }) => {
  const fixture = await prepare(page, request, { localManual: MANUAL_SHORT });
  const manual = fixture.localRow.locator(".provisioning-manual");
  await expect(manual).toContainText(MANUAL_SHORT);
  const shortManualHeight = (await manual.boundingBox())!.height;
  const ordinary = await fixture.localRow.locator(".host-detail").textContent();
  expect(ordinary).toBe("no supervisor is running on this machine");
  await expect(fixture.localRow.locator(".host-remedy:not(.provisioning-manual)")).toHaveCount(0);
  const manualClass = await manual.getAttribute("class");

  await disclosurePremise(page, fixture, true);
  await waitForFeedReadersSettled(page);
  const observation = await observePanel(page);
  const opened = await openHost(page, fixture.host("b"));
  const labels = await commands(opened.panel);
  const before = await geometry(opened.toggle);
  const local = fixture.holdRegistry({ manual: MANUAL_LONG, failed: false });
  await fixture.notice(); await awaitHeld(local);
  await expect(opened.panel).toBeVisible();
  expect(await geometry(opened.toggle)).toEqual(before);
  await releaseReply(local);
  await expect(manual).toContainText(MANUAL_LONG);
  await expect(manual).toHaveAttribute("class", manualClass!);
  await expect(fixture.localRow.locator(".host-detail")).toHaveText(ordinary!);
  await expect(fixture.localRow.locator(".provisioning-error")).toHaveText("local fixture uses the manual setup path");
  await expect(fixture.localRow.locator(".host-remedy:not(.provisioning-manual)")).toHaveCount(0);
  expect((await fixture.state()).probes, "manual text refresh must not start another local probe").toBe(1);
  const growth = (await manual.boundingBox())!.height - shortManualHeight;
  expect(growth, "the actual manual command must gain wrapped lines").toBeGreaterThan(5);
  await waitForFeedReadersSettled(page);
  const after = await integrationOutcome(page, fixture, "b", opened, before, labels, observation);
  expect(Math.abs(after.target.y - before.target.y - growth)).toBeLessThan(0.5);
});

/** Metadata replacement must update real DOM attributes without inventing flow movement.
 * Starting from completed history avoids an unseen-running-outcome warning,
 * which would be genuine new visible content rather than metadata churn.
 */
test("unchanged expanded output with new progress metadata leaves the lower menu open", async ({ page, request }) => {
  const fixture = await prepare(page, request, { progress: (ids) => ({
    a: run(ids.a, "metadata-completed-before", "completed", SHORT_MESSAGE),
  }) });
  await expect(fixture.host("a").locator(".provisioning-step-message")).toHaveText(SHORT_MESSAGE);
  await disclosurePremise(page, fixture);
  await waitForFeedReadersSettled(page);
  const observation = await observePanel(page);
  const opened = await openHost(page, fixture.host("b"));
  const labels = await commands(opened.panel);
  const before = await geometry(opened.toggle);
  const next = run(fixture.ids.a, "metadata-completed-after", "completed", SHORT_MESSAGE);
  next.json.steps[0].step = "create-directories-new-metadata";
  next.json.steps[0].paths = [{ full: "/opt/layout-fixture/metadata/full", short: "~/metadata/full" }];
  next.json.steps[0].explanation = "metadata not drawn in the expanded progress list";
  const a = fixture.holdProgress("a", next);
  await fixture.notice(); await awaitHeld(a);
  await expect(opened.panel).toBeVisible();
  expect(await geometry(opened.toggle)).toEqual(before);
  await releaseReply(a);
  await expect(fixture.host("a").locator('[data-step="create-directories-new-metadata"]')).toHaveCount(1);
  await expect(fixture.host("a").locator(".provisioning-step-message")).toHaveText(SHORT_MESSAGE);
  await expect(page.locator(".provisioning-error, .provisioning-warning, .provisioning-read-error")).toHaveCount(0);
  await waitForFeedReadersSettled(page);
  await unchangedOutcome(page, fixture, opened, before, labels, observation);
  expect((await fixture.state()).probes).toBe(0);
});

// ===== Real late Settings refusal =====================================

/** Registry writes allocate real owned ids before any route fixture is installed. */
async function registerHost(request: APIRequestContext, browserName: string, suffix: string) {
  const destination = `expanded-${browserName}-${Date.now()}-${suffix}.invalid`;
  const response = await request.post("/api/hosts", { data: { ssh: destination } });
  expect(response.ok(), await response.text()).toBe(true);
  const body = await response.json() as { id: number };
  hosts.push(body.id);
  expect(Number.isSafeInteger(body.id)).toBe(true);
  return body.id;
}

/** The only browser write is an alias collision; no SSH installation starts. */
async function beginAliasCollision(page: Page, fixture: Fixture, alias: string) {
  await openHost(page, fixture.host("a"));
  await fixture.host("a").locator(".host-settings").click();
  const dialog = page.locator(".host-settings-dialog.host-settings-refreshed");
  await expect(dialog).toBeVisible();
  await dialog.locator(".host-alias").click();
  const draft = dialog.locator(".host-destination-input");
  // This exercises the refusal, so establish input focus rather than relying
  // on autofocus when an editor mounts inside an already focused dialog.
  await draft.focus();
  await expect(draft).toBeFocused();
  await draft.fill(alias);
  await dialog.locator(".host-save-destination").click();
  return dialog;
}

/** A real delayed 409 can reach the row after Settings has closed.
 * One pending write owns the page token while B opens. Forwarding the exact
 * fetched response checks retained-outcome integration without inventing a
 * mutation API. Panel growth is allowed; unit tests isolate causal delivery.
 */
test("a real late settings refusal moves and closes the lower host menu", async ({ page, request, browserName }) => {
  const aId = await registerHost(request, browserName, "alias-source");
  const cId = await registerHost(request, browserName, "alias-rival");
  const alias = `layout conflict ${browserName} ${Date.now()} retained evidence`;
  expect([...alias].length, "the collision must pass the real 64-character syntax boundary").toBeLessThanOrEqual(64);
  const setRival = await request.post(`/api/hosts/${cId}/alias`, { data: { alias } });
  expect(setRival.ok(), await setRival.text()).toBe(true);
  const fixture = await prepare(page, request, { ids: { a: aId, b: FAKE_IDS.b, c: cId } });
  const aliasPath = `/api/hosts/${aId}/alias`;
  const rowError = fixture.host("a").locator(".host-error");
  await expect(rowError).toHaveCount(0);
  await disclosurePremise(page, fixture);
  const write = held(undefined);
  let fetched = 0;
  let forwarded = 0;
  let refusal = "";
  await page.route((url) => url.pathname === aliasPath, async (route) => {
    expect(route.request().method()).toBe("POST");
    expect(route.request().postDataJSON()).toEqual({ alias: alias.trim() });
    fetched++;
    expect(fetched, "exactly one pending Settings request owns this boundary").toBe(1);
    const response = await route.fetch();
    try {
      expect(response.status()).toBe(409);
      refusal = (await response.text()).trim();
      expect(refusal).toContain(alias.trim());
      write.seen = true;
      await write.wait();
      await route.fulfill({ response });
      forwarded++;
      write.completed = true;
    } catch (error) { write.error = String(error); throw error; }
    finally { await response.dispose(); }
  });
  const pendingDialog = await beginAliasCollision(page, fixture, alias);
  await awaitHeld(write);
  expect(refusal, "the actual held 409 body must already be known").not.toBe("");
  await expect(pendingDialog.locator(".host-save-destination")).toBeDisabled();
  await expect(pendingDialog.locator(".host-settings-close")).toBeEnabled();
  await pendingDialog.locator(".host-settings-close").click();
  await expect(fixture.host("a").locator(".host-row-menu")).toBeFocused();
  await expect(rowError).toHaveCount(0);
  await expect(page.locator(".provisioning-error, .provisioning-warning, .provisioning-read-error")).toHaveCount(0);
  await fixture.quiet();
  await page.mouse.move(1200, 1600);
  const observation = await observePanel(page);
  const opened = await openHost(page, fixture.host("b"));
  await expect(fixture.host("b").locator(".host-retry")).toHaveAttribute("aria-disabled", "true");
  const labels = await commands(opened.panel);
  const before = await geometry(opened.toggle);
  await awaitHeld(write);
  await expect(opened.panel).toBeVisible();
  expect(await geometry(opened.toggle)).toEqual(before);
  await releaseReply(write);
  await expect(rowError).toHaveText(refusal);
  const errorHeight = (await rowError.boundingBox())!.height;
  expect(errorHeight, "the actual refusal must occupy visible row flow").toBeGreaterThan(5);
  expect(fetched).toBe(1);
  expect(forwarded, "only one pending browser write owned the page token").toBe(1);
  await waitForFeedReadersSettled(page);
  const after = await integrationOutcome(page, fixture, "b", opened, before, labels, observation);
  expect(Math.abs(after.target.y - before.target.y - errorHeight)).toBeLessThan(0.5);
  expect((await fixture.state()).probes).toBe(0);
});
