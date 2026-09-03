// Shared machinery for the M6.75 UI specs (PLAN_M6_75.md items 6 and 7):
// creating and cleaning up sessions through the real API, and taking
// deterministic control of the two channels the feed work is about — the
// invalidation socket and the build stamp.
//
// This module owns the shared fleet contract: real API fixtures and the
// non-obvious feed behavior that specs need to control. The terminal island
// has a separate contract in helpers/term.ts — `window.__farhelmTerm`,
// `term.buffer.active`, and readiness globals — because terminal assertions
// rely on that surface across spec files. Genuinely one-off snippets still
// stay local.
import { APIRequestContext, expect, Locator, Page, Route, WebSocketRoute } from "@playwright/test";
import path from "node:path";
import { spawn } from "node:child_process";

/**
 * The fake agent's `basic` script, as the create form's `invocation` string
 * — an absolute path, quoted, exactly as the terminal spec family's shared
 * fixture and sidebar.spec.ts build theirs. The supervisor shell-splits it into argv
 * when the session launches.
 *
 * `basic` is deliberate: it prints, echoes, and then goes quiet, which is
 * what makes it a usable fixture for a status transition (the supervisor's
 * sampler classifies a quiet pane as idle after a few looks).
 */
export const FAKE_AGENT = `"${
  path.resolve(__dirname, "../../../target/debug/farhelm")
}" internal fake-agent --script basic`;

/**
 * The session LISTING endpoint, as a route matcher.
 *
 * A pathname predicate rather than the `"**\/api/sessions"` glob every spec
 * here used to write, and the difference is not stylistic. Playwright anchors
 * a glob against the WHOLE request URL, query string included, so that glob
 * matched the listing only while the listing carried no parameters. The
 * sidebar's sort control ended that: every list read now carries at least
 * `?sort=`, and every one of those handlers silently stopped intercepting —
 * silently being the problem, since an interceptor that never fires makes its
 * test assert against the live stack instead of against its fixture.
 *
 * Deliberately matches the POST create endpoint too, because it is the same
 * path; handlers that care keep their own method guard, exactly as they did
 * under the glob.
 *
 * A shared CONSTANT rather than an inline arrow at each call site so that
 * `page.unroute` can be handed the same reference: Playwright compares
 * function matchers by identity, so an unroute with a fresh arrow removes
 * nothing.
 */
export const SESSION_LISTING = (url: URL) => url.pathname === "/api/sessions";

/**
 * A session as the helm's JSON describes it, narrowed to what these specs
 * read.
 *
 * Narrowed DELIBERATELY rather than transcribed: a field declared here is a
 * field some assertion depends on, so the interface doubles as the list of
 * wire fields this suite is pinned to. Adding one that nothing reads makes
 * that list a lie and invites a later reader to believe the suite covers a
 * field it never looks at.
 */
export interface SessionRow {
  id: string;
  title: string;
  status?: { state: string };
  /** Working directory and invocation as the listing reports them; the
   * badge-render test compares the rendered row against these. */
  cwd?: string;
  invocation?: string;
  /**
   * The profile this session was created from, absent for a raw-created one.
   *
   * `existence` is the field specs actually wait on: the helm derives it per
   * reply from its catalog, so it changes under a session nobody touched. A
   * spec that renames a profile and then asserts about a row has to settle on
   * this value before it plays a notification, or it is telling the page to
   * re-read a view that has not moved yet.
   */
  source_profile?: { id: string; name: string; existence: string };
}

/**
 * The whole `GET /api/sessions` reply, narrowed on the same terms as
 * [`SessionRow`] — which here means the rows and nothing else.
 *
 * The reply also carries `total`, `matching` and `truncated`, and they are
 * deliberately absent: the specs read those NUMBERS off the count banner the
 * page renders, which is the claim worth pinning, and re-declaring them here
 * would suggest a wire-level assertion nothing makes.
 */
export interface SessionPage {
  sessions: SessionRow[];
}

/** A registry row from `GET /api/hosts`, narrowed to what these specs read. */
export interface HostRow {
  id: number;
  kind: string;
  /** The helm's connection token, which a session create echoes back as
   * `expected_incarnation` — captured by the wire specs so they can assert the
   * request carried THIS value rather than merely some value. The create is
   * the only guarded request; profile reads and edits carry no precondition. */
  incarnation: number;
}

/**
 * One profile from `GET /api/profiles`, narrowed on the same
 * terms as [`SessionRow`]: a field here is a field some assertion depends on.
 *
 * All of them are, now. The preservation spec reads the whole definition back
 * to prove an edit of one field rewrote nothing else, so the type says so
 * rather than leaving a reader to believe this suite only ever looks at names.
 */
export interface ProfileRow {
  id: string;
  name: string;
  invocation: string;
  agent_kind: string;
  resume_template: string[] | null;
}

/**
 * The profiles reply: the helm-wide catalog plus this helm's remembered
 * default.
 *
 * The pair travels together on the wire deliberately (the helm serves the
 * remembered id RAW, even when it names a deleted profile), and it is what
 * SPEC.md's ask-don't-guess fallback is keyed off — so a spec that wants to
 * know which state the create dialog should be in reads both from here.
 */
export interface ProfilesView {
  profiles: ProfileRow[];
  default_profile: string | null;
}

/** Fail loudly rather than returning a half-decoded body: every caller here
 * is setting up a fixture, and a fixture that quietly did not happen makes
 * the test that follows assert against the wrong world. */
