### What this was about

Triage approved nine fixes to the opt-in agent drift suite and the release brick-test coordinator. The drift suite could
copy refreshed login files back while a test session might still be alive, or report a pass after a copy failed. It
could also silently omit a requested harness. Other checks could miss the first-prompt capture boundary, fail when the
suite tried to reopen the Cursor test chat by ID, or mistake inherited Claude trust and unrelated Claude writes for the
behavior under test. The brick-test coordinator could strand two prepared Mac guests when a stop-after-P0 request
excluded their P1 tests.

### Things you should know

The drift suite waits for confirmed session deletion and terminal shutdown before writing back login files. It continues
trying the remaining files after an individual write failure and reports the case as failed. A panic still runs
teardown. The runner rejects a requested harness if the selected checks have no case for it. Cursor’s opt-in check now
uses one genuine fresh launch and a separate chat that it seeds and reopens by ID. That extra seed turn is part of a
live Cursor run. Prompt-timed cases check that the suite captures the conversation ID before the hook reports the end of
that turn. The Claude trust check clears inherited workspace trust from its private copy and detects newly accepted
trust for the test workspace in the real file.

Ordinary brick-test cleanup can remove a prepared Mac pair that falls out of scope before its first test. If cleanup
finds only a saved test record, it still needs evidence that the native test setup passed its preflight. Tested pairs
keep their existing evidence and ownership checks.

### Open questions and possible follow-ups

The Cursor launch-prompt case still needs a live run with a working login. The existing TODO now tracks that
verification. There is no decision blocking this stack.

### Pull requests

