/**
 * Dialog presentation uses controlled archive facts and the shipped renderer.
 * No directories are archived or deleted: route fixtures make long lists and
 * confirmations inexpensive while the helm, modal isolation and CSS stay real.
 */
import { expect, test } from "./helpers/evidence";
import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { localHostId, stubFeed } from "./helpers/fleet";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";
import { expectKeyboardScroll, expectKeyboardScrollFromControl } from "./helpers/keyboard-scroll";
import { recordPage } from "./helpers/timeline";

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
  // Read calls distinguish the removing host's cadence from a fleet-wide poll.
  const listings: number[] = [];
  const removing = new Set<number>();
  const issues = new Map<number, {id: string; path: string; message: string}[]>();
  const unavailable = new Set<number>();
  const registry = { fail: false, omitted: new Set<number>() };
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
    body.hosts = body.hosts.filter((host: {id: number}) => !registry.omitted.has(host.id));
    await route.fulfill({ response, json: body });
  });
  await page.route("**/api/checkout-trash/list", async (route) => {
    const { host } = route.request().postDataJSON();
    listings.push(host);
    expect(ids, "only hosts supplied by the fixture are listed").toContain(host);
    expect(unavailable.has(host), "unreachable hosts must not be listed").toBe(false);
    await fulfillAsHelm(route, { json: { removing: removing.has(host), issues: issues.get(host) ?? [], checkouts: Array.from({ length: count }, (_, i) => ({
      id: `archive-${host}-${i}`, name: `archived-checkout-${i}`, repository: "example/repository",
      path: `/work/checkouts/.farhelm-trash/archive-${i}`,
      archived_at: 1_700_000_000 + i, bytes: 1024,
    })).filter((entry) => !removed.get(host)?.has(entry.id)) } });
  });
  return { ids, removed, removing, unavailable, registry, listings, issues };
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
      await expect(dialog).toBeVisible();
      await expect(dialog.locator(".checkout-trash-confirm")).toHaveCount(0);
      const current = local.locator(".checkout-trash-entry").filter({hasText: "archived-checkout-0"});
      await expect(current).toHaveCount(1);
      await expect(dialog.locator(".trash-entry-progress, .trash-entry-waiting, .trash-host-progress, .checkout-trash-progress")).toHaveCount(0);
      await expect(diagnostic).toHaveText(diagnosticBefore, {useInnerText: true});
      await expect(dialog.locator(".trash-close")).toBeEnabled();
      await expect(dialog.locator(".trash-header-close")).toBeEnabled();
      for (const control of await dialog.locator(".trash-entry-delete, .trash-host-delete, .trash-delete-all").all()) {
        await expect(control).toBeDisabled();
      }
      // Dismissing the modal must leave its persistent owner's request alive.
      await dialog.focus();
      await expect(dialog).toBeFocused();
      await page.keyboard.press("Escape");
      await expect(dialog).toHaveCount(0);
      await expect(trigger.locator(".trash-delete-icon")).toHaveClass(/\bdeleting\b/);
      release();
      await expect(trigger.locator(".trash-delete-icon")).not.toHaveClass(/\bdeleting\b/);
      await trigger.click();
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

/** A later successful entry delete acknowledges only that entry's issue.
 * Refusals for its neighbour remain inspectable across unrelated work. */
