# The transfer's queue send ignores cancellation, stalling deletes

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Deleting a session whose viewer stopped reading stalls for a long timeout instead of proceeding — the upload transfer
cannot hear the delete while it waits to send.

## Details

Source: pre-pr-review-swarm, area data-transfer, 2026-09-17. Confidence: definite (data-flow p3). Coordinator confirmed
the bare await and the ack call site.

Every potentially-blocking wait in the transfer task observes the cancellation signal — except `send_upload`, a bare
`priority.send(Frame::control(m)).await` (uploads.rs:294-295). With a full 32-message queue (a client that stopped
reading), the transfer parks inside `send` — e.g. on the per-chunk ack (550) — unable to observe a delete/archived
signal until the send completes. A racing delete therefore waits on `finished` while the transfer waits on the queue,
delaying teardown (and everything queued behind the delete's claim) until the writer-stall timeout kills the connection.
Bounded rather than permanent — but the only transfer step with this blind spot, in code whose stated discipline is that
cancellation overtakes queued work.

Suggested fix: race the send against the signal receiver (`select!` on `priority.send()` vs `signals.recv()`), ending
via the existing `end_cancelled` path when the signal wins; the unsent ack/abort is moot because the transfer is over.
