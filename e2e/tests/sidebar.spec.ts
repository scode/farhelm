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
 *
 * The suite grew with the redesign's later PRs and now also covers the
 * row actions menu's own contracts (containment, float, selection
 * isolation, rename-in-panel, consequence wrap, stale-menu
 * reconciliation) and the sidebar's on-demand chrome (hosts/filter
 * toggles, the compact host strip, the applied-filter note).
 *
 * Like every per-area file since M6.5, new coverage starts its own spec
 * rather than growing terminal.spec.ts: that file's size already makes it
 * the suite's slowest project, and an area file keeps one subject's tests
 * findable and runnable together.
 */
import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import {
  cleanupSession,
  createSession,
  openFilterBar,
  openHostsPanel,
  openRowMenu,
  pinAutoSelect,
  stubFeed,
} from "./helpers/fleet";

function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

/**
 * Clean up every session in `sessions`, even when some cleanups fail.
 *
 * A plain loop that stopped at the first failure would abandon every
 * session after it — real sessions on the SHARED stack, still consuming a
 * pty and a tmux pane, left for every later test to trip over. Every
 * cleanup is attempted regardless of earlier failures; their errors are
 * collected and reported together once the sweep is done, rather than
 * losing all but the first.
 */
async function cleanupAll(
  request: APIRequestContext,
  sessions: { id: string }[],
): Promise<void> {
  const failures: string[] = [];
  for (const session of sessions) {
    try {
      await cleanupSession(request, session.id);
    } catch (error) {
      failures.push(`${session.id}: ${error instanceof Error ? error.message : String(error)}`);
    }
  }
  if (failures.length > 0) {
    throw new Error(`cleanup failed for ${failures.length} session(s):\n${failures.join("\n")}`);
  }
}

/**
 * Create enough sessions that `.app-sidebar` — the sidebar's real vertical
 * scroll container — must scroll to show them all, regardless of engine or
 * viewport font metrics.
 *
 * `.session-list` itself carries `overflow-y: auto` in the stylesheet but
 * no height constraint of its own, so it never actually scrolls; the
 * bounded-height ancestor that does is `.app-sidebar` (see its own
 * `onscroll` doc in lib.rs, and app.css's comment beside `.session-list`).
 * A FIXED count rather than a computed one, because row height depends on
 * font metrics this suite does not control — callers additionally poll for
 * `scrollHeight > clientHeight` before trusting the fixture rather than
 * trusting this number alone.
 *
 * Failure-atomic: a `createSession` failing partway through must not
 * strand whichever sessions DID get created — this function's own caller
 * never reaches its `finally` if this call throws before returning, so
 * nothing else would ever clean those up. Every already-created id is
 * torn down (via `cleanupAll`, so one cleanup failing does not mask
 * another) before the original error is rethrown.
 */
async function fillSidebarPastOneScreen(
  request: APIRequestContext,
  marker: string,
): Promise<{ id: string }[]> {
  const created: { id: string }[] = [];
  try {
    for (let i = 0; i < 18; i++) {
      created.push(
        await createSession(request, {
          title: `${marker}-${i}`,
          cwd: "/tmp",
          invocation: "sleep 300",
        }),
      );
    }
  } catch (error) {
    await cleanupAll(request, created);
    throw error;
  }
  return created;
}

/**
 * Wait for the always-visible compact host strip's own MOUNT-TIME read to
 * land, before opening a row menu whose test cares WHY the menu later
 * closes (or stays open).
 *
 * `hosts_strip_shape` (list.rs) is one of the six signals the menu's own
 * close-on-layout-shift effect watches: the strip changing shape — its
 * "loading hosts…" note giving way to real chips, or a refresh-error line
 * — is itself an internal layout cause, indistinguishable from whatever
 * cause a test means to isolate (a toggle click, a real scroll, a resize)
 * unless this read has already settled before the menu opens. Without
 * this, a dismissal test can pass for the wrong reason (closed by the
 * strip's own async landing, not by the cause under test), and a
 * stays-open test can flake on unrelated host-read timing.
 */
async function waitForHostsStripSettled(page: Page): Promise<void> {
  await expect(
    page.locator(".hosts-compact-note", { hasText: "loading hosts" }),
  ).toHaveCount(0, { timeout: 20_000 });
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
  let b: { id: string } | undefined;
  try {
    b = await createSession(request, {
      title: `switch-b-${Date.now()}`,
      cwd: "/tmp",
      invocation: "sleep 300",
    });
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
    if (b) await cleanupSession(request, b.id);
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
    if (b) await cleanupSession(request, b.id);
  }
});

/**
 * The actions menu's containment contract: every per-row action, and the
 * confirm exchange that replaces them, lives INSIDE the floating panel —
 * absent from the DOM while the menu is closed, mounted only as panel
 * descendants while it is open.
 *
 * This is the popup PR's central behavior stated directly. Every migrated
 * test locates controls relative to the whole row, so an implementation
 * that left the buttons inline beside an empty popup would keep the rest
 * of the suite green; only descendant-scoped locators catch it.
 */