async function ok(response: { ok(): boolean; status(): number; text(): Promise<string> }, what: string) {
  if (!response.ok()) {
    throw new Error(`${what} failed (${response.status()}): ${await response.text()}`);
  }
}

/** The whole registry. */
export async function listHosts(request: APIRequestContext): Promise<HostRow[]> {
  const response = await request.get("/api/hosts");
  await ok(response, "reading the host registry");
  return (await response.json()).hosts;
}

/** The reserved local row's id — the host every fixture here creates on. */
export async function localHostId(request: APIRequestContext): Promise<number> {
  const local = (await listHosts(request)).find((host) => host.kind === "local");
  if (!local) throw new Error("the helm reported no local host row");
  return local.id;
}

/** One page of the session list, with an optional filter query string. */
export async function listSessions(
  request: APIRequestContext,
  query = "",
): Promise<SessionPage> {
  const response = await request.get(`/api/sessions${query ? `?${query}` : ""}`);
  await ok(response, `listing sessions (${query || "unfiltered"})`);
  return await response.json();
}

/**
 * Create a session through the real API, the way every client does.
 *
 * Deliberately NOT through the UI's create form: these specs are about what
 * the UI does when the FLEET changes underneath it, so the change has to
 * arrive from somewhere other than the page under test — a create driven
 * through the form would be indistinguishable from the page having been
 * told, which is the very thing the two-client and feed tests set out to
 * prove.
 */
export async function createSession(
  request: APIRequestContext,
  body: {
    title: string;
    cwd?: string;
    invocation?: string;
    host?: number;
    profile_id?: string;
  },
): Promise<SessionRow> {
  const response = await request.post("/api/sessions", {
    data: {
      cwd: body.cwd ?? "/tmp",
      title: body.title,
      ...(body.profile_id
        ? { profile_id: body.profile_id }
        : { invocation: body.invocation ?? FAKE_AGENT }),
      ...(body.host === undefined ? {} : { host: body.host }),
    },
  });
  await ok(response, `creating session ${body.title}`);
  return await response.json();
}

/** Rename a session — the cheapest single-request mutation there is, which
 * is why the feed specs use it as their "something changed" event. */
export async function renameSession(
  request: APIRequestContext,
  id: string,
  title: string,
): Promise<void> {
  const response = await request.post(`/api/sessions/${id}/rename`, { data: { title } });
  await ok(response, `renaming session ${id}`);
}

/**
 * Stop a session, insisting that it worked.
 *
 * Separate from [`cleanupSession`], which tolerates a session that is already
 * gone: a stop used as a FIXTURE is a precondition for the assertions that
 * follow — most often "this row now reads exited" — and a refused stop there
 * has to fail as a setup error rather than as a filter or a badge mysteriously
 * disagreeing with the test's expectations.
 */
export async function stopSession(request: APIRequestContext, id: string): Promise<void> {
  const response = await request.post(`/api/sessions/${id}/stop`);
  await ok(response, `stopping session ${id}`);
}

/**
 * Stop and delete a session, tolerating either already being done.
 *
 * Cleanup runs after a failed test too, so "it is already gone" is a normal
 * outcome rather than an error worth propagating over the real failure.
 */
export async function cleanupSession(request: APIRequestContext, id: string): Promise<void> {
  for (const [what, response] of [
    ["stopping", await request.post(`/api/sessions/${id}/stop`)],
    ["deleting", await request.delete(`/api/sessions/${id}`)],
  ] as const) {
    if (!response.ok() && response.status() !== 404) {
      throw new Error(
        `cleanup: ${what} session ${id} failed (${response.status()}): ${await response.text()}`,
      );
    }
  }
}

/** The helm-wide profile catalog, as every UI consumer reads it. */
export async function listProfiles(request: APIRequestContext): Promise<ProfilesView> {
  const response = await request.get("/api/profiles");
  await ok(response, "reading profiles");
  return await response.json();
}

/**
 * Define a profile through the real API.
 *
 * Deliberately NOT through the panel, for [`createSession`]'s reason: a
 * fixture built through the surface under test cannot distinguish "the page
 * was told" from "the page did it itself", which is exactly what the
 * feed-driven and snapshot tests set out to separate. The one spec that
 * drives the panel does so as its subject, not as setup.
 *
 * `agent_kind` defaults to `generic` because the fake agent is not an
 * integrated one — naming a kind here would ask the supervisor to apply
 * Claude Code's or Codex's heuristics to a script that has neither shape.
 */
export async function createProfile(
  request: APIRequestContext,
  body: { name: string; invocation?: string; agent_kind?: string; resume_template?: string[] },
): Promise<ProfileRow> {
  const response = await request.post("/api/profiles", {
    data: {
      name: body.name,
      invocation: body.invocation ?? FAKE_AGENT,
      agent_kind: body.agent_kind ?? "generic",
      ...(body.resume_template ? { resume_template: body.resume_template } : {}),
    },
  });
  await ok(response, `creating profile ${body.name}`);
  return await response.json();
}

/**
 * Replace a profile's definition — the whole definition, because the API
 * replaces rather than merges and a partial body would clear what it omitted.
 */
export async function updateProfile(
  request: APIRequestContext,
  id: string,
  body: { name: string; invocation?: string; agent_kind?: string; resume_template?: string[] },
): Promise<ProfileRow> {
  const response = await request.post(`/api/profiles/${id}`, {
    data: {
      name: body.name,
      invocation: body.invocation ?? FAKE_AGENT,
      agent_kind: body.agent_kind ?? "generic",
      ...(body.resume_template ? { resume_template: body.resume_template } : {}),
    },
  });
  await ok(response, `updating profile ${id}`);
  return await response.json();
}

