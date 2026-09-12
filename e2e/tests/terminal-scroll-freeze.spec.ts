// A scrolled-back terminal can paint STALE rows while an agent keeps
// emitting output: the DOM disagrees with what `term.buffer.active` says
// belongs at those screen positions, and stays that way instead of
// catching up — a "split" viewport, part of it tracking new output
// correctly while the rest is stuck showing older content. This file
// drives a real session with a fake agent that never stops emitting,
// scrolls the viewport back with a real mouse wheel, and inspects
// RENDERED rows — the DOM the user actually sees — rather than xterm.js's
// buffer, which stays correct even when the paint above it does not (see
// `helpers/term.ts`'s `termText`, which reads the buffer and would show
// none of this).
//
// Farhelm's own JS never touches the viewport during live output except
// one reveal-time `scrollToBottom()` (`terminal.js`). That leaves four
// mechanisms this file's five shapes probe, none requiring a change to
// the vendored bundle (settled constraint — every fix here lives in
// Farhelm-owned code only):
//
// - Full scrollback, scrolled back: once the buffer is full and the user
//   has scrolled away from the tail, xterm.js's `BufferService.scroll`
//   decrements `ydisp` by one for every evicted line. Whether, and
//   exactly how, this leaves the DOM holding stale content is not
//   established from source (see `terminal.js`'s `refreshIfScrolledBack`
//   for the full account of what IS established); it reproduces directly,
//   and a throttled full-row `term.refresh()` fixes it. For the record of
//   a future reader deciding whether the workaround can go: with
//   `refreshIfScrolledBack` absent, the full-scrollback shape below failed
//   on both Chromium and WebKit in recorder runs, with rendered rows
//   thousands of records behind the buffer. The two scroll-region shapes
//   also failed before the fix, but their fixture was corrected afterwards
//   (offset and pacing), so those failures are not clean evidence on
//   their own.
// - Output confined to a DECSTBM scroll region: xterm.js's per-write
//   repaint maps each dirty screen row into a viewport row by adding
//   `(ybase - ydisp)` and stops requesting a refresh once that offset
//   reaches the terminal's row count, so a row further back than the
//   terminal is tall is never asked to repaint by that path. On its own
//   that is correct behavior (such a row is off-screen); it is recorded
//   because it rules out the per-write path as the repainter of a
//   scrolled-back viewport, not because it explains the stale DOM.
// - Synchronized output (DEC private mode 2026): while the mode is on,
//   xterm.js buffers every refresh request, including the workaround's
//   own; turning it off is itself a full-viewport refresh request in this
//   bundle (`case 2026` in the mode handler fires `onRequestRefreshRows`
//   with no range). So the shape does not make the split worse; it pins
//   that the workaround and xterm's own flush-time repaint coexist and
//   that a TUI using the mode does not regress.
// - Farhelm-owned: the per-island `ResizeObserver` calling `fit.fit()`
//   when a sibling band changes a pane's height mid-flood.

import { expect, test } from "./helpers/evidence";
import { type APIRequestContext, type Page } from "@playwright/test";
import path from "node:path";
import { cleanupSession, termText, waitForTermText } from "./helpers/term";
import { addTab, installTerminalSuiteHooks } from "./helpers/terminal-suite";
import { waitForSessionSocketOpen } from "./helpers/terminal-readiness";

installTerminalSuiteHooks();

const FARHELM_BIN = path.resolve(__dirname, "../../target/debug/farhelm");

/**
 * `counter` (fake_agent.rs): a plain, UNGATED, paced (2ms/record) producer
 * with no scroll region — "CUTOVER-NNNNNNNN" lines forever, until killed.
 * Used for shapes 1, 2, and 5. Shape 1 needs a pace slow enough that a
 * test can reliably observe "scrollback not yet at its 12,000-line floor"
 * without racing an unpaced burst; shape 2 needs the OPPOSITE timing
 * discipline for the same underlying reason — an unpaced producer evicts
 * so fast once scrollback is already full that the scrolled-back viewport
 * gets dragged to its floor before anything meaningful could be observed
 * (see that test's own docs). No gate is needed for any of the three: the
 * test only needs the producer running before it acts, not synchronized
 * to a specific byte.
 */
const COUNTER_INVOCATION = `"${FARHELM_BIN}" internal fake-agent --script counter`;

