# A finished task handle can be published after host purge

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A completed uninstall task can leave bookkeeping reinserted after the host was already removed.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

On the actual multithreaded Tokio runtime, the child can finish and purge host memory before its descheduled parent
queues the mutex acquisition that publishes its handle. FIFO mutex ordering does not order a parent not yet queued. The
complete conditional source route is established; occurrence, retained size and resource impact are unmeasured. Direct
cancellation of a live replacement run was independently refuted.

Source locations: `crates/farhelm-helm/src/provisioning/service.rs:1813-1821`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d. Status/provenance update, preserving f4 and its prior
correction. Parent accepts the distinction between a permissible schedule and unmeasured occurrence/frequency. Parent
independently checked the production helm Runtime::new call at lib.rs:2185 and pinned Tokio1.53.2 Runtime::new
multithread implementation:145-176: worker concurrency or OS preemption before the launch parent queues for the
provisioning-memory mutex supports the explicit source schedule. That parent source check is reported provenance, not an
observation of this race, and this reviewer has not independently read the pinned Tokio implementation. Ordinary Remove
and uninstall's in-child purge remain the concrete producers. The stronger f2 live-work-abort route remains separately
excluded by the provisioning guard. Confidence: confirmed source ordering gap/permissible schedule; occurrence,
frequency, retained-byte cost and practical memory growth remain unmeasured. Suggested bucket: other; pre-existing at
exclusive base da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c service.rs:1668-1676. proposed_drop: none. possible_cover: none
identified; parent a030 receives the schedule/confidence correction for independent reconciliation. No security or
user-work-loss consequence is claimed.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g140/f4`, `a030/g140-f4`, `a054/g140-f4`. These
are original reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent
verdicts remain in the private review evidence. No fix or triage decision was executed.
