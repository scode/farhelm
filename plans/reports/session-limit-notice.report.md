# Notices when the session list is cut

## What this was about

Farhelm cuts each host's session list and the merged list to 500 sessions. The sidebar count says it could not read the
list to the end, which left people to work out which ceiling they hit. The decision was to keep the limit and use the
navigation rail's bell to explain the cut and ask for feedback. Making the list scale beyond the limit stays in the
backlog.

## Things you should know

Alongside the sidebar's count warning, the bell names the host when its saved list leaves out sessions. The bell also
shows a separate Farhelm notice whenever the unfiltered list across all hosts exceeds 500. Both can show at once.
Choosing **Send feedback** opens the regular feedback dialog and returns focus to the bell when it closes.

The notices go quiet when the respective list fits again. A notice that returns before you clear it keeps its existing
history and read mark. An older supervisor that does not identify its installation to Farhelm still contributes the
sessions it lists while connected to the merged count. Its own cut appears in the sidebar count warning only, a caveat
accepted in the plan.

## Open questions and possible follow-ups

The existing backlog item covers scaling the session list.

## PRs

- [#4150](https://github.com/scode/farhelm/pull/4150/changes) reports host and fleet session ceilings and opens the
  existing feedback dialog from either notice.

## Checks

- `cargo nextest run -p farhelm-helm -p farhelm-helm-client -p farhelm-gui --lib -E 'test(/^notices::/) | test(/^rail_notices::/) | test(/^feedback::/) | test(/^collections::/) | test(/^aggregate::/) | test(/^store::tests::schema_/) | test(cache_truncat) | test(/^ui_channel_tests::marking_a_notice/)'`
  passed all 111 selected cases on Linux without runtime skips. It covered independent host and fleet transitions,
  read-state retention, upgrade of existing notices and the rail-to-feedback handoff. Recorder run
  `d343e983-47c1-49ce-b2d7-7b62598b4645`.
- `python3 scripts/check-gui-goldens.py` passed all 258 Linux screenshot comparisons. Recorder run
  `c0b9f047-07a3-4fae-a275-253f287da7e7`.
- `cargo clippy -p farhelm-helm -p farhelm-helm-client -p farhelm-proto -p farhelm-gui --all-targets -- -D warnings` and
  `python -B scripts/check-test-sleeps.py` with the isolated interpreter passed.
- Reused nextest, Linux goldens and Clippy from the reviewed patch at `4376a689`. The rebase preserved the patch. The
  intervening launcher, documentation and screenshot-staging changes leave the ordinary host answers used by these
  fixtures intact. Rust formatting, changed Markdown formatting, changelog format and whitespace checks passed again
  after rebasing.
- Skipped native macOS validation and the full workspace suite. The focused Linux tests exercise the notice store, the
  merged count, and the shared desktop panel and feedback handoff.

## Review gate

Passed. A fresh GPT-6.2 Sol reviewer at high effort found that concurrent changes could combine two different fleet
counts and mark an unread notice as resolved. The revised code takes a consistent count before saving the notice. It
also tests a fleet notice reopening without an alert and uses past tense when showing a resolved notice. The reviewer
rechecked the changes and closed all findings. A cold reader checked the commit message and PR text.
