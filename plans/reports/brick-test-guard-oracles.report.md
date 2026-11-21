# Make the release brick tests prove both guards

## What this was about

The release brick test checks that restarting the app after an update actually starts the candidate version. Its
regression fixture reported an old running app and omitted the command-line version. Either fact gave the same refusal,
so the test could pass after losing the old-process check. The fixture now reports the correct command-line version and
leaves the old process as the sole reason for rejection.

Installing the previous release in a test guest requires a clean fixture and verified release files. The old test
supplied neither and could pass if only one was required. It now tries each prerequisite on its own and requires refusal
before the installer can run in both cases.

## Things you should know

You approved one draft for each of these test fixes. Both retain the existing release and installer behavior. Each draft
removes its feedback item and moves the original decision to the archive.

## Open questions and possible follow-ups

None.

## Pull requests

- Supply the correct CLI version in the old running app fixture.
  [#4382](https://github.com/scode/farhelm/pull/4382/changes)
- Check each missing installer prerequisite separately, stacked on #4382.
  [#4384](https://github.com/scode/farhelm/pull/4384/changes)

## Checks

- The portable suite, `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v`, passed all 279 tests
  for each draft. Recorder `fdaf1946-ceb4-4dda-a5ab-88683931fd62` covered the old-process change based on `ce9be005`.
  Recorder `b877b1f7-bfbe-42e0-93f0-fe194a562024` covered the combined test changes based on the first draft at
  `96d58a89`. A first run of the second draft was interrupted before reaching the edited test (recorder
  `8f557178-3a42-4725-9504-8ef60e24d34e`).
- Reused the portable results after a rebase that only affected unrelated code and bookkeeping. The final draft
  amendments added their PR links to the archive. `dprint check` on the ledger and index and the instruction-link
  checker passed, with one existing triage citation warning.
- These are portable fixture changes. The suite uses test guests' simulated observations and commands, so this check did
  not boot VMs, run native GUI checks or validate release artifacts.

## Review

Both drafts passed independent source review and commit wording checks. The source reviewers checked that each fixture
distinguishes the guard it names from the other conditions that could produce the same refusal.

### Landing

#### What landed with it

installer-symlink-file-oracle landed right after this plan in the same round. It fixes a similar weak test, in the
supervisor's installer code rather than the release brick test. The triage archive is the only file both plans changed,
and it kept both plans' entries. Nothing else had landed on main since this plan's work started. Each of this plan's two
tests now fails if the check it names is removed, since its fixture leaves that check as the only reason for the refusal
the test expects.

#### Checks

This round landed two plans that change only tests: brick-test-guard-oracles (two tests of the release brick test) and
installer-symlink-file-oracle (one test of the supervisor's installer code), plus the triage bookkeeping for each. So
the landing ran the affected test groups and the main lint (clippy) on a separate arm64 Linux test machine, and the
brick test's own tests and the small checks locally on x86_64, on both plans stacked together on main:

- `cargo nextest run -p farhelm-supervisor --lib -E 'test(/installation::dirs::/)'`, on the test machine (recorder run
  `5260de68-8916-4c08-b463-de8eb73a88d2`): both tests in the group passed, the changed one included.
- `cargo clippy --all-targets -- -D warnings`, on the test machine, not through the recorder: passed.
- `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v` (the release brick test's own tests),
  locally (recorder run `065f1ee5-3561-45a4-9f8e-ac5cebd14132`): all 279 passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` and `dprint check` (formatting and test lints),
  locally: passed.

Skipped: everything else, since no product code changed. Neither the executors nor the landing ran a test with the
guarded check removed to watch it fail; an independent reviewer and I confirmed from the code that each test would.

#### Things you should know that the report does not say

The report says you approved one draft PR for each of these test fixes. You did not approve the PRs themselves: while
planning, you agreed that each fix would be its own PR.

#### What you should still do

Nothing.
