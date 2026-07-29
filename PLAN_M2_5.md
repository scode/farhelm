# Farhelm M2.5: terminal-path backpressure

NOTE: This is the plan for milestone 2.5 only. It builds toward SPEC.md using the choices in SPEC_impl.md; where this
document and those disagree, they win. The overall motivation and the coarse milestone ladder live in PLAN.md; only the
current milestone is planned at this level of detail.

## Goal

Give the terminal byte path end-to-end flow control so that a producer faster than the browser can parse degrades to
slow delivery instead of unbounded memory growth or silent discard. SPEC_impl.md already specifies the shape —
`term.write()` completion callbacks drive watermark pause/resume messages over the WebSocket, and the supervisor
throttles its pane reads accordingly — this milestone implements it. The work exists because the M1 review flagged the
terminal path's unbounded queues and because M2 made Farhelm the daily driver: sustained heavy agent output is now a
normal event, not a hypothetical.

The threat is concrete on both ends. xterm.js parses at roughly 5–35 MB/s while a PTY can produce far faster, and
`term.write()` silently discards beyond a ~50MB buffer — the exact "silent data loss" SPEC_impl.md forbids. On the
supervisor side, every hop today is an unbounded queue: the per-attachment forwarder and the per-connection writer both
buffer without limit, and (audited on this host, 2026-07-29) a tmux server whose control client stops draining grows its
RSS without bound — about 3.5 MB/s against a `yes` pane on tmux 3.4.

## User-visible outcome

Almost nothing, which is the point. `cat` of a huge file in a session terminal renders progressively instead of freezing
the tab or silently truncating; the agent keeps running at full speed regardless of how slowly any viewer parses. The
one genuinely new surface: a viewer that stops consuming entirely — a wedged tab, a laptop asleep past its WebSocket
timeout — is detached after a bounded stall with a visible reason, exactly like the existing another-client-took-over
detach. The session itself is never slowed or harmed by a stuck viewer; reattaching replays per the existing rules and
moves on.

## Scope

### In

- **Wire growth, protocol version 4**: pause and resume control messages (client → supervisor), and a stall-detach
  reason on the existing detach notification. The plan's first draft called this additive within version 3; review
  killed that — `parse_control` hard-errors on an unknown `ControlMsg` variant and the connection loop propagates the
  error, so a new message variant is exactly the "cannot be additive" case PLAN_M2's own rule sends to a version bump.
  One bump to 4 in the proto PR, with the same discipline as M2's bump: later M2.5 wire changes must be additive within
  4 or earn their own bump, tolerance tested in both decode directions, and the unknown-variant connection-fatal
  behavior itself pinned by a test so the next milestone cannot re-assume tolerance that is not there.
- **Supervisor flow control**: on pause, stop reading the attachment's control-client stream; on resume, continue. The
  overflow backstop is tmux's own client flow control (`pause-after`, audited below) so a stalled client never grows the
  tmux server's memory. Enabling `pause-after` changes the control-mode dialect: pane output arrives as
  `%extended-output` (with an age argument) instead of `%output`, and `%pause`/`%continue` notifications appear —
  today's parser recognizes none of these and would silently discard all terminal output, so the parser work is explicit
  PR-3 scope with tests on every supported tmux generation, not an incidental detail.
- **Catch-up after a tmux-side pause** is a reattach in all but the WebSocket: the existing snapshot/cutover replay
  machinery assumes an empty terminal, so the client's terminal is reset before the replay lands (normal screen,
  alternate screen, cursor modes, and scrollback all covered by tests). Never replay into a populated terminal; never
  guess at a gap. Within retained history this is lossless; the invariant that makes the extreme case honest — xterm.js
  scrollback capacity at most the tmux history floor, so the catch-up end state is observably equivalent to lossless
  slow delivery — is recorded in SPEC_impl.md (sharpened by this plan) and must be pinned by a test.
