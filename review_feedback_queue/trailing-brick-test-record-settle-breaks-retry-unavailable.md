# record() eagerly settling P1/P2 skips and retry_unavailable() counting skipped entries break retry-unavailable on p0-installer-upgrade

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

During a release brick test run in full, stop-after-P1, or stop-after-P2 scope, the documented
`brick_test.py retry-unavailable` recovery command always refuses to retry any P0 checkpoint on the installer-upgrade
pair (`p0-installer-upgrade`), forcing the operator to discard the pair and start over after repairing a fixture
prerequisite.

## Details

**other.** Source inspection. Main reference `releasing/mac-vm-test/brick_test.py:836`.

There are two interacting bugs across `Run.record()` (line 836), `Run.settle()`/`Run.dead_end()` (lines 1457-1525), and
`Run.retry_unavailable()` (lines 1150-1152):

1. Eager P1/P2 settlement on a retryable P0 `could-not-run`: In `releasing/mac-vm-test/brick_test.py:833-836`,
   `Run.record()` unconditionally calls `self.settle()` immediately after saving any result. On `p0-installer-upgrade`,
   every P0 checkpoint (`artifact-verification`, `old-fixture`, `update-relaunch`, `local-supervisor-upgrade`,
   `remote-supervisor-upgrade`, `local-supervisor-agent`, `remote-supervisor-agent`) precedes the pair's P1 checkpoints
   (`local-old-session`, `remote-old-session`, `remote-host-remove`, `remote-host-uninstall`, and for the first two also
   `old-sessions`) and P2 checkpoint (`p2-feedback-send`) in `entry['required']` (lines 308-320). When any P0 checkpoint
   on `p0-installer-upgrade` records `status == 'could-not-run'` in `full`, `p1-only`, or `p2-only` scope, `settle()` ->
   `dead_end()` -> `order_block()` (and the P0 loop for P2) treats the retryable P0 `could-not-run` as a permanent dead
   end and calls `self.skip()`, writing `status: 'skipped'` into `entry['results']` for all later in-scope P1 and P2
   checkpoints. Even if `retry_unavailable()` later allowed retrying the P0 checkpoint, those permanently persisted
   `'skipped'` results would prevent P1 and P2 from ever running after the retried P0 checkpoint passes (because
   `test_owned()` skips keys in `entry['results']` at line 930 and `record()` refuses overwriting existing keys at line
   720).

2. `retry_unavailable()` counts synthetic `'skipped'` entries as later observations: Lines 1150-1152 check:

   ordered = list(entry['required']) if any(key in entry['results'] for key in ordered[ordered.index(check) + 1:]):
   raise Refused('later checkpoint observations prevent replaying this fixture boundary')

Because `entry['results']` already contains `'skipped'` entries for later P1/P2 checkpoints — both from (1) above, AND
independently whenever an earlier `old-sessions` failure/unavailable result already settled
`local-old-session`..`remote-host-uninstall` as `'skipped'`, or whenever an earlier P0 pair
(`p0-fresh`/`p0-updater-upgrade`) failed/was discarded and pre-settled `p2-feedback-send` as `'skipped'` — line 1151
ALWAYS raises `Refused('later checkpoint observations prevent replaying this fixture boundary')` for every P0 checkpoint
on `p0-installer-upgrade` in `p1-only`, `p2-only`, and `full` scope.

Affected vs. unaffected cases:

- Affected: Every P0 checkpoint on `p0-installer-upgrade` whenever scope includes P1 or P2 (`p1-only`, `p2-only`,
  `full`), or if scope previously included P1/P2 while `p0-installer-upgrade` was ready and settled any later
  `'skipped'` entry.
- Unaffected: `p0-fresh`, `p0-updater-upgrade`, and bonus passes (`fresh`, `upgrade-*`), which declare no P1 or P2
  checkpoints; `p0-installer-upgrade` strictly in `p0-only` scope (where `settle()` writes no P1/P2 skips);
  `p1-remote-mac`, whose P1 checkpoints are intentionally excluded by line 1146.

Cross-group interface: Interacts with `settle()`/`dead_end()`/`order_block()` in group `g03_brick_test_coord_p2` and
test coverage in `g05_brick_test_tests_a` (which only tests `retry_unavailable` on `p0-fresh`).

Verification: Initialize a release run in `full`, `p1-only`, or `p2-only` scope, prepare `p0-installer-upgrade`, let any
P0 checkpoint (e.g. `local-supervisor-upgrade`) return `could-not-run` with intact evidence, then call
`run.retry_unavailable('p0-installer-upgrade', 'repaired prerequisite ...')`. It raises
`Refused('later checkpoint observations prevent replaying this fixture boundary')` instead of transitioning the
checkpoint to `retry-ready`.

Fix (both parts needed):

