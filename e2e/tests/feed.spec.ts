// The invalidation feed's contract in a real browser (PLAN_M6_75.md item
// 6): the channel that replaced all four of the UI's periodic loops, and the
// three ways it can be wrong.
//
// A dedicated per-area file, per this milestone's own testing convention
// (see sidebar.spec.ts's header for why new coverage starts its own spec
// rather than growing terminal.spec.ts). The helpers it shares with
// filters.spec.ts and m6-5-debts.spec.ts live in helpers/fleet.ts — see that
// file for why these three share a module while the older specs duplicate
// theirs.
//
// ## Why most of these tests stub the socket
//
// The e2e stack is one shared fleet, and a real feed carries whatever it
// happens to be doing — another spec's leftover session settling into `idle`
// bumps the revision, and a helm restart would reconnect a client on its
// own. Against that, "a healthy feed performs no periodic reads" could only
// ever be observed as "no reads happened to occur during this window", which
// is a weaker claim than the one the milestone makes. Replacing the socket
// with one this test drives makes each assertion about the CLIENT's rules:
// when it re-reads, when it falls back, and when it stands down.
//
// The two tests that do NOT stub are the ones whose whole subject is the
// real path end to end: the two-client test and the status transition.
//
// ## Four loops, four surfaces
//
// The milestone removed FOUR periodic reads — the list's listing walk and
// its host registry read, and the session view's detail read and the host
// registry read behind a stale session's notice — and only the MOUNTED page
// can read anything. So the no-polling proof is three tests rather than one:
// the list, an ordinary session view, and a stale session view, each with
// its own observation window. A single test on the list would leave half the
// claim resting on code it never mounted. `countReads` classifies by
// endpoint for the same reason.
import { expect, Page, Route, test } from "@playwright/test";
import {
  cleanupSession,
  countReads,
  createSession,
  FeedStub,
  forceBuildSkew,
  listSessions,
  openFilterBar,
  openRowMenu,
  renameSession,
  stubFeed,
} from "./helpers/fleet";

/** The row for one session id, as the list renders it. */
function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

/**
 * Hand the page `revision` however it can be reached: on the socket it has
 * open right now, and on every socket it opens from here on.
 *
 * The helm's own handshake, and the only form of it a test may rely on after
 * an outage. A stub socket is never greeted while the outage is being staged,
 * and the client tears an ungreeted subscription down and climbs its ladder —
 * so a bare `notify` is a bet that the socket seen a moment ago is still
 * there, and losing that bet is not a flake but a crash ("no feed socket is
 * open to notify on"). Arming first and sending second covers both worlds:
 * whichever socket exists gets the notice, and if the one that gets it is a
 * later arrival the page is greeted the instant it subscribes, exactly as a
 * real helm would.
 *
 * A doubled notice would be harmless — a repeated revision is still a
 * re-read, and reads coalesce (`reader`) — but does not arise here: a greeted
 * subscription is healthy and stops being replaced.
 *
 * The handshake-attribution test below deliberately does NOT call this: it
 * needs the arming and the sending on opposite sides of its listing gate, and
 * says so where it splits them.
 */
function handshake(feed: FeedStub, revision: number) {
  feed.notifyOnConnect(revision);
  if (feed.openSockets() > 0) feed.notify(revision);
}

/** Open one session's view from its row, and wait until it is really on
 * screen — the titlebar's title, because it is the one element that only
 * appears once the view has a session to describe. */
async function openSession(page: Page, id: string, title: string) {
  await row(page, id).locator(".session-row-open").click();
  await expect(page.locator(".titlebar .title")).toHaveText(title, { timeout: 20_000 });
}

/**
 * A switchable STALENESS gate over the page's listing reads: while it is
 * frozen, every one of them is answered with a reply the helm really gave,
 * from before the test changed anything.
 *
 * ## Why stale-but-successful rather than failed
 *
 * The obvious way to keep a mutation off the screen is to kill the reads that
 * could carry it. It is also useless for ATTRIBUTION: a read that never
 * answers leaves the surface dirty and its reader retrying on its own
 * schedule (`reader`), so the moment reads are allowed through again, the
 * page has a read of its own coming that owes nothing to any notification.
 * The one successful read that follows then proves nothing about what asked
 * for it.
 *
 * A read that SUCCEEDS satisfies the reader instead: the surface is clean,
 * the reader is idle, and no demand is standing. That is what makes the
 * notice the only thing that can produce the next read, which is the whole
 * point of the test this exists for.
 *
 * ## One route, a flag, and a capture
 *
 * The route is installed for the whole test and its behavior is a FLAG,
 * rather than a route added and removed around the interesting window:
 * `page.unroute` cannot un-issue a read already in flight, and the read it
 * lets through is exactly the one an attribution argument then has to excuse.
 * A flag can be flipped in the same synchronous statement sequence as the
 * event it is meant to precede, which no route callback can interleave with.
 *
 * The frozen reply is the last one the helm actually gave this page rather
 * than a fabrication, which keeps its headers — the build stamp above all,
 * whose absence would latch skew and stand the page down from reading at all.
 * Freezing BEFORE the test mutates anything is what makes it stale: a capture
 * taken afterwards would carry the very change it is supposed to be hiding.
 *
 * One captured reply stands in for a whole walk, which holds because this
 * suite's fleet fits in one page of the helm's list (the same assumption the
 * read counts elsewhere in this file make); the capture carries no cursor, so
 * a walk answered from it ends after one request.
 */
