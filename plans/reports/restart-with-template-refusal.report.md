## What this was about

Restart with refused a template whenever it named a choice the session keeps fixed, even when it repeated the current
value. Every template saved from the session launcher records its tab, so those saved templates could not be reused
there. Replace with also refused a template naming the session's own host.

The maintainer chose to accept equal fixed choices as no-ops while keeping real changes atomic refusals. Templates and
their shortcuts now follow that rule in Restart with, and Replace with accepts its own host. Other edits in the same
template still apply. A template containing only equal fixed choices applies silently and leaves Restart inactive.

## Things you should know

Matching an agent never reselects it, so an older session's unset permissions, model and effort survive unless the
template explicitly edits them. Host matching requires a usable recorded identity. Directory matching compares the same
text ignoring trailing slashes, without expanding `~` or resolving symlinks; session names compare with their raw text
exactly. Initial messages and clearing a command session's resume choice remain refused.

Search results and recent setups retain their previous presence-based refusal in Restart with. The helm's
`farhelm spawn` behavior is unchanged. A template or shortcut still leaves the restart action to the user. The product
and implementation specs and the Launch templates docs page now describe the chosen behavior. The TODO entry is removed
and a fixed changelog fragment is included.

## Open questions and possible follow-ups

None identified within this plan's scope.

## The PRs

- [#2711: reuse matching templates in Restart with](https://github.com/scode/farhelm/pull/2711/changes), open draft.

## Checks run, reused and skipped

- Focused UI draft and launcher unit tests through the recorder, using
  `cargo nextest run -p farhelm-ui --lib -E 'test(list::create_form::draft::tests::) | test(launch_composer::)'`: run
  `7e880ce3-7cf6-4b2f-b34b-7521c2f5d20a` passed 111 selected tests, with 654 cases selected out, zero failures, errors
  or flaky outcomes, four slots and zero retries. Source capture, report and retained output are complete; required
  pinned nextest and tmux were verified, and no runtime `SKIPPED` marker appeared.
- Browser validation through the recorder, selecting
  `npx playwright test 'restart-with\.spec\.ts|templates\.spec\.ts|launcher-template-shortcuts\.spec\.ts' -g '[Rr]estart.with|Replace with accepts its own template host|Replace with refuses a whole sequence'`:
  run `6989fb84-d0e6-4dc2-8154-416642ed4311` passed all 110 cases, 55 on Chromium and 55 on WebKit, with zero failures,
  skips, flaky outcomes, interruptions or global errors. Three worker-owned stacks, zero retries and required pinned
  tmux were used. Source capture, runner policy, report and retained output are complete, with stream EOF observed and
  no runtime `SKIPPED` marker. This covers the full Restart with spec and the affected Replace with template cases.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed. Its first attempt rejected the expanded result
  tuple's type complexity; a documented type alias resolved it without changing behavior. The release Dioxus web build
  passed.
- `cargo fmt --all -- --check`, dprint on the changed specs, TODO and docs page, and
  `python3 releasing/check-changelog.py format` passed. Dprint excludes TypeScript and fragments, so it provides no
  formatting evidence for those; the separate fragment lint covers the changelog. The isolated interpreter's
  `python -B scripts/check-test-sleeps.py` inspected 315 delays with zero missing rationales.

The runtime records capture main `3b071c8a` plus this plan's working diff. The Rust record's source fingerprint is
`cebfaa103af21272ee2dc62e79ef3166f421bac77e989fc61d660b61a340b7e2`; the browser record's is
`40302fd6cac63375c1e761062e1567f81e267db20bc16349ea418722071d8300`. The only later Rust change is the result type alias;
the Rust tests and built web behavior remain applicable. The browser run uses the corrected fixture that changes effort
while repeating kind and agent. The native backend build is reused from `b2b31b184ee5d5db3102b18ae18f968befe3c264`: this
plan changes no backend executable behavior, and the shared protocol edit is documentation only.

The final PR head is `fd35b64452629c43359f263762e1288d5a6fc8e4`. Its patch is the reviewed patch rebased onto
`e9a28b7f`; the full upstream delta contains only another plan's queue delivery and report, so it has no executable,
specification or test interaction. Each check is retained across that bookkeeping-only rebase without repeating it.

The full Rust and browser batteries, desktop smoke, installer and release checks were skipped because the changed shared
launcher path is covered by focused unit tests and both browser engine families, with no desktop-shell or backend
behavior change. The website build was skipped for this incidental wording-only edit, which changes no links, headings,
frontmatter or generated artifacts. No instruction file, heading or file location changed, so the agent-instruction
checker was not needed. No preview server was started for the incidental docs edit.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort received the complete charter, goal, settled
decisions and verbatim test-authoring checklist. It found one browser fixture that combined the equal-agent permission
check with an explicit model edit, which legitimately changes those permissions. Removing that model edit retained the
required launcher-shaped kind/agent/effort case and isolated the intended guarantee. The same reviewer checked the
correction and type alias and passed the gate with no unresolved findings. There were no rejected findings, scope
departures or weakened guarantees. Commit and PR wording received a separate cold read, and every touched file received
a separate documentation pass.

### Landing

Landed as built: in Restart with, a template, applied by name or through its ⚡ or number-key shortcut, that repeats a
value the restart keeps fixed (the launcher tab, agent, host, folder or session name) now applies, with the repeated
value treated as a no-op, and only an actual change is refused; Replace with likewise accepts a template naming the
session's own host. Templates saved from the launcher record those fixed values, so before this they could never be
reused in Restart with. The Launch templates docs page now describes this.

- #2711: reuse matching templates in restart with.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained only the Mac installer and uninstall stack, which does not touch the launcher. A
fresh-context reviewer and I read all seven plans together. template-panel-escape changes saving a template from the
launcher, where this plan changes applying one, and report-failed-settings-saves changes how the ⚡ mark saves in the
Templates dialog; this plan's ⚡ and number-key tests in Restart with passed alongside both. The reviewer found no
blocker and nothing that needs your decision. TODO.md conflicted mechanically with the earlier plans in this round,
which removed neighbouring entries; the resolution removes only this plan's entry.

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

The report's checks were the launcher's draft unit tests and the Restart with and affected Replace with browser tests;
this round ran the UI package's tests, the website build (the Launch templates docs page changed) and the whole browser
suite with the other six plans. Nothing in the executor's report became untrue at landing.
