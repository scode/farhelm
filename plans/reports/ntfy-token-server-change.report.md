## What this was about

Changing the ntfy server for waiting alerts could send the previous server's saved access token to the new one. The
agreed fix drops that token when the scheme, host or effective port changes, unless you enter a replacement, and
explains this in Settings before saving.

## Things you should know

The helm enforces the rule while saving against the current stored settings, so a stale Settings form cannot carry
another client's saved token to a different server. Changing only the topic or server path keeps the token; an
explicitly written default port counts as the same server. Explicit replacement and clearing still work.

The hint reads “changing the server clears the saved token when you save” below the password field. Its placeholder and
tooltip also stop promising preservation while the server differs. The form never receives the saved token.

## Open questions and possible follow-ups

None required for this change. There is no confirmation step or per-server token store.

## PRs

[PR #2098](https://github.com/scode/farhelm/pull/2098/changes): clear saved ntfy tokens when the server changes (draft).

## Checks run, reused and skipped

- Passed `cargo fmt --all -- --check`, both all-target and shipped-binary Clippy checks, changed Markdown/TypeScript
  formatting, changelog format lint and the isolated test-sleep checker (301 delays, zero missing reasons). These
  inspect the changed source and its required repository contracts.
- Passed 16 focused helm/UI ntfy Rust tests, including origin normalization and a two-server regression that proves the
  first server receives its bearer token and the second receives none after both null and omitted-token stale-form
  saves. Recorder: `ef59e971-03c9-45ea-800a-654b6ac5eed2`.
- Built the CLI/fixtures and release web UI, then passed all six ntfy browser cases: three Chromium and three WebKit,
  with zero skipped or flaky cases. These assert the warning, placeholder, tooltip, replacement and post-save behavior.
  Recorder: `1eb25d84-dadf-4bce-8aea-4fa00f29a6bd`. The retained screenshots show the existing form before the new hint,
  not the hint itself.
- Reused those results from reviewed revision `b4189e24` after reading all incoming main changes and rebasing. The only
  incoming executable change distinguishes installation identities in host approval displays; ntfy Settings does not use
  that display helper. Other changes are queue/report/claim and unrelated TODO bookkeeping. The ntfy source and tests
  remain identical. The shared feedback index had a mechanical conflict between adjacent removals; preserving both
  removals resolved it. Formatting initially refused the conflict markers and passed after resolution. No runtime
  validation failed.
- Skipped the full Rust/browser suites and desktop runtime checks: the focused tests cover this alert-settings boundary,
  and no desktop-specific code changed. An unrelated fixture SSH refusal supplies no SSH validation and does not affect
  these Settings cases.

## Review gate outcome

The required fresh-context gpt-6.1-sol high source review found no actionable issues. It checked the atomic save, UI
baseline and tests against the plan and the repository's test-authoring rules. The implementation was done locally; that
reviewer ran no tests. A separate wording cold read passed. No scope departure or repeated corrective round required a
scope reassessment.
