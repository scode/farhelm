// ---------------------------------------------------------------------
// M5: replay presentation and rename (PLAN_M5.md items 5 and 6)
//
// This file has its own stack reset, so the replay tests can fill their
// sessions' scrollback without polluting another section.
//
// Several of these tests HOLD a catch-up phase open (see
// `holdCatchUpFromNextLoad`) rather than racing it. That is what makes the
// acceptance assertable at all: the contract is about what is on screen
// DURING the catch-up, and a real supervisor's marker arrives too fast to
// observe from Node — the window is not merely short, it has no lower
// bound this suite controls.
// ---------------------------------------------------------------------


import { expect, newObservedContext, test } from "./helpers/evidence";
import { type Page } from "@playwright/test";
import { openRowMenu, SESSION_LISTING, stubFeed } from "./helpers/fleet";
import { attachSession, cleanupSession, termText, waitForTermText } from "./helpers/term";
import {
  addTab,
  createTabSession,
  disableReconnectFromNextLoad,
  FAKE_AGENT_INVOCATION,
  fulfillAsHelm,
  islandText,
  replayRecord,
  runInShell,
  selectTerminal,
  sharedSessionRow,
  shellMarker,
  waitForIslandText,
  waitForReplayReveal,
  installTerminalSuiteHooks,
} from "./helpers/terminal-suite";
import { waitForSessionMounted, waitForSessionRevealed } from "./helpers/terminal-readiness";

installTerminalSuiteHooks({ tabSweep: true });

/**
 * Arm terminal.js's test-only replay controls for every attach the page
 * makes from its NEXT navigation onward (`window.__farhelmTestReplay`,
 * read once per mount — see `replayControls` in terminal.js).
 *
 * An init script rather than a plain `evaluate`, because the attaches
 * these tests care about happen after a reload, which would wipe a global
 * set on the current document.
 *
 * `holdMarker` keeps a catch-up phase open past the marker that would
 * normally end it — the seam every hidden-state assertion below depends
 * on. The three limits let the graceful-degradation bounds be crossed in
 * milliseconds and kilobytes instead of in seconds and megabytes: no
 * fixture in this suite can make a real supervisor replay 3 MiB, send a
 * million frames, or go silent mid-catch-up, so without these the bounds
 * would be untested rather than tested loosely.
 *
 * The idle window is stretched to a minute by default, and that is not
 * belt-and-braces: holding a phase open means NOTHING else may end it out
 * from under the assertions, and once the real replay is buffered the
 * stream is by definition silent — so the production five-second watchdog
 * would flush the buffer partway through a test's own round trips, turning
 * a held phase into an `idle` one at random. A test that wants the idle
 * bound sets it back down with `setReplayLimits` at the moment it is ready
 * for it.
 */
async function holdCatchUpFromNextLoad(
  page: Page,
  overrides: { bufferBytes?: number; bufferChunks?: number; idleMs?: number } = {},
) {
  await page.addInitScript((overrides) => {
    (window as any).__farhelmTestReplay = {
      holdMarker: true,
      idleMs: 60_000,
      ...overrides,
    };
  }, overrides);
}

/**
 * Wait until this island's catch-up phase is being HELD open — every byte
 * the supervisor meant to replay has arrived and been buffered, the marker
 * that would have ended the phase has been recorded instead of acted on,
 * and nothing has been written to xterm.js.
 *
 * This is the synchronization point the hidden-state assertions need:
 * "mid-catch-up" is otherwise a moment no test can reliably be inside.
 */
async function waitForHeldCatchUp(page: Page, elementId: string) {
  await expect
    .poll(
      () =>
        page.evaluate(
          (el) =>
            (window as any).__farhelmIslands?.[el]?.test?.replay?.heldReason ?? null,
          elementId,
        ),
      {
        timeout: 60_000,
        message: `waiting for ${elementId} to be holding its catch-up open`,
      },
    )
    .toBe("marker");
}

/** Let a held catch-up phase finish, applying the ending it deferred. */
async function releaseCatchUp(page: Page, elementId: string) {
  await page.evaluate(
    (el) => (window as any).__farhelmIslands[el].test.releaseCatchUp(),
    elementId,
  );
}

/**
 * Retune one island's degradation bounds mid-attach.
 *
 * The limits live in the record terminal.js itself reads, so a test can
 * set them relative to what a REAL replay actually delivered — which is
 * the only way to cross the size bound deterministically, since the size
 * of a session's own replay is not something this suite can predict.
 */
async function setReplayLimits(
  page: Page,
  elementId: string,
  limits: { bufferBytes?: number; bufferChunks?: number; idleMs?: number },
) {
  await page.evaluate(
    ({ el, limits }) => {
      Object.assign((window as any).__farhelmIslands[el].test.replay.limits, limits);
    },
    { el: elementId, limits },
  );
}

/**
 * Deliver `count` synthetic terminal frames to one island, `gapMs` apart,
 * through the island's OWN message handler — the same entry point a real
 * frame takes.
 *
 * Scheduled inside the page rather than from Node: the idle bound is a
 * measurement of SILENCE, and a Node-driven loop would put a
 * remote-debugging round trip inside every gap, making the test's own
 * timing a variable the assertion cannot control for.
 *
 * Returns the exact byte count delivered, so a caller can reason about the
 * byte bound without re-deriving the encoding.
 */
