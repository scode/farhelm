// =====================================================================
// Terminal auto-reconnect (PLAN_M6.md item 7), and the client↔helm skew
// edge it depends on (item 6).
//
// Each test below covers a failure the others cannot catch — a lost socket
// coming back by itself and landing at the tail; a socket that dies
// SILENTLY being noticed at all; the ladder ending in visible background
// probing rather than in silence; the detaches that are DECISIONS staying
// where they are while the ones that are infrastructure keep recovering;
// a recovery never taking a session it was not asked to take; and
// unattended behavior switching itself off against a helm this page does
// not match.
//
// This file owns fresh, destructive sessions because it tests socket loss
// and takeover. Its own stack reset contains that damage.
//
// None of these needs a production seam to provoke, and that is worth
// stating because it looked like they would. Everything below drives the
// real stack: a close is a real close, a wedge is a real open socket with
// nothing coming back through it, a failing attempt is a real WebSocket
// handshake against a path this helm will not upgrade, and a takeover is a
// second browser context. The only thing tuned is TIME
// (`reconnectTimingsFromNextLoad`) — the shipped ladder spends thirty
// seconds before it even reaches the phase
// `retry-exhaustion-shows-reprobe-phase` is about, which is longer than
// this suite's whole per-test budget.
// =====================================================================


import { test, expect, type Page, type APIRequestContext } from "@playwright/test";
import { openFilterBar, stubFeed } from "./helpers/fleet";
import { cleanupSession, termText, waitForTermText } from "./helpers/term";
import {
  addTab,
  createTabSession,
  reconnectTimingsFromNextLoad,
  selectTerminal,
  sharedSessionRow,
  waitForReplayReveal,
  installTerminalSuiteHooks,
} from "./helpers/terminal-suite";

installTerminalSuiteHooks({ tabSweep: true });

/**
 * Open a session this test OWNS, on a fresh terminal, and hand back its id
 * for cleanup.
 *
 * The reconnect tests never destroy the shared "e2e-session". Every case
 * that mutates socket state creates a session it owns, and this helper is
 * the common path for those cases. Socket loss, wedging, and takeover are
 * destructive session states, so the file's own stack reset contains the
 * damage.
 *
 * The earlier monolithic spec had two tests that deliberately wrecked the
 * shared fixture: the multi-megabyte input test echoes 2 MiB of `a` through
 * the pane — enough to push `FAKE-AGENT READY` out of the 12,000-line tmux
 * history entirely — and the ctrl-c test then kills its agent, which is why
 * that test's own docs say nothing after it may depend on the shared
 * session. A test waiting for the banner then failed on the FIRST attempt
 * in a full-suite run while a targeted run passed against a fresh fixture.
 * (It failed exactly once per run, too, which made it look like a flake: a
 * failure restarts Playwright's worker, whose `beforeAll` recreates the
 * shared session, so the tests after it saw a clean one.)
 *
 * A session per test is also simply the right shape for what these
 * exercise: killing sockets, wedging them, and taking sessions over from a
 * second browser context are not things to do to a fixture everything else
 * shares.
 */
async function openOwnTerminal(
  page: Page,
  request: APIRequestContext,
  title: string,
): Promise<{ id: string; cwd: string }> {
  const session = await createTabSession(request, title);
  await openSessionTerminal(page, session.id);
  return session;
}

/**
 * Open an EXISTING session's terminal in this page — the second half of
 * every two-context test here, where the other context has to attach to
 * the same session to take it over.
 */
async function openSessionTerminal(page: Page, id: string) {
  await page.goto("/");
  const row = page.locator(`[data-session-id="${id}"]`);
  await expect(row).toBeVisible();
  await row.click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await waitForTermText(page, "FAKE-AGENT READY");
}


/**
 * Resolve once this island has been rebuilt — a DIFFERENT socket object
 * under the same element id.
 *
 * The identity check is what makes every assertion after it meaningful. A
 * reconnect tears the island down and mounts a fresh one, so polling on
 * anything else (the reveal record, the terminal's text) can be answered
 * by the island that just died, with its already-revealed state, before
 * the replacement exists at all.
 */
async function waitForRemount(page: Page, elementId: string, timeout = 20_000) {
  await expect
    .poll(
      () =>
        page.evaluate((el) => {
          const island = (window as any).__farhelmIslands?.[el];
          return !!island && island.ws !== (window as any).__farhelmPriorWs;
        }, elementId),
      { timeout, message: `waiting for ${elementId} to be rebuilt by a reconnect` },
    )
    .toBe(true);
}

/** Remember this island's current socket, so `waitForRemount` has a baseline. */
async function rememberSocket(page: Page, elementId: string) {
  await page.evaluate((el) => {
    (window as any).__farhelmPriorWs = (window as any).__farhelmIslands[el].ws;
  }, elementId);
}

/**
 * Hold this page's TERMINAL sockets off the helm — every reconnect attempt
 * fails to connect — until `admitTerminalSockets` puts the real constructor
 * back.
 *
 * What this pair buys is ORDERING, and the takeover test below cannot be
 * correct without it: the winner has to own the session BEFORE the loser's
 * next unattended attach reaches the helm, because that attach being
 * refused is the entire subject. The test used to buy that order with a
 * four-second backoff rung and the hope that a second browser context's
 * whole cold start — a fresh cache, the WASM bundle fetched and compiled
 * again — fit inside it. Traced on a quiet 4-core worker the winner needed
 * ~2.9s of that 4s; under WebKit in a loaded two-engine run the margin was
 * gone.
 *
 * When it went, what failed was not teardown. The loser's ladder simply
 * fired FIRST, reattached, went live, and was then displaced while
 * attached — a different, equally correct path that deliberately leaves the
 * island mounted (`recoverable()` refuses to recover a view that lost the
 * session, so nothing tears it down). Every assertion below still passed
 * except the empty-registry one, which is why the flake read as a WebKit
 * teardown bug rather than as the test having measured the wrong scenario.
 * A longer rung would only have moved the same coin flip.
 *
 * A withheld attempt is a REAL failed handshake against a route this helm
 * does not serve, so the ladder counts it exactly as it counts an
 * unreachable helm — none of the client's state machine is stubbed out.
 * Only terminal sockets are diverted, unlike the whole-`WebSocket` swap
 * `background-probes-recover-without-a-click` uses for its own purposes:
 * the feed socket (`/api/events`) keeps connecting, so the rest of the page
 * goes on behaving like a page.
 */
async function withholdTerminalSockets(page: Page) {
  await page.evaluate(() => {
    const Real = (window as any).WebSocket;
    (window as any).__realWebSocket = Real;
    // Two counters `expectTerminalSocketsWereWithheld` reads. Attempts
    // proves the ladder actually ran into the barrier (so a terminal URL
    // that stopped matching the predicate below could not slip past it
    // unnoticed); opens proves none of the diverted handshakes upgraded.
    // Their ABSENCE is what catches a caller that never installed this.
    (window as any).__withheldTerminalAttempts = 0;
    (window as any).__withheldTerminalOpens = 0;
    const Withheld: any = function (url: string, protocols?: any) {
      const terminal = String(url).includes("/term");
      if (terminal) {
        (window as any).__withheldTerminalAttempts += 1;
      }
      const socket = new Real(
        terminal ? `ws://${location.host}/api/farhelm-no-such-socket` : url,
        protocols,
      );
      if (terminal) {
        socket.addEventListener("open", () => {
          (window as any).__withheldTerminalOpens += 1;
        });
      }
      return socket;
    };
    for (const state of ["CONNECTING", "OPEN", "CLOSING", "CLOSED"]) {
      Withheld[state] = Real[state];
    }
    (window as any).WebSocket = Withheld;
  });
}

/**
 * Fail by NAME if the ordering barrier did not actually hold, rather than
 * letting a leak surface later as the assertion under test going wrong.
 *
 * That is not hypothetical bookkeeping: the flake being fixed here failed
 * exactly that way — a real, correct alternative path produced a mounted
 * island, and the empty-registry assertion took the blame for it. Three
 * ways of losing the barrier are caught: a socket that reached the helm
 * anyway counts an open; a ladder that never tried — because the terminal
 * URL stopped matching the withholding predicate, say, and went straight
 * to the helm as a real socket — counts no attempt at all; and a
 * `withholdTerminalSockets` call that was removed or reordered leaves both
 * counters undefined.
 */
async function expectTerminalSocketsWereWithheld(page: Page) {
  // Waits for the first diverted attempt rather than demanding it already
  // happened: a warm winner (Chromium on a quiet box) attaches inside the
  // loser's first rung, so at this instant the ladder may not have fired
  // yet. The rung guarantees it will within a second, which makes this a
  // bounded wait for a certainty, not a sprint.
  await expect
    .poll(() => page.evaluate(() => (window as any).__withheldTerminalAttempts), {
      timeout: 10_000,
      message: "setup: the loser's ladder must have tried (and been diverted) at least once while withheld",
    })
    .toBeGreaterThanOrEqual(1);
  expect(
    await page.evaluate(() => (window as any).__withheldTerminalOpens),
    "setup: the loser's terminal sockets must stay off the helm until the winner has attached",
  ).toBe(0);
}

/**
 * Let this page's terminal sockets reach the helm again
 * (`withholdTerminalSockets`) — through a counting pass-through rather than
 * the bare constructor, so the test can later prove the ladder went QUIET:
 * `terminalSocketsConstructedSince` reads how many terminal sockets were
 * built after a baseline, which is what "attaching nothing" means once the
 * sockets are reachable again.
 */
