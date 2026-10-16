### What this was about

In the session launcher's host picker, repeatedly typing the same letter cycles through matching host names. That failed
for İ: its lowercase form is two Unicode characters, and the old repetition check treated the second character as a
different key. The maintainer delegated a small fix. The picker now compares the complete lowercase key spelling, so a
second İ cycles rather than extending the search to a query that cannot match.

### Things you should know

The change stays within the agreed gate: one small decision helper and a unit regression. Host-name matching, the
one-second prefix timeout, focus, and other keys are unchanged. Browsing still does not change the selected host until
the user chooses it. There is no locale-specific matching change or new dependency.

### Open questions and possible follow-ups

None required for this plan.

### The PRs

- [#2107: cycle matching hosts when typing İ repeatedly](https://github.com/scode/farhelm/pull/2107/changes) — draft;
  host-picker fix, regression, changelog fragment, and completed triage bookkeeping.

### Checks run, reused and skipped

- Recorded nextest run `42be2e33-a59c-4bf7-8a1a-91cac0ee9647`, selection `test(host_picker::tests::)`, passed its one
  selected UI unit test with the pinned runner, four slots and zero retries. It covers consecutive İ presses, ASCII
  repetition and mixed prefixes; retained output confirms the test ran without an early-return skip.
- Ran `cargo fmt --all -- --check`, changed-file `dprint check`, changelog format validation and the isolated sleep
  checker: passed. The first dprint check caught only ledger wrapping; it was formatted and checked again. The sleep
  checker inspected 301 delays and found zero without a rationale.
- Ran `cargo clippy --all-targets -- -D warnings`: passed on the final executable source.
- Reused source and runtime evidence after adding the ledger's PR link; no executable code changed. Main's intervening
  changes were queue/report bookkeeping, so no rebase or additional runtime check was needed for that delta.
- Skipped browser and broader runtime suites: the plan permits this unit seam, and the change only replaces the
  repetition/query decision used by the existing handler. Matching, timing and event handling remain unchanged.

### The review gate's outcome

The required fresh-context gpt-6.1-sol/high reviewer found no issues after reading the complete diff, contract, outcome
and test checklist. Its full findings were read and accepted. The executing session inspected the source and check
evidence separately, and a valid fresh wording cold read passed. No scope departure or unresolved finding remains.

### Landing

Landed as #2107, sixth of the round's nine plans, tested stacked with the others on main.

#### Review

An independent fresh-context review found nothing blocking. Plain ASCII behaves exactly as before, İ is the only
character whose lowercase form is longer, and the test covers İ, ASCII repeats and mixed input.

#### Left as is

A host named exactly "İ" never cycles, because the length cap compares bytes of the typed text with the original name;
the ﬁ ligature and Greek final sigma still cannot match, as before.

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
