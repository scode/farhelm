// Unit coverage for client-log-shim.js: the pure formatting/queue/threshold
// helpers under plain `require()`, plus the stateful installed controller
// (arming, capture guard, batched `fetch` flushing) exercised through a
// `node:vm` fabricated window, following term-bytes.test.js's pattern (see
// that file's header for why `node --test` and why `node:vm` rather than
// mouse-mode e2e alone). Read client-log-shim.js's own module docs first —
// they are the spec this file pins against, in particular the "buffer until
// arm, ALWAYS run the original console method first, contain but don't
// retry a stalled flush" contract.
const test = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const fs = require("node:fs");
const path = require("node:path");

// Captured BEFORE the shim's first `require()`, not inside the test below —
// a review finding on the previous version of this file: capturing "the
// original" from a `console.error` that has ALREADY been through
// `require()` makes the assertion tautological (it would pass even if
// `require()` silently rewrapped `console.error`, because both sides of the
// comparison would already be the rewrapped function). Capturing here, at
// module load, is the only ordering that lets this test actually fail
// against that regression.
const preRequireConsoleError = console.error;
const preRequireConsoleWarn = console.warn;

const {
  truncateUtf8,
  formatEntry,
  pushBounded,
  takeBatchByBytes,
  isLoopbackBase,
  stringifyOne,
  formatWindowError,
  formatUnhandledRejection,
  nextFlushDelayMs,
} = require("../assets/client-log-shim.js");

const SHIM_PATH = path.join(__dirname, "../assets/client-log-shim.js");
const SHIM_SOURCE = fs.readFileSync(SHIM_PATH, "utf8");

// ---------------------------------------------------------------------
// A. Plain `require()` must never touch this test process's own console.
// ---------------------------------------------------------------------

test("requiring the shim under node leaves console.error and console.warn untouched", () => {
  // `require()` must never install browser hooks or touch this test
  // process's own global `console` — that is the load-bearing guard in the
  // shim's install section (see its module docs, "must happen ONLY in a
  // browser-like environment"). If that guard regressed, every OTHER test
  // file sharing this `node --test` process could start seeing its
  // `console.error` calls silently rerouted, which would be a confusing way
  // to fail a completely unrelated test.
  assert.equal(console.error, preRequireConsoleError);
  assert.equal(console.warn, preRequireConsoleWarn);
});

// ---------------------------------------------------------------------
// B.1 truncateUtf8
// ---------------------------------------------------------------------

test("truncateUtf8 passes a string shorter than the cap through unchanged", () => {
  assert.equal(truncateUtf8("hello", 2048), "hello");
});

test("truncateUtf8 passes a string exactly at the cap through unchanged", () => {
  const exact = "x".repeat(10);
  assert.equal(truncateUtf8(exact, 10), exact);
});

test("truncateUtf8 cuts an over-cap ASCII string to exactly the cap", () => {
  const truncated = truncateUtf8("x".repeat(20), 10);
  assert.equal(truncated, "x".repeat(10));
  assert.equal(Buffer.byteLength(truncated, "utf8"), 10);
});

test("truncateUtf8 backs up over a split multi-byte character instead of emitting it half-formed", () => {
  // "a" (1 byte) + the 4-byte U+1F600 emoji + "b": a 2-byte budget lands
  // inside the emoji's lead+continuation bytes, so the whole emoji must be
  // dropped rather than decoded into a dangling replacement character.
  assert.equal(truncateUtf8("a😀b", 2), "a");
});

test("truncateUtf8 preserves a GENUINE trailing U+FFFD that lands exactly on the cut boundary", () => {
  // U+FFFD ("�") is 3 UTF-8 bytes; "�x" is 4 bytes total, so a 3-byte
  // budget's cut point coincides exactly with the end of a real,
  // caller-supplied U+FFFD rather than one this function invented while
  // decoding a split character. The truncation algorithm must not confuse
  // "the boundary happens to be a U+FFFD" with "decoding produced a
  // U+FFFD" and delete real diagnostic content because of it.
  assert.equal(truncateUtf8("�x", 3), "�");
});

// ---------------------------------------------------------------------
// B.2 formatEntry
// ---------------------------------------------------------------------

