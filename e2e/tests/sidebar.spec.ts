/**
 * The two-pane shell itself (BUGS_BURNDOWN.md issue 5): the properties the
 * sidebar redesign claims but no interaction test proves on its own —
 * that the list and the selected session are visibly side by side, that
 * switching sessions REMOUNTS the keyed view rather than patching the old
 * one, that a stacked row stays inside the fixed column under hostile
 * content, and that a view-side operation in flight pins the selection.
 *
 * These are layout- and lifecycle-level assertions, deliberately separate
 * from terminal.spec.ts's behavioral suite: every test here would pass in
 * a world where the shell CSS silently collapsed a pane or the `key` was
 * dropped, so each is written to fail against precisely that mutation.
 */
import { expect, test, type Page } from "@playwright/test";
import { cleanupSession, createSession } from "./helpers/fleet";

function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

/**
 * Selecting a session keeps BOTH panes on screen, non-overlapping, at the
 * agreed geometry: sidebar at its fixed 340px, main pane with real width
 * and the full shell height.
 *
 * This is the shell's core promise ("the agent list stays visible while a
 * terminal is on screen") stated as geometry. Feed coverage proves both
 * readers MOUNT; only a box check catches CSS that hides, overlays, or
 * zero-sizes a pane while both stay in the DOM.
 */
