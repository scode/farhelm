/**
 * The app shell never scrolls: only the sidebar list and the terminal
 * viewport own a wheel.
 *
 * Why this exists: with enough session rows to overflow the sidebar, the
 * DOCUMENT itself became scrollable and a wheel over the main pane's
 * header or over the sidebar's edge scrolled the whole app off its ground
 * (TODO.md, "Stop the whole app scrolling"). The cause was a containing
 * block, not a height: every row's screen-reader-only spans are
 * `position: absolute`, and with no positioned ancestor they resolved
 * against the initial containing block, so an overflow scroller that was
 * not their containing block could not clip them and the document grew to
 * reach the last one. `.app-sidebar` and `.session-list` are now positioned
 * (their app.css rules say why), and `html` refuses wheel and keyboard
 * scrolling (programmatic scrolls such as focus can still move an
 * `overflow: hidden` viewport, which is why the overflow assertion below,
 * not the scroll-position one, is the real regression guard).
 *
 * What is asserted: with more rows than the viewport holds, the document
 * has no scrollable overflow; a wheel over the header and over the main
 * pane's left edge (just past the sidebar's border, a region no scroller
 * owns) leaves the document at scroll position zero while a wheel over
 * the list still scrolls the sidebar. The sidebar starts the wheel phase
 * at scroll position zero by explicit reset, so the positive case is the
 * delivery oracle for the negative ones — it proves wheel events reached
 * the page and that the sidebar is the element that took them — and the
 * negative assertions need no settle delay. Row menus keep escaping the
 * sidebar's clip by being `position: fixed`, which a containing block on
 * the sidebar does not change; that is not re-asserted here.
 */
import { expect, test } from "./helpers/evidence";
import { cleanupSession, createSession } from "./helpers/fleet";

/** Enough rows to push the last one well past a 400px-tall viewport, so a
 * leaked containing block would give the document several screens of
 * overflow rather than a rounding-error's worth. */
const FILLER_ROWS = 20;

test("a wheel over the header or the sidebar edge never scrolls the document", async ({
  page,
  request,
}) => {
  const stamp = Date.now();
  const target = await createSession(request, {
    title: `shell-scroll-target-${stamp}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  const fillers: string[] = [];
  try {
    for (let i = 0; i < FILLER_ROWS; i++) {
      const filler = await createSession(request, {
        title: `shell-scroll-filler-${i}-${stamp}`,
        cwd: "/tmp",
        invocation: "sleep 300",
      });
      fillers.push(filler.id);
    }
    await page.setViewportSize({ width: 900, height: 400 });
    await page.goto("/");
    // Fixture premise: the list holds every row this test created, so the
    // sidebar genuinely overflows the viewport before anything is measured.
    await expect
      .poll(() => page.locator("[data-session-id]").count(), { timeout: 20_000 })
      .toBeGreaterThanOrEqual(FILLER_ROWS + 1);
    await page.locator(`[data-session-id="${target.id}"]`).click();
    await expect(page.locator(".app-main .titlebar")).toBeVisible({ timeout: 20_000 });

    const sidebar = page.locator(".app-sidebar");
    const sidebarOverflow = await sidebar.evaluate((el) => el.scrollHeight - el.clientHeight);
    expect(sidebarOverflow, "the sidebar must overflow for this test to mean anything").toBeGreaterThan(400);

    // The leak itself: the document must fit its viewport exactly, however
    // tall the sidebar's content is.
    const documentOverflow = await page.evaluate(() =>
      document.documentElement.scrollHeight - document.documentElement.clientHeight
    );
    expect(documentOverflow).toBe(0);

    const sidebarBox = (await sidebar.boundingBox())!;
    // Fixture premise for the wheel phase: the click above scrolled the
    // target row into view, and with the target created before every
    // filler it sits at the bottom of the activity-ordered list, so the
    // sidebar is already scrolled. The delivery oracle below is only an
    // oracle if it starts from zero.
    await sidebar.evaluate((el) => {
      el.scrollTop = 0;
    });
    await expect.poll(() => sidebar.evaluate((el) => el.scrollTop)).toBe(0);
    // Over the main pane's header, then over the main pane's left edge two
    // pixels past the sidebar's border (the "thin bar" of the report; the
    // border pixel itself belongs to the sidebar and would scroll it):
    // neither region has a scroller of its own, so a chained scroll here
    // can only be the document's.
    await page.mouse.move(sidebarBox.x + sidebarBox.width + 120, 12);
    await page.mouse.wheel(0, 300);
    await page.mouse.move(sidebarBox.x + sidebarBox.width + 2, 200);
    await page.mouse.wheel(0, 300);
    // Delivery oracle: the same gesture over the list scrolls the sidebar.
    await page.mouse.move(sidebarBox.x + 120, 200);
    await page.mouse.wheel(0, 300);
    await expect.poll(() => sidebar.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);

    const documentScroll = await page.evaluate(() => ({
      html: document.documentElement.scrollTop,
      body: document.body.scrollTop,
      window: window.scrollY,
    }));
    expect(documentScroll).toEqual({ html: 0, body: 0, window: 0 });
    const shellScroll = await page.locator(".app-shell").evaluate((el) => el.scrollTop);
    expect(shellScroll).toBe(0);
  } finally {
    // Every row this test made must go, whatever happens to the others: the
    // suite shares one helm across spec files, and twenty leaked
    // five-minute sessions would skew the row counts and newest-first
    // auto-select later specs assert on. So clean up all of them and
    // only then surface the first failure.
    const results = await Promise.allSettled(
      [target.id, ...fillers].map((id) => cleanupSession(request, id)),
    );
    const failed = results.find((r) => r.status === "rejected");
    if (failed && failed.status === "rejected") throw failed.reason;
  }
});
