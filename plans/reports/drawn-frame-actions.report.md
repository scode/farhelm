# Desktop actions use the state the user saw

## What this was about

The desktop could show one session in its open pane and send a header action to another if the selection changed before
dispatch. A session menu could show an ended agent but send Delete with permission to stop a running one if an update
reached the app just before the press. A key could trigger an action in a frame the app had drawn and had yet to show.
On Wayland, dropping files after moving the pointer could target the spot where the drag entered the window.

The maintainer asked for one rule and one reusable mechanism. The desktop takes the target and the permission to stop an
agent or close tabs from the frame it showed. The session header, interrupted-session card, row menu and environment
Stop and suspend share the presentation check. The app and supervisor still check whether the action can run against
current state. The Wayland backend uses the last drag position even when the motion arrived while it was reading the
file list.

## Things you should know

Restart with and Replace with use the warning shown on the launch button. When the launcher asks separately about
starting a YOLO session, the source warning continues to follow the session. Each affirmative answer uses the permission
shown when the user answered. If the warning was absent at that press, an agent that restarts while the host setting
saves or the checkout preview loads is protected from being stopped. Cancel closes the question without launching.

The recorded decision split the launcher from the menu and card, with about 240 changed lines for the launcher and tests
for both waits and Cancel. The final launcher draft changed 253 lines after excluding the shared prompt helper and
bookkeeping. I kept the extra lines to cover a review finding: Enter on a choice needs to apply the first edit and
submit Restart with in one press.

## Open questions and possible follow-ups

With Skip session confirmations enabled, bulk Delete can stop a marked agent that restarted after its row showed it as
ended. The bar uses the displayed marked rows and reads their current running state when pressed. Applying the same
displayed-state rule to bulk Stop and Delete is a possible follow-up.

## PRs

