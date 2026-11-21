## What this was about

The Mac app tells the installer what it found when it checked for an update. If an older installation's record has no
ID, the updater puts a dash in that field. The installer CLI mistook the dash-leading value for an option and rejected
the update before any installation changes. Triage chose to fix the installer CLI. It now accepts the value so the
installer can check that the installation still matches what the app saw.

## Things you should know

The test uses the script's separate argument, `--expect -:0:1.2.3`, and verifies that the installer parser gets an
absent ID, generation zero and version 1.2.3. The resolved feedback and triage decision have moved to the archive.

## Open questions and possible follow-ups

Triage confirmed the CLI defect. Whether an ordinary update between published versions reaches this path remains
unverified. A disposable macOS acceptance run can check whether a published predecessor naturally leaves an ID-less
record and whether its desktop updater reaches the new parser.

## Pull request

- [#4385](https://github.com/scode/farhelm/pull/4385/changes) accepts the ID-less expectation, adds the regression test
  and archives the resolved feedback.

## Checks

- `cargo test --locked -p farhelm --bin farhelm tests::install_cli_accepts_an_expectation_without_an_id -- --exact`
  reproduced `UnknownArgument` before the change in `2ce675bd-0528-4e52-ab4c-767d6cb60fc7` and passed after it in
  `68c4fdee-0a35-48fa-8a47-fbee25a62757`.
- `cargo clippy --locked -p farhelm --bin farhelm --tests -- -D warnings`, `cargo fmt --all -- --check` and
  `python -B scripts/check-test-sleeps.py` passed on the parser and test in `c6da3ad4`. The rebases picked up only
  unrelated brick-test and installer symlink test changes and reconciled the archive. The CLI source is byte-for-byte
  identical.
- `dprint check` on the changed triage documents, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` and `git diff --check` passed. The instruction checker reported a pre-existing
  archive citation warning.
- macOS installer acceptance and the wider test suite were skipped. The change and regression are on the CLI parser
  boundary, which the selected test exercises on Linux.

## Review gate

The required single source review found no code or test issues. I revised the changelog to say that the installer
accepts the format the update script sends.