/**
 * Delete a profile, tolerating one that is already gone.
 *
 * Cleanup runs after a failed test and after tests that delete the profile
 * themselves as part of what they prove, so "already gone" is a normal
 * outcome rather than an error worth raising over the real failure. Leaving
 * one behind is not an option: the stack is shared, and a live profile
 * changes what the NEXT spec's create dialog preselects.
 */
export async function cleanupProfile(
  request: APIRequestContext,
  id: string,
): Promise<void> {
  const response = await request.delete(`/api/profiles/${id}`);
  if (!response.ok() && response.status() !== 404) {
    throw new Error(
      `cleanup: deleting profile ${id} failed (${response.status()}): ${await response.text()}`,
    );
  }
}

/**
 * A stubbed invalidation feed: the page believes it is subscribed, and this
 * test decides what it is ever told.
 *
 * The stub replaces the helm's socket entirely rather than proxying it, and
 * that is what makes the assertions in these specs provable rather than
 * probable. A proxied feed carries whatever the shared stack happens to be
 * doing — another spec's leftover session settling into `idle` bumps the
 * revision — so "a healthy feed performs no periodic reads" could only ever
 * be observed as "no reads happened to occur", which is a different claim.
 *
 * Install BEFORE `page.goto`: a route only applies to sockets opened after
 * it exists.
 */
/**
 * ## A socket this stub never greets does not live long
 *
 * The client gives a fresh subscription a bounded time to say something and
 * tears it down when nothing comes (a real helm greets immediately, so
 * silence means the far end accepted the upgrade and then wedged). A test
 * that stages a long outage therefore watches the page open, abandon and
 * reopen sockets the whole time, and "the socket I saw a moment ago" is not
 * something it may assume still exists — which is what
 * [`FeedStub::notifyOnConnect`] is for and why [`FeedStub::openSockets`] is
 * worth asking before [`FeedStub::notify`].
 */
export interface FeedStub {
  /** How many times the page has opened a feed socket, handshakes and
   * reconnects alike — the page's own recovery, made observable. Climbs on
   * its own during an ungreeted outage, per the note above. */
  connections(): number;
  /**
   * How many of those sockets are still OPEN.
   *
   * The counterpart [`FeedStub::connections`] cannot supply, and the reason
   * this stub tracks lifetime rather than just arrivals: a page that has
   * withdrawn its feed and a page holding one silent socket open forever
   * both report exactly one connection, and only the second is the bug the
   * skew tests are looking for. It is also the question to ask before
   * notifying, since an ungreeted socket is torn down on the client's own
   * schedule rather than on the test's.
   */
  openSockets(): number;
  /** Send a revision notification on the live socket, handshake or bump —
   * the helm makes no distinction and neither does this. Throws if there is
   * no socket to send on, which for a stub is a setup error rather than a
   * condition worth tolerating silently. */
  notify(revision: number): void;
  /**
   * Greet every socket the page opens FROM NOW ON with `revision`, the
   * instant it opens; called with no argument, stop greeting.
   *
   * The helm's own behavior (it sends the current revision immediately on
   * every (re)subscription), armed ahead of time, and it earns its keep two
   * ways. It is the only way to hand a notice to a page whose socket comes
   * and goes on a schedule the test does not control — see the note above —
   * and it is what lets a test say something about a subscription that was
   * never supposed to open at all: if the page opens one anyway, the notice
   * is already on the mat and any read that follows is visible.
   *
   * Sticky rather than one-shot, so a reconnect ladder's later rungs are
   * greeted too. DISARM it before staging an outage the page must not
   * recover from until the test says so, or the socket it opens next is made
   * healthy on arrival and the fallback never runs.
   */
  notifyOnConnect(revision?: number): void;
  /** Drop the live socket, as a helm restart or a lost network would. */
  kill(): void;
  /** Wait until the page has opened its `nth` socket (1-based). */
  waitForConnection(nth: number): Promise<void>;
}

export async function stubFeed(page: Page): Promise<FeedStub> {
  let live: WebSocketRoute | undefined;
  let connections = 0;
  let greeting: number | undefined;
  // The sockets that have not closed. A SET rather than a counter because
  // both the page and this stub can end a socket, and set removal is
  // idempotent where a decrement would double-count a socket whose close
  // this stub performed and whose close handler then fired anyway.
  const open = new Set<WebSocketRoute>();
  await page.routeWebSocket("**/api/events**", (ws) => {
    live = ws;
    connections += 1;
    open.add(ws);
    ws.onClose(() => {
      open.delete(ws);
      if (live === ws) live = undefined;
    });
    // Deliberately no automatic handshake unless one was ARMED: every spec
    // here is about WHEN the client is told the current revision, so the
    // moment has to be the test's to choose — and arming is how a test
    // chooses "the moment a socket appears at all".
    if (greeting !== undefined) ws.send(JSON.stringify({ revision: greeting }));
  });
  return {
    connections: () => connections,
    openSockets: () => open.size,
    notify(revision: number) {
      if (!live) throw new Error("no feed socket is open to notify on");
      live.send(JSON.stringify({ revision }));
    },
    notifyOnConnect(revision?: number) {
      greeting = revision;
    },
    kill() {
      if (!live) throw new Error("no feed socket is open to kill");
      const socket = live;
      live = undefined;
      open.delete(socket);
      socket.close();
    },
    async waitForConnection(nth: number) {
      const deadline = Date.now() + 15_000;
      while (connections < nth) {
        if (Date.now() > deadline) {
          throw new Error(`the page never opened feed socket #${nth} (saw ${connections})`);
        }
        await page.waitForTimeout(50);
      }
    },
  };
}

