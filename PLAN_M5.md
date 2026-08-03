# Farhelm M5: daily-driver polish

NOTE: This is the plan for milestone 5 only. It builds toward SPEC.md using the choices in SPEC_impl.md; where this
document and those disagree, they win. The overall motivation and the coarse milestone ladder live in PLAN.md; only the
current milestone is planned at this level of detail.

## Goal

Remove the two frictions that stand between M4's feature set and using Farhelm as the primary way to run agents.
Reopening a session today visibly re-scrolls the entire retained history — the terminal replays the whole retained
scrollback (at least the current screen plus 10,000 lines per SPEC.md's floor; 12,000 lines as configured today) as if
it were being printed live, and the cost grows with scrollback size (found in post-M3 manual testing). And sessions
cannot be renamed, which daily use wants early: SPEC.md has listed rename in the v1 client surface since the beginning,
PLAN_M2.md deferred it as "M3+", and no milestone ever claimed it until the ladder gap was found while planning M3.

The fix for the first needs one new piece of protocol: a replay-complete marker on the terminal stream, so the client
can tell "catching you up on what you missed" apart from "this is happening now" and present the former as a single
landed-at-the-tail update instead of a scroll animation. The marker rides the terminal stream, which is why this
milestone does not need M6.75's list-push channel — the old ladder bundled them purely for protocol-work batching.

The third ladder item — confirming the terminal selection-dismissal fix on real WKWebView — is MANUAL and PARKED by user
decision (2026-08-03): it blocks nothing, and it is recorded below as the milestone's one manual item rather than scoped
into the build.

## User-visible outcome

- Reopening a session shows no visible re-scroll — no watching history fly by. The catch-up still takes time that grows
  with the retained size (the bytes must transfer and parse), but it lands as one update at the tail instead of a scroll
  animation. Agent terminal and tabs behave identically, and live output that arrives during the catch-up appears
  exactly once, after it.
- A session can be renamed from the session list and from the session view. The new title shows up everywhere the old
  one did — list rows, session view — and in every other connected client via the same refresh that already carries
  status changes. A title the supervisor refuses (control characters) surfaces the supervisor's own words.

## Scope

### In

1. **One protocol bump to 7, all M5 wire vocabulary upfront.** The M2.5/M3 rule stands: new tagged-enum variants are
   connection-fatal to older decoders, so every wire shape lands in one proto PR — vocabulary first, handlers later. The
   vocabulary: a replay-complete marker and the rename request/reply pair.

   The marker is a new unsolicited control message correlated by channel — `ReplayComplete { channel }` — following the
   exact precedent of `Detached`, `UploadAck`, and `UploadAborted`. The alternative, an in-band byte sentinel spliced
   into the terminal data stream, was rejected: replay bytes are arbitrary terminal history, so any sentinel can be
   forged or collided with by content, and the data path's whole contract is that Farhelm never interprets or rewrites
   terminal bytes. A third shape, a new frame kind, was rejected as a structural change to the framing layer for no gain
   over a control message. Ordering needs no new machinery anywhere: the supervisor writes replay data frames and then
   the marker into one pipe, the helm demultiplexes that pipe sequentially, and the browser receives one WebSocket's
   messages in order — the marker's position in the stream IS its meaning, and every hop already preserves it.

   Rename is a `RenameSession { req_id, session_id, title }` request with a `SessionRenamed` reply carrying the updated
   `SessionInfo` (the `SessionCreated` shape: the caller gets the authoritative answer back, not an ack it must follow
   with a fetch). `SessionInfo` is more than a stored row — status is live-probed, tabs are rediscovered, the restart
   offer is computed — so the reply must be built the way `ListSessions` builds one, never by echoing stale or default
   dynamic fields around the new title. No new error vocabulary — `NotFound`, `InvalidRequest`, and the existing kinds
   cover every refusal below.

   Golden and both-direction tolerance tests within 7; the version-skew handshake test grows the new boundary.
2. **Supervisor marker emission at attach-cutover completion.** The attach cutover already has an exact boundary: the
   `%end` of the final refresh block separates bytes represented by the snapshot from bytes that arrive live
   (SPEC_impl.md's handoff ordering). The supervisor emits the marker on the attach's channel after writing the
   pane-mode re-synthesis and snapshot prefill for that attach, before forwarding any live output — so "exactly once per
   attach, after every replay byte, before any live byte" is not a best-effort claim but a consequence of the cutover
   contract. Live output cannot interleave with the tail of the replay for the same reason: interleaving would require
   bytes that are simultaneously in the snapshot and after it. One boundary is deliberately narrow (caught in review of
   the proto PR): "before any live byte" bounds the ATTACH's catch-up only — M2.5's flow-control recovery after a tmux
   `%pause` replays retained history into the same attachment mid-stream, and that recovery arrives as ordinary output
   with no marker of its own. Pause-recovery presentation keeps today's behavior and is out of M5's scope; the marker
   must not be emitted for it, and no consumer may assume history never reappears after the marker. A second narrowing
   settled during the proto review: "exactly once per attach" means once per attach THAT COMPLETES ITS CATCH-UP — an
   attach ended mid-catch-up (takeover, detach, stall) may never receive a marker, because guaranteeing one ahead of an
   abort would be a promise the forwarder cannot keep; the detach itself ends the catch-up phase, and consumers must
   treat it so. Every terminal selector gets the marker — agent and tab attaches share the cutover machinery, so this
   falls out rather than being built twice. A dead-pane attach (exited session, `remain-on-exit`) emits the marker after
   its snapshot like any other; a fresh terminal with nothing to replay emits it immediately. There is deliberately no
   marker outside an attach: it describes the catch-up phase of one attachment, not a property of the session.
3. **Supervisor rename handler.** Server-enforced metadata CRUD: validate, write the new title durably, install it in
   the in-memory session map, reply with the updated `SessionInfo`. Validation IS `validate_create`'s EXPLICIT-title
   arm, no more and no less — a title containing `char::is_control` characters is refused `InvalidRequest` with the same
   one-line-label reasoning (the title is echoed into terminals by `tracing` consumers, so an embedded escape sequence
   is terminal injection), and the title is capped at create's existing field cap (64 KiB, same constant, same reason:
   keep the reply that echoes it structurally deliverable). An empty title is accepted, exactly as an explicit empty
   title on create is: SPEC.md names control characters as THE refusal for a supplied title, and rename inventing a
   stricter rule than create would be an asymmetry SPEC.md nowhere asks for. There is likewise no U+FFFD sanitization
   here: sanitization exists for server-derived titles the caller never chose; a rename is always caller data, so it
   gets the refuse-don't-rewrite treatment. The size cap is a user-visible refusal SPEC.md's Creation section does not
   yet document even though create already enforces it; naming the bound in SPEC.md lands alongside this handler, so the
   spec stops being silently narrower than the implementation, for create and rename alike. Rename of a session that
   does not exist is `NotFound`. Concurrent renames are last-write-wins with no version token — this is one mutable
   metadata field, both writers hold the authority to set it, and inventing optimistic concurrency for a label would add
   a conflict surface no user flow can hit deliberately; the choice is pinned by a test so it is a decision, not an
   accident, and — like the size bound — it is user-visible semantics SPEC.md currently leaves undefined, so the same
   SPEC.md update records it rather than letting a test pin behavior the spec never states. The write is two-part by
   necessity, not choice: the durable row alone is NOT enough, because the supervisor serves `ListSessions` from
   in-memory `SessionEntry` values that are immutable-once-created behind an `Arc` and never re-read from SQLite
   mid-process — a store-only rename would vanish from every list reply until the next restart. The handler therefore
   installs a rebuilt entry (new title, everything else carried over) alongside the durable write, and a test pins the
   renamed title visible in a `ListSessions` reply immediately, same process, no restart. Two edge semantics are settled
   deliberately rather than left to fall out: validation precedes the lookup, so a malformed rename of a nonexistent
   session reports `InvalidRequest` (what the caller can fix), not `NotFound`; and a rename whose reply cannot be built
   (the committed write landed, the live-state readback failed) reports an error whose message says the rename landed —
   fabricating the dynamic fields the reply promises to have probed would be worse, and the caller's next poll shows the
   new title. Beyond the title, rename touches nothing — tmux session names are internal identifiers and stay untouched;
   the create-idempotency fingerprint records the CREATE request as sent, so a later rename does not and must not
   disturb intent-key replay. The new title persists across supervisor restart by the same mechanism as every other
   stored field, and reaches other clients through `SessionInfo.title` on the existing list/detail polls — SPEC.md's
   changes-appear-automatically rule on the interim mechanism, exactly like tab open/close; M6.75's push replaces the
   poll, not this behavior.
4. **Helm marker pass-through and rename plumbing.** The marker becomes a third `TermEvent` variant and rides the SAME
   bounded per-terminal event queue as `Data` — deliberately unlike `Detached`, which travels on an out-of-band watch.
   That contrast is the design: a detach must be deliverable when the queue is full (the stalled-viewer case), while the
   marker is meaningless except at its position between replay bytes and live bytes, so in-queue ordering is not an
   implementation convenience but the feature itself. On the per-terminal WebSocket it forwards as a
   `{"type":"replay_complete"}` text message — the socket already speaks binary-frames-for-bytes,
   text-frames-for-notices (`{"type":"detached",...}`), and one socket's message order is guaranteed, so the boundary
   survives the last hop too. Rename: `POST /api/sessions/{id}/rename` with the title in a JSON body (the verb-POST
   convention of `/stop` and `/restart`), mapped through the existing `ErrorKind`→status table unchanged
   (`InvalidRequest` 400, `NotFound` 404), plus the client convenience method the UI calls. The helm neither validates
   nor rewrites the title — the supervisor is authoritative, and the helm's job is to deliver its words.
5. **UI replay presentation.** During an attach's catch-up phase, terminal bytes accumulate in a buffer instead of being
   written to xterm.js as they arrive; on the marker, the buffer is written as one `term.write`, the terminal is
   revealed at the write-completion callback, and the viewport lands at the tail. Until then the terminal stays hidden
   behind an unobtrusive connecting placeholder — which is NEW surface this milestone adds, not a reuse: today an empty
   xterm mounts visible immediately and the only overlays are detach banners, so there is nothing standing in front of
   the catch-up to reuse. Never a half-scrolled terminal. This covers every terminal in the session view: the agent
   terminal and each tab buffer independently, since each has its own socket and its own marker. Three
   graceful-degradation bounds keep buffering from becoming a hidden-forever terminal: the buffer flushes and the
   terminal goes live if buffered bytes exceed the replay-size estimate the helm's queue sizing already documents (~3
   MiB — an estimate a legitimately wide or heavily styled history can exceed, which is exactly why crossing it degrades
   rather than errors), if the buffered CHUNK COUNT exceeds its own bound (added in review: per-frame allocation
   overhead is what a flood of tiny frames exhausts, and a byte cap alone never sees it), or if an IDLE timeout expires
   — no non-empty bytes AND no marker for the interval (empty frames deliberately do not re-arm it, or a hostile peer
   could hold the terminal hidden forever). Idle-based rather than total-duration deliberately: a slow but progressing
   replay keeps the timer armed and buffering intact, while a stream that has gone quiet without a marker (a lost marker
   is a protocol violation, but the containment must not depend on never having one) flushes promptly. All bounds
   degrade to batched-but-visible catch-up — never to data loss, since the buffer is written, not dropped — with one
   deliberate exception: an idle expiry on a socket that never finished CONNECTING presents the terminal behind a
   connection-failure banner instead of a live-looking surface, because revealing a terminal whose typing would silently
   go nowhere is exactly what SPEC.md's typing-goes-nowhere rule forbids. The no-intermediate-paint acceptance below
   applies to replays within the bounds, which is every replay the suite generates and every ordinary real one.