test("deleting another trash entry preserves an earlier refusal", async ({page, request}) => {
  const {ids, removed} = await stubArchives(page, request, ["retained-local"], 2);
  const refused = `archive-${ids[0]}-0`;
  const issue = {id: refused, path: "/work/trash/refused", message: "fixture earlier refusal"};
  await page.route("**/api/checkout-trash/delete", async route => {
    const selected = route.request().postDataJSON();
    expect(selected.ids).toHaveLength(1);
    if (selected.ids[0] === refused) {
      await fulfillAsHelm(route, {json: {removed: [], issues: [issue]}});
    } else {
      removed.set(selected.host, new Set(selected.ids));
      await fulfillAsHelm(route, {json: {removed: selected.ids, issues: []}});
    }
  });
  await page.goto("/");
  const trigger = page.locator(".checkout-trash-button");
  await expect(trigger).toHaveAttribute("data-count", "2");
  await trigger.click();
  const dialog = page.locator(".checkout-trash-dialog");
  const first = dialog.getByRole("button", {name: "Delete archived-checkout-0 permanently", exact: true});
  await expect(first).toBeEnabled();
  await first.click();
  await expect(dialog.locator(".checkout-trash-issue")).toContainText(issue.message);
  const other = dialog.getByRole("button", {name: "Delete archived-checkout-1 permanently", exact: true});
  await expect(other).toBeEnabled();
  await other.click();
  await expect(dialog.locator(".checkout-trash-entry")).toHaveCount(1);
  await expect(dialog.locator(".checkout-trash-issue")).toContainText(issue.message);
  await dialog.locator(".trash-close").click();
  await expect(trigger).toHaveAttribute("data-tooltip", /Some checkouts could not be deleted/);
  await trigger.click();
  await expect(dialog.locator(".checkout-trash-issue")).toContainText(issue.message);
});

/** A lost reply's frozen selection cannot be acknowledged by deleting a
 * neighbour. Resolving that selection later clears only its uncertainty,
 * leaving an independently refused checkout inspectable. */
test("an unrelated trash success retains an uncertain outcome until its own retry", async ({page, request}) => {
  const {ids, removed} = await stubArchives(page, request, ["uncertain-local"], 3);
  const uncertain = `archive-${ids[0]}-0`;
  const refused = `archive-${ids[0]}-2`;
  const issue = {id: refused, path: "/work/trash/refused", message: "fixture independent refusal"};
  let uncertainAttempts = 0;
  await page.route("**/api/checkout-trash/delete", async route => {
    const selected = route.request().postDataJSON();
    expect(selected.ids).toHaveLength(1);
    if (selected.ids[0] === refused) {
      return fulfillAsHelm(route, {json: {removed: [], issues: [issue]}});
    }
    if (selected.ids[0] === uncertain && ++uncertainAttempts === 1) {
      return fulfillAsHelm(route, {status: 503, contentType: "text/plain", body: "fixture lost archive A reply"});
    }
    removed.set(selected.host, new Set([...(removed.get(selected.host) ?? []), ...selected.ids]));
    await fulfillAsHelm(route, {json: {removed: selected.ids, issues: []}});
  });
  await page.goto("/");
  const trigger = page.locator(".checkout-trash-button");
  await expect(trigger).toHaveAttribute("data-count", "3");
  await trigger.click();
  const dialog = page.locator(".checkout-trash-dialog");
  const select = (index: number) => dialog.getByRole("button", {name: `Delete archived-checkout-${index} permanently`, exact: true});
  const warning = dialog.locator(".checkout-trash-issue").filter({hasText: "cleanup reply could not be confirmed"});
  await expect(select(2)).toBeEnabled();
  await select(2).click();
  await expect(dialog.locator(".checkout-trash-issue").filter({hasText: issue.message})).toHaveCount(1);
  await expect(select(0)).toBeEnabled();
  await select(0).click();
  await expect(warning).toHaveCount(1);
  await dialog.locator(".trash-close").click();
  await trigger.click();
  await expect(warning).toHaveCount(1);
  await expect(select(1)).toBeEnabled();
  await select(1).click();
  await expect(dialog.locator(".checkout-trash-entry")).toHaveCount(2);
  await expect(warning).toHaveCount(1);
  await expect(dialog.locator(".checkout-trash-issue").filter({hasText: issue.message})).toHaveCount(1);
  await expect(select(0)).toBeEnabled();
  await select(0).click();
  await expect(dialog.locator(".checkout-trash-entry")).toHaveCount(1);
  await expect(warning).toHaveCount(0);
  await expect(dialog.locator(".checkout-trash-issue").filter({hasText: issue.message})).toHaveCount(1);
  await dialog.locator(".trash-close").click();
  await expect(trigger).toHaveAttribute("data-tooltip", /Some checkouts could not be deleted/);
});

