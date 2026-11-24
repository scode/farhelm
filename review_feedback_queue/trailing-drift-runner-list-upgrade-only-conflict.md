# `run-harness-drift.py --list --upgrade-only` ignores `--list` and runs vendor upgrades

Reviewed commit: f2e5f1679bb902c7227cc2cc0dfddf893e9885fc

## TLDR

Passing `--list` together with `--upgrade-only` to the drift test runner silently ignores `--list` and upgrades all
installed agent CLIs instead of rejecting the conflicting options or listing the cases.

## Details

**other.** Source inspection. Main reference `scripts/run-harness-drift.py:211-246`.

In `scripts/run-harness-drift.py:213-225`, `--no-upgrade` and `--upgrade-only` are placed in
`modes = parser.add_mutually_exclusive_group()`, but `--list` is added outside `modes` (line 213). `main()` then checks
`if options.upgrade_only:` at line 220 before checking `if options.list:` at line 245. As a result,
`run-harness-drift.py --list --upgrade-only` mutates installed vendor CLIs even though `--list` is documented as listing
cases without running agents, and `--list --no-upgrade` is also silently accepted. Fix by adding `--list` to `modes` so
argparse rejects incompatible mode combinations.

Reviewer metadata (g12_tooling_drift_upgrade_and_worker/f3): suggested bucket: other (minor CLI flag-validation bug in
maintainer drift-test runner).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as
`g12_tooling_drift_upgrade_and_worker/f3`.