async function admitTerminalSockets(page: Page) {
  await page.evaluate(() => {
    const Real = (window as any).__realWebSocket;
    (window as any).__terminalSocketConstructions = 0;
    const Counting: any = function (url: string, protocols?: any) {
      if (String(url).includes("/term")) {
        (window as any).__terminalSocketConstructions += 1;
      }
      return new Real(url, protocols);
    };
    for (const state of ["CONNECTING", "OPEN", "CLOSING", "CLOSED"]) {
      Counting[state] = Real[state];
    }
    (window as any).WebSocket = Counting;
  });
}

/** How many terminal sockets this page has constructed since `admitTerminalSockets`. */
async function terminalSocketsConstructed(page: Page): Promise<number> {
  return page.evaluate(() => (window as any).__terminalSocketConstructions);
}

// A terminal whose socket dies gets itself back, unaided, and comes back
// where the session IS — not scrolling its history past again.
//
// Both halves are the milestone. The recovery is what removes the
// back-out-and-reopen dance a laptop sleep used to cost; landing at the
// tail is what keeps the recovery from being its own annoyance, and it
// comes for free precisely because a reconnect is an ordinary reattach
// riding M5's marker. Asserting it here is what would catch a future
// "reconnect" that took a shortcut around that path.
//
// The socket is closed from the page rather than by the server, and the
// distinction is invisible to the code under test: an island sees the same
// `onclose`, with no detach notice in front of it, either way. Producing a
// genuinely server-initiated close would mean killing the one helm this
// whole file runs against.
test("socket-killed-reconnects-and-lands-at-tail", async ({ page, request }) => {
  await reconnectTimingsFromNextLoad(page, { delaysMs: [50, 100, 200, 400, 800, 1600] });
  let ownId: string | undefined;
  try {
    const own = await openOwnTerminal(page, request, `socket-killed-reconnects-and-lands-at-ta-${Date.now()}`);
    ownId = own.id;

    // Real content in the scrollback, so "landed at the tail" is a claim
    // about a replay that actually had something to replay.
    await page.locator("#terminal").click();
    const marker = `reconnect-marker-${Date.now()}`;
    await page.keyboard.type(marker);
    await page.keyboard.press("Enter");
    await waitForTermText(page, `echo:${marker}`);

    await rememberSocket(page, "terminal");
    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());

    await waitForRemount(page, "terminal");
    const replay = await waitForReplayReveal(page, "terminal");
    expect(
      replay.revealReason,
      "a reconnect is an ordinary reattach: it ends on the marker, not on a degradation",
    ).toBe("marker");
    expect(
      replay.writesWhileHidden,
      "the whole replay reaches xterm.js as ONE write, exactly as a reopen does",
    ).toBe(1);
    expect(replay.revealedInWriteCallback).toBe(true);
    expect(
      replay.viewportAtTailOnReveal,
      "the first frame the user sees after a reconnect is the tail",
    ).toBe(true);

    // The recovered terminal is the same session, still carrying what was
    // there before — and live, which is the only thing the user actually
    // wanted back.
    await expect(page.locator("#terminal")).toBeVisible();
    expect(await termText(page)).toContain(`echo:${marker}`);
    await expect(page.locator("#term-connecting")).toBeHidden();
    await page.locator("#terminal").click();
    const after = `after-reconnect-${Date.now()}`;
    await page.keyboard.type(after);
    await page.keyboard.press("Enter");
    await waitForTermText(page, `echo:${after}`);
  } finally {
    if (ownId) await cleanupSession(request, ownId);
  }
});

// A socket can die without ever closing — a sleeping laptop, a NAT that
// dropped the flow — and that is the case this milestone exists for: the
// terminal looks connected, accepts keystrokes, and delivers none of them.
// Only the heartbeat catches it.
//
// The wedge is produced at the HARNESS level, with no production seam: the
// island's own `onmessage` is replaced by one that drops everything, so the
// socket stays genuinely open at both ends — the helm still holds the
// attachment, still answers the ping — while nothing reaches the island.
// That is as close to the real failure as a test can get; the real one
// differs only in that the far end has also stopped listening.
//
// Nothing else about the island is touched, which is what makes the
// heartbeat the thing under test: `onclose` is still terminal.js's own, so
// the recovery below can only have been started by the probe timing out.
test("wedged-socket-detected", async ({ page, request }) => {
  await reconnectTimingsFromNextLoad(page, {
    delaysMs: [50, 100, 200, 400, 800, 1600],
    heartbeatIdleMs: 300,
    heartbeatTimeoutMs: 300,
  });
  let ownId: string | undefined;
  try {
    const own = await openOwnTerminal(page, request, `wedged-socket-detected-${Date.now()}`);
    ownId = own.id;
    await rememberSocket(page, "terminal");

    const wedged = await page.evaluate(() => {
      const ws = (window as any).__farhelmIslands["terminal"].ws;
      (window as any).__wedgedWs = ws;
      ws.onmessage = () => {};
      // The closing handshake is neutered as well, and that is the sharper
      // half of the wedge rather than extra realism for its own sake. A
      // socket whose transport is gone can sit in CLOSING forever — the
      // close frame has nobody to answer it — so a recovery that waited for
      // its own `close()` to come back as an `onclose` would never start, on
      // exactly the dead transport this check exists to catch. With `close`
      // doing nothing at all, only a heartbeat that takes the transition
      // DIRECTLY can recover; a version that closes and waits hangs here.
      ws.close = () => {};
      return ws.readyState === WebSocket.OPEN;
    });
    expect(wedged, "the socket must be OPEN when it is silenced — that is the whole case").toBe(
      true,
    );

    await waitForRemount(page, "terminal");
    const replay = await waitForReplayReveal(page, "terminal");
    expect(
      replay.revealReason,
      "the wedge enters the SAME recovery a close does, ending on an ordinary marker",
    ).toBe("marker");
    expect(replay.viewportAtTailOnReveal).toBe(true);

    // And the recovered terminal really carries input again, which is the
    // failure the heartbeat exists to end: typing that goes nowhere.
    await page.locator("#terminal").click();
    const after = `after-wedge-${Date.now()}`;
    await page.keyboard.type(after);
    await page.keyboard.press("Enter");
    await waitForTermText(page, `echo:${after}`);
  } finally {
    if (ownId) await cleanupSession(request, ownId);
  }
});

// When the active window is spent, the terminal says so and keeps trying
// — it never goes quiet, and it never stops offering the control that
// skips the wait.
//
// Failing attempts are real handshakes: every new socket is pointed at a
// path this helm answers with the UI's own index.html instead of a
// protocol upgrade, so each attempt fails exactly the way it would against
// a helm that is not there — no seam, no stubbed WebSocket, no faked error.
// Restoring the real constructor and pressing "reconnect now" then proves
// the manual control is a working way out of the slow phase rather than
// decoration.
test("retry-exhaustion-shows-reprobe-phase", async ({ page, request }) => {
  test.setTimeout(90_000);
  await reconnectTimingsFromNextLoad(page, {
    delaysMs: [20, 20, 20, 20, 20, 20],
    probeIntervalMs: 300,
  });
  let ownId: string | undefined;
  try {
    const own = await openOwnTerminal(page, request, `retry-exhaustion-${Date.now()}`);
    ownId = own.id;

    await page.evaluate(() => {
      const Real = (window as any).WebSocket;
      (window as any).__realWebSocket = Real;
      // A constructor returning an object overrides `this`, so this is a
      // real WebSocket — aimed somewhere the helm will not upgrade.
      const Doomed: any = function (_url: string, protocols?: any) {
        return new Real(`ws://${location.host}/api/farhelm-no-such-socket`, protocols);
      };
      for (const state of ["CONNECTING", "OPEN", "CLOSING", "CLOSED"]) {
        Doomed[state] = Real[state];
      }
      (window as any).WebSocket = Doomed;
    });
    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());

    const surface = page.locator("#term-connecting");
    await expect(surface).toHaveAttribute("data-reconnect-phase", "probing", {
      timeout: 20_000,
    });
    await expect(
      surface,
      "past the active window the terminal promises to keep checking on its own",
    ).toContainText("retrying every");
    // The attempt count kept climbing across the phase boundary rather than
    // restarting: six rungs, then probes.
    const attempt = Number(await surface.getAttribute("data-reconnect-attempt"));
    expect(attempt).toBeGreaterThan(6);

    await expect(
      page.locator(".terminal-reconnect-now"),
      "the way out stays on screen for as long as this lasts",
    ).toBeVisible();

    // Deliberately NOT clicked here, and the split is worth stating: what
    // this test owns is the ladder ENDING in a visible, self-sustaining
    // probe phase. That the control can actually be pressed is
    // `manual-reconnect-is-offered-during-the-first-rung`, which holds a
    // rung open so the press is a real interaction rather than a race
    // against a 300ms repaint cycle no user ever sees (the shipped probe
    // interval is thirty seconds); that a probe recovers the terminal with
    // nobody pressing anything is `background-probes-recover-without-a-click`.
    // Clicking here as well bought no coverage and cost a hang under load.
  } finally {
    if (ownId) await cleanupSession(request, ownId);
  }
});