/** One held read: what the helm answered when it arrived, and (privately) the
 * switch that lets that answer through. */
export interface HeldRead {
  status: number;
  headers: Record<string, string>;
  /** The reply as TEXT, so a test can decode it, change one field, and send
   * the result back as this read's answer. */
  body: string;
}

/**
 * Hold every matching read open — reply in hand, delivery on the test's word.
 *
 * ## Why holding one read is not enough any more
 *
 * Each surface reads through ONE reader (`farhelm-ui/src/reader.rs`): a
 * notification arriving while a read is in flight becomes that read's single
 * follow-up rather than a second concurrent walk, and a mutation's own
 * refresh queues the same way. So a fixture that holds one stale reply and
 * lets everything else through is not staging a race — it is staging a QUEUE,
 * and the reply behind the stale one repairs the screen microseconds after
 * the stale one lands. Tests written that way pass with the ordering gates
 * deleted, because they never look in the window where the damage is visible.
 *
 * Holding the repair too is what opens that window. The sequence is: hold the
 * stale read, hold whatever would repair it, release the stale one, ASSERT,
 * and only then release the repair.
 *
 * ## Captured, not merely intercepted
 *
 * Each read's reply is fetched the moment the request arrives, so it
 * describes the world at THAT moment rather than at delivery — which is what
 * makes a held reply genuinely stale rather than merely late. A caller must
 * wait for the capture ([`HeldReads::waitForCaptures`]) before mutating, or
 * the "stale" reply carries the very change it was supposed to predate.
 *
 * Capture ORDER is dispatch order for reads through one reader, since the
 * next is not sent until the previous is delivered. A read from outside the
 * reader — a stop's own refetch — can arrive alongside; a test expecting one
 * should say where it lands.
 */
export interface HeldReads {
  /** How many replies are in hand. */
  captured(): number;
  /** Resolve once at least `count` replies are in hand. */
  waitForCaptures(count: number): Promise<void>;
  /** The reply captured for read `nth` (1-based). */
  reply(nth: number): HeldRead;
  /**
   * Deliver read `nth`, optionally answering with a different status or body
   * than the helm gave.
   *
   * The helm's own headers are kept either way — the build stamp above all,
   * whose absence would latch skew and stand the page down mid-test — but the
   * length and encoding headers are dropped when the body is replaced, since
   * they describe bytes that are no longer being sent.
   */
  release(nth: number, replacement?: { status?: number; body?: string }): void;
  /** Deliver everything still held, and let later reads through untouched. */
  releaseAll(): void;
}

