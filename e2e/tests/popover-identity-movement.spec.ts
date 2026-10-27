import { expectCompactRows } from "./helpers/list-header";
import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { expect, test } from "./helpers/evidence";
import {
  cleanupSession, createSession, observeFeedReaders, openRowMenu, patchPreferences,
  pinAutoSelect, readFeedReaders, resetPreferences, stubFeed, waitForFeedReadersSettled,
} from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { waitForSessionReady } from "./helpers/terminal-readiness";
import { routeGate } from "./helpers/route-gate";

// These renderer fixtures keep the selected real terminal and authenticated
// read paths, while changing only named listing inputs. Physical premises
// distinguish identity flow from index, separator, detail and host movement.
// Production hook tests cover deferred admission and additional input variants.
const LONG_TITLE = "10 identity wrapping ordinary long session title ".repeat(8);
type WireRow = { id: string; pinned: boolean; title: string } & Record<string, unknown>;
type Listing = { sessions: WireRow[] } & Record<string, unknown>;
const owned: string[] = [];
const releases: (() => void)[] = [];
let captureFailure: (() => Promise<unknown>) | undefined;
let diagnosticTarget: string | undefined;

test.beforeEach(() => { owned.length = 0; releases.length = 0; captureFailure = undefined; diagnosticTarget = undefined; });
test.afterEach(async ({ page, request }, info) => {
  await cleanupOwnedSteps(info.status !== info.expectedStatus, [
    ["probe state before retirement", async () => {
      if (info.status !== info.expectedStatus && captureFailure) {
        await info.attach("identity-wrap-failure", {
          body: JSON.stringify(await captureFailure()), contentType: "application/json",
        });
      }
    }],
    ["held real requests", () => { for (const release of releases) release(); }],
    ["read route retirement", () => page.unrouteAll({ behavior: "wait" })],
    ...owned.splice(0).reverse().map((id): [string, () => Promise<void>] => [
      `owned session ${id}`, () => cleanupSession(request, id),
    ]),
    ["preferences", () => resetPreferences(request)],
  ]);
});

/** Register cleanup immediately; only these real sessions may be changed or removed. */
async function makeSession(request: APIRequestContext, title: string) {
  const made = await createSession(request, { title, invocation: "sleep 300" });
  owned.push(made.id);
  return made.id;
}

/** Force browser layout after accepted data or width input, without elapsed-time sleeps. */
async function rendered(page: Page) {
  await page.evaluate(() => new Promise<void>((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
  ));
}

/** Use the supported separator gesture before opening; never set CSS or storage directly. */
async function resizeSidebar(page: Page, desired: number) {
  expect(desired).toBeGreaterThanOrEqual(240);
  expect(desired).toBeLessThanOrEqual(600);
  const handle = page.getByRole("separator", { name: "Sidebar width" });
  await expect(handle).toHaveAttribute("data-sidebar-bound", "true");
  const current = Number(await handle.getAttribute("aria-valuenow"));
  const box = (await handle.boundingBox())!;
  const x = box.x + box.width / 2;
  const y = box.y + Math.min(200, box.height / 2);
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + desired - current, y, { steps: 8 });
  await page.mouse.up();
  await expect(handle).toHaveAttribute("aria-valuenow", String(desired));
  await rendered(page);
}

