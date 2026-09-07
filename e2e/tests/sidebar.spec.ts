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
 * reconciliation) and the sidebar's on-demand chrome (host details, the
 * filter popover, and the applied-filter note).
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
import { expect, newObservedContext, test } from "./helpers/evidence";
import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import {
  cleanupProfile,
  cleanupSession,
  countReads,
  createProfile,
  createSession,
  forceBuildSkew,
  hideSeenState,
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
import { waitForSessionReady, waitForSessionRevealed } from "./helpers/terminal-readiness";

function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

/**
 * The id of the shared, always-present `e2e-session` fixture
 * (`terminal-suite.ts`'s `resetStack` relaunches it before every spec
 * file), for pinning auto-select away from a test's OWN fixture rows —
 * see `pinAutoSelect`'s own doc for why that pin is needed at all.
 */
async function sharedSessionId(request: APIRequestContext): Promise<string> {
  const listing = await (await request.get("/api/sessions")).json();
  const shared = listing.sessions.find((s: { title: string }) => s.title === "e2e-session");
  expect(shared, "the shared e2e-session must exist").toBeTruthy();
  return shared.id;
}

/**
 * The sidebar's first bar identifies the build answering the page before a
 * user has to interpret the separate mismatch notice. The fixture helm and
 * the bundle are the same build, so an agreeing reply cannot tell whether the
 * readout is wired to the helm's stamp at all: the first half forces every
 * API reply to carry a different stamp and expects THAT in the bar while the
 * tooltip keeps naming the real client build; the second half is the healthy
 * case, where the two are the same string.
 */
test("the sidebar app bar shows the helm build and client tooltip", async ({ page, request }) => {
  const stamp = (await request.get("/api/sessions")).headers()["x-farhelm-build"] ?? "";
  expect(stamp, "the helm must stamp its replies").toBeTruthy();
  const forced = "9.9.9-forced-helm";
  expect(forced).not.toBe(stamp);

  await forceBuildSkew(page, forced);
  await page.goto("/");
  const version = page.locator(".app-version");
  await expect(version).toHaveText(forced);
  await expect(version).toHaveAttribute("title", `this client was built as farhelm ${stamp}`);

  // Mounting host rows can leave provisioning reads inside route.fetch even
  // after the version is visible. Drain those handlers before removing the
  // interception pattern, or their later fulfill races an already handled route.
  await page.unrouteAll({ behavior: "wait" });
  await page.goto("/");
  await expect(version).toHaveText(stamp);
  await expect(version).toHaveAttribute("title", `this client was built as farhelm ${stamp}`);
});

/**
 * The readout only earns "always visible" if it survives the sidebar
 * scrolling: `.app-sidebar` is the scroll container for the whole session
 * list, so an ordinary first child would leave with the rows. Fills the
 * sidebar past one screen, scrolls it to the bottom, and expects the bar to
 * still sit at the sidebar's visible top edge.
 */
test("the sidebar app bar stays pinned while the session list scrolls", async ({ page, request }) => {
  const marker = `appbar-scroll-${Date.now()}`;
  const created = await fillSidebarPastOneScreen(request, marker);
  try {
    await page.setViewportSize({ width: 900, height: 500 });
    await page.goto("/");
    await expect(row(page, created[0].id)).toBeVisible({ timeout: 20_000 });
    const sidebar = page.locator(".app-sidebar");
    await expect
      .poll(() => sidebar.evaluate((el) => el.scrollHeight > el.clientHeight), { timeout: 20_000 })
      .toBe(true);
    await sidebar.evaluate((el) => {
      el.scrollTop = el.scrollHeight;
    });
    await expect.poll(() => sidebar.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);

    const bar = page.locator(".app-bar");
    await expect(bar).toBeVisible();
    const barBox = await bar.boundingBox();
    const sidebarBox = await sidebar.boundingBox();
    expect(barBox, "the app bar must have a box").not.toBeNull();
    expect(sidebarBox, "the sidebar must have a box").not.toBeNull();
    expect(Math.abs(barBox!.y - sidebarBox!.y)).toBeLessThanOrEqual(1);
  } finally {
    await cleanupAll(request, created);
  }
});

/**
 * The sidebar scrolls without a scrollbar. On macOS the overlay scrollbar
 * painted over the row menus and controls at the column's right edge, and a
 * classic scrollbar (which the OS can force on) would reserve a gutter the
 * column cannot spare, so the stylesheet hides it on both declared
 * scrollers. Fills the sidebar past one screen, then checks three things:
 * both scrollers resolve `scrollbar-width: none`, the sidebar takes no
 * gutter (its content box spans its padding box), and a wheel gesture over
 * it still scrolls. Only the gutter check measures geometry the rule
 * changes, and only where the engine draws a classic space-taking
 * scrollbar (Playwright's Linux Chromium does); under overlay scrollbars,
 * macOS's included, the gutter is zero either way and the reported bug —
 * the overlay painting over the row controls — is verified by eye there.
 * The wheel check is what keeps this from passing against
 * `overflow: hidden`, which would also show no scrollbar.
 */
test("the sidebar hides its scrollbar without giving up scrolling", async ({ page, request }) => {
  const marker = `sidebar-scrollbar-${Date.now()}`;
  const created = await fillSidebarPastOneScreen(request, marker);
  try {
    await page.setViewportSize({ width: 900, height: 500 });
    await page.goto("/");
    await expect(row(page, created[0].id)).toBeVisible({ timeout: 20_000 });
    const sidebar = page.locator(".app-sidebar");
    await expect
      .poll(() => sidebar.evaluate((el) => el.scrollHeight > el.clientHeight), { timeout: 20_000 })
      .toBe(true);
    // Pinned at rest before the wheel, so the wheel step can never become
    // dead weight behind some future scroll-on-open behavior: the
    // assertion below is "the wheel moved it", not "it is scrolled".
    expect(await sidebar.evaluate((el) => el.scrollTop), "the sidebar rests at the top").toBe(0);

    for (const selector of [".app-sidebar", ".session-list"]) {
      expect(
        await page.locator(selector).evaluate((el) => getComputedStyle(el).scrollbarWidth),
        `${selector} must resolve scrollbar-width: none`,
      ).toBe("none");
    }
    // A classic scrollbar narrows clientWidth below the padding box; with
    // none, the only difference between offsetWidth and clientWidth is the
    // sidebar's own border.
    const gutter = await sidebar.evaluate((el) => {
      const cs = getComputedStyle(el);
      const borders = parseFloat(cs.borderLeftWidth) + parseFloat(cs.borderRightWidth);
      return el.offsetWidth - el.clientWidth - borders;
    });
    expect(gutter, "no horizontal space may be reserved for a scrollbar").toBeLessThanOrEqual(1);

    const box = await sidebar.boundingBox();
    expect(box, "the sidebar must have a box").not.toBeNull();
    await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
    await page.mouse.wheel(0, 300);
    await expect
      .poll(() => sidebar.evaluate((el) => el.scrollTop), {
        message: "a wheel gesture over the sidebar must still scroll it",
      })
      .toBeGreaterThan(0);
  } finally {
    await cleanupAll(request, created);
  }
});

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
 * Wait for the always-visible host list's own mount-time read to
 * land, before opening a row menu whose test cares WHY the menu later
 * closes (or stays open).
 *
 * `hosts_list_shape` (list.rs) is one of the signals the menu's own
 * close-on-layout-shift effect watches: the list changing shape — its
 * "loading hosts…" note giving way to rows, or a refresh-error line
 * — is itself an internal layout cause, indistinguishable from whatever
 * cause a test means to isolate (a toggle click, a real scroll, a resize)
 * unless this read has already settled before the menu opens. Without
 * this, a dismissal test can pass for the wrong reason (closed by the
 * list's own async landing, not by the cause under test), and a
 * stays-open test can flake on unrelated host-read timing.
 */
async function waitForHostsListSettled(page: Page): Promise<void> {
  await expect(
    page.locator(".hosts-status", { hasText: "loading hosts" }),
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
 * title and invocation badge above a host/directory line.
 *
 * The stacked layout's whole reason to exist is that the old single-line
 * row overflowed the 340px column under exactly this content (the MT-8
 * class); a wrapper or flex regression that restored horizontal packing
 * or let a line force the row wide would pass every text-content
 * assertion and fail only here. The geometry asserted below is the
 * current layout: title and agent badge share the identity line, while host
 * and directory overlap vertically on the second line.
 *
 * Two further properties ride along because this is the cheapest local
 * session in the suite to assert them on: a local session still names its
 * host on the second line, and the whole row fits in a height that the old
 * four-line layout could not have.
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
    // Title and invocation share the identity line: overlapping vertical
    // extents, badge to the right of the identity copy. Asserted as an overlap
    // rather than as equal `y` because the two have different font sizes
    // and sit on a shared baseline, so their boxes start at different
    // offsets by design.
    expect(invocation.y).toBeLessThan(title.y + title.height);
    expect(title.y).toBeLessThan(invocation.y + invocation.height);
    expect(
      invocation.x,
      "the invocation badge follows the title on the identity line",
    ).toBeGreaterThanOrEqual(title.x + title.width - 1);

    // The second line names the rendered host even when locality is local;
    // the icon and text answer different questions and remain independently
    // truthful.
    await expect(target.locator(".session-host")).toHaveText("this machine");
    await expect(target.locator(".session-host-separator")).toHaveText(":");
    await expect(target).toHaveAttribute("data-host-locality", "local");
    await expect(target.locator(".host-kind-icon")).toHaveCount(1);
    // `data-glyph`, not just the count and the hidden word: those two alone
    // would still pass if the LOCAL and REMOTE svg components were swapped,
    // since neither depends on which shape actually rendered.
    await expect(target.locator(".host-kind-icon")).toHaveAttribute("data-glyph", "local");
    await expect(target.locator(".host-kind-icon + .visually-hidden")).toHaveText("local");

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
 * The host/directory line reads as one `host:~/path` string aligned under
 * the title. Three geometry claims the stylesheet makes and nothing else
 * checks: the line's text starts at the identity copy's x rather than the
 * status dot's, the host, the colon, and the path abut with no gap between
 * them, and the colon is set at the same size as the two runs it joins
 * (it used to inherit the button's larger size and read as a mark between
 * two smaller strings). Tolerances of one pixel because Chromium and
 * WebKit round subpixel boxes differently.
 */
test("the host/directory line aligns under the title as one continuous string", async ({
  page,
  request,
}) => {
  const marker = `meta-line-${Date.now()}`;
  const session = await createSession(request, {
    title: marker,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await expect(target.locator(".session-host")).toHaveText("this machine");

    const dot = (await target.locator(".session-status-slot").boundingBox())!;
    const title = (await target.locator(".session-title").boundingBox())!;
    const host = (await target.locator(".session-host").boundingBox())!;
    const separator = (await target.locator(".session-host-separator").boundingBox())!;
    // The path's TEXT, not its container: `.session-cwd` is the flex item
    // that abuts the colon whatever happens, while the glyphs inside it are
    // placed by its rtl clipper's `text-align: left` — drop that and the
    // path would sit at the far right of the line with the container still
    // touching the colon.
    const path = (await target.locator(".session-cwd-text").boundingBox())!;

    expect(host.x, "the line starts under the title, not under the status dot").toBeGreaterThan(
      dot.x + 1,
    );
    expect(Math.abs(host.x - title.x), "the line's left edge sits on the title's").toBeLessThanOrEqual(
      1,
    );
    expect(
      Math.abs(separator.x - (host.x + host.width)),
      "no gap between the host and the colon",
    ).toBeLessThanOrEqual(1);
    expect(
      Math.abs(path.x - (separator.x + separator.width)),
      "no gap between the colon and the path's text",
    ).toBeLessThanOrEqual(1);

    const fontSizes = await target.evaluate((el) =>
      [".session-host", ".session-host-separator", ".session-cwd"].map(
        (selector) => getComputedStyle(el.querySelector(selector)!).fontSize,
      ),
    );
    expect(
      new Set(fontSizes).size,
      `host, colon, and path must share one font size, got ${fontSizes.join(", ")}`,
    ).toBe(1);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * A long PROFILE-backed invocation badge clips inside its first-line column,
 * while the cwd on the second line remains usable.
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
  const profile = await createProfile(request, { name });
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
    // The directory remains usable on its separate host/cwd line; an
    // unbounded badge cannot squeeze it because the two fields no longer
    // share a flex row.
    expect(
      cwdBox.width,
      "the directory must keep a usable share of the shared line, not be squeezed to nothing " +
        "by an unbounded badge",
    ).toBeGreaterThan(20);
  } finally {
    await cleanupSession(request, session.id);
    await cleanupProfile(request, profile.id);
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
    // This test is about the ARIA relationship, not the seen-state feature
    // (which has its own tests) — hiding the field keeps the item list at
    // its fixed six regardless of whether the real supervisor's
    // classifier has settled this fixture into a live status by the time
    // the menu opens (see `hideSeenState`'s own doc for why that race is
    // otherwise real, not hypothetical).
    await hideSeenState(page);
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsListSettled(page);

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
      "replace",
      "stop",
      "archive",
      "delete",
    ]);
    // The boundary before the destructive item exists in the tree, not
    // only in the paint — six consecutive commands with nothing marking
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
 * and it cannot prove any of what this proves: that all six items
 * mounted, that each registered a handle under its own action, and that
 * the positions the key handler derives from `MenuOrder` line up with the
 * order the panel actually renders. A previous version of this test
 * walked two of the four items THEN offered, which left archive and delete
 * — the two with separately duplicated wiring, and the two whose misfire
 * is destructive — covered by nothing at all; clone and then replace
 * joined the walk when they joined the menu, for the same reason.
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
    // Fixed six-item navigation, not the seen-state feature — see
    // `hideSeenState`'s own doc.
    await hideSeenState(page);
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    // A host read landing mid-test changes the permanent list's shape,
    // which list.rs treats as a layout cause and closes any open menu for
    // — see the clipping test below for the same precaution.
    await waitForHostsListSettled(page);

    await openRowMenu(target);
    const toggle = target.locator(".session-row-menu");
    const rename = target.locator(".session-row-rename");
    const clone = target.locator(".session-row-clone");
    const replace = target.locator(".session-row-replace");
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
    await expect(replace).toBeFocused();
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
 * 5 to position 3 (clone and replace, both offered unconditionally, keep
 * positions 1 and 2 in both retention states). Nothing durable may
 * remember the old number — this is the bug that motivated keying mounted
 * handles by ACTION rather than by index — and wrapping has to happen on
 * four, not on an assumed six. Only a real browser can show that the
 * surviving nodes registered themselves under the shorter list.
 */
test("an archived row's four-item menu navigates on its own length", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `menu-archived-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    const archived = await request.post(`/api/sessions/${session.id}/archive`);
    expect(archived.ok(), await archived.text()).toBeTruthy();
    await page.goto("/");
    // The filter popover is on-demand chrome; its checkbox is not in the DOM
    // until it is open (see `openFilterBar`'s own doc).
    await openFilterBar(page);
    await page.locator(".filter-include-archived").check();
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsListSettled(page);

    await openRowMenu(target);
    const menu = target.getByRole("menu");
    await expect(menu.getByRole("menuitem")).toHaveText(["rename", "clone", "replace", "delete"]);

    const toggle = target.locator(".session-row-menu");
    const rename = target.locator(".session-row-rename");
    const clone = target.locator(".session-row-clone");
    const replace = target.locator(".session-row-replace");
    const remove = target.locator(".session-row-delete");
    await toggle.focus();
    await page.keyboard.press("ArrowDown");
    await expect(rename).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(clone).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(replace).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(remove).toBeFocused();
    // Wraps on FOUR. A list that still believed it had six would leave
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
  await waitForHostsListSettled(page);
  await openRowMenu(target);
  await expect(target.getByRole("menuitem")).toHaveCount(6);

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
  await expect(target.getByRole("menuitem")).toHaveText(["rename", "clone", "replace", "delete"]);

  // Delete is at position 3 now, not 5, and the node that survived the
  // change answers to it — clone and replace, both offered
  // unconditionally, keep positions 1 and 2 in both retention states and
  // do not need to be re-found here.
  const toggle = target.locator(".session-row-menu");
  await toggle.focus();
  await page.keyboard.press("ArrowDown");
  await expect(target.locator(".session-row-rename")).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(target.locator(".session-row-clone")).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(target.locator(".session-row-replace")).toBeFocused();
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
    // Fixed item positions, not the seen-state feature — see
    // `hideSeenState`'s own doc.
    await hideSeenState(page);
    await pinAutoSelect(page, session.id);
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsListSettled(page);
    // The pinned session auto-attaches, and replay hands focus to its terminal
    // when the reveal lands — which can be after the row is already
    // visible. Every step below is a focus move followed by a keystroke,
    // so a replay reveal landing mid-sequence steals the focus the keystroke was
    // aimed at (ArrowDown reaching the terminal instead of the toggle; the
    // post-Escape refocus overridden — both observed on loaded CI runs).
    // Waiting for that requested session to reveal makes this test's own
    // moves the last ones after the initial focus handoff.
    await waitForSessionRevealed(page, session.id);

    const toggle = target.locator(".session-row-menu");
    const rename = target.locator(".session-row-rename");
    const clone = target.locator(".session-row-clone");
    const stop = target.locator(".session-row-stop");
    const remove = target.locator(".session-row-delete");

    // ArrowDown on a CLOSED toggle: opens and lands on the first command.
    //
    // The reveal wait above only rules out the FIRST steal. This
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

    // One tab stop, not six: the focused item carries it, everything
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
    await expect(target.locator(".session-row-replace")).toBeFocused();
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
    await waitForHostsListSettled(page);

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
 * resize, host details or the filter popover opening, the create form, and a
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
    await waitForHostsListSettled(page);

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
    // Fixed item positions, not the seen-state feature — see
    // `hideSeenState`'s own doc.
    await hideSeenState(page);
    await pinAutoSelect(page, session.id);
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsListSettled(page);
    // Same precaution, for the same reason, as "opening the actions menu
    // enters it" above: this test asserts where focus IS at several points,
    // and the pinned session's terminal takes focus for itself when its replay
    // reveal lands — which can be after the row is visible. Without this wait,
    // a reveal between the open and the first focus assertion steals
    // the focus the panel just placed, and the failure reads as a menu that
    // did not enter itself (the item holding the roving `tabindex` while
    // something else holds DOM focus) rather than as the race it is.
    await waitForSessionRevealed(page, session.id);

    const toggle = target.locator(".session-row-menu");
    const rename = target.locator(".session-row-rename");
    const clone = target.locator(".session-row-clone");
    const replace = target.locator(".session-row-replace");
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
    await expect(replace).toBeFocused();
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
    await waitForHostsListSettled(page);

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
    await waitForHostsListSettled(page);

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
    // Fixed six-item geometry, not the seen-state feature — see
    // `hideSeenState`'s own doc.
    await hideSeenState(page);
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsListSettled(page);
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
    expect(items).toHaveLength(6);
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
    await waitForHostsListSettled(page);

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
 * The sidebar's resting chrome keeps one host list, the compact session
 * heading, session rows, and create control visible. Host details and the
 * filter popover remain closed until requested.
 *
 * This pins the one-list contract directly: a regression that hides the
 * list, restores the old duplicate strip, or makes host actions hover-only
 * fails at the resting state where those choices matter.
 */
test("the host list is permanent while details and filtering stay on demand", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.locator(".hosts-panel")).toBeVisible();
  await expect(page.locator(".hosts-toggle")).toHaveCount(0);
  await expect(page.locator(".sidebar-controls")).toHaveCount(0);
  const hostHeading = page.locator(".hosts-heading");
  const details = hostHeading.getByRole("checkbox", { name: "details", exact: true });
  await expect(details).toBeVisible();
  await expect(page.locator(".filter-toggle")).toBeVisible();
  // Count, compact, and new share one heading; filter and sort share the
  // following control row. DOM order matters because each count explains
  // the rows before the controls that can change them.
  const header = page.locator(".list-header");
  await expect(header.locator(":scope > .session-heading .session-count")).toHaveCount(1);
  await expect(header.locator(":scope > .session-heading .compact-toggle")).toHaveCount(1);
  await expect(header.locator(":scope > .session-heading .new-session-button")).toHaveCount(1);
  await expect(header.locator(":scope > .list-header-controls")).toHaveCount(1);
  await expect(header.locator(":scope > .list-header-controls .filter-toggle")).toHaveCount(1);
  await expect(header.locator(":scope > .list-header-controls .sort-select")).toHaveCount(1);
  await expect(details).not.toBeChecked();
  await expect(page.locator(".filter-toggle")).toHaveAttribute("aria-expanded", "false");
  await expect(page.locator(".filter-popover")).toHaveCount(0);
  const rows = page.locator(".host-row");
  await expect(rows.first()).toBeVisible({ timeout: 20_000 });
  const count = await rows.count();
  await expect(page.locator(".host-count")).toHaveText(count === 1 ? "1 host" : `${count} hosts`);
  // Being descendants of one heading is insufficient: wrapping controls
  // onto a second line would restore the vertical space this layout removes.
  const hostControls = [
    hostHeading.locator(".host-count"),
    hostHeading.locator(".host-details-control"),
    hostHeading.getByRole("button", { name: "add host", exact: true }),
  ];
  const hostBoxes = await Promise.all(hostControls.map((control) => control.boundingBox()));
  expect(hostBoxes.every((box) => box !== null)).toBe(true);
  expect(Math.max(...hostBoxes.map((box) => box!.y))).toBeLessThan(
    Math.min(...hostBoxes.map((box) => box!.y + box!.height)),
  );
  const local = rows.first();
  await expect(local.locator(".host-status .status-dot")).toBeVisible();
  await expect(local.locator(".host-status-label")).toHaveCount(0);
  await expect(local.locator(".host-row-menu")).toHaveCSS("opacity", "1");

  // Space toggles the focusable checkbox itself and still applies the global
  // disclosure to every host row.
  await details.focus();
  await page.keyboard.press("Space");
  await expect(details).toBeChecked();
  for (let index = 0; index < count; index += 1) {
    await expect(rows.nth(index).locator(".host-detail")).toBeVisible();
  }
  await page.keyboard.press("Space");
  await expect(details).not.toBeChecked();

  await openFilterBar(page);
  await page.locator(".filter-toggle").click();
  await expect(page.locator(".filter-popover")).toHaveCount(0);
});

/**
 * Both compact choices apply immediately, persist through reload, and seed
 * another client from the helm. Turning it off must persist false rather than
 * merely clear a local checkbox. Changing row heights also dismisses a menu whose
 * measured anchor would otherwise belong to the old layout. The already-open-client broadcast deliberately stays
 * out of scope; opening the second page is the next preference seed.
 */
test("compact hides the second line and persists across client seeds", async ({
  page,
  request,
  context,
}) => {
  await patchPreferences(request, { compact: null });
  const session = await createSession(request, {
    title: `compact-persistence-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  let second: Page | undefined;
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    const expandedHeight = (await target.boundingBox())!.height;
    await expect(target.locator(".session-row-meta")).toBeVisible();
    const compact = page.getByRole("checkbox", { name: "compact", exact: true });
    await expect(compact).not.toBeChecked();
    await waitForHostsListSettled(page);
    await openRowMenu(target);

    await compact.check();
    await expect(compact).toBeChecked();
    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(target.locator(".session-row-meta")).toHaveCount(0);
    await expect(target.locator(".session-row-open")).toHaveAttribute("title", session.cwd);
    expect((await target.boundingBox())!.height).toBeLessThan(expandedHeight);
    await expect.poll(async () => (await readPreferences(request)).compact).toBe(true);

    await page.reload();
    await expect(page.locator(".compact-toggle input")).toBeChecked();
    await expect(row(page, session.id).locator(".session-row-meta")).toHaveCount(0);

    second = await context.newPage();
    await second.goto("/");
    await expect(second.locator(".compact-toggle input")).toBeChecked();
    await expect(row(second, session.id).locator(".session-row-meta")).toHaveCount(0);

    await second.locator(".compact-toggle input").uncheck();
    await expect(row(second, session.id).locator(".session-row-meta")).toBeVisible();
    await expect.poll(async () => (await readPreferences(request)).compact).toBe(false);
    await page.reload();
    await expect(page.locator(".compact-toggle input")).not.toBeChecked();
    await expect(row(page, session.id).locator(".session-row-meta")).toBeVisible();
    expect((await row(page, session.id).boundingBox())!.height).toBeGreaterThan(expandedHeight - 1);
  } finally {
    if (second) await second.close();
    await patchPreferences(request, { compact: null });
    await cleanupSession(request, session.id);
  }
});

/**
 * One header disclosure reveals version evidence for every host together.
 *
 * Two connected fixture rows make the global scope observable: a per-row
 * disclosure accidentally wired only to the first row cannot satisfy both
 * version-line assertions after the single click.
 */
test("host details reveal every row's version line together", async ({ page, request }) => {
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
              id: 31,
              kind: "local",
              destination: null,
              name: "this machine",
              identity: "detail-local",
              remote_farhelm: null,
              remote_state_dir: null,
              state: {
                phase: "connected",
                identity: "detail-local",
                build_version: "1.2.3-local",
                refresh: { status: "ok", sessions: 1 },
              },
            },
            {
              id: 32,
              kind: "ssh",
              destination: "user@detail-remote",
              name: "user@detail-remote",
              identity: "detail-remote",
              remote_farhelm: null,
              remote_state_dir: null,
              state: {
                phase: "connected",
                identity: "detail-remote",
                build_version: "4.5.6-remote",
                refresh: { status: "ok", sessions: 2 },
              },
            },
          ],
        },
      });
    },
  );

  await page.goto("/");
  const rows = page.locator(".host-row");
  await expect(rows).toHaveCount(2, { timeout: 20_000 });
  await expect(rows.locator(".host-detail")).toHaveCount(0);
  await page.locator(".host-details-toggle").click();
  await expect(rows.nth(0).locator(".host-detail")).toContainText("1.2.3-local");
  await expect(rows.nth(1).locator(".host-detail")).toContainText("4.5.6-remote");
});

