# require_no_failure() allows re-running test() on a failed ready P0 pass, clobbering tester state and breaking retry-unavailable

Reviewed commit: 5fcaf735cab90dedf6dc20dd46478f9b0266c3a8

## TLDR

After a P0 check fails or records `could-not-run` on a live P0 test pair, running `brick_test.py test` again on that
pair is no longer refused; if the failure was on the last in-scope check it exits cleanly as `complete` and wipes the
failed attempt metadata needed for `retry-unavailable`, and if earlier it stamps a spurious harness `automation_failure`
on the next check.

## Details

**other.** Source inspection. Main reference `releasing/mac-vm-test/brick_test.py:1402-1406`.

Previously, `Run.require_no_failure()` refused whenever any non-P1 result in any pass had `status != 'pass'`, which
prevented `Run.test_owned()` (`releasing/mac-vm-test/brick_test.py:850-856`, which only checks
`tester['phase'] in ('running', 'reconciliation-required')` and relies on `require_priority` -> `require_no_failure` to
block re-entry of a `'failed'` tester) from running again on a stopped P0 pass until `retry_unavailable()` cleared the
result and moved `tester['phase']` to `'retry-ready'`. In `5fcaf735`, `Run.require_no_failure()`
(`releasing/mac-vm-test/brick_test.py:1402-1406`) dropped the `entry['results']` check for all
`entry.get('priority') == 'P0'` passes (not only released/deleted ones), checking only `preparation_error` and
`automation_failure` for unreleased P0 passes. Because a P0 `fail` or `could-not-run` stops `test_owned()` via
`CheckStopped` (lines 1076-1077) without setting `automation_failure`, re-invoking
`brick_test.py test <run_dir> <p0_pass>` now passes both line 850 and `require_priority()`:

1. If the failed/unavailable check was the last in-scope check (e.g., `p0-fresh/app-start`,
   `p0-updater-upgrade/update-relaunch`, or `p0-installer-upgrade/remote-supervisor-agent` under `p0-only`), line 863
   overwrites `entry['tester']` with `{'phase': 'running', 'check_id': 'native-preflight'}`, `sequence` at lines 899-900
   has no runnable in-scope checks, and line 1094 sets `entry['tester']['phase'] = 'complete'` and exits 0. This
   permanently destroys the `tester` claim (`phase='failed'`, `check_id`, `termination_observed=True`) required by
   `retry_unavailable()` (`brick_test.py:1110-1119`), turning a retriable `could-not-run` into an unretriable pass.
2. If the failed/unavailable check was an earlier check (e.g., `p0-fresh/install`), `test_owned` enters the loop for the
   next check (`app-start`), hits `chain_block()` -> `Refused` at line 921, and the `except BaseException` handler at
   lines 1082-1086 writes a spurious `automation_failure` on `app-start`, which also permanently blocks
   `retry_unavailable()` (line 1114 checks `entry.get('automation_failure')`).

Verify by recording `could-not-run` on `p0-fresh/app-start` (or `p0-fresh/install`) via `test_owned`, then calling
`test('p0-fresh')` again before `retry_unavailable('p0-fresh', ...)`: the second `test()` succeeds (or records a
spurious `automation_failure`) instead of raising `Refused`, and subsequent `retry_unavailable('p0-fresh', ...)` is
refused. Fix by refusing unreleased P0 passes with any non-pass P0 result in `require_no_failure()` (i.e.
`not released and any(result['status'] != 'pass' for key, result in entry['results'].items() if tier(entry, key) == 'P0')`),
and/or rejecting `entry.get('tester', {}).get('phase') == 'failed'` in `test_owned()` line 850.

Reviewer metadata (g03_brick_test_coord_p2/f4): suggested bucket: other (harm is confined to maintainer release-testing
tooling; verdicts remain non-passing, but an accidental re-invocation of `test` destroys `retry-unavailable`
recoverability or fabricates a harness failure).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g03_brick_test_coord_p2/f4`.
