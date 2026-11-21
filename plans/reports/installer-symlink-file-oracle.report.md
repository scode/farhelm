# Installer symlink refusal test

## What this was about

The installer is supposed to refuse symbolic links when it opens an installed file. The test pointed the link at a
directory, which the installer would refuse even after following the link. The test now checks that the link points to
an existing regular file and that the installer still refuses to open it through the link. The original directory-link
checks still cover classification and folder opens.

## Things you should know

The triage decision was to fix the test fixture. This changes test coverage only.

## Open questions and possible follow-ups

No open decisions.

## PRs

- Covers regular-file links in the installer test and closes the corresponding triage item.
  [#4383](https://github.com/scode/farhelm/pull/4383/changes)

## Checks

- `cargo nextest run -p farhelm-supervisor --lib -E 'test(/^installation::dirs::tests::/)'` passed both selected tests
  (recorded run `4b8c50b6-0251-419c-b680-dedd6b8fe678`).
- `rustfmt --check --edition 2024 crates/farhelm-supervisor/src/installation/dirs.rs`, scoped `dprint check`,
  `git diff --check`, and `python -B scripts/check-test-sleeps.py` passed. `python3 scripts/check-agents-md.py` found
  zero errors and one existing warning about a heading cited in the triage archive. No checks were reused.
- Skipped the installer acceptance and release suites because the focused unit test exercises the exact file-open
  operation and this PR changes only the test.

## Review gate

The required independent review found one issue. It asked that the fixture verify the type it reaches through the link.
The test now checks that premise before exercising the installer's refusal. That closes the only review finding. The
commit message passed a separate cold read.

### Landing

#### What landed with it

brick-test-guard-oracles landed just before this plan in the same round. It fixes two similarly weak tests in the
release brick test. The triage archive is the only file both plans changed, and it kept both plans' entries. Nothing
else had landed on main since this plan's work started. The new link in this test points at an ordinary file, so only
the installer's refusal to follow links can stop it from being opened, and the test now fails if that refusal is
removed. The old assertion opened a link to a directory, which the installer refuses even with that refusal removed, so
replacing it loses no coverage. The plan kept the checks of directory links when listing and opening folders.

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

Nothing beyond the report.

#### What you should still do

Nothing.
