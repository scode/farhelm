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

### Landing

Landed on 2026-10-06 (UTC) as #1673 (trailing slashes removed from the directory a terminal process starts in), one
squash commit on main.

#### What else was on main

Nothing that could interact: between the commit the change was built on and the landing, main gained only the planning
queue's own bookkeeping. In the same round, the installer-feedback-prompt plan lands right after this one; the two touch
different code (this one the supervisor's terminal start, that one the installer's closing message) and remove different
TODO entries, which merge without conflict.

#### Review before merging

A separate reviewer that had not worked on either plan checked both before anything merged and found nothing outside the
PR that the change breaks. The changed code is the only place the supervisor hands tmux a start directory, covering a
new session, a restart of its agent and a new terminal tab. No test anywhere uses a folder with a trailing slash, so
none sees a different working directory. The session's stored folder, and so the folder the app shows, is unchanged, as
the report says: a folder stored with a trailing slash still shows one in the app. One note: the updated tab test now
runs `python3` inside a tmux pane, the only Rust test that does; the release gate already needs `python3`, so this costs
nothing there.

#### Checks

- Reused: the report's checks. The code on main after the merge is identical to the PR they ran on, and nothing but the
  planning queue's bookkeeping reached main in between.
- Skipped: running anything again during the landing, for the same reason.

Nothing in the report above was made untrue by the landing.
