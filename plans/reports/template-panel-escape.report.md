## What this was about

After a refused "save as template" attempt, Escape could close the whole session launcher and discard the setup instead
of closing only the template panel. The maintainer chose to fix both Escape routing and keyboard focus after refusal,
keeping the change small.

## Things you should know

Escape with focus outside the launcher now closes the template panel first; a second Escape can close the launcher.
While a save is pending, Escape still cannot cancel it. After refusal, the template name regains lost focus. A usable
control the user chose keeps focus, including the Launch button when unavailable and live approval answers outside the
launcher. New, Clone and Restart with share the panel.

The TODO entry is removed and a fixed changelog fragment is included. There is no new focus-tracking state or change to
the shared modal isolation behavior.

## Open questions and possible follow-ups

None identified within this plan's scope.

## The PRs

- [#2709: preserve launcher drafts on template panel escape](https://github.com/scode/farhelm/pull/2709/changes), open
  draft.

## Checks run, reused and skipped

Evidence covers 36 distinct browser cases, 18 per engine, using Chromium and WebKit, a three-worker cap with owned
stacks, zero retries and required pinned tmux. These commands ran through the recorder:

- `npx playwright test 'templates\.spec\.ts|restart-with\.spec\.ts|modal-trap\.spec\.ts|quick-launch\.spec\.ts|clone\.spec\.ts' -g 'save as template|launcher template save refuses|restart with keeps focus inside the dialog over a live terminal while pending and after refusal|restart with pulls stray focus back and still cancels on Escape|ordinary New isolates|a clone transfer retires|launch sequence cancellation'`:
  run `6fbbf1f4-f9d7-44e9-99f8-2d6db636d0cb` had 34 passes and two timeouts, one per engine, in the synthetic
  approval-focus fixture. Both traces showed the modal backdrop blocking its click before the refusal was released. The
  fixture now establishes native focus directly. Its cleanup also preserves primary failures.
- `npx playwright test 'templates\.spec\.ts' -g 'save as template refusal respects approval focus'`: corrected fixture
  passed both cases in run `2f97d995-856d-480f-a0fb-7a86872c79a3`.
- `npx playwright test 'templates\.spec\.ts' -g 'save as template refusal respects (lost|search|launch) focus'`: all six
  cases passed in run `675dae1a-5e12-49ad-bada-f2f4a9203070`, covering the other variants after the shared cleanup
  change. Both successful reruns have complete reports and cleanup, with no skipped, interrupted or unstarted cases. The
  approval fixture checks focus ownership using an exempt native button; it does not test agent approval delivery.

An intermediate invocation used the anchored grep `^save as template refusal respects approval focus$` and selected no
tests: run `22c26b93-f117-4333-98af-7f702125ac45` exited with a selection error. All failed evidence is retained
privately. The later passes do not erase those failures. Reused: 28 unaffected cases from the initial run; the
intervening edits changed only the eight parameterized held-save fixture executions, all of which were rerun
successfully.

Also passed: native `cargo build`; `dx build --package farhelm-ui --platform web --release`;
`cargo clippy -p farhelm-ui --all-targets -- -D warnings`; `cargo fmt --all -- --check`; `dprint check TODO.md`;
`python3 releasing/check-changelog.py format`; and the isolated interpreter's `python -B scripts/check-test-sleeps.py`
(315 delays, no missing rationale). Dprint has no TypeScript plugin and excludes fragments, so it supplies no formatting
evidence for those files. The separate changelog lint covers the fragment.

Runtime evidence used the working diff on main `be12ebac`; the successful reruns share the final source fingerprint. It
applies to PR commit `06caff7da5253a2e908f3ebdd1a4a25e27fb4e33`: every upstream diff through `4456ed59` was inspected
and contains only queue bookkeeping and reports, and the rebased patch is byte-identical. Builds and static checks
remain applicable because the final fixture corrections changed no product code or delay calls.

Skipped: full Rust/browser batteries, native desktop smoke, installer and release checks. This is a local shared UI
interaction covered by focused browser tests on both renderer families; no desktop shell or backend behavior changed. No
instruction file, heading or file location changed, so the agent-instruction checker was not needed.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort received the full charter, acceptance criteria and
verbatim test-authoring checklist. Its findings about usable focus owners, the unavailable Launch fixture and the
deferred observation boundary were accepted and corrected. It also checked the runtime-discovered fixture correction and
reported no remaining actionable findings. Its review was static; the executor verified runtime evidence separately.

The required fresh scope reassessment found no unnecessary complexity. Commit and PR wording passed a separate cold
read. A separate documentation pass covered every touched file. No findings were rejected, guarantees weakened or scope
expanded.

### Landing

Landed as built: in the session launcher's "save as template" panel, Escape now closes only the panel, even when
keyboard focus was lost (for example after a refused save), instead of closing the whole launcher and discarding the
setup; a refused save also gives focus back to the template name unless the user had moved it elsewhere.

- #2709: preserve launcher drafts on template panel escape.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained only the Mac installer and uninstall stack, which does not touch the launcher. Two
other plans in this round change the launcher's template handling: restart-with-template-refusal (templates repeating a
value Restart with keeps fixed) and report-failed-settings-saves (the ⚡ quick-launch mark in the Templates dialog). A
fresh-context reviewer and I read them together: this plan changes saving a template from the launcher (in New, Clone
and Restart with alike), while restart-with-template-refusal changes applying one, and neither of the others changes the
save panel or its Escape handling. The reviewer found no blocker and nothing that needs your decision.

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

The report's checks were the save-panel, launcher focus and neighbouring Escape tests; this round ran the whole browser
suite with the other six plans, and every test in this plan's spec files passed there. Nothing in the executor's report
became untrue at landing.
