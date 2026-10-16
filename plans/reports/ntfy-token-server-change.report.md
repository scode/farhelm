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

### Landing

Landed as #2098, second of the round's nine plans, tested stacked with the others on main.

#### Review

An independent fresh-context review found nothing blocking: no path lets a saved token reach a different server. The
stored settings are read, compared and rewritten in one step on one database connection, so a form loaded before another
client's save cannot carry a token across. "Same server" uses the URL library's origin comparison, and anything
uncertain (a scheme switch, a trailing-dot host, localhost versus 127.0.0.1, an unparseable stored URL) clears the
token. The stored token is cleared, not just unused, never logged or returned, and the tests show the second server
receives no credentials.

#### Fixes made while landing

- ntfy alerts are not in any release yet, so a "fixed" changelog entry would describe a bug no release had. Under the
  rule for follow-ups to unreleased work, the separate fragment was removed and the alerts feature's own fragment now
  says changing to a different server clears a saved access token unless you enter a new one.
- SPEC.md's Settings paragraph now states the rule users see: changing to another server clears a saved token unless a
  new one is entered, and the form says so before saving.
- The browser check that a path-only edit keeps the token passed whether or not the form compared servers; it now first
  switches to another server (hint shown) and then back to the saved one with a path (hint gone).

#### Left as is

The hint is not tied to the token field for screen readers; a stale form that clears a token another client saved shows
no message after saving (it fails safe).

#### Checks

Run on all nine of this round's plans stacked on main (terminal-link-safety, ntfy-token-server-change,
remote-uninstall-fixes, update-failure-reporting, docs-accuracy-fixes, host-picker-dotted-i,
setup-advice-state-directory, copy-guidance-resize, brick-test-feedback-evidence), with every landing fix in:

- `cargo fmt --all -- --check`, `dprint check`, the changelog lint and the sleep checker (301 delays, none missing a
  reason); `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the UI crate's
  clippy with the desktop feature and `cargo check -p farhelm-desktop`; the brick-test tooling's own test.
- Through the recorder: the protocol and helm packages in full with pinned tmux, run
  `6352fad3-a3b1-48dc-9884-3efc17ee03c6`: 1258 passed; the CLI's own unit tests, run
  `0e1855d7-6f43-4da7-8a9a-495936ae565e`: 171 passed; the UI crate with the desktop feature, run
  `513addbb-2505-4c54-bc07-2ac2ec06713b`: 655 passed.
- The JS harness (257 passed) and the website build.
- The terminal-link-clicks, terminal-links, terminal-files, ntfy-settings, provisioning, settings and tooltip-coverage
  specs on Chromium and WebKit, run `97a26936-2b7b-42a5-af59-8f0fdb0b9d78`: 208 passed, 2 failed. Both failures were the
  clipped-disclosure test pinning the refusal's old layering after the quick switcher closed (see terminal-link-safety).
  After that expectation was fixed, the terminal-link-clicks spec on both engines, run
  `a698ea79-451d-4dd4-87f5-24ef1d4b1cad`: 16 passed. An intermediate run, `6113fd52-60bf-4730-ae85-5e9dc2e90d46`, failed
  on WebKit only because the first corrected expectation assumed Chromium's behavior.
- Not run: the supervisor package and the CLI's end-to-end targets, which no PR in this round changes; other browser
  specs.
