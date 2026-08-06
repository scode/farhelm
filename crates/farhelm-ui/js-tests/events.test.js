// Unit coverage for events.js — the invalidation feed's socket (PLAN_M6_75.md
// item 6), run with node's built-in test runner for the reasons
// term-bytes.test.js's header gives (node is already a CI requirement for
// Playwright; one asset file does not earn a bundler).
//
// ## Why this file exists at all
//
// The browser suite drives the feed end to end, but only along paths a real
// helm produces. The failures that matter most here are the ones a healthy
// helm never creates — a frame that cannot be read, a socket that opens and
// then says nothing — and both are silent by nature: the page keeps its feed
// marked healthy, keeps its fallback poll switched off, and stops updating.
// A test that has to arrange a lying server to see that is a test nobody
// writes; a test that hands the file a fake socket is three lines.
//
// ## Two harnesses, deliberately
//
// The PURE decisions (decoding a frame, picking a delay, building the URL)
// are `require`d directly, which exercises events.js's CommonJS branch. The
// STATEFUL half — the ladder, the withdrawal latch, the handshake deadline —
// is run through `node:vm` with a fake `WebSocket` and a fake clock, which is
// the only way to observe "what did it schedule, and for how long" without
// waiting in real time. Both run the file's ACTUAL source rather than a
// re-typed copy, so deleting the behavior under test fails the test.
const test = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const fs = require("node:fs");
const path = require("node:path");
const { decodeReport, delayFor, feedUrl } = require("../assets/events.js");

const SOURCE = fs.readFileSync(path.join(__dirname, "../assets/events.js"), "utf8");

/**
 * The identity `loadIsland`'s subscription is minted with.
 *
 * Every subscription carries one (farhelm-ui/src/feed.rs mints them from a
 * counter); the tests use small literals so a stale cleanup can be aimed at
 * a token that is no longer live.
 */
const FIRST_TOKEN = 1;

/** The Rust-side policy, as `feed::feed_policy` builds it. */
const POLICY = {
  path: "/api/events",
  delaysMs: [500, 1000, 2000, 4000, 8000, 15000],
  probeIntervalMs: 30000,
  handshakeMs: 10000,
};

/**
 * A clock whose timers only fire when a test says so.
 *
 * Real timers would make every assertion below a race and every wait a real
 * second. What the tests actually want to know is WHICH timer was scheduled
 * and for how long — the ladder rung, the handshake deadline — which is
 * exactly what a fake clock can answer and a real one cannot.
 */
function fakeClock() {
  let next = 0;
  const timers = new Map();
  return {
    setTimeout(fn, ms) {
      next += 1;
      timers.set(next, { fn: fn, ms: ms });
      return next;
    },
    clearTimeout(id) {
      timers.delete(id);
    },
    /** Every timer still armed, in the order they were scheduled. */
    pending() {
      return Array.from(timers.entries()).map(([id, timer]) => ({ id: id, ms: timer.ms }));
    },
    /** Fire the single armed timer, asserting there is exactly one. */
    fireOnly() {
      const armed = Array.from(timers.entries());
      assert.equal(armed.length, 1, "expected exactly one armed timer");
      const [id, timer] = armed[0];
      timers.delete(id);
      timer.fn();
      return timer.ms;
    },
  };
}

/**
 * Load events.js into a fresh context with a fake socket and a fake clock,
 * and return the handles a test needs to drive it.
 *
 * `withdrawnUpFront` seeds `window.farhelmFeedWithdrawn` BEFORE the file
 * executes, which is the ordering the withdrawal latch exists for: the Rust
 * side can stand the page down before this file has been injected at all.
 */
