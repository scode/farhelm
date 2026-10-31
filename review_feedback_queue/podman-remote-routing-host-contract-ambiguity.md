# Ambient Podman remote routing has an unclear host contract

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Ambient Podman routing can choose a remote engine despite Farhelm's host-local container description; the intended
configuration boundary is unresolved.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Actual Podman selector/storage source was inspected. The conditional engine selection is established, but deliberate
environment/setup acceptance and the host-local product wording do not clearly settle it. No unauthorized credential
export, wrong-machine destructive action or concrete user-work loss was established. The namespace mismatch's false Stop
success is recorded separately.

Source locations: `crates/farhelm-supervisor/src/container.rs:132-145,316-324; container/account.rs:344-361`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Parent-status revision received before this discovery pass: a035's remote-selector finding remains retained OTHER with
ambiguous coverage; no privacy-drop proposal was accepted. Preserve original g125 ID f5 and all prior route/provenance
events; do not withdraw or treat the deliberate-environment clause as an accepted exclusion. Reviewed
d6645f771bee8ed03a66486ef8755834937d759d. f5 overlaps g116 discovery4's stronger routing/privacy analysis and
parent/a035 owns the combined synthesis. Established source behavior remains inherited
CONTAINER_HOST/CONTAINER_CONNECTION selecting a remote Podman engine; conditional matching remote checkout and
client-side name-only token forwarding were traced previously. No actual unauthorized token access or harmed work was
observed. Current suggested bucket per parent: other. Confidence confirmed conditional routing, with consent/coverage
and any stronger privacy conclusion still scoped to parent synthesis. proposed_drop none accepted; possible_cover
remains SPEC.md3676-3689 deliberate shell-environment intent versus specific session-host and remote-host-boundary
promises. This status update is not a new discovery finding or an independent highest-consequence exclusion by g125.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g125/f5`, `a035/f1`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