/**
 * An applied filter remains visible in the count while its popover is closed.
 */
test("a closed filter popover still announces an applied filter", async ({ page }) => {
  // No fixture is needed: a unique query proves that the committed count,
  // rather than a separate status note, remains the closed-filter signal.
  const needle = `no-such-title-${Date.now()}`;
  await page.goto("/");
  await openFilterBar(page);
  await page.locator(".filter-title").fill(needle);
  await expect(page.locator(".session-count")).toContainText("0 matching", {
    timeout: 20_000,
  });
  // Closing the popover keeps the committed filter in force.
  await page.locator(".filter-toggle").click();
  await expect(page.locator(".filter-popover")).toHaveCount(0);
  await expect(page.locator(".session-count")).toContainText("0 matching");
  // Reopening and clearing restores the ordinary count wording.
  await openFilterBar(page);
  await page.locator(".filter-clear").click();
  await expect(page.locator(".session-count")).toHaveText(/^\d+ sessions$/, { timeout: 20_000 });
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
 * The permanent list is per-host: several hosts render several rows, each
 * named with its own status, and a long unbroken host name ellipsizes instead
 * of widening the sidebar.
 *
 * A stubbed two-host registry in mixed phases makes the row count and status
 * mapping falsifiable. The 200-character name exercises the same width
 * pressure that once pushed host actions out of the sidebar.
 */
test("the host list counts every host, humanizes phases, and clips long names", async ({
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

  const entries = page.locator(".host-row");
  await expect(entries).toHaveCount(2, { timeout: 20_000 });
  await expect(page.locator(".host-count")).toHaveText("2 hosts");
  await expect(entries.nth(0).locator(".host-name")).toHaveText("this machine");
  await expect(entries.nth(0).locator(".host-status-label")).toHaveCount(0);
  await expect(entries.nth(1).locator(".host-status-label")).toHaveText(
    "unreachable, retrying",
  );
  await expect(entries.nth(1)).toHaveAttribute("data-host-phase", "unreachable-reprobing");
  // The long name is clipped by the row, not allowed to widen it: the
  // element paints less than it holds, and its box stays inside the
  // sidebar.
  const name = entries.nth(1).locator(".host-name");
  expect(await name.evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
  const sidebarBox = (await page.locator(".app-sidebar").boundingBox())!;
  const nameBox = (await name.boundingBox())!;
  expect(nameBox.x + nameBox.width).toBeLessThanOrEqual(sidebarBox.x + sidebarBox.width + 1);
});

/**
 * A LOCAL session's locality mark is provisional until the registry answers:
 * the host name remains truthful on the second line throughout, while the
 * first-line slot changes from blank/unknown to the local glyph once the
 * registry confirms the host id.
 *
 * `shared::session_locality` answers `Unknown` for every case with no
 * evidence either way (including "the hosts read has not landed yet"), and
 * `Unknown` never suppresses an available host label (SPEC_impl.md), and the
 * settled two-line layout keeps that label after classification too. The
 * route hold makes the unknown window deterministic
 * instead of racing a fast helm: without it, `/api/hosts` ordinarily
 * answers before the first paint and the provisional state is never
 * actually observed.
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
    // Unconfirmed: the row names the host it already has, but draws no
    // locality glyph at all — asserting the local glyph here would be
    // exactly the invented claim `session_locality`'s `Unknown` case
    // refuses to make.
    await expect(target).toHaveAttribute("data-host-locality", "unknown");
    await expect(target.locator(".host-kind-icon")).toHaveCount(0);

    releaseHosts();
    await expect(target.locator(".session-host")).toContainText("this machine");
    // Confirmed: the name stays on the second line and the locality slot
    // gains the local glyph.
    await expect(target).toHaveAttribute("data-host-locality", "local");
    await expect(target.locator(".host-kind-icon")).toHaveCount(1);
    await expect(target.locator(".host-kind-icon")).toHaveAttribute("data-glyph", "local");
    await expect(target.locator(".host-kind-icon + .visually-hidden")).toHaveText("local");
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * Hostile title and host lengths stay contained at 280px while simultaneous
 * stale/archive qualifiers remain on the identity line. Two opposing content
 * shapes prove the second-line host floor and cap without coupling them to
 * the first-line title, agent, or activity columns.
 *
 * The fixture is route-controlled because stale plus archived plus a live
 * status is useful layout pressure but not a lifecycle state the harness can
 * produce on demand. The peer-controlled host also carries a bidi override,
 * preserving the escaping and tooltip assertion from the earlier layout.
 */
test("hostile identity and host text stay contained with simultaneous qualifiers", async ({
  page,
  request,
}) => {
  const stamp = (await request.get("/api/sessions")).headers()["x-farhelm-build"] ?? "";
  expect(stamp, "the helm must stamp its replies").toBeTruthy();
  // Mirrors the host's minimum width in app.css. The title gets a nonzero
  // space check below; this pixel constant does not specify its glyph floor.
  const HOST_FLOOR_PX = 40;
  // Sub-pixel/cross-engine rounding tolerance, not a meaningful slack.
  const TOL = 2;
  const rlo = String.fromCharCode(0x202e);
  const activityAgeSecs = Math.floor(Date.now() / 1000) - 5 * 3600;

  /** Route `/api/sessions` to one fabricated row and load it fresh. */
  async function loadFixture(overrides: Record<string, unknown>) {
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
              id: "width-contention-session",
              cwd: "/tmp",
              invocation: "sleep 300",
              stale: true,
              archived: true,
              status: { state: "running" },
              last_activity_at: activityAgeSecs,
              // Not a real registered host — this fixture only needs an id
              // that disagrees with the local row's, which any sufficiently
              // large one does, so `session_locality` reads it as `Remote`
              // without a second route mocking `/api/hosts`.
              host: 999_999,
              ...overrides,
            },
          ],
          total: 1,
          matching: 1,
          truncated: false,
        },
      });
    });
    await page.goto("/");
    await page.locator(".app-sidebar").evaluate((element) => {
      (element as HTMLElement).style.width = "280px";
    });
    const target = row(page, "width-contention-session");
    await expect(target).toBeVisible({ timeout: 20_000 });
    await expect(target).toHaveAttribute("data-host-locality", "remote");
    await expect(target.locator(".status-time"), "the age must actually render").toBeVisible();
    return target;
  }

  /** The boxes this test measures on every row, gathered in one place. */
  async function measure(target: ReturnType<typeof row>) {
    const sideBox = (await page.locator(".app-sidebar").boundingBox())!;
    const rowBox = (await target.boundingBox())!;
    expect(rowBox.width, "the row must not force the sidebar wider").toBeLessThanOrEqual(
      sideBox.width + 1,
    );
    const lineBox = (await target.locator(".session-row-meta").boundingBox())!;
    // The host's 40% cap resolves against the line's CONTENT box, and the
    // line now carries a left indent (the two leading tracks it skips to
    // sit under the title), so the border box `lineBox` measures would
    // let a cap regression of several points slip through the assertions
    // below.
    const lineContentWidth = await target.locator(".session-row-meta").evaluate((el) => {
      const cs = getComputedStyle(el);
      return el.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
    });
    return {
      sideBox,
      lineBox,
      lineContentWidth,
      title: (await target.locator(".session-title").boundingBox())!,
      identity: (await target.locator(".session-identity-copy").boundingBox())!,
      host: (await target.locator(".session-host").boundingBox())!,
      stale: (await target.locator(".stale-badge").boundingBox())!,
      archived: (await target.locator(".archived-badge").boundingBox())!,
      age: (await target.locator(".status-time").boundingBox())!,
      statusSlot: (await target.locator(".session-status-slot").boundingBox())!,
      localitySlot: (await target.locator(".session-locality-slot").boundingBox())!,
    };
  }

  // ===== Title-dominant: the title's content vastly exceeds the line;
  // the host name is short. Proves the host still shows something real —
  // not squeezed to nothing — while the title is being maximally greedy.
  const titleDominant = await loadFixture({
    title: `width-contention-${"t".repeat(150)}`,
    host_name: "user@short-vm",
  });
  const dominant = await measure(titleDominant);
  expect(dominant.title.width, "the title must retain visible space").toBeGreaterThan(0);
  expect(dominant.host.width, "the host must keep its floor even here").toBeGreaterThanOrEqual(
    HOST_FLOOR_PX - TOL,
  );
  expect(
    dominant.host.width,
    "the host must still respect its 40% cap",
  ).toBeLessThanOrEqual(dominant.lineContentWidth * 0.4 + TOL);
  for (const qualifier of [dominant.stale, dominant.archived]) {
    expect(qualifier.x).toBeGreaterThanOrEqual(dominant.identity.x - TOL);
    expect(qualifier.x + qualifier.width).toBeLessThanOrEqual(dominant.identity.x + dominant.identity.width + TOL);
    expect(qualifier.y + qualifier.height).toBeLessThanOrEqual(dominant.identity.y + dominant.identity.height + TOL);
  }

  // ===== Host-dominant: BOTH title and host are individually long (unlike
  // title-dominant, where only the title was) — the opposite construction,
  // proving the same floors hold when the host is ALSO an aggressor rather
  // than a passive short string. This is also F11's "long-host case": the
  // raw name embeds a directional override, so the same fixture proves the
  // `title` tooltip carries the escaped value `display_peer` produces, not
  // the raw peer-controlled string.
  const rawLongHost = `deploy@${rlo}${"build-fleet-".repeat(8)}internal.example.com`;
  const escapedLongHost = `deploy@<U+202E>${"build-fleet-".repeat(8)}internal.example.com`;
  const hostDominant = await loadFixture({
    title: `width-contention-${rlo}${"t".repeat(150)}`,
    host_name: rawLongHost,
  });
  // The new native title tooltip is a separate display surface: directional
  // controls must be legible there instead of rearranging the tooltip text.
  await expect(hostDominant.locator(".session-title")).toHaveAttribute(
    "title",
    `width-contention-<U+202E>${"t".repeat(150)}`,
  );
  const underdog = await measure(hostDominant);
  expect(underdog.title.width, "the title must retain visible space").toBeGreaterThan(0);
  expect(
    underdog.host.width,
    "the host must keep its floor even against a long title",
  ).toBeGreaterThanOrEqual(HOST_FLOOR_PX - TOL);
  expect(
    underdog.host.width,
    "the host must never grow past its cap",
  ).toBeLessThanOrEqual(underdog.lineContentWidth * 0.4 + TOL);
  // F11: the tooltip carries the SAME escaped value the visible text does
  // (`display_peer`, not the raw peer-controlled string) — reusing the
  // existing bidi-fixture pattern (see "a bidi override in the invocation
  // basename renders escaped and isolated" below) for the host name.
  await expect(hostDominant.locator(".session-host")).toHaveText(escapedLongHost);
  await expect(hostDominant.locator(".session-host")).toHaveAttribute("title", escapedLongHost);

  // Every field ellipsizes inside the sidebar rather than forcing the row —
  // or the line — wider than the column, in both cases.
  for (const [label, m] of [
    ["title-dominant", dominant],
    ["host-dominant", underdog],
  ] as const) {
    for (const [name, box] of [
      ["title", m.title],
      ["host", m.host],
    ] as const) {
      expect(
        box.x + box.width,
        `[${label}] the ${name} field must ellipsize inside the sidebar, not force the row wide`,
      ).toBeLessThanOrEqual(m.sideBox.x + m.sideBox.width + 1);
    }
  }

  // Qualifiers and activity retain stable geometry when only peer-controlled
  // title and host lengths change. This also catches a qualifier escaping its
  // identity group into an automatic grid row. Wrapping inside that group
  // is deliberate when several state words cannot fit on one line.
  for (const [name, a, b] of [
    ["stale badge", dominant.stale, underdog.stale],
    ["archived badge", dominant.archived, underdog.archived],
    ["activity age", dominant.age, underdog.age],
  ] as const) {
    expect(
      Math.abs(a.width - b.width),
      `the ${name} must render at the same width regardless of identity pressure`,
    ).toBeLessThanOrEqual(TOL);
  }
  expect(Math.abs(dominant.age.x - underdog.age.x), "activity times share one column").toBeLessThanOrEqual(TOL);
  expect(Math.abs(dominant.statusSlot.x - underdog.statusSlot.x)).toBeLessThanOrEqual(TOL);
  expect(Math.abs(dominant.localitySlot.x - underdog.localitySlot.x)).toBeLessThanOrEqual(TOL);
});

