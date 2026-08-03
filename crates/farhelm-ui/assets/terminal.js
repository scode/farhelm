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
//
// ## The catch-up buffer (PLAN_M5.md item 5)
//
// An attach starts with a REPLAY: every byte the supervisor retained for
// this terminal, written before the live stream begins. Through M4 those
// bytes went into xterm.js as they arrived, so reopening a session made
// the user watch their whole history scroll past — the cost that grows
// with retained size and that M5 exists to remove.
//
// So the bytes of the catch-up phase are BUFFERED here instead, the
// terminal element is hidden behind a connecting placeholder, and the
// whole buffer lands as ONE `term.write()` whose completion callback is
// what reveals the terminal — at the tail, with no intermediate frame
// ever painted. That is the entire feature: the marker is a PRESENTATION
// signal and nothing in this file (or anywhere else) may branch anything
// else on it (`ControlMsg::ReplayComplete`'s own docs).
//
// The phase ends on whichever of these comes first, and every one of them
// WRITES the buffer rather than dropping it:
//
// - `{"type":"replay_complete"}`, the normal end.
// - A detach notice, or the socket closing or erroring: the catch-up was
//   ENDED, and the supervisor owes no marker to an attach it tore down
//   mid-replay (again `ControlMsg::ReplayComplete`'s docs — a
//   presentation that waits for the marker alone hides those terminals
//   forever).
// - Any of the three graceful-degradation bounds below
//   (`REPLAY_BUFFER_LIMIT`, `REPLAY_CHUNK_LIMIT`,
//   `REPLAY_IDLE_TIMEOUT_MS`), which fall back to today's
//   batched-but-visible catch-up rather than to an error — with ONE
//   exception, the socket that never finished connecting, which ends the
//   phase into the detach banner instead of into a terminal that cannot
//   carry what the user types (see `armIdleTimer`).
//
// The bounds are also a TRUST boundary, not only a robustness one. Under
// `--ssh` the supervisor is a different machine, and this phase is the one
// place this file accumulates its bytes instead of handing them straight
// to xterm.js — so "hold until the peer says stop" has to be bounded in
// bytes, in frames, AND in time, or a hostile or broken peer decides how
// much memory this page uses and how long its terminal stays hidden.
//
// One catch-up phase per ATTACH, never re-entered: a tmux `%pause`
// recovery replays retained history into the SAME attachment mid-stream
// with no marker of its own, and treating that as a second catch-up would
// hide a live terminal for as long as the recovery took. A reconnect is a
// new socket and therefore a new mount, which buffers again by
// construction.
//
// Nothing this phase schedules may outlive its mount. `term.write()`'s
// completion callback still runs after `dispose()` (probed directly
// against the vendored xterm.js), and the idle timer is a plain
// `setTimeout`, so both can fire into a torn-down island whose DOM nodes
// a REPLACEMENT mount is already using — revealing a terminal that is
// mid-catch-up, or throwing inside a callback nobody is watching. Every
// deferred path therefore checks this mount's own `alive` token, which
// `unmount()` clears before it disposes anything.
//
// ## Paste and drop interception (PLAN_M4.md item 7)
//
// SPEC.md's attachments contract quantifies over "any of a session's
// terminals", so the hooks are registered PER ISLAND, inside `mount()`,
// on the island's own element: the file lands in the terminal that
// received it, which is the only reading of "at the cursor" that means
// anything once a session has several terminals.
//
// This file owns the DOM half only. The rules — which flavor of a payload
// wins, what a pasted image is named, what separates two inserted paths,
// and the exact wording of every message an upload puts on screen — are
// computed in Rust and handed here as the `attach` policy `sync()` takes
// (farhelm-ui/src/attachments.rs, whose header explains why the runtime
// path cannot be Rust's: a `File` on a DOM event is not something either
// renderer can pass across, and on wry's WKWebView the channel that would
// carry it is dead). What is left here is genuinely thin: read the
// payload, look the answer up, upload, insert.
//
// Insertion goes through `term.paste()` — the same call xterm's own paste
// handler makes — so bracketed paste, application cursor keys, and every
// other mode the pane is in are handled by the code that already handles
// them, and the path lands at whatever cursor position is current when the
// transfer finishes. Nothing here ever calls `focus()`: an upload
// completing must not steal the caret from wherever the user has since put
// it.

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

  // How many buffered replay bytes this file will hold before giving up on
  // the no-intermediate-paint presentation and going live (PLAN_M5.md item
  // 5's first graceful-degradation bound).
  //
  // The number is the helm's own worst-case-replay arithmetic, not a round
  // guess (`TERM_EVENT_QUEUE` in farhelm-helm/src/client.rs, which sizes
  // its queue from the same two constants): tmux retains at most
  // `HISTORY_LIMIT` = 12,000 lines, and a captured line is a terminal row
  // plus its `capture-pane -e` escapes, generously bounded at 256 bytes —
  // 12,000 × 256 B ≈ 3 MiB. A legitimately wide or heavily styled history
  // CAN exceed that, which is exactly why crossing it degrades to a
  // batched-but-visible catch-up instead of erroring; the buffer is
  // written, never dropped, so no byte is ever lost to this bound.
  //
  // It also sits below HIGH_WATER by design: the single flush write can
  // therefore never, on its own, push the unwritten-byte counter past the
  // pause mark, so buffering cannot manufacture a flow-control pause that
  // the same bytes arriving live would not have caused.
  const REPLAY_BUFFER_LIMIT = 3 * 1024 * 1024;

  // How many buffered FRAMES a catch-up may hold, alongside the byte bound
  // above.
  //
  // Bytes alone do not bound this: each frame is kept as its own
  // `Uint8Array` until the flush, and a peer sending millions of one-byte
  // frames stays far below 3 MiB of payload while costing a per-object
  // overhead this page has no bound on at all. The supervisor chunks a
  // replay at 32 KiB, so even a full 3 MiB history is ~96 frames — 1024
  // leaves an order of magnitude for a peer that chunks differently while
  // still refusing the pathological case.
  const REPLAY_CHUNK_LIMIT = 1024;

  // How long a catch-up may go with NO bytes and NO marker before the same
  // degradation applies (PLAN_M5.md item 5's second bound).
  //
  // Idle-based rather than total-duration, deliberately: a slow but
  // PROGRESSING replay — a deep history over `--ssh`, a loaded host — is
  // the case the buffering exists for, and a total-duration cap would
  // abandon it precisely when it is working. What must not persist is a
  // stream that has gone quiet without a marker. A lost marker is a
  // protocol violation, but the containment cannot be allowed to depend on
  // never having one, because the failure it would otherwise produce is a
  // terminal hidden forever.
  //
  // Five seconds is far longer than any gap inside a chunked replay (the
  // supervisor writes 32 KiB frames back to back) or than an attach
  // cutover, and short enough that a lost marker costs a blink of
  // "connecting…" rather than a wedged view.
  //
  // Armed at socket CONSTRUCTION rather than at `onopen`, which is the
  // only placement that also covers a socket stuck in CONNECTING: a
  // handshake that never completes produces no bytes, no marker, and no
  // close, so an open-triggered watchdog would never start and the
  // terminal would stay hidden past every other bound.
  //
  // That coverage comes with an obligation, discharged where the timer
  // fires: the two states it can expire in are NOT the same outcome. A
  // socket that opened and went quiet degrades to a visible, live catch-up;
  // a socket still CONNECTING has no path for input at all, and revealing
  // a live-looking terminal for it would make typing silently go nowhere —
  // exactly the failure SPEC.md requires be reported rather than inferred.
  const REPLAY_IDLE_TIMEOUT_MS = 5000;

  // What stands in front of a terminal for the duration of its catch-up.
  // Deliberately plain and unalarming: nothing has gone wrong, and this is
  // the state EVERY reattach passes through. In-page DOM, like every other
  // notice this file paints, because wry's WKWebView has no native dialogs
  // at all (MT-5).
  const CONNECTING_TEXT = "connecting — catching up on this terminal's history…";

  // What the banner says when the watchdog expires on a socket that never
  // finished connecting.
  //
  // It names the CONSEQUENCE, not the mechanism: what matters to the user
  // is that this terminal cannot carry input, which they would otherwise
  // discover by typing into it and watching nothing happen. It rides the
  // detach banner because that is this file's established surface for "this
  // terminal is not carrying your session anymore", and it says how to get
  // back, because unlike a detach there is no reclaim control for it.
  const UNCONNECTED_TEXT =
    "Not connected: this terminal never finished connecting, so nothing typed here would "
    + "reach the session — reopen the session to try again.";

  /**
   * This mount's catch-up controls, seeded from the three constants above
   * and then overridden by `window.__farhelmTestReplay` if the page
   * defines one.
   *
   * TEST-ONLY, and the one place in this file where a test hook feeds
   * PRODUCTION behavior rather than only observing it — so the reason has
   * to be good. It is: the contracts this phase exists for are exactly the
   * ones a real supervisor will not produce on demand. It always sends its
   * marker, promptly; it never withholds one; and no fixture in the suite
   * can make it replay 3 MiB or go quiet for five seconds mid-catch-up.
   * `holdMarker` keeps the phase open so a test can assert what is on
   * screen DURING it, and the three limits let the degradation bounds be
   * crossed in milliseconds instead of not at all.
   *
   * A production page never defines the global, so the constants above are
   * the operative values everywhere but the browser suite. Read once per
   * mount into that mount's own object: nothing here is shared between
   * islands, and a test that forgets to clear the global still only
   * affects the page it set it on.
   */
  function replayControls() {
    const overrides = window.__farhelmTestReplay || {};
    return {
      holdMarker: !!overrides.holdMarker,
      heldReason: null,
      limits: {
        bufferBytes: overrides.bufferBytes || REPLAY_BUFFER_LIMIT,
        bufferChunks: overrides.bufferChunks || REPLAY_CHUNK_LIMIT,
        idleMs: overrides.idleMs || REPLAY_IDLE_TIMEOUT_MS,
      },
    };
  }

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


  // The counter behind a pasted image's `pasted-<n>.<ext>` name, per PAGE
  // rather than per island: two terminals of one session share a
  // supervisor-side attachments directory, so a per-island counter would
  // have them both minting `pasted-1.png` and relying on collision
  // suffixing to tell the results apart. The supervisor handles that
  // correctly either way (its `name_candidates`), but the paths the user
  // reads are nicer when the client does not manufacture the collision.
  let pastedSequence = 0;

  // How long the one-byte readability probe may take before it is
  // abandoned (see `probe` inside `installAttachments`). A mechanism
  // bound, not a policy: reading a single byte off a local file is
  // microseconds, and anything that has not answered in five seconds is a
  // stuck filesystem — a network mount that went away mid-drag, a device
  // that stopped responding. Waiting forever would leave an upload the
  // user was told about pending with nothing to show for it.
  const PROBE_TIMEOUT_MS = 5000;

  /**
   * Substitute `{key}` placeholders in one of the policy's message
   * templates, in ONE pass.
   *
   * Both properties matter and both were bugs at some point. A
   * replacement FUNCTION is used rather than a string, because
   * `String.replace` interprets `$&`, `$1` and friends in a string
   * replacement — and one of the values substituted here is the helm's own
   * error text, arbitrary prose from a supervisor that under `--ssh` is
   * another machine, which could rewrite the message around it. And the
   * substitution is single-pass, because a sequence of per-key replaces
   * re-scans what earlier keys already inserted: a file named
   * `{reason}.txt` would otherwise have the error message spliced into its
   * own name.
   *
   * A `{key}` with no value keeps its braces rather than vanishing, which
   * makes a policy/JS mismatch visible instead of silently dropping the
   * one word that carried the meaning.
   */
  function fillTemplate(template, values) {
    return String(template == null ? "" : template).replace(
      /\{(\w+)\}/g,
      (whole, key) =>
        Object.prototype.hasOwnProperty.call(values, key) ? String(values[key]) : whole,
    );
  }

  /**
   * A MIME type reduced to the form the extension rule matches on:
   * parameters dropped, trimmed, lowercased. Mirrors `normalized_mime` in
   * farhelm-ui/src/attachments.rs.
   */
  function normalizeMime(mime) {
    return String(mime || "").split(";")[0].trim().toLowerCase();
  }

  /**
   * Register one island's paste/drop interception on its own element and
   * return the handle `unmount()` uses to take it all back down, or `null`
   * when there is nothing to install (no policy, or the element is gone).
   *
   * `spec.status` names the DOM node this writes progress and failures
   * into; `policy` is `attachments::attachment_policy`'s JSON;
   * `isConnected` reports whether this island's socket is open right now.
   * Everything below is scoped to this one island: its own listeners, its
   * own in-flight set, its own `AbortController`, its own readers, its own
   * error list.
   *
   * ## What is deliberate here
   *
   * - A DROP is always `preventDefault`ed, whatever it carried. An
   *   unhandled drop is acted on by the ENGINE — most usefully for us, by
   *   navigating to the dropped file — which would replace the page and
   *   take every terminal on it down. Text that reaches a drop is inserted
   *   as terminal input, which is SPEC.md's own reading ("plain text
   *   passes through as ordinary terminal input") and, unlike a paste, has
   *   no existing handler to fall through to.
   * - A PASTE is intercepted only when a file or an image wins. Plain text
   *   is left entirely alone — no `preventDefault`, no `stopPropagation`
   *   — so xterm's own paste handler runs exactly as it did before this
   *   existed. That is what makes "pasted text that looks like a path is
   *   still text" true by construction rather than by a heuristic that
   *   could misfire.
   * - The listeners are CAPTURE-phase, for the reason the selection
   *   listener above already documents: xterm registers its paste handler
   *   on its hidden helper textarea and calls `stopPropagation()`, so a
   *   bubble-phase listener on any ancestor never runs.
   * - Uploads within one payload run SEQUENTIALLY. The contract is paths
   *   inserted in completion order, which sequential satisfies while also
   *   bounding what one drop can do — dropping fifty files must not open
   *   fifty concurrent uploads.
   * - Nothing is uploaded into a terminal that cannot receive the path.
   *   The whole point of an attachment is the path that lands at the
   *   cursor, and `term.paste()` on a closed socket drops it silently, so
   *   a payload offered to a detached terminal is refused with a visible
   *   message and an upload that OUTLIVES its socket reports the published
   *   path instead of pasting into the void.
   */
  function installAttachments(spec, baseUrl, policy, term, dismissSelectionSoon, isConnected) {
    // No policy at all means no interception rather than an exception:
    // `sync()` is called from Rust, and a caller that predates item 7
    // (or a future one that turns this off) must still get a working
    // terminal.
    if (!policy || !policy.upload) return null;
    const el = document.getElementById(spec.el);
    if (!el) return null;
    // Defaulted rather than assumed present. Every message below is read
    // out of this object inside a DOM event handler, and an exception
    // thrown there would take the whole paste down — including the plain
    // text path, which has nothing to do with the policy. The Rust side
    // always sends a full set (its own tests pin the keys); this is the
    // blast radius, not a second opinion about the contract.
    const messages = policy.text || {};
    const safeChars = policy.safePathChars || "";

    // Aborts every in-flight upload when the island goes away, so a
    // transfer cannot outlive the terminal it was meant to land in and
    // then try to paste into a disposed xterm instance.
    const controller = typeof AbortController === "function" ? new AbortController() : null;
    // The uploads this island has in flight, keyed by a token minted per
    // FILE rather than counted.
    //
    // A count plus a "most recent name" cannot describe the indicator
    // correctly: finish the second of two uploads and the count says one
    // while the remembered name is the finished file's, so the line reads
    // "attaching <the one that just landed>…" for as long as the other
    // one runs. Holding the set means the sole-remaining name is a fact
    // about what is left, not a memory of what started last.
    const active = new Map();
    let nextToken = 0;
    // The readability probes in flight, so disposal can abort them: a
    // `FileReader` is not covered by the fetch `AbortController` and would
    // otherwise keep a stuck read (and its timer) alive past the island.
    const readers = new Set();
    let errors = [];
    let disposed = false;

    /**
     * Repaint the status line from the state above.
     *
     * `textContent` on every line, never `innerHTML`: the strings here
     * carry a filename from the user's own filesystem and an error message
     * from a supervisor that may be a different machine, and neither is
     * markup this page has any reason to parse.
     */
    function render() {
      if (disposed) return;
      const node = document.getElementById(spec.status);
      if (!node) return;
      node.textContent = "";
      const lines = [];
      if (active.size === 1) {
        const [name] = active.values();
        lines.push(["attach-busy", fillTemplate(messages.busyOne, { name })]);
      } else if (active.size > 1) {
        lines.push(["attach-busy", fillTemplate(messages.busyMany, { count: active.size })]);
      }
      for (const message of errors) lines.push(["attach-error", message]);
      for (const [className, message] of lines) {
        const line = document.createElement("div");
        line.className = className;
        line.textContent = message;
        node.appendChild(line);
      }
      // Cleared back to the stylesheet's own `display: none` when there is
      // nothing to say, so a finished upload leaves no empty strip behind.
      node.style.display = lines.length ? "block" : "";
    }

    function fail(message) {
      errors.push(message);
      render();
    }

    /**
     * Insert text into this terminal through the same call xterm's own
     * paste handler makes, so bracketed paste and the pane's other modes
     * are handled by the code that already handles them.
     *
     * The selection sweep rides along because an intercepted paste never
     * reaches the listener that normally does it (this island's own
     * capture handler stopped the event), and MT-6's stale highlight would
     * otherwise be painted over the text this just inserted.
     */
    function insert(text) {
      if (disposed || !text) return;
      term.paste(text);
      dismissSelectionSoon();
    }

    /**
     * A host path as it can safely be typed at a shell: bare when every
     * character is in the policy's safe set, POSIX single-quoted when it
     * is not.
     *
     * The supervisor sanitizes the FILENAME, but the path's parents come
     * from the user's own `--state-dir` — `~/Library/Application
     * Support/…` is a perfectly ordinary place to keep state, and an
     * unquoted path from there reaches the agent as two nonexistent
     * files. A `$(…)` in one would be worse than useless. See
     * `SHELL_SAFE_PATH_CHARS` in farhelm-ui/src/attachments.rs for why the
     * safe set is the conservative one.
     *
     * The escape is the standard POSIX one: single quotes cannot be
     * escaped inside single quotes, so an embedded `'` closes the string,
     * contributes an escaped quote, and reopens it.
     */
    function shellSafePath(path) {
      let safe = path.length > 0;
      for (const character of path) {
        if (safeChars.indexOf(character) < 0) {
          safe = false;
          break;
        }
      }
      if (safe) return path;
      return "'" + path.split("'").join("'\\''") + "'";
    }

    /**
     * The extension a generated name gets for this MIME type — the JS half
     * of `image_extension_for` in farhelm-ui/src/attachments.rs, whose
     * docs carry the reasoning for each step.
     *
     * Derived rather than looked up because PLAN_M4.md item 7 says the
     * extension comes from the MIME type, and no shipped list can cover
     * every `image/*` a clipboard might carry. Only the corrections live
     * in the policy (`extensionAliases`).
     */
    function extensionFor(mime) {
      const normalized = normalizeMime(mime);
      const fallback = policy.fallbackExtension || "bin";
      if (normalized.indexOf("image/") !== 0) return fallback;
      let token = normalized.slice("image/".length).split("+")[0];
      token = token.slice(token.lastIndexOf(".") + 1);
      if (token.indexOf("x-") === 0) token = token.slice(2);
      token = token.replace(/[^a-z0-9]/g, "");
      if (!token || token.length > (policy.maxExtensionLength || 12)) return fallback;
      return (policy.extensionAliases || {})[token] || token;
    }

    /**
     * Whether this entry is raw image DATA rather than a file reference —
     * the narrow rule `classify`'s docs in
     * farhelm-ui/src/attachments.rs spell out, and the only thing that
     * decides whether an upload keeps its name.
     *
     * Nameless image bytes are unambiguous. A name is treated as
     * engine-synthesized only when it is EXACTLY the placeholder for this
     * payload's own type and the `File` was stamped just now, which is
     * what keeps a copied `holiday.png` — and even a copied `image.png`
     * from last week — under its own name.
     */
    function isRawClipboardImage(source, file) {
      if (source !== "clipboard") return false;
      const mime = normalizeMime(file.type);
      if (mime.indexOf("image/") !== 0) return false;
      const name = String(file.name || "");
      if (name === "") return true;
      const placeholder = (policy.placeholderStem || "image") + "." + extensionFor(mime);
      if (name.toLowerCase() !== placeholder) return false;
      const stamped = typeof file.lastModified === "number" ? file.lastModified : 0;
      return Math.abs(Date.now() - stamped) <= (policy.placeholderMaxAgeMs || 0);
    }

    /**
     * The name one entry is uploaded under: its own filename, or a
     * generated `pasted-<n>.<ext>` when there is no name a user chose —
     * raw clipboard image data, and the occasional engine that hands over
     * a nameless `File`.
     */
    function attachmentName(file, generated) {
      if (!generated && file.name) return file.name;
      pastedSequence += 1;
      return policy.namePrefix + pastedSequence + "." + extensionFor(file.type);
    }

    /**
     * Sort one payload's contents into the buckets the policy's
     * classification table is indexed by, plus the directories that have
     * to be rejected.
     *
     * `items` is preferred over `files` because it is the only place the
     * drag-entry API lives (`webkitGetAsEntry`), which is how a dropped
     * directory is caught before anything is read. `files` is the fallback
     * for a payload that has no item list at all — some engines expose
     * only that, and the result has to be identical.
     *
     * Every entry lands in exactly one bucket and carries the name it will
     * be uploaded under, decided here so that nothing downstream has to
     * re-derive it: `files` holds file references (each keeping its own
     * name) and `images` holds raw clipboard data (each getting a
     * generated one).
     */
    function payloadFrom(source, data) {
      const files = [];
      const images = [];
      const directories = [];
      let text = "";
      if (!data) return { files, images, directories, text };
      const place = (file) => {
        const generated = isRawClipboardImage(source, file);
        const entry = { file, name: attachmentName(file, generated) };
        (generated ? images : files).push(entry);
      };
      const items = data.items ? Array.prototype.slice.call(data.items) : [];
      let sawFileItem = false;
      for (const item of items) {
        if (item.kind !== "file") continue;
        sawFileItem = true;
        const entry = item.webkitGetAsEntry ? item.webkitGetAsEntry() : null;
        const file = item.getAsFile ? item.getAsFile() : null;
        if (entry && entry.isDirectory) {
          directories.push(entry.name || (file && file.name) || "the dropped item");
          continue;
        }
        if (file) place(file);
      }
      if (!sawFileItem && data.files) {
        for (const file of Array.prototype.slice.call(data.files)) place(file);
      }
      // Guarded because `getData` is only legal during the event itself on
      // some engines, and a throw here would take the whole drop down.
      try {
        text = (data.getData && data.getData("text/plain")) || "";
      } catch (err) {
        text = "";
      }
      return { files, images, directories, text };
    }

    /**
     * The winning flavor for this payload, read out of the policy's
     * precomputed table rather than decided here — the precedence rule
     * lives in Rust and is not implemented twice (see this file's header).
     *
     * The bit order matches `payload_index` in
     * farhelm-ui/src/attachments.rs. A directory counts toward the FILE
     * bit: a dragged folder is a file-system object, so it has to outrank
     * the `text/plain` copy of its own path that the same drag carries,
     * and it is then rejected instead of uploaded.
     */
    function interpret(payload) {
      const index = (payload.files.length || payload.directories.length ? 4 : 0)
        | (payload.images.length ? 2 : 0)
        | (payload.text ? 1 : 0);
      return policy.classify[index] || "none";
    }

    /**
     * Read one byte before uploading, so that a directory masquerading as
     * a `File` fails HERE — visibly, with nothing sent — rather than as a
     * truncated upload the supervisor rejects for its size (PLAN_M4.md
     * item 7's fallback for engines with no drag-entry API).
     *
     * A `FileReader` rather than `Blob.arrayBuffer()`, because this read
     * has to be CANCELLABLE: the fetch `AbortController` cannot touch a
     * blob read, so an island torn down while a read is stuck on a dead
     * network mount would leave the read (and the promise chain behind it)
     * running with nowhere to report. The reader is registered for
     * disposal and carries its own progress timeout.
     *
     * A zero-byte file is not a failure: slicing past the end of a `Blob`
     * yields an empty one, and reading that succeeds. Skipped entirely on
     * anything without `FileReader` or `slice`, where the check is
     * unavailable and the truncated-upload path is the remaining backstop.
     */
    function probe(file) {
      return new Promise((resolve, reject) => {
        if (!file.slice || typeof FileReader !== "function") {
          resolve();
          return;
        }
        const reader = new FileReader();
        readers.add(reader);
        let timer = null;
        const finish = (problem) => {
          if (timer !== null) clearTimeout(timer);
          timer = null;
          readers.delete(reader);
          if (!problem) {
            resolve();
            return;
          }
          const unreadable = new Error(String(problem));
          unreadable.unreadable = true;
          reject(unreadable);
        };
        reader.onload = () => finish(null);
        reader.onerror = () => finish(reader.error || "read failed");
        reader.onabort = () => finish("read cancelled");
        timer = setTimeout(() => {
          try {
            reader.abort();
          } catch (err) {
            finish(err);
          }
        }, PROBE_TIMEOUT_MS);
        try {
          reader.readAsArrayBuffer(file.slice(0, 1));
        } catch (err) {
          finish(err);
        }
      });
    }

    /**
     * POST one file to the helm and return the host-side path it
     * published (the pinned attachment REST contract: raw body, the
     * proposed name in `?filename=`, `{"path"}` back).
     *
     * The `File` is handed to `fetch` directly rather than read into
     * memory: the browser streams it and sets `Content-Length` from the
     * blob, which is exactly the declared size the helm forwards to the
     * supervisor. A failure carries the response body — the supervisor's
     * own words, trimmed — because that is what makes the error
     * actionable; a body-less refusal falls back to the policy's own
     * status wording, and a 200 that is not the contract's shape is a
     * failure rather than a silent no-op.
     */
    async function upload(file, name) {
      const url = baseUrl + policy.upload + "?filename=" + encodeURIComponent(name);
      const init = { method: "POST", body: file };
      if (controller) init.signal = controller.signal;
      const response = await fetch(url, init);
      if (!response.ok) {
        let detail = "";
        try {
          detail = (await response.text()).trim();
        } catch (err) {
          detail = "";
        }
        throw new Error(
          detail || fillTemplate(messages.httpStatus, { status: response.status }),
        );
      }
      let body = null;
      try {
        body = await response.json();
      } catch (err) {
        body = null;
      }
      if (!body || typeof body.path !== "string" || body.path === "") {
        throw new Error(fillTemplate(messages.noPath, {}));
      }
      return body.path;
    }

    /**
     * Run one payload's uploads to completion, inserting each path as it
     * lands and reporting each failure where the user cannot miss it.
     *
     * A failure never stops the rest of the queue and never inserts
     * anything, which are the two halves of SPEC.md's "upload failures
     * must be visible; an attachment must never disappear silently" — one
     * bad file in a drop of five must not cost the user the other four,
     * and a path that was never published must never appear. An upload
     * that lands after the SOCKET died reports its path rather than
     * pasting it, for the same reason: `term.paste()` would drop it.
     */
    async function send(queue) {
      for (const item of queue) {
        const token = nextToken++;
        active.set(token, item.name);
        render();
        try {
          await probe(item.file);
          const path = await upload(item.file, item.name);
          if (disposed) return;
          if (isConnected()) {
            insert(shellSafePath(path) + policy.separator);
          } else {
            fail(fillTemplate(messages.landed, { name: item.name, path }));
          }
        } catch (err) {
          if (disposed) return;
          const reason = err && err.message ? err.message : String(err);
          fail(
            err && err.unreadable
              ? fillTemplate(messages.unreadable, { name: item.name })
              : fillTemplate(messages.failed, { name: item.name, reason }),
          );
        } finally {
          active.delete(token);
          render();
        }
      }
    }

    /**
     * Act on one classified payload: refuse it outright if this terminal
     * cannot receive a path, reject its directories, and queue the
     * uploads of the winning flavor.
     *
     * Clearing the previous payload's failures is deliberately the ONLY
     * thing that clears them, and only a payload this island will actually
     * ACT on gets to do it — an error stays on screen until the user does
     * something that supersedes it, rather than until a timer they were
     * not watching expires or an empty drag wipes it.
     */
    function accept(payload, flavor) {
      if (!isConnected()) {
        errors = [];
        fail(fillTemplate(messages.detached, {}));
        return;
      }
      errors = [];
      for (const name of payload.directories) {
        fail(fillTemplate(messages.directory, { name }));
      }
      // Every entry of the winning bucket, never a subset: two files in
      // one payload are two attachments, and dropping either would be the
      // silent loss SPEC.md forbids. Image data loses to a file reference
      // because they are two representations of one thing, which is what
      // the precedence order is FOR (see `classify`).
      const queue = flavor === "file" ? payload.files : flavor === "image" ? payload.images : [];
      render();
      if (queue.length) send(queue);
    }

    const onPaste = (ev) => {
      const payload = payloadFrom("clipboard", ev.clipboardData);
      const flavor = interpret(payload);
      // Text and empty payloads are none of this handler's business:
      // returning without touching the event leaves xterm's own paste
      // path exactly as it was.
      if (flavor !== "file" && flavor !== "image") return;
      ev.preventDefault();
      ev.stopPropagation();
      accept(payload, flavor);
    };

    const onDrop = (ev) => {
      ev.preventDefault();
      const payload = payloadFrom("drag", ev.dataTransfer);
      const flavor = interpret(payload);
      // Nothing this island knows how to act on. Returning before
      // `accept()` is what keeps an empty or unsupported drag from
      // clearing a failure the user has not read yet — the default is
      // still prevented, because the engine's own handling of an
      // unrecognized drop is what would navigate the page away.
      if (flavor === "none") return;
      if (flavor === "text") {
        if (!isConnected()) {
          errors = [];
          fail(fillTemplate(messages.detached, {}));
          return;
        }
        insert(payload.text);
        return;
      }
      accept(payload, flavor);
    };

    // Without a `dragover` that prevents the default, the element is not a
    // drop target at all and `drop` never fires — the single most common
    // way HTML drag and drop is wired up wrong.
    const onDragOver = (ev) => {
      ev.preventDefault();
      if (ev.dataTransfer) ev.dataTransfer.dropEffect = "copy";
    };

    el.addEventListener("paste", onPaste, true);
    el.addEventListener("drop", onDrop, true);
    el.addEventListener("dragover", onDragOver, true);

    return {
      dispose() {
        disposed = true;
        el.removeEventListener("paste", onPaste, true);
        el.removeEventListener("drop", onDrop, true);
        el.removeEventListener("dragover", onDragOver, true);
        if (controller) controller.abort();
        // Blob reads are not covered by the fetch abort above, and a read
        // stuck on an unresponsive filesystem would otherwise outlive
        // everything else here.
        for (const reader of readers) {
          try {
            reader.abort();
          } catch (err) {
            // An already-finished reader throws nothing useful; the point
            // is that no reader is left running.
          }
        }
        readers.clear();
        // The status element belongs to the PANE, which for the agent
        // terminal outlives every remount and for a tab lives until that
        // tab leaves the strip — so a stale "attaching…" or a failure from
        // the attachment that just went away has to be cleared here or it
        // would sit over the next one.
        const node = document.getElementById(spec.status);
        if (node) {
          node.textContent = "";
          node.style.display = "";
        }
      },
    };
  }

  /**
   * Show or hide one terminal's connecting placeholder — the surface that
   * stands in front of a terminal for its whole catch-up phase.
   *
   * NEW surface as of PLAN_M5.md item 5, not a reuse of the banner: a
   * banner reports that something went wrong and stays until the
   * attachment is rebuilt, while this says the ordinary thing every
   * reattach does and disappears on its own. The element itself is
   * rendered (empty) by Dioxus, like the banner and the attachment status
   * line, and its CONTENT is this file's — the reveal has to happen from
   * inside a `term.write()` completion callback, which is not something
   * the reactive layer can be driven from.
   *
   * Tolerates a missing element rather than throwing: a caller that
   * predates the field (or a future one that renders no placeholder) must
   * still get a working, buffered terminal — the no-intermediate-paint
   * contract is carried by hiding the TERMINAL, and this is what explains
   * that state to the user.
   */
  function paintConnecting(connectingId, connecting) {
    if (!connectingId) return;
    const node = document.getElementById(connectingId);
    if (!node) return;
    node.textContent = connecting ? CONNECTING_TEXT : "";
    // Cleared back to the stylesheet's own `display: none`, the same way
    // the attachment status line hides itself when it has nothing to say.
    node.style.display = connecting ? "block" : "";
  }

  /**
   * Hand one terminal element back to the stylesheet and take its
   * connecting placeholder down — the exit from the catch-up presentation,
   * whether it is reached by revealing (the normal end), by a mount that
   * failed and rolled itself back, or by an unmount.
   *
   * The visibility style is REMOVED rather than set to `visible`, and that
   * is load-bearing: `visibility` inherits, and an unselected pane hides
   * its whole subtree with it (app.css's `.terminal-pane`), so an explicit
   * `visible` here would override the pane and paint a background tab's
   * terminal over the selected one. Removing the declaration restores
   * inheritance instead of arguing with it.
   */
  function showTerminal(el, connectingId) {
    if (el) el.style.visibility = "";
    paintConnecting(connectingId, false);
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
     * Each spec is `{el, banner, status, connecting, path, gen, primary,
     * focus}`: the DOM element to mount into, the element its detach/error
     * banner writes to, the element its attachment progress and failures
     * write to, the element its connecting placeholder writes to while it
     * catches up (PLAN_M5.md item 5), the helm WebSocket path (already
     * carrying `?tab=`/`?lease=`), a remount counter, whether this island
     * owns the legacy singleton globals, and whether it should hold
     * keyboard focus.
     *
     * `attach` is the paste/drop policy every island of this view shares
     * (farhelm-ui/src/attachments.rs) — one object rather than a copy per
     * spec, since only `status` differs between terminals. Omitting it
     * leaves interception off and changes nothing else.
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
    sync(baseUrl, specs, attach) {
      lastSync = { baseUrl, specs, attach };
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
          farhelmTerm.mountWhenReady(spec, baseUrl, attach);
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
      if (lastSync) farhelmTerm.sync(lastSync.baseUrl, lastSync.specs, lastSync.attach);
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
    mountWhenReady(spec, baseUrl, attach) {
      const previous = pendings.get(spec.el);
      if (previous) clearTimeout(previous.timer);
      const attempt = { timer: null, path: spec.path, gen: spec.gen };
      pendings.set(spec.el, attempt);
      const tryMount = () => {
        if (pendings.get(spec.el) !== attempt) return;
        if (window.Terminal && window.FitAddon && document.getElementById(spec.el)) {
          pendings.delete(spec.el);
          farhelmTerm.mount(spec, baseUrl, attach);
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
    mount(spec, baseUrl, attach) {
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
      // Same reasoning as the two above: the attachment hooks register
      // three listeners on an element Dioxus owns and outlives this
      // mount, so a mount that throws after installing them must be able
      // to reach them from the catch block.
      let attachments = null;
      // The mount point itself, hoisted for the same reason: this mount
      // HIDES it behind the connecting placeholder (PLAN_M5.md item 5),
      // and the element belongs to Dioxus and outlives a failed mount — so
      // a mount that throws after hiding it must be able to hand it back
      // from the catch. Without that, a tab whose mount failed would sit
      // behind "connecting…" forever, with its own failure banner painted
      // over an invisible terminal.
      let el = null;
      // Invalidates this mount's catch-up state (its `alive` token and its
      // idle timer), once that state exists. Hoisted for the same
      // catch-block reason as everything above, and for a sharper version
      // of it: a throw AFTER the timer is armed would otherwise leave it
      // running with nothing to stop it, and it would then flush a replay
      // into the terminal this catch block just disposed.
      let disposeCatchUp = null;
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
        el = document.getElementById(spec.el);
        // Hidden from before xterm is even constructed, and revealed only
        // once this attach's catch-up has landed (PLAN_M5.md item 5).
        // Today an empty xterm mounts VISIBLE and the replay scrolls
        // through it, which is the intermediate state this milestone
        // removes — so the hiding has to precede `term.open()` rather than
        // follow it, or the empty grid is itself a frame the user sees.
        //
        // `visibility`, not `display`, for the reason `.terminal-pane`
        // documents in app.css: a `display: none` element has no layout
        // box, so `fit()` would size this terminal to zero columns and the
        // pty would be told about it.
        el.style.visibility = "hidden";
        paintConnecting(spec.connecting, true);
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
        // The catch-up's idle watchdog is armed further down, the moment
        // the state it guards exists — at CONSTRUCTION rather than at
        // `onopen`, since a socket that never finishes connecting
        // produces no bytes, no marker, and no close to end the phase on.

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
          // The catch-up phase's own observability (PLAN_M5.md's testing
          // decisions). The acceptance is deliberately NOT "sample the
          // scroll position and hope" — sampling can miss frames between
          // observations and proves nothing about paint — so the facts a
          // test needs are RECORDED here as they happen, by the code that
          // is the only witness to them:
          //
          // - `buffering`: still holding bytes back.
          // - `bufferedBytes`/`bufferedChunks`: how much is held RIGHT
          //   NOW, in each of the two currencies the bounds are counted
          //   in. Both drop back to zero at the flush, since they describe
          //   current holdings rather than running totals — and a test
          //   crossing either bound needs the baseline a REAL replay
          //   already put there, which is not a number this suite can
          //   predict.
          // - `writesWhileHidden`: how many `term.write()` calls this
          //   island made before the terminal was revealed. The whole
          //   milestone is that this is ONE for a replay of any depth.
          // - `revealReason`: which of the phase's EIGHT endings fired —
          //   `marker`, `detached`, `closed`, `error`, `size`, `chunks`,
          //   `idle`, `unconnected` — and `null` until one does. The last
          //   one is the odd one out: it ends the phase WITHOUT presenting
          //   a usable terminal (see `armIdleTimer`).
          // - `revealed`: set INSIDE the reveal, so a test that waits on
          //   it is guaranteed the two fields below have been written.
          //   `revealReason` is not that signal: it is recorded when the
          //   phase ends, which is one asynchronous write-completion
          //   ahead of the reveal itself.
          // - `revealedInWriteCallback`: whether the reveal came from the
          //   flush write's completion callback, i.e. after xterm.js had
          //   consumed the whole replay, rather than optimistically.
          // - `viewportAtTailOnReveal`: whether the FIRST VISIBLE FRAME
          //   was at the tail, captured at the instant of the reveal.
          //
          // `holdMarker`/`heldReason`/`limits` come from `replayControls`
          // — test CONTROLS rather than observations; see that function.
          replay: {
            buffering: true,
            bufferedBytes: 0,
            bufferedChunks: 0,
            writesWhileHidden: 0,
            revealReason: null,
            revealed: false,
            revealedInWriteCallback: false,
            viewportAtTailOnReveal: null,
            ...replayControls(),
          },
        };

        function sendControl(type) {
          if (ws.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify({ type }));
          }
        }

        // Sanity backstop, NOT part of the flow-control contract itself:
        // pause/resume in `writeBytes` below should keep the backlog
        // within a few MiB of HIGH_WATER. Landing here means flow control
        // is not doing its job (a pause message that never reached the
        // supervisor, a socket the browser thinks is open but isn't) and
        // the backlog is now within striking distance of term.write()'s
        // ~50MB silent-discard cliff (this file's own docs, above) —
        // worth one log line, not a spammy one per byte past the mark.
        const BACKLOG_SANITY_BOUND = 32 * 1024 * 1024;
        let sanityWarned = false;

        // ------------------------------------------------------------
        // Catch-up buffering (PLAN_M5.md item 5; this file's header
        // carries the design). State for THIS attachment only, in
        // `mount()`'s closure with the watermark counters above and for
        // the same reason: a reconnect is a new mount, and a new mount
        // buffers its own replay from scratch.
        // ------------------------------------------------------------
        //
        // `catchingUp` is one-way. It turns false at the end of the
        // phase and never comes back, which is what keeps a `%pause`
        // flow-control recovery — history replayed into the SAME
        // attachment, deliberately markerless — flowing straight to the
        // screen as ordinary live output.
        let catchingUp = true;
        let revealed = false;
        let replayChunks = [];
        let replayBytes = 0;
        let idleTimer = null;
        // Whether the reveal may place keyboard focus at all. Cleared on
        // the never-connected path (see `armIdleTimer`): focusing a
        // terminal that cannot carry input is how "typing goes nowhere"
        // becomes invisible again, one line under the banner that just
        // said so.
        let focusOnReveal = true;
        // This mount's liveness token, cleared by `unmount()` (and by the
        // rollback below) BEFORE anything is disposed. Every deferred path
        // here — write-completion callbacks, the idle timer — checks it,
        // because both outlive their island: xterm.js runs a queued write
        // callback even after `dispose()`, and a stale reveal would then
        // scroll a disposed terminal, clear a placeholder element by id,
        // and un-hide a DOM node a REPLACEMENT mount is already using for
        // its own catch-up.
        let alive = true;

        function clearIdleTimer() {
          if (idleTimer !== null) clearTimeout(idleTimer);
          idleTimer = null;
        }

        // Re-armed on every buffered chunk, so the window measures
        // SILENCE rather than elapsed time (see REPLAY_IDLE_TIMEOUT_MS).
        // The interval is read at ARM time, not captured once, so a test
        // that retunes `limits.idleMs` mid-attach is honored by the next
        // arm rather than by the next mount.
        //
        // The two ways it can expire are two different OUTCOMES, decided
        // here rather than by whoever reads the reason afterwards:
        //
        // - The socket opened and the stream went quiet. This is the
        //   graceful degradation the bound exists for: flush, reveal, go
        //   live. Input works, because the socket is there to carry it.
        // - The socket is still CONNECTING. Nothing has been received
        //   because nothing has been connected, and `term.onData` drops
        //   keystrokes on a socket that is not OPEN — so revealing a
        //   normal-looking terminal would leave the user typing into a
        //   void with nothing on screen to explain it, which is precisely
        //   the silent failure SPEC.md forbids. The catch-up ends into the
        //   detach banner instead, and the socket is CLOSED: an attach
        //   abandoned this way must not silently resurrect if its
        //   handshake completes minutes later, behind a banner saying it
        //   never connected. Reattaching is the user's move, through the
        //   same path any other detached terminal takes.
        function armIdleTimer() {
          if (!catchingUp || !alive) return;
          clearIdleTimer();
          idleTimer = setTimeout(() => {
            idleTimer = null;
            // `unmount()` clears this timer before disposing anything, so
            // this is the redundant second check the rest of this file's
            // deferred paths also carry: a timer already queued when the
            // clear ran must not banner (or close a socket) on behalf of
            // an island that no longer exists.
            if (!alive) return;
            if (ws.readyState === WebSocket.CONNECTING) {
              focusOnReveal = false;
              endCatchUp("unconnected");
              showBanner(UNCONNECTED_TEXT);
              ws.close();
              return;
            }
            endCatchUp("idle");
          }, testHook.replay.limits.idleMs);
        }

        /**
         * Reveal the terminal, at the tail, and take the placeholder
         * down. Idempotent, because several endings can race (a detach
         * notice immediately followed by the socket closing), and inert
         * on a torn-down mount — see `alive`, whose whole purpose is this
         * function not running late.
         *
         * `fromWriteCallback` is true when this ran inside the flush
         * write's completion callback — the only path on which xterm.js
         * has provably consumed the whole replay before anything became
         * visible.
         *
         * `scrollToBottom()` makes "lands at the tail" a guarantee
         * rather than a dependency on xterm's auto-scroll heuristics:
         * nothing could have scrolled this buffer (it has been hidden
         * since mount), but the contract is worth stating in code.
         *
         * Focus is (re)placed here rather than only at mount, and that is
         * a fix rather than a belt: both engines refuse `focus()` on an
         * element with `visibility: hidden`, which every island now is
         * for its whole catch-up — so the mount-time placement silently
         * did nothing and nothing retried it, leaving a freshly opened
         * session needing a click before it would accept typing.
         * `focusedEl` is read NOW, so a selection change during the
         * catch-up wins over what was wanted at mount.
         *
         * See `takesFocus` for why it is conditional: a reveal lands at a
         * time the user did not choose, and by then they may be typing
         * somewhere else entirely.
         */
        function reveal(fromWriteCallback) {
          if (revealed || !alive) return;
          revealed = true;
          term.scrollToBottom();
          const buffer = term.buffer.active;
          testHook.replay.revealed = true;
          testHook.replay.revealedInWriteCallback = !!fromWriteCallback;
          testHook.replay.viewportAtTailOnReveal = buffer.viewportY === buffer.baseY;
          showTerminal(el, spec.connecting);
          if (takesFocus()) term.focus();
        }

        /**
         * Whether this reveal may take keyboard focus.
         *
         * The island being the one `sync()` named as focused is necessary
         * but NOT sufficient, and the difference is a real bug rather than
         * a courtesy: a reveal happens whenever the replay lands, which is
         * a moment the user did not pick and can be seconds after they
         * moved on. The concrete victim is the rename field — open it
         * while a terminal is still catching up, start typing, and an
         * unconditional `focus()` here would pull the caret into the pty
         * mid-word, sending the rest of the title to the agent.
         *
         * So focus is only taken from somewhere that is not a deliberate
         * choice. The line is drawn at TYPING TARGETS, not at focus per
         * se: an editable control (the rename field is the motivating
         * case) or another island's terminal keeps focus, because pulling
         * keystrokes out from under active typing is the theft this guard
         * exists to prevent. A focused BUTTON does not hold the reveal
         * back — the tab-strip selector is the concrete case: selecting a
         * tab leaves focus on its button, and the whole point of the
         * selection was to type into that tab once it is ready. Stealing
         * from a button costs nothing (buttons consume no keystrokes
         * beyond activation), while refusing would strand every
         * select-then-type flow.
         */
        function takesFocus() {
          if (!focusOnReveal || focusedEl !== spec.el) return false;
          const active = document.activeElement;
          if (!active || active === document.body) return true;
          if (el && el.contains(active)) return true;
          const editable =
            active.matches("input, textarea, select, [contenteditable]") ||
            active.isContentEditable;
          if (editable) return false;
          // Another island's terminal (its helper textarea is editable and
          // caught above, but guard the container too for safety).
          const otherIsland = active.closest(".terminal");
          return !(otherIsland && otherIsland !== el);
        }

        /**
         * Write one chunk of terminal bytes, carrying the watermark
         * accounting that used to live inline in `ws.onmessage`.
         *
         * Shared by the live path and by the catch-up flush precisely so
         * the flush is not a second, unaccounted write path: a 3 MiB
         * batched write moves the unwritten-byte counter exactly as the
         * same bytes arriving live would have.
         */
        function writeBytes(bytes, onWritten) {
          pendingWrite += bytes.length;
          if (!revealed) testHook.replay.writesWhileHidden++;
          term.write(bytes, () => {
            // xterm.js runs this even after `dispose()`, so a mount torn
            // down with writes in flight lands here on a dead island: no
            // flow control to answer for (the socket is closed), and no
            // reveal to perform (see `alive`).
            if (!alive) return;
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
            if (onWritten) onWritten();
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
        }

        /**
         * End the catch-up phase: write everything held back as ONE
         * write, reveal at its completion, and let every later byte go
         * straight to the screen.
         *
         * Every ending comes through here — the marker, a detach, the
         * socket closing, and all three degradation bounds — because the
         * one thing none of them may do is drop the buffer. A terminal
         * that lost its attachment mid-replay still shows what it had
         * received, under whatever banner explains the loss.
         *
         * A no-op after the first call, since the endings can race, and
         * a straight reveal when there is nothing buffered (a fresh
         * terminal's marker arrives immediately, with no history at all)
         * — there is no write to hang the reveal on in that case.
         *
         * The buffer is joined into ONE `Uint8Array` for one
         * `term.write()`: writing the chunks back to back would restore
         * exactly the progressive rendering this whole feature removes,
         * since xterm.js renders between them.
         */
        function endCatchUp(reason) {
          if (!catchingUp || !alive) return;
          // A held marker keeps the phase open instead of ending it, so a
          // test can assert what is on screen DURING a catch-up (see
          // `replayControls`). Only the marker is holdable: the other six
          // endings are what the degradation and teardown specs exist to
          // exercise, and holding those would make the seam able to hide
          // the very behavior it is there to observe.
          if (reason === "marker" && testHook.replay.holdMarker) {
            testHook.replay.heldReason = reason;
            return;
          }
          catchingUp = false;
          clearIdleTimer();
          testHook.replay.buffering = false;
          testHook.replay.revealReason = reason;
          const chunks = replayChunks;
          const total = replayBytes;
          replayChunks = [];
          replayBytes = 0;
          // Current holdings, not running totals: nothing is held once
          // the flush below has taken them.
          testHook.replay.bufferedBytes = 0;
          testHook.replay.bufferedChunks = 0;
          if (total === 0) {
            reveal(false);
            return;
          }
          const joined = new Uint8Array(total);
          let offset = 0;
          for (const chunk of chunks) {
            joined.set(chunk, offset);
            offset += chunk.length;
          }
          writeBytes(joined, () => reveal(true));
        }

        // Armed now, not at `onopen`: the watchdog's job is to bound how
        // long this terminal can stay hidden, and the socket failing to
        // connect at all is one of the ways that can happen (see
        // REPLAY_IDLE_TIMEOUT_MS). Every buffered chunk re-arms it, so
        // once bytes are flowing the window only ever expires on genuine
        // silence.
        armIdleTimer();

        // From here on this mount can be invalidated — by `unmount()`, or
        // by this function's own rollback if something below throws. Both
        // must run BEFORE the terminal is disposed, since the point is to
        // stop deferred work from reaching a disposed instance.
        disposeCatchUp = () => {
          alive = false;
          clearIdleTimer();
        };
        // Test-only (e2e/tests/terminal.spec.ts): resume a catch-up phase
        // held open by `replayControls`'s `holdMarker`, applying whatever
        // ending was deferred. Published on the hook rather than inside
        // `replay` so that object stays plain data — the suite reads it
        // across the page boundary, where a function property would not
        // survive serialization.
        testHook.releaseCatchUp = () => {
          testHook.replay.holdMarker = false;
          if (testHook.replay.heldReason) endCatchUp(testHook.replay.heldReason);
        };

        ws.onmessage = (ev) => {
          if (typeof ev.data === "string") {
            // Text frames are control JSON from the helm: the detach
            // notice (SPEC.md: takeover must be visible) and, since
            // PLAN_M5.md item 4, the replay-complete marker. A
            // session-scoped takeover arrives as one detach notice per
            // terminal the losing client held (PLAN_M4.md item 3 — there
            // is deliberately no session-wide takeover message), so each
            // island banners its own, independently.
            const msg = JSON.parse(ev.data);
            if (msg.type === "replay_complete") {
              // The ONLY thing this message does, here or anywhere: end
              // one terminal's catch-up phase. Nothing about the session,
              // its lifecycle, or any other client behavior may key off it
              // (`ControlMsg::ReplayComplete`'s own docs).
              endCatchUp("marker");
              return;
            }
            if (msg.type === "detached") {
              // The buffer is flushed BEFORE the banner, so a terminal
              // detached mid-replay shows what it did receive underneath
              // the notice explaining the loss — this attach will never
              // get a marker (the supervisor owes none to a catch-up
              // something else ended), and waiting for one would hide the
              // terminal forever.
              endCatchUp("detached");
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
          // An empty frame is nothing to write and, during a catch-up,
          // nothing to count: letting one re-arm the idle watchdog would
          // hand a hostile or broken peer a way to keep this terminal
          // hidden indefinitely at no cost — silence it cannot be caught
          // out on, because the stream never actually goes quiet.
          if (bytes.length === 0) return;
          if (catchingUp) {
            // Held, not written: this is the whole no-visible-re-scroll
            // feature (see this file's header). Both bounds are checked
            // AFTER appending, so the chunk that crosses one is flushed
            // WITH the rest rather than left behind — and the buffer is
            // bounded in frames as well as bytes, because each frame
            // costs an object regardless of how little it carries.
            replayChunks.push(bytes);
            replayBytes += bytes.length;
            testHook.replay.bufferedBytes = replayBytes;
            testHook.replay.bufferedChunks = replayChunks.length;
            const limits = testHook.replay.limits;
            if (replayBytes > limits.bufferBytes) {
              endCatchUp("size");
            } else if (replayChunks.length > limits.bufferChunks) {
              endCatchUp("chunks");
            } else {
              armIdleTimer();
            }
            return;
          }
          writeBytes(bytes);
        };
        // A detach notice is immediately followed by the server closing
        // the socket; the close handler must not clobber the more
        // specific banner (the takeover message is the one SPEC.md
        // requires the user to see) — `showBanner`'s own stickiness
        // handles that.
        //
        // Both endings also end the catch-up phase, and that is not
        // belt-and-braces: a client-initiated detach gets no notice at
        // all, and an attach that fails server-side closes the socket —
        // so the socket dying is, on those paths, the only signal this
        // island will ever get that no more bytes are coming.
        ws.onclose = () => {
          endCatchUp("closed");
          showBanner("Connection closed");
        };
        ws.onerror = () => {
          endCatchUp("error");
          showBanner("Connection error");
        };

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
        // Registered after the selection sweep exists, because an
        // intercepted paste has to run it by hand — the interception
        // stops the event before it can reach the listener just above
        // (see `installAttachments`).
        // `baseUrl`, not the `base` the socket was built from: uploads are
        // ordinary HTTP to the same origin the rest of this UI's API calls
        // go to, and an empty base (origin lookup failed) leaves a
        // relative URL, which resolves against the page itself.
        // The liveness callback closes over THIS mount's socket, which is
        // what makes it honest: an island whose socket the supervisor
        // detached (a takeover, a stall, a dead terminal) still has a live
        // xterm instance, and `term.paste()` into it would swallow the
        // path with nothing to show for it.
        attachments = installAttachments(
          spec,
          baseUrl,
          attach,
          term,
          dismissSelectionSoon,
          () => ws.readyState === WebSocket.OPEN,
        );
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

        // Focus is deliberately NOT placed here. It is a per-PAGE resource
        // that belongs to the island `sync()` last named as focused —
        // several terminals mount together when a session view with open
        // tabs is first rendered, and the last one to finish is not the
        // one the user selected — but an island is `visibility: hidden`
        // for its whole catch-up now, and both engines refuse `focus()` on
        // a hidden element. So the placement moved to `reveal()`, which is
        // the first moment it can take effect and also reads the
        // then-current selection rather than the mount-time one.
        //
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
          attachments,
          testHook,
          // Everything `unmount()` needs to leave the catch-up
          // presentation behind: the invalidation that stops deferred work
          // from reaching a disposed terminal, and the placeholder element
          // this mount may still be holding open.
          disposeCatchUp,
          connecting: spec.connecting,
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
        if (attachments) attachments.dispose();
        // Before the disposal below, not after: a still-armed idle timer
        // would otherwise flush this attach's buffer into the terminal
        // this line is about to destroy.
        if (disposeCatchUp) disposeCatchUp();
        if (ws) ws.close();
        if (term) term.dispose();
        // The catch-up presentation goes back too, so the pane shows the
        // banner below over an empty terminal rather than over a
        // "connecting…" line that will never resolve.
        showTerminal(el, spec.connecting);
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
      // Same class of leak, plus a live one: the paste/drop listeners sit
      // on an element Dioxus keeps, so a remount would stack a second set
      // on top of the first and upload every dropped file twice. Disposing
      // also aborts any upload still in flight, which is what stops a
      // transfer from completing into a terminal that no longer exists.
      if (island.attachments) island.attachments.dispose();
      // The catch-up's deferred work is the same class of hazard one step
      // further along, and it has to be invalidated BEFORE the disposal
      // below rather than merely alongside it. Two things outlive this
      // island otherwise: the idle timer, which would flush a buffered
      // replay into a disposed terminal, and any `term.write()` completion
      // callback still queued — xterm.js runs those after `dispose()`, so
      // a stale reveal would un-hide a DOM node the NEXT mount is already
      // using for its own catch-up, showing a half-replayed terminal.
      island.disposeCatchUp();
      // And the presentation itself, for the reason the attachment status
      // node is cleared above: both elements belong to the PANE, which for
      // the agent terminal outlives every remount, so a teardown during
      // catch-up would leave "connecting…" painted over an element that
      // nothing is going to reveal.
      showTerminal(document.getElementById(el), island.connecting);
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
