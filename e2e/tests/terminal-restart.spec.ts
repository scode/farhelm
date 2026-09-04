// ---------------------------------------------------------------------
// Restart with resume, at the browser level (PLAN_M3.md item 9).
//
// SPEC.md's restart is a lifecycle operation with a UI contract of its
// own: an interrupted session's view leads with the resume offer,
// declining it changes nothing, a live session confirms first, and a
// reused terminal keeps whatever scrollback tmux itself retained from the
// previous run, with the new run drawing below it. All four are below.
// ---------------------------------------------------------------------

import { expect, test } from "@playwright/test";
import { hideSeenState, SESSION_LISTING } from "./helpers/fleet";
import { cleanupSession, fillCreateForm, termText, waitForTermText } from "./helpers/term";
import {
  FAKE_AGENT_INVOCATION,
  findSessionIdByTitle,
  fulfillAsHelm,
  installTerminalSuiteHooks,
  LIVE_BADGE,
  LIVE_STATES,
  rowByTitle,
  sharedSessionRow,
} from "./helpers/terminal-suite";

installTerminalSuiteHooks();

// The interrupted state cannot be produced by driving this stack: it takes
// a host reboot (or the injected boot-id change the Rust suite uses), so
// the listing is intercepted exactly like terminal.spec.ts's "deleting a
// session with unknown status confirms first, with wording that admits
// uncertainty" test does for the same reason. Everything else here is
// real — the component, its wording, and the fact that no request is sent.
//
// "Declining" has no control of its own by design (SPEC.md: opening an
// interrupted session OFFERS restart-with-resume; declining leaves it
// interrupted): the user simply does not click. So what this pins is that
// navigating away sends nothing and leaves the row exactly as it was —
// a restart affordance that fired on open, or on back, would be the bug.
test("an interrupted session's view leads with the resume offer, and declining changes nothing", async ({
  page,
}) => {
  const sessionId = "11111111-2222-3333-4444-555555555555";
  const title = `interrupted-offer-${Date.now()}`;
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    // The real listing plus one interrupted row, so every other test's
    // session (and the shared "e2e-session") keeps coming through
    // untouched.
    const response = await route.fetch();
    const listing = await response.json();
    listing.sessions.push({
      id: sessionId,
      title,
      cwd: "/tmp",
      invocation: "claude",
      status: { state: "interrupted" },
      restart_offer: "resume",
    });
    listing.total += 1;
    await route.fulfill({ response, json: listing });
  });
  let restartRequests = 0;
  await page.route(`**/api/sessions/${sessionId}/restart`, async (route) => {
    restartRequests++;
    await fulfillAsHelm(route, { status: 200, contentType: "application/json", body: "{}" });
  });

  await page.goto("/");
  await rowByTitle(page, title).locator(".session-row-open").click();

  // The offer still states WHY the terminal is gone and what restarting
  // would do to the conversation — both, because the user is being asked
  // to act on something they did not do. Since the header consolidation it
  // says so on the restart control itself rather than in a permanent band:
  // `title` as a mouse's hover tooltip, and an `aria-describedby` target as
  // assistive technology's accessible description. Both are asserted,
  // because either alone leaves one of those two channels unable to read
  // it.
  const restart = page.locator(".restart-primary");
  await expect(restart).toHaveAttribute(
    "title",
    /interrupted by a host reboot.*resumes this session's own conversation/,
  );
  const described = await restart.getAttribute("aria-describedby");
  expect(described, "the explanation must exist as a real element, not only as a tooltip").toBe(
    "restart-offer-description",
  );
  const offer = page.locator(`#${described}`);
  await expect(offer).toContainText("interrupted by a host reboot");
  await expect(offer).toContainText("resumes this session's own conversation");
  // The VISIBLE glyph is the compact "restart" every header action uses —
  // the header's supported minimum width has no room for the longest
  // offer's ~320px of wording on the button's face. SPEC.md's "restart says
  // so" instead reaches the accessible name: `aria-label` names the offer,
  // not the mechanism, which is what a screen reader announces regardless
  // of hover.
  await expect(restart).toHaveText("restart");
  await expect(restart).toHaveAttribute("aria-label", "resume conversation");
  // And the header states the session's last-known status beside the
  // title, which is where the reason a user is being offered a restart now
  // lives (the offer prose used to carry it in a band of its own).
  await expect(page.locator(".titlebar .status-badge")).toHaveText("interrupted");
  // An interrupted session has nothing running, so there is no confirm
  // step in front of it.
  await expect(page.locator(".restart-confirm")).toHaveCount(0);
  expect(restartRequests).toBe(0);

  // Declining means LEAVING — selecting another session — and the leave
  // itself must not send anything: a regression that fired the restart on
  // navigate-away would pass a stay-put assertion.
  await sharedSessionRow(page).click();
  await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
  const row = rowByTitle(page, title);
  await expect(row.locator(".status-badge")).toHaveText("interrupted");
  expect(restartRequests).toBe(0);
});

