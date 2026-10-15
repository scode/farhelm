// A real menu write and a second client prove shared pins; DOM positions
// prove the sidebar partition rather than a changed helm/quick-switcher sort.
import { expect, test } from "./helpers/evidence";
import { cleanupSession, createSession, listSessions, openRowMenu, resetPreferences } from "./helpers/fleet";

/** Pin/unpin must survive a second client, and pinned rows move to the top once the pointer is off the list.
 *
 * This does not exercise the pointer hold itself: closing the last row's menu
 * leaves the pointer below the list, so the hold is already released by then.
 * sort.spec.ts's hold tests cover keeping rows still under the pointer. */
test("pin from the menu groups the sidebar and unpin restores title order", async ({ page, request, context }, testInfo) => {
  const stamp = Date.now();
  const created: string[] = [];
  const other = await context.newPage();
  // The sort choice below is a helm-wide preference every later spec shares.
  await resetPreferences(request);
  try {
    const first = await createSession(request, { title: `a-pin-${stamp}` });
    created.push(first.id);
    const last = await createSession(request, { title: `z-pin-${stamp}` });
    created.push(last.id);
    await page.goto("/");
    await page.locator(".sort-select").selectOption("title");
    const a = page.locator(`.session-row[data-session-id="${first.id}"]`);
    const z = page.locator(`.session-row[data-session-id="${last.id}"]`);
    await expect(a).toBeVisible({ timeout: 20_000 });
    await expect(z).toBeVisible();
    await expect(a).toHaveAttribute("data-session-pinned", "false");
    await page.mouse.move(0, 0);
    const order = () => page.locator(".session-row").evaluateAll((rows) => rows.map((row) => row.getAttribute("data-session-id")));
    await expect.poll(async () => (await order()).indexOf(first.id) < (await order()).indexOf(last.id)).toBe(true);
    await other.goto("/");
    const shared = other.locator(`.session-row[data-session-id="${last.id}"]`);
    await expect(shared).toHaveAttribute("data-session-pinned", "false", { timeout: 20_000 });
    await openRowMenu(z);
    await z.getByRole("menuitem", { name: "Pin to top", exact: true }).click();
    await expect(z.getByRole("img", { name: "pinned", exact: true })).toBeVisible();
    await expect(z.locator(".session-pin")).toHaveCSS("color", await z.evaluate((element) => {
      const probe = document.createElement("span");
      probe.style.color = "var(--fg-1)";
      element.append(probe);
      const color = getComputedStyle(probe).color;
      probe.remove();
      return color;
    }));
    await expect(z.locator(".session-row-open")).toHaveAccessibleName(/pinned/);
    await page.mouse.move(0, 0);
    await expect.poll(async () => (await order()).indexOf(last.id) < (await order()).indexOf(first.id)).toBe(true);
    await expect(page.locator(".session-pin-separator")).toHaveCount(1);
    await expect(shared).toHaveAttribute("data-session-pinned", "true", { timeout: 20_000 });
    await testInfo.attach("pinned-sidebar", { body: await page.locator(".app-sidebar").screenshot(), contentType: "image/png" });
    const reply = await listSessions(request, "sort=title");
    expect(reply.sessions.findIndex((row) => row.id === first.id)).toBeLessThan(reply.sessions.findIndex((row) => row.id === last.id));
    await openRowMenu(z);
    await z.getByRole("menuitem", { name: "Unpin", exact: true }).click();
    await expect(z.locator(".session-pin")).toHaveCount(0);
    await page.mouse.move(0, 0);
    await expect.poll(async () => (await order()).indexOf(first.id) < (await order()).indexOf(last.id)).toBe(true);
    await expect(page.locator(".session-pin-separator")).toHaveCount(0);
    await expect(shared).toHaveAttribute("data-session-pinned", "false", { timeout: 20_000 });
  } finally {
    await resetPreferences(request);
    await other.close();
    for (const id of created.reverse()) await cleanupSession(request, id);
  }
});