// A client displaced by a takeover must NOT come back on its own. The
// carve-out is the whole reason auto-reconnect is scoped to transport loss:
// two clients that each reattach after being displaced take the session
// from each other forever, and the user watches their terminal alternate.
//
// The takeover here is real — a second browser context attaching to the
// same session — so this is the actual carve-out, not a simulation of one.
// The ladder is tuned FAST on purpose: a reconnect that was going to
// happen would have happened several times over inside the window this
// test then waits out.
test("takeover-does-not-bounce-back", async ({ browser, page, request }) => {
  await reconnectTimingsFromNextLoad(page, { delaysMs: [20, 20, 20, 20, 20, 20] });
  let ownId: string | undefined;
  try {
    const own = await openOwnTerminal(page, request, `takeover-does-not-bounce-back-${Date.now()}`);
    ownId = own.id;
    await rememberSocket(page, "terminal");

    const second = await browser.newContext();
    const page2 = await second.newPage();
    try {
      await openSessionTerminal(page2, ownId!);

      // The displaced client keeps the surface SPEC.md gives it: the reason,
      // and the deliberate way back.
      await expect(page.locator("#term-banner")).toContainText("Detached", {
        timeout: 10_000,
      });
      await expect(page.locator(".banner-reclaim")).toBeVisible();

      // Nothing is recovering, and nothing is going to: no reconnect
      // surface, no manual control, and — after long enough for the whole
      // tuned ladder to have run twice — the same dead island it was left
      // with.
      await expect(page.locator("#term-connecting")).toHaveCount(1);
      await expect(page.locator("#term-connecting")).toBeHidden();
      await expect(page.locator(".terminal-reconnect-now")).toHaveCount(0);
      await page.waitForTimeout(1_000);
      expect(
        await page.evaluate(
          () =>
            (window as any).__farhelmIslands["terminal"].ws
              === (window as any).__farhelmPriorWs,
        ),
        "the displaced client must still be holding the socket it lost, not a fresh one",
      ).toBe(true);

      // And the winner is unbothered: input still round-trips, which is what
      // a bounce-back would have taken away from it.
      await page2.locator("#terminal").click();
      const typed = `winner-keeps-it-${Date.now()}`;
      await page2.keyboard.type(typed);
      await page2.keyboard.press("Enter");
      await waitForTermText(page2, `echo:${typed}`, 10_000);
  } finally {
    await second.close();
  }
  } finally {
    if (ownId) await cleanupSession(request, ownId);
  }
});

// The manual control is offered from the FIRST failure onward, not only
// once the ladder is spent (PLAN_M6.md item 7, user decision 2026-08-04):
// a user who knows their VPN just came back should never have to sit
// through a backoff they can see on screen.
//
// The first rung is held open deliberately — a long first delay is the only
// way to be INSIDE the active window long enough to assert against it — and
// the control is then pressed, which is the half that matters: a button
// that is visible but wired to nothing would pass a visibility assertion
// and fail a user.
test("manual-reconnect-is-offered-during-the-first-rung", async ({ page, request }) => {
  await reconnectTimingsFromNextLoad(page, {
    delaysMs: [30_000, 30_000, 30_000, 30_000, 30_000, 30_000],
  });
  let ownId: string | undefined;
  try {
    const own = await openOwnTerminal(page, request, `manual-reconnect-is-offered-during-the-f-${Date.now()}`);
    ownId = own.id;
    await rememberSocket(page, "terminal");
    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());

    const surface = page.locator("#term-connecting");
    await expect(surface).toHaveAttribute("data-reconnect-phase", "retrying");
    await expect(surface).toHaveAttribute("data-reconnect-attempt", "1");
    await expect(
      surface,
      "the active window names which attempt this is and how long the wait is",
    ).toContainText("attempt 1 of 6");
    const manual = page.locator(".terminal-reconnect-now");
    await expect(manual).toBeVisible();

    // Pressed well inside a thirty-second rung: if this did nothing, the
    // remount below could not happen for half a minute.
    await manual.click();
    await waitForRemount(page, "terminal", 10_000);
    const replay = await waitForReplayReveal(page, "terminal");
    expect(replay.revealReason).toBe("marker");
    await expect(surface).toBeHidden();
    expect(await termText(page)).toContain("FAKE-AGENT READY");
  } finally {
    if (ownId) await cleanupSession(request, ownId);
  }
});

// A takeover that lands while a client is ON THE RECONNECT LADDER, holding
// no attachment, is the race auto-reconnect creates and must not lose: the
// takeover notice reaches that client nowhere, and its next automatic
// attach would reuse the same lease and silently displace the new owner —
// an eviction with nobody behind it.
//
// Two independent mechanisms are asserted, because each covers the other's
// gap. The supervisor REFUSES an unattended attach while another lease
// holds the session (`if_unowned`), and the browser stops attempting at
// all once it learns it was taken over. The observable consequence is the
// one that matters to a user: the winner keeps typing, uninterrupted.
//
// The ORDER — winner attached, then the loser's attempt refused — is
// established by `withholdTerminalSockets` rather than by outrunning a
// backoff rung; see that helper for the flake that taught us the
// difference.
test("takeover-during-backoff-does-not-steal-the-session", async ({ browser, page, request }) => {
  // A cadence, not a barrier: with terminal sockets withheld below, the
  // rung only decides how soon after the winner attaches the loser's first
  // reachable attempt fires. The probe interval matches so a winner that
  // takes longer than the ladder is indistinguishable from one that does
  // not — nothing here depends on which phase the recovery is in.
  await reconnectTimingsFromNextLoad(page, {
    delaysMs: [1_000, 1_000, 1_000, 1_000, 1_000, 1_000],
    probeIntervalMs: 1_000,
  });
  let ownId: string | undefined;
  try {
    const own = await openOwnTerminal(page, request, `takeover-during-backoff-does-not-steal-t-${Date.now()}`);
    ownId = own.id;
    // Something distinctive on screen, so "kept its snapshot" is a claim
    // about THIS session's content rather than about a pane being non-empty.
    await page.locator("#terminal").click();
    const marker = `snapshot-${Date.now()}`;
    await page.keyboard.type(marker);
    await page.keyboard.press("Enter");
    await waitForTermText(page, `echo:${marker}`);
    // Withheld BEFORE the close, so there is no window at all in which the
    // ladder could reach the helm ahead of the winner below.
    await withholdTerminalSockets(page);
    await rememberSocket(page, "terminal");
    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());
    await expect(page.locator(".terminal-reconnect-now")).toBeVisible();

    const second = await browser.newContext();
    const page2 = await second.newPage();
    try {
      // The winner attaches while the loser is on the ladder holding
      // nothing — however long its cold start takes, since the loser cannot
      // reach the helm meanwhile.
      await openSessionTerminal(page2, ownId!);

      // Only now can the loser attach at all, so its next rung is the first
      // attempt the helm ever sees from it — and it is unattended, which is
      // what makes it `if_unowned` and refusable.
      await expectTerminalSocketsWereWithheld(page);
      await admitTerminalSockets(page);

      // The loser's next attempt fires, is refused, and lands it in the
      // state it was actually in: taken over, with the deliberate way back.
      await expect(page.locator("#term-banner")).toContainText("Detached: another client attached", {
        timeout: 20_000,
      });
      await expect(page.locator(".banner-reclaim")).toBeVisible();
      await expect(
        page.locator(".terminal-reconnect-now"),
        "a client that has been taken over stops trying: the ladder is over",
      ).toHaveCount(0);

      // The winner was never disturbed — no bounce-back, and no attachment
      // taken from under it while it was typing.
      await page2.locator("#terminal").click();
      const typed = `winner-undisturbed-${Date.now()}`;
      await page2.keyboard.type(typed);
      await page2.keyboard.press("Enter");
      await waitForTermText(page2, `echo:${typed}`, 15_000);

      // And it stays that way: more than two further rungs elapse with the
      // loser CONSTRUCTING no terminal socket at all — counted in the page,
      // so an attempt that was built, refused, and torn down between two
      // registry samples cannot hide. The wait runs on the loser page's own
      // event loop rather than the runner's clock: under WebKit throttling
      // the runner can finish a `waitForTimeout` before the page's 1s rung
      // timers have fired even once, which would make the negative check
      // below vacuous.
      const afterRefusal = await terminalSocketsConstructed(page);
      await page.evaluate(() => new Promise((resolve) => setTimeout(resolve, 2_500)));
      expect(
        (await terminalSocketsConstructed(page)) - afterRefusal,
        "the retired ladder must construct no further terminal socket once sockets are admitted",
      ).toBe(0);
      // The registry is EMPTY here, and that is the recovery being properly
      // retired rather than a terminal being abandoned: the refused
      // attempt's island was torn down and the screen the user had was put
      // back in its place (`cancelReconnect`'s `"restore"`), which is what
      // the content assertion below actually reads. An island left mounted
      // would mean a live attachment nobody asked for.
      expect(
        await page.evaluate(() => Object.keys((window as any).__farhelmIslands ?? {})),
        "nothing is attached: the refused attempt was torn down and no replacement opened",
      ).toEqual([]);

      // SPEC.md: a displaced client keeps a non-live snapshot. The refused
      // attempt tore down and rebuilt this island, so without the held screen
      // the user would be staring at an empty pane under the banner.
      //
      // Read from the DOM rather than through the xterm buffer helper every
      // other test uses, and the difference is the point: the buffer helper
      // reads the island's published terminal, and there is deliberately no
      // island here any more. What is on screen is a rendered snapshot with
      // no attachment behind it — which is exactly what "non-live" means.
      await expect(page.locator("#terminal")).toBeVisible();
      await expect(
        page.locator("#terminal"),
        "the screen the user had must still be on screen after the refusal",
      ).toContainText(`echo:${marker}`);
  } finally {
    await second.close();
  }
  } finally {
    if (ownId) await cleanupSession(request, ownId);
  }
});

