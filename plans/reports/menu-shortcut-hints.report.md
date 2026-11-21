### What this was about

The desktop menus showed commands without their keys. You wanted people to learn the shortcut where they already use the
command, for example seeing Cmd+L next to Rename. In the sidebar, the open session’s three-dot menu now shows a shortcut
label beside Rename and, where the actions match, beside Delete. These keys act on the open session, so only its row
shows them. The Help menu shows the key for Keyboard shortcuts. The labels use the same chip styling as the Keyboard
shortcuts dialog and fit the existing menu width with the full Linux names.

### Things you should know

An ended session without terminal tabs can be deleted immediately from its row menu while the shortcut still asks for
confirmation. In that case the menu omits the Delete chip. The button above the open session still names its shortcut on
hover. Both routes always ask for disposable environments. They ask about the session when its environment can handle
the request, and about the entire environment when Farhelm knows it cannot. The Help item can open the dialog while helm
preferences are unavailable. It shows the chip once its shortcut can open the dialog too.

### Open questions and possible follow-ups

None.

### PRs

- Show equivalent shortcuts in the session and Help menus. [#4427](https://github.com/scode/farhelm/pull/4427/changes)

### Checks run, reused and skipped

- Recorded Linux nextest run `203a851f-e650-4e80-b987-9942191a05d3` ran 126 tests across menu components, Help, keyboard
  shortcuts, session lifecycle menus, environment Delete and the open-session header. All passed. Child command
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^components::tests::menu_item::/) | test(/^help_menu::tests::/) | test(/^shortcuts::tests::/) | test(/^session_list::(menu_tests|environment_delete_tests)::/) | test(/^session_pane::header_tests::/)'`.
- After the review fix to the Help availability rule, recorded Linux nextest run `02097964-82fa-46c6-b46f-d5c67f7732a4`
  ran all 19 Help and keyboard shortcuts tests. All passed. Child command
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^shortcuts::tests::/) | test(/^help_menu::tests::/)'`.
- Recorded Linux `python3 scripts/check-gui-goldens.py --update` run `b62a4413-7688-4e88-81b0-89814a983528` captured 302
  images. Inspected all six new or changed references in light and dark and published them. The existing
  unselected-session menu and all other captures matched their baseline hashes. The first capture, run
  `a3cd3cc1-b59a-48b0-861a-462fa6d89957`, cut off the Delete row in the new scene. The final scene uses enough height to
  show the whole menu and the full Ctrl+Shift+Backspace chip.
- `rustfmt --check --edition 2024 --config skip_children=true` on the changed Rust files,
  `dprint check SPEC.md TODO.md releasing/changelog.d/menu-shortcut-hints.md`,
  `python3 releasing/check-changelog.py format` and `git diff --check HEAD` passed.
  `python -B scripts/check-test-sleeps.py` in the pinned checker environment found 308 delays and zero missing
  rationales.
- Reused the pre-rebase Linux results. The rebase from `874da6fa8cc6` to `ca8185f5a41b` preserved the source patch and
  kept upstream’s independent changes to the feedback-dialog references and TODO. Upstream’s other GUI edits concern
  launcher recents, Restart wording and the terminal’s connecting placeholder, which have no bearing on where or how the
  hints appear. Skipped the full workspace battery and CI dispatch because the targeted checks cover the changed
  behavior. Native macOS screenshots were outside this Linux run. Tests checked the shortcut dialog’s labels against
  what each platform binds and which physical key opens the dialog.

### Review gate

The required independent review confirmed that the menu and shortcut share the Delete request and flagged the Help chip
when preferences were unavailable. It also found a duplicate import. Both findings were addressed, and a test now covers
the Help case. The review gate passed. A fresh reader checked the commit wording and PR text.
