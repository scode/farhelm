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

### Landing

#### What landed with it

It landed in a round of nine plans: session-group-follow-ups, model-families, session-limit-notice,
reserved-resource-names, secrets-logs-and-screen, checkout-settings-stale-save, concealed-link-target, triage-misc-nov18
and triage-discards-nov18. Since its base, main gained the previous round's plans (submenus that open on hover, Clone
keeping invisible characters, the retired old-terminal announcement, quick switcher screenshot folders, the update
survival wording) and docs screenshot tooling. No other plan in the round touches the helm's stored notices, and the
reviews found no interaction.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The plan adds what it was asked
for and nothing more.

#### Fixes made while landing

- **The host notice said the opposite of what it meant.** It read "The list shows its newest 500, and you can only reach
  those sessions from Farhelm", which reads as "Farhelm is the only way to reach them". It now says "The list shows only
  its newest 500; Farhelm cannot reach the others."
- **SPEC.md lost your stance on the limit.** The cap paragraph had said that a fleet outgrowing the cap is outside what
  the product is built for and that the notice is the whole of the answer to it, which is your "no graceful handling"
  decision. The plan replaced it with a description of the bell notice. It is back, naming both the session list count's
  "could not read to the end" warning and the new bell notice as that answer.
- **SPEC.md's new bell paragraph** said "more than 500 visible sessions" (the point is that they are not all visible),
  used an internal term for the cut list, described an implementation detail for hosts without an installation identity,
  and addressed the reader as "you". All four are fixed.
- **Two code comments lost their reasons:** why the notice table's ids never repeat (so a stale panel cannot act on a
  newer notice), and the layout of a trash notice. Both are back, the second extended to the new notices.

#### Things you should know that the report does not say

- No test covers two of the cases the plan names: a create that pushes a host past the limit raising its notice, and a
  host whose installation identity changes clearing its notice.
- SPEC_impl.md documents the general notice table and the session-limit notices under the archived-checkout trash
  heading, where few would look for them.

#### Checks

This round landed nine plans together, so these checks cover all nine on one tree: the nine stacks one after another on
main. Between them the plans change the desktop app's GUI crates, the helm and its client, the shared protocol, the
supervisor, the desktop process, SPEC.md, SPEC_impl.md and the triage records. The landing ran the lints, the whole Rust
workspace suite, the desktop process's own tests, the app's end-to-end smoke and the reference screenshots (goldens).
Builds ran locally; the workspace suite and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, `python3 -B scripts/test-gui-e2e-smoke.py`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`, once without and once with `--features agent`: passed.
- `dprint check`: failed on `TRIAGE_OUTCOMES_ARCHIVE.md`, which triage-misc-nov18 wrote unformatted. Fixed.
- `cargo nextest run --workspace --exclude farhelm-desktop`, from a locally built archive, split in three across boats:
  2744, 2653 and 2625 tests, 8022 in all (`1ffb9472-9f12-4890-b0d5-032dd87718d0`,
  `7c3ef63b-2e2b-4903-acff-22cbe9409601`, `7c539d75-d409-48c7-901b-391169215c11`). One failed: the macro
  compile-contract test, which builds a small crate offline from this machine's cargo cache, which a boat does not have,
  so it fails there every time; it passed locally (below). No runtime skips.
- `cargo nextest run -p farhelm-desktop`, locally: 176 passed (`59017e5e-7626-4dca-9c45-0b4972a89aca`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed, the new step that
  drags an unpinned session into a group included, and the clipboard paste step was skipped, as always on Linux
  (`b1164f8b-3d55-4cac-9fb1-a76e9a8da60c`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`da0dca91-646f-483a-a436-b10b43ffafe2`), the new groups
  pictures included.

On the fixed tree (comments, SPEC.md, SPEC_impl.md, changelog fragments, the triage archive's formatting, and one
notice's wording changed):

- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`: passed.
- The bell notices' tests in the GUI and the helm, locally: 26 passed (`80fb276d-1ba3-4b09-a84a-370aef2b9605`).
- `cargo nextest run -p farhelm-testtrace-macros`, locally: 2 passed, the compile-contract test included
  (`a8a9bf1f-6dc9-4e24-8831-e5d4b7f5dbc0`).

The boat suites, the smoke and the goldens were reused for the fixed tree: after them only comments, documents,
changelog fragments, formatting and one notice's wording changed, which the focused runs above cover.

Skipped:

- The Linux app lifecycle: startup and quit are untouched.
- The Mac-only checks: no Mac-specific code changed.
