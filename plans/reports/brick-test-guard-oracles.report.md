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