// A live agent is the one case SPEC.md requires a confirmation for
// ("Restart on a session whose agent is still running confirms, stops the
// agent, then relaunches"), and the confirmation is in-page for the same
// reason delete's is: wry ships no native JS dialogs on macOS's WKWebView,
// where a `window.confirm()` would silently do nothing at all.
//
// Driven against a REAL session, so the request that finally goes out is
// the real one — including `stop_if_running`, which is the whole point of
// the confirmation and is asserted on the wire rather than assumed.
// PLAN_M6_75.md item 3's no-badge rule on the RESTART path — the half its
// create-path sibling explicitly does not cover, because the mechanism is
// a different one.
//
// A restart puts a session briefly back into "nothing has classified this
// yet", exactly as a create does. What must NOT happen is the badge
// blinking out and back: the helm's merge rule refuses to let an unknown
// status overwrite one it already knows definitely, so the PREVIOUS status
// stays on screen across the whole restart. Eventual-running is not the
// assertion — a badge that vanished for two seconds and came back would
// satisfy that while being precisely the flicker this rule exists to
// prevent — so the badge is sampled CONTINUOUSLY and every sample must
// find one.
//
// Driven against the real stack, and the restart is issued through the API
// rather than the view's own button: the property is about what the LIST
// shows while a restart runs, and the button lives on the other page. What
// restarts the session is irrelevant to it.
//
// `LIVE_BADGE` matches the bare status word — never the idle-unseen
// annotation, which this test's own subject has nothing to do with, so
// `hideSeenState` keeps this session's row from ever growing the "idle —
// new output" text it would otherwise genuinely earn (SPEC.md, Status: a
// session created here and never opened is unseen from the moment a real
// classifier settles it into idle, exactly what this fixture is). Widening
// `LIVE_BADGE` instead would have been the wrong fix: that constant is
// shared, and every other caller's assertion is about the plain live word.
test("restart-keeps-a-badge-on-screen-throughout", async ({ page, request }) => {
  // The sampling window alone is 20 seconds, and it sits between a real
  // create and a real restart — comfortably past the 60-second default.
  test.setTimeout(120_000);
  const title = `restart-badge-${Date.now()}`;
  let id: string | undefined;
  try {
    const created = await request.post("/api/sessions", {
      data: { cwd: "/tmp", invocation: FAKE_AGENT_INVOCATION, title },
    });
    expect(created.ok(), "creating the session under test").toBe(true);
    id = (await created.json()).id;

    await hideSeenState(page);
    await page.goto("/");
    const badge = rowByTitle(page, title).locator(".status-badge");
    // A DEFINITE status first: the rule under test is about not losing one,
    // so there has to be one to lose.
    await expect(badge).toHaveText(LIVE_BADGE, { timeout: 20_000 });

    const restart = request.post(`/api/sessions/${id}/restart`, {
      data: { mode: "fresh", stop_if_running: true },
    });
    // Sampled while the restart is in flight AND for a stretch afterwards,
    // since the window this is about — the gap between the relaunch and
    // the first classification of the new run — opens after the request
    // returns, not during it.
    const missing: number[] = [];
    const deadline = Date.now() + 20_000;
    let samples = 0;
    while (Date.now() < deadline) {
      if ((await badge.count()) === 0) missing.push(Date.now());
      samples += 1;
      await page.waitForTimeout(100);
    }
    const restarted = await restart;
    expect(restarted.ok(), `restarting ${id}`).toBe(true);
    expect(samples).toBeGreaterThan(50);
    expect(
      missing.length,
      "the list must never blank a session's status badge across a restart: the helm holds " +
        "the previous definite status precisely so this window shows something true rather " +
        "than nothing",
    ).toBe(0);
    // And it is still a live status at the end — the restarted agent got
    // classified, rather than the badge merely being stuck on a stale word.
    await expect(badge).toHaveText(LIVE_BADGE, { timeout: 20_000 });
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

test("restarting a live session confirms first, and only then sends the request with consent", async ({
  page,
  request,
}) => {
  const title = `restart-confirm-${Date.now()}`;
  const bodies: any[] = [];
  await page.route("**/api/sessions/*/restart", async (route) => {
    bodies.push(route.request().postDataJSON());
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
    await waitForTermText(page, "FAKE-AGENT READY");

    // The >= 2 banner count at the end needs the FIRST run's banner to
    // survive the respawn, and only tmux history survives one — the
    // visible grid, where a two-line run's banner still sits, is wiped
    // (SPEC.md, Lifecycle operations/Restart). Scroll it into history
    // before restarting: `spam 60` is more lines than any terminal this
    // test runs in has rows, and the last line's arrival is the barrier,
    // exactly as in the scrollback-retention test below.
    await page.locator("#terminal").click();
    await page.keyboard.type("spam 60");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "spam-line-60");

    // Wait until the view's own status-derived decision says this click
    // will confirm rather than restart outright (`data-confirms`, set from
    // the session's status): the view opens on the create reply's
    // deliberate `Unknown` placeholder and refreshes once, so clicking
    // before that lands would exercise the stale-hint path instead of the
    // confirmation this test is about.
    const restartButton = page.locator(".restart-primary");
    await expect(restartButton).toHaveAttribute("data-confirms", "true", {
      timeout: 15_000,
    });
    // Closed before the first click — the popover this button controls
    // does not exist yet, and `aria-expanded` must say so rather than
    // defaulting to open.
    await expect(restartButton).toHaveAttribute("aria-expanded", "false");

    // The first click only opens the prompt: nothing is sent, and the
    // consequence text says what restarting would do to the running agent.
    await restartButton.click();
    await expect(page.locator(".restart-offer .confirm-consequence")).toContainText(
      "still running",
    );
    expect(bodies).toHaveLength(0);
    // Open now — this is the state a sidebar row menu opened at the same
    // time could visually cover (see app.css's `.header-confirm` z-index
    // comment), so the trigger's own record of it matters independently of
    // the popover being on screen.
    await expect(restartButton).toHaveAttribute("aria-expanded", "true");

    // Cancel returns the view to its normal state, still having sent
    // nothing — the same "cancel is the only way back" rule the delete
    // prompt follows.
    await page.locator(".restart-cancel").click();
    await expect(page.locator(".restart-primary")).toBeVisible();
    expect(bodies).toHaveLength(0);
    await expect(restartButton).toHaveAttribute("aria-expanded", "false");

    await restartButton.click();
    await expect(restartButton).toHaveAttribute("aria-expanded", "true");
    await page.locator(".restart-confirm").click();
    await expect.poll(() => bodies.length).toBe(1);
    expect(bodies[0].stop_if_running).toBe(true);
    // The mode is the one the session's own offer authorizes — a
    // fake-agent session captures no conversation, so a fresh launch is
    // the only honest thing restart can offer it.
    expect(bodies[0].mode).toBe("fresh");

    // And the relaunch actually comes up. Counted rather than merely
    // matched: the spam above pushed the FIRST run's banner into the
    // reused terminal's retained history, so `toContain` would pass
    // without the new run having printed anything at all.
    await expect
      .poll(
        async () => (await termText(page)).split("FAKE-AGENT READY").length - 1,
        {
          timeout: 30_000,
          message: "the relaunched agent's own ready banner",
        },
      )
      .toBeGreaterThanOrEqual(2);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await cleanupSession(request, id);
    }
  }
});

// SPEC.md: "Restart reuses the session's terminal when it still exists —
// whatever scrollback the terminal itself retained is still there" — and,
// in the same paragraph, restart "does NOT preserve the previous run's
// last visible screen: the pane is blank until the new agent draws". So
// what survives a restart is HISTORY, not the visible grid: `respawn-pane`
// keeps tmux's scrollback and reinitializes the screen. The marker is
// therefore pushed off the visible screen before the restart — `spam 60`
// exceeds the fitted browser terminal's height (about 45 rows at most in
// this suite's 1280x720 viewports) — so that its survival is the retained
// scrollback and nothing else. Asserted from the BROWSER's own buffer
// after the restart's remount, which is the only view a user actually has
// of that promise: the buffer starts empty on remount, so everything in it
// afterwards came back through replay of the reused pane's scrollback.
//
// The marker is typed rather than taken from the startup banner, because
// both runs print the same banner — text only the FIRST run could have
// produced is what makes this about retention rather than about the new
// run having printed something.
test("a restarted session's terminal still shows the previous run's scrollback above the new one", async ({
  page,
  request,
}) => {
  const title = `restart-scrollback-${Date.now()}`;
  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FAKE-AGENT READY");

    await page.locator("#terminal").click();
    await page.keyboard.type("PRIOR-RUN-MARKER");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:PRIOR-RUN-MARKER");
    // Push the marker into tmux's history: more lines than any terminal
    // this test runs in has rows. The last spam line's arrival is the
    // barrier, so the restart cannot land while the marker is still on
    // the visible grid that the respawn wipes.
    await page.keyboard.type("spam 60");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "spam-line-60");
    // And one marker that stays on the VISIBLE GRID only, typed after the
    // spam so nothing ever scrolls it into history. Its absence after the
    // restart is the other half of the contract: finding it would mean
    // something preserved the visible grid across the respawn — the
    // forbidden shrink-and-restore trick.
    await page.keyboard.type("GRID-ONLY-MARKER");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:GRID-ONLY-MARKER");

    const restartButton = page.locator(".restart-primary");
    await expect(restartButton).toHaveAttribute("data-confirms", "true", {
      timeout: 15_000,
    });
    await restartButton.click();
    await page.locator(".restart-confirm").click();

    // Three facts at once, and in order: the prior run's typed marker came
    // back out of retained scrollback, the new run's banner is BELOW it,
    // and the grid-only marker did NOT come back. A plain "contains both"
    // would also pass if the restart had never happened (the pre-restart
    // buffer contains both too), so the anchor is the marker's position
    // relative to the LAST banner — and the buffer starts empty on the
    // restart's remount, so a grid-only marker in it could only have come
    // from the replay, never from the pre-restart screen.
    await expect
      .poll(
        async () => {
          const text = await termText(page);
          const marker = text.indexOf("PRIOR-RUN-MARKER");
          const banner = text.lastIndexOf("FAKE-AGENT READY");
          return marker >= 0 && banner > marker && !text.includes("GRID-ONLY-MARKER");
        },
        {
          timeout: 30_000,
          message:
            "prior run's retained scrollback (and nothing from its visible grid) above the relaunched agent's output",
        },
      )
      .toBe(true);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await cleanupSession(request, id);
    }
  }
});