/**
 * Fixed status/locality tracks, right-aligned ages, and menu reservations hold
 * at an adversarial 280px sidebar width. The fixtures deliberately mix live,
 * ended, and unknown states plus short and long agent badges. One age exceeds
 * four characters: unbounded day counts must keep the same right edge without
 * overlapping the adjacent badge. A legacy row without a host name must retain
 * its directory without inventing either a host label or a dangling colon.
 */
test("narrow rows align fixed facts and reserve only control-sized menu gutters", async ({
  page,
  request,
}) => {
  const stamp = (await request.get("/api/sessions")).headers()["x-farhelm-build"] ?? "";
  const local = await localHostId(request);
  const activity = Math.floor(Date.now() / 1000) - 120;
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
            id: "narrow-live",
            title: "live row",
            cwd: "/srv/live",
            invocation: "a",
            host: 999_991,
            host_name: "remote-build-host",
            status: { state: "running" },
            last_activity_at: activity,
          },
          {
            id: "narrow-ended",
            title: "ended row",
            cwd: "/srv/ended",
            invocation: "/opt/tools/a-very-long-agent-program-name --flag",
            host: local,
            host_name: "this machine",
            status: { state: "exited", exit_code: 17 },
            stale: true,
            archived: true,
            last_activity_at: activity - 1000 * 86_400,
          },
          {
            id: "narrow-unknown",
            title: "unknown row",
            cwd: "/srv/unknown",
            invocation: "agent",
            status: { state: "unknown" },
            last_activity_at: activity,
          },
        ],
        total: 3,
        matching: 3,
        truncated: false,
      },
    });
  });

  await page.goto("/");
  await page.locator(".app-sidebar").evaluate((element) => {
    (element as HTMLElement).style.width = "280px";
  });
  const rows = [row(page, "narrow-live"), row(page, "narrow-ended"), row(page, "narrow-unknown")];
  for (const target of rows) await expect(target).toBeVisible({ timeout: 20_000 });

  async function left(target: Locator, selector: string) {
    return (await target.locator(selector).boundingBox())!.x;
  }
  for (const selector of [".session-status-slot", ".session-locality-slot"] as const) {
    const positions = await Promise.all(rows.map((target) => left(target, selector)));
    expect(Math.max(...positions) - Math.min(...positions), `${selector} must align`).toBeLessThanOrEqual(2);
  }
  const ages = await Promise.all(rows.map(async (target) => (await target.locator(".status-time").boundingBox())!));
  const ageRights = ages.map((box) => box.x + box.width);
  expect(Math.max(...ageRights) - Math.min(...ageRights), "activity right edges must align").toBeLessThanOrEqual(2);
  await expect(rows[1].locator(".status-time")).toHaveText("1000d");
  for (let index = 0; index < rows.length; index++) {
    const badge = (await rows[index].locator(".session-agent").boundingBox())!;
    expect(badge.x + badge.width, "an age must not overlap its agent badge").toBeLessThanOrEqual(ages[index].x);
  }
  await expect(rows[0].locator(".session-status-slot .status-dot")).toHaveCount(1);
  await expect(rows[1].locator(".session-status-slot .status-badge")).toHaveCount(0);
  await expect(rows[1].locator(".session-identity-copy .status-badge")).toContainText("exited");
  await expect(rows[2].locator(".session-status-slot .status-badge")).toHaveCount(0);
  await expect(rows[2].locator(".session-locality-slot .host-kind-icon")).toHaveCount(0);
  await expect(rows[2].locator(".session-row-meta")).toBeVisible();
  await expect(rows[2].locator(".session-host")).toHaveCount(0);
  await expect(rows[2].locator(".session-host-separator")).toHaveCount(0);
  await expect(rows[2].locator(".session-cwd")).toHaveText("/srv/unknown");
  await expect(rows[1].locator(".stale-badge")).toBeVisible();
  await expect(rows[1].locator(".archived-badge")).toBeVisible();
  /** DOM visibility alone misses a badge completely clipped by its parent. */
  async function expectPaintedQualifiers() {
    const identity = (await rows[1].locator(".session-identity-copy").boundingBox())!;
    for (const selector of [".status-badge", ".stale-badge", ".archived-badge"]) {
      const badge = (await rows[1].locator(`.session-identity-copy ${selector}`).boundingBox())!;
      expect(badge.width, `${selector} retains visible text space`).toBeGreaterThan(12);
      expect(badge.x).toBeGreaterThanOrEqual(identity.x - 1);
      expect(badge.x + badge.width).toBeLessThanOrEqual(identity.x + identity.width + 1);
      expect(badge.y).toBeGreaterThanOrEqual(identity.y - 1);
      expect(badge.y + badge.height).toBeLessThanOrEqual(identity.y + identity.height + 1);
    }
  }
  await expectPaintedQualifiers();
  const liveTitle = rows[0].locator(".session-title");
  const liveTitleBox = (await liveTitle.boundingBox())!;
  const fourCharacters = await liveTitle.evaluate((node) => {
    const probe = document.createElement("span");
    probe.style.cssText = `position:fixed;width:4ch;font:${getComputedStyle(node).font}`;
    document.body.appendChild(probe);
    const width = probe.getBoundingClientRect().width;
    probe.remove();
    return width;
  });
  expect(liveTitleBox.width, "an ordinary title keeps roughly four glyphs at 280px").toBeGreaterThanOrEqual(
    fourCharacters - 2,
  );

  async function menuExcess(main: Locator, content: Locator, menu: Locator) {
    const mainBox = (await main.boundingBox())!;
    const contentBox = (await content.boundingBox())!;
    const menuBox = (await menu.boundingBox())!;
    return mainBox.x + mainBox.width - (contentBox.x + contentBox.width) - menuBox.width;
  }
  const sessionExcess = await menuExcess(
    rows[0].locator(".session-row-main"),
    rows[0].locator(".session-activity-column"),
    rows[0].locator(".session-row-menu"),
  );
  expect(sessionExcess).toBeGreaterThanOrEqual(0);
  expect(sessionExcess).toBeLessThanOrEqual(8.5);

  const host = page.locator(".host-row").first();
  await expect(host).toBeVisible({ timeout: 20_000 });
  const hostExcess = await menuExcess(
    host.locator(".host-row-main"),
    host.locator(".host-status"),
    host.locator(".host-row-menu"),
  );
  expect(hostExcess).toBeGreaterThanOrEqual(0);
  expect(hostExcess).toBeLessThanOrEqual(8.5);
});

