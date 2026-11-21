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

### Landing

#### What landed with it

Two other small plans landed right after this one in the same round: the launcher's folder warning fix and a test fix
for the spare checkout builder. Since this plan's work started, main had gained only a change to the developer test
tools (the test recorder and the deflake sweep), which touches none of this plan's code. An independent reviewer and I
read the three plans' changes against each other. The only files they share are the triage records for review feedback,
and each plan's edit to those survived. The landing made no fixes of its own to this plan.

The one risk in letting the installer's expectation value start with a dash is that it could swallow the option after
it. That cannot happen with the installer script: it passes `--expect` only when it has a value, always as one quoted
word, and the option takes exactly one value.

#### Checks

This round landed three small plans together: installer-expect-hyphen-value (the installer's command line),
launcher-folder-refusal-order (the desktop launcher's folder warning) and spare-builder-fifo-release (one test). The
landing ran the affected tests, the installer's shell test suite and the main lint (clippy) on a separate arm64 Linux
test machine, and the small checks locally on x86_64, on all three plans stacked together on main:

- `cargo nextest run -p farhelm -p farhelm-gui -E 'binary(farhelm) | binary(spare_builder) | test(/folder_check::/)'`,
  on the test machine (recorder run `b91fdebc-623b-4755-9043-a5871f8e60f8`): all 151 selected tests passed, including
  each plan's changed or new test.
- `cargo build -p farhelm && bash scripts/test-install-sh.sh --farhelm target/debug/farhelm` (the Mac installer's shell
  tests, driven on Linux), on the test machine, not through the recorder, which the landing's script ran it outside of
  by oversight: all 547 checks passed.
- `cargo clippy --all-targets -- -D warnings`, on the test machine: passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format` (formatting, test and changelog lints), locally: passed.

Skipped: the workspace suite, the desktop app's other tests and the end-to-end smoke test, since each change is confined
to the code its tests cover; and Mac runs, since the installer's shell tests drive the Mac installer's script (they do
not settle installer-expect-hyphen-value's open question about real updates, which its note repeats).

#### Things you should know that the report does not say

Before the fix, updating a Farhelm installation recorded without an installation ID failed with "Farhelm X is older than
this installer and cannot be installed by it; nothing was changed." The new test checks the expectation as the last
option, while the installer script can put one more option after it. Reading the code shows that order parses the same
way, but no test checks it.

#### What you should still do

The report's open question still stands: whether a real update from a published release ever reaches this case. Its
suggested check on a Mac is still worth doing if you want that settled.
