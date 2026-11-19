## What this was about

The in-app announcement “Restart old terminals before support ends” told people on older Farhelm releases to restart or
replace their sessions before support for Farhelm’s old tmux terminals ended in v0.39.0. The announcement has been
removed from the release table so later builds stop adding it when people upgrade.

## Things you should know

If your helm already queued the warning, you can still see it after upgrading. Warnings you have already seen remain in
Announcements history.

## Open questions and possible follow-ups

None for this round.

## Pull requests

- Retire the obsolete 0.37.0 announcement and remove the completed TODO entry.
  [#4140](https://github.com/scode/farhelm/pull/4140/changes)

## Checks

- The selected Linux test passed with no runtime skip in recorder run `8eaac0ae-b5d4-4fbf-9e7d-fe20ef3363a1`. The
  recorded command was `cargo nextest run -p farhelm-helm --lib -E 'test(release_announcement_table_is_valid)'`. It ran
  against `f2fc9607` with the two-file patch applied. The draft uses the same patch on newer main. The intervening
  release-note and plan-queue changes affect neither the announcement table nor the helm test.
- `dprint check TODO.md crates/farhelm-helm/src/release_announcements.json` and `git diff --check` passed.
- `git grep -n -F '0.37.0-old-terminal-support'` found only references in the plan after the edit.
- Skipped the full test suite because this change removes one table entry and the focused test reads the compiled table.

## Review gate

Independent code review found no issues. The commit message passed a fresh cold read after clarifying the version and
which copies of the warning are affected.
