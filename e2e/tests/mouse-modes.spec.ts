// The mouse-mode fake-agent script's reattach-restoration coverage
// (PLAN_M6_5.md item 2). A dedicated file, not an addition to
// the terminal spec family: per this milestone's own testing decision,
// new coverage starts in its own per-area file so one subject stays
// findable and runnable together.
//
// This is the one end-to-end path that proves mouse-mode restoration
// survives a client detach/reattach cycle, and it deliberately drives the
// mouse through REAL pointer events (`page.mouse.click`), never synthetic
// escape bytes written straight to the socket: xterm.js itself must
// encode the click into a mouse report, so the whole chain under test is
// DOM click -> xterm.js encoding -> WebSocket -> supervisor -> tmux ->
// agent, not merely the server-side replay machinery in isolation.
//
// Two client paths are pinned together because they are genuinely
// different code, and the test exists to prove BOTH are wired: vendored
// xterm.js routes its legacy (X10-derived) mouse encoding through
// `onBinary` UNCONDITIONALLY whenever that encoding is active (a plain
// "which mode is on" check, not a per-byte inspection), while its SGR
// encoding is pure ASCII and always routed through `onData` instead. The
// legacy leg's own click is deliberately placed at a column that FORCES a
// byte above 0x7f through the wire — proving the `onBinary` path carries
// a real high byte intact, not merely that it fires at all — which is
// also, incidentally, the only thing in this suite that drives `onBinary`
// with a real browser mouse event: PLAN_M6_5.md item 1 extracted
// `term.onBinary`'s byte conversion into `term-bytes.js` behind a
// `node --test` unit contract, and that unit test alone cannot prove the
// extracted helper is actually wired to the page — this test is what
// closes that gap.
import { test, expect, Page, APIRequestContext } from "@playwright/test";
import path from "node:path";
import { cleanupSession, fillCreateForm, termText, waitForTermText } from "./helpers/term";

/**
 * The mouse-mode fake-agent script (`crates/farhelm/src/fake_agent.rs`'s
 * `mouse_modes`), built from an absolute path exactly like
 * terminal.spec.ts's own fake-agent invocations do.
 */
const MOUSE_MODES_AGENT_INVOCATION = `"${
  path.resolve(__dirname, "../../target/debug/farhelm")
}" internal fake-agent --script mouse-modes`;

/**
 * The first three bytes of each mouse-report wire shape the fake agent's
 * hex echo can produce (`fake_agent.rs`'s `CueScanner` docs cover both in
 * full): legacy is `ESC [ M` followed by three fixed data bytes; SGR is
 * `ESC [ <` followed by digits/semicolons up to a terminating `M`/`m`.
 * Used as a byte-WINDOW to search for in the reconstructed hex-echo
 * stream (see `hexEchoBytes`/`countReportWindows`), not as a text
 * substring — a report's bytes can land split across more than one
 * hex-echoed line.
 */
const LEGACY_REPORT_PREFIX = [0x1b, 0x5b, 0x4d] as const; // ESC [ M
const SGR_REPORT_PREFIX = [0x1b, 0x5b, 0x3c] as const; // ESC [ <

/**
 * Reconstruct the ordered byte stream the fake agent's hex echo produced,
 * from the terminal's rendered text. `hex_echo_loop` (fake_agent.rs)
 * emits one hex-encoded LINE per raw `read()`, and the pty/tmux are free
 * to split a single mouse report's bytes across more than one `read()` —
 * so a report's hex tokens can legitimately land on separate hex-echo
 * lines. Scanning a single line's text for a report's byte-window (as an
 * earlier version of this test did) would then miss any report split
 * exactly at that boundary — flaky by construction, not merely
 * theoretically so, since `read()` boundaries are a kernel/pty scheduling
 * detail this test does not control. Flattening every two-digit hex token
 * in the WHOLE buffer into one ordered array, independent of which line
 * it printed on, is what makes report detection correct regardless of how
 * reads happened to be chunked.
 */