export async function holdReads(
  page: Page,
  // A PREDICATE rather than a glob, because the read to hold is identified by
  // its path alone: an unfiltered listing walk asks for `/api/sessions` with
  // no query string at all and its later pages ask for the same path with a
  // cursor, and no single glob covers both without also swallowing
  // `/api/sessions/{id}`.
  matches: (url: URL) => boolean,
): Promise<HeldReads> {
  type Replacement = { status?: number; body?: string };
  const held: { reply: HeldRead; deliver: (replacement?: Replacement) => void }[] = [];
  let open = false;
  await page.route(matches, async (route: Route) => {
    if (route.request().method() !== "GET" || open) {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    const reply: HeldRead = {
      status: response.status(),
      headers: response.headers(),
      body: await response.text(),
    };
    let deliver: (replacement?: Replacement) => void = () => {};
    const released = new Promise<Replacement | undefined>((resolve) => {
      deliver = resolve;
    });
    // Pushed only once the reply is IN HAND, which is what makes a capture
    // count a statement about staleness rather than about arrival.
    held.push({ reply, deliver });
    const replacement = await released;
    if (!replacement) {
      await route.fulfill(reply);
      return;
    }
    const headers = { ...reply.headers };
    if (replacement.body !== undefined) {
      delete headers["content-length"];
      delete headers["content-encoding"];
    }
    await route.fulfill({
      status: replacement.status ?? reply.status,
      headers,
      body: replacement.body ?? reply.body,
    });
  });
  const at = (nth: number) => {
    const read = held[nth - 1];
    if (!read) throw new Error(`read #${nth} has not been captured (saw ${held.length})`);
    return read;
  };
  return {
    captured: () => held.length,
    async waitForCaptures(count: number) {
      await expect
        .poll(() => held.length, {
          timeout: 30_000,
          message: `the page must have taken ${count} read(s) by now`,
        })
        .toBeGreaterThanOrEqual(count);
    },
    reply: (nth) => at(nth).reply,
    release(nth, replacement) {
      at(nth).deliver(replacement);
    },
    releaseAll() {
      open = true;
      for (const read of held) read.deliver();
    },
  };
}

/**
 * Hold a mutation's REPLY until the test says so, while the server acts at
 * once.
 *
 * The window every ordering test around a stop or a restart needs: the
 * operation has really happened and the CLIENT does not know yet — which is
 * exactly the state the epoch bumps and the post-mutation refresh are written
 * for, and a state a real supervisor passes through too fast to assert in.
 *
 * Re-fulfilled from the reply the helm actually gave, headers included, for
 * the reason [`holdReads`] gives.
 */
export async function holdMutation(
  page: Page,
  matches: (url: URL) => boolean,
): Promise<() => void> {
  let release = () => {};
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route(matches, async (route: Route) => {
    const response = await route.fetch();
    await held;
    await route.fulfill({ response });
  });
  return () => release();
}

/**
 * Which of the removed periodic loops a read belongs to.
 *
 * There were FOUR loops, not two, and they are separated here because a
 * no-polling proof that lumps them together only ever proves the ones the
 * page under test happens to be showing: the list's listing walk and its
 * host registry read, and the session view's detail read and the host
 * registry read behind a stale session's notice. A test that mounts only the
 * list and counts "any read" says nothing at all about the other two.
 *
 * There is deliberately no `any` member: every place that wants the union
 * simply omits the argument, and a named member for "all of them" would be a
 * second spelling of the same thing — one that also has to be excluded again
 * everywhere a read's OWN surface is stored, since no single read is ever
 * `any`.
 */
export type ReadSurface = "listing" | "detail" | "hosts";

/**
 * Count the page's READS of the surfaces the periodic loops used to poll.
 *
 * Only `GET`s are counted, and only of `/api/sessions` (the listing walk,
 * cursor pages included), `/api/sessions/{id}` (one session's detail) and
 * `/api/hosts`. Mutations are excluded on purpose: a stop or a rename is the
 * user acting, and a spec that counted those could never tell a busy page
 * from a polling one.
 */
export interface ReadCounter {
  /** How many reads of `surface` — or of ALL of them, when it is omitted —
   * have been seen since the counter was installed. Take one before a window
   * and one after; the difference is what happened during it. */
  count(surface?: ReadSurface): number;
  /** The URLs behind [`ReadCounter::count`], for a failure message that can
   * name what was read rather than only how much. */
  urls(surface?: ReadSurface): string[];
}

/** Which loop a URL belongs to, or `undefined` for a request that is not one
 * of the four reads (a mutation's path, an asset, the terminal socket). */
function readSurface(url: string): ReadSurface | undefined {
  const { pathname } = new URL(url);
  if (pathname === "/api/sessions") return "listing";
  if (pathname === "/api/hosts") return "hosts";
  // One session, and nothing under it: `/api/sessions/{id}/stop` and friends
  // are mutations, and `/api/sessions/{id}/tabs` is not a read this UI makes.
  if (/^\/api\/sessions\/[^/]+$/.test(pathname)) return "detail";
  return undefined;
}

export function countReads(page: Page): ReadCounter {
  const seen: { surface: ReadSurface; url: string }[] = [];
  page.on("request", (request) => {
    if (request.method() !== "GET") return;
    const surface = readSurface(request.url());
    if (surface) seen.push({ surface, url: request.url() });
  });
  const matching = (surface?: ReadSurface) =>
    seen.filter((read) => surface === undefined || read.surface === surface);
  return {
    count: (surface) => matching(surface).length,
    urls: (surface) => matching(surface).map((read) => read.url),
  };
}

/**
 * Make every API reply carry a build stamp this bundle disagrees with, so
 * the page latches skew (`farhelm-ui/src/skew.rs`).
 *
 * Rewriting the HEADER rather than standing up a second helm is the only
 * practical way to reach this state: the mismatch is between the compiled-in
 * bundle version and what the server says, and both sides of a real
 * disagreement would have to be built from different commits.
 *
 * The stamp must be visible ASCII, and that is the transport's rule rather
 * than this helper's: an HTTP header cannot carry arbitrary text, and a
 * value with anything else in it is refused when the client reads the header
 * — so the page sees NO stamp and latches a different verdict than the
 * caller intended. Anything hostile therefore has to reach the screen
 * through a JSON body instead (see m6-5-debts.spec.ts's peer-rendering pin).
 */
export async function forceBuildSkew(page: Page, stamp: string): Promise<void> {
  await page.route("**/api/**", async (route) => {
    const response = await route.fetch();
    const headers = { ...response.headers(), "x-farhelm-build": stamp };
    await route.fulfill({ response, headers });
  });
}

/**
 * Strip `seen_activity_at` from every row of every real listing reply, so
 * the row's actions menu never grows its conditional "mark read"/"mark
 * unread" item — everything else about the reply, including real
 * classification, is untouched.
 *
 * For the many menu-mechanics tests in `sidebar.spec.ts` that predate that
 * item and assert an exact, positional item list or a fixed arrow-key
 * sequence over `rename`/`clone`/`stop`/`archive`/`delete`: those tests are
 * about generic ARIA/keyboard mechanics, not about the seen-state feature
 * (which has its own tests), and their fixture sessions genuinely do reach
 * a live classification under enough real wall-clock time — the real
 * supervisor's sampler runs on its own clock, indifferent to how fast any
 * one test happens to complete. Under a loaded, `--repeat-each` run this
 * used to be fast enough to race past every one of them; it is not
 * guaranteed to stay that way, and it already stopped being true once. This
 * closes the race at its source (the field the row's menu ACTUALLY reads)
 * rather than trying to outrun a real classifier's own timing from the test
 * side, which nothing here controls.
 */
export async function hideSeenState(page: Page): Promise<void> {
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    // Node's own `fetch`, not `route.fetch()`/`APIResponse` — the latter
    // failed with "Response has been disposed" reading `.json()` on this
    // exact route (real cause not chased further; `forceBuildSkew` above
    // only ever reads `response.headers()`, never the body, which this
    // helper is the first caller here to need). A plain, independent HTTP
    // call to the same URL sidesteps whatever `APIResponse` lifecycle
    // that depended on.
    const upstream = await fetch(route.request().url(), {
      headers: await route.request().allHeaders(),
    });
    const body = await upstream.json();
    for (const session of body.sessions ?? []) {
      delete session.seen_activity_at;
    }
    // The forwarded headers describe the UPSTREAM body's bytes, not the
    // mutated one about to be sent — deleting a field only ever shrinks the
    // JSON, so a stale `content-length` would tell the browser to expect
    // more bytes than actually arrive. Dropped rather than trusted to `json`
    // to recompute, matching `holdReads`' own precedent for the same
    // stale-header risk on any reply whose body it replaces.
    const headers = Object.fromEntries(upstream.headers.entries());
    delete headers["content-length"];
    delete headers["content-encoding"];
    delete headers["transfer-encoding"];
    await route.fulfill({
      status: upstream.status,
      headers,
      json: body,
    });
  });
}

