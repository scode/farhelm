# Delayed-launch regressions do not witness their admission wait

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

## TLDR

Several browser regressions can pass without exercising the overlap they claim to protect. They dismiss a newer dialog
before proving the application handled the launch reply or began waiting to return its naming dialog.

## Details

Suggested and recorder bucket: **other**. Confidence: **verified missing synchronization**, with actual false-pass
scheduling unmeasured. This is missing regression evidence, not a confirmed production admission defect. Reviewed
change: `112fd9d812e95df7f69f66c85820d76faaf89582`; relevant code remains unchanged at the reviewed tip.

In `e2e/tests/nonblocking-launch.spec.ts:177-198,287-318,523-538`, the pending-New, non-retired Rename and held-YOLO
cases await HTTP completion and two animation frames, then check state already true before the reply: hidden form and
the newer modal/focus. They close that modal before the later error/question assertions. Neither transport completion
nor two frames proves fetch parsing, the Rust continuation and admission observer ran while it was open. A broken
handler delayed until after closure could satisfy the test. The retired Rename variant instead witnesses form removal
and is outside this finding.

The naming case (`734-764`) observes a route-owned ID, transport completion and sidebar row before closing the host
result. The ID is captured before fulfilment; feed refreshes are independent, and `on_settled` refreshes the listing
before `completed_launch_input` reads current activity (`create_form.rs:2028-2035,7042-7052`;
`list/view.rs:2303-2323, 5528-5533`). Those observations do not prove naming admission has begun waiting. If Close
advances input first, initial admission rejects naming and the test never exercises the blocked-wait recheck it names.
Its later mutation trace cannot establish that earlier wait.

Use a causal settlement/admission-wait witness before negative assertions and modal dismissal; preserve ordinary
successful-return tests that already witness application retirement. The earlier Near term entry naming the three
failure tests has been removed; that removal establishes neither repair nor acceptance. Planned remains empty, and no
current unlanded plan states this repair. Similar queue witnesses apply to different tests/phases; no exact acceptance,
BUGS, ledger or filter cover was established.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, combined records `g024/l5` and `g024/l7`,
exclusive base `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. No runtime tests or delay injection ran. Proposed drop: none.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): nonblocking-launch.spec.ts is unchanged.
Lines 177-198,287-318,523-538 still put transport completion and two frames before already-true negative checks;
:734-764 still ends the naming overlap without witnessing that modal admission began waiting. The former Near term
mention is historical and no current unlanded plan states this repair. The synchronization gaps are source-confirmed,
without a production-defect or runtime false-pass claim.
