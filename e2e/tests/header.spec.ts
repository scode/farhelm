/**
 * The consolidated session header (the 2026-08 UI refresh): status, title,
 * age, copyable fields, and four lifecycle actions folded into one row over
 * the tab strip. `session_view.rs`'s own docs carry the design; this file
 * proves the two properties that only a real layout engine can check —
 * that the row survives the SUPPORTED minimum width without clipping a
 * control, and that the truncated identity fields still carry their full
 * value somewhere a user can read it.
 */
import { expect, test } from "./helpers/evidence";
import { type Page } from "@playwright/test";
import { createSession, cleanupSession } from "./helpers/fleet";
import { waitForTermText } from "./helpers/term";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";
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
      await waitForSessionRevealed(page, session.id);
      await waitForTermText(page, "FAKE-AGENT READY");

      const restartButton = page.locator(".restart-primary");
      // Restart is reachable the moment the agent is classified
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

      // The badge and action never shrink (`.titlebar .status-badge`
      // and `.titlebar-actions` are both `flex-shrink: 0`) and must
      // therefore stay fully on screen regardless of how much the copy
      // fields and title have to give up.
      const badgeBox = (await page.locator(".titlebar .status-badge").boundingBox())!;
      const restartBox = (await restartButton.boundingBox())!;
      for (const [name, box] of [
        ["badge", badgeBox],
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

test("oversized title and copy fields overflow their boxes, with full tooltips", async ({
  page,
  request,
}) => {
  const marker = `header-overflow-${Date.now()}`;
  // Distinct oversized values for the title and the two copy buttons, so a
  // bug that swapped the two `title` attributes
  // (or truncated one to the other's length) would show up as a mismatch
  // rather than passing by coincidence.
  const title = `${marker}-title-${"a".repeat(250)}`;
  // The readiness marker must come from the process this fixture actually launches.
  const invocation = `${FAKE_AGENT_INVOCATION} #${"b".repeat(250)}`;
  const session = await createSession(request, {
    title,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    const sessionRow = row(page, session.id);
    await expect(sessionRow, "the created session must be listed before opening it").toBeVisible();
    await sessionRow.locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(page.locator(".titlebar .title")).toHaveAttribute("title", title);
    await expect(page.locator(".titlebar .header-copy").nth(0)).toHaveAttribute("title", /\/tmp/);
    await expect(page.locator(".titlebar .header-copy").nth(1)).toHaveAttribute("title", `${invocation} — click to copy`);

    // `scrollWidth > clientWidth` is the DOM's own proof of a truncated
    // single-line box (`white-space: nowrap; overflow: hidden` on both
    // spans) — the only way the FULL string above is unreadable without
    // the tooltip this test also pins.
    const titleOverflows = await page.locator(".titlebar .title").evaluate(
      (el) => el.scrollWidth > el.clientWidth,
    );
    const commandOverflows = await page.locator(".titlebar .header-copy").nth(1).evaluate(
      (el) => el.scrollWidth > el.clientWidth,
    );
    expect(titleOverflows, "the title must actually be clipped, or the tooltip is untested").toBe(
      true,
    );
    expect(commandOverflows, "the command field must actually be clipped, or the tooltip is untested").toBe(
      true,
    );
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * The directory and command fields must use the room the header has before
 * truncating. They used to be capped at a fixed 18 characters, so even an
 * ordinary command ellipsized in a wide window while a large empty gap sat
 * before the actions. In a wide viewport, a normal-length title, directory,
 * and command must all render in full.
 */
test("copy fields use the header's free width before truncating", async ({ page, request }) => {
  await page.setViewportSize({ width: 1920, height: 800 });
  // A command of fixed, known length rather than the fake agent's absolute
  // binary path, whose length depends on where the checkout lives: 60
  // characters is far past the old 18-character cap yet leaves a 1920px
  // window ample room for the title, directory, and every action. The
  // header renders from the session record, so no agent output is needed.
  const invocation = `sleep 300 #${"x".repeat(49)}`;
  const session = await createSession(request, {
    title: `header-width-${Date.now()}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    const sessionRow = row(page, session.id);
    await expect(sessionRow, "the created session must be listed before opening it").toBeVisible();
    await sessionRow.locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await expect(page.locator(".titlebar .header-copy").nth(1)).toHaveAttribute(
      "title",
      `${invocation} — click to copy`,
    );
    const clipped = (selector: string, index = 0) =>
      page.locator(selector).nth(index).evaluate((el) => el.scrollWidth > el.clientWidth);
    expect(await clipped(".titlebar .title"), "a short title must not be clipped").toBe(false);
    expect(await clipped(".titlebar .header-copy", 0), "the directory must not be clipped").toBe(false);
    expect(await clipped(".titlebar .header-copy", 1), "the command must not be clipped").toBe(false);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * The session header is the only surface that exposes all four lifecycle
 * actions together. This test pins their shared keyboard order, proves that
 * both copy buttons hand their complete values to the native bridge, and
 * verifies that header actions reuse the existing composer prefill paths.
 */
test("header actions stay ordered, copy full values, and open the right flows", async ({
  page,
  request,
}) => {
  const cwd = "/tmp";
  const invocation = `${FAKE_AGENT_INVOCATION} #'header copy'`;
  const session = await createSession(request, {
    title: `header-actions-${Date.now()}`,
    cwd,
    invocation,
  });
  try {
    await page.goto("/");
    const sessionRow = row(page, session.id);
    await expect(sessionRow, "the created session must be listed before opening it").toBeVisible();
    await sessionRow.locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");

    const actionNames = await page.locator(".titlebar-actions button").evaluateAll((buttons) =>
      buttons.map((button) => button.textContent?.trim()),
    );
    expect(actionNames, "pointer and keyboard users must receive the same action order").toEqual([
      "restart",
      "replace",
      "clone",
      "replace with",
    ]);

    await page.evaluate(() => {
      (window as any).__headerCopies = [];
      (window as any).__farhelmNativeClipboardWrite = (value: string) => {
        (window as any).__headerCopies.push(value);
      };
    });
    const copyButtons = page.locator(".titlebar .header-copy");
    await copyButtons.nth(0).click();
    await expect
      .poll(() => page.evaluate(() => (window as any).__headerCopies), {
        message: "the directory copy must reach the native bridge untruncated",
      })
      .toEqual([cwd]);
    await expect(copyButtons.nth(0)).toContainText("copied");

    await copyButtons.nth(1).click();
    await expect
      .poll(() => page.evaluate(() => (window as any).__headerCopies), {
        message: "the command copy must reach the native bridge untruncated",
      })
      .toEqual([cwd, invocation]);
    await expect(copyButtons.nth(1)).toContainText("copied");

    const replace = page.getByRole("button", { name: "replace", exact: true });
    await replace.click();
    const confirmation = page.locator(".header-replace-confirm");
    await expect(confirmation).toBeVisible();
    await expect(confirmation.getByRole("button", { name: "replace", exact: true })).toHaveClass(
      /btn-danger/,
    );
    await confirmation.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(confirmation).toHaveCount(0);

    const form = page.locator(".create-session-form");
    await page.getByRole("button", { name: "clone", exact: true }).click();
    await expect(form).toBeVisible();
    await expect(form.locator('input[aria-label="folder"]')).toHaveValue(cwd);
    await form.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(form).toHaveCount(0);

    await page.getByRole("button", { name: "replace with", exact: true }).click();
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-submit")).toHaveText(/^replace\s+local \(this machine\) · \/tmp$/);
    await form.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(form).toHaveCount(0);
  } finally {
    await cleanupSession(request, session.id);
  }
});