1. Do not treat a retryable `could-not-run` P0 checkpoint on the same ready pass as a permanent `dead_end` in
   `settle()`/`dead_end()` (since `test_owned()` stops immediately on a P0 `could-not-run` and ordinary `cleanup()`
   refuses while P0 != `pass`, settling later P1/P2 checks on that pass is unnecessary until P0 fails or the pair is
   discarded).
2. In `retry_unavailable()` line 1151, ignore synthetic `'skipped'` entries when checking for later executed
   observations
   (`any(entry['results'].get(key, {}).get('status', 'skipped') != 'skipped' for key in ordered[ordered.index(check) + 1:])`),
   so pre-settled skips from an earlier `old-sessions` non-pass or an earlier discarded P0 pair do not block retrying a
   P0 checkpoint.

Reviewer metadata (g02_brick_test_coord_p1/f2): suggested bucket: other (breaks a documented recovery workflow in
maintainer release-testing tooling; no user data loss or security impact).; confidence: confirmed; possible cover: none.

Additional source record at `5fcaf735cab90dedf6dc20dd46478f9b0266c3a8`, `releasing/mac-vm-test/brick_test.py:1430-1479`
(g03_brick_test_coord_p2/f3):

Two interacting bugs involving `Run.settle()` (`releasing/mac-vm-test/brick_test.py:1430-1479`) and
`Run.retry_unavailable()` (`releasing/mac-vm-test/brick_test.py:1120-1122`) break documented P0 prerequisite recovery
(`BRICK-TEST.md:215-221`) on `p0-installer-upgrade` whenever scope includes P1 or P2 (`p1-only`, `p2-only`, or default
`full`):

1. Premature settlement on retriable P0 `could-not-run`: When any P0 check on `p0-installer-upgrade`
   (`artifact-verification`, `old-fixture`, `update-relaunch`, `local-supervisor-upgrade`, `remote-supervisor-upgrade`,
   `local-supervisor-agent`, `remote-supervisor-agent`) records `status='could-not-run'`, `Run.record()`
   (`brick_test.py:807`) immediately invokes `self.settle()`. Because `p0-installer-upgrade` is `'ready'` and
   `dead_end()` (`brick_test.py:1463-1479`) treats any P0 result with `status != 'pass'` (including retriable
   `'could-not-run'`) as a permanent dead end, `settle()` immediately writes `'skipped'` entries into `entry['results']`
   for `local-old-session`, `remote-old-session`, `remote-host-remove`, `remote-host-uninstall`, and (under
   `p2-only`/`full`) `p2-feedback-send` (plus `old-sessions` if earlier). When the operator later repairs the fixture
   prerequisite and runs `retry-unavailable`, `retry_unavailable()` (`brick_test.py:1120-1122`) checks
   `if any(key in entry['results'] for key in ordered[ordered.index(check) + 1:])` and unconditionally raises
   `Refused('later checkpoint observations prevent replaying this fixture boundary')`. Moreover, even if line 1121
   ignored `'skipped'`, those P1/P2 slots are already permanently occupied by `'skipped'` in `entry['results']`, so
   retrying and passing the P0 check could still never run P1 or P2.
2. Earlier `old-sessions` non-pass skips later P1 checks before later P0 checks run: Even without (1), when
   `old-sessions` (P1, position 3) records `could-not-run`, `settle()` immediately records `'skipped'` for
   `local-old-session`..`remote-host-uninstall` (positions 9..12). If any subsequent P0 check
   (`update-relaunch`..`remote-supervisor-agent`, positions 4..8) later records `could-not-run`, `retry_unavailable()`
   line 1121 (which does not skip `'skipped'` entries, unlike `order_block()` at line 1458) refuses to retry that P0
   check because those later P1 keys already exist in `entry['results']`.

Verify by recording `could-not-run` on any P0 check of `p0-installer-upgrade` under `full` (or `p1-only`/`p2-only`)
scope and calling `retry_unavailable('p0-installer-upgrade', ...)`, or by recording `old-sessions` as `could-not-run`
followed by `update-relaunch` as `could-not-run` and calling `retry_unavailable`. Fix both halves: (a) do not treat a
retriable P0 `'could-not-run'` on a `'ready'` pass as a permanent dead end in `dead_end()`/`settle()` (e.g., only skip
P1/P2 on a P0 `'fail'`, a deleted pair, or when `cleanup()` / non-retriable termination runs, since `test_owned` stops
on P0 `'could-not-run'` via `CheckStopped` anyway), and (b) in `retry_unavailable()` line 1121, check
`entry['results'].get(key, {}).get('status', 'skipped') != 'skipped'` (matching `order_block()` line 1458) so
already-skipped later P1 checks from an `old-sessions` failure do not block retrying a P0 checkpoint.

Reviewer metadata (g03_brick_test_coord_p2/f3): suggested bucket: other (harm is confined to maintainer release-testing
tooling; it blocks documented fixture-prerequisite recovery on `p0-installer-upgrade`, forcing a fresh run instead of
retrying, without causing false passes or user impact).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g02_brick_test_coord_p1/f2`,
`g03_brick_test_coord_p2/f3`.
