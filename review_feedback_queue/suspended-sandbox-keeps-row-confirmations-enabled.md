# Retained row questions remain enabled after sandbox suspension

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

A Stop or Replace question opened before suspension can remain answerable after the normal menu actions disable.
Accepting it dismisses the question and sends a request the backend refuses.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `952183b99721b44fd6fef27b5ca3b38499165b7f`. Original source location:
`crates/farhelm-ui/src/list/row.rs:2355`. Locations below use that revision unless another is named; the assessment was
checked through the reviewed tip.

Open a sidebar Stop or Replace question on a running sandbox, then suspend the sandbox elsewhere. The list-owned
question survives because reconciliation prunes only absent sessions (list/view.rs:1913-1927); closing and reopening the
same row menu does not clear it (:3654-3659). Even after stale state converges, the retained question remains current.

row.rs:1137-1148 derives sandbox_frozen, and ordinary menu actions use it, but confirm-stop, Stop-and-suspend and
confirm-replace at :2336-2375,2457-2511 disable only on busy. Their current-token handlers (view.rs:2522-2572,3040-3051)
also omit frozen availability. Thus correct token binding does not repair this separate predicate. SPEC.md:1465-1467
requires Stop/Restart/Replace disabled while suspended.

Backend routing refuses the request, with no automatic Resume or established process loss. Preserve Cancel and the
question's ownership while disabling/refusing inapplicable answers until Resume restores availability. Suggested bucket
other, confirmed stable source state. The related retained-header/interrupted-card item has different controls and
source locations; menu-owner loss and stale-token bugs are separate again.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g056/g056-inline-frozen`, exclusive
base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private
findings history, dispositions and independent consultations preserve original confidence, suggested buckets and every
revision. No fix or triage decision was executed.

## Abbreviated review through 00de0b6c

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g037/f6`.

### g037/f6

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 13 (candidate, open):

Reviewed commit: 2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa

TLDR: A row's pre-existing Stop or Replace question can retain an enabled answer after its environment enters Deleting. This is additional evidence of f1's missing deletion-phase UI/admission guard, rather than a new independent defect. It leads to an unavailable operation/refusal, not an established successful mutation.

Details: Source checks close the f6 lead. row.rs:2435 disables Confirm stop on busy alone, :2447 does the same for Stop and suspend apart from launch-target exclusion, and :2589 disables Confirm replace only on busy. In contrast the changed Delete answer at :2521 explicitly includes environment_deleting. list/view.rs:6314-6317 deliberately keeps a row's own question out of busy; host deletion in another client or the helm does not hold this client's session claim. Question cleanup at list/view.rs:2248 retains questions by listed ID only, with no host-phase reconciliation; a retained row's question can therefore persist or reappear when its menu is reopened. confirm_stop at :2885-2897 consumes the drawn Stop token, then admit_stop at :2692-2734 checks questions/rename/retirement/session claims but not Deleting before sending the request. confirm_replace at :3529-3542 similarly consumes the token and checks only begin_row_op before do_replace.

Contract is the same as f1: SPEC.md:1521-1523 and SPEC_impl.md:6648 require sessions' actions unavailable while environment deletion runs or has failed. The same f1 retained-row and no-local-request premise is required; a row that disappears loses the question. Manager status for a recorded seal withholds the client (manager.rs:2210-2228), so routing refuses these requests, just as it refuses Rename. Existing f1 already establishes the same missing host-phase availability guard and local admission pattern; a complete fix must include existing question answers as well as fresh Rename/menu controls. Production refusal does not refute the GUI defect.

Suggested bucket: other, as for f1. Could-be-highest: no established security/loss-of-work route. Confidence: confirmed source-level additional route; no runtime observation. proposed_drop: proposed consolidation into f1 as the same phase-availability omission and common guard fix, not a claim of correct presentation and not an accepted parent drop. possible_cover: own f1, original consequences preserved; no coverage-source search. No new independent finding is asserted. f6 lead closed to candidate after checking question lifetime, disabled conditions and local admission. No tests, builds or reproductions performed.

