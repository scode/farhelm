# Deleted sources retain their Replace permission questions

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

## TLDR

A session deleted elsewhere can leave its Replace permission question waiting in this page. If it was the last session,
the stale question hides with the list and can reappear when a new session arrives.

## Details

Suggested and recorder bucket: **other**. Confidence: **verified source route and specification conflict**, not a
runtime observation. Source identity and destructive guards remain captured; no wrong-source launch, deletion, security
or user-work-loss route is established.

The implementation was reviewed in `bd2fdb7ff1afd711fa4384a8c1d14c5d67f38fc7`. Its source-absence omission already
existed on the older sidebar path. The newly applicable contract is introduced by the final in-range specification
commit `6e423e30068b62c6cfa276c6d720ee35bb70387a`: SPEC.md:3276-3278 says a confirmation goes away when its subject is
gone, and `3198-3200` gives that rulebook precedence. The earlier pre-existing-only proposed exclusion no longer settles
this source/spec conflict.

At the reviewed tip, `crates/farhelm-ui/src/list/view.rs:1990-1998` prunes ordinary row questions and marks after
authoritative absence, but not the persistent Replace controller's YOLO lanes. Those questions render independently of
the live-ID set (`5818-5889`), inside the nonempty-fleet branch (`5766-5773`). The controller retains each lane until
its own answer/cancel or whole-list unmount (`replace.rs:204-223,302-327,397`); ordinary absence reconciliation does not
retire its question/error/exclusion.

Reconcile authoritatively absent subjects through the persistent controller without ending unrelated in-flight requests
or newer retry identities. A filtered, truncated or failed listing is not proof that the source disappeared. Preserve
the same complete-fleet absence boundary ordinary questions use.

Possible cover: the current Add-host ledger decision (`TRIAGE_OUTCOMES.md:995-1008`) proposes a broad convenience-state
and dialog-order lag principle. The adjacent discarded delayed-Replace-refusal item (`1010-1020`) concerns reopening
after cancellation while its subject remains. Neither explicitly accepts a confirmation surviving authoritative subject
removal under the final rule. No exact other specification acceptance, Planned item, pending repair, known bug, queue or
filter cover was established.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, `g011/l1`, exclusive base
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. The original provenance-based drop and this contract-based reversal remain
in the private history. Proposed drop: none for the retained conflict. No runtime check ran.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): view.rs:2209-2212 still prunes lifecycle
state, ordinary row questions and marks on authoritative absence without retiring ReplaceController lanes. Its :6149
question rendering remains inside the nonempty-fleet branch, and replace.rs retains the separate question lifecycle. The
later db723470 progress-guard repair changes pending progress rather than unanswered questions. SPEC.md:3303-3305 still
requires subject-gone retirement. TRIAGE_OUTCOMES.md:915-941 concerns delayed live-source convenience and refusal
ordering, leaving its coverage of authoritative deletion ambiguous. The source/spec conflict remains confirmed, without
a wrong-target action or runtime observation.
