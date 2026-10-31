# Ordinary menu opening can focus before visible placement

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An ordinary menu opening may accept keyboard input before its measured placement becomes visible; the native input/paint
sequence remains unverified.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

Matching Dioxus/Wry and menu focus/measurement paths were investigated. Opening focus can target a command while the
menu is still transparent/unmeasured, but no complete fresh physical-input/paint trace establishes unintended
acceptance. The existing retained-confirmation reopening case has a different producer. Opening focus overriding prior
hover is explicitly accepted and does not settle this ordinary-opening uncertainty.

Source locations: `crates/farhelm-ui/src/menu_panel.rs:199`. Historical revision qualifiers in those references are
retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Same g133/f1 and original opening-over-hover provenance retained. Parent identifies ordinary-opening invisibility as the
same investigated OTHER uncertainty as a059/f7. Preserve one global concern and both provenance IDs. Pinned commit
d6645f771bee8ed03a66486ef8755834937d759d; opacity:0 placement at menu_panel.rs:199 coexists with an ungated ordinary
item-opening focus request at 842-870. The established earlier-hover override alone is accepted by SPEC_impl.md:936-939.
The broader ordinary invisible-opening consequence still lacks a complete consequential native input/paint sequence.
Bucket OTHER, confidence investigated uncertainty, no established highest consequence. proposed_drop applies only to the
accepted focus-over-hover subcase; no blanket exclusion for invisible-opening activation. possible_cover
SPEC.md:1706-1711 preserves the visible-activation bound and is not a consent waiver. This is separate from the retained
confirmation-reopen queue variant. No native experiment or universal absence proof is demanded as a condition of this
source-only review's completion. This reviewer has not inspected a059's private artifact or dependency-source reads; the
cross-group equivalence comes from the parent.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g133/f1`, `a059/f7`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
