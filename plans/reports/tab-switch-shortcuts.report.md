## What this was about

With several terminal tabs open, you previously had to use the tab strip to move between them. On macOS, `Cmd+Shift+[`
now moves to the previous tab and `Cmd+Shift+]` moves to the next. Linux uses the corresponding `Ctrl+Shift` chords, as
agreed in the plan. The shortcuts wrap at either end, repeat while held, and focus the chosen terminal. The strip
scrolls to show its tab.

## Things you should know

- Dialogs and menus keep keyboard focus. The shortcuts also leave an active input-method composition alone and take no
  action when the main terminal is the only available tab.
- The source review found that a session can list more shell tabs than the app can display as terminals. The shortcut
  skips any tab without a terminal it can focus, as well as a tab whose close has already been confirmed.
- The Keyboard shortcuts dialog and the docs pages for keyboard shortcuts and working in a session describe the new
  keys.

## Open questions and possible follow-ups

None raised by this plan.

## PRs

- Add keyboard shortcuts to switch terminal tabs. [#4983](https://github.com/scode/farhelm/pull/4983/changes)

## Checks run, reused and skipped

- All 90 selected GUI tests passed on `914895ed`, run `d25d981f-6b10-47c9-87d3-61eec0831acb`. The selection covered
  physical key matching and held repeat, both platform shortcut tables, wraparound, focus and scrolling, confirmed
  closes, the tab attachment limit, and dialog and input-method guards.
  `cargo nextest run --locked -p farhelm-gui --lib -E 'test(/^actions_tests::/) | test(/^shortcuts::tests::/) | test(/^terminal::set::tests::/)'`
- The docs passed `bun install --frozen-lockfile` and `bun run build` on `5b1e2e1a`. The website inputs are identical on
  `914895ed`. Both pages rendered and all internal links passed.
- The selected light and dark `keyboard-shortcuts-dialog` screenshots matched their references on `914895ed` in run
  `7302ad67-14e8-4ad0-8b8d-2e338e064ffd`. The starting viewport ends above the new rows, which are in the scrollable
  body. The platform table tests verify the new row names and chords. The recorder ran
  `python3 scripts/check-gui-goldens.py --scene keyboard-shortcuts-dialog --jobs 2 -j 4`.
- Focused Clippy passed on `914895ed` with `cargo clippy --locked -p farhelm-gui --lib --tests -- -D warnings`.
  `cargo fmt --all -- --check`, `dprint check` on the changed docs, specs, TODO and fragment,
  `python3 releasing/check-changelog.py format`, `git diff --check` and `python -B scripts/check-test-sleeps.py` also
  passed.
- Main through `b625e36b` changed the cache sweep and Tensorlake setup sections of the implementation spec. The
  tab-switch behavior and the shortcut table were unaffected, so the recorded checks still apply.
- This run used the local Linux GUI harness. It covered simulated macOS key identities and the macOS shortcut table.
  Testing keys in a native macOS window would require a Mac.

## Review gate

Independent gpt-6.2-sol source review at high effort approved the corrected revision. Its findings led to the
attachment-limit guard and regression, and to the expected rows in both platform tables. The screenshots confirmed that
the visible part of the dialog still matches in both appearances. The new rows are below it and covered by the platform
table tests.