async function injectReplayFrames(
  page: Page,
  elementId: string,
  { prefix, size, count = 1, gapMs = 0 }: {
    prefix: string;
    size: number;
    count?: number;
    gapMs?: number;
  },
): Promise<number> {
  return page.evaluate(
    ({ el, prefix, size, count, gapMs }) => {
      const island = (window as any).__farhelmIslands[el];
      const encoder = new TextEncoder();
      // ASCII only, so one character is one byte and `size` is an exact
      // byte count. The marker leads, so it survives xterm's line wrapping
      // and can be found in the buffer afterwards.
      const build = (index: number) => {
        const head = `${prefix}${index}`;
        return head + ".".repeat(Math.max(0, size - head.length - 2)) + "\r\n";
      };
      let delivered = 0;
      return new Promise<number>((resolve) => {
        let sent = 0;
        const tick = () => {
          const payload = encoder.encode(build(sent));
          delivered += payload.length;
          island.ws.onmessage({ data: payload.buffer });
          sent += 1;
          if (sent < count) setTimeout(tick, gapMs);
          else resolve(delivered);
        };
        tick();
      });
    },
    { el: elementId, prefix, size, count, gapMs },
  );
}

/**
 * Deliver `count` ZERO-LENGTH frames to one island, `gapMs` apart.
 *
 * An empty frame is a legal WebSocket message and a supervisor that is a
 * different machine can send as many as it likes; the island must not
 * treat one as progress. Kept separate from `injectReplayFrames` because
 * the point is the absence of a payload, not a payload of size zero.
 */
async function injectEmptyFrames(
  page: Page,
  elementId: string,
  { count, gapMs }: { count: number; gapMs: number },
) {
  await page.evaluate(
    ({ el, count, gapMs }) => {
      const island = (window as any).__farhelmIslands[el];
      return new Promise<void>((resolve) => {
        let sent = 0;
        const tick = () => {
          island.ws.onmessage({ data: new ArrayBuffer(0) });
          sent += 1;
          if (sent < count) setTimeout(tick, gapMs);
          else resolve();
        };
        tick();
      });
    },
    { el: elementId, count, gapMs },
  );
}

/**
 * Replace `window.WebSocket`, from the page's next navigation onward, with
 * one that never finishes connecting: it stays in CONNECTING forever, and
 * only `close()` ever moves it.
 *
 * The one failure this suite cannot stage any other way. A real helm
 * either completes the handshake or refuses it — "the socket is still
 * negotiating and always will be" is a network state, not a server
 * behavior, and it is precisely the state in which a terminal has no path
 * for input at all.
 *
 * The static readyState constants are part of the shim because
 * terminal.js reads them off the GLOBAL `WebSocket` (`ws.readyState ===
 * WebSocket.OPEN`), which is this class once it is installed — omitting
 * them would make every liveness check compare against `undefined` and
 * quietly answer "not open" for the wrong reason.
 */
async function stuckWebSocketFromNextLoad(page: Page) {
  await page.addInitScript(() => {
    class StuckWebSocket {
      static CONNECTING = 0;
      static OPEN = 1;
      static CLOSING = 2;
      static CLOSED = 3;
      url: string;
      readyState = 0;
      binaryType = "blob";
      onopen: ((ev: unknown) => void) | null = null;
      onmessage: ((ev: unknown) => void) | null = null;
      onclose: ((ev: unknown) => void) | null = null;
      onerror: ((ev: unknown) => void) | null = null;
      constructor(url: string) {
        this.url = url;
      }
      send() {}
      close() {
        this.readyState = 3;
        const onclose = this.onclose;
        // Asynchronously, like the real close handshake — a synchronous
        // callback would run inside the caller that just closed it.
        if (onclose) setTimeout(() => onclose({}), 0);
      }
      addEventListener() {}
      removeEventListener() {}
    }
    (window as any).WebSocket = StuckWebSocket;
  });
}

/**
 * Trigger two reads of the URLs `matches` accepts and resolve once both have
 * landed in the page.
 *
 * Two, always, and it is not padding: the first read is the one that RETIRES
 * an optimistic correction (`ListView`'s and `SessionView`'s `renamed`
 * signals hold the typed title until a reply settles it), so a title still
 * standing after the SECOND is a title the view is rendering from the
 * server's own answer with no correction left underneath it — which is the
 * whole difference between a rename that landed and one that only painted.
 *
 * `trigger` is what asks for each read, and it is a parameter rather than a
 * wait because nothing asks on its own any more: this used to count polls,
 * and a healthy page performs none (PLAN_M6_75.md item 6). Callers pass the
 * feed stub's notification, which is what the helm would have sent.
 *
 * `matches` takes a parsed `URL` rather than a regex over the raw string
 * deliberately. It was a regex anchored at the end (`/\/api\/sessions$/`),
 * which stopped matching the moment the listing started carrying a query —
 * and a predicate that never matches here does not fail loudly, it hangs for
 * thirty seconds and then blames the read. Matching on `pathname` is immune
 * to whatever parameters the request grows next.
 */
async function afterTwoReads(page: Page, matches: (url: URL) => boolean, trigger: () => void) {
  for (let i = 0; i < 2; i++) {
    const landed = page.waitForResponse(
      (response) => response.request().method() === "GET" && matches(new URL(response.url())),
      { timeout: 30_000 },
    );
    trigger();
    await landed;
  }
}

