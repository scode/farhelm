/**
 * Dialog presentation uses controlled archive facts and the shipped renderer.
 * No directories are archived or deleted: route fixtures make long lists and
 * confirmations inexpensive while the helm, modal isolation and CSS stay real.
 */
import { expect, test } from "./helpers/evidence";
import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { localHostId } from "./helpers/fleet";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";

installTerminalSuiteHooks();

/** Keep the real local identity and add only display-only connected remotes. */
async function stubArchives(page: Page, request: APIRequestContext, names: string[], count = 12) {
  const local = await localHostId(request);
  const ids = [local, ...names.slice(1).map((_, i) => 9020 + i)];
  await page.route("**/api/hosts", async (route) => {
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
    await route.fulfill({ response, json: body });
  });
  await page.route("**/api/checkout-trash/list", async (route) => {
    const { host } = route.request().postDataJSON();
    expect(ids, "only hosts supplied by the fixture are listed").toContain(host);
    await fulfillAsHelm(route, { json: { issues: [], checkouts: Array.from({ length: count }, (_, i) => ({
      id: `archive-${host}-${i}`, name: `archived-checkout-${i}`, repository: "example/repository",
      path: `/work/checkouts/farhelm-archived-working-copies/archive-${i}`,
      archived_at: 1_700_000_000 + i, bytes: 1024,
    })) } });
  });
  return ids;
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
    await expect(cancel).toBeFocused();
    for (const control of [cancel, submit]) await expectExposed(control, body);
    for (const control of [all, close]) await expectExposed(control);
    expect(await dialog.evaluate((node) => node.scrollTop)).toBe(0);
    await test.info().attach("trash-global-confirmation", { body: await page.screenshot(), contentType: "image/png" });
    await page.keyboard.press("Escape");
    await expect(confirm).toHaveCount(0);
    await expect(all).toBeFocused();
    await all.click();
    await expect(cancel).toBeFocused();
    await cancel.click();
    await expect(confirm).toHaveCount(0);
    await expect(all).toBeFocused();
  });
}
