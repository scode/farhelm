import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { expect, test } from "./helpers/evidence";
import {
  cleanupSession,
  createSession,
  observeFeedReaders,
  openHostsPanel,
  patchPreferences,
  pinAutoSelect,
  resetPreferences,
  showSessionsPanel,
  type SessionRow,
  stubFeed,
  waitForFeedReadersSettled,
} from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { waitForSessionReady } from "./helpers/terminal-readiness";

// Host ids are routing identities, not ranks. Every fixture starts in a
// deliberately different display order and keeps its real local host intact.
type HostKey = "a" | "b" | "c";
const IDS = { a: 9303, b: 9301, c: 9302 } as const;
const KEYS: HostKey[] = ["a", "b", "c"];
const SHORT_BUILD = "balanced-short";
const LONG_BUILD = "balanced-detail-sentinel " + "visible detail words ".repeat(8);
const created: string[] = [];

/** The connected wire fields this renderer fixture controls.
 * Null identities retain the same visible detail suffix in all three rows;
 * explicit age flags keep long build text from changing the menu's offers.
 */
type FixtureHost = {
  id: number;
  kind: "ssh";
  destination: string;
  alias: null;
  name: string;
  identity: null;
  remote_farhelm: null;
  remote_state_dir: null;
  incarnation: number;
  yolo_without_asking: false;
  commands_without_asking: false;
  links_without_asking: false;
  local_links_without_asking: false;
  state: {
    phase: "connected";
    identity: null;
    build_version: string;
    old_version: false;
    newer_version: false;
    refresh: { status: "ok"; sessions: 0 };
  };
};

/** Register each real allocation before another setup step can fail. */
async function ownedSession(request: APIRequestContext, title: string): Promise<string> {
  const session = await createSession(request, { title, invocation: "sleep 300" });
  created.push(session.id);
  return session.id;
}

test.afterEach(async ({ page, request }, testInfo) => {
  await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
    ["routes", () => page.unrouteAll({ behavior: "ignoreErrors" })],
    ...created.splice(0).reverse().map((id): [string, () => Promise<void>] => [
      `session ${id}`, () => cleanupSession(request, id),
    ]),
    ["preferences", () => resetPreferences(request)],
  ]);
});

/** Author a host reply without changing the real registry or starting SSH. */
function fixtureHost(key: HostKey, build: string): FixtureHost {
  const name = `layout-host-${key}.invalid`;
  return {
    id: IDS[key], kind: "ssh", destination: name, alias: null, name,
    identity: null, remote_farhelm: null, remote_state_dir: null,
    incarnation: IDS[key], yolo_without_asking: false, commands_without_asking: false,
    links_without_asking: false, local_links_without_asking: false,
    state: {
      phase: "connected", identity: null, build_version: build,
      old_version: false, newer_version: false, refresh: { status: "ok", sessions: 0 },
    },
  };
}

/** Freeze competing layout sources while retaining a real selected terminal.
 * The only changing reply is the successful host snapshot. Idle progress is
 * supplied for every rendered host so unknown fixture ids cannot add diagnosis
 * or automatic disclosure, and the healthy counted feed owns each refresh.
 */