/**
 * `flood-region` (fake_agent.rs): the gated, paced producer with TWO
 * phases — plain full-screen output first (real scrollback to scroll back
 * into), then four rows of static banner text above a DECSTBM scroll
 * region, scrolling only inside it from then on. See `flood_region`'s own
 * Rust-side docs for why the first phase is required at all (a region
 * whose top sits below row 1 does not grow the buffer's scrollback, so
 * without it there would be nothing behind the region to scroll into) —
 * a real scroll region is what the DECSTBM dirty-row-mapping mechanism
 * (this file's header) needs to be reproducible at all. `--sync-output`
 * adds shape 4's DEC 2026 synchronized-output brackets around each chunk
 * of the SECOND phase.
 */
const REGION_INVOCATION = `"${FARHELM_BIN}" internal fake-agent --script flood-region`;
const REGION_SYNC_INVOCATION = `${REGION_INVOCATION} --sync-output`;

/** How many banner rows `flood_region` holds fixed above its scroll region. */
const REGION_BANNER_ROWS = 4;

async function createSession(
  request: APIRequestContext,
  title: string,
  invocation: string,
): Promise<string> {
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation, title },
  });
  expect(created.status(), await created.text()).toBe(200);
  return (await created.json()).id;
}

/**
 * Send the single gate byte `flood_gated`/`flood_region` block on, over the
 * raw socket `terminal.js` already opened — see `terminal-flood.spec.ts`'s
 * `sendFloodGateByte` for the identical reasoning (arbitrary byte value,
 * raw pty mode, socket-open wait rather than banner wait since a caller
 * may be holding writes).
 */
async function sendGateByte(page: Page, id: string) {
  await waitForSessionSocketOpen(page, id);
  await page.evaluate(() => {
    (window as any).__farhelmWs.send(new Uint8Array([0x67]));
  });
}

/**
 * Open a session's terminal in `page` and, for a gated producer, release
 * it. Mirrors `terminal-flood.spec.ts`'s `openFloodSession` shape (a
 * failed step after creation cleans up the session it already made) but
 * generalized over which fixture invocation and gating this file's five
 * shapes each need.
 */
async function openSession(
  page: Page,
  request: APIRequestContext,
  title: string,
  invocation: string,
  { gated }: { gated: boolean },
): Promise<string> {
  const id = await createSession(request, title, invocation);
  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row).toBeVisible();
    await row.click();
    if (gated) {
      await sendGateByte(page, id);
    } else {
      await waitForTermText(page, "FAKE-AGENT READY");
    }
    return id;
  } catch (err) {
    await cleanupSession(request, id);
    throw err;
  }
}

/**
 * One atomic read of the agent terminal's scroll position AND its
 * currently-rendered DOM rows, taken together in a single page turn so
 * neither can drift relative to the other between the two reads — which
 * matters most for shape 2, where the buffer can evict old scrollback and
 * shift `viewportY` out from under a caller that read it separately.
 *
 * `rendered` comes from the DOM renderer's OWN row elements
 * (`.xterm-rows` children — the only renderer this file's terminal ever
 * loads, per `terminal.js`), which is the actual painted surface a user
 * would see. `expected` is what the TERMINAL'S OWN xterm.js buffer says
 * belongs at each of those screen positions right now. A "split" (this
 * file's header defines the term) is exactly a persistent disagreement
 * between the two for part, but not all, of the viewport.
 */
async function viewportSnapshot(page: Page, elementId = "terminal") {
  return page.evaluate((elementId) => {
    const island = (window as any).__farhelmIslands?.[elementId];
    if (!island?.term) return null;
    const term = island.term;
    const buffer = term.buffer.active;
    const rowsEl = document.getElementById(elementId)?.querySelector(".xterm-rows");
    const rendered: string[] = rowsEl
      ? Array.from(rowsEl.children).map((row) => (row as HTMLElement).textContent ?? "")
      : [];
    const expected: string[] = [];
    for (let i = 0; i < term.rows; i++) {
      expected.push(buffer.getLine(buffer.viewportY + i)?.translateToString(true) ?? "");
    }
    return {
      viewportY: buffer.viewportY as number,
      baseY: buffer.baseY as number,
      rows: term.rows as number,
      rendered,
      expected,
    };
  }, elementId);
}