test("formatEntry caps a message longer than the client's message budget", () => {
  const over = "m".repeat(2048 + 128 + 1);
  const entry = formatEntry("error", over, undefined);
  assert.equal(entry.message.length, 2048 + 128);
});

test("formatEntry caps a source longer than the client's source budget", () => {
  const over = "s".repeat(256 + 32 + 1);
  const entry = formatEntry("error", "boom", over);
  assert.equal(entry.source.length, 256 + 32);
});

test("formatEntry omits an absent source rather than sending it as null", () => {
  const entry = formatEntry("error", "boom", undefined);
  assert.equal("source" in entry, false);
  assert.equal(entry.source, undefined);
});

test("formatEntry omits an empty-string source", () => {
  const entry = formatEntry("error", "boom", "");
  assert.equal("source" in entry, false);
});

// ---------------------------------------------------------------------
// B.3 stringifyOne
// ---------------------------------------------------------------------

test("stringifyOne renders undefined as the string 'undefined'", () => {
  assert.equal(stringifyOne(undefined), "undefined");
});

test("stringifyOne renders a function and a Symbol as non-empty strings", () => {
  // Both are values `JSON.stringify` returns `undefined` for (without
  // throwing) when they appear as the top-level argument; the fallback to
  // `String(value)` must still produce something a log line can carry.
  const fnRendered = stringifyOne(function namedForRendering() {});
  const symbolRendered = stringifyOne(Symbol("marker"));
  assert.equal(fnRendered.length > 0, true);
  assert.equal(symbolRendered.length > 0, true);
});

test("stringifyOne returns an Error's stack verbatim, not a re-derived message", () => {
  const error = new Error("kaboom");
  error.stack = "SENTINEL_STACK_X";
  assert.equal(stringifyOne(error), "SENTINEL_STACK_X");
});

test("stringifyOne falls back to String() without throwing when JSON.stringify cannot serialize the value", () => {
  // A circular reference makes `JSON.stringify` throw; the custom
  // `toString` (rather than the default `[object Object]`) proves the
  // fallback path — `String(value)` — actually ran, and running it without
  // the whole call throwing is the point of the `try`/`catch` under test.
  const circular = {
    toString() {
      return "CIRCULAR_SENTINEL";
    },
  };
  circular.self = circular;
  assert.doesNotThrow(() => stringifyOne(circular));
  assert.equal(stringifyOne(circular), "CIRCULAR_SENTINEL");
});

test("stringifyOne JSON-serializes a plain object", () => {
  assert.equal(stringifyOne({ a: 1, b: "two" }), '{"a":1,"b":"two"}');
});

// ---------------------------------------------------------------------
// B.4 formatWindowError
// ---------------------------------------------------------------------

test("formatWindowError uses the Error's stack, not the message, when both are supplied and differ", () => {
  const error = new Error("boom");
  error.stack = "WIN_STACK_SENTINEL";
  const formatted = formatWindowError("boom", "app.js", 3, 7, error);
  assert.equal(formatted.message, "WIN_STACK_SENTINEL (3:7)");
  assert.equal(formatted.message.includes("boom"), false);
});

test("formatWindowError falls back to the bare message when no Error object was supplied", () => {
  const formatted = formatWindowError("script error", "app.js", undefined, undefined, null);
  assert.equal(formatted.message, "script error");
  assert.equal(formatted.source, "app.js");
});

// ---------------------------------------------------------------------
// B.5 formatUnhandledRejection
// ---------------------------------------------------------------------

test("formatUnhandledRejection uses an Error reason's stack verbatim, tagged with its own source", () => {
  const error = new Error("rejected");
  error.stack = "REJ_STACK_SENTINEL";
  const formatted = formatUnhandledRejection(error);
  assert.equal(formatted.message, "REJ_STACK_SENTINEL");
  assert.equal(formatted.source, "unhandledrejection");
});

test("formatUnhandledRejection passes a string reason through unchanged", () => {
  const formatted = formatUnhandledRejection("plain string reason");
  assert.equal(formatted.message, "plain string reason");
  assert.equal(formatted.source, "unhandledrejection");
});

// ---------------------------------------------------------------------
// B.6 pushBounded
// ---------------------------------------------------------------------

