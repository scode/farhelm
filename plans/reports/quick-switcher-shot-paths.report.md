# Quick switcher screenshot folders

## What this was about

The v0.40.0 release screenshot of the quick switcher was dropped when the privacy check found the fixture sessions'
temporary working directories on their rows. The session list and details bar already showed folders supplied by the
screenshot scenario. The switcher fetched its own listing and missed that substitution.

## Things you should know

The quick switcher in a screenshot build now shows and searches the folders defined in the screenshot scenario. Rows
follow the usual activity order, and the fixture sessions run in their original working directories. This PR covers the
capture tooling for future screenshots. The new image is private validation evidence.

## Open questions and possible follow-ups

None.

## PRs

- [#4138](https://github.com/scode/farhelm/pull/4138/changes) applies scenario folders to the quick switcher and tests
  the rendered rows and folder search.

## Checks

- `cargo nextest run -p farhelm-gui --lib -E 'test(session_list::quick_switcher::) | test(store_tests::) | test(agent_startup::)'`
  with the pinned nextest and the test recorder at `771f9f87ebff`. All 61 selected tests passed without runtime skips.
  Recorder run `52c0b8e8-d452-4729-8d8c-a8795c6ae0a2`.
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `dprint check TODO.md`,
  `git diff --check`, and `python -B scripts/check-test-sleeps.py` with the isolated interpreter all passed.
- Started a private desktop with
  `scripts/gui-agent.py <run> start --no-build --scenario docs/docs-shots/scenario.json5 --size 1800x1200 --scale 2 --appearance dark`.
  Searching `src/api` in the switcher found the scenario's “migrate auth tokens” session and drew its staged folder.
  `scripts/gui-agent.py <run> capture <image> --keep-focus --park id:quick-switcher-templates-hint` passed the privacy
  checks before and after the shot and wrote a 3200×2000 PNG. An earlier run on a narrower virtual monitor passed the
  privacy checks and was refused at cropping because its window was maximized. The successful run used the docs runner's
  monitor dimensions and an explicit wait for an idle frame.
- Used fresh checks throughout. Skipped the full workspace suite because the focused tests and private desktop capture
  cover the screenshot substitution.

## Review gate

A fresh gpt-6.2-sol reviewer at high effort identified two low-priority issues. The matcher comment now describes the
unescaped values it searches. The regression test assigns staged ages that would reorder the rows if the switcher sorted
by them again. A separate cold reader checked the draft title and description.
