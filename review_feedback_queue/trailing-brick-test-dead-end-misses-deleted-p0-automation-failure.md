# dead_end() misses automation_failure on a deleted P0 pair, crashing p0-installer-upgrade at P2

Reviewed commit: 5fcaf735cab90dedf6dc20dd46478f9b0266c3a8

## TLDR

If an earlier P0 test pair finishes recording its last check and then hits an interruption or reconciliation failure
before exiting, releasing that pair and continuing to the installer pair causes the installer pair to fail with a
harness error at P2 instead of skipping P2 cleanly.

## Details

**other.** Source inspection. Main reference `releasing/mac-vm-test/brick_test.py:1473-1479`.

In `releasing/mac-vm-test/brick_test.py:1473-1479` (`Run.dead_end`), P2 (`p2-feedback-send`) is only considered dead for
an earlier P0 pair in `p0_sequence` when either (a) a recorded P0 result has `status != 'pass'`, or (b)
`pair['phase'] == 'deleted' and set(part['results']) != set(part['required'])`. It does not check whether a deleted P0
pair carries an `automation_failure` (or `preparation_error`). That state is reachable whenever the coordinator is
interrupted (`KeyboardInterrupt`, `BrokenPipeError` on `print` at line 1078, or `SIGKILL` followed by `reconcile_tester`
at line 1290) after `self.record(...)` saves the last P0 check (`app-start` on `p0-fresh` or `update-relaunch` on
`p0-updater-upgrade`, line 1070) and before `tester['phase'] = 'complete'` (line 1094). After the maintainer releases
that failed pair via `cleanup --discard-failed` (`phase == 'deleted'`), `require_no_failure()`
(`releasing/mac-vm-test/brick_test.py:1402-1406`) allows `p0-installer-upgrade` to proceed. When
`test_owned('p0-installer-upgrade')` reaches `p2-feedback-send`, `settle()` calls `dead_end()`, which returns `None`
because all P0 results are `'pass'` and `set(results) == set(required)`. Immediately afterward, `chain_block()`
(`releasing/mac-vm-test/brick_test.py:1493-1494`) calls `p0_executable_complete()`, which calls `verdict()` on the
deleted pair; `verdict()` (`releasing/mac-vm-test/brick_test.py:1624-1626`) sees `automation_failure` and returns
`'could not run'`. `test_owned` then raises `Refused('complete all P0 checks with valid evidence before P2')` at line
921, which the `except BaseException` handler at lines 1079-1092 records as a spurious `automation_failure` on
`p0-installer-upgrade` itself, breaking clean completion and normal `cleanup()` of `p0-installer-upgrade`. Verify in
unit tests by giving a deleted `p0-fresh` all `'pass'` results plus an `automation_failure` on `app-start` (or
`native-preflight`), then running `settle()` / `test_owned()` on `p0-installer-upgrade`. Fix by also treating
`pair['phase'] == 'deleted'` with a P0-affecting `automation_failure` (i.e.
`check_id == 'native-preflight' or check_id in part['required']`) or `preparation_error` as a dead end for P2 in
`dead_end()`.

Reviewer metadata (g03_brick_test_coord_p2/f2): suggested bucket: other (harm is confined to maintainer release-testing
tooling; it causes a spurious harness error on the installer pair rather than any false pass or user-facing issue).;
confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g03_brick_test_coord_p2/f2`.
