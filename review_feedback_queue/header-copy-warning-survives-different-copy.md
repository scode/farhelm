# A copy warning can describe the previous header copy

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Copying a different header field can leave the earlier field's hidden-character warning visible briefly.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The independent feedback path does not clear the previous warning until its timer expires. The copied raw bytes remain
correct. A harmful downstream paste was not established, and the narrow diagnostic filter's rarity condition is
ambiguous for ordinary successive copies.

Source locations: `crates/farhelm-ui/src/session_view.rs:3032-3036`. Historical revision qualifiers in those references
are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Parent disposition from the current instruction: retain f1 as an OTHER source mechanism; rare-diagnostic filter
applicability remains ambiguous, the trigger differs from metadata refresh, and no harmful-paste route is established.
proposed_drop: false. possible_cover: related metadata-refresh ledger/spec text does not fully cover cross-copy overlap;
FILTER.md 24-42 is not established coverage. Reviewed commit remains d6645f771bee8ed03a66486ef8755834937d759d;
attribution remains pre-existing at da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c. Preserve all original source references,
provenance and test limitations in earlier f1 events. The additional separate discovery pass found no new source
mechanism and did not broaden f1's consequence or severity. No withdrawal, security exclusion, runtime validation or
whole-lifecycle claim.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g059/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
