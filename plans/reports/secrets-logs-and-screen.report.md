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

### Landing

#### What landed with it

It landed in a round of nine plans: session-group-follow-ups, model-families, session-limit-notice,
reserved-resource-names, secrets-logs-and-screen, checkout-settings-stale-save, concealed-link-target, triage-misc-nov18
and triage-discards-nov18. Since its base, main gained the previous round's plans (submenus that open on hover, Clone
keeping invisible characters, the retired old-terminal announcement, quick switcher screenshot folders, the update
survival wording) and docs screenshot tooling. checkout-settings-stale-save, in this round, also changed the shared
text-field draft both plans use: this plan adds a count of the user's edits, the other a stored value it can reload. The
two conflicted when stacked and I merged them by keeping both; each plan uses only its own addition, and the reviewers
confirmed the merge.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. Neither change is bigger than
needed. The fix silences one of the WebSocket library's log sources; a reviewer confirmed in the library's source that
it is the only one that logs the connection request, which carries the credential.

#### Fixes made while landing

- **A code comment was deleted.** The desktop app's log filter had a comment explaining its default (debug builds log at
  debug level, release builds at info, `RUST_LOG` overrides both). It is back, with a note that the handshake log is
  excluded at every level.

#### Things you should know that the report does not say

- The test checks the log filter's setting; it does not send a real handshake log line through and confirm it is
  suppressed. The two are almost certainly the same, but the test proves slightly less than its description says.

#### Checks

This round landed nine plans together, so these checks cover all nine on one tree: the nine stacks one after another on
main. Between them the plans change the desktop app's GUI crates, the helm and its client, the shared protocol, the
supervisor, the desktop process, SPEC.md, SPEC_impl.md and the triage records. The landing ran the lints, the whole Rust
workspace suite, the desktop process's own tests, the app's end-to-end smoke and the reference screenshots (goldens).
Builds ran locally; the workspace suite and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, `python3 -B scripts/test-gui-e2e-smoke.py`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`, once without and once with `--features agent`: passed.
- `dprint check`: failed on `TRIAGE_OUTCOMES_ARCHIVE.md`, which triage-misc-nov18 wrote unformatted. Fixed.
- `cargo nextest run --workspace --exclude farhelm-desktop`, from a locally built archive, split in three across boats:
  2744, 2653 and 2625 tests, 8022 in all (`1ffb9472-9f12-4890-b0d5-032dd87718d0`,
  `7c3ef63b-2e2b-4903-acff-22cbe9409601`, `7c539d75-d409-48c7-901b-391169215c11`). One failed: the macro
  compile-contract test, which builds a small crate offline from this machine's cargo cache, which a boat does not have,
  so it fails there every time; it passed locally (below). No runtime skips.
- `cargo nextest run -p farhelm-desktop`, locally: 176 passed (`59017e5e-7626-4dca-9c45-0b4972a89aca`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed, the new step that
  drags an unpinned session into a group included, and the clipboard paste step was skipped, as always on Linux
  (`b1164f8b-3d55-4cac-9fb1-a76e9a8da60c`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`da0dca91-646f-483a-a436-b10b43ffafe2`), the new groups
  pictures included.

On the fixed tree (comments, SPEC.md, SPEC_impl.md, changelog fragments, the triage archive's formatting, and one
notice's wording changed):

- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`: passed.
- The bell notices' tests in the GUI and the helm, locally: 26 passed (`80fb276d-1ba3-4b09-a84a-370aef2b9605`).
- `cargo nextest run -p farhelm-testtrace-macros`, locally: 2 passed, the compile-contract test included
  (`a8a9bf1f-6dc9-4e24-8831-e5d4b7f5dbc0`).

The boat suites, the smoke and the goldens were reused for the fixed tree: after them only comments, documents,
changelog fragments, formatting and one notice's wording changed, which the focused runs above cover.

Skipped:

- The Linux app lifecycle: startup and quit are untouched.
- The Mac-only checks: no Mac-specific code changed.