// Background probes recover WITHOUT anyone pressing anything — the
// overnight promise in SPEC.md's Errors section, and the half
// `retry-exhaustion-shows-reprobe-phase` cannot prove because it clicks.
//
// The connection is restored while the page is already in the probing
// phase, and nothing else is touched: the next scheduled probe has to be
// what brings the terminal back.
test("background-probes-recover-without-a-click", async ({ page, request }) => {
  await reconnectTimingsFromNextLoad(page, {
    delaysMs: [20, 20, 20, 20, 20, 20],
    probeIntervalMs: 400,
  });
  let ownId: string | undefined;
  try {
    const own = await openOwnTerminal(page, request, `background-probes-recover-without-a-clic-${Date.now()}`);
    ownId = own.id;

    await page.evaluate(() => {
      const Real = (window as any).WebSocket;
      (window as any).__realWebSocket = Real;
      const Doomed: any = function (_url: string, protocols?: any) {
        return new Real(`ws://${location.host}/api/farhelm-no-such-socket`, protocols);
      };
      for (const state of ["CONNECTING", "OPEN", "CLOSING", "CLOSED"]) {
        Doomed[state] = Real[state];
      }
      (window as any).WebSocket = Doomed;
    });
    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());

    const surface = page.locator("#term-connecting");
    await expect(surface).toHaveAttribute("data-reconnect-phase", "probing", {
      timeout: 20_000,
    });
    await rememberSocket(page, "terminal");

    // The network comes back. No click, no navigation, no sync — only the
    // ladder's own next probe.
    await page.evaluate(() => {
      (window as any).WebSocket = (window as any).__realWebSocket;
    });

    await waitForRemount(page, "terminal", 20_000);
    const replay = await waitForReplayReveal(page, "terminal");
    expect(replay.revealReason).toBe("marker");
    await expect(surface).toBeHidden();
    await page.locator("#terminal").click();
    const typed = `probe-recovered-${Date.now()}`;
    await page.keyboard.type(typed);
    await page.keyboard.press("Enter");
    await waitForTermText(page, `echo:${typed}`);
  } finally {
    if (ownId) await cleanupSession(request, ownId);
  }
});

// Navigating away cancels a pending recovery outright.
//
// A timer that survived the view would fire minutes later, mount an island
// into a pane that now belongs to a different session, and attach it under
// a lease nothing on screen is using — the same class of zombie the
// pre-M4 `mountWhenReady` retry loop produced, reintroduced one layer up.
// The wait here is longer than the rung deliberately: the assertion is
// that the timer FIRED HARMLESSLY or was cancelled, not that the test
// looked before it could have run.
test("leaving-a-session-cancels-a-pending-reconnect", async ({ page, request }) => {
  await reconnectTimingsFromNextLoad(page, {
    delaysMs: [1_500, 1_500, 1_500, 1_500, 1_500, 1_500],
  });
  let ownId: string | undefined;
  try {
    const own = await openOwnTerminal(page, request, `leaving-a-session-cancels-a-pending-reco-${Date.now()}`);
    ownId = own.id;
    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());
    await expect(page.locator(".terminal-reconnect-now")).toBeVisible();

    // Leaving = selecting the shared session; its own island now owns the
    // singleton slot, so the zombie this pins would show up as the
    // `terminal` island's socket pointing back at the DEPARTED session —
    // a stale timer remounting under a lease nothing on screen is using.
    await sharedSessionRow(page).click();
    await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    // Past the pending rung, with room to spare.
    await page.waitForTimeout(2_500);
    const islands = await page.evaluate(() => {
      const map = (window as any).__farhelmIslands ?? {};
      return Object.keys(map).map((key) => ({ key, url: map[key].ws?.url ?? "" }));
    });
    expect(
      islands.map((island) => island.key),
      "only the shared session's own island may be mounted",
    ).toEqual(["terminal"]);
    expect(
      islands[0].url,
      "a recovery whose view went away must not have reattached the departed session",
    ).not.toContain(ownId);
  } finally {
    if (ownId) await cleanupSession(request, ownId);
  }
});

// A page restored from the back/forward cache is LIVE again, with sockets
// the engine closed on the way in — and those closes were correctly
// ignored at the time, because the page was navigating away. Without a
// reset, such a tab could never reconnect again: the exact papercut this
// milestone removes, reintroduced through the one path where the page does
// not reload.
//
// MANUAL-VERIFICATION GAP, stated plainly: this drives the `pagehide` /
// `pageshow(persisted)` pair directly rather than a real bfcache round
// trip. Playwright cannot reliably force a document into the back/forward
// cache — eligibility depends on engine heuristics (an open WebSocket
// alone is often disqualifying) and there is no API to demand it — so what
// is pinned here is the LATCH LOGIC the restore depends on, with the
// engine's own caching left to manual testing. The events themselves are
// exactly the ones an engine dispatches, so a regression in the handling
// still fails here; a regression in eligibility would not.
test("bfcache-restore-lets-a-terminal-reconnect-again", async ({ page, request }) => {
  await reconnectTimingsFromNextLoad(page, { delaysMs: [50, 100, 200, 400, 800, 1600] });
  let ownId: string | undefined;
  try {
    const own = await openOwnTerminal(page, request, `bfcache-restore-lets-a-terminal-reconnec-${Date.now()}`);
    ownId = own.id;
    await rememberSocket(page, "terminal");

    // Enter the cache: the page stops being a page, and its socket dies —
    // a close nothing acts on, because the page is on its way out.
    await page.evaluate(() => {
      window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true }));
      (window as any).__farhelmIslands["terminal"].ws.close();
    });
    await page.waitForTimeout(300);
    expect(
      await page.evaluate(
        () =>
          (window as any).__farhelmIslands["terminal"].ws === (window as any).__farhelmPriorWs,
      ),
      "a page on its way out must not reconnect: nothing is watching, and a reload would race it",
    ).toBe(true);

    // Restored. The document is live, the socket is not, and nothing else
    // will ever bring that news.
    await page.evaluate(() => {
      window.dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true }));
    });
    await waitForRemount(page, "terminal", 15_000);
    const replay = await waitForReplayReveal(page, "terminal");
    expect(replay.revealReason).toBe("marker");
    await page.locator("#terminal").click();
    const typed = `restored-${Date.now()}`;
    await page.keyboard.type(typed);
    await page.keyboard.press("Enter");
    await waitForTermText(page, `echo:${typed}`);
  } finally {
    if (ownId) await cleanupSession(request, ownId);
  }
});

// A latched build mismatch REVOKES unattended reconnect — including for
// terminals that are already open and ladders already running — while the
// manual control keeps working.
//
// This is the browser half of PROTOCOL_VERSION 9's argument. `if_unowned`
// is the field that stops an automatic attach from stealing a session, and
// a helm that predates it drops the field and displaces anyway. Between
// helm and supervisor a hello refusal makes that pairing impossible; on
// this edge there is no hello, so the build stamp is the handshake — and
// when it disagrees, the page must stop attaching on its own rather than
// send a request whose safety clause the far end may ignore.
//
// The revocation is applied MID-RECOVERY on purpose: a tab that survives a
// helm rollback learns about it from an ordinary REPLY, seconds after its
// ladder started, and a permission captured when the terminal mounted
// would be exactly the stale one.
//
// Which reply that is, is the one thing this test has to arrange for
// itself. The stamp rides on whatever the page happens to read, and a
// healthy page reads only when the feed says something changed — so the
// feed is stubbed and the notification below is what sends the session view
// to the helm and brings the rolled-back stamp back with it.
test("latched-skew-revokes-automatic-reconnect", async ({ page, request }) => {
  await reconnectTimingsFromNextLoad(page, {
    delaysMs: [800, 800, 800, 800, 800, 800],
  });
  const feed = await stubFeed(page);
  let ownId: string | undefined;
  try {
    const own = await openOwnTerminal(page, request, `latched-skew-revokes-automatic-reconnect-${Date.now()}`);
    ownId = own.id;
    await feed.waitForConnection(1);
    feed.notify(1);
    await rememberSocket(page, "terminal");

    // A recovery that is genuinely IN PROGRESS when the build changes: the
    // attempts are aimed at a path this helm will not upgrade, so the ladder
    // keeps climbing instead of succeeding on its first rung and leaving
    // nothing for the revocation to reach.
    await page.evaluate(() => {
      const Real = (window as any).WebSocket;
      (window as any).__realWebSocket = Real;
      const Doomed: any = function (_url: string, protocols?: any) {
        return new Real(`ws://${location.host}/api/farhelm-no-such-socket`, protocols);
      };
      for (const state of ["CONNECTING", "OPEN", "CLOSING", "CLOSED"]) {
        Doomed[state] = Real[state];
      }
      (window as any).WebSocket = Doomed;
    });
    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());
    const surface = page.locator("#term-connecting");
    await expect(surface).toHaveAttribute("data-reconnect-phase", "retrying");

    // Now the helm "changes build" under the open tab. The next reply
    // latches the mismatch, which withdraws the permission this ladder was
    // armed on.
    // The catch is load-hardening, not decoration: this handler stays
    // installed while the page keeps polling, and the unroute below (or test
    // teardown, on a failure elsewhere) can catch a request mid-handler —
    // `route.fulfill` then throws "Route is already handled!" and fails the
    // test from the background (seen twice under full-suite load, chromium
    // and webkit). On the unroute path Playwright's fallback has already
    // continued the request, so this handler simply no longer owns it; at
    // teardown its outcome is moot. Either way the race is safe to
    // suppress — but ONLY the race: any other failure here must still fail
    // the test, which is why the catch matches the known lifecycle errors
    // and rethrows the rest. The same fetch-then-fulfill shape elsewhere in
    // this suite is only exposed where a matching request can be in flight
    // at unroute or teardown time; this stamp handler matches EVERY api
    // call, which is why it is the one that keeps getting caught.
    await page.route("**/api/**", async (route) => {
      try {
        const response = await route.fetch();
        await route.fulfill({
          response,
          headers: { ...response.headers(), "x-farhelm-build": "9999.0.0-rolled-back" },
        });
      } catch (e) {
        const raced =
          e instanceof Error
          && /Route is already handled|Target page, context or browser has been closed|Test ended/.test(
            e.message,
          );
        if (!raced) throw e;
      }
    });

    // And the notification is what makes that reply happen: the session view
    // re-reads its own detail, and the stamp comes back with it.
    feed.notify(2);
    await expect(page.locator(".build-skew")).toBeVisible({ timeout: 15_000 });
    await expect(
      surface,
      "the surface says why nothing is happening on its own, rather than going quiet",
    ).toHaveAttribute("data-reconnect-phase", "manual-only", { timeout: 15_000 });
    await expect(surface).toContainText("different builds");

    // And it stays that way: several rungs' worth of time passes with no
    // attach attempted at all.
    await rememberSocket(page, "terminal").catch(() => {});
    await page.waitForTimeout(2_500);
    expect(
      await page.evaluate(() => {
        const island = (window as any).__farhelmIslands?.["terminal"];
        return island ? island.ws === (window as any).__farhelmPriorWs : true;
      }),
      "an unattended attach against a helm that may ignore if_unowned must not happen",
    ).toBe(true);

    // The manual control is unaffected: pressing it is a user asking, which
    // is the ordinary displacing attach every client has always made.
    await page.unroute("**/api/**");
    await page.evaluate(() => {
      (window as any).WebSocket = (window as any).__realWebSocket;
    });
    await page.locator(".terminal-reconnect-now").click();
    await waitForRemount(page, "terminal", 15_000);
    const replay = await waitForReplayReveal(page, "terminal");
    expect(replay.revealReason).toBe("marker");
    expect(await termText(page)).toContain("FAKE-AGENT READY");
  } finally {
    if (ownId) await cleanupSession(request, ownId);
  }
});