/** Closing the app-bar popup discards its local editor draft, so reopening
 * starts at the shared catalog rather than resurrecting an abandoned form. */
test("closing the profiles popup discards its open editor", async ({ page }) => {
  await page.goto("/");
  const toggle = page.locator(".profiles-toggle");
  await toggle.click();
  await expect(page.locator(".profiles-popover")).toBeVisible({ timeout: 20_000 });
  await page.locator(".profiles-popover .new-profile-button").click();
  await expect(page.locator(".profiles-popover .profile-form")).toBeVisible();

  await toggle.click();
  await expect(page.locator(".profiles-popover")).toHaveCount(0);
  await toggle.click();
  await expect(page.locator(".profiles-popover")).toBeVisible();
  await expect(page.locator(".profiles-popover .profile-form")).toHaveCount(0);
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
    // The auto-selection is a real requested-session attachment, not just a view.
    await waitForSessionRevealed(page, older.id);

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
  timeline,
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
    await waitForSessionRevealed(page, session.id);

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
    second = await newObservedContext(browser, timeline, {
      storageState: await page.context().storageState(),
    });
    const page2 = await second.newPage();
    await page2.goto("/");
    await waitForSessionReady(page2, session.id);

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
    // The geometry under test is the panel's height at its FIXED six-item
    // count against `MENU_PANEL_MIN_RESERVE_PX` in menu_panel.rs, the
    // room the placement keeps below the panel's clamped top. A menu
    // taller than that reserve is clamped to the viewport edge and scrolls
    // inside its own panel (see the `max-height` note above
    // `.session-row-menu-panel` in app.css), which leaves the bottom-most
    // item's box past the edge this test measures — that is how the
    // reserve sized for five items was caught when replace made it six,
    // and the failure is the same at any viewport height, since the clamp
    // measures from the bottom. The reserve now holds the seventh row
    // too, but the count is still pinned: an extra "mark unread" row
    // (this fleet's fixture sessions do reach a live status under enough
    // real wall-clock time — 18 of them, outliving every other test in
    // the file) would make the measured height depend on the classifier's
    // timing rather than on the geometry under test. See `hideSeenState`'s
    // own doc.
    await hideSeenState(page);
    await page.goto("/");
    await expect(row(page, created[0].id)).toBeVisible({ timeout: 20_000 });
    // Settled BEFORE any scroll or menu open: the permanent host list's
    // loading, count, and error shape must stop changing before this test
    // measures a menu. Its premise (a row genuinely scrolled into view,
    // then a menu that stays put) must not race an initial host read.
    await waitForHostsListSettled(page);

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
    // (what `waitForHostsListSettled` above watches) only ever flips ONCE,
    // so it cannot catch a LATER hosts re-read landing with a different
    // outcome — a real SSH host's connection flapping on a busy CI runner,
    // say — flipping `hosts_list_shape`'s error component and closing the
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
    // — panel and all — when `layout_epoch`, `show_create`, or
    // `hosts_list_shape` reports movement. The relevant background cases
    // here are sidebar geometry, create-form state, and the permanent host
    // list's loading/count/error or collapsed-trace shape; an ordinary host
    // phase change is not one of them. This test's own long
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
    // Settled before the menu opens — see `waitForHostsListSettled`'s own
    // doc for why an unsettled strip can close the menu for the wrong
    // reason.
    await waitForHostsListSettled(page);

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
 * The sidebar's own on-demand controls are layout causes too: opening host
 * details, the filter popover, or the create dialog each changes content
 * section above the rows (list.rs's `use_effect` near `show_create`), which
 * is exactly the kind of internal shift `layout_epoch` does NOT cover (that
 * counter is for the ancestor-owned scroll/resize listeners in lib.rs) —
 * this component watches these three signals directly instead. One test
 * per surface, because each is a separate signal the effect subscribes to
 * and a regression could plausibly drop any one of the three independently.
 */
test("opening host details closes an open row menu", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `menu-hosts-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsListSettled(page);
    await openRowMenu(target);

    await page.locator(".host-details-toggle").click();
    await expect(page.locator(".host-details-toggle")).toBeChecked();
    await expect(page.locator(".host-detail").first()).toBeVisible();

    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(target.locator(".session-row-menu")).toHaveAttribute("aria-expanded", "false");
  } finally {
    await cleanupSession(request, session.id);
  }
});

/** Host menus use independent state from session menus, so Details must
 * dismiss this second fixed-surface path explicitly. */
test("opening host details closes an open host-row menu", async ({ page }) => {
  await page.goto("/");
  await waitForHostsListSettled(page);
  const host = page.locator(".host-row").first();
  await openHostMenu(host);

  await page.locator(".host-details-toggle").click();
  await expect(page.locator(".host-details-toggle")).toBeChecked();
  await expect(host.locator(".host-row-menu-panel")).toHaveCount(0);
  await expect(host.locator(".host-row-menu")).toHaveAttribute("aria-expanded", "false");
});

/** The filter is anchored below the host list, so changing every host row's
 * detail height must invalidate that fixed measurement too. */
test("opening host details closes the filter popover", async ({ page }) => {
  await page.goto("/");
  await waitForHostsListSettled(page);
  await openFilterBar(page);

  await page.locator(".host-details-toggle").click();
  await expect(page.locator(".host-details-toggle")).toBeChecked();
  await expect(page.locator(".filter-popover")).toHaveCount(0);
  await expect(page.locator(".filter-toggle")).toHaveAttribute("aria-expanded", "false");
});

/** Mounting and unmounting Add host move the filter's session-header anchor.
 * Both directions must discard the old fixed coordinates. */
test("toggling the add-host form closes the filter popover", async ({ page }) => {
  await page.goto("/");
  await waitForHostsListSettled(page);

  await openFilterBar(page);
  await page.locator(".add-host-button").click();
  await expect(page.locator(".add-host-form")).toBeVisible();
  await expect(page.locator(".filter-popover")).toHaveCount(0);
  await expect(page.locator(".filter-toggle")).toHaveAttribute("aria-expanded", "false");

  await openFilterBar(page);
  await page.locator(".add-host-button").click();
  await expect(page.locator(".add-host-form")).toHaveCount(0);
  await expect(page.locator(".filter-popover")).toHaveCount(0);
  await expect(page.locator(".filter-toggle")).toHaveAttribute("aria-expanded", "false");
});

/** Opening a host row menu is the reverse half of filter/menu mutual
 * exclusion; only the newly requested surface may remain. */
test("opening a host-row menu closes the filter popover", async ({ page }) => {
  await page.goto("/");
  await waitForHostsListSettled(page);
  await openFilterBar(page);

  const host = page.locator(".host-row").first();
  await openHostMenu(host);
  await expect(host.locator(".host-row-menu-panel")).toBeVisible();
  await expect(page.locator(".filter-popover")).toHaveCount(0);
  await expect(page.locator(".filter-toggle")).toHaveAttribute("aria-expanded", "false");
});

/** Session menus enter the same mutual-exclusion path through a separate
 * signal, so they need their own browser boundary. */
test("opening a session-row menu closes the filter popover", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `filter-session-menu-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible();
    await waitForHostsListSettled(page);
    await openFilterBar(page);

    await openRowMenu(target);
    await expect(target.locator(".session-row-menu-panel")).toBeVisible();
    await expect(page.locator(".filter-popover")).toHaveCount(0);
    await expect(page.locator(".filter-toggle")).toHaveAttribute("aria-expanded", "false");
  } finally {
    await cleanupSession(request, session.id);
  }
});

/** See "opening host details closes an open row menu" — same contract,
 * the filter popover's own toggle. */
test("opening the filter popover closes an open row menu", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `menu-filter-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await waitForHostsListSettled(page);
    await openRowMenu(target);

    await page.locator(".filter-toggle").click();
    await expect(page.locator(".filter-popover")).toBeVisible();

    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(target.locator(".session-row-menu")).toHaveAttribute("aria-expanded", "false");
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * A fixed filter surface is only attached to the rect measured on open.
 * Scrolling the sidebar moves that toggle, so the surface must dismiss rather
 * than remain at stale viewport coordinates over unrelated content.
 */
test("scrolling the sidebar dismisses the filter popover after its toggle moves", async ({
  page,
  request,
}) => {
  const session = await createSession(request, {
    title: `filter-scroll-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    // The scroll is the stimulus, not the number or density of session rows.
    // Reserve overflow explicitly so a denser layout or fixture cleanup
    // cannot turn this into a test that never scrolls its real container.
    await page.setViewportSize({ width: 1280, height: 360 });
    await page.goto("/");
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });
    const sidebar = page.locator(".app-sidebar");
    const viewportHeight = await sidebar.evaluate((el) => el.clientHeight);
    await page.locator(".session-list").evaluate((el, height) => {
      (el as HTMLElement).style.minHeight = `${height * 2}px`;
    }, viewportHeight);
    await expect.poll(() => sidebar.evaluate((el) => el.scrollHeight > el.clientHeight + 1)).toBe(true);
    await page.locator(".filter-toggle").click();
    const before = await page.locator(".filter-toggle").boundingBox();
    await sidebar.evaluate((el) => { el.scrollTop = el.scrollHeight; });
    await expect.poll(async () => (await page.locator(".filter-toggle").boundingBox())?.y).not.toBe(before?.y);
    await expect(page.locator(".filter-popover")).toHaveCount(0);
    await expect(page.locator(".filter-toggle")).toHaveAttribute("aria-expanded", "false");
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * The list header is rendered in every listing state, not only after a
 * successful read: filter and sort are the only way to change or clear a
 * filter, and a slow or failing read is exactly when someone may need to.
 * Holds the first listing request open and proves the controls are usable
 * meanwhile.
 */
