// The desktop webview's console shim (PLAN_desktop_web_bug_triage.md's
// webview-shim step): the JS half of the pipe whose Rust-side receiving end
// is `crates/farhelm-helm/src/client_log.rs` — read that file's module docs
// first for the exact wire contract (`POST <base>/api/client-log`, body
// `{"entries":[{level, message, source?}, ...]}`, `Authorization: Bearer
// <device-secret>`) and the caps this file exists to stay under.
//
// ## Registered first, armed whenever
//
// `farhelm-ui/src/lib.rs`'s `App()` renders this file's `<script>` tag ahead
// of every other page script and ahead of `DesktopBootstrapGate` itself.
// That placement maximizes early coverage, but it is NOT an execution-order
// guarantee — Dioxus loads external scripts asynchronously, so
// authentication can finish before this file has run. Arming therefore works
// in either order: `auth.rs`'s `arm_client_log_shim` always writes the
// configuration to `window.__farhelmClientLogPending` before its guarded
// direct `arm()` call, and this file consumes any pending configuration the
// moment it installs. Neither side winning the race loses the arming.
//
// ## Capture starts immediately; sending waits for arming
//
// The shim captures into a bounded, in-memory queue from the moment it
// installs — there is no "not yet ready" state for CAPTURE. What it does
// NOT do before `arm()` is send anything anywhere. `arm()` runs once per
// SUCCESSFUL desktop authentication — including each reauthentication after
// a credential rotation — from `DesktopBootstrapGate`'s success path in
// `auth.rs`, with the embedded helm's loopback origin and the freshly minted
// webview device secret; `disarm()` runs when a reauthentication BEGINS, so
// a revoked credential is never spent on log batches while its replacement
// is negotiated (entries captured meanwhile just queue). This is the
// buffer-until-auth design PLAN_desktop_web_bug_triage.md calls for. The
// honest cost, recorded there and worth repeating: an error thrown before
// the first `arm()` is lost if the eval bridge itself dies before
// authentication completes, because nothing but this bridge can deliver the
// buffered backlog. That window is what the native watchdog
// (`webview_watchdog.rs`) exists to cover from the Rust side instead.
//
// ## Desktop-only, by construction rather than by a runtime check
//
// `App()` only renders this script's tag on the desktop, non-wasm32 build
// (see that function's `#[cfg]`); a browser build never references the
// `Asset` constant that would bundle this file. `arm()` still independently
// refuses a non-loopback base (see `isLoopbackBase`) as defense-in-depth
// against ever phoning home from wherever this script happened to run.
//
// ## What the containment guard does — and does not — cover
//
// The wrapped `console.error`/`console.warn` ALWAYS call the original
// method first, unconditionally. CAPTURE AND FORMATTING run behind one
// re-entrancy flag and a `try`/`catch` that swallows their own failures
// (`guardedCapture`), so a hostile `toString` or a console argument that
// recurses into `console.error` cannot loop or throw back into the caller.
// The flush pipeline (timers, `fetch`) runs OUTSIDE that guard: its failure
// modes are network-shaped, handled by the response/abort paths in `flush`,
// not by the capture guard — the guarantee is "logging never breaks the
// page's own call", not "every line of this file is exception-proof".
// One documented transparency residual: formatting a non-string console
// argument uses `JSON.stringify`, which invokes getters and `toJSON` on the
// value — observable side effects a never-wrapped console would not have
// caused. Bounding that would need a custom traversal this diagnostic pipe
// does not earn for a page whose console calls are this repo's own code;
// the capture guard contains anything such a getter throws.
//
// ## Testability
//
// The pure half — truncation, entry shaping, the bounded queue, byte-aware
// batch slicing, the loopback check, and the per-capture-site formatting —
// is exported under CommonJS for `js-tests/client-log-shim.test.js`. The
// stateful controller (arming, timers, `fetch`) is exercised there too, via
// the same `node:vm` fabricated-window pattern `term-bytes.test.js` uses,
// with a scripted `fetch` and manual timer control; the true end-to-end
// integration (shim through to a `tracing` line) is
// `scripts/desktop-smoke.sh`'s assertion leg (a later PR in this stack).
(function () {
  // Surplus entries beyond this many are dropped OLDEST-first — a
  // continuously failing page should keep reporting its MOST RECENT
  // trouble, not get stuck replaying the first error it ever saw.
  var MAX_QUEUE = 256;
  // Mirrors `client_log::MAX_ENTRIES_PER_REQUEST`.
  var MAX_BATCH = 32;
  // Deliberately ABOVE the server's caps (2048/256), not equal to them:
  // the server's `peer_text_capped` appends a visible "(truncated)" marker
  // when IT cuts a field, and a client that pre-cut to exactly the server
  // limit would hand over an in-limit value the server cannot know was
  // clipped — a shortened stack trace masquerading as a complete one. The
  // margin keeps network cost bounded while leaving the authoritative,
  // MARKED truncation to the server.
  var MAX_MESSAGE_BYTES = 2048 + 128;
  var MAX_SOURCE_BYTES = 256 + 32;
  // One batch per interval, sized so the drain rate roughly matches the
  // server's accept budget (`MAX_ACCEPTED_PER_MINUTE` = 60): 32 entries
  // every 30s is 64/minute. Draining faster would not deliver more — the
  // server drops the excess with a 204 — it would only spend the oldest
  // entries' budget on a backlog while the NEWEST evidence (what the
  // bounded queue exists to preserve) gets discarded server-side.
  var MIN_FLUSH_INTERVAL_MS = 30000;
  // Serialized-body budget per request. Fetch's `keepalive` (used below so
  // a batch in flight during page teardown can still land) caps the total
  // in-flight body at 64 KiB per the Fetch Standard, and a 32-entry batch
  // of maximum fields serializes past that before JSON escaping is even
  // counted — so batches are built against the FINAL serialized size, with
  // headroom under the quota, and the count cap alone is never trusted.
  var MAX_BODY_BYTES = 48 * 1024;
  // Deadline for one flush request. Without it, a loopback server that
  // accepts a connection and stalls would leave `flushing` latched true
  // forever, silently ending all delivery for the process's lifetime.
  var FLUSH_DEADLINE_MS = 10000;

  // ---------------------------------------------------------------------
  // Pure functions — no timers, no `fetch`, no `window` — exported for
  // `js-tests/client-log-shim.test.js`.
  // ---------------------------------------------------------------------

  /**
   * Truncate `value` (coerced to a string) to at most `maxBytes` UTF-8
   * bytes, cutting at a real character boundary found by walking back over
   * UTF-8 continuation bytes — never by decoding leniently and deleting a
   * trailing replacement character, which would also delete a GENUINE
   * U+FFFD that happened to sit at the boundary (diagnostic text about an
   * encoding failure is exactly where one lives). The result is always the
   * longest whole-character prefix that fits.
   */
  function truncateUtf8(value, maxBytes) {
    var str = value === null || value === undefined ? "" : String(value);
    if (maxBytes <= 0) return "";
    var bytes = new TextEncoder().encode(str);
    if (bytes.length <= maxBytes) return str;
    var end = maxBytes;
    // A continuation byte is 0b10xxxxxx; backing up past them (and at most
    // 3 of them can follow a lead byte) lands on the start of the split
    // character, which is then excluded entirely.
    while (end > 0 && (bytes[end] & 0xc0) === 0x80) end--;
    return new TextDecoder("utf-8").decode(bytes.slice(0, end));
  }

  /**
   * Build one wire entry — exactly the shape
   * `client_log::ClientLogEntry` accepts (`deny_unknown_fields`, so no
   * extra fields may ride along): `level`, a byte-capped `message`, and an
   * OMITTED `source` when none was given (omission keeps the payload
   * smaller; the Rust side reads a missing field and an explicit `null`
   * identically, as `None`).
   */
  function formatEntry(level, message, source) {
    var entry = { level: level, message: truncateUtf8(message, MAX_MESSAGE_BYTES) };
    if (source) entry.source = truncateUtf8(source, MAX_SOURCE_BYTES);
    return entry;
  }

  /**
   * Append `entry` to `queue`, dropping from the FRONT once `max` is
   * exceeded — oldest-dropped, so a page stuck in a failure loop keeps
   * reporting its most recent trouble instead of its first.
   */
  function pushBounded(queue, entry, max) {
    queue.push(entry);
    while (queue.length > max) queue.shift();
    return queue;
  }

  /**
   * Remove and return the largest front-of-queue batch that satisfies BOTH
   * limits: at most `maxBatch` entries, and a serialized
   * `{"entries":[...]}` body of at most `maxBytes` UTF-8 bytes. Always
   * takes at least one entry when the queue is non-empty, even if that one
   * entry alone overflows `maxBytes` — a single oversized entry must be
   * attempted (and, at worst, refused by the server) rather than wedging
   * the queue forever behind an unsendable head.
   */
  function takeBatchByBytes(queue, maxBatch, maxBytes) {
    var encoder = new TextEncoder();
    var count = 0;
    // The fixed envelope is `{"entries":[]}` (14 bytes) plus one comma per
    // entry after the first.
    var bytes = 14;
    while (count < Math.min(maxBatch, queue.length)) {
      var entryBytes = encoder.encode(JSON.stringify(queue[count])).length;
      var next = bytes + entryBytes + (count > 0 ? 1 : 0);
      if (next > maxBytes && count > 0) break;
      bytes = next;
      count++;
      if (bytes > maxBytes) break;
    }
    return queue.splice(0, Math.max(count, queue.length > 0 ? 1 : 0));
  }

  /**
   * Whether `base` names a loopback origin — defense-in-depth on a value
   * that already comes from the trusted Rust side (the embedded helm's own
   * bind address). `URL#hostname` keeps the brackets on an IPv6 literal,
   * so only the bracketed form exists to check. A malformed URL is treated
   * as non-loopback rather than throwing.
   */
  function isLoopbackBase(base) {
    try {
      var hostname = new URL(String(base)).hostname;
      return (
        hostname === "localhost" ||
        hostname === "[::1]" ||
        /^127\.\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(hostname)
      );
    } catch (_error) {
      return false;
    }
  }

  /**
   * One `console.error`/`console.warn` argument, stringified defensively.
   * `JSON.stringify` THROWS for circular structures and `BigInt`, and —
   * without throwing — returns `undefined` for `undefined`, functions, and
   * symbols; both shapes fall back to `String(value)`, whose own possible
   * failure (a hostile string-conversion hook) is contained by
   * `guardedCapture` around every capture. Long strings are cut to the
   * message budget BEFORE any encoding work, so a giant string argument
   * costs its prefix, not its full length.
   */
  function stringifyOne(value) {
    if (value instanceof Error) {
      return value.stack || value.name + ": " + value.message;
    }
    if (typeof value === "string") {
      return value.length > MAX_MESSAGE_BYTES ? value.slice(0, MAX_MESSAGE_BYTES) : value;
    }
    try {
      var json = JSON.stringify(value);
      return json === undefined ? String(value) : json;
    } catch (_error) {
      return String(value);
    }
  }

  /**
   * Join a wrapped console call's `arguments` as a stable, space-separated
   * plain-text rendering — a REPRESENTATION for the native log, not a
   * reproduction of devtools output (devtools keeps objects interactive
   * and interprets `%s`-style placeholders; this deliberately does neither).
   */
  function stringifyConsoleArgs(args) {
    var parts = [];
    for (var i = 0; i < args.length; i++) parts.push(stringifyOne(args[i]));
    return parts.join(" ");
  }

  /**
   * Shape one `window.onerror`-style report. `error.stack` is preferred
   * over the bare `message` whenever the engine supplied an `Error`
   * object, because a stack is the whole reason this pipe exists. `source`
   * is the script URL the engine attributes the error to.
   */
  function formatWindowError(message, source, lineno, colno, error) {
    var fallback = message === undefined || message === null ? "" : String(message);
    var text = error && error.stack ? error.stack : fallback;
    if (typeof lineno === "number" || typeof colno === "number") {
      text += " (" + (lineno || 0) + ":" + (colno || 0) + ")";
    }
    return { message: text, source: source ? String(source) : undefined };
  }

  /**
   * Shape one `unhandledrejection` report; `source` is a fixed tag naming
   * the handler, since a rejection has no script URL of its own. The
   * reason renders exactly like a console argument would (stack preferred
   * for `Error`s), via the same `stringifyOne` rule.
   */
  function formatUnhandledRejection(reason) {
    return { message: stringifyOne(reason), source: "unhandledrejection" };
  }

  /**
   * Milliseconds until the next flush is allowed, given the last one
   * started at `lastFlushAt` on the same clock as `now`. `0` means "flush
   * now." Callers feed this MONOTONIC time (`performance.now()`): wall
   * clock would let a backward NTP step inflate the delay by the size of
   * the correction, silently pausing delivery for however far the clock
   * jumped.
   */
  function nextFlushDelayMs(lastFlushAt, now, minIntervalMs) {
    var remaining = minIntervalMs - (now - lastFlushAt);
    return remaining > 0 ? remaining : 0;
  }

  // ---------------------------------------------------------------------
  // Stateful controller: capture, arming, and batched `fetch` flushing.
  // ---------------------------------------------------------------------

  var queue = [];
  /**
   * `{base, secret}` once `arm()` accepted a loopback base; `null` while
   * buffering (pre-auth, or disarmed during a reauthentication). The
   * device credential lives HERE, in shim memory, for as long as the shim
   * is armed — plus the `Authorization` header each flush sends. Nowhere
   * else, ever: not in a log line, not in an entry, not in an URL.
   */
  var armed = null;
  /** Re-entrancy guard around capture+formatting; see the module header. */
  var capturing = false;
  var flushTimer = null;
  /**
   * When the most recent flush REQUEST started, on the monotonic clock —
   * the value `nextFlushDelayMs` paces against.
   */
  var lastFlushAt = 0;
  /**
   * Whether a flush `fetch` is in flight. `scheduleFlush` refuses to queue
   * a timer while true — the request's own completion path schedules the
   * next flush, and a noisy page scheduling a do-nothing timer per capture
   * during a slow request would be load added exactly during an incident.
   */
  var flushing = false;

  function monotonicNow() {
    return typeof performance !== "undefined" && performance.now
      ? performance.now()
      : Date.now();
  }

  /**
   * Run `build` (which returns an entry or a falsy value to skip) behind
   * the re-entrancy guard, push what it returns, and schedule a flush.
   * Every capture site goes through this single choke point so the "never
   * recurse, never throw into the caller" guarantee is implemented once.
   */
  function guardedCapture(build) {
    if (capturing) return;
    capturing = true;
    try {
      var entry = build();
      if (entry) {
        pushBounded(queue, entry, MAX_QUEUE);
        scheduleFlush();
      }
    } catch (_error) {
      // Deliberately swallowed: a bug in this file's own formatting must
      // never propagate into the caller that was just trying to log.
    } finally {
      capturing = false;
    }
  }

  function scheduleFlush() {
    if (!armed || flushing || flushTimer !== null) return;
    var delay = nextFlushDelayMs(lastFlushAt, monotonicNow(), MIN_FLUSH_INTERVAL_MS);
    flushTimer = setTimeout(function () {
      flushTimer = null;
      flush();
    }, delay);
  }

  function flush() {
    if (!armed || flushing || queue.length === 0) return;
    var batch = takeBatchByBytes(queue, MAX_BATCH, MAX_BODY_BYTES);
    var credential = armed;
    flushing = true;
    lastFlushAt = monotonicNow();
    var controller = typeof AbortController !== "undefined" ? new AbortController() : null;
    var deadline = controller
      ? setTimeout(function () {
          controller.abort();
        }, FLUSH_DEADLINE_MS)
      : null;
    fetch(credential.base + "/api/client-log", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: "Bearer " + credential.secret,
      },
      body: JSON.stringify({ entries: batch }),
      // Best effort to land a batch in flight while the page tears down.
      // `keepalive` is why MAX_BODY_BYTES exists: its 64 KiB in-flight
      // quota rejects bigger bodies outright (Fetch Standard).
      keepalive: true,
      signal: controller ? controller.signal : undefined,
    })
      .then(function (response) {
        // `fetch` resolves for HTTP errors too. A 401 means THIS credential
        // was revoked (rotation): put the batch back at the front (still
        // bounded) and disarm until the reauthentication flow re-arms with
        // the replacement — but only if a newer credential has not already
        // been armed while this response was in flight. Every other
        // non-success is the server exercising a documented cap or refusal;
        // the drop policy for those is the same as a network failure below.
        if (response && response.status === 401) {
          Array.prototype.unshift.apply(queue, batch);
          while (queue.length > MAX_QUEUE) queue.pop();
          if (armed === credential) armed = null;
        }
      })
      .catch(function () {
        // A lost batch is lost: retrying would risk turning a transient
        // failure of THIS endpoint into the amplifying loop the server's
        // caps exist to guard against.
      })
      .then(function () {
        if (deadline !== null) clearTimeout(deadline);
        flushing = false;
        if (queue.length > 0) scheduleFlush();
      });
  }

  var shim = {
    /**
     * Arm with the embedded helm's origin and a device-session credential,
     * and start draining anything captured while unarmed. Called once per
     * successful desktop authentication — including each reauthentication
     * — from `auth.rs`. Refuses silently on a non-loopback `base`; an
     * existing armed state is left untouched by a refused call.
     *
     * `config.smokeMarker`, present only under
     * `scripts/desktop-smoke.sh`, is echoed through the wrapped console
     * immediately below — see that call's own comment.
     */
    arm: function (config) {
      if (!config || !isLoopbackBase(config.base)) return;
      armed = { base: config.base, secret: config.secret };
      scheduleFlush();
      // The smoke leg's hook, inert unless the Rust side was launched
      // with FARHELM_SMOKE_CLIENT_LOG_MARKER. Going through the WRAPPED
      // console (not a direct call into capture) is what makes this a
      // proof of the real pipeline — capture, queueing, batching, the
      // auth header, the endpoint, and tracing — rather than a shortcut.
      if (config.smokeMarker) console.error(config.smokeMarker);
    },
    /**
     * Stop sending (capture continues into the bounded queue). Called when
     * a reauthentication BEGINS, so a revoked credential is never spent on
     * batches while its replacement is negotiated.
     */
    disarm: function () {
      armed = null;
      if (flushTimer !== null) {
        clearTimeout(flushTimer);
        flushTimer = null;
      }
    },
  };

  // Installing hooks — as opposed to defining the functions above — must
  // happen ONLY in a browser-like environment: under plain `require()` in
  // node there is no `window`, and wrapping the TEST process's console
  // would contaminate every suite sharing the process. (The js-tests
  // exercise this installation branch too, via a `node:vm` fabricated
  // window, where mutating THAT console is the point.)
  if (typeof window !== "undefined") {
    var originalError = console.error.bind(console);
    var originalWarn = console.warn.bind(console);

    // Transparent wrapping: the original method always runs, first and
    // unconditionally.
    console.error = function () {
      originalError.apply(console, arguments);
      var args = arguments;
      guardedCapture(function () {
        return formatEntry("error", stringifyConsoleArgs(args));
      });
    };
    console.warn = function () {
      originalWarn.apply(console, arguments);
      var args = arguments;
      guardedCapture(function () {
        return formatEntry("warn", stringifyConsoleArgs(args));
      });
    };

    // `addEventListener` rather than assigning `window.onerror` directly:
    // both compose with any handler a library may already have installed.
    window.addEventListener("error", function (event) {
      guardedCapture(function () {
        var formatted = formatWindowError(
          event.message,
          event.filename,
          event.lineno,
          event.colno,
          event.error,
        );
        return formatEntry("error", formatted.message, formatted.source);
      });
    });
    window.addEventListener("unhandledrejection", function (event) {
      guardedCapture(function () {
        var formatted = formatUnhandledRejection(event.reason);
        return formatEntry("error", formatted.message, formatted.source);
      });
    });

    window.__farhelmClientLog = shim;

    // The arming race's other half (see the module header): if
    // authentication finished before this script executed, its
    // configuration is waiting in the pending global — consume it now.
    if (window.__farhelmClientLogPending) {
      var pending = window.__farhelmClientLogPending;
      delete window.__farhelmClientLogPending;
      shim.arm(pending);
    }
  }

  if (typeof module !== "undefined" && module.exports) {
    module.exports = {
      truncateUtf8: truncateUtf8,
      formatEntry: formatEntry,
      pushBounded: pushBounded,
      takeBatchByBytes: takeBatchByBytes,
      isLoopbackBase: isLoopbackBase,
      stringifyOne: stringifyOne,
      stringifyConsoleArgs: stringifyConsoleArgs,
      formatWindowError: formatWindowError,
      formatUnhandledRejection: formatUnhandledRejection,
      nextFlushDelayMs: nextFlushDelayMs,
    };
  }
})();
