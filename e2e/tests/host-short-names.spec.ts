import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { expect, test } from "./helpers/evidence";
import { cleanupSession, createSession, patchPreferences, pinAutoSelect, readPreferences, showHostsPanel, type Preferences } from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";

const ALIAS_ID = 9491;
const FQDN_ID = 9492;
const MISSING_ID = 9493;
const UNRECOGNIZED_ID = 9494;
const ALIAS = "Build fleet with a deliberately long caption";
const ALIAS_DESTINATION = "deploy@builder-east.example.invalid";
const FQDN_DESTINATION = "ops@render-west.example.invalid";
const MISSING_NAME = "operator@orphaned-host.example.invalid";
const UNRECOGNIZED_NAME = "Future host without a destination";
const created: string[] = [];
let previous: Preferences | undefined;

/** Read-only presentation fixtures retain every real host, including the stack's SSH target. */
type Host = { id: number; kind: string; name: string; state: { phase: string } & Record<string, unknown> } & Record<string, unknown>;

test.afterEach(async ({ page, request }, info) => {
  await cleanupOwnedSteps(info.status !== info.expectedStatus, [
    ["drain presentation routes", () => page.unrouteAll({ behavior: "wait" })],
    ...created.splice(0).reverse().map((id): [string, () => Promise<void>] => [
      `remove owned session ${id}`, () => cleanupSession(request, id),
    ]),
    ["restore changed preferences", async () => {
      if (previous) {
        await patchPreferences(request, {
          welcome_seen: previous.welcome_seen ?? null,
          compact: previous.compact ?? null,
          list_sort: previous.list_sort ?? null,
          last_selected: previous.last_selected ?? null,
        });
      }
      previous = undefined;
    }],
  ]);
});

/** Register ownership before another setup step can fail. No fixture needs a remote process. */
async function ownedSession(request: APIRequestContext, title: string): Promise<string> {
  const session = await createSession(request, { title, invocation: "sleep 300" });
  created.push(session.id);
  return session.id;
}

/** Select by registry identity rather than a name that this feature deliberately shortens. */
function entry(page: Page, id: number): Locator {
  return page.locator(`.host-rail-entry[data-rail-host-id="${id}"]`);
}

/** A fixed connected snapshot isolates naming and layout from supervisor version changes. */
function remoteHost(id: number, destination: string, alias: string | null, build: string): Host {
  return {
    id, kind: "ssh", name: alias ?? destination, destination, alias,
    identity: null, remote_farhelm: null, remote_state_dir: null, incarnation: id,
    yolo_without_asking: false, commands_without_asking: false,
    links_without_asking: false, local_links_without_asking: false,
    state: {
      phase: "connected", identity: null, build_version: build,
      old_version: false, newer_version: false, refresh: { status: "ok", sessions: 0 },
    },
  };
}

/**
 * Keep a real revealed terminal and its complete host registry. Only the two
 * extra SSH identities, an unrecognized host kind and, when requested, three
 * owned row snapshots are fabricated. The actual sessions keep their full
 * entry-bound references.
 */
