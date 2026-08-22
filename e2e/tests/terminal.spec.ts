// The M1 acceptance suite at the browser level (PLAN_M1.md criterion 5),
// grown by PLAN_M2.md step 7 to cover the list view and navigation, and by
// step 8 to cover the create dialog and per-row stop/delete actions: these
// tests cover output rendering, input round-trip, reconnect replay,
// resize, last-attach-wins takeover, the session list, the create/stop/
// delete UI, and the list/terminal navigation lifecycle — all against a
// real helm, supervisor, tmux, and fake agent, with a handful of
// deliberate exceptions that intercept `page.route` instead: the
// truncation banner (pinning a ~500-session-cap reply without actually
// creating hundreds of sessions), the Unknown-status confirm wording
// (provoking that status needs an old-shaped peer, not anything this
// build's own supervisor can produce — see that test's own docs), the
// stop/delete failure-surfacing tests (forcing failures a healthy stack
// would never hand back on its own), two confirming-state poll tests
// (a synthetic marker-carrying listing to prove a refetch's RESULT
// reached the DOM, and a synthetic one-shot 500 to prove a failed refetch
// doesn't clear `confirming` — neither is reachable by driving the real
// stack alone), and the host-state tests at the end of this file that
// synthesize a `/api/hosts` reply — the local supervisor being down, the
// two identity states, the full phase table, and a host vanishing from the
// create dialog's selector: producing any of them for real would mean
// stopping the developer's own supervisor, wiping and reinstalling one
// mid-suite, or registering seven hosts that do not exist (see the
// multi-host section's own header). Every other test drives the real stack
// end to end, and since PLAN_M6.md item 5 the stack it drives is a two-host
// FLEET.
//
// Assertions read the xterm.js BUFFER, not the DOM: the DOM renderer
// materializes only viewport rows, so scrolled-off content (exactly what
// replay tests care about) never appears in .xterm-rows. The buffer is
// the semantic truth of what the terminal holds.
//
// ## Tests that used to wait for a poll
//
// A dozen tests here were written against the four periodic loops M6.75
// removed (PLAN_M6_75.md item 6): they changed an intercepted fixture and
// waited for the next listing, detail or hosts poll to pick it up. Nothing
// polls a healthy page any more, so each of them now takes control of the
// INVALIDATION instead, through `helpers/fleet`'s feed stub — the same
// convention feed.spec.ts, filters.spec.ts and m6-5-debts.spec.ts use:
// stub the socket, hand the page a handshake so the feed is healthy (and
// therefore silent), change the fixture, then notify and let the page's own
// re-read pick it up. The handful whose subject genuinely IS the fallback
// cadence say so in their own docs and make the feed unhealthy on purpose.
//
// The feed stub and terminal-island helpers are imported because both are
// contracts: the former defines "the feed is healthy"; the latter owns
// `window.__farhelmTerm`, `term.buffer.active`, and readiness globals. A
// genuinely one-off snippet still stays local with the test that needs it.
import { test, expect, Page, APIRequestContext } from "@playwright/test";
import {
  createSession,
  openFilterBar,
  pinAutoSelect,
  openHostsPanel,
  openRowMenu,
  stubFeed,
} from "./helpers/fleet";
import path from "node:path";
import fs from "node:fs";
// The multi-host tests at the very end of this file kill and restart the
// harness's "remote" supervisor to drive a host through unreachable and
// back — the one thing in this suite that has to reach behind the API,
// because no API makes a host go away.
import { ChildProcess, spawn } from "node:child_process";
import net from "node:net";
import { cleanupSession, fillCreateForm, termText, waitForTermText } from "./helpers/term";
import {
  addTab,
  cleanUpSessionsTitled,
  createTabSession,
  disableReconnectFromNextLoad,
  FAKE_AGENT_INVOCATION,
  findSessionIdByTitle,
  fulfillAsHelm,
  helmBuild,
  installTerminalSuiteHooks,
  islandText,
  LIVE_BADGE,
  LIVE_STATES,
  openTerminal,
  reconnectTimingsFromNextLoad,
  replayRecord,
  rowByTitle,
  runInShell,
  selectTerminal,
  sharedSessionRow,
  shellMarker,
  waitForIslandMounted,
  waitForIslandText,
  waitForReplayReveal,
} from "./helpers/terminal-suite";

installTerminalSuiteHooks({ tabSweep: true });



// First pixels: the whole stack standing up and putting an agent's output
// on screen. Everything below assumes this works, so when the suite goes
// red this is the test that says whether the problem is the stack or the
// behavior under test.
test("renders the session and the agent's TUI output", async ({ page }) => {
  await openTerminal(page);
  await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
  await waitForTermText(page, "fake-agent starting");
  // The visible viewport is real DOM text too — the DOM renderer is what
  // makes these tests semantic rather than pixel-diffing.
  await expect(page.locator(".xterm-rows")).toContainText("FAKE-AGENT READY");
});

// The list view itself (PLAN_M2.md step 7): title, cwd, invocation, and a
// truthful status badge per row, sourced from the same GET /api/sessions
// every other test exercises indirectly through openTerminal. cwd and
// invocation are checked against the API's OWN listing rather than mere
// non-emptiness, so a row silently rendering the wrong session's
// metadata (e.g. a copy-paste bug swapping two fields) would still fail
// this test even though every field it prints is individually non-blank.
// The fake agent process backing "e2e-session" is long-running, so the
// row's status must settle on "running" rather than the create-time
// unclassified placeholder — `toHaveText` retries on its own, since the
// list computes status fresh from tmux on every fetch rather than
// caching the placeholder forever. What that placeholder renders as while
// it lasts is the separate no-badge rule (PLAN_M6_75.md item 3), pinned
// by the route-controlled unknown-status test further down: an
// unclassified session shows NO badge, so this assertion is waiting for a
// badge to appear at all, not for one word to replace another.
test("list renders the session row with title, cwd, invocation, and a running badge", async ({
  page,
  request,
}) => {
  const listing = await (await request.get("/api/sessions")).json();
  const expected = listing.sessions.find((s: any) => s.title === "e2e-session");
  expect(expected).toBeTruthy();

  await page.goto("/");
  const row = sharedSessionRow(page);
  await expect(row).toBeVisible();
  await expect(row.locator(".session-title")).toHaveText("e2e-session");
  await expect(row.locator(".session-cwd")).toHaveText(expected.cwd);
  await expect(row.locator(".session-invocation")).toHaveText(expected.invocation);
  await expect(row.locator(".status-badge")).toHaveText("running", {
    timeout: 10_000,
  });
});

// The create form's working-directory field defaults to "~"
// (BUGS_BURNDOWN.md issue 2): the common create needs no typing, and the
// literal "~" is what gets sent — the supervisor expands it against the
// TARGET host's home, which is why a host-independent default is possible
// at all. Pinned as the field's actual initial value (not a placeholder)
// because the decision was specifically "what you see is what is sent";
// a regression to an empty-but-hinted field would pass a weaker check.
test("the create form prefills the working directory with ~", async ({ page }) => {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await expect(form.locator('input[type="text"]').nth(0)).toHaveValue("~");
});

