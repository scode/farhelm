// The four browser tests M6.5's review deferred to this milestone BY NAME
// (PLAN_M6_75.md item 7, acceptance 5): the stop-refetch-versus-poll
// ordering, the restart-epoch staleness guard, the incoherent-banner case,
// and the PeerLine render-shape pin.
//
// They are together in one file because they share a reason for existing
// rather than a subject: each one needs the CLIENT to be put in a state a
// real server will not reliably produce — a read still in flight when a
// mutation lands, a reply whose counts contradict each other, a build stamp
// containing a directional override. All four are therefore route-controlled,
// and all four were deferred precisely because that control did not exist in
// the suite when they were first asked for.
//
// The first two are also the reason the deferral was worth honouring. They
// pin the COMMIT CLOSURES — `list::ListView`'s `commit_listing` and
// `session_view`'s `commit_detail` — which M6.75 turned into the
// invalidation feed's consumers. Their generation and epoch guards are now
// load-bearing for a mechanism nobody had written when the tests were
// postponed, so they land here checking more than they were written for.
import { APIRequestContext, expect, Page, Route, test } from "@playwright/test";
import {
  cleanupSession,
  createSession,
  holdMutation,
  holdReads,
  listSessions,
  renameSession,
  stopSession,
  stubFeed, openHostsPanel, openRowMenu } from "./helpers/fleet";

/** The row for one session id, as the list renders it. */
function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

/**
 * Wait until the helm reports this session running again.
 *
 * The observable end of a restart, and the only one this suite has: nothing
 * on the wire identifies a RUN, so "the relaunch happened" has to be read off
 * the status leaving the `exited` the stop before it produced. A test that
 * acted on the click alone would be acting on a request that may not have
 * reached the supervisor yet.
 *
 * A LIVE status rather than "anything but exited", because the absence of a
 * row reads as the latter: a title that matches nothing yields no status at
 * all, and a test built on that would sail past a restart that never
 * happened.
 */
async function waitForRestart(request: APIRequestContext, title: string): Promise<void> {
  await expect
    .poll(async () => {
      const listed = await listSessions(request, `title=${encodeURIComponent(title)}`);
      return listed.sessions[0]?.status?.state ?? "no such session";
    }, {
      timeout: 30_000,
      message: "the restart must have produced a new run server-side",
    })
    .toMatch(/^(running|waiting|idle)$/);
}