/** The helm's shared client preference as `GET /api/preferences` answers it. */
export interface Preferences {
  list_sort?: string;
  last_selected?: string;
}

/** Read the helm's shared preference row (SPEC.md, Session list). */
export async function readPreferences(request: APIRequestContext): Promise<Preferences> {
  const response = await request.get("/api/preferences");
  if (!response.ok()) throw new Error(`GET /api/preferences: ${response.status()}`);
  return (await response.json()) as Preferences;
}

/**
 * Write a sparse patch to the helm's shared preference: an absent field is
 * untouched, an explicit `null` clears it, a value replaces it.
 */
export async function patchPreferences(
  request: APIRequestContext,
  patch: { list_sort?: string | null; last_selected?: string | null },
): Promise<void> {
  const response = await request.put("/api/preferences", { data: patch });
  if (!response.ok()) throw new Error(`PUT /api/preferences: ${response.status()}`);
}

/**
 * Put the helm's shared preference back to "nothing remembered".
 *
 * The preference is ONE row on the helm shared by every client, and this
 * suite runs every spec against one helm: whatever the previous test chose
 * (an order, a selected row) is what the next test's page opens with. A
 * spec whose subject is the order or the auto-select must therefore reset
 * the row in `beforeEach` rather than inherit the last test's answer.
 */
export async function resetPreferences(request: APIRequestContext): Promise<void> {
  await patchPreferences(request, { list_sort: null, last_selected: null });
}

/**
 * Pin which session the page will auto-select on its next load.
 *
 * Auto-select (BUGS_BURNDOWN.md issue 5) opens the remembered session — the
 * helm's shared preference `last_selected`, one row for every client — falling
 * back to the newest-created non-archived one. Tests that stage route holds
 * or stubs around a SPECIFIC session's first reads must pin the selection
 * AWAY from that session (usually to the shared e2e-session) before `goto`,
 * or the auto-open races the staging exactly like a user clicking too early.
 *
 * Unlike the localStorage init script this replaced, the pin is not
 * re-applied per navigation: it is the helm's row, and a later user click in
 * the test (or in any other client) overwrites it — which is the product's
 * own "most recently selected anywhere" rule, not a harness quirk.
 */
export async function pinAutoSelect(page: Page, id: string): Promise<void> {
  await patchPreferences(page.request, { last_selected: id });
}

/**
 * The opposite of {@link pinAutoSelect}: guarantee the next load has NO
 * remembered selection.
 *
 * For the tests whose subject is the FALLBACK — what a client with nothing
 * remembered opens (SPEC.md's newest-created non-archived session). A
 * remembered id short-circuits that path entirely: the sidebar resolves it
 * against the helm and opens it, and the fallback the test came to check
 * never runs. Such a test has to state the precondition rather than inherit
 * it, because the row is shared: every earlier test that clicked a row left
 * its selection there.
 */
export async function forgetAutoSelect(page: Page): Promise<void> {
  await patchPreferences(page.request, { last_selected: null });
}

/**
 * Ensure the permanent host list's global details disclosure is open.
 *
 * The list itself is always mounted, so this helper preserves the useful
 * contract older call sites relied on: after it returns, evidence, remedies,
 * and provisioning state are available for inspection.
 */
export async function openHostsPanel(page: Page): Promise<void> {
  await expect(page.locator(".hosts-panel")).toBeVisible();
  const toggle = page.locator(".host-details-toggle");
  // Polled rather than a single read-then-click: the page opens details on
  // its own when automatic local setup needs a person (a plan, a manual
  // remedy, a probe error), and that reveal can land between this helper's
  // read and its click, turning the click into a CLOSE. Each poll clicks
  // only while the disclosure reads closed, so a reveal that raced the
  // first click is simply observed on the next look.
  await expect
    .poll(
      async () => {
        const expanded = await toggle.getAttribute("aria-expanded");
        if (expanded !== "true") await toggle.click();
        return toggle.getAttribute("aria-expanded");
      },
      { timeout: 20_000, intervals: [250, 500, 1000] },
    )
    .toBe("true");
}

/**
 * Open the session list's filter popover if it is not already open — the same
 * on-demand-toggle story as [`openHostsPanel`], for every test that
 * applies, clears, or inspects the session filter.
 */
