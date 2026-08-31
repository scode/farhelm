/**
 * The session list's ORDER: the sidebar's sort control, the preference it
 * remembers per client, and what re-sorting does not disturb.
 *
 * Its own spec file per the convention every area has followed since M6.5
 * (see sidebar.spec.ts's header). The subject is close to filters.spec.ts's
 * and deliberately separate from it: an order and a filter are different
 * dimensions of one request — a filter decides which sessions a listing
 * holds, an order decides in what sequence they arrive — and the control
 * under test here is specifically the one OUTSIDE the filter bar.
 *
 * ## What these tests are actually checking
 *
 * That the order is the HELM's answer rather than a rearrangement of rows
 * the page already had. Every listing read is inspected for the `sort`
 * parameter it carried, because a page that sorted its own rows would show
 * the same screen against a fixture that fits in one page — and would be
 * wrong the moment a walk is cut short, since the rows past the cut are the
 * ones a client-side sort never sees.
 *
 * That claim is only as strong as the fixture behind it, so one test forces
 * the helm to cut its pages at one row and walks a real multipage listing:
 * the order has to travel on the first request AND on every cursor
 * continuation, and the rendered rows have to be the pages concatenated in
 * the order they arrived. A page that re-sorted locally would survive every
 * single-page test in this file and fail that one.
 *
 * ## Why the default is pinned by the request rather than by observed activity
 *
 * The default order is "recently active", and a fixture that made activity
 * order differ VISIBLY from creation order is not practical here: the
 * supervisor quantizes `last_activity_at` to a minute
 * (`ACTIVITY_STAMP_QUANTUM` in crates/farhelm-supervisor/src/service/ticker.rs),
 * so a session's stamp cannot move above a newer session's until at least a
 * minute after its own creation — longer than this suite's per-test timeout,
 * and a sleep nobody should pay on every run. Sessions that have produced no
 * output carry their creation time as their activity stamp, so the two orders
 * coincide for anything this suite can build.
 *
 * The default is therefore pinned in two halves that together say the same
 * thing: every read asks for `sort=activity` (so the helm, not the page, is
 * doing the ordering, and the UI is not leaning on the helm's own `created`
 * default), and switching to `title` visibly reorders the rows (so the
 * parameter is not decorative). The fixture's titles are chosen so those two
 * orders are exact reverses of each other, which is the strongest contrast a
 * three-row fixture can carry.
 */
import { APIRequestContext, expect, Page, Route, test } from "@playwright/test";
import {
  cleanupSession,
  createSession,
  FeedStub,
  forgetAutoSelect,
  openFilterBar,
  SESSION_LISTING,
  stubFeed,
} from "./helpers/fleet";

/** The row for one session id, as the list renders it. */
function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

/**
 * Load the list with a stubbed, healthy feed.
 *
 * Stubbed for filters.spec.ts's reason: the shared stack's other sessions
 * keep changing status, every change is a revision bump, and each bump
 * re-reads the list — under assertions about the ORDER of rows, at moments
 * no test chose. A silent feed makes the list change exactly when a test
 * asks it to.
 *
 * Deliberately does NOT open the filter bar. The sort control's whole
 * placement decision is that it is reachable without opening anything, and a
 * helper that opened the bar on the way in would hide a regression that put
 * the control back inside it.
 *
 * The stub is handed back, unlike filters.spec.ts's: the persistence test
 * reloads the page, and a reload opens a second feed socket against the SAME
 * route (routes outlive navigations), so it needs the same handle to know the
 * new socket has arrived.
 */
async function listWithStubbedFeed(page: Page): Promise<FeedStub> {
  const feed = await stubFeed(page);
  await page.goto("/");
  await feed.waitForConnection(1);
  feed.notify(1);
  await expect(page.locator(".sort-select")).toBeVisible({ timeout: 20_000 });
  return feed;
}

/**
 * Record the `sort` parameter of every listing read the page makes from now
 * on.
 *
 * Only the parameter, unlike filters.spec.ts's reads: what is under test is
 * which ORDER was asked for, and the rows that came back are read off the
 * rendered list rather than the wire — the page's own display order is the
 * claim, and a body assertion would prove the helm sorted without proving
 * the page kept it.
 */
async function watchSortParameters(page: Page): Promise<string[]> {
  const asked: string[] = [];
  await page.route(SESSION_LISTING, async (route: Route) => {
    if (route.request().method() === "GET") {
      asked.push(new URL(route.request().url()).searchParams.get("sort") ?? "<absent>");
    }
    await route.continue();
  });
  return asked;
}

