# Guarded cancellation test lacks an effect-completion witness

Reviewed commit: f1d71445d629f8ee244a592b2173507e32b8c343

## TLDR

The edited-launcher cancellation regression can reach its manual launch before proving that the browser consumed the
released obsolete preview. That manual launch clears the pending work the test intends to inspect.

## Details

Suggested bucket: **other**. Confidence: **confirmed missing test witness**, not a demonstrated false pass or production
cancellation bug. A stronger claim needs a concrete broken implementation and reachable schedule.

`e2e/tests/github-checkout-composer.spec.ts:590–607,660–684` waits for a route-local flag set after `fulfill` returns.
That flag does not establish browser response consumption. The ready preview, edited name and enabled submit assertions
at `650–654,667–670` were already true before releasing the old response. The immediate request-count assertion at `679`
adds no completion witness. Manual submit at `682` clears `guarded_submit_pending` synchronously
(`crates/farhelm-ui/src/list/create_form.rs:5301–5304`), while preview publication and automatic submission are separate
effects (`3837–3870,6088–6131`).

Add a browser-side witness for the released-response boundary and the relevant cancellation effect before the negative
request assertion and positive control. Preserve the intentionally obsolete reply rather than waiting away that race.

This record covers the edited, still-open form only. The stronger closed-form theory was not established: closure
unmounts the form and its local state/resource/effect (`list/view.rs:3871–3872,3910–3921`;
`create_form.rs:3030,3618–3634,6088–6131`). The preview helper returns data without scheduling a launch
(`api.rs:4126–4150`). No surviving launch-task path was found.

Possible cover: no exact acceptance or existing queue record found. Proposed drop: none for the edited-form oracle gap.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g049/f1` and corroborating `g048/c4`;
cross-group synthesis corrected an initial dismissal. Count once.
