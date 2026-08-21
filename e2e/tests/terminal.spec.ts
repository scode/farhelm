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
// Several tests across the terminal spec family were written against the
// four periodic loops M6.75 removed (PLAN_M6_75.md item 6): they changed an
// intercepted fixture and waited for the next listing, detail or hosts poll
// to pick it up. Nothing polls a healthy page any more, so those tests take
// control of the INVALIDATION instead, through `helpers/fleet`'s feed stub —
// the same convention feed.spec.ts, filters.spec.ts and m6-5-debts.spec.ts use:
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
  cleanUpSessionsTitled,
  FAKE_AGENT_INVOCATION,
  findSessionIdByTitle,
  fulfillAsHelm,
  helmBuild,
  installTerminalSuiteHooks,
  LIVE_BADGE,
  LIVE_STATES,
  openTerminal,
  rowByTitle,
  sharedSessionRow,
} from "./helpers/terminal-suite";

installTerminalSuiteHooks();



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
  // reattach-lands-at-tail specs in terminal-replay-rename.spec.ts instead.
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