async function prepare(page: Page, request: APIRequestContext, longAt?: HostKey) {
  const stamp = Date.now();
  const targetSession = await ownedSession(request, `aaa-host-layout-${stamp}`);
  const selected = await ownedSession(request, `zzz-host-layout-${stamp}`);
  await patchPreferences(request, { welcome_seen: true, compact: false, list_sort: "title" });
  await pinAutoSelect(page, selected);
  const registryResponse = await request.get("/api/hosts");
  expect(registryResponse.ok(), "the real registry must load before freezing its local row").toBe(true);
  const build = requireHelmBuild(registryResponse, "host popover fixture");
  const registry = await registryResponse.json() as {
    hosts: ({ id: number; kind: string; state: { phase: string } } & Record<string, unknown>)[];
  };
  const local = registry.hosts.find((host) => host.kind === "local");
  expect(local, "the controlled list must preserve the real reserved local row").toBeTruthy();
  expect(local!.state.phase, "the selected session's local host must actually be connected").toBe("connected");
  expect(KEYS.map((key) => IDS[key])).not.toContain(local!.id);
  const listingResponse = await request.get("/api/sessions");
  expect(listingResponse.ok(), "real sessions must be listed before their row data is frozen").toBe(true);
  const listing = await listingResponse.json() as { sessions: SessionRow[] } & Record<string, unknown>;
  const now = Math.floor(Date.now() / 1000);
  const sessions = [targetSession, selected].map((id) => {
    const session = listing.sessions.find((candidate) => candidate.id === id);
    expect(session, "each owned session must exist before interception").toBeTruthy();
    return {
      ...session!, status: { state: "idle" }, stale: false, pinned: false,
      last_activity_at: now - 30, seen_activity_at: now,
      notifications: id === targetSession ? [{ seq: 1, at: now, text: "host-layout notification" }] : [],
      notifications_read_through: 1,
    };
  });
  let order: HostKey[] = [...KEYS];
  let builds = Object.fromEntries(KEYS.map((key) => [key, key === longAt ? LONG_BUILD : SHORT_BUILD])) as Record<HostKey, string>;
  let revision = 1;
  let hostReplies = 0;
  const progressReplies = new Map<number, number>();
  const renderedIds = [local!.id, ...KEYS.map((key) => IDS[key])];
  const unexpectedMutations: string[] = [];
  page.on("request", (sent) => {
    const path = new URL(sent.url()).pathname;
    const match = /^\/api\/hosts\/(\d+)(?:\/|$)/.exec(path);
    if (match && KEYS.some((key) => IDS[key] === Number(match[1])) && sent.method() !== "GET") {
      unexpectedMutations.push(`${sent.method()} ${path}`);
    }
  });
  await page.route((url) => url.pathname === "/api/hosts", async (route) => {
    expect(route.request().method(), "this fixture never mutates the registry").toBe("GET");
    await route.fulfill({
      headers: { "x-farhelm-build": build, "content-type": "application/json" },
      json: { hosts: [local, ...order.map((key) => fixtureHost(key, builds[key]))] },
    });
    hostReplies++;
  });
  await page.route((url) => {
    const match = /^\/api\/hosts\/(\d+)\/provisioning$/.exec(url.pathname);
    return !!match && renderedIds.includes(Number(match[1]));
  }, async (route) => {
    expect(route.request().method()).toBe("GET");
    const id = Number(new URL(route.request().url()).pathname.split("/")[3]);
    await route.fulfill({
      headers: { "x-farhelm-build": build, "content-type": "application/json" },
      json: { run_id: null, operation: null, status: "completed", steps: [], message: null },
    });
    progressReplies.set(id, (progressReplies.get(id) ?? 0) + 1);
  });
  await page.route((url) => url.pathname === "/api/sessions", async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    await route.fulfill({
      headers: { "x-farhelm-build": build, "content-type": "application/json" },
      json: { ...listing, sessions, total: 2, matching: 2, truncated: false },
    });
  });
  await observeFeedReaders(page);
  const feed = await stubFeed(page);
  feed.notifyOnConnect(revision);
  await page.setViewportSize({ width: 1280, height: 1400 });
  await page.goto("/");
  await waitForSessionReady(page, selected);
  await feed.waitForConnection(1);
  await waitForFeedReadersSettled(page);
  await openHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).toBeChecked();

  const host = (key: HostKey) => page.locator(`.host-row[data-host-id="${IDS[key]}"]`);
  const detail = (key: HostKey) => host(key).locator(".host-detail");
  const ids = () => page.locator(".host-row").evaluateAll((nodes) => nodes.map((node) => Number(node.getAttribute("data-host-id"))));

  /** Visible content and completed reads jointly establish the quiet premise. */
  const quiet = async () => {
    await expect(page.locator(".hosts-panel")).toHaveCount(1);
    await expect(page.locator(".hosts-panel .host-row")).toHaveCount(4);
    await expect(page.locator(".hosts-status, .hosts-refresh-error, .hosts-load-error")).toHaveCount(0);
    await expect(page.locator(".hosts-panel .host-error, .hosts-panel .host-warning, .provisioning-run, .provisioning-trace")).toHaveCount(0);
    // The notification list is deliberately role=dialog; only modal surfaces
    // would invalidate this fixture's claim that no competing handoff occurred.
    await expect(page.locator('[role="dialog"][aria-modal="true"]')).toHaveCount(0);
    await expect.poll(() => renderedIds.every((id) => (progressReplies.get(id) ?? 0) > 0), {
      message: "every mounted host must receive the stamped idle progress reply",
    }).toBe(true);
    for (const key of KEYS) {
      await expect(host(key)).toHaveAttribute("data-host-phase", "connected");
      await expect(detail(key)).toContainText(builds[key]);
      await expect(host(key).locator(".host-destination-detail")).toHaveCount(0);
    }
    expect(unexpectedMutations, "opening a menu cannot issue a fake host mutation").toEqual([]);
    expect(await page.locator(".sidebar-content").evaluate((node) => node.scrollHeight <= node.clientHeight),
      "all fixture rows must fit without a sidebar scroll").toBe(true);
  };
  await expect.poll(ids).toEqual(renderedIds);
  await quiet();
  await waitForFeedReadersSettled(page);
  await page.mouse.move(1200, 1100);

  /** One atomically committed reply changes order or text, never both by accident. */
  const publish = async (nextOrder: HostKey[], longHost?: HostKey) => {
    const previousReplies = hostReplies;
    const previousProgress = new Map(progressReplies);
    order = [...nextOrder];
    builds = Object.fromEntries(KEYS.map((key) => [key, key === longHost ? LONG_BUILD : SHORT_BUILD])) as Record<HostKey, string>;
    feed.notify(++revision);
    await expect.poll(() => hostReplies, { message: "the staged host reply must be delivered" }).toBeGreaterThan(previousReplies);
    await expect.poll(ids).toEqual([local!.id, ...nextOrder.map((key) => IDS[key])]);
    for (const key of KEYS) await expect(detail(key)).toContainText(builds[key]);
    await expect.poll(() => renderedIds.every((id) => (progressReplies.get(id) ?? 0) > (previousProgress.get(id) ?? 0)), {
      message: "the same healthy notice must also reach each host's idle progress reader",
    }).toBe(true);
    await waitForFeedReadersSettled(page);
    await quiet();
  };
  return { host, detail, publish, targetSession, selected };
}

