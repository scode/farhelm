# Scoped cleanup avoids the machine-wide process scan

## What this was about

Stopping a session, closing a terminal tab or deleting a session scanned every process on the host even after its
systemd groups had been emptied. The maintainer chose to trust those groups on Linux and keep the full scan for unscoped
work. Cleanup now omits that scan when every affected agent and tab has confirmed containment, every named group is
gone, and no explicitly tracked process survives.

## Things you should know

Programs that move themselves into another group, or merely carry a session's environment marker without belonging to
its group, are deliberately left alone by scope-only cleanup. This is the accepted tradeoff, documented in both specs
and the changelog fragment. Terminal handling and container shutdown are unchanged.

A scoped agent does not imply that its tabs were scoped: a tab may have opened while the user manager was unavailable.
Delete preserves each tab's actual selection, including unknown selections, so those cases retain the full scan. A
failed read of this extra metadata also retains the scan rather than introducing a new refusal.

The initial unit run found one old test that still expected an unrelated marked process to be killed after a confirmed
group cleanup. That test now explicitly carries the surviving process as a cleanup root, preserving its intended proof
that successful collection outweighs a failed signal-command status. Its child is also cleaned up if an assertion fails.
The corrected tests and final focused gate pass.

## Open questions and possible follow-ups

None required for this plan. No latency benchmark was needed or run; the unit test rejects invocation of the scan
itself, and the real scoped Stop test proves an outside marked process survives while a contained daemon is killed.

## The PRs

[#3237](https://github.com/scode/farhelm/pull/3237/changes) — avoids full process scans after confirmed scope cleanup;
remains a draft for the landing monitor.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E <focused cleanup selection>` through the
  recorder, pinned nextest 0.9.148 and tmux, four slots, zero retries: 57 actual tests passed, plus the fixture setup
  (run `b13b0443-565f-4afa-a713-a813258b7246`). This covers 46 supervisor tests and 11 integration tests, including real
  scoped Stop, tab close and Delete, host-terminal Restart and other host paths, and forced-unscoped marker cleanup. No
  runtime `SKIPPED` occurred; 2,286 unrelated cases were selected out.
- The exact corrected advisory-kill test and shared child-guard test passed before that gate (run
  `a7beb56e-098f-452c-9e72-64963775f305`). The initial unit command failed only on the stale expectation described above
  (run `00cb7a91-9817-4a2a-91b2-c77735ad9abe`, 45 passed and one failed); it is retained as failure evidence.
- `cargo fmt --all -- --check`, `cargo clippy -p farhelm-supervisor -p farhelm --all-targets -- -D warnings`, changed
  Markdown `dprint check`, `python3 releasing/check-changelog.py format`, and the isolated interpreter's
  `python -B scripts/check-test-sleeps.py` passed (423 delays, none unexplained).
- Runtime evidence is reused across the final comment-only correction and the careful rebase onto the remote Mac support
  changes. Those changes do not alter the scoped reaper or its portable fallback. The tests ran on the working tree
  based on `6a112842`; their recorder records the complete source fingerprint.
- The full Rust battery, browser, desktop, installer and provisioning suites were skipped because the focused cleanup
  checks cover this change. No macOS runtime run was made: it still uses the portable fallback, and no native process
  inspection code changed. No new container test run was needed because container teardown and its ordering are
  unchanged.

## Review gate's outcome

The required single fresh gpt-6.1-sol/high reviewer found the stale unit expectation. It was corrected and rechecked; no
finding remains. A fresh review of the outline's containment assumption approved the minimal correction needed to
preserve unscoped cleanup. The documentation pass includes all touched files, including the old unconditional-sweep
comments. Commit and PR wording passed its separate cold read.