test("the session header controls are usable while the first listing read is pending", async ({
  page,
}) => {
  let release: () => void = () => {};
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  let first = true;
  await page.route(
    (url) => url.pathname === "/api/sessions",
    async (route) => {
      if (first && route.request().method() === "GET") {
        first = false;
        await held;
      }
      await route.continue();
    },
  );
  try {
    await page.goto("/");
    await expect(page.locator(".list-header-controls .filter-toggle")).toBeVisible({ timeout: 20_000 });
    await expect(page.locator(".list-header-controls .sort-select")).toBeVisible();
    await expect(page.locator(".session-count")).toHaveCount(0);
    await page.locator(".filter-toggle").click();
    await expect(page.locator(".filter-popover")).toBeVisible();
  } finally {
    release();
    await page.unroute((url) => url.pathname === "/api/sessions");
  }
});

/** See "opening host details closes an open row menu" — same contract,
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
    await waitForHostsListSettled(page);
    await openRowMenu(target);

    await page.locator(".new-session-button").click();
    await expect(page.locator(".create-session-form")).toBeVisible();

    // Moving the opener into the count heading must leave the draft after
    // it in keyboard order. Otherwise forward Tab skips the newly opened
    // form entirely, even though pointer creation still works.
    await page.locator(".new-session-button").focus();
    await page.keyboard.press("Tab");
    await expect(page.locator(".create-session-form :focus")).toHaveCount(1);

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
  await waitForHostsListSettled(page);
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
    await waitForHostsListSettled(page);
    await openRowMenu(target);

    const before = page.viewportSize()!;
    await page.setViewportSize({ width: before.width - 200, height: before.height - 100 });

    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(target.locator(".session-row-menu")).toHaveAttribute("aria-expanded", "false");
  } finally {
    await cleanupSession(request, session.id);
  }
});

// =====================================================================
// Host aliases: setting a shorter label from the row menu and watching it
// propagate to every surface that names a host — the host panel row, the
// session row's host slot, and the create form's host selector — while the
// details view keeps the real destination visible underneath (SPEC.md's
// Topology paragraph). `agent-relay.spec.ts` covers the CLI-facing half
// (resolving a create by alias, and the raw destination refusing once
// aliased) against the suite's REAL second host; the two tests below are
// display-propagation only and need no live ssh connection to prove, so
// both the remote host and its session are fabricated through route
// interception instead — see each test's own doc for why that is the
// right call here specifically.
// =====================================================================

/**
 * Setting a host's alias from the row menu propagates to every CLIENT
 * surface that reads it — the panel row, the create form's host selector,
 * and the session row's host slot — with the EXACT text, never leaving the
 * raw destination visible beside the alias; clearing restores the
 * destination in the same three places. Both directions are checked with
 * exact-text matches throughout, not merely `toContain`-shaped ones.
 *
 * Deliberately NARROWED to client rendering and refetch wiring, not
 * end-to-end alias propagation: the mocked `/api/sessions` route below
 * computes `host_name` from this test's OWN `alias` variable, so this test
 * would still pass even if the real helm never derived a session's
 * `host_name` from its host's alias at all. That independent derivation is
 * what `sessions_tests.rs`'s `get_session_carries_the_hosts_alias` and
 * `hosts.rs`'s `session_rows_carry_the_alias_in_host_name` pin at the Rust
 * level, and what `agent-relay.spec.ts`'s alias cases exercise end to end
 * against the suite's real second host. What only a browser can prove —
 * and what this test is actually for — is that the UI, once told (through
 * these two routes) that a host's alias changed, redraws every one of
 * these three surfaces correctly and re-fetches them on the right trigger
 * (the mutation's own refetch for the first two, a reload standing in for
 * the session list's own poll for the third).
 *
 * The remote host and its session are entirely FABRICATED via route
 * interception rather than driven against a real second supervisor, for
 * the reason above and to avoid touching the suite's shared
 * ssh-to-localhost fixture host that `agent-relay.spec.ts` and
 * `terminal-multihost.spec.ts` already coordinate lifecycle for — aliasing
 * that real, cross-file fixture from a third place is a bigger risk than a
 * rendering test calls for. The alias mutation itself still goes through
 * the real client code path (the row menu, the shared inline editor,
 * `api::set_alias`'s real POST) — only the SERVER side is a stand-in,
 * following the same `page.route("**\/api/hosts", ...)` fetch-and-splice
 * pattern `terminal-multihost.spec.ts` already uses for host-row fixtures
 * that do not need a live connection.
 */