/** Use keyboard opening so neither pointer hold nor outside dismissal explains a close. */
async function openHost(page: Page, host: Locator) {
  const toggle = host.locator(".host-row-menu");
  await toggle.focus();
  await expect(toggle).toBeFocused();
  await page.keyboard.press("ArrowDown");
  const panel = host.locator(".host-row-menu-panel");
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  await expect(panel).toBeVisible();
  await expect(host.locator(".host-retry")).toBeFocused();
  await expect(host.locator(".provisioning-update")).toBeVisible();
  await expect(host.locator(".provisioning-uninstall")).toBeVisible();
  await expect(host.locator(".host-row-menu-flyout")).toHaveCSS("opacity", "1");
  await expect(host.locator(".host-row-menu-flyout")).toHaveAttribute("style", /left: var\(--menu-left\)/);
  return { toggle, panel };
}

/** Compare commands after movement as well as phases, excluding action withdrawal. */
async function actions(panel: Locator): Promise<string[]> {
  return panel.getByRole("menuitem").evaluateAll((nodes) => nodes.map((node) => node.textContent!.replace(/\s+/g, " ").trim()));
}

/** Observe positions and competing causes without changing DOM or application state. */
async function geometry(toggle: Locator) {
  return toggle.evaluate((node) => {
    const rect = (element: Element) => {
      const box = element.getBoundingClientRect();
      return { x: box.x, y: box.y, width: box.width, height: box.height };
    };
    const sidebar = document.querySelector(".sidebar-content")!;
    const shell = document.querySelector(".app-shell")!;
    const panel = document.querySelector(".hosts-panel")!;
    const hosts = [...panel.querySelectorAll(".host-row")];
    const row = node.closest(".host-row, .session-row")!;
    const peers = row.matches(".host-row") ? hosts : [...document.querySelectorAll(".session-row")];
    return {
      target: rect(node), index: peers.indexOf(row), panel: rect(panel),
      hostIds: hosts.map((host) => Number(host.getAttribute("data-host-id"))),
      rowHeights: hosts.map((host) => rect(host).height).sort((a, b) => a - b),
      phases: hosts.map((host) => [host.getAttribute("data-host-id"), host.getAttribute("data-host-phase")]).sort(),
      scroll: [sidebar.scrollTop, sidebar.scrollLeft, shell.scrollTop, shell.scrollLeft],
      viewport: [innerWidth, innerHeight],
    };
  });
}