test.describe("the M6.5 test debts", () => {
  const created: string[] = [];

  test.afterEach(async ({ request }) => {
    while (created.length) {
      const id = created.pop();
      if (id) await cleanupSession(request, id);
    }
  });

  /**
   * A listing read that left BEFORE a stop must not undo what a NEWER read
   * showed.
   *
   * The race is ordinary rather than exotic: a listing walk is several round
   * trips, so one easily spans a stop — and before the generation gate, the
   * older walk's pre-stop `running` badge would land on top of the fresher
   * `exited` one and sit there until something else refreshed. What makes it
   * worth a browser test rather than a unit test is that the ORDER is the
   * bug: both replies are perfectly valid, and only when each was ASKED FOR
   * decides which one speaks for the screen.
   *
   * The claim is deliberately about ordering against ANY newer read rather
   * than about the stop handler's own refetch specifically. Two reads answer
   * a stop now — the handler's refetch and the re-read triggered by the
   * revision the stop bumped — and this test cannot tell which of them
   * painted, nor does the generation gate care: it admits the later request
   * and rejects the earlier one whatever started them. M6.75 therefore
   * raises the stakes rather than retiring them; the poll is gone and a
   * second legitimate re-read took its place.
   *
   * ## The window, and why it has to be held open
   *
   * The re-read the revision bump triggers does not race the stale walk any
   * more: it QUEUES behind it, because the listing surface reads one at a
   * time (`reader`). So a test that released the stale reply and then looked
   * would be looking after the follow-up had already repaired the screen — it
   * would pass with the generation gate deleted, which is the one thing it
   * must not do. This test therefore holds the follow-up as well and asserts
   * in between: stale reply delivered, repair still held, badge inspected. In
   * that window the gate is the only thing standing between a pre-stop
   * `running` and the screen.
   *
   * The stop's REPLY is held until the kill has settled server-side, which
   * makes the refetch it triggers a read that can honestly say `exited` —
   * against a real supervisor that refetch often leaves before the status
   * moves, and this test would then be waiting on a repaint that is nobody's
   * bug.
   */
  test("a listing read older than a stop cannot resurrect the pre-stop status", async ({
    page,
    request,
  }) => {
    test.setTimeout(120_000);
    const session = await createSession(request, { title: `stop-order-${Date.now()}` });
    created.push(session.id);

    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });
    // The row must be showing a LIVE status before the stop, or there is
    // nothing for a stale reply to resurrect.
    await expect(row(page, session.id).locator(".status-badge")).toHaveText(/running|idle|waiting/, {
      timeout: 30_000,
    });

    const reads = await holdReads(page, (url) => url.pathname === "/api/sessions");
    const settled = await holdMutation(
      page,
      (url) => url.pathname === `/api/sessions/${session.id}/stop`,
    );

    // Read #1: a walk that reads the world as it is NOW — pre-stop, `running`
    // — and will be delivered long after the stop has been painted.
    feed.notify(2);
    await reads.waitForCaptures(1);

    // The stop. Its reply is held until the helm agrees the session has
    // ended, so the refetch it triggers on arrival is a read that can see
    // the new status.
    await openRowMenu(row(page, session.id));
    await row(page, session.id).locator(".session-row-stop").click();
    await expect
      .poll(async () => {
        const listed = await listSessions(request, `title=${encodeURIComponent(session.title)}`);
        return listed.sessions[0]?.status?.state;
      }, { timeout: 20_000 })
      .toBe("exited");
    settled();

    // Read #2: the stop handler's own refetch, which is NOT routed through
    // the surface reader (it exists to show one session's new status at
    // once), so it arrives while read #1 is still held. Delivering it paints
    // the outcome the stale walk must not undo.
    await reads.waitForCaptures(2);
    reads.release(2);
    await expect(row(page, session.id).locator(".status-badge")).toHaveText(/exited/, {
      timeout: 20_000,
    });

    // The notification the stop's revision bump stands for. The reader is
    // busy with read #1, so this becomes its follow-up rather than a read of
    // its own — the repair, and it must not be allowed to run yet.
    feed.notify(3);

    // The stale walk lands. The follow-up is dispatched the moment it does,
    // and the hold captures it — so reaching three captures is proof both
    // that the stale reply was delivered and that nothing has repaired the
    // screen since.
    reads.release(1);
    await reads.waitForCaptures(3);
    await expect(
      row(page, session.id).locator(".status-badge"),
      "a reply asked for before the stop must not speak for the screen after it — and with the " +
        "repair still held there is nothing else here to correct it",
    ).toHaveText(/exited/);

    // And the repair, when it is finally allowed to run, agrees.
    reads.releaseAll();
    await expect(row(page, session.id).locator(".status-badge")).toHaveText(/exited/, {
      timeout: 20_000,
    });
  });

  /**
   * A detail read that left BEFORE a restart must not describe the run that
   * no longer exists — not even to say it is GONE.
   *
   * `restart_epoch` versions every detail read against restart ATTEMPTS, and
   * this is the case it exists for: a read in flight when the user restarts
   * comes back describing the previous run, and acting on it would put the
   * old state back on top of what the restart is about to establish.
   *
   * ## The instrument is a 404, and that is the point
   *
   * A pre-restart reply that merely carries an older title is a weak probe:
   * whatever it paints, the refresh the restart owes paints over a moment
   * later, so the screen ends up correct either way and the test cannot tell
   * a working guard from an absent one. A 404 is different in kind. It is not
   * data to be overwritten — it is a CLAIM ("this session is not in the
   * helm's listing"), and the view's honest response to a claim it believes
   * is to raise the staleness notice. Believing one that describes a run the
   * user has just replaced is exactly the failure the epoch exists to
   * prevent, and it leaves a visible mark that a later good reply does not
   * erase for free.
   *
   * So the reply is delivered while the restart's own reply is still HELD:
   * the client has taken its first bump and cannot yet have refreshed, which
   * makes the absence of `.refresh-stale` in that window a statement about
   * the guard and nothing else.
   *
   * This is the FIRST bump's case — a read already in flight when the user
   * clicks. The test below it covers the second bump, which exists for a
   * read that starts during the restart and would otherwise pass the guard.
   */
  test("a detail read older than a restart cannot describe the previous run", async ({
    page,
    request,
  }) => {
    // A real create, a real stop and a real restart, each on the supervisor's
    // own schedule: past the 60-second default by construction.
    test.setTimeout(120_000);
    const original = `restart-epoch-${Date.now()}`;
    const session = await createSession(request, { title: original });
    created.push(session.id);
    // Stopped first, so the restart click acts outright rather than opening
    // the live-agent confirmation — the confirmation is a different feature
    // with its own coverage, and routing through it here would only add a
    // click between the two things being ordered.
    await stopSession(request, session.id);
    // Settled server-side before the page's one look: the stop returns when
    // the kill is issued, not when "exited" reaches the helm's cache, and a
    // stubbed silent feed means the page never re-reads on its own.
    await expect
      .poll(async () => {
        const listed = await listSessions(request, `title=${encodeURIComponent(original)}`);
        return listed.sessions[0]?.status?.state;
      }, { timeout: 20_000 })
      .toBe("exited");

    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, session.id).locator(".status-badge")).toHaveText(/exited/, {
      timeout: 20_000,
    });
    await row(page, session.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toHaveText(original, { timeout: 20_000 });
    await expect(page.locator(".restart-primary")).toHaveAttribute("data-confirms", "false");
    await expect(page.locator(".refresh-stale")).toHaveCount(0);

    const reads = await holdReads(page, (url) => url.pathname === `/api/sessions/${session.id}`);
    const finishRestart = await holdMutation(
      page,
      (url) => url.pathname === `/api/sessions/${session.id}/restart`,
    );

    // Read #1, dispatched before the restart: it belongs to the run the
    // restart is about to replace.
    feed.notify(2);
    await reads.waitForCaptures(1);

    // The restart, with its reply held — so the client is mid-restart from
    // here until this test says otherwise: one epoch bump taken, the second
    // and the refresh it precedes still to come.
    await page.locator(".restart-primary").click();

    // The pre-restart read answers 404. Nothing may come of it.
    reads.release(1, { status: 404, body: `no such session: ${session.id}\n` });
    // Given time to be believed before being disbelieved: a second is many
    // round trips on this stack, and the restart that would repair the screen
    // is still held, so anything on screen at the end of it is what the 404
    // did.
    await page.waitForTimeout(1_000);
    await expect(
      page.locator(".refresh-stale"),
      "a 404 about the run that just ended is not evidence about the run that replaced it",
    ).toHaveCount(0);
    // The other half of a discarded reply: it applied nothing, so the demand
    // behind it is still owed and the reader asks again. (Held, like
    // everything else here, so it cannot repair anything yet.)
    await reads.waitForCaptures(2);

    // A change the discarded read could not have carried, so the refresh that
    // ends this test is visibly the one that spoke.
    const renamed = `${original}-after`;
    await renameSession(request, session.id, renamed);

    // The restart completes: second bump, then its own refresh asked for
    // through the same door every other read uses.
    finishRestart();
    reads.releaseAll();
    await expect(page.locator(".titlebar .title")).toHaveText(renamed, { timeout: 30_000 });
    await expect(page.locator(".refresh-stale")).toHaveCount(0);
  });

  /**
   * The SECOND epoch bump's case: a detail read that starts DURING a restart
   * is rejected too.
   *
   * The first bump — taken before the request goes out — only invalidates
   * reads that were already in flight. A read launched while the restart runs
   * carries the new epoch and passes that guard, so without a second bump it
   * would be admitted afterwards, describing the run that just ended. The
   * bump taken when the restart finishes narrows the admissible set to reads
   * launched after it, which is exactly the set that can have seen its
   * result.
   *
   * ## Bump, then refresh — and why the refresh cannot stand in for the bump
   *
   * The restart's own refresh is no longer a direct fetch: it is asked for
   * through the same door and the same reader as every other read
   * (`session_view`'s restart closure), issued after the final bump so it
   * carries the epoch it will be judged against. That is better in every way
   * except one — it makes a test lazy. Both the mid-restart reply and the
   * refresh queue on ONE reader, so the refresh lands microseconds behind the
   * stale reply it is supposed to be protected from, and a test that looked
   * afterwards would see a correct screen whether or not anything rejected
   * the stale reply.
   *
   * So the refresh is held too, and the assertion happens in between. In that
   * window the mid-restart reply has been delivered and nothing else has run:
   * if the guard admitted it, its value is on screen with nothing to correct
   * it.
   *
   * ## The signature
   *
   * The held reply is re-served with a title no server ever produced. A
   * pre-rename title would be ambiguous — it is also what the screen shows
   * when nothing happened at all — while a value that exists nowhere but in
   * this fixture can only be there because the reply was believed.
   */
  test("a detail read that starts during a restart cannot describe the previous run", async ({
    page,
    request,
  }) => {
    test.setTimeout(120_000);
    const original = `restart-inflight-${Date.now()}`;
    const session = await createSession(request, { title: original });
    created.push(session.id);
    await stopSession(request, session.id);
    await expect
      .poll(async () => {
        const listed = await listSessions(request, `title=${encodeURIComponent(original)}`);
        return listed.sessions[0]?.status?.state;
      }, { timeout: 20_000 })
      .toBe("exited");

    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });
    await row(page, session.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toHaveText(original, { timeout: 20_000 });
    await expect(page.locator(".restart-primary")).toHaveAttribute("data-confirms", "false");

    const reads = await holdReads(page, (url) => url.pathname === `/api/sessions/${session.id}`);
    // The restart happens at once on the server; only the client's knowledge
    // of it is delayed, which is precisely the state the second bump is
    // written for and one a real supervisor passes through too fast to assert
    // in.
    const finishRestart = await holdMutation(
      page,
      (url) => url.pathname === `/api/sessions/${session.id}/restart`,
    );

    await page.locator(".restart-primary").click();
    // Inside the restart: this read takes the epoch the first bump produced,
    // which is the epoch that would let it through.
    feed.notify(2);
    await reads.waitForCaptures(1);

    // The restart finishes. The button coming back is the second bump having
    // happened — it is re-enabled immediately after it (`restarting`) — and
    // waiting for it is what keeps this test from releasing the stale reply
    // into a window where admitting it would have been correct.
    finishRestart();
    await expect(page.locator(".restart-primary")).toBeEnabled({ timeout: 30_000 });

    // The mid-restart reply, wearing a title that exists nowhere else.
    const signature = `${original}-from-the-run-that-ended`;
    const midRestart = JSON.parse(reads.reply(1).body);
    reads.release(1, { body: JSON.stringify({ ...midRestart, title: signature }) });

    // The refresh the restart owes is dispatched the moment that reply is
    // dealt with, and the hold captures it — so reaching two captures is
    // proof both that the stale reply landed and that nothing has repaired
    // the screen since.
    await reads.waitForCaptures(2);
    await expect(
      page.locator(".titlebar .title"),
      "a read launched during a restart describes the run that ended, and the second bump is " +
        "the only thing between it and the header while the refresh is held",
    ).toHaveText(original);
    // Held across a beat as well, since "never rendered" is the claim and a
    // single look is a single frame. An exact-text assertion is what makes
    // this say what it means: the signature CONTAINS the original title, so
    // anything looser would pass while the header wore it.
    await page.waitForTimeout(1_000);
    await expect(page.locator(".titlebar .title")).toHaveText(original);

    // And the refresh, once allowed to run, agrees with the server.
    reads.releaseAll();
    await waitForRestart(request, original);
    await expect(page.locator(".titlebar .title")).toHaveText(original);
  });

  /**
   * A reply whose rows outnumber its own total is reported as incoherent, in
   * the browser, with the suffix as its own text run.
   *
   * Unreachable without route control, which is why it waited: it takes a
   * listing that changed under the walk in a specific direction — a session
   * deleted from an earlier page while the rows already taken stay taken —
   * and no test can arrange that reliably against a real helm.
   *
   * What it pins beyond the wording is the SPLIT: the count and the
   * incoherence note are two text runs inside one banner element, because
   * fusing them would change the DOM without changing any string, which a
   * text-only assertion sails straight past.
   */
  test("a listing whose counts contradict its rows says so in the banner", async ({ page }) => {
    await stubFeed(page);
    await page.route(
      (url) => url.pathname === "/api/sessions",
      async (route: Route) => {
        if (route.request().method() !== "GET") {
          await route.continue();
          return;
        }
        await route.fulfill({
          json: {
            // Two rows against a total of one: the walk collected more than
            // the helm says exists, which is only possible if the list moved
            // underneath it.
            sessions: [
              { id: "incoherent-a", title: "incoherent-a", cwd: "/tmp", invocation: "agent" },
              { id: "incoherent-b", title: "incoherent-b", cwd: "/tmp", invocation: "agent" },
            ],
            total: 1,
            matching: 1,
            truncated: false,
            next_cursor: null,
          },
        });
      },
    );
    await page.goto("/");

    const banner = page.locator(".truncation-banner");
    await expect(banner).toBeVisible({ timeout: 20_000 });
    // The UNFILTERED shortfall wording: the request carried no filter a
    // person applied, so the sentence has one denominator and the note
    // beside it carries the contradiction.
    await expect(banner).toContainText("showing 2 of 1 sessions");
    await expect(banner).toContainText("the list changed while it was being read");
    // Two runs, not one sentence: the count and the note are separate text
    // nodes inside the banner.
    const runs = await banner.evaluate((element) =>
      [...element.childNodes]
        .filter((node) => node.nodeType === Node.TEXT_NODE)
        .map((node) => node.textContent ?? ""),
    );
    expect(runs.length, `the banner's two claims must stay separate runs: ${JSON.stringify(runs)}`)
      .toBe(2);
  });

  /**
   * A peer-supplied string renders in its OWN direction-isolated element,
   * with directional controls escaped.
   *
   * The rule lives in `farhelm-ui/src/peer.rs` and its escaping half is unit
   * tested there; what only a browser can check is the half that is a
   * RENDER shape — that each relayed value is a separate `span.peer-value`
   * carrying `dir="ltr"` and computing to `unicode-bidi: isolate`, rather
   * than being concatenated into the sentence around it. Escaping alone
   * would not be enough: strong-RTL letters reorder text with no control
   * character for an escape rule to catch, and isolation is what bounds
   * them.
   *
   * The vehicle is a host stuck on an identity mismatch, whose detail line
   * is the densest mixture of the two kinds of text in the app: three runs
   * this UI wrote and two identities it merely relays, alternating. It is
   * also the sentence whose meaning a reordering attack actually changes —
   * "recorded as X; the destination now reports Y" decides which install a
   * user is about to adopt.
   *
   * Delivered through the hosts JSON rather than through the build-stamp
   * header, and that is a constraint rather than a preference: an HTTP
   * header carries visible ASCII, so a directional control cannot reach the
   * skew banner at all — `reqwest` refuses to read such a value and the page
   * sees no stamp instead.
   */
  test("a peer value renders isolated in its own element with its controls escaped", async ({
    page,
  }) => {
    await stubFeed(page);
    // A directional override inside an identity: the concrete attack the
    // isolation and the escaping exist for, together.
    const hostile = "identity-‮reported";
    await page.route(
      (url) => url.pathname === "/api/hosts",
      async (route: Route) => {
        if (route.request().method() !== "GET") {
          await route.continue();
          return;
        }
        await route.fulfill({
          json: {
            hosts: [
              {
                id: 1,
                kind: "ssh",
                destination: "user@box",
                name: "user@box",
                identity: "identity-recorded",
                remote_farhelm: null,
                remote_state_dir: null,
                state: {
                  phase: "identity-mismatch",
                  recorded: "identity-recorded",
                  reported: hostile,
                },
              },
            ],
          },
        });
      },
    );
    await page.goto("/");
    // The host detail line lives in the hosts panel, which sits behind
    // the sidebar's toggle now.
    await openHostsPanel(page);

    const detail = page.locator(".host-detail");
    await expect(detail).toBeVisible({ timeout: 20_000 });

    // Two relayed identities, each in its own element — the recorded one and
    // the reported one, which is exactly the pair a reordering attack would
    // want to swap.
    const peers = detail.locator("span.peer-value");
    await expect(peers).toHaveCount(2);
    const reported = peers.nth(1);
    await expect(reported).toHaveText("identity-<U+202E>reported");
    await expect(reported).toHaveAttribute("dir", "ltr");
    expect(
      await reported.evaluate((element) => getComputedStyle(element).unicodeBidi),
    ).toContain("isolate");

    // And this UI's own words are NOT inside that element: a peer value
    // sharing a span with the sentence around it could lay that sentence
    // out, which is exactly what the split prevents. The whole line still
    // reads correctly, so the isolation costs nothing legible.
    await expect(reported).not.toContainText("recorded as install");
    await expect(detail).toContainText("recorded as install");
    await expect(detail).toContainText("the destination now reports");
  });
});
