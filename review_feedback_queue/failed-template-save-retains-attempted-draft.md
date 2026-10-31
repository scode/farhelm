# Failed template Save retains the attempted draft

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A failed template Save leaves the attempted draft reusable even though the general GUI rule says failed saves discard
it.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

SPEC.md:3845–3848's any-reason failed-Save rule conflicts with actual template draft retention. Existing tests
intentionally retain it, but tests are not a specification exception. The behavior predates the exclusive base. A later
explicit resave is required; no automatic replay or lost persisted work is claimed. The launcher Save-as-template draft
rule at 824–825 is a different surface.

Source locations: `crates/farhelm-ui/src/list/templates.rs:2167`. Historical revision qualifiers in those references are
retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Discovery scope clarification for existing a063/f3, reviewed d6645f771bee8ed03a66486ef8755834937d759d. SPEC.md:824-825
explicitly says errors preserve the launcher draft for the launcher's Save as template action. That is a named, separate
surface and must not be reported as another violation of the general failed-GUI-save rule. It does not settle the
Templates dialog's definition editor error branch at list/templates.rs:2167, which is the sole retained f3 scope. The
definition-editor behavior remains a pre-existing OTHER confirmed source/spec conflict at the already recorded base and
tip lines. proposed_drop none for this narrow f3; possible_cover: launcher Save as template exception is explicitly out
of scope, not coverage for the Templates editor. Prior events and attribution remain preserved. No additional
failed-save surface or automatic replay finding is added.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a063/f3`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
