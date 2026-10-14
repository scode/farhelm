/**
 * Dialog presentation uses controlled archive facts and the shipped renderer.
 * No directories are archived or deleted: route fixtures make long lists and
 * confirmations inexpensive while the helm, modal isolation and CSS stay real.
 */
import { expect, test } from "./helpers/evidence";
import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { localHostId, stubFeed } from "./helpers/fleet";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";

installTerminalSuiteHooks();

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
    expect(ids, "only hosts supplied by the fixture are listed").toContain(host);
    expect(unavailable.has(host), "unreachable hosts must not be listed").toBe(false);
    await fulfillAsHelm(route, { json: { issues: [], checkouts: Array.from({ length: count }, (_, i) => ({
      id: `archive-${host}-${i}`, name: `archived-checkout-${i}`, repository: "example/repository",
      path: `/work/checkouts/farhelm-archived-working-copies/archive-${i}`,
      archived_at: 1_700_000_000 + i, bytes: 1024,
    })).filter((entry) => !removed.get(host)?.has(entry.id)) } });
  });
  return { ids, removed, unavailable, registry };
}

for (const fails of [false, true]) {
  /** A held single reply proves immediate selection and busy admission; later
   * listings must preserve neighbours on the same host and on another host. */
  test(`one trash entry deletes immediately and preserves neighbours after ${fails ? "failure" : "success"}`, async ({page, request}) => {
    const {ids, removed, unavailable} = await stubArchives(page, request, ["single-local", "single-remote", "single-down"], 2);
    unavailable.add(ids[2]);
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
      await expect(down).toContainText("not reachable");
      await expect(down.locator(".trash-entry-delete")).toHaveCount(0);
      const selected = local.getByRole("button", {name: "delete archived-checkout-0 permanently", exact: true});
      await expect(selected).toBeEnabled();
      await selected.scrollIntoViewIfNeeded();
      await expectExposed(selected, dialog.locator(".checkout-trash-body"));
      await selected.click();
      await expect.poll(() => calls.length, {message: "single request reaches held reply"}).toBe(1);
      expect(calls[0].host).toBe(ids[0]);
      expect(calls[0].ids).toEqual([`archive-${ids[0]}-0`]);
      expect(calls[0].expected_incarnation).toBeGreaterThan(0);
      await expect(dialog.locator(".checkout-trash-confirm")).toHaveCount(0);
      await expect(local.locator(".trash-host-progress")).toHaveText("deleting…");
      await expect(dialog.locator(".checkout-trash-progress")).toHaveText("deleting…");
      for (const control of await dialog.locator(".trash-entry-delete, .trash-host-delete, .trash-delete-all, .trash-close").all()) {
        await expect(control).toBeDisabled();
      }
      // Disabling the clicked button can release focus. Put Escape inside the
      // dialog explicitly so this proves its deletion guard, not a missed event.
      await dialog.focus();
      await expect(dialog).toBeFocused();
      await page.keyboard.press("Escape");
      await expect(dialog).toBeVisible();
      await expect(local.locator(".trash-host-progress")).toHaveText("deleting…");
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

for (const width of [1000, 360]) {
  for (const firstFails of [false, true]) {
    /**
     * Host replies retire progress independently, including an uncertain
     * cleanup result. Holding both requests exposes the busy admission guard;
     * a later one-host retry must begin a fresh denominator.
     */
    test(`trash deletion progress survives ${firstFails ? "failure" : "success"} at width ${width}`, async ({ page, request }) => {
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
        await expect(status).toHaveText("deleting on 2 of 2 hosts…");
        for (const section of sections) await expect(section.locator(".trash-host-progress")).toHaveText("deleting…");
        await expect(close).toBeDisabled();
        await expect(all).toBeDisabled();
        await expect(dialog.locator(".trash-host-delete:enabled")).toHaveCount(0);
        await page.mouse.move(0, 0);
        for (const control of [status, all, close]) await expectExposed(control);
        await test.info().attach("trash-deleting-two-hosts", { body: await page.screenshot(), contentType: "image/png" });
        // Submission removes the focused button; explicitly focus the dialog
        // before testing Escape so this input reaches its actual close guard.
        await dialog.focus();
        await expect(dialog).toBeFocused();
        await page.keyboard.press("Escape");
        await expect(dialog).toBeVisible();
        await expect(status).toHaveText("deleting on 2 of 2 hosts…");

        held[0].release();
        await expect(status).toHaveText("deleting on 1 of 2 hosts…");
        await expect(sections[0].locator(".trash-host-progress")).toHaveCount(0);
        await expect(sections[1].locator(".trash-host-progress")).toHaveText("deleting…");
        if (firstFails) await expect(sections[0].locator(".checkout-trash-issue")).toContainText("fixture cleanup unavailable");
        else await expect(sections[0]).toHaveCount(0);
        await expect(close).toBeDisabled();
        await test.info().attach("trash-deleting-one-host", { body: await page.screenshot(), contentType: "image/png" });
        // Short windows can leave the host indicator below the description.
        // Capture its actual paint while also checking the pinned live count.
        await sections[1].locator(".trash-host-progress").scrollIntoViewIfNeeded();
        await expectExposed(sections[1].locator(".trash-host-progress"));
        for (const control of [status, all, close]) await expectExposed(control);
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
          await expect(status).toHaveText("deleting…");
          await expect(sections[0].locator(".trash-host-progress")).toHaveText("deleting…");
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
   * destructive actions must be revealed without moving the pinned header.
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
    const total = dialog.locator(".checkout-trash-total");
    await expect(all).toBeEnabled();
    await test.info().attach("trash-header-top", { body: await page.screenshot(), contentType: "image/png" });
    await expect(dialog.locator(".checkout-trash-header .trash-delete-all")).toHaveCount(1);
    const body = dialog.locator(".checkout-trash-body");
    expect(await body.evaluate((node) => node.scrollHeight > node.clientHeight + 1), "archives overflow the body").toBe(true);
    await page.mouse.move(0, 0);
    for (const control of [all, close, total]) await expectExposed(control);
    const tops = [(await all.boundingBox())!.y, (await close.boundingBox())!.y];
    await body.evaluate((node) => { node.scrollTop = node.scrollHeight; });
    expect(await body.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
    for (const control of [all, close, total]) await expectExposed(control);
    expect([(await all.boundingBox())!.y, (await close.boundingBox())!.y]).toEqual(tops);
    expect(await dialog.evaluate((node) => node.scrollTop)).toBe(0);
    await test.info().attach("trash-header-bottom", { body: await page.screenshot(), contentType: "image/png" });

    await all.click();
    const confirm = body.locator(":scope > .checkout-trash-confirm");
    await expect(confirm).toContainText("Delete 48 archived checkouts");
    const cancel = confirm.locator(".trash-confirm-cancel");
    const submit = confirm.locator(".trash-confirm-submit");
    await expect(submit).toBeFocused();
    for (const control of [cancel, submit]) await expectExposed(control, body);
    for (const control of [all, close]) await expectExposed(control);
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
  });
}