async function prepare(page: Page, request: APIRequestContext, rows = false) {
  previous = await readPreferences(request);
  await patchPreferences(request, { welcome_seen: true, compact: false, list_sort: "title" });
  const selected = await ownedSession(request, `zzz-short-name-selected-${Date.now()}`);
  const known = rows ? await ownedSession(request, `aaa-short-name-known-${Date.now()}`) : undefined;
  const missing = rows ? await ownedSession(request, `aab-short-name-missing-${Date.now()}`) : undefined;
  const unrecognized = rows ? await ownedSession(request, `aac-short-name-unrecognized-${Date.now()}`) : undefined;
  await pinAutoSelect(page, selected);
  const response = await request.get("/api/hosts");
  expect(response.ok(), "the real registry must be readable before overriding presentation").toBe(true);
  const build = requireHelmBuild(response, "short host names");
  const registry = await response.json() as { hosts: Host[] };
  const local = registry.hosts.find((host) => host.kind === "local");
  expect(local?.state.phase, "the selected session's local host must be connected").toBe("connected");
  expect(registry.hosts.some((host) => host.kind === "ssh"), "preserve the stack's maintained SSH registration").toBe(true);
  for (const id of [ALIAS_ID, FQDN_ID, MISSING_ID, UNRECOGNIZED_ID]) {
    expect(registry.hosts.map((host) => host.id), "fixture host ids must not shadow a real registration").not.toContain(id);
  }
  const alias = remoteHost(ALIAS_ID, ALIAS_DESTINATION, ALIAS, build);
  const fqdn = remoteHost(FQDN_ID, FQDN_DESTINATION, null, build);
  // A newer helm may introduce a host kind before this client knows how to
  // derive a short name. Its full wire name must still label the rail and rows.
  const future: Host = {
    ...remoteHost(UNRECOGNIZED_ID, "", null, build),
    kind: "future-short-name-fixture", name: UNRECOGNIZED_NAME, destination: null,
  };
  const hosts = registry.hosts.map((host) => host.id === local!.id ? { ...host, alias: null, name: "this machine" } : host);
  hosts.push(alias, fqdn, future);
  await page.route((url) => url.pathname === "/api/hosts", async (route) => {
    expect(route.request().method(), "the naming fixture never changes host registrations").toBe("GET");
    await route.fulfill({ headers: { "x-farhelm-build": build }, json: { ...registry, hosts } });
  });
  await page.route((url) => /^\/api\/hosts\/949[12]\/provisioning$/.test(url.pathname), async (route) => {
    expect(route.request().method()).toBe("GET");
    await route.fulfill({
      headers: { "x-farhelm-build": build },
      json: { run_id: null, operation: null, status: "completed", steps: [], message: null },
    });
  });
  if (rows) {
    const listingResponse = await request.get("/api/sessions");
    expect(listingResponse.ok()).toBe(true);
    const listing = await listingResponse.json();
    const row = (id: string) => {
      const found = listing.sessions.find((candidate: { id: string }) => candidate.id === id);
      expect(found, "an owned session must exist before its host presentation is replaced").toBeTruthy();
      return found;
    };
    const sessions = [
      { ...row(known!), host: FQDN_ID, host_name: FQDN_DESTINATION },
      { ...row(missing!), host: MISSING_ID, host_name: MISSING_NAME },
      { ...row(unrecognized!), host: UNRECOGNIZED_ID, host_name: UNRECOGNIZED_NAME },
      row(selected),
    ];
    await page.route((url) => url.pathname === "/api/sessions", async (route) => {
      if (route.request().method() !== "GET") return route.continue();
      await route.fulfill({
        headers: { "x-farhelm-build": build },
        json: { ...listing, sessions, total: sessions.length, matching: sessions.length, truncated: false },
      });
    });
  }
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");
  await waitForSessionRevealed(page, selected);
  await page.evaluate(() => document.fonts.ready);
  await expect(entry(page, ALIAS_ID).locator(".host-rail-caption")).toHaveText(ALIAS);
  await expect(entry(page, FQDN_ID).locator(".host-rail-caption")).toHaveText("render-west");
  await expect(entry(page, local!.id).locator(".host-rail-caption")).toHaveText("Local");
  await expect(entry(page, UNRECOGNIZED_ID).locator(".host-rail-caption")).toHaveText(UNRECOGNIZED_NAME);
  return { local: local!.id, known, missing, unrecognized };
}

/** Both expansion inputs must be absent before measuring the collapsed baseline. */
async function collapse(page: Page): Promise<void> {
  const outside = page.locator(".new-session-button");
  await outside.focus();
  await expect(outside).toBeFocused();
  await page.mouse.move(1279, 1);
  const hover = page.locator(".host-rail-hover");
  await expect.poll(() => hover.evaluate((node) => ({ hover: node.matches(":hover"), focus: node.matches(":focus-within") })))
    .toEqual({ hover: false, focus: false });
  const rail = page.locator(".host-rail");
  await expect.poll(() => rail.evaluate((node) => node.getBoundingClientRect().width)).toBe(56);
}

