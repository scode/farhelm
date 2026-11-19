# Show the version in Settings → App

## What this was about

A Farhelm user looked in Settings for the version they were running and couldn't find it. The chosen layout puts it in
Settings → App above **Install updates automatically**, with **Check for updates** beside it.

## Things you should know

- You'll see App in desktop installations with built-in updates. Its button uses the same check as the sidebar version,
  the **?** menu and the local host's **Update** action. It installs a newer release if one is available.
- While a check or install is in progress, the button reads **Checking…** or **Installing…**. It keeps keyboard focus
  and ignores another press. Look beside the version at the top of the sidebar for the result.
- When Settings opens directly to App, focus starts on **Install updates automatically**. The Settings and Update and
  uninstall guides describe the new row and check.

## Open questions and possible follow-ups

None.

## PR

- Running version and **Check for updates** in Settings → App.
  [#4233](https://github.com/scode/farhelm/pull/4233/changes)

## Checks

### Run

- The rebase picked up a shared click-handling change in #4199, so the Settings and sidebar update-control tests ran
  again. The first `cargo nextest run -p farhelm-gui --lib` attempt stopped during compilation and recorded no tests
  (recorder `f95de281-653f-48d0-a774-e6bf3b7cb0e8`). A retry of the same selection passed all 83 tests on draft commit
  `4ef649a1` (recorder `793893cd-bcc1-49ac-8b3a-04301b867fe5`). The new test checks that a click starts one check and
  that the open dialog updates when progress changes. Pressing again during progress leaves automatic updates as set and
  starts no new check. Finishing makes the button work again.
- `python3 -B scripts/check-gui-goldens.py --update` captured 262 Linux screenshots. Only the two Settings → App images
  changed, and both themes were inspected. Recorder `618b51f9-a72d-420e-85c3-c83215ce7a21`. The ordinary
  `python3 -B scripts/check-gui-goldens.py` matched all 262 references on the draft head. Recorder
  `36dba28c-90b3-4f0f-923c-9c6ad02afb54`.
- `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`, changed-file `dprint check`, `git diff --check`,
  `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format` and
  `python -B scripts/check-test-sleeps.py` passed after the rebase.

### Reused

- `bun install --frozen-lockfile` and `bun run build` validated the guides and the site's internal links. The site
  inputs from `5178515a`, the tested tree before rebase, are identical at the draft head.

### Skipped

- The screenshot and interaction tests ran locally on Linux with a fake desktop updater. A native macOS update was
  outside that test setup. The broader desktop smoke and workspace suite would cover unrelated surfaces.

## Review

The independent source review caught a keyboard focus issue during progress, a test that refreshed the dialog itself,
and two documentation omissions. The fixes keep focus on the busy button, verify that Settings updates on its own,
explain when App appears, and list the sidebar version in the switch help. The reviewer verified the fixes. The commit
and PR text also passed a separate cold read.
