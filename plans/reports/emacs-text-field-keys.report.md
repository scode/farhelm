# Emacs Control-key editing in text fields

## What this was about

Ctrl+F and Ctrl+B did not move the cursor in Farhelm's text fields. On Mac and Linux, Ctrl+F and Ctrl+B now move by a
character, Ctrl+N and Ctrl+P move down and up, and Ctrl+A and Ctrl+E go to the start and end of the line. Ctrl+D and
Ctrl+H delete forward and back, and Ctrl+K deletes to the end of the line. The bindings apply to single-line fields and
text areas.

## Things you should know

- Where Up and Down already move through suggestions, Ctrl+N and Ctrl+P do the same. When the terminal has focus, all
  nine keystrokes still go to the program running there.
- As decided during planning, using Ctrl+A to go to the start of the line removes the Select All shortcut in Linux text
  fields. On Mac, Cmd+A still selects all.
- In a text area, pressing Ctrl+K at an existing line break joins that line with the next. This is the existing behavior
  of the editor action used for Ctrl+K.
- The **Keyboard shortcuts** page now documents these keys.

## Open questions and possible follow-ups

The Mid term TODO records the decision still to make about a Select All shortcut in Linux text fields.

## Pull requests

- [#4279](https://github.com/scode/farhelm/pull/4279/changes) adds Emacs editing keys and tests their behavior in text
  fields and a focused terminal.

## Checks

- Ran `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, scoped
  `dprint check` on the changed prose, `python -B scripts/check-test-sleeps.py`, and
  `python3 releasing/check-changelog.py format`. All passed.
- Ran `python3 scripts/check-agents-md.py` with zero errors and four existing warnings. The website build
  (`bun install --frozen-lockfile` and `bun run build` under `website`) built 37 pages and validated all internal links.
- Reused 442 passing Linux desktop tests from the working tree based on `ebdf281b`, recorder run
  `3f2a08c9-557b-41c7-bbfb-01b0d01496d0`. The selection covered the keymap, shared text inputs, and editors whose test
  setup used Select All. The rebase changed only the plan queue and an unrelated TODO entry. After the run, the only
  Rust changes were stronger assertions in the multiline test and a comment.

  ```sh
  cargo nextest run -p farhelm-gui --lib -E 'test(/^(actions_tests|components::tests::text_input|approvals::host_add_tests|feedback::tests|hosts::(add|settings)::tests|launcher::(custom_flags_tests|folder_list_tests|replace_with_tests|search_view_tests|view_tests)|session_list::(marking_tests|rename::tests)|session_pane::carried_tests|settings::(tests|credential_tests|recipe_editor_tests)|terminal::set::tests)::/)'
  ```
- The exact middle-line test was run separately on Linux after the rebase to cover the new assertions. Its runner is
  `cargo nextest run -p farhelm-gui --lib -E 'test(=components::tests::text_input::control_keys_respect_line_boundaries_in_text_areas)'`,
  recorder run `9efd6bc1-d5f0-47d1-a06b-c71185365376`. Passed with no runtime skip.
- Skipped native macOS runtime testing because validation ran on Linux. Skipped desktop screenshot and end-to-end smoke
  checks because the diff changes no rendering, and the focused desktop tests exercise the production keymap and the
  path that sends keys from the terminal to its program. The rest of the workspace suite is outside the changed desktop
  paths.

## Review gate

The required independent review found two omissions. The spec and shortcut page now state the loss of Linux Select All
explicitly, and the test now checks that Ctrl+A, Ctrl+E and Ctrl+K work within the middle line of a text area. The
review found no input routing defect.