function loadIsland(options) {
  const clock = fakeClock();
  const sockets = [];
  function FakeSocket(url) {
    this.url = url;
    this.closed = false;
    this.onmessage = null;
    this.onclose = null;
    this.onerror = null;
    sockets.push(this);
  }
  FakeSocket.prototype.close = function () {
    this.closed = true;
  };
  const sandbox = {
    window: {},
    WebSocket: FakeSocket,
    setTimeout: clock.setTimeout,
    clearTimeout: clock.clearTimeout,
  };
  if (options && options.withdrawnUpFront) sandbox.window.farhelmFeedWithdrawn = true;
  vm.createContext(sandbox);
  vm.runInContext(SOURCE, sandbox);
  // The OTHER ordering the latch has to survive: the file has executed and
  // registered its island, and the withdrawal lands before anything
  // subscribes. Set here rather than before the run so this is genuinely a
  // different case from `withdrawnUpFront` — one exercises the read at
  // registration, this one the read at subscribe.
  if (options && options.withdrawnBeforeSubscribe) sandbox.window.farhelmFeedWithdrawn = true;

  // Copied into THIS realm on the way in. Objects the sandbox constructs
  // carry the sandbox's own `Object.prototype` (every `vm` context gets its
  // own built-ins), so `assert/strict`'s reference check on the prototype
  // chain would reject a byte-identical report — a realm mismatch, not a
  // difference in what was sent. The spread rebuilds each report locally,
  // which is the same dodge term-bytes.test.js makes with `Array.from`.
  const reports = [];
  const parked = sandbox.window.farhelmEvents.subscribe(
    "http://127.0.0.1:7433",
    POLICY,
    (report) => reports.push({ ...report }),
    FIRST_TOKEN,
  );
  return {
    events: sandbox.window.farhelmEvents,
    window: sandbox.window,
    clock: clock,
    sockets: sockets,
    reports: reports,
    parked: parked,
  };
}

// ---------------------------------------------------------------------
// The pure half
// ---------------------------------------------------------------------

test("a well-formed revision frame decodes into the report Rust expects", () => {
  // The exact shape `feed::FeedReport` deserializes, asserted as an object
  // rather than round-tripped: this is a wire contract with a Rust file the
  // JS toolchain never sees, so a renamed key has to fail HERE.
  assert.deepEqual(decodeReport('{"revision":12}'), { kind: "revision", revision: 12 });
  assert.deepEqual(decodeReport('{"revision":0,"extra":"ignored"}'), {
    kind: "revision",
    revision: 0,
  });
});

test("a frame this build cannot read decodes to null rather than to a report", () => {
  // Every one of these means the channel is not carrying what the client
  // thinks it is. `null` is what makes the caller distrust the socket — see
  // the outage test below for the half that matters.
  assert.equal(decodeReport("not json at all"), null, "unparseable");
  assert.equal(decodeReport("{}"), null, "no revision at all");
  assert.equal(decodeReport('{"revision":"12"}'), null, "a string is not a revision");
  assert.equal(decodeReport('{"revision":null}'), null, "null is not a revision");
  assert.equal(decodeReport("[1,2,3]"), null, "an array carries no revision");
  // `JSON.parse` cannot produce Infinity, but a future sender could hand
  // this function a value some other way; a non-finite revision is not a
  // number anyone can act on.
  assert.equal(decodeReport('{"revision":1e400}'), null, "an overflowing number is not finite");
});

test("a revision Rust could not decode is rejected here rather than sent on", () => {
  // The boundary is the RUST type, not JavaScript's idea of a number:
  // `feed::FeedReport` takes a u64, so each of these fails deserialization
  // on the other side of the bridge — and a decode failure there is
  // indistinguishable from the bridge dying, which retires the subscription
  // task for the life of the page. Passing such a frame on would turn a
  // malformed message into a permanently dead feed, skipping the outage path
  // entirely. That is why these belong with the unreadable frames above
  // rather than being waved through as "close enough to a number".
  assert.equal(decodeReport('{"revision":-1}'), null, "negative: no u64 holds it");
  assert.equal(decodeReport('{"revision":-0.5}'), null, "negative and fractional");
  assert.equal(decodeReport('{"revision":1.5}'), null, "fractional: not an integer at all");
  assert.equal(
    decodeReport('{"revision":9007199254740992}'),
    null,
    "2^53 and beyond cannot be represented faithfully here, so the number is already a guess",
  );

  // And the values that ARE legal stay legal, boundaries included: zero is
  // the revision a fresh helm reports, and 2^53-1 is the largest integer
  // this side can carry without lying about it.
  assert.deepEqual(decodeReport('{"revision":0}'), { kind: "revision", revision: 0 });
  assert.deepEqual(decodeReport('{"revision":9007199254740991}'), {
    kind: "revision",
    revision: 9007199254740991,
  });
});