test("aliasing a remote host renames it everywhere but the details view", async ({
  page,
  request,
}) => {
  const stamp = (await request.get("/api/sessions")).headers()["x-farhelm-build"] ?? "";
  expect(stamp, "the helm must stamp its replies").toBeTruthy();
  const hostId = 84001;
  const destination = "user@aliasable-mock";
  // Mutated by the fabricated alias route below and read by every other
  // fabricated route, so every fixture agrees on the host's CURRENT display
  // name across the several requests one test drives.
  let alias: string | null = null;
  const displayName = () => alias ?? destination;
  const hostFixture = () => ({
    id: hostId,
    kind: "ssh",
    destination,
    alias,
    name: displayName(),
    identity: "identity-aliasable-mock",
    remote_farhelm: null,
    remote_state_dir: null,
    state: {
      phase: "connected",
      identity: "identity-aliasable-mock",
      build_version: "0.1.0",
      refresh: { status: "ok", sessions: 1 },
    },
  });

  await page.route("**/api/hosts", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    body.hosts = [...body.hosts.filter((host: any) => host.kind === "local"), hostFixture()];
    await route.fulfill({ response, json: body });
  });
  await page.route(`**/api/hosts/${hostId}/alias`, async (route) => {
    const submitted = route.request().postDataJSON() as { alias: string | null };
    alias = submitted.alias;
    await route.fulfill({
      status: 200,
      headers: { "x-farhelm-build": stamp, "content-type": "application/json" },
      json: { ...hostFixture(), incarnation: 1 },
    });
  });
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
            id: "aliasable-mock-session",
            title: "mock-remote-session",
            cwd: "/srv/work",
            invocation: "sleep 300",
            host: hostId,
            host_name: displayName(),
          },
        ],
        total: 1,
        matching: 1,
        truncated: false,
      },
    });
  });

  await page.goto("/");
  await openHostsPanel(page);
  // By `data-host-id`, not by the `.host-name` text this test is about to
  // change — a locator filtered on the CURRENT name would stop matching
  // its own row the moment the alias takes effect.
  const hostRow = page.locator(`[data-host-id="${hostId}"]`);
  await expect(hostRow).toBeVisible({ timeout: 20_000 });
  await expect(hostRow.locator(".host-name")).toHaveText(destination);

  await openHostMenu(hostRow);
  await hostRow.locator(".host-alias").click();
  const input = hostRow.locator(".host-destination-input");
  await expect(input).toBeVisible();
  await expect(input).toHaveValue("");
  await input.fill("Build Box");
  await hostRow.locator(".host-save-destination").click();

  // The panel row picks up the rename immediately — `HostsPanel`'s own doc
  // calls this "on_changed", an explicit refetch a mutation asks for rather
  // than waiting out the poll. The create form's selector shares the SAME
  // `hosts` signal, so no separate wait is needed for it to agree.
  await expect(hostRow.locator(".host-name")).toHaveText("Build Box");
  await page.locator(".new-session-button").click();
  await expect(page.locator(".create-session-form")).toBeVisible();
  // Anchored, not a bare substring match: an option that showed "Build Box
  // (user@aliasable-mock)" — the alias with the raw destination still
  // exposed beside it — would satisfy `hasText: "Build Box"` but is exactly
  // the leak this assertion exists to catch.
  await expect(
    page.locator(".create-session-host option", { hasText: /^Build Box$/ }),
  ).toHaveCount(1);

  // The session row's host slot rides a SEPARATE signal (`/api/sessions`,
  // not `/api/hosts`) that the alias mutation's own refetch does not touch
  // — a reload is what proves it settled, rather than guessing this
  // suite's background poll cadence. Expect a harmless
  // "no such session: aliasable-mock-session" line in the helm's log after
  // this: auto-select tries to attach the fabricated row's terminal, which
  // the real backend correctly refuses since the session does not exist
  // there — the row's listed fields are what this test is about, not its
  // terminal.
  await page.reload();
  const sessionRow = row(page, "aliasable-mock-session");
  await expect(sessionRow).toBeVisible({ timeout: 20_000 });
  await expect(sessionRow.locator(".session-host")).toHaveText("Build Box");

  // The details view is the one place an alias never hides the real
  // destination.
  await openHostsPanel(page);
  await expect(hostRow.locator(".host-destination-detail")).toHaveText(
    `destination: ${destination}`,
  );

  // Clearing restores the destination everywhere the alias had replaced it
  // — re-checking all three surfaces again, not only the panel row: a fix
  // that repopulated the panel correctly but left the session list or the
  // create form's selector on the stale alias would otherwise pass.
  await openHostMenu(hostRow);
  await hostRow.locator(".host-alias").click();
  await expect(input).toHaveValue("Build Box");
  await input.fill("");
  await hostRow.locator(".host-save-destination").click();
  await expect(hostRow.locator(".host-name")).toHaveText(destination);
  await expect(hostRow.locator(".host-destination-detail")).toHaveCount(0);

  await page.locator(".new-session-button").click();
  await expect(page.locator(".create-session-form")).toBeVisible();
  await expect(
    page.locator(".create-session-host option", { hasText: new RegExp(`^${destination}$`) }),
  ).toHaveCount(1);

  await page.reload();
  await expect(sessionRow).toBeVisible({ timeout: 20_000 });
  await expect(sessionRow.locator(".session-host")).toHaveText(destination);
});

