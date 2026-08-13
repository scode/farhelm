// The rules about READS themselves, in a real browser: which reply is
// allowed to speak for a surface, and what happens to a demand whose read
// never answered.
//
// A per-area file of its own, per this milestone's convention (see
// titlebar.spec.ts's header). The area is the pair of mechanisms M6.75 grew
// underneath every surface once the periodic loops were removed —
// `farhelm-ui/src/ops.rs`'s generation gates and staleness epochs, which
// decide which of several completed reads counts, and
// `farhelm-ui/src/reader.rs`'s single-flight reader, which decides how many
// reads a page may have in the air and what it owes when one fails. Their
// unit tests drive the state machines directly; what only a browser can show
// is the two of them composing against a real helm, a real socket and a real
// screen.
//
// Every test here is route-controlled, and for one reason: both mechanisms
// only matter INSIDE a window a healthy stack closes in milliseconds — a
// reply in flight while the world changes underneath it, a read that fails
// with nothing left to ask again. The fixtures (`helpers/fleet`'s
// `holdReads` and `holdMutation`) hold that window open rather than racing
// it, which is also what keeps these tests from passing when the mechanism
// under test is deleted: a queued repair landing microseconds after the
// damage is invisible to any assertion taken afterwards.
import { expect, Page, Route, test } from "@playwright/test";
import {
  cleanupSession,
  countReads,
  createSession,
  holdMutation,
  holdReads,
  listSessions,
  stubFeed, openRowMenu } from "./helpers/fleet";

/** The row for one session id, as the list renders it. */
function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

