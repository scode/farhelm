## Since the last review

Only the existing session-launcher PR, #2762, changes behavior in this return. Enter in its directory field now closes
suggestions when it submits, whether the launch succeeds or is refused. The specification and existing changelog
fragment say so. The regression test keeps its ordinary click on the command tab after a server refusal and now
explicitly checks that suggestions are gone and the typed directory is preserved. No PR was added or dropped; the other
three drafts were rebased without changing their diffs.

The desktop overlap is a consequence of this plan's layout change: the launcher previously had two columns and now has
one, placing controls below the directory field under its floating suggestions. The latest Decision accepts that overlap
while suggestions are open, with a near-term TODO for reconsideration. The correction here makes submission dismiss them
consistently so a refused launch leaves those controls reachable. It adds no new focus controller or launch path.

The required exact test passed 20 times on each engine. All seven complete launcher/sidebar spec files then passed
354/354 cases, 177 per engine, with no skips. The two-hour whole battery was not rerun.

## What this was about

Settings, session setup, confirmations and menus had inconsistent layouts. This plan applies Steel and Steel Light:
Settings section icons and headings, consistent menu descriptions and destructive grouping, shared dialog headers and
answers, and labeled launcher fields with actions pinned below them. Host dialogs already using the shared structure
were reused.

Long launcher forms scroll above the footer. Long warnings and the inline YOLO question use their existing bounded
scrolling area. The footer keeps the target and choice summaries beside a compact Launch, Replace or Restart action.
Native choice controls, request guards, initial focus, Escape and focus return retain their existing owners. Tab follows
the approved visual reading order: header Close, fields in their existing relative order, then
Cancel-first/main-action-last footer actions. Trash retains its pinned header and no footer.

## Things you should know

The previous return fixed the original 34 launcher timeouts by dismissing directory suggestions during ordinary test
setup, with the typed directory checked afterward. The dedicated popup test retains its actual selection, blur, Tab,
Escape, typed Enter and phone-pointer checks. This return corrects a separate product boundary exposed at landing:
suggestions could remain open after a refused Enter, covering the next control in the new single-column layout.

Earlier fresh review also found a product bug at short phone heights: stacked actions could squeeze the inline YOLO
answer region until Cancel was clipped. The existing footer actions now wrap through that height range; normal button
sizes, notice scrolling and native focus ownership remain. Real answer visibility was checked at heights 480, 481, 500,
600, 601 and 844 in both engines.

The other original returned failures were corrected tests for changed names, labels and presentation: Trash, Settings
heading color, the compact Browse label, the named-conversation grid, compact launch text and conditional warning
elements. The actual warning text and its accessible association remain. File-drop setup now waits for its own terminal
attachment before the real upload, retaining content and terminal-isolation assertions. The WebKit host-switch failures
followed a directory test that timed out before restoring its preference; unchanged switch cases passed after that setup
was corrected.

The prior whole run also exposed fixture defects in build-help interception cleanup, a losing window's new-tab retry and
a long host-menu scenario. Existing route draining and future-connection withholding fixed the first two without
production terminal changes; only the last scenario's overall budget increased, with readiness and geometry assertions
retained. The established takeover flake record and sweep exclusion were resolved. Later focused passes remain separate
observations from the failed whole runs.

The complete target uses shared hover help, and the main action's accessible name includes it. Updated documentation
callouts point at that summary beside the compact action. Images show the real staged web UI. Native tunnel consent was
reviewed using matching markup/CSS and source, with separate native compilation/controller evidence; web images do not
prove native callbacks or AppKit behavior.

## Open questions and possible follow-ups

No required decision remains. Near-term TODO entries retain the calls on dialog Tab order, Trash's header, desktop
suggestion overlap and close-on-submission for later reconsideration. The broad sentence-case pass and removal of the
umbrella Steel TODO belong to the next plan.

The one-row Rename field can look cramped when showing a rejected pasted name containing a line break. On a short phone,
the bounded YOLO explanation and answers may require scrolling or Tab; a persistent shared scroll cue remains optional.
The footer's second summary can truncate late choices, including permissions after a long model, and has no full-text
help; the choices remain editable and required YOLO confirmation still applies. The target's existing help uses one
prose font, including its path. These earlier accepted limits remain.

## The PRs