/** Measure the actual open target, which may be the source or an earlier row. */
async function measure(source: Locator, targetId: string) {
  diagnosticTarget = targetId;
  return source.evaluate((node, targetId) => {
    const rect = (element: Element) => {
      const box = element.getBoundingClientRect();
      return { x: box.x, y: box.y, width: box.width, height: box.height };
    };
    const copy = node.querySelector(".session-identity-copy")!;
    const title = copy.querySelector(".session-title")!;
    const pin = copy.querySelector(".session-pin");
    const style = getComputedStyle(copy);
    const titleStyle = getComputedStyle(title);
    const rows = [...document.querySelectorAll(".session-list .session-row")];
    const target = rows.find((row) => row.getAttribute("data-session-id") === targetId)!;
    const sidebar = document.querySelector(".app-sidebar")!;
    const shell = document.querySelector(".app-shell")!;
    const font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
    const pinStyle = pin ? getComputedStyle(pin) : null;
    const pinFont = pinStyle ? `${pinStyle.fontWeight} ${pinStyle.fontSize} ${pinStyle.fontFamily}` : null;
    const titleRange = document.createRange();
    titleRange.selectNodeContents(title);
    const titleTextWidth = titleRange.getBoundingClientRect().width;
    titleRange.detach();
    const panel = target.querySelector(".session-row-menu-panel");
    const progress = copy.querySelector(".delete-progress");
    const activity = node.querySelector(".session-activity-column")!;
    return {
      ids: rows.map((row) => row.getAttribute("data-session-id")),
      pinned: rows.map((row) => row.getAttribute("data-session-pinned")),
      sourceIndex: rows.indexOf(node), targetIndex: rows.indexOf(target),
      rowHeights: rows.map((row) => rect(row).height),
      row: rect(node), identity: rect(node.querySelector(".session-row-identity")!),
      copy: rect(copy), title: rect(title), pin: pin ? rect(pin) : null,
      titleFloor: parseFloat(titleStyle.minWidth), titleScrollWidth: title.scrollWidth, titleTextWidth,
      titleFont: titleStyle.font, copyFont: font, wrap: style.flexWrap,
      gridTracks: getComputedStyle(node.querySelector(".session-row-identity")!).gridTemplateColumns,
      activity: rect(activity), activityFont: getComputedStyle(activity).font,
      copyLineHeight: style.lineHeight, titleLineHeight: titleStyle.lineHeight,
      progressBox: progress ? rect(progress) : null,
      progressFont: progress ? getComputedStyle(progress).font : null,
      progressLineHeight: progress ? getComputedStyle(progress).lineHeight : null,
      progressText: progress?.textContent ?? null,
      titlePosition: titleStyle.position,
      gap: style.columnGap, rowGap: style.rowGap,
      titleText: title.textContent,
      fontsLoaded: document.fonts.status === "loaded", pinFontReady: pinFont ? document.fonts.check(pinFont, "\uf08d") : true,
      target: rect(target.querySelector(".session-row-menu")!),
      targetExpanded: target.querySelector(".session-row-menu")!.getAttribute("aria-expanded"),
      menu: panel ? rect(panel) : null,
      commandLabels: panel ? [...panel.querySelectorAll(".session-row-menu-label")].map((label) => label.textContent) : [],
      detailPrefix: rows.slice(0, rows.indexOf(target)).map((row) => ({
        detail: [...row.querySelectorAll(".session-row-detail")].map((part) => part.textContent),
        refusal: [...row.querySelectorAll(".action-error")].map((part) => part.textContent),
        meta: row.querySelector(".session-row-meta")?.textContent ?? null,
      })),
      ages: rows.map((row) => row.querySelector(".status-time")?.textContent ?? null),
      separators: document.querySelectorAll(".session-pin-separator").length,
      header: rect(document.querySelector(".session-heading")!),
      hosts: rect(document.querySelector(".hosts-panel")!),
      progress: document.querySelectorAll(".provisioning-trace, .provisioning-run, .session-list .delete-progress").length,
      hostDiagnostics: document.querySelectorAll(".hosts-refresh-error, .hosts-load-error, .hosts-status").length,
      sidebar: rect(sidebar), sidebarWidth: parseFloat(getComputedStyle(sidebar).width),
      fits: sidebar.scrollHeight <= sidebar.clientHeight,
      scroll: [scrollX, scrollY, sidebar.scrollLeft, sidebar.scrollTop, shell.scrollLeft, shell.scrollTop],
      viewport: [innerWidth, innerHeight],
    };
  }, targetId);
}
type Shape = Awaited<ReturnType<typeof measure>>;

