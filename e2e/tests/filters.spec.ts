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
import { expect, test } from "./helpers/evidence";
import { Page, Route } from "@playwright/test";
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
  // The host list's first read landing is itself a layout change above the
  // session header, and the popover is a fixed surface measured once against
  // that header, so `ListView` closes it for exactly that change
  // (`hosts_list_shape`). Opening before the list has settled therefore
  // races the page's own dismissal; settle first, then open.
  await expect(page.locator(".host-row").first()).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".hosts-status", { hasText: "loading hosts" })).toHaveCount(0, {
    timeout: 20_000,
  });
  // The filter popover sits behind the session header's on-demand toggle; every
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
 * Record every listing read the page makes from now on.
 *
 * The read is fetched here and re-fulfilled from that same reply, so a
 * recorded body is by construction the one the page received rather than a
 * second look at a list that may have moved. Re-fulfilling from the
 * `APIResponse` also keeps the helm's headers — the build stamp above all,
 * without which the page latches skew and stops reading altogether.
 */
async function watchListingReads(page: Page): Promise<ListingRead[]> {
  const reads: ListingRead[] = [];
  await page.route(
    (url) => url.pathname === "/api/sessions",
    async (route: Route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      const asked = new URL(route.request().url());
      const response = await route.fetch();
      reads.push({ url: asked, body: (await response.json()) as SessionPage });
      await route.fulfill({ response });
    },
  );
  return reads;
}

/** Fill live filter fields and wait until the list is in a filtered state.
 *
 * The helper waits for the count's filtered wording rather than the debounce
 * interval: tests should observe user-visible state, not renderer timing. It
 * does not identify a particular reply once another filter is already active,
 * so callers must wait for the rows or banner their own query requires.
 */
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
  await expect(page.locator(".session-count")).toContainText("matching", { timeout: 20_000 });
}