test("pushBounded drops the OLDEST entries once the bound is exceeded, keeping insertion order", () => {
  // The queue's whole reason to exist is staying bounded through a runaway
  // error loop without losing the MOST RECENT evidence — oldest-dropped,
  // not newest-dropped or a hard stop that would refuse new entries.
  let queue = [];
  for (let i = 0; i < 5; i++) queue = pushBounded(queue, `m${i}`, 3);
  assert.deepEqual(queue, ["m2", "m3", "m4"]);
});

// ---------------------------------------------------------------------
// B.7 takeBatchByBytes
// ---------------------------------------------------------------------

test("takeBatchByBytes never takes more than maxBatch entries, even when the byte budget allows more", () => {
  const queue = [];
  for (let i = 0; i < 40; i++) queue.push({ level: "error", message: "m" });
  const taken = takeBatchByBytes(queue, 32, 1_000_000);
  assert.equal(taken.length, 32);
  assert.equal(queue.length, 8);
});

test("takeBatchByBytes stops at the exact point where including one more entry would exceed maxBytes", () => {
  const maxBatch = 32;
  const maxBytes = 200;
  const queue = [];
  for (let i = 0; i < 50; i++) {
    queue.push({ level: "error", message: `entry-${i}-` + "x".repeat(10) });
  }
  const taken = takeBatchByBytes(queue, maxBatch, maxBytes);

  const takenBytes = Buffer.byteLength(JSON.stringify({ entries: taken }), "utf8");
  assert.equal(takenBytes <= maxBytes, true, "the taken batch must fit the budget");
  assert.equal(taken.length > 0 && taken.length < 50, true, "the budget must have actually constrained the batch");

  // The split point is "exact" precisely because the very next entry would
  // not have fit — pin that directly rather than trusting a byte count in
  // isolation, which could pass by coincidence if the function stopped one
  // entry early.
  const withNextEntry = taken.concat([queue[0]]);
  const withNextBytes = Buffer.byteLength(JSON.stringify({ entries: withNextEntry }), "utf8");
  assert.equal(withNextBytes > maxBytes, true, "the next queued entry must not have fit inside the budget");
});

test("takeBatchByBytes still takes a single entry larger than maxBytes alone, so the queue never wedges", () => {
  const huge = { level: "error", message: "x".repeat(1000) };
  const small = { level: "error", message: "small" };
  const queue = [huge, small];
  const taken = takeBatchByBytes(queue, 32, 50);
  assert.deepEqual(taken, [huge]);
  assert.deepEqual(queue, [small]);
});

test("takeBatchByBytes returns an empty batch for an empty queue", () => {
  const queue = [];
  assert.deepEqual(takeBatchByBytes(queue, 32, 1000), []);
  assert.deepEqual(queue, []);
});

// ---------------------------------------------------------------------
// B.8 nextFlushDelayMs
// ---------------------------------------------------------------------

test("nextFlushDelayMs returns zero once the minimum interval has elapsed", () => {
  assert.equal(nextFlushDelayMs(1000, 4001, 3000), 0);
  assert.equal(nextFlushDelayMs(1000, 4000, 3000), 0, "exactly at the boundary counts as elapsed");
});

test("nextFlushDelayMs returns the exact remaining wait before the interval has elapsed", () => {
  assert.equal(nextFlushDelayMs(1000, 2000, 3000), 2000);
});

test("nextFlushDelayMs never goes negative, however far past the interval 'now' is", () => {
  assert.equal(nextFlushDelayMs(0, 1_000_000, 3000), 0);
});

// ---------------------------------------------------------------------
// B.9 isLoopbackBase
// ---------------------------------------------------------------------

test("isLoopbackBase accepts every loopback form the shim is specified to support", () => {
  // Named for what the function ACCEPTS, not for what any one caller
  // happens to send today — `arm()`'s defense-in-depth check (see the
  // shim's module docs, "Desktop-only, by construction") is only as good
  // as this table, independent of the desktop bootstrap's actual base.
  assert.equal(isLoopbackBase("http://127.0.0.1:7433"), true);
  assert.equal(isLoopbackBase("http://localhost:1"), true);
  assert.equal(isLoopbackBase("http://[::1]:9"), true);
  assert.equal(isLoopbackBase("http://127.9.9.9:1"), true, "the whole 127.0.0.0/8 range is loopback");
});