interface ListingGate {
  /** Answer every listing read from here on with the last reply the helm
   * gave, and reset [`ListingGate::reachedHelm`]. Throws if the page has not
   * read the list yet, since there would be nothing to answer with. */
  freeze(): void;
  /** Let listing reads reach the helm again. */
  thaw(): void;
  /** Resolve once one more read has been answered from the freeze — proof the
   * page really is re-reading while its socket is down, and a
   * synchronisation point that leaves most of a poll interval clear before
   * the next fallback tick. */
  waitForStaleAnswer(): Promise<void>;
  /** How many listing reads have reached the HELM since the freeze. */
  reachedHelm(): number;
}

/** One captured reply, in the shape `route.fulfill` takes. Spelled off
 * `Route` itself rather than written out, so the body's type is Playwright's
 * rather than a node `Buffer` this suite has no types for. */
type FrozenReply = NonNullable<Parameters<Route["fulfill"]>[0]>;

async function gateListingReads(page: Page): Promise<ListingGate> {
  /** The helm's last real reply, kept as the three fields a fulfil needs. */
  let latest: FrozenReply | undefined;
  let frozen: FrozenReply | undefined;
  let staleAnswers = 0;
  let reachedHelm = 0;
  await page.route(
    (url) => url.pathname === "/api/sessions",
    async (route: Route) => {
      // Reads only: a create posts to this same path, and answering a
      // mutation from a capture would be a fixture breaking the test's own
      // setup.
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      if (frozen) {
        staleAnswers += 1;
        await route.fulfill(frozen);
        return;
      }
      // Counted at DISPATCH rather than at completion: a read that left
      // before the freeze belongs to the world before it, and counting it
      // whenever its reply happened to land would put it on the wrong side of
      // the reset.
      reachedHelm += 1;
      const response = await route.fetch();
      latest = {
        status: response.status(),
        headers: response.headers(),
        body: await response.body(),
      };
      await route.fulfill(latest);
    },
  );
  return {
    freeze() {
      if (!latest) throw new Error("nothing has read the list yet, so there is no reply to freeze");
      frozen = latest;
      reachedHelm = 0;
    },
    thaw() {
      frozen = undefined;
    },
    async waitForStaleAnswer() {
      const before = staleAnswers;
      await expect
        .poll(() => staleAnswers, {
          timeout: 20_000,
          message: "the fallback poll must be running while the socket is down",
        })
        .toBeGreaterThan(before);
    },
    reachedHelm: () => reachedHelm,
  };
}

/**
 * Make this session's DETAIL read report a stale row, without touching
 * anything else in it.
 *
 * The stale session view is a surface with reads of its own — the detail read
 * plus the host registry behind the notice that explains the staleness — and
 * it is unreachable against a healthy stack on request: it needs a host that
 * has gone away, which this suite has no way to arrange for a session it can
 * also drive. Rewriting the one flag on the way past is the smallest fixture
 * that produces the real surface.
 *
 * The reply is fetched and re-fulfilled rather than fabricated, which keeps
 * the helm's own headers — the build stamp above all: a fabricated reply
 * carrying no stamp latches skew, and a skewed page stands down from exactly
 * the reads these tests are counting.
 *
 * The host is moved to an id the registry does not hold, and that is not
 * decoration. A stale session whose host reads CONNECTED is a disagreement
 * between two reads, and the view answers it by refreshing the session
 * immediately (`hosts::stale_session_notice`'s connected arm) — an extra
 * read triggered by the fixture rather than by the page's own rules, which
 * is the last thing a test counting reads wants underneath it. An
 * unregistered host is a settled explanation instead.
 */
async function reportStale(page: Page, id: string) {
  await page.route(
    (url) => url.pathname === `/api/sessions/${id}`,
    async (route: Route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      const response = await route.fetch();
      const session = await response.json();
      await route.fulfill({
        response,
        json: { ...session, stale: true, host: 999_999, host_name: "a host that left" },
      });
    },
  );
}

/**
 * Load the list with a stubbed feed already subscribed and healthy, and
 * hand back the pieces the test drives it with.
 *
 * Bundled because every stubbed test needs the same four steps in the same
 * order — stub, count, navigate, handshake — and getting the order wrong
 * (routing after navigation, counting after the first read) fails as a
 * confusing assertion rather than as a setup error.
 */
async function healthyFeed(page: Page) {
  const feed = await stubFeed(page);
  const reads = countReads(page);
  await page.goto("/");
  await feed.waitForConnection(1);
  // The handshake. The helm sends it immediately on every (re)subscription;
  // here the test chooses the moment, which is what the reconnect tests
  // below are built on.
  feed.notify(1);
  await expect(page.locator(".session-list")).toBeVisible();
  return { feed, reads };
}