test.describe("session list filtering", () => {
  const created: string[] = [];
  const directories: string[] = [];
  // Profiles created by a test, so a FAILURE cannot leave one behind. The
  // profile test deletes its own as part of what it is proving, but a test
  // that fails between creating and deleting would leak a profile into a
  // fleet every later run shares — and this suite's own convention is that
  // the stack a test finds is the stack the next one finds.
  const profiles: string[] = [];

  test.afterEach(async ({ request }) => {
    while (created.length) {
      const id = created.pop();
      if (id) await cleanupSession(request, id);
    }
    while (profiles.length) {
      const profile = profiles.pop();
      if (!profile) continue;
      const response = await request.delete(`/api/profiles/${profile}`);
      // Already deleted is the NORMAL outcome here, not an error: the one
      // test that makes a profile deletes it on the way through.
      if (!response.ok() && response.status() !== 404) {
        throw new Error(
          `cleanup: deleting profile ${profile} failed (${response.status()}): ${await response
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
   * Focus moving inside the popover must not be mistaken for a dismissal.
   *
   * The desktop bridge supplies no `relatedTarget`, so this covers the
   * document-active-element fallback across both keyboard and pointer moves.
   */
  test("moving focus among filter controls keeps the popover mounted", async ({ page }) => {
    await listWithStubbedFeed(page);

    await page.locator(".filter-host").press("Tab");
    await expect(page.locator(".filter-popover")).toBeVisible();
    await page.locator(".filter-parent").click();
    await expect(page.locator(".filter-popover")).toBeVisible();
    await page.locator(".filter-title").fill(`focus-${Date.now()}`);
    await page.locator(".filter-clear").click();
    await expect(page.locator(".filter-popover")).toBeVisible();
  });

  /**
   * The fixed surface follows its header toggle without leaving the viewport.
   *
   * Ordinary and narrow widths cover the two sides of the responsive clamp;
   * checking every edge prevents a placement that only looks anchored while
   * part of the controls are unreachable off-screen.
   */
  test("the filter popover stays on screen at ordinary and narrow widths", async ({ page }) => {
    for (const width of [1280, 360]) {
      await page.setViewportSize({ width, height: 720 });
      await listWithStubbedFeed(page);
      const toggle = await page.locator(".filter-toggle").boundingBox();
      const popover = await page.locator(".filter-popover").boundingBox();
      expect(toggle).not.toBeNull();
      expect(popover).not.toBeNull();
      expect(popover!.x).toBeGreaterThanOrEqual(0);
      expect(popover!.y).toBeGreaterThanOrEqual(toggle!.y + toggle!.height - 1);
      expect(popover!.x + popover!.width).toBeLessThanOrEqual(width);
      expect(popover!.y + popover!.height).toBeLessThanOrEqual(720);
      await page.locator(".filter-toggle").click();
    }
  });

  /**
   * Text edits share one delayed listing read, while a completed choice
   * retires that delay before issuing its own read. This observes requests,
   * not only the final DOM, because duplicate reads can render the same rows.
   */
  test("filter debounce coalesces text and yields to discrete changes", async ({ page }) => {
    await listWithStubbedFeed(page);
    const reads = await watchListingReads(page);

    await page.locator(".filter-title").pressSequentially("debounce");
    await page.waitForTimeout(500);
    expect(reads).toHaveLength(1);
    expect(reads[0].url.searchParams.get("title")).toBe("debounce");

    reads.splice(0);
    // Both edits land in ONE browser task, so the text debounce cannot fire
    // between them. Two separate Playwright actions leave a round trip's gap
    // on a slow engine, and a debounce that fires in that gap is the page
    // behaving correctly for a user who paused, not the retirement under test.
    await page.locator(".filter-popover").evaluate((popover) => {
      const title = popover.querySelector<HTMLInputElement>(".filter-title")!;
      title.value = "retire-delay";
      title.dispatchEvent(new Event("input", { bubbles: true }));
      popover.querySelector<HTMLInputElement>(".filter-include-archived")!.click();
    });
    await page.waitForTimeout(500);
    expect(reads).toHaveLength(1);
    expect(reads[0].url.searchParams.get("title")).toBe("retire-delay");
    expect(reads[0].url.searchParams.get("include_archived")).toBe("true");
  });

  /**
   * A live result changes the count row above the toggle, but that ordinary
   * header reflow must remeasure the popover rather than dismiss the edit.
   */
  test("a matching-count update keeps the filter popover open", async ({ page }) => {
    await listWithStubbedFeed(page);
    await page.locator(".filter-title").fill(`count-remeasure-${Date.now()}`);
    await expect(page.locator(".session-count")).toContainText("matching", { timeout: 20_000 });
    await expect(page.locator(".filter-popover")).toBeVisible();
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
    // view, which reports itself as unfiltered: it still withholds archived
    // sessions, but its count withholds them too, so there are no longer two
    // numbers to reconcile and no filter a person applied to announce (see
    // archive.spec.ts for the denominator itself). The wire distinguishes a
    // cleared title from a search for the empty string — no `title`
    // parameter versus `?title=`.
    await page.locator(".filter-clear").click();
    await expect(row(page, other.id)).toBeVisible({ timeout: 20_000 });
    await expect(page.locator(".session-count")).toHaveText(/^\d+ sessions$/);
    const cleared = reads.slice(filtered.length);
    expect(cleared.length, "clearing the filter must re-read the list").toBeGreaterThan(0);
    for (const read of cleared) {
      expect(read.url.searchParams.has("title"), `${read.url} must clear the title search`).toBe(
        false,
      );
    }
  });

  /**
 * Pins the live-popover lifecycle around a pending text debounce.
 *
 * Escape must return focus to the toggle without cancelling a request the
 * user already made; pointer and Tab dismissal instead preserve the outside
 * destination. Those paths are deliberately distinct because only Escape is
 * an instruction to return to the surface's invoker.
   */
  test("title filtering stays applied after Escape and clear restores the list", async ({
    page,
    request,
  }) => {
    const stamp = Date.now();
    const wanted = await createSession(request, { title: `live-needle-${stamp}` });
    const other = await createSession(request, { title: `live-haystack-${stamp}` });
    created.push(wanted.id, other.id);

    await listWithStubbedFeed(page);
    await expect(row(page, wanted.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, other.id)).toBeVisible();

    await page.locator(".filter-title").fill(`live-needle-${stamp}`);
    await expect(page.locator(".filter-apply")).toHaveCount(0);
    await expect(page.locator(".filter-active-note")).toHaveCount(0);
    await page.locator(".filter-title").press("Escape");
    await expect(page.locator(".filter-popover")).toHaveCount(0);
    await expect(page.locator(".filter-toggle")).toBeFocused();
    await expect(page.locator(".filter-active-note")).toHaveCount(0);
    await expect(row(page, wanted.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, other.id)).toHaveCount(0);
    await expect(page.locator(".session-count")).toHaveText(/^1 matching of \d+ sessions$/);

    await openFilterBar(page);
    // Tab chooses the next outside control, now compact in the session
    // heading. The popover must close behind it rather than stealing focus
    // back to its toggle or withdrawing the query.
    await page.locator(".filter-clear").press("Tab");
    await expect(page.locator(".filter-popover")).toHaveCount(0);
    await expect(page.locator(".compact-toggle input")).toBeFocused();
    await expect(page.locator(".session-count")).toHaveText(/^1 matching of \d+ sessions$/);

    await openFilterBar(page);
    // Clicking inert list chrome also closes the surface, but has no focus
    // destination for the popover to redirect.
    await page.locator(".session-count").click();
    await expect(page.locator(".filter-popover")).toHaveCount(0);
    await expect(page.locator(".session-count")).toHaveText(/^1 matching of \d+ sessions$/);

    await openFilterBar(page);
    await page.locator(".filter-clear").click();
    await expect(row(page, other.id)).toBeVisible({ timeout: 20_000 });
    await expect(page.locator(".session-count")).toHaveText(/^\d+ sessions$/);
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
    await expect(page.locator(".filter-popover")).toBeVisible({ timeout: 20_000 });
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });

    await applyFilter(page, { host: removedId });
    await expect(page.locator(".filter-host")).toHaveValue(String(removedId), { timeout: 20_000 });

    // The host leaves the registry, and the page finds out the way it finds
    // out about anything: a revision notification and its own re-read.
    removed = true;
    const before = reads.count("listing");
    feed.notify(2);

    // A host leaving the registry shrinks the strip above the session header,
    // which is a layout change the page answers by closing the fixed popover
    // (its measured anchor moved). Reopen it to read the select; `openFilterBar`
    // is idempotent, so a page that had not closed it yet is fine too.
    await expect
      .poll(async () => reads.count("listing"), { timeout: 20_000 })
      .toBeGreaterThan(before);
    await openFilterBar(page);

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
    const created_profile = await request.post("/api/profiles", {
      data: { name: profileName, invocation: FAKE_AGENT, agent_kind: "generic" },
    });
    expect(created_profile.ok(), await created_profile.text()).toBeTruthy();
    const profile = await created_profile.json();
    // Registered for cleanup BEFORE anything else can fail: this test deletes
    // the profile itself further down, but a failure in between would
    // otherwise leave it in a fleet every later run shares.
    profiles.push(profile.id);

    const fromProfile = await createSession(request, {
      title: `profile-session-${Date.now()}`,
      profile_id: profile.id,
    });
    created.push(fromProfile.id);

    // The profile goes away; the session does not, and neither does its
    // snapshot.
    const deleted = await request.delete(`/api/profiles/${profile.id}`);
    expect(deleted.ok(), await deleted.text()).toBeTruthy();

    await listWithStubbedFeed(page);
    await expect(row(page, fromProfile.id)).toBeVisible({ timeout: 20_000 });

    await applyFilter(page, { profile: profileName });
    await expect(page.locator(".session-row")).toHaveCount(1, { timeout: 20_000 });
    await expect(row(page, fromProfile.id)).toBeVisible();
    await expect(page.locator(".session-count")).toHaveText(/^1 matching of \d+ sessions$/);
  });
});
