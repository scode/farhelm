## What this was about

Several choices could look saved after the helm refused them: terminal-frame colors, quick-launch naming, terminal
link-click behavior, trust-hint visibility and the quick-launch mark. The maintainer chose to report these failures and
discard the failed choice. The welcome card remains a deliberate silent exception; a failed dismissal can make it appear
on a later load.

## Things you should know

Settings now shows the failure beside the affected control and restores its saved choice. A newer click supersedes an
older result. Closing Settings drops its alert while shared choices still revert. Reopening during a pending trust-hint
save cannot keep that failed choice alive in the reopened dialog or the session's hint.

The quick-launch mark waits for its save, like number keys. The Templates dialog remains busy until the result and any
corrective read finish. A lost reply can follow a committed save, so a failure restores the earlier display and checks
the helm before claiming which mark is stored.

A failed Don't show again save brings back the trust hint with an error while its session is still shown. Leaving the
session drops that error. If reading the trust setting also fails, the error appears alone; the hint's explanation and
actions wait until the setting is known to be off. No failed write is retried or replayed.

Execution found two ways an older trust choice could return: reopening Settings before a pending save failed, and a read
started before a save finished arriving afterward. Both now refresh from the helm rather than retaining the old choice.
Overlapping clicks preserve the newest choice, and an older result cannot undo it. Errors stay beside the affected
control or session.

## Open questions and possible follow-ups

None required for this plan. Device-local sound, sidebar width and text-size saves remain outside its scope.

## PRs

