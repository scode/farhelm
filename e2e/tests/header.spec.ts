/**
 * The consolidated session header (TODO.md's UI refresh): title, metadata,
 * status badge, and the archive/restart actions folded into one row over
 * the tab strip. `session_view.rs`'s own docs carry the design; this file
 * proves the two properties that only a real layout engine can check —
 * that the row survives the SUPPORTED minimum width without clipping a
 * control, and that the truncated identity fields still carry their full
 * value somewhere a user can read it.
 */
import { expect, test, type Page } from "@playwright/test";
import { createSession, cleanupSession } from "./helpers/fleet";
import { waitForTermText } from "./helpers/term";
import { FAKE_AGENT_INVOCATION } from "./helpers/terminal-suite";

function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

// `.app-main`'s own floor (app.css): the sidebar is a fixed 340px and the
// main pane refuses to shrink below 320px, so this is the narrowest the
// header is ever asked to fit into without the shell itself scrolling.
const SIDEBAR_WIDTH = 340;
const SUPPORTED_MAIN_PANE_WIDTH = 320;
const VIEWPORT_WIDTH = SIDEBAR_WIDTH + SUPPORTED_MAIN_PANE_WIDTH;
const VIEWPORT_HEIGHT = 600;

test(
  "the header stays one row and every control stays reachable at the supported minimum width",
  async ({ page, request }) => {
    const marker = `header-geometry-${Date.now()}`;
    // The title carries the hostile length; the invocation is the real
    // fake-agent command with a shell comment appended to pad it the same
    // way sidebar.spec.ts's oversized-fields test does — `#` starts a
    // comment, so the agent that actually launches is unaffected. `cwd`
    // stays a real directory: a nonexistent one is a precondition failure
    // SPEC.md has the create route refuse outright.
    const session = await createSession(request, {
      title: `${marker}-${"t".repeat(200)}`,
      cwd: "/tmp",
      invocation: `${FAKE_AGENT_INVOCATION} #${"x".repeat(200)}`,
    });
    try {
      await page.setViewportSize({ width: VIEWPORT_WIDTH, height: VIEWPORT_HEIGHT });
      await page.goto("/");
      await row(page, session.id).locator(".session-row-open").click();
      await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
      await waitForTermText(page, "FAKE-AGENT READY");

      const restartButton = page.locator(".restart-primary");
      const archiveButton = page.locator(".archive-primary");
      // Both controls are reachable the moment the agent is classified
      // live — the same signal the restart-confirmation tests wait on —
      // which is also the point at which a badge is guaranteed to exist
      // (a live status is always classified, so `status_badge` never
      // suppresses it the way `Unknown` does).
      await expect(restartButton).toHaveAttribute("data-confirms", "true", {
        timeout: 15_000,
      });

      // One row: the header's `min-height: 40px` target should never need
      // to grow past a couple of pixels of platform font-metric slack, and
      // never, under the oversized fields above, wrap to two.
      const headerBox = (await page.locator(".titlebar").boundingBox())!;
      expect(headerBox.height, "the header must stay one row even under long fields").toBeLessThanOrEqual(
        48,
      );

      // The badge and both actions never shrink (`.titlebar .status-badge`
      // and `.titlebar-actions` are both `flex-shrink: 0`) and must
      // therefore stay fully on screen regardless of how much `.title` and
      // `.meta` have to give up.
      const badgeBox = (await page.locator(".titlebar .status-badge").boundingBox())!;
      const archiveBox = (await archiveButton.boundingBox())!;
      const restartBox = (await restartButton.boundingBox())!;
      for (const [name, box] of [
        ["badge", badgeBox],
        ["archive button", archiveBox],
        ["restart button", restartBox],
      ] as const) {
        expect(box.x, `the ${name} must not be pushed off the left edge`).toBeGreaterThanOrEqual(0);
        expect(
          box.x + box.width,
          `the ${name} must stay fully inside the ${VIEWPORT_WIDTH}px viewport`,
        ).toBeLessThanOrEqual(VIEWPORT_WIDTH + 1);
      }

      // The one-badge rule's non-stale half, asserted here because this
      // test already has a live, non-stale session on screen — cheaper
      // than a dedicated test, and `status_badge_destination`'s own unit
      // test already covers the logic; this is the render actually
      // obeying it.
      await expect(page.locator(".titlebar .status-badge")).toHaveCount(1);

      const tabStripBoxBefore = (await page.locator(".tab-strip").boundingBox())!;

      // Opening a popover must not reflow anything below the header: the
      // panel is `position: absolute`, out of flow, so the tab strip's own
      // box is the cheapest proof that holds.
      await archiveButton.click();
      const archivePanel = page.locator("#archive-confirm-panel");
      await expect(archivePanel).toBeVisible();
      const archivePanelBox = (await archivePanel.boundingBox())!;
      expect(
        (await page.locator(".tab-strip").boundingBox())!,
        "an open archive confirmation must not move the tab strip",
      ).toEqual(tabStripBoxBefore);
      expect(archivePanelBox.x).toBeGreaterThanOrEqual(0);
      expect(archivePanelBox.x + archivePanelBox.width).toBeLessThanOrEqual(VIEWPORT_WIDTH + 1);
      expect(archivePanelBox.y + archivePanelBox.height).toBeLessThanOrEqual(VIEWPORT_HEIGHT + 1);
      expect(
        archivePanelBox.y,
        "the archive confirmation must hang BENEATH the button that opened it",
      ).toBeGreaterThanOrEqual(archiveBox.y + archiveBox.height - 1);
      await page.locator(".archive-cancel").click();
      await expect(archivePanel).toHaveCount(0);

      await restartButton.click();
      const restartPanel = page.locator("#restart-confirm-panel");
      await expect(restartPanel).toBeVisible();
      const restartPanelBox = (await restartPanel.boundingBox())!;
      expect(
        (await page.locator(".tab-strip").boundingBox())!,
        "an open restart confirmation must not move the tab strip",
      ).toEqual(tabStripBoxBefore);
      expect(restartPanelBox.x).toBeGreaterThanOrEqual(0);
      expect(restartPanelBox.x + restartPanelBox.width).toBeLessThanOrEqual(VIEWPORT_WIDTH + 1);
      expect(restartPanelBox.y + restartPanelBox.height).toBeLessThanOrEqual(VIEWPORT_HEIGHT + 1);
      expect(
        restartPanelBox.y,
        "the restart confirmation must hang BENEATH the button that opened it",
      ).toBeGreaterThanOrEqual(restartBox.y + restartBox.height - 1);
      await page.locator(".restart-cancel").click();
      await expect(restartPanel).toHaveCount(0);
    } finally {
      await cleanupSession(request, session.id);
    }
  },
);

