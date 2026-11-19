## What this was about

- Desktop WebSocket diagnostics could include the credential the app uses to connect to the helm when trace logging was
  enabled. The app now excludes those diagnostics at every log level. Other diagnostics still follow the configured log
  level.
- Picking a Codex login file in Settings → Credentials showed its contents on screen before Save. The agreed design
  shows the filename, validation status and Clear. Clear returns to an empty paste box, where pasted text stays visible
  for editing. Save sends the selected file's contents when a file is selected.

## Things you should know

- Validation checks that the file contains a JSON object within the helm's 64 KiB limit. An unreadable replacement
  choice leaves the current draft in place.
- An edit to the paste box takes precedence over a pending file choice, even when it restores the earlier text. Clear
  and changing agents discard the unsaved selection. Creating or replacing a saved login requires Save.
- Inspection found the helm's WebSocket upgrade and authentication paths already kept the credential out of server logs.

## Open questions and possible follow-ups

Earlier logs from affected desktop builds can still contain the connection credential if handshake tracing was enabled.
Treat credentials in shared trace logs as exposed.

## The PRs

- Desktop connection logging [#4151](https://github.com/scode/farhelm/pull/4151/changes)
- Codex login file selection [#4153](https://github.com/scode/farhelm/pull/4153/changes)

## Checks

- `cargo nextest run -p farhelm-desktop --lib -E 'test(desktop_filter_excludes_handshake_diagnostics)'` passed 1 of 1,
  recorder `ce88cca1-1064-4f5e-8284-8a674141686d`. It used synthetic metadata with default logging, global trace logging
  and an explicit trace setting for the protected logger. The logging code and test match `4c556b79705e` (#4151). The
  earlier recorder `b557038f-5230-46f9-b490-c30f138949f3` was interrupted during compilation.
- `cargo nextest run -p farhelm-gui --lib -E 'test(credential_tests::) | test(/^drafts::/)'` passed 24 of 24, recorder
  `3030ec86-3790-4827-bdbd-8b6e44e71db5` on `58056c8b6673` (#4153). The chooser exercised a fictional login and a fake
  helm to verify what the editor draws and sends. Recorder `ae5cd334-06f2-4af3-a19c-d9896d60498b` caught the test's
  incorrectly typed availability assertion during compilation. The passing run includes the correction.
- `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` passed. The equivalent desktop run with
  `--features agent` and `cargo clippy -p farhelm-gui --all-targets -- -D warnings` also passed.
- `cargo fmt --all -- --check`, `dprint check` on the touched files, `git diff --check`,
  `python3 releasing/check-changelog.py format`, and `python -B scripts/check-test-sleeps.py` passed.
  `python3 scripts/check-agents-md.py` exited successfully with one warning for a heading citation in an older archive
  entry. The warning predates these changes and the archive retains its historical entries.
- The full release suite was skipped. The targeted tests exercise the changed editor and logging behavior, including the
  editor's shared draft code. Main advanced through plan bookkeeping since the claim, so the checks still apply.

## Review

One independent reviewer checked each PR. The logging test checks that the sensitive target is suppressed at a level
that would otherwise be visible under the default settings. The editor test now checks the Clear button's availability
and verifies that a pending file choice cannot replace a paste that restores the original text. The targeted checks
passed with those corrections.
