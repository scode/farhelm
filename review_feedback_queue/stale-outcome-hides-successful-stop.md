# An old status publication can hide a successful Stop

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

A session that Stop successfully ended can keep appearing to have exited on its own. Refreshing or reconnecting does not
repair the missing stopped-by-user annotation; another lifecycle action or supervisor reload does.

## Details

Suggested bucket: `other`, for persistent status/history error. Confidence: likely; static ordering and repair-path
analysis are confirmed, but the exact interleaving was not executed. Provenance: retained
`supervisor-status-and-recovery-1/f3`, second-turn synthesis. Proposed drop: none. No loss of user work was established.

The ticker commits an observation before publishing its returned outcome
(`crates/farhelm-supervisor/src/service/ticker.rs:1284–1292`). Publication unconditionally replaces the in-memory
outcome (`service/hints.rs:97–100`). Lifecycle transitions likewise commit and then mirror
(`service/core.rs:15163–15184`). The database mutex ends before the async caller resumes (`db.rs:128–142`), so ordering
SQLite commits does not order the later publications. Stop's lifecycle claim does not exclude ticker publication.

One permitted schedule is: Stop classifies the pane as live; the pane exits; the ticker commits `ObservedExit` with a
known code and pauses before mirroring; `StopRequested` leaves that already-Exited durable row intact; the successful
sweep and `StopCompleted` add the stop annotation and publish it; the delayed ticker overwrites that mirror with its
earlier unannotated exit.

An already-Exited outcome with a known exit code emits no further observation (`service/status.rs:561–569`). Other
sessions' transition batches do not refresh this entry (`store.rs:4670–4715`); dead-pane reap excludes Exited entries;
capture refreshes conversation bindings, not this outcome; list and rename read the mirror; wholesale outcome reload is
pre-serve (`service/core.rs:6800–6865`). Thus ordinary refresh and reconnect do not correct it.

Order the commit through publication, or reject publications older than the currently mirrored commit. Verify with
barriers between the observation transaction's return and publication around a fully successful Stop.

Possible cover: the ledger's “cached-launch-error-overrides-completed-stop.md” fixes a different cache-precedence
defect. Its read-side guard does not order publication of two committed outcomes. The ledger's
“commit-window-reads-unpublished-outcome.md” concerns a transient upload refusal message, not persistent session state.
SPEC.md, “Status,” requires a user-initiated stop's annotation. No matching Planned, BUGS, current queue or
accepted-spec coverage was found. The session-history filter requires a crash, recording failure, partly failed
operation or incomplete startup evidence; this schedule contains a fully successful Stop. The rare-glitch filter does
not cover state needing another lifecycle operation or supervisor reload to repair.