test("isLoopbackBase rejects a non-loopback host, a private-but-not-loopback host, and a malformed URL", () => {
  assert.equal(isLoopbackBase("http://192.168.1.1"), false);
  assert.equal(isLoopbackBase("https://example.com"), false);
  assert.equal(isLoopbackBase("not a url"), false);
  assert.equal(isLoopbackBase(""), false);
});

// =======================================================================
// C. Stateful controller, exercised through a node:vm fabricated window —
// following term-bytes.test.js's pattern. `require()` only ever exercises
// the CommonJS branch (module is always defined there), so the install
// branch, the capture guard, and the fetch/timer flush pipeline need a
// context shaped like a real page: a `window`, a scriptable `console`, a
// scriptable `fetch`, and hand-cranked timers.
// =======================================================================

/**
 * Build a fresh `node:vm` context shaped like the desktop webview: a
 * `window`, a `console` that records calls instead of printing, a `fetch`
 * whose Promise the test settles by hand, manually stepped timers, and the
 * handful of Node-attached (not V8-intrinsic) globals the shim touches
 * directly — `TextEncoder`/`TextDecoder`, `URL`, `Date`, and a scripted
 * `AbortController`. `setTimeout`/`clearTimeout` are NOT real timers: they
 * just record `{id, callback, ran, cleared}` for `runPendingTimers` to step
 * through explicitly, which is what keeps this whole suite instant and
 * deterministic instead of racing a real 30-second flush interval.
 */
function createSandbox() {
  const consoleErrorCalls = [];
  const consoleWarnCalls = [];
  const fetchCalls = [];
  const timers = [];
  const abortControllers = [];
  const eventListeners = {};
  let nextTimerId = 1;
  let clockNow = 0;

  const sandbox = {
    console: {
      error: function () {
        consoleErrorCalls.push(Array.prototype.slice.call(arguments));
      },
      warn: function () {
        consoleWarnCalls.push(Array.prototype.slice.call(arguments));
      },
    },
    window: {
      addEventListener: function (name, handler) {
        (eventListeners[name] || (eventListeners[name] = [])).push(handler);
      },
    },
    fetch: function (url, options) {
      // Controllable: the call is recorded immediately (synchronously, like
      // real `fetch`'s request-dispatch), but the returned Promise only
      // settles when the test calls the stashed `resolve`/`reject` — that
      // is what lets the "stalled request" tests hold a flush open.
      let resolve, reject;
      const promise = new Promise((res, rej) => {
        resolve = res;
        reject = rej;
      });
      fetchCalls.push({ url: url, options: options, resolve: resolve, reject: reject });
      return promise;
    },
    setTimeout: function (callback, delay) {
      const id = nextTimerId++;
      timers.push({ id: id, delay: delay, callback: callback, ran: false, cleared: false });
      return id;
    },
    clearTimeout: function (id) {
      const timer = timers.find((t) => t.id === id);
      if (timer) timer.cleared = true;
    },
    TextEncoder: TextEncoder,
    TextDecoder: TextDecoder,
    URL: URL,
    Date: Date,
    performance: {
      now: function () {
        return clockNow;
      },
    },
    AbortController: function () {
      const controller = { aborted: false };
      controller.signal = { aborted: false };
      controller.abort = function () {
        controller.aborted = true;
        controller.signal.aborted = true;
      };
      abortControllers.push(controller);
      return controller;
    },
  };
  vm.createContext(sandbox);

  return {
    sandbox: sandbox,
    consoleErrorCalls: consoleErrorCalls,
    consoleWarnCalls: consoleWarnCalls,
    fetchCalls: fetchCalls,
    timers: timers,
    abortControllers: abortControllers,
    setClock: (ms) => {
      clockNow = ms;
    },
  };
}

/**
 * Evaluate the shim's actual source (read from disk, not re-typed here —
 * the same reasoning as term-bytes.test.js's vm test: running the real file
 * is what makes this a regression test rather than a restatement) into a
 * sandbox built by `createSandbox`. When `pendingConfig` is given it is
 * installed on `window.__farhelmClientLogPending` BEFORE evaluation, so the
 * shim's own "consume pending config on install" branch runs — the other
 * half of the arming race described in the shim's module docs.
 */