// Keyboard activation (PLAN_M2.md step 7: rows must be
// keyboard-activatable). The open action (`.session-row-open`, PLAN_M2.md
// step 8) is a native <button> rather than a div with a hand-rolled
// onkeydown, so Enter activation (and Space) come from the browser for
// free — this pins that it is actually reachable and operable via
// keyboard, not just that it happens to look like a button. Focusing
// `.session-row-open` directly, not the outer `.session-row` wrapper: step
// 8 turned the row itself into a plain (non-focusable) `<div>` so it could
// also host the stop/delete buttons as siblings — see the SessionRow doc
// in lib.rs — so the row wrapper no longer accepts focus at all.
test("keyboard activation opens the session, matching a real click", async ({
  page,
  request,
}) => {
  // Pinned to a bounce session so the shared session is provably NOT the
  // auto-selected one: without this, the page can auto-open e2e-session
  // before the keypress and the assertions hold with keyboard activation
  // broken entirely.
  const bounce = await createSession(request, {
    title: `kbd-bounce-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await pinAutoSelect(page, bounce.id);
    await page.goto("/");
    await expect(page.locator(".titlebar .title")).toContainText("kbd-bounce-");
    await sharedSessionRow(page).locator(".session-row-open").focus();
    await page.keyboard.press("Enter");
    await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  } finally {
    await cleanupSession(request, bounce.id);
  }
});

// Navigation lifecycle (PLAN_M2.md step 7): SessionView used to assume it
// never unmounted (M1 had exactly one view), so the JS island only ever
// needed a mount-time double-mount guard. This pins the FULL round trip:
// going back must actually tear down the mounted terminal (not just
// leave it running unobserved), and reopening the SAME session must
// produce a genuinely NEW mount rather than either a no-op or a reused
// instance — replay alone cannot distinguish "correctly reattached" from
// "never actually left", since replaying scrollback from a still-open
// socket would look identical to a correct fresh reattach. Stamping the
// live xterm instance before leaving, and asserting a DIFFERENT instance
// exists after reopening, is what closes that gap.
test("switching sessions tears down the mounted terminal; reselecting mounts a fresh one", async ({
  page,
  request,
}) => {
  const bounce = await createSession(request, {
    title: `bounce-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
  await openTerminal(page);

  await page.locator("#terminal").click();
  await page.keyboard.type("marker-before-back");
  await page.keyboard.press("Enter");
  await waitForTermText(page, "echo:marker-before-back");

  await page.evaluate(() => {
    (window as any).__farhelmTerm.__testMarker = "before-back";
    // Stashed under different names so they survive terminal.js's own
    // deletes on unmount — the actual WebSocket object and test hook the
    // mount owned, kept around purely so the assertions below can check
    // unmount() really tore them down (and, for the hook, that reopening
    // installs a genuinely NEW one rather than reusing this one).
    (window as any).__testWsBeforeBack = (window as any).__farhelmWs;
    (window as any).__testHookBeforeBack = (window as any).__farhelmTest;
  });

  // There is no back: leaving means selecting another session, whose
  // own mount immediately REPLACES the globals — so the teardown is
  // observed through replacement (a different xterm instance owns the
  // globals) plus the stashed socket's closure below, rather than
  // through a gone-entirely window that no longer exists.
  await page.locator(`[data-session-id="${bounce.id}"]`).click();
  await expect(page.locator(".titlebar .title")).toContainText("bounce-");
  await expect
    .poll(() =>
      page.evaluate(
        // A DEFINED replacement, not merely the old instance gone:
        // teardown deletes the global, and `undefined !== marker` would
        // declare victory over a blank pane.
        () =>
          Boolean((window as any).__farhelmTerm) &&
          (window as any).__farhelmTerm.__testMarker !== "before-back",
      ),
    )
    .toBe(true);
  // ...and the socket it owned must be genuinely closed (readyState 3 —
  // CLOSED; there is no browser `WebSocket` global in this Node-side
  // test context to reference `WebSocket.CLOSED` by name), not merely
  // abandoned with a stale reference that could still fire callbacks
  // into whatever mounts next (the WS-teardown-callbacks review finding
  // this guards against).
  await expect
    .poll(() =>
      page.evaluate(() => (window as any).__testWsBeforeBack.readyState),
    )
    .toBe(3);
  // readyState alone is not enough: a socket can be CLOSED while still
  // holding stale `onmessage`/`onclose`/etc callbacks that reference the
  // torn-down term/view (they simply never fire again once the socket is
  // closed — but a callback left in place is exactly what a regression
  // in unmount()'s "null the handlers before closing" step would look
  // like, and readyState would not catch it since assigning the socket's
  // OWN close doesn't require its handler properties to change).
  expect(
    await page.evaluate(() => {
      const ws = (window as any).__testWsBeforeBack;
      return {
        onopen: ws.onopen,
        onmessage: ws.onmessage,
        onerror: ws.onerror,
        onclose: ws.onclose,
      };
    }),
  ).toEqual({ onopen: null, onmessage: null, onerror: null, onclose: null });

  await expect(page.locator(".session-list")).toBeVisible();

  await sharedSessionRow(page).click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await waitForTermText(page, "FAKE-AGENT READY");

  const isFreshInstance = await page.evaluate(
    () => (window as any).__farhelmTerm.__testMarker !== "before-back",
  );
  expect(isFreshInstance).toBe(true);

  // The reopened attachment's test hook must be a genuinely NEW object
  // (not the old one somehow surviving unmount, and not a stale reference
  // reused) with a freshly-zeroed watermark state — the same "fresh, not
  // reused" property `isFreshInstance` above pins for the xterm instance,
  // extended to the hook PLAN_M2_5.md step 4 added.
  const hookState = await page.evaluate(() => {
    const hook = (window as any).__farhelmTest;
    const before = (window as any).__testHookBeforeBack;
    return { isDifferentObject: hook !== before, hook };
  });
  expect(hookState.isDifferentObject).toBe(true);
  // The WATERMARK half of the hook, asserted exactly: a reused or leaked
  // counter is precisely what this test exists to catch.
  //
  // Partial rather than an exact `toEqual` over the whole object, because
  // the hook also carries the catch-up record (PLAN_M5.md item 5), and
  // that half is racing this read by design — the reattach's replay may or
  // may not have landed by now (the assertion below is what waits for it),
  // so its fields have no stable value HERE. They are pinned by the
  // reattach-lands-at-tail specs at the end of this file instead.
  expect(hookState.hook).toMatchObject({
    paused: false,
    pauseCount: 0,
    resumeCount: 0,
  });

  // Replay must bring back output produced before THIS attachment
  // existed, exactly like the reload test below — the only difference
  // is that here the round trip goes through a session switch instead
  // of a full page reload.
  await waitForTermText(page, "echo:marker-before-back");

  // And the fresh mount must be genuinely live, not just showing stale
  // replayed content: a new marker must round-trip through it.
  await page.locator("#terminal").click();
  await page.keyboard.type("marker-after-reopen");
  await page.keyboard.press("Enter");
  await waitForTermText(page, "echo:marker-after-reopen");
  } finally {
    await cleanupSession(request, bounce.id);
  }
});

// Regression test for the "stale mount retry" bug: terminal.js's wait for
// xterm's globals used to live entirely in a bare `setTimeout` chain
// inside the eval'd JS, with nothing SessionView's teardown could reach
// in and cancel. Backing out of a session before that wait resolved left
// it running; if the user then opened a DIFFERENT session, the stale
// loop could eventually fire and mount the FIRST session's terminal into
// the SECOND session's view (and, since the old mount guard was already
// set by the real mount, silently no-op the real one instead).
//
// An earlier version of this test only clicked through the navigation
// quickly and asserted the Dioxus-rendered `.titlebar .title` afterward
// — which passes even if session A's socket is the one that actually
// mounted, since the titlebar text comes from the `session` PROP
// SessionView was given, entirely independent of what terminal.js did.
// It also never forced a genuinely pending retry in the first place: on
// an unloaded box, `mountWhenReady`'s very first synchronous readiness
// check routinely just succeeds, so there was nothing left running to
// cancel and the test could pass for reasons that had nothing to do with
// the fix.
//
// This version forces the pending state for real (withholding
// `window.Terminal`, which `mountWhenReady` cannot proceed without) and
// makes the resulting race deterministic with Playwright's fake clock
// instead of hoping real wall-clock timing falls out favorably: with the
// clock frozen, session A's retry and session B's retry are scheduled
// for the IDENTICAL virtual instant, and same-deadline timers fire in
// registration order — so if A's retry were never cancelled, it would
// deterministically fire before B's and mount session A's socket into
// the (shared, same-DOM-id) terminal element first, with B's later mount
// then no-opping against the "already mounted" guard. Asserting the
// MOUNTED SOCKET'S URL — not any Dioxus-rendered text — is what actually
// catches that.
//
// terminal.js actually has THREE points that can cancel session A's
// retry (`mountWhenReady`'s own `clearTimeout` on entry, `unmount()`'s
// `clearTimeout`, and `tryMount`'s `pending !== attempt` check), and any
// ONE of them alone is enough to stop the race above — checked directly
// while writing this test by disabling each in isolation and confirming
// it still passed. Only disabling all three at once reproduces the
// original bug (confirmed the same way). That is a real, if incidental,
// defense-in-depth; this test is only equipped to fail if ALL of a
// regression's remaining protections vanish together, not to identify
// which single one a future change removed.
test("switching sessions before the first terminal is ready mounts the second session's socket", async ({
  page,
  request,
}) => {
  const created = await request.post("/api/sessions", {
    data: {
      cwd: "/tmp",
      invocation: "sleep 300",
      title: "regression-session-b",
    },
  });
  expect(created.status()).toBe(200);
  const { id: idB } = await created.json();

  try {
    await page.goto("/");
    await expect(sharedSessionRow(page)).toBeVisible();

    // Freeze the page's timers. `install()` alone does NOT pause time —
    // it only swaps in fake implementations, which by themselves keep
    // ticking at native speed — so `pauseAt()` is what actually stops
    // the clock; without it, both retries below would still be driven
    // by real elapsed wall-clock time between actions, defeating the
    // whole point of using the fake clock here. Playwright's own waits
    // (`waitForFunction`) poll from OUTSIDE the page over CDP and are
    // unaffected by any of this; only the page's OWN `setTimeout` calls
    // — exactly what `mountWhenReady`'s retry loop uses — come under our
    // control.
    await page.clock.install();
    // Paused at a strictly FUTURE instant: the fake clock keeps ticking
    // in real time between install() and pauseAt(), so pausing at "now"
    // races that tick and throws "Cannot fast-forward to the past" on a
    // loaded box.
    await page.clock.pauseAt(new Date(Date.now() + 5_000));

    // Withhold a global `mountWhenReady` genuinely cannot proceed
    // without, so opening session A puts a REAL pending retry into
    // flight (rather than resolving on its first synchronous check, as
    // it almost always would on an unloaded box).
    await page.evaluate(() => {
      (window as any).__testStashedTerminal = (window as any).Terminal;
      delete (window as any).Terminal;
    });

    await sharedSessionRow(page).click();
    // Direct switch — the keyed remount tears the shared view down.
    await page.locator(`[data-session-id="${idB}"]`).click();

    // Restore the withheld global, THEN advance the frozen clock: both
    // session A's original retry (if a regression left it running) and
    // session B's fresh one were scheduled for the same virtual instant
    // (nothing advanced the clock between the two clicks), so this is
    // what actually exercises the race described above.
    await page.evaluate(() => {
      (window as any).Terminal = (window as any).__testStashedTerminal;
      delete (window as any).__testStashedTerminal;
    });
    await page.clock.runFor(500);

    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    const wsUrl = await page.evaluate(() => (window as any).__farhelmWs.url);
    expect(wsUrl).toContain(idB);
    await expect(page.locator(".titlebar .title")).toHaveText(
      "regression-session-b",
    );
  } finally {
    await request.post(`/api/sessions/${idB}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${idB}`).catch(() => {});
  }
});

// Asset tags are registered in order but execute asynchronously. Clipboard
// naming is part of mount readiness, not an optional paste enhancement: an
// island mounted before this helper exists would silently use no captured
// policy at all for its first paste.
test("terminal mounting waits for the clipboard naming helper", async ({ page, request }) => {
  // Auto-select mounts a terminal at load, before this test can withhold
  // the helper — so park the auto-selection on a bounce session, and make
  // the withheld-helper mount a FRESH one (selecting the shared row tears
  // the bounce island down, and the shared mount then has to wait).
  const bounce = await createSession(request, {
    title: `clipboard-bounce-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
  await pinAutoSelect(page, bounce.id);
  await page.goto("/");
  await expect(sharedSessionRow(page)).toBeVisible();
  await page.waitForFunction(() => Boolean((window as any).__farhelmIslands?.terminal));
  await page.evaluate(() => {
    (window as any).__testClipboardNames = (window as any).farhelmClipboardNames;
    delete (window as any).farhelmClipboardNames;
  });

  await sharedSessionRow(page).click();
  await page.waitForTimeout(150);
  expect(
    await page.evaluate(() => Boolean((window as any).__farhelmIslands?.terminal)),
    "the terminal must remain unmounted while its clipboard policy is unavailable",
  ).toBe(false);

  await page.evaluate(() => {
    (window as any).farhelmClipboardNames = (window as any).__testClipboardNames;
    delete (window as any).__testClipboardNames;
  });
  await page.waitForFunction(() => Boolean((window as any).__farhelmIslands?.terminal));
  } finally {
    await cleanupSession(request, bounce.id);
  }
});

// Playwright-level coverage for the PARTIAL-MOUNT ROLLBACK finding:
// mount() sets its guard (`active`, since terminal.js's simplification —
// see its docs) only at the very end of a successful mount, so an
// exception partway through (a `WebSocket` constructor throwing, here)
// must leave `active` exactly as it was before the attempt — not stuck
// in a state that wedges every later mount shut. Monkeypatching
// `window.WebSocket` to throw is the cleanest deterministic way to break
// mount() partway through: it is the very next thing mount() does after
// constructing the xterm.js `Terminal` (already-real work that must
// itself be rolled back — the terminal.js catch block disposes it),
// requires no changes to production code to trigger, and — restored
// before the second attempt — reproduces the exact "mount, fail, mount
// the SAME session again" sequence a real transient failure would leave
// a user facing.
test("a failed mount rolls back cleanly; the same session can be mounted again", async ({
  page,
  request,
}) => {
  // A parking spot: with auto-select there is no unselected state, so
  // "reopen the same session" bounces through this row instead of a back
  // control. Created before goto so the WebSocket sabotage below cannot
  // race its own mount in any way that matters — a failed bounce mount
  // still moves the selection, which is all the bounce is for.
  const bounce = await createSession(request, {
    title: `rollback-bounce-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
  await page.goto("/");
  await expect(sharedSessionRow(page)).toBeVisible();
  // Park on the bounce row so the LATER click on the shared row is a real
  // selection change (auto-select may have already picked either row).
  await page.locator(`[data-session-id="${bounce.id}"]`).click();

  await page.evaluate(() => {
    (window as any).__testRealWebSocket = window.WebSocket;
    (window as any).WebSocket = class {
      constructor() {
        throw new Error("injected failure for rollback test");
      }
    };
  });

  await sharedSessionRow(page).click();
  // termReady never becomes true on this path — mount() throws before
  // reaching the line that sets it — so the banner text (which the
  // catch block does set) is the only thing to wait on here.
  await expect(page.locator("#term-banner")).toContainText(
    "Failed to start terminal",
  );
  // The failed attempt must not have left anything looking mounted.
  expect(
    await page.evaluate(() => (window as any).__farhelmTerm === undefined),
  ).toBe(true);

  await page.evaluate(() => {
    window.WebSocket = (window as any).__testRealWebSocket;
    delete (window as any).__testRealWebSocket;
  });

  // Reopening the SAME session (bounce to another row, then back to it)
  // must succeed now that the guard was rolled back — a regression that
  // left `active` (or the old `__farhelmMounted` flag) stuck would make
  // this mount silently no-op instead.
  await page.locator(`[data-session-id="${bounce.id}"]`).click();
  await sharedSessionRow(page).click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await waitForTermText(page, "FAKE-AGENT READY");
  } finally {
    // The sabotage must not outlive the test even on failure — a broken
    // global WebSocket would cascade into every later mount.
    await page
      .evaluate(() => {
        if ((window as any).__testRealWebSocket) {
          (window as any).WebSocket = (window as any).__testRealWebSocket;
          delete (window as any).__testRealWebSocket;
        }
      })
      .catch(() => {});
    await cleanupSession(request, bounce.id);
  }
});

// Real keystrokes through the whole chain: xterm's onData, the WebSocket,
// the framing protocol, tmux send-keys on the dedicated input control
// client, and back out as pane output. This is the one test that would
// catch an input path wired up but dead — the failure a user would
// describe as "typing goes nowhere".
test("input round-trips through the real terminal path", async ({ page }) => {
  await openTerminal(page);
  await page.locator("#terminal").click();
  await page.keyboard.type("hello-from-playwright");
  await page.keyboard.press("Enter");
  await waitForTermText(page, "echo:hello-from-playwright", 10_000);
});

// Regression test for a real bug: xterm.js auto-answers a DECRQM mode
// query (e.g. vim's own cursor-blink probe, `ESC[?12$p`, which tmux passes
// through unmodified from the pane) with a DECRPM reply (`ESC[?12;2$y`)
// through its OWN `onData` callback — the identical callback real
// keystrokes flow through. terminal.js used to forward that reply
// straight back as pane input; the render-batch-plus-WebSocket round trip
// means it lands a full turnaround later, long after the querying app
// stopped waiting for an answer. vim then parses it as KEYSTROKES rather
// than a stale reply — '$' is a silent motion and 'y' becomes a pending
// operator, observed as a stray pending 'y' on every vim launch. The fix
// (terminal.js's `swallowDecrqm` parser handlers) intercepts the DECRQM
// QUERY itself on the output side, so xterm never mints a reply at all —
// user input is never inspected, and even a pasted look-alike of a reply
// passes through untouched. Safe because tmux answers DECRQM for its own
// panes itself, instantly (verified directly by probing), so the reply
// xterm no longer sends was a late duplicate — pure harm, never the only
// copy.
//
// This asserts on the WEBSOCKET FRAMES ACTUALLY SENT rather than driving
// vim end to end: vim is not otherwise a CI dependency of this suite, and
// reproducing its stray-'y' symptom would mean racing a real editor
// against the network — brittle and slow next to checking the fix's own
// contract directly ("this exact byte shape never leaves the browser as
// input"). Feeding a real shell's pane output raw DECRQM/OSC-11 query
// bytes via `printf` makes xterm.js generate the very same auto-replies
// vim would trigger, deterministically, with no editor involved.
//
// Runs against a fresh `bash` session, not the shared fake-agent one: the
// fake agent's `basic` script only ever echoes typed lines back as text
// (fake_agent.rs) — it never executes anything, so it could never emit
// the raw escape bytes this test needs on the wire in the first place.
test("DECRPM auto-replies to a mode query are dropped, not forwarded as pane input", async ({
  page,
  request,
}) => {
  // Patch only `send` on the WebSocket PROTOTYPE, before any navigation.
  // Replacing the `WebSocket` constructor outright (as the mount-rollback
  // test above does deliberately, to make it throw) would break RECEIVING
  // frames too — this test still needs that, to see PROBE-DONE arrive.
  // Binary frames are recorded as plain byte arrays rather than left as
  // Uint8Array/ArrayBuffer instances, so they survive the `page.evaluate`
  // round trip back to Node intact.
  await page.addInitScript(() => {
    const realSend = WebSocket.prototype.send;
    (window as any).__sentInput = [];
    WebSocket.prototype.send = function (this: WebSocket, data: any) {
      if (data instanceof Uint8Array) {
        (window as any).__sentInput.push(Array.from(data));
      } else if (data instanceof ArrayBuffer) {
        (window as any).__sentInput.push(Array.from(new Uint8Array(data)));
      } else {
        (window as any).__sentInput.push(data);
      }
      return realSend.call(this, data);
    };
  });

  const title = `decrpm-probe-${Date.now()}`;
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "bash", title },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row).toBeVisible();
    await row.click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    // `__farhelmTermReady` means the terminal MOUNTED, not that its
    // socket reached OPEN — and terminal.js drops input sent before OPEN.
    // Under WebKit's slower startup that gap is wide enough to eat the
    // first characters of the probe command, so wait for the socket
    // itself, the way sendFloodGateByte does.
    await page.waitForFunction(
      () => (window as any).__farhelmWs?.readyState === WebSocket.OPEN,
    );

    await page.locator("#terminal").click();
    // Two queries go out together: DSR-6 (`\e[6n`, "where is the
    // cursor?"), which xterm must still answer, and the DECRQM query for
    // mode 12 (`\e[?12$p`, vim's cursor-blink probe), which must now
    // provoke NO reply at all. PROBE-DONE is the synchronization marker
    // proving the whole line — including the 1-second gap — actually ran
    // in this real shell, not merely that it was typed.
    //
    // DSR-6 rather than an OSC-11 color query as the control, and that
    // choice is CI-hardened rather than arbitrary: headless WebKit on
    // the CI runner never answers OSC 11 (it has no theme colors to
    // report), so a color-reply control failed there while the real
    // assertion held. A cursor report is computed from xterm's own
    // buffer and is therefore environment-independent.
    //
    // The probe line is PASTED (xterm's programmatic paste API), not
    // typed. This test's one recurring CI flake was the control
    // assertion finding no cursor report: `page.keyboard.type` delivers
    // the line one keystroke at a time, and a single dropped character
    // inside the escape portion under CI load yields a printf that emits
    // no DSR query — while `echo PROBE-DONE`, a separate command after
    // the `;`, still runs, so the synchronization marker looked healthy.
    // A paste delivers the whole line as one input frame, so it either
    // arrives intact or not at all; PROBE-DONE then proves it ran.
    //
    // The marker is built with printf %s so the string "PROBE-DONE"
    // never appears in the COMMAND itself: the shell echoes the pasted
    // line immediately, and a literal marker in the echo would satisfy
    // the wait below before the command — including the 1-second gap
    // the queries need — had actually executed.
    const probeLine =
      "printf '\\e[6n\\e[?12$p'; sleep 1; printf 'PROBE-%s\\n' DONE";
    await page.evaluate(
      (line) => (window as any).__farhelmTerm.paste(line),
      probeLine,
    );
    await page.keyboard.press("Enter");
    await waitForTermText(page, "PROBE-DONE", 15_000);

    const recordedFrames = () =>
      page.evaluate(() =>
        ((window as any).__sentInput as unknown[]).map((f) =>
          Array.isArray(f)
            ? String.fromCharCode(...(f as number[]))
            : String(f),
        ),
      );

    // The control first, and polled rather than sampled once: a cursor-
    // position report DID reach the WebSocket, proving xterm's
    // auto-replies were genuinely live and flowing through this exact
    // recorded path — without it, the assertion below could pass
    // vacuously (e.g. if nothing were recorded at all). The recording
    // hook is synchronous and xterm parses the DSR long before the
    // marker prints, so the poll is defensive depth, not a required
    // wait — the marker output already sequences everything.
    await expect
      .poll(recordedFrames, { timeout: 5_000 })
      .toContainEqual(expect.stringMatching(/\x1b\[[0-9]+;[0-9]+R/));
    // The fix: no DECRPM reply shape ever reached the WebSocket as input.
    expect(await recordedFrames()).not.toContainEqual(
      expect.stringMatching(/\x1b\[\?[0-9;]*\$y/),
    );
  } finally {
    await cleanupSession(request, id);
  }
});

// MT-6 regression test: a select-and-copy in the terminal leaves TWO
// selections behind, and both stay painted over content the user is no
// longer selecting once input moves the buffer underneath them. xterm's
// own selection is anchored to buffer COORDINATES rather than to the
// text in them; the DOM renderer's real text nodes additionally carry a
// NATIVE document selection, which `Terminal.clearSelection()` does not
// touch. Manual testing on macOS found the second one: the highlight
// survived a paste, then survived typing, then survived a forced
// `refresh()` — because only the native selection was still there.
//
// The fix (terminal.js's `dismissSelection`) drops both on user-origin
// input: keyboard through `onKey`, paste through a capture-phase DOM
// listener (xterm's own paste handler sits on the hidden textarea and
// calls stopPropagation, so a bubble-phase listener never runs).
//
// The selection is made with a REAL MOUSE DRAG, not `selectAll()`: only
// a drag produces the native selection that carried this bug, so a
// programmatic selection would test the half that already worked. And
// the native side is asserted through `window.getSelection()` rather
// than `isCollapsed`, which WebKit reports as `true` for a drag-made
// selection whose ranges are still present and still painted.
//
// WHAT `window.getSelection()` ACTUALLY REPORTS HERE, because it is not
// the same thing on the machine that found the bug and the machine that
// runs this suite. xterm.js supports X11's PRIMARY selection by copying
// every mouse selection into its hidden helper textarea and calling
// `focus()` + `select()` on it (`onLinuxMouseSelection`, gated on
// `navigator.platform` containing "Linux" — true for BOTH Playwright
// engines on a Linux host, including WebKit, whose user agent claims
// macOS while its platform string does not). So under this suite the
// document selection a drag leaves behind is anchored in `.xterm-helpers`,
// not in the rendered rows: probing it directly, `getSelection()` tracks
// `textarea.selectionStart..selectionEnd` character for character, and
// reports zero while that textarea's selection is momentarily collapsed
// even though xterm's own selection is unchanged. On real macOS
// (`isLinux` false, no mirror) it is the row-anchored selection the MT-6
// bug was about. Both are cleared by the same `removeAllRanges()`, so
// this test pins the same contract on both — but only the macOS shape is
// ever painted over content.
//
// THE FLAKE, and why the key leg presses a key without releasing it:
// this test failed intermittently under CI load with `nativeChars` back
// at its pre-input value (150 characters on the CI viewport) after the
// dismissal. Reproduced locally at two to three failures per 50-60
// repetitions with a dozen spinning CPU hogs alongside, and traced by
// patching `Selection.prototype.removeAllRanges` and
// `HTMLTextAreaElement.prototype.focus`/`select` to log stacks. The
// blame lands on the KEY RELEASE, not on the dismissal: xterm's own
// `_keyUp` handler calls `Terminal.focus()`, and refocusing a text
// control makes the engine restore that control's cached selection —
// still the mirrored drag text, because `removeAllRanges()` cleared the
// live selection without invalidating that cache. About 30ms after the
// refocus, the selection reappears. Nothing is painted (the helper
// textarea is `opacity: 0`, parked at `left: -9999em`) and no user could
// see it, and terminal.js cannot prevent it either — the restore comes
// from xterm refocusing its own helper element. Whether this test noticed
// came down to whether its first poll sample landed inside the ~40ms
// window between the dismissal and the restore.
//
// Hence `keyboard.down("x")` with the matching `up` deferred all the way
// to `finally`, instead of `press`. That is not a weaker assertion — same
// real, trusted key event, and the keydown is where the input contract
// lives: xterm sends the character and fires `onKey` (and therefore
// `dismissSelection`) on the way DOWN, while the release carries no input
// at all. Deferring it only stops an unrelated xterm behavior from racing
// the state this test reads. It has to be deferred past the LAST
// assertion, not just its own: a restoration scheduled by releasing "x"
// (or by any intervening keypress, which is why the paste leg no longer
// erases the typed character first) lands tens of milliseconds later, by
// which point the paste leg's poll is the one it would corrupt.
//
// Both legs still poll rather than sample once, because the dismissal
// genuinely is eventually-consistent by design: terminal.js sweeps once
// synchronously and once on a `setTimeout(0)`, and traces show either
// sweep landing the actual `removeAllRanges()` depending on where the
// engine had the document selection at that instant.
test("input dismisses both the xterm and native selections", async ({
  page,
}) => {
  await openTerminal(page);
  await page.locator("#terminal").click();

  const selectionState = () =>
    page.evaluate(() => ({
      xterm: (window as any).__farhelmTerm.hasSelection(),
      nativeChars: String(window.getSelection() || "").length,
    }));

  // Drag across a few rows of the terminal's own output.
  const dragSelect = async () => {
    const box = (await page.locator("#terminal").boundingBox())!;
    await page.mouse.move(box.x + 30, box.y + 40);
    await page.mouse.down();
    await page.mouse.move(box.x + 300, box.y + 90, { steps: 10 });
    await page.mouse.up();
    await expect.poll(selectionState).toMatchObject({ xterm: true });
    expect((await selectionState()).nativeChars).toBeGreaterThan(0);
  };

  try {
    // First leg: a selection SURVIVES terminal-generated traffic. The
    // fake agent echoes the typed line back, and that inbound output —
    // like any auto-reply xterm generates in response to queries — flows
    // through paths that must NOT clear a selection: only user-origin
    // input may. A `clearSelection` that migrated into `onData` (where
    // those replies also flow) would fail here.
    await page.keyboard.type("probe");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:probe", 10_000);
    await dragSelect();
    await page.waitForTimeout(200);
    expect(await selectionState()).toMatchObject({ xterm: true });

    // Second leg: a keystroke dismisses both selections. The key is never
    // released inside the test body — see this test's own docs: a release
    // schedules the mirrored selection's restoration, and that restoration
    // would then be in flight across the paste leg below, racing ITS poll
    // the same way it raced this one. The only release is in `finally`,
    // after every assertion has been made.
    await page.keyboard.down("x");
    await expect.poll(selectionState).toEqual({ xterm: false, nativeChars: 0 });

    // Third leg: so does a paste. Dispatched as a synthetic
    // ClipboardEvent rather than driving the OS clipboard, whose
    // permissions differ per engine; the event is exactly what a real
    // ⌘V/Ctrl-V delivers to this same target. No Backspace first: erasing
    // the typed "x" would mean another press, another release, and another
    // restoration in flight — and the `Control+U` in `finally` clears the
    // whole line anyway, which is all this test owes the shared session.
    await dragSelect();
    await page.evaluate(() => {
      const data = new DataTransfer();
      data.setData("text/plain", "pasted");
      (window as any).__farhelmTerm.textarea.dispatchEvent(
        new ClipboardEvent("paste", {
          clipboardData: data,
          bubbles: true,
          cancelable: true,
        }),
      );
    });
    await expect.poll(selectionState).toEqual({ xterm: false, nativeChars: 0 });
  } finally {
    // Release the held "x" (harmless if an earlier failure meant it was
    // never pressed), and only here, so its restoration can no longer
    // overlap any assertion above.
    await page.keyboard.up("x");
    // The typed "x" and the pasted text both land at the prompt; clear the
    // whole line rather than counting characters, so the shared session's
    // prompt is left as this test found it.
    await page.keyboard.press("Control+U");
  }
});

// SPEC.md's core durability promise seen from the browser: close the tab,
// come back, and the session looks as if you had never left. A reload is
// the harshest form of it — a brand-new xterm.js with an empty buffer, so
// everything on screen afterwards came from replay.
test("reload reattaches with replayed scrollback", async ({ page }) => {
  await openTerminal(page);
  await page.locator("#terminal").click();
  await page.keyboard.type("before-reload");
  await page.keyboard.press("Enter");
  await waitForTermText(page, "echo:before-reload");

  await page.reload();
  // A reload resets the app's navigation state (App's `Signal<Option
  // <Session>>` starts at `None` on every fresh load), so it lands back
  // on the list view, not the terminal directly — the row must be
  // clicked again, same as openTerminal's own first attach.
  const row = sharedSessionRow(page);
  await expect(row).toBeVisible();
  await row.click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  // Replay must bring back output produced before this attachment
  // existed — the reconnect-with-replay acceptance criterion.
  await waitForTermText(page, "echo:before-reload");
  await waitForTermText(page, "FAKE-AGENT READY");
});

// Exercise the complete browser-to-PTY resize chain. Xterm's local
// dimensions are only the requested geometry; the fake agent's `stty`
// result proves the WebSocket message reached tmux before later input.
test("resize reaches the real terminal", async ({ page }) => {
  await openTerminal(page);
  const before = await page.evaluate(() => {
    const t = (window as any).__farhelmTerm;
    return { cols: t.cols, rows: t.rows };
  });
  await page.setViewportSize({ width: 700, height: 500 });
  await expect
    .poll(
      () =>
        page.evaluate(() => {
          const t = (window as any).__farhelmTerm;
          return { cols: t.cols, rows: t.rows };
        }),
      { message: "viewport change must reflow the terminal via fit()" },
    )
    .not.toEqual(before);
  const geometry = await page.evaluate(() => {
    const t = (window as any).__farhelmTerm;
    return { cols: t.cols, rows: t.rows };
  });
  await page.locator("#terminal").click();
  await page.keyboard.type("size");
  await page.keyboard.press("Enter");
  await waitForTermText(
    page,
    `size:${geometry.rows} ${geometry.cols}`,
    10_000,
  );
});

test("second client takes over; first shows the detach banner", async ({
  browser,
  page,
}) => {
  await openTerminal(page);

  const second = await browser.newContext();
  const page2 = await second.newPage();
  // A fresh context has its own list view, so it goes through the same
  // list-then-click path as `page` did in openTerminal(page) above —
  // there is no direct terminal URL to land on.
  await openTerminal(page2);

  // SPEC.md: last attach wins, and the loser sees it happened.
  await expect(page.locator("#term-banner")).toBeVisible({ timeout: 10_000 });
  await expect(page.locator("#term-banner")).toContainText("Detached");

  // The winner is live: input still round-trips.
  await page2.locator("#terminal").click();
  await page2.keyboard.type("takeover-works");
  await page2.keyboard.press("Enter");
  await waitForTermText(page2, "echo:takeover-works", 10_000);
  await second.close();
});

// PLAN_M2.md acceptance 4: a restart-gap session (tmux gone, metadata
// intact) must open to "metadata plus why there is no terminal", not a
// silently blank pane. Nothing before this test pinned the UI half of
// that criterion — the Rust suite covers only the supervisor side, in
// `restart_gap_lists_sessions_without_a_terminal_and_attach_fails`
// (crates/farhelm/tests/e2e.rs).
//
// This suite's stack cannot restart its supervisor mid-run
// (start-stack.sh boots one long-lived supervisor for the whole file), so
// a genuine restart gap is out of reach here. The stand-in: a session row
// the real supervisor has never heard of. That is a DIFFERENT failure
// branch on the supervisor side — an id absent from `sup.sessions`
// entirely takes the "no such session: {id}" arm of `ControlMsg::Attach`'s
// handler (service.rs), while a genuine restart-gap row (present in the
// map, `entry.terminal` empty) takes the sibling "session {id} has no
// terminal: the supervisor (or its tmux server) restarted after the agent
// ended" arm right below it — distinct branch, distinct wording, not
// reproduced here. What the two DO share, and what this test actually
// exercises, is everything downstream of that error: `serve_term` in
// farhelm-helm/src/lib.rs attaches over a REAL WebSocket, gets back
// whichever error, and relays it as a genuine `detached` control message
// the same way regardless of which arm produced it; the browser side
// (terminal.js's `showBanner`) and the list UI's metadata rendering have
// no way to tell the two apart either. So this pins the shared
// helm/WebSocket/UI error-display path — the UI CONTRACT of "metadata
// shown, plus a server-provided explanation instead of a silently blank
// terminal" — not the restart-gap-specific message, which belongs to the
// Rust test named above.
//
// Only the row's EXISTENCE is synthetic: route-intercepted GET
// /api/sessions, injecting one extra row alongside the real ones (rather
// than fabricating the whole response) so the shared "e2e-session" row
// every other test in this file depends on still comes from the real
// supervisor on THIS request. That protection is necessarily local to
// this one route handler and this one page, though: Playwright routes are
// page-scoped, so nothing here could leak into another test's page even
// if it wanted to. The banner text is asserted only to be non-empty and
// to name the session, whatever exact prose this particular arm's error
// happens to carry.
test("opening a terminal-less session shows its metadata and the server's own explanation", async ({
  page,
}) => {
  // A well-formed but unknown UUID: recognizable in the banner text
  // without colliding with any id a real session in this run could have.
  const bogusId = "00000000-0000-0000-0000-000000000000";
  const title = `terminal-less-${Date.now()}`;
  const cwd = "/tmp/terminal-less-fixture";
  const invocation = "true";

  // This test only ever issues GETs against this route (no create/stop/
  // delete call in its body), so there is no other method to fall through
  // to `route.continue()` for.
  await page.route("**/api/sessions", async (route) => {
    // Fetch the REAL listing and append one row, rather than fabricating
    // the whole response: every other row (in particular the shared
    // "e2e-session" other tests in this file depend on) must keep coming
    // from the real supervisor, unmodified.
    const response = await route.fetch();
    const listing = await response.json();
    listing.sessions.push({
      id: bogusId,
      title,
      cwd,
      invocation,
      // Exactly the shape a restart-gap row has (PLAN_M2.md, and
      // `SessionStatus::Exited` in farhelm-proto/src/lib.rs): known dead,
      // no code to fabricate.
      status: { state: "exited", exit_code: null },
    });
    listing.total += 1;
    await route.fulfill({ response, json: listing });
  });

  await page.goto("/");
  // `.click()` already waits for the target to be visible and stable, so
  // there is nothing an upfront `toBeVisible` would add here.
  await rowByTitle(page, title).locator(".session-row-open").click();

  // (a) metadata IS shown — title and titlebar `.meta` (cwd — invocation,
  // farhelm-ui/src/lib.rs) render from the row's own fields, independent
  // of whether a terminal ever comes up behind them. `toHaveText` retries
  // on its own, so this needs no separate mount-readiness wait first.
  await expect(page.locator(".titlebar .title")).toHaveText(title);
  await expect(page.locator(".titlebar .meta")).toHaveText(
    `${cwd} — ${invocation}`,
  );

  // (b) the banner becomes visible and carries the server's own reason
  // (farhelm-ui/assets/terminal.js's showBanner, fed by serve_term's
  // `detached` notice — "Detached: <reason>"), not a blank pane or a
  // generic "connection closed". This IS this test's real synchronization
  // point: the banner is asynchronous, arriving only after the WS
  // round-trips through the real attach failure, so waiting on it (rather
  // than on `__farhelmTermReady`, which flips as soon as mount() opens the
  // socket and says nothing about how the attach behind it resolves) is
  // what actually proves the failure was relayed all the way to the DOM.
  const banner = page.locator("#term-banner");
  await expect(banner).toBeVisible({ timeout: 10_000 });
  // The visibility assertion above guarantees the element exists, so
  // `textContent()` cannot be null here.
  const bannerText = await banner.textContent();
  expect(bannerText).toMatch(/^Detached: .+/);
  expect(bannerText).toContain(bogusId);

  // (c) no agent output ever reached the terminal: the attach failed
  // before any `TermEvent::Data` could exist to write into the buffer.
  expect((await termText(page)).trim()).toBe("");
});

// The creation API is the one true path (PLAN_M1.md: CLI flags AND the UI
// dialog (PLAN_M2.md step 8) both feed this same endpoint), so its HTTP
// surface needs its own direct coverage. Only the failure case is
// exercised here: a successful POST would leave an extra, untracked
// session sitting in the list for every test after this one, with nothing
// to clean it up.
// The status code is part of the contract, not an implementation detail:
// a missing cwd is the caller's own precondition failure (4xx), distinct
// from a server-side fault (5xx) the caller could not have avoided by
// sending a different request. The supervisor classifies this as
// InvalidRequest and farhelm-helm's http_error maps that to 400 — see
// ErrorKind in farhelm-proto.
//
// "Contains", not "is": the assertion below is `toContain`, not an exact
// match, because the body carries more than just the one sentence pinned
// here (an anyhow error chain — see farhelm-helm's `http_error` — can
// prefix or wrap it with additional context). The test's job is pinning
// that THIS text is present verbatim somewhere in the body, not pinning
// the whole body's exact shape.
test("create API reports a precondition failure containing the supervisor's own text", async ({
  request,
}) => {
  const resp = await request.post("/api/sessions", {
    data: { cwd: "/nonexistent/definitely/not/here", invocation: "true" },
  });
  expect(resp.status()).toBe(400);
  expect(await resp.text()).toContain("working directory does not exist");
});

// Request-level coverage for the stop/delete HTTP surface (PLAN_M2.md step
// 6): the full UI flows (stop/delete buttons, delete's confirmation
// dialog) are PLAN_M2.md step 8's PR, so this exercises the API
// directly against the real stack, following the request-fixture style of
// the create-API test above rather than driving a page. It creates its
// own session (a long-running `sleep`, distinct from the shared
// "e2e-session" every terminal test above depends on) so it can freely
// stop and delete it without perturbing the rest of the suite.
test("stop and delete a session through the HTTP API", async ({ request }) => {
  const totalBefore = (await (await request.get("/api/sessions")).json())
    .total;

  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  // Everything past creation is wrapped so a failed assertion here still
  // cleans up: this suite is one shared, serially-run stack (see the
  // config's fullyParallel/workers comment), so a leaked long-running
  // `sleep 300` session would keep sitting in the list for every test
  // after this one — and could cascade-fail any of them that assumes
  // something about which sessions exist. The `finally` delete tolerates
  // a 404 (and any other failure) because the happy path below already
  // deletes the session itself; the cleanup call is only load-bearing
  // when an assertion above threw first.
  try {
    const afterCreate = await (await request.get("/api/sessions")).json();
    expect(afterCreate.total).toBe(totalBefore + 1);

    const stopped = await request.post(`/api/sessions/${id}/stop`);
    expect(stopped.status()).toBe(200);
    expect(await stopped.json()).toEqual({});

    // tmux marks the pane dead asynchronously once the killed process is
    // reaped, so the next list poll (not the stop response itself) is what
    // proves the kill actually took effect.
    await expect
      .poll(
        async () => {
          const listing = await (await request.get("/api/sessions")).json();
          const session = listing.sessions.find((s: any) => s.id === id);
          return session?.status?.state;
        },
        { timeout: 10_000, message: "stopped session must show as exited" },
      )
      .toBe("exited");

    const deleted = await request.delete(`/api/sessions/${id}`);
    expect(deleted.status()).toBe(200);
    expect(await deleted.json()).toEqual({});

    await expect
      .poll(
        async () => {
          const listing = await (await request.get("/api/sessions")).json();
          return listing.sessions.some((s: any) => s.id === id);
        },
        { timeout: 10_000, message: "deleted session must disappear from the list" },
      )
      .toBe(false);

    const afterDelete = await (await request.get("/api/sessions")).json();
    expect(afterDelete.total).toBe(totalBefore);
  } finally {
    // Best-effort: swallow everything, including a 404 for the (expected)
    // case where the happy path already deleted the session. This must
    // never throw over the top of a real assertion failure above.
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// PLAN_M2.md step 8's UI acceptance flow, driven end to end through the
// create form and per-row buttons rather than the raw API (the test above
// already covers the API's own contract): two sessions created from the
// form run side by side, one is opened and typed into, the other is
// stopped and its badge flips live, and both are deleted — one WITHOUT a
// confirmation prompt (already exited) and one WITH (still alive),
// pinning the exact confirm/no-confirm split SPEC.md's "Lifecycle
// operations" draws between the two states.
//
// The confirmation itself is the inline per-row prompt (`.confirm-consequence`
// plus `.confirm-title` plus `.confirm-delete`/`.confirm-cancel`), not a
// native `window.confirm()` — see `SessionRow`'s doc in lib.rs for why: wry
// ships no dialogs at all on macOS's WKWebView, which made the old
// eval-based path silently do nothing on that target. `.confirm-consequence`'s
// absence after a delete click is what stands in for "no dialog fired"
// below; its presence (checked for text content) is what stands in for
// "dialog mentions the running agent".
test("multi-session flow: create two, open and type in one, stop and delete the other, then delete the first with confirmation", async ({
  page,
  request,
}) => {
  const titleA = `multi-a-${Date.now()}`;
  const titleB = `multi-b-${Date.now()}`;

  try {
    // Create session A through the dialog; success navigates straight
    // into its terminal (SPEC.md: "creation launches the agent; you type
    // your first prompt into its terminal").
    await page.goto("/");
    const formA = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title: titleA,
    });
    await formA.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(page.locator(".titlebar .title")).toHaveText(titleA);

    // Type into session A while it is open, per the flow's "open one,
    // type" step, then use the permanent sidebar to create session B.
    await page.locator("#terminal").click();
    await page.keyboard.type("marker-multi-a");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:marker-multi-a");

    const formB = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title: titleB,
    });
    await formB.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(page.locator(".titlebar .title")).toHaveText(titleB);

    // Both rows are visible in the permanent sidebar, alive.
    await expect(rowByTitle(page, titleA).locator(".status-badge")).toHaveText(
      "running",
      { timeout: 10_000 },
    );
    await expect(rowByTitle(page, titleB).locator(".status-badge")).toHaveText(
      "running",
      { timeout: 10_000 },
    );

    // Stop session B via its row button. No confirmation for stop
    // (SPEC.md gives confirmation to delete/archive, not stop), and the
    // badge must flip on the next poll WITHOUT a reload.
    //
    // The badge still SAYS exited and adds the stop annotation as a
    // qualifier: SPEC.md is explicit that "stopped" is not a distinct
    // status, so the supervisor's durable annotation (PLAN_M3.md item 4)
    // qualifies the exited badge rather than replacing its text. This is
    // the browser-side proof of that whole path — the annotation is
    // written in the supervisor's store, travels the wire and the helm's
    // JSON, and lands in the DOM. Asserted on `.status-badge.exited` so
    // the CSS class rides with it: a stopped session must still LOOK like
    // an ended one.
    await openRowMenu(rowByTitle(page, titleB));
    await rowByTitle(page, titleB)
      .locator(".session-row-stop")
      .click();
    await expect(
      rowByTitle(page, titleB).locator(".status-badge.exited"),
    ).toHaveText(/^exited — stopped by user/, { timeout: 10_000 });

    // Delete session B (now exited): no confirmation expected — pin that
    // the inline prompt never appears at all, not merely that it gets
    // auto-handled. Stalled via the same route-hold technique as the
    // dedicated "exited session deletes immediately" test: a bare
    // post-click absence check cannot tell "never appeared" from
    // "appeared and vanished before this check ran", and holding the
    // DELETE open is what closes that gap.
    const idB = await findSessionIdByTitle(request, titleB);
    let releaseDeleteB: () => void = () => {};
    const deleteBHeld = new Promise<void>((resolve) => {
      releaseDeleteB = resolve;
    });
    await page.route(`**/api/sessions/${idB}`, async (route) => {
      if (route.request().method() !== "DELETE") {
        await route.continue();
        return;
      }
      await deleteBHeld;
      await route.continue();
    });
    await openRowMenu(rowByTitle(page, titleB));
    await rowByTitle(page, titleB)
      .locator(".session-row-delete")
      .click();
    await expect(rowByTitle(page, titleB)).toHaveCount(1);
    await expect(rowByTitle(page, titleB).locator(".confirm-consequence")).toHaveCount(
      0,
    );
    releaseDeleteB();
    await expect(rowByTitle(page, titleB)).toHaveCount(0, { timeout: 10_000 });

    // Delete session A (still alive): confirmation expected, wording
    // must say the agent is still running (SPEC.md: "confirmation that
    // says so when anything is still alive").
    await openRowMenu(rowByTitle(page, titleA));
    await rowByTitle(page, titleA)
      .locator(".session-row-delete")
      .click();
    await expect(rowByTitle(page, titleA).locator(".confirm-consequence")).toContainText(
      "running",
    );
    await rowByTitle(page, titleA)
      .locator(".confirm-delete")
      .click();
    await expect(rowByTitle(page, titleA)).toHaveCount(0, { timeout: 10_000 });
  } finally {
    // Best-effort: both sessions should already be gone via the happy
    // path above, but a failed assertion partway through must not leak a
    // long-running fake-agent process into every later test.
    for (const title of [titleA, titleB]) {
      const id = await findSessionIdByTitle(request, title).catch(() => undefined);
      if (id) {
        await request.post(`/api/sessions/${id}/stop`).catch(() => {});
        await request.delete(`/api/sessions/${id}`).catch(() => {});
      }
    }
  }
});