/** Disconnection and an uncertain registry refresh cannot acknowledge a
 * failure. Only a healthy registry that actually omits the host removes its
 * warning, whose details would otherwise be impossible to open again. */
test("trash notices survive outages and disappear when their host is removed", async ({page, request}) => {
  const {ids, unavailable, registry} = await stubArchives(page, request, ["retained-local", "removable-remote"], 1);
  const feed = await stubFeed(page);
  await page.route("**/api/checkout-trash/delete", route =>
    fulfillAsHelm(route, {status: 503, contentType: "text/plain", body: "fixture remote cleanup failure"}));
  await page.goto("/");
  await feed.waitForConnection(1);
  const trigger = page.locator(".checkout-trash-button");
  await expect(trigger).toHaveAttribute("data-count", "2");
  await trigger.click();
  const dialog = page.locator(".checkout-trash-dialog");
  const remote = dialog.locator(`.checkout-trash-host[data-host-id="${ids[1]}"]`);
  await expect(remote.locator(".trash-entry-delete")).toBeEnabled();
  await remote.locator(".trash-entry-delete").click();
  const warning = remote.locator(".checkout-trash-issue").filter({hasText: "cleanup reply could not be confirmed"});
  await expect(warning).toHaveCount(1);
  unavailable.add(ids[1]);
  feed.notify(1);
  await expect(remote).toHaveClass(/unreachable/);
  await expect(warning).toHaveCount(1);
  registry.fail = true;
  feed.notify(2);
  await expect(dialog.locator(".checkout-trash-issue").filter({hasText: "Host reachability could not be refreshed"})).toBeVisible();
  await expect(warning).toHaveCount(1);
  await expect(trigger).toHaveAttribute("data-tooltip", /Some checkouts could not be deleted/);
  registry.fail = false;
  registry.omitted.add(ids[1]);
  feed.notify(3);
  await expect(remote).toHaveCount(0);
  await expect(trigger).not.toHaveAttribute("data-tooltip", /Some checkouts could not be deleted/);
  await dialog.locator(".trash-close").click();
  await trigger.click();
  await expect(remote).toHaveCount(0);
  await expect(dialog.locator(".checkout-trash-issue").filter({hasText: "cleanup reply could not be confirmed"})).toHaveCount(0);
});

/** An accepted request can answer after its host has left the registry. Once
 * that healthy removal is observed, even a subsequent registry outage cannot
 * give the late response permission to recreate an orphaned warning. */
