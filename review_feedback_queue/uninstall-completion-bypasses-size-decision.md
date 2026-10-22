# Uninstall completion bypasses a required maintainer size decision

Reviewed commit: 22059381631d91901a44fb70c49dab2f9ab24223

## TLDR

The uninstall work was marked complete for unattended landing after exceeding a size gate that required returning to the
maintainer. The committed record contains the executor's justification, not the required maintainer decision. No product
damage is established.

## Details

`plans/reports/uninstall-aliased-home.report.md:34–39` admits 160 counted lines against the roughly 100-line gate, and
`45–47` substitutes a possible later revert for blocking. The same-revision plan,
`plans/queue/uninstall-aliased-home.md:21–22,59–69,277–284`, requires block-and-return and records no Decisions
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
