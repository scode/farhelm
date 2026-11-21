# Keep automatic app updates off when settings cannot be read

## What this was about

If the desktop app could no longer load its settings after startup, it could install an update even after you had turned
automatic updates off. The maintainer decided a failed read should count as off.

Farhelm now pauses automatic checks and installs when the settings file cannot be read or parsed. A missing file or
missing setting still defaults to on so fresh installations and older settings files keep the existing default.

## Things you should know

The switch shows off while the file is unreadable, and an attempt to save a choice still reports a save failure. You can
still check for and install updates on demand. Once a scheduled check is due, Farhelm retries the settings read on the
next wake. The installation guide describes what happens if the running app loses access to its settings.

## Open questions and possible follow-ups

None.

## Pull requests

- The desktop updater keeps automatic updates off when it cannot read the setting.
  [#4407](https://github.com/scode/farhelm/pull/4407/changes)

## Checks

- All 91 focused Linux tests passed with `cargo nextest run -p farhelm-desktop --lib -E 'test(/^(updater|state)::/)'` in
  recorded run `bcb962eb-b731-47d9-b1a5-a82103ad3413` on `4638a533` plus the final code diff. The archive link added
  afterwards changes only bookkeeping. The tests include failed reads and writes, on-demand checks, and the final
  install gate.
- Rust and Markdown formatting, whitespace, changelog, test-delay and instruction-link checks passed. The
  instruction-link checker reported one existing citation warning.
- The focused tests use temporary settings and a fake installer. I skipped native macOS installation and the full
  workspace suite because the change is in the shared settings read and its existing updater gates.

## Review

Independent source review verified the fallback and the existing manual-update gate. It identified a test that used a
read error as a stand-in for a failed save. The revised fixture keeps the default readable and blocks only the write.
The reviewer verified the revision and the installation guide update. Commit wording passed a cold read.

### Landing

#### What landed with it

Nothing else landed in this round, and main had gained only plan bookkeeping since this plan's work started. The landing
made no fixes of its own to this plan.

#### How the change behaves

An independent reviewer and I read the change. A missing settings file, or one without the setting, still means
automatic updates are on. An empty or invalid file, a permission problem, or a folder where the file should be now means
off. An update you start yourself never consults the setting, so a read problem cannot cancel it. The app saves the file
by replacing it whole, so its own saves cannot cause a failed read; only a disk problem or an outside change can. The
app already refuses to start when the file cannot be read (this change did not alter that), so the new behavior matters
only when the file breaks while the app is running.

#### Checks

The plan changes only the desktop app's updater and its tests, plus SPEC_impl.md, the install guide and a changelog
fragment. The desktop app sits outside the workspace's default build, so the landing ran its tests and its own lint runs
(clippy) on a separate arm64 Linux test machine, and the small checks locally on x86_64:

- `cargo nextest run -p farhelm-desktop`, on the test machine (recorder run `3cb9ad0b-4b30-43d7-a5b8-ebc7e31fc724`): all
  180 tests passed.
- `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`, the same with `--features agent`, and
  `cargo clippy --all-targets -- -D warnings`, on the test machine, not through the recorder: passed. The executor's
  report does not mention running clippy.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format` (formatting, test and changelog lints), locally: passed.

Skipped: the workspace suite and the end-to-end smoke test, since only the desktop app's updater changed and its tests
cover it; and Mac runs, since the code that reads the setting is the same on Mac and Linux.

#### Things you should know that the report does not say

The report says the switch shows off while the file is unreadable. It does not: the **install updates automatically**
tickbox in the settings dialog keeps showing what it read at launch, usually on, so while the file is unreadable it can
say on with automatic updates paused. Changing it then reports that the setting could not be saved. While the file stays
unreadable, the app also writes a warning about it to its log every minute.

#### What you should still do

Nothing is required. A possible follow-up would make the tickbox show off, or say that the setting could not be read,
while the file is unreadable, as the report assumed. The gap is small because the file rarely breaks while the app runs.