// macOS autocorrect mangles create-form input both via suggestion popups
// and silent in-place substitution (observed directly: WKWebView silently
// capitalizing "claude" to "Claude" with no visible popup to catch and
// reject) — a corrupted command or path is not a cosmetic issue, since
// these fields hold literal text that gets executed, not prose. All three
// inputs opt out of every browser-level text-mangling feature; this test
// pins that the opt-out attributes actually made it into the rendered DOM
// (a Dioxus rsx typo or a dropped attribute would otherwise silently leave
// autocorrect back on) rather than exercising an OS-level autocorrect
// engine itself, which is not something Playwright's headless Chromium
// runs at all.
test("create form inputs opt out of autocomplete, autocorrect, autocapitalize, and spellcheck", async ({
  page,
}) => {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();

  const inputs = form.locator('input[type="text"]');
  await expect(inputs).toHaveCount(3);
  for (let i = 0; i < 3; i++) {
    const input = inputs.nth(i);
    await expect(input).toHaveAttribute("autocomplete", "off");
    await expect(input).toHaveAttribute("autocorrect", "off");
    await expect(input).toHaveAttribute("autocapitalize", "none");
    await expect(input).toHaveAttribute("spellcheck", "false");
  }
});

// SPEC.md's precondition-failure split for creation: a bad working
// directory must fail the create with the supervisor's OWN error text,
// leave the form open with its values intact (so the user can fix the one
// wrong field rather than retyping everything), and must not leave a
// session behind. The exact "does not exist" wording is the same text
// pinned at the HTTP level by `create API reports a precondition failure
// containing the supervisor's own text` above; this test is the UI's
// obligation to actually SHOW that text rather than swallowing it. It
// goes one step further than "preserved": it actually fixes the one wrong
// field and resubmits, proving the form is genuinely usable again
// afterward — not merely that its stale values are still visible — which
// is also the only thing in this file that pins `submitting` resetting to
// `false` on the failure path.
test("create dialog surfaces a precondition failure, preserves the form, and creates no session", async ({
  page,
  request,
}) => {
  const title = `create-failure-${Date.now()}`;
  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/nonexistent/definitely/not/here",
      invocation: "true",
      title,
    });
    await form.locator('button[type="submit"]').click();

    await expect(form.locator(".create-session-error")).toContainText(
      "does not exist",
    );
    // Preserved, not cleared or reset: the same values the user typed.
    await expect(form.locator('input[type="text"]').nth(0)).toHaveValue(
      "/nonexistent/definitely/not/here",
    );
    await expect(form.locator('input[type="text"]').nth(1)).toHaveValue("true");
    await expect(form.locator('input[type="text"]').nth(2)).toHaveValue(title);
    // The form itself stayed open (a failed create must not silently
    // close it and strand the user with no visible cause).
    await expect(form).toBeVisible();

    const listing = await (await request.get("/api/sessions")).json();
    expect(listing.sessions.some((s: any) => s.title === title)).toBe(false);

    // The other half of "preserved, not stuck": fixing the one wrong
    // field and resubmitting must actually succeed, which pins that
    // `submitting` was reset to `false` on the failure path (the
    // double-submission guard in `CreateSessionForm`'s `onsubmit` would
    // otherwise leave the control permanently disabled after its first,
    // failed attempt).
    await form.locator('input[type="text"]').nth(0).fill("/tmp");
    await form.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await expect(page.locator(".titlebar .title")).toHaveText(title);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// Double-submission guard (SPEC.md: "one intended create yields one
// session or a clear error, never two silently"): the submit control must
// be disabled for the WHOLE round trip, not just synchronously after the
// click handler returns. A normal create is too fast to observe that
// window reliably, so this delays the POST response by a fixed, short
// amount via route interception — long enough to deterministically
// observe the disabled state, short enough to keep the test fast. Only
// POST is intercepted (GET keeps flowing straight through) so the list's
// own background polling is unaffected. Also covers the two OTHER controls
// this same in-flight `submitting` flag locks: the "new session" toggle
// (which would otherwise unmount the form mid-POST) and every row's open
// button (which would otherwise unmount `ListView` itself mid-POST) — see
// `nav_locked`'s docs in lib.rs for why opening ANY row is unsafe here,
// not just a hypothetically "related" one.
test("create dialog disables the submit control while a create is in flight", async ({
  page,
  request,
}) => {
  const title = `double-submit-${Date.now()}`;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 800));
    await route.continue();
  });

  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    const submit = form.locator('button[type="submit"]');
    await submit.click();

    // The delayed POST is still in flight here — this is exactly the
    // window a double-click or a timeout-triggered retry would otherwise
    // land a second request into.
    await expect(submit).toBeDisabled();
    // The "new session" toggle is ALSO disabled for the same window: it
    // is this form's only cancel/close affordance, and toggling
    // `show_create` off while the create is in flight would unmount
    // `CreateSessionForm` mid-`spawn`, stranding the POST's eventual
    // response with nothing left to act on it (see the toggle button's
    // own doc in lib.rs).
    await expect(page.locator(".new-session-button")).toBeDisabled();
    // And the row-open guard from the same design: opening the shared
    // session right now would navigate away and unmount `ListView`
    // itself, cancelling this in-flight create exactly the same way —
    // see `nav_locked` in lib.rs.
    await expect(
      sharedSessionRow(page).locator(".session-row-open"),
    ).toBeDisabled();

    // Let the delayed response land: success navigates into the new
    // session's terminal, same as the multi-session flow above.
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true, {
      timeout: 15_000,
    });
    await expect(page.locator(".titlebar .title")).toHaveText(title);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// Clicking delete on an Alive session (SPEC.md's "confirmation that says
// so when anything is still alive") must open the inline confirm prompt
// — `.confirm-consequence` plus `.confirm-title` plus
// `.confirm-delete`/`.confirm-cancel` swapped in for the row's normal
// stop/delete buttons (`SessionRow`'s doc in lib.rs) — rather than calling
// any API immediately. `window.confirm()` used to be the mechanism here;
// it is gone because wry ships no native JS dialogs at all on macOS's
// WKWebView, which made that path silently do nothing on a primary
// target.
test("alive delete opens an inline confirming state with the is-still-running wording and the session title", async ({
  page,
  request,
}) => {
  const title = `confirm-open-${Date.now()}`;
  let deleteRequests = 0;
  await page.route("**/api/sessions/*", async (route) => {
    if (route.request().method() === "DELETE") {
      deleteRequests++;
    }
    await route.continue();
  });

  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    const row = rowByTitle(page, title);
    await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });
    await openRowMenu(row);
    await row.locator(".session-row-delete").click();

    // The prompt carries the exact consequence wording (the untruncatable
    // half, `SessionRow`'s doc in lib.rs) AND, separately, the session's
    // own title (rendered as plain Dioxus text — see that doc for why
    // that alone neutralizes anything the title might contain).
    await expect(row.locator(".confirm-consequence")).toHaveText(
      "still running — deleting kills the agent:",
    );
    await expect(row.locator(".confirm-title")).toHaveText(`"${title}"`);
    // Normal buttons are gone while confirming, not merely hidden behind
    // the prompt — `SessionRow` swaps them out entirely.
    await expect(row.locator(".session-row-stop")).toHaveCount(0);
    await expect(row.locator(".session-row-delete")).toHaveCount(0);
    // The open button stays present and visible but disabled (cancel is
    // the only way back to normal, not an implicit click on open — see
    // `SessionRow`'s doc). The confirm prompt lives in the floating
    // actions panel now, so it no longer competes with the open button
    // for the row's space — the MT-8 overflow that once forced the
    // button to be hidden outright cannot recur by construction.
    await expect(row.locator(".session-row-open")).toBeDisabled();
    await expect(row.locator(".session-row-open")).toBeVisible();
    expect(deleteRequests).toBe(0);

    // Cancel: the row returns to normal, with no DELETE ever sent and the
    // session still listed and alive — not just "not yet deleted" (which
    // a bug that deleted on a timer, or deleted regardless of the
    // confirmation's answer after some delay, could also satisfy).
    await row.locator(".confirm-cancel").click();
    await expect(row.locator(".confirm-consequence")).toHaveCount(0);
    await expect(row.locator(".session-row-stop")).toBeEnabled();
    await expect(row.locator(".session-row-delete")).toBeEnabled();
    await expect(row.locator(".session-row-open")).toBeEnabled();
    expect(deleteRequests).toBe(0);
    await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE);
    const listing = await (await request.get("/api/sessions")).json();
    const session = listing.sessions.find((s: any) => s.title === title);
    expect(session).toBeTruthy();
    expect(LIVE_STATES).toContain(session.status.state);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// The other half of the confirm flow: clicking "confirm delete" performs
// exactly the DELETE the old accepted `window.confirm()` used to trigger —
// pinned here as EXACTLY one DELETE request, the same request-counting
// pattern the cancel test above uses to pin exactly zero.
test("confirming an inline delete prompt deletes the session with exactly one DELETE request", async ({
  page,
  request,
}) => {
  const title = `confirm-delete-${Date.now()}`;
  let deleteRequests = 0;
  await page.route("**/api/sessions/*", async (route) => {
    if (route.request().method() === "DELETE") {
      deleteRequests++;
    }
    await route.continue();
  });

  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    const row = rowByTitle(page, title);
    await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });
    await openRowMenu(row);
    await row.locator(".session-row-delete").click();
    await expect(row.locator(".confirm-consequence")).toBeVisible();
    await row.locator(".confirm-delete").click();

    await expect(row).toHaveCount(0, { timeout: 10_000 });
    expect(deleteRequests).toBe(1);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// Exited sessions are the one status that never confirms at all (SPEC.md:
