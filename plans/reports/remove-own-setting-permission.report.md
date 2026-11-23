# Agent permissions and guarded Replace

## What this was about

A saved Codex session appeared to have the Guarded shield. When its owner chose Replace, the host asked them to confirm
a YOLO launch. The session had been saved without a permission choice. Farhelm already required YOLO confirmation for
that launch, but the old dashed shield looked solid at row size.

The maintainer chose to retire **Agent's own setting**. Farhelm now shows these saved agent launches with the slashed
YOLO shield and keeps requiring confirmation on hosts that ask. When a saved YOLO agent supports Guarded, the Replace
question offers **Replace as guarded instead**. Choosing it creates a new guarded session on the same host before
removing the original.

## Things you should know

- The upgrade records YOLO on older agent sessions with no saved permission and preserves their start and resume
  commands. The shield shows how Farhelm classifies the launch for confirmation. The saved command shows the arguments
  the session originally used. Cursor defaults to YOLO for new launches and passes `--force`. Cursor still offers
  Custom.
- The guarded Replace answer uses the current workspace trust settings and updates launch history and remembered
  defaults as **Replace with** does. It carries over the visible directory and title, harness, model, effort and
  wrapper. The original session's Delete guard still applies. A visible change to the source host or launch asks you to
  choose Replace again. The guarded answer needs no YOLO confirmation or host setting change.
- Retrying an already accepted create still recognizes a saved launch across the upgrade when only the recorded YOLO
  value changed. The create limit allows the two bytes added by that value, including launches recorded as YOLO from the
  start, so saved commands that already fit can still be copied.

## Open questions and possible follow-ups

A title or directory edit from another client can reach the helm before this window sees it. If you answer Guarded
Replace in that interval, the replacement can inherit the older value. **Replace with** already has this race. A shared
follow-up could let the helm carry over the latest value in the same operation when the user asks to preserve it.

## PRs

