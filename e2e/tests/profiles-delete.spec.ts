import { expect, test } from "./helpers/evidence";
import { cleanupProfile, createProfile, listProfiles, stubFeed } from "./helpers/fleet";

/**
 * Removing the focused Delete control can leave document focus in transit.
 * A missing replacement focus target is not an outside action and must not
 * discard the confirmation. The seam only prevents placement, not dismissal.
 */
for (const continuation of ["escape", "outside focus", "keyboard confirmation"] as const) {
test(`first delete retains confirmation when focus placement misses: ${continuation}`, async ({ page, request }, testInfo) => {
  const target = await createProfile(request, { name: "missing-delete-focus" });
  const deletes: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "DELETE") deletes.push(new URL(request.url()).pathname);
  });
  try {
    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    await expect(page.locator(".host-row").first()).toBeVisible();
    await page.locator(".profiles-toggle").click();
    const popup = page.locator(".profiles-popover");
    await expect(popup.locator(".new-profile-button")).toBeFocused();
    const row = popup.locator(`[data-profile-id="${target.id}"]`);
    await row.locator(".profile-delete").focus();
    await expect(row.locator(".profile-delete")).toBeFocused();
    await page.evaluate(() => {
      Object.assign(window, { __farhelmTestProfiles: { hideFocusTarget: true } });
    });
    await row.locator(".profile-delete").click();
    await expect.poll(() => page.evaluate(() =>
      Reflect.get(window, "__farhelmTestProfiles")?.focusSettled
    )).toMatch(/^(missing|unknown)$/);
    expect(deletes).toEqual([]);
    // sleep-ok: observe delayed dismissal after the focus worker has definitively retired.
    await page.waitForTimeout(400);
    await expect(popup).toBeVisible();
    await expect(row.locator(".profile-confirm-delete")).toBeVisible();
    const fits = await popup.evaluate((node) => node.scrollWidth <= node.clientWidth + 1);
    expect(fits, "confirmation controls must fit without sideways scrolling").toBe(true);
    await expect.poll(() => page.evaluate(() => document.activeElement === document.body)).toBe(true);
    const outcome = await page.evaluate(() => Reflect.get(window, "__farhelmTestProfiles")?.focusSettled);
    await testInfo.attach("focus-placement-outcome.json", {
      body: JSON.stringify({ outcome }),
      contentType: "application/json",
    });
    if (continuation === "escape") {
      await page.keyboard.press("Escape");
      await expect(popup).toHaveCount(0);
      await expect(page.locator(".profiles-toggle")).toBeFocused();
    } else if (continuation === "outside focus") {
      const outside = page.locator(".host-details-toggle");
      await outside.focus();
      await expect(popup).toHaveCount(0);
      await expect(outside).toBeFocused();
    } else {
      // Deliberately failed automatic placement must leave native controls
      // usable. Move focus explicitly, then use real keyboard activation.
      await row.locator(".profile-cancel-delete").focus();
      await expect(row.locator(".profile-cancel-delete")).toBeFocused();
      await page.keyboard.press("Enter");
      await expect(row.locator(".profile-delete")).toBeVisible();
      expect((await listProfiles(request)).profiles.some((profile) => profile.id === target.id)).toBe(true);
      expect(deletes).toEqual([]);
    }
  } finally {
    await cleanupProfile(request, target.id);
  }
});
}

/**
 * Starting deletion replaces the clicked control and expands a row in a
 * scrolling catalog. Neither transition is an outside dismissal choice, and
 * the first click must leave confirmation usable without sending DELETE.
 */
test("legacy profile deletion stays open at the catalog edge", async ({ page, request }, testInfo) => {
  const owned: string[] = [];
  const deletes: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "DELETE" && new URL(request.url()).pathname.startsWith("/api/profiles/")) {
      deletes.push(new URL(request.url()).pathname);
    }
  });
  await page.setViewportSize({ width: 960, height: 520 });
  await page.addInitScript(() => {
    const trace: unknown[] = [];
    Object.assign(window, { __profileDeleteTrace: trace });
    const describe = (node: EventTarget | null) => node instanceof Element
      ? `${node.tagName}.${typeof node.className === "string" ? node.className : "svg"}`
      : "outside document";
    for (const type of ["pointerdown", "click", "focusin", "focusout", "scroll"]) {
      document.addEventListener(type, (event) => {
        if (trace.length >= 120) trace.shift();
        trace.push({
          type,
          at: performance.now(),
          target: describe(event.target),
          active: describe(document.activeElement),
          popup: !!document.querySelector(".profiles-popover"),
        });
      }, true);
    }
  });
  try {
    const builtins = (await listProfiles(request)).profiles.filter((profile) => profile.builtin);
    expect(builtins.length, "the regression needs built-ins followed by stored legacy rows").toBeGreaterThan(0);
    for (let index = 0; index < 10; index++) {
      const profile = await createProfile(request, { name: `deletion-edge-${index}` });
      owned.push(profile.id);
    }
    const target = await createProfile(request, { name: builtins[0].name });
    owned.push(target.id);
    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    await expect(page.locator(".host-row").first()).toBeVisible();
    await page.locator(".profiles-toggle").click();
    const popup = page.locator(".profiles-popover");
    await expect(popup.locator(".new-profile-button")).toBeFocused();
    const row = popup.locator(`[data-profile-id="${target.id}"]`);
    await expect(row.locator(".profile-delete")).toHaveCount(1);
    const overflow = await popup.evaluate((node) => node.scrollHeight > node.clientHeight);
    expect(overflow, "a short catalog does not exercise scrolling confirmation").toBe(true);
    await row.locator(".profile-delete").scrollIntoViewIfNeeded();
    await expect(popup, "scrolling inside the popup is not an outside dismissal").toBeVisible();
    await row.locator(".profile-delete").click();
    await expect(row.locator(".profile-cancel-delete")).toBeFocused();
    await expect(row.locator(".profile-confirm-delete")).toBeVisible();
    expect(deletes, "the first click only starts confirmation").toEqual([]);
    await row.locator(".profile-cancel-delete").click();
    await expect(row.locator(".profile-edit")).toBeFocused();
    expect((await listProfiles(request)).profiles.some((profile) => profile.id === target.id)).toBe(true);
    await row.locator(".profile-delete").click();
    await expect(row.locator(".profile-cancel-delete")).toBeFocused();
    await row.locator(".profile-confirm-delete").click();
    await expect(row).toHaveCount(0);
    const remaining = (await listProfiles(request)).profiles;
    expect(remaining.some((profile) => profile.id === target.id)).toBe(false);
    expect(remaining.some((profile) => profile.id === builtins[0].id && profile.builtin)).toBe(true);
    expect(deletes).toEqual([`/api/profiles/${target.id}`]);
  } finally {
    if (!page.isClosed()) {
      const trace = await page.evaluate(() => Reflect.get(window, "__profileDeleteTrace"));
      await testInfo.attach("profile-delete-events.json", {
        body: JSON.stringify(trace),
        contentType: "application/json",
      });
    }
    for (const id of owned) await cleanupProfile(request, id);
  }
});
