# Brace-bearing literals have a Bash-inexact display

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The displayed command can give a brace-bearing literal suffix a different apparent Bash meaning from the actual
argument.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

This is a source-confirmed display/quoting mismatch, separately from the retained historical-wrapper prompt-literalness
defect. Actual execution arguments remain literal for the checked normal-prefix route; no new execution defect or abuse
example is included.

Source locations: `crates/farhelm-proto/src/session_launch.rs:371-381; crates/farhelm-ui/src/approvals.rs:1186-1197`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a013/f5`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