- [#2713 — report failed Settings saves](https://github.com/scode/farhelm/pull/2713/changes).
- [#2714 — report failed quick-launch mark saves](https://github.com/scode/farhelm/pull/2714/changes).
- [#2715 — report failed trust-hint dismissals](https://github.com/scode/farhelm/pull/2715/changes).

## Checks run, reused and skipped

All runtime commands used `scripts/record-test-run.py`. Controlled Rust runs used four nextest slots and zero retries.
Browser runs used pinned tmux, Chromium and WebKit, three isolated stacks and zero retries. Successful records have
complete source and reports, retained output through EOF, no runtime `SKIPPED`, zero command/recorder exits and complete
cleanup.

- Preference queue and save-order tests: 40 passed in run `232fada3-141b-432e-9187-befb4a2438c2`, selecting
  `cargo nextest run -p farhelm-ui --lib -E 'test(api::preference_write_tests::) | test(api::save_order::)'`. Reused
  from source `c7be2a20` because later changes only add UI reporting, tests and prose and leave that queue unchanged.
- Settings and trust regressions: 78 distinct cases, 39 per engine, passed across complementary runs
  `25dc5bc1-c52d-4915-a78d-9bcee73a373e` (eight crossing-read cases) and `a5335392-14f6-4fc9-a9a6-fca4ad987c0f` (the
  other 70). These cover PR1's final behavior; later trust-band reporting receives its own final checks below.

  From `e2e`, the eight-case selection was
  `npx playwright test '(^|/)(settings|claude-trust)\.spec\.ts$' -g 'settings repairs a (Claude|Codex) trust read crossing a successful hint save|Claude trust band (discards a newer Settings overlay after its origin closes|repairs an idle read crossing a Settings dismissal save)'`.
  The complementary command selected `(^|/)(settings|claude-trust|workspace-trust)\.spec\.ts$` with `--grep-invert` and
  that same grep expression. Both used the same corrected PR1 source, later committed as `60562058`.
- Template binding refusals, quick-launch mark refusal and recovery, held/failed mark launch behavior and the restart
  boundary: 16 passed, eight per engine, run `73960f94-6061-4b6a-b43a-a6e8e7bc86a1`, covering PR2. Reused because PR3
  does not change those paths.

  From `e2e`:
  `npx playwright test '(^|/)(templates|quick-launch|restart-with)\.spec\.ts$' -g 'number keys move|a failed number-key save|a refused quick-launch mark|quick launch (discards a failed|honors a held) local mark|restart with refuses held shortcut batches and bolt templates'`.
  This covered the final PR2 files committed as `b5f46443`.
- Final session trust-hint lifecycle, refusal/recovery/navigation and welcome behavior: 36 passed, 18 per engine, run
  `73961841-b18b-4d46-afe5-f4be1039cc88`. From `e2e`:
  `npx playwright test '(^|/)(claude-trust|workspace-trust|welcome)\.spec\.ts$'`. This covers the final PR3 source
  committed as `6702488c`, including unchanged welcome dismissal remaining silent.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed for each PR, including the final band changes.
  `cargo fmt --all`, `dprint` on the changed specification/TODO files, and `python3 releasing/check-changelog.py format`
  passed. The isolated source-delay checker inspected 314 delays with zero missing rationales.
- `cargo build -p farhelm -p farhelm-fixtures` passed at final source `6702488c`; all outputs were already current. This
  identifies the same unchanged backend and fixture inputs used by the browser runs. The earlier prerequisite build's
  exact invocation was not retained, so it is not counted as a separate reproducible check. Each behavioral UI stage
  received a fresh `dx build --package farhelm-ui --platform web --release` from `crates/farhelm-ui`. PR2 reused its
  build across a source-reviewed equivalent `match` to `if let` correction.
- Retained run `75642bb0-e103-4231-9a76-637bd5523386` demonstrated the pre-fix reopened-dialog bug. Run
  `3d390011-6518-4d3f-9c8e-5f990952a76f` failed on new fixture assertions expecting HTTP 200 rather than the actual
  successful 204 and was interrupted; none of its partial passes is counted above. Those assertions were corrected
  before the successful eight-plus-70 runs. Three source-check attempts exhausted their time budget before the later
  complete pass. These same-session failures remain retained privately and are not latent-flake claims.
- Full Rust/browser batteries, desktop runtime, installer, provisioning and release gates were skipped: this stack
  changes client preference outcomes, and focused queue/UI coverage addresses its risks. No hosted CI was requested;
  ordinary PR pushes do not start it.

## Review gate outcome

Each PR received the required separate fresh-context `gpt-6.1-sol` review at high effort, with follow-ups to that PR's
same reviewer. PR1's review found a checkbox that could retain its clicked appearance after refusal and an older trust
read that could restore a stale choice; both were fixed and the reviewer cleared the final diff. Fresh scope assessments
kept those fixes within the required newest-choice and failed-save behavior. PR2 passed with no remaining findings,
including the final equivalent conditional cleanup. PR3 passed with no findings. Commit/PR wording received separate
cold reads. All PRs remain drafts for the landing monitor.

### Landing

Landed with one test fixed. When the helm refuses a save of terminal-frame colors, naming sessions after a quick launch,
what clicking a link in a terminal does, trust-hint visibility or the ⚡ quick-launch mark, the error now shows beside
the control and the control returns to the previously saved choice, instead of the attempted choice looking saved.

- #2713: report failed settings saves.
- #2714: report failed quick-launch mark saves.
- #2715: report failed trust hint dismissals.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained only the Mac installer and uninstall stack, which touches none of these settings.
A fresh-context reviewer and I read all seven plans together. Three others share files with this one:
standard-link-style edits another test in the settings browser spec, and restart-with-template-refusal and
template-panel-escape change the launcher's template handling next to the ⚡ mark this plan changes. Those changes sit
in separate code paths, and the SPEC.md and SPEC_impl.md changes do not contradict each other. The reviewer found no
blocker and nothing that needs your decision.

#### Fixes made at landing

- With #2714, the Templates dialog stays busy while a ⚡ mark save is pending, as the report says. An older quick-launch
  test, "deletion drains an in-flight quick mark before clearing it", still clicked Delete during that pending save and
  so waited on a disabled button until it timed out, on both engines, every time; the executor's selected tests did not
  include it. Its guarantee is unchanged (deleting a template never overtakes the mark's own save, then Undo brings back
  the template without its mark), but the dialog now provides it by keeping Delete unavailable until the save settles.
  The test now checks that Delete is unavailable while the mark's reply is held, and that once the reply arrives Delete
  clears the mark and Undo restores the template unmarked. It is renamed "deletion waits for an in-flight quick mark
  before clearing it". A fresh-context reviewer checked the change against the test rules. This changes the test, not
  the app.

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

The report's checks were selected settings, trust, template and quick-launch tests; this round ran the whole browser
suite with the other six plans, which is where the out-of-date test showed. Nothing else in the executor's report became
untrue at landing.
