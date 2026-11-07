## What this was about

Clicking a printed file path could fail when a terminal program split it across two lines: Farhelm offered the head and
the continuation as separate files. The maintainer chose joining the pieces over suppressing those links, explicitly
accepting some false matches for the benefit of usable downloads.

Either half of a qualifying two-row path now discloses and downloads the whole file. Indentation and trailing padding do
not become part of its destination. Normal terminal wrapping keeps working, and relative paths still resolve from the
session's directory. The product and implementation specifications record this intentional recognition exception.

## Things you should know

The rule allows four columns for a terminal program's right margin and only joins one isolated pair. Unrelated lines
with the same shape can be joined; this is the agreed tradeoff. Three-row hard-break chains are left unchanged.

Tests reproduced the reported failure with an explicit program newline. Ordinary terminal wraps and a known wrap
crossing from history into the visible screen already worked. This establishes the failing shape, not which kind of
break the original program emitted. No host-side path resolution or terminal emulator change was needed.

The real browser test downloaded the same file from each half on Chromium and WebKit. WebKit is browser coverage of the
desktop engine family, not a direct native-app run. The live installation was not used.

## Open questions and possible follow-ups

No required work remains. Paths split across more than two hard-broken rows and continuations with gutter characters
remain outside this change.

## PRs

- [#3129: link paths split by terminal programs](https://github.com/scode/farhelm/pull/3129/changes) — reviewed draft,
  commit `6e414d0936b42e5946a3877d9b8eb9a22a521e14`.

## Checks run, reused and skipped

Runtime commands below ran through the repository's test recorder. Commands name repository-relative child arguments;
browser commands ran from `e2e/`.

- The complete UI JavaScript harness (`node --test` from `crates/farhelm-ui/js-tests`) passed 527 tests, run
  `e068de3e-5ce4-404a-9a94-9c9ac60f6e07`. This is reused from the fix's worktree based on `56d612b5`; its recorded
  source fingerprint begins `1d964d8ec06fae37`. Product behavior is identical in the pushed revision: later changes
  added a reader docstring, expanded one test and rebased over queue-only metadata. The expanded cached-endpoint test
  separately passed in `bcf255d2-07d3-4b08-9c10-ec0b76e12f66`, covering hard/soft breaks and browser/native admission:
  `node --test --test-name-pattern='program-broken printed path cannot grow' crates/farhelm-ui/js-tests/terminal-files.test.js`.
- The focused recognizer and grid-buffer suites passed 76 tests in `4a7bd9fe-73b0-4704-aa66-4d517c9302d7`. They cover
  both terminal adapters, delimiters, URI exclusions, margin boundaries, Unicode cells and the history/screen wrap:
  `node --test crates/farhelm-ui/js-tests/terminal-files.test.js crates/farhelm-ui/js-tests/grid-buffer.test.js`.
- The static grid browser selection for printed-path download and redraw admission passed four cases across Chromium and
  WebKit in `5582eebb-886c-4b00-977f-aed8658f8068`:
  `npx playwright test --config grid-client/playwright.config.ts grid-attach.spec.ts -g 'a printed path hovers|a redraw that changes the hovered path' --workers=3 --retries=0`.
- The recorded Playwright selection of terminal-file and terminal-link specs passed 46 cases across both engines,
  `43288ffc-a6ce-4702-b993-d675ee080365`. It includes the new live hard-break hover/download test on each engine.
  Command: `npx playwright test 'terminal-files\.spec\.ts' 'terminal-links\.spec\.ts' 'terminal-link-clicks\.spec\.ts'`.
  The separate old-terminal link-click selection passed 36 cases across both engines,
  `f30819bc-f7a8-46a4-8459-348d309c2b52`, command `npx playwright test 'terminal-link-clicks\.spec\.ts'`, with
  `FARHELM_E2E_SUITE_TERMINALS=tmux` explicitly retained by the recorder. Both runs required the pinned tmux, used a
  three-worker budget and zero retries, and completed their owned stack cleanup. Two projects used two workers in the
  old-terminal run. That selection was excluded from the first run as designed.
- `cargo build -p farhelm -p farhelm-fixtures` and the Dioxus 0.7.10 release web build passed to prepare those browser
  runs. An initial build invocation selected an unrelated `dx` executable and failed before compiling; selecting the
  installed Dioxus CLI resolved it.
- The source-only sleep check inspected 420 delays with none missing a rationale. Changelog format, whitespace and
  dprint checks passed for the files they cover. Asset JavaScript and its unit tests are excluded from dprint.
- The initial regression failed as intended in `10c0e315-cedf-47c6-aec5-f113f73d42a9`. A later unit run,
  `c876b0e7-b13a-4f7c-bb18-0c4926b1534d`, caught two new fixture mistakes: an undersized history row and a purported
  negative that actually matched the accepted rule. Both were corrected before the passing runs. Failure evidence is
  retained; these were same-session authoring errors, not established latent flakes.
- Rust runtime tests, Clippy, the full browser battery, native desktop and installer/release suites were skipped: this
  changes shared JavaScript recognition and its browser integration, with no Rust behavior or those subsystems changed.
  Existing JavaScript results remain applicable after the queue-only rebase.

## Review gate

A fresh-context gpt-6.1-sol reviewer at high effort reviewed the complete change and the final test/docstring addendum,
with the repository's test-authoring checklist supplied verbatim. Neither review found actionable issues. Two fresh
wording readers checked the PR text; the final text passed its cold read. The executor also completed a separate
documentation pass over all touched files.