/** Identity changes leave the legacy index/separator/detail and ancestor inputs unchanged. */
function samePremises(before: Shape, after: Shape, progress = 0) {
  expect(after.ids).toEqual(before.ids);
  expect(after.sourceIndex).toBe(before.sourceIndex);
  expect(after.targetIndex).toBe(before.targetIndex);
  expect(after.separators).toBe(before.separators);
  expect(after.detailPrefix).toEqual(before.detailPrefix);
  expect(after.header).toEqual(before.header);
  expect(after.hosts).toEqual(before.hosts);
  expect(after.progress).toBe(progress);
  expect(after.hostDiagnostics).toBe(0);
  expect(after.sidebar).toEqual(before.sidebar);
  expect(after.sidebarWidth).toBe(before.sidebarWidth);
  expect(after.scroll).toEqual(before.scroll);
  expect(after.viewport).toEqual(before.viewport);
  expect(after.fits).toBe(true);
  expect(after.fontsLoaded).toBe(true);
  expect(after.pinFontReady).toBe(true);
  expect(after.copyFont).toBe(before.copyFont);
  expect(after.titleFont).toBe(before.titleFont);
  expect(after.titleFloor).toBe(before.titleFloor);
  expect(after.target.x).toBe(before.target.x);
  expect(after.target.width).toBe(before.target.width);
  expect(after.target.height).toBe(before.target.height);
}