/** Compare the actual stable controls, not an invented minimum amount of session space. */
async function positions(page: Page) {
  return page.evaluate(() => {
    const rect = (selector: string) => {
      const node = document.querySelector(selector);
      if (!node) throw new Error(`missing layout premise: ${selector}`);
      const box = node.getBoundingClientRect();
      return { x: box.x, y: box.y, width: box.width, height: box.height };
    };
    return {
      icons: [...document.querySelectorAll(".host-rail-avatar .host-kind-icon")].map((node) => {
        const box = node.getBoundingClientRect();
        return {
          x: box.x, y: box.y, width: box.width, height: box.height,
          glyph: node.getAttribute("data-glyph"), color: getComputedStyle(node).color,
        };
      }),
      footer: rect(".host-rail-footer"),
      hosts: rect(".hosts-panel-toggle"),
      settings: rect(".host-rail-footer .app-settings-toggle"),
      help: rect(".host-rail-footer .app-help-toggle"),
      footerContent: [...document.querySelectorAll(".host-rail-footer button")].map((node) => ({
        label: node.getAttribute("aria-label"), text: node.textContent,
      })),
      content: rect(".sidebar-content"),
    };
  });
}

/** Collapsed captions give a legend without widening the rail or replacing full names elsewhere. */
test("short host captions preserve aliases, abbreviate FQDNs and truncate inside the collapsed rail", async ({ page, request }) => {
  const { local } = await prepare(page, request);
  await collapse(page);
  const caption = entry(page, ALIAS_ID).locator(".host-rail-caption");
  await expect(caption).toBeVisible();
  await expect(caption).toHaveCSS("opacity", "1");
  const measured = await caption.evaluate((node) => {
    const style = getComputedStyle(node);
    const box = node.getBoundingClientRect();
    const rail = node.closest(".host-rail")!.getBoundingClientRect();
    return { scroll: node.scrollWidth, client: node.clientWidth, overflow: style.overflowX, ellipsis: style.textOverflow, left: box.left, right: box.right, railLeft: rail.left, railRight: rail.right };
  });
  expect(measured.scroll, "fixture: the alias genuinely exceeds the caption's available width").toBeGreaterThan(measured.client);
  expect(measured.overflow).toBe("hidden");
  expect(measured.ellipsis).toBe("ellipsis");
  expect(measured.left).toBeGreaterThanOrEqual(measured.railLeft);
  expect(measured.right).toBeLessThanOrEqual(measured.railRight);
  for (const [id, name] of [[local, "Local (this machine)"], [ALIAS_ID, ALIAS], [FQDN_ID, FQDN_DESTINATION], [UNRECOGNIZED_ID, UNRECOGNIZED_NAME]] as const) {
    const host = entry(page, id);
    await expect(host.locator(".host-rail-caption")).toHaveAttribute("aria-hidden", "true");
    const button = host.locator(".host-rail-avatar");
    const label = await button.getAttribute("aria-label");
    expect(label?.startsWith(`${name}: `), "the accessible name retains the display name and host state").toBe(true);
    await expect(button).toHaveAccessibleName(label!);
    await expect(button).toHaveAttribute("data-tooltip", label!);
  }
  await test.info().attach("short-host-captions", { body: await page.locator(".app-sidebar").screenshot(), contentType: "image/png" });
  await showHostsPanel(page);
  await expect(page.locator(`[data-host-id="${FQDN_ID}"] .host-name`)).toHaveText(FQDN_DESTINATION);
  await expect(page.locator(`[data-host-id="${ALIAS_ID}"] .host-name`)).toHaveText(ALIAS);
  await expect(page.locator(`[data-host-id="${local}"] .host-name`)).toHaveText("Local (this machine)");
});

/**
 * Hover exposes names without moving controls. Only the narrow rail owns
 * pointer interaction: crossing into its names collapses it, and selecting a
 * host with the mouse must leave the session list's sort control reachable.
 */
