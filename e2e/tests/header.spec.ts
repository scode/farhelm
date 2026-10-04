/**
 * The consolidated session header (the 2026-08 UI refresh): status, title,
 * age, copyable fields, and six lifecycle actions folded into one row over
 * the tab strip. `session_view.rs`'s own docs carry the design; this file
 * proves the two properties that only a real layout engine can check —
 * that the row survives the SUPPORTED minimum width without clipping a
 * control, and that the truncated identity fields still carry their full
 * value somewhere a user can read it.
 */
import { expect, test } from "./helpers/evidence";
import { type Page } from "@playwright/test";
import {
  cleanupSession,
  createResumableSession,
  createSession,
  listSessions,
  openRowMenu,
  stopSession,
} from "./helpers/fleet";
import { waitForTermText } from "./helpers/term";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";
import { FAKE_AGENT_INVOCATION } from "./helpers/terminal-suite";

function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

// The header's six full labels need a 650px main pane. The app still permits
// a 320px pane, where its single row may clip; this test exercises the
// narrowest rounded width expected to keep all six actions visible. Measured
// when Delete joined the row: the actions end at a 598px pane in Chromium and
// a 608px pane in WebKit, and 650 keeps the same ~40px of platform font slack
// the old five-action 580 had over its own WebKit measurement.
const SIDEBAR_WIDTH = 340;
const SUPPORTED_MAIN_PANE_WIDTH = 650;
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
    // Resumable, because the test opens Restart's confirmation, and an
    // unavailable Restart opens nothing.
    const session = await createResumableSession(request, {
      title: `${marker}-${"t".repeat(200)}`,
      cwd: "/tmp",
      invocationSuffix: ` #${"x".repeat(200)}`,
    });
    try {
      await page.setViewportSize({ width: VIEWPORT_WIDTH, height: VIEWPORT_HEIGHT });
      await page.goto("/");
      await row(page, session.id).locator(".session-row-open").click();
      await waitForSessionRevealed(page, session.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      // Restart opens its confirmation (measured below) only while the
      // agent is working, and a quiet fixture agent reads idle after a few
      // samples; `busy` keeps its screen changing so it stays working.
      await page.locator("#terminal").click();
      await page.keyboard.type("busy");
      await page.keyboard.press("Enter");
      await waitForTermText(page, "busy-tick-");

      const restartButton = page.locator(".restart-primary");
      // Restart confirms once the agent is classified working — the same
      // signal the restart-confirmation tests wait on — which is also the
      // point at which a badge is guaranteed to exist (a live status is
      // always classified, so `status_badge` never suppresses it the way
      // `Unknown` does).
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
        ["restart with button", (await page.locator(".restart-with-trigger").boundingBox())!],
        ["replace with button", (await page.locator(".header-replace-with").boundingBox())!],
        ["delete button", (await page.locator(".header-delete").boundingBox())!],
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
 * The session header is the only surface that exposes all six lifecycle
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
      "restart with",
      "replace",
      "clone",
      "replace with",
      "delete",
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

/**
 * Why this matters: the header's command tooltip is the only full view of a
 * long command, and its copy button is how people take it to a shell. A host
 * or agent can put a direction override or a newline in the command, so the
 * shown text could read differently from the bytes copied. Spec: the header
 * shows the command escaped (`<U+202E>`, `<U+000A>`) in a direction-isolated
 * element and in its tooltip, the clipboard receives the exact raw bytes,
 * and after copying the button shows a readable hidden-characters warning;
 * an ordinary value keeps the ordinary feedback.
 */
test("header copy shows escaped peer text, copies raw bytes, and warns", async ({
  page,
  request,
}) => {
  const cwd = "/tmp";
  // Everything after `#` is a shell comment on both lines, so the agent
  // still starts normally.
  const invocation = `${FAKE_AGENT_INVOCATION} # \u202Eabc\n# tail`;
  // A long title and a narrow window crowd the header so the copy buttons
  // shrink, which is the condition that used to clip an in-button warning.
  await page.setViewportSize({ width: 900, height: 700 });
  const session = await createSession(request, {
    title: `header-peer-${Date.now()}-${"x".repeat(120)}`,
    cwd,
    invocation,
  });
  try {
    await page.goto("/");
    const sessionRow = row(page, session.id);
    await expect(sessionRow).toBeVisible();
    await sessionRow.locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");

    const command = page.locator(".titlebar .header-copy").nth(1);
    const shown = command.locator(".peer-value");
    await expect(shown).toHaveAttribute("dir", "ltr");
    await expect(shown).toContainText("<U+202E>abc<U+000A># tail");
    expect(await shown.textContent(), "no raw override reaches the page").not.toContain("\u202E");
    await expect(command).toHaveAttribute("title", /<U\+202E>abc<U\+000A># tail — click to copy$/);
    expect(
      await command.evaluate((el) => el.scrollWidth > el.clientWidth),
      "premise: the crowded header has shrunk the command button",
    ).toBe(true);

    await page.evaluate(() => {
      (window as any).__headerCopies = [];
      (window as any).__farhelmNativeClipboardWrite = (value: string) => {
        (window as any).__headerCopies.push(value);
      };
    });
    await command.click();
    await expect
      .poll(() => page.evaluate(() => (window as any).__headerCopies), {
        message: "the clipboard gets the exact raw command",
      })
      .toEqual([invocation]);
    const warning = page.locator(".titlebar .copy-warning");
    await expect(warning).toHaveText(
      "⚠ the copied command contains hidden characters, shown above as <U+…>",
    );
    // Readable, not merely present: nothing clips it and it lies inside the
    // viewport even in the crowded header this test sets up.
    const box = await warning.boundingBox();
    const viewport = page.viewportSize();
    expect(box && viewport && box.x >= 0 && box.x + box.width <= viewport.width).toBe(true);
    expect(await warning.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);

    // An ordinary value gets the ordinary feedback and no warning.
    await page.locator(".titlebar .header-copy").nth(0).click();
    await expect(page.locator(".titlebar .header-copy").nth(0).locator(".copy-feedback")).toHaveText(
      "✓ copied",
    );
    await expect(warning).toHaveCount(0);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * Deletes the open session from its header, and only after an answered
 * confirmation.
 *
 * Why this matters: the header's delete is a shortcut for the sidebar row's,
 * so it must be at least as careful and must end in the same place. It
 * confirms inline, anchored under the button, and cancelling sends nothing.
 * Confirming runs the list's own delete: the row leaves the sidebar, the main
 * pane stops showing the session, and the helm no longer lists it. The header
 * delete also confirms for an ended session the row would delete without
 * asking, and because that prompt said nothing was alive, the request carries
 * the supervisor-side precondition, the same guard the row's unconfirmed
 * delete sends.
 */
test("the header delete confirms in place and deletes through the list", async ({ page, request }) => {
  const liveTitle = `header-delete-live-${Date.now()}`;
  const live = await createSession(request, {
    title: liveTitle,
    cwd: "/tmp",
    invocation: FAKE_AGENT_INVOCATION,
  });
  const ended = await createSession(request, {
    title: `header-delete-ended-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  const deleteUrls: URL[] = [];
  await page.route("**/api/sessions/*", async (route) => {
    if (route.request().method() === "DELETE") {
      deleteUrls.push(new URL(route.request().url()));
    }
    await route.fallback();
  });
  try {
    await page.goto("/");
    await row(page, live.id).locator(".session-row-open").click();
    await waitForSessionRevealed(page, live.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    // Classified live (any live status), the point at which the header's
    // delete knows there is an agent to warn about.
    await expect(page.locator(".titlebar .status-badge")).toHaveClass(/\b(running|waiting|idle)\b/, {
      timeout: 15_000,
    });

    const deleteButton = page.locator(".header-delete");
    await expect(deleteButton).toHaveClass(/btn-danger/);
    const confirmation = page.locator(".header-delete-confirm");

    // Cancelling is free: nothing is sent and the session stays.
    await deleteButton.click();
    await expect(confirmation).toBeVisible();
    await expect(confirmation.locator(".confirm-consequence").first()).toContainText("still running");
    const buttonBox = (await deleteButton.boundingBox())!;
    const panelBox = (await confirmation.boundingBox())!;
    expect(panelBox.y, "the confirmation hangs beneath the delete button").toBeGreaterThanOrEqual(
      buttonBox.y + buttonBox.height - 1,
    );
    await confirmation.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(confirmation).toHaveCount(0);
    expect(deleteUrls).toHaveLength(0);

    await deleteButton.click();
    await confirmation.getByRole("button", { name: "delete", exact: true }).click();
    await expect(row(page, live.id)).toHaveCount(0, { timeout: 20_000 });
    // The main pane lets go of the deleted session. Auto-select may open
    // another one straight away, so the check is on whose header shows.
    await expect(page.locator(".titlebar .title")).not.toContainText(liveTitle);
    expect((await listSessions(request)).sessions.some((listed) => listed.id === live.id)).toBe(false);
    expect(deleteUrls).toHaveLength(1);
    expect(deleteUrls[0].searchParams.get("only_if_nothing_alive")).toBeNull();

    // An ended session still confirms here, and its "nothing alive" prompt
    // sends the precondition.
    await stopSession(request, ended.id);
    await expect
      .poll(
        async () => (await listSessions(request)).sessions.find((listed) => listed.id === ended.id)?.status?.state,
        { timeout: 20_000 },
      )
      .toBe("exited");
    await expect(row(page, ended.id).locator(".status-badge")).toHaveText(/exited/, { timeout: 20_000 });
    await row(page, ended.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar")).toBeVisible();
    await deleteButton.click();
    await expect(confirmation).toBeVisible();
    // The premise is the prompt the user reads, not the sidebar: the header
    // refreshes its own detail independently, and until it has the ended
    // status its prompt still warns of a live agent (and rightly sends no
    // precondition). "delete anyway:" is the ended, tab-less wording.
    await expect(confirmation.locator(".confirm-consequence").first()).toHaveText("delete anyway:", {
      timeout: 20_000,
    });
    await confirmation.getByRole("button", { name: "delete", exact: true }).click();
    await expect(row(page, ended.id)).toHaveCount(0, { timeout: 20_000 });
    expect(deleteUrls).toHaveLength(2);
    expect(deleteUrls[1].searchParams.get("only_if_nothing_alive")).toBe("true");
  } finally {
    await cleanupSession(request, live.id);
    await cleanupSession(request, ended.id);
  }
});

/**
 * A committed delete shows its progress on the row and in the header until
 * the supervisor's reply lands, then resolves either way.
 *
 * Why this matters: the supervisor answers a delete only once the session's
 * whole process tree is gone, which for a live agent takes a few seconds of
 * its own SIGTERM handling, and the row deliberately stays until then. Without
 * a visible state in between, the click looks ignored. Specifies, with the
 * DELETE reply held open by the test: the row gets its `deleting` state and a
 * "Stopping agent…" label in place of its title, and its open button, though
 * disabled like every row's during the delete, is not dimmed; the open
 * session's header shows the same label in place of its actions, and an
 * overlay over the terminal says it again. A refused delete (409) clears all three and leaves
 * the row with its error and the header with its actions; a delete that goes
 * through removes the row, and its own detach never paints the terminal's
 * "Detached" banner on the way.
 */
test("a committed delete shows its progress until the reply lands", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `delete-progress-${Date.now()}`,
    cwd: "/tmp",
    invocation: FAKE_AGENT_INVOCATION,
  });
  const build = (await request.get("/api/hosts")).headers()["x-farhelm-build"];
  expect(build, "the helm stamps every reply with its build").toBeTruthy();
  // Each DELETE waits for the test to release it, then either passes through
  // or is refused the way the helm relays a supervisor conflict.
  let release: ((outcome: "refuse" | "continue") => void) | undefined;
  const held = page.route(`**/api/sessions/${session.id}`, async (route) => {
    if (route.request().method() !== "DELETE") {
      await route.fallback();
      return;
    }
    const outcome = await new Promise<"refuse" | "continue">((resolve) => {
      release = resolve;
    });
    release = undefined;
    if (outcome === "refuse") {
      await route.fulfill({
        status: 409,
        headers: { "content-type": "text/plain", "x-farhelm-build": build },
        body: "held refusal",
      });
    } else {
      await route.fallback();
    }
  });
  await held;
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await target.locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    // Classified live (any live status), the point at which the header's
    // delete knows there is an agent to warn about.
    await expect(page.locator(".titlebar .status-badge")).toHaveClass(/\b(running|waiting|idle)\b/, {
      timeout: 15_000,
    });
    const rowProgress = target.locator(".delete-progress");
    const headerProgress = page.locator(".header-delete-progress");
    const overlay = page.locator(".terminal-delete-overlay");

    // Refused: progress shows while the reply is held, then gives way to the
    // row's error and the header's actions.
    await openRowMenu(target);
    await target.locator(".session-row-delete").click();
    await target.locator(".confirm-delete").click();
    await expect.poll(() => release !== undefined, { message: "the DELETE reached the route" }).toBe(true);
    await expect(target).toHaveClass(/\bdeleting\b/);
    await expect(rowProgress).toHaveText("Stopping agent…");
    await expect(target.locator(".session-title"), "the progress takes the title line's place").toHaveClass(/\bvisually-hidden\b/);
    // The delete disables every row's open button, and a disabled one is
    // drawn at half opacity. The deleting row's is exempt: its progress
    // words are the row's only marker, and half opacity would leave them
    // too faint to notice (an earlier, fainter indicator went unnoticed).
    await expect(target.locator(".session-row-open"), "the open button is disabled during the delete").toBeDisabled();
    await expect(target.locator(".session-row-open"), "the deleting row is not dimmed").toHaveCSS("opacity", "1");
    await expect(headerProgress).toHaveText("Stopping agent…");
    await expect(overlay).toHaveText("Stopping agent…");
    await expect(page.locator(".restart-primary")).toBeHidden();
    release!("refuse");
    await expect(target.locator(".action-error")).toContainText("delete: held refusal");
    await expect(target).not.toHaveClass(/\bdeleting\b/);
    await expect(rowProgress).toHaveCount(0);
    await expect(headerProgress).toHaveCount(0);
    await expect(overlay).toHaveCount(0);
    await expect(target.locator(".session-title")).not.toHaveClass(/\bvisually-hidden\b/);
    await expect(page.locator(".restart-primary")).toBeVisible();

    // Accepted, from the header this time: the same progress, then the row goes.
    await page.locator(".header-delete").click();
    await page.locator(".header-delete-confirm").getByRole("button", { name: "delete", exact: true }).click();
    await expect.poll(() => release !== undefined, { message: "the DELETE reached the route" }).toBe(true);
    await expect(rowProgress).toHaveText("Stopping agent…");
    await expect(headerProgress).toHaveText("Stopping agent…");
    await expect(overlay).toHaveText("Stopping agent…");
    // From here the real delete tears the terminal down. Whether its detach
    // reaches this view before the view goes away with the session depends
    // on the helm's feed and the socket racing, which the browser cannot
    // order; what holds either way is that nothing paints the banner until
    // the row is gone. That the banner is held, and painted if the delete
    // fails, is `terminal.spec.ts`'s "a detach banner is held while a delete
    // is in flight" test, which drives the detach deterministically.
    const banner = page.locator("#term-banner");
    await expect(banner, "premise: the banner exists and is hidden before the delete").toBeHidden();
    expect(await banner.count(), "premise: the banner element exists to be watched").toBe(1);
    await page.evaluate(() => {
      const w = window as any;
      w.__deleteBannerPainted = false;
      const node = document.getElementById("term-banner")!;
      new MutationObserver(() => {
        if (node.style.display === "block") w.__deleteBannerPainted = true;
      }).observe(node, { attributes: true, attributeFilter: ["style"] });
    });
    release!("continue");
    await expect(target).toHaveCount(0, { timeout: 20_000 });
    expect(
      await page.evaluate(() => (window as any).__deleteBannerPainted),
      "the delete's own detach must never have painted the terminal's banner",
    ).toBe(false);
  } finally {
    release?.("continue");
    await page.unrouteAll({ behavior: "ignoreErrors" });
    await cleanupSession(request, session.id);
  }
});