- [#4669](https://github.com/scode/farhelm/pull/4669/changes) Saved permission classification, Cursor default and
  upgrade.
- [#4672](https://github.com/scode/farhelm/pull/4672/changes) Guarded Replace, permission displays and screenshots.
- [#4673](https://github.com/scode/farhelm/pull/4673/changes) Specs, Cursor, Agents, Start a session, and the completed
  TODO entry.

## Checks

- Focused proto, helm, supervisor and launcher tests covered the Cursor default and Custom, saved-launch classification,
  migration and Replace retries. Run `5bfbfbe3-f9fe-4fe0-bf7d-5e8775ab229d` at `8d8a6256` passed 480 of 482. The two
  fixtures still expected Cursor to clear the choice and older records to omit the mode. Their corrected checks passed
  in `312f25e5-0208-43da-accd-0d0b8b1e7671` at `1ce3ec76`. The rebase kept the tested patch.
- The desktop selection for launch, permissions, templates and Replace passed 734 of 737 at `1472b0d5`
  (`dfbc7624-e06b-4d6d-a0d4-de67b7b5f685`). Three fixtures had stale expectations for the host, button order, and the
  removed permission search term. The corrected fixtures and affected Replace flows passed all 45 selected checks at
  `f15a4ba4` (`191a7220-da7c-47bf-844c-0edec0743bda`). The other passing desktop observations cover the unchanged flows.
- Rust formatting, test delay annotations, Markdown formatting, whitespace, changelog fragment format and the docs site
  build passed. The site checked all internal links.
- Strict `cargo clippy --all-targets -- -D warnings` passed at `0d1e5550`. The first lint run flagged the guarded
  callback type. Restoring a type alias addressed it without changing runtime behavior. Only the tall screenshot fixture
  changed after the linted source.
- The full Linux GPUI screenshot run produced 408 captures (`66ec9adf-26cb-4686-b550-0f5f56bda98e` at `2a69d4b6`). The
  visual comparison verified the shield and wording updates and the full guarded Replace question with all four answers
  in both themes. Published 30 changed and two new references. The macOS references were outside this Linux run.
- The workspace suite is pending at handoff for the landing monitor. Executor runs targeted the changed paths.

<details>
<summary>Recorded validation commands</summary>

`cargo nextest` and the screenshot capture ran through the test recorder.

```sh
# Core selection
cargo nextest run --locked --test-threads 4 --retries 0 -p farhelm-proto -p farhelm-supervisor -p farhelm-helm -p farhelm-gui --lib -E 'package(farhelm-proto) | (package(farhelm-supervisor) & test(/store::tests::.*(migrat|schema)|saved_launch_identity_survives_the_permission_migration|the_persisted_fingerprint_encoding_is_pinned/)) | (package(farhelm-helm) & test(/^(launches::|yolo_guard::)|store::tests::.*(launch_history|launch_template|remembered_permission|only_an_explicit_selection)|sessions::.*(replace|yolo)/)) | (package(farhelm-gui) & test(/yolo_tests|view_tests::each_agent_type|draft_tests::(the_first_agent|restart_with_shows_the_stored_permission)|recents_tests::the_guarded_default|restart_with_tests::guarded_over_an_older|search_tests::permission_rows|row_marks_tests|approvals::model_tests|templates::summary_tests|^permission::/))'

# Core fixture rerun
cargo nextest run --locked --test-threads 4 --retries 0 -p farhelm-proto -p farhelm-supervisor -p farhelm-helm -p farhelm-gui --lib -E 'test(=store::tests::schema_26_converts_structured_rows_and_keeps_the_rest_legacy) | test(=launcher::draft_tests::the_first_agent_escalates_only_a_deliberate_choice)'

# Desktop selection
cargo nextest run --locked --test-threads 4 --retries 0 -p farhelm-gui --lib -E 'test(/launcher::(yolo|restart_with|view_tests|draft_tests|recents_tests|search_tests|search_view_tests)|session_list::(row_marks|lifecycle|summary)|session_pane::header_tests|approvals::model_tests|templates::|^permission::|hosts::settings/)'

# Desktop rerun
cargo nextest run --locked --test-threads 4 --retries 0 -p farhelm-gui --lib -E 'test(/session_list::lifecycle_tests|session_pane::header_tests::.*(replace|host_write)|a_running_sessions_card_names_its_agent_host_folder_and_times|choice_words_before_a_harness_say_to_choose_one/)'

# Linux screenshot capture and reference publication
python3 scripts/check-gui-goldens.py --update
python3 scripts/gui-goldens.py publish
```

</details>

## Review

The independent review gate passed after addressing the findings for all three drafts. Core review covered replay across
an upgrade and the create size limit. Desktop review covered eligibility, button order, guard preservation and the
displayed source at answer time. Documentation review corrected the shield, workspace trust, template permission and
launch-confirmation descriptions.

### Landing

#### What landed with it

The plan's three PRs (#4669, #4672, #4673) and one landing fix (#4685) landed in this round ahead of
tensorlake-leftovers-discovery-failure, which touches the helm's Tensorlake leftover list and Settings and does not
interact with it. Since the plan's base, main had gained the Claude folder trust guard for the home directory and the
folders above it, Tensorlake Suspend and Resume requiring a known sandbox, the session list's notice for a sandbox's
Stop and suspend action that runs past 20 seconds, and changes to the opt-in tests that run real agent programs. The
rebase met only TODO.md, where this plan's removal of its own entry sat beside two entries main had added; both were
kept. While the plan landed, one more change to those opt-in tests reached main (#4674, which keeps their agent hook
setup out of the user's real agent configuration). It touches no launch or permission code, and with both on main every
test target still compiled and linted, and those tests' guard checks passed.

The landing's independent reviewer checked that a saved launch with no recorded permission is never treated as less than
YOLO anywhere the plan reaches: the host's YOLO question, the session's shield, approval cards, agent copies, templates,
launch history and remembered defaults. **Replace as guarded instead** is offered only for saved YOLO sessions whose
agent offers Guarded, keeps the original session's Delete guard, and creates the new session before deleting the old
one. The upgrade runs in one transaction and leaves every saved start and resume command unchanged. No protocol version
change is needed even between different releases: the permission values sent between the helm and a supervisor are the
same as before. A supervisor that has not been updated still sends no value for an old session, which a new helm counts
as YOLO, and an updated supervisor sends YOLO, a value older helms already understand.

#### Fixes made while landing

- The landing's full test run failed one test of the desktop launcher (part of the desktop interface's suite, which that
  run includes); the executor's runs had not included its file. The test checks that in **Restart with** the recent
  setups are shown but cannot be applied. Its sample session had no recorded permission, so under this plan Restart with
  starts it from YOLO and shows only YOLO recent setups, hiding the Guarded setup the test looked for: the same
  consequence as the first item under "Things you should know". #4685 gives the sample session Guarded, since the test
  is about the recent setups, not about old sessions.
- The plan rewrote the rule in the instructions Farhelm gives agents about hosts that ask before YOLO launches, and left
  "there is no override." starting a sentence in lower case. #4685 restores "Nothing you pass changes that: there is no
  override."
- The plan's new reference picture of Replace's YOLO question with its **Replace as guarded instead** answer was missing
  from the screenshot scene list, which that list's own check refuses; #4685 adds it, and corrects a renamed test's name
  in the notes comparing the desktop app with the old web app (`docs/gpui/parity/`).
- #4685 adds a caveat to the release-notes draft, explained in the first item below.

#### Checks

This round landed remove-own-setting-permission (how saved agent launches are classified, a supervisor database upgrade,
the helm's launch history and YOLO question, the desktop launcher, Replace and permission marks, reference pictures and
docs) and tensorlake-leftovers-discovery-failure (the helm's Tensorlake leftover list, a new reply type and Settings)
together. Between them they reach every crate except the desktop app's outer program, so the landing ran the whole
workspace suite, the lints (the desktop app's too), the end-to-end smoke test, every reference picture and the docs site
build on both plans as delivered, stacked on main, then narrower runs after the landing fix and after a mid-round change
to main. The worker runs were on a separate arm64 Linux test worker with a systemd user manager, rootless Podman and SSH
to itself; the rest ran locally on x86_64 Linux. None of them used a real agent program or a real Tensorlake account:
the tests use stand-ins, as the reports say.

On both plans as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `13298a6c-7f5f-46ce-a761-2caa08b85278`): 8413 of 8414 passed and none printed a runtime `SKIPPED`. The failure is a
  test of the desktop launcher's recent setups that remove-own-setting-permission broke, which that plan's landing fix
  (#4685) corrects; the failed run is kept.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's two
  `cargo clippy -p farhelm-desktop --all-targets` runs (with and without `--features agent`), on the test worker:
  passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `4e8cb1fd-10e7-4211-8f0a-d30516dcbb4d`): of
  its 50 steps, 49 passed and one was skipped as always on Linux (paste, which headless Mutter cannot give the app).
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `768fb235-2adc-486f-a06c-ed76fcf489bf`): every picture
  matched its reference, including the pictures remove-own-setting-permission added or replaced.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and the docs site build (`bun run build` in `website/`), locally:
  passed.

After remove-own-setting-permission's landing fix (#4685):

- `cargo nextest run -p farhelm-gui -p farhelm` with the desktop launcher's and the agent instructions' tests selected,
  on the test worker (recorder run `85f1444a-54cb-4501-9740-6b6cafaf5ea6`): all 548 passed, the corrected test among
  them.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `python3 -B scripts/test-gui-screenshot-batch.py` (the screenshot scene list's own checks, locally and not through the
  recorder, so it has no run id): all 47 cases passed. `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py`, `dprint check` and `python3 releasing/check-changelog.py format`, locally:
  passed.

After remove-own-setting-permission merged, main also held a change to the opt-in real-agent tests that landed mid-round
(#4674), which none of the runs above had seen. On main plus tensorlake-leftovers-discovery-failure:

- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed, so every test target still compiles.
- `cargo nextest run -p farhelm -p farhelm-supervisor` with the real-agent test module's guard tests and the
  supervisor's agent and persistent hook tests selected, on the test worker (recorder run
  `eb6f15f3-0088-46ec-b052-ce1b3533b79d`): all 113 passed, none printed a runtime `SKIPPED`.

Reused: the workspace run above for everything outside those selections, since the fix changes one test's fixture, the
wording of one paragraph of the instructions Farhelm gives agents, the screenshot scene list (which feeds only the
screenshot comparison page, whose own checks ran), a GPUI parity notes page outside the docs site and a release-notes
draft, so the reference pictures and the docs site build were reused too; and #4674 changes only the opt-in real-agent
tests and documentation. Skipped: the desktop app's outer program's tests, which nothing in either plan reaches; macOS,
where nothing compiles differently; real agent programs and a real Tensorlake account; CI, which runs only on demand.

#### Things you should know that the report does not say

- I accepted a consequence of the upgrade on your behalf, and am recording it in TODO.md's "Plan decisions to revisit"
  with this round's TODO update. Because old agent sessions that never chose a permission are now saved as YOLO,
  **Clone**, **Replace with** and **Restart with** of such a session open with YOLO selected, and starting it passes the
  agent's bypass flag (Claude's `--dangerously-skip-permissions`, Codex's `--yolo`), where before they opened with
  Guarded for agents that offer it. The user can still pick Guarded before starting, a host that asks before YOLO
  launches still asks and offers to start guarded instead, and a host set to start YOLO sessions without asking starts
  it as YOLO. A Clone started that way also makes YOLO the remembered permission for later new launches. For agents that
  offer no Guarded mode nothing new follows from this: Pi, OpenCode and Goose already counted an empty permission as
  YOLO, and Cursor now starts as YOLO (`--force`) by your decision about Cursor.
  - Why I accepted it: your decision was to record YOLO on those sessions, and once recorded it cannot be told apart
    from a YOLO the user chose, so Clone and its siblings cannot treat the two differently. The plan's criterion that
    agents offering Guarded "continue to resolve to Guarded when composing new launches" is about saved templates,
    launch history and remembered defaults with no permission, which still compose Guarded.
  - The alternative: leave those sessions' permission empty instead of recording YOLO. The YOLO question, the shield and
    every other check already count an empty permission as YOLO, so nothing would get less safe, and Clone and its
    siblings would keep opening with Guarded. That departs from the decision to record YOLO.
  - Your release note calls the change "conservative/safe" and "safe to ignore"; with this consequence that wording
    deserves a second look at curation. The release-notes draft now carries the caveat.
- Possible follow-ups:
  - Some descriptions of the old setting remain: the notes in `docs/gpui/parity/` still describe Cursor's own setting in
    a few places, two TODO.md entries about the opt-in real-agent tests still ask to check Cursor's "Agent's own
    setting", which no longer exists, and a queued review finding about template summaries still quotes the old wording.
  - Two YOLO texts got vaguer for an explicit YOLO choice: the approval card now says "Farhelm applies the host's YOLO
    rules to this launch", and the launcher's YOLO question now says a guarded mode "is available for a new launch".
  - About 70 lines of supervisor code exist only for one case: a create or Replace whose reply was lost before the
    upgrade and is retried after it, when only the newly recorded YOLO differs. The reviewer found it correct but heavy
    for so narrow a case.
  - When **Replace as guarded instead** refuses because the session's launch or host changed while the question was open
    ("This session changed while the question was open"), no test covers that refusal.