test.describe("read ordering and recovery", () => {
  const created: string[] = [];

  test.afterEach(async ({ request }) => {
    while (created.length) {
      const id = created.pop();
      if (id) await cleanupSession(request, id);
    }
  });

  /**
   * A host reading taken while a session was STALE must explain nothing once
   * the session is fresh again.
   *
   * The session view reads `/api/hosts` for exactly one purpose: to say WHY a
   * stale session has no terminal. That read is slow relative to the thing it
   * explains — a session can go stale and come back while one is in flight —
   * and the reply is then not merely old but about a different situation
   * entirely. Committing it would explain a healthy session with the outage
   * that just ended.
   *
   * The half with teeth is what the commit would DO next. A registry reply
   * saying the host is connected, arriving for a session that still reads
   * stale, is a disagreement between two reads, and the view closes it by
   * asking for a fresh session read (`session_view`'s reconnect follow-up).
   * For a session that is ALREADY fresh that follow-up is a read nobody asked
   * for, taken on the strength of a reading from before the recovery — so the
   * discriminating observable here is not the notice (which is gone the
   * moment the session is fresh, gate or no gate) but the absence of that
   * extra detail read.
   *
   * The staleness is injected because it cannot be arranged: it needs a host
   * that goes away and comes back under a session this suite can also drive,
   * and the harness's fleet cannot do that to order. Only the one flag is
   * rewritten — the host id is left alone, so the registry really does report
   * that host as connected, which is what makes the follow-up tempting.
   */
  test("a host reading from before a session went fresh explains nothing", async ({
    page,
    request,
  }) => {
    test.setTimeout(120_000);
    const session = await createSession(request, { title: `stale-epoch-${Date.now()}` });
    created.push(session.id);

    // The staleness switch. Flipped by the test, read by every detail reply
    // on its way to the page.
    let stale = true;
    await page.route(
      (url) => url.pathname === `/api/sessions/${session.id}`,
      async (route: Route) => {
        if (route.request().method() !== "GET") {
          await route.continue();
          return;
        }
        const response = await route.fetch();
        const detail = await response.json();
        await route.fulfill({ response, json: { ...detail, stale } });
      },
    );

    const feed = await stubFeed(page);
    const reads = countReads(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });

    // Held from here on, so the registry reply this test is about cannot land
    // until it says so.
    const registry = await holdReads(page, (url) => url.pathname === "/api/hosts");

    await row(page, session.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toHaveText(session.title, { timeout: 20_000 });
    // The stale surface is up, and its explanation is out asking the registry
    // — that read is capture #1 and it is going nowhere for now.
    await expect(page.locator(".host-stale-notice")).toBeVisible({ timeout: 20_000 });
    await registry.waitForCaptures(1);

    // The session comes back. The detail read that says so crosses the
    // staleness line, which is what makes the held reply describe a situation
    // that no longer exists.
    stale = false;
    feed.notify(2);
    await expect(page.locator(".host-stale-notice")).toHaveCount(0, { timeout: 20_000 });
    // Settled, so the reads that belong to the transition are behind us and
    // the window below contains only what the release causes.
    await page.waitForTimeout(1_500);
    const before = reads.count("detail");

    // The reply from the outage lands.
    registry.releaseAll();
    await page.waitForTimeout(3_000);

    expect(
      reads.count("detail") - before,
      `a registry reading from before the recovery must not send this view looking again; saw ${
        reads.urls("detail").slice(before).join(", ")
      }`,
    ).toBe(0);
    await expect(
      page.locator(".host-stale-notice"),
      "and it must not re-raise the notice it was fetched to explain",
    ).toHaveCount(0);
  });

  /**
   * A stop's own refetch that FAILS hands its demand to the surface reader,
   * and the list comes back without anyone asking again.
   *
   * The stop takes a read of its own (`ListView`'s `on_stop`), outside the
   * reader, so that one session's new status reaches the screen at once
   * rather than whenever a walk that is already running finishes. That choice
   * has a cost, and this is it: nothing outside the reader ever retries, so a
   * dropped request would replace a perfectly good list with an error line
   * and leave it there. Not for a poll interval — FOREVER, because the poll
   * is gone: the feed is healthy so the fallback is off, and the notification
   * this stop caused has already been spent. The next repaint would wait for
   * the fleet to change again, which on a quiet fleet is never.
   *
   * The handoff is one line (`stop_recovery(Trigger::Explicit)`) and this
   * test is what stops it being deleted as redundant. Delete it and the page
   * stays on its error line for the rest of the session: the assertions below
   * would find no second read at all.
   *
   * ## Why the recovery read is held too
   *
   * The handoff dispatches at once — an explicit demand carries news and does
   * not wait out a backoff — so the error line it repairs is on screen for
   * about as long as one round trip. A test that looked for it would be
   * racing the fix. Holding the recovery read makes the failed state stable,
   * and makes its existence the proof the handoff happened: nothing else in
   * this test asks for a read after the failure.
   */
  test("a failed stop refetch is handed to the reader, which brings the list back", async ({
    page,
    request,
  }) => {
    test.setTimeout(120_000);
    const session = await createSession(request, { title: `stop-handoff-${Date.now()}` });
    created.push(session.id);

    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, session.id).locator(".status-badge")).toHaveText(/running|idle|waiting/, {
      timeout: 30_000,
    });

    const reads = await holdReads(page, (url) => url.pathname === "/api/sessions");
    // The stop's reply waits until the kill has settled server-side, so the
    // refetch it triggers is a read that could have shown the new status —
    // which is what makes failing it a loss rather than a non-event.
    const settled = await holdMutation(
      page,
      (url) => url.pathname === `/api/sessions/${session.id}/stop`,
    );

    await openRowMenu(row(page, session.id));
    await row(page, session.id).locator(".session-row-stop").click();
    await expect
      .poll(async () => {
        const listed = await listSessions(request, `title=${encodeURIComponent(session.title)}`);
        return listed.sessions[0]?.status?.state;
      }, { timeout: 20_000 })
      .toBe("exited");
    settled();

    // Capture #1 is the stop's own refetch. It fails, and the list has
    // nothing left to render into.
    await reads.waitForCaptures(1);
    reads.release(1, { status: 500, body: "injected stop refetch failure" });
    await expect(page.locator(".status.error")).toBeVisible({ timeout: 20_000 });
    await expect(row(page, session.id)).toHaveCount(0);

    // Capture #2 exists only if the failed read's demand was handed to the
    // reader. NOTHING in this test asks for it: the feed is a stub and has
    // been silent since the handshake, the fallback is off on a healthy feed,
    // and the stop's own read is over.
    await reads.waitForCaptures(2);
    await expect(
      page.locator(".status.error"),
      "the error line is the state the handoff has to rescue the page from",
    ).toBeVisible();

    // And it does, on its own.
    reads.releaseAll();
    await expect(page.locator(".status.error")).toHaveCount(0, { timeout: 20_000 });
    await expect(row(page, session.id).locator(".status-badge")).toHaveText(/exited/, {
      timeout: 20_000,
    });
    expect(
      feed.connections(),
      "and without the socket having been touched: no reconnection, no second handshake",
    ).toBe(1);
  });
});
