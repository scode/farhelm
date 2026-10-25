## What this was about

When uninstalling Farhelm from a remote host was refused because its identity changed, the refusal quoted identities
that could look identical: the dialog collapses spaces. The maintainer chose to remove those values and point to the
host list, which already makes spaces visible.

Both affected messages now follow that choice. The existing mismatch message names the host list's recorded and reported
identities and retains adopt, correcting the destination, and Remove. A different identity found during uninstall's
fresh check retains the exact Retry instruction, letting the host list explain the resulting state without promising
that both identities will appear.

## Things you should know

The identity checks and uninstall protections are unchanged. Tests cover doubled-space identities, missing identities,
the preserved recovery advice and refusal before host changes. The TODO entry is removed and a fixed changelog fragment
is included. No broader identity-formatting helper or dialog change was needed.

## Open questions and possible follow-ups

The separate setup/update refusal still quotes identities in the same diagnostic style. That was explicitly outside this
uninstall-only plan and remains a possible follow-up.

## The PRs

- [#2708: clarify changed-identity uninstall refusals](https://github.com/scode/farhelm/pull/2708/changes), open draft.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::uninstall_refusal::) | test(provisioning::tests::uninstall_)'`,
  through the recorder with pinned nextest, required pinned tmux, four slots and zero retries: 29 passed, no failures or
  flaky outcomes; 1,289 cases were outside the selection. Run `cb3a3f26-395e-4b6b-af75-d03406db6083` has complete source
  capture, report and output. Retained output contains no runtime `SKIPPED` marker.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`, `dprint check TODO.md releasing/changelog.d/uninstall-identity-display.md`, and
  `python3 releasing/check-changelog.py format`: passed.
- The isolated interpreter's `python -B scripts/check-test-sleeps.py`: 315 delays inspected, zero missing rationale.

The runtime run covered main `876d7073` plus this plan's complete working diff. Those checks and the review apply to
final PR commit `35c5bb67654d461b1128c80f1091385c378e04eb`: the rebased diff is byte-identical, and the inspected
upstream changes through `2f832408` contain only queue bookkeeping and other plans' reports. No earlier runtime result
was reused; these current checks were retained across that bookkeeping-only rebase without repeating them.

Browser tests were skipped as agreed because the dialog renders the supplied string and its layout did not change. The
full workspace battery, desktop smoke and installer/release tests add no relevant evidence for these two messages and
their removed parameters. No instruction file, heading or file location changed, so the agent-instruction checker was
not needed.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no actionable issues in correctness, design,
idiomatic code or tests. It received the full review charter, acceptance criteria and verbatim test-authoring checklist.
The commit and PR wording passed a separate cold read. A documentation pass covered every touched file. There were no
rejected findings, scope departures or weakened guarantees.

### Landing

Landed as built. When uninstalling Farhelm from a remote host is refused because the install at its SSH destination is
not the one the host entry has on record, the refusal used to quote both install identities, and the dialog collapses
runs of spaces, so two identities differing only in spacing could look the same. The refusal now leaves the identities
out and sends the user to the host list, which already shows spaces as they are.

- #2708: clarify changed-identity uninstall refusals.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained the Mac installer and uninstall stack (#2058, #2059, #2061, #2065, #2066, #2067,
#2068). It changes how Farhelm installs, updates and uninstalls itself on the Mac it runs on, not the helm's refusals
for uninstalling from a remote host, which this plan changes; nothing in that stack, and no test or docs page, quoted
the old wording. A fresh-context reviewer and I read all seven plans against each other and against what main gained,
and none of the other six touches remote uninstall. The reviewer found no blocker and nothing that needs your decision.

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

The report's checks were the helm's uninstall refusal unit tests; this round ran the helm's whole package with the other
six plans. Nothing in the executor's report became untrue at landing.