// A restart whose RESPONSE is lost is not a restart that did not happen:
// the request reaches the supervisor, the agent is relaunched, and only
// the reply dies on the way back. The view has to recover from that on its
// own, because the server has already torn its attachment down — a client
// that treated the failure as "nothing happened" would leave the user
// staring at a permanently detached terminal for a session that is running
// perfectly well.
//
// `route.fetch()` then `route.abort()` reproduces exactly that: the real
// request is performed, and the page sees a network error instead of its
// answer.
test("a restart whose response is lost still recovers the terminal", async ({
  page,
  request,
}) => {
  const title = `restart-lost-reply-${Date.now()}`;
  await page.route("**/api/sessions/*/restart", async (route) => {
    await route.fetch();
    await route.abort("connectionfailed");
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
    await waitForTermText(page, "FAKE-AGENT READY");

    await page.locator("#terminal").click();
    await page.keyboard.type("BEFORE-LOST-RESTART");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:BEFORE-LOST-RESTART");

    // The marker anchors the ordering assertion below, so it has to
    // survive the respawn — and only tmux history survives one; the
    // visible grid, where the marker still sits, is wiped (SPEC.md,
    // Lifecycle operations/Restart). Scroll it off-screen first.
    await page.keyboard.type("spam 60");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "spam-line-60");

    const restartButton = page.locator(".restart-primary");
    await expect(restartButton).toHaveAttribute("data-confirms", "true", {
      timeout: 15_000,
    });
    await restartButton.click();
    await page.locator(".restart-confirm").click();

    // The failure is surfaced rather than swallowed — the user is owed
    // that much when their action's outcome is genuinely unknown to the
    // client.
    await expect(page.locator(".restart-error")).toBeVisible({ timeout: 15_000 });

    // ...and the view recovers anyway: it re-reads the session, remounts,
    // and the relaunched agent's own banner appears BELOW the previous
    // run's output in the reused terminal. A view that had concluded
    // "nothing happened" would sit detached here forever.
    await expect
      .poll(
        async () => {
          const text = await termText(page);
          const marker = text.indexOf("BEFORE-LOST-RESTART");
          const banner = text.lastIndexOf("FAKE-AGENT READY");
          return marker >= 0 && banner > marker;
        },
        {
          timeout: 30_000,
          message: "the relaunched agent's terminal, recovered after a lost reply",
        },
      )
      .toBe(true);

    // The session really was restarted, which is what makes the recovery
    // the correct behavior rather than a lucky one.
    const listing = await (await request.get("/api/sessions")).json();
    const session = listing.sessions.find((s: any) => s.title === title);
    expect(session).toBeTruthy();
    expect(LIVE_STATES).toContain(session.status.state);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await cleanupSession(request, id);
    }
  }
});

