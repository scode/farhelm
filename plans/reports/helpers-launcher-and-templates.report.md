## What this was about

The launcher and Templates repeated several rules for spelling labels, explaining failures, and looking up templates.
The same choice could drift between surfaces if a future change updated one copy. The screenshot tests also repeated the
setup that loads test data before opening a dialog. The stack shares those rules and fixtures. All displayed words,
requests and Linux reference pictures match the behavior on the main revision used for the final check.

## Things you should know

- Searching for `default` still finds the default reasoning effort. The first draft had left a second spelling in the
  scoped search path. Review caught it, and the final code uses the same fixed search word in both paths.
- Recent setups still show the permission the agent will use. Start keeps its chip and hides the ordinary default.
  Search shows the permission inline. Shortened folder labels still keep each hidden-character escape whole.
- When Farhelm cannot resume sessions for the selected agent type, Restart with still says that type's sessions cannot
  be resumed. It keeps a separate explanation when there is no captured conversation to resume.
- A missing template name still gets its own explanation. When the template list is unavailable, the launcher and quick
  switcher explain how to retry. A queued quick-launch sequence still reports a read failure if the helm says its list
  is unavailable, even if earlier names are retained. Before the first list arrives there are no names to match.
  Suggestions and duplicate-name checks need an available list.
- The template editor and Save as template share the same Base name and Fixed name control. The launcher and template
  editor still load the same model catalog when needed and keep their own behavior if the read fails.
- Saving a template can display a repository recorded as free-form text. It keeps that text verbatim. The shared
  checkout label takes a repository with a separate owner and name, so it is used for launcher summaries and recents.
- Seventeen screenshot setups now wait for their test data through the same helper. The attached-terminal scenes keep
  their own attachment and stream lifetime rules. Trash waits for its host listings. The screenshot rows share test
  timestamps for activity and last seen. The unread row retains an older last-seen time.
- The stack removes its share of the combined TODO entry. The physical keypad lookup and other pending helper work still
  listed on main are retained.

## Open questions and possible follow-ups

None.

## PRs

