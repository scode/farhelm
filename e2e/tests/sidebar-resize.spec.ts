/** Sidebar geometry is a device preference, while the terminal's grid must
 * still reach the remote program. Real pointer and keyboard input catches
 * clipping, capture and focus errors that pure parsing tests cannot see. */
import { expect, test } from "./helpers/evidence";
import { type Page } from "@playwright/test";
import { cleanupSession, createSession, openRowMenu } from "./helpers/fleet";
import { FAKE_AGENT_INVOCATION } from "./helpers/terminal-suite";
import { waitForTermText } from "./helpers/term";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";

/** Observe the content width; the existing border is deliberately extra. */
async function width(page: Page): Promise<number> {
  return page.locator(".app-sidebar").evaluate((el) => parseFloat(getComputedStyle(el).width));
}

/** Narrowing the list must not hide the actions that create its contents.
 * Visibility alone ignores sidebar clipping, so compare the actual boxes.
 */
async function headingActionsFit(page: Page): Promise<void> {
  const sidebar = (await page.locator(".app-sidebar").boundingBox())!;
  for (const selector of [
    ".hosts-heading .add-host-button",
    ".hosts-heading .update-all-button",
    ".session-heading .checkout-trash-button",
    ".session-heading .new-session-button",
    ".session-heading .templates-button",
  ]) {
    const control = page.locator(selector);
    await expect(control).toBeVisible();
    const box = (await control.boundingBox())!;
    expect(box.x, `${selector} starts inside the sidebar`).toBeGreaterThanOrEqual(sidebar.x);
    expect(box.x + box.width, `${selector} stays inside the sidebar`).toBeLessThanOrEqual(
      sidebar.x + sidebar.width - 1,
    );
  }
  const trash = (await page.locator(".session-heading .checkout-trash-button").boundingBox())!;
  const next = (await page.locator(".session-heading .new-session-button").boundingBox())!;
  expect(trash.height, "trash and New use the same control height").toBe(next.height);
  expect(trash.y, "the pair stays on the same wrapped header line").toBe(next.y);
  expect(next.x - trash.x - trash.width, "trash sits immediately left of New").toBeLessThanOrEqual(6);
}

/** Drag the real separator from its current position, including outside it. */
async function dragBy(page: Page, delta: number): Promise<void> {
  const handle = page.getByRole("separator", { name: "Sidebar width" });
  const box = (await handle.boundingBox())!;
  expect(box.width, "fixture: the separator has a usable pointer hit area").toBeGreaterThanOrEqual(5);
  const x = box.x + box.width / 2;
  const y = box.y + Math.min(200, box.height / 2);
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + delta, y, { steps: 8 });
  await page.mouse.up();
}

/** A fresh context has no stored width. Persistence, reset and both clamps
 * must be checked through gestures, rather than setting the CSS directly. */
