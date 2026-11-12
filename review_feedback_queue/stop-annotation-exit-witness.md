# Stop annotation test still assumes synchronous host exit observation

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The test can reject a successful Stop if its immediate listing runs before the host runtime processes the exit record.
This is a conditional source-confirmed race, not an observed flake or frequency claim.

## Details

Source coordinates from the original records: `crates/farhelm/tests/e2e/restart_under_concurrency.rs:224-226`.
References below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

The assigned test blob is unchanged at pinned tip 407a49435278844248d573083fdea6e23fd96c02.
restart_under_concurrency.rs:224-226 awaits Stop, immediately calls listed, then asserts the stopped-by-user annotation.
Stop success already satisfies durable StopCompleted and its in-memory mirror: service/sweep.rs:1965-1995 performs the
sweep then records StopCompleted; service/core.rs:18625-18645 awaits the store transition and mirrors the committed
outcome before return; handlers.rs:927-930,1140-1145 then sends SessionStopped. This is not a missing
durable-stop/reconciliation premise.

The missing named oracle is the same supervisor runtime reaching Liveness::Exited after processing the host exit record.
Stop's exit-code read (`entry.terminal.as_ref().and_then(|host| host.exit_code())` in service/sweep.rs
`stop_live_agent`, which replaced the since-removed `dead_pane_exit_code`) simply reads host.exit_code and explicitly
allows that record to lag the sweep. HostStatus::evidence at hosts/terminal.rs:847-861 remains dead=false while
Liveness::Running, and session_status at status.rs:345-347 hides every annotation under live evidence, regardless of the
already committed StopCompleted. list_all at listing.rs:90-98,184-203 only reads this status; it does not wait for an
exit record or run reconciliation. The Stop reply, durable store write, reply transport and one listing add scheduling
opportunities, but none is an acknowledgement that the independent host runtime processed Exit.

Use wait_for_dead_agent(&h.sup, h.state.path(), id) at this boundary (harness.rs:1471-1515), or a bounded listing
predicate for the annotation, before the assertion and launch-directory permission change. Explicit reconciliation is
unnecessary to make StopCompleted durable here; the existing wait_for_reconciled_listing counterpart at
restart_with_resume.rs:1173-1180 works by also polling the reply-visible predicate. SPEC_impl.md Terminal
host:2022-2024,2106-2123 establishes the processed-exit boundary. Parent SupervisorSeams defaults to Tmux at f42178f^
service/core.rs:1269-1288, and parent harness.rs:1630-1644 passes those default seams, so this host-specific missing
boundary accompanies the migration.

Suggested bucket: other, test reliability only. Confidence: confirmed conditional race by source; no reproduction or
incidence estimate. proposed_drop: false. possible_cover: none. Synthesis removes the prior unmeasured word often
concerning the repointed-directory sibling; that sibling is only an analogous possible race, not independently
established frequency. This finding remains focused on the immediate assertion at 224-226.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g043/f2`. The retained
append-only histories preserve all earlier claims and corrections. Reviewed source references name their own revisions;
the inclusive reviewed tip is the hash above. This review used source inspection only, with no product runtime
experiments or live-install changes.
