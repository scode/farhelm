// The xterm.js island. Dioxus owns everything around the terminal; this
// file owns the terminal's content path: WebSocket bytes go straight into
// term.write() and keystrokes go straight back, bypassing the reactive
// layer entirely (SPEC_impl.md: the bypass is load-bearing — PTY-rate
// output through a vdom would be a performance disaster).
//
// Loaded as plain scripts (xterm.js, addon-fit.js, then this) — no
// bundler, no CDN: the UI must be fully self-contained.

(function () {
  "use strict";

  // Watermark backpressure (PLAN_M2_5.md step 4): term.write() buffers
  // asynchronously up to a hard ~50MB cap, then silently discards, so
  // the unwritten-byte counter below (driven by write callbacks) is the
  // only honest signal of how far behind the renderer has fallen.
  // Crossing HIGH_WATER sends `{"type":"pause"}`, which the helm
  // forwards to the supervisor as `ControlMsg::PauseOutput`
  // (crates/farhelm-helm/src/lib.rs, `term_ws`'s docs); the supervisor
  // stops reading that attachment's tmux control client until
  // `{"type":"resume"}` arrives, or — if the pause outlives the
  // supervisor's stall timeout — detaches the attachment as stalled
  // instead (PLAN_M2_5.md, "Stall detach").
  const HIGH_WATER = 4 * 1024 * 1024;

  // A quarter of HIGH_WATER, not just "some smaller number": the gap
  // between the two marks is what stops a producer hovering right at the
  // boundary from flapping pause/resume/pause on every few bytes drained.
  // A resume this close behind the pause still recovers in well under a
  // second even at xterm.js's slowest realistic parse rate (PLAN_M2_5.md:
  // 5-35 MB/s), so nothing is given up by not waiting for a fuller drain.
  // Derived from HIGH_WATER, not a second duplicated literal — a future
  // edit to one mark must not silently un-derive the ratio this comment
  // describes.
  const LOW_WATER = HIGH_WATER / 4;

  // Handle to the single mounted terminal instance (one terminal per
  // page at a time). Doubles as the mount guard: `mount()` refuses to
  // run again while this is non-null, and — together with `pending`
  // below — this is the ENTIRE guard now; there is no separate
  // `window.__farhelmMounted` flag to keep in sync. That works because
  // `mount()` is synchronous start to finish (nothing here yields to
  // the event loop mid-mount) and its own catch block nulls this back
  // out on failure, so at every point where other code could run,
  // `active` already reflects reality. `unmount()` needs this to reach
  // the socket, the xterm instance, and the window resize listener
  // registered at mount time. `null` when nothing is mounted.
  let active = null;

  // A `mountWhenReady()` call still waiting for xterm's globals and the
  // target DOM element to exist. At most one at a time: starting a new
  // one, or calling `unmount()`, cancels whatever is still pending.
  // `null` when nothing is pending.
  let pending = null;

  window.farhelmTerm = {
    /**
     * Wait for xterm's globals (`Terminal`, `FitAddon`) and `#elementId`
     * to exist, then mount — owning the ENTIRE retry loop that used to
     * live in the `document::eval` snippet calling this (lib.rs).
     *
     * That move closes a real bug (the "stale mount retry" finding): the
     * old loop was a bare `setTimeout` chain with no handle anything
     * outside it could reach, so backing out of a session before the
     * loop resolved left it running — unowned and un-cancellable — and
     * it could later fire `mount()` for the OLD session into whatever
     * view was open by the time it finally resolved, racing (and
     * potentially losing to) the REAL mount that was supposed to happen
     * for the NEW session.
     *
     * The fix is `pending`, and it cancels a superseded attempt TWICE
     * over, deliberately redundantly: this function's own entry
     * `clearTimeout`s whatever the PREVIOUS `pending` was before
     * installing a fresh `attempt`, and every tick of `tryMount` also
     * checks it is still the CURRENT `attempt` before proceeding (in
     * case a timer already in flight fires despite that `clearTimeout`
     * — see `unmount()`'s docs for a third such backstop). Any ONE of
     * these alone stops an old session's retry from firing into
     * whatever view is open by the time it would otherwise have
     * resolved. (lib.rs additionally guards its OWN outer wait — for
     * `window.farhelmTerm` to exist at all, before this function is even
     * reachable — with a separate generation token; that layer is
     * unrelated to the mechanisms here.)
     */
    mountWhenReady(elementId, wsPath, baseUrl) {
      if (pending) clearTimeout(pending.timer);
      const attempt = { timer: null };
      pending = attempt;
      const tryMount = () => {
        if (pending !== attempt) return;
        if (window.Terminal && window.FitAddon && document.getElementById(elementId)) {
          pending = null;
          farhelmTerm.mount(elementId, wsPath, baseUrl);
        } else {
          attempt.timer = setTimeout(tryMount, 50);
        }
      };
      tryMount();
    },

    /**
     * Mount a terminal into #elementId, attached to the helm terminal
     * WebSocket at wsPath (e.g. /api/sessions/<id>/term).
     *
     * baseUrl is the helm's absolute HTTP origin in both builds — the
     * page's own origin for the web build, FARHELM_URL for the desktop
     * webview (whose origin is not the helm). An empty string falls back
     * to the current page's host, which only happens if origin lookup
     * failed.
     */
    mount(elementId, wsPath, baseUrl) {
      // Re-renders may call mount again; one terminal per page at a
      // time. `active` (see its declaration above) IS the guard;
      // `unmount()` nulls it on the way out, so a session reopened after
      // navigating back to the list gets a fresh mount here rather than
      // silently no-opping against state that no longer has a live DOM
      // node underneath it.
      if (active) return null;

      // Declared before the try/catch, not inside it: the rollback path
      // below needs to reach whatever got created before the exception,
      // and `bannered`/`showBanner` need to be visible from both the
      // happy path and the catch (a `let` inside a `try` block is not
      // visible to its `catch`).
      let term = null;
      let ws = null;
      let bannered = false;
      function showBanner(text) {
        // Sticky by design: the first banner wins for the life of the
        // socket, so a specific reason (a takeover) is never overwritten
        // by the generic close or error that follows it a moment later.
        // Callers must not need to remember to check the flag.
        if (bannered) return;
        bannered = true;
        const banner = document.getElementById("term-banner");
        if (banner) {
          banner.textContent = text;
          banner.style.display = "block";
        }
      }

      try {
        const el = document.getElementById(elementId);
        term = new Terminal({
          // At most the tmux history floor (`HISTORY_LIMIT`,
          // farhelm-supervisor/src/tmux.rs), never more: PLAN_M2_5.md
          // holds this browser-side cap at or below that floor so that
          // when tmux ITSELF pauses a stalled control client (its own
          // `pause-after`, distinct from the supervisor's stall-detach
          // timeout — a stall-detach just ends the attachment, no replay
          // involved) the reset-then-replay catch-up that follows is
          // observably equivalent to lossless slow delivery — a bigger
          // buffer here would let a user watch history they already had
          // get truncated by the very recovery meant to save it (and, on
          // the other end, SPEC.md's own 10,000-line minimum). Pinned
          // cross-language by farhelm/tests/e2e.rs's
          // `browser_scrollback_stays_within_the_product_floor_and_the_tmux_history_ceiling`,
          // which reads this literal.
          scrollback: 12000,
          fontSize: 14,
          cursorBlink: true,
        });
        const fit = new FitAddon.FitAddon();
        term.loadAddon(fit);
        term.open(el);
        fit.fit();

        const base = baseUrl
          ? baseUrl.replace(/^http/, "ws")
          : (location.protocol === "https:" ? "wss://" : "ws://") + location.host;
        ws = new WebSocket(
          `${base}${wsPath}?cols=${term.cols}&rows=${term.rows}`,
        );
        ws.binaryType = "arraybuffer";

        // Watermark state for THIS attachment only. Declared inside
        // mount() (not module scope) so every fresh WS — a reload, or a
        // back-then-reopen — starts from pendingWrite 0 / unpaused,
        // regardless of what the previous socket last sent: there is no
        // way for a stale pause/resume to leak across attachments when
        // the counters themselves do not survive the old closure's death.
        let pendingWrite = 0;
        let paused = false;

        // Test-only observability (e2e/tests/terminal.spec.ts): the
        // watermark state machine lives entirely inside this closure, so
        // the suite has no way to see it crossed a mark or which way
        // without a hook. Never read by production code.
        //
        // Captured in a LOCAL (`testHook`) rather than written straight
        // to `window.__farhelmTest`, and published there only once mount
        // finishes successfully (see the bottom of this function) — a
        // real hazard closed here: `term.write()`'s completion callback
        // below is queued asynchronously and can still fire AFTER
        // `unmount()` has deleted `window.__farhelmTest`, or after a
        // LATER mount has replaced it with a different attachment's
        // object. Either way, a callback that wrote through the global
        // would throw (deleted) or corrupt a stranger's counts (replaced)
        // instead of harmlessly updating an object nobody reads anymore.
        // Every update below goes through `testHook` directly, so a
        // late-firing callback from a torn-down mount only ever touches
        // its OWN already-orphaned object.
        const testHook = {
          paused: false,
          pauseCount: 0,
          resumeCount: 0,
        };

        function sendControl(type) {
          if (ws.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify({ type }));
          }
        }

        // Sanity backstop, NOT part of the flow-control contract itself:
        // pause/resume above should keep the backlog within a few MiB of
        // HIGH_WATER. Landing here means flow control is not doing its
        // job (a pause message that never reached the supervisor, a
        // socket the browser thinks is open but isn't) and the backlog is
        // now within striking distance of term.write()'s ~50MB silent-
        // discard cliff (this file's own docs, above) — worth one log
        // line, not a spammy one per byte past the mark.
        const BACKLOG_SANITY_BOUND = 32 * 1024 * 1024;
        let sanityWarned = false;

        ws.onmessage = (ev) => {
          if (typeof ev.data === "string") {
            // Text frames are control JSON from the helm; today that is
            // only the detach notice (SPEC.md: takeover must be visible).
            const msg = JSON.parse(ev.data);
            if (msg.type === "detached") {
              showBanner(`Detached: ${msg.reason}`);
            }
            return;
          }
          const bytes = new Uint8Array(ev.data);
          pendingWrite += bytes.length;
          term.write(bytes, () => {
            pendingWrite -= bytes.length;
            // Resume only out of an ACTUAL prior pause: this is the other
            // half of exactly-once semantics (see the pause check below)
            // — without the `paused` guard, a producer whose backlog
            // merely dips below LOW_WATER before ever crossing HIGH_WATER
            // would send a resume the supervisor never asked to answer.
            if (paused && pendingWrite <= LOW_WATER) {
              paused = false;
              testHook.paused = false;
              testHook.resumeCount++;
              sendControl("resume");
            }
          });
          // Exactly once per crossing: `paused` blocks every repeat check
          // while the backlog stays above HIGH_WATER, so one crossing
          // sends one pause, not a flood of them.
          if (!paused && pendingWrite > HIGH_WATER) {
            paused = true;
            testHook.paused = true;
            testHook.pauseCount++;
            sendControl("pause");
          }
          if (pendingWrite > BACKLOG_SANITY_BOUND && !sanityWarned) {
            sanityWarned = true;
            console.warn(
              "farhelm: terminal write backlog far past high-water",
              pendingWrite,
            );
          }
        };
        // A detach notice is immediately followed by the server closing
        // the socket; the close handler must not clobber the more
        // specific banner (the takeover message is the one SPEC.md
        // requires the user to see) — `showBanner`'s own stickiness
        // handles that.
        ws.onclose = () => showBanner("Connection closed");
        ws.onerror = () => showBanner("Connection error");

        const enc = new TextEncoder();
        // Swallow DECRQM mode queries (CSI Pm $ p and CSI ? Pm $ p, e.g.
        // vim's cursor-blink probe ESC[?12$p) at the PARSER, so xterm.js
        // never mints a DECRPM reply for them at all. The reply is the
        // problem: it would take a full render-batch-plus-websocket round
        // trip and land as pane input long after the asking application
        // stopped waiting — and unlike the color/DA/cursor reports (which
        // vim and friends keep accepting late), a late DECRPM reply parses
        // as KEYSTROKES: '$' is a silent motion and 'y' becomes a pending
        // operator, observed as a stray 'y' on every vim launch. Nothing
        // is lost by not answering: tmux answers DECRQM for its panes
        // itself, instantly (verified by probing — the pane sees tmux's
        // reply long before ours could arrive), so xterm's late duplicate
        // was the only copy that ever did harm. Intercepting the QUERY on
        // the output side — rather than filtering reply-shaped chunks out
        // of the input side, as an earlier version of this fix did — means
        // user input is never inspected at all: even a pasted look-alike
        // of a DECRPM reply passes through untouched. Both handlers return
        // true ("handled"), which stops xterm's built-in responder.
        const swallowDecrqm = () => true;
        term.parser.registerCsiHandler(
          { prefix: "?", intermediates: "$", final: "p" },
          swallowDecrqm,
        );
        term.parser.registerCsiHandler(
          { intermediates: "$", final: "p" },
          swallowDecrqm,
        );
        term.onData((d) => {
          if (ws.readyState === WebSocket.OPEN) ws.send(enc.encode(d));
        });
        // onBinary carries mouse reports and other non-UTF8 input as a
        // binary string; encode byte-for-byte.
        term.onBinary((d) => {
          if (ws.readyState !== WebSocket.OPEN) return;
          const bytes = new Uint8Array(d.length);
          for (let i = 0; i < d.length; i++) bytes[i] = d.charCodeAt(i) & 0xff;
          ws.send(bytes);
        });

        const sendResize = () => {
          if (ws.readyState === WebSocket.OPEN) {
            ws.send(
              JSON.stringify({ type: "resize", cols: term.cols, rows: term.rows }),
            );
          }
        };
        term.onResize(sendResize);
        // A resize between socket construction and open would otherwise
        // be dropped forever, leaving the pane sized to the stale
        // dimensions in the connect URL. Assigned as the `onopen`
        // property (not `addEventListener`) so `unmount()` can null it
        // out by name along with the other WS callbacks.
        //
        // Also clears any banner left over from a PRIOR mount — the fix
        // for a real bug found in manual testing (MT-4): `#term-banner`
        // lives outside the `#terminal` div this file owns, so a
        // restart's remount inherited the OLD attachment's sticky
        // "Detached: session restarted" banner (painted by the restart's
        // own teardown, `detach_for_restart` in the supervisor) and
        // nothing ever cleared it — it sat there mislabeling a genuinely
        // live terminal as detached. Resetting the inline `display:
        // block` lets `.banner`'s CSS `display: none` (app.css) take
        // back over; `bannered` needs no reset, being a fresh `false` in
        // this closure already.
        //
        // `onopen` is a TRANSPORT-level signal — the WebSocket upgrade
        // completed — not proof the supervisor-side attach succeeded
        // (that happens after the upgrade, server-side). Clearing here is
        // still honest: if the attach then fails, the server closes THIS
        // socket, and its own close handler re-banners. What onopen does
        // guarantee is that the stale banner's claim about the PREVIOUS
        // attachment is obsolete either way.
        ws.onopen = () => {
          const banner = document.getElementById("term-banner");
          if (banner) {
            banner.style.display = "";
            banner.textContent = "";
          }
          sendResize();
        };
        // Named (not inline) so unmount() can remove exactly this
        // listener: an anonymous closure captured here would be
        // unreachable later, leaking one stale listener (closing over
        // this disposed term/fit pair) per mount/unmount cycle, each
        // still firing on every future window resize.
        const onWindowResize = () => fit.fit();
        window.addEventListener("resize", onWindowResize);

        term.focus();
        // Test hooks: tests wait on the flag instead of sleeping, read
        // terminal content through the buffer API — the DOM renderer
        // only materializes viewport rows, so DOM text misses
        // scrollback — and reach the raw socket to exercise
        // message-size limits that keyboard-driven input cannot
        // produce.
        window.__farhelmTerm = term;
        window.__farhelmWs = ws;
        window.__farhelmTermReady = true;
        // Published only now, alongside the other readiness globals —
        // see `testHook`'s own docs above for why a callback queued
        // earlier must never have been able to reach this reference
        // before mount succeeded.
        window.__farhelmTest = testHook;
        active = { ws, term, onWindowResize, testHook };
        return { term, ws };
      } catch (err) {
        // Roll back completely: `active` must not stay stuck non-null
        // after a failed mount attempt (it never actually got set on
        // this path, but the assignment is explicit here anyway — see
        // below) or the guard above would silently wedge shut every
        // later mount attempt for the rest of the page's life (the
        // PARTIAL-MOUNT ROLLBACK finding this closes) — a constructor
        // throwing partway through (a malformed URL, a missing element)
        // must leave the world looking like the mount never started.
        active = null;
        if (ws) ws.close();
        if (term) term.dispose();
        showBanner(`Failed to start terminal: ${err}`);
        throw err;
      }
    },

    /**
     * Tear down the mounted terminal (SessionView's `use_drop`, lib.rs)
     * and cancel any still-pending `mountWhenReady()` wait, so
     * navigating back to the list and reopening a session — the SAME
     * session or a different one — gets a genuine fresh mount: a fresh
     * xterm instance, a fresh socket, and a fresh attach/replay, rather
     * than either compounding onto the previous mount's state or losing
     * a race to a zombie retry loop. That reopen-after-close path is
     * exactly the regression this lifecycle work exists to prevent.
     *
     * A no-op when nothing is mounted or pending, so callers never need
     * to track mount state themselves.
     */
    unmount() {
      if (pending) {
        clearTimeout(pending.timer);
        pending = null;
      }
      if (!active) return;
      window.removeEventListener("resize", active.onWindowResize);
      // Null out every WS callback BEFORE closing: close() starts an
      // asynchronous close handshake with the helm, and a stale
      // `onclose` in particular would otherwise fire later and paint
      // "Connection closed" onto #term-banner — an element ID the NEXT
      // SessionView instance reuses, so a late callback here would
      // show a stale banner over an unrelated, healthy session. `close()`
      // and `dispose()` are themselves tolerant of whatever state the
      // socket/terminal are already in, so no try/catch is needed around
      // them here.
      active.ws.onopen = null;
      active.ws.onmessage = null;
      active.ws.onerror = null;
      active.ws.onclose = null;
      active.ws.close();
      active.term.dispose();
      // Guarded, unlike the three deletes below it: comparing against
      // `active.testHook` (this mount's own object, captured at publish
      // time — see `mount()`'s docs) before deleting means this teardown
      // can only ever remove a `__farhelmTest` reference IT installed,
      // never one a later mount already replaced it with. Every
      // `term.write()` callback queued by THIS mount already writes to
      // its own closed-over `testHook` object directly rather than through
      // `window.__farhelmTest` (same docs), so those callbacks firing
      // late are harmless regardless of this guard; this guard is the
      // second, independent line of defense — for `unmount()` itself
      // running out of the order callers are supposed to keep it in.
      if (window.__farhelmTest === active.testHook) {
        delete window.__farhelmTest;
      }
      active = null;
      delete window.__farhelmTerm;
      delete window.__farhelmWs;
      delete window.__farhelmTermReady;
    },
  };
})();
