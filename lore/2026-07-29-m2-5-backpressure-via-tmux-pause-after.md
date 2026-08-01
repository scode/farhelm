# M2.5 backpressure: tmux pause-after as the overflow backstop, detach as the stall bound

Decision, 2026-07-29, while planning M2.5: when a browser pauses its terminal stream, the supervisor stops reading that
attachment's control-client stdout and lets tmux's own client flow control (`pause-after`) absorb the overflow; a
client that makes no progress for ~60s is detached with a visible stall reason rather than honored forever; and the
catch-up after a tmux-side pause reuses the existing reattach snapshot/cutover replay instead of a second replay
implementation.

## The alternatives as they looked

Three ways to handle a client that stops draining while a pane produces fast:

1. **Buffer it all** (just stop reading; no pause-after). Truly lossless, but the memory moves into the tmux server.
   Audited on this host the day of the decision: an undrained control client against a `yes` pane grows tmux's RSS
   without bound, ~3.5 MB/s on tmux 3.4. A wedged tab could OOM the component whose whole job is to outlive everything
   else. Rejected on that alone.
2. **Drop output to catch up.** Bounded and simple, but it is exactly the "silent data loss" SPEC_impl.md forbids —
   the screen would be missing bytes no one decided to discard. Rejected.
3. **tmux pause-after + history replay on continue.** Audited the same day on 3.4 and 3.7b: with `pause-after` set,
   tmux pauses the lagging client's pane stream and RSS stays flat under the same producer. The paused gap is
   recovered from pane history through the same snapshot/cutover sequence reattach uses — as a reattach in full: that
   sequence assumes an empty terminal, so the client's terminal is reset before the replay lands, and the user sees
   exactly what a reconnect would have shown. Within the history floor this is lossless; beyond it, it degrades to the
   floor. What makes that floor honest rather than a quiet loss: xterm.js scrollback capacity is held at or below the
   tmux history floor, so a full-depth replay refills everything the terminal could have retained anyway — the end
   state is observably equivalent to having delivered every byte slowly. Option 3 does technically lose bytes option 1
   would have streamed through a wedged client's buffer, but no byte any client could ever have displayed or scrolled
   back to. Chosen, and SPEC_impl.md's "degrade to slow, never to silent data loss" sentence was sharpened to say
   precisely this rather than leaving the equivalence argument implicit.

The stall detach could have gone the other way too: honoring a pause indefinitely never surprises a slow-but-alive
client. The timeout is a hard maximum pause duration — not a zero-progress test, because between pause and resume the
supervisor hears nothing and progress inside the browser is unobservable by design. That is safe because a live
client's pauses are short by construction: the backlog it must drain to cross back under low-water is bounded by the
high-water mark plus the bounded in-flight queues, a few MiB that even the slowest real parser clears in seconds. A
pause that outlives a 60-second timeout is a wedged tab or a sleeping machine, not a slow reader. A false detach costs
one automatic replay on reattach; an honored wedge pins buffers at every hop for as long as it lasts — which is why
the timeout is generous but exists. A per-pause progress heartbeat was considered and declined: it adds chatter to the
hot path to sharpen a distinction the bounded-backlog argument already makes moot.

Also considered and declined: throttling the agent itself (the PTY plus tmux history absorb bursts; pausing the agent
would let a viewer slow the work, inverting who serves whom), and pausing the input path (keystrokes are tiny and
latency-critical; there is no world where delaying ctrl-C is right).
