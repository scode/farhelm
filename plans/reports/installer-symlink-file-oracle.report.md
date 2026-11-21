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
