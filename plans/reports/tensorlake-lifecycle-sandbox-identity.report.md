## What this was about

Farhelm adds a host entry before asking Tensorlake to create its sandbox. If setup stops before that request, the entry
can hold only a reserved name. Suspend or Resume could use that name to change another sandbox.

The maintainer kept the Hosts panel controls and required a saved sandbox ID or a record of an outstanding Create. With
neither, either action leaves the connection alone and asks the operator to finish setup or delete the host entry, with
no API call. Farhelm writes the Create record before sending the request. That record permits a name lookup, even if
Farhelm was interrupted before sending Create. When the lookup succeeds, Farhelm saves the ID and clears the record
together, then uses the ID for the action, its confirmation and any retry. If the lookup fails, the action stops and a
retry can look up the name again.

## Things you should know

If Farhelm cannot save the recovered ID, it sends no Suspend or Resume. One provider deadline covers lookup, the action
and confirmation, with 60 seconds for Suspend and 120 for Resume. The local save finishes under the host's operation
lock even if the deadline passes. If the sandbox still needs to change and the save finishes past the deadline, Farhelm
returns a timeout before sending the action. An already-at-target sandbox can still settle the outstanding Create and
return success. A timeout while confirming a sent action leaves its outcome unknown.

## Open questions and possible follow-ups

None.

## PRs

- [#4670](https://github.com/scode/farhelm/pull/4670/changes) — Require a sandbox ID for Tensorlake Suspend and Resume.

## Checks

- `cargo nextest run -p farhelm-helm --lib -E 'test(tensorlake::hosts::tests::)'` passed 41 tests without runtime skips
  on snapshot `9b71df7c` (run `9650a260-ec16-4687-a53e-e55d8749b4db`). The tests covered both verbs, retries after a
  recovered ID, already-settled status, failure to save an ID, and the deadline expiring during a real local SQLite
  write. The earlier build was interrupted before test evidence was available (run
  `d014b441-8b2d-45db-989f-66c7ceae2237`).
- `cargo clippy --all-targets -- -D warnings` passed on the same snapshot (run `944f4c8f-3e36-44d3-9a7a-4677ba56c6aa`).
  `cargo fmt --all -- --check` and `python -B scripts/check-test-sleeps.py` passed before the rebase. `dprint check` on
  changed Markdown and `python3 releasing/check-changelog.py format` passed after it.
- The rebase added Claude trust work and planning bookkeeping. The Tensorlake patch, provider code and test inputs
  stayed the same. The focused tests and the pre-rebase Clippy result cover this change. The local fake API and SQLite
  covered Farhelm's decisions, persistence and timing, so a broader suite was skipped. The checks used fixtures
  throughout and left live Tensorlake behavior unverified.

## Review gate

The independent source review of the first candidate found three gaps in the tests and no production defects. I
tightened the tests to verify that a refusal leaves routing alone, that the provider timer expires while the ID write is
still held, and that a save that cannot update the host entry prevents the action. The final focused run verified those
changes. The commit wording passed a separate cold read.