test("the delay ladder is consumed once and then gives way to probing forever", () => {
  // The two regimes are the Rust side's (feed::feed_policy), and this pins
  // that the file consumes them as intended: the ladder covers the ordinary
  // blip, and FALLING OFF it is the normal steady state rather than an error
  // — a feed that comes back overnight has to resubscribe with nobody
  // watching.
  const rungs = [0, 1, 2, 3, 4, 5].map((attempt) => delayFor(POLICY, attempt));
  assert.deepEqual(rungs, POLICY.delaysMs);
  assert.equal(delayFor(POLICY, 6), POLICY.probeIntervalMs);
  assert.equal(delayFor(POLICY, 600), POLICY.probeIntervalMs);
  // A policy with no ladder at all still probes rather than returning
  // undefined, which would schedule a zero-delay timer and spin.
  assert.equal(delayFor({ probeIntervalMs: 30000 }, 0), 30000);
});

test("the feed URL swaps the scheme and keeps the origin", () => {
  // Both schemes through one substitution, the same trick terminal.js uses.
  // The desktop renderer is why `base` is passed at all: its webview origin
  // is not the helm's.
  assert.equal(feedUrl("http://127.0.0.1:7433", "/api/events"), "ws://127.0.0.1:7433/api/events");
  assert.equal(feedUrl("https://helm.example", "/api/events"), "wss://helm.example/api/events");
});

// ---------------------------------------------------------------------
// The stateful half
// ---------------------------------------------------------------------

test("a message resets the ladder; consecutive failures walk it", () => {
  // The attempt counter is reset by a MESSAGE, never by a socket opening —
  // an open socket that says nothing is exactly what a wedged helm looks
  // like. This pins both directions: failures climb, and one good frame puts
  // the next outage back on the first rung.
  const island = loadIsland();
  assert.equal(island.sockets.length, 1);

  island.sockets[0].onclose();
  assert.deepEqual(island.reports, [{ kind: "down" }]);
  assert.equal(island.clock.fireOnly(), 500, "the first retry is the ladder's first rung");
  island.sockets[1].onclose();
  assert.equal(island.clock.fireOnly(), 1000, "a second failure climbs");

  island.sockets[2].onmessage({ data: '{"revision":7}' });
  assert.deepEqual(island.reports, [{ kind: "down" }, { kind: "revision", revision: 7 }]);
  island.sockets[2].onclose();
  assert.equal(
    island.clock.fireOnly(),
    500,
    "a connection that proved itself starts the next outage over",
  );
});

test("an unreadable frame costs the socket its trust", () => {
  // The finding this pins: dropping a frame quietly leaves the page
  // believing in a feed it can no longer read, with its fallback poll off.
  // If that frame was the notice for the last mutation anyone makes, the
  // page is stale forever and says nothing. Distrusting the socket routes
  // the failure through the ordinary outage path, so Rust turns the fallback
  // back on and the ladder reconnects.
  const island = loadIsland();
  island.sockets[0].onmessage({ data: "<html>proxy error</html>" });

  assert.deepEqual(island.reports, [{ kind: "down" }], "the outage is reported to Rust");
  assert.ok(island.sockets[0].closed, "and the socket it came from is closed");
  assert.equal(island.clock.fireOnly(), 500, "the ladder starts climbing");
  assert.equal(island.sockets.length, 2, "which reconnects");
});

test("a socket that opens and never greets is given up on", () => {
  // The helm answers every subscription with the current revision at once,
  // so silence here is not slowness — it is an upgrade that was accepted by
  // something that then stopped serving it. Without the deadline that
  // connection parks forever with the ladder suspended and no outage ever
  // reported: the worst state available, because the page looks connected.
  const island = loadIsland();
  assert.deepEqual(
    island.clock.pending(),
    [{ id: 1, ms: POLICY.handshakeMs }],
    "the deadline is armed from the attempt, so a connect that never completes is covered too",
  );

  island.clock.fireOnly();
  assert.deepEqual(island.reports, [{ kind: "down" }]);
  assert.ok(island.sockets[0].closed);
  assert.equal(island.clock.fireOnly(), 500, "and the attempt goes on the ladder like any other");
});

test("a greeting cancels the deadline for that socket", () => {
  // The complement of the test above, and the one that keeps the deadline
  // from being a three-hour cap on a working subscription: a feed that has
  // greeted is under no further obligation to speak, since a quiet fleet is
  // the ordinary case.
  const island = loadIsland();
  island.sockets[0].onmessage({ data: '{"revision":1}' });
  assert.deepEqual(island.clock.pending(), [], "nothing is left waiting on this socket");
  assert.deepEqual(island.reports, [{ kind: "revision", revision: 1 }]);
});