function hexEchoBytes(text: string): number[] {
  const bytes: number[] = [];
  for (const match of text.matchAll(/\b([0-9a-f]{2})\b/g)) {
    bytes.push(parseInt(match[1], 16));
  }
  return bytes;
}

/**
 * How many times the 3-byte sequence `prefix` occurs as a CONTIGUOUS
 * window in `bytes` — a click's own mousedown+mouseup dispatches two
 * reports (press and release), so this is a count, not a presence check,
 * and every assertion in this test settles to an EXACT expected count
 * rather than merely "more than before" (see `waitForReportCount`'s
 * docs for why that distinction matters for the reattach legs).
 */
function countReportWindows(bytes: number[], prefix: readonly [number, number, number]): number {
  let count = 0;
  for (let i = 0; i + 2 < bytes.length; i++) {
    if (bytes[i] === prefix[0] && bytes[i + 1] === prefix[1] && bytes[i + 2] === prefix[2]) {
      count++;
    }
  }
  return count;
}

/**
 * The three DATA bytes (button, column, row for a legacy report)
 * immediately following the FIRST occurrence of `prefix` in `bytes`.
 * Used only for the legacy leg's byte-fidelity assertion, where "the
 * first occurrence" is unambiguous — it happens before any other click in
 * this test has produced a report of this shape, so there is no earlier
 * occurrence it could be confused with.
 */
function firstReportBody(
  bytes: number[],
  prefix: readonly [number, number, number],
): number[] | undefined {
  for (let i = 0; i + 5 < bytes.length; i++) {
    if (bytes[i] === prefix[0] && bytes[i + 1] === prefix[1] && bytes[i + 2] === prefix[2]) {
      return bytes.slice(i + 3, i + 6);
    }
  }
  return undefined;
}

/** Current count of `prefix`-shaped reports in the live terminal buffer. */
async function countReports(
  page: Page,
  prefix: readonly [number, number, number],
): Promise<number> {
  return countReportWindows(hexEchoBytes(await termText(page)), prefix);
}

/**
 * Poll until exactly `expected` reports of `prefix`'s shape have arrived.
 *
 * An EXACT target, not a `toBeGreaterThan(baseline)` check: a real click
 * dispatches mousedown AND mouseup, so it always produces TWO reports —
 * and a poll that is satisfied by just the press would resolve while the
 * release is still in flight. If that release then lands AFTER this test
 * has gone on to read a "post-reattach baseline" or detach the client
 * entirely, it either inflates that baseline out from under the next
 * assertion or vanishes into whatever the client missed by detaching too
 * early — either way, a later assertion could pass for the wrong reason.
 * Settling to the FULL expected count before moving on is what closes
 * that race.
 */
async function waitForReportCount(
  page: Page,
  prefix: readonly [number, number, number],
  expected: number,
  message: string,
) {
  await expect
    .poll(() => countReports(page, prefix), { timeout: 15_000, message })
    .toBe(expected);
}

/**
 * Wait until THIS attachment's replay has fully revealed — the same
 * `replay.revealed` flag the `waitForReplayReveal`/`ReplayRecord` machinery in
 * helpers/terminal-suite.ts synchronizes on (mainly for
 * terminal-replay-rename.spec.ts's reattach tests),
 * read here directly off the primary terminal's singleton test hook
 * (`window.__farhelmTest`, published alongside `__farhelmTermReady`)
 * rather than the per-island registry those tests use, since this file
 * only ever drives the one primary `#terminal`.
 *
 * Waiting for replayed TEXT (e.g. `FAKE-AGENT READY` reappearing) is not
 * equivalent: text already present in scrollback can satisfy a substring
 * check the instant the DOM exists, before the reattach's own catch-up
 * phase — during which terminal.js buffers writes rather than applying
 * them — has actually finished. A click dispatched into that window
 * races the restored DECSET mode itself: `PaneModes`' escape sequences
 * are part of what gets replayed, so a click before they have landed
 * could hit a pane the client-side xterm instance does not yet believe
 * has mouse reporting on, encoding nothing at all.
 */