/** Freeze real row identities and all non-pin listing fields while the real terminal stays live. */
async function prepare(page: Page, request: APIRequestContext, options: {
  compact?: boolean; selectedA?: boolean; pinnedPrefix?: boolean;
} = {}) {
  const stamp = Date.now();
  const c = await makeSession(request, `00 identity context ${stamp}`);
  const a = await makeSession(request, `${LONG_TITLE}${stamp}`);
  const b = await makeSession(request, `20 identity target ${stamp}`);
  await patchPreferences(request, { welcome_seen: true, compact: options.compact ?? false,
    list_sort: "title", skip_session_confirmations: true });
  await pinAutoSelect(page, options.selectedA ? a : c);
  const real = await request.get("/api/sessions");
  expect(real.ok()).toBe(true);
  const build = requireHelmBuild(real, "identity wrapping probe");
  const listing = await real.json() as Listing;
  const now = Math.floor(Date.now() / 1000);
  const ids = [c, a, b];
  const rows = ids.map((id) => {
    const row = listing.sessions.find((candidate) => candidate.id === id);
    expect(row, "every frozen row must come from the actual owned listing").toBeTruthy();
    return { ...row!, pinned: !!options.pinnedPrefix && id !== b, stale: false, status: { state: "idle" },
      last_activity_at: now - 7200, seen_activity_at: now,
      notifications: [], notifications_read_through: 0 };
  });
  const hostsResponse = await request.get("/api/hosts");
  expect(hostsResponse.ok()).toBe(true);
  expect(requireHelmBuild(hostsResponse, "identity probe hosts")).toBe(build);
  const registry = await hostsResponse.json() as { hosts: { id: number }[] };
  const hostIds = new Set(registry.hosts.map((host) => host.id));
  let pinned = !!options.pinnedPrefix;
  let revision = 1;
  let replies = 0;
  await page.route((url) => url.pathname === "/api/sessions", async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    const snapshot = rows.map((row) => ({ ...row, pinned: row.id === a ? pinned : row.pinned }));
    // This is the real pin partition's served order. The UI hold must keep
    // [C,A,B] displayed, with the newly pinned A below leading unpinned C.
    snapshot.sort((left, right) => Number(right.pinned) - Number(left.pinned) || left.title.localeCompare(right.title));
    await route.fulfill({ headers: { "x-farhelm-build": build, "content-type": "application/json" },
      json: { ...listing, sessions: snapshot, total: 3, matching: 3, truncated: false } });
    replies++;
  });
  await page.route((url) => url.pathname === "/api/hosts", async (route) => {
    expect(route.request().method()).toBe("GET");
    await route.fulfill({ headers: { "x-farhelm-build": build }, json: registry });
  });
  await page.route((url) => /^\/api\/hosts\/\d+\/provisioning$/.test(url.pathname), async (route) => {
    expect(route.request().method()).toBe("GET");
    const id = Number(new URL(route.request().url()).pathname.split("/")[3]);
    expect(hostIds.has(id)).toBe(true);
    await route.fulfill({ headers: { "x-farhelm-build": build }, json: {
      host_id: id, run_id: null, operation: null, status: "completed", steps: [],
      message: "no provisioning run has been recorded for this host",
    } });
  });
  await observeFeedReaders(page);
  const feed = await stubFeed(page);
  feed.notifyOnConnect(revision);
  await page.setViewportSize({ width: 1280, height: 1800 });
  await page.goto("/");
  await waitForSessionReady(page, options.selectedA ? a : c);
  await feed.waitForConnection(1);
  // The resting pointer over C starts a delayed summary card whose bold
  // title uses a face absent from the list. A bare fonts.ready wait cannot
  // cover a load that starts later; load the declared faces before any
  // pointer-dependent geometry so the card cannot disturb that premise.
  await page.evaluate(async () => {
    const loads: Promise<FontFace>[] = [];
    document.fonts.forEach((face) => { loads.push(face.load()); });
    await Promise.all(loads);
    await document.fonts.ready;
  });
  await waitForFeedReadersSettled(page);
  await expect(page.locator(".session-row")).toHaveCount(3);
  await expectCompactRows(page, !!options.compact);
  const source = page.locator(`.session-row[data-session-id="${a}"]`);
  const target = page.locator(`.session-row[data-session-id="${b}"]`);
  await expect(source.locator(".session-title")).toHaveText(`${LONG_TITLE}${stamp}`);
  await expect(target).toHaveAttribute("data-session-selected", "false");
  captureFailure = async () => ({ shape: await measure(source, diagnosticTarget ?? b), readers: await readFeedReaders(page), replies,
    healthySockets: feed.openSockets(), pinned });

  /** Complete a stamped accepted listing after replacing only owned wire-fixture fields. */
  const refresh = async () => {
    // Refresh the ordinary hold immediately before publication, after any
    // real API write/readback. A slow request must not make a correct close
    // release into pin sorting before the physical premise is inspected.
    const openPanel = page.locator(".session-row-menu-panel");
    if (await openPanel.count()) {
      const safe = (await page.locator(`.session-row[data-session-id="${c}"] .session-row-open`).boundingBox())!;
      await page.mouse.move(safe.x + 21, safe.y + 9);
      await expect(openPanel).toBeVisible();
    }
    const completed = page.waitForResponse((response) =>
      new URL(response.url()).pathname === "/api/sessions" && response.request().method() === "GET"
    );
    expect(feed.openSockets()).toBeGreaterThan(0);
    feed.notify(++revision);
    const response = await completed;
    expect(response.ok()).toBe(true);
    expect(response.headers()["x-farhelm-build"]).toBe(build);
    const decoded = await response.json() as Listing;
    const served = rows.map((row) => ({ ...row, pinned: row.id === a ? pinned : row.pinned }))
      .sort((left, right) => Number(right.pinned) - Number(left.pinned) || left.title.localeCompare(right.title));
    expect(decoded.sessions.map((row) => row.id)).toEqual(served.map((row) => row.id));
    expect(decoded.sessions.find((row) => row.id === a)?.pinned).toBe(pinned);
    expect(await response.finished()).toBeNull();
    await expect(source).toHaveAttribute("data-session-pinned", String(pinned));
    await waitForFeedReadersSettled(page);
    await page.evaluate(() => document.fonts.ready);
    await rendered(page);
    return decoded;
  };
  /** Keep pin acceptance real while its normal read-render surface remains controlled. */
  const publishPin = async (value: boolean) => {
    const changed = await request.put(`/api/sessions/${encodeURIComponent(a)}/pin`, { data: { pinned: value } });
    expect(changed.ok(), await changed.text()).toBe(true);
    const readback = await request.get("/api/sessions");
    expect(readback.ok()).toBe(true);
    expect((await readback.json() as Listing).sessions.find((row) => row.id === a)?.pinned).toBe(value);
    pinned = value;
    await refresh();
  };
  /** This changes an API reply fixture, never browser application state or DOM. */
  const replaceSourceRead = async (fields: Record<string, unknown>) => {
    Object.assign(rows[1], fields);
    const decoded = await refresh();
    for (const [name, value] of Object.entries(fields)) {
      expect(decoded.sessions.find((row) => row.id === a)![name], `accepted source ${name}`).toEqual(value);
    }
  };
  /** One accepted listing owns both prefix titles; no per-row publication can split the exchange. */
  const replacePrefixTitles = async (cTitle: string, aTitle: string) => {
    rows[0].title = cTitle;
    rows[1].title = aTitle;
    const decoded = await refresh();
    expect(decoded.sessions.find((row) => row.id === c)!.title).toBe(cTitle);
    expect(decoded.sessions.find((row) => row.id === a)!.title).toBe(aTitle);
  };
  const row = (id: string) => page.locator(`.session-row[data-session-id="${id}"]`);
  return { a, b, c, ids, source, target, row, publishPin, replaceSourceRead, replacePrefixTitles };
}