type Geometry = Awaited<ReturnType<typeof geometry>>;

/** A balanced change must not borrow coverage from aggregate resize or scrolling. */
function expectStableFrame(before: Geometry, after: Geometry) {
  for (const key of ["x", "y", "width", "height"] as const) {
    expect(Math.abs(after.panel[key] - before.panel[key]), `host panel ${key} must stay fixed`).toBeLessThan(0.5);
  }
  expect(after.scroll).toEqual(before.scroll);
  expect(after.viewport).toEqual(before.viewport);
  expect(after.phases).toEqual(before.phases);
  expect(after.rowHeights).toEqual(before.rowHeights);
  expect(after.target.x).toBe(before.target.x);
  expect(after.target.width).toBe(before.target.width);
  expect(after.target.height).toBe(before.target.height);
}

/** Fixture symmetry is asserted before a permutation relies on equal row heights. */
async function equalHostHeights(hosts: Locator[]) {
  const heights = await Promise.all(hosts.map(async (host) => (await host.boundingBox())!.height));
  expect(Math.max(...heights) - Math.min(...heights), "the controlled host rows must have equal heights").toBeLessThan(0.5);
}

for (const scenario of [
  { name: "a same-count host reorder closes its moved menu", target: "b", order: ["a", "c", "b"], moved: true },
  { name: "a host reorder below an unchanged menu leaves it open", target: "a", order: ["a", "c", "b"], moved: false },
  { name: "an equal-height host permutation above an unchanged menu leaves it open", target: "c", order: ["b", "a", "c"], moved: false },
] as const) {
  /** Row identity and physical y distinguish true movement from an index-only
   * suffix rule. The complements keep anchors outside the changed contribution.
   */
  test(scenario.name, async ({ page, request }) => {
    const fixture = await prepare(page, request);
    await equalHostHeights(KEYS.map(fixture.host));
    const host = fixture.host(scenario.target);
    const { toggle, panel } = await openHost(page, host);
    const commands = await actions(panel);
    const before = await geometry(toggle);
    await fixture.publish([...scenario.order]);
    const after = await geometry(toggle);
    expectStableFrame(before, after);
    expect(after.hostIds).not.toEqual(before.hostIds);
    if (scenario.moved) {
      expect(after.index).toBeGreaterThan(before.index);
      expect(after.target.y).toBeGreaterThan(before.target.y);
      await expect(panel).toHaveCount(0);
      await expect(toggle).toHaveAttribute("aria-expanded", "false");
      const reopened = await openHost(page, host);
      expect(await actions(reopened.panel), "the same commands must survive the reorder").toEqual(commands);
    } else {
      expect(after.index).toBe(before.index);
      expect(after.target.y).toBe(before.target.y);
      await expect(panel).toBeVisible();
      await expect(toggle).toHaveAttribute("aria-expanded", "true");
      expect(await actions(panel)).toEqual(commands);
    }
  });
}

