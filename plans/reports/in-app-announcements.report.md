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

### Landing

Landed with one instruction fix and one test fix. The helm now keeps a queue of announcements and their shown history,
the app shows queued ones in a dialog after sign-in and Welcome and lists shown ones under Help, and release requests
now ask the maintainer up front which announcements a release carries. The release table ships empty, so no announcement
appears yet.

- #2978: the helm's announcement queue and history, and the release announcement table.
- #2979: the dialog, the Help item and the specification.
- #2980: the release instructions, and the removed TODO entry.

#### What the landing fixed, and why

#2980 adds a merge an agent may make on its own, a release candidate's small announcements PR. The root agent
instructions list every such merge by name in their testing and validation section, and that list did not name it, so an
agent reading only those instructions would have found it unauthorized. The landing added it there; the instructions
check passed.

Two helm tests rebuild a database at schema 26 from the current one, dropping every table and column added since, and
compare it with the frozen schema-26 definitions. #2978's schema 69 adds two tables (the announcement queue and the
build that created the database) that the rebuild did not drop, so both failed in the whole workspace run; the executor
had run only focused helm tests. The landing added the two tables to that rebuild. No product code changed.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained vs16-emoji-width (emoji widths in new terminals), launch-progress (a waiting status
for slow launches, Restarts and Replaces) and session-summary-beside-list, plus, while this round's checks ran, a
test-harness fix and flake records that touch neither the app nor the helm. A fresh-context review found no interaction:
the Help item appears only once the helm has shown an announcement, so the help menu's tested contents and order are
unchanged; an announcement never opens over a launch in progress, because the hidden launcher still counts as an open
dialog until the launch ends, while launch-progress's status pill stays visible and announced; the new styles use only
theme tokens, as the stylesheet test requires. The rebase had no conflicts.

#### Things you should know

- The helm's database schema moves from 68 to 69. As with the previous schema steps, an older helm refuses a database a
  newer one has opened, so going back to an earlier release after this one needs a restored database. SPEC.md asks that
  a missing downgrade path be raised with the maintainer; the plan's decisions do not record that it was, and the
  executor's report mentions it only in passing.
- The release instructions' reading lists for release candidates, stable releases and fixing forward do not name the new
  "In-app announcements" section, though their steps point to it.
- If adding the built-in announcements fails at startup, the helm refuses to start; today that needs an invalid table
  entry, which three separate checks reject.

#### Checks

This round landed only this plan, stacked on main `f0ab17a7`. On the stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 403 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (with the
  landing's instruction fix: no errors; three warnings about older reports), the docs website build (`bun run build` in
  `website/`), the workspace debug build and the UI's release web build: all passed.