type Fixture = Awaited<ReturnType<typeof prepare>>;

/** Keep the real display hold after a correct close, so pin sorting cannot
 * contaminate the measured row-height cause. Movement over C is fresh and
 * ordinary; the rendered-order assertions prove the hold actually survived.
 */
async function openHeldMenu(page: Page, fixture: Fixture, targetId: string) {
  const target = fixture.row(targetId);
  await openRowMenu(target);
  await expect(target.locator(".session-row-rename")).toBeFocused();
  const safe = (await fixture.row(fixture.c).locator(".session-row-open").boundingBox())!;
  await page.mouse.move(safe.x + 20, safe.y + 8);
  expect(await page.locator(".session-list").evaluate((node) => node.matches(":hover"))).toBe(true);
  await expect(target.locator(".session-row-menu-panel")).toBeVisible();
  // Settle this mounted surface's font layout. The delayed row hover card's
  // faces were loaded in prepare, before the pointer could schedule it.
  await page.evaluate(() => document.fonts.ready);
  const before = await measure(fixture.source, targetId);
  expect(before.ids).toEqual(fixture.ids);
  expect(before.targetExpanded).toBe("true");
  expect(before.commandLabels.length).toBeGreaterThan(3);
  expect(before.fits).toBe(true);
  expect(before.fontsLoaded).toBe(true);
  expect(before.pinFontReady).toBe(true);
  return before;
}