async function watchListingReads(page: Page): Promise<URL[]> {
  const reads: URL[] = [];
  await page.route(SESSION_LISTING, async (route: Route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    reads.push(new URL(route.request().url()));
    await route.continue();
  });
  return reads;
}

/**
 * The most recent listing request URL in `reads`.
 *
 * The array holds every read the page has made since the watcher went in —
 * the mount read and whatever the test provoked afterwards — and the last
 * one is what an assertion about "the read this test caused" is about.
 */
function latestRead(reads: URL[]): URL {
  const read = reads.at(-1);
  if (read === undefined) throw new Error("no listing read has been recorded yet");
  return read;
}

/** Every listed session id, in the order the sidebar renders them. */
async function renderedOrder(page: Page): Promise<string[]> {
  return await page.locator(".session-row").evaluateAll((rows) =>
    rows.map((element) => element.getAttribute("data-session-id") ?? "")
  );
}

/**
 * The positions of `ids` within the rendered list, in rendering order.
 *
 * Relative positions rather than the whole list, because the stack is shared:
 * other specs' sessions are listed too, and they sort in among the fixture's
 * rows wherever their own titles and creation times put them. What a test can
 * assert is the order of ITS OWN rows relative to each other, which is
 * exactly what an order change has to move.
 */
async function orderOf(page: Page, ids: string[]): Promise<string[]> {
  return (await renderedOrder(page)).filter((id) => ids.includes(id));
}

/**
 * The build stamp the live helm is sending.
 *
 * Needed by every FABRICATED reply below. The UI reads a build stamp off
 * every reply and treats its absence as a version mismatch — a helm that
 * sends none predates the stamp — so an unstamped fixture is not standing in
 * for the helm, it is playing an incompatible one, which latches the skew
 * banner and withdraws the feed-driven reads these tests depend on. Read off
 * the live helm rather than hardcoded so a version bump cannot silently turn
 * every fixture here into a mismatch.
 *
 * The terminal spec family gets this from its own `resetStack`; this file
 * deliberately does not reset the stack (nothing here needs the shared
 * session's scrollback), so it takes the stamp directly.
 */
async function helmStamp(request: APIRequestContext): Promise<string> {
  const probe = await request.get("/api/sessions");
  const stamp = probe.headers()["x-farhelm-build"] ?? "";
  expect(stamp, "the helm must stamp its replies; every fixture below borrows this value")
    .toBeTruthy();
  return stamp;
}

/** Fulfil one intercepted request the way the helm would: stamped. See {@link helmStamp}. */
async function fulfillAsHelm(
  route: Route,
  stamp: string,
  options: { status: number; contentType: string; body: string },
) {
  const { contentType, ...rest } = options;
  await route.fulfill({
    ...rest,
    headers: { "x-farhelm-build": stamp, "content-type": contentType },
  });
}

/**
 * One synthetic session row, in the shape the helm's listing serves.
 *
 * `created_at` is spelled out on every row because it is the field the
 * auto-select fallback picks by — a fixture that omitted it would decode to
 * zero, which the UI reads as "this helm is too old to say" and answers with
 * a different rule entirely.
 */
function syntheticRow(id: string, title: string, createdAt: number) {
  return {
    id,
    title,
    cwd: "/tmp",
    invocation: "true",
    created_at: createdAt,
    archived: false,
    status: { state: "exited", exit_code: 0 },
    annotation: null,
  };
}

