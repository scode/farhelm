# Clipboard-notice draining lacks a per-turn work bound

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The viewer drains concurrently arriving clipboard writes without a per-turn work or cancellation bound. The missing
bound is confirmed; material starvation or wider supervisor disruption remains unverified after investigation.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed structural
work-bound gap; material starvation unverified after investigation

Source locations: crates/farhelm-supervisor/src/service/grid_stream.rs:631-635,680-687,834-838.

grid_stream.rs:834-838 loops until live NoticeSubscription::try_recv returns None; there is no captured end, iteration
budget, yield or cancellation check. The drain occurs before input-failure checks at :631-635 and before the next
emulator read at :680-687. GridSender outer shutdown select at :950-957 can only regain control when the streaming
future yields/returns; it cannot preempt a synchronous poll. At T, hosts/terminal.rs:965-1015 bounds queued notices at
eight but try_recv itself retries Lagged indefinitely and then consumes a live broadcast receiver; :3120-3142 publishes
every live Clipboard notice from a separate runtime task. The emulator does not collapse these before publication:
farhelm-terminal-emulator/src/effects.rs:108-125 collects every notice, :151-163 queues all effects, and :197-213 routes
every live Clipboard effect. The terminal runtime on_output/feed path at hosts/terminal.rs:3226-3284 feeds normal live
output. The same shipped supervisor uses a multithread runtime (service/core.rs:18622-18630, root Cargo.toml:227-238).
Therefore no source invariant prevents independent publication between successive reads or lag retries, so queue
capacity does not impose a bound on one drain or task poll. Exact locked dependency check: T Cargo.lock:6277-6280
selects Tokio 1.53.2; local primary source tokio-1.53.2/src/sync/broadcast.rs:1257-1362 advances against live slot/tail
state and :1549-1552 implements try_recv synchronously without cooperative polling. An already closed producer is finite
and a quiet producer drains normally; those do not bound concurrent live publication.

a program continually issuing clipboard writes can keep its attached viewer draining notices instead of advancing its
screen or observing stream cancellation. The established issue is an unbounded synchronous drain under concurrent
publication, not measured system-wide unavailability. Exact

Possible coverage and caveats: none found. SPEC_impl.md at T:1915-1921 specifies the short lossy backlog, :2442-2482
specifies latest-write coalescing and attachment termination, and SPEC.md:3850-3855 calls OSC 52 an explicitly bounded
effect; none accepts unbounded draining in a stream poll. SPEC.md:2298-2303 permits clipboard failure/drop, not a stall
of unrelated work. TODO.md Planned is empty (:23-25). Searches of unlanded plans/queue, full previously read BUGS.md and
queue INDEX plus matching queue-file phrases found no same trigger/consequence/scope record.
TRIAGE_OUTCOMES.md:5083-5106 concerns native blocking-pool admission under a hung OS clipboard; :5993-6018 concerns
browser HTTP clipboard backlog, fixed by one in-flight plus latest-pending coalescing. Both are different stages and do
not cover supervisor-side broadcast draining. FILTER.md rare diagnostic/display filter is not established coverage for a
continuously replenished synchronous worker poll; hung-clipboard acceptance is also a different trigger. Fix direction:
bound both returned notices and lag retries per drain, coalesce to the latest retained write, and return to the async
scheduler/cancellation checks between batches.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g175/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 70cd68a03c101493e0adbf00bb27bb8004adfe41;
grid_stream.rs is unchanged at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review;
runtime occurrence and frequency were not measured.