export async function openFilterBar(page: Page): Promise<void> {
  const toggle = page.locator(".filter-toggle");
  if ((await toggle.getAttribute("aria-expanded")) !== "true") {
    await toggle.click();
  }
  await expect(page.locator(".filter-popover")).toBeVisible();
}

/**
 * Open a session row's actions menu when it is not already open.
 *
 * The sidebar redesign (BUGS_BURNDOWN.md issue 5) moved every per-row
 * action — rename, stop, archive, delete, and their confirms — off the row
 * and into a floating panel behind the row's `⋯` toggle, so any test that
 * clicks or asserts on those controls opens the menu first through this
 * helper.
 *
 * The `⋯` button is a genuine TOGGLE — list.rs's own `onclick` flips
 * `menu_open` unconditionally, closed→open and open→closed alike — so a
 * caller that clicked it blindly on an already-open menu would close it,
 * the opposite of this function's whole contract. The `aria-expanded`
 * check below is what makes this function idempotent instead: it reads
 * the toggle's own current truth and clicks ONLY when that truth is not
 * already `"true"`, so calling this on an already-open menu (a defensive
 * reopen after a background dismissal raced some earlier step, say) is a
 * safe no-op rather than an accidental close.
 *
 * The toggle is also hover-revealed (`opacity: 0` at rest, `opacity: 1` on
 * `.session-row:hover`/`:focus-within`/`.selected`, or while its own menu
 * is open — see app.css). `hover()` first is not strictly required for
 * `click()` to land — Playwright's actionability checks do not treat
 * `opacity: 0` as hidden, only `visibility`/`display` do — but every
 * caller here is standing in for a real mouse user, and a real one has to
 * hover the toggle before it is even visible to click. Doing it
 * explicitly, rather than relying on `click()`'s own incidental pointer
 * move, keeps this helper testing the same path a person takes instead of
 * a shortcut only available to automation.
 *
 * Hovers the TOGGLE, not the row's center: another row's already-open
 * menu panel is `position: fixed` (see `.session-row-menu-panel`'s own
 * comment in app.css for why) and can float directly over this row's
 * center point, which would make Playwright's actionability check land
 * the hover on the covering panel instead of this row. The toggle sits at
 * the row's trailing edge, clear of where a neighboring panel opens. Only
 * hovered when about to open it, too — an already-open menu has no
 * reason to be re-hovered, and its toggle may itself now sit under its
 * own panel depending on placement.
 */
/**
 * The renderer workaround shared by `openRowMenu` and `openHostMenu`:
 * click a row's "⋯" toggle open (idempotently) and wait for its floating
 * panel to finish the async toggle-rect measurement `menu_panel.rs` races
 * against the render that opened it. Both row kinds build their panel on
 * that identical, generically shared mechanism, so this is the one place
 * that workaround is written — the two exported functions below are thin,
 * selector-only wrappers that keep the readable, row-specific call sites
 * `openRowMenu(row)` / `openHostMenu(row)`.
 *
 * `toggleSelector`/`panelSelector` name the row's own toggle and floating
 * panel (`.session-row-menu`/`.session-row-menu-panel` for a session row,
 * `.host-row-menu`/`.host-row-menu-panel` for a host row); `waitMessage` is
 * the row-specific wording surfaced if the poll below times out.
 *
 * The toggle is a genuine TOGGLE — the row's own `onclick` flips
 * `menu_open` unconditionally, closed→open and open→closed alike — so
 * clicking it blindly on an already-open menu would close it, the
 * opposite of either exported function's contract. The `aria-expanded`
 * check below is what makes this idempotent instead: it reads the
 * toggle's own current truth and clicks ONLY when that truth is not
 * already `"true"`, so calling this on an already-open menu (a defensive
 * reopen after a background dismissal raced some earlier step, say) is a
 * safe no-op rather than an accidental close.
 *
 * The toggle is also hover-revealed (`opacity: 0` at rest, `opacity: 1` on
 * hover, keyboard focus within the row, or while its own menu is open —
 * see app.css). `hover()` first is not strictly required for `click()` to
 * land — Playwright's actionability checks do not treat `opacity: 0` as
 * hidden, only `visibility`/`display` do — but every caller here is
 * standing in for a real mouse user, and a real one has to hover the
 * toggle before it is even visible to click. Doing it explicitly, rather
 * than relying on `click()`'s own incidental pointer move, keeps this
 * helper testing the same path a person takes instead of a shortcut only
 * available to automation.
 *
 * Hovers the TOGGLE, not the row's center: another row's already-open menu
 * panel is `position: fixed` (see `.session-row-menu-panel`'s own comment
 * in app.css for why) and can float directly over this row's center point,
 * which would make Playwright's actionability check land the hover on the
 * covering panel instead of this row. The toggle sits at the row's
 * trailing edge, clear of where a neighboring panel opens. Only hovered
 * when about to open it, too — an already-open menu has no reason to be
 * re-hovered, and its toggle may itself now sit under its own panel
 * depending on placement.
 */
