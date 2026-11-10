# Long plan history counts claims from before submission

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

A plan with a long edit history can be treated as overdue using claims made before that plan existed.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `6ca409d17dba9b2578e5dfd272c4e3f1fd64c164`. Original source location:
`scripts/plans-queue.py:1403-1407,1420-1423`. Locations below use that revision unless another is named; the assessment
was checked through the reviewed tip.

When the plan-file history exceeds its 100-revision search, creation becomes unknown. The counter can then count INDEX
claims from before submission even when the INDEX history itself is complete, falsely treating the new plan as overdue.
This is the opposite counter error from old overdue evidence aging out.

Recover or explicitly preserve the true creation boundary rather than silently treating unknown creation as the
beginning of retained INDEX history. Keep the supported long-edit-history replay separate from the 300-transition INDEX
window so one repair cannot mask the other. The picker consumer and current coverage were inspected; no Planned item or
unlanded plan explicitly repairs this cause. Suggested bucket other, source-confirmed; no live planning mutation or
runtime test occurred.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g007/g007-unknown-creation`,
exclusive base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`.
Private findings history, dispositions and independent consultations preserve original confidence, suggested buckets and
every revision. No fix or triage decision was executed.
