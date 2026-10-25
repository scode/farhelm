## What this was about

A second SSH connection-failure message could suggest a wrongly quoted command for starting a remote supervisor. The
maintainer chose to delete that message if real connections could no longer reach it. Execution confirmed that premise:
production SSH connections always provide process diagnostics and take the newer failure-reporting path before this
fallback.

The obsolete message, its host-kind switch and its four private tests are removed. Connections without process evidence
keep their original failure context. Real SSH failures retain the existing diagnosis and exact-or-descriptive start
advice, including the configured program and state directory. The corresponding TODO entry is removed.

## Things you should know

There is no user-visible change and no changelog fragment. The current production failure path already handles missing
supervisors; this removes unreachable advice rather than adding another command-quoting implementation. The typed
distinction between a clean pre-handshake close and an incomplete protocol frame remains in use.

Existing tests cover the system transport attaching diagnostics, exit 75 reporting a missing supervisor, exact displayed
commands or a descriptive fallback, and partial-frame failures retaining their protocol meaning. No additional test was
needed. A documentation pass removed obsolete annotation references and explains why a failure without process evidence
must not guess a start remedy.

## Open questions and possible follow-ups

None from this plan.

## The PR

- [#2706](https://github.com/scode/farhelm/pull/2706/changes): remove unreachable SSH handshake advice. Open draft;
  landing remains the monitor's job.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-helm --lib -E 'test(ssh::) | test(start_remedy) | test(ssh_natural_exit) | test(ssh_status) | test(transport::tests::) | test(long_remote_paths)'`,
  through the recorder with four slots, zero retries and required pinned tmux: 29 passed, no failures. Run
  `01940a3e-14f1-4e33-8965-1f4c7e187e1f`; 1,285 cases were outside the selection.
- `cargo nextest run -p farhelm-proto --lib -E 'test(handshake_)'`, through the recorder with four slots, zero retries
  and no tmux use: five passed, no failures. Run `b981c8fe-4765-4cbe-802c-ac147f88d78f`; 181 cases were outside the
  selection. Both reports are complete, retained output is untruncated, and neither output contains a runtime `SKIPPED`
  marker.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, `cargo fmt --all -- --check` and `dprint check TODO.md`:
  passed. The isolated test-sleep checker inspected 315 delays with no missing rationale.

The recorded runs used main `36c8d7b0` plus this plan's working diff. Their functional coverage and Clippy result apply
to final PR commit `b91bbeef18128de9dd147d1e9af84cf23ec828f6`: only a comment example changed after the initial
snapshot, and no executable code changed. Main moved only through another plan's claim, whose full diff was inspected;
no rebase or repeated runtime check was needed.

Browser tests, the full workspace battery, release checks and live-install checks were skipped because this deletes a
production-unreachable branch and changes no UI, transport behavior or release machinery. No instruction file or
Markdown heading changed, so the agent-instruction checker was not needed.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort confirmed the deletion premise and preserved
live-path coverage. Its sole finding was a stale comment referring to the removed annotation. That comment was
corrected, and the same reviewer confirmed no remaining findings. The commit and PR wording also passed a fresh cold
read. There were no rejected findings, scope departures or weakened guarantees.

### Landing

Landed as built. When an SSH connection to a host closes before its supervisor answers, the helm has two ways to explain
it: a diagnosis read from how the remote program exited, which names the exact command to start the supervisor, and an
older fallback message whose suggested command was wrongly quoted for some paths. The TODO asked to fix that quoting;
you had decided to delete the fallback instead if real connections could no longer reach it, and the executor confirmed
they cannot, since real SSH connections always carry the exit information the diagnosis needs. The fallback is deleted;
nothing users see changes, and there is no changelog entry.

- #2706: remove dead ssh handshake advice.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained only the Mac installer and uninstall stack, which does not touch how the helm
connects to hosts. A fresh-context reviewer and I read all seven plans together: nothing else still used the deleted
advice, and no other plan changes the connection path. The reviewer found no blocker and nothing that needs your
decision.

#### Fixes made at landing

None.

#### Checks

This round landed seven plans together, in this order: the changed-identity uninstall refusals
(uninstall-identity-display), Escape in the launcher's save-as-template panel (template-panel-escape), the dead SSH
start advice (ssh-close-start-quoting), Cancel on Replace's YOLO question (replace-yolo-cancel-race), reporting failed
settings saves (report-failed-settings-saves), the standard link style (standard-link-style) and matching held values in
Restart with templates (restart-with-template-refusal). This round ran the following on all seven stacked together on
this round's main, before the one landing fix:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason, `dprint check` on the changed Markdown files,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (two warnings, both about
  citations main already had), the docs website build, the workspace debug build, the UI's release web build and
  `scripts/check-desktop-assets.sh` (the desktop app bundles exactly the files the web UI asks for): all passed.