test("row actions exist only inside the open actions panel", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `containment-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });

    // Closed: no actions anywhere in the row, only open + toggle.
    for (const control of [
      ".session-row-rename",
      ".session-row-stop",
      ".session-row-archive",
      ".session-row-delete",
    ]) {
      await expect(target.locator(control)).toHaveCount(0);
    }

    await openRowMenu(target);
    // Open: each control exists exactly once in the row, and that one
    // instance is a descendant of the panel — the two counts together
    // prove "inside the panel" rather than "somewhere in the row".
    for (const control of [
      ".session-row-rename",
      ".session-row-stop",
      ".session-row-archive",
      ".session-row-delete",
    ]) {
      await expect(target.locator(control)).toHaveCount(1);
      await expect(target.locator(`.session-row-menu-panel ${control}`)).toHaveCount(1);
    }

    // A destructive click swaps the SAME panel's contents for the
    // confirm exchange — the action items leave, the prompt arrives, all
    // without a second surface.
    await target.locator(".session-row-delete").click();
    await expect(target.locator(".session-row-menu-panel .confirm-consequence")).toBeVisible();
    await expect(target.locator(".session-row-stop")).toHaveCount(0);
    await expect(target.locator(".session-row-menu-panel .confirm-cancel")).toBeVisible();
    await target.locator(".confirm-cancel").click();
    await expect(target.locator(".session-row-menu-panel .session-row-stop")).toBeVisible();

    // Closing the toggle empties the row of actions again.
    await target.locator(".session-row-menu").click();
    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(target.locator(".session-row-stop")).toHaveCount(0);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * The panel FLOATS: opening one row's menu must not move the rows below
 * it.
 *
 * The whole reason the panel is absolutely positioned is that a menu that
 * reflowed the list would move the very row the user is acting on;
 * geometry inside the panel cannot catch a regression to normal-flow
 * positioning, only the next row's box can.
 */
test("opening a menu does not move the rows below it", async ({ page, request }) => {
  const a = await createSession(request, {
    title: `float-a-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  let b: { id: string } | undefined;
  try {
    b = await createSession(request, {
      title: `float-b-${Date.now()}`,
      cwd: "/tmp",
      invocation: "sleep 300",
    });
    await page.goto("/");
    await expect(row(page, a.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, b.id)).toBeVisible({ timeout: 20_000 });

    // Rows sort newest-first or by some stable order; whichever of the
    // two is FIRST gets its menu opened, and the OTHER is the one that
    // must not move.
    const firstIsA = (await row(page, a.id).boundingBox())!.y < (await row(page, b.id).boundingBox())!.y;
    const first = firstIsA ? row(page, a.id) : row(page, b.id);
    const second = firstIsA ? row(page, b.id) : row(page, a.id);

    const before = (await second.boundingBox())!;
    await openRowMenu(first);
    await expect(first.locator(".session-row-menu-panel")).toBeVisible();
    const after = (await second.boundingBox())!;
    expect(after.y).toBe(before.y);
    expect(after.x).toBe(before.x);
  } finally {
    await cleanupSession(request, a.id);
    if (b) await cleanupSession(request, b.id);
  }
});

/**
 * Opening another row's menu leaves the selected session alone: the
 * toggle is an inspection control, never an implicit row-open.
 *
 * Guards against event bubbling or a future row-level click handler
 * making the ellipsis behave like the open button — which would replace
 * the terminal the user is working in just for peeking at another
 * session's actions.
 */
