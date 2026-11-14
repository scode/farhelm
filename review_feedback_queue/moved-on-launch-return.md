# A failed launch reopens over work the user moved to

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

Current-main recheck: `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`. Retain high. The moved source still gates
`list/create_form.rs:2071–2120` only for naming; ordinary failures pass no baseline at `6105,6579,7208`.
`SPEC.md:1348–1352,3240–3246` retains the notice requirement, and `SPEC_impl.md:1179–1185` still calls this
implementation lag. No exact later repair or cover was found. The earlier recording-main statements below are
historical.

## TLDR

A failed launch or follow-up question can reopen the launcher and take the keyboard after the user has switched sessions
or started typing elsewhere. It should wait behind a notice the user opens.

## Details

Bucket: **high**. Reviewer confidence: **confirmed by source**, with no runtime reproduction. Original product reviewers
suggested high; supporting test reviewers suggested high or other for their test manifestations. No security, data-loss
or process-loss route is claimed.

Reviewed source change: `112fd9d812e95df7f69f66c85820d76faaf89582`. The behavior followed the automatic-return contract
when that change landed. The later reviewed tip introduces the notice-based focus rule and acknowledges that the UI is
behind; this is a gap against that final target, rather than a claim that the earlier patch violated its original rule.

At the reviewed tip, `crates/farhelm-ui/src/list/create_form.rs:1951-1997`, `admit_composer_return` checks activity only
when supplied a naming baseline. Failure and question paths pass `None`: ordinary create/Replace with at 7056-7090,
sandbox create at 5955-5990, Restart with at 6417-6459, and wake/preparation return through 2004-2014 and 6145-6161.
They reserve modal admission once other dialogs leave. The presentation effect at 5447-5473 restores isolation and
opening focus through 2055-2072. Closing a newer dialog can therefore release that old return even though the
intervening interaction advanced the activity count.

The acted-on session's claims settle before this wait. The local launcher token, draft and pill remain; switching views
does not retire that submitted owner. Those protections keep the destination and retry identity stable, but do not
authorize taking the page back. Successful New/Clone selection and naming have separate activity checks; preserve them.

`SPEC.md:1326-1331` and “Keyboard focus”, “Who may move focus” require a notice after intervening input and no automatic
dialog opening. `SPEC_impl.md:1172-1177` explicitly identifies this gap and directs using the specification as the
target. That acknowledgment is not acceptance. At recording-time main `cb03def00870e18a98a1fd9046073d2849be3253`, the
broad asynchronous-focus TODO has been removed in anticipation of a GUI rewrite, without repairing this path or changing
its product contract. Its earlier Near term presence was not Planned coverage. No exact current queue, decision,
unlanded plan, known-bug or filter cover was established. Whether an unopened waiting notice should keep new launches
refused is expressly unsettled at `SPEC.md:1328-1329`; this finding does not choose that policy.

The regression tests encode the same old return and need to change with the product:

- `e2e/tests/launch-progress.spec.ts:168-187,305-324` requires modal return/focus after switching sessions or opening
  and closing the switcher; `restart-replace-progress.spec.ts:331-357` does likewise after selecting another session.
- `e2e/tests/nonblocking-launch.spec.ts:198-208,318-326,536-542,860-866,1107-1116,1168-1173` requires return after newer
  switcher, Rename or menu interactions.
- `e2e/tests/tensorlake-sandbox-sessions.spec.ts:167-177,240-249` requires modal return after selecting another session;
  `tensorlake-lifecycle.spec.ts:631-679` supplies the related lifecycle-return manifestation.

These assertions were faithful to the earlier contract and need not fail today. Preserve their useful settlement and
readiness coverage while changing return admission. Bind presentation to the initiating activity and retain the draft
behind a notice when that activity is no longer current.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`; canonical `failed-launch-reopens`; records
`g016/f1`, `g018/l1`, `g019/l1`, `g020/f1`, `g021/l1`, `g023/g023-l1`, `g024/l1`, `g025/l1`, `g026/f1`. Original
histories remain in `prep/groups/<group>/findings.jsonl`; they are corroboration of one defect.

## Abbreviated review through 00de0b6c

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g087/l2`.

### g087/l2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 16 (finding, open):