test.describe("the invalidation feed", () => {
  const created: string[] = [];

  test.afterEach(async ({ request }) => {
    while (created.length) {
      const id = created.pop();
      if (id) await cleanupSession(request, id);
    }
  });

  /**
   * SPEC.md's multi-client sentence, executable: a change made in one client
   * shows up in every other one without a refresh.
   *
   * Driven through the UI in the first context on purpose. An API-driven
   * mutation would prove the feed carries changes, which the tests below
   * already do; what this adds is that a change a USER makes on one screen
   * reaches another screen, which is the promise as a person experiences it.
   *
   * The real socket, not the stub: this is the one test whose subject is the
   * whole path — helm bump, socket, client re-read, DOM.
   */
  test("a rename in one client appears in another without a refresh", async ({
    browser,
    request,
  }) => {
    const session = await createSession(request, { title: `two-client-${Date.now()}` });
    created.push(session.id);

    const first = await browser.newContext();
    const second = await browser.newContext();
    try {
      const author = await first.newPage();
      const observer = await second.newPage();
      await author.goto("/");
      await observer.goto("/");
      await expect(row(author, session.id)).toBeVisible({ timeout: 20_000 });
      await expect(row(observer, session.id).locator(".session-title")).toHaveText(
        session.title,
        { timeout: 20_000 },
      );

      const renamed = `${session.title}-renamed`;
      await openRowMenu(row(author, session.id));
      await row(author, session.id).locator(".session-row-rename").click();
      await row(author, session.id).locator(".rename-input").fill(renamed);
      await row(author, session.id).locator(".rename-submit").click();

      // The observer never reloaded and never polled: the only thing that
      // can put this title on its screen is a revision notification followed
      // by its own re-read.
      await expect(row(observer, session.id).locator(".session-title")).toHaveText(renamed, {
        timeout: 20_000,
      });
    } finally {
      await first.close();
      await second.close();
    }
  });

  /**
   * The no-polling proof, on the LIST: a healthy feed performs no periodic
   * reads of the listing or of the host registry.
   *
   * The two-client test alone cannot distinguish push from a surviving poll
   * — a three-second poll would pass it just as happily — so this observes
   * the requests instead. The window is comfortably longer than three poll
   * intervals, which is what makes zero reads a statement about the loops
   * being GONE rather than about timing.
   *
   * Counted from a mark taken after the page has settled, because the reads
   * that are supposed to happen — one on mount, one per notification — are
   * not polls and would otherwise be indistinguishable from them.
   *
   * This test covers two of the four removed loops, and only two: the other
   * pair belongs to the session view, which is a DIFFERENT mounted page and
   * therefore cannot read anything while the list is up. The two tests below
   * mount it.
   */
  test("a healthy feed performs no periodic reads on the list", async ({ page }) => {
    const { reads } = await healthyFeed(page);
    // Let the mount reads and the handshake's re-read land before the window
    // opens.
    await page.waitForTimeout(1_500);
    const before = reads.count();

    await page.waitForTimeout(12_000);

    expect(
      reads.count() - before,
      `a healthy feed must read nothing on a timer; saw ${reads.urls().slice(before).join(", ")}`,
    ).toBe(0);
  });

  /**
   * The same proof for the SESSION VIEW's detail loop, which the list-side
   * test cannot make.
   *
   * The detail read was its own three-second poll (`session_view`'s M2-era
   * loop), removed by this milestone along with the listing's — and a page
   * showing the list can no more prove that than it can prove anything else
   * about a component it has not mounted. Only the MOUNTED page re-reads, and
   * that property is exactly what makes a separate test necessary rather than
   * pedantic.
   */
  test("a healthy feed performs no periodic reads with a session view open", async ({
    page,
    request,
  }) => {
    const session = await createSession(request, { title: `quiet-detail-${Date.now()}` });
    created.push(session.id);
    const { reads } = await healthyFeed(page);
    await openSession(page, session.id, session.title);
    await page.waitForTimeout(1_500);
    const before = reads.count();
    const detailBefore = reads.count("detail");

    await page.waitForTimeout(12_000);

    // The detail surface first, since a surviving detail loop is what this
    // test exists for and a message about "one read" would leave which
    // surface to guesswork.
    expect(
      reads.count("detail") - detailBefore,
      `the removed detail poll must be gone; saw ${
        reads.urls("detail").slice(detailBefore).join(", ")
      }`,
    ).toBe(0);
    expect(
      reads.count() - before,
      `an open session view must read nothing on a timer; saw ${
        reads.urls().slice(before).join(", ")
      }`,
    ).toBe(0);
  });

  /**
   * And the fourth loop: a STALE session view, whose notice is backed by a
   * host registry read of its own.
   *
   * The stale surface is the only place `/api/hosts` is read from the session
   * view at all, so it is the only place its removed loop can be observed —
   * an ordinary session view would report zero host reads whether the loop
   * existed or not, which is a pass that proves nothing.
   */
  test("a healthy feed performs no periodic reads with a stale session view open", async ({
    page,
    request,
  }) => {
    const session = await createSession(request, { title: `quiet-stale-${Date.now()}` });
    created.push(session.id);
    await reportStale(page, session.id);
    const { reads } = await healthyFeed(page);
    await openSession(page, session.id, session.title);
    // The notice is what says the stale surface is really up: without it the
    // host read below would be absent for the boring reason.
    await expect(page.locator(".host-stale-notice")).toBeVisible({ timeout: 20_000 });
    await page.waitForTimeout(1_500);
    const before = reads.count();
    const hostsBefore = reads.count("hosts");

    await page.waitForTimeout(12_000);

    expect(
      reads.count("hosts") - hostsBefore,
      `the stale notice's host read must not be on a timer; saw ${
        reads.urls("hosts").slice(hostsBefore).join(", ")
      }`,
    ).toBe(0);
    expect(
      reads.count() - before,
      `a stale session view must read nothing on a timer; saw ${
        reads.urls().slice(before).join(", ")
      }`,
    ).toBe(0);
  });

  /**
   * Feed death and recovery: the documented poll fallback covers the outage,
   * and the feed comes back on its own — on BOTH pages.
   *
   * Three claims in sequence, and each is a different half of the rule
   * (`farhelm-ui/src/feed.rs`): a dead socket on a MATCHING build polls, the
   * client reconnects without being asked (the reconnect ladder the terminal
   * islands already climb), and a fresh handshake takes it off the fallback
   * again. The last one is what stops the fallback from being a one-way
   * door.
   *
   * Run twice, once with the list mounted and once with a session view, for
   * the reason the no-polling tests are split the same way: the fallback is
   * per-surface (each page owns its own loop and its own gate), so a
   * list-only run leaves the session view's fallback entirely unobserved —
   * both the half that must poll and the half that must stop.
   *
   * ## Why the handover is counted exactly
   *
   * The recovery half is not "fewer reads afterwards" but "exactly the reads
   * the handshake owes, and nothing else". Marking the counter AFTER the
   * notification would hide the failure worth catching: a page that
   * re-handshakes but leaves its fallback running double-reads for one
   * interval and then settles, which a window opened after the settling
   * cannot see. So the mark is taken BEFORE the notification, and the
   * notification is fired just after a fallback tick has landed — which
   * leaves most of a poll interval clear, so a tick and the handshake's
   * re-read cannot both fall inside the same counted window by accident.
   *
   * The read the greeting produces IS the handshake's re-read, and it is what
   * the counts below expect: one listing walk and one host read on the list,
   * one detail read on the session view. The mark is taken with a socket
   * confirmed open, so the notice is delivered in the same breath rather than
   * waiting for the next rung of the page's ladder — which would leave the
   * counted window open across a fallback tick that has nothing to do with
   * the handover.
   */
  test("a dead feed falls back to polling and recovers on its own", async ({ page, request }) => {
    // Two kill-and-recover cycles at a three-second cadence, plus a real
    // session create: past the 60-second default by construction rather than
    // by being slow.
    test.setTimeout(150_000);
    const session = await createSession(request, { title: `fallback-${Date.now()}` });
    created.push(session.id);
    const { feed, reads } = await healthyFeed(page);
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });

    /**
     * Wait until one more fallback tick has been observed, then leave a beat
     * for the rest of that tick's reads to be issued.
     *
     * A tick fires its reads together (the list's listing walk and its host
     * read are spawned in one go), so "the count moved" does not mean the
     * tick is over — half a second later it is, and the remainder of the
     * interval is then clear for the caller to notify into.
     */
    const afterFallbackTick = async () => {
      const before = reads.count();
      await expect
        .poll(() => reads.count(), {
          timeout: 15_000,
          message: "the fallback must keep polling until the feed comes back",
        })
        .toBeGreaterThan(before);
      await page.waitForTimeout(500);
    };

    /**
     * Wait until the page has a feed socket open at this very moment.
     *
     * A different question from `connections()`, and the one that matters
     * before greeting: an ungreeted subscription is torn down by the client's
     * own handshake deadline and reopened a ladder rung later, so an outage
     * has gaps in it with no socket at all. Greeting one that is really there
     * is what keeps the counted window below down to the delivery itself.
     */
    const waitForLiveSocket = async () => {
      await expect
        .poll(() => feed.openSockets(), {
          timeout: 20_000,
          message: "the page must keep trying to resubscribe through the outage",
        })
        .toBeGreaterThan(0);
    };

    await page.waitForTimeout(1_500);
    const quiet = reads.count("listing");
    feed.kill();
    // Two poll intervals plus slack: the fallback's first tick is one
    // interval away, so a single interval could pass on timing alone.
    await page.waitForTimeout(8_000);
    expect(
      reads.count("listing") - quiet,
      "a dead feed on a matching build must fall back to the documented poll",
    ).toBeGreaterThan(0);

    // The client reconnects unasked — the ladder's first rung is half a
    // second, so this has long since happened by now. It may well be on its
    // second or third attempt: nothing has greeted any of them.
    expect(feed.connections()).toBeGreaterThan(1);
    await afterFallbackTick();
    await waitForLiveSocket();
    const handover = {
      listing: reads.count("listing"),
      hosts: reads.count("hosts"),
    };
    handshake(feed, 2);

    // Auto-select (BUGS_BURNDOWN.md issue 5) means a session view is
    // mounted from page load, so even this "first" phase has TWO fallback
    // loops with independent phases — the same straggler race the
    // selected phase below always had. The claim splits the same way:
    // recovery observed per surface, then a quiet window proving every
    // fallback stood down.
    await expect
      .poll(() => reads.count("listing") - handover.listing, {
        timeout: 10_000,
        message: "the handshake owes one listing walk",
      })
      .toBeGreaterThanOrEqual(1);
    await expect
      .poll(() => reads.count("hosts") - handover.hosts, {
        timeout: 10_000,
        message: "and one host read",
      })
      .toBeGreaterThanOrEqual(1);
    await page.waitForTimeout(2_000);
    const settledFirst = reads.count();
    await page.waitForTimeout(9_000);
    expect(
      reads.urls().slice(settledFirst),
      "no fallback may keep polling once the feed is healthy again",
    ).toHaveLength(0);

    // The same cycle with a SESSION SELECTED. Under the sidebar layout
    // (BUGS_BURNDOWN.md issue 5) selecting a session mounts the session
    // view BESIDE the list rather than instead of it, so a notification
    // now fans out to both readers: the view's own detail read, plus the
    // list's listing walk and host read — the accepted cost of keeping
    // both panes live. (The host read behind a stale notice only fires
    // for a stale session, which this one is not, so the detail side
    // still owes exactly one.)
    await openSession(page, session.id, session.title);
    await page.waitForTimeout(1_500);
    const detailQuiet = reads.count("detail");
    // Disarmed before the kill, or the socket the page opens next is greeted
    // on arrival and there is no outage left to cover.
    feed.notifyOnConnect();
    feed.kill();
    await page.waitForTimeout(8_000);
    expect(
      reads.count("detail") - detailQuiet,
      "the session view's own fallback must cover a dead feed too",
    ).toBeGreaterThan(0);

    await afterFallbackTick();
    await waitForLiveSocket();
    const detailHandover = {
      detail: reads.count("detail"),
      listing: reads.count("listing"),
      hosts: reads.count("hosts"),
    };
    handshake(feed, 3);

    // Two independently phased fallback loops are live here (the list's
    // and the detail's), and either can have a tick already in flight at
    // the instant the greeting lands — so exact-count assertions taken
    // across the handshake race that straggler. Split the claim instead:
    // first every surface observes the recovery (the handshake's owed
    // re-reads arrive)...
    await expect
      .poll(() => reads.count("detail") - detailHandover.detail, {
        timeout: 10_000,
        message: "the handshake owes the session view a detail re-read",
      })
      .toBeGreaterThanOrEqual(1);
    await expect
      .poll(() => reads.count("listing") - detailHandover.listing, {
        timeout: 10_000,
        message: "and the still-mounted sidebar its listing walk",
      })
      .toBeGreaterThanOrEqual(1);
    await expect
      .poll(() => reads.count("hosts") - detailHandover.hosts, {
        timeout: 10_000,
        message: "and its host read",
      })
      .toBeGreaterThanOrEqual(1);
    // ...then, with recovery observed everywhere and any straggler tick
    // given time to land, the loops must fall SILENT: a healthy feed
    // switches every fallback off rather than running beside it. This
    // quiet window is the assertion the exact counts were standing in for.
    await page.waitForTimeout(2_000);
    const settled = reads.count();
    await page.waitForTimeout(9_000);
    expect(
      reads.urls().slice(settled),
      "no fallback may keep polling once the feed is healthy again",
    ).toHaveLength(0);
  });

  /**
   * The handshake's own race: a change landing while the socket is down, and
   * after the fallback's last look, is still seen.
   *
   * This is the case the lagged-subscriber test cannot cover, because it is
   * about a NEW subscription: the bump happened before the socket existed,
   * so there is no notification coming for it, and the fallback has already
   * stopped by the time anyone would look. The helm's answer is to send the
   * current revision immediately on every (re)subscription; the client's is
   * to re-read on it BEFORE trusting the feed again.
   *
   * ## The mutation happens during the outage, which is the whole point
   *
   * Renaming while the feed was still healthy would test a different and
   * much weaker thing — a page that simply had not looked yet. The window
   * this test is named for opens when the socket dies and closes when the
   * next one greets, and the rename has to land INSIDE it.
   *
   * ## Attribution is by construction, not by timing
   *
   * Every listing read taken during the outage is answered from a capture of
   * what the helm said BEFORE the rename, so the fallback poll — which will
   * certainly fire, since the reconnect ladder can outlast a three-second
   * interval on a slow engine — cannot put the new title on screen. That the
   * answers are stale rather than failed is the load-bearing choice, and the
   * gate's own docs carry why: a failed read leaves its reader retrying on a
   * schedule of its own, so the first successful read after the freeze lifts
   * might be that retry rather than anything the feed asked for, and no
   * counter can tell the two apart. A satisfied reader has no standing
   * demand, so once the freeze lifts the ONLY thing that can produce a read
   * is the notice — and `reachedHelm() === 1` says exactly one read was
   * produced.
   *
   * The freeze lifts in the same synchronous breath as the notification, and
   * immediately after a stale answer has been served — so the next fallback
   * tick is a whole interval away rather than a coin flip.
   *
   * ## The revision is a REPEAT, because that is what a resubscription sends
   *
   * The greeting carries revision 1: the very number this page acted on
   * before the socket died. A fresh subscription cannot invent a newer one —
   * the helm sends whatever the current revision is — so a client that
   * treated notifications as change detection would discard this message as
   * "nothing new" and keep a world the outage made stale. Greeting with 2
   * would let exactly that client pass, which is why the number here is not
   * incidental.
   *
   * The notice is ARMED before the freeze lifts and SENT after it, and both
   * halves are deliberate. Arming binds sockets that do not exist yet, which
   * is what covers the client tearing an ungreeted subscription down
   * mid-staging and reopening it a rung later; it is inert for the socket
   * that is already there, so it cannot deliver anything while stale answers
   * are still being served. The send is then the second half of the same
   * synchronous breath as `gate.thaw()`.
   */
  test("a mutation that lands while the socket is down is seen at the handshake", async ({
    page,
    request,
  }) => {
    // A real session create, a staged outage at the fallback's own cadence,
    // and a repaint deadline on the far side of it: the waiting is the
    // mechanism rather than slowness.
    test.setTimeout(120_000);
    const session = await createSession(request, { title: `handshake-${Date.now()}` });
    created.push(session.id);
    const gate = await gateListingReads(page);
    const { feed } = await healthyFeed(page);
    await expect(row(page, session.id).locator(".session-title")).toHaveText(session.title, {
      timeout: 20_000,
    });

    // The outage. The freeze is taken BEFORE the rename below, so the reply
    // it holds is a world without it — and every read the fallback takes
    // while the socket is down is answered from that world, successfully.
    gate.freeze();
    feed.kill();
    await feed.waitForConnection(2);

    // The change the page cannot possibly know about — made DURING the
    // outage, and behind a gate that cannot serve it.
    const renamed = `${session.title}-unseen`;
    await renameSession(request, session.id, renamed);
    // Synchronised against the fallback's own cadence: waiting for a stale
    // answer to be served proves the fallback really is reading (so the gate
    // is doing work rather than guarding an idle page) and leaves most of an
    // interval before the next tick.
    await gate.waitForStaleAnswer();
    // The rename has not been painted, and the row is still there saying the
    // old name — which is the honest mid-outage picture: the page has been
    // reading all along and every answer described the world it started in.
    await expect(row(page, session.id).locator(".session-title")).toHaveText(session.title);
    await expect(page.locator(".session-title", { hasText: renamed })).toHaveCount(0);

    // The SAME revision the page already acted on. Armed for a socket that
    // may not exist yet, then thawed and sent with nothing awaited in
    // between, so no route callback can run between the thaw and the notice.
    feed.notifyOnConnect(1);
    gate.thaw();
    if (feed.openSockets() > 0) feed.notify(1);
    await expect(row(page, session.id).locator(".session-title")).toHaveText(renamed, {
      timeout: 20_000,
    });
    expect(
      gate.reachedHelm(),
      "the handshake owes exactly one read: the reads taken during the outage were all " +
        "answered, so nothing was owed to a retry, and a greeted subscription is healthy so " +
        "the fallback is off — a second read here would mean the repaint had another author",
    ).toBe(1);
  });

  /**
   * The withdrawal rule (SPEC_impl.md): under build SKEW the feed AND the
   * fallback poll both stop.
   *
   * The distinction this pins is the one that is easy to get wrong in the
   * comfortable direction — a page whose feed has just been withdrawn looks
   * exactly like a page whose feed has died, and "the feed is gone, so poll"
   * is the rule everywhere else in this file. It must not apply here:
   * polling a helm on another build means re-reading rows written in a
   * vocabulary this bundle does not have, three seconds at a time, forever.
   *
   * The reload prompt is asserted alongside, because withdrawal without an
   * explanation is the silent degradation the skew gate exists to prevent.
   */
  test("build skew stops the feed and the fallback poll both", async ({ page }) => {
    const feed = await stubFeed(page);
    const reads = countReads(page);
    await forceBuildSkew(page, "9.9.9-not-this-bundle");
    await page.goto("/");

    // Deliberately NO handshake anywhere in this test, which is what makes
    // it the strong version: the feed is never healthy, so every other rule
    // in this file says the fallback should be polling. Skew is the one
    // thing that stops it, and nothing else here could be mistaken for the
    // cause.
    await expect(page.locator(".build-skew")).toBeVisible({ timeout: 20_000 });
    await expect(page.locator(".build-skew")).toContainText("reload");

    await page.waitForTimeout(2_000);
    const subscriptions = feed.connections();
    expect(
      subscriptions,
      "the page withdraws the feed rather than reconnecting into it",
    ).toBeLessThanOrEqual(1);
    // The half a connection count cannot express. One connection is what a
    // withdrawn page and a page holding a silent socket open forever BOTH
    // report, and only the second is the failure: a live socket is a helm on
    // another build one notification away from sending this bundle off to
    // re-read rows it cannot decode. Zero open sockets covers both honest
    // outcomes — the subscription that opened and was withdrawn, and (on an
    // engine where the mismatch latched first) the one that was never opened
    // at all, which the delayed-asset test below is about.
    await expect
      .poll(() => feed.openSockets(), {
        timeout: 10_000,
        message: "a withdrawn feed must not leave its socket open",
      })
      .toBe(0);
    const before = reads.count();

    await page.waitForTimeout(12_000);
    expect(
      reads.count() - before,
      `a skewed page must stand down entirely; saw ${reads.urls().slice(before).join(", ")}`,
    ).toBe(0);
    expect(
      feed.connections(),
      "and must not climb a reconnect ladder back to a helm it cannot decode",
    ).toBe(subscriptions);
  });

  /**
   * The OTHER half of the withdrawal rule: a skewed page still does what a
   * person asks it to.
   *
   * SPEC_impl.md draws the line at attendance rather than at risk —
   * "withdraws every UNATTENDED behavior … while anything the user explicitly
   * asks for keeps working" — and both halves have teeth. The test above pins
   * the first; without this one, a page that stood every read down would pass
   * it perfectly while leaving a skewed user with a search box that does
   * nothing, which is the failure mode a reviewer would never see because it
   * looks exactly like the rule being obeyed.
   *
   * The classification lives one layer down (`reader::Trigger`) and its unit
   * tests already prove an Explicit demand survives a latched mismatch. What
   * only a browser can say is that the UI CLASSIFIES a filter submit that
   * way: the assertion is a request on the wire carrying the search, and rows
   * that changed because of it.
   *
   * The unattended half is asserted in the same test rather than trusted from
   * the one above, because the interesting failure is a fix that reopens the
   * floodgates — restoring the submit by ungating reads altogether.
   */
  test("a skewed page still reads when a person submits a filter", async ({ page, request }) => {
    const stamp = Date.now();
    const needle = `skew-needle-${stamp}`;
    const wanted = await createSession(request, { title: needle });
    const other = await createSession(request, { title: `skew-haystack-${stamp}` });
    created.push(wanted.id, other.id);

    const feed = await stubFeed(page);
    const reads = countReads(page);
    await forceBuildSkew(page, "9.9.9-not-this-bundle");
    await page.goto("/");
    await openFilterBar(page);
    await expect(page.locator(".build-skew")).toBeVisible({ timeout: 20_000 });
    // The mount read is explicit too (a person navigated here), so the rows
    // are on screen even under skew — which is what gives the filter below
    // something to narrow.
    await expect(row(page, wanted.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, other.id)).toBeVisible();

    // Nothing unattended is running: four poll intervals with a socket the
    // page was told to give up and a fallback it was told not to start.
    await page.waitForTimeout(1_500);
    const quiet = reads.count();
    await page.waitForTimeout(12_000);
    expect(
      reads.count() - quiet,
      `a skewed page reads nothing on its own; saw ${reads.urls().slice(quiet).join(", ")}`,
    ).toBe(0);

    // And then a person asks.
    const before = reads.count("listing");
    await page.locator(".filter-title").fill(needle);
    await page.locator(".filter-apply").click();

    await expect(page.locator(".session-row")).toHaveCount(1, { timeout: 20_000 });
    await expect(row(page, wanted.id)).toBeVisible();
    await expect(row(page, other.id)).toHaveCount(0);
    const asked = reads.urls("listing").slice(before);
    expect(
      asked.length,
      "a submit under skew must reach the helm, or the search box is decorative",
    ).toBeGreaterThan(0);
    expect(
      asked.every((url) => new URL(url).searchParams.get("title") === needle),
      `every read the submit produced must carry the search; saw ${asked.join(", ")}`,
    ).toBe(true);
    // The banner is the helm's own count, so it is the half the page could
    // not have produced by narrowing rows it already held.
    await expect(page.locator(".session-count")).toHaveText(/^1 matching of \d+ sessions$/);
    // Still skewed, and still saying so: the read the user asked for is not a
    // reason to forget the mismatch.
    await expect(page.locator(".build-skew")).toBeVisible();
  });

  /**
   * The withdrawal binds a subscription that had not been made yet.
   *
   * The ordering hole this pins is real and asymmetric: the page subscribes
   * from a task and withdraws from an effect, and nothing orders those two
   * against each other. A mismatch latched on the very first reply therefore
   * calls the withdrawal while `events.js` may still be in flight — and a
   * withdrawal that only stops what is already running does nothing at all in
   * that case, leaving the skewed page to open its socket a moment later and
   * hold it, which is precisely the unattended behavior the withdrawal rule
   * exists to revoke.
   *
   * Stalling the asset is how the unlikely order is made the certain one: the
   * script cannot register until this test releases it, and by then the skew
   * banner is on screen, so what happens next is entirely about whether the
   * withdrawal BINDS or merely stops.
   *
   * The queued handshake is what makes the second assertion mean something. A
   * socket that opens against a stubbed feed and is never spoken to reads
   * nothing for the boring reason; arming the greeting means any subscription
   * that does open is notified at once, so a page that both subscribes and
   * re-reads fails loudly rather than only in the connection count.
   */
  test("a page that latched skew before the feed asset loaded never subscribes", async ({
    page,
  }) => {
    const feed = await stubFeed(page);
    const reads = countReads(page);
    feed.notifyOnConnect(7);

    let release = () => {};
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    // The asset is fingerprinted by the bundler (`events-dxh<hash>.js`), so
    // the route matches the stem rather than a fixed name. Holding it costs
    // the page nothing else: this script is injected by the running app
    // rather than by the HTML, so nothing about the app's own startup waits
    // on it.
    await page.route(
      (url) => /^\/assets\/events[^/]*\.js$/.test(url.pathname),
      async (route: Route) => {
        await held;
        await route.continue();
      },
    );
    await forceBuildSkew(page, "9.9.9-not-this-bundle");
    await page.goto("/");

    // Skew is latched and announced, with the feed's own JavaScript still
    // sitting in the gate.
    await expect(page.locator(".build-skew")).toBeVisible({ timeout: 20_000 });
    expect(feed.connections(), "the asset is held, so nothing can have subscribed yet").toBe(0);
    // Auto-select mounts a session at load, and its mount read is
    // ATTENDED — permitted under skew exactly like the list's own — so it
    // must finish landing before the counted quiet window opens (on a
    // slow engine it otherwise trails into it and reads as unattended).
    await expect(page.locator(".titlebar .title")).toBeVisible({ timeout: 20_000 });
    await expect
      .poll(() => reads.count("detail"), { timeout: 20_000 })
      .toBeGreaterThanOrEqual(1);
    await page.waitForTimeout(1_500);
    const before = reads.count();

    release();
    // Long enough for the island's registration poll (50ms) and a
    // subscription to follow it many times over.
    await page.waitForTimeout(5_000);

    expect(
      feed.connections(),
      "a withdrawal that only stops a LIVE subscription leaves a skewed page holding a socket " +
        "it was told to give up",
    ).toBe(0);
    expect(
      reads.count() - before,
      `a skewed page reads nothing, notice or no notice; saw ${
        reads.urls().slice(before).join(", ")
      }`,
    ).toBe(0);
  });

  /**
   * A status transition arrives through the feed, without a refresh —
   * PLAN_M6_75.md's testing decision, and the milestone's user-visible point
   * in one assertion.
   *
   * The real socket, deliberately: what is being proven is that the
   * supervisor's sampler, the helm's changed-only publish, and the client's
   * re-read compose into a badge that changes on screen with nobody
   * touching the page.
   *
   * The session is created AFTER the page is loaded, so the row itself
   * arrives through the feed and its badge is watched from the moment it
   * appears. `basic` prints and then goes quiet, which is exactly the shape
   * the sampler decays to `idle`.
   *
   * What is asserted is the TRANSITION and only that: the badge was
   * something other than `idle` first and became `idle` without a reload.
   * The "something else" is deliberately unpinned — it may be no badge at
   * all (nothing has classified the session yet, and an unclassified status
   * is rendered as nothing) or a live word, and which of the two is observed
   * depends on how the sampler's schedule lines up with this page's reads.
   * Requiring a specific first observation would make this a test of that
   * timing rather than of the feed carrying a change.
   */
  test("a status transition arrives through the feed without a refresh", async ({
    page,
    request,
  }) => {
    await page.goto("/");
    await expect(page.locator(".session-list")).toBeVisible({ timeout: 20_000 });

    const session = await createSession(request, { title: `status-${Date.now()}` });
    created.push(session.id);

    // The row arrives without a reload, which is the feed carrying a create.
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });

    // Every badge text this row is ever seen wearing. Collected while
    // polling the DOM — a client-side read, not a network one, so it says
    // nothing about whether the page is polling the helm.
    const seen = new Set<string>();
    const badge = row(page, session.id).locator(".status-badge");
    await expect
      .poll(
        async () => {
          const texts = await badge.allTextContents();
          const current = texts.length ? texts[0].trim() : "";
          seen.add(current);
          return current;
        },
        {
          timeout: 40_000,
          message: "a quiet fake agent must decay to idle, and say so without a refresh",
        },
      )
      .toBe("idle");

    expect(
      [...seen].filter((state) => state !== "idle").length,
      `the badge must have been something else first (saw ${[...seen].join(", ")})`,
    ).toBeGreaterThan(0);

    // And the session really is the one the helm is describing, rather than
    // a stale row this page happened to keep.
    const listing = await listSessions(request, `title=${encodeURIComponent(session.title)}`);
    expect(listing.sessions[0]?.status?.state).toBe("idle");
  });
});