test("short host rail hover keeps icons fixed and releases the list after a mouse host choice", async ({ page, request }) => {
  const { local } = await prepare(page, request);
  await collapse(page);
  const before = await positions(page);
  expect(before.icons.length).toBeGreaterThanOrEqual(3);
  expect(before.footer.width).toBe(56);
  const rail = page.locator(".host-rail");
  const hover = page.locator(".host-rail-hover");
  await expect(hover).toHaveCSS("width", "56px");
  // The narrow rail's own padding avoids opening an unrelated avatar tooltip over
  // the expanded names, while still using the real pointer/dwell behavior.
  await hover.hover({ position: { x: 2, y: 2 } });
  await expect.poll(() => hover.evaluate((node) => node.matches(":hover"))).toBe(true);
  await expect.poll(() => rail.evaluate((node) => node.getBoundingClientRect().width)).toBeGreaterThan(56);
  await expect(entry(page, ALIAS_ID).locator(".host-rail-expanded-name")).toBeVisible();
  await expect(entry(page, ALIAS_ID).locator(".host-rail-expanded")).toHaveCSS("opacity", "1");
  await expect(entry(page, ALIAS_ID).locator(".host-rail-expanded-name")).toHaveText(ALIAS);
  await expect(entry(page, ALIAS_ID).locator(".host-rail-destination")).toHaveText(ALIAS_DESTINATION);
  await expect(entry(page, FQDN_ID).locator(".host-rail-expanded-name")).toHaveText(FQDN_DESTINATION);
  await expect(entry(page, FQDN_ID).locator(".host-rail-destination")).toHaveCount(0);
  await expect(entry(page, local).locator(".host-rail-expanded-name")).toHaveText("Local (this machine)");
  await expect(entry(page, local).locator(".host-rail-destination")).toHaveCount(0);
  await expect.poll(() => positions(page)).toEqual(before);
  const overlay = await rail.boundingBox();
  expect(overlay).not.toBeNull();
  expect(overlay!.x + overlay!.width, "the expanded rail must cover the existing session content").toBeGreaterThan(before.content.x);
  await expect(page.locator(".host-rail-all")).toContainText("All");
  await expect(page.locator(".host-rail-add")).toContainText("Add host");
  await expect(page.locator(".host-rail-all")).toHaveAccessibleName("All hosts");
  await expect(page.locator(".host-rail-add")).toHaveAccessibleName("Add host");
  await expect(page.locator(".host-rail-all .host-rail-expanded")).toHaveCSS("opacity", "1");
  await expect(page.locator(".host-rail-add .host-rail-expanded")).toHaveCSS("opacity", "1");
  await test.info().attach("expanded-host-rail", { body: await page.locator(".app-sidebar").screenshot(), contentType: "image/png" });
  const expandedName = await entry(page, ALIAS_ID).locator(".host-rail-expanded-name").boundingBox();
  const narrow = await hover.boundingBox();
  expect(expandedName).not.toBeNull();
  expect(narrow).not.toBeNull();
  const namePoint = { x: expandedName!.x + expandedName!.width / 2, y: expandedName!.y + expandedName!.height / 2 };
  expect(namePoint.x, "the crossing must enter the widened names, beyond the narrow rail").toBeGreaterThan(narrow!.x + narrow!.width);
  await page.mouse.move(namePoint.x, namePoint.y);
  await expect.poll(() => hover.evaluate((node) => node.matches(":hover"))).toBe(false);
  await expect.poll(() => rail.evaluate((node) => node.getBoundingClientRect().width)).toBe(56);

  const avatar = entry(page, local).locator(".host-rail-avatar");
  await avatar.click();
  await expect(avatar).toHaveAttribute("aria-pressed", "true");
  await expect.poll(() => hover.evaluate((node) => node.querySelector(":focus-visible") !== null)).toBe(false);
  await page.mouse.move(1279, 1);
  await expect.poll(() => rail.evaluate((node) => node.getBoundingClientRect().width)).toBe(56);
  await page.locator(".sort-title").click();
  const sort = page.getByRole("menu", { name: "Sort", exact: true });
  await expect(sort).toBeVisible();
  await sort.getByRole("menuitemradio", { name: /^Newest created(?:$|\s)/ }).click();
  await expect(page.locator(".sort-title")).toHaveAttribute("data-sort", "created");
  await expect(sort).toHaveCount(0);
  await collapse(page);
  await expect.poll(() => positions(page)).toEqual(before);
});