/** The agent terminal's current `term.rows`/`term.cols`, or `null` before it is mounted. */
async function terminalGeometry(
  page: Page,
  elementId = "terminal",
): Promise<{ rows: number; cols: number } | null> {
  return page.evaluate((elementId) => {
    const term = (window as any).__farhelmIslands?.[elementId]?.term;
    return term ? { rows: term.rows as number, cols: term.cols as number } : null;
  }, elementId);
}

/**
 * How many times the agent terminal's `refreshIfScrolledBack` workaround
 * (`terminal.js`) actually issued a `term.refresh()`, published on the
 * agent terminal's own `window.__farhelmTest` test hook. Reading this
 * before and after `expectStableViewport` is how a caller distinguishes
 * "the workaround engaged" from "the viewport happened to stay correct
 * regardless" — see `terminal.js`'s own docs on `scrolledRefreshCount`.
 */
async function scrolledRefreshCount(page: Page): Promise<number> {
  return page.evaluate(() => (window as any).__farhelmTest.scrolledRefreshCount as number);
}

/**
 * The shape of one reconciliation observation, retained across both phases
 * below so a failure message can show exactly which rows disagreed and
 * whether the viewport's own scroll position moved during the attempt —
 * recorded independent of pass/fail, since a full scrollback can
 * legitimately shift `viewportY` as old lines are evicted without that
 * drift alone being the reported bug (see `expectStableViewport`'s own
 * closing annotation).
 */
interface ReconcileObservation {
  matched: boolean;
  mismatchedRows: number[];
  viewportY: number;
  baseY: number;
  rendered: string[];
  expected: string[];
}

async function observeOnce(page: Page, elementId: string): Promise<ReconcileObservation> {
  const snap = await viewportSnapshot(page, elementId);
  if (!snap) {
    throw new Error(`the island at ${elementId} is not mounted`);
  }
  const mismatchedRows: number[] = [];
  for (let i = 0; i < snap.expected.length; i++) {
    if ((snap.rendered[i] ?? "").trimEnd() !== snap.expected[i].trimEnd()) {
      mismatchedRows.push(i);
    }
  }
  return {
    matched: mismatchedRows.length === 0 && snap.rendered.length === snap.expected.length,
    mismatchedRows,
    viewportY: snap.viewportY,
    baseY: snap.baseY,
    rendered: snap.rendered,
    expected: snap.expected,
  };
}

/**
 * Assert the scrolled-back viewport is, and REMAINS, an honest paint of
 * the buffer while output keeps arriving underneath it — the two-phase
 * shape this whole file's regression coverage rests on.
 *
 * Phase one tolerates a brief, genuine race between a completed
 * `term.write()` and the DOM catching up (see `viewportSnapshot`'s docs:
 * these are static, already-written buffer lines once scrolled away from
 * the tail, so this phase should settle almost immediately on a healthy
 * terminal — it exists only to absorb that one paint tick, not to paper
 * over a real split). Phase two is the actual regression pin: once
 * reconciled, it must STAY reconciled while output continues, which is
 * exactly the reported "the upper portion freezes while the lower portion
 * keeps scrolling" symptom — a bug that resolves the FIRST poll and then
 * drifts back out of sync would pass a one-shot check and fail this one.
 *
 * `intervalMs` (300ms) is deliberately generous relative to the
 * production fix's own 50ms refresh throttle (`terminal.js`,
 * `refreshIfScrolledBack`) and to a browser's ordinary ~16ms paint cadence
 * — several refresh opportunities pass between any two samples, so an
 * ordinary bounded double-buffering lag (the DOM briefly behind a buffer
 * that updates synchronously on every write, inherent to any renderer and
 * NOT the reported bug) has time to resolve on its own between samples.
 * The flip side is what this check does NOT catch: with three consecutive
 * 300ms samples required, it pins "no staleness lasting about a second or
 * more", not the 50ms window itself. A throttle silently lengthened to a
 * few hundred milliseconds would still pass; a green run is evidence the
 * viewport stays honest, not that the throttle constant is intact. A
 * real freeze does not resolve merely because more wall-clock time
 * passed, so this margin costs nothing against catching one.
 *
 * Failure carries the mismatched row indices and both the rendered and
 * expected text for the CURRENT viewport (not the whole scrollback, which
 * `.agents/test-authoring.md` asks polling helpers not to hoard), plus
 * the viewportY/baseY drift observed across the whole call, recorded
 * regardless of pass or fail (see `ReconcileObservation`'s own docs for
 * why).
 */
