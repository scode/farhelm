# Replace YOLO Cancel race

## What this was about

Clicking Cancel immediately after “don't ask again” on a session's Replace YOLO question could hide the question while
the Replace continued. If saving the host setting failed, nothing was replaced but the reason had nowhere visible to
appear. The agreed fix was to refuse Cancel while that Replace runs: the host setting write cannot be undone.

## Things you should know

Cancel now does nothing during the save and Replace, including clicks dispatched before the buttons appear disabled. A
failed save leaves the question open with its reason and sends no confirmed Replace. Once the request settles, Cancel
works normally. The sidebar's question keeps its existing behavior.

The regression test reaches the question through Replace at the top of the session, avoiding the interrupted card's
known click flake. It verifies that Cancel was still enabled and connected when clicked in the same burst as the first
answer. The fix remains a guard on this session's running state; unrelated operations do not disable an idle question's
Cancel. The matching TODO entry is removed and a changelog fragment is included.

## Open questions and possible follow-ups

None identified. The agreed scope and behavior are complete.

## The PRs

- [#2707: keep replace yolo save errors visible](https://github.com/scode/farhelm/pull/2707/changes), draft.

## Checks run, reused and skipped

Run on commit `b2b31b184ee5d5db3102b18ae18f968befe3c264` or identical source content before its commit description was
set:

- Recorded Playwright selection across `terminal-restart.spec.ts` and `yolo-guard.spec.ts`: the new Cancel burst and
  neighbouring session/sidebar Replace YOLO answers, hostless questions and captured preconditions. Run
  `142a417e-1c7a-413d-848f-02dde06560ae`: 18 passed, nine per engine on Chromium and WebKit, zero failed or skipped,
  zero retries, three worker-owned stacks and required pinned tmux. The report and source capture are complete.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings` and the wasm32 web-feature UI Clippy run passed, checking
  the shared handler under both compile configurations.
- Native `cargo build` and Dioxus's release web build passed, preparing the actual application driven by the browser
  run.
- `cargo fmt --all -- --check`, dprint on changed Markdown, changelog format, and the isolated test-delay checker
  passed; the latter inspected 315 delays with zero missing rationale. Dprint excludes browser TypeScript, so it
  supplies no TypeScript formatting evidence.

Reused: no earlier runtime results. Skipped: the full Rust/browser batteries and native desktop smoke, because this
surface-local handler change is covered by the focused browser integration run on both renderer families, and both UI
compile configurations passed. No desktop shell behavior changed.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no issues in correctness, design, idiomatic
code or test quality. It received the plan's acceptance criteria and the full test-authoring checklist. The commit/PR
wording passed its separate cold read. A separate documentation pass covered every touched file.

### Landing

Landed as built. When Replace on a session asks whether to replace in YOLO mode, answering "don't ask again" saves the
host's setting to replace with YOLO without asking, then replaces. Cancel now does nothing while that save and the
Replace run, even when both clicks arrive at once, so a failed save keeps the question open with its reason instead of
vanishing.

- #2707: keep replace yolo save errors visible.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained only the Mac installer and uninstall stack, which does not touch Replace. A
fresh-context reviewer and I read all seven plans together. report-failed-settings-saves also changes how failed saves
are reported, but for Settings choices, trust hints and the ⚡ mark, not this question's host setting, and its
SPEC_impl.md changes agree with this plan's. restart-with-template-refusal changes which templates Replace with (the
session launcher's replace mode, a different flow from this question) accepts. The reviewer found no blocker and nothing
that needs your decision.

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

The report's checks were the Replace YOLO question tests on both engines; this round ran the whole browser suite with
the other six plans, and they passed there. Nothing in the executor's report became untrue at landing.
