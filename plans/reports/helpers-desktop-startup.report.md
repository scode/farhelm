# Shared desktop startup and updater helpers

## What this was about

Desktop startup and the updater kept separate copies of rules that could drift when changed separately. This refactor
preserves behavior and shares the wait before reopening an app, the supervisor discovery and saved-window size checks,
the stable-release version check, the updater's state-file access, and the wording used for build labels and reinstall
advice. The file-backed updater tests now use the app's own state-file callback setup.

## Things you should know

The missing-supervisor recovery path still waits until the old app exits and supplies its installation marker to the
opener. Restart to update keeps a wait of about a minute and gives up if the old app is still running. Review caught an
inherited-shell-variable difference in the first extraction. A comparison with stand-in openers confirmed that the
corrected version passes the same argument values and exported shell variables as the old scripts.

The final draft combines the two planned text extractions so the peer labels and reinstall tooltip share one screenshot
comparison. The existing screenshot references are the comparison target.

## Open questions and possible follow-ups

None.

## Pull requests

- [#4581](https://github.com/scode/farhelm/pull/4581/changes) shares the wait-and-reopen helper.
- [#4585](https://github.com/scode/farhelm/pull/4585/changes) shares the supervisor discovery and saved-window checks.
- [#4586](https://github.com/scode/farhelm/pull/4586/changes) shares the stable release version rule.
- [#4587](https://github.com/scode/farhelm/pull/4587/changes) shares the updater state-file callbacks.
- [#4591](https://github.com/scode/farhelm/pull/4591/changes) shares build-label and reinstall wording with the places
  that display or test it.

## Checks run, reused and skipped

- Ran `cargo nextest run -p farhelm-desktop` through the recorder at each of the first four draft steps. The reopen,
  startup checks, stable versions and state-file steps passed 184, 184, 185 and 185 cases. Runs
  `4ad8b296-5341-4f5e-94d8-27a6805b6145`, `5d5d994b-c542-45c5-8a9d-3f5da9d4c309`, `c029e487-63fc-4c4d-9ca8-0fe616565090`
  and `87b2a1ca-6ad6-4ad4-a47a-160b457e417f`. Run `cc5be8ef-8815-479c-b9b8-905bcb2007b1` compared the old and new opener
  commands using stand-ins with preexisting exported variables.
- On `081b26e8b53c`, recorded nextest ran the full desktop package and the shared text, update readout and session
  details selections from `farhelm-proto` and `farhelm-gui`. All 237 selected cases passed, with no runtime skips. Run
  `7f3aa00c-6ee7-4dcf-9733-2d710b8eb514`.
- `python3 scripts/check-gui-goldens.py` compared the full Linux catalog, including light and dark appearances, against
  the existing references. All four render processes matched. Run `9a27cdab-afcf-410b-b00d-853f082923e0`.
- Strict `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and its `--features agent` variant passed. Shell
  syntax and ShellCheck for the shared script, Rust formatting, the test-delay checker, scoped Markdown formatting and
  whitespace checks passed.
- Reused the runtime, screenshot and Clippy results after rebasing onto `4942ccfdaa8c`. Every draft had an empty
  interdiff. Main changed the launcher, trust prompts, database maintenance and test worker. The supervisor readiness
  replies and control flow, the installed opener commands, and the affected details and readout fixtures are intact.
  Main's new screenshot teardown callback runs after capture and is set only on a launcher fixture. Formatting and
  whitespace passed again.
- Skipped the full workspace suite. The tests above cover the changed paths. Mac process launches were checked with
  stand-ins on Linux. Native LaunchServices and an installed Mac update were outside this run.

## Review gate

Each draft passed an independent source review and cold wording check. The first draft preserved the bounded updater's
inherited shell-variable behavior and the unbounded recovery's positional argument handling after review. The startup
draft clarified version-skew replies and the saved frame size threshold. The updater still decides why a reinstall is
needed. The verification and minimum-version notices each keep their explanation and share the same reinstall command.

### Landing

#### What landed with it

The plan's five PRs (#4581, #4585, #4586, #4587, #4591) landed second in this round, after recipe credentials in the
launch environment, with no landing fix. Neither that plan nor what main gained since this plan's work was written
touches the desktop startup and updater code this plan changed, apart from separate TODO.md edits, and the rebase had no
conflicts.

An independent reviewer checked each helper move by hand and found that all five keep behavior exactly the same,
including error paths and the updater's state file on disk: the reopen and Restart-to-update paths run the same commands
with the same limits, the supervisor discovery check is the same expression, saved window sizes come out the same, the
new stable-release check asks the opposite question from the old one and every caller was flipped to match, so each
behaves the same, and build labels and the reinstall tooltip produce the same text.

#### Fixes made while landing

None.

#### Checks

This round landed two plans stacked in this order on main: recipe credentials in the launch environment (protocol 85)
and the desktop startup helpers. The checks ran on the combined result; the worker runs were on a separate arm64 Linux
test worker, the rest locally on x86_64.

On both plans as delivered, stacked onto main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, with four nextest slots, no retries and four build jobs, on
  the test worker (recorder run `0533a2d1-482b-4855-8997-6048700f9783`): all 8318 passed, the real Podman environment
  test among them.
- The four clippy runs (`--all-targets`, `-p farhelm --bins`, and the desktop app with and without `--features agent`),
  on the test worker, not through the recorder: passed.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `e684d4f8-2f9e-4959-a29b-b54a9d260b29`): all
  185 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `f73f7f73-6517-4404-ac56-ac4d02ee04f8`): 49 of
  its 50 steps passed and the paste step was skipped, as on every Linux run.
- `python3 -B scripts/gui-lifecycle.py`, the Linux app lifecycle test (startup, quitting, the saved window frame), on
  the test worker (recorder run `198b3e2a-a687-4689-8fd2-9d0bba408994`): every step passed.
- `python3 scripts/check-gui-goldens.py`, every scene, locally (recorder run `993f1622-0462-4776-95eb-e35602d8230d`):
  passed.
- The docs website build, `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its isolated
  environment), `dprint check` and `python3 releasing/check-changelog.py format`, locally: passed.

After the recipe credentials' landing fix (#4600), which changes only Markdown: `dprint check`, the website build and
`python3 releasing/check-changelog.py format`, locally: passed. The runs above still apply.

Skipped: a launch on a real Tensorlake sandbox with Claude recipe credentials, which spends cloud resources (the real
Podman environment test in the workspace run delivers the credentials to a container); macOS, since no Mac was part of
this landing; CI, which runs only on demand.

#### Things you should know that the report does not say

- The shared wait-and-reopen script is harder to read than the two it replaced: it takes its arguments in two orders and
  rewrites one into the other, to keep every detail of both old scripts, including one no caller depends on (whether the
  program it opens inherits the script's own variable names). The plan's own reviewer asked for that; accepting one
  argument order would be a simpler possible follow-up.
- The reinstall sentence ("Reinstall it by running this in a terminal: …") is now built in two places, the desktop
  updater and the GUI's minimum-version notice, so the rule that the command comes last could drift between them.
- Three code comments lost their reasons in the moves, and restoring them is a small possible follow-up: why Restart to
  update gives up instead of opening the app when the old one will not quit, why the reinstall command needs no signing
  key (it fetches the installer over TLS), and why the build identity shown in session details must not be escaped a
  second time.