- The UI's JavaScript tests (`node --test` in `crates/farhelm-ui/js-tests`, the stylesheet token test included), run
  `2678edf4-6fe7-443b-9530-bb715cf55d0c`: all 516 passed. The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run
  `e4359fc4-48b4-450d-bffa-456c3b278a14`: all 842 passed; its desktop-build tests,
  `cargo nextest run -p farhelm-ui --features desktop`, run `e99d6e1f-5805-4675-a8ce-bb7f38537f8b`: all 1031 passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, no retries, on a machine with rootless Podman, run `4215fac4-de8e-4232-af5c-959afd58db34`: 5436 passed, 70
  skipped by the test runner, 7 failed. Two were this plan's, the helm tests that rebuild a database at schema 26
  ("checkout open existing refuses older schema without migrating" and "checkout migration from 26 recreates the config
  tables"), which the landing's test fix addresses; after it, `cargo fmt --all -- --check` and
  `cargo clippy -p farhelm-helm --all-targets -- -D warnings` passed and the helm's whole library suite,
  `cargo nextest run -p farhelm-helm --lib`, run `64ec035a-9474-47fc-b9d2-c342a808b674`, passed all 1359. "Shallow pause
  resumes without reset or replay" is a recorded open flake. Two tests of sessions whose tmux died across a supervisor
  restart (they must stay listed as exited while attaching fails) and one of an OMP agent's report that arrives before
  its terminal pane is known have failed the same way on main for three rounds, in code this plan does not change;
  recorded nowhere yet, the maintainer was told in conversation, and finding their cause is an open follow-up.
  "Websocket round trip uses IPv6 loopback fallback" failed on its own fixture's premise (an IPv4 listener appeared on
  the port its IPv6-only fixture holds, on a machine shared with other agents' tests) and passed three times alone (runs
  `e2e3220f-88f6-4204-8126-02090f79667b`, `67659415-eb24-4e38-8fae-269cc072dae2`,
  `37beb870-7315-419a-9a84-f0431822199e`); one failure, not recorded as a flake.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each booted the usual three test stacks and ran its share of the suite's projects through the recorder's
  generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack before the landing's helm test fix, which changes no product code; Node 24.16 there, against 24.20
  locally), runs `59b5a325-0fd8-41bd-ad3c-0c7edf57073c`, `1e72d5f3-a2d4-419d-8a4d-6614b3f92eb7`,
  `1485f1f6-69dc-40c9-8faf-e4a0684b1435`, `89c2a0c5-ebd1-4c52-8e31-c22596d657d3`,
  `11bdb5ec-880e-438e-b494-987424e3cbd5`, `5f54d51a-2b29-4b9a-89b3-8c35ffe62067`,
  `587acfaf-3194-40d9-b158-0d86d97f1b2a`, `fab34beb-dc1c-4d9a-b054-6cf4c71bc0df`,
  `3fcbadbf-9a43-4dc6-be2a-6543e14c1a72`, `bcc68855-3e06-4fee-9f52-0430fcd52527`: 2918 passed, 18 skipped (17 by the
  specs' own design, among them WebKit's clipboard limits and the real-agent leg; one in the provisioning shard printed
  no reason), 2 failed, both recorded as flakes by open draft PRs: "post-clone commands with carriage returns refuse
  edits but can be cleared" (WebKit, #2549) and "a wrong-host UPDATE acceptance warns on its source row and claims
  nothing" (Chromium, #2670). Then the browser suite's host-terminal run (every stack's terminals in terminal hosts,
  `FARHELM_E2E_SUITE_TERMINALS=host`) the same way, runs `84b61527-fc20-4077-870a-c916b3bc668e`,
  `36d0696d-f127-4ab8-a7af-350aced0d275`, `7033e514-e01f-435e-993c-57b6e4ba5126`,
  `51edd242-b79e-4713-9870-c0a7022842d0`, `1274b43f-c703-4332-b90b-3e45b1c04f01`,
  `c925a890-f51a-4ea0-b3b3-7c8b6d42b7fc`, `7d7a004b-f37d-452d-bfae-a64ee7bf608e`,
  `d70c7024-ae4f-4172-87da-c21680228f62`, `b68342d4-bce4-4e39-92c5-bdeb4ac4aabb`,
  `698f8f26-2d91-495d-a0d0-4d97cf7337db`: 2078 passed, 75 skipped by the specs' own design, 3 failed. Each passed three
  re-runs on the same build (runs `993cab6d-f635-42e0-9dff-833ce9b554f3`, `633348d2-d992-4047-9130-d75f76382776`,
  `8d7be18c-fefc-431e-a0d4-f42797f90c8e`): "create-intent-key-rebinds-on- retarget" (WebKit, multi-host), "a pending
  post-clone save orders reopened settings before the next save" (WebKit) and "header controls stay interactive and text
  stays selectable beside the forced-class spacer" (Chromium; double- clicking the version selected nothing, as once in
  the previous round). None is recorded as a flake: the first two are single failures, and the landing judged the
  third's two failures in two rounds, each followed by three passing re-runs, not yet enough to establish one.
- Reused: nothing. Not run: the doctests (no doc examples changed), the desktop smoke tests (no desktop-specific code
  changed), the development screenshot review (no shot changed) and the native Mac app (not runnable on Linux).

The executor's report names its commits before the landing's rebase and calls the PRs drafts; what landed is those
commits rebased onto current main, with the test fix in #2978's and the instruction fix in #2980's. Nothing else in it
became untrue at landing.
