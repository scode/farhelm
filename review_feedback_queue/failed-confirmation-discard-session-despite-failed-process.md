# Failed confirmation can discard the session despite a failed process sweep

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Failed create recovery could discard the session needed to retry process cleanup.

## Details

F24 — **possible** — `crates/farhelm-supervisor/src/service/core.rs:9664`;
`crates/farhelm-supervisor/src/service/core.rs:9615` — Failed confirmation can discard the session despite a failed
process sweep

After launch confirmation fails, the supervisor records a process-sweep error but treats successful terminal removal as
enough to delete an ordinary session record. If a detached descendant survives, it loses the session through which Stop
or Delete could retry cleanup. A healthy ordinary confirmation-failure trigger and the complete survivor sequence remain
unverified; fresh-checkout creates retain their records separately. Require both process cleanup and terminal teardown
to succeed before discarding the session, and otherwise preserve retryable cleanup.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:9521 confirms the launched pane through a database transition.
- crates/farhelm-supervisor/src/service/core.rs:9615 retains a failed process sweep only as error context; :9664 gates
  record abandonment solely on successful tmux teardown.
- crates/farhelm-supervisor/src/service/core.rs:13507 deletes the launching record.
- crates/farhelm-supervisor/src/service/sweep.rs:1167 reports signaling/disappearance failures; :1428 propagates a
  portable-sweep error even under Warn policy.
- crates/farhelm-supervisor/src/store.rs:5455 deletes eligible ordinary-session records.
- core.rs:9615-9631 appends an actual process-sweep error to the diagnostic. :9664-9687 nevertheless abandons ordinary
  creates when tmux removal succeeds. :13507-13519 deletes the durable row and retains publication only if deletion
  fails. sweep.rs:1428-1431 returns actual sweep failures independently of Warn.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- TRIAGE_OUTCOMES.md:6119 and TODO.md:492 cover unconfirmed scope teardown with a clean portable sweep; the ignored
  portable-sweep error is a distinct mechanism. SPEC.md:870 requires unconfirmed cleanup to remain retryable.
  Healthy-filesystem acceptance overlaps the initial database-error premise and prevents stronger confidence.
- TRIAGE_OUTCOMES.md:6119-6137 and TODO.md:492-499 concretely address a clean sweep with unconfirmed scope cleanup.
  Their consistency decision may cover this variant, but the explicit-sweep-error trigger is not unambiguously included.
  SPEC.md:2021-2031 accepts local-filesystem failures; no non-filesystem confirmation-failure trigger was demonstrated.
  Retain this coverage ambiguity explicitly.

Caveats:

- Static verification only; no runtime tests were run.
- Requires confirmation failure, unsuccessful process cleanup, successful tmux teardown, and successful subsequent
  database deletion.
- A sweep error does not itself prove a surviving process; the code nevertheless proceeds without establishing cleanup.
- Fresh allocated checkouts are retained, and the store also refuses deletion of a managed checkout's last membership.
- The inspected files match frozen commit bd8d5d76439f8f36bbc2637b44d7ba4026b65675.
- No ordinary healthy-filesystem confirmation failure was demonstrated: transition error exits are
  storage/decoding/worker failures; missing rows and changed generations are successful outcomes.
- Requires confirmation failure, unsuccessful sweep, successful tmux teardown, successful record deletion, and a
  surviving identifiable descendant for the full consequence.
- A sweep error alone does not prove a survivor.
- Fresh allocated checkouts and protected last checkout memberships retain their records.
- Do not retain the original highest/data-loss classification without additional evidence.
- Requires confirmation failure, unsuccessful process cleanup, a surviving detached process and successful subsequent
  row deletion.
- No production reproduction or non-filesystem confirmation-failure trigger was demonstrated.
- Fresh-checkout creates retain their record through a separate branch.
- The related TODO item is in Maybe later, not Planned; the ledger is the relevant potential coverage basis.
- No runtime reproduction. Requires confirmation-write failure, failed sweep, surviving detached process and successful
  subsequent row deletion. Fresh-checkout creation uses a separate retained-record branch.
- Completed independent confirmation-cleanup adjudication explicitly limits this to possible cleanup ownership loss and
  rejects original highest/data-loss classification absent a healthy-filesystem trigger. Apply that same-site assessment
  to the older duplicate.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_sec_02:p1:F1`, `ss_systems:p1:F1`.

- `gap_supervisor_state_sec_02:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
- `ss_systems:p1:F1`: confidence as filed: possible; suggested bucket as filed: highest.