- The Rust tests of the helm, the protocol and the UI,
  `cargo nextest run -p farhelm-helm -p farhelm-proto -p farhelm-ui`, with the pinned test runner and tmux, four tests
  at a time, no retries, run `577def59-f63d-46a6-b0af-add1af5b13a2`: 2283 ran, 2282 passed. The one failure was a known
  open flake in the timeout that bounds a settings read queued behind a stuck save
  (`api::save_order::tests::a_hung_save_releases_waiters_at_their_deadline`), already in TODO.md's Deflake list, in code
  no plan touched.
- The connection tests that dial a real SSH login to this machine,
  `cargo nextest run -p farhelm --test e2e -E 'test(/^host_connection::/)'`, run `a4079f11-dfab-45c6-8df5-7af6c39e4d6b`:
  4 passed, including the missing-supervisor and missing-program diagnoses, with no test skipping itself. My first
  attempt passed the test runner an option the test recorder does not accept, so it never started; the run named here is
  the second attempt.
- The UI's desktop-only Rust tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `cb833a48-311e-418a-b285-062a9186dec6`: 959 passed. The tests of the UI's own JavaScript, including the color-contrast
  checks on the stylesheet (`node --test` in `crates/farhelm-ui/js-tests`), run `52bd1c47-c2ec-4072-a4ba-bc1edcf72675`:
  333 passed.
- `npx playwright test` in `e2e/`, the whole browser suite on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `326bf334-6466-47f5-8819-fa25754dffec`: 2610 passed, 14 skipped by design (WebKit
  cannot be granted clipboard access), 20 failed, while other agents also loaded the machine heavily.
  - The quick-launch test "deletion drains an in-flight quick mark before clearing it" failed on both engines and failed
    4 of 4 when run alone (run `330ec0e2-d00d-48bd-a40c-2d39d0ff5ad8`). report-failed-settings-saves made it out of
    date; it was fixed at landing in that plan.
  - The other 18 each passed when run again alone on this stack. Ten WebKit tests in the spec on popovers moving with
    the session header, and WebKit's trash dialog test, found trash entries that the earlier timed-out WebKit test
    "clone defaults to managed checkouts and searches only untouched names" left on the same helm. That test passed 3 of
    3 (run `d5436e22-8a0e-4f1d-89ad-7873fceaca0f`), the ten popover tests each passed once (run
    `3b92f5cb-bc35-4c36-9028-f8d978029322`) and the trash test passed 3 of 3 (run
    `b071c8e2-40f6-4b97-bb6b-42edf3fbdf13`). Six more passed 3 of 3 each: the host picker's host mark (run
    `8b1296b4-e68b-40a6-8763-d877123b8f53`), delayed host dialogs waiting for Settings (run
    `6c895a71-dfa8-4abc-938b-92eb5f101a45`), a click outside an open row menu (run
    `50b7dbb7-cb91-4b26-a70c-89b8c82a296f`), touch input resuming sounds (run `0a18333b-0342-4626-bd79-a32cae2b2c29`),
    terminal link ownership on Chromium, which had timed out waiting for its terminal to start (run
    `bac0d6e3-45c8-498d-b915-4956f68fb282`), and removing and re-adding a host (run
    `ea9d5b81-d0c3-4eee-8eb9-674569f012a6`). One failure each under that load is not recorded as a flake.
- After the landing fix: the fixed quick-launch test passed 6 of 6, three on each engine (run
  `908b369d-67bb-49b2-999a-af1f554c04dc`), and the test-delay check passed. The fix changes only that test, so the full
  suite was not rerun.
- Reused: none. Skipped: the rest of the Rust workspace, because no plan changes the supervisor or the command-line tool
  (the helm code two plans change is covered above); the doctests (no documentation example changed); the desktop smoke
  test, because the desktop app's own code did not change and the stylesheet it shares with the web UI was covered by
  the browser suite on WebKit, the desktop app's engine family; and the native Mac app (not runnable here).

The report's checks were the helm's SSH unit tests and the protocol's handshake tests. This round also ran the
connection tests that dial a real SSH login, which take the production path the plan says makes the deleted advice
unreachable, and they still report a missing supervisor with its start command. Nothing in the executor's report became
untrue at landing.