// delete confirms only when something might still be alive) — this pins
// that directly, rather than relying on it as a side effect of the
// multi-session flow test above.
test("exited session deletes immediately with no confirming state", async ({
  page,
  request,
}) => {
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "true" },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  // Stalled, not answered instantly: a bare post-click check for the
  // prompt's absence cannot distinguish "never appeared" from "appeared
  // and vanished again before this check ran" — a real gap for a status
  // this fast to slip through undetected. Holding the DELETE response
  // open keeps the row on screen long enough to make the absence
  // assertion actually mean something, then releases it to let the
  // delete complete normally.
  let releaseDelete: () => void = () => {};
  const deleteHeld = new Promise<void>((resolve) => {
    releaseDelete = resolve;
  });
  await page.route(`**/api/sessions/${id}`, async (route) => {
    if (route.request().method() !== "DELETE") {
      await route.continue();
      return;
    }
    await deleteHeld;
    await route.continue();
  });

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText(/^exited/, {
      timeout: 10_000,
    });

    await openRowMenu(row);
    await row.locator(".session-row-delete").click();
    // The DELETE is stalled, so the row is still here — and, while it
    // is, the confirm prompt has never appeared at all, a synchronous
    // property of `on_delete`'s Exited arm (see lib.rs), not merely a
    // narrow timing window this stall makes easier to hit by luck.
    await expect(row).toHaveCount(1);
    await expect(row.locator(".confirm-consequence")).toHaveCount(0);

    releaseDelete();
    await expect(row).toHaveCount(0, { timeout: 10_000 });
  } finally {
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// A session's title is untrusted data (a supervisor over `--ssh` is a
// different, possibly compromised host) that can legally contain anything,
// including markup — and the confirm prompt is rendered by ordinary Dioxus
// text interpolation straight to a DOM text node (`SessionRow`'s doc in
// lib.rs), which is what actually neutralizes it. The OLD eval-based path
// needed `serde_json::to_string`-encoding to stop a title from breaking
// out of a JS *string literal*; that whole concern is gone along with the
// eval call. The risk that remains is a DIFFERENT regression class:
// something along this render path someday using
// `innerHTML`/`dangerouslySetInnerHTML`-style markup injection instead of
// a text node, which would parse a title as HTML rather than display it
// as text.
//
// Two INDEPENDENT oracles cover that risk, deliberately, rather than
// relying on either alone: the exact `toHaveText` checks below would
// already catch MOST such a regression — a title parsed as markup would
// render a broken `<img>` icon, not the literal `<img src=x
// onerror="...">` text this asserts verbatim — but `toHaveText` only
// proves the WRONG output didn't happen, not that nothing executed;
// asserting `__pwned` stays unset is a genuinely separate signal (was
// anything ever RUN), immune to a hypothetical bug where broken markup
// happened to still leave matching text behind. Together they cover both
// "did the display come out right" and "did anything execute", neither
// implied by the other.
test("delete confirmation safely displays a title containing executable HTML without ever parsing it as markup", async ({
  page,
  request,
}) => {
  const title = `inject-${Date.now()}-<img src=x onerror="window.__pwned=1">`;
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300", title },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });
    await openRowMenu(row);
    await row.locator(".session-row-delete").click();

    await expect(row.locator(".confirm-consequence")).toHaveText(
      "still running — deleting kills the agent:",
    );
    await expect(row.locator(".confirm-title")).toHaveText(`"${title}"`);
    // The title's own row (open button) renders the same untrusted string
    // too — checked here as well, since it is a second, independent
    // render site for the exact same data.
    await expect(row.locator(".session-title")).toHaveText(title);
    expect(await page.evaluate(() => (window as any).__pwned)).toBeUndefined();
  } finally {
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// A session's title has no length limit the UI enforces (a legal title
// can run tens of KB — see the existing overflow comment on
// `.session-title` in app.css). Unlike the SPACE-CONTAINING metadata that
// overflow handling elsewhere in this file exercises, a title with NO
// whitespace at all cannot wrap or break naturally — CSS's default
// min-content floor would let such a title claim its own full rendered
// width and push everything after it off the visible row, which is
// exactly what `.confirm-title`'s `min-width: 0` (app.css) exists to
// prevent. `.confirm-consequence`, in contrast, must NEVER be the one
// that gives: it is the safety-critical "will be killed" half, rendered
// as its own untruncatable element specifically so a long title can never
// clip it (`confirm_consequence`'s doc in lib.rs).
//
// This pins the actual CONTRACT, not just an emergent side effect: the
// consequence text renders in full (exact match, not `toContainText`),
// the title element is genuinely being clipped (not merely short enough
// to fit), both buttons stay on screen and don't overlap the title, and
// both buttons keep their own declared `flex-shrink: 0` — checked via
// computed style directly, since that is the one assertion that fails
// immediately and deterministically if a future edit ever drops that
// declaration, independent of whatever the emergent flex arithmetic at
// this particular viewport width happens to produce.
//
// Created via the raw API (a create-FORM round trip through this much
// text would only slow the test down, not exercise anything the API path
// doesn't already), then asserted in the browser's actual layout engine,
// not just in the CSS source.
test("a legal multi-KB, unbroken title keeps the consequence text intact and clips only the title, without disturbing the confirm/cancel buttons", async ({
  page,
  request,
}) => {
  const hugeTitle = "x".repeat(20_000);
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300", title: hugeTitle },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });
    await openRowMenu(row);
    await row.locator(".session-row-delete").click();

    // The safety-critical consequence half renders in full, exact text —
    // not merely "contains", so any accidental truncation (an ellipsis
    // rule wrongly applied to THIS element instead of just the title)
    // fails immediately.
    await expect(row.locator(".confirm-consequence")).toHaveText(
      "still running — deleting kills the agent:",
    );

    // The title itself IS being clipped — proving the overflow/min-width
    // CSS is actually doing its job on a title this large, not merely
    // absent because a shorter title happened to fit anyway.
    const titleOverflowing = await row
      .locator(".confirm-title")
      .evaluate((el) => el.scrollWidth > el.clientWidth);
    expect(titleOverflowing).toBe(true);

    // Both buttons stay on screen, reachable...
    await expect(row.locator(".confirm-delete")).toBeInViewport();
    await expect(row.locator(".confirm-cancel")).toBeInViewport();
    // ...and undisturbed by the (massively wide, if unclipped) title — a
    // real geometry check, not just individual visibility. The confirm
    // lives in the floating actions panel, a COLUMN: the title gets a
    // line of its own ABOVE the buttons and must ellipsize inside the
    // panel's width rather than forcing the panel (and the buttons' line)
    // wide or painting over them.
    const [panelBox, titleBox, confirmBox, cancelBox] = await Promise.all([
      row.locator(".session-row-menu-panel").boundingBox(),
      row.locator(".confirm-title").boundingBox(),
      row.locator(".confirm-delete").boundingBox(),
      row.locator(".confirm-cancel").boundingBox(),
    ]);
    expect(panelBox).not.toBeNull();
    expect(titleBox).not.toBeNull();
    expect(confirmBox).not.toBeNull();
    expect(cancelBox).not.toBeNull();
    /** All four edges inside the container — one-edge checks let a box
     * overlap a sibling or escape on an unchecked side and still pass. */
    const inside = (name: string, box: NonNullable<typeof panelBox>) => {
      expect(box.x, `${name} left edge`).toBeGreaterThanOrEqual(panelBox!.x - 1);
      expect(box.x + box.width, `${name} right edge`).toBeLessThanOrEqual(
        panelBox!.x + panelBox!.width + 1,
      );
      expect(box.y, `${name} top edge`).toBeGreaterThanOrEqual(panelBox!.y - 1);
      expect(box.y + box.height, `${name} bottom edge`).toBeLessThanOrEqual(
        panelBox!.y + panelBox!.height + 1,
      );
    };
    // The clipped title stays fully inside the panel WITH a real height:
    // the shared `.confirm-title` rule's zero flex basis once collapsed
    // it to an invisible zero-height box in this column container (the
    // panel-scoped `flex: none` in app.css is the fix this pins).
    inside("title", titleBox!);
    expect(titleBox!.height).toBeGreaterThan(5);
    // On its own line above both buttons, never overlapping them...
    expect(titleBox!.y + titleBox!.height).toBeLessThanOrEqual(confirmBox!.y + 1);
    // ...the buttons stacked in order (confirm above cancel), each fully
    // inside the panel and not overlapping each other.
    inside("confirm", confirmBox!);
    inside("cancel", cancelBox!);
    expect(confirmBox!.y + confirmBox!.height).toBeLessThanOrEqual(cancelBox!.y + 1);
  } finally {
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// The confirming state lives in `ListView`'s own client-side signal, keyed
// by session id (see `confirming`'s doc in lib.rs) — a listing refresh
// refetches and re-renders the whole listing for reasons the user did not
// cause, and must not silently revert an in-progress confirmation out from
// under them.
//
// A distinguishable field on a LATER response — not merely a counted
// request — is what actually proves a real refetch's RESULT reached the
// DOM: counting requests alone cannot rule out a regression that fires the
// request but never applies its response (a dropped `listing.set`, a
// silently-ignored decode failure), which would still increment a request
// counter while never actually re-rendering anything. Route-intercepting
// the GET with a synthetic listing carrying a marker invocation is what
// turns "a refresh happened" into "a refresh's response was applied and
// rendered" — but the marker is only armed AFTER the confirm prompt is
// already open, not from page load onward: arming it up front would let the
// marker show up as a leftover of the FIRST fetch (the one that populates
// the initial list, before any click), which would pass this test even if
// nothing ever landed again while confirming — exactly the false positive
// this ordering exists to rule out.
//
// The refresh used to be M2's three-second poll and is now a feed
// notification (PLAN_M6_75.md item 6), which is a change of TRIGGER and
// nothing else as far as this rule is concerned. Stubbed rather than left
// to the real feed: the moment the re-render happens has to be strictly
// after the marker is armed, and a shared stack bumps its revision whenever
// it likes.
test("an inline confirming state survives a listing refresh; cancel still works afterward", async ({
  page,
  request,
}) => {
  const title = `confirm-survives-poll-${Date.now()}`;
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300", title },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();
  const marker = "poll-marker-invocation";

  // Baseline listing (the session's real invocation) until `markerArmed`
  // flips — see the comment above for why arming has to wait.
  let markerArmed = false;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        sessions: [
          {
            id,
            title,
            cwd: "/tmp",
            invocation: markerArmed ? marker : "sleep 300",
            status: { state: "running" },
          },
        ],
        total: 1,
        truncated: false,
      }),
    });
  });

  const feed = await stubFeed(page);
  try {
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });
    await openRowMenu(row);
    await row.locator(".session-row-delete").click();
    await expect(row.locator(".confirm-title")).toHaveText(`"${title}"`);

    // Only NOW does the route start serving the marker — strictly after
    // the prompt is already open, so the marker appearing can only be
    // the result of a refresh that happened WHILE confirming, not the
    // initial page-load fetch.
    markerArmed = true;
    feed.notify(2);

    // The marker invocation only ever appears once THIS route's synthetic
    // response has actually been fetched, decoded, and rendered — proof
    // the refresh's result reached the DOM, not just that a request fired.
    await expect(row.locator(".session-invocation")).toHaveText(marker, {
      timeout: 10_000,
    });

    // Still confirming, still the same wording and title — a refresh must
    // not have cleared it (nor silently deleted anything: no DELETE was
    // ever confirmed).
    await expect(row.locator(".confirm-title")).toHaveText(`"${title}"`);
    await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE);

    await row.locator(".confirm-cancel").click();
    await expect(row.locator(".confirm-consequence")).toHaveCount(0);
    await expect(row.locator(".session-row-delete")).toBeEnabled();
  } finally {
    await page.unroute("**/api/sessions");
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// Confirming is per-row state, not global (see `confirming`'s doc in
// lib.rs): a delete click on one row must never bleed into another row's
// buttons, the same "per-session, not one shared slot" property `errors`
// and `pending` already have their own dedicated tests for above.
test("one row's confirming state does not affect another row's controls", async ({
  page,
  request,
}) => {
  const createdA = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  const createdB = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(createdA.status()).toBe(200);
  expect(createdB.status()).toBe(200);
  const { id: idA } = await createdA.json();
  const { id: idB } = await createdB.json();

  try {
    await page.goto("/");
    const rowA = page.locator(`[data-session-id="${idA}"]`);
    const rowB = page.locator(`[data-session-id="${idB}"]`);
    await expect(rowA.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });
    await expect(rowB.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });

    await openRowMenu(rowA);
    await rowA.locator(".session-row-delete").click();
    await expect(rowA.locator(".confirm-consequence")).toBeVisible();

    // B is completely untouched: opening ITS menu (which, menus being a
    // one-at-a-time slot, closes A's panel) shows the normal stop/delete
    // pair, both enabled, and no confirm prompt of its own. The slot
    // itself is asserted, not just implied: A's panel must be GONE and
    // its toggle must say so — two independently open menus would pass
    // every other check here.
    await openRowMenu(rowB);
    await expect(rowA.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(rowA.locator(".session-row-menu")).toHaveAttribute("aria-expanded", "false");
    await expect(rowB.locator(".confirm-consequence")).toHaveCount(0);
    await expect(rowB.locator(".session-row-stop")).toBeEnabled();
    await expect(rowB.locator(".session-row-delete")).toBeEnabled();
    await expect(rowB.locator(".session-row-open")).toBeEnabled();

    // A's confirming state is per-row and independent of which panel is
    // open: reopening A's menu must land back on the pending prompt, not
    // reset to the action items — and take the one-open slot back from B.
    await openRowMenu(rowA);
    await expect(rowB.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(rowA.locator(".confirm-consequence")).toBeVisible();
    await rowA.locator(".confirm-cancel").click();

    // The toggle's own close branch, exercised directly: clicking an
    // already-open menu's toggle closes it rather than reopening.
    await rowA.locator(".session-row-menu").click();
    await expect(rowA.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(rowA.locator(".session-row-menu")).toHaveAttribute("aria-expanded", "false");
  } finally {
    for (const id of [idA, idB]) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// Confirming is `ListView`'s own state, decoupled from the status that
// triggered it (see `confirming`'s doc in lib.rs): a status change under
// an open confirm prompt — this session getting stopped from another
// client, say — must not silently close the prompt or swap back to the
// normal stop/delete pair. `confirm_consequence`'s wording is,
// deliberately, NOT frozen at the moment the prompt opened: it recomputes
// from whatever status the row's LATEST render carries (see that
// function's own doc), and its `Exited` arm exists specifically for this
// transition — a residual case, not dead code, so this pins its exact
// fallback wording rather than leaving it unexercised by anything in this
// suite. The title element is unaffected by any of this (the status
// change touches only the consequence text), so it is checked once,
// before the transition, rather than redundantly re-checked after.
test("an alive-to-exited status change under an open confirm prompt keeps confirming, with the fallback wording", async ({
  page,
  request,
}) => {
  const title = `alive-to-exited-${Date.now()}`;
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300", title },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });
    await openRowMenu(row);
    await row.locator(".session-row-delete").click();
    await expect(row.locator(".confirm-consequence")).toContainText("running");
    await expect(row.locator(".confirm-title")).toHaveText(`"${title}"`);

    // Stopped from "elsewhere" (the raw API, standing in for another
    // client) while this row's prompt sits open.
    await request.post(`/api/sessions/${id}/stop`);

    // The stop bumps the helm's revision, so the real feed carries it here
    // without anything in this test playing helm: the re-read it triggers
    // picks the exited status up and re-words the SAME open prompt — it does
    // not close it, and does not swap back to the normal stop/delete pair.
    await expect(row.locator(".confirm-consequence")).toHaveText(
      "delete anyway:",
      { timeout: 10_000 },
    );
    // Cancel's continued presence is the interesting half here — proving
    // the row is still genuinely IN the confirming state, not merely that
    // SOME element with that text exists; confirm-delete is about to be
    // clicked below, so Playwright's own actionability wait already
    // covers its visibility.
    await expect(row.locator(".confirm-cancel")).toBeVisible();
    await expect(row.locator(".session-row-stop")).toHaveCount(0);

    await row.locator(".confirm-delete").click();
    await expect(row).toHaveCount(0, { timeout: 10_000 });
  } finally {
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// A failed listing read (`fetch_sessions` failing) swaps the WHOLE list
// view for an error banner (`ListView`'s `Some(Err(e))` render arm) rather
// than leaving stale rows on screen — which means a row's `confirming`
// entry has nothing left to render into for as long as that banner is
// showing. This pins that the entry itself, held in `ListView`'s own state
// independent of any particular render, survives that gap intact and
// reappears the moment the list recovers — a bare "the request count went
// up" would not prove this, since it says nothing about whether the confirm
// prompt for THIS id came back correctly afterward.
//
// The read that fails is a feed-triggered one now rather than a poll, and
// the failure is a LATCH the test opens and closes rather than the one-shot
// it used to be. Both changes are about controlling the moment: the page
// re-reads when this test says so, and the error state ends when this test
// says so — a one-shot failure would be repaired by the reader's own retry
// at a moment nothing here chose, which is exactly the sort of race that
// makes a transient assertion flake.
test("a failed listing read while confirming does not clear the confirming state", async ({
  page,
  request,
}) => {
  const title = `read-error-while-confirming-${Date.now()}`;
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300", title },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  // Armed only once the confirm prompt is genuinely open (below) — not from
  // the start: arming up front would fail the page's own mount read, which
  // has nothing to do with confirming at all.
  let failing = false;
  await page.route("**/api/sessions", async (route) => {
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
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });
    await openRowMenu(row);
    await row.locator(".session-row-delete").click();
    await expect(row.locator(".confirm-title")).toHaveText(`"${title}"`);

    // Only NOW does a listing read fail — strictly after the prompt is
    // confirmed open, and only because this test asked for a read.
    failing = true;
    feed.notify(2);

    // The failed fetch swaps the list view for an error banner — this IS
    // that transient state, not a bug this test is tripping over.
    await expect(page.locator(".status.error")).toBeVisible({
      timeout: 10_000,
    });

    // The next read succeeds; the list — and the SAME confirming prompt,
    // restored from `ListView`'s own state rather than anything baked
    // into this particular render — comes back. The reader retries a failed
    // read on its own, so the notification here is belt and braces rather
    // than the only way back.
    failing = false;
    feed.notify(3);
    await expect(row.locator(".confirm-title")).toHaveText(`"${title}"`, {
      timeout: 10_000,
    });
    await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE);
  } finally {
    await page.unroute("**/api/sessions");
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// The other confirm-wording branch: a session whose status is Unknown
// (rather than a known-alive one) must ALSO confirm before deleting, but
// with wording that admits uncertainty rather than borrowing the Alive
// branch's "is still running" claim — SPEC.md's no-guessing rule applies
// to this confirm text exactly as it does to the status badge itself.
// Driven through a synthetic, route-intercepted listing (like the
// truncation-banner test above) rather than a real session: a supervisor
// restart is NOT how to provoke this — PLAN_M2.md's restart-gap behavior
// yields `Exited { exit_code: None }` when tmux did not survive (an
// explicit "known dead, unknown code", not "unknown whether alive" — see
// `SessionStatus::Exited` in lib.rs), and ordinary `Alive`/`Exited` when
// it did. Genuine `Unknown` only ever comes from `Session::status`'s
// serde default kicking in on an old-shaped reply with no `status` field
// PLAN_M6_75.md item 3's no-badge-until-classified rule, on the CREATE
// path — the half with no prior status to fall back on.
//
// A create establishes that the session and its terminal exist, not that
// anything has classified the agent inside it, so the first listings after
// one can legitimately carry no status at all. What the row must show for
// that window is NOTHING: no badge element, not the word "unknown", which
// would read as a verdict about the agent rather than an admission that
// the system has not looked yet. Then, when a status does arrive, the
// badge appears.
//
// Route-controlled rather than driven through a real create, because the
// window this is about is precisely the one a real stack closes as fast as
// it can — a real create's row is classified within a poll or two, and a
// test that raced that would pass by luck on a fast machine and fail on a
// slow one. Holding the row unclassified for as long as the assertions
// need is the only way to assert on the window at all.
//
// The RESTART path's counterpart is a different mechanism entirely (the
// helm's never-overwrite-definite merge, which keeps the previous status
// on screen) and gets its own real-stack test further down.
test("create-shows-no-badge-until-a-status-is-classified", async ({ page }) => {
  const sessionId = "unclassified-create-session";
  let classified = false;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        sessions: [
          {
            id: sessionId,
            title: sessionId,
            cwd: "/tmp",
            invocation: "agent",
            // No "status" key at all until `classified` flips — exactly
            // what a session nothing has looked at yet decodes as.
            ...(classified ? { status: { state: "running" } } : {}),
          },
        ],
        total: 1,
        truncated: false,
      }),
    });
  });

  // Stubbed, because both halves of this test are about what a REFRESH
  // does: the badge staying absent across one, and then appearing on one.
  // Nothing refreshes a healthy page on its own any more (PLAN_M6_75.md
  // item 6), so the refreshes have to be this test's to trigger.
  const feed = await stubFeed(page);
  await page.goto("/");
  await feed.waitForConnection(1);
  feed.notify(1);
  const row = page.locator(`[data-session-id="${sessionId}"]`);
  // The ROW renders fully — this is a missing badge, not a missing row,
  // and the difference is the whole point: an unclassified session is
  // still listed, still openable, still stoppable.
  await expect(row.locator(".session-title")).toHaveText(sessionId);
  await expect(row.locator(".session-cwd")).toHaveText("/tmp");
  await expect(row.locator(".status-badge")).toHaveCount(0);
  // Held across a listing refresh, so this is "no badge for as long as the
  // status is unknown" rather than "no badge in the instant we looked".
  feed.notify(2);
  await page.waitForTimeout(3_000);
  await expect(row.locator(".status-badge")).toHaveCount(0);

  classified = true;
  feed.notify(3);
  await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE, {
    timeout: 15_000,
  });
});

// The stale SESSION VIEW's half of the same rule (PLAN_M6_75.md item 3).
//
// SPEC.md's metadata triple for a session on an unreachable host is
// "title, directory, last-known status" — but a session whose status was
// never classified has no last-known status to show, and the view must
// then show the other two and no badge rather than inventing a word. The
// fleet-level stale test further down covers the ordinary case (a status
// the helm did observe before the host went away); this covers the case
// that test deliberately waits past.
//
// Route-controlled for the same reason as its create-path sibling: the
// real fleet test has to WAIT for a status precisely because provoking the
// absence of one is not something a healthy stack does on request.
test("stale-session-view-with-no-status-renders-metadata-and-no-badge", async ({
  page,
}) => {
  const sessionId = "unclassified-stale-session";
  const listing = {
    sessions: [
      {
        id: sessionId,
        title: sessionId,
        cwd: "/tmp",
        invocation: "sleep 600",
        stale: true,
        host_name: "far-host",
        // Again: no status key at all.
      },
    ],
    total: 1,
    truncated: false,
  };
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(listing),
    });
  });
  await page.route(`**/api/sessions/${sessionId}`, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(listing.sessions[0]),
    });
  });

  await page.goto("/");
  await page.locator(`[data-session-id="${sessionId}"] .session-row-open`).click();

  await expect(page.locator(".titlebar .title")).toHaveText(sessionId);
  await expect(page.locator(".titlebar .meta")).toHaveText("/tmp — sleep 600");
  await expect(page.locator(".stale-metadata .status-badge")).toHaveCount(0);
  // The notice itself still renders: the point is a MISSING badge inside a
  // present stale surface, not a stale surface that failed to appear.
  await expect(page.locator(".host-stale-notice")).toBeVisible();
});

// at all (see that derive's own docs) — i.e. an old PEER, not a restart of
// this same build's own supervisor — which is not something this suite's
// single, current-build stack can produce, hence the synthetic listing.
test("deleting a session with unknown status confirms first, with wording that admits uncertainty", async ({
  page,
}) => {
  const sessionId = "unknown-status-session";
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        sessions: [
          {
            id: sessionId,
            title: sessionId,
            cwd: "/tmp",
            invocation: "true",
            // No "status" field at all — exactly what decodes as Unknown
            // per `Session::status`'s own serde default in lib.rs.
          },
        ],
        total: 1,
        truncated: false,
      }),
    });
  });
  let deleteRequests = 0;
  await page.route(`**/api/sessions/${sessionId}`, async (route) => {
    if (route.request().method() === "DELETE") {
      deleteRequests++;
    }
    await fulfillAsHelm(route, { status: 200, contentType: "application/json", body: "{}" });
  });

  await page.goto("/");
  const row = page.locator(`[data-session-id="${sessionId}"]`);
  // PLAN_M6_75.md item 3's no-badge-until-classified rule, at the level
  // that can actually see the DOM: an unclassified status paints NO badge
  // element — not the word "unknown", not an empty box. The row is still
  // fully rendered (its title is asserted below), so this is a missing
  // badge rather than a missing row.
  await expect(row.locator(".session-title")).toHaveText(sessionId);
  await expect(row.locator(".status-badge")).toHaveCount(0);
  await openRowMenu(row);
  await row.locator(".session-row-delete").click();

  // Confirms before any DELETE: since there is no async eval in the way
  // anymore, the assertion right after the click already proves ordering
  // (the click handler's whole synchronous body only ever inserts into
  // `confirming` for this status — see `on_delete` in lib.rs — so a
  // DELETE this soon could only come from a regression that skipped
  // confirmation outright).
  await expect(row.locator(".confirm-consequence")).toHaveText(
    "status unknown — the agent may still be running and will be killed:",
  );
  await expect(row.locator(".confirm-title")).toHaveText(`"${sessionId}"`);
  expect(deleteRequests).toBe(0);

  await row.locator(".confirm-delete").click();
  await expect.poll(() => deleteRequests).toBe(1);
});

// Double-submission guard, taken one step further than the disabled-button
// test above: that test only proves the CONTROL looks disabled, which a
// user could still defeat with a second Enter keypress landing on the form
// itself rather than the button, or any other path that dispatches a
// native `submit` event without going through the (disabled) button.
// `HTMLFormElement.requestSubmit()` is exactly such a path — it fires a
// real `submit` event the disabled button cannot intercept — so a second
// call here is what actually pins the RUST-SIDE `submitting` guard, not
// merely the disabled attribute's cosmetic effect.
test("submitting the create form twice while one create is in flight produces exactly one session", async ({
  page,
  request,
}) => {
  const title = `double-submit-guard-${Date.now()}`;
  let postCount = 0;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    postCount++;
    await new Promise((resolve) => setTimeout(resolve, 500));
    await route.continue();
  });

  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    // Bypasses the (already disabling) submit button entirely.
    await page.evaluate(() => {
      document
        .querySelector<HTMLFormElement>(".create-session-form")
        ?.requestSubmit();
    });

    // Auto-select means __farhelmTermReady is ALREADY true (the page
    // attached a session at load), so it cannot gate on the create any
    // more; the created session's own title taking the pane is what
    // proves the POST completed and creation selected it.
    await expect(page.locator(".titlebar .title")).toHaveText(title, { timeout: 15_000 });
    expect(postCount).toBe(1);

    const listing = await (await request.get("/api/sessions")).json();
    expect(listing.sessions.filter((s: any) => s.title === title)).toHaveLength(1);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// Per-session error surfacing: a stop or delete failure must render in