// PLAN_M5.md acceptance 1 for the agent terminal, in the deterministic
// form that plan settles on: reattaching to a session whose scrollback
// runs deeper than one screen lands at the tail with NO intermediate state
// shown.
//
// The assertions come in two halves, and both are needed. DURING the
// catch-up — held open, which is the only way to be inside that window —
// the terminal element is genuinely hidden, the placeholder is genuinely
// on screen, and xterm.js has been given nothing: those are computed-style
// and buffer-content facts, not the island's opinion of itself, so
// deleting the line that hides the terminal fails this test. AFTER the
// release, the replay reached xterm.js as EXACTLY ONE `term.write()`, the
// reveal happened inside that write's completion callback (so xterm.js had
// consumed the whole replay before anything became visible), and the first
// visible frame was at the tail.
//
// This test fails against M4's behavior — where every replay chunk was its
// own write into a visible terminal — which is what makes it the
// milestone's executable acceptance rather than a description of the code.
test("reattach-lands-at-tail (agent terminal): hidden through catch-up, one write, viewport at the tail", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `replay-agent-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await attachSession(page, id);
    await waitForTermText(page, "FAKE-AGENT READY");

    // Scrollback deeper than one screen: the fake agent's `spam` exists
    // for exactly this (see its own docs), and 200 lines is far past any
    // viewport this suite runs at.
    await page.locator("#terminal").click();
    await page.keyboard.type("spam 200");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "spam-line-200");

    // A reload is the harshest detach/reattach there is: a brand-new
    // xterm.js with an empty buffer, so everything below came from this
    // attach's own replay. The hold is armed first, so the reattach's
    // catch-up stays open instead of completing before Node can look.
    await holdCatchUpFromNextLoad(page);
    await page.reload();
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);
    await waitForHeldCatchUp(page, "terminal");

    // Mid-catch-up, asserted against the DOM and the buffer rather than
    // against the island's own bookkeeping.
    await expect(page.locator("#terminal")).toBeHidden();
    await expect(page.locator("#term-connecting")).toBeVisible();
    const held = await replayRecord(page, "terminal");
    expect(held.bufferedBytes, "the replay has arrived and is being held").toBeGreaterThan(0);
    expect(held.writesWhileHidden, "nothing reaches xterm.js during the catch-up").toBe(0);
    expect(
      await termText(page),
      "not one line of the replay may be painted before it is complete",
    ).not.toContain("spam-line-");

    await releaseCatchUp(page, "terminal");
    const replay = await waitForReplayReveal(page, "terminal");
    expect(replay.revealReason, "the marker is what ended the catch-up").toBe("marker");
    expect(
      replay.writesWhileHidden,
      "the whole replay must reach xterm.js as one write, never chunk by chunk",
    ).toBe(1);
    expect(
      replay.revealedInWriteCallback,
      "the reveal must wait for xterm.js to have consumed the replay",
    ).toBe(true);
    expect(
      replay.viewportAtTailOnReveal,
      "the first visible frame is at the tail",
    ).toBe(true);
    expect(replay.buffering).toBe(false);
    expect(replay.bufferedBytes, "nothing is held once the flush has taken it").toBe(0);

    // The replay is not merely batched but complete, and the terminal is
    // genuinely on screen afterwards.
    expect(await termText(page)).toContain("spam-line-200");
    await expect(page.locator("#terminal")).toBeVisible();
    await expect(page.locator("#term-connecting")).toBeHidden();
    // Live, and typeable without a click: focus is placed at the reveal,
    // since an element that is `visibility: hidden` cannot take it (the
    // regression this covers made every freshly opened session need a
    // click before it would accept a keystroke).
    await page.keyboard.type("after-replay");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:after-replay");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The same acceptance for a TAB, which is a separate claim rather than a
// duplicate: every terminal has its own socket and its own marker
// (PLAN_M5.md item 5), so the buffering is per island — a version that
// only ever hid the agent terminal would pass the test above and fail this
// one.
//
// The tab is SELECTED before the hidden-state assertions, deliberately: an
// unselected pane is hidden as a whole (app.css's `.terminal-pane`), so a
// hidden terminal inside one proves nothing at all. Selected, the pane is
// on screen and the only thing hiding the terminal is its own catch-up.
test("reattach-lands-at-tail (tab): hidden through catch-up, one write, viewport at the tail", async ({
  page,
  request,
}) => {
  test.setTimeout(150_000);
  const title = `replay-tab-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await attachSession(page, id);

    const tabId = await addTab(page, 0);
    const element = `terminal-${tabId}`;
    await waitForSessionMounted(page, id, { tabId });
    // Deeper than one screen, printed by the tab's own shell. The marker
    // only ever appears in the OUTPUT, never in the typed line, for the
    // reason `runInShell` documents.
    await runInShell(
      page,
      element,
      "sh -c 'for i in $(seq 1 200); do echo TABSPAM-$i; done'",
      "TABSPAM-200",
    );

    await holdCatchUpFromNextLoad(page);
    await page.reload();
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);
    await waitForSessionMounted(page, id, { tabId });
    await waitForHeldCatchUp(page, element);

    // Selection is not persisted, so the reattached view lands on the
    // agent terminal; this brings the tab's pane on screen so that what is
    // hidden below is the TERMINAL, not the pane around it.
    await selectTerminal(page, tabId);
    await expect(page.locator(`[id="${element}"]`)).toBeHidden();
    await expect(page.locator(`[id="term-connecting-${tabId}"]`)).toBeVisible();
    expect(
      await islandText(page, element),
      "a tab's replay is held back exactly like the agent terminal's",
    ).not.toContain("TABSPAM-");

    await releaseCatchUp(page, element);
    const replay = await waitForReplayReveal(page, element);
    expect(replay.revealReason).toBe("marker");
    expect(
      replay.writesWhileHidden,
      "a tab's replay is batched exactly like the agent terminal's",
    ).toBe(1);
    expect(replay.revealedInWriteCallback).toBe(true);
    expect(replay.viewportAtTailOnReveal).toBe(true);

    await waitForIslandText(page, element, "TABSPAM-200", 30_000);
    await expect(page.locator(`[id="${element}"]`)).toBeVisible();
    await expect(page.locator(`[id="term-connecting-${tabId}"]`)).toBeHidden();

    // Typed with NO click first, which is the tab's half of the focus
    // contract: focus is placed when the terminal is revealed, reading the
    // selection AS IT IS THEN. An implementation that placed focus at
    // mount time would have given it to the agent terminal — the
    // reattached view's own initial selection — and these keystrokes would
    // have gone there instead, leaving this wait to time out.
    const after = shellMarker("AFTER-REPLAY");
    await page.keyboard.type(after.command);
    await page.keyboard.press("Enter");
    await waitForIslandText(page, element, after.expected, 30_000);
    expect(
      await termText(page),
      "the keystrokes went to the selected tab, not to the agent terminal",
    ).not.toContain(after.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The first graceful-degradation bound (PLAN_M5.md item 5): a replay that
// outgrows the buffer flushes and goes live rather than being dropped or
// erroring. The limit is retuned against what THIS session's real replay
// actually delivered, since no fixture here can produce 3 MiB of history —
// what is under test is the rule, not the constant.
//
// Three properties, and the middle one is the easy thing to get wrong: a
// chunk that reaches the limit exactly is still buffered (the bound is a
// crossing, not a ceiling), the chunk that crosses it flushes everything
// as ONE write with nothing lost, and bytes after the flush go straight to
// the screen — degraded to a batched-but-visible catch-up, never to data
// loss.
test("replay-degrades-on-size: crossing the byte bound flushes once, keeps every byte, then goes live", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `replay-size-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await holdCatchUpFromNextLoad(page);
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);
    await waitForHeldCatchUp(page, "terminal");

    // Everything the real attach replayed is already held, so the bound
    // can be set exactly 200 bytes above it.
    const held = await replayRecord(page, "terminal");
    await setReplayLimits(page, "terminal", { bufferBytes: held.bufferedBytes + 200 });

    const first = await injectReplayFrames(page, "terminal", {
      prefix: "SIZE-UNDER-",
      size: 200,
    });
    const atLimit = await replayRecord(page, "terminal");
    expect(
      atLimit.bufferedBytes,
      "a chunk landing exactly ON the limit is still buffered — the bound is a crossing",
    ).toBe(held.bufferedBytes + first);
    expect(atLimit.revealReason, "nothing has ended the catch-up yet").toBeNull();
    expect(atLimit.writesWhileHidden).toBe(0);

    await injectReplayFrames(page, "terminal", { prefix: "SIZE-OVER-", size: 64 });
    const replay = await waitForReplayReveal(page, "terminal", 15_000);
    expect(replay.revealReason, "the byte bound is what ended it").toBe("size");
    expect(
      replay.writesWhileHidden,
      "the crossing chunk is flushed WITH the rest, as one write",
    ).toBe(1);
    expect(replay.bufferedBytes, "the buffer is emptied by the flush").toBe(0);
    await expect(page.locator("#terminal")).toBeVisible();

    // Every byte survived the degradation — the one before the bound, the
    // one that crossed it, and the session's own replay underneath both.
    const flushed = await termText(page);
    expect(flushed).toContain("SIZE-UNDER-0");
    expect(flushed).toContain("SIZE-OVER-0");
    expect(flushed).toContain("FAKE-AGENT READY");

    // And the terminal is live rather than still batching: a later frame
    // is written on its own, which is what "goes live" means.
    await injectReplayFrames(page, "terminal", { prefix: "SIZE-AFTER-", size: 64 });
    await waitForTermText(page, "SIZE-AFTER-0");
    const live = await replayRecord(page, "terminal");
    expect(
      live.writesWhileHidden,
      "writes after the reveal are not part of the hidden catch-up",
    ).toBe(1);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The second bound, and the one whose SHAPE the plan argues for: idle, not
// total duration. A slow but progressing replay must keep buffering — that
// is the case the whole feature exists for — while a stream that has gone
// quiet without a marker must flush promptly.
//
// Both halves are asserted here, and the first is what a total-duration
// cap would fail: frames arrive at intervals shorter than the window, for
// a total time LONGER than the window, and the terminal is still hidden at
// the end of it. Then the frames stop, and the same window expires.
//
// This test waits on wall-clock time, which the rest of this suite avoids
// on principle. Here the elapsed time IS the behavior under test, and the
// frames are scheduled inside the page (see `injectReplayFrames`) so the
// intervals are the browser's own rather than a round trip's.
test("replay-degrades-on-idle: a progressing replay keeps buffering; a quiet one flushes once", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `replay-idle-${Date.now()}`;
  const idleMs = 1_500;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await holdCatchUpFromNextLoad(page);
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);
    await waitForHeldCatchUp(page, "terminal");
    // The short window is armed only NOW, once the real replay is safely
    // buffered: mounting with it would have let the watchdog expire during
    // this test's own setup, since a held phase is silent by construction
    // (see `holdCatchUpFromNextLoad`). The next injected frame is what
    // re-arms it at this interval.
    await setReplayLimits(page, "terminal", { idleMs });

    // Four frames, 700 ms apart: each gap is comfortably under the window,
    // while the last frame lands ~2.1 s after the first — past a window
    // that never re-armed, which would have fired at 1.5 s and revealed
    // the terminal before this returns. The margin is deliberate: a gap
    // would have to more than double under load to trip the bound early.
    await injectReplayFrames(page, "terminal", {
      prefix: "IDLE-",
      size: 64,
      count: 4,
      gapMs: 700,
    });
    const progressing = await replayRecord(page, "terminal");
    expect(
      progressing.revealReason,
      "a replay that keeps arriving keeps buffering, however long it takes in total",
    ).toBeNull();
    expect(progressing.writesWhileHidden).toBe(0);
    await expect(page.locator("#terminal")).toBeHidden();

    // Now nothing arrives, and the same window expires.
    const replay = await waitForReplayReveal(page, "terminal", 15_000);
    expect(replay.revealReason, "silence without a marker is what ends it").toBe("idle");
    expect(
      replay.writesWhileHidden,
      "the degradation is a batched-but-visible catch-up, not a chunk-by-chunk one",
    ).toBe(1);
    await expect(page.locator("#terminal")).toBeVisible();
    await expect(page.locator("#term-connecting")).toBeHidden();
    const flushed = await termText(page);
    expect(flushed).toContain("IDLE-0");
    expect(flushed).toContain("IDLE-3");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A catch-up ENDED by a detach, which is the case the protocol's own docs
// warn about: the supervisor owes no marker to an attach something else
// tore down, so a presentation that waited for one would hide those
// terminals forever. The buffered bytes must become visible UNDER the
// banner rather than being dropped with the attachment.
//
// Provoked by a real takeover — a second view of the same session — rather
// than by a synthetic notice, so what ends the phase is the same
// `Detached` the supervisor really sends.
test("replay-degrades-on-detach: a takeover mid-catch-up shows what arrived, under the banner", async ({
  browser,
  timeline,
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `replay-detach-${Date.now()}`;
  let id: string | undefined;
  let second: Awaited<ReturnType<typeof browser.newContext>> | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await holdCatchUpFromNextLoad(page);
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);
    await waitForHeldCatchUp(page, "terminal");
    const held = await replayRecord(page, "terminal");
    expect(held.bufferedBytes).toBeGreaterThan(0);
    await expect(page.locator("#terminal")).toBeHidden();

    // A second client takes the session, which detaches this one where it
    // stands: mid-catch-up, with a marker it will now never receive.
    second = await newObservedContext(browser, timeline);
    const page2 = await second.newPage();
    await page2.goto("/");
    await page2.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionRevealed(page2, id);

    const replay = await waitForReplayReveal(page, "terminal", 30_000);
    expect(replay.revealReason, "the detach is what ended the catch-up").toBe("detached");
    expect(
      replay.writesWhileHidden,
      "what did arrive is written — batched, once — rather than dropped",
    ).toBe(1);
    await expect(page.locator("#terminal")).toBeVisible();
    await expect(page.locator("#term-connecting")).toBeHidden();
    await expect(page.locator("#term-banner")).toContainText("Detached");
    expect(
      await termText(page),
      "the replay this attach did receive is on screen under the banner",
    ).toContain("FAKE-AGENT READY");
  } finally {
    if (second) await second.close();
    if (id) await cleanupSession(request, id);
  }
});

// The other markerless ending: the socket simply closes. A client-initiated
// detach gets no notice at all and a failed attach only closes, so on those
// paths the close IS the signal that no more bytes are coming — a
// presentation that ignored it would leave the terminal hidden with a
// "connecting…" line over it forever.
test("replay-degrades-on-close: a socket closing mid-catch-up flushes, reveals, and banners", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `replay-close-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await holdCatchUpFromNextLoad(page);
    // What is under test is M5's degradation and the banner that explains
    // it, both of which describe a terminal that STAYS closed. Since
    // PLAN_M6.md item 7 a dropped connection is instead recovered from,
    // with the recovery surface replacing that banner — a different
    // contract, pinned by its own tests, and this one still holds
    // underneath it (see `disableReconnectFromNextLoad`).
    await disableReconnectFromNextLoad(page);
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);
    await waitForHeldCatchUp(page, "terminal");
    await expect(page.locator("#terminal")).toBeHidden();

    // Closed from the client side, which is what a dropped connection
    // looks like to this island: the same `onclose`, with no notice
    // preceding it.
    await page.evaluate(() => (window as any).__farhelmWs.close());

    const replay = await waitForReplayReveal(page, "terminal", 30_000);
    expect(replay.revealReason, "the close is what ended the catch-up").toBe("closed");
    expect(replay.writesWhileHidden, "the buffer is written, not discarded").toBe(1);
    await expect(page.locator("#terminal")).toBeVisible();
    await expect(page.locator("#term-connecting")).toBeHidden();
    await expect(page.locator("#term-banner")).toContainText("Connection closed");
    expect(await termText(page)).toContain("FAKE-AGENT READY");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md has listed rename in the v1 client surface since the beginning
// (PLAN_M5.md item 6). From the list, the row's own field renames the
// session and the row takes the new title AT ONCE — and, two re-reads
// later, still has it, which is what separates a rename that reached the
// supervisor from one this view merely painted over its own listing.
//
// The re-reads are driven from here through a stubbed feed rather than
// waited for: they used to be the listing poll, which M6.75 removed, and
// making them this test's to trigger also removes the shared stack's
// revision churn from the picture.
test("rename-from-list: the row takes the new title and keeps it across re-reads", async ({
  page,
  request,
}) => {
  test.setTimeout(90_000);
  const title = `rename-list-${Date.now()}`;
  const renamed = `${title}-renamed`;
  let id: string | undefined;
  const feed = await stubFeed(page);
  let revision = 1;
  try {
    const created = await request.post("/api/sessions", {
      data: { cwd: "/tmp", invocation: FAKE_AGENT_INVOCATION, title },
    });
    expect(created.status(), await created.text()).toBe(200);
    id = (await created.json()).id as string;

    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(revision);
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row).toBeVisible({ timeout: 15_000 });
    await openRowMenu(row);
    await row.locator(".session-row-rename").click();
    // The field opens seeded with the current title, which is what makes
    // renaming an edit rather than a retype.
    await expect(row.locator(".rename-input")).toHaveValue(title);
    await row.locator(".rename-input").fill(renamed);
    await row.locator(".rename-submit").click();

    // The field closes and the row reads the new name without waiting for
    // any re-read — the optimistic half of the contract.
    await expect(row.locator(".rename-form")).toHaveCount(0);
    await expect(row.locator(".session-title")).toHaveText(renamed);

    // The server's own answer, not this view's: the supervisor serves its
    // listing from the in-memory session map, so a store-only rename would
    // fail exactly here.
    const listed = await (await request.get(`/api/sessions/${id}`)).json();
    expect(listed.title).toBe(renamed);

    // And the re-read listing agrees — see `afterTwoReads` for why two.
    await afterTwoReads(page, SESSION_LISTING, () => feed.notify(++revision));
    await expect(row.locator(".session-title")).toHaveText(renamed);
    await expect(row.locator(".action-error")).toHaveCount(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});


// SPEC.md names control characters as THE refusal for a supplied title,
// and PLAN_M5.md item 6 makes the supervisor's own message the contract:
// the client sends what was typed verbatim and shows what comes back,
// rather than duplicating the rule client-side and inventing its own
// wording for it.
//
// What must survive the refusal is everything else, and "everywhere" is
// meant literally: the row still SHOWS the old title while the rejected
// draft sits in the still-open field, the server still holds it, and the
// list is unchanged once the field is closed. A version that replaced the
// title with the field would leave the only visible name being the one the
// supervisor just refused.
test("rename-refused: a control-character title shows the supervisor's words and changes nothing", async ({
  page,
  request,
}) => {
  test.setTimeout(90_000);
  const title = `rename-refused-${Date.now()}`;
  // A C0 control character embedded in an otherwise ordinary title: this
  // is the terminal-injection case the supervisor refuses, since a title
  // is echoed into terminals by `tracing` consumers.
  const refused = "bad\u0001title";
  let id: string | undefined;
  try {
    const created = await request.post("/api/sessions", {
      data: { cwd: "/tmp", invocation: FAKE_AGENT_INVOCATION, title },
    });
    expect(created.status(), await created.text()).toBe(200);
    id = (await created.json()).id as string;

    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row).toBeVisible({ timeout: 15_000 });
    await openRowMenu(row);
    await row.locator(".session-row-rename").click();
    await row.locator(".rename-input").fill(refused);
    await row.locator(".rename-submit").click();

    // The supervisor's own sentence, relayed verbatim through the helm.
    await expect(row.locator(".action-error")).toContainText("control characters", {
      timeout: 15_000,
    });
    // The old title is still on screen next to the field that was
    // refused, not replaced by the draft.
    await expect(row.locator(".rename-current-title")).toHaveText(title);
    // The field stays open with the rejected text still in it, so fixing
    // the title is one keystroke rather than a retype.
    await expect(row.locator(".rename-input")).toHaveValue(refused);
    // Nothing about the session moved: not on the server...
    const fetched = await (await request.get(`/api/sessions/${id}`)).json();
    expect(fetched.title).toBe(title);
    // ...and not in the list, which still shows the old name once the
    // field is closed.
    await row.locator(".rename-cancel").click();
    await expect(row.locator(".session-title")).toHaveText(title);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The verbatim contract's sharpest case, and the reason the field is a
// `<textarea>`: `<input type=text>` STRIPS line breaks out of a pasted
// value, so a multi-line title would have been silently repaired into
// something acceptable before it was ever sent — the client quietly
// altering the user's data AND stepping in front of the supervisor's own
// refusal. Both halves are pinned here: the field really holds the
// newline, and the supervisor really refuses it.
//
// Inserted with `insertText` rather than typed: it is the paste path (an
// input insertion no keydown handler sees), which matters because Enter in
// this field submits instead of inserting a line break — typing cannot
// produce this title at all.
//
// The session view is the surface under test so that its own half of "the
// old title stays" is covered too: the header keeps showing the real name
// while the refusal is on screen.
test("rename-refused-pasted-newline: a multi-line title reaches the supervisor intact and is refused", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `rename-newline-${Date.now()}`;
  const pasted = `${title}\nsecond-line`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionRevealed(page, id);
    await waitForTermText(page, "FAKE-AGENT READY");

    await openRowMenu(page.locator(`[data-session-id="${id}"]`));
    await page.locator(".session-row-rename").click();
    const field = page.locator(".session-row-menu-panel .rename-input");
    await field.click();
    await page.keyboard.press("Control+A");
    await page.keyboard.insertText(pasted);
    // The field kept the line break — the property an `<input>` would have
    // destroyed before anything could be sent.
    await expect(field).toHaveValue(pasted);

    await page.locator(".rename-submit").click();

    // The supervisor's own refusal, for the newline it actually received —
    // rendered in the row's action-error line, which the open panel hosts.
    await expect(
      page.locator(`[data-session-id="${id}"] .action-error`),
    ).toContainText("control characters", { timeout: 15_000 });
    // The header still shows the real title while the refusal stands.
    await expect(page.locator(".titlebar .title")).toHaveText(title);
    await expect(field).toHaveValue(pasted);
    const fetched = await (await request.get(`/api/sessions/${id}`)).json();
    expect(fetched.title).toBe(title);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The third graceful-degradation bound, and the one a byte count cannot
// stand in for: every buffered frame costs an object regardless of how
// little it carries, so a peer sending a million one-byte frames stays far
// below the byte limit while the page pays for all of them.
//
// The frames here are tiny on purpose — the whole batch is a few hundred
// bytes against a multi-megabyte byte bound — so an implementation that
// counted only bytes would buffer them forever and fail this test on the
// reveal that never comes.
test("replay-degrades-on-chunks: crossing the frame bound flushes once, keeping every frame", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `replay-chunks-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await holdCatchUpFromNextLoad(page);
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);
    await waitForHeldCatchUp(page, "terminal");

    // Relative to what the real replay already holds, which is not a
    // number this suite can predict. The BYTE bound is deliberately left
    // at its production value: nothing below may cross it.
    const held = await replayRecord(page, "terminal");
    await setReplayLimits(page, "terminal", { bufferChunks: held.bufferedChunks + 8 });

    await injectReplayFrames(page, "terminal", {
      prefix: "CHUNK-",
      size: 16,
      count: 8,
      gapMs: 0,
    });
    const atLimit = await replayRecord(page, "terminal");
    expect(
      atLimit.bufferedChunks,
      "a batch landing exactly ON the frame limit is still buffered",
    ).toBe(held.bufferedChunks + 8);
    expect(atLimit.revealReason).toBeNull();
    expect(
      atLimit.bufferedBytes,
      "these frames are nowhere near the byte bound — the frame count is the only thing that can end this",
    ).toBeLessThan(atLimit.limits.bufferBytes);

    await injectReplayFrames(page, "terminal", { prefix: "CHUNK-CROSS-", size: 16 });
    const replay = await waitForReplayReveal(page, "terminal", 15_000);
    expect(replay.revealReason, "the frame bound is what ended it").toBe("chunks");
    expect(replay.writesWhileHidden, "still one write, however many frames").toBe(1);
    expect(replay.bufferedChunks, "the buffer is emptied by the flush").toBe(0);
    await expect(page.locator("#terminal")).toBeVisible();

    // Lossless: the first frame, the last one before the bound, and the
    // one that crossed it are all on screen.
    const flushed = await termText(page);
    expect(flushed).toContain("CHUNK-0");
    expect(flushed).toContain("CHUNK-7");
    expect(flushed).toContain("CHUNK-CROSS-0");
    expect(flushed).toContain("FAKE-AGENT READY");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The idle bound measures SILENCE, and an empty frame is not sound: a peer
// that cannot be caught out on going quiet — because it keeps sending
// nothing, forever — would hold this terminal hidden for as long as it
// liked. Empty frames must therefore not re-arm the watchdog.
//
// The discriminator is the shape of the test rather than an assertion in
// it: one real frame arms the (shortened) window, then empty frames arrive
// faster than that window for longer than it. An implementation that
// re-armed on them would push the deadline out with every one and never
// reveal, timing out below.
test("replay-idle-ignores-empty-frames: a stream of empty frames still counts as silence", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `replay-empty-${Date.now()}`;
  const idleMs = 1_200;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await holdCatchUpFromNextLoad(page);
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);
    await waitForHeldCatchUp(page, "terminal");

    // The short window is armed by the one real frame below, not by this
    // call — a limit change alone does not re-arm a running timer.
    await setReplayLimits(page, "terminal", { idleMs });
    await injectReplayFrames(page, "terminal", { prefix: "EMPTY-PROBE-", size: 64 });

    // Ten empty frames at 300 ms — three windows' worth of "activity" that
    // carries nothing.
    await injectEmptyFrames(page, "terminal", { count: 10, gapMs: 300 });

    const replay = await waitForReplayReveal(page, "terminal", 15_000);
    expect(
      replay.revealReason,
      "frames carrying nothing cannot keep a catch-up alive",
    ).toBe("idle");
    expect(replay.writesWhileHidden).toBe(1);
    expect(
      await termText(page),
      "the real frame that did arrive is written, whatever the empties did",
    ).toContain("EMPTY-PROBE-0");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A socket that never finishes connecting is the one state where revealing
// the terminal would be a lie: `term.onData` drops keystrokes on a socket
// that is not OPEN, so a normal-looking pane would swallow everything
// typed into it with nothing on screen to say why — the silent failure
// SPEC.md requires be reported instead of left to be inferred.
//
// So the watchdog's two expiries are two outcomes, and this pins the one
// that is NOT the ordinary degradation: the banner explains the terminal
// cannot carry input, and focus is deliberately left where the user put
// it rather than being placed into a dead pane.
//
// It also pins the watchdog's ARMING: a version that armed at `onopen`
// would never start a timer here at all — the open never comes — and this
// test would time out waiting for a banner that never appears.
test("replay-unconnected: a socket stuck connecting reports it instead of revealing a dead terminal", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `replay-unconnected-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await stuckWebSocketFromNextLoad(page);
    await holdCatchUpFromNextLoad(page, { idleMs: 1_500 });
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);

    await expect(page.locator("#term-banner")).toContainText("Not connected", {
      timeout: 20_000,
    });
    const replay = await replayRecord(page, "terminal");
    expect(
      replay.revealReason,
      "the never-connected ending is its own outcome, not the idle degradation",
    ).toBe("unconnected");
    // The placeholder is gone — the catch-up really did end — but nothing
    // pretends the terminal is usable: the banner is the message, and the
    // caret was not moved into a pane that cannot send.
    await expect(page.locator("#term-connecting")).toBeHidden();
    expect(
      await page.evaluate(() => {
        const active = document.activeElement;
        const terminal = document.getElementById("terminal");
        return !!active && !!terminal && terminal.contains(active);
      }),
      "focus must not be placed into a terminal that cannot carry input",
    ).toBe(false);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The lifecycle guard, from the outside: a catch-up whose mount is gone
// must not be able to act when its deferred ending finally runs. xterm.js
// runs a queued write callback even after `dispose()`, and the replacement
// mount is by then using the SAME DOM nodes — so a stale reveal would
// un-hide a terminal that is mid-catch-up on someone else's replay.
//
// Staged with the hold seam because the real race is a millisecond wide:
// the old mount's ending is held, the view is navigated away and back
// (which unmounts and remounts), and only then is the OLD mount's ending
// released.
test("replay-stale-mount: a torn-down island's deferred ending cannot touch its replacement", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `replay-stale-${Date.now()}`;
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await holdCatchUpFromNextLoad(page);
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await row.click();
    await waitForSessionMounted(page, id);
    await waitForHeldCatchUp(page, "terminal");
    // Captured before the teardown: after it, the registry points at the
    // replacement, and this reference is the only way back to the mount
    // under test.
    await page.evaluate(() => {
      (window as any).__staleIsland = (window as any).__farhelmIslands["terminal"].test;
    });

    // Bounce to the shared session and back: a full unmount, then a fresh mount
    // into the same element, whose own catch-up is held too.
    const sharedRow = sharedSessionRow(page);
    await expect(sharedRow).toHaveAttribute("data-session-id", /.+/, { timeout: 20_000 });
    const sharedId = (await sharedRow.getAttribute("data-session-id"))!;
    await sharedRow.click();
    await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
    await waitForSessionMounted(page, sharedId);
    await row.click();
    await waitForSessionMounted(page, id);
    await waitForHeldCatchUp(page, "terminal");

    // A list selection can commit before terminal reconciliation. Prove this
    // is a replacement before releasing a callback saved from the old mount.
    await expect.poll(() => page.evaluate(() =>
      (window as any).__farhelmIslands?.["terminal"]?.test !== (window as any).__staleIsland
    ), { timeout: 20_000 }).toBe(true);

    await page.evaluate(() => (window as any).__staleIsland.releaseCatchUp());

    const stale = await page.evaluate(() => (window as any).__staleIsland.replay);
    expect(
      stale.revealReason,
      "the torn-down mount's ending is inert, not merely harmless-looking",
    ).toBeNull();
    const replacement = await replayRecord(page, "terminal");
    expect(replacement.revealed, "the replacement is still catching up").toBe(false);
    await expect(page.locator("#terminal")).toBeHidden();
    await expect(page.locator("#term-connecting")).toBeVisible();
    expect(pageErrors, "a stale callback must not throw into the page").toEqual([]);

    // And the replacement still completes normally on its own release —
    // the guard stops the stale mount, not this one.
    await releaseCatchUp(page, "terminal");
    const replay = await waitForReplayReveal(page, "terminal");
    expect(replay.revealReason).toBe("marker");
    await expect(page.locator("#terminal")).toBeVisible();
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A rename field is an edit in progress, and the list re-renders for
// reasons the user did not cause: one failed listing read swaps the rows
// for an error line, unmounting every row and the open field with them.
// The draft has to survive that, or a transient network blip silently
// throws away what someone was in the middle of typing.
//
// The failure is injected the same way the confirming-state tests inject
// theirs — a 500 latched on while the field is open, and a feed
// notification to ask for the read that hits it — so what is exercised is
// the real reader's real failure path.
test("rename-draft-survives-a-failed-read: the field keeps what was typed", async ({
  page,
  request,
}) => {
  test.setTimeout(90_000);
  const title = `rename-read-fail-${Date.now()}`;
  const draft = `half-typed-${Date.now()}`;
  let id: string | undefined;

  let failing = false;
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    if (failing) {
      await fulfillAsHelm(route, {
        status: 500,
        contentType: "text/plain",
        body: "injected read failure",
      });
      return;
    }
    await route.continue();
  });

  const feed = await stubFeed(page);
  try {
    const created = await request.post("/api/sessions", {
      data: { cwd: "/tmp", invocation: FAKE_AGENT_INVOCATION, title },
    });
    expect(created.status(), await created.text()).toBe(200);
    id = (await created.json()).id as string;

    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row).toBeVisible({ timeout: 15_000 });
    await openRowMenu(row);
    await row.locator(".session-row-rename").click();
    await row.locator(".rename-input").fill(draft);

    // Only now does a listing read fail, strictly after the field holds
    // the draft.
    failing = true;
    feed.notify(2);
    // The rows really do go away — this is the transient state the draft
    // has to survive, not a bug the test is stepping around.
    await expect(page.locator(".status.error")).toBeVisible({ timeout: 10_000 });
    await expect(row).toHaveCount(0);

    // The next read succeeds and the row comes back, with the field still
    // open and still holding what was typed into it. The reader retries a
    // failed read on its own; the notification is belt and braces.
    failing = false;
    feed.notify(3);
    await expect(row).toHaveCount(1, { timeout: 10_000 });
    await expect(row.locator(".rename-input")).toHaveValue(draft);

    // And it still works: submitting from here renames the session.
    await row.locator(".rename-submit").click();
    await expect(row.locator(".session-title")).toHaveText(draft);
  } finally {
    await page.unroute(SESSION_LISTING);
    if (id) await cleanupSession(request, id);
  }
});


// The reveal happens when the replay lands, which is a moment the user did
// not choose — and by then they may be typing somewhere else. The rename
// field is the obvious victim: open it while a terminal is still catching
// up, start typing a title, and a reveal that took focus unconditionally
// would pull the caret into the pty mid-word and send the rest of the name
// to the agent.
//
// Held on purpose, because that is what makes the window wide enough to
// stage. Everything asserted here is about where focus and keystrokes
// ended up, not about the island's opinion of itself.
test("reveal-does-not-steal-focus: a rename field being typed into keeps focus and its text", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `focus-steal-${Date.now()}`;
  const typed = `renaming-while-connecting-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await holdCatchUpFromNextLoad(page);
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);
    await waitForHeldCatchUp(page, "terminal");

    // The user turns to the row menu's rename field (the one rename
    // surface) while the terminal is still catching up, and types a new
    // title — the reveal must not steal the caret out from under them.
    await openRowMenu(page.locator(`[data-session-id="${id}"]`));
    await page.locator(".session-row-rename").click();
    const field = page.locator(".session-row-menu-panel .rename-input");
    await expect(field).toBeFocused();
    await field.fill("");
    await page.keyboard.type(typed);

    await releaseCatchUp(page, "terminal");
    const replay = await waitForReplayReveal(page, "terminal");
    expect(replay.revealReason).toBe("marker");
    await expect(page.locator("#terminal")).toBeVisible();

    // Focus stayed where the user put it, with what they typed intact.
    await expect(field).toBeFocused();
    await expect(field).toHaveValue(typed);
    expect(
      await termText(page),
      "not one keystroke of the title may have reached the agent",
    ).not.toContain(typed);

    // And the keyboard still belongs to the field: more typing lands
    // there, not in the terminal that just appeared.
    await page.keyboard.type("-tail");
    await expect(field).toHaveValue(`${typed}-tail`);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});
