# OMP context changes after initial eligibility check

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A retained reporter context leaves an unresolved foreground-ownership question.

## Details

F331 — **possible** — `crates/farhelm-supervisor/assets/omp-conversation-v1.ts:74` — OMP context changes after initial
eligibility check

The OMP reporter cancels ordinary delivered identifier changes and changed captured manager identifiers, but eligibility
fields are snapshots. A hypothesized foreground-ownership transition without those changes could leave the retained
context attributed incorrectly. No vendor transition proves reachability, so this is only a possible wrong-conversation
candidate. Establish that transition first, then refresh or validate foreground eligibility at reporting time if
necessary, preserving the existing identifier fences.

## Evidence and triage context

- omp-conversation-v1.ts:74–93 captures eligible context and identity; lines 98–104 cancel for a later eligible
  different ID or a changed captured manager ID. Lines 41–45 state that eligibility fields are snapshots.
  SPEC.md:1490–1496 requires foreground attribution; its stale-file-move allowance at 1521–1525 does not accept another
  conversation's identity. FILTER.md excludes wrong-conversation consequences. Vendor reachability remains unverified.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `sr_edges:p1:C6`.

- `sr_edges:p1:C6`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