async function openMenuPanel(
  row: Locator,
  toggleSelector: string,
  panelSelector: string,
  waitMessage: string,
): Promise<void> {
  const menu = row.locator(toggleSelector);
  if ((await menu.getAttribute("aria-expanded")) !== "true") {
    await menu.hover();
    await menu.click();
  }
  // Await the panel itself, not just the click: the toggle's signal write
  // and the panel's mount land on a LATER render, and several callers go
  // straight into bare-DOM `querySelector(...).click()` calls (the
  // actionability-bypass tests), where a not-yet-mounted button turns
  // into a silent no-op via `?.click()` rather than a visible failure.
  const panel = row.locator(panelSelector);
  await expect(panel).toBeVisible();
  // `toBeVisible()` alone is not the MEASURED state: the panel mounts the
  // instant the toggle opens, at `opacity: 0; pointer-events: none` —
  // genuinely present and painted-nothing (`PanelPlacement::Unmeasured` in
  // menu_panel.rs, while the toggle's own async `get_client_rect()`
  // measurement is still in flight) — and Playwright's visibility check
  // does not consult opacity or pointer-events at all, so a caller reading
  // this panel's geometry immediately after this function used to return
  // could be racing that measurement.
  //
  // Detected by reading the panel's own inline `style` for a literal
  // `left: auto` — the one substring ONLY `PanelPlacement::Measured` ever
  // writes (`Unmeasured` sets no `left` at all; `Fallback`, the renderer-
  // could-not-measure-at-all state, sets a literal `left: 8px` instead) —
  // rather than by comparing the panel's box to the toggle's own. Geometry
  // would be the wrong test here: a genuinely MEASURED panel can still be
  // clamped far from its toggle on a short viewport (`menu_panel.rs`'s
  // `menu_panel_style` — precisely what the clipping regression test in
  // sidebar.spec.ts exercises), so a box-proximity check would either
  // reject a real measured-and-clamped panel or, worse, accept `Fallback`
  // coordinates that happen to fall near the toggle by coincidence. The
  // style string has no such ambiguity: every non-`Measured` state is
  // structurally incapable of containing it.
  await expect
    .poll(
      async () => {
        const info = await panel.evaluate((el) => ({
          opacity: getComputedStyle(el).opacity,
          style: el.getAttribute("style") ?? "",
        }));
        return info.opacity === "1" && info.style.includes("left: auto");
      },
      { message: waitMessage },
    )
    .toBe(true);
}

/**
 * Open a session row's actions menu when it is not already open.
 *
 * The sidebar redesign (BUGS_BURNDOWN.md issue 5) moved every per-row
 * action — rename, stop, archive, delete, and their confirms — off the row
 * and into a floating panel behind the row's `⋯` toggle, so any test that
 * clicks or asserts on those controls opens the menu first through this
 * helper. See `openMenuPanel`'s own doc for the toggle-idempotence,
 * hover-before-click, and measured-vs-unmeasured mechanics this drives.
 */
export async function openRowMenu(row: Locator): Promise<void> {
  await openMenuPanel(
    row,
    ".session-row-menu",
    ".session-row-menu-panel",
    "waiting for the actions panel to finish measuring against its own toggle",
  );
}

/**
 * Open a host row's actions menu when it is not already open.
 *
 * The host row's own "⋯" menu folds `retry`/`adopt`/`edit destination`/
 * `remove` off `.host-row-main` into a floating panel
 * behind a `.host-row-menu` toggle — built on the identical mechanics
 * `openRowMenu` above already drives for the session row's menu (shared
 * generically in `menu_panel.rs`; see `openMenuPanel`'s own doc for the
 * toggle-idempotence, hover-before-click, and measured-vs-unmeasured
 * mechanics both wrappers drive). This is the fix for TODO.md's
 * now-closed near-term entry: `remove` used to render clipped off the
 * sidebar's right edge on an ssh host, invisible and unclickable, and
 * every spec that clicks or asserts on a host row's management verbs now
 * has to open this menu first — exactly the discipline `openRowMenu`
 * already imposes for the session row.
 */
export async function openHostMenu(row: Locator): Promise<void> {
  await openMenuPanel(
    row,
    ".host-row-menu",
    ".host-row-menu-panel",
    "waiting for the host actions panel to finish measuring against its own toggle",
  );
}

/**
 * Whether passwordless `ssh localhost` works, probed DIRECTLY rather than
 * inferred from the helm.
 *
 * Independence is the whole value: inferring it from "the ssh host never
 * reached connected" conflates the one condition a fleet suite may skip for
 * with every condition it must not — a broken transport, a supervisor that
 * will not start, a helm that mis-registers the ensure file. Those are bugs
 * those suites exist to catch, and a fleet probe that treats them as "no
 * self-ssh here" reports them as a skip.
 *
 * The options mirror the Rust suite's own probe exactly: `BatchMode=yes` so
 * every interactive fallback fails instead of hanging, and
 * `StrictHostKeyChecking=yes` rather than `accept-new` because a test suite
 * must not write to the developer's `known_hosts`.
 *
 * Shared rather than copied per suite because the answer must not differ
 * between them: two probes that drifted in their ssh options would have one
 * suite skip where the other ran, which reads as a flake rather than as the
 * configuration difference it is. What each suite still owns is what to DO
 * with the answer — the skip text, and whether a missing fleet is a failure
 * in CI — because that depends on what the suite is for.
 *
 * Not `async`: it builds and returns one promise, and an `async` wrapper
 * around `return await` only adds a microtask tick and an extra frame.
 */
export function selfSshAvailable(): Promise<boolean> {
  return new Promise<boolean>((resolve) => {
    const probe = spawn(
      "ssh",
      ["-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes", "-o", "ConnectTimeout=10", "localhost", "true"],
      { stdio: "ignore" },
    );
    probe.on("error", () => resolve(false));
    probe.on("exit", (code) => resolve(code === 0));
  });
}
