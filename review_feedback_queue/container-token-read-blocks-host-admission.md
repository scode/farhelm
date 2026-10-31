# An unbounded token read blocks host-wide admission

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A blocking configured token file can hold up other session creates and deletes across the host.

## Details

Suggested bucket: **high**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

container.rs:90,112–128 reads the configured token synchronously without a size/time/type bound while
core.rs:1994–1997,7636–7666 retains host-wide directory admission. The consequence extends beyond one request or runtime
worker. Deliberate configuration acceptance remains a possible coverage question; the source also admits reasonable
mistaken path selection. Giant-stream resource exhaustion or user-work loss was not established and is not promoted to
highest. No blocking file or resource experiment was run.

Source locations:
`crates/farhelm-supervisor/src/container.rs:90,112-128; service/core.rs:1994-1997,7636-7666; service/handlers.rs:363-373,1166-1179`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reconciliation with g020/f2 at d6645f771bee8ed03a66486ef8755834937d759d. Direct lock-chain check confirms an admitted
synchronous token read can indefinitely block host-wide Create/Delete and read an arbitrarily large file into supervisor
memory. Retain high for an in-scope mistaken token-file selection, but do not use deliberate administrator choice of
/dev/zero/a FIFO or deliberate host file reshaping as an unqualified product trigger: SPEC.md:3676-3689 accepts
intentional host environment/files. Nor does a hostile container's pure host-resource-exhaustion goal establish a new
security guarantee: SPEC.md:3654-3658 and docs:115 explicitly omit resource limits; SPEC.md:3875-3878 declines
availability defense against same-account hostile processes. The GUI field preserves arbitrary absolute input, so
ordinary wrong-file input remains an accessible producer; no evidence establishes how often a user accidentally chooses
an unwritten FIFO or a sufficiently large regular file. Mark mechanism and management-lock consequence source-confirmed,
practical accidental trigger/frequency unmeasured. Copy paths preserve the token name
(create_form/draft.rs:1070-1095,2720-2807; container_controls.rs:44-79), but malicious retargeting is not needed to
claim the remaining mistaken-input correctness issue, and its confidentiality consequence belongs to f3. Coverage is
partial/policy-sensitive, not a blanket drop; proposed_drop false for the remainder, possible_cover deliberate setup
subset only (3676-3689) and no-resource-limit/hostile-availability subset (3654-3658,3875-3878). Suggested bucket high
conditional on a reasonable accidental input; if parent lacks that premise, other with this exact caveat is more
accurate than asserting a confirmed common outage. No OOM-caused work loss or cross-host freeze is claimed, and no
runtime experiment ran.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g019/f2`, `g020/f2`, `a024/g019-f2`,
`a024/g020-f2`. These are original reviewer identities for this concern, not additional bugs. Full revisions,
withdrawals and independent verdicts remain in the private review evidence. No fix or triage decision was executed.
