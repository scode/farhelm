# Arriving effort catalog replaces the focused control

Reviewed commit: c47ac88acac73384d77310160c1e78672ef8ec6b

## TLDR

A keyboard user editing effort while the model catalog loads can lose their editing position when it arrives. Their next
key is consumed and focus moves to the dialog container.

## Details

Suggested bucket: **other**. Confidence: **confirmed source lifecycle and missing focus handoff**, conditional on effort
having focus before the reply. Timing and rendering were not measured. Background input stays blocked; no unauthorized
action or loss of user work is claimed.

`crates/farhelm-ui/src/launch_controls.rs:385–419` replaces the segmented buttons with a native select when the offered
list plus Default exceeds five choices. New initially uses an empty catalog
(`src/list/create_form.rs:2610–2614, 3551–3556`) while keeping choices interactive (`6055–6063`). The shipped Claude
catalog offers five efforts (`crates/farhelm-helm/src/launches.rs:39–45,123–126`), supplying a concrete transition
across that threshold.

No handoff follows removal of the focused button. New does not enable root-scroll focus recovery
(`create_form.rs:5912–5930`; `modal_dialog.rs:951–953`). The isolation fallback consumes the next outside-focus key and
focuses the root (`modal_isolation.rs:196–208`). These paths remain unchanged at review tip
`98bb15850f5f5ba1df6ed66873338261c45ff948`.

Keep a stable control kind through an opening, or transfer focus to the replacement only when the removed control owned
focus. Verify the pending-catalog boundary without waiting away the condition being tested.

Possible cover: no exact specification, Planned, BUGS, queue or ledger match found. The Guarded-answer focus finding has
a different removal path; mixed-input race acceptance at `SPEC.md:1556–1561` does not cover catalog arrival. FILTER
coverage is ambiguous because retrying a key or ordinary refresh does not restore effort focus. Proposed drop: none.

Provenance: trailing-review run `47edc805-f43c-4ef2-abb1-0bba27a41c2f/review-98bb1585`, `g031/f1`, verified new
discovery in pass 1; pass 2 found no additional issue.
