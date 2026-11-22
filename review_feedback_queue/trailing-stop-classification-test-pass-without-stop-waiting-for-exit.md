# Stop classification test can pass without Stop waiting for the exit record

Reviewed commit: 85cf6b2e7715c5dce446bcc95170662c1fbc8bb8

## TLDR

A regression that stops waiting for a dead agent's final error output could escape the Stop test.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core/tests/host_terminals.rs:746`.

SPEC_impl.md:2445-2455 requires the already-dead Stop's brief wait before quoting the final screen.
host_terminals.rs:743-773 kills the child, spawns Stop and immediately releases the complaint and exit record, then
checks the durable complete error. If Stop is first polled after the record is processed, those assertions can pass
without exercising its missing-record branch. The separate runtime test (:787-826) proves holds_final_state waits when
directly called, but does not prove Stop calls it. A witness at the production wait boundary would join those two
properties. Bucket: other for the coverage gap. Confidence: source-confirmed assertion gap, no observed failure or
established current production breach. proposed_drop: none. The proposed closure because a new scheduling hook is needed
does not supply equivalent coverage. Possible coverage: stop, including the named lower-level runtime wait test for the
helper only.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_unit/f5`.