test.describe("session list ordering", () => {
  const created: string[] = [];

  test.afterEach(async ({ request }) => {
    while (created.length) {
      const id = created.pop();
      if (id) await cleanupSession(request, id);
    }
  });

  /**
   * Three sessions whose creation order is the exact reverse of their title
   * order, so either ordering is visibly wrong for the other.
   *
   * `sleep 300` rather than the fake agent: it produces no output at all, so
   * every row's activity stamp stays its creation time for the life of the
   * test and the activity order cannot drift under an assertion mid-run (see
   * this file's header on the quantum). The waits between creates are what
   * make the creation order deterministic in the first place — `created_at`
   * has one-second granularity and the helm tiebreaks equal stamps by
   * session id, which is a UUID.
   */
  async function threeOrderedSessions(
    request: APIRequestContext,
    stamp: number,
  ): Promise<{ a: string; m: string; z: string }> {
    const first = await createSession(request, {
      title: `sortfix-${stamp}-aaa`,
      cwd: "/tmp",
      invocation: "sleep 300",
    });
    created.push(first.id);
    await new Promise((resolve) => setTimeout(resolve, 1_100));
    const second = await createSession(request, {
      title: `sortfix-${stamp}-mmm`,
      cwd: "/tmp",
      invocation: "sleep 300",
    });
    created.push(second.id);
    await new Promise((resolve) => setTimeout(resolve, 1_100));
    const third = await createSession(request, {
      title: `sortfix-${stamp}-zzz`,
      cwd: "/tmp",
      invocation: "sleep 300",
    });
    created.push(third.id);
    return { a: first.id, m: second.id, z: third.id };
  }

  /**
   * A client that has chosen nothing lists by activity, and says so to the
   * helm on every read.
   *
   * The parameter is the assertion that matters. The helm answers a request
   * naming no order with `created` — the order every client written before
   * there was a choice still gets — so a UI that simply sent nothing would
   * pass any row-order check this fixture can make (activity and creation
   * order coincide for sessions with no output) while showing creation order
   * under a control that reads "recently active". The rows are checked too,
   * for the half a parameter cannot prove: that what arrived is what is on
   * screen.
   */
  test("an unchosen client lists by activity and asks the helm for it", async ({
    page,
    request,
  }) => {
    const stamp = Date.now();
    const ids = await threeOrderedSessions(request, stamp);

    const asked = await watchSortParameters(page);
    await listWithStubbedFeed(page);
    await expect(row(page, ids.a)).toBeVisible({ timeout: 20_000 });

    expect(asked.length, "loading the list must read it").toBeGreaterThan(0);
    for (const sort of asked) {
      expect(sort, "every listing read must name the order it wants").toBe("activity");
    }
    await expect(page.locator(".sort-select")).toHaveValue("activity");
    // The control is reachable with the filter bar shut, which is the whole
    // reason it does not live inside it.
    await expect(page.locator(".session-filter")).toHaveCount(0);

    // Newest first, and for these rows that is both the activity order and
    // the creation order (see the fixture's docstring).
    expect(await orderOf(page, [ids.a, ids.m, ids.z])).toEqual([ids.z, ids.m, ids.a]);
  });

  /**
   * Choosing "title A–Z" reorders the rows alphabetically, at the helm.
   *
   * The fixture's title order is the exact reverse of its creation order, so
   * this cannot pass by accident: the three rows have to physically swap ends
   * of the list. The request is inspected alongside, because the rows alone
   * would look identical if the page had sorted the single page it was
   * holding — which is the implementation this test exists to reject, since
   * it silently misorders anything past a page cut.
   */
  test("switching to title order reorders the rows and re-reads the list", async ({
    page,
    request,
  }) => {
    const stamp = Date.now();
    const ids = await threeOrderedSessions(request, stamp);

    await listWithStubbedFeed(page);
    await expect(row(page, ids.a)).toBeVisible({ timeout: 20_000 });
    expect(await orderOf(page, [ids.a, ids.m, ids.z])).toEqual([ids.z, ids.m, ids.a]);

    // Installed after the initial load, so what it records is the re-sorted
    // walk and nothing else.
    const asked = await watchSortParameters(page);
    await page.locator(".sort-select").selectOption("title");

    await expect
      .poll(() => orderOf(page, [ids.a, ids.m, ids.z]), { timeout: 20_000 })
      .toEqual([ids.a, ids.m, ids.z]);
    expect(asked.length, "changing the order must re-read the list").toBeGreaterThan(0);
    for (const sort of asked) {
      expect(sort, "and every read after the change must carry the new order").toBe("title");
    }
  });

  /**
   * The choice survives a reload, because it is stored per client rather than
   * held in the page.
   *
   * A preference that had to be re-picked on every load would be worse than
   * none: the list is the first thing a client draws, and re-drawing it in an
   * order the user has already rejected once is the exact annoyance the
   * control exists to end. The stored value is asserted directly as well as
   * through its effect — the effect alone would also pass if the page had
   * simply not reloaded, and the key is a contract the desktop build will
   * have to honor when its own persistence lands.
   */
  test("the chosen order survives a reload", async ({ page, request }) => {
    const stamp = Date.now();
    const ids = await threeOrderedSessions(request, stamp);

    const feed = await listWithStubbedFeed(page);
    await expect(row(page, ids.a)).toBeVisible({ timeout: 20_000 });
    await page.locator(".sort-select").selectOption("title");
    await expect
      .poll(() => orderOf(page, [ids.a, ids.m, ids.z]), { timeout: 20_000 })
      .toEqual([ids.a, ids.m, ids.z]);
    expect(await page.evaluate(() => window.localStorage.getItem("farhelm.sort"))).toBe("title");

    const asked = await watchSortParameters(page);
    await page.reload();
    await feed.waitForConnection(2);
    feed.notify(2);

    await expect(page.locator(".sort-select")).toHaveValue("title", { timeout: 20_000 });
    await expect
      .poll(() => orderOf(page, [ids.a, ids.m, ids.z]), { timeout: 20_000 })
      .toEqual([ids.a, ids.m, ids.z]);
    expect(asked.length, "the reloaded page must read the list").toBeGreaterThan(0);
    for (const sort of asked) {
      expect(sort, "a reloaded client must ask for the order it remembered").toBe("title");
    }
  });

  /**
   * Re-sorting is not deselecting: the selected session stays selected, and
   * its row stays marked wherever the new order puts it.
   *
   * The same principle SPEC.md states for filtering, and the failure it
   * guards against is worse here, because the row does not even leave the
   * list — a re-sort that dropped the selection would swap the main pane's
   * session for whichever row the fallback auto-select then picked, in
   * response to nothing more than a change of viewing order.
   *
   * The selected row is deliberately the one the two orders move FURTHEST:
   * last under activity, first under title. A selection reconciled by
   * position rather than by id would survive a row that barely moved.
   */
  test("re-sorting keeps the selected session selected and marked", async ({ page, request }) => {
    const stamp = Date.now();
    const ids = await threeOrderedSessions(request, stamp);

    await listWithStubbedFeed(page);
    await expect(row(page, ids.a)).toBeVisible({ timeout: 20_000 });
    await row(page, ids.a).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toContainText(`sortfix-${stamp}-aaa`);
    await expect(row(page, ids.a)).toHaveAttribute("data-session-selected", "true");

    await page.locator(".sort-select").selectOption("title");
    await expect
      .poll(() => orderOf(page, [ids.a, ids.m, ids.z]), { timeout: 20_000 })
      .toEqual([ids.a, ids.m, ids.z]);

    await expect(page.locator(".titlebar .title")).toContainText(`sortfix-${stamp}-aaa`);
    await expect(row(page, ids.a)).toHaveAttribute("data-session-selected", "true");
    await expect(row(page, ids.a)).toHaveClass(/(^| )selected( |$)/);
    await expect(row(page, ids.z)).toHaveAttribute("data-session-selected", "false");
  });

  /**
   * The control is a labelled `combobox` offering exactly the three orders,
   * each with the word the helm takes and the words a person reads.
   *
   * Located by ROLE and accessible name rather than by `.sort-select`,
   * because the class is an implementation detail while the role and the
   * name are the contract a keyboard or screen-reader user actually meets —
   * a control that lost its label would still match the class selector every
   * other test in this file uses, and nothing would notice.
   *
   * The pairs are asserted exactly. A value the helm does not know is a
   * listing that 400s rather than one that sorts oddly, and a label that
   * drifted from its value ("newest created" wired to `activity`) would be a
   * control that lies about what it does while every order-of-rows assertion
   * still passes.
   */
  test("the sort control is a labelled combobox offering the three wire orders", async ({ page }) => {
    await listWithStubbedFeed(page);

    const control = page.getByRole("combobox", { name: "sort" });
    await expect(control).toBeVisible();

    const offered = await control.locator("option").evaluateAll((options) =>
      options.map((option) => [
        (option as HTMLOptionElement).value,
        (option.textContent ?? "").trim(),
      ])
    );
    expect(offered).toEqual([
      ["activity", "recently active"],
      ["created", "newest created"],
      ["title", "title A–Z"],
    ]);
  });

  /**
   * A client that has expressed no preference writes nothing, and choosing
   * the order already in force is not a choice.
   *
   * Both halves are about the same promise: the preference is written on
   * CHANGE, so a client that never touches the control never touches its
   * storage. The stored key is a contract the desktop build will have to
   * honor when its own persistence lands, and a UI that wrote the default on
   * load would make "has this user chosen?" unanswerable from that day on —
   * every client would look like one that picked activity deliberately, and
   * a future change of default could never reach them.
   *
   * The no-op half also has to hold at the LIST: re-selecting the active
   * option restarts nothing, because a walk restart under a cursor-paginated
   * list is a real cost and there is nothing to re-ask for.
   */
  test("the default order is never written, and re-choosing it re-reads nothing", async ({
    page,
    request,
  }) => {
    const stamp = Date.now();
    const ids = await threeOrderedSessions(request, stamp);

    const asked = await watchSortParameters(page);
    await listWithStubbedFeed(page);
    await expect(row(page, ids.a)).toBeVisible({ timeout: 20_000 });

    expect(
      await page.evaluate(() => window.localStorage.getItem("farhelm.sort")),
      "a client that has chosen nothing has nothing to remember",
    ).toBeNull();

    const before = asked.length;
    await page.locator(".sort-select").selectOption("activity");
    // A settle window rather than an assertion that resolves as soon as it
    // is true: what is being proved is that nothing happens, and nothing
    // happening is only observable by waiting long enough for it to have.
    await page.waitForTimeout(1_500);
    expect(asked.length, "re-choosing the active order must not restart the walk").toBe(before);
    expect(
      await page.evaluate(() => window.localStorage.getItem("farhelm.sort")),
      "and must not write a preference either",
    ).toBeNull();
  });

  /**
   * Order and filter are independent dimensions of one request, and stay
   * that way across a whole session of using both.
   *
   * One flow rather than four tests because the failures worth catching are
   * about the INTERACTION: a re-sort that dropped the applied filter would
   * silently widen the list under a filter badge still claiming to be in
   * force, and a filter apply that dropped the order would answer the user's
   * search in a sequence their control does not name. Both look like a
   * working list until someone reads the numbers.
   *
   * Clearing the filter at the end is the other half of the split: "clear"
   * undoes a narrowing, and the order is not one — a client that lost its
   * chosen order to a filter reset would have to re-pick it every time it
   * finished searching.
   */
  test("an order and a filter survive each other, and clearing the filter keeps the order", async ({
    page,
    request,
  }) => {
    const stamp = Date.now();
    const ids = await threeOrderedSessions(request, stamp);
    const search = `sortfix-${stamp}-`;

    const reads = await watchListingReads(page);
    await listWithStubbedFeed(page);
    await expect(row(page, ids.a)).toBeVisible({ timeout: 20_000 });

    await page.locator(".sort-select").selectOption("title");
    await expect
      .poll(() => orderOf(page, [ids.a, ids.m, ids.z]), { timeout: 20_000 })
      .toEqual([ids.a, ids.m, ids.z]);

    await openFilterBar(page);
    await page.locator(".filter-title").fill(search);
    await page.locator(".filter-apply").click();
    await expect(page.locator(".session-count")).toHaveText(/^3 matching of \d+ sessions$/);
    await expect(page.locator(".session-row")).toHaveCount(3);
    expect(await orderOf(page, [ids.a, ids.m, ids.z])).toEqual([ids.a, ids.m, ids.z]);
    const filtered = latestRead(reads);
    expect(filtered.searchParams.get("sort")).toBe("title");
    expect(filtered.searchParams.get("title")).toBe(search);

    // Re-sorting WHILE filtered: the request has to carry both, and the
    // membership and the banner must not move — only the sequence does.
    await page.locator(".sort-select").selectOption("created");
    await expect
      .poll(() => orderOf(page, [ids.a, ids.m, ids.z]), { timeout: 20_000 })
      .toEqual([ids.z, ids.m, ids.a]);
    await expect(page.locator(".session-count")).toHaveText(/^3 matching of \d+ sessions$/);
    await expect(page.locator(".session-row")).toHaveCount(3);
    await expect(page.locator(".filter-active-note")).toHaveCount(1);
    const resorted = latestRead(reads);
    expect(resorted.searchParams.get("sort")).toBe("created");
    expect(
      resorted.searchParams.get("title"),
      "changing the order must not clear the filter the list is under",
    ).toBe(search);

    // Clearing the filter widens the list and leaves the order alone.
    await page.locator(".filter-clear").click();
    await expect(page.locator(".session-count")).toHaveText(/^\d+ sessions$/);
    await expect(page.locator(".sort-select")).toHaveValue("created");
    await expect
      .poll(() => orderOf(page, [ids.a, ids.m, ids.z]), { timeout: 20_000 })
      .toEqual([ids.z, ids.m, ids.a]);
    const cleared = latestRead(reads);
    expect(cleared.searchParams.get("sort")).toBe("created");
    expect(cleared.searchParams.has("title")).toBe(false);
  });

  /**
   * A stored order this build cannot use is the default, not a broken page.
   *
   * The value survives across builds and is editable by anyone with a
   * devtools console, and the helm answers an unrecognized `sort` with a
   * 400 — so passing one through unchecked would not sort the list oddly, it
   * would leave the sidebar reading "failed to load sessions" until someone
   * cleared their browser storage. Staged with the word a LATER build might
   * plausibly have written, which is the case that will actually happen.
   */
  test("a stored order this build does not know falls back to the default", async ({
    page,
    request,
  }) => {
    const stamp = Date.now();
    const ids = await threeOrderedSessions(request, stamp);

    await page.addInitScript(() => {
      window.localStorage.setItem("farhelm.sort", "most-recent");
    });
    const asked = await watchSortParameters(page);
    await listWithStubbedFeed(page);
    await expect(row(page, ids.a)).toBeVisible({ timeout: 20_000 });

    await expect(page.locator(".sort-select")).toHaveValue("activity");
    expect(asked.length, "loading the list must read it").toBeGreaterThan(0);
    for (const sort of asked) {
      expect(sort, "an unusable preference must not reach the helm").toBe("activity");
    }
  });

  /**
   * A storage that REFUSES to be read costs the preference, not the list.
   *
   * Reading localStorage throws for real reasons the user did not choose —
   * a browser configured to block site data, a private window under some
   * policies — and the whole point of keeping the preference outside the
   * page's own state is that losing it is a small thing. A page that let the
   * exception escape would fail to draw a session list because of a feature
   * that only decides what order the rows are in.
   *
   * The stub throws for THIS key only, so the failure under test is the sort
   * preference's own read rather than a page-wide storage outage that would
   * take the remembered selection down with it and prove something else.
   */
  test("a storage that refuses to be read still lists, in the default order", async ({
    page,
    request,
  }) => {
    const stamp = Date.now();
    const ids = await threeOrderedSessions(request, stamp);

    await page.addInitScript(() => {
      const real = Storage.prototype.getItem;
      Storage.prototype.getItem = function(key: string) {
        if (key === "farhelm.sort") throw new DOMException("blocked", "SecurityError");
        return real.call(this, key);
      };
    });
    const asked = await watchSortParameters(page);
    await listWithStubbedFeed(page);
    await expect(row(page, ids.a)).toBeVisible({ timeout: 20_000 });

    await expect(page.locator(".sort-select")).toHaveValue("activity");
    for (const sort of asked) {
      expect(sort, "an unreadable preference reads as no preference").toBe("activity");
    }
  });

  /**
   * A storage that refuses to be WRITTEN costs the next load, not this one.
   *
   * The asymmetry is the contract: a failed write means the choice is
   * forgotten by the next visit, and it must not mean the choice was refused
   * now. A quota-exceeded `setItem` is the realistic shape of this (storage
   * fills up for reasons that have nothing to do with this page), and a page
   * that let it escape would leave the user staring at a control that says
   * "title A–Z" over rows in activity order.
   */
  test("a storage that refuses to be written still re-sorts the page", async ({ page, request }) => {
    const stamp = Date.now();
    const ids = await threeOrderedSessions(request, stamp);

    await page.addInitScript(() => {
      const real = Storage.prototype.setItem;
      Storage.prototype.setItem = function(key: string, value: string) {
        if (key === "farhelm.sort") throw new DOMException("full", "QuotaExceededError");
        return real.call(this, key, value);
      };
    });
    const asked = await watchSortParameters(page);
    await listWithStubbedFeed(page);
    await expect(row(page, ids.a)).toBeVisible({ timeout: 20_000 });

    const before = asked.length;
    await page.locator(".sort-select").selectOption("title");
    await expect
      .poll(() => orderOf(page, [ids.a, ids.m, ids.z]), { timeout: 20_000 })
      .toEqual([ids.a, ids.m, ids.z]);
    await expect(page.locator(".sort-select")).toHaveValue("title");
    for (const sort of asked.slice(before)) {
      expect(sort, "the read that follows the choice must carry the new order").toBe("title");
    }
    expect(
      await page.evaluate(() => window.localStorage.getItem("farhelm.sort")),
      "the write failed, so there is nothing stored — that is the whole cost",
    ).toBeNull();
  });

  /**
   * A listing walked under the PREVIOUS order is refused, even when it is
   * the newer successful read.
   *
   * The generation gate cannot catch this one. The stale read started first
   * and is the newest SUCCESS the page has, so every ordering rule the gate
   * knows says to apply it; the only thing standing between it and the
   * screen is the check that the reply answers the order currently applied.
   * Applied, it would paint a correctly-ordered list of the wrong sequence
   * under a control naming another one — and, because the read that WOULD
   * have corrected it is the one being held here, it would stay that way.
   *
   * Staged around the surface reader's single-flight rule, which decides the
   * order of events: while the activity read is in flight the re-sort can
   * only record demand, so the stale reply necessarily lands BEFORE the
   * title read is dispatched. Holding the title read open after that is what
   * makes the assertion decidable — with nothing else able to touch the
   * surface, a stale reply that was wrongly admitted would still be on
   * screen rather than overwritten a moment later by the correct one.
   */
  test("a reply answering the previous order is refused after the order changes", async ({
    page,
    request,
  }) => {
    const stamp = Date.now();
    const ids = await threeOrderedSessions(request, stamp);
    const helm = await helmStamp(request);
    const staleId = "stale-order-listing-row";

    const feed = await listWithStubbedFeed(page);
    await expect(row(page, ids.a)).toBeVisible({ timeout: 20_000 });

    let activityReads = 0;
    let titleReads = 0;
    let releaseActivity: () => void = () => {};
    let releaseTitle: () => void = () => {};
    const activityHeld = new Promise<void>((resolve) => {
      releaseActivity = resolve;
    });
    const titleHeld = new Promise<void>((resolve) => {
      releaseTitle = resolve;
    });
    await page.route(SESSION_LISTING, async (route: Route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      const sort = new URL(route.request().url()).searchParams.get("sort");
      if (sort === "activity") {
        activityReads += 1;
        await activityHeld;
        // A listing nothing else could produce, so "was it applied?" is a
        // question about one row rather than about a diff of the fleet.
        await fulfillAsHelm(route, helm, {
          status: 200,
          contentType: "application/json",
          body: JSON.stringify({
            sessions: [syntheticRow(staleId, "stale-order-listing", 1)],
            total: 1,
            matching: 1,
            truncated: false,
          }),
        });
        return;
      }
      titleReads += 1;
      await titleHeld;
      await fulfillAsHelm(route, helm, {
        status: 500,
        contentType: "text/plain",
        body: "injected read failure",
      });
    });

    // Put an activity-order read in flight and leave it there.
    feed.notify(2);
    await expect.poll(() => activityReads, { timeout: 20_000 }).toBeGreaterThan(0);

    await page.locator(".sort-select").selectOption("title");
    await expect(page.locator(".sort-select")).toHaveValue("title");

    releaseActivity();
    // The title read being dispatched proves the stale reply has been
    // through `commit_listing` already: the reader starts the follow-up only
    // once the read it was holding has finished.
    await expect.poll(() => titleReads, { timeout: 20_000 }).toBeGreaterThan(0);

    await expect(
      row(page, staleId),
      "a listing walked under the previous order must never reach the screen",
    ).toHaveCount(0);
    await expect(row(page, ids.a)).toBeVisible();

    // And the read that IS on topic decides the surface: releasing it as a
    // failure replaces the rows with the error, which is the state the stale
    // success was competing to prevent.
    releaseTitle();
    await expect(page.locator(".status.error")).toBeVisible({ timeout: 20_000 });
    await expect(row(page, staleId)).toHaveCount(0);
  });

  /**
   * The three rows the auto-select fallback tests are staged against, in
   * TITLE order with the newest-created one last.
   *
   * Synthetic rather than created for real, because what is being staged is
   * a walk that stopped early — a state the helm reaches at a 500-row
   * ceiling and that no fixture this suite could build would otherwise
   * produce. The stamps are chosen so that every wrong rule picks a
   * different row: reading position picks "aaa", reading the truncated
   * prefix's newest picks "mmm", and only reading the fleet's newest picks
   * "zzz".
   */
  const FALLBACK_ROWS = {
    aaa: syntheticRow("sortfallback-aaa", "sortfallback-aaa", 1_000),
    mmm: syntheticRow("sortfallback-mmm", "sortfallback-mmm", 1_100),
    zzz: syntheticRow("sortfallback-zzz", "sortfallback-zzz", 1_200),
  };

  /**
   * Serve the listing synthetically, in title order, and record any
   * creation-order request the page makes on the side.
   *
   * The paged design's auto-select fallback used to ask the helm for the
   * newest session with a one-row creation-order request when the listing
   * was incomplete; that request is gone, and the recorded list is how the
   * test below proves it stays gone.
   *
   * Also states the precondition the whole fallback rests on: NOTHING
   * remembered. SPEC.md's fallback is what a client with no remembered
   * selection opens, and a remembered id skips it — under a truncated walk
   * the sidebar resolves that id with the helm and opens whatever it names,
   * which is a real session rather than one of the rows below. The suite's
   * shared `storageState` is supposed to hold only the device secret, but it
   * is rewritten mid-run by auth.spec.ts and once carried a selection out of
   * that rewrite, which failed a test on this fixture on both engines in
   * both full runs of the suite. Cheap to state here, and it makes the test
   * say what it needs rather than inherit it.
   */
  async function stubFallbackListing(page: Page, helm: string, cut: boolean): Promise<URL[]> {
    await forgetAutoSelect(page);
    const { aaa, mmm, zzz } = FALLBACK_ROWS;
    const newestReads: URL[] = [];
    await page.route(SESSION_LISTING, async (route: Route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      const url = new URL(route.request().url());
      if (url.searchParams.get("sort") === "created") {
        newestReads.push(url);
        await fulfillAsHelm(route, helm, {
          status: 200,
          contentType: "application/json",
          body: JSON.stringify({ sessions: [zzz], total: 3, matching: 3, truncated: false }),
        });
        return;
      }
      await fulfillAsHelm(route, helm, {
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(
          cut
            // The cap's shape: the title order put "zzz" past the cut, so
            // the newest-created row is NOT in the reply.
            ? { sessions: [aaa, mmm], total: 3, matching: 3, truncated: true }
            : { sessions: [aaa, mmm, zzz], total: 3, matching: 3, truncated: false },
        ),
      });
    });
    return newestReads;
  }

  /**
   * The auto-select fallback picks the newest-created session out of the
   * rows in hand, under a non-creation order, with no extra request.
   *
   * SPEC.md's fallback is "the newest-created non-archived session" for a
   * client with no remembered selection. The rows arrive in title order, so
   * the newest is not the first row — it has to be picked by `created_at`
   * — and the whole-list reply is the whole list, so nothing needs asking
   * twice. The negative half is what the recorded creation-order requests
   * pin: the paged design's one-row fallback request must not come back.
   */
  test("the fallback picks the newest session from the rows in hand without asking again", async ({
    page,
    request,
  }) => {
    const helm = await helmStamp(request);
    const newestReads = await stubFallbackListing(page, helm, false);
    await listWithStubbedFeed(page);
    await expect(page.locator(".titlebar .title")).toHaveText("sortfallback-zzz", {
      timeout: 20_000,
    });
    // Settled before the negative is claimed: the extra read, if it were
    // going to happen, would follow the same listing commit that produced
    // the row above.
    await page.waitForTimeout(1_500);
    expect(
      newestReads,
      "the rows in hand contain the newest row; nothing extra may be asked",
    ).toEqual([]);
  });

  /**
   * A CUT title-ordered listing still picks from the rows in hand and
   * still asks nothing extra — the leg that actually distinguishes the new
   * behavior from the paged design's.
   *
   * The old implementation only issued its one-row creation-order request
   * when the listing was incomplete, so a complete-listing fixture would
   * stay green with that gate and request still in place. Here the reply
   * is truncated and the fleet's true newest ("zzz") is past the cut: the
   * accepted behavior (SPEC.md's Session list section) is to open the
   * newest row the reply DID reach ("mmm") rather than ask the helm — the
   * recorded creation-order requests pin that nothing is asked.
   */
  test("a cut listing picks the newest row in hand and still asks nothing", async ({
    page,
    request,
  }) => {
    const helm = await helmStamp(request);
    const newestReads = await stubFallbackListing(page, helm, true);
    await listWithStubbedFeed(page);
    await expect(page.locator(".titlebar .title")).toHaveText("sortfallback-mmm", {
      timeout: 20_000,
    });
    await page.waitForTimeout(1_500);
    expect(
      newestReads,
      "a cut list is not a reason to ask the helm; the pick is from the rows in hand",
    ).toEqual([]);
  });
});
