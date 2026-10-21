# Guarded preview widens source deletion consent

Reviewed commit: f1d71445d629f8ee244a592b2173507e32b8c343

## TLDR

Choosing Guarded in Replace with can later delete a source session that has started running, although the click only
authorized deleting an idle source. A delayed checkout preview lets the source change before automatic submission.

## Details

Suggested bucket: **highest**, for stopping user-owned work beyond the click's deletion scope. Confidence: **confirmed
conditional source mechanism**; the complete interaction and its frequency were not reproduced.

In `crates/farhelm-ui/src/list/create_form.rs:6470–6493`, the Guarded answer consumes the question and arms pending
submission. The pending stamp at `1397–1420` does not retain the click-time deletion guard. While a managed-checkout
preview is pending, no operation claim prevents another client from starting the source. Listing updates supply the
changed source at `crates/farhelm-ui/src/list/view.rs:3891–3908`; the guard is recomputed at `create_form.rs:3763–3769`.
An idle or tabs-only source can therefore change from guarded to unconditional deletion under
`crates/farhelm-ui/src/status.rs:389–408`.

When the preview completes, the effect at `create_form.rs:6088–6116` calls the current submit callback, created at
`5295` and dispatching the recomputed guard at `5914`. Matching Dioxus 0.7.10 `use_effect`/`use_callback` sources
confirm that the effect receives the replacement closure after rendering; the original closure does not preserve the
older guard. The existing input stamp and preview checks do not detect this source-status change.

`SPEC.md:1195–1258` and `TRIAGE_OUTCOMES.md:5498–5558` require deletion scope to come from the affirmative click.
Possible cover: no exact acceptance, Planned item, known bug or duplicate was found. The subsecond-input acceptance does
not waive explicit destructive preconditions, and the preview wait need not be subsecond. Proposed drop: none. Preserve
the click-time guard through the wait and let a changed source require another answer or fail that guard.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, independently reported as `g046/f1` and
`g047/f1`; later synthesis reconciled the dependency source and shared mechanism. This is one finding.