async function waitForReplayReveal(page: Page, timeout = 15_000) {
  await expect
    .poll(
      () => page.evaluate(() => !!(window as any).__farhelmTest?.replay?.revealed),
      {
        timeout,
        message: "waiting for the reattach's replay to fully reveal before treating it as settled",
      },
    )
    .toBe(true);
}

/** Locator for a session row by its exact title. */
function rowByTitle(page: Page, title: string) {
  return page.locator(".session-row").filter({
    has: page.locator(".session-title", { hasText: new RegExp(`^${title}$`) }),
  });
}

/**
 * Look up a session's id from the real API by its title. Used ONCE, right
 * after creation, to capture the id this test's own cleanup will need —
 * not re-called at cleanup time itself: a title is not a stable enough
 * handle to re-resolve there (a title collision, or the row already being
 * gone by the time cleanup runs for a reason unrelated to this test, would
 * silently point cleanup at the wrong session or at none at all). The id
 * captured here is the one and only source of truth `cleanupSession`
 * receives.
 *
 * POLLED, not single-shot: the helm's listing is eventually consistent
 * with respect to a just-created session — a terminal can be fully
 * attached and interactive while a listing read taken at that instant
 * still lacks the row (observed live: a stale list, then the session
 * present 90ms later). A single-shot read here intermittently returned
 * `undefined`, which left `id` unset for the WHOLE test and turned any
 * later outcome — including a fully passing body — into the `finally`
 * block's loud lost-track failure, leaking the fake agent and (because
 * cleanup never ran) compounding the odds for every later repeat against
 * the same stack. Ten seconds is far above the observed lag and still
 * fails fast enough to diagnose when something is genuinely wrong.
 */