/**
 * The local row accepts an alias too (SPEC.md's Topology paragraph: "the
 * local host can carry one too") and the panel shows it. Narrower than the
 * remote case above on purpose: that test already owns the wider
 * surface-propagation assertions (session row, create-form selector,
 * details-view destination line, clearing), and repeating all of them here
 * for the local row would prove the same client-side wiring a second time
 * rather than anything specific to the local row's own exception.
 *
 * Fabricated the same way as the remote case, for the same reason: this is
 * a display check, not a connectivity one, and mocking means no real state
 * on the actual local row (a fixture every OTHER spec file in this suite
 * also reads) is ever touched.
 */
test("aliasing the local host shows it in the host panel", async ({ page, request }) => {
  const stamp = (await request.get("/api/sessions")).headers()["x-farhelm-build"] ?? "";
  expect(stamp, "the helm must stamp its replies").toBeTruthy();
  const localId = await localHostId(request);
  let alias: string | null = null;

  await page.route("**/api/hosts", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    body.hosts = body.hosts.map((host: any) =>
      host.id === localId ? { ...host, alias, name: alias ?? "this machine" } : host,
    );
    await route.fulfill({ response, json: body });
  });
  await page.route(`**/api/hosts/${localId}/alias`, async (route) => {
    const submitted = route.request().postDataJSON() as { alias: string | null };
    alias = submitted.alias;
    await route.fulfill({
      status: 200,
      headers: { "x-farhelm-build": stamp, "content-type": "application/json" },
      json: {
        id: localId,
        kind: "local",
        destination: null,
        alias,
        name: alias ?? "this machine",
        identity: null,
        remote_farhelm: null,
        remote_state_dir: null,
        state: { phase: "connected", identity: null, build_version: "0.1.0", refresh: { status: "ok", sessions: 0 } },
        incarnation: 1,
      },
    });
  });

  await page.goto("/");
  await openHostsPanel(page);
  // By `data-host-id`, for the same reason the remote test above uses it:
  // stable across the very rename this test performs.
  const hostRow = page.locator(`[data-host-id="${localId}"]`);
  await expect(hostRow).toBeVisible({ timeout: 20_000 });
  await expect(hostRow.locator(".host-name")).toHaveText("this machine");

  await openHostMenu(hostRow);
  await hostRow.locator(".host-alias").click();
  const input = hostRow.locator(".host-destination-input");
  await expect(input).toBeVisible();
  await input.fill("My Laptop");
  await hostRow.locator(".host-save-destination").click();

  await expect(hostRow.locator(".host-name")).toHaveText("My Laptop");
  // The local row never shows a destination line, aliased or not — there is
  // no real destination to reveal.
  await expect(hostRow.locator(".host-destination-detail")).toHaveCount(0);
});

/**
 * A session nobody has opened reads unseen once it goes idle, and opening
 * it clears that within one feed round trip (SPEC.md, Status).
 *
 * A real `fake-agent basic` session, never a synthetic listing: this is the
 * one property no mock can stand in for — that a session nobody has ever
 * selected genuinely reads unseen the moment the real supervisor's sampler
 * settles it into idle, with no client having done anything to it at all.
 * `basic` "prints, echoes, and then goes quiet" (`FAKE_AGENT`'s own doc),
 * which is exactly what the idle-classification wait below needs.
 *
 * "Never opened" is deliberately the SIMPLER of the two readings
 * `Session::has_unseen_output` treats identically ("a session with no
 * recorded stamp has never been seen") — rather than "opened once, then
 * produced new output while elsewhere" — because reliably driving fresh
 * output into an UNATTACHED session's real pane from this suite would need
 * a second raw terminal socket with no test helper behind it yet; both
 * readings reach the exact same observable state this test asserts on
 * (blue, "idle — new output").
 */
