// The xterm.js islands. Dioxus owns everything around the terminals; this
// file owns their content path: WebSocket bytes go straight into
// term.write() and keystrokes go straight back, bypassing the reactive
// layer entirely (SPEC_impl.md: the bypass is load-bearing — PTY-rate
// output through a vdom would be a performance disaster).
//
// Loaded as plain scripts (xterm.js, addon-fit.js, then this) — no
// bundler, no CDN: the UI must be fully self-contained.
//
// ## One island per terminal, reconciled declaratively (PLAN_M4.md item 6)
//
// Through M3 a page held at most ONE terminal, and this file was written
// around that: a single `active` record doubled as the mount guard. M4's
// terminal tabs make a session view hold N terminals at once — the agent's
// plus one per open tab — every one of them ATTACHED CONCURRENTLY, because
// switching tabs is a CSS visibility change rather than an attach cutover
// (per-terminal flow control is what makes background tabs safe to leave
// consuming). So the singleton became two maps keyed by the island's DOM
// element id: `islands` (mounted) and `pendings` (still waiting for xterm's
// globals). Every invariant the singleton carried is now a per-key
// invariant; nothing about the byte path itself changed.
//
// The Rust side does not drive mounts and unmounts individually. It hands
// this file the FULL SET of terminals the session view currently wants —
// `sync()` below — and this file reconciles: mount what is missing, unmount
// what is gone, remount what changed identity. That direction is
// deliberate. Only this file knows what is actually mounted, so a diff
// computed in Rust would be a guess about JS state that a failed mount, a
// stale retry, or a page the user reloaded could silently invalidate;
// re-stating the desired set is idempotent, so a spurious re-run costs
// nothing and a missed one self-heals on the next desired-set change.
//
// ## The takeover latch
//
// A view that LOST a session-scoped takeover must stop opening sockets for
// that session, and this file is where that has to live, because this file
// is the only layer that sees the `Detached` notice at all.
//
// The bug it closes is specific and bad: the losing view keeps polling the
// session detail, so when the WINNER opens a tab, the loser learns about it
// and — with no latch — hands it to `sync()`, which attaches it under the
// LOSER's still-valid lease. The supervisor reads that as a new client
// taking the session, and evicts the winner. A user who was never told they
// lost the session silently steals it back the moment anyone else opens a
// tab, which inverts SPEC.md's one-attached-client rule instead of
// enforcing it.
//
// So a takeover-reason `Detached` on ANY of this view's terminals latches
// the whole view: existing islands keep their frozen last-known screens and
// their banners, newly discovered tabs render but never attach, and every
// pending mount is cancelled. Reclaiming is an explicit user act — the
// "take control" button this file paints into the detach banner — which
// unlatches, tears the stale islands down, and re-syncs the full desired
// set under the same lease, displacing whoever holds it now. That is the
// same visible, deliberate takeover the other client performed, not an
// accident.

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
  //
  // Per ISLAND, not per page: the marks are constants, but the counters
  // they gate live inside each `mount()` closure, so one wedged tab pauses
  // only its own stream. That isolation is the whole reason each terminal
  // gets its own channel and its own tmux control client (PLAN_M4.md item
  // 3) — a shared counter here would have quietly undone it.
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

  // Every mounted terminal, keyed by the DOM element id it was mounted
  // into. Doubles as the mount guard it replaced: `mount()` refuses to run
  // again for an element id already present, and — together with
  // `pendings` below — this is the ENTIRE guard; there is no separate
  // mounted flag to keep in sync. That works because `mount()` is
  // synchronous start to finish (nothing yields to the event loop
  // mid-mount) and its own catch block leaves the key absent on failure,
  // so at every point where other code could run these maps already
  // reflect reality.
  //
  // Each value carries what `unmount()` needs to reach — the socket, the
  // xterm instance, the window resize listener registered at mount time,
  // this mount's own test hook — plus the `path`/`gen` pair `sync()`
  // compares against to decide whether a still-wanted island is the SAME
  // attachment or a different one that has to be torn down and rebuilt.
  const islands = new Map();

  // `mountWhenReady()` calls still waiting for xterm's globals and their
  // target DOM element to exist, keyed the same way. At most one per
  // element id: starting a new one, or unmounting that id, cancels
  // whatever was still pending for it. Islands wait independently, so a
  // tab whose element has not rendered yet cannot hold up the agent
  // terminal's mount.
  const pendings = new Map();

  // Which island keyboard focus belongs to, by element id (`null` for
  // none). Tracked across `sync()` calls rather than derived per call so
  // that focus is moved only when the SELECTION actually changes: a sync
  // triggered by something else entirely — a poll picking up a tab another
  // client opened, say — must not yank focus back into the terminal from
  // wherever the user just put it.
  let focusedEl = null;

  // The reason string the supervisor sends every channel a SESSION-SCOPED
  // takeover displaced (its `DETACH_REASON_TAKEOVER`), relayed verbatim by
  // the helm. Matched exactly, and only this one: a stall detach, a
  // restart's own teardown, a closed tab, and the same client's reconnect
  // (`DETACH_REASON_REPLACED`) are all detaches this view either caused or
  // recovers from by itself, and latching on them would freeze a view that
  // has nothing to reclaim.
  //
  // This is a cross-language coupling to a string that is private to the
  // supervisor, so nothing in either language forces the two to move
  // together — what pins it is the browser suite's two-client takeover
  // test, which provokes a REAL takeover through the real stack and fails
  // if the loser does not latch. That test is the contract; this constant
  // is only its client-side half.
  const TAKEOVER_DETACH_REASON = "another client attached";

  // Non-null once a takeover-reason `Detached` has arrived on any of this
  // view's terminals: the reason string, used both as the latch flag and as
  // the text painted onto terminals that never got to attach. Cleared by
  // `reclaim()` and by `unmountAll()` — the latter matters because the
  // latch describes THIS view's relationship to a session, and a fresh view
  // (a reopen, a different session) has no such history to inherit.
  let takeover = null;

  // The last desired set `sync()` was handed, kept so `reclaim()` can
  // rebuild everything from it without needing Rust to re-render first.
  // Reclaim has to work from a latched state where, by construction, no
  // further sync has been allowed to change anything.
  let lastSync = null;

  // Test-only registry (e2e/tests/terminal.spec.ts), keyed by element id:
  // the per-island term/ws/test-hook triple, so the suite can read a
  // SPECIFIC terminal's buffer and socket rather than only whichever one
  // happens to own the legacy singletons below. Published and deleted in
  // lockstep with `islands` itself. Never read by production code.
  function publishIsland(el, entry) {
    if (!window.__farhelmIslands) window.__farhelmIslands = {};
    window.__farhelmIslands[el] = entry;
  }

  function unpublishIsland(el) {
    if (window.__farhelmIslands) delete window.__farhelmIslands[el];
  }

  /**
   * Paint one terminal's banner, optionally with the "take control" button
   * that reclaims a session this view lost.
   *
   * The button is built here rather than in the Dioxus tree deliberately:
   * the banner's CONTENT has always been this file's to own (Dioxus renders
   * an empty div and never diffs children into it), and the latch that
   * decides whether the button belongs there lives here too. Putting the
   * control anywhere else would mean shipping the latch state across the
   * eval boundary — the same channel that is dead on wry's WKWebView
   * (MT-5), for a control whose whole job is to work when things have
   * already gone wrong.
   */
  function paintBanner(bannerId, text, reclaimable) {
    const banner = document.getElementById(bannerId);
    if (!banner) return;
    // Assignment clears any previous children, so the button below can
    // never accumulate copies across repaints.
    banner.textContent = text;
    if (reclaimable) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "btn banner-reclaim";
      button.textContent = "take control";
      button.addEventListener("click", () => farhelmTerm.reclaim());
      banner.appendChild(button);
    }
    banner.style.display = "block";
  }

  window.farhelmTerm = {
    /**
     * Reconcile the mounted terminals against `specs`, the FULL set the
     * session view currently wants (see this file's header for why the
     * diff is computed here rather than in Rust).
     *
     * Each spec is `{el, banner, path, gen, primary, focus}`: the DOM
     * element to mount into, the element its detach/error banner writes
     * to, the helm WebSocket path (already carrying `?tab=`/`?lease=`),
     * a remount counter, whether this island owns the legacy singleton
     * globals, and whether it should hold keyboard focus.
     *
     * `path` and `gen` together are the island's IDENTITY. A still-wanted
     * island whose path changed is a different attachment and is rebuilt;
     * `gen` exists for the case where the path is identical but the
     * attachment must be rebuilt anyway — a restart, whose server-side
     * teardown (`detach_for_restart`) leaves the client holding a socket
     * the supervisor has already abandoned. Restart detaches the AGENT
     * terminal alone, so only the agent's spec ever bumps its `gen`; a
     * tab's attachment survives a restart untouched and must not be
     * disturbed by one.
     *
     * Idempotent by construction: calling this with an unchanged set does
     * nothing at all, so the Rust side never has to prove a change
     * happened before calling. (It memoizes anyway and only calls on a
     * real desired-set change — see `terminal_specs` in lib.rs — but that
     * is an efficiency choice this function does not depend on.)
     *
     * What it is NOT is order-independent — the last call wins, so a
     * caller that delivered a stale set after a fresh one would leave the
     * stale set mounted, with nothing to correct it until the next real
     * change. Callers get that for free today (Dioxus's eval channel is
     * FIFO, and lib.rs's generation token cancels a superseded OUTER wait
     * rather than reordering anything), which is why no sequence number
     * rides along here; anything that starts issuing syncs concurrently
     * would have to add one.
     */
    sync(baseUrl, specs) {
      lastSync = { baseUrl, specs };
      const wanted = new Map(specs.map((spec) => [spec.el, spec]));

      // Tear down first, over the UNION of both maps (an element id is in
      // at most one of them), so an island being REBUILT — its path or
      // generation changed — has released its element, socket, and banner
      // before the replacement mount touches any of them.
      //
      // While latched, only DEPARTURES are honored: a terminal that left
      // the desired set goes, but an identity change is not allowed to tear
      // down a still-listed island, because its frozen last screen and its
      // detach banner are the only record the user has of the session they
      // lost. Rebuilding it would mean opening a socket, which is exactly
      // what the latch exists to prevent.
      for (const el of new Set([...islands.keys(), ...pendings.keys()])) {
        const spec = wanted.get(el);
        const held = islands.get(el) ?? pendings.get(el);
        const departed = !spec;
        const changed = spec && (spec.path !== held.path || spec.gen !== held.gen);
        if (departed || (changed && !takeover)) {
          farhelmTerm.unmount(el);
        }
      }

      // Focus bookkeeping runs BEFORE the mounts below, not after: a
      // freshly mounted island reads `focusedEl` to decide whether to take
      // focus itself (see `mount()`), and `mountWhenReady` may resolve
      // arbitrarily later, so the intent has to be recorded first for that
      // deferred mount to see it.
      const focusEl = specs.find((spec) => spec.focus)?.el ?? null;
      if (focusEl !== focusedEl) {
        focusedEl = focusEl;
        const island = focusEl === null ? null : islands.get(focusEl);
        if (island) island.term.focus();
      }

      for (const spec of specs) {
        // Anything still in `islands` here matched on identity above, and
        // anything still in `pendings` is already waiting for exactly this
        // spec — in both cases there is nothing to do.
        if (islands.has(spec.el) || pendings.has(spec.el)) continue;
        if (takeover) {
          // Discovered, rendered, and explained — but not attached. Left
          // to the placeholder rather than silently blank, because a tab
          // the winner opened is a real tab this view can see and will
          // attach the moment the user reclaims the session.
          paintBanner(spec.banner, `Detached: ${takeover}`, true);
          continue;
        }
        // One island's mount failure must not strand its siblings.
        // `mount()` rethrows after rolling itself back and banners the
        // failure where the user can see it (see its catch block), so the
        // only thing left for this loop to decide is whether the
        // exception also takes down the terminals that had nothing to do
        // with it — and with tabs, it must not: a single malformed URL
        // would otherwise leave a session view with no terminals at all
        // instead of one broken one.
        //
        // The `try` covers only the SYNCHRONOUS part of `mountWhenReady`
        // (its first readiness check, which on an unloaded page usually
        // mounts immediately). A mount deferred to a timer throws on that
        // timer's own stack instead, where nothing here can catch it —
        // harmless for the same reason, since the failing island has
        // already rolled itself back and bannered before rethrowing.
        try {
          farhelmTerm.mountWhenReady(spec, baseUrl);
        } catch (err) {
          console.error("farhelm: mounting terminal", spec.el, "failed", err);
        }
      }
    },

    /**
     * Latch this view out of attaching anything more, because it lost the
     * session to another client (see this file's header for the eviction
     * loop this closes).
     *
     * Idempotent: a session-scoped takeover arrives as one `Detached` per
     * terminal the loser held, so this runs once per terminal and only the
     * first call does anything. Pending mounts are cancelled here rather
     * than merely skipped in `sync()`, because a `mountWhenReady` already
     * in flight would otherwise resolve on its own timer and open exactly
     * the socket the latch exists to prevent.
     */
    latchTakeover(reason) {
      if (takeover) return;
      takeover = reason;
      for (const el of [...pendings.keys()]) farhelmTerm.unmount(el);
    },

    /**
     * Take the session back after losing it: unlatch, discard the stale
     * islands, and re-attach the full desired set under this view's own
     * (unchanged) lease — which displaces whoever holds the session now.
     *
     * Deliberately an explicit user action, wired to the "take control"
     * button in the detach banner, not something that happens on its own.
     * Automatic reclaim is precisely the eviction loop this whole mechanism
     * exists to stop; a takeover is supposed to be a decision someone
     * makes, and this is the client side of making it.
     *
     * Rebuilds from `lastSync`, which includes any tab the winner opened
     * while this view was latched — those were rendered and explained but
     * never attached, and reclaiming is when they finally come up.
     */
    reclaim() {
      if (!takeover) return;
      takeover = null;
      for (const el of new Set([...islands.keys(), ...pendings.keys()])) {
        farhelmTerm.unmount(el);
      }
      if (lastSync) farhelmTerm.sync(lastSync.baseUrl, lastSync.specs);
    },

    /**
     * Wait for xterm's globals (`Terminal`, `FitAddon`) and `spec.el` to
     * exist, then mount — owning the ENTIRE retry loop that used to live
     * in the `document::eval` snippet calling this (lib.rs).
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
     * The fix is `pendings`, and it cancels a superseded attempt TWICE
     * over, deliberately redundantly: this function's own entry cancels
     * whatever was pending for the SAME element id before installing a
     * fresh `attempt`, and every tick of `tryMount` also checks it is
     * still the CURRENT attempt for that id before proceeding (in case a
     * timer already in flight fires despite that `clearTimeout` — see
     * `unmount()`'s docs for a third such backstop). Any ONE of these
     * alone stops an old session's retry from firing into whatever view
     * is open by the time it would otherwise have resolved. (lib.rs
     * additionally guards its OWN outer wait — for `window.farhelmTerm`
     * to exist at all, before this function is even reachable — with a
     * separate generation token; that layer is unrelated to the
     * mechanisms here.)
     *
     * Keyed per element id rather than globally, which is what lets a tab
     * whose DOM node has not rendered yet wait without blocking the agent
     * terminal — and, because the agent terminal's element id is stable
     * across session views, still gives the cross-session cancellation
     * above exactly the same key it had when there was only one island.
     */
    mountWhenReady(spec, baseUrl) {
      const previous = pendings.get(spec.el);
      if (previous) clearTimeout(previous.timer);
      const attempt = { timer: null, path: spec.path, gen: spec.gen };
      pendings.set(spec.el, attempt);
      const tryMount = () => {
        if (pendings.get(spec.el) !== attempt) return;
        if (window.Terminal && window.FitAddon && document.getElementById(spec.el)) {
          pendings.delete(spec.el);
          farhelmTerm.mount(spec, baseUrl);
        } else {
          attempt.timer = setTimeout(tryMount, 50);
        }
      };
      tryMount();
    },

    /**
     * Mount one terminal into `spec.el`, attached to the helm terminal
     * WebSocket at `spec.path` (e.g.
     * `/api/sessions/<id>/term?tab=<tab>&lease=<lease>`).
     *
     * `spec.path` may already carry a query string — the tab selector and
     * the session-attachment lease both ride there (PLAN_M4.md items 1
     * and 5) — so the size parameters below are appended with whichever
     * separator is correct rather than an unconditional `?`.
     *
     * baseUrl is the helm's absolute HTTP origin in both builds — the
     * page's own origin for the web build, FARHELM_URL for the desktop
     * webview (whose origin is not the helm). An empty string falls back
     * to the current page's host, which only happens if origin lookup
     * failed.
     */
    mount(spec, baseUrl) {
      // Re-renders may call mount again; one island per element id.
      // `islands` (see its declaration above) IS the guard; `unmount()`
      // drops the key on the way out, so a session reopened after
      // navigating back to the list gets a fresh mount here rather than
      // silently no-opping against state that no longer has a live DOM
      // node underneath it.
      if (islands.has(spec.el)) return null;

      // Declared before the try/catch, not inside it: the rollback path
      // below needs to reach whatever got created before the exception,
      // and `bannered`/`showBanner` need to be visible from both the
      // happy path and the catch (a `let` inside a `try` block is not
      // visible to its `catch`).
      let term = null;
      let ws = null;
      // Declared out here with `term`/`ws` for the same reason, and it is
      // not cosmetic: both of these REGISTER something with the browser
      // that outlives a throw, so the catch block has to be able to reach
      // them. A mount that failed after registering either would otherwise
      // leak a listener and an observer closing over a disposed terminal,
      // once per failed attempt, each still firing forever.
      let onWindowResize = null;
      let paneObserver = null;
      let bannered = false;
      function showBanner(text, reclaimable) {
        // Sticky by design: the first banner wins for the life of the
        // socket, so a specific reason (a takeover) is never overwritten
        // by the generic close or error that follows it a moment later.
        // Callers must not need to remember to check the flag.
        if (bannered) return;
        bannered = true;
        paintBanner(spec.banner, text, !!reclaimable);
      }

      try {
        const el = document.getElementById(spec.el);
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
        // `?` or `&` depending on whether the caller's path already
        // carries a query (`?tab=`/`?lease=` — see this function's docs).
        const sep = spec.path.indexOf("?") === -1 ? "?" : "&";
        ws = new WebSocket(
          `${base}${spec.path}${sep}cols=${term.cols}&rows=${term.rows}`,
        );
        ws.binaryType = "arraybuffer";

        // Watermark state for THIS attachment only. Declared inside
        // mount() (not module scope) so every fresh WS — a reload, a
        // back-then-reopen, or simply a SIBLING terminal in the same
        // session view — starts from pendingWrite 0 / unpaused,
        // regardless of what any other socket last sent: there is no way
        // for a stale pause/resume to leak across attachments when the
        // counters themselves do not survive the old closure's death, and
        // no way for one tab's backlog to pause another's stream.
        let pendingWrite = 0;
        let paused = false;

        // Test-only observability (e2e/tests/terminal.spec.ts): the
        // watermark state machine lives entirely inside this closure, so
        // the suite has no way to see it crossed a mark or which way
        // without a hook. Never read by production code.
        //
        // Captured in a LOCAL (`testHook`) rather than written straight
        // to the published globals, and published only once mount
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
            // A session-scoped takeover arrives as one such notice per
            // terminal the losing client held (PLAN_M4.md item 3 —
            // there is deliberately no session-wide takeover message), so
            // each island banners its own, independently.
            const msg = JSON.parse(ev.data);
            if (msg.type === "detached") {
              // The latch goes up BEFORE the banner so the banner it
              // paints can already carry the reclaim control (see this
              // file's header). Only a session-scoped takeover latches;
              // every other reason is a detach this view caused or
              // recovers from on its own.
              const lost = msg.reason === TAKEOVER_DETACH_REASON;
              if (lost) farhelmTerm.latchTakeover(msg.reason);
              showBanner(`Detached: ${msg.reason}`, lost);
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
        // MT-6: xterm.js anchors a selection to buffer COORDINATES, not
        // to the text under it. Any input that reaches the pty can move
        // the cursor, scroll the viewport, or overwrite cells, so a
        // selection made before that input stays highlighted over
        // whatever now occupies those coordinates — most visibly,
        // select-and-copy followed by a paste (or by just resuming
        // typing) leaves a stale highlight painted over unrelated
        // content. Cleared on USER-ORIGIN input only: keyboard via
        // `onKey`, paste via a DOM listener on xterm's own element (which
        // dies with the terminal on dispose, so no unmount bookkeeping).
        //
        // The paste listener must be a CAPTURE-phase one, and that is not
        // a style choice: xterm registers its own paste handler on the
        // hidden helper TEXTAREA and that handler calls
        // `stopPropagation()`, so a bubble-phase listener anywhere above
        // the textarea — including on the terminal element — never runs
        // at all. Capture travels downward before the target's handler,
        // so it fires first and is unaffected. Manual testing on macOS
        // caught this: typing cleared the highlight (the `onKey` path)
        // while a real paste left it painted over the pasted text.
        //
        // Deliberately NOT cleared in `onData`: that funnel also carries
        // xterm's auto-generated replies (color/DA/cursor reports a
        // background TUI can provoke at any moment), and clearing there
        // would erase a selection the user just made because an
        // application happened to query the palette. Mouse reports
        // (`onBinary`) are likewise left alone — a shift-selection
        // legitimately coexists with mouse reporting, and clearing on
        // every report would erase it the instant the mouse moved.
        // Output the PTY sends us never touches selection either, so
        // watching a selection while output streams in still works,
        // exactly as it does in a native terminal.
        //
        // Clearing xterm's own selection is not enough on WebKit (both
        // Safari and the desktop app's WKWebView): the highlight survives
        // on screen over rows whose model no longer claims any selection,
        // and it even survived subsequent typing — which is what sent this
        // looking for a second selection rather than a missed clear. The
        // second one is the NATIVE document selection, and `removeAllRanges`
        // in `dismissSelection` below is what actually drops it; an earlier
        // shape of this fix reached for `term.refresh()` on the theory that
        // it was a repaint problem, which it was not. The whole body is
        // guarded by "is anything selected at all", so the common case
        // (input with nothing selected) stays free.
        const dismissSelection = () => {
          // TWO selections have to go, and forgetting the second one is
          // what made this bug look unfixable during manual testing on
          // macOS: the highlight survived the paste, then survived
          // typing, then survived a full repaint. xterm's own
          // selection (its `.xterm-selection` overlay) is only half the
          // story — a mouse drag over the DOM renderer's real text nodes
          // ALSO leaves a native document selection behind, and that one
          // is what stays painted, because `clearSelection` never touches
          // it. Confirmed by probing both engines after a real drag: the
          // xterm overlay clears itself, `window.getSelection()` does
          // not.
          //
          // Detected by `rangeCount`, NOT by `isCollapsed`: WebKit
          // reports a drag-made selection as collapsed while its ranges
          // still exist and stay painted (probed directly — `isCollapsed`
          // true, `toString()` still returning the selected text). A
          // guard on `isCollapsed` therefore skips exactly the case this
          // exists for. Scoped by the selection's anchor so input here
          // never wipes a selection the user made elsewhere on the page
          // (the banner text, say) — and, with several terminals on one
          // page, never wipes a selection made in a DIFFERENT island's
          // terminal either, since the anchor test is against THIS
          // island's own element.
          const native = window.getSelection && window.getSelection();
          const nativeAnchor = native && (native.anchorNode || native.focusNode);
          const nativeInTerm = native
            && String(native) !== ""
            && (!nativeAnchor || term.element.contains(nativeAnchor));
          if (!term.hasSelection() && !nativeInTerm) return;
          term.clearSelection();
          if (nativeInTerm) native.removeAllRanges();
        };
        // Run once now and once after the current event finishes. The
        // second pass is not paranoia: while xterm is handling a
        // keystroke the document selection briefly belongs to its hidden
        // helper textarea, so the row selection is invisible to the
        // synchronous pass and reappears immediately afterwards
        // (observed on Chromium; WebKit clears on the first pass). One
        // deferred sweep catches that without leaving a timer running.
        const dismissSelectionSoon = () => {
          dismissSelection();
          setTimeout(dismissSelection, 0);
        };
        term.onKey(dismissSelectionSoon);
        term.element.addEventListener("paste", dismissSelectionSoon, true);
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
        // for a real bug found in manual testing (MT-4): the banner
        // element lives outside the div this file owns, so a restart's
        // remount inherited the OLD attachment's sticky "Detached:
        // session restarted" banner (painted by the restart's own
        // teardown, `detach_for_restart` in the supervisor) and nothing
        // ever cleared it — it sat there mislabeling a genuinely live
        // terminal as detached. Resetting the inline `display: block`
        // lets `.banner`'s CSS `display: none` (app.css) take back over;
        // `bannered` needs no reset, being a fresh `false` in this
        // closure already. Scoped to THIS island's own banner element, so
        // one terminal reattaching never clears a sibling's still-truthful
        // detach notice.
        //
        // `onopen` is a TRANSPORT-level signal — the WebSocket upgrade
        // completed — not proof the supervisor-side attach succeeded
        // (that happens after the upgrade, server-side). Clearing here is
        // still honest: if the attach then fails, the server closes THIS
        // socket, and its own close handler re-banners. What onopen does
        // guarantee is that the stale banner's claim about the PREVIOUS
        // attachment is obsolete either way.
        ws.onopen = () => {
          const banner = document.getElementById(spec.banner);
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
        onWindowResize = () => fit.fit();
        window.addEventListener("resize", onWindowResize);
        // A window resize is not the only thing that changes a terminal's
        // box, and since tabs it is not even the common one: the tab
        // strip, the close-confirmation row, the tab-error lines, and the
        // lease-error line all sit ABOVE `.terminal-panes` in the same
        // flex column, so opening or closing any of them resizes every
        // pane while the window never moves. Before this observer, a
        // terminal in that state kept its stale geometry until something
        // else happened to refit it — the pane and the pty disagreeing
        // about how many rows exist, which is exactly the class of bug
        // full-screen TUIs render as garbage.
        //
        // Per island and observing that island's own element: the panes
        // are absolutely positioned siblings, so they all change together
        // today, but each terminal owning its own observer is what keeps
        // that an implementation detail rather than a load-bearing
        // assumption. Feature-detected because it is the only browser API
        // this file uses that a very old engine might lack; without it the
        // window listener above still covers the resize case, so the
        // degradation is partial rather than fatal.
        paneObserver = typeof ResizeObserver === "function"
          ? new ResizeObserver(() => fit.fit())
          : null;
        if (paneObserver) paneObserver.observe(el);

        // Focus is a per-PAGE resource, so it goes to the island `sync()`
        // last named as focused rather than unconditionally to whichever
        // island mounted most recently — several terminals mount together
        // when a session view with open tabs is first rendered, and the
        // last one to finish is not the one the user selected.
        if (focusedEl === spec.el) term.focus();
        // Test hooks: tests wait on the flag instead of sleeping, read
        // terminal content through the buffer API — the DOM renderer
        // only materializes viewport rows, so DOM text misses
        // scrollback — and reach the raw socket to exercise
        // message-size limits that keyboard-driven input cannot
        // produce.
        //
        // The singletons are the AGENT terminal's alone (`spec.primary`),
        // kept because they are what every pre-M4 test in the suite reads
        // and because "the terminal" still has an unambiguous meaning for
        // a session view: its agent terminal. Per-tab access goes through
        // `window.__farhelmIslands`, keyed by element id.
        if (spec.primary) {
          window.__farhelmTerm = term;
          window.__farhelmWs = ws;
          window.__farhelmTermReady = true;
          // Published only now, alongside the other readiness globals —
          // see `testHook`'s own docs above for why a callback queued
          // earlier must never have been able to reach this reference
          // before mount succeeded.
          window.__farhelmTest = testHook;
        }
        publishIsland(spec.el, { term, ws, test: testHook });
        islands.set(spec.el, {
          ws,
          term,
          onWindowResize,
          paneObserver,
          testHook,
          path: spec.path,
          gen: spec.gen,
          primary: !!spec.primary,
        });
        return { term, ws };
      } catch (err) {
        // Roll back completely: `islands` is only written on the last line
        // of the happy path, so a mount that threw partway through never
        // registered — which is exactly the property the guard at the top
        // depends on. Were it otherwise, that guard would silently wedge
        // shut every later mount attempt for this terminal for the rest of
        // the page's life (the PARTIAL-MOUNT ROLLBACK finding this
        // closes). What DOES need undoing is the real work already done:
        // a constructor throwing partway (a malformed URL, a missing
        // element) can leave a live socket and a live xterm instance
        // behind, and both are disposed here so the world looks like the
        // mount never started.
        if (onWindowResize) window.removeEventListener("resize", onWindowResize);
        if (paneObserver) paneObserver.disconnect();
        if (ws) ws.close();
        if (term) term.dispose();
        showBanner(`Failed to start terminal: ${err}`);
        throw err;
      }
    },

    /**
     * Tear down one mounted terminal and cancel any still-pending
     * `mountWhenReady()` wait for the same element id, so remounting it —
     * a restart, a reopened session, a tab closed and another opened into
     * the same slot — gets a genuine fresh mount: a fresh xterm instance,
     * a fresh socket, and a fresh attach/replay, rather than either
     * compounding onto the previous mount's state or losing a race to a
     * zombie retry loop. That reopen-after-close path is exactly the
     * regression this lifecycle work exists to prevent.
     *
     * A no-op when that id is neither mounted nor pending, so callers
     * never need to track mount state themselves.
     */
    unmount(el) {
      const attempt = pendings.get(el);
      if (attempt) {
        clearTimeout(attempt.timer);
        pendings.delete(el);
      }
      const island = islands.get(el);
      if (!island) return;
      window.removeEventListener("resize", island.onWindowResize);
      // The observer holds a reference to the element AND to the disposed
      // terminal's fit addon; leaving it connected would keep firing
      // `fit()` on a dead instance every time the (still-present, since
      // Dioxus may keep the pane) element changed size.
      if (island.paneObserver) island.paneObserver.disconnect();
      // Null out every WS callback BEFORE closing: close() starts an
      // asynchronous close handshake with the helm, and a stale
      // `onclose` in particular would otherwise fire later and paint
      // "Connection closed" onto a banner element the NEXT mount reuses,
      // so a late callback here would show a stale banner over an
      // unrelated, healthy terminal. `close()` and `dispose()` are
      // themselves tolerant of whatever state the socket/terminal are
      // already in, so no try/catch is needed around them here.
      island.ws.onopen = null;
      island.ws.onmessage = null;
      island.ws.onerror = null;
      island.ws.onclose = null;
      island.ws.close();
      island.term.dispose();
      islands.delete(el);
      unpublishIsland(el);
      if (!island.primary) return;
      // Guarded, unlike the three deletes below it: comparing against
      // `island.testHook` (this mount's own object, captured at publish
      // time — see `mount()`'s docs) before deleting means this teardown
      // can only ever remove a `__farhelmTest` reference IT installed,
      // never one a later mount already replaced it with. Every
      // `term.write()` callback queued by THIS mount already writes to
      // its own closed-over `testHook` object directly rather than through
      // `window.__farhelmTest` (same docs), so those callbacks firing
      // late are harmless regardless of this guard; this guard is the
      // second, independent line of defense — for `unmount()` itself
      // running out of the order callers are supposed to keep it in.
      if (window.__farhelmTest === island.testHook) {
        delete window.__farhelmTest;
      }
      delete window.__farhelmTerm;
      delete window.__farhelmWs;
      delete window.__farhelmTermReady;
    },

    /**
     * Tear down every terminal and cancel every pending mount — the whole
     * session view going away (SessionView's `use_drop`, lib.rs).
     *
     * Iterates over a snapshot of the union of both key sets because
     * `unmount()` mutates them as it goes. Every scrap of view-scoped
     * state goes with them:
     *
     * - focus ownership, because nothing is mounted afterwards to hold it
     *   and a stale value would make the NEXT view's first `sync()`
     *   believe focus was already where it wanted it and skip placing it;
     * - the takeover latch and the last desired set, because both describe
     *   THIS view's relationship to a session — a view opened afterwards
     *   has its own lease and has lost nothing, so inheriting a latch
     *   would leave it permanently unable to attach.
     */
    unmountAll() {
      for (const el of new Set([...islands.keys(), ...pendings.keys()])) {
        farhelmTerm.unmount(el);
      }
      focusedEl = null;
      takeover = null;
      lastSync = null;
    },
  };
})();