6. **UI rename controls.** A rename affordance on the session list row and in the session view, using the same
   optimistic-then-poll-corrected pattern the tab strip established: the new title paints immediately, the next poll
   confirms or corrects it, and a refusal surfaces the supervisor's message where the user can see it while the old
   title stays. Input is a single-line text field sent to the supervisor verbatim — no trimming, no client-side
   validation: the supervisor's refusal text is the contract, duplicating its rules client-side would let them drift,
   and rewriting the user's input before sending would be the same silently-altering-caller-data move the supervisor
   itself refuses to make.

### Out (deliberately)

The WKWebView selection-dismissal confirmation — parked as the milestone's one manual item (see Acceptance). Tab rename
(SPEC.md: close is the whole per-tab operation set). Any "reset to derived title" operation — once renamed, a session's
title is explicit forever; create is the only place derivation happens. Rename history or undo. Marker use beyond attach
presentation (the marker drives exactly one transition — a terminal's catch-up phase ending — and no session, lifecycle,
or any other client behavior may branch on it). Status heuristics, profiles, live push (M6.75); multi-host, terminal
auto-reconnect, cursor pagination (M6); archive, spawn, web auth (M7).

## Testing decisions (settled while planning)

Proto changes get the same golden and tolerance coverage every bump has gotten, plus the version-skew boundary at 7.

