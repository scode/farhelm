# Muse capability documentation denies current support

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A capability docstring still says Muse lacks reporting and Resume command support although the current integration
provides both.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The current capability implementation and explanatory docstring disagree. This is documentation correctness, separately
from native same-process producer uncertainty and the intentional older-session missing-Resume exception.

Source locations: `crates/farhelm-proto/src/launch.rs:369-373`. Historical revision qualifiers in those references are
retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed full commit d6645f771bee8ed03a66486ef8755834937d759d. Discovery owner/ID a062/d1. Confidence confirmed;
suggested bucket other. User-facing TLDR: developer-facing documentation incorrectly lists Muse among agents without
conversation reporting or a stored resume command; the shipped GUI and capability values are correct in the inspected
source. The unconditional claim at launch.rs:371-373 conflicts with captures_named_conversation at 410-418 and Muse's
composed resume template at crates/farhelm-helm/src/launches.rs:622. SPEC.md:305-308 and
SPEC_impl.md:6025-6027,6063-6068 explicitly distinguish current Muse reporting/resume support from older saved launches
that keep no resume command. e2e/tests/named-conversations.spec.ts:89-99 expects Muse's reporting capability; UI
launch_controls.rs:576-593 renders the warning from that capability. Historical e84f6b110543cf6848d041a76562bd94de44c5d0
changed Muse's capture capability to true and corrected the enum-level documentation, but left this method's docstring
unchanged, establishing the drift within the reviewed commit range. Coverage searches of both specifications, TODO
Planned (empty), unlanded plans/queue, BUGS.md, queue index/files and TRIAGE_OUTCOMES.md found no exact stale-doc
correction or accepted exception. plans/queue/muse-session-tracking.md:100-131 is implementation context, already landed
per plans/REPORTS.md:32, not an unlanded covering plan. proposed_drop none; possible_cover none. No highest consequence
or runtime defect alleged. Narrow remedy would remove Muse from the unconditional example or explicitly qualify older
stored launches; no code or test change is implied.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a062/d1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
