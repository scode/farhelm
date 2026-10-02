# Provisioning can lose its deadline while draining child output

Reviewed commit: 1ec60cc06baff3442bcdd73760a9ebdc8b08c49c

## TLDR

A provisioning command whose direct process exits can still hold the host setup/update slot forever if a descendant
keeps an inherited output pipe open.

## Details

Found by pre-pr-review-swarm run `20261001-1343-1ec60cc-5337` as `F16 / COR-PROVISION-DRAIN`, tagged **definite**.

At `crates/farhelm-helm/src/provisioning/backend.rs:1530`, `capture_child` bounds waiting for the direct child, then
calls `finish_child_output`, which awaits stdout and stderr drain tasks without the original deadline. A descendant that
inherited either pipe can keep it open after the direct child exits. The cleanup path also tries to derive the
process-group identifier from `child.id()` after waiting may have reaped the child. Payload transfer uses the same drain
helper.

A stuck setup or update can retain the host provisioning lock and one of the four run slots indefinitely. Retain the
process-group identity before waiting and bound child exit, output drains, and cleanup as one operation. Add a
regression with a descendant that keeps an inherited pipe open after its parent exits.

## Review evidence at f087e0b68aed3eb57d90f71b23ef9aa5499cb023

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F18 / COR-PROVISION-DRAIN`, reviewer `correctness_systems`, pass 1. Confidence: **definite**. Review disposition:
**would fix**. Queue priority at recording: **high**.

Anchor against the reviewed commit: `crates/farhelm-helm/src/provisioning/backend.rs:1637`. Recorded from the completed
review without rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: Same
finding as this existing item. TRIAGE_OUTCOMES.md already decides not to fix it and instead record it in BUGS.md,
planned in `plans/triage-restart-takeover-update.md`. Its assessment found no observed realistic trigger: remote
descendants keep ssh itself alive and remain deadline-covered; the residual case requires a local helper retaining the
pipe after ssh exits. It also treats retained process-group identity as a constraint on a future fix, not a separate
current bug. This review evidence does not reopen or supersede that decision.

Provisioning enforces a command deadline while waiting for the direct child process, then waits without a deadline for
its stdout and stderr readers to finish. A command can exit after starting a descendant that retains either pipe; the
child has finished, but the output reader still cannot reach end-of-file. Payload transfer uses the same unbounded final
drain. Cleanup also derives the process-group ID from the child handle, whose `id()` becomes unavailable after the child
has been reaped.

A setup or update can remain Running indefinitely, retaining both its host's provisioning claim and one of four
fleet-wide run slots. Four such failures can block provisioning on unrelated hosts. This is the still-present mechanism
recorded in `review_feedback_queue/provisioning-child-output-drain-deadline.md`, separate from the accepted pre-transfer
SSH setup wait. Retain the process-group identity through cleanup and keep ordinary command deadlines active through
output collection. For transfers, preserve the applicable progress/stall contract rather than adding an overall transfer
deadline. On expiry, terminate the retained group and bound or close the drain tasks so cleanup cannot itself wait
forever. Test a child whose parent exits while a descendant retains stdout or stderr.