// MT-4 (manual testing): restarting a live session left the red "Detached:
// session restarted" banner painted over a terminal that had, by the time
// a human noticed it, already reattached and was working fine — typing
// round-tripped, output rendered, the banner just never went away. Root
// cause was `#term-banner` (farhelm-ui/src/lib.rs) living OUTSIDE the
// `#terminal` div terminal.js remounts, so nothing about a later mount
// ever told a PRIOR mount's sticky banner to clear (terminal.js's
// `showBanner` is deliberately sticky FOR THE LIFE OF ITS OWN SOCKET, so a
// takeover reason survives the generic close that follows it — see that
// function's docs — but nothing was clearing it for the NEXT socket
// either). The fix hooks the new socket's `onopen` — a transport-level
// signal (the upgrade completed), not proof the supervisor-side attach
// succeeded; clearing there is still honest because a failed attach
// closes that same socket and its own close handler re-banners.
//
// This is exactly the restart sequence that produces the bug: a live
// agent's restart tears the OLD attachment down with reason "session
// restarted" (`detach_for_restart`, farhelm-supervisor/src/service.rs)
// before the new one ever exists. Proving the banner APPEARED cannot be a
// locator poll — the fix clears it as soon as the new socket opens, which
// on loopback routinely beats Playwright's first poll, so the transient
// visible state is unobservable from outside (this test flaked exactly
// that way when it polled). A MutationObserver installed before the page
// loads records every banner transition instead, so the assertion reads
// the recorded history: shown with the exact restart reason, then hidden
// once the relaunch is confirmed live — the real sticky-then-clear
// sequence, with no window for the poll to miss.
test("a restarted session's banner clears once the new attachment is live", async ({
  page,
  request,
}) => {
  const title = `restart-banner-clears-${Date.now()}`;
  try {
    // Armed ON DEMAND rather than at DOMContentLoaded: auto-select mounts
    // a different session's view (and #term-banner) at load, and an
    // observer armed on THAT element would record the wrong terminal's
    // banner while the created session's went unobserved.
    await page.addInitScript(() => {
      (window as any).__bannerLog = [];
      (window as any).__armBannerLog = () => {
        const el = document.getElementById("term-banner");
        if (!el) throw new Error("no term-banner to observe");
        new MutationObserver(() => {
          (window as any).__bannerLog.push({
            shown: el.style.display === "block",
            text: el.textContent,
          });
        }).observe(el, {
          attributes: true,
          attributeFilter: ["style"],
          childList: true,
          characterData: true,
          subtree: true,
        });
      };
    });
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    await expect(page.locator(".titlebar .title")).toHaveText(title, { timeout: 15_000 });
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FAKE-AGENT READY");

    // The count-of-two anchor at the end needs the FIRST run's banner in
    // tmux history, because the respawn wipes the visible grid where it
    // would otherwise still sit (SPEC.md, Lifecycle operations/Restart).
    // Done before arming the banner observer so the observer's window
    // stays tight around the restart itself.
    await page.locator("#terminal").click();
    await page.keyboard.type("spam 60");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "spam-line-60");

    await page.evaluate(() => (window as any).__armBannerLog());

    const restartButton = page.locator(".restart-primary");
    await expect(restartButton).toHaveAttribute("data-confirms", "true", {
      timeout: 15_000,
    });
    await restartButton.click();
    await page.locator(".restart-confirm").click();

    // The banner's appearance is read from the observer's recorded
    // history (see the doc comment above for why a locator poll cannot
    // see it): the old attachment's detach must have painted the banner
    // with the restart's exact reason at SOME point, however briefly.
    await expect
      .poll(
        async () =>
          await page.evaluate(() =>
            (window as any).__bannerLog.some(
              (e: { shown: boolean; text: string }) =>
                e.shown && e.text.includes("Detached: session restarted"),
            ),
          ),
        { timeout: 15_000, message: "the restart's detach banner was recorded" },
      )
      .toBe(true);

    // The relaunch comes up in the SAME (reused) terminal, so its ready
    // banner is the SECOND occurrence in the buffer — the same anchor the
    // confirm test above uses to prove the new run actually printed
    // something rather than merely reattaching to stale output.
    await expect
      .poll(
        async () => (await termText(page)).split("FAKE-AGENT READY").length - 1,
        { timeout: 30_000, message: "the relaunched agent's own ready banner" },
      )
      .toBeGreaterThanOrEqual(2);

    // The bug: the OLD attachment's detach banner stayed painted over a
    // terminal that is now genuinely live again.
    await expect(page.locator("#term-banner")).toBeHidden({ timeout: 15_000 });

    // And "live" is proven functionally, not just by the banner's
    // absence: typing still round-trips through the new attachment.
    await page.locator("#terminal").click();
    await page.keyboard.type("post-restart-roundtrip");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:post-restart-roundtrip", 10_000);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await cleanupSession(request, id);
    }
  }
});
