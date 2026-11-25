# A displaced UI channel can lose its Displaced notice to a reader/writer drain race

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

When a second window takes over the helm, the first window is supposed to receive "another window took over" and stay
inert until the user takes the seat back. If the first window sends any frame (its 5-second heartbeat, say) in the
instant the helm closes the old channel, the takeover notice can be dropped and the socket just closes. The first window
then treats it as an ordinary connection loss, reconnects immediately, and takes the seat back from the window that just
took over; the two windows can keep displacing each other. In today's embedded app a second window cannot arise, so this
is latent: it bites the test harness that exercises the seat, and any future remote-helm/multi-window use.

## Details

**other.** Source inspection; commissioner re-read the cited code and confirmed the mechanism. Bucketed by the
established consequence: SPEC.md "One GUI at a time" states a second UI cannot arise in the shipped embedded app ("The
seat is enforced for a future remote helm, and the test harness exercises it"), so today the defect is in the seat
mechanism the test harness exercises. A shipped topology with two live UIs would make it `high` (repeated resets, seat
ping-pong, approvals flapping); it is not a security/data-loss consequence even then.

In `crates/farhelm-helm/src/ui_channel.rs`, the displaced branch of `run` (lines 1025-1028) queues
`ServerMessage::Displaced` with `outgoing.send(...).await` and returns; returning drops `run`'s incoming
`mpsc::Receiver`. In `serve_socket` (lines 1160-1247) the writer (lines 1182-1197) drains the outgoing queue only after
`core` completes (`channel_and_writer`, lines 1198-1207), but the outer select (lines 1240-1243) also polls the reader
(lines 1209-1237). The reader forwards any incoming frame via `incoming_tx.send` (line 1233); once `run` has returned
that send fails, the reader returns, the outer select completes, and `channel_and_writer` is dropped — including the
writer mid-drain, with `Displaced` possibly still queued (a slow socket write widens the window: the writer awaits
`tx.send` under a 10-second deadline). The socket is dropped without `Displaced` (or the Close frame) ever written. The
module's own doc (lines 1166-1169) promises "a displaced channel's last message still reaches the GUI before the socket
closes"; the outer select with the reader breaks that guarantee.

Client consequence (reviewer verified in `crates/farhelm-helm-client/src/channel.rs:513` — `Displaced` → wait for
TakeBack at `:375` — vs `:466` — rx `None` → `Ended::Broken` — and `reconnect.rs:48-56` — `after_break(proven=true)` →
immediate reconnect): a displaced window that misses `Displaced` sees a plain drop, reconnects at once (its link was
proven by heartbeats), and `ui_ws` (`ui_channel.rs:1123`) takes the seat, displacing the new holder. This violates
SPEC.md "One GUI at a time" ("Only the helm's own word that another UI took the seat counts as a takeover; a connection
that merely drops is a reset") and SPEC_impl.md "The seat: one UI per helm". The race SPEC_impl "The seat" accepts is
different: a displaced window's terminal reconnect landing before its Displaced notice arrives — that acceptance
presumes the notice arrives; losing the notice itself is not accepted.

Trigger: any frame from the old GUI arriving between `run`'s return and the writer's flush of `Displaced` — the
heartbeat, or a frame already buffered while `run` handled the displaced branch. Rare per takeover, but an established
mechanism. Verify (read-only review; not run): in the test harness hold two channels, displace the first while feeding a
`Ping` into its socket at the moment of displacement, and observe the first client ends `Broken` (not `Displaced`) and
reconnects. Fix shape: make the drain authoritative once `run` ends — incoming frames must not abort the writer; e.g.
when `incoming_tx.send` fails, the reader awaits `channel_and_writer` instead of completing the outer select, or wait on
`channel_and_writer` alone and signal the reader's socket-end to the writer. Keep `SILENCE_CLOSE` and write-failure
teardown unchanged.

Reviewer metadata (area 1 chunk B, helm UI channel): suggested bucket: other; confidence: confirmed mechanism (read end
to end), race frequency unmeasured; possible cover: none (SPEC_impl "The seat"'s accepted terminal-reconnect race does
not cover losing the notice). Commissioner validation: mechanism confirmed at the cited lines; coverage searched (queue,
TODO.md, BUGS.md, TRIAGE_OUTCOMES.md, plans/queue) — no match.
