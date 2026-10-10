# OMP transition assertions cannot detect a stale conversation

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Conversation-switch tests cannot distinguish the final target from a stale one.

## Details

F156 — **definite** — `crates/farhelm-supervisor/src/service/core.rs:21566` — OMP transition assertions cannot detect a
stale conversation

Changing transitions reuse the same effective conversation identity, so saving only the first admitted locator can still
satisfy the final binding assertion. The test cannot detect failure to update the Resume target on switches or branches.
Use distinct valid conversation identities and files for changing transitions, and inspect the stored binding after each
transition.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:21532–21534: the contract says the binding names the latest transition.
- crates/farhelm-supervisor/src/service/core.rs:21554–21572: every report uses the same identity and file; only source
  changes.
- crates/farhelm-supervisor/src/service/core.rs:21107–21117: binding observes captured_conversation and ownership
  version.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- The non-first-class-harness filter in review_feedback_queue/FILTER.md concerns rare or unconfirmed integration
  triggers, not this deterministic fixture/oracle defect.

Caveats:

- This is a defective oracle, not proof of an existing stale-conversation product bug.
- No existing stale-conversation product bug is established.
- No mutation test was run.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_03:p1:F3`.

- `gap_supervisor_state_cor_03:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
