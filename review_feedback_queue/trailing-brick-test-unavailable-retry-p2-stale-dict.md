# test_unavailable_retry_cannot_clear_product_failure_or_retry_feedback mutates a stale dict and never tests P2

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

The brick-test coordinator unit test meant to guard against retrying a P2 live feedback send never actually tests P2; it
tests P1 twice because it edits a detached in-memory copy of the workflow state after the first iteration reloads from
disk.

## Details

**other.** Source inspection. Main reference `releasing/mac-vm-test/test_native_coordinator.py:1080-1091`.

In `releasing/mac-vm-test/test_native_coordinator.py` lines 1080-1091 (introduced in
`5fcaf735cab90dedf6dc20dd46478f9b0266c3a8`), `test_unavailable_retry_cannot_clear_product_failure_or_retry_feedback`
captures `entry = self.run.selected('p0-fresh')` once before the `for priority in ('P1', 'P2'):` loop. On the `'P1'`
iteration, line 1087 sets `entry['required']['app-start']['priority'] = 'P1'`, line 1088 calls `self.run.save()`, and
line 1090 calls `self.run.retry_unavailable(...)`. Inside `Run.retry_unavailable`
(`releasing/mac-vm-test/brick_test.py:1138`), `self.locked()` (`brick_test.py:597`) reassigns
`self.data = json.loads(self.path.read_text())`, replacing `self.run.data` with a fresh dict object while the local
variable `entry` still points to the old, detached dict. On the `'P2'` iteration, line 1087 mutates the detached
`entry`, line 1088 saves `self.run.data` (which still has `'priority': 'P1'`), and line 1090 calls `retry_unavailable`,
which reloads `'P1'` from disk and raises `Refused` for `'P1'` a second time. Removing `'P2'` from the
`tier(entry, check) in ('P1', 'P2')` guard in `brick_test.py:1146` would not fail this test. To verify: assert
`self.run.selected('p0-fresh')['required']['app-start']['priority'] == priority` inside the loop after
`retry_unavailable` (it fails on `'P2'` because the reloaded priority is still `'P1'`). Fix: re-fetch
`entry = self.run.selected('p0-fresh')` inside the loop (ideally inside `with self.run.locked():`) before mutating
`entry['required']['app-start']['priority'] = priority` and calling `self.run.save()`.

Reviewer metadata (g08_brick_test_tests_d/f1): suggested bucket: other (harm is confined to maintainer release-test unit
tests; the underlying `brick_test.py:1146` guard currently still rejects `'P2'`, so this is a silent test-coverage gap
rather than a user-facing or catastrophic tooling bug).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g08_brick_test_tests_d/f1`.
