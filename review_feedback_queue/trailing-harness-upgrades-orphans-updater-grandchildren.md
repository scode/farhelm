# Agent updater timeouts and runner termination orphan updater grandchildren that can race subsequent drift cases

Reviewed commit: f2e5f1679bb902c7227cc2cc0dfddf893e9885fc

## TLDR

If a vendor agent updater or fallback shell installer hangs until its timeout, or the drift runner is stopped with
SIGTERM during the upgrade phase, only the top-level shell/wrapper process is killed while its download or extraction
subprocesses keep running and can mutate the agent binary while the drift test runs.

## Details

**other.** Source inspection. Main reference `scripts/harness_upgrades.py:106-120`.

In `scripts/harness_upgrades.py:106-120`, `run_updater()` invokes `subprocess.run(argv, ..., timeout=UPGRADE_TIMEOUT)`
without `start_new_session=True` or process-group teardown. Verified in Python's stdlib `subprocess.run`: on POSIX
`TimeoutExpired`, it calls `process.kill()` on the direct child PID only, then `process.wait()` and raises. For
`bash install.sh` (`run_installer` at line 133) or wrapper launchers (`npm`, `bun`, `claude update`), grandchildren such
as `curl`, `tar`, or `node` survive the timeout. `scripts/run-harness-drift.py:248-255` immediately proceeds to execute
the drift test for that harness even after `upgrade()` reports `failed`, so surviving installer/updater subprocesses can
overwrite binaries mid-test. Likewise, unlike `recorded_case()` (`scripts/run-harness-drift.py:131-172`), `prepare()`
(`scripts/run-harness-drift.py:174-191`) installs no `SIGINT`/`SIGTERM` cleanup, so terminating the runner during an
upgrade orphans the updater. Fix by launching updater/version subprocesses in their own session
(`start_new_session=True`) and killing the process group (`os.killpg`) on timeout or runner signal.

Reviewer metadata (g12_tooling_drift_upgrade_and_worker/l1): suggested bucket: other (maintainer drift-test tooling
reliability/cleanup defect; does not affect shipped Farhelm).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as
`g12_tooling_drift_upgrade_and_worker/l1`.