test("an idle session that was never opened reads unseen, and opening it clears that", async ({
  page,
  request,
}) => {
  // Budgeted above the sum of this test's own explicit waits (20s + 45s +
  // 20s + 20s, plus the untimed assertions' 5s default each) rather than
  // the config's 60s default, which that sum alone already clears with no
  // margin for the setup and clicks between them.
  test.setTimeout(150_000);
  const session = await createSession(request, { title: `seen-state-open-${Date.now()}` });
  try {
    // The session must stay UNOPENED until this test opens it by hand —
    // but auto-select (SPEC.md: opening a client counts as opening a
    // session) would otherwise attach it the instant `goto` loads if it
    // happens to be the fleet's newest, silently marking it seen before the
    // first assertion below ever runs. Pinning away to the shared fixture
    // is the suite's own established idiom for exactly this — see
    // `pinAutoSelect`'s own doc.
    await pinAutoSelect(page, await sharedSessionId(request));
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await expect(target.locator(".status-badge.idle.unseen")).toHaveText("idle — new output", {
      timeout: 45_000,
    });

    // Opening it is the automatic mark: the session view's own effect
    // fires on mount and PUTs the current activity as seen, and the row
    // must relabel itself grey once that write's fleet-events bump brings
    // the next listing read back — within a feed round trip, not the
    // fallback poll's much longer cadence.
    await target.locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await expect(target).toHaveAttribute("data-session-selected", "true");
    await expect(target.locator(".status-badge.idle")).toHaveText("idle", { timeout: 20_000 });
    await expect(target.locator(".status-badge.idle.unseen")).toHaveCount(0);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * A manual "mark unread" from the OPEN session's own row menu turns its
 * dot blue again while it stays the selected session, and the unseen state
 * STICKS rather than being immediately re-marked seen by the session's own
 * still-mounted auto-mark effect — the whole reason that effect keys on
 * the activity STAMP rather than on the unseen predicate (see
 * `session_view.rs`'s `mark_key` memo).
 */
test("a manual mark-unread on the open session sticks", async ({ page, request }) => {
  // Budgeted above the sum of this test's own explicit waits (20s + 45s +
  // 20s + 20s + 20s, plus the untimed assertions' 5s default each) rather
  // than the config's 60s default, which that sum alone already clears
  // with no margin for the setup, menu open, and clicks between them.
  test.setTimeout(150_000);
  const session = await createSession(request, { title: `seen-state-sticky-${Date.now()}` });
  try {
    await pinAutoSelect(page, await sharedSessionId(request));
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await expect(target.locator(".status-badge.idle.unseen")).toHaveText("idle — new output", {
      timeout: 45_000,
    });

    await target.locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await expect(target.locator(".status-badge.idle")).toHaveText("idle", { timeout: 20_000 });

    // The label must already read "mark unread" — the row is currently
    // seen — and clicking it must turn the dot blue again without moving
    // the selection.
    const reads = countReads(page);
    const listingReadsBeforeClear = reads.count("listing");
    await openRowMenu(target);
    const markSeenItem = target.locator(".session-row-mark-seen");
    await expect(markSeenItem).toHaveText("mark unread");
    await markSeenItem.click();
    await expect(target.locator(".status-badge.idle.unseen")).toHaveText("idle — new output", {
      timeout: 20_000,
    });
    await expect(target).toHaveAttribute("data-session-selected", "true");
    // Polled past at least one MORE listing read, rather than a fixed
    // sleep: SPEC_impl.md's 3-second connected-host refresh interval means
    // a plain `waitForTimeout` would either race a slow refresh under load
    // or waste time waiting on a fast one, and neither actually proves a
    // redraw happened — this does, by observing the read itself, which is
    // what proves the manual clear survives more than one redraw of the
    // still-open session, not just the instant after the click.
    await expect
      .poll(() => reads.count("listing"), {
        timeout: 20_000,
        message: "the sidebar must poll the listing at least once more before this proves anything",
      })
      .toBeGreaterThan(listingReadsBeforeClear);
    await expect(target.locator(".status-badge.idle.unseen")).toHaveText("idle — new output");
    await expect(target).toHaveAttribute("data-session-selected", "true");
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * `MarkSeen` is reachable and operable through the SAME role/keyboard path
 * every other menu command is.
 *
 * Nothing else in this file proves that. The ARIA/keyboard mechanics tests
 * above (`the actions menu exposes a real menu-button relationship`, `the
 * actions menu walks every item and wraps at both ends`, and their
 * siblings) all call `hideSeenState` to keep the item list at its
 * pre-existing five — deliberately, since they are about generic menu
 * mechanics, not this feature — and the seen-state tests above locate the
 * item by CSS class (`.session-row-mark-seen`) and drive it with a plain
 * `.click()`, never through `getByRole` or a keyboard step. A regression
 * that dropped this one item's `role="menuitem"`, its accessible name, or
 * its reachability from Enter/ArrowDown activation would pass every
 * existing test in this file untouched.
 *
 * `pinAutoSelect` is why: without it, this session — freshly created and
 * possibly the fleet's newest — is exactly the row `goto`'s own auto-select
 * would attach to (SPEC.md: opening a CLIENT counts as opening a session),
 * which marks it seen before the menu is ever opened and leaves the item
 * reading "mark unread" instead of the "mark read" this test looks for —
 * the same race `pinAutoSelect`'s own doc and DECISIONS.md's entry on it
 * already cover for the other seen-state tests above.
 */
test("the mark-seen item is reachable and operable by role and keyboard", async ({
  page,
  request,
}) => {
  test.setTimeout(90_000);
  const title = `seen-state-keyboard-${Date.now()}`;
  const session = await createSession(request, { title, cwd: "/tmp", invocation: "sleep 300" });
  try {
    await pinAutoSelect(page, await sharedSessionId(request));
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });

    await openRowMenu(target);
    const toggle = target.getByRole("button", { name: `session actions for ${title}` });
    const menu = target.getByRole("menu", { name: `session actions for ${title}` });
    // Offered only once the real supervisor's classifier settles this
    // fixture into a live status and the helm answers the seen-state
    // question — neither of which `openRowMenu` alone waits for, so this
    // item can legitimately take longer to appear than the others already
    // in the panel.
    const markRead = menu.getByRole("menuitem", { name: "mark read" });
    await expect(markRead).toBeVisible({ timeout: 45_000 });

    // Driven from OUTSIDE the menu, the same way `the actions menu walks
    // every item` drives its own navigation — Rename is the first item and
    // MarkSeen sits right after it (`MENU_ACTIONS`'s own order).
    await toggle.focus();
    await expect(toggle).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(menu.getByRole("menuitem", { name: "rename" })).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(markRead).toBeFocused();

    await page.keyboard.press("Enter");
    // MarkSeen deliberately does NOT close the menu on activation, unlike
    // Stop/Archive/Delete/Clone (`row.rs`'s `on_mark_seen` handler calls
    // only `on_mark_seen`, never `on_menu_toggle`) — the toggle stays
    // visible so its own label flip is the thing to watch, in place,
    // without navigating away and back. That flip — not a closed menu — is
    // the proof this was a real activation, once the write's fleet-events
    // bump brings the next listing read back.
    await expect(menu.getByRole("menuitem", { name: "mark unread" })).toBeVisible({
      timeout: 20_000,
    });
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * A dot with no toggle to offer still opens the row on click — the
 * regression an unconditional `stop_propagation` on `StatusBadgeView`'s
 * dot would reintroduce: that pixel area would neither open the row (its
 * click never reaching the ancestor `.session-row-open` button) nor do
 * anything else, since a no-toggle badge's `dot_onclick` is a no-op. Real
 * scenario: an old helm that predates `seen_activity_at` entirely, which
 * `hideSeenState` simulates the same way the ARIA/keyboard tests above use
 * it to keep the item list fixed.
 */
test("a dot with no seen-state toggle still opens the row on click", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `seen-state-dead-spot-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await hideSeenState(page);
    await pinAutoSelect(page, await sharedSessionId(request));
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await expect(target.locator(".status-dot")).toBeVisible({ timeout: 45_000 });
    // No toggle offered at all with the field hidden — the dead-spot risk
    // this test exists to rule out only exists when there is genuinely
    // nothing to toggle.
    await expect(target.locator(".status-dot-toggle")).toHaveCount(0);

    await target.locator(".status-dot").click();
    await waitForSessionRevealed(page, session.id);
    await expect(target).toHaveAttribute("data-session-selected", "true");
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * The dot click marks a DIFFERENT row's seen state without stealing the
 * selection away from whichever row is actually open — the click's
 * `stop_propagation` proven end to end, since `.status-dot` sits inside
 * the SAME `.session-row-open` button that a real open click would
 * otherwise fire.
 *
 * Session A is opened first and fully settled before session B is even
 * created, deliberately avoiding two sessions racing toward idle
 * together: this test's subject is the click's selection isolation, not
 * the classifier, and giving the two fixtures no reason to interleave
 * keeps a failure here pointing at the one property under test.
 */
test("the dot click marks a different row read without moving the selection", async ({
  page,
  request,
}) => {
  // Budgeted above the sum of this test's own explicit waits, both rows
  // combined (20s + 20s + 20s + 45s + 20s, plus the untimed assertions'
  // 5s default each) rather than the config's 60s default, which that sum
  // alone already clears with no margin for the setup and clicks between
  // them.
  test.setTimeout(150_000);
  const stamp = Date.now();
  const a = await createSession(request, { title: `seen-state-dot-a-${stamp}` });
  try {
    await pinAutoSelect(page, await sharedSessionId(request));
    await page.goto("/");
    const rowA = row(page, a.id);
    await expect(rowA).toBeVisible({ timeout: 20_000 });
    await rowA.locator(".session-row-open").click();
    await waitForSessionRevealed(page, a.id);
    await expect(rowA).toHaveAttribute("data-session-selected", "true");

    const b = await createSession(request, { title: `seen-state-dot-b-${stamp}` });
    try {
      const rowB = row(page, b.id);
      await expect(rowB).toBeVisible({ timeout: 20_000 });
      await expect(rowB.locator(".status-badge.idle.unseen")).toHaveText("idle — new output", {
        timeout: 45_000,
      });

      await rowB.locator(".status-dot").click();
      await expect(rowA).toHaveAttribute("data-session-selected", "true");
      await expect(rowB).toHaveAttribute("data-session-selected", "false");
      await expect(rowB.locator(".status-badge.idle")).toHaveText("idle", { timeout: 20_000 });
      await expect(rowB.locator(".status-badge.idle.unseen")).toHaveCount(0);
      // The click never even routed through the row's open handler — the
      // terminal pane on screen must still be A's, not B's.
      await expect(page.locator(".titlebar .title")).toContainText(`seen-state-dot-a-${stamp}`);
    } finally {
      await cleanupSession(request, b.id);
    }
  } finally {
    await cleanupSession(request, a.id);
  }
});

/**
 * The seen state is a helm-kept, shared fact (SPEC.md, Status), not a
 * per-client one — the same "no client keeps its own copy" rule the list
 * order and last-selected session already carry (Session list). A second
 * client that never touched the session either must see exactly the verdict
 * the first client's manual mark-unread left behind, the same two-context
 * shape `"a second client's launch alone takes the terminal over"` above
 * uses for the equivalent claim about selection.
 */
test("a manual mark-unread is visible to a second client that never touched the session", async ({
  browser,
  timeline,
  page,
  request,
}) => {
  // Budgeted above the sum of this test's own explicit waits, TWO clients'
  // worth (20s + 45s + 20s + 20s + 20s across the first client, then 20s +
  // 20s again for the second, plus the untimed assertions' 5s default
  // each), with generous headroom rather than the config's 60s default,
  // which a single client's own sum already clears on its own.
  test.setTimeout(180_000);
  const session = await createSession(request, { title: `seen-state-shared-${Date.now()}` });
  let second: import("@playwright/test").BrowserContext | undefined;
  try {
    // Same pin as the previous test, for the same reason: this session
    // must start genuinely unopened, not auto-attached by `goto` because it
    // happens to be the fleet's newest.
    await pinAutoSelect(page, await sharedSessionId(request));
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await expect(target.locator(".status-badge.idle.unseen")).toHaveText("idle — new output", {
      timeout: 45_000,
    });

    // Open it (clearing the unseen state) and then mark it unread by hand —
    // two writes, so the second client's answer cannot be explained by
    // having merely inherited the FIRST client's own never-marked default.
    await target.locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await expect(target.locator(".status-badge.idle")).toHaveText("idle", { timeout: 20_000 });
    await openRowMenu(target);
    await target.locator(".session-row-mark-seen").click();
    await expect(target.locator(".status-badge.idle.unseen")).toHaveText("idle — new output", {
      timeout: 20_000,
    });

    second = await newObservedContext(browser, timeline, {
      storageState: await page.context().storageState(),
    });
    const page2 = await second.newPage();
    // `session` is now the shared `last_selected` preference (client one
    // just opened it), so an unpinned `goto` here would auto-attach page2
    // to it too — SPEC.md's "opening a client counts as opening a session"
    // applies to EVERY client alike, and page2's own auto-mark effect would
    // then re-mark it seen from underneath this very assertion, defeating
    // "never touched" before it can be checked. Re-pinned for page2
    // specifically: the preference is shared, but a pin only holds until
    // the next write, and client one's own click already overwrote it once.
    await pinAutoSelect(page2, await sharedSessionId(request));
    await page2.goto("/");
    const target2 = row(page2, session.id);
    await expect(target2).toBeVisible({ timeout: 20_000 });
    await expect(target2.locator(".status-badge.idle.unseen")).toHaveText("idle — new output", {
      timeout: 20_000,
    });
  } finally {
    await second?.close();
    await cleanupSession(request, session.id);
  }
});