async function expectStableViewport(
  page: Page,
  elementId: string,
  { settleTimeoutMs = 5_000, holdMs = 3_000, intervalMs = 300 } = {},
) {
  const startedAt = Date.now();
  let last: ReconcileObservation | null = null;
  const initialViewportY = (await viewportSnapshot(page, elementId))?.viewportY ?? null;

  // Phase one: wait for the viewport to become an honest paint at all.
  for (;;) {
    last = await observeOnce(page, elementId);
    if (last.matched) break;
    if (Date.now() - startedAt > settleTimeoutMs) {
      // Carry the same premise evidence phase two reports: a stalled
      // producer (count not advancing) and a real split read differently
      // only if the message says which it was.
      const hooks = await page.evaluate(() => (window as any).__farhelmTest);
      throw new Error(
        `viewport at "${elementId}" never reconciled with the buffer within ${settleTimeoutMs}ms; ` +
          `mismatched rows ${JSON.stringify(last.mismatchedRows)}; ` +
          `viewportY=${last.viewportY} baseY=${last.baseY} scrolledRefreshCount=${hooks?.scrolledRefreshCount}\n` +
          `rendered: ${JSON.stringify(last.rendered)}\nexpected: ${JSON.stringify(last.expected)}`,
      );
    }
    // sleep-ok: bounded poll cadence while the viewport catches up to a completed write; the deadline above is checked every iteration.
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }

  // Phase two: it must not fall back out of sync while output keeps coming.
  //
  // A single mismatched sample here is deliberately NOT enough to fail:
  // `observeOnce` reads rendered DOM text and buffer text together in one
  // atomic browser turn, so within one sample there is no read/read race —
  // but a real renderer can still be caught mid-paint of a row that just
  // changed, one sample after a completed `term.write()`, on a healthy
  // terminal. This file's header defines a "split" as a SUSTAINED
  // disagreement, not a single busy tick, so this requires the mismatch to
  // reappear on `CONSECUTIVE_MISMATCH_THRESHOLD` samples in a row —
  // roughly `intervalMs` * that count of continuous staleness — before
  // treating it as the reported bug rather than ordinary paint latency.
  const CONSECUTIVE_MISMATCH_THRESHOLD = 3;
  let consecutiveMismatches = 0;
  const holdUntil = Date.now() + holdMs;
  while (Date.now() < holdUntil) {
    // sleep-ok: sample the held viewport at a fixed cadence during a finite observation window; the deadline is the loop condition, not this delay.
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
    last = await observeOnce(page, elementId);
    if (!last.matched) {
      consecutiveMismatches++;
      if (consecutiveMismatches >= CONSECUTIVE_MISMATCH_THRESHOLD) {
        const hooks = await page.evaluate(() => (window as any).__farhelmTest);
        throw new Error(
          `viewport at "${elementId}" reconciled once but drifted back out of sync while output kept arriving, ` +
            `sustained across ${consecutiveMismatches} consecutive samples ` +
            `(the reported "upper portion freezes" split); mismatched rows ${JSON.stringify(last.mismatchedRows)}; ` +
            `viewportY=${last.viewportY} baseY=${last.baseY} scrolledRefreshCount=${hooks?.scrolledRefreshCount}\n` +
            `rendered: ${JSON.stringify(last.rendered)}\nexpected: ${JSON.stringify(last.expected)}`,
        );
      }
    } else {
      consecutiveMismatches = 0;
    }
  }

  // Informational only: whether the scroll position itself held or
  // drifted is recorded on the test, not asserted — a full scrollback can
  // legitimately shift `viewportY` (`BufferService.scroll` decrementing
  // it as old lines are evicted, this file's header) without that being
  // the reported bug on its own.
  test.info().annotations.push({
    type: "viewport-y-drift",
    description: `initial=${initialViewportY} final=${last.viewportY} baseY=${last.baseY}`,
  });
}

/**
 * Scroll the agent terminal back with a REAL mouse wheel — not
 * `term.scrollLines()` — because the reported bug is about what a user's
 * own wheel input does, and Farhelm installs no wheel listener of its own
 * (`terminal.js`): whatever handles this is xterm.js's, which is the
 * point.
 *
 * Retries wheel ticks (rather than sending one large delta) until the
 * viewport has moved at least `minLines` back from wherever it started,
 * bounded by `timeoutMs` — the exact rows-per-wheel-tick mapping is an
 * xterm.js internal (`scrollSensitivity`) this file has no reason to
 * pin, so "keep nudging until far enough back" is the portable approach.
 */
