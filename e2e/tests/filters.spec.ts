// The session list's filter and search surface (PLAN_M6_75.md item 7):
// SPEC.md's five dimensions, answered server-side, with the count banner
// honest about filter-versus-truncation.
//
// A per-area spec of its own, per this milestone's convention (see
// sidebar.spec.ts's header). Its helpers are shared with feed.spec.ts and
// m6-5-debts.spec.ts — see helpers/fleet.ts for why these three share a
// module rather than duplicating one.
//
// ## What these tests are actually checking
//
// Not "does typing narrow the rows" — a client-side filter would pass that
// just as well, and would be wrong for a reason no single-page fixture can
// show: it hides matches beyond the page cut while the banner reports a
// count that includes them. What is checked instead is that the FILTER
// REACHES THE HELM and the reply's two counts reach the banner, which is
// what makes "N matching of M sessions" a claim about the fleet rather than
// about whatever this page happens to hold.
import { expect, Page, Route, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import {
  cleanupSession,
  countReads,
  createSession,
  FAKE_AGENT,
  listHosts,
  listSessions,
  localHostId,
  openFilterBar,
  SessionPage,
  stopSession,
  stubFeed,
} from "./helpers/fleet";
import { stackScratchDir } from "./helpers/scratch";

/** The row for one session id, as the list renders it. */
function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

/**
 * Load the list with a stubbed, healthy feed.
 *
 * Stubbed for the same reason feed.spec.ts stubs: the shared stack's other
 * sessions keep changing status, and each change is a revision bump that
 * would re-read — and therefore re-apply — the filter under the assertions
 * below at unpredictable moments. A silent feed makes each test's DOM change
 * exactly when the test asks for it.
 *
 * The stub itself is deliberately NOT handed back: every test here drives the
 * list through its own controls, so a returned handle would only invite a
 * notification that re-reads underneath an assertion — which is the very
 * thing stubbing is here to prevent.
 */
async function listWithStubbedFeed(page: Page): Promise<void> {
  const feed = await stubFeed(page);
  await page.goto("/");
  await feed.waitForConnection(1);
  feed.notify(1);
  // The filter bar sits behind the sidebar's on-demand toggle now; every
  // test in this suite drives it, so opening it is part of arriving. The
  // helper's own visibility wait doubles as the page-arrived assertion
  // this line used to be.
  await openFilterBar(page);
}

/**
 * One listing read, as the PAGE asked for it and as the helm answered.
 *
 * The pair is the whole point: the URL says what was asked of the server, and
 * the body says what the server sent back. Rows and a banner alone cannot
 * tell a server-side filter from a client-side pass over an exhausted
 * fixture, and these two can — a request carrying `title=` that comes back
 * carrying only matches is a narrowing that happened on the far side of the
 * wire.
 */
interface ListingRead {
  /** What the page asked for, recorded BEFORE this fixture touches it. */
  url: URL;
  /** What came back — the same bytes the page then decoded. */
  body: SessionPage;
}

/**
 * Record every listing read the page makes from now on, optionally forcing
 * the helm to cut its pages at `limit` rows.
 *
 * The read is fetched here and re-fulfilled from that same reply, so a
 * recorded body is by construction the one the page received rather than a
 * second look at a list that may have moved. Re-fulfilling from the
 * `APIResponse` also keeps the helm's headers — the build stamp above all,
 * without which the page latches skew and stops reading altogether.
 *
 * `limit` is what makes a page cut reachable at all: the helm's default page
 * is 500 rows and this UI never asks for fewer, so no honest fixture this
 * suite can build would ever produce a second page. Appending the parameter
 * on the way past leaves the page's OWN parameters — the ones under test —
 * untouched and recorded as they were.
 */
async function watchListingReads(page: Page, limit?: number): Promise<ListingRead[]> {
  const reads: ListingRead[] = [];
  await page.route(
    (url) => url.pathname === "/api/sessions",
    async (route: Route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      const asked = new URL(route.request().url());
      const sent = new URL(asked);
      if (limit !== undefined) sent.searchParams.set("limit", String(limit));
      const response = await route.fetch({ url: sent.toString() });
      reads.push({ url: asked, body: (await response.json()) as SessionPage });
      await route.fulfill({ response });
    },
  );
  return reads;
}

/** Fill the search box and apply. Submitting, not typing: the filter is a
 * query, and this UI deliberately sends one per submit rather than one per
 * keystroke (see `list::ListView`). */
async function applyFilter(
  page: Page,
  fields: { title?: string; directory?: string; profile?: string; status?: string; host?: number },
) {
  if (fields.title !== undefined) await page.locator(".filter-title").fill(fields.title);
  if (fields.directory !== undefined) await page.locator(".filter-directory").fill(fields.directory);
  if (fields.profile !== undefined) await page.locator(".filter-profile").fill(fields.profile);
  if (fields.status !== undefined) await page.locator(".filter-status").selectOption(fields.status);
  if (fields.host !== undefined) {
    await page.locator(".filter-host").selectOption(String(fields.host));
  }
  await page.locator(".filter-apply").click();
}

test.describe("session list filtering", () => {
  const created: string[] = [];
  const directories: string[] = [];
  // Profiles created by a test, so a FAILURE cannot leave one behind. The
  // profile test deletes its own as part of what it is proving, but a test
  // that fails between creating and deleting would leak a profile into a
  // fleet every later run shares — and this suite's own convention is that
  // the stack a test finds is the stack the next one finds.
  const profiles: { host: number; id: string }[] = [];

  test.afterEach(async ({ request }) => {
    while (created.length) {
      const id = created.pop();
      if (id) await cleanupSession(request, id);
    }
    while (profiles.length) {
      const profile = profiles.pop();
      if (!profile) continue;
      const response = await request.delete(`/api/hosts/${profile.host}/profiles/${profile.id}`);
      // Already deleted is the NORMAL outcome here, not an error: the one
      // test that makes a profile deletes it on the way through.
      if (!response.ok() && response.status() !== 404) {
        throw new Error(
          `cleanup: deleting profile ${profile.id} failed (${response.status()}): ${await response
            .text()}`,
        );
      }
    }
    while (directories.length) {
      const dir = directories.pop();
      if (dir) fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  /**
   * Searching by title narrows the list AND the banner reports both counts —
   * with the narrowing shown to have happened at the HELM.
   *
   * The banner is the half that cannot be faked by a client-side filter: the
   * matching count is the helm's answer over the whole merged view, so it is
   * a number this page could not have computed from the rows it holds.
   * Asserted as a pattern rather than an exact sentence because the fleet
   * total belongs to a shared stack — what must be exact is the SHAPE, "N
   * matching of M sessions", and that the N is the one this filter produced.
   *
   * Rows and banner together still leave one reading open: a client filtering
   * the page it was handed would produce exactly the same screen against a
   * fixture small enough to fit in one page, which every fixture in this file
   * is. So the request and the reply are inspected as well — the query string
   * carries the search, and what comes back is ALREADY narrowed. A page doing
   * its own filtering would be handed the whole fleet.
   */
  test("searching by title narrows the rows and reports N matching of M", async ({
    page,
    request,
  }) => {
    const stamp = Date.now();
    const wanted = await createSession(request, { title: `needle-${stamp}` });
    const other = await createSession(request, { title: `haystack-${stamp}` });
    created.push(wanted.id, other.id);

    await listWithStubbedFeed(page);
    await expect(row(page, wanted.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, other.id)).toBeVisible();

    // Installed after the initial load, so what it records is the filtered
    // walk and nothing else.
    const reads = await watchListingReads(page);
    await applyFilter(page, { title: `needle-${stamp}` });

    await expect(page.locator(".session-row")).toHaveCount(1, { timeout: 20_000 });
    await expect(row(page, wanted.id)).toBeVisible();
    await expect(page.locator(".session-count")).toHaveText(/^1 matching of \d+ sessions$/);

    const filtered = [...reads];
    expect(filtered.length, "applying a filter must re-read the list").toBeGreaterThan(0);
    for (const read of filtered) {
      expect(read.url.searchParams.get("title"), `${read.url} must carry the search`).toBe(
        `needle-${stamp}`,
      );
      expect(
        read.body.sessions.map((session) => session.title),
        "the helm must answer a filtered request with matches only; anything else means this " +
          "page was handed the fleet and narrowed it itself",
      ).toEqual([`needle-${stamp}`]);
    }

    // Clearing removes the explicit title search and returns to the default
    // view. That view still excludes archived sessions, so it is still a
    // filter and the banner keeps both numbers: matching rows and the whole
    // fleet. The wire distinguishes a cleared title from a search for the
    // empty string — no `title` parameter versus `?title=`.
    await page.locator(".filter-clear").click();
    await expect(row(page, other.id)).toBeVisible({ timeout: 20_000 });
    await expect(page.locator(".session-count")).toHaveText(/^\d+ matching of \d+ sessions$/);
    const cleared = reads.slice(filtered.length);
    expect(cleared.length, "clearing the filter must re-read the list").toBeGreaterThan(0);
    for (const read of cleared) {
      expect(read.url.searchParams.has("title"), `${read.url} must clear the title search`).toBe(
        false,
      );
    }
  });

  /**
   * The page cut: a filter that matches more sessions than fit on one page
   * is applied by the HELM, before the cut, and travels on every cursor page
   * of the walk.
   *
   * This is the case the single-page tests structurally cannot reach, and the
   * one where a client-side filter stops being merely a different
   * implementation and becomes wrong: it hides matches beyond the cut while
   * the banner reports a count that includes them. Two rows per page against
   * three matches is the smallest fixture that has a cut in it at all.
   *
   * The cursor half (`ListQuery::cursor` in the helm, `fetch_sessions` in the
   * UI) is the other thing only a cut can show: a cursor names a position in
   * the helm's ORDER rather than in a result set, so a second page requested
   * without the filter would quietly start listing rows the first page had
   * excluded. Unit tests cover the encoding of that query string; nothing but
   * a walk in a browser covers it actually being sent on page two.
   *
   * Deliberately NOT exercised here: a banner whose `matching` exceeds the
   * rows on screen. It takes a TRUNCATED walk, and this UI walks the cursor
   * to exhaustion unless one of four ceilings stops it — pages, rows, bytes,
   * and (since the surface gained a single reader) elapsed time. Only the
   * first two need a fleet this suite could not build; the byte and time
   * ceilings can cut a walk of very few rows, given fat rows or a slow helm,
   * and a route fixture could produce either. That is a test worth having and
   * a different one from this: its subject is the BANNER's honesty about a
   * walk that stopped short, while this one's is the filter reaching the helm
   * and travelling on the cursor.
   */
  test("a filter is applied before the page cut and travels on every cursor page", async ({
    page,
    request,
  }) => {
    const stamp = Date.now();
    const needle = `cut-needle-${stamp}`;
    const matches = [];
    for (const nth of [0, 1, 2]) {
      const session = await createSession(request, { title: `${needle}-${nth}` });
      created.push(session.id);
      matches.push(session);
    }
    const other = await createSession(request, { title: `cut-haystack-${stamp}` });
    created.push(other.id);

    await listWithStubbedFeed(page);
    await expect(row(page, matches[0].id)).toBeVisible({ timeout: 20_000 });

    const reads = await watchListingReads(page, 2);
    await applyFilter(page, { title: needle });

    // Every match is on screen, including the one that cannot be on the
    // first page — the user-visible consequence of filtering before the cut.
    await expect(page.locator(".session-row")).toHaveCount(3, { timeout: 20_000 });
    for (const match of matches) await expect(row(page, match.id)).toBeVisible();
    await expect(row(page, other.id)).toHaveCount(0);
    await expect(page.locator(".session-count")).toHaveText(/^3 matching of \d+ sessions$/);

    expect(
      reads.length,
      `three matches at two rows a page must take more than one request; saw ${
        reads.map((read) => read.url.href).join(", ")
      }`,
    ).toBeGreaterThan(1);
    for (const read of reads) {
      expect(read.url.searchParams.get("title"), `${read.url} must carry the filter`).toBe(needle);
      expect(
        read.body.sessions.every((session) => session.title.startsWith(needle)),
        `every page must come back already narrowed; ${read.url} carried ${
          read.body.sessions.map((session) => session.title).join(", ")
        }`,
      ).toBe(true);
      expect(
        read.body.sessions.length,
        "and no page may exceed the cut, or there was no cut to test",
      ).toBeLessThanOrEqual(2);
    }
    const cursored = reads.filter((read) => read.url.searchParams.has("cursor"));
    expect(cursored.length, "a cut list is walked by cursor").toBeGreaterThan(0);
    for (const read of cursored) {
      expect(
        read.url.searchParams.get("title"),
        "a cursor page that dropped the filter would resume listing rows the first page " +
          `excluded; ${read.url}`,
      ).toBe(needle);
    }
  });

  /**
   * The working-directory dimension, over a directory that exists on the
   * supervisor's own machine.
   *
   * A freshly made temp directory rather than a shared one, because the
   * assertion is an exact row count: `/tmp` is where every other fixture in
   * this suite puts its sessions, so filtering on it would count whatever
   * else happens to be alive.
   */
  test("filtering by working directory finds only sessions launched there", async ({
    page,
    request,
  }) => {
    const dir = stackScratchDir("fh-filter-");
    directories.push(dir);
    const here = await createSession(request, { title: `dir-here-${Date.now()}`, cwd: dir });
    const elsewhere = await createSession(request, { title: `dir-elsewhere-${Date.now()}` });
    created.push(here.id, elsewhere.id);

    await listWithStubbedFeed(page);
    await expect(row(page, here.id)).toBeVisible({ timeout: 20_000 });

    await applyFilter(page, { directory: path.basename(dir) });

    await expect(page.locator(".session-row")).toHaveCount(1, { timeout: 20_000 });
    await expect(row(page, here.id)).toBeVisible();
    await expect(row(page, elsewhere.id)).toHaveCount(0);
  });

  /**
   * The status dimension, driven by a stop rather than by waiting for the
   * sampler.
   *
   * A stop produces `exited` and, unlike every live status, it STAYS there:
   * the running/waiting/idle words move on the supervisor's sampling
   * schedule, so an assertion about one of them is an assertion about
   * timing, while `exited` is terminal and can be waited for once. That is
   * what makes this a test of the FILTER rather than of classification. The
   * waiting is still real — the stop's transition lands on the supervisor's
   * schedule too, not on the HTTP reply's — which is what the poll below is
   * for.
   */
  test("filtering by status separates an ended session from a live one", async ({
    page,
    request,
  }) => {
    const stamp = Date.now();
    const stopped = await createSession(request, { title: `status-stopped-${stamp}` });
    const alive = await createSession(request, { title: `status-alive-${stamp}` });
    created.push(stopped.id, alive.id);
    await stopSession(request, stopped.id);
    // The stop's status transition lands on the supervisor's schedule, not
    // the HTTP reply's, and a stubbed silent feed means the page never
    // re-reads on its own — so the fixture must be fully "exited" before
    // the page takes its one look, or the filter honestly finds nothing.
    await expect
      .poll(async () => {
        const listed = await listSessions(request, `title=${encodeURIComponent(`status-stopped-${stamp}`)}`);
        return listed.sessions[0]?.status?.state;
      }, { timeout: 20_000 })
      .toBe("exited");

    await listWithStubbedFeed(page);
    await expect(row(page, stopped.id)).toBeVisible({ timeout: 20_000 });

    // Narrowed by title as well, so the row count is about these two
    // sessions rather than about everything else the shared stack has ended.
    await applyFilter(page, { title: `status-`, status: "exited" });
    await expect(row(page, stopped.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, alive.id)).toHaveCount(0);

    // And the complement: the ended session is absent from every live
    // status, which is what stops a status filter from being decorative.
    await applyFilter(page, { status: "running" });
    await expect(row(page, stopped.id)).toHaveCount(0, { timeout: 20_000 });
  });

  /**
   * The host dimension, and the empty-result wording.
   *
   * The stack registers a second host that carries no sessions, which makes
   * it the one place in this suite where a legitimate filter matches
   * NOTHING — and that is worth pinning, because a count over an empty box
   * reads as a list that failed to load rather than as a search that found
   * nothing, and the two call for opposite reactions from the user.
   */
  test("filtering by host can match nothing, and says so in words", async ({ page, request }) => {
    const session = await createSession(request, { title: `host-filter-${Date.now()}` });
    created.push(session.id);
    const hosts = await listHosts(request);
    const local = await localHostId(request);
    const remote = hosts.find((host) => host.id !== local);
    expect(remote, "the e2e stack registers a second host; without it this test proves nothing")
      .toBeTruthy();

    await listWithStubbedFeed(page);
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });

    await applyFilter(page, { host: local });
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });

    await applyFilter(page, { host: remote!.id });
    await expect(page.locator(".session-row")).toHaveCount(0, { timeout: 20_000 });
    await expect(page.locator(".filter-empty")).toBeVisible();
    await expect(page.locator(".session-count")).toHaveText(/^0 matching of \d+ sessions$/);
  });

  /**
   * A host that leaves the registry while its filter is applied becomes a
   * TOMBSTONE, and the filter goes on asking for it.
   *
   * The failure this rules out is the quiet one, and it is quiet in a
   * specific direction: with the removed id gone from the option list, a
   * `select` falls back to displaying its first option — "any host" — while
   * the applied filter keeps sending the dead id with every read. The control
   * then says "everything" while the request says "that one machine" and the
   * rows agree with neither, which is the worst of the three possible states
   * because it is the one a user would not think to question.
   *
   * The alternative fix — clearing the filter when its host disappears —
   * looks tidier and is worse: it silently WIDENS a query someone chose, and
   * a list that quietly starts showing other machines' sessions is exactly
   * what a host filter exists to prevent. So the rule is that the id stays on
   * the wire until a person changes it, and the control says so in words.
   *
   * The registry is edited on the way past rather than for real: removing the
   * harness's second host would take the fleet down for every later test, and
   * what is under test here is entirely this page's reaction to a registry
   * that no longer lists something.
   */
  test("a host removed from the registry becomes a tombstone the filter still names", async ({
    page,
    request,
  }) => {
    const session = await createSession(request, { title: `host-tombstone-${Date.now()}` });
    created.push(session.id);
    const hosts = await listHosts(request);
    const local = await localHostId(request);
    const remote = hosts.find((host) => host.id !== local);
    expect(remote, "the e2e stack registers a second host; without it this test proves nothing")
      .toBeTruthy();
    const removedId = remote!.id;

    let removed = false;
    await page.route(
      (url) => url.pathname === "/api/hosts",
      async (route: Route) => {
        if (route.request().method() !== "GET") {
          await route.continue();
          return;
        }
        const response = await route.fetch();
        const body = await response.json();
        if (removed) {
          body.hosts = body.hosts.filter((host: { id: number }) => host.id !== removedId);
        }
        await route.fulfill({ response, json: body });
      },
    );

    // Its own setup rather than the shared helper, because this test needs
    // the stub's handle: the registry edit below is only READ when something
    // asks, and on a healthy feed the only thing that asks is a notification.
    const feed = await stubFeed(page);
    const reads = countReads(page);
    await page.goto("/");
    await openFilterBar(page);
    await feed.waitForConnection(1);
    feed.notify(1);
    await expect(page.locator(".session-filter")).toBeVisible({ timeout: 20_000 });
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });

    await applyFilter(page, { host: removedId });
    await expect(page.locator(".filter-host")).toHaveValue(String(removedId), { timeout: 20_000 });

    // The host leaves the registry, and the page finds out the way it finds
    // out about anything: a revision notification and its own re-read.
    removed = true;
    const before = reads.count("listing");
    feed.notify(2);

    const tombstone = page.locator(`.filter-host option[value="${removedId}"]`);
    await expect(tombstone).toHaveCount(1, { timeout: 20_000 });
    // The DOM property, not toBeDisabled(): Playwright's disabled-state
    // matcher does not honor a plain [disabled] attribute on an <option>
    // (it reports the option enabled even as the browser refuses to select
    // it), and the property is what the browser actually consults.
    await expect(tombstone).toHaveJSProperty("disabled", true);
    await expect(tombstone).toContainText("no longer registered");
    await expect(
      page.locator(".filter-host"),
      "the control must keep naming the host the request still carries",
    ).toHaveValue(String(removedId));

    // And the wire agrees. Every listing read since the removal still asks
    // for that host — a page that quietly widened to the whole fleet would
    // have dropped the parameter here.
    await expect
      .poll(() => reads.count("listing"), {
        timeout: 20_000,
        message: "the notification must have produced a listing read to inspect",
      })
      .toBeGreaterThan(before);
    const asked = reads.urls("listing").slice(before);
    expect(
      asked.every((url) => new URL(url).searchParams.get("host") === String(removedId)),
      `a removed host is still the filter until a person changes it; saw ${asked.join(", ")}`,
    ).toBe(true);
  });

  /**
   * A DELETED profile's sessions stay findable under the name they
   * snapshotted at creation.
   *
   * This is the dimension whose whole point is the snapshot rule
   * (PLAN_M6_75.md item 3): nothing rewrites a historical session when its
   * profile goes away, so the only way those sessions remain reachable is a
   * filter that matches the snapshotted name as well as the id. A picker
   * built from the live catalog could not even offer this search, which is
   * why the UI's profile field is free text.
   *
   * The profile is created for this test rather than using a starter,
   * because a starter names a real agent binary this suite does not require
   * to be installed — and because deleting a starter would leave the shared
   * stack short one for every later run.
   */
  test("a deleted profile's sessions are still findable by its snapshotted name", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const profileName = `e2e-profile-${Date.now()}`;
    const created_profile = await request.post(`/api/hosts/${local}/profiles`, {
      data: { name: profileName, invocation: FAKE_AGENT, agent_kind: "generic" },
    });
    expect(created_profile.ok(), await created_profile.text()).toBeTruthy();
    const profile = await created_profile.json();
    // Registered for cleanup BEFORE anything else can fail: this test deletes
    // the profile itself further down, but a failure in between would
    // otherwise leave it in a fleet every later run shares.
    profiles.push({ host: local, id: profile.id });

    const fromProfile = await createSession(request, {
      title: `profile-session-${Date.now()}`,
      profile_id: profile.id,
    });
    created.push(fromProfile.id);

    // The profile goes away; the session does not, and neither does its
    // snapshot.
    const deleted = await request.delete(`/api/hosts/${local}/profiles/${profile.id}`);
    expect(deleted.ok(), await deleted.text()).toBeTruthy();

    await listWithStubbedFeed(page);
    await expect(row(page, fromProfile.id)).toBeVisible({ timeout: 20_000 });

    await applyFilter(page, { profile: profileName });
    await expect(page.locator(".session-row")).toHaveCount(1, { timeout: 20_000 });
    await expect(row(page, fromProfile.id)).toBeVisible();
    await expect(page.locator(".session-count")).toHaveText(/^1 matching of \d+ sessions$/);
  });
});
