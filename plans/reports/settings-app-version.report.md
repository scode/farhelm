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

### Landing

#### What landed with it

It landed in a round of two plans, gui-ux-enter-and-focus and settings-app-version. Since its base, main gained the
previous round's plans, among them drawn-frame-actions (session actions and the launcher act only on what the user was
shown) and one-helm-per-supervisor, and a repair of the triage archive, which an earlier landing had garbled. It edits a
different part of SPEC.md and different code from the other plan; the reviewer found no interaction.

At your request this landing was reviewed with extra skepticism, by me and by an independent reviewer, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The new Version row in Settings
→ App is proportionate: it uses the same wording as the version shown in the sidebar, adds no new update logic, keeps
keyboard focus on Check for updates while that button shows Checking…, and its test checks what is actually drawn. In
the reference picture the row reads well: "Version" on the left, the version and a Check for updates button on the
right, matching the switch rows.

#### Fixes made while landing

- **The help under the automatic-updates switch, when off, lost what "off" means.** It read "Check for updates here,
  from the version in the sidebar, …", which dropped the earlier sentence's point that Farhelm installs a new release
  only when you check, and "here" pointed at nothing beside the line. It now reads "Farhelm installs a new release only
  when you check for updates (above, from the version in the sidebar, the ? menu or this machine's update item) or run
  the installer yourself." ("This machine's update item" is Update on this machine's row in Hosts, the plan's wording.)
  Its test follows.
- **A SPEC.md sentence came before what it referred to.** "The Settings button shows Checking… or Installing… while that
  progress is reported" now follows the sentences that introduce that progress.

#### Things you should know that the report does not say

- The version's hover help repeats the help line printed under it.
- The note above the section, "This one belongs to this Farhelm app on this Mac.", now sits directly above the Version
  row, so "This one" seems to refer to the version rather than the settings below it. The plan kept the note on purpose.

#### Checks

The two plans change the desktop app's GUI crate, the specs, two docs pages and the triage archive. The landing ran the
lints, the GUI crate's whole suite, the app's end-to-end smoke, the reference screenshots (goldens) and the website
build. Builds ran locally; the GUI suite and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, and
  `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings` once without and once with `--features agent`:
  passed.
- `cargo nextest run -p farhelm-gui`, from a locally built archive, split in three across boats: 1097, 1036 and 1030
  passed, 3163 in all, with no runtime skips (`2b47e057-bb12-44ed-9b4e-0eb107bd7f93`,
  `33b0a8db-29ec-4348-81d9-7fe3eb4f3c59`, `da09c3c6-1297-47b9-839b-2960e7aaf280`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed and the clipboard
  paste step was skipped, as always on Linux (`0d719710-e8fc-4f7d-a99e-4a98147bf820`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`f2fafc15-3eab-4786-8d26-dee83ba73b58`).
- The docs website build (`bun install --frozen-lockfile && bun run build` in `website/`): passed.

On the fixed tree (a held Enter's repeat, the Settings help line and its test, comments and SPEC wording), rebased onto
main with the archive repair:

- `cargo fmt --all -- --check`, `dprint check` and `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`:
  passed.
- The focus, Settings, session pane, marked-rows, bulk action, launcher launch and quick launch tests, locally: 607
  passed (`5edad3a8-c790-4866-86f9-51d2b58187ca`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`0b4c779b-c1c6-4b49-af6e-c450b6a5bc5e`).

The boat suites, the smoke and the website build were reused for the fixed tree: after them only the changes above,
which the focused runs cover, and the archive repair, which only restores text. The sleep check and the clippy run with
`--features agent` were reused too: the changed tests only changed assertions and added no delays, and no feature-gated
code changed.

Skipped:

- The rest of the Rust workspace: the plans stay in the GUI crate.
- The Mac-only checks: no Mac-specific code changed.