async function scrollBackAtLeast(
  page: Page,
  elementId: string,
  minLines: number,
  timeoutMs = 10_000,
) {
  await page.locator(`#${elementId}`).hover();
  const before = await viewportSnapshot(page, elementId);
  if (!before) throw new Error(`the island at ${elementId} is not mounted`);
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    await page.mouse.wheel(0, -400);
    const now = await viewportSnapshot(page, elementId);
    if (now && before.viewportY - now.viewportY >= minLines) return;
    if (Date.now() > deadline) {
      throw new Error(
        `scrolling back at least ${minLines} lines at "${elementId}" did not land within ${timeoutMs}ms; ` +
          `started at viewportY=${before.viewportY}, now at ${now?.viewportY}`,
      );
    }
    // sleep-ok: pace successive wheel ticks so the browser has a chance to process each one; the deadline above bounds the whole retry loop.
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}

/**
 * Scroll back with a real wheel gesture until the ENTIRE viewport sits
 * below the active screen (`viewportY + rows <= baseY`) — every row shown
 * is then genuinely frozen, already-written scrollback, none of it still
 * being live-updated.
 *
 * The DECSTBM shapes need this precision that `scrollBackAtLeast`'s "at
 * least N lines" does not give: a scroll region can occupy most of the
 * screen, so scrolling back by a small, fixed offset can still leave the
 * BOTTOM of the viewport inside the active screen — showing rows that are
 * still genuinely live and racing the producer, not frozen. Sampling a
 * still-live row at an arbitrary instant will always show SOME bounded
 * lag relative to the production fix's own throttle window, no matter how
 * healthy the terminal is; that is real but ordinary latency, not the
 * reported split, and a test that does not fully clear the active screen
 * conflates the two. This was found directly: an earlier version of the
 * DECSTBM tests scrolled back by only `scrollBackAtLeast`'s fixed 20
 * lines, which left several of the viewport's bottom rows still inside
 * the live region, and consistently observed a small (single-digit
 * record), non-growing offset there even with the production fix in
 * place and engaged (confirmed via `scrolledRefreshCount` advancing) —
 * exactly the signature of bounded, expected latency, not a freeze.
 */
async function scrollFullyPastActiveScreen(
  page: Page,
  elementId: string,
  timeoutMs = 10_000,
) {
  await page.locator(`#${elementId}`).hover();
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    await page.mouse.wheel(0, -400);
    const now = await viewportSnapshot(page, elementId);
    if (now && now.viewportY + now.rows <= now.baseY) return;
    if (Date.now() > deadline) {
      throw new Error(
        `scrolling fully past the active screen at "${elementId}" did not land within ${timeoutMs}ms; ` +
          `now=${JSON.stringify(now)}`,
      );
    }
    // sleep-ok: pace successive wheel ticks so the browser has a chance to process each one; the deadline above bounds the whole retry loop.
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}

/**
 * Snap the viewport back to the live tail via `term.scrollToBottom()`
 * rather than a wheel gesture — legitimate here because returning to the
 * tail is setup for a POST-scroll assertion (the banner's own integrity),
 * not the behavior any of this file's tests are actually about; only the
 * scroll-BACK action itself needs to be a real user gesture (see
 * `scrollBackAtLeast`'s own docs).
 */
async function scrollToTail(page: Page, elementId: string) {
  await page.evaluate((elementId) => {
    (window as any).__farhelmIslands?.[elementId]?.term?.scrollToBottom();
  }, elementId);
}

