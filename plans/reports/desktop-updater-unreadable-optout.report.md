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