test("oversized title and metadata overflow their box, and their tooltip carries the full value", async ({
  page,
  request,
}) => {
  const marker = `header-overflow-${Date.now()}`;
  // Distinct oversized values for title and for the cwd/invocation pair
  // that makes up `.meta`, so a bug that swapped the two `title` attributes
  // (or truncated one to the other's length) would show up as a mismatch
  // rather than passing by coincidence.
  const title = `${marker}-title-${"a".repeat(250)}`;
  const invocation = `sleep 300 #${"b".repeat(250)}`;
  const session = await createSession(request, {
    title,
    cwd: "/tmp",
    invocation,
  });
  const expectedMeta = `/tmp — ${invocation}`;
  try {
    await page.goto("/");
    await row(page, session.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toHaveAttribute("title", title);
    await expect(page.locator(".titlebar .meta")).toHaveAttribute("title", expectedMeta);

    // `scrollWidth > clientWidth` is the DOM's own proof of a truncated
    // single-line box (`white-space: nowrap; overflow: hidden` on both
    // spans) — the only way the FULL string above is unreadable without
    // the tooltip this test also pins.
    const titleOverflows = await page.locator(".titlebar .title").evaluate(
      (el) => el.scrollWidth > el.clientWidth,
    );
    const metaOverflows = await page.locator(".titlebar .meta").evaluate(
      (el) => el.scrollWidth > el.clientWidth,
    );
    expect(titleOverflows, "the title must actually be clipped, or the tooltip is untested").toBe(
      true,
    );
    expect(metaOverflows, "the meta line must actually be clipped, or the tooltip is untested").toBe(
      true,
    );
  } finally {
    await cleanupSession(request, session.id);
  }
});