test("a late failed trash reply cannot restore notices for a removed host", async ({page, request}) => {
  const {ids, registry} = await stubArchives(page, request, ["kept-local", "removed-remote"], 1);
  const feed = await stubFeed(page);
  const held = deletionGate();
  let sent = false;
  let answered = false;
  await page.route("**/api/checkout-trash/delete", async route => {
    expect(route.request().postDataJSON().host).toBe(ids[1]);
    sent = true;
    await held.ready;
    await fulfillAsHelm(route, {status: 503, contentType: "text/plain", body: "fixture late removed-host failure"});
    answered = true;
  });
  try {
    await page.goto("/");
    await feed.waitForConnection(1);
    const trigger = page.locator(".checkout-trash-button");
    await expect(trigger).toHaveAttribute("data-count", "2");
    await trigger.click();
    const dialog = page.locator(".checkout-trash-dialog");
    const remote = dialog.locator(`.checkout-trash-host[data-host-id="${ids[1]}"]`);
    await expect(remote.locator(".trash-entry-delete")).toBeEnabled();
    await remote.locator(".trash-entry-delete").click();
    await expect.poll(() => sent, {message: "the remote request is held while its host still exists"}).toBe(true);
    registry.omitted.add(ids[1]);
    feed.notify(1);
    await expect(remote).toHaveCount(0);
    registry.fail = true;
    feed.notify(2);
    await expect(dialog.locator(".checkout-trash-issue").filter({hasText: "Host reachability could not be refreshed"})).toBeVisible();
    held.release();
    await expect.poll(() => answered, {message: "the removed host's failed response is delivered during the registry outage"}).toBe(true);
    await expect(trigger.locator(".trash-delete-icon")).not.toHaveClass(/\bdeleting\b/);
    await expect(trigger).not.toHaveAttribute("data-tooltip", /Some checkouts could not be deleted/);
    await expect(dialog.locator(".checkout-trash-issue").filter({hasText: "late removed-host failure"})).toHaveCount(0);
    await dialog.locator(".trash-close").click();
    await trigger.click();
    await expect(remote).toHaveCount(0);
    await expect(dialog.locator(".checkout-trash-issue").filter({hasText: "cleanup reply could not be confirmed"})).toHaveCount(0);
  } finally {
    held.release();
    await page.unrouteAll({behavior: "wait"});
  }
});

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
   * Confirmation hands one immutable batch to each host and releases the modal
   * before any reply. Keeping that reply held past sixty seconds also proves
   * compatibility with supervisors that still remove inside the request.
   */
  test(`${action} trash cleanup closes immediately with ${action === "host" ? "reduced" : "normal"} motion`, async ({page, request}) => {
    await page.clock.install();
    const reduced = action === "host";
    await page.emulateMedia({reducedMotion: reduced ? "reduce" : "no-preference"});
    const {ids, removed, listings} = await stubArchives(page, request, ["batch-local", "batch-remote"], 3);
    const held = ids.map(deletionGate);
    const calls: {host: number; ids: string[]; expected_incarnation: number}[] = [];
    await page.route("**/api/checkout-trash/delete", async (route) => {
      const selected = route.request().postDataJSON();
      const index = ids.indexOf(selected.host);
      expect(index, "only a selected host receives cleanup").toBeGreaterThanOrEqual(0);
      calls.push(selected);
      await held[index].ready;
      removed.set(selected.host, new Set(selected.ids));
      await fulfillAsHelm(route, {json: {removed: selected.ids, issues: []}});
    });
    try {
      await page.goto("/");
      const trigger = page.locator(".checkout-trash-button");
      await expect(trigger).toHaveAttribute("data-count", "6");
      await trigger.click();
      const dialog = page.locator(".checkout-trash-dialog");
      await expect(dialog).not.toContainText("Measuring…");
      await expect(dialog.locator(".trash-delete-all")).toBeEnabled();
      const button = action === "global" ? dialog.locator(".trash-delete-all") :
        dialog.locator(`.checkout-trash-host[data-host-id="${ids[0]}"] .trash-host-delete`);
      await button.click();
      await expect(dialog.locator(".checkout-trash-confirm")).toContainText(action === "global" ? "Delete 6 archived checkouts" : "Delete 3 archived checkouts");
      await expect(dialog.locator(".trash-confirm-submit")).toBeFocused();
      const readsBefore = listings.length;
      await page.keyboard.press("Enter");
      await expect(dialog).toHaveCount(0);
      await expect.poll(() => calls.length, {message: "each selected host reaches its held batch"}).toBe(action === "global" ? 2 : 1);
      for (const call of calls) {
        expect(call.ids).toEqual([0, 1, 2].map(index => `archive-${call.host}-${index}`));
        expect(call.expected_incarnation).toBeGreaterThan(0);
      }
      await expect(page.locator(".new-session-button")).toBeEnabled();
      const icon = trigger.locator(".trash-delete-icon");
      await expect(icon).toHaveClass(/\bdeleting\b/);
      await expect(trigger).toHaveAttribute("data-tooltip", /Emptying the trash: freeing space on batch-local/);
      await expect(icon).toHaveCSS("animation-name", reduced ? "none" : "trash-delete-bob");
      if (!reduced) {
        await expect(icon).toHaveCSS("animation-timing-function", /^(steps\(|step-start|step-end)/);
        await expect(icon).toHaveCSS("animation-iteration-count", "infinite");
        const offsets = await icon.evaluate(node => {
          const effect = node.getAnimations().find(animation => animation instanceof CSSAnimation && animation.animationName === "trash-delete-bob")?.effect as KeyframeEffect | null;
          return effect?.getKeyframes().map(frame => new DOMMatrixReadOnly(String(frame.transform)).f) ?? [];
        });
        expect(new Set(offsets).size, "the active animation moves the inverted can vertically").toBeGreaterThan(1);
      }
      const matrix = await icon.evaluate(node => {
        const transform = new DOMMatrixReadOnly(getComputedStyle(node).transform);
        return [transform.a, transform.b, transform.c, transform.d];
      });
      expect(matrix[0]).toBeCloseTo(-1);
      expect(matrix[1]).toBeCloseTo(0);
      expect(matrix[2]).toBeCloseTo(0);
      expect(matrix[3]).toBeCloseTo(-1);
      await page.clock.fastForward(61_000);
      await expect(icon).toHaveClass(/\bdeleting\b/);
      expect(calls).toHaveLength(action === "global" ? 2 : 1);
      expect(listings, "pending old-host requests do not speculate about completion").toHaveLength(readsBefore);
      for (const gate of held) gate.release();
      await expect(trigger).toHaveAttribute("data-count", action === "global" ? "0" : "3");
      await expect(icon).not.toHaveClass(/\bdeleting\b/);
      await expect(icon).toHaveCSS("animation-name", "none");
      expect(calls).toHaveLength(action === "global" ? 2 : 1);
    } finally {
      for (const gate of held) gate.release();
      await page.unrouteAll({behavior: "wait"});
    }
  });
}