test("opening another row's menu does not change the selection", async ({ page, request }) => {
  const a = await createSession(request, {
    title: `keep-sel-a-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  let b: { id: string } | undefined;
  try {
    b = await createSession(request, {
      title: `keep-sel-b-${Date.now()}`,
      cwd: "/tmp",
      invocation: "sleep 300",
    });
    await page.goto("/");
    await expect(row(page, a.id)).toBeVisible({ timeout: 20_000 });
    await row(page, a.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toContainText(a.title);

    await openRowMenu(row(page, b.id));
    await expect(row(page, b.id).locator(".session-row-menu-panel")).toBeVisible();
    // Still A's session view, still mounted — the menu open was not a
    // navigation.
    await expect(page.locator(".titlebar .title")).toContainText(a.title);
  } finally {
    await cleanupSession(request, a.id);
    if (b) await cleanupSession(request, b.id);
  }
});

/**
 * Rename happens inside the panel, with the row's open button visible
 * but disabled for the duration — cancel restores the action items and
 * re-enables navigation.
 *
 * Pins the rename form's new home (a panel descendant, not an inline
 * row swap) and the navigation lock around an open editor: an enabled
 * open button would let one stray click abandon the edit implicitly.
 */
test("rename lives in the panel and locks the row's open button while editing", async ({
  page,
  request,
}) => {
  const session = await createSession(request, {
    title: `rename-panel-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await openRowMenu(target);
    await target.locator(".session-row-rename").click();

    await expect(target.locator(".session-row-menu-panel .rename-form")).toBeVisible();
    await expect(target.locator(".session-row-open")).toBeVisible();
    await expect(target.locator(".session-row-open")).toBeDisabled();

    await target.locator(".rename-cancel").click();
    await expect(target.locator(".rename-form")).toHaveCount(0);
    await expect(target.locator(".session-row-menu-panel .session-row-rename")).toBeVisible();
    await expect(target.locator(".session-row-open")).toBeEnabled();
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * The archive consequence — the longest safety sentence a panel shows —
 * wraps inside the panel with its full text readable.
 *
 * The panel relies on the consequence wrapping (`.confirm-consequence`
 * inherits normal white-space there); a stray `nowrap` would clip or
 * push the "what will be destroyed" half out of the 300px panel right
 * before the user confirms, and string assertions cannot see that.
 */
test("the archive consequence wraps fully visible inside the panel", async ({
  page,
  request,
}) => {
  const session = await createSession(request, {
    title: `wrap-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    // A LIVE session's archive confirms with the longest wording (the
    // agent will be killed); wait for the live badge so the click takes
    // the confirming branch rather than archiving outright.
    await expect(target.locator(".status-badge")).toHaveText(/running|idle|waiting/, {
      timeout: 30_000,
    });
    await openRowMenu(target);
    await target.locator(".session-row-archive").click();

    const consequence = target.locator(".session-row-menu-panel .confirm-consequence");
    await expect(consequence).toBeVisible();
    const panelBox = (await target.locator(".session-row-menu-panel").boundingBox())!;
    const box = (await consequence.boundingBox())!;
    expect(box.x).toBeGreaterThanOrEqual(panelBox.x - 1);
    expect(box.x + box.width).toBeLessThanOrEqual(panelBox.x + panelBox.width + 1);
    expect(box.y + box.height).toBeLessThanOrEqual(panelBox.y + panelBox.height + 1);
    // Wrapped, not horizontally clipped: everything the element holds is
    // painted within its own box.
    expect(
      await consequence.evaluate((el) => el.scrollWidth <= el.clientWidth + 1),
      "the consequence must wrap rather than clip horizontally",
    ).toBe(true);

    await target.locator(".archive-cancel").click();
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * The sidebar's resting chrome is rows-plus-create: the hosts panel and
 * the filter bar are closed until toggled, while the compact host strip
 * keeps SPEC.md's per-host connection state (name and phase word) on
 * screen the whole time.
 *
 * This is the interviewed contents decision (BUGS_BURNDOWN.md issue 5)
 * plus the SPEC amendment's compact-indicator half stated as one test: a
 * regression that either re-inlines a panel permanently or drops the
 * strip (losing the always-visible phase) fails here.
 */
test("hosts and filter live behind toggles while the compact strip keeps phases visible", async ({
  page,
}) => {
  await page.goto("/");
  // Resting state: neither surface mounted, both toggles present.
  await expect(page.locator(".hosts-toggle")).toBeVisible();
  await expect(page.locator(".filter-toggle")).toBeVisible();
  // The toggles SAY they are closed — the state assistive technology
  // reads, and the state openHostsPanel/openFilterBar key off.
  await expect(page.locator(".hosts-toggle")).toHaveAttribute("aria-expanded", "false");
  await expect(page.locator(".filter-toggle")).toHaveAttribute("aria-expanded", "false");
  // The hosts panel stays MOUNTED while collapsed (unmounting would
  // discard in-flight host work — see list.rs), so closed means hidden,
  // not absent; the filter bar owns no tasks and really unmounts.
  await expect(page.locator(".hosts-panel")).toBeHidden();
  await expect(page.locator(".session-filter")).toHaveCount(0);
  // The compact strip carries every host's name and phase chip — the
  // stack's local supervisor is connected, and the phase word is the
  // SAME vocabulary the full panel uses.
  const entry = page.locator(".hosts-compact-entry").first();
  await expect(entry).toBeVisible({ timeout: 20_000 });
  await expect(entry.locator(".host-chip")).toHaveText("connected", { timeout: 20_000 });

  // Each toggle opens its surface; toggling again closes it, back to the
  // resting state rather than accumulating panels.
  await openHostsPanel(page);
  await expect(page.locator(".hosts-toggle")).toHaveAttribute("aria-expanded", "true");
  await page.locator(".hosts-toggle").click();
  await expect(page.locator(".hosts-panel")).toBeHidden();

  await openFilterBar(page);
  await page.locator(".filter-toggle").click();
  await expect(page.locator(".session-filter")).toHaveCount(0);
});

/**
 * An applied filter announces itself beside the toggles while its bar is
 * closed.
 *
 * The note is what keeps the on-demand bar honest: without it, a filter
 * applied, then closed, silently narrows the list and the missing rows
 * read as a shrunken fleet rather than as a query in force.
 */
test("a closed filter bar still announces an applied filter", async ({ page }) => {
  // A query no title matches: the note must reflect the APPLIED query,
  // and the banner proving zero matches is what ties the two together —
  // a note that appeared without the query narrowing anything (or vice
  // versa) fails one of the pair. No fixture session is needed; the
  // shared stack's fleet, whatever it holds, matches nothing here.
  const needle = `no-such-title-${Date.now()}`;
  await page.goto("/");
  await expect(page.locator(".filter-active-note")).toHaveCount(0);
  await openFilterBar(page);
  await page.locator(".filter-title").fill(needle);
  await page.locator(".filter-apply").click();
  await expect(page.locator(".filter-active-note")).toHaveText("filtered", {
    timeout: 20_000,
  });
  await expect(page.locator(".session-count")).toContainText("0 matching", {
    timeout: 20_000,
  });
  // Closing the bar keeps the note: the filter is still in force.
  await page.locator(".filter-toggle").click();
  await expect(page.locator(".session-filter")).toHaveCount(0);
  await expect(page.locator(".filter-active-note")).toBeVisible();
  // Reopening and clearing retires the note with the query.
  await openFilterBar(page);
  await page.locator(".filter-clear").click();
  await expect(page.locator(".filter-active-note")).toHaveCount(0, { timeout: 20_000 });
});

/**
 * A menu whose row leaves the listing does not come back already open
 * when the row returns.
 *
 * `menu_open` is reconciled on every COMMITTED listing (see list.rs) —
 * deliberately unlike the confirmation and rename state, which only
 * fleet-absence clears: a filtered reply is not evidence a session left
 * the fleet, but it absolutely removes the row this transient popup was
 * anchored to. Without that, filtering a row out and clearing the filter
 * restored its floating panel with no new click — a popup nobody
 * re-requested, exposing controls for a session whose state may have
 * changed while the row was gone.
 */
test("a filtered-out row's open menu stays closed when the row returns", async ({
  page,
  request,
}) => {
  const marker = `stale-menu-${Date.now()}`;
  const target = await createSession(request, {
    title: marker,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  let decoy: { id: string } | undefined;
  try {
    decoy = await createSession(request, {
      title: `decoy-${Date.now()}`,
      cwd: "/tmp",
      invocation: "sleep 300",
    });
    await page.goto("/");
    await expect(row(page, target.id)).toBeVisible({ timeout: 20_000 });
    await openRowMenu(row(page, target.id));

    // Filter the open-menu row OUT (the decoy keeps the list non-empty,
    // so an empty-fleet placeholder cannot mask a wrong result)...
    await openFilterBar(page);
    await page.locator(".filter-title").fill(`decoy-`);
    await page.locator(".filter-apply").click();
    await expect(row(page, target.id)).toHaveCount(0, { timeout: 20_000 });
    await expect(row(page, decoy.id)).toBeVisible();

    // ...and bring it back: present again, menu CLOSED.
    await page.locator(".filter-clear").click();
    await expect(row(page, target.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, target.id).locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(row(page, target.id).locator(".session-row-menu")).toHaveAttribute(
      "aria-expanded",
      "false",
    );
  } finally {
    await cleanupSession(request, target.id);
    if (decoy) await cleanupSession(request, decoy.id);
  }
});

/**
 * The compact strip is PER-HOST: several hosts render several entries,
 * each named with its own phase word and color class, and a long
 * unbroken host name ellipsizes instead of widening the strip.
 *
 * The single-host stack only ever shows one connected chip, which a
 * strip that collapsed the fleet to one entry (or hard-coded
 * "connected") would also pass; a stubbed two-host registry in mixed
 * phases is what makes the per-host claim falsifiable, and the 200-char
 * name is what exercises the ellipsis where the strip actually lives.
 */
test("the compact strip names every host with its own phase and clips long names", async ({
  page,
  request,
}) => {
  const longName = `user@${"h".repeat(200)}`;
  // Fabricated replies must carry the helm's own build stamp, or the
  // client latches skew and stands down from reading anything at all.
  const stamp = (await request.get("/api/sessions")).headers()["x-farhelm-build"] ?? "";
  expect(stamp, "the helm must stamp its replies").toBeTruthy();
  await page.route(
    (url) => url.pathname === "/api/hosts",
    async (route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      await route.fulfill({
        headers: { "x-farhelm-build": stamp, "content-type": "application/json" },
        json: {
          hosts: [
            {
              id: 1,
              kind: "local",
              destination: null,
              name: "this machine",
              identity: "id-local",
              remote_farhelm: null,
              remote_state_dir: null,
              state: {
                phase: "connected",
                identity: "id-local",
                build_version: "0.0.3-test",
                refresh: { status: "ok", sessions: 0 },
              },
            },
            {
              id: 2,
              kind: "ssh",
              destination: longName,
              name: longName,
              identity: null,
              remote_farhelm: null,
              remote_state_dir: null,
              state: {
                phase: "unreachable-reprobing",
                cause: "connect-failed",
                last_error: "connection refused",
              },
            },
          ],
        },
      });
    },
  );
  await page.goto("/");

  const entries = page.locator(".hosts-compact-entry");
  await expect(entries).toHaveCount(2, { timeout: 20_000 });
  await expect(entries.nth(0).locator(".hosts-compact-name")).toHaveText("this machine");
  await expect(entries.nth(0).locator(".host-chip")).toHaveText("connected");
  await expect(entries.nth(1).locator(".host-chip")).toHaveText("unreachable-reprobing");
  // The long name is clipped by the strip, not allowed to widen it: the
  // element paints less than it holds, and its box stays inside the
  // sidebar.
  const name = entries.nth(1).locator(".hosts-compact-name");
  expect(await name.evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
  const sidebarBox = (await page.locator(".app-sidebar").boundingBox())!;
  const nameBox = (await name.boundingBox())!;
  expect(nameBox.x + nameBox.width).toBeLessThanOrEqual(sidebarBox.x + sidebarBox.width + 1);
});

/**
 * Closing the hosts panel takes its open profiles section with it: on
 * reopen the section starts closed rather than silently reading a
 * catalog behind a collapsed surface.
 *
 * The panel itself stays MOUNTED while collapsed (in-flight host work
 * must survive a close — see list.rs), so this reset is the one piece of
 * panel state the close deliberately clears, and nothing else pins it.
 */
test("closing the hosts panel closes its profiles section", async ({ page }) => {
  await page.goto("/");
  await openHostsPanel(page);
  const toggleProfiles = page.locator(".host-profiles-toggle").first();
  await toggleProfiles.click();
  await expect(page.locator(".profiles-section")).toBeVisible({ timeout: 20_000 });

  await page.locator(".hosts-toggle").click();
  await expect(page.locator(".hosts-panel")).toBeHidden();
  await openHostsPanel(page);
  await expect(page.locator(".profiles-section")).toHaveCount(0);
});

/**
 * The selection policy itself: a clicked session is remembered across a
 * reload, a stale remembered id falls back to the newest-created
 * non-archived session, and the automatic selection ATTACHES — all
 * without any click after load.
 *
 * This is the PR's primary behavior stated directly; every other test
 * either pins the selection away for staging or clicks rows manually, so
 * none of them fails if remembering, fallback ordering, or the automatic
 * attach silently regress.
 */
test("auto-select remembers the last click, falls back to newest, and attaches", async ({
  page,
  request,
}) => {
  const older = await createSession(request, {
    title: `policy-older-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  let newer: { id: string } | undefined;
  try {
    // created_at has one-second granularity and the merged order
    // tiebreaks by id within a second — a real gap is what makes
    // "newest" deterministic.
    await new Promise((resolve) => setTimeout(resolve, 1_100));
    newer = await createSession(request, {
      title: `policy-newer-${Date.now()}`,
      cwd: "/tmp",
      invocation: "sleep 300",
    });

    // Click the OLDER session — the one the fallback would never pick —
    // then reload: the remembered choice must win over newest-created.
    await page.goto("/");
    await expect(row(page, older.id)).toBeVisible({ timeout: 20_000 });
    await row(page, older.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toContainText("policy-older-");
    await page.reload();
    await expect(page.locator(".titlebar .title")).toContainText("policy-older-", {
      timeout: 20_000,
    });
    // The auto-selection is a real attachment, not just a mounted view.
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    // A stale remembered id — a session that no longer exists — falls
    // back to the newest-created non-archived session.
    await page.evaluate(() => {
      const raw = window.localStorage.getItem("farhelm.last-selected")!;
      const record = JSON.parse(raw);
      record.id = "00000000-0000-0000-0000-000000000000";
      window.localStorage.setItem("farhelm.last-selected", JSON.stringify(record));
    });
    await page.reload();
    await expect(page.locator(".titlebar .title")).toContainText("policy-newer-", {
      timeout: 20_000,
    });
  } finally {
    await cleanupSession(request, older.id);
    if (newer) await cleanupSession(request, newer.id);
  }
});

/**
 * Launching a second client displaces the first WITHOUT any click: the
 * takeover the amended SPEC names ("opening a client counts as opening a
 * session") — a manual row click in the second client would mask an
 * auto-attach that never happened.
 */
test("a second client's launch alone takes the terminal over", async ({
  browser,
  page,
  request,
}) => {
  const session = await createSession(request, {
    title: `launch-takeover-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  let second: import("@playwright/test").BrowserContext | undefined;
  try {
    await page.goto("/");
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });
    await row(page, session.id).locator(".session-row-open").click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    // The second client remembers the same session and merely LOADS.
    second = await browser.newContext({
      storageState: await page.context().storageState(),
    });
    const page2 = await second.newPage();
    await pinAutoSelect(page2, session.id);
    await page2.goto("/");
    await page2.waitForFunction(() => (window as any).__farhelmTermReady === true, undefined, {
      timeout: 20_000,
    });

    // Client one shows its displaced banner; client two owns the live
    // terminal — with no click anywhere in client two.
    await expect(page.locator("#term-banner")).toContainText(/detached|took over|displaced/i, {
      timeout: 20_000,
    });
  } finally {
    await second?.close();
    await cleanupSession(request, session.id);
  }
});

/**
 * The retired surfaces stay retired: an open session view renders neither
 * a back button nor a titlebar rename, while the row menu's rename — the
 * one surviving surface — remains present.
 *
 * Reintroducing Back restores an unselected pane the shell no longer
 * models; reintroducing the titlebar rename restores the dual optimistic
 * title overlays the redesign removed.
 */
test("the back button and titlebar rename do not come back", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `retired-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });
    await row(page, session.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toContainText("retired-");

    await expect(page.locator(".back-button")).toHaveCount(0);
    await expect(page.locator(".session-rename")).toHaveCount(0);
    await expect(page.locator(".titlebar .rename-form")).toHaveCount(0);

    await openRowMenu(row(page, session.id));
    await expect(row(page, session.id).locator(".session-row-rename")).toBeVisible();
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * The placeholder's three states are honest: "no active sessions" appears
 * only once a committed listing PROVED an empty fleet, and the moment a
 * session exists the pane switches to the auto-selected view.
 *
 * Route-controlled because the shared stack always has sessions: this is
 * the only way to watch the empty→non-empty transition, and the only test
 * that fails if the placeholder claims emptiness during loading or keeps
 * claiming it beside available sessions.
 */
test("the empty-fleet placeholder appears only when proven and yields to auto-select", async ({
  page,
  request,
}) => {
  const stamp = (await request.get("/api/sessions")).headers()["x-farhelm-build"] ?? "";
  expect(stamp, "the helm must stamp its replies").toBeTruthy();
  let fleet: unknown[] = [];
  await page.route(
    (url) => url.pathname === "/api/sessions",
    async (route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      await route.fulfill({
        headers: { "x-farhelm-build": stamp, "content-type": "application/json" },
        json: {
          sessions: fleet,
          total: fleet.length,
          matching: fleet.length,
          truncated: false,
        },
      });
    },
  );
  const feed = await stubFeed(page);
  await page.goto("/");
  await feed.waitForConnection(1);
  feed.notify(1);

  await expect(page.locator(".main-empty")).toHaveText("no active sessions — create one", {
    timeout: 20_000,
  });

  fleet = [
    {
      id: "placeholder-cycle-session",
      title: "placeholder-cycle",
      cwd: "/tmp",
      invocation: "sleep 300",
    },
  ];
  feed.notify(2);
  // The pane replaces the placeholder with the auto-selected session's
  // view (the attach itself will fail against this fabricated session,
  // which is fine — the placeholder yielding is the contract here).
  await expect(page.locator(".titlebar .title")).toHaveText("placeholder-cycle", {
    timeout: 20_000,
  });
});

/**
 * PR #162's own regression, proven directly: opening the menu on a row near
 * the bottom of a SCROLLED list keeps the panel — including its last action
 * button — entirely inside the browser VIEWPORT, not merely inside the
 * sidebar's own clipped scroll window.
 *
 * The pre-fix scheme anchored the panel with `position: absolute` inside
 * its row, so `.session-list`'s and `.app-sidebar`'s `overflow` clipped it
 * at the scroll container's edge the moment the row sat near the bottom of
 * the visible window — a row need not be the list's LAST row to trigger
 * this, only near the bottom of what is currently scrolled into view. The
 * sidebar spans the shell's full height (app.css's height:100% chain), so
 * that scroll-container edge and the viewport's own bottom edge are nearly
 * the same line — which is what makes a plain "bottom edge inside the
 * viewport" assertion a faithful stand-in for "not clipped by the old
 * scheme": under the old positioning this would have failed for the same
 * reason the panel was invisible, just observed as geometry rather than as
 * a screenshot. The fix (`menu_panel_style` in list.rs) escapes that clip
 * by positioning `fixed` against the viewport instead, clamped on every
 * edge.
 */
test("opening the last visible row's menu in a scrolled list stays inside the viewport", async ({
  page,
  request,
}) => {
  const marker = `clip-${Date.now()}`;
  const created = await fillSidebarPastOneScreen(request, marker);
  try {
    // A modest viewport, set before navigation: fewer fixture sessions are
    // needed to force a scroll, and the height is exactly what the
    // "bottom edge inside the viewport" assertion below is measured
    // against.
    await page.setViewportSize({ width: 900, height: 500 });
    await page.goto("/");
    await expect(row(page, created[0].id)).toBeVisible({ timeout: 20_000 });
    // Settled BEFORE any scroll or menu open: the strip's own async host
    // read landing is itself one of the six internal layout causes
    // list.rs's close-on-shift effect watches, and this test's whole
    // premise (a row genuinely scrolled into view, then a menu that stays
    // put) must not race it.
    await waitForHostsStripSettled(page);

    const sidebar = page.locator(".app-sidebar");
    await expect
      .poll(() => sidebar.evaluate((el) => el.scrollHeight > el.clientHeight + 1), {
        timeout: 20_000,
        message: "the sidebar must actually need to scroll for this regression to be meaningful",
      })
      .toBe(true);

    // Actually scroll: the reported bug was about a row scrolled INTO
    // view near the bottom of a long list, not merely one that happens to
    // sit near the fold at rest. Scrolling all the way down also makes
    // "the last visible row" and "the list's real last row" the same row
    // — the truest reproduction of the original geometry.
    await sidebar.evaluate((el) => {
      el.scrollTop = el.scrollHeight;
    });
    const scrollTop = await sidebar.evaluate((el) => el.scrollTop);
    expect(
      scrollTop,
      "the sidebar must have actually scrolled for this regression to be meaningful",
    ).toBeGreaterThan(0);
    // Let the scroll's own dismissal effect (proven by its own test below)
    // fully settle before opening any menu: opening one while THIS
    // scroll's `layout_epoch` bump is still being processed would race the
    // very close-on-scroll behavior this test does not mean to exercise
    // here (no menu is open yet to close, but a late-landing effect run
    // could still catch a menu opened moments later). Two animation
    // frames is a generous, standard barrier for "whatever this scroll
    // was going to trigger has already run".
    await page.evaluate(
      () =>
        new Promise<void>((resolve) =>
          requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
        ),
    );

    // The last row FULLY visible in the now-scrolled window that is one of
    // THIS TEST'S OWN fixture rows — not one whose top merely peeks above
    // the fold, and never the shared `e2e-session` row. Scrolling all the
    // way down (above) means the bottom-most fully-visible row is the
    // fleet's own last row in whatever order the helm lists it, and on
    // this shared stack that can genuinely be `e2e-session` itself — the
    // stack's oldest session, and the one every later test in this suite
    // depends on still existing. Walking upward from the bottom until
    // landing on one of `created`'s own ids is what keeps the delete
    // confirmation below aimed at a fixture this test made and cleans up
    // itself; the geometry under test (a panel measured near the
    // sidebar's own clipped bottom edge) holds identically for whichever
    // row that walk lands on, since every row in a scrolled-to-bottom list
    // is equally "near the bottom".
    const ownIds = created.map((session) => session.id);
    const targetId = await page.evaluate((ownIds) => {
      const sidebarBottom = document.querySelector(".app-sidebar")!.getBoundingClientRect().bottom;
      const rows = [...document.querySelectorAll(".session-row")];
      const fullyVisible = rows.filter(
        (candidate) => candidate.getBoundingClientRect().bottom <= sidebarBottom,
      );
      for (let i = fullyVisible.length - 1; i >= 0; i--) {
        const id = fullyVisible[i].getAttribute("data-session-id");
        if (id && ownIds.includes(id)) return id;
      }
      return null;
    }, ownIds);
    // Asserted loudly and separately from the evaluate's own `null` case:
    // a regression that let the walk drift back onto a foreign row (a
    // logic slip in the filter above, say) must name exactly which id it
    // picked, not merely fail a later, unrelated-looking assertion three
    // steps downstream with no hint this was the actual cause.
    expect(
      targetId,
      "expected at least one of this test's own fixture rows fully visible after scrolling",
    ).not.toBeNull();
    expect(
      ownIds,
      `the chosen row ${JSON.stringify(targetId)} must be one of this test's own fixture sessions, never the shared stack's`,
    ).toContain(targetId);

    const target = row(page, targetId!);
    await openRowMenu(target);

    const panel = target.locator(".session-row-menu-panel");
    // `openRowMenu` already refuses to return on a `Fallback` placement
    // (see its own doc in fleet.ts), but this is the one test whose whole
    // point IS the measured geometry, so it also asserts the discriminant
    // directly: a silent regression back to the top-left fallback corner
    // must fail HERE, loudly and specifically, rather than merely time out
    // somewhere upstream with a less legible message.
    const panelStyle = (await panel.getAttribute("style")) ?? "";
    expect(
      panelStyle,
      "the panel must have measured against its real toggle, not fallen back to the top-left corner placement",
    ).toContain("left: auto");

    const viewport = page.viewportSize()!;
    const panelBox = (await panel.boundingBox())!;
    // All four edges: the old absolute scheme's failure mode was clipping
    // at the BOTTOM/RIGHT, but a regression in the opposite direction —
    // the panel pushed off the TOP or LEFT edge by a broken clamp — is
    // just as real a way to fail "entirely inside the viewport".
    expect(panelBox.x).toBeGreaterThanOrEqual(0);
    expect(panelBox.y).toBeGreaterThanOrEqual(0);
    expect(panelBox.y + panelBox.height).toBeLessThanOrEqual(viewport.height);
    expect(panelBox.x + panelBox.width).toBeLessThanOrEqual(viewport.width);

    // The LAST action button specifically (rename → stop → archive →
    // delete, per SessionRow's own doc): if the panel's bottom clipped at
    // all, this is the control most likely to be cut off or unreachable.
    const deleteButton = target.locator(".session-row-menu-panel .session-row-delete");
    const deleteBox = (await deleteButton.boundingBox())!;
    expect(deleteBox.y + deleteBox.height).toBeLessThanOrEqual(viewport.height);
    // Re-asserted open immediately before this test's own longest stretch
    // of real elapsed time (the 18-session fixture plus every wait above
    // it already make this the slowest test in the file) closes with an
    // actual click: this is the one place in this file that both opens a
    // menu AND still needs it open several real seconds later. `is_loading`
    // (what `waitForHostsStripSettled` above watches) only ever flips ONCE,
    // so it cannot catch a LATER hosts re-read landing with a different
    // outcome — a real SSH host's connection flapping on a busy CI runner,
    // say — flipping `hosts_strip_shape`'s error component and closing the
    // menu out from under this test through no fault of the clipping fix
    // itself. `openRowMenu` is idempotent on an already-open menu (its own
    // doc, verified: it checks `aria-expanded` before ever clicking), so
    // calling it again here is a no-op in the ordinary case and a genuine
    // reopen in the raced one.
    await openRowMenu(target);
    // Genuinely clickable, not merely painted within bounds: Playwright's
    // click waits for the target to be visible, stable, and unobscured by
    // anything else before it fires.
    await deleteButton.click();

    // ONE bounded retry around the confirm prompt actually appearing.
    // This is honest tolerance for a real, DESIGNED behavior, not
    // flake-papering over an unproven race: list.rs closes the whole menu
    // — panel and all — on any of six background layout causes it
    // deliberately does not distinguish from a user-driven one (a host's
    // phase flapping among them, via the compact strip's shape — see
    // `waitForHostsStripSettled`'s own doc), and this test's own long
    // fixture setup gives a real host on a shared, long-lived stack far
    // more wall-clock time to do exactly that than an isolated repeat of
    // this one test ever sees. None of those causes are a bug in the
    // clipping fix this test exists to pin — its subject is measured-panel
    // geometry and last-action clickability, and every dismissal behavior
    // already has its own dedicated test elsewhere in this file. A retry
    // that just re-clicked delete unconditionally would itself be wrong,
    // though: `confirming` (list.rs) is set the instant the FIRST click
    // lands and — per `SessionRow`'s own doc on why its cancel button's
    // `autofocus` can fire more than once — SURVIVES the panel closing, so
    // reopening after a raced dismissal typically lands directly back on
    // the confirm view with no delete button left to click at all. Only
    // re-click it if reopening genuinely did not.
    const confirmConsequence = target.locator(".session-row-menu-panel .confirm-consequence");
    const confirmedOnFirstTry = await confirmConsequence
      .waitFor({ state: "visible", timeout: 3_000 })
      .then(() => true)
      .catch(() => false);
    if (!confirmedOnFirstTry) {
      await openRowMenu(target);
      if (!(await confirmConsequence.isVisible())) {
        await target.locator(".session-row-menu-panel .session-row-delete").click();
      }
    }
    await expect(confirmConsequence).toBeVisible();
    await target.locator(".confirm-cancel").click();
  } finally {
    await cleanupAll(request, created);
  }
});

/**
 * Scrolling the sidebar — the panel's real invalidation trigger, not merely
 * "something happened somewhere" — closes an open row menu.
 *
 * The panel is positioned `fixed` against the viewport from a ONE-TIME
 * measurement of the toggle's rect (list.rs's `PanelPlacement`); once the
 * row scrolls, that snapshot is stale and the panel would float over
 * whatever content the scroll left behind. `AppBody`'s `onscroll` on
 * `.app-sidebar` (lib.rs) is what notices and closes it — this is the
 * direct proof that listener actually fires and actually reaches the menu,
 * as opposed to the layout-dismissal tests below, which go through the
 * sidebar's OWN toggles instead.
 */
test("scrolling the sidebar closes an open row menu", async ({ page, request }) => {
  const marker = `scroll-close-${Date.now()}`;
  const created = await fillSidebarPastOneScreen(request, marker);
  try {
    await page.setViewportSize({ width: 900, height: 500 });
    await page.goto("/");
    await expect(row(page, created[0].id)).toBeVisible({ timeout: 20_000 });
    // Settled before the menu opens — see `waitForHostsStripSettled`'s own
    // doc for why an unsettled strip can close the menu for the wrong
    // reason.
    await waitForHostsStripSettled(page);

    const sidebar = page.locator(".app-sidebar");
    await expect
      .poll(() => sidebar.evaluate((el) => el.scrollHeight > el.clientHeight + 1), {
        timeout: 20_000,
        message: "the sidebar must actually need to scroll for this regression to be meaningful",
      })
      .toBe(true);

    // The topmost row, deliberately — NOT a specific fixture session: a
    // row further down would already be scrolled into view by Playwright's
    // own click-into-view behavior the moment its toggle is clicked, which
    // would make the scroll below a no-op and the test vacuous. The first
    // row is on screen at the sidebar's resting scrollTop of 0, so opening
    // ITS menu cannot itself move the scroll position.
    const target = page.locator(".session-row").first();
    await openRowMenu(target);
    const before = await sidebar.evaluate((el) => el.scrollTop);
    expect(
      before,
      "opening the first row's menu must not itself have scrolled the sidebar, or the assertion below would be vacuous",
    ).toBe(0);

    await sidebar.evaluate((el) => {
      el.scrollTop = el.scrollHeight;
    });
    // The scroll must have actually MOVED the container — asserting the
    // menu closed without this would say nothing if the assignment above
    // were a no-op (the list already fully visible, a stray CSS change
    // pinning scrollTop, and so on).
    await expect
      .poll(() => sidebar.evaluate((el) => el.scrollTop), {
        message: "the sidebar's scrollTop must actually change for this test to prove anything",
      })
      .not.toBe(before);

    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(target.locator(".session-row-menu")).toHaveAttribute("aria-expanded", "false");
  } finally {
    await cleanupAll(request, created);
  }
});

/**
 * The sidebar's own on-demand toggles are layout causes too: opening the
 * hosts panel, the filter bar, or the create dialog each mounts a whole
 * section above the rows (list.rs's `use_effect` near `show_create`), which
 * is exactly the kind of internal shift `layout_epoch` does NOT cover (that
 * counter is for the ancestor-owned scroll/resize listeners in lib.rs) —
 * this component watches these three signals directly instead. One test
 * per surface, because each is a separate signal the effect subscribes to
 * and a regression could plausibly drop any one of the three independently.
 */
test("opening the hosts panel closes an open row menu", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `menu-hosts-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsStripSettled(page);
    await openRowMenu(target);

    await page.locator(".hosts-toggle").click();
    // The surface actually appeared — without this, a toggle that silently
    // failed to open anything would still make the assertions below pass
    // (the menu was never touched either way), proving nothing about the
    // dismissal this test claims.
    await expect(page.locator(".hosts-panel")).toBeVisible();

    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(target.locator(".session-row-menu")).toHaveAttribute("aria-expanded", "false");
  } finally {
    await cleanupSession(request, session.id);
  }
});

/** See "opening the hosts panel closes an open row menu" — same contract,
 * the filter bar's own toggle. */
test("opening the filter bar closes an open row menu", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `menu-filter-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsStripSettled(page);
    await openRowMenu(target);

    await page.locator(".filter-toggle").click();
    await expect(page.locator(".session-filter")).toBeVisible();

    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(target.locator(".session-row-menu")).toHaveAttribute("aria-expanded", "false");
  } finally {
    await cleanupSession(request, session.id);
  }
});

/** See "opening the hosts panel closes an open row menu" — same contract,
 * the "new session" create dialog's own toggle. */
test("opening the create-session form closes an open row menu", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `menu-create-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsStripSettled(page);
    await openRowMenu(target);

    await page.locator(".new-session-button").click();
    await expect(page.locator(".create-session-form")).toBeVisible();

    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(target.locator(".session-row-menu")).toHaveAttribute("aria-expanded", "false");
  } finally {
    // Backend cleanup lives on its own, unconditional of any UI action:
    // an earlier version of this test also clicked the toggle closed here
    // for tidiness, which meant a failure in THAT click — nothing to do
    // with the session itself — could skip `cleanupSession` entirely and
    // strand a real session on the shared stack. Not worth the risk for
    // pure hygiene, so this finally does the one thing it must.
    await cleanupSession(request, session.id);
  }
});

/**
 * The complement to the three layout-dismissal tests above: a listing
 * refresh that changes nothing LAYOUT-relevant — no scroll, no toggle, no
 * row reorder, no row vanishing — leaves an open menu open.
 *
 * Without this, the three tests above could pass for the wrong reason: an
 * implementation that closed the menu on every reply from the feed-driven
 * re-read (rather than specifically on the layout causes list.rs's
 * `use_effect` watches) would also close it here, and nothing else in this
 * suite would catch that regression — every other menu test either never
 * triggers a refresh mid-open or triggers one that DOES change the
 * listing.
 *
 * The listing itself is a CONTROLLED, frozen fabrication — a route stub
 * answering every `GET /api/sessions` with the exact same body, byte for
 * byte — rather than the real backend's own live reply. A real
 * `sleep 300` fixture session's status is free to flip between polls (say,
 * from "unknown" to "idle" once the supervisor's sampler gets a look at
 * it), and that would be a real, if harmless, content change riding along
 * with the notification this test triggers on purpose — exactly the kind
 * of confound "nothing layout-relevant changed" cannot afford. A frozen
 * reply removes that possibility structurally instead of hoping the timing
 * works out.
 *
 * The completion barrier is two-part, because dispatching a request is not
 * receiving — let alone RENDERING — a reply: `page.waitForResponse` proves
 * the round trip actually completed over the network (a plain read-COUNT
 * check only proves a request was SENT), and the animation-frame pair past
 * it is this suite's standard stand-in for "whatever that reply was going
 * to change has already been patched into the DOM" (list.rs's own render
 * commit runs inside the browser's normal frame-batched update cycle, with
 * no other externally observable hook to await instead).
 */
test("a listing refresh with nothing layout-relevant to report leaves an open row menu open", async ({
  page,
  request,
}) => {
  const stamp = (await request.get("/api/sessions")).headers()["x-farhelm-build"] ?? "";
  expect(stamp, "the helm must stamp its replies").toBeTruthy();
  const sessionId = "menu-refresh-fixture-session";
  // Served identically on every GET this route sees — see this test's own
  // doc for why a frozen fabrication, not the real backend's reply, is
  // what "nothing changed" actually needs.
  const listingBody = {
    sessions: [
      {
        id: sessionId,
        title: `menu-refresh-${Date.now()}`,
        cwd: "/tmp",
        invocation: "sleep 300",
        status: { state: "idle" },
      },
    ],
    total: 1,
    matching: 1,
    truncated: false,
  };
  let listingReads = 0;
  await page.route(
    (url) => url.pathname === "/api/sessions",
    async (route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      listingReads += 1;
      await route.fulfill({
        headers: { "x-farhelm-build": stamp, "content-type": "application/json" },
        json: listingBody,
      });
    },
  );
  const feed = await stubFeed(page);
  await page.goto("/");
  await feed.waitForConnection(1);
  feed.notify(1); // The handshake — the helm's own greeting on subscribe.

  const target = row(page, sessionId);
  await expect(target).toBeVisible({ timeout: 20_000 });
  await waitForHostsStripSettled(page);
  await openRowMenu(target);

  const before = listingReads;
  const responded = page.waitForResponse(
    (r) => new URL(r.url()).pathname === "/api/sessions" && r.request().method() === "GET",
  );
  // A LATER bump: the helm's "something changed" signal, with the stub
  // above guaranteeing the reply it provokes is identical to what the
  // page already has.
  feed.notify(2);
  await responded;
  expect(
    listingReads,
    "the notification must actually provoke a re-read for this test to prove anything",
  ).toBeGreaterThan(before);
  // Two animation frames past the response: see this test's own doc for
  // why network arrival alone is not proof the reply was RENDERED.
  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
      ),
  );

  // The row survives (same position, same content) and so does its menu.
  await expect(target.locator(".session-row-menu-panel")).toBeVisible();
  await expect(target.locator(".session-row-menu")).toHaveAttribute("aria-expanded", "true");
});

/**
 * Resizing the whole browser window closes an open row menu — the other
 * half of `layout_epoch`'s external causes (lib.rs's `onresize` on
 * `.app-sidebar`), independent of the scroll test above: a window resize
 * narrow enough to trigger `.app-shell`'s horizontal scroll can move every
 * row without ever firing `onscroll` on its own (lib.rs's own doc), so
 * `onresize` is not a redundant listener and needs its own proof.
 */
test("resizing the viewport closes an open row menu", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `menu-resize-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsStripSettled(page);
    await openRowMenu(target);

    const before = page.viewportSize()!;
    await page.setViewportSize({ width: before.width - 200, height: before.height - 100 });

    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(target.locator(".session-row-menu")).toHaveAttribute("aria-expanded", "false");
  } finally {
    await cleanupSession(request, session.id);
  }
});
