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
 * The menu's own group is the largest of those and is worth naming, since
 * almost none of it can be checked anywhere else: the anchor's covered-
 * toggle safety property, the ARIA menu-button relationship read out of
 * the accessibility tree rather than out of class names, arrow/Home/End
 * navigation over the real nodes (including an archived row's shorter
 * list), the roving `tabindex` and Tab's exit, the keys the menu leaves
 * native, the prompt states' refusal to answer any of them, focus
 * returning to the toggle on an automatic dismissal, a busy menu staying
 * navigable, and the raised surface's computed style. The pure decisions
 * behind all of it live in menu_panel.rs (shared with the host row's own
 * menu — hosts.rs) and are unit-tested there;
 * nothing in the Rust suite can dispatch a key, move focus, or resolve a
 * computed style, which is what this file is for.
 *
 * Like every per-area file since M6.5, new coverage starts its own spec
 * rather than growing the terminal spec family. An area file keeps one
 * subject's tests findable and runnable together.
 */
import { expect, test, type APIRequestContext, type Locator, type Page } from "@playwright/test";
import {
  cleanupProfile,
  cleanupSession,
  createProfile,
  createSession,
  localHostId,
  openFilterBar,
  openHostMenu,
  openHostsPanel,
  openRowMenu,
  patchPreferences,
  pinAutoSelect,
  readPreferences,
  SESSION_LISTING,
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
    // The sidebar highlight (SPEC.md: the selected session's row is
    // visibly marked) tracks the selection through the whole switch.
    // Three layers, each catching a different regression: the data
    // attribute (the selection bookkeeping), the production class (the
    // styling hook), and the computed backgrounds (the VISIBLE result —
    // a deleted or misspelled CSS rule leaves the first two green). The
    // visual check runs in the post-click state on purpose: the clicked
    // button is still focused, which is exactly when the button's own
    // focus tint once painted over the row's highlight.
    await expect(row(page, a.id)).toHaveAttribute("data-session-selected", "true");
    await expect(row(page, b.id)).toHaveAttribute("data-session-selected", "false");
    await expect(row(page, a.id)).toHaveClass(/(^| )selected( |$)/);
    await expect(row(page, b.id)).not.toHaveClass(/(^| )selected( |$)/);
    const background = (id: string) =>
      row(page, id).evaluate((el) => getComputedStyle(el).backgroundColor);
    expect(
      await background(a.id),
      "the selected row must be visibly distinct, not just attributed",
    ).not.toBe(await background(b.id));
    // The accessible counterpart: aria-current marks exactly the selected
    // row's open button, and is ABSENT (not "false") elsewhere.
    await expect(row(page, a.id).locator(".session-row-open")).toHaveAttribute(
      "aria-current",
      "true",
    );
    await expect(row(page, b.id).locator(".session-row-open")).not.toHaveAttribute(
      "aria-current",
      /.*/,
    );
    const viewA = await page.locator(".app-main .layout").elementHandle();
    expect(viewA).not.toBeNull();

    // No Back in between — this is the direct switch the key exists for.
    await row(page, b.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toContainText(b.title);
    // The highlight moved with the selection: exactly one row marked, in
    // every layer.
    await expect(row(page, b.id)).toHaveAttribute("data-session-selected", "true");
    await expect(row(page, a.id)).toHaveAttribute("data-session-selected", "false");
    await expect(row(page, b.id)).toHaveClass(/(^| )selected( |$)/);
    await expect(row(page, a.id)).not.toHaveClass(/(^| )selected( |$)/);
    await expect(row(page, b.id).locator(".session-row-open")).toHaveAttribute(
      "aria-current",
      "true",
    );
    await expect(row(page, a.id).locator(".session-row-open")).not.toHaveAttribute(
      "aria-current",
      /.*/,
    );
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
 * cwd, and invocation — stays INSIDE the fixed sidebar column, with the
 * title above a meta line that pairs the directory with the invocation
 * badge.
 *
 * The stacked layout's whole reason to exist is that the old single-line
 * row overflowed the 340px column under exactly this content (the MT-8
 * class); a wrapper or flex regression that restored horizontal packing
 * or let a line force the row wide would pass every text-content
 * assertion and fail only here. The geometry asserted below is the
 * density pass's layout (the 2026-08 UI refresh): directory and invocation
 * deliberately SHARE a line now, so the check is that they overlap
 * vertically and sit side by side, not that one is under the other.
 *
 * Two further properties ride along because this is the cheapest local
 * session in the suite to assert them on: a session on the helm's own
 * machine renders no host line at all, and the whole row fits in a height
 * that the four-line layout could not have.
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
    // The title still leads on a line of its own — a regression that
    // packed it in beside the metadata would land here.
    expect(title.y).toBeLessThan(cwd.y);
    // Directory and invocation share the meta line: overlapping vertical
    // extents, badge to the right of the path. Asserted as an overlap
    // rather than as equal `y` because the two have different font sizes
    // and sit on a shared baseline, so their boxes start at different
    // offsets by design.
    expect(invocation.y).toBeLessThan(cwd.y + cwd.height);
    expect(cwd.y).toBeLessThan(invocation.y + invocation.height);
    expect(
      invocation.x,
      "the invocation badge follows the directory on the same line",
    ).toBeGreaterThanOrEqual(cwd.x + cwd.width - 1);

    // This session is on the helm's own machine, so naming its host would
    // be a line spent on a word every row would carry.
    await expect(target.locator(".session-host")).toHaveCount(0);

    // A deliberately loose ceiling: the point is the density decision (a
    // row roughly half the four-line layout's ~90px), not a pixel-exact
    // layout, and Chromium and WebKit disagree about font metrics by a
    // few pixels either way.
    expect(
      rowBox.height,
      "the dense row must not drift back toward the four-line layout's height",
    ).toBeLessThan(64);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * A long PROFILE-backed invocation badge clips inside the row, and the cwd
 * beside it keeps a usable share of the shared meta line.
 *
 * The 300-char raw invocation above no longer exercises the badge's own
 * overflow handling: the 2026-08 UI refresh compacts any RAW command line to
 * one short word (the program's basename, plus at most a short marker —
 * `list::row::compact_invocation`), so a long raw invocation can no longer
 * widen the badge. A profile-backed session answers differently — the
 * badge shows the profile's own snapshotted NAME
 * (`source_profile_label`/`display_peer` in row.rs), which carries no such
 * compaction and can legitimately run long — so this is the fixture that
 * still proves the badge clips rather than pushing the row wide.
 */
test("a long profile-backed invocation badge clips inside the row", async ({ page, request }) => {
  const local = await localHostId(request);
  const name = `contained-profile-${"p".repeat(280)}`;
  const profile = await createProfile(request, local, { name });
  const session = await createSession(request, {
    title: `contained-profile-session-${Date.now()}`,
    cwd: "/tmp",
    profile_id: profile.id,
    host: local,
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });

    const sideBox = (await page.locator(".app-sidebar").boundingBox())!;
    const badge = target.locator(".session-invocation");
    const cwd = target.locator(".session-cwd");
    const badgeBox = (await badge.boundingBox())!;
    const cwdBox = (await cwd.boundingBox())!;

    // The badge never forces the row wide...
    expect(badgeBox.x + badgeBox.width).toBeLessThanOrEqual(sideBox.x + sideBox.width + 1);
    // ...and it really is being clipped, not merely short enough to fit —
    // proving the ellipsis rule did the constraining rather than a
    // conveniently narrow fixture.
    expect(await badge.evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
    // The directory keeps a USABLE share of the line: `.session-row-meta`'s
    // split caps the badge at 55% (app.css) precisely so an unbounded
    // profile name cannot squeeze the path down to nothing.
    expect(
      cwdBox.width,
      "the directory must keep a usable share of the shared line, not be squeezed to nothing " +
        "by an unbounded badge",
    ).toBeGreaterThan(20);
  } finally {
    await cleanupSession(request, session.id);
    await cleanupProfile(request, local, profile.id);
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
    // placeholder — same discipline as terminal-restart.spec.ts's tests),
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
      ".session-row-clone",
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
      ".session-row-clone",
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
 * Resolve a design token to the exact string `getComputedStyle` will
 * report for it, by asking the page to compute it on a throwaway element.
 *
 * Comparing a rule's computed color against the raw `:root` declaration
 * does not work — the engine normalizes `#1c2530` to `rgb(28, 37, 48)` —
 * and hard-coding the normalized form in a test would pin the palette's
 * VALUES here, exactly the duplication the token layer exists to end. A
 * probe element resolves the token through the same machinery the rule
 * under test goes through, so the two strings are comparable and the test
 * keeps saying "this rule spends THIS token" rather than "this rule is
 * this shade of blue".
 */
async function tokenColor(page: Page, token: string): Promise<string> {
  return page.evaluate((name) => {
    const probe = document.createElement("div");
    probe.style.color = `var(${name})`;
    document.body.append(probe);
    const resolved = getComputedStyle(probe).color;
    probe.remove();
    return resolved;
  }, token);
}

/**
 * The actions menu's ARIA contract, read out of the accessibility tree
 * rather than out of its class names.
 *
 * Every other test in this file finds the menu's parts by CSS class,
 * which means every one of them would stay green if `aria-haspopup`, the
 * menu role, its accessible name, or every `menuitem` role vanished — the
 * markup a screen-reader user has instead of the panel's appearance. This
 * is the one test that fails for those, and it checks the sub-states too:
 * a confirm prompt is not a menu, it stops advertising itself as one, and
 * the toggle's own `aria-haspopup` follows it rather than promising a
 * list of commands the panel is not about to show.
 */
test("the actions menu exposes a real menu-button relationship", async ({ page, request }) => {
  const title = `menu-aria-${Date.now()}`;
  const session = await createSession(request, { title, cwd: "/tmp", invocation: "sleep 300" });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsStripSettled(page);

    const toggle = target.getByRole("button", { name: `session actions for ${title}` });
    await expect(toggle).toHaveAttribute("aria-haspopup", "menu");
    await expect(toggle).toHaveAttribute("aria-expanded", "false");

    await openRowMenu(target);
    await expect(toggle).toHaveAttribute("aria-expanded", "true");

    // Located by ROLE and NAME, never by class: the name is what tells a
    // screen-reader user which of the several identical menus in this
    // list they have opened.
    const menu = target.getByRole("menu", { name: `session actions for ${title}` });
    await expect(menu).toBeVisible();
    await expect(menu.getByRole("menuitem")).toHaveText([
      "rename",
      "clone",
      "stop",
      "archive",
      "delete",
    ]);
    // The boundary before the destructive item exists in the tree, not
    // only in the paint — five consecutive commands with nothing marking
    // the last as different in kind is what this replaces.
    await expect(menu.getByRole("separator")).toHaveCount(1);
    // The profile footer and any refusal line are the panel's, not the
    // menu's: a `role="menu"` whose children are not all commands is a
    // grouping a screen reader has to guess at.
    await expect(menu.locator(".session-profile")).toHaveCount(0);

    await target.locator(".session-row-delete").click();
    // The prompt is a named exchange, not a menu with no items in it.
    await expect(target.getByRole("menu")).toHaveCount(0);
    await expect(
      target.getByRole("dialog", { name: `delete confirmation for ${title}` }),
    ).toBeVisible();
    await expect(toggle).toHaveAttribute("aria-haspopup", "dialog");
    await expect(toggle).toHaveAttribute("aria-expanded", "true");

    await target.locator(".confirm-cancel").click();
    await expect(target.getByRole("menu", { name: `session actions for ${title}` })).toBeVisible();
    await expect(toggle).toHaveAttribute("aria-haspopup", "menu");
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * Arrow navigation reaches EVERY item, wraps at both ends, and Home/End
 * jump — through the real nodes, not through index arithmetic.
 *
 * `next_menu_focus` in menu_panel.rs already pins the arithmetic,
 * and it cannot prove any of what this proves: that all five items
 * mounted, that each registered a handle under its own action, and that
 * the positions the key handler derives from `MenuOrder` line up with the
 * order the panel actually renders. A previous version of this test
 * walked two of the four items THEN offered, which left archive and delete
 * — the two with separately duplicated wiring, and the two whose misfire
 * is destructive — covered by nothing at all; clone joined the walk when
 * it joined the menu, for the same reason.
 *
 * The separator sitting between archive and delete is part of what is
 * being checked: it is not focusable and not counted, so ArrowDown must
 * step straight over it.
 */
test("the actions menu walks every item and wraps at both ends", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `menu-keys-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    // A host read landing mid-test changes the compact strip's shape,
    // which list.rs treats as a layout cause and closes any open menu for
    // — see the clipping test below for the same precaution.
    await waitForHostsStripSettled(page);

    await openRowMenu(target);
    const toggle = target.locator(".session-row-menu");
    const rename = target.locator(".session-row-rename");
    const clone = target.locator(".session-row-clone");
    const stop = target.locator(".session-row-stop");
    const archive = target.locator(".session-row-archive");
    const remove = target.locator(".session-row-delete");

    // Focus is put on the toggle EXPLICITLY rather than left where the
    // opening click put it: this test is about stepping through the list
    // from OUTSIDE it, and the opening focus is the next test's subject.
    await toggle.focus();
    await expect(toggle).toBeFocused();

    await page.keyboard.press("ArrowDown");
    await expect(rename).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(clone).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(stop).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(archive).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(remove, "the separator is not a stop on the way to delete").toBeFocused();
    // Both wrap boundaries, in the two directions that reach them.
    await page.keyboard.press("ArrowDown");
    await expect(rename).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(remove).toBeFocused();

    await page.keyboard.press("Home");
    await expect(rename).toBeFocused();
    await page.keyboard.press("End");
    await expect(remove).toBeFocused();

    await page.keyboard.press("Escape");
    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    // The half a plain "Escape closes it" assertion would miss: closing
    // destroys the element that held focus, and without the handoff the
    // user lands on the document body with the row they were working on
    // dozens of Tab presses away.
    await expect(toggle).toBeFocused();
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * An archived row's SHORTER menu navigates on its own length.
 *
 * Archiving withdraws stop and archive, which moves delete from position
 * 4 to position 2 (clone, offered unconditionally, keeps position 1 in
 * both retention states). Nothing durable may remember the old number —
 * this is the bug that motivated keying mounted handles by ACTION rather
 * than by index — and wrapping has to happen on three, not on an assumed
 * five. Only a real browser can show that the surviving nodes registered
 * themselves under the shorter list.
 */
test("an archived row's three-item menu navigates on its own length", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `menu-archived-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    const archived = await request.post(`/api/sessions/${session.id}/archive`);
    expect(archived.ok(), await archived.text()).toBeTruthy();
    await page.goto("/");
    // The filter bar is on-demand chrome; its checkbox is not in the DOM
    // until the bar is open (see `openFilterBar`'s own doc).
    await openFilterBar(page);
    await page.locator(".filter-include-archived").check();
    await page.locator(".filter-apply").click();
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsStripSettled(page);

    await openRowMenu(target);
    const menu = target.getByRole("menu");
    await expect(menu.getByRole("menuitem")).toHaveText(["rename", "clone", "delete"]);

    const toggle = target.locator(".session-row-menu");
    const rename = target.locator(".session-row-rename");
    const clone = target.locator(".session-row-clone");
    const remove = target.locator(".session-row-delete");
    await toggle.focus();
    await page.keyboard.press("ArrowDown");
    await expect(rename).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(clone).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(remove).toBeFocused();
    // Wraps on THREE. A list that still believed it had five would leave
    // focus where it was, or reach for a handle nothing mounted.
    await page.keyboard.press("ArrowDown");
    await expect(rename).toBeFocused();
    await page.keyboard.press("End");
    await expect(remove).toBeFocused();
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * The item set changing UNDER an open menu re-numbers it, and navigation
 * follows.
 *
 * This is the regression for a bug the type system cannot prevent.
 * Archiving a session withdraws stop and archive while the panel stays
 * up; delete survives that change, and a scheme that filed its mounted
 * handle under "position 3" left that handle stranded at an index the
 * two-item list no longer reaches, while the key handler had already
 * moved on to the shorter numbering. Arrow, Home and End then targeted a
 * node that was not there and silently did nothing.
 *
 * The listing is STUBBED rather than driven through a real archive call,
 * for the same reason the refresh test below stubs it: the row must stay
 * at the same index across the change, and a shared stack with other
 * specs' sessions in it cannot promise that. Flipping one field in a
 * frozen fabrication isolates exactly the transition under test.
 */
test("archiving under an open menu renumbers it and navigation follows", async ({
  page,
  request,
}) => {
  const stamp = (await request.get("/api/sessions")).headers()["x-farhelm-build"] ?? "";
  expect(stamp, "the helm must stamp its replies").toBeTruthy();
  const sessionId = "menu-renumber-fixture-session";
  const listingBody = {
    sessions: [
      {
        id: sessionId,
        title: `menu-renumber-${Date.now()}`,
        cwd: "/tmp",
        invocation: "sleep 300",
        status: { state: "idle" },
        archived: false,
      },
    ],
    total: 1,
    matching: 1,
    truncated: false,
  };
  await page.route(
    (url) => url.pathname === "/api/sessions",
    async (route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      await route.fulfill({
        headers: { "x-farhelm-build": stamp, "content-type": "application/json" },
        json: listingBody,
      });
    },
  );
  const feed = await stubFeed(page);
  await page.goto("/");
  await feed.waitForConnection(1);
  feed.notify(1);

  const target = row(page, sessionId);
  await expect(target).toBeVisible({ timeout: 20_000 });
  await waitForHostsStripSettled(page);
  await openRowMenu(target);
  await expect(target.getByRole("menuitem")).toHaveCount(5);

  // The change lands through an ordinary refresh, with the row keeping
  // its place in the list — so nothing closes the menu, which is the
  // whole premise.
  listingBody.sessions[0].archived = true;
  const responded = page.waitForResponse(
    (r) => new URL(r.url()).pathname === "/api/sessions" && r.request().method() === "GET",
  );
  feed.notify(2);
  await responded;
  await expect(target).toHaveAttribute("data-session-archived", "true");
  await expect(target.locator(".session-row-menu-panel")).toBeVisible();
  await expect(target.getByRole("menuitem")).toHaveText(["rename", "clone", "delete"]);

  // Delete is at position 2 now, not 4, and the node that survived the
  // change answers to it — clone, offered unconditionally, keeps position
  // 1 in both retention states and does not need to be re-found here.
  const toggle = target.locator(".session-row-menu");
  await toggle.focus();
  await page.keyboard.press("ArrowDown");
  await expect(target.locator(".session-row-rename")).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(target.locator(".session-row-clone")).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(target.locator(".session-row-delete")).toBeFocused();
  await page.keyboard.press("End");
  await expect(target.locator(".session-row-delete")).toBeFocused();
});

/**
 * Opening the menu ENTERS it, the menu is a single tab stop, and the keys
 * it does not claim stay native.
 *
 * Four separate promises, all of which only exist in a real browser:
 *
 * - Pointer, Enter, Space and ArrowDown open onto the FIRST command and
 *   ArrowUp onto the last, so a keyboard user never opens a menu and then
 *   has to aim separately.
 * - The roving `tabindex` gives exactly one item `tabindex="0"`, which is
 *   what makes Tab mean "leave" rather than "walk four commands".
 * - Tab and Shift+Tab dismiss the menu onto the toggle, and the next Tab
 *   from a closed toggle continues out of the row natively — so focus is
 *   never trapped and never dropped.
 * - Enter and Space still activate the focused `<button>`. The pure key
 *   map asserts they are unclaimed; only the real handler can show that
 *   `prevent_default` was not hoisted above the mapping's early return,
 *   which would leave every unit test green and every command dead.
 */
test("opening the actions menu enters it, and Tab leaves it", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `menu-entry-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsStripSettled(page);
    // The lone session auto-attaches, and its terminal takes focus for
    // itself when the mount lands — which can be after the row is already
    // visible. Every step below is a focus move followed by a keystroke,
    // so a mount landing mid-sequence steals the focus the keystroke was
    // aimed at (ArrowDown reaching the terminal instead of the toggle; the
    // post-Escape refocus overridden — both observed on loaded CI runs).
    // Waiting for the mount makes this test's own moves the last ones.
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true, undefined, {
      timeout: 20_000,
    });

    const toggle = target.locator(".session-row-menu");
    const rename = target.locator(".session-row-rename");
    const clone = target.locator(".session-row-clone");
    const stop = target.locator(".session-row-stop");
    const remove = target.locator(".session-row-delete");

    // ArrowDown on a CLOSED toggle: opens and lands on the first command.
    //
    // The mount-ready wait above only rules out the FIRST steal. This
    // session's `sleep 300` invocation is not a real agent — nothing ever
    // proves the attach — so the reconnect ladder in terminal.js keeps
    // unmounting and remounting the island (see that file's "Auto-reconnect"
    // header comment), and every remount runs its own one-time `reveal()`.
    // `reveal()`'s own `takesFocus()` deliberately does NOT hold back for a
    // focused button — only for an editable control or another terminal —
    // so a reconnect landing in the gap between `.focus()` and the
    // assertion below steals focus right back out from under the toggle,
    // on WebKit under CI load (`chrome.spec.ts`'s focus-ring test documents
    // the same per-second churn against the same fixture). Retrying the
    // whole focus-then-check, rather than only waiting on the check, is
    // what lets a losing attempt recover: the next iteration re-focuses
    // after whatever reconnect just fired, instead of polling a focus that
    // is never coming back on its own.
    await expect(async () => {
      await toggle.focus();
      await expect(toggle).toBeFocused();
    }).toPass({ timeout: 10_000 });
    await page.keyboard.press("ArrowDown");
    await expect(target.locator(".session-row-menu-panel")).toBeVisible();
    await expect(rename).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(toggle).toBeFocused();

    // ArrowUp on a closed toggle opens at the OTHER end.
    await page.keyboard.press("ArrowUp");
    await expect(remove).toBeFocused();
    await page.keyboard.press("Escape");

    // A pointer open enters the menu too. `openRowMenu` clicks, which is
    // the ordinary path every other test in this file takes.
    await openRowMenu(target);
    await expect(rename).toBeFocused();

    // One tab stop, not five: the focused item carries it, everything
    // else is skipped by Tab.
    await expect(rename).toHaveAttribute("tabindex", "0");
    await expect(clone).toHaveAttribute("tabindex", "-1");
    await expect(stop).toHaveAttribute("tabindex", "-1");
    await expect(remove).toHaveAttribute("tabindex", "-1");
    await page.keyboard.press("ArrowDown");
    await expect(clone).toBeFocused();
    await expect(clone).toHaveAttribute("tabindex", "0");
    await expect(rename).toHaveAttribute("tabindex", "-1");

    // Tab dismisses the menu and lands on the toggle — the tab stop the
    // whole menu stands in for — rather than walking the remaining
    // commands one at a time.
    await page.keyboard.press("Tab");
    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await expect(toggle).toBeFocused();
    // And from there the next Tab is the browser's own again: a CLOSED
    // toggle claims neither Tab nor Shift+Tab, so focus continues out of
    // the row rather than being trapped on it.
    await page.keyboard.press("Tab");
    await expect(toggle).not.toBeFocused();

    // Shift+Tab out of an item is the same dismissal in the other
    // direction.
    await openRowMenu(target);
    await expect(rename).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(toggle).toBeFocused();

    // Enter activates the focused command natively.
    await openRowMenu(target);
    await expect(rename).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(target.locator(".rename-form")).toBeVisible();
    await target.locator(".rename-cancel").click();

    // And so does Space, on a different command, so that neither key is
    // proven only through the other. Focus is re-established from the
    // TOGGLE rather than assumed: cancelling the rename above unmounted
    // the button that held it, and the panel never closed, so there was
    // no fresh open to place focus anywhere. Same retry as above and for
    // the same reason — the reconnect ladder is still running this whole
    // test, not just at its start.
    await expect(target.locator(".session-row-menu-panel")).toBeVisible();
    await expect(async () => {
      await toggle.focus();
      await expect(toggle).toBeFocused();
    }).toPass({ timeout: 10_000 });
    await page.keyboard.press("ArrowDown");
    await expect(rename).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(clone).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(stop).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(target.locator(".session-row-archive")).toBeFocused();
    await page.keyboard.press("Space");
    await expect(target.locator(".confirm-archive")).toBeVisible();
    await target.locator(".archive-cancel").click();
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * The panel's prompt states answer NO menu keys, and the sub-state they
 * hold survives them.
 *
 * The toggle binds the navigation set only while the panel is showing its
 * items. Without that guard, Shift+Tab back to the toggle would offer an
 * Escape that dismisses the panel while leaving the confirmation
 * unanswered — and since `ListView` keeps confirming/renaming state
 * independently of whether the panel is open, the row would sit primed to
 * reopen straight back into a prompt the user thought they had dismissed.
 * An arrow would be worse still: it would reach for item handles that
 * belong to a list this state is not showing.
 */
test("a confirm prompt ignores the menu's keys and keeps its state", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `menu-prompt-keys-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsStripSettled(page);

    await openRowMenu(target);
    await target.locator(".session-row-delete").click();
    const consequence = target.locator(".confirm-consequence");
    await expect(consequence).toBeVisible();

    const toggle = target.locator(".session-row-menu");
    await toggle.focus();
    await page.keyboard.press("Escape");
    await expect(
      consequence,
      "Escape on the toggle must not dismiss a confirmation that was never answered",
    ).toBeVisible();
    await expect(toggle).toHaveAttribute("aria-expanded", "true");

    await page.keyboard.press("ArrowDown");
    await expect(consequence).toBeVisible();
    await expect(
      target.locator(".session-row-menu-item"),
      "there are no items in this state for an arrow to reach",
    ).toHaveCount(0);

    // Cancel is the only way out, and it is the one the prompt autofocuses.
    await target.locator(".confirm-cancel").click();
    await expect(target.locator(".session-row-stop")).toBeVisible();
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * An automatic dismissal hands focus back to the toggle instead of
 * dropping it on the document body.
 *
 * Escape is not the only way this menu closes. A sidebar scroll or
 * resize, the hosts panel or filter bar opening, the create form, and a
 * refresh that reorders the row all close it through `ListView`, which
 * owns `menu_open` and does not consult the row at all. If an item held
 * focus, unmounting it leaves the user at the top of the document — one
 * of the worst things a keyboard interface can do — and no test of the
 * Escape path would ever notice.
 *
 * A viewport RESIZE is the cause chosen here because it is the cheapest
 * to cause honestly: `.app-sidebar` is observed by a `ResizeObserver`
 * (lib.rs), so changing the window's height changes the sidebar's and
 * bumps the layout epoch, with no fixture of eighteen sessions needed to
 * make anything scroll.
 */
test("a menu dismissed by a layout change returns focus to its toggle", async ({
  page,
  request,
}) => {
  const session = await createSession(request, {
    title: `menu-dismiss-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  const original = page.viewportSize();
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsStripSettled(page);

    await openRowMenu(target);
    const toggle = target.locator(".session-row-menu");
    await expect(target.locator(".session-row-rename")).toBeFocused();

    await page.setViewportSize({
      width: original?.width ?? 1280,
      height: (original?.height ?? 720) - 120,
    });
    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(toggle).toBeFocused();
  } finally {
    if (original) await page.setViewportSize(original);
    await cleanupSession(request, session.id);
  }
});

/**
 * An in-flight operation makes the menu's commands inert WITHOUT making
 * them unfocusable.
 *
 * A browser cannot focus a natively `disabled` control, and the menu
 * keeps consuming Arrow/Home/End regardless — so an implementation that
 * used the `disabled` attribute here produced a menu that swallowed every
 * navigation key and honoured none of them, and an item that lost focus
 * the moment its own action made the row busy, putting Escape out of
 * reach with the panel still open. `aria-disabled` plus a guarded
 * `onclick` is the fix, and it is invisible to every other test: the
 * items still report as disabled to Playwright, still refuse clicks, and
 * still look inert.
 *
 * The stalled route is the whole fixture. It holds the shared operation
 * token for as long as this test needs the busy window to last, and is
 * released in `finally` so the request completes rather than being left
 * dangling.
 */
test("a busy menu stays navigable while refusing to act", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `menu-busy-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  let release = () => {};
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  try {
    await page.route(`**/api/sessions/${session.id}/stop`, async (route) => {
      await held;
      // Swallowed on purpose: by the time the gate opens the test may
      // already be tearing down, and a route whose page has gone away
      // rejects rather than resolving. The stop the row asked for is not
      // this test's subject — `cleanupSession` issues its own through the
      // API context, which `page.route` never touches.
      await route.continue().catch(() => {});
    });
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsStripSettled(page);
    // Same precaution, for the same reason, as "opening the actions menu
    // enters it" above: this test asserts where focus IS at several points,
    // and the lone session's terminal takes focus for itself when its mount
    // lands — which can be after the row is visible. Without this wait a
    // mount landing between the open and the first focus assertion steals
    // the focus the panel just placed, and the failure reads as a menu that
    // did not enter itself (the item holding the roving `tabindex` while
    // something else holds DOM focus) rather than as the race it is.
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true, undefined, {
      timeout: 20_000,
    });

    const toggle = target.locator(".session-row-menu");
    const rename = target.locator(".session-row-rename");
    const clone = target.locator(".session-row-clone");
    const stop = target.locator(".session-row-stop");
    const archive = target.locator(".session-row-archive");

    // Start a stalled action FROM a focused item: the item that made the
    // menu busy must keep its focus rather than being blurred out from
    // under the user mid-keystroke.
    await openRowMenu(target);
    await expect(rename).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(clone).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(stop).toBeFocused();
    await stop.click();
    await expect(stop).toHaveAttribute("aria-disabled", "true");
    await expect(stop, "the item that went busy must not lose focus").toBeFocused();

    // And the menu still navigates while every command in it is inert —
    // the property a native `disabled` cannot have.
    await page.keyboard.press("ArrowDown");
    await expect(archive).toBeFocused();
    await expect(archive).toHaveAttribute("aria-disabled", "true");
    await page.keyboard.press("Home");
    await expect(rename).toBeFocused();
    // Escape is reachable, which is the point of keeping focus inside a
    // busy menu at all.
    await page.keyboard.press("Escape");
    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(toggle).toBeFocused();

    // Opening DURING the global operation is the other direction of the
    // same property: nothing has ever been focused inside this panel, and
    // the open still lands on the first command.
    await openRowMenu(target);
    await expect(rename).toBeFocused();
    await expect(rename).toHaveAttribute("aria-disabled", "true");
  } finally {
    release();
    await page.unroute(`**/api/sessions/${session.id}/stop`).catch(() => {});
    await cleanupSession(request, session.id);
  }
});

/**
 * An open panel leaves every OTHER row's "⋯" clickable.
 *
 * This is the anchor's reason for existing, stated as the failure it
 * prevents. The panel extends downward over the rows below it, and at the
 * point where those rows draw their own toggle it is showing `stop` and
 * `delete` — a click aimed at a neighbouring session's menu would stop
 * that session's process tree or delete the row outright, for the WRONG
 * session, with no confirmation step in between. Anchoring the panel to
 * the LEFT of the toggle column is what makes that impossible, and only a
 * coordinate-level click can prove it: a Playwright `click()` on the
 * locator would helpfully scroll and retarget, hiding exactly the overlap
 * this test is about.
 */
test("an open menu leaves the other rows' toggles clickable", async ({ page, request }) => {
  const a = await createSession(request, {
    title: `menu-cover-a-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  let b: { id: string } | undefined;
  try {
    b = await createSession(request, {
      title: `menu-cover-b-${Date.now()}`,
      cwd: "/tmp",
      invocation: "sleep 300",
    });
    await page.goto("/");
    await expect(row(page, a.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, b.id)).toBeVisible({ timeout: 20_000 });
    await waitForHostsStripSettled(page);

    const firstIsA = (await row(page, a.id).boundingBox())!.y < (await row(page, b.id).boundingBox())!.y;
    const first = firstIsA ? row(page, a.id) : row(page, b.id);
    const second = firstIsA ? row(page, b.id) : row(page, a.id);

    await openRowMenu(first);
    const panel = first.locator(".session-row-menu-panel");
    const panelBox = (await panel.boundingBox())!;
    const coveredToggle = second.locator(".session-row-menu");
    const toggleBox = (await coveredToggle.boundingBox())!;
    // The geometric statement of the anchor: the panel's right edge stops
    // before the toggle column starts.
    expect(
      panelBox.x + panelBox.width,
      "the panel must not extend into the column the rows below draw their own toggles in",
    ).toBeLessThanOrEqual(toggleBox.x);

    // And the behavioral one. A raw coordinate click is what a user aiming
    // at the visible "⋯" actually does.
    await page.mouse.click(toggleBox.x + toggleBox.width / 2, toggleBox.y + toggleBox.height / 2);

    await expect(coveredToggle).toHaveAttribute("aria-expanded", "true");
    await expect(first.locator(".session-row-menu")).toHaveAttribute("aria-expanded", "false");
    // Nothing destructive was reached on the way: a covered `delete`
    // would have swapped the first row's panel for a confirm prompt, and
    // a covered `stop` would have left its row busy.
    await expect(page.locator(".confirm-consequence")).toHaveCount(0);
    await expect(first).toBeVisible();
  } finally {
    await cleanupSession(request, a.id);
    if (b) await cleanupSession(request, b.id);
  }
});

/**
 * The row and its toggle say which menu is open, in computed style, after
 * the pointer and the keyboard have both moved away.
 *
 * The panel covers the rows below it, so "which of the several visible ⋯
 * did I open?" has to be answerable from the row itself. Existing tests
 * check the class string and the toggle's opacity, neither of which would
 * notice a missing selector or a specificity regression that let the
 * hover rule win — and the cue that matters most is precisely the one
 * that has to survive the pointer leaving, since a menu stays up after
 * the mouse has moved off.
 *
 * The selected row is checked separately because it is the case where two
 * rules compete: the neutral menu tint would erase the accent fill
 * SPEC.md requires the current session to keep.
 */
test("an open menu tints its own row and presses its own toggle", async ({ page, request }) => {
  const a = await createSession(request, {
    title: `menu-cues-a-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  let b: { id: string } | undefined;
  try {
    b = await createSession(request, {
      title: `menu-cues-b-${Date.now()}`,
      cwd: "/tmp",
      invocation: "sleep 300",
    });
    await page.goto("/");
    const selectedRow = row(page, a.id);
    const plainRow = row(page, b.id);
    await expect(selectedRow).toBeVisible({ timeout: 20_000 });
    await expect(plainRow).toBeVisible({ timeout: 20_000 });
    await waitForHostsStripSettled(page);

    const [hoverTint, accent, selectedTint] = await Promise.all([
      tokenColor(page, "--bg-2"),
      tokenColor(page, "--accent"),
      tokenColor(page, "--accent-fill-hover"),
    ]);
    // Selection is established BEFORE any menu is opened: an open panel
    // hangs over the row's own open button, so a click aimed at it would
    // be intercepted by the very surface under test.
    await selectedRow.locator(".session-row-open").click();
    await expect(selectedRow).toHaveAttribute("data-session-selected", "true", { timeout: 20_000 });
    await expect(plainRow).toHaveAttribute("data-session-selected", "false");

    // Both the pointer and the keyboard are parked OUTSIDE the row before
    // every reading: hover and `:focus-within` reveal the toggle too, so
    // leaving either in place would let this pass with the
    // `aria-expanded` rules missing entirely.
    const styles = async (target: Locator) => {
      await page.mouse.move(0, 0);
      await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
      return target.evaluate((element) => {
        const rowStyle = getComputedStyle(element);
        const toggleStyle = getComputedStyle(element.querySelector(".session-row-menu")!);
        return {
          rowBackground: rowStyle.backgroundColor,
          toggleColor: toggleStyle.color,
          toggleOpacity: toggleStyle.opacity,
        };
      });
    };

    const closed = await styles(plainRow);
    expect(closed.rowBackground).not.toBe(hoverTint);
    expect(closed.toggleColor).not.toBe(accent);

    await openRowMenu(plainRow);
    // The toggle's opacity is transitioned (100ms, app.css), and the
    // reveal that matters here — `aria-expanded` — starts that fade from
    // whatever the hover reveal left it at. A one-shot read right after
    // the open landed mid-fade (`0.9835…`, observed on a loaded CI run);
    // the polling assertion waits for it to settle, with the pointer and
    // keyboard already parked outside the row so nothing else holds it up.
    await page.mouse.move(0, 0);
    await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
    await expect(
      plainRow.locator(".session-row-menu"),
      "the toggle stays visible while its menu is up",
    ).toHaveCSS("opacity", "1");
    const open = await styles(plainRow);
    expect(open.rowBackground, "the owning row holds the tint with nothing hovering it").toBe(
      hoverTint,
    );
    expect(open.toggleColor, "the pressed toggle wears the accent").toBe(accent);
    expect(open.toggleOpacity, "and stays visible while its menu is up").toBe("1");

    // The SELECTED row keeps its accent fill rather than taking the
    // neutral tint: SPEC.md requires the current session to stay readable
    // at a glance, and three classes deep is where a specificity
    // regression would quietly hand that to the menu tint.
    await openRowMenu(selectedRow);
    const selected = await styles(selectedRow);
    expect(selected.rowBackground).toBe(selectedTint);
    expect(selected.toggleColor).toBe(accent);
  } finally {
    await cleanupSession(request, a.id);
    if (b) await cleanupSession(request, b.id);
  }
});

/**
 * The menu is a RAISED surface of full-bleed rows, not a stack of
 * outlined buttons in a box.
 *
 * That is the entire visual half of the redesign, and nothing else
 * asserts it: the old appearance — a bordered box per item, each inset
 * from the panel's edges — would come back without breaking a single
 * behavioral test. The shadow is the load-bearing one, because this panel
 * floats over other session rows and would otherwise read as part of the
 * list rather than in front of it.
 *
 * The width assertions are also the regression for a real box-model bug:
 * an item is `width: 100%` and carries 10px of padding plus a 1px border
 * on each side, so without `box-sizing: border-box` every row is 22px
 * wider than the panel and spills past its rounded corners.
 */
test("the actions menu is a raised surface of full-bleed rows", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `menu-visuals-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsStripSettled(page);
    await openRowMenu(target);

    const panel = target.locator(".session-row-menu-panel");
    const surface = await panel.evaluate((element) => {
      const style = getComputedStyle(element);
      return {
        background: style.backgroundColor,
        borderWidth: style.borderTopWidth,
        shadow: style.boxShadow,
      };
    });
    expect(surface.background).toBe(await tokenColor(page, "--bg-2"));
    expect(surface.borderWidth).toBe("1px");
    expect(surface.shadow, "the shadow is what says this is in FRONT of the rows").not.toBe("none");

    // Items: borderless, left-aligned, and exactly as wide as the panel's
    // content box, so their hover fill reaches both edges.
    const items = await panel.evaluate((element) => {
      const panelBox = element.getBoundingClientRect();
      return [...element.querySelectorAll(".session-row-menu-item")].map((item) => {
        const style = getComputedStyle(item);
        const box = item.getBoundingClientRect();
        return {
          borderColor: style.borderTopColor,
          textAlign: style.textAlign,
          overhangLeft: panelBox.left - box.left,
          overhangRight: box.right - panelBox.right,
        };
      });
    });
    expect(items).toHaveLength(5);
    for (const item of items) {
      // `.btn` reserves a 1px border so an opaque edge costs no layout
      // shift; on a menu item it must stay fully transparent.
      expect(item.borderColor).toBe("rgba(0, 0, 0, 0)");
      expect(item.textAlign).toBe("left");
      expect(item.overhangLeft).toBeLessThanOrEqual(0);
      expect(item.overhangRight).toBeLessThanOrEqual(0);
    }

    // The one rule in the list, on its own element between archive and
    // delete rather than drawn on delete itself.
    const separator = panel.locator(".session-row-menu-separator");
    await expect(separator).toHaveCount(1);
    expect(
      await separator.evaluate((element) => getComputedStyle(element).borderTopColor),
    ).toBe(await tokenColor(page, "--border-dim"));
    const separatorBox = (await separator.boundingBox())!;
    const deleteBox = (await panel.locator(".session-row-delete").boundingBox())!;
    const archiveBox = (await panel.locator(".session-row-archive").boundingBox())!;
    expect(separatorBox.y).toBeGreaterThanOrEqual(archiveBox.y + archiveBox.height);
    expect(separatorBox.y).toBeLessThanOrEqual(deleteBox.y);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * The panel stays inside a NARROW viewport, and no wider than the room
 * actually left in it.
 *
 * The measured placement derives its `max-width` from the toggle's own
 * coordinate, while the `right` inset it is paired with is separately
 * floored at 8px. When the two disagree — a narrow shell where the toggle
 * sits at or past the viewport's right edge — the width can be computed
 * from space that is not there and take the panel's LEFT edge off screen.
 * The fix is a `calc(100vw - 16px)` term the engine resolves at paint
 * time, which is exactly what this asserts.
 *
 * Honest about what it does NOT do: the arithmetic band where the old
 * clamp actually overflowed needs a viewport narrower than the sidebar's
 * fixed 340px by enough that the toggle is no longer reachable to click,
 * so this is an invariant check at the narrowest width the menu can still
 * be opened at, not a reproduction. The clamp's own shape is pinned
 * exactly in `menu_panel.rs`'s unit tests, which is where the emitted
 * expression can be read directly.
 */
test("the actions panel stays inside a narrow viewport", async ({ page, request }) => {
  const session = await createSession(request, {
    // A long title is the panel's own widest content: the confirm state
    // quotes it, and a panel that grew to fit would be the one to spill.
    title: `menu-narrow-${"wide".repeat(20)}-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  const original = page.viewportSize();
  try {
    await page.setViewportSize({ width: 360, height: 700 });
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsStripSettled(page);

    await openRowMenu(target);
    const panel = target.locator(".session-row-menu-panel");
    const inViewport = async () => {
      const box = (await panel.boundingBox())!;
      expect(box.x).toBeGreaterThanOrEqual(0);
      expect(box.x + box.width).toBeLessThanOrEqual(360);
      expect(box.width).toBeLessThanOrEqual(360 - 16);
    };
    await inViewport();

    // The confirm state is the widest thing this panel ever holds.
    await target.locator(".session-row-delete").click();
    await expect(target.locator(".confirm-consequence")).toBeVisible();
    await inViewport();
    await target.locator(".confirm-cancel").click();
  } finally {
    if (original) await page.setViewportSize(original);
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
 * A LOCAL session's host line is PROVISIONAL, not a settled fact, until the
 * registry actually answers — it shows the moment the listing renders, and
 * disappears the instant `/api/hosts` confirms this session's host id IS
 * the local row.
 *
 * `shared::session_is_local` answers "not local" for every unknown
 * (including "the hosts read has not landed yet"), and unknown locality
 * never SUPPRESSES an available host label (SPEC_impl.md) — so a session
 * that is in fact local still names its host for as long as the registry
 * has not confirmed otherwise. The route hold makes that window
 * deterministic instead of racing a fast helm: without it, `/api/hosts`
 * ordinarily answers before the first paint and the provisional state is
 * never actually observed.
 */
test("a local session's host line is provisional until the registry confirms it", async ({
  page,
  request,
}) => {
  const session = await createSession(request, {
    title: `provisional-host-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  let releaseHosts: () => void = () => {};
  const hostsHeld = new Promise<void>((resolve) => {
    releaseHosts = resolve;
  });
  await page.route(
    (url) => url.pathname === "/api/hosts",
    async (route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      await hostsHeld;
      await route.continue();
    },
  );
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await expect(target.locator(".session-host")).toBeVisible();
    await expect(target.locator(".session-host")).toContainText("this machine");

    releaseHosts();
    await expect(target.locator(".session-host")).toHaveCount(0);
  } finally {
    await cleanupSession(request, session.id);
  }
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
  // `.host-profiles-toggle` now lives inside the host row's own "⋯" menu.
  const firstHostRow = page.locator(".host-row").first();
  await openHostMenu(firstHostRow);
  const toggleProfiles = firstHostRow.locator(".host-profiles-toggle");
  await toggleProfiles.click();
  await expect(page.locator(".profiles-section")).toBeVisible({ timeout: 20_000 });

  await page.locator(".hosts-toggle").click();
  await expect(page.locator(".hosts-panel")).toBeHidden();
  await openHostsPanel(page);
  await expect(page.locator(".profiles-section")).toHaveCount(0);
});

/**
 * F10/TEST-CROSS-MENU: host menus and session menus keep separate
 * open-menu signals, and each toggle's own callback explicitly clears the
 * OTHER signal when it opens (`HostsPanel`'s own doc calls this "one row
 * menu open, across BOTH panels"). Nothing before this opened one kind and
 * then the other and checked that the first actually closed — removing
 * either cross-close write would leave two floating panels mounted at
 * once, and both are `position: fixed` panels that can overlap unrelated
 * rows, making a command — including a destructive one — look attached to
 * the wrong owner.
 *
 * Both directions: the two signals are written independently, so a
 * regression in either write is invisible to a test that only opens the
 * other order.
 */
test("opening one row's menu closes the other row kind's open one", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `cross-menu-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const sessionRow = row(page, session.id);
    await expect(sessionRow).toBeVisible({ timeout: 20_000 });
    await openHostsPanel(page);
    const hostRow = page.locator(".host-row").first();

    // Session menu open, then a host menu opens: the session's must close.
    await openRowMenu(sessionRow);
    await expect(sessionRow.locator(".session-row-menu-panel")).toBeVisible();
    await openHostMenu(hostRow);
    await expect(sessionRow.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(sessionRow.locator(".session-row-menu")).toHaveAttribute("aria-expanded", "false");
    await expect(hostRow.locator(".host-row-menu-panel")).toBeVisible();

    // The reverse: with the host menu still open, a session menu opens —
    // the host's must close.
    await openRowMenu(sessionRow);
    await expect(hostRow.locator(".host-row-menu-panel")).toHaveCount(0);
    await expect(hostRow.locator(".host-row-menu")).toHaveAttribute("aria-expanded", "false");
    await expect(sessionRow.locator(".session-row-menu-panel")).toBeVisible();
  } finally {
    await cleanupSession(request, session.id);
  }
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
    // The preference write is fire-and-forget: only the helm's row itself
    // says the click was persisted, and reloading before it lands would
    // race the very state this test restores from.
    await expect
      .poll(async () => (await readPreferences(request)).last_selected, { timeout: 20_000 })
      .toBe(older.id);
    await page.reload();
    await expect(page.locator(".titlebar .title")).toContainText("policy-older-", {
      timeout: 20_000,
    });
    // The auto-selection is a real attachment, not just a mounted view.
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    // A stale remembered id — a session that no longer exists — falls
    // back to the newest-created non-archived session.
    await patchPreferences(request, {
      last_selected: "00000000-0000-0000-0000-000000000000",
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

    // Client one's click is the ONLY preference write in this test: the
    // second client must inherit the selection from the helm's shared row.
    // Seeding it here (the old pinAutoSelect staging) would let the test
    // pass with client one's click never persisting anything — it would
    // prove client two can read a hand-planted value, not that a click in
    // one client is what another client opens with.
    await expect
      .poll(async () => (await readPreferences(request)).last_selected, { timeout: 20_000 })
      .toBe(session.id);

    // The second client merely LOADS.
    second = await browser.newContext({
      storageState: await page.context().storageState(),
    });
    const page2 = await second.newPage();
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
 * A `/home/<user>` cwd shows the FOLDED `~` form while the row's `title`
 * carries the exact, unabbreviated path — the untouched value is one
 * hover away no matter how the visible text was shortened
 * (`row::abbreviate_home`).
 *
 * Route-controlled rather than a real create: SPEC.md refuses a create
 * whose cwd does not exist on the target host, and `/home/alice` need not
 * exist on whatever machine runs this suite, so the shape-based fold can
 * only be proven here against a fabricated reply.
 */
test("a /home/<user> cwd shows the folded form with the exact path on title", async ({
  page,
  request,
}) => {
  const stamp = (await request.get("/api/sessions")).headers()["x-farhelm-build"] ?? "";
  expect(stamp, "the helm must stamp its replies").toBeTruthy();
  const cwd = "/home/alice/src/api";
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    await route.fulfill({
      headers: { "x-farhelm-build": stamp, "content-type": "application/json" },
      json: {
        sessions: [
          {
            id: "tilde-fold-session",
            title: "tilde-fold",
            cwd,
            invocation: "sleep 300",
          },
        ],
        total: 1,
        matching: 1,
        truncated: false,
      },
    });
  });
  await page.goto("/");
  const target = row(page, "tilde-fold-session");
  await expect(target).toBeVisible({ timeout: 20_000 });
  await expect(target.locator(".session-cwd-text")).toHaveText("~/src/api");
  await expect(target.locator(".session-cwd")).toHaveAttribute("title", cwd);
});

/**
 * A directional override (U+202E) inside a program name renders as a
 * visible escape, isolated in its own element — the same two-layer defence
 * every other peer-supplied value on this page gets (`crate::peer`), now
 * pinned specifically for the invocation badge's basename span.
 *
 * `display_peer` (a Rust unit test) already proves the escaping half; what
 * only a real browser can prove is the isolation half — that the override
 * cannot reach past its own span and reorder a sibling. There is no marker
 * to reorder here (the corrupted basename cannot match a recognized vendor
 * — see `row::tests::a_bidi_override_in_the_basename_survives_unmangled_
 * and_earns_no_false_marker`), so what this asserts is the computed
 * direction staying `ltr` under the isolate, exactly as the cwd's own bidi
 * regression above does for the same construction.
 */
test("a bidi override in the invocation basename renders escaped and isolated", async ({
  page,
  request,
}) => {
  const stamp = (await request.get("/api/sessions")).headers()["x-farhelm-build"] ?? "";
  expect(stamp, "the helm must stamp its replies").toBeTruthy();
  // Built from its code point, not the literal glyph, so the override
  // cannot silently reorder this SOURCE FILE'S own text around it.
  const rlo = String.fromCharCode(0x202e);
  const invocation = `/opt/bin/${rlo}evil-agent --some-flag`;
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    await route.fulfill({
      headers: { "x-farhelm-build": stamp, "content-type": "application/json" },
      json: {
        sessions: [
          {
            id: "bidi-invocation-session",
            title: "bidi-invocation",
            cwd: "/tmp",
            invocation,
          },
        ],
        total: 1,
        matching: 1,
        truncated: false,
      },
    });
  });
  await page.goto("/");
  const target = row(page, "bidi-invocation-session");
  await expect(target).toBeVisible({ timeout: 20_000 });
  const basename = target.locator(".session-invocation .peer-value");
  // Escaped to a visible `<U+202E>` form rather than an invisible control
  // character (`display_peer`), and the raw override still rides along on
  // `title` unmangled — the full truth is a hover away, never on screen.
  await expect(basename).toHaveText("<U+202E>evil-agent");
  await expect(target.locator(".session-invocation")).toHaveAttribute("title", invocation);
  expect(await basename.evaluate((el) => getComputedStyle(el).direction)).toBe("ltr");
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