Returned proposal (claims-022, event 13): **retain-verified**, suggested bucket **other**.

Deleting supplies a verified additional phase for retained Stop/Replace answers. Existing suspension coverage does not exactly cover it; it is a useful extension. Rename is another control/admission surface, so consolidation with f1 is editorial grouping, not exact aliasing.

Original stronger consequence: The lead suspected an enabled confirming answer during Deleting. Latest record proposes consolidating into f1 and preserves the avoidable refused operation, with no successful mutation or work loss.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the Deleting/provider extension as other; do not silently discard it as an f1 duplicate. Can be grouped with f1 when recording if the queue file explicitly preserves all controls and phase-specific admission paths. No narrow timing bound is required when deletion fails with a retained row.

Existing parent disposition (event 13): verified. Parent explicitly accepted named retain-verified proposal from claims-022 after inspecting it. Deleting supplies a verified additional phase for retained Stop/Replace answers. Existing suspension coverage does not exactly cover it; it is a useful extension. Rename is another control/admission surface, so consolidation with f1 is editorial grouping, not exact aliasing. Retain the Deleting/provider extension as other; do not silently discard it as an f1 duplicate. Can be grouped with f1 when recording if the queue file explicitly preserves all controls and phase-specific admission paths. No narrow timing bound is required when deletion fails with a retained row. Checked source/results: ["At 2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa, row.rs:2435,2447,2589 disables retained Stop/Stop-and-suspend/Replace answers on local busy (plus launch-target exclusion for suspend), unlike Delete :2521 which checks environment_deleting.","list/view.rs:6314-6317 excludes a row's own question from busy. :2248 retains questions by listed identity, not phase, so a retained row's question survives a Deleting transition.","list/view.rs:2692-2734 stop admission and :3529-3542 Replace answer admission omit Deleting; manager.rs:2210-2228 plus sessions.rs:905-926 prevent routing to a sealed host.","2ef10815 is in range; conditional source assessment, not observed execution."] Current coverage/results: ["Recording-time main 24bc058066fd57848e788b465719b0e6da2f98f7: TODO.md:25-26 Planned is empty.","BUGS.md searched for actual Restart, Deleting, marking, Replace-progress and notification claims; its sole SSH-helper install/update hang is unrelated.","plans/queue searched for retirement/stale/floor/success, Deleting/Rename/question, marking/cleanup, ReplaceProgressGuard and OldTerminalEnded. Matching session-action-leftovers, environment-agent-operations, tmux and nonblocking plans have landed per plans/REPORTS.md; none is an unlanded repair covering these claims.","review_feedback_queue/INDEX.md read in bounded chunks. Matching retained-restart-hides-reboot-card, suspended-sandbox-keeps-row-confirmations-enabled, frozen-sandbox-retains-header-lifecycle-answers and bulk-sandbox-delete-drops-eligible-peer files read; their trigger/consequence boundaries differ as noted per record.","TRIAGE_OUTCOMES.md searched for the actual retirement/stale-success, Deleting/Rename/question, marking cleanup, progress-guard and notification claims; no matching active disposition. Archive not read.","SPEC.md:1525-1528 and SPEC_impl.md:6148-6157 require unavailable Deleting actions. Existing suspended-sandbox-keeps-row-confirmations-enabled.md:19-29 covers the same retained-answer control/admission omission for suspension, not Deleting or container scope. Extend it with the new phase/provider evidence instead of treating its suspension text as exact coverage. frozen-sandbox-retains-header-lifecycle-answers is a different header surface."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "The lead suspected an enabled confirming answer during Deleting. Latest record proposes consolidating into f1 and preserves the avoidable refused operation, with no successful mutation or work loss."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g037 record](records/review-through-00de0b6c/groups/g037.json). Record ID: `f6`.