for (const failure of ["http", "transport"] as const) {
  /** One failed batch cannot stop another host, and closing or reopening the
   * dialog never acknowledges an uncertain result or a per-checkout refusal. */
  test(`trash batch issues survive reopening after ${failure} failure`, async ({page, request}) => {
    const {ids, removed} = await stubArchives(page, request, ["failed-local", "mixed-remote"], 3);
    const held = ids.map(deletionGate);
    const calls: {host: number; ids: string[]}[] = [];
    const issue = {id: `archive-${ids[1]}-1`, path: "/work/trash/refused", message: "fixture retained checkout refusal"};
    await page.route("**/api/checkout-trash/delete", async route => {
      const selected = route.request().postDataJSON();
      calls.push(selected);
      const index = ids.indexOf(selected.host);
      expect(index).toBeGreaterThanOrEqual(0);
      await held[index].ready;
      if (index === 0) {
        if (failure === "transport") await route.abort("failed");
        else await fulfillAsHelm(route, {status: 503, contentType: "text/plain", body: "fixture cleanup unavailable"});
      } else {
        const successful = selected.ids.filter((id: string) => id !== issue.id);
        removed.set(selected.host, new Set(successful));
        await fulfillAsHelm(route, {json: {removed: successful, issues: [issue]}});
      }
    });
    try {
      await page.goto("/");
      const trigger = page.locator(".checkout-trash-button");
      await expect(trigger).toHaveAttribute("data-count", "6");
      await trigger.click();
      const dialog = page.locator(".checkout-trash-dialog");
      await expect(dialog.locator(".trash-delete-all")).toBeEnabled();
      await dialog.locator(".trash-delete-all").click();
      await dialog.locator(".trash-confirm-submit").click();
      await expect(dialog).toHaveCount(0);
      await expect.poll(() => calls.length, {message: "both host batches are admitted independently"}).toBe(2);
      held[0].release();
      await expect(trigger).toHaveAttribute("data-tooltip", /Some checkouts could not be deleted\. Open the trash to see why/);
      await expect(trigger.locator(".trash-delete-icon")).toHaveClass(/\bdeleting\b/);
      held[1].release();
      await expect(trigger).toHaveAttribute("data-count", "4");
      await expect(trigger.locator(".trash-delete-icon")).not.toHaveClass(/\bdeleting\b/);
      for (let opening = 0; opening < 2; opening++) {
        await trigger.click();
        const local = dialog.locator(`.checkout-trash-host[data-host-id="${ids[0]}"]`);
        const remote = dialog.locator(`.checkout-trash-host[data-host-id="${ids[1]}"]`);
        await expect(local.locator(".checkout-trash-entry")).toHaveCount(3);
        await expect(local.locator(".checkout-trash-issue")).toContainText("cleanup reply could not be confirmed");
        await expect(remote.locator(".checkout-trash-entry")).toHaveCount(1);
        await expect(remote.locator(".checkout-trash-issue")).toContainText(issue.message);
        await expect(dialog.locator(".trash-host-progress, .trash-entry-progress, .trash-entry-waiting, .checkout-trash-progress")).toHaveCount(0);
        await dialog.locator(".trash-close").click();
        await expect(dialog).toHaveCount(0);
      }
      expect(calls).toHaveLength(2);
    } finally {
      for (const gate of held) gate.release();
      await page.unrouteAll({behavior: "wait"});
    }
  });
}