- Require confirmed drift teardown and reject unmatched harness selections.
  [#5044](https://github.com/scode/farhelm/pull/5044/changes)
- Strengthen Cursor launch prompts, first-prompt capture ordering, and Claude workspace trust checks.
  [#5047](https://github.com/scode/farhelm/pull/5047/changes)
- Allow ordinary cleanup of a prepared Mac pair excluded by stop-after-P0 before testing.
  [#5048](https://github.com/scode/farhelm/pull/5048/changes)

### Checks

- Drift cleanup and runner. Recorded `cargo nextest run --locked -p farhelm --test e2e` with the offline drift group and
  `custom_agent_positional_report_corrects_to_the_parent_and_restarts_it` at `408c0b0a`. All 39 passed, including
  Restart through an owned test supervisor (run `928fb981-eba8-49f1-8e10-6b69371a513f`). Reused the 21 passing
  `python3 -B scripts/test-run-harness-drift.py` checks from `5f057e04` (run `6bfdb319-a7ba-422a-9736-4352a340f1bd`) and
  passing license and embedded-notice checks from `6b77a19f`. The runner and dependency inputs stayed unchanged.
- Launch and trust. The same nextest command selected the offline drift group at `dea6cc02`. All 42 passed, including
  Cursor’s selector, prompt timing, and Claude’s private configuration (run `7a2009dd-1be7-4ec9-94a3-78f2411cc27e`).
  Both Rust changes passed formatting, the test-sleep check, and integration Clippy.
- Brick coordinator. All 333 passed with `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v` at
  `7cfd4ee0` (run `f81b65f7-b00b-43fd-b2ce-5e16720e0d6b`). The final revision `406da567` kept the coordinator code and
  added an assertion isolating a saved tester record. Both focused tests for remote Mac cleanup passed with
  `test_native_coordinator.py` selected by discovery and `-k remote_mac` (run `d5082f3b-fa63-4adb-88c4-3327d25a1aed`).
  Markdown and whitespace checks passed. The instruction-link checker passed with one existing stale-citation warning in
  the archive.
- The Rust tests excluded opt-in vendor turns. Brick tests used modeled guests and synthetic evidence. No real Mac or
  release test was required. The changes have no GUI output to review.

### Review

Independent Sol source review passed each draft after corrections. The Cursor scope review confirmed that the fresh
launch, named seed, and named recall cover the two required launch behaviors. The executor compared the stack with
current main and found no changes to the affected behavior.

### Landing

#### What landed with it

Only this plan landed this round. Since its PRs were based, main gained the provisioning, Podman and runtime triage, the
spec and comment updates, the queue discards and the host menu and terminal triage; the only one that touches a file
this plan touches is the upload cancel change, in a different part of the attachment upload tests. The rebase met only
conflicts in the shared triage records, kept both ways, and the queue's files, index and open outcomes agree afterwards.
The landing's independent review also checked the plan against the earlier drift and brick test changes it builds on
(the committed hook report check after Restart, the brick test's retry and reopen fixes) and found them compatible: the
tighter launch evidence reads only the hook's own log lines, and the brick cleanup waiver applies only to a pair with no
checks, results, preflight or tester record.

On the parts that touch real files, the review found the guards hold: copying the agents' rewritten configuration back
now requires every session the run created to be confirmed deleted and its terminal hosts gone, still requires the real
file to be unchanged, and refuses a destination that aliases a copied vendor file; a panicking case runs the same
cleanup first. The new `futures-util` dependency is for the tests only and was already in the build.

#### Fixes made while landing

- None. The review found no blocker or should-fix item.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the plan's PRs rebased onto main as it stood at the start of the landing (`76ac115c`), with the shared triage
records' conflicts kept both ways; every other change is exactly what the executor pushed.

- The drift, hook identity and Codex identity tests of the end-to-end suite,
  `cargo nextest run -p farhelm --test e2e -E 'test(/^real_agent_capture::/) | test(/^hook_identity::/) | test(/^codex_identity::/)'`,
  on the test worker (recorder run `09b58c40-65c0-4aec-b0d8-fde78e11d8a7`): all 82 passed. The drift cases that drive
  real agents are ignored by default and did not run.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- The dependency license notices, `python3 -B scripts/dependency-notices.py --check`, on the test worker: passed
  (`Cargo.lock` changed); and the dependency policy, `cargo-deny --locked check licenses sources` with the pinned
  cargo-deny, locally: passed.
- The drift runner's tests, `python3 -B scripts/test-run-harness-drift.py`, locally (recorder run
  `0ede4521-4eca-4b59-a094-68878a6bac46`): all 21 passed; and the release brick test harness's portable tests,
  `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py'`, locally (recorder run
  `c94c6f80-8a4c-49f8-86f9-c8ef396711df`): all 333 passed.
- `python3 scripts/check-agents-md.py`, `python3 releasing/check-changelog.py format`, `cargo fmt --all -- --check` and
  `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally: this plan's files pass; it fails
  only on over-long lines in the triage outcomes archive that main already had. No PR fixes them yet; a formatting-only
  one would, and I recommend it, as earlier rounds' notes did.

Reused: none. The executor's runs were on an older base, and everything they covered ran again above.

Skipped: the rest of the workspace Rust suite, since only test code, the drift runner and the brick harness changed; the
real-agent drift cases and a real Mac brick test, which spend vendor turns or need a Mac guest and run only when asked;
the desktop app's checks and the screenshot comparison, since nothing in the app changed; and CI, which runs only on
demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- **A real Claude drift run might clear your Claude trust for the test workspace's parent directories (unverified).** A
  drift run gives Claude a private copy of its configuration and, at the end, copies back the files Claude rewrote in
  that copy, provided the real files did not change meanwhile. To keep the trust case from passing on trust inherited
  from a parent directory, the plan now clears trust for the test workspace and its parents in that private copy. If
  Claude rewrites the file holding those trust entries during the run, the copy-back carries the cleared entries into
  your real configuration, and the unchanged-real-file guard does not stop it, since your real file did not change. You
  would lose the trust approvals for those parent directories, which are real directories on the machine, and Claude
  would ask again. Whether current Claude rewrites that file was not established. The drift tests run only when asked,
  so nothing happens until then. I recommend checking this before the next real Claude drift run, by running it once
  against a throwaway home; say if you want a TODO entry.
- **Smaller points the review noticed.** The offline tests exercise the drift cleanup helpers but not how the harness
  wires them in, which `.agents/validation.md` slightly overstates; a "real Claude config changed" note prints twice;
  one error branch in the drift runner is unreachable when a harness is named; and a Cursor login that expires between
  two turns is reported as a failure rather than "could not run". None needs a follow-up on its own.
