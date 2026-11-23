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

### Landing

#### What landed with it

The plan's one PR (#4670) landed alone in this round, with no landing fix. Since the plan's base, main had gained only
two changes to the opt-in tests against real agent programs (#4665, #4666), which touch nothing in the helm or
Tensorlake, so the rebase was trivial and nothing interacted.

The landing's independent reviewer found no way left for Suspend or Resume to reach a sandbox Farhelm does not own: a
host that only reserved a name is refused before any request to Tensorlake, and once a name lookup recovers the ID,
every later request and retry uses that ID. The recovered ID is saved in one write under the host's operation lock (the
one setup, update and removal also take), and nothing can interrupt that write once it starts. Waking a suspended host
on demand, Stop and suspend, and the automatic start all go through the same check; there is no automatic idle suspend
for it to miss. When the action fails after Farhelm had already taken the host's connection down for it, Farhelm
reconnects the host.

#### Fixes made while landing

None.

#### Checks

The plan changes only the helm: how Tensorlake Suspend and Resume find and record the host's sandbox, new helm tests,
spec wording and a release-notes draft. The executor ran only the Tensorlake hosts tests, so the landing ran the helm's
whole test suite and the lints. The worker runs were on a separate arm64 Linux test worker; the rest ran locally on
x86_64 Linux. None of them reached the real Tensorlake service: the tests use a scripted stand-in for it, as the report
says.

On the plan as delivered, rebased onto main:

- `cargo nextest run -p farhelm-helm`, on the test worker (recorder run `fa9f2851-ab2a-4442-a186-a4d000952374`): all
  1853 passed, none printed a runtime `SKIPPED`.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

Skipped: the supervisor's, the farhelm command's and the desktop app's tests, the end-to-end smoke test and the
reference screenshots, which nothing in the plan reaches; macOS, where nothing compiles differently; the real Tensorlake
service; CI, which runs only on demand.

#### Things you should know that the report does not say

- A create request that was recorded but never sent (setup stopped just before sending it) keeps allowing a lookup by
  name however old it is, as the plan decided. So if setup died that way and someone later created another sandbox under
  the same name, Suspend or Resume on that host would adopt and act on that other sandbox. Deleting such a host already
  looks it up by name the same way. A possible follow-up is to let such a record expire.

Three smaller possible follow-ups, none of which affects which sandbox is touched:

- If Farhelm cannot save the recovered ID, the user sees a bare internal error that does not say nothing was suspended
  or resumed, or that trying again is safe. Deleting a host explains the same failure; Suspend and Resume could too.
- If saving the ID finishes only after the time Suspend or Resume allows (60 or 120 seconds), the timeout says
  Tensorlake did not finish suspending or resuming, although Farhelm never asked it to. This needs a local save that
  takes about a minute, so it is unlikely.
- The step that recovers the ID would adopt any sandbox with the host's name if it were called for a host with neither a
  saved ID nor a recorded create request, which is the bug this plan fixes. Every current path refuses such a host
  first, but the code's documentation does not say the step depends on that.
