// The M1 acceptance suite at the browser level (PLAN_M1.md criterion 5),
// grown by PLAN_M2.md step 7 to cover the list view and navigation: these
// tests cover output rendering, input round-trip, reconnect replay,
// resize, last-attach-wins takeover, the session list, and the
// list/terminal navigation lifecycle — all against a real helm,
// supervisor, tmux, and fake agent, with one deliberate exception: the
// truncation-banner test below intercepts GET /api/sessions with
// `page.route`, since pinning that behavior against a REAL ~500-session
// listing would mean actually creating hundreds of sessions for one
// assertion. Every other test drives the real stack end to end.
//
// Assertions read the xterm.js BUFFER, not the DOM: the DOM renderer
// materializes only viewport rows, so scrolled-off content (exactly what
// replay tests care about) never appears in .xterm-rows. The buffer is
// the semantic truth of what the terminal holds.
import { test, expect, Page } from "@playwright/test";

/** Full text content of the terminal buffer (scrollback + viewport). */
async function termText(page: Page): Promise<string> {
  return page.evaluate(() => {
    const term = (window as any).__farhelmTerm;
    if (!term) return "";
    const buf = term.buffer.active;
    const lines: string[] = [];
    for (let i = 0; i < buf.length; i++) {
      lines.push(buf.getLine(i)?.translateToString(true) ?? "");
    }
    return lines.join("\n");
  });
}

/**
 * Poll the buffer until `needle` shows up. Polling, not a one-shot read:
 * terminal output arrives asynchronously over the WebSocket with no DOM
 * event to await, so there is nothing else to hook.
 */
async function waitForTermText(page: Page, needle: string, timeout = 15_000) {
  await expect
    .poll(() => termText(page), { timeout, message: `waiting for ${needle}` })
    .toContain(needle);
}

/**
 * Locator for the shared "e2e-session" row in the list view, matched by
 * an EXACT `.session-title` match rather than `hasText` on the whole
 * row: `hasText` matches against the row's full text content (title,
 * cwd, AND invocation concatenated), so it would happily also match a
 * row whose cwd or invocation merely CONTAINS "e2e-session" as a
 * substring. Anchoring the regex against just the title element is what
 * actually pins "the row named e2e-session", not "some row that
 * mentions it somewhere".
 */
function sharedSessionRow(page: Page) {
  return page.locator(".session-row").filter({
    has: page.locator(".session-title", { hasText: /^e2e-session$/ }),
  });
}

/**
 * Load the app and wait until the terminal is genuinely usable — mounted,
 * socket attached, agent listening. Every wait keys on a marker rather
 * than a sleep, which is why these tests are not flaky on a loaded CI box.
 *
 * PLAN_M2.md step 7 replaces M1's single hardwired session view with a
 * list-then-terminal navigation, so getting to a live terminal now goes
 * through the list UI itself (goto, wait for the row, click it) rather
 * than landing straight on the terminal — every test below that used to
 * assume M1's one-view app keeps passing through this same helper.
 */
