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
