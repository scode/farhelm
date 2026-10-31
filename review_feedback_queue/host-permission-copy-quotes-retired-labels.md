# Host permission copy quotes retired labels

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Specifications and helm refusal text quote old host-permission labels rather than the controls users now see.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The explanatory text still uses here/from-this-host wording after controls adopted short host names and revised labels.
Permission enforcement is unchanged; the retained defect is the documented/refusal wording mismatch.

Source locations: `SPEC.md:2978,3119,3742; SPEC_impl.md:2160,6449; crates/farhelm-helm/src/yolo_guard.rs:30-31,163`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Retain f1, other, confirmed textual mismatch, proposed_drop false, possible_cover none. Reviewed commit
d6645f771bee8ed03a66486ef8755834937d759d; initial assigned patch provenance 46b7faac834a28c57716446d7e591805dfc15775
remains unchanged. The specification and both helm refusals quote the longer YOLO switch name; specifications also quote
the longer ordinary-command switch name. Actual GUI labels are at hosts/settings_dialog.rs:635,655. This drift predates
the assigned capitalization patch and changes neither setting's intended permission semantics. No permission bypass,
wrong target, unintended consent or loss of user work is established. Supporting source-reference correction to the
preceding synthesis event: the GUI-launch check consults row.yolo_without_asking at yolo_guard.rs:69, not :67 (which
selects the matching host row); the agent-launch check is :148. Numbered immutable source reads confirmed both. All
other evidence and coverage notes in preceding f1 events are retained. The later-synthesis pass is complete with no new
verified IDs and no uncertainty IDs; separate discovery remains pending parent audit.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g069/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
