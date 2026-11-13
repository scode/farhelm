# Body focus still admits a moved-on Replace question

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

Current-main recheck: `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`. Retain other. `yolo_confirm.rs:234–240` and
`modal_dialog.rs:388–398` still admit body focus after a changed or unknown count. `SPEC.md:3244–3246` and the express
lag acknowledgment at `SPEC_impl.md:1183–1185` remain. No exact later repair or cover was found. The abandoned plan
below remains historical provenance.

## TLDR

A delayed Replace YOLO question can focus and scroll Cancel after the user has clicked elsewhere, if that click left
focus on empty page space. Farhelm already knows the user pressed something and still moves focus.

## Details

Bucket: **other**. Confidence: **confirmed by source**; no browser reproduction or destructive consequence is claimed.
Original reviewers suggested other. Reviewed source changes: `bd2fdb7ff1afd711fa4384a8c1d14c5d67f38fc7` and
`112fd9d812e95df7f69f66c85820d76faaf89582`; the target focus rule arrives at the reviewed tip.

`crates/farhelm-ui/src/list/view.rs:3016-3020,5831-5841` captures and supplies the answer's activity baseline to the
asynchronous inline question. In `yolo_confirm.rs:233-241`, a changed or unknown count takes the generated JavaScript
fallback branch using `target.async_script(None)`; it does not mutate the saved baseline. The common guard in
`modal_dialog.rs:388-398` admits `body`/`documentElement` regardless of that activity. Cancel receives focus and is
scrolled into view at `yolo_confirm.rs:250-251`.

`SPEC.md`, “Keyboard focus”, 3212-3223 says a moved-on result takes no focus. `SPEC_impl.md:1172-1177` names the
body-guard gap and directs using the new rule as the target. Preserve the positive unchanged-count path: the user who
waited for the question should receive Cancel focus. The repair must carry known moved-on rejection through focus
admission; a correct activity count must not become permission merely because no element holds focus.

Earlier possible cover was `yolo-focus-origin`: it named this question and required leaving focus alone after later
input, but also retained today's guard as its fallback, so even that proposed repair was ambiguous. At recording-time
main `cb03def00870e18a98a1fd9046073d2849be3253` the plan has been abandoned and its TODO and the broad
asynchronous-focus TODO removed in anticipation of a GUI rewrite. Neither `yolo_confirm.rs` nor `modal_dialog.rs`
changed after the reviewed pin. There is no landed repair or remaining unlanded-plan coverage from that proposal.

Four older focus findings were explicitly discarded, but their removed-control, reselection and terminal-sync triggers
differ from this changed-count/body-admission route. That limited decision is not a blanket acceptance of all later
focus findings. The implementation specification still names this gap as behind the target. No exact current queue,
ledger, Planned item, accepted bug or whole-finding filter cover was established. Keep the earlier possible-cover
history as provenance, not an active plan or agreed repair.

Provenance: run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, canonical `async-body-focus`; `g006/l1`, `g017/f1`. Original
findings and interface correction remain in `prep/groups/<group>/findings.jsonl`.