Reviewed introducing revision for this new trigger: 533169cf19571d34af3a8497eaceb8d2d7b7c852. TLDR: A user can start a
fresh-checkout launch that wakes its host, switch sessions and resume typing, then have the failed launcher
automatically cover that work and take keyboard focus when the window's host snapshot misses the new 60-second
connection limit. The error should be offered in a notice after the user moved on. Source route:
create_form.rs:6237-6263 captures press_baseline, pins the submitted host and releases launcher isolation; :6286-6297
parks the successfully woken press and arms the new timer. :7490-7522 calls discard_guarded_preparation when the same
guard-bearing press still waits at expiry. :7461-7488 consumes that slot, sets the error and starts
return_prepared_composer without passing the stamp's input baseline. :2124-2135 calls
admit_composer_return(opening,None), then clears resuming_host. At :2054-2117 the baseline check and activity listener
run only for a non-null naming baseline; the failure return checks opening identity and modal absence, not subsequent
user input, before setting aria-modal=true. resuming_host clearing ends submitted_wait (:3488-3492); error-driven reveal
clears hidden quick presentation (:5595-5603); :5608-5641 restores the revealed modal. restore_composer_modal at
:2175-2193 tests only whether focus is already inside that dialog; active focus in another session therefore triggers
its surface_script. modal_dialog.rs:376-377 and :415-426 resolve the search input and call focus({preventScroll:true})
directly, with no activity/external-focus veto. This settles the original lead's focus premise, beyond merely reserving
modality. No alternate notice handoff or parent-owned activity gate occurs in this source route. Contract, exact pinned
SPEC.md:1341-1345: 'A failure, unknown outcome or follow-up question restores the same launcher with its draft and
ordinary retry rules if the user has not moved on since submitting it. Otherwise a notice offers it, and the launcher
returns when the user opens that notice'. Keyboard focus :3226: 'Once the user has moved on, the result moves no focus,
opens no dialog and switches no session.' SPEC_impl.md:1181-1187 acknowledges 'a launcher owed back after the user moved
on should wait behind a notice instead of returning by itself' and instructs 'Treat the spec as the target when changing
any of these paths.' This is an acknowledged implementation gap, not acceptance of the mismatch. The new Creation
wording saying the timeout returns with its draft supplies no explicit moved-on exception. Suggested bucket:
high/material UX from an asynchronous modal and redirected typing; no mutation is claimed. Could-be-highest: false; no
destructive action, security effect or loss-of-work route established. Confidence: confirmed source/contract analysis,
not runtime observed. proposed_drop: false; no parent drop/lowering accepted. possible_cover: supplied g001/f1 is a
separate queued host-add focus defect; it quotes the same documented gap but does not report this launcher-timeout
route. Attribution limits: the unguarded general return helper predates the exclusive base (base
create_form.rs:1950-2014 already has naming-only activity checks and failure returns); do not claim this commit
introduced all launcher return theft. Its actual newly added timer reaches that helper from the previously parked
connection wait even with unchanged draft/authority. Full range -G history identifies only 533169cf for
WOKEN_CONNECTION/these helper changes; its parent has no timer between discard and pending-submit effects. The complete
form diff from 533169cf to inclusive 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db changes only restart result typing,
sandbox warning and a test fixture; it leaves this timeout/return/focus route unchanged. Caveats: the window must still
await its woken connection at timeout; another modal delays the unwanted return until it closes, and a genuinely retired
opening prevents it. Cancellation or successful continuation consumes the slot first and is outside the route. No
runtime tests, reproduction or live-install actions. All readily checkable source premises resolved; no unresolved lead
remains for this ID.

Returned proposal (claims-057, event 16): **duplicate**, suggested bucket **high**.

The new wake timeout reaches the already queued failure-return admission defect. The existing item explicitly includes
wake/preparation return, the same missing activity baseline, modal admission and focus consequence. Preserve the newly
introduced timer as useful extension evidence in that item rather than a second root finding.

Original stronger consequence: Lead alleged possible focus theft; later source checks established both modal reopening
and direct focus transfer after switching sessions/typing. Keep that consequence. No original record established
mutation, security, data loss or process loss, and no consequence was lowered out of highest.

Unresolved premise or limitation: No missing source premise for unwanted automatic return/focus. No execution or
destructive follow-on sequence is established; do not infer security or user-work loss from redirected typing alone.

Proposed basis: Exact same underlying admission defect already queued at high. Extend moved-on-launch-return.md with
533169cf's timer trigger and its still-waiting-connection caveat. This is a consolidation proposal, not authorization to
drop independently.

Existing parent disposition (event 16): verified. Parent read complete claims057 and full existing
moved-on-launch-return item. Newly introduced533169cf one-shot60s wake timer is a useful caller extension of the same
existing naming-only activity admission defect. At7462-7487/7519 the failure return passes no
pending.press_baseline;2071-2135 no-baseline admission and5595-5634 isolation/direct focus can reopen over later session
typing. Preserve still-waiting matching press, consumed/cancelled/retired opening and later-modal caveats; no
destructive follow-on or highest consequence. Extend existing high item with timer provenance rather than duplicate or
dismiss useful new scope.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g087 record](records/review-through-00de0b6c/groups/g087.json). Record ID: `l2`.