- [#4607](https://github.com/scode/farhelm/pull/4607/changes) Share effort wording across search, choices and Restart.
- [#4608](https://github.com/scode/farhelm/pull/4608/changes) Share managed checkout labels in the launcher.
- [#4611](https://github.com/scode/farhelm/pull/4611/changes) Share Restart refusal wording.
- [#4615](https://github.com/scode/farhelm/pull/4615/changes) Preserve whole escapes when a folder label is shortened.
- [#4616](https://github.com/scode/farhelm/pull/4616/changes) Share recent-setup permission presentation.
- [#4618](https://github.com/scode/farhelm/pull/4618/changes) Share template name lookups and availability checks.
- [#4620](https://github.com/scode/farhelm/pull/4620/changes) Share the session name choice and model catalog read.
- [#4621](https://github.com/scode/farhelm/pull/4621/changes) Share screenshot fixtures for dialogs and session ages.

## Checks run, reused and skipped

- The final tree, checked at `bc5b3d50` on main `2a2e6568`, passed all 358 selected tests
  (`b6e7b65a-6760-4926-a2bf-df45c96becb9`). There were no runtime skips. Command used:

  ```sh
  cargo nextest run --locked -p farhelm-gui --lib -E 'test(/^launcher::(checkout_tests|environment_view_tests|wake_tests|folder_list_tests|view_tests|search_view_tests|recents_view_tests)::/) | test(/^templates::(dispatch_tests|editor_tests|save_tests)::/) | test(/^host_text::/)'
  ```

- `python3 -B scripts/check-gui-goldens.py --jobs 4` matched all 406 Linux screenshot references
  (`29538dab-b511-4449-9149-a3e47cb731e7`), including the compact checkout and wide-terminal-glyph references that had
  landed on main. The screenshot catalog and reference manifest are identical to this plan's base.
- `cargo fmt --all`, `dprint check TODO.md`, `git diff --check` and the isolated
  `python -B scripts/check-test-sleeps.py` passed. The sleep checker found a rationale for all 309 test delays.
- The seven earlier PR patches were identical after the rebase. Their focused tests are reused alongside the final
  interaction run. The last adjustments to those patches narrowed one helper's visibility and clarified two comments.
  The first PR's default-effort adjustment had its own exact test selection.

Earlier nextest runs used `cargo nextest run --locked -p farhelm-gui --lib -E '<filter>'`. The text escape run added
`-p farhelm-proto`. The recorder IDs identify the exact filters.

| Checked revision | Tests passed | Selection                                                                                               | Recorder                               |
| ---------------- | -----------: | ------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| `64df6e21`       |          176 | Launcher search, view and Restart with, permission words                                                | `a1f81732-af42-461b-8922-554bd81590c6` |
| `6cb2ce39`       |            2 | `effort_rows_follow_the_agent_and_its_model` and `the_whole_keyboard_path_builds_a_launch_and_sends_it` | `a0520c09-9d77-4063-9359-df523e90c923` |
| `a588c02c`       |           81 | Checkout, recents, recent setup view, environment view                                                  | `9dc93143-c9b4-472e-9a7c-2b2bd041892b` |
| `4992e012`       |          109 | Launcher entry, Restart with, session header                                                            | `b4ffbfa3-c988-4411-affc-4644d1a6078f` |
| `1a5a3c38`       |          316 | Protocol library, launcher folder list, GUI host text                                                   | `17897a49-f552-45ca-852d-54a3d7a40b5e` |
| `46dadcab`       |           77 | Recents, recent setup view, recent environments, launcher search view                                   | `2d49ff9c-aab7-471e-bb51-fdce90a03e80` |
| `3dd02a9d`       |          457 | Template library, quick switcher, launcher search                                                       | `8e967e80-d72b-4b3b-9fed-0d715d79ce1d` |
| `a3dff3d9`       |          310 | Template editor, dialog, save and dispatch, launcher view, models, search view and launch               | `a0d4cab1-7d10-4d89-a327-d20d17a076ae` |

The final screenshot comparison supersedes the earlier comparisons. Each used `python3 -B scripts/check-gui-goldens.py`,
in compare mode, with the listed arguments.

| Checked revision | References matched | Arguments                                                                                  | Recorder                               |
| ---------------- | -----------------: | ------------------------------------------------------------------------------------------ | -------------------------------------- |
| `64df6e21`       |                 30 | `--scene 'launcher-*'`                                                                     | `68d0e8f6-146d-429f-8fac-8c7d171d6b1c` |
| `a588c02c`       |                 32 | `--scene 'launcher-*' --scene template-launcher`                                           | `e00fe484-ec2c-4c4a-bd70-96865e382b6d` |
| `4992e012`       |                  2 | `--scene launcher-restart-with`                                                            | `171ded62-1642-44f5-b00f-8af4b744946e` |
| `1a5a3c38`       |                400 | `--jobs 4`                                                                                 | `d0169e70-5442-413b-ad21-9d684cd5a3df` |
| `46dadcab`       |                 32 | `--scene 'launcher-*' --scene template-launcher`                                           | `a7b7264c-1e76-4f67-bebc-cf8107b1534f` |
| `3dd02a9d`       |                 48 | `--scene 'template-*' --scene templates-dialog --scene 'launcher-*' --scene sequence-pill` | `3b987192-63ea-4126-a497-c3c90678a875` |
| `a3dff3d9`       |                 46 | `--scene 'template-*' --scene templates-dialog --scene 'launcher-*'`                       | `e1734d29-e819-4ac2-8208-8ab37222dd4e` |

The plan called for focused GUI and text checks. Workspace-wide tests and live host or desktop acceptance were outside
these changes. The plan's visual gate used pixel comparison on the Linux software renderer.

## Review gate

A fresh Sol reviewer checked each draft against its own base. Accepted findings tightened the shared query word, helper
visibility and two comments. The final fixture review checked which event opens each dialog, what its subscription keeps
alive, the session rows' timestamps and the TODO deletion. It found no issues. Each commit message and PR title passed a
cold read.
