# OMP npx tracking guidance omits its qualification

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The OMP guide overstates tracking support for npx launches without the supported launch-chain qualification.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The implementation/recorded decision qualifies the supported chain, while the public guide omits that qualification.
This does not claim every npx launch fails. A documentation TODO outside Planned is not recording coverage.

Source locations: `website/src/content/docs/docs/agents/omp.md:22-24`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed full commit d6645f771bee8ed03a66486ef8755834937d759d. Suggested bucket OTHER. Confirmed narrow wording
conflict: the guide advertises tracking through npx without the Bun-resulting and supported-chain qualifications stated
in SPEC_impl.md:3608-3627; TODO.md:338-343 expressly requests qualifying/correcting this claim. The guide separately
excludes resident wrappers and Node launchers, but does not explain the supported npx launch-chain boundary. Parent
reports fresh a045/f5 agrees this narrow qualification omission is confirmed and retained. The broader claim that every
npx launch fails is NOT made. Source agent_kind/omp.rs:28-86 classifies bare npm/npx as Npm;
procs/omp.rs:298-302,344-400 exempts a readable owned pane and requires recognized middle links. Actual ordinary npx
topology failure remains a separate unestablished premise, unnecessary for this narrow documentation omission. To prove
that stronger claim would require pinned vendor spawn/exec topology and actual image/argv/pane/middle-link locations for
a concrete supported command, or authenticated owned chain evidence; none was established and no vendor was launched.
proposed_drop: none. possible_cover: TODO.md:338-343 is Doc todo, not Planned; TRIAGE_OUTCOMES.md:7138-7143 expressly
leaves the documentation issue separate from its top-process decision, so no whole exclusion. Provenance:
initial/synthesis coverage claim was corrected following parent/a045 feedback; discovery investigated source acceptance
conditions; this revision narrows the retained finding rather than inflating vendor-failure confidence. Earlier C1
events remain.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g082/C1`, `a045/f5`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
