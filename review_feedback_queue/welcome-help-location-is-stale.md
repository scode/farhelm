# Welcome points to Help's old location

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Welcome directs users to the top of the sidebar for Help after Help moved to the footer.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The ordinary Welcome instruction and current footer placement disagree. The source-confirmed navigation wording is
separate from the configured SSH-copy exception considered by the same review group.

Source locations: `crates/farhelm-ui/src/welcome.rs:69-72`. Historical revision qualifiers in those references are
retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Retained after same-identity later synthesis. Reviewed d6645f771bee8ed03a66486ef8755834937d759d; assigned
8f894cc81db3faff836763e7e03a872cabfbcc20 retains this inherited sentence. TLDR: Welcome tells users to find feedback at
the top of the sidebar, while Help is below Settings at the rail's foot. Source: welcome.rs:69-72; app_bar.rs:892-917;
CSS app.css:809-824 absolutely anchors host-rail-footer at bottom:0 within positioned app-sidebar (590-615), so this is
actual placement, not an inference from a class name. SPEC.md:3206-3213 expressly places Help there for desktop and web.
Welcome tests at e2e/tests/welcome.spec.ts:157-239 test links, handoff and reopening but do not check that location
guidance. Base da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c:welcome.rs:69-72 proves it predates the casing patches.
Suggested bucket other; confidence confirmed; proposed_drop none. possible_cover remains steel-sentence-case.md:598-601,
which acknowledges the observation as recorded but names no item or accepted exception. Parent independently reported
the same absence of exact coverage. Repeated targeted search of TODO/BUGS/ledger/index/plans found only this plan
acknowledgment. Planned bucket is empty. No filter applies to this persistent ordinary introductory guidance. No
security, consent, resource-lifecycle or user-work-loss consequence claimed. Fix direction: describe the foot of the
rail or use location-independent wording. No runtime evidence or historical approval authenticated.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g054/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