// THAT row's own error line with the server's actual text, without
// disturbing the row itself (no vanishing, no badge lying) or the rest of
// the list. Both failures here happen on the SAME session, one after the
// other — proving errors are keyed by session at all (a failure on one
// session must not touch another's error line) is a separate concern,
// covered by "a failed action's error is keyed to its own session, not
// shared across rows" below. Route-intercepted with distinct sentinel
// bodies for stop and delete so each assertion can tell exactly which
// call produced which text.
test("stop and delete failures surface in the row's own error line, without disturbing the rest of the list", async ({
  page,
  request,
}) => {
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });

    await page.route(`**/api/sessions/${id}/stop`, (route) =>
      fulfillAsHelm(route, {
        status: 500,
        contentType: "text/plain",
        body: "stop-failure-sentinel",
      }),
    );
    await openRowMenu(row);
    await row.locator(".session-row-stop").click();
    await expect(row.locator(".action-error")).toContainText(
      "stop-failure-sentinel",
    );
    // Scoped to this row and this action: no optimistic flip either way,
    // and the rest of the list keeps working normally.
    await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE);
    await expect(page.locator(".session-list")).toBeVisible();
    await page.unroute(`**/api/sessions/${id}/stop`);

    await page.route(`**/api/sessions/${id}`, async (route) => {
      if (route.request().method() !== "DELETE") {
        await route.continue();
        return;
      }
      await fulfillAsHelm(route, {
        status: 500,
        contentType: "text/plain",
        body: "delete-failure-sentinel",
      });
    });
    // Still alive, so delete opens the inline confirm prompt first —
    // click through it the same way a real user would.
    await openRowMenu(row);
    await row.locator(".session-row-delete").click();
    await row.locator(".confirm-delete").click();
    await expect(row.locator(".action-error")).toContainText(
      "delete-failure-sentinel",
    );
    // A failed delete must not vanish the row.
    await expect(row).toHaveCount(1);
    await page.unroute(`**/api/sessions/${id}`);
  } finally {
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// The other half of "per-session, not one shared slot" (see `errors`'s
// own docs in lib.rs): a failure on session A must not just render in A's
// own row (already covered above) but must ALSO survive an unrelated
// SUCCESS on session B untouched, and B must pick up no error of its own
// from any of it. A single shared `Option<String>` would have failed this
// in either direction — B's success clearing A's error, or A's failure
// somehow bleeding into B's row. Finishes by retrying A (now
// unintercepted) to confirm a later SUCCESS clears only A's own entry.
test("a failed action's error is keyed to its own session, not shared across rows", async ({
  page,
  request,
}) => {
  const createdA = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  const createdB = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(createdA.status()).toBe(200);
  expect(createdB.status()).toBe(200);
  const { id: idA } = await createdA.json();
  const { id: idB } = await createdB.json();

  try {
    await page.goto("/");
    const rowA = page.locator(`[data-session-id="${idA}"]`);
    const rowB = page.locator(`[data-session-id="${idB}"]`);
    await expect(rowA.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });
    await expect(rowB.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });

    // A's stop fails (route-intercepted); B's stop is real and succeeds.
    await page.route(`**/api/sessions/${idA}/stop`, (route) =>
      fulfillAsHelm(route, {
        status: 500,
        contentType: "text/plain",
        body: "error-a-sentinel",
      }),
    );
    await openRowMenu(rowA);
    await rowA.locator(".session-row-stop").click();
    await expect(rowA.locator(".action-error")).toContainText("error-a-sentinel");

    await openRowMenu(rowB);
    await rowB.locator(".session-row-stop").click();
    // "exited — stopped by user": the durable stop annotation qualifies
    // the exited badge (PLAN_M3.md item 4, SPEC.md's "'stopped' is not a
    // distinct status").
    await expect(rowB.locator(".status-badge")).toHaveText(
      /^exited — stopped by user/,
      { timeout: 10_000 },
    );

    // A's error survives B's unrelated success untouched, and B picked up
    // no error of its own from any of this.
    await expect(rowA.locator(".action-error")).toContainText("error-a-sentinel");
    await expect(rowB.locator(".action-error")).toHaveCount(0);

    // Retrying A (now unintercepted) must succeed and clear ONLY A's
    // error — B's (already-empty) state is untouched by this too.
    await page.unroute(`**/api/sessions/${idA}/stop`);
    await openRowMenu(rowA);
    await rowA.locator(".session-row-stop").click();
    await expect(rowA.locator(".status-badge")).toHaveText(
      /^exited — stopped by user/,
      { timeout: 10_000 },
    );
    await expect(rowA.locator(".action-error")).toHaveCount(0);
    await expect(rowB.locator(".action-error")).toHaveCount(0);
  } finally {
    for (const id of [idA, idB]) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// The per-session in-flight guard (`pending` in lib.rs's `ListView`, and
// the GLOBAL `nav_locked` derived from it — see that flag's own docs):
// while session A has a stop or delete running, A's own stop, delete, AND
// open buttons must all be disabled (open via the global nav lock, since
// opening ANY row would unmount `ListView` and cancel A's in-flight op
// just the same), while an unrelated session B's stop and delete stay
// perfectly usable (that half of the guard IS per-session) — and B's OWN
// open button is disabled too, which is the interesting, easy-to-miss
// half of this: the nav lock does not care WHICH session is busy.
test("stop's in-flight guard disables this row's stop, delete, and open, while another row's stop and delete stay usable", async ({
  page,
  request,
}) => {
  const createdA = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  const createdB = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(createdA.status()).toBe(200);
  expect(createdB.status()).toBe(200);
  const { id: idA } = await createdA.json();
  const { id: idB } = await createdB.json();

  // Held until this test releases it, then let through with
  // `route.continue()` — NOT fulfilled here: this route needs the REAL
  // stop to actually reach the supervisor and kill session A's real
  // `sleep 300`, or its badge would never flip to exited below and the
  // test could never distinguish "the guard is working" from "the
  // request never even landed". An explicit release rather than a fixed
  // delay because the in-flight window now has to cover opening B's
  // actions panel between assertions, which takes render round-trips a
  // timer can only outguess.
  let stopRequests = 0;
  let releaseStop: () => void = () => {};
  const stopHeld = new Promise<void>((resolve) => {
    releaseStop = resolve;
  });
  await page.route(`**/api/sessions/${idA}/stop`, async (route) => {
    stopRequests++;
    await stopHeld;
    await route.continue();
  });

  try {
    await page.goto("/");
    const rowA = page.locator(`[data-session-id="${idA}"]`);
    const rowB = page.locator(`[data-session-id="${idB}"]`);
    await expect(rowA.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });
    await expect(rowB.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });

    // The stop button only exists in the DOM while A's actions panel is
    // open — the bare DOM clicks below cannot open it for us.
    await openRowMenu(rowA);

    // Two native clicks dispatched synchronously in the same JS tick:
    // Playwright's own `.click()` waits for an element to be enabled
    // before clicking, which would never let a second click land on an
    // already-disabled button, so it could only ever exercise the
    // `disabled` ATTRIBUTE, not the guard behind it. A bare DOM
    // `.click()` bypasses that actionability wait entirely and is what
    // actually exercises the RUST-SIDE `pending` re-entry guard (the
    // `if !pending.write().insert(...)` check in `on_stop`).
    await page.evaluate((id) => {
      const btn = document.querySelector<HTMLButtonElement>(
        `[data-session-id="${id}"] .session-row-stop`,
      );
      btn?.click();
      btn?.click();
    }, idA);
    // The double click must have produced exactly one request BEFORE the
    // lock assertions run: a silently-missed click (a not-yet-mounted
    // panel button, say) would leave every row idle and let the disabled
    // checks below fail confusingly far from the cause.
    await expect.poll(() => stopRequests).toBe(1);

    // While the delayed stop is in flight: A's own controls are locked...
    await expect(rowA.locator(".session-row-stop")).toBeDisabled();
    await expect(rowA.locator(".session-row-delete")).toBeDisabled();
    await expect(rowA.locator(".session-row-open")).toBeDisabled();
    // ...B's stop/delete (per-session) are unaffected — switching the
    // one-at-a-time menu slot to B's panel is what makes them visible...
    await openRowMenu(rowB);
    await expect(rowB.locator(".session-row-stop")).toBeEnabled();
    await expect(rowB.locator(".session-row-delete")).toBeEnabled();
    // ...but B's open is ALSO disabled — the nav lock is global, not
    // scoped to whichever session happens to be busy.
    await expect(rowB.locator(".session-row-open")).toBeDisabled();

    releaseStop();
    await expect
      .poll(() => rowA.locator(".status-badge").textContent(), {
        timeout: 10_000,
      })
      .toMatch(/^exited — stopped by user/);

    // Everything is usable again once the operation completes, and only
    // ONE request ever reached the route — the second click was rejected
    // by the guard, not merely delayed behind the first. A's panel was
    // displaced by B's above, so bring it back before inspecting it.
    await openRowMenu(rowA);
    await expect(rowA.locator(".session-row-stop")).toBeEnabled();
    await expect(rowA.locator(".session-row-delete")).toBeEnabled();
    await expect(rowA.locator(".session-row-open")).toBeEnabled();
    await expect(rowB.locator(".session-row-open")).toBeEnabled();
    expect(stopRequests).toBe(1);
  } finally {
    // Unconditional: a failed assertion above must not leave the page's
    // stop request parked on the held route.
    releaseStop();
    for (const id of [idA, idB]) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// Cross-guard regression test: a rapid stop/delete pair on the SAME row —
// two native DOM clicks dispatched in the same JS tick, the same
// bare-`.click()` technique the guard test above uses to bypass
// Playwright's own actionability wait — must never let `on_stop` and the
// delete-confirm flow interleave badly, in EITHER click order:
//   - delete-then-stop: without `on_stop`'s own `confirming` check (see
//     its doc in lib.rs), a stop queued right behind a delete click could
//     slip this id into `pending` WHILE the confirm prompt is opening, so
//     a later, perfectly genuine "confirm delete" click would find
//     `pending` already occupied and silently no-op via `do_delete`'s
//     re-entry guard instead of deleting — a confirmed delete vanishing
//     with no error at all.
//   - stop-then-delete: without `on_delete`'s own `pending` check, the
//     delete click could open a confirm prompt for a session a stop is
//     already acting on, whose eventual confirm would then race that
//     in-flight stop.
// Both sessions use a real, killable `sleep 300` so `on_stop`'s own API
// call has something to reach — a synthetic stub would leave "the guard
// refused it" indistinguishable from "the request never landed at all".
test("rapid stop/delete clicks on the same row never let a confirmed delete silently vanish", async ({
  page,
  request,
}) => {
  const createdA = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  const createdB = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(createdA.status()).toBe(200);
  expect(createdB.status()).toBe(200);
  const { id: idA } = await createdA.json();
  const { id: idB } = await createdB.json();

  let stopRequestsA = 0;
  await page.route(`**/api/sessions/${idA}/stop`, async (route) => {
    stopRequestsA++;
    await route.continue();
  });
  let deleteRequestsA = 0;
  await page.route(`**/api/sessions/${idA}`, async (route) => {
    if (route.request().method() === "DELETE") {
      deleteRequestsA++;
    }
    await route.continue();
  });
  let stopRequestsB = 0;
  await page.route(`**/api/sessions/${idB}/stop`, async (route) => {
    stopRequestsB++;
    await route.continue();
  });

  try {
    await page.goto("/");
    const rowA = page.locator(`[data-session-id="${idA}"]`);
    const rowB = page.locator(`[data-session-id="${idB}"]`);
    await expect(rowA.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });
    await expect(rowB.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });

    // Ordering 1 (session A): delete, then stop, dispatched together.
    // The buttons only exist inside an open actions panel, so the panel
    // must be open before the bare DOM clicks can find them.
    await openRowMenu(rowA);
    await page.evaluate((id) => {
      const row = document.querySelector(`[data-session-id="${id}"]`)!;
      row.querySelector<HTMLButtonElement>(".session-row-delete")?.click();
      row.querySelector<HTMLButtonElement>(".session-row-stop")?.click();
    }, idA);

    // The confirm prompt won; the queued stop click was refused outright
    // — no stop request ever reached the network.
    await expect(rowA.locator(".confirm-consequence")).toBeVisible();
    expect(stopRequestsA).toBe(0);

    // The genuinely user-driven confirm click must still work normally:
    // this is the exact click the old (pre-cross-guard) bug would have
    // silently swallowed had a stop slipped into `pending` first.
    await rowA.locator(".confirm-delete").click();
    await expect(rowA).toHaveCount(0, { timeout: 10_000 });
    expect(deleteRequestsA).toBe(1);
    expect(stopRequestsA).toBe(0);

    // Ordering 2 (session B): stop, then delete, dispatched together.
    await openRowMenu(rowB);
    await page.evaluate((id) => {
      const row = document.querySelector(`[data-session-id="${id}"]`)!;
      row.querySelector<HTMLButtonElement>(".session-row-stop")?.click();
      row.querySelector<HTMLButtonElement>(".session-row-delete")?.click();
    }, idB);

    // The stop won; the queued delete click was refused, so no confirm
    // prompt ever appeared for B at all.
    await expect(rowB.locator(".status-badge")).toHaveText(
      /^exited — stopped by user/,
      { timeout: 10_000 },
    );
    await expect(rowB.locator(".confirm-consequence")).toHaveCount(0);
    expect(stopRequestsB).toBe(1);
  } finally {
    for (const id of [idA, idB]) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// The other cross-guard regression test: `confirm_delete`'s "proceed ONLY
// when `confirming.remove` reports the id was actually present" check
// (lib.rs) exists specifically for a cancel and a confirm click racing
// each other, not for the stop/delete race the test above covers. Both
// buttons are captured BEFORE either is clicked, then clicked together in
// one synchronous block — cancel first, confirm second — the same
// bare-`.click()` technique used elsewhere in this file to bypass
// Playwright's own actionability wait, which would otherwise never let a
// click reach a button its own prior click had logically superseded.
// Without the guard, the confirm click (processed second, after cancel
// has already cleared `confirming`) would still fall through to
// `do_delete` regardless — deleting a session the user had just told the
// UI, in the very same gesture, to leave alone.
test("dispatching cancel and confirm in the same tick never deletes the session", async ({
  page,
  request,
}) => {
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  let deleteRequests = 0;
  await page.route(`**/api/sessions/${id}`, async (route) => {
    if (route.request().method() === "DELETE") {
      deleteRequests++;
    }
    await route.continue();
  });

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });
    await openRowMenu(row);
    await row.locator(".session-row-delete").click();
    await expect(row.locator(".confirm-consequence")).toBeVisible();

    await page.evaluate((sessionId) => {
      const row = document.querySelector(`[data-session-id="${sessionId}"]`)!;
      const cancel = row.querySelector<HTMLButtonElement>(".confirm-cancel")!;
      const confirm = row.querySelector<HTMLButtonElement>(".confirm-delete")!;
      cancel.click();
      confirm.click();
    }, id);

    // Cancel won: the row is back to normal, and no DELETE was ever sent
    // — not merely "not yet", but never at all.
    await expect(row.locator(".confirm-consequence")).toHaveCount(0);
    await expect(row.locator(".session-row-delete")).toBeEnabled();
    expect(deleteRequests).toBe(0);
  } finally {
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// `autofocus` on the cancel button (`SessionRow`'s "Focus-on-open" doc in
// lib.rs) is the safety default: the instant the confirm prompt mounts,
// keyboard focus must already be ON cancel, not confirm, so a stray
// Enter/Space reaching the page right after the delete click (residual
// focus, a fast typist) backs OUT of the destructive action instead of
// into it. Checked via `document.activeElement` (Playwright's
// `toBeFocused`), then exercised through a genuine Enter keypress via
// Playwright's own keyboard API — the actual mechanism a stray keystroke
// would use, not just a synthetic click on cancel.
test("the confirm prompt focuses cancel on open; Enter closes it without deleting", async ({
  page,
  request,
}) => {
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  let deleteRequests = 0;
  await page.route(`**/api/sessions/${id}`, async (route) => {
    if (route.request().method() === "DELETE") {
      deleteRequests++;
    }
    await route.continue();
  });

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });
    await openRowMenu(row);
    await row.locator(".session-row-delete").click();
    await expect(row.locator(".confirm-consequence")).toBeVisible();

    await expect(row.locator(".confirm-cancel")).toBeFocused();

    await page.keyboard.press("Enter");
    await expect(row.locator(".confirm-consequence")).toHaveCount(0);
    await expect(row.locator(".session-row-delete")).toBeEnabled();
    expect(deleteRequests).toBe(0);
  } finally {
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// SPEC.md's "Title: optional; auto-generated when omitted" through the
// real create endpoint (farhelm-supervisor's `create_session` derives the
// working directory's basename — see its own doc). A regression that sent
// an empty STRING instead of omitting/nulling the field would ask the
// supervisor to name the session "" verbatim (see `create_session`'s doc
// in lib.rs) rather than triggering the derivation at all, so this checks
// both ends: the wire request itself, and the title the created session
// actually got.
test("a blank title creates a session titled after the working directory's basename, not an empty string", async ({
  page,
  request,
}) => {
  let capturedBody: any = null;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    capturedBody = route.request().postDataJSON();
    await route.continue();
  });

  let id: string | undefined;
  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title: "",
    });
    await form.locator('button[type="submit"]').click();
    // The created session's derived title taking the pane is the create's
    // completion signal (termReady is already true under auto-select).
    await expect(page.locator(".titlebar .title")).toHaveText("tmp", { timeout: 15_000 });

    // `== null` deliberately covers BOTH an omitted key (`undefined`
    // after JSON parsing) and an explicit `null` value — `create_session`
    // in lib.rs sends `Option<&str>` through `serde_json::json!`, which
    // serializes `None` as a JSON `null` rather than dropping the key,
    // and either shape is equally correct here: what matters is that it
    // is NOT the empty string.
    expect(capturedBody.title == null).toBe(true);

    const titleText = await page.locator(".titlebar .title").textContent();
    expect(titleText).toBe("tmp");
    expect(titleText).not.toBe("");

    const listing = await (await request.get("/api/sessions")).json();
    const createdSession = listing.sessions.find(
      (s: any) => s.title === "tmp" && s.cwd === "/tmp",
    );
    expect(createdSession).toBeTruthy();
    id = createdSession.id;
  } finally {
    if (id) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// A real end-to-end status flip, observed through the LIST UI rather than
// the API this time (the test above already covers the API's own view of
// stop/delete). This deliberately does NOT use a session whose command
// exits near-instantly (`sh -c 'exit 7'`, an earlier design): a command
// that is already dead by the time the FIRST list fetch happens would
// only prove that a freshly-fetched already-exited row renders
// correctly — it would never prove that the list REFRESHES an EXISTING
// row from alive to exited, which is the actual polling behavior this
// test exists to pin. `trap ... TERM` keeps the session observably alive
// first, so stopping it via the API is what exercises a genuine in-place
// refresh of the same `data-session-id` row.
test("list refreshes an existing row from alive to exited, then drops it on delete", async ({
  page,
  request,
}) => {
  const created = await request.post("/api/sessions", {
    data: {
      cwd: "/tmp",
      invocation: `sh -c 'trap "exit 7" TERM; sleep 300'`,
    },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText(LIVE_BADGE, {
      timeout: 10_000,
    });

    await request.post(`/api/sessions/${id}/stop`);
    // The exact exit code is NOT pinned here: `kill_process_tree`
    // (farhelm-supervisor/src/service.rs) sends SIGTERM, waits a grace
    // period, then RE-ENUMERATES the tree and SIGKILLs only whatever it
    // still finds alive at that point — a process that already exited
    // from the trap's `exit 7` before the grace period elapsed simply
    // will not be found, and keeps its trap-driven exit code. So the
    // genuine race is whether the trap's `exit 7` completes before that
    // grace period does: if it does not, the process is still alive at
    // re-enumeration time and gets SIGSTOPped then SIGKILLed instead,
    // turning the eventual death into a signal death tmux cannot reduce
    // to a code. The badge could legitimately read "exited (code 7)" or
    // plain "exited" either way; only the COARSE state transition is
    // asserted here. The exact text each `SessionStatus` renders into is
    // already pinned unconditionally by
    // `status_badge_matches_text_and_class_for_each_status` in lib.rs.
    //
    // "exited — stopped by user": this session ended because the user
    // stopped it, and PLAN_M3.md item 4's durable annotation QUALIFIES
    // the exited badge rather than replacing it (SPEC.md: "'stopped' is
    // not a distinct status").
    await expect(row.locator(".status-badge.exited")).toHaveText(
      /^exited — stopped by user/,
      { timeout: 10_000 },
    );

    await request.delete(`/api/sessions/${id}`);
    await expect(row).toHaveCount(0, { timeout: 10_000 });
  } finally {
    // Best-effort cleanup for the case where an assertion above threw
    // before the happy-path delete ran; see the identical pattern (and
    // its rationale) on the HTTP-level test above.
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// PLAN_M2.md acceptance 5: a capped, truncated list reply must be VISIBLE
// as such, not silently presented as complete. Reaching a real truncation
// (the supervisor's ~500-session cap) would mean creating hundreds of
// sessions just to exercise one banner, so this intercepts the same GET
// /api/sessions the real list polls and fulfills it with a small,
// synthetic truncated listing instead — enough to prove the UI's
// truncation logic without the cost or flakiness of a 500-session stack.
// No method check, no unroute: this page never makes a non-GET request
// to /api/sessions, and Playwright tears the route down with the page
// when the test ends.
// PLAN_M3.md item 2 in the browser: an interrupted session must render
// its own badge AND route delete like an ended session — straight through,
// no confirmation. The confirmation prompt exists to protect an agent that
// might still be running, and a host reboot is what produced this status,
// so there is not even a stray descendant left for a delete to kill.
//
// The listing is synthesized rather than provoked, because provoking it
// for real would mean rebooting the machine running the suite: the status
// comes from a boot-id comparison the Rust suite covers directly (e2e.rs's
// `a_reboot_interrupts_live_sessions_and_preserves_ended_ones`). What is
// under test here is only what the UI does with the status, which is
// exactly the half that needs a browser. The DELETE is counted and stalled
// so "no confirm prompt appeared" cannot be confused with "one appeared
// and vanished before the assertion ran".
test("an interrupted session shows its badge and deletes without confirming", async ({
  page,
}) => {
  const session = {
    id: "synthetic-interrupted",
    title: "synthetic-interrupted",
    cwd: "/tmp",
    invocation: "true",
    status: { state: "interrupted" },
    annotation: null,
  };
  await page.route("**/api/sessions", (route) =>
    fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ sessions: [session], total: 1, truncated: false }),
    }),
  );
  let deleteRequests = 0;
  let releaseDelete: () => void = () => {};
  const deleteHeld = new Promise<void>((resolve) => {
    releaseDelete = resolve;
  });
  await page.route(`**/api/sessions/${session.id}`, async (route) => {
    if (route.request().method() !== "DELETE") {
      await route.continue();
      return;
    }
    deleteRequests += 1;
    await deleteHeld;
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: "{}",
    });
  });

  await page.goto("/");
  const row = page.locator(`[data-session-id="${session.id}"]`);
  await expect(row.locator(".status-badge.interrupted")).toHaveText(
    "interrupted",
    { timeout: 10_000 },
  );

  await openRowMenu(row);
  await row.locator(".session-row-delete").click();
  // The DELETE is stalled, so the row is still on screen — and while it
  // is, no confirmation controls exist at all.
  await expect(row).toHaveCount(1);
  await expect(row.locator(".confirm-consequence")).toHaveCount(0);
  await expect(row.locator(".confirm-delete")).toHaveCount(0);
  expect(deleteRequests).toBe(1);
  releaseDelete();
});

// PLAN_M3.md item 3: the launch shim's exec-failure sentinel, surfaced on
// the wire as `SessionStatus::Error { detail }`. Synthetic and route-mocked
// exactly like the `interrupted` test just above — real end-to-end coverage
// of the classification itself (create with a genuinely missing binary,
// wait for the supervisor to read the sentinel and commit `Error`) lives in
// `crates/farhelm/tests/e2e.rs`; this test is scoped to what only the
// BROWSER can prove: the badge's exact text and CSS class, and that
// deleting an error row — like an exited or interrupted one — skips the
// confirmation prompt entirely. The reason is not "nothing ever ran": the
// login shell and the launch shim DID run (the shim is what writes this
// very sentinel, from inside a real process) — it is that the AGENT'S OWN
// exec is what failed, before it or anything it might have spawned ever
// existed, so there is no lingering process tree for a delete to warn
// about.
test("an error session shows its badge with detail and deletes without confirming", async ({
  page,
}) => {
  const detail = "exec_failed argv0=/nope errno=2";
  const session = {
    id: "synthetic-error",
    title: "synthetic-error",
    cwd: "/tmp",
    invocation: "/nope",
    status: { state: "error", detail },
    annotation: null,
  };
  await page.route("**/api/sessions", (route) =>
    fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ sessions: [session], total: 1, truncated: false }),
    }),
  );
  let deleteRequests = 0;
  let releaseDelete: () => void = () => {};
  const deleteHeld = new Promise<void>((resolve) => {
    releaseDelete = resolve;
  });
  await page.route(`**/api/sessions/${session.id}`, async (route) => {
    if (route.request().method() !== "DELETE") {
      await route.continue();
      return;
    }
    deleteRequests += 1;
    await deleteHeld;
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: "{}",
    });
  });

  await page.goto("/");
  const row = page.locator(`[data-session-id="${session.id}"]`);
  // The badge must state the shim's own recorded detail, not just the
  // bare word "error" — it is the one piece of information that actually
  // explains why the row needs attention, and the class must be the
  // dedicated `error` modifier (red family), never `exited`'s.
  await expect(row.locator(".status-badge.error")).toHaveText(
    `error — ${detail}`,
    { timeout: 10_000 },
  );

  await openRowMenu(row);
  await row.locator(".session-row-delete").click();
  // The DELETE is stalled, so the row is still on screen — and while it
  // is, no confirmation controls exist at all: the agent's own exec never
  // succeeded, so there is no lingering process tree for a delete to warn
  // about (see this test's own top-of-file comment for why that is not
  // the same claim as "nothing ever ran").
  await expect(row).toHaveCount(1);
  await expect(row.locator(".confirm-consequence")).toHaveCount(0);
  await expect(row.locator(".confirm-delete")).toHaveCount(0);
  expect(deleteRequests).toBe(1);
  releaseDelete();
});

// Review-swarm fix batch item 21: the shim's own detail is argv-derived,
// so — unlike every OTHER badge's fixed, short vocabulary — its length is
// not bounded by anything this UI controls. Without `app.css`'s
// `.status-badge` cap (`max-width`/`min-width: 0`/`overflow: hidden`), a
// long detail can widen the row past its siblings' shrink budget and push
// the stop/delete buttons out of reach. Pinned in the browser's actual
// layout engine, not just against the CSS source: the badge visibly
// clips (its scrollWidth exceeds its clientWidth) and the delete button
// stays on screen and clickable regardless.
test("a long error detail clips the badge without pushing the delete button out of reach", async ({
  page,
}) => {
  const detail = `exec_failed argv0=${"/very/long/path/segment".repeat(40)} errno=2`;
  const session = {
    id: "synthetic-error-long",
    title: "synthetic-error-long",
    cwd: "/tmp",
    invocation: "/nope",
    status: { state: "error", detail },
    annotation: null,
  };
  await page.route("**/api/sessions", (route) =>
    fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ sessions: [session], total: 1, truncated: false }),
    }),
  );

  await page.goto("/");
  const row = page.locator(`[data-session-id="${session.id}"]`);
  const badge = row.locator(".status-badge.error");
  await expect(badge).toBeVisible();

  const clips = await badge.evaluate((el) => el.scrollWidth > el.clientWidth);
  expect(clips).toBe(true);

  await openRowMenu(row);
  const deleteButton = row.locator(".session-row-delete");
  await expect(deleteButton).toBeVisible();
  const box = await deleteButton.boundingBox();
  expect(box).not.toBeNull();
  const viewport = page.viewportSize();
  expect(viewport).not.toBeNull();
  expect(box!.x + box!.width).toBeLessThanOrEqual(viewport!.width);
  await deleteButton.click();
  await expect(row.locator(".confirm-consequence")).toHaveCount(0);
});

// Review-swarm fix batch item 21's other half: the SAME injection idiom
// `delete confirmation safely displays a title containing executable
// HTML...` (above) applied to the error badge's `detail` — it renders
// through Dioxus's normal text interpolation exactly like every other
// server-controlled string here, but the badge is a NEW render site this
// PR adds, so it earns its own direct pin rather than relying on the
// title test's coverage to imply it.
test("an error detail containing executable HTML renders literally in the badge", async ({
  page,
}) => {
  const detail = `exec_failed argv0=<img src=x onerror="window.__pwned=1"> errno=2`;
  const session = {
    id: "synthetic-error-xss",
    title: "synthetic-error-xss",
    cwd: "/tmp",
    invocation: "/nope",
    status: { state: "error", detail },
    annotation: null,
  };
  await page.route("**/api/sessions", (route) =>
    fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ sessions: [session], total: 1, truncated: false }),
    }),
  );

  await page.goto("/");
  const row = page.locator(`[data-session-id="${session.id}"]`);
  await expect(row.locator(".status-badge.error")).toHaveText(`error — ${detail}`, {
    timeout: 10_000,
  });
  expect(await page.evaluate(() => (window as any).__pwned)).toBeUndefined();
});

test("truncation banner shows when the listing reports truncated", async ({
  page,
}) => {
  await page.route("**/api/sessions", (route) =>
    fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        sessions: [
          { id: "synthetic-1", title: "synthetic-1", cwd: "/tmp", invocation: "true" },
          { id: "synthetic-2", title: "synthetic-2", cwd: "/tmp", invocation: "true" },
        ],
        total: 700,
        // The ordinary request excludes archived rows, so it is a filter
        // even though the query string is empty. Keep the fixture on the
        // current helm contract instead of exercising the old-peer fallback.
        matching: 700,
        truncated: true,
      }),
    }),
  );

  await page.goto("/");
  await expect(page.locator(".truncation-banner")).toBeVisible();
  await expect(page.locator(".truncation-banner")).toHaveText(
    "showing 2 of 700 matching sessions (700 in all)",
  );
});