function loadShim(box, pendingConfig) {
  if (pendingConfig !== undefined) {
    box.sandbox.window.__farhelmClientLogPending = pendingConfig;
  }
  vm.runInContext(SHIM_SOURCE, box.sandbox);
}

/**
 * Run every timer currently scheduled and not yet run or cleared, in
 * scheduling order. Deliberately a SNAPSHOT: a timer scheduled by a
 * callback running during this pass (e.g. `flush()`'s own deadline timer,
 * scheduled from inside the flush-delay timer's callback) is left for the
 * next call rather than chased immediately — that mirrors real event-loop
 * turns closely enough to let tests assert "one tick" behavior like "abort
 * only fires once the deadline timer specifically is run", without this
 * helper accidentally fast-forwarding through multiple ticks at once.
 */
function runPendingTimers(box) {
  const due = box.timers.filter((t) => !t.ran && !t.cleared);
  for (const timer of due) {
    timer.ran = true;
    if (!timer.cleared) timer.callback();
  }
}

/**
 * Drain the microtask queue so a `fetch` Promise the test just settled by
 * hand (via `resolve`/`reject`) has actually run its `.then`/`.catch`
 * chain before assertions look at the result. `setImmediate` is a real
 * Node macrotask, but it costs nothing worth caring about (no simulated
 * delay, no network) and is the simplest way to guarantee every already-
 * queued microtask has run — unlike a fixed number of `await
 * Promise.resolve()` hops, it does not need updating if `flush()` grows
 * another `.then` link.
 */
function flushMicrotasks() {
  return new Promise((resolve) => setImmediate(resolve));
}

test("installing into a fabricated window wraps console.error/warn and exposes arm/disarm", () => {
  const box = createSandbox();
  const originalErrorFn = box.sandbox.console.error;
  const originalWarnFn = box.sandbox.console.warn;
  loadShim(box);

  assert.notEqual(box.sandbox.console.error, originalErrorFn);
  assert.notEqual(box.sandbox.console.warn, originalWarnFn);
  assert.equal(typeof box.sandbox.window.__farhelmClientLog.arm, "function");
  assert.equal(typeof box.sandbox.window.__farhelmClientLog.disarm, "function");

  // Transparent wrapping (module docs: "the original method always runs,
  // first and unconditionally") means the pre-install recorder must fire
  // exactly once per call, with the exact arguments given — not zero times,
  // not twice, not a reshaped copy.
  box.sandbox.console.error("marker", 42);
  assert.deepEqual(box.consoleErrorCalls, [["marker", 42]]);
});

test("capture without arm() never sends: entries queue but no fetch happens even after every pending timer runs", () => {
  const box = createSandbox();
  loadShim(box);

  box.sandbox.console.error("one");
  box.sandbox.console.warn("two");
  box.sandbox.console.error("three");
  runPendingTimers(box);

  assert.equal(box.fetchCalls.length, 0);
});

test("a pending config left by auth.rs before this script runs is consumed on install and flushes captured entries", () => {
  // The other half of the arming race in the shim's module docs: this
  // pins that `window.__farhelmClientLogPending` is read and deleted
  // during evaluation itself, not merely supported if `arm()` is called
  // again later.
  const box = createSandbox();
  const pending = { base: "http://127.0.0.1:7433", secret: "s" };
  loadShim(box, pending);

  assert.equal(box.sandbox.window.__farhelmClientLogPending, undefined);

  box.sandbox.console.error("pending-marker");
  runPendingTimers(box);

  assert.equal(box.fetchCalls.length, 1);
  const call = box.fetchCalls[0];
  assert.equal(call.url, "http://127.0.0.1:7433/api/client-log");
  assert.equal(call.options.headers.Authorization, "Bearer s");
  assert.equal(call.options.headers["Content-Type"], "application/json");
  const body = JSON.parse(call.options.body);
  assert.equal(body.entries.length <= 32, true);
  assert.equal(body.entries.some((e) => e.message === "pending-marker"), true);
});