- Use the shown session at the top of the open pane and on the interrupted-session card.
  [#4176](https://github.com/scode/farhelm/pull/4176/changes)
- Use the shown permission in the row menu and for the interrupted-session card's Restart.
  [#4193](https://github.com/scode/farhelm/pull/4193/changes)
- Bind Restart with and Replace with to the displayed warning through both waits.
  [#4204](https://github.com/scode/farhelm/pull/4204/changes)
- Use the displayed environment state for Stop and suspend. [#4181](https://github.com/scode/farhelm/pull/4181/changes)
- Use the release position for Wayland file drops. [#4183](https://github.com/scode/farhelm/pull/4183/changes)

## Checks

All runtime commands ran through `scripts/record-test-run.py`. Nextest used the pinned runner, four slots and zero
retries.

### Run on the final stack

- `cargo nextest run -p farhelm-gui --lib` selected the exact
  `launcher::restart_with_tests::restart_with_an_idle_agent_asks_no_consent` case. It passed in run
  `fa0d73b9-5e27-49c9-83b2-7532c13ca4ed`. Enter applied Default, sent one Restart and left the permission to stop a
  working agent unset.
- The same nextest command selected `launcher::replace_with_tests::` and the exact
  `launcher::checkout_tests::the_guarded_answer_waits_for_the_new_preview_then_launches_once` case. All 19 passed in run
  `47c1099b-4976-47ef-8ae2-8ac85daf4478`, including both delayed answers, Cancel and the checkout hints.
- `python3 -B scripts/gui-e2e-smoke.py` passed 49 of 50 steps in run `62e7bdd1-6753-42ab-a12d-c5163b5cd7c4`. Headless
  Mutter skipped the clipboard step. Header Restart, row-menu Delete, Stop and Replace, and the YOLO answer ran against
  the desktop, helm and supervisor with scripted agents.
- `python3 -B scripts/check-gui-goldens.py` matched all 262 Linux captures to their reference images in run
  `8e4e29ce-3d80-4ad5-9553-77abe8ea6150`.
- `cargo clippy --locked -p farhelm-gui -p farhelm-grid-model -p farhelm-input-mapping --all-targets -- -D warnings`
  passed. So did `cargo fmt --all -- --check`, scoped dprint, `python3 releasing/check-changelog.py format` and the
  isolated `scripts/check-test-sleeps.py` check. The instruction-link checker reported two existing citation warnings
  and zero errors. The Wayland patch reverses cleanly on the vendored tree.

### Reused checks

- `cargo nextest run -p farhelm-gui --lib` selected the header click, Delete shortcut, newly available Replace, submenu,
  Clone and prompt presentation regressions on `d1a33808` plus the retained diff. All seven passed (run
  `e3031cdb-1bc5-4861-af4c-76e92fb8c1d7`). They exercised a click aimed at the old selection and a key dispatched before
  the replacement frame was shown. The header and presentation signal retained those behaviors on rebase.
- `cargo nextest run -p farhelm-gui -p farhelm --lib --test e2e` selected GUI menu, card and header tests plus
  `host_terminals::restart_of_a_host_agent_stops_its_tree_before_a_new_host_starts` on `c24b7166` plus the retained
  diff. All 92 passed (run `d39b96f2-877d-4c64-add0-fe69ae7d9da9`). The real supervisor refused the strict Delete guard
  after a session restarted, and the supervisor regression remains identical.
- `cargo nextest run -p farhelm-gui --lib` selected the environment, lifecycle, menu, header and card cases on
  `2c3ebab9` plus the retained diff. All 181 passed (run `5f8d11db-5a70-4206-bd77-01400f183536`). The exact
  `session_list::environment_tests::stop_and_suspend_keeps_its_verb_and_guards_across_a_push` case passed again from the
  same base after asserting that the source was ended and tabless (run `945aa18d-db3f-443c-a61d-f34dbd8ee1a2`). The
  environment code, test and resulting menu composition preserve those assumptions.
- The vendored Linux command was
  `cargo test --manifest-path vendor/gpui-pre-linux/Cargo.toml --no-default-features --features wayland,test-support --lib`,
  with Cargo path overrides to the vendored `gpui-pre` and `gpui-pre-wgpu` trees. On `655efe2d` plus the retained diff,
  all 14 tests selected by `linux::wayland::client::tests` passed (run `899f6803-116c-46aa-b822-4f4fdb7e8a6c`). Next,
  the exact `drop_uses_the_latest_motion` case passed on `2d789cef` plus the final patch (run
  `7b52033d-c566-4d64-90d7-8381362f24b1`). Both pointer motions preceded the file-list reply. The final patch and Linux
  backend are identical. Reapplying the patches to the published Linux crate reproduced the backend byte for byte.
- `cargo test --locked --manifest-path vendor/gpui-pre/Cargo.toml --features test-support --test native_key_metadata --test driver_record`
  passed on `655efe2d` plus the retained diff (run `694516bb-b0dc-4e8f-9280-bdcaf91754ff`). The rebase added main's
  independent hitbox-occlusion query, leaving native keys and the driver record intact.
- The separate `cargo test --locked --manifest-path` checks for `vendor/vte/Cargo.toml` with `--features ansi`,
  `vendor/alacritty_terminal/Cargo.toml` with `--lib`, and `vendor/pair-tests/Cargo.toml` each passed on `295e7da1`
  (runs `96351d68-06e2-4627-b23e-dee55a148904`, `5a94daf1-5af4-43b8-888f-c82eaddf8b99` and
  `5a4cc572-6ef5-4e75-85bb-b8d5de124ace`). The rebase brought no changes to those source trees.

The earlier `cargo nextest run -p farhelm-gui --lib` launcher, menu, card and prompt selection failed overall on
`1474ba21` plus the retained launcher diff (run `503ee299-6307-4d77-8e17-3036c1d875b0`). The menu, card, prompt, both
delayed Replace with answers and Cancel passed individually. The single failure was the new keyboard fixture's focus
assertion. The final exact rerun confirms that the fixture exposes Default and Enter submits it.

### Skipped checks

A native Wayland drag and macOS GUI runs were outside this local Linux run. The backend test drove the Wayland event
ordering, while the smoke covered desktop actions with scripted agents. The full workspace battery was skipped because
the targeted checks cover the changed paths and the desktop smoke covers their integration.

## Review

Each draft passed its required independent source review. Review tightened the live availability check for the
interrupted card and fixed the launcher Enter behavior. For the launcher's live warning, the shared prompt tests
exercise frame admission and the launcher cases check the displayed permission across both delayed paths and Cancel. The
additional launcher-specific raw-input test proposed during review was left out under the scoped allowance.