/** Pin updates must be real read witnesses while all older close inputs stay constant. */
async function pinCase(page: Page, request: APIRequestContext, where: "lower" | "own" | "earlier" | "compact") {
  const fixture = await prepare(page, request, { compact: where === "compact" });
  await resizeSidebar(page, 240);
  const targetId = where === "own" ? fixture.a : where === "earlier" ? fixture.c : fixture.b;
  const before = await openHeldMenu(page, fixture, targetId);
  expect(before.separators).toBe(0);
  expect(before.pinned).toEqual(["false", "false", "false"]);
  expect(before.pin).toBeNull();
  await fixture.publishPin(true);
  const after = await measure(fixture.source, targetId);
  samePremises(before, after);
  expect(after.ages).toEqual(before.ages);
  expect(after.copy.width).toBe(before.copy.width);
  expect(after.titleText).toBe(before.titleText);
  expect(after.pinned).toEqual(["false", "true", "false"]);
  expect(after.pin).not.toBeNull();
  const panel = fixture.row(targetId).locator(".session-row-menu-panel");
  if (where === "compact") {
    expect(after.wrap).toBe("nowrap");
    expect(Math.abs(after.row.height - before.row.height)).toBeLessThan(0.1);
    expect(after.target).toEqual(before.target);
    await expect(panel).toBeVisible();
    expect(after.commandLabels).toEqual(before.commandLabels);
  } else {
    expect(after.title.y).toBeGreaterThan(after.pin!.y + 2);
    expect(after.copy.height).toBeGreaterThan(before.copy.height);
    const growth = after.row.height - before.row.height;
    expect(growth, "pin and title must add a physical flex line").toBeGreaterThan(5);
    if (where === "lower") {
      expect(Math.abs(after.target.y - before.target.y - growth)).toBeLessThan(0.5);
      await expect(panel).toHaveCount(0);
      await expect(fixture.row(targetId).locator(".session-row-menu")).toHaveAttribute("aria-expanded", "false");
    } else {
      expect(after.target).toEqual(before.target);
      await expect(panel).toBeVisible();
      expect(after.targetExpanded).toBe("true");
      expect(after.commandLabels).toEqual(where === "own"
        ? before.commandLabels.map((label) => label === "Pin to top" ? "Unpin" : label)
        : before.commandLabels);
    }
  }
}

/** A held external pin moves B solely through A's real identity wrapping. */
test("pin wrapping above closes the lower actions menu", async ({ page, request }) => {
  await pinCase(page, request, "lower");
});

/** Lower copy growth does not move its own top-aligned action toggle. */
test("pin wrapping preserves its own actions menu", async ({ page, request }) => {
  await pinCase(page, request, "own");
});

/** A source below the open row must not select the unchanged earlier anchor. */
test("pin wrapping below preserves an earlier actions menu", async ({ page, request }) => {
  await pinCase(page, request, "earlier");
});

/** The actual compact row has enough shared line-height floor for its new pin. */
test("compact pin preserves the lower actions menu and row heights", async ({ page, request }) => {
  await pinCase(page, request, "compact");
});

/** One listing can exchange complete earlier contributions without moving B.
 * Both rows stay pinned, so the existing separator is not another cause.
 * Individual height changes are required; an inert string swap is insufficient.
 */
test("one accepted identity contribution exchange wholly above preserves the menu", async ({ page, request }) => {
  const fixture = await prepare(page, request, { pinnedPrefix: true });
  await resizeSidebar(page, 240);
  const long = "00 whole prefix ordinary long title ".repeat(8);
  const short = "10 T";
  await fixture.replacePrefixTitles(long, short);
  await expect(fixture.row(fixture.c).locator(".session-title")).toHaveText(long);
  await expect(fixture.source.locator(".session-title")).toHaveText(short);
  const before = await openHeldMenu(page, fixture, fixture.b);
  expect(before.pinned).toEqual(["true", "true", "false"]);
  expect(before.separators).toBe(1);
  expect(before.rowHeights[0] - before.rowHeights[1]).toBeGreaterThan(5);
  await fixture.replacePrefixTitles(short, long);
  await expect(fixture.row(fixture.c).locator(".session-title")).toHaveText(short);
  await expect(fixture.source.locator(".session-title")).toHaveText(long);
  const after = await measure(fixture.source, fixture.b);
  samePremises(before, after);
  expect(after.ages).toEqual(before.ages);
  expect(after.pinned).toEqual(before.pinned);
  expect(after.rowHeights.slice(0, 2)).toEqual([before.rowHeights[1], before.rowHeights[0]]);
  expect(after.target).toEqual(before.target);
  await expect(fixture.target.locator(".session-row-menu-panel")).toBeVisible();
  expect(after.commandLabels).toEqual(before.commandLabels);
});