The marker's contract is pinned at the supervisor integration level against real tmux: exactly one marker per attach,
ordered after every replay byte and before the first live byte — on an agent attach, a tab attach, a reattach to a
session with deep scrollback, an alternate-screen session (visible-snapshot replay path), a dead-pane attach, and a
fresh-terminal attach. The once-per-ATTACH shape matters: a second attach to the same terminal gets its own marker, and
a takeover's incumbent must never receive the replacement's. The helm's pass-through is pinned with the scripted
supervisor peer the client tests already use: marker in-queue ordering relative to data events, and the WebSocket text
message arriving between the replay bytes and live bytes on the real socket.

Rename at the supervisor integration level: the renamed title visible in a `ListSessions` reply immediately — same
process, no restart (the in-memory entry rebuild is the load-bearing half of the write; a store-only implementation
passes every other test and fails this one); persistence across a supervisor kill-and-restart; `NotFound` on a deleted
session; the control-character refusal carrying the supervisor's message; an explicit empty title accepted (pinning the
create-symmetry decision); the last-write-wins outcome of two concurrent renames (both succeed, the store holds the
later writer's title); and a rename during an active attach changing nothing about the attachment. Helm REST tests cover
the status mapping and that the body's title reaches the supervisor verbatim.

Playwright covers the user-visible contracts end to end against the fake agent, with named tests the UI PR cites:
reattach-lands-at-tail for the agent terminal and for a tab (generate scrollback past one screen, detach, reattach, and
assert deterministically rather than by sampling — sampling scroll state can miss frames between observations and proves
nothing about paint. The deterministic form: the terminal element stays hidden until the marker (asserted on its
visibility state), the replay reaches xterm.js as exactly one `term.write` (asserted at the island seam), and the first
visible frame has the viewport at the tail); rename-from-list; rename-from-session-view; rename-refused (a
control-character title, with the supervisor's words visible and the old title intact). The hidden-through-catch-up
assertion is the acceptance made executable: it fails against today's behavior and passes with batching, which is the
whole point of the milestone.

## Order of work

Each step leaves something runnable; later steps only add. Tests ride with their step.

1. This plan, plus the PLAN.md header update it implies.
2. Proto: the complete M5 wire vocabulary, one bump to 7.
3. Supervisor: marker emission and the rename handler, with their integration tests.
4. Helm: marker pass-through, REST rename, client method, scripted-peer tests.
5. UI: replay presentation and rename controls, Playwright coverage.

## Acceptance

M5 is done when all of the following hold, pinned by automated tests except where a manual item is named:

1. Reattaching to a session with scrollback deeper than one screen lands at the tail with no intermediate state shown —
   the terminal stays hidden through catch-up, the replay is one `term.write`, and the first visible frame is at the
   tail — agent terminal and tab alike, pinned by the named Playwright tests.
2. The supervisor emits the replay-complete marker exactly once per attach that completes its catch-up, after every
   attach-replay byte and before any live byte, for every terminal selector and replay path (history, alternate-screen,
   dead-pane, empty) — never for a `%pause` flow-control recovery, whose replay stays markerless by design, and with no
   marker owed to an attach that a takeover, detach, or stall ended mid-catch-up.
3. A rename from either surface is visible in list replies immediately and persists across a supervisor restart, appears
   in other clients via the existing refresh, and a refused rename (control characters) surfaces the supervisor's
   message while the old title stays everywhere.
4. Concurrent renames resolve last-write-wins with both callers receiving success replies.
5. The full CI gate is green on every PR.
6. The parked manual item is recorded for a morning pass: confirm on real WKWebView (macOS desktop app) that the M4
   selection-dismissal fix holds — a select-and-copy followed by paste, typing, and a forced repaint leaves no selection
   painted. PLAN.md's M5 entry carries the suspect (WKWebView holding the selection layer after its ranges are gone);
   the confirmation has no automated stand-in, blocks nothing, and its outcome lands in PLAN.md whichever way it goes.

## Risks retired by this milestone

- The reattach experience stops PAINTING scrollback depth, which is the half of the cost that compounds worst with real
  use: the deeper the history the longer today's visible re-scroll. Transfer and parsing still scale with retained size
  after M5 — the bytes must move and xterm.js must consume them — but they are paid once, behind one batched update,
  instead of being amplified through progressive rendering.
- The terminal stream gains its first ORDERING-SENSITIVE notice: `Detached` already travels the channel-correlated
  supervisor → helm → WebSocket-text path, but deliberately out-of-band (a detach must be deliverable when the data
  queue is full); the marker is the first notice whose position in the data stream is its meaning, proving the in-queue
  shape works end to end before M6.75's push channel leans on richer unsolicited traffic.
- Rename closes one of the two v1 client-surface verbs still unimplemented (archive, deliberately M7's, is the other),
  and its validation seam (mirror create, refuse-don't-rewrite) sets the precedent the M6.75 profile CRUD will reuse.