test("a withdrawal that predates this file still binds the subscription", () => {
  // The race the latch exists for, in its hardest ordering: the Rust side
  // latches a build mismatch on the very first reply and withdraws the feed
  // before events.js has been injected at all. A withdrawal that only called
  // `stop()` would find nothing to stop, and the subscription would open a
  // moment later on a page that had already stood down — the one behavior
  // SPEC_impl.md's withdrawal rule exists to revoke.
  const island = loadIsland({ withdrawnUpFront: true });
  assert.equal(island.sockets.length, 0, "no socket is opened");
  assert.deepEqual(island.reports, [], "and nothing is reported");
  return island.parked; // Settles at once, which is what ends the Rust snippet.
});

test("a withdrawal landing between load and subscribe still binds", () => {
  // The gap the load-time read cannot see: the island is registered and
  // waiting, and the Rust side latches the mismatch before its subscription
  // task gets its turn. A latch consulted only at load would be stale by
  // then, and the page would open a socket it had already been told to give
  // up — the same unsupervised subscription the withdrawal rule revokes,
  // reached through a different door.
  const island = loadIsland({ withdrawnBeforeSubscribe: true });
  assert.equal(island.sockets.length, 0, "no socket is opened");
  assert.deepEqual(island.reports, []);
  assert.deepEqual(island.clock.pending(), [], "and nothing is scheduled");
  return island.parked;
});

test("releasing a subscription ends the socket without withdrawing the feed", () => {
  // What the Rust side does when its channel to this island dies: the socket
  // has to go — nobody is reading its reports, it holds a helm-side
  // subscriber, and its handshake timer would keep climbing a ladder into a
  // closed channel — but the page has NOT stood down, so a later mount must
  // be able to subscribe again. Latching here would retire the feed for the
  // life of the page over a failure that says nothing about the helm.
  const island = loadIsland();
  island.events.release(FIRST_TOKEN);

  assert.ok(island.sockets[0].closed, "the socket is closed");
  assert.deepEqual(island.reports, [], "and no outage is claimed for it");
  assert.deepEqual(island.clock.pending(), [], "no timer is left running");
  assert.equal(
    island.window.farhelmFeedWithdrawn,
    undefined,
    "a dead bridge is not a withdrawal",
  );

  const again = island.events.subscribe("http://127.0.0.1:7433", POLICY, () => {}, 2);
  assert.equal(island.sockets.length, 2, "so a later subscribe opens a fresh socket");
  island.events.stop();
  return Promise.all([island.parked, again]);
});

test("a stale cleanup cannot tear down the subscription that replaced it", () => {
  // The ordering this forbids: a subscription's task dies, its cleanup is
  // queued as an eval, and a remount installs a fresh subscription before
  // that eval lands. An unscoped release would then close the socket the
  // page is actually using — and nothing would ever open another, because
  // the mount that would have has already happened. The page would sit on
  // its fallback poll for the rest of its life with no sign of why.
  const island = loadIsland();
  const replacement = island.events.subscribe("http://127.0.0.1:7433", POLICY, () => {}, 2);
  assert.equal(island.sockets.length, 2, "the replacement supersedes the first subscription");
  assert.ok(island.sockets[0].closed, "which ends the one it replaced");

  island.events.release(FIRST_TOKEN);

  assert.equal(
    island.sockets[1].closed,
    false,
    "the late cleanup names a subscription that is gone, so it must do nothing",
  );
  island.events.stop();
  return Promise.all([island.parked, replacement]);
});

test("stopping a live subscription is silent and permanent", () => {
  // Silent because the feed did not fail: the page withdrew it, and a `down`
  // report would put Rust on its fallback poll — which is precisely what the
  // withdrawal is refusing to do. Permanent because the mismatch behind it
  // is latched for the life of the page.
  const island = loadIsland();
  island.events.stop();

  assert.ok(island.sockets[0].closed);
  assert.deepEqual(island.reports, [], "no outage is claimed for a deliberate stop");
  assert.deepEqual(island.clock.pending(), [], "and nothing is left scheduled");
  assert.equal(
    island.window.farhelmFeedWithdrawn,
    true,
    "the latch is mirrored onto the global both sides read",
  );

  const again = island.events.subscribe("http://127.0.0.1:7433", POLICY, () => {}, 2);
  assert.equal(island.sockets.length, 1, "a later subscribe opens nothing");
  return Promise.all([island.parked, again]);
});