/** A request's completion hands the indicator to the host's actual remover.
 * Only that host is polled, including across one failed read; an explicit false
 * reply ends the cadence even though a refused leftover could still exist. */
test("trash polls only the host freeing space and stops on its completion", async ({page, request}) => {
  await page.clock.install();
  const {ids, removed, removing, listings} = await stubArchives(page, request, ["removing-local", "idle-remote"], 1);
  const held = deletionGate();
  let sent = false;
  await page.route("**/api/checkout-trash/delete", async route => {
    const selected = route.request().postDataJSON();
    sent = true;
    await held.ready;
    removed.set(selected.host, new Set(selected.ids));
    removing.add(selected.host);
    await fulfillAsHelm(route, {json: {removed: selected.ids, issues: []}});
  });
  try {
    await page.goto("/");
    const trigger = page.locator(".checkout-trash-button");
    await expect(trigger).toHaveAttribute("data-count", "2");
    await trigger.click();
    const dialog = page.locator(".checkout-trash-dialog");
    await expect(dialog.locator(".trash-host-delete").first()).toBeEnabled();
    await dialog.locator(`.checkout-trash-host[data-host-id="${ids[0]}"] .trash-host-delete`).click();
    await dialog.locator(".trash-confirm-submit").click();
    await expect(dialog).toHaveCount(0);
    await expect.poll(() => sent, {message: "the delete request is held before handoff"}).toBe(true);
    held.release();
    await expect(trigger).toHaveAttribute("data-count", "1");
    // Reopening exposes trash-local admission: the remaining idle host's bulk
    // action is enabled only once this page's delete requests have settled.
    await trigger.click();
    await expect(dialog.locator(".trash-delete-all")).toBeEnabled();
    await expect(dialog.locator(".checkout-trash-header .trash-delete-icon")).not.toHaveClass(/\bdeleting\b/);
    await Promise.all([
      ...ids.map(id => page.waitForResponse(response => new URL(response.url()).pathname === "/api/checkout-trash/list" &&
        response.request().postDataJSON().host === id && !response.request().postDataJSON().measure_sizes)),
      dialog.locator(".trash-close").click(),
    ]);
    await expect(trigger.locator(".trash-delete-icon")).toHaveClass(/\bdeleting\b/);
    await expect(trigger).toHaveAttribute("data-tooltip", /freeing space on removing-local/);
    const reads = listings.length;
    let failNext = true;
    await page.route("**/api/checkout-trash/list", async route => {
      if (route.request().postDataJSON().host === ids[0] && failNext) {
        failNext = false;
        listings.push(ids[0]);
        return fulfillAsHelm(route, {status: 503, contentType: "text/plain", body: "fixture read unavailable"});
      }
      await route.fallback();
    });
    await page.clock.runFor(3_100);
    await expect.poll(() => listings.length, {message: "the removing host is polled"}).toBe(reads + 1);
    await expect(trigger.locator(".trash-delete-icon")).toHaveClass(/\bdeleting\b/);
    await page.clock.runFor(3_100);
    await expect.poll(() => listings.length, {message: "a failed poll retains the last running report"}).toBe(reads + 2);
    expect(listings.slice(reads)).toEqual([ids[0], ids[0]]);
    removing.delete(ids[0]);
    await page.clock.runFor(3_100);
    await expect(trigger.locator(".trash-delete-icon")).not.toHaveClass(/\bdeleting\b/);
    const stopped = listings.length;
    await page.clock.runFor(9_100);
    expect(listings, "an idle host and a completed remover have no polling cadence").toHaveLength(stopped);
  } finally {
    held.release();
    await page.unrouteAll({behavior: "wait"});
  }
});

