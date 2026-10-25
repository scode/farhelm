# Uninstall completion bypasses a required maintainer size decision

Reviewed commit: 22059381631d91901a44fb70c49dab2f9ab24223

## TLDR

The uninstall work was marked complete for unattended landing after exceeding a size gate that required returning to the
maintainer. The committed record contains the executor's justification, not the required maintainer decision. No product
damage is established.

## Details

`plans/reports/uninstall-mixed-spelling.report.md:34–39` admits 160 counted lines against the roughly 100-line gate, and
`45–47` substitutes a possible later revert for blocking. The same-revision plan,
`plans/queue/uninstall-mixed-spelling.md:21–22,59–69,277–284`, requires block-and-return and records no Decisions
override. `scripts/plans-queue.py:969–980` accepts delivery from in-flight, stores the report and marks complete.
`plans/.agents/landing.md:91–116` directs the monitor to claim and land complete stacks without an additional
size-decision prerequisite. Thus the status transition grants concrete unattended landing eligibility; it does not
merely describe unfinished work.

Suggested bucket: **other**, maintainer workflow correctness. Confidence: **confirmed committed gate/state mismatch**. A
landing reviewer might notice and block, and the code fix may be correct. Neither supplies the missing decision. The
original private conversation was not authenticated, so this is limited to the pinned record, not a claim that no
maintainer ever agreed elsewhere.

Possible cover: `TRIAGE_OUTCOMES.md:17567–17593` preserves the gate and the executor's explanation, without a maintainer
waiver. No exact acceptance, Planned, BUGS, queue or filter match was found. Proposed drop: none. Reconcile the recorded
authorization rather than assuming a line-count overrun itself implies a defective implementation.

Provenance: trailing review through `616d79f45d6a1d817833291429d144a5c686bb4b`, private run
`4fae501c-3686-4822-aa6c-037266637632`, `g007/f2`, independently repeated as `g017/f1`. Static review only.

### Additional approximate-size decisions

The path correction above comes from immutable commit `22059381631d91901a44fb70c49dab2f9ab24223`: this incident is
`uninstall-mixed-spelling`, not `uninstall-aliased-home`. Its mechanism and original reviewed commit are unchanged.

At `2a879614b1664dbfec0c0223e4458350e545c9bf`, `plans/reports/checkout-ownership-oct22.report.md:47–51` records 108,
108, 117 and 101 counted lines and accepts them through executor judgment. Plan `:74–93` counts code, tests and docs and
requires block-and-return beyond about 100; its final retained snapshot contains no Decisions override. The tolerance of
“about” is undefined, and 101 alone does not establish a violation.

At `732f3a8a90ba1a96926509e89012463df3bb12bc`, `plans/reports/session-view-fixes-oct22.report.md:30–34` similarly
accepts about 130 lines against about 100; the implementation count is 144. Plan `:20,75–77,86–93,303–311` preserves the
prior decision boundary. Later commit `276bc39a941ac311858bc9622e8e5ff3bbebca0d` explicitly approves the report and
removes tracking. The plan is **not still unapproved**. That later acceptance is material context, but differs from a
recorded waiver before the earlier completion transition.

At `293ccf06a5f69bba2d91226100c874334bf6b59e`, `plans/reports/dialog-saves-oct22.report.md:27–34` self-assesses a
123-line trash change. Plan `:21,73–89,303–309` retains its approximate gate. Decisions `:313–369` approve a smaller
number-key fix and say to land it with the other four fixes, which may be broader authorization; they do not disclose or
expressly waive the trash count. Later report approval and undefined tolerance remain possible cover. Explicit 122-line
and larger waivers in other plans apply to those plans, not automatically to this one.

These are additional committed gate/state observations, not new queue items or evidence of product harm. Private
authorization conversations remain unauthenticated. Suggested bucket stays **other**; confidence is confirmed counts and
transitions, with approximate-limit and authorization interpretations unresolved. Provenance: run
`ef6e18fa-8ac0-4850-a557-d59b8af5efb9`, `g139/f1`, `g141/f1`, `g143/f1`, `g197/f1`, `g209/f1`.
