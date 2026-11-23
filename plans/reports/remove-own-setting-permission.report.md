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
