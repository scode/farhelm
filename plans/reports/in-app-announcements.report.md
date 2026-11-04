## What this was about

Serious changes are easy to miss when release notes are their only notice. The maintainer chose a general queue of
announcements: releases supply its first messages, while the dialog and history remain usable by later sources.

The helm now keeps pending announcements and shown history. The app checks after sign-in and Welcome, shows a plain-text
dialog, and offers shown history under Help. RC and stable release requests ask for explicitly selected content before
release work begins. The original TODO entry is removed.

## Things you should know

The release table is empty. This ships no real announcement, and the Help item remains hidden until the helm has shown
one. It never derives announcements from the changelog's Breaking category.

Dismissal belongs to the helm, shared by every client. Closing records exactly the displayed batch; a failed save still
closes and may let it appear again on a later load. Reopening history writes nothing. Each id keeps its original text
forever on a helm that already received it, even when a later release rewords that id.

The release producer includes intermediate RC and dev versions in semver order. Fresh installs receive nothing from
their first version or earlier; older databases without a creation baseline remain eligible for future messages.
Prerelease messages omit release-notes links because curated notes need not exist. The normal one-way helm schema
upgrade preserves existing data and makes older helms refuse the upgraded database.

The dialog summary is “Updates and information from Farhelm.” Existing modal admission keeps late results behind other
dialogs. There is no live announcement subscriber. The release procedure settles selection in the initial reply, carries
prior selections through later candidates, and keeps signing, publication and tag fences intact.

## Open questions and possible follow-ups

No unresolved implementation decision. The first real announcement will be selected during a future release request;
this plan deliberately provides no shipped sample. Synthetic Rust inputs and browser route fixtures exercise delivery
without changing a shared test helm's announcement history.

## The PRs

- [#2978](https://github.com/scode/farhelm/pull/2978/changes): durable queue, shared history, migration and release
  producer.
- [#2979](https://github.com/scode/farhelm/pull/2979/changes): dialog, load check, Help history and browser coverage.
- [#2980](https://github.com/scode/farhelm/pull/2980/changes): request-time release selection and source TODO removal.

All three are draft PRs in that linear order, not merged.

## Checks run, reused and skipped

All runtime runs used the recorder, zero automatic test retries, four nextest slots or the browser suite's three-worker
budget. Rust used pinned nextest 0.9.148; browser runs used the repository-built pinned tmux, Chromium and WebKit.

- `cargo nextest run -p farhelm-helm --lib -E 'test(/^announcements::tests::/) | test(/^release_announcements::tests::/) | test(/^store::tests::/) | test(/^preferences::tests::/)'`:
  208 passed, covering the backend source in `9a4d6ba9`; run `19a6de50-b37d-4d42-bc69-42cb121630b4`. Later edits affect
  only client code and prose, so this evidence is reused unchanged.
- `cargo nextest run -p farhelm-ui --lib`: 838 passed; run `bd0a97e2-ca6d-48fe-8938-e3a3104b9564`. It captured
  `9a4d6ba9` plus the client diff before the keyboard-scroll correction (source fingerprint
  `9fe2ab8d912096138b569bba3dc725e9a0c73bb80aecc02bc36e61e9556a0d41`). No unit-tested logic changed afterwards; final
  web/desktop compilation and browser cases cover that presentation correction.
- `npx playwright test 'announcements\.spec\.ts'`: 26 passed, 13 per engine; run `cb5594d4-4233-49f7-9bbb-e28fe1591c57`.
  The same selection with `FARHELM_E2E_SUITE_TERMINALS=host` and the recorder's explicit environment retention also
  passed 26, 13 per engine; run `0acc1149-2125-4652-bf07-9b2c30f19f2d`. Host metadata and per-test annotations confirm
  that substrate. Both short-window screenshots were inspected.
- `npx playwright test 'welcome\.spec\.ts' 'feedback\.spec\.ts' 'tooltip-coverage\.spec\.ts' 'filters\.spec\.ts'`: 78
  passed, 39 per engine; run `0a141287-9550-49ba-964d-1b743b29ce8b`. All three successful browser runs cover
  `ef1f175d7`, have complete reports and output, and have no skipped or failed cases.
- The first browser attempt, `2ab05cc0-8a94-466f-ba03-194f1ef7015d`, failed during startup because the fixture helper
  had not been built: zero cases ran. Its evidence is retained separately. Building `farhelm-fixtures` corrected
  preparation before the same-scope retry; it is neither a product failure nor a flake.
- Passed: native and fixture builds; Dioxus 0.7.10 web release build; desktop-feature UI compile; Clippy on both changed
  crates with all targets and warnings denied; Cargo format; changed Markdown/JSON formatting; changelog format;
  instruction checks; and the isolated test-sleep checker (403 delays, none unannotated). The instruction checker has
  three pre-existing citation warnings outside this change. A local command-name collision was resolved by invoking the
  pinned Dioxus executable explicitly; the earlier attempt built nothing.

Main remained `5404961d2` at the final fetch, so no rebase was needed. The other four unfinished-plan PR diffs were read
in full: launch progress preserves the ordinary HTTP interface, session-card placement is explicitly scoped, and emoji
width affects terminal output rather than dialog text. This is a scope audit, not combined runtime evidence.

Skipped the full workspace/browser batteries, installer/provisioning checks and native desktop runtime: the focused
storage, modal, menu and renderer checks cover the changed behavior. Release instructions are prose and need no runtime
release execution. No actual release or live installation was changed.

## The review gate's outcome

Each PR received its single fresh-context gpt-6.1-sol high adversarial review. PRs 1 and 3 had no findings. PR 2 found
that long linkless messages lacked keyboard scrolling in WebKit; the existing ScrollRegion component fixed it, a native
Tab/PageDown browser case was added, and the same reviewer confirmed resolution. No findings remain. Commit/PR wording
passed the required cold reads.