// Baseline control: NO scroll region and scrollback still well short of
// its 12,000-line floor, so a split here would implicate the plainest
// possible live-scroll path rather than anything DECSTBM- or
// eviction-specific (this file's header). Uses the paced, ungated
// `counter` script: unpaced `flood`/`flood_gated` cannot promise "not yet
// full" deterministically (see `COUNTER_INVOCATION`'s own docs).
test("a plain flood, scrolled back mid-burst before scrollback fills, paints a consistent viewport", async ({
  page,
  request,
}) => {
  test.setTimeout(60_000);
  const title = `scroll-freeze-plain-${Date.now()}`;
  let id: string | undefined;
  try {
    id = await openSession(page, request, title, COUNTER_INVOCATION, { gated: false });
    // Confirms the premise before scrolling: `.agents/test-authoring.md`
    // asks a fixture premise be established, not merely assumed, before
    // the measurement that depends on it holds.
    const beforeScroll = await viewportSnapshot(page, "terminal");
    expect(
      beforeScroll && beforeScroll.baseY < 12_000,
      "the fixture premise is scrollback NOT yet at its floor; a slow test host may have already filled it",
    ).toBe(true);

    await scrollBackAtLeast(page, "terminal", 20);
    // The count advancing is the only proof this shape has that output was
    // still arriving during the hold (a producer that stalled would let a
    // static viewport pass trivially) and that the workaround engaged.
    const refreshCountBefore = await scrolledRefreshCount(page);
    await expectStableViewport(page, "terminal");
    expect(await scrolledRefreshCount(page)).toBeGreaterThan(refreshCountBefore);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The full-scrollback mechanism (this file's header): `BufferService.scroll`
// decrements `ydisp` once scrollback is FULL and evicting old lines. This
// is the one shape that deliberately wants scrollback already past its
// 12,000-line floor before the scroll.
//
// Uses the PACED `counter` script, not the unpaced full-speed
// `flood`/`flood_gated` producer — found directly while writing this
// test: at full, unpaced speed, ongoing evictions walk the whole
// ~12,000-line `ydisp` down to 0 within tens of milliseconds
// (`Math.max(ydisp-1,0)` per evicted line in `BufferService.scroll`),
// before the terminal (or a real user) could ever observe a stable
// "scrolled back by N" view at all. Once pinned at that
// floor, the viewport is showing "whichever line is currently oldest",
// which changes on EVERY new line — an unpaced producer can evict far
// faster than the production fix's own throttled repaint (or any
// realistic repaint rate) could ever track, which is a different, far
// more extreme condition than the reported bug (a user scrolls back to a
// chosen position and it goes stale) and made this shape fail
// unpredictably on host load alone, independent of any real defect. The
// paced producer keeps evictions at a human-observable rate, giving the
// scrolled-back view time to be meaningfully sampled while still full and
// still evicting — matching what the report actually describes.
test("a plain flood, scrolled back after scrollback is already full, paints a consistent viewport", async ({
  page,
  request,
}) => {
  test.setTimeout(90_000);
  const title = `scroll-freeze-full-${Date.now()}`;
  let id: string | undefined;
  try {
    id = await openSession(page, request, title, COUNTER_INVOCATION, { gated: false });
    await expect
      .poll(
        async () => (await viewportSnapshot(page, "terminal"))?.baseY ?? 0,
        // 12,000 records at 2ms is at least 24s of pure sleep; scheduling and
        // pty overhead usually land the fill nearer 30s, more under load.
        { timeout: 60_000, message: "waiting for scrollback to reach its 12,000-line floor" },
      )
      .toBeGreaterThanOrEqual(12_000);

    await scrollBackAtLeast(page, "terminal", 20);
    // The workaround itself, not merely the outcome, must have engaged:
    // see `scrolledRefreshCount`'s own docs (terminal.js) for why "the
    // viewport happened to stay correct" and "the workaround engaged" are
    // different claims, and only this assertion tells them apart.
    const refreshCountBefore = await scrolledRefreshCount(page);
    await expectStableViewport(page, "terminal");
    expect(await scrolledRefreshCount(page)).toBeGreaterThan(refreshCountBefore);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The DECSTBM dirty-row-mapping mechanism (this file's header): a scroll
// region's rows may never be told to repaint once they fall further back
// than the viewport is tall. `flood_region` holds four fixed banner rows
// above a region that scrolls underneath them; the fixed rows are
// asserted intact both before scrolling away and again after returning to
// the tail (a real regression there would show as the banner silently
// vanishing or repeating), and the scrolled-BACK view — which lands in
// the plain scrollback `flood_region`'s first phase built, not the region
// itself; see that fixture's own docs for why a DECSTBM region below row
// 1 keeps no scrollback of its own — gets the same reconciliation check
// as the plain-flood shapes above.
test("output confined to a DECSTBM scroll region, scrolled back mid-burst, paints a consistent viewport", async ({
  page,
  request,
}) => {
  test.setTimeout(60_000);
  const title = `scroll-freeze-region-${Date.now()}`;
  let id: string | undefined;
  try {
    id = await openSession(page, request, title, REGION_INVOCATION, { gated: true });
    // The banner only appears once phase 1 (plain scrollback-building
    // output — see `flood_region`'s Rust-side docs) has ended and the
    // DECSTBM region is active; this is the readiness boundary for "the
    // region has started", not `baseY` — a region below row 1 does not
    // grow the buffer's own scrollback at all (confirmed directly against
    // the vendored xterm.js bundle while building this fixture), so
    // `baseY` stops advancing almost as soon as the region begins and
    // cannot signal this moment.
    await waitForTermText(page, "REGION-BANNER-1", 15_000);

    const banner = await viewportSnapshot(page, "terminal");
    expect(banner, "the agent island must be mounted before scrolling").toBeTruthy();
    for (let i = 0; i < REGION_BANNER_ROWS; i++) {
      expect(banner!.expected[i].trimEnd()).toBe(`REGION-BANNER-${i + 1}`);
    }

    await scrollFullyPastActiveScreen(page, "terminal");
    const refreshCountBefore = await scrolledRefreshCount(page);
    await expectStableViewport(page, "terminal");
    expect(await scrolledRefreshCount(page)).toBeGreaterThan(refreshCountBefore);
    // The fixture premise this whole test depends on: the region producer
    // must still be running throughout the observation above, or the
    // reconciliation check just passed against a quiet, finished
    // producer instead of a live one (`.agents/test-authoring.md`: assert
    // the fixture premise before relying on it). `REGION_RECORDS`
    // (fake_agent.rs) is sized to make this true by a wide margin, but
    // this is what actually proves it for THIS run.
    expect(await termText(page)).not.toContain("REGION-DONE");

    // Back at the tail, the fixed banner rows must still be untouched by
    // however much the region kept scrolling underneath them during the
    // scrolled-back observation above — this is what actually
    // distinguishes "a scroll region exists and was honored" from "the
    // terminal just scrolled the whole screen and happened to still look
    // fine".
    await scrollToTail(page, "terminal");
    const afterScroll = await viewportSnapshot(page, "terminal");
    for (let i = 0; i < REGION_BANNER_ROWS; i++) {
      expect(afterScroll!.expected[i].trimEnd()).toBe(`REGION-BANNER-${i + 1}`);
    }
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Synchronized output (DEC private mode 2026), this file's header:
// while the mode is on xterm.js buffers every refresh request, and
// turning it off is a full-viewport refresh, so this shape checks that the
// workaround and that flush-time repaint coexist rather than that the mode
// worsens the split. Identical to the
// scroll-region test above except the producer brackets every chunk of
// scrolled records in `ESC[?2026h` ... `ESC[?2026l`, so both mechanisms
// are live at once here.
test("output confined to a synchronized-update scroll region, scrolled back mid-burst, paints a consistent viewport", async ({
  page,
  request,
}) => {
  test.setTimeout(60_000);
  const title = `scroll-freeze-region-sync-${Date.now()}`;
  let id: string | undefined;
  try {
    id = await openSession(page, request, title, REGION_SYNC_INVOCATION, { gated: true });
    // See the plain scroll-region test above for why the banner's own text
    // is the readiness boundary here rather than `baseY`.
    await waitForTermText(page, "REGION-BANNER-1", 15_000);

    await scrollFullyPastActiveScreen(page, "terminal");
    const refreshCountBefore = await scrolledRefreshCount(page);
    await expectStableViewport(page, "terminal");
    expect(await scrolledRefreshCount(page)).toBeGreaterThan(refreshCountBefore);
    // See the plain scroll-region test above for why this premise must be
    // asserted, not assumed.
    expect(await termText(page)).not.toContain("REGION-DONE");

    await scrollToTail(page, "terminal");
    const afterScroll = await viewportSnapshot(page, "terminal");
    for (let i = 0; i < REGION_BANNER_ROWS; i++) {
      expect(afterScroll!.expected[i].trimEnd()).toBe(`REGION-BANNER-${i + 1}`);
    }
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The Farhelm-owned mechanism (this file's header): a mid-flood
// `fit.fit()` triggered by the per-island `ResizeObserver`, which observes
// the agent pane's OWN element directly (`paneObserver.observe(el)`,
// terminal.js). The tab strip's close-confirmation row sits above
// `.terminal-panes` in the same flex column, so showing it shrinks that
// column's `flex: 1` remainder — and with it the agent pane's own box —
// which is what fires the observer; vendored `addon-fit.js` then resizes
// the terminal only if `term.rows`/`term.cols` actually changed. The test
// below asserts that a toggle actually produces that change at least
// once, rather than assuming it. This flood is the plain, unregioned
// `counter` producer on the AGENT pane (which stays selected and visible
// throughout — only a second tab's controls are touched), so a split here
// points specifically at the resize/fit path.
test("a plain flood survives a sibling band repeatedly changing height mid-burst", async ({
  page,
  request,
}) => {
  test.setTimeout(60_000);
  const title = `scroll-freeze-band-${Date.now()}`;
  let id: string | undefined;
  try {
    id = await openSession(page, request, title, COUNTER_INVOCATION, { gated: false });

    // A second tab, WITHIN this same flooding session, exists purely to
    // drive `.tab-close`'s confirmation row — opening it selects it, but
    // the agent pane (this test's actual subject) is reselected right
    // after so its own live flood is what the rest of this test observes.
    const tabId = await addTab(page, 0);
    await page.locator('.tab-strip [data-terminal="agent"]').click();
    await expect(page.locator('.terminal-pane[data-terminal="agent"]')).toBeVisible();

    await scrollBackAtLeast(page, "terminal", 20);

    const restGeometry = await terminalGeometry(page, "terminal");
    expect(restGeometry, "the agent island must be mounted before toggling the band").toBeTruthy();

    // Toggle the confirmation row several times while output keeps
    // arriving underneath the still-visible agent pane. Each toggle
    // shrinks or restores `.terminal-panes`' own box (the agent pane's
    // container), which fires the `ResizeObserver` this test's own docs
    // describe; `sawGeometryChange` is the fixture premise this test
    // actually needs — that at least one toggle really changed
    // `term.rows`/`term.cols`, not merely that the observer fired.
    const close = page.locator(`.tab-slot[data-tab-id="${tabId}"] .tab-close`);
    let sawGeometryChange = false;
    for (let i = 0; i < 6; i++) {
      await close.click();
      await expect(page.locator(".tab-confirm")).toHaveCount(1);
      // Poll for the geometry to differ from rest (bounded, ~1s) rather
      // than sleeping a fixed settle: a late ResizeObserver callback on a
      // slow host would otherwise read as "no resize" and fail the premise
      // spuriously. Falling through on timeout is deliberate — a single
      // toggle that did resize is enough for `sawGeometryChange`.
      const changed = await page
        .waitForFunction(
          ({ elementId, rest }) => {
            const term = (window as any).__farhelmIslands?.[elementId]?.term;
            return !!term && (term.rows !== rest.rows || term.cols !== rest.cols);
          },
          { elementId: "terminal", rest: { rows: restGeometry!.rows, cols: restGeometry!.cols } },
          { timeout: 1_000, polling: 50 },
        )
        .then(() => true, () => false);
      if (changed) sawGeometryChange = true;
      await page.locator(".tab-confirm .confirm-cancel").click();
      await expect(page.locator(".tab-confirm")).toHaveCount(0);
      // sleep-ok: settle after the row is removed so the restore resize can land before the next toggle; there is no readiness signal for "the observer callback, if any, has fired".
      await new Promise((resolve) => setTimeout(resolve, 150));
    }
    expect(
      sawGeometryChange,
      "toggling the band must actually resize the agent terminal, or this test exercises no fit() at all",
    ).toBe(true);

    // A resize re-derives `ydisp` inside xterm (`Buffer.resize`), so the
    // premise this whole shape is about — the viewport is still scrolled
    // back when the hold begins — has to be re-established here rather
    // than assumed from the wheel gesture above the toggles.
    const afterToggles = await viewportSnapshot(page, "terminal");
    expect(
      afterToggles && afterToggles.viewportY < afterToggles.baseY,
      "the six re-fits must leave the viewport scrolled back; a resize that snapped it to the tail would turn the hold into a live-tail comparison",
    ).toBe(true);
    // As in the other shapes: the count advancing proves output was still
    // arriving during the hold and that the workaround engaged.
    const refreshCountBefore = await scrolledRefreshCount(page);
    await expectStableViewport(page, "terminal");
    expect(await scrolledRefreshCount(page)).toBeGreaterThan(refreshCountBefore);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});