// A reconnect attempt that ATTACHES NOTHING — the socket opens, the helm
// accepts the upgrade, and then it sits there — is a failed attempt, not a
// success.
//
// The stuck state this pins against is specific: an idle catch-up with
// neither marker nor bytes used to end the phase, reveal an empty
// terminal, clear the recovery surface, and leave the controller with no
// timer armed. Nothing would ever have retried it. The terminal has to
// stay behind its surface, and the ladder has to keep climbing — which is
// also why the attempt counter is read: a reset would mean each silent
// attach restarted the backoff from the bottom, hammering forever.
test("silent-attach-is-a-failed-attempt-not-a-recovery", async ({ page, request }) => {
  await reconnectTimingsFromNextLoad(page, {
    delaysMs: [50, 300, 300, 300, 300, 300],
  });
  let ownId: string | undefined;
  try {
    const own = await openOwnTerminal(page, request, `silent-attach-is-a-failed-attempt-not-a-${Date.now()}`);
    ownId = own.id;

    // Replacement sockets connect for real and then say nothing: the frames
    // the helm sends are dropped before the island can see them, which is
    // exactly what a helm that accepted the upgrade and stalled looks like.
    await page.evaluate(() => {
      const Real = (window as any).WebSocket;
      (window as any).__realWebSocket = Real;
      const Mute: any = function (url: string, protocols?: any) {
        const ws = new Real(url, protocols);
        let handler: any = null;
        Object.defineProperty(ws, "onmessage", {
          get: () => handler,
          set: (fn) => {
            handler = fn;
          },
          configurable: true,
        });
        return ws;
      };
      for (const state of ["CONNECTING", "OPEN", "CLOSING", "CLOSED"]) {
        Mute[state] = Real[state];
      }
      (window as any).WebSocket = Mute;
    });
    // A short catch-up watchdog so the silence is judged in a test's budget
    // rather than in five seconds per attempt.
    await page.evaluate(() => {
      (window as any).__farhelmTestReplay = { idleMs: 250 };
    });
    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());

    const surface = page.locator("#term-connecting");
    await expect(surface).toHaveAttribute("data-reconnect-phase", "retrying");
    // Past several silent attempts: the surface is still up, the terminal is
    // still hidden behind it, and the ladder has ADVANCED rather than reset.
    await expect
      .poll(
        async () => Number(await surface.getAttribute("data-reconnect-attempt")),
        { timeout: 20_000, message: "each silent attach must count as a failure" },
      )
      .toBeGreaterThan(2);
    await expect(surface).toBeVisible();
    await expect(page.locator(".terminal-reconnect-now")).toBeVisible();
    expect(
      await page.evaluate(() => Object.keys((window as any).__farhelmIslands ?? {}).length > 0),
      "the island is being rebuilt each attempt; what must not happen is the recovery ENDING",
    ).toBe(true);

    // Un-mute: the very next attempt attaches for real and the recovery ends.
    await page.evaluate(() => {
      (window as any).WebSocket = (window as any).__realWebSocket;
    });
    await expect(surface).toBeHidden({ timeout: 20_000 });
    await expect(page.locator("#terminal")).toBeVisible();
    expect(await termText(page)).toContain("FAKE-AGENT READY");
  } finally {
    if (ownId) await cleanupSession(request, ownId);
  }
});