/** Raw peer spellings can change while the actual displayed identity stays identical. */
test("same displayed pinned title with new raw spelling preserves the menu", async ({ page, request }) => {
  const fixture = await prepare(page, request, { pinnedPrefix: true });
  await resizeSidebar(page, 240);
  await fixture.replaceSourceRead({ title: "10 ordinary\u200b" });
  await expect(fixture.source.locator(".session-title")).toHaveText("10 ordinary<U+200B>");
  const before = await openHeldMenu(page, fixture, fixture.b);
  await fixture.replaceSourceRead({ title: "10 ordinary<U+200B>" });
  const after = await measure(fixture.source, fixture.b);
  samePremises(before, after);
  expect(after.ages).toEqual(before.ages);
  expect(after.titleText).toBe(before.titleText);
  expect(after.rowHeights).toEqual(before.rowHeights);
  expect(after.target).toEqual(before.target);
  await expect(fixture.target.locator(".session-row-menu-panel")).toBeVisible();
  expect(after.commandLabels).toEqual(before.commandLabels);
});

for (const compact of [false, true]) {
  /** The header can begin deletion while a different row menu is already open.
   * Hold the actual DELETE before forwarding; its later removal/refusal must
   * never explain the close. Entering deletion changes the in-flow child and
   * its font treatment: the causal projection invalidates a later opening
   * even where this loaded face happens to leave its physical bounds equal.
   */
  test(`unpinned delete style invalidates the lower menu with compact=${compact}`, async ({ page, request }) => {
    const fixture = await prepare(page, request, { compact, selectedA: true });
    const gate = routeGate();
    releases.push(() => gate.release());
    let reached = 0;
    await page.route((url) => url.pathname === `/api/sessions/${encodeURIComponent(fixture.a)}`, async (route) => {
      if (route.request().method() !== "DELETE") return route.fallback();
      reached++;
      await gate.wait();
      await route.continue();
    });
    await resizeSidebar(page, 240);
    await openHeldMenu(page, fixture, fixture.b);
    const headerDelete = page.locator(".header-delete");
    await headerDelete.focus();
    await expect(headerDelete).toBeFocused();
    await expect(fixture.target.locator(".session-row-menu-panel"), "focus alone must preserve the measured menu").toBeVisible();
    const before = await measure(fixture.source, fixture.b);
    expect(before.pinned).toEqual(["false", "false", "false"]);
    expect(before.progress).toBe(0);
    await headerDelete.press("Enter");
    await expect.poll(() => reached, { message: "the real DELETE must stay held before forwarding" }).toBe(1);
    await expect(fixture.source.locator(".delete-progress")).toHaveText("Stopping agent…");
    await rendered(page);
    const after = await measure(fixture.source, fixture.b);
    samePremises(before, after, 1);
    expect(after.ages).toEqual(before.ages);
    expect(after.pinned).toEqual(before.pinned);
    expect(after.titleText).toBe(before.titleText);
    expect(after.titlePosition).toBe("absolute");
    expect(after.progressText).toBe("Stopping agent…");
    expect(after.progressBox).not.toBeNull();
    await expect(fixture.target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(fixture.target.locator(".session-row-menu")).toHaveAttribute("aria-expanded", "false");
  });
}

/** A wider real activity track can rewrap pinned copy without a pin change.
 * Fit a short actual title before opening; one accepted age update then
 * supplies the physical cause. No root-font threshold is encoded as policy.
 */
test("activity track growth rewraps pinned copy and closes the lower menu", async ({ page, request }) => {
  const fixture = await prepare(page, request, { pinnedPrefix: true });
  await resizeSidebar(page, 340);
  const now = Math.floor(Date.now() / 1000);
  const old = now - 9999 * 86400 - 3600;
  const older = now - 10000 * 86400 - 3600;
  expect(older).toBeGreaterThan(0);
  await fixture.replaceSourceRead({ title: "10 ordinary", last_activity_at: old,
    notifications: [], notifications_read_through: 0 });
  const initial = await measure(fixture.source, fixture.b);
  const available = initial.copy.width - initial.pin!.width - parseFloat(initial.gap);
  // A proportional face has no average character width that can size another
  // string. Measure the repeated glyph in the loaded title face to make a
  // nearby candidate; real DOM budgets below establish the actual crossing.
  const title = await fixture.source.locator(".session-title").evaluate((node, available) => {
    const canvas = document.createElement("canvas");
    const context = canvas.getContext("2d")!;
    context.font = getComputedStyle(node).font;
    const prefix = "10 ";
    const advance = context.measureText("i").width;
    if (!(advance > 0)) throw new Error("the loaded title face needs a measurable i advance");
    const count = Math.floor((available - context.measureText(prefix).width - 1) / advance);
    if (count < 1 || count > 200) throw new Error(`unexpected title capacity: ${count}`);
    return prefix + "i".repeat(count);
  }, available);
  await fixture.replaceSourceRead({ title });
  const oldShape = await measure(fixture.source, fixture.b);
  await fixture.replaceSourceRead({ last_activity_at: older });
  const newShape = await measure(fixture.source, fixture.b);
  const budget = (shape: Shape) => shape.copy.width - shape.pin!.width - parseFloat(shape.gap);
  expect(budget(oldShape)).toBeGreaterThan(budget(newShape));
  // The activity track's 4ch floor can make the remaining interval narrower
  // than one proportional glyph. Shift the supported sidebar width once so
  // the actual DOM text falls midway between old/new capacities. This is
  // bounded fixture preparation, before the menu's observed update.
  const shift = Math.round(oldShape.titleTextWidth - (budget(oldShape) + budget(newShape)) / 2);
  expect(Math.abs(shift), JSON.stringify({ oldShape, newShape })).toBeLessThanOrEqual(8);
  await resizeSidebar(page, 340 + shift);
  const wrapped = await measure(fixture.source, fixture.b);
  expect(wrapped.title.y).toBeGreaterThan(wrapped.pin!.y + 2);
  await fixture.replaceSourceRead({ last_activity_at: old });
  const fitted = await measure(fixture.source, fixture.b);
  expect(fitted.title.y).toBeLessThan(fitted.pin!.y + fitted.pin!.height);
  expect(fitted.titleTextWidth).toBeLessThanOrEqual(fitted.title.width + 0.1);
  expect(wrapped.row.height - fitted.row.height).toBeGreaterThan(5);
  await expect(fixture.source.locator(".status-time")).toHaveText("9999d");
  const before = await openHeldMenu(page, fixture, fixture.b);
  expect(before.pinned).toEqual(["true", "true", "false"]);
  expect(before.title.y).toBeLessThan(before.pin!.y + before.pin!.height);
  await fixture.replaceSourceRead({ last_activity_at: older });
  await expect(fixture.source.locator(".status-time")).toHaveText("10000d");
  const after = await measure(fixture.source, fixture.b);
  samePremises(before, after);
  expect(after.titleText).toBe(before.titleText);
  expect(after.pinned).toEqual(before.pinned);
  expect(after.ages.filter((_, index) => index !== 1)).toEqual(before.ages.filter((_, index) => index !== 1));
  expect(after.activity.width).toBeGreaterThan(before.activity.width);
  expect(after.copy.width).toBeLessThan(before.copy.width);
  expect(after.title.y).toBeGreaterThan(after.pin!.y + 2);
  const growth = after.row.height - before.row.height;
  expect(growth).toBeGreaterThan(5);
  expect(Math.abs(after.target.y - before.target.y - growth)).toBeLessThan(0.5);
  await expect(fixture.target.locator(".session-row-menu-panel")).toHaveCount(0);
  await expect(fixture.target.locator(".session-row-menu")).toHaveAttribute("aria-expanded", "false");
});