async function findSessionIdByTitle(
  request: APIRequestContext,
  title: string,
): Promise<string | undefined> {
  const deadline = Date.now() + 10_000;
  for (;;) {
    const listing = await (await request.get("/api/sessions")).json();
    const id = listing.sessions.find((s: any) => s.title === title)?.id;
    if (id !== undefined || Date.now() > deadline) return id;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

/**
 * Pick a column this test can legally click, wide enough to leave margin
 * on both sides of the required [96, 223] window.
 *
 * 223 is xterm.js's OWN cap for its legacy (X10-derived) mouse encoding:
 * each coordinate is packed into a single byte offset by 32, and once
 * `column + 32` (or `row + 32`) would exceed 255 the encoder returns an
 * EMPTY string — the click produces no report at all rather than a
 * corrupted one, so a column past 223 would not fail loudly, it would
 * just silently starve every assertion below of anything to find. 96 is
 * the first column whose encoded byte (`96 + 32 = 128`) genuinely leaves
 * the 7-bit ASCII range — a click below it keeps every report byte ASCII,
 * which cannot distinguish correct high-byte transport through `onBinary`
 * from a UTF-8-mangling or truncation bug there, exactly the fidelity
 * this test exists to prove (PLAN_M6_5.md item 2 review).
 */
async function chooseClickColumn(page: Page): Promise<number> {
  const cols = await page.evaluate(() => (window as any).__farhelmTerm.cols);
  expect(
    cols,
    "the terminal must be comfortably wider than 101 columns for this test's column choice to stay inside [96, 223]",
  ).toBeGreaterThan(101);
  return Math.min(cols - 5, 200);
}

/**
 * Click the CENTER of a specific (1-based) terminal cell, computed from
 * real pixel geometry (`box.width`/`term.cols`, `box.height`/`term.rows`)
 * rather than a fixed pixel offset — the terminal's on-screen size
 * depends on the viewport and font metrics, neither of which this test
 * controls directly. A real click (`page.mouse.click` performs a genuine
 * mousedown then mouseup) always yields TWO reports under button
 * tracking, which every assertion in this test accounts for via
 * `waitForReportCount`'s exact targets rather than assuming a count of 1.
 */
async function clickTerminalCell(page: Page, col: number, row: number) {
  const box = (await page.locator("#terminal").boundingBox())!;
  const geometry = await page.evaluate(() => {
    const t = (window as any).__farhelmTerm;
    return { cols: t.cols, rows: t.rows };
  });
  const cellWidth = box.width / geometry.cols;
  const cellHeight = box.height / geometry.rows;
  await page.mouse.click(
    box.x + (col - 0.5) * cellWidth,
    box.y + (row - 0.5) * cellHeight,
  );
}

test("mouse-modes-restored-on-reattach", async ({ page, request }) => {
  // Four settle-to-exact-count polls (up to 15s each) across TWO full
  // detach/reattach cycles push comfortably past the suite's default
  // 60s budget under any real load — the same reasoning multi-step
  // terminal.spec.ts tests bump this for.
  test.setTimeout(120_000);
  const title = `mouse-modes-${Date.now()}`;
  let id: string | undefined;
  // Flips true only once the fake agent has actually printed its ready
  // marker — the point past which a real session is KNOWN to exist
  // server-side, distinct from `id` (which additionally requires the
  // lookup below to have succeeded). `finally` needs both: `created`
  // without `id` means a session exists that this test has lost track
  // of, which is the leak `cleanupSession`'s own docs warn a swallowed
  // error would hide — that case must fail loudly, not be treated the
  // same as "nothing was ever created" (an early exception, before the
  // form was even submitted).
  let created = false;
  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: MOUSE_MODES_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    // Success navigates straight into the new session's terminal, same
    // as every create-dialog flow in terminal.spec.ts.
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FAKE-AGENT READY");
    created = true;
    // Captured ONCE, here, and never re-derived — see `id`'s own
    // `finally` handling below and `findSessionIdByTitle`'s docs.
    id = await findSessionIdByTitle(request, title);

    const clickCol = await chooseClickColumn(page);
    const clickRow = 2;

    // --- Legacy leg: cue DECSET 1000 alone, click at a column that
    // FORCES a high byte, and prove the full report body — including
    // that byte — reached the agent via onBinary. ---
    await page.locator("#terminal").click();
    await page.keyboard.type("legacy");
    await page.keyboard.press("Enter");
    // The fake agent's own confirmation that the cue was RECOGNIZED and
    // acted on (fake_agent.rs's `mouse_modes` docs) — waiting for this
    // rather than the cue's own hex echo is what makes the click below
    // race-free: the mode is guaranteed live by the time this resolves.
    await waitForTermText(page, "MOUSE-MODE:legacy");

    await clickTerminalCell(page, clickCol, clickRow);
    // Settle to BOTH reports (press + release) before inspecting
    // anything — see `waitForReportCount`'s own docs for why accepting
    // just one is a real race, not excess caution.
    await waitForReportCount(
      page,
      LEGACY_REPORT_PREFIX,
      2,
      "a legacy mouse report must reach the agent via onBinary",
    );
    const legacyBody = firstReportBody(hexEchoBytes(await termText(page)), LEGACY_REPORT_PREFIX);
    expect(legacyBody, "the full three-byte report body must have arrived").toHaveLength(3);
    expect(
      legacyBody![1],
      "the column byte must survive onBinary as the genuine high byte this click's column encodes to, not a mangled or truncated one",
    ).toBeGreaterThan(0x7f);

    // --- Detach and reattach through the list/back UI — the same
    // mechanism terminal.spec.ts's own "back tears down the mounted
    // terminal; reopening the same session mounts a fresh one" test uses
    // and proves down to WebSocket readyState: going back genuinely
    // closes THIS attachment's socket, and reopening the row is a fresh
    // reattach with real replay, not a still-open connection quietly
    // continuing (which would prove nothing about restoration). The
    // fake-agent PROCESS itself is untouched throughout — only the
    // client's attachment cycles — exactly matching what a real user's
    // tab-close-and-reopen does. ---
    // Detach = select another session (the shared row), then reselect.
    await rowByTitle(page, "e2e-session").locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
    await rowByTitle(page, title).locator(".session-row-open").click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForReplayReveal(page);

    // The reveal above guarantees the reattach's replay — including any
    // restored DECSET escape sequences — has fully landed, so this read
    // is a settled baseline, not a value still catching up. The replay
    // carries the first click's two reports forward (tmux's own pane
    // history), so the baseline for "new reports arrived" is whatever it
    // settled at here, not zero.
    const legacyBaselineAfterReattach = await countReports(page, LEGACY_REPORT_PREFIX);
    await clickTerminalCell(page, clickCol, clickRow);
    await waitForReportCount(
      page,
      LEGACY_REPORT_PREFIX,
      legacyBaselineAfterReattach + 2,
      "legacy mouse mode must survive the reattach (PaneModes restoration)",
    );

    // --- SGR leg: cue DECSET 1006 on top, click, and prove an
    // SGR-shaped (onData-delivered) report reaches the agent. No fresh
    // focus click is needed first: `clickTerminalCell` above already
    // focused xterm's hidden input — mouse-tracking mode does not change
    // that xterm still focuses on mousedown. ---
    await page.keyboard.type("sgr");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "MOUSE-MODE:sgr");

    const sgrBaseline = await countReports(page, SGR_REPORT_PREFIX);
    await clickTerminalCell(page, clickCol, clickRow);
    await waitForReportCount(
      page,
      SGR_REPORT_PREFIX,
      sgrBaseline + 2,
      "an SGR mouse report must reach the agent via onData",
    );

    // --- A SECOND detach/reattach: the legacy leg above already proves
    // legacy-mode restoration, but nothing yet exercises SGR's OWN
    // restoration — a regression that dropped `mouse_sgr` specifically
    // from `PaneModes`' replay (while leaving `mouse_standard` intact)
    // would pass every assertion above. Repeating the cycle for the SGR
    // leg is what actually pins that branch. ---
    // Detach = select another session (the shared row), then reselect.
    await rowByTitle(page, "e2e-session").locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
    await rowByTitle(page, title).locator(".session-row-open").click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForReplayReveal(page);

    const sgrBaselineAfterReattach = await countReports(page, SGR_REPORT_PREFIX);
    await clickTerminalCell(page, clickCol, clickRow);
    await waitForReportCount(
      page,
      SGR_REPORT_PREFIX,
      sgrBaselineAfterReattach + 2,
      "SGR mouse mode must survive the reattach (PaneModes restoration)",
    );
  } finally {
    if (id) {
      // The ordinary path: a failed assertion partway through must not
      // leak a long-running fake-agent process into a later run in this
      // serially-run suite — `cleanupSession` itself throws (rather than
      // swallowing) on a failed stop/delete for the identical reason.
      await cleanupSession(request, id);
    } else if (created) {
      // A real session exists (the agent proved it by printing READY)
      // but this test never captured its id — re-deriving one HERE by
      // title, as an earlier version of this cleanup did, is exactly
      // the silent-leak shape under review: a title collision or a race
      // against the row's own listing could resolve to the wrong
      // session, or to none, and either way the failure would vanish
      // into a `.catch(() => undefined)` with nothing left alive to
      // clean up and no signal that anything went wrong. Failing loudly
      // instead turns an invisible leak into a visible, diagnosable test
      // failure — console.error first so the leaked title is on record
      // even if this throw ends up racing a primary failure already in
      // flight (JS's `finally`-throw-wins semantics mean only one of the
      // two errors ultimately surfaces from this `try`).
      console.error(
        `mouse-modes-restored-on-reattach: session "${title}" was created but its id was never captured — it is now leaking`,
      );
      throw new Error(`cleanup: lost track of session "${title}"'s id; it cannot be cleaned up`);
    }
  }
});
