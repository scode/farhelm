/**
 * Dialog presentation uses controlled archive facts and the shipped renderer.
 * No directories are archived or deleted: route fixtures make long lists and
 * confirmations inexpensive while the helm, modal isolation and CSS stay real.
 */
import { expect, test } from "./helpers/evidence";
import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { localHostId, patchPreferences, readPreferences, stubFeed } from "./helpers/fleet";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";
import { expectKeyboardScroll, expectKeyboardScrollFromControl } from "./helpers/keyboard-scroll";

installTerminalSuiteHooks();

/**
 * A pending measurement disables every action inside the retained list, so no
 * child can accidentally supply keyboard access for its scroll owner. Tab from
 * the header close must still reach and scroll that overflowing list in
 * WebKit as well as Chromium; no pointer or script scroll substitutes for it.
 * Once measurement restores real actions, the redundant body stop disappears
 * without taking focus from footer Close. Tab then follows the visible footer
 * actions, wraps through header close and enters the first enabled body action.
 */
test("trash list stays keyboard-scrollable while every list action is disabled", async ({ page, request }) => {
  await stubArchives(page, request, ["keyboard-local"], 20);
  let release!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  let measuring = false;
  await page.route("**/api/checkout-trash/list", async (route) => {
    if (route.request().postDataJSON().measure_sizes) {
      measuring = true;
      await held;
    }
    await route.fallback();
  });
  try {
    await page.setViewportSize({ width: 360, height: 400 });
    await page.goto("/");
    await expect(page.locator(".checkout-trash-button")).toHaveAttribute("data-count", "20");
    await page.locator(".checkout-trash-button").click();
    const dialog = page.locator(".checkout-trash-dialog");
    await expect.poll(() => measuring, { message: "measurement remains held during keyboard input" }).toBe(true);
    await expect(dialog.locator(".checkout-trash-entry")).toHaveCount(20);
    const region = dialog.locator(".checkout-trash-body");
    await expect(region).toHaveAccessibleName("Archived checkouts");
    const actions = region.getByRole("button");
    expect(await actions.count(), "the list contains disabled actions, not an empty placeholder").toBeGreaterThan(0);
    for (const action of await actions.all()) await expect(action).toBeDisabled();
    await expect(dialog.locator(".trash-close")).toBeFocused();
    await expectKeyboardScroll(page, dialog.locator(".trash-header-close"), region);
    for (const action of await actions.all()) await expect(action).toBeDisabled();
    const close = dialog.locator(".trash-close");
    await close.focus();
    await expect(close).toBeFocused();
    release();
    for (const action of await actions.all()) await expect(action).toBeEnabled();
    await expect(region).not.toHaveAttribute("tabindex");
    await expect(region).not.toHaveAttribute("role");
    await expect(region).not.toHaveAttribute("aria-label");
    // The shared recovery can defer two frames. Cross that boundary before
    // claiming that withdrawing the old stop preserved the chosen footer.
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
    await expect(close).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(dialog.locator(".trash-delete-all")).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(dialog.locator(".trash-header-close")).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(dialog.locator(".trash-host-delete").first()).toBeFocused();
    await page.keyboard.press("Home");
    await expect.poll(() => region.evaluate((node) => node.scrollTop)).toBe(0);
    await page.keyboard.press("PageDown");
    await expect.poll(() => region.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
  } finally {
    release();
    await page.unrouteAll({ behavior: "wait" });
  }
});

/**
 * Keep the real local identity and add only display-only connected remotes.
 * Per-ID removal keeps refreshes consistent with both single and bulk cleanup;
 * reachability and registry failures are controlled only at the API boundary.
 */
async function stubArchives(page: Page, request: APIRequestContext, names: string[], count = 12) {
  const local = await localHostId(request);
  const ids = [local, ...names.slice(1).map((_, i) => 9020 + i)];
  // Successful deletion changes later listings too; final refresh must not
  // resurrect archives just because the route fixture forgot the mutation.
  const removed = new Map<number, Set<string>>();
  // Keep read calls observable: progress must not obtain fresh listing bytes
  // until all selected hosts have finished their queues.
  const listings: number[] = [];
  const issues = new Map<number, {id: string; path: string; message: string}[]>();
  const unavailable = new Set<number>();
  const registry = { fail: false };
  await page.route("**/api/hosts", async (route) => {
    if (registry.fail) {
      return fulfillAsHelm(route, { status: 503, contentType: "text/plain", body: "fixture host refresh unavailable" });
    }
    const response = await route.fetch();
    const body = await response.json();
    const original = body.hosts.find((host: any) => host.id === local);
    expect(original, "the real local host supplies identity and connection metadata").toBeTruthy();
    body.hosts = [{ ...original, name: names[0] }, ...names.slice(1).map((name, i) => ({
      id: ids[i + 1], kind: "ssh", destination: `trash-${i}.example.invalid`, name,
      identity: `trash-layout-${i}`, incarnation: 1, remote_farhelm: null, remote_state_dir: null,
      state: { phase: "connected", identity: `trash-layout-${i}`, build_version: "0.1.0",
        refresh: { status: "ok", sessions: 0 } },
    }))];
    for (const host of body.hosts) {
      if (unavailable.has(host.id)) host.state = { phase: "connecting", attempt: 1, last_error: "fixture unreachable" };
    }
    await route.fulfill({ response, json: body });
  });
  await page.route("**/api/checkout-trash/list", async (route) => {
    const { host } = route.request().postDataJSON();
    listings.push(host);
    expect(ids, "only hosts supplied by the fixture are listed").toContain(host);
    expect(unavailable.has(host), "unreachable hosts must not be listed").toBe(false);
    await fulfillAsHelm(route, { json: { issues: issues.get(host) ?? [], checkouts: Array.from({ length: count }, (_, i) => ({
      id: `archive-${host}-${i}`, name: `archived-checkout-${i}`, repository: "example/repository",
      path: `/work/checkouts/.farhelm-trash/archive-${i}`,
      archived_at: 1_700_000_000 + i, bytes: 1024,
    })).filter((entry) => !removed.get(host)?.has(entry.id)) } });
  });
  return { ids, removed, unavailable, registry, listings, issues };
}

for (const fails of [false, true]) {
  /** A held single reply proves immediate selection and busy admission; later
   * listings must preserve neighbours on the same host and on another host. */
  test(`one trash entry deletes immediately and preserves neighbours after ${fails ? "failure" : "success"}`, async ({page, request}) => {
    const {ids, removed, unavailable, issues} = await stubArchives(page, request, ["single-local", "single-remote", "single-down"], 2);
    unavailable.add(ids[2]);
    issues.set(ids[1], [{id: "unconfirmed-archive", path: "/work/trash/unconfirmed", message: "fixture archive identity unconfirmed"}]);
    let release!: () => void;
    const held = new Promise<void>((resolve) => { release = resolve; });
    const calls: {host: number; ids: string[]; expected_incarnation: number}[] = [];
    await page.route("**/api/checkout-trash/delete", async (route) => {
      const selected = route.request().postDataJSON();
      calls.push(selected);
      await held;
      if (fails) await fulfillAsHelm(route, {status: 503, contentType: "text/plain", body: "fixture single delete unavailable"});
      else {
        removed.set(selected.host, new Set(selected.ids));
        await fulfillAsHelm(route, {json: {removed: selected.ids, issues: []}});
      }
    });
    try {
      await page.setViewportSize({width: 360, height: 500});
      await page.goto("/");
      const trigger = page.locator(".checkout-trash-button");
      await expect(trigger).toHaveAttribute("data-count", "4");
      await trigger.click();
      const dialog = page.locator(".checkout-trash-dialog");
      const local = dialog.locator(`.checkout-trash-host[data-host-id="${ids[0]}"]`);
      const remote = dialog.locator(`.checkout-trash-host[data-host-id="${ids[1]}"]`);
      const down = dialog.locator(`.checkout-trash-host[data-host-id="${ids[2]}"]`);
      await expect(local.locator(".checkout-trash-entry")).toHaveCount(2);
      await expect(remote.locator(".checkout-trash-entry")).toHaveCount(2);
      const diagnostic = remote.locator(".checkout-trash-issue");
      await expect(diagnostic).toContainText("fixture archive identity unconfirmed");
      const diagnosticBefore = await diagnostic.innerText();
      await expect(down).toContainText("Not reachable");
      await expect(down.locator(".trash-entry-delete")).toHaveCount(0);
      const selected = local.getByRole("button", {name: "Delete archived-checkout-0 permanently", exact: true});
      await expect(selected).toBeEnabled();
      await selected.scrollIntoViewIfNeeded();
      await expectExposed(selected, dialog.locator(".checkout-trash-body"));
      await selected.click();
      await expect.poll(() => calls.length, {message: "single request reaches held reply"}).toBe(1);
      expect(calls[0].host).toBe(ids[0]);
      expect(calls[0].ids).toEqual([`archive-${ids[0]}-0`]);
      expect(calls[0].expected_incarnation).toBeGreaterThan(0);
      await expect(dialog.locator(".checkout-trash-confirm")).toHaveCount(0);
      await expect(local.locator(".trash-host-progress")).toHaveText("0 of 1 deleted");
      await expect(dialog.locator(".checkout-trash-progress")).toHaveText("Deleting… 1 checkout left");
      const current = local.locator(".checkout-trash-entry").filter({hasText: "archived-checkout-0"});
      await expect(current.locator(".trash-entry-progress .delete-spinner")).toBeVisible();
      await expect(local.locator(".trash-entry-waiting")).toHaveCount(0);
      await expect(local.locator(".checkout-trash-entry").filter({hasText: "archived-checkout-1"})
        .locator(".trash-entry-progress, .trash-entry-waiting")).toHaveCount(0);
      await expect(remote.locator(".trash-entry-progress, .trash-entry-waiting")).toHaveCount(0);
      await expect(diagnostic).toHaveText(diagnosticBefore, {useInnerText: true});
      await expect(diagnostic.locator(".trash-entry-progress, .trash-entry-waiting")).toHaveCount(0);
      await expect(dialog.locator(".trash-close-help")).toHaveAttribute("data-tooltip", "Available when deletion finishes.");
      for (const control of await dialog.locator(".trash-entry-delete, .trash-host-delete, .trash-delete-all, .trash-close").all()) {
        await expect(control).toBeDisabled();
      }
      // Disabling the clicked button can release focus. Put Escape inside the
      // dialog explicitly so this proves its deletion guard, not a missed event.
      await dialog.focus();
      await expect(dialog).toBeFocused();
      await page.keyboard.press("Escape");
      await expect(dialog).toBeVisible();
      await expect(local.locator(".trash-host-progress")).toHaveText("0 of 1 deleted");
      release();
      await expect(dialog.locator(".checkout-trash-progress")).toHaveText("");
      await expect(dialog.locator(".trash-close")).toBeFocused();
      await expect(local.locator(".checkout-trash-entry")).toHaveCount(fails ? 2 : 1);
      await expect(local.locator(".checkout-trash-entry").filter({hasText: "archived-checkout-1"})).toHaveCount(1);
      await expect(remote.locator(".checkout-trash-entry")).toHaveCount(2);
      await expect(trigger).toHaveAttribute("data-count", fails ? "4" : "3");
      await expect(local.locator(".trash-entry-delete").last()).toBeEnabled();
      if (fails) await expect(local.locator(".checkout-trash-issue")).toContainText("cleanup reply could not be confirmed");
      expect(calls).toHaveLength(1);
    } finally {
      release();
      await page.unrouteAll({behavior: "wait"});
    }
  });
}

/** Retained archives are display evidence, not deletion authority, while a
 * measurement is pending or a later registry read cannot confirm reachability. */
test("single trash delete requires settled listings and a healthy host refresh", async ({page, request}) => {
  const {registry} = await stubArchives(page, request, ["eligibility-local"], 2);
  const feed = await stubFeed(page);
  let release!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  let measured = false;
  await page.route("**/api/checkout-trash/list", async (route) => {
    if (route.request().postDataJSON().measure_sizes) {
      measured = true;
      await held;
    }
    await route.fallback();
  });
  try {
    await page.goto("/");
    await feed.waitForConnection(1);
    await expect(page.locator(".checkout-trash-button")).toHaveAttribute("data-count", "2");
    await page.locator(".checkout-trash-button").click();
    await expect.poll(() => measured, {message: "dialog measurement is held"}).toBe(true);
    const dialog = page.locator(".checkout-trash-dialog");
    const buttons = dialog.locator(".trash-entry-delete");
    await expect(buttons).toHaveCount(2);
    for (const button of await buttons.all()) await expect(button).toBeDisabled();
    release();
    await expect(buttons.first()).toBeEnabled();
    registry.fail = true;
    feed.notify(1);
    await expect(dialog.locator(".checkout-trash-issue").filter({hasText: "Host reachability could not be refreshed"})).toBeVisible();
    for (const button of await buttons.all()) await expect(button).toBeDisabled();
  } finally {
    release();
    await page.unrouteAll({behavior: "wait"});
  }
});

/**
 * An entry delete has no confirmation, so a click aimed at one row must not
 * delete the row that moved under the pointer. The dialog's own delete on one
 * host re-reads every host, and the re-read inserts an archive above the row
 * that was aimed at. A click at its old spot right after the move does nothing; the
 * same click once the pause ends deletes the row now drawn there, so the pause,
 * not a missed click, kept the first one inert. The page clock is paused so the
 * 700 ms pause is crossed explicitly rather than raced.
 */
test("trash entry deletes pause after rows move under the pointer", async ({page, request}) => {
  await page.clock.install();
  const {ids, removed} = await stubArchives(page, request, ["settle-local", "settle-remote"], 2);
  let inserted = false;
  await page.route("**/api/checkout-trash/list", async (route) => {
    const {host} = route.request().postDataJSON();
    if (!inserted || host !== ids[0]) return route.fallback();
    await fulfillAsHelm(route, {json: {issues: [], checkouts: ["new", 0, 1].map((suffix) => ({
      id: `archive-${host}-${suffix}`, name: `archived-checkout-${suffix}`, repository: "example/repository",
      path: `/work/checkouts/.farhelm-trash/archive-${suffix}`, archived_at: 1_700_000_000, bytes: 1024,
    })).filter((entry) => !removed.get(host)?.has(entry.id))}});
  });
  const calls: {host: number; ids: string[]}[] = [];
  await page.route("**/api/checkout-trash/delete", async (route) => {
    const selected = route.request().postDataJSON();
    calls.push(selected);
    removed.set(selected.host, new Set([...(removed.get(selected.host) ?? []), ...selected.ids]));
    await fulfillAsHelm(route, {json: {removed: selected.ids, issues: []}});
  });
  await page.setViewportSize({width: 1000, height: 700});
  await page.goto("/");
  await page.locator(".checkout-trash-button").click();
  const dialog = page.locator(".checkout-trash-dialog");
  const local = dialog.locator(`.checkout-trash-host[data-host-id="${ids[0]}"]`);
  const remote = dialog.locator(`.checkout-trash-host[data-host-id="${ids[1]}"]`);
  await expect(local.locator(".checkout-trash-entry")).toHaveCount(2);
  // Every finished re-read starts a pause; let them all land before freezing time.
  await expect(dialog).not.toContainText("Measuring…");
  await page.clock.pauseAt(new Date(Date.now() + 5_000));
  await page.clock.runFor(700);
  const aimed = local.getByRole("button", {name: "Delete archived-checkout-0 permanently", exact: true});
  await expect(aimed).toBeEnabled();
  const box = (await aimed.boundingBox())!;
  const point = {x: box.x + box.width / 2, y: box.y + box.height / 2};

  inserted = true;
  await remote.getByRole("button", {name: "Delete archived-checkout-0 permanently", exact: true}).click();
  await expect.poll(() => calls.length, {message: "the remote delete was sent"}).toBe(1);
  await expect(local.locator(".checkout-trash-entry")).toHaveCount(3);
  await expect(dialog).not.toContainText("Measuring…");
  const under = () => page.evaluate(({x, y}) => document.elementFromPoint(x, y)?.closest("button")?.getAttribute("aria-label"), point);
  expect(await under(), "premise: the inserted row now sits under the pointer").toBe("Delete archived-checkout-new permanently");
  const moved = local.getByRole("button", {name: "Delete archived-checkout-new permanently", exact: true});
  await expect(moved).toBeDisabled();
  await page.mouse.click(point.x, point.y);
  await page.clock.runFor(700);
  await expect(moved).toBeEnabled();
  expect(calls, "the click during the pause deleted nothing").toHaveLength(1);
  await page.mouse.click(point.x, point.y);
  await expect.poll(() => calls.length, {message: "the same click deletes once settled"}).toBe(2);
  expect(calls[1]).toMatchObject({host: ids[0], ids: [`archive-${ids[0]}-new`]});
});

/**
 * Each gate owns one response boundary. Releasing every gate in teardown also
 * admits requests queued after a failed assertion, while the page is still live.
 */
function deletionGate() {
  let release!: () => void;
  const ready = new Promise<void>((resolve) => { release = resolve; });
  return {ready, release};
}

for (const action of ["global", "host"] as const) {
  /**
   * A bulk selection must expose real per-checkout progress, including a refusal
   * between two successes. Counts reflect confirmed removals, not submitted IDs;
   * an issue remains after the next success and the final listing refresh. The
   * held first response crosses the old 60-second deadline under Clock, so the
   * elapsed indicator and extended request wait have observable coverage.
   */
  test(`${action} trash cleanup advances one checkout at a time with ${action === "host" ? "reduced" : "normal"} motion`, async ({page, request}) => {
    await page.clock.install();
    const reduced = action === "host";
    await page.emulateMedia({reducedMotion: reduced ? "reduce" : "no-preference"});
    const {ids, removed, listings} = await stubArchives(page, request, ["sequence-local"], 3);
    const host = ids[0];
    const gates = Array.from({length: 3}, deletionGate);
    const calls: {host: number; ids: string[]; expected_incarnation: number}[] = [];
    const issue = {id: `archive-${host}-1`, path: "/work/trash/refused", message: "fixture archive remains in use"};
    await page.route("**/api/checkout-trash/delete", async (route) => {
      const selected = route.request().postDataJSON();
      const index = calls.length;
      calls.push(selected);
      expect(selected.host).toBe(host);
      expect(selected.ids, "every request contains one frozen checkout").toHaveLength(1);
      expect(selected.expected_incarnation).toBeGreaterThan(0);
      expect(index, "a three-checkout selection cannot dispatch a fourth request").toBeLessThan(3);
      await gates[index].ready;
      if (index === 1) {
        await fulfillAsHelm(route, {json: {removed: [], issues: [{...issue, id: selected.ids[0]}]}});
      } else {
        removed.set(host, new Set([...(removed.get(host) ?? []), ...selected.ids]));
        await fulfillAsHelm(route, {json: {removed: selected.ids, issues: []}});
      }
    });
    try {
      await page.goto("/");
      const trigger = page.locator(".checkout-trash-button");
      await expect(trigger).toHaveAttribute("data-count", "3");
      await trigger.click();
      const dialog = page.locator(".checkout-trash-dialog");
      const section = dialog.locator(`.checkout-trash-host[data-host-id="${host}"]`);
      const entries = section.locator(".checkout-trash-entry");
      const close = dialog.locator(".trash-close");
      const headerClose = dialog.locator(".trash-header-close");
      const total = dialog.locator(".checkout-trash-total");
      const status = dialog.locator(".dialog-footer .checkout-trash-progress");
      const start = action === "global" ? dialog.locator(".trash-delete-all") : section.locator(".trash-host-delete");
      await expect(entries).toHaveCount(3);
      await expect(start).toBeEnabled();
      await expect(dialog).not.toContainText("Measuring…");
      await expect(total).toHaveText("3 checkouts · 3.0 KiB on reachable hosts");
      await expect(status).toHaveText("");
      await expect(status).toHaveAttribute("role", "status");
      const mountedStatus = await status.elementHandle();
      const icons = [trigger.locator(".trash-delete-icon"), dialog.locator(".checkout-trash-header .trash-delete-icon")];
      for (const icon of icons) await expect(icon).not.toHaveClass(/\bdeleting\b/);
      // Freeze only after measurement and focus have settled. The existing row
      // movement pause must also expire before this controlled run starts.
      await page.clock.pauseAt(new Date(Date.now() + 5_000));
      await page.clock.runFor(700);
      const readsBefore = listings.length;
      await start.click();
      await page.clock.runFor(100);
      const confirm = dialog.locator(".checkout-trash-confirm");
      await expect(confirm).toContainText("Delete 3 archived checkouts");
      await expect(confirm.locator(".trash-confirm-submit")).toBeFocused();
      await page.keyboard.press("Enter");
      await expect.poll(() => calls.length, {message: "the first checkout response is held"}).toBe(1);
      const firstName = calls[0].ids[0].split("-").at(-1)!;
      const first = entries.filter({hasText: `archived-checkout-${firstName}`});
      await expect(first.locator(".trash-entry-progress .delete-spinner")).toBeVisible();
      await expect(first.locator(".trash-entry-elapsed")).toHaveText("0s");
      // A pending delete is the only state that mounts this technical age.
      // Require the bundled mono face, not merely tabular digits in Inter.
      await expect.poll(() => first.locator(".trash-entry-elapsed").evaluate(async (element) => {
        const style = getComputedStyle(element);
        if (style.fontFamily.split(",")[0].replace(/["']/g, "").trim() !== "JetBrains Mono") return false;
        const faces = await document.fonts.load(`${style.fontWeight} ${style.fontSize} "JetBrains Mono"`, element.textContent || "0s");
        return faces.length > 0 && faces.every((face) => face.status === "loaded");
      }), { message: "pending cleanup elapsed time uses the loaded mono face" }).toBe(true);
      await expect(section.locator(".trash-entry-waiting")).toHaveCount(2);
      for (const waiting of await section.locator(".trash-entry-waiting").all()) await expect(waiting).toHaveText("Waiting");
      await expect(section.locator(".trash-host-progress")).toHaveText("0 of 3 deleted");
      await expect(status).toHaveText("Deleting… 3 checkouts left");
      await expect(total).toHaveText("3 checkouts · 3.0 KiB on reachable hosts");
      await expect(trigger).toHaveAttribute("data-count", "3");
      await expect(close).toBeDisabled();
      await expect(headerClose).toBeDisabled();
      // The explanation must be reachable over a genuinely disabled control,
      // including WebKit's native disabled-button pointer suppression.
      for (const closeHelp of [dialog.locator(".trash-close-help"), dialog.locator(".trash-header-close-help")]) {
        await expect(closeHelp).toHaveAttribute("data-tooltip", "Available when deletion finishes.");
        await closeHelp.hover();
        await page.clock.runFor(400);
        await expect(page.locator("body > .farhelm-tooltip")).toHaveText("Available when deletion finishes.");
        await expect(page.locator("body > .farhelm-tooltip")).toBeVisible();
        await page.mouse.move(0, 0);
      }
      for (const icon of icons) {
        await expect(icon).toHaveClass(/\bdeleting\b/);
        await expect(icon).toHaveCSS("animation-name", reduced ? "none" : "trash-delete-bob");
        if (!reduced) {
          await expect(icon).toHaveCSS("animation-timing-function", /^(steps\(|step-start|step-end)/);
          await expect(icon).toHaveCSS("animation-iteration-count", "infinite");
          const offsets = await icon.evaluate((node) => {
            const effect = node.getAnimations().find((animation) => animation instanceof CSSAnimation &&
              animation.animationName === "trash-delete-bob")?.effect as KeyframeEffect | null;
            return effect?.getKeyframes().map((frame) => new DOMMatrixReadOnly(String(frame.transform)).f) ?? [];
          });
          expect(new Set(offsets).size, "the active animation moves the flipped can vertically").toBeGreaterThan(1);
        }
        const upsideDown = await icon.evaluate((node) => {
          const matrix = new DOMMatrixReadOnly(getComputedStyle(node).transform);
          return [matrix.a, matrix.b, matrix.c, matrix.d];
        });
        expect(upsideDown[0]).toBeCloseTo(-1);
        expect(upsideDown[1]).toBeCloseTo(0);
        expect(upsideDown[2]).toBeCloseTo(0);
        expect(upsideDown[3]).toBeCloseTo(-1);
      }
      await dialog.focus();
      await expect(dialog).toBeFocused();
      await page.keyboard.press("Escape");
      await expect(dialog).toBeVisible();
      await page.clock.fastForward(61_000);
      await expect(first.locator(".trash-entry-elapsed")).toHaveText("61s");
      await expect(first.locator(".delete-spinner")).toBeVisible();
      expect(calls, "the held request neither times out at 60s nor starts its neighbour").toHaveLength(1);
      expect(listings, "held deletes do not trigger speculative listing refreshes").toHaveLength(readsBefore);

      // The shared phone footer consumes two answer rows and a progress row.
      // Keep the list's keyboard entry usable even at the shortest supported
      // stress frame, while both dismissal controls remain truly disabled.
      await page.setViewportSize({width: 360, height: 300});
      await page.clock.runFor(50);
      const body = dialog.locator(".checkout-trash-body");
      expect(await body.evaluate(node => node.clientHeight), "busy phone list retains a usable scrollport").toBeGreaterThanOrEqual(32);
      for (const control of [dialog.locator(".trash-close-help"), dialog.locator(".trash-header-close-help"), status]) {
        await expectExposed(control);
      }
      await test.info().attach("trash-busy-phone-list-height", {
        body: JSON.stringify({viewport: page.viewportSize(), listHeight: await body.evaluate(node => node.clientHeight)}),
        contentType: "application/json",
      });

      gates[0].release();
      await expect.poll(() => calls.length, {message: "the second request follows the first confirmed reply"}).toBe(2);
      await expect(first).toHaveCount(0);
      await expect(entries).toHaveCount(2);
      await expect(section.locator(".trash-host-progress")).toHaveText("1 of 3 deleted");
      await expect(section.locator(".checkout-trash-host-header")).toContainText("2 checkouts · 2.0 KiB");
      await expect(total).toHaveText("2 checkouts · 2.0 KiB on reachable hosts");
      await expect(trigger).toHaveAttribute("data-count", "2");
      await expect(status).toHaveText("Deleting… 2 checkouts left");
      await expect(section.locator(".trash-entry-elapsed")).toHaveText("0s");
      await expect(section.locator(".trash-entry-waiting")).toHaveText("Waiting");
      expect(new Set(calls.flatMap((call) => call.ids)).size, "success advances to a different selected checkout").toBe(2);
      expect(listings).toHaveLength(readsBefore);

      gates[1].release();
      await expect.poll(() => calls.length, {message: "a per-record issue does not stop this host's queue"}).toBe(3);
      await expect(section.locator(".checkout-trash-issue")).toContainText(issue.message);
      await expect(section.locator(".trash-host-progress")).toHaveText("1 of 3 deleted");
      await expect(status).toHaveText("Deleting… 1 checkout left");
      await expect(section.locator(".trash-entry-progress .delete-spinner")).toHaveCount(1);
      await expect(section.locator(".trash-entry-waiting")).toHaveCount(0);
      await expect(total).toHaveText("2 checkouts · 2.0 KiB on reachable hosts");
      await expect(trigger).toHaveAttribute("data-count", "2");
      expect(listings, "an issue reply still does not cause a mid-run refresh").toHaveLength(readsBefore);
      gates[2].release();
      await expect(status).toHaveText("");
      await page.clock.runFor(100);
      await expect(close).toBeFocused();
      await expect(close).toBeEnabled();
      await expect(headerClose).toBeEnabled();
      for (const closeHelp of [dialog.locator(".trash-close-help"), dialog.locator(".trash-header-close-help")]) {
        await expect(closeHelp).not.toHaveAttribute("data-tooltip", "Available when deletion finishes.");
      }
      await expect(total).toHaveText("1 checkout · 1.0 KiB on reachable hosts");
      await expect(trigger).toHaveAttribute("data-count", "1");
      await expect(entries).toHaveCount(1);
      await expect(section.locator(".checkout-trash-issue")).toContainText(issue.message);
      await expect(dialog.locator(".trash-host-progress, .trash-entry-progress, .trash-entry-waiting")).toHaveCount(0);
      for (const icon of icons) {
        await expect(icon).not.toHaveClass(/\bdeleting\b/);
        await expect(icon).toHaveCSS("animation-name", "none");
      }
      expect(calls.flatMap((call) => call.ids).sort()).toEqual([0, 1, 2].map((index) => `archive-${host}-${index}`));
      await expect.poll(() => listings.length, {message: "one final listing refresh follows the whole run"}).toBe(readsBefore + 1);
      expect(await mountedStatus!.evaluate((node) => node.isConnected), "the live region survives start and completion").toBe(true);
    } finally {
      for (const gate of gates) gate.release();
      await page.unrouteAll({behavior: "wait"});
    }
  });
}

for (const failure of ["http", "transport"] as const) {
  /**
   * An unconfirmed reply cannot authorize later deletes on the same host. Other
   * hosts keep progressing, and only the end of their run permits a refetch of
   * both listings. A transport abort and an HTTP failure have the same safety
   * consequence even though their browser/network failure paths differ.
   */
  test(`trash cleanup stops only its own host after ${failure} failure`, async ({page, request}) => {
    const {ids, removed, listings} = await stubArchives(page, request, ["stopped-local", "continuing-remote"], 3);
    const gates = ids.map(() => Array.from({length: 3}, deletionGate));
    const calls = ids.map(() => [] as string[][]);
    await page.route("**/api/checkout-trash/delete", async (route) => {
      const selected = route.request().postDataJSON();
      const index = ids.indexOf(selected.host);
      expect(index).toBeGreaterThanOrEqual(0);
      const attempt = calls[index].length;
      calls[index].push(selected.ids);
      expect(selected.ids).toHaveLength(1);
      expect(attempt).toBeLessThan(3);
      await gates[index][attempt].ready;
      if (index === 0 && attempt === 0) {
        if (failure === "transport") await route.abort("failed");
        else await fulfillAsHelm(route, {status: 503, contentType: "text/plain", body: "fixture cleanup unavailable"});
      } else {
        removed.set(selected.host, new Set([...(removed.get(selected.host) ?? []), ...selected.ids]));
        await fulfillAsHelm(route, {json: {removed: selected.ids, issues: []}});
      }
    });
    try {
      await page.goto("/");
      const trigger = page.locator(".checkout-trash-button");
      await expect(trigger).toHaveAttribute("data-count", "6");
      await trigger.click();
      const dialog = page.locator(".checkout-trash-dialog");
      const sections = ids.map((id) => dialog.locator(`.checkout-trash-host[data-host-id="${id}"]`));
      const status = dialog.locator(".checkout-trash-progress");
      const close = dialog.locator(".trash-close");
      await expect(dialog.locator(".checkout-trash-entry")).toHaveCount(6);
      await expect(dialog.locator(".trash-delete-all")).toBeEnabled();
      await expect(dialog).not.toContainText("Measuring…");
      const readsBefore = listings.length;
      await dialog.locator(".trash-delete-all").click();
      await expect(dialog.locator(".trash-confirm-submit")).toBeFocused();
      await page.keyboard.press("Enter");
      await expect.poll(() => calls.map((host) => host.length), {message: "both hosts begin before either reply is released"}).toEqual([1, 1]);
      for (const section of sections) {
        await expect(section.locator(".trash-entry-progress .delete-spinner")).toHaveCount(1);
        await expect(section.locator(".trash-entry-waiting")).toHaveCount(2);
      }
      await expect(status).toHaveText("Deleting… 6 checkouts left");
      await expect(trigger).toHaveAttribute("data-count", "6");
      gates[0][0].release();
      await expect(sections[0].locator(".checkout-trash-issue")).toContainText("cleanup reply could not be confirmed");
      await expect(sections[0].locator(".checkout-trash-issue")).toContainText("may already");
      await expect(sections[0].locator(".trash-entry-progress, .trash-entry-waiting")).toHaveCount(0);
      await expect(sections[0].locator(".trash-host-progress")).toHaveText("0 of 3 deleted");
      await expect(status).toHaveText("Deleting… 3 checkouts left");
      await expect(trigger).toHaveAttribute("data-count", "6");
      expect(listings).toHaveLength(readsBefore);
      for (let index = 0; index < 3; index++) {
        expect(calls.map((host) => host.length), "neither host overlaps requests while the current remote reply is held").toEqual([1, index + 1]);
        await expect(sections[1].locator(".trash-host-progress")).toHaveText(`${index} of 3 deleted`);
        await expect(close).toBeDisabled();
        gates[1][index].release();
        if (index < 2) {
          await expect.poll(() => calls[1].length, {message: "the healthy host advances after each confirmed deletion"}).toBe(index + 2);
          await expect(trigger).toHaveAttribute("data-count", String(5 - index));
          await expect(status).toHaveText(`Deleting… ${2 - index} ${index === 0 ? "checkouts" : "checkout"} left`);
          expect(listings, "completed requests do not refetch while another is pending").toHaveLength(readsBefore);
        }
      }
      await expect(status).toHaveText("");
      await expect(close).toBeEnabled();
      await expect(close).toBeFocused();
      await expect(trigger).toHaveAttribute("data-count", "3");
      await expect(sections[0].locator(".checkout-trash-entry")).toHaveCount(3);
      await expect(sections[0].locator(".checkout-trash-issue")).toContainText("cleanup reply could not be confirmed");
      await expect(sections[1]).toHaveCount(0);
      expect(calls[0]).toHaveLength(1);
      expect(new Set(calls[1].flat()).size).toBe(3);
      await expect.poll(() => listings.slice(readsBefore).sort(), {message: "each connected host is refetched once after both queues settle"}).toEqual([...ids].sort());
    } finally {
      for (const host of gates) for (const gate of host) gate.release();
      await page.unrouteAll({behavior: "wait"});
    }
  });
}

for (const width of [1000, 360]) {
  for (const firstFails of [false, true]) {
    /**
     * Completed hosts retain their result while another host is still working,
     * including an uncertain cleanup result. Holding both requests exposes the
     * busy admission guard; a later retry must begin a fresh denominator.
     */
    test(`trash deletion progress survives ${firstFails ? "failure" : "success"} at width ${width}`, async ({ page, request }) => {
      const previous = await readPreferences(request);
      try {
        // Permanent checkout loss remains explicit despite the session opt-out.
        await patchPreferences(request, { skip_session_confirmations: true });
        const { ids, removed } = await stubArchives(page, request, ["progress-local", "progress-remote"], 1);
        const held = ids.map(() => {
          let release!: () => void;
          const ready = new Promise<void>((resolve) => { release = resolve; });
          return { ready, release };
        });
        let releaseRetry!: () => void;
        const retryReady = new Promise<void>((resolve) => { releaseRetry = resolve; });
        const attempts = new Map<number, number>();
        await page.route("**/api/checkout-trash/delete", async (route) => {
          const { host, ids: selected, expected_incarnation } = route.request().postDataJSON();
          const index = ids.indexOf(host);
          expect(index, "only the fixture's hosts can receive cleanup").toBeGreaterThanOrEqual(0);
          expect(selected, "cleanup freezes the rendered archive IDs").toEqual([`archive-${host}-0`]);
          expect(expected_incarnation, "cleanup retains its connected incarnation").toBeGreaterThan(0);
          const attempt = (attempts.get(host) ?? 0) + 1;
          attempts.set(host, attempt);
          await (attempt === 1 ? held[index].ready : retryReady);
          if (index === 0 && firstFails && attempt === 1) {
            await fulfillAsHelm(route, { status: 503, contentType: "text/plain", body: "fixture cleanup unavailable" });
          } else {
            removed.set(host, new Set(selected));
            await fulfillAsHelm(route, { json: { removed: selected, issues: [] } });
          }
        });
        try {
          await page.setViewportSize({ width, height: 300 });
          await page.goto("/");
          const trigger = page.locator(".checkout-trash-button");
          await expect(trigger).toHaveAttribute("data-count", "2");
          await trigger.click();
          const dialog = page.locator(".checkout-trash-dialog");
          const sections = ids.map((id) => dialog.locator(`.checkout-trash-host[data-host-id="${id}"]`));
          await expect(dialog.locator(".checkout-trash-entry")).toHaveCount(2);
          const all = dialog.locator(".trash-delete-all");
          const close = dialog.locator(".trash-close");
          await expect(all).toBeEnabled();
          await all.click();
          const confirmation = dialog.locator(".checkout-trash-confirm");
          await expect(confirmation).toContainText("Delete 2 archived checkouts");
          await expect(confirmation.locator(".trash-confirm-submit")).toBeFocused();
          // The live region must already exist, empty, before the delete starts:
          // only a text change in a mounted region is announced reliably.
          const status = dialog.locator(".checkout-trash-progress");
          await expect(status).toHaveAttribute("role", "status");
          await expect(status).toHaveText("");
          await page.keyboard.press("Enter");
          await expect.poll(() => ids.map((id) => attempts.get(id) ?? 0), { message: "both host requests reached the held boundary" }).toEqual([1, 1]);
          await expect(status).toHaveText("Deleting… 2 checkouts left");
          for (const section of sections) await expect(section.locator(".trash-host-progress")).toHaveText("0 of 1 deleted");
          await expect(close).toBeDisabled();
          await expect(dialog.locator(".trash-close-help")).toHaveAttribute("data-tooltip", "Available when deletion finishes.");
          await expect(all).toBeDisabled();
          await expect(dialog.locator(".trash-host-delete:enabled")).toHaveCount(0);
          await page.mouse.move(0, 0);
          // Disabled Close hands pointer hits to its tooltip wrapper. Check
          // that same painted area rather than demanding hits on the button.
          const closeHelp = dialog.locator(".trash-close-help");
          for (const control of [status, all, closeHelp]) await expectExposed(control);
          await test.info().attach("trash-deleting-two-hosts", { body: await page.screenshot(), contentType: "image/png" });
          // Submission removes the focused button; explicitly focus the dialog
          // before testing Escape so this input reaches its actual close guard.
          await dialog.focus();
          await expect(dialog).toBeFocused();
          await page.keyboard.press("Escape");
          await expect(dialog).toBeVisible();
          await expect(status).toHaveText("Deleting… 2 checkouts left");

          held[0].release();
          await expect(status).toHaveText("Deleting… 1 checkout left");
          await expect(sections[0].locator(".trash-host-progress")).toHaveText(firstFails ? "0 of 1 deleted" : "1 of 1 deleted");
          await expect(sections[1].locator(".trash-host-progress")).toHaveText("0 of 1 deleted");
          if (firstFails) await expect(sections[0].locator(".checkout-trash-issue")).toContainText("fixture cleanup unavailable");
          else await expect(sections[0].locator(".checkout-trash-entry")).toHaveCount(0);
          await expect(close).toBeDisabled();
          await test.info().attach("trash-deleting-one-host", { body: await page.screenshot(), contentType: "image/png" });
          // Short windows can leave the host indicator below the description.
          // Capture its actual paint while also checking the pinned live count.
          await sections[1].locator(".trash-host-progress").scrollIntoViewIfNeeded();
          await expectExposed(sections[1].locator(".trash-host-progress"));
          for (const control of [status, all, closeHelp]) await expectExposed(control);
          await test.info().attach("trash-pending-host", { body: await page.screenshot(), contentType: "image/png" });
          held[1].release();
          await expect(status).toHaveText("");
          await expect(dialog.locator(".trash-host-progress")).toHaveCount(0);
          await expect(close).toBeEnabled();
          await expect(close).toBeFocused();
          await expect(trigger).toHaveAttribute("data-count", firstFails ? "1" : "0");
          await expect(dialog.locator(".checkout-trash-entry")).toHaveCount(firstFails ? 1 : 0);
          await expect(sections[1]).toHaveCount(0);
          if (firstFails) {
            await expect(sections[0].locator(".checkout-trash-issue")).toContainText("cleanup reply could not be confirmed");
            await expect(all).toBeEnabled();
            await all.click();
            await expect(confirmation).toContainText("Delete 1 archived checkout");
            await expect(confirmation.locator(".trash-confirm-submit")).toBeFocused();
            await page.keyboard.press("Enter");
            await expect.poll(() => attempts.get(ids[0]), { message: "one-host retry reached its held boundary" }).toBe(2);
            await expect(status).toHaveText("Deleting… 1 checkout left");
            await expect(sections[0].locator(".trash-host-progress")).toHaveText("0 of 1 deleted");
            await expect(sections[1].locator(".trash-host-progress")).toHaveCount(0);
            releaseRetry();
            await expect(status).toHaveText("");
            await expect(dialog.locator(".trash-host-progress")).toHaveCount(0);
            await expect(close).toBeFocused();
            await expect(trigger).toHaveAttribute("data-count", "0");
          }
          await expect(dialog.locator(".checkout-trash-empty")).toHaveText("The trash is empty on reachable hosts.");
          await test.info().attach("trash-deletion-finished", { body: await page.screenshot(), contentType: "image/png" });
          await close.click();
          await expect(dialog).toHaveCount(0);
          await expect(trigger).toBeFocused();
        } finally {
          // Let every intercepted request finish while its page remains alive,
          // even when an earlier assertion fails at a deliberately held boundary.
          for (const host of held) host.release();
          releaseRetry();
          await page.unrouteAll({ behavior: "wait" });
        }
      } finally {
        try {
          await page.unrouteAll({ behavior: "wait" });
        } finally {
          await patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null });
        }
      }
    });
  }
}

/** Probe painted edges as well as bounds: ancestor clipping leaves the box intact. */
async function expectExposed(control: Locator, region?: Locator) {
  await expect.poll(() => control.evaluate((node) => {
    const box = node.getBoundingClientRect();
    if (box.width <= 0 || box.height <= 0 || box.top < 0 || box.bottom > innerHeight ||
      box.left < 0 || box.right > innerWidth) return ["outside viewport or empty"];
    const style = getComputedStyle(node);
    // These controls use pixel radii. Probe inside each rounded painted corner
    // so transparent corner pixels cannot masquerade as a clipping ancestor.
    const inset = (radius: string) => Math.max(1, Math.ceil(parseFloat(radius) || 0));
    const tl = inset(style.borderTopLeftRadius), tr = inset(style.borderTopRightRadius);
    const bl = inset(style.borderBottomLeftRadius), br = inset(style.borderBottomRightRadius);
    const points = [
      [box.x + box.width / 2, box.y + box.height / 2],
      [box.left + tl, box.top + tl], [box.right - tr, box.top + tr],
      [box.left + bl, box.bottom - bl], [box.right - br, box.bottom - br],
    ];
    return points.flatMap(([x, y]) => {
      const hit = document.elementFromPoint(x, y);
      return hit && node.contains(hit) ? [] : [`(${x}, ${y}) hit ${hit?.tagName}.${hit?.className}`];
    });
  }), { message: "control is inside the viewport with center and painted corners exposed" }).toEqual([]);
  if (region) {
    const outer = (await region.boundingBox())!;
    const box = (await control.boundingBox())!;
    // scrollIntoView can leave a subpixel difference at the body's boundary.
    // Allow one CSS pixel of rounding, with hit testing still proving access.
    expect(box.y).toBeGreaterThanOrEqual(outer.y - 1);
    expect(box.y + box.height).toBeLessThanOrEqual(outer.y + outer.height + 1);
  }
}

for (const width of [1000, 360]) {
  /**
   * Global actions stay visible regardless of archive-list length. Long host
   * names make the confirmation taller than the short body, so its safe and
   * destructive actions must be revealed in their own bounded region without
   * moving the pinned header/footer or consuming the archive list's entry.
   */
  test(`trash header and global confirmation remain reachable at width ${width}`, async ({ page, request }) => {
    const names = ["layout-local", ...[1, 2, 3].map((i) => `archive-host-${i}-` + "long-name-".repeat(20))];
    await stubArchives(page, request, names);
    await page.setViewportSize({ width, height: 300 });
    await page.goto("/");
    const trigger = page.locator(".checkout-trash-button");
    await expect(trigger).toHaveAttribute("data-count", "48");
    await trigger.click();
    const dialog = page.locator(".checkout-trash-dialog");
    await expect(dialog.locator(".checkout-trash-entry")).toHaveCount(48);
    const all = dialog.locator(".trash-delete-all");
    const close = dialog.locator(".trash-close");
    const headerClose = dialog.locator(".trash-header-close");
    const total = dialog.locator(".checkout-trash-total");
    await expect(all).toBeEnabled();
    await test.info().attach("trash-header-top", { body: await page.screenshot(), contentType: "image/png" });
    await expect(close).toBeFocused();
    await expect(headerClose).not.toBeFocused();
    const footer = dialog.locator(".dialog-footer");
    await expect(footer.locator(".dialog-actions button")).toHaveText(["Close", "Delete all"]);
    await expect(dialog.locator(".checkout-trash-header .trash-delete-all, .checkout-trash-header .trash-close")).toHaveCount(0);
    await expect(dialog.locator(".dialog-header .dialog-title")).toHaveText("Trash");
    await expect(footer).toHaveCount(1);
    const body = dialog.locator(".checkout-trash-body");
    expect(await body.evaluate((node) => node.scrollHeight > node.clientHeight + 1), "archives overflow the body").toBe(true);
    await page.mouse.move(0, 0);
    for (const control of [all, close, total, headerClose]) await expectExposed(control);
    const idleHeight = await body.evaluate(node => node.clientHeight);
    expect(idleHeight, "idle list has a usable scrollport").toBeGreaterThanOrEqual(32);
    if (width === 360) {
      const actions = footer.locator(".dialog-actions");
      const bounds = (await actions.boundingBox())!;
      for (const control of [close, all]) {
        expect((await control.boundingBox())!.width, "each phone action fills the shared action row").toBeCloseTo(bounds.width, 0);
      }
    }
    const tops = [(await all.boundingBox())!.y, (await close.boundingBox())!.y];
    await body.evaluate((node) => { node.scrollTop = node.scrollHeight; });
    expect(await body.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
    for (const control of [all, close, total, headerClose]) await expectExposed(control);
    expect([(await all.boundingBox())!.y, (await close.boundingBox())!.y]).toEqual(tops);
    expect(await dialog.evaluate((node) => node.scrollTop)).toBe(0);
    await test.info().attach("trash-header-bottom", { body: await page.screenshot(), contentType: "image/png" });

    await all.click();
    const region = dialog.locator(".checkout-trash-global-confirmation");
    const confirm = region.locator(":scope > .checkout-trash-confirm");
    await expect(body.locator(".checkout-trash-confirm")).toHaveCount(0);
    await expect(confirm).toContainText("Delete 48 archived checkouts");
    const cancel = confirm.locator(".trash-confirm-cancel");
    const submit = confirm.locator(".trash-confirm-submit");
    await expect(submit).toBeFocused();
    for (const control of [cancel, submit]) await expectExposed(control, region);
    for (const control of [all, close, headerClose]) await expectExposed(control);
    const regionBox = (await region.boundingBox())!;
    expect(regionBox.y + regionBox.height).toBeCloseTo((await footer.boundingBox())!.y, 0);
    const confirmingHeight = await body.evaluate(node => node.clientHeight);
    expect(confirmingHeight, "confirmation leaves a usable archive scrollport").toBeGreaterThanOrEqual(32);
    await test.info().attach("trash-list-heights", {
      body: JSON.stringify({viewport: page.viewportSize(), idleHeight, confirmingHeight}), contentType: "application/json",
    });
    // Per-host bulk actions stay available while a global question is open;
    // those native controls, rather than an extra region stop, supply entry.
    const hostDeletes = body.locator(".trash-host-delete");
    await expect(hostDeletes).toHaveCount(names.length);
    for (const action of await hostDeletes.all()) await expect(action).toBeEnabled();
    await expectKeyboardScrollFromControl(page, headerClose, hostDeletes.first(), body);
    for (let index = 1; index < names.length; index++) {
      await page.keyboard.press("Tab");
      await expect(hostDeletes.nth(index)).toBeFocused();
    }
    await page.keyboard.press("Tab");
    await expect(cancel).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(submit).toBeFocused();
    // Pointer-opened questions retain quiet focus; returning by native Tab
    // must still expose the shared keyboard ring on the permanent answer.
    expect(await submit.evaluate(node => node.matches(":focus-visible"))).toBe(true);
    await expect(submit).toHaveCSS("outline-style", "solid");
    await expect(submit).toHaveCSS("outline-width", "2px");
    await page.keyboard.press("Tab");
    await expect(close).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(all).toBeFocused();
    // Its native answers provide keyboard entry, and scrolling them into view
    // must leave the long warning accessible in the same local scrollport.
    await cancel.focus();
    await page.keyboard.press("Home");
    await expect.poll(() => region.evaluate(node => node.scrollTop)).toBe(0);
    await page.keyboard.press("PageDown");
    await expect.poll(() => region.evaluate(node => node.scrollTop)).toBeGreaterThan(0);
    await submit.focus();
    await expectExposed(submit, region);
    expect(await dialog.evaluate((node) => node.scrollTop)).toBe(0);
    await test.info().attach("trash-global-confirmation", { body: await page.screenshot(), contentType: "image/png" });
    await page.keyboard.press("Escape");
    await expect(confirm).toHaveCount(0);
    await expect(all).toBeFocused();
    await all.click();
    await expect(submit).toBeFocused();
    await cancel.click();
    await expect(confirm).toHaveCount(0);
    await expect(all).toBeFocused();
    await all.click();
    await expect(submit).toBeFocused();
    await headerClose.click();
    await expect(confirm).toHaveCount(0);
    await expect(all).toBeFocused();
    await expect(dialog).toBeVisible();
    await headerClose.click();
    await expect(dialog).toHaveCount(0);
    await expect(trigger).toBeFocused();
  });
}
