/**
 * The browser side of the unconfirmed-delete precondition
 * (`DeleteSession::only_if_nothing_alive`).
 *
 * The sidebar deletes a session without asking when its row shows the agent
 * ended and no terminal tabs open, but that row can be stale: another client
 * may have restarted the session since it was drawn. So the one delete the
 * user did not confirm carries a precondition, and the supervisor refuses it
 * if anything turned out to be alive. The supervisor's check and the helm's
 * pass-through have their own Rust tests; this spec pins the part only a
 * browser can show: that the unconfirmed click really sends the
 * precondition, and that a refusal reaches the user as the row's ordinary
 * delete error, with the row kept, no confirmation opened on its own, and a
 * fresh listing read so the next click decides from current state.
 *
 * The refusal is injected with `page.route` rather than produced by a real
 * race, because making a live supervisor disagree with a row the page has
 * already drawn would need a restart timed between the render and the click.
 * The injected reply has the shape the helm gives a supervisor `Conflict`:
 * status 409 with the supervisor's message as the body, and the helm's own
 * build stamp, without which the page would read the reply as version skew
 * and withdraw the very reads this spec counts.
 *
 * The invalidation feed is stubbed and never notified after its handshake,
 * so once setup work has retired, a listing read can only come from the
 * refusal itself.
 */
import { expect, test } from "./helpers/evidence";
import type { Route } from "@playwright/test";
import {
  cleanupSession,
  countReads,
  createSession,
  listSessions,
  observeFeedReaders,
  openRowMenu,
  stopSession,
  stubFeed,
  waitForFeedReadersSettled,
} from "./helpers/fleet";

const REFUSAL = "cannot delete without confirmation: the agent is still running";

/**
 * Why this matters: without the precondition an unconfirmed delete from a
 * stale row kills an agent nobody agreed to stop, and without a visible
 * error and a re-read the user would keep clicking a Delete that keeps
 * failing. Spec: a Delete on an ended, tabless row opens no confirmation and
 * sends `only_if_nothing_alive=true`; a 409 reply shows as
 * "delete: <message>" on the row, keeps the row, opens no confirmation, and
 * triggers a listing read.
 */
test("an unconfirmed delete sends the precondition and shows the supervisor's refusal", async ({
  page,
  request,
}) => {
  const session = await createSession(request, { title: "delete-precondition" });
  try {
    await stopSession(request, session.id);
    // Settled server-side before the page looks: the stop returns when the
    // kill is issued, not when "exited" reaches the helm's cache.
    await expect
      .poll(
        async () => (await listSessions(request)).sessions.find((listed) => listed.id === session.id)?.status?.state,
        { timeout: 20_000 },
      )
      .toBe("exited");
    const build = (await request.get("/api/hosts")).headers()["x-farhelm-build"];
    expect(build, "the helm stamps every reply with its build").toBeTruthy();

    await observeFeedReaders(page);
    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    const row = page.locator(`.session-row[data-session-id="${session.id}"]`);

    // Premise: the row itself shows the agent ended (and the session never
    // opened a tab), which is what makes the next Delete the unconfirmed
    // kind. Without this, a row still drawn as live would open the
    // confirmation and the assertions below would fail for the wrong reason.
    await expect(row.locator(".status-badge")).toHaveText(/exited/, { timeout: 20_000 });
    // Setup reads (the handshake's listing, the selected session's detail)
    // must retire before the baseline below, or one of them could be counted
    // as the refusal's re-read.
    await waitForFeedReadersSettled(page);
    await openRowMenu(row);

    const deleteUrls: string[] = [];
    const matchesSession = (url: URL) => url.pathname === `/api/sessions/${session.id}`;
    const refuse = async (route: Route) => {
      if (route.request().method() !== "DELETE") {
        await route.fallback();
        return;
      }
      deleteUrls.push(route.request().url());
      await route.fulfill({
        status: 409,
        headers: { "content-type": "text/plain", "x-farhelm-build": build },
        body: REFUSAL,
      });
    };
    await page.route(matchesSession, refuse);
    const reads = countReads(page);
    const listingReadsBefore = reads.count("listing");

    await row.locator(".session-row-delete").click();

    await expect(row.locator(".action-error")).toContainText(`delete: ${REFUSAL}`);
    expect(deleteUrls).toHaveLength(1);
    expect(new URL(deleteUrls[0]).searchParams.get("only_if_nothing_alive")).toBe("true");
    await expect(row).toBeVisible();
    await expect(row.locator(".confirm-delete")).toHaveCount(0);
    await expect(page.locator(".build-skew")).toHaveCount(0);
    await expect
      .poll(() => reads.count("listing"), { message: "a refused delete must re-read the listing" })
      .toBeGreaterThan(listingReadsBefore);

    await page.unroute(matchesSession, refuse);
  } finally {
    await cleanupSession(request, session.id);
  }
});