- **Bounded queues, including the helm's**: the supervisor's per-attachment forwarder and per-connection writer, and the
  helm's per-terminal `TermEvent` channels and supervisor-connection writer, become bounded. A bound's behavior is
  backpressure (stop pulling from upstream), never dropping frames — with one deliberate exception: the helm's
  per-terminal bound must not block the shared multiplexed supervisor reader (one wedged tab would stall every other
  session's traffic and the control channel itself), so a full per-terminal channel detaches that terminal with the
  stall reason instead of blocking. Each bound is a named constant with a doc comment saying what it protects and why
  the value is safe.
- **Stall detach**: an attachment whose pause lasts longer than a generous timeout (~60s, named constant) is detached
  with a stall-specific reason the client renders. The timeout is a hard maximum pause duration, deliberately not a
  "zero progress" test — between pause and resume the supervisor receives nothing, so progress during a pause is
  unobservable by design. That is sound because a live client's pauses are short by construction: the drainable backlog
  is bounded by the high-water mark plus the in-flight bounded queues (a few MiB), which even the slowest real parser
  clears in seconds. A pause that outlives the timeout is a wedge, not a slow reader. SPEC.md's terminal section now
  records the product-level contract (added by this plan).
- **UI watermarks**: terminal.js's existing high-water seam (today a `console.warn` at 4MiB of unflushed writes) becomes
  the real driver: pause past the high-water mark, resume when `term.write()` completions drain below a low-water mark.
  The byte path keeps bypassing Dioxus state — that bypass is load-bearing.
- **Tests in the same PR as the behavior**: proto golden/tolerance tests; Rust integration tests with a genuinely fast
  producer proving byte-for-byte delivery across pause/resume cycles and proving the stall detach; Playwright coverage
  that heavy output arrives complete and that a stalled client sees the detach reason.

### Out (deliberately)

Everything in PLAN.md's later milestones, notably M3's durability/resume work. Also out: throttling the agent itself
(the PTY and tmux history absorb bursts; the agent is never paused), any persistent-buffer scheme (tmux history is the
buffer), and input-path flow control (keystrokes are tiny and latency-critical; pausing input is never correct).

## Design: how the pieces hold together

The chain has four links, each with its own bound. Browser: xterm.js tracks unflushed `term.write()` bytes; above
high-water it sends pause, below low-water (a fraction of high-water, so the boundary doesn't chatter) it sends resume.
Helm: forwards pause/resume upstream per-attachment; its per-terminal channels are bounded as backstops, and because
they hang off one multiplexed supervisor connection, a full per-terminal channel detaches that terminal rather than
blocking the shared reader — head-of-line blocking across sessions is the one failure this hop must never have.
Supervisor: a paused attachment's reader simply stops pulling from the control client's stdout; the per-attachment and
per-connection channels are bounded so a slow unix-socket consumer propagates backpressure the same way. tmux: the
control client is attached with `pause-after` set, so when the supervisor stops draining, tmux pauses that client's pane
stream instead of buffering (audited 2026-07-29 on 3.4 and 3.7b: RSS flat under a `yes` producer with an undrained
client; without the flag, unbounded growth at ~3.5 MB/s). History keeps accumulating per `history-limit` while paused.
The audit does not yet cover tmux 3.3, the version floor SPEC_impl.md commits to — PR 3 extends the existing
multi-version validation (the `scripts/check-tmux-cutover.py` precedent) to `pause-after` and the `%extended-output`
dialect on a 3.3a build, or surfaces a floor raise as its own decision if 3.3a cannot be validated.

Catching up after tmux pauses is the part that must not be improvised. tmux marks the pane paused and the client must
explicitly continue it; the bytes tmux dropped from the live stream are exactly the bytes the reattach machinery already
knows how to replay from history with a clean cutover. The resume path reuses that sequence rather than growing a second
replay implementation — but as a reattach in full: the replay assumes an empty terminal, so the client's terminal is
reset first, and the result is visually identical to a reconnect. Within the replay floor this is lossless; a stall
extreme enough to overflow `history-limit` degrades to the floor — and because xterm.js scrollback capacity is held at
or below that floor (SPEC_impl.md now records this invariant; a test pins it), the end state is observably equivalent to
lossless slow delivery. No byte a client could ever have retained is dropped by Farhelm code.

The stall detach is what makes the bounded story airtight. A slow client is served indefinitely — its pauses are short,
its progress real. What cannot be honored forever is a single pause that never ends: buffers at every hop stay pinned
for as long as the wedge lasts. After the stall timeout the supervisor detaches the attachment with a stall reason,
frees it, and the session continues unwatched. The timeout is generous because a false detach costs a reattach (cheap,
automatic replay) while a missed one costs memory for exactly as long as the stall lasts.

## Order of work

Each step is a PR on the single stack; tests ride with their behavior.

1. **This plan**, plus the SPEC.md stall-detach contract and the SPEC_impl.md sharpening it forced. Reviewable statement
   of intent before mechanism.
2. **Proto: pause/resume messages, the stall-detach reason, the version bump to 4.** Wire shapes, golden tests,
   tolerance both ways within 4, and a test pinning that an unknown control message is connection-fatal.
3. **Supervisor: honor pause/resume, bound the queues, stall detach.** `pause-after` on the control client; the
   `%extended-output`/`%pause`/`%continue` parser work; the reset-then-replay catch-up path; bounded channels including
   the helm's (with its detach-not-block rule); the timeout; the 3.3a validation. Integration tests with a fast
   producer: byte-exact delivery across pause/resume cycles, flat supervisor and tmux memory during a stall, detach with
   the stall reason, session healthy afterwards, catch-up correctness on normal screen, alternate screen, cursor modes,
   and scrollback.
4. **UI: watermark-driven pause/resume.** terminal.js high/low-water logic on `term.write()` completions, WS messages,
   Playwright: multi-megabyte burst complete and in order with at least one pause/resume cycle observed; stalled client
   shows the detach reason.

## Acceptance criteria

- A multi-megabyte burst reaches the terminal complete and in order through a real helm+supervisor+browser stack, with
  the client demonstrably pausing and resuming during it — pinned by an automated test.
- No unbounded queue remains on the terminal output path — supervisor or helm; every bound is a named, documented
  constant with coverage, and no bound can head-of-line-block traffic for other sessions.
- During a stalled client, supervisor and tmux memory stay flat; after the stall timeout the client is detached with a
  visible stall reason and the session is unharmed.
- The `%extended-output` dialect and catch-up replay are validated on every supported tmux generation, 3.3 included (or
  the version floor is explicitly raised as its own recorded decision).
- Post-pause catch-up leaves the terminal visually identical to a fresh reattach — no duplicated scrollback, correct
  alternate-screen and cursor state — and the scrollback-capacity-at-most-history-floor invariant is pinned.
- Wire changes ride one version bump to 4; decode tolerance holds in both directions within 4, and the connection-fatal
  unknown-message behavior is pinned so no later change can assume tolerance that is not there.
- The full CI gate is green on every PR.

## Risks retired by this milestone

- The M1 review's unbounded-queue debt on the terminal path — supervisor and helm — closed rather than re-deferred.
- The `term.write()` ~50MB silent-discard cliff, now unreachable by construction.
- The untested assumption that tmux tolerates a lagging supervisor; now pinned to `pause-after` behavior audited on 3.4
  and 3.7b (3.3 validation owed by PR 3), with the catch-up path shared with reattach instead of a second bespoke
  replay.
- The false belief that new control messages are additive within a protocol version — now pinned by a test as
  connection-fatal, so future wire growth starts from the truth.
