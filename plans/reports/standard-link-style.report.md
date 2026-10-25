## What this was about

The help link in settings used the browser's default blue against a dark dialog and was hard to read. The maintainer
chose one standard treatment for app links: a readable blue, always underlined, with the same appearance after a visit.
Buttons presented as inline links share it, and links opening pages outside the app end in ↗.

That treatment now covers settings help, the welcome card's guide and feedback actions, and Add host's SSH help action.
A global rule also styles future anchors. The settings help link gains the external-page marker.

## Things you should know

Links have their own color role, separate from selection and status. The underline makes links identifiable without
relying on color alone. Keyboard focus keeps its existing accent ring. Terminal links, menus, the docs website and
frozen release screenshots are unchanged. The matching TODO entry is removed and a fixed changelog fragment is included.

The visited and unvisited states share the same rule. Browser privacy hides visited styles from computed-style reads, so
equality was checked in the stylesheet review rather than claimed from a browser assertion.

## Open questions and possible follow-ups

None identified within this plan's scope.

## The PRs

- [#2710: make app links readable](https://github.com/scode/farhelm/pull/2710/changes), open draft.

## Checks run, reused and skipped

- `npx playwright test 'settings\.spec\.ts' 'welcome\.spec\.ts' 'ssh-help\.spec\.ts' --grep 'prepared checkout settings preserve saved choices across reads, refusals and close|welcome links, modal isolation and short-window dismissal|send feedback now records seen and hands focus to feedback|add host opens ssh help for its draft without losing the form'`,
  through the recorder, selecting the prepared-checkout settings read/refusal/close case, welcome
  links/modal/short-window case, feedback handoff, and Add host's SSH help draft/focus case. Run
  `a75315d3-f0e3-4f09-876e-a1aca65e7dee`: eight passed, four each on Chromium and WebKit, zero failures or skips, zero
  retries, three worker-owned stacks and required pinned tmux. Source capture, runner policy, report and retained output
  are complete. Assertions cover all four rendered link surfaces, resting and hovered style, and the shared keyboard
  ring. The welcome screenshot was also visually checked.
- `node --test` in the UI asset JavaScript harness, through the recorder: 333 passed, zero failures or skips. Run
  `2f62720c-f308-45b3-8294-c25c17507558` has complete source capture and untruncated output. Its contrast and token
  checks cover the new link role on the actual dialog background.
- Native `cargo build`, Dioxus's `dx build --package farhelm-ui --platform web --release`, and
  `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed. The initial web-build attempt selected an unrelated
  `dx` executable on PATH and did not build the app; using the installed Dioxus CLI explicitly succeeded.
- `cargo fmt --all -- --check`, dprint on the changed Markdown, and `python3 releasing/check-changelog.py format`
  passed. Dprint has no CSS, JavaScript or TypeScript plugin and excludes fragments, so it supplies no formatting
  evidence for those files; the separate changelog lint covers the fragment. The isolated interpreter's
  `python -B scripts/check-test-sleeps.py` inspected 315 delays with zero missing rationale.

The checks covered main `2a7b20c7` plus this plan's complete working diff. They and the independent review apply to PR
commit `0127d474e957a043fd0183c4d512072f5b4895b4`: the rebased patch is byte-identical, and every upstream change
through `3b071c8a` was inspected and contains only queue bookkeeping and another plan's report. No older runtime results
were reused; these current checks were retained across that bookkeeping-only rebase without repeating them.

The full Rust/browser batteries, desktop smoke and installer/release checks were skipped because all current visible
link surfaces and their existing actions are covered by the focused browser run on both renderer families, with no
desktop shell or backend change. No instruction file, heading or file location changed, so the agent-instruction checker
was not needed.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no findings in correctness, design,
idiomatic code or tests. It received the full charter, acceptance criteria and verbatim test-authoring checklist. The
commit and PR wording passed a separate cold read. A separate documentation pass covered every touched file. There were
no rejected findings, scope departures or weakened guarantees.

### Landing

Landed as built: links in the app now share one readable blue and stay underlined, visited or not; buttons shown as
inline links share the look, and links that open a page outside the app end in ↗. That covers the settings help link
that was hard to read, the welcome card's guide link and its "Send feedback now" button, and Add host's SSH help button.

- #2710: make app links readable.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained only the Mac installer and uninstall stack, which has no UI styling. A
fresh-context reviewer and I read all seven plans together. The new style applies to every link in the app, and the app
has only two real links (the settings help link and the welcome card's guide); no other plan in this round adds one.
report-failed-settings-saves edits another test in the same settings spec, and both pass together. The reviewer found no
blocker and nothing that needs your decision.

#### Fixes made at landing

None.

#### Possible follow-ups

- The reviewer noted that the new link look wins over the app's normal button look, so a link drawn as a button in
  future (for example a "Read the docs" button that is really a link) would show blue and underlined instead. No such
  link exists today; making the link look apply only where nothing else styles the element would prevent it if one is
  ever added.

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

The report's checks were the four link surfaces' browser tests and the asset script tests; this round ran the whole
browser suite and the asset script tests with the other six plans. Nothing in the executor's report became untrue at
landing.
