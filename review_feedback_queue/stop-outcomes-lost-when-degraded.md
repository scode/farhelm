# Stops silently lose their outcome while the supervisor is degraded

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

While the supervisor is in a degraded state (still serving, but not allowed to write), stopping a session kills the
agent, reports full success, and records nothing — the session can show as still running after it is dead, and "stopped
by user" silently becomes "finished on its own".

## Details

Source: pre-pr-review-swarm, area supervisor-lifecycle, 2026-09-17. Confidence: definite (correctness-data-flow p1).
Coordinator confirmed the degraded-but-serving state, the missing gate, and the sibling-path contrast.

`Supervisor::record()` is the choke point through which stop outcomes reach the database and the in-memory mirror. When
`may_record` is false (no state-dir claim, or the boot-id read failed — states in which the supervisor keeps serving
requests), it hits `if !self.may_record() { return Ok(()); }`
(`crates/farhelm-supervisor/src/service/core.rs:10608-10610`) without writing anything or updating the mirror. Its own
docs promise "Errors are returned, not swallowed" (core.rs:10598-10601).

Every stop-path caller trusts that `Ok`: `handle_stop_session` (handlers.rs:900-1228) contains no `may_record` gate at
all, and `stop_live_agent` (sweep.rs:1726-1752) maps `record()` errors onto purpose-built `StopFailure::Unrecorded`
("refuses rather than killing anyway", sweep.rs:1664-1668) and `StopFailure::UnrecordedOutcome` (sweep.rs:1673-1675) —
mappings that can never fire. Every sibling gates explicitly: restart refuses (core.rs:7677), reports refuse
(handlers.rs:6629-6643), the list path gates its observations (listing.rs:186, status.rs:631/643), capture gates its
writes (capture.rs:614). Note status.rs:599 references a prior review-swarm fix batch that closed the analogous
list-path hole — the stop path was missed.

Consequences during any degraded window: the `StopRequested` intent loss reopens the crash-mid-kill hole the intent
exists to close; the `StopCompleted` loss leaves the durable outcome `Running` for a dead agent, later correctable only
as a bare annotation-less exit; the in-memory mirror goes equally stale.

Suggested fix: return an `Err` from `record()` when it skips the write due to `!may_record()`, naming the degraded
recording state. No caller changes needed: the existing `StopFailure` mapping then refuses live-agent stops before
killing and reports completed ones as killed-but-unrecorded — the behavior those variants were written for.