test("arm() with a non-loopback base sends nothing and leaves an already-armed state untouched", () => {
  const box = createSandbox();
  loadShim(box);

  box.sandbox.window.__farhelmClientLog.arm({ base: "http://127.0.0.1:7433", secret: "good" });
  // A refused arm() call must be a pure no-op on state — not partially
  // apply, not clear the existing credential defensively.
  box.sandbox.window.__farhelmClientLog.arm({ base: "https://evil.example.com", secret: "evil" });

  box.sandbox.console.error("still-good");
  runPendingTimers(box);

  assert.equal(box.fetchCalls.length, 1);
  assert.equal(box.fetchCalls[0].url, "http://127.0.0.1:7433/api/client-log");
  assert.equal(box.fetchCalls[0].options.headers.Authorization, "Bearer good");
});

test("arm() with a smokeMarker echoes it through the wrapped console and flushes an entry containing it", () => {
  // PLAN_desktop_web_bug_triage.md's CI leg (scripts/desktop-smoke.sh)
  // proves the shim -> /api/client-log -> tracing pipeline by arming with
  // a marker and grepping for it in the native log. This test pins the JS
  // half of that proof: the marker must travel through the REAL wrapped
  // console.error (so capture, queueing, and batching all run), not a
  // shortcut that pushed straight onto the queue.
  const box = createSandbox();
  loadShim(box);

  // Spy on the WRAPPED console.error (installed by the shim), delegating
  // through it, so the assertion below distinguishes "the marker traveled
  // through the real capture path" from an implementation that called the
  // original console directly and pushed the marker onto the queue by
  // hand — both of which would satisfy weaker observations.
  const wrapped = box.sandbox.console.error;
  let wrappedMarkerCalls = 0;
  box.sandbox.console.error = function () {
    if (String(arguments[0]).includes("farhelm-smoke-clientlog-marker")) wrappedMarkerCalls++;
    return wrapped.apply(box.sandbox.console, arguments);
  };

  box.sandbox.window.__farhelmClientLog.arm({
    base: "http://127.0.0.1:7433",
    secret: "s",
    smokeMarker: "farhelm-smoke-clientlog-marker",
  });

  assert.equal(
    wrappedMarkerCalls,
    1,
    "arming with a smoke marker must route it through the wrapped console.error exactly once",
  );
  assert.equal(
    box.consoleErrorCalls.some((call) => call.includes("farhelm-smoke-clientlog-marker")),
    true,
    "the original console must still have been reached through the wrapper",
  );

  runPendingTimers(box);

  assert.equal(box.fetchCalls.length, 1);
  const body = JSON.parse(box.fetchCalls[0].options.body);
  assert.equal(
    body.entries.filter((e) => e.message === "farhelm-smoke-clientlog-marker").length,
    1,
    "the flushed batch must contain exactly one entry for the marker — a duplicate would mean a second, out-of-band insertion path",
  );
});

test("arm() without a smokeMarker sends nothing extra", () => {
  // The marker hook must be strictly opt-in: every ordinary desktop
  // authentication (config.smokeMarker absent, the real-world case) must
  // produce no console call and no traffic beyond whatever the page
  // itself already queued.
  const box = createSandbox();
  loadShim(box);

  box.sandbox.window.__farhelmClientLog.arm({ base: "http://127.0.0.1:7433", secret: "s" });
  runPendingTimers(box);

  assert.equal(box.consoleErrorCalls.length, 0);
  assert.equal(box.fetchCalls.length, 0);
});

test("guardedCapture contains a hostile toString: the original console call still ran and a later capture still works", () => {
  const box = createSandbox();
  loadShim(box);

  // Circular so JSON.stringify throws and formatting falls through to
  // String(value), which is where the hostile toString actually runs —
  // see stringifyOne's module docs for why JSON.stringify alone would
  // never reach a toString hook on a plain object.
  const hostile = {
    toString() {
      throw new Error("hostile toString");
    },
  };
  hostile.self = hostile;

  assert.doesNotThrow(() => box.sandbox.console.error(hostile));
  assert.equal(box.consoleErrorCalls.length, 1, "the original console.error must still run despite the throw");

  box.sandbox.console.error("normal-after-hostile");
  assert.equal(
    box.consoleErrorCalls.length,
    2,
    "the re-entrancy guard must have reset so a later, unrelated capture is not silently dropped",
  );
});