test("sidebar drag clamps, persists, resets and refits the program", async ({ page, request }, info) => {
  const session = await createSession(request, {
    title: `sidebar-resize-${Date.now()}`,
    cwd: "/tmp",
    invocation: FAKE_AGENT_INVOCATION,
  });
  try {
    await page.setViewportSize({ width: 1280, height: 700 });
    await page.goto("/");
    const row = page.locator(`[data-session-id="${session.id}"]`);
    await expect(row).toBeVisible({ timeout: 20_000 });
    await row.locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    const handle = page.getByRole("separator", { name: "Sidebar width" });
    await expect(handle).toHaveAttribute("data-sidebar-bound", "true");
    expect(await width(page), "fixture: a fresh device starts at the default").toBe(340);
    await headingActionsFit(page);
    // The resize strip may use the terminal gutter, but its first content
    // column must still receive clicks for selection, links and mouse mode.
    const screen = (await page.locator("#terminal .xterm-screen").boundingBox())!;
    expect(await page.evaluate(({ x, y }) => {
      return !!document.elementFromPoint(x, y)?.closest(".xterm-screen");
    }, { x: screen.x + 0.5, y: screen.y + 20 })).toBe(true);
    await page.screenshot({ path: info.outputPath("sidebar-default.png") });

    await dragBy(page, 90);
    await expect.poll(() => width(page)).toBe(430);
    await expect(handle).toHaveAttribute("aria-valuenow", "430");
    await expect.poll(() => page.evaluate(() => localStorage.getItem("farhelm.sidebar-width"))).toBe("430");
    await page.reload();
    await waitForSessionRevealed(page, session.id);
    await expect.poll(() => width(page)).toBe(430);
    await expect(handle).toHaveAttribute("aria-valuenow", "430");

    await dragBy(page, 500);
    await expect.poll(() => width(page)).toBe(600);
    await headingActionsFit(page);
    // Focus is the premise: a keypress that went elsewhere would also leave
    // the width at the maximum and pass the clamp check vacuously.
    await handle.focus();
    await expect(handle).toBeFocused();
    await page.keyboard.press("ArrowRight");
    await expect.poll(() => width(page)).toBe(600);
    await expect(handle).toBeFocused();
    await page.screenshot({ path: info.outputPath("sidebar-maximum.png") });
    const before = await page.evaluate(() => (window as any).__farhelmTerm.cols);
    await dragBy(page, -500);
    await expect.poll(() => width(page)).toBe(240);
    await headingActionsFit(page);
    await expect.poll(() => page.evaluate(() => (window as any).__farhelmTerm.cols)).toBeGreaterThan(before);
    await page.screenshot({ path: info.outputPath("sidebar-minimum.png") });

    // Reveal and focus must finish before giving the separator the keyboard.
    // Otherwise an unrelated terminal handoff can make an arrow test flaky.
    await page.locator("#terminal").click();
    await expect(page.locator("#terminal .xterm-helper-textarea")).toBeFocused();
    const grid = await page.evaluate(() => {
      const t = (window as any).__farhelmTerm;
      return { rows: t.rows, cols: t.cols };
    });
    await expect(async () => {
      await page.keyboard.type("size");
      await page.keyboard.press("Enter");
      await waitForTermText(page, `size:${grid.rows} ${grid.cols}`, 2_000);
    }).toPass({ timeout: 15_000 });

    await handle.focus();
    await expect(handle).toBeFocused();
    await page.keyboard.press("ArrowRight");
    await expect.poll(() => width(page)).toBe(250);
    await expect(handle).toBeFocused();
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowLeft");
    await expect.poll(() => width(page)).toBe(240);
    await expect(handle).toBeFocused();
    await handle.dblclick({ position: { x: 3, y: 200 } });
    await expect.poll(() => width(page)).toBe(340);
    await expect.poll(() => page.evaluate(() => localStorage.getItem("farhelm.sidebar-width"))).toBe("340");
    // The stylesheet's own default is also 340, so the reloaded width proves
    // the reset persisted only once the width script has applied the stored
    // value; before that, a reset that left 240 stored would read 340 too.
    await page.reload();
    await expect(handle).toHaveAttribute("data-sidebar-bound", "true");
    await expect.poll(() => width(page)).toBe(340);
    await expect(handle).toHaveAttribute("aria-valuenow", "340");
  } finally {
    await cleanupSession(request, session.id);
  }
});

/** Width-derived chrome must change its threshold with the sidebar; testing
 * only 340px would let the old fixed media queries pass unnoticed. */
test("sidebar width moves narrow-window chrome and menu-pointer cutoffs", async ({ page, request }, info) => {
  const session = await createSession(request, {
    title: `sidebar-cutoffs-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.setViewportSize({ width: 1280, height: 700 });
    await page.goto("/");
    const row = page.locator(`[data-session-id="${session.id}"]`);
    await expect(row).toBeVisible({ timeout: 20_000 });
    await expect(page.getByRole("separator")).toHaveAttribute("data-sidebar-bound", "true");
    await dragBy(page, 260);
    await expect.poll(() => width(page)).toBe(600);
    await page.setViewportSize({ width: 800, height: 700 });
    await page.locator(".app-shell").evaluate((el) => el.classList.add("macos-window"));
    await expect(page.locator("html")).toHaveClass(/sidebar-narrow-window/);
    expect(await page.locator(".app-bar").evaluate((el) => getComputedStyle(el).position)).toBe("fixed");
    expect(await page.locator(".app-shell").evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
    await openRowMenu(row);
    await expect(row.locator(".session-row-menu-pointer")).toBeHidden();
    await page.screenshot({ path: info.outputPath("sidebar-narrow.png") });
    await page.setViewportSize({ width: 1000, height: 700 });
    await expect(page.locator("html")).not.toHaveClass(/sidebar-narrow-window/);
    await openRowMenu(row);
    await expect(row.locator(".session-row-menu-pointer")).toBeVisible();
  } finally {
    await request.delete(`/api/sessions/${session.id}`);
  }
});