// Polling is M2's whole live-update mechanism (PLAN_M2.md's "Out" defers
// live push out of M2; current PLAN.md places it in M6.75), so it needs
// its own direct test: with the list
// already open, a session created from elsewhere (the HTTP API, standing
// in for "any other client") must appear without a reload. Bounded at
// ~10s — comfortably above the 3s poll interval — so a regression to
// "never polls" fails the test instead of hanging the suite.
test("list polls and picks up a session created elsewhere", async ({
  page,
  request,
}) => {
  await page.goto("/");
  await expect(page.locator(".session-list")).toBeVisible();

  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  try {
    await expect(page.locator(`[data-session-id="${id}"]`)).toBeVisible({
      timeout: 10_000,
    });
  } finally {
    // Best-effort: the session is long-running (`sleep 300`), so it must
    // be stopped and deleted regardless of whether the assertion above
    // passed, or it would sit in the list for every test after this one.
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// Host/Origin validation is what keeps a hostile page from driving the
// helm through DNS rebinding; loopback binding alone does not.
test("requests from a foreign origin are refused", async ({ request }) => {
  const resp = await request.get("/api/sessions", {
    headers: { Origin: "http://evil.example" },
  });
  expect(resp.status()).toBe(403);
});

// The framing defense: the Origin check cannot stop an <iframe> pointed at
// the helm (a GET navigation sends no Origin), and a framed terminal is a
// clickjacking target where delivered keystrokes are command execution.
// The exact header values ARE the contract, so this is an exact-value
// assertion, not a change detector.
test("responses carry the anti-framing headers", async ({ request }) => {
  const resp = await request.get("/api/sessions");
  expect(resp.status()).toBe(200);
  expect(resp.headers()["x-frame-options"]).toBe("DENY");
  expect(resp.headers()["content-security-policy"]).toBe(
    "frame-ancestors 'none'",
  );
});


// ---------------------------------------------------------------------
// M5: replay presentation and rename (PLAN_M5.md items 5 and 6)
//
// Placed last for the same reason the flood fixture is kept away from its
// neighbours: the replay tests deliberately fill a session's scrollback,
// and the sessions they do it to are their own — created, polluted, and
// deleted here.
//
// Several of these tests HOLD a catch-up phase open (see
// `holdCatchUpFromNextLoad`) rather than racing it. That is what makes the
// acceptance assertable at all: the contract is about what is on screen
// DURING the catch-up, and a real supervisor's marker arrives too fast to
// observe from Node — the window is not merely short, it has no lower
// bound this suite controls.
// ---------------------------------------------------------------------


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
 * Trigger two reads matching `pattern` and resolve once both have landed in
 * the page.
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
 */
async function afterTwoReads(page: Page, pattern: RegExp, trigger: () => void) {
  for (let i = 0; i < 2; i++) {
    const landed = page.waitForResponse(
      (response) =>
        response.request().method() === "GET" && pattern.test(response.url()),
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
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
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
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
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
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    const tabId = await addTab(page, 0);
    const element = `terminal-${tabId}`;
    await waitForIslandMounted(page, element);
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
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForIslandMounted(page, element);
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
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
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
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
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
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForHeldCatchUp(page, "terminal");
    const held = await replayRecord(page, "terminal");
    expect(held.bufferedBytes).toBeGreaterThan(0);
    await expect(page.locator("#terminal")).toBeHidden();

    // A second client takes the session, which detaches this one where it
    // stands: mid-catch-up, with a marker it will now never receive.
    second = await browser.newContext();
    const page2 = await second.newPage();
    await page2.goto("/");
    await page2.locator(`[data-session-id="${id}"]`).click();
    await page2.waitForFunction(() => (window as any).__farhelmTermReady === true);

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
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
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
    await afterTwoReads(page, /\/api\/sessions$/, () => feed.notify(++revision));
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
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
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
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
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
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
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
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

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
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForHeldCatchUp(page, "terminal");
    // Captured before the teardown: after it, the registry points at the
    // replacement, and this reference is the only way back to the mount
    // under test.
    await page.evaluate(() => {
      (window as any).__staleIsland = (window as any).__farhelmIslands["terminal"].test;
    });

    // Bounce to the shared session and back: a full unmount, then a fresh mount
    // into the same element, whose own catch-up is held too.
    await sharedSessionRow(page).click();
    await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
    await row.click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForHeldCatchUp(page, "terminal");

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
  await page.route("**/api/sessions", async (route) => {
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
    await page.unroute("**/api/sessions");
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
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
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
// Every test below owns its session (`openOwnTerminal`) because these tests
// close, wedge, or take over sockets. None may leave that disruptive lifecycle
// behind in a fixture another test shares.
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

/**
 * Open a session this test OWNS, on a fresh terminal, and hand back its id
 * for cleanup.
 *
 * A session per test contains the disruptive lifecycles this section
 * exercises. Killing sockets, wedging them, and taking sessions over from a
 * second browser context are not things to do to a fixture other tests share.
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

// A takeover that lands while a client is BETWEEN reconnect attempts is
// the race auto-reconnect creates and must not lose: that client holds no
// socket, so the takeover notice reaches it nowhere, and its next
// automatic attach would reuse the same lease and silently displace the
// new owner — an eviction with nobody behind it.
//
// Two independent mechanisms are asserted, because each covers the other's
// gap. The supervisor REFUSES an unattended attach while another lease
// holds the session (`if_unowned`), and the browser stops attempting at
// all once it learns it was taken over. The observable consequence is the
// one that matters to a user: the winner keeps typing, uninterrupted.
test("takeover-during-backoff-does-not-steal-the-session", async ({ browser, page, request }) => {
  // A long first rung: the takeover below has to land while this client is
  // WAITING, which is the whole scenario.
  await reconnectTimingsFromNextLoad(page, {
    delaysMs: [4_000, 4_000, 4_000, 4_000, 4_000, 4_000],
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
    await rememberSocket(page, "terminal");
    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());
    await expect(page.locator(".terminal-reconnect-now")).toBeVisible();

    const second = await browser.newContext();
    const page2 = await second.newPage();
    try {
      // The winner attaches while the loser is mid-backoff, holding nothing.
      await openSessionTerminal(page2, ownId!);

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

      // And it stays that way: several further rungs elapse with the loser
      // attaching nothing.
      await page.waitForTimeout(1_000);
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
  // waited for a read would wait forever; applying the (empty) filter is a
  // person asking, and a submit always reads (`ListView`'s `apply_filter`).
  //
  // So these two waits assert the explicit half of that rule as much as they
  // stage the latch: a page that stood its EXPLICIT reads down under skew
  // would hang here rather than fail an assertion, which is the shape this
  // exact regression took on WebKit. Two submits, two agreeing replies.
  await page.unroute("**/api/**");
  for (let i = 0; i < 2; i++) {
    const landed = page.waitForResponse(
      (response) =>
        response.request().method() === "GET" && /\/api\/sessions/.test(response.url()),
      { timeout: 30_000 },
    );
    await page.locator(".filter-apply").click();
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

// =====================================================================
// Multi-host: the hosts panel, the stale list, and host management
// (PLAN_M6.md item 6).
//
// The stack Playwright drives is a two-host FLEET (see start-stack.sh):
// the helm's own machine as the reserved local row, plus a second real
// supervisor on an isolated state directory registered as an
// ssh-to-localhost "remote". Everything below drives that fleet for real
// — a killed supervisor, a genuinely re-registered host — with a set of
// deliberate exceptions that use `page.route` instead, each noted at the
// test that takes it: the local host's not-running state (provoking it for
// real would mean stopping the developer's own supervisor), the identity
// states (provoking those for real means wiping and reinstalling a
// supervisor mid-suite; the helm-side contract for them is pinned in Rust,
// and what is left to prove here is the RENDERING — `identity-mismatch-
// surfaced` — and the request body — `adopt-requires-current-identity`,
// which is the one that actually sends an adoption), the full phase table
// (seven of its
// nine phases have no cheap real cause), and the create dialog's
// vanishing-selection case.
//
// SKIPPING is decided by an INDEPENDENT self-ssh probe, never by "the host
// did not connect" — see `selfSshAvailable`. The distinction is the
// difference between skipping a precondition this suite may not create and
// skipping the bugs it exists to catch. In CI, where self-ssh is
// provisioned, an absent fleet FAILS (`requireFleet`).
//
// =====================================================================

/**
 * What `start-stack.sh` publishes about the stack it booted.
 *
 * The tests here need four things no API exposes and none of which they
 * may guess: which binary to relaunch the "remote" supervisor from, which
 * state directory it serves (the isolated one — never the developer's real
 * `~/.local/state/farhelm`), which process to kill to make that host go away,
 * and where the injected provisioning backend reads its per-target behavior.
 *
 * The pid is a CLAIM, not an identity, and is never signalled on its own
 * authority — see `verifiedRemoteSupervisorPid`.
 */
type StackInfo = {
  farhelm: string;
  remote_state: string;
  remote_supervisor_pid: number;
  remote_ssh: string;
  provisioning_backend: string;
};

/**
 * Read the published stack description, failing loudly if it is absent.
 *
 * A missing file means the harness did not write it — a real breakage,
 * not a condition to degrade around, since every fleet test below would
 * otherwise fail one assertion at a time with no hint of the cause.
 */
function stackInfo(): StackInfo {
  const at = path.resolve(__dirname, "../.stack-info.json");
  if (!fs.existsSync(at)) {
    throw new Error(
      `${at} is missing: start-stack.sh publishes it before the helm starts, so the stack under test is not the one this suite expects`,
    );
  }
  return JSON.parse(fs.readFileSync(at, "utf8"));
}

/**
 * Make one injected probe report a supervisor at concrete dial coordinates.
 *
 * The injected backend defaults to `absent`, which is right for provisioning
 * scenarios but wrong for the harness's already-running self-SSH supervisor.
 * This target-only override leaves every unrelated destination on the absent
 * path. Publishing by rename matters because the helm reads this file for
 * each probe and must never observe a half-written JSON document.
 */
function configureDiscoveredProbe(
  destination: string,
  dialFarhelm: string,
  dialStateDir: string | null,
): void {
  const root = stackInfo().provisioning_backend;
  const at = path.join(root, "config.json");
  const config = JSON.parse(fs.readFileSync(at, "utf8"));
  config.targets ??= {};
  config.targets[`ssh:${destination}`] = {
    probe: "supervisor",
    build_version: helmBuild(),
    dial_farhelm: dialFarhelm,
    dial_state_dir: dialStateDir,
  };
  const next = path.join(root, `config.${process.pid}.next`);
  fs.writeFileSync(next, `${JSON.stringify(config)}\n`, { mode: 0o600 });
  fs.renameSync(next, at);
}

/** Every registered host as `GET /api/hosts` currently reports it. */
async function apiHosts(request: APIRequestContext): Promise<any[]> {
  const resp = await request.get("/api/hosts");
  expect(resp.ok(), `GET /api/hosts: ${resp.status()}`).toBe(true);
  return (await resp.json()).hosts;
}

/**
 * The fleet's ssh row — the harness's second supervisor.
 *
 * Found by KIND rather than by id, because its id changes whenever a test
 * removes and re-adds it (a fresh registry row is exactly what SPEC.md's
 * remove-then-re-add contract produces), and by kind rather than by name
 * because the local row is the only other one and is never `ssh`.
 */
async function apiRemoteHost(
  request: APIRequestContext,
): Promise<any | undefined> {
  return (await apiHosts(request)).find((host: any) => host.kind === "ssh");
}

/** Wait until the fleet's ssh row reports `phase`, or fail the test. */
async function waitForRemotePhase(
  request: APIRequestContext,
  phase: string,
  timeout: number,
) {
  await expect
    .poll(async () => (await apiRemoteHost(request))?.state?.phase, {
      timeout,
      message: `waiting for the ssh host to reach ${phase}`,
    })
    .toBe(phase);
}

/**
 * The same wait as an ANSWER rather than an assertion, for the one caller
 * that has to decide something from it instead of failing on it (the fleet
 * probe below).
 */
function remoteReachesPhase(
  request: APIRequestContext,
  phase: string,
  timeout: number,
): Promise<boolean> {
  return waitForRemotePhase(request, phase, timeout).then(
    () => true,
    () => false,
  );
}

/**
 * Escape a literal for use inside a `RegExp`.
 *
 * Host names here are ssh DESTINATIONS, which routinely contain regex
 * metacharacters — a dotted hostname is the common case, and `.` matches
 * anything. Interpolating one raw builds a pattern that quietly matches more
 * rows than it names, so `user@a.b` would also select `user@axb`; with a
 * bracket or a paren in a name it stops being a valid pattern at all and the
 * test fails for a reason that has nothing to do with what it asserts.
 */
function escapeRegExp(literal: string): string {
  return literal.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Locator for one host's row in the panel, matched against `.host-name`
 * exactly — the same anchoring `rowByTitle` uses and for the same reason:
 * a row's full text contains its state detail too, which mentions other
 * hosts (a duplicate names its twin), so `hasText` on the row would match
 * rows that merely refer to the wanted host.
 */
function hostRowByName(page: Page, name: string) {
  return page.locator(".host-row").filter({
    has: page.locator(".host-name", {
      hasText: new RegExp(`^${escapeRegExp(name)}$`),
    }),
  });
}

/**
 * Whether the two-host fleet is actually up, decided once per project pass.
 *
 * Passwordless `ssh localhost` is a precondition this suite is not entitled
 * to create (writing to the developer's `known_hosts` to get it would be
 * worse than skipping), so where it is absent every test that needs a second
 * real machine is skipped — LOUDLY, with the reason on the skip, exactly as
 * the Rust ssh tests and the cgroup tests skip.
 */
let fleetReady = false;

/**
 * Gate one fleet test — skip where self-ssh is genuinely unavailable, FAIL
 * in CI.
 *
 * The asymmetry is the point. CI provisions self-ssh explicitly (keygen,
 * authorized_keys, sshd — see the workflow), so a fleet that is not up there
 * means the provisioning or the harness broke, and a skip would let the
 * entire multi-host surface go unexercised while the run stayed green. On a
 * developer's machine the same condition is an environment this suite may
 * not modify, and skipping is correct.
 */
function requireFleet() {
  if (process.env.CI) {
    expect(
      fleetReady,
      "CI provisions passwordless self-ssh, so a fleet that is not up is a broken harness rather than a missing prerequisite — this must not be skipped here",
    ).toBe(true);
    return;
  }
  test.skip(
    !fleetReady,
    "the harness's ssh-to-localhost host is not connected (passwordless `ssh localhost` is unavailable here; CI provisions it)",
  );
}

/**
 * Whether passwordless `ssh localhost` works, probed DIRECTLY rather than
 * inferred from the helm.
 *
 * Independence is the whole value: inferring it from "the ssh host never
 * reached connected" conflates the one condition this suite may skip for
 * with every condition it must not — a broken transport, a supervisor that
 * will not start, a helm that mis-registers the ensure file. Those are bugs
 * this suite exists to catch, and a fleet probe that treats them as "no
 * self-ssh here" reports them as a skip.
 *
 * The options mirror the Rust suite's own probe exactly: `BatchMode=yes` so
 * every interactive fallback fails instead of hanging, and
 * `StrictHostKeyChecking=yes` rather than `accept-new` because a test suite
 * must not write to the developer's `known_hosts`.
 */
async function selfSshAvailable(): Promise<boolean> {
  return await new Promise<boolean>((resolve) => {
    const probe = spawn(
      "ssh",
      [
        "-o",
        "BatchMode=yes",
        "-o",
        "StrictHostKeyChecking=yes",
        "-o",
        "ConnectTimeout=10",
        "localhost",
        "true",
      ],
      { stdio: "ignore" },
    );
    probe.on("error", () => resolve(false));
    probe.on("exit", (code) => resolve(code === 0));
  });
}

/** The replacement supervisor a down-host test started, if any. */
let restartedRemote: ChildProcess | undefined;

/**
 * Whether anything is actually SERVING the "remote" supervisor's socket.
 *
 * The socket FILE is not the answer: a supervisor that is killed leaves it
 * behind (the next one to hold the state dir's ownership lock is what
 * unlinks it — see the supervisor's own `serve`), so an existence check
 * cannot tell a live supervisor from the corpse of one. Connecting can.
 *
 * Two callers, both of which need that distinction rather than the file's
 * existence: the fleet probe, deciding whether an ssh host that is not
 * connected means an earlier pass through this file took the supervisor
 * down (put one back) or something else is wrong (fail); and the kill path,
 * which polls this to know the supervisor it signalled has genuinely stopped
 * answering before the tests that depend on that begin. Skipping is decided
 * by `selfSshAvailable`, never by this.
 */
async function remoteSupervisorAlive(): Promise<boolean> {
  const socket = path.join(stackInfo().remote_state, "supervisor.sock");
  if (!fs.existsSync(socket)) return false;
  return await new Promise<boolean>((resolve) => {
    const probe = net.connect(socket);
    probe.on("connect", () => {
      probe.destroy();
      resolve(true);
    });
    probe.on("error", () => {
      probe.destroy();
      resolve(false);
    });
  });
}

/**
 * Confirm that `pid` really is this run's remote supervisor before anything
 * signals it.
 *
 * A pid read from a file is a claim, not an identity: the process it named
 * can have exited and the number been reused by something else entirely, and
 * on a developer's machine the something else is their editor as readily as
 * anything. Signalling on the strength of the file alone is how a test
 * harness kills a bystander.
 *
 * The check is a BIRTH-IDENTITY one — the process's own argv, which no
 * later pid reuse can imitate: it must be a `supervisor run` serving exactly
 * this run's isolated remote state directory. Refusing loudly where `/proc`
 * is unavailable is deliberate: this suite runs on Linux (see the CI job),
 * and the alternative to verifying is signalling blind.
 */
function verifiedRemoteSupervisorPid(): number {
  const info = stackInfo();
  const pid = info.remote_supervisor_pid;
  const at = `/proc/${pid}/cmdline`;
  if (!fs.existsSync(at)) {
    throw new Error(
      `refusing to signal pid ${pid}: ${at} does not exist, so it cannot be confirmed to be this run's remote supervisor`,
    );
  }
  // NUL-delimited argv, with a trailing NUL to drop.
  const argv = fs.readFileSync(at, "utf8").split("\0").filter(Boolean);
  const looksRight =
    argv.some((arg) => arg === "supervisor") &&
    argv.some((arg) => arg === info.remote_state);
  if (!looksRight) {
    throw new Error(
      `refusing to signal pid ${pid}: its argv is ${JSON.stringify(argv)}, which is not a supervisor serving ${info.remote_state} — the pid was probably reused`,
    );
  }
  return pid;
}

/**
 * Kill whichever supervisor is currently serving the "remote" state dir, and
 * wait for it to actually be gone.
 *
 * Awaiting matters: the caller's next move is asserting that the helm has
 * noticed, and a supervisor that is merely signalled is still answering.
 */
async function killRemoteSupervisor() {
  // A replacement THIS file started takes precedence: after one down-host
  // pass the pid the harness published is a corpse, and signalling it would
  // either throw or — worse, after pid reuse — hit something else.
  if (restartedRemote) {
    await stopRestartedRemote();
    return;
  }
  process.kill(verifiedRemoteSupervisorPid(), "SIGTERM");
  await expect
    .poll(remoteSupervisorAlive, {
      timeout: 30_000,
      message: "waiting for the killed remote supervisor to stop answering",
    })
    .toBe(false);
}

/**
 * Stop the replacement supervisor this file started, waiting for its exit.
 *
 * Awaited rather than fire-and-forget because the next thing that happens is
 * either another test or the whole run ending: a signalled-but-not-yet-dead
 * supervisor still holds its state directory's ownership lock, so a
 * replacement started immediately afterwards would refuse to serve, and a
 * run that ended here would leak a live process past the suite.
 */
async function stopRestartedRemote() {
  const child = restartedRemote;
  restartedRemote = undefined;
  if (!child || child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  const exited = new Promise<void>((resolve) => {
    child.once("exit", () => resolve());
  });
  child.kill("SIGTERM");
  await exited;
}

/**
 * Put the shared fleet's ssh row back, whatever state a test left it in, and
 * wait for it to be usable again.
 *
 * The two tests that deliberately unregister that host call this from a
 * `finally`. Without it, a failure between the removal and the re-add leaves
 * every later fleet test — and the entire second engine's pass — running
 * against a one-host stack, which reports as a cascade of unrelated
 * failures with one cause buried at the top.
 *
 * Idempotent, and idempotent about the RIGHT row: it checks the destination
 * AND both install fields, not merely that some ssh host exists. A test that
 * failed mid-way can leave a row that is ssh-shaped but wrong — the
 * destination re-added without the harness's install fields, say, which
 * registers happily and then never connects — and accepting it would hand
 * every later test a fleet that looks restored and is not. A row that does
 * not match is replaced rather than adjusted, because the retarget verb
 * deliberately cannot change install fields.
 */
async function restoreFleetRow(request: APIRequestContext) {
  const info = stackInfo();
  const wanted = (host: any) =>
    host.destination === info.remote_ssh &&
    host.remote_farhelm === info.farhelm &&
    host.remote_state_dir === info.remote_state;

  const existing = await apiRemoteHost(request);
  if (existing && !wanted(existing)) {
    const removed = await request.delete(`/api/hosts/${existing.id}`);
    expect(
      removed.ok(),
      `dropping a wrong shared fleet row: ${await removed.text()}`,
    ).toBe(true);
  }
  if (!(await apiRemoteHost(request))) {
    const added = await request.post("/api/hosts", {
      data: {
        ssh: info.remote_ssh,
        remote_farhelm: info.farhelm,
        remote_state_dir: info.remote_state,
      },
    });
    expect(
      added.ok(),
      `restoring the shared fleet row: ${await added.text()}`,
    ).toBe(true);
  }
  await waitForRemotePhase(request, "connected", 60_000);
}

/**
 * Bring the "remote" supervisor back up on its own isolated state dir and
 * wait for the helm to notice.
 *
 * Retries are DRIVEN rather than waited out: a host past its active-retry
 * window is re-probed every 45 seconds, which is most of a test timeout
 * spent asleep, and forcing an attempt is exactly what the retry verb
 * exists for. Poking on each poll is safe — retry is one attempt, not a
 * fresh ladder — and it also covers the window where the new supervisor has
 * not finished binding, since an attempt that lands too early simply fails
 * and the next one does not.
 *
 * Restarting over the killed supervisor's leftover socket is safe: the new
 * process takes the state dir's ownership lock first, which is what proves
 * the file is a corpse rather than a rival.
 */
async function restoreRemoteSupervisor(request: APIRequestContext) {
  const info = stackInfo();
  restartedRemote = spawn(
    info.farhelm,
    ["supervisor", "run", "--state-dir", info.remote_state],
    { stdio: "ignore" },
  );
  await expect
    .poll(
      async () => {
        const host = await apiRemoteHost(request);
        if (host && host.state.phase !== "connected") {
          await request.post(`/api/hosts/${host.id}/retry`);
        }
        return host?.state?.phase;
      },
      {
        timeout: 90_000,
        intervals: [2_000],
        message: "waiting for the restarted remote supervisor to be reconnected",
      },
    )
    .toBe("connected");
}

test.describe("multi-host", () => {
  test.beforeAll(async ({ request }) => {
    // The hook's own budget, not a test's: the resurrection path below can
    // legitimately spend a minute and a half, and the default would abort it
    // half-way and report a fleet that is missing when it is merely slow.
    test.setTimeout(180_000);

    // The skip decision comes from the PRECONDITION, probed directly —
    // never from "the host did not connect", which would let a broken
    // transport, a supervisor that will not start, or a mis-registered
    // ensure file all masquerade as a missing prerequisite and skip the
    // tests that exist to catch them.
    if (!(await selfSshAvailable())) {
      fleetReady = false;
      console.log(
        "SKIPPED the multi-host fleet tests: passwordless `ssh localhost` is unavailable here, " +
          "and this suite may not create it (writing to your known_hosts to get it would be " +
          "worse). CI provisions it, where these tests FAIL rather than skip.",
      );
      return;
    }

    const info = stackInfo();
    configureDiscoveredProbe(info.remote_ssh, info.farhelm, info.remote_state);

    // Self-ssh works, so the fleet is expected to be up — and anything that
    // goes wrong from here is a real failure rather than a reason to skip.
    // Short first wait, because the stack has been serving for this whole
    // file by now: a healthy ssh host connected minutes ago.
    fleetReady = await remoteReachesPhase(request, "connected", 45_000);
    if (!fleetReady) {
      // Nothing serving that state dir is what an EARLIER project's pass
      // through this file leaves behind: its down-host group killed the
      // harness's supervisor, and the replacement was reaped at the end of
      // the file. Put one back — and let a failure to do so FAIL the hook,
      // because with self-ssh working there is no honest reading of it as a
      // missing prerequisite.
      expect(
        await remoteSupervisorAlive(),
        "the ssh host is not connected and its supervisor IS serving: the fleet is broken in a way self-ssh cannot explain",
      ).toBe(false);
      await restoreRemoteSupervisor(request);
      fleetReady = true;
    }
  });

  // Both hosts, both chips, both identities — the OPENED panel's detailed
  // two-host baseline, and the one assertion that proves the fleet is a
  // fleet rather than one host drawn twice. (SPEC.md's without-opening-
  // anything visibility is the compact strip's contract, pinned in
  // sidebar.spec.ts; the identities and evidence here are what the panel
  // adds behind the toggle.)
  test("hosts-panel-states: both harness hosts render connected chips with identities", async ({
    page,
    request,
  }) => {
    requireFleet();

    await page.goto("/");
    await openHostsPanel(page);
    const rows = page.locator(".host-row");
    await expect(rows).toHaveCount(2);

    // The local row: named as itself, never as an address, and with no
    // management affordances — SPEC.md's "never a ghost, never needing
    // registration" is also a promise that it cannot be removed.
    const local = hostRowByName(page, "this machine");
    await expect(local).toHaveAttribute("data-host-phase", "connected");
    await expect(local).toHaveAttribute("data-host-kind", "local");
    await expect(local.locator(".host-chip")).toHaveText("connected");
    await expect(local.locator(".host-remove")).toHaveCount(0);
    await expect(local.locator(".host-edit")).toHaveCount(0);

    const info = stackInfo();
    const remote = hostRowByName(page, info.remote_ssh);
    await expect(remote).toHaveAttribute("data-host-phase", "connected");
    await expect(remote).toHaveAttribute("data-host-kind", "ssh");
    await expect(remote.locator(".host-remove")).toHaveCount(1);

    // The identities are the point of the two rows being two rows: the
    // helm records one per install, and two hosts reporting the SAME one
    // would be a duplicate rather than a fleet. Compared against the API's
    // own answer so this asserts what is rendered matches what is served,
    // rather than merely that something is on screen.
    const hosts = await apiHosts(request);
    const identities = hosts.map((host: any) => host.identity);
    expect(identities.every((identity: string | null) => !!identity)).toBe(true);
    expect(new Set(identities).size).toBe(2);
    for (const host of hosts) {
      await expect(
        page.locator(`[data-host-id="${host.id}"] .host-detail`),
      ).toContainText(host.identity);
    }
  });

  // The local host is ALWAYS listed, and when its supervisor is not running
  // it says so with a manual path — never an offer to install anything
  // (provisioning is M7's, and PLAN_M6.md is explicit that a registered
  // destination with no supervisor gets a hint, not an installer).
  //
  // Route-intercepted, and this is the one place in this file where that is
  // not merely convenient: producing the state for real means stopping the
  // supervisor this suite's every other test depends on, on the developer's
  // own machine. The state itself is the helm's to produce; what is under
  // test here is entirely this UI's rendering of it.
  test("local-host-always-listed: the local row renders its manual-start hint", async ({
    page,
  }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = body.hosts.map((host: any) =>
        host.kind === "local"
          ? {
              ...host,
              state: {
                phase: "unreachable-reprobing",
                cause: "local-supervisor-not-running",
                // The helm's OWN dial failure, in the shape
                // `farhelm_supervisor::service::connect` actually produces:
                // an anyhow chain whose middle layer carries the exact
                // start command, state directory and all. That exactness is
                // the point of the fixture — the UI's job here is to hand
                // that command to the user rather than paraphrase it from
                // facts it does not have (the state dir is not on
                // /api/hosts and never will be).
                last_error:
                  "no supervisor is running on this machine: supervisor does not appear to be "
                  + "running (socket /srv/fh-state/supervisor.sock is not accepting connections); "
                  + "start it with `farhelm supervisor run --state-dir /srv/fh-state`: "
                  + "Connection refused (os error 111)",
              },
            }
          : host,
      );
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await openHostsPanel(page);
    const local = hostRowByName(page, "this machine");
    await expect(local).toHaveAttribute(
      "data-host-phase",
      "unreachable-reprobing",
    );
    await expect(local.locator(".host-chip")).toHaveText(
      "unreachable-reprobing",
    );
    // The remedy is the helm's own sentence, with the state directory
    // intact: a hint that said only `farhelm supervisor run` would send the
    // user to start a supervisor their helm never dials, and leave the row
    // exactly as it was after they did what it told them.
    await expect(local.locator(".host-remedy")).toContainText(
      "farhelm supervisor run --state-dir /srv/fh-state",
    );
    // And it appears ONCE: the diagnosis line beside it says what happened,
    // not the same long chain over again.
    await expect(local.locator(".host-detail")).not.toContainText(
      "farhelm supervisor run",
    );
    // The row stays unmanageable even while it is down: it is still the
    // reserved local row, and offering remove would offer an operation the
    // helm refuses outright.
    await expect(local.locator(".host-remove")).toHaveCount(0);
  });

  // An identity change at a known destination freezes the host and asks the
  // user to decide, naming BOTH identities — SPEC.md forbids silently
  // merging two installs, and the decision is not presentable without both.
  //
  // Route-intercepted by choice, and the choice is worth stating: producing
  // a real mismatch means wiping and reinstalling a supervisor mid-suite,
  // which would destroy the very host the tests around this one depend on.
  // The helm-side contract is already pinned against a real manager in Rust
  // (farhelm-helm's `adopting_resolves_an_identity_mismatch_and_purges_the_old_cache`
  // and `adopting_without_the_displayed_identity_is_refused`), so what is
  // left for a browser to prove — and what only a browser can — is that
  // this UI renders the decision and sends the identity it displayed.
  test("identity-mismatch-surfaced: both identities and an adopt for the reported one", async ({
    page,
  }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = [
        ...body.hosts.filter((host: any) => host.kind === "local"),
        {
          id: 9001,
          kind: "ssh",
          destination: "user@reinstalled",
          name: "user@reinstalled",
          identity: "identity-before",
          remote_farhelm: null,
          remote_state_dir: null,
          state: {
            phase: "identity-mismatch",
            recorded: "identity-before",
            reported: "identity-after",
          },
        },
      ];
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await openHostsPanel(page);
    const row = hostRowByName(page, "user@reinstalled");
    await expect(row).toHaveAttribute("data-host-phase", "identity-mismatch");
    await expect(row.locator(".host-chip")).toHaveText("identity-mismatch");
    // Both, because the decision is between them.
    await expect(row.locator(".host-detail")).toContainText("identity-before");
    await expect(row.locator(".host-detail")).toContainText("identity-after");
    // The control names what adopting would accept, so the click and the
    // sentence above it cannot disagree.
    await expect(row.locator(".host-adopt")).toHaveText("adopt identity-after");
  });

  // An identity-UNVERIFIED host must offer no adopt at all. It looks
  // adjacent to a mismatch and is not: the host answered with no identity,
  // so there is nothing to compare and the helm refuses the verb. Offering
  // it would put a button on screen whose only possible outcome is a
  // refusal, while implying a decision the user does not have — which is
  // why the helm's own state docs make this a renderer obligation rather
  // than a suggestion.
  test("identity-unverified-offers-no-adopt: the remedies are named instead", async ({
    page,
  }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = [
        ...body.hosts.filter((host: any) => host.kind === "local"),
        {
          id: 9002,
          kind: "ssh",
          destination: "user@silent",
          name: "user@silent",
          identity: "identity-recorded",
          remote_farhelm: null,
          remote_state_dir: null,
          state: {
            phase: "identity-unverified",
            recorded: "identity-recorded",
          },
        },
      ];
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await openHostsPanel(page);
    const row = hostRowByName(page, "user@silent");
    await expect(row).toHaveAttribute("data-host-phase", "identity-unverified");
    await expect(row.locator(".host-adopt")).toHaveCount(0);
    await expect(row.locator(".host-detail")).toContainText(
      "identity-recorded",
    );
    // The three things that DO help, since adopting is not one of them.
    await expect(row.locator(".host-remedy")).toContainText("retarget");
    await expect(row.locator(".host-remedy")).toContainText("remove");
  });

  // The adopt request must carry the identity the user was SHOWN, not
  // whatever the host reports when the click lands. A re-probe can change
  // the reported identity between the prompt appearing and the request
  // arriving, and an empty-bodied adopt would then silently adopt a third
  // install — so the helm 409s a stale approval, and this pins both halves
  // of the UI's side: what it sends, and that it surfaces the refusal.
  test("adopt-requires-current-identity: the displayed identity is sent, and a 409 is shown", async ({
    page,
  }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = [
        ...body.hosts.filter((host: any) => host.kind === "local"),
        {
          id: 9003,
          kind: "ssh",
          destination: "user@racing",
          name: "user@racing",
          identity: "identity-before",
          remote_farhelm: null,
          remote_state_dir: null,
          state: {
            phase: "identity-mismatch",
            recorded: "identity-before",
            reported: "identity-displayed",
          },
        },
      ];
      await route.fulfill({ response, json: body });
    });

    // The helm's refusal, in its own words — the shape `http_error` gives a
    // superseded adoption, which is prose rather than JSON.
    const refusal =
      "host 9003 now reports identity-since-changed, not identity-displayed; look again and decide against what it reports now";
    let adoptBody: any;
    await page.route("**/api/hosts/9003/adopt", async (route) => {
      adoptBody = JSON.parse(route.request().postData() ?? "null");
      await fulfillAsHelm(route, { status: 409, contentType: "text/plain", body: refusal });
    });

    await page.goto("/");
    await openHostsPanel(page);
    const row = hostRowByName(page, "user@racing");
    await row.locator(".host-adopt").click();

    // The refusal is the helm's, verbatim, in this row's own error line —
    // never a generic "adopt failed", which would leave the user unable to
    // tell a race from a bug.
    await expect(row.locator(".host-error")).toContainText(refusal);
    expect(
      adoptBody,
      "the adopt must name the identity that was displayed, which is the whole content of the promise the helm checks",
    ).toEqual({ reported: "identity-displayed" });
  });

  // Adding a host through the FORM registers it and the chip then reports
  // what was found — here, a real supervisor, so the row progresses to
  // connected on its own.
  //
  // The host it adds is the harness's own "remote", deliberately: a second
  // entry for the same install would be a DUPLICATE (correctly), and a
  // destination with no supervisor behind it could only ever prove the
  // unreachable path, which `unreachable-host-goes-stale` already covers
  // against a real one. So the entry is dropped through the API first —
  // setup, not the thing under test — and re-registered through the form,
  // which is also what exercises the two optional install fields the
  // harness's isolated state directory makes mandatory.
  test("add-host-discovers: a form-registered ssh host progresses to connected", async ({
    page,
    request,
  }) => {
    requireFleet();
    const info = stackInfo();

    // The DELETE is inside the protected block, not before it. Outside, a
    // failure between removing the row and entering the `try` would skip the
    // restore entirely and leave every later fleet test — and the whole
    // second engine's pass — running against a one-host stack.
    try {
      const existing = await apiRemoteHost(request);
      expect(existing, "the harness registers its remote through --ensure-hosts").toBeTruthy();
      const removed = await request.delete(`/api/hosts/${existing.id}`);
      expect(removed.ok(), `removing the ssh host: ${await removed.text()}`).toBe(true);

      await page.goto("/");
      await openHostsPanel(page);
      await expect(hostRowByName(page, info.remote_ssh)).toHaveCount(0);

      await page.locator(".add-host-button").click();
      const form = page.locator(".add-host-form");
      await form.locator(".add-host-ssh").fill(info.remote_ssh);
      await form.locator(".add-host-farhelm").fill(info.farhelm);
      await form.locator(".add-host-state-dir").fill(info.remote_state);
      await form.locator(".add-host-submit").click();

      // The row appears at once — registration does not wait for a
      // connection — and reaches connected without any further action.
      const row = hostRowByName(page, info.remote_ssh);
      await expect(row).toBeVisible();
      await expect(row).toHaveAttribute("data-host-phase", "connected", {
        timeout: 60_000,
      });
      // The install fields reached the row, not just the form: without them
      // this entry would dial a farhelm and a state directory that are not
      // the harness's, and would never have connected at all.
      const readded = await apiRemoteHost(request);
      expect(readded.remote_farhelm).toBe(info.farhelm);
      expect(readded.remote_state_dir).toBe(info.remote_state);
    } finally {
      // The shared fleet row is restored whatever happened above. This test
      // deliberately unregisters the host every other fleet test depends on,
      // and a failure between the removal and the re-add would otherwise
      // leave the rest of the file — and the whole second engine's pass —
      // running against a one-host stack, reporting a cascade of failures
      // that all have one cause.
      await restoreFleetRow(request);
    }
  });

  // The create dialog's host selector, end to end: a session created on the
  // SECOND host appears in the one merged list, tagged to that host.
  test("create-dialog-host-selector: a session is created on the chosen host", async ({
    page,
    request,
  }) => {
    requireFleet();
    const info = stackInfo();
    const title = `on-the-remote-${Date.now()}`;
    let id: string | undefined;
    try {
      await page.goto("/");
      await openHostsPanel(page);
      await expect(hostRowByName(page, info.remote_ssh)).toHaveAttribute(
        "data-host-phase",
        "connected",
      );

      const form = await fillCreateForm(page, {
        cwd: "/tmp",
        invocation: FAKE_AGENT_INVOCATION,
        title,
      });
      // By LABEL, which is the helm's own display name for the host — the
      // same string the session row will carry, so selecting and asserting
      // key off one vocabulary rather than two.
      await form
        .locator(".create-session-host")
        .selectOption({ label: info.remote_ssh });
      await form.locator(".create-session-submit").click();

      // A successful create navigates into the new session, exactly as a
      // local one does; back out to see how the list describes it.
      await expect(page.locator(".titlebar .title")).toHaveText(title, {
        timeout: 30_000,
      });
      const row = rowByTitle(page, title);
      await expect(row.locator(".session-host")).toHaveText(info.remote_ssh, {
        timeout: 30_000,
      });
      id = await findSessionIdByTitle(request, title);

      // The list's own answer must agree with the row: the session lives on
      // the selected host, not on the local one the body would have
      // defaulted to had the selection been dropped.
      const remote = await apiRemoteHost(request);
      const listing = await (await request.get("/api/sessions")).json();
      const created = listing.sessions.find((s: any) => s.title === title);
      expect(created.host).toBe(remote.id);
    } finally {
      if (!id) id = await findSessionIdByTitle(request, title);
      if (id) await cleanupSession(request, id);
    }
  });

  // A host that GOES AWAY, driven for real: the harness's remote supervisor
  // is killed, and everything that follows from that — the chip's phase,
  // its sessions' staleness, what an operation against them does, and what
  // opening one shows — is asserted against the actual helm.
  //
  // Serial and grouped because they share one expensive, destructive setup:
  // there is exactly one remote supervisor, and killing it per test would
  // pay the active-retry window (about a minute) three times over. The
  // group restores it afterwards, so everything after this file's point
  // still sees a two-host fleet.
  test.describe.serial("with the remote supervisor killed", () => {
    let staleSessionId: string | undefined;
    const staleTitle = `stale-on-remote-${Date.now()}`;

    test.beforeAll(async ({ request }) => {
      if (!fleetReady) return;
      const remote = await apiRemoteHost(request);
      const created = await request.post("/api/sessions", {
        data: {
          cwd: "/tmp",
          invocation: "sleep 600",
          title: staleTitle,
          host: remote.id,
        },
      });
      expect(
        created.ok(),
        `creating a session on the remote host: ${await created.text()}`,
      ).toBe(true);
      staleSessionId = (await created.json()).id;

      // Wait for the helm to have REFRESHED this session from its host
      // before taking that host away.
      //
      // Not padding: a create's reply reports `Unknown` deliberately (the
      // supervisor cannot claim the agent has execed yet) and the helm seeds
      // its cache from exactly that reply, so a host killed inside the first
      // refresh interval leaves "unknown" as the honest last-known status.
      // That is correct behavior and a coin toss to assert against — it
      // failed on one engine and passed on the other. Waiting for a probed
      // status makes the last-known one a real observation, which is what
      // `stale-session-metadata-view` is actually about.
      await expect
        .poll(
          async () => {
            const listing = await (await request.get("/api/sessions")).json();
            return listing.sessions.find((s: any) => s.id === staleSessionId)
              ?.status?.state;
          },
          {
            timeout: 30_000,
            message: "waiting for the helm to refresh the remote session's status",
          },
        )
        // Any LIVE status will do — the point is that the helm has probed
        // this session at all, so its last-known status is a real
        // observation rather than the create-time placeholder. Asserting
        // one exact word here would fail the moment the classifier decided
        // a quiet agent was idle.
        .toMatch(LIVE_BADGE);

      // The one thing no API can do. SIGTERM rather than SIGKILL so the
      // supervisor unwinds; either way the helm loses its connection when
      // the process serving it goes. Awaited, so the tests below start
      // against a host that is genuinely gone rather than one that has
      // merely been signalled.
      await killRemoteSupervisor();
    });

    test.afterAll(async ({ request }) => {
      if (!fleetReady) return;
      // The hook's own budget: bringing the host back is a real reconnect,
      // and every test after this group depends on it having happened.
      test.setTimeout(180_000);
      await restoreRemoteSupervisor(request);
      if (staleSessionId) await cleanupSession(request, staleSessionId);
    });

    // SPEC.md: sessions on an unreachable host "stay in the list from the
    // helm's last-known knowledge, clearly marked". Both halves are the
    // assertion — the chip reaching the phase that says re-probing
    // continues forever, and the rows staying put with their marking.
    test("unreachable-host-goes-stale: the chip re-probes and its sessions are marked", async ({
      page,
      request,
    }) => {
      requireFleet();
      // The active-retry window is about a minute of real backoff before
      // the phase becomes `unreachable-reprobing`, which is the phase under
      // test: a shorter wait would only ever observe `connecting`.
      test.setTimeout(240_000);
      const info = stackInfo();

      await page.goto("/");
      await openHostsPanel(page);
      // Staleness arrives FIRST and does not wait for the retry ladder: a
      // host stops being connected the moment its connection drops, and
      // every one of its rows is last-known knowledge from that instant.
      const row = rowByTitle(page, staleTitle);
      await expect(row.locator(".stale-badge")).toBeVisible({ timeout: 60_000 });
      await expect(row).toHaveAttribute("data-session-stale", "true");
      // Still listed, not vanished — the actual promise.
      await expect(row.locator(".session-title")).toHaveText(staleTitle);

      await waitForRemotePhase(request, "unreachable-reprobing", 180_000);
      await expect(hostRowByName(page, info.remote_ssh)).toHaveAttribute(
        "data-host-phase",
        "unreachable-reprobing",
        { timeout: 30_000 },
      );
      // The transport's own words, which is the only thing anyone can
      // actually search for when a host will not answer.
      await expect(
        hostRowByName(page, info.remote_ssh).locator(".host-detail"),
      ).not.toBeEmpty();
    });

    // SPEC.md: "Opening such a session shows its metadata — title,
    // directory, last-known status — behind a clear host-unreachable
    // notice; there is no terminal to show." All three clauses are
    // asserted, the last one negatively: no terminal element at all, rather
    // than a terminal that happens to be blank.
    test("stale-session-metadata-view: metadata behind the notice, and no terminal", async ({
      page,
    }) => {
      requireFleet();
      const info = stackInfo();

      await page.goto("/");
      await rowByTitle(page, staleTitle).locator(".session-row-open").click();

      // SPEC.md's metadata triple, all three of it: title, directory, and
      // the LAST-KNOWN status. The status is the one an earlier shape of
      // this view dropped — the titlebar carries the first two, and the
      // restart offer beside them describes what a relaunch WOULD do rather
      // than what the session last was.
      await expect(page.locator(".titlebar .title")).toHaveText(staleTitle);
      await expect(page.locator(".titlebar .meta")).toHaveText(
        "/tmp — sleep 600",
      );
      const badge = page.locator(".stale-metadata .status-badge");
      await expect(badge).toBeVisible();
      // `sleep 600` was running, and the group's setup waited for the helm
      // to have OBSERVED that before killing the host — so the last thing
      // the helm knew is that it was alive. Rendered with the list's own
      // badge, so the two surfaces cannot describe one session differently.
      await expect(badge).toHaveText(LIVE_BADGE);

      const notice = page.locator(".host-stale-notice");
      await expect(notice).toBeVisible();
      // The host is named, and by its ACTUAL state rather than a generic
      // "unreachable" — a skewed or identity-frozen host reaching this
      // surface must not be described as merely down, and the only way to
      // keep that true is to render the phase the helm reports.
      await expect(notice).toContainText(info.remote_ssh);
      await expect(notice).toContainText("unreachable-reprobing");
      await expect(notice).toContainText("no terminal");

      expect(
        await page.locator(".terminal").count(),
        "a stale session must mount no terminal at all, not an empty one",
      ).toBe(0);
      expect(await page.locator(".tab-strip").count()).toBe(0);
    });

    // SPEC.md: operations against a session on an unreachable host "are
    // refused with a clear error; nothing queues for later delivery in v1".
    // The refusal has to name the host's state — the same phase word the
    // chip shows — because "it failed" is not something a user can act on.
    test("op-refused-on-unreachable: the helm's own 409 words, and nothing queued", async ({
      page,
      request,
    }) => {
      requireFleet();

      await page.goto("/");
      const row = rowByTitle(page, staleTitle);
      // The controls are deliberately still live on a stale row: the
      // helm's refusal is a better answer than a disabled button that
      // explains nothing.
      await openRowMenu(row);
      await row.locator(".session-row-stop").click();

      const error = row.locator(".action-error");
      await expect(error).toContainText("unreachable-reprobing");
      await expect(error).toContainText("nothing was queued");

      // "Nothing queued" is a claim about the SERVER, so it is checked
      // there: the session is still exactly as it was, and no stop is
      // waiting to be delivered when the host returns.
      const listing = await (await request.get("/api/sessions")).json();
      const still = listing.sessions.find((s: any) => s.title === staleTitle);
      expect(still, "a refused stop must not remove the row").toBeTruthy();
      expect(still.stale).toBe(true);
    });
  });

  // SPEC.md's remove-merely-forgets contract, executable end to end
  // (PLAN_M6.md names this test): removing forgets the host AND the
  // sessions the helm cached for it, while the host itself — its
  // supervisor, its running agent — is untouched, so re-adding the same
  // destination rediscovers everything.
  //
  // The session is created through the API and never stopped: that is what
  // makes the rediscovery meaningful. A session that had been stopped would
  // reappear just as readily from a re-registration that had somehow killed
  // things on the way out.
  test("remove-and-re-add-host: removal forgets, re-adding rediscovers", async ({
    page,
    request,
  }) => {
    requireFleet();
    test.setTimeout(120_000);
    const info = stackInfo();
    const title = `survives-removal-${Date.now()}`;
    let id: string | undefined;

    try {
      const remote = await apiRemoteHost(request);
      const created = await request.post("/api/sessions", {
        data: {
          cwd: "/tmp",
          invocation: "sleep 600",
          title,
          host: remote.id,
        },
      });
      expect(created.ok(), `creating on the remote: ${await created.text()}`).toBe(true);
      id = (await created.json()).id;

      await page.goto("/");
      await openHostsPanel(page);
      // Explicitly bounded rather than left on the 5s default: the row
      // appears only after the client's next listing poll (a three-second
      // cadence, and a walk is several round trips), so the default is
      // barely one interval and flakes on a loaded runner.
      await expect(rowByTitle(page, title).locator(".session-host")).toHaveText(
        info.remote_ssh,
        { timeout: 30_000 },
      );

      // Remove through the in-page confirmation — wry has no native
      // dialogs, so there is no browser prompt to accept, and the flow is
      // the same on both renderers.
      const row = hostRowByName(page, info.remote_ssh);
      await row.locator(".host-remove").click();
      await expect(row.locator(".confirm-consequence")).toContainText(
        "leaves its supervisor and sessions running",
      );
      await row.locator(".host-confirm-remove").click();

      await expect(hostRowByName(page, info.remote_ssh)).toHaveCount(0, {
        timeout: 30_000,
      });
      // The cached sessions went with the host: a row left behind would be
      // a session with no host to name.
      await expect(rowByTitle(page, title)).toHaveCount(0, { timeout: 30_000 });

      // Re-register the same destination. A fresh registry row, a fresh
      // host id — and the same install behind it.
      await page.locator(".add-host-button").click();
      const form = page.locator(".add-host-form");
      await form.locator(".add-host-ssh").fill(info.remote_ssh);
      await form.locator(".add-host-farhelm").fill(info.farhelm);
      await form.locator(".add-host-state-dir").fill(info.remote_state);
      await form.locator(".add-host-submit").click();

      await expect(hostRowByName(page, info.remote_ssh)).toHaveAttribute(
        "data-host-phase",
        "connected",
        { timeout: 60_000 },
      );
      // The session the removal forgot is back, live rather than stale,
      // because it never stopped running.
      const rediscovered = rowByTitle(page, title);
      await expect(rediscovered).toBeVisible({ timeout: 30_000 });
      await expect(rediscovered).toHaveAttribute("data-session-stale", "false");
      await expect(rediscovered.locator(".session-host")).toHaveText(
        info.remote_ssh,
      );
      // The AGENT survived, which is the half of SPEC.md's contract a
      // reappearing row does not prove on its own: a re-registration that
      // had killed and relaunched things would produce exactly the same row.
      // `sleep 600` was never stopped, so anything other than alive means
      // the removal reached past the registry.
      await expect(rediscovered.locator(".status-badge")).toHaveText(LIVE_BADGE);
      const listing = await (await request.get("/api/sessions")).json();
      const survivor = listing.sessions.find((s: any) => s.title === title);
      expect(
        survivor.id,
        "the rediscovered session must be the SAME session, not a lookalike relaunched under one name",
      ).toBe(id);
    } finally {
      // The HOST goes back FIRST, and the order is load-bearing rather than
      // tidy. This test can fail while the host is unregistered, and the
      // helm routes a session operation by owner lookup — with no host, the
      // session is not in the merged view at all, so `cleanupSession`'s
      // 404-tolerant stop and delete both succeed at doing nothing and the
      // `sleep 600` on the remote is leaked into every test that follows.
      // Restoring first puts the session back in the view where the cleanup
      // can actually reach it.
      await restoreFleetRow(request);
      if (!id) id = await findSessionIdByTitle(request, title);
      if (id) await cleanupSession(request, id);
    }
  });

  // Every phase, in one intercepted reply, each field carrying a sentinel
  // no other field could produce.
  //
  // A per-phase spot check cannot catch what this does: a renderer that
  // dropped one field, or printed some other variant's payload, still emits
  // plausible-looking text. Unique sentinels make each assertion about THAT
  // field in THAT row, and the table is exhaustive so a phase added later
  // arrives here without coverage rather than silently unrendered.
  test("hosts-panel-phase-table: every phase chips and details itself", async ({
    page,
  }) => {
    const phases = [
      {
        id: 8001,
        phase: "connecting",
        state: { phase: "connecting", attempt: 3, last_error: "sentinel-connecting" },
        needles: ["3", "sentinel-connecting"],
      },
      {
        id: 8002,
        phase: "unreachable-reprobing",
        state: {
          phase: "unreachable-reprobing",
          cause: "transport-failure",
          last_error: "sentinel-unreachable",
        },
        needles: ["sentinel-unreachable"],
      },
      {
        id: 8003,
        phase: "connected",
        state: {
          phase: "connected",
          identity: "sentinel-identity",
          build_version: "sentinel-build",
          refresh: { status: "ok", sessions: 7 },
        },
        needles: ["sentinel-identity", "sentinel-build", "7 sessions"],
      },
      {
        id: 8004,
        phase: "version-skew",
        state: {
          phase: "version-skew",
          peer_protocol: 99,
          peer_build: "sentinel-peer-build",
          our_protocol: 8,
          our_build: "sentinel-our-build",
          remediation: "sentinel-remediation",
        },
        needles: ["99", "sentinel-peer-build", "sentinel-our-build"],
        remedy: "sentinel-remediation",
      },
      {
        id: 8005,
        phase: "identity-mismatch",
        state: {
          phase: "identity-mismatch",
          recorded: "sentinel-recorded",
          reported: "sentinel-reported",
        },
        needles: ["sentinel-recorded", "sentinel-reported"],
      },
      {
        id: 8006,
        phase: "identity-unverified",
        state: { phase: "identity-unverified", recorded: "sentinel-unverified" },
        needles: ["sentinel-unverified"],
      },
      {
        id: 8007,
        phase: "duplicate",
        state: { phase: "duplicate", twin: 4242, identity: "sentinel-duplicate" },
        needles: ["4242", "sentinel-duplicate"],
      },
      {
        id: 8008,
        phase: "retired",
        state: { phase: "retired", reason: "sentinel-retired" },
        needles: ["sentinel-retired"],
      },
      // Not a state the helm can be in — it is what a UI one version behind
      // sees, and the panel must degrade that ONE row rather than the fleet.
      {
        id: 8009,
        phase: "unrecognized",
        state: { phase: "invented-by-a-later-helm" },
        needles: ["does not know"],
      },
    ];

    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = phases.map((entry) => ({
        id: entry.id,
        kind: "ssh",
        destination: `user@${entry.phase}`,
        name: `user@${entry.phase}`,
        identity: null,
        remote_farhelm: null,
        remote_state_dir: null,
        state: entry.state,
      }));
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await openHostsPanel(page);
    await expect(page.locator(".host-row")).toHaveCount(phases.length);
    for (const entry of phases) {
      const row = page.locator(`[data-host-id="${entry.id}"]`);
      await expect(row).toBeVisible();
      await expect(row).toHaveAttribute("data-host-phase", entry.phase);
      // The chip carries the helm's own word, which is also the word its
      // refusals use — visible, not merely present in the DOM.
      const chip = row.locator(".host-chip");
      await expect(chip).toBeVisible();
      await expect(chip).toHaveText(entry.phase);
      const detail = row.locator(".host-detail");
      await expect(detail).toBeVisible();
      for (const needle of entry.needles) {
        await expect(detail).toContainText(needle);
      }
      if (entry.remedy) {
        await expect(row.locator(".host-remedy")).toContainText(entry.remedy);
      }
    }
  });

  // Retry is offered in every state and must actually DIAL: it is one
  // attempt rather than a shortened wait, and for a retired host it is the
  // only thing that brings the actor back at all.
  test("host-retry-click: the control posts the retry verb for its own host", async ({
    page,
    request,
  }) => {
    const local = (await apiHosts(request)).find((host: any) => host.kind === "local");
    const retried: string[] = [];
    await page.route("**/api/hosts/*/retry", async (route) => {
      retried.push(new URL(route.request().url()).pathname);
      await route.continue();
    });

    await page.goto("/");
    await openHostsPanel(page);
    const row = page.locator(`[data-host-id="${local.id}"]`);
    await expect(row.locator(".host-retry")).toBeVisible();
    await row.locator(".host-retry").click();

    await expect
      .poll(() => retried, { message: "waiting for the retry POST" })
      .toEqual([`/api/hosts/${local.id}/retry`]);
    // The local row survives its own retry: this is a reconnect, not a
    // removal, and SPEC.md has the helm's own machine always listed.
    await expect(row).toBeVisible();
  });

  // Cancelling a removal must leave the host exactly as it was — the safe
  // half of the confirmation, and the one a focus-on-cancel default exists
  // to make easy to reach by accident.
  test("remove-cancel: backing out of the prompt forgets nothing", async ({
    page,
    request,
  }) => {
    requireFleet();
    const info = stackInfo();
    const before = await apiRemoteHost(request);

    await page.goto("/");
    await openHostsPanel(page);
    const row = hostRowByName(page, info.remote_ssh);
    await row.locator(".host-remove").click();
    await expect(row.locator(".host-confirm-remove")).toBeVisible();
    // Focus lands on the way OUT of the destructive action, so a stray
    // Enter after the remove click backs out rather than in.
    await expect(row.locator(".host-cancel-remove")).toBeFocused();
    await row.locator(".host-cancel-remove").click();

    // Back to the ordinary controls, same host, same id — a cancel that
    // "worked" by re-adding the host would look identical without this.
    await expect(row.locator(".host-remove")).toBeVisible();
    await expect(row.locator(".host-confirm-remove")).toHaveCount(0);
    expect((await apiRemoteHost(request)).id).toBe(before.id);
  });


  // A chosen host leaving the registry must be reconciled VISIBLY. The
  // failure this rules out is the quiet one: a selector still displaying
  // host A while the create body carries host B, which is an agent launched
  // on a machine nobody picked.
  test("create-dialog-selector-disappearance: a vanished choice is announced, not substituted", async ({
    page,
  }) => {
    // The registry read that discovers the disappearance is a feed-triggered
    // one (PLAN_M6_75.md item 6 removed the hosts poll), so the moment it
    // happens is this test's to choose rather than the shared fleet's.
    const feed = await stubFeed(page);
    let offerExtra = true;
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      if (offerExtra) {
        body.hosts = [
          ...body.hosts,
          {
            id: 8100,
            kind: "ssh",
            destination: "user@ephemeral",
            name: "user@ephemeral",
            identity: null,
            remote_farhelm: null,
            remote_state_dir: null,
            state: {
              phase: "connected",
              identity: "identity-ephemeral",
              build_version: "0.0.1",
              refresh: { status: "ok", sessions: 0 },
            },
          },
        ];
      }
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    await page.locator(".new-session-button").click();
    const selector = page.locator(".create-session-host");
    await selector.selectOption({ label: "user@ephemeral" });
    await expect(selector).toHaveValue("8100");
    await expect(page.locator(".create-session-host-note")).toHaveCount(0);

    // The host is removed from under the open dialog, and the page finds out
    // the way it finds out about anything now: a revision notification and
    // its own re-read.
    offerExtra = false;
    feed.notify(2);
    await expect(page.locator(".create-session-host-note")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.locator(".create-session-host-note")).toContainText(
      "no longer registered",
    );
    await expect(
      selector,
      "the selector must SHOW the target that would actually be used",
    ).not.toHaveValue("8100");
  });

  // The host selector is inert for the whole round trip, exactly like the
  // text fields — and for a sharper reason than tidiness: the idempotency
  // key is minted BOUND to the target, so a selection that changed between
  // minting and sending would publish a key belonging to a different
  // machine.
  test("create-dialog-selector-disabled-in-flight: the target cannot move under a create", async ({
    page,
    request,
  }) => {
    const title = `selector-inflight-${Date.now()}`;
    let release: (() => void) | undefined;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }
      await held;
      await route.continue();
    });

    try {
      await page.goto("/");
      const form = await fillCreateForm(page, {
        cwd: "/tmp",
        invocation: FAKE_AGENT_INVOCATION,
        title,
      });
      await form.locator('button[type="submit"]').click();
      await expect(form.locator(".create-session-host")).toBeDisabled();
      await expect(form.locator('input[type="text"]').nth(0)).toBeDisabled();
      release?.();
      await page.waitForFunction(() => (window as any).__farhelmTermReady === true, {
        timeout: 20_000,
      });
    } finally {
      release?.();
      await cleanUpSessionsTitled(request, title);
    }
  });

  // PLAN_M6.md names this one: a create against a host that is not connected
  // is a PRECONDITION FAILURE — a visible error naming the host's state, and
  // no session anywhere.
  //
  // The host is registered for real (a destination nothing answers at)
  // rather than mocked, because what is under test is the helm's refusal
  // reaching the form: the same phase word the chip shows has to be in the
  // message, or a user comparing the two has nothing to match.
  test("create-on-unreachable-refused: the helm's words in place, and no session", async ({
    page,
    request,
  }) => {
    const title = `refused-on-down-${Date.now()}`;
    const added = await request.post("/api/hosts", {
      data: { ssh: "user@nothing-answers-here.invalid" },
    });
    expect(added.ok(), `registering a host that is down: ${await added.text()}`).toBe(true);
    const down = (await added.json()).id;

    try {
      await page.goto("/");
      await openHostsPanel(page);
      // Its phase is whatever the dial has reached — connecting first, then
      // unreachable-reprobing — and either refuses a create. The label is
      // what proves a non-connected host is still SELECTABLE, which is the
      // half of SPEC.md's default this test exists alongside.
      const row = page.locator(`[data-host-id="${down}"]`);
      await expect(row).toBeVisible();

      await page.locator(".new-session-button").click();
      const form = page.locator(".create-session-form");
      await form.locator(".create-session-host").selectOption(String(down));
      // Command mode explicitly, as `fillCreateForm` does and for the same
      // reason — and here it also has to follow the host change, which clears
      // any agent choice (a profile id means nothing on another supervisor).
      await form.locator(".create-session-profile").selectOption("");
      await form.locator('input[type="text"]').nth(0).fill("/tmp");
      await form.locator('input[type="text"]').nth(1).fill(FAKE_AGENT_INVOCATION);
      await form.locator('input[type="text"]').nth(2).fill(title);
      await form.locator('button[type="submit"]').click();

      const error = form.locator(".create-session-error");
      await expect(error).toBeVisible({ timeout: 30_000 });
      // The helm's own sentence: the host's state named, and nothing
      // queued for when it comes back (SPEC.md v1 refuses rather than
      // deferring).
      await expect(error).toContainText(`host ${down} is`);
      await expect(error).toContainText("refused");
      // And no session anywhere — a precondition failure creates nothing.
      const listing = await (await request.get("/api/sessions")).json();
      expect(listing.sessions.filter((s: any) => s.title === title)).toHaveLength(0);
    } finally {
      await cleanUpSessionsTitled(request, title);
      await request.delete(`/api/hosts/${down}`).catch(() => {});
    }
  });

  // The two optional install fields left blank must reach the helm as
  // ABSENT, never as empty strings: the helm takes `""` literally, and a
  // host registered to dial a binary named nothing never connects for a
  // reason no chip can explain.
  test("add-host-blank-optional-fields: blanks are omitted rather than sent empty", async ({
    page,
    request,
  }) => {
    const destination = `user@blank-fields-${Date.now()}.invalid`;
    configureDiscoveredProbe(destination, "farhelm", null);
    let body: any;
    await page.route("**/api/hosts/probe", async (route) => {
      if (route.request().method() === "POST") {
        body = JSON.parse(route.request().postData() ?? "{}");
      }
      await route.continue();
    });

    let added: number | undefined;
    try {
      await page.goto("/");
      await openHostsPanel(page);
      await page.locator(".add-host-button").click();
      const form = page.locator(".add-host-form");
      await form.locator(".add-host-ssh").fill(destination);
      await form.locator(".add-host-submit").click();

      await expect(hostRowByName(page, destination)).toBeVisible({
        timeout: 30_000,
      });
      expect(body.remote_farhelm ?? null).toBeNull();
      expect(body.remote_state_dir ?? null).toBeNull();
      // The row records what discovery actually dialed. The plain `farhelm`
      // value comes from the injected supervisor observation, not an empty
      // form field silently converted into a path.
      const row = (await apiHosts(request)).find(
        (host: any) => host.destination === destination,
      );
      added = row.id;
      expect(row.remote_farhelm).toBe("farhelm");
      expect(row.remote_state_dir).toBeNull();
    } finally {
      if (added) await request.delete(`/api/hosts/${added}`).catch(() => {});
    }
  });

  // Changing the target after a failed create must mint a NEW key and send
  // the NEW host — the two together, from one submit.
  //
  // The key alone is not enough to assert: the helm scopes idempotency keys
  // per host, so a body pairing the old key with the new host would not
  // dedup on that machine at all, and a retry after an ambiguous failure
  // would launch a second real agent there.
  test("create-intent-key-rebinds-on-host-change: a new target is a new intent", async ({
    page,
    request,
  }) => {
    requireFleet();
    const info = stackInfo();
    const title = `rebind-${Date.now()}`;
    const bodies: any[] = [];
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }
      bodies.push(JSON.parse(route.request().postData() ?? "{}"));
      await route.continue();
    });

    try {
      await page.goto("/");
      // A directory that exists on neither host, so BOTH attempts fail and
      // the assertion is about the body rather than about which create
      // happened to succeed.
      const form = await fillCreateForm(page, {
        cwd: "/nonexistent/definitely/not/here",
        invocation: FAKE_AGENT_INVOCATION,
        title,
      });
      await form.locator('button[type="submit"]').click();
      await expect(form.locator(".create-session-error")).toBeVisible();

      await form
        .locator(".create-session-host")
        .selectOption({ label: info.remote_ssh });
      await form.locator('button[type="submit"]').click();
      await expect(form.locator(".create-session-error")).toBeVisible();

      expect(bodies).toHaveLength(2);
      expect(bodies[0].host).toBeTruthy();
      expect(bodies[1].host).not.toBe(bodies[0].host);
      expect(bodies[1].intent_key).toBeTruthy();
      expect(
        bodies[1].intent_key,
        "a key carried to another machine would not dedup there",
      ).not.toBe(bodies[0].intent_key);
    } finally {
      await cleanUpSessionsTitled(request, title);
    }
  });

  // The client walks the helm's cursor to exhaustion, and this is the only
  // place that walk is exercised against a genuinely multi-page list: the
  // real stack has a handful of sessions and answers in one page, so the
  // behavior that matters — resuming, replaying the cursor verbatim,
  // continuing past an empty page, preserving order — never runs.
  //
  // Route-interception rather than five hundred real sessions: this is a
  // property of the CLIENT's paging loop, and creating a page's worth of
  // agents to observe it would take minutes per engine and prove nothing
  // extra about the walk.
  test("session-list-walks-every-page: the cursor is followed to exhaustion, in order", async ({
    page,
  }) => {
    // Page 3 is deliberately EMPTY while still carrying a cursor: the helm
    // advances past cache rows whose stored metadata no longer decodes, so a
    // page can legitimately contain nothing and still have more behind it. A
    // loop driven by "did this page have rows" stops there and silently
    // hides everything after it.
    const pages: Record<string, { ids: string[]; next?: string }> = {
      "": { ids: ["walk-a", "walk-b"], next: "cursor-1" },
      "cursor-1": { ids: ["walk-c"], next: "cursor-2" },
      "cursor-2": { ids: [], next: "cursor-3" },
      "cursor-3": { ids: ["walk-d"] },
    };
    const requested: string[] = [];

    await page.route("**/api/sessions*", async (route) => {
      if (route.request().method() !== "POST") {
        const cursor = new URL(route.request().url()).searchParams.get("cursor") ?? "";
        requested.push(cursor);
        const served = pages[cursor];
        await fulfillAsHelm(route, {
          status: 200,
          contentType: "application/json",
          body: JSON.stringify({
            sessions: served.ids.map((id) => ({
              id,
              title: id,
              cwd: "/tmp",
              invocation: "true",
              host: 1,
              host_name: "this machine",
              stale: false,
            })),
            total: 4,
            // No explicit search is still the default archive-exclusion
            // filter. `total` is the fleet; `matching` is the walk's size.
            matching: 4,
            truncated: !!served.next,
            next_cursor: served.next,
          }),
        });
        return;
      }
      await route.continue();
    });

    await page.goto("/");
    // Every page's rows, and ONLY those: a walk that stopped early would
    // render a prefix, and one that re-walked would render duplicates.
    await expect(page.locator(".session-row")).toHaveCount(4);
    const titles = await page.locator(".session-row .session-title").allTextContents();
    expect(
      titles,
      "the helm's order is the display order — no client-side sort re-interleaves the pages",
    ).toEqual(["walk-a", "walk-b", "walk-c", "walk-d"]);

    // The cursors were replayed verbatim, in order, starting with none.
    expect(requested.slice(0, 4)).toEqual(["", "cursor-1", "cursor-2", "cursor-3"]);

    // A completed walk does NOT claim to be showing a subset: "showing N of
    // M" is reserved for a walk that stopped short. The ordinary request is
    // still a filter, so the complete form reports both counts.
    await expect(page.locator(".session-count")).toHaveText("4 matching of 4 sessions");
    await expect(page.locator(".truncation-banner")).toHaveCount(0);
  });

  // Retargeting a host between a failed create and its retry must mint a NEW
  // key, even though the host's ID never changed.
  //
  // This is the case an id-keyed binding cannot see, and the expensive one:
  // the registry row is the same row, so a key bound to the id survives into
  // a retry aimed at a machine that has never seen it — where it dedups
  // nothing and launches a second real agent. The binding is the host's
  // INCARNATION (`hosts::host_incarnation`), which the destination is part
  // of.
  //
  // The retarget is applied to the hosts READ rather than to the real
  // registry: what is under test is the client's binding, and actually
  // retargeting the harness's remote would disconnect the fleet for a
  // minute to prove something about a string comparison. The read that
  // discovers it is triggered from here through a stubbed feed — a healthy
  // page re-reads the registry when it is told something changed, and
  // nothing else (PLAN_M6_75.md item 6).
  test("create-intent-key-rebinds-on-retarget: a moved host is a new intent", async ({
    page,
    request,
  }) => {
    const feed = await stubFeed(page);
    const local = (await apiHosts(request)).find((host: any) => host.kind === "local");
    const title = `rebind-retarget-${Date.now()}`;
    let moved = false;
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = body.hosts.map((host: any) =>
        host.id === local.id && moved
          ? {
              ...host,
              // The REGISTRY fields are what the incarnation is built from
              // (`hosts::host_incarnation`), and changing them is the whole
              // point of the test.
              destination: "user@moved-elsewhere",
              identity: "identity-after-the-move",
              // The connection state carries its own copy, which is the one
              // the row's detail line renders — so this is what gives the
              // test something observable to wait on before resubmitting.
              state: { ...host.state, identity: "identity-after-the-move" },
            }
          : host,
      );
      await route.fulfill({ response, json: body });
    });

    const keys: (string | undefined)[] = [];
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }
      keys.push(JSON.parse(route.request().postData() ?? "{}").intent_key);
      await route.continue();
    });

    try {
      await page.goto("/");
      await openHostsPanel(page);
      await feed.waitForConnection(1);
      feed.notify(1);
      // A directory that does not exist, so both attempts fail and the
      // assertion is about the key rather than about which create happened
      // to succeed.
      const form = await fillCreateForm(page, {
        cwd: "/nonexistent/definitely/not/here",
        invocation: FAKE_AGENT_INVOCATION,
        title,
      });
      await form.locator('button[type="submit"]').click();
      await expect(form.locator(".create-session-error")).toBeVisible();

      // Same row, same id, different machine behind it — and a notification
      // to send the page looking, since nothing else will.
      moved = true;
      feed.notify(2);
      await expect(page.locator(`[data-host-id="${local.id}"] .host-name`)).toHaveText(
        "this machine",
        { timeout: 15_000 },
      );
      await expect
        .poll(
          async () =>
            await page
              .locator(`[data-host-id="${local.id}"] .host-detail`)
              .textContent(),
          { timeout: 15_000, message: "waiting for the retargeted host to reach the panel" },
        )
        .toContain("identity-after-the-move");

      // The retarget did not just rotate the intent key — it also discarded
      // the form's agent choice (`list::CreateSessionForm`: a moved host is
      // another machine, so the dialog re-asks rather than carrying an answer
      // across). What the re-ask resolves to depends on suite history: in a
      // full run the profile tests leave the host's remembered default
      // pointing at a deleted profile, so the dialog blocks with nothing
      // selected and the submit button is DISABLED — clicking it would wait
      // forever. Answering "custom command" again is what a user in front of
      // that dialog does, and it makes the second submit reachable in every
      // state instead of only in a targeted run.
      await form.locator(".create-session-profile").selectOption("");

      await form.locator('button[type="submit"]').click();
      await expect(form.locator(".create-session-error")).toBeVisible();

      expect(keys).toHaveLength(2);
      expect(keys[0]).toBeTruthy();
      expect(
        keys[1],
        "the id is unchanged, but the machine behind it is not — replaying the key there would not dedup",
      ).not.toBe(keys[0]);
    } finally {
      await cleanUpSessionsTitled(request, title);
    }
  });

  test.afterAll(async () => {
    // Only ever the replacement THIS file started; the harness's own
    // supervisor is start-stack.sh's to reap. Left running it would leak a
    // process past the suite, and killing indiscriminately would take the
    // harness's original down with the fleet.
    //
    // Awaited rather than fired and forgotten: the next project's pass
    // through this file starts by looking for a supervisor on that state
    // directory, and a signalled-but-not-yet-dead one still holds the
    // ownership lock its replacement would need.
    await stopRestartedRemote();
  });
});
