# managed_result rejects permission-block without brick_action receipts, allowing P1 permission blocks to bypass the pair-stop fence

Reviewed commit: 5fcaf735cab90dedf6dc20dd46478f9b0266c3a8

## TLDR

In the release brick test, if the guest tester hits an OS/CUA/authentication permission prompt during UI inspection
before running any fixed command operation, the executor turns `permission-block` into an `automation-failure`. On P1
checkpoints, the coordinator then treats that `automation-failure` as `could-not-run` and continues running the pair
instead of stopping on the permission block as required.

## Details

**other.** Source inspection. Main reference `releasing/mac-vm-test/test_guest_operations.py:266-280`.

Commit 5fcaf735cab90dedf6dc20dd46478f9b0266c3a8 relaxed `managed_result` in
`releasing/mac-vm-test/guest_tester.py:292-298` (same lines at both 5fcaf735 and tip
1b3fe49af86979aebc2719965682a17f8c6366ac) to accept a missing `managed-operations.json` only when
`value['status'] in ('fail', 'could-not-run')`, and added `test_nonpass_observation_imports_without_any_fixed_operation`
in `releasing/mac-vm-test/test_guest_operations.py:266-280` at 5fcaf735 (`test_guest_operations.py:359-373` at tip
1b3fe49af) covering only `('could-not-run', 'fail')` vs `'pass'`. Because `managed-operations.json` is written only by
`Operations.save_record()` when a `brick_action` or receipt runs (never by `brick_ui` calls like `desktop/getState`,
`bind`, or `getScreenshot`), a tester that observes an OS/CUA/login permission prompt on its initial UI call and stops
immediately with `status: 'permission-block'` (as instructed by `guest_tester.py:240,263`) causes `managed_result` to
raise `TesterError('managed tester retained no operation receipts')`. `guest_tester.py:855-898` catches that
`TesterError`, overwrites `observation['status']` to `'automation-failure'` (only `'fail'` is preserved as
`claimed_failure` at line 864), and writes `execution.json` with `status: 'automation-failure'`, erasing
`'permission-block'`. On any P1 checkpoint (`old-sessions`, `local-old-session`, `remote-old-session`,
`remote-host-remove`, `remote-host-uninstall`, or `p1-remote-mac` checks), `brick_test.py:1039-1057` converts
`automation-failure` into `could-not-run` and `continue`s the pair (explicitly assuming at line 1045 that 'A permission
block still stops the pair below' via line 1058), bypassing the `BRICK-TEST.md` fence that OS/CUA/authentication blocks
must immediately end the attempt. On P0/P2/bonus checkpoints it also loses `guest_status: 'permission-block'` and
misreports the error as missing receipts. Verify by calling
`managed_result(task, output, {'status': 'permission-block', 'detail': 'OS prompt'})` when `managed-operations.json`
does not exist and observing `TesterError`. Fix by including `'permission-block'` alongside `('fail', 'could-not-run')`
in `guest_tester.py:295` (and preserving `'permission-block'` in `guest_tester.py:864-868` when another `TesterError`
occurs, paired with the coordinator-side fix in `brick_test.py:1039` reported by `g02_brick_test_coord_p1`), and
updating `test_nonpass_observation_imports_without_any_fixed_operation` (`test_guest_operations.py:368` at tip) to
assert `'permission-block'` imports cleanly without `managed-operations.json`. (Cross-group note: production guest
executor lives in `guest_tester.py`, owned by `g04b_brick_test_guest_workflow` / `g04a_brick_test_guest_ops`;
coordinator side `brick_test.py:1039` was independently reported by `g02_brick_test_coord_p1`.)

Reviewer metadata (g07_brick_test_tests_c/l3): suggested bucket: other (harm is confined to maintainer
release-verification tooling; not a product security/data-loss bug, though it violates a documented brick-test harness
fence).; confidence: confirmed (established directly from source in `guest_tester.py:292-298,853-898`,
`brick_test.py:1039-1065`, `test_guest_operations.py:266-280` [5fcaf735] / `359-373` [1b3fe49a], and `BRICK-TEST.md`).;
possible cover: none.

Additional source record at `1b3fe49af86979aebc2719965682a17f8c6366ac`, `releasing/mac-vm-test/brick_test.py:1039`
(g02_brick_test_coord_p1/f3):

In releasing/mac-vm-test/brick_test.py:1039-1057 (Run.test_owned), commit 5fcaf735 added:

    if priority == 'P1' and check_id != 'native-preflight' and execution.get('status') == 'automation-failure':
        # ... A permission block still stops the pair below, and a failure
        # the tester claimed stays visible for review.
        ...
        self.record(pass_id, check_id, 'could-not-run', [], ..., native=native, in_flight=True)
        continue

This assumes a permission block always arrives with `execution['status'] == 'permission-block'` so it falls through to
lines 1058-1065 and raises `Refused` (stopping the pair). However, when an OS/CUA/authentication permission refusal
causes any MCP tool call (`brick_ui`, `brick_action`, or `js`) to fail or return `isError` — the exact way CUA/OS
refusals manifest in a turn — `guest_tester.py:880-881` writes `root/result.json` with `status: 'permission-block'`,
then calls `validate_result()` -> `execution_calls()` -> `native_events()` (`guest_tester.py:428-429`), which raises
`TesterError('native computer-use operation failed or was refused')`. The exception handler (`guest_tester.py:886-920`)
sets `summary['status'] = 'automation-failure'` and retains the model's observation with `status == 'permission-block'`
in `summary['unverified_result']` (the same also happens in unmanaged mode whenever a refused tool call precedes
`result.json`).

When `brick_test.py:1035-1039` imports that attempt, `observation` is `execution['unverified_result']` with
`observation.get('status') == 'permission-block'`, yet line 1039 checks only
`execution.get('status') == 'automation-failure'` without excluding
`(observation or {}).get('status') == 'permission-block'`. As a result, `brick_test.py` records the P1 checkpoint as
`could-not-run`, skips remaining P1 checks, and continues executing later P0 checkpoints (after `old-sessions`) or P2
(`p2-feedback-send`) on the permission-blocked guest, violating BRICK-TEST.md ('An OS/CUA/authentication permission
block during P0 or an evaluation is a harness bug: end that attempt, retain the failure... A P1 permission block still
stops the pair') and the comment at line 1044.

Cross-group interface note: In managed mode (`guest_tester.py:287-298, 864-868`, outside g02), if the permission refusal
also wrote `managed-failure.json` or occurred before any `brick_action` created `managed-operations.json`,
`managed_result()` raises `TesterError` and line 864 only preserves `reported['status'] == 'fail'`, overwriting
`reported['status'] == 'permission-block'` with `'automation-failure'` before `brick_test.py` even sees it. Both sides
need to preserve `'permission-block'`.

Verification: Run a unit test where a P1 checkpoint (`old-sessions` or `local-old-session`) returns an archive whose
`execution.json` has `status: 'automation-failure'` and
`unverified_result: {'identity': ..., 'status': 'permission-block', 'detail': 'OS permission prompt'}`. Observe that
`test()` records `could-not-run` and continues to `update-relaunch` / `p2-feedback-send` instead of raising `Refused`
and setting `entry['automation_failure']`.

Fix: Exclude permission blocks in `brick_test.py:1039`: add
`and (observation or {}).get('status') != 'permission-block'` so permission blocks fall through to lines 1058-1065 and
stop the pair. (And coordinate with `guest_tester.py` so `managed_result` / its error handler preserves
`permission-block` in `unverified_result`.)

Reviewer metadata (g02_brick_test_coord_p1/f3): suggested bucket: other (breaks the documented permission-block stop
fence in maintainer release-testing tooling; confined to disposable test VMs).; confidence: confirmed; possible cover:
none.

Additional source record at `5fcaf735cab90dedf6dc20dd46478f9b0266c3a8`, `releasing/mac-vm-test/guest_tester.py:295`
(g04b_brick_test_guest_workflow/f1):

Commit `5fcaf735cab90dedf6dc20dd46478f9b0266c3a8` introduced P1 non-blocking harness-failure handling in
`releasing/mac-vm-test/brick_test.py:1039-1057`: when `priority == 'P1'` and
`execution.get('status') == 'automation-failure'`, the coordinator records `could-not-run` and continues the pair,
relying on permission blocks arriving with `execution['status'] == 'permission-block'` so they fall through to lines
1058-1065 and stop the pair (per `releasing/mac-vm-test/BRICK-TEST.md:122-123` and `brick_test.py:1044-1045`). However,
`releasing/mac-vm-test/guest_tester.py` converts a tester's `permission-block` report into
`execution['status'] = 'automation-failure'` along two paths:

1. Early `permission-block` before any `brick_action` (`guest_tester.py:292-298`, edited in `5fcaf735`):
   `managed-operations.json` is written only by `Operations.save_record()` (`guest_operations.py:426-435`) on
   `brick_action` or restart receipts, never on ordinary `brick_ui` calls. If the tester observes an OS/CUA/login
   permission prompt during initial `brick_ui` calls and returns `{'status': 'permission-block', 'detail': ...}` (per
   `guest_tester.py:263`), `elif value['status'] in ('fail', 'could-not-run'):` at `guest_tester.py:295` omits
   `'permission-block'`, so line 298 raises `TesterError('managed tester retained no operation receipts')`. `run()`
   (`guest_tester.py:855-871, 896`) catches `TesterError`, sees `claimed_failure` is `False` (line 864 only checks
   `reported.get('status') == 'fail'`), overwrites `observation` with `status: 'automation-failure'` and discards the
   permission-block detail, and writes `execution.json` with `status: 'automation-failure'`.

2. Native tool / action permission refusal (`guest_tester.py:287-289, 428-429, 864-871`): if a `brick_ui` or
   `brick_action` call itself returns an OS/CUA/SSH permission refusal (`isError: True`),
   `guest_operations.py:1117-1124` latches `managed-failure.json`. When the model follows `guest_tester.py:240, 263` and
   reports `status: 'permission-block'`, `guest_tester.py:288-289` raises `TesterError('managed action failed: ...')`
   (and `native_events` at lines 428-429 also raises `TesterError` on any `isError` tool call), and lines 864-869 again
   overwrite the status as `'automation-failure'`.

In both cases, on any P1 checkpoint (`old-sessions`, `local-old-session`, `remote-old-session`, `remote-host-remove`,
`remote-host-uninstall`, or `p1-remote-mac` GUI checks), `brick_test.py:1039` swallows the permission block as
`could-not-run` and continues the pair (and `brick_test.py:1864-1877` then allows ordinary `cleanup` to delete the guest
clones). On P0/P2/bonus checkpoints, path 1 also replaces the `permission-block` verdict and reason with
`'managed tester retained no operation receipts'`.

How to verify: (1) Call
`managed_result(task, output, {'status': 'permission-block', 'detail': 'OS accessibility prompt'})` with no
`managed-operations.json`; it raises `TesterError('managed tester retained no operation receipts')` instead of returning
`status: 'permission-block'`. (2) Run `guest_tester.run()` where a `brick_ui` call fails with a permission refusal
(`managed-failure.json` present) and `final.txt` reports `permission-block`; `execution.json` records
`status: 'automation-failure'`, which `brick_test.py:1039` converts to `could-not-run` on P1.

Fix: (1) Include `'permission-block'` in `elif value['status'] in ('fail', 'could-not-run', 'permission-block'):` at
`guest_tester.py:295`. (2) Preserve `reported.get('status') == 'permission-block'` in `guest_tester.py:864-871` (and
surface it in `execution.json` or check `(observation or {}).get('status') == 'permission-block'` in
`brick_test.py:1039` before downgrading P1 `automation-failure` to `could-not-run`) so a tool-error permission refusal
stops the pair rather than continuing.

Reviewer metadata (g04b_brick_test_guest_workflow/f1): suggested bucket: other (harm is confined to release brick-test
maintainer tooling; breaks the brick-test permission-block stop/retention contract on P1 checkpoints and obscures
permission-block diagnostics, with no direct impact on shipped user installations or user data).; confidence: confirmed
(established directly from `guest_tester.py:287-298, 428-429, 855-899`, `guest_operations.py:426-435, 1117-1124`,
`brick_test.py:1039-1065, 1864-1877`, and `BRICK-TEST.md:122-123`).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g02_brick_test_coord_p1/f3`,
`g04b_brick_test_guest_workflow/f1`, `g07_brick_test_tests_c/l3`.
