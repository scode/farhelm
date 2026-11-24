# Plain (managed=False) update-relaunch instructions still require candidate local supervisor on P0 pairs, untested by test_guest_workflow.py

Reviewed commit: 5fcaf735cab90dedf6dc20dd46478f9b0266c3a8

## TLDR

The release brick test's non-managed instruction envelope for P0 relaunches still tells the guest tester to wait for the
upgraded local supervisor during `update-relaunch` instead of deferring that check to `local-supervisor-upgrade`, and
the new unit test only checks the managed envelope.

## Details

**other.** Source inspection. Main reference `releasing/mac-vm-test/test_guest_workflow.py:78-90`.

Commit 5fcaf735cab90dedf6dc20dd46478f9b0266c3a8 split local supervisor upgrade verification out of `update-relaunch`
into `local-supervisor-upgrade` for both P0 upgrade pairs (`p0-installer-upgrade` and `p0-updater-upgrade`) and updated
the plain (`managed=False`) `instructions()` contracts in `releasing/mac-vm-test/guest_workflow.py` for the rest of the
new P0/P1/P2 layout (`old-fixture`, `old-sessions`, `local-supervisor-upgrade`, `remote-supervisor-upgrade`,
`local-supervisor-agent`, `remote-supervisor-agent`, `local-old-session`, `remote-old-session`, `p2-feedback-send`).
However, `instructions()` at `releasing/mac-vm-test/guest_workflow.py:335-345` at 5fcaf735 (`guest_workflow.py:338-348`
at tip 1b3fe49af, shifted +3 lines by commit 4816d0041) still emits
`so wait for the connected candidate local supervisor` unconditionally for `update-relaunch`, including when `pass_id`
is `p0-installer-upgrade` or `p0-updater-upgrade`. In `releasing/mac-vm-test/test_guest_workflow.py:78-90` (unchanged
between 5fcaf735 and tip 1b3fe49af), `test_p0_relaunches_leave_the_local_supervisor_to_its_own_checkpoint` only
exercises `managed=True`, unlike `OldSessionInstructionsTests` and
`test_session_launches_use_the_same_model_as_the_guest_tester` which assert parity across both `managed=True` and
`managed=False`. Note that `brick_test.py:913` currently passes `managed_operations=True` on live runs, so this drift
only affects callers of the plain `instructions()` fallback path. Verify by calling
`instructions('update-relaunch', 'p0-installer-upgrade', 'manual')` and observing
`wait for the connected candidate local supervisor`. Fix by branching `instructions('update-relaunch', ...)` on
`relaunch_checks_app_only({'pass_id': pass_id})` (matching `checkpoint_contract.update_instructions`) and asserting both
`managed=True` and `managed=False` in `test_p0_relaunches_leave_the_local_supervisor_to_its_own_checkpoint`.
(Cross-group note: production builder lives in `guest_workflow.py`, owned by `g04b_brick_test_guest_workflow`.)

Reviewer metadata (g07_brick_test_tests_c/f2): suggested bucket: other (maintainer release-test tooling; currently
unreachable while `brick_test.py` hardcodes `managed_operations=True`).; confidence: confirmed (established directly
from source in `guest_workflow.py`, `test_guest_workflow.py`, and `brick_test.py`).; possible cover: none.

Additional source record at `5fcaf735cab90dedf6dc20dd46478f9b0266c3a8`, `releasing/mac-vm-test/guest_workflow.py:342`
(g04b_brick_test_guest_workflow/c1):

Commit `5fcaf735cab90dedf6dc20dd46478f9b0266c3a8` split local supervisor verification into its own P0 checkpoint
(`local-supervisor-upgrade`) on `p0-installer-upgrade` and updated `managed_instructions('update-relaunch', ...)` (via
`checkpoint_contract.py:331-338`) so neither P0 upgrade pair waits on or updates the local supervisor during
`update-relaunch`. The same commit actively updated `instructions()` (`managed=False`) across `guest_workflow.py` —
including removing the `p0-installer-upgrade` local-supervisor suffix at `guest_workflow.py:394-396` (patch lines
369-374) — but left `contracts['update-relaunch']` at `guest_workflow.py:339-341` (`342-344` at `1b3fe49a`)
unconditionally instructing all passes:
`An old app that runs its supervisor detached leaves it running when it quits; the candidate replaces it on first launch, so wait for the connected candidate local supervisor.`
In addition, `test_guest_workflow.py:78-90` (`test_p0_relaunches_leave_the_local_supervisor_to_its_own_checkpoint`)
checks only `managed=True` and misses this drift in `instructions()`. Reachability caveat:
`releasing/mac-vm-test/brick_test.py:913` unconditionally sets `managed_operations=True` for coordinator runs, so
`guest_workflow.py:678` currently dispatches `managed_instructions` rather than `instructions()` in live runs; the
defect affects callers/tests using `managed=False` or any future non-managed fallback.

How to verify: Call `guest_workflow.instructions('update-relaunch', 'p0-installer-upgrade', 'manual')` and
`guest_workflow.instructions('update-relaunch', 'p0-updater-upgrade', 'ordinary')`; both return text containing
`wait for the connected candidate local supervisor`.

Fix: Branch the supervisor sentence in `guest_workflow.py:335-345` using
`relaunch_checks_app_only({'pass_id': pass_id})` (matching `checkpoint_contract.py:331-344`) so P0 relaunches omit the
local-supervisor wait while bonus `upgrade-*` passes keep it, and cover `managed=False` in
`test_p0_relaunches_leave_the_local_supervisor_to_its_own_checkpoint`.

Reviewer metadata (g04b_brick_test_guest_workflow/c1): suggested bucket: other (maintainer brick-test tooling
instruction/test drift in the currently unreached `managed=False` path).; confidence: confirmed (established directly
from `guest_workflow.py:335-345, 394-396, 678`, `checkpoint_contract.py:331-344`, `brick_test.py:913`, and
`test_guest_workflow.py:78-90`).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g04b_brick_test_guest_workflow/c1`,
`g07_brick_test_tests_c/f2`.