for (const kind of ["menu", "bell"] as const) {
  /** Concealed management cannot move a session anchor. Registry order still
   * changes behind Sessions, so a host-only invalidation broadcast is visible.
   */
  test(`a concealed same-height host reorder leaves the session ${kind} open`, async ({ page, request }) => {
    const fixture = await prepare(page, request);
    await equalHostHeights(KEYS.map(fixture.host));
    await showSessionsPanel(page);
    const target = page.locator(`[data-session-id="${fixture.targetSession}"]`);
    const toggle = target.locator(kind === "menu" ? ".session-row-menu" : ".session-row-bell");
    await toggle.focus();
    await expect(toggle).toBeFocused();
    await page.keyboard.press(kind === "menu" ? "ArrowDown" : "Enter");
    const panel = target.locator(kind === "menu" ? ".session-row-menu-panel" : ".session-bell-panel");
    await expect(panel).toBeVisible();
    await expect(kind === "menu" ? target.locator(".session-row-rename") : panel).toBeFocused();
    const flyout = target.locator(kind === "menu" ? ".session-row-menu-flyout" : ".session-bell-flyout");
    await expect(flyout).toHaveCSS("opacity", "1");
    await expect(flyout).toHaveAttribute("style", /left: var\(--menu-left\)/);
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    const before = await geometry(toggle);
    await fixture.publish(["a", "c", "b"]);
    const after = await geometry(toggle);
    expectStableFrame(before, after);
    expect(after.hostIds).not.toEqual(before.hostIds);
    expect(after.index).toBe(before.index);
    expect(after.target.y).toBe(before.target.y);
    await expect(panel).toBeVisible();
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
  });
}

for (const scenario of [
  { name: "balanced detail growth above and shrinkage below closes the same-index host menu", target: "b", longBefore: "c", longAfter: "a", moved: true },
  { name: "balanced detail changes below an earlier host menu leave it open", target: "a", longBefore: "c", longAfter: "b", moved: false },
  { name: "the same detail contributions exchanged wholly above a host menu leave it open", target: "c", longBefore: "b", longAfter: "a", moved: false },
] as const) {
  /** Text remains visible in the same connected rows, but one contribution
   * grows as another shrinks. Total panel resize cannot explain the positive;
   * unchanged prefixes must retain their menus in the complementary cases.
   */
  test(scenario.name, async ({ page, request }) => {
    const fixture = await prepare(page, request, scenario.longBefore);
    const growing = fixture.detail(scenario.longAfter);
    const shrinking = fixture.detail(scenario.longBefore);
    await expect(growing).toContainText(SHORT_BUILD);
    await expect(shrinking).toContainText("balanced-detail-sentinel");
    const shortHeight = (await growing.boundingBox())!.height;
    const longHeight = (await shrinking.boundingBox())!.height;
    expect(longHeight, "the already-visible long detail must really wrap before opening").toBeGreaterThan(shortHeight + 10);
    const host = fixture.host(scenario.target);
    const { toggle, panel } = await openHost(page, host);
    const commands = await actions(panel);
    const before = await geometry(toggle);
    await fixture.publish([...KEYS], scenario.longAfter);
    await expect(growing).toContainText("balanced-detail-sentinel");
    await expect(shrinking).toContainText(SHORT_BUILD);
    expect((await growing.boundingBox())!.height).toBe(longHeight);
    expect((await shrinking.boundingBox())!.height).toBe(shortHeight);
    const after = await geometry(toggle);
    expectStableFrame(before, after);
    expect(after.hostIds).toEqual(before.hostIds);
    expect(after.index).toBe(before.index);
    if (scenario.moved) {
      expect(after.target.y - before.target.y).toBe(longHeight - shortHeight);
      await expect(panel).toHaveCount(0);
      await expect(toggle).toHaveAttribute("aria-expanded", "false");
      const reopened = await openHost(page, host);
      expect(await actions(reopened.panel), "only preceding detail height changed, not this menu's commands").toEqual(commands);
    } else {
      expect(after.target.y).toBe(before.target.y);
      await expect(panel).toBeVisible();
      await expect(toggle).toHaveAttribute("aria-expanded", "true");
      expect(await actions(panel)).toEqual(commands);
    }
  });
}