async function openTerminal(page: Page) {
  await page.goto("/");
  const row = sharedSessionRow(page);
  await expect(row).toBeVisible();
  await row.click();
  // The island sets this once xterm is mounted and the WS is opening.
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  // The fake agent prints this banner once its modes are set.
  await waitForTermText(page, "FAKE-AGENT READY");
}

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
// row's status must settle on "alive" rather than the create-time
// "unknown" placeholder — `toHaveText` retries on its own, since the
// list computes status fresh from tmux on every fetch rather than
// caching the placeholder forever.
test("list renders the session row with title, cwd, invocation, and an alive badge", async ({
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
  await expect(row.locator(".status-badge")).toHaveText("alive", {
    timeout: 10_000,
  });
});

// Keyboard activation (PLAN_M2.md step 7: rows must be
// keyboard-activatable). Since SessionRow is a native <button> rather
// than a div with a hand-rolled onkeydown, Enter activation (and Space)
// come from the browser for free — this pins that the row is actually
// reachable and operable via keyboard, not just that it happens to look
// like a button.
test("keyboard activation opens the session, matching a real click", async ({
  page,
}) => {
  await page.goto("/");
  await sharedSessionRow(page).focus();
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
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
test("back tears down the mounted terminal; reopening the same session mounts a fresh one", async ({
  page,
}) => {
  await openTerminal(page);
  await expect(page.locator(".back-button")).toBeVisible();

  await page.locator("#terminal").click();
  await page.keyboard.type("marker-before-back");
  await page.keyboard.press("Enter");
  await waitForTermText(page, "echo:marker-before-back");

  await page.evaluate(() => {
    (window as any).__farhelmTerm.__testMarker = "before-back";
    // Stashed under a different name so it survives terminal.js's own
    // `delete window.__farhelmWs` on unmount — this is the actual
    // WebSocket object the mount owned, kept around purely so the
    // assertion below can check that unmount() really closed it.
    (window as any).__testWsBeforeBack = (window as any).__farhelmWs;
  });

  await page.locator(".back-button").click();

  // The teardown itself must be observable, not just its later effects:
  // every global terminal.js publishes on mount must be gone...
  await expect
    .poll(() =>
      page.evaluate(() => ({
        ready: (window as any).__farhelmTermReady,
        term: (window as any).__farhelmTerm,
        ws: (window as any).__farhelmWs,
      })),
    )
    .toEqual({ ready: undefined, term: undefined, ws: undefined });
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
  await expect(page.locator("#terminal")).toHaveCount(0);

  await sharedSessionRow(page).click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

  const isFreshInstance = await page.evaluate(
    () => (window as any).__farhelmTerm.__testMarker !== "before-back",
  );
  expect(isFreshInstance).toBe(true);

  // Replay must bring back output produced before THIS attachment
  // existed, exactly like the reload test below — the only difference
  // is that here the round trip goes through the list/back UI instead
  // of a full page reload.
  await waitForTermText(page, "echo:marker-before-back");

  // And the fresh mount must be genuinely live, not just showing stale
  // replayed content: a new marker must round-trip through it.
  await page.locator("#terminal").click();
  await page.keyboard.type("marker-after-reopen");
  await page.keyboard.press("Enter");
  await waitForTermText(page, "echo:marker-after-reopen");
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
test("backing out before a terminal is ready, then opening a different session, mounts the right one", async ({
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
    await page.clock.pauseAt(new Date());

    // Withhold a global `mountWhenReady` genuinely cannot proceed
    // without, so opening session A puts a REAL pending retry into
    // flight (rather than resolving on its first synchronous check, as
    // it almost always would on an unloaded box).
    await page.evaluate(() => {
      (window as any).__testStashedTerminal = (window as any).Terminal;
      delete (window as any).Terminal;
    });

    await sharedSessionRow(page).click();
    await page.locator(".back-button").click();
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
}) => {
  await page.goto("/");
  await expect(sharedSessionRow(page)).toBeVisible();

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

  // Reopening the SAME session (back to the list, then the same row)
  // must succeed now that the guard was rolled back — a regression that
  // left `active` (or the old `__farhelmMounted` flag) stuck would make
  // this mount silently no-op instead.
  await page.locator(".back-button").click();
  await sharedSessionRow(page).click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await waitForTermText(page, "FAKE-AGENT READY");
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

// The creation API is the one true path (PLAN_M1.md: CLI flags feed the
// same API a UI dialog will call), so its HTTP surface needs coverage
// even though the UI has no create dialog yet (PLAN_M2.md step 8). Only
// the failure case is exercised here: a successful POST would leave an
// extra, untracked session sitting in the list for every test after this
// one, with nothing to clean it up.
// The status code is part of the contract, not an implementation detail:
// a missing cwd is the caller's own precondition failure (4xx), distinct
// from a server-side fault (5xx) the caller could not have avoided by
// sending a different request. The supervisor classifies this as
// InvalidRequest and farhelm-helm's http_error maps that to 400 — see
// ErrorKind in farhelm-proto.
test("create API reports precondition failures verbatim", async ({ request }) => {
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
    await expect(row.locator(".status-badge")).toHaveText("alive", {
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
    await expect(row.locator(".status-badge")).toHaveText(/^exited/, {
      timeout: 10_000,
    });

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
test("truncation banner shows when the listing reports truncated", async ({
  page,
}) => {
  await page.route("**/api/sessions", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        sessions: [
          { id: "synthetic-1", title: "synthetic-1", cwd: "/tmp", invocation: "true" },
          { id: "synthetic-2", title: "synthetic-2", cwd: "/tmp", invocation: "true" },
        ],
        total: 700,
        truncated: true,
      }),
    }),
  );

  await page.goto("/");
  await expect(page.locator(".truncation-banner")).toBeVisible();
  await expect(page.locator(".truncation-banner")).toContainText(
    "showing 2 of 700 sessions",
  );
});

// Polling is M2's whole live-update mechanism (PLAN_M2.md: "Out" defers
// live push to M5), so it needs its own direct test: with the list
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

// An attach failure must arrive as a detach notice on the socket, not a
// bare close: the helm sends the reason before closing precisely because
// a bare close renders as a generic "connection closed" and tells the
// user nothing. Driven through a raw WebSocket because the UI only ever
// opens sockets for sessions the API listed.
test("a terminal socket for an unknown session reports why", async ({
  page,
}) => {
  await page.goto("/");
  const notice = await page.evaluate(
    () =>
      new Promise<string>((resolve, reject) => {
        const ws = new WebSocket(
          `ws://${location.host}/api/sessions/no-such-session/term`,
        );
        const timer = setTimeout(() => reject(new Error("no message")), 10_000);
        ws.onmessage = (ev) => {
          clearTimeout(timer);
          resolve(String(ev.data));
        };
        ws.onclose = () => {
          clearTimeout(timer);
          reject(new Error("socket closed with no detach notice"));
        };
      }),
  );
  const msg = JSON.parse(notice);
  expect(msg.type).toBe("detached");
  expect(msg.reason).toContain("no such session");
});

// The WebSocket message-size cap is sized for large pastes (xterm.js
// hands a whole clipboard paste over as ONE message), and lore records a
// review fix that nearly shipped a 1 MiB cap — which would have dropped
// the connection on exactly the paste chunking exists to support. Only a
// direct socket send can produce a multi-megabyte message, hence the
// __farhelmWs test hook.
//
// SECOND TO LAST in the file on purpose: the suite shares one session and
// runs in file order, and this payload's PTY echo pollutes the terminal
// state — observed directly: with the takeover test after this one, its
// fresh page's READY wait found only a wall of echoed 'a's. (How much
// echoes is bounded by canonical-mode input handling and was not pinned
// down; the placement rule, not the mechanism, is the contract.) The one
// test allowed after this one is the ctrl-c regression below, which kills
// the shared session's fake agent outright — nothing may come after THAT
// depends on this session at all.
test("a multi-megabyte message does not drop the terminal socket", async ({
  page,
}) => {
  await openTerminal(page);
  await page.evaluate(() => {
    const ws = (window as any).__farhelmWs as WebSocket;
    ws.send(new Uint8Array(2 * 1024 * 1024).fill(0x61));
  });
  // The send is async; poll until the socket has drained it, still open.
  await expect
    .poll(
      () =>
        page.evaluate(() => {
          const ws = (window as any).__farhelmWs as WebSocket;
          return { state: ws.readyState, buffered: ws.bufferedAmount };
        }),
      { timeout: 15_000, message: "socket must stay open and drain" },
    )
    .toEqual({ state: WebSocket.OPEN, buffered: 0 });
  await page.locator("#terminal").click();
  await page.keyboard.press("Enter");
  await page.keyboard.type("after-big-message");
  await page.keyboard.press("Enter");
  // A generous timeout, not the suite's usual 10-15s: the supervisor's
  // dedicated input control client (`InputClient::send`, tmux.rs) now
  // waits for tmux's `%end` reply to each 256-byte `send-keys` chunk
  // before sending the next, so tmux must fully process this 2 MiB
  // payload — many thousands of chunk round trips — before it even
  // reaches the "after-big-message" line queued behind it on the same
  // connection. That synchronous-per-chunk design is deliberate (a
  // fire-and-forget write could not distinguish "tmux accepted the bytes"
  // from "tmux executed them"), so this test's budget reflects the real
  // cost of validating a payload this large rather than papering over it.
  await waitForTermText(page, "echo:after-big-message", 60_000);
});

// Regression test for the tmux paste-buffer input-mangling bug at the
// browser level: real Backspace and Ctrl+C keypresses go through xterm.js's
// own key handling (DEL, ETX — no custom key binding intercepts them, see
// terminal.js), the WebSocket, and the framing protocol exactly like any
// other keystroke, landing on `basic`'s pty in its ordinary canonical/cooked
// mode (nothing here puts it in raw mode, unlike the `hexecho` fixture the
// Rust e2e suite uses for byte-exact coverage of every mangled control byte,
// including ESC/arrow-up).
//
// ArrowUp is deliberately NOT exercised here, after checking what a
// canonical-mode pty actually does with it: Linux's ECHOCTL local-echo
// renders ANY control byte with no special canonical role — ESC included —
// as two-character caret notation ("^["), regardless of whether it arrived
// as a genuine 0x1b byte or as the bug's literal caret text. The pane's
// rendered output is identical either way, so `basic`'s canonical pty gives
// no browser-observable signal for ESC at all; that gap is exactly what
// `hexecho`'s raw mode (no ECHOCTL, no canonical processing) exists to
// close, and the Rust e2e suite's `input_bytes_survive_verbatim_through_hexecho`
// covers it directly.
//
// Backspace escapes that trap because DEL, unlike ESC, has a special
// canonical-mode role: a correctly delivered 0x7f is consumed as the ERASE
// character (removing the previous character, never echoed as text at
// all), while the bug's mangled delivery is two ordinary printable
// characters that erase nothing and sit in the buffer as literal `^?`. That
// gives a real positive/negative pair to assert on, checked below.
//
// Ctrl+C escapes it differently: ECHOCTL still renders a correctly
// delivered ETX as "^C" text, so the caret text alone proves nothing. What
// DOES distinguish the two is what happens next — a correct ETX is also
// consumed as INTR, raising SIGINT on the fake agent's foreground process
// group and killing it (default disposition; `basic` installs no handler),
// while the bug's mangled two-character delivery is inert text that leaves
// the process running. So the assertion below is "the pane no longer echoes
// new input", not "no ^C text appeared" — and it must reject the marker
// appearing ANYWHERE on an echoed line, not just as an exact substring: a
// mangled ctrl-c leaves the buffered "x" (see below) sitting in canonical
// input, so a later Enter would flush "x" plus the marker together as one
// line, echoing as `echo:x^Cpost-ctrlc-marker` — which a bare
// `.not.toContain("echo:post-ctrlc-marker")` would miss entirely, since
// that exact substring never occurs even though the marker plainly reached
// the (still-alive, still-buggy) agent.
//
// This does NOT end the tmux session, despite ending the fake-agent
// process: `remain-on-exit on` (SPEC.md) keeps a dead pane's session and
// window around so its terminal stays viewable, it just stops accepting
// input. Killing the agent is still sufficient for the assertion, and is
// LAST in the file because it is destructive to every other test's shared
// fixture: a correct fix permanently kills the fake agent every other test
// in this file was typing into.
test("real backspace erases; real ctrl-c kills the fake agent", async ({
  page,
}) => {
  // NOT `openTerminal()`: that helper waits for the "FAKE-AGENT READY"
  // banner, but the multi-megabyte test just before this one pushed well
  // over tmux's 12,000-line history-limit through this same session —
  // observed directly, the banner is gone from replay by the time this
  // test's fresh page attaches. The session is still alive underneath
  // (only its scrollback was evicted), so liveness is reproven below with
  // a fresh marker instead of the banner. Still has to go through the
  // list view to get there, same as openTerminal does.
  await page.goto("/");
  await sharedSessionRow(page).click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await page.locator("#terminal").click();

  // Backspace: type a two-character marker, erase the second character,
  // and require the marker to actually disappear — not just that nothing
  // new appeared. A caret-escaped DEL would leave "xy^?" in the buffer
  // (marker intact, artifact appended); a correct erase leaves neither.
  await page.keyboard.type("xy");
  await waitForTermText(page, "xy");
  await page.keyboard.press("Backspace");
  await expect
    .poll(() => termText(page), {
      message: "backspace must erase the typed marker, not print ^?",
    })
    .not.toContain("xy");

  // Flush the "x" still sitting in canonical input BEFORE ctrl-c, and wait
  // for its echo. Without this, ctrl-c's own canonical-mode fate (consumed
  // as INTR vs. left as inert "^C" text) is entangled with "x": a mangled
  // ctrl-c would leave "x^C" buffered together, and the marker typed below
  // would flush on the SAME line as "x", one line earlier than expected.
  // Waiting for "x" to echo here is what makes the post-ctrlc assertion
  // unambiguous about what ctrl-c itself did.
  await page.keyboard.press("Enter");
  await waitForTermText(page, "echo:x");

  await page.keyboard.press("Control+c");

  // A mangled ctrl-c leaves `basic` alive and still echoing; only a real
  // SIGINT kills it. Typing a fresh marker and requiring it to NEVER echo
  // is the proof — and, being an absence, needs sustained observation
  // rather than one poll: the process take-down is not instantaneous, and
  // a single early check could pass before a still-alive process would
  // have replied. The regex (not a plain substring) is deliberate: a
  // mangled ctrl-c is inert TEXT, not a control action, so it stays in the
  // canonical buffer ahead of the marker and both flush together on the
  // same line — e.g. `echo:^Cpost-ctrlc-marker` — which
  // `.not.toContain("echo:post-ctrlc-marker")` would not catch since that
  // exact substring never appears. Matching the marker anywhere after
  // `echo:` on one line closes that gap.
  await page.keyboard.type("post-ctrlc-marker");
  await page.keyboard.press("Enter");
  const deadline = Date.now() + 3_000;
  while (Date.now() < deadline) {
    expect(await termText(page)).not.toMatch(/echo:.*post-ctrlc-marker/);
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
});
