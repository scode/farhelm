# Terminal tabs start without a trailing slash

## What this was about

Opening a terminal tab beside an agent whose folder is the home directory could show `~/` rather than `~` in the shell
prompt. The stored folder could retain a trailing slash, and tmux handed that spelling to the new process as its `PWD`.
A shell that retained that value then displayed the unwanted slash.

## Things you should know

Farhelm now removes trailing slashes when starting a terminal process. The same boundary covers terminal tabs, agent
launches and restarts, including sessions created before this fix. The root directory stays `/`. Stored session folders
stay unchanged, and ordinary symlink resolution still applies.

The exact source in the reported installation was not established: the live installation was not inspected. Supported
inputs can acquire a slash from an absolute folder typed with one, a caller's shell expanding `~/`, or a supervisor home
path already ending in `/` when Farhelm expands `~` or `~/`. The fix covers all three.

## Open questions and possible follow-ups

None required. No stored-data migration or new folder validation was needed.

## The PRs

- [#1673](https://github.com/scode/farhelm/pull/1673/changes) — remove trailing slashes from terminal start directories
  (draft).

## Checks run, reused and skipped

- Ran `cargo fmt --all -- --check`, `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`,
  `python -B scripts/check-test-sleeps.py` with its isolated pinned parser environment,
  `python3 releasing/check-changelog.py format`, and `dprint check TODO.md`. All passed; the delay checker inspected 269
  delays with zero missing rationales.
- Ran the normalization unit test and the real tmux create, restart and new-tab format-syntax tests through the recorder
  with pinned nextest and tmux, four slots and zero retries:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(tmux_start_directory_removes_trailing_slashes) | test(starts_in_a_directory_named_with_tmux_format_syntax)'`.
  Run `8aebb39f-0235-4570-bf3f-08ba14c4fddc`: four passed, 1,026 outside the selection. No selected test returned early
  for unavailable substrate. The tab test reads a directly executed child's inherited `PWD`, so shell cleanup cannot
  hide the bug.
- Reused that runtime pass after review changed only a failure diagnostic and explanatory prose; production behavior and
  successful assertions are identical. The recorded source was base `dc7504a7` plus the working tree, fingerprint
  `783193bb0aa880e10a3fa2b3578f63b0fe9655098dd85788e26be48b05f14410`. Final source checks and Clippy include the review
  changes.
- Examined all main changes since the base: only queue claims for the other two plans. They have no interaction with
  this fix, so no rebase or additional runtime run was needed.
- Skipped broader Rust, browser, desktop and installer suites: the focused tests cover every tmux starting-directory
  call path; no UI, packaging or installer behavior changed.

## The review gate's outcome

Astra high found no issues. Opus 5.5 high found no correctness defects and suggested exposing the actual inherited `PWD`
in mismatch diagnostics and spelling out the test's expanded contract; both suggestions were applied. Both reviewers
received the complete test-authoring checklist. Commit and PR wording passed a separate fresh-context cold read. The
separate report cold read found this account sufficient for approval or follow-up, with no public-repository hygiene
issues.