/**
 * Native Tab focus exposes full names without taking pointer events from the
 * sort control beneath them. Known hosts with no short form and rows absent
 * from the registry both retain their full name, for different reasons.
 */
test("short host rail keyboard expansion leaves sort usable and rows retain full-name fallbacks", async ({ page, request }) => {
  const { known, missing, unrecognized } = await prepare(page, request, true);
  await expect(page.locator(`[data-session-id="${known}"] .session-host`)).toHaveText("render-west");
  await expect(page.locator(`[data-session-id="${missing}"] .session-host`)).toHaveText(MISSING_NAME);
  await expect(page.locator(`[data-session-id="${unrecognized}"] .session-host`)).toHaveText(UNRECOGNIZED_NAME);
  await collapse(page);
  const before = await positions(page);
  const rail = page.locator(".host-rail");
  const hover = page.locator(".host-rail-hover");
  const avatar = entry(page, ALIAS_ID).locator(".host-rail-avatar");
  // Seed the adjacent ordinary button only as a focus origin. Shift+Tab,
  // rather than a scripted focus call, must establish keyboard-visible focus.
  const nextAvatar = entry(page, FQDN_ID).locator(".host-rail-avatar");
  await nextAvatar.focus();
  await expect(nextAvatar).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(avatar).toBeFocused();
  await expect.poll(() => avatar.evaluate((node) => node.matches(":focus-visible"))).toBe(true);
  await expect.poll(() => hover.evaluate((node) => ({ hover: node.matches(":hover"), keyboard: node.querySelector(":focus-visible") !== null })))
    .toEqual({ hover: false, keyboard: true });
  await expect.poll(() => rail.evaluate((node) => node.getBoundingClientRect().width)).toBeGreaterThan(56);
  await expect(entry(page, ALIAS_ID).locator(".host-rail-expanded-name")).toBeVisible();
  await expect(entry(page, ALIAS_ID).locator(".host-rail-expanded")).toHaveCSS("opacity", "1");
  await expect(entry(page, ALIAS_ID).locator(".host-rail-destination")).toHaveText(ALIAS_DESTINATION);
  expect(await entry(page, ALIAS_ID).locator(".host-rail-expanded-name").evaluate((node) => !!node.closest('[aria-hidden="true"]')))
    .toBe(true);
  await expect(avatar).toHaveAccessibleName((await avatar.getAttribute("aria-label"))!);
  await expect.poll(() => positions(page)).toEqual(before);
  const sortTitle = page.locator(".sort-title");
  const sortBox = await sortTitle.boundingBox();
  const overlay = await rail.boundingBox();
  const narrow = await hover.boundingBox();
  expect(sortBox).not.toBeNull();
  expect(overlay).not.toBeNull();
  expect(narrow).not.toBeNull();
  const sortPoint = { x: sortBox!.x + sortBox!.width / 2, y: sortBox!.y + sortBox!.height / 2 };
  expect(sortPoint.x, "the sort click must lie beyond the narrow interactive rail").toBeGreaterThan(narrow!.x + narrow!.width);
  expect(sortPoint.x, "the expanded rail must actually overlap the tested sort click").toBeLessThan(overlay!.x + overlay!.width);
  expect(sortPoint.y).toBeGreaterThan(overlay!.y);
  expect(sortPoint.y).toBeLessThan(overlay!.y + overlay!.height);
  await page.mouse.move(sortPoint.x, sortPoint.y);
  await expect(avatar).toBeFocused();
  await expect.poll(() => avatar.evaluate((node) => node.matches(":focus-visible"))).toBe(true);
  await expect.poll(() => rail.evaluate((node) => node.getBoundingClientRect().width)).toBeGreaterThan(56);
  // This ordinary click must hit the control while the overlay is still
  // painted above it; forcing a click would conceal the original regression.
  await sortTitle.click();
  const sort = page.getByRole("menu", { name: "Sort", exact: true });
  await expect(sort).toBeVisible();
  await sort.getByRole("menuitemradio", { name: /^Newest created(?:$|\s)/ }).click();
  await expect(sortTitle).toHaveAttribute("data-sort", "created");
  await expect(sort).toHaveCount(0);
  await collapse(page);
});