// A detach notice that is NOT a decision — the helm losing its supervisor,
// a host that went away — is transport loss one layer up, so the ladder
// must carry on through it to a successful reattachment.
//
// The implementation this pins against treated ANY detach notice as a
// veto, which stopped the ladder on the first rung: each failed attempt
// provokes its own notice, so a blanket cancel meant a terminal that could
// never come back from an outage it was explicitly told about — the
// opposite of SPEC.md's "comes back overnight" promise.
test("a-non-decision-detach-keeps-the-ladder-climbing", async ({ page, request }) => {
  test.setTimeout(90_000);
  const title = `nondecision-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    await reconnectTimingsFromNextLoad(page, {
      delaysMs: [100, 100, 100, 100, 100, 100],
    });
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    // Attempts are aimed at a tab id no supervisor has ever heard of, so
    // each one is REFUSED with the supervisor's own words — a detach
    // notice that explains itself and is not a decision.
    await page.evaluate(() => {
      const Real = (window as any).WebSocket;
      (window as any).__realWebSocket = Real;
      const Wrong: any = function (url: string, protocols?: any) {
        // Both terminal routes, because an AUTOMATIC attempt goes to the
        // non-displacing one (`/term/unowned`) and a manual one to the
        // base path — a rewrite that matched only the base would leave the
        // ladder attaching successfully and prove nothing.
        return new Real(
          String(url).replace(
            /\/term(\/unowned)?\?/,
            (_m: string, unowned: string | undefined) =>
              `/term${unowned ?? ""}?tab=00000000-0000-4000-8000-00000000dead&`,
          ),
          protocols,
        );
      };
      for (const state of ["CONNECTING", "OPEN", "CLOSING", "CLOSED"]) {
        Wrong[state] = Real[state];
      }
      (window as any).WebSocket = Wrong;
    });
    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());

    // The refusals arrive, and the recovery survives them: the attempt
    // counter climbs past several notices.
    const surface = page.locator("#term-connecting");
    await expect
      .poll(
        async () => Number(await surface.getAttribute("data-reconnect-attempt")),
        {
          timeout: 20_000,
          message: "an explained outage is still an outage: the ladder must keep going",
        },
      )
      .toBeGreaterThan(2);

    // The outage ends, and the terminal comes back on its own.
    await page.evaluate(() => {
      (window as any).WebSocket = (window as any).__realWebSocket;
    });
    await expect(surface).toBeHidden({ timeout: 20_000 });
    await expect(page.locator("#terminal")).toBeVisible();
    await waitForTermText(page, "FAKE-AGENT READY", 20_000);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The MANUAL control displaces, unlike every automatic attempt.
//
// The two halves of the same rule: an unattended attach must never take a
// session (it carries `if_unowned`), and a press must always be able to,
// because a press is a user asking — the same thing opening the session
// from the list does. An implementation that leaked `if_unowned` into the
// manual path would leave a user pressing a button that politely refuses
// to do the one thing they asked for.
test("manual-reconnect-takes-the-session-back", async ({ browser, page, request }) => {
  await reconnectTimingsFromNextLoad(page, {
    delaysMs: [30_000, 30_000, 30_000, 30_000, 30_000, 30_000],
  });
  let ownId: string | undefined;
  try {
    const own = await openOwnTerminal(page, request, `manual-reconnect-takes-the-session-back-${Date.now()}`);
    ownId = own.id;
    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());
    await expect(page.locator(".terminal-reconnect-now")).toBeVisible();

    const second = await browser.newContext();
    const page2 = await second.newPage();
    try {
      // Another client takes the session while this one waits out its rung.
      await openSessionTerminal(page2, ownId!);
      await rememberSocket(page, "terminal");

      // The press takes it back, and the other client is told so — which is
      // the ordinary visible takeover, not a bounce-back: nothing here
      // happened without someone asking for it.
      await page.locator(".terminal-reconnect-now").click();
      await waitForRemount(page, "terminal", 20_000);
      const replay = await waitForReplayReveal(page, "terminal");
      expect(replay.revealReason).toBe("marker");
      await expect(page2.locator("#term-banner")).toContainText(
        "Detached: another client attached",
        { timeout: 15_000 },
      );

      // And the session really is this page's again: typing round-trips.
      await page.locator("#terminal").click();
      const typed = `took-it-back-${Date.now()}`;
      await page.keyboard.type(typed);
      await page.keyboard.press("Enter");
      await waitForTermText(page, `echo:${typed}`, 15_000);
  } finally {
    await second.close();
  }
  } finally {
    if (ownId) await cleanupSession(request, ownId);
  }
});

// A revocation must disarm the heartbeat that is ALREADY ARMED, at both
// of its deadlines — not merely decline to arm the next one.
//
// The hole this pins shut was an ordering one: with the capability guard
// ahead of the clear, a withdrawal left the running timer alone, and that
// timer went on to ping a helm that cannot answer and then tear down a
// perfectly healthy socket. The two cases are separate because the timers
// are: one fires the probe, the other decides the socket is dead.
//
// The socket is deliberately left ALIVE and quiet (unlike the skew test
// above, which closes it first) — a live, silent socket is the only state
// in which an armed heartbeat can do damage. The deadline under test has to
// fall AFTER the revocation lands, or the test would be watching the
// heartbeat work rather than watching it be withdrawn.
//
// That ordering used to rest on a poll: the mismatch latched on whatever
// read came next, which was never more than an interval away. M6.75 removed
// the polls, so the latch now waits for a read the FEED asks for — and on a
// quiet fleet that is an unbounded wait, while the heartbeat's deadline runs
// on regardless. Both variants below therefore drive the latch themselves
// (stub the feed, rewrite the stamp, notify), which turns "the revocation
// lands before the deadline" from an assumption about the shared stack into
// a fact about this test.
//
// The probe count is likewise taken only once the revocation is ON SCREEN. A
// probe sent before that is a probe sent to a helm this page still trusted —
// correct behavior, not the failure under test — and counting it was how a
// slow engine turned a legitimate probe into a failure.
for (const when of [
  {
    name: "before the probe is sent",
    // Probe due well after the mismatch can latch.
    timings: { heartbeatIdleMs: 6_000, heartbeatTimeoutMs: 2_000 },
    awaitProbe: false,
    settleMs: 8_000,
  },
  {
    name: "while the answer is outstanding",
    // Probe goes out almost at once; the answer stays due long enough that
    // the revocation — which the test triggers, so it lands in a round trip
    // rather than whenever the fleet next stirs — arrives while it is still
    // outstanding.
    timings: { heartbeatIdleMs: 800, heartbeatTimeoutMs: 12_000 },
    awaitProbe: true,
    settleMs: 13_000,
  },
]) {
  test(`skew-revocation-disarms-the-heartbeat (${when.name})`, async ({ page, request }) => {
    test.setTimeout(90_000);
    const pings: string[] = [];
    page.on("websocket", (ws) => {
      ws.on("framesent", (frame) => {
        if (typeof frame.payload === "string" && frame.payload.includes("\"ping\"")) {
          pings.push(frame.payload);
        }
      });
    });
    await reconnectTimingsFromNextLoad(page, {
      ...when.timings,
      delaysMs: [200, 200, 200, 200, 200, 200],
    });
    const feed = await stubFeed(page);
    let ownId: string | undefined;
    try {
      const own = await openOwnTerminal(
        page,
        request,
        `skew-revocation-${Date.now()}`,
      );
      ownId = own.id;
      /**
       * Hand the page a revision however it can be reached: on the socket it
       * has open now, and on every socket it opens later.
       *
       * An ungreeted stub socket is torn down by the client's own handshake
       * deadline and reopened a ladder rung later, so a bare `notify` is a
       * bet that the socket seen a moment ago is still there — and losing
       * that bet throws rather than flakes.
       */
      const greet = (revision: number) => {
        feed.notifyOnConnect(revision);
        if (feed.openSockets() > 0) feed.notify(revision);
      };
      // Healthy from here on, so nothing reads until this test says so — and
      // so the one read that matters below is unmistakably the one it asked
      // for. A handshake is an HTTP read; it says nothing on the terminal
      // socket and so leaves the heartbeat's idle window alone.
      await feed.waitForConnection(1);
      greet(1);
      await rememberSocket(page, "terminal");

      // Silence the island so the idle window actually elapses: the socket
      // stays open and healthy, it simply has nothing to say to this island.
      await page.evaluate(() => {
        (window as any).__farhelmIslands["terminal"].ws.onmessage = () => {};
      });
      if (when.awaitProbe) {
        await expect
          .poll(() => pings.length, { timeout: 15_000, message: "the probe must go out first" })
          .toBeGreaterThan(0);
      }

      await page.route("**/api/**", async (route) => {
        const response = await route.fetch();
        await route.fulfill({
          response,
          headers: { ...response.headers(), "x-farhelm-build": "9999.0.0-rolled-back" },
        });
      });
      // The read that carries the rolled-back stamp, asked for rather than
      // waited for: this view re-reads its own session on a notification, and
      // the reply latches the mismatch a round trip later.
      greet(2);
      await expect(page.locator(".build-skew")).toBeVisible({ timeout: 15_000 });

      // Sampled HERE, with the revocation on screen: everything counted from
      // now on was sent by a page that had already given up on this helm,
      // which is the only thing this test is entitled to complain about.
      const pingsBefore = pings.length;

      // Past the deadline the withdrawal was supposed to disarm.
      await page.waitForTimeout(when.settleMs);
      expect(
        pings.length,
        "no probe may be sent to a helm this page has stopped trusting",
      ).toBe(pingsBefore);
      expect(
        await page.evaluate(() => {
          const island = (window as any).__farhelmIslands?.["terminal"];
          return island
            ? island.ws === (window as any).__farhelmPriorWs
              && island.ws.readyState === WebSocket.OPEN
            : false;
        }),
        "a withdrawn heartbeat may not tear down the socket it was watching",
      ).toBe(true);
      await expect(page.locator("#term-connecting")).toBeHidden();
    } finally {
      if (ownId) await cleanupSession(request, ownId);
    }
  });
}

// An ordinary `sync()` — a tab selected, focus moved, a poll noticing a
// sibling — must not disturb a recovery in progress.
//
// Two failures in one, and both were live: rescheduling on every sync
// RESET the current backoff deadline, so a user clicking around could
// postpone their own recovery indefinitely; and a sync landing during an
// attempt could arm a second attempt against it. The first is what this
// measures — the recovery has to land on the schedule it was given,
// regardless of what else the view was doing.
test("view-changes-do-not-postpone-a-recovery", async ({ page, request }) => {
  test.setTimeout(90_000);
  const title = `sync-churn-${Date.now()}`;
  let id: string | undefined;
  try {
    // One long rung: any reset is worth a full second and shows up as a
    // recovery that never lands inside the window below.
    await reconnectTimingsFromNextLoad(page, {
      delaysMs: [1_500, 1_500, 1_500, 1_500, 1_500, 1_500],
    });
    const session = await createTabSession(request, title);
    id = session.id;
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    // A second terminal, so selecting between them produces real
    // desired-set changes — the syncs this test is about.
    const tabId = await addTab(page, 0);

    await rememberSocket(page, "terminal");
    const lostAt = Date.now();
    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());
    await expect(page.locator("#term-connecting")).toHaveAttribute(
      "data-reconnect-phase",
      "retrying",
    );

    // Churn the view while the agent terminal waits out its rung: each of
    // these is a `sync()` with a different desired set.
    for (let i = 0; i < 4; i++) {
      await selectTerminal(page, i % 2 === 0 ? "agent" : tabId);
      await page.waitForTimeout(250);
    }

    await waitForRemount(page, "terminal", 20_000);
    const recoveredAfterMs = Date.now() - lostAt;
    expect(
      recoveredAfterMs,
      "the recovery must land on the schedule it was given, not on one the view kept resetting",
    ).toBeLessThan(4_000);
    const replay = await waitForReplayReveal(page, "terminal");
    expect(replay.revealReason).toBe("marker");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A STALL detach that lands while a terminal is recovering leaves it
// detached — and it STAYS detached through whatever the reconciler does
// next.
//
// SPEC.md is absolute that a stalled viewer comes back "because someone
// asks", and the recovery machinery created a way for nobody to ask: the
// decision tears the island down, and the very next desired-set change
// would find the element unmounted and reattach it. That is a bounce-back
// arriving through the reconciler instead of through the ladder, and it is
// the same violation either way.
test("a-stall-during-recovery-stays-detached-across-syncs", async ({ page, request }) => {
test.setTimeout(90_000);
const title = `stall-tombstone-${Date.now()}`;
let id: string | undefined;
try {
  await reconnectTimingsFromNextLoad(page, {
    delaysMs: [100, 100, 100, 100, 100, 100],
  });
  const session = await createTabSession(request, title);
  id = session.id;
  await page.goto("/");
  await page.locator(`[data-session-id="${id}"]`).click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await waitForTermText(page, "FAKE-AGENT READY");

  // Something on screen, so "kept its screen" is a claim about content.
  await page.locator("#terminal").click();
  const marker = `stalled-${Date.now()}`;
  await page.keyboard.type(marker);
  await page.keyboard.press("Enter");
  await waitForTermText(page, `echo:${marker}`);

  // The recovery starts, and every attempt is answered with the STALL
  // detach the supervisor would send — the decision, delivered while
  // this client is mid-ladder and holding no socket of its own.
  await page.evaluate(() => {
    const Real = (window as any).WebSocket;
    (window as any).__realWebSocket = Real;
    const Stalling: any = function (url: string, protocols?: any) {
      const ws = new Real(url, protocols);
      let handler: any = null;
      Object.defineProperty(ws, "onmessage", {
        get: () => handler,
        set: (fn) => {
          handler = fn;
          // Deliver the decision as soon as the island is listening.
          setTimeout(() => {
            if (!handler) return;
            handler({
              data: JSON.stringify({
                type: "detached",
                reason: "terminal stopped consuming output (stalled)",
              }),
            });
          }, 20);
        },
        configurable: true,
      });
      return ws;
    };
    for (const state of ["CONNECTING", "OPEN", "CLOSING", "CLOSED"]) {
      Stalling[state] = Real[state];
    }
    (window as any).WebSocket = Stalling;
  });
  await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());

  await expect(page.locator("#term-banner")).toContainText("stalled", { timeout: 20_000 });
  await expect(
    page.locator(".terminal-reconnect-now"),
    "a stall is a decision: the ladder is over",
  ).toHaveCount(0);

  // Now provoke the reconciler: opening a tab changes the desired set,
  // which is exactly the sync that used to reattach.
  // Sockets behave normally again from here: what is under test next is
  // the RECONCILER, not another decision.
  await page.evaluate(() => {
    (window as any).WebSocket = (window as any).__realWebSocket;
  });
  await addTab(page, 0);
  await page.waitForTimeout(1_000);

  expect(
    await page.evaluate(() => Object.keys((window as any).__farhelmIslands ?? {})),
    "the agent terminal must not have reattached: nobody asked",
  ).not.toContain("terminal");
  await expect(
    page.locator("#terminal"),
    "and it still shows the screen it was detached with",
  ).toContainText(`echo:${marker}`);
  await expect(page.locator("#term-banner")).toContainText("stalled");

  // Someone finally ASKS: leaving and reopening the session is the
  // user's own attach, and it must both work and leave nothing of the
  // old screen behind — a kept screen that outlived its element would
  // stack a dead terminal under the live one, and leak an xterm
  // instance per detach.
  await sharedSessionRow(page).click();
  await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
  await page.locator(`[data-session-id="${id}"]`).click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await waitForTermText(page, "FAKE-AGENT READY", 20_000);
  expect(
    await page.evaluate(() => document.querySelectorAll("#terminal .xterm-rows").length),
    "exactly one terminal in the element: the reopened one",
  ).toBe(1);
} finally {
  if (id) await cleanupSession(request, id);
}
});

// A frame QUEUED before the evidence-free idle gave up must not confirm
// the recovery it arrives after.
//
// The race: the idle expiry commits `socketEnded()` and starts closing,
// and a marker or a byte already in the queue then calls the
// attach-is-real path — retiring the ladder and revealing a CLOSING
// socket, whose own close is suppressed by the first-ending guard. The
// terminal is then stranded: no surface, no timer, nothing that will ever
// try again. What must happen instead is that the late evidence is
// ignored and the ladder carries on to a real recovery.
test("late-evidence-cannot-confirm-a-closing-socket", async ({ page, request }) => {
await reconnectTimingsFromNextLoad(page, {
  delaysMs: [50, 250, 250, 250, 250, 250],
});
let ownId: string | undefined;
try {
  const own = await openOwnTerminal(page, request, `late-evidence-cannot-confirm-a-closing-s-${Date.now()}`);
  ownId = own.id;

  // Replacement sockets hold every frame back until just after the
  // catch-up watchdog has given up on them, then deliver the lot — the
  // queued-frame race, made deterministic.
  await page.evaluate(() => {
    const Real = (window as any).WebSocket;
    (window as any).__realWebSocket = Real;
    const Late: any = function (url: string, protocols?: any) {
      const ws = new Real(url, protocols);
      let handler: any = null;
      const held: any[] = [];
      let releasing = false;
      Object.defineProperty(ws, "onmessage", {
        get: () => handler,
        set: (fn) => {
          handler = fn;
          if (releasing) return;
          releasing = true;
          setTimeout(() => {
            for (const ev of held.splice(0)) if (handler) handler(ev);
          }, 320);
        },
        configurable: true,
      });
      Real.prototype.addEventListener.call(ws, "message", (ev: any) => held.push(ev));
      return ws;
    };
    for (const state of ["CONNECTING", "OPEN", "CLOSING", "CLOSED"]) {
      Late[state] = Real[state];
    }
    (window as any).WebSocket = Late;
    (window as any).__farhelmTestReplay = { idleMs: 200 };
  });
  await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());

  // The ladder must still be running after the late frames land: a
  // confirmed corpse would leave no surface at all.
  const surface = page.locator("#term-connecting");
  await expect(surface).toHaveAttribute("data-reconnect-phase", "retrying");
  await expect
    .poll(async () => Number(await surface.getAttribute("data-reconnect-attempt")), {
      timeout: 20_000,
      message: "late evidence must not retire the ladder",
    })
    .toBeGreaterThan(1);

  // And a real recovery still happens once the frames flow normally,
  // ending in a terminal that carries input.
  await page.evaluate(() => {
    (window as any).WebSocket = (window as any).__realWebSocket;
    delete (window as any).__farhelmTestReplay;
  });
  await expect(surface).toBeHidden({ timeout: 20_000 });
  await page.locator("#terminal").click();
  const typed = `after-late-${Date.now()}`;
  await page.keyboard.type(typed);
  await page.keyboard.press("Enter");
  await waitForTermText(page, `echo:${typed}`);
} finally {
  if (ownId) await cleanupSession(request, ownId);
}
});

// The rollback window: a helm that went BACK to a build without the
// non-displacing route, in the seconds before the page's next reply can
// latch the mismatch.
//
// The build stamp alone cannot close this — it is only as fresh as the last
// reply the page happened to read, and the first retry fires in half a
// second. What closes it is that the automatic attach asks by CHOOSING A
// PATH the old helm does not serve: the handshake fails, the attempt counts
// as a failure, and the ladder carries on until the stamp catches up. The
// property under test is negative and absolute: no displacing attach may
// occur in that window.
//
// The window is STAGED here rather than raced: the ladder is allowed to
// climb until it has really made an attempt, and only then does a feed
// notification send the page to the helm for the reply that latches the
// mismatch. Against the real feed the window's length is whatever the fleet
// happens to be doing, which is a different test on every run — and a short
// one is a test that proves nothing, since a ladder that never attempted
// has no displacing attach to have avoided.
test("a-rolled-back-helm-gets-no-automatic-attach", async ({ page, request }) => {
await reconnectTimingsFromNextLoad(page, {
  delaysMs: [200, 200, 200, 200, 200, 200],
});
const feed = await stubFeed(page);
let ownId: string | undefined;
try {
  const own = await openOwnTerminal(page, request, `a-rolled-back-helm-gets-no-automatic-att-${Date.now()}`);
  ownId = own.id;
  await feed.waitForConnection(1);
  feed.notify(1);

  // The old helm, simulated where the socket is CONSTRUCTED rather than
  // through `page.routeWebSocket` (which only governs sockets created
  // after navigation, and this page is already open): the unattended route
  // is aimed at a path nothing serves, which is precisely what a helm
  // predating it does — its static fallback answers with the UI's own
  // index.html, and no WebSocket comes of it. Ordinary attaches pass
  // through untouched, so a displacing attach would SUCCEED and be
  // counted, which is the failure this test exists to catch.
  await page.evaluate(() => {
    const Real = (window as any).WebSocket;
    (window as any).__attachCounts = { unowned: 0, ordinary: 0 };
    const OldHelm: any = function (url: string, protocols?: any) {
      const asked = String(url);
      const unowned = asked.includes("/term/unowned");
      (window as any).__attachCounts[unowned ? "unowned" : "ordinary"] += 1;
      return new Real(
        unowned ? asked.replace("/term/unowned", "/term/unowned-not-served") : asked,
        protocols,
      );
    };
    for (const state of ["CONNECTING", "OPEN", "CLOSING", "CLOSED"]) {
      OldHelm[state] = Real[state];
    }
    (window as any).WebSocket = OldHelm;
  });
  await page.route("**/api/**", async (route) => {
    const response = await route.fetch();
    await route.fulfill({
      response,
      headers: { ...response.headers(), "x-farhelm-build": "0.0.0-rolled-back" },
    });
  });

  await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());
  // The rollback window itself: the ladder is genuinely climbing against a
  // helm whose stamp this page has not read yet.
  await expect
    .poll(() => page.evaluate(() => (window as any).__attachCounts.unowned), {
      timeout: 15_000,
      message: "a ladder that never attempted proves nothing about what it attempted with",
    })
    .toBeGreaterThan(0);

  // And now the stamp catches up, because the page reads a reply: several
  // rungs' worth of time then passes in the manual-only state.
  feed.notify(2);
  await expect(page.locator(".build-skew")).toBeVisible({ timeout: 15_000 });
  await page.waitForTimeout(1_500);

  const counts = await page.evaluate(() => (window as any).__attachCounts);
  expect(
    counts.ordinary,
    "not one DISPLACING attach may be made on this page's own initiative",
  ).toBe(0);
  expect(
    counts.unowned,
    "the attempts that did happen asked on the route an old helm does not serve",
  ).toBeGreaterThan(0);
  await expect(page.locator("#term-connecting")).toHaveAttribute(
    "data-reconnect-phase",
    "manual-only",
  );
} finally {
  if (ownId) await cleanupSession(request, ownId);
}
});

// A tab left open across a helm upgrade says so, instead of failing in
// ways nothing on screen explains (PLAN_M6.md item 6's client↔helm skew
// edge).
//
// Route-intercepted, because the real cause — rebuilding and restarting
// the helm mid-suite — is not something this file may do to the stack every
// other test shares. What is under test is entirely this UI's half: that it
// READS the stamp the helm really does send (asserted here against the live
// header before anything is rewritten), compares it, and says something
// actionable when the two disagree.
test("client-helm-skew-prompts-reload", async ({ page, request }) => {
  const live = await request.get("/api/sessions");
  expect(
    live.headers()["x-farhelm-build"],
    "the helm stamps every reply; the check below is meaningless without it",
  ).toBeTruthy();

  await page.route("**/api/**", async (route) => {
    const response = await route.fetch();
    await route.fulfill({
      response,
      headers: {
        ...response.headers(),
        "x-farhelm-build": "9999.0.0-from-a-newer-helm",
      },
    });
  });

  await page.goto("/");
  await openFilterBar(page);
  const notice = page.locator(".build-skew");
  await expect(notice).toBeVisible({ timeout: 15_000 });
  await expect(notice).toContainText("9999.0.0-from-a-newer-helm");
  await expect(notice, "the remedy is the point of the line").toContainText("reload");
  // Not a blocking prompt: the app underneath keeps working, which is the
  // deliberate posture — a stale bundle mostly works, and taking the app
  // away from someone mid-session would be the bigger harm.
  await expect(sharedSessionRow(page)).toBeVisible();

  // The verdict is LATCHED: a later reply that agrees does not clear it.
  // Replies race and land in completion order, so a mismatch clearable by
  // a fresh-looking observation is a mismatch clearable by a STALE one —
  // the slow reply that left the old helm arriving after a fast one from
  // the new build. Certainty here is worth more than tidiness, and the
  // remedy it keeps recommending (reload) stays correct either way.
  //
  // The agreeing replies have to be USER-DRIVEN, and that is the withdrawal
  // rule showing through (SPEC_impl.md's version-and-skew section): a skewed
  // page withdraws every UNATTENDED behavior — the feed, the fallback poll,
  // the heartbeat, automatic reconnect — while "anything the user explicitly
  // asks for keeps working". Nothing reads on its own here, so a test that
  // waited for a read would wait forever; each live filter action below is a
  // person asking and issues an attended read.
  //
  // So these two waits assert the explicit half of that rule as much as they
  // stage the latch: a page that stood its EXPLICIT reads down under skew
  // would hang here rather than fail an assertion, which is the shape this
  // exact regression took on WebKit. Two live edits, two agreeing replies.
  await page.unroute("**/api/**");
  for (let i = 0; i < 2; i++) {
    const landed = page.waitForResponse(
      (response) =>
        response.request().method() === "GET" && /\/api\/sessions/.test(response.url()),
      { timeout: 30_000 },
    );
    await page.locator(".filter-include-archived").setChecked(i === 0);
    await landed;
  }
  await expect(
    notice,
    "a mismatch, once seen, survives until the page is actually reloaded",
  ).toBeVisible();
});

// A helm that reports NO build stamp is skewed too — it predates the
// stamp, which means it predates the rest of this milestone's vocabulary
// including the terminal heartbeat. Treating that as agreement (which this
// first did) leaves a user on an interface that half-works with nothing on
// screen about why.
//
// The heartbeat gate is the half with teeth, and it is asserted on the
// terminal itself: against such a helm the browser must send NO pings,
// because an unrecognized message is ignored by contract — the probe would
// go unanswered on a perfectly healthy socket and be read as death, every
// fifteen seconds, forever.
test("absent-build-stamp-is-skew-and-silences-the-heartbeat", async ({ page, request }) => {
  await page.route("**/api/**", async (route) => {
    const response = await route.fetch();
    const headers = { ...response.headers() };
    delete headers["x-farhelm-build"];
    await route.fulfill({ response, headers });
  });
  // Heartbeat timings a working page would fire many times over inside the
  // observation window below.
  await reconnectTimingsFromNextLoad(page, {
    heartbeatIdleMs: 100,
    heartbeatTimeoutMs: 100,
  });

  const pings: string[] = [];
  page.on("websocket", (ws) => {
    ws.on("framesent", (frame) => {
      if (typeof frame.payload === "string" && frame.payload.includes("\"ping\"")) {
        pings.push(frame.payload);
      }
    });
  });

  let ownId: string | undefined;
  try {
    const own = await openOwnTerminal(page, request, `absent-build-stamp-is-skew-and-silences-${Date.now()}`);
    ownId = own.id;
    await expect(page.locator(".build-skew")).toBeVisible();
    await expect(page.locator(".build-skew")).toContainText("predates this interface");

    // Long enough for a dozen idle windows to have elapsed.
    await page.waitForTimeout(1_500);
    expect(
      pings,
      "a helm that never showed a build stamp does not speak ping; probing it would manufacture the failure the probe exists to detect",
    ).toEqual([]);
    // And the terminal is still perfectly usable — the gate silences the
    // probe, not the session.
    await page.locator("#terminal").click();
    const typed = `no-heartbeat-${Date.now()}`;
    await page.keyboard.type(typed);
    await page.keyboard.press("Enter");
    await waitForTermText(page, `echo:${typed}`);
  } finally {
    if (ownId) await cleanupSession(request, ownId);
  }
});

// The heartbeat is IDLE-GATED: a terminal with output flowing costs
// nothing extra, because every frame from the helm restarts the window.
//
// The failure this catches is an unconditional periodic probe, which
// works, passes the wedge test, and quietly puts a message on every
// terminal in a fleet every fifteen seconds forever. With the window tuned
// to a fraction of a second, a periodic version sends a probe every few
// echoes; the gated one sends none until the typing stops.
//
// STEADY LOW-VOLUME traffic rather than the flood fixture, and that choice
// is a finding rather than a preference: a multi-megabyte producer trips
// flow control, and a paused stream is GENUINELY silent — the heartbeat
// firing there is correct behavior, not the periodic bug this is looking
// for. An earlier version of this test used the flood and "passed" only
// because it sampled once, early; strengthened to span the whole stream it
// failed, having caught the pause rather than a regression. Echoes keep
// the socket honestly busy with no volume at all.
test("heartbeat-stays-idle-under-output", async ({ page, request }) => {
  const pings: string[] = [];
  page.on("websocket", (ws) => {
    ws.on("framesent", (frame) => {
      if (typeof frame.payload === "string" && frame.payload.includes("\"ping\"")) {
        pings.push(frame.payload);
      }
    });
  });
  // The window is a second and a half, not a fraction of one, and the
  // margin is deliberate: under a loaded suite a single echo round trip
  // can take a few hundred milliseconds, and a window tighter than that
  // makes the socket GENUINELY idle between keystrokes — at which point a
  // probe is correct behavior and the test would be failing the feature
  // for working. The evidence loop below keeps the traffic flowing for the
  // required duration rather than guessing how many round trips will take
  // that long on a particular engine or machine.
  await reconnectTimingsFromNextLoad(page, {
    heartbeatIdleMs: 1_500,
    heartbeatTimeoutMs: 10_000,
  });
  let ownId: string | undefined;
  try {
    const own = await openOwnTerminal(page, request, `heartbeat-stays-idle-under-output-${Date.now()}`);
    ownId = own.id;
    await page.locator("#terminal").click();

    // Round trips, each well inside the idle window, spanning several times
    // the window in total: an unconditional probe fires repeatedly across
    // this, a gated one not at all.
    const started = performance.now();
    const evidenceDeadline = started + 30_000;
    let echoes = 0;
    while (performance.now() - started <= 3_000) {
      if (performance.now() >= evidenceDeadline) {
        throw new Error("the terminal stalled before it could establish the busy-window evidence");
      }
      const line = `busy-${echoes++}`;
      await page.keyboard.type(line);
      await page.keyboard.press("Enter");
      await waitForTermText(page, `echo:${line}`);
      if (performance.now() >= evidenceDeadline) {
        throw new Error("the terminal stalled before it could establish the busy-window evidence");
      }
    }
    const busyForMs = performance.now() - started;
    expect(
      busyForMs,
      "the busy window has to span several idle windows for its silence to mean anything",
    ).toBeGreaterThan(3_000);
    expect(
      pings.length,
      "an active terminal must cost nothing: its own bytes are the liveness proof",
    ).toBe(0);

    // Once the traffic stops, the same window expires and the probe fires —
    // which is what makes the assertion above a statement about GATING
    // rather than about a heartbeat that never runs at all.
    await expect
      .poll(() => pings.length, {
        timeout: 15_000,
        message: "a quiet socket is probed",
      })
      .toBeGreaterThan(0);
  } finally {
    if (ownId) await cleanupSession(request, ownId);
  }
});
