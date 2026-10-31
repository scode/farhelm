# Anonymous container volumes survive removal

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Removing a container can leave its anonymous image volumes behind; whether Farhelm promises to remove them is
unresolved.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The matching removal path omits the volume-removal option, and primary Podman semantics confirm retention. This is a
source-complete conditional cleanup behavior with ambiguous volume ownership/product policy. No general permission to
delete arbitrary named/shared volumes is implied.

Source locations: `crates/farhelm-supervisor/src/container.rs:417-425,663-680`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed d6645f771bee8ed03a66486ef8755834937d759d. Recording repair only. Parent confirms f4 stays OTHER and
policy-ambiguous, retained. Preserve the source checks in event-f4-discovery-final.json and guide/policy checks in
event-f4-policy-pass2.json: image-created anonymous volumes survive Farhelm's rm without --volumes; neither the explicit
specification nor the complete container user guide settles whether those volumes belong to session cleanup or
intentionally persistent user configuration. Confidence confirmed conditional retention, unresolved product ownership
policy; no security or lost-work consequence asserted. proposed_drop none; possible_cover SPEC.md 3659,3676-3689 and
user-guide configuration/removal wording remain ambiguity, not established exclusion. No new source read, search or
discovery count.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g116/f4`, `a035/f4`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