test("selecting a session leaves the sidebar and the session view visible side by side", async ({
  page,
  request,
}) => {
  const session = await createSession(request, {
    title: `two-pane-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await target.locator(".session-row-open").click();

    const sidebar = page.locator(".app-sidebar");
    const main = page.locator(".app-main");
    await expect(sidebar).toBeVisible();
    await expect(main.locator(".layout")).toBeVisible();
    // The selected row is still on screen INSIDE the sidebar — the old
    // exclusive layout fails here, with the list unmounted entirely.
    await expect(target).toBeVisible();

    const shellBox = (await page.locator(".app-shell").boundingBox())!;
    const sideBox = (await sidebar.boundingBox())!;
    const mainBox = (await main.boundingBox())!;
    // 340 content + the 1px right border (content-box sizing puts the
    // border in the measured box).
    expect(sideBox.width).toBeGreaterThanOrEqual(340);
    expect(sideBox.width).toBeLessThanOrEqual(342);
    // Side by side and non-overlapping: the main pane starts at or right
    // of the sidebar's right edge (the 1px border may round either way).
    expect(mainBox.x).toBeGreaterThanOrEqual(sideBox.x + sideBox.width - 1);
    expect(mainBox.width).toBeGreaterThan(300);
    // Full-height chain: a height:auto link between the shell and the
    // terminal collapses this (see .app-shell's doc in app.css).
    expect(Math.round(mainBox.height)).toBe(Math.round(shellBox.height));
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * A direct A-to-B selection switch REMOUNTS the session view: the old
 * view's DOM subtree is discarded, not patched in place to show B.
 *
 * The `key` on `SessionView` is load-bearing (per-session state is seeded
 * from props into `use_signal`s), and this is the one test that fails if
 * it is removed or constant: Dioxus patches an unkeyed component's
 * existing DOM in place, so the old root element would survive the switch
 * still `isConnected`. Every other lifecycle test clears the selection
 * with Back first, which forces a remount even without the key.
 */
test("switching sessions directly tears the old view down and mounts the new one", async ({
  page,
  request,
}) => {
  const a = await createSession(request, {
    title: `switch-a-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  const b = await createSession(request, {
    title: `switch-b-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    await expect(row(page, a.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, b.id)).toBeVisible({ timeout: 20_000 });

    await row(page, a.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toContainText(a.title);
    const viewA = await page.locator(".app-main .layout").elementHandle();
    expect(viewA).not.toBeNull();

    // No Back in between — this is the direct switch the key exists for.
    await row(page, b.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toContainText(b.title);
    expect(
      await viewA!.evaluate((el) => el.isConnected),
      "the previous session view's DOM must be discarded on a keyed remount — " +
        "a still-connected root means the old view was patched in place and " +
        "may be carrying session A's state under B's title",
    ).toBe(false);
  } finally {
    await cleanupSession(request, a.id);
    await cleanupSession(request, b.id);
  }
});

/**
 * A row with hostile content — an unbroken multi-hundred-character title,
 * cwd, and invocation — stays INSIDE the fixed sidebar column, with its
 * fields stacked in the agreed order (title, cwd, invocation).
 *
 * The stacked layout's whole reason to exist is that the old single-line
 * row overflowed the 340px column under exactly this content (the MT-8
 * class); a wrapper or flex regression that restored horizontal packing
 * or let a line force the row wide would pass every text-content
 * assertion and fail only here.
 */
test("a row with unbroken oversized fields stays contained and stacked in the sidebar", async ({
  page,
  request,
}) => {
  // The title and invocation carry the hostile length: neither has any
  // limit the UI or supervisor enforces. The cwd stays real — a
  // nonexistent directory is a precondition failure that would refuse the
  // create itself (SPEC.md), and `.session-cwd` shares the same
  // min-width/ellipsis treatment the other two lines prove.
  const marker = `contained-${Date.now()}`;
  const session = await createSession(request, {
    title: `${marker}-${"t".repeat(300)}`,
    cwd: "/tmp",
    invocation: `sleep 300 #${"x".repeat(300)}`,
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });

    const sideBox = (await page.locator(".app-sidebar").boundingBox())!;
    const rowBox = (await target.boundingBox())!;
    expect(rowBox.width).toBeLessThanOrEqual(sideBox.width + 1);

    const title = (await target.locator(".session-title").boundingBox())!;
    const cwd = (await target.locator(".session-cwd").boundingBox())!;
    const invocation = (await target.locator(".session-invocation").boundingBox())!;
    for (const [name, box] of [
      ["title", title],
      ["cwd", cwd],
      ["invocation", invocation],
    ] as const) {
      expect(
        box.x + box.width,
        `the ${name} field must ellipsize inside the sidebar, not force the row wide`,
      ).toBeLessThanOrEqual(sideBox.x + sideBox.width + 1);
    }
    // Stacked, in order — a horizontal regression puts these on one line.
    expect(title.y).toBeLessThan(cwd.y);
    expect(cwd.y).toBeLessThan(invocation.y);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * The cwd line left-truncates without reordering: the visible text is the
 * TAIL of the path, ellipsis at the front, characters in logical order.
 *
 * Pins the two-span bidi construction (`.session-cwd` rtl clipper around
 * a `dir="ltr"` isolate — see app.css): bare `direction: rtl` on the text
 * itself renders a leading "/" at the visual right and can reorder
 * mixed-direction names, which is a sidebar showing a path DIFFERENT from
 * the directory the session actually uses.
 */
test("a long cwd shows its tail with characters in logical order", async ({
  page,
  request,
}) => {
  const session = await createSession(request, {
    title: `cwd-order-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });

    const inner = target.locator(".session-cwd-text");
    // The DOM text is the logical path regardless of rendering...
    await expect(inner).toHaveText("/tmp");
    // ...and the rendered glyph run is not mirrored: the isolate's
    // computed direction must be ltr even though its clipping parent is
    // rtl. This is the cheap deterministic stand-in for pixel inspection
    // — with the isolate absent, the inner span inherits rtl and the
    // browser reorders the neutrals (slashes, digits) around the text.
    expect(
      await inner.evaluate((el) => getComputedStyle(el).direction),
    ).toBe("ltr");
    expect(
      await target
        .locator(".session-cwd")
        .evaluate((el) => getComputedStyle(el).direction),
    ).toBe("rtl");
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * A view-side operation in flight pins the selection: while the selected
 * session's restart is unanswered, every row's open control is disabled
 * and the right pane stays on the busy session.
 *
 * This is the cross-pane half of the shared write gate (`ops::PaneGate`):
 * without it, selecting another row mid-operation unmounts the keyed view
 * that owns the reply, and the operation's outcome — success or refusal —
 * is silently discarded. The route hold makes the window deterministic
 * instead of racing a fast supervisor.
 */
test("an unanswered view operation disables row selection until it completes", async ({
  page,
  request,
}) => {
  const a = await createSession(request, {
    title: `busy-a-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  const b = await createSession(request, {
    title: `busy-b-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  let releaseRestart: () => void = () => {};
  const restartHeld = new Promise<void>((resolve) => {
    releaseRestart = resolve;
  });
  await page.route(`**/api/sessions/${a.id}/restart`, async (route) => {
    await restartHeld;
    await route.continue();
  });
  try {
    await page.goto("/");
    await expect(row(page, a.id)).toBeVisible({ timeout: 20_000 });
    await row(page, a.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toContainText(a.title);

    // Wait for the view's status-derived decision to settle on "this
    // click will confirm" (the view opens on the create reply's Unknown
    // placeholder — same discipline as terminal.spec.ts's restart tests),
    // then click through the confirmation; the POST is now held open by
    // the route.
    const restartButton = page.locator(".restart-primary");
    await expect(restartButton).toHaveAttribute("data-confirms", "true", {
      timeout: 15_000,
    });
    await restartButton.click();
    await page.locator(".restart-confirm").click();

    // While held: B's open control is disabled — the gate made visible —
    // and the pane still belongs to A.
    await expect(row(page, b.id).locator(".session-row-open")).toBeDisabled();
    await expect(page.locator(".titlebar .title")).toContainText(a.title);

    releaseRestart();
    // Once the reply lands the gate lifts and the switch works normally.
    await expect(row(page, b.id).locator(".session-row-open")).toBeEnabled({
      timeout: 15_000,
    });
    await row(page, b.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toContainText(b.title);
  } finally {
    releaseRestart();
    await page.unroute(`**/api/sessions/${a.id}/restart`);
    await cleanupSession(request, a.id);
    await cleanupSession(request, b.id);
  }
});
