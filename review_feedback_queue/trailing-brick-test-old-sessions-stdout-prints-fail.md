# test_owned() prints status 'fail' on stdout after record() rewrites old-sessions fail to 'could-not-run'

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

During a release brick test, if the guest tester reports `fail` on the previous-release `old-sessions` check, the
coordinator saves it as `could-not-run` in workflow state but prints `"status": "fail"` in its live progress line, which
can mislead an operator or supervising agent reporting live progress while the pair is still running.

## Details

**other.** Source inspection. Main reference `releasing/mac-vm-test/brick_test.py:1108`.

In releasing/mac-vm-test/brick_test.py:724-729 (Run.record), commit 5fcaf735 added a rewrite for `old-sessions`:

    if check_id == 'old-sessions' and status == 'fail':
        status = 'could-not-run'
        detail = ('Reported as fail; old-sessions runs on the previous release, so it could not run: '
                  + detail)[:4000]

However, in Run.test_owned() at releasing/mac-vm-test/brick_test.py:1100-1108, after calling
`self.record(pass_id, check_id, result['status'], ...)`, the live progress line prints the un-rewritten
`result['status']`:

    print(json.dumps({'pass': pass_id, 'check': check_id, 'status': result['status']}), flush=True)

Because `result` is the raw guest observation dict, stdout emits
`{"pass": "p0-installer-upgrade", "check": "old-sessions", "status": "fail"}` even though
`entry['results']['old-sessions']['status']` and the decision journal recorded `'could-not-run'` (and `report()` treats
it as `'could not run'`).

Verification: Run `test()` on `p0-installer-upgrade` with the guest returning `status: 'fail'` for `old-sessions`.
Capture stdout and observe `"status": "fail"` while `workflow.json` and `report.json` record
`"status": "could-not-run"`.

Fix: Capture `recorded_status = entry['results'][check_id]['status']` inside the `with self.locked():` block (falling
back to `result['status']` for `native-preflight`) and print `recorded_status` at line 1108.

Reviewer metadata (g02_brick_test_coord_p1/f4): suggested bucket: other (cosmetic/diagnostic live-stdout mismatch in
maintainer release-testing tooling; persisted state and report verdicts are unaffected).; confidence: confirmed;
possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g02_brick_test_coord_p1/f4`.