/**
 * Bare narrow-rail padding and gaps must retain native wheel scrolling. The
 * widened nav is pointer-transparent, so only an inner hit area gives those
 * gestures the nav as a scroll ancestor; scrolling over a button misses that
 * distinction. The session list remains an independent scroll owner.
 */
test("host rail scrolls from padding and gaps", async ({ page, request }) => {
  await prepare(page, request);
  await page.setViewportSize({ width: 1280, height: 420 });
  await collapse(page);
  const rail = page.locator(".host-rail");
  const content = page.locator(".sidebar-content");
  await expect.poll(() => rail.evaluate((node) => node.scrollHeight - node.clientHeight), {
    message: "the short viewport must genuinely overflow the real and naming-fixture host stack",
  }).toBeGreaterThan(80);
  const contentTop = await content.evaluate((node) => node.scrollTop);

  for (const region of ["side padding", "entry gap"] as const) {
    await expect.poll(() => rail.evaluate((node) => node.scrollTop), {
      message: `${region}: start from the top so All and the first host establish the input geometry`,
    }).toBe(0);
    const point = await rail.evaluate((node, region) => {
      const narrow = node.querySelector(".host-rail-hover")!;
      const all = node.querySelector(".host-rail-all")!.getBoundingClientRect();
      const first = node.querySelector(".host-rail-entry")!.getBoundingClientRect();
      const inner = narrow.getBoundingClientRect();
      const box = node.getBoundingClientRect();
      const x = region === "side padding" ? inner.left + 2 : all.left + all.width / 2;
      const y = region === "side padding" ? all.top + all.height / 2 : (all.bottom + first.top) / 2;
      return { x, y, gap: first.top - all.bottom, left: box.left, right: box.right, top: box.top, bottom: box.bottom };
    }, region);
    expect(point.gap, "the fixture must contain an actual gap between All and the first host").toBeGreaterThan(0);
    expect(point.x).toBeGreaterThan(point.left);
    expect(point.x).toBeLessThan(point.right);
    expect(point.y).toBeGreaterThan(point.top);
    expect(point.y).toBeLessThan(point.bottom);
    await page.mouse.move(point.x, point.y);
    await expect.poll(() => page.evaluate(({ x, y }) => {
      const hit = document.elementFromPoint(x, y);
      let owner = hit;
      while (owner && !["auto", "scroll"].includes(getComputedStyle(owner).overflowY)) owner = owner.parentElement;
      return {
        narrow: hit === document.querySelector(".host-rail-hover"),
        button: !!hit?.closest("button"),
        owner: owner?.matches("nav.host-rail") ?? false,
        name: owner?.getAttribute("aria-label"),
        keyboard: document.querySelector(".host-rail :focus-visible") !== null,
      };
    }, point), { message: `${region}: a bare narrow hit target must belong to the named rail scroller` })
      .toEqual({ narrow: true, button: false, owner: true, name: "Hosts", keyboard: false });
    await page.mouse.wheel(0, 80);
    await expect.poll(() => rail.evaluate((node) => node.scrollTop), {
      message: `${region}: native wheel input must scroll the host rail`,
    }).toBeGreaterThan(0);
    await expect.poll(() => content.evaluate((node) => node.scrollTop), {
      message: `${region}: the same wheel must leave session content where it was`,
    }).toBe(contentTop);

    // Reset through the same native input path; assigning scrollTop would
    // conceal a broken hit target before the second region is exercised.
    await page.mouse.wheel(0, -10_000);
    await expect.poll(() => rail.evaluate((node) => node.scrollTop)).toBe(0);
  }
});