/** A poll begun before a confirmed delete is stale even when it answers after
 * the delete's fresh listing. It must restore neither the entry nor animation. */
test("a late trash poll cannot restore a confirmed deletion", async ({page, request}) => {
  await page.clock.install();
  const {ids, removed, removing, listings} = await stubArchives(page, request, ["stale-local"], 1);
  removing.add(ids[0]);
  const held = deletionGate();
  let holdPoll = false;
  let pollHeld = false;
  let pollReturned = false;
  await page.route("**/api/checkout-trash/list", async route => {
    if (!holdPoll) return route.fallback();
    holdPoll = false;
    pollHeld = true;
    listings.push(ids[0]);
    await held.ready;
    await fulfillAsHelm(route, {json: {removing: true, issues: [], checkouts: [{
      id: `archive-${ids[0]}-0`, name: "archived-checkout-0", repository: "example/repository",
      path: "/work/checkouts/.farhelm-trash/archive-0", archived_at: 1_700_000_000, bytes: 1024,
    }]}});
    pollReturned = true;
  });
  await page.route("**/api/checkout-trash/delete", async route => {
    const selected = route.request().postDataJSON();
    removed.set(selected.host, new Set(selected.ids));
    removing.delete(selected.host);
    await fulfillAsHelm(route, {json: {removed: selected.ids, issues: []}});
  });
  try {
    await page.goto("/");
    const trigger = page.locator(".checkout-trash-button");
    await expect(trigger).toHaveAttribute("data-count", "1");
    await trigger.click();
    const dialog = page.locator(".checkout-trash-dialog");
    await expect(dialog.locator(".trash-delete-all")).toBeEnabled();
    await dialog.locator(".trash-delete-all").click();
    await expect(dialog.locator(".trash-confirm-submit")).toBeFocused();
    holdPoll = true;
    await page.clock.runFor(3_100);
    await expect.poll(() => pollHeld, {message: "the pre-delete poll remains in flight"}).toBe(true);
    await dialog.locator(".trash-confirm-submit").click();
    await expect(dialog).toHaveCount(0);
    await expect(trigger).toHaveAttribute("data-count", "0");
    await expect(trigger.locator(".trash-delete-icon")).not.toHaveClass(/\bdeleting\b/);
    held.release();
    await expect.poll(() => pollReturned, {message: "the stale reply was delivered after the fresh empty listing"}).toBe(true);
    await expect(trigger).toHaveAttribute("data-count", "0");
    await expect(trigger.locator(".trash-delete-icon")).not.toHaveClass(/\bdeleting\b/);
    const settledReads = listings.length;
    await page.clock.runFor(6_100);
    expect(listings, "the stale running flag cannot restart polling").toHaveLength(settledReads);
    await trigger.click();
    await expect(dialog.locator(".checkout-trash-entry")).toHaveCount(0);
    await expect(dialog.locator(".checkout-trash-empty")).toBeVisible();
  } finally {
    held.release();
    await page.unrouteAll({behavior: "wait"});
  }
});

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
    // Record the premise with the existing scroll sample: a lost focus handoff
    // and a confirmation that no longer overflows both otherwise read as zero.
    await expect.poll(async () => {
      const state = await region.evaluate(node => ({
        scroll_top: node.scrollTop,
        scroll_height: node.scrollHeight,
        client_height: node.clientHeight,
        overflow_y: getComputedStyle(node).overflowY,
        focused_inside: node.contains(document.activeElement),
        focused_tag: document.activeElement?.tagName ?? "",
        focused_classes: document.activeElement?.getAttribute("class")?.slice(0, 128) ?? "",
        document_focused: document.hasFocus(),
      }));
      recordPage(page, "trash-pagedown-sample", Object.entries(state));
      return state.scroll_top;
    }).toBeGreaterThan(0);
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
