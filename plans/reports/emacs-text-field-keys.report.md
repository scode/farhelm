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

### Landing

#### What landed with it

It landed alone in its round. Since its base, main gained antigravity-support (an Antigravity agent type, with a
launcher chip and hook-based conversation tracking). The independent reviewer checked it against this plan: it adds no
key bindings and no use of Select All, so they do not interact. Your plan for SSH sign-in prompts (#4284) merged during
the landing and touches only TODO.md and `plans/`.

At your request this landing was reviewed with extra skepticism, by me and by an independent reviewer, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The change is one nine-row
table of key bindings, and the reviewer confirmed it does what the report says:

- On Linux the new keys win over the text-field library's own, so Ctrl+A moves to the start of the line.
- None of them collides with a Farhelm shortcut, since every Linux shortcut uses Ctrl+Shift.
- A focused terminal still gets all nine keys.
- Where a field drives a suggestion list, Ctrl+N and Ctrl+P move through it as the arrows do.
- Nothing in the specs or docs still says Ctrl+A selects all in a Linux text field.

#### Fixes made while landing

Three wording fixes, no behavior change:

- **SPEC.md** said "On Linux, this removes the Select All shortcut in text fields", which describes a change rather than
  the product. It now says "Linux text fields therefore have no Select All shortcut."
- **The tests' select-all helper** gained the reason it must stay an action rather than a keystroke. Linux text fields
  no longer have a Select All key, so a test pressing Ctrl+A there would quietly type at the start of the line instead
  of replacing the text.
- **The keymap module's overview** said every binding lives in Farhelm's own key context. It now names the new
  text-editing bindings and why they are registered after the library's defaults.

#### Checks

The plan changes only the desktop app's GUI crate (key bindings and its tests), SPEC.md, a docs page and a changelog
fragment, so the landing ran the GUI crate's whole test suite and the app's end-to-end smoke, which types into the app's
text fields. Builds ran locally; the suite and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plan as delivered, rebased on main:

- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`,
  `cargo clippy --locked -p farhelm-gui --all-targets --features agent -- -D warnings` and the website build
  (`cd website && bun install --frozen-lockfile && bun run build`): passed.
- `cargo nextest run -p farhelm-gui`, from a locally built archive, split in three across boats
  (`aebd7a63-aa68-422a-8620-551ed0678c08`, `84987687-7c51-420f-ae51-e4ee5fd77cc0`,
  `c9f05c2e-7a34-47ff-9eb7-6a720d958eab`): all 3177 passed, with no runtime skips.
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed, and the clipboard
  paste step was skipped, as always on Linux (`7f03baf8-e82b-4b22-96ce-f1921b4acf61`).

On the fixed tree: `cargo fmt --all -- --check` and `dprint check SPEC.md` passed. The fixes change only a SPEC.md
sentence and two comments, so the runs above were reused.

Skipped:

- The reference screenshots (goldens): nothing draws differently.
- The rest of the workspace suite: nothing outside the GUI crate changed.
- Anything on a Mac: the Mac's Cmd+A Select All and its Control keys were not tried on macOS.

#### Things you should know that the report does not say

- At the very end or start of a field's text, Ctrl+F, Ctrl+B and Ctrl+H have nothing to do. The text-field library then
  hands the key to its next binding: its own Search for Ctrl+F and Replace for Ctrl+H. No Farhelm field turns those
  features on, so nothing happens today. If one ever does, Ctrl+F at the end of the text would open the library's search
  panel.
- The changelog fragment is a `fixed` entry, but it also takes away Linux text fields' Select All key; the release
  curation may want to say so plainly.
