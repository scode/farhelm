// The invalidation feed's socket (PLAN_M6_75.md item 6): the JS half of the
// channel that replaced all four of the UI's periodic loops.
//
// Same division of labour as terminal.js, drawn in the same place and for
// the same reason: a WebSocket lives on a side of the boundary neither
// renderer can reach across, while the DECISIONS around it — how long to
// wait between attempts, when the active window is spent, whether polling
// should take over — are Rust's, reviewable and unit-tested there
// (farhelm-ui/src/feed.rs). This file therefore contains no numbers of its
// own: the ladder and the probe interval arrive in the policy object, and
// what it does with a dropped socket is report it and keep trying.
//
// Loaded as a plain script alongside the terminal islands (see terminal.js's
// header). Registration order is not execution order, so the Rust side waits
// for `window.farhelmEvents` to exist rather than assuming it.
//
// ## The subscription parks, and that is not an accident
//
// `subscribe` returns a promise that settles only when the subscription is
// superseded or stopped, and the caller is expected to await it. Dioxus
// closes the Rust↔JS channel the moment an evaluated snippet's own promise
// resolves, so a snippet that installed this callback and returned would
// close the channel out from under the callback it had just installed. The
// snippet's lifetime has to BE the subscription's lifetime, which is what
// the parked promise gives it.
//
// ## What is sent back, and what is deliberately not
//
// Two reports, both objects tagged with `kind`:
//
//   {"kind": "revision", "revision": N}   a notification arrived
//   {"kind": "down"}                      the socket is gone; retrying
//
// The handshake — the current revision the helm sends immediately on every
// (re)subscription — is reported as an ordinary `revision`, deliberately
// indistinguishable from a later bump. The server refuses to mark them apart
// for the same reason (farhelm-helm's events.rs): the client's correct
// answer to either is to re-read, and a distinguishable greeting would only
// invite someone to treat them differently and lose the fallback handover
// the handshake exists to make safe.
//
// Nothing is ever SENT on the socket. The feed has no client vocabulary at
// all, and the helm closes a subscription that says anything.
//
// ## Testability
//
// The decisions here that are pure — decoding a frame, picking a delay,
// building the URL — are exported under CommonJS as well as installed on
// `window`, exactly as term-bytes.js is, so `js-tests/events.test.js` can
// assert them directly instead of inferring them from a browser run. Both
// installs are guarded on the environment having the thing they install
// into: under `require()` there is no `window`, and in a browser there is no
// `module`. Everything stateful (the socket, the ladder, the withdrawal
// latch) is exercised in the same tests through `node:vm` with a fake
// `WebSocket` and fake timers — see that file's header.
(function () {
  const DEVICE_SECRET_KEY = "farhelm.device-secret";

  /**
   * Protocols for an authenticated browser upgrade.
   *
   * The secret is read at each attempt rather than captured at subscription:
   * token rotation keeps the same retrying feed object alive while the auth
   * surface replaces its localStorage value.
   */
  function deviceProtocols() {
    try {
      const secret = window.localStorage.getItem(DEVICE_SECRET_KEY);
      return secret ? ["farhelm", `farhelm-device-${secret}`] : ["farhelm"];
    } catch (_error) {
      return ["farhelm"];
    }
  }

  /**
   * The one live subscription, or null. Singular by construction: a second
   * `subscribe` supersedes the first rather than running beside it, so the
   * page can never hold two sockets whose reports interleave into one
   * channel.
   */
  var live = null;

  /**
   * Whether the page has WITHDRAWN the feed, as opposed to simply not having
   * subscribed yet.
   *
   * The latch lives on `window` and NOWHERE ELSE, deliberately: a local copy
   * beside it would be a second source of truth for one fact, and the two
   * can be written in either order. It exists to close a race rather than to
   * save work — the Rust side subscribes from a task and withdraws from an
   * effect, and nothing orders those two against each other, so a build
   * mismatch latched on the very first reply can withdraw the feed before
   * `subscribe` has run. Worse, this whole file is injected asynchronously,
   * so a withdrawal can be evaluated before `window.farhelmEvents` exists at
   * all and have no `stop` to call. The Rust side therefore writes the
   * global FIRST and calls `stop()` only if the island happens to be there
   * already (farhelm-ui/src/feed.rs); reading it here at every `subscribe`
   * catches whichever order the two sides ran in.
   *
   * Permanent, matching what it mirrors: the mismatch it answers to is
   * itself latched for the life of the page (farhelm-ui/src/skew.rs), and
   * the remedy for both is the same reload.
   *
   * Guarded on `window` existing so this file can be `require`d by the unit
   * tests, where it is not.
   */
  function withdrawn() {
    return typeof window !== "undefined" && window.farhelmFeedWithdrawn === true;
  }


  /**
   * The feed's WebSocket URL for an API base.
   *
   * `base` is the helm's absolute origin — the page's own on web, and the
   * embedded bootstrap's on desktop, where the webview's origin is NOT the helm.
   * Swapping the scheme's `http` prefix covers both `http` and `https` in
   * one substitution, which is the same trick terminal.js uses for the
   * terminal sockets.
   *
   * @param {string} base - absolute origin, e.g. `http://127.0.0.1:7433`.
   * @param {string} path - the feed route, from the Rust-side policy.
   */
  function feedUrl(base, path) {
    var origin = base && base.length > 0 ? base : location.origin;
    return origin.replace(/^http/, "ws") + path;
  }

  /**
   * How long to wait before attempt number `attempt` (zero-based).
   *
   * The two regimes are the policy's, not this file's: the ladder covers the
   * ordinary blip — a helm restart, a wifi handover — and everything past it
   * is unbounded low-frequency probing, so a feed that comes back overnight
   * resubscribes with nobody watching. Falling off the end of the ladder is
   * therefore the NORMAL steady state rather than a failure to handle.
   */
  function delayFor(policy, attempt) {
    var ladder = policy.delaysMs || [];
    if (attempt < ladder.length) return ladder[attempt];
    return policy.probeIntervalMs;
  }

  /**
   * Stop both of a subscription's timers: the retry wait and the handshake
   * deadline.
   *
   * Together, because every path that ends an attempt ends both — a socket
   * that is being replaced has no deadline left to enforce, and one being
   * retried is not waiting on anything yet.
   */
  function clearTimers(sub) {
    if (sub.timer !== null) {
      clearTimeout(sub.timer);
      sub.timer = null;
    }
    if (sub.greeting !== null) {
      clearTimeout(sub.greeting);
      sub.greeting = null;
    }
  }

  /**
   * Drop one socket's handlers and close it.
   *
   * Handlers are cleared BEFORE the close so a close WE perform cannot come
   * back through `onclose` as an outage — the difference between "the page
   * withdrew the feed" and "the feed died" is exactly what the Rust side's
   * fallback rule turns on. No `onopen` is cleared because none is ever
   * assigned: this file deliberately treats an open socket as evidence of
   * nothing (see `connect`).
   */
  function discard(socket) {
    socket.onmessage = null;
    socket.onclose = null;
    socket.onerror = null;
    try {
      socket.close();
    } catch (e) {
      // A socket that is already closing throws on some engines; there is
      // nothing left to do about it either way.
    }
  }

  /**
   * Tear one subscription down: stop its timers, drop its socket, and settle
   * its parked promise.
   */
  function teardown(sub) {
    if (sub.stopped) return;
    sub.stopped = true;
    clearTimers(sub);
    var socket = sub.socket;
    sub.socket = null;
    if (socket) discard(socket);
    sub.settle();
  }

  /** Report the socket's loss once per outage, then schedule the retry. */
  function lost(sub) {
    if (sub.stopped) return;
    clearTimers(sub);
    if (sub.socket) {
      var socket = sub.socket;
      sub.socket = null;
      discard(socket);
    }
    if (!sub.down) {
      sub.down = true;
      sub.send({ kind: "down" });
    }
    var wait = delayFor(sub.policy, sub.attempt);
    sub.attempt += 1;
    sub.timer = setTimeout(function () {
      sub.timer = null;
      connect(sub);
    }, wait);
  }

  /**
   * One frame's payload as a report for the Rust side, or `null` if this
   * build cannot read it.
   *
   * `null` is a strong statement here, not a shrug, and the caller acts on
   * it by distrusting the socket. The helm sends exactly one shape on this
   * channel — a JSON object with a `revision` that is a nonnegative integer
   * — so a frame that does not parse, or parses to something else, means the
   * thing on the other end is not the feed this client thinks it is talking
   * to: a proxy injecting its own message, a helm whose stream has
   * desynchronized, a socket connected somewhere unintended. Dropping such a
   * frame quietly is the dangerous option, because a dropped frame may have
   * been the notice for the last mutation that will ever happen — the page
   * would then sit permanently stale with a feed it still believes in.
   *
   * The number is checked against what the RUST side can hold, not merely
   * against being a number. `feed::FeedReport` decodes `revision` as a
   * `u64`, so a negative, fractional or unsafe-integer value fails
   * deserialization there — and a decode failure on that channel is
   * indistinguishable from the bridge dying, which retires the subscription
   * task PERMANENTLY. Letting such a frame through would therefore convert a
   * malformed message into a page that never receives another notification,
   * bypassing the very outage path this function exists to route it into.
   * `Number.isSafeInteger` is the exact boundary: above 2^53-1 an integer
   * cannot be represented faithfully here anyway, so a "revision" past it
   * was never going to mean what it says.
   */
  function decodeReport(data) {
    var revision;
    try {
      revision = JSON.parse(data).revision;
    } catch (e) {
      return null;
    }
    if (!Number.isSafeInteger(revision) || revision < 0) return null;
    return { kind: "revision", revision: revision };
  }

  /**
   * Open one socket, wire its handlers, and put it on the clock.
   *
   * The attempt counter is reset by a MESSAGE rather than by the socket
   * opening, and that distinction is load-bearing: an open socket that never
   * says anything is indistinguishable from a helm that accepted the upgrade
   * and then wedged, and treating that as success would walk the ladder back
   * to its fastest rung on every such attempt. The helm's handshake makes a
   * healthy subscription say something immediately, so "a message arrived"
   * is the honest evidence that this connection works.
   *
   * The handshake deadline is the other half of that argument, and what it
   * protects is the LADDER. Refusing to call an open socket healthy already
   * keeps the Rust side honest — it marks the feed healthy on a delivered
   * frame and nothing else, so a silent socket leaves the page on its
   * fallback poll rather than blind. What the page loses without a deadline
   * is the feed itself: this file would sit on that open socket forever with
   * no outage to react to, never reconnecting, so a subscription lost to a
   * wedged proxy would never come back and the page would poll for the rest
   * of its life. Expiry hands the attempt to the ordinary outage path, which
   * reconnects and gets push delivery back.
   *
   * The timer is armed from the ATTEMPT rather than from `onopen` (which is
   * why no `onopen` handler exists at all), so it also covers a connect that
   * never completes: some engines and some networks will sit on a dead TCP
   * connection for minutes without ever reporting an error.
   */
  function connect(sub) {
    if (sub.stopped) return;
    var socket;
    try {
      socket = new WebSocket(feedUrl(sub.base, sub.policy.path), deviceProtocols());
    } catch (e) {
      // A malformed URL or a blocked scheme: the same outage as any other,
      // handled by the same ladder rather than by a special case.
      lost(sub);
      return;
    }
    sub.socket = socket;
    if (sub.policy.handshakeMs > 0) {
      sub.greeting = setTimeout(function () {
        sub.greeting = null;
        if (sub.stopped || sub.socket !== socket) return;
        // Never greeted: treated as any other outage, so the page falls back
        // to polling and this attempt goes on the ladder.
        lost(sub);
      }, sub.policy.handshakeMs);
    }
    socket.onmessage = function (event) {
      if (sub.stopped || sub.socket !== socket) return;
      // The deadline is spent the moment this socket says ANYTHING, readable
      // or not: what it was watching for is silence, and a frame that turns
      // out to be undecodable is answered below by distrusting the socket
      // outright rather than by continuing to wait on it.
      if (sub.greeting !== null) {
        clearTimeout(sub.greeting);
        sub.greeting = null;
      }
      var report = decodeReport(event.data);
      if (report === null) {
        // A frame this build cannot read means the channel is not carrying
        // what it is supposed to (see `decodeReport`), so the socket loses
        // its trust: reported down, closed, and retried. Dropping it instead
        // would leave a page that believes in a feed it can no longer read.
        lost(sub);
        return;
      }
      sub.attempt = 0;
      sub.down = false;
      sub.send(report);
    };
    socket.onclose = function () {
      if (sub.stopped || sub.socket !== socket) return;
      lost(sub);
    };
    socket.onerror = function () {
      if (sub.stopped || sub.socket !== socket) return;
      lost(sub);
    };
  }

  var island = {
    /**
     * Subscribe to the helm's invalidation feed, reporting through `send`
     * until the subscription is superseded or stopped.
     *
     * @param {string} base - the helm's absolute origin.
     * @param {{path: string, delaysMs: number[], probeIntervalMs: number,
     *   handshakeMs: number}} policy - the Rust-side feed policy
     *   (farhelm-ui/src/feed.rs).
     * @param {(report: object) => void} send - the Rust channel's sender.
     * @param {number} token - this subscription's identity, minted by the
     *   Rust side (farhelm-ui/src/feed.rs). Only `release` reads it, and
     *   only to refuse ending a subscription that is not the one it meant.
     * @returns {Promise<void>} settles when this subscription ends — at
     *   once if the feed has already been withdrawn. The caller MUST await
     *   it: see this file's header for why returning early would close the
     *   channel it just installed.
     */
    subscribe: function (base, policy, send, token) {
      // Already withdrawn: open nothing. This is the other half of the race
      // `withdrawn` exists for — a withdrawal that arrived before there was
      // anything to stop must still bind what comes after it.
      if (withdrawn()) return Promise.resolve();
      // A second subscribe supersedes the first: two sockets reporting into
      // one channel would make "the feed is down" a claim about whichever of
      // them happened to speak last.
      if (live) teardown(live);
      var sub = {
        base: base,
        policy: policy,
        send: send,
        /** This subscription's identity — see `release`. */
        token: token,
        socket: null,
        /** The retry wait between attempts, or null while none is pending. */
        timer: null,
        /**
         * The handshake deadline for the CURRENT attempt, or null once it
         * has been met or cancelled (see `connect`).
         */
        greeting: null,
        /** How many attempts have failed since the last message arrived. */
        attempt: 0,
        /** Whether the current outage has already been reported. */
        down: false,
        stopped: false,
        settle: null,
      };
      live = sub;
      var parked = new Promise(function (resolve) {
        sub.settle = resolve;
      });
      connect(sub);
      return parked;
    },

    /**
     * End the live subscription, if any.
     *
     * Called by the Rust side when a build mismatch is latched: under skew
     * the feed and the fallback poll are BOTH withdrawn (SPEC_impl.md's
     * withdrawal rule), and a socket left open would keep sending this
     * bundle off to re-read rows written in a vocabulary it does not have.
     *
     * Deliberately silent — no `down` report — because the feed did not
     * fail: the page withdrew it. The Rust side learns the subscription is
     * over from its channel closing, which is what the parked promise
     * settling causes.
     */
    stop: function () {
      // Written here as well as by the Rust side, because either may run
      // first: this is the same one latch, not a copy of it.
      if (typeof window !== "undefined") window.farhelmFeedWithdrawn = true;
      if (live) {
        teardown(live);
        live = null;
      }
    },

    /**
     * End ONE named subscription without withdrawing the feed.
     *
     * Called when the Rust↔JS channel dies while the page itself is fine —
     * a bridge failure rather than a decision (farhelm-ui/src/feed.rs). The
     * socket must not be left running: nobody is listening to its reports
     * any more, it holds a subscriber on the helm, and its handshake timer
     * would go on climbing a ladder into a closed channel.
     *
     * The TOKEN is what keeps this cleanup from doing harm. A dead
     * subscription's cleanup is evaluated asynchronously, so it can arrive
     * after a remount has already installed a replacement — and an unscoped
     * "end the subscription" would then kill the healthy one, leaving a page
     * with no feed and nothing that would ever open another. Ending only the
     * subscription the caller actually held makes a late cleanup a no-op
     * instead.
     *
     * The difference from `stop` is the latch, and it is the whole reason
     * this is a second entry point rather than a flag on the first: nothing
     * about a dead bridge says this page may never subscribe again, so a
     * later mount must be free to open a fresh subscription. Setting the
     * withdrawal latch here would silently retire the feed for the life of
     * the page over a failure it has nothing to do with.
     *
     * @param {number} token - the identity handed to `subscribe`.
     */
    release: function (token) {
      if (!live || live.token !== token) return;
      teardown(live);
      live = null;
    },
  };

  // Installed on whichever of the two environments this file finds itself
  // in — a browser tab has `window` and no `module`, node's test runner the
  // reverse. The exported set is the PURE half plus the island itself; the
  // tests drive the stateful half through `node:vm` instead, where they can
  // supply a fake `WebSocket` and fake timers (see js-tests/events.test.js).
  if (typeof window !== "undefined") window.farhelmEvents = island;
  if (typeof module !== "undefined" && module.exports) {
    module.exports = { decodeReport: decodeReport, delayFor: delayFor, feedUrl: feedUrl };
  }
})();