- [Settings navigation and headings, #2756](https://github.com/scode/farhelm/pull/2756/changes).
- [Command-menu descriptions and destructive grouping, #2757](https://github.com/scode/farhelm/pull/2757/changes).
- [Dialog layouts and confirmation answers, #2761](https://github.com/scode/farhelm/pull/2761/changes).
- [Session launcher rows and pinned actions, #2762](https://github.com/scode/farhelm/pull/2762/changes).

All remain drafts; the landing monitor owns merging them. Verified pushed heads are Settings
`0318b1333aef4a53065eea021c130ed98b791984`, menus `94d1791d3892949cf14e575aef1e1020869b8c0e`, confirmations
`3e52fe62ac8cc8873be995faaae774d7dbbb59be`, launcher `b160a47c265e9295c2c7ae6fad4fb9d8316c922d`. GitHub shows those
heads and the original linear bases; the working copy is clean. The aggregate reviewed diff was compared byte-for-byte
before and after amendment.

## Checks run, reused and skipped

New browser behavior runs use the recorder, pinned tmux, Chromium and WebKit, three workers and zero retries. Appshots
use the normal staged single-worker fleet. Overlapping selections are separate evidence, not an additive denominator.

- `npx playwright test tests/sidebar.spec.ts -g 'composer folder dropdown preserves keyboard launch and grouped history'`
  passed once per engine, 2/2 (`553331fd-0c8b-457c-b8cd-85435a57a53b`). It checks list closure after the real server
  refusal, preserved typed directory, exactly one request and the unchanged command-tab click.
- From `e2e`,
  `python3 ../scripts/hunt-browser-tests.py --repeat 20 --timeout 180 --execute -- npx playwright test tests/sidebar.spec.ts -g 'composer folder dropdown preserves keyboard launch and grouped history'`
  passed all 40 checks, 20 on each engine, without failures or skips (batch `05f16d9f-767f-43e6-9fe1-e45a1a71b6ee`;
  private output-root argument omitted). Every attempt retained a complete structural report, identical source
  fingerprint, output EOF and clean cleanup. The original exact test and its command-tab click remain unchanged apart
  from stronger premises and closure assertions.
- `npx playwright test tests/sidebar.spec.ts tests/sidebar-resize.spec.ts tests/launcher-enter.spec.ts tests/launcher-tabs.spec.ts tests/launcher-template-shortcuts.spec.ts tests/launch-refusal.spec.ts tests/launch-button-alignment.spec.ts`
  passed 354/354 cases, 177 on each engine, with no failures or skips (`a7a71601-c945-4ea4-876e-6dfb2e1e79de`, 20.2
  minutes). Source identity matches the web build and repetition batch; reports, output EOF and cleanup are complete.
  The recorder enforced both engines, three workers and zero retries without project overrides.
- `dx build --package farhelm-ui --platform web --release` passed (`e1c14e7c-d778-4c12-84b3-439f8447bef9`). Its complete
  source fingerprint is `c4005d7d6c691b1e207472fcf85faa5c70e9abd6ea72f20d95ed4f1f9d39ca7e`, shared by the first exact
  regression run. `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  `dprint check SPEC.md releasing/changelog.d/steel-dialogs.md`, `python3 releasing/check-changelog.py format` and
  `git diff --check` passed. The isolated pinned interpreter ran `python -B scripts/check-test-sleeps.py`: 322 delays,
  none missing a rationale.
- Normal `scripts/docs-screenshots.sh --only visual-review --no-build` passed staging and four palette/width legs,
  producing 76 launcher images (`d7d2fdad-5435-4bf6-b829-a2e425502191`). It uses the same complete source fingerprint as
  the build and browser checks. All images were inspected locally; eight before/after views went through the new
  independent visual gate. Nothing was published.

Reused evidence covers unchanged behavior and surfaces. The previously delivered stack ended at
`430eadd860b4950883d47ffd3403563f11719993`; the fresh four-unit rebase had empty per-unit interdiffs. Intervening main
changes were queue/goal/review records and TODO decisions, with no product, spec or dependency change. This return's
five-path delta adds only the submit-time suggestion closure, its specification, regression assertions, capture
transition and fragment wording. Unchanged `cargo build -p farhelm -p farhelm-fixtures`
(`1ac54cb7-5ea8-4d75-b88c-7c8381473e7f`), UI JavaScript `node --test` (358/358, `8885b05e-de95-4146-97c8-4cb4ea5b1a51`),
`cargo nextest run -p farhelm-ui --lib` (805, `8177fa16-a156-4da9-b519-d9fa67857d00`),
`cargo nextest run -p farhelm-ui --features desktop --lib -E 'test(tunnels::tests::)'` (six,
`728ebe9f-0b39-42ea-a442-4f1df70b34fd`) and
`cargo nextest run -p farhelm-ui --lib -E 'test(list::create_form::tests::)'` (52,
`780e0381-c6f0-481b-95a7-99d9d4bcbc72`) remain relevant to their unchanged code. Those Rust runs used the recorder's
four-slot, zero-retry policy. The older pure-test runs covered the earlier stack ending
`618e6d0013b90b1c3909c7b61d1cdd5918f26ea9`; later product changes have their separately identified browser/build/visual
evidence. No old unit pass is claimed to exercise this new DOM submission behavior.

The earlier required `npx playwright test` whole browser run, `1c66d7f4-79b2-41bf-b6f1-225f1fc10cf6`, failed with 2,764
passes, three assertion failures, one timeout and 14 skips. Its original recorder summary exceeded the 16 MiB collection
cap and remains incomplete; a separate bounded 32 MiB audit reconciled the retained policy and report, whose SHA256 is
`8fceec953c29640c7c1073392fe6c0343bc7085ec3d1d9b031db042cc6acb4a1`. All original returned failures passed. After the
four later fixture failures were corrected, `npx playwright test <affected specs> -g <affected cases>` passed 28 checks
(`f7e0bf2f-a13f-48c0-bc16-b7beb93179bc`), followed by both engines for a final cleanup correction
(`8a67a3b8-5454-424c-82ce-d39874864984`). These do not turn the failed whole run into a pass.

At the next landing, `npx playwright test` whole run `e25b17a4-5f3e-447b-9816-b89164862de2` failed the WebKit folder
test. Generic recorded
`npx playwright test --project=webkit-sidebar --project=chromium-sidebar tests/sidebar.spec.ts -g 'composer folder dropdown preserves keyboard launch and grouped history' --repeat-each=5 --retries=0`
(`a187dab9-7564-4944-9202-4f19fe0cae8d`) reproduced it once in five WebKit attempts while Chromium passed five. Both
retain complete source identity and terminal output. The whole run's original summary again exceeded its collection cap;
its raw totals are diagnostic, not a newly validated pass denominator. Those retained failures are the reason for this
correction and the new repetition checks.

Another whole suite and unrelated Rust, installer, release, desktop and CI batteries were skipped: the latest Decision
specifically requires repeated exact behavior plus launcher/sidebar coverage and says the rest stands. The new event
boundary has matching build and browser evidence; unchanged logic retains the checks above. No macOS/AppKit runtime
capture was performed and no live installation was changed. No images were published.

## Review gate outcome

Each original unit retains its exact single gpt-6.1-sol/high code and claude-opus-5-5/high visual gate for unchanged
surfaces. A fresh single Sol6.1/high reviewer read the full current launcher PR and the five-path correction and
reported no actionable findings. Fresh own-model scope review supported using the existing form-submit boundary without
another key handler or speculative focus policy.

The fresh single claude-opus-5-5/high visual reviewer inspected all eleven requested images: eight new before/after
views across both palettes and widths, plus three rendered references. Actual successful image delivery and text-read
completeness were audited, and its terminal result and complete findings were checked. It reported no required fixes.
The reviewer noted the accepted open-popup overlap and optional elevation styling for a later revisit; existing refusal
casing belongs to the next sentence-case plan. Static images establish neither timing nor server refusal, which have
separate browser evidence, and imply no native callback or AppKit coverage. The other 68 unchanged views retain their
earlier visual gates.

### Landing

Landed on the third delivery. Settings, menus, confirmations and the session launcher now follow Steel: dialogs share a
header, a scrolling body and a pinned footer with Cancel first and the main action last; Settings sections have icons;
command menus describe their actions and group destructive ones; the launcher groups its fields into labeled rows in one
column, with its target summary and buttons in a footer that stays visible while the fields scroll.

- #2756: Settings navigation icons and consistent headings.
- #2757: command-menu descriptions and destructive grouping.
- #2761: shared dialog layout and confirmations.
- #2762: the restyled session launcher.

#### Decisions made on your behalf

The landing monitor answered this plan's three blocks under your instruction to make the call unless an issue is huge.
Two of them needed judgment calls; each call is recorded as a near-term TODO entry for you to revisit:

- The dialogs' Tab order follows what is on screen: header Close, the fields, then the footer with Cancel first and the
  main action last. The inline YOLO question takes the new answer order only. Trash keeps its totals, progress, Delete
  all and Close in its pinned header instead of a footer.
- On desktop the open folder suggestions in the launcher now cover the controls below them until dismissed (Escape, Tab
  or a click outside the list), because the launcher is one column. A click aimed at the agent buttons can instead pick
  a suggestion such as "local home", which resets the folder to home on the local host; the footer's target shows that
  before Launch. Accepted for this plan.
- When Enter in the folder field submits a launch, the folder suggestions close, whether the launch is accepted or
  refused, so a refused launch leaves the launcher's controls reachable. SPEC.md says so.

#### What landed since the stack was based, and whether it interacted

Nothing but plan bookkeeping and TODO entries landed since the plan was based, so nothing interacted.

The first landing attempt found 57 browser failures, mostly launcher tests whose clicks the open folder suggestions
intercepted; the second found one intermittent failure from the same overlap. A fresh-context reviewer read each
delivery's fixes, found them honest with no assertion weakened, and the final review found nothing that must be fixed.
On the way the executor also found and fixed a real product bug: at some phone heights the YOLO question's Cancel was
clipped. It also closed the open flake about a browser tab opened unaware of a takeover (TODO.md's Deflake entry), and
raised one host-menu test's time budget from 60 to 120 seconds after a single WebKit failure, with no flake entry.

#### Fixes made at landing

None.

#### Things you should know

- A regression of the new close-on-Enter rule would be caught by the regular browser suite only some of the time; the
  check that catches it every time is in the visual-review screenshot spec, which only runs for visual reviews. A
  deterministic regular test is a possible follow-up.
- The executor's explanation of why the suggestions only sometimes stayed open before the fix (whether the engine takes
  focus off the folder field when the launcher goes busy, which also closes them) fits the evidence but was not shown
  separately; it is also why the regular suite catches a regression only some of the time.
- A launcher test no longer guarantees that the launcher fits a 1440×900 window without scrolling; the pinned footer
  keeps Launch reachable instead. The main YOLO-question test ("a YOLO launch asks first and starts only when
  confirmed") now runs at six phone heights instead of the default desktop window, so that check no longer runs at
  desktop size.
- The docs website's start-a-session and launch-templates pages still describe the old Launch button, and two pages
  still put the gear "at the top of the sidebar"; they change with "refresh the docs".

#### Checks

This round landed only this plan. On the stack as delivered:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo check -p farhelm-desktop`, the check that every test delay states its reason, `dprint check` across the
  repository, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, the workspace debug
  build, the UI's release web build and `scripts/check-desktop-assets.sh`: all passed.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `7c95094e-d252-47f0-b87f-93aaf5463bbd`: all 805 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `2b2fa1ef-1b0e-4937-8a86-64dd2e99a263`: all 988 passed. The UI's JavaScript and stylesheet tests (`node --test` in
  `crates/farhelm-ui/js-tests`), run `d673ef91-973c-4136-bedf-9e4905f0bc3e`: 358 passed.
- `npx playwright test` in `e2e/` with the launcher-related specs (`sidebar`, `launcher-enter`, `yolo-guard`,
  `github-checkout-composer`, `remembered-permissions`, `destination-authority`, `goose-pi-composer`, `opencode`,
  `templates`), on Chromium and WebKit, three workers each with its own helm and supervisors, no retries, run
  `2eef74e1-8969-4543-896c-08231caa0ee1`: all 556 passed.
- The formerly intermittent test "composer folder dropdown preserves keyboard launch and grouped history", repeated 10
  times on each engine, run `f18230dc-8349-41a3-8732-4dd89a1844d0`: all 20 passed.
- Reused: the whole browser suite on Chromium and WebKit from the previous landing attempt, run
  `e25b17a4-5f3e-447b-9816-b89164862de2` (2767 passed, 14 skipped by design, 1 failed: the folder test above), on the
  previous delivery. This delivery differs from it only in the launcher closing the folder suggestions when Enter
  submits a launch (six lines), a SPEC.md sentence, the changelog draft, the sidebar test and the visual-review
  screenshot spec; the regular launcher specs ran again above, and the visual-review spec, which checks the new rule
  every time, ran only in the executor's own checks.
- Skipped: the rest of the Rust workspace (only the UI crate changed, and only the desktop app depends on it), the
  desktop smoke tests (the only desktop-only change is the tunnel consent dialog's layout, covered by the desktop-build
  UI tests above and the desktop app's compile check) and the native Mac app (not runnable here).

The report's checks were the executor's own runs of the changed cases; this landing ran the whole suite at the previous
delivery and the launcher specs at this one. Nothing else in the executor's report became untrue at landing.