test("guardedCapture stops a console argument whose own toString re-enters console.error", () => {
  const box = createSandbox();
  loadShim(box);

  // Circular for the same reason as the hostile-toString case above: it is
  // what forces formatting to fall through to String(value) and therefore
  // actually invoke this toString, which re-enters the wrapped
  // console.error while the OUTER call's capture is still in progress.
  const reentrant = {
    toString() {
      box.sandbox.console.error("nested-call");
      return "outer-rendered";
    },
  };
  reentrant.self = reentrant;

  assert.doesNotThrow(() => box.sandbox.console.error(reentrant));

  // The original method runs unconditionally on EVERY wrapped call, inner
  // and outer alike (module docs) — so both calls are recorded. What must
  // NOT have happened is an infinite loop or an escaped exception; if the
  // re-entrancy guard failed to short-circuit the inner capture, this
  // assertion would still pass but the process would already have blown
  // its stack getting here.
  assert.deepEqual(box.consoleErrorCalls, [[reentrant], ["nested-call"]]);
});

test("a 401 response restores the batch to the queue front, disarms, and a fresh arm() resends it with the new credential", async () => {
  const box = createSandbox();
  loadShim(box);

  box.sandbox.window.__farhelmClientLog.arm({ base: "http://127.0.0.1:7433", secret: "secret1" });
  box.sandbox.console.error("marker-401");
  runPendingTimers(box);

  assert.equal(box.fetchCalls.length, 1);
  assert.equal(box.fetchCalls[0].options.headers.Authorization, "Bearer secret1");
  box.fetchCalls[0].resolve({ status: 401 });
  await flushMicrotasks();

  // Disarmed: further timers (there should be none scheduled — scheduleFlush
  // refuses while unarmed) send nothing.
  runPendingTimers(box);
  assert.equal(box.fetchCalls.length, 1, "disarming must stop delivery, not merely delay it");

  box.sandbox.window.__farhelmClientLog.arm({ base: "http://127.0.0.1:7433", secret: "secret2" });
  runPendingTimers(box);

  assert.equal(box.fetchCalls.length, 2);
  const resent = box.fetchCalls[1];
  assert.equal(resent.options.headers.Authorization, "Bearer secret2");
  const body = JSON.parse(resent.options.body);
  assert.equal(
    body.entries.some((e) => e.message === "marker-401"),
    true,
    "the SAME entry lost to the 401 must be the one that goes out under the new credential",
  );
});

test("a stalled flush aborts on its deadline timer, and a queued entry still flushes once the flag clears", async () => {
  const box = createSandbox();
  loadShim(box);

  box.sandbox.window.__farhelmClientLog.arm({ base: "http://127.0.0.1:7433", secret: "s" });
  box.sandbox.console.error("stalled-marker");
  runPendingTimers(box);

  assert.equal(box.fetchCalls.length, 1);
  assert.equal(box.abortControllers.length, 1);
  assert.equal(box.abortControllers[0].aborted, false, "the deadline timer has not run yet");

  // Captured WHILE the first request is stuck in flight: `flushing` is
  // already true, so this entry queues without scheduling its own timer —
  // it can only go out once the stalled flush's completion handler
  // reschedules.
  box.sandbox.console.warn("queued-during-stall");

  // Running pending timers now advances only the deadline timer (the
  // flush-delay timer that started the stalled request already ran above).
  runPendingTimers(box);
  assert.equal(box.abortControllers[0].aborted, true, "the deadline timer must abort the stalled request");

  // The real fetch would reject once its AbortController fires; the stub
  // isn't wired to do that automatically, so the test completes the
  // causal chain by hand.
  box.fetchCalls[0].reject(new Error("aborted"));
  await flushMicrotasks();

  // `flushing` must have cleared and the queued entry rescheduled — not
  // wedged behind the lost, unretried first batch (module docs: "a lost
  // batch is lost").
  runPendingTimers(box);
  assert.equal(box.fetchCalls.length, 2, "the entry queued during the stall must still flush");
  const body = JSON.parse(box.fetchCalls[1].options.body);
  assert.equal(body.entries.some((e) => e.message === "queued-during-stall"), true);
});
